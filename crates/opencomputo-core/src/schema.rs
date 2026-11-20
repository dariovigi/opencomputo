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

/// Documento della versione corrente, letto direttamente nel modello: `Some` solo se
/// `formato` e `versione_schema` (corrente) precedono `progetto`, come nei file scritti
/// da [`esporta_json`]. In ogni altro caso la lettura si ferma appena possibile.
struct Corrente(Option<Progetto>);

impl<'de> Deserialize<'de> for Corrente {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitatore;
        impl<'de> serde::de::Visitor<'de> for Visitatore {
            type Value = Corrente;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("un documento OpenComputo")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<Corrente, A::Error> {
                use serde::de::{Error, IgnoredAny};
                let (mut formato, mut versione, mut progetto) = (false, false, None);
                while let Some(chiave) = m.next_key::<std::borrow::Cow<'de, str>>()? {
                    match chiave.as_ref() {
                        "formato" if !formato => formato = m.next_value::<String>()? == FORMATO,
                        "versione_schema" if !versione => {
                            versione = m.next_value::<u32>()? == VERSIONE_SCHEMA_CORRENTE;
                        }
                        "progetto" if formato && versione && progetto.is_none() => {
                            progetto = Some(m.next_value::<Progetto>()?);
                        }
                        "progetto" | "formato" | "versione_schema" => {
                            return Err(A::Error::custom("non è un documento corrente"));
                        }
                        _ => {
                            m.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(Corrente(progetto.filter(|_| formato && versione)))
            }
        }
        d.deserialize_map(Visitatore)
    }
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
    // File della versione corrente (il caso comune): lettura diretta nel modello, senza
    // l'albero JSON intermedio che serve solo alle migrazioni. Ogni altro caso, errori
    // compresi, segue la strada generale, con gli stessi messaggi di sempre.
    if versione_corrente == VERSIONE_SCHEMA_CORRENTE
        && let Ok(Corrente(Some(progetto))) = serde_json::from_str::<Corrente>(testo)
    {
        return Ok(progetto);
    }
    importa_generale(testo, migrazioni, versione_corrente)
}

/// Testo massimo per la lettura generale, che costruisce l'albero JSON (5–6 volte la
/// memoria del testo). Serve ai documenti di versioni precedenti, piccoli nella pratica;
/// quelli della versione corrente passano dalla lettura diretta, senza questo limite.
pub const TESTO_MASSIMO_MIGRAZIONI: usize = 256 << 20;

/// Lettura generale: albero JSON, controllo dell'involucro, migrazioni.
fn importa_generale(
    testo: &str,
    migrazioni: &[Migrazione],
    versione_corrente: u32,
) -> Result<Progetto, ErroreSchema> {
    if testo.len() > TESTO_MASSIMO_MIGRAZIONI {
        return Err(ErroreSchema::Json(format!(
            "documento di {} byte: oltre {TESTO_MASSIMO_MIGRAZIONI} byte si leggono solo \
             i documenti della versione {VERSIONE_SCHEMA_CORRENTE}",
            testo.len()
        )));
    }
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

#[cfg(test)]
mod prove {
    use super::*;

    const V7: &str = include_str!("../tests/fixtures/progetto_esempio_v7.json");
    const V3: &str = include_str!("../tests/fixtures/progetto_esempio_v3.json");

    fn generale(t: &str) -> Result<Progetto, ErroreSchema> {
        importa_generale(t, MIGRAZIONI, VERSIONE_SCHEMA_CORRENTE)
    }

    /// La lettura diretta dà lo stesso risultato (o lo stesso errore) della generale.
    #[test]
    fn lettura_diretta_uguale_alla_generale() {
        let riscritto = esporta_json(&importa_json(V7).unwrap()).unwrap();
        let mut casi: Vec<String> = vec![V7.into(), V3.into(), riscritto.clone()];
        // Chiavi dell'involucro in altro ordine, ripetute, mancanti, sbagliate.
        let corpo = riscritto
            .split_once("\"progetto\":")
            .map(|(_, c)| c.trim_end().trim_end_matches('}').to_owned())
            .unwrap();
        for involucro in [
            format!(
                "{{\"progetto\": {corpo}, \"formato\": \"{FORMATO}\", \"versione_schema\": 7}}"
            ),
            format!(
                "{{\"versione_schema\": 7, \"formato\": \"{FORMATO}\", \"altro\": [1, {{}}], \"progetto\": {corpo}}}"
            ),
            format!(
                "{{\"formato\": \"{FORMATO}\", \"versione_schema\": 7, \"versione_schema\": 6, \"progetto\": {corpo}}}"
            ),
            format!(
                "{{\"formato\": \"{FORMATO}\", \"versione_schema\": 7, \"progetto\": {corpo}, \"progetto\": {{}}}}"
            ),
            format!("{{\"formato\": \"altro\", \"versione_schema\": 7, \"progetto\": {corpo}}}"),
            format!(
                "{{\"formato\": \"{FORMATO}\", \"versione_schema\": 8, \"progetto\": {corpo}}}"
            ),
            format!("{{\"formato\": \"{FORMATO}\", \"progetto\": {corpo}}}"),
            format!(
                "{{\"formato\": \"{FORMATO}\", \"versione_schema\": \"7\", \"progetto\": {corpo}}}"
            ),
            format!(
                "{{\"formato\": \"{FORMATO}\", \"versione_schema\": 7, \"progetto\": {{\"id\": 1}}}}"
            ),
            format!("{{\"formato\": \"{FORMATO}\", \"versione_schema\": 7, \"progetto\": {corpo}"),
            "[]".into(),
            String::new(),
        ] {
            casi.push(involucro);
        }
        for (i, t) in casi.iter().enumerate() {
            let (a, b) = (importa_json(t), generale(t));
            match (&a, &b) {
                (Ok(x), Ok(y)) => assert_eq!(x, y, "caso {i}"),
                (Err(x), Err(y)) => assert_eq!(x.to_string(), y.to_string(), "caso {i}"),
                _ => panic!("caso {i}: {:?} / {:?}", a.as_ref().err(), b.as_ref().err()),
            }
        }
    }
}
