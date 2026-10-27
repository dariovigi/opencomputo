//! Validazione della risposta di selezione e traduzione in comandi (ADR 0025).
//!
//! La risposta del modello non modifica nulla: diventa una [`Proposta`] con
//!
//! * le lavorazioni accettate, ciascuna legata alla voce del prezzario, con forma,
//!   parametri (valore dato, unità, valore in unità base) e provenienza di ogni valore;
//! * le lavorazioni escluse, con il motivo;
//! * ipotesi, informazioni mancanti, avvisi con codici stabili;
//! * i [`Comando`] da eseguire: `copia_da_prezzario` per le voci che mancano
//!   nell'elenco prezzi, poi un `aggiungi_voce_computo` per lavorazione.
//!
//! Controlli, oltre a quelli dello schema:
//!
//! | Codice | Effetto | Quando |
//! |---|---|---|
//! | `candidato_inesistente` | lavorazione esclusa | handle non inviato in questa richiesta |
//! | `voce_non_trovata` | lavorazione esclusa (avviso) | il modello risponde `NESSUNO` |
//! | `unita_non_supportata` | esclusa | unità della voce senza forma corrispondente |
//! | `unita_incompatibile` | esclusa | la forma dà una grandezza diversa dall'unità della voce |
//! | `parametri_errati` | esclusa | parametri mancanti, in più, ripetuti, unità non ammessa |
//! | `valore_non_valido` | esclusa | valore non numerico, zero, N non intero |
//! | `ipotesi_inesistente` | esclusa | `rif_ipotesi` che non esiste |
//! | `ruolo_incoerente` | esclusa | campi di ruolo in contrasto con il ruolo |
//! | `righe_assenti` | esclusa | nessun rigo |
//! | `duplicato` | esclusa | stessa voce e stessi righi di una lavorazione precedente |
//! | `valore_non_fondato` | avviso, valore reso ipotesi | «utente» ma assente dalla richiesta |
//! | `possibile_doppio_conteggio` | avviso | stessa voce in più lavorazioni con righi diversi |
//! | `alternativa_non_inclusa` | informazione | alternativa dopo la prima del suo gruppo |
//! | `complementare_senza_principale` | avviso | `complementare_di` non è una lavorazione accettata |
//! | `prezzo_elenco_diverso` | avviso | la voce è già nell'elenco con un altro prezzo |
//!
//! L'esito è `completo` solo se il modello lo dichiara e non ci sono esclusioni, avvisi,
//! ipotesi né informazioni mancanti; `impossibile` se nessuna lavorazione è accettata;
//! altrimenti `provvisorio`.

use std::collections::{BTreeMap, BTreeSet};

use opencomputo_application::{Comando, NuovoRigo};
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::validazione::Gravita;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::candidati::Candidati;
use crate::forme::{Forma, Parametro, rigo};
use crate::risposta::{InformazioneMancante, RispostaSelezione};
use crate::unita::{Dimensione, riconosci};
use crate::valori::{UnitaValore, ValoreFondato, estrai};

/// Lunghezza massima delle note scritte nella voce di computo, in caratteri.
const NOTA_MASSIMA: usize = 1000;

/// Esito della proposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EsitoProposta {
    /// Nessun problema noto.
    Completo,
    /// Da rivedere: esclusioni, ipotesi, avvisi o informazioni mancanti.
    Provvisorio,
    /// Nessuna lavorazione accettata.
    Impossibile,
}

/// Ruolo di una lavorazione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ruolo {
    /// Lavorazione principale.
    Principale,
    /// Complementare a un'altra lavorazione.
    Complementare,
    /// Una tra più alternative.
    Alternativa,
}

/// Impatto di un'ipotesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Impatto {
    /// Basso.
    Basso,
    /// Medio.
    Medio,
    /// Alto.
    Alto,
}

/// Provenienza di un valore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ProvenienzaValore {
    /// Scritto nella richiesta.
    Utente {
        /// Testo della richiesta che lo contiene (vuoto per N = 1).
        testo: String,
    },
    /// Ipotesi.
    Ipotesi {
        /// Identificativo dell'ipotesi.
        id: String,
    },
}

/// Parametro accettato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParametroProposto {
    /// Parametro.
    pub nome: Parametro,
    /// Valore come indicato.
    pub valore: Decimal,
    /// Unità indicata.
    pub unita: UnitaValore,
    /// Valore nell'unità base (m, m², kg/m, numero).
    pub valore_base: Decimal,
    /// Provenienza.
    pub provenienza: ProvenienzaValore,
}

/// Rigo accettato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigaProposta {
    /// Descrizione.
    pub descrizione: String,
    /// Forma.
    pub forma: Forma,
    /// Rigo in deduzione.
    pub deduzione: bool,
    /// Parametri, nell'ordine della forma.
    pub parametri: Vec<ParametroProposto>,
}

/// Lavorazione accettata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LavorazioneProposta {
    /// Identificativo.
    pub id: String,
    /// Descrizione.
    pub descrizione: String,
    /// Ruolo.
    pub ruolo: Ruolo,
    /// Gruppo delle alternative.
    pub gruppo_alternativa: Option<String>,
    /// Lavorazione di cui è complementare.
    pub complementare_di: Option<String>,
    /// Handle scelto.
    pub handle: String,
    /// Codice della voce.
    pub codice: String,
    /// Unità di misura della voce.
    pub unita_misura: String,
    /// Righi.
    pub righe: Vec<RigaProposta>,
    /// Ipotesi usate dai parametri.
    pub ipotesi: Vec<String>,
    /// Motivazione del modello.
    pub motivazione: String,
    /// `false` per le alternative non scelte: non entrano nei comandi.
    pub inclusa: bool,
}

/// Lavorazione esclusa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LavorazioneEsclusa {
    /// Identificativo.
    pub id: String,
    /// Descrizione.
    pub descrizione: String,
    /// Motivi.
    pub motivi: Vec<String>,
}

/// Ipotesi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ipotesi {
    /// Identificativo (`I01` del modello, `R01` se creata dalla validazione).
    pub id: String,
    /// Testo.
    pub testo: String,
    /// Impatto.
    pub impatto: Impatto,
    /// `true` se creata dalla validazione (valore dichiarato fornito ma non trovato).
    pub dalla_validazione: bool,
}

/// Avviso della validazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Avviso {
    /// Gravità.
    pub gravita: Gravita,
    /// Codice stabile (`assistente.…`).
    pub codice: String,
    /// Messaggio.
    pub messaggio: String,
    /// Lavorazione interessata.
    pub lavorazione: Option<String>,
}

/// Proposta di computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposta {
    /// Esito deciso dalla validazione.
    pub esito: EsitoProposta,
    /// Esito dichiarato dal modello.
    pub esito_modello: String,
    /// Lavorazioni accettate.
    pub lavorazioni: Vec<LavorazioneProposta>,
    /// Lavorazioni escluse.
    pub escluse: Vec<LavorazioneEsclusa>,
    /// Ipotesi.
    pub ipotesi: Vec<Ipotesi>,
    /// Informazioni mancanti.
    pub informazioni_mancanti: Vec<InformazioneMancante>,
    /// Avvisi.
    pub avvisi: Vec<Avviso>,
    /// Valori trovati nella richiesta.
    pub valori_fondati: Vec<ValoreFondato>,
    /// Comandi da provare ed eseguire.
    pub comandi: Vec<Comando>,
}

/// Contesto della validazione.
#[derive(Debug, Clone, Copy)]
pub struct Contesto<'a> {
    /// Testo della richiesta dell'utente.
    pub richiesta: &'a str,
    /// Nome con cui il prezzario è caricato nella sessione.
    pub nome_prezzario: &'a str,
    /// Prezzario di consultazione.
    pub prezzario: &'a Prezzario,
    /// Candidati inviati al modello.
    pub candidati: &'a Candidati,
    /// Progetto corrente (elenco prezzi e computi).
    pub progetto: &'a Progetto,
    /// Computo di destinazione (facoltativo se il progetto ne ha uno solo).
    pub computo: Option<&'a str>,
}

/// Risposta che non si può usare nel suo insieme.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "codice", rename_all = "snake_case")]
pub enum ErroreRisposta {
    /// JSON non conforme allo schema.
    #[error("risposta del modello non conforme allo schema: {motivo}")]
    NonConforme {
        /// Motivo.
        motivo: String,
    },
    /// Identificativo ripetuto.
    #[error("identificativo «{id}» ripetuto tra {tipo}")]
    IdRipetuto {
        /// Lavorazioni o ipotesi.
        tipo: String,
        /// Identificativo.
        id: String,
    },
    /// Esito sconosciuto.
    #[error("esito «{esito}» sconosciuto")]
    EsitoSconosciuto {
        /// Esito.
        esito: String,
    },
}

/// Legge una risposta di selezione dal JSON dello strumento.
///
/// # Errors
/// JSON non conforme alla struttura attesa.
pub fn leggi_selezione(json: &serde_json::Value) -> Result<RispostaSelezione, ErroreRisposta> {
    serde_json::from_value(json.clone()).map_err(|e| ErroreRisposta::NonConforme {
        motivo: e.to_string(),
    })
}

/// Valore di un elenco chiuso, senza distinzione di maiuscole.
fn scegli<T: Copy>(testo: &str, valori: &[(&str, T)]) -> Option<T> {
    let t = testo.trim();
    valori
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(t))
        .map(|(_, v)| *v)
}

fn forma(testo: &str) -> Option<Forma> {
    let t = testo.trim();
    Forma::TUTTE
        .into_iter()
        .find(|f| f.nome().eq_ignore_ascii_case(t))
}

const PARAMETRI: [(&str, Parametro); 10] = [
    ("N", Parametro::N),
    ("L", Parametro::L),
    ("B", Parametro::B),
    ("H", Parametro::H),
    ("D", Parametro::D),
    ("B1", Parametro::B1),
    ("B2", Parametro::B2),
    ("A", Parametro::A),
    ("V", Parametro::V),
    ("P", Parametro::P),
];

const UNITA: [(&str, UnitaValore); 7] = [
    ("n", UnitaValore::N),
    ("mm", UnitaValore::Mm),
    ("cm", UnitaValore::Cm),
    ("m", UnitaValore::M),
    ("m2", UnitaValore::M2),
    ("m3", UnitaValore::M3),
    ("kg/m", UnitaValore::KgM),
];

fn avviso(gravita: Gravita, codice: &str, messaggio: String, lavorazione: Option<&str>) -> Avviso {
    Avviso {
        gravita,
        codice: format!("assistente.{codice}"),
        messaggio,
        lavorazione: lavorazione.map(str::to_owned),
    }
}

fn tronca(testo: &str, massimo: usize) -> String {
    if testo.chars().count() <= massimo {
        return testo.to_owned();
    }
    let mut t: String = testo.chars().take(massimo.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// Lavorazione accettata con i righi per il comando.
struct Accettata {
    proposta: LavorazioneProposta,
    righi: Vec<NuovoRigo>,
}

/// Controlla una lavorazione. Restituisce i motivi di esclusione se non è accettabile.
#[allow(clippy::too_many_lines)]
fn controlla(
    l: &crate::risposta::LavorazioneSelezione,
    risposta: &RispostaSelezione,
    ctx: &Contesto<'_>,
    fondati: &[ValoreFondato],
    ipotesi: &mut Vec<Ipotesi>,
    avvisi: &mut Vec<Avviso>,
) -> Result<Accettata, Vec<(String, String)>> {
    let mut errori: Vec<(String, String)> = Vec::new();
    let id = l.id.trim();
    let ruolo = scegli(
        &l.ruolo,
        &[
            ("principale", Ruolo::Principale),
            ("complementare", Ruolo::Complementare),
            ("alternativa", Ruolo::Alternativa),
        ],
    );
    let gruppo = l.gruppo_alternativa.trim();
    let di = l.complementare_di.trim();
    match ruolo {
        None => errori.push((
            "ruolo_incoerente".into(),
            format!("ruolo «{}» sconosciuto", l.ruolo),
        )),
        Some(Ruolo::Principale) if !gruppo.is_empty() || !di.is_empty() => errori.push((
            "ruolo_incoerente".into(),
            "una lavorazione principale non ha gruppo di alternative né lavorazione principale"
                .into(),
        )),
        Some(Ruolo::Complementare)
            if di.is_empty() || !gruppo.is_empty() || di.eq_ignore_ascii_case(id) =>
        {
            errori.push((
                "ruolo_incoerente".into(),
                "una lavorazione complementare indica la sua principale e non ha gruppo di alternative".into(),
            ));
        }
        Some(Ruolo::Alternativa) if gruppo.is_empty() || !di.is_empty() => errori.push((
            "ruolo_incoerente".into(),
            "un'alternativa indica il suo gruppo e non è complementare".into(),
        )),
        _ => {}
    }
    let Some(candidato) = ctx.candidati.per_handle(&l.candidato) else {
        errori.push((
            "candidato_inesistente".into(),
            format!("il candidato «{}» non è tra quelli inviati", l.candidato),
        ));
        return Err(errori);
    };
    // La voce che entrerà nel computo è quella dell'elenco prezzi, se c'è già.
    let voce = ctx
        .progetto
        .elenco_prezzi
        .voce_per_codice(&candidato.codice)
        .or_else(|| ctx.prezzario.voce_per_codice(&candidato.codice));
    let Some(voce) = voce else {
        errori.push((
            "candidato_inesistente".into(),
            format!("la voce «{}» non è più nel prezzario", candidato.codice),
        ));
        return Err(errori);
    };
    let unita_voce = riconosci(&voce.unita_misura);
    if Forma::per_dimensione(unita_voce.dimensione).is_empty() {
        errori.push((
            "unita_non_supportata".into(),
            format!(
                "l'unità «{}» della voce {} non ha una forma di misura: va misurata a mano",
                voce.unita_misura, voce.codice
            ),
        ));
    }
    if l.righe.is_empty() {
        errori.push(("righe_assenti".into(), "nessun rigo di misura".into()));
    }
    let ipotesi_modello: BTreeSet<String> = risposta
        .ipotesi
        .iter()
        .map(|i| i.id.trim().to_uppercase())
        .collect();

    let mut righe = Vec::new();
    let mut righi = Vec::new();
    let mut usate: BTreeSet<String> = BTreeSet::new();
    // Ipotesi e avvisi dei valori non fondati: registrati solo se la lavorazione è
    // accettata, così una lavorazione esclusa non lascia ipotesi orfane.
    let mut nuove_ipotesi: Vec<Ipotesi> = Vec::new();
    let mut nuovi_avvisi: Vec<Avviso> = Vec::new();
    for (n, r) in l.righe.iter().enumerate() {
        let nr = n + 1;
        let Some(f) = forma(&r.forma) else {
            errori.push((
                "parametri_errati".into(),
                format!("rigo {nr}: forma «{}» sconosciuta", r.forma),
            ));
            continue;
        };
        if unita_voce.dimensione != Dimensione::Sconosciuta
            && !Forma::per_dimensione(unita_voce.dimensione).is_empty()
            && f.dimensione() != unita_voce.dimensione
        {
            errori.push((
                "unita_incompatibile".into(),
                format!(
                    "rigo {nr}: la forma «{}» misura {}, la voce {} è in «{}» ({})",
                    f.nome(),
                    f.dimensione().nome(),
                    voce.codice,
                    voce.unita_misura,
                    unita_voce.dimensione.nome()
                ),
            ));
        }
        let deduzione = match scegli(&r.segno, &[("positivo", false), ("detrazione", true)]) {
            Some(d) => d,
            None => {
                errori.push((
                    "parametri_errati".into(),
                    format!("rigo {nr}: segno «{}» sconosciuto", r.segno),
                ));
                false
            }
        };
        let mut valori: BTreeMap<Parametro, Decimal> = BTreeMap::new();
        let mut parametri = Vec::new();
        for p in &r.parametri {
            let Some(nome) = scegli(&p.nome, &PARAMETRI) else {
                errori.push((
                    "parametri_errati".into(),
                    format!("rigo {nr}: parametro «{}» sconosciuto", p.nome),
                ));
                continue;
            };
            if valori.contains_key(&nome) {
                errori.push((
                    "parametri_errati".into(),
                    format!("rigo {nr}: parametro {nome:?} ripetuto"),
                ));
                continue;
            }
            let Some(unita) = scegli(&p.unita, &UNITA) else {
                errori.push((
                    "parametri_errati".into(),
                    format!("rigo {nr}: unità «{}» sconosciuta", p.unita),
                ));
                continue;
            };
            if !nome.unita_ammesse().contains(&unita) {
                errori.push((
                    "parametri_errati".into(),
                    format!("rigo {nr}: unità «{}» non ammessa per {nome:?}", p.unita),
                ));
                continue;
            }
            let valore = match Decimal::from_str_exact(p.valore.trim()) {
                Ok(v) if v > Decimal::ZERO => v,
                Ok(_) => {
                    errori.push((
                        "valore_non_valido".into(),
                        format!("rigo {nr}: {nome:?} vale zero"),
                    ));
                    continue;
                }
                Err(_) => {
                    errori.push((
                        "valore_non_valido".into(),
                        format!("rigo {nr}: valore «{}» di {nome:?} non numerico", p.valore),
                    ));
                    continue;
                }
            };
            if nome == Parametro::N && valore.fract() != Decimal::ZERO {
                errori.push((
                    "valore_non_valido".into(),
                    format!("rigo {nr}: N deve essere intero ({valore})"),
                ));
                continue;
            }
            let provenienza = match scegli(&p.provenienza, &[("utente", true), ("ipotesi", false)])
            {
                Some(true) => {
                    let trovato = fondati.iter().find(|v| v.corrisponde(valore, unita));
                    match trovato {
                        Some(v) => ProvenienzaValore::Utente {
                            testo: v.testo.clone(),
                        },
                        None if nome == Parametro::N && valore == Decimal::ONE => {
                            ProvenienzaValore::Utente {
                                testo: String::new(),
                            }
                        }
                        None => {
                            let nuovo = format!(
                                "R{:02}",
                                ipotesi.iter().filter(|i| i.dalla_validazione).count()
                                    + nuove_ipotesi.len()
                                    + 1
                            );
                            let testo = format!(
                                "{nome:?} = {valore} {} (lavorazione {id}, rigo {nr}) indicato come dato dell'utente, ma la richiesta non lo contiene",
                                p.unita
                            );
                            nuovi_avvisi.push(avviso(
                                Gravita::Avviso,
                                "valore_non_fondato",
                                testo.clone(),
                                Some(id),
                            ));
                            nuove_ipotesi.push(Ipotesi {
                                id: nuovo.clone(),
                                testo,
                                impatto: Impatto::Alto,
                                dalla_validazione: true,
                            });
                            ProvenienzaValore::Ipotesi { id: nuovo }
                        }
                    }
                }
                Some(false) => {
                    let rif = p.rif_ipotesi.trim().to_uppercase();
                    if !ipotesi_modello.contains(&rif) {
                        errori.push((
                            "ipotesi_inesistente".into(),
                            format!(
                                "rigo {nr}: {nome:?} rimanda all'ipotesi «{}» che non esiste",
                                p.rif_ipotesi
                            ),
                        ));
                        continue;
                    }
                    ProvenienzaValore::Ipotesi { id: rif }
                }
                None => {
                    errori.push((
                        "parametri_errati".into(),
                        format!("rigo {nr}: provenienza «{}» sconosciuta", p.provenienza),
                    ));
                    continue;
                }
            };
            if let ProvenienzaValore::Ipotesi { id } = &provenienza {
                usate.insert(id.clone());
            }
            let valore_base = unita.in_base(valore);
            valori.insert(nome, valore_base);
            parametri.push(ParametroProposto {
                nome,
                valore,
                unita,
                valore_base,
                provenienza,
            });
        }
        let attesi: BTreeSet<Parametro> = f.parametri().iter().copied().collect();
        let dati: BTreeSet<Parametro> = valori.keys().copied().collect();
        let mancanti: Vec<_> = attesi.difference(&dati).collect();
        let in_piu: Vec<_> = dati.difference(&attesi).collect();
        if !mancanti.is_empty() || !in_piu.is_empty() {
            errori.push((
                "parametri_errati".into(),
                format!(
                    "rigo {nr}: la forma «{}» vuole {:?}; mancano {mancanti:?}, in più {in_piu:?}",
                    f.nome(),
                    f.parametri()
                ),
            ));
            continue;
        }
        parametri.sort_by_key(|p| f.parametri().iter().position(|x| *x == p.nome));
        righi.push(rigo(
            f,
            &valori,
            unita_voce.fattore,
            r.descrizione.trim(),
            deduzione,
        ));
        righe.push(RigaProposta {
            descrizione: r.descrizione.trim().to_owned(),
            forma: f,
            deduzione,
            parametri,
        });
    }
    if !errori.is_empty() {
        return Err(errori);
    }
    ipotesi.append(&mut nuove_ipotesi);
    avvisi.append(&mut nuovi_avvisi);
    Ok(Accettata {
        proposta: LavorazioneProposta {
            id: id.to_owned(),
            descrizione: l.descrizione.trim().to_owned(),
            ruolo: ruolo.unwrap_or(Ruolo::Principale),
            gruppo_alternativa: (!gruppo.is_empty()).then(|| gruppo.to_owned()),
            complementare_di: (!di.is_empty()).then(|| di.to_uppercase()),
            handle: candidato.handle.clone(),
            codice: voce.codice.clone(),
            unita_misura: voce.unita_misura.clone(),
            righe,
            ipotesi: usate.into_iter().collect(),
            motivazione: l.motivazione.trim().to_owned(),
            inclusa: true,
        },
        righi,
    })
}

/// Misure di un rigo senza la descrizione: forma, segno e valori in unità base.
type MisuraRigo = (Forma, bool, Vec<(Parametro, Decimal)>);

/// Misure di una lavorazione senza le descrizioni.
fn misure(l: &LavorazioneProposta) -> Vec<MisuraRigo> {
    l.righe
        .iter()
        .map(|r| {
            (
                r.forma,
                r.deduzione,
                r.parametri
                    .iter()
                    .map(|p| (p.nome, p.valore_base.normalize()))
                    .collect(),
            )
        })
        .collect()
}

fn nota(l: &LavorazioneProposta, ipotesi: &[Ipotesi]) -> String {
    let mut n = format!("Assistente AI, {}: {}.", l.id, l.descrizione);
    if !l.motivazione.is_empty() {
        n.push_str(&format!(" Scelta: {}.", l.motivazione));
    }
    if !l.ipotesi.is_empty() {
        let testi: Vec<String> = l
            .ipotesi
            .iter()
            .map(|id| match ipotesi.iter().find(|i| i.id == *id) {
                Some(i) => format!("{id} ({})", i.testo),
                None => id.clone(),
            })
            .collect();
        n.push_str(&format!(" Ipotesi: {}.", testi.join("; ")));
    }
    tronca(&n, NOTA_MASSIMA)
}

/// Valida una risposta di selezione e la traduce in una proposta con i comandi.
///
/// # Errors
/// Esito sconosciuto o identificativi ripetuti: la risposta va richiesta di nuovo.
#[allow(clippy::too_many_lines)]
pub fn valida_selezione(
    risposta: &RispostaSelezione,
    ctx: &Contesto<'_>,
) -> Result<Proposta, ErroreRisposta> {
    let esito_modello = scegli(
        &risposta.esito,
        &[("completo", 0u8), ("provvisorio", 1), ("impossibile", 2)],
    )
    .ok_or_else(|| ErroreRisposta::EsitoSconosciuto {
        esito: risposta.esito.clone(),
    })?;
    for (tipo, ids) in [
        (
            "lavorazioni",
            risposta
                .lavorazioni
                .iter()
                .map(|l| l.id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "ipotesi",
            risposta.ipotesi.iter().map(|i| i.id.as_str()).collect(),
        ),
    ] {
        let mut visti = BTreeSet::new();
        for id in ids {
            if !visti.insert(id.trim().to_uppercase()) {
                return Err(ErroreRisposta::IdRipetuto {
                    tipo: tipo.into(),
                    id: id.to_owned(),
                });
            }
        }
    }
    let fondati = estrai(ctx.richiesta);
    let mut ipotesi: Vec<Ipotesi> = risposta
        .ipotesi
        .iter()
        .map(|i| Ipotesi {
            id: i.id.trim().to_uppercase(),
            testo: i.testo.trim().to_owned(),
            impatto: scegli(
                &i.impatto,
                &[
                    ("basso", Impatto::Basso),
                    ("medio", Impatto::Medio),
                    ("alto", Impatto::Alto),
                ],
            )
            .unwrap_or(Impatto::Alto),
            dalla_validazione: false,
        })
        .collect();
    let mut avvisi = Vec::new();
    let mut accettate: Vec<Accettata> = Vec::new();
    let mut escluse = Vec::new();

    for l in &risposta.lavorazioni {
        if l.candidato.trim().eq_ignore_ascii_case("NESSUNO") {
            avvisi.push(avviso(
                Gravita::Avviso,
                "voce_non_trovata",
                format!("{}: nessuna voce pertinente tra i candidati; va scelta a mano o creato un nuovo prezzo", l.descrizione.trim()),
                Some(l.id.trim()),
            ));
            escluse.push(LavorazioneEsclusa {
                id: l.id.trim().to_owned(),
                descrizione: l.descrizione.trim().to_owned(),
                motivi: vec!["nessuna voce pertinente tra i candidati".into()],
            });
            continue;
        }
        match controlla(l, risposta, ctx, &fondati, &mut ipotesi, &mut avvisi) {
            Ok(a) => accettate.push(a),
            Err(errori) => {
                for (codice, messaggio) in &errori {
                    avvisi.push(avviso(
                        Gravita::Errore,
                        codice,
                        messaggio.clone(),
                        Some(l.id.trim()),
                    ));
                }
                escluse.push(LavorazioneEsclusa {
                    id: l.id.trim().to_owned(),
                    descrizione: l.descrizione.trim().to_owned(),
                    motivi: errori.into_iter().map(|(_, m)| m).collect(),
                });
            }
        }
    }

    // Duplicati: stessa voce e stessi righi di una lavorazione precedente.
    let mut tenute: Vec<Accettata> = Vec::new();
    for a in accettate {
        let doppione = tenute
            .iter()
            .find(|t| {
                t.proposta.codice == a.proposta.codice && misure(&t.proposta) == misure(&a.proposta)
            })
            .map(|t| t.proposta.id.clone());
        if let Some(prima) = doppione {
            let m = format!("stessa voce e stessi righi della lavorazione {prima}");
            avvisi.push(avviso(
                Gravita::Errore,
                "duplicato",
                m.clone(),
                Some(&a.proposta.id),
            ));
            escluse.push(LavorazioneEsclusa {
                id: a.proposta.id.clone(),
                descrizione: a.proposta.descrizione.clone(),
                motivi: vec![m],
            });
            continue;
        }
        if let Some(altra) = tenute
            .iter()
            .find(|t| t.proposta.codice == a.proposta.codice)
        {
            avvisi.push(avviso(
                Gravita::Avviso,
                "possibile_doppio_conteggio",
                format!(
                    "la voce {} è usata anche dalla lavorazione {}: verificare che le quantità non si sovrappongano",
                    a.proposta.codice, altra.proposta.id
                ),
                Some(&a.proposta.id),
            ));
        }
        tenute.push(a);
    }

    // Alternative: entra nei comandi solo la prima di ogni gruppo.
    let mut gruppi_visti: BTreeMap<String, String> = BTreeMap::new();
    for a in &mut tenute {
        if a.proposta.ruolo != Ruolo::Alternativa {
            continue;
        }
        let gruppo = a
            .proposta
            .gruppo_alternativa
            .clone()
            .unwrap_or_default()
            .to_lowercase();
        match gruppi_visti.get(&gruppo) {
            Some(prima) => {
                a.proposta.inclusa = false;
                avvisi.push(avviso(
                    Gravita::Informazione,
                    "alternativa_non_inclusa",
                    format!(
                        "alternativa a {prima} (gruppo «{gruppo}»): non inclusa nel computo, si può scegliere al suo posto"
                    ),
                    Some(&a.proposta.id),
                ));
            }
            None => {
                gruppi_visti.insert(gruppo, a.proposta.id.clone());
            }
        }
    }

    // Complementari: la principale deve essere tra le lavorazioni incluse.
    let incluse: BTreeSet<String> = tenute
        .iter()
        .filter(|a| a.proposta.inclusa)
        .map(|a| a.proposta.id.to_uppercase())
        .collect();
    for a in &tenute {
        if let Some(di) = &a.proposta.complementare_di
            && !incluse.contains(di)
        {
            avvisi.push(avviso(
                Gravita::Avviso,
                "complementare_senza_principale",
                format!("complementare di {di}, che non è tra le lavorazioni incluse"),
                Some(&a.proposta.id),
            ));
        }
    }

    // Prezzi: una voce già nell'elenco mantiene il suo prezzo.
    let mut segnalati = BTreeSet::new();
    for a in tenute.iter().filter(|a| a.proposta.inclusa) {
        let codice = &a.proposta.codice;
        if let (Some(e), Some(p)) = (
            ctx.progetto.elenco_prezzi.voce_per_codice(codice),
            ctx.prezzario.voce_per_codice(codice),
        ) && e.prezzo != p.prezzo
            && segnalati.insert(codice.clone())
        {
            avvisi.push(avviso(
                Gravita::Avviso,
                "prezzo_elenco_diverso",
                format!(
                    "la voce {codice} è già nell'elenco prezzi a {} invece di {} del prezzario: vale il prezzo dell'elenco",
                    e.prezzo, p.prezzo
                ),
                Some(&a.proposta.id),
            ));
        }
    }

    // Comandi.
    let mut comandi = Vec::new();
    let mut da_copiare: Vec<String> = Vec::new();
    for a in tenute.iter().filter(|a| a.proposta.inclusa) {
        let c = &a.proposta.codice;
        if ctx.progetto.elenco_prezzi.voce_per_codice(c).is_none() && !da_copiare.contains(c) {
            da_copiare.push(c.clone());
        }
    }
    if !da_copiare.is_empty() {
        comandi.push(Comando::CopiaDaPrezzario {
            prezzario: ctx.nome_prezzario.to_owned(),
            codici: da_copiare,
        });
    }
    for a in tenute.iter().filter(|a| a.proposta.inclusa) {
        comandi.push(Comando::AggiungiVoceComputo {
            computo: ctx.computo.map(str::to_owned),
            codice: a.proposta.codice.clone(),
            posizione: None,
            righi: a.righi.clone(),
            categorie: None,
            wbs: None,
            note: Some(nota(&a.proposta, &ipotesi)),
        });
    }

    let lavorazioni: Vec<LavorazioneProposta> = tenute.into_iter().map(|a| a.proposta).collect();
    let esito = if !lavorazioni.iter().any(|l| l.inclusa) {
        EsitoProposta::Impossibile
    } else if esito_modello == 0
        && escluse.is_empty()
        && ipotesi.is_empty()
        && risposta.informazioni_mancanti.is_empty()
        && avvisi.iter().all(|a| a.gravita == Gravita::Informazione)
    {
        EsitoProposta::Completo
    } else {
        EsitoProposta::Provvisorio
    };
    Ok(Proposta {
        esito,
        esito_modello: risposta.esito.trim().to_lowercase(),
        lavorazioni,
        escluse,
        ipotesi,
        informazioni_mancanti: risposta.informazioni_mancanti.clone(),
        avvisi,
        valori_fondati: fondati,
        comandi,
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema;

    #[test]
    fn schema_e_validazione_usano_gli_stessi_valori() {
        let parametri: Vec<&str> = PARAMETRI.iter().map(|(n, _)| *n).collect();
        assert_eq!(parametri, schema::PARAMETRI);
        let unita: Vec<&str> = UNITA.iter().map(|(n, _)| *n).collect();
        assert_eq!(unita, schema::UNITA);
        for (_, p) in PARAMETRI {
            assert!(
                Forma::TUTTE.iter().any(|f| f.parametri().contains(&p)),
                "{p:?} inutilizzato"
            );
        }
        for nome in schema::DIMENSIONI_ATTESE {
            let d = crate::candidati::dimensioni_attese(nome);
            assert_eq!(d.is_empty(), nome == "qualsiasi", "{nome}");
        }
    }

    #[test]
    fn note_troncate() {
        assert_eq!(tronca("abc", 10), "abc");
        assert_eq!(tronca("abcdef", 4), "abc…");
        assert_eq!(tronca("abcdef", 4).chars().count(), 4);
    }
}
