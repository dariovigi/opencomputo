//! Unità di misura: dimensione fisica e fattore verso l'unità base.
//!
//! Unità base: metro, metro quadrato, metro cubo, chilogrammo, ora, pezzo. Le unità dei
//! prezzari si scrivono in molti modi (`mq`, `m²`, `m2`, `metri quadrati`): si
//! riconoscono dopo la normalizzazione. Un'unità non riconosciuta resta
//! [`Dimensione::Sconosciuta`] e la voce va misurata a mano.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::testo::parole;

/// Dimensione fisica di una grandezza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimensione {
    /// Lunghezza (m).
    Lunghezza,
    /// Superficie (m²).
    Area,
    /// Volume (m³).
    Volume,
    /// Massa (kg).
    Massa,
    /// Numero di elementi (cad, n., pezzi).
    Conteggio,
    /// Compenso a corpo.
    ACorpo,
    /// Tempo (h).
    Tempo,
    /// Percentuale.
    Percentuale,
    /// Unità non riconosciuta.
    Sconosciuta,
}

impl Dimensione {
    /// Nome nei messaggi.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Self::Lunghezza => "lunghezza",
            Self::Area => "superficie",
            Self::Volume => "volume",
            Self::Massa => "massa",
            Self::Conteggio => "numero",
            Self::ACorpo => "a corpo",
            Self::Tempo => "tempo",
            Self::Percentuale => "percentuale",
            Self::Sconosciuta => "sconosciuta",
        }
    }
}

/// Unità riconosciuta: dimensione e quante unità base vale una unità.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unita {
    /// Dimensione.
    pub dimensione: Dimensione,
    /// Unità base in una unità (es. 1000 per la tonnellata).
    pub fattore: Decimal,
}

fn u(dimensione: Dimensione, fattore: i64) -> Unita {
    Unita {
        dimensione,
        fattore: Decimal::from(fattore),
    }
}

/// Riconosce un'unità di misura scritta in un prezzario.
#[must_use]
pub fn riconosci(testo: &str) -> Unita {
    let p = parole(testo).join(" ");
    let compatto: String = p.chars().filter(|c| *c != ' ').collect();
    use Dimensione as D;
    match compatto.as_str() {
        "m" | "ml" | "mt" | "metro" | "metri" | "metrolineare" | "metrilineari" => {
            u(D::Lunghezza, 1)
        }
        "km" => u(D::Lunghezza, 1000),
        "m2" | "mq" | "metroquadrato" | "metriquadrati" | "metroquadro" | "metriquadri" => {
            u(D::Area, 1)
        }
        "m3" | "mc" | "metrocubo" | "metricubi" => u(D::Volume, 1),
        "kg" | "chilogrammo" | "chilogrammi" | "kilogrammo" | "kilogrammi" => u(D::Massa, 1),
        "t" | "ton" | "tonnellata" | "tonnellate" => u(D::Massa, 1000),
        "q" | "ql" | "qle" | "qli" | "quintale" | "quintali" => u(D::Massa, 100),
        "cad" | "cadauno" | "cadauna" | "n" | "nr" | "num" | "numero" | "pz" | "pezzo"
        | "pezzi" | "cd" => u(D::Conteggio, 1),
        "acorpo" | "corpo" => u(D::ACorpo, 1),
        "h" | "ora" | "ore" => u(D::Tempo, 1),
        "giorno" | "giorni" | "gg" => u(D::Tempo, 8),
        "%" | "percento" => u(D::Percentuale, 1),
        _ if testo.trim() == "%" => u(D::Percentuale, 1),
        _ => u(D::Sconosciuta, 1),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn unita_dei_prezzari() {
        for t in ["m²", "mq", "m2", "MQ", "metri quadrati"] {
            assert_eq!(riconosci(t).dimensione, Dimensione::Area, "{t}");
        }
        for t in ["m³", "mc", "m3"] {
            assert_eq!(riconosci(t).dimensione, Dimensione::Volume, "{t}");
        }
        for t in ["m", "ml", "m.l."] {
            assert_eq!(riconosci(t).dimensione, Dimensione::Lunghezza, "{t}");
        }
        for t in ["cad", "n.", "nr", "cad."] {
            assert_eq!(riconosci(t).dimensione, Dimensione::Conteggio, "{t}");
        }
        assert_eq!(riconosci("a corpo").dimensione, Dimensione::ACorpo);
        assert_eq!(riconosci("%").dimensione, Dimensione::Percentuale);
        assert_eq!(
            riconosci("t"),
            Unita {
                dimensione: Dimensione::Massa,
                fattore: dec!(1000)
            }
        );
        assert_eq!(riconosci("q.li").fattore, dec!(100));
        assert_eq!(riconosci("dm³").dimensione, Dimensione::Sconosciuta);
    }
}
