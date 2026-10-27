//! Generatore di DCF sintetici per i test.
//!
//! Riproduce la struttura descritta in `contenitore` (ricavata da file reali): serve a
//! provare casi limite senza includere nel repository file di terzi.
#![allow(dead_code)]

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use opencomputo_dcf_parser::contenitore::{
    DIMENSIONE_SETTORE, FIRMA_FILE, FIRMA_FLUSSO, FIRMA_SETTORI, INIZIO_SETTORI,
    MARCATORE_PROTEZIONE, TESTATA_FLUSSO, TESTATA_SETTORE, TIPO_DATI,
};

fn zlib(dati: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(dati).unwrap();
    e.finish().unwrap()
}

fn u32le(v: &mut Vec<u8>, x: usize) {
    v.extend_from_slice(&u32::try_from(x).unwrap().to_le_bytes());
}

/// Dati di un flusso (dalla firma `AACS`), con pezzi da `pezzo` byte.
pub fn flusso(contenuto: &[u8], pezzo: usize) -> Vec<u8> {
    let pezzi: Vec<&[u8]> = if contenuto.is_empty() {
        vec![contenuto]
    } else {
        contenuto.chunks(pezzo).collect()
    };
    let mut f = Vec::new();
    f.extend_from_slice(FIRMA_FLUSSO);
    u32le(&mut f, pezzo);
    u32le(&mut f, 0);
    u32le(&mut f, pezzi.len());
    u32le(&mut f, 0x4000_0000);
    u32le(&mut f, 2);
    f.resize(TESTATA_FLUSSO, 0);
    for (i, p) in pezzi.iter().enumerate() {
        let z = zlib(p);
        if i > 0 {
            u32le(&mut f, z.len());
            u32le(&mut f, p.len());
            u32le(&mut f, 0);
            u32le(&mut f, 0);
        }
        f.extend_from_slice(&z);
    }
    f
}

/// Contenuto di un flusso protetto: marcatore seguito da byte pseudo-casuali.
pub fn protetto(n: usize) -> Vec<u8> {
    let mut v = vec![0x10, 0, 0, 0];
    v.extend_from_slice(MARCATORE_PROTEZIONE);
    let mut x: u32 = 2_463_534_242;
    for _ in 0..n {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push(x.to_le_bytes()[0]);
    }
    v
}

fn settore(tipo: u32, dati: &[u8]) -> Vec<u8> {
    let mut s = vec![0u8; TESTATA_SETTORE];
    s[8..12].copy_from_slice(&tipo.to_le_bytes());
    s.extend_from_slice(dati);
    s.resize(DIMENSIONE_SETTORE, 0);
    s
}

/// File DCF con i flussi indicati (dati già nel formato di [`flusso`]).
pub fn dcf(flussi: &[Vec<u8>]) -> Vec<u8> {
    let mut settori: Vec<Vec<u8>> = Vec::new();
    for tipo in [4, 2, 3, 0, 5] {
        settori.push(settore(tipo, &[]));
    }
    let area = DIMENSIONE_SETTORE - TESTATA_SETTORE;
    for f in flussi {
        for parte in f.chunks(area) {
            settori.push(settore(TIPO_DATI, parte));
        }
        settori.push(settore(5, &[1, 2, 3]));
    }
    let mut file = vec![0u8; INIZIO_SETTORI];
    file[..8].copy_from_slice(FIRMA_FILE);
    file[8..16].copy_from_slice(&[0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
    let mut testata = FIRMA_SETTORI.to_vec();
    testata.resize(0x10, 0);
    u32le(&mut testata, DIMENSIONE_SETTORE);
    testata.resize(0x30, 0);
    u32le(&mut testata, settori.len() + 1);
    file.extend_from_slice(&testata);
    file.resize(INIZIO_SETTORI + DIMENSIONE_SETTORE, 0);
    for s in settori {
        file.extend_from_slice(&s);
    }
    file
}

/// Flusso InfoDoc con le password indicate.
pub fn info_doc(psw_open: &str, psw_mod: &str) -> Vec<u8> {
    format!("\t<InfoDoc>\r\n<DGDatiGenerali TpDoc=\"1\" Vrsn=\"1\" PswOpen=\"{psw_open}\" PswMod=\"{psw_mod}\" />\r\n</InfoDoc>\r\n").into_bytes()
}

/// Dati generali con capitoli e categorie di prova.
pub fn dati_generali() -> Vec<u8> {
    br#"<DatiGenerali>
  <DGDatiGenerali Oprtr="" Cmn="Comune di Prova" Prvnc="ZZ" Ogg="Prezzario di prova 2026" Comm="Ente di prova" Imprs="" />
  <DGCollectionSpCap>
    <ItemSpCap Id="0" DesSnt="&lt;nessuna&gt;" Cod="" />
    <ItemSpCap Id="1" DesSnt="Edilizia" DesEst="Opere edili" Cod="E" />
  </DGCollectionSpCap>
  <DGCollectionCap>
    <ItemCap Id="0" DesSnt="&lt;nessuna&gt;" Cod="" />
    <ItemCap Id="7" DesSnt="" DesEst="Scavi" Cod="E.01" />
  </DGCollectionCap>
  <DGCollectionSbCap><ItemSbCap Id="0" DesSnt="&lt;nessuna&gt;" Cod="" /></DGCollectionSbCap>
  <DGCollectionSpCat><ItemSpCat Id="0" DesSnt="&lt;nessuna&gt;" Cod="" /></DGCollectionSpCat>
  <DGCollectionCat>
    <ItemCat Id="0" DesSnt="&lt;nessuna&gt;" Cod="" />
    <ItemCat Id="3" DesSnt="Strutture" Cod="ST" />
  </DGCollectionCat>
  <DGCollectionSbCat />
  <WBSCap><ItemCap Id="99" DesSnt="fuori dagli elenchi" Cod="X" /></WBSCap>
</DatiGenerali>"#
        .to_vec()
}

/// Elenco prezzi con le voci (attributi già serializzati) indicate.
pub fn elenco(voci: &[&str]) -> Vec<u8> {
    let mut s = String::from("\t<CollectionEP CountId=\"100\" IsIncAnls=\"false\">\r\n");
    for v in voci {
        s.push_str("\t\t<ItemEP ");
        s.push_str(v);
        s.push_str(" />\r\n");
    }
    s.push_str("\t</CollectionEP>\r\n");
    s.into_bytes()
}

/// Voce di prezzo minima.
pub fn voce(id: u32, trf: &str, prz: &str, altri: &str) -> String {
    format!(
        "IdEP=\"{id}\" Trf=\"{trf}\" DesRid=\"{trf}\" DesEst=\"Descrizione {trf}\" UnMsr=\"m\" Prz1=\"{prz}\" {altri}"
    )
}
