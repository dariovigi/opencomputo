//! Aggiornamento dell'elenco prezzi a un nuovo listino (ADR 0022).
//!
//! Il [`confronto`](confronta_listino) mette a fianco ogni voce dell'elenco prezzi e
//! ogni risorsa delle analisi con la voce del listino che ha lo stesso codice
//! (eventualmente con il prefisso sostituito, es. `TOS24_` al posto di `TOS23_`). Per
//! ogni voce riporta le differenze, la variazione del prezzo e l'effetto sugli importi
//! dei computi. L'[`aggiornamento`](aggiorna_da_listino) applica solo le voci e le
//! risorse scelte, nei campi scelti. Poi ricalcola le voci analizzate e riallinea le
//! voci dei computi collegate alle voci cambiate.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::analisi::TipoRisorsa;
use crate::errore::ErroreDominio;
use crate::id::{ComputoId, RisorsaId, VocePrezzarioId};
use crate::numeri::{Importo, Percentuale, Prezzo};
use crate::partita::{CampoPrezzo, DifferenzaPrezzo, PrezzoApplicato};
use crate::prezzario::{Prezzario, VocePrezzario};
use crate::progetto::Progetto;

/// Sostituzione del prefisso dei codici: una voce dell'elenco con codice che comincia
/// per `da` si cerca nel listino con `a` al suo posto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SostituzionePrefisso {
    /// Prefisso dei codici dell'elenco.
    pub da: String,
    /// Prefisso dei codici del listino.
    #[serde(default)]
    pub a: String,
}

impl SostituzionePrefisso {
    /// Codice da cercare nel listino.
    #[must_use]
    pub fn applica(&self, codice: &str) -> String {
        match codice.strip_prefix(self.da.as_str()) {
            Some(resto) if !self.da.is_empty() => format!("{}{resto}", self.a),
            _ => codice.to_owned(),
        }
    }
}

/// Codice da cercare nel listino, con la sostituzione del prefisso se indicata.
fn codice_listino(codice: &str, prefisso: Option<&SostituzionePrefisso>) -> String {
    prefisso.map_or_else(|| codice.to_owned(), |p| p.applica(codice))
}

/// Esito del confronto di una voce o di una risorsa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EsitoConfronto {
    /// Nel listino con almeno un dato diverso.
    Variata,
    /// Nel listino con gli stessi dati.
    Invariata,
    /// Codice assente dal listino.
    NonTrovata,
    /// Voce con analisi: il prezzo segue l'analisi (si aggiornano le risorse).
    Analizzata,
}

/// Confronto di una voce dell'elenco prezzi con il listino.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfrontoVoce {
    /// Voce dell'elenco.
    pub voce: VocePrezzarioId,
    /// Codice nell'elenco.
    pub codice: String,
    /// Descrizione breve nell'elenco.
    pub descrizione_breve: String,
    /// Unità di misura nell'elenco.
    pub unita_misura: String,
    /// Codice cercato nel listino.
    pub codice_listino: String,
    /// Voce corrispondente del listino.
    pub listino: Option<VocePrezzarioId>,
    /// Esito.
    pub esito: EsitoConfronto,
    /// Prezzo nell'elenco.
    pub prezzo: Prezzo,
    /// Prezzo nel listino.
    pub prezzo_listino: Option<Prezzo>,
    /// Variazione percentuale del prezzo (due decimali).
    pub variazione: Option<Decimal>,
    /// Dati diversi: valore dell'elenco e valore del listino.
    pub differenze: Vec<DifferenzaPrezzo>,
    /// Voci dei computi che usano la voce.
    pub voci_computo: usize,
    /// Somma degli importi di quelle voci (con i prezzi applicati oggi).
    pub importo_computi: Importo,
    /// Gli stessi importi con il prezzo del listino.
    pub importo_computi_listino: Option<Importo>,
}

/// Confronto di una risorsa delle analisi con il listino.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfrontoRisorsa {
    /// Risorsa.
    pub risorsa: RisorsaId,
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo.
    pub tipo: TipoRisorsa,
    /// Codice cercato nel listino.
    pub codice_listino: String,
    /// Esito (mai [`EsitoConfronto::Analizzata`]).
    pub esito: EsitoConfronto,
    /// Prezzo attuale.
    pub prezzo: Prezzo,
    /// Prezzo nel listino.
    pub prezzo_listino: Option<Prezzo>,
    /// Variazione percentuale (due decimali).
    pub variazione: Option<Decimal>,
    /// Dati diversi (codice, descrizione, unità di misura, prezzo).
    pub differenze: Vec<DifferenzaPrezzo>,
    /// Analisi che la usano.
    pub analisi: usize,
}

/// Confronto completo dell'elenco prezzi con un listino.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfrontoListino {
    /// Voci dell'elenco, nel loro ordine.
    pub voci: Vec<ConfrontoVoce>,
    /// Risorse delle analisi, nel loro ordine.
    pub risorse: Vec<ConfrontoRisorsa>,
    /// Voci del listino senza corrispondenza nell'elenco.
    pub voci_solo_nel_listino: usize,
    /// Codici presenti più volte nel listino (vale la prima voce).
    pub codici_ripetuti: Vec<String>,
    /// Computi non calcolabili, esclusi dagli importi.
    pub computi_non_calcolabili: Vec<String>,
}

/// Dati di listino di una risorsa: dalla risorsa con lo stesso codice o, in mancanza,
/// dalla voce.
struct DatiListino<'a> {
    codice: &'a str,
    descrizione: &'a str,
    unita_misura: &'a str,
    prezzo: Prezzo,
}

fn dati_risorsa<'a>(listino: &'a Prezzario, codice: &str) -> Option<DatiListino<'a>> {
    listino
        .risorse
        .iter()
        .find(|r| r.codice == codice)
        .map(|r| DatiListino {
            codice: &r.codice,
            descrizione: &r.descrizione,
            unita_misura: &r.unita_misura,
            prezzo: r.prezzo,
        })
        .or_else(|| {
            listino.voce_per_codice(codice).map(|v| DatiListino {
                codice: &v.codice,
                descrizione: &v.descrizione_breve,
                unita_misura: &v.unita_misura,
                prezzo: v.prezzo,
            })
        })
}

/// Variazione percentuale da `prima` a `dopo`, a due decimali.
fn variazione(prima: Prezzo, dopo: Prezzo) -> Option<Decimal> {
    let p = prima.valore();
    if p.is_zero() {
        return None;
    }
    let v = dopo
        .valore()
        .checked_sub(p)?
        .checked_mul(Decimal::ONE_HUNDRED)?
        .checked_div(p)?;
    Some(v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero))
}

fn testo_percentuale(p: Option<Percentuale>) -> String {
    p.map(|v| v.to_string()).unwrap_or_default()
}

/// Differenze tra una voce dell'elenco e una del listino, nei campi aggiornabili.
fn differenze_voce(e: &VocePrezzario, l: &VocePrezzario) -> Vec<DifferenzaPrezzo> {
    let mut d = Vec::new();
    let mut aggiungi = |campo, a: String, b: String| {
        if a != b {
            d.push(DifferenzaPrezzo {
                campo,
                applicato: a,
                prezzario: b,
            });
        }
    };
    aggiungi(CampoPrezzo::Codice, e.codice.clone(), l.codice.clone());
    aggiungi(
        CampoPrezzo::DescrizioneBreve,
        e.descrizione_breve.clone(),
        l.descrizione_breve.clone(),
    );
    aggiungi(
        CampoPrezzo::DescrizioneEstesa,
        e.descrizione_estesa.clone(),
        l.descrizione_estesa.clone(),
    );
    aggiungi(
        CampoPrezzo::UnitaMisura,
        e.unita_misura.clone(),
        l.unita_misura.clone(),
    );
    if e.prezzo != l.prezzo {
        aggiungi(
            CampoPrezzo::Prezzo,
            e.prezzo.to_string(),
            l.prezzo.to_string(),
        );
    }
    if e.incidenza_manodopera != l.incidenza_manodopera {
        aggiungi(
            CampoPrezzo::IncidenzaManodopera,
            testo_percentuale(e.incidenza_manodopera),
            testo_percentuale(l.incidenza_manodopera),
        );
    }
    if e.incidenza_sicurezza != l.incidenza_sicurezza {
        aggiungi(
            CampoPrezzo::IncidenzaSicurezza,
            testo_percentuale(e.incidenza_sicurezza),
            testo_percentuale(l.incidenza_sicurezza),
        );
    }
    d
}

/// Voce dell'elenco da cui viene un prezzo applicato: per identificativo d'origine o,
/// in mancanza, per codice.
fn voce_di(
    elenco: &Prezzario,
    per_codice: &HashMap<&str, VocePrezzarioId>,
    prezzo: &PrezzoApplicato,
) -> Option<VocePrezzarioId> {
    prezzo
        .origine
        .filter(|o| o.prezzario == elenco.id && elenco.voce(o.voce).is_some())
        .map(|o| o.voce)
        .or_else(|| per_codice.get(prezzo.codice.as_str()).copied())
}

/// Indice dei codici di un listino (prima voce per codice) e codici ripetuti.
fn indice(listino: &Prezzario) -> (HashMap<&str, &VocePrezzario>, Vec<String>) {
    let mut per_codice = HashMap::with_capacity(listino.voci.len());
    let mut ripetuti = Vec::new();
    for v in &listino.voci {
        if per_codice.contains_key(v.codice.as_str()) {
            if !ripetuti.contains(&v.codice) {
                ripetuti.push(v.codice.clone());
            }
        } else {
            per_codice.insert(v.codice.as_str(), &**v);
        }
    }
    (per_codice, ripetuti)
}

/// Confronta l'elenco prezzi e le risorse del progetto con un listino.
#[must_use]
pub fn confronta_listino(
    p: &Progetto,
    listino: &Prezzario,
    prefisso: Option<&SostituzionePrefisso>,
) -> ConfrontoListino {
    let elenco = &p.elenco_prezzi;
    let regole = &p.regole;
    let (nel_listino, codici_ripetuti) = indice(listino);
    let per_codice: HashMap<&str, VocePrezzarioId> = elenco
        .voci
        .iter()
        .map(|v| (v.codice.as_str(), v.id))
        .collect();

    // Voci dei computi per voce dell'elenco: quantità e importo con il prezzo applicato.
    let mut uso: HashMap<VocePrezzarioId, Vec<(crate::numeri::Quantita, Importo)>> = HashMap::new();
    let mut computi_non_calcolabili = Vec::new();
    for c in &p.computi {
        let Ok(r) = c.calcola(regole) else {
            computi_non_calcolabili.push(c.nome.clone());
            continue;
        };
        for (v, ris) in c.voci.iter().zip(&r.voci) {
            if let Some(id) = voce_di(elenco, &per_codice, &v.prezzo) {
                uso.entry(id)
                    .or_default()
                    .push((ris.calcolo.quantita, ris.calcolo.importo));
            }
        }
    }

    let mut usate: HashSet<&str> = HashSet::new();
    let voci = elenco
        .voci
        .iter()
        .map(|v| {
            let codice = codice_listino(&v.codice, prefisso);
            let corrispondente = nel_listino.get(codice.as_str()).copied();
            if let Some(l) = corrispondente {
                usate.insert(l.codice.as_str());
            }
            let righe = uso.get(&v.id).map(Vec::as_slice).unwrap_or_default();
            let importo_computi = righe
                .iter()
                .try_fold(Importo::ZERO, |t, (_, i)| t.somma(*i))
                .unwrap_or(Importo::ZERO);
            let importo_computi_listino = corrispondente.and_then(|l| {
                righe.iter().try_fold(Importo::ZERO, |t, (q, _)| {
                    t.somma(l.prezzo.per_quantita(*q, regole).ok()?).ok()
                })
            });
            let differenze = corrispondente.map_or_else(Vec::new, |l| differenze_voce(v, l));
            let esito = match corrispondente {
                None => EsitoConfronto::NonTrovata,
                Some(_) if v.analisi.is_some() => EsitoConfronto::Analizzata,
                Some(_) if differenze.is_empty() => EsitoConfronto::Invariata,
                Some(_) => EsitoConfronto::Variata,
            };
            ConfrontoVoce {
                voce: v.id,
                codice: v.codice.clone(),
                descrizione_breve: v.descrizione_breve.clone(),
                unita_misura: v.unita_misura.clone(),
                codice_listino: codice,
                listino: corrispondente.map(|l| l.id),
                esito,
                prezzo: v.prezzo,
                prezzo_listino: corrispondente.map(|l| l.prezzo),
                variazione: corrispondente.and_then(|l| variazione(v.prezzo, l.prezzo)),
                differenze,
                voci_computo: righe.len(),
                importo_computi,
                importo_computi_listino,
            }
        })
        .collect();

    let mut analisi_per_risorsa: BTreeMap<RisorsaId, usize> = BTreeMap::new();
    for a in &elenco.analisi {
        let mut viste = HashSet::new();
        for r in &a.righe {
            if let Some(crate::analisi::OrigineRigaAnalisi::Risorsa(id)) = r.origine
                && viste.insert(id)
            {
                *analisi_per_risorsa.entry(id).or_default() += 1;
            }
        }
    }
    let risorse = elenco
        .risorse
        .iter()
        .map(|r| {
            let codice = codice_listino(&r.codice, prefisso);
            let dati = dati_risorsa(listino, &codice);
            let mut differenze = Vec::new();
            if let Some(l) = &dati {
                usate.insert(l.codice);
                let mut aggiungi = |campo, a: &str, b: &str| {
                    if a != b {
                        differenze.push(DifferenzaPrezzo {
                            campo,
                            applicato: a.to_owned(),
                            prezzario: b.to_owned(),
                        });
                    }
                };
                aggiungi(CampoPrezzo::Codice, &r.codice, l.codice);
                aggiungi(CampoPrezzo::DescrizioneBreve, &r.descrizione, l.descrizione);
                aggiungi(CampoPrezzo::UnitaMisura, &r.unita_misura, l.unita_misura);
                if r.prezzo != l.prezzo {
                    differenze.push(DifferenzaPrezzo {
                        campo: CampoPrezzo::Prezzo,
                        applicato: r.prezzo.to_string(),
                        prezzario: l.prezzo.to_string(),
                    });
                }
            }
            ConfrontoRisorsa {
                risorsa: r.id,
                codice: r.codice.clone(),
                descrizione: r.descrizione.clone(),
                unita_misura: r.unita_misura.clone(),
                tipo: r.tipo,
                codice_listino: codice,
                esito: match &dati {
                    None => EsitoConfronto::NonTrovata,
                    Some(_) if differenze.is_empty() => EsitoConfronto::Invariata,
                    Some(_) => EsitoConfronto::Variata,
                },
                prezzo: r.prezzo,
                prezzo_listino: dati.as_ref().map(|l| l.prezzo),
                variazione: dati.as_ref().and_then(|l| variazione(r.prezzo, l.prezzo)),
                differenze,
                analisi: analisi_per_risorsa.get(&r.id).copied().unwrap_or(0),
            }
        })
        .collect();

    ConfrontoListino {
        voci,
        risorse,
        voci_solo_nel_listino: listino
            .voci
            .iter()
            .filter(|v| !usate.contains(v.codice.as_str()))
            .count(),
        codici_ripetuti,
        computi_non_calcolabili,
    }
}

/// Campi da aggiornare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampiAggiornamento {
    /// Prezzo.
    #[serde(default = "vero")]
    pub prezzo: bool,
    /// Incidenze di manodopera e sicurezza (solo voci).
    #[serde(default = "vero")]
    pub incidenze: bool,
    /// Descrizioni breve ed estesa.
    #[serde(default)]
    pub descrizioni: bool,
    /// Unità di misura.
    #[serde(default)]
    pub unita_misura: bool,
    /// Codice (con la sostituzione del prefisso: le voci prendono il codice del
    /// listino).
    #[serde(default)]
    pub codice: bool,
}

fn vero() -> bool {
    true
}

impl Default for CampiAggiornamento {
    fn default() -> Self {
        Self {
            prezzo: true,
            incidenze: true,
            descrizioni: false,
            unita_misura: false,
            codice: false,
        }
    }
}

/// Errore dell'aggiornamento: nessuna modifica è applicata.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreAggiornamento {
    /// Codice assente dall'elenco o dalle risorse.
    #[error("{0} «{1}» non presente nel progetto")]
    NelProgetto(&'static str, String),
    /// Codice senza corrispondenza nel listino.
    #[error("«{0}» non trovato nel listino (cercato come «{1}»)")]
    NelListino(String, String),
    /// Voce con analisi.
    #[error("la voce «{0}» ha un'analisi: il suo prezzo segue l'analisi, aggiornare le risorse")]
    Analizzata(String),
    /// Il nuovo codice è già usato da un'altra voce o risorsa.
    #[error("il codice «{0}» è già usato nel progetto")]
    CodiceGiaUsato(String),
    /// Nessun campo da aggiornare o nessuna voce scelta.
    #[error("{0}")]
    Richiesta(String),
    /// Errore nel ricalcolo delle analisi.
    #[error(transparent)]
    Dominio(#[from] ErroreDominio),
}

/// Esito dell'aggiornamento.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EsitoAggiornamento {
    /// Voci dell'elenco aggiornate dal listino.
    pub voci: usize,
    /// Risorse aggiornate.
    pub risorse: usize,
    /// Voci analizzate con il prezzo cambiato dal ricalcolo.
    pub analisi_ricalcolate: usize,
    /// Voci dei computi riallineate, per computo.
    pub riallineate: BTreeMap<ComputoId, usize>,
}

/// Scelte dell'aggiornamento.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceltaAggiornamento<'a> {
    /// Codici (nell'elenco) delle voci da aggiornare.
    pub voci: &'a [String],
    /// Codici delle risorse da aggiornare.
    pub risorse: &'a [String],
    /// Campi.
    pub campi: CampiAggiornamento,
    /// Sostituzione del prefisso.
    pub prefisso: Option<&'a SostituzionePrefisso>,
    /// Ricalcola i prezzi delle voci analizzate.
    pub ricalcola_analisi: bool,
    /// Riallinea le voci dei computi collegate alle voci cambiate.
    pub riallinea_computi: bool,
}

/// Dati di una voce che le voci dei computi copiano: serve a sapere quali voci sono
/// cambiate.
fn impronta(v: &VocePrezzario) -> PrezzoApplicato {
    let mut p = PrezzoApplicato::manuale(
        v.codice.clone(),
        String::new(),
        v.unita_misura.clone(),
        v.prezzo,
    );
    p.descrizione_breve.clone_from(&v.descrizione_breve);
    p.descrizione_estesa.clone_from(&v.descrizione_estesa);
    p.natura = v.natura;
    p.incidenza_manodopera = v.incidenza_manodopera;
    p.incidenza_sicurezza = v.incidenza_sicurezza;
    p
}

/// Aggiorna le voci e le risorse scelte dal listino.
///
/// # Errors
/// Codice assente dal progetto o dal listino, voce con analisi, codice nuovo già
/// usato, nessuna scelta o nessun campo, errore nel ricalcolo delle analisi. In caso
/// di errore il progetto non è modificato.
pub fn aggiorna_da_listino(
    p: &mut Progetto,
    listino: &Prezzario,
    s: &SceltaAggiornamento<'_>,
) -> Result<EsitoAggiornamento, ErroreAggiornamento> {
    let c = s.campi;
    if s.voci.is_empty() && s.risorse.is_empty() {
        return Err(ErroreAggiornamento::Richiesta(
            "nessuna voce né risorsa scelta".into(),
        ));
    }
    if !(c.prezzo || c.incidenze || c.descrizioni || c.unita_misura || c.codice) {
        return Err(ErroreAggiornamento::Richiesta(
            "nessun campo da aggiornare".into(),
        ));
    }
    let mut nuovo = p.clone();
    let (nel_listino, _) = indice(listino);
    let prima: HashMap<VocePrezzarioId, PrezzoApplicato> = p
        .elenco_prezzi
        .voci
        .iter()
        .map(|v| (v.id, impronta(v)))
        .collect();
    let per_codice_prima: HashMap<&str, VocePrezzarioId> = p
        .elenco_prezzi
        .voci
        .iter()
        .map(|v| (v.codice.as_str(), v.id))
        .collect();
    let mut esito = EsitoAggiornamento::default();

    let e = &mut nuovo.elenco_prezzi;
    for codice in s.voci {
        let cercato = codice_listino(codice, s.prefisso);
        let l = nel_listino
            .get(cercato.as_str())
            .copied()
            .ok_or_else(|| ErroreAggiornamento::NelListino(codice.clone(), cercato.clone()))?;
        let id = e
            .voce_per_codice(codice)
            .map(|v| v.id)
            .ok_or_else(|| ErroreAggiornamento::NelProgetto("voce", codice.clone()))?;
        if c.codice && l.codice != *codice && e.voce_per_codice(&l.codice).is_some() {
            return Err(ErroreAggiornamento::CodiceGiaUsato(l.codice.clone()));
        }
        let Some(v) = e.voce_mut(id) else { continue };
        if v.analisi.is_some() && (c.prezzo || c.incidenze) {
            return Err(ErroreAggiornamento::Analizzata(codice.clone()));
        }
        if c.prezzo {
            v.prezzo = l.prezzo;
        }
        if c.incidenze {
            v.incidenza_manodopera = l.incidenza_manodopera;
            v.incidenza_sicurezza = l.incidenza_sicurezza;
        }
        if c.descrizioni {
            v.descrizione_breve.clone_from(&l.descrizione_breve);
            v.descrizione_estesa.clone_from(&l.descrizione_estesa);
        }
        if c.unita_misura {
            v.unita_misura.clone_from(&l.unita_misura);
        }
        if c.codice {
            v.codice.clone_from(&l.codice);
        }
        esito.voci += 1;
    }
    for codice in s.risorse {
        let cercato = codice_listino(codice, s.prefisso);
        let l = dati_risorsa(listino, &cercato)
            .ok_or_else(|| ErroreAggiornamento::NelListino(codice.clone(), cercato.clone()))?;
        if c.codice && l.codice != codice && e.risorse.iter().any(|r| r.codice == l.codice) {
            return Err(ErroreAggiornamento::CodiceGiaUsato(l.codice.to_owned()));
        }
        let r = e
            .risorse
            .iter_mut()
            .find(|r| r.codice == *codice)
            .ok_or_else(|| ErroreAggiornamento::NelProgetto("risorsa", codice.clone()))?;
        if c.prezzo {
            r.prezzo = l.prezzo;
        }
        if c.descrizioni {
            l.descrizione.clone_into(&mut r.descrizione);
        }
        if c.unita_misura {
            l.unita_misura.clone_into(&mut r.unita_misura);
        }
        if c.codice {
            l.codice.clone_into(&mut r.codice);
        }
        esito.risorse += 1;
    }
    // Codici unici dopo l'aggiornamento (due voci portate allo stesso codice).
    if c.codice {
        let mut visti = HashSet::new();
        for v in &e.voci {
            if !visti.insert(v.codice.as_str()) {
                return Err(ErroreAggiornamento::CodiceGiaUsato(v.codice.clone()));
            }
        }
    }
    if s.ricalcola_analisi && !e.analisi.is_empty() {
        esito.analisi_ricalcolate = e.ricalcola_prezzi_da_analisi(&nuovo.regole)?.len();
    }

    if s.riallinea_computi {
        let elenco = nuovo.elenco_prezzi.clone();
        let cambiate: HashSet<VocePrezzarioId> = elenco
            .voci
            .iter()
            .filter(|v| prima.get(&v.id).is_some_and(|i| *i != impronta(v)))
            .map(|v| v.id)
            .collect();
        for computo in &mut nuovo.computi {
            let mut n = 0;
            for v in &mut computo.voci {
                // La voce d'origine si cerca con i codici di prima dell'aggiornamento.
                let id = v
                    .prezzo
                    .origine
                    .filter(|o| o.prezzario == elenco.id && prima.contains_key(&o.voce))
                    .map(|o| o.voce)
                    .or_else(|| per_codice_prima.get(v.prezzo.codice.as_str()).copied());
                let Some(sorgente) = id
                    .filter(|i| cambiate.contains(i))
                    .and_then(|i| elenco.voce(i))
                else {
                    continue;
                };
                if !Arc::make_mut(v)
                    .prezzo
                    .riallinea(&elenco, sorgente)
                    .is_empty()
                {
                    n += 1;
                }
            }
            if n > 0 {
                esito.riallineate.insert(computo.id, n);
            }
        }
    }
    *p = nuovo;
    Ok(esito)
}
