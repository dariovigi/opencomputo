//! Computo metrico (estimativo) nel modello tradizionale.

use opencomputo_core::classificazione::LivelloCategoria;
use opencomputo_core::computo::{Computo, QuantitaComputo, VoceComputo};
use opencomputo_core::id::ComputoId;
use opencomputo_core::misura::{self, Fattore, RigoMisura, SegnoRigo, TipoRigo};
use opencomputo_core::numeri::RegoleArrotondamento;
use opencomputo_core::partita::CalcoloPartita;
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;

use super::{
    Allineamento, Carattere, Colore, Corpo, Intestazione, Linea, OpzioniTradizionale, PaginaCorpo,
    SOTTILE, SPESSO, Tela, Testo, X_DESTRA, X_NUMERO, X_SINISTRA, Y_FONDO, capacita, disegna_linee,
    file_pdf, impagina, numero, riduci, righe, righe_paragrafi,
};
use crate::pdf::RisultatoPdf;
use crate::report::{ErroreReport, Quota, errore_calcolo};

// Colonne.

/// Testo della colonna del numero e della tariffa.
const NUMERO: (i64, i64) = (
    X_SINISTRA + super::RIENTRO,
    X_SINISTRA + super::RIENTRO + super::LARGHEZZA_TARIFFA,
);

/// Colonne di una quota (manodopera o sicurezza): euro e incidenza.
#[derive(Debug, Clone, Copy)]
struct ColonneQuota {
    quota: Quota,
    euro: (i64, i64),
    percentuale: (i64, i64),
}

/// Ascisse delle colonne. Ogni campo è il bordo destro della colonna che lo precede:
/// `designazione` chiude la designazione, `par` le parti uguali e così via fino a
/// `fine_totale`, che chiude la colonna dell'importo.
#[derive(Debug, Clone)]
struct Griglia {
    designazione: i64,
    par: i64,
    lung: i64,
    larg: i64,
    peso: i64,
    quantita: i64,
    unitario: i64,
    fine_totale: i64,
    quote: Vec<ColonneQuota>,
}

impl Griglia {
    /// Colonne delle stampe di riferimento (ADR 0015), senza quote.
    const STANDARD: Self = Self {
        designazione: 28_768,
        par: 32_226,
        lung: 35_684,
        larg: 39_538,
        peso: 43_393,
        quantita: 47_644,
        unitario: 51_896,
        fine_totale: X_DESTRA,
        quote: Vec::new(),
    };

    /// Con le quote come colonne aggiuntive a destra (ADR 0021): le misure e gli
    /// importi si stringono e la designazione prende lo spazio che resta.
    fn new(quote: &[Quota]) -> Self {
        if quote.is_empty() {
            return Self::STANDARD;
        }
        const FATTORE: i64 = 3_000;
        const QUANTITA: i64 = 4_000;
        const UNITARIO: i64 = 4_000;
        const TOTALE: i64 = 5_600;
        const EURO: i64 = 4_600;
        const PERCENTUALE: i64 = 2_600;
        let mut x = X_DESTRA;
        let mut colonne = Vec::with_capacity(quote.len());
        for q in quote.iter().rev() {
            colonne.push(ColonneQuota {
                quota: *q,
                euro: (x - PERCENTUALE - EURO, x - PERCENTUALE),
                percentuale: (x - PERCENTUALE, x),
            });
            x -= PERCENTUALE + EURO;
        }
        colonne.reverse();
        let fine_totale = x;
        let unitario = fine_totale - TOTALE;
        let quantita = unitario - UNITARIO;
        let peso = quantita - QUANTITA;
        let larg = peso - FATTORE;
        let lung = larg - FATTORE;
        let par = lung - FATTORE;
        Self {
            designazione: par - FATTORE,
            par,
            lung,
            larg,
            peso,
            quantita,
            unitario,
            fine_totale,
            quote: colonne,
        }
    }

    /// Testo della designazione.
    fn testo(&self) -> (i64, i64) {
        (
            X_NUMERO + super::RIENTRO,
            self.designazione - super::RIENTRO,
        )
    }

    /// Bordi destri delle colonne dei fattori.
    fn fattori(&self) -> [i64; 4] {
        [self.par, self.lung, self.larg, self.peso]
    }

    /// Filetto della quantità.
    fn filetto_quantita(&self) -> (i64, i64) {
        (self.peso + super::RIENTRO, self.quantita - super::RIENTRO)
    }

    /// Filetto degli importi e delle quote.
    fn filetto_totale(&self) -> (i64, i64) {
        (self.unitario + super::RIENTRO, X_DESTRA - super::RIENTRO)
    }

    /// Filetti verticali interni del corpo, da sinistra.
    fn filetti(&self) -> Vec<i64> {
        let mut x = vec![
            X_NUMERO,
            self.designazione,
            self.par,
            self.lung,
            self.larg,
            self.peso,
            self.quantita,
            self.unitario,
        ];
        for q in &self.quote {
            x.push(q.euro.0);
            x.push(q.percentuale.0);
        }
        x
    }
}

/// Riga del piè della tabella («A RIPORTARE»).
const Y_PIEDE: i64 = 78_673;
/// Altezza dell'intestazione delle colonne.
const ALTEZZA_TESTATA: i64 = 3_855;
/// Fine della riga «RIPORTO».
const ALTEZZA_RIPORTO: i64 = 5_782;
/// Distanza del testo dal bordo della riga.
const SCARTO: i64 = 626;

/// Decimali dei fattori: parti uguali, lunghezza, larghezza, altezza/peso.
const DECIMALI_FATTORI: [u32; 4] = [2, 2, 3, 3];

fn destra(x: i64) -> (i64, i64) {
    (x - 4_000, x - super::RIENTRO)
}

/// Valori delle quote nelle loro colonne: euro e incidenza percentuale.
fn valori_quote(g: &Griglia, valori: &[(Option<Decimal>, Option<Decimal>)], di: u32) -> Vec<Testo> {
    let mut out = Vec::new();
    for (c, (euro, pc)) in g.quote.iter().zip(valori) {
        match euro {
            Some(e) => out.push(valore(c.euro.1, numero(*e, di), Colore::Nero)),
            None => out.push(valore(c.euro.1, "—".into(), Colore::Nero)),
        }
        if let Some(pc) = pc {
            out.push(valore(c.percentuale.1, numero(*pc, 2), Colore::Nero));
        }
    }
    out
}

fn valore(x_colonna: i64, testo: String, colore: Colore) -> Testo {
    Testo::new(destra(x_colonna), Allineamento::Destra, testo).colore(colore)
}

struct Costruttore<'a> {
    corpo: Corpo,
    regole: &'a RegoleArrotondamento,
    /// Computo stampato e quantità delle sue voci, per i «vedi voce».
    computo: &'a Computo,
    riferite: QuantitaComputo,
    con_prezzi: bool,
    ridotte: bool,
    progressivo: Decimal,
    /// Colonne, con quelle delle quote se richieste.
    griglia: Griglia,
    /// Progressivi delle quote (somme esatte delle quote stampate).
    progressivi_quote: [Decimal; 2],
}

impl Costruttore<'_> {
    fn dq(&self) -> u32 {
        self.regole.decimali_quantita
    }

    fn di(&self) -> u32 {
        self.regole.decimali_importo
    }

    fn intestazione_gruppo(&mut self, titolo: &str) {
        self.corpo.spazio();
        self.corpo.linea(Linea::con(vec![
            Testo::new(
                (X_NUMERO, self.griglia.designazione),
                Allineamento::Centro,
                titolo,
            )
            .grassetto()
            .sottolineato(),
        ]));
        self.corpo.vuota();
    }

    /// Riga con un totale in euro (parziale o totale), le quote e i loro filetti.
    fn riga_euro(&mut self, etichetta: &str, importo: Decimal, quote: &[Decimal]) {
        let di = self.di();
        let g = &self.griglia;
        let mut testi = vec![
            Testo::new(g.testo(), Allineamento::Destra, etichetta).grassetto(),
            valore(g.fine_totale, numero(importo, di), Colore::Nero),
        ];
        let valori: Vec<_> = quote
            .iter()
            .map(|q| (Some(*q), crate::report::incidenza(*q, importo)))
            .collect();
        testi.extend(valori_quote(g, &valori, di));
        let filetto = g.filetto_totale();
        self.corpo
            .linea(Linea::con(testi).filetto(filetto, SOTTILE, false));
        self.corpo.linea(Linea {
            eliminabile: true,
            ..Linea::default().filetto(filetto, SPESSO, true)
        });
        self.corpo.vuota();
    }

    fn voce(
        &mut self,
        numero_ordine: usize,
        v: &VoceComputo,
        calcolo: &CalcoloPartita,
    ) -> Result<(), ErroreReport> {
        let prezzo = &v.prezzo;
        let um = prezzo.unita_misura.trim();
        let g = self.griglia.clone();
        let designazione = g.testo();
        let descrizione = if prezzo.descrizione_estesa.trim().is_empty() {
            prezzo.descrizione_breve.clone()
        } else {
            prezzo.descrizione_estesa.clone()
        };
        let descrizione = if self.ridotte {
            riduci(&descrizione)
        } else {
            descrizione
        };
        let mut sinistra = vec![numero_ordine.to_string()];
        sinistra.extend(righe(
            &prezzo.codice,
            Carattere::Times,
            super::CORPO,
            super::LARGHEZZA_TARIFFA,
        ));
        let testo = righe_paragrafi(
            &descrizione,
            Carattere::Times,
            super::CORPO,
            designazione.1 - designazione.0,
        );
        for i in 0..sinistra.len().max(testo.len()) {
            let mut l = Linea::default();
            if let Some(s) = sinistra.get(i) {
                l.testi
                    .push(Testo::new(NUMERO, Allineamento::Sinistra, s.clone()));
            }
            if let Some((t, ultima)) = testo.get(i) {
                let a = if *ultima {
                    Allineamento::Sinistra
                } else {
                    Allineamento::Giustificato
                };
                l.testi.push(Testo::new(designazione, a, t.clone()));
            }
            self.corpo.linea(l);
        }

        let dettaglio = misura::dettaglio_quantita_con(&v.righi, self.regole, &self.riferite)
            .map_err(errore_calcolo)?;
        let mut deduzioni = false;
        for ((rigo, risultato), progressivo) in v
            .righi
            .iter()
            .zip(&dettaglio.righi)
            .zip(&dettaglio.progressivi)
        {
            if rigo.tipo == TipoRigo::Commento {
                for r in righe(
                    &rigo.descrizione,
                    Carattere::Times,
                    super::CORPO,
                    designazione.1 - designazione.0,
                ) {
                    self.corpo.linea(Linea::con(vec![Testo::new(
                        designazione,
                        Allineamento::Sinistra,
                        r,
                    )]));
                }
                continue;
            }
            let rosso = rigo.segno == SegnoRigo::Deduzione;
            deduzioni |= rosso;
            let colore = if rosso { Colore::Rosso } else { Colore::Nero };
            let etichette = righe(
                &etichetta(
                    rigo,
                    rigo.vedi_voce.as_ref().map(|vv| {
                        crate::vedi_voce::testo(
                            vv,
                            Some(self.computo),
                            &self.riferite,
                            self.regole.decimali_quantita,
                        )
                    }),
                ),
                Carattere::Times,
                super::CORPO,
                designazione.1 - designazione.0,
            );
            let n = etichette.len();
            for (i, e) in etichette.into_iter().enumerate() {
                let mut l = Linea::con(vec![
                    Testo::new(designazione, Allineamento::Sinistra, e).colore(colore),
                ]);
                if i + 1 == n {
                    let contesto = rigo.contesto().map_err(errore_calcolo)?;
                    let fattori = [
                        contesto.parti_uguali,
                        contesto.lunghezza,
                        contesto.larghezza,
                        contesto.altezza_peso,
                    ];
                    for ((f, x), d) in fattori.iter().zip(g.fattori().iter()).zip(DECIMALI_FATTORI)
                    {
                        if let Some(f) = f {
                            l.testi.push(valore(*x, numero(*f, d), colore));
                        }
                    }
                    if let Some(r) = risultato {
                        l.testi.push(valore(
                            g.quantita,
                            numero(r.quantita.valore(), self.dq()),
                            colore,
                        ));
                    }
                    self.corpo.riporto.quantita = Some(progressivo.valore());
                }
                self.corpo.linea(l);
            }
        }

        let dq = self.dq();
        if deduzioni {
            self.corpo.vuota();
            self.corpo.linea(
                Linea::con(vec![
                    Testo::new(
                        designazione,
                        Allineamento::Destra,
                        format!("Sommano positivi {um}"),
                    ),
                    valore(
                        g.quantita,
                        numero(dettaglio.positivi.valore(), dq),
                        Colore::Nero,
                    ),
                ])
                .filetto(g.filetto_quantita(), SOTTILE, false),
            );
            self.corpo.linea(Linea::con(vec![
                Testo::new(
                    designazione,
                    Allineamento::Destra,
                    format!("Sommano negativi {um}"),
                )
                .colore(Colore::Rosso),
                valore(
                    g.quantita,
                    numero(dettaglio.deduzioni.valore(), dq),
                    Colore::Rosso,
                ),
            ]));
        }
        self.corpo.vuota();
        let mut sommano = vec![
            Testo::new(
                designazione,
                Allineamento::Destra,
                format!("SOMMANO {um}").trim_end().to_owned(),
            ),
            valore(
                g.quantita,
                numero(calcolo.quantita.valore(), dq),
                Colore::Nero,
            ),
        ];
        self.corpo.riporto.quantita = None;
        if self.con_prezzi {
            let di = self.di();
            sommano.push(valore(
                g.unitario,
                numero(prezzo.prezzo.valore(), di),
                Colore::Nero,
            ));
            sommano.push(valore(
                g.fine_totale,
                numero(calcolo.importo.valore(), di),
                Colore::Nero,
            ));
            self.progressivo = self
                .progressivo
                .checked_add(calcolo.importo.valore())
                .ok_or_else(|| ErroreReport::Calcolo("importo progressivo fuori scala".into()))?;
            self.corpo.riporto.totale = Some(self.progressivo);
            // Quote nelle loro colonne, con i progressivi per i riporti.
            let mut valori = Vec::new();
            for (k, c) in g.quote.iter().enumerate() {
                let quota = c.quota.importo(calcolo);
                let pc = quota.and(c.quota.incidenza(prezzo));
                if let (Some(q), Some(p)) = (quota, self.progressivi_quote.get_mut(k)) {
                    *p = p.checked_add(q).ok_or_else(|| {
                        ErroreReport::Calcolo("quota progressiva fuori scala".into())
                    })?;
                }
                if let (Some(r), Some(p)) = (
                    self.corpo.riporto.quote.get_mut(k),
                    self.progressivi_quote.get(k),
                ) {
                    *r = Some(*p);
                }
                valori.push((quota, pc));
            }
            sommano.extend(valori_quote(&g, &valori, di));
        }
        self.corpo
            .linea(Linea::con(sommano).filetto(g.filetto_quantita(), SOTTILE, false));
        self.corpo.linea(Linea {
            eliminabile: true,
            ..Linea::default().filetto(g.filetto_quantita(), SPESSO, true)
        });
        self.corpo.vuota();
        Ok(())
    }
}

/// Descrizione del rigo con le espressioni dei fattori, come «(par.ug.=4*15*2)».
fn etichetta(rigo: &RigoMisura, vedi: Option<String>) -> String {
    let mut t = crate::vedi_voce::con_descrizione(vedi, &rigo.descrizione);
    let mut aggiungi = |s: String| {
        if !t.is_empty() {
            t.push(' ');
        }
        t.push_str(&s);
    };
    for (nome, f) in [
        ("par.ug.", &rigo.parti_uguali),
        ("lung.", &rigo.lunghezza),
        ("larg.", &rigo.larghezza),
        ("H/peso", &rigo.altezza_peso),
    ] {
        if let Some(Fattore::Espressione(e)) = f {
            let e = e.trim();
            if e.chars()
                .any(|c| "+-*/^()".contains(c) || c.is_alphabetic())
            {
                aggiungi(format!("({nome}={e})"));
            }
        }
    }
    if let Some(f) = rigo
        .formula
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
    {
        aggiungi(format!("(quantità={f})"));
    }
    t
}

/// Gruppo di voci da stampare: titolo, indici delle voci, parziale e parziali delle
/// quote (dal core).
struct Gruppo {
    titolo: Option<String>,
    voci: Vec<usize>,
    parziale: Option<Decimal>,
    quote: Vec<Decimal>,
}

/// Raggruppa le voci per super categoria, nell'ordine dell'elenco delle super categorie,
/// se almeno una voce ne ha una; altrimenti un solo gruppo senza titolo.
fn gruppi(
    p: &Progetto,
    c: &opencomputo_core::computo::Computo,
    r: &opencomputo_core::computo::RiepilogoComputo,
    quote: &[Quota],
) -> Vec<Gruppo> {
    let quote_di = |chiave: Option<opencomputo_core::id::ClasseId>| -> Vec<Decimal> {
        quote
            .iter()
            .map(|q| {
                let t = &q.raggruppate(r).per_super_categoria;
                match chiave {
                    Some(k) => t.per_chiave.get(&k).copied().unwrap_or_default(),
                    None => t.non_assegnato,
                }
                .valore()
            })
            .collect()
    };
    let assegnate = c.voci.iter().any(|v| v.categorie.super_categoria.is_some());
    if !assegnate {
        return vec![Gruppo {
            titolo: None,
            voci: (0..c.voci.len()).collect(),
            parziale: None,
            quote: Vec::new(),
        }];
    }
    let elenco = p.categorie.elenco(LivelloCategoria::SuperCategoria);
    let mut out = Vec::new();
    for classe in elenco {
        let voci: Vec<usize> = c
            .voci
            .iter()
            .enumerate()
            .filter(|(_, v)| v.categorie.super_categoria == Some(classe.id))
            .map(|(i, _)| i)
            .collect();
        if voci.is_empty() {
            continue;
        }
        let titolo = if classe.descrizione.trim().is_empty() {
            classe.codice.clone()
        } else {
            classe.descrizione.trim().to_owned()
        };
        out.push(Gruppo {
            titolo: Some(titolo),
            voci,
            parziale: r
                .per_super_categoria
                .per_chiave
                .get(&classe.id)
                .map(|i| i.valore()),
            quote: quote_di(Some(classe.id)),
        });
    }
    let altre: Vec<usize> = c
        .voci
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            v.categorie
                .super_categoria
                .is_none_or(|id| !elenco.iter().any(|x| x.id == id))
        })
        .map(|(i, _)| i)
        .collect();
    if !altre.is_empty() {
        // Il parziale del core comprende solo le voci senza super categoria.
        let solo_senza = altre
            .iter()
            .filter_map(|i| c.voci.get(*i))
            .all(|v| v.categorie.super_categoria.is_none());
        out.push(Gruppo {
            titolo: Some("SENZA SUPER CATEGORIA".into()),
            voci: altre,
            parziale: solo_senza.then(|| r.per_super_categoria.non_assegnato.valore()),
            quote: quote_di(None),
        });
    }
    out
}

/// Computo metrico estimativo (o solo metrico, con `senza_prezzi`) nel modello
/// tradizionale.
///
/// # Errors
/// Computo inesistente o non calcolabile.
pub fn computo(
    p: &Progetto,
    id: ComputoId,
    o: &OpzioniTradizionale,
) -> Result<RisultatoPdf, ErroreReport> {
    let c = p
        .computo(id)
        .ok_or_else(|| ErroreReport::ComputoInesistente(id.to_string()))?;
    let r = p.calcola_computo(id).map_err(errore_calcolo)?;
    let con_prezzi = !o.senza_prezzi;
    let mut k = Costruttore {
        corpo: Corpo::default(),
        regole: &p.regole,
        computo: c,
        riferite: c.quantita_riferite(&p.regole),
        con_prezzi,
        ridotte: o.descrizioni_ridotte,
        progressivo: Decimal::ZERO,
        griglia: Griglia::new(&if con_prezzi {
            o.incidenze.quote()
        } else {
            Vec::new()
        }),
        progressivi_quote: [Decimal::ZERO; 2],
    };
    let quote: Vec<Quota> = k.griglia.quote.iter().map(|c| c.quota).collect();
    let con_riepilogo = con_prezzi && o.incidenze.riepilogo;
    let gruppi = gruppi(p, c, &r, &quote);
    let n_gruppi = gruppi.len();
    for (g, gruppo) in gruppi.iter().enumerate() {
        if let Some(t) = &gruppo.titolo {
            k.intestazione_gruppo(t);
        } else {
            k.corpo.spazio();
        }
        for i in &gruppo.voci {
            let (Some(v), Some(ris)) = (c.voci.get(*i), r.voci.get(*i)) else {
                continue;
            };
            k.voce(ris.numero, v, &ris.calcolo)?;
        }
        if con_prezzi && let (Some(t), Some(parziale)) = (&gruppo.titolo, gruppo.parziale) {
            k.riga_euro(&format!("Parziale {t} euro"), parziale, &gruppo.quote);
        }
        if g + 1 < n_gruppi {
            k.corpo.vuota();
            k.corpo.chiudi();
        }
    }
    if con_prezzi {
        if k.progressivo != r.totale.valore() {
            return Err(ErroreReport::Calcolo(format!(
                "la somma degli importi stampati ({}) non coincide con il totale del computo ({})",
                k.progressivo,
                r.totale.valore()
            )));
        }
        let totali: Vec<Decimal> = quote.iter().map(|q| q.totale(&r)).collect();
        for (stampata, (q, totale)) in k.progressivi_quote.iter().zip(quote.iter().zip(&totali)) {
            if stampata != totale {
                return Err(ErroreReport::Calcolo(format!(
                    "la somma delle quote di {} stampate ({stampata}) non coincide con il totale del computo ({totale})",
                    q.nome()
                )));
            }
        }
        k.riga_euro("T O T A L E   euro", r.totale.valore(), &totali);
    }
    let int = Intestazione::new(p, o);
    let centro = (X_NUMERO, k.griglia.designazione);
    let mut riepilogo = None;
    if con_riepilogo {
        k.corpo.vuota();
        k.corpo.chiudi();
        let mut corpo = Corpo::default();
        let colonne =
            super::riepilogo::riepilogo(&mut corpo, p, &r, &o.incidenze.quote(), &p.regole)?;
        firma(&mut corpo, &int, (X_NUMERO, colonne.descrizione.1));
        riepilogo = Some((corpo, colonne));
    } else {
        firma(&mut k.corpo, &int, centro);
    }

    let alto = int.alto_riquadro();
    let inizio = alto + ALTEZZA_RIPORTO;
    let cap = capacita(inizio, SCARTO, Y_PIEDE);
    let tratteggio = Testo::new(centro, Allineamento::Centro, "-".repeat(40));
    let pagine = impagina(k.corpo.elementi, cap, &tratteggio);
    let pagine_riepilogo = riepilogo.map(|(corpo, colonne)| {
        let cap = capacita(
            alto + super::riepilogo::ALTEZZA_TESTATA + super::riepilogo::STACCO,
            SCARTO,
            Y_FONDO - super::riepilogo::STACCO,
        );
        let tratteggio = Testo::new(
            (X_NUMERO, colonne.descrizione.1),
            Allineamento::Centro,
            "-".repeat(40),
        );
        (impagina(corpo.elementi, cap, &tratteggio), colonne)
    });

    let titolo = if con_prezzi {
        "COMPUTO METRICO ESTIMATIVO"
    } else {
        "COMPUTO METRICO"
    };
    let mut tele = Vec::new();
    if o.frontespizio {
        let mut t = Tela::default();
        int.frontespizio(&mut t, titolo);
        tele.push(t);
    }
    let primo = tele.len() + 1;
    let regole = &p.regole;
    for (i, pc) in pagine.iter().enumerate() {
        let mut t = Tela::default();
        pagina(&mut t, &int, primo + i, pc, regole, &k.griglia);
        tele.push(t);
    }
    if let Some((pagine_riepilogo, colonne)) = &pagine_riepilogo {
        let primo = primo + pagine.len();
        for (i, pc) in pagine_riepilogo.iter().enumerate() {
            let mut t = Tela::default();
            super::riepilogo::pagina(&mut t, &int, primo + i, pc, colonne);
            tele.push(t);
        }
    }
    Ok(file_pdf(tele, titolo, o.comprimi))
}

/// Luogo e data, «Il Tecnico» e il nome del progettista, poi chiude la pagina.
fn firma(corpo: &mut Corpo, int: &Intestazione, centro: (i64, i64)) {
    let spazi = super::larghezza("     ", Carattere::Times, super::CORPO);
    corpo.linea(Linea::con(vec![Testo::new(
        (X_NUMERO + super::RIENTRO + spazi, X_DESTRA),
        Allineamento::Sinistra,
        int.luogo_data.clone(),
    )]));
    corpo.vuota();
    corpo.linea(Linea::con(vec![
        Testo::new(centro, Allineamento::Centro, int.ruolo.clone()).grassetto(),
    ]));
    if let Some(t) = &int.tecnico {
        corpo.linea(Linea::con(vec![Testo::new(
            centro,
            Allineamento::Centro,
            t.clone(),
        )]));
    }
    corpo.vuota();
    corpo.vuota();
    corpo.chiudi();
}

/// Disegna una pagina della tabella del computo.
fn pagina(
    tela: &mut Tela,
    int: &Intestazione,
    numero_pagina: usize,
    pc: &PaginaCorpo,
    regole: &RegoleArrotondamento,
    g: &Griglia,
) {
    let alto = int.alto_riquadro();
    int.cornice(tela, numero_pagina, alto);
    let verde = Colore::Verde;
    let ca = Carattere::Times;
    let corpo = super::CORPO;

    // Intestazione delle colonne.
    let basso_testata = alto + ALTEZZA_TESTATA;
    let mut spessi = vec![X_NUMERO, g.designazione, g.peso, g.quantita];
    spessi.extend(g.quote.iter().map(|q| q.euro.0));
    for x in spessi {
        tela.verticale(x, alto, basso_testata, SPESSO, verde);
    }
    let mut sottili = vec![g.par, g.lung, g.larg, g.unitario];
    sottili.extend(g.quote.iter().map(|q| q.percentuale.0));
    for x in sottili {
        tela.verticale(x, alto + 1_928, basso_testata, SOTTILE, verde);
    }
    let mut gruppi = vec![(g.designazione, g.peso), (g.quantita, g.fine_totale)];
    gruppi.extend(g.quote.iter().map(|q| (q.euro.0, q.percentuale.1)));
    for (x0, x1) in gruppi {
        tela.orizzontale(x0, x1, alto + 1_758, SOTTILE, verde);
        tela.orizzontale(x0, x1, alto + 1_928, SOTTILE, verde);
    }
    tela.orizzontale(X_SINISTRA, X_DESTRA, basso_testata, SPESSO, verde);
    tela.testo_centro(
        (g.designazione, g.peso),
        alto + 626,
        "D I M E N S I O N I",
        ca,
        corpo,
        verde,
    );
    tela.testo_centro(
        (g.quantita, g.fine_totale),
        alto + 626,
        "I M P O R T I",
        ca,
        corpo,
        verde,
    );
    for q in &g.quote {
        let titolo = match q.quota {
            Quota::Manodopera => "MANODOPERA",
            Quota::Sicurezza => "SICUREZZA",
        };
        tela.testo_centro(
            (q.euro.0, q.percentuale.1),
            alto + 626,
            titolo,
            ca,
            corpo,
            verde,
        );
        tela.testo_centro(q.euro, alto + 2_553, "euro", ca, corpo, verde);
        tela.testo_centro(q.percentuale, alto + 2_553, "incid.%", ca, corpo, verde);
    }
    tela.testo_centro(
        (X_SINISTRA, X_NUMERO),
        alto + 1_108,
        "Num.Ord.",
        ca,
        corpo,
        verde,
    );
    tela.testo_centro(
        (X_SINISTRA, X_NUMERO),
        alto + 2_072,
        "TARIFFA",
        ca,
        corpo,
        verde,
    );
    tela.testo_centro(
        (X_NUMERO, g.designazione),
        alto + 1_590,
        "DESIGNAZIONE DEI LAVORI",
        ca,
        corpo,
        verde,
    );
    tela.testo_centro(
        (g.peso, g.quantita),
        alto + 1_590,
        "Quantità",
        ca,
        corpo,
        verde,
    );
    for ((x0, x1), t) in [
        ((g.designazione, g.par), "par.ug."),
        ((g.par, g.lung), "lung."),
        ((g.lung, g.larg), "larg."),
        ((g.larg, g.peso), "H/peso"),
        ((g.quantita, g.unitario), "unitario"),
        ((g.unitario, g.fine_totale), "TOTALE"),
    ] {
        tela.testo_centro((x0, x1), alto + 2_553, t, ca, corpo, verde);
    }

    // Riga «RIPORTO», corpo e piè «A RIPORTARE».
    let inizio = alto + ALTEZZA_RIPORTO;
    for x in g.filetti() {
        tela.verticale(x, basso_testata, Y_FONDO, SOTTILE, verde);
    }
    tela.orizzontale(X_SINISTRA, X_DESTRA, inizio - 170, SOTTILE, verde);
    tela.orizzontale(X_SINISTRA, X_DESTRA, inizio, SOTTILE, verde);
    tela.orizzontale(X_SINISTRA, X_DESTRA, Y_PIEDE, SOTTILE, verde);
    tela.orizzontale(X_SINISTRA, X_DESTRA, Y_PIEDE + 170, SOTTILE, verde);
    let riporti = [
        (basso_testata + 626, "R I P O R T O ", pc.riporto_iniziale),
        (Y_PIEDE + 626, "A   R I P O R T A R E ", pc.riporto_finale),
    ];
    for (y, etichetta, riporto) in riporti {
        tela.testo_destra(g.testo().1, y, etichetta, ca, corpo, verde);
        let mut valori = Vec::new();
        if let Some(q) = riporto.quantita {
            valori.push((g.quantita, numero(q, regole.decimali_quantita)));
        }
        if let Some(t) = riporto.totale {
            valori.push((g.fine_totale, numero(t, regole.decimali_importo)));
        }
        for (c, q) in g.quote.iter().zip(riporto.quote) {
            if let Some(q) = q {
                valori.push((c.euro.1, numero(q, regole.decimali_importo)));
            }
        }
        for (x, t) in valori {
            tela.testo_destra(x - super::RIENTRO, y, &t, ca, corpo, Colore::Nero);
        }
    }
    disegna_linee(tela, &pc.linee, inizio, SCARTO);
}
