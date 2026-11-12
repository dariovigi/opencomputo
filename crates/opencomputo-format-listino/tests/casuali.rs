//! Listini danneggiati a caso (sequenza pseudo-casuale fissa, quindi ripetibile): byte
//! cambiati, tolti, inseriti, righe duplicate, file troncati. L'importazione non deve mai
//! andare in errore irreversibile (panic) né produrre un prezzario incoerente.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_format_listino::{OpzioniListino, importa};

const CSV: &[u8] = include_bytes!("fixtures/listino_esempio.csv");
const JSON: &[u8] = include_bytes!("fixtures/listino_esempio.json");

/// Generatore xorshift: stessa sequenza a ogni esecuzione.
struct Caso(u64);

impl Caso {
    fn prossimo(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn fino_a(&mut self, n: usize) -> usize {
        (self.prossimo() % n.max(1) as u64) as usize
    }
}

const INSERTI: &[&[u8]] = &[
    b";",
    b"\"",
    b"\n",
    b"\r\n",
    b",",
    b"\t",
    b"\xff\xfe",
    b"\xef\xbb\xbf",
    b"1.234,56",
    b"-1",
    b"1e99",
    b"99999999999999999999999999999999999",
    b"{",
    b"[",
    b"}",
    b"null",
    b"\0",
    b"\xc3",
    b"m\xc2\xb2",
    b"\"\"\"",
    b";;;;;;;;;;;;;;;;;;;;",
    b"Codice;Codice",
    b"0,0000000000000000000000000001",
];

fn danneggia(c: &mut Caso, originale: &[u8]) -> Vec<u8> {
    let mut b = originale.to_vec();
    for _ in 0..=c.fino_a(6) {
        if b.is_empty() {
            break;
        }
        let i = c.fino_a(b.len());
        match c.fino_a(7) {
            0 => b[i] = c.prossimo() as u8,
            1 => {
                b.remove(i);
            }
            2 => {
                let ins = INSERTI[c.fino_a(INSERTI.len())];
                b.splice(i..i, ins.iter().copied());
            }
            3 => b.truncate(i),
            4 => {
                // Duplica una riga.
                let inizio = b[..i]
                    .iter()
                    .rposition(|x| *x == b'\n')
                    .map_or(0, |p| p + 1);
                let fine = b[i..]
                    .iter()
                    .position(|x| *x == b'\n')
                    .map_or(b.len(), |p| i + p + 1);
                let riga = b[inizio..fine].to_vec();
                b.splice(fine..fine, riga);
            }
            5 => {
                let j = c.fino_a(b.len());
                b.swap(i, j);
            }
            _ => {
                let fine = (i + c.fino_a(40)).min(b.len());
                b.drain(i..fine);
            }
        }
    }
    b
}

fn controlla(byte: &[u8]) -> bool {
    let mut g = GeneratoreSequenziale::default();
    let Ok(r) = importa(byte, &mut g, &OpzioniListino::default()) else {
        return false;
    };
    let p = &r.prezzario;
    assert_eq!(r.rapporto.voci_importate, p.voci.len());
    assert_eq!(r.rapporto.capitoli, p.capitoli.len());
    let codici: BTreeSet<&str> = p.voci.iter().map(|v| v.codice.as_str()).collect();
    assert_eq!(codici.len(), p.voci.len(), "codici duplicati");
    let ids: BTreeSet<_> = p.voci.iter().map(|v| v.id).collect();
    assert_eq!(ids.len(), p.voci.len(), "identificativi duplicati");
    let capitoli: BTreeSet<_> = p.capitoli.iter().map(|c| c.id).collect();
    for v in &p.voci {
        assert!(!v.codice.trim().is_empty(), "codice vuoto");
        assert!(
            v.capitolo.is_none_or(|c| capitoli.contains(&c)),
            "capitolo inesistente"
        );
    }
    for c in &p.capitoli {
        assert!(
            c.padre.is_none_or(|x| capitoli.contains(&x) && x != c.id),
            "padre non valido"
        );
    }
    // Il prezzario importato si serializza e si rilegge uguale.
    let json = serde_json::to_string(p).unwrap();
    let riletto: opencomputo_core::prezzario::Prezzario = serde_json::from_str(&json).unwrap();
    assert_eq!(&riletto, p);
    true
}

#[test]
fn listini_danneggiati_non_fanno_mai_danni() {
    let mut c = Caso(0x9E37_79B9_7F4A_7C15);
    let (mut letti, mut rifiutati) = (0, 0);
    for n in 0..4000 {
        let originale = if n % 2 == 0 { CSV } else { JSON };
        let b = danneggia(&mut c, originale);
        if controlla(&b) {
            letti += 1;
        } else {
            rifiutati += 1;
        }
    }
    // Entrambi gli esiti capitano davvero: il test esercita sia i percorsi di recupero
    // sia quelli di rifiuto.
    assert!(
        letti > 500 && rifiutati > 100,
        "letti {letti}, rifiutati {rifiutati}"
    );
}

#[test]
fn byte_qualsiasi() {
    let mut c = Caso(42);
    for _ in 0..2000 {
        let n = c.fino_a(300);
        let b: Vec<u8> = (0..n).map(|_| c.prossimo() as u8).collect();
        controlla(&b);
    }
    for b in [
        &b""[..],
        b"\n",
        b";",
        b"\"",
        b"{",
        b"[",
        b"[]",
        b"{}",
        b"\xef\xbb\xbf",
        b"Codice;Descrizione;U.M.;Prezzo",
        b"\0\0\0\0",
    ] {
        controlla(b);
    }
}
