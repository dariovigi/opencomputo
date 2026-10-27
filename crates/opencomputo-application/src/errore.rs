//! Errori dei casi d'uso, serializzabili per le risposte JSON.

use opencomputo_core::validazione::Problema;
use serde::Serialize;
use thiserror::Error;

/// Errore di un caso d'uso. In ogni caso il progetto della sessione resta invariato.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "codice", rename_all = "snake_case")]
pub enum ErroreApplicazione {
    /// Richiesta senza comandi.
    #[error("nessun comando da eseguire")]
    NessunComando,
    /// Un comando non è applicabile.
    #[error("comando n. {} non applicabile: {messaggio}", indice + 1)]
    Comando {
        /// Indice del comando (da 0).
        indice: usize,
        /// Motivo.
        messaggio: String,
    },
    /// I comandi introdurrebbero errori di validazione.
    #[error("i comandi introdurrebbero {} errori di validazione: {}", problemi.len(), problemi.first().map_or(String::new(), |p| p.messaggio.clone()))]
    NuoviErrori {
        /// Errori che comparirebbero.
        problemi: Vec<Problema>,
    },
    /// Nessun passo da annullare.
    #[error("nessuna operazione da annullare")]
    NienteDaAnnullare,
    /// Nessun passo da ripristinare.
    #[error("nessuna operazione da ripristinare")]
    NienteDaRipristinare,
    /// Errore di lettura o scrittura del file di progetto.
    #[error("{messaggio}")]
    File {
        /// Messaggio.
        messaggio: String,
    },
    /// Errore di importazione.
    #[error("importazione non riuscita: {messaggio}")]
    Importazione {
        /// Messaggio.
        messaggio: String,
    },
    /// Errore di esportazione o di report.
    #[error("esportazione non riuscita: {messaggio}")]
    Esportazione {
        /// Messaggio.
        messaggio: String,
    },
    /// Richiesta non valida (riferimento inesistente, parametro errato).
    #[error("{messaggio}")]
    Richiesta {
        /// Messaggio.
        messaggio: String,
    },
    /// Errore dell'assistente AI (ADR 0025): consenso mancante, chiave assente, errore
    /// di un provider, risposta del modello inutilizzabile.
    #[error("{messaggio}")]
    Assistente {
        /// Codice stabile (`consenso_mancante`, `chiave_assente`, `rete`, …).
        motivo: String,
        /// Messaggio.
        messaggio: String,
    },
}

impl From<opencomputo_storage::ErroreStorage> for ErroreApplicazione {
    fn from(e: opencomputo_storage::ErroreStorage) -> Self {
        Self::File {
            messaggio: e.to_string(),
        }
    }
}
