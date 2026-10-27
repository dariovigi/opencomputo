//! Test di integrazione: validazione del progetto.

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
use opencomputo_core::prelude::*;
use rust_decimal_macros::dec;

fn codici(p: &Progetto) -> Vec<String> {
    valida_progetto(p)
        .problemi
        .into_iter()
        .map(|x| x.codice)
        .collect()
}

#[test]
fn il_progetto_di_esempio_e_valido() {
    let p = progetto_di_esempio();
    let r = valida_progetto(&p);
    assert!(r.problemi.is_empty(), "{:#?}", r.problemi);
}

#[test]
fn problemi_del_prezzario() {
    let mut p = progetto_di_esempio();
    let voci = &mut p.elenco_prezzi.voci;
    voci[0].codice = voci[1].codice.clone();
    voci[1].unita_misura = " ".into();
    voci[2].incidenza_manodopera = Some(Percentuale::new(dec!(120)));
    voci[3].prezzo = Prezzo::new(dec!(-1));
    let c = codici(&p);
    for atteso in [
        "prezzario.codice_duplicato",
        "prezzario.unita_misura_vuota",
        "percentuale_fuori_intervallo",
        "prezzario.prezzo_negativo",
    ] {
        assert!(c.iter().any(|x| x == atteso), "manca {atteso} in {c:?}");
    }
}

#[test]
fn problemi_delle_misure() {
    let mut p = progetto_di_esempio();
    let voce = p.computi[0].voce_mut(VC_SCAVO).unwrap();
    voce.righi.push(RigoMisura::misura(
        RigoMisuraId::da_u128(6001),
        "da completare",
    ));
    voce.righi.push(
        RigoMisura::misura(RigoMisuraId::da_u128(6002), "doppio segno")
            .con_lunghezza(dec!(-2))
            .come_deduzione(),
    );
    voce.righi.push(
        RigoMisura::misura(RigoMisuraId::da_u128(6003), "formula errata").con_formula("lun*"),
    );
    p.computi[0].voce_mut(VC_CLS).unwrap().righi.clear();
    let c = codici(&p);
    for atteso in [
        "misura.rigo_vuoto",
        "misura.deduzione_positiva",
        "calcolo.errore",
        "misura.nessun_rigo",
    ] {
        assert!(c.iter().any(|x| x == atteso), "manca {atteso} in {c:?}");
    }
    let r = valida_progetto(&p);
    let errore = r.con_codice("calcolo.errore").next().unwrap();
    assert_eq!(errore.percorso.len(), 3, "computo › voce › rigo");
}

#[test]
fn riferimenti_inesistenti() {
    let mut p = progetto_di_esempio();
    let voce = p.computi[0].voce_mut(VC_SCAVO).unwrap();
    voce.categorie.categoria = Some(ClasseId::da_u128(999));
    voce.wbs = Some(WbsNodoId::da_u128(999));
    p.wbs.nodi[1].padre = Some(WbsNodoId::da_u128(12345));
    let c = codici(&p);
    for atteso in [
        "categorie.riferimento_inesistente",
        "wbs.riferimento_inesistente",
        "wbs.non_valida",
    ] {
        assert!(c.iter().any(|x| x == atteso), "manca {atteso} in {c:?}");
    }
}

#[test]
fn contratto_e_regole_non_validi() {
    let mut p = progetto_con_contabilita();
    p.contratto.as_mut().unwrap().parametri.ribasso = Percentuale::new(dec!(100));
    p.regole.decimali_importo = 40;
    let c = codici(&p);
    assert!(c.iter().any(|x| x == "contratto.percentuale_non_valida"));
    assert!(c.iter().any(|x| x == "regole.non_valide"));
}

#[test]
fn incidenza_manodopera_mancante_con_esclusione_dal_ribasso() {
    let mut p = progetto_con_contabilita();
    p.contratto
        .as_mut()
        .unwrap()
        .parametri
        .manodopera_esclusa_dal_ribasso = true;
    // La recinzione (registrazione 3) è sicurezza: non conta. Si toglie
    // l'incidenza allo scavo (registrazione 1).
    assert_eq!(
        valida_progetto(&p)
            .con_codice("contabilita.incidenza_manodopera_mancante")
            .count(),
        0
    );
    p.contabilita
        .modifica_registrazione(RegistrazioneId::da_u128(1), |r| {
            r.prezzo.incidenza_manodopera = None
        })
        .unwrap();
    let r = valida_progetto(&p);
    let avvisi: Vec<_> = r
        .con_codice("contabilita.incidenza_manodopera_mancante")
        .collect();
    assert_eq!(avvisi.len(), 1);
    assert_eq!(avvisi[0].gravita, Gravita::Avviso);
}

/// Regressione (campagna di test del 2026-10-22): capitoli con padre inesistente o in
/// ciclo passavano la validazione senza segnalazioni.
#[test]
fn capitoli_con_padre_inesistente_o_in_ciclo() {
    use opencomputo_core::id::CapitoloId;
    use opencomputo_core::prezzario::CapitoloPrezzario;
    let mut p = progetto_di_esempio();
    let c = |n: u128, padre: Option<u128>| CapitoloPrezzario {
        id: CapitoloId::da_u128(n),
        padre: padre.map(CapitoloId::da_u128),
        codice: format!("C{n}"),
        descrizione: String::new(),
    };
    p.elenco_prezzi
        .capitoli
        .extend([c(1, Some(2)), c(2, Some(1)), c(3, Some(99)), c(4, Some(4))]);
    let r = valida_progetto(&p);
    assert_eq!(r.con_codice("prezzario.capitoli_in_ciclo").count(), 3);
    assert_eq!(
        r.con_codice("prezzario.capitolo_padre_inesistente").count(),
        1
    );
    assert!(!r.ha_errori());
}
