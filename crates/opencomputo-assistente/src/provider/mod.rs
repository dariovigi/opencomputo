//! Provider AI: OpenAI per gli embedding, Anthropic per la generazione (ADR 0025).
//!
//! Questo modulo non apre connessioni: costruisce le richieste HTTP, interpreta le
//! risposte e decide i nuovi tentativi. La rete passa da un [`Trasporto`] fornito dal
//! chiamante (il crate `opencomputo-rete` per l'uso reale, un trasporto finto nei test),
//! così ogni comportamento si prova senza chiamate reali.
//!
//! **I dati inviati ai provider escono dal computer dell'utente** (vedi
//! [`crate::consenso`]).

pub mod anthropic;
pub mod chiavi;
pub mod openai;
pub mod riprova;

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// OpenAI (embedding).
    Openai,
    /// Anthropic (generazione).
    Anthropic,
}

impl Provider {
    /// Nome per i messaggi.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic",
        }
    }
}

/// Segreto (chiave API): non compare in `Debug` né nei messaggi d'errore.
#[derive(Clone, PartialEq, Eq)]
pub struct Segreto(String);

impl Segreto {
    /// Nuovo segreto; spazi iniziali e finali tolti.
    #[must_use]
    pub fn new(valore: &str) -> Self {
        Self(valore.trim().to_owned())
    }

    /// Valore, da usare solo per l'intestazione della richiesta.
    #[must_use]
    pub fn esponi(&self) -> &str {
        &self.0
    }

    /// `true` se vuoto.
    #[must_use]
    pub fn vuoto(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Segreto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segreto(***)")
    }
}

/// Richiesta HTTP POST con corpo JSON.
#[derive(Clone, PartialEq, Eq)]
pub struct RichiestaHttp {
    /// URL completo.
    pub url: String,
    /// Intestazioni (nome, valore). Possono contenere la chiave: mai registrarle.
    pub intestazioni: Vec<(String, String)>,
    /// Corpo JSON.
    pub corpo: Vec<u8>,
    /// Tempo massimo per la richiesta completa.
    pub timeout: Duration,
}

impl fmt::Debug for RichiestaHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RichiestaHttp")
            .field("url", &self.url)
            .field("intestazioni", &"[omesse]")
            .field("byte", &self.corpo.len())
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Risposta HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RispostaHttp {
    /// Codice di stato.
    pub stato: u16,
    /// Intestazioni (nomi in minuscolo).
    pub intestazioni: Vec<(String, String)>,
    /// Corpo.
    pub corpo: Vec<u8>,
}

impl RispostaHttp {
    /// Valore di un'intestazione (nome senza distinzione di maiuscole).
    #[must_use]
    pub fn intestazione(&self, nome: &str) -> Option<&str> {
        self.intestazioni
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, v)| v.as_str())
    }
}

/// Errore del trasporto, prima di qualsiasi risposta HTTP.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreTrasporto {
    /// Tempo scaduto.
    #[error("tempo scaduto")]
    Timeout,
    /// Connessione non riuscita o interrotta.
    #[error("connessione non riuscita: {0}")]
    Connessione(String),
    /// Altro errore (TLS, richiesta non valida…).
    #[error("{0}")]
    Altro(String),
}

/// Trasporto HTTP.
pub trait Trasporto: Send + Sync {
    /// Invia una richiesta POST e restituisce la risposta, qualunque sia lo stato.
    ///
    /// # Errors
    /// Errore di rete prima di una risposta.
    fn invia(&self, richiesta: &RichiestaHttp) -> Result<RispostaHttp, ErroreTrasporto>;
}

/// Attesa tra un tentativo e il successivo (sostituibile nei test).
pub trait Attesa: Send + Sync {
    /// Attende la durata indicata.
    fn attendi(&self, durata: Duration);
    /// Variazione casuale da 0 a `massimo` millisecondi.
    fn variazione_ms(&self, massimo: u64) -> u64;
}

/// Segnale di annullamento delle operazioni lunghe, controllato tra un passo e l'altro.
#[derive(Debug, Clone, Default)]
pub struct Annullamento(Arc<AtomicBool>);

impl Annullamento {
    /// Nuovo segnale, non annullato.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Chiede l'annullamento.
    pub fn annulla(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// `true` se è stato chiesto l'annullamento.
    #[must_use]
    pub fn annullato(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Calcolo degli embedding (OpenAI o un sostituto nei test).
pub trait Embedding: Send + Sync {
    /// Spazio dei vettori prodotti: modello e dimensioni.
    fn spazio(&self) -> crate::vettori::SpazioEmbedding;
    /// Dimensione massima dei lotti.
    fn limiti(&self) -> openai::LimitiLotto;
    /// Un vettore per testo, nello stesso ordine.
    ///
    /// # Errors
    /// Errore del provider o annullamento.
    fn embed(
        &self,
        testi: &[String],
        annullamento: &Annullamento,
    ) -> Result<Vec<Vec<f32>>, ErroreAi>;
}

/// Generazione con risposta vincolata a uno schema (Anthropic o un sostituto nei test).
pub trait Generazione: Send + Sync {
    /// Esegue la chiamata e restituisce il JSON della risposta.
    ///
    /// # Errors
    /// Errore del provider, rifiuto, risposta troncata o annullamento.
    fn genera(
        &self,
        chiamata: &anthropic::Chiamata,
        annullamento: &Annullamento,
    ) -> Result<anthropic::RispostaModello, ErroreAi>;
}

/// Errore di una chiamata a un provider.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "codice", rename_all = "snake_case")]
pub enum ErroreAi {
    /// Chiave non impostata.
    #[error("chiave {} non impostata", provider.nome())]
    ChiaveAssente {
        /// Provider.
        provider: Provider,
    },
    /// Chiave rifiutata (401, 403).
    #[error("chiave {} rifiutata ({stato}): {messaggio}", provider.nome())]
    ChiaveRifiutata {
        /// Provider.
        provider: Provider,
        /// Stato HTTP.
        stato: u16,
        /// Messaggio del provider.
        messaggio: String,
    },
    /// Credito, quota o limite di spesa esauriti: ritentare non serve.
    #[error("{}: credito o limite di spesa esaurito: {messaggio}", provider.nome())]
    Quota {
        /// Provider.
        provider: Provider,
        /// Messaggio del provider.
        messaggio: String,
    },
    /// Errore HTTP che non si risolve ritentando, o che persiste dopo i tentativi.
    #[error("{} ha risposto {stato} ({tipo}): {messaggio}", provider.nome())]
    Http {
        /// Provider.
        provider: Provider,
        /// Stato HTTP.
        stato: u16,
        /// Tipo di errore dichiarato dal provider.
        tipo: String,
        /// Messaggio del provider.
        messaggio: String,
        /// Identificativo della richiesta, per l'assistenza.
        id_richiesta: Option<String>,
    },
    /// Rete non disponibile o tempo scaduto, anche dopo i tentativi.
    #[error("{}: {messaggio}", provider.nome())]
    Rete {
        /// Provider.
        provider: Provider,
        /// Messaggio.
        messaggio: String,
    },
    /// Risposta che non rispetta il formato atteso.
    #[error("{}: risposta non valida: {motivo}", provider.nome())]
    RispostaNonValida {
        /// Provider.
        provider: Provider,
        /// Motivo.
        motivo: String,
    },
    /// Il modello ha rifiutato la richiesta (`stop_reason: refusal`).
    #[error("il modello ha rifiutato la richiesta")]
    Rifiuto,
    /// Risposta interrotta dal limite di token (`stop_reason: max_tokens`).
    #[error("risposta interrotta dal limite di {max_tokens} token: aumentare il limite")]
    Troncata {
        /// Limite usato.
        max_tokens: u32,
    },
    /// Modello non adatto (per esempio senza output strutturati).
    #[error("modello «{modello}» non utilizzabile: {motivo}")]
    ModelloNonSupportato {
        /// Modello.
        modello: String,
        /// Motivo.
        motivo: String,
    },
    /// Operazione annullata dall'utente.
    #[error("operazione annullata")]
    Annullato,
}

impl ErroreAi {
    /// Codice stabile per le risposte JSON.
    #[must_use]
    pub fn codice(&self) -> &'static str {
        match self {
            Self::ChiaveAssente { .. } => "chiave_assente",
            Self::ChiaveRifiutata { .. } => "chiave_rifiutata",
            Self::Quota { .. } => "quota",
            Self::Http { .. } => "http",
            Self::Rete { .. } => "rete",
            Self::RispostaNonValida { .. } => "risposta_non_valida",
            Self::Rifiuto => "rifiuto",
            Self::Troncata { .. } => "troncata",
            Self::ModelloNonSupportato { .. } => "modello_non_supportato",
            Self::Annullato => "annullato",
        }
    }
}

/// Testo di un errore restituito da un provider, accorciato.
pub(crate) fn accorcia(testo: &str) -> String {
    const MASSIMO: usize = 300;
    if testo.chars().count() <= MASSIMO {
        return testo.to_owned();
    }
    let mut t: String = testo.chars().take(MASSIMO).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn la_chiave_non_compare_nei_debug() {
        let s = Segreto::new("  sk-segretissima  ");
        assert_eq!(s.esponi(), "sk-segretissima");
        assert!(!format!("{s:?}").contains("segretissima"));
        let r = RichiestaHttp {
            url: "https://esempio".into(),
            intestazioni: vec![("x-api-key".into(), s.esponi().into())],
            corpo: Vec::new(),
            timeout: Duration::from_secs(1),
        };
        assert!(!format!("{r:?}").contains("segretissima"));
    }

    #[test]
    fn annullamento_condiviso() {
        let a = Annullamento::new();
        let b = a.clone();
        assert!(!b.annullato());
        a.annulla();
        assert!(b.annullato());
    }
}
