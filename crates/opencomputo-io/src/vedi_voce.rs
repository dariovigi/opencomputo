//! Testo dei righi «vedi voce» nelle stampe (ADR 0019).

use opencomputo_core::computo::Computo;
use opencomputo_core::misura::{QuantitaRiferite, VediVoce};

use crate::formato;

/// Testo del riferimento: «Vedi voce n. 12 [m² 125,40]» per una voce collegata,
/// «Vedi voce n. 1 «A.01» del computo «Variante» [10,00]» per una statica.
pub(crate) fn testo(
    v: &VediVoce,
    computo: Option<&Computo>,
    riferite: &dyn QuantitaRiferite,
    decimali: u32,
) -> String {
    match v {
        VediVoce::Statica {
            quantita,
            riferimento,
        } => {
            let q = formato::numero(*quantita, decimali);
            if riferimento.trim().is_empty() {
                format!("Vedi voce [{q}]")
            } else {
                format!("Vedi {} [{q}]", riferimento.trim())
            }
        }
        VediVoce::Collegata(id) => {
            let trovata =
                computo.and_then(|c| c.voci.iter().enumerate().find(|(_, x)| x.id == *id));
            let Some((i, voce)) = trovata else {
                return "Vedi voce (inesistente)".into();
            };
            let um = voce.prezzo.unita_misura.trim();
            match riferite.quantita(*id) {
                Ok(q) => {
                    let q = formato::numero(q, decimali);
                    if um.is_empty() {
                        format!("Vedi voce n. {} [{q}]", i + 1)
                    } else {
                        format!("Vedi voce n. {} [{um} {q}]", i + 1)
                    }
                }
                Err(_) => format!("Vedi voce n. {}", i + 1),
            }
        }
    }
}

/// Descrizione del rigo preceduta dal testo del «vedi voce», se c'è.
pub(crate) fn con_descrizione(vedi: Option<String>, descrizione: &str) -> String {
    let d = descrizione.trim();
    match vedi {
        None => d.to_owned(),
        Some(v) if d.is_empty() => v,
        Some(v) => format!("{v} — {d}"),
    }
}
