//! Scrittura PDF dei report, senza dipendenze esterne di impaginazione.
//!
//! * Pagine A4 (verticali o orizzontali), font standard PDF Helvetica con codifica
//!   WinAnsi: lettere accentate italiane, `€`, `²`, `³` sono rappresentati; i simboli
//!   tecnici frequenti (`≤`, `≥`, `λ`, `Δ`…) sono trascritti; gli altri caratteri fuori
//!   da Windows-1252 diventano `?` e sono contati nel risultato.
//! * Tabelle con intestazione ripetuta a ogni pagina, testo a capo nelle colonne
//!   descrittive, righe lunghe spezzate tra le pagine, numeri allineati a destra.
//! * Intestazione e piè di pagina con titolo, oggetto e «Pagina n di N».
//! * Uscita **deterministica**: nessuna data o identificativo casuale nel file.
//!
//! Le coordinate sono intere, in centesimi di punto tipografico: il workspace vieta
//! l'aritmetica in virgola mobile.
//!
//! Limite noto: i font non sono incorporati (i font standard PDF sono disponibili in
//! ogni lettore), quindi il file non è conforme a PDF/A.

use std::fmt::Write as _;
use std::io::Write as _;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::documento::{Allineamento, Documento, Riga, Sezione, StileRiga, Tabella};
use crate::metriche;

/// Opzioni di scrittura.
#[derive(Debug, Clone)]
pub struct OpzioniPdf {
    /// Comprime i contenuti delle pagine (zlib).
    pub comprimi: bool,
}

impl Default for OpzioniPdf {
    fn default() -> Self {
        Self { comprimi: true }
    }
}

/// Risultato della scrittura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RisultatoPdf {
    /// Byte del file PDF.
    pub byte: Vec<u8>,
    /// Numero di pagine.
    pub pagine: usize,
    /// Caratteri non rappresentabili sostituiti con `?`.
    pub caratteri_sostituiti: usize,
}

// Misure in centesimi di punto.
const A4_LATO_CORTO: i64 = 59_528;
const A4_LATO_LUNGO: i64 = 84_189;
const MARGINE: i64 = 4_000;
const SPAZIO_TESTATA: i64 = 2_200;
const SPAZIO_PIEDE: i64 = 2_000;
const PAD_ORIZZ: i64 = 200;
const PAD_VERT: i64 = 140;

const CORPO: i64 = 800;
const CORPO_RIGO: i64 = 700;
const CORPO_TITOLO: i64 = 1_400;
const CORPO_SEZIONE: i64 = 1_050;
const CORPO_INTESTAZIONE: i64 = 900;
const CORPO_PICCOLO: i64 = 650;

/// Font standard usati.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Font {
    Normale,
    Grassetto,
    Corsivo,
}

impl Font {
    fn nome_risorsa(self) -> &'static str {
        match self {
            Self::Normale => "F1",
            Self::Grassetto => "F2",
            Self::Corsivo => "F3",
        }
    }

    fn larghezze(self) -> &'static [u16; 256] {
        match self {
            Self::Grassetto => &metriche::HELVETICA_BOLD,
            Self::Normale | Self::Corsivo => &metriche::HELVETICA,
        }
    }
}

/// Codice WinAnsi (Windows-1252) di un carattere.
fn winansi(c: char) -> Option<u8> {
    let n = u32::from(c);
    match n {
        0x20..=0x7E | 0xA0..=0xFF => u8::try_from(n).ok(),
        _ => Some(match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            '\t' => b' ',
            '−' => b'-',
            _ => return None,
        }),
    }
}

/// Trascrizione dei simboli tecnici frequenti nei prezzari che non esistono in
/// WinAnsi (es. «λ ≤ 0,035 W/mK»), così il significato non va perso.
fn trascrizione(c: char) -> Option<&'static str> {
    Some(match c {
        '≤' => "<=",
        '≥' => ">=",
        '≠' => "<>",
        '≈' => "~",
        '∕' | '⁄' => "/",
        'Δ' => "Delta ",
        'λ' => "lambda ",
        'Ω' => "Ohm",
        'α' => "alfa ",
        'β' => "beta ",
        'φ' | 'ϕ' => "fi ",
        'ρ' => "rho ",
        'σ' => "sigma ",
        'τ' => "tau ",
        'π' => "pi ",
        // Mu greca → segno di micro (presente in WinAnsi); U+0085 è un «…» codificato male.
        'μ' => "\u{b5}",
        '\u{85}' => "…",
        _ => return None,
    })
}

/// Testo convertito in WinAnsi con il conteggio dei caratteri sostituiti.
pub(crate) fn codifica(testo: &str, sostituiti: &mut usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(testo.len());
    for c in testo.chars().filter(|c| *c != '\r' && *c != '\n') {
        if let Some(b) = winansi(c) {
            out.push(b);
        } else if let Some(t) = trascrizione(c) {
            out.extend(t.chars().filter_map(winansi));
        } else {
            *sostituiti += 1;
            out.push(b'?');
        }
    }
    out
}

/// Larghezza di un testo già codificato, in centesimi di punto.
fn larghezza_byte(byte: &[u8], font: Font, corpo: i64) -> i64 {
    let w = font.larghezze();
    let somma: i64 = byte
        .iter()
        .map(|b| i64::from(w.get(usize::from(*b)).copied().unwrap_or(500)))
        .sum();
    somma * corpo / 1000
}

fn larghezza(testo: &str, font: Font, corpo: i64) -> i64 {
    let mut ignorati = 0;
    larghezza_byte(&codifica(testo, &mut ignorati), font, corpo)
}

/// Divide il testo in righe entro la larghezza indicata (a capo sulle parole;
/// le parole troppo lunghe sono spezzate).
fn a_capo(testo: &str, font: Font, corpo: i64, massimo: i64) -> Vec<String> {
    a_capo_misurato(testo, &|t| larghezza(t, font, corpo), massimo)
}

/// Come [`a_capo`], con una funzione che misura la larghezza del testo.
pub(crate) fn a_capo_misurato(
    testo: &str,
    misura: &dyn Fn(&str) -> i64,
    massimo: i64,
) -> Vec<String> {
    let mut righe = Vec::new();
    for paragrafo in testo.split('\n') {
        let mut corrente = String::new();
        for parola in paragrafo.split_whitespace() {
            let candidata = if corrente.is_empty() {
                parola.to_owned()
            } else {
                format!("{corrente} {parola}")
            };
            if misura(&candidata) <= massimo {
                corrente = candidata;
                continue;
            }
            if !corrente.is_empty() {
                righe.push(std::mem::take(&mut corrente));
            }
            if misura(parola) <= massimo {
                corrente = parola.to_owned();
            } else {
                // Parola più larga della colonna: spezzata per caratteri.
                for c in parola.chars() {
                    let mut prova = corrente.clone();
                    prova.push(c);
                    if misura(&prova) > massimo && !corrente.is_empty() {
                        righe.push(std::mem::take(&mut corrente));
                        corrente.push(c);
                    } else {
                        corrente = prova;
                    }
                }
            }
        }
        righe.push(corrente);
    }
    while righe.len() > 1 && righe.last().is_some_and(String::is_empty) {
        righe.pop();
    }
    righe
}

/// Numero in centesimi di punto scritto come numero PDF (`123.45`).
pub(crate) fn pt(v: i64) -> String {
    let segno = if v < 0 { "-" } else { "" };
    let a = v.abs();
    let (intera, cent) = (a / 100, a % 100);
    if cent == 0 {
        format!("{segno}{intera}")
    } else {
        format!("{segno}{intera}.{cent:02}")
    }
}

/// Stringa PDF letterale.
pub(crate) fn stringa_pdf(byte: &[u8]) -> String {
    let mut s = String::with_capacity(byte.len() + 2);
    s.push('(');
    for b in byte {
        match b {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(char::from(*b));
            }
            0x20..=0x7E => s.push(char::from(*b)),
            _ => {
                let _ = write!(s, "\\{b:03o}");
            }
        }
    }
    s.push(')');
    s
}

struct Pagina {
    contenuto: String,
}

struct Impaginatore {
    larghezza: i64,
    altezza: i64,
    pagine: Vec<Pagina>,
    y: i64,
    sostituiti: usize,
    titolo: String,
    oggetto: String,
    piede: String,
}

impl Impaginatore {
    fn sinistra(&self) -> i64 {
        MARGINE
    }

    fn destra(&self) -> i64 {
        self.larghezza - MARGINE
    }

    fn alto(&self) -> i64 {
        self.altezza - MARGINE - SPAZIO_TESTATA
    }

    fn basso(&self) -> i64 {
        MARGINE + SPAZIO_PIEDE
    }

    fn nuova_pagina(&mut self) {
        self.pagine.push(Pagina {
            contenuto: String::new(),
        });
        self.y = self.alto();
    }

    fn ops(&mut self) -> &mut String {
        if self.pagine.is_empty() {
            self.nuova_pagina();
        }
        match self.pagine.last_mut() {
            Some(p) => &mut p.contenuto,
            None => unreachable!("una pagina è appena stata aggiunta"),
        }
    }

    fn testo(&mut self, x: i64, y: i64, testo: &str, font: Font, corpo: i64) {
        let byte = codifica(testo, &mut self.sostituiti);
        let s = format!(
            "BT /{} {} Tf {} {} Td {} Tj ET\n",
            font.nome_risorsa(),
            pt(corpo),
            pt(x),
            pt(y),
            stringa_pdf(&byte)
        );
        self.ops().push_str(&s);
    }

    /// Testo allineato nell'intervallo orizzontale `(x0, x1)`.
    fn testo_allineato(
        &mut self,
        (x0, x1): (i64, i64),
        y: i64,
        testo: &str,
        font: Font,
        corpo: i64,
        all: Allineamento,
    ) {
        let w = larghezza(testo, font, corpo);
        let x = match all {
            Allineamento::Sinistra => x0,
            Allineamento::Destra => x1 - w,
            Allineamento::Centro => x0 + (x1 - x0 - w) / 2,
        };
        self.testo(x, y, testo, font, corpo);
    }

    fn linea(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, spessore: i64) {
        let s = format!(
            "{} w {} {} m {} {} l S\n",
            pt(spessore),
            pt(x0),
            pt(y0),
            pt(x1),
            pt(y1)
        );
        self.ops().push_str(&s);
    }

    fn riempi(&mut self, x: i64, y: i64, w: i64, h: i64, grigio: &str) {
        let s = format!(
            "{grigio} g {} {} {} {} re f 0 g\n",
            pt(x),
            pt(y),
            pt(w),
            pt(h)
        );
        self.ops().push_str(&s);
    }

    /// Spazio verticale disponibile sulla pagina corrente.
    fn disponibile(&self) -> i64 {
        self.y - self.basso()
    }
}

/// Larghezze delle colonne: le colonne numeriche e centrate prendono la larghezza
/// del loro contenuto, quelle di testo si dividono il resto in proporzione ai pesi.
fn larghezze_colonne(t: &Tabella, totale: i64) -> Vec<i64> {
    let n = t.colonne.len();
    let mut w = vec![0i64; n];
    let mut fisse = 0i64;
    let mut peso_testo = 0i64;
    for (i, c) in t.colonne.iter().enumerate() {
        let parole_titolo = c
            .titolo
            .split_whitespace()
            .map(|p| larghezza(p, Font::Grassetto, CORPO))
            .max()
            .unwrap_or(0);
        if c.allineamento == Allineamento::Sinistra {
            peso_testo += i64::from(c.peso.max(1));
            if let Some(x) = w.get_mut(i) {
                *x = parole_titolo + 2 * PAD_ORIZZ;
            }
            continue;
        }
        let contenuto = t
            .righe
            .iter()
            .filter(|r| r.stile != StileRiga::Gruppo)
            .filter_map(|r| r.celle.get(i))
            .map(|v| {
                let font = Font::Grassetto;
                larghezza(&v.in_testo(), font, CORPO)
            })
            .max()
            .unwrap_or(0);
        // Il titolo resta su una riga se non è molto più largo del contenuto.
        let titolo_intero = larghezza(&c.titolo, Font::Grassetto, CORPO);
        let titolo = if titolo_intero <= contenuto * 2 {
            titolo_intero
        } else {
            parole_titolo
        };
        let minimo = contenuto.max(titolo) + 2 * PAD_ORIZZ;
        let massimo = totale / 4;
        let v = minimo.min(massimo);
        if let Some(x) = w.get_mut(i) {
            *x = v;
        }
        fisse += v;
    }
    let resto = (totale - fisse).max(0);
    if peso_testo > 0 {
        for (i, c) in t.colonne.iter().enumerate() {
            if c.allineamento == Allineamento::Sinistra {
                let quota = resto * i64::from(c.peso.max(1)) / peso_testo;
                if let Some(x) = w.get_mut(i) {
                    *x = quota.max(*x).max(2 * PAD_ORIZZ + 400);
                }
            }
        }
    }
    // Se lo spazio non basta, riduzione proporzionale.
    let somma: i64 = w.iter().sum();
    if somma > totale && somma > 0 {
        for x in &mut w {
            *x = *x * totale / somma;
        }
    }
    w
}

/// Righe di testo di una cella, con font e corpo.
struct CellaImpaginata {
    righe: Vec<String>,
    font: Font,
    corpo: i64,
    allineamento: Allineamento,
    x0: i64,
    x1: i64,
}

fn stile_font(stile: StileRiga, numerica: bool) -> (Font, i64) {
    match stile {
        StileRiga::Voce | StileRiga::Sommano | StileRiga::Totale | StileRiga::Gruppo => {
            (Font::Grassetto, CORPO)
        }
        StileRiga::Rigo => (Font::Normale, CORPO_RIGO),
        StileRiga::Commento => (
            if numerica {
                Font::Normale
            } else {
                Font::Corsivo
            },
            CORPO_RIGO,
        ),
        StileRiga::Normale => (Font::Normale, CORPO),
    }
}

fn interlinea(corpo: i64) -> i64 {
    corpo * 12 / 10
}

fn impagina_riga(
    r: &Riga,
    t: &Tabella,
    x: &[i64],
    larg: &[i64],
    destra: i64,
) -> Vec<CellaImpaginata> {
    let mut celle = Vec::new();
    for (i, v) in r.celle.iter().enumerate() {
        if v.is_vuoto() {
            continue;
        }
        let all = t
            .colonne
            .get(i)
            .map_or(Allineamento::Sinistra, |c| c.allineamento);
        let numerica = all != Allineamento::Sinistra;
        let (font, corpo) = stile_font(r.stile, numerica);
        let x0 = x.get(i).copied().unwrap_or(0) + PAD_ORIZZ;
        // Le righe di gruppo e i testi dei totali possono occupare le colonne vuote a destra.
        let estesa = matches!(r.stile, StileRiga::Gruppo)
            || (!numerica
                && matches!(r.stile, StileRiga::Totale | StileRiga::Sommano)
                && r.celle
                    .get(i + 1)
                    .is_some_and(crate::documento::Valore::is_vuoto));
        let x1 = if estesa {
            let mut fine = i + 1;
            while fine < r.celle.len()
                && r.celle
                    .get(fine)
                    .is_some_and(crate::documento::Valore::is_vuoto)
            {
                fine += 1;
            }
            x.get(fine).copied().unwrap_or(destra) - PAD_ORIZZ
        } else {
            x.get(i).copied().unwrap_or(0) + larg.get(i).copied().unwrap_or(0) - PAD_ORIZZ
        };
        let testo = v.in_testo();
        let righe = if numerica {
            vec![testo]
        } else {
            a_capo(&testo, font, corpo, (x1 - x0).max(100))
        };
        celle.push(CellaImpaginata {
            righe,
            font,
            corpo,
            allineamento: all,
            x0,
            x1,
        });
    }
    celle
}

fn disegna_intestazione_tabella(imp: &mut Impaginatore, t: &Tabella, x: &[i64], larg: &[i64]) {
    let righe: Vec<Vec<String>> = t
        .colonne
        .iter()
        .enumerate()
        .map(|(i, c)| {
            a_capo(
                &c.titolo,
                Font::Grassetto,
                CORPO,
                larg.get(i).copied().unwrap_or(0) - 2 * PAD_ORIZZ,
            )
        })
        .collect();
    let n = righe.iter().map(Vec::len).max().unwrap_or(1) as i64;
    let h = n * interlinea(CORPO) + 2 * PAD_VERT;
    let (sx, dx) = (imp.sinistra(), imp.destra());
    let y_alto = imp.y;
    imp.riempi(sx, y_alto - h, dx - sx, h, "0.85");
    for (i, c) in t.colonne.iter().enumerate() {
        let x0 = x.get(i).copied().unwrap_or(0) + PAD_ORIZZ;
        let x1 = x0 + larg.get(i).copied().unwrap_or(0) - 2 * PAD_ORIZZ;
        if let Some(rr) = righe.get(i) {
            for (k, testo) in rr.iter().enumerate() {
                let base = y_alto - PAD_VERT - (k as i64 + 1) * interlinea(CORPO) + CORPO / 5;
                imp.testo_allineato(
                    (x0, x1),
                    base,
                    testo,
                    Font::Grassetto,
                    CORPO,
                    c.allineamento,
                );
            }
        }
    }
    imp.linea(sx, y_alto - h, dx, y_alto - h, 50);
    imp.y = y_alto - h;
}

fn disegna_tabella(imp: &mut Impaginatore, t: &Tabella, titolo_sezione: &str) {
    let (sx, dx) = (imp.sinistra(), imp.destra());
    let larg = larghezze_colonne(t, dx - sx);
    let mut x = Vec::with_capacity(larg.len());
    let mut cursore = sx;
    for w in &larg {
        x.push(cursore);
        cursore += w;
    }
    let altezza_intestazione = 3 * interlinea(CORPO) + 2 * PAD_VERT;
    if imp.disponibile() < altezza_intestazione + 3 * interlinea(CORPO) {
        imp.nuova_pagina();
    }
    disegna_intestazione_tabella(imp, t, &x, &larg);

    for r in &t.righe {
        let celle = impagina_riga(r, t, &x, &larg, dx);
        let corpo_riga = celle.iter().map(|c| c.corpo).max().unwrap_or(CORPO);
        let passo = interlinea(corpo_riga);
        let totale_righe = celle
            .iter()
            .map(|c| c.righe.len())
            .max()
            .unwrap_or(1)
            .max(1);
        let mut fatte = 0usize;
        while fatte < totale_righe {
            let spazio = imp.disponibile() - 2 * PAD_VERT;
            let possibili = usize::try_from((spazio / passo).max(0)).unwrap_or(0);
            if possibili == 0 {
                imp.nuova_pagina();
                imp.testo(
                    sx,
                    imp.y + 300,
                    &format!("{titolo_sezione} (segue)"),
                    Font::Corsivo,
                    CORPO_PICCOLO,
                );
                disegna_intestazione_tabella(imp, t, &x, &larg);
                continue;
            }
            // Una riga breve non viene spezzata: se non sta, va alla pagina successiva.
            let restanti = totale_righe - fatte;
            if fatte == 0 && restanti <= 6 && possibili < restanti {
                imp.nuova_pagina();
                imp.testo(
                    sx,
                    imp.y + 300,
                    &format!("{titolo_sezione} (segue)"),
                    Font::Corsivo,
                    CORPO_PICCOLO,
                );
                disegna_intestazione_tabella(imp, t, &x, &larg);
                continue;
            }
            let quante = restanti.min(possibili);
            let h = i64::try_from(quante).unwrap_or(0) * passo + 2 * PAD_VERT;
            let y_alto = imp.y;
            match r.stile {
                StileRiga::Gruppo => imp.riempi(sx, y_alto - h, dx - sx, h, "0.92"),
                StileRiga::Totale if fatte == 0 => {
                    imp.riempi(sx, y_alto - h, dx - sx, h, "0.92");
                    imp.linea(sx, y_alto, dx, y_alto, 60);
                }
                StileRiga::Sommano if fatte == 0 => {
                    let x_inizio = x.get(2).copied().unwrap_or(sx);
                    imp.linea(x_inizio, y_alto, dx, y_alto, 25);
                }
                _ => {}
            }
            for c in &celle {
                for (k, testo) in c.righe.iter().enumerate().skip(fatte).take(quante) {
                    let indice = i64::try_from(k - fatte).unwrap_or(0);
                    let base = y_alto - PAD_VERT - (indice + 1) * passo + passo / 5;
                    imp.testo_allineato((c.x0, c.x1), base, testo, c.font, c.corpo, c.allineamento);
                }
            }
            imp.y = y_alto - h;
            fatte += quante;
        }
    }
    imp.linea(sx, imp.y, dx, imp.y, 50);
}

fn disegna_sezione(imp: &mut Impaginatore, s: &Sezione, prima: bool) {
    let spazio_titolo = interlinea(CORPO_SEZIONE)
        + 600
        + i64::try_from(s.note.len()).unwrap_or(0) * interlinea(CORPO_PICCOLO);
    let minimo = spazio_titolo + 3 * interlinea(CORPO) + 4 * interlinea(CORPO);
    if !prima && imp.disponibile() < minimo {
        imp.nuova_pagina();
    } else if !prima {
        imp.y -= 900;
    }
    imp.y -= interlinea(CORPO_SEZIONE);
    let (sx, y) = (imp.sinistra(), imp.y);
    imp.testo(sx, y, &s.titolo, Font::Grassetto, CORPO_SEZIONE);
    imp.y -= 300;
    for n in &s.note {
        imp.y -= interlinea(CORPO_PICCOLO);
        let y = imp.y;
        imp.testo(sx, y, n, Font::Corsivo, CORPO_PICCOLO);
    }
    imp.y -= 300;
    disegna_tabella(imp, &s.tabella, &s.titolo);
}

fn disegna_frontespizio(imp: &mut Impaginatore, d: &Documento) {
    imp.nuova_pagina();
    let (sx, dx) = (imp.sinistra(), imp.destra());
    for riga in a_capo(&d.titolo, Font::Grassetto, CORPO_TITOLO, dx - sx) {
        imp.y -= interlinea(CORPO_TITOLO);
        let y = imp.y;
        imp.testo(sx, y, &riga, Font::Grassetto, CORPO_TITOLO);
    }
    imp.y -= 400;
    let colonna_valori = sx + 9_000;
    for (k, v) in &d.intestazione {
        let righe = a_capo(v, Font::Normale, CORPO_INTESTAZIONE, dx - colonna_valori);
        imp.y -= interlinea(CORPO_INTESTAZIONE);
        let y = imp.y;
        imp.testo(sx, y, k, Font::Grassetto, CORPO_INTESTAZIONE);
        for (i, r) in righe.iter().enumerate() {
            if i > 0 {
                imp.y -= interlinea(CORPO_INTESTAZIONE);
            }
            let y = imp.y;
            imp.testo(colonna_valori, y, r, Font::Normale, CORPO_INTESTAZIONE);
        }
    }
    imp.y -= 500;
    let y = imp.y;
    imp.linea(sx, y, dx, y, 50);
}

/// Testata e piè di pagina, aggiunti alla fine quando il numero di pagine è noto.
fn testate(imp: &mut Impaginatore) {
    let totale = imp.pagine.len();
    let (sx, dx) = (imp.sinistra(), imp.destra());
    let alto = imp.altezza - MARGINE;
    let titolo = imp.titolo.clone();
    let oggetto = imp.oggetto.clone();
    let piede = imp.piede.clone();
    let max_oggetto = (dx - sx) / 2;
    let oggetto = a_capo(&oggetto, Font::Normale, CORPO_PICCOLO, max_oggetto)
        .into_iter()
        .next()
        .unwrap_or_default();
    for n in 0..totale {
        let mut ops = String::new();
        let mut sostituiti = 0usize;
        let mut testo = |x: i64, y: i64, t: &str, font: Font| {
            let byte = codifica(t, &mut sostituiti);
            let _ = writeln!(
                ops,
                "BT /{} {} Tf {} {} Td {} Tj ET",
                font.nome_risorsa(),
                pt(CORPO_PICCOLO),
                pt(x),
                pt(y),
                stringa_pdf(&byte)
            );
        };
        let y_testata = alto - CORPO_PICCOLO;
        testo(sx, y_testata, &titolo, Font::Grassetto);
        let w = larghezza(&oggetto, Font::Normale, CORPO_PICCOLO);
        testo(dx - w, y_testata, &oggetto, Font::Normale);
        let y_piede = MARGINE - CORPO_PICCOLO;
        testo(sx, y_piede, &piede, Font::Normale);
        let pagina = format!("Pagina {} di {}", n + 1, totale);
        let w = larghezza(&pagina, Font::Normale, CORPO_PICCOLO);
        testo(dx - w, y_piede, &pagina, Font::Normale);
        let _ = writeln!(
            ops,
            "{} w {} {} m {} {} l S",
            pt(25),
            pt(sx),
            pt(alto - CORPO_PICCOLO - 300),
            pt(dx),
            pt(alto - CORPO_PICCOLO - 300)
        );
        let _ = writeln!(
            ops,
            "{} w {} {} m {} {} l S",
            pt(25),
            pt(sx),
            pt(MARGINE + 500),
            pt(dx),
            pt(MARGINE + 500)
        );
        imp.sostituiti += sostituiti;
        if let Some(p) = imp.pagine.get_mut(n) {
            p.contenuto.insert_str(0, &ops);
        }
    }
}

pub(crate) fn stringa_utf16(testo: &str) -> String {
    let mut s = String::from("<FEFF");
    for u in testo.encode_utf16() {
        let _ = write!(s, "{u:04X}");
    }
    s.push('>');
    s
}

pub(crate) fn comprimi(dati: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    if e.write_all(dati).is_err() {
        return Vec::new();
    }
    e.finish().unwrap_or_default()
}

/// Assembla il file PDF: catalogo, albero delle pagine, font standard (risorse `F1`,
/// `F2`, … nell'ordine indicato, codifica WinAnsi), informazioni e una pagina per
/// contenuto. Uscita deterministica.
pub(crate) fn assembla(
    contenuti: &[&str],
    (larghezza, altezza): (i64, i64),
    font: &[&str],
    titolo: &str,
    comprimi_contenuti: bool,
) -> Vec<u8> {
    // Oggetti: 1 catalogo, 2 albero delle pagine, poi i font, le informazioni e, per
    // ogni pagina, pagina e contenuto.
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut posizioni: Vec<usize> = Vec::new();
    let mut oggetto = |out: &mut Vec<u8>, corpo: &[u8]| {
        posizioni.push(out.len());
        let n = posizioni.len();
        out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
        out.extend_from_slice(corpo);
        out.extend_from_slice(b"\nendobj\n");
    };
    let pagine = contenuti.len();
    let info = 3 + font.len();
    let primo = info + 1;
    let figli: Vec<String> = (0..pagine)
        .map(|i| format!("{} 0 R", primo + 2 * i))
        .collect();
    oggetto(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    oggetto(
        &mut out,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} /MediaBox [0 0 {} {}] >>",
            figli.join(" "),
            pagine,
            pt(larghezza),
            pt(altezza)
        )
        .as_bytes(),
    );
    for nome in font {
        oggetto(
            &mut out,
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{nome} /Encoding /WinAnsiEncoding >>"
            )
            .as_bytes(),
        );
    }
    oggetto(
        &mut out,
        format!(
            "<< /Title {} /Producer (OpenComputo) /Creator (OpenComputo) >>",
            stringa_utf16(titolo)
        )
        .as_bytes(),
    );
    let risorse: Vec<String> = (0..font.len())
        .map(|i| format!("/F{} {} 0 R", i + 1, i + 3))
        .collect();
    let risorse = risorse.join(" ");
    for (i, p) in contenuti.iter().enumerate() {
        let contenuto = primo + 2 * i + 1;
        oggetto(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /Resources << /Font << {risorse} >> >> /Contents {contenuto} 0 R >>"
            )
            .as_bytes(),
        );
        let (dati, filtro) = if comprimi_contenuti {
            (comprimi(p.as_bytes()), " /Filter /FlateDecode")
        } else {
            (p.as_bytes().to_vec(), "")
        };
        let mut corpo = format!("<< /Length {}{filtro} >>\nstream\n", dati.len()).into_bytes();
        corpo.extend_from_slice(&dati);
        corpo.extend_from_slice(b"\nendstream");
        oggetto(&mut out, &corpo);
    }
    let xref = out.len();
    let n = posizioni.len() + 1;
    let mut tabella = format!("xref\n0 {n}\n0000000000 65535 f \n");
    for p in &posizioni {
        let _ = writeln!(tabella, "{p:010} 00000 n ");
    }
    let _ = write!(
        tabella,
        "trailer\n<< /Size {n} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    );
    out.extend_from_slice(tabella.as_bytes());
    out
}

/// Scrive il documento in PDF.
#[must_use]
pub fn documento(d: &Documento, o: &OpzioniPdf) -> RisultatoPdf {
    let (larghezza, altezza) = if d.orizzontale {
        (A4_LATO_LUNGO, A4_LATO_CORTO)
    } else {
        (A4_LATO_CORTO, A4_LATO_LUNGO)
    };
    let oggetto = d
        .intestazione
        .iter()
        .find(|(k, _)| k == "Oggetto")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    let mut imp = Impaginatore {
        larghezza,
        altezza,
        pagine: Vec::new(),
        y: 0,
        sostituiti: 0,
        titolo: d.titolo.clone(),
        oggetto,
        piede: d.piede.clone().map_or_else(
            || "OpenComputo".to_owned(),
            |p| format!("OpenComputo — {p}"),
        ),
    };
    disegna_frontespizio(&mut imp, d);
    for (i, s) in d.sezioni.iter().enumerate() {
        disegna_sezione(&mut imp, s, i == 0);
    }
    testate(&mut imp);

    let contenuti: Vec<&str> = imp.pagine.iter().map(|p| p.contenuto.as_str()).collect();
    let out = assembla(
        &contenuti,
        (larghezza, altezza),
        &["Helvetica", "Helvetica-Bold", "Helvetica-Oblique"],
        &d.titolo,
        o.comprimi,
    );
    let pagine = contenuti.len();
    RisultatoPdf {
        byte: out,
        pagine,
        caratteri_sostituiti: imp.sostituiti,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codifica_winansi() {
        let mut s = 0;
        assert_eq!(
            codifica("àèéìòù €", &mut s),
            vec![0xE0, 0xE8, 0xE9, 0xEC, 0xF2, 0xF9, b' ', 0x80]
        );
        assert_eq!(
            codifica("m³ – “x”", &mut s),
            vec![b'm', 0xB3, b' ', 0x96, b' ', 0x93, b'x', 0x94]
        );
        assert_eq!(s, 0);
        assert_eq!(
            codifica("λ ≤ 0,035 μm", &mut s),
            b"lambda  <= 0,035 \xB5m".to_vec()
        );
        assert_eq!(s, 0);
        assert_eq!(codifica("ж", &mut s), vec![b'?']);
        assert_eq!(s, 1);
        assert_eq!(stringa_pdf(b"a(b)\\c\xE0"), "(a\\(b\\)\\\\c\\340)");
    }

    #[test]
    fn misure_e_a_capo() {
        assert_eq!(pt(1234), "12.34");
        assert_eq!(pt(1200), "12");
        assert_eq!(pt(-5), "-0.05");
        // «Hello» in Helvetica 10 pt: 722+556+222+222+556 = 2278 millesimi → 22,78 pt.
        assert_eq!(larghezza("Hello", Font::Normale, 1000), 2278);
        let righe = a_capo("uno due tre quattro cinque", Font::Normale, 1000, 5000);
        assert!(righe.len() > 1);
        assert!(
            righe
                .iter()
                .all(|r| larghezza(r, Font::Normale, 1000) <= 5000)
        );
        assert_eq!(righe.join(" "), "uno due tre quattro cinque");
        let lunga = a_capo(&"x".repeat(100), Font::Normale, 1000, 3000);
        assert!(lunga.len() > 1);
        assert_eq!(lunga.concat(), "x".repeat(100));
        assert_eq!(a_capo("", Font::Normale, 1000, 3000), vec![String::new()]);
        assert_eq!(a_capo("a\nb", Font::Normale, 1000, 3000), vec!["a", "b"]);
    }
}
