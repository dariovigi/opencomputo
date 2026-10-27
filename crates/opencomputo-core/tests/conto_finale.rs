//! Test di integrazione: stato finale, conto finale, collaudo e rata di saldo.
//!
//! Valori verificati a mano su `progetto_per_conto_finale` (contratto 13.000,00 €,
//! ribasso 12,5 %, ritenuta 0,5 %, anticipazione 2.600,00 € recuperata al 20 %,
//! IVA 10 %):
//!
//! | | SAL 1 | SAL 2 | rata di saldo |
//! |---|---:|---:|---:|
//! | importo netto progressivo | 8.159,40 | 10.852,65 | 11.974,84 |
//! | importo (SAL) / credito residuo | 8.118,60 | 2.679,79 | 1.176,45 |
//! | recupero anticipazione | 1.631,88 | 538,65 | 429,47 (residuo) |
//! | penali | — | 19,50 | 52,00 |
//! | imponibile | 6.486,72 | 2.121,64 | 694,98 |
//! | IVA 10 % | 648,67 | 212,16 (212,164) | 69,50 (69,498) |
//! | totale | 7.135,39 | 2.333,80 | 764,48 |
//!
//! Stato finale: lavori 11.031,60 + 1.282,50 = 12.314,10; ribasso 12,5 % = 1.539,26
//! (1.539,2625); netto = 12.314,10 + 1.200,00 − 1.539,26 = 11.974,84; ritenute
//! svincolate 0,5 % = 59,87 (59,8742); acconti = credito del SAL 2 = 10.798,39; credito
//! residuo = 11.974,84 − 10.798,39 = 1.176,45 (= 59,87 di ritenute + 1.116,58 di lavori).
//! Verifica: anticipazione 2.600,00 + imponibili 6.486,72 + 2.121,64 + 694,98 =
//! 11.903,34 = netto 11.974,84 − penali 71,50.

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

fn codici(p: &Progetto) -> Vec<String> {
    valida_progetto(p)
        .problemi
        .iter()
        .map(|x| x.codice.clone())
        .collect()
}

#[test]
fn certificati_e_rata_di_saldo_verificati_a_mano() {
    let p = progetto_per_conto_finale();
    let c2 = p.contabilita.sal()[1].certificato_emesso.clone().unwrap();
    assert_eq!(c2.recupero_anticipazione, imp(dec!(538.65)));
    assert_eq!(c2.penali, imp(dec!(19.50)));
    assert_eq!(c2.imponibile, imp(dec!(2121.64)));
    assert_eq!(c2.iva, imp(dec!(212.16)));
    assert_eq!(c2.totale, imp(dec!(2333.80)));
    assert_eq!(c2.anticipazione_residua, imp(dec!(429.47)));

    let stato_finale: Vec<u32> = p
        .contabilita
        .registrazioni_stato_finale()
        .iter()
        .map(|r| r.numero)
        .collect();
    assert_eq!(stato_finale, vec![5]);
    let r = p.calcola_conto_finale().unwrap();
    assert_eq!(r.lordo_stato_finale, imp(dec!(1282.50)));
    assert_eq!(r.lavori_lordi, imp(dec!(12314.10)));
    assert_eq!(r.oneri_sicurezza, imp(dec!(1200.00)));
    assert_eq!(r.ribasso, imp(dec!(1539.26)));
    assert_eq!(r.importo_netto, imp(dec!(11974.84)));
    assert_eq!(r.ritenute_garanzia, imp(dec!(59.87)));
    assert_eq!(r.acconti, imp(dec!(10798.39)));
    assert_eq!(r.credito_residuo, imp(dec!(1176.45)));
    assert_eq!(r.anticipazione, imp(dec!(2600.00)));
    assert_eq!(r.anticipazione_residua, imp(dec!(429.47)));
    assert_eq!(r.penali, imp(dec!(52.00)));
    assert_eq!(r.imponibile, imp(dec!(694.98)));
    assert_eq!(r.iva, imp(dec!(69.50)));
    assert_eq!(r.totale, imp(dec!(764.48)));

    // Quadratura: anticipazione + imponibili dei certificati = netto − penali.
    let c1 = p.contabilita.sal()[0].certificato_emesso.clone().unwrap();
    let pagato =
        Importo::totale([r.anticipazione, c1.imponibile, c2.imponibile, r.imponibile]).unwrap();
    let dovuto = r
        .importo_netto
        .differenza(imp(dec!(19.50)))
        .unwrap()
        .differenza(r.penali)
        .unwrap();
    assert_eq!(pagato, dovuto);

    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
}

#[test]
fn emissione_chiude_la_contabilita() {
    let mut p = progetto_per_conto_finale();
    // In bozza non si possono redigere altri SAL; le registrazioni nuove entrano nello
    // stato finale.
    assert!(
        p.contabilita
            .nuovo_sal(SalId::da_u128(9003), data(2026, 5, 22))
            .is_err()
    );
    assert!(
        p.contabilita
            .firma_conto_finale(Some(data(2026, 6, 30)), None)
            .is_err()
    );
    assert!(
        p.emetti_conto_finale(data(2026, 5, 20)).is_err(),
        "prima dell'ultimazione"
    );

    let r = p.emetti_conto_finale(data(2026, 6, 15)).unwrap();
    assert_eq!(r.totale, imp(dec!(764.48)));
    let conto = p.contabilita.conto_finale().unwrap();
    assert_eq!(conto.stato, StatoSal::Emesso);
    assert_eq!(conto.registrazioni, vec![RegistrazioneId::da_u128(5)]);
    assert_eq!(conto.penali, vec![PENALE_2]);
    assert!(p.contabilita.chiusa());

    // Tutto bloccato.
    let r6 = registrazione(&p, 6, 6, data(2026, 5, 21), SCAVO, &[dec!(1)]);
    assert!(p.contabilita.registra(r6).is_err());
    assert!(
        p.contabilita
            .modifica_registrazione(RegistrazioneId::da_u128(5), |r| r.annotazioni =
                Some("x".into()))
            .is_err()
    );
    assert!(
        p.contabilita
            .rimuovi_registrazione(RegistrazioneId::da_u128(5))
            .is_err()
    );
    assert!(p.contabilita.rimuovi_penale(PENALE_2).is_err());
    assert!(p.contabilita.annulla_conto_finale().is_err());
    assert!(
        p.contabilita
            .redigi_conto_finale(data(2026, 5, 25), data(2026, 6, 16), None)
            .is_err()
    );

    // Firma con riserve e certificato di regolare esecuzione.
    assert!(
        p.contabilita
            .firma_conto_finale(Some(data(2026, 6, 1)), None)
            .is_err()
    );
    p.contabilita
        .firma_conto_finale(
            Some(data(2026, 6, 30)),
            Some("Riserva n. 1 confermata".into()),
        )
        .unwrap();
    assert!(
        p.contabilita
            .registra_collaudo(Some(Collaudo {
                tipo: TipoCollaudo::RegolareEsecuzione,
                data: data(2026, 5, 1),
                approvato_il: None,
            }))
            .is_err()
    );
    p.contabilita
        .registra_collaudo(Some(Collaudo {
            tipo: TipoCollaudo::RegolareEsecuzione,
            data: data(2026, 7, 20),
            approvato_il: Some(data(2026, 7, 31)),
        }))
        .unwrap();
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
    assert!(codici(&p).contains(&"contabilita.conto_finale_riserve".to_string()));

    // Parametri alterati dopo l'emissione: rilevati.
    let mut alterato = p.clone();
    alterato.contratto.as_mut().unwrap().parametri.aliquota_iva = Some(Percentuale::new(dec!(22)));
    assert!(codici(&alterato).contains(&"contabilita.conto_finale_alterato".to_string()));
}

#[test]
fn annullare_la_bozza_riapre_i_sal() {
    let mut p = progetto_per_conto_finale();
    p.contabilita.annulla_conto_finale().unwrap();
    assert!(p.contabilita.conto_finale().is_none());
    p.contabilita
        .nuovo_sal(SalId::da_u128(9003), data(2026, 5, 22))
        .unwrap();
    // Con un SAL in bozza il conto finale non si redige.
    assert!(
        p.contabilita
            .redigi_conto_finale(data(2026, 5, 25), data(2026, 6, 15), None)
            .is_err()
    );
    // Ultimazione anteriore all'ultimo SAL: rifiutata.
    p.emetti_sal(SalId::da_u128(9003), data(2026, 5, 23))
        .unwrap();
    assert!(
        p.contabilita
            .redigi_conto_finale(data(2026, 5, 21), data(2026, 6, 15), None)
            .is_err()
    );
    // Con tutto nei SAL il credito residuo sono le sole ritenute.
    p.contabilita
        .redigi_conto_finale(data(2026, 5, 25), data(2026, 6, 15), None)
        .unwrap();
    let r = p.calcola_conto_finale().unwrap();
    assert_eq!(r.lordo_stato_finale, imp(dec!(0)));
    assert_eq!(r.credito_residuo, r.ritenute_garanzia);
    assert_eq!(r.credito_residuo, imp(dec!(59.87)));
}

#[test]
fn termini_segnalati() {
    let mut p = progetto_per_conto_finale();
    // Registrazione successiva all'ultimazione: avviso.
    let r6 = registrazione(&p, 6, 6, data(2026, 6, 2), SCAVO, &[dec!(1)]);
    p.contabilita.registra(r6).unwrap();
    assert!(codici(&p).contains(&"contabilita.registrazione_dopo_ultimazione".to_string()));
    p.contabilita
        .rimuovi_registrazione(RegistrazioneId::da_u128(6))
        .unwrap();
    p.emetti_conto_finale(data(2026, 6, 15)).unwrap();
    // Certificato di regolare esecuzione oltre tre mesi, collaudo oltre sei mesi.
    p.contabilita
        .registra_collaudo(Some(Collaudo {
            tipo: TipoCollaudo::RegolareEsecuzione,
            data: data(2026, 9, 1),
            approvato_il: None,
        }))
        .unwrap();
    let c = codici(&p);
    assert!(c.contains(&"contabilita.cre_oltre_tre_mesi".to_string()));
    assert!(c.contains(&"contabilita.collaudo_da_approvare".to_string()));
    p.contabilita
        .registra_collaudo(Some(Collaudo {
            tipo: TipoCollaudo::Collaudo,
            data: data(2027, 1, 10),
            approvato_il: None,
        }))
        .unwrap();
    assert!(codici(&p).contains(&"contabilita.collaudo_oltre_sei_mesi".to_string()));
    // Conto emesso senza firma: informazione.
    assert!(codici(&p).contains(&"contabilita.conto_finale_da_firmare".to_string()));
}

#[test]
fn sommario_con_lo_stato_finale() {
    let p = progetto_per_conto_finale();
    let s = p.contabilita.sommario(&p.regole).unwrap();
    assert_eq!(s.sal, vec![1, 2]);
    assert!(s.stato_finale);
    assert_eq!(
        s.totali_per_sal,
        vec![imp(dec!(9153.60)), imp(dec!(3078.00)), imp(dec!(1282.50))]
    );
    assert_eq!(s.totale, imp(dec!(13514.10)));
    let muratura = s.articoli.iter().find(|a| a.codice == "C.03.020").unwrap();
    assert_eq!(muratura.per_sal[2].1, imp(dec!(1282.50)));
    assert_eq!(muratura.importo, imp(dec!(4360.50)));
}
