//! Anticipazione del prezzo, penali e certificato di pagamento.
//!
//! ## Certificato di pagamento di un SAL
//!
//! ```text
//! importo del SAL             = dal riepilogo del SAL (credito del periodo, già al netto
//!                               della ritenuta di garanzia)
//! recupero dell'anticipazione = min(anticipazione, % recupero × importo netto progressivo)
//!                               − anticipazione già recuperata              (arrotondato)
//! penali                      = Σ penali applicate in questo certificato
//! imponibile                  = importo del SAL − recupero − penali
//! IVA                         = imponibile × aliquota                       (arrotondata)
//! totale del certificato      = imponibile + IVA
//! ```
//!
//! Il recupero è calcolato sul **progressivo** e il certificato recupera la differenza
//! con quanto già recuperato, come il ribasso e la ritenuta nei SAL: gli arrotondamenti
//! non si accumulano e, dopo un SAL con il recupero sospeso, il recupero riprende
//! allineandosi alla proporzione prevista. Un SAL può anche recuperare tutto il residuo
//! (di norma l'ultimo).
//!
//! L'art. 125 del D.Lgs. 36/2023 fissa l'anticipazione al 20 % del valore del contratto
//! (fino al 30 % se previsto dai documenti di gara) ma rimette al contratto le modalità
//! di recupero: la percentuale di recupero è quindi un parametro, per impostazione
//! predefinita uguale a quella dell'anticipazione. Le penali per ritardo (art. 126:
//! dallo 0,5 all'1,5 per mille giornaliero dell'ammontare netto contrattuale, al massimo
//! il 10 % complessivo) sono importi registrati con la loro data e applicati nel primo
//! certificato il cui SAL arriva a quella data. I limiti sono verificati dalla
//! validazione, non imposti dal calcolo.

use jiff::civil::Date;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::id::PenaleId;
use crate::numeri::{Importo, Percentuale};

/// Anticipazione del prezzo erogata all'esecutore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anticipazione {
    /// Percentuale del valore del contratto (di norma 20 %).
    pub percentuale: Percentuale,
    /// Importo erogato.
    pub importo: Importo,
    /// Percentuale dell'importo netto progressivo recuperata nei certificati.
    pub percentuale_recupero: Percentuale,
    /// Data di erogazione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_erogazione: Option<Date>,
}

/// Penale applicata all'esecutore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Penale {
    /// Identificativo.
    pub id: PenaleId,
    /// Numero d'ordine (1, 2, …).
    pub numero: u32,
    /// Data dell'applicazione: la penale va nel primo certificato il cui SAL arriva a
    /// questa data.
    pub data: Date,
    /// Motivo.
    pub descrizione: String,
    /// Importo.
    pub importo: Importo,
    /// Giorni di ritardo, se calcolata come penale per ritardo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub giorni: Option<u32>,
    /// Misura giornaliera in millesimi dell'ammontare netto contrattuale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_mille: Option<Decimal>,
}

/// Certificato di pagamento di un SAL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificatoPagamento {
    /// Numero (uguale a quello del SAL).
    pub numero: u32,
    /// Importo del SAL (credito del periodo al netto della ritenuta).
    pub importo_sal: Importo,
    /// Anticipazione erogata (zero se non prevista).
    pub anticipazione: Importo,
    /// Anticipazione recuperata nei certificati precedenti.
    pub anticipazione_recuperata_prima: Importo,
    /// Recupero dell'anticipazione in questo certificato.
    pub recupero_anticipazione: Importo,
    /// Anticipazione ancora da recuperare dopo questo certificato.
    pub anticipazione_residua: Importo,
    /// `true` se il recupero è stato sospeso per questo certificato.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recupero_sospeso: bool,
    /// Penali applicate.
    pub penali: Importo,
    /// Imponibile.
    pub imponibile: Importo,
    /// Aliquota IVA (assente se non indicata nel contratto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliquota_iva: Option<Percentuale>,
    /// IVA (zero senza aliquota).
    pub iva: Importo,
    /// Totale del certificato.
    pub totale: Importo,
}
