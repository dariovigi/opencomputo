//! L'assistente nel servizio in condizioni reali: trasporto HTTP vero verso un server
//! locale (127.0.0.1) che imita OpenAI e Anthropic, più lotti, interruzioni, annullamenti,
//! lavori contemporanei, file di configurazione e di cache danneggiati, richieste
//! malformate. Nessuna rete esterna e nessuna chiave vera.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use opencomputo_agent_api::assistente::{
    Ambiente, FILE_CONFIGURAZIONE, Indirizzi, RICHIESTA_MASSIMA,
};
use opencomputo_agent_api::servizio::{Passo, Servizio};
use opencomputo_assistente::provider::chiavi::ChiaviInMemoria;
use opencomputo_assistente::testo::termini;
use opencomputo_rete::{AttesaReale, TrasportoHttps};
use serde_json::{Value, json};

const DIM: usize = 16;
const VOCI: usize = 600; // tre lotti da 256
const CHIAVE_OPENAI: &str = "sk-openai-robustezza";
const CHIAVE_ANTHROPIC: &str = "sk-ant-robustezza";

fn cartella(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "opencomputo-robustezza-{nome}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const LAVORAZIONI: [&str; 6] = [
    "Massetto",
    "Pavimento",
    "Intonaco",
    "Tinteggiatura",
    "Muratura",
    "Demolizione",
];
const MATERIALI: [&str; 5] = ["gres", "laterizio", "cemento", "legno", "ceramica"];

fn listino(voci: usize) -> String {
    let mut s = String::from("Codice;Capitolo;Descrizione capitolo;Descrizione;U.M.;Prezzo\n");
    for n in 0..voci {
        let c = n % 12;
        writeln!(
            s,
            "V{n:04};K{c:02};Capitolo {};{} in {} spessore {} cm, variante {n};m²;{},{:02}",
            LAVORAZIONI[c % 6],
            LAVORAZIONI[n % 6],
            MATERIALI[(n / 6) % 5],
            n % 30 + 1,
            n % 90 + 5,
            n % 100
        )
        .unwrap();
    }
    s
}

/// Comportamento del provider finto, modificabile durante il test.
#[derive(Default)]
struct Regole {
    /// Le chiamate di embedding dalla numero N (da 1) rispondono 401.
    embedding_rifiutati_da: Option<usize>,
    /// Attesa prima di ogni risposta di embedding.
    ritardo_embedding: Duration,
    /// Attesa prima di ogni risposta del modello.
    ritardo_messaggi: Duration,
    /// Quante risposte 529 dare prima di quella buona.
    sovraccarichi: usize,
}

struct Provider {
    base: String,
    regole: Arc<Mutex<Regole>>,
    embedding: Arc<AtomicUsize>,
    messaggi: Arc<AtomicUsize>,
    testi_embedding: Arc<AtomicUsize>,
    chiavi_viste: Arc<Mutex<Vec<String>>>,
}

fn vettore(testo: &str) -> Vec<u32> {
    let mut v = vec![0u32; DIM];
    for t in termini(testo) {
        let h = t
            .bytes()
            .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
        v[h as usize % DIM] += 1;
    }
    v[0] += 1;
    v
}

fn rispondi(mut s: TcpStream, stato: u16, corpo: &str) {
    let _ = s.write_all(
        format!("HTTP/1.1 {stato} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\nrequest-id: req_prova\r\n\r\n", corpo.len()).as_bytes(),
    );
    let _ = s.write_all(corpo.as_bytes());
}

impl Provider {
    /// Server con un thread per connessione: più richieste insieme, come in rete.
    fn avvia() -> Self {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let p = Self {
            base: format!("http://{}", l.local_addr().unwrap()),
            regole: Arc::default(),
            embedding: Arc::default(),
            messaggi: Arc::default(),
            testi_embedding: Arc::default(),
            chiavi_viste: Arc::default(),
        };
        let (regole, embedding, messaggi, testi, chiavi) = (
            p.regole.clone(),
            p.embedding.clone(),
            p.messaggi.clone(),
            p.testi_embedding.clone(),
            p.chiavi_viste.clone(),
        );
        thread::spawn(move || {
            for s in l.incoming() {
                let Ok(s) = s else { return };
                let (regole, embedding, messaggi, testi, chiavi) = (
                    regole.clone(),
                    embedding.clone(),
                    messaggi.clone(),
                    testi.clone(),
                    chiavi.clone(),
                );
                thread::spawn(move || {
                    let mut lettore = BufReader::new(s.try_clone().unwrap());
                    let mut prima = String::new();
                    let _ = lettore.read_line(&mut prima);
                    let mut lunghezza = 0usize;
                    loop {
                        let mut riga = String::new();
                        if lettore.read_line(&mut riga).unwrap_or(0) == 0 || riga == "\r\n" {
                            break;
                        }
                        if let Some((n, v)) = riga.trim_end().split_once(':') {
                            match n.to_lowercase().as_str() {
                                "content-length" => lunghezza = v.trim().parse().unwrap_or(0),
                                "authorization" | "x-api-key" => chiavi
                                    .lock()
                                    .unwrap()
                                    .push(v.trim().trim_start_matches("Bearer ").to_owned()),
                                _ => {}
                            }
                        }
                    }
                    let mut corpo = vec![0u8; lunghezza];
                    if lettore.read_exact(&mut corpo).is_err() {
                        return;
                    }
                    let Ok(json) = serde_json::from_slice::<Value>(&corpo) else {
                        return rispondi(s, 400, "{}");
                    };
                    if prima.contains("/v1/embeddings") {
                        let n = embedding.fetch_add(1, Ordering::SeqCst) + 1;
                        let (rifiuta, ritardo) = {
                            let r = regole.lock().unwrap();
                            (
                                r.embedding_rifiutati_da.is_some_and(|da| n >= da),
                                r.ritardo_embedding,
                            )
                        };
                        thread::sleep(ritardo);
                        if rifiuta {
                            return rispondi(
                                s,
                                401,
                                r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error","code":"invalid_api_key"}}"#,
                            );
                        }
                        let input = json["input"].as_array().unwrap();
                        testi.fetch_add(input.len(), Ordering::SeqCst);
                        assert_eq!(json["dimensions"], json!(DIM));
                        let data: Vec<Value> = input.iter().enumerate().map(|(i, t)| json!({"index": i, "embedding": vettore(t.as_str().unwrap())})).collect();
                        return rispondi(s, 200, &json!({"data": data}).to_string());
                    }
                    messaggi.fetch_add(1, Ordering::SeqCst);
                    let (ritardo, sovraccarico) = {
                        let mut r = regole.lock().unwrap();
                        let sovraccarico = r.sovraccarichi > 0;
                        r.sovraccarichi = r.sovraccarichi.saturating_sub(1);
                        (r.ritardo_messaggi, sovraccarico)
                    };
                    thread::sleep(ritardo);
                    if sovraccarico {
                        return rispondi(
                            s,
                            529,
                            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
                        );
                    }
                    let utente = json["messages"][0]["content"].as_str().unwrap_or_default();
                    let risposta = if let Some(i) = utente.find("handle):\n") {
                        let candidati: Vec<Value> =
                            serde_json::from_str(&utente[i + "handle):\n".len()..]).unwrap();
                        let opzioni: Vec<Value> = candidati
                            .iter()
                            .filter(|c| c["descrizione"].as_str().unwrap().to_lowercase().starts_with("massetto"))
                            .take(4)
                            .enumerate()
                            .map(|(i, c)| json!({"candidato": c["handle"], "pertinenza": if i == 0 { "alta" } else { "media" }, "motivazione": "massetto"}))
                            .collect();
                        json!({
                            "lavorazioni": [{
                                "id": "L01", "descrizione": "Massetto", "chiarezza": "chiara", "da_precisare": "",
                                "righe": [{"descrizione": "stanza", "forma": "rettangolo", "segno": "positivo", "parametri": [
                                    {"nome": "N", "valore": "1", "unita": "n", "provenienza": "utente", "rif_ipotesi": ""},
                                    {"nome": "L", "valore": "4", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""},
                                    {"nome": "B", "valore": "3", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""}]}],
                                "opzioni": opzioni
                            }],
                            "ipotesi": [], "informazioni_mancanti": []
                        })
                    } else {
                        json!({"esito": "procedi", "lavorazioni": [{"id": "L01", "descrizione": "Massetto", "query_ricerca": "massetto cemento", "dimensione_attesa": "superficie"}], "informazioni_mancanti": []})
                    };
                    rispondi(
                        s,
                        200,
                        &json!({"content": [{"type": "text", "text": risposta.to_string()}], "stop_reason": "end_turn", "usage": {"input_tokens": 50, "output_tokens": 10}}).to_string(),
                    );
                });
            }
        });
        p
    }

    fn regole(&self, f: impl FnOnce(&mut Regole)) {
        f(&mut self.regole.lock().unwrap());
    }
}

fn servizio(d: &Path, p: &Provider, chiavi: &Arc<ChiaviInMemoria>) -> Servizio {
    Servizio::new().con_ambiente(Ambiente {
        cartella: d.join("config"),
        chiavi: chiavi.clone(),
        trasporto: Arc::new(TrasportoHttps::new().unwrap()),
        attesa: Arc::new(AttesaReale),
        indirizzi: Indirizzi {
            embedding: format!("{}/v1/embeddings", p.base),
            generazione: format!("{}/v1/messages", p.base),
        },
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
    let testo = r.to_string();
    assert!(
        !testo.contains(CHIAVE_OPENAI) && !testo.contains(CHIAVE_ANTHROPIC),
        "chiave in un errore: {testo}"
    );
    r["errore"].clone()
}

/// Progetto su file con il listino caricato come «listino», consenso, chiavi, dimensioni.
fn prepara(s: &mut Servizio, d: &Path) {
    let csv = d.join("listino.csv");
    if !csv.exists() {
        std::fs::write(&csv, listino(VOCI)).unwrap();
    }
    chiedi(
        s,
        json!({"richiesta": "nuovo", "titolo": "Robustezza", "percorso": d.join("lavori.ocp")}),
    );
    chiedi(
        s,
        json!({"richiesta": "carica_prezzario", "nome": "listino", "percorso": csv}),
    );
    chiedi(
        s,
        json!({"richiesta": "assistente_consenso", "accetto": true, "versione": 1}),
    );
    chiedi(
        s,
        json!({"richiesta": "assistente_configura", "configurazione": {"dimensioni_embedding": DIM}}),
    );
    chiedi(
        s,
        json!({"richiesta": "assistente_chiave", "provider": "openai", "chiave": CHIAVE_OPENAI}),
    );
    chiedi(
        s,
        json!({"richiesta": "assistente_chiave", "provider": "anthropic", "chiave": CHIAVE_ANTHROPIC}),
    );
}

fn pronto(nome: &str) -> (PathBuf, Provider, Arc<ChiaviInMemoria>, Servizio) {
    let d = cartella(nome);
    let p = Provider::avvia();
    let chiavi = Arc::new(ChiaviInMemoria::default());
    let mut s = servizio(&d, &p, &chiavi);
    prepara(&mut s, &d);
    (d, p, chiavi, s)
}

fn lavoro(s: &mut Servizio, v: Value) -> Box<opencomputo_agent_api::assistente::Lavoro> {
    match s.avvia(v) {
        Passo::Lavoro(l) => l,
        Passo::Risposta { risposta, .. } => panic!("atteso un lavoro: {risposta:#}"),
    }
}

fn file_cache(d: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(d.join("config/cache"))
        .map(|r| r.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default()
}

#[test]
fn flusso_completo_con_trasporto_reale() {
    let (d, p, _chiavi, s) = pronto("flusso");
    let avanzamenti = Arc::new(Mutex::new(Vec::new()));
    let a2 = avanzamenti.clone();
    let mut s = s.con_avanzamento(Box::new(move |v: &Value| {
        a2.lock().unwrap().push(v.clone())
    }));

    // Indicizzazione: tre lotti, tre avanzamenti, un file di cache.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(
        r["rapporto"],
        json!({"voci": VOCI, "riusate": 0, "calcolate": VOCI, "rimosse": 0, "lotti": 3})
    );
    let stati: Vec<(u64, u64)> = avanzamenti
        .lock()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["stato"]["lotto"].as_u64().unwrap(),
                a["stato"]["calcolate"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(stati, [(1, 256), (2, 512), (3, 600)]);
    assert_eq!(p.testi_embedding.load(Ordering::SeqCst), VOCI);
    assert_eq!(file_cache(&d).len(), 1);

    // Ricerca ibrida e proposta, con due sovraccarichi (529) del modello prima della risposta.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_cerca", "prezzario": "listino", "testo": "massetto in cemento", "limite": 5, "unita": "m2"}),
    );
    assert_eq!(r["ricerca"], "ibrida");
    assert!(
        r["voci"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["descrizione"].as_str().unwrap().contains("Massetto")),
        "{r:#}"
    );
    p.regole(|r| r.sovraccarichi = 2);
    let inizio = Instant::now();
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "Massetto in una stanza di 4 x 3 m"}),
    );
    assert!(
        inizio.elapsed() >= Duration::from_secs(2),
        "attese vere tra i tentativi: {:?}",
        inizio.elapsed()
    );
    assert_eq!(
        p.messaggi.load(Ordering::SeqCst),
        4,
        "due 529, analisi, selezione"
    );
    assert_eq!(r["ricerca_ibrida"], true);
    let opzioni = r["proposta"]["lavorazioni"][0]["opzioni"]
        .as_array()
        .unwrap();
    assert!(!opzioni.is_empty() && opzioni[0]["preselezionata"] == true);
    let codice = opzioni[0]["codice"].as_str().unwrap().to_owned();
    // 12,00 m² al prezzo del listino: importo calcolato dal motore, in centesimi interi.
    let centesimi = |t: &str| -> u64 {
        let (i, d) = t.split_once('.').unwrap_or((t, "0"));
        i.parse::<u64>().unwrap() * 100 + format!("{d:0<2}")[..2].parse::<u64>().unwrap()
    };
    let importo = r["anteprima"]["opzioni"][0]["importo"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        centesimi(&importo),
        centesimi(opzioni[0]["prezzo"].as_str().unwrap()) * 12,
        "{r:#}"
    );

    // Applicazione, salvataggio su file, riapertura in un nuovo servizio.
    let e = chiedi(&mut s, json!({"richiesta": "assistente_applica"}));
    assert_eq!(e["voci"][0]["codice"], codice.as_str());
    chiedi(&mut s, json!({"richiesta": "salva"}));
    let mut s2 = Servizio::new();
    chiedi(
        &mut s2,
        json!({"richiesta": "apri", "percorso": d.join("lavori.ocp")}),
    );
    let c = chiedi(&mut s2, json!({"richiesta": "computo"}));
    assert_eq!(c["voci"][0]["codice"], codice.as_str());
    assert_eq!(c["voci"][0]["quantita"], "12.00");
    assert_eq!(
        centesimi(c["totale"].as_str().unwrap()),
        centesimi(&importo)
    );

    // Le chiavi sono arrivate solo ai rispettivi provider, e non stanno in nessun file.
    let viste = p.chiavi_viste.lock().unwrap().clone();
    assert!(
        viste
            .iter()
            .all(|c| c == CHIAVE_OPENAI || c == CHIAVE_ANTHROPIC)
            && viste.len() >= 8
    );
    fn cerca_chiavi(d: &Path) {
        for e in std::fs::read_dir(d).unwrap().filter_map(Result::ok) {
            if e.path().is_dir() {
                cerca_chiavi(&e.path());
            } else {
                let b = std::fs::read(e.path()).unwrap();
                let t = String::from_utf8_lossy(&b);
                assert!(
                    !t.contains(CHIAVE_OPENAI) && !t.contains(CHIAVE_ANTHROPIC),
                    "chiave in {}",
                    e.path().display()
                );
            }
        }
    }
    cerca_chiavi(&d);
}

#[test]
fn indicizzazione_interrotta_e_ripresa_in_un_altro_processo() {
    let (d, p, chiavi, mut s) = pronto("ripresa");
    // Il provider rifiuta la chiave dalla seconda chiamata: resta salvato il primo lotto.
    p.regole(|r| r.embedding_rifiutati_da = Some(2));
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(e["motivo"], "chiave_rifiutata");
    assert_eq!(
        p.embedding.load(Ordering::SeqCst),
        2,
        "nessun nuovo tentativo con la chiave rifiutata"
    );
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(
        (
            i["indicizzate"].as_u64(),
            i["completo"].as_bool(),
            i["ricerca"].as_str()
        ),
        (Some(256), Some(false), Some("testuale"))
    );
    // Con l'indice a metà la proposta funziona lo stesso, con la ricerca per parole.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "Massetto di 4 x 3 m"}),
    );
    assert_eq!(r["ricerca_ibrida"], false);
    assert!(r["avviso_ricerca"].as_str().unwrap().contains("incompleto"));
    drop(s);

    // «Nuovo processo»: stesso disco, niente in memoria. Si calcolano solo le voci mancanti.
    p.regole(|r| r.embedding_rifiutati_da = None);
    let prima = p.testi_embedding.load(Ordering::SeqCst);
    let mut s = servizio(&d, &p, &chiavi);
    prepara(&mut s, &d);
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(i["indicizzate"], 256);
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(
        r["rapporto"],
        json!({"voci": VOCI, "riusate": 256, "calcolate": VOCI - 256, "rimosse": 0, "lotti": 2})
    );
    assert_eq!(p.testi_embedding.load(Ordering::SeqCst) - prima, VOCI - 256);
}

#[test]
fn annullamento_e_lavori_contemporanei() {
    let (_d, p, _chiavi, mut s) = pronto("insieme");
    p.regole(|r| r.ritardo_embedding = Duration::from_millis(400));
    let interruttore = s.interruttore();
    let indicizza = lavoro(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino", "id": "idx"}),
    );
    assert_eq!(interruttore.in_corso(), 1);
    let t = thread::spawn(move || indicizza.esegui(None));
    thread::sleep(Duration::from_millis(150));

    // Mentre l'indicizzazione aspetta il provider, il servizio risponde subito a tutto.
    let inizio = Instant::now();
    chiedi(&mut s, json!({"richiesta": "info"}));
    chiedi(&mut s, json!({"richiesta": "assistente_stato"}));
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(
        (i["in_corso"].as_bool(), i["ricerca"].as_str()),
        (Some(true), Some("testuale"))
    );
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_cerca", "prezzario": "listino", "testo": "pavimento gres"}),
    );
    assert_eq!(r["ricerca"], "testuale");
    // Anche una proposta sullo stesso prezzario non aspetta la fine dell'indicizzazione.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "Massetto di 4 x 3 m"}),
    );
    assert!(
        r["avviso_ricerca"]
            .as_str()
            .unwrap()
            .contains("indicizzazione in corso")
    );
    assert!(
        inizio.elapsed() < Duration::from_millis(1500),
        "bloccato dall'indicizzazione: {:?}",
        inizio.elapsed()
    );
    // Una seconda indicizzazione dello stesso prezzario è rifiutata, non accodata.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(e["motivo"], "indicizzazione_in_corso");

    // Annullamento: smette entro poco, anche se il provider non ha ancora risposto.
    let inizio = Instant::now();
    let r = chiedi(&mut s, json!({"richiesta": "assistente_annulla"}));
    assert_eq!(r["lavori"], 1);
    let (risposta, _) = s.completa(t.join().unwrap());
    assert!(
        inizio.elapsed() < Duration::from_millis(700),
        "{:?}",
        inizio.elapsed()
    );
    assert_eq!(
        (
            risposta["id"].as_str(),
            risposta["errore"]["motivo"].as_str()
        ),
        (Some("idx"), Some("annullato")),
        "{risposta:#}"
    );
    assert_eq!(interruttore.in_corso(), 0);

    // Dopo l'annullamento l'indice parziale è al suo posto e si può riprendere.
    p.regole(|r| r.ritardo_embedding = Duration::ZERO);
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(i["in_corso"], false);
    let fatte = i["indicizzate"].as_u64().unwrap();
    assert!(fatte < VOCI as u64);
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(r["rapporto"]["riusate"].as_u64().unwrap(), fatte);
    assert_eq!(
        chiedi(
            &mut s,
            json!({"richiesta": "assistente_indice", "prezzario": "listino"})
        )["completo"],
        true
    );

    // Due proposte lente insieme: «Annulla» le ferma entrambe.
    p.regole(|r| r.ritardo_messaggi = Duration::from_secs(5));
    let a = lavoro(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "Massetto di 4 x 3 m", "id": 1}),
    );
    let b = lavoro(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "Massetto di 5 x 3 m", "id": 2}),
    );
    let (ta, tb) = (
        thread::spawn(move || a.esegui(None)),
        thread::spawn(move || b.esegui(None)),
    );
    thread::sleep(Duration::from_millis(300));
    let inizio = Instant::now();
    assert_eq!(interruttore.annulla(), 2);
    for t in [ta, tb] {
        let (r, _) = s.completa(t.join().unwrap());
        assert_eq!(r["errore"]["motivo"], "annullato");
    }
    assert!(
        inizio.elapsed() < Duration::from_millis(700),
        "{:?}",
        inizio.elapsed()
    );
    // Resta valida la proposta precedente (4 x 3 m), non un miscuglio con quelle annullate.
    let e = chiedi(&mut s, json!({"richiesta": "assistente_applica"}));
    assert_eq!(e["voci"].as_array().unwrap().len(), 1, "{e:#}");
    let c = chiedi(&mut s, json!({"richiesta": "computo"}));
    assert_eq!(c["voci"][0]["quantita"], "12.00", "{c:#}");
}

#[test]
fn prezzario_ricaricato_durante_l_indicizzazione() {
    let (d, p, _chiavi, mut s) = pronto("ricaricato");
    p.regole(|r| r.ritardo_embedding = Duration::from_millis(250));
    let l = lavoro(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    let t = thread::spawn(move || l.esegui(None));
    thread::sleep(Duration::from_millis(100));
    // L'utente carica un altro listino con lo stesso nome mentre il primo si indicizza.
    let altro = d.join("altro.csv");
    std::fs::write(&altro, listino(40)).unwrap();
    chiedi(
        &mut s,
        json!({"richiesta": "carica_prezzario", "nome": "listino", "percorso": altro}),
    );
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(
        (i["voci"].as_u64(), i["in_corso"].as_bool()),
        (Some(40), Some(false))
    );
    // Il lavoro vecchio finisce bene (ha salvato la sua cache) e non tocca gli indici nuovi.
    let (r, _) = s.completa(t.join().unwrap());
    assert_eq!(r["risultato"]["rapporto"]["voci"], VOCI, "{r:#}");
    p.regole(|r| r.ritardo_embedding = Duration::ZERO);
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(
        (i["voci"].as_u64(), i["indicizzate"].as_u64()),
        (Some(40), Some(0))
    );
    // Le 40 voci sono le prime del listino grande: stessi codici e stessi testi, quindi
    // la nuova indicizzazione le prende dalla cache appena scritta senza chiamate.
    let prima = p.embedding.load(Ordering::SeqCst);
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(
        r["rapporto"],
        json!({"voci": 40, "riusate": 40, "calcolate": 0, "rimosse": VOCI - 40, "lotti": 0})
    );
    assert_eq!(p.embedding.load(Ordering::SeqCst), prima);
}

#[test]
fn configurazione_e_cache_danneggiate() {
    let (d, p, chiavi, mut s) = pronto("danneggiati");
    chiedi(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    drop(s);
    let config = d.join("config").join(FILE_CONFIGURAZIONE);
    let cache = file_cache(&d).pop().unwrap();
    let buona = std::fs::read(&cache).unwrap();

    // Configurazione troncata a metà (scrittura interrotta): si riparte dai valori
    // predefiniti, senza consenso, con un avviso; il file rotto è messo da parte.
    let testo = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, &testo[..testo.len() / 2]).unwrap();
    let mut s = servizio(&d, &p, &chiavi);
    let st = chiedi(&mut s, json!({"richiesta": "assistente_stato"}));
    assert!(
        st["avviso"].as_str().unwrap().contains("non era leggibile"),
        "{st:#}"
    );
    assert_eq!(st["consenso"]["valido"], false);
    assert!(d.join("config/assistente.json.danneggiato").exists());
    chiedi(&mut s, json!({"richiesta": "nuovo", "titolo": "x"}));
    chiedi(
        &mut s,
        json!({"richiesta": "carica_prezzario", "nome": "listino", "percorso": d.join("listino.csv")}),
    );
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "massetto"}),
    );
    assert_eq!(e["motivo"], "consenso_mancante");
    drop(s);

    // Cache danneggiata in vari modi: ignorata (ricerca per parole), poi ricostruita.
    for (nome, byte) in [
        ("troncata", buona[..buona.len() / 3].to_vec()),
        ("vuota", Vec::new()),
        ("spazzatura", vec![0xAB; 5000]),
        ("coda in più", [buona.clone(), vec![1, 2, 3]].concat()),
    ] {
        std::fs::write(&cache, &byte).unwrap();
        // Resto di una scrittura interrotta di un altro processo.
        std::fs::write(cache.with_extension("bin.99999.tmp"), b"a meta").unwrap();
        let mut s = servizio(&d, &p, &chiavi);
        prepara(&mut s, &d);
        let i = chiedi(
            &mut s,
            json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
        );
        assert_eq!(
            (i["indicizzate"].as_u64(), i["ricerca"].as_str()),
            (Some(0), Some("testuale")),
            "{nome}"
        );
        let r = chiedi(
            &mut s,
            json!({"richiesta": "assistente_cerca", "prezzario": "listino", "testo": "massetto"}),
        );
        assert_eq!(r["ricerca"], "testuale", "{nome}");
        let r = chiedi(
            &mut s,
            json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
        );
        assert_eq!(r["rapporto"]["calcolate"], VOCI, "{nome}");
        assert_eq!(
            std::fs::read(&cache).unwrap(),
            buona,
            "{nome}: la cache ricostruita è identica"
        );
    }

    // Cartella della cache non scrivibile (al suo posto c'è un file): errore chiaro,
    // e l'indice calcolato resta utilizzabile in memoria.
    let cartella_cache = d.join("config/cache");
    std::fs::remove_dir_all(&cartella_cache).unwrap();
    std::fs::write(&cartella_cache, b"non una cartella").unwrap();
    let mut s = servizio(&d, &p, &chiavi);
    prepara(&mut s, &d);
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "assistente_indicizza", "prezzario": "listino"}),
    );
    assert_eq!(e["codice"], "file", "{e:#}");
    let i = chiedi(
        &mut s,
        json!({"richiesta": "assistente_indice", "prezzario": "listino"}),
    );
    assert_eq!(
        (i["completo"].as_bool(), i["ricerca"].as_str()),
        (Some(true), Some("ibrida"))
    );
}

#[test]
fn richieste_malformate_e_limiti() {
    let (_d, p, _chiavi, mut s) = pronto("malformate");
    // Righe qualsiasi: sempre una risposta JSON con `ok`, mai un blocco.
    for riga in [
        "",
        " ",
        "{",
        "[]",
        "null",
        "42",
        "\"assistente_stato\"",
        "{\"richiesta\": 7}",
        "{\"richiesta\": \"inesistente\"}",
        "{\"richiesta\": \"assistente_proponi\"}",
        "{\"richiesta\": \"assistente_proponi\", \"prezzario\": 1, \"testo\": []}",
        "{\"richiesta\": \"assistente_cerca\", \"prezzario\": \"listino\", \"testo\": \"x\", \"limite\": -1}",
        "{\"richiesta\": \"assistente_cerca\", \"prezzario\": \"listino\", \"testo\": \"x\", \"limite\": 1e400}",
        "{\"richiesta\": \"assistente_applica\", \"scelte\": \"tutte\"}",
        "{\"richiesta\": \"assistente_anteprima\"}",
        "{\"richiesta\": \"assistente_chiave\", \"provider\": \"google\", \"chiave\": \"k\"}",
        "{\"richiesta\": \"assistente_configura\", \"configurazione\": {\"max_token_analisi\": -5}}",
        "{\"richiesta\": \"assistente_configura\", \"configurazione\": {\"dimensioni_embedding\": 99999999999}}",
        "{\"richiesta\": \"assistente_consenso\", \"accetto\": \"sì\"}",
        "{\"richiesta\": \"assistente_stato\", \"extra\": true}",
        "\u{feff}{\"richiesta\": \"assistente_stato\"}",
    ] {
        let r = s.rispondi(riga);
        assert_eq!(r["ok"], false, "{riga} → {r}");
        assert!(r["errore"]["descrizione"].is_string(), "{riga} → {r}");
    }
    // Limiti: richiesta vuota o enorme rifiutate prima di qualsiasi chiamata.
    let prima = p.messaggi.load(Ordering::SeqCst);
    for (testo, motivo) in [
        (" \n\t".to_owned(), "richiesta_vuota"),
        ("a ".repeat(RICHIESTA_MASSIMA), "richiesta_troppo_lunga"),
    ] {
        let e = rifiuta(
            &mut s,
            json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": testo}),
        );
        assert_eq!(e["motivo"], motivo);
    }
    assert_eq!(p.messaggi.load(Ordering::SeqCst), prima);
    // Testi insoliti ma leciti: gestiti.
    for testo in [
        "Massetto 4×3 m 🚧",
        "MASSETTO\u{0}\u{1}\u{202e} di 4 x 3 m",
        "massetto\r\n\r\n4 x 3 m",
        &"massetto di 4 x 3 m, ".repeat(900),
    ] {
        let r = chiedi(
            &mut s,
            json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": testo}),
        );
        assert_eq!(r["esito"], "proposta", "{testo:?}");
        let r = chiedi(
            &mut s,
            json!({"richiesta": "assistente_cerca", "prezzario": "listino", "testo": testo, "limite": 3}),
        );
        assert!(r["voci"].as_array().unwrap().len() <= 3);
    }
    // Limite di ricerca fuori misura: mai più voci di quelle del prezzario, nessun errore.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_cerca", "prezzario": "listino", "testo": "massetto", "limite": 1_000_000}),
    );
    assert!(r["voci"].as_array().unwrap().len() <= VOCI);
    // Il servizio è ancora in piedi.
    assert_eq!(
        chiedi(&mut s, json!({"richiesta": "assistente_stato"}))["consenso"]["valido"],
        true
    );
}

#[test]
fn provider_irraggiungibile() {
    let d = cartella("irraggiungibile");
    // Porta su cui nessuno ascolta.
    let porta = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let p = Provider {
        base: format!("http://127.0.0.1:{porta}"),
        regole: Arc::default(),
        embedding: Arc::default(),
        messaggi: Arc::default(),
        testi_embedding: Arc::default(),
        chiavi_viste: Arc::default(),
    };
    let chiavi = Arc::new(ChiaviInMemoria::default());
    let mut s = servizio(&d, &p, &chiavi);
    prepara(&mut s, &d);
    // L'annullamento interrompe anche le attese tra i tentativi di connessione.
    let interruttore = s.interruttore();
    let l = lavoro(
        &mut s,
        json!({"richiesta": "assistente_proponi", "prezzario": "listino", "testo": "massetto 4 x 3 m"}),
    );
    let t = thread::spawn(move || l.esegui(None));
    thread::sleep(Duration::from_millis(300));
    let inizio = Instant::now();
    interruttore.annulla();
    let (r, _) = s.completa(t.join().unwrap());
    assert!(inizio.elapsed() < Duration::from_millis(700));
    assert_eq!(r["errore"]["motivo"], "annullato");
    // La ricerca per parole e il resto dell'applicazione funzionano senza rete.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "assistente_cerca", "prezzario": "listino", "testo": "massetto cemento"}),
    );
    assert_eq!(r["ricerca"], "testuale");
    assert!(!r["voci"].as_array().unwrap().is_empty());
    chiedi(&mut s, json!({"richiesta": "salva"}));
}

/// Ingresso alimentato a pezzi da un altro thread, come lo standard input di un processo.
struct Tubo(std::sync::mpsc::Receiver<Vec<u8>>, Vec<u8>);

impl Read for Tubo {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if self.1.is_empty() {
            match self.0.recv() {
                Ok(v) => self.1 = v,
                Err(_) => return Ok(0),
            }
        }
        let n = b.len().min(self.1.len());
        b[..n].copy_from_slice(&self.1[..n]);
        self.1.drain(..n);
        Ok(n)
    }
}

#[test]
fn servizio_a_righe_annullamento_durante_un_lavoro() {
    let (_d, p, _chiavi, mut s) = pronto("righe");
    p.regole(|r| r.ritardo_messaggi = Duration::from_secs(6));
    let (tx, rx) = std::sync::mpsc::channel();
    let invia = move |v: Value| tx.send(format!("{v}\n").into_bytes()).unwrap();
    let scrittore = thread::spawn(move || {
        invia(
            json!({"id": 1, "richiesta": "assistente_proponi", "prezzario": "listino", "testo": "Massetto di 4 x 3 m"}),
        );
        thread::sleep(Duration::from_millis(400));
        // Arrivano mentre il lavoro aspetta il modello: l'annullamento agisce subito, le
        // altre aspettano il loro turno. Una riga rotta in mezzo non disturba.
        invia(json!({"id": 2, "richiesta": "info"}));
        invia(json!({"id": 3, "richiesta": "assistente_annulla"}));
        tx_riga_rotta(&invia);
        invia(json!({"id": 5, "richiesta": "assistente_annulla"}));
    });
    fn tx_riga_rotta(invia: &impl Fn(Value)) {
        invia(json!("non una richiesta"));
    }
    let inizio = Instant::now();
    let mut uscita = Vec::new();
    s.servi(BufReader::new(Tubo(rx, Vec::new())), &mut uscita)
        .unwrap();
    scrittore.join().unwrap();
    assert!(
        inizio.elapsed() < Duration::from_secs(2),
        "{:?}",
        inizio.elapsed()
    );
    let righe: Vec<Value> = String::from_utf8(uscita)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    // Risposte nell'ordine delle richieste.
    assert_eq!(righe.len(), 5, "{righe:#?}");
    assert_eq!(
        (
            righe[0]["id"].as_u64(),
            righe[0]["errore"]["motivo"].as_str()
        ),
        (Some(1), Some("annullato"))
    );
    assert_eq!(
        (righe[1]["id"].as_u64(), righe[1]["ok"].as_bool()),
        (Some(2), Some(true))
    );
    assert_eq!(
        righe[2],
        json!({"id": 3, "ok": true, "risultato": {"annullamento": "chiesto", "lavori": 1}})
    );
    assert_eq!(righe[3]["ok"], false);
    assert_eq!(
        (righe[4]["id"].as_u64(), righe[4]["ok"].as_bool()),
        (Some(5), Some(true))
    );
}
