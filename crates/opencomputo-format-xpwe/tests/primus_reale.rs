//! Confronto con un export reale di PriMus-DCF (formato 5.01, documento vuoto).
//!
//! Fixture: `tests/fixtures/primus_dcf_5_01_vuoto.xpwe`, identica al file esportato
//! tranne il percorso in `FileNameDocumento`, sostituito con uno generico.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::prelude::*;
use opencomputo_format_xpwe::xml::{Nodo, analizza};
use opencomputo_format_xpwe::{OpzioniImport, esporta, importa};
use rust_decimal_macros::dec;
use std::sync::Arc;

const REALE: &[u8] = include_bytes!("fixtures/primus_dcf_5_01_vuoto.xpwe");

fn importa_reale() -> opencomputo_format_xpwe::RisultatoImport {
    importa(
        REALE,
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
    .unwrap()
}

/// Albero senza testi di soli spazi (PriMus scrive i campi vuoti come «a capo + rientro»).
fn normalizza(mut n: Nodo) -> Nodo {
    if n.testo.trim().is_empty() {
        n.testo.clear();
    }
    n.riga = 0;
    n.figli = n.figli.into_iter().map(normalizza).collect();
    n
}

fn nomi_figli(n: &Nodo) -> Vec<&str> {
    n.figli.iter().map(|f| f.nome.as_str()).collect()
}

#[test]
fn import_senza_anomalie() {
    let r = importa_reale();
    assert!(r.rapporto.anomalie.is_empty(), "{:#?}", r.rapporto.anomalie);
    assert_eq!(r.rapporto.codifica, "UTF-8");
    assert_eq!(r.progetto.metadati.titolo, "Nuovo lavoro");
    assert_eq!(r.progetto.metadati.luogo, None);
    let regole = &r.progetto.regole;
    assert_eq!(
        (
            regole.decimali_quantita,
            regole.decimali_prezzo,
            regole.decimali_importo
        ),
        (2, 2, 2)
    );
    assert!(r.progetto.elenco_prezzi.voci.is_empty());
    assert!(r.progetto.computi[0].voci.is_empty());
}

#[test]
fn reimportato_ed_esportato_ha_la_stessa_struttura() {
    let r = importa_reale();
    let xml = esporta(&r.progetto, r.computo).unwrap().xml;
    let originale = normalizza(analizza(REALE).unwrap().radice);
    let esportato = normalizza(analizza(xml.as_bytes()).unwrap().radice);
    assert_eq!(esportato, originale, "{xml}");
    assert_eq!(
        xml.matches("\r\n").count(),
        xml.matches('\n').count(),
        "fine riga CRLF"
    );
}

#[test]
fn documento_nuovo_con_la_struttura_osservata() {
    let reale = analizza(REALE).unwrap().radice;
    let mut g = GeneratoreSequenziale::new(500);
    let mut p = Progetto::new(g.nuovo(), "Lavoro nuovo", g.nuovo());
    p.regole.decimali_quantita = 3;
    let c = Computo::new(g.nuovo(), "Computo");
    let id = c.id;
    p.aggiungi_computo(c).unwrap();
    let nuovo = analizza(esporta(&p, id).unwrap().xml.as_bytes())
        .unwrap()
        .radice;

    // Testata e sezioni nello stesso ordine.
    let attesi: Vec<&str> = nomi_figli(&reale)
        .into_iter()
        .filter(|n| *n != "FileNameDocumento")
        .collect();
    assert_eq!(nomi_figli(&nuovo), attesi);
    for campo in ["TipoDocumento", "TipoFormato", "Versione"] {
        assert_eq!(nuovo.testo_di(campo), reale.testo_di(campo), "{campo}");
    }
    assert_eq!(nuovo.testo_di("SourceNome"), Some("OpenComputo"));

    let dg = |n: &'_ Nodo| n.figlio("PweDatiGenerali").unwrap().clone();
    let anag = |n: &Nodo| {
        dg(n)
            .figlio("PweDGProgetto")
            .unwrap()
            .figlio("PweDGDatiGenerali")
            .unwrap()
            .clone()
    };
    assert_eq!(nomi_figli(&anag(&nuovo)), nomi_figli(&anag(&reale)));
    assert_eq!(anag(&nuovo).testo_di("Oggetto"), Some("Lavoro nuovo"));
    let numeri = |n: &Nodo| {
        dg(n)
            .figlio("PweDGConfigurazione")
            .unwrap()
            .figlio("PweDGConfigNumeri")
            .unwrap()
            .clone()
    };
    assert_eq!(nomi_figli(&numeri(&nuovo)), nomi_figli(&numeri(&reale)));
    assert_eq!(numeri(&nuovo).testo_di("Quantita"), Some("11.3|0"));
    assert_eq!(
        numeri(&nuovo).testo_di("Prezzi"),
        numeri(&reale).testo_di("Prezzi")
    );

    // Elenco prezzi vuoto omesso, come in PriMus.
    assert_eq!(
        nomi_figli(nuovo.figlio("PweMisurazioni").unwrap()),
        ["PweVociComputo"]
    );
}

#[test]
fn voci_aggiunte_a_un_documento_reale_vuoto() {
    let mut r = importa_reale();
    let mut g = GeneratoreSequenziale::new(900);
    let v = VocePrezzario::new(g.nuovo(), "A.1", "Scavo", "m³", Prezzo::new(dec!(8.50)));
    r.progetto.elenco_prezzi.aggiungi_voce(v.clone()).unwrap();
    let mut vc = VoceComputo::new(
        g.nuovo(),
        PrezzoApplicato::da_voce(&r.progetto.elenco_prezzi, &v),
    );
    vc.righi.push(
        RigoMisura::misura(g.nuovo(), "Platea")
            .con_lunghezza(dec!(10))
            .con_larghezza(dec!(2)),
    );
    r.progetto.computi[0].voci.push(Arc::new(vc));

    let xml = esporta(&r.progetto, r.computo).unwrap().xml;
    let radice = analizza(xml.as_bytes()).unwrap().radice;
    // L'elenco, assente nel file di origine, è scritto prima delle voci di computo.
    assert_eq!(
        nomi_figli(radice.figlio("PweMisurazioni").unwrap()),
        ["PweElencoPrezzi", "PweVociComputo"]
    );
    let di_nuovo = importa(
        xml.as_bytes(),
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
    .unwrap();
    assert!(
        !di_nuovo.rapporto.ha_errori(),
        "{:#?}",
        di_nuovo.rapporto.anomalie
    );
    let totale = di_nuovo
        .progetto
        .calcola_computo(di_nuovo.computo)
        .unwrap()
        .totale;
    assert_eq!(totale, Importo::new(dec!(170.00)));
}
