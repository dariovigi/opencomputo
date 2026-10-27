//! Test di integrazione: nuovi prezzi, verbali di concordamento ed esclusione dal
//! ribasso nei SAL.
//!
//! SAL 2 di `progetto_con_nuovi_prezzi` (ribasso 12,5 %, ritenuta 0,5 %), valori
//! progressivi verificati a mano:
//!
//! | | importo |
//! |---|---:|
//! | lavori lordi 11.031,60 + 177,10 (NP.1) + 50,00 (NP.2) | 11.258,70 |
//! | esclusi dal ribasso (NP.1) | 177,10 |
//! | soggetto a ribasso | 11.081,60 |
//! | ribasso 12,5 % | 1.385,20 |
//! | netto = 11.258,70 + 1.200,00 − 1.385,20 | 11.073,50 |
//! | ritenuta 0,5 % (55,3675) | 55,37 |
//! | credito | 11.018,13 |
//! | importo del SAL 2 = 11.018,13 − 8.118,60 | 2.899,53 |

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

fn codici(p: &Progetto) -> Vec<String> {
    valida_progetto(p)
        .problemi
        .into_iter()
        .map(|x| x.codice)
        .collect()
}

#[test]
fn nuovo_prezzo_escluso_dal_ribasso_nel_sal() {
    let mut p = progetto_con_nuovi_prezzi();
    let r5 = p
        .contabilita
        .libretto()
        .registrazione(RegistrazioneId::da_u128(5))
        .unwrap();
    assert!(r5.prezzo.escluso_dal_ribasso);
    assert_eq!(
        r5.calcola(&p.regole).unwrap().importo,
        Importo::new(dec!(177.10))
    );
    // L'analisi del NP.1 dà il prezzo dell'elenco.
    let a = p.elenco_prezzi.calcola_analisi(AN_NP1, &p.regole).unwrap();
    assert_eq!(a.prezzo_unitario, Prezzo::new(dec!(17.71)));
    assert_eq!(a.incidenza_manodopera, Some(Percentuale::new(dec!(45.17))));

    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    let s1 = p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    assert_eq!(s1.importo_sal, Importo::new(dec!(8118.60)));
    assert!(s1.lavori_esclusi_dal_ribasso.is_zero());
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    let s2 = p.calcola_sal(SAL2).unwrap();
    assert_eq!(s2.lavori_lordi, Importo::new(dec!(11258.70)));
    assert_eq!(s2.lavori_esclusi_dal_ribasso, Importo::new(dec!(177.10)));
    assert_eq!(s2.soggetto_a_ribasso, Importo::new(dec!(11081.60)));
    assert_eq!(s2.ribasso, Importo::new(dec!(1385.20)));
    assert_eq!(s2.importo_netto, Importo::new(dec!(11073.50)));
    assert_eq!(s2.ritenuta_garanzia, Importo::new(dec!(55.37)));
    assert_eq!(s2.importo_sal, Importo::new(dec!(2899.53)));
}

#[test]
fn approvazione_e_segnalazioni() {
    let mut p = progetto_con_nuovi_prezzi();
    let c = codici(&p);
    assert!(!valida_progetto(&p).ha_errori(), "{c:?}");
    // Usati nel libretto senza approvazione: un avviso per ciascun nuovo prezzo.
    assert_eq!(
        c.iter()
            .filter(|x| *x == "contabilita.nuovo_prezzo_non_approvato")
            .count(),
        2
    );
    assert!(c.contains(&"prezzario.nuovo_prezzo_escluso_dal_ribasso".to_string()));
    let mut v = p.contabilita.verbale_nuovi_prezzi(1).unwrap().clone();
    v.approvato_il = Some(data(2026, 4, 9));
    assert!(
        p.contabilita
            .imposta_verbale_nuovi_prezzi(v.clone())
            .is_err(),
        "approvazione anteriore al verbale"
    );
    v.approvato_il = Some(data(2026, 4, 15));
    v.atto_approvazione = Some("Determina RUP n. 12".into());
    p.contabilita.imposta_verbale_nuovi_prezzi(v).unwrap();
    assert!(!codici(&p).contains(&"contabilita.nuovo_prezzo_non_approvato".to_string()));
    assert!(p.contabilita.rimuovi_verbale_nuovi_prezzi(1).is_err());

    // Verbale inesistente e criterio «analisi» senza analisi.
    let np2 = p.elenco_prezzi.voce_mut(NP2).unwrap();
    let n = np2.nuovo_prezzo.as_mut().unwrap();
    n.verbale = Some(9);
    n.criterio = CriterioNuovoPrezzo::Analisi;
    let c = codici(&p);
    assert!(c.contains(&"prezzario.verbale_inesistente".to_string()));
    assert!(c.contains(&"prezzario.nuovo_prezzo_senza_analisi".to_string()));
}

#[test]
fn prezzo_applicato_e_confronto() {
    let p = progetto_con_nuovi_prezzi();
    let e = &p.elenco_prezzi;
    let mut voce = e.voce(NP1).unwrap().clone();
    let applicato = PrezzoApplicato::da_voce(e, &voce);
    assert!(applicato.escluso_dal_ribasso);
    voce.nuovo_prezzo.as_mut().unwrap().soggetto_a_ribasso = true;
    let d = applicato.confronta(&voce);
    assert_eq!(d.len(), 1);
    assert_eq!(
        d[0].campo,
        opencomputo_core::partita::CampoPrezzo::EsclusoDalRibasso
    );
}
