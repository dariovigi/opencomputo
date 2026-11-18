//! Viste di sola lettura del progetto, pensate per l'interfaccia grafica e per gli agenti
//! che devono leggere il dettaglio dei dati (non solo i totali).
//!
//! Le viste espongono codici e numeri d'ordine, gli stessi riferimenti usati dai
//! [`Comando`](crate::Comando), e i valori calcolati dal core. I fattori di misura sono
//! riportati come testo con la virgola decimale (`"1,20"`) o come espressione così come
//! scritta: rimandati in un comando, producono lo stesso fattore.

use std::collections::HashMap;

use jiff::civil::Date;
use opencomputo_core::aggiornamento::{
    ConfrontoRisorsa, ConfrontoVoce, SostituzionePrefisso, confronta_listino,
};
use opencomputo_core::analisi::{
    BaseUtile, OrigineRigaAnalisi, ParametriAnalisi, RisultatoAnalisi, TipoRisorsa,
};
use opencomputo_core::classificazione::{
    AssegnazioneCategorie, Categorie, Classe, LivelloCategoria,
};
use opencomputo_core::computo::{Computo, QuantitaComputo, TipoComputo};
use opencomputo_core::contabilita::StatoSal;
use opencomputo_core::contabilita::sal::{ParametriContabili, RiepilogoSal};
use opencomputo_core::contabilita::{CertificatoPagamento, Collaudo, RiepilogoContoFinale};
use opencomputo_core::fabbisogno::Fabbisogno;
use opencomputo_core::id::AnalisiId;
use opencomputo_core::id::{ComputoId, VoceComputoId};
use opencomputo_core::misura::{
    Fattore, QuantitaRiferite, RigoMisura, SegnoRigo, SenzaRiferimenti, TipoRigo, VediVoce,
};
use opencomputo_core::numeri::{Importo, Percentuale, Prezzo, Quantita, RegoleArrotondamento};
use opencomputo_core::partita::PrezzoApplicato;
use opencomputo_core::prezzario::{
    CriterioNuovoPrezzo, NaturaVoce, NuovoPrezzo, Prezzario, VocePrezzario,
};
use opencomputo_core::progetto::{DatiContratto, MetadatiProgetto, Progetto};
use opencomputo_io::documento::{Allineamento, Documento};
use serde::Serialize;

use crate::casi_uso::trova_computo;
use crate::errore::ErroreApplicazione;

/// Classe (categoria o nodo) per codice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaClasse {
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
}

/// Nodo WBS con il codice completo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaNodoWbs {
    /// Codice completo (es. `01.02`).
    pub codice_completo: String,
    /// Codice relativo al padre.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Profondità (0 per i nodi radice).
    pub livello: usize,
}

/// Capitolo dell'elenco prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaCapitolo {
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Codice del capitolo padre.
    pub padre: Option<String>,
}

/// Computo nell'elenco dei computi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoceElencoComputi {
    /// Identificativo (lo stesso dei riferimenti nei problemi di validazione).
    pub id: ComputoId,
    /// Nome.
    pub nome: String,
    /// Tipo.
    pub tipo: TipoComputo,
    /// Numero di voci.
    pub voci: usize,
}

/// Dati generali del progetto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaProgetto {
    /// Metadati.
    pub metadati: MetadatiProgetto,
    /// Contratto.
    pub contratto: Option<DatiContratto>,
    /// Regole di arrotondamento.
    pub regole: RegoleArrotondamento,
    /// Super categorie.
    pub super_categorie: Vec<VistaClasse>,
    /// Categorie.
    pub categorie: Vec<VistaClasse>,
    /// Sotto categorie.
    pub sotto_categorie: Vec<VistaClasse>,
    /// Nodi WBS in ordine gerarchico.
    pub wbs: Vec<VistaNodoWbs>,
    /// Capitoli dell'elenco prezzi.
    pub capitoli: Vec<VistaCapitolo>,
    /// Computi.
    pub computi: Vec<VoceElencoComputi>,
}

fn classi(c: &[Classe]) -> Vec<VistaClasse> {
    c.iter()
        .map(|x| VistaClasse {
            codice: x.codice.clone(),
            descrizione: x.descrizione.clone(),
        })
        .collect()
}

/// Dati generali del progetto.
#[must_use]
pub fn progetto(p: &Progetto) -> VistaProgetto {
    let mut wbs = Vec::new();
    let mut pila: Vec<_> = p
        .wbs
        .figli(None)
        .into_iter()
        .rev()
        .map(|n| (n, 0usize))
        .collect();
    while let Some((n, livello)) = pila.pop() {
        wbs.push(VistaNodoWbs {
            codice_completo: p
                .wbs
                .codice_completo(n.id)
                .unwrap_or_else(|_| n.codice.clone()),
            codice: n.codice.clone(),
            descrizione: n.descrizione.clone(),
            livello,
        });
        pila.extend(
            p.wbs
                .figli(Some(n.id))
                .into_iter()
                .rev()
                .map(|f| (f, livello + 1)),
        );
    }
    let capitoli = &p.elenco_prezzi.capitoli;
    VistaProgetto {
        metadati: p.metadati.clone(),
        contratto: p.contratto.clone(),
        regole: p.regole.clone(),
        super_categorie: classi(&p.categorie.super_categorie),
        categorie: classi(&p.categorie.categorie),
        sotto_categorie: classi(&p.categorie.sotto_categorie),
        wbs,
        capitoli: capitoli
            .iter()
            .map(|c| VistaCapitolo {
                codice: c.codice.clone(),
                descrizione: c.descrizione.clone(),
                padre: c
                    .padre
                    .and_then(|id| capitoli.iter().find(|x| x.id == id))
                    .map(|x| x.codice.clone()),
            })
            .collect(),
        computi: p
            .computi
            .iter()
            .map(|c| VoceElencoComputi {
                id: c.id,
                nome: c.nome.clone(),
                tipo: c.tipo,
                voci: c.voci.len(),
            })
            .collect(),
    }
}

/// Voce di un elenco prezzi, senza descrizione estesa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoceElenco {
    /// Codice.
    pub codice: String,
    /// Descrizione breve.
    pub descrizione_breve: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo.
    pub prezzo: Prezzo,
    /// Natura.
    pub natura: NaturaVoce,
    /// Incidenza della manodopera.
    pub incidenza_manodopera: Option<Percentuale>,
    /// Incidenza della sicurezza.
    pub incidenza_sicurezza: Option<Percentuale>,
    /// Codice del capitolo.
    pub capitolo: Option<String>,
    /// `true` se il prezzo deriva da un'analisi.
    pub analizzata: bool,
    /// Sigla del nuovo prezzo (`NP.1`), se lo è.
    pub nuovo_prezzo: Option<String>,
}

/// Elenco prezzi, eventualmente filtrato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaElenco {
    /// Nome del prezzario.
    pub nome: String,
    /// Voci in totale nel prezzario.
    pub totale: usize,
    /// Voci corrispondenti al filtro, nell'ordine del prezzario.
    pub voci: Vec<VoceElenco>,
    /// Capitoli del prezzario (per il pannello dei filtri).
    pub capitoli: Vec<VistaCapitolo>,
}

fn codice_capitolo(pr: &Prezzario, v: &VocePrezzario) -> Option<String> {
    v.capitolo
        .and_then(|id| pr.capitoli.iter().find(|c| c.id == id))
        .map(|c| c.codice.clone())
}

/// Capitoli di un prezzario con il codice del padre.
fn vista_capitoli(pr: &Prezzario) -> Vec<VistaCapitolo> {
    let codici: HashMap<_, _> = pr
        .capitoli
        .iter()
        .map(|c| (c.id, c.codice.as_str()))
        .collect();
    pr.capitoli
        .iter()
        .map(|c| VistaCapitolo {
            codice: c.codice.clone(),
            descrizione: c.descrizione.clone(),
            padre: c
                .padre
                .and_then(|id| codici.get(&id))
                .map(|x| (*x).to_owned()),
        })
        .collect()
}

/// Elenco prezzi filtrato con la ricerca testuale del core (tutte le parole su codice e
/// descrizioni); senza testo, tutte le voci.
#[must_use]
pub fn elenco(pr: &Prezzario, testo: Option<&str>) -> VistaElenco {
    // Codici dei capitoli per identificativo: i prezzari regionali hanno migliaia di
    // capitoli e decine di migliaia di voci.
    let capitoli: HashMap<_, _> = pr
        .capitoli
        .iter()
        .map(|c| (c.id, c.codice.as_str()))
        .collect();
    let riga = |v: &VocePrezzario| VoceElenco {
        codice: v.codice.clone(),
        descrizione_breve: v.descrizione_breve.clone(),
        unita_misura: v.unita_misura.clone(),
        prezzo: v.prezzo,
        natura: v.natura,
        incidenza_manodopera: v.incidenza_manodopera,
        incidenza_sicurezza: v.incidenza_sicurezza,
        capitolo: v
            .capitolo
            .and_then(|id| capitoli.get(&id))
            .map(|c| (*c).to_owned()),
        analizzata: v.analisi.is_some(),
        nuovo_prezzo: v.nuovo_prezzo.as_ref().map(NuovoPrezzo::sigla),
    };
    let voci = match testo.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => pr.cerca(t).map(riga).collect(),
        None => pr.voci.iter().map(riga).collect(),
    };
    VistaElenco {
        nome: pr.nome.clone(),
        totale: pr.voci.len(),
        voci,
        capitoli: vista_capitoli(pr),
    }
}

/// Voce di elenco prezzi completa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DettaglioVoce {
    /// Dati della voce.
    #[serde(flatten)]
    pub voce: VoceElenco,
    /// Descrizione estesa.
    pub descrizione_estesa: String,
    /// Origine dei dati (file e formato), se importata.
    pub fonte: Option<String>,
    /// Analisi del prezzo, calcolata con i prezzi correnti.
    pub analisi: Option<VistaAnalisi>,
    /// Dati del nuovo prezzo.
    pub nuovo_prezzo: Option<NuovoPrezzo>,
}

/// Dettaglio di una voce per codice.
///
/// # Errors
/// Codice inesistente.
pub fn voce(
    pr: &Prezzario,
    codice: &str,
    regole: &RegoleArrotondamento,
) -> Result<DettaglioVoce, ErroreApplicazione> {
    let v = pr
        .voce_per_codice(codice)
        .ok_or_else(|| ErroreApplicazione::Richiesta {
            messaggio: format!("voce «{codice}» inesistente in «{}»", pr.nome),
        })?;
    Ok(DettaglioVoce {
        voce: VoceElenco {
            codice: v.codice.clone(),
            descrizione_breve: v.descrizione_breve.clone(),
            unita_misura: v.unita_misura.clone(),
            prezzo: v.prezzo,
            natura: v.natura,
            incidenza_manodopera: v.incidenza_manodopera,
            incidenza_sicurezza: v.incidenza_sicurezza,
            capitolo: codice_capitolo(pr, v),
            analizzata: v.analisi.is_some(),
            nuovo_prezzo: v.nuovo_prezzo.as_ref().map(NuovoPrezzo::sigla),
        },
        analisi: v.analisi.and_then(|a| vista_analisi(pr, v, a, regole)),
        nuovo_prezzo: v.nuovo_prezzo.clone(),
        descrizione_estesa: v.descrizione_estesa.clone(),
        fonte: v.provenienza.as_ref().map(|p| {
            let file = p.file.as_deref().unwrap_or("");
            format!("{:?} {file}", p.formato)
                .to_lowercase()
                .trim()
                .to_owned()
        }),
    })
}

/// Fattore di misura in forma di testo modificabile.
#[must_use]
pub fn testo_fattore(f: &Fattore) -> String {
    match f {
        Fattore::Valore(d) => d.to_string().replace('.', ","),
        Fattore::Espressione(e) => e.clone(),
    }
}

/// Rigo di misura con la quantità calcolata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaRigo {
    /// Numero del rigo, da 1.
    pub numero: usize,
    /// Rigo di solo commento.
    pub commento: bool,
    /// Descrizione.
    pub descrizione: String,
    /// Parti uguali.
    pub parti_uguali: Option<String>,
    /// Lunghezza.
    pub lunghezza: Option<String>,
    /// Larghezza.
    pub larghezza: Option<String>,
    /// Altezza o peso.
    pub altezza_peso: Option<String>,
    /// Formula.
    pub formula: Option<String>,
    /// Rigo in deduzione.
    pub deduzione: bool,
    /// Quantità del rigo (assente per i commenti o se non calcolabile).
    pub quantita: Option<Quantita>,
    /// Errore di calcolo.
    pub errore: Option<String>,
    /// «Vedi voce».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vedi_voce: Option<VistaVediVoce>,
}

/// «Vedi voce» di un rigo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaVediVoce {
    /// Testo da mostrare: «Vedi voce n. 5» o il riferimento statico.
    pub testo: String,
    /// `true` se la quantità è copiata (non segue più la voce).
    pub statica: bool,
    /// Voce collegata (identificativo), se il riferimento è dinamico.
    pub id: Option<VoceComputoId>,
    /// Numero d'ordine attuale della voce collegata.
    pub numero: Option<usize>,
    /// Codice della voce collegata.
    pub codice: Option<String>,
    /// Descrizione breve della voce collegata.
    pub descrizione: Option<String>,
    /// Unità di misura della voce collegata.
    pub unita_misura: Option<String>,
    /// Testo del riferimento statico.
    pub riferimento: Option<String>,
    /// Quantità citata (assente se non calcolabile).
    pub quantita: Option<Quantita>,
}

fn vista_vedi_voce(
    v: &VediVoce,
    computo: Option<&Computo>,
    riferite: &dyn QuantitaRiferite,
) -> VistaVediVoce {
    match v {
        VediVoce::Statica {
            quantita,
            riferimento,
        } => VistaVediVoce {
            testo: if riferimento.is_empty() {
                "Vedi voce".into()
            } else {
                format!("Vedi {riferimento}")
            },
            statica: true,
            id: None,
            numero: None,
            codice: None,
            descrizione: None,
            unita_misura: None,
            riferimento: Some(riferimento.clone()),
            quantita: Some(Quantita::new(*quantita)),
        },
        VediVoce::Collegata(id) => {
            let trovata =
                computo.and_then(|c| c.voci.iter().enumerate().find(|(_, x)| x.id == *id));
            VistaVediVoce {
                testo: trovata.map_or_else(
                    || "Vedi voce (inesistente)".into(),
                    |(i, _)| format!("Vedi voce n. {}", i + 1),
                ),
                statica: false,
                id: Some(*id),
                numero: trovata.map(|(i, _)| i + 1),
                codice: trovata.map(|(_, x)| x.prezzo.codice.clone()),
                descrizione: trovata.map(|(_, x)| x.prezzo.descrizione_breve.clone()),
                unita_misura: trovata.map(|(_, x)| x.prezzo.unita_misura.clone()),
                riferimento: None,
                quantita: riferite.quantita(*id).ok().map(Quantita::new),
            }
        }
    }
}

/// Righi di una voce: `computo` e `riferite` servono ai «vedi voce» collegati.
fn righi(
    r: &[RigoMisura],
    regole: &RegoleArrotondamento,
    computo: Option<&Computo>,
    riferite: &dyn QuantitaRiferite,
) -> Vec<VistaRigo> {
    r.iter()
        .enumerate()
        .map(|(i, rigo)| {
            let (quantita, errore) = match rigo.calcola_con(regole, riferite) {
                Ok(q) => (q.map(|q| q.quantita), None),
                Err(e) => (None, Some(e.to_string())),
            };
            VistaRigo {
                numero: i + 1,
                commento: rigo.tipo == TipoRigo::Commento,
                descrizione: rigo.descrizione.clone(),
                parti_uguali: rigo.parti_uguali.as_ref().map(testo_fattore),
                lunghezza: rigo.lunghezza.as_ref().map(testo_fattore),
                larghezza: rigo.larghezza.as_ref().map(testo_fattore),
                altezza_peso: rigo.altezza_peso.as_ref().map(testo_fattore),
                formula: rigo.formula.clone(),
                deduzione: rigo.segno == SegnoRigo::Deduzione,
                quantita,
                errore,
                vedi_voce: rigo
                    .vedi_voce
                    .as_ref()
                    .map(|v| vista_vedi_voce(v, computo, riferite)),
            }
        })
        .collect()
}

/// Categorie assegnate, per codice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct VistaCategorie {
    /// Super categoria.
    pub super_categoria: Option<String>,
    /// Categoria.
    pub categoria: Option<String>,
    /// Sotto categoria.
    pub sotto_categoria: Option<String>,
}

fn categorie(c: &Categorie, a: &AssegnazioneCategorie) -> VistaCategorie {
    let codice = |l: LivelloCategoria| {
        a.di(l)
            .and_then(|id| c.trova(l, id))
            .map(|x| x.codice.clone())
    };
    VistaCategorie {
        super_categoria: codice(LivelloCategoria::SuperCategoria),
        categoria: codice(LivelloCategoria::Categoria),
        sotto_categoria: codice(LivelloCategoria::SottoCategoria),
    }
}

/// Stato del prezzo fotografato rispetto all'elenco prezzi corrente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatoPrezzo {
    /// Uguale alla voce dell'elenco prezzi.
    Allineato,
    /// La voce dell'elenco prezzi è cambiata: `riallinea_prezzo` la aggiorna.
    NonAllineato,
    /// Nessuna voce con questo codice nell'elenco prezzi.
    AssenteInElenco,
}

fn stato_prezzo(p: &Progetto, prezzo: &PrezzoApplicato) -> StatoPrezzo {
    match p.elenco_prezzi.voce_per_codice(&prezzo.codice) {
        None => StatoPrezzo::AssenteInElenco,
        Some(v) if prezzo.confronta(v).is_empty() => StatoPrezzo::Allineato,
        Some(_) => StatoPrezzo::NonAllineato,
    }
}

/// Voce di computo con righi e importi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaVoceComputo {
    /// Numero d'ordine, da 1.
    pub numero: usize,
    /// Identificativo (stabile: i comandi dell'interfaccia lo usano al posto del
    /// numero, che cambia se un'altra finestra aggiunge o toglie voci).
    pub id: VoceComputoId,
    /// Codice (tariffa).
    pub codice: String,
    /// Descrizione breve.
    pub descrizione_breve: String,
    /// Descrizione estesa.
    pub descrizione_estesa: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo applicato.
    pub prezzo: Prezzo,
    /// Natura.
    pub natura: NaturaVoce,
    /// Quantità (assente se non calcolabile).
    pub quantita: Option<Quantita>,
    /// Importo (assente se non calcolabile).
    pub importo: Option<Importo>,
    /// Errore di calcolo.
    pub errore: Option<String>,
    /// Stato del prezzo rispetto all'elenco.
    pub stato_prezzo: StatoPrezzo,
    /// Categorie.
    pub categorie: VistaCategorie,
    /// Codice completo del nodo WBS.
    pub wbs: Option<String>,
    /// Capitolo della voce dell'elenco prezzi da cui viene (per i filtri).
    pub capitolo: Option<String>,
    /// Note.
    pub note: Option<String>,
    /// Righi di misura.
    pub righi: Vec<VistaRigo>,
}

/// Computo completo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaComputo {
    /// Nome.
    pub nome: String,
    /// Tipo.
    pub tipo: TipoComputo,
    /// Totale lavori.
    pub totale_lavori: Option<Importo>,
    /// Totale oneri della sicurezza.
    pub totale_oneri_sicurezza: Option<Importo>,
    /// Totale.
    pub totale: Option<Importo>,
    /// Totale della manodopera stimata.
    pub totale_manodopera: Option<Importo>,
    /// Errore che impedisce il calcolo dei totali.
    pub errore: Option<String>,
    /// Voci.
    pub voci: Vec<VistaVoceComputo>,
}

/// Computo per nome (facoltativo se ce n'è uno solo).
///
/// # Errors
/// Computo inesistente o ambiguo.
pub fn computo(p: &Progetto, nome: Option<&str>) -> Result<VistaComputo, ErroreApplicazione> {
    let id = trova_computo(p, nome)?;
    let c = p.computo(id).ok_or_else(|| ErroreApplicazione::Richiesta {
        messaggio: "computo inesistente".into(),
    })?;
    let totali = p.calcola_computo(id);
    let riferite: QuantitaComputo = c.quantita_riferite(&p.regole);
    let elenco = &p.elenco_prezzi;
    let capitoli: HashMap<_, _> = elenco
        .capitoli
        .iter()
        .map(|c| (c.id, c.codice.as_str()))
        .collect();
    let per_id: HashMap<_, _> = elenco.voci.iter().map(|v| (v.id, v.capitolo)).collect();
    let per_codice: HashMap<_, _> = elenco
        .voci
        .iter()
        .map(|v| (v.codice.as_str(), v.capitolo))
        .collect();
    let capitolo_di = |v: &opencomputo_core::computo::VoceComputo| -> Option<String> {
        let cap = v
            .prezzo
            .origine
            .filter(|o| o.prezzario == elenco.id)
            .and_then(|o| per_id.get(&o.voce).copied())
            .or_else(|| per_codice.get(v.prezzo.codice.as_str()).copied())
            .flatten()?;
        capitoli.get(&cap).map(|c| (*c).to_owned())
    };
    let voci = c
        .voci
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let (quantita, importo, errore) = match v.calcola_in(&p.regole, &riferite) {
                Ok(r) => (Some(r.quantita), Some(r.importo), None),
                Err(e) => (None, None, Some(e.to_string())),
            };
            VistaVoceComputo {
                numero: i + 1,
                id: v.id,
                codice: v.prezzo.codice.clone(),
                descrizione_breve: v.prezzo.descrizione_breve.clone(),
                descrizione_estesa: v.prezzo.descrizione_estesa.clone(),
                unita_misura: v.prezzo.unita_misura.clone(),
                prezzo: v.prezzo.prezzo,
                natura: v.prezzo.natura,
                quantita,
                importo,
                errore,
                stato_prezzo: stato_prezzo(p, &v.prezzo),
                categorie: categorie(&p.categorie, &v.categorie),
                wbs: v.wbs.and_then(|id| p.wbs.codice_completo(id).ok()),
                capitolo: capitolo_di(v),
                note: v.note.clone(),
                righi: righi(&v.righi, &p.regole, Some(c), &riferite),
            }
        })
        .collect();
    let (lavori, sicurezza, totale, manodopera, errore) = match totali {
        Ok(r) => (
            Some(r.totale_lavori),
            Some(r.totale_oneri_sicurezza),
            Some(r.totale),
            Some(r.totale_manodopera),
            None,
        ),
        Err(e) => (None, None, None, None, Some(e.to_string())),
    };
    Ok(VistaComputo {
        nome: c.nome.clone(),
        tipo: c.tipo,
        totale_lavori: lavori,
        totale_oneri_sicurezza: sicurezza,
        totale,
        totale_manodopera: manodopera,
        errore,
        voci,
    })
}

/// Registrazione del libretto delle misure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaRegistrazione {
    /// Numero d'ordine.
    pub numero: u32,
    /// Data.
    pub data: Date,
    /// Codice.
    pub codice: String,
    /// Descrizione breve.
    pub descrizione_breve: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo applicato.
    pub prezzo: Prezzo,
    /// Natura.
    pub natura: NaturaVoce,
    /// Quantità.
    pub quantita: Option<Quantita>,
    /// Importo lordo.
    pub importo: Option<Importo>,
    /// Errore di calcolo.
    pub errore: Option<String>,
    /// SAL in cui è contabilizzata.
    pub sal: Option<u32>,
    /// `true` se il SAL è emesso o la contabilità è chiusa: non modificabile.
    pub bloccata: bool,
    /// `true` se fa parte dello stato finale (non compresa nei SAL, con il conto
    /// finale redatto).
    pub stato_finale: bool,
    /// Annotazioni.
    pub annotazioni: Option<String>,
    /// Righi di misura.
    pub righi: Vec<VistaRigo>,
}

/// Stato di avanzamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaSal {
    /// Numero.
    pub numero: u32,
    /// Lavori a tutto il.
    pub lavori_a_tutto_il: Date,
    /// Data di emissione.
    pub data_emissione: Option<Date>,
    /// Stato.
    pub stato: StatoSal,
    /// Numeri delle registrazioni contabilizzate.
    pub registrazioni: Vec<u32>,
    /// Prospetto: congelato se emesso, ricalcolato se in bozza.
    pub riepilogo: Option<RiepilogoSal>,
    /// Errore del calcolo del prospetto.
    pub errore: Option<String>,
    /// Certificato di pagamento (congelato o ricalcolato).
    pub certificato: Option<CertificatoPagamento>,
    /// `true` se il certificato è quello congelato all'emissione.
    pub certificato_congelato: bool,
    /// Errore del calcolo del certificato.
    pub errore_certificato: Option<String>,
    /// Recupero dell'anticipazione sospeso (SAL in bozza o emesso).
    pub recupero_anticipazione_sospeso: bool,
    /// Recupero di tutto il residuo dell'anticipazione.
    pub recupera_residuo_anticipazione: bool,
}

/// Penale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaPenale {
    /// Numero.
    pub numero: u32,
    /// Data.
    pub data: Date,
    /// Motivo.
    pub descrizione: String,
    /// Importo.
    pub importo: Importo,
    /// Giorni di ritardo.
    pub giorni: Option<u32>,
    /// Per mille giornaliero.
    pub per_mille: Option<rust_decimal::Decimal>,
    /// SAL emesso in cui è applicata (non più modificabile).
    pub sal: Option<u32>,
    /// `true` se applicata nella rata di saldo del conto finale emesso.
    pub conto_finale: bool,
}

/// Conto finale, firma, collaudo e rata di saldo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaContoFinale {
    /// Data di ultimazione dei lavori.
    pub data_ultimazione: Date,
    /// Data del conto finale.
    pub data: Date,
    /// Bozza o emesso.
    pub stato: StatoSal,
    /// Numeri delle registrazioni dello stato finale.
    pub registrazioni: Vec<u32>,
    /// Numeri delle penali della rata di saldo.
    pub penali: Vec<u32>,
    /// Riepilogo e rata di saldo (congelati se emesso).
    pub riepilogo: Option<RiepilogoContoFinale>,
    /// Errore del calcolo.
    pub errore: Option<String>,
    /// Firma dell'esecutore.
    pub firma_esecutore: Option<Date>,
    /// Riserve confermate.
    pub riserve: Option<String>,
    /// Collaudo o certificato di regolare esecuzione.
    pub collaudo: Option<Collaudo>,
    /// Note.
    pub note: Option<String>,
}

/// Contabilità.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaContabilita {
    /// Parametri contrattuali (assenti senza contratto).
    pub parametri: Option<ParametriContabili>,
    /// Registrazioni del libretto.
    pub registrazioni: Vec<VistaRegistrazione>,
    /// SAL.
    pub sal: Vec<VistaSal>,
    /// Penali.
    pub penali: Vec<VistaPenale>,
    /// Importo contrattuale (base delle penali e dell'anticipazione).
    pub importo_contrattuale: Option<Importo>,
    /// Conto finale, se redatto.
    pub conto_finale: Option<VistaContoFinale>,
    /// `true` se il conto finale è emesso: niente più modifiche alla contabilità.
    pub chiusa: bool,
}

/// Contabilità completa: libretto e SAL.
#[must_use]
pub fn contabilita(p: &Progetto) -> VistaContabilita {
    let c = &p.contabilita;
    let registrazioni = c
        .libretto()
        .registrazioni
        .iter()
        .map(|r| {
            let (quantita, importo, errore) = match r.calcola(&p.regole) {
                Ok(x) => (Some(x.quantita), Some(x.importo), None),
                Err(e) => (None, None, Some(e.to_string())),
            };
            let sal = c.sal_di(r.id);
            VistaRegistrazione {
                numero: r.numero,
                data: r.data,
                codice: r.prezzo.codice.clone(),
                descrizione_breve: r.prezzo.descrizione_breve.clone(),
                unita_misura: r.prezzo.unita_misura.clone(),
                prezzo: r.prezzo.prezzo,
                natura: r.prezzo.natura,
                quantita,
                importo,
                errore,
                sal: sal.map(|s| s.numero),
                bloccata: c.chiusa() || sal.is_some_and(|s| s.stato == StatoSal::Emesso),
                stato_finale: sal.is_none() && c.conto_finale().is_some(),
                annotazioni: r.annotazioni.clone(),
                righi: righi(&r.righi, &p.regole, None, &SenzaRiferimenti),
            }
        })
        .collect();
    let sal = c
        .sal()
        .iter()
        .map(|s| {
            let (riepilogo, errore) = match (&s.riepilogo_emesso, s.stato) {
                (Some(r), StatoSal::Emesso) => (Some(r.clone()), None),
                _ => match p.calcola_sal(s.id) {
                    Ok(r) => (Some(r), None),
                    Err(e) => (None, Some(e.to_string())),
                },
            };
            let (certificato, errore_certificato) = match p.calcola_certificato(s.id) {
                Ok(c) => (Some(c), None),
                Err(e) => (None, Some(e.to_string())),
            };
            VistaSal {
                certificato,
                certificato_congelato: s.stato == StatoSal::Emesso
                    && s.certificato_emesso.is_some(),
                errore_certificato,
                recupero_anticipazione_sospeso: s.recupero_anticipazione_sospeso,
                recupera_residuo_anticipazione: s.recupera_residuo_anticipazione,
                numero: s.numero,
                lavori_a_tutto_il: s.lavori_a_tutto_il,
                data_emissione: s.data_emissione,
                stato: s.stato,
                registrazioni: s
                    .registrazioni
                    .iter()
                    .filter_map(|id| c.libretto().registrazione(*id).map(|r| r.numero))
                    .collect(),
                riepilogo,
                errore,
            }
        })
        .collect();
    VistaContabilita {
        parametri: p.contratto.as_ref().map(|c| c.parametri.clone()),
        registrazioni,
        sal,
        penali: c
            .penali()
            .iter()
            .map(|x| VistaPenale {
                numero: x.numero,
                data: x.data,
                descrizione: x.descrizione.clone(),
                importo: x.importo,
                giorni: x.giorni,
                per_mille: x.per_mille,
                sal: c.sal_della_penale(x.id).map(|s| s.numero),
                conto_finale: c
                    .conto_finale()
                    .is_some_and(|k| k.stato == StatoSal::Emesso && k.penali.contains(&x.id)),
            })
            .collect(),
        importo_contrattuale: p.contratto.as_ref().and_then(|c| c.importo_contrattuale),
        conto_finale: c.conto_finale().map(|k| {
            let (riepilogo, errore) = match p.calcola_conto_finale() {
                Ok(r) => (Some(r), None),
                Err(e) => (None, Some(e.to_string())),
            };
            VistaContoFinale {
                data_ultimazione: k.data_ultimazione,
                data: k.data,
                stato: k.stato,
                registrazioni: c
                    .registrazioni_stato_finale()
                    .iter()
                    .map(|r| r.numero)
                    .collect(),
                penali: c.penali_conto_finale().iter().map(|x| x.numero).collect(),
                riepilogo,
                errore,
                firma_esecutore: k.firma_esecutore,
                riserve: k.riserve.clone(),
                collaudo: k.collaudo.clone(),
                note: k.note.clone(),
            }
        }),
        chiusa: c.chiusa(),
    }
}

/// Colonna dell'anteprima.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColonnaAnteprima {
    /// Intestazione.
    pub titolo: String,
    /// Larghezza relativa.
    pub peso: u16,
    /// `sinistra`, `centro` o `destra`.
    pub allineamento: &'static str,
}

/// Riga dell'anteprima, con le celle già formattate come nella stampa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RigaAnteprima {
    /// Ruolo della riga (`voce`, `rigo`, `sommano`, `totale`…).
    pub stile: &'static str,
    /// Celle.
    pub celle: Vec<String>,
}

/// Sezione dell'anteprima.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SezioneAnteprima {
    /// Titolo.
    pub titolo: String,
    /// Note.
    pub note: Vec<String>,
    /// Colonne.
    pub colonne: Vec<ColonnaAnteprima>,
    /// Righe.
    pub righe: Vec<RigaAnteprima>,
}

/// Anteprima di un report: lo stesso documento da cui nascono PDF, XLSX e CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anteprima {
    /// Nome di base dei file.
    pub nome: String,
    /// Titolo.
    pub titolo: String,
    /// Intestazione (etichetta, valore).
    pub intestazione: Vec<(String, String)>,
    /// Sezioni.
    pub sezioni: Vec<SezioneAnteprima>,
    /// Piè di pagina.
    pub piede: Option<String>,
}

/// Converte un documento di report in anteprima.
#[must_use]
pub fn anteprima(nome: String, d: &Documento) -> Anteprima {
    Anteprima {
        nome,
        titolo: d.titolo.clone(),
        intestazione: d.intestazione.clone(),
        sezioni: d
            .sezioni
            .iter()
            .map(|s| SezioneAnteprima {
                titolo: s.titolo.clone(),
                note: s.note.clone(),
                colonne: s
                    .tabella
                    .colonne
                    .iter()
                    .map(|c| ColonnaAnteprima {
                        titolo: c.titolo.clone(),
                        peso: c.peso,
                        allineamento: match c.allineamento {
                            Allineamento::Sinistra => "sinistra",
                            Allineamento::Centro => "centro",
                            Allineamento::Destra => "destra",
                        },
                    })
                    .collect(),
                righe: s
                    .tabella
                    .righe
                    .iter()
                    .map(|r| RigaAnteprima {
                        stile: r.stile.codice(),
                        celle: r.celle.iter().map(|c| c.in_testo()).collect(),
                    })
                    .collect(),
            })
            .collect(),
        piede: d.piede.clone(),
    }
}

/// Riga di un'analisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaRigaAnalisi {
    /// Numero, da 1.
    pub numero: usize,
    /// `risorsa`, `voce`, `sotto_analisi` (voce analizzata) o `libera`.
    pub origine: &'static str,
    /// Codice della risorsa o della voce collegata (per i comandi).
    pub riferimento: Option<String>,
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo.
    pub tipo: TipoRisorsa,
    /// Quantità come testo modificabile (espressione o numero con la virgola).
    pub quantita: String,
    /// Quantità calcolata (dall'espressione, se c'è).
    pub valore_quantita: Option<Quantita>,
    /// Prezzo corrente usato nel calcolo.
    pub prezzo: Option<Prezzo>,
    /// Importo.
    pub importo: Option<Importo>,
    /// `true` se la risorsa o la voce collegata non esiste più.
    pub orfana: bool,
}

/// Variabile di un'analisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaVariabile {
    /// Nome.
    pub nome: String,
    /// Espressione o valore come scritto.
    pub espressione: String,
    /// Descrizione.
    pub descrizione: String,
    /// Valore calcolato.
    pub valore: Option<rust_decimal::Decimal>,
}

/// Analisi di una voce, calcolata con i prezzi correnti.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaAnalisi {
    /// Codice della voce.
    pub voce: String,
    /// Quantità di riferimento.
    pub quantita_riferimento: Quantita,
    /// Spese generali.
    pub spese_generali: Percentuale,
    /// Utile d'impresa.
    pub utile_impresa: Percentuale,
    /// Base dell'utile.
    pub base_utile: BaseUtile,
    /// Quota degli oneri aziendali della sicurezza nelle spese generali.
    pub quota_oneri_sicurezza: Option<Percentuale>,
    /// Variabili con il valore calcolato (assente se non calcolabile).
    pub variabili: Vec<VistaVariabile>,
    /// Righe.
    pub righe: Vec<VistaRigaAnalisi>,
    /// Risultato (assente se non calcolabile).
    pub risultato: Option<RisultatoAnalisi>,
    /// Errore di calcolo.
    pub errore: Option<String>,
    /// `true` se il prezzo della voce coincide con quello calcolato.
    pub allineata: bool,
}

fn vista_analisi(
    pr: &Prezzario,
    v: &VocePrezzario,
    id: AnalisiId,
    regole: &RegoleArrotondamento,
) -> Option<VistaAnalisi> {
    let a = pr.analisi(id)?;
    let (risultato, errore) = match pr.calcola_analisi(id, regole) {
        Ok(r) => (Some(r), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let righe = a
        .righe
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (origine, riferimento, orfana) = match r.origine {
                None => ("libera", None, false),
                Some(OrigineRigaAnalisi::Risorsa(x)) => match pr.risorsa(x) {
                    Some(x) => ("risorsa", Some(x.codice.clone()), false),
                    None => ("risorsa", None, true),
                },
                Some(OrigineRigaAnalisi::VocePrezzario(x)) => match pr.voce(x) {
                    Some(x) if x.analisi.is_some() => {
                        ("sotto_analisi", Some(x.codice.clone()), false)
                    }
                    Some(x) => ("voce", Some(x.codice.clone()), false),
                    None => ("voce", None, true),
                },
            };
            let calcolata = risultato.as_ref().and_then(|x| x.righe.get(i));
            VistaRigaAnalisi {
                numero: i + 1,
                origine,
                riferimento,
                codice: r.codice.clone(),
                descrizione: r.descrizione.clone(),
                unita_misura: r.unita_misura.clone(),
                tipo: r.tipo,
                quantita: r
                    .espressione
                    .clone()
                    .unwrap_or_else(|| testo_fattore(&Fattore::Valore(r.quantita.valore()))),
                valore_quantita: calcolata.map(|c| c.quantita),
                prezzo: calcolata.map(|c| c.prezzo),
                importo: calcolata.map(|c| c.importo),
                orfana,
            }
        })
        .collect();
    Some(VistaAnalisi {
        voce: v.codice.clone(),
        quantita_riferimento: a.quantita_riferimento,
        spese_generali: a.spese_generali,
        utile_impresa: a.utile_impresa,
        base_utile: a.base_utile,
        quota_oneri_sicurezza: a.quota_oneri_sicurezza,
        variabili: a
            .variabili
            .iter()
            .map(|x| VistaVariabile {
                nome: x.nome.clone(),
                espressione: x.espressione.replace('.', ","),
                descrizione: x.descrizione.clone(),
                valore: risultato
                    .as_ref()
                    .and_then(|r| r.variabili.iter().find(|v| v.nome == x.nome))
                    .map(|v| v.valore),
            })
            .collect(),
        righe,
        allineata: risultato
            .as_ref()
            .is_some_and(|r| r.prezzo_unitario == v.prezzo),
        risultato,
        errore,
    })
}

/// Risorsa elementare con le analisi che la usano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaRisorsa {
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Tipo.
    pub tipo: TipoRisorsa,
    /// Prezzo.
    pub prezzo: Prezzo,
    /// Codici delle voci analizzate che la usano.
    pub usata_in: Vec<String>,
}

/// Sintesi dell'analisi di una voce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SintesiAnalisi {
    /// Codice della voce.
    pub voce: String,
    /// Descrizione breve.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo della voce in elenco.
    pub prezzo_voce: Prezzo,
    /// Prezzo calcolato dall'analisi.
    pub prezzo_calcolato: Option<Prezzo>,
    /// Incidenze per tipo.
    pub incidenze: std::collections::BTreeMap<TipoRisorsa, Percentuale>,
    /// Righe dell'analisi.
    pub righe: usize,
    /// Errore di calcolo.
    pub errore: Option<String>,
    /// Sigla del nuovo prezzo.
    pub nuovo_prezzo: Option<String>,
}

/// Risorse e analisi dell'elenco prezzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaAnalisiPrezzi {
    /// Spese generali e utile predefiniti.
    pub parametri: ParametriAnalisi,
    /// Risorse elementari.
    pub risorse: Vec<VistaRisorsa>,
    /// Voci analizzate, nell'ordine dell'elenco.
    pub analisi: Vec<SintesiAnalisi>,
}

/// Risorse e analisi dell'elenco prezzi del progetto.
#[must_use]
pub fn analisi_prezzi(p: &Progetto) -> VistaAnalisiPrezzi {
    let e = &p.elenco_prezzi;
    let risorse = e
        .risorse
        .iter()
        .map(|r| VistaRisorsa {
            codice: r.codice.clone(),
            descrizione: r.descrizione.clone(),
            unita_misura: r.unita_misura.clone(),
            tipo: r.tipo,
            prezzo: r.prezzo,
            usata_in: e
                .voci
                .iter()
                .filter(|v| {
                    v.analisi.and_then(|a| e.analisi(a)).is_some_and(|a| {
                        a.righe
                            .iter()
                            .any(|x| x.origine == Some(OrigineRigaAnalisi::Risorsa(r.id)))
                    })
                })
                .map(|v| v.codice.clone())
                .collect(),
        })
        .collect();
    let analisi = e
        .voci
        .iter()
        .filter_map(|v| {
            let a = e.analisi(v.analisi?)?;
            let (prezzo_calcolato, incidenze, errore) = match e.calcola_analisi(a.id, &p.regole) {
                Ok(r) => (Some(r.prezzo_unitario), r.incidenze, None),
                Err(x) => (None, Default::default(), Some(x.to_string())),
            };
            Some(SintesiAnalisi {
                voce: v.codice.clone(),
                descrizione: v.descrizione_breve.clone(),
                unita_misura: v.unita_misura.clone(),
                prezzo_voce: v.prezzo,
                prezzo_calcolato,
                incidenze,
                righe: a.righe.len(),
                errore,
                nuovo_prezzo: v.nuovo_prezzo.as_ref().map(NuovoPrezzo::sigla),
            })
        })
        .collect();
    VistaAnalisiPrezzi {
        parametri: e.parametri_analisi(),
        risorse,
        analisi,
    }
}

/// Nuovo prezzo con i dati della voce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaNuovoPrezzo {
    /// Sigla (`NP.1`).
    pub sigla: String,
    /// Numero.
    pub numero: u32,
    /// Codice della voce.
    pub codice: String,
    /// Descrizione breve.
    pub descrizione: String,
    /// Unità di misura.
    pub unita_misura: String,
    /// Prezzo.
    pub prezzo: Prezzo,
    /// Data di formazione.
    pub data: Date,
    /// Motivo.
    pub motivo: String,
    /// Criterio.
    pub criterio: CriterioNuovoPrezzo,
    /// Soggetto a ribasso.
    pub soggetto_a_ribasso: bool,
    /// Numero del verbale.
    pub verbale: Option<u32>,
    /// `true` se il verbale è approvato.
    pub approvato: bool,
    /// `true` se il prezzo deriva da un'analisi.
    pub analizzata: bool,
    /// Registrazioni del libretto che lo usano.
    pub registrazioni: usize,
}

/// Verbale di concordamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaVerbale {
    /// Numero.
    pub numero: u32,
    /// Data.
    pub data: Date,
    /// Firma dell'esecutore.
    pub firma_esecutore: Option<Date>,
    /// Riserve.
    pub riserve: Option<String>,
    /// Approvazione.
    pub approvato_il: Option<Date>,
    /// Atto di approvazione.
    pub atto_approvazione: Option<String>,
    /// Note.
    pub note: Option<String>,
    /// Codici delle voci comprese.
    pub nuovi_prezzi: Vec<String>,
}

/// Nuovi prezzi e verbali di concordamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaNuoviPrezzi {
    /// Nuovi prezzi in ordine di numero.
    pub nuovi_prezzi: Vec<VistaNuovoPrezzo>,
    /// Verbali in ordine di numero.
    pub verbali: Vec<VistaVerbale>,
}

/// Nuovi prezzi e verbali del progetto.
#[must_use]
pub fn nuovi_prezzi(p: &Progetto) -> VistaNuoviPrezzi {
    let e = &p.elenco_prezzi;
    let c = &p.contabilita;
    let mut nuovi: Vec<VistaNuovoPrezzo> = e
        .voci
        .iter()
        .filter_map(|v| {
            let n = v.nuovo_prezzo.as_ref()?;
            Some(VistaNuovoPrezzo {
                sigla: n.sigla(),
                numero: n.numero,
                codice: v.codice.clone(),
                descrizione: v.descrizione_breve.clone(),
                unita_misura: v.unita_misura.clone(),
                prezzo: v.prezzo,
                data: n.data,
                motivo: n.motivo.clone(),
                criterio: n.criterio,
                soggetto_a_ribasso: n.soggetto_a_ribasso,
                verbale: n.verbale,
                approvato: n
                    .verbale
                    .and_then(|x| c.verbale_nuovi_prezzi(x))
                    .is_some_and(|x| x.approvato()),
                analizzata: v.analisi.is_some(),
                registrazioni: c
                    .libretto()
                    .registrazioni
                    .iter()
                    .filter(|r| {
                        r.prezzo.origine.map_or(r.prezzo.codice == v.codice, |o| {
                            o.prezzario == e.id && o.voce == v.id
                        })
                    })
                    .count(),
            })
        })
        .collect();
    nuovi.sort_by_key(|n| n.numero);
    let verbali = c
        .verbali_nuovi_prezzi()
        .iter()
        .map(|v| VistaVerbale {
            numero: v.numero,
            data: v.data,
            firma_esecutore: v.firma_esecutore,
            riserve: v.riserve.clone(),
            approvato_il: v.approvato_il,
            atto_approvazione: v.atto_approvazione.clone(),
            note: v.note.clone(),
            nuovi_prezzi: nuovi
                .iter()
                .filter(|n| n.verbale == Some(v.numero))
                .map(|n| n.codice.clone())
                .collect(),
        })
        .collect();
    VistaNuoviPrezzi {
        nuovi_prezzi: nuovi,
        verbali,
    }
}

/// Fabbisogno di cantiere di un computo.
///
/// # Errors
/// Computo inesistente o dati non calcolabili.
pub fn fabbisogno(p: &Progetto, nome: Option<&str>) -> Result<Fabbisogno, ErroreApplicazione> {
    let id = trova_computo(p, nome)?;
    p.fabbisogno(id).map_err(|e| ErroreApplicazione::Richiesta {
        messaggio: e.to_string(),
    })
}

/// Voce dell'elenco nel confronto con un listino, con il capitolo (per i filtri).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaConfrontoVoce {
    /// Confronto calcolato dal core.
    #[serde(flatten)]
    pub confronto: ConfrontoVoce,
    /// Codice del capitolo nell'elenco.
    pub capitolo: Option<String>,
}

/// Confronto dell'elenco prezzi con un prezzario caricato (ADR 0022).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaConfronto {
    /// Nome del prezzario caricato.
    pub prezzario: String,
    /// Fonte (nome ed edizione).
    pub fonte: String,
    /// Voci del listino.
    pub voci_listino: usize,
    /// Voci dell'elenco.
    pub voci: Vec<VistaConfrontoVoce>,
    /// Risorse delle analisi.
    pub risorse: Vec<ConfrontoRisorsa>,
    /// Voci del listino senza corrispondenza nell'elenco.
    pub voci_solo_nel_listino: usize,
    /// Codici ripetuti nel listino.
    pub codici_ripetuti: Vec<String>,
    /// Computi esclusi dagli importi perché non calcolabili.
    pub computi_non_calcolabili: Vec<String>,
}

/// Confronto dell'elenco prezzi e delle risorse con un listino.
#[must_use]
pub fn confronto_prezzario(
    p: &Progetto,
    nome: &str,
    listino: &Prezzario,
    prefisso: Option<&SostituzionePrefisso>,
) -> VistaConfronto {
    let c = confronta_listino(p, listino, prefisso);
    let capitoli: HashMap<_, _> = p
        .elenco_prezzi
        .capitoli
        .iter()
        .map(|c| (c.id, c.codice.clone()))
        .collect();
    let capitolo_di: HashMap<_, _> = p
        .elenco_prezzi
        .voci
        .iter()
        .map(|v| (v.id, v.capitolo.and_then(|c| capitoli.get(&c).cloned())))
        .collect();
    VistaConfronto {
        prezzario: nome.to_owned(),
        fonte: listino.fonte(),
        voci_listino: listino.voci.len(),
        voci: c
            .voci
            .into_iter()
            .map(|v| VistaConfrontoVoce {
                capitolo: capitolo_di.get(&v.voce).cloned().flatten(),
                confronto: v,
            })
            .collect(),
        risorse: c.risorse,
        voci_solo_nel_listino: c.voci_solo_nel_listino,
        codici_ripetuti: c.codici_ripetuti,
        computi_non_calcolabili: c.computi_non_calcolabili,
    }
}

/// Cronoprogramma con il piano calcolato (ADR 0024).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VistaCronoprogramma {
    /// Inizio dei lavori impostato nel cronoprogramma.
    pub inizio_lavori: Option<Date>,
    /// Consegna dei lavori del contratto (inizio se il precedente manca).
    pub consegna_lavori: Option<Date>,
    /// Calendario.
    pub calendario: opencomputo_core::cronoprogramma::Calendario,
    /// Livello di dettaglio impostato.
    pub livello_dettaglio: Option<u32>,
    /// Computo degli importi impostato (nome).
    pub computo: Option<String>,
    /// Nomi dei computi.
    pub computi: Vec<String>,
    /// Giorno di salvataggio della linea di base.
    pub linea_base: Option<Date>,
    /// Numero di nodi WBS.
    pub nodi_wbs: usize,
    /// Codice completo di ogni nodo WBS (per i legami con nodi non visibili).
    pub codici_wbs: HashMap<String, String>,
    /// Piano calcolato.
    pub piano: Option<opencomputo_core::cronoprogramma::Piano>,
    /// Errore del calcolo (inizio mancante, ciclo…).
    pub errore: Option<String>,
}

/// Cronoprogramma del progetto.
#[must_use]
pub fn cronoprogramma(p: &Progetto) -> VistaCronoprogramma {
    let c = &p.cronoprogramma;
    let (piano, errore) = match opencomputo_core::cronoprogramma::calcola(p, None) {
        Ok(piano) => (Some(piano), None),
        Err(e) => (None, Some(e.to_string())),
    };
    VistaCronoprogramma {
        inizio_lavori: c.inizio_lavori,
        consegna_lavori: p.contratto.as_ref().and_then(|ct| ct.data_consegna_lavori),
        calendario: c.calendario.clone(),
        livello_dettaglio: c.livello_dettaglio,
        computo: c
            .computo
            .and_then(|id| p.computo(id))
            .map(|x| x.nome.clone()),
        computi: p.computi.iter().map(|x| x.nome.clone()).collect(),
        linea_base: c.linea_base.as_ref().map(|b| b.salvata_il),
        nodi_wbs: p.wbs.nodi.len(),
        codici_wbs: p
            .wbs
            .nodi
            .iter()
            .map(|n| {
                (
                    n.id.to_string(),
                    p.wbs.codice_completo(n.id).unwrap_or_default(),
                )
            })
            .collect(),
        piano,
        errore,
    }
}
