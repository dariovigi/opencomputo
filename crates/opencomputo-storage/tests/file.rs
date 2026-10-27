//! Salvataggio e apertura su filesystem, in una cartella temporanea per test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use opencomputo_core::progetto::Progetto;
use opencomputo_core::schema::importa_json;
use opencomputo_storage::{
    Compressione, ErroreStorage, OpzioniSalvataggio, apri, codifica, copia_di_sicurezza,
    decodifica, salva,
};

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json");

fn progetto() -> Progetto {
    importa_json(FIXTURE).unwrap()
}

/// Cartella di prova vuota, distinta per test.
fn cartella(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("opencomputo-storage-{}-{nome}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn file_nella(d: &PathBuf) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn salva_e_riapri_in_chiaro_e_compresso() {
    let d = cartella("andata");
    let p = progetto();
    for (nome, c) in [
        ("chiaro.ocp", Compressione::Nessuna),
        ("compresso.ocp", Compressione::Gzip),
    ] {
        let percorso = d.join(nome);
        let esito = salva(
            &percorso,
            &p,
            &OpzioniSalvataggio {
                compressione: c,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(esito.copia_di_sicurezza, None);
        assert_eq!(
            esito.byte as u64,
            std::fs::metadata(&percorso).unwrap().len()
        );
        assert_eq!(apri(&percorso).unwrap(), p);
    }
    let chiaro = std::fs::read(d.join("chiaro.ocp")).unwrap();
    let compresso = std::fs::read(d.join("compresso.ocp")).unwrap();
    assert!(chiaro.starts_with(b"{"));
    assert!(compresso.starts_with(&[0x1f, 0x8b]));
    assert!(compresso.len() * 3 < chiaro.len());
    // Nessun file temporaneo rimasto.
    assert_eq!(file_nella(&d), ["chiaro.ocp", "compresso.ocp"]);
    // Contenuto deterministico.
    assert_eq!(codifica(&p, Compressione::Gzip).unwrap(), compresso);
}

#[test]
fn copie_di_sicurezza_a_rotazione() {
    let d = cartella("copie");
    let percorso = d.join("lavori.ocp");
    let mut p = progetto();
    let o = OpzioniSalvataggio {
        copie_di_sicurezza: 2,
        ..Default::default()
    };
    for i in 1..=4 {
        p.metadati.titolo = format!("versione {i}");
        let esito = salva(&percorso, &p, &o).unwrap();
        assert_eq!(esito.copia_di_sicurezza.is_some(), i > 1);
    }
    assert_eq!(
        file_nella(&d),
        ["lavori.ocp", "lavori.ocp.bak1", "lavori.ocp.bak2"]
    );
    let titolo = |n: usize| {
        apri(&copia_di_sicurezza(&percorso, n))
            .unwrap()
            .metadati
            .titolo
    };
    assert_eq!(apri(&percorso).unwrap().metadati.titolo, "versione 4");
    assert_eq!(titolo(1), "versione 3");
    assert_eq!(titolo(2), "versione 2");

    let o = OpzioniSalvataggio {
        copie_di_sicurezza: 0,
        ..Default::default()
    };
    p.metadati.titolo = "versione 5".into();
    assert_eq!(salva(&percorso, &p, &o).unwrap().copia_di_sicurezza, None);
    assert_eq!(titolo(1), "versione 3");
}

#[test]
fn un_errore_lascia_intatto_il_file_precedente() {
    let d = cartella("errore");
    let percorso = d.join("lavori.ocp");
    let p = progetto();
    salva(&percorso, &p, &OpzioniSalvataggio::default()).unwrap();
    let prima = std::fs::read(&percorso).unwrap();
    // Destinazione in una cartella inesistente: errore, nessun file creato altrove.
    let e = salva(
        &d.join("manca/lavori.ocp"),
        &p,
        &OpzioniSalvataggio::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            e,
            ErroreStorage::Io {
                operazione: "creazione di",
                ..
            }
        ),
        "{e}"
    );
    assert_eq!(std::fs::read(&percorso).unwrap(), prima);
    assert_eq!(file_nella(&d), ["lavori.ocp"]);
}

#[test]
fn file_assente_danneggiato_o_di_versione_futura() {
    let d = cartella("lettura");
    assert!(matches!(
        apri(&d.join("assente.ocp")),
        Err(ErroreStorage::Io {
            operazione: "lettura di",
            ..
        })
    ));
    let mut gz = codifica(&progetto(), Compressione::Gzip).unwrap();
    let n = gz.len();
    gz.truncate(n / 2);
    assert!(matches!(decodifica(&gz), Err(ErroreStorage::Contenuto(_))));
    let futuro = FIXTURE.replacen("\"versione_schema\": 1", "\"versione_schema\": 99", 1);
    assert!(matches!(
        decodifica(futuro.as_bytes()),
        Err(ErroreStorage::Schema(
            opencomputo_core::schema::ErroreSchema::VersioneFutura { .. }
        ))
    ));
    // La fixture v1 si apre (BOM tollerato).
    let mut con_bom = vec![0xEF, 0xBB, 0xBF];
    con_bom.extend_from_slice(FIXTURE.as_bytes());
    assert_eq!(decodifica(&con_bom).unwrap(), progetto());
}
