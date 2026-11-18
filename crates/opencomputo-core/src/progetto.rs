//! Progetto: radice del modello di dominio.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::classificazione::{Categorie, StrutturaWbs};
use crate::computo::{Computo, RiepilogoComputo};
use crate::contabilita::{
    CertificatoPagamento, Contabilita, ParametriContabili, RiepilogoContoFinale, RiepilogoSal,
};
use crate::cronoprogramma::Cronoprogramma;
use crate::errore::ErroreDominio;
use crate::id::{ComputoId, PrezzarioId, ProgettoId, SalId};
use crate::numeri::{Importo, RegoleArrotondamento};
use crate::prezzario::Prezzario;
use crate::provenienza::Provenienza;

/// Metadati descrittivi del progetto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadatiProgetto {
    /// Titolo.
    pub titolo: String,
    /// Oggetto dei lavori.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oggetto: Option<String>,
    /// Committente / stazione appaltante.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committente: Option<String>,
    /// Impresa esecutrice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impresa: Option<String>,
    /// Luogo dei lavori.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub luogo: Option<String>,
    /// Codice identificativo di gara.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cig: Option<String>,
    /// Codice unico di progetto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cup: Option<String>,
    /// Responsabile del procedimento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsabile_procedimento: Option<String>,
    /// Progettista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progettista: Option<String>,
    /// Direttore dei lavori.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direttore_lavori: Option<String>,
    /// Data del documento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Date>,
    /// Note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Dati contrattuali dell'appalto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatiContratto {
    /// Numero di repertorio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numero_repertorio: Option<String>,
    /// Data di stipula.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_stipula: Option<Date>,
    /// Data di consegna dei lavori.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_consegna_lavori: Option<Date>,
    /// Tempo utile contrattuale in giorni.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo_utile_giorni: Option<u32>,
    /// Importo contrattuale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importo_contrattuale: Option<Importo>,
    /// Parametri contabili (ribasso, ritenute…).
    #[serde(default)]
    pub parametri: ParametriContabili,
}

/// Progetto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progetto {
    /// Identificativo.
    pub id: ProgettoId,
    /// Metadati.
    pub metadati: MetadatiProgetto,
    /// Dati contrattuali (assenti in fase di progettazione).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contratto: Option<DatiContratto>,
    /// Regole di arrotondamento.
    #[serde(default)]
    pub regole: RegoleArrotondamento,
    /// Elenco prezzi del progetto.
    pub elenco_prezzi: Prezzario,
    /// Categorie.
    #[serde(default)]
    pub categorie: Categorie,
    /// Struttura WBS.
    #[serde(default)]
    pub wbs: StrutturaWbs,
    /// Computi e preventivi.
    #[serde(default)]
    pub computi: Vec<Computo>,
    /// Contabilità dei lavori.
    #[serde(default)]
    pub contabilita: Contabilita,
    /// Cronoprogramma dei lavori (non entra negli export verso altri programmi).
    #[serde(default, skip_serializing_if = "Cronoprogramma::vuoto")]
    pub cronoprogramma: Cronoprogramma,
    /// Provenienza.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenienza: Option<Provenienza>,
}

impl Progetto {
    /// Nuovo progetto vuoto con elenco prezzi vuoto e regole predefinite.
    #[must_use]
    pub fn new(id: ProgettoId, titolo: impl Into<String>, elenco_prezzi: PrezzarioId) -> Self {
        Self {
            id,
            metadati: MetadatiProgetto {
                titolo: titolo.into(),
                ..MetadatiProgetto::default()
            },
            contratto: None,
            regole: RegoleArrotondamento::default(),
            elenco_prezzi: Prezzario::new(elenco_prezzi, "Elenco prezzi"),
            categorie: Categorie::default(),
            wbs: StrutturaWbs::default(),
            computi: Vec::new(),
            contabilita: Contabilita::default(),
            cronoprogramma: Cronoprogramma::default(),
            provenienza: None,
        }
    }

    /// Computo per identificativo.
    #[must_use]
    pub fn computo(&self, id: ComputoId) -> Option<&Computo> {
        self.computi.iter().find(|c| c.id == id)
    }

    /// Computo per identificativo, modificabile.
    pub fn computo_mut(&mut self, id: ComputoId) -> Option<&mut Computo> {
        self.computi.iter_mut().find(|c| c.id == id)
    }

    /// Aggiunge un computo.
    ///
    /// # Errors
    /// Identificativo duplicato.
    pub fn aggiungi_computo(&mut self, computo: Computo) -> Result<(), ErroreDominio> {
        if self.computo(computo.id).is_some() {
            return Err(ErroreDominio::IdDuplicato {
                entita: "computo",
                id: computo.id.to_string(),
            });
        }
        self.computi.push(computo);
        Ok(())
    }

    /// Calcola un computo con le regole del progetto.
    ///
    /// # Errors
    /// Computo inesistente o errore di calcolo localizzato.
    pub fn calcola_computo(&self, id: ComputoId) -> Result<RiepilogoComputo, ErroreDominio> {
        let computo = self.computo(id).ok_or_else(|| ErroreDominio::NonTrovato {
            entita: "computo",
            id: id.to_string(),
        })?;
        Ok(computo.calcola(&self.regole)?)
    }

    /// Parametri contabili del contratto.
    ///
    /// # Errors
    /// [`ErroreDominio::OperazioneNonConsentita`] se il contratto non è definito.
    pub fn parametri_contabili(&self) -> Result<&ParametriContabili, ErroreDominio> {
        self.contratto
            .as_ref()
            .map(|c| &c.parametri)
            .ok_or_else(|| {
                ErroreDominio::OperazioneNonConsentita(
                    "i dati contrattuali non sono definiti".into(),
                )
            })
    }

    /// Calcola un SAL con i parametri del contratto e le regole del progetto.
    ///
    /// # Errors
    /// Contratto non definito o errori di [`Contabilita::calcola_sal`].
    pub fn calcola_sal(&self, id: SalId) -> Result<RiepilogoSal, ErroreDominio> {
        self.contabilita
            .calcola_sal(id, self.parametri_contabili()?, &self.regole)
    }

    /// Certificato di pagamento di un SAL con i parametri del contratto.
    ///
    /// # Errors
    /// Contratto non definito o errori di [`Contabilita::calcola_certificato`].
    pub fn calcola_certificato(&self, id: SalId) -> Result<CertificatoPagamento, ErroreDominio> {
        self.contabilita
            .calcola_certificato(id, self.parametri_contabili()?, &self.regole)
    }

    /// Emette un SAL con i parametri del contratto e le regole del progetto.
    ///
    /// # Errors
    /// Contratto non definito o errori di [`Contabilita::emetti_sal`].
    pub fn emetti_sal(
        &mut self,
        id: SalId,
        data_emissione: Date,
    ) -> Result<RiepilogoSal, ErroreDominio> {
        let parametri = self.parametri_contabili()?.clone();
        self.contabilita
            .emetti_sal(id, data_emissione, &parametri, &self.regole)
    }

    /// Riepilogo del conto finale e della rata di saldo con i parametri del contratto.
    ///
    /// # Errors
    /// Contratto non definito o errori di [`Contabilita::calcola_conto_finale`].
    pub fn calcola_conto_finale(&self) -> Result<RiepilogoContoFinale, ErroreDominio> {
        self.contabilita
            .calcola_conto_finale(self.parametri_contabili()?, &self.regole)
    }

    /// Emette il conto finale con i parametri del contratto.
    ///
    /// # Errors
    /// Contratto non definito o errori di [`Contabilita::emetti_conto_finale`].
    pub fn emetti_conto_finale(
        &mut self,
        data: Date,
    ) -> Result<RiepilogoContoFinale, ErroreDominio> {
        let parametri = self.parametri_contabili()?.clone();
        self.contabilita
            .emetti_conto_finale(data, &parametri, &self.regole)
    }
}
