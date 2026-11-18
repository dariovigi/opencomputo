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
    pub voci: Vec<VocePrezzario>,
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
        self.voci.iter().find(|v| v.id == id)
    }

    /// Voce per identificativo, modificabile.
    pub fn voce_mut(&mut self, id: VocePrezzarioId) -> Option<&mut VocePrezzario> {
        self.voci.iter_mut().find(|v| v.id == id)
    }

    /// Voce per codice esatto.
    #[must_use]
    pub fn voce_per_codice(&self, codice: &str) -> Option<&VocePrezzario> {
        self.voci.iter().find(|v| v.codice == codice)
    }

    /// Ricerca testuale, senza distinzione tra maiuscole e minuscole, su codice
    /// e descrizioni. Tutte le parole del testo devono comparire.
    pub fn cerca<'a>(&'a self, testo: &str) -> impl Iterator<Item = &'a VocePrezzario> + 'a {
        let parole: Vec<String> = testo.split_whitespace().map(str::to_lowercase).collect();
        self.voci.iter().filter(move |v| {
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
        self.voci.push(voce);
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
