//! Candidati inviati al modello per la scelta delle voci.
//!
//! Ogni candidato riceve un handle (`c01`, `c02`…) valido per una sola richiesta: il
//! modello sceglie handle, non codici, e non vede prezzi. Rust risolve l'handle nella
//! voce del prezzario, così una voce che non è stata proposta non può essere scelta.

use std::collections::BTreeSet;

use opencomputo_core::prezzario::Prezzario;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ricerca::{FiltroVoci, IndiceTestuale, SCALA};
use crate::risposta::RispostaAnalisi;
use crate::unita::{Dimensione, riconosci};
use crate::vettori::IndiceVettoriale;

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

/// Costante della *reciprocal rank fusion*.
const RRF_K: i64 = 60;

/// Codici dei primi `k` risultati: classifica testuale, fusa con quella vettoriale se il
/// vettore della domanda è disponibile. Punteggio RRF = Σ 10⁶ / (60 + rango).
#[must_use]
pub fn cerca_ibrida(
    indice: &IndiceTestuale,
    vettore: Option<(&IndiceVettoriale, &[f32])>,
    domanda: &str,
    k: usize,
    filtro: &FiltroVoci,
) -> Vec<String> {
    let profondita = k.saturating_mul(3).max(k);
    let testuali: Vec<String> = indice
        .cerca(domanda, profondita, filtro)
        .into_iter()
        .map(|r| r.codice)
        .collect();
    let Some((iv, v)) = vettore else {
        let mut t = testuali;
        t.truncate(k);
        return t;
    };
    let vettoriali: Vec<String> = iv
        .cerca(v, profondita, |c| indice.ammette(c, filtro))
        .into_iter()
        .map(|(c, _)| c)
        .collect();
    let mut punteggi: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for lista in [&testuali, &vettoriali] {
        for (rango, c) in lista.iter().enumerate() {
            let r = i64::try_from(rango).unwrap_or(i64::MAX - RRF_K - 1) + 1;
            *punteggi.entry(c.clone()).or_default() += SCALA / (RRF_K + r);
        }
    }
    let mut fusi: Vec<(String, i64)> = punteggi.into_iter().collect();
    fusi.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    fusi.into_iter().take(k).map(|(c, _)| c).collect()
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
        let mappa = crate::ricerca::mappa_capitoli(p);
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
                let Some(c) = mappa.get(&id) else {
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

    /// Candidati per le lavorazioni dell'analisi con la sola ricerca testuale. Vedi
    /// [`Candidati::da_analisi_ibrida`].
    #[must_use]
    pub fn da_analisi(
        indice: &IndiceTestuale,
        p: &Prezzario,
        analisi: &RispostaAnalisi,
        per_lavorazione: usize,
        massimo: usize,
    ) -> Self {
        Self::da_analisi_ibrida(indice, None, p, analisi, per_lavorazione, massimo)
    }

    /// Candidati per le lavorazioni dell'analisi: per ognuna i primi `per_lavorazione`
    /// risultati, filtrati per la dimensione attesa (se il filtro non trova nulla si
    /// cerca senza filtro). Con l'indice vettoriale e un vettore per lavorazione (della
    /// sua `query_ricerca`), le classifiche testuale e vettoriale si fondono con la
    /// *reciprocal rank fusion* (k = 60, in interi). I risultati sono alternati tra le
    /// lavorazioni, così che ognuna abbia candidati anche con il limite `massimo`.
    #[must_use]
    pub fn da_analisi_ibrida(
        indice: &IndiceTestuale,
        vettori: Option<(&IndiceVettoriale, &[Vec<f32>])>,
        p: &Prezzario,
        analisi: &RispostaAnalisi,
        per_lavorazione: usize,
        massimo: usize,
    ) -> Self {
        let mut liste: Vec<(String, Vec<String>)> = Vec::new();
        for (i, l) in analisi.lavorazioni.iter().enumerate() {
            let filtro = FiltroVoci {
                dimensioni: dimensioni_attese(&l.dimensione_attesa),
                ..FiltroVoci::default()
            };
            let vettore = vettori.and_then(|(iv, vs)| vs.get(i).map(|v| (iv, v.as_slice())));
            let mut trovati =
                cerca_ibrida(indice, vettore, &l.query_ricerca, per_lavorazione, &filtro);
            if trovati.is_empty() && !filtro.dimensioni.is_empty() {
                trovati = cerca_ibrida(
                    indice,
                    vettore,
                    &l.query_ricerca,
                    per_lavorazione,
                    &FiltroVoci::default(),
                );
            }
            liste.push((l.id.clone(), trovati));
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
