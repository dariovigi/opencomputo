//! Computi metrici estimativi e preventivi.
//!
//! Un [`Computo`] è una sequenza ordinata di [`VoceComputo`]; il numero
//! d'ordine di una voce è la sua posizione (a base 1). Ogni voce contiene la
//! fotografia del prezzo applicato e i righi di misurazione.
//!
//! L'importo di ciascuna voce è arrotondato; i totali sono somme esatte degli
//! importi arrotondati, quindi il totale di un computo coincide sempre con la
//! somma delle voci stampate.

use std::collections::{BTreeMap, HashMap};

use rust_decimal::Decimal;

use serde::{Deserialize, Serialize};

use crate::classificazione::{AssegnazioneCategorie, StrutturaWbs};
use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
use crate::id::{ClasseId, ComputoId, VoceComputoId, VocePrezzarioId, WbsNodoId};
use crate::misura::{QuantitaRiferite, RigoMisura, TipoRigo, quantita_totale_con};
use crate::numeri::{Importo, Prezzo, Quantita, RegoleArrotondamento};
use crate::partita::{CalcoloPartita, PrezzoApplicato, calcola_partita_con};
use crate::prezzario::{NaturaVoce, Prezzario};
use crate::provenienza::Provenienza;
use crate::riferimento::Riferimento;

/// Tipo di documento di stima.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoComputo {
    /// Computo metrico estimativo.
    #[default]
    ComputoMetricoEstimativo,
    /// Computo metrico (senza prezzi; gli importi restano calcolabili).
    ComputoMetrico,
    /// Preventivo.
    Preventivo,
    /// Perizia di variante.
    PeriziaVariante,
}

/// Voce di computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoceComputo {
    /// Identificativo.
    pub id: VoceComputoId,
    /// Prezzo applicato (fotografia).
    pub prezzo: PrezzoApplicato,
    /// Righi di misurazione.
    #[serde(default)]
    pub righi: Vec<RigoMisura>,
    /// Categorie.
    #[serde(default)]
    pub categorie: AssegnazioneCategorie,
    /// Nodo WBS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wbs: Option<WbsNodoId>,
    /// Note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Provenienza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenienza: Option<Provenienza>,
}

impl VoceComputo {
    /// Nuova voce senza righi.
    #[must_use]
    pub fn new(id: VoceComputoId, prezzo: PrezzoApplicato) -> Self {
        Self {
            id,
            prezzo,
            righi: Vec::new(),
            categorie: AssegnazioneCategorie::default(),
            wbs: None,
            note: None,
            provenienza: None,
        }
    }

    /// Calcola la voce con le quantità delle voci citate dai «vedi voce».
    ///
    /// # Errors
    /// Errore localizzato sul rigo responsabile.
    pub fn calcola_con(
        &self,
        regole: &RegoleArrotondamento,
        riferite: &dyn QuantitaRiferite,
    ) -> Result<CalcoloPartita, ErroreLocalizzato> {
        calcola_partita_con(&self.prezzo, &self.righi, regole, riferite)
    }

    /// Calcola la voce dentro il suo computo: con le quantità delle voci citate e, per
    /// una voce in un riferimento circolare, con l'errore che lo descrive.
    ///
    /// # Errors
    /// Errore localizzato sul rigo responsabile.
    pub fn calcola_in(
        &self,
        regole: &RegoleArrotondamento,
        riferite: &QuantitaComputo,
    ) -> Result<CalcoloPartita, ErroreLocalizzato> {
        if let Some(Err(e)) = riferite.di(self.id) {
            return Err(e.clone());
        }
        self.calcola_con(regole, riferite)
    }

    /// Voci citate dai righi di misura con un «vedi voce» collegato.
    pub fn voci_citate(&self) -> impl Iterator<Item = VoceComputoId> + '_ {
        self.righi
            .iter()
            .filter(|r| r.tipo == TipoRigo::Misura)
            .filter_map(|r| r.vedi_voce.as_ref().and_then(crate::misura::VediVoce::voce))
    }
}

/// Quantità delle voci di un computo, calcolate nell'ordine dei collegamenti «vedi
/// voce» ([`Computo::quantita_riferite`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantitaComputo {
    /// Per voce: numero d'ordine (da 1) e quantità, o errore localizzato sul rigo (il
    /// percorso parte dal rigo, senza la voce).
    voci: HashMap<VoceComputoId, (usize, Result<Quantita, ErroreLocalizzato>)>,
}

impl QuantitaComputo {
    /// Quantità della voce, o l'errore che ne impedisce il calcolo (localizzato sul
    /// rigo).
    #[must_use]
    pub fn di(&self, voce: VoceComputoId) -> Option<&Result<Quantita, ErroreLocalizzato>> {
        self.voci.get(&voce).map(|(_, r)| r)
    }
}

impl QuantitaRiferite for QuantitaComputo {
    fn quantita(&self, voce: VoceComputoId) -> Result<Decimal, ErroreCalcolo> {
        match self.voci.get(&voce) {
            None => Err(ErroreCalcolo::VediVoce(
                "la voce citata non è in questo computo".into(),
            )),
            Some((_, Ok(q))) => Ok(q.valore()),
            Some((numero, Err(_))) => Err(ErroreCalcolo::VediVoce(format!(
                "la voce citata n. {numero} non è calcolabile"
            ))),
        }
    }
}

/// Voci già calcolate durante [`Computo::quantita_riferite`]: le voci in un ciclo
/// non ci arrivano mai.
struct Parziale<'a> {
    calcolate: &'a HashMap<VoceComputoId, (usize, Result<Quantita, ErroreLocalizzato>)>,
    numeri: &'a HashMap<VoceComputoId, usize>,
}

impl QuantitaRiferite for Parziale<'_> {
    fn quantita(&self, voce: VoceComputoId) -> Result<Decimal, ErroreCalcolo> {
        match (self.calcolate.get(&voce), self.numeri.get(&voce)) {
            (Some((_, Ok(q))), _) => Ok(q.valore()),
            (Some((numero, Err(_))), _) => Err(ErroreCalcolo::VediVoce(format!(
                "la voce citata n. {numero} non è calcolabile"
            ))),
            (None, Some(numero)) => Err(ErroreCalcolo::VediVoce(format!(
                "riferimento circolare con la voce n. {numero}"
            ))),
            (None, None) => Err(ErroreCalcolo::VediVoce(
                "la voce citata non è in questo computo".into(),
            )),
        }
    }
}

/// Risultato del calcolo di una voce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RisultatoVoce {
    /// Numero d'ordine.
    pub numero: usize,
    /// Identificativo.
    pub id: VoceComputoId,
    /// Codice.
    pub codice: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo unitario.
    pub prezzo: Prezzo,
    /// Natura.
    pub natura: NaturaVoce,
    /// Quantità, importo e quote.
    #[serde(flatten)]
    pub calcolo: CalcoloPartita,
}

/// Totali raggruppati per chiave, con il totale delle voci non assegnate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotaliRaggruppati<K: Ord> {
    /// Totale per chiave.
    pub per_chiave: BTreeMap<K, Importo>,
    /// Totale delle voci prive di assegnazione.
    pub non_assegnato: Importo,
}

impl<K: Ord> Default for TotaliRaggruppati<K> {
    fn default() -> Self {
        Self {
            per_chiave: BTreeMap::new(),
            non_assegnato: Importo::ZERO,
        }
    }
}

impl<K: Ord> TotaliRaggruppati<K> {
    fn aggiungi(&mut self, chiave: Option<K>, importo: Importo) -> Result<(), ErroreCalcolo> {
        let destinazione = match chiave {
            Some(k) => self.per_chiave.entry(k).or_insert(Importo::ZERO),
            None => &mut self.non_assegnato,
        };
        *destinazione = destinazione.somma(importo)?;
        Ok(())
    }
}

impl TotaliRaggruppati<WbsNodoId> {
    /// Totali cumulati: ogni nodo riceve il totale delle voci assegnate a esso o a un
    /// suo discendente.
    ///
    /// # Errors
    /// Nodo WBS inesistente, struttura ciclica o overflow.
    pub fn cumulati(
        &self,
        wbs: &StrutturaWbs,
    ) -> Result<BTreeMap<WbsNodoId, Importo>, ErroreDominio> {
        let mut cumulati: BTreeMap<WbsNodoId, Importo> = BTreeMap::new();
        for (nodo, importo) in &self.per_chiave {
            for antenato in wbs.percorso(*nodo)? {
                let t = cumulati.entry(antenato).or_insert(Importo::ZERO);
                *t = t.somma(*importo).map_err(|e| {
                    ErroreDominio::Calcolo(ErroreLocalizzato::new(
                        Riferimento::NodoWbs { id: antenato },
                        e,
                    ))
                })?;
            }
        }
        Ok(cumulati)
    }
}

/// Quote (di manodopera o di sicurezza) raggruppate come gli importi: ogni voce entra
/// nei gruppi in cui entra il suo importo, con quota zero se l'incidenza non è nota.
/// Le chiavi coincidono quindi con quelle dei totali degli importi.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteRaggruppate {
    /// Per super categoria.
    pub per_super_categoria: TotaliRaggruppati<ClasseId>,
    /// Per categoria.
    pub per_categoria: TotaliRaggruppati<ClasseId>,
    /// Per sotto categoria.
    pub per_sotto_categoria: TotaliRaggruppati<ClasseId>,
    /// Per nodo WBS (solo assegnazione diretta).
    pub per_wbs: TotaliRaggruppati<WbsNodoId>,
}

impl QuoteRaggruppate {
    fn aggiungi(&mut self, voce: &VoceComputo, quota: Importo) -> Result<(), ErroreCalcolo> {
        self.per_super_categoria
            .aggiungi(voce.categorie.super_categoria, quota)?;
        self.per_categoria
            .aggiungi(voce.categorie.categoria, quota)?;
        self.per_sotto_categoria
            .aggiungi(voce.categorie.sotto_categoria, quota)?;
        self.per_wbs.aggiungi(voce.wbs, quota)
    }
}

/// Riepilogo di un computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiepilogoComputo {
    /// Risultati delle voci, nell'ordine del computo.
    pub voci: Vec<RisultatoVoce>,
    /// Totale lavori (natura [`NaturaVoce::Lavoro`]).
    pub totale_lavori: Importo,
    /// Totale oneri della sicurezza (natura [`NaturaVoce::OneriSicurezza`]).
    pub totale_oneri_sicurezza: Importo,
    /// Totale complessivo.
    pub totale: Importo,
    /// Somma delle quote di manodopera delle voci con incidenza nota.
    pub totale_manodopera: Importo,
    /// Numero di voci prive di incidenza della manodopera.
    pub voci_senza_incidenza_manodopera: usize,
    /// Somma delle quote di sicurezza comprese nei prezzi, delle voci con incidenza
    /// della sicurezza nota (distinta dagli oneri, che sono voci di natura propria).
    #[serde(default)]
    pub totale_sicurezza: Importo,
    /// Numero di voci prive di incidenza della sicurezza.
    #[serde(default)]
    pub voci_senza_incidenza_sicurezza: usize,
    /// Quote di manodopera per categorie e WBS.
    #[serde(default)]
    pub manodopera: QuoteRaggruppate,
    /// Quote di sicurezza comprese nei prezzi per categorie e WBS.
    #[serde(default)]
    pub sicurezza: QuoteRaggruppate,
    /// Totali per super categoria.
    pub per_super_categoria: TotaliRaggruppati<ClasseId>,
    /// Totali per categoria.
    pub per_categoria: TotaliRaggruppati<ClasseId>,
    /// Totali per sotto categoria.
    pub per_sotto_categoria: TotaliRaggruppati<ClasseId>,
    /// Totali per nodo WBS (solo assegnazione diretta).
    pub per_wbs: TotaliRaggruppati<WbsNodoId>,
}

impl RiepilogoComputo {
    /// Totali WBS cumulati: ogni nodo riceve l'importo delle voci assegnate a
    /// esso o a un suo discendente.
    ///
    /// # Errors
    /// Nodo WBS inesistente, struttura ciclica o overflow.
    pub fn totali_wbs_cumulati(
        &self,
        wbs: &StrutturaWbs,
    ) -> Result<BTreeMap<WbsNodoId, Importo>, ErroreDominio> {
        self.per_wbs.cumulati(wbs)
    }
}

/// Computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Computo {
    /// Identificativo.
    pub id: ComputoId,
    /// Nome.
    pub nome: String,
    /// Tipo di documento.
    #[serde(default)]
    pub tipo: TipoComputo,
    /// Voci, nell'ordine del documento.
    #[serde(default)]
    pub voci: Vec<VoceComputo>,
    /// Provenienza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenienza: Option<Provenienza>,
}

impl Computo {
    /// Nuovo computo vuoto.
    #[must_use]
    pub fn new(id: ComputoId, nome: impl Into<String>) -> Self {
        Self {
            id,
            nome: nome.into(),
            tipo: TipoComputo::default(),
            voci: Vec::new(),
            provenienza: None,
        }
    }

    /// Voce per identificativo.
    #[must_use]
    pub fn voce(&self, id: VoceComputoId) -> Option<&VoceComputo> {
        self.voci.iter().find(|v| v.id == id)
    }

    /// Voce per identificativo, modificabile.
    pub fn voce_mut(&mut self, id: VoceComputoId) -> Option<&mut VoceComputo> {
        self.voci.iter_mut().find(|v| v.id == id)
    }

    fn posizione(&self, id: VoceComputoId) -> Result<usize, ErroreDominio> {
        self.voci
            .iter()
            .position(|v| v.id == id)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "voce di computo",
                id: id.to_string(),
            })
    }

    /// Inserisce una voce nella posizione indicata (base 0) o in coda.
    ///
    /// # Errors
    /// Identificativo duplicato o posizione oltre la fine.
    pub fn inserisci_voce(
        &mut self,
        posizione: Option<usize>,
        voce: VoceComputo,
    ) -> Result<(), ErroreDominio> {
        if self.voce(voce.id).is_some() {
            return Err(ErroreDominio::IdDuplicato {
                entita: "voce di computo",
                id: voce.id.to_string(),
            });
        }
        match posizione {
            None => self.voci.push(voce),
            Some(p) if p <= self.voci.len() => self.voci.insert(p, voce),
            Some(p) => {
                return Err(ErroreDominio::OperazioneNonConsentita(format!(
                    "posizione {p} oltre la fine del computo ({} voci)",
                    self.voci.len()
                )));
            }
        }
        Ok(())
    }

    /// Copia una voce di prezzario nel computo (la sorgente resta invariata) e
    /// restituisce la nuova voce, ancora priva di righi.
    ///
    /// # Errors
    /// Voce di prezzario inesistente, identificativo duplicato o posizione non valida.
    pub fn aggiungi_da_prezzario(
        &mut self,
        id: VoceComputoId,
        prezzario: &Prezzario,
        voce_prezzario: VocePrezzarioId,
        posizione: Option<usize>,
    ) -> Result<&mut VoceComputo, ErroreDominio> {
        let sorgente = prezzario
            .voce(voce_prezzario)
            .ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "voce di prezzario",
                id: voce_prezzario.to_string(),
            })?;
        let voce = VoceComputo::new(id, PrezzoApplicato::da_voce(prezzario, sorgente));
        self.inserisci_voce(posizione, voce)?;
        self.voce_mut(id).ok_or_else(|| ErroreDominio::NonTrovato {
            entita: "voce di computo",
            id: id.to_string(),
        })
    }

    /// Rimuove una voce e la restituisce.
    ///
    /// # Errors
    /// Voce inesistente.
    pub fn rimuovi_voce(&mut self, id: VoceComputoId) -> Result<VoceComputo, ErroreDominio> {
        let p = self.posizione(id)?;
        Ok(self.voci.remove(p))
    }

    /// Sposta una voce nella nuova posizione (base 0).
    ///
    /// # Errors
    /// Voce inesistente o posizione non valida.
    pub fn sposta_voce(
        &mut self,
        id: VoceComputoId,
        nuova_posizione: usize,
    ) -> Result<(), ErroreDominio> {
        let p = self.posizione(id)?;
        if nuova_posizione >= self.voci.len() {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "posizione {nuova_posizione} oltre la fine del computo"
            )));
        }
        let voce = self.voci.remove(p);
        self.voci.insert(nuova_posizione, voce);
        Ok(())
    }

    /// Quantità di tutte le voci, calcolate nell'ordine dei collegamenti «vedi voce»:
    /// prima le voci citate, poi quelle che le citano. Le voci in un riferimento
    /// circolare (anche con sé stesse) e quelle che ne dipendono restano con un errore
    /// sul rigo che cita.
    #[must_use]
    pub fn quantita_riferite(&self, regole: &RegoleArrotondamento) -> QuantitaComputo {
        let numeri: HashMap<VoceComputoId, usize> = self
            .voci
            .iter()
            .enumerate()
            .map(|(i, v)| (v.id, i + 1))
            .collect();
        // Ordine topologico (Kahn): `attese[i]` = voci citate da i non ancora calcolate.
        let mut attese: Vec<usize> = Vec::with_capacity(self.voci.len());
        let mut citata_da: Vec<Vec<usize>> = vec![Vec::new(); self.voci.len()];
        for (i, voce) in self.voci.iter().enumerate() {
            let mut citate: Vec<usize> = voce
                .voci_citate()
                .filter_map(|id| numeri.get(&id).map(|n| n - 1))
                .collect();
            citate.sort_unstable();
            citate.dedup();
            attese.push(citate.len());
            for j in citate {
                if let Some(c) = citata_da.get_mut(j) {
                    c.push(i);
                }
            }
        }
        let mut pronte: Vec<usize> = attese
            .iter()
            .enumerate()
            .filter(|(_, a)| **a == 0)
            .map(|(i, _)| i)
            .collect();
        let mut calcolate = HashMap::with_capacity(self.voci.len());
        while let Some(i) = pronte.pop() {
            let Some(voce) = self.voci.get(i) else {
                continue;
            };
            let risultato = quantita_totale_con(
                &voce.righi,
                regole,
                &Parziale {
                    calcolate: &calcolate,
                    numeri: &numeri,
                },
            );
            calcolate.insert(voce.id, (i + 1, risultato));
            for &k in citata_da.get(i).map_or(&[][..], Vec::as_slice) {
                if let Some(a) = attese.get_mut(k) {
                    *a -= 1;
                    if *a == 0 {
                        pronte.push(k);
                    }
                }
            }
        }
        // Voci rimaste: in un ciclo o dipendenti da un ciclo. Il calcolo con le sole
        // voci calcolate fallisce sul primo rigo che cita una voce rimasta.
        let rimaste: Vec<usize> = (0..self.voci.len())
            .filter(|i| {
                self.voci
                    .get(*i)
                    .is_some_and(|v| !calcolate.contains_key(&v.id))
            })
            .collect();
        let mut esiti = Vec::with_capacity(rimaste.len());
        for i in rimaste {
            let Some(voce) = self.voci.get(i) else {
                continue;
            };
            let parziale = Parziale {
                calcolate: &calcolate,
                numeri: &numeri,
            };
            let errore = match quantita_totale_con(&voce.righi, regole, &parziale) {
                Err(e) => e,
                // Non dovrebbe accadere: una voce rimasta cita sempre una voce rimasta.
                Ok(_) => ErroreLocalizzato {
                    percorso: Vec::new(),
                    errore: ErroreCalcolo::VediVoce("riferimento circolare".into()),
                },
            };
            esiti.push((voce.id, (i + 1, Err(errore))));
        }
        calcolate.extend(esiti);
        QuantitaComputo { voci: calcolate }
    }

    /// Calcola il computo.
    ///
    /// # Errors
    /// Errore localizzato sulla voce e sul rigo responsabili.
    pub fn calcola(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<RiepilogoComputo, ErroreLocalizzato> {
        let riferite = self.quantita_riferite(regole);
        let rif_computo = Riferimento::Computo { id: self.id };
        let mut r = RiepilogoComputo {
            voci: Vec::with_capacity(self.voci.len()),
            totale_lavori: Importo::ZERO,
            totale_oneri_sicurezza: Importo::ZERO,
            totale: Importo::ZERO,
            totale_manodopera: Importo::ZERO,
            voci_senza_incidenza_manodopera: 0,
            totale_sicurezza: Importo::ZERO,
            voci_senza_incidenza_sicurezza: 0,
            manodopera: QuoteRaggruppate::default(),
            sicurezza: QuoteRaggruppate::default(),
            per_super_categoria: TotaliRaggruppati::default(),
            per_categoria: TotaliRaggruppati::default(),
            per_sotto_categoria: TotaliRaggruppati::default(),
            per_wbs: TotaliRaggruppati::default(),
        };
        for (i, voce) in self.voci.iter().enumerate() {
            let rif_voce = Riferimento::VoceComputo {
                numero: i + 1,
                id: voce.id,
            };
            let localizza =
                |e: ErroreLocalizzato| e.dentro(rif_voce.clone()).dentro(rif_computo.clone());
            let calcolo = voce.calcola_in(regole, &riferite).map_err(localizza)?;
            let importo = calcolo.importo;
            let accumula = |r: &mut RiepilogoComputo| -> Result<(), ErroreCalcolo> {
                match voce.prezzo.natura {
                    NaturaVoce::Lavoro => r.totale_lavori = r.totale_lavori.somma(importo)?,
                    NaturaVoce::OneriSicurezza => {
                        r.totale_oneri_sicurezza = r.totale_oneri_sicurezza.somma(importo)?;
                    }
                }
                r.totale = r.totale.somma(importo)?;
                match calcolo.importo_manodopera {
                    Some(m) => r.totale_manodopera = r.totale_manodopera.somma(m)?,
                    None => r.voci_senza_incidenza_manodopera += 1,
                }
                match calcolo.importo_sicurezza {
                    Some(s) => r.totale_sicurezza = r.totale_sicurezza.somma(s)?,
                    None => r.voci_senza_incidenza_sicurezza += 1,
                }
                r.manodopera
                    .aggiungi(voce, calcolo.importo_manodopera.unwrap_or(Importo::ZERO))?;
                r.sicurezza
                    .aggiungi(voce, calcolo.importo_sicurezza.unwrap_or(Importo::ZERO))?;
                r.per_super_categoria
                    .aggiungi(voce.categorie.super_categoria, importo)?;
                r.per_categoria
                    .aggiungi(voce.categorie.categoria, importo)?;
                r.per_sotto_categoria
                    .aggiungi(voce.categorie.sotto_categoria, importo)?;
                r.per_wbs.aggiungi(voce.wbs, importo)?;
                Ok(())
            };
            accumula(&mut r).map_err(|e| ErroreLocalizzato {
                percorso: vec![rif_computo.clone(), rif_voce.clone()],
                errore: e,
            })?;
            r.voci.push(RisultatoVoce {
                numero: i + 1,
                id: voce.id,
                codice: voce.prezzo.codice.clone(),
                unita_misura: voce.prezzo.unita_misura.clone(),
                prezzo: voce.prezzo.prezzo,
                natura: voce.prezzo.natura,
                calcolo,
            });
        }
        Ok(r)
    }

    /// Quantità totale di una voce (scorciatoia).
    ///
    /// # Errors
    /// Voce inesistente o errore di calcolo.
    pub fn quantita_voce(
        &self,
        id: VoceComputoId,
        regole: &RegoleArrotondamento,
    ) -> Result<Quantita, ErroreDominio> {
        let p = self.posizione(id)?;
        let voce = self.voci.get(p).ok_or_else(|| ErroreDominio::NonTrovato {
            entita: "voce di computo",
            id: id.to_string(),
        })?;
        Ok(voce
            .calcola_in(regole, &self.quantita_riferite(regole))
            .map_err(|e| e.dentro(Riferimento::VoceComputo { numero: p + 1, id }))?
            .quantita)
    }
}
