//! Export sperimentale dell'elenco prezzi in DCF (ADR 0016): struttura coerente e
//! andata e ritorno con il lettore.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use opencomputo_core::data::OperazioniData;
use opencomputo_core::data::data_amg;
use opencomputo_core::id::{CapitoloId, GeneratoreId, GeneratoreSequenziale};
use opencomputo_core::numeri::{Percentuale, Prezzo};
use opencomputo_core::prezzario::{CapitoloPrezzario, NaturaVoce};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::schema::importa_json;
use opencomputo_dcf_parser::esporta::{OpzioniExport, esporta};
use opencomputo_dcf_parser::{OpzioniImport, contenitore, importa, struttura};
use rust_decimal_macros::dec;
use std::sync::Arc;

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v5.json");

fn opzioni() -> OpzioniExport {
    OpzioniExport {
        data: data_amg(2026, 10, 21).unwrap().alle(16, 30, 0).unwrap(),
        versione_programma: "0.1.0".into(),
    }
}

fn rileggi(byte: &[u8]) -> Progetto {
    importa(
        byte,
        &mut GeneratoreSequenziale::new(90_000),
        &OpzioniImport::default(),
    )
    .unwrap()
    .progetto
}

fn progetto() -> Progetto {
    let mut p = importa_json(FIXTURE).unwrap();
    p.metadati.oggetto = Some("Nuova scuola primaria: \"lotto 1\" & opere esterne".into());
    p
}

#[test]
fn elenco_esportato_e_riletto() {
    let p = progetto();
    let r = esporta(&p, &opzioni()).unwrap();
    assert_eq!(r.voci, p.elenco_prezzi.voci.len());
    assert!(
        r.anomalie
            .iter()
            .any(|a| a.codice == "dcf.export_sperimentale")
    );
    assert!(
        r.anomalie
            .iter()
            .any(|a| a.codice == "dcf.computo_non_esportato")
    );

    // Struttura: somme di controllo, mappa, CRC dei pezzi.
    assert_eq!(struttura::verifica(&r.byte).unwrap(), Vec::<String>::new());
    let s = struttura::leggi(&r.byte).unwrap();
    let percorsi: Vec<String> = (0..s.voci.len()).map(|i| s.percorso(i)).collect();
    assert_eq!(
        percorsi,
        [
            "Documento",
            "Documento/CollectionEP.xml",
            "Documento/CollectionGP.xml",
            "Documento/CollectionOD.xml",
            "Documento/CollectionVC.xml",
            "Documento/DatiGenerali.xml",
            "Documento/InfoDoc.xml",
            "Documento/InfoMRR.xml",
            "Versione",
            "VersioneFix",
        ]
    );
    // Nessun flusso protetto e nessuna dichiarazione di copyright di terzi.
    let c = contenitore::leggi(&r.byte).unwrap();
    assert_eq!(c.protetti(), 0);
    let info = c.xml("InfoDoc").next().unwrap();
    assert!(info.contains("CopyRight=\"\""), "{info}");
    assert!(info.contains("SrcNm=\"OpenComputo\""));
    assert!(info.contains("Vrsn=\"5.01\""));
    let ep = c.xml("CollectionEP").next().unwrap();
    assert!(ep.starts_with("\t<CollectionEP CountId=\"8\" IsIncAnls=\"false\" >\r\n"));
    assert_eq!(ep.matches("<ItemEP ").count(), 6);
    // Attributi nell'ordine dei prezzari.
    let prima = ep.lines().nth(1).unwrap();
    assert!(
        prima
            .trim_start()
            .starts_with("<ItemEP IdEP=\"2\" IdSrc=\"0\" Trf=\"")
    );
    assert!(prima.trim_end().ends_with("TagBIM=\"\" IdWBS=\"\" />"));
    assert_eq!(prima.matches("=\"").count(), 59);

    // Andata e ritorno.
    let q = rileggi(&r.byte);
    assert_eq!(q.elenco_prezzi.voci.len(), p.elenco_prezzi.voci.len());
    for (a, b) in p.elenco_prezzi.voci.iter().zip(&q.elenco_prezzi.voci) {
        assert_eq!(a.codice, b.codice);
        assert_eq!(a.unita_misura, b.unita_misura);
        assert_eq!(a.prezzo, b.prezzo, "{}", a.codice);
        assert_eq!(a.natura, b.natura, "{}", a.codice);
        assert_eq!(
            a.incidenza_manodopera, b.incidenza_manodopera,
            "{}",
            a.codice
        );
        assert_eq!(a.incidenza_sicurezza, b.incidenza_sicurezza, "{}", a.codice);
        let estesa = if a.descrizione_estesa.is_empty() {
            &a.descrizione_breve
        } else {
            &a.descrizione_estesa
        };
        assert_eq!(&b.descrizione_estesa, estesa);
    }
    assert_eq!(q.metadati.oggetto, p.metadati.oggetto);
    assert_eq!(q.metadati.committente, p.metadati.committente);
    assert_eq!(q.metadati.luogo, p.metadati.luogo);
    assert_eq!(
        q.categorie.super_categorie.len(),
        p.categorie.super_categorie.len()
    );
    assert_eq!(q.categorie.categorie.len(), p.categorie.categorie.len());

    // Deterministico.
    assert_eq!(esporta(&p, &opzioni()).unwrap().byte, r.byte);
}

#[test]
fn capitoli_su_tre_livelli_e_testi_speciali() {
    let mut p = progetto();
    let mut g = GeneratoreSequenziale::new(70_000);
    let mut capitolo = |padre: Option<CapitoloId>, codice: &str| -> CapitoloId {
        let id: CapitoloId = g.nuovo();
        p.elenco_prezzi.capitoli.push(CapitoloPrezzario {
            id,
            padre,
            codice: codice.into(),
            descrizione: format!("Capitolo {codice}"),
        });
        id
    };
    let a = capitolo(None, "A");
    let a1 = capitolo(Some(a), "A.1");
    let a11 = capitolo(Some(a1), "A.1.1");
    let a111 = capitolo(Some(a11), "A.1.1.1");
    let v = &mut p.elenco_prezzi.voci;
    Arc::make_mut(&mut v[0]).capitolo = Some(a11);
    Arc::make_mut(&mut v[1]).capitolo = Some(a1);
    Arc::make_mut(&mut v[2]).capitolo = Some(a111);
    Arc::make_mut(&mut v[3]).natura = NaturaVoce::OneriSicurezza;
    Arc::make_mut(&mut v[3]).incidenza_manodopera = Some(Percentuale::new(dec!(12.5)));
    Arc::make_mut(&mut v[0]).descrizione_estesa =
        "Scavo <a sezione> con l'uso \"di\" mezzi & attrezzi\nseconda riga, città".into();
    Arc::make_mut(&mut v[1]).prezzo = Prezzo::new(dec!(1234.567));
    let r = esporta(&p, &opzioni()).unwrap();
    assert!(
        r.anomalie
            .iter()
            .any(|a| a.codice == "dcf.capitoli_oltre_tre_livelli")
    );
    assert!(struttura::verifica(&r.byte).unwrap().is_empty());
    let c = contenitore::leggi(&r.byte).unwrap();
    let ep = c.xml("CollectionEP").next().unwrap();
    // Una voce per elemento (le descrizioni possono andare a capo).
    let riga = |codice: &str| {
        ep.split("<ItemEP ")
            .find(|l| l.contains(&format!("Trf=\"{codice}\"")))
            .unwrap()
            .to_owned()
    };
    let codici: Vec<String> = p
        .elenco_prezzi
        .voci
        .iter()
        .map(|v| v.codice.clone())
        .collect();
    assert!(riga(&codici[0]).contains("IdSpCap=\"1\" IdCap=\"1\" IdSbCap=\"1\""));
    assert!(riga(&codici[1]).contains("IdSpCap=\"1\" IdCap=\"1\" IdSbCap=\"0\""));
    // Quarto livello scritto come terzo.
    assert!(riga(&codici[2]).contains("IdSpCap=\"1\" IdCap=\"1\" IdSbCap=\"2\""));
    assert!(riga(&codici[3]).contains("Fgs=\"134217728\""));
    assert!(riga(&codici[3]).contains("MDOInc=\"12.500000000000000000\""));
    let g = c.xml("DatiGenerali").next().unwrap();
    assert!(g.contains("<ItemSbCap Id=\"2\" DesSnt=\"Capitolo A.1.1.1\""));
    assert!(g.contains("&lt;nessuna&gt;"));

    let q = rileggi(&r.byte);
    let b = &q.elenco_prezzi.voci;
    // L'a capo è scritto come nei file del programma di riferimento (CR LF nel valore
    // dell'attributo); il lettore normalizza i valori come vuole XML, in uno spazio.
    assert!(ep.contains("attrezzi\r\nseconda riga"));
    assert_eq!(
        b[0].descrizione_estesa,
        p.elenco_prezzi.voci[0]
            .descrizione_estesa
            .replace('\n', " ")
    );
    assert_eq!(b[1].prezzo.valore(), dec!(1234.567));
    assert_eq!(b[3].natura, NaturaVoce::OneriSicurezza);
    assert_eq!(q.elenco_prezzi.capitoli.len(), 4);
    let cap = |i: usize| {
        let id = b[i].capitolo.unwrap();
        q.elenco_prezzi
            .capitoli
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .codice
            .clone()
    };
    // Il lettore assegna il livello più profondo.
    assert_eq!(cap(0), "A.1.1");
    assert_eq!(cap(1), "A.1");
    assert_eq!(cap(2), "A.1.1.1");
}

#[test]
fn elenco_vuoto_rifiutato() {
    let mut p = progetto();
    p.elenco_prezzi.voci.clear();
    assert!(esporta(&p, &opzioni()).is_err());
}

/// Regressione (campagna di test del 2026-10-22): una testata di flusso danneggiata
/// chiedeva un'allocazione di decine di gigabyte e terminava il processo.
#[test]
fn testate_dei_flussi_danneggiate_rifiutate_senza_allocare() {
    use opencomputo_dcf_parser::scrittura;
    let f = scrittura::flusso(&[0u8; 70_000]).unwrap();
    assert!(struttura::decomprimi(&f).is_ok());
    let mut g = f.clone();
    g[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(struttura::pezzi(&g).is_err());
    let mut g = f.clone();
    g[0x84..0x88].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(struttura::decomprimi(&g).is_err());
    // Lunghezza dichiarata più piccola del contenuto: la lettura si ferma subito.
    let mut g = f;
    g[0x84..0x88].copy_from_slice(&10u32.to_le_bytes());
    assert!(
        struttura::decomprimi(&g)
            .unwrap_err()
            .contains("dichiarati")
    );
}
