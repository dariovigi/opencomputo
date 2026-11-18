//! Verbali di concordamento dei nuovi prezzi (Allegato II.14 al D.Lgs. 36/2023, art. 5,
//! commi 7–9).
//!
//! I nuovi prezzi sono voci dell'elenco prezzi con i dati di
//! [`NuovoPrezzo`](crate::prezzario::NuovoPrezzo): numero, criterio di formazione
//! (prezzario, analogia, analisi) e soggezione al ribasso. Il direttore dei lavori li
//! concorda con l'esecutore in un verbale, approvato dal RUP (o dalla stazione appaltante
//! se comportano maggiori spese). Se l'esecutore non firma, i prezzi possono comunque
//! essere ammessi in contabilità e l'esecutore li contesta iscrivendo riserva; il
//! programma registra firma, riserve e approvazione e ne segnala la mancanza.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use super::Contabilita;
use crate::errore::ErroreDominio;

/// Verbale di concordamento dei nuovi prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerbaleNuoviPrezzi {
    /// Numero.
    pub numero: u32,
    /// Data del verbale.
    pub data: Date,
    /// Data della firma dell'esecutore (assente: non firmato).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firma_esecutore: Option<Date>,
    /// Riserve dell'esecutore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub riserve: Option<String>,
    /// Data dell'approvazione (RUP o stazione appaltante).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvato_il: Option<Date>,
    /// Estremi dell'atto di approvazione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atto_approvazione: Option<String>,
    /// Note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl VerbaleNuoviPrezzi {
    /// `true` se approvato.
    #[must_use]
    pub fn approvato(&self) -> bool {
        self.approvato_il.is_some()
    }
}

impl Contabilita {
    /// Verbali di concordamento dei nuovi prezzi.
    #[must_use]
    pub fn verbali_nuovi_prezzi(&self) -> &[VerbaleNuoviPrezzi] {
        &self.verbali_nuovi_prezzi
    }

    /// Verbale per numero.
    #[must_use]
    pub fn verbale_nuovi_prezzi(&self, numero: u32) -> Option<&VerbaleNuoviPrezzi> {
        self.verbali_nuovi_prezzi
            .iter()
            .find(|v| v.numero == numero)
    }

    /// Prossimo numero di verbale.
    #[must_use]
    pub fn prossimo_numero_verbale(&self) -> u32 {
        self.verbali_nuovi_prezzi
            .iter()
            .map(|v| v.numero)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    /// Registra un verbale nuovo o sostituisce quello con lo stesso numero.
    ///
    /// # Errors
    /// Numero nullo, firma o approvazione anteriori al verbale.
    pub fn imposta_verbale_nuovi_prezzi(
        &mut self,
        verbale: VerbaleNuoviPrezzi,
    ) -> Result<(), ErroreDominio> {
        if verbale.numero == 0 {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "il numero del verbale deve essere almeno 1".into(),
            ));
        }
        for (cosa, data) in [
            ("la firma dell'esecutore", verbale.firma_esecutore),
            ("l'approvazione", verbale.approvato_il),
        ] {
            if let Some(d) = data.filter(|d| *d < verbale.data) {
                return Err(ErroreDominio::OperazioneNonConsentita(format!(
                    "{cosa} ({d}) precede il verbale ({})",
                    verbale.data
                )));
            }
        }
        match self
            .verbali_nuovi_prezzi
            .iter_mut()
            .find(|v| v.numero == verbale.numero)
        {
            Some(v) => *v = verbale,
            None => {
                self.verbali_nuovi_prezzi.push(verbale);
                self.verbali_nuovi_prezzi.sort_by_key(|v| v.numero);
            }
        }
        Ok(())
    }

    /// Rimuove un verbale non approvato.
    ///
    /// # Errors
    /// Verbale inesistente o già approvato.
    pub fn rimuovi_verbale_nuovi_prezzi(
        &mut self,
        numero: u32,
    ) -> Result<VerbaleNuoviPrezzi, ErroreDominio> {
        let posizione = self
            .verbali_nuovi_prezzi
            .iter()
            .position(|v| v.numero == numero)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "verbale nuovi prezzi",
                id: numero.to_string(),
            })?;
        if self
            .verbali_nuovi_prezzi
            .get(posizione)
            .is_some_and(VerbaleNuoviPrezzi::approvato)
        {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "il verbale n. {numero} è approvato: togliere prima l'approvazione"
            )));
        }
        Ok(self.verbali_nuovi_prezzi.remove(posizione))
    }
}
