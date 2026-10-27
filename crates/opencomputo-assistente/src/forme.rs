//! Catalogo chiuso delle forme di rigo (ADR 0025, decisione 4).
//!
//! Il modello indica una forma e i suoi parametri; non scrive espressioni. Rust controlla
//! i parametri, li converte nell'unità base e scrive il rigo con i fattori del core:
//! parti uguali (`par`), lunghezza (`lun`), larghezza (`lar`), altezza o peso (`alt`).
//! Per le forme che non sono un prodotto la formula del rigo è generata qui, sempre
//! dalle stesse poche espressioni costanti.
//!
//! | Forma | Parametri | Fattori del rigo | Formula | Risultato |
//! |---|---|---|---|---|
//! | `conteggio` | N | par = N | | numero |
//! | `lunghezza` | N, L | par, lun | | m |
//! | `superficie` | N, A | par, lun = A | | m² |
//! | `rettangolo` | N, L, B | par, lun, lar | | m² |
//! | `triangolo` | N, B, H | par, lun = B, lar = H | `par*lun*lar/2` | m² |
//! | `trapezio` | N, B1, B2, H | par, lun = B1, lar = B2, alt = H | `par*(lun+lar)*alt/2` | m² |
//! | `cerchio` | N, D | par, lun = D | `par*pi*lun^2/4` | m² |
//! | `volume` | N, V | par, lun = V | | m³ |
//! | `parallelepipedo` | N, L, B, H | par, lun, lar, alt | | m³ |
//! | `cilindro` | N, D, H | par, lun = D, alt = H | `par*pi*lun^2/4*alt` | m³ |
//! | `prisma` | N, A, H | par, lun = A, alt = H | | m³ |
//! | `massa_lineare` | N, L, P | par, lun, alt = P | | kg |
//! | `a_corpo` | nessuno | par = 1 | | a corpo |
//!
//! Se l'unità della voce non è l'unità base (es. tonnellate), la formula divide il
//! risultato per il fattore dell'unità.

use std::collections::BTreeMap;

use opencomputo_application::NuovoRigo;
use opencomputo_application::comandi::FattoreIngresso;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::unita::Dimensione;
use crate::valori::UnitaValore;

/// Forma di rigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Forma {
    /// N elementi.
    Conteggio,
    /// N × L.
    Lunghezza,
    /// N × A (superficie già nota).
    Superficie,
    /// N × L × B.
    Rettangolo,
    /// N × B × H / 2.
    Triangolo,
    /// N × (B1 + B2) × H / 2.
    Trapezio,
    /// N × π × D² / 4.
    Cerchio,
    /// N × V (volume già noto).
    Volume,
    /// N × L × B × H.
    Parallelepipedo,
    /// N × π × D² / 4 × H.
    Cilindro,
    /// N × A × H.
    Prisma,
    /// N × L × P (P in kg/m).
    MassaLineare,
    /// Compenso a corpo: 1.
    ACorpo,
}

/// Parametro di una forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Parametro {
    /// Numero di parti uguali.
    N,
    /// Lunghezza.
    L,
    /// Larghezza o base.
    B,
    /// Altezza o spessore.
    H,
    /// Diametro.
    D,
    /// Base minore del trapezio.
    B1,
    /// Base maggiore del trapezio.
    B2,
    /// Superficie (area di base nel prisma).
    A,
    /// Volume.
    V,
    /// Peso per metro.
    P,
}

impl Parametro {
    /// Unità ammesse per il parametro.
    #[must_use]
    pub fn unita_ammesse(self) -> &'static [UnitaValore] {
        match self {
            Self::N => &[UnitaValore::N],
            Self::L | Self::B | Self::H | Self::D | Self::B1 | Self::B2 => {
                &[UnitaValore::Mm, UnitaValore::Cm, UnitaValore::M]
            }
            Self::A => &[UnitaValore::M2],
            Self::V => &[UnitaValore::M3],
            Self::P => &[UnitaValore::KgM],
        }
    }
}

impl Forma {
    /// Tutte le forme, nell'ordine dello schema.
    pub const TUTTE: [Self; 13] = [
        Self::Conteggio,
        Self::Lunghezza,
        Self::Superficie,
        Self::Rettangolo,
        Self::Triangolo,
        Self::Trapezio,
        Self::Cerchio,
        Self::Volume,
        Self::Parallelepipedo,
        Self::Cilindro,
        Self::Prisma,
        Self::MassaLineare,
        Self::ACorpo,
    ];

    /// Nome nello schema JSON.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Self::Conteggio => "conteggio",
            Self::Lunghezza => "lunghezza",
            Self::Superficie => "superficie",
            Self::Rettangolo => "rettangolo",
            Self::Triangolo => "triangolo",
            Self::Trapezio => "trapezio",
            Self::Cerchio => "cerchio",
            Self::Volume => "volume",
            Self::Parallelepipedo => "parallelepipedo",
            Self::Cilindro => "cilindro",
            Self::Prisma => "prisma",
            Self::MassaLineare => "massa_lineare",
            Self::ACorpo => "a_corpo",
        }
    }

    /// Parametri richiesti, tutti obbligatori.
    #[must_use]
    pub fn parametri(self) -> &'static [Parametro] {
        use Parametro as P;
        match self {
            Self::Conteggio => &[P::N],
            Self::Lunghezza => &[P::N, P::L],
            Self::Superficie => &[P::N, P::A],
            Self::Rettangolo => &[P::N, P::L, P::B],
            Self::Triangolo => &[P::N, P::B, P::H],
            Self::Trapezio => &[P::N, P::B1, P::B2, P::H],
            Self::Cerchio => &[P::N, P::D],
            Self::Volume => &[P::N, P::V],
            Self::Parallelepipedo => &[P::N, P::L, P::B, P::H],
            Self::Cilindro => &[P::N, P::D, P::H],
            Self::Prisma => &[P::N, P::A, P::H],
            Self::MassaLineare => &[P::N, P::L, P::P],
            Self::ACorpo => &[],
        }
    }

    /// Dimensione del risultato.
    #[must_use]
    pub fn dimensione(self) -> Dimensione {
        match self {
            Self::Conteggio => Dimensione::Conteggio,
            Self::Lunghezza => Dimensione::Lunghezza,
            Self::Superficie
            | Self::Rettangolo
            | Self::Triangolo
            | Self::Trapezio
            | Self::Cerchio => Dimensione::Area,
            Self::Volume | Self::Parallelepipedo | Self::Cilindro | Self::Prisma => {
                Dimensione::Volume
            }
            Self::MassaLineare => Dimensione::Massa,
            Self::ACorpo => Dimensione::ACorpo,
        }
    }

    /// Forme compatibili con una dimensione.
    #[must_use]
    pub fn per_dimensione(d: Dimensione) -> Vec<Self> {
        Self::TUTTE
            .into_iter()
            .filter(|f| f.dimensione() == d)
            .collect()
    }

    /// Formula propria della forma, se non è il prodotto dei fattori.
    fn formula_propria(self) -> Option<&'static str> {
        match self {
            Self::Triangolo => Some("par*lun*lar/2"),
            Self::Trapezio => Some("par*(lun+lar)*alt/2"),
            Self::Cerchio => Some("par*pi*lun^2/4"),
            Self::Cilindro => Some("par*pi*lun^2/4*alt"),
            _ => None,
        }
    }
}

/// Rigo di misura con i parametri già controllati e convertiti nell'unità base.
///
/// `fattore_voce` è il numero di unità base in una unità della voce (1 per m, m², m³,
/// kg, cad; 1000 per la tonnellata).
#[must_use]
pub fn rigo(
    forma: Forma,
    valori: &BTreeMap<Parametro, Decimal>,
    fattore_voce: Decimal,
    descrizione: &str,
    deduzione: bool,
) -> NuovoRigo {
    let v = |p: Parametro| valori.get(&p).copied().map(FattoreIngresso::Numero);
    let (par, lun, lar, alt) = match forma {
        Forma::Conteggio => (v(Parametro::N), None, None, None),
        Forma::Lunghezza => (v(Parametro::N), v(Parametro::L), None, None),
        Forma::Superficie => (v(Parametro::N), v(Parametro::A), None, None),
        Forma::Volume => (v(Parametro::N), v(Parametro::V), None, None),
        Forma::Rettangolo => (v(Parametro::N), v(Parametro::L), v(Parametro::B), None),
        Forma::Triangolo => (v(Parametro::N), v(Parametro::B), v(Parametro::H), None),
        Forma::Trapezio => (
            v(Parametro::N),
            v(Parametro::B1),
            v(Parametro::B2),
            v(Parametro::H),
        ),
        Forma::Cerchio => (v(Parametro::N), v(Parametro::D), None, None),
        Forma::Parallelepipedo => (
            v(Parametro::N),
            v(Parametro::L),
            v(Parametro::B),
            v(Parametro::H),
        ),
        Forma::Cilindro => (v(Parametro::N), v(Parametro::D), None, v(Parametro::H)),
        Forma::Prisma => (v(Parametro::N), v(Parametro::A), None, v(Parametro::H)),
        Forma::MassaLineare => (v(Parametro::N), v(Parametro::L), None, v(Parametro::P)),
        Forma::ACorpo => (
            Some(FattoreIngresso::Numero(Decimal::ONE)),
            None,
            None,
            None,
        ),
    };
    let formula = if fattore_voce == Decimal::ONE {
        forma.formula_propria().map(str::to_owned)
    } else {
        let base = forma.formula_propria().map_or_else(
            || {
                [("par", &par), ("lun", &lun), ("lar", &lar), ("alt", &alt)]
                    .into_iter()
                    .filter(|(_, f)| f.is_some())
                    .map(|(n, _)| n)
                    .collect::<Vec<_>>()
                    .join("*")
            },
            str::to_owned,
        );
        Some(format!("({base})/{}", fattore_voce.normalize()))
    };
    NuovoRigo {
        descrizione: descrizione.to_owned(),
        parti_uguali: par,
        lunghezza: lun,
        larghezza: lar,
        altezza_peso: alt,
        formula,
        deduzione,
        ..NuovoRigo::default()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn ogni_forma_ha_parametri_con_unita() {
        for f in Forma::TUTTE {
            for p in f.parametri() {
                assert!(!p.unita_ammesse().is_empty(), "{f:?} {p:?}");
            }
            assert_ne!(f.dimensione(), Dimensione::Sconosciuta);
        }
        assert_eq!(
            Forma::per_dimensione(Dimensione::Area),
            [
                Forma::Superficie,
                Forma::Rettangolo,
                Forma::Triangolo,
                Forma::Trapezio,
                Forma::Cerchio
            ]
        );
        assert!(Forma::per_dimensione(Dimensione::Tempo).is_empty());
    }

    #[test]
    fn righi_delle_forme() {
        let valori: BTreeMap<_, _> = [
            (Parametro::N, dec!(2)),
            (Parametro::D, dec!(0.5)),
            (Parametro::H, dec!(3)),
        ]
        .into();
        let r = rigo(Forma::Cilindro, &valori, Decimal::ONE, "pilastri", false);
        assert_eq!(r.parti_uguali, Some(FattoreIngresso::Numero(dec!(2))));
        assert_eq!(r.lunghezza, Some(FattoreIngresso::Numero(dec!(0.5))));
        assert_eq!(r.larghezza, None);
        assert_eq!(r.altezza_peso, Some(FattoreIngresso::Numero(dec!(3))));
        assert_eq!(r.formula.as_deref(), Some("par*pi*lun^2/4*alt"));

        let valori: BTreeMap<_, _> = [
            (Parametro::N, dec!(1)),
            (Parametro::L, dec!(12)),
            (Parametro::P, dec!(7.85)),
        ]
        .into();
        let r = rigo(Forma::MassaLineare, &valori, dec!(1000), "", true);
        assert_eq!(r.formula.as_deref(), Some("(par*lun*alt)/1000"));
        assert!(r.deduzione);

        let r = rigo(Forma::ACorpo, &BTreeMap::new(), Decimal::ONE, "", false);
        assert_eq!(r.parti_uguali, Some(FattoreIngresso::Numero(Decimal::ONE)));
        assert_eq!(r.formula, None);
    }
}
