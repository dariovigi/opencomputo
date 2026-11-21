//! Configurazione del convertitore DCF/OCP esterno (ADR 0027): il percorso del programma
//! scelto dall'utente, in `convertitore.json` nella cartella di configurazione; in sua
//! assenza la variabile d'ambiente `DCF_BIN` della guida di integrazione (percorso
//! assoluto).

use std::path::{Path, PathBuf};

use opencomputo_application::convertitore::verifica_programma;
use opencomputo_application::errore::ErroreApplicazione;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Nome del file di configurazione.
pub const FILE_CONFIGURAZIONE: &str = "convertitore.json";

/// Variabile d'ambiente della guida di integrazione con il percorso del programma.
pub const VARIABILE: &str = "DCF_BIN";

/// Avviso mostrato prima di configurare e usare il convertitore.
pub const AVVISO: &str = "Il convertitore DCF esterno è un programma di terzi, installato sul tuo computer: \
OpenComputo non lo contiene, non lo distribuisce e non ne verifica il funzionamento. \
I documenti vengono elaborati in locale da quel programma (OpenComputo non li invia in rete): \
il progetto importato viene controllato da OpenComputo come qualunque file, il DCF esportato \
è prodotto interamente dal convertitore. Dopo ogni conversione OpenComputo chiede al \
convertitore la verifica di fedeltà (prezzi, quantità, importi e SAL). \
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

/// Programma scelto e da dove viene: la configurazione prevale su `DCF_BIN`.
fn scelto(
    cartella: &Path,
    dcf_bin: Option<std::ffi::OsString>,
) -> Result<Option<(PathBuf, &'static str)>, String> {
    let c = leggi(cartella)?;
    Ok(c.programma.map(|p| (p, "configurazione")).or_else(|| {
        dcf_bin
            .filter(|v| !v.is_empty())
            .map(|v| (PathBuf::from(v), VARIABILE))
    }))
}

fn dcf_bin() -> Option<std::ffi::OsString> {
    std::env::var_os(VARIABILE)
}

/// Programma configurato e pronto all'uso.
///
/// # Errors
/// Nessun programma configurato, configurazione illeggibile, programma non più presente.
pub fn programma(cartella: &Path) -> Result<PathBuf, ErroreApplicazione> {
    programma_con(cartella, dcf_bin())
}

fn programma_con(
    cartella: &Path,
    dcf_bin: Option<std::ffi::OsString>,
) -> Result<PathBuf, ErroreApplicazione> {
    let (p, _) = scelto(cartella, dcf_bin)
        .map_err(|messaggio| ErroreApplicazione::Richiesta { messaggio })?
        .ok_or_else(|| ErroreApplicazione::Richiesta {
            messaggio: format!(
                "nessun convertitore DCF esterno configurato: impostarlo con `convertitore_imposta` (o con la variabile {VARIABILE})"
            ),
        })?;
    verifica_programma(&p)?;
    Ok(p)
}

/// Stato per l'interfaccia: programma, se è utilizzabile, problema, avviso.
#[must_use]
pub fn stato(cartella: &Path) -> Value {
    stato_con(cartella, dcf_bin())
}

fn stato_con(cartella: &Path, dcf_bin: Option<std::ffi::OsString>) -> Value {
    match scelto(cartella, dcf_bin) {
        Ok(s) => {
            let problema = s
                .as_ref()
                .and_then(|(p, _)| verifica_programma(p).err())
                .map(|e| e.to_string());
            json!({
                "programma": s.as_ref().map(|(p, _)| p),
                "origine": s.as_ref().map(|(_, o)| o),
                "utilizzabile": s.is_some() && problema.is_none(),
                "problema": problema,
                "avviso": AVVISO,
            })
        }
        Err(problema) => json!({
            "programma": null,
            "origine": null,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod prove {
    use super::*;

    #[test]
    fn dcf_bin_usato_solo_senza_configurazione() {
        let cartella =
            std::env::temp_dir().join(format!("opencomputo-prova-dcf-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cartella);
        let questo = std::env::current_exe().unwrap();
        // Nessuna configurazione: vale DCF_BIN, se assoluto ed esistente.
        let s = stato_con(&cartella, Some(questo.clone().into_os_string()));
        assert_eq!(s["origine"], "DCF_BIN");
        assert_eq!(s["utilizzabile"], true);
        assert_eq!(
            programma_con(&cartella, Some(questo.clone().into())).unwrap(),
            questo
        );
        // Relativo (il predefinito «dcf» del ponte): segnalato, non usato.
        let s = stato_con(&cartella, Some("dcf".into()));
        assert_eq!(s["utilizzabile"], false);
        assert!(s["problema"].as_str().unwrap().contains("assoluto"));
        assert!(programma_con(&cartella, Some("dcf".into())).is_err());
        assert!(programma_con(&cartella, None).is_err());
        assert_eq!(
            stato_con(&cartella, Some("".into()))["programma"],
            Value::Null
        );
        // La configurazione prevale.
        let altro = std::env::temp_dir();
        imposta(&cartella, Some(questo.clone())).unwrap();
        let s = stato_con(&cartella, Some(altro.into_os_string()));
        assert_eq!(s["origine"], "configurazione");
        assert_eq!(s["programma"], json!(questo));
        let _ = std::fs::remove_dir_all(&cartella);
    }
}
