//! Consenso all'invio dei dati ai provider AI.
//!
//! Prima di qualsiasi chiamata l'utente deve accettare il testo di questa versione. Se il
//! testo cambia (nuovi dati inviati, nuovi provider) cambia la versione e il consenso va
//! dato di nuovo.

use serde::{Deserialize, Serialize};

/// Versione corrente del testo.
pub const VERSIONE: u32 = 1;

/// Testo da mostrare prima del consenso.
pub const TESTO: &str = "L'assistente AI invia dati fuori dal tuo computer.

• A OpenAI (calcolo degli embedding): il testo delle voci del prezzario caricato (codice, descrizioni, capitoli), una volta per voce e di nuovo solo se cambia, e le parole di ricerca ricavate dalle tue richieste.
• Ad Anthropic (generazione): il testo delle tue richieste e la descrizione, l'unità di misura e i capitoli delle voci candidate.

Non vengono inviati prezzi, importi, totali, dati del committente né il file del progetto. I provider trattano i dati secondo le loro condizioni per l'uso dell'API.

Le chiavi API restano nel portachiavi del sistema operativo e non sono mai scritte nei file di OpenComputo.

Se il prezzario proviene da un DCF distribuito da ACCA, le sue condizioni d'uso ne vietano l'uso con altri software: verifica di avere il diritto di inviarne il contenuto.

Puoi revocare il consenso in qualsiasi momento: l'assistente smette di chiamare i provider; la ricerca testuale e il calcolo restano disponibili.";

/// Stato del consenso registrato.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consenso {
    /// Versione del testo accettata (0: nessuna).
    #[serde(default)]
    pub versione: u32,
    /// Momento dell'accettazione, come dichiarato dal chiamante (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl Consenso {
    /// `true` se è stato accettato il testo corrente.
    #[must_use]
    pub fn valido(&self) -> bool {
        self.versione == VERSIONE
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn consenso_della_versione_corrente() {
        assert!(!Consenso::default().valido());
        assert!(
            Consenso {
                versione: VERSIONE,
                data: None
            }
            .valido()
        );
        assert!(
            !Consenso {
                versione: VERSIONE + 1,
                data: None
            }
            .valido()
        );
        assert!(TESTO.contains("fuori dal tuo computer"));
    }
}
