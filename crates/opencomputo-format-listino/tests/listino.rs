//! Import dei listini di esempio con risultati attesi espliciti.
//!
//! Le fixture sono sintetiche: codici, descrizioni e prezzi sono inventati.

use std::collections::BTreeMap;

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::prezzario::{NaturaVoce, Prezzario};
use opencomputo_core::provenienza::{Anomalia, FormatoOrigine};
use opencomputo_core::validazione::Gravita;
use opencomputo_format_listino::{
    ErroreListino, FormatoListino, OpzioniListino, SeparatoreDecimale, importa, importa_csv,
    importa_json,
};

const CSV: &[u8] = include_bytes!("fixtures/listino_esempio.csv");
const JSON: &[u8] = include_bytes!("fixtures/listino_esempio.json");

fn opzioni(nome_file: &str) -> OpzioniListino {
    OpzioniListino {
        nome_file: Some(nome_file.into()),
        ..OpzioniListino::default()
    }
}

fn prezzo(p: &Prezzario, codice: &str) -> String {
    p.voce_per_codice(codice)
        .map(|v| v.prezzo.to_string())
        .unwrap_or_default()
}

/// Codici delle anomalie per posizione.
fn per_posizione(anomalie: &[Anomalia]) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for a in anomalie {
        m.entry(a.posizione.clone().unwrap_or_default())
            .or_default()
            .push(a.codice.clone());
    }
    m
}

#[test]
fn csv_di_esempio() {
    let r = importa_csv(
        CSV,
        &mut GeneratoreSequenziale::default(),
        &opzioni("listino_esempio.csv"),
    )
    .unwrap();
    let rap = &r.rapporto;
    let p = &r.prezzario;

    assert_eq!(rap.formato, FormatoListino::Csv);
    assert_eq!(rap.codifica, "utf-8");
    assert_eq!(rap.separatore_campi, Some(';'));
    assert_eq!(rap.separatore_decimale, SeparatoreDecimale::Virgola);
    assert_eq!(rap.righe_dati, 12);
    assert_eq!(rap.righe_vuote, 1);
    assert_eq!(rap.voci_importate, 7);
    assert_eq!(rap.righe_scartate, 5);
    assert_eq!(rap.righe_dati, rap.voci_importate + rap.righe_scartate);

    let codici: Vec<&str> = p.voci.iter().map(|v| v.codice.as_str()).collect();
    assert_eq!(
        codici,
        ["E.01", "E.02", "M.01.01", "M.01.02", "M.02", "S.01", "S.02"]
    );
    assert_eq!(prezzo(p, "E.01"), "8.50");
    assert_eq!(prezzo(p, "M.01.01"), "1234.56");
    assert_eq!(prezzo(p, "M.02"), "0.00");

    let e01 = p.voce_per_codice("E.01").unwrap();
    assert_eq!(e01.unita_misura, "m³");
    assert_eq!(
        e01.incidenza_manodopera.map(|x| x.to_string()).as_deref(),
        Some("25.5")
    );
    let e02 = p.voce_per_codice("E.02").unwrap();
    assert_eq!(
        e02.descrizione_estesa,
        "Scavo a sezione obbligata; fino a 2 m di profondità"
    );
    let prov = e02.provenienza.as_ref().unwrap();
    assert_eq!(prov.identificativo_originale.as_deref(), Some("riga 3"));
    assert_eq!(
        prov.dati_originali.get("Note").map(String::as_str),
        Some("terreno sciolto")
    );
    assert_eq!(
        p.voce_per_codice("M.01.01").unwrap().descrizione_estesa,
        "Muratura in blocchi\ndi laterizio, spessore 30 cm"
    );
    // Incidenza oltre 100: ignorata, valore originale conservato.
    let m0102 = p.voce_per_codice("M.01.02").unwrap();
    assert_eq!(m0102.incidenza_manodopera, None);
    assert_eq!(
        m0102
            .provenienza
            .as_ref()
            .unwrap()
            .dati_originali
            .get("incidenza manodopera")
            .map(String::as_str),
        Some("150")
    );
    assert_eq!(
        p.voce_per_codice("S.01").unwrap().natura,
        NaturaVoce::OneriSicurezza
    );
    assert_eq!(
        p.voce_per_codice("S.02").unwrap().natura,
        NaturaVoce::Lavoro
    );

    // Capitoli: X non esiste perché la sua unica riga è stata scartata; M.01 è figlio di M.
    let caps: Vec<&str> = p.capitoli.iter().map(|c| c.codice.as_str()).collect();
    assert_eq!(caps, ["E", "M", "M.01", "S"]);
    let m = p.capitoli.iter().find(|c| c.codice == "M").unwrap();
    let m01 = p.capitoli.iter().find(|c| c.codice == "M.01").unwrap();
    assert_eq!(m01.padre, Some(m.id));
    assert_eq!(m01.descrizione, "Murature in laterizio");
    assert_eq!(rap.capitoli, 4);

    let attese: BTreeMap<String, Vec<String>> = [
        ("riga 4", vec!["listino.codice_duplicato"]),
        ("riga 5", vec!["listino.codice_vuoto"]),
        ("riga 6", vec!["listino.prezzo_non_valido"]),
        ("riga 7", vec!["listino.prezzo_negativo"]),
        ("riga 11", vec!["listino.incidenza_non_valida"]),
        ("riga 12", vec!["listino.prezzo_zero"]),
        ("riga 14", vec!["listino.natura_sconosciuta"]),
        ("riga 15", vec!["listino.campi_in_eccesso"]),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.into_iter().map(str::to_owned).collect()))
    .collect();
    assert_eq!(per_posizione(&rap.anomalie), attese);
    let errori = rap
        .anomalie
        .iter()
        .filter(|a| a.gravita == Gravita::Errore)
        .count();
    assert_eq!(errori, rap.righe_scartate);

    let prov = p.provenienza.as_ref().unwrap();
    assert_eq!(prov.formato, FormatoOrigine::Csv);
    assert_eq!(prov.file.as_deref(), Some("listino_esempio.csv"));
    assert_eq!(
        prov.impronta_sha256.as_deref(),
        Some(rap.impronta_sha256.as_str())
    );
    assert_eq!(rap.impronta_sha256.len(), 64);
    assert_eq!(p.nome, "listino_esempio");
}

#[test]
fn json_di_esempio() {
    let r = importa_json(
        JSON,
        &mut GeneratoreSequenziale::default(),
        &opzioni("listino.json"),
    )
    .unwrap();
    let rap = &r.rapporto;
    let p = &r.prezzario;

    assert_eq!(rap.formato, FormatoListino::Json);
    assert_eq!(rap.separatore_campi, None);
    assert_eq!(rap.separatore_decimale, SeparatoreDecimale::Virgola);
    assert_eq!(rap.righe_dati, 8);
    assert_eq!(rap.voci_importate, 4);
    assert_eq!(rap.righe_scartate, 4);

    assert_eq!(p.nome, "Listino di prova");
    assert_eq!(p.ente.as_deref(), Some("Ente sintetico"));
    assert_eq!(p.edizione.as_deref(), Some("2026"));
    assert_eq!(
        p.data_pubblicazione.map(|d| d.to_string()).as_deref(),
        Some("2026-01-15")
    );
    assert_eq!(
        p.provenienza.as_ref().map(|x| x.formato.clone()),
        Some(FormatoOrigine::Altro("json".into()))
    );

    // Numero JSON letto con il punto anche se le stringhe usano la virgola.
    assert_eq!(prezzo(p, "E.01.001"), "14.2");
    assert_eq!(prezzo(p, "E.01.002"), "6.80");
    assert_eq!(prezzo(p, "P.001"), "45.00");
    assert_eq!(prezzo(p, "S.001"), "12");
    let e1 = p.voce_per_codice("E.01.001").unwrap();
    assert_eq!(
        e1.incidenza_manodopera.map(|x| x.to_string()).as_deref(),
        Some("30.5")
    );
    let e2 = p.voce_per_codice("E.01.002").unwrap();
    assert_eq!(e2.descrizione_estesa, "Rinterro");
    let orig = &e2.provenienza.as_ref().unwrap().dati_originali;
    assert_eq!(
        orig.get("codice_regionale").map(String::as_str),
        Some("R-77")
    );
    assert_eq!(orig.get("tag").map(String::as_str), Some("[\"a\",\"b\"]"));
    assert_eq!(
        p.voce_per_codice("S.001").unwrap().natura,
        NaturaVoce::OneriSicurezza
    );

    let caps: Vec<(&str, Option<&str>)> = p
        .capitoli
        .iter()
        .map(|c| {
            let padre = c
                .padre
                .and_then(|id| p.capitoli.iter().find(|x| x.id == id))
                .map(|x| x.codice.as_str());
            (c.codice.as_str(), padre)
        })
        .collect();
    assert_eq!(
        caps,
        [
            ("E", None),
            ("E.01", Some("E")),
            ("P", None),
            ("S", None),
            ("Z", None)
        ]
    );
    let cap_e1 = e1
        .capitolo
        .and_then(|id| p.capitoli.iter().find(|c| c.id == id));
    assert_eq!(cap_e1.map(|c| c.codice.as_str()), Some("E.01"));

    let attese: BTreeMap<String, Vec<String>> = [
        ("fonte", vec!["listino.campo_sconosciuto"]),
        ("voci[3]", vec!["listino.codice_duplicato"]),
        ("voci[4]", vec!["listino.prezzo_non_valido"]),
        (
            "voci[5]",
            vec!["listino.campo_non_scalare", "listino.prezzo_non_valido"],
        ),
        ("voci[6]", vec!["listino.elemento_non_oggetto"]),
        ("voci[7]", vec!["listino.capitolo_non_dichiarato"]),
        ("capitoli", vec!["listino.capitolo_padre_inesistente"]),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.into_iter().map(str::to_owned).collect()))
    .collect();
    assert_eq!(per_posizione(&rap.anomalie), attese);
}

#[test]
fn riconoscimento_del_formato() {
    let mut g = GeneratoreSequenziale::default();
    let o = OpzioniListino::default();
    assert_eq!(
        importa(CSV, &mut g, &o).unwrap().rapporto.formato,
        FormatoListino::Csv
    );
    assert_eq!(
        importa(JSON, &mut g, &o).unwrap().rapporto.formato,
        FormatoListino::Json
    );
}

#[test]
fn stesso_file_stessa_impronta_e_stesso_risultato() {
    let a = importa_csv(
        CSV,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap();
    let b = importa_csv(
        CSV,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn codifica_windows_1252() {
    // «Profondità» con la «à» in Windows-1252 (0xE0), separatore a virgola.
    let mut byte = b"Codice,Descrizione,UM,Prezzo\nA,Profondit".to_vec();
    byte.push(0xE0);
    byte.extend_from_slice(b",m,1.50\n");
    let r = importa_csv(
        &byte,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap();
    assert_eq!(r.rapporto.codifica, "windows-1252");
    assert_eq!(r.rapporto.separatore_campi, Some(','));
    assert_eq!(r.rapporto.separatore_decimale, SeparatoreDecimale::Punto);
    assert_eq!(
        r.prezzario.voci.first().unwrap().descrizione_estesa,
        "Profondità"
    );
    assert_eq!(prezzo(&r.prezzario, "A"), "1.50");
    assert_eq!(
        r.rapporto.anomalie.first().unwrap().codice,
        "listino.codifica"
    );
}

#[test]
fn separatore_decimale_imposto() {
    // Con la virgola imposta, «1.250» sono milleduecentocinquanta euro.
    let csv = b"Codice;Descrizione;UM;Prezzo\nA;Voce;cad;1.250\n";
    let o = OpzioniListino {
        separatore_decimale: SeparatoreDecimale::Virgola,
        ..OpzioniListino::default()
    };
    let r = importa_csv(csv, &mut GeneratoreSequenziale::default(), &o).unwrap();
    assert_eq!(prezzo(&r.prezzario, "A"), "1250");
    // Riconoscimento automatico: nessuna virgola nei prezzi, quindi punto decimale.
    let r = importa_csv(
        csv,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap();
    assert_eq!(prezzo(&r.prezzario, "A"), "1.250");
}

#[test]
fn colonne_personalizzate() {
    let csv = b"Art.;Testo;Misura;Euro unitari\nA;Voce;m;2,00\n";
    let mut o = OpzioniListino::default();
    o.colonne.codice.push("Art.".into());
    o.colonne.descrizione.push("Testo".into());
    o.colonne.unita_misura.push("Misura".into());
    o.colonne.prezzo.push("Euro unitari".into());
    let r = importa_csv(csv, &mut GeneratoreSequenziale::default(), &o).unwrap();
    assert_eq!(prezzo(&r.prezzario, "A"), "2.00");
}

#[test]
fn errori_del_file() {
    let mut g = GeneratoreSequenziale::default();
    let o = OpzioniListino::default();
    assert_eq!(importa_csv(b"", &mut g, &o), Err(ErroreListino::Vuoto));
    // Una riga di soli separatori è vuota: nessuna intestazione.
    assert_eq!(
        importa_csv(b"\n;;\n", &mut g, &o),
        Err(ErroreListino::Vuoto)
    );
    assert_eq!(
        importa_csv(b"Codice;Descrizione;UM;Prezzo\n", &mut g, &o),
        Err(ErroreListino::Vuoto)
    );
    assert!(matches!(
        importa_csv(b"Codice;Descrizione;UM;Prezzo\nA;;m;x\n", &mut g, &o),
        Err(ErroreListino::NessunaVoceValida { scartate: 1, .. })
    ));
    assert!(matches!(
        importa_csv(b"Codice;Descrizione\n", &mut g, &o),
        Err(ErroreListino::ColonneMancanti { .. })
    ));
    assert!(matches!(
        importa_json(b"{", &mut g, &o),
        Err(ErroreListino::Json { .. })
    ));
    assert!(matches!(
        importa_json(b"{\"nome\": \"x\"}", &mut g, &o),
        Err(ErroreListino::Json { .. })
    ));
    assert!(matches!(
        importa_json(b"42", &mut g, &o),
        Err(ErroreListino::Json { .. })
    ));
}

#[test]
fn elenco_json_semplice() {
    let json =
        br#"[{"codice": "A", "descrizione": "Voce", "unita_misura": "cad", "prezzo": "3.10"}]"#;
    let r = importa_json(
        json,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap();
    assert_eq!(r.prezzario.nome, "Listino");
    assert_eq!(prezzo(&r.prezzario, "A"), "3.10");
}
