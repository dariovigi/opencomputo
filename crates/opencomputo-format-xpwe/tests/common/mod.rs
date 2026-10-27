//! Utilità condivise dai test di integrazione.
#![allow(dead_code)]

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::prelude::*;
use opencomputo_format_xpwe::{OpzioniImport, RisultatoImport, importa};

/// Fixture sintetica scritta a mano.
pub const FIXTURE: &[u8] = include_bytes!("../fixtures/computo_sintetico.xpwe");

/// Importa con un generatore sequenziale che parte da 1.
pub fn importa_da(dati: &[u8]) -> RisultatoImport {
    importa(
        dati,
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport {
            nome_file: Some("computo_sintetico.xpwe".into()),
            ..OpzioniImport::default()
        },
    )
    .unwrap()
}

/// Progetto senza i dati che dipendono dal file (impronta, anomalie con posizione).
pub fn senza_dati_del_file(mut p: Progetto) -> Progetto {
    if let Some(prov) = p.provenienza.as_mut() {
        prov.impronta_sha256 = None;
        prov.anomalie.clear();
    }
    p
}

/// Codici delle anomalie, ordinati.
pub fn codici(r: &RisultatoImport) -> Vec<String> {
    let mut c: Vec<String> = r
        .rapporto
        .anomalie
        .iter()
        .map(|a| a.codice.clone())
        .collect();
    c.sort();
    c
}

/// Documento XPWE minimo con le voci di elenco e di computo indicate.
pub fn documento(elenco: &str, voci: &str) -> Vec<u8> {
    format!(
        "<PweDocumento><PweDatiGenerali/><PweMisurazioni><PweElencoPrezzi>{elenco}</PweElencoPrezzi><PweVociComputo>{voci}</PweVociComputo></PweMisurazioni></PweDocumento>"
    )
    .into_bytes()
}

/// Voce di elenco minima.
pub fn ep(id: &str, tariffa: &str, prezzo: &str) -> String {
    format!(
        "<EPItem ID=\"{id}\"><Tariffa>{tariffa}</Tariffa><DesBreve>{tariffa}</DesBreve><DesEstesa>{tariffa}</DesEstesa><UnMisura>m</UnMisura><Prezzo1>{prezzo}</Prezzo1></EPItem>"
    )
}

/// Voce di computo con i righi indicati (già serializzati).
pub fn vc(id: &str, idep: &str, righi: &str) -> String {
    format!("<VCItem ID=\"{id}\"><IDEP>{idep}</IDEP><PweVCMisure>{righi}</PweVCMisure></VCItem>")
}

/// Rigo di misura: parti uguali, lunghezza, larghezza, altezza/peso, quantità dichiarata.
pub fn rg(pu: &str, l: &str, b: &str, h: &str, q: &str) -> String {
    format!(
        "<RGItem><IDVV>-2</IDVV><Descrizione>r</Descrizione><PartiUguali>{pu}</PartiUguali><Lunghezza>{l}</Lunghezza><Larghezza>{b}</Larghezza><HPeso>{h}</HPeso><Quantita>{q}</Quantita><Flags>0</Flags></RGItem>"
    )
}
