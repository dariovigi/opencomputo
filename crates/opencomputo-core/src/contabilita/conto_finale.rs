//! Stato finale, conto finale dei lavori e certificato di pagamento della rata di saldo.
//!
//! ## Calcolo
//!
//! ```text
//! stato finale         = tutte le registrazioni: quelle dei SAL emessi più quelle non
//!                        ancora contabilizzate («lavori dello stato finale»)
//! importo netto finale = lavori lordi + oneri sicurezza − ribasso        (come nei SAL)
//! ritenute svincolate  = importo netto finale × ritenuta %                (arrotondata)
//! acconti              = Σ importi dei SAL emessi (credito dell'ultimo SAL)
//! credito residuo      = importo netto finale − acconti
//! rata di saldo        = credito residuo − anticipazione non recuperata
//!                        − penali non applicate nei certificati
//! IVA                  = rata di saldo × aliquota                          (arrotondata)
//! totale               = rata di saldo + IVA
//! ```
//!
//! Il credito residuo comprende le ritenute di garanzia, che si svincolano con la
//! liquidazione finale dopo l'approvazione del collaudo o del certificato di regolare
//! esecuzione (D.Lgs. 36/2023, art. 11, comma 6), e i lavori dello stato finale.
//!
//! ## Procedura (Allegato II.14 al D.Lgs. 36/2023)
//!
//! 1. Il direttore dei lavori certifica l'ultimazione dei lavori (data di ultimazione).
//! 2. Compila il conto finale (bozza, poi emissione): all'emissione il riepilogo, le
//!    registrazioni dello stato finale e le penali applicate vengono congelati e la
//!    contabilità si chiude (niente più registrazioni, SAL o penali).
//! 3. L'esecutore firma il conto finale entro trenta giorni dall'invito del RUP,
//!    confermando le riserve già iscritte nel registro (art. 7, comma 3, e art. 12):
//!    non può iscriverne di nuove. Se non firma, il conto si intende accettato.
//! 4. Collaudo (entro sei mesi dall'ultimazione, fino a un anno nei casi complessi:
//!    art. 116, comma 2) oppure certificato di regolare esecuzione del direttore dei
//!    lavori (entro tre mesi: Allegato II.14, art. 28). Dopo, la rata di saldo è
//!    pagabile; prima dell'accettazione definitiva dell'opera serve la garanzia
//!    dell'art. 117, comma 9.
//!
//! I termini sono verificati dalla validazione come avvisi: il programma non li impone.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::sal::{ParametriContabili, StatoSal};
use super::{Contabilita, Penale, RegistrazioneLibretto};
use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
use crate::id::{PenaleId, RegistrazioneId};
use crate::numeri::{Importo, Percentuale, RegoleArrotondamento};
use crate::prezzario::NaturaVoce;
use crate::riferimento::Riferimento;

/// Tipo di verifica finale dei lavori.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoCollaudo {
    /// Certificato di collaudo (provvisorio; definitivo dopo due anni).
    Collaudo,
    /// Certificato di regolare esecuzione del direttore dei lavori.
    RegolareEsecuzione,
}

impl TipoCollaudo {
    /// Nome per esteso.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Self::Collaudo => "certificato di collaudo",
            Self::RegolareEsecuzione => "certificato di regolare esecuzione",
        }
    }
}

/// Collaudo o certificato di regolare esecuzione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collaudo {
    /// Tipo.
    pub tipo: TipoCollaudo,
    /// Data di emissione del certificato.
    pub data: Date,
    /// Data di approvazione da parte della stazione appaltante.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approvato_il: Option<Date>,
}

/// Riepilogo del conto finale e della rata di saldo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiepilogoContoFinale {
    /// Lavori lordi dello stato finale non compresi nei SAL.
    pub lordo_stato_finale: Importo,
    /// Lavori lordi complessivi.
    pub lavori_lordi: Importo,
    /// Oneri della sicurezza complessivi.
    pub oneri_sicurezza: Importo,
    /// Manodopera esclusa dal ribasso.
    pub manodopera_esclusa: Importo,
    /// Lavori a nuovi prezzi già netti, esclusi dal ribasso.
    #[serde(default, skip_serializing_if = "Importo::zero_ref")]
    pub lavori_esclusi_dal_ribasso: Importo,
    /// Importo soggetto a ribasso.
    pub soggetto_a_ribasso: Importo,
    /// Ribasso.
    pub ribasso: Importo,
    /// Importo netto finale dei lavori.
    pub importo_netto: Importo,
    /// Ritenute di garanzia da svincolare.
    pub ritenute_garanzia: Importo,
    /// Acconti: somma degli importi dei SAL emessi.
    pub acconti: Importo,
    /// Credito residuo dell'esecutore (importo netto − acconti).
    pub credito_residuo: Importo,
    /// Anticipazione erogata (zero se non prevista).
    pub anticipazione: Importo,
    /// Anticipazione non recuperata nei certificati, dedotta dalla rata di saldo.
    pub anticipazione_residua: Importo,
    /// Penali non applicate nei certificati.
    pub penali: Importo,
    /// Imponibile della rata di saldo.
    pub imponibile: Importo,
    /// Aliquota IVA.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliquota_iva: Option<Percentuale>,
    /// IVA.
    pub iva: Importo,
    /// Totale del certificato di pagamento della rata di saldo.
    pub totale: Importo,
}

/// Conto finale dei lavori.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContoFinale {
    /// Data di ultimazione dei lavori (certificato di ultimazione).
    pub data_ultimazione: Date,
    /// Data del conto finale.
    pub data: Date,
    /// Bozza o emesso.
    pub stato: StatoSal,
    /// Registrazioni dello stato finale (non comprese nei SAL), fissate all'emissione.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registrazioni: Vec<RegistrazioneId>,
    /// Riepilogo congelato all'emissione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub riepilogo_emesso: Option<RiepilogoContoFinale>,
    /// Penali applicate nella rata di saldo, fissate all'emissione.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub penali: Vec<PenaleId>,
    /// Data della firma dell'esecutore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firma_esecutore: Option<Date>,
    /// Riserve confermate dall'esecutore alla firma.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub riserve: Option<String>,
    /// Collaudo o certificato di regolare esecuzione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaudo: Option<Collaudo>,
    /// Note (relazione sul conto finale).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn non_esiste() -> ErroreDominio {
    ErroreDominio::OperazioneNonConsentita("il conto finale non è stato redatto".into())
}

fn in_calcolo(e: ErroreCalcolo) -> ErroreDominio {
    ErroreDominio::Calcolo(ErroreLocalizzato::new(Riferimento::ContoFinale, e))
}

impl Contabilita {
    /// Conto finale, se redatto.
    #[must_use]
    pub fn conto_finale(&self) -> Option<&ContoFinale> {
        self.conto_finale.as_ref()
    }

    /// `true` se il conto finale è emesso: la contabilità è chiusa.
    #[must_use]
    pub fn chiusa(&self) -> bool {
        self.conto_finale
            .as_ref()
            .is_some_and(|c| c.stato == StatoSal::Emesso)
    }

    /// Errore se il conto finale è emesso (contabilità chiusa).
    pub(crate) fn verifica_aperta(&self) -> Result<(), ErroreDominio> {
        if self.chiusa() {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "il conto finale è emesso: la contabilità è chiusa".into(),
            ));
        }
        Ok(())
    }

    /// Registrazioni dello stato finale: quelle fissate all'emissione oppure, per il
    /// conto in bozza, quelle non comprese in alcun SAL, in ordine di numero.
    #[must_use]
    pub fn registrazioni_stato_finale(&self) -> Vec<&RegistrazioneLibretto> {
        let mut elenco: Vec<&RegistrazioneLibretto> = match &self.conto_finale {
            Some(c) if c.stato == StatoSal::Emesso => c
                .registrazioni
                .iter()
                .filter_map(|id| self.libretto.registrazione(*id))
                .collect(),
            _ => self
                .libretto
                .registrazioni
                .iter()
                .map(Arc::as_ref)
                .filter(|r| self.sal_di(r.id).is_none())
                .collect(),
        };
        elenco.sort_by_key(|r| r.numero);
        elenco
    }

    /// Penali della rata di saldo: quelle fissate all'emissione oppure, per il conto in
    /// bozza, tutte quelle non applicate in un SAL emesso.
    #[must_use]
    pub fn penali_conto_finale(&self) -> Vec<&Penale> {
        match &self.conto_finale {
            Some(c) if c.stato == StatoSal::Emesso => self
                .penali
                .iter()
                .filter(|p| c.penali.contains(&p.id))
                .collect(),
            _ => self
                .penali
                .iter()
                .filter(|p| self.sal_della_penale(p.id).is_none())
                .collect(),
        }
    }

    /// Redige il conto finale in bozza o ne aggiorna date e note.
    ///
    /// # Errors
    /// Contabilità chiusa, SAL in bozza, date incoerenti o nessuna registrazione.
    pub fn redigi_conto_finale(
        &mut self,
        data_ultimazione: Date,
        data: Date,
        note: Option<String>,
    ) -> Result<&ContoFinale, ErroreDominio> {
        self.verifica_aperta()?;
        if let Some(s) = self.sal.iter().find(|s| s.stato == StatoSal::Bozza) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "il SAL n. {} è in bozza: emetterlo o annullarlo prima del conto finale",
                s.numero
            )));
        }
        if self.libretto.registrazioni.is_empty() {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "nessuna registrazione nel libretto delle misure".into(),
            ));
        }
        if let Some(s) = self
            .sal
            .last()
            .filter(|s| s.lavori_a_tutto_il > data_ultimazione)
        {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "l'ultimazione ({data_ultimazione}) precede i lavori del SAL n. {} ({})",
                s.numero, s.lavori_a_tutto_il
            )));
        }
        if data < data_ultimazione {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la data del conto finale ({data}) precede l'ultimazione dei lavori \
                 ({data_ultimazione})"
            )));
        }
        let conto = match self.conto_finale.take() {
            Some(c) => ContoFinale {
                data_ultimazione,
                data,
                note,
                ..c
            },
            None => ContoFinale {
                data_ultimazione,
                data,
                stato: StatoSal::Bozza,
                registrazioni: Vec::new(),
                riepilogo_emesso: None,
                penali: Vec::new(),
                firma_esecutore: None,
                riserve: None,
                collaudo: None,
                note,
            },
        };
        Ok(self.conto_finale.insert(conto))
    }

    /// Annulla il conto finale in bozza.
    ///
    /// # Errors
    /// Conto finale assente o già emesso.
    pub fn annulla_conto_finale(&mut self) -> Result<ContoFinale, ErroreDominio> {
        self.verifica_aperta()?;
        self.conto_finale.take().ok_or_else(non_esiste)
    }

    /// Calcola il riepilogo del conto finale: congelato se emesso, altrimenti
    /// ricalcolato (vedi il modulo).
    ///
    /// # Errors
    /// Conto finale assente, SAL in bozza o non emessi, riferimenti incoerenti, errori di
    /// calcolo.
    pub fn calcola_conto_finale(
        &self,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<RiepilogoContoFinale, ErroreDominio> {
        let conto = self.conto_finale.as_ref().ok_or_else(non_esiste)?;
        if let (StatoSal::Emesso, Some(r)) = (&conto.stato, &conto.riepilogo_emesso) {
            return Ok(r.clone());
        }
        self.ricalcola_conto_finale(parametri, regole)
    }

    /// Ricalcola sempre il riepilogo, anche per un conto emesso.
    pub(crate) fn ricalcola_conto_finale(
        &self,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<RiepilogoContoFinale, ErroreDominio> {
        if let Some(s) = self.sal.iter().find(|s| s.stato != StatoSal::Emesso) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "il SAL n. {} non è emesso",
                s.numero
            )));
        }
        // Registrazioni dei SAL emessi e dello stato finale.
        let mut ids: Vec<RegistrazioneId> = self
            .sal
            .iter()
            .flat_map(|s| s.registrazioni.iter().copied())
            .collect();
        let finali: Vec<RegistrazioneId> = self
            .registrazioni_stato_finale()
            .iter()
            .map(|r| r.id)
            .collect();
        if let Some(c) = self
            .conto_finale
            .as_ref()
            .filter(|c| c.stato == StatoSal::Emesso)
            && let Some(mancante) = c
                .registrazioni
                .iter()
                .find(|id| self.libretto.registrazione(**id).is_none())
        {
            return Err(ErroreDominio::RiferimentoNonValido(format!(
                "il conto finale riferisce la registrazione inesistente {mancante}"
            )));
        }
        ids.extend(finali.iter().copied());

        let mut lavori_lordi = Importo::ZERO;
        let mut oneri_sicurezza = Importo::ZERO;
        let mut manodopera_esclusa = Importo::ZERO;
        let mut esclusi = Importo::ZERO;
        let mut lordo_stato_finale = Importo::ZERO;
        let mut viste = std::collections::BTreeSet::new();
        for id in &ids {
            if !viste.insert(*id) {
                return Err(ErroreDominio::RiferimentoNonValido(format!(
                    "la registrazione {id} è contabilizzata due volte"
                )));
            }
            let reg = self.libretto.registrazione(*id).ok_or_else(|| {
                ErroreDominio::RiferimentoNonValido(format!("registrazione inesistente {id}"))
            })?;
            let c = reg
                .calcola(regole)
                .map_err(|e| ErroreDominio::Calcolo(e.dentro(Riferimento::ContoFinale)))?;
            match reg.prezzo.natura {
                NaturaVoce::Lavoro => {
                    lavori_lordi = lavori_lordi.somma(c.importo).map_err(in_calcolo)?;
                    if reg.prezzo.escluso_dal_ribasso {
                        esclusi = esclusi.somma(c.importo).map_err(in_calcolo)?;
                    } else if parametri.manodopera_esclusa_dal_ribasso {
                        let m = c.importo_manodopera.unwrap_or(Importo::ZERO);
                        manodopera_esclusa = manodopera_esclusa.somma(m).map_err(in_calcolo)?;
                    }
                }
                NaturaVoce::OneriSicurezza => {
                    oneri_sicurezza = oneri_sicurezza.somma(c.importo).map_err(in_calcolo)?;
                }
            }
            if finali.contains(id) {
                lordo_stato_finale = lordo_stato_finale.somma(c.importo).map_err(in_calcolo)?;
            }
        }

        let acconti = match self.sal.last() {
            None => Importo::ZERO,
            Some(s) => match &s.riepilogo_emesso {
                Some(r) => r.credito_progressivo,
                None => {
                    self.calcola_sal(s.id, parametri, regole)?
                        .credito_progressivo
                }
            },
        };
        let (anticipazione, anticipazione_residua) =
            match (&parametri.anticipazione, self.sal.last()) {
                (None, _) => (Importo::ZERO, Importo::ZERO),
                (Some(a), None) => (a.importo, a.importo),
                (Some(a), Some(s)) => {
                    let c = self.calcola_certificato(s.id, parametri, regole)?;
                    (a.importo, c.anticipazione_residua)
                }
            };
        let penali = Importo::totale(self.penali_conto_finale().iter().map(|p| p.importo))
            .map_err(in_calcolo)?;

        let calcolo = || -> Result<RiepilogoContoFinale, ErroreCalcolo> {
            let soggetto_a_ribasso = lavori_lordi
                .differenza(manodopera_esclusa)?
                .differenza(esclusi)?;
            let ribasso = parametri.ribasso.quota_di(soggetto_a_ribasso, regole)?;
            let importo_netto = lavori_lordi.somma(oneri_sicurezza)?.differenza(ribasso)?;
            let ritenute_garanzia = parametri
                .ritenuta_garanzia
                .quota_di(importo_netto, regole)?;
            let credito_residuo = importo_netto.differenza(acconti)?;
            let imponibile = credito_residuo
                .differenza(anticipazione_residua)?
                .differenza(penali)?;
            let iva = match parametri.aliquota_iva {
                Some(a) => a.quota_di(imponibile, regole)?,
                None => Importo::ZERO,
            };
            Ok(RiepilogoContoFinale {
                lordo_stato_finale,
                lavori_lordi,
                oneri_sicurezza,
                manodopera_esclusa,
                lavori_esclusi_dal_ribasso: esclusi,
                soggetto_a_ribasso,
                ribasso,
                importo_netto,
                ritenute_garanzia,
                acconti,
                credito_residuo,
                anticipazione,
                anticipazione_residua,
                penali,
                imponibile,
                aliquota_iva: parametri.aliquota_iva,
                iva,
                totale: imponibile.somma(iva)?,
            })
        };
        calcolo().map_err(in_calcolo)
    }

    /// Emette il conto finale: congela riepilogo, registrazioni dello stato finale e
    /// penali applicate, e chiude la contabilità.
    ///
    /// # Errors
    /// Conto finale assente o già emesso, data anteriore alla redazione, errori di
    /// [`Self::calcola_conto_finale`].
    pub fn emetti_conto_finale(
        &mut self,
        data: Date,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<RiepilogoContoFinale, ErroreDominio> {
        self.verifica_aperta()?;
        let conto = self.conto_finale.as_ref().ok_or_else(non_esiste)?;
        if data < conto.data_ultimazione {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la data di emissione ({data}) precede l'ultimazione dei lavori ({})",
                conto.data_ultimazione
            )));
        }
        let riepilogo = self.ricalcola_conto_finale(parametri, regole)?;
        let registrazioni = self
            .registrazioni_stato_finale()
            .iter()
            .map(|r| r.id)
            .collect();
        let penali = self.penali_conto_finale().iter().map(|p| p.id).collect();
        let conto = self.conto_finale.as_mut().ok_or_else(non_esiste)?;
        conto.stato = StatoSal::Emesso;
        conto.data = data;
        conto.registrazioni = registrazioni;
        conto.penali = penali;
        conto.riepilogo_emesso = Some(riepilogo.clone());
        Ok(riepilogo)
    }

    /// Registra la firma dell'esecutore sul conto finale emesso, con le riserve
    /// confermate.
    ///
    /// # Errors
    /// Conto finale assente o non emesso, data anteriore al conto.
    pub fn firma_conto_finale(
        &mut self,
        data: Option<Date>,
        riserve: Option<String>,
    ) -> Result<(), ErroreDominio> {
        let conto = self.conto_emesso_mut()?;
        if let Some(d) = data.filter(|d| *d < conto.data) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la firma ({d}) precede il conto finale ({})",
                conto.data
            )));
        }
        conto.firma_esecutore = data;
        conto.riserve = riserve.filter(|r| !r.trim().is_empty());
        Ok(())
    }

    /// Registra (o rimuove) il collaudo o il certificato di regolare esecuzione.
    ///
    /// # Errors
    /// Conto finale assente o non emesso, date incoerenti.
    pub fn registra_collaudo(&mut self, collaudo: Option<Collaudo>) -> Result<(), ErroreDominio> {
        let conto = self.conto_emesso_mut()?;
        if let Some(c) = &collaudo {
            if c.data < conto.data_ultimazione {
                return Err(ErroreDominio::OperazioneNonConsentita(format!(
                    "il {} ({}) precede l'ultimazione dei lavori ({})",
                    c.tipo.nome(),
                    c.data,
                    conto.data_ultimazione
                )));
            }
            if let Some(a) = c.approvato_il.filter(|a| *a < c.data) {
                return Err(ErroreDominio::OperazioneNonConsentita(format!(
                    "l'approvazione ({a}) precede il {} ({})",
                    c.tipo.nome(),
                    c.data
                )));
            }
        }
        conto.collaudo = collaudo;
        Ok(())
    }

    fn conto_emesso_mut(&mut self) -> Result<&mut ContoFinale, ErroreDominio> {
        match self.conto_finale.as_mut() {
            None => Err(non_esiste()),
            Some(c) if c.stato != StatoSal::Emesso => Err(ErroreDominio::OperazioneNonConsentita(
                "il conto finale non è ancora emesso".into(),
            )),
            Some(c) => Ok(c),
        }
    }
}
