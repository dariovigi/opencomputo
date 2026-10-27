//! Scrittura CSV delle tabelle dei report.
//!
//! Impostazioni predefinite pensate per Excel e LibreOffice in italiano: separatore
//! `;`, virgola decimale, nessun separatore delle migliaia, UTF-8 con BOM, righe CRLF.

use crate::documento::{Documento, Tabella, Valore};
use crate::formato;

/// Opzioni di scrittura.
#[derive(Debug, Clone)]
pub struct OpzioniCsv {
    /// Separatore dei campi.
    pub separatore: char,
    /// Separatore decimale dei numeri.
    pub decimale: char,
    /// Scrive il BOM UTF-8 iniziale (necessario a Excel per riconoscere l'UTF-8).
    pub bom: bool,
    /// Aggiunge la prima colonna «Tipo» con il ruolo della riga (voce, rigo, totale…).
    pub colonna_tipo: bool,
}

impl Default for OpzioniCsv {
    fn default() -> Self {
        Self {
            separatore: ';',
            decimale: ',',
            bom: true,
            colonna_tipo: true,
        }
    }
}

fn campo(testo: &str, separatore: char, out: &mut String) {
    let da_quotare = testo.contains(separatore)
        || testo.contains('"')
        || testo.contains('\n')
        || testo.contains('\r')
        || testo.starts_with(' ')
        || testo.ends_with(' ');
    if da_quotare {
        out.push('"');
        out.push_str(&testo.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(testo);
    }
}

/// Testo che un foglio di calcolo eseguirebbe come formula (inizia con `=`, `+`, `@`,
/// `-` non numerico, tabulazione o ritorno a capo): preceduto da un apostrofo, perché
/// resti testo («CSV injection»).
fn neutralizza(testo: &str) -> String {
    let numero = |t: &str| {
        t.len() > 1
            && t[1..]
                .chars()
                .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
    };
    match testo.chars().next() {
        Some('=' | '+' | '@' | '\t' | '\r') => format!("'{testo}"),
        Some('-') if !numero(testo) => format!("'{testo}"),
        _ => testo.to_owned(),
    }
}

/// Una tabella in CSV.
#[must_use]
pub fn tabella(t: &Tabella, o: &OpzioniCsv) -> String {
    let mut out = String::new();
    if o.bom {
        out.push('\u{FEFF}');
    }
    let riga = |celle: Vec<String>, out: &mut String| {
        for (i, c) in celle.iter().enumerate() {
            if i > 0 {
                out.push(o.separatore);
            }
            campo(c, o.separatore, out);
        }
        out.push_str("\r\n");
    };
    let mut intestazione: Vec<String> = Vec::new();
    if o.colonna_tipo {
        intestazione.push("Tipo".into());
    }
    intestazione.extend(t.colonne.iter().map(|c| c.titolo.clone()));
    riga(intestazione, &mut out);
    for r in &t.righe {
        let mut celle: Vec<String> = Vec::new();
        if o.colonna_tipo {
            celle.push(r.stile.codice().into());
        }
        celle.extend(r.celle.iter().map(|v| match v {
            Valore::Vuoto => String::new(),
            Valore::Testo(s) => neutralizza(s),
            Valore::Numero { valore, decimali } => {
                formato::numero_semplice(*valore, *decimali, o.decimale)
            }
        }));
        riga(celle, &mut out);
    }
    out
}

/// Un CSV per ciascuna sezione del documento: (titolo della sezione, contenuto).
#[must_use]
pub fn documento(d: &Documento, o: &OpzioniCsv) -> Vec<(String, String)> {
    d.sezioni
        .iter()
        .map(|s| (s.titolo.clone(), tabella(&s.tabella, o)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documento::{Colonna, StileRiga};
    use rust_decimal_macros::dec;

    #[test]
    fn testi_che_sembrano_formule_restano_testo() {
        assert_eq!(neutralizza("=1+1"), "'=1+1");
        assert_eq!(neutralizza("@SUM(A1)"), "'@SUM(A1)");
        assert_eq!(neutralizza("+39 0432"), "'+39 0432");
        assert_eq!(neutralizza("-2+3"), "'-2+3");
        assert_eq!(neutralizza("-12,50"), "-12,50");
        assert_eq!(neutralizza("Scavo"), "Scavo");
        assert_eq!(neutralizza(""), "");
        let mut t = Tabella::new(vec![Colonna::testo("A", 5), Colonna::numero("B", 5)]);
        t.riga(
            StileRiga::Rigo,
            vec![
                Valore::testo("=HYPERLINK(\"x\")"),
                Valore::Numero {
                    valore: rust_decimal::Decimal::new(-125, 1),
                    decimali: 2,
                },
            ],
        );
        let csv = tabella(&t, &OpzioniCsv::default());
        assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\";-12,50"), "{csv}");
    }

    #[test]
    fn quotatura_numeri_e_bom() {
        let mut t = Tabella::new(vec![
            Colonna::testo("Descrizione", 1),
            Colonna::numero("Importo €", 1),
        ]);
        t.riga(
            StileRiga::Voce,
            vec![
                Valore::testo("Scavo; a \"sezione\"\nobbligata"),
                Valore::numero(dec!(1234.5), 2),
            ],
        );
        t.riga(
            StileRiga::Totale,
            vec![Valore::testo(" totale"), Valore::numero(dec!(-3), 2)],
        );
        let s = tabella(&t, &OpzioniCsv::default());
        assert_eq!(
            s,
            "\u{FEFF}Tipo;Descrizione;Importo €\r\nvoce;\"Scavo; a \"\"sezione\"\"\nobbligata\";1234,50\r\ntotale;\" totale\";-3,00\r\n"
        );
        let s = tabella(
            &t,
            &OpzioniCsv {
                separatore: ',',
                decimale: '.',
                bom: false,
                colonna_tipo: false,
            },
        );
        assert!(s.starts_with("Descrizione,Importo €\r\n"));
        assert!(s.contains(",1234.50\r\n"));
    }
}
