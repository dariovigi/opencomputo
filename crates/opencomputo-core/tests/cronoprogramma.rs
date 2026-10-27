//! Cronoprogramma (ADR 0024): calendario, percorso critico, livelli della WBS,
//! produzione mensile e linea di base, con date verificate a mano sul calendario 2026.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::cronoprogramma::{
    AttivitaAggiunta, Calendario, Chiusura, DatiAttivita, ErroreCronoprogramma, Legame, Piano,
    RifAttivita, TipoLegame, calcola, festivita_nazionale, linea_base, pasqua, verifica_legami,
};
use opencomputo_core::prelude::*;
use rust_decimal_macros::dec;

fn d(a: i32, m: u32, g: u32) -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(a, m, g).unwrap()
}

const FINE_LAVORI: AttivitaId = AttivitaId::da_u128(8001);

fn durata(n: u32) -> DatiAttivita {
    DatiAttivita {
        durata: Some(n),
        ..DatiAttivita::default()
    }
}

fn legame(da: RifAttivita, a: RifAttivita, tipo: TipoLegame, ritardo: i32) -> Legame {
    Legame {
        da,
        a,
        tipo,
        ritardo,
    }
}

const FOND: RifAttivita = RifAttivita::Wbs(WBS_FONDAZIONI);
const ELEV: RifAttivita = RifAttivita::Wbs(WBS_ELEVAZIONI);
const SIC: RifAttivita = RifAttivita::Wbs(WBS_SICUREZZA);
const EDIF: RifAttivita = RifAttivita::Wbs(WBS_EDIFICIO);
const FINE: RifAttivita = RifAttivita::Attivita(FINE_LAVORI);

/// Lavori dal 2 marzo 2026 (lunedì): fondazioni 10 gg, poi elevazioni 15 gg;
/// sicurezza 20 gg insieme alle fondazioni; pietra miliare a fine lavori.
fn progetto() -> Progetto {
    let mut p = progetto_di_esempio();
    let c = &mut p.cronoprogramma;
    c.inizio_lavori = Some(d(2026, 3, 2));
    c.imposta_dati_wbs(WBS_FONDAZIONI, durata(10));
    c.imposta_dati_wbs(WBS_ELEVAZIONI, durata(15));
    c.imposta_dati_wbs(WBS_SICUREZZA, durata(20));
    c.imposta_dati_wbs(WBS_EDIFICIO, durata(30));
    c.attivita.push(AttivitaAggiunta {
        id: FINE_LAVORI,
        nome: "Fine dei lavori".into(),
        padre: None,
        pietra_miliare: true,
        dati: DatiAttivita::default(),
    });
    c.legami = vec![
        legame(FOND, ELEV, TipoLegame::FineInizio, 0),
        legame(FOND, SIC, TipoLegame::InizioInizio, 0),
        legame(ELEV, FINE, TipoLegame::FineInizio, 0),
    ];
    p
}

fn riga(piano: &Piano, rif: RifAttivita) -> &opencomputo_core::cronoprogramma::RigaPiano {
    piano.righe.iter().find(|r| r.attivita == rif).unwrap()
}

#[test]
fn calendario_con_festivita_nazionali_e_chiusure() {
    assert_eq!(pasqua(2026), Some(d(2026, 4, 5)));
    assert_eq!(pasqua(2027), Some(d(2027, 3, 28)));
    assert_eq!(
        festivita_nazionale(d(2026, 4, 6)),
        Some("Lunedì dell'Angelo")
    );
    assert_eq!(
        festivita_nazionale(d(2026, 6, 2)),
        Some("Festa della Repubblica")
    );
    assert_eq!(festivita_nazionale(d(2026, 6, 3)), None);
    let mut c = Calendario::default();
    assert!(c.lavorativo(d(2026, 3, 2)));
    assert!(!c.lavorativo(d(2026, 3, 7)), "sabato");
    assert!(!c.lavorativo(d(2026, 4, 6)), "Pasquetta");
    c.chiusure.push(Chiusura {
        dal: d(2026, 8, 10),
        al: d(2026, 8, 21),
        descrizione: "Ferie".into(),
    });
    assert!(!c.lavorativo(d(2026, 8, 17)));
    c.festivita_nazionali = false;
    assert!(c.lavorativo(d(2026, 4, 6)));
}

#[test]
fn percorso_critico_date_e_margini() {
    let p = progetto();
    let piano = calcola(&p, None).unwrap();
    // Ordine della WBS: 01, 01.01, 01.02, 02, poi la pietra miliare alla radice.
    let ordine: Vec<_> = piano.righe.iter().map(|r| r.attivita).collect();
    assert_eq!(ordine, vec![EDIF, FOND, ELEV, SIC, FINE]);

    let f = riga(&piano, FOND);
    assert_eq!(
        (f.inizio, f.fine, f.durata),
        (d(2026, 3, 2), d(2026, 3, 13), 10)
    );
    assert!(f.critica && f.margine == 0);
    // Elevazioni: dal 16 marzo, 15 giorni lavorativi fino a venerdì 3 aprile.
    let e = riga(&piano, ELEV);
    assert_eq!((e.inizio, e.fine), (d(2026, 3, 16), d(2026, 4, 3)));
    assert_eq!(e.predecessori.len(), 1);
    assert_eq!(e.predecessori[0].riga, 1);
    // Sicurezza: inizia con le fondazioni, 20 giorni fino al 27 marzo; margine 5.
    let s = riga(&piano, SIC);
    assert_eq!((s.inizio, s.fine), (d(2026, 3, 2), d(2026, 3, 27)));
    assert_eq!(s.margine, 5);
    assert!(!s.critica);
    assert_eq!(
        (s.inizio_tardi, s.fine_tardi),
        (d(2026, 3, 9), d(2026, 4, 3))
    );
    // Riepilogo: dal primo inizio all'ultima fine dei figli; la sua durata propria (30)
    // non conta finché i figli sono visibili.
    let ed = riga(&piano, EDIF);
    assert!(ed.riepilogo);
    assert_eq!(
        (ed.inizio, ed.fine, ed.durata),
        (d(2026, 3, 2), d(2026, 4, 3), 25)
    );
    assert_eq!(ed.importo, Some(Importo::new(dec!(13257.12))));
    // Pietra miliare alla fine delle elevazioni (il 6 aprile è Pasquetta).
    let fine = riga(&piano, FINE);
    assert!(fine.pietra_miliare);
    assert_eq!((fine.inizio, fine.fine), (d(2026, 4, 3), d(2026, 4, 3)));
    assert_eq!(
        (piano.inizio, piano.fine, piano.durata),
        (d(2026, 3, 2), d(2026, 4, 3), 25)
    );
    assert_eq!(piano.durata_naturale, 33);
    // Sabati e domeniche tra inizio e fine: 9 marzo 7-8, 14-15, 21-22, 28-29 → 8 giorni.
    assert_eq!(piano.giorni_non_lavorativi.len(), 8);
}

#[test]
fn legami_fine_fine_inizio_fine_ritardi_e_vincoli() {
    let mut p = progetto();
    let c = &mut p.cronoprogramma;
    // Elevazioni finiscono 2 giorni dopo la fine della sicurezza (FF+2).
    c.legami = vec![legame(SIC, ELEV, TipoLegame::FineFine, 2)];
    // Sicurezza non prima del 9 marzo.
    let mut dati = durata(20);
    dati.inizio_non_prima = Some(d(2026, 3, 9));
    c.imposta_dati_wbs(WBS_SICUREZZA, dati);
    let piano = calcola(&p, None).unwrap();
    let s = riga(&piano, SIC);
    // Dal 9 marzo, 20 giorni: 9-13, 16-20, 23-27, 30 marzo-3 aprile.
    assert_eq!((s.inizio, s.fine), (d(2026, 3, 9), d(2026, 4, 3)));
    // Elevazioni: fine ≥ fine sicurezza + 2 → fine all'indice 26 (mercoledì 8 aprile,
    // dopo Pasquetta), inizio 15 giorni prima.
    let e = riga(&piano, ELEV);
    assert_eq!(e.fine, d(2026, 4, 8));
    assert_eq!(e.inizio, d(2026, 3, 18));
    // Inizio-fine: la sicurezza finisce non prima di 25 giorni dopo l'inizio delle
    // fondazioni.
    let mut p2 = progetto();
    p2.cronoprogramma.legami = vec![legame(FOND, SIC, TipoLegame::InizioFine, 25)];
    let piano = calcola(&p2, None).unwrap();
    // Fine sicurezza ≥ inizio fondazioni (0) + 25 → fine all'indice 24 (3 aprile).
    assert_eq!(riga(&piano, SIC).fine, d(2026, 4, 3));
}

#[test]
fn livello_di_dettaglio_della_wbs() {
    let p = progetto();
    let piano = calcola(&p, Some(Some(1))).unwrap();
    // Solo i nodi radice e la pietra miliare: l'edificio diventa un'attività di 30 gg.
    let ordine: Vec<_> = piano.righe.iter().map(|r| r.attivita).collect();
    assert_eq!(ordine, vec![EDIF, SIC, FINE]);
    let ed = riga(&piano, EDIF);
    assert!(!ed.riepilogo);
    assert_eq!(
        (ed.inizio, ed.fine, ed.durata),
        (d(2026, 3, 2), d(2026, 4, 13), 30)
    );
    // I legami dei figli passano al nodo visibile: sicurezza II edificio, fine lavori
    // FI edificio; il legame tra fondazioni ed elevazioni resta dentro l'edificio.
    assert_eq!(riga(&piano, SIC).inizio, d(2026, 3, 2));
    assert_eq!(riga(&piano, FINE).fine, d(2026, 4, 13));
    assert_eq!(piano.livelli_wbs, 2);
    assert_eq!(piano.livello_dettaglio, Some(1));
}

#[test]
fn a_livello_ridotto_senza_durata_propria_vale_il_riepilogo() {
    let mut p = progetto();
    let mut dati = p.cronoprogramma.dati_wbs(WBS_EDIFICIO);
    dati.durata = None;
    p.cronoprogramma.imposta_dati_wbs(WBS_EDIFICIO, dati);
    // Nel piano completo l'edificio va dal 2 marzo al 3 aprile: 25 giorni lavorativi.
    let completo = calcola(&p, Some(None)).unwrap();
    assert_eq!(riga(&completo, EDIF).durata, 25);
    let piano = calcola(&p, Some(Some(1))).unwrap();
    let ed = riga(&piano, EDIF);
    assert!(!ed.riepilogo && !ed.durata_stimata);
    assert_eq!(
        (ed.inizio, ed.fine, ed.durata),
        (d(2026, 3, 2), d(2026, 4, 3), 25)
    );
}

#[test]
fn produzione_mensile_con_somme_esatte() {
    let p = progetto();
    let piano = calcola(&p, None).unwrap();
    assert_eq!(piano.computo.as_deref(), Some("Computo metrico estimativo"));
    assert_eq!(piano.importo_pianificato, Importo::new(dec!(14457.12)));
    assert_eq!(piano.importo_non_assegnato, Importo::ZERO);
    assert_eq!(piano.mesi.len(), 2);
    // Marzo: fondazioni 8 588,82 + sicurezza 1 200,00 + elevazioni 12/15 di 4 668,30.
    let marzo = &piano.mesi[0];
    assert_eq!((marzo.anno, marzo.mese), (2026, 3));
    assert_eq!(marzo.importo, Importo::new(dec!(13523.46)));
    assert_eq!(marzo.percentuale, dec!(93.54));
    let aprile = &piano.mesi[1];
    assert_eq!(aprile.importo, Importo::new(dec!(933.66)));
    assert_eq!(aprile.cumulato, Importo::new(dec!(14457.12)));
    assert_eq!(aprile.percentuale, dec!(100.00));
}

#[test]
fn linea_di_base_e_scostamenti() {
    let mut p = progetto();
    let piano = calcola(&p, None).unwrap();
    p.cronoprogramma.linea_base = Some(linea_base(&piano, d(2026, 2, 20)));
    // Le fondazioni si allungano di 3 giorni: tutto slitta tranne la sicurezza.
    p.cronoprogramma
        .imposta_dati_wbs(WBS_FONDAZIONI, durata(13));
    let piano = calcola(&p, None).unwrap();
    let e = riga(&piano, ELEV);
    assert_eq!(e.fine_base, Some(d(2026, 4, 3)));
    assert_eq!(e.fine, d(2026, 4, 9));
    assert_eq!(e.scostamento, Some(3));
    assert_eq!(riga(&piano, SIC).scostamento, Some(0));
}

#[test]
fn cicli_inizio_mancante_e_durate_stimate() {
    let mut p = progetto();
    p.cronoprogramma
        .legami
        .push(legame(ELEV, FOND, TipoLegame::FineInizio, 0));
    assert!(matches!(
        calcola(&p, None),
        Err(ErroreCronoprogramma::Ciclo(_))
    ));
    assert!(verifica_legami(&p).is_err());
    assert!(verifica_legami(&progetto()).is_ok());

    let mut p = progetto();
    p.cronoprogramma.inizio_lavori = None;
    assert_eq!(calcola(&p, None), Err(ErroreCronoprogramma::InizioMancante));
    // Con la consegna dei lavori nel contratto l'inizio è quello.
    let mut p = progetto_con_contabilita();
    p.cronoprogramma.inizio_lavori = None;
    let consegna = p.contratto.as_ref().and_then(|c| c.data_consegna_lavori);
    if let Some(consegna) = consegna {
        let piano = calcola(&p, None).unwrap();
        assert!(piano.inizio >= consegna);
    }
    // Attività senza durata: 1 giorno, segnalata come stimata.
    let mut p = progetto();
    p.cronoprogramma.attivita_wbs.clear();
    let piano = calcola(&p, None).unwrap();
    assert!(riga(&piano, FOND).durata_stimata);
    assert_eq!(riga(&piano, FOND).durata, 1);
    // Il cronoprogramma vuoto non si salva nel file.
    let json = opencomputo_core::schema::esporta_json(&progetto_di_esempio()).unwrap();
    assert!(!json.contains("cronoprogramma"));
    let json = opencomputo_core::schema::esporta_json(&progetto()).unwrap();
    assert!(json.contains("cronoprogramma"));
    let riletto = opencomputo_core::schema::importa_json(&json).unwrap();
    assert_eq!(riletto.cronoprogramma, progetto().cronoprogramma);
}
