//! Riepilogo finale del computo nel modello tradizionale (ADR 0021): importi per super
//! categorie, categorie, sotto categorie e WBS con i subtotali delle quote di
//! manodopera e di sicurezza, poi il quadro generale.
//!
//! Le pagine del riepilogo hanno colonne proprie (descrizione, importo e, per ogni
//! quota, euro e incidenza) e nessun riporto: i valori vengono tutti dal core.

use opencomputo_core::computo::RiepilogoComputo;
use opencomputo_core::numeri::RegoleArrotondamento;
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;

use super::{
    Allineamento, Carattere, Colore, Corpo, Intestazione, Linea, PaginaCorpo, RIENTRO, SOTTILE,
    SPESSO, Tela, Testo, X_DESTRA, X_NUMERO, X_SINISTRA, Y_FONDO, disegna_linee, numero, righe,
};
use crate::report::{self, ErroreReport, Quota};

/// Altezza dell'intestazione delle colonne.
pub(super) const ALTEZZA_TESTATA: i64 = 3_855;
/// Spazio tra l'intestazione e la prima riga, e prima del fondo del riquadro.
pub(super) const STACCO: i64 = 300;
/// Distanza del testo dal bordo della riga.
const SCARTO: i64 = 626;
/// Inizio delle colonne numeriche con due quote (come la colonna dei fattori del
/// computo): l'unità di larghezza divide questo spazio in 29 parti.
const X_NUMERI_MINIMO: i64 = 28_768;
/// Larghezze in unità: importo ed euro di una quota 7, incidenza 4.
const UNITA_IMPORTO: i64 = 7;
const UNITA_PERCENTUALE: i64 = 4;

/// Colonne di una quota: euro e incidenza percentuale.
#[derive(Debug, Clone, Copy)]
pub(super) struct ColonneQuota {
    quota: Quota,
    euro: (i64, i64),
    percentuale: (i64, i64),
}

/// Colonne delle pagine del riepilogo.
#[derive(Debug, Clone)]
pub(super) struct Colonne {
    /// Testo del codice.
    codice: (i64, i64),
    /// Testo della descrizione (dal filetto del numero all'inizio dei valori).
    pub(super) descrizione: (i64, i64),
    /// Colonna dell'importo.
    importo: (i64, i64),
    quote: Vec<ColonneQuota>,
}

impl Colonne {
    fn new(quote: &[Quota]) -> Self {
        let n = i64::try_from(quote.len()).unwrap_or(0);
        let unita = (X_DESTRA - X_NUMERI_MINIMO)
            / (UNITA_IMPORTO + 2 * (UNITA_IMPORTO + UNITA_PERCENTUALE));
        let larghezza = (UNITA_IMPORTO + n * (UNITA_IMPORTO + UNITA_PERCENTUALE)) * unita;
        let inizio = X_DESTRA - larghezza;
        let mut x = inizio + UNITA_IMPORTO * unita;
        let importo = (inizio, x);
        let mut colonne_quote = Vec::new();
        for (i, q) in quote.iter().enumerate() {
            let euro = (x, x + UNITA_IMPORTO * unita);
            let fine = if i + 1 == quote.len() {
                X_DESTRA
            } else {
                euro.1 + UNITA_PERCENTUALE * unita
            };
            colonne_quote.push(ColonneQuota {
                quota: *q,
                euro,
                percentuale: (euro.1, fine),
            });
            x = fine;
        }
        let importo = if quote.is_empty() {
            (importo.0, X_DESTRA)
        } else {
            importo
        };
        Self {
            codice: (X_SINISTRA + RIENTRO, X_NUMERO - RIENTRO),
            descrizione: (X_NUMERO + RIENTRO, inizio - RIENTRO),
            importo,
            quote: colonne_quote,
        }
    }

    /// Filetti verticali interni, da sinistra.
    fn filetti(&self) -> Vec<i64> {
        let mut x = vec![X_NUMERO, self.importo.0];
        for q in &self.quote {
            x.push(q.euro.0);
            x.push(q.percentuale.0);
        }
        x
    }

    /// Ascisse dei numeri delle colonne numeriche (fine del testo).
    fn interno(x: (i64, i64)) -> (i64, i64) {
        (x.0 + RIENTRO, x.1 - RIENTRO)
    }
}

/// Valori di una riga: importo e, per ogni quota, euro e incidenza sull'importo.
fn valori(
    c: &Colonne,
    importo: Decimal,
    quote: &[Decimal],
    di: u32,
    grassetto: bool,
) -> Vec<Testo> {
    let testo = |x: (i64, i64), t: String| {
        let t = Testo::new(Colonne::interno(x), Allineamento::Destra, t);
        if grassetto { t.grassetto() } else { t }
    };
    let mut out = vec![testo(c.importo, numero(importo, di))];
    for (q, v) in c.quote.iter().zip(quote) {
        out.push(testo(q.euro, numero(*v, di)));
        if let Some(pc) = report::incidenza(*v, importo) {
            out.push(testo(q.percentuale, numero(pc, 2)));
        }
    }
    out
}

/// Riga con codice e descrizione (anche su più righe) e i valori sull'ultima.
fn riga(
    corpo: &mut Corpo,
    c: &Colonne,
    codice: &str,
    descrizione: &str,
    valori: Vec<Testo>,
    grassetto: bool,
) {
    let carattere = if grassetto {
        Carattere::TimesGrassetto
    } else {
        Carattere::Times
    };
    let codici = righe(codice, carattere, super::CORPO, c.codice.1 - c.codice.0);
    let testi = righe(
        descrizione,
        carattere,
        super::CORPO,
        c.descrizione.1 - c.descrizione.0,
    );
    let n = codici.len().max(testi.len()).max(1);
    let mut valori = Some(valori);
    for i in 0..n {
        let mut l = Linea::default();
        let stile = |t: Testo| if grassetto { t.grassetto() } else { t };
        if let Some(t) = codici.get(i) {
            l.testi.push(stile(Testo::new(
                c.codice,
                Allineamento::Sinistra,
                t.clone(),
            )));
        }
        if let Some(t) = testi.get(i) {
            l.testi.push(stile(Testo::new(
                c.descrizione,
                Allineamento::Sinistra,
                t.clone(),
            )));
        }
        if i + 1 == n
            && let Some(v) = valori.take()
        {
            l.testi.extend(v);
        }
        corpo.linea(l);
    }
}

/// Riga di totale in grassetto, con il filetto sopra i valori (dopo una riga vuota,
/// perché il testo della riga precedente scende nella parte alta della successiva).
fn totale(corpo: &mut Corpo, c: &Colonne, etichetta: &str, valori: Vec<Testo>) {
    corpo.vuota();
    let mut testi = vec![Testo::new(c.descrizione, Allineamento::Destra, etichetta).grassetto()];
    testi.extend(valori);
    corpo.linea(Linea::con(testi).filetto(
        (c.importo.0 + RIENTRO, X_DESTRA - RIENTRO),
        SOTTILE,
        false,
    ));
}

/// Testo su più righe nella colonna della descrizione.
fn nota(corpo: &mut Corpo, c: &Colonne, testo: &str) {
    for r in righe(
        testo,
        Carattere::Times,
        super::CORPO,
        c.descrizione.1 - c.descrizione.0,
    ) {
        corpo.linea(Linea::con(vec![Testo::new(
            c.descrizione,
            Allineamento::Sinistra,
            r,
        )]));
    }
}

/// Aggiunge al corpo il riepilogo con le quote indicate e restituisce le colonne con
/// cui disegnare le pagine.
///
/// # Errors
/// Struttura WBS non valida.
pub(super) fn riepilogo(
    corpo: &mut Corpo,
    p: &Progetto,
    r: &RiepilogoComputo,
    quote: &[Quota],
    regole: &RegoleArrotondamento,
) -> Result<Colonne, ErroreReport> {
    let c = Colonne::new(quote);
    let di = regole.decimali_importo;
    let totali_quote: Vec<Decimal> = quote.iter().map(|q| q.totale(r)).collect();
    let totale_computo = r.totale.valore();

    corpo.spazio();
    corpo.linea(Linea::con(vec![
        Testo::new(
            (X_NUMERO, c.importo.0),
            Allineamento::Centro,
            "R I E P I L O G O",
        )
        .grassetto()
        .sottolineato(),
    ]));
    corpo.vuota();

    for gruppo in report::riepiloghi(p, r, quote)? {
        corpo.linea(Linea::con(vec![
            Testo::new(
                c.descrizione,
                Allineamento::Sinistra,
                gruppo.titolo.to_uppercase(),
            )
            .grassetto()
            .sottolineato(),
        ]));
        for g in &gruppo.righe {
            let descrizione = format!("{}{}", "   ".repeat(g.livello), g.descrizione);
            riga(
                corpo,
                &c,
                &g.codice,
                &descrizione,
                valori(&c, g.importo, &g.quote, di, g.evidenza),
                g.evidenza,
            );
        }
        totale(
            corpo,
            &c,
            "Totale",
            valori(&c, totale_computo, &totali_quote, di, true),
        );
        if let Some(n) = gruppo.nota {
            nota(corpo, &c, n);
        }
        corpo.vuota();
    }

    corpo.linea(Linea::con(vec![
        Testo::new(c.descrizione, Allineamento::Sinistra, "QUADRO GENERALE")
            .grassetto()
            .sottolineato(),
    ]));
    let senza_quote = |v: Decimal| {
        vec![Testo::new(
            Colonne::interno(c.importo),
            Allineamento::Destra,
            numero(v, di),
        )]
    };
    riga(
        corpo,
        &c,
        "",
        "Lavori",
        senza_quote(r.totale_lavori.valore()),
        false,
    );
    riga(
        corpo,
        &c,
        "",
        "Oneri della sicurezza",
        senza_quote(r.totale_oneri_sicurezza.valore()),
        false,
    );
    totale(
        corpo,
        &c,
        "T O T A L E   euro",
        valori(&c, totale_computo, &totali_quote, di, true),
    );
    corpo.vuota();
    if !quote.is_empty() {
        nota(
            corpo,
            &c,
            "Quote calcolate sull'importo di ogni voce con l'incidenza del suo prezzo; \
             incidenze dei gruppi sul loro importo.",
        );
    }
    for q in quote {
        if let Some(n) = q.nota(r) {
            nota(corpo, &c, &n);
        }
    }
    corpo.vuota();
    Ok(c)
}

/// Disegna una pagina del riepilogo.
pub(super) fn pagina(
    tela: &mut Tela,
    int: &Intestazione,
    numero_pagina: usize,
    pc: &PaginaCorpo,
    c: &Colonne,
) {
    let alto = int.alto_riquadro();
    int.cornice(tela, numero_pagina, alto);
    let verde = Colore::Verde;
    let g = Carattere::Times;
    let corpo = super::CORPO;
    let basso = alto + ALTEZZA_TESTATA;

    // Intestazione delle colonne.
    let mut spessi = vec![X_NUMERO, c.importo.0];
    spessi.extend(c.quote.iter().map(|q| q.euro.0));
    for x in spessi {
        tela.verticale(x, alto, basso, SPESSO, verde);
    }
    for q in &c.quote {
        tela.verticale(q.percentuale.0, alto + 1_928, basso, SOTTILE, verde);
        tela.orizzontale(q.euro.0, q.percentuale.1, alto + 1_758, SOTTILE, verde);
        tela.orizzontale(q.euro.0, q.percentuale.1, alto + 1_928, SOTTILE, verde);
        let titolo = match q.quota {
            Quota::Manodopera => "M A N O D O P E R A",
            Quota::Sicurezza => "S I C U R E Z Z A",
        };
        tela.testo_centro(
            (q.euro.0, q.percentuale.1),
            alto + 626,
            titolo,
            g,
            corpo,
            verde,
        );
        tela.testo_centro(q.euro, alto + 2_553, "euro", g, corpo, verde);
        tela.testo_centro(q.percentuale, alto + 2_553, "incid. %", g, corpo, verde);
    }
    tela.orizzontale(X_SINISTRA, X_DESTRA, basso, SPESSO, verde);
    tela.testo_centro(
        (X_SINISTRA, X_NUMERO),
        alto + 1_590,
        "Codice",
        g,
        corpo,
        verde,
    );
    tela.testo_centro(
        (X_NUMERO, c.importo.0),
        alto + 1_590,
        "D E S C R I Z I O N E",
        g,
        corpo,
        verde,
    );
    tela.testo_centro(c.importo, alto + 1_108, "I M P O R T O", g, corpo, verde);
    tela.testo_centro(c.importo, alto + 2_072, "euro", g, corpo, verde);

    // Corpo.
    for x in c.filetti() {
        tela.verticale(x, basso, Y_FONDO, SOTTILE, verde);
    }
    disegna_linee(tela, &pc.linee, basso + STACCO, SCARTO);
}
