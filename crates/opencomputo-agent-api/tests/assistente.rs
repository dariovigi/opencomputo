//! Assistente AI nel servizio JSON, con portachiavi in memoria e un trasporto HTTP finto
//! che imita OpenAI e Anthropic: nessuna chiamata reale.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use opencomputo_agent_api::assistente::{Ambiente, FILE_CONFIGURAZIONE, Indirizzi};
use opencomputo_agent_api::servizio::{Passo, Servizio};
use opencomputo_assistente::provider::chiavi::{ArchivioChiavi, ChiaviInMemoria};
use opencomputo_assistente::provider::{
    Attesa, ErroreTrasporto, Provider, RichiestaHttp, RispostaHttp, Trasporto,
};
use opencomputo_assistente::testo::termini;
use serde_json::{Value, json};

const CSV: &[u8] =
    include_bytes!("../../opencomputo-assistente/tests/fixtures/prezzario_prova.csv");
const DIM: usize = 8;

fn cartella(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "opencomputo-assistente-{nome}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Richiesta ricevuta dal trasporto finto.
#[derive(Debug, Clone)]
struct Ricevuta {
    url: String,
    chiave: Option<String>,
    corpo: Value,
}

/// Trasporto finto: embedding come conteggi di termini in 8 posizioni; il «modello»
/// risponde all'analisi con una lavorazione e alla selezione scegliendo il candidato
/// con «idropittura» nella descrizione.
#[derive(Default)]
struct Finto {
    ricevute: Mutex<Vec<Ricevuta>>,
    /// Dopo quante chiamate di embedding rispondere 401.
    embedding_rifiutati_dopo: Mutex<Option<usize>>,
}

impl Finto {
    fn ricevute(&self, parte: &str) -> Vec<Ricevuta> {
        self.ricevute
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.url.contains(parte))
            .cloned()
            .collect()
    }
}

fn vettore(testo: &str) -> Vec<u32> {
    let mut v = vec![0u32; DIM];
    for t in termini(testo) {
        let h = t
            .bytes()
            .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
        v[h as usize % DIM] += 1;
    }
    // Mai il vettore nullo.
    v[0] += 1;
    v
}

fn risposta(stato: u16, corpo: &Value) -> RispostaHttp {
    RispostaHttp {
        stato,
        intestazioni: vec![("request-id".into(), "req_finto".into())],
        corpo: corpo.to_string().into_bytes(),
    }
}

fn analisi() -> Value {
    json!({
        "esito": "procedi",
        "lavorazioni": [{"id": "L01", "descrizione": "Tinteggiatura della parete", "query_ricerca": "tinteggiatura idropittura pareti", "dimensione_attesa": "superficie"}],
        "informazioni_mancanti": []
    })
}

fn selezione(utente: &str) -> Value {
    let i = utente.find("handle):\n").unwrap() + "handle):\n".len();
    let candidati: Vec<Value> = serde_json::from_str(&utente[i..]).unwrap();
    // Opzioni: l'idropittura per prima, poi le altre tinteggiature.
    let mut opzioni: Vec<Value> = Vec::new();
    for (parola, pertinenza) in [("idropittura", "alta"), ("smalto", "media")] {
        if let Some(c) = candidati.iter().find(|c| {
            c["descrizione"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains(parola)
        }) {
            opzioni.push(
                json!({"candidato": c["handle"], "pertinenza": pertinenza, "motivazione": parola}),
            );
        }
    }
    json!({
        "lavorazioni": [{
            "id": "L01", "descrizione": "Tinteggiatura della parete", "chiarezza": "chiara", "da_precisare": "",
            "righe": [{"descrizione": "parete", "forma": "rettangolo", "segno": "positivo", "parametri": [
                {"nome": "N", "valore": "1", "unita": "n", "provenienza": "utente", "rif_ipotesi": ""},
                {"nome": "L", "valore": "4", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""},
                {"nome": "B", "valore": "3", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""}
            ]}],
            "opzioni": opzioni
        }],
        "ipotesi": [],
        "informazioni_mancanti": []
    })
}

impl Trasporto for Finto {
    fn invia(&self, r: &RichiestaHttp) -> Result<RispostaHttp, ErroreTrasporto> {
        let corpo: Value = serde_json::from_slice(&r.corpo).unwrap();
        let chiave = r
            .intestazioni
            .iter()
            .find(|(n, _)| n == "authorization" || n == "x-api-key")
            .map(|(_, v)| v.clone());
        self.ricevute.lock().unwrap().push(Ricevuta {
            url: r.url.clone(),
            chiave,
            corpo: corpo.clone(),
        });
        if r.url.contains("/embeddings") {
            let fatte = self.ricevute("/embeddings").len();
            if self
                .embedding_rifiutati_dopo
                .lock()
                .unwrap()
                .is_some_and(|n| fatte > n)
            {
                return Ok(risposta(
                    401,
                    &json!({"error": {"message": "Incorrect API key", "type": "invalid_request_error", "code": "invalid_api_key"}}),
                ));
            }
            assert_eq!(corpo["dimensions"], json!(DIM));
            let data: Vec<Value> = corpo["input"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, t)| json!({"index": i, "embedding": vettore(t.as_str().unwrap())}))
                .collect();
            return Ok(risposta(200, &json!({"data": data})));
        }
        let utente = corpo["messages"][0]["content"].as_str().unwrap();
        let json = if utente.contains("handle):\n") {
            selezione(utente)
        } else {
            analisi()
        };
        Ok(risposta(
            200,
            &json!({
                "content": [{"type": "text", "text": json.to_string()}],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 100, "output_tokens": 20}
            }),
        ))
    }
}

struct SenzaAttesa;

impl Attesa for SenzaAttesa {
    fn attendi(&self, _: Duration) {}
    fn variazione_ms(&self, _: u64) -> u64 {
        0
    }
}

fn servizio(d: &std::path::Path, chiavi: &Arc<ChiaviInMemoria>, finto: &Arc<Finto>) -> Servizio {
    Servizio::new().con_ambiente(Ambiente {
        cartella: d.to_path_buf(),
        chiavi: chiavi.clone(),
        trasporto: finto.clone(),
        attesa: Arc::new(SenzaAttesa),
        indirizzi: Indirizzi::default(),
    })
}

fn chiedi(s: &mut Servizio, v: Value) -> Value {
    let r = s.rispondi(&v.to_string());
    assert_eq!(r["ok"], true, "{v}\n→ {r:#}");
    r["risultato"].clone()
}

fn rifiuta(s: &mut Servizio, v: Value) -> Value {
    let r = s.rispondi(&v.to_string());
    assert_eq!(r["ok"], false, "{v}\n→ {r:#}");
    r["errore"].clone()
}

/// Progetto nuovo con il prezzario di prova caricato come «prova».
fn prepara(s: &mut Servizio, d: &std::path::Path) {
    let csv = d.join("prezzario.csv");
    std::fs::write(&csv, CSV).unwrap();
    chiedi(
        s,
        json!({"richiesta": "nuovo", "titolo": "Prova", "percorso": d.join("lavori.ocp")}),
    );
    chiedi(
        s,
        json!({"richiesta": "carica_prezzario", "nome": "prova", "percorso": csv}),
    );
}

#[test]
fn senza_consenso_e_chiavi_nessuna_chiamata() {
    let d = cartella("consenso");
    let chiavi = Arc::new(ChiaviInMemoria::default());
    let finto = Arc::new(Finto::default());
    let mut s = servizio(&d, &chiavi, &finto);
    prepara(&mut s, &d);

    let stato = chiedi(&mut s, json!({"richiesta": "assistente_stato"}));
    assert_eq!(stato["consenso"]["valido"], false);
    assert_eq!(stato["consenso"]["versione_corrente"], 1);
    assert!(
        stato["consenso"]["testo"]
            .as_str()
            .unwrap()
            .contains("fuori dal tuo computer")
    );
    assert_eq!(
        stato["chiavi"],
        json!({"openai": false, "anthropic": false, "archivio": chiavi.descrizione()})
    );

    for r in [
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "tinteggiare una parete di 4 x 3 m"}),
    ] {
        let e = rifiuta(&mut s, r);
        assert_eq!(
            (e["codice"].as_str(), e["motivo"].as_str()),
            (Some("assistente"), Some("consenso_mancante"))
        );
    }
    // Versione sbagliata: rifiutata.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_consenso", "accetto": true, "versione": 99}),
    );
    assert_eq!(e["motivo"], "consenso_versione");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_consenso", "accetto": true}),
    );
    assert_eq!(e["motivo"], "consenso_versione");

    let c = chiedi(
        &mut s,
        json!({"richiesta": "assistente_consenso", "accetto": true, "versione": 1}),
    );
    assert_eq!(c["valido"], true);
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    assert_eq!(e["motivo"], "chiave_assente");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "parete"}),
    );
    assert_eq!(e["motivo"], "chiave_assente");

    // La ricerca testuale funziona sempre, senza rete.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_cerca", "prezzario": "prova", "testo": "idropittura pareti", "unita": "m2"}),
    );
    assert_eq!(r["ricerca"], "testuale");
    assert_eq!(r["voci"][0]["codice"], "T.01");

    // Revoca: di nuovo nessuna chiamata.
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_consenso", "accetto": false}),
    );
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    assert_eq!(e["motivo"], "consenso_mancante");
    assert!(finto.ricevute.lock().unwrap().is_empty());

    // Nel file di configurazione non c'è nessuna chiave.
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "openai", "chiave": "sk-segretissima"}),
    );
    let config = std::fs::read_to_string(d.join(FILE_CONFIGURAZIONE)).unwrap();
    assert!(!config.contains("sk-"));
    assert!(config.contains("\"modello_generazione\""));
}

#[test]
fn configurazione_e_chiavi() {
    let d = cartella("configura");
    let chiavi = Arc::new(ChiaviInMemoria::default());
    let finto = Arc::new(Finto::default());
    let mut s = servizio(&d, &chiavi, &finto);

    let st = chiedi(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"dimensioni_embedding": 512, "modello_generazione": "claude-haiku-5-5", "max_token_selezione": 20000}}),
    );
    assert_eq!(st["configurazione"]["dimensioni_embedding"], 512);
    assert_eq!(
        st["configurazione"]["modello_generazione"],
        "claude-haiku-5-5"
    );
    // Modello sconosciuto con modalità automatica: rifiutato, configurazione invariata.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"modello_generazione": "modello-inventato"}}),
    );
    assert_eq!(e["motivo"], "modello_non_supportato");
    // Uso forzato di uno strumento su un modello che non lo accetta: rifiutato.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"modello_generazione": "claude-sonnet-5-5", "modalita_output": "strumento_forzato"}}),
    );
    assert_eq!(e["motivo"], "modello_non_supportato");
    // Modello sconosciuto con modalità indicata: accettato.
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"modello_generazione": "modello-futuro", "modalita_output": "json"}}),
    );
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"max_token_analisi": 10}}),
    );
    assert_eq!(e["motivo"], "configurazione");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"sconosciuto": 1}}),
    );
    assert_eq!(e["codice"], "richiesta");

    // Persistenza: un nuovo servizio legge la stessa configurazione.
    let mut s2 = servizio(&d, &chiavi, &finto);
    let st = chiedi(&mut s2, json!({"richiesta": "assistente_stato"}));
    assert_eq!(
        st["configurazione"]["modello_generazione"],
        "modello-futuro"
    );
    assert_eq!(st["configurazione"]["modalita_output"], "json");
    assert_eq!(st["configurazione"]["max_token_selezione"], 20000);

    // Chiavi: salvate, eliminate con chiave vuota o assente; mai restituite.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "anthropic", "chiave": "  sk-ant-1  "}),
    );
    assert_eq!(r["impostata"], true);
    assert!(!r.to_string().contains("sk-ant"));
    assert_eq!(
        chiavi.leggi(Provider::Anthropic).unwrap().unwrap().esponi(),
        "sk-ant-1"
    );
    let st = chiedi(&mut s, json!({"richiesta": "assistente_stato"}));
    assert_eq!(
        (
            st["chiavi"]["anthropic"].as_bool(),
            st["chiavi"]["openai"].as_bool()
        ),
        (Some(true), Some(false))
    );
    assert!(!st.to_string().contains("sk-ant"));
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "anthropic", "chiave": ""}),
    );
    assert_eq!(chiavi.leggi(Provider::Anthropic), Ok(None));
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "anthropic", "chiave": "sk-ant-2"}),
    );
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "anthropic"}),
    );
    assert_eq!(chiavi.leggi(Provider::Anthropic), Ok(None));
    assert!(finto.ricevute.lock().unwrap().is_empty());
}

fn pronto(nome: &str) -> (PathBuf, Arc<ChiaviInMemoria>, Arc<Finto>, Servizio) {
    pronto_con(nome, |s| s)
}

/// Servizio pronto: progetto e prezzario, consenso, dimensioni 8, due chiavi.
fn pronto_con(
    nome: &str,
    f: impl FnOnce(Servizio) -> Servizio,
) -> (PathBuf, Arc<ChiaviInMemoria>, Arc<Finto>, Servizio) {
    let d = cartella(nome);
    let chiavi = Arc::new(ChiaviInMemoria::default());
    let finto = Arc::new(Finto::default());
    let mut s = f(servizio(&d, &chiavi, &finto));
    prepara(&mut s, &d);
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_consenso", "accetto": true, "versione": 1}),
    );
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"dimensioni_embedding": DIM}}),
    );
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "openai", "chiave": "sk-openai-prova"}),
    );
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_chiave", "provider": "anthropic", "chiave": "sk-ant-prova"}),
    );
    (d, chiavi, finto, s)
}

#[test]
fn indicizzazione_con_cache_e_ricerca_ibrida() {
    let avanzamenti = Arc::new(Mutex::new(Vec::new()));
    let a2 = avanzamenti.clone();
    let (d, chiavi, finto, mut s) = pronto_con("indice", |s| {
        s.con_avanzamento(Box::new(move |v: &Value| {
            a2.lock().unwrap().push(v.clone())
        }))
    });

    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    assert_eq!(r["rapporto"]["voci"], 26);
    assert_eq!(r["rapporto"]["calcolate"], 26);
    assert_eq!(r["spazio"]["dimensioni"], DIM);
    let file = PathBuf::from(r["file"].as_str().unwrap());
    assert!(file.starts_with(d.join("cache")) && file.exists());
    assert_eq!(avanzamenti.lock().unwrap().len(), 1);
    assert_eq!(
        avanzamenti.lock().unwrap()[0]["avanzamento"],
        "indicizzazione"
    );
    let emb = finto.ricevute("/embeddings");
    assert_eq!(emb.len(), 1);
    assert_eq!(emb[0].chiave.as_deref(), Some("Bearer sk-openai-prova"));
    // Ai provider vanno le descrizioni, non i prezzi.
    assert!(!emb[0].corpo.to_string().contains("7,20"));

    // Di nuovo: niente da calcolare.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    assert_eq!(
        (
            r["rapporto"]["calcolate"].as_u64(),
            r["rapporto"]["riusate"].as_u64()
        ),
        (Some(0), Some(26))
    );
    assert_eq!(finto.ricevute("/embeddings").len(), 1);

    // Un nuovo servizio legge la cache: ricerca ibrida con una sola chiamata (la domanda).
    let mut s2 = servizio(&d, &chiavi, &finto);
    prepara(&mut s2, &d);
    let r = chiedi(
        &mut s2,
        json!({"richiesta": "assistente_cerca", "prezzario": "prova", "testo": "idropittura pareti", "limite": 5}),
    );
    assert_eq!(r["ricerca"], "ibrida");
    let voci = r["voci"].as_array().unwrap();
    assert!(
        voci.len() <= 5 && voci.iter().any(|v| v["codice"] == "T.01"),
        "{r:#}"
    );
    assert_eq!(finto.ricevute("/embeddings").len(), 2);

    // Prezzario modificato (una descrizione cambiata, una voce tolta): nuovo file di
    // cache, ma si ricalcola solo la voce cambiata.
    let csv = String::from_utf8(CSV.to_vec()).unwrap();
    let mut righe: Vec<&str> = csv.lines().collect();
    righe.pop();
    let modificato =
        righe
            .join("\n")
            .replacen("compreso il carico", "compreso il carico e il trasporto", 1);
    assert_ne!(modificato, csv);
    let nuovo = d.join("prezzario2.csv");
    std::fs::write(&nuovo, modificato).unwrap();
    chiedi(
        &mut s2,
        json!({"richiesta": "carica_prezzario", "nome": "prova", "percorso": nuovo}),
    );
    let r = chiedi(
        &mut s2,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    assert_eq!(
        (
            r["rapporto"]["voci"].as_u64(),
            r["rapporto"]["riusate"].as_u64(),
            r["rapporto"]["calcolate"].as_u64(),
            r["rapporto"]["rimosse"].as_u64()
        ),
        (Some(25), Some(24), Some(1), Some(1)),
        "{r:#}"
    );
    assert_ne!(PathBuf::from(r["file"].as_str().unwrap()), file);
    assert_eq!(finto.ricevute("/embeddings").len(), 3);
    assert_eq!(
        finto.ricevute("/embeddings")[2].corpo["input"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Spazio diverso (altre dimensioni): la cache non vale, ricerca solo testuale.
    chiedi(
        &mut s2,
        json!({"richiesta": "assistente_configura", "configurazione": {"dimensioni_embedding": 16}}),
    );
    let r = chiedi(
        &mut s2,
        json!({"richiesta": "assistente_cerca", "prezzario": "prova", "testo": "idropittura pareti"}),
    );
    assert_eq!(r["ricerca"], "testuale");
    assert_eq!(finto.ricevute("/embeddings").len(), 3);

    // Consenso revocato: ricerca testuale anche con la cache.
    chiedi(
        &mut s2,
        json!({"richiesta": "assistente_configura", "configurazione": {"dimensioni_embedding": DIM}}),
    );
    chiedi(
        &mut s2,
        json!({"richiesta": "assistente_consenso", "accetto": false}),
    );
    let r = chiedi(
        &mut s2,
        json!({"richiesta": "assistente_cerca", "prezzario": "prova", "testo": "idropittura pareti"}),
    );
    assert_eq!(r["ricerca"], "testuale");
    assert_eq!(finto.ricevute("/embeddings").len(), 3);
}

#[test]
fn chiave_rifiutata_durante_l_indicizzazione() {
    let (_d, _chiavi, finto, mut s) = pronto("rifiutata");
    *finto.embedding_rifiutati_dopo.lock().unwrap() = Some(0);
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    assert_eq!(e["motivo"], "chiave_rifiutata");
    assert!(!e.to_string().contains("sk-openai-prova"));
    // Nessun nuovo tentativo con una chiave rifiutata.
    assert_eq!(finto.ricevute("/embeddings").len(), 1);
    // Prezzario inesistente.
    *finto.embedding_rifiutati_dopo.lock().unwrap() = None;
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "altro"}),
    );
    assert_eq!(e["codice"], "richiesta");
}

#[test]
fn proposta_provata_e_applicata() {
    let (d, _chiavi, finto, mut s) = pronto("proposta");
    let e = rifiuta(&mut s, json!({"richiesta": "assistente_applica"}));
    assert_eq!(e["motivo"], "nessuna_proposta");

    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "Tinteggiatura di una parete di 4 x 3 m"}),
    );
    assert_eq!(r["esito"], "proposta");
    assert_eq!(r["ricerca_ibrida"], false);
    assert!(
        r["avviso_ricerca"]
            .as_str()
            .unwrap()
            .contains("solo testuale")
    );
    // Opzioni: la più pertinente preselezionata, anteprima di entrambe.
    let opzioni = r["proposta"]["lavorazioni"][0]["opzioni"]
        .as_array()
        .unwrap();
    assert_eq!(
        opzioni
            .iter()
            .map(|o| (
                o["codice"].as_str().unwrap(),
                o["preselezionata"].as_bool().unwrap()
            ))
            .collect::<Vec<_>>(),
        [("T.01", true), ("T.03", false)]
    );
    let a = &r["anteprima"];
    assert_eq!(a["errore"], Value::Null, "{r:#}");
    assert_eq!(a["opzioni"][1]["importo"], "115.20"); // 12,00 m² × 9,60
    assert_eq!(a["esito"]["totali"][0]["dopo"], "86.40");
    assert_eq!(r["consumi"]["token_ingresso"], 200);
    let msg = finto.ricevute("/messages");
    assert_eq!(msg.len(), 2);
    assert!(
        msg.iter()
            .all(|m| m.chiave.as_deref() == Some("sk-ant-prova"))
    );
    assert!(
        msg.iter()
            .all(|m| m.corpo["output_config"]["format"]["type"] == "json_schema")
    );
    // Nella selezione nessun codice né prezzo.
    let sel = msg[1].corpo["messages"][0]["content"].as_str().unwrap();
    assert!(!sel.contains("7,20") && !sel.contains("7.20") && !sel.contains("T.01"));

    // Il progetto non è cambiato finché non si applica.
    let computi = |s: &mut Servizio| chiedi(s, json!({"richiesta": "info"}))["computi"].clone();
    let prima = computi(&mut s);
    assert_eq!(prima[0]["voci"], 0);
    let esito = chiedi(&mut s, json!({"richiesta": "assistente_applica"}));
    let dopo = computi(&mut s);
    assert_eq!(
        (dopo[0]["voci"].as_u64(), dopo[0]["totale"].as_str()),
        (Some(1), Some("86.40"))
    );
    assert_ne!(prima, dopo);
    assert!(esito.to_string().contains("86.4"), "{esito:#}");
    assert_eq!(
        esito["voci"],
        json!([{"lavorazione": "L01", "codice": "T.01", "da_completare": false}])
    );
    // La proposta resta: si aggiunge anche lo smalto, con l'altezza corretta.
    let scelte = json!([{"lavorazione": "L01", "codici": ["T.03"], "misure": [{"B": "2,5"}]}]);
    let a = chiedi(
        &mut s,
        json!({"richiesta": "assistente_anteprima", "scelte": scelte}),
    );
    // 4 × 2,5 = 10,00 m²: 72,00 € l'idropittura, 96,00 € lo smalto.
    assert_eq!(a["opzioni"][0]["importo"], "72.00");
    assert_eq!(a["opzioni"][1]["importo"], "96.00");
    assert_eq!(a["esito"]["totali"][0]["dopo"], "182.40");
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_applica", "scelte": scelte}),
    );
    assert_eq!(computi(&mut s)[0]["totale"], "182.40");
    // Scelte non valide: rifiutate senza modificare.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_applica", "scelte": [{"lavorazione": "L01", "codici": ["P.01"]}]}),
    );
    assert_eq!(e["motivo"], "scelta_non_valida");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_anteprima", "scelte": [{"lavorazione": "L01", "codici": [], "misure": [{"L": "x"}]}]}),
    );
    assert_eq!(e["motivo"], "scelta_non_valida");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_applica", "scelte": []}),
    );
    assert_eq!(e["motivo"], "nessuna_scelta");
    // Ogni applicazione si annulla come ogni passo.
    chiedi(&mut s, json!({"richiesta": "annulla"}));
    chiedi(&mut s, json!({"richiesta": "annulla"}));
    assert_eq!(computi(&mut s), prima);

    // Una nuova proposta, poi un altro progetto: la proposta è dimenticata.
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "parete 4 x 3 m"}),
    );
    chiedi(&mut s, json!({"richiesta": "salva"}));
    chiedi(
        &mut s,
        json!({"richiesta": "apri", "percorso": d.join("lavori.ocp")}),
    );
    let e = rifiuta(&mut s, json!({"richiesta": "assistente_applica"}));
    assert_eq!(e["motivo"], "nessuna_proposta");
}

#[test]
fn proposta_con_ricerca_ibrida() {
    let (_d, _chiavi, finto, mut s) = pronto("ibrida");
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "Tinteggiatura di una parete di 4 x 3 m"}),
    );
    assert_eq!(r["ricerca_ibrida"], true);
    assert!(r.get("avviso_ricerca").is_none());
    assert_eq!(r["anteprima"]["esito"]["totali"][0]["dopo"], "86.40");
    // Un lotto per l'indice e un testo per la domanda.
    assert_eq!(finto.ricevute("/embeddings").len(), 2);
}

#[test]
fn errori_del_servizio_senza_progetto() {
    let d = cartella("senza");
    let chiavi = Arc::new(ChiaviInMemoria::default());
    let finto = Arc::new(Finto::default());
    let mut s = servizio(&d, &chiavi, &finto);
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_cerca", "prezzario": "p", "testo": "x"}),
    );
    assert!(
        e["descrizione"]
            .as_str()
            .unwrap()
            .contains("nessun progetto aperto")
    );
    // La chiave non compare nel Debug della richiesta.
    let r: opencomputo_agent_api::servizio::Richiesta = serde_json::from_value(
        json!({"richiesta": "assistente_chiave", "provider": "openai", "chiave": "sk-mai-nei-log"}),
    )
    .unwrap();
    assert!(!format!("{r:?}").contains("sk-mai-nei-log"));
}

/// L'interfaccia tiene il servizio in uno stato condiviso tra thread.
#[test]
fn servizio_trasferibile_tra_thread() {
    fn invia<T: Send + 'static>() {}
    invia::<Servizio>();
    invia::<std::sync::Mutex<Servizio>>();
}

fn lavoro(s: &mut Servizio, v: Value) -> Box<opencomputo_agent_api::assistente::Lavoro> {
    match s.avvia(v) {
        Passo::Lavoro(l) => l,
        Passo::Risposta { risposta, .. } => panic!("atteso un lavoro: {risposta:#}"),
    }
}

#[test]
fn proposta_eseguita_fuori_dal_servizio_con_anteprima() {
    let (_d, _chiavi, _finto, mut s) = pronto("fuori");
    let l = lavoro(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "Tinteggiatura di una parete di 4 x 3 m", "id": 7}),
    );
    // Mentre il lavoro è in corso in un altro thread il servizio risponde ad altro.
    let t = std::thread::spawn(move || l.esegui(None));
    let info = chiedi(&mut s, json!({"richiesta": "info"}));
    assert_eq!(info["computi"][0]["voci"], 0);
    let (r, cambiato) = s.completa(t.join().unwrap());
    assert!(!cambiato);
    assert_eq!(
        (r["ok"].as_bool(), r["id"].as_i64()),
        (Some(true), Some(7)),
        "{r:#}"
    );
    let a = &r["risultato"]["anteprima"];
    assert_eq!(a["computo"], "Computo metrico estimativo");
    let voci = a["opzioni"].as_array().unwrap();
    assert_eq!(voci.len(), 2);
    assert_eq!(voci[0]["lavorazione"], "L01");
    assert_eq!(voci[0]["codice"], "T.01");
    assert_eq!(voci[0]["quantita"], "12.00");
    assert_eq!(voci[0]["importo"], "86.40");
    assert_eq!(a["scelte"].as_array().unwrap().len(), 1);
    assert_eq!(a["esito"]["totali"][0]["dopo"], "86.40");
    // L'anteprima non tocca il progetto; la proposta resta da applicare.
    assert_eq!(
        chiedi(&mut s, json!({"richiesta": "info"}))["computi"][0]["voci"],
        0
    );
    let esito = chiedi(&mut s, json!({"richiesta": "assistente_applica"}));
    assert_eq!(esito["applicato"], true);
}

#[test]
fn progetto_cambiato_durante_la_proposta() {
    let (d, _chiavi, _finto, mut s) = pronto("cambiato");
    let l = lavoro(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "prova", "testo": "parete di 4 x 3 m"}),
    );
    chiedi(&mut s, json!({"richiesta": "chiudi", "forza": true}));
    chiedi(
        &mut s,
        json!({"richiesta": "nuovo", "titolo": "Altro", "percorso": d.join("altro.ocp")}),
    );
    let (r, _) = s.completa(l.esegui(None));
    assert_eq!(r["ok"], false);
    assert_eq!(r["errore"]["motivo"], "progetto_cambiato");
    let e = rifiuta(&mut s, json!({"richiesta": "assistente_applica"}));
    assert_eq!(e["motivo"], "nessuna_proposta");
}

#[test]
fn annullamento_dall_interruttore() {
    let (_d, _chiavi, finto, mut s) = pronto("annulla");
    let interruttore = s.interruttore();
    let l = lavoro(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    interruttore.annulla();
    let (r, _) = s.completa(l.esegui(None));
    assert_eq!(r["errore"]["motivo"], "annullato", "{r:#}");
    assert!(finto.ricevute("/embeddings").is_empty());
    // Il lavoro successivo ha un segnale nuovo.
    let l = lavoro(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    let (r, _) = s.completa(l.esegui(None));
    assert_eq!(r["ok"], true, "{r:#}");
    // Dal servizio a righe la richiesta è accettata (non c'è nulla in corso).
    assert_eq!(
        chiedi(&mut s, json!({"richiesta": "assistente_annulla"}))["annullamento"],
        "chiesto"
    );
    // Le richieste brevi non diventano lavori.
    assert!(matches!(
        s.avvia(json!({"richiesta": "info"})),
        Passo::Risposta {
            cambiato: false,
            ..
        }
    ));
    assert!(matches!(
        s.avvia(json!({"richiesta": "assistente_cerca", "prezzario": "nessuno", "testo": "x"})),
        Passo::Risposta {
            cambiato: false,
            ..
        }
    ));
}

#[test]
fn stato_dell_indice_e_modelli() {
    let (_d, _chiavi, finto, mut s) = pronto("indice-stato");
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "prova"}),
    );
    assert_eq!(
        (
            i["voci"].as_u64(),
            i["indicizzate"].as_u64(),
            i["completo"].as_bool(),
            i["ricerca"].as_str()
        ),
        (Some(26), Some(0), Some(false), Some("testuale"))
    );
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "prova"}),
    );
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "prova"}),
    );
    assert_eq!(
        (
            i["indicizzate"].as_u64(),
            i["completo"].as_bool(),
            i["ricerca"].as_str()
        ),
        (Some(26), Some(true), Some("ibrida"))
    );
    assert_eq!(i["spazio"]["dimensioni"], DIM);
    // Lo stato dell'indice non fa chiamate.
    assert_eq!(finto.ricevute("/embeddings").len(), 1);

    let st = chiedi(&mut s, json!({"richiesta": "assistente_stato"}));
    let modelli = st["modelli"]["generazione"].as_array().unwrap();
    assert!(modelli.contains(&json!("claude-sonnet-5-5")));
    assert!(
        st["modelli"]["senza_strumento_forzato"]
            .as_array()
            .unwrap()
            .contains(&json!("claude-opus-5-5"))
    );
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"modello_embedding": "text-embedding-ada-002"}}),
    );
    assert_eq!(
        e["motivo"], "configurazione",
        "dimensioni 8 non accettate da ada-002"
    );
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_configura", "configurazione": {"modello_embedding": "text-embedding-ada-002", "dimensioni_embedding": 0}}),
    );
}
