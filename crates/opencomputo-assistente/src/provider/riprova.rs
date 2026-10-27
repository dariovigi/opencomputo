//! Classificazione delle risposte e nuovi tentativi con attesa crescente.
//!
//! * Si ritenta su errori di rete, tempo scaduto, 408, 409, 429, 500, 502, 503, 504 e
//!   529, rispettando `retry-after` (in secondi) se presente, altrimenti con attesa
//!   esponenziale (1 s, 2 s, 4 s… fino a 30 s) più una variazione casuale fino a metà
//!   dell'attesa.
//! * Non si ritenta su 400, 401, 402, 403, 404, 413, né sui 429 di OpenAI per credito o
//!   quota esauriti (`insufficient_quota`): ritentare non cambierebbe l'esito.
//! * Il numero di tentativi è limitato; l'annullamento si controlla prima di ogni
//!   tentativo e di ogni attesa.

use std::time::Duration;

use serde_json::Value;

use super::{
    Annullamento, Attesa, ErroreAi, ErroreTrasporto, Provider, RichiestaHttp, RispostaHttp,
    Trasporto, accorcia,
};

/// Regole dei nuovi tentativi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoliticaRiprova {
    /// Tentativi complessivi (il primo compreso).
    pub tentativi: u32,
    /// Prima attesa.
    pub attesa_iniziale: Duration,
    /// Attesa massima calcolata.
    pub attesa_massima: Duration,
    /// Attesa massima accettata da `retry-after`.
    pub retry_after_massimo: Duration,
}

impl Default for PoliticaRiprova {
    fn default() -> Self {
        Self {
            tentativi: 5,
            attesa_iniziale: Duration::from_secs(1),
            attesa_massima: Duration::from_secs(30),
            retry_after_massimo: Duration::from_secs(120),
        }
    }
}

/// Esito della lettura di una risposta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decisione {
    /// Risposta valida (2xx).
    Successo,
    /// Errore temporaneo: ritentare, eventualmente dopo l'attesa indicata dal server.
    Riprova {
        /// Attesa chiesta da `retry-after`.
        dopo: Option<Duration>,
        /// Errore da restituire se i tentativi finiscono.
        errore: ErroreAi,
    },
    /// Errore definitivo.
    Errore(ErroreAi),
}

/// Tipo, messaggio e codice dell'errore nel corpo della risposta (forme Anthropic e
/// OpenAI), con l'identificativo della richiesta.
fn dettagli(r: &RispostaHttp) -> (String, String, String, Option<String>) {
    let v: Value = serde_json::from_slice(&r.corpo).unwrap_or(Value::Null);
    let e = v.get("error").cloned().unwrap_or(Value::Null);
    let testo = |x: Option<&Value>| x.and_then(Value::as_str).unwrap_or("").to_owned();
    let tipo = testo(e.get("type"));
    let codice = testo(e.get("code"));
    let mut messaggio = testo(e.get("message"));
    if messaggio.is_empty() {
        messaggio = accorcia(String::from_utf8_lossy(&r.corpo).trim());
    }
    let id = v
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| r.intestazione("request-id").map(str::to_owned))
        .or_else(|| r.intestazione("x-request-id").map(str::to_owned));
    (tipo, accorcia(&messaggio), codice, id)
}

/// Secondi di `retry-after`, se è un numero.
fn retry_after(r: &RispostaHttp) -> Option<Duration> {
    let v = r.intestazione("retry-after")?.trim();
    v.parse::<u64>().ok().map(Duration::from_secs)
}

/// Classifica una risposta HTTP.
#[must_use]
pub fn classifica(provider: Provider, r: &RispostaHttp) -> Decisione {
    if (200..300).contains(&r.stato) {
        return Decisione::Successo;
    }
    let (tipo, messaggio, codice, id_richiesta) = dettagli(r);
    let http = || ErroreAi::Http {
        provider,
        stato: r.stato,
        tipo: tipo.clone(),
        messaggio: messaggio.clone(),
        id_richiesta: id_richiesta.clone(),
    };
    match r.stato {
        401 | 403 => Decisione::Errore(ErroreAi::ChiaveRifiutata {
            provider,
            stato: r.stato,
            messaggio: messaggio.clone(),
        }),
        402 => Decisione::Errore(ErroreAi::Quota {
            provider,
            messaggio: messaggio.clone(),
        }),
        429 if provider == Provider::Openai
            && (tipo == "insufficient_quota"
                || matches!(
                    codice.as_str(),
                    "insufficient_quota"
                        | "billing_hard_limit_reached"
                        | "credit_balance_exhausted"
                )) =>
        {
            Decisione::Errore(ErroreAi::Quota {
                provider,
                messaggio: messaggio.clone(),
            })
        }
        408 | 409 | 429 | 500 | 502 | 503 | 504 | 529 => Decisione::Riprova {
            dopo: retry_after(r),
            errore: http(),
        },
        _ => Decisione::Errore(http()),
    }
}

/// Attesa prima del tentativo `n` (da 1): esponenziale con variazione casuale.
fn attesa_calcolata(p: &PoliticaRiprova, n: u32, attesa: &dyn Attesa) -> Duration {
    let base = p
        .attesa_iniziale
        .saturating_mul(2u32.saturating_pow(n.saturating_sub(1)))
        .min(p.attesa_massima);
    let base_ms = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    base + Duration::from_millis(attesa.variazione_ms(base_ms / 2))
}

/// Invia la richiesta con i nuovi tentativi previsti dalla politica.
///
/// # Errors
/// Errore definitivo del provider, tentativi esauriti, annullamento.
pub fn esegui(
    provider: Provider,
    trasporto: &dyn Trasporto,
    attesa: &dyn Attesa,
    richiesta: &RichiestaHttp,
    politica: &PoliticaRiprova,
    annullamento: &Annullamento,
) -> Result<RispostaHttp, ErroreAi> {
    let mut ultimo: Option<ErroreAi> = None;
    for n in 1..=politica.tentativi.max(1) {
        if annullamento.annullato() {
            return Err(ErroreAi::Annullato);
        }
        let dopo = match trasporto.invia(richiesta) {
            Ok(r) => match classifica(provider, &r) {
                Decisione::Successo => return Ok(r),
                Decisione::Errore(e) => return Err(e),
                Decisione::Riprova { dopo, errore } => {
                    ultimo = Some(errore);
                    dopo
                }
            },
            Err(ErroreTrasporto::Altro(m)) => {
                return Err(ErroreAi::Rete {
                    provider,
                    messaggio: m,
                });
            }
            Err(e) => {
                ultimo = Some(ErroreAi::Rete {
                    provider,
                    messaggio: e.to_string(),
                });
                None
            }
        };
        if n == politica.tentativi {
            break;
        }
        let pausa = match dopo {
            Some(d) if d > politica.retry_after_massimo => break,
            Some(d) => d,
            None => attesa_calcolata(politica, n, attesa),
        };
        if annullamento.annullato() {
            return Err(ErroreAi::Annullato);
        }
        attesa.attendi(pausa);
    }
    Err(ultimo.unwrap_or(ErroreAi::Rete {
        provider,
        messaggio: "nessun tentativo eseguito".into(),
    }))
}

#[cfg(test)]
pub(crate) mod prova {
    //! Trasporto e attesa finti per i test.
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    /// Trasporto che restituisce risposte preparate, nell'ordine, e registra le richieste.
    #[derive(Debug, Default)]
    pub struct TrasportoFinto {
        pub risposte: Mutex<VecDeque<Result<RispostaHttp, ErroreTrasporto>>>,
        pub richieste: Mutex<Vec<RichiestaHttp>>,
    }

    impl TrasportoFinto {
        pub fn con(risposte: Vec<Result<RispostaHttp, ErroreTrasporto>>) -> Self {
            Self {
                risposte: Mutex::new(risposte.into()),
                richieste: Mutex::default(),
            }
        }

        pub fn inviate(&self) -> Vec<RichiestaHttp> {
            self.richieste.lock().map(|r| r.clone()).unwrap_or_default()
        }
    }

    impl Trasporto for TrasportoFinto {
        fn invia(&self, richiesta: &RichiestaHttp) -> Result<RispostaHttp, ErroreTrasporto> {
            if let Ok(mut r) = self.richieste.lock() {
                r.push(richiesta.clone());
            }
            self.risposte
                .lock()
                .ok()
                .and_then(|mut r| r.pop_front())
                .unwrap_or(Err(ErroreTrasporto::Altro(
                    "nessuna risposta preparata".into(),
                )))
        }
    }

    /// Attesa che non attende e registra le pause.
    #[derive(Debug, Default)]
    pub struct AttesaFinta {
        pub pause: Mutex<Vec<Duration>>,
    }

    impl AttesaFinta {
        pub fn registrate(&self) -> Vec<Duration> {
            self.pause.lock().map(|p| p.clone()).unwrap_or_default()
        }
    }

    impl Attesa for AttesaFinta {
        fn attendi(&self, durata: Duration) {
            if let Ok(mut p) = self.pause.lock() {
                p.push(durata);
            }
        }
        fn variazione_ms(&self, _massimo: u64) -> u64 {
            0
        }
    }

    pub fn risposta(stato: u16, corpo: &str, intestazioni: &[(&str, &str)]) -> RispostaHttp {
        RispostaHttp {
            stato,
            intestazioni: intestazioni
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            corpo: corpo.as_bytes().to_vec(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::prova::*;
    use super::*;

    fn richiesta() -> RichiestaHttp {
        RichiestaHttp {
            url: "https://esempio/v1".into(),
            intestazioni: vec![],
            corpo: b"{}".to_vec(),
            timeout: Duration::from_secs(5),
        }
    }

    fn esegui_con(
        provider: Provider,
        risposte: Vec<Result<RispostaHttp, ErroreTrasporto>>,
    ) -> (Result<RispostaHttp, ErroreAi>, usize, Vec<Duration>) {
        let t = TrasportoFinto::con(risposte);
        let a = AttesaFinta::default();
        let r = esegui(
            provider,
            &t,
            &a,
            &richiesta(),
            &PoliticaRiprova::default(),
            &Annullamento::new(),
        );
        (r, t.inviate().len(), a.registrate())
    }

    const SOVRACCARICO: &str = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"},"request_id":"req_1"}"#;

    #[test]
    fn retry_after_rispettato_poi_successo() {
        let (r, n, pause) = esegui_con(
            Provider::Anthropic,
            vec![
                Ok(risposta(
                    429,
                    r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow"}}"#,
                    &[("retry-after", "7")],
                )),
                Ok(risposta(200, "{}", &[])),
            ],
        );
        assert!(r.is_ok());
        assert_eq!(n, 2);
        assert_eq!(pause, [Duration::from_secs(7)]);
    }

    #[test]
    fn attesa_esponenziale_e_tentativi_finiti() {
        let (r, n, pause) = esegui_con(
            Provider::Anthropic,
            (0..6)
                .map(|_| Ok(risposta(529, SOVRACCARICO, &[])))
                .collect(),
        );
        assert_eq!(n, 5);
        assert_eq!(pause, [1, 2, 4, 8].map(Duration::from_secs));
        let Err(ErroreAi::Http {
            stato,
            tipo,
            id_richiesta,
            ..
        }) = r
        else {
            panic!("errore inatteso");
        };
        assert_eq!(
            (stato, tipo.as_str(), id_richiesta.as_deref()),
            (529, "overloaded_error", Some("req_1"))
        );
    }

    #[test]
    fn errori_definitivi_senza_nuovi_tentativi() {
        let casi = [
            (
                Provider::Anthropic,
                401,
                r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
                "chiave_rifiutata",
            ),
            (
                Provider::Anthropic,
                400,
                r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad"}}"#,
                "http",
            ),
            (
                Provider::Anthropic,
                402,
                r#"{"type":"error","error":{"type":"billing_error","message":"pay"}}"#,
                "quota",
            ),
            (
                Provider::Openai,
                429,
                r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#,
                "quota",
            ),
            (
                Provider::Openai,
                403,
                r#"{"error":{"message":"Country not supported","type":"invalid_request_error"}}"#,
                "chiave_rifiutata",
            ),
            (Provider::Openai, 404, "not json", "http"),
        ];
        for (p, stato, corpo, atteso) in casi {
            let (r, n, pause) = esegui_con(
                p,
                vec![
                    Ok(risposta(stato, corpo, &[])),
                    Ok(risposta(200, "{}", &[])),
                ],
            );
            assert_eq!(r.map_err(|e| e.codice()), Err(atteso), "{stato}");
            assert_eq!((n, pause.len()), (1, 0), "{stato}");
        }
    }

    #[test]
    fn errori_di_rete_ritentati() {
        let (r, n, _) = esegui_con(
            Provider::Openai,
            vec![
                Err(ErroreTrasporto::Timeout),
                Err(ErroreTrasporto::Connessione("reset".into())),
                Ok(risposta(
                    503,
                    r#"{"error":{"message":"overloaded","type":"server_error"}}"#,
                    &[],
                )),
                Ok(risposta(200, "{}", &[])),
            ],
        );
        assert!(r.is_ok());
        assert_eq!(n, 4);
        let (r, n, _) = esegui_con(
            Provider::Openai,
            vec![Err(ErroreTrasporto::Altro("tls".into()))],
        );
        assert_eq!(r.map_err(|e| e.codice()), Err("rete"));
        assert_eq!(n, 1);
    }

    #[test]
    fn retry_after_eccessivo_interrompe() {
        let (r, n, pause) = esegui_con(
            Provider::Anthropic,
            vec![
                Ok(risposta(429, "{}", &[("retry-after", "3600")])),
                Ok(risposta(200, "{}", &[])),
            ],
        );
        assert_eq!(r.map_err(|e| e.codice()), Err("http"));
        assert_eq!((n, pause.len()), (1, 0));
    }

    #[test]
    fn annullamento_prima_del_tentativo() {
        let t = TrasportoFinto::con(vec![Ok(risposta(200, "{}", &[]))]);
        let a = Annullamento::new();
        a.annulla();
        let r = esegui(
            Provider::Openai,
            &t,
            &AttesaFinta::default(),
            &richiesta(),
            &PoliticaRiprova::default(),
            &a,
        );
        assert_eq!(r, Err(ErroreAi::Annullato));
        assert!(t.inviate().is_empty());
    }
}
