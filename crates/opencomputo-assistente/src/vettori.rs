//! Indice vettoriale di un prezzario (ADR 0025, decisioni 9 e 12).
//!
//! * Lo **spazio** (modello, dimensioni, versione del testo indicizzato) è parte
//!   dell'indice: vettori di spazi diversi non vengono mai confrontati.
//! * Ogni voce ha l'impronta SHA-256 del testo indicizzato: rileggendo l'indice si
//!   ricalcolano solo le voci nuove o cambiate, e un'indicizzazione interrotta riprende
//!   dai lotti mancanti.
//! * L'indice è una cache: si ricostruisce dal prezzario e non lo modifica.
//! * La similitudine coseno è l'unico calcolo in virgola mobile del workspace, isolato
//!   nel modulo `calcolo`; il punteggio esce come intero in scala 10⁶. Nessun valore
//!   contabile passa da qui.
//!
//! Formato del file (little endian): `OCVEC` + versione (u16) + lunghezza della testata
//! JSON (u32) + testata (spazio, impronta del prezzario, numero di voci) + per ogni voce:
//! lunghezza del codice (u16), codice UTF-8, impronta del testo (32 byte), vettore
//! (`dimensioni` × f32).

use std::collections::BTreeMap;

use opencomputo_core::prezzario::Prezzario;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::provider::openai::LimitiLotto;
use crate::provider::{Annullamento, Embedding, ErroreAi, Provider};
use crate::ricerca::{mappa_capitoli, testo_di};

/// Versione del testo indicizzato: cambia se cambia il modo di comporre il testo.
pub const VERSIONE_TESTO: u16 = 1;
/// Versione del formato del file.
pub const VERSIONE_FORMATO: u16 = 1;
const MAGIA: &[u8; 5] = b"OCVEC";
/// Voci massime accettate in lettura.
const VOCI_MASSIME: usize = 2_000_000;
/// Lunghezza massima della testata JSON, in byte.
const TESTATA_MASSIMA: usize = 65_536;
/// Dimensioni massime accettate in lettura.
const DIMENSIONI_MASSIME: u32 = 16_384;

/// Spazio di embedding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpazioEmbedding {
    /// Modello.
    pub modello: String,
    /// Dimensioni dei vettori.
    pub dimensioni: u32,
    /// Versione del testo indicizzato.
    pub versione_testo: u16,
}

impl SpazioEmbedding {
    /// Spazio per modello e dimensioni, con la versione corrente del testo.
    #[must_use]
    pub fn new(modello: &str, dimensioni: u32) -> Self {
        Self {
            modello: modello.to_owned(),
            dimensioni,
            versione_testo: VERSIONE_TESTO,
        }
    }

    /// Nome breve e sicuro per i file (`text-embedding-3-small-1536-t1`).
    #[must_use]
    pub fn nome_file(&self) -> String {
        let modello: String = self
            .modello
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{modello}-{}-t{}", self.dimensioni, self.versione_testo)
    }
}

/// Voce da indicizzare: codice, testo e impronta del testo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoceDaIndicizzare {
    /// Codice.
    pub codice: String,
    /// Testo inviato al provider.
    pub testo: String,
    /// SHA-256 del testo.
    pub impronta: [u8; 32],
}

/// Testi delle voci del prezzario (codice, descrizioni, capitoli), accorciati a
/// `caratteri_massimi`.
#[must_use]
pub fn da_indicizzare(p: &Prezzario, caratteri_massimi: usize) -> Vec<VoceDaIndicizzare> {
    let mappa = mappa_capitoli(p);
    p.voci
        .iter()
        .map(|v| {
            let testo: String = testo_di(&mappa, v)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(caratteri_massimi)
                .collect();
            VoceDaIndicizzare {
                codice: v.codice.clone(),
                impronta: Sha256::digest(testo.as_bytes()).into(),
                testo,
            }
        })
        .collect()
}

/// Impronta del contenuto indicizzabile del prezzario (codici e testi, nell'ordine).
#[must_use]
pub fn impronta_prezzario(voci: &[VoceDaIndicizzare]) -> String {
    let mut h = Sha256::new();
    for v in voci {
        h.update(v.codice.as_bytes());
        h.update([0]);
        h.update(v.impronta);
    }
    h.finalize()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            s.push_str(&format!("{b:02x}"));
            s
        })
}

/// Errore di lettura del file dell'indice.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreIndice {
    /// File non riconosciuto o danneggiato.
    #[error("indice vettoriale non valido: {0}")]
    NonValido(String),
    /// Spazio diverso da quello atteso.
    #[error("indice vettoriale di un altro spazio ({trovato}, atteso {atteso})")]
    AltroSpazio {
        /// Spazio trovato.
        trovato: String,
        /// Spazio atteso.
        atteso: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct VoceVettore {
    impronta: [u8; 32],
    vettore: Vec<f32>,
    /// Norma del vettore, calcolata una volta alla registrazione o alla lettura.
    norma: f64,
}

/// Indice vettoriale.
#[derive(Debug, Clone, PartialEq)]
pub struct IndiceVettoriale {
    spazio: SpazioEmbedding,
    impronta_prezzario: String,
    voci: BTreeMap<String, VoceVettore>,
}

#[derive(Serialize, Deserialize)]
struct Testata {
    spazio: SpazioEmbedding,
    impronta_prezzario: String,
    voci: usize,
}

/// Avanzamento dell'indicizzazione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Avanzamento {
    /// Lotto appena completato (da 1).
    pub lotto: usize,
    /// Lotti totali.
    pub lotti: usize,
    /// Voci calcolate finora.
    pub calcolate: usize,
    /// Voci da calcolare in tutto.
    pub da_calcolare: usize,
}

/// Esito dell'indicizzazione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RapportoIndicizzazione {
    /// Voci nel prezzario.
    pub voci: usize,
    /// Voci riusate dalla cache.
    pub riusate: usize,
    /// Voci calcolate ora.
    pub calcolate: usize,
    /// Voci tolte perché non più nel prezzario.
    pub rimosse: usize,
    /// Lotti inviati.
    pub lotti: usize,
}

impl IndiceVettoriale {
    /// Indice vuoto.
    #[must_use]
    pub fn new(spazio: SpazioEmbedding, impronta_prezzario: &str) -> Self {
        Self {
            spazio,
            impronta_prezzario: impronta_prezzario.to_owned(),
            voci: BTreeMap::new(),
        }
    }

    /// Spazio.
    #[must_use]
    pub fn spazio(&self) -> &SpazioEmbedding {
        &self.spazio
    }

    /// Numero di vettori.
    #[must_use]
    pub fn len(&self) -> usize {
        self.voci.len()
    }

    /// `true` se non ci sono vettori.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.voci.is_empty()
    }

    /// Voci da calcolare: assenti o con un testo diverso.
    #[must_use]
    pub fn mancanti<'a>(&self, da: &'a [VoceDaIndicizzare]) -> Vec<&'a VoceDaIndicizzare> {
        da.iter()
            .filter(|v| {
                self.voci
                    .get(&v.codice)
                    .is_none_or(|x| x.impronta != v.impronta)
            })
            .collect()
    }

    /// `true` se ogni voce ha il suo vettore aggiornato.
    #[must_use]
    pub fn completo(&self, da: &[VoceDaIndicizzare]) -> bool {
        self.mancanti(da).is_empty()
    }

    /// Toglie le voci che non sono più nel prezzario e aggiorna l'impronta.
    pub fn allinea(&mut self, da: &[VoceDaIndicizzare]) -> usize {
        let codici: std::collections::BTreeSet<&str> =
            da.iter().map(|v| v.codice.as_str()).collect();
        let prima = self.voci.len();
        self.voci.retain(|c, _| codici.contains(c.as_str()));
        self.impronta_prezzario = impronta_prezzario(da);
        prima - self.voci.len()
    }

    /// Registra il vettore di una voce.
    ///
    /// # Errors
    /// Vettore di dimensioni diverse da quelle dello spazio.
    pub fn registra(
        &mut self,
        voce: &VoceDaIndicizzare,
        vettore: Vec<f32>,
    ) -> Result<(), ErroreIndice> {
        if vettore.len() != self.spazio.dimensioni as usize {
            return Err(ErroreIndice::NonValido(format!(
                "vettore di {} dimensioni nello spazio da {}",
                vettore.len(),
                self.spazio.dimensioni
            )));
        }
        self.voci.insert(
            voce.codice.clone(),
            VoceVettore {
                impronta: voce.impronta,
                norma: calcolo::norma(&vettore),
                vettore,
            },
        );
        Ok(())
    }

    /// Le `k` voci più simili, tra quelle ammesse, con il punteggio coseno in scala 10⁶.
    /// A parità di punteggio vale l'ordine dei codici.
    #[must_use]
    pub fn cerca(
        &self,
        domanda: &[f32],
        k: usize,
        ammessa: impl Fn(&str) -> bool,
    ) -> Vec<(String, i64)> {
        if domanda.len() != self.spazio.dimensioni as usize {
            return Vec::new();
        }
        let norma = calcolo::norma(domanda);
        // Si confrontano i riferimenti e si copiano solo i codici dei `k` risultati.
        let mut r: Vec<(i64, &str)> = self
            .voci
            .iter()
            .filter(|(c, _)| ammessa(c))
            .map(|(c, v)| {
                (
                    calcolo::coseno_scalato(domanda, norma, &v.vettore, v.norma),
                    c.as_str(),
                )
            })
            .collect();
        let ordine = |a: &(i64, &str), b: &(i64, &str)| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1));
        if k < r.len() {
            if k == 0 {
                return Vec::new();
            }
            r.select_nth_unstable_by(k - 1, ordine);
            r.truncate(k);
        }
        r.sort_unstable_by(ordine);
        r.into_iter().map(|(p, c)| (c.to_owned(), p)).collect()
    }

    /// Scrive l'indice nel formato del file, una voce per volta: un indice grande non
    /// viene mai copiato per intero in memoria.
    ///
    /// # Errors
    /// Errore di scrittura.
    pub fn scrivi(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
        let testata = serde_json::to_vec(&Testata {
            spazio: self.spazio.clone(),
            impronta_prezzario: self.impronta_prezzario.clone(),
            voci: self.voci.len(),
        })
        .unwrap_or_default();
        w.write_all(MAGIA)?;
        w.write_all(&VERSIONE_FORMATO.to_le_bytes())?;
        w.write_all(
            &u32::try_from(testata.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        )?;
        w.write_all(&testata)?;
        let dim = self.spazio.dimensioni as usize;
        let mut riga: Vec<u8> = Vec::with_capacity(4 * dim);
        for (codice, v) in &self.voci {
            let c = codice.as_bytes();
            w.write_all(&u16::try_from(c.len()).unwrap_or(u16::MAX).to_le_bytes())?;
            w.write_all(c)?;
            w.write_all(&v.impronta)?;
            riga.clear();
            for x in &v.vettore {
                riga.extend_from_slice(&x.to_le_bytes());
            }
            w.write_all(&riga)?;
        }
        Ok(())
    }

    /// Codifica nel formato del file.
    #[must_use]
    pub fn codifica(&self) -> Vec<u8> {
        let dim = self.spazio.dimensioni as usize;
        let mut out = Vec::with_capacity(256 + self.voci.len() * (64 + 4 * dim));
        // Scrivere in un vettore non fallisce.
        let _ = self.scrivi(&mut out);
        out
    }

    /// Legge un indice da un file (o altro flusso) senza caricarlo prima per intero.
    ///
    /// # Errors
    /// File non valido, troncato, con byte in eccesso o di un altro spazio.
    pub fn leggi(
        r: &mut impl std::io::Read,
        atteso: &SpazioEmbedding,
    ) -> Result<Self, ErroreIndice> {
        let mut l = Flusso(r);
        if l.array::<5>()? != *MAGIA {
            return Err(ErroreIndice::NonValido("firma assente".into()));
        }
        let versione = u16::from_le_bytes(l.array::<2>()?);
        if versione != VERSIONE_FORMATO {
            return Err(ErroreIndice::NonValido(format!(
                "versione {versione} non supportata"
            )));
        }
        let n = u32::from_le_bytes(l.array::<4>()?) as usize;
        if n > TESTATA_MASSIMA {
            return Err(ErroreIndice::NonValido("testata troppo lunga".into()));
        }
        let mut testata = vec![0u8; n];
        l.riempi(&mut testata)?;
        let testata: Testata = serde_json::from_slice(&testata)
            .map_err(|e| ErroreIndice::NonValido(format!("testata: {e}")))?;
        if &testata.spazio != atteso {
            return Err(ErroreIndice::AltroSpazio {
                trovato: testata.spazio.nome_file(),
                atteso: atteso.nome_file(),
            });
        }
        if testata.voci > VOCI_MASSIME || testata.spazio.dimensioni > DIMENSIONI_MASSIME {
            return Err(ErroreIndice::NonValido(
                "dimensioni fuori dai limiti".into(),
            ));
        }
        let dim = testata.spazio.dimensioni as usize;
        let mut voci = BTreeMap::new();
        let mut riga = vec![0u8; 4 * dim];
        for _ in 0..testata.voci {
            let lc = u16::from_le_bytes(l.array::<2>()?) as usize;
            let mut codice = vec![0u8; lc];
            l.riempi(&mut codice)?;
            let codice = String::from_utf8(codice)
                .map_err(|_| ErroreIndice::NonValido("codice non UTF-8".into()))?;
            let impronta = l.array::<32>()?;
            l.riempi(&mut riga)?;
            let mut vettore = Vec::with_capacity(dim);
            for b in riga.chunks_exact(4) {
                let x = f32::from_le_bytes(b.try_into().unwrap_or([0; 4]));
                if !x.is_finite() {
                    return Err(ErroreIndice::NonValido("valore non finito".into()));
                }
                vettore.push(x);
            }
            voci.insert(
                codice,
                VoceVettore {
                    impronta,
                    norma: calcolo::norma(&vettore),
                    vettore,
                },
            );
        }
        let mut uno = [0u8; 1];
        match l.0.read(&mut uno) {
            Ok(0) => {}
            Ok(_) => return Err(ErroreIndice::NonValido("byte in eccesso".into())),
            Err(e) => return Err(ErroreIndice::NonValido(format!("lettura: {e}"))),
        }
        Ok(Self {
            spazio: testata.spazio,
            impronta_prezzario: testata.impronta_prezzario,
            voci,
        })
    }

    /// Legge un indice dal formato del file e verifica che sia dello spazio atteso.
    ///
    /// # Errors
    /// File non valido o di un altro spazio.
    pub fn decodifica(mut byte: &[u8], atteso: &SpazioEmbedding) -> Result<Self, ErroreIndice> {
        Self::leggi(&mut byte, atteso)
    }
}

/// Lettura esatta da un flusso, con gli errori dell'indice.
struct Flusso<'a, R: std::io::Read>(&'a mut R);

impl<R: std::io::Read> Flusso<'_, R> {
    fn riempi(&mut self, b: &mut [u8]) -> Result<(), ErroreIndice> {
        self.0.read_exact(b).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                ErroreIndice::NonValido("file troncato".into())
            } else {
                ErroreIndice::NonValido(format!("lettura: {e}"))
            }
        })
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ErroreIndice> {
        let mut b = [0u8; N];
        self.riempi(&mut b)?;
        Ok(b)
    }
}

/// Lotti di voci da inviare, nei limiti indicati.
#[must_use]
pub fn lotti<'a>(
    voci: &[&'a VoceDaIndicizzare],
    limiti: &LimitiLotto,
) -> Vec<Vec<&'a VoceDaIndicizzare>> {
    let mut out: Vec<Vec<&VoceDaIndicizzare>> = Vec::new();
    let mut corrente: Vec<&VoceDaIndicizzare> = Vec::new();
    let mut caratteri = 0usize;
    for v in voci {
        let n = v.testo.chars().count();
        if !corrente.is_empty()
            && (corrente.len() >= limiti.testi.max(1) || caratteri + n > limiti.caratteri)
        {
            out.push(std::mem::take(&mut corrente));
            caratteri = 0;
        }
        corrente.push(v);
        caratteri += n;
    }
    if !corrente.is_empty() {
        out.push(corrente);
    }
    out
}

/// Calcola i vettori mancanti dell'indice, a lotti. Dopo ogni lotto chiama `dopo_lotto`
/// (per salvare l'indice parziale e riportare l'avanzamento): un'interruzione perde al
/// più il lotto in corso.
///
/// # Errors
/// Spazio del provider diverso da quello dell'indice; errore del provider; annullamento.
pub fn indicizza(
    provider: &dyn Embedding,
    indice: &mut IndiceVettoriale,
    da: &[VoceDaIndicizzare],
    annullamento: &Annullamento,
    mut dopo_lotto: impl FnMut(&IndiceVettoriale, Avanzamento),
) -> Result<RapportoIndicizzazione, ErroreAi> {
    if provider.spazio() != indice.spazio {
        return Err(ErroreAi::ModelloNonSupportato {
            modello: provider.spazio().modello,
            motivo: format!(
                "lo spazio del provider ({}) è diverso da quello dell'indice ({})",
                provider.spazio().nome_file(),
                indice.spazio.nome_file()
            ),
        });
    }
    let rimosse = indice.allinea(da);
    let mancanti = indice.mancanti(da);
    let gruppi = lotti(&mancanti, &provider.limiti());
    let mut calcolate = 0usize;
    for (i, lotto) in gruppi.iter().enumerate() {
        if annullamento.annullato() {
            return Err(ErroreAi::Annullato);
        }
        let testi: Vec<String> = lotto.iter().map(|v| v.testo.clone()).collect();
        let vettori = provider.embed(&testi, annullamento)?;
        if vettori.len() != lotto.len() {
            return Err(ErroreAi::RispostaNonValida {
                provider: Provider::Openai,
                motivo: format!("{} vettori per {} testi", vettori.len(), lotto.len()),
            });
        }
        for (v, x) in lotto.iter().zip(vettori) {
            indice
                .registra(v, x)
                .map_err(|e| ErroreAi::RispostaNonValida {
                    provider: Provider::Openai,
                    motivo: e.to_string(),
                })?;
        }
        calcolate += lotto.len();
        dopo_lotto(
            indice,
            Avanzamento {
                lotto: i + 1,
                lotti: gruppi.len(),
                calcolate,
                da_calcolare: mancanti.len(),
            },
        );
    }
    Ok(RapportoIndicizzazione {
        voci: da.len(),
        riusate: da.len() - mancanti.len(),
        calcolate,
        rimosse,
        lotti: gruppi.len(),
    })
}

/// Similitudine coseno: l'unico calcolo in virgola mobile (ADR 0025, decisione 12).
#[allow(clippy::float_arithmetic, clippy::cast_possible_truncation)]
mod calcolo {
    /// Norma euclidea.
    pub(super) fn norma(a: &[f32]) -> f64 {
        a.iter()
            .map(|x| f64::from(*x) * f64::from(*x))
            .sum::<f64>()
            .sqrt()
    }

    /// Coseno tra due vettori della stessa dimensione, date le loro norme, in scala 10⁶
    /// (0 se un vettore è nullo).
    pub(super) fn coseno_scalato(a: &[f32], norma_a: f64, b: &[f32], norma_b: f64) -> i64 {
        if norma_a <= 0.0 || norma_b <= 0.0 {
            return 0;
        }
        // Quattro accumulatori: il compilatore non può riordinare le somme in virgola
        // mobile da solo, e così il ciclo si vettorizza. L'ordine resta fisso, quindi il
        // risultato è lo stesso su ogni macchina.
        let mut s = [0.0f64; 4];
        let (ca, cb) = (a.chunks_exact(4), b.chunks_exact(4));
        let (ra, rb) = (ca.remainder(), cb.remainder());
        for (x, y) in ca.zip(cb) {
            for ((acc, x), y) in s.iter_mut().zip(x).zip(y) {
                *acc += f64::from(*x) * f64::from(*y);
            }
        }
        let mut ab = (s[0] + s[1]) + (s[2] + s[3]);
        for (x, y) in ra.iter().zip(rb) {
            ab += f64::from(*x) * f64::from(*y);
        }
        let c = (ab / (norma_a * norma_b)).clamp(-1.0, 1.0);
        (c * 1_000_000.0).round() as i64
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn coseno() {
            let cos = |a: &[f32], b: &[f32]| coseno_scalato(a, norma(a), b, norma(b));
            assert_eq!(cos(&[1.0, 0.0], &[2.0, 0.0]), 1_000_000);
            assert_eq!(cos(&[1.0, 0.0], &[0.0, 3.0]), 0);
            assert_eq!(cos(&[1.0, 0.0], &[-1.0, 0.0]), -1_000_000);
            assert_eq!(cos(&[1.0, 1.0], &[1.0, 0.0]), 707_107);
            assert_eq!(cos(&[0.0, 0.0], &[1.0, 0.0]), 0);
            // Più di quattro componenti: blocchi e resto.
            let a = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
            assert_eq!(cos(&a, &a), 1_000_000);
            let b = [7.0f32, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
            assert_eq!(cos(&a, &b), 600_000); // 84 / 140
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn voce(codice: &str, testo: &str) -> VoceDaIndicizzare {
        VoceDaIndicizzare {
            codice: codice.into(),
            testo: testo.into(),
            impronta: Sha256::digest(testo.as_bytes()).into(),
        }
    }

    #[test]
    fn andata_e_ritorno_e_spazio_diverso() {
        let s = SpazioEmbedding::new("m", 2);
        let mut i = IndiceVettoriale::new(s.clone(), "abc");
        i.registra(&voce("A", "a"), vec![1.0, 0.5]).unwrap();
        i.registra(&voce("B", "b"), vec![-0.25, 2.0]).unwrap();
        assert!(i.registra(&voce("C", "c"), vec![1.0]).is_err());
        let b = i.codifica();
        assert_eq!(IndiceVettoriale::decodifica(&b, &s).unwrap(), i);
        assert!(matches!(
            IndiceVettoriale::decodifica(&b, &SpazioEmbedding::new("m", 3)),
            Err(ErroreIndice::AltroSpazio { .. })
        ));
        assert!(matches!(
            IndiceVettoriale::decodifica(&b, &SpazioEmbedding::new("altro", 2)),
            Err(ErroreIndice::AltroSpazio { .. })
        ));
        for n in [0, 3, 10, b.len() - 1] {
            assert!(
                IndiceVettoriale::decodifica(b.get(..n).unwrap(), &s).is_err(),
                "{n}"
            );
        }
        let mut lungo = b.clone();
        lungo.push(0);
        assert!(IndiceVettoriale::decodifica(&lungo, &s).is_err());
    }

    #[test]
    fn mancanti_allinea_e_ricerca() {
        let s = SpazioEmbedding::new("m", 2);
        let mut i = IndiceVettoriale::new(s, "");
        i.registra(&voce("A", "a"), vec![1.0, 0.0]).unwrap();
        i.registra(&voce("B", "b"), vec![0.0, 1.0]).unwrap();
        i.registra(&voce("X", "x"), vec![1.0, 1.0]).unwrap();
        let da = vec![voce("A", "a"), voce("B", "b cambiato"), voce("C", "c")];
        let m: Vec<&str> = i.mancanti(&da).iter().map(|v| v.codice.as_str()).collect();
        assert_eq!(m, ["B", "C"]);
        assert_eq!(i.allinea(&da), 1);
        assert_eq!(i.len(), 2);
        let r = i.cerca(&[1.0, 0.1], 5, |_| true);
        assert_eq!(r.first().map(|x| x.0.as_str()), Some("A"));
        assert_eq!(
            i.cerca(&[1.0, 0.1], 5, |c| c != "A")
                .first()
                .map(|x| x.0.as_str()),
            Some("B")
        );
        assert!(i.cerca(&[1.0], 5, |_| true).is_empty());
    }

    #[test]
    fn lotti_nei_limiti() {
        let v: Vec<VoceDaIndicizzare> = (0..7)
            .map(|n| voce(&format!("V{n}"), "0123456789"))
            .collect();
        let r: Vec<&VoceDaIndicizzare> = v.iter().collect();
        let l = lotti(
            &r,
            &LimitiLotto {
                testi: 3,
                caratteri: 25,
                caratteri_per_testo: 100,
            },
        );
        assert_eq!(l.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 2, 1]);
        let l = lotti(
            &r,
            &LimitiLotto {
                testi: 3,
                caratteri: 1000,
                caratteri_per_testo: 100,
            },
        );
        assert_eq!(l.iter().map(Vec::len).collect::<Vec<_>>(), [3, 3, 1]);
        assert!(lotti(&[], &LimitiLotto::default()).is_empty());
    }

    #[test]
    fn nome_del_file_dello_spazio() {
        assert_eq!(
            SpazioEmbedding::new("text-embedding-3-small", 512).nome_file(),
            "text-embedding-3-small-512-t1"
        );
        assert_eq!(SpazioEmbedding::new("a/b:c", 8).nome_file(), "a_b_c-8-t1");
    }
}
