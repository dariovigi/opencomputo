//! # opencomputo-gui
//!
//! Applicazione desktop di OpenComputo con Tauri v2 ([ADR 0009]).
//!
//! Il lato Rust è volutamente sottile: tiene un [`Servizio`] di `opencomputo-agent-api`
//! ed espone il comando `richiesta`, che riceve la stessa richiesta JSON di
//! `opencomputo servizio` e ne restituisce la risposta. L'interfaccia (cartella `ui`,
//! JavaScript senza build) non ha altro accesso ai dati: GUI e agenti passano dagli stessi
//! casi d'uso e dalle stesse validazioni.
//!
//! Altri comandi:
//! * `apri_finestra`: una vista in una finestra separata sullo stesso progetto, a cascata
//!   rispetto alla principale;
//! * `trascinamento`: il contenuto dell'ultimo trascinamento di voci iniziato in una
//!   finestra. Serve da riserva quando il sistema non passa tra finestre diverse i dati
//!   propri del trascinamento;
//! * `apri_file`: apre un file prodotto con l'applicazione di sistema;
//! * `esci`: chiusura confermata dall'utente.
//!
//! Dopo ogni richiesta che cambia il progetto viene emesso l'evento
//! [`EVENTO_AGGIORNATO`], così le altre finestre si aggiornano.
//!
//! [ADR 0009]: ../../docs/adr/0009-gui-tauri.md

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};

use opencomputo_agent_api::servizio::Servizio;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WindowEvent};

/// Etichetta della finestra principale.
pub const FINESTRA_PRINCIPALE: &str = "principale";
/// Evento emesso dopo ogni richiesta che cambia il progetto; il dato è
/// `{"origine": etichetta della finestra che l'ha inviata}`.
pub const EVENTO_AGGIORNATO: &str = "progetto-aggiornato";
/// Evento emesso alla finestra principale quando si tenta di chiuderla con modifiche
/// non salvate: l'interfaccia chiede conferma e poi invoca `esci`.
pub const EVENTO_CHIUSURA: &str = "chiusura-richiesta";
/// Evento emesso quando il sistema chiede di aprire un file (doppio clic su un `.ocp`
/// con un progetto già aperto): il dato è `{"percorso": …}` e l'interfaccia chiede se
/// salvare le modifiche prima di aprirlo.
pub const EVENTO_APRI_FILE: &str = "apri-file";

/// Esito di una richiesta di apertura arrivata dal sistema operativo.
#[derive(Debug, Clone, PartialEq)]
pub enum AperturaDalSistema {
    /// Nessun progetto era aperto: il file è stato aperto (risposta del servizio).
    Aperto(Value),
    /// C'è già un progetto: decide l'interfaccia (eventuali modifiche da salvare).
    DaProporre(String),
}

/// Stato condiviso dalle finestre: un progetto per istanza dell'applicazione.
#[derive(Debug, Default)]
pub struct Stato {
    servizio: Mutex<Servizio>,
    uscita_confermata: AtomicBool,
    finestre: AtomicU32,
    trascinamento: Mutex<Option<Value>>,
}

impl Stato {
    /// Accesso al servizio (un mutex avvelenato non blocca l'applicazione: lo stato del
    /// servizio resta coerente perché ogni richiesta è atomica).
    pub fn servizio(&self) -> MutexGuard<'_, Servizio> {
        self.servizio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `true` se il progetto aperto ha modifiche non salvate.
    pub fn modificato(&self) -> bool {
        self.servizio().sessione().is_some_and(|s| s.modificato())
    }

    /// Gestisce una richiesta JSON: risposta e indicazione di cambiamento.
    pub fn gestisci(&self, richiesta: Value) -> (Value, bool) {
        self.servizio().rispondi_valore(richiesta)
    }

    /// Apre un file chiesto dal sistema: subito se non c'è un progetto aperto, altrimenti
    /// lo propone all'interfaccia.
    pub fn apri_dal_sistema(&self, percorso: &Path) -> AperturaDalSistema {
        let percorso = percorso.to_string_lossy().into_owned();
        if self.servizio().sessione().is_some() {
            return AperturaDalSistema::DaProporre(percorso);
        }
        AperturaDalSistema::Aperto(
            self.gestisci(json!({"richiesta": "apri", "percorso": percorso}))
                .0,
        )
    }

    /// Trascinamento di voci tra finestre: `inizia` conserva i dati, `leggi` li restituisce,
    /// `fine` li cancella. Restituisce i dati presenti dopo l'azione.
    ///
    /// # Errors
    /// Azione sconosciuta o dati troppo grandi.
    pub fn trascinamento(
        &self,
        azione: &str,
        dati: Option<Value>,
    ) -> Result<Option<Value>, String> {
        let mut t = self
            .trascinamento
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match azione {
            "inizia" => {
                let d = dati.ok_or("dati del trascinamento assenti")?;
                if d.to_string().len() > 1 << 20 {
                    return Err("dati del trascinamento troppo grandi".into());
                }
                *t = Some(d);
            }
            "leggi" => {}
            "fine" => *t = None,
            altro => return Err(format!("azione sconosciuta: {altro}")),
        }
        Ok(t.clone())
    }

    /// Apre il primo argomento che non è un'opzione, se presente.
    pub fn apri_iniziale(&self, argomenti: impl IntoIterator<Item = String>) -> Option<Value> {
        let percorso = argomenti.into_iter().find(|a| !a.starts_with('-'))?;
        Some(
            self.gestisci(json!({"richiesta": "apri", "percorso": percorso}))
                .0,
        )
    }
}

/// Inoltra una richiesta al servizio. Asincrono: import e report lunghi non bloccano
/// le finestre.
#[tauri::command]
async fn richiesta(
    app: AppHandle,
    finestra: WebviewWindow,
    stato: State<'_, Stato>,
    richiesta: Value,
) -> Result<Value, String> {
    let (risposta, cambiato) = stato.gestisci(richiesta);
    if cambiato {
        let _ = app.emit(EVENTO_AGGIORNATO, json!({"origine": finestra.label()}));
    }
    Ok(risposta)
}

/// Caratteri ammessi nell'indirizzo interno di una vista.
fn vista_valida(vista: &str) -> bool {
    !vista.is_empty()
        && vista.len() <= 200
        && vista
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-=&.%".contains(c))
}

/// Apre una vista in una nuova finestra sullo stesso progetto.
#[tauri::command]
async fn apri_finestra(
    app: AppHandle,
    stato: State<'_, Stato>,
    vista: String,
    titolo: String,
) -> Result<String, String> {
    if !vista_valida(&vista) {
        return Err(format!("vista non valida: {vista}"));
    }
    let n = stato
        .finestre
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    let etichetta = format!("vista-{n}");
    tauri::WebviewWindowBuilder::new(
        &app,
        &etichetta,
        WebviewUrl::App(format!("index.html#{vista}").into()),
    )
    .title(titolo)
    .inner_size(1100.0, 760.0)
    .min_inner_size(640.0, 420.0)
    .disable_drag_drop_handler()
    .build()
    .map_err(|e| e.to_string())?;
    // A cascata rispetto alla principale, così le finestre non si coprono esattamente.
    if let (Some(p), Ok(nuova)) = (
        app.get_webview_window(FINESTRA_PRINCIPALE)
            .and_then(|w| w.outer_position().ok()),
        app.get_webview_window(&etichetta).ok_or(()),
    ) {
        let passo = i32::try_from(n % 8).unwrap_or(0) * 36 + 36;
        let _ = nuova.set_position(tauri::PhysicalPosition::new(p.x + passo, p.y + passo));
    }
    Ok(etichetta)
}

/// Dati del trascinamento di voci in corso (vedi [`Stato::trascinamento`]).
#[tauri::command]
fn trascinamento(
    stato: State<'_, Stato>,
    azione: String,
    dati: Option<Value>,
) -> Result<Option<Value>, String> {
    stato.trascinamento(&azione, dati)
}

/// Apre un file (report, export) con l'applicazione predefinita del sistema.
#[tauri::command]
async fn apri_file(percorso: String) -> Result<(), String> {
    let p = Path::new(&percorso);
    if !p.is_file() {
        return Err(format!("file inesistente: {percorso}"));
    }
    let mut c = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer");
        c.arg("/select,");
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    c.arg(p).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Chiude l'applicazione dopo la conferma dell'utente (modifiche salvate o scartate).
#[tauri::command]
fn esci(app: AppHandle, stato: State<'_, Stato>) {
    stato.uscita_confermata.store(true, Ordering::Relaxed);
    app.exit(0);
}

/// Avvia l'applicazione.
pub fn run() {
    let stato = Stato::default();
    if let Some(r) = stato.apri_iniziale(std::env::args().skip(1))
        && r.get("ok") != Some(&Value::Bool(true))
    {
        eprintln!("opencomputo-gui: {r}");
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(stato)
        .invoke_handler(tauri::generate_handler![
            richiesta,
            apri_finestra,
            trascinamento,
            apri_file,
            esci
        ])
        .on_window_event(|finestra, evento| {
            if finestra.label() != FINESTRA_PRINCIPALE {
                return;
            }
            match evento {
                WindowEvent::CloseRequested { api, .. } => {
                    let stato = finestra.state::<Stato>();
                    if stato.modificato() && !stato.uscita_confermata.load(Ordering::Relaxed) {
                        api.prevent_close();
                        let _ = finestra.emit_to(FINESTRA_PRINCIPALE, EVENTO_CHIUSURA, ());
                    }
                }
                WindowEvent::Destroyed => finestra.app_handle().exit(0),
                _ => {}
            }
        })
        .build(tauri::generate_context!());
    let app = match app {
        Ok(a) => a,
        Err(e) => {
            eprintln!("opencomputo-gui: {e}");
            std::process::exit(1);
        }
    };
    app.run(|_app, _evento| {
        // macOS consegna i file aperti dal Finder (doppio clic, «Apri con») come evento,
        // non come argomento.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = &_evento {
            for p in urls.iter().filter_map(|u| u.to_file_path().ok()) {
                apri_dal_sistema(_app, &p);
            }
        }
    });
}

/// Apre un file chiesto dal sistema e avvisa le finestre.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn apri_dal_sistema(app: &AppHandle, percorso: &Path) {
    match app.state::<Stato>().apri_dal_sistema(percorso) {
        AperturaDalSistema::Aperto(r) => {
            if r.get("ok") != Some(&Value::Bool(true)) {
                eprintln!("opencomputo-gui: {r}");
            }
            let _ = app.emit(EVENTO_AGGIORNATO, json!({"origine": "sistema"}));
        }
        AperturaDalSistema::DaProporre(p) => {
            let _ = app.emit_to(
                FINESTRA_PRINCIPALE,
                EVENTO_APRI_FILE,
                json!({"percorso": p}),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;

    #[test]
    fn richieste_e_modifiche() {
        let s = Stato::default();
        assert!(!s.modificato());
        let (r, cambiato) = s.gestisci(json!({"richiesta": "nuovo", "titolo": "Prova"}));
        assert_eq!(r["ok"], true);
        assert!(cambiato);
        assert!(s.modificato(), "un progetto nuovo non è ancora salvato");
        let (r, cambiato) = s.gestisci(json!({"richiesta": "computo"}));
        assert_eq!(r["risultato"]["nome"], "Computo metrico estimativo");
        assert!(!cambiato);
    }

    #[test]
    fn apertura_dagli_argomenti() {
        let s = Stato::default();
        assert!(s.apri_iniziale(["--verbose".to_owned()]).is_none());
        let r = s.apri_iniziale(["/inesistente/x.ocp".to_owned()]).unwrap();
        assert_eq!(r["errore"]["codice"], "file");
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        );
        let r = s.apri_iniziale([fixture.to_owned()]).unwrap();
        assert_eq!(r["risultato"]["computi"][0]["totale"], "14457.12");
    }

    #[test]
    fn file_aperti_dal_sistema() {
        let s = Stato::default();
        let fixture = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        ));
        let AperturaDalSistema::Aperto(r) = s.apri_dal_sistema(fixture) else {
            panic!("senza progetto aperto il file si apre subito");
        };
        assert_eq!(r["ok"], true);
        // Con un progetto aperto la scelta passa all'interfaccia.
        assert_eq!(
            s.apri_dal_sistema(Path::new("/altro.ocp")),
            AperturaDalSistema::DaProporre("/altro.ocp".into())
        );
    }

    #[test]
    fn trascinamento_tra_finestre() {
        let s = Stato::default();
        assert_eq!(s.trascinamento("leggi", None).unwrap(), None);
        let d = json!({"tipo": "computo", "computo": "A", "voci": [1, 2]});
        assert_eq!(
            s.trascinamento("inizia", Some(d.clone())).unwrap(),
            Some(d.clone())
        );
        assert_eq!(s.trascinamento("leggi", None).unwrap(), Some(d));
        assert_eq!(s.trascinamento("fine", None).unwrap(), None);
        assert!(s.trascinamento("inizia", None).is_err());
        assert!(s.trascinamento("altro", None).is_err());
        let grande = json!({"x": "y".repeat(2 << 20)});
        assert!(s.trascinamento("inizia", Some(grande)).is_err());
    }

    #[test]
    fn viste_ammesse_nelle_finestre() {
        assert!(vista_valida("elenco"));
        assert!(vista_valida("computo=Computo%20metrico"));
        assert!(!vista_valida(""));
        assert!(!vista_valida("../../etc"));
        assert!(!vista_valida("x\"onload"));
    }
}
