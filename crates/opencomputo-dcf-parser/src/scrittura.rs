//! Scrittura del contenitore DCF (struttura in [`crate::struttura`]).
//!
//! Il file prodotto ha la forma dei prezzari distribuiti in DCF (contenitore 2,6):
//! * testata e settore 0;
//! * mappa dei settori liberi e i suoi due settori ausiliari (1–3);
//! * settori dell'indice consecutivi dal 4;
//! * per ogni flusso, un settore con l'elenco dei suoi settori seguito dai settori di
//!   dati;
//! * il totale dei settori, escluso il settore 0, è un multiplo di 8.
//!
//! Sono calcolati tutti i campi di controllo noti: somma XOR di ogni settore, CRC dei
//! pezzi, fine dei pezzi, mappe e conteggi. I campi senza significato noto, che nei
//! file reali contengono valori diversi da file a file, sono scritti a zero. I
//! contenuti sono scritti in chiaro: nessuna protezione.

use std::io::Write as _;

use chrono::NaiveDateTime;
use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::contenitore::{
    DIMENSIONE_SETTORE, FIRMA_FILE, FIRMA_FLUSSO, FIRMA_SETTORI, INIZIO_SETTORI, TESTATA_SETTORE,
};
use crate::errore::ErroreDcf;
use crate::struttura::{
    ATTRIBUTO_CARTELLA, DATI_SETTORE, DIMENSIONE_VOCE, NESSUNO, POSIZIONE_NOME, VOCI_PER_SETTORE,
    crc16, tipo, xor_parole,
};

/// Dimensione massima decompressa di un pezzo.
pub const PEZZO: usize = 1 << 20;
/// Settore del primo settore dell'indice.
const PRIMO_INDICE: usize = 4;
/// Tipo dei settori dell'indice nei prezzari distribuiti in DCF.
const TIPO_INDICE: u32 = 0x7400_0001;
/// Versione del contenitore nella testata: 2,6 (f32), quella dei prezzari distribuiti
/// in DCF, letta anche dalle versioni recenti del programma. In questa versione la fine
/// dei pezzi è assoluta.
const VERSIONE: [u32; 2] = [0x4026_7665, 0x4026_6666];

/// Elemento da scrivere nell'indice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elemento {
    /// Cartella.
    Cartella {
        /// Nome.
        nome: String,
        /// Cartella genitrice (indice nell'elenco degli elementi; `None` = radice).
        genitore: Option<usize>,
    },
    /// Flusso.
    File {
        /// Nome.
        nome: String,
        /// Cartella genitrice.
        genitore: Option<usize>,
        /// Contenuto (non compresso).
        contenuto: Vec<u8>,
    },
}

impl Elemento {
    fn nome(&self) -> &str {
        match self {
            Self::Cartella { nome, .. } | Self::File { nome, .. } => nome,
        }
    }

    fn genitore(&self) -> Option<usize> {
        match self {
            Self::Cartella { genitore, .. } | Self::File { genitore, .. } => *genitore,
        }
    }
}

fn scrivi_u32(buf: &mut [u8], pos: usize, v: u32) {
    if let Some(b) = buf.get_mut(pos..pos + 4) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

fn scrivi_u64(buf: &mut [u8], pos: usize, v: u64) {
    if let Some(b) = buf.get_mut(pos..pos + 8) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

fn u32_di(v: usize) -> Result<u32, ErroreDcf> {
    u32::try_from(v).map_err(|_| ErroreDcf::OltreLimiti(format!("valore {v} oltre 32 bit")))
}

/// Data e ora in formato FILETIME (centinaia di nanosecondi dal 1601).
#[must_use]
pub fn filetime(t: NaiveDateTime) -> u64 {
    let secondi = t.and_utc().timestamp().saturating_add(11_644_473_600);
    u64::try_from(secondi)
        .unwrap_or(0)
        .saturating_mul(10_000_000)
}

/// Comprime un contenuto nel formato dei flussi: testata `AACS` e pezzi zlib da
/// 1 MiB al massimo, ognuno con il CRC dei byte decompressi.
///
/// # Errors
/// Contenuto oltre i limiti del formato.
pub fn flusso(contenuto: &[u8]) -> Result<Vec<u8>, ErroreDcf> {
    let parti: Vec<&[u8]> = if contenuto.is_empty() {
        vec![contenuto]
    } else {
        contenuto.chunks(PEZZO).collect()
    };
    let mut out = vec![0u8; 0x80];
    if let Some(f) = out.get_mut(..4) {
        f.copy_from_slice(FIRMA_FLUSSO);
    }
    scrivi_u32(&mut out, 4, u32_di(PEZZO)?);
    scrivi_u32(&mut out, 12, u32_di(parti.len())?);
    scrivi_u32(&mut out, 16, 0x4000_0000);
    scrivi_u32(&mut out, 20, 2);
    for p in parti {
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(p)
            .and_then(|()| z.flush())
            .map_err(|e| ErroreDcf::Danneggiato(e.to_string()))?;
        let compresso = z
            .finish()
            .map_err(|e| ErroreDcf::Danneggiato(e.to_string()))?;
        let inizio = out.len();
        out.resize(inizio + 16, 0);
        let fine = inizio + 16 + compresso.len();
        scrivi_u32(&mut out, inizio, u32_di(compresso.len())?);
        scrivi_u32(&mut out, inizio + 4, u32_di(p.len())?);
        scrivi_u32(&mut out, inizio + 8, u32::from(crc16(p)));
        scrivi_u32(&mut out, inizio + 12, u32_di(fine)?);
        out.extend_from_slice(&compresso);
    }
    Ok(out)
}

fn nuovo_settore() -> Vec<u8> {
    vec![0u8; DIMENSIONE_SETTORE]
}

/// Settore con testata: tipo, somma XOR dei dati e campo 0.
fn chiudi_settore(s: &mut [u8], campo0: u32, tipo_settore: u32) {
    let somma = xor_parole(s.get(TESTATA_SETTORE..).unwrap_or_default());
    scrivi_u32(s, 0, campo0);
    scrivi_u32(s, 4, somma);
    scrivi_u32(s, 8, tipo_settore);
}

/// Scrive il contenitore con gli elementi indicati, nell'ordine dell'indice.
///
/// # Errors
/// Genitori non validi, nomi troppo lunghi, file oltre i limiti del formato.
pub fn scrivi(elementi: &[Elemento], data: NaiveDateTime) -> Result<Vec<u8>, ErroreDcf> {
    for (i, e) in elementi.iter().enumerate() {
        if let Some(g) = e.genitore()
            && !matches!(elementi.get(g), Some(Elemento::Cartella { .. }) if g < i)
        {
            return Err(ErroreDcf::Danneggiato(format!(
                "«{}»: la cartella genitrice deve precederlo",
                e.nome()
            )));
        }
        if e.nome().is_empty()
            || !e.nome().is_ascii()
            || e.nome().len() >= DIMENSIONE_VOCE - POSIZIONE_NOME
        {
            return Err(ErroreDcf::Danneggiato(format!(
                "nome non valido per l'indice: «{}»",
                e.nome()
            )));
        }
    }
    let flussi: Vec<Option<Vec<u8>>> = elementi
        .iter()
        .map(|e| match e {
            Elemento::File { contenuto, .. } => flusso(contenuto).map(Some),
            Elemento::Cartella { .. } => Ok(None),
        })
        .collect::<Result<_, _>>()?;

    // Disposizione dei settori.
    let settori_indice = elementi.len().div_ceil(VOCI_PER_SETTORE).max(1);
    let mut prossimo = PRIMO_INDICE + settori_indice;
    let mut posizioni: Vec<Option<(usize, Vec<usize>)>> = Vec::with_capacity(flussi.len());
    for f in &flussi {
        match f {
            Some(b) => {
                let quanti = b.len().div_ceil(DATI_SETTORE).max(1);
                if quanti > DATI_SETTORE / 4 {
                    return Err(ErroreDcf::OltreLimiti(format!(
                        "flusso di {} byte: oltre {} settori",
                        b.len(),
                        DATI_SETTORE / 4
                    )));
                }
                let elenco = prossimo;
                let dati: Vec<usize> = (elenco + 1..elenco + 1 + quanti).collect();
                prossimo = elenco + 1 + quanti;
                posizioni.push(Some((elenco, dati)));
            }
            None => posizioni.push(None),
        }
    }
    let usati = prossimo;
    let settori = 1 + (usati - 1).div_ceil(8) * 8;
    let byte_mappa = (settori - 1) / 8;
    if byte_mappa > DATI_SETTORE {
        return Err(ErroreDcf::OltreLimiti(format!(
            "{settori} settori: oltre la mappa dei settori liberi"
        )));
    }

    let mut out = vec![0u8; INIZIO_SETTORI + settori * DIMENSIONE_SETTORE];
    if let Some(f) = out.get_mut(..8) {
        f.copy_from_slice(FIRMA_FILE);
    }
    scrivi_u64(&mut out, 8, 0x7FFF_FFFF_FFFF_FFFE);
    let mut scritti: Vec<(usize, Vec<u8>)> = Vec::new();

    // Settore 0.
    let mut s0 = nuovo_settore();
    if let Some(f) = s0.get_mut(..8) {
        f.copy_from_slice(FIRMA_SETTORI);
    }
    let [v1, v2] = VERSIONE;
    for (pos, v) in [
        (0x08, v1),
        (0x0C, v2),
        (0x10, u32_di(DIMENSIONE_SETTORE)?),
        (0x14, 8),
        (0x18, 1),
        (0x1C, 1),
        (0x20, 1),
        (0x24, u32_di(settori_indice)?),
        (0x28, u32_di(PRIMO_INDICE)?),
        (0x2C, u32_di(elementi.len())?),
        (0x30, u32_di(settori)?),
        (0xE0, u32_di(DIMENSIONE_VOCE)?),
    ] {
        scrivi_u32(&mut s0, pos, v);
    }
    scritti.push((0, s0));

    // Mappa dei settori liberi e settori ausiliari.
    let mut mappa = nuovo_settore();
    let mut gruppi = nuovo_settore();
    let mut ultimo = nuovo_settore();
    for k in 0..DATI_SETTORE * 8 {
        let n = k + 1;
        let libero = n >= usati;
        if libero && let Some(b) = mappa.get_mut(TESTATA_SETTORE + k / 8) {
            *b |= 1 << (k % 8);
        }
    }
    for k in 0..DATI_SETTORE * 8 {
        if k >= byte_mappa
            && let Some(b) = gruppi.get_mut(TESTATA_SETTORE + k / 8)
        {
            *b |= 1 << (k % 8);
        }
    }
    let k = byte_mappa - 1;
    if let Some(b) = ultimo.get_mut(TESTATA_SETTORE + k / 8) {
        *b |= 1 << (k % 8);
    }
    for (n, mut s, t, campo4) in [
        (1, mappa, tipo::MAPPA, 0x64),
        (2, gruppi, tipo::GRUPPI, 0x14),
        (3, ultimo, tipo::ULTIMO, 0x14),
    ] {
        scrivi_u32(&mut s, 16, campo4);
        scrivi_u32(&mut s, 20, 1);
        chiudi_settore(&mut s, 0, t);
        scritti.push((n, s));
    }

    // Indice.
    let istante = filetime(data);
    for j in 0..settori_indice {
        let n = PRIMO_INDICE + j;
        let mut s = nuovo_settore();
        for k in 0..VOCI_PER_SETTORE {
            let i = j * VOCI_PER_SETTORE + k;
            let Some(e) = elementi.get(i) else { break };
            let base = TESTATA_SETTORE + k * DIMENSIONE_VOCE;
            let (elenco, byte, attributi, cartella) = match (e, posizioni.get(i), flussi.get(i)) {
                (Elemento::File { .. }, Some(Some((elenco, _))), Some(Some(b))) => {
                    (u32_di(*elenco)?, u32_di(b.len())?, 0, 0)
                }
                _ => (NESSUNO, 0, ATTRIBUTO_CARTELLA, 1),
            };
            scrivi_u32(&mut s, base, elenco);
            scrivi_u32(&mut s, base + 4, byte);
            for d in [0x0C, 0x14, 0x1C] {
                scrivi_u64(&mut s, base + d, istante);
            }
            scrivi_u32(&mut s, base + 0x24, attributi);
            let genitore = e.genitore().map_or(Ok(NESSUNO), u32_di)?;
            scrivi_u32(&mut s, base + 0x28, genitore);
            scrivi_u32(&mut s, base + 0xDC, cartella);
            if let Some(nome) =
                s.get_mut(base + POSIZIONE_NOME..base + POSIZIONE_NOME + e.nome().len())
            {
                nome.copy_from_slice(e.nome().as_bytes());
            }
        }
        scrivi_u32(&mut s, 24, 2);
        let successivo = if j + 1 < settori_indice { n + 1 } else { n };
        chiudi_settore(&mut s, u32_di(successivo)?, TIPO_INDICE);
        scritti.push((n, s));
    }

    // Flussi: elenco dei settori e dati.
    for (pos, f) in posizioni.iter().zip(&flussi) {
        let (Some((elenco, dati)), Some(byte)) = (pos, f) else {
            continue;
        };
        let mut s = nuovo_settore();
        for (j, d) in dati.iter().enumerate() {
            scrivi_u32(&mut s, TESTATA_SETTORE + 4 * j, u32_di(*d)?);
        }
        scrivi_u32(&mut s, 16, 1);
        scrivi_u32(&mut s, 24, 1);
        chiudi_settore(&mut s, NESSUNO, tipo::ELENCO);
        scritti.push((*elenco, s));
        for (j, d) in dati.iter().enumerate() {
            let mut s = nuovo_settore();
            let inizio = j * DATI_SETTORE;
            let pezzo = byte
                .get(inizio..(inizio + DATI_SETTORE).min(byte.len()))
                .unwrap_or_default();
            if let Some(area) = s.get_mut(TESTATA_SETTORE..TESTATA_SETTORE + pezzo.len()) {
                area.copy_from_slice(pezzo);
            }
            chiudi_settore(&mut s, NESSUNO, tipo::DATI);
            scritti.push((*d, s));
        }
    }

    for (n, s) in scritti {
        let inizio = INIZIO_SETTORI + n * DIMENSIONE_SETTORE;
        if let Some(dest) = out.get_mut(inizio..inizio + DIMENSIONE_SETTORE) {
            dest.copy_from_slice(&s);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{contenitore, struttura};

    fn data() -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 21)
            .and_then(|d| d.and_hms_opt(12, 0, 0))
            .unwrap_or_default()
    }

    #[test]
    fn contenitore_riletto_e_coerente() {
        let grande: Vec<u8> = (0..3_000_000u32)
            .flat_map(|i| (i % 251).to_le_bytes())
            .collect();
        let elementi = vec![
            Elemento::Cartella {
                nome: "Documento".into(),
                genitore: None,
            },
            Elemento::File {
                nome: "CollectionGP.xml".into(),
                genitore: Some(0),
                contenuto: b"\t<CollectionGP CountId=\"1\" />\r\n".to_vec(),
            },
            Elemento::File {
                nome: "Grande".into(),
                genitore: Some(0),
                contenuto: grande.clone(),
            },
            Elemento::File {
                nome: "Versione".into(),
                genitore: None,
                contenuto: vec![0x0A, 0xD7, 0xA3, 0x70, 0x3D, 0x0A, 0x14, 0x40],
            },
        ];
        let b = scrivi(&elementi, data()).expect("scrittura");
        assert_eq!((b.len() - INIZIO_SETTORI) % DIMENSIONE_SETTORE, 0);
        assert_eq!((b.len() - INIZIO_SETTORI) / DIMENSIONE_SETTORE % 8, 1);
        assert_eq!(
            struttura::verifica(&b).expect("indice"),
            Vec::<String>::new()
        );
        let s = struttura::leggi(&b).expect("indice");
        assert_eq!(s.voci.len(), 4);
        assert_eq!(s.percorso(1), "Documento/CollectionGP.xml");
        assert_eq!(s.percorso(3), "Versione");
        let i = s.trova("Documento/Grande").expect("voce");
        let letto = struttura::decomprimi(&s.flusso(&b, i)).expect("flusso");
        assert_eq!(letto, grande);
        // Il lettore sequenziale trova gli stessi flussi.
        let c = contenitore::leggi(&b).expect("contenitore");
        assert_eq!(c.settori, c.settori_dichiarati);
        assert_eq!(c.xml("CollectionGP").count(), 1);
        assert_eq!(c.flussi.len(), 3);
        // Deterministico.
        assert_eq!(scrivi(&elementi, data()).expect("scrittura"), b);
    }

    #[test]
    fn errori() {
        let figlio_prima = vec![Elemento::File {
            nome: "a".into(),
            genitore: Some(0),
            contenuto: Vec::new(),
        }];
        assert!(scrivi(&figlio_prima, data()).is_err());
        let nome = vec![Elemento::Cartella {
            nome: "à".into(),
            genitore: None,
        }];
        assert!(scrivi(&nome, data()).is_err());
        assert_eq!(filetime(data()), 134_370_576_000_000_000);
    }
}
