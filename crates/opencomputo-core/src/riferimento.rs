//! Riferimenti alle entità del modello, usati per localizzare errori,
//! problemi di validazione e anomalie di importazione.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::classificazione::LivelloCategoria;
use crate::id::{
    AnalisiId, CapitoloId, ClasseId, ComputoId, PrezzarioId, RegistrazioneId, RigoMisuraId,
    RisorsaId, SalId, VoceComputoId, VocePrezzarioId, WbsNodoId,
};

/// Riferimento a un'entità del modello. I numeri d'ordine sono a base 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Riferimento {
    /// Il progetto nel suo complesso.
    Progetto,
    /// Regole di arrotondamento del progetto.
    Regole,
    /// Dati contrattuali.
    Contratto,
    /// Cronoprogramma dei lavori.
    Cronoprogramma,
    /// Prezzario o elenco prezzi.
    Prezzario {
        /// Identificativo.
        id: PrezzarioId,
    },
    /// Capitolo di prezzario.
    Capitolo {
        /// Identificativo.
        id: CapitoloId,
    },
    /// Voce di prezzario.
    VocePrezzario {
        /// Identificativo.
        id: VocePrezzarioId,
        /// Codice (tariffa).
        codice: String,
    },
    /// Risorsa elementare.
    Risorsa {
        /// Identificativo.
        id: RisorsaId,
        /// Codice.
        codice: String,
    },
    /// Analisi prezzi.
    Analisi {
        /// Identificativo.
        id: AnalisiId,
        /// Codice.
        codice: String,
    },
    /// Riga di un'analisi prezzi.
    RigaAnalisi {
        /// Numero d'ordine.
        numero: usize,
    },
    /// Classe di categoria.
    Classe {
        /// Livello.
        livello: LivelloCategoria,
        /// Identificativo.
        id: ClasseId,
    },
    /// Nodo della WBS.
    NodoWbs {
        /// Identificativo.
        id: WbsNodoId,
    },
    /// Computo.
    Computo {
        /// Identificativo.
        id: ComputoId,
    },
    /// Voce di computo.
    VoceComputo {
        /// Numero d'ordine nel computo.
        numero: usize,
        /// Identificativo.
        id: VoceComputoId,
    },
    /// Rigo di misurazione.
    Rigo {
        /// Numero d'ordine nella voce.
        numero: usize,
        /// Identificativo.
        id: RigoMisuraId,
    },
    /// Registrazione del libretto delle misure.
    Registrazione {
        /// Numero d'ordine nel libretto.
        numero: u32,
        /// Identificativo.
        id: RegistrazioneId,
    },
    /// Stato di avanzamento lavori.
    Sal {
        /// Numero del SAL.
        numero: u32,
        /// Identificativo.
        id: SalId,
    },
    /// Conto finale dei lavori.
    ContoFinale,
}

impl fmt::Display for Riferimento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Progetto => f.write_str("progetto"),
            Self::Regole => f.write_str("regole di arrotondamento"),
            Self::Contratto => f.write_str("dati contrattuali"),
            Self::Cronoprogramma => f.write_str("cronoprogramma"),
            Self::Prezzario { id } => write!(f, "prezzario {id}"),
            Self::Capitolo { id } => write!(f, "capitolo {id}"),
            Self::VocePrezzario { codice, .. } => write!(f, "voce di prezzario «{codice}»"),
            Self::Risorsa { codice, .. } => write!(f, "risorsa «{codice}»"),
            Self::Analisi { codice, .. } => write!(f, "analisi «{codice}»"),
            Self::RigaAnalisi { numero } => write!(f, "riga di analisi n. {numero}"),
            Self::Classe { livello, id } => write!(f, "{livello} {id}"),
            Self::NodoWbs { id } => write!(f, "nodo WBS {id}"),
            Self::Computo { id } => write!(f, "computo {id}"),
            Self::VoceComputo { numero, .. } => write!(f, "voce n. {numero}"),
            Self::Rigo { numero, .. } => write!(f, "rigo n. {numero}"),
            Self::Registrazione { numero, .. } => write!(f, "registrazione n. {numero}"),
            Self::Sal { numero, .. } => write!(f, "SAL n. {numero}"),
            Self::ContoFinale => f.write_str("conto finale"),
        }
    }
}
