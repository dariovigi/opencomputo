//! Conto finale e rata di saldo dalla fixture v3 del core (valori verificati a mano in
//! `opencomputo-core/tests/conto_finale.rs`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use chrono::NaiveDate;
use opencomputo_core::schema::importa_json;
use opencomputo_io::documento::{Documento, StileRiga, Valore};
use opencomputo_io::report::{self, OpzioniReport};
use opencomputo_io::{csv, pdf, xlsx};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v3.json");

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

/// Importo della riga con l'etichetta indicata nella sezione con quel titolo.
fn voce(d: &Documento, sezione: &str, etichetta: &str) -> Decimal {
    let s = d
        .sezioni
        .iter()
        .find(|s| s.titolo == sezione)
        .unwrap_or_else(|| panic!("sezione «{sezione}» assente"));
    let r = s
        .tabella
        .righe
        .iter()
        .find(|r| r.celle[0].in_testo().starts_with(etichetta))
        .unwrap_or_else(|| panic!("riga «{etichetta}» assente"));
    numero(&r.celle[1]).unwrap()
}

#[test]
fn conto_finale_e_rata_di_saldo() {
    let p = importa_json(FIXTURE).unwrap();
    let d = report::conto_finale(&p, &opzioni()).unwrap();
    assert_eq!(d.titolo, "Conto finale dei lavori");
    let titoli: Vec<_> = d.sezioni.iter().map(|s| s.titolo.as_str()).collect();
    assert_eq!(
        titoli,
        [
            "Stato finale",
            "Stati di avanzamento e certificati di pagamento",
            "Conto finale",
            "Certificato di pagamento della rata di saldo",
            "Lavori dello stato finale non compresi nei SAL",
            "Penali della rata di saldo",
            "Firma e collaudo",
        ]
    );
    assert_eq!(voce(&d, "Stato finale", "Lavori a tutto"), dec!(12314.10));
    assert_eq!(
        voce(&d, "Stato finale", "— di cui lavori non"),
        dec!(1282.50)
    );
    assert_eq!(voce(&d, "Stato finale", "IMPORTO NETTO"), dec!(11974.84));
    assert_eq!(voce(&d, "Conto finale", "a dedurre"), dec!(10798.39));
    assert_eq!(voce(&d, "Conto finale", "Credito residuo"), dec!(1176.45));
    assert_eq!(voce(&d, "Conto finale", "— di cui ritenute"), dec!(59.87));
    let rata = "Certificato di pagamento della rata di saldo";
    assert_eq!(voce(&d, rata, "a dedurre: anticipazione"), dec!(429.47));
    assert_eq!(voce(&d, rata, "a dedurre: penali"), dec!(52.00));
    assert_eq!(voce(&d, rata, "Imponibile"), dec!(694.98));
    assert_eq!(voce(&d, rata, "IVA 10 %"), dec!(69.50));
    assert_eq!(voce(&d, rata, "TOTALE DELLA RATA"), dec!(764.48));

    // Certificati dei SAL con la riga dei totali.
    let t = &d.sezioni[1].tabella;
    let totale = t
        .righe
        .iter()
        .find(|r| r.stile == StileRiga::Totale)
        .unwrap();
    assert_eq!(numero(&totale.celle[3]), Some(dec!(10798.39)));
    assert_eq!(numero(&totale.celle[6]), Some(dec!(8608.36)));
    assert_eq!(numero(&totale.celle[8]), Some(dec!(9469.19)));

    let firma: Vec<String> = d.sezioni[6]
        .tabella
        .righe
        .iter()
        .map(|r| r.celle[1].in_testo())
        .collect();
    assert_eq!(
        firma,
        [
            "firmato il 30/06/2026",
            "Riserva n. 1 iscritta nel registro il 30/04/2026, confermata",
            "emesso il 20/07/2026, approvato il 31/07/2026",
        ]
    );
    assert_eq!(
        d.sezioni[6].tabella.righe[2].celle[0].in_testo(),
        "Certificato di regolare esecuzione"
    );

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
        fogli
            .iter()
            .any(|(_, testo)| testo.contains("TOTALE DELLA RATA DI SALDO;764,48"))
    );
}

#[test]
fn sommario_con_lo_stato_finale() {
    let p = importa_json(FIXTURE).unwrap();
    let d = report::sommario_registro(&p, &opzioni()).unwrap();
    let t = &d.sezioni[0].tabella;
    // 4 colonne fisse, 2 × (SAL 1, SAL 2, stato finale), quantità e importo totali.
    assert_eq!(t.colonne.len(), 12);
    assert_eq!(t.colonne[8].titolo, "Stato finale quantità");
    let totale = t
        .righe
        .iter()
        .find(|r| r.stile == StileRiga::Totale)
        .unwrap();
    assert_eq!(numero(&totale.celle[9]), Some(dec!(1282.50)));
    assert_eq!(numero(&totale.celle[11]), Some(dec!(13514.10)));
}

#[test]
fn senza_conto_finale_il_report_e_rifiutato() {
    let v2 = importa_json(include_str!(
        "../../opencomputo-core/tests/fixtures/progetto_esempio_v2.json"
    ))
    .unwrap();
    assert!(report::conto_finale(&v2, &opzioni()).is_err());
}
