//! Errori bloccanti della lettura DCF.

use thiserror::Error;

/// Errore bloccante: il file non produce alcun risultato.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreDcf {
    /// Il file non ha la firma di un contenitore DCF.
    #[error("il file non è un DCF: {0}")]
    NonDcf(String),
    /// Il contenitore è incompleto o incoerente.
    #[error("contenitore DCF danneggiato: {0}")]
    Danneggiato(String),
    /// Il contenuto supera i limiti di sicurezza (protezione da file «bomba»).
    #[error("contenuto DCF oltre i limiti di sicurezza: {0}")]
    OltreLimiti(String),
    /// I dati del documento sono protetti: OpenComputo non li decifra.
    #[error(
        "il documento DCF è protetto ({flussi} blocchi cifrati): OpenComputo non legge i \
         contenuti protetti. Esportare il documento in XPWE da PriMus e importare il file XPWE"
    )]
    Protetto {
        /// Numero di flussi protetti.
        flussi: usize,
    },
    /// Il documento richiede una password di apertura: OpenComputo non la aggira.
    #[error(
        "il documento DCF è protetto da password di apertura: OpenComputo non lo legge. \
         Rimuovere la password in PriMus oppure esportare in XPWE"
    )]
    ProtettoDaPassword,
    /// Nessun flusso leggibile contiene dati importabili.
    #[error("nessun dato importabile nel DCF: {0}")]
    NessunDato(String),
}
