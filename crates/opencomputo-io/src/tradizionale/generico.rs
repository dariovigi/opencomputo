//! Documenti di cantiere nel modello tradizionale (ADR 0023): libretto, registro, SAL,
//! certificato, sommario, conto finale, analisi, fabbisogno, nuovi prezzi, computo con
//! le analisi.
//!
//! Ogni documento è il [`Documento`] ordinario, con gli stessi valori del core, disegnato
//! con l'aspetto delle stampe tradizionali:
//! * frontespizio e testata dello studio;
//! * riquadro verde e caratteri Times;
//! * per ogni sezione una tabella con l'intestazione verde, ripetuta in cima alle
//!   pagine in cui la tabella continua;
//! * filetti sopra «SOMMANO» e i totali;
//! * firma in fondo.
//!
//! Le colonne nascono dalla tabella del documento: le numeriche prendono la larghezza
//! dei loro valori, quelle di testo si dividono il resto secondo il peso.

use opencomputo_core::progetto::Progetto;

use super::{
    Allineamento, Carattere, Colore, Intestazione, Linea, OpzioniTradizionale, RIENTRO, SOTTILE,
    SPESSO, Tela, Testo, X_DESTRA, X_SINISTRA, Y_FONDO, capacita, disegna_linee, file_pdf,
    larghezza, numero, riduci, righe,
};
use crate::documento::{self, Documento, StileRiga, Tabella, Valore};
use crate::pdf::RisultatoPdf;

/// Spazio tra il bordo superiore del riquadro e la prima riga, e prima del fondo.
const STACCO: i64 = 300;
/// Distanza del testo dal bordo della riga: il testo sta dentro la sua riga, così i
/// filetti sui bordi (intestazioni, totali, fine tabella) non lo attraversano.
const SCARTO: i64 = 110;
/// Larghezza minima di una colonna di testo.
const MINIMO_TESTO: i64 = 2_400;

/// Testo di una cella, con i numeri all'italiana e l'apice per le migliaia.
fn testo_cella(v: &Valore) -> String {
    match v {
        Valore::Vuoto => String::new(),
        Valore::Testo(t) => t.clone(),
        Valore::Numero { valore, decimali } => numero(*valore, *decimali),
    }
}

fn numerica(c: &documento::Colonna) -> bool {
    c.allineamento != documento::Allineamento::Sinistra
}

fn carattere(stile: StileRiga) -> Carattere {
    match stile {
        StileRiga::Totale | StileRiga::Gruppo => Carattere::TimesGrassetto,
        _ => Carattere::Times,
    }
}

/// Larghezza della parola più lunga.
fn parola_massima(t: &str, c: Carattere) -> i64 {
    t.split_whitespace()
        .map(|p| larghezza(p, c, super::CORPO))
        .max()
        .unwrap_or(0)
}

/// Ascisse delle colonne (inizio e fine) su tutta la larghezza del riquadro.
fn colonne(t: &Tabella) -> Vec<(i64, i64)> {
    /// Colonna in calcolo: larghezza minima, peso e se è numerica.
    struct Misura {
        larga: i64,
        peso: i64,
        numerica: bool,
        libera: bool,
    }
    let totale = X_DESTRA - X_SINISTRA;
    let mut m: Vec<Misura> = t
        .colonne
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let titolo = parola_massima(&c.titolo, Carattere::Times);
            let peso = i64::from(c.peso.max(1));
            if numerica(c) {
                let valori = t
                    .righe
                    .iter()
                    .filter_map(|r| r.celle.get(i).map(|v| (r.stile, v)))
                    .map(|(s, v)| larghezza(&testo_cella(v), carattere(s), super::CORPO))
                    .max()
                    .unwrap_or(0);
                // Il titolo intero su una riga, se non allarga troppo la colonna.
                let intero = larghezza(&c.titolo, Carattere::Times, super::CORPO);
                let titolo = if intero <= valori.max(titolo) * 3 / 2 {
                    intero
                } else {
                    titolo
                };
                Misura {
                    larga: titolo.max(valori) + 4 * RIENTRO,
                    peso,
                    numerica: true,
                    libera: false,
                }
            } else {
                Misura {
                    larga: (titolo + 2 * RIENTRO).max(MINIMO_TESTO),
                    peso,
                    numerica: false,
                    libera: true,
                }
            }
        })
        .collect();
    let fisse: i64 = m.iter().filter(|c| c.numerica).map(|c| c.larga).sum();
    let minimi: i64 = m.iter().filter(|c| !c.numerica).map(|c| c.larga).sum();
    let resto = totale - fisse;
    if m.iter().all(|c| c.numerica) || resto < minimi {
        // Colonne troppo larghe per il riquadro: tutte in proporzione al peso.
        let somma: i64 = m.iter().map(|c| c.peso).sum::<i64>().max(1);
        for c in &mut m {
            c.larga = totale * c.peso / somma;
        }
    } else {
        // Le colonne di testo che resterebbero sotto il minimo lo tengono; le altre si
        // dividono il resto in proporzione al peso.
        loop {
            let da_dividere: i64 = resto
                - m.iter()
                    .filter(|c| !c.numerica && !c.libera)
                    .map(|c| c.larga)
                    .sum::<i64>();
            let pesi: i64 = m
                .iter()
                .filter(|c| c.libera)
                .map(|c| c.peso)
                .sum::<i64>()
                .max(1);
            let mut cambiato = false;
            for c in m.iter_mut().filter(|c| c.libera) {
                if da_dividere * c.peso / pesi < c.larga {
                    c.libera = false;
                    cambiato = true;
                }
            }
            if !cambiato {
                for c in m.iter_mut().filter(|c| c.libera) {
                    c.larga = da_dividere * c.peso / pesi;
                }
                break;
            }
        }
    }
    // L'ultima colonna chiude sul riquadro (resti di arrotondamento).
    let n = m.len();
    let mut x = X_SINISTRA;
    m.iter()
        .enumerate()
        .map(|(i, c)| {
            let fine = if i + 1 == n { X_DESTRA } else { x + c.larga };
            let col = (x, fine);
            x = fine;
            col
        })
        .collect()
}

/// Pagine in costruzione.
struct Pagine {
    pagine: Vec<Vec<Linea>>,
    capacita: usize,
}

impl Pagine {
    fn libere(&self) -> usize {
        self.capacita
            .saturating_sub(self.pagine.last().map_or(0, Vec::len))
    }

    fn nuova(&mut self) {
        self.pagine.push(Vec::new());
    }

    fn linea(&mut self, l: Linea) {
        if self.libere() == 0 {
            self.nuova();
        }
        if let Some(p) = self.pagine.last_mut() {
            if l.eliminabile && p.is_empty() {
                return;
            }
            p.push(l);
        }
    }

    /// Righe che restano insieme; se non entrano nella pagina, prima `testata`.
    fn gruppo(&mut self, righe: Vec<Linea>, testata: &[Linea]) {
        if righe.len() > self.libere() && righe.len() <= self.capacita - testata.len() {
            self.nuova();
            for l in testata {
                self.linea(l.clone());
            }
        }
        for l in righe {
            self.linea(l);
        }
    }

    fn in_cima(&self) -> bool {
        self.pagine.last().is_none_or(Vec::is_empty)
    }
}

/// Riga vuota con i filetti delle colonne.
fn vuota_con(verticali: &[i64]) -> Linea {
    Linea {
        verticali: verticali.to_vec(),
        ..Linea::default()
    }
}

/// Intestazione delle colonne: titoli verdi su una o più righe, tra due filetti spessi.
fn testata(t: &Tabella, x: &[(i64, i64)], verticali: &[i64]) -> Vec<Linea> {
    let titoli: Vec<Vec<String>> = t
        .colonne
        .iter()
        .zip(x)
        .map(|(c, (x0, x1))| {
            righe(
                &c.titolo,
                Carattere::Times,
                super::CORPO,
                x1 - x0 - 2 * RIENTRO,
            )
        })
        .collect();
    let n = titoli.iter().map(Vec::len).max().unwrap_or(1).max(1);
    (0..n)
        .map(|k| {
            let mut l = vuota_con(verticali);
            for (righe_titolo, (x0, x1)) in titoli.iter().zip(x) {
                // Titoli allineati in basso.
                let salto = n - righe_titolo.len();
                if let Some(r) = k.checked_sub(salto).and_then(|i| righe_titolo.get(i)) {
                    l.testi.push(
                        Testo::new(
                            (x0 + RIENTRO, x1 - RIENTRO),
                            Allineamento::Centro,
                            r.clone(),
                        )
                        .colore(Colore::Verde),
                    );
                }
            }
            if k == 0 {
                l = l.filetto_verde((X_SINISTRA, X_DESTRA), SPESSO, false);
            }
            if k + 1 == n {
                l = l.filetto_verde((X_SINISTRA, X_DESTRA), SPESSO, true);
            }
            l
        })
        .collect()
}

/// Righe di una riga della tabella: testi a capo nelle loro colonne, numeri sull'ultima.
fn righe_di(
    riga: &documento::Riga,
    t: &Tabella,
    x: &[(i64, i64)],
    verticali: &[i64],
    ridotte: bool,
) -> Vec<Linea> {
    let stile = riga.stile;
    let c = carattere(stile);
    // Colonna principale: la più larga tra quelle di testo. I valori alla sua destra
    // (unità di misura, quantità, prezzi, importi) stanno sull'ultima riga del testo,
    // quelli alla sua sinistra (numero, codice) sulla prima.
    let principale = t
        .colonne
        .iter()
        .zip(x)
        .enumerate()
        .filter(|(_, (c, _))| !numerica(c))
        .max_by_key(|(_, (_, (x0, x1)))| x1 - x0)
        .map_or(0, |(i, _)| i);
    let mut testi: Vec<(usize, Vec<String>)> = Vec::new();
    let mut numeri: Vec<(usize, String)> = Vec::new();
    for (i, v) in riga.celle.iter().enumerate() {
        let testo = testo_cella(v);
        if testo.is_empty() {
            continue;
        }
        let Some((x0, x1)) = x.get(i) else { continue };
        let testo = if ridotte && stile == StileRiga::Voce {
            riduci(&testo)
        } else {
            testo
        };
        let r = righe(&testo, c, super::CORPO, x1 - x0 - 2 * RIENTRO);
        if i > principale && (matches!(v, Valore::Numero { .. }) || r.len() <= 1) {
            numeri.push((i, testo));
        } else {
            testi.push((i, r));
        }
    }
    let n = testi.iter().map(|(_, r)| r.len()).max().unwrap_or(0).max(1);
    // Nelle righe «SOMMANO» e nei totali anche il testo va a destra, verso i valori.
    let allineamento = |i: usize| match t.colonne.get(i).map(|c| c.allineamento) {
        _ if matches!(stile, StileRiga::Sommano | StileRiga::Totale) => Allineamento::Destra,
        Some(documento::Allineamento::Destra) => Allineamento::Destra,
        Some(documento::Allineamento::Centro) => Allineamento::Centro,
        _ => Allineamento::Sinistra,
    };
    let stilizza = |testo: Testo| {
        let testo = if c == Carattere::TimesGrassetto {
            testo.grassetto()
        } else {
            testo
        };
        if stile == StileRiga::Gruppo {
            testo.sottolineato()
        } else {
            testo
        }
    };
    let mut out: Vec<Linea> = (0..n).map(|_| vuota_con(verticali)).collect();
    for (i, r) in &testi {
        let Some((x0, x1)) = x.get(*i) else { continue };
        for (k, t) in r.iter().enumerate() {
            if let Some(l) = out.get_mut(k) {
                l.testi.push(stilizza(Testo::new(
                    (x0 + RIENTRO, x1 - RIENTRO),
                    allineamento(*i),
                    t.clone(),
                )));
            }
        }
    }
    if let Some(ultima) = out.last_mut() {
        for (i, t) in &numeri {
            let Some((x0, x1)) = x.get(*i) else { continue };
            ultima.testi.push(stilizza(Testo::new(
                (x0 + RIENTRO, x1 - RIENTRO),
                allineamento(*i),
                t.clone(),
            )));
        }
    }
    // Filetto sopra i valori di «SOMMANO» e dei totali.
    if matches!(stile, StileRiga::Sommano | StileRiga::Totale)
        && let Some(prima) = out.first_mut()
    {
        for (i, _) in &numeri {
            if let Some((x0, x1)) = x.get(*i) {
                prima.filetti.push(super::Filetto {
                    x: (x0 + RIENTRO, x1 - RIENTRO),
                    spessore: SOTTILE,
                    sotto: false,
                    colore: Colore::Nero,
                });
            }
        }
    }
    // Spazio prima delle voci, dei totali e dei gruppi: il testo della riga precedente
    // scende nella parte alta della successiva.
    if matches!(
        stile,
        StileRiga::Voce | StileRiga::Totale | StileRiga::Gruppo | StileRiga::Sommano
    ) {
        out.insert(0, vuota_con(verticali));
    }
    out
}

/// Righe di testo su tutta la larghezza.
fn paragrafo(p: &mut Pagine, testo: &str, c: Carattere) {
    let x = (X_SINISTRA + 2 * RIENTRO, X_DESTRA - 2 * RIENTRO);
    for r in righe(testo, c, super::CORPO, x.1 - x.0) {
        let t = Testo::new(x, Allineamento::Sinistra, r);
        p.linea(Linea::con(vec![if c == Carattere::TimesGrassetto {
            t.grassetto()
        } else {
            t
        }]));
    }
}

/// Documento ordinario disegnato nel modello tradizionale.
#[must_use]
pub fn documento(
    p: &Progetto,
    doc: &Documento,
    titolo: &str,
    o: &OpzioniTradizionale,
) -> RisultatoPdf {
    let int = Intestazione::new(p, o);
    let alto = int.alto_riquadro();
    let inizio = alto + STACCO;
    let cap = capacita(inizio, SCARTO, Y_FONDO - STACCO);
    let mut pg = Pagine {
        pagine: vec![Vec::new()],
        capacita: cap,
    };
    let tutta = (X_SINISTRA, X_DESTRA);

    pg.linea(Linea::con(vec![
        Testo::new(tutta, Allineamento::Centro, titolo.to_uppercase())
            .grassetto()
            .sottolineato(),
    ]));
    pg.linea(Linea::default());
    if !o.frontespizio {
        for (k, v) in &doc.intestazione {
            paragrafo(&mut pg, &format!("{k}: {v}"), Carattere::Times);
        }
        pg.linea(Linea::default());
    }

    for sezione in &doc.sezioni {
        let t = &sezione.tabella;
        if t.colonne.is_empty() {
            continue;
        }
        let x = colonne(t);
        let verticali: Vec<i64> = x.iter().skip(1).map(|(x0, _)| *x0).collect();
        let intestazione = testata(t, &x, &verticali);
        // Titolo, note, intestazione e due righe nella stessa pagina.
        let note: usize = sezione
            .note
            .iter()
            .map(|n| {
                righe(
                    n,
                    Carattere::Times,
                    super::CORPO,
                    X_DESTRA - X_SINISTRA - 4 * RIENTRO,
                )
                .len()
            })
            .sum();
        if !pg.in_cima() && pg.libere() < 2 + note + intestazione.len() + 3 {
            pg.nuova();
        }
        paragrafo(&mut pg, &sezione.titolo, Carattere::TimesGrassetto);
        for n in &sezione.note {
            paragrafo(&mut pg, n, Carattere::Times);
        }
        for l in &intestazione {
            pg.linea(l.clone());
        }
        let n = t.righe.len();
        for (i, riga) in t.righe.iter().enumerate() {
            let mut gruppo = righe_di(riga, t, &x, &verticali, o.descrizioni_ridotte);
            if i + 1 == n
                && let Some(ultima) = gruppo.last_mut()
            {
                *ultima = std::mem::take(ultima).filetto_verde(tutta, SOTTILE, true);
            }
            if pg.in_cima() {
                for l in &intestazione {
                    pg.linea(l.clone());
                }
            }
            pg.gruppo(gruppo, &intestazione);
        }
        pg.linea(Linea {
            eliminabile: true,
            ..Linea::default()
        });
        pg.linea(Linea {
            eliminabile: true,
            ..Linea::default()
        });
    }

    // Luogo, data e firma.
    let centro = (X_DESTRA - 22_000, X_DESTRA - 2_000);
    let mut firma = vec![
        Linea::con(vec![Testo::new(
            (X_SINISTRA + 2_000, X_DESTRA),
            Allineamento::Sinistra,
            int.luogo_data.clone(),
        )]),
        Linea::default(),
        Linea::con(vec![
            Testo::new(centro, Allineamento::Centro, int.ruolo.clone()).grassetto(),
        ]),
    ];
    if let Some(t) = &int.tecnico {
        firma.push(Linea::con(vec![Testo::new(
            centro,
            Allineamento::Centro,
            t.clone(),
        )]));
    }
    pg.gruppo(firma, &[]);

    // Pagina finale completata con il tratteggio.
    let tratteggio = Testo::new(tutta, Allineamento::Centro, "-".repeat(40));
    if let Some(ultima) = pg.pagine.last_mut() {
        if !ultima.is_empty() {
            ultima.push(Linea::default());
        }
        while ultima.len() < cap {
            ultima.push(Linea::con(vec![tratteggio.clone()]));
        }
    }

    let mut tele = Vec::new();
    if o.frontespizio {
        let mut t = Tela::default();
        int.frontespizio(&mut t, &titolo.to_uppercase());
        tele.push(t);
    }
    let primo = tele.len() + 1;
    let pagine: Vec<Vec<Linea>> = pg.pagine.into_iter().filter(|l| !l.is_empty()).collect();
    for (i, linee) in pagine.iter().enumerate() {
        let mut t = Tela::default();
        int.cornice(&mut t, primo + i, alto);
        disegna_linee(&mut t, linee, inizio, SCARTO);
        tele.push(t);
    }
    file_pdf(tele, &titolo.to_uppercase(), o.comprimi)
}
