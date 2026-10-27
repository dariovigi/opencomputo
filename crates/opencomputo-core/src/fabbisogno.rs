//! Fabbisogno di cantiere: quantità delle risorse (manodopera, materiali, noli,
//! trasporti) necessarie per eseguire le voci di un computo, ricavate dalle analisi
//! prezzi dell'elenco.
//!
//! ```text
//! quantità della risorsa = Σ quantità della voce × quantità nella riga
//!                          / quantità di riferimento dell'analisi
//! ```
//!
//! Le righe che sono voci analizzate vengono espanse ricorsivamente nelle loro risorse;
//! le voci semplici del prezzario usate come righe compaiono come tali. Le quantità sono
//! sommate esatte e arrotondate alla fine; gli importi sono quantità × prezzo corrente,
//! senza spese generali né utile. Le voci di computo senza analisi sono elencate a parte.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::analisi::{OrigineRigaAnalisi, TipoRisorsa};
use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
use crate::id::{AnalisiId, ComputoId, RisorsaId, VocePrezzarioId};
use crate::numeri::{Importo, Prezzo, Quantita};
use crate::progetto::Progetto;
use crate::riferimento::Riferimento;

/// Risorsa del fabbisogno.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RisorsaFabbisogno {
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo.
    pub tipo: TipoRisorsa,
    /// Quantità necessaria.
    pub quantita: Quantita,
    /// Prezzo unitario corrente.
    pub prezzo: Prezzo,
    /// Importo = quantità × prezzo.
    pub importo: Importo,
}

/// Fabbisogno di cantiere di un computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fabbisogno {
    /// Risorse per tipo e codice.
    pub risorse: Vec<RisorsaFabbisogno>,
    /// Importi per tipo.
    pub totali_per_tipo: BTreeMap<TipoRisorsa, Importo>,
    /// Importo complessivo.
    pub totale: Importo,
    /// Codici delle voci di computo senza analisi (escluse dal fabbisogno).
    pub voci_senza_analisi: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Chiave {
    Risorsa(RisorsaId),
    Voce(VocePrezzarioId),
    Libera(String, String),
}

struct Accumulo {
    codice: String,
    descrizione: String,
    unita_misura: String,
    tipo: TipoRisorsa,
    prezzo: Prezzo,
    quantita: Decimal,
}

struct Espansione<'a> {
    progetto: &'a Progetto,
    pila: Vec<AnalisiId>,
    risorse: BTreeMap<Chiave, Accumulo>,
}

fn fuori_scala() -> ErroreDominio {
    ErroreDominio::Calcolo(ErroreLocalizzato::new(
        Riferimento::Progetto,
        ErroreCalcolo::Overflow,
    ))
}

impl Espansione<'_> {
    fn aggiungi(&mut self, chiave: Chiave, dati: Accumulo) -> Result<(), ErroreDominio> {
        match self.risorse.get_mut(&chiave) {
            Some(a) => {
                a.quantita = a
                    .quantita
                    .checked_add(dati.quantita)
                    .ok_or_else(fuori_scala)?;
            }
            None => {
                self.risorse.insert(chiave, dati);
            }
        }
        Ok(())
    }

    fn espandi(&mut self, id: AnalisiId, quantita: Decimal) -> Result<(), ErroreDominio> {
        let elenco = &self.progetto.elenco_prezzi;
        let analisi = elenco
            .analisi(id)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "analisi",
                id: id.to_string(),
            })?;
        if self.pila.contains(&id) {
            return Err(ErroreDominio::RiferimentoNonValido(format!(
                "analisi ciclica: «{}»",
                analisi.codice
            )));
        }
        let riferimento = analisi.quantita_riferimento.valore();
        if riferimento <= Decimal::ZERO {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "l'analisi «{}» ha una quantità di riferimento non positiva",
                analisi.codice
            )));
        }
        let fattore = quantita.checked_div(riferimento).ok_or_else(fuori_scala)?;
        let quantita_righe = analisi.quantita_righe().map_err(ErroreDominio::Calcolo)?;
        self.pila.push(id);
        for (riga, qr) in analisi.righe.iter().zip(quantita_righe) {
            let q = qr.valore().checked_mul(fattore).ok_or_else(fuori_scala)?;
            let libera = || {
                (
                    Chiave::Libera(riga.codice.clone(), riga.unita_misura.clone()),
                    Accumulo {
                        codice: riga.codice.clone(),
                        descrizione: riga.descrizione.clone(),
                        unita_misura: riga.unita_misura.clone(),
                        tipo: riga.tipo,
                        prezzo: riga.prezzo,
                        quantita: q,
                    },
                )
            };
            match riga.origine {
                Some(OrigineRigaAnalisi::Risorsa(r)) => match elenco.risorsa(r) {
                    Some(x) => self.aggiungi(
                        Chiave::Risorsa(r),
                        Accumulo {
                            codice: x.codice.clone(),
                            descrizione: x.descrizione.clone(),
                            unita_misura: x.unita_misura.clone(),
                            tipo: x.tipo,
                            prezzo: x.prezzo,
                            quantita: q,
                        },
                    )?,
                    None => {
                        let (k, a) = libera();
                        self.aggiungi(k, a)?;
                    }
                },
                Some(OrigineRigaAnalisi::VocePrezzario(v)) => match elenco.voce(v) {
                    Some(voce) => match voce.analisi {
                        Some(a) => self.espandi(a, q)?,
                        None => self.aggiungi(
                            Chiave::Voce(v),
                            Accumulo {
                                codice: voce.codice.clone(),
                                descrizione: voce.descrizione_breve.clone(),
                                unita_misura: voce.unita_misura.clone(),
                                tipo: riga.tipo,
                                prezzo: voce.prezzo,
                                quantita: q,
                            },
                        )?,
                    },
                    None => {
                        let (k, a) = libera();
                        self.aggiungi(k, a)?;
                    }
                },
                None => {
                    let (k, a) = libera();
                    self.aggiungi(k, a)?;
                }
            }
        }
        self.pila.pop();
        Ok(())
    }
}

impl Progetto {
    /// Fabbisogno di cantiere di un computo (vedi il modulo [`crate::fabbisogno`]).
    ///
    /// # Errors
    /// Computo inesistente, voci non calcolabili, analisi cicliche o non valide.
    pub fn fabbisogno(&self, computo: ComputoId) -> Result<Fabbisogno, ErroreDominio> {
        let c = self
            .computo(computo)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "computo",
                id: computo.to_string(),
            })?;
        let elenco = &self.elenco_prezzi;
        let mut e = Espansione {
            progetto: self,
            pila: Vec::new(),
            risorse: BTreeMap::new(),
        };
        let mut senza = Vec::new();
        let riferite = c.quantita_riferite(&self.regole);
        for voce in &c.voci {
            let quantita = voce
                .calcola_in(&self.regole, &riferite)
                .map_err(ErroreDominio::Calcolo)?
                .quantita;
            let elenco_voce = voce
                .prezzo
                .origine
                .filter(|o| o.prezzario == elenco.id)
                .and_then(|o| elenco.voce(o.voce))
                .or_else(|| elenco.voce_per_codice(&voce.prezzo.codice));
            match elenco_voce.and_then(|v| v.analisi) {
                Some(a) => e.espandi(a, quantita.valore())?,
                None => {
                    if !senza.contains(&voce.prezzo.codice) {
                        senza.push(voce.prezzo.codice.clone());
                    }
                }
            }
        }
        let mut risorse = Vec::with_capacity(e.risorse.len());
        let mut totali_per_tipo: BTreeMap<TipoRisorsa, Importo> = BTreeMap::new();
        let mut totale = Importo::ZERO;
        let errore = |e: ErroreCalcolo| {
            ErroreDominio::Calcolo(ErroreLocalizzato::new(
                Riferimento::Computo { id: computo },
                e,
            ))
        };
        for a in e.risorse.into_values() {
            let quantita = self.regole.arrotonda_quantita(a.quantita);
            let importo = a
                .prezzo
                .per_quantita(quantita, &self.regole)
                .map_err(errore)?;
            let t = totali_per_tipo.entry(a.tipo).or_insert(Importo::ZERO);
            *t = t.somma(importo).map_err(errore)?;
            totale = totale.somma(importo).map_err(errore)?;
            risorse.push(RisorsaFabbisogno {
                codice: a.codice,
                descrizione: a.descrizione,
                unita_misura: a.unita_misura,
                tipo: a.tipo,
                quantita,
                prezzo: a.prezzo,
                importo,
            });
        }
        risorse.sort_by(|x, y| x.tipo.cmp(&y.tipo).then_with(|| x.codice.cmp(&y.codice)));
        Ok(Fabbisogno {
            risorse,
            totali_per_tipo,
            totale,
            voci_senza_analisi: senza,
        })
    }
}

/// Elemento dell'analisi di una voce riportato alla quantità del computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementoVoceComputo {
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo.
    pub tipo: TipoRisorsa,
    /// Quantità per la quantità della voce del computo.
    pub quantita: Quantita,
    /// Prezzo corrente.
    pub prezzo: Prezzo,
    /// Importo.
    pub importo: Importo,
}

/// Voce di computo con la sua analisi riportata alla quantità computata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalisiVoceComputo {
    /// Numero della voce nel computo.
    pub numero: usize,
    /// Elementi dell'analisi (assenti per le voci senza analisi).
    pub elementi: Vec<ElementoVoceComputo>,
    /// Spese generali riportate alla quantità.
    pub spese_generali: Importo,
    /// Utile riportato alla quantità.
    pub utile: Importo,
    /// Importo della voce nel computo meno elementi, spese generali e utile: arrotondamenti
    /// o prezzo fotografato diverso da quello dell'analisi.
    pub differenza: Importo,
    /// `true` se la voce ha un'analisi.
    pub analizzata: bool,
}

impl Progetto {
    /// Analisi delle voci di un computo riportate alle quantità computate: per ogni
    /// elemento quantità = quantità nella riga × quantità della voce / quantità di
    /// riferimento, importo = quantità × prezzo corrente (arrotondati); spese generali e
    /// utile in proporzione. Le sotto-analisi restano un elemento al loro prezzo.
    ///
    /// # Errors
    /// Computo inesistente o dati non calcolabili.
    pub fn analisi_computo(
        &self,
        computo: ComputoId,
    ) -> Result<Vec<AnalisiVoceComputo>, ErroreDominio> {
        let c = self
            .computo(computo)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "computo",
                id: computo.to_string(),
            })?;
        let elenco = &self.elenco_prezzi;
        let regole = &self.regole;
        let errore = |e: ErroreCalcolo| {
            ErroreDominio::Calcolo(ErroreLocalizzato::new(
                Riferimento::Computo { id: computo },
                e,
            ))
        };
        let mut voci = Vec::with_capacity(c.voci.len());
        let riferite = c.quantita_riferite(regole);
        for (i, voce) in c.voci.iter().enumerate() {
            let calcolo = voce
                .calcola_in(regole, &riferite)
                .map_err(ErroreDominio::Calcolo)?;
            let analisi = voce
                .prezzo
                .origine
                .filter(|o| o.prezzario == elenco.id)
                .and_then(|o| elenco.voce(o.voce))
                .or_else(|| elenco.voce_per_codice(&voce.prezzo.codice))
                .and_then(|v| v.analisi)
                .and_then(|a| elenco.analisi(a).map(|x| (a, x)));
            let Some((id, a)) = analisi else {
                voci.push(AnalisiVoceComputo {
                    numero: i + 1,
                    elementi: Vec::new(),
                    spese_generali: Importo::ZERO,
                    utile: Importo::ZERO,
                    differenza: Importo::ZERO,
                    analizzata: false,
                });
                continue;
            };
            let r = elenco.calcola_analisi(id, regole)?;
            let fattore = calcolo
                .quantita
                .valore()
                .checked_div(a.quantita_riferimento.valore())
                .ok_or_else(fuori_scala)?;
            let scala = |v: Decimal| v.checked_mul(fattore).ok_or_else(fuori_scala);
            let mut elementi = Vec::with_capacity(a.righe.len());
            let mut somma = Importo::ZERO;
            for (riga, calcolata) in a.righe.iter().zip(&r.righe) {
                let quantita = regole.arrotonda_quantita(scala(calcolata.quantita.valore())?);
                let importo = calcolata
                    .prezzo
                    .per_quantita(quantita, regole)
                    .map_err(errore)?;
                somma = somma.somma(importo).map_err(errore)?;
                elementi.push(ElementoVoceComputo {
                    codice: riga.codice.clone(),
                    descrizione: riga.descrizione.clone(),
                    unita_misura: riga.unita_misura.clone(),
                    tipo: riga.tipo,
                    quantita,
                    prezzo: calcolata.prezzo,
                    importo,
                });
            }
            let spese_generali = regole.arrotonda_importo(scala(r.spese_generali.valore())?);
            let utile = regole.arrotonda_importo(scala(r.utile.valore())?);
            let differenza = calcolo
                .importo
                .differenza(somma)
                .and_then(|x| x.differenza(spese_generali))
                .and_then(|x| x.differenza(utile))
                .map_err(errore)?;
            voci.push(AnalisiVoceComputo {
                numero: i + 1,
                elementi,
                spese_generali,
                utile,
                differenza,
                analizzata: true,
            });
        }
        Ok(voci)
    }
}
