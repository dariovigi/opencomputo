//! Test di integrazione: analisi prezzi dinamiche e annidate, incidenze, ricalcolo dei
//! prezzi e fabbisogno di cantiere.
//!
//! Valori verificati a mano (arrotondamento commerciale a 2 decimali):
//!
//! Malta (1 m³): cemento 3 q × 15,00 = 45,00; sabbia 1 m³ × 28,00 = 28,00; operaio
//! comune 2 h × 32,00 = 64,00; betoniera 1,5 h × 4,00 = 6,00. Costo 143,00; spese
//! generali 15 % = 21,45; utile 10 % di 164,45 = 16,45 (16,445); totale e prezzo
//! 180,90. Incidenze: manodopera 64/180,90 = 35,38 %; materiali 73/180,90 = 40,35 %;
//! noli 6/180,90 = 3,32 %.
//!
//! Muratura (1 m²): malta 0,03 m³ × 180,90 = 5,43 (5,427); mattoni 60 × 0,45 = 27,00;
//! operaio qualificato 0,8 h × 35,50 = 28,40; operaio comune 0,8 h × 32,00 = 25,60;
//! autocarro 0,02 h × 50,00 = 1,00. Costo 87,43; spese generali 13,11 (13,1145); utile
//! 10,05 (10,054); totale e prezzo 110,59. Incidenze con la malta ripartita:
//! manodopera (28,40 + 25,60 + 5,43 × 64/180,90) / 110,59 = 50,57 %; materiali
//! (27,00 + 5,43 × 73/180,90) / 110,59 = 26,40 %; noli 0,16 %; trasporti 0,90 %.
//!
//! Con il cemento a 16,00: malta 146,00 + 21,90 + 16,79 = 184,69; muratura: malta
//! 5,54, costo 87,54, spese generali 13,13, utile 10,07, prezzo 110,74.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use opencomputo_core::analisi::{
    AnalisiPrezzo, OrigineRigaAnalisi, ParametriAnalisi, RigaAnalisi, RisorsaElementare,
    TipoRisorsa,
};
use opencomputo_core::prelude::*;
use opencomputo_core::validazione::valida_progetto;
use rust_decimal_macros::dec;
use std::sync::Arc;

const ELENCO: PrezzarioId = PrezzarioId::da_u128(1);
const MALTA: VocePrezzarioId = VocePrezzarioId::da_u128(10);
const MUR: VocePrezzarioId = VocePrezzarioId::da_u128(11);
const MATTONI: VocePrezzarioId = VocePrezzarioId::da_u128(12);
const A_MALTA: AnalisiId = AnalisiId::da_u128(20);
const A_MUR: AnalisiId = AnalisiId::da_u128(21);
const CEMENTO: RisorsaId = RisorsaId::da_u128(31);

fn imp(v: Decimal) -> Importo {
    Importo::new(v)
}

fn risorsa(
    n: u128,
    codice: &str,
    tipo: TipoRisorsa,
    um: &str,
    prezzo: Decimal,
) -> RisorsaElementare {
    RisorsaElementare {
        id: RisorsaId::da_u128(n),
        codice: codice.into(),
        descrizione: codice.into(),
        unita_misura: um.into(),
        tipo,
        prezzo: Prezzo::new(prezzo),
    }
}

fn riga_voce(p: &Prezzario, voce: VocePrezzarioId, tipo: TipoRisorsa, q: Decimal) -> RigaAnalisi {
    let v = p.voce(voce).unwrap();
    RigaAnalisi {
        codice: v.codice.clone(),
        descrizione: v.descrizione_breve.clone(),
        unita_misura: v.unita_misura.clone(),
        tipo,
        prezzo: v.prezzo,
        quantita: Quantita::new(q),
        espressione: None,
        origine: Some(OrigineRigaAnalisi::VocePrezzario(voce)),
    }
}

fn analisi(id: AnalisiId, codice: &str, um: &str, righe: Vec<RigaAnalisi>) -> AnalisiPrezzo {
    let par = ParametriAnalisi::default();
    AnalisiPrezzo {
        id,
        codice: codice.into(),
        descrizione: codice.into(),
        unita_misura: um.into(),
        quantita_riferimento: Quantita::new(dec!(1)),
        righe,
        spese_generali: par.spese_generali,
        utile_impresa: par.utile_impresa,
        base_utile: par.base_utile,
        variabili: Vec::new(),
        quota_oneri_sicurezza: None,
    }
}

fn elenco() -> Prezzario {
    let mut p = Prezzario::new(ELENCO, "Elenco prezzi");
    let r = |n, c, t, um, v| risorsa(n, c, t, um, v);
    let cemento = r(31, "MA.01", TipoRisorsa::Materiale, "q", dec!(15.00));
    let sabbia = r(32, "MA.02", TipoRisorsa::Materiale, "m³", dec!(28.00));
    let qualificato = r(33, "MO.01", TipoRisorsa::Manodopera, "h", dec!(35.50));
    let comune = r(34, "MO.02", TipoRisorsa::Manodopera, "h", dec!(32.00));
    let betoniera = r(35, "NO.01", TipoRisorsa::Nolo, "h", dec!(4.00));
    let autocarro = r(36, "TR.01", TipoRisorsa::Trasporto, "h", dec!(50.00));
    p.risorse = vec![
        cemento.clone(),
        sabbia.clone(),
        qualificato.clone(),
        comune.clone(),
        betoniera.clone(),
        autocarro.clone(),
    ];
    let mut malta = VocePrezzario::new(
        MALTA,
        "NP.MALTA",
        "Malta cementizia",
        "m³",
        Prezzo::new(dec!(180.90)),
    );
    malta.analisi = Some(A_MALTA);
    let mut mur = VocePrezzario::new(
        MUR,
        "NP.MUR",
        "Muratura in mattoni pieni",
        "m²",
        Prezzo::new(dec!(110.59)),
    );
    mur.analisi = Some(A_MUR);
    let mattoni = VocePrezzario::new(
        MATTONI,
        "MAT.01",
        "Mattoni pieni",
        "cad",
        Prezzo::new(dec!(0.45)),
    );
    p.aggiungi_voce(malta).unwrap();
    p.aggiungi_voce(mur).unwrap();
    p.aggiungi_voce(mattoni).unwrap();
    let q = |v| Quantita::new(v);
    p.analisi.push(analisi(
        A_MALTA,
        "NP.MALTA",
        "m³",
        vec![
            RigaAnalisi::da_risorsa(&cemento, q(dec!(3))),
            RigaAnalisi::da_risorsa(&sabbia, q(dec!(1))),
            RigaAnalisi::da_risorsa(&comune, q(dec!(2))),
            RigaAnalisi::da_risorsa(&betoniera, q(dec!(1.5))),
        ],
    ));
    let righe_mur = vec![
        riga_voce(&p, MALTA, TipoRisorsa::Materiale, dec!(0.03)),
        riga_voce(&p, MATTONI, TipoRisorsa::Materiale, dec!(60)),
        RigaAnalisi::da_risorsa(&qualificato, q(dec!(0.8))),
        RigaAnalisi::da_risorsa(&comune, q(dec!(0.8))),
        RigaAnalisi::da_risorsa(&autocarro, q(dec!(0.02))),
    ];
    p.analisi.push(analisi(A_MUR, "NP.MUR", "m²", righe_mur));
    p
}

fn progetto() -> Progetto {
    let mut p = Progetto::new(ProgettoId::da_u128(1), "Prova analisi", ELENCO);
    p.elenco_prezzi = elenco();
    p
}

#[test]
fn analisi_annidata_verificata_a_mano() {
    let p = elenco();
    let r = RegoleArrotondamento::default();
    let malta = p.calcola_analisi(A_MALTA, &r).unwrap();
    assert_eq!(malta.costo_diretto, imp(dec!(143.00)));
    assert_eq!(malta.spese_generali, imp(dec!(21.45)));
    assert_eq!(malta.utile, imp(dec!(16.45)));
    assert_eq!(malta.prezzo_unitario, Prezzo::new(dec!(180.90)));
    assert_eq!(
        malta.incidenze[&TipoRisorsa::Manodopera],
        Percentuale::new(dec!(35.38))
    );
    assert_eq!(
        malta.incidenze[&TipoRisorsa::Materiale],
        Percentuale::new(dec!(40.35))
    );
    assert_eq!(
        malta.incidenze[&TipoRisorsa::Nolo],
        Percentuale::new(dec!(3.32))
    );

    let mur = p.calcola_analisi(A_MUR, &r).unwrap();
    assert_eq!(mur.righe[0].prezzo, Prezzo::new(dec!(180.90)));
    assert_eq!(mur.righe[0].importo, imp(dec!(5.43)));
    assert_eq!(mur.costo_diretto, imp(dec!(87.43)));
    assert_eq!(mur.spese_generali, imp(dec!(13.11)));
    assert_eq!(mur.utile, imp(dec!(10.05)));
    assert_eq!(mur.prezzo_unitario, Prezzo::new(dec!(110.59)));
    assert_eq!(
        mur.incidenza_manodopera,
        Some(Percentuale::new(dec!(50.57)))
    );
    assert_eq!(
        mur.incidenze[&TipoRisorsa::Materiale],
        Percentuale::new(dec!(26.40))
    );
    assert_eq!(
        mur.incidenze[&TipoRisorsa::Nolo],
        Percentuale::new(dec!(0.16))
    );
    assert_eq!(
        mur.incidenze[&TipoRisorsa::Trasporto],
        Percentuale::new(dec!(0.90))
    );
    // L'analisi isolata usa le copie: la malta come materiale, senza ripartizione.
    let isolata = p.analisi(A_MUR).unwrap().calcola(&r).unwrap();
    assert_eq!(isolata.prezzo_unitario, Prezzo::new(dec!(110.59)));
    assert_eq!(
        isolata.incidenza_manodopera,
        Some(Percentuale::new(dec!(48.83)))
    );
}

#[test]
fn prezzi_correnti_e_ricalcolo_esplicito() {
    let mut p = progetto();
    let r = p.regole.clone();
    assert!(!valida_progetto(&p).ha_errori());
    p.elenco_prezzi
        .risorse
        .iter_mut()
        .find(|x| x.id == CEMENTO)
        .unwrap()
        .prezzo = Prezzo::new(dec!(16.00));
    let mur = p.elenco_prezzi.calcola_analisi(A_MUR, &r).unwrap();
    assert_eq!(mur.righe[0].importo, imp(dec!(5.54)));
    assert_eq!(mur.prezzo_unitario, Prezzo::new(dec!(110.74)));
    // Il prezzo della voce non cambia da solo: la validazione lo segnala.
    assert_eq!(
        p.elenco_prezzi.voce(MUR).unwrap().prezzo,
        Prezzo::new(dec!(110.59))
    );
    let codici: Vec<String> = valida_progetto(&p)
        .problemi
        .into_iter()
        .map(|x| x.codice)
        .collect();
    assert_eq!(
        codici
            .iter()
            .filter(|c| *c == "prezzario.prezzo_analisi_non_allineato")
            .count(),
        2
    );
    let cambiati = p.elenco_prezzi.ricalcola_prezzi_da_analisi(&r).unwrap();
    assert_eq!(
        cambiati,
        vec![
            (MALTA, Prezzo::new(dec!(180.90)), Prezzo::new(dec!(184.69))),
            (MUR, Prezzo::new(dec!(110.59)), Prezzo::new(dec!(110.74))),
        ]
    );
    let v = p.elenco_prezzi.voce(MUR).unwrap();
    assert_eq!(v.incidenza_manodopera, Some(Percentuale::new(dec!(50.50))));
    assert!(p.elenco_prezzi.prezzi_non_allineati(&r).unwrap().is_empty());
}

#[test]
fn analisi_ciclica_rifiutata() {
    let mut p = progetto();
    let riga = riga_voce(&p.elenco_prezzi, MUR, TipoRisorsa::Altro, dec!(0.1));
    p.elenco_prezzi.analisi[0].righe.push(riga);
    let e = p
        .elenco_prezzi
        .calcola_analisi(A_MUR, &p.regole)
        .unwrap_err();
    assert!(e.to_string().contains("NP.MUR → NP.MALTA → NP.MUR"), "{e}");
    let rapporto = valida_progetto(&p);
    assert!(
        rapporto
            .problemi
            .iter()
            .any(|x| x.codice == "prezzario.analisi_ciclica")
    );
    assert!(
        p.elenco_prezzi
            .ricalcola_prezzi_da_analisi(&p.regole)
            .is_err()
    );
}

#[test]
fn fabbisogno_di_cantiere() {
    let mut p = progetto();
    let mut c = Computo::new(ComputoId::da_u128(50), "Computo");
    let voce = |id: u128, v: VocePrezzarioId, q: Decimal| {
        let e = &p.elenco_prezzi;
        let mut x = VoceComputo::new(
            VoceComputoId::da_u128(id),
            PrezzoApplicato::da_voce(e, e.voce(v).unwrap()),
        );
        x.righi
            .push(RigoMisura::misura(RigoMisuraId::da_u128(id * 10), "").con_lunghezza(q));
        x
    };
    c.voci.push(Arc::new(voce(51, MUR, dec!(100))));
    c.voci.push(Arc::new(voce(52, MALTA, dec!(2))));
    c.voci.push(Arc::new(voce(53, MATTONI, dec!(10))));
    p.computi.push(c);
    let f = p.fabbisogno(ComputoId::da_u128(50)).unwrap();
    let righe: Vec<(String, Decimal, Decimal)> = f
        .risorse
        .iter()
        .map(|r| (r.codice.clone(), r.quantita.valore(), r.importo.valore()))
        .collect();
    assert_eq!(
        righe,
        vec![
            ("MO.01".into(), dec!(80.00), dec!(2840.00)),
            ("MO.02".into(), dec!(90.00), dec!(2880.00)),
            ("MA.01".into(), dec!(15.00), dec!(225.00)),
            ("MA.02".into(), dec!(5.00), dec!(140.00)),
            ("MAT.01".into(), dec!(6000.00), dec!(2700.00)),
            ("NO.01".into(), dec!(7.50), dec!(30.00)),
            ("TR.01".into(), dec!(2.00), dec!(100.00)),
        ]
    );
    assert_eq!(
        f.totali_per_tipo[&TipoRisorsa::Manodopera],
        imp(dec!(5720.00))
    );
    assert_eq!(
        f.totali_per_tipo[&TipoRisorsa::Materiale],
        imp(dec!(3065.00))
    );
    assert_eq!(f.totale, imp(dec!(8915.00)));
    assert_eq!(f.voci_senza_analisi, vec!["MAT.01".to_string()]);
}

/// Variabili (verificato a mano): spessore = 0,015; kg = spessore × 1400 = 21.
/// Operaio comune `0,02+spessore*10` = 0,17 h × 32,00 = 5,44; cemento `kg/100` = 0,21 q ×
/// 15,00 = 3,15; betoniera `q1/2` = 0,085 h × 4,00 = 0,34. Costo 8,93; spese generali
/// 1,34 (1,3395); utile 1,03 (1,027); prezzo 11,30. Oneri aziendali della sicurezza 20 %
/// delle spese generali = 0,27 (0,268).
#[test]
fn variabili_quantita_dalle_righe_e_oneri_della_sicurezza() {
    use opencomputo_core::analisi::VariabileAnalisi;
    let mut p = elenco();
    let r = RegoleArrotondamento::default();
    let risorsa = |codice: &str| {
        p.risorse
            .iter()
            .find(|x| x.codice == codice)
            .unwrap()
            .clone()
    };
    let riga = |codice: &str, espressione: &str| {
        let mut x = RigaAnalisi::da_risorsa(&risorsa(codice), Quantita::ZERO);
        x.espressione = Some(espressione.into());
        x
    };
    let mut a = analisi(
        AnalisiId::da_u128(22),
        "INT",
        "m²",
        vec![
            riga("MO.02", "0,02+spessore*10"),
            riga("MA.01", "kg/100"),
            riga("NO.01", "q1/2"),
        ],
    );
    a.variabili = vec![
        VariabileAnalisi {
            nome: "spessore".into(),
            espressione: "0,015".into(),
            descrizione: "spessore dell'intonaco (m)".into(),
        },
        VariabileAnalisi {
            nome: "kg".into(),
            espressione: "spessore*1400".into(),
            descrizione: String::new(),
        },
    ];
    a.quota_oneri_sicurezza = Some(Percentuale::new(dec!(20)));
    let quantita: Vec<Decimal> = a
        .quantita_righe()
        .unwrap()
        .iter()
        .map(|q| q.valore())
        .collect();
    assert_eq!(quantita, vec![dec!(0.17), dec!(0.21), dec!(0.085)]);
    p.analisi.push(a.clone());
    let x = p.calcola_analisi(a.id, &r).unwrap();
    assert_eq!(x.costo_diretto, imp(dec!(8.93)));
    assert_eq!(x.spese_generali, imp(dec!(1.34)));
    assert_eq!(x.utile, imp(dec!(1.03)));
    assert_eq!(x.prezzo_unitario, Prezzo::new(dec!(11.30)));
    assert_eq!(x.oneri_sicurezza_aziendali, Some(imp(dec!(0.27))));
    assert_eq!(x.variabili[1].valore, dec!(21.000));
    assert_eq!(x.righe[2].quantita, Quantita::new(dec!(0.085)));

    // Nomi riservati e variabili sconosciute.
    let mut b = a.clone();
    b.variabili[0].nome = "q1".into();
    assert!(
        b.quantita_righe()
            .unwrap_err()
            .to_string()
            .contains("non valido")
    );
    let mut b = a.clone();
    b.righe[0].espressione = Some("2*altezza".into());
    assert!(
        b.quantita_righe()
            .unwrap_err()
            .to_string()
            .contains("altezza")
    );
    let mut b = a;
    b.righe[0].espressione = Some("q2".into());
    assert!(
        b.quantita_righe().is_err(),
        "una riga usa solo le righe precedenti"
    );
}

/// Muratura 100 m² nel computo: malta 3 m³ × 180,90 = 542,70; mattoni 6.000 × 0,45 =
/// 2.700,00; operaio qualificato 80 h × 35,50 = 2.840,00; operaio comune 80 h × 32,00 =
/// 2.560,00; autocarro 2 h × 50,00 = 100,00 (totale 8.742,70); spese generali 13,11 × 100
/// = 1.311,00; utile 10,05 × 100 = 1.005,00; importo della voce 100 × 110,59 = 11.059,00;
/// differenza di arrotondamento 0,30.
#[test]
fn analisi_riportata_alle_quantita_del_computo() {
    let mut p = progetto();
    let mut c = Computo::new(ComputoId::da_u128(50), "Computo");
    let e = &p.elenco_prezzi;
    let mut voce = VoceComputo::new(
        VoceComputoId::da_u128(51),
        PrezzoApplicato::da_voce(e, e.voce(MUR).unwrap()),
    );
    voce.righi
        .push(RigoMisura::misura(RigoMisuraId::da_u128(510), "").con_lunghezza(dec!(100)));
    let mut semplice = VoceComputo::new(
        VoceComputoId::da_u128(52),
        PrezzoApplicato::da_voce(e, e.voce(MATTONI).unwrap()),
    );
    semplice
        .righi
        .push(RigoMisura::misura(RigoMisuraId::da_u128(520), "").con_lunghezza(dec!(10)));
    c.voci = vec![Arc::new(voce), Arc::new(semplice)];
    p.computi.push(c);
    let a = p.analisi_computo(ComputoId::da_u128(50)).unwrap();
    let importi: Vec<Decimal> = a[0].elementi.iter().map(|x| x.importo.valore()).collect();
    assert_eq!(
        importi,
        vec![
            dec!(542.70),
            dec!(2700.00),
            dec!(2840.00),
            dec!(2560.00),
            dec!(100.00)
        ]
    );
    assert_eq!(a[0].elementi[0].quantita, Quantita::new(dec!(3.00)));
    assert_eq!(a[0].spese_generali, imp(dec!(1311.00)));
    assert_eq!(a[0].utile, imp(dec!(1005.00)));
    assert_eq!(a[0].differenza, imp(dec!(0.30)));
    assert!(!a[1].analizzata);
}
