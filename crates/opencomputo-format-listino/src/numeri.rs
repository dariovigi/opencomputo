//! Numeri dei listini: prezzi e percentuali scritti all'italiana o con il punto.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Separatore decimale dei numeri del listino.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeparatoreDecimale {
    /// Riconosciuto dal contenuto: virgola se almeno un prezzo contiene una virgola,
    /// altrimenti punto. La scelta vale per tutto il file ed è riportata nel rapporto.
    #[default]
    Automatico,
    /// Virgola decimale; il punto separa le migliaia (`1.234,56`).
    Virgola,
    /// Punto decimale; la virgola separa le migliaia (`1,234.56`).
    Punto,
}

/// Interpreta un numero con il separatore indicato (non `Automatico`, che va risolto
/// prima per tutto il file). Ammessi spazi, il simbolo `€` e, se `percentuale`, il
/// simbolo `%`. I separatori delle migliaia devono separare gruppi di tre cifre.
///
/// Restituisce `None` se il testo non è un numero valido.
#[must_use]
pub fn interpreta(
    testo: &str,
    separatore: SeparatoreDecimale,
    percentuale: bool,
) -> Option<Decimal> {
    let mut t: String = testo
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '€')
        .collect();
    if percentuale && let Some(s) = t.strip_suffix('%') {
        t = s.to_owned();
    }
    let (decimale, migliaia) = match separatore {
        SeparatoreDecimale::Virgola | SeparatoreDecimale::Automatico => (',', '.'),
        SeparatoreDecimale::Punto => ('.', ','),
    };
    let (segno, corpo) = match t.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", t.strip_prefix('+').unwrap_or(&t)),
    };
    let mut parti = corpo.splitn(2, decimale);
    let intera = parti.next().unwrap_or("");
    let frazione = parti.next();
    if frazione.is_some_and(|f| f.is_empty() || !f.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    if intera.is_empty() {
        return None;
    }
    let intera_pulita = if intera.contains(migliaia) {
        let gruppi: Vec<&str> = intera.split(migliaia).collect();
        let (primo, altri) = gruppi.split_first()?;
        let primo_ok =
            !primo.is_empty() && primo.len() <= 3 && primo.chars().all(|c| c.is_ascii_digit());
        if !primo_ok
            || !altri
                .iter()
                .all(|g| g.len() == 3 && g.chars().all(|c| c.is_ascii_digit()))
        {
            return None;
        }
        gruppi.concat()
    } else {
        if !intera.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        intera.to_owned()
    };
    let normale = match frazione {
        Some(f) => format!("{segno}{intera_pulita}.{f}"),
        None => format!("{segno}{intera_pulita}"),
    };
    Decimal::from_str_exact(&normale).ok()
}

/// Risolve [`SeparatoreDecimale::Automatico`] su un insieme di testi di prezzo.
#[must_use]
pub fn risolvi<'a>(
    scelto: SeparatoreDecimale,
    prezzi: impl IntoIterator<Item = &'a str>,
) -> SeparatoreDecimale {
    match scelto {
        SeparatoreDecimale::Automatico => {
            if prezzi.into_iter().any(|p| p.contains(',')) {
                SeparatoreDecimale::Virgola
            } else {
                SeparatoreDecimale::Punto
            }
        }
        altro => altro,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn virgola_decimale() {
        let v = SeparatoreDecimale::Virgola;
        assert_eq!(interpreta("8,50", v, false), Some(dec!(8.50)));
        assert_eq!(interpreta("1.234,56", v, false), Some(dec!(1234.56)));
        assert_eq!(interpreta("1.234.567", v, false), Some(dec!(1234567)));
        assert_eq!(interpreta(" € 12,345 ", v, false), Some(dec!(12.345)));
        assert_eq!(interpreta("-3,5", v, false), Some(dec!(-3.5)));
        assert_eq!(interpreta("12,5%", v, true), Some(dec!(12.5)));
    }

    #[test]
    fn punto_decimale() {
        let p = SeparatoreDecimale::Punto;
        assert_eq!(interpreta("8.50", p, false), Some(dec!(8.50)));
        assert_eq!(interpreta("1,234.56", p, false), Some(dec!(1234.56)));
        assert_eq!(interpreta("0.125", p, false), Some(dec!(0.125)));
    }

    #[test]
    fn numeri_non_validi() {
        let v = SeparatoreDecimale::Virgola;
        for t in [
            "", "abc", "1.2,5", "12,", ",5", "1,2,3", "12.34.5", "1e3", "12%", "--1",
        ] {
            assert_eq!(interpreta(t, v, false), None, "«{t}»");
        }
        assert_eq!(interpreta("1.2.3", SeparatoreDecimale::Punto, false), None);
    }

    #[test]
    fn risoluzione_automatica() {
        assert_eq!(
            risolvi(SeparatoreDecimale::Automatico, ["8.50", "1,20"]),
            SeparatoreDecimale::Virgola
        );
        assert_eq!(
            risolvi(SeparatoreDecimale::Automatico, ["8.50", "12"]),
            SeparatoreDecimale::Punto
        );
        assert_eq!(
            risolvi(SeparatoreDecimale::Punto, ["1,20"]),
            SeparatoreDecimale::Punto
        );
    }
}
