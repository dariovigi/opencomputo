//! Misura dei tempi dell'assistente su un listino sintetico grande, con provider finti in
//! memoria (nessuna rete): `cargo run --release -p opencomputo-agent-api --example
//! misura_assistente [voci] [dimensioni]`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    missing_docs
)]

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opencomputo_agent_api::assistente::{Ambiente, Indirizzi};
use opencomputo_agent_api::servizio::Servizio;
use opencomputo_assistente::provider::chiavi::ChiaviInMemoria;
use opencomputo_assistente::provider::{
    Attesa, ErroreTrasporto, RichiestaHttp, RispostaHttp, Trasporto,
};
use serde_json::{Value, json};

const LAVORAZIONI: [&str; 10] = [
    "Scavo",
    "Massetto",
    "Pavimento",
    "Intonaco",
    "Tinteggiatura",
    "Muratura",
    "Calcestruzzo",
    "Demolizione",
    "Impermeabilizzazione",
    "Serramento",
];
const MATERIALI: [&str; 6] = [
    "gres",
    "laterizio",
    "cemento",
    "acciaio",
    "legno",
    "ceramica",
];

fn listino(voci: usize) -> String {
    let mut s = String::from("Codice;Capitolo;Descrizione capitolo;Descrizione;U.M.;Prezzo\n");
    for n in 0..voci {
        let c = n % 300;
        writeln!(
            s,
            "V{n:05};K{c:03};Capitolo {} {};{} in {} spessore {} cm, tipo {n}, compresi oneri, sfridi, trasporti e ogni altro magistero per dare l'opera finita a regola d'arte;m²;{},{:02}",
            LAVORAZIONI[c % 10], MATERIALI[c % 6], LAVORAZIONI[n % 10], MATERIALI[(n / 10) % 6], n % 40, n % 500 + 1, n % 100
        )
        .unwrap();
    }
    s
}

struct Finto(usize);

impl Trasporto for Finto {
    fn invia(&self, r: &RichiestaHttp) -> Result<RispostaHttp, ErroreTrasporto> {
        let corpo: Value = serde_json::from_slice(&r.corpo).unwrap();
        let risposta = if r.url.contains("embeddings") {
            let n = corpo["input"].as_array().unwrap().len();
            let mut s = String::from("{\"data\":[");
            for i in 0..n {
                if i > 0 {
                    s.push(',');
                }
                write!(s, "{{\"index\":{i},\"embedding\":[").unwrap();
                for d in 0..self.0 {
                    if d > 0 {
                        s.push(',');
                    }
                    write!(
                        s,
                        "{}",
                        if (i + d) % 7 == 0 {
                            "0.0123456"
                        } else {
                            "-0.0034567"
                        }
                    )
                    .unwrap();
                }
                s.push_str("]}");
            }
            s.push_str("]}");
            s
        } else {
            let utente = corpo["messages"][0]["content"].as_str().unwrap();
            let j = if let Some(i) = utente.find("handle):\n") {
                let c: Vec<Value> = serde_json::from_str(&utente[i + 9..]).unwrap();
                let opzioni: Vec<Value> = c.iter().take(5).map(|x| json!({"candidato": x["handle"], "pertinenza": "media", "motivazione": "m"})).collect();
                json!({"lavorazioni": [{"id": "L01", "descrizione": "Massetto", "chiarezza": "chiara", "da_precisare": "",
                    "righe": [{"descrizione": "stanza", "forma": "rettangolo", "segno": "positivo", "parametri": [
                        {"nome": "N", "valore": "1", "unita": "n", "provenienza": "utente", "rif_ipotesi": ""},
                        {"nome": "L", "valore": "4", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""},
                        {"nome": "B", "valore": "3", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""}]}],
                    "opzioni": opzioni}], "ipotesi": [], "informazioni_mancanti": []})
            } else {
                json!({"esito": "procedi", "lavorazioni": [{"id": "L01", "descrizione": "Massetto", "query_ricerca": "massetto cemento spessore", "dimensione_attesa": "superficie"}], "informazioni_mancanti": []})
            };
            json!({"content": [{"type": "text", "text": j.to_string()}], "stop_reason": "end_turn", "usage": {"input_tokens": 1, "output_tokens": 1}}).to_string()
        };
        Ok(RispostaHttp {
            stato: 200,
            intestazioni: vec![],
            corpo: risposta.into_bytes(),
        })
    }
}

struct SenzaAttesa;
impl Attesa for SenzaAttesa {
    fn attendi(&self, _: Duration) {}
    fn variazione_ms(&self, _: u64) -> u64 {
        0
    }
}

fn memoria_mb() -> (u64, u64) {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let campo = |n: &str| {
        s.lines()
            .find(|l| l.starts_with(n))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .map_or(0, |kb| kb / 1024)
    };
    (campo("VmRSS:"), campo("VmHWM:"))
}

fn main() {
    let mut argomenti = std::env::args().skip(1);
    let voci: usize = argomenti.next().map_or(30_000, |v| v.parse().unwrap());
    let dimensioni: usize = argomenti.next().map_or(1536, |v| v.parse().unwrap());
    let d = std::env::temp_dir().join(format!("opencomputo-misura-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let csv = d.join("listino.csv");
    std::fs::write(&csv, listino(voci)).unwrap();

    let nuovo = || {
        Servizio::new().con_ambiente(Ambiente {
            cartella: d.clone(),
            chiavi: Arc::new(ChiaviInMemoria::default()),
            trasporto: Arc::new(Finto(dimensioni)),
            attesa: Arc::new(SenzaAttesa),
            indirizzi: Indirizzi::default(),
        })
    };
    let passo = |s: &mut Servizio, nome: &str, v: Value| -> Value {
        let t = Instant::now();
        let r = s.rispondi(&v.to_string());
        let (rss, picco) = memoria_mb();
        assert_eq!(r["ok"], true, "{nome}: {r:#}");
        println!(
            "{nome:<44} {:>7} ms   RSS {rss:>5} MB  picco {picco:>5} MB",
            t.elapsed().as_millis()
        );
        r["risultato"].clone()
    };
    let prepara = |s: &mut Servizio| {
        passo(
            s,
            "nuovo",
            json!({"richiesta": "nuovo", "titolo": "Misura"}),
        );
        passo(
            s,
            "carica_prezzario (CSV)",
            json!({"richiesta": "carica_prezzario", "nome": "p", "percorso": csv}),
        );
        s.rispondi(
            &json!({"richiesta": "assistente_consenso", "accetto": true, "versione": 1})
                .to_string(),
        );
        s.rispondi(
            &json!({"richiesta": "assistente_chiave", "provider": "openai", "chiave": "k"})
                .to_string(),
        );
        s.rispondi(
            &json!({"richiesta": "assistente_chiave", "provider": "anthropic", "chiave": "k"})
                .to_string(),
        );
        s.rispondi(&json!({"richiesta": "assistente_configura", "configurazione": {"dimensioni_embedding": dimensioni}}).to_string());
    };
    println!("== {voci} voci, {dimensioni} dimensioni");
    let mut s = nuovo();
    prepara(&mut s);
    passo(
        &mut s,
        "assistente_indice (senza cache)",
        json!({"richiesta": "assistente_indice", "prezzario": "p"}),
    );
    passo(
        &mut s,
        "assistente_cerca testuale (prima)",
        json!({"richiesta": "assistente_cerca", "prezzario": "p", "testo": "massetto cemento spessore 5"}),
    );
    passo(
        &mut s,
        "assistente_cerca testuale (seconda)",
        json!({"richiesta": "assistente_cerca", "prezzario": "p", "testo": "pavimento gres"}),
    );
    passo(
        &mut s,
        "assistente_indicizza",
        json!({"richiesta": "assistente_indicizza", "prezzario": "p"}),
    );
    passo(
        &mut s,
        "assistente_indicizza (niente da fare)",
        json!({"richiesta": "assistente_indicizza", "prezzario": "p"}),
    );
    passo(
        &mut s,
        "assistente_cerca ibrida",
        json!({"richiesta": "assistente_cerca", "prezzario": "p", "testo": "massetto cemento spessore 5"}),
    );
    passo(
        &mut s,
        "assistente_proponi",
        json!({"richiesta": "assistente_proponi", "prezzario": "p", "testo": "massetto 4 x 3 m"}),
    );
    passo(
        &mut s,
        "assistente_anteprima",
        json!({"richiesta": "assistente_anteprima", "scelte": []}),
    );
    passo(
        &mut s,
        "assistente_applica",
        json!({"richiesta": "assistente_applica"}),
    );
    passo(&mut s, "info", json!({"richiesta": "info"}));
    drop(s);
    println!("-- nuovo processo (cache su disco)");
    let mut s = nuovo();
    prepara(&mut s);
    passo(
        &mut s,
        "assistente_indice (legge la cache)",
        json!({"richiesta": "assistente_indice", "prezzario": "p"}),
    );
    passo(
        &mut s,
        "assistente_cerca ibrida (prima)",
        json!({"richiesta": "assistente_cerca", "prezzario": "p", "testo": "massetto cemento"}),
    );
    passo(
        &mut s,
        "assistente_proponi",
        json!({"richiesta": "assistente_proponi", "prezzario": "p", "testo": "massetto 4 x 3 m"}),
    );
    let cache: u64 = std::fs::read_dir(d.join("cache"))
        .unwrap()
        .map(|f| f.unwrap().metadata().unwrap().len())
        .sum();
    println!("cache su disco: {} MB", cache / 1_048_576);
    let _ = std::fs::remove_dir_all(&d);
}
