//! Embedding OpenAI (`POST /v1/embeddings`).
//!
//! Limiti dell'API rispettati dai lotti di [`LimitiLotto`]: al massimo 2048 testi e
//! 300 000 token per richiesta, 8192 token per testo. I lotti predefiniti restano ben
//! sotto: 256 testi, 200 000 caratteri in tutto, 8000 caratteri per testo.
//!
//! La risposta è controllata: un vettore per testo, nell'ordine di `index`, tutti della
//! dimensione attesa e con valori finiti.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use super::riprova::{PoliticaRiprova, esegui};
use super::{Annullamento, Attesa, ErroreAi, Provider, RichiestaHttp, Segreto, Trasporto};
use crate::vettori::SpazioEmbedding;

/// Indirizzo dell'API.
pub const URL_EMBEDDING: &str = "https://api.openai.com/v1/embeddings";
/// Modello predefinito.
pub const MODELLO_PREDEFINITO: &str = "text-embedding-3-small";

/// Dimensioni predefinite dei modelli noti.
#[must_use]
pub fn dimensioni_predefinite(modello: &str) -> Option<u32> {
    match modello {
        "text-embedding-3-small" | "text-embedding-ada-002" => Some(1536),
        "text-embedding-3-large" => Some(3072),
        _ => None,
    }
}

/// `true` se il modello accetta il parametro `dimensions`.
#[must_use]
pub fn accetta_dimensioni(modello: &str) -> bool {
    modello.starts_with("text-embedding-3")
}

/// Dimensione dei lotti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitiLotto {
    /// Testi per richiesta.
    pub testi: usize,
    /// Caratteri complessivi per richiesta.
    pub caratteri: usize,
    /// Caratteri per testo (i testi più lunghi vanno accorciati prima).
    pub caratteri_per_testo: usize,
}

impl Default for LimitiLotto {
    fn default() -> Self {
        Self {
            testi: 256,
            caratteri: 200_000,
            caratteri_per_testo: 8_000,
        }
    }
}

/// Configurazione del client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEmbedding {
    /// Modello.
    pub modello: String,
    /// Dimensioni chieste (solo modelli `text-embedding-3`); assenti: quelle del modello.
    pub dimensioni: Option<u32>,
    /// Indirizzo dell'API.
    pub url: String,
    /// Tempo massimo per richiesta.
    pub timeout: Duration,
}

impl Default for ConfigEmbedding {
    fn default() -> Self {
        Self {
            modello: MODELLO_PREDEFINITO.into(),
            dimensioni: None,
            url: URL_EMBEDDING.into(),
            timeout: Duration::from_secs(60),
        }
    }
}

/// Client degli embedding.
pub struct ClienteEmbedding {
    config: ConfigEmbedding,
    spazio: SpazioEmbedding,
    chiave: Segreto,
    trasporto: Arc<dyn Trasporto>,
    attesa: Arc<dyn Attesa>,
    politica: PoliticaRiprova,
}

impl std::fmt::Debug for ClienteEmbedding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClienteEmbedding")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Risposta {
    data: Vec<Elemento>,
}

#[derive(Deserialize)]
struct Elemento {
    index: usize,
    embedding: Vec<f32>,
}

impl ClienteEmbedding {
    /// Nuovo client.
    ///
    /// # Errors
    /// Chiave vuota; modello sconosciuto senza dimensioni indicate; dimensioni chieste a
    /// un modello che non le accetta.
    pub fn new(
        config: ConfigEmbedding,
        chiave: Segreto,
        trasporto: Arc<dyn Trasporto>,
        attesa: Arc<dyn Attesa>,
    ) -> Result<Self, ErroreAi> {
        if chiave.vuoto() {
            return Err(ErroreAi::ChiaveAssente {
                provider: Provider::Openai,
            });
        }
        if config.dimensioni.is_some() && !accetta_dimensioni(&config.modello) {
            return Err(ErroreAi::ModelloNonSupportato {
                modello: config.modello.clone(),
                motivo: "il modello non accetta un numero di dimensioni diverso dal suo".into(),
            });
        }
        let dimensioni = config
            .dimensioni
            .or_else(|| dimensioni_predefinite(&config.modello))
            .ok_or_else(|| ErroreAi::ModelloNonSupportato {
                modello: config.modello.clone(),
                motivo: "modello non noto: indicare le dimensioni dei vettori".into(),
            })?;
        let spazio = SpazioEmbedding::new(&config.modello, dimensioni);
        Ok(Self {
            config,
            spazio,
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

    /// Richiesta HTTP per i testi.
    fn richiesta(&self, testi: &[String]) -> RichiestaHttp {
        let mut corpo = json!({
            "model": self.config.modello,
            "input": testi,
            "encoding_format": "float",
        });
        if let (Some(d), Some(o)) = (self.config.dimensioni, corpo.as_object_mut()) {
            o.insert("dimensions".into(), json!(d));
        }
        RichiestaHttp {
            url: self.config.url.clone(),
            intestazioni: vec![
                (
                    "authorization".into(),
                    format!("Bearer {}", self.chiave.esponi()),
                ),
                ("content-type".into(), "application/json".into()),
            ],
            corpo: corpo.to_string().into_bytes(),
            timeout: self.config.timeout,
        }
    }
}

impl super::Embedding for ClienteEmbedding {
    fn spazio(&self) -> SpazioEmbedding {
        self.spazio.clone()
    }

    fn limiti(&self) -> LimitiLotto {
        LimitiLotto::default()
    }

    fn embed(
        &self,
        testi: &[String],
        annullamento: &Annullamento,
    ) -> Result<Vec<Vec<f32>>, ErroreAi> {
        if testi.is_empty() {
            return Ok(Vec::new());
        }
        let non_valida = |motivo: String| ErroreAi::RispostaNonValida {
            provider: Provider::Openai,
            motivo,
        };
        if let Some(t) = testi.iter().find(|t| t.trim().is_empty()) {
            return Err(non_valida(format!("testo vuoto da inviare ({t:?})")));
        }
        let r = esegui(
            Provider::Openai,
            self.trasporto.as_ref(),
            self.attesa.as_ref(),
            &self.richiesta(testi),
            &self.politica,
            annullamento,
        )?;
        let risposta: Risposta =
            serde_json::from_slice(&r.corpo).map_err(|e| non_valida(e.to_string()))?;
        if risposta.data.len() != testi.len() {
            return Err(non_valida(format!(
                "{} vettori per {} testi",
                risposta.data.len(),
                testi.len()
            )));
        }
        let attese = self.spazio.dimensioni as usize;
        let mut vettori: Vec<Option<Vec<f32>>> = vec![None; testi.len()];
        for e in risposta.data {
            if e.embedding.len() != attese {
                return Err(non_valida(format!(
                    "vettore di {} dimensioni invece di {attese}",
                    e.embedding.len()
                )));
            }
            if !e.embedding.iter().all(|x| x.is_finite()) {
                return Err(non_valida("valori non finiti nel vettore".into()));
            }
            match vettori.get_mut(e.index) {
                Some(posto @ None) => *posto = Some(e.embedding),
                _ => {
                    return Err(non_valida(format!(
                        "indice {} fuori posto o ripetuto",
                        e.index
                    )));
                }
            }
        }
        vettori
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| non_valida("vettori mancanti".into()))
    }
}

#[cfg(test)]
mod test {
    use super::super::Embedding;
    use super::super::riprova::prova::*;
    use super::*;
    use serde_json::Value;

    fn cliente(t: Arc<TrasportoFinto>, dimensioni: Option<u32>) -> ClienteEmbedding {
        ClienteEmbedding::new(
            ConfigEmbedding {
                dimensioni,
                ..ConfigEmbedding::default()
            },
            Segreto::new("sk-prova"),
            t,
            Arc::new(AttesaFinta::default()),
        )
        .unwrap()
    }

    fn corpo(vettori: &[(usize, Vec<f32>)]) -> String {
        let data: Vec<Value> = vettori
            .iter()
            .map(|(i, v)| json!({"object": "embedding", "index": i, "embedding": v}))
            .collect();
        json!({"object": "list", "data": data, "model": "text-embedding-3-small", "usage": {"prompt_tokens": 3, "total_tokens": 3}}).to_string()
    }

    #[test]
    fn richiesta_e_ordine_dei_vettori() {
        let t = Arc::new(TrasportoFinto::con(vec![Ok(risposta(
            200,
            &corpo(&[(1, vec![0.0, 1.0, 0.0, 0.0]), (0, vec![1.0, 0.0, 0.0, 0.0])]),
            &[],
        ))]));
        let c = cliente(t.clone(), Some(4));
        let v = c
            .embed(&["a".into(), "b".into()], &Annullamento::new())
            .unwrap();
        assert_eq!(v, vec![vec![1.0, 0.0, 0.0, 0.0], vec![0.0, 1.0, 0.0, 0.0]]);
        let inviate = t.inviate();
        let r = inviate.first().unwrap();
        assert_eq!(r.url, URL_EMBEDDING);
        assert!(
            r.intestazioni
                .contains(&("authorization".into(), "Bearer sk-prova".into()))
        );
        let corpo: Value = serde_json::from_slice(&r.corpo).unwrap();
        assert_eq!(
            corpo,
            json!({"model": "text-embedding-3-small", "input": ["a", "b"], "encoding_format": "float", "dimensions": 4})
        );
        assert_eq!(
            c.spazio(),
            SpazioEmbedding::new("text-embedding-3-small", 4)
        );
    }

    #[test]
    fn risposte_non_valide() {
        let casi = [
            corpo(&[(0, vec![1.0, 0.0, 0.0, 0.0])]), // un vettore per due testi
            corpo(&[(0, vec![1.0, 0.0]), (1, vec![1.0, 0.0])]), // dimensioni sbagliate
            corpo(&[(0, vec![1.0, 0.0, 0.0, 0.0]), (0, vec![1.0, 0.0, 0.0, 0.0])]), // indice ripetuto
            "{\"data\": 3}".to_owned(),
        ];
        for c in casi {
            let t = Arc::new(TrasportoFinto::con(vec![Ok(risposta(200, &c, &[]))]));
            let e = cliente(t, Some(4)).embed(&["a".into(), "b".into()], &Annullamento::new());
            assert_eq!(e.map_err(|e| e.codice()), Err("risposta_non_valida"), "{c}");
        }
    }

    #[test]
    fn configurazioni_rifiutate() {
        let t: Arc<dyn Trasporto> = Arc::new(TrasportoFinto::default());
        let a: Arc<dyn Attesa> = Arc::new(AttesaFinta::default());
        let nuovo = |modello: &str, dimensioni, chiave: &str| {
            ClienteEmbedding::new(
                ConfigEmbedding {
                    modello: modello.into(),
                    dimensioni,
                    ..ConfigEmbedding::default()
                },
                Segreto::new(chiave),
                t.clone(),
                a.clone(),
            )
            .map(|_| ())
            .map_err(|e| e.codice())
        };
        assert_eq!(
            nuovo("text-embedding-3-small", None, ""),
            Err("chiave_assente")
        );
        assert_eq!(
            nuovo("modello-ignoto", None, "k"),
            Err("modello_non_supportato")
        );
        assert_eq!(
            nuovo("text-embedding-ada-002", Some(256), "k"),
            Err("modello_non_supportato")
        );
        assert_eq!(
            nuovo("modello-ignoto", Some(768), "k"),
            Err("modello_non_supportato")
        );
        assert_eq!(nuovo("text-embedding-3-large", None, "k"), Ok(()));
    }
}
