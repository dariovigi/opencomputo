//! # opencomputo-application
//!
//! Casi d'uso di OpenComputo condivisi da GUI, riga di comando e API per agenti:
//! tutti passano da qui, con le stesse validazioni.
//!
//! * [`Comando`]: modifiche al progetto come dati serializzabili (JSON).
//! * [`Sessione`]: richieste atomiche di comandi, rifiuto dei comandi che introducono
//!   errori di validazione, prova senza applicare, annulla e ripristina, apertura e
//!   salvataggio del file di progetto.
//! * [`casi_uso`]: import XPWE e DCF, prezzari di consultazione, export XPWE, report.
//!
//! ```
//! use opencomputo_application::{Comando, Sessione};
//!
//! let mut s = Sessione::nuovo("Manutenzione scuola");
//! let comandi: Vec<Comando> = serde_json::from_str(r#"[
//!   {"comando": "aggiungi_voce_prezzario", "codice": "A.1", "descrizione_breve": "Scavo",
//!    "unita_misura": "m³", "prezzo": "8.50"},
//!   {"comando": "aggiungi_voce_computo", "codice": "A.1",
//!    "righi": [{"descrizione": "Platea", "lunghezza": 12, "larghezza": 8, "altezza_peso": "1,20"}]}
//! ]"#).unwrap();
//! let esito = s.esegui(&comandi).unwrap();
//! // 12 × 8 × 1,20 = 115,20 m³ × 8,50 = 979,20 €
//! assert_eq!(esito.totali[0].dopo.map(|t| t.to_string()).as_deref(), Some("979.20"));
//! s.annulla().unwrap();
//! assert!(s.progetto().elenco_prezzi.voci.is_empty());
//! ```

pub mod casi_uso;
pub mod comandi;
mod comandi_analisi;
mod comandi_crono;
pub mod convertitore;
pub mod errore;
pub mod sessione;
pub mod viste;

pub use comandi::{Comando, NuovoRigo, RifVoce};
pub use errore::ErroreApplicazione;
pub use sessione::{Esito, Politica, Sessione};

/// Importo all'italiana per i messaggi.
pub(crate) fn formato_importo(i: opencomputo_core::numeri::Importo) -> String {
    opencomputo_io::formato::numero(i.valore(), 2)
}
