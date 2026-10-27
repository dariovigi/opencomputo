//! Formato di persistenza versionato e migrazioni esplicite.
//!
//! Il progetto è serializzato in JSON all'interno di un involucro che
//! dichiara il formato e la versione dello schema:
//!
//! ```json
//! { "formato": "opencomputo.progetto", "versione_schema": 1, "progetto": { … } }
//! ```
//!
//! Alla lettura, un documento di versione precedente viene portato alla
//! versione corrente applicando in sequenza le [`Migrazione`] registrate
//! (trasformazioni pure `serde_json::Value → serde_json::Value`). Un documento
//! di versione successiva a quella supportata viene rifiutato invece di essere
//! interpretato in modo parziale.
//!
//! Tutti i valori numerici sono serializzati come stringhe decimali esatte
//! (`"12.50"`), mai come numeri in virgola mobile.
//!
//! Questo modulo è puro (nessun accesso al filesystem): la gestione dei file,
//! delle copie di sicurezza e del versionamento dei documenti spetta al
//! futuro crate `opencomputo-storage`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::progetto::Progetto;

/// Identificativo del formato.
pub const FORMATO: &str = "opencomputo.progetto";
/// Versione corrente dello schema.
pub const VERSIONE_SCHEMA_CORRENTE: u32 = 7;

/// Migrazioni registrate, in ordine di versione di partenza.
pub const MIGRAZIONI: &[Migrazione] = &[
    Migrazione {
        da: 1,
        descrizione: "versione 2: anticipazione, IVA, penali e certificati di pagamento \
                      congelati (campi nuovi e facoltativi; i SAL emessi in precedenza \
                      restano senza certificato congelato e lo ricalcolano)",
        applica: da_1_a_2,
    },
    Migrazione {
        da: 2,
        descrizione: "versione 3: conto finale, collaudo e rata di saldo (campo nuovo e \
                      facoltativo)",
        applica: da_2_a_3,
    },
    Migrazione {
        da: 3,
        descrizione: "versione 4: nuovi prezzi e verbali di concordamento, esclusione dal \
                      ribasso, parametri e righe delle analisi (campi nuovi e facoltativi)",
        applica: da_3_a_4,
    },
    Migrazione {
        da: 4,
        descrizione: "versione 5: variabili e oneri aziendali della sicurezza nelle analisi \
                      (campi nuovi e facoltativi)",
        applica: da_4_a_5,
    },
    Migrazione {
        da: 5,
        descrizione: "versione 6: righi «vedi voce», collegati a una voce del computo o \
                      statici (campo nuovo e facoltativo)",
        applica: da_5_a_6,
    },
    Migrazione {
        da: 6,
        descrizione: "versione 7: cronoprogramma dei lavori (campo nuovo e facoltativo)",
        applica: da_6_a_7,
    },
];

/// Da 1 a 2: solo campi aggiunti e facoltativi, il contenuto resta invariato. La
/// versione cambia perché un programma che conosce solo la 1 perderebbe i dati nuovi
/// risalvando il file: con la 2 lo rifiuta esplicitamente.
#[allow(clippy::unnecessary_wraps)]
fn da_1_a_2(progetto: Value) -> Result<Value, String> {
    Ok(progetto)
}

/// Da 2 a 3: solo il conto finale, facoltativo; il contenuto resta invariato (vedi
/// [`da_1_a_2`] per il motivo del cambio di versione).
#[allow(clippy::unnecessary_wraps)]
fn da_2_a_3(progetto: Value) -> Result<Value, String> {
    Ok(progetto)
}

/// Da 3 a 4: solo campi facoltativi; il contenuto resta invariato (vedi [`da_1_a_2`]).
#[allow(clippy::unnecessary_wraps)]
fn da_3_a_4(progetto: Value) -> Result<Value, String> {
    Ok(progetto)
}

/// Da 4 a 5: solo campi facoltativi; il contenuto resta invariato (vedi [`da_1_a_2`]).
#[allow(clippy::unnecessary_wraps)]
fn da_4_a_5(progetto: Value) -> Result<Value, String> {
    Ok(progetto)
}

/// Da 5 a 6: solo il «vedi voce» dei righi, facoltativo; il contenuto resta invariato
/// (vedi [`da_1_a_2`]).
#[allow(clippy::unnecessary_wraps)]
fn da_5_a_6(progetto: Value) -> Result<Value, String> {
    Ok(progetto)
}

/// Da 6 a 7: solo il cronoprogramma, facoltativo; il contenuto resta invariato (vedi
/// [`da_1_a_2`]).
#[allow(clippy::unnecessary_wraps)]
fn da_6_a_7(progetto: Value) -> Result<Value, String> {
    Ok(progetto)
}

/// Errore di lettura o scrittura del formato.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroreSchema {
    /// JSON non valido o non conforme al modello.
    #[error("documento non valido: {0}")]
    Json(String),
    /// Il documento non dichiara il formato atteso.
    #[error("formato «{0}» non riconosciuto")]
    FormatoSconosciuto(String),
    /// Versione dello schema assente o non numerica.
    #[error("versione dello schema assente o non valida")]
    VersioneMancante,
    /// Documento prodotto da una versione più recente del programma.
    #[error("versione dello schema {trovata} più recente di quella supportata ({supportata})")]
    VersioneFutura {
        /// Versione trovata.
        trovata: u32,
        /// Versione supportata.
        supportata: u32,
    },
    /// Nessuna migrazione disponibile dalla versione indicata.
    #[error("nessuna migrazione disponibile dalla versione {0}")]
    MigrazioneMancante(u32),
    /// Errore durante una migrazione.
    #[error("migrazione dalla versione {da} non riuscita: {messaggio}")]
    Migrazione {
        /// Versione di partenza.
        da: u32,
        /// Messaggio.
        messaggio: String,
    },
}

/// Trasformazione del documento da una versione alla successiva.
#[derive(Debug, Clone, Copy)]
pub struct Migrazione {
    /// Versione di partenza; il risultato ha versione `da + 1`.
    pub da: u32,
    /// Descrizione della modifica.
    pub descrizione: &'static str,
    /// Trasformazione del contenuto del campo `progetto`.
    pub applica: fn(Value) -> Result<Value, String>,
}

#[derive(Serialize)]
struct InvolucroScrittura<'a> {
    formato: &'a str,
    versione_schema: u32,
    progetto: &'a Progetto,
}

#[derive(Deserialize)]
struct InvolucroLettura {
    formato: String,
    versione_schema: u32,
    progetto: Value,
}

/// Serializza il progetto nel formato corrente (JSON indentato).
///
/// # Errors
/// [`ErroreSchema::Json`] in caso di errore di serializzazione.
pub fn esporta_json(progetto: &Progetto) -> Result<String, ErroreSchema> {
    serde_json::to_string_pretty(&InvolucroScrittura {
        formato: FORMATO,
        versione_schema: VERSIONE_SCHEMA_CORRENTE,
        progetto,
    })
    .map_err(|e| ErroreSchema::Json(e.to_string()))
}

/// Legge un progetto, applicando le migrazioni necessarie.
///
/// # Errors
/// Formato non riconosciuto, versione non supportata, migrazione non riuscita
/// o documento non conforme al modello.
pub fn importa_json(testo: &str) -> Result<Progetto, ErroreSchema> {
    importa_json_con(testo, MIGRAZIONI, VERSIONE_SCHEMA_CORRENTE)
}

/// Come [`importa_json`], con migrazioni e versione corrente esplicite (per i test).
///
/// # Errors
/// Vedi [`importa_json`].
pub fn importa_json_con(
    testo: &str,
    migrazioni: &[Migrazione],
    versione_corrente: u32,
) -> Result<Progetto, ErroreSchema> {
    let grezzo: Value =
        serde_json::from_str(testo).map_err(|e| ErroreSchema::Json(e.to_string()))?;
    match grezzo.get("versione_schema") {
        Some(v) if v.as_u64().and_then(|n| u32::try_from(n).ok()).is_some() => {}
        _ => return Err(ErroreSchema::VersioneMancante),
    }
    let involucro: InvolucroLettura =
        serde_json::from_value(grezzo).map_err(|e| ErroreSchema::Json(e.to_string()))?;
    if involucro.formato != FORMATO {
        return Err(ErroreSchema::FormatoSconosciuto(involucro.formato));
    }
    let progetto = migra(
        involucro.progetto,
        involucro.versione_schema,
        migrazioni,
        versione_corrente,
    )?;
    serde_json::from_value(progetto).map_err(|e| ErroreSchema::Json(e.to_string()))
}

/// Porta il contenuto di un progetto dalla versione `da` alla versione `a`.
///
/// # Errors
/// Versione futura, migrazione mancante o non riuscita.
pub fn migra(
    mut progetto: Value,
    da: u32,
    migrazioni: &[Migrazione],
    a: u32,
) -> Result<Value, ErroreSchema> {
    if da > a {
        return Err(ErroreSchema::VersioneFutura {
            trovata: da,
            supportata: a,
        });
    }
    let mut versione = da;
    while versione < a {
        let m = migrazioni
            .iter()
            .find(|m| m.da == versione)
            .ok_or(ErroreSchema::MigrazioneMancante(versione))?;
        progetto = (m.applica)(progetto).map_err(|messaggio| ErroreSchema::Migrazione {
            da: versione,
            messaggio,
        })?;
        versione += 1;
    }
    Ok(progetto)
}
