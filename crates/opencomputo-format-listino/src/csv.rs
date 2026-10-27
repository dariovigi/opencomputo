//! Lettore CSV minimo (RFC 4180): campi tra virgolette con `""` per le virgolette
//! interne, a capo dentro i campi tra virgolette, fine riga `\n` o `\r\n`.
//!
//! Il separatore di campo si riconosce dall'intestazione: `;` (il più comune nei fogli
//! di calcolo in italiano), `,` o tabulazione.

use crate::errore::ErroreListino;

/// Lunghezza massima di un campo, in caratteri.
pub const CAMPO_MASSIMO: usize = 65_536;

/// Riga letta: numero della riga fisica in cui inizia (da 1) e campi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RigaCsv {
    /// Riga fisica di inizio, da 1.
    pub numero: usize,
    /// Campi, senza virgolette di delimitazione.
    pub campi: Vec<String>,
}

impl RigaCsv {
    /// `true` se tutti i campi sono vuoti o fatti di soli spazi.
    #[must_use]
    pub fn vuota(&self) -> bool {
        self.campi.iter().all(|c| c.trim().is_empty())
    }
}

/// Separatore di campo riconosciuto dalla prima riga non vuota: il carattere più
/// frequente tra `;`, `,` e tabulazione, fuori dalle virgolette. A parità vince `;`.
#[must_use]
pub fn riconosci_separatore(testo: &str) -> char {
    let prima = testo.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut conteggi = [(';', 0usize), (',', 0), ('\t', 0)];
    let mut tra_virgolette = false;
    for c in prima.chars() {
        if c == '"' {
            tra_virgolette = !tra_virgolette;
        } else if !tra_virgolette {
            for (s, n) in &mut conteggi {
                if c == *s {
                    *n += 1;
                }
            }
        }
    }
    let mut migliore = (';', 0usize);
    for (s, n) in conteggi {
        if n > migliore.1 {
            migliore = (s, n);
        }
    }
    migliore.0
}

/// Legge tutte le righe del testo.
///
/// # Errors
/// Virgolette non chiuse a fine file, campo oltre [`CAMPO_MASSIMO`], troppe righe.
pub fn leggi(
    testo: &str,
    separatore: char,
    righe_massime: usize,
) -> Result<Vec<RigaCsv>, ErroreListino> {
    let mut righe = Vec::new();
    let mut campi: Vec<String> = Vec::new();
    let mut campo = String::new();
    let mut lunghezza = 0usize;
    let mut tra_virgolette = false;
    let mut riga_fisica = 1usize;
    let mut inizio_riga = 1usize;
    let mut caratteri = testo.chars().peekable();
    let mut in_corso = false;

    while let Some(c) = caratteri.next() {
        in_corso = true;
        if tra_virgolette {
            if c == '"' {
                if caratteri.peek() == Some(&'"') {
                    caratteri.next();
                    campo.push('"');
                    lunghezza += 1;
                } else {
                    tra_virgolette = false;
                }
            } else if c == '\r' && caratteri.peek() == Some(&'\n') {
                // `\r\n` dentro un campo diventa `\n`.
            } else {
                if c == '\n' {
                    riga_fisica += 1;
                }
                campo.push(c);
                lunghezza += 1;
            }
        } else if c == '"' && campo.trim().is_empty() {
            campo.clear();
            tra_virgolette = true;
        } else if c == separatore {
            campi.push(std::mem::take(&mut campo));
            lunghezza = 0;
        } else if c == '\r' && caratteri.peek() == Some(&'\n') {
            // Il fine riga è gestito sul '\n'.
        } else if c == '\n' {
            campi.push(std::mem::take(&mut campo));
            righe.push(RigaCsv {
                numero: inizio_riga,
                campi: std::mem::take(&mut campi),
            });
            if righe.len() > righe_massime {
                return Err(ErroreListino::TroppeRighe {
                    massimo: righe_massime,
                });
            }
            lunghezza = 0;
            riga_fisica += 1;
            inizio_riga = riga_fisica;
            in_corso = false;
        } else {
            campo.push(c);
            lunghezza += 1;
        }
        if lunghezza > CAMPO_MASSIMO {
            return Err(ErroreListino::Csv {
                riga: inizio_riga,
                motivo: format!("campo oltre {CAMPO_MASSIMO} caratteri"),
            });
        }
    }
    if tra_virgolette {
        return Err(ErroreListino::Csv {
            riga: inizio_riga,
            motivo: "virgolette aperte e non chiuse".into(),
        });
    }
    if in_corso {
        campi.push(campo);
        righe.push(RigaCsv {
            numero: inizio_riga,
            campi,
        });
    }
    Ok(righe)
}

#[cfg(test)]
mod test {
    use super::*;

    fn campi(t: &str) -> Vec<Vec<String>> {
        leggi(t, ';', 100)
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.campi)
            .collect()
    }

    #[test]
    fn virgolette_separatori_e_a_capo() {
        let r = leggi("a;\"b;c\";\"d\"\"e\"\r\n\"riga\ndue\";x\n", ';', 100).unwrap_or_default();
        assert_eq!(r.len(), 2);
        assert_eq!(
            r.first().map(|x| x.campi.clone()),
            Some(vec!["a".into(), "b;c".into(), "d\"e".into()])
        );
        assert_eq!(r.get(1).map(|x| x.numero), Some(2));
        assert_eq!(
            r.get(1).map(|x| x.campi.clone()),
            Some(vec!["riga\ndue".into(), "x".into()])
        );
    }

    #[test]
    fn numero_di_riga_dopo_campo_su_piu_righe() {
        let r = leggi("\"a\nb\nc\";1\nx;2\n", ';', 100).unwrap_or_default();
        assert_eq!(r.get(1).map(|x| x.numero), Some(4));
    }

    #[test]
    fn ultima_riga_senza_a_capo_e_campi_vuoti() {
        assert_eq!(
            campi("a;;b"),
            vec![vec!["a".to_owned(), String::new(), "b".into()]]
        );
        assert_eq!(campi(""), Vec::<Vec<String>>::new());
    }

    #[test]
    fn virgolette_non_chiuse() {
        assert!(matches!(
            leggi("a;\"b\n", ';', 100),
            Err(ErroreListino::Csv { riga: 1, .. })
        ));
    }

    #[test]
    fn limite_di_righe() {
        assert!(matches!(
            leggi("a\nb\nc\n", ';', 2),
            Err(ErroreListino::TroppeRighe { massimo: 2 })
        ));
    }

    #[test]
    fn riconoscimento_del_separatore() {
        assert_eq!(riconosci_separatore("a;b;c\n1,5;2;3"), ';');
        assert_eq!(riconosci_separatore("a,b,c\n"), ',');
        assert_eq!(riconosci_separatore("a\tb\tc\n"), '\t');
        assert_eq!(riconosci_separatore("\"a,b\";c\n"), ';');
        assert_eq!(riconosci_separatore("solo"), ';');
    }
}
