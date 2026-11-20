//! Tipi di errore del dominio.
//!
//! Gli errori sono suddivisi in tre livelli:
//!
//! * [`ErroreCalcolo`]: errore numerico elementare (overflow, divisione per zero,
//!   formula non valida, regola di arrotondamento non valida…), privo di contesto;
//! * [`ErroreLocalizzato`]: un [`ErroreCalcolo`] accompagnato dal percorso
//!   dell'entità che lo ha generato (es. «computo › voce n. 3 › rigo n. 2»);
//! * [`ErroreDominio`]: errore delle operazioni sul modello (identificativi
//!   duplicati, riferimenti non validi, operazioni non consentite), che può
//!   incapsulare un errore di calcolo localizzato.

use core::fmt;

use crate::formula::ErroreFormula;
use crate::riferimento::Riferimento;

/// Errore numerico elementare, privo di contesto.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroreCalcolo {
    /// Il risultato eccede la capacità del tipo decimale (circa 28 cifre significative).
    #[error("superamento della capacità numerica (overflow)")]
    Overflow,
    /// Quantità o importo oltre [`crate::numeri::VALORE_MASSIMO`] in valore assoluto.
    #[error("valore oltre il limite gestito (un milione di miliardi)")]
    FuoriLimite,
    /// Divisione per zero.
    #[error("divisione per zero")]
    DivisionePerZero,
    /// Errore di sintassi o di valutazione di una formula.
    #[error("formula non valida: {0}")]
    Formula(#[from] ErroreFormula),
    /// Regola di arrotondamento non ammessa.
    #[error("regola di arrotondamento non valida: {0}")]
    RegolaNonValida(String),
    /// Dato di ingresso non valido per il calcolo richiesto.
    #[error("valore non valido: {0}")]
    ValoreNonValido(String),
    /// Riferimento «vedi voce» non risolvibile.
    #[error("«vedi voce»: {0}")]
    VediVoce(String),
}

/// Errore di calcolo con il percorso dell'entità che lo ha generato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroreLocalizzato {
    /// Percorso dall'entità più esterna a quella più interna.
    pub percorso: Vec<Riferimento>,
    /// Errore elementare.
    pub errore: ErroreCalcolo,
}

impl std::error::Error for ErroreLocalizzato {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.errore)
    }
}

impl ErroreLocalizzato {
    /// Crea un errore localizzato su una singola entità.
    #[must_use]
    pub fn new(riferimento: Riferimento, errore: ErroreCalcolo) -> Self {
        Self {
            percorso: vec![riferimento],
            errore,
        }
    }

    /// Antepone un'entità contenitore al percorso.
    #[must_use]
    pub fn dentro(mut self, contenitore: Riferimento) -> Self {
        self.percorso.insert(0, contenitore);
        self
    }
}

impl fmt::Display for ErroreLocalizzato {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, r) in self.percorso.iter().enumerate() {
            if i > 0 {
                f.write_str(" › ")?;
            }
            write!(f, "{r}")?;
        }
        if !self.percorso.is_empty() {
            f.write_str(": ")?;
        }
        write!(f, "{}", self.errore)
    }
}

/// Errore delle operazioni sul modello di dominio.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroreDominio {
    /// Un'entità con lo stesso identificativo è già presente.
    #[error("{entita} con identificativo {id} già presente")]
    IdDuplicato {
        /// Tipo di entità.
        entita: &'static str,
        /// Identificativo duplicato.
        id: String,
    },
    /// Un'entità con lo stesso codice è già presente nello stesso ambito.
    #[error("{entita} con codice «{codice}» già presente")]
    CodiceDuplicato {
        /// Tipo di entità.
        entita: &'static str,
        /// Codice duplicato.
        codice: String,
    },
    /// L'entità richiesta non esiste.
    #[error("{entita} {id} non trovato")]
    NonTrovato {
        /// Tipo di entità.
        entita: &'static str,
        /// Identificativo cercato.
        id: String,
    },
    /// Un riferimento tra entità è incoerente (padre inesistente, ciclo…).
    #[error("riferimento non valido: {0}")]
    RiferimentoNonValido(String),
    /// L'operazione viola una regola del dominio (es. modifica di un SAL emesso).
    #[error("operazione non consentita: {0}")]
    OperazioneNonConsentita(String),
    /// Errore di calcolo.
    #[error(transparent)]
    Calcolo(#[from] ErroreLocalizzato),
}
