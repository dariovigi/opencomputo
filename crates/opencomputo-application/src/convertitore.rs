//! Convertitore DCF/OCP esterno (ADR 0027): la CLI `dcf` (pacchetto «primus-dcf»), un
//! programma scelto dall'utente e installato sul suo computer, che converte un documento
//! `.dcf` in un progetto `.ocp` e viceversa e verifica la fedeltà della conversione.
//!
//! OpenComputo non contiene né distribuisce il convertitore e non ne conosce i formati:
//! lo usa secondo il contratto della sua riga di comando, come il modulo ponte di
//! riferimento della guida di integrazione (`dcf_bridge.py`), qui riscritto in Rust. Il
//! progetto prodotto si legge come un file qualunque (limiti, migrazioni, validazione).
//!
//! Contratto:
//! * `dcf convert <input> <output> [--report <file>]`: la direzione è decisa dal
//!   contenuto dell'input (JSON → `.dcf`, altrimenti → `.ocp`), mai dall'estensione;
//! * `dcf confronta <a> <b>`: un `.dcf` e un `.ocp` in qualunque ordine; verdetto
//!   «PREZZI PRESERVATI» o elenco delle differenze;
//! * `dcf info <file>`: informazioni su un `.dcf`;
//! * uscita 0 riuscito, 1 errore; stderr `errore: <descrizione>` solo in caso di errore;
//!   stdout esito e resoconto in UTF-8.
//!
//! Misure (oltre a quelle del ponte: lista di argomenti, percorsi assoluti, 200 MB,
//! 120 secondi): solo le variabili d'ambiente indispensabili, così il programma non
//! riceve eventuali credenziali dell'ambiente; lavoro in una cartella temporanea privata
//! rimossa alla fine; output letto fino a un limite; file di destinazione scritti in modo
//! atomico solo a conversione riuscita.

use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use opencomputo_core::progetto::Progetto;
use serde::Serialize;

use crate::errore::ErroreApplicazione;

/// Durata massima di un'invocazione (come il ponte di riferimento).
pub const DURATA_MASSIMA: Duration = Duration::from_secs(120);

/// Dimensione massima dei file passati al convertitore (come il ponte: 200 MB).
pub const FILE_MASSIMO: u64 = 200 << 20;

/// Versione dello schema `.ocp` letta e scritta dal convertitore (schema v7).
pub const SCHEMA_DEL_CONVERTITORE: u32 = 7;

/// Verdetto di `confronta` quando prezzi, quantità, importi e SAL coincidono.
pub const VERDETTO_PRESERVATI: &str = "PREZZI PRESERVATI";

/// Byte letti al massimo da stdout, da stderr e dal file del resoconto.
const TESTO_MASSIMO: usize = 1 << 20;

/// Caratteri di ogni testo riportati al chiamante.
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

/// Testi di una conversione.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Resoconto {
    /// Nome del programma usato (senza la cartella).
    pub programma: String,
    /// Resoconto della conversione (trasferimenti, avvisi, note): il file `--report` se
    /// scritto, altrimenti stdout.
    pub testo: String,
    /// Messaggi su stderr, se presenti.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub messaggi: String,
    /// `true` se un testo è stato troncato.
    pub troncato: bool,
}

/// Esito della verifica di fedeltà (`confronta`) dopo una conversione.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Verifica {
    /// `false` se il confronto non è stato possibile (il motivo è in `dettaglio`).
    pub eseguita: bool,
    /// Verdetto «PREZZI PRESERVATI».
    pub preservati: bool,
    /// Uscita del confronto: una riga per voce e l'esito, o il motivo dell'errore.
    pub dettaglio: String,
}

/// Esito di un import riuscito (`.dcf` → progetto).
#[derive(Debug)]
pub struct Importazione {
    /// Progetto prodotto, letto e migrato come un file `.ocp`.
    pub progetto: Progetto,
    /// Resoconto della conversione.
    pub resoconto: Resoconto,
    /// Verifica di fedeltà tra il `.dcf` e il progetto prodotto.
    pub verifica: Verifica,
}

/// Esito di un export riuscito (progetto → `.dcf`).
#[derive(Debug, Clone, Serialize)]
pub struct Esportazione {
    /// File `.dcf` scritto.
    pub file: PathBuf,
    /// Resoconto scritto accanto (`<nome>.report.txt`), se è stato possibile.
    pub file_resoconto: Option<PathBuf>,
    /// Resoconto della conversione.
    pub resoconto: Resoconto,
    /// Verifica di fedeltà tra il progetto esportato e il `.dcf` prodotto.
    pub verifica: Verifica,
}

fn errore(messaggio: impl Into<String>) -> ErroreApplicazione {
    ErroreApplicazione::Importazione {
        messaggio: messaggio.into(),
    }
}

fn errore_export(messaggio: impl Into<String>) -> ErroreApplicazione {
    ErroreApplicazione::Esportazione {
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

/// Percorso assoluto di un file esistente entro il limite (come `_file` del ponte).
fn file_valido(
    percorso: &Path,
    crea_errore: fn(String) -> ErroreApplicazione,
) -> Result<PathBuf, ErroreApplicazione> {
    let p = std::path::absolute(percorso)
        .map_err(|e| crea_errore(format!("{}: {e}", percorso.display())))?;
    let m = std::fs::metadata(&p)
        .map_err(|e| crea_errore(format!("file non trovato: {} ({e})", p.display())))?;
    if !m.is_file() {
        return Err(crea_errore(format!("non è un file: {}", p.display())));
    }
    if m.len() > FILE_MASSIMO {
        return Err(crea_errore(format!(
            "file troppo grande: {} (massimo {} MB)",
            p.display(),
            FILE_MASSIMO >> 20
        )));
    }
    Ok(p)
}

/// Formato dal contenuto, come la CLI: JSON (primo byte `{`) è `.ocp`, altrimenti `.dcf`.
/// Un `.ocp` compresso (gzip) non è riconosciuto come tale dalla CLI.
fn e_json(percorso: &Path) -> bool {
    let mut b = [0u8; 1];
    std::fs::File::open(percorso)
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok_and(|()| b[0] == b'{')
}

fn e_gzip(percorso: &Path) -> bool {
    let mut b = [0u8; 2];
    std::fs::File::open(percorso)
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok_and(|()| b == [0x1f, 0x8b])
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

/// Uscita di un'invocazione.
struct Uscita {
    riuscita: bool,
    codice: String,
    stdout: String,
    stderr: String,
    troncata: bool,
}

impl Uscita {
    /// Il messaggio d'errore del contratto: la prima riga di stderr senza «errore:».
    fn motivo(&self) -> String {
        let m = self
            .stderr
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim().trim_start_matches("errore:").trim().to_owned())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "nessun messaggio".to_owned());
        m.chars().take(500).collect()
    }
}

/// Esegue `programma args…` nella cartella indicata, con ambiente ridotto e durata
/// massima (come `_esegui` del ponte, senza shell).
fn esegui(
    programma: &Path,
    argomenti: &[&OsStr],
    cartella: &Path,
    durata: Duration,
) -> Result<Uscita, ErroreApplicazione> {
    verifica_programma(programma)?;
    let mut comando = Command::new(programma);
    comando
        .args(argomenti)
        .current_dir(cartella)
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
    let (stdout, t1) = testo_pulito(&out, RESOCONTO_MASSIMO);
    let (stderr, t2) = testo_pulito(&err, RESOCONTO_MASSIMO);
    Ok(Uscita {
        riuscita: stato.success(),
        codice: stato
            .code()
            .map_or_else(|| "interrotto".to_owned(), |c| format!("uscita {c}")),
        stdout,
        stderr,
        troncata: t1 || t2 || out_oltre || err_oltre,
    })
}

fn nome_programma(programma: &Path) -> String {
    programma
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `convert input output --report resoconto` nella cartella temporanea; resoconto dal
/// file se scritto, altrimenti da stdout.
fn converti(
    programma: &Path,
    input: &Path,
    output: &Path,
    cartella: &CartellaTemporanea,
    durata: Duration,
    crea_errore: fn(String) -> ErroreApplicazione,
) -> Result<Resoconto, ErroreApplicazione> {
    let file_resoconto = cartella.0.join("resoconto.txt");
    let u = esegui(
        programma,
        &[
            OsStr::new("convert"),
            input.as_os_str(),
            output.as_os_str(),
            OsStr::new("--report"),
            file_resoconto.as_os_str(),
        ],
        &cartella.0,
        durata,
    )?;
    if !u.riuscita {
        return Err(crea_errore(format!(
            "il convertitore non è riuscito ({}): {}",
            u.codice,
            u.motivo()
        )));
    }
    let da_file = std::fs::File::open(&file_resoconto).ok().map(|f| {
        let mut b = Vec::new();
        let _ = f
            .take(u64::try_from(TESTO_MASSIMO).unwrap_or(u64::MAX))
            .read_to_end(&mut b);
        b
    });
    let (testo, troncato) = match da_file {
        Some(b) if !b.is_empty() => testo_pulito(&b, RESOCONTO_MASSIMO),
        _ => (u.stdout, false),
    };
    Ok(Resoconto {
        programma: nome_programma(programma),
        testo,
        messaggi: u.stderr,
        troncato: troncato || u.troncata,
    })
}

/// Verifica di fedeltà tra un `.dcf` e un `.ocp` (`confronta`, come `verifica` del
/// ponte). Non fallisce: un confronto impossibile è riportato in [`Verifica`].
#[must_use]
pub fn confronta(programma: &Path, a: &Path, b: &Path, durata: Duration) -> Verifica {
    let esito = (|| {
        let a = file_valido(a, errore)?;
        let b = file_valido(b, errore)?;
        let cartella = CartellaTemporanea::nuova()?;
        esegui(
            programma,
            &[OsStr::new("confronta"), a.as_os_str(), b.as_os_str()],
            &cartella.0,
            durata,
        )
    })();
    match esito {
        Err(e) => Verifica {
            eseguita: false,
            preservati: false,
            dettaglio: e.to_string(),
        },
        // Con differenze il confronto riporta comunque l'elenco su stdout.
        Ok(u) if u.riuscita || !u.stdout.is_empty() => Verifica {
            eseguita: true,
            preservati: u.riuscita && u.stdout.contains(VERDETTO_PRESERVATI),
            dettaglio: if u.stderr.is_empty() {
                u.stdout
            } else {
                format!("{}\n{}", u.stdout, u.stderr)
            },
        },
        Ok(u) => Verifica {
            eseguita: false,
            preservati: false,
            dettaglio: format!("confronto non riuscito ({}): {}", u.codice, u.motivo()),
        },
    }
}

/// Informazioni su un documento `.dcf` (`info`, come il ponte).
///
/// # Errors
/// File non valido; il convertitore non riesce.
pub fn info(programma: &Path, file: &Path, durata: Duration) -> Result<String, ErroreApplicazione> {
    let file = file_valido(file, errore)?;
    let cartella = CartellaTemporanea::nuova()?;
    let u = esegui(
        programma,
        &[OsStr::new("info"), file.as_os_str()],
        &cartella.0,
        durata,
    )?;
    if !u.riuscita {
        return Err(errore(format!(
            "il convertitore non è riuscito ({}): {}",
            u.codice,
            u.motivo()
        )));
    }
    Ok(u.stdout)
}

/// Converte un `.dcf` in progetto, lo legge come un file `.ocp` qualunque e ne verifica
/// la fedeltà con `confronta`.
///
/// # Errors
/// Programma o origine non validi (anche: l'origine è già un progetto OpenComputo), avvio
/// non riuscito, durata superata, uscita diversa da 0 (con il messaggio del convertitore),
/// risultato non leggibile come progetto OpenComputo.
pub fn importa(
    programma: &Path,
    origine: &Path,
    durata: Duration,
) -> Result<Importazione, ErroreApplicazione> {
    verifica_programma(programma)?;
    let origine = file_valido(origine, errore)?;
    // Regola della direzione: con un .ocp in ingresso la CLI produrrebbe un .dcf.
    if e_json(&origine) || e_gzip(&origine) {
        return Err(errore(format!(
            "{} è già un progetto OpenComputo: aprirlo con «Apri»",
            origine.display()
        )));
    }
    let cartella = CartellaTemporanea::nuova()?;
    let uscita = cartella.0.join("convertito.ocp");
    let resoconto = converti(programma, &origine, &uscita, &cartella, durata, |m| {
        errore(m)
    })?;
    let progetto = opencomputo_storage::apri(&uscita).map_err(|e| {
        errore(format!(
            "il convertitore ha terminato senza errori ma il risultato non è un progetto OpenComputo leggibile: {e}"
        ))
    })?;
    let verifica = confronta(programma, &origine, &uscita, durata);
    Ok(Importazione {
        progetto,
        resoconto,
        verifica,
    })
}

/// Scrive `byte` in `destinazione` in modo atomico (file temporaneo accanto, poi
/// rinomina): un file esistente è sostituito solo a scrittura completata.
fn scrivi_atomico(destinazione: &Path, byte: &[u8]) -> std::io::Result<()> {
    let nome = destinazione
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporaneo = destinazione.with_file_name(format!(".{nome}.{}.tmp", std::process::id()));
    let scrivi = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&temporaneo)?;
        f.write_all(byte)?;
        f.sync_all()?;
        std::fs::rename(&temporaneo, destinazione)
    };
    let esito = scrivi();
    if esito.is_err() {
        let _ = std::fs::remove_file(&temporaneo);
    }
    esito
}

/// Converte il progetto in `.dcf` nella `destinazione` e ne verifica la fedeltà con
/// `confronta`; il resoconto va accanto (`<nome>.report.txt`), come nel ponte.
///
/// # Errors
/// Programma non valido, destinazione non scrivibile, progetto oltre il limite, schema
/// diverso da quello del convertitore, conversione non riuscita o risultato non `.dcf`.
pub fn esporta(
    programma: &Path,
    progetto: &Progetto,
    destinazione: &Path,
    durata: Duration,
) -> Result<Esportazione, ErroreApplicazione> {
    verifica_programma(programma)?;
    if opencomputo_core::schema::VERSIONE_SCHEMA_CORRENTE != SCHEMA_DEL_CONVERTITORE {
        return Err(errore_export(format!(
            "il convertitore legge progetti dello schema {SCHEMA_DEL_CONVERTITORE}, OpenComputo scrive lo schema {}",
            opencomputo_core::schema::VERSIONE_SCHEMA_CORRENTE
        )));
    }
    let destinazione = std::path::absolute(destinazione)
        .map_err(|e| errore_export(format!("{}: {e}", destinazione.display())))?;
    if destinazione.is_dir() {
        return Err(errore_export(format!(
            "{} è una cartella",
            destinazione.display()
        )));
    }
    let testo = opencomputo_core::schema::esporta_json(progetto)
        .map_err(|e| errore_export(e.to_string()))?;
    if u64::try_from(testo.len()).unwrap_or(u64::MAX) > FILE_MASSIMO {
        return Err(errore_export(format!(
            "progetto troppo grande per il convertitore (massimo {} MB)",
            FILE_MASSIMO >> 20
        )));
    }
    let cartella = CartellaTemporanea::nuova()?;
    // JSON in chiaro: la CLI riconosce un .ocp dal primo byte `{`.
    let input = cartella.0.join("progetto.ocp");
    std::fs::write(&input, testo.as_bytes())
        .map_err(|e| errore_export(format!("{}: {e}", input.display())))?;
    let uscita = cartella.0.join("convertito.dcf");
    let resoconto = converti(programma, &input, &uscita, &cartella, durata, |m| {
        errore_export(m)
    })?;
    let dcf = std::fs::metadata(&uscita)
        .ok()
        .filter(|m| m.is_file() && m.len() > 0 && m.len() <= FILE_MASSIMO)
        .and_then(|_| std::fs::read(&uscita).ok())
        .filter(|b| b.first() != Some(&b'{'))
        .ok_or_else(|| {
            errore_export(
                "il convertitore ha terminato senza errori ma non ha prodotto un file .dcf",
            )
        })?;
    let verifica = confronta(programma, &input, &uscita, durata);
    scrivi_atomico(&destinazione, &dcf)
        .map_err(|e| errore_export(format!("{}: {e}", destinazione.display())))?;
    // Il resoconto accanto al file ne conserva la traccia (come il ponte).
    let file_resoconto = destinazione.with_extension("report.txt");
    let file_resoconto = scrivi_atomico(&file_resoconto, resoconto.testo.as_bytes())
        .ok()
        .map(|()| file_resoconto);
    Ok(Esportazione {
        file: destinazione,
        file_resoconto,
        resoconto,
        verifica,
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

    #[test]
    fn formato_dal_contenuto_e_scrittura_atomica() {
        let c = CartellaTemporanea::nuova().unwrap();
        let json = c.0.join("a.dcf");
        std::fs::write(&json, b"{\"formato\":1}").unwrap();
        assert!(e_json(&json));
        let dcf = c.0.join("b.ocp");
        std::fs::write(&dcf, b"AAMVHFSS").unwrap();
        assert!(!e_json(&dcf) && !e_gzip(&dcf));
        let vuoto = c.0.join("vuoto");
        std::fs::write(&vuoto, b"").unwrap();
        assert!(!e_json(&vuoto));
        // Sostituzione atomica, nessun temporaneo rimasto.
        let d = c.0.join("d.dcf");
        std::fs::write(&d, b"vecchio").unwrap();
        scrivi_atomico(&d, b"nuovo").unwrap();
        assert_eq!(std::fs::read(&d).unwrap(), b"nuovo");
        assert_eq!(std::fs::read_dir(&c.0).unwrap().count(), 4);
        // Cartella inesistente: errore, niente file.
        assert!(scrivi_atomico(&c.0.join("manca/x.dcf"), b"1").is_err());
    }

    #[test]
    fn schema_del_convertitore_uguale_a_quello_corrente() {
        // Se lo schema cambia, l'export con il convertitore va rivisto (ADR 0027).
        assert_eq!(
            opencomputo_core::schema::VERSIONE_SCHEMA_CORRENTE,
            SCHEMA_DEL_CONVERTITORE
        );
    }
}
