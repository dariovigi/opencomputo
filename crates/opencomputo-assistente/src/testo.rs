//! Normalizzazione del testo italiano per la ricerca.
//!
//! * minuscole, senza accenti; `m²`/`m³` diventano `m2`/`m3`;
//! * parole: sequenze di lettere e cifre (`c25`, `30`); i separatori dividono;
//! * parole vuote (articoli, preposizioni) e parole di una sola lettera scartate;
//! * radice leggera: la vocale finale cade nelle parole di almeno quattro lettere, così
//!   singolare e plurale coincidono (`scavo`, `scavi` → `scav`; `demolizione`,
//!   `demolizioni` → `demolizion`).

/// Parole vuote dell'italiano tecnico.
const PAROLE_VUOTE: &[&str] = &[
    "di", "da", "in", "con", "su", "per", "tra", "fra", "il", "lo", "la", "gli", "le", "un", "uno",
    "una", "del", "dello", "della", "dei", "degli", "delle", "al", "allo", "alla", "ai", "agli",
    "alle", "dal", "dallo", "dalla", "dai", "dagli", "dalle", "nel", "nello", "nella", "nei",
    "negli", "nelle", "sul", "sullo", "sulla", "sui", "sugli", "sulle", "ed", "od", "che", "ogni",
    "come", "anche", "non", "sono", "tipo",
];

/// Carattere normalizzato: minuscolo, senza accento; `None` per i separatori.
fn normale(c: char) -> Option<char> {
    let c = c.to_lowercase().next().unwrap_or(c);
    Some(match c {
        'à' | 'á' | 'â' | 'ä' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        '²' => '2',
        '³' => '3',
        c if c.is_alphanumeric() => c,
        _ => return None,
    })
}

/// Parole normalizzate del testo, nell'ordine, comprese le parole vuote.
#[must_use]
pub fn parole(testo: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut corrente = String::new();
    for c in testo.chars() {
        match normale(c) {
            Some(n) => corrente.push(n),
            None => {
                if !corrente.is_empty() {
                    out.push(std::mem::take(&mut corrente));
                }
            }
        }
    }
    if !corrente.is_empty() {
        out.push(corrente);
    }
    out
}

/// Radice leggera di una parola già normalizzata.
#[must_use]
pub fn radice(parola: &str) -> String {
    let n = parola.chars().count();
    if n >= 4 && parola.ends_with(['a', 'e', 'i', 'o']) {
        let mut s = parola.to_owned();
        s.pop();
        s
    } else {
        parola.to_owned()
    }
}

/// Termini di ricerca: parole normalizzate, senza parole vuote né parole di una
/// lettera, ridotte alla radice.
#[must_use]
pub fn termini(testo: &str) -> Vec<String> {
    parole(testo)
        .into_iter()
        .filter(|p| p.chars().count() > 1 && !PAROLE_VUOTE.contains(&p.as_str()))
        .map(|p| radice(&p))
        .collect()
}

/// Trigrammi di un termine, con un segno di bordo ai due lati (`_sc`, `sca`, …, `vo_`).
#[must_use]
pub fn trigrammi(termine: &str) -> Vec<String> {
    let caratteri: Vec<char> = std::iter::once('_')
        .chain(termine.chars())
        .chain(std::iter::once('_'))
        .collect();
    let mut out: Vec<String> = caratteri
        .windows(3)
        .map(|w| w.iter().collect::<String>())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn normalizzazione() {
        assert_eq!(
            parole("Demolizione di MURATURA in c.a., sp. 30 cm – m²"),
            [
                "demolizione",
                "di",
                "muratura",
                "in",
                "c",
                "a",
                "sp",
                "30",
                "cm",
                "m2"
            ]
        );
        assert_eq!(parole("Profondità  più"), ["profondita", "piu"]);
    }

    #[test]
    fn termini_con_radice() {
        assert_eq!(
            termini("Scavi di sbancamento per le fondazioni"),
            ["scav", "sbancament", "fondazion"]
        );
        assert_eq!(termini("scavo"), termini("SCAVI"));
        assert_eq!(termini("Calcestruzzo C25/30"), ["calcestruzz", "c25", "30"]);
    }

    #[test]
    fn trigrammi_con_bordo() {
        assert_eq!(trigrammi("cls"), ["_cl", "cls", "ls_"]);
        assert_eq!(trigrammi("aa"), ["_aa", "aa_"]);
    }
}
