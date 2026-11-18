//! Registro di contabilità.
//!
//! Il registro è una vista **derivata** dal libretto delle misure e dai SAL:
//! riporta, SAL per SAL e nell'ordine del libretto, ogni registrazione
//! contabilizzata con quantità, prezzo e importo, insieme ai totali del
//! periodo e progressivi. Le registrazioni non ancora attribuite a un SAL non
//! compaiono nel registro.
//!
//! Limite noto: le righe sono una per registrazione; il raggruppamento per
//! articolo di elenco prezzi tipico di alcuni modelli di registro è demandato
//! al livello di reportistica.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::id::{RegistrazioneId, SalId};
use crate::numeri::{Importo, Prezzo, Quantita};
use crate::prezzario::NaturaVoce;

/// Riga del registro di contabilità.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigaRegistro {
    /// Numero progressivo della riga nel registro.
    pub numero: u32,
    /// SAL in cui la registrazione è contabilizzata.
    pub sal: SalId,
    /// Numero del SAL.
    pub numero_sal: u32,
    /// Registrazione del libretto.
    pub registrazione: RegistrazioneId,
    /// Numero della registrazione nel libretto.
    pub numero_libretto: u32,
    /// Data della misurazione.
    pub data: Date,
    /// Codice dell'articolo.
    pub codice: String,
    /// Descrizione breve.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Natura.
    pub natura: NaturaVoce,
    /// Quantità.
    pub quantita: Quantita,
    /// Prezzo unitario contrattuale.
    pub prezzo: Prezzo,
    /// Importo lordo.
    pub importo: Importo,
}

/// Totali del registro per SAL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotaleSalRegistro {
    /// SAL.
    pub sal: SalId,
    /// Numero del SAL.
    pub numero_sal: u32,
    /// Importo lordo del periodo.
    pub importo_periodo: Importo,
    /// Importo lordo progressivo.
    pub importo_progressivo: Importo,
}

/// Registro di contabilità.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistroContabilita {
    /// Righe.
    pub righe: Vec<RigaRegistro>,
    /// Totali per SAL.
    pub totali: Vec<TotaleSalRegistro>,
}
