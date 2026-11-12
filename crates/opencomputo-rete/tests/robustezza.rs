//! Client dei provider con il trasporto reale contro un server locale (127.0.0.1) che si
//! comporta male come capita in rete: risposte lente, troncate, pagine HTML di un proxy,
//! connessioni chiuse, risposte a pezzi, dati sbagliati. Nessuna rete esterna.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use opencomputo_assistente::provider::anthropic::{
    Chiamata, ClienteGenerazione, ConfigGenerazione,
};
use opencomputo_assistente::provider::openai::{ClienteEmbedding, ConfigEmbedding};
use opencomputo_assistente::provider::riprova::PoliticaRiprova;
use opencomputo_assistente::provider::{Annullamento, Embedding, ErroreAi, Generazione, Segreto};
use opencomputo_rete::{AttesaReale, TrasportoHttps};
use serde_json::{Value, json};

const CHIAVE: &str = "sk-segreta-da-non-mostrare";

/// Comportamento del server per una connessione.
enum Mossa {
    /// Risposta completa: stato, intestazioni in più, corpo.
    Risposta(u16, Vec<(&'static str, String)>, String),
    /// Legge la richiesta e chiude senza rispondere.
    Chiudi,
    /// Dichiara una lunghezza e invia solo una parte del corpo.
    Tronca(String),
    /// Aspetta prima di rispondere 200 con il corpo.
    Lenta(Duration, String),
    /// Risposta 200 in `Transfer-Encoding: chunked`, a pezzi di 7 byte.
    APezzi(String),
}

fn leggi_richiesta(s: &TcpStream) -> Vec<u8> {
    let mut lettore = BufReader::new(s.try_clone().unwrap());
    let mut lunghezza = 0usize;
    loop {
        let mut riga = String::new();
        if lettore.read_line(&mut riga).unwrap_or(0) == 0 || riga == "\r\n" {
            break;
        }
        if let Some((n, v)) = riga.split_once(':')
            && n.eq_ignore_ascii_case("content-length")
        {
            lunghezza = v.trim().parse().unwrap_or(0);
        }
    }
    let mut corpo = vec![0u8; lunghezza];
    let _ = lettore.read_exact(&mut corpo);
    corpo
}

/// Server che esegue una mossa per connessione; conta le richieste ricevute.
fn server(mosse: Vec<Mossa>) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/x", l.local_addr().unwrap());
    let ricevute = Arc::new(AtomicUsize::new(0));
    let r2 = ricevute.clone();
    thread::spawn(move || {
        for m in mosse {
            let Ok((mut s, _)) = l.accept() else { return };
            leggi_richiesta(&s);
            r2.fetch_add(1, Ordering::SeqCst);
            let testa = |stato: u16, extra: &str, n: usize| {
                format!(
                    "HTTP/1.1 {stato} X\r\ncontent-length: {n}\r\nconnection: close\r\n{extra}\r\n"
                )
            };
            match m {
                Mossa::Risposta(stato, intestazioni, corpo) => {
                    let extra: String = intestazioni
                        .iter()
                        .map(|(n, v)| format!("{n}: {v}\r\n"))
                        .collect();
                    let _ = s.write_all(testa(stato, &extra, corpo.len()).as_bytes());
                    let _ = s.write_all(corpo.as_bytes());
                }
                Mossa::Chiudi => {}
                Mossa::Tronca(corpo) => {
                    let _ = s.write_all(testa(200, "", corpo.len() + 500).as_bytes());
                    let _ = s.write_all(corpo.as_bytes());
                }
                Mossa::Lenta(d, corpo) => {
                    thread::sleep(d);
                    let _ = s.write_all(testa(200, "", corpo.len()).as_bytes());
                    let _ = s.write_all(corpo.as_bytes());
                }
                Mossa::APezzi(corpo) => {
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n");
                    for pezzo in corpo.as_bytes().chunks(7) {
                        let _ = s.write_all(format!("{:x}\r\n", pezzo.len()).as_bytes());
                        let _ = s.write_all(pezzo);
                        let _ = s.write_all(b"\r\n");
                        let _ = s.flush();
                    }
                    let _ = s.write_all(b"0\r\n\r\n");
                }
            }
        }
    });
    (url, ricevute)
}

fn messaggio() -> String {
    json!({
        "content": [{"type": "text", "text": "{\"esito\": \"ok\"}"}],
        "stop_reason": "end_turn", "usage": {"input_tokens": 3, "output_tokens": 2}
    })
    .to_string()
}

fn ok(corpo: String) -> Mossa {
    Mossa::Risposta(200, vec![], corpo)
}

fn veloce() -> PoliticaRiprova {
    PoliticaRiprova {
        attesa_iniziale: Duration::from_millis(10),
        ..PoliticaRiprova::default()
    }
}

fn generazione(url: String) -> ClienteGenerazione {
    ClienteGenerazione::new(
        ConfigGenerazione {
            url,
            ..ConfigGenerazione::default()
        },
        Segreto::new(CHIAVE),
        Arc::new(TrasportoHttps::new().unwrap()),
        Arc::new(AttesaReale),
    )
    .unwrap()
    .con_politica(veloce())
}

fn chiamata() -> Chiamata {
    Chiamata {
        sistema: "s".into(),
        utente: "u".into(),
        schema: json!({"type": "object"}),
        nome: "n".into(),
        descrizione: "d".into(),
        max_tokens: 100,
    }
}

fn genera(mosse: Vec<Mossa>) -> (Result<Value, ErroreAi>, usize) {
    let (url, ricevute) = server(mosse);
    let r = generazione(url)
        .genera(&chiamata(), &Annullamento::new())
        .map(|r| r.json);
    if let Err(e) = &r {
        assert!(
            !e.to_string().contains(CHIAVE) && !format!("{e:?}").contains(CHIAVE),
            "{e:?}"
        );
    }
    (r, ricevute.load(Ordering::SeqCst))
}

#[test]
fn connessione_chiusa_e_risposta_troncata_si_ritentano() {
    let (r, n) = genera(vec![
        Mossa::Chiudi,
        Mossa::Tronca("{\"content\": [".into()),
        ok(messaggio()),
    ]);
    assert_eq!(r.unwrap(), json!({"esito": "ok"}));
    assert_eq!(n, 3);
}

#[test]
fn server_sempre_irraggiungibile_tentativi_finiti() {
    let (r, n) = genera((0..5).map(|_| Mossa::Chiudi).collect());
    let e = r.unwrap_err();
    assert_eq!(e.codice(), "rete", "{e:?}");
    assert_eq!(n, 5, "cinque tentativi e non di più");
}

#[test]
fn pagina_html_di_un_proxy() {
    // 200 con una pagina HTML: risposta non valida, senza nuovi tentativi.
    let (r, n) = genera(vec![
        ok("<html><body>Accesso alla rete: accettare le condizioni</body></html>".into()),
        ok(messaggio()),
    ]);
    assert_eq!(r.unwrap_err().codice(), "risposta_non_valida");
    assert_eq!(n, 1);
    // 502 e 503 con corpo HTML: temporanei, poi la risposta buona.
    let html = |stato| Mossa::Risposta(stato, vec![], "<html>Bad Gateway</html>".into());
    let (r, n) = genera(vec![html(502), html(503), ok(messaggio())]);
    assert_eq!(r.unwrap(), json!({"esito": "ok"}));
    assert_eq!(n, 3);
    // 404 con HTML (indirizzo sbagliato): errore definitivo con lo stato.
    let (r, n) = genera(vec![html(404), ok(messaggio())]);
    let e = r.unwrap_err();
    assert!(matches!(e, ErroreAi::Http { stato: 404, .. }), "{e:?}");
    assert_eq!(n, 1);
}

#[test]
fn risposta_a_pezzi_e_risposte_strane_del_modello() {
    let (r, _) = genera(vec![Mossa::APezzi(messaggio())]);
    assert_eq!(r.unwrap(), json!({"esito": "ok"}));
    let con = |v: Value| genera(vec![ok(v.to_string())]).0;
    // JSON valido ma senza i campi attesi, blocchi vuoti, testo che non è JSON.
    for (v, codice) in [
        (json!({}), "risposta_non_valida"),
        (json!([1, 2, 3]), "risposta_non_valida"),
        (
            json!({"content": [], "stop_reason": "end_turn"}),
            "risposta_non_valida",
        ),
        (
            json!({"content": [{"type": "text", "text": "Ecco il JSON: {}"}], "stop_reason": "end_turn"}),
            "risposta_non_valida",
        ),
        (
            json!({"content": [{"type": "text", "text": "{\"a\": "}], "stop_reason": "max_tokens"}),
            "troncata",
        ),
        (json!({"content": [], "stop_reason": "refusal"}), "rifiuto"),
    ] {
        assert_eq!(con(v.clone()).unwrap_err().codice(), codice, "{v}");
    }
    // Corpo vuoto con 200.
    assert_eq!(
        genera(vec![ok(String::new())]).0.unwrap_err().codice(),
        "risposta_non_valida"
    );
}

#[test]
fn retry_after_non_numerico_o_enorme() {
    // Una data HTTP non si interpreta: si usa l'attesa calcolata (qui 10 ms).
    let inizio = Instant::now();
    let (r, n) = genera(vec![
        Mossa::Risposta(
            429,
            vec![("retry-after", "Wed, 21 Oct 2026 07:28:00 GMT".into())],
            "{}".into(),
        ),
        ok(messaggio()),
    ]);
    assert!(r.is_ok() && n == 2);
    assert!(inizio.elapsed() < Duration::from_secs(3));
    // Un'attesa di un'ora non si rispetta: errore subito, senza restare bloccati.
    let inizio = Instant::now();
    let (r, n) = genera(vec![
        Mossa::Risposta(429, vec![("retry-after", "3600".into())], "{}".into()),
        ok(messaggio()),
    ]);
    assert!(matches!(r.unwrap_err(), ErroreAi::Http { stato: 429, .. }));
    assert_eq!(n, 1);
    assert!(inizio.elapsed() < Duration::from_secs(3));
}

#[test]
fn annullamento_durante_una_risposta_lenta() {
    let (url, ricevute) = server(vec![
        Mossa::Lenta(Duration::from_secs(8), messaggio()),
        ok(messaggio()),
    ]);
    let c = generazione(url);
    let annullamento = Annullamento::new();
    let a2 = annullamento.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));
        a2.annulla();
    });
    let inizio = Instant::now();
    let e = c.genera(&chiamata(), &annullamento).unwrap_err();
    assert_eq!(e.codice(), "annullato", "{e:?}");
    assert!(
        inizio.elapsed() < Duration::from_secs(2),
        "{:?}",
        inizio.elapsed()
    );
    assert_eq!(
        ricevute.load(Ordering::SeqCst),
        1,
        "nessun nuovo tentativo dopo l'annullamento"
    );
}

#[test]
fn annullamento_durante_l_attesa_di_retry_after() {
    let (url, ricevute) = server(vec![
        Mossa::Risposta(429, vec![("retry-after", "60".into())], "{}".into()),
        ok(messaggio()),
    ]);
    let c = generazione(url);
    let annullamento = Annullamento::new();
    let a2 = annullamento.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));
        a2.annulla();
    });
    let inizio = Instant::now();
    let e = c.genera(&chiamata(), &annullamento).unwrap_err();
    assert_eq!(e.codice(), "annullato");
    assert!(
        inizio.elapsed() < Duration::from_secs(2),
        "{:?}",
        inizio.elapsed()
    );
    assert_eq!(ricevute.load(Ordering::SeqCst), 1);
}

#[test]
fn tempo_massimo_della_risposta() {
    let (url, ricevute) = server(vec![
        Mossa::Lenta(Duration::from_secs(3), messaggio()),
        Mossa::Lenta(Duration::from_secs(3), messaggio()),
    ]);
    let c = ClienteGenerazione::new(
        ConfigGenerazione {
            url,
            timeout: Duration::from_millis(300),
            ..ConfigGenerazione::default()
        },
        Segreto::new(CHIAVE),
        Arc::new(TrasportoHttps::new().unwrap()),
        Arc::new(AttesaReale),
    )
    .unwrap()
    .con_politica(PoliticaRiprova {
        tentativi: 2,
        ..veloce()
    });
    let inizio = Instant::now();
    let e = c.genera(&chiamata(), &Annullamento::new()).unwrap_err();
    assert_eq!(e.codice(), "rete", "{e:?}");
    assert!(e.to_string().contains("tempo scaduto"), "{e}");
    // Due tentativi da 300 ms (il server di prova serve una connessione per volta, quindi
    // il secondo resta in coda: conta il tempo, non le richieste ricevute).
    assert!(
        inizio.elapsed() >= Duration::from_millis(600),
        "{:?}",
        inizio.elapsed()
    );
    assert!(inizio.elapsed() < Duration::from_secs(2));
    assert!(ricevute.load(Ordering::SeqCst) >= 1);
}

fn embedding(mosse: Vec<Mossa>, testi: usize) -> (Result<Vec<Vec<f32>>, ErroreAi>, usize) {
    let (url, ricevute) = server(mosse);
    let c = ClienteEmbedding::new(
        ConfigEmbedding {
            url,
            dimensioni: Some(3),
            ..ConfigEmbedding::default()
        },
        Segreto::new(CHIAVE),
        Arc::new(TrasportoHttps::new().unwrap()),
        Arc::new(AttesaReale),
    )
    .unwrap()
    .con_politica(veloce());
    let testi: Vec<String> = (0..testi).map(|i| format!("testo {i}")).collect();
    let r = c.embed(&testi, &Annullamento::new());
    if let Err(e) = &r {
        assert!(!e.to_string().contains(CHIAVE), "{e}");
    }
    (r, ricevute.load(Ordering::SeqCst))
}

#[test]
fn embedding_con_dati_sbagliati() {
    let dato = |i: usize, v: Value| json!({"index": i, "embedding": v});
    // Buono, in ordine inverso: riordinato per indice.
    let (r, _) = embedding(
        vec![ok(
            json!({"data": [dato(1, json!([0, 1, 0])), dato(0, json!([1, 0, 0]))]}).to_string(),
        )],
        2,
    );
    assert_eq!(r.unwrap(), vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]]);
    for (corpo, cosa) in [
        (
            json!({"data": [dato(0, json!([1, 0, 0]))]}),
            "un vettore per due testi",
        ),
        (
            json!({"data": [dato(0, json!([1, 0])), dato(1, json!([1, 0, 0]))]}),
            "dimensioni sbagliate",
        ),
        (
            json!({"data": [dato(0, json!([1, 0, 0])), dato(0, json!([1, 0, 0]))]}),
            "indice ripetuto",
        ),
        (
            json!({"data": [dato(0, json!([1, 0, 0])), dato(7, json!([1, 0, 0]))]}),
            "indice fuori dai testi",
        ),
        (
            json!({"data": [dato(0, json!([1, null, 0])), dato(1, json!([1, 0, 0]))]}),
            "valore nullo",
        ),
        (
            json!({"data": [dato(0, json!([1, "x", 0])), dato(1, json!([1, 0, 0]))]}),
            "valore non numerico",
        ),
        (
            json!({"data": [dato(0, json!([1e39, 0, 0])), dato(1, json!([1, 0, 0]))]}),
            "valore fuori da f32",
        ),
        (json!({"data": "nessuno"}), "data non è un elenco"),
        (json!({"object": "list"}), "data assente"),
    ] {
        let (r, n) = embedding(vec![ok(corpo.to_string()), ok(corpo.to_string())], 2);
        let e = r.expect_err(cosa);
        assert_eq!(e.codice(), "risposta_non_valida", "{cosa}: {e:?}");
        assert_eq!(n, 1, "{cosa}: una risposta sbagliata non si ritenta");
    }
    // Quota esaurita: definitivo, con il messaggio del provider e senza la chiave.
    let (r, n) = embedding(
        vec![
            Mossa::Risposta(429, vec![], json!({"error": {"message": "You exceeded your current quota", "type": "insufficient_quota", "code": "insufficient_quota"}}).to_string()),
            ok("{}".into()),
        ],
        1,
    );
    let e = r.unwrap_err();
    assert_eq!(e.codice(), "quota");
    assert!(e.to_string().contains("exceeded your current quota"));
    assert_eq!(n, 1);
}
