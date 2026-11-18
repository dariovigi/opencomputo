//! Genera tutti i report di un progetto in PDF, XLSX e CSV, più le stampe del computo e
//! dell'elenco prezzi nel modello tradizionale.
//!
//! `cargo run -p opencomputo-io --example report -- progetto.json cartella_di_uscita [aaaa-mm-gg]`
//!
//! Il progetto è un file JSON di OpenComputo (vedi `opencomputo_core::schema`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use jiff::civil::Date;
use opencomputo_core::schema::importa_json;
use opencomputo_io::documento::Documento;
use opencomputo_io::report::{self, OpzioniReport};
use opencomputo_io::tradizionale::{self, OpzioniTradizionale};
use opencomputo_io::{csv, pdf, xlsx};

fn scrivi(cartella: &Path, nome: &str, doc: &Documento, data: Option<Date>) {
    let p = pdf::documento(doc, &pdf::OpzioniPdf::default());
    std::fs::write(cartella.join(format!("{nome}.pdf")), &p.byte).unwrap();
    let x = xlsx::documento(doc, &xlsx::OpzioniXlsx { data }).unwrap();
    std::fs::write(cartella.join(format!("{nome}.xlsx")), x).unwrap();
    for (i, (titolo, testo)) in csv::documento(doc, &csv::OpzioniCsv::default())
        .into_iter()
        .enumerate()
    {
        let file: String = titolo
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        std::fs::write(cartella.join(format!("{nome}_{}_{file}.csv", i + 1)), testo).unwrap();
    }
    println!(
        "{nome}: {} pagine PDF, {} sezioni, {} caratteri sostituiti",
        p.pagine,
        doc.sezioni.len(),
        p.caratteri_sostituiti
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let ingresso = args.next().expect("file JSON del progetto");
    let uscita = args.next().expect("cartella di uscita");
    let data = args
        .next()
        .map(|d| Date::strptime("%Y-%m-%d", &d).expect("data aaaa-mm-gg"));
    let progetto = importa_json(&std::fs::read_to_string(ingresso).unwrap()).unwrap();
    let cartella = Path::new(&uscita);
    std::fs::create_dir_all(cartella).unwrap();
    let o = OpzioniReport { data_stampa: data };

    scrivi(
        cartella,
        "elenco_prezzi",
        &report::elenco_prezzi(&progetto, &o),
        data,
    );
    let t = OpzioniTradizionale {
        data_stampa: data,
        ..OpzioniTradizionale::default()
    };
    let r = tradizionale::elenco_prezzi(&progetto, &t);
    std::fs::write(cartella.join("elenco_prezzi_tradizionale.pdf"), &r.byte).unwrap();
    println!("elenco_prezzi_tradizionale: {} pagine PDF", r.pagine);
    for (i, c) in progetto.computi.iter().enumerate() {
        scrivi(
            cartella,
            &format!("computo_{}", i + 1),
            &report::computo(&progetto, c.id, &o).unwrap(),
            data,
        );
        let r = tradizionale::computo(&progetto, c.id, &t).unwrap();
        let nome = format!("computo_{}_tradizionale.pdf", i + 1);
        std::fs::write(cartella.join(&nome), &r.byte).unwrap();
        println!("{nome}: {} pagine PDF", r.pagine);
    }
    if !progetto.contabilita.libretto().registrazioni.is_empty() {
        scrivi(
            cartella,
            "libretto",
            &report::libretto(&progetto, &o).unwrap(),
            data,
        );
        scrivi(
            cartella,
            "registro",
            &report::registro(&progetto, &o).unwrap(),
            data,
        );
    }
    for s in progetto.contabilita.sal() {
        scrivi(
            cartella,
            &format!("sal_{}", s.numero),
            &report::stato_avanzamento(&progetto, s.id, &o).unwrap(),
            data,
        );
    }
}
