//! Collaudo lungo (ignorato): XPWE mutati a caso; ciò che si importa si valida, si calcola, si
//! esporta e si reimporta con lo stesso totale. `ITER=20000 cargo test -p opencomputo-format-xpwe --test collaudo_xpwe -- --ignored --nocapture`
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    missing_docs
)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::validazione::valida_progetto;
use opencomputo_format_xpwe::{OpzioniImport, esporta, importa};

const FIXTURE: &str = include_str!("fixtures/computo_sintetico.xpwe");

struct Rng(u64);
impl Rng {
    fn n(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn sotto(&mut self, m: usize) -> usize {
        (self.n() % m.max(1) as u64) as usize
    }
}

const PEZZI: &[&str] = &[
    "<",
    ">",
    "</",
    "/>",
    "&",
    "&amp;",
    "&#0;",
    "&#xFFFFFF;",
    "&lt;",
    "]]>",
    "<![CDATA[x]]>",
    "\"",
    "'",
    "99999999999999999999999999999999",
    "-0",
    "1e9",
    "1,5",
    "1.2.3",
    "NaN",
    "-1",
    "0",
    "4294967296",
    "2*(",
    ")",
    "((((((((((",
    "1/0",
    "2^999",
    "sqrt(-1)",
    "€",
    "\u{0}",
    "\u{FFFF}",
    "😀",
    "\r\n",
    "\t",
    "<RGItem>",
    "</RGItem>",
    "<VCItem ID=\"1\">",
    "<IDVV>1</IDVV>",
    "<IDVV>5</IDVV>",
    "<IDEP>1</IDEP>",
    "<Quantita>1e400</Quantita>",
    "<Flags>-5</Flags>",
    "<Flags>4294967295</Flags>",
    "<PartiUguali>-0,5</PartiUguali>",
];

fn muta(r: &mut Rng, base: &str) -> String {
    let mut v: Vec<char> = base.chars().collect();
    for _ in 0..1 + r.sotto(3) {
        let k = r.sotto(10);
        match if k >= 4 { 4 } else { k } {
            0 => {
                let i = r.sotto(v.len());
                let p = PEZZI[r.sotto(PEZZI.len())];
                v.splice(i..i, p.chars());
            }
            1 => {
                let i = r.sotto(v.len());
                let n = r.sotto(40).min(v.len() - i);
                v.drain(i..i + n);
            }
            2 => {
                let i = r.sotto(v.len());
                v[i] = char::from_u32(32 + r.sotto(95) as u32).unwrap();
            }
            3 => {
                let n = r.sotto(v.len());
                v.truncate(n);
            }
            _ => {
                // Sostituisce il contenuto di un elemento numerico con un valore estremo.
                let s: String = v.iter().collect();
                let tag = [
                    "Quantita",
                    "PartiUguali",
                    "Lunghezza",
                    "Prezzo1",
                    "IDVV",
                    "IDEP",
                    "ID",
                    "IncMDO",
                    "Flags",
                    "IDSpCat",
                ][r.sotto(10)];
                let apri = format!("<{tag}>");
                if let Some(i) = s.find(&apri) {
                    if let Some(j) = s[i..].find("</") {
                        let nuovo = PEZZI[r.sotto(PEZZI.len())];
                        let s2 = format!("{}{}{}", &s[..i + apri.len()], nuovo, &s[i + j..]);
                        v = s2.chars().collect();
                    }
                }
            }
        }
        if v.is_empty() {
            v.push('x');
        }
    }
    v.into_iter().collect()
}

#[test]
#[ignore = "collaudo lungo: cargo test -- --ignored"]
fn esplora_xpwe() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut r = Rng(0xDEAD_BEEF_1234_5678);
    let iter: usize = std::env::var("ITER")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(3000);
    let mut panici: Vec<(String, String)> = Vec::new();
    let mut lenti = Vec::new();
    let mut importati = 0;
    for it in 0..iter {
        let testo = muta(&mut r, FIXTURE);
        let t = Instant::now();
        let esito = catch_unwind(AssertUnwindSafe(|| {
            let r = importa(
                testo.as_bytes(),
                &mut GeneratoreSequenziale::new(1),
                &OpzioniImport::default(),
            );
            if let Ok(r) = r {
                // Tutto ciò che si importa deve poter essere validato, calcolato ed esportato senza panico.
                let _ = valida_progetto(&r.progetto);
                let _ = r.progetto.calcola_computo(r.computo);
                if let Ok(e) = esporta(&r.progetto, r.computo) {
                    let r2 = importa(
                        e.xml.as_bytes(),
                        &mut GeneratoreSequenziale::new(1),
                        &OpzioniImport::default(),
                    )
                    .unwrap_or_else(|x| panic!("RIIMPORT FALLITO: {x}"));
                    let t1 = r.progetto.calcola_computo(r.computo).map(|x| x.totale);
                    let t2 = r2.progetto.calcola_computo(r2.computo).map(|x| x.totale);
                    if let (Ok(a), Ok(b)) = (&t1, &t2) {
                        if a != b {
                            panic!("TOTALE DIVERSO DOPO EXPORT: {a} → {b}");
                        }
                    }
                    if t1.is_ok() && t2.is_err() {
                        panic!("NON CALCOLABILE DOPO EXPORT: {:?}", t2.err());
                    }
                }
                let _ = opencomputo_core::schema::esporta_json(&r.progetto)
                    .map(|j| opencomputo_core::schema::importa_json(&j).unwrap());
                true
            } else {
                false
            }
        }));
        let dt = t.elapsed().as_millis();
        if dt > 1500 {
            lenti.push((it, dt));
        }
        match esito {
            Ok(true) => importati += 1,
            Ok(false) => {}
            Err(e) => {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                if panici.len() < 5 {
                    std::fs::write(format!("{}/xpwe_{it}.xpwe", cartella()), &testo).unwrap();
                }
                panici.push((format!("{it}"), msg));
            }
        }
    }
    let _ = std::panic::take_hook();
    let mut tipi: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, m) in &panici {
        *tipi
            .entry(m.lines().next().unwrap_or("").to_string())
            .or_default() += 1;
    }
    println!(
        "PANICI {} su {} (importati {}); LENTI {:?}",
        panici.len(),
        iter,
        importati,
        lenti.iter().take(5).collect::<Vec<_>>()
    );
    for (k, n) in tipi {
        println!("  {n} × {k}");
    }
    assert!(
        panici.is_empty() && lenti.is_empty(),
        "campioni in {}",
        cartella()
    );
}

/// Cartella dei campioni che fanno fallire il collaudo.
fn cartella() -> String {
    let d = std::env::temp_dir().join("opencomputo-collaudo");
    std::fs::create_dir_all(&d).unwrap();
    d.display().to_string()
}
