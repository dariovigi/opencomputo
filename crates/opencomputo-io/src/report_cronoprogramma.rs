//! Stampe del cronoprogramma (ADR 0024): tabella delle attività e produzione mensile
//! (documento ordinario, in PDF, XLSX e CSV) e diagramma di Gantt (PDF A3 orizzontale).
//!
//! Date, durate, margini, legami e importi sono quelli del piano calcolato dal core
//! (ADR 0007): il diagramma li disegna senza ricalcolarli.

use std::fmt::Write as _;

use chrono::{Datelike, Duration, NaiveDate};
use opencomputo_core::cronoprogramma::{self, Piano, RigaPiano, TipoLegame};
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;

use crate::documento::{Colonna, Documento, Sezione, StileRiga, Tabella, Valore};
use crate::formato;
use crate::metriche;
use crate::pdf::{self, RisultatoPdf};
use crate::report::{ErroreReport, OpzioniReport, documento};

const MESI: [&str; 12] = [
    "gennaio",
    "febbraio",
    "marzo",
    "aprile",
    "maggio",
    "giugno",
    "luglio",
    "agosto",
    "settembre",
    "ottobre",
    "novembre",
    "dicembre",
];

fn nome_mese(mese: u32) -> &'static str {
    MESI.get(usize::try_from(mese.saturating_sub(1)).unwrap_or(0))
        .copied()
        .unwrap_or("")
}

/// Testo dei predecessori di una riga: «2;3II+2» (numeri di riga da 1).
#[must_use]
pub fn predecessori(r: &RigaPiano) -> String {
    r.predecessori
        .iter()
        .map(|p| {
            let tipo = if p.tipo == TipoLegame::FineInizio && p.ritardo == 0 {
                ""
            } else {
                p.tipo.sigla()
            };
            let ritardo = match p.ritardo {
                0 => String::new(),
                x if x > 0 => format!("+{x}"),
                x => x.to_string(),
            };
            format!("{}{tipo}{ritardo}", p.riga + 1)
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn piano(p: &Progetto) -> Result<Piano, ErroreReport> {
    cronoprogramma::calcola(p, None).map_err(|e| ErroreReport::Calcolo(e.to_string()))
}

/// Cronoprogramma come documento: attività e produzione mensile.
///
/// # Errors
/// Piano non calcolabile (inizio mancante, ciclo…).
pub fn cronoprogramma(p: &Progetto, o: &OpzioniReport) -> Result<Documento, ErroreReport> {
    let piano = piano(p)?;
    let di = p.regole.decimali_importo;
    let mut doc = documento(p, "Cronoprogramma dei lavori", true, o);

    let mut note = vec![format!(
        "Inizio {} — fine {} — {} giorni lavorativi ({} naturali e consecutivi).",
        formato::data(piano.inizio),
        formato::data(piano.fine),
        piano.durata,
        piano.durata_naturale
    )];
    if let Some(s) = piano.scadenza_contrattuale {
        note.push(if piano.fine > s {
            format!(
                "Ultimazione contrattuale {}: il piano la supera.",
                formato::data(s)
            )
        } else {
            format!("Ultimazione contrattuale {}.", formato::data(s))
        });
    }
    note.push(match piano.livello_dettaglio {
        Some(l) => format!("Dettaglio della WBS fino al livello {l}."),
        None => "Dettaglio della WBS: tutti i livelli.".into(),
    });
    if let Some(b) = &p.cronoprogramma.linea_base {
        note.push(format!(
            "Scostamenti dalla linea di base del {}.",
            formato::data(b.salvata_il)
        ));
    }
    let con_base = p.cronoprogramma.linea_base.is_some();
    let mut colonne = vec![
        Colonna::centrata("N.", 4),
        Colonna::testo("WBS", 7),
        Colonna::testo("Attività", 40),
        Colonna::numero("Durata gg", 7),
        Colonna::centrata("Inizio", 9),
        Colonna::centrata("Fine", 9),
        Colonna::testo("Predecessori", 10),
        Colonna::numero("Margine gg", 7),
        Colonna::centrata("Critica", 6),
        Colonna::centrata("Tipo", 7),
        Colonna::numero("Avanz. %", 6),
        Colonna::numero("Importo €", 11),
    ];
    if con_base {
        colonne.push(Colonna::numero("Scost. gg", 6));
    }
    let mut t = Tabella::new(colonne);
    for (i, r) in piano.righe.iter().enumerate() {
        let rientro = "  ".repeat(usize::try_from(r.livello.saturating_sub(1)).unwrap_or(0));
        let nome = format!("{rientro}{}", r.nome);
        let durata = if r.durata_stimata {
            Valore::testo(format!("{}?", r.durata))
        } else {
            Valore::numero(Decimal::from(r.durata), 0)
        };
        let mut celle = vec![
            Valore::testo((i + 1).to_string()),
            Valore::testo(r.codice.clone()),
            Valore::testo(nome),
            durata,
            Valore::testo(formato::data(r.inizio)),
            Valore::testo(formato::data(r.fine)),
            Valore::testo(predecessori(r)),
            Valore::numero(Decimal::from(r.margine), 0),
            Valore::testo(if r.critica { "sì" } else { "" }),
            Valore::testo(if r.pietra_miliare {
                "p. miliare"
            } else if r.riepilogo {
                "riepilogo"
            } else {
                ""
            }),
            Valore::numero(Decimal::from(r.avanzamento), 0),
            Valore::numero_opz(r.importo.map(|x| x.valore()), di),
        ];
        if con_base {
            celle.push(Valore::numero_opz(r.scostamento.map(Decimal::from), 0));
        }
        t.riga(
            if r.riepilogo {
                StileRiga::Voce
            } else {
                StileRiga::Normale
            },
            celle,
        );
    }
    doc.sezioni.push(Sezione {
        titolo: "Attività".into(),
        note,
        tabella: t,
    });

    if !piano.mesi.is_empty() {
        let mut t = Tabella::new(vec![
            Colonna::testo("Mese", 20),
            Colonna::numero("Importo del mese €", 14),
            Colonna::numero("Cumulato €", 14),
            Colonna::numero("Cumulato %", 8),
        ]);
        for m in &piano.mesi {
            t.riga(
                StileRiga::Normale,
                vec![
                    Valore::testo(format!("{} {}", nome_mese(m.mese), m.anno)),
                    Valore::numero(m.importo.valore(), di),
                    Valore::numero(m.cumulato.valore(), di),
                    Valore::numero(m.percentuale, 2),
                ],
            );
        }
        t.riga(
            StileRiga::Totale,
            vec![
                Valore::testo("TOTALE"),
                Valore::numero(piano.importo_pianificato.valore(), di),
            ],
        );
        let mut note = vec![format!(
            "Importi del computo «{}» ripartiti in modo uniforme sui giorni lavorativi delle attività.",
            piano.computo.clone().unwrap_or_default()
        )];
        if !piano.importo_non_assegnato.is_zero() {
            note.push(format!(
                "Voci senza nodo WBS, non distribuite nel tempo: {} €.",
                formato::numero(piano.importo_non_assegnato.valore(), di)
            ));
        }
        doc.sezioni.push(Sezione {
            titolo: "Produzione mensile".into(),
            note,
            tabella: t,
        });
    }
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Diagramma di Gantt.

/// A3 orizzontale, in centesimi di punto.
const LARGHEZZA: i64 = 119_055;
const ALTEZZA: i64 = 84_189;
const MARGINE: i64 = 1_500;
const RIGA: i64 = 1_300;
const CORPO: i64 = 700;
const TESTATA_TEMPO: i64 = 2_400;
/// Colonne della tabella a sinistra: N., WBS, attività, durata, inizio, fine.
const COLONNE: [(i64, &str); 6] = [
    (1_800, "N."),
    (4_400, "WBS"),
    (19_000, "Attività"),
    (3_400, "Durata"),
    (4_600, "Inizio"),
    (4_600, "Fine"),
];

#[derive(Default)]
struct Pagina {
    ops: String,
    sostituiti: usize,
}

/// Colori RGB.
const NERO: &str = "0 0 0";
const GRIGIO: &str = "0.55 0.55 0.55";
const GRIGIO_CHIARO: &str = "0.93 0.93 0.93";
const LINEA: &str = "0.82 0.82 0.82";
const BLU: &str = "0.231 0.490 0.847";
const BLU_SCURO: &str = "0.12 0.30 0.58";
const ROSSO: &str = "0.776 0.157 0.157";
const ROSSO_SCURO: &str = "0.50 0.08 0.08";

fn larghezza_testo(t: &str, grassetto: bool, corpo: i64) -> i64 {
    let mut s = 0;
    let w = if grassetto {
        &metriche::HELVETICA_BOLD
    } else {
        &metriche::HELVETICA
    };
    pdf::codifica(t, &mut s)
        .iter()
        .map(|b| i64::from(w.get(usize::from(*b)).copied().unwrap_or(500)))
        .sum::<i64>()
        * corpo
        / 1000
}

/// Testo ridotto con «…» alla larghezza indicata.
fn adatta(t: &str, grassetto: bool, corpo: i64, massimo: i64) -> String {
    if larghezza_testo(t, grassetto, corpo) <= massimo {
        return t.to_owned();
    }
    let mut s: String = t.to_owned();
    while !s.is_empty() && larghezza_testo(&format!("{s}…"), grassetto, corpo) > massimo {
        s.pop();
    }
    format!("{}…", s.trim_end())
}

impl Pagina {
    /// Testo con la linea di base a `y` dall'alto.
    fn testo(&mut self, x: i64, y: i64, t: &str, grassetto: bool, corpo: i64, colore: &str) {
        if t.is_empty() {
            return;
        }
        let byte = pdf::codifica(t, &mut self.sostituiti);
        let _ = writeln!(
            self.ops,
            "BT /{} {} Tf {colore} rg {} {} Td {} Tj ET",
            if grassetto { "F2" } else { "F1" },
            pdf::pt(corpo),
            pdf::pt(x),
            pdf::pt(ALTEZZA - y),
            pdf::stringa_pdf(&byte)
        );
    }

    fn testo_destra(&mut self, x1: i64, y: i64, t: &str, grassetto: bool, corpo: i64) {
        let w = larghezza_testo(t, grassetto, corpo);
        self.testo(x1 - w, y, t, grassetto, corpo, NERO);
    }

    fn rettangolo(&mut self, x: i64, y: i64, w: i64, h: i64, colore: &str) {
        let _ = writeln!(
            self.ops,
            "{colore} rg {} {} {} {} re f",
            pdf::pt(x),
            pdf::pt(ALTEZZA - y - h),
            pdf::pt(w),
            pdf::pt(h)
        );
    }

    fn linea(&mut self, punti: &[(i64, i64)], spessore: i64, colore: &str) {
        let Some((primo, resto)) = punti.split_first() else {
            return;
        };
        let mut s = format!(
            "{colore} RG {} w {} {} m",
            pdf::pt(spessore),
            pdf::pt(primo.0),
            pdf::pt(ALTEZZA - primo.1)
        );
        for (x, y) in resto {
            let _ = write!(s, " {} {} l", pdf::pt(*x), pdf::pt(ALTEZZA - y));
        }
        let _ = writeln!(self.ops, "{s} S");
    }

    fn poligono(&mut self, punti: &[(i64, i64)], colore: &str) {
        let Some((primo, resto)) = punti.split_first() else {
            return;
        };
        let mut s = format!(
            "{colore} rg {} {} m",
            pdf::pt(primo.0),
            pdf::pt(ALTEZZA - primo.1)
        );
        for (x, y) in resto {
            let _ = write!(s, " {} {} l", pdf::pt(*x), pdf::pt(ALTEZZA - y));
        }
        let _ = writeln!(self.ops, "{s} h f");
    }
}

/// Opzioni del diagramma di Gantt.
#[derive(Debug, Clone, Default)]
pub struct OpzioniGantt {
    /// Data di stampa.
    pub data_stampa: Option<NaiveDate>,
    /// Evidenzia il percorso critico (barre rosse).
    pub percorso_critico: bool,
    /// Disegna la linea di base sotto le barre.
    pub linea_base: bool,
    /// Comprime i contenuti delle pagine.
    pub comprimi: bool,
}

/// Diagramma di Gantt del cronoprogramma in PDF (A3 orizzontale, più pagine se le
/// attività non entrano in una).
///
/// # Errors
/// Piano non calcolabile.
#[allow(clippy::too_many_lines)]
pub fn gantt(p: &Progetto, o: &OpzioniGantt) -> Result<RisultatoPdf, ErroreReport> {
    let piano = piano(p)?;
    let x_tabella_fine = MARGINE + COLONNE.iter().map(|(w, _)| *w).sum::<i64>();
    let x_grafico = x_tabella_fine;
    let x_fine = LARGHEZZA - MARGINE;
    let y_testata = MARGINE + 3_600;
    let y_righe = y_testata + TESTATA_TEMPO;
    let y_fondo = ALTEZZA - MARGINE - 2_400;
    let per_pagina = usize::try_from(((y_fondo - y_righe) / RIGA).max(1)).unwrap_or(1);

    // Asse dei tempi: dal primo giorno del mese d'inizio all'ultimo del mese di fine.
    let base_fine = piano
        .righe
        .iter()
        .filter_map(|r| r.fine_base)
        .max()
        .unwrap_or(piano.fine)
        .max(piano.fine);
    let base_inizio = piano
        .righe
        .iter()
        .filter_map(|r| r.inizio_base)
        .min()
        .unwrap_or(piano.inizio)
        .min(piano.inizio);
    let origine =
        NaiveDate::from_ymd_opt(base_inizio.year(), base_inizio.month(), 1).unwrap_or(base_inizio);
    let (a, m) = if base_fine.month() == 12 {
        (base_fine.year() + 1, 1)
    } else {
        (base_fine.year(), base_fine.month() + 1)
    };
    let termine = NaiveDate::from_ymd_opt(a, m, 1).unwrap_or(base_fine);
    let giorni = (termine - origine).num_days().max(1);
    let x_di = |d: NaiveDate| x_grafico + (d - origine).num_days() * (x_fine - x_grafico) / giorni;
    let x_fine_di = |d: NaiveDate| x_di(d + Duration::days(1));
    let passo_giorno = (x_fine - x_grafico) / giorni;

    let righe = &piano.righe;
    let pagine_n = righe.len().div_ceil(per_pagina).max(1);
    let mut pagine: Vec<Pagina> = Vec::with_capacity(pagine_n);
    let titolo_progetto = p
        .metadati
        .oggetto
        .clone()
        .filter(|x| !x.trim().is_empty())
        .unwrap_or_else(|| p.metadati.titolo.clone());
    for np in 0..pagine_n {
        let mut pg = Pagina::default();
        let da = np * per_pagina;
        let a = (da + per_pagina).min(righe.len());
        let y_riga = |i: usize| y_righe + i64::try_from(i - da).unwrap_or(0) * RIGA;

        // Intestazione della pagina.
        pg.testo(
            MARGINE,
            MARGINE + 1_200,
            "CRONOPROGRAMMA DEI LAVORI",
            true,
            1_400,
            NERO,
        );
        pg.testo(
            MARGINE,
            MARGINE + 2_500,
            &adatta(&titolo_progetto, false, 900, 60_000),
            false,
            900,
            NERO,
        );
        let mut sintesi = format!(
            "Inizio {}  ·  Fine {}  ·  {} giorni lavorativi ({} naturali)",
            formato::data(piano.inizio),
            formato::data(piano.fine),
            piano.durata,
            piano.durata_naturale
        );
        if let Some(s) = piano.scadenza_contrattuale {
            let _ = write!(
                sintesi,
                "  ·  Ultimazione contrattuale {}",
                formato::data(s)
            );
        }
        pg.testo_destra(x_fine, MARGINE + 1_200, &sintesi, false, 800);

        // Testata della tabella e dell'asse dei tempi.
        pg.rettangolo(
            MARGINE,
            y_testata,
            x_fine - MARGINE,
            TESTATA_TEMPO,
            GRIGIO_CHIARO,
        );
        let mut x = MARGINE;
        for (w, t) in COLONNE {
            pg.testo(x + 150, y_testata + 2_050, t, true, CORPO, NERO);
            x += w;
        }
        // Mesi (riga alta) e giorni o settimane (riga bassa).
        let mut d = origine;
        while d < termine {
            let (a2, m2) = if d.month() == 12 {
                (d.year() + 1, 1)
            } else {
                (d.year(), d.month() + 1)
            };
            let fine_mese = NaiveDate::from_ymd_opt(a2, m2, 1).unwrap_or(termine);
            let x0 = x_di(d);
            let x1 = x_di(fine_mese);
            pg.linea(&[(x0, y_testata), (x0, y_fondo)], 20, LINEA);
            let etichetta = format!("{} {}", nome_mese(d.month()), d.year());
            let etichetta = adatta(&etichetta, true, CORPO, x1 - x0 - 200);
            pg.testo(x0 + 150, y_testata + 900, &etichetta, true, CORPO, NERO);
            d = fine_mese;
        }
        if passo_giorno >= 350 {
            let mut d = origine;
            while d < termine {
                pg.testo(
                    x_di(d) + 40,
                    y_testata + 2_050,
                    &d.day().to_string(),
                    false,
                    550,
                    GRIGIO,
                );
                d += Duration::days(1);
            }
        } else {
            let mut d = origine;
            while d.weekday() != chrono::Weekday::Mon {
                d += Duration::days(1);
            }
            while d < termine {
                if passo_giorno * 7 >= 900 {
                    pg.testo(
                        x_di(d) + 40,
                        y_testata + 2_050,
                        &d.day().to_string(),
                        false,
                        550,
                        GRIGIO,
                    );
                }
                pg.linea(
                    &[
                        (x_di(d), y_testata + 1_300),
                        (x_di(d), y_testata + TESTATA_TEMPO),
                    ],
                    10,
                    LINEA,
                );
                d += Duration::days(7);
            }
        }
        pg.linea(
            &[(MARGINE, y_testata + 1_300), (x_fine, y_testata + 1_300)],
            10,
            LINEA,
        );

        // Giorni non lavorativi.
        if passo_giorno >= 150 {
            let alto = y_righe;
            let basso = y_righe + i64::try_from(a - da).unwrap_or(0) * RIGA;
            for g in &piano.giorni_non_lavorativi {
                pg.rettangolo(
                    x_di(*g),
                    alto,
                    passo_giorno.max(1),
                    basso - alto,
                    "0.96 0.96 0.96",
                );
            }
        }

        // Righe.
        for (i, r) in righe.iter().enumerate().take(a).skip(da) {
            let y = y_riga(i);
            let base = y + 900;
            if i % 2 == 1 {
                pg.rettangolo(MARGINE, y, x_grafico - MARGINE, RIGA, "0.975 0.975 0.975");
            }
            pg.linea(&[(MARGINE, y + RIGA), (x_fine, y + RIGA)], 10, LINEA);
            let grassetto = r.riepilogo;
            let mut x = MARGINE;
            let rientro = i64::from(r.livello.saturating_sub(1)) * 500;
            let durata = if r.durata_stimata {
                format!("{} g?", r.durata)
            } else {
                format!("{} g", r.durata)
            };
            let testi = [
                (i + 1).to_string(),
                r.codice.clone(),
                r.nome.clone(),
                durata,
                formato::data(r.inizio),
                formato::data(r.fine),
            ];
            for (k, ((w, _), t)) in COLONNE.iter().zip(testi.iter()).enumerate() {
                let spazio = w - 300 - if k == 2 { rientro } else { 0 };
                let t = adatta(t, grassetto, CORPO, spazio);
                if k == 0 || k == 3 {
                    pg.testo_destra(x + w - 150, base, &t, grassetto, CORPO);
                } else {
                    let dx = if k == 2 { rientro } else { 0 };
                    pg.testo(x + 150 + dx, base, &t, grassetto, CORPO, NERO);
                }
                x += w;
            }

            // Barra.
            let xa = x_di(r.inizio);
            let xb = x_fine_di(r.fine);
            let critica = o.percorso_critico && r.critica;
            if o.linea_base
                && let (Some(bi), Some(bf)) = (r.inizio_base, r.fine_base)
            {
                let (ba, bb) = (x_di(bi), x_fine_di(bf));
                pg.rettangolo(ba, y + 900, (bb - ba).max(150), 220, GRIGIO);
            }
            if r.pietra_miliare {
                let cx = xb;
                let cy = y + RIGA / 2;
                pg.poligono(
                    &[
                        (cx, cy - 380),
                        (cx + 380, cy),
                        (cx, cy + 380),
                        (cx - 380, cy),
                    ],
                    if critica { ROSSO } else { NERO },
                );
                pg.testo(cx + 500, cy + 250, &formato::data(r.fine), false, 600, NERO);
            } else if r.riepilogo {
                let cy = y + 380;
                pg.rettangolo(xa, cy, (xb - xa).max(100), 250, NERO);
                pg.poligono(&[(xa, cy), (xa + 300, cy), (xa, cy + 550)], NERO);
                pg.poligono(&[(xb, cy), (xb - 300, cy), (xb, cy + 550)], NERO);
            } else {
                let (chiaro, scuro) = if critica {
                    (ROSSO, ROSSO_SCURO)
                } else {
                    (BLU, BLU_SCURO)
                };
                let w = (xb - xa).max(100);
                pg.rettangolo(xa, y + 300, w, 600, chiaro);
                if r.avanzamento > 0 {
                    let pw = w * i64::from(r.avanzamento.min(100)) / 100;
                    pg.rettangolo(xa, y + 520, pw, 160, scuro);
                }
            }
        }

        // Legami tra righe della stessa pagina (anche in parte fuori pagina: tratto fino
        // al bordo).
        for l in &piano.legami {
            let (Some(rp), Some(rs)) = (righe.get(l.riga_da), righe.get(l.riga_a)) else {
                continue;
            };
            let in_pagina = |i: usize| i >= da && i < a;
            if !in_pagina(l.riga_da) && !in_pagina(l.riga_a) {
                continue;
            }
            let y_di = |i: usize| -> i64 {
                if i < da {
                    y_righe
                } else if i >= a {
                    y_righe + i64::try_from(a - da).unwrap_or(0) * RIGA
                } else {
                    y_riga(i) + RIGA / 2
                }
            };
            let x_da = if l.tipo.dalla_fine() {
                x_fine_di(rp.fine)
            } else {
                x_di(rp.inizio)
            };
            let x_a = if l.tipo.sull_inizio() {
                if rs.pietra_miliare {
                    x_fine_di(rs.fine) - 380
                } else {
                    x_di(rs.inizio)
                }
            } else {
                x_fine_di(rs.fine)
            };
            let (y1, y2) = (y_di(l.riga_da), y_di(l.riga_a));
            let uscita = if l.tipo.dalla_fine() {
                x_da + 300
            } else {
                x_da - 300
            };
            // L'ultimo tratto entra nella barra da sinistra (inizio) o da destra (fine).
            let (accosto, diretto) = if l.tipo.sull_inizio() {
                (x_a - 300, uscita <= x_a - 300)
            } else {
                (x_a + 300, uscita >= x_a + 300)
            };
            let y_mezzo = if y2 > y1 {
                y2 - RIGA / 2
            } else {
                y2 + RIGA / 2
            };
            let punti = if diretto {
                vec![(x_da, y1), (uscita, y1), (uscita, y2), (x_a, y2)]
            } else {
                vec![
                    (x_da, y1),
                    (uscita, y1),
                    (uscita, y_mezzo),
                    (accosto, y_mezzo),
                    (accosto, y2),
                    (x_a, y2),
                ]
            };
            pg.linea(&punti, 25, "0.35 0.35 0.35");
            if in_pagina(l.riga_a) {
                let dir = if l.tipo.sull_inizio() { 1 } else { -1 };
                pg.poligono(
                    &[
                        (x_a, y2),
                        (x_a - dir * 250, y2 - 150),
                        (x_a - dir * 250, y2 + 150),
                    ],
                    "0.35 0.35 0.35",
                );
            }
        }

        // Bordi, legenda, numero di pagina.
        let y_fine_righe = y_righe + i64::try_from(a - da).unwrap_or(0) * RIGA;
        pg.linea(&[(MARGINE, y_testata), (x_fine, y_testata)], 40, NERO);
        pg.linea(&[(MARGINE, y_righe), (x_fine, y_righe)], 30, NERO);
        pg.linea(
            &[
                (x_grafico, y_testata),
                (x_grafico, y_fine_righe.max(y_righe)),
            ],
            30,
            NERO,
        );
        let y_legenda = ALTEZZA - MARGINE - 600;
        let mut x = MARGINE;
        pg.rettangolo(x, y_legenda - 450, 1_200, 500, BLU);
        pg.testo(x + 1_400, y_legenda, "Attività", false, 650, NERO);
        x += 5_500;
        if o.percorso_critico {
            pg.rettangolo(x, y_legenda - 450, 1_200, 500, ROSSO);
            pg.testo(x + 1_400, y_legenda, "Attività critica", false, 650, NERO);
            x += 7_000;
        }
        pg.rettangolo(x, y_legenda - 350, 1_200, 250, NERO);
        pg.testo(x + 1_400, y_legenda, "Riepilogo", false, 650, NERO);
        x += 5_500;
        pg.poligono(
            &[
                (x + 300, y_legenda - 550),
                (x + 600, y_legenda - 250),
                (x + 300, y_legenda + 50),
                (x, y_legenda - 250),
            ],
            NERO,
        );
        pg.testo(x + 900, y_legenda, "Pietra miliare", false, 650, NERO);
        x += 6_000;
        if o.linea_base {
            pg.rettangolo(x, y_legenda - 250, 1_200, 220, GRIGIO);
            pg.testo(x + 1_400, y_legenda, "Linea di base", false, 650, NERO);
        }
        let mut piede = format!("Pagina {} di {pagine_n}", np + 1);
        if let Some(d) = o.data_stampa {
            piede = format!("Stampato il {}  ·  {piede}", formato::data(d));
        }
        pg.testo_destra(x_fine, y_legenda, &piede, false, 650);
        pg.testo(
            x_fine - 30_000,
            y_legenda,
            "OpenComputo",
            false,
            650,
            GRIGIO,
        );
        pagine.push(pg);
    }
    let sostituiti = pagine.iter().map(|p| p.sostituiti).sum();
    let contenuti: Vec<&str> = pagine.iter().map(|p| p.ops.as_str()).collect();
    let byte = pdf::assembla(
        &contenuti,
        (LARGHEZZA, ALTEZZA),
        &["Helvetica", "Helvetica-Bold"],
        "Cronoprogramma dei lavori",
        o.comprimi,
    );
    Ok(RisultatoPdf {
        byte,
        pagine: pagine.len(),
        caratteri_sostituiti: sostituiti,
        anteprima: Vec::new(),
    })
}
