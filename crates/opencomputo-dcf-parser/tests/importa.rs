//! Import di DCF sintetici: struttura, mappatura, protezioni, robustezza.

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
use opencomputo_core::provenienza::FormatoOrigine;
use opencomputo_dcf_parser::contenitore::{self, Contenuto};
use opencomputo_dcf_parser::{ErroreDcf, OpzioniImport, RisultatoImport, importa};
use rust_decimal_macros::dec;

fn prova(dati: &[u8]) -> Result<RisultatoImport, ErroreDcf> {
    importa(
        dati,
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
}

/// Prezzario sintetico: pezzi da 300 byte, flussi su più settori.
fn prezzario() -> Vec<u8> {
    // Testo poco comprimibile, perché il flusso occupi più settori.
    let mut x: u32 = 7;
    let lunga: String = (0..5000)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            char::from(b'a' + u8::try_from((x >> 16) % 26).unwrap())
        })
        .collect();
    let voci = [
        voce(2, "", "0", "Art=\"Avvertenze generali\""),
        voce(
            3,
            "E.01.010",
            "9.20",
            "IdSpCap=\"1\" IdCap=\"7\" IdSbCap=\"0\" MDOInc=\"62.159999999999996600\" SICInc=\"0.660453226917961045\" Fgs=\"576\" Prz2=\"10.5\" TipoRsrs=\"2\"",
        ),
        voce(
            4,
            "S.01.001",
            "1450",
            "Fgs=\"134217728\" DesBrv=\"Recinzione\"",
        ),
        voce(
            5,
            "E.01.020",
            "4,10",
            &format!("IdCap=\"7\" Art=\"{lunga}\""),
        ),
    ];
    let voci: Vec<&str> = voci.iter().map(String::as_str).collect();
    dcf(&[
        flusso(&info_doc("", ""), 300),
        flusso(&dati_generali(), 300),
        flusso(&elenco(&voci), 300),
        flusso(b"\t<CollectionVC CountId=\"1\" />\r\n", 300),
        flusso(b"<OpzioniInterfaccia />", 300),
        flusso(&[0u8, 1, 2, 3, 4, 5, 6, 7], 300),
    ])
}

#[test]
fn contenitore_sintetico() {
    let c = contenitore::leggi(&prezzario()).unwrap();
    assert_eq!(c.settori_dichiarati, c.settori);
    let tipi: Vec<_> = c
        .flussi
        .iter()
        .map(|f| match &f.contenuto {
            Contenuto::Xml { radice, .. } => radice.clone(),
            altro => format!("{altro:?}"),
        })
        .collect();
    assert_eq!(
        tipi,
        [
            "InfoDoc",
            "DatiGenerali",
            "CollectionEP",
            "CollectionVC",
            "OpzioniInterfaccia",
            "Binario"
        ]
    );
    // L'elenco occupa più settori ed è diviso in molti pezzi.
    assert!(c.flussi[2].settori > 1);
}

#[test]
fn mappatura_elenco_capitoli_categorie() {
    let r = prova(&prezzario()).unwrap();
    assert!(!r.rapporto.ha_errori(), "{:#?}", r.rapporto.anomalie);
    let p = &r.progetto;
    assert_eq!(p.metadati.titolo, "Prezzario di prova 2026");
    assert_eq!(p.metadati.luogo.as_deref(), Some("Comune di Prova"));
    assert_eq!(p.metadati.committente.as_deref(), Some("Ente di prova"));
    assert_eq!(p.metadati.impresa, None);
    assert_eq!(p.elenco_prezzi.nome, "Prezzario di prova 2026");

    // Capitoli e categorie: Id 0 è il segnaposto, gli elementi fuori dagli elenchi sono ignorati.
    let cap: Vec<_> = p
        .elenco_prezzi
        .capitoli
        .iter()
        .map(|c| (c.codice.as_str(), c.descrizione.as_str()))
        .collect();
    assert_eq!(cap, [("E", "Edilizia"), ("E.01", "Scavi")]);
    let cat: Vec<_> = p
        .categorie
        .categorie
        .iter()
        .map(|c| c.codice.as_str())
        .collect();
    assert_eq!(cat, ["ST"]);
    assert!(p.categorie.super_categorie.is_empty());

    let s = &r.rapporto.statistiche;
    assert_eq!(
        (
            s.voci_lette,
            s.voci_elenco,
            s.voci_senza_tariffa,
            s.voci_scartate
        ),
        (4, 3, 1, 0)
    );
    assert_eq!(s.incidenze_arrotondate, 2);

    let scavo = p.elenco_prezzi.voce_per_codice("E.01.010").unwrap();
    assert_eq!(scavo.prezzo, Prezzo::new(dec!(9.20)));
    assert_eq!(
        scavo.incidenza_manodopera,
        Some(Percentuale::new(dec!(62.16)))
    );
    assert_eq!(
        scavo.incidenza_sicurezza,
        Some(Percentuale::new(dec!(0.660453)))
    );
    assert_eq!(scavo.natura, NaturaVoce::Lavoro);
    let e01 = p
        .elenco_prezzi
        .capitoli
        .iter()
        .find(|c| c.codice == "E.01")
        .unwrap();
    assert_eq!(scavo.capitolo, Some(e01.id));
    let prov = scavo.provenienza.as_ref().unwrap();
    assert_eq!(prov.formato, FormatoOrigine::Dcf);
    assert_eq!(prov.identificativo_originale.as_deref(), Some("3"));
    assert_eq!(
        prov.dati_originali.get("MDOInc").unwrap(),
        "62.159999999999996600"
    );
    assert_eq!(prov.dati_originali.get("Prz2").unwrap(), "10.5");
    assert_eq!(prov.dati_originali.get("Fgs").unwrap(), "576");

    let recinzione = p.elenco_prezzi.voce_per_codice("S.01.001").unwrap();
    assert_eq!(recinzione.natura, NaturaVoce::OneriSicurezza);
    assert_eq!(recinzione.descrizione_breve, "Recinzione");
    assert_eq!(recinzione.prezzo, Prezzo::new(dec!(1450)));
    let rinterro = p.elenco_prezzi.voce_per_codice("E.01.020").unwrap();
    assert_eq!(rinterro.prezzo, Prezzo::new(dec!(4.10)));
    assert_eq!(
        rinterro
            .provenienza
            .as_ref()
            .unwrap()
            .dati_originali
            .get("Art")
            .map(String::len),
        Some(5000)
    );

    let codici: Vec<_> = r
        .rapporto
        .anomalie
        .iter()
        .map(|a| a.codice.as_str())
        .collect();
    assert!(codici.contains(&"dcf.voci_senza_tariffa"));
    assert!(codici.contains(&"dcf.flussi_non_interpretati"));
    assert!(!codici.contains(&"dcf.computo_non_importato"));
    assert!(valida_progetto(&r.progetto).problemi.is_empty());
    let prov = r.progetto.provenienza.as_ref().unwrap();
    assert_eq!(prov.formato, FormatoOrigine::Dcf);
    assert_eq!(
        prov.dati_originali.get("DGDatiGenerali/Prvnc").unwrap(),
        "ZZ"
    );
}

#[test]
fn import_deterministico_e_serializzabile() {
    let a = prova(&prezzario()).unwrap().progetto;
    assert_eq!(a, prova(&prezzario()).unwrap().progetto);
    let json = opencomputo_core::schema::esporta_json(&a).unwrap();
    assert_eq!(opencomputo_core::schema::importa_json(&json).unwrap(), a);
}

#[test]
fn documento_protetto_rifiutato() {
    let d = dcf(&[
        flusso(&protetto(3000), 300),
        flusso(&protetto(500), 300),
        flusso(&[9u8; 64], 300),
    ]);
    let e = prova(&d).unwrap_err();
    assert_eq!(e, ErroreDcf::Protetto { flussi: 2 });
    assert!(e.to_string().contains("XPWE"));
}

#[test]
fn parti_protette_segnalate_se_l_elenco_e_leggibile() {
    let d = dcf(&[
        flusso(&elenco(&[&voce(1, "A", "1", "")]), 4096),
        flusso(&protetto(100), 4096),
    ]);
    let r = prova(&d).unwrap();
    assert_eq!(r.rapporto.statistiche.flussi_protetti, 1);
    let a = r.rapporto.con_codice("dcf.flussi_protetti").next().unwrap();
    assert_eq!(a.gravita, Gravita::Avviso);
    assert_eq!(r.progetto.elenco_prezzi.voci.len(), 1);
}

#[test]
fn password_di_apertura_rispettata() {
    let el = elenco(&[&voce(1, "A", "1", "")]);
    let d = dcf(&[flusso(&info_doc("x7Kq", ""), 4096), flusso(&el, 4096)]);
    assert_eq!(prova(&d).unwrap_err(), ErroreDcf::ProtettoDaPassword);
    // La sola password di modifica non impedisce la lettura.
    let d = dcf(&[flusso(&info_doc("", "m0d"), 4096), flusso(&el, 4096)]);
    let r = prova(&d).unwrap();
    assert!(
        r.rapporto
            .con_codice("dcf.password_modifica")
            .next()
            .is_some()
    );
}

#[test]
fn computo_presente_segnalato_e_utf16() {
    let el: Vec<u8> = String::from_utf8(elenco(&[&voce(1, "Città", "2.5", "")]))
        .unwrap()
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let vc =
        b"<CollectionVC CountId=\"3\"><ItemVC IdVC=\"1\" /><ItemVC IdVC=\"2\" /></CollectionVC>";
    let r = prova(&dcf(&[flusso(&el, 4096), flusso(vc, 4096)])).unwrap();
    assert_eq!(r.progetto.elenco_prezzi.voci[0].codice, "Città");
    let a = r
        .rapporto
        .con_codice("dcf.computo_non_importato")
        .next()
        .unwrap();
    assert!(a.messaggio.starts_with("2 voci di computo"));
}

#[test]
fn voci_non_valide_scartate_con_errore() {
    let d = dcf(&[flusso(
        &elenco(&[
            &voce(1, "A", "1", ""),
            &voce(1, "B", "2", ""),
            &voce(2, "C", "uno", ""),
            &voce(3, "D", "", ""),
            &voce(4, "A", "3", "IdCap=\"42\""),
        ]),
        4096,
    )]);
    let r = prova(&d).unwrap();
    assert!(r.rapporto.ha_errori());
    let s = &r.rapporto.statistiche;
    assert_eq!((s.voci_elenco, s.voci_scartate), (2, 3));
    for c in [
        "dcf.id_duplicato",
        "dcf.numero_non_valido",
        "dcf.prezzo_mancante",
        "dcf.tariffa_duplicata",
        "dcf.riferimento_non_risolto",
    ] {
        assert!(r.rapporto.con_codice(c).next().is_some(), "manca {c}");
    }
    let errore = r
        .rapporto
        .con_codice("dcf.numero_non_valido")
        .next()
        .unwrap();
    assert!(errore.posizione.as_deref().unwrap().contains("riga"));
}

#[test]
fn file_non_dcf_danneggiati_o_troncati() {
    assert!(matches!(
        prova(b"<PweDocumento/>"),
        Err(ErroreDcf::NonDcf(_))
    ));
    assert!(matches!(prova(&[]), Err(ErroreDcf::NonDcf(_))));
    let buono = prezzario();
    // Dimensione dei settori diversa da 4096.
    let mut d = buono.clone();
    d[0x118 + 0x10] = 0;
    d[0x118 + 0x11] = 0x02;
    assert!(matches!(prova(&d), Err(ErroreDcf::Danneggiato(_))));
    // Un byte alterato nei dati compressi: flusso illeggibile, segnalato, nessun panico.
    let mut d = buono.clone();
    let pos = (0x118 + 4096 * 6 + 0x20 + 0x90 + 40).min(d.len() - 1);
    d[pos] ^= 0xFF;
    match prova(&d) {
        Ok(r) => assert!(r.rapporto.statistiche.flussi_illeggibili > 0),
        Err(e) => assert!(matches!(e, ErroreDcf::NessunDato(_)), "{e}"),
    }
    // Prefissi del file: mai un panico.
    for n in (0..buono.len()).step_by(997) {
        let _ = prova(&buono[..n]);
    }
}

#[test]
fn limiti_contro_i_file_bomba() {
    // Pezzo dichiarato oltre il massimo consentito.
    let mut f = flusso(&elenco(&[&voce(1, "A", "1", "")]), 4096);
    f[4..8].copy_from_slice(&(64u32 << 20).to_le_bytes());
    let c = contenitore::leggi(&dcf(&[f])).unwrap();
    assert!(
        matches!(&c.flussi[0].contenuto, Contenuto::Illeggibile(m) if m.contains("dimensione dei pezzi"))
    );
    // Pezzo che si decomprime oltre la dimensione dichiarata.
    let mut f = flusso(&vec![b'a'; 10_000], 1000);
    f[4..8].copy_from_slice(&500u32.to_le_bytes());
    let c = contenitore::leggi(&dcf(&[f])).unwrap();
    assert!(
        matches!(&c.flussi[0].contenuto, Contenuto::Illeggibile(m) if m.contains("oltre")),
        "{:?}",
        c.flussi[0].contenuto
    );
}
