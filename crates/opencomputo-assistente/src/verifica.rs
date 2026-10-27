//! Prova e anteprima delle scelte nella sessione, senza modificare il progetto.
//!
//! I comandi passano dalla stessa [`Sessione`] della GUI, della CLI e degli agenti:
//! quantità e importi sono calcolati dal core con le regole di arrotondamento del
//! progetto, e una scelta che introdurrebbe errori di validazione è rifiutata.
//! L'esecuzione vera (`Sessione::esegui`) resta un'azione dell'utente.

use std::collections::BTreeSet;

use opencomputo_application::{Comando, ErroreApplicazione, Esito, Politica, Sessione, viste};
use opencomputo_core::numeri::{Importo, Quantita};
use serde::Serialize;

use crate::proposta::Proposta;
use crate::scelta::{self, ErroreScelta, Scelta, VoceScelta};

/// Errore di una scelta come errore dell'applicazione.
#[must_use]
pub fn errore_scelta(e: &ErroreScelta) -> ErroreApplicazione {
    ErroreApplicazione::Assistente {
        motivo: match e {
            ErroreScelta::NessunaScelta => "nessuna_scelta",
            _ => "scelta_non_valida",
        }
        .into(),
        messaggio: e.to_string(),
    }
}

/// Comandi delle scelte.
///
/// # Errors
/// Scelte non valide.
pub fn comandi(
    sessione: &Sessione,
    proposta: &Proposta,
    scelte: &[Scelta],
) -> Result<(Vec<Comando>, Vec<VoceScelta>), ErroreApplicazione> {
    scelta::comandi(proposta, scelte, sessione.progetto()).map_err(|e| errore_scelta(&e))
}

/// Prova le scelte.
///
/// # Errors
/// Scelte non valide, prezzario non caricato con il nome della proposta, comando non
/// applicabile, nuovi errori di validazione.
pub fn prova(
    sessione: &Sessione,
    proposta: &Proposta,
    scelte: &[Scelta],
) -> Result<Esito, ErroreApplicazione> {
    sessione.prova(&comandi(sessione, proposta, scelte)?.0)
}

/// Valori di un'opzione con le misure correnti.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValoriOpzione {
    /// Lavorazione.
    pub lavorazione: String,
    /// Codice della voce.
    pub codice: String,
    /// Quantità (assente se il core non la può calcolare).
    pub quantita: Option<Quantita>,
    /// Importo.
    pub importo: Option<Importo>,
    /// Qualche rigo entrerebbe vuoto.
    pub da_completare: bool,
}

/// Anteprima: valori di ogni opzione e prova delle opzioni scelte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anteprima {
    /// Computo di destinazione.
    pub computo: String,
    /// Valori di tutte le opzioni di tutte le lavorazioni, con le misure correnti.
    pub opzioni: Vec<ValoriOpzione>,
    /// Voci che le scelte aggiungerebbero.
    pub scelte: Vec<VoceScelta>,
    /// Esito della prova delle scelte (assente se non è scelto nulla o la prova fallisce).
    pub esito: Option<Esito>,
    /// Motivo per cui le scelte non si possono applicare.
    pub errore: Option<ErroreApplicazione>,
}

/// Prova le scelte e calcola i valori di ogni opzione: tutte le opzioni sono aggiunte a
/// una copia del progetto, con lo stesso prezzario; il progetto della sessione non
/// cambia. Quantità e importi vengono dalle viste del core, mai ricalcolati qui.
///
/// # Errors
/// Misure non valide; computo inesistente.
pub fn anteprima(
    sessione: &Sessione,
    proposta: &Proposta,
    scelte: &[Scelta],
) -> Result<Anteprima, ErroreApplicazione> {
    let computo = proposta.computo.as_deref();
    let prima = viste::computo(sessione.progetto(), computo)?;
    let esistenti: BTreeSet<_> = prima.voci.iter().map(|v| v.id).collect();

    let tutte = scelta::tutte(proposta, scelte);
    let (comandi_tutti, voci_tutte) = match scelta::comandi(proposta, &tutte, sessione.progetto()) {
        Ok(c) => c,
        Err(ErroreScelta::NessunaScelta) => (Vec::new(), Vec::new()),
        Err(e) => return Err(errore_scelta(&e)),
    };
    let mut opzioni = Vec::new();
    if !comandi_tutti.is_empty() {
        // Politica permissiva: servono solo i numeri, la prova vera è sotto.
        let mut copia = Sessione::new(sessione.progetto().clone()).con_politica(Politica {
            rifiuta_nuovi_errori: false,
            ..Politica::default()
        });
        if let Some((_, p)) = sessione
            .prezzari()
            .iter()
            .find(|(n, _)| *n == proposta.nome_prezzario)
        {
            copia.carica_prezzario(&proposta.nome_prezzario, p.clone());
        }
        copia.esegui(&comandi_tutti)?;
        let dopo = viste::computo(copia.progetto(), computo)?;
        opzioni = dopo
            .voci
            .iter()
            .filter(|v| !esistenti.contains(&v.id))
            .zip(&voci_tutte)
            .map(|(v, s)| ValoriOpzione {
                lavorazione: s.lavorazione.clone(),
                codice: s.codice.clone(),
                quantita: v.quantita,
                importo: v.importo,
                da_completare: s.da_completare,
            })
            .collect();
    }
    let (esito, voci, errore) = match comandi(sessione, proposta, scelte) {
        Ok((c, voci)) => match sessione.prova(&c) {
            Ok(e) => (Some(e), voci, None),
            Err(e) => (None, voci, Some(e)),
        },
        Err(e) => (None, Vec::new(), Some(e)),
    };
    Ok(Anteprima {
        computo: prima.nome,
        opzioni,
        scelte: voci,
        esito,
        errore,
    })
}
