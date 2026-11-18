//! Contabilità dei lavori: libretto delle misure, registro di contabilità e SAL.
//!
//! ## Regole di integrità
//!
//! * ogni registrazione ha identificativo e numero d'ordine univoci;
//! * una registrazione è contabilizzata in al più un SAL;
//! * i SAL sono numerati 1, 2, … con date «lavori a tutto il» crescenti;
//! * può esistere un solo SAL in bozza, sempre l'ultimo;
//! * un SAL si emette solo se tutti i precedenti sono emessi;
//! * all'emissione il riepilogo viene congelato: le registrazioni contabilizzate
//!   in un SAL emesso non possono più essere modificate né rimosse tramite le
//!   operazioni del modello;
//! * dopo l'emissione del conto finale la contabilità è chiusa: niente più
//!   registrazioni, SAL o penali (modulo [`conto_finale`]);
//! * [`Contabilita::verifica_integrita`] rileva comunque le incoerenze
//!   presenti in dati caricati da file o alterati esternamente, confrontando
//!   i riepiloghi congelati con il ricalcolo.
//!
//! I campi sono privati: le modifiche passano dalle operazioni che applicano
//! queste regole.

pub mod certificato;
pub mod conto_finale;
pub mod libretto;
pub mod nuovi_prezzi;
pub mod registro;
pub mod sal;
pub mod sommario;

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
use crate::id::{PenaleId, RegistrazioneId, SalId};
use crate::numeri::{Importo, RegoleArrotondamento};
use crate::prezzario::NaturaVoce;
use crate::riferimento::Riferimento;
use crate::validazione::Problema;

pub use certificato::{Anticipazione, CertificatoPagamento, Penale};
pub use conto_finale::{Collaudo, ContoFinale, RiepilogoContoFinale, TipoCollaudo};
pub use libretto::{LibrettoMisure, RegistrazioneLibretto};
pub use nuovi_prezzi::VerbaleNuoviPrezzi;
pub use registro::{RegistroContabilita, RigaRegistro, TotaleSalRegistro};
pub use sal::{ParametriContabili, RiepilogoSal, StatoAvanzamentoLavori, StatoSal};
pub use sommario::{ArticoloSommario, SommarioRegistro};

/// Contabilità di un appalto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contabilita {
    #[serde(default)]
    libretto: LibrettoMisure,
    #[serde(default)]
    sal: Vec<StatoAvanzamentoLavori>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    penali: Vec<Penale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conto_finale: Option<ContoFinale>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    verbali_nuovi_prezzi: Vec<VerbaleNuoviPrezzi>,
}

fn rif_sal(s: &StatoAvanzamentoLavori) -> Riferimento {
    Riferimento::Sal {
        numero: s.numero,
        id: s.id,
    }
}

fn sal_non_trovato(id: SalId) -> ErroreDominio {
    ErroreDominio::NonTrovato {
        entita: "SAL",
        id: id.to_string(),
    }
}

fn registrazione_non_trovata(id: RegistrazioneId) -> ErroreDominio {
    ErroreDominio::NonTrovato {
        entita: "registrazione",
        id: id.to_string(),
    }
}

impl Contabilita {
    /// Libretto delle misure (sola lettura).
    #[must_use]
    pub fn libretto(&self) -> &LibrettoMisure {
        &self.libretto
    }

    /// SAL in ordine di numero (sola lettura).
    #[must_use]
    pub fn sal(&self) -> &[StatoAvanzamentoLavori] {
        &self.sal
    }

    /// SAL per identificativo.
    #[must_use]
    pub fn sal_per_id(&self, id: SalId) -> Option<&StatoAvanzamentoLavori> {
        self.sal.iter().find(|s| s.id == id)
    }

    /// SAL in cui è contabilizzata una registrazione.
    #[must_use]
    pub fn sal_di(&self, registrazione: RegistrazioneId) -> Option<&StatoAvanzamentoLavori> {
        self.sal
            .iter()
            .find(|s| s.registrazioni.contains(&registrazione))
    }

    fn verifica_numero(
        &self,
        numero: u32,
        escluso: Option<RegistrazioneId>,
    ) -> Result<(), ErroreDominio> {
        if numero == 0 {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "il numero d'ordine di una registrazione deve essere almeno 1".into(),
            ));
        }
        if self
            .libretto
            .registrazioni
            .iter()
            .any(|r| r.numero == numero && Some(r.id) != escluso)
        {
            return Err(ErroreDominio::CodiceDuplicato {
                entita: "registrazione",
                codice: numero.to_string(),
            });
        }
        Ok(())
    }

    /// Aggiunge una registrazione al libretto.
    ///
    /// # Errors
    /// Identificativo o numero d'ordine già presenti, oppure numero nullo.
    pub fn registra(&mut self, registrazione: RegistrazioneLibretto) -> Result<(), ErroreDominio> {
        self.verifica_aperta()?;
        if self.libretto.registrazione(registrazione.id).is_some() {
            return Err(ErroreDominio::IdDuplicato {
                entita: "registrazione",
                id: registrazione.id.to_string(),
            });
        }
        self.verifica_numero(registrazione.numero, None)?;
        self.libretto.registrazioni.push(registrazione);
        Ok(())
    }

    /// Modifica una registrazione non ancora contabilizzata in un SAL emesso.
    ///
    /// La modifica è applicata a una copia e accettata solo se rispetta le
    /// regole (identificativo invariato, numero univoco, data entro il periodo
    /// dell'eventuale SAL in bozza).
    ///
    /// # Errors
    /// Registrazione inesistente o bloccata, oppure modifica non ammessa.
    pub fn modifica_registrazione<F>(
        &mut self,
        id: RegistrazioneId,
        modifica: F,
    ) -> Result<(), ErroreDominio>
    where
        F: FnOnce(&mut RegistrazioneLibretto),
    {
        self.verifica_aperta()?;
        let posizione = self
            .libretto
            .registrazioni
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| registrazione_non_trovata(id))?;
        let sal = self.sal_di(id);
        if let Some(s) = sal.filter(|s| s.stato == StatoSal::Emesso) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la registrazione è contabilizzata nel SAL n. {} già emesso",
                s.numero
            )));
        }
        let limite = sal.map(|s| s.lavori_a_tutto_il);
        let mut copia = self
            .libretto
            .registrazioni
            .get(posizione)
            .cloned()
            .ok_or_else(|| registrazione_non_trovata(id))?;
        modifica(&mut copia);
        if copia.id != id {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "l'identificativo di una registrazione non può essere modificato".into(),
            ));
        }
        self.verifica_numero(copia.numero, Some(id))?;
        if let Some(limite) = limite.filter(|l| copia.data > *l) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la data {} è successiva al periodo del SAL in bozza (lavori a tutto il {limite})",
                copia.data
            )));
        }
        if let Some(r) = self.libretto.registrazioni.get_mut(posizione) {
            *r = copia;
        }
        Ok(())
    }

    /// Rimuove una registrazione non contabilizzata in un SAL emesso; se è in
    /// un SAL in bozza viene tolta anche da quello.
    ///
    /// # Errors
    /// Registrazione inesistente o bloccata.
    pub fn rimuovi_registrazione(
        &mut self,
        id: RegistrazioneId,
    ) -> Result<RegistrazioneLibretto, ErroreDominio> {
        self.verifica_aperta()?;
        if let Some(s) = self.sal_di(id).filter(|s| s.stato == StatoSal::Emesso) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la registrazione è contabilizzata nel SAL n. {} già emesso",
                s.numero
            )));
        }
        let posizione = self
            .libretto
            .registrazioni
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| registrazione_non_trovata(id))?;
        for s in &mut self.sal {
            s.registrazioni.retain(|r| *r != id);
        }
        Ok(self.libretto.registrazioni.remove(posizione))
    }

    /// Registrazioni non ancora attribuite a un SAL con data non successiva a
    /// `fino_al`, in ordine di numero.
    #[must_use]
    pub fn registrazioni_da_contabilizzare(&self, fino_al: Date) -> Vec<&RegistrazioneLibretto> {
        let assegnate: BTreeSet<RegistrazioneId> = self
            .sal
            .iter()
            .flat_map(|s| s.registrazioni.iter().copied())
            .collect();
        let mut elenco: Vec<&RegistrazioneLibretto> = self
            .libretto
            .registrazioni
            .iter()
            .filter(|r| r.data <= fino_al && !assegnate.contains(&r.id))
            .collect();
        elenco.sort_by_key(|r| r.numero);
        elenco
    }

    /// Crea un nuovo SAL in bozza con tutte le registrazioni da contabilizzare
    /// fino alla data indicata.
    ///
    /// # Errors
    /// Identificativo duplicato, SAL precedente non emesso, data non successiva
    /// al SAL precedente o nessuna registrazione da contabilizzare.
    pub fn nuovo_sal(
        &mut self,
        id: SalId,
        lavori_a_tutto_il: Date,
    ) -> Result<&StatoAvanzamentoLavori, ErroreDominio> {
        self.verifica_aperta()?;
        if self.conto_finale.is_some() {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "il conto finale è in bozza: annullarlo per redigere un altro SAL".into(),
            ));
        }
        if self.sal_per_id(id).is_some() {
            return Err(ErroreDominio::IdDuplicato {
                entita: "SAL",
                id: id.to_string(),
            });
        }
        let numero = match self.sal.last() {
            None => 1,
            Some(ultimo) => {
                if ultimo.stato != StatoSal::Emesso {
                    return Err(ErroreDominio::OperazioneNonConsentita(format!(
                        "il SAL n. {} è ancora in bozza",
                        ultimo.numero
                    )));
                }
                if lavori_a_tutto_il <= ultimo.lavori_a_tutto_il {
                    return Err(ErroreDominio::OperazioneNonConsentita(format!(
                        "la data {lavori_a_tutto_il} non è successiva a quella del SAL n. {} ({})",
                        ultimo.numero, ultimo.lavori_a_tutto_il
                    )));
                }
                ultimo.numero.saturating_add(1)
            }
        };
        let registrazioni: Vec<RegistrazioneId> = self
            .registrazioni_da_contabilizzare(lavori_a_tutto_il)
            .iter()
            .map(|r| r.id)
            .collect();
        if registrazioni.is_empty() {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "nessuna registrazione da contabilizzare fino al {lavori_a_tutto_il}"
            )));
        }
        self.sal.push(StatoAvanzamentoLavori {
            id,
            numero,
            lavori_a_tutto_il,
            data_emissione: None,
            stato: StatoSal::Bozza,
            registrazioni,
            riepilogo_emesso: None,
            certificato_emesso: None,
            penali: Vec::new(),
            recupero_anticipazione_sospeso: false,
            recupera_residuo_anticipazione: false,
            note: None,
        });
        self.sal.last().ok_or_else(|| sal_non_trovato(id))
    }

    /// Annulla un SAL in bozza; le sue registrazioni tornano da contabilizzare.
    ///
    /// # Errors
    /// SAL inesistente o già emesso.
    pub fn annulla_bozza_sal(
        &mut self,
        id: SalId,
    ) -> Result<StatoAvanzamentoLavori, ErroreDominio> {
        let posizione = self
            .sal
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| sal_non_trovato(id))?;
        if self
            .sal
            .get(posizione)
            .is_some_and(|s| s.stato == StatoSal::Emesso)
        {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "un SAL emesso non può essere annullato".into(),
            ));
        }
        Ok(self.sal.remove(posizione))
    }

    /// Calcola il riepilogo di un SAL (in bozza o emesso).
    ///
    /// # Errors
    /// SAL inesistente, SAL precedenti non emessi, riferimenti incoerenti o
    /// errori di calcolo.
    pub fn calcola_sal(
        &self,
        id: SalId,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<RiepilogoSal, ErroreDominio> {
        let posizione = self
            .sal
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| sal_non_trovato(id))?;
        let fino_a_questo = self.sal.get(..=posizione).unwrap_or_default();
        let (corrente, precedenti) = fino_a_questo
            .split_last()
            .ok_or_else(|| sal_non_trovato(id))?;
        for p in precedenti {
            if p.stato != StatoSal::Emesso || p.riepilogo_emesso.is_none() {
                return Err(ErroreDominio::OperazioneNonConsentita(format!(
                    "il SAL n. {} precedente non è stato emesso",
                    p.numero
                )));
            }
        }
        let certificati_precedenti = precedenti
            .last()
            .and_then(|p| p.riepilogo_emesso.as_ref())
            .map_or(Importo::ZERO, |r| r.credito_progressivo);
        let in_calcolo =
            |e: ErroreCalcolo| ErroreDominio::Calcolo(ErroreLocalizzato::new(rif_sal(corrente), e));

        let mut viste = BTreeSet::new();
        let mut lavori_lordi = Importo::ZERO;
        let mut oneri_sicurezza = Importo::ZERO;
        let mut manodopera_esclusa = Importo::ZERO;
        let mut esclusi = Importo::ZERO;
        let mut lordo_periodo = Importo::ZERO;
        for s in fino_a_questo {
            for reg_id in &s.registrazioni {
                if !viste.insert(*reg_id) {
                    return Err(ErroreDominio::RiferimentoNonValido(format!(
                        "la registrazione {reg_id} è contabilizzata in più SAL"
                    )));
                }
                let reg = self.libretto.registrazione(*reg_id).ok_or_else(|| {
                    ErroreDominio::RiferimentoNonValido(format!(
                        "il SAL n. {} riferisce la registrazione inesistente {reg_id}",
                        s.numero
                    ))
                })?;
                let c = reg
                    .calcola(regole)
                    .map_err(|e| ErroreDominio::Calcolo(e.dentro(rif_sal(s))))?;
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
                if s.id == corrente.id {
                    lordo_periodo = lordo_periodo.somma(c.importo).map_err(in_calcolo)?;
                }
            }
        }

        let calcolo = || -> Result<RiepilogoSal, ErroreCalcolo> {
            let soggetto_a_ribasso = lavori_lordi
                .differenza(manodopera_esclusa)?
                .differenza(esclusi)?;
            let ribasso = parametri.ribasso.quota_di(soggetto_a_ribasso, regole)?;
            let importo_netto = lavori_lordi.somma(oneri_sicurezza)?.differenza(ribasso)?;
            let ritenuta_garanzia = parametri
                .ritenuta_garanzia
                .quota_di(importo_netto, regole)?;
            let credito_progressivo = importo_netto.differenza(ritenuta_garanzia)?;
            let importo_sal = credito_progressivo.differenza(certificati_precedenti)?;
            Ok(RiepilogoSal {
                numero: corrente.numero,
                lordo_periodo,
                lavori_lordi,
                oneri_sicurezza,
                manodopera_esclusa,
                lavori_esclusi_dal_ribasso: esclusi,
                soggetto_a_ribasso,
                ribasso,
                importo_netto,
                ritenuta_garanzia,
                credito_progressivo,
                certificati_precedenti,
                importo_sal,
            })
        };
        calcolo().map_err(in_calcolo)
    }

    /// Emette un SAL in bozza: calcola il riepilogo, lo congela e blocca le
    /// registrazioni incluse.
    ///
    /// # Errors
    /// SAL inesistente o già emesso, data di emissione anteriore al periodo,
    /// oppure gli errori di [`Self::calcola_sal`].
    pub fn emetti_sal(
        &mut self,
        id: SalId,
        data_emissione: Date,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<RiepilogoSal, ErroreDominio> {
        let sal = self.sal_per_id(id).ok_or_else(|| sal_non_trovato(id))?;
        if sal.stato == StatoSal::Emesso {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "il SAL n. {} è già emesso",
                sal.numero
            )));
        }
        if data_emissione < sal.lavori_a_tutto_il {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la data di emissione {data_emissione} precede la data dei lavori ({})",
                sal.lavori_a_tutto_il
            )));
        }
        let riepilogo = self.calcola_sal(id, parametri, regole)?;
        let certificato = self.calcola_certificato(id, parametri, regole)?;
        let penali: Vec<PenaleId> = {
            let s = self.sal_per_id(id).ok_or_else(|| sal_non_trovato(id))?;
            self.penali_del_sal(s).iter().map(|p| p.id).collect()
        };
        let sal = self
            .sal
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| sal_non_trovato(id))?;
        sal.stato = StatoSal::Emesso;
        sal.data_emissione = Some(data_emissione);
        sal.riepilogo_emesso = Some(riepilogo.clone());
        sal.certificato_emesso = Some(certificato);
        sal.penali = penali;
        Ok(riepilogo)
    }

    /// Penali registrate.
    #[must_use]
    pub fn penali(&self) -> &[Penale] {
        &self.penali
    }

    /// Prossimo numero d'ordine delle penali.
    #[must_use]
    pub fn prossimo_numero_penale(&self) -> u32 {
        self.penali
            .iter()
            .map(|p| p.numero)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    /// Registra una penale.
    ///
    /// # Errors
    /// Identificativo o numero già presenti, importo negativo.
    pub fn registra_penale(&mut self, penale: Penale) -> Result<(), ErroreDominio> {
        self.verifica_aperta()?;
        if self.penali.iter().any(|p| p.id == penale.id) {
            return Err(ErroreDominio::IdDuplicato {
                entita: "penale",
                id: penale.id.to_string(),
            });
        }
        if self.penali.iter().any(|p| p.numero == penale.numero) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "penale n. {} già presente",
                penale.numero
            )));
        }
        if penale.importo.is_negativo() {
            return Err(ErroreDominio::OperazioneNonConsentita(
                "l'importo della penale non può essere negativo".into(),
            ));
        }
        self.penali.push(penale);
        Ok(())
    }

    /// SAL emesso in cui la penale è stata applicata.
    #[must_use]
    pub fn sal_della_penale(&self, id: PenaleId) -> Option<&StatoAvanzamentoLavori> {
        self.sal
            .iter()
            .find(|s| s.stato == StatoSal::Emesso && s.penali.contains(&id))
    }

    /// Rimuove una penale non ancora applicata in un SAL emesso.
    ///
    /// # Errors
    /// Penale inesistente o già applicata.
    pub fn rimuovi_penale(&mut self, id: PenaleId) -> Result<Penale, ErroreDominio> {
        self.verifica_aperta()?;
        if let Some(s) = self.sal_della_penale(id) {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "la penale è applicata nel SAL n. {} emesso",
                s.numero
            )));
        }
        let posizione = self.penali.iter().position(|p| p.id == id).ok_or_else(|| {
            ErroreDominio::NonTrovato {
                entita: "penale",
                id: id.to_string(),
            }
        })?;
        Ok(self.penali.remove(posizione))
    }

    /// Penali di un SAL: quelle fissate all'emissione, oppure, per un SAL in bozza,
    /// quelle con data fino ai lavori del SAL non applicate in un SAL emesso.
    fn penali_del_sal(&self, s: &StatoAvanzamentoLavori) -> Vec<&Penale> {
        if s.stato == StatoSal::Emesso {
            return self
                .penali
                .iter()
                .filter(|p| s.penali.contains(&p.id))
                .collect();
        }
        self.penali
            .iter()
            .filter(|p| p.data <= s.lavori_a_tutto_il && self.sal_della_penale(p.id).is_none())
            .collect()
    }

    /// Imposta le opzioni di recupero dell'anticipazione di un SAL in bozza.
    ///
    /// # Errors
    /// SAL inesistente o già emesso.
    pub fn imposta_recupero_anticipazione(
        &mut self,
        id: SalId,
        sospeso: Option<bool>,
        recupera_residuo: Option<bool>,
    ) -> Result<(), ErroreDominio> {
        let s = self
            .sal
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| sal_non_trovato(id))?;
        if s.stato == StatoSal::Emesso {
            return Err(ErroreDominio::OperazioneNonConsentita(format!(
                "il SAL n. {} è emesso: il certificato non è modificabile",
                s.numero
            )));
        }
        if let Some(v) = sospeso {
            s.recupero_anticipazione_sospeso = v;
        }
        if let Some(v) = recupera_residuo {
            s.recupera_residuo_anticipazione = v;
        }
        Ok(())
    }

    /// Certificato di pagamento di un SAL: congelato se il SAL è emesso con questa
    /// versione, altrimenti calcolato (vedi il modulo [`certificato`]).
    ///
    /// # Errors
    /// Gli errori di [`Self::calcola_sal`] o di calcolo.
    pub fn calcola_certificato(
        &self,
        id: SalId,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<CertificatoPagamento, ErroreDominio> {
        let posizione = self
            .sal
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| sal_non_trovato(id))?;
        let mut recuperata = Importo::ZERO;
        for s in self.sal.get(..=posizione).unwrap_or_default() {
            let certificato = match (&s.stato, &s.certificato_emesso) {
                (StatoSal::Emesso, Some(c)) => c.clone(),
                _ => self.certificato_di(s, recuperata, parametri, regole)?,
            };
            if s.id == id {
                return Ok(certificato);
            }
            recuperata = recuperata
                .somma(certificato.recupero_anticipazione)
                .map_err(|e| ErroreDominio::Calcolo(ErroreLocalizzato::new(rif_sal(s), e)))?;
        }
        Err(sal_non_trovato(id))
    }

    /// Calcola il certificato di un SAL dato quanto già recuperato dell'anticipazione.
    fn certificato_di(
        &self,
        s: &StatoAvanzamentoLavori,
        recuperata_prima: Importo,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Result<CertificatoPagamento, ErroreDominio> {
        let riepilogo = match (&s.stato, &s.riepilogo_emesso) {
            (StatoSal::Emesso, Some(r)) => r.clone(),
            _ => self.calcola_sal(s.id, parametri, regole)?,
        };
        let penali: Vec<Importo> = self.penali_del_sal(s).iter().map(|p| p.importo).collect();
        let calcolo = || -> Result<CertificatoPagamento, ErroreCalcolo> {
            let (anticipazione, recupero) = match &parametri.anticipazione {
                None => (Importo::ZERO, Importo::ZERO),
                Some(a) => {
                    let residuo = a.importo.differenza(recuperata_prima)?;
                    let residuo = if residuo.is_negativo() {
                        Importo::ZERO
                    } else {
                        residuo
                    };
                    let recupero = if s.recupero_anticipazione_sospeso {
                        Importo::ZERO
                    } else if s.recupera_residuo_anticipazione {
                        residuo
                    } else {
                        let proporzionale = a
                            .percentuale_recupero
                            .quota_di(riepilogo.importo_netto, regole)?;
                        let dovuto = if proporzionale.valore() > a.importo.valore() {
                            a.importo
                        } else {
                            proporzionale
                        };
                        let r = dovuto.differenza(recuperata_prima)?;
                        if r.is_negativo() {
                            Importo::ZERO
                        } else if r.valore() > residuo.valore() {
                            residuo
                        } else {
                            r
                        }
                    };
                    (a.importo, recupero)
                }
            };
            let anticipazione_residua = if anticipazione.is_zero() {
                Importo::ZERO
            } else {
                anticipazione
                    .differenza(recuperata_prima)?
                    .differenza(recupero)?
            };
            let penali = Importo::totale(penali.iter().copied())?;
            let imponibile = riepilogo
                .importo_sal
                .differenza(recupero)?
                .differenza(penali)?;
            let iva = match parametri.aliquota_iva {
                Some(a) => a.quota_di(imponibile, regole)?,
                None => Importo::ZERO,
            };
            Ok(CertificatoPagamento {
                numero: s.numero,
                importo_sal: riepilogo.importo_sal,
                anticipazione,
                anticipazione_recuperata_prima: if anticipazione.is_zero() {
                    Importo::ZERO
                } else {
                    recuperata_prima
                },
                recupero_anticipazione: recupero,
                anticipazione_residua,
                recupero_sospeso: s.recupero_anticipazione_sospeso && !anticipazione.is_zero(),
                penali,
                imponibile,
                aliquota_iva: parametri.aliquota_iva,
                iva,
                totale: imponibile.somma(iva)?,
            })
        };
        calcolo().map_err(|e| ErroreDominio::Calcolo(ErroreLocalizzato::new(rif_sal(s), e)))
    }

    /// Genera il registro di contabilità.
    ///
    /// # Errors
    /// Riferimenti incoerenti o errori di calcolo.
    pub fn registro(
        &self,
        regole: &RegoleArrotondamento,
    ) -> Result<RegistroContabilita, ErroreDominio> {
        let mut righe = Vec::new();
        let mut totali = Vec::with_capacity(self.sal.len());
        let mut progressivo = Importo::ZERO;
        let mut numero: u32 = 0;
        for s in &self.sal {
            let in_calcolo =
                |e: ErroreCalcolo| ErroreDominio::Calcolo(ErroreLocalizzato::new(rif_sal(s), e));
            let mut registrazioni = s
                .registrazioni
                .iter()
                .map(|id| {
                    self.libretto.registrazione(*id).ok_or_else(|| {
                        ErroreDominio::RiferimentoNonValido(format!(
                            "il SAL n. {} riferisce la registrazione inesistente {id}",
                            s.numero
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            registrazioni.sort_by_key(|r| r.numero);
            let mut periodo = Importo::ZERO;
            for reg in registrazioni {
                let c = reg
                    .calcola(regole)
                    .map_err(|e| ErroreDominio::Calcolo(e.dentro(rif_sal(s))))?;
                numero = numero.saturating_add(1);
                periodo = periodo.somma(c.importo).map_err(in_calcolo)?;
                righe.push(RigaRegistro {
                    numero,
                    sal: s.id,
                    numero_sal: s.numero,
                    registrazione: reg.id,
                    numero_libretto: reg.numero,
                    data: reg.data,
                    codice: reg.prezzo.codice.clone(),
                    descrizione: reg.prezzo.descrizione_breve.clone(),
                    unita_misura: reg.prezzo.unita_misura.clone(),
                    natura: reg.prezzo.natura,
                    quantita: c.quantita,
                    prezzo: reg.prezzo.prezzo,
                    importo: c.importo,
                });
            }
            progressivo = progressivo.somma(periodo).map_err(in_calcolo)?;
            totali.push(TotaleSalRegistro {
                sal: s.id,
                numero_sal: s.numero,
                importo_periodo: periodo,
                importo_progressivo: progressivo,
            });
        }
        Ok(RegistroContabilita { righe, totali })
    }

    /// Verifica l'integrità della contabilità, inclusa la corrispondenza tra i
    /// riepiloghi congelati dei SAL emessi e il ricalcolo.
    #[must_use]
    pub fn verifica_integrita(
        &self,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
    ) -> Vec<Problema> {
        let mut problemi = Vec::new();

        let mut id_visti = BTreeSet::new();
        let mut numeri_visti = BTreeSet::new();
        for r in &self.libretto.registrazioni {
            let percorso = vec![r.riferimento()];
            if !id_visti.insert(r.id) {
                problemi.push(Problema::errore(
                    "contabilita.registrazione_id_duplicato",
                    format!("identificativo {} duplicato", r.id),
                    percorso.clone(),
                ));
            }
            if r.numero == 0 {
                problemi.push(Problema::errore(
                    "contabilita.registrazione_numero_nullo",
                    "il numero d'ordine deve essere almeno 1",
                    percorso.clone(),
                ));
            } else if !numeri_visti.insert(r.numero) {
                problemi.push(Problema::errore(
                    "contabilita.registrazione_numero_duplicato",
                    format!("numero d'ordine {} duplicato", r.numero),
                    percorso,
                ));
            }
        }

        let mut sal_visti = BTreeSet::new();
        let mut assegnazioni: BTreeMap<RegistrazioneId, u32> = BTreeMap::new();
        let mut data_precedente: Option<Date> = None;
        let ultimo = self.sal.len().saturating_sub(1);
        for (i, s) in self.sal.iter().enumerate() {
            let percorso = vec![rif_sal(s)];
            if !sal_visti.insert(s.id) {
                problemi.push(Problema::errore(
                    "contabilita.sal_id_duplicato",
                    format!("identificativo {} duplicato", s.id),
                    percorso.clone(),
                ));
            }
            let atteso = u32::try_from(i).map_or(u32::MAX, |n| n.saturating_add(1));
            if s.numero != atteso {
                problemi.push(Problema::errore(
                    "contabilita.sal_numerazione",
                    format!("numero {} non consecutivo: atteso {atteso}", s.numero),
                    percorso.clone(),
                ));
            }
            if let Some(d) = data_precedente.filter(|d| s.lavori_a_tutto_il <= *d) {
                problemi.push(Problema::errore(
                    "contabilita.sal_date",
                    format!(
                        "la data dei lavori {} non è successiva a quella del SAL precedente ({d})",
                        s.lavori_a_tutto_il
                    ),
                    percorso.clone(),
                ));
            }
            data_precedente = Some(s.lavori_a_tutto_il);
            match s.stato {
                StatoSal::Bozza if i != ultimo => problemi.push(Problema::errore(
                    "contabilita.sal_bozza_non_ultimo",
                    "solo l'ultimo SAL può essere in bozza",
                    percorso.clone(),
                )),
                StatoSal::Emesso if s.riepilogo_emesso.is_none() => {
                    problemi.push(Problema::errore(
                        "contabilita.sal_senza_riepilogo",
                        "SAL emesso privo di riepilogo congelato",
                        percorso.clone(),
                    ))
                }
                _ => {}
            }
            for reg_id in &s.registrazioni {
                if let Some(altro) = assegnazioni.insert(*reg_id, s.numero) {
                    problemi.push(Problema::errore(
                        "contabilita.registrazione_in_piu_sal",
                        format!(
                            "la registrazione {reg_id} è contabilizzata anche nel SAL n. {altro}"
                        ),
                        percorso.clone(),
                    ));
                }
                match self.libretto.registrazione(*reg_id) {
                    None => problemi.push(Problema::errore(
                        "contabilita.registrazione_inesistente",
                        format!("la registrazione {reg_id} non esiste nel libretto"),
                        percorso.clone(),
                    )),
                    Some(r) if r.data > s.lavori_a_tutto_il => problemi.push(Problema::errore(
                        "contabilita.registrazione_oltre_periodo",
                        format!(
                            "registrazione del {} oltre il periodo del SAL (lavori a tutto il {})",
                            r.data, s.lavori_a_tutto_il
                        ),
                        vec![rif_sal(s), r.riferimento()],
                    )),
                    Some(_) => {}
                }
            }
        }

        for s in &self.sal {
            let Some(congelato) = s.riepilogo_emesso.as_ref() else {
                continue;
            };
            if s.stato != StatoSal::Emesso {
                continue;
            }
            match self.calcola_sal(s.id, parametri, regole) {
                Ok(ricalcolato) if &ricalcolato == congelato => {}
                Ok(_) => problemi.push(Problema::errore(
                    "contabilita.sal_alterato",
                    "il ricalcolo non coincide con il riepilogo emesso: registrazioni o parametri \
                     contrattuali sono stati modificati dopo l'emissione",
                    vec![rif_sal(s)],
                )),
                Err(e) => problemi.push(Problema::errore(
                    "contabilita.sal_non_calcolabile",
                    e.to_string(),
                    vec![rif_sal(s)],
                )),
            }
        }

        // Penali: riferimenti dei SAL emessi e applicazione in un solo certificato.
        let mut applicate: BTreeMap<PenaleId, u32> = BTreeMap::new();
        for s in self.sal.iter().filter(|s| s.stato == StatoSal::Emesso) {
            for id in &s.penali {
                if !self.penali.iter().any(|p| p.id == *id) {
                    problemi.push(Problema::errore(
                        "contabilita.penale_inesistente",
                        format!(
                            "il SAL n. {} riferisce la penale inesistente {id}",
                            s.numero
                        ),
                        vec![rif_sal(s)],
                    ));
                }
                if let Some(altro) = applicate.insert(*id, s.numero) {
                    problemi.push(Problema::errore(
                        "contabilita.penale_in_piu_sal",
                        format!("la penale {id} è applicata anche nel SAL n. {altro}"),
                        vec![rif_sal(s)],
                    ));
                }
            }
        }

        // Certificati congelati: devono coincidere con il ricalcolo.
        let mut recuperata = Importo::ZERO;
        for s in &self.sal {
            let ricalcolato = self.certificato_di(s, recuperata, parametri, regole);
            let certificato = match (&s.stato, &s.certificato_emesso, ricalcolato) {
                (StatoSal::Emesso, Some(congelato), Ok(r)) => {
                    if &r != congelato {
                        problemi.push(Problema::errore(
                            "contabilita.certificato_alterato",
                            "il ricalcolo del certificato di pagamento non coincide con quello \
                             emesso: anticipazione, penali o IVA sono stati modificati dopo \
                             l'emissione",
                            vec![rif_sal(s)],
                        ));
                    }
                    congelato.clone()
                }
                (StatoSal::Emesso, Some(congelato), Err(_)) => congelato.clone(),
                (_, _, Ok(r)) => r,
                (_, _, Err(_)) => break,
            };
            if certificato.imponibile.is_negativo() {
                problemi.push(Problema::avviso(
                    "contabilita.certificato_negativo",
                    format!(
                        "il certificato di pagamento n. {} ha un imponibile negativo: recupero \
                         dell'anticipazione e penali superano l'importo del SAL",
                        s.numero
                    ),
                    vec![rif_sal(s)],
                ));
            }
            match recuperata.somma(certificato.recupero_anticipazione) {
                Ok(r) => recuperata = r,
                Err(_) => break,
            }
        }
        self.verifica_conto_finale(parametri, regole, &mut problemi);
        problemi
    }

    /// Integrità del conto finale: date, registrazioni e penali fissate, riepilogo.
    fn verifica_conto_finale(
        &self,
        parametri: &ParametriContabili,
        regole: &RegoleArrotondamento,
        problemi: &mut Vec<Problema>,
    ) {
        let Some(c) = &self.conto_finale else {
            return;
        };
        let percorso = || vec![Riferimento::ContoFinale];
        if let Some(s) = self.sal.iter().find(|s| s.stato == StatoSal::Bozza) {
            problemi.push(Problema::errore(
                "contabilita.conto_finale_sal_in_bozza",
                format!("il SAL n. {} è in bozza insieme al conto finale", s.numero),
                percorso(),
            ));
        }
        if let Some(s) = self
            .sal
            .iter()
            .find(|s| s.lavori_a_tutto_il > c.data_ultimazione)
        {
            problemi.push(Problema::errore(
                "contabilita.conto_finale_date",
                format!(
                    "l'ultimazione dei lavori ({}) precede i lavori del SAL n. {} ({})",
                    c.data_ultimazione, s.numero, s.lavori_a_tutto_il
                ),
                percorso(),
            ));
        }
        if c.data < c.data_ultimazione {
            problemi.push(Problema::errore(
                "contabilita.conto_finale_date",
                format!(
                    "la data del conto finale ({}) precede l'ultimazione dei lavori ({})",
                    c.data, c.data_ultimazione
                ),
                percorso(),
            ));
        }
        if c.stato != StatoSal::Emesso {
            if c.firma_esecutore.is_some() || c.collaudo.is_some() || !c.registrazioni.is_empty() {
                problemi.push(Problema::errore(
                    "contabilita.conto_finale_bozza_incoerente",
                    "conto finale in bozza con firma, collaudo o registrazioni fissate",
                    percorso(),
                ));
            }
        } else {
            for id in &c.registrazioni {
                if self.libretto.registrazione(*id).is_none() {
                    problemi.push(Problema::errore(
                        "contabilita.registrazione_inesistente",
                        format!("il conto finale riferisce la registrazione inesistente {id}"),
                        percorso(),
                    ));
                }
                if let Some(s) = self.sal_di(*id) {
                    problemi.push(Problema::errore(
                        "contabilita.registrazione_in_piu_sal",
                        format!(
                            "la registrazione {id} è nel conto finale e nel SAL n. {}",
                            s.numero
                        ),
                        percorso(),
                    ));
                }
            }
            for r in &self.libretto.registrazioni {
                if self.sal_di(r.id).is_none() && !c.registrazioni.contains(&r.id) {
                    problemi.push(Problema::errore(
                        "contabilita.registrazione_dopo_conto_finale",
                        "registrazione aggiunta dopo l'emissione del conto finale",
                        vec![Riferimento::ContoFinale, r.riferimento()],
                    ));
                }
            }
            for id in &c.penali {
                if !self.penali.iter().any(|p| p.id == *id) {
                    problemi.push(Problema::errore(
                        "contabilita.penale_inesistente",
                        format!("il conto finale riferisce la penale inesistente {id}"),
                        percorso(),
                    ));
                }
                if let Some(s) = self.sal_della_penale(*id) {
                    problemi.push(Problema::errore(
                        "contabilita.penale_in_piu_sal",
                        format!(
                            "la penale {id} è applicata nel conto finale e nel SAL n. {}",
                            s.numero
                        ),
                        percorso(),
                    ));
                }
            }
            match (
                &c.riepilogo_emesso,
                self.ricalcola_conto_finale(parametri, regole),
            ) {
                (None, _) => problemi.push(Problema::errore(
                    "contabilita.conto_finale_senza_riepilogo",
                    "conto finale emesso privo di riepilogo congelato",
                    percorso(),
                )),
                (Some(congelato), Ok(r)) if &r != congelato => problemi.push(Problema::errore(
                    "contabilita.conto_finale_alterato",
                    "il ricalcolo del conto finale non coincide con quello emesso: \
                     registrazioni, penali o parametri contrattuali sono stati modificati",
                    percorso(),
                )),
                (Some(_), Ok(_)) => {}
                (Some(_), Err(e)) => problemi.push(Problema::errore(
                    "contabilita.conto_finale_non_calcolabile",
                    e.to_string(),
                    percorso(),
                )),
            }
        }
        if let Ok(r) = self.calcola_conto_finale(parametri, regole)
            && r.imponibile.is_negativo()
        {
            problemi.push(Problema::avviso(
                "contabilita.rata_saldo_negativa",
                "la rata di saldo è negativa: anticipazione non recuperata e penali superano il \
                 credito residuo dell'esecutore",
                percorso(),
            ));
        }
    }
}
