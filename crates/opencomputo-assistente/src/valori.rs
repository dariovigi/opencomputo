//! Valori numerici presenti nella richiesta dell'utente («valori fondati»).
//!
//! L'estrazione è deterministica: ogni numero del testo, con la virgola o il punto
//! decimale, e l'unità che lo segue se riconosciuta (`12 m`, `10 cm`, `30 mq`,
//! `4,50 metri`). Nelle forme `3 x 4 m` l'unità finale vale per tutti i numeri.
//!
//! Un parametro che il modello dichiara fornito dall'utente è accettato solo se coincide
//! con un valore fondato: stesso valore nell'unità base (10 cm = 0,10 m) oppure, per i
//! numeri senza unità, stesso numero.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::unita::Dimensione;

/// Unità di un valore della richiesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitaValore {
    /// Millimetri.
    Mm,
    /// Centimetri.
    Cm,
    /// Metri.
    M,
    /// Metri quadrati.
    M2,
    /// Metri cubi.
    M3,
    /// Chilogrammi.
    Kg,
    /// Chilogrammi per metro.
    KgM,
    /// Numero di elementi.
    N,
}

impl UnitaValore {
    /// Dimensione e fattore verso l'unità base (m, m², m³, kg, kg/m, numero).
    #[must_use]
    pub fn base(self) -> (Dimensione, Decimal) {
        match self {
            Self::Mm => (Dimensione::Lunghezza, Decimal::new(1, 3)),
            Self::Cm => (Dimensione::Lunghezza, Decimal::new(1, 2)),
            Self::M => (Dimensione::Lunghezza, Decimal::ONE),
            Self::M2 => (Dimensione::Area, Decimal::ONE),
            Self::M3 => (Dimensione::Volume, Decimal::ONE),
            Self::Kg => (Dimensione::Massa, Decimal::ONE),
            // Massa lineare: dimensione propria, rappresentata come massa per metro.
            Self::KgM => (Dimensione::Sconosciuta, Decimal::ONE),
            Self::N => (Dimensione::Conteggio, Decimal::ONE),
        }
    }

    /// Valore nell'unità base.
    #[must_use]
    pub fn in_base(self, valore: Decimal) -> Decimal {
        valore * self.base().1
    }
}

/// Valore trovato nel testo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValoreFondato {
    /// Testo così come compare (es. `4,50 m`).
    pub testo: String,
    /// Valore numerico.
    pub valore: Decimal,
    /// Unità, se riconosciuta.
    pub unita: Option<UnitaValore>,
}

impl ValoreFondato {
    /// `true` se il valore coincide con `valore` espresso in `unita`.
    #[must_use]
    pub fn corrisponde(&self, valore: Decimal, unita: UnitaValore) -> bool {
        match self.unita {
            Some(u) => {
                u.base().0 == unita.base().0 && u.in_base(self.valore) == unita.in_base(valore)
            }
            None => self.valore == valore,
        }
    }
}

fn unita_da_parole(p1: &str, p2: Option<&str>) -> Option<(UnitaValore, usize)> {
    let p1 = p1.trim_end_matches([',', '.']).to_lowercase();
    let p2 = p2.map(|x| x.trim_end_matches([',', '.']).to_lowercase());
    let due = p2.as_deref();
    let quadri = matches!(due, Some("quadrati" | "quadri" | "quadrato" | "quadro"));
    let cubi = matches!(due, Some("cubi" | "cubo"));
    Some(match p1.as_str() {
        "mm" | "millimetri" | "millimetro" => (UnitaValore::Mm, 1),
        "cm" | "centimetri" | "centimetro" => (UnitaValore::Cm, 1),
        "m" | "mt" | "ml" | "metri" | "metro" if quadri => (UnitaValore::M2, 2),
        "m" | "mt" | "metri" | "metro" if cubi => (UnitaValore::M3, 2),
        "m" | "mt" | "ml" | "metri" | "metro" => (UnitaValore::M, 1),
        "m2" | "m²" | "mq" => (UnitaValore::M2, 1),
        "m3" | "m³" | "mc" => (UnitaValore::M3, 1),
        "kg/m" | "kg/ml" => (UnitaValore::KgM, 1),
        "kg" | "chili" | "chilogrammi" => (UnitaValore::Kg, 1),
        "n" | "nr" | "pz" | "pezzi" | "cad" => (UnitaValore::N, 1),
        _ => return None,
    })
}

/// Parola del testo con la posizione di inizio e fine (in byte).
fn spezza(testo: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut inizio = None;
    for (i, c) in testo.char_indices() {
        let parte = c.is_alphanumeric() || matches!(c, ',' | '.' | '/' | '²' | '³');
        match (parte, inizio) {
            (true, None) => inizio = Some(i),
            (false, Some(s)) => {
                if let Some(p) = testo.get(s..i) {
                    out.push((s, i, p));
                }
                inizio = None;
            }
            _ => {}
        }
    }
    if let Some(s) = inizio
        && let Some(p) = testo.get(s..)
    {
        out.push((s, testo.len(), p));
    }
    out
}

/// Numero in testa a una parola (`4,50`, `12m`, `3x4`), con il resto della parola.
fn numero_in_testa(parola: &str) -> Option<(Decimal, &str)> {
    let fine = parola
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == ',' || *c == '.'))
        .map_or(parola.len(), |(i, _)| i);
    let cifre = parola.get(..fine)?.trim_end_matches(['.', ',']);
    if cifre.is_empty() || !cifre.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let separatori = cifre.chars().filter(|c| matches!(c, ',' | '.')).count();
    if separatori > 1 {
        return None;
    }
    let valore = Decimal::from_str_exact(&cifre.replace(',', ".")).ok()?;
    Some((
        valore,
        parola.get(cifre.len()..)?.trim_start_matches(['.', ',']),
    ))
}

/// Valori numerici della richiesta, nell'ordine in cui compaiono.
#[must_use]
pub fn estrai(testo: &str) -> Vec<ValoreFondato> {
    let parole = spezza(testo);
    let mut out: Vec<ValoreFondato> = Vec::new();
    // Numeri in attesa di un'unità comune (`3 x 4 m`): indici in `out`.
    let mut in_attesa: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while let Some(&(s, e, p)) = parole.get(i) {
        let Some((valore, resto)) = numero_in_testa(p) else {
            in_attesa.clear();
            i += 1;
            continue;
        };
        // `3x4`: due numeri nella stessa parola.
        if let Some(dopo_x) = resto.strip_prefix(['x', 'X', '×'])
            && let Some((secondo, resto2)) = numero_in_testa(dopo_x)
        {
            out.push(ValoreFondato {
                testo: p.to_owned(),
                valore,
                unita: None,
            });
            in_attesa.push(out.len() - 1);
            out.push(ValoreFondato {
                testo: p.to_owned(),
                valore: secondo,
                unita: None,
            });
            in_attesa.push(out.len() - 1);
            i += 1;
            let unita = if resto2.is_empty() {
                let p1 = parole.get(i).map(|x| x.2);
                let p2 = parole.get(i + 1).map(|x| x.2);
                let trovata = p1.and_then(|p1| unita_da_parole(p1, p2));
                if let Some((_, consumate)) = trovata {
                    i += consumate;
                }
                trovata.map(|(u, _)| u)
            } else {
                unita_da_parole(resto2, None).map(|(u, _)| u)
            };
            if let Some(u) = unita {
                for &j in &in_attesa {
                    if let Some(v) = out.get_mut(j)
                        && v.unita.is_none()
                    {
                        v.unita = Some(u);
                    }
                }
                in_attesa.clear();
            }
            continue;
        }
        let mut unita = None;
        let mut fine = e;
        let mut avanzo = 1;
        if !resto.is_empty() {
            unita = unita_da_parole(resto, None).map(|(u, _)| u);
        } else if let Some(&(_, e1, p1)) = parole.get(i + 1) {
            let p2 = parole.get(i + 2).map(|x| x.2);
            if let Some((u, consumate)) = unita_da_parole(p1, p2) {
                unita = Some(u);
                avanzo += consumate;
                fine = if consumate == 2 {
                    parole.get(i + 2).map_or(e1, |x| x.1)
                } else {
                    e1
                };
            }
        }
        out.push(ValoreFondato {
            testo: testo
                .get(s..fine)
                .unwrap_or(p)
                .trim_end_matches([',', '.', ';', ':'])
                .to_owned(),
            valore,
            unita,
        });
        if let Some(u) = unita {
            for &j in &in_attesa {
                if let Some(v) = out.get_mut(j)
                    && v.unita.is_none()
                {
                    v.unita = Some(u);
                }
            }
            in_attesa.clear();
            i += avanzo;
        } else {
            in_attesa.push(out.len() - 1);
            i += avanzo;
            // Una «x» tra due numeri mantiene l'attesa; ogni altra parola la chiude.
            match parole.get(i).map(|x| x.2) {
                Some("x" | "X" | "per" | "×") => i += 1,
                Some(q) if numero_in_testa(q).is_some() => {}
                _ => in_attesa.clear(),
            }
        }
    }
    out
}

#[cfg(test)]
mod test {
    use super::*;
    use rust_decimal_macros::dec;

    fn coppie(t: &str) -> Vec<(Decimal, Option<UnitaValore>)> {
        estrai(t).into_iter().map(|v| (v.valore, v.unita)).collect()
    }

    #[test]
    fn numeri_e_unita() {
        assert_eq!(
            coppie("Massetto di 120 mq, spessore 6 cm, su 2 piani"),
            [
                (dec!(120), Some(UnitaValore::M2)),
                (dec!(6), Some(UnitaValore::Cm)),
                (dec!(2), None)
            ]
        );
        assert_eq!(
            coppie("parete lunga 4,50 metri e alta 3 m"),
            [
                (dec!(4.50), Some(UnitaValore::M)),
                (dec!(3), Some(UnitaValore::M))
            ]
        );
        assert_eq!(
            coppie("30 metri quadrati"),
            [(dec!(30), Some(UnitaValore::M2))]
        );
        assert_eq!(coppie("scavo di 12m3"), [(dec!(12), Some(UnitaValore::M3))]);
    }

    #[test]
    fn dimensioni_moltiplicate() {
        assert_eq!(
            coppie("stanza 3 x 4 m"),
            [
                (dec!(3), Some(UnitaValore::M)),
                (dec!(4), Some(UnitaValore::M))
            ]
        );
        assert_eq!(
            coppie("vano 3,20x4,10 m"),
            [
                (dec!(3.20), Some(UnitaValore::M)),
                (dec!(4.10), Some(UnitaValore::M))
            ]
        );
    }

    #[test]
    fn testi_senza_numeri_e_numeri_non_validi() {
        assert!(estrai("rifacimento del bagno").is_empty());
        assert!(estrai("versione 1.2.3").is_empty());
        assert_eq!(coppie("fine frase 12."), [(dec!(12), None)]);
    }

    #[test]
    fn corrispondenza_nell_unita_base() {
        let v = ValoreFondato {
            testo: "10 cm".into(),
            valore: dec!(10),
            unita: Some(UnitaValore::Cm),
        };
        assert!(v.corrisponde(dec!(0.10), UnitaValore::M));
        assert!(v.corrisponde(dec!(100), UnitaValore::Mm));
        assert!(!v.corrisponde(dec!(10), UnitaValore::M));
        assert!(!v.corrisponde(dec!(0.10), UnitaValore::M2));
        let n = ValoreFondato {
            testo: "2".into(),
            valore: dec!(2),
            unita: None,
        };
        assert!(n.corrisponde(dec!(2), UnitaValore::N));
        assert!(n.corrisponde(dec!(2), UnitaValore::M));
        assert!(!n.corrisponde(dec!(2.5), UnitaValore::M));
    }
}
