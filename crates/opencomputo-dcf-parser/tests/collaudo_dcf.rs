//! Collaudo lungo (ignorato): migliaia di DCF validi mutati a caso; nessun panico, nessun blocco.
//! `ITER=20000 cargo test -p opencomputo-dcf-parser --test collaudo_dcf -- --ignored --nocapture`
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    missing_docs
)]

use opencomputo_core::data::OperazioniData;
use opencomputo_core::data::data_amg;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::schema::importa_json;
use opencomputo_dcf_parser::esporta::{OpzioniExport, esporta};
use opencomputo_dcf_parser::{OpzioniImport, contenitore, importa, struttura};

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v6.json");

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

fn muta(r: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    let k = 1 + r.sotto(8);
    for _ in 0..k {
        match r.sotto(7) {
            0 => {
                let i = r.sotto(v.len());
                v[i] = r.n() as u8;
            }
            1 => {
                let i = r.sotto(v.len());
                v[i] ^= 1 << r.sotto(8);
            }
            2 => {
                let i = r.sotto(v.len().saturating_sub(4));
                let x = [0xFFu8, 0xFF, 0xFF, 0x7F];
                v[i..i + 4].copy_from_slice(&x);
            }
            3 => {
                let i = r.sotto(v.len().saturating_sub(4));
                v[i..i + 4].copy_from_slice(&0u32.to_le_bytes());
            }
            4 => {
                let n = r.sotto(v.len());
                v.truncate(n.max(1));
            }
            5 => {
                let i = r.sotto(v.len().saturating_sub(4));
                let x = (r.n() as u32 % 70000).to_le_bytes();
                v[i..i + 4].copy_from_slice(&x);
            }
            _ => {
                let i = r.sotto(v.len());
                let j = r.sotto(v.len());
                let (a, b) = (i.min(j), i.max(j));
                let pezzo = v[a..b.min(a + 512)].to_vec();
                let p = r.sotto(v.len());
                v.splice(p..p, pezzo);
            }
        }
    }
    v
}

#[test]
#[ignore = "collaudo lungo: cargo test -- --ignored"]
fn esplora_dcf() {
    let p = importa_json(FIXTURE).unwrap();
    let o = OpzioniExport {
        data: data_amg(2026, 1, 1).unwrap().alle(0, 0, 0).unwrap(),
        versione_programma: "x".into(),
    };
    let base = esporta(&p, &o).unwrap().byte;
    std::panic::set_hook(Box::new(|_| {}));
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut panici: Vec<(String, usize, String)> = Vec::new();
    let mut lenti = Vec::new();
    let iter: usize = std::env::var("ITER")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(3000);
    for it in 0..iter {
        let v = muta(&mut r, &base);
        for (nome, f) in [
            (
                "contenitore::leggi",
                &(|d: &[u8]| {
                    let _ = contenitore::leggi(d);
                }) as &dyn Fn(&[u8]),
            ),
            ("importa", &|d: &[u8]| {
                let _ = importa(
                    d,
                    &mut GeneratoreSequenziale::new(1),
                    &OpzioniImport::default(),
                );
            }),
            ("struttura::leggi", &|d: &[u8]| {
                let _ = struttura::leggi(d);
            }),
            ("struttura::verifica", &|d: &[u8]| {
                let _ = struttura::verifica(d);
            }),
            ("struttura::flussi", &|d: &[u8]| {
                if let Ok(s) = struttura::leggi(d) {
                    for i in 0..s.voci.len() {
                        let f = s.flusso(d, i);
                        let _ = struttura::decomprimi(&f);
                    }
                }
            }),
        ] {
            let t = Instant::now();
            let esito = catch_unwind(AssertUnwindSafe(|| f(&v)));
            let dt = t.elapsed().as_millis();
            if dt > 1500 {
                lenti.push((nome, it, dt));
            }
            if let Err(e) = esito {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                if panici.iter().filter(|x| x.0 == nome).count() < 3 {
                    std::fs::write(
                        format!("{}/dcf_{nome}_{it}.dcf", cartella()).replace("::", "_"),
                        &v,
                    )
                    .unwrap();
                }
                panici.push((nome.to_string(), it, msg));
            }
        }
    }
    let _ = std::panic::take_hook();
    let mut tipi: std::collections::BTreeMap<String, usize> = Default::default();
    for (n, _, m) in &panici {
        *tipi
            .entry(format!("{n}: {}", m.lines().next().unwrap_or("")))
            .or_default() += 1;
    }
    println!(
        "PANICI {} su {} input; LENTI {:?}",
        panici.len(),
        iter,
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

#[test]
#[ignore = "collaudo lungo: cargo test -- --ignored"]
fn esplora_dcf_mirati() {
    use opencomputo_dcf_parser::scrittura;
    let f = scrittura::flusso(b"<a>ciao</a>").unwrap();
    // Numero di pezzi enorme nella testata.
    let mut g = f.clone();
    g[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| struttura::pezzi(&g).map(|v| v.len())));
    println!(
        "pezzi n=MAX: {:?} in {:?}",
        r.as_ref().map(|x| x.as_ref().map_err(|e| e.clone())),
        t.elapsed()
    );
    assert!(matches!(r, Ok(Err(_))));
    // Lunghezza decompressa dichiarata enorme.
    let mut g = f.clone();
    g[0x84..0x88].copy_from_slice(&u32::MAX.to_le_bytes());
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| {
        struttura::decomprimi(&g).map(|v| v.len())
    }));
    println!(
        "decomprimi d=MAX: {:?} in {:?}",
        r.as_ref().map(|x| x.as_ref().map_err(|e| e.clone())),
        t.elapsed()
    );
    assert!(matches!(r, Ok(Err(_))));
}

/// Cartella dei campioni che fanno fallire il collaudo.
fn cartella() -> String {
    let d = std::env::temp_dir().join("opencomputo-collaudo");
    std::fs::create_dir_all(&d).unwrap();
    d.display().to_string()
}
