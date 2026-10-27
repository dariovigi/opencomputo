//! Certificato di pagamento e sommario del registro dalla fixture v2 del core
//! (valori verificati a mano in `opencomputo-core/tests/certificato.rs`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use chrono::NaiveDate;
use opencomputo_core::schema::importa_json;
use opencomputo_io::documento::{StileRiga, Valore};
use opencomputo_io::report::{self, OpzioniReport};
use opencomputo_io::{csv, pdf, xlsx};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v2.json");

fn opzioni() -> OpzioniReport {
    OpzioniReport {
        data_stampa: NaiveDate::from_ymd_opt(2026, 10, 21),
    }
}

fn numero(v: &Valore) -> Option<Decimal> {
    match v {
        Valore::Numero { valore, .. } => Some(*valore),
        _ => None,
    }
}

/// Importo della riga con l'etichetta indicata nella prima tabella.
fn voce(d: &opencomputo_io::documento::Documento, etichetta: &str) -> Decimal {
    let r = d.sezioni[0]
        .tabella
        .righe
        .iter()
        .find(|r| r.celle[0].in_testo().starts_with(etichetta))
        .unwrap_or_else(|| panic!("riga «{etichetta}» assente"));
    numero(&r.celle[1]).unwrap()
}

#[test]
fn certificato_di_pagamento() {
    let p = importa_json(FIXTURE).unwrap();
    let sal2 = p.contabilita.sal()[1].id;
    let d = report::certificato_pagamento(&p, sal2, &opzioni()).unwrap();
    assert_eq!(d.titolo, "Certificato di pagamento n. 2");
    assert_eq!(voce(&d, "Importo del presente stato"), dec!(2679.79));
    assert_eq!(
        voce(&d, "a dedurre: recupero dell'anticipazione"),
        dec!(538.65)
    );
    assert_eq!(voce(&d, "a dedurre: penali"), dec!(150.00));
    assert_eq!(voce(&d, "Imponibile"), dec!(1991.14));
    assert_eq!(voce(&d, "IVA 10 %"), dec!(199.11));
    assert_eq!(voce(&d, "TOTALE DEL CERTIFICATO"), dec!(2190.25));
    assert!(
        d.sezioni[0].note.is_empty(),
        "certificato congelato, IVA indicata"
    );
    let titoli: Vec<_> = d.sezioni.iter().map(|s| s.titolo.as_str()).collect();
    assert_eq!(titoli, ["Certificato", "Anticipazione", "Penali applicate"]);
    let penale = &d.sezioni[2].tabella.righe[0];
    assert_eq!(
        penale.celle[2].in_testo(),
        "Ritardo sulla scadenza intermedia (3 giorni × 0,5 ‰)"
    );
    // I tre formati.
    assert!(
        pdf::documento(&d, &pdf::OpzioniPdf::default())
            .byte
            .starts_with(b"%PDF")
    );
    assert!(
        xlsx::documento(&d, &xlsx::OpzioniXlsx { data: None })
            .unwrap()
            .starts_with(b"PK")
    );
    let fogli = csv::documento(&d, &csv::OpzioniCsv::default());
    assert!(
        fogli[0]
            .1
            .contains("TOTALE DEL CERTIFICATO DI PAGAMENTO;2190,25")
    );
}

#[test]
fn certificato_ricalcolato_per_i_sal_della_versione_1() {
    let v1 = importa_json(include_str!(
        "../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
    ))
    .unwrap();
    let d = report::certificato_pagamento(&v1, v1.contabilita.sal()[0].id, &opzioni()).unwrap();
    assert_eq!(voce(&d, "TOTALE DEL CERTIFICATO"), dec!(8118.60));
    let note = d.sezioni[0].note.join(" ");
    assert!(note.contains("IVA non indicata"));
    assert!(note.contains("versione precedente"));
    assert_eq!(d.sezioni.len(), 1, "niente anticipazione né penali");
}

#[test]
fn sommario_del_registro() {
    let p = importa_json(FIXTURE).unwrap();
    let d = report::sommario_registro(&p, &opzioni()).unwrap();
    let t = &d.sezioni[0].tabella;
    // Tariffa, designazione, U.M., prezzo, 2 colonne per SAL × 2, quantità e importo.
    assert_eq!(t.colonne.len(), 10);
    assert_eq!(t.righe.len(), 5);
    let muratura = &t.righe[2];
    assert_eq!(muratura.celle[0].in_testo(), "C.03.020");
    assert!(
        matches!(muratura.celle[4], Valore::Vuoto),
        "nessuna quantità nel SAL 1"
    );
    assert_eq!(numero(&muratura.celle[7]), Some(dec!(3078.00)));
    let totale = t
        .righe
        .iter()
        .find(|r| r.stile == StileRiga::Totale)
        .unwrap();
    assert_eq!(numero(&totale.celle[5]), Some(dec!(9153.60)));
    assert_eq!(numero(&totale.celle[9]), Some(dec!(12231.60)));
    assert!(d.orizzontale);
}
