//! Stati di avanzamento lavori (SAL).
//!
//! ## Calcolo (valori progressivi, cioè cumulati dal primo SAL)
//!
//! ```text
//! lavori lordi            = Σ importi delle registrazioni di natura «lavoro»
//! oneri sicurezza         = Σ importi delle registrazioni di natura «oneri sicurezza»
//! manodopera esclusa      = Σ quote di manodopera dei lavori (solo se l'opzione è attiva)
//! lavori esclusi          = Σ importi dei nuovi prezzi non soggetti a ribasso
//! soggetto a ribasso      = lavori lordi − manodopera esclusa − lavori esclusi
//! ribasso                 = soggetto a ribasso × ribasso %                 (arrotondato)
//! importo netto           = lavori lordi + oneri sicurezza − ribasso
//! ritenuta di garanzia    = importo netto × ritenuta %                     (arrotondata)
//! credito progressivo     = importo netto − ritenuta
//! importo del SAL         = credito progressivo − credito del SAL precedente (emesso)
//! ```
//!
//! Il ribasso e la ritenuta sono calcolati sui **progressivi** e l'importo del
//! singolo SAL è la differenza con il precedente: gli arrotondamenti non si
//! accumulano tra un SAL e l'altro.
//!
//! Anticipazione, penali, IVA e certificato di pagamento sono nel modulo
//! [`certificato`](super::certificato). Non sono ancora gestiti: lavori a corpo
//! contabilizzati a percentuale, revisione prezzi. Le aliquote sono parametri del
//! contratto: il core non impone valori normativi.

use jiff::civil::Date;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::certificato::{Anticipazione, CertificatoPagamento};
use crate::id::{PenaleId, RegistrazioneId, SalId};
use crate::numeri::{Importo, Percentuale};

/// Parametri contrattuali che influenzano la contabilità.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParametriContabili {
    /// Ribasso d'asta offerto.
    pub ribasso: Percentuale,
    /// Ritenuta di garanzia sull'importo netto progressivo.
    pub ritenuta_garanzia: Percentuale,
    /// Se `true`, la quota di manodopera dei lavori è esclusa dal ribasso.
    #[serde(default)]
    pub manodopera_esclusa_dal_ribasso: bool,
    /// Aliquota IVA dei certificati di pagamento (assente: certificati senza IVA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliquota_iva: Option<Percentuale>,
    /// Anticipazione del prezzo, se erogata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anticipazione: Option<Anticipazione>,
}

impl ParametriContabili {
    /// Ritenuta di garanzia predefinita: 0,50 %. È solo un valore iniziale
    /// modificabile: l'aliquota applicabile va verificata sul contratto.
    #[must_use]
    pub fn ritenuta_predefinita() -> Percentuale {
        Percentuale::new(Decimal::new(50, 2))
    }
}

impl Default for ParametriContabili {
    fn default() -> Self {
        Self {
            ribasso: Percentuale::ZERO,
            ritenuta_garanzia: Self::ritenuta_predefinita(),
            manodopera_esclusa_dal_ribasso: false,
            aliquota_iva: None,
            anticipazione: None,
        }
    }
}

/// Stato di un SAL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatoSal {
    /// In preparazione: le registrazioni incluse possono ancora cambiare.
    Bozza,
    /// Emesso: il riepilogo è congelato e le registrazioni incluse sono bloccate.
    Emesso,
}

/// Stato di avanzamento lavori.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatoAvanzamentoLavori {
    /// Identificativo.
    pub id: SalId,
    /// Numero progressivo (1, 2, …).
    pub numero: u32,
    /// Lavori contabilizzati a tutto il giorno indicato.
    pub lavori_a_tutto_il: Date,
    /// Data di emissione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_emissione: Option<Date>,
    /// Stato.
    pub stato: StatoSal,
    /// Registrazioni del libretto contabilizzate in questo SAL.
    pub registrazioni: Vec<RegistrazioneId>,
    /// Riepilogo congelato all'emissione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub riepilogo_emesso: Option<RiepilogoSal>,
    /// Certificato di pagamento congelato all'emissione (assente nei SAL emessi con
    /// versioni precedenti: in quel caso è ricalcolato).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificato_emesso: Option<CertificatoPagamento>,
    /// Penali applicate in questo certificato, fissate all'emissione.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub penali: Vec<PenaleId>,
    /// Recupero dell'anticipazione sospeso in questo certificato.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recupero_anticipazione_sospeso: bool,
    /// Recupera in questo certificato tutta l'anticipazione residua (di norma
    /// nell'ultimo SAL).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recupera_residuo_anticipazione: bool,
    /// Note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Riepilogo economico di un SAL. Tutti i valori sono progressivi, salvo
/// `lordo_periodo` e `importo_sal`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiepilogoSal {
    /// Numero del SAL.
    pub numero: u32,
    /// Importo lordo (lavori + sicurezza) delle sole registrazioni del periodo.
    pub lordo_periodo: Importo,
    /// Lavori lordi progressivi.
    pub lavori_lordi: Importo,
    /// Oneri della sicurezza progressivi.
    pub oneri_sicurezza: Importo,
    /// Manodopera esclusa dal ribasso.
    pub manodopera_esclusa: Importo,
    /// Lavori a nuovi prezzi già netti, esclusi dal ribasso.
    #[serde(default, skip_serializing_if = "Importo::zero_ref")]
    pub lavori_esclusi_dal_ribasso: Importo,
    /// Importo soggetto a ribasso.
    pub soggetto_a_ribasso: Importo,
    /// Ribasso.
    pub ribasso: Importo,
    /// Importo netto progressivo.
    pub importo_netto: Importo,
    /// Ritenuta di garanzia progressiva.
    pub ritenuta_garanzia: Importo,
    /// Credito progressivo dell'impresa.
    pub credito_progressivo: Importo,
    /// Credito progressivo del SAL precedente.
    pub certificati_precedenti: Importo,
    /// Importo del presente SAL.
    pub importo_sal: Importo,
}
