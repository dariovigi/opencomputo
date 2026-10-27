//! Test di integrazione: libretto delle misure, registro e SAL.

// Le opzioni `allow-*-in-tests` di clippy.toml coprono solo le funzioni `#[test]`:
// negli helper condivisi (`common`) e nei costruttori di dati di prova un errore
// deve far fallire il test immediatamente.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::contabilita::{Contabilita, StatoSal};
use opencomputo_core::prelude::*;
use rust_decimal_macros::dec;

fn imp(v: Decimal) -> Importo {
    Importo::new(v)
}

fn emetti_due_sal(p: &mut Progetto) -> (RiepilogoSal, RiepilogoSal) {
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    let r1 = p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    let r2 = p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    (r1, r2)
}

#[test]
fn sal_verificati_a_mano() {
    let mut p = progetto_con_contabilita();
    let (r1, r2) = emetti_due_sal(&mut p);

    // SAL 1: registrazioni 1-3.
    assert_eq!(r1.numero, 1);
    assert_eq!(r1.lordo_periodo, imp(dec!(9153.60)));
    assert_eq!(r1.lavori_lordi, imp(dec!(7953.60)));
    assert_eq!(r1.oneri_sicurezza, imp(dec!(1200.00)));
    assert_eq!(r1.manodopera_esclusa, Importo::ZERO);
    assert_eq!(r1.soggetto_a_ribasso, imp(dec!(7953.60)));
    assert_eq!(r1.ribasso, imp(dec!(994.20)));
    assert_eq!(r1.importo_netto, imp(dec!(8159.40)));
    // 8 159,40 × 0,5 % = 40,797 → 40,80
    assert_eq!(r1.ritenuta_garanzia, imp(dec!(40.80)));
    assert_eq!(r1.credito_progressivo, imp(dec!(8118.60)));
    assert_eq!(r1.certificati_precedenti, Importo::ZERO);
    assert_eq!(r1.importo_sal, imp(dec!(8118.60)));

    // SAL 2: registrazione 4 (progressivi).
    assert_eq!(r2.lordo_periodo, imp(dec!(3078.00)));
    assert_eq!(r2.lavori_lordi, imp(dec!(11031.60)));
    assert_eq!(r2.ribasso, imp(dec!(1378.95)));
    assert_eq!(r2.importo_netto, imp(dec!(10852.65)));
    // 10 852,65 × 0,5 % = 54,26325 → 54,26
    assert_eq!(r2.ritenuta_garanzia, imp(dec!(54.26)));
    assert_eq!(r2.credito_progressivo, imp(dec!(10798.39)));
    assert_eq!(r2.certificati_precedenti, imp(dec!(8118.60)));
    assert_eq!(r2.importo_sal, imp(dec!(2679.79)));

    // La somma dei SAL coincide con il credito progressivo.
    assert_eq!(
        r1.importo_sal.somma(r2.importo_sal).unwrap(),
        r2.credito_progressivo
    );
    assert!(
        valida_progetto(&p)
            .problemi
            .iter()
            .all(|x| x.gravita != Gravita::Errore)
    );
}

#[test]
fn manodopera_esclusa_dal_ribasso() {
    let mut p = progetto_con_contabilita();
    p.contratto
        .as_mut()
        .unwrap()
        .parametri
        .manodopera_esclusa_dal_ribasso = true;
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    let r = p.calcola_sal(SAL1).unwrap();
    // 244,80 + 1 290,26 (la recinzione è sicurezza)
    assert_eq!(r.manodopera_esclusa, imp(dec!(1535.06)));
    assert_eq!(r.soggetto_a_ribasso, imp(dec!(6418.54)));
    // 6 418,54 × 12,5 % = 802,3175 → 802,32
    assert_eq!(r.ribasso, imp(dec!(802.32)));
    assert_eq!(r.importo_netto, imp(dec!(8351.28)));
    // 8 351,28 × 0,5 % = 41,7564 → 41,76
    assert_eq!(r.ritenuta_garanzia, imp(dec!(41.76)));
    assert_eq!(r.credito_progressivo, imp(dec!(8309.52)));
}

#[test]
fn registro_di_contabilita() {
    let mut p = progetto_con_contabilita();
    emetti_due_sal(&mut p);
    let registro = p.contabilita.registro(&p.regole).unwrap();
    let righe: Vec<(u32, u32, u32, Importo)> = registro
        .righe
        .iter()
        .map(|r| (r.numero, r.numero_sal, r.numero_libretto, r.importo))
        .collect();
    assert_eq!(
        righe,
        vec![
            (1, 1, 1, imp(dec!(979.20))),
            (2, 1, 2, imp(dec!(6974.40))),
            (3, 1, 3, imp(dec!(1200.00))),
            (4, 2, 4, imp(dec!(3078.00))),
        ]
    );
    assert_eq!(registro.totali[0].importo_periodo, imp(dec!(9153.60)));
    assert_eq!(registro.totali[0].importo_progressivo, imp(dec!(9153.60)));
    assert_eq!(registro.totali[1].importo_periodo, imp(dec!(3078.00)));
    assert_eq!(registro.totali[1].importo_progressivo, imp(dec!(12231.60)));
}

#[test]
fn le_registrazioni_di_un_sal_emesso_sono_bloccate() {
    let mut p = progetto_con_contabilita();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();

    // In bozza la modifica è ammessa, ma non oltre il periodo del SAL.
    p.contabilita
        .modifica_registrazione(RegistrazioneId::da_u128(1), |r| {
            r.annotazioni = Some("verificato in contraddittorio".into());
        })
        .unwrap();
    let fuori_periodo = p
        .contabilita
        .modifica_registrazione(RegistrazioneId::da_u128(1), |r| r.data = data(2026, 4, 2));
    assert!(matches!(
        fuori_periodo,
        Err(ErroreDominio::OperazioneNonConsentita(_))
    ));

    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    let modifica = p
        .contabilita
        .modifica_registrazione(RegistrazioneId::da_u128(1), |r| {
            r.righi[0].lunghezza = Some(Fattore::Valore(dec!(13)));
        });
    assert!(matches!(
        modifica,
        Err(ErroreDominio::OperazioneNonConsentita(_))
    ));
    let rimozione = p
        .contabilita
        .rimuovi_registrazione(RegistrazioneId::da_u128(2));
    assert!(matches!(
        rimozione,
        Err(ErroreDominio::OperazioneNonConsentita(_))
    ));
    let riemissione = p.emetti_sal(SAL1, data(2026, 4, 6));
    assert!(matches!(
        riemissione,
        Err(ErroreDominio::OperazioneNonConsentita(_))
    ));
    assert!(p.contabilita.annulla_bozza_sal(SAL1).is_err());

    // La registrazione 4 non è contabilizzata: resta modificabile.
    p.contabilita
        .modifica_registrazione(RegistrazioneId::da_u128(4), |r| {
            r.annotazioni = Some("ok".into())
        })
        .unwrap();
    // Il cambio di identificativo e i numeri duplicati sono rifiutati.
    assert!(
        p.contabilita
            .modifica_registrazione(RegistrazioneId::da_u128(4), |r| r.id =
                RegistrazioneId::da_u128(99))
            .is_err()
    );
    assert!(
        p.contabilita
            .modifica_registrazione(RegistrazioneId::da_u128(4), |r| r.numero = 1)
            .is_err()
    );
}

#[test]
fn sequenza_dei_sal() {
    let mut p = progetto_con_contabilita();
    // Nessuna registrazione prima del 10 marzo.
    assert!(p.contabilita.nuovo_sal(SAL1, data(2026, 3, 1)).is_err());
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    assert_eq!(p.contabilita.sal()[0].registrazioni.len(), 3);
    // Un solo SAL in bozza alla volta.
    assert!(p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).is_err());
    // Data di emissione anteriore al periodo.
    assert!(p.emetti_sal(SAL1, data(2026, 3, 30)).is_err());
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    // La data del SAL successivo deve essere posteriore.
    assert!(p.contabilita.nuovo_sal(SAL2, data(2026, 3, 31)).is_err());
    // Identificativo già usato.
    assert!(p.contabilita.nuovo_sal(SAL1, data(2026, 4, 30)).is_err());

    // Una misurazione registrata in ritardo, con data nel periodo del SAL 1,
    // confluisce nel SAL successivo.
    let tardiva = registrazione(&p, 5, 5, data(2026, 3, 28), SCAVO, &[dec!(10)]);
    p.contabilita.registra(tardiva).unwrap();
    let sal2 = p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    assert_eq!(
        sal2.registrazioni,
        vec![RegistrazioneId::da_u128(4), RegistrazioneId::da_u128(5)]
    );
    assert_eq!(sal2.stato, StatoSal::Bozza);
    assert_eq!(sal2.numero, 2);

    // Annullando la bozza le registrazioni tornano da contabilizzare.
    p.contabilita.annulla_bozza_sal(SAL2).unwrap();
    assert_eq!(
        p.contabilita
            .registrazioni_da_contabilizzare(data(2026, 12, 31))
            .len(),
        2
    );
}

#[test]
fn rimozione_da_un_sal_in_bozza() {
    let mut p = progetto_con_contabilita();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    let rimossa = p
        .contabilita
        .rimuovi_registrazione(RegistrazioneId::da_u128(2))
        .unwrap();
    assert_eq!(rimossa.numero, 2);
    assert_eq!(p.contabilita.sal()[0].registrazioni.len(), 2);
    let r = p.calcola_sal(SAL1).unwrap();
    assert_eq!(r.lavori_lordi, imp(dec!(979.20)));
}

#[test]
fn numerazione_del_libretto() {
    let mut p = progetto_con_contabilita();
    assert_eq!(p.contabilita.libretto().prossimo_numero(), 5);
    let duplicata = registrazione(&p, 50, 3, data(2026, 4, 1), SCAVO, &[dec!(1)]);
    assert!(matches!(
        p.contabilita.registra(duplicata),
        Err(ErroreDominio::CodiceDuplicato { .. })
    ));
    let zero = registrazione(&p, 51, 0, data(2026, 4, 1), SCAVO, &[dec!(1)]);
    assert!(p.contabilita.registra(zero).is_err());
    let stesso_id = registrazione(&p, 1, 60, data(2026, 4, 1), SCAVO, &[dec!(1)]);
    assert!(matches!(
        p.contabilita.registra(stesso_id),
        Err(ErroreDominio::IdDuplicato { .. })
    ));
}

#[test]
fn alterazione_di_un_sal_emesso_rilevata() {
    let mut p = progetto_con_contabilita();
    emetti_due_sal(&mut p);
    assert!(
        p.contabilita
            .verifica_integrita(&p.contratto.as_ref().unwrap().parametri, &p.regole)
            .is_empty()
    );

    // Alterazione esterna del file: la quantità della registrazione 1 cambia.
    let mut json = serde_json::to_value(&p.contabilita).unwrap();
    json["libretto"]["registrazioni"][0]["righi"][0]["lunghezza"] =
        serde_json::json!({ "valore": "13" });
    let alterata: Contabilita = serde_json::from_value(json).unwrap();
    let problemi = alterata.verifica_integrita(&p.contratto.as_ref().unwrap().parametri, &p.regole);
    let codici: Vec<&str> = problemi.iter().map(|x| x.codice.as_str()).collect();
    // Entrambi i SAL sono progressivi e quindi entrambi risultano alterati.
    assert_eq!(
        codici,
        vec!["contabilita.sal_alterato", "contabilita.sal_alterato"]
    );

    // Anche la modifica dei parametri contrattuali dopo l'emissione è rilevata.
    p.contratto.as_mut().unwrap().parametri.ribasso = Percentuale::new(dec!(13));
    let rapporto = valida_progetto(&p);
    assert_eq!(rapporto.con_codice("contabilita.sal_alterato").count(), 2);
    assert!(rapporto.ha_errori());
}

#[test]
fn incoerenze_strutturali_rilevate() {
    let mut p = progetto_con_contabilita();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    let mut json = serde_json::to_value(&p.contabilita).unwrap();
    // Una registrazione inesistente e una registrazione fuori periodo.
    json["sal"][0]["registrazioni"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("00000000-0000-0000-0000-0000000003e7"));
    json["sal"][0]["registrazioni"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(RegistrazioneId::da_u128(4).to_string()));
    let c: Contabilita = serde_json::from_value(json).unwrap();
    let problemi = c.verifica_integrita(&ParametriContabili::default(), &p.regole);
    let codici: Vec<&str> = problemi.iter().map(|x| x.codice.as_str()).collect();
    assert!(codici.contains(&"contabilita.registrazione_inesistente"));
    assert!(codici.contains(&"contabilita.registrazione_oltre_periodo"));
    assert!(
        c.calcola_sal(SAL1, &ParametriContabili::default(), &p.regole)
            .is_err()
    );
}

#[test]
fn sal_senza_contratto() {
    let mut p = progetto_con_contabilita();
    p.contratto = None;
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    assert!(matches!(
        p.calcola_sal(SAL1),
        Err(ErroreDominio::OperazioneNonConsentita(_))
    ));
}
