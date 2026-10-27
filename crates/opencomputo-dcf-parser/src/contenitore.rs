//! Contenitore binario dei file DCF.
//!
//! Struttura **ricavata dall'analisi di file reali** (2026-10-20: due prezzari e un
//! documento di contabilità), confrontata con il lettore di LeenO (LGPL-2.1, usato
//! solo come fonte di fatti). Vedi `docs/adr/0006-formati-primus.md`.
//!
//! ```text
//! 0x000  "AAMVHFSS"                         firma del file
//! 0x118  settore 0: "AASFSSGN" + testata    u32 a +0x10 = dimensione settore (4096)
//!                                           u32 a +0x30 = numero di settori
//! 0x118 + n·4096  settore n                 testata di 0x20 byte, u32 a +8 = tipo
//!                                           (6 = dati)
//! settore di dati che inizia un flusso:     "AACS" a +0x20, testata di flusso di 0x90 byte
//!     u32 a +4  = dimensione massima di un pezzo decompresso (1 MiB)
//!     u32 a +12 = numero di pezzi
//! dati del flusso = concatenazione delle aree dati (da +0x20) dei settori di dati
//! consecutivi, fino al prossimo "AACS" o a un settore di altro tipo:
//!     [pezzo zlib 0] ([testata 16 byte][pezzo zlib i])…
//!     testata del pezzo: u32 lunghezza compressa, u32 lunghezza decompressa, 2 × u32 non usati
//! ```
//!
//! Un flusso **protetto** è riconosciuto dal contenuto decompresso: 4 byte seguiti dal
//! testo «Copyright ACCA software S.p.A.» e da dati cifrati. Il contenuto protetto non
//! viene mai decifrato: il flusso è solo contato e segnalato.

use flate2::{Decompress, FlushDecompress, Status};

use crate::errore::ErroreDcf;

/// Firma iniziale del file.
pub const FIRMA_FILE: &[u8; 8] = b"AAMVHFSS";
/// Firma del settore 0.
pub const FIRMA_SETTORI: &[u8; 8] = b"AASFSSGN";
/// Posizione del settore 0.
pub const INIZIO_SETTORI: usize = 0x118;
/// Dimensione attesa dei settori.
pub const DIMENSIONE_SETTORE: usize = 4096;
/// Testata di ogni settore.
pub const TESTATA_SETTORE: usize = 0x20;
/// Tipo dei settori di dati.
pub const TIPO_DATI: u32 = 6;
/// Firma di inizio flusso.
pub const FIRMA_FLUSSO: &[u8; 4] = b"AACS";
/// Testata di flusso.
pub const TESTATA_FLUSSO: usize = 0x90;
/// Testata di ogni pezzo successivo al primo.
pub const TESTATA_PEZZO: usize = 16;
/// Testo che precede i dati cifrati nei flussi protetti.
pub const MARCATORE_PROTEZIONE: &[u8] = b"Copyright ACCA software S.p.A.";

/// Dimensione massima di un pezzo decompresso accettata.
pub const PEZZO_MASSIMO: usize = 16 << 20;
/// Numero massimo di pezzi di un flusso.
pub const PEZZI_MASSIMI: usize = 4096;
/// Totale massimo decompresso per file.
pub const TOTALE_MASSIMO: usize = 1 << 30;

/// Contenuto di un flusso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contenuto {
    /// Testo XML decodificato.
    Xml {
        /// Nome dell'elemento radice.
        radice: String,
        /// Testo.
        testo: String,
    },
    /// Flusso protetto (cifrato): non letto.
    Protetto,
    /// Dati binari non XML (anteprime, immagini, dati accessori).
    Binario,
    /// Flusso non decomprimibile.
    Illeggibile(String),
}

/// Flusso del contenitore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flusso {
    /// Primo settore.
    pub settore: usize,
    /// Numero di settori occupati.
    pub settori: usize,
    /// Byte decompressi.
    pub byte: usize,
    /// Contenuto.
    pub contenuto: Contenuto,
}

/// Contenitore letto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contenitore {
    /// Numero di settori dichiarato nella testata.
    pub settori_dichiarati: usize,
    /// Numero di settori presenti nel file.
    pub settori: usize,
    /// Flussi nell'ordine del file.
    pub flussi: Vec<Flusso>,
}

impl Contenitore {
    /// Flussi XML con la radice indicata.
    pub fn xml<'a>(&'a self, radice: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.flussi.iter().filter_map(move |f| match &f.contenuto {
            Contenuto::Xml { radice: r, testo } if r == radice => Some(testo.as_str()),
            _ => None,
        })
    }

    /// Numero di flussi protetti.
    #[must_use]
    pub fn protetti(&self) -> usize {
        self.flussi
            .iter()
            .filter(|f| f.contenuto == Contenuto::Protetto)
            .count()
    }
}

fn u32_le(dati: &[u8], pos: usize) -> Option<u32> {
    let b = dati.get(pos..pos.checked_add(4)?)?;
    Some(u32::from_le_bytes([
        *b.first()?,
        *b.get(1)?,
        *b.get(2)?,
        *b.get(3)?,
    ]))
}

/// Decomprime un pezzo zlib: (byte decompressi, byte compressi consumati).
fn decomprimi_pezzo(input: &[u8], limite: usize) -> Result<(Vec<u8>, usize), String> {
    let mut d = Decompress::new(true);
    let mut out: Vec<u8> = Vec::with_capacity(limite.clamp(1024, 1 << 20));
    loop {
        if out.len() == out.capacity() {
            if out.len() > limite {
                return Err(format!("pezzo oltre {limite} byte decompressi"));
            }
            out.reserve((limite + 1 - out.len()).clamp(1, 1 << 20));
        }
        let consumati = usize::try_from(d.total_in()).map_err(|e| e.to_string())?;
        let resto = input.get(consumati..).unwrap_or_default();
        let prima = (d.total_in(), d.total_out());
        let stato = d
            .decompress_vec(resto, &mut out, FlushDecompress::None)
            .map_err(|e| format!("zlib: {e}"))?;
        if out.len() > limite {
            return Err(format!("pezzo oltre {limite} byte decompressi"));
        }
        match stato {
            Status::StreamEnd => break,
            Status::Ok | Status::BufError => {
                if (d.total_in(), d.total_out()) == prima {
                    return Err("flusso zlib troncato".into());
                }
            }
        }
    }
    let consumati = usize::try_from(d.total_in()).map_err(|e| e.to_string())?;
    Ok((out, consumati))
}

/// Decomprime i pezzi di un flusso (dati a partire dalla firma `AACS`).
fn decomprimi_flusso(dati: &[u8], totale: &mut usize) -> Result<Vec<u8>, String> {
    let dimensione_pezzo = u32_le(dati, 4).ok_or("testata di flusso troncata")? as usize;
    let pezzi = u32_le(dati, 12).ok_or("testata di flusso troncata")? as usize;
    if dimensione_pezzo == 0 || dimensione_pezzo > PEZZO_MASSIMO {
        return Err(format!(
            "dimensione dei pezzi non valida ({dimensione_pezzo})"
        ));
    }
    if pezzi == 0 || pezzi > PEZZI_MASSIMI {
        return Err(format!("numero di pezzi non valido ({pezzi})"));
    }
    let mut pos = TESTATA_FLUSSO;
    let mut uscita = Vec::new();
    for i in 0..pezzi {
        let mut limite = dimensione_pezzo;
        if i > 0 {
            let dichiarata = u32_le(dati, pos + 4).ok_or("testata di pezzo troncata")? as usize;
            if dichiarata > dimensione_pezzo {
                return Err(format!(
                    "pezzo {i}: {dichiarata} byte dichiarati, oltre il massimo"
                ));
            }
            limite = dichiarata.max(1);
            pos += TESTATA_PEZZO;
        }
        let input = dati.get(pos..).ok_or("dati del flusso troncati")?;
        let (pezzo, consumati) =
            decomprimi_pezzo(input, limite).map_err(|e| format!("pezzo {i}: {e}"))?;
        if i > 0 && pezzo.len() != limite {
            return Err(format!(
                "pezzo {i}: {} byte decompressi, {limite} dichiarati",
                pezzo.len()
            ));
        }
        *totale = totale.saturating_add(pezzo.len());
        if *totale > TOTALE_MASSIMO {
            return Err(format!("oltre {TOTALE_MASSIMO} byte decompressi in totale"));
        }
        uscita.extend_from_slice(&pezzo);
        pos += consumati;
    }
    Ok(uscita)
}

/// Decodifica il testo e ne riconosce l'elemento radice, se è XML.
fn come_xml(byte: &[u8]) -> Option<(String, String)> {
    let testo = if let Some(r) = byte.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8(r.to_vec()).ok()?
    } else if byte.starts_with(&[0xFF, 0xFE]) || utf16_probabile(byte) {
        let (t, _, errori) = encoding_rs::UTF_16LE.decode(byte);
        if errori {
            return None;
        }
        t.trim_start_matches('\u{FEFF}').to_owned()
    } else {
        String::from_utf8(byte.to_vec()).ok()?
    };
    let radice = radice_xml(&testo)?;
    Some((radice, testo))
}

fn utf16_probabile(byte: &[u8]) -> bool {
    let campione: Vec<u8> = byte.iter().skip(1).step_by(2).take(64).copied().collect();
    campione.len() >= 8 && campione.iter().filter(|b| **b == 0).count() * 10 >= campione.len() * 9
}

/// Primo elemento del testo, saltando spazi, dichiarazione e commenti.
fn radice_xml(testo: &str) -> Option<String> {
    let mut t = testo.trim_start();
    loop {
        if let Some(r) = t.strip_prefix("<?") {
            t = r.split_once("?>")?.1.trim_start();
        } else if let Some(r) = t.strip_prefix("<!--") {
            t = r.split_once("-->")?.1.trim_start();
        } else {
            break;
        }
    }
    let r = t.strip_prefix('<')?;
    let nome: String = r
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
        .collect();
    (!nome.is_empty() && nome.chars().next().is_some_and(char::is_alphabetic)).then_some(nome)
}

/// Legge il contenitore di un file DCF.
///
/// # Errors
/// [`ErroreDcf::NonDcf`] se mancano le firme, [`ErroreDcf::Danneggiato`] se la
/// geometria dei settori è incoerente, [`ErroreDcf::OltreLimiti`] oltre i limiti di
/// sicurezza. I singoli flussi non decomprimibili non bloccano la lettura.
pub fn leggi(dati: &[u8]) -> Result<Contenitore, ErroreDcf> {
    if dati.get(..8) != Some(FIRMA_FILE.as_slice()) {
        return Err(ErroreDcf::NonDcf("firma iniziale assente".into()));
    }
    if dati.get(INIZIO_SETTORI..INIZIO_SETTORI + 8) != Some(FIRMA_SETTORI.as_slice()) {
        return Err(ErroreDcf::NonDcf("testata dei settori assente".into()));
    }
    let dimensione = u32_le(dati, INIZIO_SETTORI + 0x10)
        .ok_or_else(|| ErroreDcf::Danneggiato("testata dei settori troncata".into()))?
        as usize;
    if dimensione != DIMENSIONE_SETTORE {
        return Err(ErroreDcf::Danneggiato(format!(
            "settori da {dimensione} byte: è supportata solo la dimensione {DIMENSIONE_SETTORE}"
        )));
    }
    let settori_dichiarati = u32_le(dati, INIZIO_SETTORI + 0x30)
        .ok_or_else(|| ErroreDcf::Danneggiato("testata dei settori troncata".into()))?
        as usize;
    let settori = (dati.len() - INIZIO_SETTORI) / DIMENSIONE_SETTORE;
    let settore = |n: usize| {
        let inizio = INIZIO_SETTORI + n * DIMENSIONE_SETTORE;
        dati.get(inizio..inizio + DIMENSIONE_SETTORE)
    };
    let tipo = |n: usize| settore(n).and_then(|s| u32_le(s, 8));
    let inizia_flusso = |n: usize| {
        settore(n).and_then(|s| s.get(TESTATA_SETTORE..TESTATA_SETTORE + 4))
            == Some(FIRMA_FLUSSO.as_slice())
    };

    let mut flussi = Vec::new();
    let mut totale = 0usize;
    let mut n = 1;
    while n < settori {
        if tipo(n) != Some(TIPO_DATI) || !inizia_flusso(n) {
            n += 1;
            continue;
        }
        let inizio = n;
        let mut contenuto = Vec::new();
        loop {
            if let Some(s) = settore(n).and_then(|s| s.get(TESTATA_SETTORE..)) {
                contenuto.extend_from_slice(s);
            }
            n += 1;
            if n >= settori || tipo(n) != Some(TIPO_DATI) || inizia_flusso(n) {
                break;
            }
        }
        let (byte, contenuto) = match decomprimi_flusso(&contenuto, &mut totale) {
            Ok(b) => {
                let c = if b.get(4..4 + MARCATORE_PROTEZIONE.len()) == Some(MARCATORE_PROTEZIONE) {
                    Contenuto::Protetto
                } else if let Some((radice, testo)) = come_xml(&b) {
                    Contenuto::Xml { radice, testo }
                } else {
                    Contenuto::Binario
                };
                (b.len(), c)
            }
            Err(e) if e.contains("in totale") => return Err(ErroreDcf::OltreLimiti(e)),
            Err(e) => (0, Contenuto::Illeggibile(e)),
        };
        flussi.push(Flusso {
            settore: inizio,
            settori: n - inizio,
            byte,
            contenuto,
        });
    }
    Ok(Contenitore {
        settori_dichiarati,
        settori,
        flussi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radici_e_codifiche() {
        assert_eq!(
            radice_xml("\t<CollectionEP CountId=\"1\">").as_deref(),
            Some("CollectionEP")
        );
        assert_eq!(
            radice_xml("<?xml version=\"1.0\"?>\n<!-- c --><A/>").as_deref(),
            Some("A")
        );
        assert_eq!(radice_xml("testo"), None);
        let utf16: Vec<u8> = "<Radice/>"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(come_xml(&utf16).map(|(r, _)| r).as_deref(), Some("Radice"));
        assert_eq!(come_xml(&[0, 1, 2, 3, 200, 201]), None);
    }

    #[test]
    fn firme_obbligatorie() {
        assert!(matches!(leggi(b"PK\x03\x04"), Err(ErroreDcf::NonDcf(_))));
        let mut d = vec![0u8; 0x200];
        d[..8].copy_from_slice(FIRMA_FILE);
        assert!(matches!(leggi(&d), Err(ErroreDcf::NonDcf(_))));
    }
}
