//! Dalle scelte dell'utente ai comandi (ADR 0025).
//!
//! Per ogni lavorazione della [`Proposta`] l'utente spunta una o più opzioni e può
//! correggere o completare le misure. Le misure valgono per tutte le opzioni spuntate
//! della lavorazione. Qui si controllano le scelte e si costruiscono i comandi:
//! `copia_da_prezzario` per le voci che mancano nell'elenco prezzi, poi un
//! `aggiungi_voce_computo` per ogni opzione spuntata, nell'ordine delle lavorazioni e delle
//! opzioni.
//!
//! Un rigo con valori mancanti entra **vuoto** (nessun fattore, quantità zero), con nella
//! descrizione i valori noti e quelli da completare: mai una quantità parziale. Lo stesso
//! per le voci con un'unità senza forme di misura.

use std::collections::{BTreeMap, BTreeSet};

use opencomputo_application::{Comando, NuovoRigo};
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::forme::{Parametro, rigo};
use crate::proposta::{
    Ipotesi, LavorazioneProposta, Opzione, PARAMETRI, Proposta, ProvenienzaValore, RigaProposta,
    scegli, tronca,
};
use crate::unita::riconosci;

/// Lunghezza massima delle note scritte nella voce di computo, in caratteri.
const NOTA_MASSIMA: usize = 1000;

/// Scelta dell'utente per una lavorazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scelta {
    /// Lavorazione (`L01`…).
    pub lavorazione: String,
    /// Codici delle opzioni spuntate.
    pub codici: Vec<String>,
    /// Misure corrette: per ogni rigo della proposta, nome del parametro → valore come
    /// testo (vuoto: da completare). Assenti: quelle della proposta.
    #[serde(default)]
    pub misure: Option<Vec<BTreeMap<String, String>>>,
}

/// Scelta non applicabile.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ErroreScelta {
    /// Nessuna opzione spuntata.
    #[error("nessuna voce scelta")]
    NessunaScelta,
    /// Lavorazione non della proposta.
    #[error("la lavorazione «{lavorazione}» non è nella proposta")]
    Lavorazione {
        /// Lavorazione.
        lavorazione: String,
    },
    /// Opzione non della lavorazione.
    #[error("la voce «{codice}» non è tra le opzioni della lavorazione {lavorazione}")]
    Opzione {
        /// Lavorazione.
        lavorazione: String,
        /// Codice.
        codice: String,
    },
    /// Misure non valide.
    #[error("lavorazione {lavorazione}: {motivo}")]
    Misure {
        /// Lavorazione.
        lavorazione: String,
        /// Motivo.
        motivo: String,
    },
}

/// Voce che i comandi aggiungono.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoceScelta {
    /// Lavorazione.
    pub lavorazione: String,
    /// Codice.
    pub codice: String,
    /// `true` se qualche rigo entra vuoto (misure da completare nel computo).
    pub da_completare: bool,
}

/// Righi di misura con le correzioni dell'utente.
///
/// # Errors
/// Numero di righi diverso, parametro sconosciuto, valore non numerico, non positivo o
/// N non intero.
pub fn misure(
    l: &LavorazioneProposta,
    correzioni: Option<&[BTreeMap<String, String>]>,
) -> Result<Vec<RigaProposta>, ErroreScelta> {
    let errore = |motivo: String| ErroreScelta::Misure {
        lavorazione: l.id.clone(),
        motivo,
    };
    let Some(correzioni) = correzioni else {
        return Ok(l.righe.clone());
    };
    if correzioni.len() != l.righe.len() {
        return Err(errore(format!(
            "{} righi di misure invece di {}",
            correzioni.len(),
            l.righe.len()
        )));
    }
    let mut righe = l.righe.clone();
    for (n, (r, c)) in righe.iter_mut().zip(correzioni).enumerate() {
        for (nome, testo) in c {
            let p = scegli(nome, &PARAMETRI)
                .and_then(|p| r.parametri.iter_mut().find(|x| x.nome == p))
                .ok_or_else(|| {
                    errore(format!("rigo {}: parametro «{nome}» non previsto", n + 1))
                })?;
            let t = testo.trim().replace(',', ".");
            if t.is_empty() {
                p.valore = None;
                p.provenienza = ProvenienzaValore::Mancante;
                continue;
            }
            let v = Decimal::from_str_exact(&t)
                .ok()
                .filter(|v| *v > Decimal::ZERO)
                .ok_or_else(|| {
                    errore(format!(
                        "rigo {}: {nome} = «{testo}» non è un numero maggiore di zero",
                        n + 1
                    ))
                })?;
            if p.nome == Parametro::N && v.fract() != Decimal::ZERO {
                return Err(errore(format!("rigo {}: N deve essere intero", n + 1)));
            }
            if p.valore != Some(v) {
                p.valore = Some(v);
                p.provenienza = ProvenienzaValore::Utente {
                    testo: "indicato nella proposta".into(),
                };
            }
        }
    }
    Ok(righe)
}

/// Testo di un valore per le descrizioni dei righi da completare.
fn testo_valore(r: &RigaProposta) -> String {
    r.parametri
        .iter()
        .map(|p| match p.valore {
            Some(v) => format!("{:?} = {} {}", p.nome, v.normalize(), unita(p))
                .trim_end()
                .to_owned(),
            None => format!("{:?} = ?", p.nome),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn unita(p: &crate::proposta::ParametroProposto) -> &'static str {
    match p.unita {
        crate::valori::UnitaValore::Mm => "mm",
        crate::valori::UnitaValore::Cm => "cm",
        crate::valori::UnitaValore::M => "m",
        crate::valori::UnitaValore::M2 => "m²",
        crate::valori::UnitaValore::M3 => "m³",
        crate::valori::UnitaValore::Kg => "kg",
        crate::valori::UnitaValore::KgM => "kg/m",
        crate::valori::UnitaValore::N => "",
    }
}

/// Righi per un'opzione: completi dove possibile, vuoti con la descrizione altrimenti.
fn righi(righe: &[RigaProposta], o: &Opzione, descrizione: &str) -> (Vec<NuovoRigo>, bool) {
    let fattore = riconosci(&o.unita_misura).fattore;
    if righe.is_empty() || o.misura_manuale {
        let testo = if righe.is_empty() {
            format!("{descrizione} — misure da completare")
        } else {
            let misure: Vec<String> = righe
                .iter()
                .map(|r| format!("{} ({})", r.descrizione, testo_valore(r)))
                .collect();
            format!("misure da completare: {}", misure.join("; "))
        };
        return (
            vec![NuovoRigo {
                descrizione: tronca(&testo, 300),
                ..NuovoRigo::default()
            }],
            true,
        );
    }
    let mut da_completare = false;
    let r = righe
        .iter()
        .map(|r| {
            if r.da_completare() {
                da_completare = true;
                NuovoRigo {
                    descrizione: tronca(
                        &format!("{} — da completare: {}", r.descrizione, testo_valore(r)),
                        300,
                    ),
                    deduzione: r.deduzione,
                    ..NuovoRigo::default()
                }
            } else {
                let valori: BTreeMap<Parametro, Decimal> = r
                    .parametri
                    .iter()
                    .filter_map(|p| p.valore.map(|v| (p.nome, p.unita.in_base(v))))
                    .collect();
                rigo(r.forma, &valori, fattore, &r.descrizione, r.deduzione)
            }
        })
        .collect();
    (r, da_completare)
}

fn nota(
    l: &LavorazioneProposta,
    righe: &[RigaProposta],
    o: &Opzione,
    ipotesi: &[Ipotesi],
    da_completare: bool,
) -> String {
    let mut n = format!("Assistente AI, {}: {}.", l.id, l.descrizione);
    if !o.motivazione.is_empty() {
        n.push_str(&format!(" Scelta: {}.", o.motivazione));
    }
    let usate: BTreeSet<&String> = righe
        .iter()
        .flat_map(|r| &r.parametri)
        .filter_map(|p| match &p.provenienza {
            ProvenienzaValore::Ipotesi { id } => Some(id),
            _ => None,
        })
        .collect();
    if !usate.is_empty() {
        let testi: Vec<String> = usate
            .iter()
            .map(|id| match ipotesi.iter().find(|i| &&i.id == id) {
                Some(i) => format!("{id} ({})", i.testo),
                None => (*id).clone(),
            })
            .collect();
        n.push_str(&format!(" Ipotesi: {}.", testi.join("; ")));
    }
    if da_completare {
        n.push_str(" Misure da completare.");
    }
    tronca(&n, NOTA_MASSIMA)
}

/// Comandi per le scelte, e le voci che aggiungono nell'ordine dei comandi.
///
/// # Errors
/// Nessuna voce scelta, lavorazione od opzione non della proposta, misure non valide.
pub fn comandi(
    proposta: &Proposta,
    scelte: &[Scelta],
    progetto: &Progetto,
) -> Result<(Vec<Comando>, Vec<VoceScelta>), ErroreScelta> {
    let mut aggiunte = Vec::new();
    let mut voci = Vec::new();
    let mut da_copiare: Vec<String> = Vec::new();
    // Nell'ordine delle lavorazioni della proposta, qualunque sia l'ordine delle scelte.
    for l in &proposta.lavorazioni {
        let scelte_l: Vec<&Scelta> = scelte
            .iter()
            .filter(|s| s.lavorazione.trim().eq_ignore_ascii_case(&l.id))
            .collect();
        let Some(scelta) = scelte_l.last() else {
            continue;
        };
        let righe = misure(l, scelta.misure.as_deref())?;
        let spuntati: BTreeSet<String> =
            scelta.codici.iter().map(|c| c.trim().to_owned()).collect();
        if let Some(c) = spuntati
            .iter()
            .find(|c| !l.opzioni.iter().any(|o| &o.codice == *c))
        {
            return Err(ErroreScelta::Opzione {
                lavorazione: l.id.clone(),
                codice: c.clone(),
            });
        }
        for o in l.opzioni.iter().filter(|o| spuntati.contains(&o.codice)) {
            if progetto.elenco_prezzi.voce_per_codice(&o.codice).is_none()
                && !da_copiare.contains(&o.codice)
            {
                da_copiare.push(o.codice.clone());
            }
            let (r, da_completare) = righi(&righe, o, &l.descrizione);
            aggiunte.push(Comando::AggiungiVoceComputo {
                computo: proposta.computo.clone(),
                codice: o.codice.clone(),
                posizione: None,
                righi: r,
                categorie: None,
                wbs: None,
                note: Some(nota(l, &righe, o, &proposta.ipotesi, da_completare)),
            });
            voci.push(VoceScelta {
                lavorazione: l.id.clone(),
                codice: o.codice.clone(),
                da_completare,
            });
        }
    }
    if let Some(s) = scelte
        .iter()
        .find(|s| proposta.lavorazione(&s.lavorazione).is_none())
    {
        return Err(ErroreScelta::Lavorazione {
            lavorazione: s.lavorazione.clone(),
        });
    }
    if voci.is_empty() {
        return Err(ErroreScelta::NessunaScelta);
    }
    let mut comandi = Vec::new();
    if !da_copiare.is_empty() {
        comandi.push(Comando::CopiaDaPrezzario {
            prezzario: proposta.nome_prezzario.clone(),
            codici: da_copiare,
        });
    }
    comandi.extend(aggiunte);
    Ok((comandi, voci))
}

/// Tutte le opzioni di tutte le lavorazioni, con le misure delle scelte dove indicate:
/// serve all'anteprima dei valori di ogni opzione.
#[must_use]
pub fn tutte(proposta: &Proposta, scelte: &[Scelta]) -> Vec<Scelta> {
    proposta
        .lavorazioni
        .iter()
        .map(|l| Scelta {
            lavorazione: l.id.clone(),
            codici: l.opzioni.iter().map(|o| o.codice.clone()).collect(),
            misure: scelte
                .iter()
                .rev()
                .find(|s| s.lavorazione.trim().eq_ignore_ascii_case(&l.id))
                .and_then(|s| s.misure.clone()),
        })
        .collect()
}
