//! Convertitore DCF esterno (ADR 0027) attraverso il servizio JSON.
//!
//! Questo test non usa l'harness standard: lo stesso eseguibile fa da convertitore finto
//! quando è lanciato come `… convert <origine> <uscita> --report <resoconto>` (il contratto
//! della CLI esterna), così la prova funziona allo stesso modo su Linux, macOS e Windows
//! senza script. Il comportamento del finto dipende dal nome del documento d'origine.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use opencomputo_agent_api::servizio::Servizio;
use opencomputo_application::convertitore::{self, AMBIENTE_CONSENTITO};
use serde_json::{Value, json};

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v7.json");

// ---------------------------------------------------------------------------
// Il convertitore finto

/// Comportamento del finto secondo il nome dell'origine; codice di uscita.
fn finto(argomenti: &[String]) -> i32 {
    let [_, comando, origine, uscita, opzione, resoconto] = argomenti else {
        eprintln!("errore: argomenti inattesi: {argomenti:?}");
        return 2;
    };
    if comando != "convert" || opzione != "--report" {
        eprintln!("errore: comando inatteso");
        return 2;
    }
    let origine = Path::new(origine);
    if !origine.is_absolute() || !origine.is_file() || !Path::new(uscita).is_absolute() {
        eprintln!("errore: percorsi non validi");
        return 1;
    }
    let nome = origine.file_stem().unwrap().to_string_lossy().into_owned();
    let riuscito = |resoconto_su_file: bool| {
        std::fs::write(uscita, FIXTURE).unwrap();
        println!(
            "convertito {} (.dcf) -> convertito.ocp (.ocp)",
            origine.display()
        );
        if resoconto_su_file {
            std::fs::write(
                resoconto,
                "trasferito (3):\n  + elenco prezzi: 6 voci\n  + computo: 3 voci\navvisi (0):\n",
            )
            .unwrap();
        }
        0
    };
    match nome.split('-').next().unwrap_or("") {
        "riuscito" | "file con spazi & 'apici'" => riuscito(true),
        "avvisi" => {
            eprintln!("attenzione: quadro economico non rappresentabile");
            riuscito(true)
        }
        "errore" => {
            eprintln!("errore: formato non riconosciuto");
            1
        }
        "lento" => {
            std::thread::sleep(Duration::from_secs(60));
            riuscito(true)
        }
        "spazzatura" => {
            std::fs::write(uscita, b"\x00\x01 non un progetto").unwrap();
            0
        }
        "senzauscita" => 0,
        "chiacchierone" => {
            use std::io::Write as _;
            let riga = "x".repeat(1023) + "\n";
            let mut o = std::io::stdout().lock();
            let mut e = std::io::stderr().lock();
            for _ in 0..5 * 1024 {
                let _ = o.write_all(riga.as_bytes());
                let _ = e.write_all(riga.as_bytes());
            }
            drop((o, e));
            riuscito(false)
        }
        "ambiente" => {
            let mut nomi: Vec<String> = std::env::vars_os()
                .map(|(k, _)| k.to_string_lossy().into_owned())
                .collect();
            nomi.sort();
            std::fs::write(uscita, FIXTURE).unwrap();
            std::fs::write(resoconto, nomi.join("\n")).unwrap();
            0
        }
        _ => {
            eprintln!("errore: caso di prova sconosciuto: {nome}");
            1
        }
    }
}

// ---------------------------------------------------------------------------
// Le prove

fn chiedi(s: &mut Servizio, r: &Value) -> Value {
    s.rispondi(&r.to_string())
}

fn ok(s: &mut Servizio, r: &Value) -> Value {
    let v = chiedi(s, r);
    assert_eq!(v["ok"], true, "{r}: {v}");
    v["risultato"].clone()
}

fn errore(s: &mut Servizio, r: &Value) -> String {
    let v = chiedi(s, r);
    assert_eq!(v["ok"], false, "{r}: {v}");
    v["errore"]["descrizione"].as_str().unwrap().to_owned()
}

struct Prova {
    cartella: PathBuf,
    programma: PathBuf,
}

impl Prova {
    fn nuova(nome: &str) -> Self {
        let cartella = std::env::temp_dir().join(format!(
            "opencomputo-prova-convertitore-{}-{nome}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cartella);
        std::fs::create_dir_all(&cartella).unwrap();
        Self {
            cartella,
            programma: std::env::current_exe().unwrap(),
        }
    }
    fn servizio(&self) -> Servizio {
        Servizio::new().con_cartella(self.cartella.join("config"))
    }
    fn origine(&self, nome: &str) -> String {
        let p = self.cartella.join(format!("{nome}.dcf"));
        std::fs::write(&p, b"AAMVHFSS documento finto").unwrap();
        p.display().to_string()
    }
}

impl Drop for Prova {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.cartella);
    }
}

fn configurazione() {
    let t = Prova::nuova("configurazione");
    let mut s = t.servizio();
    let stato = ok(&mut s, &json!({"richiesta": "convertitore_stato"}));
    assert_eq!(stato["programma"], Value::Null);
    assert_eq!(stato["utilizzabile"], false);
    assert!(
        stato["avviso"]
            .as_str()
            .unwrap()
            .contains("non ne verifica")
    );
    let origine = t.origine("riuscito");
    let e = errore(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": origine}),
    );
    assert!(e.contains("nessun convertitore"), "{e}");

    // Percorsi rifiutati: relativo, inesistente, una cartella.
    for p in [
        json!("dcf"),
        json!(t.cartella.join("non-esiste").display().to_string()),
        json!(t.cartella.display().to_string()),
    ] {
        errore(
            &mut s,
            &json!({"richiesta": "convertitore_imposta", "programma": p}),
        );
    }
    assert!(!t.cartella.join("config/convertitore.json").exists());

    let stato = ok(
        &mut s,
        &json!({"richiesta": "convertitore_imposta", "programma": t.programma}),
    );
    assert_eq!(stato["utilizzabile"], true);
    assert_eq!(stato["programma"], json!(t.programma));
    // Persistente: un altro servizio con la stessa cartella lo ritrova.
    let mut altro = t.servizio();
    assert_eq!(
        ok(&mut altro, &json!({"richiesta": "convertitore_stato"}))["utilizzabile"],
        true
    );

    // Tolto.
    let stato = ok(
        &mut s,
        &json!({"richiesta": "convertitore_imposta", "programma": null}),
    );
    assert_eq!(stato["programma"], Value::Null);

    // File di configurazione danneggiato: segnalato, nessun programma.
    std::fs::write(t.cartella.join("config/convertitore.json"), b"{rotto").unwrap();
    let stato = ok(&mut s, &json!({"richiesta": "convertitore_stato"}));
    assert_eq!(stato["utilizzabile"], false);
    assert!(
        stato["problema"]
            .as_str()
            .unwrap()
            .contains("non leggibile")
    );
    errore(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": origine}),
    );
}

fn configurato(t: &Prova) -> Servizio {
    let mut s = t.servizio();
    ok(
        &mut s,
        &json!({"richiesta": "convertitore_imposta", "programma": t.programma}),
    );
    s
}

fn conversione_riuscita() {
    let t = Prova::nuova("riuscita");
    let mut s = configurato(&t);
    for nome in ["riuscito", "file con spazi & 'apici'"] {
        let r = ok(
            &mut s,
            &json!({"richiesta": "importa_con_convertitore", "origine": t.origine(nome)}),
        );
        assert_eq!(r["importazione"]["formato"], "dcf (convertitore esterno)");
        assert_eq!(r["importazione"]["voci_elenco"], 6, "{r}");
        assert_eq!(
            r["importazione"]["anomalie"][0]["codice"],
            "convertitore.esterno"
        );
        assert!(
            r["convertitore"]["testo"]
                .as_str()
                .unwrap()
                .contains("trasferito (3)")
        );
        assert_eq!(r["convertitore"]["troncato"], false);
        // Progetto aperto, non ancora salvato.
        assert_eq!(r["progetto"]["file"], Value::Null, "{r}");
        let info = ok(&mut s, &json!({"richiesta": "info"}));
        assert_eq!(info["titolo"], r["progetto"]["titolo"]);
        ok(&mut s, &json!({"richiesta": "valida"}));
    }

    // Avvisi su stderr riportati come segnalazione.
    let r = ok(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": t.origine("avvisi")}),
    );
    assert!(
        r["convertitore"]["messaggi"]
            .as_str()
            .unwrap()
            .contains("quadro economico")
    );
    assert!(
        r["importazione"]["anomalie"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["codice"] == "convertitore.messaggi")
    );
}

fn conversioni_non_riuscite() {
    let t = Prova::nuova("errori");
    let mut s = configurato(&t);
    ok(
        &mut s,
        &json!({"richiesta": "nuovo", "titolo": "Già aperto"}),
    );
    let casi = [
        ("errore", "formato non riconosciuto"),
        ("spazzatura", "non è un progetto OpenComputo leggibile"),
        ("senzauscita", "non è un progetto OpenComputo leggibile"),
        ("sconosciuto", "caso di prova sconosciuto"),
    ];
    for (nome, atteso) in casi {
        let e = errore(
            &mut s,
            &json!({"richiesta": "importa_con_convertitore", "origine": t.origine(nome)}),
        );
        assert!(e.contains(atteso), "{nome}: {e}");
        // Il progetto aperto resta quello di prima.
        assert_eq!(
            ok(&mut s, &json!({"richiesta": "info"}))["titolo"],
            "Già aperto"
        );
    }
    let e = errore(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": t.cartella.join("assente.dcf")}),
    );
    assert!(e.contains("assente.dcf"), "{e}");
}

fn output_enorme_troncato() {
    let t = Prova::nuova("enorme");
    let mut s = configurato(&t);
    let r = ok(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": t.origine("chiacchierone")}),
    );
    let c = &r["convertitore"];
    assert_eq!(c["troncato"], true);
    assert!(c["testo"].as_str().unwrap().chars().count() <= convertitore::RESOCONTO_MASSIMO);
    assert!(c["messaggi"].as_str().unwrap().chars().count() <= convertitore::RESOCONTO_MASSIMO);
}

fn ambiente_ridotto() {
    let t = Prova::nuova("ambiente");
    let mut s = configurato(&t);
    let r = ok(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": t.origine("ambiente")}),
    );
    let nomi = r["convertitore"]["testo"].as_str().unwrap().to_owned();
    for n in nomi.lines() {
        // Windows aggiunge da sé alcune variabili di sistema al processo figlio.
        let di_sistema = cfg!(windows) && n.starts_with('=');
        assert!(
            di_sistema
                || AMBIENTE_CONSENTITO
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(n)),
            "variabile passata al convertitore: {n}"
        );
    }
}

fn durata_massima() {
    let t = Prova::nuova("durata");
    let origine = PathBuf::from(t.origine("lento"));
    let inizio = Instant::now();
    let e = convertitore::converti(&t.programma, &origine, Duration::from_secs(1)).unwrap_err();
    assert!(e.to_string().contains("interrotto"), "{e}");
    assert!(
        inizio.elapsed() < Duration::from_secs(20),
        "{:?}",
        inizio.elapsed()
    );
}

fn cartelle_temporanee_rimosse() {
    let prefisso = format!("opencomputo-convertitore-{}-", std::process::id());
    let rimaste: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefisso))
        .map(|e| e.path())
        .collect();
    assert!(rimaste.is_empty(), "{rimaste:?}");
}

fn main() {
    let argomenti: Vec<String> = std::env::args().collect();
    if argomenti.get(1).map(String::as_str) == Some("convert") {
        std::process::exit(finto(&argomenti));
    }
    let prove: [(&str, fn()); 7] = [
        ("configurazione", configurazione),
        ("conversione_riuscita", conversione_riuscita),
        ("conversioni_non_riuscite", conversioni_non_riuscite),
        ("output_enorme_troncato", output_enorme_troncato),
        ("ambiente_ridotto", ambiente_ridotto),
        ("durata_massima", durata_massima),
        ("cartelle_temporanee_rimosse", cartelle_temporanee_rimosse),
    ];
    // Filtro facoltativo come con l'harness standard (`cargo test --test convertitore x`).
    let filtro = argomenti.iter().skip(1).find(|a| !a.starts_with('-'));
    let mut eseguite = 0;
    for (nome, prova) in prove {
        if filtro.is_some_and(|f| !nome.contains(f.as_str())) {
            continue;
        }
        print!("test {nome} ... ");
        prova();
        println!("ok");
        eseguite += 1;
    }
    println!("\ntest result: ok. {eseguite} passed; 0 failed");
}
