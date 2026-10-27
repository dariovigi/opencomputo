//! Prova di carico su prezzari DCF reali, che **non** sono nel repository:
//! `OPENCOMPUTO_DCF_REALI=/cartella cargo test -p opencomputo-io --release --test prezzari_reali -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::time::Instant;

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_dcf_parser::{OpzioniImport, importa};
use opencomputo_io::report::{self, OpzioniReport};
use opencomputo_io::{csv, pdf, xlsx};

#[test]
#[ignore = "richiede DCF reali indicati da OPENCOMPUTO_DCF_REALI"]
fn elenco_prezzi_di_un_prezzario_regionale() {
    let Ok(cartella) = std::env::var("OPENCOMPUTO_DCF_REALI") else {
        eprintln!("OPENCOMPUTO_DCF_REALI non impostata: nessuna verifica");
        return;
    };
    let uscita = std::env::var("OPENCOMPUTO_USCITA").ok();
    let mut file: Vec<_> = std::fs::read_dir(&cartella)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dcf")))
        .collect();
    file.sort();
    for percorso in file {
        let Ok(r) = importa(
            &std::fs::read(&percorso).unwrap(),
            &mut GeneratoreSequenziale::default(),
            &OpzioniImport::default(),
        ) else {
            continue;
        };
        let nome = percorso.file_stem().unwrap().to_string_lossy().into_owned();
        let voci = r.progetto.elenco_prezzi.voci.len();
        let t0 = Instant::now();
        let doc = report::elenco_prezzi(&r.progetto, &OpzioniReport::default());
        let righe = doc.sezioni[0].tabella.righe.len();
        let p = pdf::documento(&doc, &pdf::OpzioniPdf::default());
        let t_pdf = t0.elapsed();
        let x = xlsx::documento(&doc, &xlsx::OpzioniXlsx::default()).unwrap();
        let c = csv::documento(&doc, &csv::OpzioniCsv::default());
        println!(
            "{nome}: {voci} voci, {righe} righe, PDF {} pagine {} MB in {:?}, {} caratteri sostituiti; XLSX {} MB; CSV {} MB; totale {:?}",
            p.pagine,
            p.byte.len() / 1_000_000,
            t_pdf,
            p.caratteri_sostituiti,
            x.len() / 1_000_000,
            c[0].1.len() / 1_000_000,
            t0.elapsed()
        );
        // Ogni voce compare una volta: righe = voci + titoli dei capitoli.
        let gruppi = doc.sezioni[0]
            .tabella
            .righe
            .iter()
            .filter(|r| r.stile == opencomputo_io::documento::StileRiga::Gruppo)
            .count();
        assert_eq!(righe, voci + gruppi);
        if let Some(u) = &uscita {
            std::fs::create_dir_all(u).unwrap();
            std::fs::write(format!("{u}/{nome}.pdf"), &p.byte).unwrap();
            std::fs::write(format!("{u}/{nome}.xlsx"), &x).unwrap();
            std::fs::write(format!("{u}/{nome}.csv"), &c[0].1).unwrap();
        }
    }
}
