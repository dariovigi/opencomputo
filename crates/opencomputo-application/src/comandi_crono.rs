//! Applicazione dei comandi del cronoprogramma (ADR 0024).

use opencomputo_core::cronoprogramma::{
    self, AttivitaAggiunta, DatiAttivita, Legame, RifAttivita, linea_base,
};
use opencomputo_core::data::data_amg;
use opencomputo_core::id::GeneratoreId;
use opencomputo_core::progetto::Progetto;

use crate::comandi::{Comando, RifAttivitaComando, nodo_wbs, non_trovato};

/// Riferimento del core da quello del comando.
fn rif(p: &Progetto, r: &RifAttivitaComando) -> Result<RifAttivita, String> {
    match r {
        RifAttivitaComando::Wbs(codice) => nodo_wbs(p, codice)?
            .map(RifAttivita::Wbs)
            .ok_or_else(|| "indicare il codice del nodo WBS".to_owned()),
        RifAttivitaComando::Attivita(id) => p
            .cronoprogramma
            .attivita_aggiunta(*id)
            .map(|_| RifAttivita::Attivita(*id))
            .ok_or_else(|| non_trovato("attività", &id.to_string())),
    }
}

/// Nome leggibile di un'attività per i messaggi.
fn nome(p: &Progetto, r: RifAttivita) -> String {
    match r {
        RifAttivita::Wbs(n) => {
            let codice = p.wbs.codice_completo(n).unwrap_or_default();
            let descrizione = p
                .wbs
                .nodo(n)
                .map(|x| x.descrizione.clone())
                .unwrap_or_default();
            format!("{codice} {descrizione}").trim().to_owned()
        }
        RifAttivita::Attivita(a) => p
            .cronoprogramma
            .attivita_aggiunta(a)
            .map(|x| x.nome.clone())
            .unwrap_or_default(),
    }
}

/// Nodo WBS antenato dell'attività (per vietare legami tra un riepilogo e ciò che
/// contiene).
fn contiene(p: &Progetto, sopra: RifAttivita, sotto: RifAttivita) -> bool {
    let RifAttivita::Wbs(nodo) = sopra else {
        return false;
    };
    let mut corrente = match sotto {
        RifAttivita::Wbs(n) => p.wbs.nodo(n).and_then(|x| x.padre),
        RifAttivita::Attivita(a) => p.cronoprogramma.attivita_aggiunta(a).and_then(|x| x.padre),
    };
    let mut passi = 0;
    while let Some(c) = corrente {
        if c == nodo {
            return true;
        }
        corrente = p.wbs.nodo(c).and_then(|x| x.padre);
        passi += 1;
        if passi > p.wbs.nodi.len() {
            return false;
        }
    }
    false
}

/// I legami non devono formare cicli, né con tutti i livelli né con quello corrente.
fn verifica_cicli(p: &Progetto) -> Result<(), String> {
    cronoprogramma::verifica_legami(p).map_err(|e| e.to_string())?;
    let mut prova = p.clone();
    if cronoprogramma::inizio_lavori(&prova).is_none() {
        prova.cronoprogramma.inizio_lavori = data_amg(2000, 1, 3);
    }
    if !prova.cronoprogramma.calendario.ha_giorni_lavorativi() {
        return Ok(());
    }
    for livello in [None, Some(prova.cronoprogramma.livello_dettaglio)] {
        if let Err(e @ cronoprogramma::ErroreCronoprogramma::Ciclo(_)) =
            cronoprogramma::calcola(&prova, livello)
        {
            return Err(e.to_string());
        }
    }
    Ok(())
}

fn giorni(n: u32) -> String {
    if n == 1 {
        "1 giorno".into()
    } else {
        format!("{n} giorni")
    }
}

/// Applica un comando del cronoprogramma.
#[allow(clippy::too_many_lines)]
pub(crate) fn applica<G: GeneratoreId>(
    p: &mut Progetto,
    comando: &Comando,
    g: &mut G,
) -> Result<String, String> {
    match comando {
        Comando::ImpostaCronoprogramma {
            inizio_lavori,
            livello_dettaglio,
            computo,
            giorni_lavorativi,
            festivita_nazionali,
            chiusure,
        } => {
            let computo_id = match computo {
                Some(Some(nome)) => Some(Some(
                    p.computi
                        .iter()
                        .find(|c| &c.nome == nome)
                        .map(|c| c.id)
                        .ok_or_else(|| non_trovato("computo", nome))?,
                )),
                Some(None) => Some(None),
                None => None,
            };
            if let Some(Some(0)) = livello_dettaglio {
                return Err("il livello di dettaglio parte da 1".into());
            }
            if let Some(g) = giorni_lavorativi
                && !g.iter().any(|x| *x)
            {
                return Err("indicare almeno un giorno lavorativo".into());
            }
            if let Some(ch) = chiusure
                && let Some(c) = ch.iter().find(|c| c.al < c.dal)
            {
                return Err(format!(
                    "chiusura «{}»: la fine viene prima dell'inizio",
                    c.descrizione
                ));
            }
            let c = &mut p.cronoprogramma;
            if let Some(v) = inizio_lavori {
                c.inizio_lavori = *v;
            }
            if let Some(v) = livello_dettaglio {
                c.livello_dettaglio = *v;
            }
            if let Some(v) = computo_id {
                c.computo = v;
            }
            if let Some(v) = giorni_lavorativi {
                c.calendario.giorni_lavorativi = *v;
            }
            if let Some(v) = festivita_nazionali {
                c.calendario.festivita_nazionali = *v;
            }
            if let Some(v) = chiusure {
                c.calendario.chiusure.clone_from(v);
            }
            Ok("dati del cronoprogramma aggiornati".into())
        }
        Comando::ImpostaAttivita {
            attivita,
            durata,
            inizio_non_prima,
            avanzamento,
            note,
            nome: nuovo_nome,
            pietra_miliare,
            padre,
        } => {
            let r = rif(p, attivita)?;
            if avanzamento.is_some_and(|a| a > 100) {
                return Err("l'avanzamento va da 0 a 100".into());
            }
            if let Some(Some(d)) = durata
                && *d > 20_000
            {
                return Err("durata oltre 20.000 giorni lavorativi".into());
            }
            let padre_id = match padre {
                Some(Some(codice)) => Some(nodo_wbs(p, codice)?),
                Some(None) => Some(None),
                None => None,
            };
            let applica_dati = |d: &mut DatiAttivita| {
                if let Some(v) = durata {
                    d.durata = *v;
                }
                if let Some(v) = inizio_non_prima {
                    d.inizio_non_prima = *v;
                }
                if let Some(v) = avanzamento {
                    d.avanzamento = *v;
                }
                if let Some(v) = note {
                    d.note = v.clone().filter(|n| !n.trim().is_empty());
                }
            };
            let descrizione = nome(p, r);
            match r {
                RifAttivita::Wbs(n) => {
                    if nuovo_nome.is_some() || pietra_miliare.is_some() || padre.is_some() {
                        return Err(
                            "nome, pietra miliare e padre si cambiano solo per le attività \
                             aggiunte (per i nodi WBS: in «Categorie, WBS, computi»)"
                                .into(),
                        );
                    }
                    let mut d = p.cronoprogramma.dati_wbs(n);
                    applica_dati(&mut d);
                    p.cronoprogramma.imposta_dati_wbs(n, d);
                }
                RifAttivita::Attivita(id) => {
                    if let Some(n) = nuovo_nome
                        && n.trim().is_empty()
                    {
                        return Err("il nome dell'attività non può essere vuoto".into());
                    }
                    let a = p
                        .cronoprogramma
                        .attivita
                        .iter_mut()
                        .find(|a| a.id == id)
                        .ok_or_else(|| non_trovato("attività", &id.to_string()))?;
                    applica_dati(&mut a.dati);
                    if let Some(n) = nuovo_nome {
                        a.nome = n.trim().to_owned();
                    }
                    if let Some(m) = pietra_miliare {
                        a.pietra_miliare = *m;
                    }
                    if let Some(pd) = padre_id {
                        a.padre = pd;
                    }
                }
            }
            if padre_id.is_some() {
                verifica_cicli(p)?;
            }
            Ok(match durata {
                Some(Some(d)) => format!("{descrizione}: durata {}", giorni(*d)),
                _ => format!("attività «{descrizione}» aggiornata"),
            })
        }
        Comando::AggiungiAttivita {
            nome: n,
            padre,
            pietra_miliare,
            durata,
        } => {
            if n.trim().is_empty() {
                return Err("il nome dell'attività non può essere vuoto".into());
            }
            let padre = match padre {
                Some(c) if !c.trim().is_empty() => nodo_wbs(p, c)?,
                _ => None,
            };
            p.cronoprogramma.attivita.push(AttivitaAggiunta {
                id: g.nuovo(),
                nome: n.trim().to_owned(),
                padre,
                pietra_miliare: *pietra_miliare,
                dati: DatiAttivita {
                    durata: if *pietra_miliare { None } else { *durata },
                    ..DatiAttivita::default()
                },
            });
            Ok(if *pietra_miliare {
                format!("pietra miliare «{}» aggiunta", n.trim())
            } else {
                format!("attività «{}» aggiunta", n.trim())
            })
        }
        Comando::RimuoviAttivita { attivita } => {
            let a = p
                .cronoprogramma
                .attivita_aggiunta(*attivita)
                .ok_or_else(|| non_trovato("attività", &attivita.to_string()))?
                .nome
                .clone();
            p.cronoprogramma.rimuovi_attivita(*attivita);
            Ok(format!("attività «{a}» rimossa"))
        }
        Comando::CollegaAttivita {
            da,
            a,
            tipo,
            ritardo,
        } => {
            let (rda, ra) = (rif(p, da)?, rif(p, a)?);
            if rda == ra {
                return Err("un'attività non può precedere sé stessa".into());
            }
            if contiene(p, rda, ra) || contiene(p, ra, rda) {
                return Err(format!(
                    "«{}» contiene «{}»: un riepilogo non si collega alle attività che contiene",
                    nome(p, if contiene(p, rda, ra) { rda } else { ra }),
                    nome(p, if contiene(p, rda, ra) { ra } else { rda })
                ));
            }
            if ritardo.unsigned_abs() > 20_000 {
                return Err("ritardo oltre 20.000 giorni lavorativi".into());
            }
            let c = &mut p.cronoprogramma;
            let esisteva = c.legami.iter().any(|l| l.da == rda && l.a == ra);
            c.legami.retain(|l| !(l.da == rda && l.a == ra));
            c.legami.push(Legame {
                da: rda,
                a: ra,
                tipo: *tipo,
                ritardo: *ritardo,
            });
            verifica_cicli(p).map_err(|e| format!("legame rifiutato: {e}"))?;
            let ritardo_testo = match ritardo {
                0 => String::new(),
                r if *r > 0 => format!("+{r}"),
                r => r.to_string(),
            };
            Ok(format!(
                "{} {} → {}{ritardo_testo} ({})",
                if esisteva {
                    "legame aggiornato:"
                } else {
                    "legame:"
                },
                nome(p, rda),
                nome(p, ra),
                tipo.sigla()
            ))
        }
        Comando::ScollegaAttivita { da, a } => {
            let (rda, ra) = (rif(p, da)?, rif(p, a)?);
            let prima = p.cronoprogramma.legami.len();
            p.cronoprogramma
                .legami
                .retain(|l| !(l.da == rda && l.a == ra));
            if p.cronoprogramma.legami.len() == prima {
                return Err(format!(
                    "nessun legame tra «{}» e «{}»",
                    nome(p, rda),
                    nome(p, ra)
                ));
            }
            Ok(format!("legame tolto: {} → {}", nome(p, rda), nome(p, ra)))
        }
        Comando::LineaBaseCronoprogramma { data, cancella } => {
            if *cancella {
                if p.cronoprogramma.linea_base.take().is_none() {
                    return Err("non c'è una linea di base".into());
                }
                return Ok("linea di base tolta".into());
            }
            let data = data.ok_or("indicare il giorno della linea di base")?;
            let piano = cronoprogramma::calcola(p, Some(None)).map_err(|e| e.to_string())?;
            let n = piano.righe.len();
            p.cronoprogramma.linea_base = Some(linea_base(&piano, data));
            Ok(format!("linea di base salvata ({n} attività)"))
        }
        _ => Err("comando non del cronoprogramma".into()),
    }
}
