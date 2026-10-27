//! Candidati inviati al modello per la scelta delle voci.
//!
//! Ogni candidato riceve un handle (`c01`, `c02`…) valido per una sola richiesta: il
//! modello sceglie handle, non codici, e non vede prezzi. Rust risolve l'handle nella
//! voce del prezzario, così una voce che non è stata proposta non può essere scelta.

use std::collections::BTreeSet;

use opencomputo_core::prezzario::Prezzario;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ricerca::{FiltroVoci, IndiceTestuale};
use crate::risposta::RispostaAnalisi;
use crate::unita::{Dimensione, riconosci};

/// Numero massimo di candidati per richiesta (gli handle hanno due cifre).
pub const CANDIDATI_MASSIMI: usize = 99;
/// Caratteri massimi della descrizione inviata per ogni candidato.
const DESCRIZIONE_MASSIMA: usize = 600;

/// Candidato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidato {
    /// Handle (`c01`…).
    pub handle: String,
    /// Codice della voce nel prezzario.
    pub codice: String,
    /// Descrizione estesa (troncata per il modello).
    pub descrizione: String,
    /// Unità di misura della voce.
    pub unita_misura: String,
    /// Dimensione dell'unità.
    pub dimensione: Dimensione,
    /// Capitoli, dalla radice al capitolo della voce.
    pub capitoli: Vec<String>,
    /// Lavorazioni dell'analisi per cui la voce è stata trovata.
    pub lavorazioni: Vec<String>,
}

/// Insieme dei candidati di una richiesta.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidati {
    /// Candidati, nell'ordine degli handle.
    pub elenco: Vec<Candidato>,
}

/// Dimensioni ammesse per la dimensione attesa dell'analisi (vuoto: tutte).
#[must_use]
pub fn dimensioni_attese(testo: &str) -> Vec<Dimensione> {
    match testo.trim().to_lowercase().as_str() {
        "lunghezza" => vec![Dimensione::Lunghezza],
        "superficie" => vec![Dimensione::Area],
        "volume" => vec![Dimensione::Volume],
        "massa" => vec![Dimensione::Massa],
        "numero" => vec![Dimensione::Conteggio],
        "a_corpo" => vec![Dimensione::ACorpo],
        _ => Vec::new(),
    }
}

fn tronca(testo: &str, massimo: usize) -> String {
    if testo.chars().count() <= massimo {
        return testo.to_owned();
    }
    let mut t: String = testo.chars().take(massimo).collect();
    t.push('…');
    t
}

impl Candidati {
    /// Candidati per i codici indicati, nell'ordine dato (codici assenti o ripetuti
    /// ignorati), fino a [`CANDIDATI_MASSIMI`].
    #[must_use]
    pub fn da_codici(p: &Prezzario, codici: &[(String, Vec<String>)]) -> Self {
        let mut visti = BTreeSet::new();
        let mut elenco = Vec::new();
        for (codice, lavorazioni) in codici {
            if elenco.len() >= CANDIDATI_MASSIMI || !visti.insert(codice.clone()) {
                continue;
            }
            let Some(v) = p.voce_per_codice(codice) else {
                continue;
            };
            let mut capitoli = Vec::new();
            let mut corrente = v.capitolo;
            while let Some(id) = corrente {
                if capitoli.len() >= 32 {
                    break;
                }
                let Some(c) = p.capitoli.iter().find(|c| c.id == id) else {
                    break;
                };
                capitoli.push(if c.descrizione.is_empty() {
                    c.codice.clone()
                } else {
                    c.descrizione.clone()
                });
                corrente = c.padre;
            }
            capitoli.reverse();
            elenco.push(Candidato {
                handle: format!("c{:02}", elenco.len() + 1),
                codice: v.codice.clone(),
                descrizione: tronca(&v.descrizione_estesa, DESCRIZIONE_MASSIMA),
                unita_misura: v.unita_misura.clone(),
                dimensione: riconosci(&v.unita_misura).dimensione,
                capitoli,
                lavorazioni: lavorazioni.clone(),
            });
        }
        Self { elenco }
    }

    /// Candidati per le lavorazioni dell'analisi: per ognuna i primi `per_lavorazione`
    /// risultati della ricerca, filtrati per la dimensione attesa (se il filtro non trova
    /// nulla si cerca senza filtro). I risultati sono alternati tra le lavorazioni, così
    /// che ognuna abbia candidati anche con il limite `massimo`.
    #[must_use]
    pub fn da_analisi(
        indice: &IndiceTestuale,
        p: &Prezzario,
        analisi: &RispostaAnalisi,
        per_lavorazione: usize,
        massimo: usize,
    ) -> Self {
        let mut liste: Vec<(String, Vec<String>)> = Vec::new();
        for l in &analisi.lavorazioni {
            let filtro = FiltroVoci {
                dimensioni: dimensioni_attese(&l.dimensione_attesa),
                ..FiltroVoci::default()
            };
            let mut trovati = indice.cerca(&l.query_ricerca, per_lavorazione, &filtro);
            if trovati.is_empty() && !filtro.dimensioni.is_empty() {
                trovati = indice.cerca(&l.query_ricerca, per_lavorazione, &FiltroVoci::default());
            }
            liste.push((
                l.id.clone(),
                trovati.into_iter().map(|r| r.codice).collect(),
            ));
        }
        // Alternanza per rango; ogni codice ricorda tutte le lavorazioni che lo trovano.
        let mut ordine: Vec<(String, Vec<String>)> = Vec::new();
        let profondita = liste.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
        for rango in 0..profondita {
            for (id, codici) in &liste {
                let Some(codice) = codici.get(rango) else {
                    continue;
                };
                match ordine.iter_mut().find(|(c, _)| c == codice) {
                    Some((_, ls)) => {
                        if !ls.contains(id) {
                            ls.push(id.clone());
                        }
                    }
                    None => ordine.push((codice.clone(), vec![id.clone()])),
                }
            }
        }
        ordine.truncate(massimo.min(CANDIDATI_MASSIMI));
        Self::da_codici(p, &ordine)
    }

    /// Candidato per handle, senza distinzione di maiuscole.
    #[must_use]
    pub fn per_handle(&self, handle: &str) -> Option<&Candidato> {
        self.elenco
            .iter()
            .find(|c| c.handle.eq_ignore_ascii_case(handle.trim()))
    }

    /// Candidati come li vede il modello: handle, descrizione, unità, capitoli,
    /// lavorazioni. Niente codici né prezzi.
    #[must_use]
    pub fn per_modello(&self) -> Value {
        Value::Array(
            self.elenco
                .iter()
                .map(|c| {
                    json!({
                        "handle": c.handle,
                        "descrizione": c.descrizione,
                        "unita_misura": c.unita_misura,
                        "capitoli": c.capitoli,
                        "lavorazioni": c.lavorazioni,
                    })
                })
                .collect(),
        )
    }
}
