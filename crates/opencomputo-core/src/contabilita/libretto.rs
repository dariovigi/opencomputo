//! Libretto delle misure.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::classificazione::AssegnazioneCategorie;
use crate::errore::ErroreLocalizzato;
use crate::id::{RegistrazioneId, VoceComputoId, WbsNodoId};
use crate::misura::RigoMisura;
use crate::numeri::RegoleArrotondamento;
use crate::partita::{CalcoloPartita, PrezzoApplicato, calcola_partita};
use crate::riferimento::Riferimento;

/// Registrazione del libretto delle misure: misurazione di una lavorazione
/// eseguita, con data, prezzo contrattuale applicato e righi di misura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrazioneLibretto {
    /// Identificativo.
    pub id: RegistrazioneId,
    /// Numero d'ordine nel libretto (≥ 1, univoco).
    pub numero: u32,
    /// Data della misurazione.
    pub data: NaiveDate,
    /// Prezzo contrattuale applicato (fotografia).
    pub prezzo: PrezzoApplicato,
    /// Righi di misura.
    #[serde(default)]
    pub righi: Vec<RigoMisura>,
    /// Categorie.
    #[serde(default)]
    pub categorie: AssegnazioneCategorie,
    /// Nodo WBS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wbs: Option<WbsNodoId>,
    /// Voce del computo di progetto a cui la misurazione si riferisce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voce_computo: Option<VoceComputoId>,
    /// Annotazioni.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotazioni: Option<String>,
}

impl RegistrazioneLibretto {
    /// Nuova registrazione senza righi.
    #[must_use]
    pub fn new(id: RegistrazioneId, numero: u32, data: NaiveDate, prezzo: PrezzoApplicato) -> Self {
        Self {
            id,
            numero,
            data,
            prezzo,
            righi: Vec::new(),
            categorie: AssegnazioneCategorie::default(),
            wbs: None,
            voce_computo: None,
            annotazioni: None,
        }
    }

    /// Riferimento alla registrazione.
    #[must_use]
    pub fn riferimento(&self) -> Riferimento {
        Riferimento::Registrazione {
            numero: self.numero,
            id: self.id,
        }
    }

    /// Calcola la registrazione.
    ///
    /// # Errors
    /// Errore localizzato sul rigo responsabile.
    pub fn calcola(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<CalcoloPartita, ErroreLocalizzato> {
        calcola_partita(&self.prezzo, &self.righi, regole).map_err(|e| e.dentro(self.riferimento()))
    }
}

/// Libretto delle misure.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibrettoMisure {
    /// Registrazioni, nell'ordine di inserimento.
    #[serde(default)]
    pub registrazioni: Vec<RegistrazioneLibretto>,
}

impl LibrettoMisure {
    /// Registrazione per identificativo.
    #[must_use]
    pub fn registrazione(&self, id: RegistrazioneId) -> Option<&RegistrazioneLibretto> {
        self.registrazioni.iter().find(|r| r.id == id)
    }

    /// Prossimo numero d'ordine disponibile.
    #[must_use]
    pub fn prossimo_numero(&self) -> u32 {
        self.registrazioni
            .iter()
            .map(|r| r.numero)
            .max()
            .map_or(1, |n| n.saturating_add(1))
    }
}
