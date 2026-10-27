//! Archivio delle chiavi API.
//!
//! In uso reale le chiavi stanno nel portachiavi del sistema operativo
//! (`opencomputo-rete`); qui c'è solo il tratto e un archivio in memoria per i test e
//! per i sistemi senza portachiavi (la chiave vale fino alla chiusura del programma).
//! Le chiavi non vengono mai scritte nei file di OpenComputo.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;
use thiserror::Error;

use super::{Provider, Segreto};

/// Errore dell'archivio delle chiavi.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "codice", rename_all = "snake_case")]
pub enum ErroreChiavi {
    /// Portachiavi non disponibile o accesso negato.
    #[error("portachiavi non disponibile: {messaggio}")]
    NonDisponibile {
        /// Messaggio del sistema.
        messaggio: String,
    },
    /// Altro errore.
    #[error("errore del portachiavi: {messaggio}")]
    Altro {
        /// Messaggio del sistema.
        messaggio: String,
    },
}

/// Archivio delle chiavi.
pub trait ArchivioChiavi: Send + Sync {
    /// Chiave del provider, se impostata.
    ///
    /// # Errors
    /// Portachiavi non leggibile.
    fn leggi(&self, provider: Provider) -> Result<Option<Segreto>, ErroreChiavi>;
    /// Imposta la chiave.
    ///
    /// # Errors
    /// Portachiavi non scrivibile.
    fn scrivi(&self, provider: Provider, chiave: &Segreto) -> Result<(), ErroreChiavi>;
    /// Elimina la chiave (nessun errore se non c'era).
    ///
    /// # Errors
    /// Portachiavi non scrivibile.
    fn elimina(&self, provider: Provider) -> Result<(), ErroreChiavi>;
    /// Descrizione dell'archivio per i messaggi (es. «Portachiavi di macOS»).
    fn descrizione(&self) -> String;
}

/// Archivio in memoria: la chiave vale fino alla chiusura del programma.
#[derive(Debug, Default)]
pub struct ChiaviInMemoria(Mutex<BTreeMap<Provider, Segreto>>);

impl ArchivioChiavi for ChiaviInMemoria {
    fn leggi(&self, provider: Provider) -> Result<Option<Segreto>, ErroreChiavi> {
        Ok(self.0.lock().ok().and_then(|m| m.get(&provider).cloned()))
    }

    fn scrivi(&self, provider: Provider, chiave: &Segreto) -> Result<(), ErroreChiavi> {
        if let Ok(mut m) = self.0.lock() {
            m.insert(provider, chiave.clone());
        }
        Ok(())
    }

    fn elimina(&self, provider: Provider) -> Result<(), ErroreChiavi> {
        if let Ok(mut m) = self.0.lock() {
            m.remove(&provider);
        }
        Ok(())
    }

    fn descrizione(&self) -> String {
        "memoria (fino alla chiusura del programma)".into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn archivio_in_memoria() {
        let a = ChiaviInMemoria::default();
        assert_eq!(a.leggi(Provider::Openai), Ok(None));
        a.scrivi(Provider::Openai, &Segreto::new("k1")).unwrap();
        assert_eq!(
            a.leggi(Provider::Openai)
                .unwrap()
                .map(|s| s.esponi().to_owned()),
            Some("k1".into())
        );
        assert_eq!(a.leggi(Provider::Anthropic), Ok(None));
        a.elimina(Provider::Openai).unwrap();
        a.elimina(Provider::Openai).unwrap();
        assert_eq!(a.leggi(Provider::Openai), Ok(None));
    }
}
