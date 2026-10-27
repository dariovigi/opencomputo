//! Andata e ritorno: XPWE → modello → XPWE → modello, e modello nativo → XPWE → modello.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::prelude::*;
use opencomputo_format_xpwe::{OpzioniImport, esporta, importa};
use rust_decimal_macros::dec;

#[test]
fn xpwe_modello_xpwe_senza_perdite() {
    let prima = importa_da(FIXTURE);
    let export = esporta(&prima.progetto, prima.computo).unwrap();
    let seconda = importa_da(export.xml.as_bytes());
    assert!(
        !seconda.rapporto.ha_errori(),
        "{:#?}",
        seconda.rapporto.anomalie
    );
    // Stesso modello, inclusi i dati originali conservati; cambiano solo impronta
    // e posizioni delle anomalie (il file riscritto ha un'altra impaginazione).
    assert_eq!(
        senza_dati_del_file(seconda.progetto.clone()),
        senza_dati_del_file(prima.progetto.clone())
    );
    assert_eq!(codici(&seconda), codici(&prima));
    // Il secondo export è identico al primo: il formato è stabile.
    let di_nuovo = esporta(&seconda.progetto, seconda.computo).unwrap();
    assert_eq!(di_nuovo.xml, export.xml);
}

#[test]
fn l_export_conserva_identificativi_ordine_e_dati_sconosciuti() {
    let r = importa_da(FIXTURE);
    let xml = esporta(&r.progetto, r.computo).unwrap().xml;
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<PweDocumento>\r\n"));
    for frammento in [
        "<Versione>1.0</Versione>",
        "<CodiceInterno>X-001</CodiceInterno>",
        "<EPItem ID=\"6\">",
        "<VCItem ID=\"5\">",
        "<RGItem ID=\"12\">",
        "<IDVV>1</IDVV>",
        "<Flags>134217728</Flags>",
        "<Flags>2</Flags>",
        "<PartiUguali>-1</PartiUguali>",
        "<Lunghezza>2*(15+10)</Lunghezza>",
        "<Quantita>214.35</Quantita>",
        "<PrezziTotale>14.2|0</PrezziTotale>",
        "<SpeseGenerali>15</SpeseGenerali>",
        "<DesEstesa>Calcestruzzo C25/30 per fondazioni, gettato in opera &amp; vibrato.</DesEstesa>",
        "<DGCategorieItem ID=\"2\">",
    ] {
        assert!(xml.contains(frammento), "manca {frammento}\n{xml}");
    }
    // Ordine originale dei campi di EPItem: TipoEP, Tariffa, Articolo, DesRidotta…
    let ep = xml.find("<EPItem ID=\"1\">").unwrap();
    let pos = |s: &str| xml[ep..].find(s).unwrap();
    assert!(pos("<TipoEP>") < pos("<Tariffa>"));
    assert!(pos("<Tariffa>") < pos("<Articolo/>"));
    assert!(pos("<IncMDO>") < pos("<CodiceInterno>"));
}

#[test]
fn le_modifiche_al_modello_arrivano_nell_export() {
    let mut r = importa_da(FIXTURE);
    let voce = r
        .progetto
        .elenco_prezzi
        .voci
        .iter_mut()
        .find(|v| v.codice == "E.01.010")
        .unwrap();
    voce.prezzo = Prezzo::new(dec!(9.50));
    voce.natura = NaturaVoce::OneriSicurezza;
    r.progetto.regole.decimali_quantita = 3;
    let export = esporta(&r.progetto, r.computo).unwrap();
    let ep = export.xml.find("<EPItem ID=\"1\">").unwrap();
    let fine = export.xml[ep..].find("</EPItem>").unwrap();
    let blocco = &export.xml[ep..ep + fine];
    assert!(blocco.contains("<Prezzo1>9.50</Prezzo1>"));
    assert!(blocco.contains("<Flags>134217728</Flags>"));
    assert!(export.xml.contains("<Quantita>9.3|0</Quantita>"));
    // Il prezzo applicato nel computo è una fotografia: non coincide più con
    // l'elenco, quindi l'export aggiunge una voce di elenco e lo segnala.
    assert!(
        export
            .anomalie
            .iter()
            .any(|a| a.codice == "xpwe.prezzo_non_allineato")
    );
    assert!(export.xml.contains("<EPItem ID=\"7\">"));
    // Con 3 decimali le quantità non sono più arrotondate al centesimo (15,552 resta
    // 15,552): il totale cambia, e il file reimportato deve dare lo stesso totale.
    let atteso = r.progetto.calcola_computo(r.computo).unwrap().totale;
    assert_ne!(atteso, Importo::new(dec!(9291.84)));
    let reimportato = importa_da(export.xml.as_bytes());
    assert_eq!(reimportato.progetto.regole.decimali_quantita, 3);
    assert_eq!(reimportato.rapporto.statistiche.quantita_diverse, 0);
    let totale = reimportato
        .progetto
        .calcola_computo(reimportato.computo)
        .unwrap()
        .totale;
    assert_eq!(totale, atteso);
}

/// Progetto costruito direttamente nel modello, senza origine XPWE.
fn progetto_nativo() -> (Progetto, ComputoId) {
    let mut g = GeneratoreSequenziale::new(100);
    let mut p = Progetto::new(g.nuovo(), "Manutenzione palestra", g.nuovo());
    p.metadati.luogo = Some("Paese".into());
    p.metadati.cup = Some("J00000000000000".into());
    let pav = VocePrezzario {
        incidenza_manodopera: Some(Percentuale::new(dec!(42.5))),
        ..VocePrezzario::new(
            g.nuovo(),
            "P.10",
            "Pavimento in gomma",
            "m²",
            Prezzo::new(dec!(38.70)),
        )
    };
    let bat = VocePrezzario::new(
        g.nuovo(),
        "P.20",
        "Battiscopa",
        "m",
        Prezzo::new(dec!(6.15)),
    );
    let sic = VocePrezzario {
        natura: NaturaVoce::OneriSicurezza,
        ..VocePrezzario::new(
            g.nuovo(),
            "S.1",
            "Delimitazione area",
            "a corpo",
            Prezzo::new(dec!(250)),
        )
    };
    p.elenco_prezzi.aggiungi_voce(pav.clone()).unwrap();
    p.elenco_prezzi.aggiungi_voce(bat.clone()).unwrap();
    p.elenco_prezzi.aggiungi_voce(sic.clone()).unwrap();
    let cat = Classe {
        id: g.nuovo(),
        codice: "PAV".into(),
        descrizione: "Pavimenti".into(),
    };
    p.categorie
        .aggiungi(LivelloCategoria::Categoria, cat.clone())
        .unwrap();

    let mut c = Computo::new(g.nuovo(), "Computo");
    let mut v1 = VoceComputo::new(g.nuovo(), PrezzoApplicato::da_voce(&p.elenco_prezzi, &pav));
    v1.categorie.categoria = Some(cat.id);
    v1.righi = vec![
        RigoMisura::commento(g.nuovo(), "Campo da gioco"),
        RigoMisura::misura(g.nuovo(), "Campo")
            .con_lunghezza(dec!(24.00))
            .con_larghezza(dec!(14.00)),
        RigoMisura::misura(g.nuovo(), "A dedurre pedana")
            .con_lunghezza("2,50+0,50")
            .con_larghezza(dec!(2.00))
            .come_deduzione(),
        RigoMisura::misura(g.nuovo(), "Spogliatoi")
            .con_parti(dec!(2))
            .con_lunghezza(dec!(5.40))
            .con_larghezza(dec!(3.80))
            .con_formula("par*lun*lar*0,95"),
    ];
    let mut v2 = VoceComputo::new(g.nuovo(), PrezzoApplicato::da_voce(&p.elenco_prezzi, &bat));
    v2.righi = vec![RigoMisura::misura(g.nuovo(), "Perimetro").con_lunghezza("2*(24+14)")];
    let mut v3 = VoceComputo::new(g.nuovo(), PrezzoApplicato::da_voce(&p.elenco_prezzi, &sic));
    v3.righi = vec![RigoMisura::misura(g.nuovo(), "").con_parti(dec!(1))];
    c.voci = vec![v1, v2, v3];
    let id = c.id;
    p.aggiungi_computo(c).unwrap();
    (p, id)
}

#[test]
fn progetto_nativo_esportato_e_reimportato() {
    let (p, id) = progetto_nativo();
    let atteso = p.calcola_computo(id).unwrap();
    // Pavimento: 24×14 = 336,00; − 3,00×2,00 = 6,00; 2×5,40×3,80×0,95 = 38,988 → 38,99;
    // quantità 368,99 × 38,70 = 14.279,913 → 14.279,91.
    // Battiscopa: 76,00 × 6,15 = 467,40. Sicurezza: 1 × 250 = 250,00.
    assert_eq!(atteso.totale, Importo::new(dec!(14997.31)));

    let export = esporta(&p, id).unwrap();
    let codici_export: Vec<_> = export.anomalie.iter().map(|a| a.codice.as_str()).collect();
    for c in [
        "xpwe.export_non_verificato",
        "xpwe.formula_riscritta",
        "xpwe.metadati_non_esportati",
    ] {
        assert!(codici_export.contains(&c), "manca {c}: {codici_export:?}");
    }
    assert!(
        export
            .xml
            .contains("<PartiUguali>(2)*(5.40)*(3.80)*0,95</PartiUguali>"),
        "{}",
        export.xml
    );
    assert!(
        export.xml.contains("<Lunghezza>-(2,50+0,50)</Lunghezza>"),
        "{}",
        export.xml
    );
    assert!(export.xml.contains("<Flags>134217728</Flags>"));

    let r = importa(
        export.xml.as_bytes(),
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
    .unwrap();
    assert!(!r.rapporto.ha_errori(), "{:#?}", r.rapporto.anomalie);
    assert_eq!(r.rapporto.statistiche.quantita_diverse, 0);
    let ottenuto = r.progetto.calcola_computo(r.computo).unwrap();
    assert_eq!(ottenuto.totale, atteso.totale);
    assert_eq!(
        ottenuto.totale_oneri_sicurezza,
        atteso.totale_oneri_sicurezza
    );
    let riassunto = |c: &RiepilogoComputo| -> Vec<(String, Quantita, Importo)> {
        c.voci
            .iter()
            .map(|v| (v.codice.clone(), v.calcolo.quantita, v.calcolo.importo))
            .collect()
    };
    assert_eq!(riassunto(&ottenuto), riassunto(&atteso));
    assert_eq!(r.progetto.metadati.titolo, "Manutenzione palestra");
    assert_eq!(r.progetto.metadati.luogo.as_deref(), Some("Paese"));
    let pav = r.progetto.elenco_prezzi.voce_per_codice("P.10").unwrap();
    assert_eq!(pav.incidenza_manodopera, Some(Percentuale::new(dec!(42.5))));
    let voce = &r.progetto.computi[0].voci[0];
    let cat = r
        .progetto
        .categorie
        .categorie
        .iter()
        .find(|c| Some(c.id) == voce.categorie.categoria)
        .unwrap();
    assert_eq!(cat.codice, "PAV");
    // La deduzione torna deduzione, il commento torna commento.
    assert_eq!(voce.righi[0].tipo, TipoRigo::Commento);
    assert_eq!(voce.righi[2].segno, SegnoRigo::Deduzione);
    assert_eq!(
        voce.righi[2].lunghezza,
        Some(Fattore::Espressione("2,50+0,50".into()))
    );
}

#[test]
fn computo_inesistente_o_non_calcolabile() {
    let (mut p, id) = progetto_nativo();
    let altro = opencomputo_core::id::ComputoId::da_u128(999);
    assert!(matches!(
        esporta(&p, altro),
        Err(opencomputo_format_xpwe::ErroreXpwe::ComputoInesistente(_))
    ));
    p.computi[0].voci[1].righi[0].lunghezza = Some(Fattore::Espressione("2*(".into()));
    assert!(matches!(
        esporta(&p, id),
        Err(opencomputo_format_xpwe::ErroreXpwe::Calcolo(_))
    ));
}

#[test]
fn vedi_voce_inseriti_in_opencomputo_esportati_con_la_quantita() {
    use opencomputo_core::misura::VediVoce;
    let mut r = importa_da(FIXTURE);
    let c = r.progetto.computo_mut(r.computo).unwrap();
    let scavo = c.voci[0].id;
    // Nella recinzione: metà dello scavo (243,00) e un «vedi voce» statico con formula.
    c.voci[3].righi.push(
        RigoMisura::misura(RigoMisuraId::da_u128(90_001), "Metà scavo")
            .con_vedi_voce(VediVoce::Collegata(scavo))
            .con_parti(dec!(0.5)),
    );
    c.voci[3].righi.push(
        RigoMisura::misura(RigoMisuraId::da_u128(90_002), "Da altro computo")
            .con_vedi_voce(VediVoce::Statica {
                quantita: dec!(10),
                riferimento: "voce n. 2".into(),
            })
            .con_lunghezza(dec!(4))
            .con_formula("vv/lun"),
    );
    let export = esporta(&r.progetto, r.computo).unwrap();
    assert!(
        export
            .anomalie
            .iter()
            .any(|a| a.codice == "xpwe.vedi_voce_esportato" && a.messaggio.starts_with("2 righi")),
        "{:?}",
        export.anomalie
    );
    assert!(
        export
            .xml
            .contains("<PartiUguali>243.00*(0.5)</PartiUguali>"),
        "{}",
        export.xml
    );
    assert!(export.xml.contains("<PartiUguali>(10)/(4)</PartiUguali>"));
    // Riletti: righi normali con la stessa quantità (50 + 121,50 + 2,50).
    let riletto = importa_da(export.xml.as_bytes());
    let q = riletto
        .progetto
        .calcola_computo(riletto.computo)
        .unwrap()
        .voci[3]
        .calcolo
        .quantita;
    assert_eq!(q, Quantita::new(dec!(174.00)));
}
