//! Prova della proposta nella sessione, senza modificare il progetto.
//!
//! I comandi della proposta passano dalla stessa [`Sessione`] della GUI, della CLI e
//! degli agenti: quantità e importi sono calcolati dal core con le regole di
//! arrotondamento del progetto, e una proposta che introdurrebbe errori di validazione è
//! rifiutata. L'esecuzione vera (`Sessione::esegui`) resta un'azione dell'utente.

use std::collections::BTreeSet;

use opencomputo_application::{Comando, ErroreApplicazione, Esito, Politica, Sessione, viste};
use opencomputo_core::numeri::{Importo, Prezzo, Quantita};
use serde::Serialize;

use crate::proposta::Proposta;

/// Prova i comandi della proposta.
///
/// # Errors
/// Proposta senza comandi, prezzario non caricato con il nome usato nella proposta,
/// comando non applicabile, nuovi errori di validazione.
pub fn prova(sessione: &Sessione, proposta: &Proposta) -> Result<Esito, ErroreApplicazione> {
    sessione.prova(&proposta.comandi)
}

/// Voce che la proposta aggiungerebbe, con quantità e importo calcolati dal core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoceAnteprima {
    /// Lavorazione della proposta.
    pub lavorazione: String,
    /// Codice della voce.
    pub codice: String,
    /// Descrizione breve.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo applicato (quello dell'elenco se la voce c'era già).
    pub prezzo: Prezzo,
    /// Quantità (assente se il core non la può calcolare).
    pub quantita: Option<Quantita>,
    /// Importo.
    pub importo: Option<Importo>,
}

/// Anteprima: esito della prova e voci che verrebbero aggiunte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anteprima {
    /// Esito della prova con la politica della sessione.
    pub esito: Esito,
    /// Computo di destinazione.
    pub computo: String,
    /// Voci aggiunte, nell'ordine delle lavorazioni incluse.
    pub voci: Vec<VoceAnteprima>,
}

/// Prova i comandi e calcola le voci che verrebbero aggiunte: i comandi sono eseguiti su
/// una copia del progetto, con lo stesso prezzario; il progetto della sessione non
/// cambia. Quantità e importi vengono dalle viste del core, mai ricalcolati qui.
///
/// # Errors
/// Come [`prova`].
pub fn anteprima(
    sessione: &Sessione,
    proposta: &Proposta,
) -> Result<Anteprima, ErroreApplicazione> {
    let esito = prova(sessione, proposta)?;
    let computo = proposta
        .comandi
        .iter()
        .find_map(|c| match c {
            Comando::AggiungiVoceComputo { computo, .. } => Some(computo.clone()),
            _ => None,
        })
        .flatten();
    let prima = viste::computo(sessione.progetto(), computo.as_deref())?;
    let esistenti: BTreeSet<_> = prima.voci.iter().map(|v| v.id).collect();

    // Copia della sessione con i prezzari usati dai comandi; politica permissiva: la
    // prova vera è quella sopra, qui servono solo i numeri.
    let mut copia = Sessione::new(sessione.progetto().clone()).con_politica(Politica {
        rifiuta_nuovi_errori: false,
        ..Politica::default()
    });
    for c in &proposta.comandi {
        if let Comando::CopiaDaPrezzario { prezzario, .. } = c
            && let Some((_, p)) = sessione.prezzari().iter().find(|(n, _)| n == prezzario)
        {
            copia.carica_prezzario(prezzario, p.clone());
        }
    }
    copia.esegui(&proposta.comandi)?;
    let dopo = viste::computo(copia.progetto(), computo.as_deref())?;
    let incluse = proposta.lavorazioni.iter().filter(|l| l.inclusa);
    let voci = dopo
        .voci
        .iter()
        .filter(|v| !esistenti.contains(&v.id))
        .zip(incluse)
        .map(|(v, l)| VoceAnteprima {
            lavorazione: l.id.clone(),
            codice: v.codice.clone(),
            descrizione: v.descrizione_breve.clone(),
            unita_misura: v.unita_misura.clone(),
            prezzo: v.prezzo,
            quantita: v.quantita,
            importo: v.importo,
        })
        .collect();
    Ok(Anteprima {
        esito,
        computo: dopo.nome,
        voci,
    })
}
