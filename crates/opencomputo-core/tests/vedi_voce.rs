//! Righi «vedi voce»: collegamenti tra voci dello stesso computo, ordine di calcolo,
//! riferimenti circolari, formula con `vv`, quantità statiche.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::errore::ErroreCalcolo;
use opencomputo_core::misura::{QuantitaRiferite, VediVoce};
use opencomputo_core::prelude::*;
use opencomputo_core::riferimento::Riferimento;
use opencomputo_core::validazione::valida_progetto;
use rust_decimal_macros::dec;

fn vedi(id: u128, voce: VoceComputoId) -> RigoMisura {
    RigoMisura::misura(RigoMisuraId::da_u128(id), "").con_vedi_voce(VediVoce::Collegata(voce))
}

fn quantita(p: &Progetto, voce: VoceComputoId) -> Decimal {
    p.computo(COMPUTO)
        .unwrap()
        .quantita_voce(voce, &p.regole)
        .unwrap()
        .valore()
}

fn righi(p: &mut Progetto, voce: VoceComputoId) -> &mut Vec<RigoMisura> {
    &mut p
        .computo_mut(COMPUTO)
        .unwrap()
        .voce_mut(voce)
        .unwrap()
        .righi
}

#[test]
fn la_quantita_segue_la_voce_citata_anche_piu_avanti() {
    let mut p = progetto_di_esempio();
    // La voce 1 (scavo, 126,00) cita la recinzione (voce 4, 100,00) per 2 e, in
    // deduzione, la muratura (voce 3), che a sua volta cita il calcestruzzo (voce 2).
    righi(&mut p, VC_MURATURA).push(vedi(9001, VC_CLS)); // 109,20 + 51,74 = 160,94
    righi(&mut p, VC_SCAVO).push(vedi(9002, VC_RECINZIONE).con_parti(dec!(2)));
    righi(&mut p, VC_SCAVO).push(vedi(9003, VC_MURATURA).come_deduzione());
    assert_eq!(quantita(&p, VC_MURATURA), dec!(160.94));
    // 126,00 + 200,00 − 160,94
    assert_eq!(quantita(&p, VC_SCAVO), dec!(165.06));
    // Cambia la voce citata: cambia chi la cita.
    righi(&mut p, VC_RECINZIONE)[0].lunghezza = Some(Fattore::Espressione("2*(30+25)".into()));
    assert_eq!(quantita(&p, VC_SCAVO), dec!(185.06));
    let r = p.computo(COMPUTO).unwrap().calcola(&p.regole).unwrap();
    assert_eq!(r.voci[0].calcolo.quantita, Quantita::new(dec!(185.06)));
    assert!(!valida_progetto(&p).ha_errori());
}

#[test]
fn formula_con_la_quantita_citata() {
    let mut p = progetto_di_esempio();
    righi(&mut p, VC_RECINZIONE).push(
        vedi(9001, VC_SCAVO)
            .con_lunghezza(dec!(3))
            .con_formula("vv/lun"),
    );
    // 100,00 + 126,00 / 3
    assert_eq!(quantita(&p, VC_RECINZIONE), dec!(142.00));
    // `vv` senza «vedi voce» è una variabile non valorizzata.
    righi(&mut p, VC_RECINZIONE)[1].vedi_voce = None;
    let e = p
        .computo(COMPUTO)
        .unwrap()
        .quantita_voce(VC_RECINZIONE, &p.regole)
        .unwrap_err();
    assert!(e.to_string().contains("vv"), "{e}");
}

#[test]
fn statico_e_fuori_dal_computo() {
    let rigo = RigoMisura::misura(RigoMisuraId::da_u128(1), "")
        .con_vedi_voce(VediVoce::Statica {
            quantita: dec!(12.345),
            riferimento: "voce n. 3".into(),
        })
        .con_parti(dec!(2));
    let regole = RegoleArrotondamento::default();
    assert_eq!(
        rigo.calcola(&regole).unwrap().unwrap().quantita,
        Quantita::new(dec!(24.69))
    );
    // Un collegamento fuori da un computo non si calcola.
    let collegato = vedi(2, VC_SCAVO);
    assert!(matches!(
        collegato.calcola(&regole),
        Err(ErroreCalcolo::VediVoce(_))
    ));
    // Un commento con «vedi voce» resta un commento.
    let mut commento = vedi(3, VC_SCAVO);
    commento.tipo = TipoRigo::Commento;
    assert_eq!(commento.calcola(&regole).unwrap(), None);
}

#[test]
fn riferimenti_circolari_rifiutati_e_localizzati() {
    let mut p = progetto_di_esempio();
    // 1 → 2 → 1, e la 3 dipende dal ciclo; la 4 resta calcolabile.
    righi(&mut p, VC_SCAVO).push(vedi(9001, VC_CLS));
    righi(&mut p, VC_CLS).push(vedi(9002, VC_SCAVO));
    righi(&mut p, VC_MURATURA).push(vedi(9003, VC_SCAVO));
    let c = p.computo(COMPUTO).unwrap();
    let q = c.quantita_riferite(&p.regole);
    let e = q.di(VC_SCAVO).unwrap().as_ref().unwrap_err();
    assert_eq!(
        e.percorso,
        vec![Riferimento::Rigo {
            numero: 3,
            id: RigoMisuraId::da_u128(9001)
        }]
    );
    assert!(e.to_string().contains("circolare"), "{e}");
    assert!(q.di(VC_CLS).unwrap().is_err());
    assert!(q.di(VC_MURATURA).unwrap().is_err());
    assert_eq!(q.quantita(VC_RECINZIONE).unwrap(), dec!(100.00));
    assert!(c.calcola(&p.regole).is_err());
    let rapporto = valida_progetto(&p);
    assert_eq!(rapporto.con_codice("calcolo.errore").count(), 3);

    // Una voce che cita sé stessa.
    let mut p = progetto_di_esempio();
    righi(&mut p, VC_RECINZIONE).push(vedi(9004, VC_RECINZIONE));
    let e = p
        .computo(COMPUTO)
        .unwrap()
        .quantita_voce(VC_RECINZIONE, &p.regole)
        .unwrap_err();
    assert!(e.to_string().contains("circolare con la voce n. 4"), "{e}");
}

#[test]
fn voce_citata_assente_o_non_calcolabile() {
    let mut p = progetto_di_esempio();
    righi(&mut p, VC_SCAVO).push(vedi(9001, VoceComputoId::da_u128(99_999)));
    let e = p
        .computo(COMPUTO)
        .unwrap()
        .quantita_voce(VC_SCAVO, &p.regole)
        .unwrap_err();
    assert!(e.to_string().contains("non è in questo computo"), "{e}");

    let mut p = progetto_di_esempio();
    righi(&mut p, VC_CLS)[0].lunghezza = Some(Fattore::Espressione("1/0".into()));
    righi(&mut p, VC_SCAVO).push(vedi(9002, VC_CLS));
    let e = p
        .computo(COMPUTO)
        .unwrap()
        .quantita_voce(VC_SCAVO, &p.regole)
        .unwrap_err();
    assert!(
        e.to_string().contains("voce citata n. 2 non è calcolabile"),
        "{e}"
    );
}

#[test]
fn nel_libretto_solo_statico() {
    let mut p = progetto_con_contabilita();
    let mut r = registrazione(&p, 50, 5, data(2026, 5, 2), SCAVO, &[]);
    r.righi = vec![vedi(501, VC_SCAVO)];
    p.contabilita.registra(r).unwrap();
    let rapporto = valida_progetto(&p);
    assert_eq!(
        rapporto
            .con_codice("contabilita.vedi_voce_collegato")
            .count(),
        1
    );
}

/// Regressione (campagna di test del 2026-10-22): il fabbisogno calcolava le voci da
/// sole e falliva su ogni «vedi voce» collegato.
#[test]
fn fabbisogno_con_vedi_voce() {
    let mut p = progetto_con_nuovi_prezzi();
    let prima = p.fabbisogno(COMPUTO).unwrap();
    righi(&mut p, VC_RECINZIONE).push(vedi(9001, VC_SCAVO));
    let dopo = p.fabbisogno(COMPUTO).unwrap();
    assert_eq!(dopo.voci_senza_analisi, prima.voci_senza_analisi);
}
