//! Sommario del registro di contabilità (Allegato II.14 al D.Lgs. 36/2023, art. 12,
//! comma 2): per ogni articolo di elenco, le quantità e gli importi contabilizzati in
//! ciascun SAL, per verificarne la corrispondenza con il registro.
//!
//! Le registrazioni sono raggruppate per tariffa e prezzo applicato: la stessa tariffa
//! con prezzi fotografati diversi compare su righe distinte. Sono comprese le
//! registrazioni contabilizzate in un SAL (emesso o in bozza) e, se il conto finale è
//! redatto, quelle dello stato finale in un'ultima colonna.

use serde::{Deserialize, Serialize};

use super::Contabilita;
use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
use crate::id::RegistrazioneId;
use crate::numeri::{Importo, Prezzo, Quantita, RegoleArrotondamento};
use crate::prezzario::NaturaVoce;

/// Riga del sommario: un articolo di elenco con un prezzo applicato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticoloSommario {
    /// Tariffa.
    pub codice: String,
    /// Descrizione breve.
    pub descrizione_breve: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo applicato.
    pub prezzo: Prezzo,
    /// Natura.
    pub natura: NaturaVoce,
    /// Quantità e importo lordo per SAL, nell'ordine di [`SommarioRegistro::sal`].
    pub per_sal: Vec<(Quantita, Importo)>,
    /// Quantità complessiva.
    pub quantita: Quantita,
    /// Importo lordo complessivo.
    pub importo: Importo,
}

/// Sommario del registro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SommarioRegistro {
    /// Numeri dei SAL (colonne).
    pub sal: Vec<u32>,
    /// Articoli in ordine di tariffa.
    pub articoli: Vec<ArticoloSommario>,
    /// Importo lordo per SAL.
    pub totali_per_sal: Vec<Importo>,
    /// `true` se l'ultima colonna di `per_sal` e `totali_per_sal` è lo stato finale
    /// (registrazioni non comprese nei SAL, con il conto finale redatto).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stato_finale: bool,
    /// Importo lordo complessivo.
    pub totale: Importo,
}

impl Contabilita {
    /// Sommario del registro di contabilità.
    ///
    /// # Errors
    /// Registrazioni non calcolabili o riferimenti incoerenti.
    pub fn sommario(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<SommarioRegistro, ErroreDominio> {
        let mut gruppi: Vec<(Vec<RegistrazioneId>, String)> = self
            .sal
            .iter()
            .map(|s| (s.registrazioni.clone(), format!("il SAL n. {}", s.numero)))
            .collect();
        let finali: Vec<RegistrazioneId> = if self.conto_finale.is_some() {
            self.registrazioni_stato_finale()
                .iter()
                .map(|r| r.id)
                .collect()
        } else {
            Vec::new()
        };
        let stato_finale = !finali.is_empty();
        if stato_finale {
            gruppi.push((finali, "il conto finale".into()));
        }
        let colonne = gruppi.len();
        let mut articoli: Vec<ArticoloSommario> = Vec::new();
        let mut totali_per_sal = vec![Importo::ZERO; colonne];
        let somma = |a: Importo, b: Importo, rif| {
            a.somma(b)
                .map_err(|e: ErroreCalcolo| ErroreDominio::Calcolo(ErroreLocalizzato::new(rif, e)))
        };
        for (colonna, (registrazioni, nome)) in gruppi.iter().enumerate() {
            for reg_id in registrazioni {
                let r = self.libretto.registrazione(*reg_id).ok_or_else(|| {
                    ErroreDominio::RiferimentoNonValido(format!(
                        "{nome} riferisce la registrazione inesistente {reg_id}"
                    ))
                })?;
                let c = r.calcola(regole).map_err(ErroreDominio::Calcolo)?;
                let posizione = articoli
                    .iter()
                    .position(|a| a.codice == r.prezzo.codice && a.prezzo == r.prezzo.prezzo);
                let articolo = if let Some(i) = posizione {
                    articoli.get_mut(i)
                } else {
                    articoli.push(ArticoloSommario {
                        codice: r.prezzo.codice.clone(),
                        descrizione_breve: r.prezzo.descrizione_breve.clone(),
                        unita_misura: r.prezzo.unita_misura.clone(),
                        prezzo: r.prezzo.prezzo,
                        natura: r.prezzo.natura,
                        per_sal: vec![(Quantita::ZERO, Importo::ZERO); colonne],
                        quantita: Quantita::ZERO,
                        importo: Importo::ZERO,
                    });
                    articoli.last_mut()
                };
                let Some(a) = articolo else { continue };
                let errore = |e: ErroreCalcolo| {
                    ErroreDominio::Calcolo(ErroreLocalizzato::new(r.riferimento(), e))
                };
                if let Some((q, i)) = a.per_sal.get_mut(colonna) {
                    *q = q.somma(c.quantita).map_err(errore)?;
                    *i = i.somma(c.importo).map_err(errore)?;
                }
                a.quantita = a.quantita.somma(c.quantita).map_err(errore)?;
                a.importo = a.importo.somma(c.importo).map_err(errore)?;
                if let Some(t) = totali_per_sal.get_mut(colonna) {
                    *t = somma(*t, c.importo, r.riferimento())?;
                }
            }
        }
        articoli.sort_by(|a, b| {
            a.codice
                .cmp(&b.codice)
                .then(a.prezzo.valore().cmp(&b.prezzo.valore()))
        });
        let mut totale = Importo::ZERO;
        for t in &totali_per_sal {
            totale = totale
                .somma(*t)
                .map_err(|e| ErroreDominio::RiferimentoNonValido(e.to_string()))?;
        }
        Ok(SommarioRegistro {
            sal: self.sal.iter().map(|s| s.numero).collect(),
            articoli,
            totali_per_sal,
            stato_finale,
            totale,
        })
    }
}
