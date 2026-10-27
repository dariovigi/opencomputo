//! Importa un file XPWE e ne stampa rapporto ed export: `cargo run --example analizza_xpwe -- file.xpwe`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_format_xpwe::{OpzioniImport, esporta, importa};

fn main() {
    let percorso = std::env::args().nth(1).expect("percorso del file XPWE");
    let dati = std::fs::read(&percorso).unwrap();
    let r = importa(
        &dati,
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
    .unwrap();
    println!("codifica: {}", r.rapporto.codifica);
    println!("statistiche: {:?}", r.rapporto.statistiche);
    for a in &r.rapporto.anomalie {
        println!(
            "[{:?}] {}: {} ({})",
            a.gravita,
            a.codice,
            a.messaggio,
            a.posizione.as_deref().unwrap_or("")
        );
    }
    println!(
        "titolo: {:?}, regole: {:?}",
        r.progetto.metadati.titolo, r.progetto.regole
    );
    let e = esporta(&r.progetto, r.computo).unwrap();
    for a in &e.anomalie {
        println!("export [{:?}] {}: {}", a.gravita, a.codice, a.messaggio);
    }
    println!("----- export -----\n{}", e.xml);
}
