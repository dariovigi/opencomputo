//! Configurazione del convertitore DCF esterno (ADR 0027): il percorso del programma
//! scelto dall'utente, in `convertitore.json` nella cartella di configurazione.

use std::path::{Path, PathBuf};

use opencomputo_application::convertitore::verifica_programma;
use opencomputo_application::errore::ErroreApplicazione;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Nome del file di configurazione.
pub const FILE_CONFIGURAZIONE: &str = "convertitore.json";

/// Avviso mostrato prima di configurare e usare il convertitore.
pub const AVVISO: &str = "Il convertitore DCF esterno è un programma di terzi, installato sul tuo computer: \
OpenComputo non lo contiene, non lo distribuisce e non ne verifica il funzionamento. \
Il documento viene elaborato in locale da quel programma (non viene inviato in rete da \
OpenComputo); il progetto prodotto viene poi controllato da OpenComputo come qualunque file. \
Usa solo un programma di cui ti fidi e che sei autorizzato a usare sui documenti che converti.";

/// Contenuto del file di configurazione.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configurazione {
    /// Percorso assoluto del programma.
    #[serde(default)]
    programma: Option<PathBuf>,
}

/// Legge la configurazione: assente → nessun programma; illeggibile → errore descritto.
fn leggi(cartella: &Path) -> Result<Configurazione, String> {
    match std::fs::read(cartella.join(FILE_CONFIGURAZIONE)) {
        Ok(b) => serde_json::from_slice(&b)
            .map_err(|e| format!("configurazione del convertitore non leggibile: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Configurazione::default()),
        Err(e) => Err(format!(
            "configurazione del convertitore non leggibile: {e}"
        )),
    }
}

/// Programma configurato e pronto all'uso.
///
/// # Errors
/// Nessun programma configurato, configurazione illeggibile, programma non più presente.
pub fn programma(cartella: &Path) -> Result<PathBuf, ErroreApplicazione> {
    let c = leggi(cartella).map_err(|messaggio| ErroreApplicazione::Richiesta { messaggio })?;
    let p = c.programma.ok_or_else(|| ErroreApplicazione::Richiesta {
        messaggio:
            "nessun convertitore DCF esterno configurato: impostarlo con `convertitore_imposta`"
                .into(),
    })?;
    verifica_programma(&p)?;
    Ok(p)
}

/// Stato per l'interfaccia: programma, se è utilizzabile, problema, avviso.
#[must_use]
pub fn stato(cartella: &Path) -> Value {
    match leggi(cartella) {
        Ok(c) => {
            let problema = c
                .programma
                .as_deref()
                .and_then(|p| verifica_programma(p).err())
                .map(|e| e.to_string());
            json!({
                "programma": c.programma,
                "utilizzabile": c.programma.is_some() && problema.is_none(),
                "problema": problema,
                "avviso": AVVISO,
            })
        }
        Err(problema) => json!({
            "programma": null,
            "utilizzabile": false,
            "problema": problema,
            "avviso": AVVISO,
        }),
    }
}

/// Imposta (o toglie, con `None`) il programma; lo verifica prima di salvarlo.
///
/// # Errors
/// Programma non valido; scrittura non riuscita.
pub fn imposta(cartella: &Path, programma: Option<PathBuf>) -> Result<Value, ErroreApplicazione> {
    if let Some(p) = &programma {
        verifica_programma(p)?;
    }
    std::fs::create_dir_all(cartella).map_err(|e| ErroreApplicazione::File {
        messaggio: format!("{}: {e}", cartella.display()),
    })?;
    let b = serde_json::to_vec_pretty(&Configurazione { programma }).unwrap_or_default();
    crate::assistente::scrivi_atomico(&cartella.join(FILE_CONFIGURAZIONE), &b)?;
    Ok(stato(cartella))
}
