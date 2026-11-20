//! Import della fixture sintetica: valori verificati a mano.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::misura::VediVoce;
use opencomputo_core::prelude::*;
use opencomputo_core::provenienza::FormatoOrigine;
use rust_decimal_macros::dec;
use std::sync::Arc;

#[test]
fn totali_verificati_a_mano() {
    let r = importa_da(FIXTURE);
    assert!(!r.rapporto.ha_errori(), "{:#?}", r.rapporto.anomalie);
    let riepilogo = r.progetto.calcola_computo(r.computo).unwrap();
    // Scavo 243,00 × 9,20 = 2.235,60; calcestruzzo 28,65 × 152,40 = 4.366,26;
    // acciaio 587,10 × 1,85 = 1.086,135 → 1.086,14; rinterro 214,35 × 4,10 = 878,835 → 878,84.
    assert_eq!(riepilogo.totale_lavori, Importo::new(dec!(8566.84)));
    // Recinzione (sicurezza) 50,00 × 14,50.
    assert_eq!(riepilogo.totale_oneri_sicurezza, Importo::new(dec!(725.00)));
    assert_eq!(riepilogo.totale, Importo::new(dec!(9291.84)));
    let quantita: Vec<_> = riepilogo.voci.iter().map(|v| v.calcolo.quantita).collect();
    assert_eq!(
        quantita,
        [
            dec!(243.00),
            dec!(28.65),
            dec!(587.10),
            dec!(50.00),
            dec!(214.35)
        ]
        .map(Quantita::new)
    );
    let s = &r.rapporto.statistiche;
    assert_eq!((s.voci_elenco, s.voci_computo, s.righi), (6, 5, 12));
    assert_eq!((s.capitoli, s.categorie), (3, 3));
    assert_eq!(s.quantita_diverse, 0);
    // 9 righi verificati (esclusi commento, parziale e «vedi voce», che usa la
    // quantità del file) + 5 voci.
    assert_eq!(s.quantita_verificate, 14);
    assert_eq!(r.rapporto.codifica, "UTF-8");
}

#[test]
fn dati_generali_elenco_e_classificazioni() {
    let r = importa_da(FIXTURE);
    let p = &r.progetto;
    assert_eq!(p.metadati.titolo, "Ampliamento scuola primaria");
    assert_eq!(p.metadati.luogo.as_deref(), Some("Comune di Esempio"));
    assert_eq!(p.metadati.committente.as_deref(), Some("Comune di Esempio"));
    assert_eq!(p.metadati.impresa, None);
    assert_eq!(p.regole.decimali_quantita, 2);
    assert_eq!(p.regole.decimali_importo, 2);

    let scavo = p.elenco_prezzi.voce_per_codice("E.01.010").unwrap();
    assert_eq!(scavo.prezzo, Prezzo::new(dec!(9.20)));
    assert_eq!(scavo.unita_misura, "m³");
    assert_eq!(scavo.incidenza_manodopera, Some(Percentuale::new(dec!(30))));
    assert_eq!(scavo.incidenza_sicurezza, None);
    let cap = p
        .elenco_prezzi
        .capitoli
        .iter()
        .find(|c| Some(c.id) == scavo.capitolo)
        .unwrap();
    assert_eq!(cap.codice, "E.01");
    let cls = p.elenco_prezzi.voce_per_codice("E.02.020").unwrap();
    assert_eq!(
        cls.descrizione_estesa,
        "Calcestruzzo C25/30 per fondazioni, gettato in opera & vibrato."
    );
    let recinzione = p.elenco_prezzi.voce_per_codice("S.01.001").unwrap();
    assert_eq!(recinzione.natura, NaturaVoce::OneriSicurezza);
    assert_eq!(recinzione.capitolo, None);

    let codici_cat: Vec<_> = p
        .categorie
        .categorie
        .iter()
        .map(|c| c.codice.as_str())
        .collect();
    assert_eq!(codici_cat, ["FOND", "ELEV"]);
    let computo = p.computo(r.computo).unwrap();
    let prima = &computo.voci[0];
    assert_eq!(
        prima.categorie.categoria,
        p.categorie.categorie.first().map(|c| c.id)
    );
    assert_eq!(computo.voci[3].categorie, AssegnazioneCategorie::default());
    assert_eq!(prima.prezzo.codice, "E.01.010");
    assert_eq!(prima.prezzo.fonte.as_deref(), Some("Elenco prezzi"));
    let prov = prima.provenienza.as_ref().unwrap();
    assert_eq!(prov.formato, FormatoOrigine::Xpwe);
    assert_eq!(prov.identificativo_originale.as_deref(), Some("1"));
    // Campi noti non mappati conservati.
    assert_eq!(
        prov.dati_originali.get("DataMis").unwrap(),
        "<DataMis>15/03/2026</DataMis>"
    );
}

#[test]
fn righi_deduzioni_commenti_parziali_e_vedi_voce() {
    let r = importa_da(FIXTURE);
    let c = r.progetto.computo(r.computo).unwrap();
    let pozzetto = &c.voci[0].righi[2];
    assert_eq!(pozzetto.segno, SegnoRigo::Deduzione);
    assert_eq!(pozzetto.parti_uguali, Some(Fattore::Valore(dec!(1))));
    let travi = &c.voci[1].righi[1];
    assert_eq!(
        travi.lunghezza,
        Some(Fattore::Espressione("15.00+0.60".into()))
    );
    assert_eq!(c.voci[1].righi[2].tipo, TipoRigo::Commento);
    // Rigo parziale (Flags = 2): commento, non conta nella quantità.
    let parziale = &c.voci[2].righi[1];
    assert_eq!(parziale.tipo, TipoRigo::Commento);
    assert_eq!(parziale.descrizione, "Parziale plinti");
    // «Vedi voce»: la quantità dichiarata coincide con quella della voce ID 1, quindi il
    // rigo è collegato; il riferimento originale resta per l'export.
    let vedi = &c.voci[4].righi[0];
    assert_eq!(vedi.vedi_voce, Some(VediVoce::Collegata(c.voci[0].id)));
    assert_eq!(vedi.parti_uguali, None);
    let dati = &c.voci[4].provenienza.as_ref().unwrap().dati_originali;
    assert_eq!(dati.get(&format!("rigo:{}/=IDVV", vedi.id)).unwrap(), "1");
    // Cambia la voce citata, cambia il rinterro.
    let mut p = r.progetto.clone();
    let scavo = Arc::make_mut(&mut p.computo_mut(r.computo).unwrap().voci[0]);
    scavo.righi.truncate(1);
    let q = |p: &Progetto, i: usize| {
        let c = p.computo(r.computo).unwrap();
        c.quantita_voce(c.voci[i].id, &p.regole).unwrap().valore()
    };
    assert_eq!(q(&p, 4), q(&p, 0) - dec!(28.65));
}

#[test]
fn vedi_voce_non_coincidente_o_senza_voce_resta_statico() {
    let originale = "<IDVV>1</IDVV>\n            <Descrizione>Vedi voce n. 1</Descrizione>\n            <PartiUguali/>\n            <Lunghezza/>\n            <Larghezza/>\n            <HPeso/>\n            <Quantita>243.00</Quantita>";
    let testo = std::str::from_utf8(FIXTURE).unwrap();
    assert!(testo.contains(originale));
    for (sostituto, codice) in [
        (originale.replace("243.00", "200.00"), "xpwe.vedi_voce"),
        (
            originale.replace("<IDVV>1<", "<IDVV>99<"),
            "xpwe.vedi_voce_non_collegato",
        ),
    ] {
        let r = importa_da(testo.replace(originale, &sostituto).as_bytes());
        let c = r.progetto.computo(r.computo).unwrap();
        let vedi = &c.voci[4].righi[0];
        assert!(
            matches!(&vedi.vedi_voce, Some(VediVoce::Statica { quantita, .. }) if *quantita == dec!(200.00) || *quantita == dec!(243.00)),
            "{:?}",
            vedi.vedi_voce
        );
        assert!(
            codici(&r).iter().any(|x| x == codice),
            "{codice}: {:?}",
            codici(&r)
        );
        assert!(!r.rapporto.ha_errori());
    }
    // Con i fattori del file come moltiplicatori: metà della voce citata.
    let meta = originale
        .replace("<PartiUguali/>", "<PartiUguali>0.5</PartiUguali>")
        .replace("243.00", "121.50");
    let r = importa_da(testo.replace(originale, &meta).as_bytes());
    let c = r.progetto.computo(r.computo).unwrap();
    let vedi = &c.voci[4].righi[0];
    assert_eq!(vedi.vedi_voce, Some(VediVoce::Collegata(c.voci[0].id)));
    assert_eq!(vedi.parti_uguali, Some(Fattore::Valore(dec!(0.5))));
}

#[test]
fn anomalie_attese_e_dati_conservati() {
    let r = importa_da(FIXTURE);
    let c = codici(&r);
    for atteso in [
        "xpwe.analisi_non_importata",
        "xpwe.elemento_conservato",
        "xpwe.rigo_parziale",
        "xpwe.vedi_voce_collegato",
    ] {
        assert!(c.iter().any(|x| x == atteso), "manca {atteso}: {c:?}");
    }
    let sconosciuti: Vec<_> = r
        .rapporto
        .con_codice("xpwe.elemento_conservato")
        .map(|a| a.messaggio.clone())
        .collect();
    // Versione e TipoDocumento sono campi di testata noti (export PriMus-DCF): conservati
    // senza segnalazione. Resta sconosciuto solo CodiceInterno.
    assert_eq!(sconosciuti.len(), 1, "{sconosciuti:?}");
    assert!(sconosciuti[0].contains("CodiceInterno"));
    let doc = &r.progetto.provenienza.as_ref().unwrap().dati_originali;
    assert_eq!(doc.get("doc:Versione").unwrap(), "<Versione>1.0</Versione>");
    assert!(
        doc.get("dg:PweDGModuli")
            .unwrap()
            .contains("<SpeseGenerali>15</SpeseGenerali>")
    );
    let prov = r.progetto.provenienza.as_ref().unwrap();
    assert_eq!(prov.impronta_sha256.as_ref().map(String::len), Some(64));
    assert_eq!(prov.file.as_deref(), Some("computo_sintetico.xpwe"));
    assert_eq!(prov.anomalie, r.rapporto.anomalie);
}

#[test]
fn il_progetto_importato_e_valido_e_serializzabile() {
    let r = importa_da(FIXTURE);
    let v = valida_progetto(&r.progetto);
    assert!(!v.ha_errori(), "{:#?}", v.problemi);
    let json = opencomputo_core::schema::esporta_json(&r.progetto).unwrap();
    let letto = opencomputo_core::schema::importa_json(&json).unwrap();
    assert_eq!(letto, r.progetto);
}

#[test]
fn import_deterministico() {
    assert_eq!(importa_da(FIXTURE).progetto, importa_da(FIXTURE).progetto);
}
