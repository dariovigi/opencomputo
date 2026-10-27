//! Prezzo applicato e calcolo di una partita (prezzo × righi di misura).
//!
//! Il [`PrezzoApplicato`] è la **fotografia** dei dati di una voce di
//! prezzario nel momento in cui viene inserita in un computo o in una
//! registrazione contabile. Conserva un riferimento all'origine solo a scopo
//! di tracciabilità: aggiornare il prezzario non modifica la fotografia.
//! Il riallineamento è un'operazione esplicita che restituisce l'elenco
//! delle differenze applicate.

use serde::{Deserialize, Serialize};

use crate::errore::{ErroreCalcolo, ErroreLocalizzato};
use crate::id::{PrezzarioId, VocePrezzarioId};
use crate::misura::{QuantitaRiferite, RigoMisura, SenzaRiferimenti, quantita_totale_con};
use crate::numeri::{Importo, Percentuale, Prezzo, Quantita, RegoleArrotondamento};
use crate::prezzario::{NaturaVoce, Prezzario, VocePrezzario};
use crate::riferimento::Riferimento;

/// Riferimento a una voce di un prezzario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RiferimentoPrezzario {
    /// Prezzario.
    pub prezzario: PrezzarioId,
    /// Voce.
    pub voce: VocePrezzarioId,
}

/// Fotografia dei dati di prezzo applicati a una voce di computo o contabilità.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrezzoApplicato {
    /// Voce di origine (solo tracciabilità).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origine: Option<RiferimentoPrezzario>,
    /// Descrizione della fonte (es. «Prezzario regionale (2025)»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonte: Option<String>,
    /// Codice (tariffa).
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
    /// Incidenza della manodopera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incidenza_manodopera: Option<Percentuale>,
    /// Incidenza della sicurezza compresa nel prezzo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incidenza_sicurezza: Option<Percentuale>,
    /// Nuovo prezzo già netto: escluso dal ribasso d'asta nei SAL.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub escluso_dal_ribasso: bool,
}

/// Campo di un prezzo applicato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampoPrezzo {
    /// Codice.
    Codice,
    /// Descrizione breve.
    DescrizioneBreve,
    /// Descrizione estesa.
    DescrizioneEstesa,
    /// Unità di misura.
    UnitaMisura,
    /// Prezzo unitario.
    Prezzo,
    /// Natura.
    Natura,
    /// Incidenza manodopera.
    IncidenzaManodopera,
    /// Incidenza sicurezza.
    IncidenzaSicurezza,
    /// Esclusione dal ribasso d'asta.
    EsclusoDalRibasso,
}

/// Differenza tra prezzo applicato e voce di prezzario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DifferenzaPrezzo {
    /// Campo.
    pub campo: CampoPrezzo,
    /// Valore applicato.
    pub applicato: String,
    /// Valore nel prezzario.
    pub prezzario: String,
}

fn testo_opzionale(p: Option<Percentuale>) -> String {
    p.map(|v| v.to_string()).unwrap_or_default()
}

impl PrezzoApplicato {
    /// Fotografia di una voce di prezzario.
    #[must_use]
    pub fn da_voce(prezzario: &Prezzario, voce: &VocePrezzario) -> Self {
        Self {
            origine: Some(RiferimentoPrezzario {
                prezzario: prezzario.id,
                voce: voce.id,
            }),
            fonte: Some(prezzario.fonte()),
            codice: voce.codice.clone(),
            descrizione_breve: voce.descrizione_breve.clone(),
            descrizione_estesa: voce.descrizione_estesa.clone(),
            unita_misura: voce.unita_misura.clone(),
            prezzo: voce.prezzo,
            natura: voce.natura,
            incidenza_manodopera: voce.incidenza_manodopera,
            incidenza_sicurezza: voce.incidenza_sicurezza,
            escluso_dal_ribasso: voce
                .nuovo_prezzo
                .as_ref()
                .is_some_and(|n| !n.soggetto_a_ribasso),
        }
    }

    /// Prezzo inserito manualmente (es. nuovo prezzo non presente in prezzario).
    #[must_use]
    pub fn manuale(
        codice: impl Into<String>,
        descrizione: impl Into<String>,
        unita_misura: impl Into<String>,
        prezzo: Prezzo,
    ) -> Self {
        let descrizione = descrizione.into();
        Self {
            origine: None,
            fonte: None,
            codice: codice.into(),
            descrizione_breve: descrizione.clone(),
            descrizione_estesa: descrizione,
            unita_misura: unita_misura.into(),
            prezzo,
            natura: NaturaVoce::Lavoro,
            incidenza_manodopera: None,
            incidenza_sicurezza: None,
            escluso_dal_ribasso: false,
        }
    }

    /// Differenze rispetto alla voce di prezzario indicata.
    #[must_use]
    pub fn confronta(&self, voce: &VocePrezzario) -> Vec<DifferenzaPrezzo> {
        let mut d = Vec::new();
        let mut confronta = |campo, a: String, b: String| {
            if a != b {
                d.push(DifferenzaPrezzo {
                    campo,
                    applicato: a,
                    prezzario: b,
                });
            }
        };
        confronta(
            CampoPrezzo::Codice,
            self.codice.clone(),
            voce.codice.clone(),
        );
        confronta(
            CampoPrezzo::DescrizioneBreve,
            self.descrizione_breve.clone(),
            voce.descrizione_breve.clone(),
        );
        confronta(
            CampoPrezzo::DescrizioneEstesa,
            self.descrizione_estesa.clone(),
            voce.descrizione_estesa.clone(),
        );
        confronta(
            CampoPrezzo::UnitaMisura,
            self.unita_misura.clone(),
            voce.unita_misura.clone(),
        );
        // Confronto numerico: 10 e 10.00 sono lo stesso prezzo.
        if self.prezzo != voce.prezzo {
            confronta(
                CampoPrezzo::Prezzo,
                self.prezzo.to_string(),
                voce.prezzo.to_string(),
            );
        }
        if self.natura != voce.natura {
            confronta(
                CampoPrezzo::Natura,
                format!("{:?}", self.natura),
                format!("{:?}", voce.natura),
            );
        }
        if self.incidenza_manodopera != voce.incidenza_manodopera {
            confronta(
                CampoPrezzo::IncidenzaManodopera,
                testo_opzionale(self.incidenza_manodopera),
                testo_opzionale(voce.incidenza_manodopera),
            );
        }
        if self.incidenza_sicurezza != voce.incidenza_sicurezza {
            confronta(
                CampoPrezzo::IncidenzaSicurezza,
                testo_opzionale(self.incidenza_sicurezza),
                testo_opzionale(voce.incidenza_sicurezza),
            );
        }
        let escluso = voce
            .nuovo_prezzo
            .as_ref()
            .is_some_and(|n| !n.soggetto_a_ribasso);
        if self.escluso_dal_ribasso != escluso {
            confronta(
                CampoPrezzo::EsclusoDalRibasso,
                self.escluso_dal_ribasso.to_string(),
                escluso.to_string(),
            );
        }
        d
    }

    /// Riallinea esplicitamente la fotografia alla voce di prezzario e
    /// restituisce le differenze applicate.
    pub fn riallinea(
        &mut self,
        prezzario: &Prezzario,
        voce: &VocePrezzario,
    ) -> Vec<DifferenzaPrezzo> {
        let differenze = self.confronta(voce);
        *self = Self::da_voce(prezzario, voce);
        differenze
    }
}

/// Risultato del calcolo di una partita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalcoloPartita {
    /// Quantità totale.
    pub quantita: Quantita,
    /// Importo = quantità × prezzo, arrotondato.
    pub importo: Importo,
    /// Quota di manodopera (se è nota l'incidenza).
    pub importo_manodopera: Option<Importo>,
    /// Quota di sicurezza compresa nel prezzo (se è nota l'incidenza).
    pub importo_sicurezza: Option<Importo>,
}

/// Calcola una partita: quantità dai righi, importo, quote di manodopera e sicurezza.
///
/// # Errors
/// Regole non valide o errore di calcolo localizzato sul rigo.
pub fn calcola_partita(
    prezzo: &PrezzoApplicato,
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
) -> Result<CalcoloPartita, ErroreLocalizzato> {
    calcola_partita_con(prezzo, righi, regole, &SenzaRiferimenti)
}

/// Come [`calcola_partita`], con le quantità delle voci citate dai «vedi voce».
///
/// # Errors
/// Regole non valide o errore di calcolo localizzato sul rigo.
pub fn calcola_partita_con(
    prezzo: &PrezzoApplicato,
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
    riferite: &dyn QuantitaRiferite,
) -> Result<CalcoloPartita, ErroreLocalizzato> {
    regole
        .valida()
        .map_err(|e| ErroreLocalizzato::new(Riferimento::Regole, e))?;
    let quantita = quantita_totale_con(righi, regole, riferite)?;
    let quota =
        |p: Option<Percentuale>, importo: Importo| -> Result<Option<Importo>, ErroreCalcolo> {
            p.map(|p| p.quota_di(importo, regole)).transpose()
        };
    let calcolo = || -> Result<CalcoloPartita, ErroreCalcolo> {
        let importo = prezzo.prezzo.per_quantita(quantita, regole)?;
        Ok(CalcoloPartita {
            quantita,
            importo,
            importo_manodopera: quota(prezzo.incidenza_manodopera, importo)?,
            importo_sicurezza: quota(prezzo.incidenza_sicurezza, importo)?,
        })
    };
    calcolo().map_err(|e| ErroreLocalizzato {
        percorso: Vec::new(),
        errore: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::RigoMisuraId;
    use rust_decimal_macros::dec;

    fn prezzario() -> Prezzario {
        let mut p = Prezzario::new(PrezzarioId::da_u128(1), "Elenco");
        let mut v = VocePrezzario::new(
            VocePrezzarioId::da_u128(2),
            "E.01",
            "Muratura",
            "m³",
            Prezzo::new(dec!(250.00)),
        );
        v.incidenza_manodopera = Some(Percentuale::new(dec!(40)));
        p.aggiungi_voce(v).unwrap();
        p
    }

    #[test]
    fn fotografia_indipendente_dal_prezzario() {
        let mut p = prezzario();
        let applicato = PrezzoApplicato::da_voce(&p, &p.voci[0]);
        p.voci[0].prezzo = Prezzo::new(dec!(275.00));
        assert_eq!(applicato.prezzo, Prezzo::new(dec!(250.00)));
        let diff = applicato.confronta(&p.voci[0]);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].campo, CampoPrezzo::Prezzo);
    }

    #[test]
    fn riallineamento_esplicito() {
        let mut p = prezzario();
        let mut applicato = PrezzoApplicato::da_voce(&p, &p.voci[0]);
        p.voci[0].prezzo = Prezzo::new(dec!(275.00));
        p.voci[0].descrizione_breve = "Muratura in blocchi".into();
        let diff = applicato.riallinea(&p, &p.voci[0]);
        assert_eq!(diff.len(), 2);
        assert_eq!(applicato.prezzo, Prezzo::new(dec!(275.00)));
        assert!(applicato.confronta(&p.voci[0]).is_empty());
    }

    #[test]
    fn prezzi_numericamente_uguali_non_differiscono() {
        let p = prezzario();
        let mut applicato = PrezzoApplicato::da_voce(&p, &p.voci[0]);
        applicato.prezzo = Prezzo::new(dec!(250));
        assert!(applicato.confronta(&p.voci[0]).is_empty());
    }

    #[test]
    fn calcolo_partita_con_manodopera() {
        let p = prezzario();
        let applicato = PrezzoApplicato::da_voce(&p, &p.voci[0]);
        let righi = vec![
            RigoMisura::misura(RigoMisuraId::da_u128(1), "")
                .con_lunghezza(dec!(4.00))
                .con_larghezza(dec!(0.30))
                .con_altezza(dec!(2.75)),
        ];
        let c = calcola_partita(&applicato, &righi, &RegoleArrotondamento::default()).unwrap();
        // 4 × 0,30 × 2,75 = 3,30 m³; × 250 = 825,00; manodopera 40 % = 330,00
        assert_eq!(c.quantita, Quantita::new(dec!(3.30)));
        assert_eq!(c.importo, Importo::new(dec!(825.00)));
        assert_eq!(c.importo_manodopera, Some(Importo::new(dec!(330.00))));
        assert_eq!(c.importo_sicurezza, None);
    }
}
