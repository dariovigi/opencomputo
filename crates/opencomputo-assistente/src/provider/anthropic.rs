//! Generazione con l'API Messages di Anthropic (`POST /v1/messages`).
//!
//! La risposta deve rispettare uno schema JSON. Due modalità, scelte per modello:
//!
//! * [`ModalitaOutput::Json`]: `output_config.format` con `type: "json_schema"`
//!   (structured outputs, GA); la risposta è un blocco di testo con il JSON. È la
//!   modalità predefinita: la supportano tutti i modelli dell'elenco.
//! * [`ModalitaOutput::StrumentoForzato`]: uno strumento con `strict: true` e
//!   `tool_choice: {"type": "tool"}`; la risposta è l'`input` del blocco `tool_use`.
//!   Claude Opus 5.5, Sonnet 5.5, Fable 5.1 e Mythos 5.1 **non** accettano l'uso forzato
//!   di uno strumento (errore 400), quindi per loro questa modalità è rifiutata prima
//!   della chiamata.
//!
//! Fonti (verificate il 2026-10-27): documentazione Anthropic «Structured outputs»
//! (modelli supportati, limiti dello schema) e «Errors» (uso forzato di strumenti non
//! supportato). Un modello fuori dall'elenco si può usare solo indicando la modalità.
//!
//! `stop_reason: "refusal"` e `"max_tokens"` producono errori tipizzati: in quei casi la
//! risposta può non rispettare lo schema.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::riprova::{PoliticaRiprova, esegui};
use super::{Annullamento, Attesa, ErroreAi, Provider, RichiestaHttp, Segreto, Trasporto};

/// Indirizzo dell'API.
pub const URL_MESSAGGI: &str = "https://api.anthropic.com/v1/messages";
/// Versione dell'API.
pub const VERSIONE_API: &str = "2023-06-01";
/// Modello predefinito.
pub const MODELLO_PREDEFINITO: &str = "claude-sonnet-5-5";

/// Modelli con structured outputs secondo la documentazione (2026-10-27).
pub const MODELLI_CON_OUTPUT_STRUTTURATI: [&str; 17] = [
    "claude-fable-5-1",
    "claude-mythos-5-1",
    "claude-fable-5",
    "claude-mythos-5",
    "claude-mythos-preview",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-opus-4-8",
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-5-5",
    "claude-sonnet-5",
    "claude-sonnet-4-6",
    "claude-sonnet-4-5-20250929",
    "claude-opus-4-5-20251101",
    "claude-haiku-5-5",
    "claude-haiku-4-5-20251001",
];

/// Modelli che non accettano `tool_choice` di tipo `tool` o `any`.
pub const MODELLI_SENZA_STRUMENTO_FORZATO: [&str; 4] = [
    "claude-opus-5-5",
    "claude-sonnet-5-5",
    "claude-fable-5-1",
    "claude-mythos-5-1",
];

/// Modalità di output strutturato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalitaOutput {
    /// Scelta dall'elenco dei modelli noti: JSON per tutti.
    #[default]
    Automatica,
    /// `output_config.format`.
    Json,
    /// Strumento con `strict: true` e uso forzato.
    StrumentoForzato,
}

/// Modalità effettiva per un modello.
///
/// # Errors
/// Modello sconosciuto in modalità automatica; uso forzato chiesto a un modello che non
/// lo supporta.
pub fn modalita_per(modello: &str, scelta: ModalitaOutput) -> Result<ModalitaOutput, ErroreAi> {
    let noto = MODELLI_CON_OUTPUT_STRUTTURATI.contains(&modello);
    match scelta {
        ModalitaOutput::Automatica if noto => Ok(ModalitaOutput::Json),
        ModalitaOutput::Automatica => Err(ErroreAi::ModelloNonSupportato {
            modello: modello.into(),
            motivo: "modello non in elenco per gli output strutturati: indicare la modalità (json o strumento_forzato)".into(),
        }),
        ModalitaOutput::StrumentoForzato if MODELLI_SENZA_STRUMENTO_FORZATO.contains(&modello) => {
            Err(ErroreAi::ModelloNonSupportato {
                modello: modello.into(),
                motivo: "il modello non accetta l'uso forzato di uno strumento: usare la modalità json".into(),
            })
        }
        altra => Ok(altra),
    }
}

/// Configurazione del client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigGenerazione {
    /// Modello.
    pub modello: String,
    /// Modalità di output.
    pub modalita: ModalitaOutput,
    /// Indirizzo dell'API.
    pub url: String,
    /// Tempo massimo per richiesta.
    pub timeout: Duration,
}

impl Default for ConfigGenerazione {
    fn default() -> Self {
        Self {
            modello: MODELLO_PREDEFINITO.into(),
            modalita: ModalitaOutput::Automatica,
            url: URL_MESSAGGI.into(),
            timeout: Duration::from_secs(300),
        }
    }
}

/// Una chiamata con risposta vincolata a uno schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Chiamata {
    /// Istruzioni di sistema.
    pub sistema: String,
    /// Messaggio dell'utente.
    pub utente: String,
    /// Schema JSON della risposta.
    pub schema: Value,
    /// Nome dello strumento (modalità a strumento) e della chiamata nei messaggi.
    pub nome: String,
    /// Descrizione dello strumento.
    pub descrizione: String,
    /// Limite di token della risposta (ragionamento compreso).
    pub max_tokens: u32,
}

/// Risposta del modello.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RispostaModello {
    /// JSON conforme allo schema.
    pub json: Value,
    /// Token in ingresso.
    pub token_ingresso: u64,
    /// Token in uscita.
    pub token_uscita: u64,
    /// Identificativo della richiesta.
    pub id_richiesta: Option<String>,
}

/// Client della generazione.
pub struct ClienteGenerazione {
    config: ConfigGenerazione,
    modalita: ModalitaOutput,
    chiave: Segreto,
    trasporto: Arc<dyn Trasporto>,
    attesa: Arc<dyn Attesa>,
    politica: PoliticaRiprova,
}

impl std::fmt::Debug for ClienteGenerazione {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClienteGenerazione")
            .field("config", &self.config)
            .field("modalita", &self.modalita)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Messaggio {
    #[serde(default)]
    content: Vec<Value>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<Uso>,
}

#[derive(Deserialize)]
struct Uso {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl ClienteGenerazione {
    /// Nuovo client.
    ///
    /// # Errors
    /// Chiave vuota; modalità non adatta al modello.
    pub fn new(
        config: ConfigGenerazione,
        chiave: Segreto,
        trasporto: Arc<dyn Trasporto>,
        attesa: Arc<dyn Attesa>,
    ) -> Result<Self, ErroreAi> {
        if chiave.vuoto() {
            return Err(ErroreAi::ChiaveAssente {
                provider: Provider::Anthropic,
            });
        }
        let modalita = modalita_per(&config.modello, config.modalita)?;
        Ok(Self {
            config,
            modalita,
            chiave,
            trasporto,
            attesa,
            politica: PoliticaRiprova::default(),
        })
    }

    /// Imposta la politica dei nuovi tentativi.
    #[must_use]
    pub fn con_politica(mut self, politica: PoliticaRiprova) -> Self {
        self.politica = politica;
        self
    }

    /// Modalità effettiva.
    #[must_use]
    pub fn modalita(&self) -> ModalitaOutput {
        self.modalita
    }

    /// Corpo della richiesta.
    #[must_use]
    pub fn corpo(&self, c: &Chiamata) -> Value {
        let mut corpo = serde_json::Map::new();
        corpo.insert("model".into(), json!(self.config.modello));
        corpo.insert("max_tokens".into(), json!(c.max_tokens));
        corpo.insert("system".into(), json!(c.sistema));
        corpo.insert(
            "messages".into(),
            json!([{"role": "user", "content": c.utente}]),
        );
        match self.modalita {
            ModalitaOutput::StrumentoForzato => {
                corpo.insert(
                    "tools".into(),
                    json!([{
                        "name": c.nome,
                        "description": c.descrizione,
                        "strict": true,
                        "input_schema": c.schema,
                    }]),
                );
                corpo.insert(
                    "tool_choice".into(),
                    json!({"type": "tool", "name": c.nome}),
                );
            }
            ModalitaOutput::Json | ModalitaOutput::Automatica => {
                corpo.insert(
                    "output_config".into(),
                    json!({"format": {"type": "json_schema", "schema": c.schema}}),
                );
            }
        }
        Value::Object(corpo)
    }

    fn richiesta(&self, c: &Chiamata) -> RichiestaHttp {
        RichiestaHttp {
            url: self.config.url.clone(),
            intestazioni: vec![
                ("x-api-key".into(), self.chiave.esponi().to_owned()),
                ("anthropic-version".into(), VERSIONE_API.into()),
                ("content-type".into(), "application/json".into()),
            ],
            corpo: self.corpo(c).to_string().into_bytes(),
            timeout: self.config.timeout,
        }
    }

    /// JSON della risposta, secondo la modalità.
    fn estrai(&self, c: &Chiamata, m: &Messaggio) -> Result<Value, ErroreAi> {
        let non_valida = |motivo: String| ErroreAi::RispostaNonValida {
            provider: Provider::Anthropic,
            motivo,
        };
        let tipo = |b: &Value| {
            b.get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        match self.modalita {
            ModalitaOutput::StrumentoForzato => m
                .content
                .iter()
                .find(|b| {
                    tipo(b) == "tool_use"
                        && b.get("name").and_then(Value::as_str) == Some(c.nome.as_str())
                })
                .and_then(|b| b.get("input").cloned())
                .ok_or_else(|| non_valida(format!("nessuna chiamata allo strumento «{}»", c.nome))),
            ModalitaOutput::Json | ModalitaOutput::Automatica => {
                let testo: String = m
                    .content
                    .iter()
                    .filter(|b| tipo(b) == "text")
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect();
                if testo.trim().is_empty() {
                    return Err(non_valida("nessun blocco di testo con il JSON".into()));
                }
                serde_json::from_str(&testo)
                    .map_err(|e| non_valida(format!("JSON non leggibile: {e}")))
            }
        }
    }
}

impl super::Generazione for ClienteGenerazione {
    fn genera(
        &self,
        c: &Chiamata,
        annullamento: &Annullamento,
    ) -> Result<RispostaModello, ErroreAi> {
        let r = esegui(
            Provider::Anthropic,
            self.trasporto.as_ref(),
            self.attesa.as_ref(),
            &self.richiesta(c),
            &self.politica,
            annullamento,
        )?;
        let m: Messaggio =
            serde_json::from_slice(&r.corpo).map_err(|e| ErroreAi::RispostaNonValida {
                provider: Provider::Anthropic,
                motivo: e.to_string(),
            })?;
        match m.stop_reason.as_deref() {
            Some("refusal") => return Err(ErroreAi::Rifiuto),
            Some("max_tokens") => {
                return Err(ErroreAi::Troncata {
                    max_tokens: c.max_tokens,
                });
            }
            _ => {}
        }
        let json = self.estrai(c, &m)?;
        Ok(RispostaModello {
            json,
            token_ingresso: m.usage.as_ref().map_or(0, |u| u.input_tokens),
            token_uscita: m.usage.as_ref().map_or(0, |u| u.output_tokens),
            id_richiesta: r.intestazione("request-id").map(str::to_owned),
        })
    }
}

#[cfg(test)]
mod test {
    use super::super::Generazione;
    use super::super::riprova::prova::*;
    use super::*;

    fn chiamata() -> Chiamata {
        Chiamata {
            sistema: "sistema".into(),
            utente: "utente".into(),
            schema: json!({"type": "object", "additionalProperties": false, "required": ["a"], "properties": {"a": {"type": "string"}}}),
            nome: "registra".into(),
            descrizione: "registra il risultato".into(),
            max_tokens: 1000,
        }
    }

    fn cliente(
        t: Arc<TrasportoFinto>,
        modello: &str,
        modalita: ModalitaOutput,
    ) -> Result<ClienteGenerazione, ErroreAi> {
        ClienteGenerazione::new(
            ConfigGenerazione {
                modello: modello.into(),
                modalita,
                ..ConfigGenerazione::default()
            },
            Segreto::new("sk-ant-prova"),
            t,
            Arc::new(AttesaFinta::default()),
        )
    }

    #[test]
    fn modalita_per_modello() {
        assert_eq!(
            modalita_per("claude-sonnet-5-5", ModalitaOutput::Automatica),
            Ok(ModalitaOutput::Json)
        );
        assert_eq!(
            modalita_per("claude-haiku-5-5", ModalitaOutput::StrumentoForzato),
            Ok(ModalitaOutput::StrumentoForzato)
        );
        assert_eq!(
            modalita_per("claude-sonnet-5-5", ModalitaOutput::StrumentoForzato)
                .map_err(|e| e.codice()),
            Err("modello_non_supportato")
        );
        assert_eq!(
            modalita_per("claude-futuro", ModalitaOutput::Automatica).map_err(|e| e.codice()),
            Err("modello_non_supportato")
        );
        assert_eq!(
            modalita_per("claude-futuro", ModalitaOutput::Json),
            Ok(ModalitaOutput::Json)
        );
    }

    #[test]
    fn richiesta_e_risposta_in_modalita_json() {
        let corpo = json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-sonnet-5-5",
            "content": [{"type": "thinking", "thinking": "…", "signature": "x"}, {"type": "text", "text": "{\"a\": \"ok\"}"}],
            "stop_reason": "end_turn", "usage": {"input_tokens": 120, "output_tokens": 30}
        });
        let t = Arc::new(TrasportoFinto::con(vec![Ok(risposta(
            200,
            &corpo.to_string(),
            &[("request-id", "req_9")],
        ))]));
        let c = cliente(t.clone(), "claude-sonnet-5-5", ModalitaOutput::Automatica).unwrap();
        let r = c.genera(&chiamata(), &Annullamento::new()).unwrap();
        assert_eq!(r.json, json!({"a": "ok"}));
        assert_eq!(
            (r.token_ingresso, r.token_uscita, r.id_richiesta.as_deref()),
            (120, 30, Some("req_9"))
        );
        let inviata = t.inviate().into_iter().next().unwrap();
        assert!(
            inviata
                .intestazioni
                .contains(&("anthropic-version".into(), "2023-06-01".into()))
        );
        assert!(
            inviata
                .intestazioni
                .contains(&("x-api-key".into(), "sk-ant-prova".into()))
        );
        let v: Value = serde_json::from_slice(&inviata.corpo).unwrap();
        assert_eq!(v["output_config"]["format"]["type"], "json_schema");
        assert_eq!(v["output_config"]["format"]["schema"], chiamata().schema);
        assert!(v.get("tools").is_none() && v.get("tool_choice").is_none());
        assert_eq!(
            v["messages"],
            json!([{"role": "user", "content": "utente"}])
        );
    }

    #[test]
    fn richiesta_e_risposta_con_strumento_forzato() {
        let corpo = json!({
            "content": [{"type": "tool_use", "id": "toolu_1", "name": "registra", "input": {"a": "ok"}}],
            "stop_reason": "tool_use", "usage": {"input_tokens": 1, "output_tokens": 1}
        });
        let t = Arc::new(TrasportoFinto::con(vec![Ok(risposta(
            200,
            &corpo.to_string(),
            &[],
        ))]));
        let c = cliente(
            t.clone(),
            "claude-haiku-5-5",
            ModalitaOutput::StrumentoForzato,
        )
        .unwrap();
        assert_eq!(
            c.genera(&chiamata(), &Annullamento::new()).unwrap().json,
            json!({"a": "ok"})
        );
        let v: Value = serde_json::from_slice(&t.inviate().first().unwrap().corpo).unwrap();
        assert_eq!(v["tools"][0]["strict"], true);
        assert_eq!(
            v["tool_choice"],
            json!({"type": "tool", "name": "registra"})
        );
        assert!(v.get("output_config").is_none());
    }

    #[test]
    fn rifiuto_troncamento_e_risposte_vuote() {
        let casi = [
            (
                json!({"content": [{"type": "text", "text": "Non posso"}], "stop_reason": "refusal"}),
                "rifiuto",
            ),
            (
                json!({"content": [{"type": "text", "text": "{\"a\": "}], "stop_reason": "max_tokens"}),
                "troncata",
            ),
            (
                json!({"content": [], "stop_reason": "end_turn"}),
                "risposta_non_valida",
            ),
            (
                json!({"content": [{"type": "text", "text": "non json"}], "stop_reason": "end_turn"}),
                "risposta_non_valida",
            ),
        ];
        for (corpo, atteso) in casi {
            let t = Arc::new(TrasportoFinto::con(vec![Ok(risposta(
                200,
                &corpo.to_string(),
                &[],
            ))]));
            let c = cliente(t, "claude-sonnet-5-5", ModalitaOutput::Json).unwrap();
            assert_eq!(
                c.genera(&chiamata(), &Annullamento::new())
                    .map_err(|e| e.codice()),
                Err(atteso)
            );
        }
    }

    #[test]
    fn strumento_mancante_nella_risposta() {
        let corpo = json!({"content": [{"type": "text", "text": "{}"}], "stop_reason": "end_turn"});
        let t = Arc::new(TrasportoFinto::con(vec![Ok(risposta(
            200,
            &corpo.to_string(),
            &[],
        ))]));
        let c = cliente(t, "claude-haiku-5-5", ModalitaOutput::StrumentoForzato).unwrap();
        assert_eq!(
            c.genera(&chiamata(), &Annullamento::new())
                .map_err(|e| e.codice()),
            Err("risposta_non_valida")
        );
    }
}
