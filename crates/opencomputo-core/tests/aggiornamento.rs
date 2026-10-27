//! Aggiornamento dell'elenco prezzi a un nuovo listino (ADR 0022): confronto, scelta
//! delle voci e dei campi, ricalcolo delle analisi e riallineamento dei computi.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::aggiornamento::{
    CampiAggiornamento, ErroreAggiornamento, EsitoConfronto, SceltaAggiornamento,
    SostituzionePrefisso, aggiorna_da_listino, confronta_listino,
};
use opencomputo_core::partita::CampoPrezzo;
use opencomputo_core::prelude::*;
use rust_decimal_macros::dec;

/// Listino nuovo: scavo a 8,80 con incidenza della sicurezza, calcestruzzo uguale,
/// muratura assente, recinzione a 12,50 con descrizione nuova, una voce in più.
fn listino() -> Prezzario {
    let mut l = elenco_prezzi();
    l.id = PrezzarioId::da_u128(50_000);
    l.nome = "Prezzario regionale 2027".into();
    l.voci.retain(|v| v.codice != "C.03.020");
    for (i, v) in l.voci.iter_mut().enumerate() {
        v.id = VocePrezzarioId::da_u128(50_001 + i as u128);
        match v.codice.as_str() {
            "A.01.001" => {
                v.prezzo = Prezzo::new(dec!(8.80));
                v.incidenza_sicurezza = Some(Percentuale::new(dec!(2)));
            }
            "S.01.001" => {
                v.prezzo = Prezzo::new(dec!(12.50));
                v.descrizione_breve = "Recinzione provvisoria in rete metallica".into();
            }
            _ => {}
        }
    }
    let mut nuova = l.voci[0].clone();
    nuova.id = VocePrezzarioId::da_u128(50_100);
    nuova.codice = "Z.99.001".into();
    l.voci.push(nuova);
    l
}

#[test]
fn confronto_con_differenze_variazioni_e_importi_dei_computi() {
    let p = progetto_di_esempio();
    let c = confronta_listino(&p, &listino(), None);
    let esiti: Vec<_> = c
        .voci
        .iter()
        .map(|v| (v.codice.as_str(), v.esito))
        .collect();
    assert_eq!(
        esiti,
        vec![
            ("A.01.001", EsitoConfronto::Variata),
            ("B.02.010", EsitoConfronto::Invariata),
            ("C.03.020", EsitoConfronto::NonTrovata),
            ("S.01.001", EsitoConfronto::Variata),
        ]
    );
    let scavo = &c.voci[0];
    assert_eq!(scavo.prezzo_listino, Some(Prezzo::new(dec!(8.80))));
    // (8,80 − 8,50) / 8,50 = 3,529 %
    assert_eq!(scavo.variazione, Some(dec!(3.53)));
    let campi: Vec<_> = scavo.differenze.iter().map(|d| d.campo).collect();
    assert_eq!(
        campi,
        vec![CampoPrezzo::Prezzo, CampoPrezzo::IncidenzaSicurezza]
    );
    // 126,00 m³ nel computo: 1 071,00 oggi, 126 × 8,80 = 1 108,80 con il listino.
    assert_eq!(scavo.voci_computo, 1);
    assert_eq!(scavo.importo_computi, Importo::new(dec!(1071.00)));
    assert_eq!(
        scavo.importo_computi_listino,
        Some(Importo::new(dec!(1108.80)))
    );
    let recinzione = &c.voci[3];
    assert_eq!(
        recinzione.differenze[0].campo,
        CampoPrezzo::DescrizioneBreve
    );
    assert_eq!(recinzione.variazione, Some(dec!(4.17)));
    assert_eq!(c.voci[2].prezzo_listino, None);
    assert_eq!(c.voci[2].importo_computi, Importo::new(dec!(4668.30)));
    assert_eq!(c.voci_solo_nel_listino, 1);
    assert!(c.codici_ripetuti.is_empty() && c.computi_non_calcolabili.is_empty());
}

#[test]
fn aggiornamento_delle_voci_scelte_con_riallineamento_dei_computi() {
    let mut p = progetto_di_esempio();
    let l = listino();
    let voci = ["A.01.001".to_owned(), "S.01.001".to_owned()];
    let e = aggiorna_da_listino(
        &mut p,
        &l,
        &SceltaAggiornamento {
            voci: &voci,
            campi: CampiAggiornamento::default(),
            ricalcola_analisi: true,
            riallinea_computi: true,
            ..SceltaAggiornamento::default()
        },
    )
    .unwrap();
    assert_eq!(e.voci, 2);
    assert_eq!(e.riallineate[&COMPUTO], 2);
    let scavo = p.elenco_prezzi.voce(SCAVO).unwrap();
    assert_eq!(scavo.prezzo, Prezzo::new(dec!(8.80)));
    assert_eq!(scavo.incidenza_sicurezza, Some(Percentuale::new(dec!(2))));
    // Le descrizioni restano (campo non scelto); il calcestruzzo non è toccato.
    let recinzione = p.elenco_prezzi.voce(RECINZIONE).unwrap();
    assert_eq!(
        recinzione.descrizione_breve,
        "Recinzione provvisoria di cantiere"
    );
    assert_eq!(
        p.elenco_prezzi.voce(CLS).unwrap().prezzo,
        Prezzo::new(dec!(145.30))
    );
    // 14 457,12 − 1 071,00 + 1 108,80 − 1 200,00 + 1 250,00
    let r = p.calcola_computo(COMPUTO).unwrap();
    assert_eq!(r.totale, Importo::new(dec!(14544.92)));
    assert_eq!(r.totale_sicurezza, Importo::new(dec!(22.18)));
    // Il confronto successivo le dà invariate (salvo la descrizione non scelta).
    let c = confronta_listino(&p, &l, None);
    assert_eq!(c.voci[0].esito, EsitoConfronto::Invariata);
    assert_eq!(c.voci[3].differenze.len(), 1);
}

#[test]
fn senza_riallineamento_i_computi_restano_e_gli_errori_non_modificano() {
    let mut p = progetto_di_esempio();
    let l = listino();
    let voci = ["A.01.001".to_owned()];
    aggiorna_da_listino(
        &mut p,
        &l,
        &SceltaAggiornamento {
            voci: &voci,
            campi: CampiAggiornamento::default(),
            ..SceltaAggiornamento::default()
        },
    )
    .unwrap();
    assert_eq!(
        p.calcola_computo(COMPUTO).unwrap().totale,
        Importo::new(dec!(14457.12))
    );

    let prima = p.clone();
    let voci = ["B.02.010".to_owned(), "C.03.020".to_owned()];
    let e = aggiorna_da_listino(
        &mut p,
        &l,
        &SceltaAggiornamento {
            voci: &voci,
            campi: CampiAggiornamento::default(),
            riallinea_computi: true,
            ..SceltaAggiornamento::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        e,
        ErroreAggiornamento::NelListino("C.03.020".into(), "C.03.020".into())
    );
    assert_eq!(p, prima);
    let e = aggiorna_da_listino(&mut p, &l, &SceltaAggiornamento::default()).unwrap_err();
    assert!(matches!(e, ErroreAggiornamento::Richiesta(_)));
}

#[test]
fn prefisso_sostituito_e_codice_del_listino() {
    let mut p = progetto_di_esempio();
    let mut l = listino();
    for v in &mut l.voci {
        v.codice = v.codice.replacen("A.", "X.", 1);
    }
    let prefisso = SostituzionePrefisso {
        da: "A.".into(),
        a: "X.".into(),
    };
    let c = confronta_listino(&p, &l, Some(&prefisso));
    assert_eq!(c.voci[0].codice_listino, "X.01.001");
    assert_eq!(c.voci[0].esito, EsitoConfronto::Variata);
    assert_eq!(c.voci[0].differenze[0].campo, CampoPrezzo::Codice);
    // Senza sostituzione lo scavo non si trova.
    assert_eq!(
        confronta_listino(&p, &l, None).voci[0].esito,
        EsitoConfronto::NonTrovata
    );
    let voci = ["A.01.001".to_owned()];
    aggiorna_da_listino(
        &mut p,
        &l,
        &SceltaAggiornamento {
            voci: &voci,
            campi: CampiAggiornamento {
                codice: true,
                ..CampiAggiornamento::default()
            },
            prefisso: Some(&prefisso),
            riallinea_computi: true,
            ..SceltaAggiornamento::default()
        },
    )
    .unwrap();
    assert_eq!(p.elenco_prezzi.voce(SCAVO).unwrap().codice, "X.01.001");
    let vc = p.computo(COMPUTO).unwrap().voce(VC_SCAVO).unwrap();
    assert_eq!(vc.prezzo.codice, "X.01.001");
    assert_eq!(vc.prezzo.prezzo, Prezzo::new(dec!(8.80)));
}

#[test]
fn risorse_aggiornate_e_analisi_ricalcolate() {
    let mut p = progetto_con_nuovi_prezzi();
    let mut l = listino();
    let mut operaio = p.elenco_prezzi.risorse[0].clone();
    operaio.prezzo = Prezzo::new(dec!(34.00));
    l.risorse.push(operaio);
    // Il premiscelato è nel listino come voce.
    let mut premiscelato = l.voci[0].clone();
    premiscelato.id = VocePrezzarioId::da_u128(50_200);
    premiscelato.codice = "MA.20".into();
    premiscelato.descrizione_breve = "Intonaco premiscelato in sacchi".into();
    premiscelato.unita_misura = "kg".into();
    premiscelato.prezzo = Prezzo::new(dec!(0.55));
    l.voci.push(premiscelato);
    // Il nuovo prezzo analizzato compare nel listino.
    let mut np = l.voci[0].clone();
    np.id = VocePrezzarioId::da_u128(50_300);
    np.codice = "NP.01".into();
    l.voci.push(np);

    let c = confronta_listino(&p, &l, None);
    let r: Vec<_> = c
        .risorse
        .iter()
        .map(|r| (r.codice.as_str(), r.esito, r.analisi))
        .collect();
    assert_eq!(
        r,
        vec![
            ("MO.02", EsitoConfronto::Variata, 1),
            ("MA.20", EsitoConfronto::Variata, 1)
        ]
    );
    assert_eq!(c.risorse[0].variazione, Some(dec!(6.25)));
    let np1 = c.voci.iter().find(|v| v.codice == "NP.01").unwrap();
    assert_eq!(np1.esito, EsitoConfronto::Analizzata);

    // La voce analizzata non si aggiorna dal listino.
    let voci = ["NP.01".to_owned()];
    let e = aggiorna_da_listino(
        &mut p,
        &l,
        &SceltaAggiornamento {
            voci: &voci,
            campi: CampiAggiornamento::default(),
            ..SceltaAggiornamento::default()
        },
    )
    .unwrap_err();
    assert_eq!(e, ErroreAggiornamento::Analizzata("NP.01".into()));

    let risorse = ["MO.02".to_owned()];
    let e = aggiorna_da_listino(
        &mut p,
        &l,
        &SceltaAggiornamento {
            risorse: &risorse,
            campi: CampiAggiornamento::default(),
            ricalcola_analisi: true,
            riallinea_computi: true,
            ..SceltaAggiornamento::default()
        },
    )
    .unwrap();
    assert_eq!((e.risorse, e.analisi_ricalcolate), (1, 1));
    // 0,25 h × 34,00 = 8,50; + 6,00 = 14,50; spese generali 15 % = 2,175 → 2,18;
    // utile 10 % di 16,68 = 1,668 → 1,67; prezzo 18,35.
    let np1 = p.elenco_prezzi.voce(NP1).unwrap();
    assert_eq!(np1.prezzo, Prezzo::new(dec!(18.35)));
    // Le registrazioni del libretto conservano il prezzo fotografato.
    assert!(
        p.contabilita
            .libretto()
            .registrazioni
            .iter()
            .any(|r| r.prezzo.codice == "NP.01" && r.prezzo.prezzo == Prezzo::new(dec!(17.71)))
    );
}
