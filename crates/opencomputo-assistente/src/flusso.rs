//! Workflow B completo: dalla richiesta alla proposta validata (ADR 0025).
//!
//! 1. analisi della richiesta (prima chiamata al modello, schema [`schema::schema_analisi`]);
//!    il flusso si ferma con le domande per l'utente solo se non c'è nessuna lavorazione
//!    riconoscibile; altrimenti le informazioni mancanti diventano opzioni da scegliere e
//!    domande accanto alla proposta;
//! 2. ricerca dei candidati per ogni lavorazione: testuale e, se c'è l'indice vettoriale
//!    del prezzario, vettoriale (un solo lotto di embedding per tutte le domande);
//! 3. voci possibili in ordine di pertinenza e righi (seconda chiamata, schema
//!    [`schema::schema_selezione`]);
//! 4. validazione ([`valida_selezione`]); i comandi nascono dalle scelte dell'utente
//!    ([`crate::scelta`]).
//!
//! I provider entrano come tratti: nei test sono sostituti deterministici.

use std::collections::BTreeSet;

use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use serde::Serialize;
use thiserror::Error;

use crate::candidati::Candidati;
use crate::prompt;
use crate::proposta::{Contesto, ErroreRisposta, Proposta, valida_selezione};
use crate::provider::anthropic::Chiamata;
use crate::provider::{Annullamento, Embedding, ErroreAi, Generazione};
use crate::ricerca::IndiceTestuale;
use crate::risposta::{RispostaAnalisi, RispostaSelezione};
use crate::schema::{self, STRUMENTO_ANALISI, STRUMENTO_SELEZIONE};
use crate::vettori::IndiceVettoriale;

/// Parametri del flusso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parametri {
    /// Limite di token della risposta di analisi.
    pub max_token_analisi: u32,
    /// Limite di token della risposta di selezione.
    pub max_token_selezione: u32,
    /// Candidati per lavorazione.
    pub candidati_per_lavorazione: usize,
    /// Candidati in tutto.
    pub candidati_massimi: usize,
}

impl Default for Parametri {
    fn default() -> Self {
        Self {
            max_token_analisi: 8_000,
            max_token_selezione: 16_000,
            candidati_per_lavorazione: 8,
            candidati_massimi: 40,
        }
    }
}

/// Strumenti del flusso.
#[derive(Clone, Copy)]
pub struct Strumenti<'a> {
    /// Generazione.
    pub generazione: &'a dyn Generazione,
    /// Embedding delle domande (solo con l'indice vettoriale).
    pub embedding: Option<&'a dyn Embedding>,
    /// Indice testuale del prezzario.
    pub indice_testo: &'a IndiceTestuale,
    /// Indice vettoriale del prezzario.
    pub indice_vettori: Option<&'a IndiceVettoriale>,
}

impl std::fmt::Debug for Strumenti<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Strumenti")
            .field("embedding", &self.embedding.is_some())
            .field(
                "indice_vettori",
                &self.indice_vettori.map(IndiceVettoriale::len),
            )
            .finish_non_exhaustive()
    }
}

/// Dati della richiesta.
#[derive(Debug, Clone, Copy)]
pub struct Richiesta<'a> {
    /// Testo dell'utente.
    pub testo: &'a str,
    /// Nome con cui il prezzario è caricato nella sessione.
    pub nome_prezzario: &'a str,
    /// Prezzario.
    pub prezzario: &'a Prezzario,
    /// Progetto corrente.
    pub progetto: &'a Progetto,
    /// Computo di destinazione.
    pub computo: Option<&'a str>,
}

/// Token usati dalle chiamate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Consumi {
    /// Token in ingresso (generazione).
    pub token_ingresso: u64,
    /// Token in uscita (generazione).
    pub token_uscita: u64,
    /// Testi inviati per gli embedding.
    pub testi_embedding: usize,
}

/// Esito del flusso.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "esito", rename_all = "snake_case")]
pub enum EsitoFlusso {
    /// Nessuna lavorazione riconoscibile: rispondere alle domande e ripetere la richiesta.
    DaChiarire {
        /// Analisi con le informazioni mancanti.
        analisi: RispostaAnalisi,
        /// Consumi.
        consumi: Consumi,
    },
    /// Proposta validata.
    Proposta {
        /// Analisi.
        analisi: RispostaAnalisi,
        /// Candidati inviati al modello.
        candidati: Candidati,
        /// `true` se la ricerca ha usato anche l'indice vettoriale.
        ricerca_ibrida: bool,
        /// Proposta.
        proposta: Box<Proposta>,
        /// Consumi.
        consumi: Consumi,
    },
}

/// Errore del flusso.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "codice", rename_all = "snake_case")]
pub enum ErroreFlusso {
    /// Errore di un provider.
    #[error(transparent)]
    Ai(ErroreAi),
    /// Risposta di analisi inutilizzabile.
    #[error("analisi non utilizzabile: {motivo}")]
    Analisi {
        /// Motivo.
        motivo: String,
    },
    /// Risposta di selezione inutilizzabile.
    #[error(transparent)]
    Selezione(ErroreRisposta),
    /// Nessun candidato trovato nel prezzario.
    #[error("nessuna voce del prezzario corrisponde alle lavorazioni richieste")]
    NessunCandidato,
}

impl From<ErroreAi> for ErroreFlusso {
    fn from(e: ErroreAi) -> Self {
        Self::Ai(e)
    }
}

/// Controlla la risposta di analisi.
fn controlla_analisi(a: &RispostaAnalisi) -> Result<bool, ErroreFlusso> {
    let chiedi = match a.esito.trim().to_lowercase().as_str() {
        "procedi" => false,
        "chiedi" => true,
        altro => {
            return Err(ErroreFlusso::Analisi {
                motivo: format!("esito «{altro}» sconosciuto"),
            });
        }
    };
    let mut visti = BTreeSet::new();
    for l in &a.lavorazioni {
        if !visti.insert(l.id.trim().to_uppercase()) {
            return Err(ErroreFlusso::Analisi {
                motivo: format!("lavorazione «{}» ripetuta", l.id),
            });
        }
    }
    if !chiedi && a.lavorazioni.is_empty() {
        return Err(ErroreFlusso::Analisi {
            motivo: "nessuna lavorazione individuata".into(),
        });
    }
    Ok(chiedi)
}

/// Esegue il flusso completo.
///
/// # Errors
/// Errore di un provider, risposta inutilizzabile, nessun candidato, annullamento.
pub fn proponi(
    s: &Strumenti<'_>,
    r: &Richiesta<'_>,
    parametri: &Parametri,
    annullamento: &Annullamento,
) -> Result<EsitoFlusso, ErroreFlusso> {
    let mut consumi = Consumi::default();
    let analisi = s.generazione.genera(
        &Chiamata {
            sistema: prompt::sistema_analisi(),
            utente: prompt::utente_analisi(r.testo),
            schema: schema::schema_analisi(),
            nome: STRUMENTO_ANALISI.into(),
            descrizione: "Registra le lavorazioni richieste e le informazioni mancanti.".into(),
            max_tokens: parametri.max_token_analisi,
        },
        annullamento,
    )?;
    consumi.token_ingresso += analisi.token_ingresso;
    consumi.token_uscita += analisi.token_uscita;
    let analisi: RispostaAnalisi =
        serde_json::from_value(analisi.json).map_err(|e| ErroreFlusso::Analisi {
            motivo: e.to_string(),
        })?;
    if controlla_analisi(&analisi)? && analisi.lavorazioni.is_empty() {
        return Ok(EsitoFlusso::DaChiarire { analisi, consumi });
    }

    // Embedding delle domande, solo se c'è un indice dello stesso spazio.
    let vettori_domande = match (s.embedding, s.indice_vettori) {
        (Some(e), Some(iv)) if !iv.is_empty() && &e.spazio() == iv.spazio() => {
            let domande: Vec<String> = analisi
                .lavorazioni
                .iter()
                .map(|l| {
                    let q = l.query_ricerca.trim();
                    if q.is_empty() {
                        l.descrizione.trim().to_owned()
                    } else {
                        q.to_owned()
                    }
                })
                .map(|q| {
                    if q.is_empty() {
                        "lavorazione".to_owned()
                    } else {
                        q
                    }
                })
                .collect();
            consumi.testi_embedding += domande.len();
            Some((iv, e.embed(&domande, annullamento)?))
        }
        _ => None,
    };
    let candidati = Candidati::da_analisi_ibrida(
        s.indice_testo,
        vettori_domande.as_ref().map(|(iv, v)| (*iv, v.as_slice())),
        r.prezzario,
        &analisi,
        parametri.candidati_per_lavorazione,
        parametri.candidati_massimi,
    );
    if candidati.elenco.is_empty() {
        return Err(ErroreFlusso::NessunCandidato);
    }
    if annullamento.annullato() {
        return Err(ErroreFlusso::Ai(ErroreAi::Annullato));
    }

    let selezione = s.generazione.genera(
        &Chiamata {
            sistema: prompt::sistema_selezione(),
            utente: prompt::utente_selezione(r.testo, &analisi, &candidati),
            schema: schema::schema_selezione(),
            nome: STRUMENTO_SELEZIONE.into(),
            descrizione:
                "Registra le voci possibili, i righi di misura, le ipotesi e le informazioni mancanti."
                    .into(),
            max_tokens: parametri.max_token_selezione,
        },
        annullamento,
    )?;
    consumi.token_ingresso += selezione.token_ingresso;
    consumi.token_uscita += selezione.token_uscita;
    let selezione: RispostaSelezione =
        crate::proposta::leggi_selezione(&selezione.json).map_err(ErroreFlusso::Selezione)?;
    let mut proposta = valida_selezione(
        &selezione,
        &Contesto {
            richiesta: r.testo,
            nome_prezzario: r.nome_prezzario,
            prezzario: r.prezzario,
            candidati: &candidati,
            progetto: r.progetto,
            computo: r.computo,
        },
    )
    .map_err(ErroreFlusso::Selezione)?;
    // Le domande dell'analisi restano, senza ripetere quelle della selezione.
    for i in &analisi.informazioni_mancanti {
        if !proposta
            .informazioni_mancanti
            .iter()
            .any(|x| x.domanda.trim() == i.domanda.trim())
        {
            proposta.informazioni_mancanti.push(i.clone());
        }
    }
    if !proposta.informazioni_mancanti.is_empty()
        && proposta.esito == crate::proposta::EsitoProposta::Completo
    {
        proposta.esito = crate::proposta::EsitoProposta::DaRivedere;
    }
    Ok(EsitoFlusso::Proposta {
        analisi,
        candidati,
        ricerca_ibrida: vettori_domande.is_some(),
        proposta: Box::new(proposta),
        consumi,
    })
}
