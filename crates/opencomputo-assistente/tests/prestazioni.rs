//! Prezzario sintetico di 30 000 voci in 300 capitoli: costruzione degli indici e
//! ricerca in tempi lineari. I limiti sono larghi (build di debug): servono a fermare
//! regressioni quadratiche, che qui impiegherebbero minuti.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::{Duration, Instant};

use opencomputo_assistente::candidati::cerca_ibrida;
use opencomputo_assistente::ricerca::{FiltroVoci, IndiceTestuale};
use opencomputo_assistente::vettori::{IndiceVettoriale, SpazioEmbedding, da_indicizzare};
use opencomputo_core::id::{GeneratoreId, GeneratoreSequenziale};
use opencomputo_core::numeri::Prezzo;
use opencomputo_core::prezzario::{CapitoloPrezzario, Prezzario, VocePrezzario};
use rust_decimal::Decimal;

const LAVORAZIONI: [&str; 10] = [
    "scavo",
    "massetto",
    "pavimento",
    "intonaco",
    "tinteggiatura",
    "muratura",
    "calcestruzzo",
    "demolizione",
    "impermeabilizzazione",
    "serramento",
];
const MATERIALI: [&str; 6] = [
    "gres",
    "laterizio",
    "cemento",
    "acciaio",
    "legno",
    "ceramica",
];

fn prezzario_grande() -> Prezzario {
    let mut g = GeneratoreSequenziale::default();
    let mut p = Prezzario::new(g.nuovo(), "sintetico");
    let mut capitoli = Vec::new();
    for c in 0..300 {
        let id = g.nuovo();
        p.aggiungi_capitolo(CapitoloPrezzario {
            id,
            padre: None,
            codice: format!("K{c:03}"),
            descrizione: format!("Capitolo {} {}", LAVORAZIONI[c % 10], MATERIALI[c % 6]),
        })
        .unwrap();
        capitoli.push(id);
    }
    for n in 0..30_000usize {
        let mut v = VocePrezzario::new(
            g.nuovo(),
            format!("V{n:05}"),
            format!(
                "{} in {} spessore {} cm, tipo {}",
                LAVORAZIONI[n % 10],
                MATERIALI[(n / 10) % 6],
                n % 40,
                n
            ),
            "m²",
            Prezzo::new(Decimal::from(n % 500 + 1)),
        );
        v.capitolo = Some(capitoli[n % 300]);
        p.voci.push(v);
    }
    p
}

#[test]
fn trentamila_voci() {
    let p = prezzario_grande();
    let limite = Duration::from_secs(60);

    let t = Instant::now();
    let it = IndiceTestuale::new(&p);
    let costruzione = t.elapsed();
    assert_eq!(it.len(), 30_000);

    let t = Instant::now();
    let da = da_indicizzare(&p, 8_000);
    let testi = t.elapsed();
    assert_eq!(da.len(), 30_000);

    // Indice vettoriale con vettori fittizi di 64 dimensioni.
    let spazio = SpazioEmbedding::new("finto", 64);
    let mut iv = IndiceVettoriale::new(spazio, "");
    for (i, v) in da.iter().enumerate() {
        let mut x = vec![0.0f32; 64];
        x[i % 64] = 1.0;
        iv.registra(v, x).unwrap();
    }

    let t = Instant::now();
    let mut domanda = vec![0.0f32; 64];
    domanda[3] = 1.0;
    let mut trovati = Vec::new();
    for _ in 0..10 {
        trovati = cerca_ibrida(
            &it,
            Some((&iv, &domanda)),
            "massetto cemento",
            8,
            &FiltroVoci::default(),
        );
    }
    let ricerca = t.elapsed();
    assert_eq!(trovati.len(), 8);

    eprintln!("indice testuale {costruzione:?}, testi {testi:?}, 10 ricerche ibride {ricerca:?}");
    assert!(costruzione < limite, "indice testuale: {costruzione:?}");
    assert!(testi < limite, "testi da indicizzare: {testi:?}");
    assert!(ricerca < limite, "ricerca: {ricerca:?}");
}
