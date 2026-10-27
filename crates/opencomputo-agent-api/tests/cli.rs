//! Il binario `opencomputo`: uscita JSON e codici di uscita.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
);
const XPWE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../opencomputo-format-xpwe/tests/fixtures/computo_sintetico.xpwe"
);

fn cartella(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("opencomputo-cli-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Esegue il binario; restituisce JSON e codice di uscita.
fn oc(argomenti: &[&str], stdin: Option<&str>) -> (Value, i32) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_opencomputo"))
        .args(argomenti)
        .arg("--compatto")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ingresso = c.stdin.take().unwrap();
    if let Some(t) = stdin {
        ingresso.write_all(t.as_bytes()).unwrap();
    }
    drop(ingresso);
    let o = c.wait_with_output().unwrap();
    let testo = String::from_utf8(o.stdout).unwrap();
    let v = serde_json::from_str(&testo)
        .unwrap_or_else(|e| panic!("{e}: {testo:?} {}", String::from_utf8_lossy(&o.stderr)));
    (v, o.status.code().unwrap())
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn progetto_completo_da_riga_di_comando() {
    let d = cartella("completo");
    let f = d.join("p.ocp");
    let (v, c) = oc(&["nuovo", s(&f), "--titolo", "Scuola"], None);
    assert_eq!((v["ok"].clone(), c), (Value::Bool(true), 0));
    let (v, c) = oc(&["nuovo", s(&f)], None);
    assert_eq!(c, 3);
    assert!(
        v["errore"]["descrizione"]
            .as_str()
            .unwrap()
            .contains("esiste già")
    );

    let comandi = d.join("comandi.json");
    std::fs::write(
        &comandi,
        r#"{"comandi": [
            {"comando": "aggiungi_voce_prezzario", "codice": "A.1", "unita_misura": "m", "prezzo": "2.50"},
            {"comando": "aggiungi_voce_computo", "codice": "A.1", "righi": [{"lunghezza": "4"}]}]}"#,
    )
    .unwrap();
    let (v, c) = oc(
        &["esegui", s(&f), "--comandi", s(&comandi), "--prova"],
        None,
    );
    assert_eq!(c, 0);
    assert_eq!(v["risultato"]["applicato"], false);
    let (v, _) = oc(&["info", s(&f)], None);
    assert_eq!(
        v["risultato"]["elenco_prezzi"]["voci"], 0,
        "la prova non salva"
    );
    let (v, c) = oc(&["esegui", s(&f), "--comandi", s(&comandi)], None);
    assert_eq!(c, 0);
    assert_eq!(v["risultato"]["totali"][0]["dopo"], "10.00");
    assert!(
        v["risultato"]["salvataggio"]["copia_di_sicurezza"]
            .as_str()
            .unwrap()
            .ends_with("p.ocp.bak1")
    );

    // Comando singolo da stdin; rifiuto con codice 1.
    let (v, c) = oc(
        &["esegui", s(&f)],
        Some(r#"{"comando": "rimuovi_voce_computo", "voce": 9}"#),
    );
    assert_eq!(c, 1);
    assert_eq!(v["errore"]["codice"], "comando");
    let (v, c) = oc(
        &["esegui", s(&f)],
        Some("[{\"comando\": \"aggiungi_rigo\", \"voce\": 1, \"rigo\": {\"lunghezza\": \"(\"}}]"),
    );
    assert_eq!(c, 1);
    assert_eq!(v["errore"]["codice"], "nuovi_errori");
    let (v, c) = oc(
        &["esegui", s(&f), "--permissivo"],
        Some("[{\"comando\": \"aggiungi_rigo\", \"voce\": 1, \"rigo\": {\"lunghezza\": \"(\"}}]"),
    );
    assert_eq!(c, 0, "{v}");
    let (v, c) = oc(&["valida", s(&f)], None);
    assert_eq!(c, 1);
    assert_eq!(v["ok"], true);
    assert_eq!(v["risultato"]["valido"], false);

    let (v, c) = oc(&["esegui", s(&f)], Some("{ rotto"));
    assert_eq!(c, 1);
    assert!(
        v["errore"]["descrizione"]
            .as_str()
            .unwrap()
            .starts_with("JSON non valido")
    );
}

#[test]
fn import_ricerca_report_ed_export() {
    let d = cartella("formati");
    let f = d.join("x.ocp");
    let (v, c) = oc(&["importa", XPWE, s(&f), "--gzip"], None);
    assert_eq!(c, 0, "{v}");
    assert_eq!(
        v["risultato"]["progetto"]["computi"][0]["totale"],
        "9291.84"
    );
    assert_eq!(v["risultato"]["salvataggio"]["gzip"], true);
    let (v, _) = oc(&["cerca", s(&f), "E.01"], None);
    assert!(v["risultato"]["trovate"].as_u64().unwrap() >= 1);

    // Prezzario esterno: il progetto della fixture del core convertito in .ocp.
    let fixture = d.join("fixture.ocp");
    let p =
        opencomputo_core::schema::importa_json(&std::fs::read_to_string(FIXTURE).unwrap()).unwrap();
    opencomputo_storage::salva(
        &fixture,
        &p,
        &opencomputo_storage::OpzioniSalvataggio::default(),
    )
    .unwrap();
    let (v, _) = oc(&["cerca", s(&f), "scavo", "--prezzario", s(&fixture)], None);
    assert_eq!(v["risultato"]["voci"][0]["codice"], "A.01.001");
    let (v, c) = oc(
        &[
            "esegui",
            s(&f),
            "--prezzario",
            &format!("fx={}", s(&fixture)),
        ],
        Some(r#"[{"comando": "copia_da_prezzario", "prezzario": "fx", "codici": ["A.01.001"]}]"#),
    );
    assert_eq!(c, 0, "{v}");
    let (_, c) = oc(
        &["esegui", s(&f), "--prezzario", "senza-uguale"],
        Some("[]"),
    );
    assert_eq!(c, 1);

    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "sal",
            "--numero",
            "2",
            "--uscita",
            s(&d.join("r")),
            "--data",
            "2026-05-06",
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    assert!(
        std::fs::read(d.join("r/sal_2.pdf"))
            .unwrap()
            .starts_with(b"%PDF")
    );
    let (_, c) = oc(&["report", s(&fixture), "sal", "--numero", "7"], None);
    assert_eq!(c, 1);
    // Modello tradizionale (solo PDF).
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "computo",
            "--tradizionale",
            "--ente",
            "Comune di Esempio\\nProvincia di Esempio",
            "--studio",
            "Studio tecnico",
            "--uscita",
            s(&d.join("r")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    assert!(
        std::fs::read(d.join("r/computo_tradizionale.pdf"))
            .unwrap()
            .starts_with(b"%PDF")
    );
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "elenco-prezzi",
            "--tradizionale",
            "--senza-frontespizio",
            "--uscita",
            s(&d.join("r")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    assert!(d.join("r/elenco_prezzi_tradizionale.pdf").exists());
    let (_, c) = oc(
        &[
            "report",
            s(&fixture),
            "computo",
            "--tradizionale",
            "--formato",
            "xlsx",
        ],
        None,
    );
    assert_eq!(c, 1);
    // Anche gli altri documenti hanno il modello tradizionale (ADR 0023).
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "registro",
            "--tradizionale",
            "--uscita",
            s(&d.join("r")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    assert!(
        std::fs::read(d.join("r/registro_tradizionale.pdf"))
            .unwrap()
            .starts_with(b"%PDF")
    );
    let (_, c) = oc(
        &[
            "report",
            s(&fixture),
            "registro",
            "--tradizionale",
            "--senza-prezzi",
        ],
        None,
    );
    assert_eq!(c, 1);
    // Incidenze di manodopera e sicurezza (solo computo).
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "computo",
            "--manodopera",
            "--sicurezza",
            "--subtotali",
            "--formato",
            "csv",
            "--uscita",
            s(&d.join("inc")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    let csv = std::fs::read_to_string(d.join("inc/computo_1_computo.csv")).unwrap();
    assert!(
        csv.contains("Manodopera €") && csv.contains(";3759,29;26,00;"),
        "{csv}"
    );
    let (v, c) = oc(&["report", s(&fixture), "libretto", "--manodopera"], None);
    assert_eq!(c, 1, "{v}");
    let o = Command::new(env!("CARGO_BIN_EXE_opencomputo"))
        .args(["report", s(&fixture), "computo", "--senza-prezzi"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "--senza-prezzi richiede --tradizionale"
    );
    let (v, c) = oc(&["esporta", s(&fixture), s(&d.join("f.xpwe"))], None);
    assert_eq!(c, 0, "{v}");
    // Elenco prezzi in DCF (sperimentale), riletto come prezzario.
    let (v, c) = oc(&["esporta", s(&fixture), s(&d.join("elenco.DCF"))], None);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["risultato"]["voci"], 4);
    assert!(
        std::fs::read(d.join("elenco.DCF"))
            .unwrap()
            .starts_with(b"AAMVHFSS")
    );
    let (v, c) = oc(
        &[
            "cerca",
            s(&f),
            "scavo",
            "--prezzario",
            s(&d.join("elenco.DCF")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["risultato"]["voci"][0]["codice"], "A.01.001");
    let (_, c) = oc(
        &[
            "esporta",
            s(&fixture),
            s(&d.join("x.dcf")),
            "--computo",
            "C",
        ],
        None,
    );
    assert_eq!(c, 1);
    let (_, c) = oc(&["info", s(&d.join("manca.ocp"))], None);
    assert_eq!(c, 3);
}

#[test]
fn schema_comandi_e_uso_errato() {
    let (v, c) = oc(&["schema-comandi"], None);
    assert_eq!(c, 0);
    assert_eq!(v["risultato"]["comandi"].as_array().unwrap().len(), 57);
    let o = Command::new(env!("CARGO_BIN_EXE_opencomputo"))
        .arg("sconosciuto")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let o = Command::new(env!("CARGO_BIN_EXE_opencomputo"))
        .arg("--help")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(
        String::from_utf8(o.stdout)
            .unwrap()
            .contains("schema-comandi")
    );
}

#[test]
fn servizio_su_stdin() {
    let d = cartella("servizio");
    let f = d.join("s.ocp");
    let mut c = Command::new(env!("CARGO_BIN_EXE_opencomputo"))
        .arg("servizio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let richieste = format!(
        "{}\n{}\n{}\n{}\n",
        serde_json::json!({"id": 1, "richiesta": "nuovo", "titolo": "S", "percorso": f}),
        r#"{"id": 2, "richiesta": "esegui", "comandi": [{"comando": "nuovo_computo", "nome": "Variante"}]}"#,
        r#"{"id": 3, "richiesta": "salva"}"#,
        r#"{"id": 4, "richiesta": "info"}"#,
    );
    c.stdin
        .take()
        .unwrap()
        .write_all(richieste.as_bytes())
        .unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let righe: Vec<Value> = String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    assert_eq!(righe.len(), 4);
    assert!(righe.iter().all(|r| r["ok"] == true), "{righe:#?}");
    assert_eq!(righe[3]["id"], 4);
    assert_eq!(
        righe[3]["risultato"]["computi"].as_array().unwrap().len(),
        2
    );
    assert!(f.exists());
}

/// Regressione (campagna di test del 2026-10-22): una riga non UTF-8 terminava il
/// servizio; ora riceve un errore e le richieste successive proseguono.
#[test]
fn servizio_resiste_a_una_riga_non_utf8() {
    let mut c = Command::new(env!("CARGO_BIN_EXE_opencomputo"))
        .arg("servizio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ingresso = c.stdin.take().unwrap();
    ingresso
        .write_all(b"\xff\xfe{\"richiesta\":\"info\"}\r\n")
        .unwrap();
    ingresso
        .write_all(b"{\"richiesta\":\"nuovo\",\"titolo\":\"T\"}\r\n")
        .unwrap();
    drop(ingresso);
    let o = c.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let righe: Vec<Value> = String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    assert_eq!(righe.len(), 2);
    assert_eq!(righe[0]["ok"], false);
    assert!(
        righe[0]["errore"]["messaggio"]
            .as_str()
            .unwrap()
            .contains("UTF-8")
    );
    assert_eq!(righe[1]["ok"], true);
}

#[test]
fn cronoprogramma_e_diagramma_di_gantt() {
    let d = cartella("cronoprogramma");
    let fixture = d.join("v7.ocp");
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../opencomputo-core/tests/fixtures/progetto_esempio_v7.json"
    ))
    .unwrap();
    let p = opencomputo_core::schema::importa_json(&json).unwrap();
    opencomputo_storage::salva(
        &fixture,
        &p,
        &opencomputo_storage::OpzioniSalvataggio::default(),
    )
    .unwrap();
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "cronoprogramma",
            "--gantt",
            "--uscita",
            s(&d.join("g")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    assert!(
        std::fs::read(d.join("g/cronoprogramma_gantt.pdf"))
            .unwrap()
            .starts_with(b"%PDF")
    );
    // Tabella delle attività anche in CSV; il Gantt solo in PDF.
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "cronoprogramma",
            "--formato",
            "csv",
            "--uscita",
            s(&d.join("t")),
        ],
        None,
    );
    assert_eq!(c, 0, "{v}");
    let (v, c) = oc(
        &[
            "report",
            s(&fixture),
            "cronoprogramma",
            "--gantt",
            "--formato",
            "xlsx",
            "--uscita",
            s(&d.join("x")),
        ],
        None,
    );
    assert_eq!(c, 1, "{v}");
    let (v, c) = oc(&["report", s(&fixture), "computo", "--gantt"], None);
    assert_ne!(c, 0, "{v}");
}
