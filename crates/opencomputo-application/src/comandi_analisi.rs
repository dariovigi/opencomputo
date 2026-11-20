//! Applicazione dei comandi di risorse, analisi prezzi, nuovi prezzi e verbali di
//! concordamento (separati da `comandi` per leggibilità).

use opencomputo_core::analisi::{
    AnalisiPrezzo, OrigineRigaAnalisi, RigaAnalisi, RisorsaElementare, TipoRisorsa,
};
use opencomputo_core::contabilita::{StatoSal, VerbaleNuoviPrezzi};
use opencomputo_core::id::GeneratoreId;
use opencomputo_core::misura::Fattore;
use opencomputo_core::numeri::Quantita;
use opencomputo_core::prezzario::{CriterioNuovoPrezzo, NuovoPrezzo};
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;
use std::sync::Arc;

use crate::comandi::{
    Comando, FattoreIngresso, OggettoAggiornamento, RigaAnalisiIngresso, non_trovato, voce_elenco,
};
use crate::formato_importo;
use opencomputo_core::analisi::VariabileAnalisi;

fn valuta(f: &FattoreIngresso) -> Result<(Decimal, Option<String>), String> {
    match f.in_fattore() {
        Fattore::Valore(v) => Ok((v, None)),
        Fattore::Espressione(e) => {
            let v = Fattore::Espressione(e.clone())
                .valuta()
                .map_err(|x| format!("quantità «{e}»: {x}"))?;
            Ok((v, Some(e)))
        }
    }
}

/// Quantità in ingresso: un numero, oppure un'espressione valutata poi con le variabili
/// dell'analisi (il valore provvisorio è zero).
fn quantita_ingresso(f: &FattoreIngresso) -> (Decimal, Option<String>) {
    match f.in_fattore() {
        Fattore::Valore(v) => (v, None),
        Fattore::Espressione(e) => (Decimal::ZERO, Some(e)),
    }
}

fn riga(p: &Progetto, r: &RigaAnalisiIngresso, n: usize) -> Result<RigaAnalisi, String> {
    let (quantita, espressione) = quantita_ingresso(&r.quantita);
    let e = &p.elenco_prezzi;
    let mut riga = match (&r.risorsa, &r.voce) {
        (Some(_), Some(_)) => return Err(format!("riga {n}: indicare risorsa oppure voce")),
        (Some(c), None) => {
            let x = e
                .risorse
                .iter()
                .find(|x| &x.codice == c)
                .ok_or_else(|| format!("riga {n}: {}", non_trovato("risorsa", c)))?;
            RigaAnalisi::da_risorsa(x, Quantita::new(quantita))
        }
        (None, Some(c)) => {
            let v = voce_elenco(p, c).map_err(|x| format!("riga {n}: {x}"))?;
            RigaAnalisi {
                codice: v.codice.clone(),
                descrizione: v.descrizione_breve.clone(),
                unita_misura: v.unita_misura.clone(),
                tipo: r.tipo.unwrap_or(TipoRisorsa::Materiale),
                prezzo: v.prezzo,
                quantita: Quantita::new(quantita),
                espressione: None,
                origine: Some(OrigineRigaAnalisi::VocePrezzario(v.id)),
            }
        }
        (None, None) => RigaAnalisi {
            codice: r.codice.clone().unwrap_or_default(),
            descrizione: r
                .descrizione
                .clone()
                .ok_or_else(|| format!("riga {n}: indicare risorsa, voce o descrizione"))?,
            unita_misura: r
                .unita_misura
                .clone()
                .ok_or_else(|| format!("riga {n}: indicare l'unità di misura"))?,
            tipo: r.tipo.unwrap_or(TipoRisorsa::Materiale),
            prezzo: r
                .prezzo
                .ok_or_else(|| format!("riga {n}: indicare il prezzo"))?,
            quantita: Quantita::new(quantita),
            espressione: None,
            origine: None,
        },
    };
    if let Some(d) = r
        .descrizione
        .as_ref()
        .filter(|_| r.risorsa.is_some() || r.voce.is_some())
    {
        riga.descrizione = d.clone();
    }
    if let Some(t) = r.tipo.filter(|_| r.risorsa.is_some()) {
        riga.tipo = t;
    }
    riga.espressione = espressione;
    Ok(riga)
}

/// Allinea l'esclusione dal ribasso delle registrazioni modificabili di una voce.
fn allinea_registrazioni(p: &mut Progetto, voce: opencomputo_core::id::VocePrezzarioId) -> usize {
    let elenco = p.elenco_prezzi.id;
    let Some(v) = p.elenco_prezzi.voce(voce) else {
        return 0;
    };
    let escluso = v
        .nuovo_prezzo
        .as_ref()
        .is_some_and(|n| !n.soggetto_a_ribasso);
    let codice = v.codice.clone();
    let da_allineare: Vec<_> = p
        .contabilita
        .libretto()
        .registrazioni
        .iter()
        .filter(|r| {
            r.prezzo.escluso_dal_ribasso != escluso
                && r.prezzo.origine.map_or(r.prezzo.codice == codice, |o| {
                    o.prezzario == elenco && o.voce == voce
                })
                && !p
                    .contabilita
                    .sal_di(r.id)
                    .is_some_and(|s| s.stato == StatoSal::Emesso)
        })
        .map(|r| r.id)
        .collect();
    da_allineare
        .into_iter()
        .filter(|id| {
            p.contabilita
                .modifica_registrazione(*id, |r| r.prezzo.escluso_dal_ribasso = escluso)
                .is_ok()
        })
        .count()
}

#[allow(clippy::too_many_lines)]
pub(crate) fn applica<G: GeneratoreId>(
    p: &mut Progetto,
    comando: &Comando,
    g: &mut G,
) -> Result<String, String> {
    match comando {
        Comando::ImpostaRisorsa {
            codice,
            nuovo_codice,
            descrizione,
            unita_misura,
            tipo,
            prezzo,
        } => {
            let e = &mut p.elenco_prezzi;
            if let Some(n) = nuovo_codice.as_ref().filter(|n| *n != codice)
                && e.risorse.iter().any(|x| &x.codice == n)
            {
                return Err(format!("risorsa «{n}» già presente"));
            }
            match e.risorse.iter_mut().find(|x| &x.codice == codice) {
                Some(x) => {
                    if let Some(n) = nuovo_codice {
                        x.codice.clone_from(n);
                    }
                    if let Some(d) = descrizione {
                        x.descrizione.clone_from(d);
                    }
                    if let Some(u) = unita_misura {
                        x.unita_misura.clone_from(u);
                    }
                    if let Some(t) = tipo {
                        x.tipo = *t;
                    }
                    if let Some(v) = prezzo {
                        x.prezzo = *v;
                    }
                    Ok(format!("risorsa «{}» modificata", x.codice))
                }
                None => {
                    if codice.trim().is_empty() {
                        return Err("il codice della risorsa è obbligatorio".into());
                    }
                    let r = RisorsaElementare {
                        id: g.nuovo(),
                        codice: codice.clone(),
                        descrizione: descrizione.clone().unwrap_or_else(|| codice.clone()),
                        unita_misura: unita_misura
                            .clone()
                            .ok_or("indicare l'unità di misura della risorsa")?,
                        tipo: tipo.ok_or(
                            "indicare il tipo: manodopera, materiale, nolo, trasporto o altro",
                        )?,
                        prezzo: prezzo.ok_or("indicare il prezzo della risorsa")?,
                    };
                    e.risorse.push(r);
                    Ok(format!("risorsa «{codice}» aggiunta"))
                }
            }
        }
        Comando::RimuoviRisorsa { codice } => {
            let e = &mut p.elenco_prezzi;
            let id = e
                .risorse
                .iter()
                .find(|x| &x.codice == codice)
                .map(|x| x.id)
                .ok_or_else(|| non_trovato("risorsa", codice))?;
            let usata: Vec<&str> = e
                .analisi
                .iter()
                .filter(|a| {
                    a.righe
                        .iter()
                        .any(|r| r.origine == Some(OrigineRigaAnalisi::Risorsa(id)))
                })
                .map(|a| a.codice.as_str())
                .collect();
            if !usata.is_empty() {
                return Err(format!(
                    "la risorsa «{codice}» è usata nelle analisi {}",
                    usata.join(", ")
                ));
            }
            e.risorse.retain(|x| x.id != id);
            Ok(format!("risorsa «{codice}» rimossa"))
        }
        Comando::ImpostaAnalisi {
            voce,
            quantita_riferimento,
            spese_generali,
            utile_impresa,
            base_utile,
            variabili,
            quota_oneri_sicurezza,
            righe,
            applica_prezzo,
        } => {
            let v = voce_elenco(p, voce)?;
            let (voce_id, esistente) = (v.id, v.analisi);
            let (codice, descrizione, um) = (
                v.codice.clone(),
                v.descrizione_breve.clone(),
                v.unita_misura.clone(),
            );
            let nuove = match righe {
                Some(r) => Some(
                    r.iter()
                        .enumerate()
                        .map(|(i, x)| riga(p, x, i + 1))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                None => None,
            };
            let quantita = match quantita_riferimento {
                Some(q) => Some(valuta(q)?.0),
                None => None,
            };
            let variabili: Option<Vec<VariabileAnalisi>> = variabili.as_ref().map(|v| {
                v.iter()
                    .map(|x| VariabileAnalisi {
                        nome: x.nome.trim().to_lowercase(),
                        espressione: match &x.valore {
                            FattoreIngresso::Numero(d) => d.to_string(),
                            FattoreIngresso::Testo(t) => t.trim().to_owned(),
                        },
                        descrizione: x.descrizione.trim().to_owned(),
                    })
                    .collect()
            });
            let par = p.elenco_prezzi.parametri_analisi();
            let e = &mut p.elenco_prezzi;
            let id = match esistente.and_then(|a| e.analisi.iter().position(|x| x.id == a)) {
                Some(i) => {
                    let a = e.analisi.get_mut(i).ok_or("analisi inesistente")?;
                    if let Some(q) = quantita {
                        a.quantita_riferimento = Quantita::new(q);
                    }
                    if let Some(x) = spese_generali {
                        a.spese_generali = *x;
                    }
                    if let Some(x) = utile_impresa {
                        a.utile_impresa = *x;
                    }
                    if let Some(x) = base_utile {
                        a.base_utile = *x;
                    }
                    if let Some(r) = nuove {
                        a.righe = r;
                    }
                    if let Some(v) = variabili {
                        a.variabili = v;
                    }
                    if let Some(q) = quota_oneri_sicurezza {
                        a.quota_oneri_sicurezza = *q;
                    }
                    a.codice.clone_from(&codice);
                    a.unita_misura.clone_from(&um);
                    a.id
                }
                None => {
                    let a = AnalisiPrezzo {
                        id: g.nuovo(),
                        codice: codice.clone(),
                        descrizione,
                        unita_misura: um,
                        quantita_riferimento: Quantita::new(quantita.unwrap_or(Decimal::ONE)),
                        righe: nuove.unwrap_or_default(),
                        spese_generali: spese_generali.unwrap_or(par.spese_generali),
                        utile_impresa: utile_impresa.unwrap_or(par.utile_impresa),
                        base_utile: base_utile.unwrap_or(par.base_utile),
                        variabili: variabili.unwrap_or_default(),
                        quota_oneri_sicurezza: quota_oneri_sicurezza
                            .unwrap_or(par.quota_oneri_sicurezza),
                    };
                    let id = a.id;
                    e.analisi.push(a);
                    if let Some(v) = e.voce_mut(voce_id) {
                        v.analisi = Some(id);
                    }
                    id
                }
            };
            // Quantità delle righe con espressione: valutate con le variabili e le righe
            // precedenti, e conservate anche come valore.
            if let Some(a) = e.analisi.iter_mut().find(|x| x.id == id) {
                let q = a.quantita_righe().map_err(|x| x.to_string())?;
                for (riga, q) in a.righe.iter_mut().zip(q) {
                    riga.quantita = q;
                }
            }
            let r = e
                .calcola_analisi(id, &p.regole)
                .map_err(|x| x.to_string())?;
            let vuota = e.analisi(id).is_none_or(|a| a.righe.is_empty());
            if *applica_prezzo && !vuota {
                e.aggiorna_prezzo_da_analisi(voce_id, &p.regole)
                    .map_err(|x| x.to_string())?;
                Ok(format!(
                    "analisi di «{codice}» salvata: prezzo {} €",
                    formato_importo(opencomputo_core::numeri::Importo::new(
                        r.prezzo_unitario.valore()
                    ))
                ))
            } else {
                Ok(format!(
                    "analisi di «{codice}» salvata (prezzo calcolato {} €, non applicato)",
                    formato_importo(opencomputo_core::numeri::Importo::new(
                        r.prezzo_unitario.valore()
                    ))
                ))
            }
        }
        Comando::RimuoviAnalisi { voce } => {
            let v = voce_elenco(p, voce)?;
            let (id, analisi) = (v.id, v.analisi);
            let a = analisi.ok_or_else(|| format!("la voce «{voce}» non ha un'analisi"))?;
            let e = &mut p.elenco_prezzi;
            e.analisi.retain(|x| x.id != a);
            if let Some(v) = e.voce_mut(id) {
                v.analisi = None;
            }
            Ok(format!("analisi di «{voce}» rimossa"))
        }
        Comando::RicalcolaPrezziAnalisi {} => {
            let cambiati = p
                .elenco_prezzi
                .ricalcola_prezzi_da_analisi(&p.regole)
                .map_err(|e| e.to_string())?;
            Ok(match cambiati.len() {
                0 => "prezzi delle voci analizzate già aggiornati".into(),
                1 => "prezzo di 1 voce aggiornato dall'analisi".into(),
                n => format!("prezzi di {n} voci aggiornati dalle analisi"),
            })
        }
        Comando::ImpostaParametriAnalisi {
            spese_generali,
            utile_impresa,
            base_utile,
            quota_oneri_sicurezza,
            applica_a_tutte,
        } => {
            let e = &mut p.elenco_prezzi;
            let mut par = e.parametri_analisi();
            if let Some(q) = quota_oneri_sicurezza {
                par.quota_oneri_sicurezza = *q;
            }
            if let Some(x) = spese_generali {
                par.spese_generali = *x;
            }
            if let Some(x) = utile_impresa {
                par.utile_impresa = *x;
            }
            if let Some(x) = base_utile {
                par.base_utile = *x;
            }
            e.parametri_analisi = Some(par);
            if *applica_a_tutte {
                for a in &mut e.analisi {
                    a.spese_generali = par.spese_generali;
                    a.utile_impresa = par.utile_impresa;
                    a.base_utile = par.base_utile;
                    a.quota_oneri_sicurezza = par.quota_oneri_sicurezza;
                }
                Ok(format!(
                    "parametri delle analisi aggiornati, anche nelle {} analisi esistenti",
                    e.analisi.len()
                ))
            } else {
                Ok("parametri predefiniti delle analisi aggiornati".into())
            }
        }
        Comando::AggiornaPrezzi {
            oggetto,
            percentuale,
            tipo,
            codici,
            capitolo,
            ricalcola_analisi,
        } => {
            let fattore = Decimal::ONE
                .checked_add(
                    percentuale
                        .checked_div(Decimal::ONE_HUNDRED)
                        .ok_or("percentuale fuori scala")?,
                )
                .ok_or("percentuale fuori scala")?;
            if fattore <= Decimal::ZERO {
                return Err("la variazione non può azzerare o rendere negativi i prezzi".into());
            }
            let regole = p.regole.clone();
            let nuovo = |prezzo: opencomputo_core::numeri::Prezzo| {
                prezzo
                    .valore()
                    .checked_mul(fattore)
                    .map(|v| regole.arrotonda_prezzo(v))
                    .ok_or("prezzo fuori scala")
            };
            let scelto = |c: &str| codici.as_ref().is_none_or(|l| l.iter().any(|x| x == c));
            if let Some(l) = codici {
                for c in l {
                    let esiste = match oggetto {
                        OggettoAggiornamento::Risorse => {
                            p.elenco_prezzi.risorse.iter().any(|x| &x.codice == c)
                        }
                        OggettoAggiornamento::Voci => p.elenco_prezzi.voce_per_codice(c).is_some(),
                    };
                    if !esiste {
                        return Err(non_trovato(
                            match oggetto {
                                OggettoAggiornamento::Risorse => "risorsa",
                                OggettoAggiornamento::Voci => "voce",
                            },
                            c,
                        ));
                    }
                }
            }
            let e = &mut p.elenco_prezzi;
            let mut aggiornati = 0usize;
            let mut analizzate = 0usize;
            match oggetto {
                OggettoAggiornamento::Risorse => {
                    for r in &mut e.risorse {
                        if scelto(&r.codice) && tipo.is_none_or(|t| t == r.tipo) {
                            r.prezzo = nuovo(r.prezzo)?;
                            aggiornati += 1;
                        }
                    }
                }
                OggettoAggiornamento::Voci => {
                    let capitoli: Option<Vec<opencomputo_core::id::CapitoloId>> =
                        capitolo.as_ref().map(|c| {
                            e.capitoli
                                .iter()
                                .filter(|x| {
                                    x.codice == *c || x.codice.starts_with(&format!("{c}."))
                                })
                                .map(|x| x.id)
                                .collect()
                        });
                    if capitoli.as_ref().is_some_and(Vec::is_empty) {
                        return Err(non_trovato("capitolo", capitolo.as_deref().unwrap_or("")));
                    }
                    for v in &mut e.voci {
                        let nel_capitolo = capitoli
                            .as_ref()
                            .is_none_or(|l| v.capitolo.is_some_and(|c| l.contains(&c)));
                        if !scelto(&v.codice) || !nel_capitolo {
                            continue;
                        }
                        if v.analisi.is_some() {
                            analizzate += 1;
                            continue;
                        }
                        let prezzo = nuovo(v.prezzo)?;
                        Arc::make_mut(v).prezzo = prezzo;
                        aggiornati += 1;
                    }
                }
            }
            if aggiornati == 0 {
                return Err("nessun prezzo corrisponde ai criteri indicati".into());
            }
            let ricalcolate = if *ricalcola_analisi {
                e.ricalcola_prezzi_da_analisi(&regole)
                    .map_err(|x| x.to_string())?
                    .len()
            } else {
                0
            };
            let cosa = match oggetto {
                OggettoAggiornamento::Risorse => "risorse",
                OggettoAggiornamento::Voci => "voci",
            };
            let mut msg = format!("prezzi di {aggiornati} {cosa} variati del {percentuale} %");
            if analizzate > 0 {
                msg.push_str(&format!(
                    "; {analizzate} voci analizzate seguono le analisi"
                ));
            }
            if ricalcolate > 0 {
                msg.push_str(&format!("; {ricalcolate} prezzi aggiornati dalle analisi"));
            }
            Ok(msg)
        }
        Comando::ImpostaNuovoPrezzo {
            voce,
            data,
            motivo,
            criterio,
            soggetto_a_ribasso,
            rimuovi,
        } => {
            let v = voce_elenco(p, voce)?;
            let (id, analisi) = (v.id, v.analisi);
            let attuale = v.nuovo_prezzo.clone();
            let prossimo = p
                .elenco_prezzi
                .voci
                .iter()
                .filter_map(|x| x.nuovo_prezzo.as_ref().map(|n| n.numero))
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            let nuovo = if *rimuovi {
                None
            } else {
                Some(match attuale.clone() {
                    Some(n) => NuovoPrezzo {
                        data: data.unwrap_or(n.data),
                        motivo: motivo.clone().unwrap_or(n.motivo),
                        criterio: criterio.unwrap_or(n.criterio),
                        soggetto_a_ribasso: soggetto_a_ribasso.unwrap_or(n.soggetto_a_ribasso),
                        ..n
                    },
                    None => NuovoPrezzo {
                        numero: prossimo,
                        data: data.ok_or("indicare la data del nuovo prezzo")?,
                        motivo: motivo.clone().unwrap_or_default(),
                        criterio: criterio.unwrap_or(if analisi.is_some() {
                            CriterioNuovoPrezzo::Analisi
                        } else {
                            CriterioNuovoPrezzo::Prezzario
                        }),
                        soggetto_a_ribasso: soggetto_a_ribasso.unwrap_or(true),
                        verbale: None,
                    },
                })
            };
            if let (Some(n), true) = (&attuale, *rimuovi)
                && n.verbale
                    .and_then(|v| p.contabilita.verbale_nuovi_prezzi(v))
                    .is_some_and(VerbaleNuoviPrezzi::approvato)
            {
                return Err(format!(
                    "{} è compreso in un verbale approvato e non può essere tolto",
                    n.sigla()
                ));
            }
            let sigla = nuovo.as_ref().or(attuale.as_ref()).map(NuovoPrezzo::sigla);
            if let Some(v) = p.elenco_prezzi.voce_mut(id) {
                v.nuovo_prezzo = nuovo;
            }
            let allineate = allinea_registrazioni(p, id);
            let sigla = sigla.unwrap_or_default();
            let mut msg = if *rimuovi {
                format!("«{voce}» non è più un nuovo prezzo ({sigla})")
            } else if attuale.is_some() {
                format!("{sigla} «{voce}» aggiornato")
            } else {
                format!("«{voce}» registrato come {sigla}")
            };
            if allineate > 0 {
                msg.push_str(&format!(
                    "; ribasso aggiornato in {allineate} registrazioni del libretto"
                ));
            }
            Ok(msg)
        }
        Comando::ImpostaVerbaleNuoviPrezzi {
            numero,
            data,
            nuovi_prezzi,
            firma_esecutore,
            riserve,
            approvato_il,
            atto_approvazione,
            note,
        } => {
            let c = &p.contabilita;
            let attuale = match numero {
                Some(n) => Some(
                    c.verbale_nuovi_prezzi(*n)
                        .cloned()
                        .ok_or_else(|| format!("verbale n. {n} inesistente"))?,
                ),
                None => None,
            };
            let base = match attuale {
                Some(v) => v,
                None => VerbaleNuoviPrezzi {
                    numero: c.prossimo_numero_verbale(),
                    data: data.ok_or("indicare la data del verbale")?,
                    firma_esecutore: None,
                    riserve: None,
                    approvato_il: None,
                    atto_approvazione: None,
                    note: None,
                },
            };
            let pulisci = |t: &Option<String>| t.clone().filter(|x| !x.trim().is_empty());
            let v = VerbaleNuoviPrezzi {
                data: data.unwrap_or(base.data),
                firma_esecutore: firma_esecutore.unwrap_or(base.firma_esecutore),
                riserve: riserve.as_ref().map_or(base.riserve.clone(), pulisci),
                approvato_il: approvato_il.unwrap_or(base.approvato_il),
                atto_approvazione: atto_approvazione
                    .as_ref()
                    .map_or(base.atto_approvazione.clone(), pulisci),
                note: note.as_ref().map_or(base.note.clone(), pulisci),
                ..base
            };
            let n = v.numero;
            // Membri: le voci indicate devono essere nuovi prezzi.
            if let Some(codici) = nuovi_prezzi {
                let mut ids = Vec::new();
                for codice in codici {
                    let voce = voce_elenco(p, codice)?;
                    let np = voce
                        .nuovo_prezzo
                        .as_ref()
                        .ok_or_else(|| format!("«{codice}» non è un nuovo prezzo"))?;
                    if let Some(altro) = np.verbale.filter(|x| *x != n) {
                        return Err(format!(
                            "{} è già compreso nel verbale n. {altro}",
                            np.sigla()
                        ));
                    }
                    ids.push(voce.id);
                }
                for voce in &mut p.elenco_prezzi.voci {
                    // Solo le voci che cambiano: le altre restano condivise.
                    let verbale = voce.nuovo_prezzo.as_ref().map(|np| np.verbale);
                    let nuovo = match verbale {
                        Some(_) if ids.contains(&voce.id) => Some(Some(n)),
                        Some(Some(x)) if x == n => Some(None),
                        _ => None,
                    };
                    if let Some(valore) = nuovo.filter(|v| Some(*v) != verbale)
                        && let Some(np) = Arc::make_mut(voce).nuovo_prezzo.as_mut()
                    {
                        np.verbale = valore;
                    }
                }
            }
            let nuovo = p.contabilita.verbale_nuovi_prezzi(n).is_none();
            p.contabilita
                .imposta_verbale_nuovi_prezzi(v)
                .map_err(|e| e.to_string())?;
            Ok(if nuovo {
                format!("verbale nuovi prezzi n. {n} registrato")
            } else {
                format!("verbale nuovi prezzi n. {n} aggiornato")
            })
        }
        Comando::RimuoviVerbaleNuoviPrezzi { numero } => {
            p.contabilita
                .rimuovi_verbale_nuovi_prezzi(*numero)
                .map_err(|e| e.to_string())?;
            for voce in &mut p.elenco_prezzi.voci {
                if voce
                    .nuovo_prezzo
                    .as_ref()
                    .is_some_and(|x| x.verbale == Some(*numero))
                    && let Some(np) = Arc::make_mut(voce).nuovo_prezzo.as_mut()
                {
                    np.verbale = None;
                }
            }
            Ok(format!("verbale nuovi prezzi n. {numero} rimosso"))
        }
        _ => Err("comando non gestito".into()),
    }
}
