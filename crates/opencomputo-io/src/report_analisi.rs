//! Report di analisi prezzi, fabbisogno di cantiere, elenco e verbali dei nuovi prezzi.
//!
//! Come per gli altri report, i valori vengono dal core (analisi calcolate con i prezzi
//! correnti, fabbisogno espanso dalle sotto-analisi): qui sono solo impaginati.

use std::collections::BTreeMap;

use opencomputo_core::analisi::TipoRisorsa;
use opencomputo_core::id::ComputoId;
use opencomputo_core::prezzario::VocePrezzario;
use opencomputo_core::progetto::Progetto;

use crate::documento::{Colonna, Documento, Sezione, StileRiga, Tabella, Valore};
use crate::formato;
use crate::report::{ErroreReport, OpzioniReport, documento, errore_calcolo};

/// Nome del tipo di risorsa al plurale, per stampe e titoli.
#[must_use]
pub fn nome_tipo(t: TipoRisorsa) -> &'static str {
    match t {
        TipoRisorsa::Manodopera => "Manodopera",
        TipoRisorsa::Materiale => "Materiali",
        TipoRisorsa::Nolo => "Noli",
        TipoRisorsa::Trasporto => "Trasporti",
        TipoRisorsa::Altro => "Altro",
    }
}

fn incidenze(i: &BTreeMap<TipoRisorsa, opencomputo_core::numeri::Percentuale>) -> String {
    i.iter()
        .filter(|(_, p)| !p.valore().is_zero())
        .map(|(t, p)| {
            format!(
                "{} {} %",
                nome_tipo(*t).to_lowercase(),
                formato::numero(p.valore(), 2)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Voci analizzate dell'elenco, eventualmente solo quella indicata.
fn voci_analizzate<'a>(
    p: &'a Progetto,
    voce: Option<&str>,
) -> Result<Vec<&'a VocePrezzario>, ErroreReport> {
    let voci: Vec<&VocePrezzario> = p
        .elenco_prezzi
        .voci
        .iter()
        .filter(|v| v.analisi.is_some() && voce.is_none_or(|c| v.codice == c))
        .collect();
    if voci.is_empty() {
        return Err(ErroreReport::Calcolo(match voce {
            Some(c) => format!("la voce «{c}» non ha un'analisi"),
            None => "nessuna voce dell'elenco ha un'analisi".into(),
        }));
    }
    Ok(voci)
}

/// Sezione con l'analisi completa di una voce.
fn sezione_analisi(p: &Progetto, v: &VocePrezzario) -> Result<Sezione, ErroreReport> {
    let regole = &p.regole;
    let (dq, dp, di) = (
        regole.decimali_quantita.max(2),
        regole.decimali_prezzo,
        regole.decimali_importo,
    );
    let e = &p.elenco_prezzi;
    let id = v
        .analisi
        .ok_or_else(|| ErroreReport::Calcolo(format!("«{}» senza analisi", v.codice)))?;
    let a = e
        .analisi(id)
        .ok_or_else(|| ErroreReport::Calcolo(format!("analisi di «{}» inesistente", v.codice)))?;
    let r = e.calcola_analisi(id, regole).map_err(errore_calcolo)?;
    let mut t = Tabella::new(vec![
        Colonna::testo("Codice", 14),
        Colonna::testo("Elemento", 44),
        Colonna::centrata("U.M.", 7),
        Colonna::numero("Quantità", 11),
        Colonna::numero("Prezzo €", 11),
        Colonna::numero("Importo €", 12),
    ]);
    let mut per_tipo: BTreeMap<TipoRisorsa, Vec<usize>> = BTreeMap::new();
    for (i, riga) in a.righe.iter().enumerate() {
        per_tipo.entry(riga.tipo).or_default().push(i);
    }
    for (tipo, indici) in &per_tipo {
        t.riga_testo(StileRiga::Gruppo, 1, nome_tipo(*tipo));
        for i in indici {
            let (Some(riga), Some(c)) = (a.righe.get(*i), r.righe.get(*i)) else {
                continue;
            };
            t.riga(
                StileRiga::Normale,
                vec![
                    Valore::testo(riga.codice.clone()),
                    Valore::testo(riga.descrizione.clone()),
                    Valore::testo(riga.unita_misura.clone()),
                    Valore::numero(c.quantita.valore(), dq),
                    Valore::numero(c.prezzo.valore(), dp),
                    Valore::numero(c.importo.valore(), di),
                ],
            );
        }
    }
    let riga = |t: &mut Tabella, stile, testo: String, v: rust_decimal::Decimal| {
        t.riga(
            stile,
            vec![
                Valore::Vuoto,
                Valore::testo(testo),
                Valore::Vuoto,
                Valore::Vuoto,
                Valore::Vuoto,
                Valore::numero(v, di),
            ],
        );
    };
    riga(
        &mut t,
        StileRiga::Sommano,
        "Costo diretto".into(),
        r.costo_diretto.valore(),
    );
    riga(
        &mut t,
        StileRiga::Normale,
        format!(
            "Spese generali {} %",
            formato::numero(a.spese_generali.valore(), 2)
        ),
        r.spese_generali.valore(),
    );
    if let (Some(q), Some(o)) = (a.quota_oneri_sicurezza, r.oneri_sicurezza_aziendali) {
        riga(
            &mut t,
            StileRiga::Normale,
            format!(
                "— di cui oneri aziendali della sicurezza ({} % delle spese generali)",
                formato::numero(q.valore(), 2)
            ),
            o.valore(),
        );
    }
    riga(
        &mut t,
        StileRiga::Normale,
        format!(
            "Utile d'impresa {} %{}",
            formato::numero(a.utile_impresa.valore(), 2),
            match a.base_utile {
                opencomputo_core::analisi::BaseUtile::CostoPiuSpeseGenerali => {
                    " (su costo e spese generali)"
                }
                opencomputo_core::analisi::BaseUtile::SoloCosto => " (sul solo costo)",
            }
        ),
        r.utile.valore(),
    );
    let quantita = a.quantita_riferimento.valore();
    riga(
        &mut t,
        StileRiga::Sommano,
        format!(
            "Totale per {} {}",
            formato::numero(quantita, if quantita.fract().is_zero() { 0 } else { dq }),
            v.unita_misura
        ),
        r.totale.valore(),
    );
    t.riga(
        StileRiga::Totale,
        vec![
            Valore::Vuoto,
            Valore::testo(format!("PREZZO UNITARIO ({})", v.unita_misura)),
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::numero(r.prezzo_unitario.valore(), dp),
        ],
    );
    let mut note = Vec::new();
    if !a.variabili.is_empty() {
        let elenco: Vec<String> = a
            .variabili
            .iter()
            .zip(&r.variabili)
            .map(|(v, x)| {
                let valore = formato::numero(x.valore.normalize(), x.valore.normalize().scale());
                let nome = if v.descrizione.is_empty() {
                    v.nome.clone()
                } else {
                    format!("{} ({})", v.nome, v.descrizione)
                };
                if v.espressione.replace(',', ".") == x.valore.normalize().to_string() {
                    format!("{nome} = {valore}")
                } else {
                    format!("{nome} = {} = {valore}", v.espressione)
                }
            })
            .collect();
        note.push(format!("Variabili: {}.", elenco.join("; ")));
    }
    let inc = incidenze(&r.incidenze);
    if !inc.is_empty() {
        note.push(format!("Incidenze sul prezzo: {inc}."));
    }
    if r.prezzo_unitario != v.prezzo {
        note.push(format!(
            "Il prezzo in elenco ({} €) non è ancora aggiornato all'analisi.",
            formato::numero(v.prezzo.valore(), dp)
        ));
    }
    let titolo = match &v.nuovo_prezzo {
        Some(n) => format!("{} {} — {}", n.sigla(), v.codice, v.descrizione_breve),
        None => format!("{} — {}", v.codice, v.descrizione_breve),
    };
    Ok(Sezione {
        titolo,
        note,
        tabella: t,
    })
}

/// Analisi dei prezzi: completa (una sezione per voce con le righe) o sintetica (una
/// tabella con costo, spese generali, utile, prezzo e incidenze).
///
/// # Errors
/// Nessuna voce analizzata (o la voce indicata non lo è), analisi non calcolabili.
pub fn analisi_prezzi(
    p: &Progetto,
    voce: Option<&str>,
    sintetica: bool,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let voci = voci_analizzate(p, voce)?;
    let mut doc = documento(
        p,
        if sintetica {
            "Analisi dei prezzi — quadro sintetico"
        } else {
            "Analisi dei prezzi"
        },
        sintetica,
        o,
    );
    if !sintetica {
        for v in voci {
            doc.sezioni.push(sezione_analisi(p, v)?);
        }
        return Ok(doc);
    }
    let regole = &p.regole;
    let (dp, di) = (regole.decimali_prezzo, regole.decimali_importo);
    let mut t = Tabella::new(vec![
        Colonna::testo("Codice", 14),
        Colonna::testo("Descrizione", 40),
        Colonna::centrata("U.M.", 7),
        Colonna::numero("Costo diretto €", 13),
        Colonna::numero("Spese generali €", 13),
        Colonna::numero("Utile €", 11),
        Colonna::numero("Prezzo €", 11),
        Colonna::numero("Manodopera %", 10),
        Colonna::numero("Materiali %", 10),
        Colonna::numero("Noli %", 9),
    ]);
    for v in voci {
        let Some(id) = v.analisi else { continue };
        let r = p
            .elenco_prezzi
            .calcola_analisi(id, regole)
            .map_err(errore_calcolo)?;
        let inc = |t| Valore::numero_opz(r.incidenze.get(&t).map(|x| x.valore()), 2);
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::testo(match &v.nuovo_prezzo {
                    Some(n) => format!("{} {}", n.sigla(), v.codice),
                    None => v.codice.clone(),
                }),
                Valore::testo(v.descrizione_breve.clone()),
                Valore::testo(v.unita_misura.clone()),
                Valore::numero(r.costo_diretto.valore(), di),
                Valore::numero(r.spese_generali.valore(), di),
                Valore::numero(r.utile.valore(), di),
                Valore::numero(r.prezzo_unitario.valore(), dp),
                inc(TipoRisorsa::Manodopera),
                inc(TipoRisorsa::Materiale),
                inc(TipoRisorsa::Nolo),
            ],
        );
    }
    doc.sezioni.push(Sezione {
        titolo: "Voci analizzate".into(),
        note: vec![
            "Costi e prezzi per la quantità di riferimento di ciascuna analisi, con i prezzi \
             correnti delle risorse; le incidenze comprendono le sotto-analisi."
                .into(),
        ],
        tabella: t,
    });
    Ok(doc)
}

/// Fabbisogno di cantiere di un computo, per tipo di risorsa.
///
/// # Errors
/// Computo inesistente o dati non calcolabili.
pub fn fabbisogno(
    p: &Progetto,
    computo: ComputoId,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let f = p.fabbisogno(computo).map_err(errore_calcolo)?;
    let nome = p
        .computo(computo)
        .map(|c| c.nome.clone())
        .unwrap_or_default();
    let regole = &p.regole;
    let (dq, dp, di) = (
        regole.decimali_quantita,
        regole.decimali_prezzo,
        regole.decimali_importo,
    );
    let mut doc = documento(p, "Fabbisogno di cantiere", false, o);
    doc.intestazione.push(("Computo".into(), nome));
    let mut t = Tabella::new(vec![
        Colonna::testo("Codice", 14),
        Colonna::testo("Risorsa", 44),
        Colonna::centrata("U.M.", 7),
        Colonna::numero("Quantità", 12),
        Colonna::numero("Prezzo €", 11),
        Colonna::numero("Importo €", 13),
    ]);
    let mut tipo_corrente = None;
    for r in &f.risorse {
        if tipo_corrente != Some(r.tipo) {
            if let Some(prec) = tipo_corrente {
                let totale = f.totali_per_tipo.get(&prec).map(|x| x.valore());
                t.riga(
                    StileRiga::Sommano,
                    vec![
                        Valore::Vuoto,
                        Valore::testo(format!("Totale {}", nome_tipo(prec).to_lowercase())),
                        Valore::Vuoto,
                        Valore::Vuoto,
                        Valore::Vuoto,
                        Valore::numero_opz(totale, di),
                    ],
                );
            }
            t.riga_testo(StileRiga::Gruppo, 1, nome_tipo(r.tipo));
            tipo_corrente = Some(r.tipo);
        }
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::testo(r.codice.clone()),
                Valore::testo(r.descrizione.clone()),
                Valore::testo(r.unita_misura.clone()),
                Valore::numero(r.quantita.valore(), dq),
                Valore::numero(r.prezzo.valore(), dp),
                Valore::numero(r.importo.valore(), di),
            ],
        );
    }
    if let Some(prec) = tipo_corrente {
        let totale = f.totali_per_tipo.get(&prec).map(|x| x.valore());
        t.riga(
            StileRiga::Sommano,
            vec![
                Valore::Vuoto,
                Valore::testo(format!("Totale {}", nome_tipo(prec).to_lowercase())),
                Valore::Vuoto,
                Valore::Vuoto,
                Valore::Vuoto,
                Valore::numero_opz(totale, di),
            ],
        );
    }
    t.riga(
        StileRiga::Totale,
        vec![
            Valore::Vuoto,
            Valore::testo("TOTALE RISORSE"),
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::numero(f.totale.valore(), di),
        ],
    );
    let mut note = vec![
        "Quantità ricavate dalle analisi delle voci del computo (sotto-analisi comprese), ai \
         prezzi correnti delle risorse, senza spese generali né utile."
            .to_owned(),
    ];
    if !f.voci_senza_analisi.is_empty() {
        note.push(format!(
            "Voci senza analisi, non comprese: {}.",
            f.voci_senza_analisi.join(", ")
        ));
    }
    doc.sezioni.push(Sezione {
        titolo: "Risorse".into(),
        note,
        tabella: t,
    });
    Ok(doc)
}

/// Elenco dei nuovi prezzi o, con `verbale`, verbale di concordamento con le firme e le
/// analisi dei prezzi che ne derivano.
///
/// # Errors
/// Nessun nuovo prezzo, verbale inesistente o analisi non calcolabili.
#[allow(clippy::too_many_lines)]
pub fn nuovi_prezzi(
    p: &Progetto,
    verbale: Option<u32>,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let regole = &p.regole;
    let dp = regole.decimali_prezzo;
    let c = &p.contabilita;
    let mut voci: Vec<&VocePrezzario> = p
        .elenco_prezzi
        .voci
        .iter()
        .filter(|v| {
            v.nuovo_prezzo
                .as_ref()
                .is_some_and(|n| verbale.is_none_or(|x| n.verbale == Some(x)))
        })
        .collect();
    voci.sort_by_key(|v| v.nuovo_prezzo.as_ref().map_or(0, |n| n.numero));
    let si_no = |b: bool| if b { "sì" } else { "no" };
    let Some(numero) = verbale else {
        if voci.is_empty() {
            return Err(ErroreReport::Calcolo("nessun nuovo prezzo".into()));
        }
        let mut doc = documento(p, "Elenco dei nuovi prezzi", true, o);
        let mut t = Tabella::new(vec![
            Colonna::centrata("N.", 7),
            Colonna::testo("Codice", 13),
            Colonna::testo("Descrizione", 40),
            Colonna::centrata("U.M.", 7),
            Colonna::numero("Prezzo €", 11),
            Colonna::testo("Criterio", 22),
            Colonna::centrata("Ribasso", 8),
            Colonna::testo("Verbale", 18),
        ]);
        for v in &voci {
            let Some(n) = &v.nuovo_prezzo else { continue };
            let stato = match n.verbale.and_then(|x| c.verbale_nuovi_prezzi(x)) {
                Some(vb) if vb.approvato() => format!("n. {} approvato", vb.numero),
                Some(vb) => format!("n. {} da approvare", vb.numero),
                None => "—".into(),
            };
            t.riga(
                StileRiga::Normale,
                vec![
                    Valore::testo(n.sigla()),
                    Valore::testo(v.codice.clone()),
                    Valore::testo(v.descrizione_breve.clone()),
                    Valore::testo(v.unita_misura.clone()),
                    Valore::numero(v.prezzo.valore(), dp),
                    Valore::testo(n.criterio.descrizione()),
                    Valore::testo(si_no(n.soggetto_a_ribasso)),
                    Valore::testo(stato),
                ],
            );
        }
        doc.sezioni.push(Sezione {
            titolo: "Nuovi prezzi".into(),
            note: vec![
                "Prezzi di lavorazioni non previste nel contratto (Allegato II.14 al D.Lgs. \
                 36/2023, art. 5). «Ribasso: no» indica un prezzo già netto, escluso dal \
                 ribasso d'asta nella contabilità."
                    .into(),
            ],
            tabella: t,
        });
        return Ok(doc);
    };

    let vb = c
        .verbale_nuovi_prezzi(numero)
        .ok_or_else(|| ErroreReport::Calcolo(format!("verbale n. {numero} inesistente")))?;
    let mut doc = documento(
        p,
        &format!("Verbale di concordamento nuovi prezzi n. {numero}"),
        false,
        o,
    );
    doc.intestazione
        .push(("Data del verbale".into(), formato::data(vb.data)));
    let mut t = Tabella::new(vec![
        Colonna::centrata("N.", 7),
        Colonna::testo("Codice", 12),
        Colonna::testo("Descrizione", 52),
        Colonna::centrata("U.M.", 7),
        Colonna::numero("Prezzo €", 11),
        Colonna::centrata("Ribasso", 8),
    ]);
    for v in &voci {
        let Some(n) = &v.nuovo_prezzo else { continue };
        let descrizione = if v.descrizione_estesa.trim().is_empty() {
            v.descrizione_breve.clone()
        } else {
            v.descrizione_estesa.clone()
        };
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::testo(n.sigla()),
                Valore::testo(v.codice.clone()),
                Valore::testo(format!(
                    "{descrizione}\n[{}{}]",
                    n.criterio.descrizione(),
                    if n.motivo.trim().is_empty() {
                        String::new()
                    } else {
                        format!("; {}", n.motivo.trim())
                    }
                )),
                Valore::testo(v.unita_misura.clone()),
                Valore::numero(v.prezzo.valore(), dp),
                Valore::testo(si_no(n.soggetto_a_ribasso)),
            ],
        );
    }
    let m = &p.metadati;
    let mut premessa = vec![format!(
        "Il giorno {} il direttore dei lavori{} e l'esecutore{} concordano i seguenti nuovi \
         prezzi per lavorazioni non previste nel contratto, ai sensi dell'Allegato II.14 al \
         D.Lgs. 36/2023, art. 5.",
        formato::data(vb.data),
        m.direttore_lavori
            .as_deref()
            .map(|x| format!(" {x}"))
            .unwrap_or_default(),
        m.impresa
            .as_deref()
            .map(|x| format!(" {x}"))
            .unwrap_or_default(),
    )];
    if voci.iter().any(|v| {
        v.nuovo_prezzo
            .as_ref()
            .is_some_and(|n| n.soggetto_a_ribasso)
    }) {
        premessa.push(
            "I prezzi con «Ribasso: sì» sono soggetti al ribasso d'asta contrattuale.".into(),
        );
    }
    if let Some(n) = &vb.note {
        premessa.push(n.clone());
    }
    doc.sezioni.push(Sezione {
        titolo: "Nuovi prezzi concordati".into(),
        note: premessa,
        tabella: t,
    });

    let mut firme = Tabella::new(vec![
        Colonna::testo("", 28),
        Colonna::testo("Nome", 32),
        Colonna::testo("Data e firma", 30),
    ]);
    firme.riga(
        StileRiga::Normale,
        vec![
            Valore::testo("Il direttore dei lavori"),
            Valore::testo(m.direttore_lavori.clone().unwrap_or_default()),
            Valore::testo(formato::data(vb.data)),
        ],
    );
    firme.riga(
        StileRiga::Normale,
        vec![
            Valore::testo("L'esecutore"),
            Valore::testo(m.impresa.clone().unwrap_or_default()),
            Valore::testo(vb.firma_esecutore.map(formato::data).unwrap_or_default()),
        ],
    );
    firme.riga(
        StileRiga::Normale,
        vec![
            Valore::testo("Visto, si approva: il RUP"),
            Valore::testo(m.responsabile_procedimento.clone().unwrap_or_default()),
            Valore::testo(vb.approvato_il.map(formato::data).unwrap_or_default()),
        ],
    );
    let mut note = Vec::new();
    if let Some(r) = &vb.riserve {
        note.push(format!("Riserve dell'esecutore: {r}"));
    }
    if let Some(a) = &vb.atto_approvazione {
        note.push(format!("Atto di approvazione: {a}"));
    }
    doc.sezioni.push(Sezione {
        titolo: "Firme".into(),
        note,
        tabella: firme,
    });
    for v in voci.iter().filter(|v| v.analisi.is_some()) {
        let mut s = sezione_analisi(p, v)?;
        s.titolo = format!("Analisi del prezzo {}", s.titolo);
        doc.sezioni.push(s);
    }
    Ok(doc)
}

/// Computo metrico estimativo con l'analisi di ogni voce riportata alla quantità
/// computata (elementi, spese generali e utile in proporzione, differenza di
/// arrotondamento rispetto all'importo della voce).
///
/// # Errors
/// Computo inesistente o dati non calcolabili.
pub fn computo_con_analisi(
    p: &Progetto,
    computo: ComputoId,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let c = p
        .computo(computo)
        .ok_or_else(|| ErroreReport::ComputoInesistente(computo.to_string()))?;
    let r = p.calcola_computo(computo).map_err(errore_calcolo)?;
    let analisi = p.analisi_computo(computo).map_err(errore_calcolo)?;
    let regole = &p.regole;
    let (dq, dp, di) = (
        regole.decimali_quantita,
        regole.decimali_prezzo,
        regole.decimali_importo,
    );
    let mut doc = documento(p, &format!("{} con analisi dei prezzi", c.nome), true, o);
    let mut t = Tabella::new(vec![
        Colonna::centrata("N.", 6),
        Colonna::testo("Codice", 14),
        Colonna::testo("Descrizione ed elementi dell'analisi", 50),
        Colonna::centrata("U.M.", 7),
        Colonna::numero("Quantità", 12),
        Colonna::numero("Prezzo €", 11),
        Colonna::numero("Importo €", 13),
    ]);
    let vuoto = Valore::Vuoto;
    for ((voce, risultato), a) in c.voci.iter().zip(&r.voci).zip(&analisi) {
        t.riga(
            StileRiga::Voce,
            vec![
                Valore::testo(risultato.numero.to_string()),
                Valore::testo(voce.prezzo.codice.clone()),
                Valore::testo(voce.prezzo.descrizione_breve.clone()),
                Valore::testo(voce.prezzo.unita_misura.clone()),
                Valore::numero(risultato.calcolo.quantita.valore(), dq),
                Valore::numero(voce.prezzo.prezzo.valore(), dp),
                Valore::numero(risultato.calcolo.importo.valore(), di),
            ],
        );
        if !a.analizzata {
            t.riga_testo(StileRiga::Commento, 2, "voce senza analisi");
            continue;
        }
        for e in &a.elementi {
            t.riga(
                StileRiga::Rigo,
                vec![
                    vuoto.clone(),
                    Valore::testo(e.codice.clone()),
                    Valore::testo(format!(
                        "{} ({})",
                        e.descrizione,
                        nome_tipo(e.tipo).to_lowercase()
                    )),
                    Valore::testo(e.unita_misura.clone()),
                    Valore::numero(e.quantita.valore(), dq),
                    Valore::numero(e.prezzo.valore(), dp),
                    Valore::numero(e.importo.valore(), di),
                ],
            );
        }
        let mut riga = |testo: &str, v: rust_decimal::Decimal| {
            t.riga(
                StileRiga::Rigo,
                vec![
                    vuoto.clone(),
                    vuoto.clone(),
                    Valore::testo(testo),
                    vuoto.clone(),
                    vuoto.clone(),
                    vuoto.clone(),
                    Valore::numero(v, di),
                ],
            );
        };
        riga("Spese generali", a.spese_generali.valore());
        riga("Utile d'impresa", a.utile.valore());
        if !a.differenza.is_zero() {
            riga(
                "Differenza di arrotondamento o di prezzo",
                a.differenza.valore(),
            );
        }
    }
    for (testo, v) in [
        ("Totale lavori", r.totale_lavori),
        ("Totale oneri della sicurezza", r.totale_oneri_sicurezza),
        ("TOTALE", r.totale),
    ] {
        t.riga(
            StileRiga::Totale,
            vec![
                vuoto.clone(),
                vuoto.clone(),
                Valore::testo(testo),
                vuoto.clone(),
                vuoto.clone(),
                vuoto.clone(),
                Valore::numero(v.valore(), di),
            ],
        );
    }
    doc.sezioni.push(Sezione {
        titolo: "Computo con analisi".into(),
        note: vec![
            "Per ogni voce, gli elementi dell'analisi riportati alla quantità computata ai \
             prezzi correnti; la differenza comprende gli arrotondamenti e l'eventuale \
             scostamento tra il prezzo fotografato nel computo e quello dell'analisi."
                .into(),
        ],
        tabella: t,
    });
    Ok(doc)
}
