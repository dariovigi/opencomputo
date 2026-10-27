//! Scrittura XLSX dei report: un foglio per sezione.
//!
//! I numeri sono scritti come **valori** (non formule) già calcolati dal core, con un
//! formato numerico dei decimali corrispondenti: il foglio riproduce esattamente il
//! report e non dipende dagli arrotondamenti di Excel. La conversione da decimale a
//! numero in virgola mobile è richiesta dal formato XLSX e conserva le 15 cifre
//! significative visualizzate da Excel.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{
    Color, DocProperties, ExcelDateTime, Format, FormatAlign, FormatBorder, Workbook, Worksheet,
    XlsxError,
};
use thiserror::Error;

use crate::documento::{Allineamento, Documento, StileRiga, Valore};

/// Errore di scrittura XLSX.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreXlsx {
    /// Errore della libreria di scrittura.
    #[error("errore nella scrittura XLSX: {0}")]
    Scrittura(String),
    /// Numero non rappresentabile.
    #[error("numero non rappresentabile in XLSX: {0}")]
    Numero(String),
    /// Troppe righe o colonne per un foglio.
    #[error("tabella oltre i limiti di un foglio XLSX")]
    OltreLimiti,
}

impl From<XlsxError> for ErroreXlsx {
    fn from(e: XlsxError) -> Self {
        Self::Scrittura(e.to_string())
    }
}

/// Opzioni di scrittura.
#[derive(Debug, Clone, Default)]
pub struct OpzioniXlsx {
    /// Data registrata nelle proprietà del file. Senza data viene usato il
    /// 1° gennaio 2000, così il file è riproducibile e non legge l'orologio.
    pub data: Option<NaiveDate>,
}

/// Larghezza totale indicativa di una tabella, in caratteri.
const LARGHEZZA_TABELLA: u32 = 150;
const GRIGIO_CHIARO: u32 = 0x00E7_E6E6;
const GRIGIO: u32 = 0x00D9_D9D9;

fn nome_foglio(titolo: &str, usati: &mut Vec<String>) -> String {
    let pulito: String = titolo
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { '-' } else { c })
        .collect();
    let base: String = pulito.trim().trim_matches('\'').chars().take(28).collect();
    let base = if base.is_empty() {
        "Foglio".to_owned()
    } else {
        base
    };
    let mut nome = base.clone();
    let mut n = 2;
    while usati.iter().any(|u| u.eq_ignore_ascii_case(&nome)) {
        nome = format!("{base} {n}");
        n += 1;
    }
    usati.push(nome.clone());
    nome
}

/// Formati per stile di riga, allineamento e decimali.
#[derive(Default)]
struct Formati {
    cache: HashMap<(u8, u8, Option<u32>, bool), Format>,
}

fn indice_stile(s: StileRiga) -> u8 {
    match s {
        StileRiga::Normale => 0,
        StileRiga::Voce => 1,
        StileRiga::Rigo => 2,
        StileRiga::Commento => 3,
        StileRiga::Sommano => 4,
        StileRiga::Gruppo => 5,
        StileRiga::Totale => 6,
    }
}

impl Formati {
    fn per(
        &mut self,
        stile: StileRiga,
        all: Allineamento,
        decimali: Option<u32>,
        a_capo: bool,
    ) -> &Format {
        let a = match all {
            Allineamento::Sinistra => 0,
            Allineamento::Centro => 1,
            Allineamento::Destra => 2,
        };
        self.cache
            .entry((indice_stile(stile), a, decimali, a_capo))
            .or_insert_with(|| {
                let mut f = Format::new()
                    .set_font_name("Arial")
                    .set_font_size(9)
                    .set_align(FormatAlign::Top);
                f = match all {
                    Allineamento::Sinistra => f.set_align(FormatAlign::Left),
                    Allineamento::Centro => f.set_align(FormatAlign::Center),
                    Allineamento::Destra => f.set_align(FormatAlign::Right),
                };
                if a_capo {
                    f = f.set_text_wrap();
                }
                if let Some(d) = decimali {
                    let formato = if d == 0 {
                        "#,##0".to_owned()
                    } else {
                        format!("#,##0.{}", "0".repeat(d as usize))
                    };
                    f = f.set_num_format(formato);
                }
                match stile {
                    StileRiga::Normale => f,
                    StileRiga::Voce => f.set_bold(),
                    StileRiga::Rigo => f.set_font_size(8),
                    StileRiga::Commento => f.set_font_size(8).set_italic(),
                    StileRiga::Sommano => f.set_bold().set_border_top(FormatBorder::Hair),
                    StileRiga::Gruppo => {
                        f.set_bold().set_background_color(Color::RGB(GRIGIO_CHIARO))
                    }
                    StileRiga::Totale => f
                        .set_bold()
                        .set_border_top(FormatBorder::Thin)
                        .set_background_color(Color::RGB(GRIGIO_CHIARO)),
                }
            })
    }
}

fn colonna_u16(c: usize) -> Result<u16, ErroreXlsx> {
    u16::try_from(c).map_err(|_| ErroreXlsx::OltreLimiti)
}

fn scrivi_sezione(
    ws: &mut Worksheet,
    doc: &Documento,
    sezione: &crate::documento::Sezione,
    formati: &mut Formati,
) -> Result<(), ErroreXlsx> {
    let t = &sezione.tabella;
    let ncol = t.colonne.len().max(1);
    let ultima = colonna_u16(ncol - 1)?;
    let titolo = Format::new()
        .set_font_name("Arial")
        .set_font_size(13)
        .set_bold();
    let etichetta = Format::new()
        .set_font_name("Arial")
        .set_font_size(9)
        .set_bold();
    let testo = Format::new().set_font_name("Arial").set_font_size(9);
    let nota = Format::new()
        .set_font_name("Arial")
        .set_font_size(8)
        .set_italic();
    let mut r: u32 = 0;
    ws.write_string_with_format(r, 0, &doc.titolo, &titolo)?;
    r += 1;
    // Etichette sulle prime due colonne (la prima è spesso stretta), valori sul resto.
    for (k, v) in &doc.intestazione {
        if ultima >= 3 {
            ws.merge_range(r, 0, r, 1, k, &etichetta)?;
            ws.merge_range(r, 2, r, ultima, v, &testo)?;
        } else {
            ws.write_string_with_format(r, 0, k, &etichetta)?;
            ws.write_string_with_format(r, 1, v, &testo)?;
        }
        r += 1;
    }
    r += 1;
    ws.write_string_with_format(r, 0, &sezione.titolo, &etichetta.clone().set_font_size(11))?;
    r += 1;
    for n in &sezione.note {
        ws.write_string_with_format(r, 0, n, &nota)?;
        r += 1;
    }

    // Intestazione della tabella.
    let riga_intestazione = r;
    for (i, c) in t.colonne.iter().enumerate() {
        let f = Format::new()
            .set_font_name("Arial")
            .set_font_size(9)
            .set_bold()
            .set_text_wrap()
            .set_background_color(Color::RGB(GRIGIO))
            .set_border_bottom(FormatBorder::Thin)
            .set_align(match c.allineamento {
                Allineamento::Sinistra => FormatAlign::Left,
                Allineamento::Centro => FormatAlign::Center,
                Allineamento::Destra => FormatAlign::Right,
            });
        ws.write_string_with_format(r, colonna_u16(i)?, &c.titolo, &f)?;
    }
    r += 1;

    // Larghezze proporzionali ai pesi; le colonne di testo larghe vanno a capo.
    let somma: u32 = t
        .colonne
        .iter()
        .map(|c| u32::from(c.peso))
        .sum::<u32>()
        .max(1);
    let mut a_capo = vec![false; ncol];
    for (i, c) in t.colonne.iter().enumerate() {
        let caratteri = (u32::from(c.peso) * LARGHEZZA_TABELLA / somma).max(4);
        if let Some(a) = a_capo.get_mut(i) {
            *a = c.allineamento == Allineamento::Sinistra && caratteri >= 20;
        }
        ws.set_column_width(colonna_u16(i)?, f64::from(caratteri))?;
    }

    for riga in &t.righe {
        for (i, v) in riga.celle.iter().enumerate() {
            let col = colonna_u16(i)?;
            let all = t
                .colonne
                .get(i)
                .map_or(Allineamento::Sinistra, |c| c.allineamento);
            let wrap = a_capo.get(i).copied().unwrap_or(false);
            match v {
                Valore::Vuoto => {
                    let f = formati.per(riga.stile, all, None, false);
                    ws.write_blank(r, col, f)?;
                }
                Valore::Testo(s) => {
                    let f = formati.per(riga.stile, all, None, wrap);
                    ws.write_string_with_format(r, col, s, f)?;
                }
                Valore::Numero { valore, decimali } => {
                    let d = (*decimali).max(valore.scale());
                    let n = valore
                        .to_f64()
                        .ok_or_else(|| ErroreXlsx::Numero(valore.to_string()))?;
                    let f = formati.per(riga.stile, all, Some(d), false);
                    ws.write_number_with_format(r, col, n, f)?;
                }
            }
        }
        r = r.checked_add(1).ok_or(ErroreXlsx::OltreLimiti)?;
    }

    // Stampa: A4, adattata in larghezza, intestazione ripetuta, numeri di pagina.
    ws.set_paper_size(9);
    if doc.orizzontale {
        ws.set_landscape();
    }
    ws.set_print_fit_to_pages(1, 0);
    ws.set_repeat_rows(riga_intestazione, riga_intestazione)?;
    ws.set_freeze_panes(riga_intestazione + 1, 0)?;
    ws.set_header(format!("&L&8{}", doc.titolo.replace('&', "&&")));
    let piede = doc.piede.as_deref().unwrap_or("").replace('&', "&&");
    ws.set_footer(format!("&L&8{piede}&R&8Pagina &P di &N"));
    Ok(())
}

/// Scrive il documento in un file XLSX (byte).
///
/// # Errors
/// Errori della libreria, numeri non rappresentabili, limiti del foglio.
pub fn documento(doc: &Documento, o: &OpzioniXlsx) -> Result<Vec<u8>, ErroreXlsx> {
    let mut wb = Workbook::new();
    let data = o
        .data
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap_or_default());
    let creazione = ExcelDateTime::from_ymd(
        u16::try_from(data.year()).map_err(|_| ErroreXlsx::OltreLimiti)?,
        u8::try_from(data.month()).map_err(|_| ErroreXlsx::OltreLimiti)?,
        u8::try_from(data.day()).map_err(|_| ErroreXlsx::OltreLimiti)?,
    )?;
    wb.set_properties(
        &DocProperties::new()
            .set_title(doc.titolo.as_str())
            .set_comment("Generato da OpenComputo")
            .set_creation_datetime(&creazione),
    );
    let mut usati = Vec::new();
    let mut formati = Formati::default();
    for s in &doc.sezioni {
        let ws = wb.add_worksheet();
        ws.set_name(nome_foglio(&s.titolo, &mut usati))?;
        scrivi_sezione(ws, doc, s, &mut formati)?;
    }
    if doc.sezioni.is_empty() {
        wb.add_worksheet().set_name("Vuoto")?;
    }
    Ok(wb.save_to_buffer()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomi_dei_fogli_validi_e_unici() {
        let mut usati = Vec::new();
        assert_eq!(
            nome_foglio("Riepilogo [WBS]: 1/2", &mut usati),
            "Riepilogo -WBS-- 1-2"
        );
        assert_eq!(
            nome_foglio("riepilogo -wbs-- 1-2", &mut usati),
            "riepilogo -wbs-- 1-2 2"
        );
        assert_eq!(nome_foglio("", &mut usati), "Foglio");
        assert!(nome_foglio(&"x".repeat(40), &mut usati).chars().count() <= 31);
    }
}
