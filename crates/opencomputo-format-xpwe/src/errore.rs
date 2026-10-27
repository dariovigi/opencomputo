//! Errori bloccanti di import ed export.
//!
//! Gli errori di questo modulo impediscono di produrre un risultato. Tutti i
//! problemi che permettono di proseguire sono invece **anomalie** riportate nel
//! rapporto ([`crate::RapportoImport`], [`crate::RisultatoExport`]).

use thiserror::Error;

/// Errore bloccante.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreXpwe {
    /// Il file non è XML ben formato.
    #[error("XML non valido alla riga {riga}, colonna {colonna}: {messaggio}")]
    XmlNonValido {
        /// Riga (a base 1).
        riga: usize,
        /// Colonna (a base 1, in caratteri).
        colonna: usize,
        /// Descrizione dell'errore.
        messaggio: String,
    },
    /// Il file è XML ma non ha la struttura di un documento XPWE.
    #[error("il documento non è un file XPWE: {0}")]
    NonXpwe(String),
    /// Annidamento oltre il limite di sicurezza.
    #[error("documento troppo annidato (oltre {0} livelli)")]
    TroppoAnnidato(usize),
    /// Numero di elementi oltre il limite di sicurezza.
    #[error("documento troppo grande (oltre {0} elementi)")]
    TroppiElementi(usize),
    /// Il computo richiesto per l'export non esiste.
    #[error("computo inesistente: {0}")]
    ComputoInesistente(String),
    /// Il computo da esportare non è calcolabile.
    #[error("il computo non è calcolabile: {0}")]
    Calcolo(String),
}
