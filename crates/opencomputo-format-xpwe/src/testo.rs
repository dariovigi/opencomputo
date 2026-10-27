//! Interpretazione dei campi testuali: numeri, fattori di misura, configurazione.

use core::str::FromStr;

use opencomputo_core::formula;
use opencomputo_core::misura::Fattore;
use rust_decimal::Decimal;

/// Numero decimale con `.` o `,` come separatore decimale.
///
/// * testo vuoto → `Ok(None)`;
/// * separatore delle migliaia non ammesso: un testo con sia `.` sia `,` è ambiguo
///   e viene rifiutato.
///
/// # Errors
/// Messaggio leggibile se il testo non è un numero.
pub fn decimale(testo: &str) -> Result<Option<Decimal>, String> {
    let t = testo.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.contains('.') && t.contains(',') {
        return Err(format!(
            "«{t}» è ambiguo: contiene sia il punto sia la virgola"
        ));
    }
    let normalizzato = t.replace(',', ".");
    let normalizzato = normalizzato.strip_prefix('+').unwrap_or(&normalizzato);
    Decimal::from_str_exact(normalizzato)
        .or_else(|_| Decimal::from_str(normalizzato))
        .map(Some)
        .map_err(|_| format!("«{t}» non è un numero"))
}

/// Numero intero non negativo (identificativi, flag).
#[must_use]
pub fn intero(testo: &str) -> Option<u64> {
    testo.trim().parse().ok()
}

/// Esito dell'interpretazione di un campo di misura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FattoreLetto {
    /// Fattore (valore o espressione).
    pub fattore: Fattore,
    /// L'espressione è stata riscritta (moltiplicazione implicita resa esplicita).
    pub normalizzato: bool,
    /// Errore di valutazione dell'espressione, se presente.
    pub errore: Option<String>,
}

/// Rende esplicite le moltiplicazioni implicite: `2(3+4)` → `2*(3+4)`,
/// `(1+2)(3)` → `(1+2)*(3)`.
#[must_use]
pub fn moltiplicazioni_esplicite(espressione: &str) -> (String, bool) {
    let mut out = String::with_capacity(espressione.len() + 4);
    let mut precedente: Option<char> = None;
    let mut cambiato = false;
    for c in espressione.chars() {
        if c == '('
            && let Some(p) = precedente
            && (p.is_ascii_digit() || p == ')')
        {
            out.push('*');
            cambiato = true;
        }
        out.push(c);
        if !c.is_whitespace() {
            precedente = Some(c);
        }
    }
    (out, cambiato)
}

/// Interpreta un campo di misura (parti uguali, lunghezza, larghezza, altezza/peso).
#[must_use]
pub fn fattore(testo: &str) -> Option<FattoreLetto> {
    let t = testo.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(Some(v)) = decimale(t) {
        return Some(FattoreLetto {
            fattore: Fattore::Valore(v),
            normalizzato: false,
            errore: None,
        });
    }
    let (espressione, normalizzato) = moltiplicazioni_esplicite(t);
    let errore = formula::valuta(&espressione).err().map(|e| e.to_string());
    Some(FattoreLetto {
        fattore: Fattore::Espressione(espressione),
        normalizzato,
        errore,
    })
}

/// Testo di un fattore per l'export.
#[must_use]
pub fn testo_fattore(f: &Fattore) -> String {
    match f {
        Fattore::Valore(v) => v.to_string(),
        Fattore::Espressione(e) => e.clone(),
    }
}

/// Decimali da un campo di `PweDGConfigNumeri` nella forma `L.D|x`
/// (es. `9.2|0` → 2). Interpretazione dedotta da LeenO, **non verificata**.
#[must_use]
pub fn decimali_configurazione(testo: &str) -> Option<u32> {
    let prima = testo.trim().split('|').next()?;
    let (_, decimali) = prima.split_once('.')?;
    let d: u32 = decimali.trim().parse().ok()?;
    (d <= 10).then_some(d)
}

/// Sostituisce i decimali in un campo `L.D|x`, conservando il resto.
#[must_use]
pub fn con_decimali_configurazione(originale: Option<&str>, decimali: u32) -> String {
    if let Some(o) = originale {
        let (prima, resto) = match o.split_once('|') {
            Some((p, r)) => (p, Some(r)),
            None => (o, None),
        };
        if let Some((larghezza, _)) = prima.split_once('.') {
            return match resto {
                Some(r) => format!("{larghezza}.{decimali}|{r}"),
                None => format!("{larghezza}.{decimali}"),
            };
        }
    }
    format!("9.{decimali}|0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn numeri() {
        assert_eq!(decimale(""), Ok(None));
        assert_eq!(decimale(" 12.50 "), Ok(Some(dec!(12.50))));
        assert_eq!(decimale("12,5"), Ok(Some(dec!(12.5))));
        assert_eq!(decimale("-0,30"), Ok(Some(dec!(-0.30))));
        assert_eq!(decimale("+3"), Ok(Some(dec!(3))));
        assert!(decimale("1.234,56").is_err());
        assert!(decimale("dodici").is_err());
        assert_eq!(intero(" 42 "), Some(42));
        assert_eq!(intero("-2"), None);
    }

    #[test]
    fn fattori() {
        let v = fattore("7,80").unwrap();
        assert_eq!(v.fattore, Fattore::Valore(dec!(7.80)));
        let e = fattore("2(3+4)").unwrap();
        assert_eq!(e.fattore, Fattore::Espressione("2*(3+4)".into()));
        assert!(e.normalizzato);
        assert!(e.errore.is_none());
        let e = fattore("(1+2) (3)").unwrap();
        assert_eq!(e.fattore, Fattore::Espressione("(1+2) *(3)".into()));
        let e = fattore("7,50+0,30").unwrap();
        assert!(!e.normalizzato);
        assert!(e.errore.is_none());
        assert!(fattore("2+*3").unwrap().errore.is_some());
        assert!(fattore("   ").is_none());
    }

    #[test]
    fn configurazione_numeri() {
        assert_eq!(decimali_configurazione("9.2|0"), Some(2));
        assert_eq!(decimali_configurazione("14.3"), Some(3));
        assert_eq!(decimali_configurazione("x"), None);
        assert_eq!(decimali_configurazione("9.99|0"), None);
        assert_eq!(con_decimali_configurazione(Some("9.2|0"), 3), "9.3|0");
        assert_eq!(con_decimali_configurazione(Some("14.2"), 2), "14.2");
        assert_eq!(con_decimali_configurazione(None, 2), "9.2|0");
    }
}
