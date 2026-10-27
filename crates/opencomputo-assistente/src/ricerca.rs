//! Ricerca testuale nelle voci di un prezzario (ADR 0025).
//!
//! * Il testo di ogni voce è codice, descrizioni e descrizioni dei capitoli antenati:
//!   una voce foglia («spessore 10 cm») eredita il contesto del capitolo
//!   («Massetti in sabbia e cemento»). Le parole della voce pesano il doppio di quelle
//!   dei capitoli.
//! * Punteggio BM25 (k1 = 1,2; b = 0,75) sui termini di [`crate::testo::termini`].
//! * Un termine della domanda assente dal vocabolario viene sostituito dai termini simili
//!   per trigrammi (somiglianza di Jaccard ≥ 0,5), con un peso ridotto: errori di
//!   battitura e varianti non bloccano la ricerca.
//! * Tutti i calcoli sono in interi a virgola fissa (scala 10⁶) e il logaritmo
//!   dell'IDF in [`Decimal`]: nessuna aritmetica in virgola mobile, risultati identici su
//!   ogni piattaforma. A parità di punteggio vale l'ordine delle voci nel prezzario.
//!
//! L'indice legge il prezzario e non lo modifica.

use std::collections::{BTreeMap, BTreeSet};

use opencomputo_core::id::CapitoloId;
use opencomputo_core::prezzario::{NaturaVoce, Prezzario};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};

use crate::testo::{termini, trigrammi};
use crate::unita::{Dimensione, riconosci};

/// Scala dei punteggi: 1,0 = 1 000 000.
pub const SCALA: i64 = 1_000_000;
const SCALA_I: i128 = 1_000_000;
/// Somiglianza minima per sostituire un termine assente (in decimi: 5 = 0,5).
const SOMIGLIANZA_MINIMA_DECIMI: i128 = 5;
/// Termini simili considerati per ogni termine assente.
const SIMILI_MASSIMI: usize = 5;
/// Profondità massima della catena dei capitoli (protezione dai cicli).
const PROFONDITA_CAPITOLI: usize = 32;

/// Filtri strutturati della ricerca. Un filtro vuoto non esclude nulla.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FiltroVoci {
    /// Dimensioni ammesse dell'unità di misura (vuoto: tutte).
    pub dimensioni: Vec<Dimensione>,
    /// Prefisso del codice del capitolo (o di un capitolo antenato).
    pub capitolo: Option<String>,
    /// Natura della voce.
    pub natura: Option<NaturaVoce>,
}

/// Voce trovata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RisultatoRicerca {
    /// Codice della voce nel prezzario.
    pub codice: String,
    /// Punteggio in scala [`SCALA`].
    pub punteggio: i64,
    /// Termini della domanda (o simili) trovati nella voce.
    pub termini_trovati: Vec<String>,
}

#[derive(Debug, Clone)]
struct Documento {
    codice: String,
    lunghezza: u32,
    dimensione: Dimensione,
    natura: NaturaVoce,
    /// Codici del capitolo e dei suoi antenati.
    capitoli: Vec<String>,
}

#[derive(Debug, Clone)]
struct Termine {
    /// IDF in scala.
    idf: i128,
    /// (documento, frequenza nel documento).
    occorrenze: Vec<(u32, u32)>,
    trigrammi: usize,
}

/// Indice testuale di un prezzario.
#[derive(Debug, Clone)]
pub struct IndiceTestuale {
    documenti: Vec<Documento>,
    vocabolario: BTreeMap<String, Termine>,
    per_trigramma: BTreeMap<String, Vec<String>>,
    lunghezza_totale: u64,
}

/// Catena dei capitoli (dal capitolo della voce verso la radice): codici e descrizioni.
fn catena(p: &Prezzario, capitolo: Option<CapitoloId>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut corrente = capitolo;
    while let Some(id) = corrente {
        if out.len() >= PROFONDITA_CAPITOLI {
            break;
        }
        match p.capitoli.iter().find(|c| c.id == id) {
            Some(c) => {
                out.push((c.codice.clone(), c.descrizione.clone()));
                corrente = c.padre;
            }
            None => break,
        }
    }
    out
}

/// Testo indicizzato di una voce: codice e descrizioni della voce, poi descrizioni dei
/// capitoli antenati.
#[must_use]
pub fn testo_voce(p: &Prezzario, codice: &str) -> Option<String> {
    let (proprio, capitoli) = testi_voce(p, codice)?;
    Some(format!("{proprio} {capitoli}"))
}

/// Testo proprio della voce e testo dei capitoli, separati.
fn testi_voce(p: &Prezzario, codice: &str) -> Option<(String, String)> {
    let v = p.voce_per_codice(codice)?;
    let mut proprio = format!("{} {}", v.codice, v.descrizione_breve);
    if v.descrizione_estesa != v.descrizione_breve {
        proprio.push(' ');
        proprio.push_str(&v.descrizione_estesa);
    }
    let capitoli: Vec<String> = catena(p, v.capitolo).into_iter().map(|(_, d)| d).collect();
    Some((proprio, capitoli.join(" ")))
}

/// IDF di BM25: ln(1 + (N − df + 0,5) / (df + 0,5)), in scala.
fn idf(n: usize, df: usize) -> i128 {
    let n = Decimal::from(n);
    let df = Decimal::from(df);
    let mezzo = Decimal::new(5, 1);
    let rapporto = (n - df + mezzo) / (df + mezzo);
    (Decimal::ONE + rapporto)
        .checked_ln()
        .and_then(|l| (l * Decimal::from(SCALA)).round().to_i128())
        .unwrap_or(0)
}

/// Somiglianza di Jaccard tra due insiemi di trigrammi, in scala.
fn jaccard(comuni: usize, a: usize, b: usize) -> i128 {
    let unione = a + b - comuni.min(a + b);
    if unione == 0 {
        return 0;
    }
    i128::try_from(comuni).unwrap_or(0) * SCALA_I / i128::try_from(unione).unwrap_or(1)
}

impl IndiceTestuale {
    /// Costruisce l'indice di tutte le voci del prezzario.
    #[must_use]
    pub fn new(p: &Prezzario) -> Self {
        let mut documenti = Vec::with_capacity(p.voci.len());
        let mut conteggi: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
        let mut lunghezza_totale = 0u64;
        for (i, v) in p.voci.iter().enumerate() {
            let Ok(indice) = u32::try_from(i) else { break };
            // Le parole della voce contano il doppio di quelle dei capitoli: il capitolo
            // dà il contesto, la descrizione decide tra voci dello stesso capitolo.
            let (proprio, capitoli) = testi_voce(p, &v.codice).unwrap_or_default();
            let t: Vec<String> = termini(&proprio)
                .into_iter()
                .flat_map(|x| [x.clone(), x])
                .chain(termini(&capitoli))
                .collect();
            let mut frequenze: BTreeMap<String, u32> = BTreeMap::new();
            for x in &t {
                *frequenze.entry(x.clone()).or_default() += 1;
            }
            for (termine, f) in frequenze {
                conteggi.entry(termine).or_default().push((indice, f));
            }
            let lunghezza = u32::try_from(t.len()).unwrap_or(u32::MAX);
            lunghezza_totale += u64::from(lunghezza);
            documenti.push(Documento {
                codice: v.codice.clone(),
                lunghezza,
                dimensione: riconosci(&v.unita_misura).dimensione,
                natura: v.natura,
                capitoli: catena(p, v.capitolo).into_iter().map(|(c, _)| c).collect(),
            });
        }
        let n = documenti.len();
        let mut vocabolario = BTreeMap::new();
        let mut per_trigramma: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (termine, occorrenze) in conteggi {
            let tri = trigrammi(&termine);
            for g in &tri {
                per_trigramma
                    .entry(g.clone())
                    .or_default()
                    .push(termine.clone());
            }
            vocabolario.insert(
                termine,
                Termine {
                    idf: idf(n, occorrenze.len()),
                    occorrenze,
                    trigrammi: tri.len(),
                },
            );
        }
        Self {
            documenti,
            vocabolario,
            per_trigramma,
            lunghezza_totale,
        }
    }

    /// Numero di voci indicizzate.
    #[must_use]
    pub fn len(&self) -> usize {
        self.documenti.len()
    }

    /// `true` se l'indice non contiene voci.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documenti.is_empty()
    }

    /// Termini del vocabolario simili a un termine assente, con il peso in scala.
    fn simili(&self, termine: &str) -> Vec<(String, i128)> {
        let tri = trigrammi(termine);
        let mut comuni: BTreeMap<&str, usize> = BTreeMap::new();
        for g in &tri {
            if let Some(ts) = self.per_trigramma.get(g) {
                for t in ts {
                    *comuni.entry(t.as_str()).or_default() += 1;
                }
            }
        }
        let mut out: Vec<(String, i128)> = comuni
            .into_iter()
            .filter_map(|(t, c)| {
                let altri = self.vocabolario.get(t)?.trigrammi;
                let j = jaccard(c, tri.len(), altri);
                (j * 10 >= SOMIGLIANZA_MINIMA_DECIMI * SCALA_I).then(|| (t.to_owned(), j * 8 / 10))
            })
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.truncate(SIMILI_MASSIMI);
        out
    }

    fn ammesso(&self, d: &Documento, filtro: &FiltroVoci) -> bool {
        (filtro.dimensioni.is_empty() || filtro.dimensioni.contains(&d.dimensione))
            && filtro.natura.is_none_or(|n| n == d.natura)
            && filtro
                .capitolo
                .as_ref()
                .is_none_or(|pre| d.capitoli.iter().any(|c| c.starts_with(pre.as_str())))
    }

    /// Cerca le voci più pertinenti alla domanda, al massimo `k`.
    #[must_use]
    pub fn cerca(&self, domanda: &str, k: usize, filtro: &FiltroVoci) -> Vec<RisultatoRicerca> {
        let n = i128::try_from(self.documenti.len()).unwrap_or(0);
        if n == 0 || k == 0 {
            return Vec::new();
        }
        let totale = i128::from(self.lunghezza_totale).max(1);
        let domanda: BTreeSet<String> = termini(domanda).into_iter().collect();
        // Termine dell'indice → peso in scala (il massimo se più termini vi portano).
        let mut pesi: BTreeMap<String, i128> = BTreeMap::new();
        for t in domanda {
            if self.vocabolario.contains_key(&t) {
                pesi.insert(t, SCALA_I);
            } else {
                for (s, w) in self.simili(&t) {
                    let p = pesi.entry(s).or_default();
                    *p = (*p).max(w);
                }
            }
        }
        let mut punteggi: BTreeMap<u32, (i128, Vec<String>)> = BTreeMap::new();
        for (termine, peso) in &pesi {
            let Some(voce) = self.vocabolario.get(termine) else {
                continue;
            };
            for &(doc, tf) in &voce.occorrenze {
                let Some(d) = self.documenti.get(doc as usize) else {
                    continue;
                };
                if !self.ammesso(d, filtro) {
                    continue;
                }
                let tf = i128::from(tf);
                let dl = i128::from(d.lunghezza);
                // norma = 1 − b + b · dl / dl_medio, con dl_medio = totale / n.
                let norma = SCALA_I * 25 / 100 + SCALA_I * 75 / 100 * dl * n / totale;
                let numeratore = tf * SCALA_I * 22 / 10;
                let denominatore = tf * SCALA_I + norma * 12 / 10;
                let parte_tf = numeratore * SCALA_I / denominatore.max(1);
                let contributo = voce.idf * parte_tf / SCALA_I * peso / SCALA_I;
                let e = punteggi.entry(doc).or_default();
                e.0 += contributo;
                e.1.push(termine.clone());
            }
        }
        let mut ordinati: Vec<(u32, i128, Vec<String>)> = punteggi
            .into_iter()
            .map(|(d, (p, t))| (d, p, t))
            .filter(|(_, p, _)| *p > 0)
            .collect();
        ordinati.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ordinati
            .into_iter()
            .take(k)
            .filter_map(|(d, p, t)| {
                Some(RisultatoRicerca {
                    codice: self.documenti.get(d as usize)?.codice.clone(),
                    punteggio: i64::try_from(p).unwrap_or(i64::MAX),
                    termini_trovati: t,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn idf_decresce_con_la_frequenza() {
        assert!(idf(100, 1) > idf(100, 10));
        assert!(idf(100, 10) > idf(100, 90));
        assert!(idf(100, 100) > 0);
    }

    #[test]
    fn jaccard_in_scala() {
        assert_eq!(jaccard(3, 3, 3), SCALA_I);
        assert_eq!(jaccard(1, 2, 2), SCALA_I / 3);
        assert_eq!(jaccard(0, 0, 0), 0);
    }
}
