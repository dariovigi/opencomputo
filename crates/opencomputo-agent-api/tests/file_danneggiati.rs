//! File di progetto danneggiati o costruiti apposta (sequenza pseudo-casuale fissa,
//! quindi ripetibile), aperti attraverso il servizio JSON come fa l'interfaccia: le
//! fixture di tutte le versioni dello schema con valori cambiati (numeri enormi o
//! negativi, testi al posto dei numeri, `null`, elementi duplicati o tolti, campi
//! mancanti), byte alterati e file troncati, in chiaro e compressi.
//!
//! Nessuna richiesta deve terminare il processo (panic): un file rifiutato dà un errore,
//! un file accettato si mostra, si valida, si stampa, si esporta e si salva.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use opencomputo_agent_api::servizio::Servizio;
use serde_json::{Value, json};

const FIXTURE: [&str; 7] = [
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"),
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v2.json"),
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v3.json"),
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v4.json"),
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v5.json"),
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v6.json"),
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v7.json"),
];

/// Generatore xorshift: stessa sequenza a ogni esecuzione.
struct Caso(u64);

impl Caso {
    fn prossimo(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn fino_a(&mut self, n: usize) -> usize {
        usize::try_from(self.prossimo() % u64::try_from(n.max(1)).unwrap()).unwrap()
    }
    fn su(&mut self, n: usize) -> bool {
        self.fino_a(n) == 0
    }
}

/// Valori al posto di quelli originali.
fn valore_strano(c: &mut Caso) -> Value {
    match c.fino_a(22) {
        0 => Value::Null,
        1 => json!(0),
        2 => json!(-1),
        3 => json!(u64::MAX),
        4 => json!(i64::MIN),
        5 => json!(1e300),
        6 => json!(""),
        7 => json!("0"),
        8 => json!("-1"),
        9 => json!("79228162514264337593543950335"),
        10 => json!("0.0000000000000000000000000001"),
        11 => json!("1e30"),
        12 => json!("2*(3+"),
        13 => json!("9999-12-31"),
        14 => json!("2026-02-30"),
        15 => json!("00000000-0000-0000-0000-000000000001"),
        16 => json!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        17 => json!([]),
        18 => json!({}),
        19 => json!(true),
        20 => json!(1_000_000_000),
        _ => json!("x".repeat(300)),
    }
}

/// Percorsi (puntatori JSON) di tutti i nodi e testi presenti (per scambiare
/// riferimenti tra elementi esistenti).
fn nodi(v: &Value, qui: &str, percorsi: &mut Vec<String>, testi: &mut Vec<String>) {
    percorsi.push(qui.to_owned());
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                nodi(
                    x,
                    &format!("{qui}/{}", k.replace('~', "~0").replace('/', "~1")),
                    percorsi,
                    testi,
                );
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                nodi(x, &format!("{qui}/{i}"), percorsi, testi);
            }
        }
        Value::String(t) => testi.push(t.clone()),
        _ => {}
    }
}

/// Da una a tre modifiche mirate: per lo più dello stesso tipo del valore originale
/// (numeri limite, testi come numeri o date impossibili, identificativi di altri
/// elementi), oppure elementi tolti o duplicati, campi tolti, valori di altro tipo.
fn altera(c: &mut Caso, v: &mut Value) {
    for _ in 0..=c.fino_a(3) {
        let (mut percorsi, mut testi) = (Vec::new(), Vec::new());
        nodi(v, "", &mut percorsi, &mut testi);
        let percorso = percorsi[c.fino_a(percorsi.len())].clone();
        if percorso.ends_with("/formato") || percorso.ends_with("/versione_schema") {
            continue;
        }
        let Some(nodo) = v.pointer_mut(&percorso) else {
            continue;
        };
        if c.su(8) {
            *nodo = valore_strano(c);
            continue;
        }
        match nodo {
            Value::Number(_) => {
                *nodo = [
                    json!(0),
                    json!(-1),
                    json!(3),
                    json!(1_000_000_000),
                    json!(u64::MAX),
                    json!(1e300),
                ][c.fino_a(6)]
                .clone()
            }
            Value::String(t) if t.parse::<f64>().is_ok() => {
                *nodo = [
                    json!("0"),
                    json!("-1"),
                    json!("-0.01"),
                    json!("79228162514264337593543950335"),
                    json!("0.0000000000000000000000000001"),
                    json!("99999999999999999999.99"),
                ][c.fino_a(6)]
                .clone()
            }
            Value::String(_) if c.su(2) && !testi.is_empty() => {
                *nodo = json!(testi[c.fino_a(testi.len())].clone())
            }
            Value::String(_) => *nodo = valore_strano(c),
            Value::Bool(b) => *b = !*b,
            Value::Array(a) if !a.is_empty() => {
                let i = c.fino_a(a.len());
                match c.fino_a(3) {
                    0 => {
                        a.remove(i);
                    }
                    1 => {
                        let copia = a[i].clone();
                        a.push(copia);
                    }
                    _ => a.clear(),
                }
            }
            Value::Object(m) if !m.is_empty() => {
                let k = m.keys().nth(c.fino_a(m.len())).cloned().unwrap();
                m.remove(&k);
            }
            _ => *nodo = valore_strano(c),
        }
    }
}

/// Byte cambiati, tolti o troncati.
fn danneggia_byte(c: &mut Caso, b: &mut Vec<u8>) {
    for _ in 0..=c.fino_a(4) {
        if b.is_empty() {
            return;
        }
        let i = c.fino_a(b.len());
        match c.fino_a(4) {
            0 => b[i] = u8::try_from(c.fino_a(256)).unwrap(),
            1 => {
                b.remove(i);
            }
            2 => b.truncate(i),
            _ => b.insert(i, *b"{}[]\",:\\0\xff".get(c.fino_a(11)).unwrap_or(&b'"')),
        }
    }
}

fn gzip(b: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

fn cartella() -> PathBuf {
    let d = std::env::temp_dir().join(format!("opencomputo-danneggiati-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Ogni risposta è un oggetto con `ok`.
fn chiedi(s: &mut Servizio, r: &Value) -> Value {
    let risposta = s.rispondi(&r.to_string());
    assert!(risposta["ok"].is_boolean(), "{r}: {risposta}");
    risposta
}

/// Tutto ciò che l'interfaccia fa con un progetto aperto.
fn usa(s: &mut Servizio, dir: &std::path::Path) {
    let p = |n: &str| dir.join(n).display().to_string();
    for r in [
        json!({"richiesta": "info"}),
        json!({"richiesta": "valida"}),
        json!({"richiesta": "progetto"}),
        json!({"richiesta": "elenco_prezzi"}),
        json!({"richiesta": "computo"}),
        json!({"richiesta": "contabilita"}),
        json!({"richiesta": "cronoprogramma"}),
        json!({"richiesta": "analisi_prezzi"}),
        json!({"richiesta": "nuovi_prezzi"}),
        json!({"richiesta": "fabbisogno"}),
        json!({"richiesta": "cerca", "testo": "a"}),
        json!({"richiesta": "anteprima", "report": {"tipo": "computo"}}),
        json!({"richiesta": "anteprima", "report": {"tipo": "computo", "tradizionale": {}}}),
        json!({"richiesta": "anteprima", "report": {"tipo": "elenco_prezzi", "tradizionale": {}}}),
        json!({"richiesta": "anteprima", "report": {"tipo": "libretto"}}),
        json!({"richiesta": "anteprima", "report": {"tipo": "registro"}}),
        json!({"richiesta": "anteprima", "report": {"tipo": "cronoprogramma", "gantt": true}}),
        json!({"richiesta": "report", "report": {"tipo": "computo"}, "formato": "xlsx", "cartella": p("")}),
        json!({"richiesta": "report", "report": {"tipo": "computo", "tradizionale": {}}, "formato": "pdf", "cartella": p("")}),
        json!({"richiesta": "esporta_xpwe", "percorso": p("e.xpwe")}),
        json!({"richiesta": "esporta_dcf", "percorso": p("e.dcf")}),
        json!({"richiesta": "esegui", "comandi": [{"comando": "imposta_metadati", "titolo": "Modificato"}]}),
        json!({"richiesta": "annulla"}),
    ] {
        chiedi(s, &r);
    }
    // Salvato e riaperto, è lo stesso progetto.
    let prima = chiedi(s, &json!({"richiesta": "progetto"}));
    if chiedi(
        s,
        &json!({"richiesta": "salva", "percorso": p("salvato.ocp")}),
    )["ok"]
        == true
    {
        let r = chiedi(
            s,
            &json!({"richiesta": "apri", "percorso": p("salvato.ocp")}),
        );
        assert_eq!(r["ok"], true, "il file salvato non si riapre: {r}");
        assert_eq!(chiedi(s, &json!({"richiesta": "progetto"})), prima);
    }
}

fn campagna(seme: u64, prove: usize) -> (usize, usize) {
    let mut c = Caso(seme);
    let dir = cartella().join(seme.to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("p.ocp");
    let (mut aperti, mut rifiutati) = (0, 0);
    for n in 0..prove {
        let versione = c.fino_a(FIXTURE.len());
        let mut v: Value = serde_json::from_str(FIXTURE[versione]).unwrap();
        altera(&mut c, &mut v);
        let mut byte = serde_json::to_vec(&v).unwrap();
        if c.su(8) {
            danneggia_byte(&mut c, &mut byte);
        }
        if c.su(3) {
            byte = gzip(&byte);
            if c.su(6) {
                danneggia_byte(&mut c, &mut byte);
            }
        }
        std::fs::write(&file, &byte).unwrap();
        let esito = catch_unwind(AssertUnwindSafe(|| {
            let mut s = Servizio::new();
            let r = chiedi(
                &mut s,
                &json!({"richiesta": "apri", "percorso": file.display().to_string()}),
            );
            if r["ok"] == true {
                usa(&mut s, &dir);
                true
            } else {
                if std::env::var("OPENCOMPUTO_MOTIVI").is_ok() {
                    eprintln!("{}", r["errore"]);
                }
                false
            }
        }));
        match esito {
            Ok(true) => aperti += 1,
            Ok(false) => rifiutati += 1,
            Err(_) => {
                let copia = std::env::temp_dir().join(format!("opencomputo-panic-{seme}-{n}.ocp"));
                std::fs::write(&copia, &byte).unwrap();
                panic!(
                    "seme {seme}, prova {n} (fixture v{}): panic; file in {}",
                    versione + 1,
                    copia.display()
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    (aperti, rifiutati)
}

/// Prove: 150, o quante indicate in `OPENCOMPUTO_PROVE` per una campagna lunga.
fn prove() -> usize {
    std::env::var("OPENCOMPUTO_PROVE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150)
}

#[test]
fn file_danneggiati_aperti_senza_panic() {
    let (aperti, rifiutati) = campagna(0x2545_F491_4F6C_DD1D, prove());
    eprintln!("{aperti} file aperti, {rifiutati} rifiutati");
    // Entrambi i casi devono essere stati provati davvero.
    assert!(aperti > prove() / 10, "solo {aperti} file aperti");
    assert!(rifiutati > prove() / 10, "solo {rifiutati} file rifiutati");
}

/// Regressione (campagna del 2026-11-20): un nodo WBS ripetuto con il padre uguale al
/// proprio figlio chiude un ciclo raggiungibile dalla radice. La vista del progetto lo
/// percorreva all'infinito fino a esaurire la memoria; il riepilogo WBS dei report e il
/// cronoprogramma lo percorrevano ricorsivamente fino all'esaurimento della pila.
#[test]
fn ciclo_nella_wbs_da_identificativi_ripetuti() {
    let mut v: Value = serde_json::from_str(FIXTURE[6]).unwrap();
    let nodi = v
        .pointer_mut("/progetto/wbs/nodi")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let mut copia = nodi[0].clone();
    copia["padre"] = nodi[1]["id"].clone();
    nodi.push(copia);
    let dir = cartella().join("ciclo");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("p.ocp");
    std::fs::write(&file, serde_json::to_vec(&v).unwrap()).unwrap();
    let mut s = Servizio::new();
    let r = chiedi(
        &mut s,
        &json!({"richiesta": "apri", "percorso": file.display().to_string()}),
    );
    assert_eq!(r["ok"], true, "{r}");
    // Il problema è segnalato dalla validazione.
    let valida = chiedi(&mut s, &json!({"richiesta": "valida"}));
    assert_eq!(valida["risultato"]["valido"], false, "{valida}");
    let wbs = chiedi(&mut s, &json!({"richiesta": "progetto"}));
    assert!(
        wbs["risultato"]["wbs"].as_array().unwrap().len() <= 4,
        "{wbs}"
    );
    for r in [
        json!({"richiesta": "anteprima", "report": {"tipo": "computo"}}),
        json!({"richiesta": "cronoprogramma"}),
        json!({"richiesta": "anteprima", "report": {"tipo": "cronoprogramma", "gantt": true}}),
    ] {
        chiedi(&mut s, &r);
    }
    usa(&mut s, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Regressione (campagna del 2026-11-20): regole con un miliardo di decimali e una
/// misura minuscola terminavano il processo all'apertura (somma delle quantità).
#[test]
fn regole_fuori_misura_in_un_file() {
    let mut v: Value = serde_json::from_str(FIXTURE[3]).unwrap();
    v["progetto"]["regole"]["decimali_quantita"] = json!(1_000_000_000);
    *v.pointer_mut("/progetto/computi/0/voci/2/righi/3/altezza_peso/valore")
        .unwrap() = json!("0.0000000000000000000000000001");
    let dir = cartella().join("regole");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("p.ocp");
    std::fs::write(&file, serde_json::to_vec(&v).unwrap()).unwrap();
    let mut s = Servizio::new();
    let r = chiedi(
        &mut s,
        &json!({"richiesta": "apri", "percorso": file.display().to_string()}),
    );
    assert_eq!(r["ok"], true, "{r}");
    let valida = chiedi(&mut s, &json!({"richiesta": "valida"}));
    assert_eq!(valida["risultato"]["valido"], false, "{valida}");
    // Report ed export rifiutati con un messaggio chiaro.
    let r = chiedi(
        &mut s,
        &json!({"richiesta": "anteprima", "report": {"tipo": "computo"}}),
    );
    assert_eq!(r["ok"], false);
    assert!(
        r["errore"]["descrizione"]
            .as_str()
            .unwrap()
            .contains("regole di arrotondamento"),
        "{r}"
    );
    usa(&mut s, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}
