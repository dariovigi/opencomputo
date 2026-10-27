//! Provenienza dei dati importati e anomalie rilevate.
//!
//! Ogni entità importata da un formato esterno può conservare: il formato e il
//! file di origine, l'impronta del file, l'identificativo originale, le
//! anomalie rilevate durante l'importazione e i dati originali non mappati
//! sul modello interno (necessari per ricostruire, quando possibile, il
//! documento di origine).

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::validazione::Gravita;

/// Formato di origine dei dati.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatoOrigine {
    /// Inserimento manuale.
    Manuale,
    /// Formato nativo di OpenComputo.
    OpenComputo,
    /// XPWE (formato XML di interscambio dell'ecosistema PriMus).
    Xpwe,
    /// DCF (prezzari e documenti dell'ecosistema PriMus; sola lettura dei contenuti non protetti).
    Dcf,
    /// Foglio di calcolo XLSX.
    Xlsx,
    /// CSV.
    Csv,
    /// Altro formato.
    Altro(String),
}

/// Anomalia rilevata durante l'importazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anomalia {
    /// Gravità.
    pub gravita: Gravita,
    /// Codice stabile dell'anomalia (es. `xpwe.campo_sconosciuto`).
    pub codice: String,
    /// Descrizione leggibile.
    pub messaggio: String,
    /// Posizione nel file di origine (riga, percorso XML, offset…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posizione: Option<String>,
}

/// Provenienza di un'entità.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenienza {
    /// Formato di origine.
    pub formato: FormatoOrigine,
    /// Nome del file di origine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Impronta SHA-256 (esadecimale) del file di origine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impronta_sha256: Option<String>,
    /// Momento dell'importazione (ora locale dichiarata dal chiamante).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importato_il: Option<NaiveDateTime>,
    /// Identificativo dell'entità nel documento di origine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identificativo_originale: Option<String>,
    /// Anomalie rilevate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anomalie: Vec<Anomalia>,
    /// Dati originali non mappati sul modello interno (chiave → valore testuale).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dati_originali: BTreeMap<String, String>,
}

impl Provenienza {
    /// Nuova provenienza con il solo formato.
    #[must_use]
    pub fn new(formato: FormatoOrigine) -> Self {
        Self {
            formato,
            file: None,
            impronta_sha256: None,
            importato_il: None,
            identificativo_originale: None,
            anomalie: Vec::new(),
            dati_originali: BTreeMap::new(),
        }
    }
}
