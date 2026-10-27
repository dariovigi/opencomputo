//! Lettura in streaming degli elementi XML dei flussi DCF.
//!
//! I flussi possono superare i 50 MB: invece di costruire un albero, il testo è
//! percorso una volta e per ogni elemento viene chiamata una funzione con il
//! percorso dei genitori e gli attributi. I dati dei DCF sono negli attributi.

use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

/// Profondità massima accettata.
pub const PROFONDITA_MASSIMA: usize = 64;

/// Elemento letto.
#[derive(Debug)]
pub struct Elemento<'a> {
    /// Nome.
    pub nome: &'a str,
    /// Nomi dei genitori, dalla radice.
    pub genitori: &'a [String],
    /// Attributi nell'ordine del documento.
    pub attributi: &'a [(String, String)],
    /// Riga del documento (a base 1).
    pub riga: usize,
}

impl Elemento<'_> {
    /// Valore di un attributo.
    #[must_use]
    pub fn attributo(&self, nome: &str) -> Option<&str> {
        self.attributi
            .iter()
            .find(|(k, _)| k == nome)
            .map(|(_, v)| v.as_str())
    }

    /// Nome del genitore diretto.
    #[must_use]
    pub fn genitore(&self) -> Option<&str> {
        self.genitori.last().map(String::as_str)
    }
}

fn attributi(e: &BytesStart<'_>) -> Result<Vec<(String, String)>, String> {
    let mut v = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|x| x.to_string())?;
        let k = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let val = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|x| x.to_string())?
            .into_owned();
        v.push((k, val));
    }
    Ok(v)
}

/// Percorre tutti gli elementi del testo.
///
/// # Errors
/// Messaggio con la riga se l'XML è malformato o troppo annidato.
pub fn per_elementi(testo: &str, mut f: impl FnMut(&Elemento<'_>)) -> Result<(), String> {
    let mut lettore = Reader::from_str(testo);
    let mut pila: Vec<String> = Vec::new();
    let mut righe_viste = 0usize;
    let mut ultima_pos = 0usize;
    loop {
        let pos = usize::try_from(lettore.buffer_position()).unwrap_or(usize::MAX);
        let evento = lettore.read_event().map_err(|e| {
            let p = usize::try_from(lettore.error_position()).unwrap_or(pos);
            let riga = 1 + testo
                .get(..p.min(testo.len()))
                .map_or(0, |s| s.matches('\n').count());
            format!("XML non valido alla riga {riga}: {e}")
        })?;
        let mut riga = || {
            if let Some(s) = testo.get(ultima_pos..pos.min(testo.len())) {
                righe_viste += s.matches('\n').count();
                ultima_pos = pos.min(testo.len());
            }
            righe_viste + 1
        };
        match evento {
            Event::Start(e) => {
                if pila.len() >= PROFONDITA_MASSIMA {
                    return Err(format!("annidamento oltre {PROFONDITA_MASSIMA} livelli"));
                }
                let nome = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attr = attributi(&e)?;
                let r = riga();
                f(&Elemento {
                    nome: &nome,
                    genitori: &pila,
                    attributi: &attr,
                    riga: r,
                });
                pila.push(nome);
            }
            Event::Empty(e) => {
                let nome = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let attr = attributi(&e)?;
                let r = riga();
                f(&Elemento {
                    nome: &nome,
                    genitori: &pila,
                    attributi: &attr,
                    riga: r,
                });
            }
            Event::End(_) => {
                pila.pop();
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elementi_genitori_attributi_e_righe() {
        let mut visti = Vec::new();
        per_elementi(
            "<A x=\"1 &amp; 2\">\n  <B/>\n  <C><D y=\"&lt;n&gt;\"/></C>\n</A>\n<E/>",
            |e| {
                visti.push((
                    e.nome.to_owned(),
                    e.genitori.join("/"),
                    e.attributi.to_vec(),
                    e.riga,
                ));
            },
        )
        .unwrap();
        assert_eq!(visti.len(), 5);
        assert_eq!(visti[0].2, vec![("x".to_owned(), "1 & 2".to_owned())]);
        assert_eq!(
            (visti[3].0.as_str(), visti[3].1.as_str(), visti[3].3),
            ("D", "A/C", 3)
        );
        assert_eq!(visti[3].2[0].1, "<n>");
        assert_eq!(visti[4].1, "");
        assert!(
            per_elementi("<A><B></A>", |_| {})
                .unwrap_err()
                .contains("riga 1")
        );
    }
}
