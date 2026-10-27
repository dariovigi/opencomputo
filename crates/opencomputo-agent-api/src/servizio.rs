//! Servizio con stato: una richiesta JSON per riga in ingresso, una risposta JSON per
//! riga in uscita. È il protocollo pensato per gli agenti che lavorano su un progetto
//! per più passi (prova, esegui, annulla) senza riaprire il file a ogni comando.
//!
//! Ogni risposta ha la forma `{"id": …, "ok": true, "risultato": …}` oppure
//! `{"id": …, "ok": false, "errore": {"codice": …, "descrizione": …, …}}`; il campo
//! `id` riporta quello della richiesta, se presente.

use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use opencomputo_application::casi_uso::{self, FormatoReport, TipoReport};
use opencomputo_application::{Comando, ErroreApplicazione, Politica, Sessione, viste};
use opencomputo_core::aggiornamento::SostituzionePrefisso;
use opencomputo_io::report::OpzioniReport;
use opencomputo_storage::{Compressione, OpzioniSalvataggio};
use serde::Deserialize;
use serde_json::{Value, json};

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
            } => {
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
                Ok(json!({"chiuso": chiuso}))
            }
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
    pub fn rispondi_valore(&mut self, mut v: Value) -> (Value, bool) {
        let id = v.as_object_mut().and_then(|m| m.remove("id"));
        let r = match serde_json::from_value::<Richiesta>(v) {
            Ok(r) => r,
            Err(e) => {
                return (
                    risposta(id, Err(richiesta(format!("richiesta non valida: {e}")))),
                    false,
                );
            }
        };
        let modifica = r.modifica_stato();
        let esito = self.gestisci(r);
        let cambiato = modifica && esito.is_ok();
        (risposta(id, esito), cambiato)
    }

    /// Ciclo del servizio: una richiesta per riga, fino alla fine dell'ingresso.
    /// Le righe vuote sono ignorate.
    ///
    /// # Errors
    /// Errori di lettura o scrittura dei flussi.
    pub fn servi(&mut self, ingresso: impl BufRead, mut uscita: impl Write) -> io::Result<()> {
        for riga in ingresso.split(b'\n') {
            let riga = riga?;
            // Una riga non UTF-8 riceve un errore, ma il servizio continua.
            let r = match String::from_utf8(riga) {
                Ok(riga) if riga.trim().is_empty() => continue,
                Ok(riga) => self.rispondi(riga.trim_end_matches('\r')),
                Err(_) => risposta(
                    None,
                    Err(richiesta("riga non valida: il testo non è UTF-8")),
                ),
            };
            serde_json::to_writer(&mut uscita, &r)?;
            uscita.write_all(b"\n")?;
            uscita.flush()?;
        }
        Ok(())
    }
}
