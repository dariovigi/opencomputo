//! Elenco prezzi nel modello tradizionale, con il prezzo in lettere.

use opencomputo_core::prezzario::VocePrezzario;
use opencomputo_core::progetto::Progetto;

use super::{
    Allineamento, Carattere, Colore, Corpo, Intestazione, Linea, OpzioniTradizionale, PaginaCorpo,
    SOTTILE, Tela, Testo, X_DESTRA, X_NUMERO, X_SINISTRA, Y_FONDO, capacita, disegna_linee,
    file_pdf, impagina, numero, righe, righe_paragrafi,
};
use crate::formato;
use crate::pdf::RisultatoPdf;

const X_DESCRIZIONE: i64 = 49_232;
const X_UNITA: i64 = 52_689;
const NUMERO: (i64, i64) = (
    X_SINISTRA + super::RIENTRO,
    X_SINISTRA + super::RIENTRO + super::LARGHEZZA_TARIFFA,
);
const DESCRIZIONE: (i64, i64) = (X_NUMERO + super::RIENTRO, X_DESCRIZIONE - super::RIENTRO);
const ALTEZZA_TESTATA: i64 = 3_855;
const SCARTO: i64 = 697;

fn voce(corpo: &mut Corpo, n: usize, v: &VocePrezzario, decimali: u32) {
    let mut sinistra = vec![format!("Nr. {n}")];
    sinistra.extend(righe(
        &v.codice,
        Carattere::Times,
        super::CORPO,
        super::LARGHEZZA_TARIFFA,
    ));
    let descrizione = if v.descrizione_estesa.trim().is_empty() {
        &v.descrizione_breve
    } else {
        &v.descrizione_estesa
    };
    let testo = righe_paragrafi(
        descrizione,
        Carattere::Times,
        super::CORPO,
        DESCRIZIONE.1 - DESCRIZIONE.0,
    );
    let prezzo = v.prezzo.valore();
    let riga_euro = testo.len();
    for i in 0..(riga_euro + 1).max(sinistra.len()) {
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
            l.testi.push(Testo::new(DESCRIZIONE, a, t.clone()));
        }
        if i == riga_euro {
            l.testi.push(
                Testo::new(
                    DESCRIZIONE,
                    Allineamento::Sinistra,
                    format!("euro ({})", formato::in_lettere(prezzo, decimali)),
                )
                .grassetto(),
            );
            l.testi.push(Testo::new(
                (X_DESCRIZIONE, X_UNITA),
                Allineamento::Centro,
                v.unita_misura.trim().to_owned(),
            ));
            l.testi.push(Testo::new(
                (X_UNITA, X_DESTRA - super::RIENTRO),
                Allineamento::Destra,
                numero(prezzo, decimali),
            ));
        }
        corpo.linea(l);
    }
    corpo.vuota();
}

fn capitolo(corpo: &mut Corpo, titolo: &str) {
    for r in righe(
        titolo,
        Carattere::TimesGrassetto,
        super::CORPO,
        DESCRIZIONE.1 - DESCRIZIONE.0,
    ) {
        corpo.linea(Linea::con(vec![
            Testo::new((X_NUMERO, X_DESCRIZIONE), Allineamento::Centro, r)
                .grassetto()
                .sottolineato(),
        ]));
    }
    corpo.vuota();
}

/// Elenco prezzi nel modello tradizionale: voci numerate per capitolo, descrizione
/// completa, prezzo in cifre e in lettere.
#[must_use]
pub fn elenco_prezzi(p: &Progetto, o: &OpzioniTradizionale) -> RisultatoPdf {
    let ep = &p.elenco_prezzi;
    let decimali = p.regole.decimali_importo;
    let mut corpo = Corpo::default();
    corpo.spazio();
    let mut n = 0usize;
    for cap in &ep.capitoli {
        let mut voci = ep
            .voci
            .iter()
            .filter(|v| v.capitolo == Some(cap.id))
            .peekable();
        if voci.peek().is_none() {
            continue;
        }
        let titolo = if cap.codice.trim().is_empty() {
            cap.descrizione.trim().to_owned()
        } else {
            format!("{} - {}", cap.codice.trim(), cap.descrizione.trim())
        };
        capitolo(&mut corpo, &titolo);
        for v in voci {
            n += 1;
            voce(&mut corpo, n, v, decimali);
        }
    }
    let senza: Vec<&VocePrezzario> = ep
        .voci
        .iter()
        .filter(|v| {
            v.capitolo
                .is_none_or(|c| !ep.capitoli.iter().any(|x| x.id == c))
        })
        .collect();
    if !senza.is_empty() && n > 0 {
        capitolo(&mut corpo, "Voci senza capitolo");
    }
    for v in senza {
        n += 1;
        voce(&mut corpo, n, v, decimali);
    }

    let int = Intestazione::new(p, o);
    let spazi = super::larghezza("     ", Carattere::Times, super::CORPO);
    corpo.linea(Linea::con(vec![Testo::new(
        (DESCRIZIONE.0 + spazi, X_DESTRA),
        Allineamento::Sinistra,
        int.luogo_data.clone(),
    )]));
    corpo.vuota();
    let centro = (X_NUMERO, X_DESCRIZIONE);
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

    let alto = int.alto_riquadro();
    let inizio = alto + ALTEZZA_TESTATA;
    let cap = capacita(inizio, SCARTO, Y_FONDO);
    let tratteggio = Testo::new(centro, Allineamento::Centro, "-".repeat(78));
    let pagine = impagina(corpo.elementi, cap, &tratteggio);

    let titolo = "ELENCO PREZZI";
    let mut tele = Vec::new();
    if o.frontespizio {
        let mut t = Tela::default();
        int.frontespizio(&mut t, titolo);
        tele.push(t);
    }
    let primo = tele.len() + 1;
    for (i, pc) in pagine.iter().enumerate() {
        let mut t = Tela::default();
        pagina(&mut t, &int, primo + i, pc);
        tele.push(t);
    }
    file_pdf(tele, titolo, o.comprimi)
}

fn pagina(tela: &mut Tela, int: &Intestazione, numero_pagina: usize, pc: &PaginaCorpo) {
    let alto = int.alto_riquadro();
    int.cornice(tela, numero_pagina, alto);
    let verde = Colore::Verde;
    let g = Carattere::Times;
    let corpo = super::CORPO;
    for x in [X_NUMERO, X_DESCRIZIONE, X_UNITA] {
        tela.verticale(x, alto, Y_FONDO, SOTTILE, verde);
    }
    let basso = alto + ALTEZZA_TESTATA;
    tela.orizzontale(X_SINISTRA, X_DESTRA, basso - 170, SOTTILE, verde);
    tela.orizzontale(X_SINISTRA, X_DESTRA, basso, SOTTILE, verde);
    let a = (X_SINISTRA, X_NUMERO);
    tela.testo_centro(a, alto + 1_179, "Num.Ord.", g, corpo, verde);
    tela.testo_centro(a, alto + 2_143, "TARIFFA", g, corpo, verde);
    tela.testo_centro(
        (X_NUMERO, X_DESCRIZIONE),
        alto + 1_661,
        "D E S C R I Z I O N E   D E L L' A R T I C O L O",
        g,
        corpo,
        verde,
    );
    let u = (X_DESCRIZIONE, X_UNITA);
    tela.testo_centro(u, alto + 697, "unità", g, corpo, verde);
    tela.testo_centro(u, alto + 1_661, "di", g, corpo, verde);
    tela.testo_centro(u, alto + 2_625, "misura", g, corpo, verde);
    let pz = (X_UNITA, X_DESTRA);
    tela.testo_centro(pz, alto + 1_179, "P R E Z Z O", g, corpo, verde);
    tela.testo_centro(pz, alto + 2_143, "UNITARIO", g, corpo, verde);
    disegna_linee(tela, &pc.linee, basso, SCARTO);
}
