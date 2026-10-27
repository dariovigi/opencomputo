//! Errori che impediscono di leggere un listino nel suo complesso.
//!
//! I problemi delle singole righe non sono errori: diventano anomalie nel rapporto.

use thiserror::Error;

/// Errore di lettura di un listino.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreListino {
    /// File vuoto o senza righe di dati.
    #[error("il listino è vuoto")]
    Vuoto,
    /// File oltre la dimensione ammessa.
    #[error("il listino supera la dimensione massima di {massimo} byte")]
    TroppoGrande {
        /// Dimensione massima, in byte.
        massimo: usize,
    },
    /// Troppe righe.
    #[error("il listino supera il numero massimo di {massimo} righe")]
    TroppeRighe {
        /// Numero massimo di righe.
        massimo: usize,
    },
    /// CSV non leggibile (virgolette non chiuse, campo troppo lungo…).
    #[error("CSV non valido alla riga {riga}: {motivo}")]
    Csv {
        /// Riga (da 1).
        riga: usize,
        /// Motivo.
        motivo: String,
    },
    /// Mancano colonne obbligatorie nell'intestazione.
    #[error(
        "colonne obbligatorie mancanti: {}; intestazione trovata: {}",
        mancanti.join(", "),
        trovate.join(" | ")
    )]
    ColonneMancanti {
        /// Colonne mancanti.
        mancanti: Vec<String>,
        /// Intestazione letta.
        trovate: Vec<String>,
    },
    /// La stessa colonna compare due volte nell'intestazione.
    #[error("la colonna «{colonna}» compare più volte nell'intestazione")]
    ColonnaAmbigua {
        /// Colonna.
        colonna: String,
    },
    /// JSON non valido o con una struttura diversa da quella attesa.
    #[error("JSON non valido: {motivo}")]
    Json {
        /// Motivo.
        motivo: String,
    },
    /// Nessuna voce valida: tutte le righe sono state scartate.
    #[error("nessuna voce valida nel listino ({scartate} righe scartate; prima anomalia: {prima})")]
    NessunaVoceValida {
        /// Righe scartate.
        scartate: usize,
        /// Messaggio della prima anomalia.
        prima: String,
    },
}
