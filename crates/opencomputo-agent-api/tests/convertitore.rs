//! Convertitore DCF/OCP esterno (ADR 0027) attraverso il servizio JSON.
//!
//! Questo test non usa l'harness standard: lo stesso eseguibile fa da convertitore finto
//! quando è lanciato con i comandi della CLI esterna (`convert`, `confronta`, `info`),
//! così la prova funziona allo stesso modo su Linux, macOS e Windows senza script. Come
//! la CLI vera, il finto decide la direzione dal contenuto dell'input; il caso di prova
//! dipende dal nome del documento d'origine o, per i progetti, dal titolo.

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

/// Firma dei `.dcf` prodotti dal finto.
const FIRMA_DCF: &[u8] = b"AAMVHFSS";

// ---------------------------------------------------------------------------
// Il convertitore finto

fn e_json(p: &Path) -> bool {
    std::fs::read(p).is_ok_and(|b| b.first() == Some(&b'{'))
}

/// Nome del file e, per un `.ocp`, il titolo del progetto (decidono il caso di prova).
fn chiave(p: &Path) -> String {
    let nome = p.file_name().unwrap().to_string_lossy().into_owned();
    let contenuto = std::fs::read(p).unwrap_or_default();
    let titolo = if e_json(p) {
        serde_json::from_slice::<Value>(&contenuto)
            .ok()
            .and_then(|v| {
                v["progetto"]["metadati"]["titolo"]
                    .as_str()
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    } else {
        String::from_utf8_lossy(&contenuto).into_owned()
    };
    format!("{nome} {titolo}")
}

fn finto(argomenti: &[String]) -> i32 {
    match argomenti.get(1).map(String::as_str) {
        Some("convert") => finto_convert(argomenti),
        Some("confronta") => finto_confronta(argomenti),
        Some("info") => {
            let f = Path::new(&argomenti[2]);
            if chiave(f).contains("errore") {
                eprintln!("errore: documento danneggiato");
                return 1;
            }
            println!("versione: 5.01\nvoci: 6");
            0
        }
        _ => {
            eprintln!("errore: comando sconosciuto");
            1
        }
    }
}

fn finto_convert(argomenti: &[String]) -> i32 {
    let [_, _, input, uscita, opzione, resoconto] = argomenti else {
        eprintln!("errore: argomenti inattesi: {argomenti:?}");
        return 2;
    };
    let input = Path::new(input);
    if opzione != "--report" || !input.is_absolute() || !Path::new(uscita).is_absolute() {
        eprintln!("errore: argomenti non validi");
        return 1;
    }
    if !input.is_file() {
        eprintln!("errore: file non trovato: {}", input.display());
        return 1;
    }
    let k = chiave(input);
    // Regola della direzione: dal contenuto.
    if e_json(input) {
        if k.contains("export-errore") {
            eprintln!("errore: scrittura fallita");
            return 1;
        }
        if k.contains("export-vuoto") {
            return 0;
        }
        let mut dcf = FIRMA_DCF.to_vec();
        dcf.extend_from_slice(k.as_bytes());
        std::fs::write(uscita, dcf).unwrap();
        let testo = "convertito progetto.ocp (.ocp) -> convertito.dcf (.dcf)\ntrasferito (3):\n  + elenco prezzi: 6 voci\n";
        print!("{testo}");
        std::fs::write(resoconto, testo).unwrap();
        return 0;
    }
    let riuscito = |resoconto_su_file: bool| {
        std::fs::write(uscita, FIXTURE).unwrap();
        println!(
            "convertito {} (.dcf) -> convertito.ocp (.ocp)",
            input.display()
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
    let nome = input.file_stem().unwrap().to_string_lossy().into_owned();
    match nome.as_str() {
        "riuscito" | "file con spazi & 'apici'" | "differenze" | "confrontaerrore" => {
            riuscito(true)
        }
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

fn finto_confronta(argomenti: &[String]) -> i32 {
    let [_, _, a, b] = argomenti else {
        eprintln!("errore: argomenti inattesi");
        return 2;
    };
    let (a, b) = (Path::new(a), Path::new(b));
    if !a.is_file() || !b.is_file() {
        eprintln!("errore: file non trovato");
        return 1;
    }
    if e_json(a) == e_json(b) {
        eprintln!("errore: confronta richiede un file .dcf e un file .ocp");
        return 1;
    }
    let k = format!("{} {}", chiave(a), chiave(b));
    if k.contains("confrontaerrore") {
        eprintln!("errore: documento danneggiato");
        return 1;
    }
    if k.contains("differenze") {
        println!("  DIFF  P.01.010.030.a   21.15 -> 21.16");
        println!("ESITO: elenco prezzi 5/6 · voci computo 3/3 · registrazioni 0/0 · SAL 0/0");
        println!("       -> DIFFERENZE");
        return 1;
    }
    println!("  ok  P.01.010.030.a   21.15 -> 21.15   um mq -> mq");
    println!("ESITO: elenco prezzi 6/6 · voci computo 3/3 · registrazioni 0/0 · SAL 0/0");
    println!("       -> PREZZI PRESERVATI");
    0
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

fn codici(r: &Value) -> Vec<String> {
    r["importazione"]["anomalie"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["codice"].as_str().unwrap().to_owned())
        .collect()
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
    fn configurato(&self) -> Servizio {
        let mut s = self.servizio();
        ok(
            &mut s,
            &json!({"richiesta": "convertitore_imposta", "programma": self.programma}),
        );
        s
    }
    fn origine(&self, nome: &str) -> String {
        let p = self.cartella.join(format!("{nome}.dcf"));
        std::fs::write(&p, b"AAMVHFSS documento finto").unwrap();
        p.display().to_string()
    }
    fn importa(&self, s: &mut Servizio, nome: &str) -> Value {
        ok(
            s,
            &json!({"richiesta": "importa_con_convertitore", "origine": self.origine(nome)}),
        )
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
    // Il banco di prova non ha DCF_BIN: nessun programma.
    if std::env::var_os("DCF_BIN").is_none() {
        assert_eq!(stato["programma"], Value::Null);
        assert_eq!(stato["utilizzabile"], false);
        let e = errore(
            &mut s,
            &json!({"richiesta": "importa_con_convertitore", "origine": t.origine("riuscito")}),
        );
        assert!(e.contains("nessun convertitore"), "{e}");
    }
    assert!(
        stato["avviso"]
            .as_str()
            .unwrap()
            .contains("non ne verifica")
    );

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
    assert_eq!(stato["origine"], "configurazione");
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
    if std::env::var_os("DCF_BIN").is_none() {
        assert_eq!(stato["programma"], Value::Null);
    }

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
        &json!({"richiesta": "importa_con_convertitore", "origine": t.origine("riuscito")}),
    );
}

fn importazione_con_verifica() {
    let t = Prova::nuova("importazione");
    let mut s = t.configurato();
    for nome in ["riuscito", "file con spazi & 'apici'"] {
        let r = t.importa(&mut s, nome);
        assert_eq!(r["importazione"]["formato"], "dcf (convertitore esterno)");
        assert_eq!(r["importazione"]["voci_elenco"], 6, "{r}");
        let c = codici(&r);
        assert!(c.contains(&"convertitore.esterno".to_owned()), "{c:?}");
        assert!(
            c.contains(&"convertitore.verifica_superata".to_owned()),
            "{c:?}"
        );
        assert!(
            r["convertitore"]["testo"]
                .as_str()
                .unwrap()
                .contains("trasferito (3)")
        );
        assert_eq!(r["convertitore"]["troncato"], false);
        assert_eq!(r["verifica"]["eseguita"], true);
        assert_eq!(r["verifica"]["preservati"], true);
        assert!(
            r["verifica"]["dettaglio"]
                .as_str()
                .unwrap()
                .contains("PREZZI PRESERVATI")
        );
        // Progetto aperto, non ancora salvato; nessun file accanto all'origine.
        assert_eq!(r["progetto"]["file"], Value::Null, "{r}");
        assert!(!t.cartella.join(format!("{nome}.ocp")).exists());
        let info = ok(&mut s, &json!({"richiesta": "info"}));
        assert_eq!(info["titolo"], r["progetto"]["titolo"]);
        ok(&mut s, &json!({"richiesta": "valida"}));
    }

    // Avvisi su stderr riportati come segnalazione.
    let r = t.importa(&mut s, "avvisi");
    assert!(
        r["convertitore"]["messaggi"]
            .as_str()
            .unwrap()
            .contains("quadro economico")
    );
    assert!(codici(&r).contains(&"convertitore.messaggi".to_owned()));

    // Differenze: progetto importato, verifica con l'elenco delle differenze.
    let r = t.importa(&mut s, "differenze");
    assert_eq!(r["verifica"]["eseguita"], true);
    assert_eq!(r["verifica"]["preservati"], false);
    assert!(
        r["verifica"]["dettaglio"]
            .as_str()
            .unwrap()
            .contains("DIFF")
    );
    assert!(codici(&r).contains(&"convertitore.verifica_differenze".to_owned()));

    // Confronto impossibile: progetto importato, verifica non eseguita con il motivo.
    let r = t.importa(&mut s, "confrontaerrore");
    assert_eq!(r["verifica"]["eseguita"], false);
    assert!(
        r["verifica"]["dettaglio"]
            .as_str()
            .unwrap()
            .contains("documento danneggiato")
    );
    assert!(codici(&r).contains(&"convertitore.verifica_non_eseguita".to_owned()));
}

fn conversioni_non_riuscite() {
    let t = Prova::nuova("errori");
    let mut s = t.configurato();
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

    // Un progetto OpenComputo (in chiaro o compresso) non si importa: la CLI lo
    // convertirebbe in .dcf.
    let ocp = t.cartella.join("progetto.dcf");
    std::fs::write(&ocp, FIXTURE).unwrap();
    let e = errore(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": ocp}),
    );
    assert!(e.contains("già un progetto OpenComputo"), "{e}");
    ok(
        &mut s,
        &json!({"richiesta": "salva", "percorso": t.cartella.join("c.ocp"), "gzip": true}),
    );
    let e = errore(
        &mut s,
        &json!({"richiesta": "importa_con_convertitore", "origine": t.cartella.join("c.ocp")}),
    );
    assert!(e.contains("già un progetto OpenComputo"), "{e}");
}

fn esportazione_con_verifica() {
    let t = Prova::nuova("esportazione");
    let mut s = t.configurato();
    let uscita = t.cartella.join("uscita");
    std::fs::create_dir_all(&uscita).unwrap();
    let dcf = uscita.join("lavori stradali.dcf");
    // Senza progetto aperto.
    errore(
        &mut s,
        &json!({"richiesta": "esporta_con_convertitore", "percorso": dcf}),
    );

    t.importa(&mut s, "riuscito");
    std::fs::write(&dcf, b"vecchio").unwrap();
    let r = ok(
        &mut s,
        &json!({"richiesta": "esporta_con_convertitore", "percorso": dcf}),
    );
    assert_eq!(r["file"], json!(dcf));
    let scritto = std::fs::read(&dcf).unwrap();
    assert!(scritto.starts_with(FIRMA_DCF), "{scritto:?}");
    // Il resoconto accanto, come il ponte: «<nome>.report.txt».
    let resoconto = uscita.join("lavori stradali.report.txt");
    assert_eq!(r["file_resoconto"], json!(resoconto));
    assert!(
        std::fs::read_to_string(&resoconto)
            .unwrap()
            .contains("(.ocp) -> convertito.dcf (.dcf)")
    );
    assert!(
        r["resoconto"]["testo"]
            .as_str()
            .unwrap()
            .contains("trasferito (3)")
    );
    assert_eq!(r["verifica"]["preservati"], true, "{r}");
    // Nessun file temporaneo rimasto nella cartella di destinazione.
    let mut nomi: Vec<String> = std::fs::read_dir(&uscita)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    nomi.sort();
    assert_eq!(nomi, ["lavori stradali.dcf", "lavori stradali.report.txt"]);

    // Differenze nella verifica: il file è scritto, il verdetto le riporta.
    ok(
        &mut s,
        &json!({"richiesta": "esegui", "comandi": [{"comando": "imposta_metadati", "titolo": "differenze"}]}),
    );
    let r = ok(
        &mut s,
        &json!({"richiesta": "esporta_con_convertitore", "percorso": dcf}),
    );
    assert_eq!(r["verifica"]["eseguita"], true);
    assert_eq!(r["verifica"]["preservati"], false);

    // Conversione non riuscita o senza risultato: la destinazione resta com'era.
    std::fs::write(&dcf, b"vecchio").unwrap();
    for (titolo, atteso) in [
        ("export-errore", "scrittura fallita"),
        ("export-vuoto", "non ha prodotto un file .dcf"),
    ] {
        ok(
            &mut s,
            &json!({"richiesta": "esegui", "comandi": [{"comando": "imposta_metadati", "titolo": titolo}]}),
        );
        let e = errore(
            &mut s,
            &json!({"richiesta": "esporta_con_convertitore", "percorso": dcf}),
        );
        assert!(e.contains(atteso), "{titolo}: {e}");
        assert_eq!(std::fs::read(&dcf).unwrap(), b"vecchio");
    }
    // Cartella di destinazione inesistente o destinazione che è una cartella.
    ok(
        &mut s,
        &json!({"richiesta": "esegui", "comandi": [{"comando": "imposta_metadati", "titolo": "Lavori"}]}),
    );
    errore(
        &mut s,
        &json!({"richiesta": "esporta_con_convertitore", "percorso": t.cartella.join("manca/x.dcf")}),
    );
    errore(
        &mut s,
        &json!({"richiesta": "esporta_con_convertitore", "percorso": uscita}),
    );
}

fn confronta_e_info() {
    let t = Prova::nuova("confronta");
    let mut s = t.configurato();
    let dcf = t.origine("riuscito");
    let ocp = t.cartella.join("p.ocp");
    std::fs::write(&ocp, FIXTURE).unwrap();
    // In qualunque ordine.
    for (a, b) in [(json!(dcf), json!(ocp)), (json!(ocp), json!(dcf))] {
        let v = ok(
            &mut s,
            &json!({"richiesta": "convertitore_confronta", "file_a": a, "file_b": b}),
        );
        assert_eq!(v["preservati"], true, "{v}");
    }
    // Due file dello stesso formato: il messaggio della CLI.
    let v = ok(
        &mut s,
        &json!({"richiesta": "convertitore_confronta", "file_a": dcf, "file_b": dcf}),
    );
    assert_eq!(v["eseguita"], false);
    assert!(
        v["dettaglio"]
            .as_str()
            .unwrap()
            .contains("un file .dcf e un file .ocp")
    );
    let v = ok(
        &mut s,
        &json!({"richiesta": "convertitore_confronta", "file_a": dcf, "file_b": t.cartella.join("manca.ocp")}),
    );
    assert_eq!(v["eseguita"], false);

    let i = ok(
        &mut s,
        &json!({"richiesta": "convertitore_info", "file": dcf}),
    );
    assert!(i["testo"].as_str().unwrap().contains("versione: 5.01"));
    let e = errore(
        &mut s,
        &json!({"richiesta": "convertitore_info", "file": t.origine("errore")}),
    );
    assert!(e.contains("documento danneggiato"), "{e}");
}

fn output_enorme_troncato() {
    let t = Prova::nuova("enorme");
    let mut s = t.configurato();
    let r = t.importa(&mut s, "chiacchierone");
    let c = &r["convertitore"];
    assert_eq!(c["troncato"], true);
    assert!(c["testo"].as_str().unwrap().chars().count() <= convertitore::RESOCONTO_MASSIMO);
    assert!(c["messaggi"].as_str().unwrap().chars().count() <= convertitore::RESOCONTO_MASSIMO);
}

fn ambiente_ridotto() {
    let t = Prova::nuova("ambiente");
    let mut s = t.configurato();
    let r = t.importa(&mut s, "ambiente");
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
    let e = convertitore::importa(&t.programma, &origine, Duration::from_secs(1)).unwrap_err();
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
    if matches!(
        argomenti.get(1).map(String::as_str),
        Some("convert" | "confronta" | "info")
    ) {
        std::process::exit(finto(&argomenti));
    }
    let prove: [(&str, fn()); 9] = [
        ("configurazione", configurazione),
        ("importazione_con_verifica", importazione_con_verifica),
        ("conversioni_non_riuscite", conversioni_non_riuscite),
        ("esportazione_con_verifica", esportazione_con_verifica),
        ("confronta_e_info", confronta_e_info),
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
