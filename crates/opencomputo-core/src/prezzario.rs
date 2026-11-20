//! Prezzari ed elenchi prezzi.
//!
//! Un [`Prezzario`] raccoglie voci con codice (tariffa), descrizione, unità di
//! misura e prezzo unitario, organizzate in capitoli, insieme alle risorse
//! elementari e alle analisi prezzi da cui alcune voci derivano.
//!
//! Le voci di prezzario sono **definizioni**: le voci di computo e le
//! registrazioni contabili ne conservano una copia (vedi
//! [`crate::partita::PrezzoApplicato`]). Modificare un prezzario non altera mai
//! i documenti in cui le sue voci sono già state usate.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::analisi::{AnalisiPrezzo, ParametriAnalisi, RisorsaElementare, RisultatoAnalisi};
use crate::errore::ErroreDominio;
use crate::id::{AnalisiId, CapitoloId, PrezzarioId, RisorsaId, VocePrezzarioId};
use crate::numeri::{Percentuale, Prezzo, RegoleArrotondamento};
use crate::provenienza::Provenienza;
use crate::riferimento::Riferimento;

/// Natura economica di una voce.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum NaturaVoce {
    /// Lavorazione soggetta a ribasso d'asta.
    #[default]
    Lavoro,
    /// Oneri (costi) della sicurezza, non soggetti a ribasso.
    OneriSicurezza,
}

/// Capitolo di un prezzario (struttura gerarchica dei codici).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapitoloPrezzario {
    /// Identificativo.
    pub id: CapitoloId,
    /// Capitolo padre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padre: Option<CapitoloId>,
    /// Codice (es. `A.01`).
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
}

/// Criterio di formazione di un nuovo prezzo (Allegato II.14 al D.Lgs. 36/2023,
/// art. 5, comma 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterioNuovoPrezzo {
    /// Desunto dai prezzari di riferimento.
    Prezzario,
    /// Ragguagliato a lavorazioni consimili comprese nel contratto.
    Analogia,
    /// Formato con analisi dei prezzi elementari.
    Analisi,
}

impl CriterioNuovoPrezzo {
    /// Descrizione per le stampe.
    #[must_use]
    pub fn descrizione(self) -> &'static str {
        match self {
            Self::Prezzario => "desunto dal prezzario",
            Self::Analogia => "per analogia con prezzi contrattuali",
            Self::Analisi => "da analisi dei prezzi elementari",
        }
    }
}

fn vero() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn e_vero(b: &bool) -> bool {
    *b
}

/// Dati di un nuovo prezzo concordato in corso d'opera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NuovoPrezzo {
    /// Numero (NP.1, NP.2, …).
    pub numero: u32,
    /// Data di formazione.
    pub data: Date,
    /// Motivo (variante, lavorazione non prevista…).
    #[serde(default)]
    pub motivo: String,
    /// Criterio di formazione.
    pub criterio: CriterioNuovoPrezzo,
    /// Se `false` il prezzo è già netto e nei SAL è escluso dal ribasso d'asta.
    #[serde(default = "vero", skip_serializing_if = "e_vero")]
    pub soggetto_a_ribasso: bool,
    /// Numero del verbale di concordamento in cui è compreso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbale: Option<u32>,
}

impl NuovoPrezzo {
    /// Sigla: `NP.3`.
    #[must_use]
    pub fn sigla(&self) -> String {
        format!("NP.{}", self.numero)
    }
}

/// Voce di prezzario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VocePrezzario {
    /// Identificativo.
    pub id: VocePrezzarioId,
    /// Codice (tariffa), univoco nel prezzario.
    pub codice: String,
    /// Descrizione breve.
    #[serde(default)]
    pub descrizione_breve: String,
    /// Descrizione estesa.
    #[serde(default)]
    pub descrizione_estesa: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo unitario.
    pub prezzo: Prezzo,
    /// Natura economica.
    #[serde(default)]
    pub natura: NaturaVoce,
    /// Incidenza percentuale della manodopera sul prezzo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incidenza_manodopera: Option<Percentuale>,
    /// Incidenza percentuale dei costi della sicurezza compresi nel prezzo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incidenza_sicurezza: Option<Percentuale>,
    /// Capitolo di appartenenza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capitolo: Option<CapitoloId>,
    /// Analisi da cui deriva il prezzo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analisi: Option<AnalisiId>,
    /// Dati del nuovo prezzo, se la voce è stata aggiunta in corso d'opera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nuovo_prezzo: Option<NuovoPrezzo>,
    /// Provenienza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenienza: Option<Provenienza>,
}

impl VocePrezzario {
    /// Nuova voce con i dati essenziali.
    #[must_use]
    pub fn new(
        id: VocePrezzarioId,
        codice: impl Into<String>,
        descrizione: impl Into<String>,
        unita_misura: impl Into<String>,
        prezzo: Prezzo,
    ) -> Self {
        let descrizione = descrizione.into();
        Self {
            id,
            codice: codice.into(),
            descrizione_breve: descrizione.clone(),
            descrizione_estesa: descrizione,
            unita_misura: unita_misura.into(),
            prezzo,
            natura: NaturaVoce::Lavoro,
            incidenza_manodopera: None,
            incidenza_sicurezza: None,
            capitolo: None,
            analisi: None,
            nuovo_prezzo: None,
            provenienza: None,
        }
    }
}

/// Indice delle voci per codice e per identificativo, costruito alla prima ricerca.
///
/// Le voci sono un campo pubblico e possono cambiare senza passare da qui: ogni
/// risultato dell'indice viene quindi verificato sulla voce vera, e un indice non più
/// valido viene ricostruito. Una ricerca non trovata nell'indice ripiega sulla scansione
/// di tutte le voci: il risultato è sempre quello della scansione, l'indice la evita
/// soltanto quando è possibile.
#[derive(Default)]
pub struct IndiceVoci(std::sync::Mutex<Option<Arc<Indice>>>);

#[derive(Debug)]
struct Indice {
    voci: usize,
    per_codice: std::collections::HashMap<String, usize>,
    per_id: std::collections::HashMap<VocePrezzarioId, usize>,
}

impl Indice {
    fn di(voci: &[Arc<VocePrezzario>]) -> Self {
        let mut per_codice = std::collections::HashMap::with_capacity(voci.len());
        let mut per_id = std::collections::HashMap::with_capacity(voci.len());
        for (i, v) in voci.iter().enumerate() {
            // Codici ripetuti (prezzario non valido): vale il primo, come nella scansione.
            per_codice.entry(v.codice.clone()).or_insert(i);
            per_id.entry(v.id).or_insert(i);
        }
        Self {
            voci: voci.len(),
            per_codice,
            per_id,
        }
    }
}

impl IndiceVoci {
    /// Indice valido per `voci` (stesso numero di voci), ricostruito se serve.
    fn per(&self, voci: &[Arc<VocePrezzario>], ricostruisci: bool) -> Arc<Indice> {
        let mut g = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match g.as_ref() {
            Some(i) if !ricostruisci && i.voci == voci.len() => Arc::clone(i),
            _ => {
                let i = Arc::new(Indice::di(voci));
                *g = Some(Arc::clone(&i));
                i
            }
        }
    }

    /// Posizione della voce che soddisfa `e_lei`, cercata con `chiave` nell'indice.
    fn cerca(
        &self,
        voci: &[Arc<VocePrezzario>],
        chiave: impl Fn(&Indice) -> Option<usize>,
        e_lei: impl Fn(&VocePrezzario) -> bool,
    ) -> Option<usize> {
        // Pochi elementi: la scansione costa meno dell'indice.
        if voci.len() < 64 {
            return voci.iter().position(|v| e_lei(v));
        }
        match chiave(&self.per(voci, false)) {
            Some(i) if voci.get(i).is_some_and(|v| e_lei(v)) => return Some(i),
            // Indice non più valido (voce spostata o modificata): si ricostruisce.
            Some(_) => {
                if let Some(i) = chiave(&self.per(voci, true))
                    && voci.get(i).is_some_and(|v| e_lei(v))
                {
                    return Some(i);
                }
            }
            None => {}
        }
        // Non nell'indice: di solito la voce non c'è. La scansione lo conferma; se invece
        // la trova (codice cambiato o voce sostituita dopo la costruzione), l'indice
        // viene rifatto per le ricerche successive.
        let trovata = voci.iter().position(|v| e_lei(v));
        if trovata.is_some() {
            self.per(voci, true);
        }
        trovata
    }
}

impl Clone for IndiceVoci {
    fn clone(&self) -> Self {
        // La copia ha le stesse voci: l'indice resta valido.
        let g = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self(std::sync::Mutex::new(g.clone()))
    }
}

impl std::fmt::Debug for IndiceVoci {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IndiceVoci")
    }
}

/// L'indice non fa parte del valore: due prezzari con le stesse voci sono uguali.
impl PartialEq for IndiceVoci {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for IndiceVoci {}

/// Prezzario o elenco prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prezzario {
    /// Identificativo.
    pub id: PrezzarioId,
    /// Nome (es. «Prezzario regionale 2025»).
    pub nome: String,
    /// Ente emittente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ente: Option<String>,
    /// Edizione o versione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edizione: Option<String>,
    /// Data di pubblicazione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_pubblicazione: Option<Date>,
    /// Capitoli.
    #[serde(default)]
    pub capitoli: Vec<CapitoloPrezzario>,
    /// Voci, nell'ordine di presentazione.
    #[serde(default)]
    pub voci: Vec<Arc<VocePrezzario>>,
    /// Risorse elementari.
    #[serde(default)]
    pub risorse: Vec<RisorsaElementare>,
    /// Analisi prezzi.
    #[serde(default)]
    pub analisi: Vec<AnalisiPrezzo>,
    /// Spese generali e utile predefiniti per le nuove analisi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametri_analisi: Option<ParametriAnalisi>,
    /// Provenienza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenienza: Option<Provenienza>,
    /// Indice delle voci per le ricerche (non salvato).
    #[serde(skip)]
    pub indice: IndiceVoci,
}

impl Prezzario {
    /// Nuovo prezzario vuoto.
    #[must_use]
    pub fn new(id: PrezzarioId, nome: impl Into<String>) -> Self {
        Self {
            id,
            nome: nome.into(),
            ente: None,
            edizione: None,
            data_pubblicazione: None,
            capitoli: Vec::new(),
            voci: Vec::new(),
            risorse: Vec::new(),
            analisi: Vec::new(),
            parametri_analisi: None,
            provenienza: None,
            indice: IndiceVoci::default(),
        }
    }

    /// Descrizione sintetica della fonte, usata nei prezzi applicati.
    #[must_use]
    pub fn fonte(&self) -> String {
        match &self.edizione {
            Some(e) if !e.trim().is_empty() => format!("{} ({e})", self.nome),
            _ => self.nome.clone(),
        }
    }

    /// Voce per identificativo.
    #[must_use]
    pub fn voce(&self, id: VocePrezzarioId) -> Option<&VocePrezzario> {
        let i = self
            .indice
            .cerca(&self.voci, |x| x.per_id.get(&id).copied(), |v| v.id == id)?;
        self.voci.get(i).map(Arc::as_ref)
    }

    /// Voce per identificativo, modificabile.
    ///
    /// Le voci sono condivise con le copie del progetto (cronologia di «Annulla»): se
    /// questa lo è, viene prima duplicata, e le altre copie restano invariate.
    pub fn voce_mut(&mut self, id: VocePrezzarioId) -> Option<&mut VocePrezzario> {
        let i = self
            .indice
            .cerca(&self.voci, |x| x.per_id.get(&id).copied(), |v| v.id == id)?;
        self.voci.get_mut(i).map(Arc::make_mut)
    }

    /// Voce per codice esatto.
    #[must_use]
    pub fn voce_per_codice(&self, codice: &str) -> Option<&VocePrezzario> {
        let i = self.indice.cerca(
            &self.voci,
            |x| x.per_codice.get(codice).copied(),
            |v| v.codice == codice,
        )?;
        self.voci.get(i).map(Arc::as_ref)
    }

    /// Ricerca testuale, senza distinzione tra maiuscole e minuscole, su codice
    /// e descrizioni. Tutte le parole del testo devono comparire.
    pub fn cerca<'a>(&'a self, testo: &str) -> impl Iterator<Item = &'a VocePrezzario> + 'a {
        let parole: Vec<String> = testo.split_whitespace().map(str::to_lowercase).collect();
        self.voci.iter().map(Arc::as_ref).filter(move |v| {
            let contenuto = format!(
                "{} {} {}",
                v.codice, v.descrizione_breve, v.descrizione_estesa
            )
            .to_lowercase();
            parole.iter().all(|p| contenuto.contains(p.as_str()))
        })
    }

    /// Aggiunge una voce in coda.
    ///
    /// # Errors
    /// Identificativo o codice già presenti.
    pub fn aggiungi_voce(&mut self, voce: VocePrezzario) -> Result<(), ErroreDominio> {
        if self.voce(voce.id).is_some() {
            return Err(ErroreDominio::IdDuplicato {
                entita: "voce di prezzario",
                id: voce.id.to_string(),
            });
        }
        if self.voce_per_codice(&voce.codice).is_some() {
            return Err(ErroreDominio::CodiceDuplicato {
                entita: "voce di prezzario",
                codice: voce.codice,
            });
        }
        if let Some(c) = voce.capitolo
            && !self.capitoli.iter().any(|x| x.id == c)
        {
            return Err(ErroreDominio::RiferimentoNonValido(format!(
                "il capitolo {c} della voce «{}» non esiste",
                voce.codice
            )));
        }
        self.voci.push(Arc::new(voce));
        Ok(())
    }

    /// Aggiunge un capitolo.
    ///
    /// # Errors
    /// Identificativo duplicato o capitolo padre inesistente.
    pub fn aggiungi_capitolo(&mut self, capitolo: CapitoloPrezzario) -> Result<(), ErroreDominio> {
        if self.capitoli.iter().any(|c| c.id == capitolo.id) {
            return Err(ErroreDominio::IdDuplicato {
                entita: "capitolo",
                id: capitolo.id.to_string(),
            });
        }
        if let Some(p) = capitolo.padre
            && !self.capitoli.iter().any(|c| c.id == p)
        {
            return Err(ErroreDominio::RiferimentoNonValido(format!(
                "il capitolo padre {p} non esiste"
            )));
        }
        self.capitoli.push(capitolo);
        Ok(())
    }

    /// Risorsa elementare per identificativo.
    #[must_use]
    pub fn risorsa(&self, id: RisorsaId) -> Option<&RisorsaElementare> {
        self.risorse.iter().find(|r| r.id == id)
    }

    /// Analisi per identificativo.
    #[must_use]
    pub fn analisi(&self, id: AnalisiId) -> Option<&AnalisiPrezzo> {
        self.analisi.iter().find(|a| a.id == id)
    }

    /// Ricalcola il prezzo di una voce dalla sua analisi (con i prezzi correnti e le
    /// sotto-analisi, vedi [`Self::calcola_analisi`]) e aggiorna prezzo e incidenza
    /// della manodopera. È un'operazione esplicita: non viene mai eseguita
    /// automaticamente e non tocca i computi già redatti.
    ///
    /// # Errors
    /// Voce o analisi inesistenti, oppure errore di calcolo dell'analisi.
    pub fn aggiorna_prezzo_da_analisi(
        &mut self,
        voce: VocePrezzarioId,
        regole: &RegoleArrotondamento,
    ) -> Result<RisultatoAnalisi, ErroreDominio> {
        let v = self.voce(voce).ok_or_else(|| ErroreDominio::NonTrovato {
            entita: "voce di prezzario",
            id: voce.to_string(),
        })?;
        let analisi_id = v.analisi.ok_or_else(|| {
            ErroreDominio::OperazioneNonConsentita(format!(
                "la voce «{}» non è collegata a un'analisi",
                v.codice
            ))
        })?;
        let rif_voce = Riferimento::VocePrezzario {
            id: v.id,
            codice: v.codice.clone(),
        };
        let risultato = match self.calcola_analisi(analisi_id, regole) {
            Err(ErroreDominio::Calcolo(e)) => {
                return Err(ErroreDominio::Calcolo(e.dentro(rif_voce)));
            }
            r => r?,
        };
        if let Some(v) = self.voce_mut(voce) {
            v.prezzo = risultato.prezzo_unitario;
            v.incidenza_manodopera = risultato.incidenza_manodopera;
        }
        Ok(risultato)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn prezzario() -> Prezzario {
        let mut p = Prezzario::new(PrezzarioId::da_u128(1), "Prezzario di prova");
        p.edizione = Some("2025".into());
        p.aggiungi_voce(VocePrezzario::new(
            VocePrezzarioId::da_u128(10),
            "A.01.001",
            "Scavo di sbancamento in terreno di qualsiasi natura",
            "m³",
            Prezzo::new(dec!(8.50)),
        ))
        .unwrap();
        p.aggiungi_voce(VocePrezzario::new(
            VocePrezzarioId::da_u128(11),
            "B.02.010",
            "Calcestruzzo per strutture di fondazione C25/30",
            "m³",
            Prezzo::new(dec!(145.30)),
        ))
        .unwrap();
        p
    }

    #[test]
    fn codici_e_id_univoci() {
        let mut p = prezzario();
        let dup_codice = VocePrezzario::new(
            VocePrezzarioId::da_u128(99),
            "A.01.001",
            "x",
            "m",
            Prezzo::ZERO,
        );
        assert!(matches!(
            p.aggiungi_voce(dup_codice),
            Err(ErroreDominio::CodiceDuplicato { .. })
        ));
        let dup_id =
            VocePrezzario::new(VocePrezzarioId::da_u128(10), "Z.99", "x", "m", Prezzo::ZERO);
        assert!(matches!(
            p.aggiungi_voce(dup_id),
            Err(ErroreDominio::IdDuplicato { .. })
        ));
    }

    #[test]
    fn ricerca_testuale() {
        let p = prezzario();
        let trovate: Vec<_> = p
            .cerca("calcestruzzo FONDAZIONE")
            .map(|v| v.codice.as_str())
            .collect();
        assert_eq!(trovate, vec!["B.02.010"]);
        assert_eq!(p.cerca("a.01").count(), 1);
        assert_eq!(p.cerca("inesistente").count(), 0);
        assert_eq!(p.fonte(), "Prezzario di prova (2025)");
    }

    #[test]
    fn capitolo_inesistente_rifiutato() {
        let mut p = prezzario();
        let mut v = VocePrezzario::new(VocePrezzarioId::da_u128(50), "C.1", "x", "m", Prezzo::ZERO);
        v.capitolo = Some(CapitoloId::da_u128(5));
        assert!(matches!(
            p.aggiungi_voce(v),
            Err(ErroreDominio::RiferimentoNonValido(_))
        ));
    }
}

#[cfg(test)]
mod prove_indice {
    use super::*;

    fn elenco(n: usize) -> Prezzario {
        let mut p = Prezzario::new(PrezzarioId::da_u128(1), "Prova");
        for i in 0..n {
            let mut v = VocePrezzario::new(
                VocePrezzarioId::da_u128(1000 + i as u128),
                format!("C.{i:04}"),
                format!("Voce {i}"),
                "m",
                Prezzo::new(rust_decimal::Decimal::ONE),
            );
            v.descrizione_estesa = String::new();
            p.voci.push(Arc::new(v));
        }
        p
    }

    /// L'indice dà sempre lo stesso risultato della scansione, anche quando le voci
    /// cambiano direttamente nel campo pubblico.
    #[test]
    fn indice_coerente_con_la_scansione() {
        let scansione = |p: &Prezzario, c: &str| p.voci.iter().position(|v| v.codice == c);
        let per_id = |p: &Prezzario, id| p.voci.iter().position(|v| v.id == id);
        let pos = |p: &Prezzario, v: Option<&VocePrezzario>| {
            v.and_then(|v| p.voci.iter().position(|x| std::ptr::eq(&**x, v)))
        };
        for n in [0, 3, 63, 64, 500] {
            let mut p = elenco(n);
            for c in ["C.0000", "C.0002", "C.0063", "C.0499", "X"] {
                assert_eq!(pos(&p, p.voce_per_codice(c)), scansione(&p, c), "{n} {c}");
            }
            if n < 64 {
                continue;
            }
            // Codice cambiato direttamente: il vecchio non si trova più, il nuovo sì.
            Arc::make_mut(&mut p.voci[10]).codice = "NUOVO".into();
            assert_eq!(p.voce_per_codice("C.0010"), None);
            assert_eq!(pos(&p, p.voce_per_codice("NUOVO")), Some(10));
            // Voci riordinate e tolte: le posizioni nell'indice non valgono più.
            p.voci.reverse();
            p.voci.remove(0);
            for c in ["C.0000", "C.0100", "C.0498", "C.0499", "NUOVO"] {
                assert_eq!(pos(&p, p.voce_per_codice(c)), scansione(&p, c), "{c}");
            }
            let id = VocePrezzarioId::da_u128(1000 + n as u128 / 2);
            assert_eq!(pos(&p, p.voce(id)), per_id(&p, id));
            // Voce sostituita con un'altra con lo stesso numero di voci.
            let ultima = p.voci.len() - 1;
            Arc::make_mut(&mut p.voci[ultima]).id = VocePrezzarioId::da_u128(9);
            assert_eq!(pos(&p, p.voce(VocePrezzarioId::da_u128(9))), Some(ultima));
            // Modifica attraverso voce_mut.
            if let Some(v) = p.voce_mut(id) {
                v.codice = "MOD".into();
            }
            assert_eq!(p.voce(id).map(|v| v.codice.as_str()), Some("MOD"));
            assert_eq!(pos(&p, p.voce_per_codice("MOD")), per_id(&p, id));
            // La copia ha un indice valido e uguale valore.
            let q = p.clone();
            assert_eq!(q, p);
            assert_eq!(pos(&q, q.voce_per_codice("MOD")), per_id(&q, id));
        }
    }

    /// Codici ripetuti (elenco non valido): vale la prima voce, come nella scansione.
    #[test]
    fn codici_ripetuti() {
        let mut p = elenco(100);
        Arc::make_mut(&mut p.voci[70]).codice = "C.0005".into();
        assert_eq!(
            p.voce_per_codice("C.0005")
                .map(|v| v.descrizione_breve.as_str()),
            Some("Voce 5")
        );
    }
}
