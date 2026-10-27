//! Struttura completa del contenitore DCF: indice delle voci (cartelle e flussi),
//! settori di ogni flusso, controlli di integrità.
//!
//! Ricavata il 2026-10-21 dall'analisi di tre file reali (due prezzari in chiaro e un
//! documento di contabilità, di cui si è letto solo il contenitore). Vedi
//! `docs/adr/0016-export-dcf.md`. Il modulo [`crate::contenitore`] legge i flussi in
//! sequenza; questo modulo segue l'indice, come il programma che scrive i file.
//!
//! ```text
//! 0x000          "AAMVHFSS", u64 0x7FFF_FFFF_FFFF_FFFE, zeri fino a 0x118
//! settore 0      "AASFSSGN" e parametri (u32):
//!                +0x08, +0x0C versione del contenitore (f32: 2,6 nei prezzari, 8,0 nei
//!                documenti salvati da versioni recenti) · +0x10 dimensione dei settori
//!                (4096) · +0x14 8 · +0x18, +0x1C, +0x20 1 · +0x24 settori dell'indice
//!                · +0x28 primo settore dell'indice (4) · +0x2C voci dell'indice
//!                · +0x30 settori del file · +0xE0 dimensione di una voce (512)
//! settori 1..N   testata di 0x20 byte (8 × u32): +0x04 = XOR delle parole u32 dei
//!                4064 byte di dati; +0x08 = tipo
//!   tipo 4       mappa dei settori liberi: bit k (dal meno significativo) = settore
//!                k + 1, 1 = libero
//!   tipo 2       un bit per byte della mappa: 0 = il byte ha settori in uso
//!   tipo 3       un bit per byte della mappa: solo l'ultimo byte a 1
//!   tipo …01     settore dell'indice (byte meno significativo 1): +0x00 settore
//!                successivo dell'indice (sé stesso nell'ultimo); 7 voci da 512 byte
//!   tipo 5       elenco dei settori di dati di un flusso (u32)
//!   tipo 6       dati di un flusso
//! voce (512 B)   +0x00 settore dell'elenco (0xFFFFFFFF per le cartelle)
//!                +0x04 byte del flusso · +0x0C, +0x14, +0x1C date FILETIME
//!                +0x24 attributi (0x10 cartella) · +0x28 voce genitrice
//!                (0xFFFFFFFF = radice) · +0xDC 1 per le cartelle · +0xE0 nome
//! flusso         "AACS", u32 dimensione massima dei pezzi (1 MiB), 0, u32 pezzi,
//!                0x40000000, 2, zeri; da +0x80 i pezzi: [u32 byte compressi,
//!                u32 byte decompressi, u32 CRC-16/ARC dei byte decompressi,
//!                u32 fine del pezzo nel flusso (nei contenitori 8.0 contata dopo i
//!                primi 0x80 byte)][zlib]
//! ```
//!
//! Gli altri campi delle testate contengono valori diversi da file a file (anche
//! indirizzi di memoria del programma che li ha scritti): il lettore li ignora.

use crate::contenitore::{
    DIMENSIONE_SETTORE, FIRMA_FILE, FIRMA_FLUSSO, FIRMA_SETTORI, INIZIO_SETTORI, TESTATA_SETTORE,
};
use crate::errore::ErroreDcf;

/// Byte di dati di un settore.
pub const DATI_SETTORE: usize = DIMENSIONE_SETTORE - TESTATA_SETTORE;
/// Dimensione di una voce dell'indice.
pub const DIMENSIONE_VOCE: usize = 0x200;
/// Voci dell'indice in un settore.
pub const VOCI_PER_SETTORE: usize = 7;
/// Posizione del nome nella voce.
pub const POSIZIONE_NOME: usize = 0xE0;
/// Valore «nessuno» (radice, cartella).
pub const NESSUNO: u32 = u32::MAX;
/// Attributo delle cartelle.
pub const ATTRIBUTO_CARTELLA: u32 = 0x10;

/// Tipi dei settori.
pub mod tipo {
    /// Mappa dei settori liberi.
    pub const MAPPA: u32 = 4;
    /// Mappa dei byte della mappa.
    pub const GRUPPI: u32 = 2;
    /// Ultimo byte della mappa.
    pub const ULTIMO: u32 = 3;
    /// Elenco dei settori di un flusso.
    pub const ELENCO: u32 = 5;
    /// Dati.
    pub const DATI: u32 = 6;
    /// Indice (byte meno significativo; gli altri variano).
    pub const INDICE: u32 = 1;
}

/// Voce dell'indice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voce {
    /// Nome.
    pub nome: String,
    /// `true` per le cartelle.
    pub cartella: bool,
    /// Voce genitrice (indice nell'elenco delle voci).
    pub genitore: Option<usize>,
    /// Settore dell'elenco dei settori del flusso.
    pub elenco: Option<usize>,
    /// Byte del flusso.
    pub byte: usize,
    /// Settori di dati del flusso, in ordine.
    pub settori: Vec<usize>,
}

/// Struttura del contenitore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struttura {
    /// Settori del file.
    pub settori: usize,
    /// Settori dell'indice, in ordine.
    pub settori_indice: Vec<usize>,
    /// Voci dell'indice.
    pub voci: Vec<Voce>,
}

pub(crate) fn u32_le(dati: &[u8], pos: usize) -> Option<u32> {
    let b = dati.get(pos..pos.checked_add(4)?)?;
    Some(u32::from_le_bytes([
        *b.first()?,
        *b.get(1)?,
        *b.get(2)?,
        *b.get(3)?,
    ]))
}

fn settore(dati: &[u8], n: usize) -> Option<&[u8]> {
    let inizio = INIZIO_SETTORI.checked_add(n.checked_mul(DIMENSIONE_SETTORE)?)?;
    dati.get(inizio..inizio.checked_add(DIMENSIONE_SETTORE)?)
}

/// XOR delle parole u32 (little endian) dei dati.
#[must_use]
pub fn xor_parole(dati: &[u8]) -> u32 {
    dati.chunks(4).fold(0, |x, c| {
        let mut b = [0u8; 4];
        for (d, s) in b.iter_mut().zip(c) {
            *d = *s;
        }
        x ^ u32::from_le_bytes(b)
    })
}

/// CRC-16/ARC (polinomio 0x8005 riflesso, valore iniziale 0).
#[must_use]
pub fn crc16(dati: &[u8]) -> u16 {
    let mut c: u16 = 0;
    for b in dati {
        c ^= u16::from(*b);
        for _ in 0..8 {
            c = if c & 1 == 1 {
                (c >> 1) ^ 0xA001
            } else {
                c >> 1
            };
        }
    }
    c
}

fn danneggiato(m: impl Into<String>) -> ErroreDcf {
    ErroreDcf::Danneggiato(m.into())
}

/// Legge l'indice del contenitore.
///
/// # Errors
/// Firme assenti, indice o elenchi dei settori incoerenti.
pub fn leggi(dati: &[u8]) -> Result<Struttura, ErroreDcf> {
    if dati.get(..8) != Some(FIRMA_FILE.as_slice()) {
        return Err(ErroreDcf::NonDcf("firma iniziale assente".into()));
    }
    let s0 = settore(dati, 0).ok_or_else(|| ErroreDcf::NonDcf("settore 0 assente".into()))?;
    if s0.get(..8) != Some(FIRMA_SETTORI.as_slice()) {
        return Err(ErroreDcf::NonDcf("testata dei settori assente".into()));
    }
    if u32_le(s0, 0x10) != u32::try_from(DIMENSIONE_SETTORE).ok() {
        return Err(danneggiato("dimensione dei settori non supportata"));
    }
    let settori = (dati.len() - INIZIO_SETTORI) / DIMENSIONE_SETTORE;
    let voci_dichiarate = u32_le(s0, 0x2C).unwrap_or(0) as usize;
    let mut n = u32_le(s0, 0x28).unwrap_or(0) as usize;
    let mut settori_indice = Vec::new();
    let mut grezze = Vec::new();
    while grezze.len() < voci_dichiarate {
        if n == 0 || n >= settori || settori_indice.contains(&n) {
            return Err(danneggiato(format!("indice interrotto al settore {n}")));
        }
        let s = settore(dati, n).ok_or_else(|| danneggiato("settore dell'indice assente"))?;
        if u32_le(s, 8).map(|t| t & 0xFF) != Some(tipo::INDICE) {
            return Err(danneggiato(format!(
                "il settore {n} non è un settore dell'indice"
            )));
        }
        settori_indice.push(n);
        for k in 0..VOCI_PER_SETTORE {
            if grezze.len() >= voci_dichiarate {
                break;
            }
            let inizio = TESTATA_SETTORE + k * DIMENSIONE_VOCE;
            if let Some(v) = s.get(inizio..inizio + DIMENSIONE_VOCE) {
                grezze.push(v);
            }
        }
        let successivo = u32_le(s, 0).unwrap_or(0) as usize;
        if successivo == n {
            break;
        }
        n = successivo;
    }
    if grezze.len() != voci_dichiarate {
        return Err(danneggiato(format!(
            "indice con {} voci, {voci_dichiarate} dichiarate",
            grezze.len()
        )));
    }
    let mut voci = Vec::with_capacity(grezze.len());
    for (i, v) in grezze.iter().enumerate() {
        let nome_byte = v.get(POSIZIONE_NOME..).unwrap_or_default();
        let fine = nome_byte
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(nome_byte.len());
        let nome: String = nome_byte
            .get(..fine)
            .unwrap_or_default()
            .iter()
            .map(|b| char::from(*b))
            .collect();
        let elenco = u32_le(v, 0).unwrap_or(NESSUNO);
        let byte = u32_le(v, 4).unwrap_or(0) as usize;
        let attributi = u32_le(v, 0x24).unwrap_or(0);
        let genitore = u32_le(v, 0x28).unwrap_or(NESSUNO);
        let cartella = attributi & ATTRIBUTO_CARTELLA != 0;
        let genitore = (genitore != NESSUNO).then_some(genitore as usize);
        if genitore.is_some_and(|g| g >= grezze.len()) {
            return Err(danneggiato(format!(
                "voce {i} «{nome}»: genitore inesistente"
            )));
        }
        let mut voce = Voce {
            nome,
            cartella,
            genitore,
            elenco: None,
            byte,
            settori: Vec::new(),
        };
        if !cartella && elenco != NESSUNO {
            let e = elenco as usize;
            let s = settore(dati, e)
                .ok_or_else(|| danneggiato(format!("voce «{}»: elenco assente", voce.nome)))?;
            if u32_le(s, 8) != Some(tipo::ELENCO) {
                return Err(danneggiato(format!(
                    "voce «{}»: il settore {e} non è un elenco di settori",
                    voce.nome
                )));
            }
            let quanti = byte.div_ceil(DATI_SETTORE);
            if quanti > DATI_SETTORE / 4 {
                return Err(danneggiato(format!(
                    "voce «{}»: flusso oltre un elenco di settori",
                    voce.nome
                )));
            }
            for j in 0..quanti {
                let d = u32_le(s, TESTATA_SETTORE + 4 * j).unwrap_or(0) as usize;
                if d == 0 || d >= settori {
                    return Err(danneggiato(format!(
                        "voce «{}»: settore di dati {d} fuori dal file",
                        voce.nome
                    )));
                }
                voce.settori.push(d);
            }
            voce.elenco = Some(e);
        }
        voci.push(voce);
    }
    Ok(Struttura {
        settori,
        settori_indice,
        voci,
    })
}

impl Struttura {
    /// Percorso della voce (es. `Documento/CollectionEP.xml`).
    #[must_use]
    pub fn percorso(&self, i: usize) -> String {
        let mut parti = Vec::new();
        let mut corrente = Some(i);
        let mut passi = 0;
        while let Some(k) = corrente {
            let Some(v) = self.voci.get(k) else { break };
            parti.push(v.nome.as_str());
            corrente = v.genitore;
            passi += 1;
            if passi > self.voci.len() {
                break;
            }
        }
        parti.reverse();
        parti.join("/")
    }

    /// Indice della voce con il percorso indicato.
    #[must_use]
    pub fn trova(&self, percorso: &str) -> Option<usize> {
        (0..self.voci.len()).find(|i| self.percorso(*i) == percorso)
    }

    /// Byte del flusso della voce (testata `AACS` e pezzi compressi).
    #[must_use]
    pub fn flusso(&self, dati: &[u8], i: usize) -> Vec<u8> {
        let Some(v) = self.voci.get(i) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(v.byte);
        for s in &v.settori {
            if let Some(d) = settore(dati, *s).and_then(|s| s.get(TESTATA_SETTORE..)) {
                out.extend_from_slice(d);
            }
        }
        out.truncate(v.byte);
        out
    }
}

/// Pezzi di un flusso: (byte compressi, byte decompressi, CRC, fine assoluta nel
/// flusso).
///
/// # Errors
/// Testata assente o pezzi incoerenti.
pub fn pezzi(flusso: &[u8]) -> Result<Vec<(usize, usize, u32, usize)>, String> {
    if flusso.get(..4) != Some(FIRMA_FLUSSO.as_slice()) {
        return Err("firma di flusso assente".into());
    }
    let n = u32_le(flusso, 12).ok_or("testata troncata")? as usize;
    // Ogni pezzo ha una testata di 16 byte: un numero più grande è un file danneggiato
    // (e non deve diventare un'allocazione enorme).
    if n > flusso.len().saturating_sub(0x80) / 16 {
        return Err(format!(
            "{n} pezzi dichiarati: più di quanti ne stiano nel flusso"
        ));
    }
    let mut out = Vec::with_capacity(n);
    let mut pos = 0x80;
    for i in 0..n {
        let c = u32_le(flusso, pos).ok_or("testata di pezzo troncata")? as usize;
        let d = u32_le(flusso, pos + 4).ok_or("testata di pezzo troncata")? as usize;
        let crc = u32_le(flusso, pos + 8).ok_or("testata di pezzo troncata")?;
        let fine = u32_le(flusso, pos + 12).ok_or("testata di pezzo troncata")? as usize;
        pos += 16;
        // Fine del pezzo: assoluta nei contenitori 2.6, dopo i primi 0x80 byte nei 8.0.
        let assoluta = if fine == pos + c {
            fine
        } else if fine + 0x80 == pos + c {
            pos + c
        } else {
            return Err(format!(
                "pezzo {i}: fine dichiarata {fine}, calcolata {}",
                pos + c
            ));
        };
        if assoluta > flusso.len() {
            return Err(format!("pezzo {i} oltre la fine del flusso"));
        }
        out.push((c, d, crc, assoluta));
        pos = assoluta;
    }
    if pos != flusso.len() {
        return Err(format!("{} byte oltre l'ultimo pezzo", flusso.len() - pos));
    }
    Ok(out)
}

/// Dimensione massima di un pezzo decompresso (i pezzi dei file reali sono di 64 KB).
const MASSIMO_DECOMPRESSO: usize = 64 << 20;

/// Decomprime il flusso di una voce e controlla i CRC dei pezzi.
///
/// # Errors
/// Flusso incoerente o pezzo non decomprimibile.
pub fn decomprimi(flusso: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for (i, (c, d, crc, fine)) in pezzi(flusso)?.into_iter().enumerate() {
        let compresso = flusso.get(fine - c..fine).ok_or("pezzo fuori dal flusso")?;
        if d > MASSIMO_DECOMPRESSO {
            return Err(format!("pezzo {i}: {d} byte dichiarati, oltre il massimo"));
        }
        // Si legge al più un byte oltre il dichiarato: una «bomba» zlib si ferma subito.
        let z = flate2::read::ZlibDecoder::new(compresso);
        let mut pezzo = Vec::with_capacity(d.min(1 << 20));
        std::io::Read::read_to_end(&mut std::io::Read::take(z, d as u64 + 1), &mut pezzo)
            .map_err(|e| format!("pezzo {i}: {e}"))?;
        if pezzo.len() != d {
            return Err(format!("pezzo {i}: {} byte, {d} dichiarati", pezzo.len()));
        }
        if u32::from(crc16(&pezzo)) != crc {
            return Err(format!("pezzo {i}: CRC diverso"));
        }
        out.extend_from_slice(&pezzo);
    }
    Ok(out)
}

/// Controlla la coerenza del contenitore: somme di controllo dei settori, mappa dei
/// settori liberi, conteggi della testata, pezzi dei flussi. Restituisce i problemi
/// trovati (vuoto se il file è coerente).
///
/// # Errors
/// Indice illeggibile.
pub fn verifica(dati: &[u8]) -> Result<Vec<String>, ErroreDcf> {
    let s = leggi(dati)?;
    let mut problemi = Vec::new();
    let s0 = settore(dati, 0).unwrap_or_default();
    if u32_le(s0, 0x30).map(|v| v as usize) != Some(s.settori) {
        problemi.push(format!(
            "la testata dichiara {:?} settori, il file ne ha {}",
            u32_le(s0, 0x30),
            s.settori
        ));
    }
    if u32_le(s0, 0x24).map(|v| v as usize) != Some(s.settori_indice.len()) {
        problemi.push("numero dei settori dell'indice diverso dalla testata".into());
    }
    for n in 1..s.settori {
        let Some(sett) = settore(dati, n) else {
            continue;
        };
        let tipo_settore = u32_le(sett, 8).unwrap_or(0);
        if tipo_settore == 0 && sett.iter().all(|b| *b == 0) {
            continue;
        }
        let dichiarata = u32_le(sett, 4).unwrap_or(0);
        let calcolata = xor_parole(sett.get(TESTATA_SETTORE..).unwrap_or_default());
        if dichiarata != calcolata {
            problemi.push(format!("settore {n}: somma di controllo diversa"));
        }
    }
    // Mappa dei settori liberi: i settori usati dall'indice devono risultare occupati.
    let mappa = settore(dati, 1).and_then(|m| m.get(TESTATA_SETTORE..));
    let libero = |n: usize| -> bool {
        let k = n - 1;
        mappa
            .and_then(|m| m.get(k / 8))
            .is_some_and(|b| b >> (k % 8) & 1 == 1)
    };
    let mut usati: Vec<usize> = vec![1, 2, 3];
    usati.extend(&s.settori_indice);
    for v in &s.voci {
        usati.extend(v.elenco);
        usati.extend(&v.settori);
    }
    for n in usati {
        if libero(n) {
            problemi.push(format!("settore {n} in uso ma segnato libero"));
        }
    }
    for (i, v) in s.voci.iter().enumerate() {
        if v.cartella || v.elenco.is_none() {
            continue;
        }
        if let Err(e) = decomprimi(&s.flusso(dati, i)) {
            problemi.push(format!("{}: {e}", s.percorso(i)));
        }
    }
    Ok(problemi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn somme_di_controllo() {
        // CRC-16/ARC di «123456789» = 0xBB3D.
        assert_eq!(crc16(b"123456789"), 0xBB3D);
        assert_eq!(xor_parole(&[1, 0, 0, 0, 3, 0, 0, 0]), 2);
        assert_eq!(xor_parole(&[1, 2]), 0x0201);
    }
}
