//! Grandezze numeriche e regole di arrotondamento.
//!
//! Tutti i valori contabili sono rappresentati con [`Decimal`] (aritmetica
//! decimale a 96 bit, 28 cifre significative) e mai con `f32`/`f64`. Il
//! workspace vieta l'aritmetica in virgola mobile con il lint
//! `clippy::float_arithmetic`.
//!
//! Le grandezze sono incapsulate in tipi distinti ([`Quantita`], [`Prezzo`],
//! [`Importo`], [`Percentuale`]) per impedire, a livello di compilazione,
//! di sommare un prezzo unitario a un importo o di usare una quantità al posto
//! di una percentuale.
//!
//! ## Regole di arrotondamento
//!
//! Le regole sono esplicite e configurabili per progetto ([`RegoleArrotondamento`]).
//! Il comportamento predefinito è:
//!
//! * quantità di ciascun rigo di misura arrotondata a 2 decimali;
//! * quantità totale della voce = somma dei righi arrotondati, arrotondata a 2 decimali;
//! * importo della voce = quantità totale × prezzo unitario, arrotondato a 2 decimali;
//! * totali = somma esatta degli importi delle voci già arrotondati
//!   (nessun ulteriore arrotondamento, quindi nessuna deriva tra righe e totali);
//! * modalità «commerciale»: metà arrotondata per eccesso in valore assoluto
//!   (2,345 → 2,35; −2,345 → −2,35).
//!
//! Gli ingressi (dimensioni, prezzi unitari di listino) non vengono mai
//! arrotondati: vengono conservati esattamente come forniti.

use core::fmt;
use core::str::FromStr;

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::errore::ErroreCalcolo;

macro_rules! grandezza_decimale {
    ($(#[$meta:meta])* $nome:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $nome(Decimal);

        impl $nome {
            /// Valore nullo.
            pub const ZERO: Self = Self(Decimal::ZERO);

            /// Crea la grandezza da un valore decimale esatto, senza arrotondamenti.
            #[must_use]
            pub const fn new(valore: Decimal) -> Self {
                Self(valore)
            }

            /// Valore decimale esatto.
            #[must_use]
            pub const fn valore(self) -> Decimal {
                self.0
            }

            /// `true` se il valore è zero.
            #[must_use]
            pub fn is_zero(self) -> bool {
                self.0.is_zero()
            }

            /// Come [`Self::is_zero`] per riferimento (per `skip_serializing_if`).
            #[must_use]
            #[allow(clippy::trivially_copy_pass_by_ref)]
            pub fn zero_ref(valore: &Self) -> bool {
                valore.is_zero()
            }

            /// `true` se il valore è strettamente negativo.
            #[must_use]
            pub fn is_negativo(self) -> bool {
                self.0 < Decimal::ZERO
            }

            /// Valore con segno opposto.
            #[must_use]
            pub fn opposto(self) -> Self {
                Self(-self.0)
            }

            /// Somma controllata.
            ///
            /// # Errors
            /// [`ErroreCalcolo::Overflow`] se il risultato eccede la capacità del tipo.
            pub fn somma(self, altro: Self) -> Result<Self, ErroreCalcolo> {
                self.0.checked_add(altro.0).map(Self).ok_or(ErroreCalcolo::Overflow)
            }

            /// Differenza controllata (`self - altro`).
            ///
            /// # Errors
            /// [`ErroreCalcolo::Overflow`] se il risultato eccede la capacità del tipo.
            pub fn differenza(self, altro: Self) -> Result<Self, ErroreCalcolo> {
                self.0.checked_sub(altro.0).map(Self).ok_or(ErroreCalcolo::Overflow)
            }

            /// Somma controllata di una sequenza di valori.
            ///
            /// # Errors
            /// [`ErroreCalcolo::Overflow`] se un risultato parziale eccede la capacità del tipo.
            pub fn totale<I: IntoIterator<Item = Self>>(valori: I) -> Result<Self, ErroreCalcolo> {
                valori.into_iter().try_fold(Self::ZERO, Self::somma)
            }
        }

        impl fmt::Display for $nome {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl From<Decimal> for $nome {
            fn from(valore: Decimal) -> Self {
                Self(valore)
            }
        }

        impl FromStr for $nome {
            type Err = ErroreCalcolo;

            /// Interpreta un numero nel formato interno (separatore decimale `.`).
            /// La localizzazione italiana (virgola decimale) è compito dei livelli di I/O.
            fn from_str(testo: &str) -> Result<Self, Self::Err> {
                Decimal::from_str_exact(testo.trim()).map(Self).map_err(|_| {
                    ErroreCalcolo::ValoreNonValido(format!(
                        "«{testo}» non è un numero decimale valido"
                    ))
                })
            }
        }
    };
}

grandezza_decimale!(
    /// Quantità misurata (m, m², m³, kg, cad…). Può essere negativa (deduzioni).
    Quantita
);
grandezza_decimale!(
    /// Prezzo unitario in euro per unità di misura.
    Prezzo
);
grandezza_decimale!(
    /// Importo monetario in euro.
    Importo
);
grandezza_decimale!(
    /// Percentuale espressa in centesimi: `12.5` significa 12,5 %.
    Percentuale
);

/// Scala minima con cui gli importi vengono serializzati (`"0.00"`, `"12.50"`).
///
/// La scala viene solo estesa, mai ridotta: un importo con più decimali (ad esempio
/// in un progetto configurato con 3 decimali) è scritto esattamente com'è.
pub const SCALA_MINIMA_IMPORTO_SERIALIZZATO: u32 = 2;

impl Serialize for Quantita {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Serialize::serialize(&self.0, serializer)
    }
}

impl Serialize for Prezzo {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Serialize::serialize(&self.0, serializer)
    }
}

impl Serialize for Percentuale {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Serialize::serialize(&self.0, serializer)
    }
}

impl Serialize for Importo {
    /// Serializza con almeno [`SCALA_MINIMA_IMPORTO_SERIALIZZATO`] decimali.
    /// L'estensione della scala non altera il valore.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut valore = self.0;
        if valore.scale() < SCALA_MINIMA_IMPORTO_SERIALIZZATO {
            valore.rescale(SCALA_MINIMA_IMPORTO_SERIALIZZATO);
        }
        Serialize::serialize(&valore, serializer)
    }
}

impl Prezzo {
    /// Importo = prezzo × quantità, arrotondato secondo le regole.
    ///
    /// # Errors
    /// [`ErroreCalcolo::Overflow`] se il prodotto eccede la capacità del tipo.
    pub fn per_quantita(
        self,
        quantita: Quantita,
        regole: &RegoleArrotondamento,
    ) -> Result<Importo, ErroreCalcolo> {
        let prodotto = self
            .0
            .checked_mul(quantita.0)
            .ok_or(ErroreCalcolo::Overflow)?;
        Ok(regole.arrotonda_importo(prodotto))
    }
}

impl Percentuale {
    /// Cento per cento.
    pub const CENTO: Self = Self(Decimal::ONE_HUNDRED);

    /// `true` se la percentuale è compresa tra 0 e 100 inclusi.
    #[must_use]
    pub fn nell_intervallo_0_100(self) -> bool {
        self.0 >= Decimal::ZERO && self.0 <= Decimal::ONE_HUNDRED
    }

    /// Quota percentuale di un importo, arrotondata secondo le regole:
    /// `importo × percentuale / 100`.
    ///
    /// # Errors
    /// [`ErroreCalcolo::Overflow`] se il prodotto eccede la capacità del tipo.
    pub fn quota_di(
        self,
        importo: Importo,
        regole: &RegoleArrotondamento,
    ) -> Result<Importo, ErroreCalcolo> {
        let prodotto = importo
            .0
            .checked_mul(self.0)
            .ok_or(ErroreCalcolo::Overflow)?
            .checked_div(Decimal::ONE_HUNDRED)
            .ok_or(ErroreCalcolo::Overflow)?;
        Ok(regole.arrotonda_importo(prodotto))
    }

    /// Rapporto percentuale `parte / totale × 100`, arrotondato secondo le regole.
    ///
    /// # Errors
    /// [`ErroreCalcolo::DivisionePerZero`] se `totale` è zero.
    pub fn rapporto(
        parte: Importo,
        totale: Importo,
        regole: &RegoleArrotondamento,
    ) -> Result<Self, ErroreCalcolo> {
        if totale.is_zero() {
            return Err(ErroreCalcolo::DivisionePerZero);
        }
        let r = parte
            .0
            .checked_mul(Decimal::ONE_HUNDRED)
            .ok_or(ErroreCalcolo::Overflow)?
            .checked_div(totale.0)
            .ok_or(ErroreCalcolo::Overflow)?;
        Ok(regole.arrotonda_percentuale(r))
    }
}

/// Criterio di arrotondamento al numero di decimali richiesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalitaArrotondamento {
    /// Metà arrotondata lontano da zero («commerciale»): 2,345 → 2,35.
    #[default]
    Commerciale,
    /// Metà arrotondata al pari («bancario»): 2,345 → 2,34; 2,355 → 2,36.
    MetaAlPari,
    /// Troncamento verso zero: 2,349 → 2,34.
    Troncamento,
}

impl ModalitaArrotondamento {
    const fn strategia(self) -> RoundingStrategy {
        match self {
            Self::Commerciale => RoundingStrategy::MidpointAwayFromZero,
            Self::MetaAlPari => RoundingStrategy::MidpointNearestEven,
            Self::Troncamento => RoundingStrategy::ToZero,
        }
    }
}

/// Regole di precisione e arrotondamento applicate ai calcoli di un progetto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegoleArrotondamento {
    /// Decimali delle quantità (righi e totali di voce).
    pub decimali_quantita: u32,
    /// Decimali dei prezzi unitari calcolati (es. da analisi prezzi).
    pub decimali_prezzo: u32,
    /// Decimali degli importi.
    pub decimali_importo: u32,
    /// Decimali delle percentuali calcolate (es. incidenze).
    pub decimali_percentuale: u32,
    /// Criterio di arrotondamento.
    pub modalita: ModalitaArrotondamento,
    /// Se `true` ogni rigo di misura è arrotondato prima della somma; altrimenti
    /// si sommano i valori esatti e si arrotonda solo il totale della voce.
    pub arrotonda_quantita_parziali: bool,
}

impl Default for RegoleArrotondamento {
    fn default() -> Self {
        Self {
            decimali_quantita: 2,
            decimali_prezzo: 2,
            decimali_importo: 2,
            decimali_percentuale: 2,
            modalita: ModalitaArrotondamento::Commerciale,
            arrotonda_quantita_parziali: true,
        }
    }
}

impl RegoleArrotondamento {
    /// Numero massimo di decimali ammesso per qualunque grandezza.
    pub const DECIMALI_MASSIMI: u32 = 10;

    /// Verifica la coerenza delle regole.
    ///
    /// # Errors
    /// [`ErroreCalcolo::RegolaNonValida`] se un numero di decimali eccede
    /// [`Self::DECIMALI_MASSIMI`].
    pub fn valida(&self) -> Result<(), ErroreCalcolo> {
        let campi = [
            ("quantità", self.decimali_quantita),
            ("prezzi", self.decimali_prezzo),
            ("importi", self.decimali_importo),
            ("percentuali", self.decimali_percentuale),
        ];
        for (nome, decimali) in campi {
            if decimali > Self::DECIMALI_MASSIMI {
                return Err(ErroreCalcolo::RegolaNonValida(format!(
                    "{decimali} decimali per le {nome}: il massimo è {}",
                    Self::DECIMALI_MASSIMI
                )));
            }
        }
        Ok(())
    }

    /// Arrotonda un valore al numero di decimali indicato e ne fissa la scala,
    /// così che `5` diventi `5.00` e la rappresentazione sia uniforme.
    #[must_use]
    pub fn arrotonda(&self, valore: Decimal, decimali: u32) -> Decimal {
        let mut r = valore.round_dp_with_strategy(decimali, self.modalita.strategia());
        r.rescale(decimali);
        // Niente «-0,00»: uno zero arrotondato è sempre positivo.
        if r.is_zero() {
            r.set_sign_positive(true);
        }
        r
    }

    /// Arrotonda una quantità.
    #[must_use]
    pub fn arrotonda_quantita(&self, valore: Decimal) -> Quantita {
        Quantita(self.arrotonda(valore, self.decimali_quantita))
    }

    /// Arrotonda un prezzo unitario calcolato.
    #[must_use]
    pub fn arrotonda_prezzo(&self, valore: Decimal) -> Prezzo {
        Prezzo(self.arrotonda(valore, self.decimali_prezzo))
    }

    /// Arrotonda un importo.
    #[must_use]
    pub fn arrotonda_importo(&self, valore: Decimal) -> Importo {
        Importo(self.arrotonda(valore, self.decimali_importo))
    }

    /// Arrotonda una percentuale calcolata.
    #[must_use]
    pub fn arrotonda_percentuale(&self, valore: Decimal) -> Percentuale {
        Percentuale(self.arrotonda(valore, self.decimali_percentuale))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn nessuno_zero_negativo_dopo_l_arrotondamento() {
        let r = RegoleArrotondamento::default();
        for v in [dec!(-0.001), dec!(-0.004), -Decimal::ZERO, dec!(-0.00)] {
            let q = r.arrotonda_quantita(v);
            assert!(!q.valore().is_sign_negative(), "{v}");
            assert_eq!(serde_json::to_string(&q).unwrap(), "\"0.00\"");
            assert!(!r.arrotonda_importo(v).valore().is_sign_negative());
        }
        assert_eq!(r.arrotonda_quantita(dec!(-0.005)).valore(), dec!(-0.01));
    }

    #[test]
    fn importi_serializzati_con_almeno_due_decimali() {
        let json = |v: Decimal| serde_json::to_string(&Importo::new(v)).unwrap();
        assert_eq!(json(Decimal::ZERO), r#""0.00""#);
        assert_eq!(json(dec!(12.5)), r#""12.50""#);
        assert_eq!(json(dec!(-3)), r#""-3.00""#);
        assert_eq!(json(dec!(1.234)), r#""1.234""#);
        // Le altre grandezze conservano la scala d'ingresso.
        assert_eq!(
            serde_json::to_string(&Quantita::new(dec!(12))).unwrap(),
            r#""12""#
        );
        assert_eq!(
            serde_json::to_string(&Prezzo::new(dec!(8.5))).unwrap(),
            r#""8.5""#
        );
        // La lettura accetta qualsiasi scala e il valore resta identico.
        let letto: Importo = serde_json::from_str(r#""0""#).unwrap();
        assert_eq!(letto, Importo::ZERO);
        assert_eq!(serde_json::to_string(&letto).unwrap(), r#""0.00""#);
    }

    fn regole(modalita: ModalitaArrotondamento) -> RegoleArrotondamento {
        RegoleArrotondamento {
            modalita,
            ..RegoleArrotondamento::default()
        }
    }

    #[test]
    fn arrotondamento_commerciale() {
        let r = regole(ModalitaArrotondamento::Commerciale);
        assert_eq!(r.arrotonda(dec!(2.345), 2), dec!(2.35));
        assert_eq!(r.arrotonda(dec!(-2.345), 2), dec!(-2.35));
        assert_eq!(r.arrotonda(dec!(1.005), 2), dec!(1.01));
        assert_eq!(r.arrotonda(dec!(2.344), 2), dec!(2.34));
    }

    #[test]
    fn arrotondamento_al_pari() {
        let r = regole(ModalitaArrotondamento::MetaAlPari);
        assert_eq!(r.arrotonda(dec!(2.345), 2), dec!(2.34));
        assert_eq!(r.arrotonda(dec!(2.355), 2), dec!(2.36));
    }

    #[test]
    fn troncamento() {
        let r = regole(ModalitaArrotondamento::Troncamento);
        assert_eq!(r.arrotonda(dec!(2.349), 2), dec!(2.34));
        assert_eq!(r.arrotonda(dec!(-2.349), 2), dec!(-2.34));
    }

    #[test]
    fn la_scala_viene_fissata() {
        let r = RegoleArrotondamento::default();
        assert_eq!(r.arrotonda_importo(dec!(5)).to_string(), "5.00");
        assert_eq!(r.arrotonda_quantita(dec!(1.5)).to_string(), "1.50");
    }

    #[test]
    fn importo_da_prezzo_e_quantita() {
        let r = RegoleArrotondamento::default();
        let importo = Prezzo::new(dec!(12.345))
            .per_quantita(Quantita::new(dec!(10.13)), &r)
            .unwrap();
        // 10,13 × 12,345 = 125,05485
        assert_eq!(importo, Importo::new(dec!(125.05)));
    }

    #[test]
    fn quota_percentuale() {
        let r = RegoleArrotondamento::default();
        let q = Percentuale::new(dec!(0.5))
            .quota_di(Importo::new(dec!(9625.00)), &r)
            .unwrap();
        // 9625 × 0,5 % = 48,125 → 48,13
        assert_eq!(q, Importo::new(dec!(48.13)));
    }

    #[test]
    fn rapporto_percentuale_e_divisione_per_zero() {
        let r = RegoleArrotondamento::default();
        let p =
            Percentuale::rapporto(Importo::new(dec!(71)), Importo::new(dec!(236.74)), &r).unwrap();
        assert_eq!(p, Percentuale::new(dec!(29.99)));
        assert_eq!(
            Percentuale::rapporto(Importo::new(dec!(1)), Importo::ZERO, &r),
            Err(ErroreCalcolo::DivisionePerZero)
        );
    }

    #[test]
    fn overflow_esplicito() {
        let grande = Importo::new(Decimal::MAX);
        assert_eq!(
            grande.somma(Importo::new(dec!(1))),
            Err(ErroreCalcolo::Overflow)
        );
        assert_eq!(
            Importo::totale([grande, grande]),
            Err(ErroreCalcolo::Overflow)
        );
    }

    #[test]
    fn regole_non_valide() {
        let r = RegoleArrotondamento {
            decimali_importo: 11,
            ..RegoleArrotondamento::default()
        };
        assert!(matches!(r.valida(), Err(ErroreCalcolo::RegolaNonValida(_))));
        assert!(RegoleArrotondamento::default().valida().is_ok());
    }

    #[test]
    fn parsing_formato_interno() {
        assert_eq!("12.50".parse::<Prezzo>().unwrap(), Prezzo::new(dec!(12.5)));
        assert!("12,50".parse::<Prezzo>().is_err());
        assert!("abc".parse::<Importo>().is_err());
    }

    #[test]
    fn serializzazione_come_stringa_esatta() {
        let json = serde_json::to_string(&Importo::new(dec!(0.10))).unwrap();
        assert_eq!(json, "\"0.10\"");
        let letto: Importo = serde_json::from_str("\"1234567890123456.78\"").unwrap();
        assert_eq!(letto.valore(), dec!(1234567890123456.78));
    }
}
