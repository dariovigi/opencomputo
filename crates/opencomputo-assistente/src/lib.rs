//! # opencomputo-assistente
//!
//! Assistente AI per il computo metrico (ADR 0025). Questo crate contiene le parti
//! **locali e deterministiche**; le chiamate ai provider arrivano nella fase successiva.
//!
//! Flusso di una richiesta:
//!
//! 1. il modello analizza la richiesta ([`risposta::RispostaAnalisi`], schema in
//!    [`schema`]) e indica le lavorazioni con le parole da cercare;
//! 2. [`ricerca::IndiceTestuale`] trova le voci del prezzario e
//!    [`candidati::Candidati`] le numera con handle validi per una sola richiesta;
//! 3. il modello sceglie un handle per lavorazione e propone i righi con le forme del
//!    catalogo chiuso ([`forme`]);
//! 4. [`proposta::valida_selezione`] controlla tutto (handle, unità, parametri, valori
//!    fondati nella richiesta, ruoli, duplicati) e produce una [`proposta::Proposta`] con i
//!    comandi di `opencomputo-application`;
//! 5. [`verifica::prova`] esegue i comandi in prova: quantità e importi li calcola il core.
//!
//! Il modello non sceglie prezzi né codici e non scrive formule; ogni voce proposta
//! conserva nelle note la lavorazione, la motivazione e le ipotesi.

pub mod candidati;
pub mod forme;
pub mod proposta;
pub mod ricerca;
pub mod risposta;
pub mod schema;
pub mod testo;
pub mod unita;
pub mod valori;
pub mod verifica;

pub use candidati::{Candidati, Candidato};
pub use proposta::{Contesto, ErroreRisposta, EsitoProposta, Proposta, valida_selezione};
pub use ricerca::{FiltroVoci, IndiceTestuale, RisultatoRicerca};
