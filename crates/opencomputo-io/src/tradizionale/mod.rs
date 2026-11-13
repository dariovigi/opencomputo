//! Stampa del computo e dell'elenco prezzi nel **modello tradizionale** (ADR 0015).
//!
//! È l'impaginazione dei computi in uso negli studi tecnici italiani: un'alternativa
//! al report ordinario ([`crate::report`]), solo in PDF. Comprende:
//! * frontespizio con ente, titolo, oggetto, committente, data e tecnico;
//! * tabella su tutta la pagina con riquadro verde e colonne fisse;
//! * righe «R I P O R T O» e «A R I P O R T A R E» con la quantità della voce in corso e
//!   l'importo progressivo;
//! * per ogni voce numero d'ordine, tariffa, descrizione giustificata, righi di misura
//!   con le deduzioni in rosso, «SOMMANO»;
//! * nell'elenco prezzi, il prezzo in lettere.
//!
//! I valori vengono dal core (ADR 0007): quantità, progressivi e somme dei righi da
//! [`opencomputo_core::misura::dettaglio_quantita`], importi e totali da
//! `Progetto::calcola_computo`. L'unica somma fatta qui è il progressivo degli importi
//! per i riporti di pagina: è una somma esatta, senza arrotondamenti, e alla fine deve
//! coincidere con il totale del core, altrimenti la stampa non viene prodotta.
//!
//! Font standard PDF: Times per la tabella, Helvetica per il frontespizio. Le
//! coordinate sono intere, in centesimi di punto, misurate dall'alto della pagina.

mod computo;
mod elenco;
mod generico;

use std::fmt::Write as _;

use chrono::NaiveDate;
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;

use crate::formato;
use crate::metriche;
use crate::pdf::{self, RisultatoPdf};

mod riepilogo;

pub use crate::report::Incidenze;
pub use computo::computo;
pub use elenco::elenco_prezzi;
pub use generico::documento;

/// Opzioni della stampa tradizionale.
#[derive(Debug, Clone)]
pub struct OpzioniTradizionale {
    /// Data di stampa, usata in calce se il progetto non ha una data.
    pub data_stampa: Option<NaiveDate>,
    /// Righe del riquadro in alto del frontespizio (es. «Comune di …», «Provincia di …»).
    pub ente: Vec<String>,
    /// Righe dello studio tecnico: testata di ogni pagina e riquadro in basso del
    /// frontespizio.
    pub studio: Vec<String>,
    /// Stampa il frontespizio (pagina 1).
    pub frontespizio: bool,
    /// Computo metrico senza prezzi né importi.
    pub senza_prezzi: bool,
    /// Descrizioni lunghe ridotte all'inizio e alla fine, come nelle stampe dei computi
    /// (la descrizione completa resta nell'elenco prezzi).
    pub descrizioni_ridotte: bool,
    /// Comprime i contenuti delle pagine.
    pub comprimi: bool,
    /// Incidenze di manodopera e sicurezza sotto ogni voce e riepilogo finale con i
    /// loro subtotali (solo computo estimativo, ADR 0021).
    pub incidenze: Incidenze,
    /// Chi firma (frontespizio e fine del documento); assente: il progettista come
    /// «Il Tecnico».
    pub firma: Option<Firma>,
}

/// Firma del documento: ruolo (es. «Il Direttore dei Lavori») e nome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Firma {
    /// Ruolo, con l'iniziale maiuscola.
    pub ruolo: String,
    /// Nome (assente: solo il ruolo).
    pub nome: Option<String>,
}

impl Default for OpzioniTradizionale {
    fn default() -> Self {
        Self {
            data_stampa: None,
            ente: Vec::new(),
            studio: Vec::new(),
            frontespizio: true,
            senza_prezzi: false,
            descrizioni_ridotte: true,
            comprimi: true,
            incidenze: Incidenze::default(),
            firma: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Misure (centesimi di punto, dall'alto).

const LARGHEZZA: i64 = 59_532;
const ALTEZZA: i64 = 84_192;
/// Corpo del testo della tabella.
const CORPO: i64 = 794;
/// Interlinea della tabella: 3,4 mm, in decimillesimi di punto.
const PASSO_10K: i64 = 96_378;
/// Margine del testo dai filetti delle colonne.
const RIENTRO: i64 = 142;

/// Filetti verticali del riquadro.
const X_SINISTRA: i64 = 1_757;
const X_DESTRA: i64 = 57_734;
/// Colonna del numero d'ordine e della tariffa.
const X_NUMERO: i64 = 6_405;
/// Larghezza massima della tariffa.
const LARGHEZZA_TARIFFA: i64 = 4_450;
/// Fine del riquadro e del corpo.
const Y_FONDO: i64 = 80_601;
/// Riga «COMMITTENTE» sotto il riquadro.
const Y_COMMITTENTE: i64 = 81_226;
/// Prima riga della testata.
const Y_TESTATA: i64 = 3_240;

const SOTTILE: i64 = 28;
const SPESSO: i64 = 113;

/// Font standard nell'ordine delle risorse `F1`…`F4`.
const FONT_PDF: [&str; 4] = ["Times-Roman", "Times-Bold", "Helvetica", "Helvetica-Bold"];

/// Carattere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carattere {
    Times,
    TimesGrassetto,
    Arial,
    ArialGrassetto,
}

impl Carattere {
    fn risorsa(self) -> &'static str {
        match self {
            Self::Times => "F1",
            Self::TimesGrassetto => "F2",
            Self::Arial => "F3",
            Self::ArialGrassetto => "F4",
        }
    }

    fn larghezze(self) -> &'static [u16; 256] {
        match self {
            Self::Times => &metriche::TIMES,
            Self::TimesGrassetto => &metriche::TIMES_BOLD,
            Self::Arial => &metriche::HELVETICA,
            Self::ArialGrassetto => &metriche::HELVETICA_BOLD,
        }
    }

    /// Famiglia e peso per l'anteprima: caratteri con le stesse metriche di quelli
    /// standard del PDF, tra quelli di solito installati.
    fn css(self) -> (&'static str, &'static str) {
        const TIMES: &str = "Times New Roman, Times, Liberation Serif, Nimbus Roman, serif";
        const ARIAL: &str = "Helvetica, Arial, Liberation Sans, Nimbus Sans, sans-serif";
        match self {
            Self::Times => (TIMES, "normal"),
            Self::TimesGrassetto => (TIMES, "bold"),
            Self::Arial => (ARIAL, "normal"),
            Self::ArialGrassetto => (ARIAL, "bold"),
        }
    }

    /// Discendente in millesimi di em (dalle metriche AFM).
    fn discesa(self) -> i64 {
        match self {
            Self::Times | Self::TimesGrassetto => 217,
            Self::Arial | Self::ArialGrassetto => 207,
        }
    }
}

/// Colore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Colore {
    Nero,
    Verde,
    Rosso,
    Blu,
    Bordeaux,
}

impl Colore {
    fn rgb(self) -> &'static str {
        match self {
            Self::Nero => "0 0 0",
            Self::Verde => "0 0.502 0",
            Self::Rosso => "1 0 0",
            Self::Blu => "0 0 1",
            Self::Bordeaux => "0.502 0 0",
        }
    }
}

fn larghezza(testo: &str, c: Carattere, corpo: i64) -> i64 {
    let mut ignorati = 0;
    let w = c.larghezze();
    let somma: i64 = pdf::codifica(testo, &mut ignorati)
        .iter()
        .map(|b| i64::from(w.get(usize::from(*b)).copied().unwrap_or(500)))
        .sum();
    somma * corpo / 1000
}

/// Righe di un testo entro la larghezza indicata, con l'indicazione dell'ultima riga di
/// ogni paragrafo (che non si giustifica).
fn righe_paragrafi(testo: &str, c: Carattere, corpo: i64, massimo: i64) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let paragrafi: Vec<&str> = testo.trim_end().split('\n').collect();
    for p in paragrafi {
        let righe = pdf::a_capo_misurato(p, &|t| larghezza(t, c, corpo), massimo);
        let n = righe.len();
        out.extend(righe.into_iter().enumerate().map(|(i, r)| (r, i + 1 == n)));
    }
    out
}

/// Righe di un testo, senza giustificazione.
fn righe(testo: &str, c: Carattere, corpo: i64, massimo: i64) -> Vec<String> {
    righe_paragrafi(testo, c, corpo, massimo)
        .into_iter()
        .map(|(r, _)| r)
        .collect()
}

/// Descrizione ridotta come nelle stampe dei computi: i primi 180 caratteri, « ... » e
/// gli ultimi 120, se la descrizione è più lunga di 300 caratteri.
fn riduci(testo: &str) -> String {
    let caratteri: Vec<char> = testo.trim().chars().collect();
    if caratteri.len() <= 300 {
        return testo.trim().to_owned();
    }
    let inizio: String = caratteri.iter().take(180).collect();
    let fine: String = caratteri.iter().skip(caratteri.len() - 120).collect();
    format!("{inizio} ... {fine}")
}

/// Numero con l'apice per le migliaia.
fn numero(v: Decimal, decimali: u32) -> String {
    formato::numero_apice(v, decimali)
}

// ---------------------------------------------------------------------------
// Disegno.

/// Colore PDF (`r g b` tra 0 e 1) in notazione CSS, per l'anteprima.
fn css(rgb: &str) -> String {
    let mut canali = rgb.split_whitespace().map(|c| {
        c.parse::<Decimal>()
            .ok()
            .and_then(|d| (d * Decimal::from(255)).round().try_into().ok())
            .unwrap_or(0u8)
    });
    let (r, g, b) = (
        canali.next().unwrap_or(0),
        canali.next().unwrap_or(0),
        canali.next().unwrap_or(0),
    );
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Testo di un nodo o di un attributo XML.
fn xml(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Caratteri di controllo: non ammessi in XML.
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Contenuto di una pagina: le operazioni PDF e, in parallelo, lo stesso disegno in SVG
/// per l'anteprima (stesse coordinate, in punti dall'alto della pagina).
#[derive(Debug, Default)]
struct Tela {
    ops: String,
    svg: String,
    sostituiti: usize,
}

impl Tela {
    fn testo(&mut self, x: i64, alto: i64, t: &str, c: Carattere, corpo: i64, colore: Colore) {
        self.testo_spaziato(x, alto, t, c, corpo, colore, 0);
    }

    #[allow(clippy::too_many_arguments)]
    fn testo_spaziato(
        &mut self,
        x: i64,
        alto: i64,
        t: &str,
        c: Carattere,
        corpo: i64,
        colore: Colore,
        spazio: i64,
    ) {
        if t.is_empty() {
            return;
        }
        let byte = pdf::codifica(t, &mut self.sostituiti);
        let base = alto + corpo * (1000 - c.discesa()) / 1000;
        // La spaziatura delle parole fa parte dello stato grafico: va sempre impostata.
        let tw = format!("{} Tw ", pdf::pt(spazio));
        let _ = writeln!(
            self.ops,
            "BT /{} {} Tf {} rg {tw}{} {} Td {} Tj ET",
            c.risorsa(),
            pdf::pt(corpo),
            colore.rgb(),
            pdf::pt(x),
            pdf::pt(ALTEZZA - base),
            pdf::stringa_pdf(&byte)
        );
        // Anteprima: `textLength` fissa la larghezza calcolata con le metriche del PDF,
        // così l'impaginazione è la stessa anche se il carattere a video è diverso.
        let spazi = i64::try_from(t.matches(' ').count()).unwrap_or(0);
        let lunga = larghezza(t, c, corpo) + spazi * spazio;
        let (famiglia, peso) = c.css();
        let _ = writeln!(
            self.svg,
            "<text x=\"{}\" y=\"{}\" font-family=\"{famiglia}\" font-weight=\"{peso}\" font-size=\"{}\" fill=\"{}\" word-spacing=\"{}\" textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\" xml:space=\"preserve\">{}</text>",
            pdf::pt(x),
            pdf::pt(base),
            pdf::pt(corpo),
            css(colore.rgb()),
            pdf::pt(spazio),
            pdf::pt(lunga.max(1)),
            xml(t)
        );
    }

    fn testo_destra(&mut self, x1: i64, alto: i64, t: &str, c: Carattere, corpo: i64, k: Colore) {
        let w = larghezza(t, c, corpo);
        self.testo(x1 - w, alto, t, c, corpo, k);
    }

    #[allow(clippy::too_many_arguments)]
    fn testo_centro(
        &mut self,
        (x0, x1): (i64, i64),
        alto: i64,
        t: &str,
        c: Carattere,
        corpo: i64,
        k: Colore,
    ) {
        // Un testo più largo dello spazio (titolo di una colonna stretta) si rimpicciolisce
        // fino al 60 % del corpo, centrato in verticale, per non toccare i filetti.
        let disponibile = (x1 - x0 - 2 * RIENTRO).max(1);
        let w = larghezza(t, c, corpo);
        let (corpo_usato, alto) = if w > disponibile {
            let ridotto = (corpo * disponibile / w).max(corpo * 6 / 10);
            (ridotto, alto + (corpo - ridotto) / 2)
        } else {
            (corpo, alto)
        };
        let w = larghezza(t, c, corpo_usato);
        self.testo(x0 + (x1 - x0 - w) / 2, alto, t, c, corpo_usato, k);
    }

    /// Testo giustificato tra `x0` e `x1` con la spaziatura delle parole.
    fn testo_giustificato(&mut self, (x0, x1): (i64, i64), alto: i64, t: &str, c: Carattere) {
        let spazi = i64::try_from(t.matches(' ').count()).unwrap_or(0);
        let w = larghezza(t, c, CORPO);
        let extra = if spazi > 0 && w < x1 - x0 {
            (x1 - x0 - w) / spazi
        } else {
            0
        };
        self.testo_spaziato(x0, alto, t, c, CORPO, Colore::Nero, extra);
    }

    fn linea(&mut self, (x0, y0): (i64, i64), (x1, y1): (i64, i64), spessore: i64, k: Colore) {
        let _ = writeln!(
            self.ops,
            "{} RG {} w {} {} m {} {} l S",
            k.rgb(),
            pdf::pt(spessore),
            pdf::pt(x0),
            pdf::pt(ALTEZZA - y0),
            pdf::pt(x1),
            pdf::pt(ALTEZZA - y1)
        );
        let _ = writeln!(
            self.svg,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
            pdf::pt(x0),
            pdf::pt(y0),
            pdf::pt(x1),
            pdf::pt(y1),
            css(k.rgb()),
            pdf::pt(spessore)
        );
    }

    fn orizzontale(&mut self, x0: i64, x1: i64, y: i64, spessore: i64, k: Colore) {
        self.linea((x0, y), (x1, y), spessore, k);
    }

    fn verticale(&mut self, x: i64, y0: i64, y1: i64, spessore: i64, k: Colore) {
        self.linea((x, y0), (x, y1), spessore, k);
    }

    fn rettangolo(&mut self, (x0, y0): (i64, i64), (x1, y1): (i64, i64), spessore: i64, k: Colore) {
        let _ = writeln!(
            self.ops,
            "{} RG {} w {} {} {} {} re S",
            k.rgb(),
            pdf::pt(spessore),
            pdf::pt(x0),
            pdf::pt(ALTEZZA - y1),
            pdf::pt(x1 - x0),
            pdf::pt(y1 - y0)
        );
        let _ = writeln!(
            self.svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
            pdf::pt(x0),
            pdf::pt(y0),
            pdf::pt(x1 - x0),
            pdf::pt(y1 - y0),
            css(k.rgb()),
            pdf::pt(spessore)
        );
    }

    fn riempi(&mut self, (x0, y0): (i64, i64), (x1, y1): (i64, i64), rgb: &str) {
        let _ = writeln!(
            self.ops,
            "{rgb} rg {} {} {} {} re f 0 0 0 rg",
            pdf::pt(x0),
            pdf::pt(ALTEZZA - y1),
            pdf::pt(x1 - x0),
            pdf::pt(y1 - y0)
        );
        let _ = writeln!(
            self.svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            pdf::pt(x0),
            pdf::pt(y0),
            pdf::pt(x1 - x0),
            pdf::pt(y1 - y0),
            css(rgb)
        );
    }
}

// ---------------------------------------------------------------------------
// Righe del corpo e impaginazione.

/// Allineamento di un testo nel corpo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Allineamento {
    Sinistra,
    Destra,
    Centro,
    Giustificato,
}

/// Testo di una riga del corpo, tra due ascisse.
#[derive(Debug, Clone)]
struct Testo {
    x: (i64, i64),
    allineamento: Allineamento,
    testo: String,
    carattere: Carattere,
    colore: Colore,
    sottolineato: bool,
}

impl Testo {
    fn new(x: (i64, i64), allineamento: Allineamento, testo: impl Into<String>) -> Self {
        Self {
            x,
            allineamento,
            testo: testo.into(),
            carattere: Carattere::Times,
            colore: Colore::Nero,
            sottolineato: false,
        }
    }

    fn grassetto(mut self) -> Self {
        self.carattere = Carattere::TimesGrassetto;
        self
    }

    fn colore(mut self, k: Colore) -> Self {
        self.colore = k;
        self
    }

    fn sottolineato(mut self) -> Self {
        self.sottolineato = true;
        self
    }
}

/// Filetto nero orizzontale sopra (o sotto) una riga.
#[derive(Debug, Clone, Copy)]
struct Filetto {
    x: (i64, i64),
    spessore: i64,
    sotto: bool,
    colore: Colore,
}

/// Valori dei riporti dopo una riga: quantità della voce in corso e importo progressivo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Riporto {
    quantita: Option<Decimal>,
    totale: Option<Decimal>,
    /// Progressivi delle quote di manodopera e sicurezza nelle loro colonne (ADR 0021).
    quote: [Option<Decimal>; 2],
}

/// Riga del corpo.
#[derive(Debug, Clone, Default)]
struct Linea {
    testi: Vec<Testo>,
    filetti: Vec<Filetto>,
    /// Riga vuota che non si stampa in cima a una pagina.
    eliminabile: bool,
    riporto: Riporto,
    /// Filetti verticali verdi per l'altezza della riga (colonne delle tabelle dei
    /// documenti senza intestazione fissa di pagina).
    verticali: Vec<i64>,
}

impl Linea {
    fn con(testi: Vec<Testo>) -> Self {
        Self {
            testi,
            ..Self::default()
        }
    }

    fn filetto(mut self, x: (i64, i64), spessore: i64, sotto: bool) -> Self {
        self.filetti.push(Filetto {
            x,
            spessore,
            sotto,
            colore: Colore::Nero,
        });
        self
    }

    fn filetto_verde(mut self, x: (i64, i64), spessore: i64, sotto: bool) -> Self {
        self.filetti.push(Filetto {
            x,
            spessore,
            sotto,
            colore: Colore::Verde,
        });
        self
    }
}

/// Elemento del corpo.
#[derive(Debug, Clone)]
enum Elemento {
    Linea(Linea),
    /// Completa la pagina con le righe di tratteggio e passa alla successiva.
    Chiudi,
}

/// Raccoglie le righe con lo stato dei riporti.
#[derive(Debug, Default)]
struct Corpo {
    elementi: Vec<Elemento>,
    riporto: Riporto,
}

impl Corpo {
    fn linea(&mut self, mut l: Linea) {
        l.riporto = self.riporto;
        self.elementi.push(Elemento::Linea(l));
    }

    fn vuota(&mut self) {
        self.linea(Linea {
            eliminabile: true,
            ..Linea::default()
        });
    }

    /// Riga vuota stampata anche in cima a una pagina.
    fn spazio(&mut self) {
        self.linea(Linea::default());
    }

    fn chiudi(&mut self) {
        self.elementi.push(Elemento::Chiudi);
    }
}

/// Pagina impaginata: righe e riporti in testa e in calce.
#[derive(Debug, Default)]
struct PaginaCorpo {
    linee: Vec<Linea>,
    riporto_iniziale: Riporto,
    riporto_finale: Riporto,
}

/// Distribuisce le righe nelle pagine. `tratteggio` è la riga che completa le pagine
/// chiuse.
fn impagina(elementi: Vec<Elemento>, capacita: usize, tratteggio: &Testo) -> Vec<PaginaCorpo> {
    let mut pagine: Vec<Vec<Linea>> = vec![Vec::new()];
    let mut forza = false;
    for e in elementi {
        match e {
            Elemento::Linea(l) => {
                let piena = pagine.last().is_some_and(|p| p.len() >= capacita);
                if forza || piena {
                    pagine.push(Vec::new());
                    forza = false;
                }
                let Some(p) = pagine.last_mut() else { continue };
                if l.eliminabile && p.is_empty() {
                    continue;
                }
                p.push(l);
            }
            Elemento::Chiudi => {
                let Some(p) = pagine.last_mut() else { continue };
                if p.is_empty() {
                    continue;
                }
                let riporto = p.last().map(|l| l.riporto).unwrap_or_default();
                while p.len() < capacita {
                    p.push(Linea {
                        testi: vec![tratteggio.clone()],
                        riporto,
                        ..Linea::default()
                    });
                }
                forza = true;
            }
        }
    }
    if pagine.len() > 1 && pagine.last().is_some_and(Vec::is_empty) {
        pagine.pop();
    }
    let n = pagine.len();
    let mut out = Vec::with_capacity(n);
    let mut precedente = Riporto::default();
    for (i, linee) in pagine.into_iter().enumerate() {
        let finale = if i + 1 == n {
            Riporto::default()
        } else {
            linee.last().map(|l| l.riporto).unwrap_or(precedente)
        };
        out.push(PaginaCorpo {
            linee,
            riporto_iniziale: precedente,
            riporto_finale: finale,
        });
        precedente = finale;
    }
    out
}

/// Ordinata del bordo superiore della riga `k`.
fn alto_riga(inizio: i64, k: usize) -> i64 {
    inizio + i64::try_from(k).unwrap_or(0) * PASSO_10K / 100
}

/// Numero di righe che entrano tra `inizio` (bordo della prima riga) e `fondo`.
fn capacita(inizio: i64, scarto: i64, fondo: i64) -> usize {
    let mut k = 0;
    while alto_riga(inizio, k) + scarto + CORPO <= fondo {
        k += 1;
    }
    k
}

/// Disegna le righe del corpo: `inizio` è il bordo della prima riga, `scarto` la
/// distanza del testo dal bordo.
fn disegna_linee(tela: &mut Tela, linee: &[Linea], inizio: i64, scarto: i64) {
    for (k, l) in linee.iter().enumerate() {
        let bordo = alto_riga(inizio, k);
        let alto = bordo + scarto;
        for f in &l.filetti {
            let y = if f.sotto {
                alto_riga(inizio, k + 1)
            } else {
                bordo
            };
            tela.orizzontale(f.x.0, f.x.1, y, f.spessore, f.colore);
        }
        for x in &l.verticali {
            tela.verticale(*x, bordo, alto_riga(inizio, k + 1), SOTTILE, Colore::Verde);
        }
        for t in &l.testi {
            let w = larghezza(&t.testo, t.carattere, CORPO);
            let x = match t.allineamento {
                Allineamento::Sinistra => {
                    tela.testo(t.x.0, alto, &t.testo, t.carattere, CORPO, t.colore);
                    t.x.0
                }
                Allineamento::Destra => {
                    tela.testo(t.x.1 - w, alto, &t.testo, t.carattere, CORPO, t.colore);
                    t.x.1 - w
                }
                Allineamento::Centro => {
                    let x = t.x.0 + (t.x.1 - t.x.0 - w) / 2;
                    tela.testo(x, alto, &t.testo, t.carattere, CORPO, t.colore);
                    x
                }
                Allineamento::Giustificato => {
                    tela.testo_giustificato(t.x, alto, &t.testo, t.carattere);
                    t.x.0
                }
            };
            if t.sottolineato {
                tela.orizzontale(x, x + w, alto + 735, 43, t.colore);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parti comuni delle pagine.

/// Dati del progetto usati nelle pagine.
#[derive(Debug, Clone)]
struct Intestazione {
    studio: Vec<String>,
    ente: Vec<String>,
    oggetto: String,
    committente: String,
    luogo_data: String,
    tecnico: Option<String>,
    /// Ruolo di chi firma («Il Tecnico», «Il Direttore dei Lavori»…).
    ruolo: String,
}

impl Intestazione {
    fn new(p: &Progetto, o: &OpzioniTradizionale) -> Self {
        let m = &p.metadati;
        let pulito = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_owned)
        };
        let righe_pulite = |v: &[String]| -> Vec<String> {
            v.iter()
                .flat_map(|r| r.lines())
                .map(|r| r.trim().to_owned())
                .filter(|r| !r.is_empty())
                .collect()
        };
        let mut ente = righe_pulite(&o.ente);
        if ente.is_empty()
            && let Some(l) = pulito(&m.luogo)
        {
            ente.push(l);
        }
        let data = m.data.or(o.data_stampa).map(formato::data);
        let luogo_data = match (pulito(&m.luogo), data) {
            (Some(l), Some(d)) => format!("{l}, {d}"),
            (Some(l), None) => format!("{l},"),
            (None, Some(d)) => format!("Data, {d}"),
            (None, None) => "Data,".to_owned(),
        };
        Self {
            studio: righe_pulite(&o.studio),
            ente,
            oggetto: pulito(&m.oggetto).unwrap_or_else(|| m.titolo.trim().to_owned()),
            committente: pulito(&m.committente).unwrap_or_default(),
            luogo_data,
            tecnico: match &o.firma {
                Some(f) => f
                    .nome
                    .as_deref()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(str::to_owned),
                None => pulito(&m.progettista),
            },
            ruolo: o
                .firma
                .as_ref()
                .map_or_else(|| "Il Tecnico".to_owned(), |f| f.ruolo.clone()),
        }
    }

    /// Ordinata del riquadro della tabella, sotto la testata.
    fn alto_riquadro(&self) -> i64 {
        let righe = i64::try_from(self.studio.len().max(1)).unwrap_or(1);
        Y_TESTATA + (righe - 1) * 964 + 1_230
    }

    /// Testata (studio e numero di pagina), riquadro esterno e committente in calce.
    fn cornice(&self, tela: &mut Tela, pagina: usize, alto: i64) {
        let mut y = Y_TESTATA;
        for (i, r) in self.studio.iter().enumerate() {
            if i > 0 {
                y += 964;
            }
            tela.testo(1_899, y, r, Carattere::Times, CORPO, Colore::Nero);
        }
        tela.testo_destra(
            57_593,
            y,
            &format!("pag. {pagina}"),
            Carattere::Times,
            CORPO,
            Colore::Nero,
        );
        tela.rettangolo(
            (X_SINISTRA, alto),
            (X_DESTRA, Y_FONDO),
            SPESSO,
            Colore::Verde,
        );
        tela.rettangolo(
            (X_SINISTRA - 170, alto - 170),
            (X_DESTRA + 170, Y_FONDO + 170),
            SOTTILE,
            Colore::Verde,
        );
        tela.testo(
            1_899,
            Y_COMMITTENTE,
            &format!("COMMITTENTE: {}", self.committente),
            Carattere::Times,
            CORPO,
            Colore::Nero,
        );
    }

    /// Frontespizio (pagina 1).
    fn frontespizio(&self, tela: &mut Tela, titolo: &str) {
        let nero = Colore::Nero;
        // Riquadri e fascia gialla.
        tela.riempi((3_978, 18_163), (15_343, 75_459), "1 1 0");
        tela.rettangolo((3_978, 3_967), (55_690, 18_163), SOTTILE, nero);
        tela.rettangolo((15_343, 18_163), (55_690, 75_459), SOTTILE, nero);
        tela.rettangolo((3_978, 75_459), (55_690, 79_434), SOTTILE, nero);
        tela.verticale(3_978, 3_967, 79_434, 170, nero);
        tela.orizzontale(3_978, 55_690, 79_434, 170, nero);
        for x in [4_404, 4_688] {
            tela.verticale(x, 3_967, 18_163, SOTTILE, nero);
            tela.verticale(x, 75_459, 79_434, SOTTILE, nero);
        }
        tela.orizzontale(3_978, 4_688, 75_459, SOTTILE, nero);
        tela.orizzontale(3_978, 15_343, 18_163, SOTTILE, nero);

        let x = 15_913;
        let destra = 55_100;
        let blu = Colore::Blu;
        for (i, r) in self.ente.iter().take(4).enumerate() {
            let y = 9_857 + i64::try_from(i).unwrap_or(0) * 2_254;
            if i == 0 {
                tela.testo(x, y, r, Carattere::ArialGrassetto, 2_019, blu);
            } else {
                tela.testo(x, y, r, Carattere::Arial, 1_815, blu);
            }
        }
        tela.testo_destra(55_120, 18_798, "pag. 1", Carattere::Arial, 793, nero);

        // Titolo ridotto alla larghezza disponibile.
        let mut corpo = 3_606;
        let w = larghezza(titolo, Carattere::ArialGrassetto, corpo);
        if w > 37_500 {
            corpo = corpo * 37_500 / w;
        }
        let alto_titolo = 28_200 - corpo / 2;
        tela.testo(
            x,
            alto_titolo,
            titolo,
            Carattere::ArialGrassetto,
            corpo,
            Colore::Bordeaux,
        );

        // Oggetto, committente, data, tecnico.
        let passo = 1_344;
        let etichetta = |tela: &mut Tela, y: i64, t: &str| {
            tela.testo_destra(
                14_770,
                y,
                t,
                Carattere::ArialGrassetto,
                1_190,
                Colore::Bordeaux,
            );
        };
        let mut y = 35_347;
        etichetta(tela, y, "OGGETTO:");
        let righe_oggetto = righe(&self.oggetto, Carattere::Arial, 1_190, destra - x);
        for (i, r) in righe_oggetto.iter().enumerate() {
            tela.testo(
                x,
                y + i64::try_from(i).unwrap_or(0) * passo,
                r,
                Carattere::Arial,
                1_190,
                nero,
            );
        }
        let n = i64::try_from(righe_oggetto.len().max(1)).unwrap_or(1);
        y = (y + (n + 1) * passo).max(42_162);
        etichetta(tela, y, "COMMITTENTE:");
        let righe_committente = righe(&self.committente, Carattere::Arial, 1_190, destra - x);
        for (i, r) in righe_committente.iter().enumerate() {
            tela.testo(
                x,
                y + i64::try_from(i).unwrap_or(0) * passo,
                r,
                Carattere::Arial,
                1_190,
                nero,
            );
        }
        let n = i64::try_from(righe_committente.len().max(1)).unwrap_or(1);
        y = (y + (n + 4) * passo).max(50_179);
        tela.testo(x, y, &self.luogo_data, Carattere::Arial, 1_190, nero);
        y = (y + 4 * passo).max(56_417);
        let centro = (35_000, 47_548);
        tela.testo_centro(
            centro,
            y,
            &self.ruolo.to_uppercase(),
            Carattere::ArialGrassetto,
            1_190,
            nero,
        );
        if let Some(t) = &self.tecnico {
            tela.testo_centro(centro, y + passo, t, Carattere::Arial, 1_190, nero);
        }

        // Studio nel riquadro in basso.
        let n = i64::try_from(self.studio.len().min(3)).unwrap_or(0);
        let inizio = 76_438 - (n - 2).max(0) * 553;
        for (i, r) in self.studio.iter().take(3).enumerate() {
            let y = inizio + i64::try_from(i).unwrap_or(0) * 1_106;
            tela.testo(x, y, r, Carattere::Arial, 998, blu);
        }
    }
}

/// Assembla le pagine nel file PDF.
fn file_pdf(pagine: Vec<Tela>, titolo: &str, comprimi: bool) -> RisultatoPdf {
    let sostituiti = pagine.iter().map(|t| t.sostituiti).sum();
    let contenuti: Vec<&str> = pagine.iter().map(|t| t.ops.as_str()).collect();
    let byte = pdf::assembla(
        &contenuti,
        (LARGHEZZA, ALTEZZA),
        &FONT_PDF,
        titolo,
        comprimi,
    );
    let numero = contenuti.len();
    RisultatoPdf {
        byte,
        pagine: numero,
        caratteri_sostituiti: sostituiti,
        anteprima: pagine.into_iter().map(|t| pagina_svg(&t.svg)).collect(),
    }
}

/// Pagina dell'anteprima: un documento SVG con le dimensioni della pagina in punti e il
/// fondo bianco.
fn pagina_svg(disegno: &str) -> String {
    let (w, h) = (pdf::pt(LARGHEZZA), pdf::pt(ALTEZZA));
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}pt\" height=\"{h}pt\"><rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n{disegno}</svg>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titolo_centrato_rimpicciolito_nelle_colonne_strette() {
        // Colonna d'incidenza di 36 pt: «INCIDENZA» al corpo pieno non entra.
        let colonna = (50_000, 53_600);
        let disponibile = colonna.1 - colonna.0 - 2 * RIENTRO;
        assert!(larghezza("INCIDENZA", Carattere::Times, CORPO) > disponibile);
        let mut t = Tela::default();
        t.testo_centro(
            colonna,
            1_000,
            "INCIDENZA",
            Carattere::Times,
            CORPO,
            Colore::Verde,
        );
        let w = larghezza("INCIDENZA", Carattere::Times, CORPO);
        let atteso = (CORPO * disponibile / w).max(CORPO * 6 / 10);
        assert!(atteso < CORPO);
        assert!(larghezza("INCIDENZA", Carattere::Times, atteso) <= disponibile);
        assert!(
            t.ops.contains(&format!("/F1 {} Tf", pdf::pt(atteso))),
            "{}",
            t.ops
        );
        // Un titolo che entra resta al corpo pieno.
        let mut t = Tela::default();
        t.testo_centro(
            (0, 20_000),
            1_000,
            "INCIDENZA",
            Carattere::Times,
            CORPO,
            Colore::Verde,
        );
        assert!(t.ops.contains(&format!("/F1 {} Tf", pdf::pt(CORPO))));
    }

    #[test]
    fn descrizioni_ridotte() {
        let breve = "a".repeat(300);
        assert_eq!(riduci(&breve), breve);
        let lunga: String = (0..400)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let r = riduci(&lunga);
        assert_eq!(r.chars().count(), 180 + 5 + 120);
        assert!(r.contains(" ... "));
        assert!(r.starts_with(&lunga[..180]));
        assert!(r.ends_with(&lunga[280..]));
    }

    #[test]
    fn impaginazione_con_riporti_e_chiusura() {
        let t = Testo::new((0, 100), Allineamento::Centro, "---");
        let mut c = Corpo::default();
        c.vuota();
        for i in 0..5 {
            c.riporto.totale = Some(Decimal::from(i));
            c.linea(Linea::con(vec![Testo::new(
                (0, 1),
                Allineamento::Sinistra,
                "x",
            )]));
        }
        c.chiudi();
        c.vuota();
        c.linea(Linea::con(vec![Testo::new(
            (0, 1),
            Allineamento::Sinistra,
            "y",
        )]));
        let pagine = impagina(c.elementi, 3, &t);
        // La riga vuota iniziale è scartata; 5 righe su 3 per pagina; poi la chiusura
        // completa la seconda pagina con un tratteggio e la riga «y» va sulla terza.
        assert_eq!(pagine.len(), 3);
        assert_eq!(pagine[0].linee.len(), 3);
        assert_eq!(pagine[1].linee.len(), 3);
        assert_eq!(pagine[1].linee[2].testi[0].testo, "---");
        assert_eq!(pagine[2].linee.len(), 1);
        assert_eq!(pagine[0].riporto_iniziale, Riporto::default());
        assert_eq!(pagine[0].riporto_finale.totale, Some(Decimal::from(2)));
        assert_eq!(pagine[1].riporto_iniziale.totale, Some(Decimal::from(2)));
        assert_eq!(pagine[2].riporto_iniziale.totale, Some(Decimal::from(4)));
        assert_eq!(pagine[2].riporto_finale, Riporto::default());
    }
}
