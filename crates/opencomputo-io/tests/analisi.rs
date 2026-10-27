//! Analisi prezzi, fabbisogno e nuovi prezzi dalla fixture v4 del core (valori verificati
//! a mano in `opencomputo-core/tests/nuovi_prezzi.rs` e `common`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use chrono::NaiveDate;
use opencomputo_core::schema::importa_json;
use opencomputo_io::documento::{StileRiga, Valore};
use opencomputo_io::report::OpzioniReport;
use opencomputo_io::report_analisi;
use opencomputo_io::{csv, pdf, xlsx};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v4.json");

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

#[test]
fn analisi_completa_e_sintetica() {
    let p = importa_json(FIXTURE).unwrap();
    let d = report_analisi::analisi_prezzi(&p, None, false, &opzioni()).unwrap();
    assert_eq!(d.sezioni.len(), 1);
    let s = &d.sezioni[0];
    assert_eq!(s.titolo, "NP.1 NP.01 — Intonaco premiscelato");
    let righe: Vec<(String, Option<Decimal>)> = s
        .tabella
        .righe
        .iter()
        .map(|r| (r.celle[1].in_testo(), numero(&r.celle[5])))
        .collect();
    assert_eq!(
        righe,
        vec![
            ("Manodopera".into(), None),
            ("Operaio comune".into(), Some(dec!(8.00))),
            ("Materiali".into(), None),
            ("Intonaco premiscelato in sacchi".into(), Some(dec!(6.00))),
            ("Costo diretto".into(), Some(dec!(14.00))),
            ("Spese generali 15,00 %".into(), Some(dec!(2.10))),
            (
                "Utile d'impresa 10,00 % (su costo e spese generali)".into(),
                Some(dec!(1.61))
            ),
            ("Totale per 1 m²".into(), Some(dec!(17.71))),
            ("PREZZO UNITARIO (m²)".into(), Some(dec!(17.71))),
        ]
    );
    assert_eq!(s.tabella.righe.last().unwrap().stile, StileRiga::Totale);
    assert_eq!(
        s.note,
        vec!["Incidenze sul prezzo: manodopera 45,17 %, materiali 33,88 %.".to_string()]
    );

    let d = report_analisi::analisi_prezzi(&p, Some("NP.01"), true, &opzioni()).unwrap();
    let r = &d.sezioni[0].tabella.righe[0];
    assert_eq!(r.celle[0].in_testo(), "NP.1 NP.01");
    assert_eq!(numero(&r.celle[6]), Some(dec!(17.71)));
    assert_eq!(numero(&r.celle[7]), Some(dec!(45.17)));
    assert!(report_analisi::analisi_prezzi(&p, Some("A.01.001"), true, &opzioni()).is_err());

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
    assert!(!csv::documento(&d, &csv::OpzioniCsv::default()).is_empty());
}

#[test]
fn elenco_e_verbale_dei_nuovi_prezzi() {
    let p = importa_json(FIXTURE).unwrap();
    let d = report_analisi::nuovi_prezzi(&p, None, &opzioni()).unwrap();
    let t = &d.sezioni[0].tabella;
    let righe: Vec<Vec<String>> = t
        .righe
        .iter()
        .map(|r| r.celle.iter().map(Valore::in_testo).collect())
        .collect();
    assert_eq!(righe[0][0], "NP.1");
    assert_eq!(righe[0][5], "da analisi dei prezzi elementari");
    assert_eq!(righe[0][6], "no");
    assert_eq!(righe[0][7], "n. 1 approvato");
    assert_eq!(righe[1][6], "sì");

    let v = report_analisi::nuovi_prezzi(&p, Some(1), &opzioni()).unwrap();
    assert_eq!(v.titolo, "Verbale di concordamento nuovi prezzi n. 1");
    let firme: Vec<String> = v.sezioni[1]
        .tabella
        .righe
        .iter()
        .map(|r| r.celle[2].in_testo())
        .collect();
    assert_eq!(firme, ["10/04/2026", "12/04/2026", "15/04/2026"]);
    assert_eq!(
        v.sezioni[1].note,
        vec!["Atto di approvazione: Determina del RUP n. 12".to_string()]
    );
    assert_eq!(v.sezioni.len(), 3, "con l'analisi del NP.1");
    assert!(report_analisi::nuovi_prezzi(&p, Some(5), &opzioni()).is_err());
}

#[test]
fn fabbisogno_senza_analisi_nel_computo() {
    let p = importa_json(FIXTURE).unwrap();
    let id = p.computi[0].id;
    let d = report_analisi::fabbisogno(&p, id, &opzioni()).unwrap();
    let s = &d.sezioni[0];
    assert_eq!(s.tabella.righe.len(), 1, "solo il totale");
    assert!(s.note[1].contains("A.01.001, B.02.010, C.03.020, S.01.001"));
}

#[test]
fn sal_con_nuovi_prezzi_esclusi_dal_ribasso() {
    let p = importa_json(FIXTURE).unwrap();
    let sal2 = p.contabilita.sal()[1].id;
    let d = opencomputo_io::report::stato_avanzamento(&p, sal2, &opzioni()).unwrap();
    let riga = d
        .sezioni
        .iter()
        .flat_map(|s| s.tabella.righe.iter())
        .find(|r| r.celle[0].in_testo() == "di cui nuovi prezzi esclusi dal ribasso")
        .unwrap();
    assert_eq!(numero(&riga.celle[1]), Some(dec!(177.10)));
}

#[test]
fn variabili_e_oneri_della_sicurezza_nella_stampa() {
    let p = importa_json(include_str!(
        "../../opencomputo-core/tests/fixtures/progetto_esempio_v5.json"
    ))
    .unwrap();
    let d = report_analisi::analisi_prezzi(&p, None, false, &opzioni()).unwrap();
    let s = &d.sezioni[0];
    assert_eq!(
        s.note[0],
        "Variabili: sacchi (kg di premiscelato per m²) = 12."
    );
    let oneri = s
        .tabella
        .righe
        .iter()
        .find(|r| {
            r.celle[1]
                .in_testo()
                .starts_with("— di cui oneri aziendali")
        })
        .unwrap();
    assert_eq!(
        oneri.celle[1].in_testo(),
        "— di cui oneri aziendali della sicurezza (20,00 % delle spese generali)"
    );
    assert_eq!(numero(&oneri.celle[5]), Some(dec!(0.42)));
}
