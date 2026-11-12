//! File anomali, incompleti o malformati: errori espliciti, mai dati persi in silenzio.

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
use opencomputo_format_xpwe::{ErroreXpwe, OpzioniImport, importa};
use rust_decimal_macros::dec;

fn prova(dati: &[u8]) -> Result<opencomputo_format_xpwe::RisultatoImport, ErroreXpwe> {
    importa(
        dati,
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
}

#[test]
fn xml_malformato_e_documenti_non_xpwe() {
    let e = prova(b"<PweDocumento>\n<PweMisurazioni>\n</PweDocumento>").unwrap_err();
    assert!(matches!(e, ErroreXpwe::XmlNonValido { riga: 3, .. }), "{e}");
    assert!(e.to_string().starts_with("XML non valido alla riga 3"));
    assert!(matches!(
        prova(b"<html><body/></html>"),
        Err(ErroreXpwe::NonXpwe(_))
    ));
    assert!(matches!(
        prova(b"non xml"),
        Err(ErroreXpwe::NonXpwe(_) | ErroreXpwe::XmlNonValido { .. })
    ));
}

#[test]
fn nessun_panico_su_file_troncati() {
    // Ogni prefisso del file deve dare un risultato o un errore, mai un panico.
    for n in (0..FIXTURE.len()).step_by(7) {
        let _ = prova(&FIXTURE[..n]);
    }
}

#[test]
fn idep_non_risolto_e_prezzo_non_valido() {
    let dati = documento(
        &format!("{}{}", ep("1", "A", "10.00"), ep("2", "B", "dieci")),
        &format!(
            "{}{}{}",
            vc("1", "1", &rg("", "2", "", "", "2.00")),
            vc("2", "2", &rg("", "3", "", "", "3.00")),
            vc("3", "99", &rg("", "4", "", "", "4.00"))
        ),
    );
    let r = prova(&dati).unwrap();
    assert!(r.rapporto.ha_errori());
    let s = &r.rapporto.statistiche;
    assert_eq!((s.voci_elenco, s.voci_elenco_scartate), (1, 1));
    assert_eq!((s.voci_computo, s.voci_computo_scartate), (1, 2));
    let errori: Vec<_> = r
        .rapporto
        .anomalie
        .iter()
        .filter(|a| a.gravita == Gravita::Errore)
        .map(|a| (a.codice.as_str(), a.posizione.clone().unwrap_or_default()))
        .collect();
    assert_eq!(errori.len(), 3, "{errori:?}");
    assert_eq!(errori[0].0, "xpwe.numero_non_valido");
    assert!(errori[0].1.contains("EPItem[ID=2]"));
    assert_eq!(errori[1].0, "xpwe.idep_non_risolto");
    assert_eq!(errori[2].0, "xpwe.idep_non_risolto");
    assert!(errori[2].1.contains("VCItem[ID=3]"));
    // Il resto è importato e calcolabile.
    let t = r.progetto.calcola_computo(r.computo).unwrap().totale;
    assert_eq!(t, Importo::new(dec!(20.00)));
}

#[test]
fn virgola_decimale_moltiplicazione_implicita_e_codifica() {
    let mut dati = b"<?xml version=\"1.0\" encoding=\"windows-1252\"?>".to_vec();
    dati.extend(documento(
        &ep("1", "M", "12,50"),
        &vc("1", "1", &rg("2(1+1)", "1,5", "", "", "6,00")),
    ));
    // Descrizione con «à» in Windows-1252.
    let pos = dati.windows(10).position(|w| w == b"<DesEstesa").unwrap() + 11;
    dati.splice(pos..pos + 1, [0xE0u8]);
    let r = prova(&dati).unwrap();
    assert!(!r.rapporto.ha_errori(), "{:#?}", r.rapporto.anomalie);
    assert_eq!(r.rapporto.codifica, "windows-1252");
    assert_eq!(r.progetto.elenco_prezzi.voci[0].descrizione_estesa, "à");
    assert_eq!(
        r.progetto.elenco_prezzi.voci[0].prezzo,
        Prezzo::new(dec!(12.50))
    );
    let rigo = &r.progetto.computi[0].voci[0].righi[0];
    assert_eq!(
        rigo.parti_uguali,
        Some(Fattore::Espressione("2*(1+1)".into()))
    );
    assert_eq!(rigo.lunghezza, Some(Fattore::Valore(dec!(1.5))));
    assert!(
        r.rapporto
            .con_codice("xpwe.espressione_normalizzata")
            .next()
            .is_some()
    );
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(75.00))
    );
}

#[test]
fn zeri_interpretati_come_vuoti_solo_se_confermati_dal_file() {
    // 2 × 3 con larghezza e altezza «0.00»: il file dichiara 6, quindi gli zeri sono campi vuoti.
    let r = prova(&documento(
        &ep("1", "A", "1"),
        &vc("1", "1", &rg("2", "3", "0.00", "0.00", "6.00")),
    ))
    .unwrap();
    assert!(
        r.rapporto
            .con_codice("xpwe.zeri_come_vuoti")
            .next()
            .is_some()
    );
    assert_eq!(r.rapporto.statistiche.quantita_diverse, 0);
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(6.00))
    );
    // Se il file dichiara 0, lo zero è un vero fattore.
    let r = prova(&documento(
        &ep("1", "A", "1"),
        &vc("1", "1", &rg("2", "3", "0", "", "0")),
    ))
    .unwrap();
    assert!(
        r.rapporto
            .con_codice("xpwe.zeri_come_vuoti")
            .next()
            .is_none()
    );
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::ZERO
    );
}

#[test]
fn quantita_dichiarata_diversa_segnalata() {
    let r = prova(&documento(
        &ep("1", "A", "1"),
        &vc("1", "1", &rg("", "3", "2", "", "7.00")),
    ))
    .unwrap();
    let a: Vec<_> = r.rapporto.con_codice("xpwe.quantita_diversa").collect();
    assert_eq!(a.len(), 1);
    assert!(
        a[0].messaggio.contains("7.00") && a[0].messaggio.contains("6.00"),
        "{}",
        a[0].messaggio
    );
    assert_eq!(a[0].gravita, Gravita::Avviso);
    // Prevale il ricalcolo, mai il numero dichiarato.
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(6.00))
    );
}

#[test]
fn espressione_non_valida_bloccata_esplicitamente() {
    let r = prova(&documento(
        &ep("1", "A", "1"),
        &vc("1", "1", &rg("", "2*(3", "", "", "")),
    ))
    .unwrap();
    assert!(
        r.rapporto
            .con_codice("xpwe.espressione_non_valida")
            .next()
            .is_some()
    );
    assert!(r.rapporto.ha_errori());
    // Il rigo resta com'era nel file e il calcolo fallisce con un errore localizzato.
    assert!(r.progetto.calcola_computo(r.computo).is_err());
}

#[test]
fn identificativi_duplicati_e_tariffe_ripetute() {
    let r = prova(&documento(
        &format!(
            "{}{}{}",
            ep("1", "A", "1"),
            ep("1", "B", "2"),
            ep("2", "A", "3")
        ),
        &vc("1", "2", &rg("", "1", "", "", "1")),
    ))
    .unwrap();
    assert!(r.rapporto.con_codice("xpwe.id_duplicato").next().is_some());
    assert!(
        r.rapporto
            .con_codice("xpwe.tariffa_duplicata")
            .next()
            .is_some()
    );
    assert_eq!(r.progetto.elenco_prezzi.voci.len(), 2);
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(3.00))
    );
}

#[test]
fn struttura_con_elemento_intermedio_e_sezioni_per_posizione() {
    let dati = b"<Radice><Documento><PweDatiGenerali/><PweMisurazioni><Elenco><EPItem ID=\"1\"><Tariffa>A</Tariffa><UnMisura>m</UnMisura><Prezzo1>2</Prezzo1></EPItem></Elenco><Voci><VCItem ID=\"1\"><IDEP>1</IDEP><Misure><RGItem><Lunghezza>3</Lunghezza></RGItem></Misure></VCItem></Voci></PweMisurazioni></Documento></Radice>";
    let r = prova(dati).unwrap();
    for c in [
        "xpwe.radice_inattesa",
        "xpwe.struttura_annidata",
        "xpwe.sezione_per_contenuto",
    ] {
        assert!(r.rapporto.con_codice(c).next().is_some(), "manca {c}");
    }
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(6.00))
    );
}

#[test]
fn anagrafica_con_nomi_diversi_e_sezioni_note_non_confuse() {
    let dati = b"<PweDocumento><PweDatiGenerali><PweDGCapitoliCategorie><PweDGCategorie><DGCategorieItem ID=\"1\"><Codice>C</Codice><DesSintetica>Cat</DesSintetica></DGCategorieItem></PweDGCategorie></PweDGCapitoliCategorie><Anagrafica><Dati><Oggetto>Lavori</Oggetto></Dati></Anagrafica></PweDatiGenerali><PweMisurazioni><PweElencoPrezzi/><PweVociComputo/></PweMisurazioni></PweDocumento>";
    let r = prova(dati).unwrap();
    assert_eq!(r.progetto.metadati.titolo, "Lavori");
    assert_eq!(r.progetto.categorie.categorie.len(), 1);
    let xml = opencomputo_format_xpwe::esporta(&r.progetto, r.computo)
        .unwrap()
        .xml;
    assert_eq!(xml.matches("<PweDGCapitoliCategorie>").count(), 1, "{xml}");
    assert!(
        xml.contains("<Anagrafica>") && xml.contains("<Dati>"),
        "{xml}"
    );
    let di_nuovo = prova(xml.as_bytes()).unwrap();
    assert_eq!(di_nuovo.progetto.metadati, r.progetto.metadati);
    assert_eq!(di_nuovo.progetto.categorie, r.progetto.categorie);
    // L'export aggiunge la configurazione dei numeri mancante; da lì in poi è stabile.
    assert!(xml.contains("<PweDGConfigurazione>"));
    let secondo = opencomputo_format_xpwe::esporta(&di_nuovo.progetto, di_nuovo.computo)
        .unwrap()
        .xml;
    assert_eq!(secondo, xml);
}

/// Fatto osservato in un export reale con voci: la `Quantita` di ogni rigo ripete il
/// totale della voce. Non è un'anomalia; un totale di voce sbagliato lo resta.
#[test]
fn quantita_del_rigo_che_ripete_il_totale_della_voce() {
    let voce = |totale: &str, righi: &str| {
        format!(
            "<VCItem ID=\"1\"><IDEP>1</IDEP><Quantita>{totale}</Quantita><PweVCMisure>{righi}</PweVCMisure></VCItem>"
        )
    };
    let righi = format!(
        "{}{}",
        rg("", "", "", "184.32", "303.60"),
        rg("", "", "", "119.28", "303.60")
    );
    let r = prova(&documento(&ep("1", "A", "2"), &voce("303.6", &righi))).unwrap();
    assert_eq!(
        codici(&r),
        Vec::<String>::new(),
        "{:?}",
        r.rapporto.anomalie
    );
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(607.20))
    );
    // Totale dichiarato diverso dalla somma dei righi: resta segnalato, una volta sola.
    let r = prova(&documento(
        &ep("1", "A", "2"),
        &voce("999", &righi.replace("303.60", "999")),
    ))
    .unwrap();
    assert_eq!(r.rapporto.con_codice("xpwe.quantita_diversa").count(), 1);
    assert_eq!(
        r.progetto.calcola_computo(r.computo).unwrap().totale,
        Importo::new(dec!(607.20))
    );
}

/// Un contenitore di analisi vuoto (`<PweEPAR />`) non è un'analisi da segnalare.
#[test]
fn analisi_vuota_non_segnalata() {
    let voce = ep("1", "A", "2").replace(
        "</EPItem>",
        "<PweEPAnalisi><PweEPAR /></PweEPAnalisi></EPItem>",
    );
    let r = prova(&documento(&voce, "")).unwrap();
    assert_eq!(
        r.rapporto.con_codice("xpwe.analisi_non_importata").count(),
        0
    );
}
