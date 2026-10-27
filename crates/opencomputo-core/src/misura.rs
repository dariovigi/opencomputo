//! Righi di misurazione analitica.
//!
//! Un rigo riproduce la struttura classica del foglio di misura:
//! *descrizione | parti uguali | lunghezza | larghezza | altezza/peso | quantità*.
//!
//! ## Regole di calcolo
//!
//! 1. Un rigo di tipo [`TipoRigo::Commento`] non contribuisce alla quantità.
//! 2. Ogni fattore può essere un valore o un'espressione (es. `2+1,5`), valutata
//!    con [`crate::formula`].
//! 3. Se il rigo ha una `formula`, questa sostituisce il prodotto dei fattori e
//!    può riferirsi ai fattori con le variabili `par`, `lun`, `lar`, `alt`.
//!    Usare una variabile il cui fattore è vuoto è un errore esplicito.
//! 4. Altrimenti la quantità è il prodotto dei soli fattori valorizzati.
//!    Un rigo di misura senza alcun fattore vale zero (la validazione lo segnala).
//! 5. Un rigo [`SegnoRigo::Deduzione`] cambia segno al risultato.
//! 6. Se le regole lo prevedono, ogni rigo è arrotondato prima della somma.
//!
//! ## «Vedi voce»
//!
//! Un rigo può partire dalla quantità di un'altra voce ([`VediVoce`]): collegata a una
//! voce dello stesso computo, e allora segue la voce, oppure statica, cioè copiata al
//! momento dell'inserimento (libretto delle misure, collegamento perso).
//!
//! * Senza formula, la quantità citata è un fattore in più: la quantità del rigo è la
//!   quantità citata per i fattori valorizzati (solo la quantità citata se non ce ne
//!   sono).
//! * Con una formula, questa sostituisce il prodotto e usa la quantità citata con la
//!   variabile `vv`.
//! * Le quantità delle voci collegate arrivano da un [`QuantitaRiferite`]: per un
//!   computo lo fornisce [`crate::computo::Computo::quantita_riferite`], che calcola le
//!   voci nell'ordine dei collegamenti e rifiuta i riferimenti circolari.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::errore::{ErroreCalcolo, ErroreLocalizzato};
use crate::formula::{self, ContestoFormula, ErroreFormula};
use crate::id::{RigoMisuraId, VoceComputoId};
use crate::numeri::{Quantita, RegoleArrotondamento};
use crate::riferimento::Riferimento;

/// Nomi delle variabili disponibili nella formula di un rigo.
pub const VARIABILI_RIGO: [&str; 4] = ["par", "lun", "lar", "alt"];

/// Variabile della formula con la quantità della voce citata da un «vedi voce».
pub const VARIABILE_VEDI_VOCE: &str = "vv";

/// Riferimento «vedi voce» di un rigo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VediVoce {
    /// Collegamento a una voce dello stesso computo: la quantità segue la voce.
    Collegata(VoceComputoId),
    /// Quantità copiata da una voce al momento dell'inserimento: non cambia più.
    Statica {
        /// Quantità della voce citata.
        quantita: Decimal,
        /// Voce citata, per le stampe (es. «voce n. 5 A.01.001 del computo …»).
        riferimento: String,
    },
}

impl VediVoce {
    /// Voce collegata, se il riferimento è dinamico.
    #[must_use]
    pub fn voce(&self) -> Option<VoceComputoId> {
        match self {
            Self::Collegata(id) => Some(*id),
            Self::Statica { .. } => None,
        }
    }
}

/// Fornisce le quantità delle voci citate dai righi «vedi voce» collegati.
pub trait QuantitaRiferite {
    /// Quantità della voce indicata.
    ///
    /// # Errors
    /// [`ErroreCalcolo::VediVoce`] se la voce non è disponibile o non è calcolabile.
    fn quantita(&self, voce: VoceComputoId) -> Result<Decimal, ErroreCalcolo>;
}

/// Nessuna voce disponibile: righi calcolati fuori da un computo.
#[derive(Debug, Clone, Copy, Default)]
pub struct SenzaRiferimenti;

impl QuantitaRiferite for SenzaRiferimenti {
    fn quantita(&self, _voce: VoceComputoId) -> Result<Decimal, ErroreCalcolo> {
        Err(ErroreCalcolo::VediVoce(
            "collegamento a una voce di computo non disponibile qui".into(),
        ))
    }
}

/// Fattore di misura: valore numerico o espressione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fattore {
    /// Valore esatto.
    Valore(Decimal),
    /// Espressione aritmetica, valutata a ogni calcolo.
    Espressione(String),
}

impl Fattore {
    /// Valore numerico del fattore.
    ///
    /// # Errors
    /// Errore di formula se l'espressione non è valida.
    pub fn valuta(&self) -> Result<Decimal, ErroreFormula> {
        match self {
            Self::Valore(v) => Ok(*v),
            Self::Espressione(e) => formula::valuta(e),
        }
    }
}

impl From<Decimal> for Fattore {
    fn from(valore: Decimal) -> Self {
        Self::Valore(valore)
    }
}

impl From<&str> for Fattore {
    fn from(espressione: &str) -> Self {
        Self::Espressione(espressione.to_owned())
    }
}

/// Tipo di rigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoRigo {
    /// Rigo che produce una quantità.
    #[default]
    Misura,
    /// Rigo descrittivo, privo di quantità.
    Commento,
}

/// Segno del contributo del rigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegnoRigo {
    /// Contributo positivo.
    #[default]
    Positivo,
    /// Deduzione (vuoto per pieno, detrazioni…): contributo negativo.
    Deduzione,
}

/// Rigo di misurazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigoMisura {
    /// Identificativo stabile.
    pub id: RigoMisuraId,
    /// Tipo di rigo.
    #[serde(default)]
    pub tipo: TipoRigo,
    /// Descrizione di riferimento (es. «muro asse A, piano terra»).
    #[serde(default)]
    pub descrizione: String,
    /// Parti uguali.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parti_uguali: Option<Fattore>,
    /// Lunghezza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lunghezza: Option<Fattore>,
    /// Larghezza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub larghezza: Option<Fattore>,
    /// Altezza o peso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altezza_peso: Option<Fattore>,
    /// Formula che sostituisce il prodotto dei fattori.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// Segno del contributo.
    #[serde(default)]
    pub segno: SegnoRigo,
    /// «Vedi voce»: quantità di un'altra voce da cui parte il rigo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vedi_voce: Option<VediVoce>,
}

/// Risultato del calcolo di un rigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RisultatoRigo {
    /// Valore esatto con segno, prima di ogni arrotondamento.
    pub valore_esatto: Decimal,
    /// Quantità del rigo con segno, arrotondata se le regole lo prevedono.
    pub quantita: Quantita,
}

/// Contesto delle variabili di un rigo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContestoRigo {
    /// Parti uguali.
    pub parti_uguali: Option<Decimal>,
    /// Lunghezza.
    pub lunghezza: Option<Decimal>,
    /// Larghezza.
    pub larghezza: Option<Decimal>,
    /// Altezza o peso.
    pub altezza_peso: Option<Decimal>,
    /// Quantità della voce citata («vedi voce»).
    pub vedi_voce: Option<Decimal>,
}

impl ContestoFormula for ContestoRigo {
    fn variabile(&self, nome: &str) -> Result<Decimal, ErroreFormula> {
        let valore = match nome {
            "par" => self.parti_uguali,
            "lun" => self.lunghezza,
            "lar" => self.larghezza,
            "alt" => self.altezza_peso,
            VARIABILE_VEDI_VOCE => self.vedi_voce,
            _ => {
                return Err(ErroreFormula::VariabileSconosciuta {
                    nome: nome.to_owned(),
                });
            }
        };
        valore.ok_or_else(|| ErroreFormula::VariabileNonValorizzata {
            nome: nome.to_owned(),
        })
    }
}

fn valuta_opzionale(f: Option<&Fattore>) -> Result<Option<Decimal>, ErroreFormula> {
    f.map(Fattore::valuta).transpose()
}

impl RigoMisura {
    /// Nuovo rigo di misura vuoto.
    #[must_use]
    pub fn misura(id: RigoMisuraId, descrizione: impl Into<String>) -> Self {
        Self {
            id,
            tipo: TipoRigo::Misura,
            descrizione: descrizione.into(),
            parti_uguali: None,
            lunghezza: None,
            larghezza: None,
            altezza_peso: None,
            formula: None,
            segno: SegnoRigo::Positivo,
            vedi_voce: None,
        }
    }

    /// Nuovo rigo di commento.
    #[must_use]
    pub fn commento(id: RigoMisuraId, testo: impl Into<String>) -> Self {
        Self {
            tipo: TipoRigo::Commento,
            ..Self::misura(id, testo)
        }
    }

    /// Imposta le parti uguali.
    #[must_use]
    pub fn con_parti(mut self, f: impl Into<Fattore>) -> Self {
        self.parti_uguali = Some(f.into());
        self
    }

    /// Imposta la lunghezza.
    #[must_use]
    pub fn con_lunghezza(mut self, f: impl Into<Fattore>) -> Self {
        self.lunghezza = Some(f.into());
        self
    }

    /// Imposta la larghezza.
    #[must_use]
    pub fn con_larghezza(mut self, f: impl Into<Fattore>) -> Self {
        self.larghezza = Some(f.into());
        self
    }

    /// Imposta l'altezza o il peso.
    #[must_use]
    pub fn con_altezza(mut self, f: impl Into<Fattore>) -> Self {
        self.altezza_peso = Some(f.into());
        self
    }

    /// Imposta la formula.
    #[must_use]
    pub fn con_formula(mut self, formula: impl Into<String>) -> Self {
        self.formula = Some(formula.into());
        self
    }

    /// Imposta il «vedi voce».
    #[must_use]
    pub fn con_vedi_voce(mut self, v: VediVoce) -> Self {
        self.vedi_voce = Some(v);
        self
    }

    /// Marca il rigo come deduzione.
    #[must_use]
    pub fn come_deduzione(mut self) -> Self {
        self.segno = SegnoRigo::Deduzione;
        self
    }

    /// `true` se almeno un fattore, la formula o un «vedi voce» sono valorizzati.
    #[must_use]
    pub fn ha_valori(&self) -> bool {
        self.fattori().iter().any(Option::is_some)
            || self.formula_effettiva().is_some()
            || self.vedi_voce.is_some()
    }

    fn fattori(&self) -> [Option<&Fattore>; 4] {
        [
            self.parti_uguali.as_ref(),
            self.lunghezza.as_ref(),
            self.larghezza.as_ref(),
            self.altezza_peso.as_ref(),
        ]
    }

    fn formula_effettiva(&self) -> Option<&str> {
        self.formula.as_deref().filter(|f| !f.trim().is_empty())
    }

    /// Valuta i fattori del rigo (la quantità citata da un «vedi voce» statico
    /// compresa; quella di una voce collegata no, vedi [`Self::contesto_con`]).
    ///
    /// # Errors
    /// Errore di formula se un'espressione non è valida.
    pub fn contesto(&self) -> Result<ContestoRigo, ErroreFormula> {
        Ok(ContestoRigo {
            parti_uguali: valuta_opzionale(self.parti_uguali.as_ref())?,
            lunghezza: valuta_opzionale(self.lunghezza.as_ref())?,
            larghezza: valuta_opzionale(self.larghezza.as_ref())?,
            altezza_peso: valuta_opzionale(self.altezza_peso.as_ref())?,
            vedi_voce: match &self.vedi_voce {
                Some(VediVoce::Statica { quantita, .. }) => Some(*quantita),
                _ => None,
            },
        })
    }

    /// Valuta i fattori del rigo e la quantità citata dal «vedi voce».
    ///
    /// # Errors
    /// Errore di formula o voce citata non disponibile.
    pub fn contesto_con(
        &self,
        riferite: &dyn QuantitaRiferite,
    ) -> Result<ContestoRigo, ErroreCalcolo> {
        let mut c = self.contesto()?;
        if let Some(VediVoce::Collegata(id)) = &self.vedi_voce {
            c.vedi_voce = Some(riferite.quantita(*id)?);
        }
        Ok(c)
    }

    /// Calcola il rigo fuori da un computo: un «vedi voce» collegato è un errore.
    /// Restituisce `None` per i righi di commento.
    ///
    /// # Errors
    /// Errori di formula o di overflow, «vedi voce» collegato.
    pub fn calcola(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<Option<RisultatoRigo>, ErroreCalcolo> {
        self.calcola_con(regole, &SenzaRiferimenti)
    }

    /// Calcola il rigo con le quantità delle voci citate. Restituisce `None` per i
    /// righi di commento.
    ///
    /// # Errors
    /// Errori di formula o di overflow, voce citata non disponibile.
    pub fn calcola_con(
        &self,
        regole: &RegoleArrotondamento,
        riferite: &dyn QuantitaRiferite,
    ) -> Result<Option<RisultatoRigo>, ErroreCalcolo> {
        if self.tipo == TipoRigo::Commento {
            return Ok(None);
        }
        let contesto = self.contesto_con(riferite)?;
        let valore = if let Some(f) = self.formula_effettiva() {
            formula::valuta_con(f, &contesto)?
        } else {
            let presenti = [
                contesto.vedi_voce,
                contesto.parti_uguali,
                contesto.lunghezza,
                contesto.larghezza,
                contesto.altezza_peso,
            ];
            let mut prodotto: Option<Decimal> = None;
            for v in presenti.into_iter().flatten() {
                prodotto = Some(match prodotto {
                    None => v,
                    Some(p) => p.checked_mul(v).ok_or(ErroreCalcolo::Overflow)?,
                });
            }
            prodotto.unwrap_or(Decimal::ZERO)
        };
        let valore_esatto = match self.segno {
            SegnoRigo::Positivo => valore,
            SegnoRigo::Deduzione => -valore,
        };
        let quantita = if regole.arrotonda_quantita_parziali {
            regole.arrotonda_quantita(valore_esatto)
        } else {
            Quantita::new(valore_esatto)
        };
        Ok(Some(RisultatoRigo {
            valore_esatto,
            quantita,
        }))
    }
}

/// Quantità totale di una serie di righi, arrotondata secondo le regole.
///
/// # Errors
/// Errore localizzato sul rigo che non è stato possibile calcolare.
pub fn quantita_totale(
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
) -> Result<Quantita, ErroreLocalizzato> {
    quantita_totale_con(righi, regole, &SenzaRiferimenti)
}

/// Quantità totale con le quantità delle voci citate dai «vedi voce».
///
/// # Errors
/// Errore localizzato sul rigo che non è stato possibile calcolare.
pub fn quantita_totale_con(
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
    riferite: &dyn QuantitaRiferite,
) -> Result<Quantita, ErroreLocalizzato> {
    Ok(dettaglio_quantita_con(righi, regole, riferite)?.totale)
}

/// Dettaglio della quantità di una serie di righi, per le stampe che riportano i
/// progressivi e le somme separate di positivi e deduzioni.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DettaglioQuantita {
    /// Risultato di ogni rigo (`None` per i commenti).
    pub righi: Vec<Option<RisultatoRigo>>,
    /// Quantità progressiva dopo ogni rigo, arrotondata come il totale: l'ultima
    /// coincide con il totale.
    pub progressivi: Vec<Quantita>,
    /// Somma dei righi positivi, arrotondata come il totale.
    pub positivi: Quantita,
    /// Somma delle deduzioni (negativa o nulla), arrotondata come il totale.
    pub deduzioni: Quantita,
    /// Quantità totale (come [`quantita_totale`]).
    pub totale: Quantita,
}

/// Calcola il [`DettaglioQuantita`] di una serie di righi.
///
/// # Errors
/// Errore localizzato sul rigo che non è stato possibile calcolare.
pub fn dettaglio_quantita(
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
) -> Result<DettaglioQuantita, ErroreLocalizzato> {
    dettaglio_quantita_con(righi, regole, &SenzaRiferimenti)
}

/// Calcola il [`DettaglioQuantita`] con le quantità delle voci citate dai «vedi voce».
///
/// # Errors
/// Errore localizzato sul rigo che non è stato possibile calcolare.
pub fn dettaglio_quantita_con(
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
    riferite: &dyn QuantitaRiferite,
) -> Result<DettaglioQuantita, ErroreLocalizzato> {
    let mut somma = Decimal::ZERO;
    let mut positivi = Decimal::ZERO;
    let mut deduzioni = Decimal::ZERO;
    let mut risultati = Vec::with_capacity(righi.len());
    let mut progressivi = Vec::with_capacity(righi.len());
    for (i, rigo) in righi.iter().enumerate() {
        let rif = || Riferimento::Rigo {
            numero: i + 1,
            id: rigo.id,
        };
        let risultato = rigo
            .calcola_con(regole, riferite)
            .map_err(|e| ErroreLocalizzato::new(rif(), e))?;
        if let Some(r) = risultato {
            let overflow = || ErroreLocalizzato::new(rif(), ErroreCalcolo::Overflow);
            let q = r.quantita.valore();
            somma = somma.checked_add(q).ok_or_else(overflow)?;
            if rigo.segno == SegnoRigo::Deduzione {
                deduzioni = deduzioni.checked_add(q).ok_or_else(overflow)?;
            } else {
                positivi = positivi.checked_add(q).ok_or_else(overflow)?;
            }
        }
        risultati.push(risultato);
        progressivi.push(regole.arrotonda_quantita(somma));
    }
    Ok(DettaglioQuantita {
        righi: risultati,
        progressivi,
        positivi: regole.arrotonda_quantita(positivi),
        deduzioni: regole.arrotonda_quantita(deduzioni),
        totale: regole.arrotonda_quantita(somma),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn id(n: u128) -> RigoMisuraId {
        RigoMisuraId::da_u128(n)
    }

    fn regole() -> RegoleArrotondamento {
        RegoleArrotondamento::default()
    }

    fn q(r: &RigoMisura) -> Decimal {
        r.calcola(&regole()).unwrap().unwrap().quantita.valore()
    }

    #[test]
    fn prodotto_dei_fattori_presenti() {
        let r = RigoMisura::misura(id(1), "muro")
            .con_parti(dec!(2))
            .con_lunghezza(dec!(3.50))
            .con_larghezza(dec!(0.30))
            .con_altezza(dec!(2.00));
        assert_eq!(q(&r), dec!(4.20));
        let solo_lunghezza = RigoMisura::misura(id(2), "").con_lunghezza(dec!(12.5));
        assert_eq!(q(&solo_lunghezza), dec!(12.50));
    }

    #[test]
    fn fattori_come_espressioni() {
        let r = RigoMisura::misura(id(1), "")
            .con_lunghezza("3,50+0,30*2")
            .con_altezza(dec!(2));
        assert_eq!(q(&r), dec!(8.20));
    }

    #[test]
    fn formula_con_variabili() {
        let r = RigoMisura::misura(id(1), "pilastro circolare")
            .con_parti(dec!(4))
            .con_lunghezza(dec!(0.25))
            .con_altezza(dec!(3))
            .con_formula("par * pi * lun^2 * alt");
        // 4 × π × 0,0625 × 3 = 2,35619… → 2,36
        assert_eq!(q(&r), dec!(2.36));
        let mancante = RigoMisura::misura(id(2), "").con_formula("lun*lar");
        assert!(matches!(
            mancante.calcola(&regole()),
            Err(ErroreCalcolo::Formula(
                ErroreFormula::VariabileNonValorizzata { .. }
            ))
        ));
    }

    #[test]
    fn deduzioni_e_commenti() {
        let d = RigoMisura::misura(id(1), "a dedurre finestra")
            .con_lunghezza(dec!(1.20))
            .con_altezza(dec!(1.50))
            .come_deduzione();
        assert_eq!(q(&d), dec!(-1.80));
        let c = RigoMisura::commento(id(2), "piano terra");
        assert_eq!(c.calcola(&regole()).unwrap(), None);
    }

    #[test]
    fn rigo_vuoto_vale_zero() {
        let r = RigoMisura::misura(id(1), "da completare");
        assert!(!r.ha_valori());
        assert_eq!(q(&r), dec!(0));
    }

    #[test]
    fn arrotondamento_dei_parziali() {
        // Due righi da 1,005: arrotondati singolarmente 1,01 + 1,01 = 2,02;
        // senza arrotondamento dei parziali 2,010 → 2,01.
        let righi = vec![
            RigoMisura::misura(id(1), "").con_lunghezza(dec!(1.005)),
            RigoMisura::misura(id(2), "").con_lunghezza(dec!(1.005)),
        ];
        assert_eq!(
            quantita_totale(&righi, &regole()).unwrap(),
            Quantita::new(dec!(2.02))
        );
        let senza = RegoleArrotondamento {
            arrotonda_quantita_parziali: false,
            ..regole()
        };
        assert_eq!(
            quantita_totale(&righi, &senza).unwrap(),
            Quantita::new(dec!(2.01))
        );
    }

    #[test]
    fn errore_localizzato_sul_rigo() {
        let righi = vec![
            RigoMisura::misura(id(1), "").con_lunghezza(dec!(1)),
            RigoMisura::misura(id(7), "").con_lunghezza("2/0"),
        ];
        let e = quantita_totale(&righi, &regole()).unwrap_err();
        assert_eq!(
            e.percorso,
            vec![Riferimento::Rigo {
                numero: 2,
                id: id(7)
            }]
        );
        assert_eq!(
            e.errore,
            ErroreCalcolo::Formula(ErroreFormula::DivisionePerZero)
        );
        assert_eq!(
            e.to_string(),
            "rigo n. 2: formula non valida: divisione per zero"
        );
    }

    #[test]
    fn dettaglio_con_progressivi_e_deduzioni() {
        let righi = vec![
            RigoMisura {
                lunghezza: Some(Fattore::Valore(dec!(10))),
                larghezza: Some(Fattore::Valore(dec!(2))),
                ..RigoMisura::misura(id(1), "a")
            },
            RigoMisura::commento(id(2), "nota"),
            RigoMisura {
                lunghezza: Some(Fattore::Valore(dec!(1.5))),
                segno: SegnoRigo::Deduzione,
                ..RigoMisura::misura(id(3), "vano")
            },
            RigoMisura {
                parti_uguali: Some(Fattore::Espressione("2*2".into())),
                lunghezza: Some(Fattore::Valore(dec!(0.25))),
                ..RigoMisura::misura(id(4), "b")
            },
        ];
        let d = dettaglio_quantita(&righi, &regole()).unwrap();
        assert_eq!(d.righi.len(), 4);
        assert!(d.righi[1].is_none());
        let p: Vec<Decimal> = d.progressivi.iter().map(|q| q.valore()).collect();
        assert_eq!(p, vec![dec!(20), dec!(20), dec!(18.5), dec!(19.5)]);
        assert_eq!(d.positivi.valore(), dec!(21));
        assert_eq!(d.deduzioni.valore(), dec!(-1.5));
        assert_eq!(d.totale.valore(), dec!(19.5));
        assert_eq!(quantita_totale(&righi, &regole()).unwrap(), d.totale);
    }
}
