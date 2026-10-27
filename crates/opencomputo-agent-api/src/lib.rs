//! Interfaccia di OpenComputo per persone e agenti AI.
//!
//! * il binario `opencomputo` offre comandi singoli (`info`, `esegui`, `report`, …)
//!   che leggono e scrivono file di progetto e rispondono sempre in JSON;
//! * `opencomputo servizio` apre un [`Servizio`](servizio::Servizio) con stato che
//!   accetta una richiesta JSON per riga (protocollo descritto nel modulo
//!   [`servizio`]);
//! * `opencomputo schema-comandi` restituisce regole ed esempi dei comandi.
//!
//! I comandi singoli sono costruiti sulle stesse richieste del servizio, così i due
//! modi d'uso si comportano in modo identico.
//!
//! ```
//! use opencomputo_agent_api::servizio::Servizio;
//!
//! let mut s = Servizio::new();
//! s.rispondi(r#"{"richiesta": "nuovo", "titolo": "Prova"}"#);
//! let r = s.rispondi(r#"{"id": 7, "richiesta": "esegui", "comandi": [
//!     {"comando": "aggiungi_voce_prezzario", "codice": "A.1", "unita_misura": "m", "prezzo": "2.50"},
//!     {"comando": "aggiungi_voce_computo", "codice": "A.1", "righi": [{"lunghezza": "4"}]}]}"#);
//! assert_eq!(r["id"], 7);
//! assert_eq!(r["ok"], true);
//! assert_eq!(r["risultato"]["totali"][0]["dopo"], "10.00");
//! ```

pub mod assistente;
pub mod riepilogo;
pub mod schema;
pub mod servizio;

/// Codici di uscita del binario.
pub mod uscita {
    use opencomputo_application::ErroreApplicazione;

    /// Richiesta eseguita.
    pub const OK: u8 = 0;
    /// Richiesta rifiutata (comando non applicabile, nuovi errori di validazione,
    /// riferimento inesistente) o progetto non valido per `valida`.
    pub const RIFIUTATA: u8 = 1;
    /// Argomenti della riga di comando non validi.
    pub const USO: u8 = 2;
    /// Errore di file o di formato (lettura, scrittura, importazione, esportazione).
    pub const FILE: u8 = 3;
    /// Errore dell'assistente AI: consenso mancante, chiave assente o rifiutata, errore
    /// di un provider o della rete, risposta del modello inutilizzabile.
    pub const ASSISTENTE: u8 = 4;

    /// Codice di uscita per un errore.
    #[must_use]
    pub fn per_errore(e: &ErroreApplicazione) -> u8 {
        match e {
            ErroreApplicazione::File { .. }
            | ErroreApplicazione::Importazione { .. }
            | ErroreApplicazione::Esportazione { .. } => FILE,
            ErroreApplicazione::Assistente { .. } => ASSISTENTE,
            _ => RIFIUTATA,
        }
    }
}
