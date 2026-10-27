//! Lettura e scrittura XML di basso livello.
//!
//! Il documento viene caricato in un albero minimo ([`Nodo`]) che conserva nome,
//! attributi, testo, figli e riga di origine di ogni elemento. Commenti,
//! istruzioni di elaborazione e DTD sono ignorati. I limiti su profondità e
//! numero di elementi proteggono da file malformati o ostili.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use quick_xml::XmlVersion;
use std::borrow::Cow;

use quick_xml::escape::{escape as escape_xml, resolve_predefined_entity};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use crate::errore::ErroreXpwe;

/// Profondità massima di annidamento accettata.
pub const PROFONDITA_MASSIMA: usize = 64;
/// Numero massimo di elementi accettato (circa 50 MB di XPWE tipico).
pub const ELEMENTI_MASSIMI: usize = 5_000_000;

/// Elemento XML.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Nodo {
    /// Nome dell'elemento (con eventuale prefisso).
    pub nome: String,
    /// Attributi nell'ordine del documento.
    pub attributi: Vec<(String, String)>,
    /// Testo contenuto (concatenazione dei nodi di testo diretti).
    pub testo: String,
    /// Elementi figli.
    pub figli: Vec<Nodo>,
    /// Riga di inizio nel documento (a base 1).
    pub riga: usize,
}

impl Nodo {
    /// Primo figlio con il nome indicato.
    #[must_use]
    pub fn figlio(&self, nome: &str) -> Option<&Nodo> {
        self.figli.iter().find(|f| f.nome == nome)
    }

    /// Figli con il nome indicato.
    pub fn figli_con_nome<'a>(&'a self, nome: &'a str) -> impl Iterator<Item = &'a Nodo> + 'a {
        self.figli.iter().filter(move |f| f.nome == nome)
    }

    /// Valore di un attributo.
    #[must_use]
    pub fn attributo(&self, nome: &str) -> Option<&str> {
        self.attributi
            .iter()
            .find(|(k, _)| k == nome)
            .map(|(_, v)| v.as_str())
    }

    /// Testo del primo figlio con il nome indicato.
    #[must_use]
    pub fn testo_di(&self, nome: &str) -> Option<&str> {
        self.figlio(nome).map(|f| f.testo.as_str())
    }

    /// `true` se l'elemento non ha figli (è un campo).
    #[must_use]
    pub fn is_foglia(&self) -> bool {
        self.figli.is_empty()
    }

    /// Serializza l'elemento in forma compatta (senza rientri).
    #[must_use]
    pub fn in_xml(&self) -> String {
        let mut s = String::new();
        scrivi_compatto(self, &mut s);
        s
    }
}

fn scrivi_compatto(n: &Nodo, s: &mut String) {
    s.push('<');
    s.push_str(&n.nome);
    for (k, v) in &n.attributi {
        s.push(' ');
        s.push_str(k);
        s.push_str("=\"");
        s.push_str(&escape(v.as_str()));
        s.push('"');
    }
    if n.figli.is_empty() && n.testo.is_empty() {
        s.push_str("/>");
        return;
    }
    s.push('>');
    s.push_str(&escape(n.testo.as_str()));
    for f in &n.figli {
        scrivi_compatto(f, s);
    }
    s.push_str("</");
    s.push_str(&n.nome);
    s.push('>');
}

/// Documento caricato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Documento {
    /// Elemento radice.
    pub radice: Nodo,
    /// Codifica usata per la decodifica.
    pub codifica: &'static str,
    /// `true` se alcuni byte non erano validi nella codifica e sono stati sostituiti.
    pub caratteri_sostituiti: bool,
    /// Codifica dichiarata ma non riconosciuta (sostituita da UTF-8 o Windows-1252).
    pub codifica_sconosciuta: Option<String>,
}

/// Codifica dichiarata nella dichiarazione XML (`encoding="…"`), se presente.
fn codifica_dichiarata(dati: &[u8]) -> Option<String> {
    let inizio = dati.get(..dati.len().min(256))?;
    let testo = String::from_utf8_lossy(inizio);
    let decl_fine = testo.find("?>")?;
    let decl = testo.get(..decl_fine)?;
    if !decl.trim_start().starts_with("<?xml") {
        return None;
    }
    let pos = decl.find("encoding")?;
    let resto = decl.get(pos + "encoding".len()..)?.trim_start();
    let resto = resto.strip_prefix('=')?.trim_start();
    let apice = resto.chars().next()?;
    if apice != '"' && apice != '\'' {
        return None;
    }
    let corpo = resto.get(1..)?;
    let fine = corpo.find(apice)?;
    Some(corpo.get(..fine)?.trim().to_owned())
}

/// Decodifica i byte in testo: BOM, poi dichiarazione XML, poi UTF-8 con
/// ripiego su Windows-1252 se l'UTF-8 non è valido e non è dichiarato.
fn decodifica(dati: &[u8]) -> (String, &'static str, bool, Option<String>) {
    if let Some((enc, lunghezza_bom)) = Encoding::for_bom(dati) {
        let corpo = dati.get(lunghezza_bom..).unwrap_or_default();
        let (testo, errori) = enc.decode_without_bom_handling(corpo);
        return (testo.into_owned(), enc.name(), errori, None);
    }
    let dichiarata = codifica_dichiarata(dati);
    let mut sconosciuta = None;
    let enc = match &dichiarata {
        Some(etichetta) => match Encoding::for_label(etichetta.as_bytes()) {
            Some(e) => Some(e),
            None => {
                sconosciuta = Some(etichetta.clone());
                None
            }
        },
        None => None,
    };
    if let Some(enc) = enc {
        let (testo, errori) = enc.decode_without_bom_handling(dati);
        return (testo.into_owned(), enc.name(), errori, sconosciuta);
    }
    let (testo, errori) = UTF_8.decode_without_bom_handling(dati);
    if !errori {
        return (testo.into_owned(), UTF_8.name(), false, sconosciuta);
    }
    // Non UTF-8 e codifica non dichiarata: i file prodotti su Windows sono
    // tipicamente in Windows-1252, che decodifica qualsiasi sequenza di byte.
    let (testo, errori) = WINDOWS_1252.decode_without_bom_handling(dati);
    (testo.into_owned(), WINDOWS_1252.name(), errori, sconosciuta)
}

/// Inizio di ogni riga (offset in byte) per convertire posizioni in righe/colonne.
struct Righe {
    inizi: Vec<usize>,
}

impl Righe {
    fn new(testo: &str) -> Self {
        let mut inizi = vec![0];
        inizi.extend(testo.match_indices('\n').map(|(i, _)| i + 1));
        Self { inizi }
    }

    fn riga(&self, offset: usize) -> usize {
        match self.inizi.binary_search(&offset) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }

    fn colonna(&self, testo: &str, offset: usize) -> usize {
        let riga = self.riga(offset);
        let inizio = self.inizi.get(riga.saturating_sub(1)).copied().unwrap_or(0);
        testo
            .get(inizio..offset.min(testo.len()))
            .map_or(1, |s| s.chars().count() + 1)
    }
}

fn errore_xml(testo: &str, righe: &Righe, offset: usize, messaggio: String) -> ErroreXpwe {
    ErroreXpwe::XmlNonValido {
        riga: righe.riga(offset),
        colonna: righe.colonna(testo, offset),
        messaggio,
    }
}

fn nuovo_nodo(
    e: &BytesStart<'_>,
    riga: usize,
    testo: &str,
    righe: &Righe,
    offset: usize,
) -> Result<Nodo, ErroreXpwe> {
    let nome = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut attributi = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|err| errore_xml(testo, righe, offset, err.to_string()))?;
        let chiave = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let valore = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|err| errore_xml(testo, righe, offset, err.to_string()))?
            .into_owned();
        attributi.push((chiave, valore));
    }
    Ok(Nodo {
        nome,
        attributi,
        testo: String::new(),
        figli: Vec::new(),
        riga,
    })
}

/// Carica un documento XML dai byte del file.
///
/// # Errors
/// [`ErroreXpwe::XmlNonValido`] con riga e colonna se l'XML è malformato;
/// [`ErroreXpwe::TroppoAnnidato`] / [`ErroreXpwe::TroppiElementi`] oltre i limiti.
pub fn analizza(dati: &[u8]) -> Result<Documento, ErroreXpwe> {
    let (testo, codifica, caratteri_sostituiti, codifica_sconosciuta) = decodifica(dati);
    let righe = Righe::new(&testo);
    let mut lettore = Reader::from_str(&testo);
    let mut pila: Vec<Nodo> = Vec::new();
    let mut radice: Option<Nodo> = None;
    let mut elementi = 0usize;

    loop {
        let offset = usize::try_from(lettore.buffer_position()).unwrap_or(usize::MAX);
        let evento = lettore.read_event().map_err(|err| {
            let pos = usize::try_from(lettore.error_position()).unwrap_or(offset);
            errore_xml(&testo, &righe, pos, err.to_string())
        })?;
        match evento {
            Event::Start(e) | Event::Empty(e) if radice.is_some() => {
                let _ = e;
                return Err(errore_xml(
                    &testo,
                    &righe,
                    offset,
                    "contenuto dopo la chiusura dell'elemento radice".into(),
                ));
            }
            Event::Start(e) => {
                elementi += 1;
                if elementi > ELEMENTI_MASSIMI {
                    return Err(ErroreXpwe::TroppiElementi(ELEMENTI_MASSIMI));
                }
                if pila.len() >= PROFONDITA_MASSIMA {
                    return Err(ErroreXpwe::TroppoAnnidato(PROFONDITA_MASSIMA));
                }
                let riga = righe.riga(offset);
                pila.push(nuovo_nodo(&e, riga, &testo, &righe, offset)?);
            }
            Event::Empty(e) => {
                elementi += 1;
                if elementi > ELEMENTI_MASSIMI {
                    return Err(ErroreXpwe::TroppiElementi(ELEMENTI_MASSIMI));
                }
                let riga = righe.riga(offset);
                let nodo = nuovo_nodo(&e, riga, &testo, &righe, offset)?;
                match pila.last_mut() {
                    Some(padre) => padre.figli.push(nodo),
                    None => radice = Some(nodo),
                }
            }
            Event::End(_) => {
                let Some(mut nodo) = pila.pop() else {
                    return Err(errore_xml(
                        &testo,
                        &righe,
                        offset,
                        "chiusura senza apertura".into(),
                    ));
                };
                if !nodo.figli.is_empty() && nodo.testo.trim().is_empty() {
                    nodo.testo.clear();
                }
                match pila.last_mut() {
                    Some(padre) => padre.figli.push(nodo),
                    None => radice = Some(nodo),
                }
            }
            Event::Text(t) => {
                let contenuto = t
                    .xml10_content()
                    .map_err(|err| errore_xml(&testo, &righe, offset, err.to_string()))?;
                match pila.last_mut() {
                    Some(n) => n.testo.push_str(&contenuto),
                    None if contenuto.trim().is_empty() => {}
                    None => {
                        return Err(errore_xml(
                            &testo,
                            &righe,
                            offset,
                            "testo fuori dall'elemento radice".into(),
                        ));
                    }
                }
            }
            Event::CData(c) => {
                let contenuto = c
                    .decode()
                    .map_err(|err| errore_xml(&testo, &righe, offset, err.to_string()))?;
                if let Some(n) = pila.last_mut() {
                    n.testo.push_str(&contenuto);
                }
            }
            Event::GeneralRef(r) => {
                let carattere = r
                    .resolve_char_ref()
                    .map_err(|err| errore_xml(&testo, &righe, offset, err.to_string()))?;
                let nome = r
                    .decode()
                    .map_err(|err| errore_xml(&testo, &righe, offset, err.to_string()))?;
                let sostituto: String = match carattere {
                    Some(c) => c.to_string(),
                    None => match resolve_predefined_entity(&nome) {
                        Some(s) => s.to_owned(),
                        None => {
                            return Err(errore_xml(
                                &testo,
                                &righe,
                                offset,
                                format!("entità sconosciuta «&{nome};»"),
                            ));
                        }
                    },
                };
                if let Some(n) = pila.last_mut() {
                    n.testo.push_str(&sostituto);
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }

    if !pila.is_empty() {
        return Err(errore_xml(
            &testo,
            &righe,
            testo.len(),
            "documento troncato: elementi non chiusi".into(),
        ));
    }
    let radice = radice.ok_or_else(|| ErroreXpwe::NonXpwe("nessun elemento radice".into()))?;
    Ok(Documento {
        radice,
        codifica,
        caratteri_sostituiti,
        codifica_sconosciuta,
    })
}

/// Fine riga CRLF anche nei testi (il parser li normalizza a LF in lettura).
fn crlf(testo: &str) -> String {
    testo.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Scrittore XML con rientro di due spazi e fine riga CRLF, come negli export di
/// Testo per XML 1.0: senza i caratteri che XML non ammette (controlli diversi da
/// tabulazione e a capo, U+FFFE, U+FFFF), che renderebbero il file illeggibile ai
/// programmi che lo verificano, poi con le entità.
fn escape(testo: &str) -> Cow<'_, str> {
    let ammesso = |c: char| {
        matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')
    };
    if testo.chars().all(ammesso) {
        escape_xml(testo)
    } else {
        let pulito: String = testo.chars().filter(|c| ammesso(*c)).collect();
        Cow::Owned(escape_xml(&pulito).into_owned())
    }
}

/// PriMus-DCF osservati.
#[derive(Debug, Default)]
pub struct Scrittore {
    testo: String,
    livello: usize,
}

impl Scrittore {
    /// Nuovo documento con dichiarazione XML in UTF-8.
    #[must_use]
    pub fn new() -> Self {
        Self {
            testo: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n"),
            livello: 0,
        }
    }

    fn rientro(&mut self) {
        for _ in 0..self.livello {
            self.testo.push_str("  ");
        }
    }

    fn apertura(&mut self, nome: &str, attributi: &[(&str, &str)]) {
        self.testo.push('<');
        self.testo.push_str(nome);
        for (k, v) in attributi {
            self.testo.push(' ');
            self.testo.push_str(k);
            self.testo.push_str("=\"");
            self.testo.push_str(&escape(v));
            self.testo.push('"');
        }
    }

    /// Apre un elemento contenitore.
    pub fn apri(&mut self, nome: &str, attributi: &[(&str, &str)]) {
        self.rientro();
        self.apertura(nome, attributi);
        self.testo.push_str(">\r\n");
        self.livello += 1;
    }

    /// Chiude un elemento contenitore.
    pub fn chiudi(&mut self, nome: &str) {
        self.livello = self.livello.saturating_sub(1);
        self.rientro();
        self.testo.push_str("</");
        self.testo.push_str(nome);
        self.testo.push_str(">\r\n");
    }

    /// Scrive un campo con testo (vuoto → elemento vuoto).
    pub fn campo(&mut self, nome: &str, valore: &str) {
        self.rientro();
        self.apertura(nome, &[]);
        if valore.is_empty() {
            self.testo.push_str("/>\r\n");
        } else {
            self.testo.push('>');
            self.testo.push_str(&crlf(&escape(valore)));
            self.testo.push_str("</");
            self.testo.push_str(nome);
            self.testo.push_str(">\r\n");
        }
    }

    /// Inserisce un frammento XML già serializzato (dati originali conservati).
    pub fn grezzo(&mut self, xml: &str) {
        self.rientro();
        self.testo.push_str(&crlf(xml));
        self.testo.push_str("\r\n");
    }

    /// Testo prodotto.
    #[must_use]
    pub fn fine(self) -> String {
        self.testo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn albero_testo_attributi_ed_entita() {
        let d = analizza(
            "<?xml version=\"1.0\"?>\n<A x=\"1 &amp; 2\">\n  <B>uno &lt;due&gt; &#233;</B>\n  <C/>\n</A>"
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(d.codifica, "UTF-8");
        assert_eq!(d.radice.nome, "A");
        assert_eq!(d.radice.attributo("x"), Some("1 & 2"));
        assert_eq!(d.radice.testo_di("B"), Some("uno <due> é"));
        assert_eq!(d.radice.figlio("C").map(|c| c.riga), Some(4));
        assert_eq!(d.radice.testo, "");
    }

    #[test]
    fn codifica_dichiarata_windows_1252() {
        let mut dati = b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><A>".to_vec();
        dati.push(0xE0); // «à» in Windows-1252
        dati.extend_from_slice(b"</A>");
        let d = analizza(&dati).unwrap();
        assert_eq!(d.codifica, "windows-1252");
        assert_eq!(d.radice.testo, "à");
        assert!(!d.caratteri_sostituiti);
    }

    #[test]
    fn utf8_non_valido_senza_dichiarazione_ripiega_su_windows_1252() {
        let d = analizza(&[b'<', b'A', b'>', 0xE8, b'<', b'/', b'A', b'>']).unwrap();
        assert_eq!(d.codifica, "windows-1252");
        assert_eq!(d.radice.testo, "è");
    }

    #[test]
    fn errori_con_posizione() {
        let e = analizza(b"<A>\n  <B>\n</A>").unwrap_err();
        assert!(
            matches!(e, ErroreXpwe::XmlNonValido { riga: 3, .. }),
            "{e:?}"
        );
        assert!(matches!(
            analizza(b"<A><B>"),
            Err(ErroreXpwe::XmlNonValido { .. })
        ));
        assert!(matches!(analizza(b""), Err(ErroreXpwe::NonXpwe(_))));
        assert!(matches!(
            analizza(b"<A/><B/>"),
            Err(ErroreXpwe::XmlNonValido { .. })
        ));
        assert!(matches!(
            analizza(b"<A>&ignota;</A>"),
            Err(ErroreXpwe::XmlNonValido { .. })
        ));
    }

    #[test]
    fn caratteri_non_ammessi_da_xml_tolti_in_scrittura() {
        let mut s = Scrittore::new();
        s.campo("D", "a\u{1}b\u{8}c\td & <e>\u{FFFF}");
        let x = s.fine();
        assert!(x.contains("<D>abc\td &amp; &lt;e&gt;</D>"), "{x}");
        assert!(analizza(x.as_bytes()).is_ok());
    }

    #[test]
    fn limite_di_profondita() {
        let mut s = String::new();
        for _ in 0..=PROFONDITA_MASSIMA {
            s.push_str("<a>");
        }
        assert_eq!(
            analizza(s.as_bytes()),
            Err(ErroreXpwe::TroppoAnnidato(PROFONDITA_MASSIMA))
        );
    }

    #[test]
    fn serializzazione_compatta_reversibile() {
        let d = analizza(b"<A k=\"&quot;v&quot;\"><B>x &amp; y</B><C/></A>").unwrap();
        let xml = d.radice.in_xml();
        let di_nuovo = analizza(xml.as_bytes()).unwrap();
        assert_eq!(di_nuovo.radice.in_xml(), xml);
        assert_eq!(di_nuovo.radice.testo_di("B"), Some("x & y"));
    }
}
