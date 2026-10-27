//! Export di un computo in XPWE (**sperimentale**).
//!
//! La struttura scritta è quella dedotta in [`crate::struttura`]: l'andata e
//! ritorno con questo crate è verificata dai test, ma l'apertura in PriMus no.
//! Per i progetti importati da XPWE vengono ripercorsi l'ordine originale dei
//! campi e reinseriti i dati conservati; i campi mappati sono scritti dal modello.

use std::collections::{HashMap, HashSet};

use opencomputo_core::computo::{Computo, QuantitaComputo, VoceComputo};
use opencomputo_core::id::{
    CapitoloId, ClasseId, ComputoId, RigoMisuraId, VoceComputoId, VocePrezzarioId,
};
use opencomputo_core::misura::{
    Fattore, QuantitaRiferite, RigoMisura, SegnoRigo, TipoRigo, VARIABILE_VEDI_VOCE,
    VARIABILI_RIGO, VediVoce,
};
use opencomputo_core::partita::PrezzoApplicato;
use opencomputo_core::prezzario::{NaturaVoce, VocePrezzario};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::provenienza::{Anomalia, FormatoOrigine, Provenienza};
use opencomputo_core::validazione::Gravita;

use crate::conservati::{self, Dati};
use crate::errore::ErroreXpwe;
use crate::struttura as st;
use crate::testo;
use crate::xml::Scrittore;

/// Risultato dell'export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RisultatoExport {
    /// Documento XPWE (UTF-8).
    pub xml: String,
    /// Approssimazioni e dati non esportati.
    pub anomalie: Vec<Anomalia>,
}

/// Dati originali di un'entità solo se provengono da XPWE.
fn dati_xpwe(p: Option<&Provenienza>) -> Option<&Dati> {
    p.filter(|p| p.formato == FormatoOrigine::Xpwe)
        .map(|p| &p.dati_originali)
}

/// Assegna identificativi XPWE: riusa quelli originali se univoci, altrimenti
/// numera dal massimo intero usato più uno.
#[derive(Default)]
struct Assegnatore {
    usati: HashSet<String>,
    prossimo: u64,
}

impl Assegnatore {
    /// I nuovi identificativi partono dal massimo intero preferito più uno, quindi
    /// non possono coincidere con un identificativo originale.
    fn new<'a>(preferiti: impl IntoIterator<Item = Option<&'a str>>) -> Self {
        let massimo = preferiti
            .into_iter()
            .flatten()
            .filter_map(testo::intero)
            .max()
            .unwrap_or(0);
        Self {
            usati: HashSet::new(),
            prossimo: massimo.saturating_add(1),
        }
    }

    fn assegna(&mut self, preferito: Option<&str>) -> String {
        if let Some(p) = preferito.filter(|p| !p.trim().is_empty())
            && self.usati.insert(p.to_owned())
        {
            return p.to_owned();
        }
        loop {
            let candidato = self.prossimo.to_string();
            self.prossimo = self.prossimo.saturating_add(1);
            if self.usati.insert(candidato.clone()) {
                return candidato;
            }
        }
    }
}

struct Anomalie {
    elenco: Vec<Anomalia>,
}

impl Anomalie {
    fn aggiungi(&mut self, gravita: Gravita, codice: &str, messaggio: impl Into<String>) {
        self.elenco.push(Anomalia {
            gravita,
            codice: codice.into(),
            messaggio: messaggio.into(),
            posizione: None,
        });
    }

    fn fine(self) -> Vec<Anomalia> {
        self.elenco
    }
}

/// Voce di elenco da scrivere: definita nell'elenco prezzi o aggiunta per un
/// prezzo applicato non allineato.
enum VoceEp<'a> {
    Elenco(&'a VocePrezzario),
    Applicato(&'a PrezzoApplicato),
}

struct Piano<'a> {
    progetto: &'a Progetto,
    computo: &'a Computo,
    doc: Option<&'a Dati>,
    capitoli: HashMap<CapitoloId, (usize, String)>,
    classi: HashMap<ClasseId, String>,
    voci_ep: Vec<(String, VoceEp<'a>)>,
    id_ep_di_voce: HashMap<VocePrezzarioId, String>,
    idep_vc: Vec<String>,
    id_vc: Vec<String>,
    id_rg: HashMap<RigoMisuraId, String>,
    /// Quantità delle voci, per i «vedi voce».
    riferite: QuantitaComputo,
    anomalie: Anomalie,
}

fn livello_capitolo(doc: Option<&Dati>, id: CapitoloId) -> usize {
    conservati::valore(doc, &format!("capitolo:{id}/_livello"))
        .and_then(|l| l.parse::<usize>().ok())
        .filter(|l| *l < st::ELENCHI_CAPITOLI.len())
        .unwrap_or(1)
}

fn stesso_prezzo(a: &PrezzoApplicato, v: &VocePrezzario) -> bool {
    a.codice == v.codice
        && a.descrizione_breve == v.descrizione_breve
        && a.descrizione_estesa == v.descrizione_estesa
        && a.unita_misura == v.unita_misura
        && a.prezzo == v.prezzo
        && a.natura == v.natura
        && a.incidenza_manodopera == v.incidenza_manodopera
        && a.incidenza_sicurezza == v.incidenza_sicurezza
}

/// Esporta un computo del progetto.
///
/// # Errors
/// [`ErroreXpwe::ComputoInesistente`] o [`ErroreXpwe::Calcolo`] se il computo non
/// è calcolabile (l'export scrive quantità ricalcolate e non può farlo su dati
/// incoerenti).
pub fn esporta(progetto: &Progetto, computo: ComputoId) -> Result<RisultatoExport, ErroreXpwe> {
    let c = progetto
        .computo(computo)
        .ok_or_else(|| ErroreXpwe::ComputoInesistente(computo.to_string()))?;
    progetto
        .calcola_computo(computo)
        .map_err(|e| ErroreXpwe::Calcolo(e.to_string()))?;
    let doc = dati_xpwe(progetto.provenienza.as_ref());
    let mut piano = Piano {
        progetto,
        computo: c,
        doc,
        capitoli: HashMap::new(),
        classi: HashMap::new(),
        voci_ep: Vec::new(),
        id_ep_di_voce: HashMap::new(),
        idep_vc: Vec::new(),
        id_vc: Vec::new(),
        id_rg: HashMap::new(),
        riferite: c.quantita_riferite(&progetto.regole),
        anomalie: Anomalie { elenco: Vec::new() },
    };
    piano.anomalie.aggiungi(
        Gravita::Informazione,
        "xpwe.export_non_verificato",
        "testata, dati generali e configurazione verificati su un export di PriMus-DCF; voci di elenco, voci di computo e righi ancora dedotti da fonti secondarie e non verificati (vedi ADR 0006)",
    );
    piano.assegna_identificativi();
    piano.segnala_dati_non_esportati();
    let mut s = Scrittore::new();
    piano.scrivi_documento(&mut s)?;
    Ok(RisultatoExport {
        xml: s.fine(),
        anomalie: piano.anomalie.fine(),
    })
}

impl<'a> Piano<'a> {
    fn assegna_identificativi(&mut self) {
        let p = self.progetto;
        let doc = self.doc;

        // Capitoli, per livello.
        for livello in 0..st::ELENCHI_CAPITOLI.len() {
            let del_livello: Vec<_> = p
                .elenco_prezzi
                .capitoli
                .iter()
                .filter(|c| livello_capitolo(doc, c.id) == livello)
                .collect();
            let preferiti: Vec<Option<&str>> = del_livello
                .iter()
                .map(|c| conservati::valore(doc, &format!("capitolo:{}/_id", c.id)))
                .collect();
            let mut a = Assegnatore::new(preferiti.iter().copied());
            for (c, pref) in del_livello.iter().zip(preferiti) {
                self.capitoli.insert(c.id, (livello, a.assegna(pref)));
            }
        }
        if p.elenco_prezzi.capitoli.iter().any(|c| c.padre.is_some()) {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.capitoli_appiattiti",
                "la gerarchia dei capitoli non è rappresentabile: i capitoli sono esportati come elenchi piatti",
            );
        }

        // Categorie, per elenco del modello.
        for elenco in [
            &p.categorie.super_categorie,
            &p.categorie.categorie,
            &p.categorie.sotto_categorie,
        ] {
            let preferiti: Vec<Option<&str>> = elenco
                .iter()
                .map(|c| conservati::valore(doc, &format!("classe:{}/_id", c.id)))
                .collect();
            let mut a = Assegnatore::new(preferiti.iter().copied());
            for (c, pref) in elenco.iter().zip(preferiti) {
                self.classi.insert(c.id, a.assegna(pref));
            }
        }

        // Voci di elenco: prima quelle dell'elenco prezzi, poi quelle aggiunte.
        let preferiti: Vec<Option<&str>> = p
            .elenco_prezzi
            .voci
            .iter()
            .map(|v| {
                v.provenienza
                    .as_ref()
                    .filter(|x| x.formato == FormatoOrigine::Xpwe)
                    .and_then(|x| x.identificativo_originale.as_deref())
            })
            .collect();
        let mut a_ep = Assegnatore::new(preferiti.iter().copied());
        for (v, pref) in p.elenco_prezzi.voci.iter().zip(preferiti) {
            let id = a_ep.assegna(pref);
            self.id_ep_di_voce.insert(v.id, id.clone());
            self.voci_ep.push((id, VoceEp::Elenco(v)));
        }
        let mut aggiunte: Vec<(String, &PrezzoApplicato)> = Vec::new();
        for voce in &self.computo.voci {
            let allineata = voce
                .prezzo
                .origine
                .filter(|o| o.prezzario == p.elenco_prezzi.id)
                .and_then(|o| p.elenco_prezzi.voce(o.voce))
                .filter(|v| stesso_prezzo(&voce.prezzo, v))
                .and_then(|v| self.id_ep_di_voce.get(&v.id).cloned());
            let id = match allineata {
                Some(id) => id,
                None => {
                    if let Some((id, _)) = aggiunte.iter().find(|(_, x)| **x == voce.prezzo) {
                        id.clone()
                    } else {
                        let id = a_ep.assegna(None);
                        self.anomalie.aggiungi(
                            Gravita::Avviso,
                            "xpwe.prezzo_non_allineato",
                            format!(
                                "il prezzo applicato «{}» non coincide con una voce dell'elenco prezzi: esportata una voce di elenco aggiuntiva (ID {id})",
                                voce.prezzo.codice
                            ),
                        );
                        aggiunte.push((id.clone(), &voce.prezzo));
                        id
                    }
                }
            };
            self.idep_vc.push(id);
        }
        for (id, prezzo) in aggiunte {
            self.voci_ep.push((id, VoceEp::Applicato(prezzo)));
        }

        // Voci di computo e righi.
        let preferiti: Vec<Option<&str>> = self
            .computo
            .voci
            .iter()
            .map(|v| {
                v.provenienza
                    .as_ref()
                    .filter(|x| x.formato == FormatoOrigine::Xpwe)
                    .and_then(|x| x.identificativo_originale.as_deref())
            })
            .collect();
        let mut a_vc = Assegnatore::new(preferiti.iter().copied());
        for pref in preferiti {
            self.id_vc.push(a_vc.assegna(pref));
        }
        let mut righi_pref = Vec::new();
        for v in &self.computo.voci {
            let d = dati_xpwe(v.provenienza.as_ref());
            for r in &v.righi {
                righi_pref.push((r.id, conservati::valore(d, &format!("rigo:{}/_id", r.id))));
            }
        }
        let mut a_rg = Assegnatore::new(righi_pref.iter().map(|(_, p)| *p));
        for (id, pref) in righi_pref {
            self.id_rg.insert(id, a_rg.assegna(pref));
        }
    }

    fn segnala_dati_non_esportati(&mut self) {
        let p = self.progetto;
        if !p.contabilita.libretto().registrazioni.is_empty() || !p.contabilita.sal().is_empty() {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.contabilita_non_esportata",
                "libretto, registro e SAL non sono esportati: l'export riguarda il solo computo",
            );
        }
        if self.computo.voci.iter().any(|v| v.wbs.is_some()) {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.wbs_non_esportata",
                "l'assegnazione delle voci alla WBS non è esportata",
            );
        }
        if self.computo.voci.iter().any(|v| v.note.is_some()) {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.note_non_esportate",
                "le note delle voci di computo non sono esportate",
            );
        }
        let vedi_voce = self
            .computo
            .voci
            .iter()
            .flat_map(|v| {
                let d = dati_xpwe(v.provenienza.as_ref());
                v.righi.iter().filter(move |r| {
                    r.tipo == TipoRigo::Misura
                        && r.vedi_voce.is_some()
                        && conservati::testo(d, &format!("rigo:{}/", r.id), "IDVV").is_none()
                })
            })
            .count();
        if vedi_voce > 0 {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.vedi_voce_esportato",
                format!(
                    "{vedi_voce} righi «vedi voce» inseriti in OpenComputo: esportati con la quantità citata nei campi di misura, senza collegamento (la convenzione di PriMus per i «vedi voce» non è verificata)"
                ),
            );
        }
        let con_formula = self
            .computo
            .voci
            .iter()
            .flat_map(|v| &v.righi)
            .filter(|r| {
                r.tipo == TipoRigo::Misura
                    && r.formula.as_deref().is_some_and(|f| !f.trim().is_empty())
            })
            .count();
        if con_formula > 0 {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.formula_riscritta",
                format!(
                    "{con_formula} righi con formula: XPWE non ha un campo formula, la formula è scritta come espressione numerica nel campo «parti uguali» (la quantità non cambia, la formula non è più modificabile come tale)"
                ),
            );
        }
        if !p.elenco_prezzi.analisi.is_empty() {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.analisi_non_esportate",
                "le analisi prezzi del modello non sono esportate: le voci riportano il solo prezzo",
            );
        }
        let m = &p.metadati;
        if m.cig.is_some()
            || m.cup.is_some()
            || m.responsabile_procedimento.is_some()
            || m.progettista.is_some()
            || m.direttore_lavori.is_some()
            || m.data.is_some()
            || m.note.is_some()
        {
            self.anomalie.aggiungi(
                Gravita::Informazione,
                "xpwe.metadati_non_esportati",
                "CIG, CUP, figure professionali, data e note del progetto non sono esportati",
            );
        }
    }

    fn nome(&self, chiave: &str, predefinito: &'static str) -> String {
        conservati::valore(self.doc, chiave).map_or_else(|| predefinito.to_owned(), str::to_owned)
    }

    fn scrivi_documento(&self, s: &mut Scrittore) -> Result<(), ErroreXpwe> {
        let radice = self.nome("doc:_nome", st::RADICE);
        let attributi = conservati::attributi(self.doc, "doc:");
        let attr: Vec<(&str, &str)> = attributi
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        s.apri(&radice, &attr);
        let doc = self.doc;
        let mut predefinito: Vec<&str> = st::TESTATA.to_vec();
        predefinito.extend([st::DATI_GENERALI, st::MISURAZIONI]);
        conservati::scrivi_figli(
            s,
            doc,
            "doc:",
            &predefinito,
            &[st::DATI_GENERALI, st::MISURAZIONI],
            |s, nome| match nome {
                st::DATI_GENERALI => {
                    self.scrivi_dati_generali(s);
                    true
                }
                st::MISURAZIONI => {
                    self.scrivi_misurazioni(s);
                    true
                }
                // Documento nuovo: testata come nell'export osservato di PriMus-DCF.
                _ if doc.is_none() => {
                    let valore = match nome {
                        "CopyRight" => "",
                        "TipoDocumento" => "1",
                        "TipoFormato" => "XMLPwe",
                        "Versione" => st::VERSIONE_FORMATO,
                        "SourceVersione" => env!("CARGO_PKG_VERSION"),
                        "SourceNome" => "OpenComputo",
                        _ => return false,
                    };
                    s.campo(nome, valore);
                    true
                }
                _ => false,
            },
        );
        s.chiudi(&radice);
        Ok(())
    }

    fn apri_con_attributi(&self, s: &mut Scrittore, nome: &str, prefisso: &str) {
        let attributi = conservati::attributi(self.doc, prefisso);
        let attr: Vec<(&str, &str)> = attributi
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        s.apri(nome, &attr);
    }

    fn scrivi_dati_generali(&self, s: &mut Scrittore) {
        let doc = self.doc;
        self.apri_con_attributi(s, st::DATI_GENERALI, "dg:");
        let nome_prog = self.nome("dg:prog/_nome", st::DG_PROGETTO);
        let nome_cfg_numeri = self.nome("cfg:num/_nome", st::DG_CONFIG_NUMERI);
        let obbligatori = vec![
            nome_prog.as_str(),
            st::DG_CAPITOLI_CATEGORIE,
            st::DG_CONFIGURAZIONE,
        ];
        conservati::scrivi_figli(
            s,
            doc,
            "dg:",
            &[
                st::DG_PROGETTO,
                st::DG_CAPITOLI_CATEGORIE,
                st::DG_CONFIGURAZIONE,
            ],
            &obbligatori,
            |s, nome| {
                if nome == nome_prog {
                    self.scrivi_anagrafica(s, &nome_prog);
                    true
                } else if nome == st::DG_CAPITOLI_CATEGORIE {
                    self.scrivi_capitoli_categorie(s);
                    true
                } else if nome == st::DG_CONFIGURAZIONE {
                    self.scrivi_configurazione(s, &nome_cfg_numeri);
                    true
                } else {
                    false
                }
            },
        );
        s.chiudi(st::DATI_GENERALI);
    }

    fn scrivi_anagrafica(&self, s: &mut Scrittore, nome_prog: &str) {
        let doc = self.doc;
        let m = &self.progetto.metadati;
        let nome_anag = self.nome("dg:anag/_nome", st::DG_DATI_GENERALI);
        self.apri_con_attributi(s, nome_prog, "dg:prog/");
        conservati::scrivi_figli(
            s,
            doc,
            "dg:prog/",
            &[st::DG_DATI_GENERALI],
            &[nome_anag.as_str()],
            |s, nome| {
                if nome != nome_anag {
                    return false;
                }
                self.apri_con_attributi(s, &nome_anag, "dg:anag/");
                let oggetto = m.oggetto.clone().unwrap_or_else(|| m.titolo.clone());
                let valori: [(&str, Option<&str>); 4] = [
                    ("Comune", m.luogo.as_deref()),
                    ("Oggetto", Some(oggetto.as_str())),
                    ("Committente", m.committente.as_deref()),
                    ("Impresa", m.impresa.as_deref()),
                ];
                let obbligatori: Vec<&str> = valori
                    .iter()
                    .filter(|(_, v)| v.is_some_and(|t| !t.is_empty()))
                    .map(|(n, _)| *n)
                    .collect();
                conservati::scrivi_figli(
                    s,
                    doc,
                    "dg:anag/",
                    &st::ORDINE_DATI_GENERALI,
                    &obbligatori,
                    |s, campo| match valori.iter().find(|(n, _)| *n == campo) {
                        Some((_, v)) => {
                            s.campo(campo, v.unwrap_or_default());
                            true
                        }
                        // Documento nuovo: campi presenti nell'export osservato.
                        None if doc.is_none() => {
                            let valore = match campo {
                                "PercPrezzi" => "0",
                                "Provincia" | "ParteOpera" => "",
                                _ => return false,
                            };
                            s.campo(campo, valore);
                            true
                        }
                        None => false,
                    },
                );
                s.chiudi(&nome_anag);
                true
            },
        );
        s.chiudi(nome_prog);
    }

    fn scrivi_capitoli_categorie(&self, s: &mut Scrittore) {
        let doc = self.doc;
        let p = self.progetto;
        self.apri_con_attributi(s, st::DG_CAPITOLI_CATEGORIE, "cc:");
        let tutti: Vec<&str> = st::ELENCHI_CAPITOLI
            .iter()
            .chain(st::ELENCHI_CATEGORIE.iter())
            .map(|(n, _)| *n)
            .collect();
        let con_contenuto: Vec<&str> = st::ELENCHI_CAPITOLI
            .iter()
            .enumerate()
            .filter(|(l, _)| self.capitoli.values().any(|(lv, _)| lv == l))
            .map(|(_, (n, _))| *n)
            .chain(
                st::ELENCHI_CATEGORIE
                    .iter()
                    .zip([
                        &p.categorie.super_categorie,
                        &p.categorie.categorie,
                        &p.categorie.sotto_categorie,
                    ])
                    .filter(|(_, e)| !e.is_empty())
                    .map(|((n, _), _)| *n),
            )
            .collect();
        conservati::scrivi_figli(s, doc, "cc:", &tutti, &con_contenuto, |s, nome| {
            if let Some(l) = st::ELENCHI_CAPITOLI.iter().position(|(n, _)| *n == nome) {
                let elemento = st::ELENCHI_CAPITOLI.get(l).map_or("", |(_, e)| *e);
                let voci: Vec<(String, String, String, String)> = p
                    .elenco_prezzi
                    .capitoli
                    .iter()
                    .filter_map(|c| {
                        let (lv, id) = self.capitoli.get(&c.id)?;
                        (*lv == l).then(|| {
                            (
                                format!("capitolo:{}/", c.id),
                                id.clone(),
                                c.codice.clone(),
                                c.descrizione.clone(),
                            )
                        })
                    })
                    .collect();
                self.scrivi_elenco_classi(s, nome, elemento, &voci);
                return true;
            }
            if let Some(l) = st::ELENCHI_CATEGORIE.iter().position(|(n, _)| *n == nome) {
                let elemento = st::ELENCHI_CATEGORIE.get(l).map_or("", |(_, e)| *e);
                let elenco = match l {
                    0 => &p.categorie.super_categorie,
                    1 => &p.categorie.categorie,
                    _ => &p.categorie.sotto_categorie,
                };
                let voci: Vec<(String, String, String, String)> = elenco
                    .iter()
                    .map(|c| {
                        (
                            format!("classe:{}/", c.id),
                            self.classi.get(&c.id).cloned().unwrap_or_default(),
                            c.codice.clone(),
                            c.descrizione.clone(),
                        )
                    })
                    .collect();
                self.scrivi_elenco_classi(s, nome, elemento, &voci);
                return true;
            }
            false
        });
        s.chiudi(st::DG_CAPITOLI_CATEGORIE);
    }

    /// Scrive un elenco di capitoli o categorie: (prefisso, ID, codice, descrizione).
    fn scrivi_elenco_classi(
        &self,
        s: &mut Scrittore,
        nome_elenco: &str,
        elemento_predefinito: &str,
        voci: &[(String, String, String, String)],
    ) {
        let doc = self.doc;
        if voci.is_empty() && conservati::attributi(doc, &format!("cc:{nome_elenco}/")).is_empty() {
            s.campo(nome_elenco, "");
            return;
        }
        self.apri_con_attributi(s, nome_elenco, &format!("cc:{nome_elenco}/"));
        for (prefisso, id, codice, descrizione) in voci {
            let elemento = conservati::valore(doc, &format!("{prefisso}_elemento"))
                .unwrap_or(elemento_predefinito)
                .to_owned();
            let mut attributi = vec![("ID".to_owned(), id.clone())];
            attributi.extend(conservati::attributi(doc, prefisso));
            let attr: Vec<(&str, &str)> = attributi
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            s.apri(&elemento, &attr);
            conservati::scrivi_figli(
                s,
                doc,
                prefisso,
                &st::ORDINE_CLASSE[..2],
                &["Codice", "DesSintetica"],
                |s, campo| match campo {
                    "Codice" => {
                        s.campo(campo, codice);
                        true
                    }
                    "DesSintetica" => {
                        s.campo(campo, descrizione);
                        true
                    }
                    _ => false,
                },
            );
            s.chiudi(&elemento);
        }
        s.chiudi(nome_elenco);
    }

    fn scrivi_configurazione(&self, s: &mut Scrittore, nome_numeri: &str) {
        let doc = self.doc;
        let r = &self.progetto.regole;
        self.apri_con_attributi(s, st::DG_CONFIGURAZIONE, "cfg:");
        if conservati::valore(doc, "cfg:_ordine").is_none() {
            // Documento nuovo: configurazione osservata in PriMus-DCF con i decimali del progetto.
            s.apri(st::DG_CONFIG_NUMERI, &[]);
            for (campo, valore) in st::CONFIG_NUMERI_PREDEFINITA {
                let decimali = match campo {
                    "Quantita" => Some(r.decimali_quantita),
                    "Prezzi" => Some(r.decimali_prezzo),
                    "PrezziTotale" => Some(r.decimali_importo),
                    _ => None,
                };
                match decimali {
                    Some(d) => s.campo(campo, &testo::con_decimali_configurazione(Some(valore), d)),
                    None => s.campo(campo, valore),
                }
            }
            s.chiudi(st::DG_CONFIG_NUMERI);
            s.chiudi(st::DG_CONFIGURAZIONE);
            return;
        }
        conservati::scrivi_figli(s, doc, "cfg:", &[], &[], |s, nome| {
            if nome != nome_numeri {
                return false;
            }
            self.apri_con_attributi(s, nome_numeri, "cfg:num/");
            conservati::scrivi_figli(s, doc, "cfg:num/", &[], &[], |s, campo| {
                let decimali = match campo {
                    "Quantita" => r.decimali_quantita,
                    "Prezzi" => r.decimali_prezzo,
                    "PrezziTotale" => r.decimali_importo,
                    _ => return false,
                };
                let originale = conservati::testo(doc, "cfg:num/", campo);
                s.campo(
                    campo,
                    &testo::con_decimali_configurazione(originale, decimali),
                );
                true
            });
            s.chiudi(nome_numeri);
            true
        });
        s.chiudi(st::DG_CONFIGURAZIONE);
    }

    fn scrivi_elenco(&self, s: &mut Scrittore, nome_ep: &str) {
        let doc = self.doc;
        self.apri_con_attributi(s, nome_ep, "mis:ep/");
        conservati::scrivi_figli(s, doc, "mis:ep/", &[st::EP_ITEM], &[st::EP_ITEM], |s, n| {
            if n != st::EP_ITEM {
                return false;
            }
            for (id, voce) in &self.voci_ep {
                self.scrivi_voce_elenco(s, id, voce);
            }
            true
        });
        s.chiudi(nome_ep);
    }

    fn scrivi_misurazioni(&self, s: &mut Scrittore) {
        let doc = self.doc;
        let nome_ep = self.nome("mis:ep/_nome", st::ELENCO_PREZZI);
        let nome_vc = self.nome("mis:vc/_nome", st::VOCI_COMPUTO);
        self.apri_con_attributi(s, st::MISURAZIONI, "mis:");
        // PriMus omette l'elenco prezzi vuoto (verificato). Se l'elenco va scritto ma
        // il file di origine non lo aveva, è scritto prima delle voci di computo.
        let ordine_originale = conservati::valore(doc, "mis:_ordine");
        let ep_nell_ordine = ordine_originale.is_some_and(|o| o.split('|').any(|k| k == nome_ep));
        let mut ep_da_scrivere = !self.voci_ep.is_empty();
        if ep_da_scrivere && ordine_originale.is_some() && !ep_nell_ordine {
            self.scrivi_elenco(s, &nome_ep);
            ep_da_scrivere = false;
        }
        let mut obbligatori = vec![nome_vc.as_str()];
        if ep_da_scrivere {
            obbligatori.insert(0, nome_ep.as_str());
        }
        conservati::scrivi_figli(
            s,
            doc,
            "mis:",
            &[st::ELENCO_PREZZI, st::VOCI_COMPUTO],
            &obbligatori,
            |s, nome| {
                if nome == nome_ep && !ep_da_scrivere && !ep_nell_ordine {
                    true
                } else if nome == nome_ep {
                    self.scrivi_elenco(s, &nome_ep);
                    true
                } else if nome == nome_vc {
                    self.apri_con_attributi(s, &nome_vc, "mis:vc/");
                    conservati::scrivi_figli(
                        s,
                        doc,
                        "mis:vc/",
                        &[st::VC_ITEM],
                        &[st::VC_ITEM],
                        |s, n| {
                            if n != st::VC_ITEM {
                                return false;
                            }
                            for (i, voce) in self.computo.voci.iter().enumerate() {
                                self.scrivi_voce_computo(s, i, voce);
                            }
                            true
                        },
                    );
                    s.chiudi(&nome_vc);
                    true
                } else {
                    false
                }
            },
        );
        s.chiudi(st::MISURAZIONI);
    }

    fn scrivi_voce_elenco(&self, s: &mut Scrittore, id: &str, voce: &VoceEp<'_>) {
        let (dati, codice, breve, estesa, um, prezzo, natura, mdo, sic, capitolo) = match voce {
            VoceEp::Elenco(v) => (
                dati_xpwe(v.provenienza.as_ref()),
                &v.codice,
                &v.descrizione_breve,
                &v.descrizione_estesa,
                &v.unita_misura,
                v.prezzo,
                v.natura,
                v.incidenza_manodopera,
                v.incidenza_sicurezza,
                v.capitolo,
            ),
            VoceEp::Applicato(a) => (
                None,
                &a.codice,
                &a.descrizione_breve,
                &a.descrizione_estesa,
                &a.unita_misura,
                a.prezzo,
                a.natura,
                a.incidenza_manodopera,
                a.incidenza_sicurezza,
                None,
            ),
        };
        let mut attributi = vec![("ID".to_owned(), id.to_owned())];
        attributi.extend(conservati::attributi(dati, ""));
        let attr: Vec<(&str, &str)> = attributi
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        s.apri(st::EP_ITEM, &attr);
        let breve_da = conservati::valore(dati, "_breve_da").unwrap_or("DesBreve");
        let capitolo_esportato = capitolo.and_then(|c| self.capitoli.get(&c));
        conservati::scrivi_figli(
            s,
            dati,
            "",
            &st::ORDINE_EP,
            &["Tariffa", "DesEstesa", "UnMisura", "Prezzo1"],
            |s, campo| {
                let valore: String = match campo {
                    "Tariffa" => codice.clone(),
                    "DesRidotta" | "DesBreve" if dati.is_none() || campo == breve_da => {
                        breve.clone()
                    }
                    "DesEstesa" => estesa.clone(),
                    "UnMisura" => um.clone(),
                    "Prezzo1" => prezzo.to_string(),
                    "Flags" => {
                        let base = conservati::testo(dati, "", "Flags")
                            .and_then(testo::intero)
                            .unwrap_or(0);
                        let flags = match natura {
                            NaturaVoce::OneriSicurezza => base | st::FLAG_VOCE_SICUREZZA,
                            NaturaVoce::Lavoro => base & !st::FLAG_VOCE_SICUREZZA,
                        };
                        flags.to_string()
                    }
                    "IncMDO" => mdo.map_or_else(|| "0".to_owned(), |p| p.to_string()),
                    "IncSIC" => sic.map_or_else(|| "0".to_owned(), |p| p.to_string()),
                    "IDSpCap" | "IDCap" | "IDSbCap" => {
                        let livello = match campo {
                            "IDSpCap" => 0,
                            "IDCap" => 1,
                            _ => 2,
                        };
                        match capitolo_esportato {
                            Some((l, id)) if *l == livello => id.clone(),
                            _ => conservati::testo(dati, "", campo).unwrap_or("0").to_owned(),
                        }
                    }
                    _ => return false,
                };
                s.campo(campo, &valore);
                true
            },
        );
        s.chiudi(st::EP_ITEM);
    }

    fn scrivi_voce_computo(&self, s: &mut Scrittore, indice: usize, voce: &VoceComputo) {
        let dati = dati_xpwe(voce.provenienza.as_ref());
        let regole = &self.progetto.regole;
        let id = self.id_vc.get(indice).cloned().unwrap_or_default();
        let idep = self.idep_vc.get(indice).cloned().unwrap_or_default();
        let quantita = voce
            .calcola_in(regole, &self.riferite)
            .map(|c| c.quantita.to_string())
            .unwrap_or_default();
        let nome_misure = conservati::valore(dati, "_misure").unwrap_or(st::VC_MISURE);
        let mut attributi = vec![("ID".to_owned(), id)];
        attributi.extend(conservati::attributi(dati, ""));
        let attr: Vec<(&str, &str)> = attributi
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        s.apri(st::VC_ITEM, &attr);
        let mut predefinito: Vec<&str> = st::ORDINE_VC.to_vec();
        if let Some(ultimo) = predefinito.last_mut() {
            *ultimo = nome_misure;
        }
        conservati::scrivi_figli(
            s,
            dati,
            "",
            &predefinito,
            &["IDEP", "Quantita", nome_misure],
            |s, campo| {
                let valore: String = match campo {
                    "IDEP" => idep.clone(),
                    "Quantita" => quantita.clone(),
                    "IDSpCat" | "IDCat" | "IDSbCat" => {
                        let assegnata = match campo {
                            "IDSpCat" => voce.categorie.super_categoria,
                            "IDCat" => voce.categorie.categoria,
                            _ => voce.categorie.sotto_categoria,
                        };
                        match assegnata.and_then(|c| self.classi.get(&c)) {
                            Some(id) => id.clone(),
                            None => conservati::testo(dati, "", campo).unwrap_or("0").to_owned(),
                        }
                    }
                    n if n == nome_misure => {
                        s.apri(nome_misure, &[]);
                        conservati::scrivi_figli(
                            s,
                            dati,
                            "misure/",
                            &[st::RG_ITEM],
                            &[st::RG_ITEM],
                            |s, n| {
                                if n != st::RG_ITEM {
                                    return false;
                                }
                                for rigo in &voce.righi {
                                    self.scrivi_rigo(s, dati, rigo);
                                }
                                true
                            },
                        );
                        s.chiudi(nome_misure);
                        return true;
                    }
                    _ => return false,
                };
                s.campo(campo, &valore);
                true
            },
        );
        s.chiudi(st::VC_ITEM);
    }

    fn scrivi_rigo(&self, s: &mut Scrittore, dati: Option<&Dati>, rigo: &RigoMisura) {
        let regole = &self.progetto.regole;
        let prefisso = format!("rigo:{}/", rigo.id);
        let id = self.id_rg.get(&rigo.id).cloned().unwrap_or_default();
        let mut attributi = vec![("ID".to_owned(), id)];
        attributi.extend(conservati::attributi(dati, &prefisso));
        let attr: Vec<(&str, &str)> = attributi
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        // Righi «vedi voce» e «parziali»: i campi di misura originali sono riscritti com'erano.
        let originali = conservati::testo(dati, &prefisso, "PartiUguali").is_some();
        let campi = if originali {
            None
        } else {
            Some(self.campi_rigo(rigo))
        };
        let quantita = match rigo.calcola_con(regole, &self.riferite) {
            Ok(Some(r)) => r.quantita.to_string(),
            _ => conservati::testo(dati, &prefisso, "Quantita")
                .unwrap_or("0")
                .to_owned(),
        };
        s.apri(st::RG_ITEM, &attr);
        conservati::scrivi_figli(
            s,
            dati,
            &prefisso,
            &st::ORDINE_RG,
            &st::ORDINE_RG,
            |s, campo| {
                let valore: String = match campo {
                    // Un rigo collegato del file cita la voce con l'ID assegnato ora.
                    "IDVV" => match (originali, rigo.vedi_voce.as_ref().and_then(VediVoce::voce)) {
                        (true, Some(id)) => self.id_di_voce(id),
                        _ => None,
                    }
                    .or_else(|| conservati::testo(dati, &prefisso, "IDVV").map(str::to_owned))
                    .unwrap_or_else(|| st::IDVV_NESSUNO.to_owned()),
                    "Descrizione" => rigo.descrizione.clone(),
                    "PartiUguali" | "Lunghezza" | "Larghezza" | "HPeso" => match &campi {
                        None => conservati::testo(dati, &prefisso, campo)
                            .unwrap_or_default()
                            .to_owned(),
                        Some(c) => {
                            let i = match campo {
                                "PartiUguali" => 0,
                                "Lunghezza" => 1,
                                "Larghezza" => 2,
                                _ => 3,
                            };
                            c.get(i).cloned().unwrap_or_default()
                        }
                    },
                    "Quantita" => quantita.clone(),
                    "Flags" => conservati::testo(dati, &prefisso, "Flags")
                        .unwrap_or("0")
                        .to_owned(),
                    _ => return false,
                };
                s.campo(campo, &valore);
                true
            },
        );
        s.chiudi(st::RG_ITEM);
    }

    /// ID di esportazione di una voce del computo.
    fn id_di_voce(&self, id: VoceComputoId) -> Option<String> {
        let i = self.computo.voci.iter().position(|v| v.id == id)?;
        self.id_vc.get(i).cloned()
    }

    /// Testi dei quattro campi di misura con segno e formula applicati. La quantità
    /// citata da un «vedi voce» entra come fattore (o al posto di `vv` nella formula).
    fn campi_rigo(&self, rigo: &RigoMisura) -> [String; 4] {
        if rigo.tipo == TipoRigo::Commento {
            return Default::default();
        }
        let fattori = [
            rigo.parti_uguali.as_ref(),
            rigo.lunghezza.as_ref(),
            rigo.larghezza.as_ref(),
            rigo.altezza_peso.as_ref(),
        ];
        let citata = match &rigo.vedi_voce {
            Some(VediVoce::Statica { quantita, .. }) => Some(Fattore::Valore(*quantita)),
            Some(VediVoce::Collegata(id)) => self.riferite.quantita(*id).ok().map(Fattore::Valore),
            None => None,
        };
        let mut campi: [Option<String>; 4] = fattori.map(|f| f.map(testo::testo_fattore));
        if let Some(formula) = rigo.formula.as_deref().filter(|f| !f.trim().is_empty()) {
            // XPWE non ha un campo formula: la formula è riscritta come espressione
            // numerica nel campo «parti uguali», con le variabili sostituite.
            let espressione = sostituisci_variabili(formula, &fattori, citata.as_ref());
            campi = [Some(espressione), None, None, None];
        } else if let Some(q) = &citata
            && let Some(primo) = campi.first_mut()
        {
            let q = testo::testo_fattore(q);
            *primo = Some(match primo.take() {
                Some(par) => format!("{q}*({par})"),
                None => q,
            });
        }
        if rigo.segno == SegnoRigo::Deduzione
            && let Some(primo) = campi.iter_mut().flatten().next()
        {
            *primo = nega(primo);
        }
        campi.map(Option::unwrap_or_default)
    }
}

fn nega(testo: &str) -> String {
    let t = testo.trim();
    let numerico = t
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',');
    if numerico {
        format!("-{t}")
    } else {
        format!("-({t})")
    }
}

/// Sostituisce `par`, `lun`, `lar`, `alt` (e `vv`, la quantità citata) con i valori
/// tra parentesi.
fn sostituisci_variabili(
    formula: &str,
    fattori: &[Option<&Fattore>; 4],
    citata: Option<&Fattore>,
) -> String {
    let mut out = String::with_capacity(formula.len() * 2);
    let mut parola = String::new();
    let svuota = |parola: &mut String, out: &mut String| {
        if parola.is_empty() {
            return;
        }
        let minuscola = parola.to_lowercase();
        let posizione = if minuscola == VARIABILE_VEDI_VOCE {
            Some(VARIABILI_RIGO.len())
        } else {
            VARIABILI_RIGO.iter().position(|v| *v == minuscola)
        };
        match posizione {
            Some(i) => {
                let valore = fattori
                    .get(i)
                    .copied()
                    .flatten()
                    .or(citata.filter(|_| i == VARIABILI_RIGO.len()))
                    .map(testo::testo_fattore)
                    .unwrap_or_else(|| "0".into());
                out.push('(');
                out.push_str(&valore);
                out.push(')');
            }
            None => out.push_str(parola),
        }
        parola.clear();
    };
    for c in formula.chars() {
        if c.is_alphanumeric() || c == '_' {
            if parola.is_empty() && c.is_ascii_digit() {
                out.push(c);
            } else {
                parola.push(c);
            }
        } else {
            svuota(&mut parola, &mut out);
            out.push(c);
        }
    }
    svuota(&mut parola, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn sostituzione_delle_variabili() {
        let l = Fattore::Valore(dec!(6.00));
        let b = Fattore::Espressione("2+1".into());
        let f = [None, Some(&l), Some(&b), None];
        assert_eq!(
            sostituisci_variabili("0,5*LUN*lar + max(lun; 2)", &f, None),
            "0,5*(6.00)*(2+1) + max((6.00); 2)"
        );
        assert_eq!(nega("6"), "-6");
        assert_eq!(nega("2+3"), "-(2+3)");
    }

    #[test]
    fn sostituzione_della_quantita_citata() {
        let l = Fattore::Valore(dec!(2));
        let q = Fattore::Valore(dec!(126.00));
        let f = [None, Some(&l), None, None];
        assert_eq!(
            sostituisci_variabili("vv/lun", &f, Some(&q)),
            "(126.00)/(2)"
        );
        assert_eq!(sostituisci_variabili("vv*2", &f, None), "(0)*2");
    }

    #[test]
    fn assegnatore_riusa_e_numera() {
        let mut a = Assegnatore::new([Some("5"), Some("5"), Some("x"), None]);
        assert_eq!(a.assegna(Some("5")), "5");
        assert_eq!(a.assegna(Some("5")), "6");
        assert_eq!(a.assegna(Some("x")), "x");
        assert_eq!(a.assegna(None), "7");
    }
}
