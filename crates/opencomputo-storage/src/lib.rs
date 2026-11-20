//! # opencomputo-storage
//!
//! File di progetto di OpenComputo (estensione consigliata `.ocp`).
//!
//! * **Contenuto**: il documento JSON versionato di [`opencomputo_core::schema`],
//!   leggibile da qualsiasi editor; facoltativamente compresso con gzip (riconosciuto
//!   in lettura dai byte iniziali, qualunque sia l'estensione).
//! * **Salvataggio atomico**: il file viene scritto accanto alla destinazione con un
//!   nome temporaneo, sincronizzato su disco e poi rinominato sopra la destinazione.
//!   Un'interruzione lascia il file precedente intatto, mai uno troncato.
//! * **Verifica prima di scrivere**: il contenuto prodotto viene riletto e confrontato
//!   con il progetto in memoria; se non coincide il salvataggio è annullato.
//! * **Copie di sicurezza**: prima di sostituire un file esistente ne viene copiata la
//!   versione precedente in `nome.ocp.bak1`; le copie più vecchie scalano a `bak2`,
//!   `bak3`… fino al numero richiesto.
//!
//! Le funzioni [`codifica`] e [`decodifica`] non usano il filesystem e funzionano anche
//! in WebAssembly; [`salva`] e [`apri`] usano `std::fs`.

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::schema::{self, ErroreSchema};
use thiserror::Error;

/// Estensione consigliata dei file di progetto.
pub const ESTENSIONE: &str = "ocp";
/// Byte iniziali di un flusso gzip.
const MAGIA_GZIP: [u8; 2] = [0x1f, 0x8b];
/// Dimensione massima del documento, compresso o no (protezione da file «bomba»). Un
/// progetto di 20.000 voci con 100.000 righi occupa circa 115 MB: il limite lascia
/// margine ai progetti reali e impedisce che un file di pochi MB, decompresso, esaurisca
/// la memoria del computer.
pub const DIMENSIONE_MASSIMA: u64 = 512 << 20;

/// Errore di lettura o scrittura di un file di progetto.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreStorage {
    /// Errore del filesystem.
    #[error("{operazione} «{percorso}»: {messaggio}")]
    Io {
        /// Operazione tentata.
        operazione: &'static str,
        /// Percorso coinvolto.
        percorso: String,
        /// Messaggio del sistema.
        messaggio: String,
    },
    /// Il contenuto non è un progetto valido (o è di una versione futura).
    #[error("progetto non valido: {0}")]
    Schema(#[from] ErroreSchema),
    /// Il file compresso è danneggiato o non è testo UTF-8.
    #[error("file non leggibile: {0}")]
    Contenuto(String),
    /// La rilettura del contenuto prodotto non coincide con il progetto: nulla è stato scritto.
    #[error(
        "verifica del salvataggio non riuscita: il contenuto riletto non coincide con il progetto; il file non è stato modificato"
    )]
    VerificaFallita,
}

fn io(operazione: &'static str, percorso: &Path, e: &std::io::Error) -> ErroreStorage {
    ErroreStorage::Io {
        operazione,
        percorso: percorso.display().to_string(),
        messaggio: e.to_string(),
    }
}

/// Compressione del file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compressione {
    /// JSON in chiaro (predefinito).
    #[default]
    Nessuna,
    /// JSON compresso con gzip: utile per progetti con prezzari di decine di migliaia di voci.
    Gzip,
}

/// Opzioni di salvataggio.
#[derive(Debug, Clone)]
pub struct OpzioniSalvataggio {
    /// Compressione.
    pub compressione: Compressione,
    /// Copie di sicurezza da conservare (0 = nessuna).
    pub copie_di_sicurezza: usize,
    /// Rilegge il contenuto prodotto e lo confronta con il progetto prima di scrivere.
    pub verifica: bool,
}

impl Default for OpzioniSalvataggio {
    fn default() -> Self {
        Self {
            compressione: Compressione::Nessuna,
            copie_di_sicurezza: 3,
            verifica: true,
        }
    }
}

/// Esito di un salvataggio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsitoSalvataggio {
    /// Byte scritti.
    pub byte: usize,
    /// Copia di sicurezza creata, se il file esisteva.
    pub copia_di_sicurezza: Option<PathBuf>,
}

/// Contenuto del file per il progetto (senza filesystem).
///
/// # Errors
/// Errori di serializzazione.
pub fn codifica(progetto: &Progetto, compressione: Compressione) -> Result<Vec<u8>, ErroreStorage> {
    let mut json = schema::esporta_json(progetto)?;
    json.push('\n');
    match compressione {
        Compressione::Nessuna => Ok(json.into_bytes()),
        Compressione::Gzip => {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            e.write_all(json.as_bytes())
                .and_then(|()| e.finish())
                .map_err(|x| ErroreStorage::Contenuto(x.to_string()))
        }
    }
}

/// Progetto dal contenuto del file (JSON in chiaro o compresso con gzip), con le
/// migrazioni dello schema applicate.
///
/// # Errors
/// File compresso danneggiato, testo non UTF-8, progetto non valido o di versione futura.
pub fn decodifica(byte: &[u8]) -> Result<Progetto, ErroreStorage> {
    decodifica_entro(byte, DIMENSIONE_MASSIMA)
}

fn decodifica_entro(byte: &[u8], massimo: u64) -> Result<Progetto, ErroreStorage> {
    let oltre = || ErroreStorage::Contenuto(format!("documento oltre {massimo} byte"));
    let testo = if byte.starts_with(&MAGIA_GZIP) {
        let mut s = String::new();
        GzDecoder::new(byte)
            .take(massimo.saturating_add(1))
            .read_to_string(&mut s)
            .map_err(|e| ErroreStorage::Contenuto(format!("gzip: {e}")))?;
        if s.len() as u64 > massimo {
            return Err(oltre());
        }
        s
    } else {
        if byte.len() as u64 > massimo {
            return Err(oltre());
        }
        let t = byte.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(byte);
        // Senza copiare il contenuto: il testo resta nel buffer letto dal file.
        let testo = std::str::from_utf8(t)
            .map_err(|e| ErroreStorage::Contenuto(format!("il file non è testo UTF-8: {e}")))?;
        return Ok(schema::importa_json(testo)?);
    };
    Ok(schema::importa_json(&testo)?)
}

/// Apre un file di progetto.
///
/// # Errors
/// Errori del filesystem o di [`decodifica`].
pub fn apri(percorso: &Path) -> Result<Progetto, ErroreStorage> {
    // Prima della lettura: un file enorme non viene caricato in memoria.
    let dimensione = fs::metadata(percorso)
        .map_err(|e| io("lettura di", percorso, &e))?
        .len();
    if dimensione > DIMENSIONE_MASSIMA {
        return Err(ErroreStorage::Contenuto(format!(
            "{}: file di {dimensione} byte, oltre il massimo di {DIMENSIONE_MASSIMA}",
            percorso.display()
        )));
    }
    let byte = fs::read(percorso).map_err(|e| io("lettura di", percorso, &e))?;
    decodifica(&byte)
}

fn con_suffisso(percorso: &Path, suffisso: &str) -> PathBuf {
    let mut nome: OsString = percorso.as_os_str().to_owned();
    nome.push(suffisso);
    PathBuf::from(nome)
}

/// Percorso della copia di sicurezza n-esima (`nome.ocp.bak1`, …).
#[must_use]
pub fn copia_di_sicurezza(percorso: &Path, n: usize) -> PathBuf {
    con_suffisso(percorso, &format!(".bak{n}"))
}

fn sincronizza_cartella(cartella: &Path) {
    // Su Unix la rinomina è durevole solo dopo la sincronizzazione della cartella.
    // Su altri sistemi aprire una cartella può non essere consentito: ignorato.
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(cartella) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = cartella;
}

/// Salva il progetto in modo atomico, con verifica e copie di sicurezza.
///
/// # Errors
/// Errori di serializzazione, verifica non riuscita (nulla viene scritto), errori del
/// filesystem (il file precedente resta intatto).
pub fn salva(
    percorso: &Path,
    progetto: &Progetto,
    opzioni: &OpzioniSalvataggio,
) -> Result<EsitoSalvataggio, ErroreStorage> {
    let contenuto = codifica(progetto, opzioni.compressione)?;
    if opzioni.verifica && decodifica(&contenuto)? != *progetto {
        return Err(ErroreStorage::VerificaFallita);
    }
    let cartella = match percorso.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let temporaneo = con_suffisso(percorso, &format!(".tmp{}", std::process::id()));
    let scrivi = || -> Result<(), ErroreStorage> {
        let mut f =
            fs::File::create(&temporaneo).map_err(|e| io("creazione di", &temporaneo, &e))?;
        f.write_all(&contenuto)
            .map_err(|e| io("scrittura di", &temporaneo, &e))?;
        f.sync_all()
            .map_err(|e| io("sincronizzazione di", &temporaneo, &e))
    };
    if let Err(e) = scrivi() {
        let _ = fs::remove_file(&temporaneo);
        return Err(e);
    }

    let mut copia = None;
    if opzioni.copie_di_sicurezza > 0 && percorso.is_file() {
        for n in (1..opzioni.copie_di_sicurezza).rev() {
            let da = copia_di_sicurezza(percorso, n);
            if da.is_file() {
                let a = copia_di_sicurezza(percorso, n + 1);
                if let Err(e) = fs::rename(&da, &a) {
                    let _ = fs::remove_file(&temporaneo);
                    return Err(io("rinomina della copia", &da, &e));
                }
            }
        }
        let prima = copia_di_sicurezza(percorso, 1);
        if let Err(e) = fs::copy(percorso, &prima) {
            let _ = fs::remove_file(&temporaneo);
            return Err(io("copia di sicurezza di", percorso, &e));
        }
        copia = Some(prima);
    }
    if let Err(e) = fs::rename(&temporaneo, percorso) {
        let _ = fs::remove_file(&temporaneo);
        return Err(io("sostituzione di", percorso, &e));
    }
    sincronizza_cartella(&cartella);
    Ok(EsitoSalvataggio {
        byte: contenuto.len(),
        copia_di_sicurezza: copia,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffissi() {
        let p = Path::new("/tmp/lavori.ocp");
        assert_eq!(
            copia_di_sicurezza(p, 2),
            PathBuf::from("/tmp/lavori.ocp.bak2")
        );
        assert_eq!(
            con_suffisso(p, ".tmp1"),
            PathBuf::from("/tmp/lavori.ocp.tmp1")
        );
    }

    /// File «bomba»: pochi KB che decompressi supererebbero il limite. La lettura si
    /// ferma al limite, senza decomprimere il resto.
    #[test]
    fn file_compresso_oltre_il_limite() {
        let mut e = GzEncoder::new(Vec::new(), Compression::best());
        for _ in 0..64 {
            e.write_all(&[b' '; 1 << 16]).unwrap();
        }
        let bomba = e.finish().unwrap();
        assert!(bomba.len() < 16 << 10, "{}", bomba.len());
        let errore = decodifica_entro(&bomba, 1 << 20).unwrap_err();
        assert!(
            errore.to_string().contains("oltre 1048576 byte"),
            "{errore}"
        );
        // Sotto il limite il contenuto viene letto (e qui rifiutato perché non è JSON).
        let errore = decodifica_entro(&bomba, 8 << 20).unwrap_err();
        assert!(!errore.to_string().contains("oltre"), "{errore}");
        // Lo stesso per un documento non compresso.
        let testo = vec![b' '; 2 << 20];
        assert!(
            decodifica_entro(&testo, 1 << 20)
                .unwrap_err()
                .to_string()
                .contains("oltre")
        );
    }

    /// Un file più grande del limite non viene neppure letto.
    #[test]
    fn file_troppo_grande_non_letto() {
        let d = std::env::temp_dir().join(format!("opencomputo-grande-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        let f = d.join("enorme.ocp");
        // File sparso: occupa la dimensione dichiarata senza usare disco.
        fs::File::create(&f)
            .unwrap()
            .set_len(DIMENSIONE_MASSIMA + 1)
            .unwrap();
        let inizio = std::time::Instant::now();
        let errore = apri(&f).unwrap_err();
        assert!(errore.to_string().contains("oltre il massimo"), "{errore}");
        assert!(inizio.elapsed() < std::time::Duration::from_secs(1));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn contenuto_non_valido() {
        assert!(matches!(
            decodifica(&[0xff, 0xfe, 0x00]),
            Err(ErroreStorage::Contenuto(_))
        ));
        assert!(matches!(
            decodifica(&[0x1f, 0x8b, 0x08, 0x00]),
            Err(ErroreStorage::Contenuto(_))
        ));
        assert!(matches!(decodifica(b"{}"), Err(ErroreStorage::Schema(_))));
    }
}
