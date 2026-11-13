//! Servizio con stato: una richiesta JSON per riga in ingresso, una risposta JSON per
//! riga in uscita. È il protocollo pensato per gli agenti che lavorano su un progetto
//! per più passi (prova, esegui, annulla) senza riaprire il file a ogni comando.
//!
//! Ogni risposta ha la forma `{"id": …, "ok": true, "risultato": …}` oppure
//! `{"id": …, "ok": false, "errore": {"codice": …, "descrizione": …, …}}`; il campo
//! `id` riporta quello della richiesta, se presente.

use std::collections::VecDeque;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;

use chrono::NaiveDate;
use opencomputo_application::casi_uso::{self, FormatoReport, TipoReport};
use opencomputo_application::{Comando, ErroreApplicazione, Politica, Sessione, viste};
use opencomputo_core::aggiornamento::SostituzionePrefisso;
use opencomputo_io::report::OpzioniReport;
use opencomputo_storage::{Compressione, OpzioniSalvataggio};
use serde::Deserialize;
use serde_json::{Value, json};

use opencomputo_assistente::provider::Provider;
use opencomputo_assistente::scelta::Scelta;
use opencomputo_assistente::verifica;

use crate::assistente::{
    self, Ambiente, Assistente, Avanzamento, Bisogno, EsitoLavoro, Interruttore, Lavoro, TipoLavoro,
};
use crate::{riepilogo, schema};

/// Risultati mostrati da `cerca` se non indicato diversamente.
pub const LIMITE_RICERCA: usize = 20;

/// Richiesta al servizio.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "richiesta", rename_all = "snake_case", deny_unknown_fields)]
pub enum Richiesta {
    /// Apre un file di progetto.
    Apri {
        /// Percorso del file `.ocp`.
        percorso: PathBuf,
    },
    /// Crea un progetto vuoto (salvato solo con `salva`).
    Nuovo {
        /// Titolo.
        titolo: String,
        /// Percorso in cui salvarlo.
        #[serde(default)]
        percorso: Option<PathBuf>,
    },
    /// Crea un progetto da un file XPWE o DCF.
    Importa {
        /// File di origine.
        origine: PathBuf,
    },
    /// Salva il progetto (scrittura atomica con copie di sicurezza).
    Salva {
        /// Percorso; se assente quello di apertura.
        #[serde(default)]
        percorso: Option<PathBuf>,
        /// Compressione gzip; se assente si conserva quella del file aperto.
        #[serde(default)]
        gzip: Option<bool>,
    },
    /// Stato sintetico del progetto.
    Info {},
    /// Validazione completa.
    Valida {},
    /// Esegue i comandi (richiesta atomica).
    Esegui {
        /// Comandi.
        comandi: Vec<Comando>,
    },
    /// Simula i comandi senza modificare il progetto.
    Prova {
        /// Comandi.
        comandi: Vec<Comando>,
    },
    /// Annulla l'ultima richiesta eseguita.
    Annulla {},
    /// Ripristina l'ultima richiesta annullata.
    Ripristina {},
    /// Cerca voci nell'elenco prezzi del progetto o in un prezzario caricato.
    Cerca {
        /// Parole da cercare.
        testo: String,
        /// Nome del prezzario caricato; se assente l'elenco prezzi del progetto.
        #[serde(default)]
        prezzario: Option<String>,
        /// Numero massimo di voci restituite.
        #[serde(default)]
        limite: Option<usize>,
    },
    /// Carica un prezzario di consultazione (DCF, XPWE o file di progetto).
    CaricaPrezzario {
        /// Nome con cui riferirlo nei comandi.
        nome: String,
        /// File.
        percorso: PathBuf,
    },
    /// Esporta un computo in XPWE (sperimentale).
    EsportaXpwe {
        /// File di uscita.
        percorso: PathBuf,
        /// Nome del computo.
        #[serde(default)]
        computo: Option<String>,
    },
    /// Esporta l'elenco prezzi in DCF (sperimentale).
    EsportaDcf {
        /// File di uscita.
        percorso: PathBuf,
        /// Data del documento, se il progetto non ne ha una (AAAA-MM-GG).
        #[serde(default)]
        data: Option<NaiveDate>,
    },
    /// Produce un report.
    Report {
        /// Tipo di report, es. `{"tipo": "sal", "numero": 1}`.
        report: TipoReport,
        /// Formato (predefinito PDF).
        #[serde(default)]
        formato: Option<FormatoReport>,
        /// Cartella di uscita (nomi dei file predefiniti).
        #[serde(default)]
        cartella: Option<PathBuf>,
        /// File di uscita esatto, per i formati che producono un solo file (PDF, XLSX).
        #[serde(default)]
        file: Option<PathBuf>,
        /// Data di stampa a piè di pagina.
        #[serde(default)]
        data_stampa: Option<NaiveDate>,
    },
    /// Anteprima di un report: lo stesso documento di PDF, XLSX e CSV, con le celle
    /// già formattate.
    Anteprima {
        /// Tipo di report.
        report: TipoReport,
        /// Data di stampa a piè di pagina.
        #[serde(default)]
        data_stampa: Option<NaiveDate>,
        /// Modello tradizionale: prima pagina da restituire (da 1; predefinita 1).
        #[serde(default)]
        prima_pagina: Option<usize>,
        /// Modello tradizionale: quante pagine restituire (predefinite 6, al massimo 40).
        #[serde(default)]
        pagine: Option<usize>,
    },
    /// Dati generali: metadati, contratto, regole, categorie, WBS, capitoli, computi.
    Progetto {},
    /// Elenco prezzi del progetto o di un prezzario caricato, filtrato per testo.
    ElencoPrezzi {
        /// Nome del prezzario caricato; se assente l'elenco prezzi del progetto.
        #[serde(default)]
        prezzario: Option<String>,
        /// Parole da cercare; se assente tutte le voci.
        #[serde(default)]
        testo: Option<String>,
    },
    /// Confronto dell'elenco prezzi e delle risorse con un prezzario caricato (nuovo
    /// listino): differenze, variazioni e importi dei computi, per scegliere le voci da
    /// aggiornare con il comando `aggiorna_da_prezzario`.
    ConfrontoPrezzario {
        /// Nome del prezzario caricato.
        prezzario: String,
        /// Sostituzione del prefisso dei codici (es. `{"da": "TOS23_", "a": "TOS24_"}`).
        #[serde(default)]
        prefisso: Option<SostituzionePrefisso>,
    },
    /// Dettaglio di una voce di elenco prezzi, con la descrizione estesa.
    VocePrezzario {
        /// Codice.
        codice: String,
        /// Nome del prezzario caricato; se assente l'elenco prezzi del progetto.
        #[serde(default)]
        prezzario: Option<String>,
    },
    /// Computo con voci, righi, quantità e importi.
    Computo {
        /// Nome del computo (facoltativo se ce n'è uno solo).
        #[serde(default)]
        computo: Option<String>,
    },
    /// Libretto delle misure e SAL con i prospetti.
    Contabilita {},
    /// Cronoprogramma con il piano calcolato: date, margini, percorso critico,
    /// produzione mensile.
    Cronoprogramma {},
    /// Risorse, parametri e analisi dell'elenco prezzi.
    AnalisiPrezzi {},
    /// Nuovi prezzi e verbali di concordamento.
    NuoviPrezzi {},
    /// Fabbisogno di cantiere di un computo (dalle analisi).
    Fabbisogno {
        /// Nome del computo (facoltativo se ce n'è uno solo).
        #[serde(default)]
        computo: Option<String>,
    },
    /// Regole ed esempi dei comandi.
    SchemaComandi {},
    /// Chiude il progetto; con modifiche non salvate richiede `forza`.
    Chiudi {
        /// Scarta le modifiche non salvate.
        #[serde(default)]
        forza: bool,
    },
    /// Assistente AI: configurazione, chiavi presenti, consenso e suo testo.
    AssistenteStato {},
    /// Assistente AI: modifica la configurazione (modelli, dimensioni, limiti).
    AssistenteConfigura {
        /// Campi da modificare.
        configurazione: assistente::Modifiche,
    },
    /// Assistente AI: salva la chiave di un provider nel portachiavi del sistema;
    /// chiave vuota o assente: la elimina.
    AssistenteChiave {
        /// Provider (`openai` o `anthropic`).
        provider: Provider,
        /// Chiave API.
        #[serde(default)]
        chiave: Option<assistente::ChiaveApi>,
    },
    /// Assistente AI: accetta (con la versione del testo mostrato) o revoca il consenso
    /// all'invio dei dati ai provider.
    AssistenteConsenso {
        /// `true` per accettare, `false` per revocare.
        accetto: bool,
        /// Versione del testo accettato (obbligatoria per accettare).
        #[serde(default)]
        versione: Option<u32>,
    },
    /// Assistente AI: calcola gli embedding mancanti di un prezzario caricato (invia le
    /// descrizioni delle voci a OpenAI).
    AssistenteIndicizza {
        /// Nome del prezzario caricato.
        prezzario: String,
    },
    /// Assistente AI: cerca nel prezzario caricato; ibrida se l'indice vettoriale è
    /// completo (invia il testo a OpenAI), altrimenti solo testuale e locale.
    AssistenteCerca {
        /// Nome del prezzario caricato.
        prezzario: String,
        /// Testo da cercare.
        testo: String,
        /// Numero massimo di voci.
        #[serde(default)]
        limite: Option<usize>,
        /// Grandezza attesa (`m2`, `m3`, …) per filtrare le unità di misura.
        #[serde(default)]
        unita: Option<String>,
    },
    /// Assistente AI: propone voci di computo dalla richiesta (invia richiesta, progetto
    /// sintetico e candidati ad Anthropic). Il progetto non cambia.
    AssistenteProponi {
        /// Nome del prezzario caricato da cui scegliere le voci.
        prezzario: String,
        /// Richiesta in linguaggio naturale.
        testo: String,
        /// Computo di destinazione (facoltativo se ce n'è uno solo).
        #[serde(default)]
        computo: Option<String>,
    },
    /// Assistente AI: valori di tutte le opzioni dell'ultima proposta con le misure
    /// indicate, e prova delle opzioni scelte. Il progetto non cambia.
    AssistenteAnteprima {
        /// Scelte: per lavorazione, i codici spuntati e le misure corrette.
        scelte: Vec<Scelta>,
    },
    /// Assistente AI: aggiunge al computo le opzioni scelte dell'ultima proposta (un passo
    /// annullabile). Senza `scelte`, quelle preselezionate. La proposta resta disponibile.
    AssistenteApplica {
        /// Scelte.
        #[serde(default)]
        scelte: Option<Vec<Scelta>>,
    },
    /// Assistente AI: stato dell'indice di un prezzario caricato (voci, voci indicizzate,
    /// tipo di ricerca possibile), senza chiamate.
    AssistenteIndice {
        /// Nome del prezzario caricato.
        prezzario: String,
    },
    /// Assistente AI: annulla l'operazione lunga in corso (indicizzazione, ricerca,
    /// proposta). Utile solo quando il servizio esegue i lavori in parallelo (interfaccia
    /// grafica); il servizio a righe le esegue una per volta.
    AssistenteAnnulla {},
}

impl Richiesta {
    /// `true` per le operazioni lunghe dell'assistente, eseguibili come [`Lavoro`].
    #[must_use]
    pub fn e_lavoro(&self) -> bool {
        matches!(
            self,
            Self::AssistenteIndice { .. }
                | Self::AssistenteIndicizza { .. }
                | Self::AssistenteCerca { .. }
                | Self::AssistenteProponi { .. }
        )
    }
}

fn richiesta(messaggio: impl Into<String>) -> ErroreApplicazione {
    ErroreApplicazione::Richiesta {
        messaggio: messaggio.into(),
    }
}

fn errore_file(percorso: &Path, e: &io::Error) -> ErroreApplicazione {
    ErroreApplicazione::File {
        messaggio: format!("{}: {e}", percorso.display()),
    }
}

/// Legge un file intero.
///
/// # Errors
/// File non leggibile.
pub fn leggi(percorso: &Path) -> Result<Vec<u8>, ErroreApplicazione> {
    std::fs::read(percorso).map_err(|e| errore_file(percorso, &e))
}

/// `true` se il file esiste ed è compresso con gzip.
fn compresso(percorso: &Path) -> bool {
    let mut testa = [0u8; 2];
    std::fs::File::open(percorso)
        .and_then(|mut f| f.read_exact(&mut testa))
        .is_ok()
        && testa == [0x1f, 0x8b]
}

fn valore(v: &impl serde::Serialize) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Elenco prezzi del progetto o prezzario caricato per nome.
fn prezzario_di<'a>(
    s: &'a Sessione,
    nome: Option<&str>,
) -> Result<&'a opencomputo_core::prezzario::Prezzario, ErroreApplicazione> {
    match nome {
        None => Ok(&s.progetto().elenco_prezzi),
        Some(nome) => s
            .prezzari()
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, p)| p)
            .ok_or_else(|| richiesta(format!("prezzario caricato «{nome}» inesistente"))),
    }
}

impl Richiesta {
    /// `true` se la richiesta, quando riesce, può cambiare il progetto aperto o lo
    /// stato della sessione (usato dalla GUI per aggiornare le altre finestre).
    #[must_use]
    pub fn modifica_stato(&self) -> bool {
        matches!(
            self,
            Self::Apri { .. }
                | Self::Nuovo { .. }
                | Self::Importa { .. }
                | Self::Salva { .. }
                | Self::Esegui { .. }
                | Self::Annulla {}
                | Self::Ripristina {}
                | Self::CaricaPrezzario { .. }
                | Self::Chiudi { .. }
                | Self::AssistenteApplica { .. }
        )
    }
}

/// Errore in forma JSON: il codice e i campi dell'errore, più la descrizione leggibile.
#[must_use]
pub fn errore_json(e: &ErroreApplicazione) -> Value {
    let mut v = serde_json::to_value(e).unwrap_or_else(|_| json!({"codice": "interno"}));
    if let Value::Object(m) = &mut v {
        m.insert("descrizione".into(), Value::String(e.to_string()));
    }
    v
}

/// Risposta completa per un esito.
#[must_use]
pub fn risposta(id: Option<Value>, esito: Result<Value, ErroreApplicazione>) -> Value {
    let mut v = match esito {
        Ok(r) => json!({"ok": true, "risultato": r}),
        Err(e) => json!({"ok": false, "errore": errore_json(&e)}),
    };
    if let (Some(id), Value::Object(m)) = (id, &mut v) {
        m.insert("id".into(), id);
    }
    v
}

/// Stato del servizio: al più un progetto aperto.
#[derive(Default)]
pub struct Servizio {
    sessione: Option<Sessione>,
    percorso: Option<PathBuf>,
    gzip: bool,
    politica: Politica,
    /// Assistente AI, creato alla prima richiesta `assistente_*`.
    assistente: Option<Assistente>,
    /// Ambiente dell'assistente; se assente quello del sistema.
    ambiente: Option<Ambiente>,
    avanzamento: Option<Avanzamento>,
    interruttore: Interruttore,
    /// Cresce a ogni cambio di progetto: un lavoro concluso su un altro progetto non
    /// lascia proposte da applicare.
    generazione: u64,
    /// Cresce a ogni prezzario caricato e a ogni cambio di progetto.
    caricamenti: u64,
}

/// Primo passo di una richiesta: risposta pronta oppure lavoro da eseguire.
#[derive(Debug)]
pub enum Passo {
    /// Risposta e indicazione di cambiamento (come [`Servizio::rispondi_valore`]).
    Risposta {
        /// Risposta completa.
        risposta: Value,
        /// `true` se lo stato può essere cambiato.
        cambiato: bool,
    },
    /// Operazione lunga dell'assistente: da eseguire (anche in un altro thread) e poi da
    /// concludere con [`Servizio::completa`].
    Lavoro(Box<Lavoro>),
}

impl std::fmt::Debug for Servizio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Servizio")
            .field("aperto", &self.sessione.is_some())
            .field("percorso", &self.percorso)
            .field("gzip", &self.gzip)
            .finish_non_exhaustive()
    }
}

impl Servizio {
    /// Servizio senza progetto aperto, con la politica predefinita.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Politica dei comandi per i progetti aperti da qui in poi.
    #[must_use]
    pub fn con_politica(mut self, politica: Politica) -> Self {
        self.politica = politica;
        self
    }

    /// Ambiente dell'assistente AI (cartella, portachiavi, trasporto); senza questa
    /// chiamata, alla prima richiesta `assistente_*` si usa [`Ambiente::di_sistema`].
    #[must_use]
    pub fn con_ambiente(mut self, ambiente: Ambiente) -> Self {
        self.ambiente = Some(ambiente);
        self.assistente = None;
        self
    }

    /// Funzione che riceve gli avanzamenti delle operazioni lunghe dell'assistente
    /// (oggetti JSON con il campo `avanzamento`).
    #[must_use]
    pub fn con_avanzamento(mut self, f: Avanzamento) -> Self {
        self.avanzamento = Some(f);
        self
    }

    fn assistente(&mut self) -> Result<&mut Assistente, ErroreApplicazione> {
        if self.assistente.is_none() {
            let ambiente = self.ambiente.take().unwrap_or_else(Ambiente::di_sistema);
            self.assistente = Some(Assistente::new(ambiente)?);
        }
        self.assistente
            .as_mut()
            .ok_or_else(|| richiesta("assistente non disponibile"))
    }

    /// Interruttore delle operazioni lunghe dell'assistente: si può usare da un altro
    /// thread mentre un [`Lavoro`] è in corso.
    #[must_use]
    pub fn interruttore(&self) -> Interruttore {
        self.interruttore.clone()
    }

    fn gestisci_assistente(&mut self, r: Richiesta) -> Result<Value, ErroreApplicazione> {
        match r {
            Richiesta::AssistenteStato {} => self.assistente()?.stato(),
            Richiesta::AssistenteConfigura { configurazione } => {
                self.assistente()?.configura(configurazione)
            }
            Richiesta::AssistenteChiave { provider, chiave } => self
                .assistente()?
                .imposta_chiave(provider, chiave.as_ref().map(assistente::ChiaveApi::valore)),
            Richiesta::AssistenteConsenso { accetto, versione } => {
                self.assistente()?.consenso(accetto, versione)
            }
            Richiesta::AssistenteAnteprima { scelte } => {
                self.aperta()?;
                let p = self.assistente()?.proposta()?.clone();
                Ok(valore(&verifica::anteprima(self.aperta()?, &p, &scelte)?))
            }
            Richiesta::AssistenteApplica { scelte } => {
                self.aperta()?;
                let p = self.assistente()?.proposta()?.clone();
                let scelte = scelte.unwrap_or_else(|| p.scelte_predefinite());
                let (comandi, voci) = verifica::comandi(self.aperta()?, &p, &scelte)?;
                let mut v = valore(&self.aperta_mut()?.esegui(&comandi)?);
                if let Value::Object(m) = &mut v {
                    m.insert("voci".into(), valore(&voci));
                }
                Ok(v)
            }
            Richiesta::AssistenteAnnulla {} => {
                let lavori = self.interruttore.annulla();
                Ok(json!({"annullamento": "chiesto", "lavori": lavori}))
            }
            r if r.e_lavoro() => {
                let l = self.prepara(r, None)?;
                let e = l.esegui(self.avanzamento.as_deref().map(|f| f as &dyn Fn(&Value)));
                self.concludi(e)
            }
            _ => Err(richiesta("richiesta non dell'assistente")),
        }
    }

    /// Prepara un lavoro: controlla progetto, prezzario, consenso e chiavi, e copia i dati.
    fn prepara(&mut self, r: Richiesta, id: Option<Value>) -> Result<Lavoro, ErroreApplicazione> {
        let (nome, tipo, bisogno) = match r {
            Richiesta::AssistenteIndice { prezzario } => {
                (prezzario, TipoLavoro::Indice, Bisogno::Niente)
            }
            Richiesta::AssistenteIndicizza { prezzario } => {
                (prezzario, TipoLavoro::Indicizza, Bisogno::Embedding)
            }
            Richiesta::AssistenteCerca {
                prezzario,
                testo,
                limite,
                unita,
            } => (
                prezzario,
                TipoLavoro::Cerca {
                    testo,
                    limite: limite.unwrap_or(LIMITE_RICERCA),
                    unita,
                },
                Bisogno::Niente,
            ),
            Richiesta::AssistenteProponi {
                prezzario,
                testo,
                computo,
            } => {
                if testo.trim().is_empty() {
                    return Err(assistente::errore(
                        "richiesta_vuota",
                        "la richiesta è vuota",
                    ));
                }
                if testo.chars().count() > assistente::RICHIESTA_MASSIMA {
                    return Err(assistente::errore(
                        "richiesta_troppo_lunga",
                        format!(
                            "la richiesta supera {} caratteri: dividerla in più richieste",
                            assistente::RICHIESTA_MASSIMA
                        ),
                    ));
                }
                (
                    prezzario,
                    TipoLavoro::Proponi { testo, computo },
                    Bisogno::Generazione,
                )
            }
            _ => return Err(richiesta("richiesta non eseguibile come lavoro")),
        };
        let (prezzario, progetto) = {
            let s = self.aperta()?;
            let p = prezzario_di(s, Some(&nome))?.clone();
            let progetto = matches!(tipo, TipoLavoro::Proponi { .. }).then(|| s.progetto().clone());
            (p, progetto)
        };
        let chiave = self.caricamenti;
        let attrezzi = self.assistente()?.attrezzi(&nome, chiave, bisogno)?;
        Ok(Lavoro {
            id,
            generazione: self.generazione,
            tipo,
            attrezzi,
            nome,
            prezzario,
            progetto,
            segnale: self.interruttore.nuovo(),
        })
    }

    /// Conclude un lavoro: per le proposte, anteprima delle scelte iniziali sul progetto
    /// aperto, e proposta conservata per `assistente_anteprima` e `assistente_applica`.
    fn concludi(&mut self, e: EsitoLavoro) -> Result<Value, ErroreApplicazione> {
        let mut v = e.risultato?;
        if !e.proposta_attesa {
            return Ok(v);
        }
        if e.generazione != self.generazione || self.sessione.is_none() {
            return Err(assistente::errore(
                "progetto_cambiato",
                "il progetto aperto è cambiato durante la proposta: ripetere la richiesta",
            ));
        }
        if let Some(p) = &e.proposta {
            let anteprima = match verifica::anteprima(self.aperta()?, p, &p.scelte_predefinite()) {
                Ok(a) => valore(&a),
                Err(err) => json!({"errore": errore_json(&err)}),
            };
            if let Value::Object(m) = &mut v {
                m.insert("anteprima".into(), anteprima);
            }
        }
        self.assistente()?.conserva_proposta(e.proposta);
        Ok(v)
    }

    /// Gestisce una richiesta JSON fino al primo passo: le operazioni lunghe
    /// dell'assistente tornano come [`Passo::Lavoro`], da eseguire senza tenere il
    /// servizio (l'interfaccia grafica resta libera) e da concludere con
    /// [`Servizio::completa`].
    pub fn avvia(&mut self, mut v: Value) -> Passo {
        let id = v.as_object_mut().and_then(|m| m.remove("id"));
        let r = match serde_json::from_value::<Richiesta>(v) {
            Ok(r) => r,
            Err(e) => {
                return Passo::Risposta {
                    risposta: risposta(id, Err(richiesta(format!("richiesta non valida: {e}")))),
                    cambiato: false,
                };
            }
        };
        if r.e_lavoro() {
            return match self.prepara(r, id.clone()) {
                Ok(l) => Passo::Lavoro(Box::new(l)),
                Err(e) => Passo::Risposta {
                    risposta: risposta(id, Err(e)),
                    cambiato: false,
                },
            };
        }
        let modifica = r.modifica_stato();
        let esito = self.gestisci(r);
        let cambiato = modifica && esito.is_ok();
        Passo::Risposta {
            risposta: risposta(id, esito),
            cambiato,
        }
    }

    /// Conclude un lavoro eseguito e restituisce la risposta (i lavori non cambiano il
    /// progetto).
    pub fn completa(&mut self, e: EsitoLavoro) -> (Value, bool) {
        let id = e.id.clone();
        (risposta(id, self.concludi(e)), false)
    }

    /// Sessione aperta.
    #[must_use]
    pub fn sessione(&self) -> Option<&Sessione> {
        self.sessione.as_ref()
    }

    /// Percorso del file del progetto aperto.
    #[must_use]
    pub fn percorso(&self) -> Option<&Path> {
        self.percorso.as_deref()
    }

    fn aperta(&self) -> Result<&Sessione, ErroreApplicazione> {
        self.sessione
            .as_ref()
            .ok_or_else(|| richiesta("nessun progetto aperto: usare `apri`, `nuovo` o `importa`"))
    }

    fn aperta_mut(&mut self) -> Result<&mut Sessione, ErroreApplicazione> {
        self.sessione
            .as_mut()
            .ok_or_else(|| richiesta("nessun progetto aperto: usare `apri`, `nuovo` o `importa`"))
    }

    fn sostituisci(
        &mut self,
        s: Sessione,
        percorso: Option<PathBuf>,
    ) -> Result<(), ErroreApplicazione> {
        if self.sessione.as_ref().is_some_and(Sessione::modificato) {
            return Err(richiesta(
                "il progetto aperto ha modifiche non salvate: usare `salva` oppure `chiudi` con `forza`",
            ));
        }
        self.gzip = percorso.as_deref().is_some_and(compresso);
        self.sessione = Some(s.con_politica(self.politica.clone()));
        self.generazione = self.generazione.wrapping_add(1);
        self.caricamenti = self.caricamenti.wrapping_add(1);
        if let Some(a) = self.assistente.as_mut() {
            a.progetto_cambiato();
        }
        self.percorso = percorso;
        Ok(())
    }

    /// Gestisce una richiesta.
    ///
    /// # Errors
    /// Richiesta non applicabile; in ogni caso lo stato resta coerente e il progetto
    /// aperto non viene modificato da una richiesta fallita.
    pub fn gestisci(&mut self, r: Richiesta) -> Result<Value, ErroreApplicazione> {
        match r {
            Richiesta::Apri { percorso } => {
                if self.sessione.as_ref().is_some_and(Sessione::modificato) {
                    return Err(richiesta(
                        "il progetto aperto ha modifiche non salvate: usare `salva` oppure `chiudi` con `forza`",
                    ));
                }
                let s = Sessione::apri(&percorso)?;
                self.sostituisci(s, Some(percorso))?;
                self.info()
            }
            Richiesta::Nuovo { titolo, percorso } => {
                self.sostituisci(Sessione::nuovo(&titolo), None)?;
                self.percorso = percorso;
                self.info()
            }
            Richiesta::Importa { origine } => {
                let byte = leggi(&origine)?;
                let nome = origine.file_name().and_then(|n| n.to_str());
                let (s, rapporto) = if byte.starts_with(b"AAMVHFSS") {
                    casi_uso::importa_dcf(&byte, nome)?
                } else {
                    casi_uso::importa_xpwe(&byte, nome)?
                };
                self.sostituisci(s, None)?;
                Ok(json!({"importazione": rapporto, "progetto": self.info()?}))
            }
            Richiesta::Salva { percorso, gzip } => self.salva(percorso, gzip),
            Richiesta::Info {} => self.info(),
            Richiesta::Valida {} => Ok(riepilogo::valida(self.aperta()?)),
            Richiesta::Esegui { comandi } => {
                Ok(serde_json::to_value(self.aperta_mut()?.esegui(&comandi)?)
                    .unwrap_or(Value::Null))
            }
            Richiesta::Prova { comandi } => {
                Ok(serde_json::to_value(self.aperta()?.prova(&comandi)?).unwrap_or(Value::Null))
            }
            Richiesta::Annulla {} => {
                let passo = self.aperta_mut()?.annulla()?;
                Ok(json!({"annullato": passo}))
            }
            Richiesta::Ripristina {} => {
                let passo = self.aperta_mut()?.ripristina()?;
                Ok(json!({"ripristinato": passo}))
            }
            Richiesta::Cerca {
                testo,
                prezzario,
                limite,
            } => {
                let s = self.aperta()?;
                let pr = prezzario_di(s, prezzario.as_deref())?;
                Ok(riepilogo::cerca(
                    pr,
                    &testo,
                    limite.unwrap_or(LIMITE_RICERCA),
                ))
            }
            Richiesta::CaricaPrezzario { nome, percorso } => {
                let byte = leggi(&percorso)?;
                let (prezzario, rapporto) = casi_uso::leggi_prezzario(
                    &byte,
                    percorso.file_name().and_then(|n| n.to_str()),
                )?;
                let voci = prezzario.voci.len();
                self.aperta_mut()?.carica_prezzario(&nome, prezzario);
                self.caricamenti = self.caricamenti.wrapping_add(1);
                Ok(json!({
                    "nome": nome,
                    "formato": rapporto.formato,
                    "voci": voci,
                    "anomalie": rapporto.anomalie.len(),
                }))
            }
            Richiesta::EsportaXpwe { percorso, computo } => {
                let x = casi_uso::esporta_xpwe(self.aperta()?.progetto(), computo.as_deref())?;
                std::fs::write(&percorso, x.xml.as_bytes())
                    .map_err(|e| errore_file(&percorso, &e))?;
                Ok(json!({
                    "percorso": percorso,
                    "byte": x.xml.len(),
                    "anomalie": x.anomalie,
                }))
            }
            Richiesta::EsportaDcf { percorso, data } => {
                let x = casi_uso::esporta_dcf(self.aperta()?.progetto(), data)?;
                std::fs::write(&percorso, &x.byte).map_err(|e| errore_file(&percorso, &e))?;
                Ok(json!({
                    "percorso": percorso,
                    "byte": x.byte.len(),
                    "voci": x.voci,
                    "capitoli": x.capitoli,
                    "anomalie": x.anomalie,
                }))
            }
            Richiesta::Report {
                report,
                formato,
                cartella,
                file,
                data_stampa,
            } => {
                let prodotti = casi_uso::report(
                    self.aperta()?.progetto(),
                    &report,
                    formato.unwrap_or(FormatoReport::Pdf),
                    &OpzioniReport { data_stampa },
                )?;
                let destinazioni: Vec<PathBuf> = match (file, cartella, prodotti.as_slice()) {
                    (Some(f), _, [_]) => vec![f],
                    (Some(_), _, _) => {
                        return Err(richiesta(
                            "il formato produce più file: indicare `cartella` invece di `file`",
                        ));
                    }
                    (None, Some(c), _) => {
                        std::fs::create_dir_all(&c).map_err(|e| errore_file(&c, &e))?;
                        prodotti.iter().map(|f| c.join(&f.nome)).collect()
                    }
                    (None, None, _) => return Err(richiesta("indicare `cartella` oppure `file`")),
                };
                let mut scritti = Vec::new();
                for (f, p) in prodotti.iter().zip(destinazioni) {
                    std::fs::write(&p, &f.byte).map_err(|e| errore_file(&p, &e))?;
                    scritti.push(json!({"percorso": p, "byte": f.byte.len()}));
                }
                Ok(json!({"file": scritti}))
            }
            Richiesta::Anteprima {
                report,
                data_stampa,
                prima_pagina,
                pagine,
            } => {
                // Modello tradizionale: le pagine del PDF, in SVG.
                if let Some(a) = casi_uso::anteprima_tradizionale(
                    self.aperta()?.progetto(),
                    &report,
                    &OpzioniReport { data_stampa },
                    prima_pagina.unwrap_or(1),
                    pagine.unwrap_or(6).clamp(1, 40),
                )? {
                    return Ok(valore(&a));
                }
                let (nome, doc) = casi_uso::documento(
                    self.aperta()?.progetto(),
                    &report,
                    &OpzioniReport { data_stampa },
                )?;
                Ok(valore(&viste::anteprima(nome, &doc)))
            }
            Richiesta::Progetto {} => Ok(valore(&viste::progetto(self.aperta()?.progetto()))),
            Richiesta::ElencoPrezzi { prezzario, testo } => {
                let s = self.aperta()?;
                let pr = prezzario_di(s, prezzario.as_deref())?;
                Ok(valore(&viste::elenco(pr, testo.as_deref())))
            }
            Richiesta::ConfrontoPrezzario {
                prezzario,
                prefisso,
            } => {
                let s = self.aperta()?;
                let listino = prezzario_di(s, Some(&prezzario))?;
                Ok(valore(&viste::confronto_prezzario(
                    s.progetto(),
                    &prezzario,
                    listino,
                    prefisso.as_ref().filter(|p| !p.da.is_empty()),
                )))
            }
            Richiesta::VocePrezzario { codice, prezzario } => {
                let s = self.aperta()?;
                let pr = prezzario_di(s, prezzario.as_deref())?;
                Ok(valore(&viste::voce(pr, &codice, &s.progetto().regole)?))
            }
            Richiesta::Computo { computo } => Ok(valore(&viste::computo(
                self.aperta()?.progetto(),
                computo.as_deref(),
            )?)),
            Richiesta::Contabilita {} => Ok(valore(&viste::contabilita(self.aperta()?.progetto()))),
            Richiesta::Cronoprogramma {} => {
                Ok(valore(&viste::cronoprogramma(self.aperta()?.progetto())))
            }
            Richiesta::AnalisiPrezzi {} => {
                Ok(valore(&viste::analisi_prezzi(self.aperta()?.progetto())))
            }
            Richiesta::NuoviPrezzi {} => {
                Ok(valore(&viste::nuovi_prezzi(self.aperta()?.progetto())))
            }
            Richiesta::Fabbisogno { computo } => Ok(valore(&viste::fabbisogno(
                self.aperta()?.progetto(),
                computo.as_deref(),
            )?)),
            Richiesta::SchemaComandi {} => Ok(schema::schema_comandi()),
            Richiesta::Chiudi { forza } => {
                if !forza && self.sessione.as_ref().is_some_and(Sessione::modificato) {
                    return Err(richiesta(
                        "il progetto ha modifiche non salvate: usare `salva` oppure `forza`",
                    ));
                }
                let chiuso = self.sessione.take().is_some();
                self.percorso = None;
                self.generazione = self.generazione.wrapping_add(1);
                self.caricamenti = self.caricamenti.wrapping_add(1);
                if let Some(a) = self.assistente.as_mut() {
                    a.progetto_cambiato();
                }
                Ok(json!({"chiuso": chiuso}))
            }
            r @ (Richiesta::AssistenteStato {}
            | Richiesta::AssistenteConfigura { .. }
            | Richiesta::AssistenteChiave { .. }
            | Richiesta::AssistenteConsenso { .. }
            | Richiesta::AssistenteIndicizza { .. }
            | Richiesta::AssistenteCerca { .. }
            | Richiesta::AssistenteProponi { .. }
            | Richiesta::AssistenteApplica { .. }
            | Richiesta::AssistenteAnteprima { .. }
            | Richiesta::AssistenteIndice { .. }
            | Richiesta::AssistenteAnnulla {}) => self.gestisci_assistente(r),
        }
    }

    fn info(&self) -> Result<Value, ErroreApplicazione> {
        Ok(riepilogo::info(self.aperta()?, self.percorso.as_deref()))
    }

    fn salva(
        &mut self,
        percorso: Option<PathBuf>,
        gzip: Option<bool>,
    ) -> Result<Value, ErroreApplicazione> {
        let percorso = percorso
            .or_else(|| self.percorso.clone())
            .ok_or_else(|| richiesta("indicare `percorso`: il progetto non ha ancora un file"))?;
        let gzip = gzip.unwrap_or(self.gzip);
        let opzioni = OpzioniSalvataggio {
            compressione: if gzip {
                Compressione::Gzip
            } else {
                Compressione::Nessuna
            },
            ..OpzioniSalvataggio::default()
        };
        let esito = self.aperta_mut()?.salva(&percorso, &opzioni)?;
        self.percorso = Some(percorso.clone());
        self.gzip = gzip;
        Ok(json!({
            "percorso": percorso,
            "byte": esito.byte,
            "gzip": gzip,
            "copia_di_sicurezza": esito.copia_di_sicurezza,
        }))
    }

    /// Gestisce una riga di testo JSON e restituisce la risposta.
    pub fn rispondi(&mut self, riga: &str) -> Value {
        match serde_json::from_str(riga) {
            Ok(v) => self.rispondi_valore(v).0,
            Err(e) => risposta(None, Err(richiesta(format!("JSON non valido: {e}")))),
        }
    }

    /// Gestisce una richiesta già decodificata come JSON. Restituisce la risposta e
    /// `true` se la richiesta è riuscita e può aver cambiato lo stato
    /// ([`Richiesta::modifica_stato`]).
    pub fn rispondi_valore(&mut self, v: Value) -> (Value, bool) {
        match self.avvia(v) {
            Passo::Risposta { risposta, cambiato } => (risposta, cambiato),
            Passo::Lavoro(l) => {
                let e = l.esegui(self.avanzamento.as_deref().map(|f| f as &dyn Fn(&Value)));
                self.completa(e)
            }
        }
    }

    /// Ciclo del servizio: una richiesta per riga, fino alla fine dell'ingresso.
    /// Le righe vuote sono ignorate. Le risposte escono nell'ordine delle richieste.
    ///
    /// L'ingresso è letto da un thread a parte: mentre un lavoro lungo dell'assistente è
    /// in corso, una riga `assistente_annulla` ha effetto subito (la sua risposta esce
    /// comunque dopo quella del lavoro); le altre righe aspettano il loro turno.
    ///
    /// # Errors
    /// Errori di lettura o scrittura dei flussi.
    pub fn servi(
        &mut self,
        ingresso: impl BufRead + Send + 'static,
        mut uscita: impl Write,
    ) -> io::Result<()> {
        /// Una riga letta, oppure la risposta già pronta di un annullamento anticipato.
        enum Voce {
            Riga(io::Result<Vec<u8>>),
            Pronta(Value),
        }
        let (tx, rx) = mpsc::channel::<io::Result<Vec<u8>>>();
        // Il ricevitore passa alla sentinella per la durata di un lavoro.
        let rx = Mutex::new(rx);
        let ricevi = |attesa: Option<Duration>| {
            let rx = rx.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            match attesa {
                Some(d) => rx.recv_timeout(d).ok(),
                None => rx.recv().ok(),
            }
        };
        // Thread non legato al ciclo: se l'uscita si chiude, il servizio termina anche con
        // la lettura ancora in attesa.
        thread::spawn(move || {
            for riga in ingresso.split(b'\n') {
                if tx.send(riga).is_err() {
                    break;
                }
            }
        });
        {
            let mut in_attesa: VecDeque<Voce> = VecDeque::new();
            loop {
                let voce = match in_attesa.pop_front() {
                    Some(v) => v,
                    None => match ricevi(None) {
                        Some(r) => Voce::Riga(r),
                        None => return Ok(()),
                    },
                };
                let r = match voce {
                    Voce::Pronta(v) => v,
                    Voce::Riga(riga) => {
                        // Una riga non UTF-8 riceve un errore, ma il servizio continua.
                        match String::from_utf8(riga?) {
                            Ok(riga) if riga.trim().is_empty() => continue,
                            Ok(riga) => match serde_json::from_str(riga.trim_end_matches('\r')) {
                                Ok(v) => match self.avvia(v) {
                                    Passo::Risposta { risposta, .. } => risposta,
                                    Passo::Lavoro(l) => {
                                        let finito = AtomicBool::new(false);
                                        let interruttore = self.interruttore.clone();
                                        let esito = thread::scope(|lavoro| {
                                            // Sentinella: legge le righe che arrivano
                                            // durante il lavoro e applica gli annullamenti.
                                            let sentinella = lavoro.spawn(|| {
                                                let mut lette = Vec::new();
                                                while !finito.load(Ordering::Acquire) {
                                                    let Some(riga) =
                                                        ricevi(Some(Duration::from_millis(50)))
                                                    else {
                                                        // Anche a ingresso chiuso: nessun giro a vuoto.
                                                        thread::sleep(Duration::from_millis(20));
                                                        continue;
                                                    };
                                                    lette.push(
                                                        annullamento_anticipato(
                                                            &riga,
                                                            &interruttore,
                                                        )
                                                        .map_or(Voce::Riga(riga), Voce::Pronta),
                                                    );
                                                }
                                                lette
                                            });
                                            let e = l.esegui(
                                                self.avanzamento
                                                    .as_deref()
                                                    .map(|f| f as &dyn Fn(&Value)),
                                            );
                                            finito.store(true, Ordering::Release);
                                            in_attesa.extend(sentinella.join().unwrap_or_default());
                                            e
                                        });
                                        self.completa(esito).0
                                    }
                                },
                                Err(e) => {
                                    risposta(None, Err(richiesta(format!("JSON non valido: {e}"))))
                                }
                            },
                            Err(_) => risposta(
                                None,
                                Err(richiesta("riga non valida: il testo non è UTF-8")),
                            ),
                        }
                    }
                };
                serde_json::to_writer(&mut uscita, &r)?;
                uscita.write_all(b"\n")?;
                uscita.flush()?;
            }
        }
    }
}

/// Se la riga è una richiesta `assistente_annulla`, la applica e ne prepara la risposta.
fn annullamento_anticipato(
    riga: &io::Result<Vec<u8>>,
    interruttore: &Interruttore,
) -> Option<Value> {
    let v: Value = serde_json::from_slice(riga.as_ref().ok()?).ok()?;
    let m = v.as_object()?;
    // Solo la forma esatta: qualsiasi altra riga segue il percorso normale, errori compresi.
    if m.get("richiesta")? != "assistente_annulla"
        || m.keys().any(|k| k != "richiesta" && k != "id")
    {
        return None;
    }
    let lavori = interruttore.annulla();
    Some(risposta(
        m.get("id").cloned(),
        Ok(json!({"annullamento": "chiesto", "lavori": lavori})),
    ))
}
