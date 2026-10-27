//! Prova della proposta nella sessione, senza modificare il progetto.
//!
//! I comandi della proposta passano dalla stessa [`Sessione`] della GUI, della CLI e
//! degli agenti: quantità e importi sono calcolati dal core con le regole di
//! arrotondamento del progetto, e una proposta che introdurrebbe errori di validazione è
//! rifiutata. L'esecuzione vera (`Sessione::esegui`) resta un'azione dell'utente.

use opencomputo_application::{ErroreApplicazione, Esito, Sessione};

use crate::proposta::Proposta;

/// Prova i comandi della proposta.
///
/// # Errors
/// Proposta senza comandi, prezzario non caricato con il nome usato nella proposta,
/// comando non applicabile, nuovi errori di validazione.
pub fn prova(sessione: &Sessione, proposta: &Proposta) -> Result<Esito, ErroreApplicazione> {
    sessione.prova(&proposta.comandi)
}
