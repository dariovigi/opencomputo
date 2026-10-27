//! Analisi dei prezzi e risorse elementari.
//!
//! ## Procedimento di calcolo
//!
//! 1. Importo di ogni riga = quantità × prezzo della risorsa, arrotondato agli importi.
//! 2. Costo diretto = somma delle righe; i costi sono riepilogati per tipo di risorsa.
//! 3. Spese generali = costo diretto × % spese generali (arrotondato).
//! 4. Utile d'impresa = base × % utile, dove la base è, per default,
//!    costo diretto + spese generali ([`BaseUtile`]).
//! 5. Totale = costo diretto + spese generali + utile.
//! 6. Prezzo unitario = totale / quantità di riferimento dell'analisi,
//!    arrotondato ai decimali dei prezzi.
//! 7. Incidenza della manodopera = costo della manodopera / totale × 100.
//!
//! Le percentuali di spese generali e utile sono parametri dell'analisi, con valori
//! predefiniti per il prezzario ([`ParametriAnalisi`]: 15 % e 10 % sul costo più spese
//! generali, come nei prezzari regionali): il core non impone valori normativi, che vanno
//! verificati dal progettista.
//!
//! ## Analisi nel prezzario (dinamiche e annidate)
//!
//! [`Prezzario::calcola_analisi`] calcola un'analisi con i prezzi **correnti** delle
//! risorse e delle voci collegate alle righe: una riga può essere una risorsa, una voce
//! semplice del prezzario o una voce a sua volta analizzata (sotto-analisi, calcolata
//! ricorsivamente con il suo prezzo unitario; i cicli sono un errore). La copia dei dati
//! nella riga serve per le righe libere e come riferimento se l'origine non esiste più.
//!
//! Le incidenze per tipo di risorsa comprendono le risorse delle sotto-analisi: il costo
//! di una riga che è una voce analizzata è ripartito secondo le incidenze di quella voce
//! (la parte di spese generali e utile della sotto-analisi non è attribuita ad alcun
//! tipo); per una voce semplice vale la sua incidenza della manodopera.
//!
//! ## Variabili e quantità
//!
//! Le quantità delle righe possono essere espressioni (`0,5*1,2`) che usano le
//! [`VariabileAnalisi`] dell'analisi (es. `spessore = 0,02`, `peso = spessore * 1800`, nel
//! loro ordine) e le quantità delle righe precedenti (`q1`, `q2`, …): per esempio il nolo
//! di una betoniera pari alle ore di manodopera della riga 1 si scrive `q1`.
//!
//! ## Oneri aziendali della sicurezza
//!
//! Un'analisi può indicare quale parte delle spese generali è costituita dagli oneri
//! aziendali della sicurezza ([`AnalisiPrezzo::quota_oneri_sicurezza`]): l'importo è
//! evidenziato nel risultato e nelle stampe, senza cambiare il prezzo.
//!
//! Il prezzo della voce di elenco **non** cambia da solo: va applicato con
//! [`Prezzario::aggiorna_prezzo_da_analisi`] o [`Prezzario::ricalcola_prezzi_da_analisi`],
//! e i computi conservano comunque i prezzi fotografati.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
use crate::formula::{ContestoFormula, ErroreFormula, valuta_con};
use crate::id::{AnalisiId, RisorsaId, VocePrezzarioId};
use crate::numeri::{Importo, Percentuale, Prezzo, Quantita, RegoleArrotondamento};
use crate::prezzario::Prezzario;
use crate::riferimento::Riferimento;

/// Tipo di risorsa elementare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoRisorsa {
    /// Manodopera.
    Manodopera,
    /// Materiale.
    Materiale,
    /// Nolo di mezzi e attrezzature.
    Nolo,
    /// Trasporto.
    Trasporto,
    /// Altro.
    Altro,
}

/// Risorsa elementare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RisorsaElementare {
    /// Identificativo.
    pub id: RisorsaId,
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo.
    pub tipo: TipoRisorsa,
    /// Prezzo unitario.
    pub prezzo: Prezzo,
}

/// Origine dei dati di una riga di analisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "id", rename_all = "snake_case")]
pub enum OrigineRigaAnalisi {
    /// Copiata da una risorsa elementare.
    Risorsa(RisorsaId),
    /// Copiata da una voce di prezzario.
    VocePrezzario(VocePrezzarioId),
}

/// Riga di un'analisi prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigaAnalisi {
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo di risorsa.
    pub tipo: TipoRisorsa,
    /// Prezzo unitario (copia).
    pub prezzo: Prezzo,
    /// Quantità impiegata per la quantità di riferimento dell'analisi.
    pub quantita: Quantita,
    /// Espressione da cui è stata calcolata la quantità (es. `0,5*1,2`), conservata per
    /// la modifica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub espressione: Option<String>,
    /// Origine dei dati.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origine: Option<OrigineRigaAnalisi>,
}

impl RigaAnalisi {
    /// Riga copiata da una risorsa elementare.
    #[must_use]
    pub fn da_risorsa(risorsa: &RisorsaElementare, quantita: Quantita) -> Self {
        Self {
            codice: risorsa.codice.clone(),
            descrizione: risorsa.descrizione.clone(),
            unita_misura: risorsa.unita_misura.clone(),
            tipo: risorsa.tipo,
            prezzo: risorsa.prezzo,
            quantita,
            espressione: None,
            origine: Some(OrigineRigaAnalisi::Risorsa(risorsa.id)),
        }
    }
}

/// Spese generali e utile predefiniti delle analisi di un prezzario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParametriAnalisi {
    /// Spese generali.
    pub spese_generali: Percentuale,
    /// Utile d'impresa.
    pub utile_impresa: Percentuale,
    /// Base di calcolo dell'utile.
    #[serde(default)]
    pub base_utile: BaseUtile,
    /// Parte delle spese generali costituita dagli oneri aziendali della sicurezza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_oneri_sicurezza: Option<Percentuale>,
}

impl Default for ParametriAnalisi {
    /// 15 % di spese generali e 10 % di utile sul costo più spese generali.
    fn default() -> Self {
        Self {
            spese_generali: Percentuale::new(Decimal::new(15, 0)),
            utile_impresa: Percentuale::new(Decimal::new(10, 0)),
            base_utile: BaseUtile::CostoPiuSpeseGenerali,
            quota_oneri_sicurezza: None,
        }
    }
}

/// Quantità, prezzo e importo di una riga come calcolati.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigaCalcolata {
    /// Quantità (dall'espressione, se c'è).
    pub quantita: Quantita,
    /// Prezzo unitario usato (corrente o copiato).
    pub prezzo: Prezzo,
    /// Importo = quantità × prezzo, arrotondato.
    pub importo: Importo,
}

/// Prezzo di una riga e ripartizione del suo costo per tipo (frazioni dell'importo).
struct RigaRisolta {
    prezzo: Prezzo,
    quote: Vec<(TipoRisorsa, Decimal)>,
}

/// Variabile di un'analisi, usabile nelle quantità delle righe e nelle variabili
/// successive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariabileAnalisi {
    /// Nome (minuscolo: lettere, cifre e `_`, non `pi` né `q1`, `q2`…).
    pub nome: String,
    /// Valore o espressione.
    pub espressione: String,
    /// Descrizione (es. «peso specifico»).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub descrizione: String,
}

/// `true` se `nome` può essere il nome di una variabile d'analisi.
#[must_use]
pub fn nome_variabile_valido(nome: &str) -> bool {
    let mut c = nome.chars();
    let primo_ok = c.next().is_some_and(|x| x.is_ascii_lowercase() || x == '_');
    let resto_ok = nome
        .chars()
        .all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '_');
    let riservato = nome == "pi"
        || (nome.len() > 1
            && nome.starts_with('q')
            && nome.chars().skip(1).all(|x| x.is_ascii_digit()));
    primo_ok && resto_ok && !riservato
}

/// Valore di una variabile calcolata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValoreVariabile {
    /// Nome.
    pub nome: String,
    /// Valore.
    pub valore: Decimal,
}

struct ContestoAnalisi<'a> {
    valori: &'a [ValoreVariabile],
}

impl ContestoFormula for ContestoAnalisi<'_> {
    fn variabile(&self, nome: &str) -> Result<Decimal, ErroreFormula> {
        self.valori
            .iter()
            .rev()
            .find(|v| v.nome == nome)
            .map(|v| v.valore)
            .ok_or_else(|| ErroreFormula::VariabileSconosciuta {
                nome: nome.to_owned(),
            })
    }
}

/// Base di calcolo dell'utile d'impresa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseUtile {
    /// Utile calcolato su costo diretto + spese generali.
    #[default]
    CostoPiuSpeseGenerali,
    /// Utile calcolato sul solo costo diretto.
    SoloCosto,
}

/// Analisi prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalisiPrezzo {
    /// Identificativo.
    pub id: AnalisiId,
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura del prezzo risultante.
    pub unita_misura: String,
    /// Quantità a cui si riferisce l'analisi (es. 1 m³, 100 m²).
    pub quantita_riferimento: Quantita,
    /// Righe.
    pub righe: Vec<RigaAnalisi>,
    /// Spese generali.
    pub spese_generali: Percentuale,
    /// Utile d'impresa.
    pub utile_impresa: Percentuale,
    /// Base di calcolo dell'utile.
    #[serde(default)]
    pub base_utile: BaseUtile,
    /// Variabili usate nelle quantità delle righe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variabili: Vec<VariabileAnalisi>,
    /// Parte delle spese generali costituita dagli oneri aziendali della sicurezza (in
    /// percentuale delle spese generali).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_oneri_sicurezza: Option<Percentuale>,
}

/// Risultato di un'analisi prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RisultatoAnalisi {
    /// Costi diretti per tipo di risorsa.
    pub costi_per_tipo: BTreeMap<TipoRisorsa, Importo>,
    /// Costo diretto totale.
    pub costo_diretto: Importo,
    /// Spese generali.
    pub spese_generali: Importo,
    /// Utile d'impresa.
    pub utile: Importo,
    /// Totale per la quantità di riferimento.
    pub totale: Importo,
    /// Prezzo unitario risultante.
    pub prezzo_unitario: Prezzo,
    /// Incidenza della manodopera sul totale; `None` se il totale è zero.
    pub incidenza_manodopera: Option<Percentuale>,
    /// Incidenze sul totale per tipo di risorsa, comprese le sotto-analisi.
    #[serde(default)]
    pub incidenze: BTreeMap<TipoRisorsa, Percentuale>,
    /// Prezzi e importi delle righe, nell'ordine dell'analisi.
    #[serde(default)]
    pub righe: Vec<RigaCalcolata>,
    /// Valori delle variabili.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variabili: Vec<ValoreVariabile>,
    /// Oneri aziendali della sicurezza compresi nelle spese generali.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oneri_sicurezza_aziendali: Option<Importo>,
}

impl AnalisiPrezzo {
    fn riferimento(&self) -> Riferimento {
        Riferimento::Analisi {
            id: self.id,
            codice: self.codice.clone(),
        }
    }

    /// Valori delle variabili, nel loro ordine.
    ///
    /// # Errors
    /// Nome non valido o duplicato, espressione non valida.
    pub fn valori_variabili(&self) -> Result<Vec<ValoreVariabile>, ErroreLocalizzato> {
        let err = |e: ErroreCalcolo| ErroreLocalizzato::new(self.riferimento(), e);
        let mut valori: Vec<ValoreVariabile> = Vec::with_capacity(self.variabili.len());
        for v in &self.variabili {
            if !nome_variabile_valido(&v.nome) {
                return Err(err(ErroreCalcolo::ValoreNonValido(format!(
                    "nome di variabile «{}» non valido: lettere minuscole, cifre e _, non pi né \
                     q1, q2…",
                    v.nome
                ))));
            }
            if valori.iter().any(|x| x.nome == v.nome) {
                return Err(err(ErroreCalcolo::ValoreNonValido(format!(
                    "variabile «{}» duplicata",
                    v.nome
                ))));
            }
            let valore =
                valuta_con(&v.espressione, &ContestoAnalisi { valori: &valori }).map_err(|e| {
                    err(ErroreCalcolo::ValoreNonValido(format!(
                        "variabile «{}»: {e}",
                        v.nome
                    )))
                })?;
            valori.push(ValoreVariabile {
                nome: v.nome.clone(),
                valore,
            });
        }
        Ok(valori)
    }

    /// Quantità effettive delle righe: l'espressione, se c'è, valutata con le variabili e
    /// le quantità delle righe precedenti (`q1`, `q2`, …); altrimenti la quantità.
    ///
    /// # Errors
    /// Variabili o espressioni non valide.
    pub fn quantita_righe(&self) -> Result<Vec<Quantita>, ErroreLocalizzato> {
        let mut valori = self.valori_variabili()?;
        let mut quantita = Vec::with_capacity(self.righe.len());
        for (i, riga) in self.righe.iter().enumerate() {
            let q = match &riga.espressione {
                Some(e) => valuta_con(e, &ContestoAnalisi { valori: &valori }).map_err(|x| {
                    ErroreLocalizzato::new(
                        Riferimento::RigaAnalisi { numero: i + 1 },
                        ErroreCalcolo::ValoreNonValido(format!("quantità «{e}»: {x}")),
                    )
                    .dentro(self.riferimento())
                })?,
                None => riga.quantita.valore(),
            };
            valori.push(ValoreVariabile {
                nome: format!("q{}", i + 1),
                valore: q,
            });
            quantita.push(Quantita::new(q));
        }
        Ok(quantita)
    }

    /// Calcola l'analisi con i prezzi copiati nelle righe (analisi isolata).
    ///
    /// # Errors
    /// Quantità di riferimento non positiva, regole non valide o overflow.
    pub fn calcola(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<RisultatoAnalisi, ErroreLocalizzato> {
        self.calcola_con(regole, |_, riga| {
            Ok(RigaRisolta {
                prezzo: riga.prezzo,
                quote: vec![(riga.tipo, Decimal::ONE)],
            })
        })
        .map(|(r, _)| r)
    }

    /// Calcolo con i prezzi delle righe forniti da `risolvi`. Restituisce anche le
    /// frazioni esatte del totale per tipo di risorsa.
    fn calcola_con<F>(
        &self,
        regole: &RegoleArrotondamento,
        mut risolvi: F,
    ) -> Result<(RisultatoAnalisi, BTreeMap<TipoRisorsa, Decimal>), ErroreLocalizzato>
    where
        F: FnMut(usize, &RigaAnalisi) -> Result<RigaRisolta, ErroreLocalizzato>,
    {
        let err = |e: ErroreCalcolo| ErroreLocalizzato::new(self.riferimento(), e);
        regole.valida().map_err(err)?;
        if self.quantita_riferimento.valore() <= Decimal::ZERO {
            return Err(err(ErroreCalcolo::ValoreNonValido(
                "la quantità di riferimento dell'analisi deve essere positiva".into(),
            )));
        }

        let quantita = self.quantita_righe()?;
        let variabili = self.valori_variabili()?;
        let mut costi_per_tipo: BTreeMap<TipoRisorsa, Importo> = BTreeMap::new();
        let mut quote_esatte: BTreeMap<TipoRisorsa, Decimal> = BTreeMap::new();
        let mut costo_diretto = Importo::ZERO;
        let mut righe = Vec::with_capacity(self.righe.len());
        for (i, riga) in self.righe.iter().enumerate() {
            let err_riga = |e: ErroreCalcolo| {
                ErroreLocalizzato::new(Riferimento::RigaAnalisi { numero: i + 1 }, e)
                    .dentro(self.riferimento())
            };
            let risolta = risolvi(i, riga)?;
            let q = quantita.get(i).copied().unwrap_or(riga.quantita);
            let importo = risolta.prezzo.per_quantita(q, regole).map_err(err_riga)?;
            let voce = costi_per_tipo.entry(riga.tipo).or_insert(Importo::ZERO);
            *voce = voce.somma(importo).map_err(err_riga)?;
            costo_diretto = costo_diretto.somma(importo).map_err(err_riga)?;
            for (tipo, quota) in risolta.quote {
                let parte = importo
                    .valore()
                    .checked_mul(quota)
                    .ok_or(ErroreCalcolo::Overflow)
                    .map_err(err_riga)?;
                let q = quote_esatte.entry(tipo).or_insert(Decimal::ZERO);
                *q = q
                    .checked_add(parte)
                    .ok_or(ErroreCalcolo::Overflow)
                    .map_err(err_riga)?;
            }
            righe.push(RigaCalcolata {
                quantita: q,
                prezzo: risolta.prezzo,
                importo,
            });
        }

        let spese_generali = self
            .spese_generali
            .quota_di(costo_diretto, regole)
            .map_err(err)?;
        let base_utile = match self.base_utile {
            BaseUtile::CostoPiuSpeseGenerali => costo_diretto.somma(spese_generali).map_err(err)?,
            BaseUtile::SoloCosto => costo_diretto,
        };
        let utile = self
            .utile_impresa
            .quota_di(base_utile, regole)
            .map_err(err)?;
        let totale = Importo::totale([costo_diretto, spese_generali, utile]).map_err(err)?;
        let unitario = totale
            .valore()
            .checked_div(self.quantita_riferimento.valore())
            .ok_or(ErroreCalcolo::Overflow)
            .map_err(err)?;
        let prezzo_unitario = regole.arrotonda_prezzo(unitario);
        let oneri_sicurezza_aziendali = match self.quota_oneri_sicurezza {
            Some(q) => Some(q.quota_di(spese_generali, regole).map_err(err)?),
            None => None,
        };

        // Frazioni esatte del totale e incidenze arrotondate.
        let mut frazioni = BTreeMap::new();
        let mut incidenze = BTreeMap::new();
        if !totale.is_zero() {
            for (tipo, parte) in &quote_esatte {
                let f = parte
                    .checked_div(totale.valore())
                    .ok_or(ErroreCalcolo::Overflow)
                    .map_err(err)?;
                let percento = f
                    .checked_mul(Decimal::ONE_HUNDRED)
                    .ok_or(ErroreCalcolo::Overflow)
                    .map_err(err)?;
                frazioni.insert(*tipo, f);
                incidenze.insert(*tipo, regole.arrotonda_percentuale(percento));
            }
        }
        let incidenza_manodopera = if totale.is_zero() {
            None
        } else {
            Some(
                incidenze
                    .get(&TipoRisorsa::Manodopera)
                    .copied()
                    .unwrap_or_else(|| Percentuale::new(Decimal::ZERO)),
            )
        };

        Ok((
            RisultatoAnalisi {
                costi_per_tipo,
                costo_diretto,
                spese_generali,
                utile,
                totale,
                prezzo_unitario,
                incidenza_manodopera,
                incidenze,
                righe,
                variabili,
                oneri_sicurezza_aziendali,
            },
            frazioni,
        ))
    }
}

/// Calcolo ricorsivo delle analisi di un prezzario con memoria dei risultati.
struct CalcoloPrezzario<'a> {
    prezzario: &'a Prezzario,
    regole: &'a RegoleArrotondamento,
    pila: Vec<AnalisiId>,
    fatti: BTreeMap<AnalisiId, (RisultatoAnalisi, BTreeMap<TipoRisorsa, Decimal>)>,
}

impl CalcoloPrezzario<'_> {
    fn calcola(
        &mut self,
        id: AnalisiId,
    ) -> Result<(RisultatoAnalisi, BTreeMap<TipoRisorsa, Decimal>), ErroreDominio> {
        if let Some(r) = self.fatti.get(&id) {
            return Ok(r.clone());
        }
        let analisi = self
            .prezzario
            .analisi(id)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "analisi",
                id: id.to_string(),
            })?;
        if self.pila.contains(&id) {
            let catena: Vec<String> = self
                .pila
                .iter()
                .filter_map(|a| self.prezzario.analisi(*a).map(|x| x.codice.clone()))
                .chain(std::iter::once(analisi.codice.clone()))
                .collect();
            return Err(ErroreDominio::RiferimentoNonValido(format!(
                "analisi ciclica: {}",
                catena.join(" → ")
            )));
        }
        self.pila.push(id);
        // Le sotto-analisi si calcolano prima, fuori dalla chiusura.
        let mut risolte = Vec::with_capacity(analisi.righe.len());
        for riga in &analisi.righe {
            risolte.push(self.risolvi(riga)?);
        }
        self.pila.pop();
        let mut risolte = risolte.into_iter();
        let risultato = analisi
            .calcola_con(self.regole, |_, riga| {
                Ok(risolte.next().unwrap_or(RigaRisolta {
                    prezzo: riga.prezzo,
                    quote: vec![(riga.tipo, Decimal::ONE)],
                }))
            })
            .map_err(ErroreDominio::Calcolo)?;
        self.fatti.insert(id, risultato.clone());
        Ok(risultato)
    }

    fn risolvi(&mut self, riga: &RigaAnalisi) -> Result<RigaRisolta, ErroreDominio> {
        let copia = RigaRisolta {
            prezzo: riga.prezzo,
            quote: vec![(riga.tipo, Decimal::ONE)],
        };
        Ok(match riga.origine {
            None => copia,
            Some(OrigineRigaAnalisi::Risorsa(r)) => match self.prezzario.risorsa(r) {
                Some(risorsa) => RigaRisolta {
                    prezzo: risorsa.prezzo,
                    quote: vec![(risorsa.tipo, Decimal::ONE)],
                },
                None => copia,
            },
            Some(OrigineRigaAnalisi::VocePrezzario(v)) => match self.prezzario.voce(v) {
                None => copia,
                Some(voce) => match voce.analisi {
                    Some(a) => {
                        let (r, frazioni) = self.calcola(a)?;
                        RigaRisolta {
                            prezzo: r.prezzo_unitario,
                            quote: frazioni.into_iter().collect(),
                        }
                    }
                    None => {
                        let mdo = voce
                            .incidenza_manodopera
                            .map_or(Decimal::ZERO, |p| p.valore() / Decimal::ONE_HUNDRED);
                        let mut quote = vec![(riga.tipo, Decimal::ONE - mdo)];
                        if !mdo.is_zero() {
                            quote.push((TipoRisorsa::Manodopera, mdo));
                        }
                        RigaRisolta {
                            prezzo: voce.prezzo,
                            quote,
                        }
                    }
                },
            },
        })
    }
}

impl Prezzario {
    /// Spese generali e utile predefiniti per le nuove analisi.
    #[must_use]
    pub fn parametri_analisi(&self) -> ParametriAnalisi {
        self.parametri_analisi.unwrap_or_default()
    }

    /// Calcola un'analisi con i prezzi correnti di risorse e voci, comprese le
    /// sotto-analisi (vedi il modulo).
    ///
    /// # Errors
    /// Analisi inesistente, ciclo tra analisi o errori di calcolo.
    pub fn calcola_analisi(
        &self,
        id: AnalisiId,
        regole: &RegoleArrotondamento,
    ) -> Result<RisultatoAnalisi, ErroreDominio> {
        CalcoloPrezzario {
            prezzario: self,
            regole,
            pila: Vec::new(),
            fatti: BTreeMap::new(),
        }
        .calcola(id)
        .map(|(r, _)| r)
    }

    /// Voci collegate a un'analisi il cui prezzo differisce da quello calcolato.
    ///
    /// # Errors
    /// Errori di calcolo di un'analisi.
    pub fn prezzi_non_allineati(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<Vec<(VocePrezzarioId, Prezzo, Prezzo)>, ErroreDominio> {
        let mut calcolo = CalcoloPrezzario {
            prezzario: self,
            regole,
            pila: Vec::new(),
            fatti: BTreeMap::new(),
        };
        let mut diversi = Vec::new();
        for v in &self.voci {
            if let Some(a) = v.analisi {
                let (r, _) = calcolo.calcola(a)?;
                if r.prezzo_unitario != v.prezzo {
                    diversi.push((v.id, v.prezzo, r.prezzo_unitario));
                }
            }
        }
        Ok(diversi)
    }

    /// Applica a tutte le voci analizzate il prezzo e l'incidenza della manodopera
    /// calcolati. Restituisce le voci cambiate con prezzo precedente e nuovo.
    ///
    /// # Errors
    /// Errori di calcolo di un'analisi: in tal caso nessuna voce è modificata.
    pub fn ricalcola_prezzi_da_analisi(
        &mut self,
        regole: &RegoleArrotondamento,
    ) -> Result<Vec<(VocePrezzarioId, Prezzo, Prezzo)>, ErroreDominio> {
        let mut calcolo = CalcoloPrezzario {
            prezzario: self,
            regole,
            pila: Vec::new(),
            fatti: BTreeMap::new(),
        };
        let mut nuovi = Vec::new();
        for v in &self.voci {
            if let Some(a) = v.analisi {
                let (r, _) = calcolo.calcola(a)?;
                nuovi.push((v.id, v.prezzo, r.prezzo_unitario, r.incidenza_manodopera));
            }
        }
        let mut cambiati = Vec::new();
        for (id, prima, dopo, mdo) in nuovi {
            if let Some(v) = self.voce_mut(id) {
                v.prezzo = dopo;
                v.incidenza_manodopera = mdo;
            }
            if prima != dopo {
                cambiati.push((id, prima, dopo));
            }
        }
        Ok(cambiati)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn risorsa(
        n: u128,
        codice: &str,
        tipo: TipoRisorsa,
        um: &str,
        prezzo: Decimal,
    ) -> RisorsaElementare {
        RisorsaElementare {
            id: RisorsaId::da_u128(n),
            codice: codice.into(),
            descrizione: codice.into(),
            unita_misura: um.into(),
            tipo,
            prezzo: Prezzo::new(prezzo),
        }
    }

    pub(crate) fn analisi_calcestruzzo() -> AnalisiPrezzo {
        let operaio = risorsa(1, "MO.01", TipoRisorsa::Manodopera, "h", dec!(35.50));
        let cls = risorsa(2, "MA.10", TipoRisorsa::Materiale, "m³", dec!(110.00));
        let betoniera = risorsa(3, "NO.05", TipoRisorsa::Nolo, "h", dec!(12.30));
        AnalisiPrezzo {
            id: AnalisiId::da_u128(100),
            codice: "AP.01".into(),
            descrizione: "Getto di calcestruzzo".into(),
            unita_misura: "m³".into(),
            quantita_riferimento: Quantita::new(dec!(1)),
            righe: vec![
                RigaAnalisi::da_risorsa(&operaio, Quantita::new(dec!(2))),
                RigaAnalisi::da_risorsa(&cls, Quantita::new(dec!(1))),
                RigaAnalisi::da_risorsa(&betoniera, Quantita::new(dec!(0.5))),
            ],
            spese_generali: Percentuale::new(dec!(15)),
            utile_impresa: Percentuale::new(dec!(10)),
            base_utile: BaseUtile::CostoPiuSpeseGenerali,
            variabili: Vec::new(),
            quota_oneri_sicurezza: None,
        }
    }

    #[test]
    fn calcolo_analisi_verificato_a_mano() {
        let r = analisi_calcestruzzo()
            .calcola(&RegoleArrotondamento::default())
            .unwrap();
        // 2 × 35,50 = 71,00; 1 × 110,00 = 110,00; 0,5 × 12,30 = 6,15
        assert_eq!(r.costo_diretto, Importo::new(dec!(187.15)));
        assert_eq!(
            r.costi_per_tipo[&TipoRisorsa::Manodopera],
            Importo::new(dec!(71.00))
        );
        assert_eq!(
            r.costi_per_tipo[&TipoRisorsa::Nolo],
            Importo::new(dec!(6.15))
        );
        // 187,15 × 15 % = 28,0725 → 28,07
        assert_eq!(r.spese_generali, Importo::new(dec!(28.07)));
        // (187,15 + 28,07) × 10 % = 21,522 → 21,52
        assert_eq!(r.utile, Importo::new(dec!(21.52)));
        assert_eq!(r.totale, Importo::new(dec!(236.74)));
        assert_eq!(r.prezzo_unitario, Prezzo::new(dec!(236.74)));
        // 71 / 236,74 × 100 = 29,9907… → 29,99
        assert_eq!(r.incidenza_manodopera, Some(Percentuale::new(dec!(29.99))));
    }

    #[test]
    fn utile_sul_solo_costo_e_quantita_di_riferimento() {
        let mut a = analisi_calcestruzzo();
        a.base_utile = BaseUtile::SoloCosto;
        a.quantita_riferimento = Quantita::new(dec!(10));
        let r = a.calcola(&RegoleArrotondamento::default()).unwrap();
        // 187,15 × 10 % = 18,715 → 18,72
        assert_eq!(r.utile, Importo::new(dec!(18.72)));
        // 187,15 + 28,07 + 18,72 = 233,94; / 10 = 23,394 → 23,39
        assert_eq!(r.totale, Importo::new(dec!(233.94)));
        assert_eq!(r.prezzo_unitario, Prezzo::new(dec!(23.39)));
    }

    #[test]
    fn quantita_di_riferimento_non_valida() {
        let mut a = analisi_calcestruzzo();
        a.quantita_riferimento = Quantita::ZERO;
        let e = a.calcola(&RegoleArrotondamento::default()).unwrap_err();
        assert!(matches!(e.errore, ErroreCalcolo::ValoreNonValido(_)));
    }

    #[test]
    fn analisi_vuota() {
        let mut a = analisi_calcestruzzo();
        a.righe.clear();
        let r = a.calcola(&RegoleArrotondamento::default()).unwrap();
        assert_eq!(r.totale, Importo::ZERO);
        assert_eq!(r.incidenza_manodopera, None);
    }
}
