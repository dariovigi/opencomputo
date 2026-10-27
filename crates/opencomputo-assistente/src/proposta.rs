//! Validazione della risposta di selezione (ADR 0025).
//!
//! La risposta del modello non modifica nulla: diventa una [`Proposta`] in cui ogni
//! lavorazione ha
//!
//! * i righi di misura, comuni alle sue voci possibili, con forma, parametri e
//!   provenienza di ogni valore (dalla richiesta, ipotesi, da completare);
//! * le **opzioni**: fino a [`OPZIONI_MASSIME`] voci del prezzario, dalla più alla meno
//!   pertinente; per le lavorazioni chiare la prima è preselezionata, per quelle da
//!   precisare (manca il materiale, il tipo…) la scelta è dell'utente.
//!
//! I comandi non sono decisi qui: li costruisce [`crate::scelta`] dalle voci spuntate e
//! dalle misure, eventualmente corrette, dall'utente.
//!
//! La validazione scarta il meno possibile: un valore non valido diventa «da
//! completare», un'opzione incoerente è tolta, e una lavorazione è senza voce solo se non
//! resta nessuna opzione. Controlli, oltre a quelli dello schema:
//!
//! | Codice | Effetto | Quando |
//! |---|---|---|
//! | `candidato_inesistente` | opzione tolta | handle non inviato in questa richiesta |
//! | `opzione_ripetuta` | opzione tolta | stesso candidato due volte nella lavorazione |
//! | `unita_incompatibile` | opzione tolta | l'unità della voce misura una grandezza diversa dai righi |
//! | `misura_manuale` | opzione con rigo da completare | unità della voce senza forma di misura (h, m²/mese…) |
//! | `rigo_scartato` | rigo tolto | forma o segno sconosciuti, grandezza diversa dagli altri righi |
//! | `parametro_scartato` | parametro ignorato | parametro sconosciuto, ripetuto o non della forma |
//! | `valore_da_completare` | valore da completare | valore non numerico, zero, N non intero, unità non ammessa |
//! | `ipotesi_inesistente` | ipotesi creata | `rif_ipotesi` che non esiste |
//! | `valore_non_fondato` | valore reso ipotesi (avviso) | «utente» ma assente dalla richiesta |
//! | `prezzo_elenco_diverso` | informazione | la voce è già nell'elenco con un altro prezzo |
//!
//! Gli avvisi di gravità «informazione» sono dettagli della verifica; solo
//! `valore_non_fondato` è un avviso da guardare.

use std::collections::BTreeSet;

use opencomputo_core::numeri::Prezzo;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::validazione::Gravita;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::candidati::Candidati;
use crate::forme::{Forma, Parametro};
use crate::risposta::{InformazioneMancante, RispostaSelezione};
use crate::scelta::Scelta;
use crate::unita::{Dimensione, riconosci};
use crate::valori::{UnitaValore, ValoreFondato, estrai};

pub use crate::schema::OPZIONI_MASSIME;

/// Caratteri della descrizione estesa riportata in ogni opzione.
const DESCRIZIONE_ESTESA: usize = 600;

/// Esito della proposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EsitoProposta {
    /// Tutte le lavorazioni chiare, misure complete e fondate.
    Completo,
    /// Da guardare: lavorazioni da precisare, ipotesi, valori da completare, avvisi o
    /// lavorazioni senza voce.
    DaRivedere,
    /// Nessuna lavorazione con voci possibili.
    Vuoto,
}

/// Pertinenza di un'opzione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pertinenza {
    /// Alta.
    Alta,
    /// Media.
    Media,
    /// Bassa.
    Bassa,
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
    /// Non indicato: da completare.
    Mancante,
}

/// Parametro di un rigo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParametroProposto {
    /// Parametro.
    pub nome: Parametro,
    /// Valore come indicato (assente se da completare).
    pub valore: Option<Decimal>,
    /// Unità del valore.
    pub unita: UnitaValore,
    /// Provenienza.
    pub provenienza: ProvenienzaValore,
}

/// Rigo di misura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RigaProposta {
    /// Descrizione.
    pub descrizione: String,
    /// Forma.
    pub forma: Forma,
    /// Rigo in deduzione.
    pub deduzione: bool,
    /// Parametri, nell'ordine della forma (tutti presenti, eventualmente senza valore).
    pub parametri: Vec<ParametroProposto>,
}

impl RigaProposta {
    /// `true` se manca almeno un valore.
    #[must_use]
    pub fn da_completare(&self) -> bool {
        self.parametri.iter().any(|p| p.valore.is_none())
    }
}

/// Voce possibile per una lavorazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opzione {
    /// Handle del candidato.
    pub handle: String,
    /// Codice della voce.
    pub codice: String,
    /// Descrizione breve (o l'inizio dell'estesa).
    pub descrizione: String,
    /// Descrizione estesa, accorciata.
    pub descrizione_estesa: String,
    /// Unità di misura della voce.
    pub unita_misura: String,
    /// Prezzo che verrà applicato: quello dell'elenco prezzi se la voce c'è già.
    pub prezzo: Prezzo,
    /// Prezzo del prezzario, se diverso da quello dell'elenco.
    pub prezzo_prezzario: Option<Prezzo>,
    /// Pertinenza secondo il modello.
    pub pertinenza: Pertinenza,
    /// Motivazione del modello.
    pub motivazione: String,
    /// L'unità della voce non ha forme di misura: entra con un rigo da completare.
    pub misura_manuale: bool,
    /// Proposta come scelta iniziale.
    pub preselezionata: bool,
}

/// Lavorazione con misure e opzioni.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LavorazioneProposta {
    /// Identificativo.
    pub id: String,
    /// Descrizione.
    pub descrizione: String,
    /// `true` se la richiesta basta a scegliere la voce.
    pub chiara: bool,
    /// Cosa manca per scegliere (vuoto se chiara).
    pub da_precisare: String,
    /// Righi di misura, comuni alle opzioni.
    pub righe: Vec<RigaProposta>,
    /// Voci possibili, dalla più alla meno pertinente.
    pub opzioni: Vec<Opzione>,
    /// Ipotesi usate dai valori.
    pub ipotesi: Vec<String>,
}

impl LavorazioneProposta {
    /// `true` se qualche valore delle misure è da completare.
    #[must_use]
    pub fn da_completare(&self) -> bool {
        self.righe.iter().any(RigaProposta::da_completare)
    }
}

/// Lavorazione senza voci possibili nel prezzario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LavorazioneSenzaVoce {
    /// Identificativo.
    pub id: String,
    /// Descrizione.
    pub descrizione: String,
    /// Motivo.
    pub motivo: String,
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
    /// `true` se creata dalla validazione.
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
    /// Nome con cui il prezzario è caricato nella sessione.
    pub nome_prezzario: String,
    /// Computo di destinazione.
    pub computo: Option<String>,
    /// Lavorazioni con voci possibili.
    pub lavorazioni: Vec<LavorazioneProposta>,
    /// Lavorazioni senza voci possibili.
    pub senza_voce: Vec<LavorazioneSenzaVoce>,
    /// Ipotesi.
    pub ipotesi: Vec<Ipotesi>,
    /// Informazioni mancanti.
    pub informazioni_mancanti: Vec<InformazioneMancante>,
    /// Avvisi e dettagli della verifica.
    pub avvisi: Vec<Avviso>,
    /// Valori trovati nella richiesta.
    pub valori_fondati: Vec<ValoreFondato>,
}

impl Proposta {
    /// Scelte iniziali: le opzioni preselezionate, con le misure della proposta.
    #[must_use]
    pub fn scelte_predefinite(&self) -> Vec<Scelta> {
        self.lavorazioni
            .iter()
            .filter_map(|l| {
                let codici: Vec<String> = l
                    .opzioni
                    .iter()
                    .filter(|o| o.preselezionata)
                    .map(|o| o.codice.clone())
                    .collect();
                (!codici.is_empty()).then(|| Scelta {
                    lavorazione: l.id.clone(),
                    codici,
                    misure: None,
                })
            })
            .collect()
    }

    /// Lavorazione per identificativo.
    #[must_use]
    pub fn lavorazione(&self, id: &str) -> Option<&LavorazioneProposta> {
        self.lavorazioni
            .iter()
            .find(|l| l.id.eq_ignore_ascii_case(id.trim()))
    }
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
pub(crate) fn scegli<T: Copy>(testo: &str, valori: &[(&str, T)]) -> Option<T> {
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

pub(crate) const PARAMETRI: [(&str, Parametro); 10] = [
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

pub(crate) const UNITA: [(&str, UnitaValore); 7] = [
    ("n", UnitaValore::N),
    ("mm", UnitaValore::Mm),
    ("cm", UnitaValore::Cm),
    ("m", UnitaValore::M),
    ("m2", UnitaValore::M2),
    ("m3", UnitaValore::M3),
    ("kg/m", UnitaValore::KgM),
];

/// Unità predefinita di un parametro (quando quella indicata non è ammessa).
fn unita_predefinita(p: Parametro) -> UnitaValore {
    p.unita_ammesse()
        .iter()
        .copied()
        .find(|u| {
            matches!(
                u,
                UnitaValore::M
                    | UnitaValore::N
                    | UnitaValore::M2
                    | UnitaValore::M3
                    | UnitaValore::KgM
            )
        })
        .unwrap_or(UnitaValore::M)
}

fn avviso(gravita: Gravita, codice: &str, messaggio: String, lavorazione: Option<&str>) -> Avviso {
    Avviso {
        gravita,
        codice: format!("assistente.{codice}"),
        messaggio,
        lavorazione: lavorazione.map(str::to_owned),
    }
}

pub(crate) fn tronca(testo: &str, massimo: usize) -> String {
    if testo.chars().count() <= massimo {
        return testo.to_owned();
    }
    let mut t: String = testo.chars().take(massimo.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// Valori della validazione condivisi tra le lavorazioni.
struct Stato<'a> {
    ctx: &'a Contesto<'a>,
    fondati: Vec<ValoreFondato>,
    ipotesi_modello: BTreeSet<String>,
    ipotesi: Vec<Ipotesi>,
    avvisi: Vec<Avviso>,
}

impl Stato<'_> {
    fn nuova_ipotesi(&mut self, testo: String, impatto: Impatto) -> String {
        let id = format!(
            "R{:02}",
            self.ipotesi.iter().filter(|i| i.dalla_validazione).count() + 1
        );
        self.ipotesi.push(Ipotesi {
            id: id.clone(),
            testo,
            impatto,
            dalla_validazione: true,
        });
        id
    }

    /// Righi della lavorazione: i righi non utilizzabili sono tolti, i valori non validi
    /// diventano da completare.
    #[allow(clippy::too_many_lines)]
    fn righe(&mut self, l: &crate::risposta::LavorazioneSelezione) -> Vec<RigaProposta> {
        let id = l.id.trim().to_uppercase();
        let mut righe: Vec<RigaProposta> = Vec::new();
        for (n, r) in l.righe.iter().enumerate() {
            let nr = n + 1;
            let Some(f) = forma(&r.forma) else {
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "rigo_scartato",
                    format!("rigo {nr}: forma «{}» sconosciuta", r.forma),
                    Some(&id),
                ));
                continue;
            };
            if let Some(prima) = righe.first()
                && prima.forma.dimensione() != f.dimensione()
            {
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "rigo_scartato",
                    format!(
                        "rigo {nr}: la forma «{}» misura {}, i righi precedenti {}",
                        f.nome(),
                        f.dimensione().nome(),
                        prima.forma.dimensione().nome()
                    ),
                    Some(&id),
                ));
                continue;
            }
            let Some(deduzione) = scegli(&r.segno, &[("positivo", false), ("detrazione", true)])
            else {
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "rigo_scartato",
                    format!("rigo {nr}: segno «{}» sconosciuto", r.segno),
                    Some(&id),
                ));
                continue;
            };
            let mut parametri: Vec<ParametroProposto> = Vec::new();
            for p in &r.parametri {
                let Some(nome) = scegli(&p.nome, &PARAMETRI) else {
                    self.avvisi.push(avviso(
                        Gravita::Informazione,
                        "parametro_scartato",
                        format!("rigo {nr}: parametro «{}» sconosciuto", p.nome),
                        Some(&id),
                    ));
                    continue;
                };
                if !f.parametri().contains(&nome) || parametri.iter().any(|x| x.nome == nome) {
                    self.avvisi.push(avviso(
                        Gravita::Informazione,
                        "parametro_scartato",
                        format!(
                            "rigo {nr}: {nome:?} ripetuto o non previsto dalla forma «{}»",
                            f.nome()
                        ),
                        Some(&id),
                    ));
                    continue;
                }
                let unita = scegli(&p.unita, &UNITA).filter(|u| nome.unita_ammesse().contains(u));
                let testo = p.valore.trim();
                let mancante =
                    scegli(&p.provenienza, &[("mancante", ())]).is_some() || testo.is_empty();
                let valore = match (Decimal::from_str_exact(testo), unita) {
                    (Ok(v), Some(_))
                        if v > Decimal::ZERO
                            && (nome != Parametro::N || v.fract() == Decimal::ZERO) =>
                    {
                        Some(v)
                    }
                    _ => None,
                };
                let unita = unita.unwrap_or_else(|| unita_predefinita(nome));
                let Some(valore) = valore.filter(|_| !mancante) else {
                    if !mancante {
                        self.avvisi.push(avviso(
                            Gravita::Informazione,
                            "valore_da_completare",
                            format!(
                                "rigo {nr}: valore «{}» {} di {nome:?} non utilizzabile: da completare",
                                p.valore, p.unita
                            ),
                            Some(&id),
                        ));
                    }
                    parametri.push(ParametroProposto {
                        nome,
                        valore: None,
                        unita,
                        provenienza: ProvenienzaValore::Mancante,
                    });
                    continue;
                };
                let provenienza = if scegli(&p.provenienza, &[("utente", ())]).is_some() {
                    match self.fondati.iter().find(|v| v.corrisponde(valore, unita)) {
                        Some(v) => ProvenienzaValore::Utente {
                            testo: v.testo.clone(),
                        },
                        None if nome == Parametro::N && valore == Decimal::ONE => {
                            ProvenienzaValore::Utente {
                                testo: String::new(),
                            }
                        }
                        None => {
                            let testo = format!(
                                "{nome:?} = {valore} {} (lavorazione {id}, rigo {nr}) indicato come dato dell'utente, ma la richiesta non lo contiene",
                                p.unita
                            );
                            self.avvisi.push(avviso(
                                Gravita::Avviso,
                                "valore_non_fondato",
                                testo.clone(),
                                Some(&id),
                            ));
                            ProvenienzaValore::Ipotesi {
                                id: self.nuova_ipotesi(testo, Impatto::Alto),
                            }
                        }
                    }
                } else {
                    let rif = p.rif_ipotesi.trim().to_uppercase();
                    if self.ipotesi_modello.contains(&rif) {
                        ProvenienzaValore::Ipotesi { id: rif }
                    } else {
                        self.avvisi.push(avviso(
                            Gravita::Informazione,
                            "ipotesi_inesistente",
                            format!(
                                "rigo {nr}: {nome:?} rimanda all'ipotesi «{}» che non esiste",
                                p.rif_ipotesi
                            ),
                            Some(&id),
                        ));
                        let testo = format!(
                            "{nome:?} = {valore} {} (lavorazione {id}, rigo {nr}) ipotizzato senza spiegazione",
                            p.unita
                        );
                        ProvenienzaValore::Ipotesi {
                            id: self.nuova_ipotesi(testo, Impatto::Medio),
                        }
                    }
                };
                parametri.push(ParametroProposto {
                    nome,
                    valore: Some(valore),
                    unita,
                    provenienza,
                });
            }
            // Ogni parametro della forma c'è: quelli non indicati sono da completare.
            for &nome in f.parametri() {
                if !parametri.iter().any(|p| p.nome == nome) {
                    parametri.push(ParametroProposto {
                        nome,
                        valore: None,
                        unita: unita_predefinita(nome),
                        provenienza: ProvenienzaValore::Mancante,
                    });
                }
            }
            parametri.sort_by_key(|p| f.parametri().iter().position(|x| *x == p.nome));
            righe.push(RigaProposta {
                descrizione: r.descrizione.trim().to_owned(),
                forma: f,
                deduzione,
                parametri,
            });
        }
        righe
    }

    /// Opzioni utilizzabili, in ordine di pertinenza.
    fn opzioni(
        &mut self,
        l: &crate::risposta::LavorazioneSelezione,
        dimensione: Option<Dimensione>,
        chiara: bool,
    ) -> (Vec<Opzione>, Vec<String>) {
        let id = l.id.trim().to_uppercase();
        let mut opzioni: Vec<Opzione> = Vec::new();
        let mut scarti: Vec<String> = Vec::new();
        for o in &l.opzioni {
            let Some(c) = self.ctx.candidati.per_handle(&o.candidato) else {
                scarti.push(format!("«{}» non è tra i candidati inviati", o.candidato));
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "candidato_inesistente",
                    format!(
                        "opzione «{}» non tra i candidati inviati: tolta",
                        o.candidato
                    ),
                    Some(&id),
                ));
                continue;
            };
            if opzioni.iter().any(|x| x.codice == c.codice) {
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "opzione_ripetuta",
                    format!("voce {} proposta due volte", c.codice),
                    Some(&id),
                ));
                continue;
            }
            let elenco = self.ctx.progetto.elenco_prezzi.voce_per_codice(&c.codice);
            let prezzario = self.ctx.prezzario.voce_per_codice(&c.codice);
            let Some(voce) = elenco.or(prezzario) else {
                scarti.push(format!("la voce {} non è più nel prezzario", c.codice));
                continue;
            };
            let u = riconosci(&voce.unita_misura);
            let misura_manuale = Forma::per_dimensione(u.dimensione).is_empty();
            if let Some(d) = dimensione
                && !misura_manuale
                && u.dimensione != d
            {
                let m = format!(
                    "{} è in «{}» ({}), le misure danno {}",
                    voce.codice,
                    voce.unita_misura,
                    u.dimensione.nome(),
                    d.nome()
                );
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "unita_incompatibile",
                    format!("opzione tolta: {m}"),
                    Some(&id),
                ));
                scarti.push(m);
                continue;
            }
            if misura_manuale {
                self.avvisi.push(avviso(
                    Gravita::Informazione,
                    "misura_manuale",
                    format!(
                        "{}: l'unità «{}» non ha forme di misura, la quantità va inserita nel computo",
                        voce.codice, voce.unita_misura
                    ),
                    Some(&id),
                ));
            }
            let prezzo_prezzario = match (elenco, prezzario) {
                (Some(e), Some(p)) if e.prezzo != p.prezzo => {
                    self.avvisi.push(avviso(
                        Gravita::Informazione,
                        "prezzo_elenco_diverso",
                        format!(
                            "la voce {} è già nell'elenco prezzi a {} invece di {} del prezzario: vale il prezzo dell'elenco",
                            voce.codice, e.prezzo, p.prezzo
                        ),
                        Some(&id),
                    ));
                    Some(p.prezzo)
                }
                _ => None,
            };
            let breve = voce.descrizione_breve.trim();
            opzioni.push(Opzione {
                handle: c.handle.clone(),
                codice: voce.codice.clone(),
                descrizione: if breve.is_empty() {
                    tronca(voce.descrizione_estesa.trim(), 160)
                } else {
                    breve.to_owned()
                },
                descrizione_estesa: tronca(voce.descrizione_estesa.trim(), DESCRIZIONE_ESTESA),
                unita_misura: voce.unita_misura.clone(),
                prezzo: voce.prezzo,
                prezzo_prezzario,
                pertinenza: scegli(
                    &o.pertinenza,
                    &[
                        ("alta", Pertinenza::Alta),
                        ("media", Pertinenza::Media),
                        ("bassa", Pertinenza::Bassa),
                    ],
                )
                .unwrap_or(Pertinenza::Bassa),
                motivazione: o.motivazione.trim().to_owned(),
                misura_manuale,
                preselezionata: false,
            });
        }
        // Ordine di pertinenza, a parità l'ordine del modello.
        opzioni.sort_by_key(|o| o.pertinenza);
        opzioni.truncate(OPZIONI_MASSIME);
        if chiara && let Some(prima) = opzioni.first_mut() {
            prima.preselezionata = true;
        }
        (opzioni, scarti)
    }
}

/// Valida una risposta di selezione e la traduce in una proposta.
///
/// # Errors
/// Identificativi ripetuti: la risposta va richiesta di nuovo.
pub fn valida_selezione(
    risposta: &RispostaSelezione,
    ctx: &Contesto<'_>,
) -> Result<Proposta, ErroreRisposta> {
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
    let mut st = Stato {
        ctx,
        fondati: estrai(ctx.richiesta),
        ipotesi_modello: risposta
            .ipotesi
            .iter()
            .map(|i| i.id.trim().to_uppercase())
            .collect(),
        ipotesi: risposta
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
            .collect(),
        avvisi: Vec::new(),
    };
    let mut lavorazioni = Vec::new();
    let mut senza_voce = Vec::new();
    for l in &risposta.lavorazioni {
        let id = l.id.trim().to_uppercase();
        let chiara =
            scegli(&l.chiarezza, &[("chiara", true), ("da_precisare", false)]).unwrap_or(false);
        let righe = st.righe(l);
        let dimensione = righe.first().map(|r| r.forma.dimensione());
        let (opzioni, scarti) = st.opzioni(l, dimensione, chiara);
        if opzioni.is_empty() {
            senza_voce.push(LavorazioneSenzaVoce {
                id,
                descrizione: l.descrizione.trim().to_owned(),
                motivo: if scarti.is_empty() {
                    "nessuna voce pertinente nel prezzario: va scelta a mano o creato un nuovo prezzo".into()
                } else {
                    format!("nessuna voce utilizzabile ({})", scarti.join("; "))
                },
            });
            continue;
        }
        let ipotesi: BTreeSet<String> = righe
            .iter()
            .flat_map(|r| &r.parametri)
            .filter_map(|p| match &p.provenienza {
                ProvenienzaValore::Ipotesi { id } => Some(id.clone()),
                _ => None,
            })
            .collect();
        let da_precisare = l.da_precisare.trim();
        lavorazioni.push(LavorazioneProposta {
            id,
            descrizione: l.descrizione.trim().to_owned(),
            chiara,
            da_precisare: if chiara {
                String::new()
            } else if da_precisare.is_empty() {
                "scegliere tra le opzioni".into()
            } else {
                da_precisare.to_owned()
            },
            righe,
            opzioni,
            ipotesi: ipotesi.into_iter().collect(),
        });
    }
    // Solo le ipotesi usate restano.
    let usate: BTreeSet<&String> = lavorazioni.iter().flat_map(|l| &l.ipotesi).collect();
    let ipotesi: Vec<Ipotesi> = st
        .ipotesi
        .iter()
        .filter(|i| usate.contains(&i.id))
        .cloned()
        .collect();
    let esito = if lavorazioni.is_empty() {
        EsitoProposta::Vuoto
    } else if senza_voce.is_empty()
        && ipotesi.is_empty()
        && risposta.informazioni_mancanti.is_empty()
        && lavorazioni.iter().all(|l| l.chiara && !l.da_completare())
        && st.avvisi.iter().all(|a| a.gravita == Gravita::Informazione)
    {
        EsitoProposta::Completo
    } else {
        EsitoProposta::DaRivedere
    };
    Ok(Proposta {
        esito,
        nome_prezzario: ctx.nome_prezzario.to_owned(),
        computo: ctx.computo.map(str::to_owned),
        lavorazioni,
        senza_voce,
        ipotesi,
        informazioni_mancanti: risposta.informazioni_mancanti.clone(),
        avvisi: st.avvisi,
        valori_fondati: st.fondati,
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
            assert!(p.unita_ammesse().contains(&unita_predefinita(p)), "{p:?}");
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
