//! Formattazione dei numeri e delle date per la stampa, secondo l'uso italiano.
//!
//! I valori arrivano dal core già arrotondati: qui **non si arrotonda mai**. La
//! scala viene solo estesa fino ai decimali richiesti (`12.5` con 2 decimali →
//! `12,50`); un valore con più decimali è scritto per intero.

use chrono::NaiveDate;
use rust_decimal::Decimal;

/// Numero con separatore delle migliaia `.` e decimale `,` (es. `14.457,12`).
#[must_use]
pub fn numero(valore: Decimal, decimali: u32) -> String {
    componi(valore, decimali, Some('.'), ',')
}

/// Numero senza separatore delle migliaia, con il separatore decimale indicato
/// (per i file CSV, dove i separatori delle migliaia ostacolano la rilettura).
#[must_use]
pub fn numero_semplice(valore: Decimal, decimali: u32, decimale: char) -> String {
    componi(valore, decimali, None, decimale)
}

fn componi(valore: Decimal, decimali: u32, migliaia: Option<char>, decimale: char) -> String {
    let mut v = valore;
    if v.scale() < decimali {
        v.rescale(decimali);
    }
    let testo = v.abs().to_string();
    let (intera, frazione) = testo.split_once('.').unwrap_or((testo.as_str(), ""));
    let mut out = String::with_capacity(testo.len() + 8);
    if v.is_sign_negative() && !v.is_zero() {
        out.push('-');
    }
    let cifre = intera.len();
    for (i, c) in intera.chars().enumerate() {
        if i > 0
            && (cifre - i) % 3 == 0
            && let Some(m) = migliaia
        {
            out.push(m);
        }
        out.push(c);
    }
    if !frazione.is_empty() {
        out.push(decimale);
        out.push_str(frazione);
    }
    out
}

/// Numero con separatore delle migliaia `´` e decimale `,` (es. `14´457,12`), come nelle
/// stampe tradizionali dei computi.
#[must_use]
pub fn numero_apice(valore: Decimal, decimali: u32) -> String {
    componi(valore, decimali, Some('´'), ',')
}

/// Importo in lettere alla maniera dei prezzi di elenco: parte intera in lettere e
/// decimali in cifre dopo la barra (es. `117,53` → `centodiciassette/53`). La scala è
/// estesa ai decimali indicati, mai ridotta; un valore negativo è preceduto da «meno».
#[must_use]
pub fn in_lettere(valore: Decimal, decimali: u32) -> String {
    let mut v = valore.abs();
    if v.scale() < decimali {
        v.rescale(decimali);
    }
    let testo = v.to_string();
    let (intera, frazione) = testo.split_once('.').unwrap_or((testo.as_str(), ""));
    let parole = intera
        .parse::<u128>()
        .map_or_else(|_| intera.to_owned(), lettere_intero);
    let segno = if valore.is_sign_negative() && !valore.is_zero() {
        "meno "
    } else {
        ""
    };
    if frazione.is_empty() {
        format!("{segno}{parole}")
    } else {
        format!("{segno}{parole}/{frazione}")
    }
}

const UNITA: [&str; 20] = [
    "zero",
    "uno",
    "due",
    "tre",
    "quattro",
    "cinque",
    "sei",
    "sette",
    "otto",
    "nove",
    "dieci",
    "undici",
    "dodici",
    "tredici",
    "quattordici",
    "quindici",
    "sedici",
    "diciassette",
    "diciotto",
    "diciannove",
];
const DECINE: [&str; 10] = [
    "",
    "",
    "venti",
    "trenta",
    "quaranta",
    "cinquanta",
    "sessanta",
    "settanta",
    "ottanta",
    "novanta",
];

fn unita(n: u128) -> &'static str {
    usize::try_from(n)
        .ok()
        .and_then(|i| UNITA.get(i))
        .copied()
        .unwrap_or("")
}

/// Numero da 0 a 999 in lettere (senza accento finale).
fn lettere_centinaia(n: u128) -> String {
    let (c, resto) = (n / 100, n % 100);
    let mut s = match c {
        0 => String::new(),
        1 => "cento".to_owned(),
        _ => format!("{}cento", unita(c)),
    };
    if c > 0 && (resto == 8 || resto / 10 == 8) {
        // Elisione: centotto, duecentottanta.
        s.pop();
    }
    if resto == 0 {
        if s.is_empty() {
            s.push_str("zero");
        }
        return s;
    }
    if resto < 20 {
        s.push_str(unita(resto));
        return s;
    }
    let (d, u) = (resto / 10, resto % 10);
    let decina = usize::try_from(d)
        .ok()
        .and_then(|i| DECINE.get(i))
        .copied()
        .unwrap_or("");
    if u == 1 || u == 8 {
        // Elisione: ventuno, trentotto.
        s.push_str(decina.get(..decina.len() - 1).unwrap_or(decina));
    } else {
        s.push_str(decina);
    }
    if u > 0 {
        s.push_str(unita(u));
    }
    s
}

/// Numero intero in lettere, unito come negli assegni (es. `duemilatrecentoventitré`).
fn lettere_intero(n: u128) -> String {
    if n == 0 {
        return "zero".to_owned();
    }
    let gruppi = [
        (1_000_000_000_000_000_000u128, "", ""),
        (1_000_000_000_000_000, "unbiliardo", "biliardi"),
        (1_000_000_000_000, "unbilione", "bilioni"),
        (1_000_000_000, "unmiliardo", "miliardi"),
        (1_000_000, "unmilione", "milioni"),
        (1_000, "mille", "mila"),
    ];
    let mut s = String::new();
    let mut resto = n;
    for (base, uno, molti) in gruppi {
        let q = resto / base;
        resto %= base;
        if q == 0 {
            continue;
        }
        if uno.is_empty() {
            // Oltre i biliardi: cifre.
            s.push_str(&q.to_string());
            continue;
        }
        if q == 1 {
            s.push_str(uno);
        } else {
            s.push_str(&lettere_intero(q));
            s.push_str(molti);
        }
    }
    if resto > 0 {
        s.push_str(&lettere_centinaia(resto));
    }
    // «tre» finale accentato nei composti (ventitré, centotré, milletré).
    if s.len() > 3 && s.ends_with("tre") {
        s.truncate(s.len() - 3);
        s.push_str("tré");
    }
    s
}

/// Data nel formato `gg/mm/aaaa`.
#[must_use]
pub fn data(d: NaiveDate) -> String {
    d.format("%d/%m/%Y").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn numeri_italiani() {
        assert_eq!(numero(dec!(14457.12), 2), "14.457,12");
        assert_eq!(numero(dec!(1234567.5), 2), "1.234.567,50");
        assert_eq!(numero(dec!(-994.2), 2), "-994,20");
        assert_eq!(numero(dec!(0), 2), "0,00");
        assert_eq!(numero(dec!(-0.00), 2), "0,00");
        assert_eq!(numero(dec!(999), 0), "999");
        assert_eq!(numero(dec!(1000), 0), "1.000");
        assert_eq!(numero(dec!(3.744), 2), "3,744");
        assert_eq!(numero_semplice(dec!(14457.1), 2, ','), "14457,10");
        assert_eq!(numero_semplice(dec!(-1234.5), 2, '.'), "-1234.50");
        assert_eq!(
            data(NaiveDate::from_ymd_opt(2026, 4, 5).unwrap()),
            "05/04/2026"
        );
    }

    #[test]
    fn numeri_con_apice_e_in_lettere() {
        assert_eq!(numero_apice(dec!(126024.29), 2), "126´024,29");
        assert_eq!(numero_apice(dec!(-348.16), 2), "-348,16");
        assert_eq!(in_lettere(dec!(117.53), 2), "centodiciassette/53");
        assert_eq!(in_lettere(dec!(99.24), 2), "novantanove/24");
        assert_eq!(in_lettere(dec!(0.5), 2), "zero/50");
        assert_eq!(in_lettere(dec!(21), 2), "ventuno/00");
        assert_eq!(in_lettere(dec!(28.1), 2), "ventotto/10");
        assert_eq!(in_lettere(dec!(23), 0), "ventitré");
        assert_eq!(in_lettere(dec!(3), 0), "tre");
        assert_eq!(in_lettere(dec!(103), 0), "centotré");
        assert_eq!(in_lettere(dec!(1000), 0), "mille");
        assert_eq!(in_lettere(dec!(1003), 0), "milletré");
        assert_eq!(in_lettere(dec!(2323.45), 2), "duemilatrecentoventitré/45");
        assert_eq!(in_lettere(dec!(21000), 0), "ventunomila");
        assert_eq!(
            in_lettere(dec!(1250000), 0),
            "unmilioneduecentocinquantamila"
        );
        assert_eq!(in_lettere(dec!(3000000), 0), "tremilioni");
        assert_eq!(in_lettere(dec!(180), 0), "centottanta");
        assert_eq!(in_lettere(dec!(108), 0), "centotto");
        assert_eq!(in_lettere(dec!(1001), 0), "milleuno");
        assert_eq!(in_lettere(dec!(2000), 0), "duemila");
        assert_eq!(in_lettere(dec!(1.234), 2), "uno/234");
        assert_eq!(in_lettere(dec!(-5), 2), "meno cinque/00");
    }
}
