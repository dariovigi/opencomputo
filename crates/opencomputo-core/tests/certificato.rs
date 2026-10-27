//! Test di integrazione: anticipazione, penali, IVA e certificati di pagamento.
//!
//! Valori verificati a mano sul progetto di prova (`progetto_con_contabilita`):
//! importo netto contrattuale 100.000,00 €, anticipazione 20 % = 20.000,00 € recuperata
//! al 20 % dell'importo netto progressivo, IVA 10 %, una penale di 3 giorni allo
//! 0,5 per mille = 150,00 € datata nel periodo del SAL 2.
//!
//! | | SAL 1 | SAL 2 |
//! |---|---:|---:|
//! | importo netto progressivo | 8.159,40 | 10.852,65 |
//! | importo del SAL | 8.118,60 | 2.679,79 |
//! | recupero (20 % del progressivo − già recuperato) | 1.631,88 | 2.170,53 − 1.631,88 = 538,65 |
//! | penali | 0,00 | 150,00 |
//! | imponibile | 6.486,72 | 1.991,14 |
//! | IVA 10 % | 648,67 (648,672) | 199,11 (199,114) |
//! | totale | 7.135,39 | 2.190,25 |
//! | anticipazione residua | 18.368,12 | 17.829,47 |

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::prelude::*;
use opencomputo_core::validazione::valida_progetto;
use rust_decimal_macros::dec;

fn imp(v: Decimal) -> Importo {
    Importo::new(v)
}

const PENALE: PenaleId = PenaleId::da_u128(7001);

fn progetto() -> Progetto {
    let mut p = progetto_con_contabilita();
    let c = p.contratto.as_mut().unwrap();
    c.importo_contrattuale = Some(imp(dec!(100000.00)));
    c.parametri.aliquota_iva = Some(Percentuale::new(dec!(10)));
    c.parametri.anticipazione = Some(Anticipazione {
        percentuale: Percentuale::new(dec!(20)),
        importo: imp(dec!(20000.00)),
        percentuale_recupero: Percentuale::new(dec!(20)),
        data_erogazione: Some(data(2026, 3, 2)),
    });
    p.contabilita
        .registra_penale(Penale {
            id: PENALE,
            numero: 1,
            data: data(2026, 4, 20),
            descrizione: "Ritardo sulla scadenza intermedia".into(),
            importo: imp(dec!(150.00)),
            giorni: Some(3),
            per_mille: Some(dec!(0.5)),
        })
        .unwrap();
    p
}

#[test]
fn certificati_verificati_a_mano() {
    let mut p = progetto();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    // In bozza il certificato è ricalcolato; la penale (20/04) non rientra nel SAL 1.
    let c1 = p.calcola_certificato(SAL1).unwrap();
    assert_eq!(c1.importo_sal, imp(dec!(8118.60)));
    assert_eq!(c1.recupero_anticipazione, imp(dec!(1631.88)));
    assert_eq!(c1.penali, imp(dec!(0)));
    assert_eq!(c1.imponibile, imp(dec!(6486.72)));
    assert_eq!(c1.iva, imp(dec!(648.67)));
    assert_eq!(c1.totale, imp(dec!(7135.39)));
    assert_eq!(c1.anticipazione_residua, imp(dec!(18368.12)));
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    let emesso = p.contabilita.sal()[0].certificato_emesso.clone().unwrap();
    assert_eq!(emesso, c1);
    assert!(p.contabilita.sal()[0].penali.is_empty());

    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    let c2 = p.calcola_certificato(SAL2).unwrap();
    assert_eq!(c2.importo_sal, imp(dec!(2679.79)));
    assert_eq!(c2.anticipazione_recuperata_prima, imp(dec!(1631.88)));
    assert_eq!(c2.recupero_anticipazione, imp(dec!(538.65)));
    assert_eq!(c2.penali, imp(dec!(150.00)));
    assert_eq!(c2.imponibile, imp(dec!(1991.14)));
    assert_eq!(c2.iva, imp(dec!(199.11)));
    assert_eq!(c2.totale, imp(dec!(2190.25)));
    assert_eq!(c2.anticipazione_residua, imp(dec!(17829.47)));
    assert_eq!(p.contabilita.sal()[1].penali, vec![PENALE]);
    assert!(
        !valida_progetto(&p).ha_errori(),
        "{:#?}",
        valida_progetto(&p)
    );
}

#[test]
fn recupero_sospeso_e_residuo() {
    let mut p = progetto();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.contabilita
        .imposta_recupero_anticipazione(SAL1, Some(true), None)
        .unwrap();
    let c1 = p.calcola_certificato(SAL1).unwrap();
    assert!(c1.recupero_sospeso);
    assert_eq!(c1.recupero_anticipazione, imp(dec!(0)));
    assert_eq!(c1.imponibile, imp(dec!(8118.60)));
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    // Dopo la sospensione il recupero si riallinea alla proporzione: 2.170,53 − 0.
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    assert_eq!(
        p.calcola_certificato(SAL2).unwrap().recupero_anticipazione,
        imp(dec!(2170.53))
    );
    // Ultimo SAL: recupero di tutto il residuo.
    p.contabilita
        .imposta_recupero_anticipazione(SAL2, None, Some(true))
        .unwrap();
    let c2 = p.calcola_certificato(SAL2).unwrap();
    assert_eq!(c2.recupero_anticipazione, imp(dec!(20000.00)));
    assert_eq!(c2.anticipazione_residua, imp(dec!(0)));
    // 2.679,79 − 20.000,00 − 150,00 < 0: certificato negativo segnalato.
    assert!(c2.imponibile.is_negativo());
    assert!(
        valida_progetto(&p)
            .problemi
            .iter()
            .any(|x| x.codice == "contabilita.certificato_negativo")
    );
    // Un SAL emesso non accetta più opzioni.
    let e = p
        .contabilita
        .imposta_recupero_anticipazione(SAL1, Some(false), None)
        .unwrap_err();
    assert!(e.to_string().contains("emesso"));
}

#[test]
fn certificati_congelati_e_penali_bloccate() {
    let mut p = progetto();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    // La penale applicata in un SAL emesso non si rimuove.
    let e = p.contabilita.rimuovi_penale(PENALE).unwrap_err();
    assert!(e.to_string().contains("SAL n. 2"));
    // Cambiare l'IVA o il recupero dopo l'emissione altera i certificati: rilevato.
    p.contratto.as_mut().unwrap().parametri.aliquota_iva = Some(Percentuale::new(dec!(22)));
    let codici: Vec<_> = valida_progetto(&p)
        .problemi
        .into_iter()
        .filter(|x| x.codice == "contabilita.certificato_alterato")
        .collect();
    assert_eq!(codici.len(), 2);
    // Il certificato emesso resta quello congelato.
    assert_eq!(p.calcola_certificato(SAL2).unwrap().iva, imp(dec!(199.11)));
}

#[test]
fn limiti_normativi_segnalati() {
    let mut p = progetto();
    // 10 % di 100.000,00 = 10.000,00: con 10.000,01 di penali si supera il limite.
    p.contabilita
        .registra_penale(Penale {
            id: PenaleId::da_u128(7002),
            numero: 2,
            data: data(2026, 5, 1),
            descrizione: "Ritardo sull'ultimazione".into(),
            importo: imp(dec!(9850.01)),
            giorni: Some(60),
            per_mille: Some(dec!(2)),
        })
        .unwrap();
    p.contratto
        .as_mut()
        .unwrap()
        .parametri
        .anticipazione
        .as_mut()
        .unwrap()
        .importo = imp(dec!(30000.01));
    let r = valida_progetto(&p);
    let codici: Vec<_> = r.problemi.iter().map(|x| x.codice.as_str()).collect();
    assert!(
        codici.contains(&"contabilita.penali_oltre_limite"),
        "{codici:?}"
    );
    assert!(codici.contains(&"contabilita.penale_fuori_misura"));
    assert!(codici.contains(&"contabilita.anticipazione_oltre_30"));
    // Numero duplicato e importo negativo rifiutati.
    let doppia = Penale {
        id: PenaleId::da_u128(7003),
        numero: 2,
        data: data(2026, 5, 1),
        descrizione: String::new(),
        importo: imp(dec!(1)),
        giorni: None,
        per_mille: None,
    };
    assert!(p.contabilita.registra_penale(doppia.clone()).is_err());
    assert!(
        p.contabilita
            .registra_penale(Penale {
                numero: 3,
                importo: imp(dec!(-1)),
                ..doppia
            })
            .is_err()
    );
}

#[test]
fn sommario_del_registro() {
    let mut p = progetto();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    let s = p.contabilita.sommario(&p.regole).unwrap();
    assert_eq!(s.sal, vec![1, 2]);
    let codici: Vec<_> = s.articoli.iter().map(|a| a.codice.as_str()).collect();
    assert_eq!(codici, ["A.01.001", "B.02.010", "C.03.020", "S.01.001"]);
    let muratura = &s.articoli[2];
    assert_eq!(muratura.per_sal[0].1, imp(dec!(0)));
    assert_eq!(muratura.per_sal[1].0, Quantita::new(dec!(72.00)));
    assert_eq!(muratura.importo, imp(dec!(3078.00)));
    // Totali per SAL = lordo del periodo dei riepiloghi.
    assert_eq!(
        s.totali_per_sal,
        vec![imp(dec!(9153.60)), imp(dec!(3078.00))]
    );
    assert_eq!(s.totale, imp(dec!(12231.60)));
}
