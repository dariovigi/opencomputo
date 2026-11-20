//! Tempi e memoria delle operazioni principali su un progetto grande, attraverso il
//! servizio JSON (la stessa strada dell'interfaccia): import, salvataggio, apertura,
//! viste, modifica con annulla, anteprima e report, export.
//!
//! `cargo run --release -p opencomputo-agent-api --example misura_progetto -- computo.xpwe`
//!
//! Il computo da misurare si genera con dati sintetici (vedi `docs/STATO.md`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::indexing_slicing,
    missing_docs
)]

use std::time::Instant;

use opencomputo_agent_api::servizio::Servizio;
use serde_json::{Value, json};

/// Memoria massima del processo finora (Linux), in MB.
fn memoria_massima() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb / 1024)
}

fn misura(s: &mut Servizio, nome: &str, richiesta: &Value) -> Value {
    let inizio = Instant::now();
    let r = s.rispondi(&richiesta.to_string());
    let tempo = inizio.elapsed();
    let byte = r.to_string().len();
    assert_eq!(r["ok"], true, "{nome}: {r}");
    println!(
        "{nome:<34} {:>7} ms  risposta {:>9} byte  memoria max {:>5} MB",
        tempo.as_millis(),
        byte,
        memoria_massima()
    );
    r["risultato"].clone()
}

fn main() {
    let origine = std::env::args()
        .nth(1)
        .expect("percorso di un computo XPWE");
    let cartella = std::env::temp_dir().join(format!("opencomputo-misura-{}", std::process::id()));
    std::fs::create_dir_all(&cartella).unwrap();
    let p = |n: &str| cartella.join(n).display().to_string();
    let mut s = Servizio::new();

    misura(
        &mut s,
        "importa XPWE",
        &json!({"richiesta": "importa", "origine": origine}),
    );
    let info = misura(&mut s, "info", &json!({"richiesta": "info"}));
    println!(
        "  {}",
        serde_json::to_string(&info["computi"]).unwrap_or_default()
    );
    misura(
        &mut s,
        "salva .ocp (gzip)",
        &json!({"richiesta": "salva", "percorso": p("p.ocp"), "gzip": true}),
    );
    misura(
        &mut s,
        "salva .ocp (testo)",
        &json!({"richiesta": "salva", "percorso": p("t.ocp"), "gzip": false}),
    );
    for f in ["p.ocp", "t.ocp"] {
        let m = std::fs::metadata(cartella.join(f)).map_or(0, |m| m.len());
        println!("  {f}: {} KB", m / 1024);
    }
    misura(
        &mut s,
        "apri .ocp (gzip)",
        &json!({"richiesta": "apri", "percorso": p("p.ocp")}),
    );
    misura(&mut s, "computo (vista)", &json!({"richiesta": "computo"}));
    misura(
        &mut s,
        "computo (vista, di nuovo)",
        &json!({"richiesta": "computo"}),
    );
    misura(
        &mut s,
        "elenco prezzi (vista)",
        &json!({"richiesta": "elenco_prezzi"}),
    );
    misura(&mut s, "valida", &json!({"richiesta": "valida"}));
    misura(&mut s, "progetto", &json!({"richiesta": "progetto"}));
    let modifica = json!({"richiesta": "esegui", "comandi": [{"comando": "modifica_rigo", "voce": 1, "numero": 1,
        "rigo": {"descrizione": "Modificato", "lunghezza": "3", "larghezza": "2", "altezza_peso": "1"}}]});
    misura(&mut s, "modifica di un rigo", &modifica);
    misura(
        &mut s,
        "computo dopo la modifica",
        &json!({"richiesta": "computo"}),
    );
    misura(&mut s, "annulla", &json!({"richiesta": "annulla"}));
    misura(&mut s, "ripristina", &json!({"richiesta": "ripristina"}));
    for _ in 0..20 {
        let _ = s.rispondi(&modifica.to_string());
    }
    misura(&mut s, "modifica dopo 20 modifiche", &modifica);
    misura(
        &mut s,
        "cerca",
        &json!({"richiesta": "cerca", "testo": "calcestruzzo"}),
    );
    misura(
        &mut s,
        "anteprima ordinaria",
        &json!({"richiesta": "anteprima", "report": {"tipo": "computo"}}),
    );
    misura(
        &mut s,
        "anteprima tradizionale (6 pag.)",
        &json!({"richiesta": "anteprima", "report": {"tipo": "computo", "tradizionale": {}}}),
    );
    misura(
        &mut s,
        "report PDF ordinario",
        &json!({"richiesta": "report", "report": {"tipo": "computo"}, "formato": "pdf", "cartella": p("")}),
    );
    misura(
        &mut s,
        "report PDF tradizionale",
        &json!({"richiesta": "report", "report": {"tipo": "computo", "tradizionale": {}}, "formato": "pdf", "cartella": p("")}),
    );
    misura(
        &mut s,
        "report Excel",
        &json!({"richiesta": "report", "report": {"tipo": "computo"}, "formato": "xlsx", "cartella": p("")}),
    );
    misura(
        &mut s,
        "esporta XPWE",
        &json!({"richiesta": "esporta_xpwe", "percorso": p("e.xpwe")}),
    );
    misura(
        &mut s,
        "esporta DCF (elenco)",
        &json!({"richiesta": "esporta_dcf", "percorso": p("e.dcf")}),
    );
    let _ = std::fs::remove_dir_all(&cartella);
}
