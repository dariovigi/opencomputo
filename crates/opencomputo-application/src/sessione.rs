//! Sessione di lavoro su un progetto: comandi, annulla/ripristina, prova senza applicare.

use std::collections::BTreeSet;
use std::path::Path;

use opencomputo_core::id::{GeneratoreId, GeneratoreSequenziale};
use opencomputo_core::numeri::Importo;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::validazione::{Gravita, Problema, valida_progetto};
use opencomputo_storage::{EsitoSalvataggio, OpzioniSalvataggio};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::comandi::{Comando, EsitoComando, applica};
use crate::errore::ErroreApplicazione;

/// Passi di annullamento conservati per impostazione predefinita.
pub const LIMITE_ANNULLA: usize = 50;

/// Totale di un computo prima e dopo i comandi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariazioneTotale {
    /// Nome del computo.
    pub computo: String,
    /// Totale prima (assente se il computo non esisteva o non era calcolabile).
    pub prima: Option<Importo>,
    /// Totale dopo (assente se il computo non esiste più o non è calcolabile).
    pub dopo: Option<Importo>,
}

/// Esito di una richiesta di comandi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Esito {
    /// `false` per una prova: il progetto non è stato modificato.
    pub applicato: bool,
    /// Esito di ciascun comando.
    pub comandi: Vec<EsitoComando>,
    /// Problemi di validazione comparsi.
    pub nuovi_problemi: Vec<Problema>,
    /// Problemi di validazione scomparsi.
    pub problemi_risolti: Vec<Problema>,
    /// Totali dei computi che sono cambiati.
    pub totali: Vec<VariazioneTotale>,
}

/// Regole applicate ai comandi.
#[derive(Debug, Clone)]
pub struct Politica {
    /// Rifiuta le richieste che introducono nuovi errori di validazione (predefinito).
    /// Gli avvisi sono sempre consentiti e riportati nell'esito.
    pub rifiuta_nuovi_errori: bool,
    /// Passi di annullamento conservati.
    pub limite_annulla: usize,
}

impl Default for Politica {
    fn default() -> Self {
        Self {
            rifiuta_nuovi_errori: true,
            limite_annulla: LIMITE_ANNULLA,
        }
    }
}

struct Passo {
    descrizione: String,
    progetto: Progetto,
}

/// Sessione di lavoro.
///
/// Ogni richiesta (uno o più comandi) è **atomica**: i comandi sono applicati su una
/// copia del progetto e la copia sostituisce l'originale solo se tutti riescono e la
/// validazione lo consente. Ogni richiesta applicata è un passo di annullamento.
pub struct Sessione {
    progetto: Progetto,
    generatore: GeneratoreSequenziale,
    annulla: Vec<Passo>,
    ripristina: Vec<Passo>,
    politica: Politica,
    modificato: bool,
    prezzari: Vec<(String, Prezzario)>,
}

impl std::fmt::Debug for Sessione {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessione")
            .field("titolo", &self.progetto.metadati.titolo)
            .field("annulla", &self.annulla.len())
            .field("ripristina", &self.ripristina.len())
            .field("modificato", &self.modificato)
            .field("prezzari", &self.prezzari.len())
            .finish_non_exhaustive()
    }
}

/// Identificativo più alto usato nel progetto (tutti gli UUID del documento).
fn id_massimo(p: &Progetto) -> u128 {
    fn visita(v: &Value, max: &mut u128) {
        match v {
            Value::String(s) => {
                if s.len() == 36
                    && let Ok(u) = Uuid::parse_str(s)
                {
                    *max = (*max).max(u.as_u128());
                }
            }
            Value::Array(a) => a.iter().for_each(|x| visita(x, max)),
            Value::Object(o) => o.values().for_each(|x| visita(x, max)),
            _ => {}
        }
    }
    let mut max = 0;
    if let Ok(v) = serde_json::to_value(p) {
        visita(&v, &mut max);
    }
    max
}

/// Chiave di confronto di un problema: indipendente dai numeri d'ordine, che cambiano
/// quando si inseriscono o spostano voci.
fn chiave(p: &Problema) -> String {
    fn senza_numeri(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("numero");
                o.values_mut().for_each(senza_numeri);
            }
            Value::Array(a) => a.iter_mut().for_each(senza_numeri),
            _ => {}
        }
    }
    let mut percorso = serde_json::to_value(&p.percorso).unwrap_or(Value::Null);
    senza_numeri(&mut percorso);
    format!("{:?}|{}|{}", p.gravita, p.codice, percorso)
}

fn totali(p: &Progetto) -> Vec<(String, Option<Importo>)> {
    p.computi
        .iter()
        .map(|c| {
            (
                c.nome.clone(),
                p.calcola_computo(c.id).ok().map(|r| r.totale),
            )
        })
        .collect()
}

impl Sessione {
    /// Sessione su un progetto esistente.
    #[must_use]
    pub fn new(progetto: Progetto) -> Self {
        let inizio = id_massimo(&progetto).wrapping_add(1);
        Self {
            progetto,
            generatore: GeneratoreSequenziale::new(inizio),
            annulla: Vec::new(),
            ripristina: Vec::new(),
            politica: Politica::default(),
            modificato: false,
            prezzari: Vec::new(),
        }
    }

    /// Sessione su un progetto nuovo con un computo vuoto.
    #[must_use]
    pub fn nuovo(titolo: &str) -> Self {
        let mut g = GeneratoreSequenziale::default();
        let mut p = Progetto::new(g.nuovo(), titolo, g.nuovo());
        p.elenco_prezzi.nome = "Elenco prezzi di progetto".into();
        let c = opencomputo_core::computo::Computo::new(g.nuovo(), "Computo metrico estimativo");
        p.computi.push(c);
        let mut s = Self::new(p);
        s.modificato = true;
        s
    }

    /// Apre un file di progetto.
    ///
    /// # Errors
    /// Errori di lettura del file.
    pub fn apri(percorso: &Path) -> Result<Self, ErroreApplicazione> {
        Ok(Self::new(opencomputo_storage::apri(percorso)?))
    }

    /// Salva il progetto e segna la sessione come non modificata.
    ///
    /// # Errors
    /// Errori di scrittura (il file precedente resta intatto).
    pub fn salva(
        &mut self,
        percorso: &Path,
        opzioni: &OpzioniSalvataggio,
    ) -> Result<EsitoSalvataggio, ErroreApplicazione> {
        let e = opencomputo_storage::salva(percorso, &self.progetto, opzioni)?;
        self.modificato = false;
        Ok(e)
    }

    /// Imposta la politica.
    pub fn con_politica(mut self, politica: Politica) -> Self {
        self.politica = politica;
        self
    }

    /// Progetto corrente (sola lettura: le modifiche passano dai comandi).
    #[must_use]
    pub fn progetto(&self) -> &Progetto {
        &self.progetto
    }

    /// `true` se ci sono modifiche non salvate.
    #[must_use]
    pub fn modificato(&self) -> bool {
        self.modificato
    }

    /// Descrizioni dei passi annullabili, dal più recente.
    #[must_use]
    pub fn passi_annullabili(&self) -> Vec<&str> {
        self.annulla
            .iter()
            .rev()
            .map(|p| p.descrizione.as_str())
            .collect()
    }

    /// Descrizioni dei passi ripristinabili, dal più recente.
    #[must_use]
    pub fn passi_ripristinabili(&self) -> Vec<&str> {
        self.ripristina
            .iter()
            .rev()
            .map(|p| p.descrizione.as_str())
            .collect()
    }

    /// Carica un prezzario di consultazione (non salvato nel progetto), da cui copiare
    /// voci con il comando `copia_da_prezzario`. Un prezzario con lo stesso nome è sostituito.
    pub fn carica_prezzario(&mut self, nome: &str, prezzario: Prezzario) {
        self.prezzari.retain(|(n, _)| n != nome);
        self.prezzari.push((nome.to_owned(), prezzario));
    }

    /// Prezzari di consultazione caricati.
    #[must_use]
    pub fn prezzari(&self) -> &[(String, Prezzario)] {
        &self.prezzari
    }

    fn calcola(
        &self,
        comandi: &[Comando],
    ) -> Result<(Progetto, GeneratoreSequenziale, Esito), ErroreApplicazione> {
        if comandi.is_empty() {
            return Err(ErroreApplicazione::NessunComando);
        }
        let mut copia = self.progetto.clone();
        let mut g = self.generatore.clone();
        let mut esiti = Vec::with_capacity(comandi.len());
        for (indice, c) in comandi.iter().enumerate() {
            let descrizione = applica(&mut copia, c, &mut g, &self.prezzari)
                .map_err(|messaggio| ErroreApplicazione::Comando { indice, messaggio })?;
            esiti.push(EsitoComando {
                indice,
                descrizione,
            });
        }
        let prima = valida_progetto(&self.progetto).problemi;
        let dopo = valida_progetto(&copia).problemi;
        let chiavi_prima: BTreeSet<String> = prima.iter().map(chiave).collect();
        let chiavi_dopo: BTreeSet<String> = dopo.iter().map(chiave).collect();
        let nuovi: Vec<Problema> = dopo
            .into_iter()
            .filter(|p| !chiavi_prima.contains(&chiave(p)))
            .collect();
        let risolti: Vec<Problema> = prima
            .into_iter()
            .filter(|p| !chiavi_dopo.contains(&chiave(p)))
            .collect();
        if self.politica.rifiuta_nuovi_errori {
            let errori: Vec<Problema> = nuovi
                .iter()
                .filter(|p| p.gravita == Gravita::Errore)
                .cloned()
                .collect();
            if !errori.is_empty() {
                return Err(ErroreApplicazione::NuoviErrori { problemi: errori });
            }
        }
        let t_prima = totali(&self.progetto);
        let t_dopo = totali(&copia);
        let mut variazioni = Vec::new();
        for (nome, dopo) in &t_dopo {
            let prima = t_prima
                .iter()
                .find(|(n, _)| n == nome)
                .and_then(|(_, t)| *t);
            if prima != *dopo {
                variazioni.push(VariazioneTotale {
                    computo: nome.clone(),
                    prima,
                    dopo: *dopo,
                });
            }
        }
        for (nome, prima) in &t_prima {
            if !t_dopo.iter().any(|(n, _)| n == nome) {
                variazioni.push(VariazioneTotale {
                    computo: nome.clone(),
                    prima: *prima,
                    dopo: None,
                });
            }
        }
        Ok((
            copia,
            g,
            Esito {
                applicato: false,
                comandi: esiti,
                nuovi_problemi: nuovi,
                problemi_risolti: risolti,
                totali: variazioni,
            },
        ))
    }

    /// Prova i comandi senza modificare il progetto.
    ///
    /// # Errors
    /// Come [`Sessione::esegui`].
    pub fn prova(&self, comandi: &[Comando]) -> Result<Esito, ErroreApplicazione> {
        self.calcola(comandi).map(|(_, _, e)| e)
    }

    /// Esegue i comandi come un unico passo atomico.
    ///
    /// # Errors
    /// Nessun comando, comando non applicabile (con il suo indice), nuovi errori di
    /// validazione: in tutti i casi il progetto resta invariato.
    pub fn esegui(&mut self, comandi: &[Comando]) -> Result<Esito, ErroreApplicazione> {
        let (nuovo, g, mut esito) = self.calcola(comandi)?;
        let descrizione = match esito.comandi.as_slice() {
            [unico] => unico.descrizione.clone(),
            molti => format!("{} comandi", molti.len()),
        };
        let precedente = std::mem::replace(&mut self.progetto, nuovo);
        self.annulla.push(Passo {
            descrizione,
            progetto: precedente,
        });
        if self.annulla.len() > self.politica.limite_annulla {
            self.annulla.remove(0);
        }
        self.ripristina.clear();
        self.generatore = g;
        self.modificato = true;
        esito.applicato = true;
        Ok(esito)
    }

    /// Annulla l'ultimo passo e ne restituisce la descrizione.
    ///
    /// # Errors
    /// Nessun passo da annullare.
    pub fn annulla(&mut self) -> Result<String, ErroreApplicazione> {
        let passo = self
            .annulla
            .pop()
            .ok_or(ErroreApplicazione::NienteDaAnnullare)?;
        let attuale = std::mem::replace(&mut self.progetto, passo.progetto);
        self.ripristina.push(Passo {
            descrizione: passo.descrizione.clone(),
            progetto: attuale,
        });
        self.modificato = true;
        Ok(passo.descrizione)
    }

    /// Ripristina l'ultimo passo annullato e ne restituisce la descrizione.
    ///
    /// # Errors
    /// Nessun passo da ripristinare.
    pub fn ripristina(&mut self) -> Result<String, ErroreApplicazione> {
        let passo = self
            .ripristina
            .pop()
            .ok_or(ErroreApplicazione::NienteDaRipristinare)?;
        let attuale = std::mem::replace(&mut self.progetto, passo.progetto);
        self.annulla.push(Passo {
            descrizione: passo.descrizione.clone(),
            progetto: attuale,
        });
        self.modificato = true;
        Ok(passo.descrizione)
    }
}
