//! Convertitore DCF esterno (ADR 0027): un programma scelto dall'utente, installato sul
//! suo computer, che converte un documento `.dcf` in un progetto `.ocp`.
//!
//! OpenComputo non contiene né distribuisce il convertitore e non ne conosce i formati
//! interni: lo esegue secondo il contratto della sua riga di comando e tratta il
//! risultato come un file qualunque da aprire (limiti di dimensione, migrazioni,
//! validazione). Il lettore DCF di OpenComputo resta quello di ADR 0006.
//!
//! Contratto del programma:
//! * `programma convert <origine> <uscita> --report <rapporto>`;
//! * uscita 0: riuscito, file `.ocp` scritto; uscita diversa: errore;
//! * stderr `errore: <descrizione>` solo in caso di errore; stdout il resoconto in UTF-8.
//!
//! Misure: argomenti passati come lista (mai una shell); solo le variabili d'ambiente
//! indispensabili, così il programma non riceve eventuali credenziali presenti
//! nell'ambiente; cartella di lavoro temporanea privata, rimossa alla fine; durata
//! massima, dopo la quale il processo viene terminato; output letto fino a un limite.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use opencomputo_core::progetto::Progetto;
use serde::Serialize;

use crate::errore::ErroreApplicazione;

/// Durata massima di una conversione.
pub const DURATA_MASSIMA: Duration = Duration::from_secs(120);

/// Dimensione massima del documento da convertire (come gli altri import, ADR 0026).
pub const ORIGINE_MASSIMA: u64 = 256 << 20;

/// Byte letti al massimo da stdout, da stderr e dal file del resoconto.
const TESTO_MASSIMO: usize = 1 << 20;

/// Caratteri del resoconto riportati al chiamante.
pub const RESOCONTO_MASSIMO: usize = 100_000;

/// Variabili d'ambiente passate al convertitore: quelle che servono a un programma per
/// avviarsi e trovare le cartelle temporanee, nessun'altra.
pub const AMBIENTE_CONSENTITO: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "SystemRoot",
    "SYSTEMROOT",
    "windir",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "LC_ALL",
];

/// Esito di una conversione riuscita.
#[derive(Debug)]
pub struct Conversione {
    /// Progetto prodotto, già letto e migrato come un file `.ocp`.
    pub progetto: Progetto,
    /// Resoconto del convertitore.
    pub resoconto: Resoconto,
}

/// Testi prodotti dal convertitore.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Resoconto {
    /// Nome del programma usato (senza la cartella).
    pub programma: String,
    /// Resoconto della conversione: il file `--report` se scritto, altrimenti stdout.
    pub testo: String,
    /// Messaggi su stderr (avvisi), se presenti.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub messaggi: String,
    /// `true` se uno dei testi è stato troncato.
    pub troncato: bool,
}

fn errore(messaggio: impl Into<String>) -> ErroreApplicazione {
    ErroreApplicazione::Importazione {
        messaggio: messaggio.into(),
    }
}

/// Controlla il percorso del programma: assoluto, esistente, un file.
///
/// # Errors
/// Percorso relativo, inesistente o non un file.
pub fn verifica_programma(programma: &Path) -> Result<(), ErroreApplicazione> {
    if !programma.is_absolute() {
        return Err(ErroreApplicazione::Richiesta {
            messaggio: format!(
                "il percorso del convertitore deve essere assoluto: {}",
                programma.display()
            ),
        });
    }
    if !programma.is_file() {
        return Err(ErroreApplicazione::Richiesta {
            messaggio: format!(
                "convertitore non trovato (o non è un file): {}",
                programma.display()
            ),
        });
    }
    Ok(())
}

/// Cartella temporanea privata, rimossa quando esce di scena.
struct CartellaTemporanea(PathBuf);

impl CartellaTemporanea {
    fn nuova() -> Result<Self, ErroreApplicazione> {
        static CONTATORE: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        for _ in 0..16 {
            let n = CONTATORE.fetch_add(1, Ordering::Relaxed);
            let istante = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos());
            let p = base.join(format!(
                "opencomputo-convertitore-{}-{istante:x}-{n}",
                std::process::id()
            ));
            // `create_dir` fallisce se la cartella esiste già: nessuna cartella altrui
            // viene riusata.
            match std::fs::create_dir(&p) {
                Ok(()) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt as _;
                        let _ =
                            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700));
                    }
                    return Ok(Self(p));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => {
                    return Err(errore(format!(
                        "cartella temporanea non creata in {}: {e}",
                        base.display()
                    )));
                }
            }
        }
        Err(errore("cartella temporanea non creata"))
    }
}

impl Drop for CartellaTemporanea {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Legge un flusso fino al limite in un thread; il resto viene scartato (così il
/// programma non resta bloccato su una pipe piena).
fn leggi_in_disparte(
    flusso: Option<impl std::io::Read + Send + 'static>,
) -> mpsc::Receiver<(Vec<u8>, bool)> {
    let (tx, rx) = mpsc::channel();
    if let Some(mut f) = flusso {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = (&mut f)
                .take(u64::try_from(TESTO_MASSIMO).unwrap_or(u64::MAX))
                .read_to_end(&mut buf);
            let resto = std::io::copy(&mut f, &mut std::io::sink()).unwrap_or(0);
            let _ = tx.send((buf, resto > 0));
        });
    } else {
        let _ = tx.send((Vec::new(), false));
    }
    rx
}

/// Testo leggibile: UTF-8 con sostituzione, senza caratteri di controllo (tranne a capo
/// e tabulazione), al più `massimo` caratteri.
fn testo_pulito(byte: &[u8], massimo: usize) -> (String, bool) {
    let t = String::from_utf8_lossy(byte);
    let mut troncato = false;
    let mut out = String::new();
    for (i, c) in t
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .enumerate()
    {
        if i >= massimo {
            troncato = true;
            break;
        }
        out.push(c);
    }
    (out.trim_end().to_owned(), troncato)
}

/// Attende la fine del processo entro `durata`; oltre, lo termina.
fn attendi(
    figlio: &mut Child,
    durata: Duration,
) -> Result<std::process::ExitStatus, ErroreApplicazione> {
    let inizio = Instant::now();
    loop {
        match figlio.try_wait() {
            Ok(Some(stato)) => return Ok(stato),
            Ok(None) => {}
            Err(e) => return Err(errore(format!("attesa del convertitore non riuscita: {e}"))),
        }
        if inizio.elapsed() >= durata {
            let _ = figlio.kill();
            let _ = figlio.wait();
            return Err(errore(format!(
                "il convertitore non ha finito entro {} secondi ed è stato interrotto",
                durata.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Converte `origine` con il programma indicato e legge il progetto prodotto.
///
/// # Errors
/// Programma o origine non validi, avvio non riuscito, durata superata, uscita diversa
/// da 0 (con il messaggio del convertitore), file prodotto assente o non leggibile come
/// progetto OpenComputo.
pub fn converti(
    programma: &Path,
    origine: &Path,
    durata: Duration,
) -> Result<Conversione, ErroreApplicazione> {
    verifica_programma(programma)?;
    let dimensione =
        std::fs::metadata(origine).map_err(|e| errore(format!("{}: {e}", origine.display())))?;
    if !dimensione.is_file() {
        return Err(errore(format!("{} non è un file", origine.display())));
    }
    if dimensione.len() > ORIGINE_MASSIMA {
        return Err(errore(format!(
            "{} supera il massimo di {} MB per un import",
            origine.display(),
            ORIGINE_MASSIMA >> 20
        )));
    }
    // Percorso assoluto: il programma lavora nella cartella temporanea.
    let origine =
        std::path::absolute(origine).map_err(|e| errore(format!("{}: {e}", origine.display())))?;

    let cartella = CartellaTemporanea::nuova()?;
    let uscita = cartella.0.join("convertito.ocp");
    let rapporto = cartella.0.join("resoconto.txt");

    let mut comando = Command::new(programma);
    comando
        .arg("convert")
        .arg(&origine)
        .arg(&uscita)
        .arg("--report")
        .arg(&rapporto)
        .current_dir(&cartella.0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    for nome in AMBIENTE_CONSENTITO {
        if let Some(v) = std::env::var_os(nome) {
            comando.env(nome, v);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW: nessuna finestra di console sopra l'interfaccia.
        comando.creation_flags(0x0800_0000);
    }
    let mut figlio = comando.spawn().map_err(|e| {
        errore(format!(
            "convertitore non avviato ({}): {e}",
            programma.display()
        ))
    })?;
    let stdout = leggi_in_disparte(figlio.stdout.take());
    let stderr = leggi_in_disparte(figlio.stderr.take());
    let stato = attendi(&mut figlio, durata)?;
    // Un processo figlio del convertitore potrebbe tenere aperte le pipe: si aspetta poco.
    let raccogli = |rx: mpsc::Receiver<(Vec<u8>, bool)>| {
        rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default()
    };
    let (out, out_oltre) = raccogli(stdout);
    let (err, err_oltre) = raccogli(stderr);
    let (messaggi, messaggi_troncati) = testo_pulito(&err, RESOCONTO_MASSIMO);

    if !stato.success() {
        let codice = stato
            .code()
            .map_or_else(|| "interrotto".to_owned(), |c| format!("uscita {c}"));
        let motivo = messaggi
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim().trim_start_matches("errore:").trim().to_owned())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "nessun messaggio".to_owned());
        let motivo: String = motivo.chars().take(500).collect();
        return Err(errore(format!(
            "il convertitore non è riuscito ({codice}): {motivo}"
        )));
    }

    let progetto = opencomputo_storage::apri(&uscita).map_err(|e| {
        errore(format!(
            "il convertitore ha terminato senza errori ma il risultato non è un progetto OpenComputo leggibile: {e}"
        ))
    })?;

    // Il resoconto su file è quello completo; altrimenti stdout.
    let da_file = std::fs::File::open(&rapporto).ok().map(|f| {
        let mut b = Vec::new();
        let _ = f
            .take(u64::try_from(TESTO_MASSIMO).unwrap_or(u64::MAX))
            .read_to_end(&mut b);
        b
    });
    let (testo, testo_troncato) = match da_file {
        Some(b) if !b.is_empty() => testo_pulito(&b, RESOCONTO_MASSIMO),
        _ => testo_pulito(&out, RESOCONTO_MASSIMO),
    };
    Ok(Conversione {
        progetto,
        resoconto: Resoconto {
            programma: programma
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            testo,
            messaggi,
            troncato: testo_troncato || messaggi_troncati || out_oltre || err_oltre,
        },
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod prove {
    use super::*;

    #[test]
    fn testo_pulito_toglie_i_controlli_e_tronca() {
        let (t, tr) = testo_pulito(b"riga\x1b[31m 1\r\n\triga 2\x00\xff", 100);
        assert_eq!(t, "riga[31m 1\n\triga 2\u{fffd}");
        assert!(!tr);
        let (t, tr) = testo_pulito("àèìòù".as_bytes(), 3);
        assert_eq!(t, "àèì");
        assert!(tr);
    }

    #[test]
    fn programma_relativo_o_assente_rifiutato() {
        assert!(verifica_programma(Path::new("dcf")).is_err());
        let assente = std::env::temp_dir().join("opencomputo-convertitore-che-non-esiste");
        assert!(verifica_programma(&assente).is_err());
        assert!(verifica_programma(&std::env::temp_dir()).is_err());
    }

    #[test]
    fn cartelle_temporanee_distinte_e_rimosse() {
        let a = CartellaTemporanea::nuova().unwrap();
        let b = CartellaTemporanea::nuova().unwrap();
        assert_ne!(a.0, b.0);
        let p = a.0.clone();
        std::fs::write(p.join("x"), b"1").unwrap();
        drop(a);
        assert!(!p.exists());
    }
}
