//! Trasporto reale contro un server HTTP locale (127.0.0.1): nessuna rete esterna.
//! Portachiavi sull'archivio finto di `keyring-core`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use opencomputo_assistente::provider::anthropic::{
    Chiamata, ClienteGenerazione, ConfigGenerazione, ModalitaOutput,
};
use opencomputo_assistente::provider::chiavi::ArchivioChiavi;
use opencomputo_assistente::provider::riprova::PoliticaRiprova;
use opencomputo_assistente::provider::{
    Annullamento, ErroreTrasporto, Generazione, Provider, RichiestaHttp, Segreto, Trasporto,
};
use opencomputo_rete::{AttesaReale, PortachiaviSistema, TrasportoHttps};
use serde_json::json;

/// Risposta preparata: stato, intestazioni, corpo, ritardo prima di rispondere.
struct Preparata {
    stato: u16,
    intestazioni: Vec<(&'static str, String)>,
    corpo: String,
    ritardo: Duration,
}

fn ok(corpo: String) -> Preparata {
    Preparata {
        stato: 200,
        intestazioni: vec![],
        corpo,
        ritardo: Duration::ZERO,
    }
}

/// Richiesta ricevuta: intestazioni (minuscole) e corpo.
type Ricevuta = (Vec<(String, String)>, Vec<u8>);

/// Server che risponde, una connessione per volta, con le risposte preparate.
fn server(risposte: Vec<Preparata>) -> (String, Arc<Mutex<Vec<Ricevuta>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/messages", l.local_addr().unwrap());
    let ricevute = Arc::new(Mutex::new(Vec::new()));
    let r2 = ricevute.clone();
    thread::spawn(move || {
        for p in risposte {
            let Ok((s, _)) = l.accept() else { return };
            gestisci(s, &p, &r2);
        }
    });
    (url, ricevute)
}

fn gestisci(s: TcpStream, p: &Preparata, ricevute: &Mutex<Vec<Ricevuta>>) {
    let mut lettore = BufReader::new(s.try_clone().unwrap());
    let mut intestazioni = Vec::new();
    let mut lunghezza = 0usize;
    loop {
        let mut riga = String::new();
        if lettore.read_line(&mut riga).unwrap_or(0) == 0 || riga == "\r\n" {
            break;
        }
        if let Some((n, v)) = riga.trim_end().split_once(':') {
            let (n, v) = (n.trim().to_lowercase(), v.trim().to_owned());
            if n == "content-length" {
                lunghezza = v.parse().unwrap_or(0);
            }
            intestazioni.push((n, v));
        }
    }
    let mut corpo = vec![0u8; lunghezza];
    lettore.read_exact(&mut corpo).unwrap();
    ricevute.lock().unwrap().push((intestazioni, corpo));
    thread::sleep(p.ritardo);
    let mut out = s;
    let mut testa = format!(
        "HTTP/1.1 {} X\r\ncontent-length: {}\r\nconnection: close\r\n",
        p.stato,
        p.corpo.len()
    );
    for (n, v) in &p.intestazioni {
        testa.push_str(&format!("{n}: {v}\r\n"));
    }
    testa.push_str("\r\n");
    let _ = out.write_all(testa.as_bytes());
    let _ = out.write_all(p.corpo.as_bytes());
}

fn richiesta(url: &str, timeout: Duration) -> RichiestaHttp {
    RichiestaHttp {
        url: url.into(),
        intestazioni: vec![
            ("x-api-key".into(), "sk-segreta".into()),
            ("content-type".into(), "application/json".into()),
        ],
        corpo: br#"{"a":1}"#.to_vec(),
        timeout,
    }
}

#[test]
fn invio_e_intestazioni() {
    let (url, ricevute) = server(vec![Preparata {
        stato: 201,
        intestazioni: vec![("request-id", "req_7".into())],
        corpo: "{\"ok\":true}".into(),
        ritardo: Duration::ZERO,
    }]);
    let t = TrasportoHttps::new().unwrap();
    let r = t.invia(&richiesta(&url, Duration::from_secs(5))).unwrap();
    assert_eq!(r.stato, 201);
    assert_eq!(r.intestazione("Request-Id"), Some("req_7"));
    assert_eq!(r.corpo, b"{\"ok\":true}");
    let ricevute = ricevute.lock().unwrap();
    let (intestazioni, corpo) = &ricevute[0];
    assert!(intestazioni.contains(&("x-api-key".into(), "sk-segreta".into())));
    assert!(
        intestazioni
            .iter()
            .any(|(n, v)| n == "user-agent" && v.starts_with("OpenComputo/"))
    );
    assert_eq!(corpo, br#"{"a":1}"#);
}

#[test]
fn tempo_scaduto_e_connessione_rifiutata() {
    let (url, _) = server(vec![Preparata {
        ritardo: Duration::from_secs(3),
        ..ok("{}".into())
    }]);
    let t = TrasportoHttps::new().unwrap();
    let inizio = Instant::now();
    let e = t
        .invia(&richiesta(&url, Duration::from_millis(300)))
        .unwrap_err();
    assert_eq!(e, ErroreTrasporto::Timeout);
    assert!(inizio.elapsed() < Duration::from_secs(2));

    // Porta chiusa: nessuno in ascolto.
    let porta = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let e = t
        .invia(&richiesta(
            &format!("http://127.0.0.1:{porta}/"),
            Duration::from_secs(2),
        ))
        .unwrap_err();
    assert!(matches!(e, ErroreTrasporto::Connessione(_)), "{e:?}");
    assert!(!format!("{e:?}").contains("sk-segreta"));
}

#[test]
fn generazione_con_sovraccarico_retry_after_e_successo() {
    let messaggio = json!({
        "content": [{"type": "text", "text": "{\"esito\": \"ok\"}"}],
        "stop_reason": "end_turn", "usage": {"input_tokens": 10, "output_tokens": 5}
    })
    .to_string();
    let (url, ricevute) = server(vec![
        Preparata {
            stato: 529,
            intestazioni: vec![],
            corpo: r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#
                .into(),
            ritardo: Duration::ZERO,
        },
        Preparata {
            stato: 429,
            intestazioni: vec![("retry-after", "1".into())],
            corpo: r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#
                .into(),
            ritardo: Duration::ZERO,
        },
        ok(messaggio),
    ]);
    let c = ClienteGenerazione::new(
        ConfigGenerazione {
            url,
            modalita: ModalitaOutput::Automatica,
            ..ConfigGenerazione::default()
        },
        Segreto::new("sk-ant-locale"),
        Arc::new(TrasportoHttps::new().unwrap()),
        Arc::new(AttesaReale),
    )
    .unwrap()
    .con_politica(PoliticaRiprova {
        attesa_iniziale: Duration::from_millis(20),
        ..PoliticaRiprova::default()
    });
    let inizio = Instant::now();
    let r = c
        .genera(
            &Chiamata {
                sistema: "s".into(),
                utente: "u".into(),
                schema: json!({"type": "object", "additionalProperties": false, "required": ["esito"], "properties": {"esito": {"type": "string"}}}),
                nome: "n".into(),
                descrizione: "d".into(),
                max_tokens: 100,
            },
            &Annullamento::new(),
        )
        .unwrap();
    assert_eq!(r.json, json!({"esito": "ok"}));
    assert!(
        inizio.elapsed() >= Duration::from_secs(1),
        "retry-after rispettato"
    );
    let ricevute = ricevute.lock().unwrap();
    assert_eq!(ricevute.len(), 3);
    for (intestazioni, _) in ricevute.iter() {
        assert!(intestazioni.contains(&("anthropic-version".into(), "2023-06-01".into())));
    }
}

#[test]
fn chiave_rifiutata_senza_nuovi_tentativi() {
    let (url, ricevute) = server(vec![
        Preparata { stato: 401, intestazioni: vec![], corpo: r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.into(), ritardo: Duration::ZERO },
        ok("{}".into()),
    ]);
    let c = ClienteGenerazione::new(
        ConfigGenerazione {
            url,
            ..ConfigGenerazione::default()
        },
        Segreto::new("sk-ant-sbagliata"),
        Arc::new(TrasportoHttps::new().unwrap()),
        Arc::new(AttesaReale),
    )
    .unwrap();
    let chiamata = Chiamata {
        sistema: String::new(),
        utente: "u".into(),
        schema: json!({}),
        nome: "n".into(),
        descrizione: String::new(),
        max_tokens: 10,
    };
    let e = c.genera(&chiamata, &Annullamento::new()).unwrap_err();
    assert_eq!(e.codice(), "chiave_rifiutata");
    assert!(!e.to_string().contains("sk-ant-sbagliata"));
    assert_eq!(ricevute.lock().unwrap().len(), 1);
}

#[test]
fn portachiavi_su_archivio_finto() {
    let store: Arc<keyring_core::CredentialStore> = keyring_core::mock::Store::new().unwrap();
    let p = PortachiaviSistema::con_archivio(&store, "prova").unwrap();
    assert_eq!(p.leggi(Provider::Openai), Ok(None));
    p.scrivi(Provider::Openai, &Segreto::new(" sk-1 ")).unwrap();
    assert_eq!(
        p.leggi(Provider::Openai)
            .unwrap()
            .map(|s| s.esponi().to_owned()),
        Some("sk-1".into())
    );
    assert_eq!(p.leggi(Provider::Anthropic), Ok(None));
    p.elimina(Provider::Openai).unwrap();
    p.elimina(Provider::Openai).unwrap();
    assert_eq!(p.leggi(Provider::Openai), Ok(None));
    p.scrivi(Provider::Anthropic, &Segreto::new("sk-2"))
        .unwrap();
    p.scrivi(Provider::Anthropic, &Segreto::new("   ")).unwrap(); // vuota: elimina
    assert_eq!(p.leggi(Provider::Anthropic), Ok(None));
    assert!(!format!("{p:?}").contains("sk-"));
}
