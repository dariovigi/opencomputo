//! Import di un documento XPWE in un [`Progetto`].
//!
//! Principi:
//!
//! * nessun dato viene perso in silenzio: ciò che non è mappato sul modello è
//!   conservato nella provenienza e, se sconosciuto, segnalato;
//! * un'entità con dati essenziali non validi (es. voce di elenco senza prezzo,
//!   voce di computo con `IDEP` non risolvibile) non viene importata e produce
//!   un'anomalia di gravità «errore» con la posizione nel file;
//! * le quantità dichiarate nel file sono confrontate con quelle ricalcolate;
//! * gli identificativi sono generati dal [`GeneratoreId`] del chiamante, in ordine
//!   di documento: lo stesso file con lo stesso generatore produce lo stesso progetto.

use std::collections::{BTreeMap, HashMap};

use jiff::civil::DateTime;
use opencomputo_core::analisi::{AnalisiPrezzo, BaseUtile, OrigineRigaAnalisi, RigaAnalisi};
use opencomputo_core::classificazione::{AssegnazioneCategorie, Classe, LivelloCategoria};
use opencomputo_core::computo::{Computo, VoceComputo};
use opencomputo_core::id::{
    CapitoloId, ClasseId, ComputoId, GeneratoreId, RigoMisuraId, VoceComputoId, VocePrezzarioId,
};
use opencomputo_core::misura::{
    Fattore, RigoMisura, SegnoRigo, TipoRigo, VediVoce, quantita_totale,
};
use opencomputo_core::numeri::{Percentuale, Prezzo, Quantita, RegoleArrotondamento};
use opencomputo_core::partita::PrezzoApplicato;
use opencomputo_core::prezzario::{CapitoloPrezzario, NaturaVoce, Prezzario, VocePrezzario};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::provenienza::{Anomalia, FormatoOrigine, Provenienza};
use opencomputo_core::validazione::Gravita;
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};

use crate::analisi as deduzioni;
use crate::conservati::{self, Dati, Regole};
use crate::errore::ErroreXpwe;
use crate::struttura as st;
use crate::testo;
use crate::xml::{self, Nodo};

/// Opzioni di import.
#[derive(Debug, Clone)]
pub struct OpzioniImport {
    /// Nome del file di origine (per la provenienza e i messaggi).
    pub nome_file: Option<String>,
    /// Momento dell'importazione (il core non legge l'orologio di sistema).
    pub importato_il: Option<DateTime>,
    /// Nome del computo creato dalle voci di computo.
    pub nome_computo: String,
}

impl Default for OpzioniImport {
    fn default() -> Self {
        Self {
            nome_file: None,
            importato_il: None,
            nome_computo: "Computo metrico estimativo".into(),
        }
    }
}

/// Conteggi dell'import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statistiche {
    /// Voci di elenco prezzi importate.
    pub voci_elenco: usize,
    /// Voci di elenco prezzi scartate.
    pub voci_elenco_scartate: usize,
    /// Analisi dei prezzi importate.
    pub analisi: usize,
    /// Voci di computo importate.
    pub voci_computo: usize,
    /// Voci di computo scartate.
    pub voci_computo_scartate: usize,
    /// Righi di misura importati.
    pub righi: usize,
    /// Capitoli importati (tutti i livelli).
    pub capitoli: usize,
    /// Categorie importate (tutti i livelli).
    pub categorie: usize,
    /// Quantità del file confrontate con quelle ricalcolate.
    pub quantita_verificate: usize,
    /// Quantità del file diverse da quelle ricalcolate.
    pub quantita_diverse: usize,
}

/// Rapporto dell'import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportoImport {
    /// Anomalie, nell'ordine in cui sono state rilevate (quelle aggregate in coda).
    pub anomalie: Vec<Anomalia>,
    /// Conteggi.
    pub statistiche: Statistiche,
    /// Codifica dei caratteri usata.
    pub codifica: String,
}

impl RapportoImport {
    /// `true` se almeno un'anomalia è un errore (dati scartati o non calcolabili).
    #[must_use]
    pub fn ha_errori(&self) -> bool {
        self.anomalie.iter().any(|a| a.gravita == Gravita::Errore)
    }

    /// Anomalie con il codice indicato.
    pub fn con_codice<'a>(&'a self, codice: &'a str) -> impl Iterator<Item = &'a Anomalia> + 'a {
        self.anomalie.iter().filter(move |a| a.codice == codice)
    }
}

/// Risultato dell'import.
#[derive(Debug, Clone)]
pub struct RisultatoImport {
    /// Progetto importato.
    pub progetto: Progetto,
    /// Identificativo del computo creato.
    pub computo: ComputoId,
    /// Rapporto.
    pub rapporto: RapportoImport,
}

/// Anomalia aggregata: conteggio e prima occorrenza.
struct Aggregata {
    gravita: Gravita,
    messaggio: String,
    conteggio: usize,
    prima: String,
}

struct Contesto<'g, G: GeneratoreId> {
    generatore: &'g mut G,
    anomalie: Vec<Anomalia>,
    aggregate: BTreeMap<String, Aggregata>,
    statistiche: Statistiche,
    /// Voci di computo per ID del file (`None` se l'ID è duplicato).
    id_vc: HashMap<String, Option<VoceComputoId>>,
    /// Righi «vedi voce» importati statici, da collegare a fine computo.
    vedi_voce: Vec<VediVoceDaCollegare>,
}

/// Rigo «vedi voce» del file, da collegare alla voce citata se le quantità tornano.
struct VediVoceDaCollegare {
    rigo: RigoMisuraId,
    idvv: String,
    dichiarata: Option<Decimal>,
    fattori: [Option<Fattore>; 4],
    riga: usize,
    percorso: String,
}

fn posizione(riga: usize, percorso: &str) -> String {
    format!("riga {riga}, {percorso}")
}

impl<G: GeneratoreId> Contesto<'_, G> {
    fn anomalia(
        &mut self,
        gravita: Gravita,
        codice: &str,
        messaggio: impl Into<String>,
        riga: usize,
        percorso: &str,
    ) {
        self.anomalie.push(Anomalia {
            gravita,
            codice: codice.into(),
            messaggio: messaggio.into(),
            posizione: Some(posizione(riga, percorso)),
        });
    }

    /// Anomalia frequente: una sola voce nel rapporto con il numero di occorrenze.
    fn aggregata(
        &mut self,
        gravita: Gravita,
        codice: &str,
        chiave: &str,
        messaggio: impl Into<String>,
        riga: usize,
        percorso: &str,
    ) {
        let k = format!("{codice}\u{0}{chiave}");
        self.aggregate
            .entry(k)
            .and_modify(|a| a.conteggio += 1)
            .or_insert_with(|| Aggregata {
                gravita,
                messaggio: messaggio.into(),
                conteggio: 1,
                prima: posizione(riga, percorso),
            });
    }

    fn sconosciuti(&mut self, esito: conservati::Registrazione, percorso_tipo: &str) {
        for (nome, riga) in esito.sconosciuti {
            let base = conservati::nome_base(&nome).to_owned();
            let messaggio = if nome == base {
                format!("elemento «{base}» non interpretato: conservato e reinserito in export")
            } else {
                format!("elemento «{base}» ripetuto: conservato, il modello usa il primo")
            };
            self.aggregata(
                Gravita::Informazione,
                "xpwe.elemento_conservato",
                &format!("{percorso_tipo}/{base}"),
                messaggio,
                riga,
                &format!("{percorso_tipo}/{base}"),
            );
        }
    }

    fn chiudi(mut self) -> (Vec<Anomalia>, Statistiche) {
        for (k, a) in std::mem::take(&mut self.aggregate) {
            let codice = k.split('\u{0}').next().unwrap_or_default().to_owned();
            let messaggio = if a.conteggio > 1 {
                format!(
                    "{} ({} occorrenze, la prima in {})",
                    a.messaggio, a.conteggio, a.prima
                )
            } else {
                a.messaggio
            };
            self.anomalie.push(Anomalia {
                gravita: a.gravita,
                codice,
                messaggio,
                posizione: Some(a.prima),
            });
        }
        (self.anomalie, self.statistiche)
    }
}

fn testo_campo<'a>(nodo: &'a Nodo, nome: &str) -> &'a str {
    nodo.testo_di(nome).map_or("", str::trim)
}

/// Riferimento (identificativo XPWE) significativo: vuoto, `0` e `-1` indicano assenza.
fn riferimento(nodo: &Nodo, nome: &str) -> Option<String> {
    let t = testo_campo(nodo, nome);
    if t.is_empty() || t == "0" || t == "-1" {
        None
    } else {
        Some(t.to_owned())
    }
}

fn percentuale_facoltativa<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    nodo: &Nodo,
    nome: &str,
    percorso: &str,
) -> Option<Percentuale> {
    match testo::decimale(testo_campo(nodo, nome)) {
        Ok(Some(v)) if !v.is_zero() => Some(Percentuale::new(v)),
        Ok(_) => None,
        Err(e) => {
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.numero_non_valido",
                format!("{nome}: {e}; campo ignorato"),
                nodo.riga,
                percorso,
            );
            None
        }
    }
}

fn provenienza(id_originale: Option<String>, dati: Dati) -> Provenienza {
    let mut p = Provenienza::new(FormatoOrigine::Xpwe);
    p.identificativo_originale = id_originale;
    p.dati_originali = dati;
    p
}

/// Individua sezione dati generali e misurazioni, anche sotto un elemento intermedio.
fn sezioni(radice: &Nodo) -> (Option<&Nodo>, Option<&Nodo>, bool) {
    let dg = radice.figlio(st::DATI_GENERALI);
    let mis = radice.figlio(st::MISURAZIONI);
    if dg.is_some() || mis.is_some() {
        return (dg, mis, false);
    }
    for intermedio in &radice.figli {
        let dg = intermedio.figlio(st::DATI_GENERALI);
        let mis = intermedio.figlio(st::MISURAZIONI);
        if dg.is_some() || mis.is_some() {
            return (dg, mis, true);
        }
    }
    (None, None, false)
}

/// Importa un documento XPWE.
///
/// # Errors
/// Solo per file non leggibili come XML o privi della struttura XPWE; ogni altro
/// problema è un'anomalia nel [`RapportoImport`].
pub fn importa<G: GeneratoreId>(
    dati: &[u8],
    generatore: &mut G,
    opzioni: &OpzioniImport,
) -> Result<RisultatoImport, ErroreXpwe> {
    let documento = xml::analizza(dati)?;
    let radice = &documento.radice;
    let (dg, mis, intermedio) = sezioni(radice);
    if dg.is_none() && mis.is_none() {
        return Err(ErroreXpwe::NonXpwe(format!(
            "la radice «{}» non contiene «{}» né «{}»",
            radice.nome,
            st::DATI_GENERALI,
            st::MISURAZIONI
        )));
    }

    let mut cx = Contesto {
        generatore,
        anomalie: Vec::new(),
        aggregate: BTreeMap::new(),
        statistiche: Statistiche::default(),
        id_vc: HashMap::new(),
        vedi_voce: Vec::new(),
    };
    let percorso_radice = radice.nome.clone();

    if documento.caratteri_sostituiti {
        cx.anomalia(
            Gravita::Avviso,
            "xpwe.caratteri_sostituiti",
            format!(
                "alcuni byte non sono validi nella codifica {} e sono stati sostituiti con «\u{FFFD}»",
                documento.codifica
            ),
            1,
            &percorso_radice,
        );
    }
    if let Some(c) = &documento.codifica_sconosciuta {
        cx.anomalia(
            Gravita::Avviso,
            "xpwe.codifica_sconosciuta",
            format!(
                "codifica «{c}» non riconosciuta: usata {}",
                documento.codifica
            ),
            1,
            &percorso_radice,
        );
    }
    if radice.nome != st::RADICE {
        cx.anomalia(
            Gravita::Informazione,
            "xpwe.radice_inattesa",
            format!(
                "elemento radice «{}» invece di «{}»",
                radice.nome,
                st::RADICE
            ),
            radice.riga,
            &percorso_radice,
        );
    }
    if intermedio {
        cx.anomalia(
            Gravita::Informazione,
            "xpwe.struttura_annidata",
            "dati generali e misurazioni sono sotto un elemento intermedio; l'export userà la struttura senza elemento intermedio",
            radice.riga,
            &percorso_radice,
        );
    }

    // Identificativi principali, generati per primi e sempre nello stesso ordine.
    let progetto_id = cx.generatore.nuovo();
    let prezzario_id = cx.generatore.nuovo();
    let computo_id: ComputoId = cx.generatore.nuovo();

    let mut doc = Dati::new();
    if radice.nome != st::RADICE {
        doc.insert("doc:_nome".into(), radice.nome.clone());
    }
    if !intermedio {
        let esito = conservati::registra(
            &mut doc,
            "doc:",
            radice,
            Regole {
                mappati: &[st::DATI_GENERALI, st::MISURAZIONI],
                noti: &st::TESTATA,
                testi: &[],
                ripetibili: &[],
            },
        );
        cx.sconosciuti(esito, &percorso_radice);
    }

    let mut progetto = Progetto::new(progetto_id, "", prezzario_id);
    progetto.elenco_prezzi = Prezzario::new(prezzario_id, "Elenco prezzi");
    let mut prezzario_prov = Provenienza::new(FormatoOrigine::Xpwe);
    prezzario_prov.file.clone_from(&opzioni.nome_file);
    progetto.elenco_prezzi.provenienza = Some(prezzario_prov);

    let mut capitoli: HashMap<(usize, String), CapitoloId> = HashMap::new();
    let mut classi: HashMap<(usize, String), ClasseId> = HashMap::new();

    if let Some(dg) = dg {
        importa_dati_generali(
            &mut cx,
            dg,
            &percorso_radice,
            &mut doc,
            &mut progetto,
            &mut capitoli,
            &mut classi,
        );
    } else {
        cx.anomalia(
            Gravita::Avviso,
            "xpwe.sezione_mancante",
            format!("sezione «{}» assente", st::DATI_GENERALI),
            radice.riga,
            &percorso_radice,
        );
    }

    if progetto.metadati.titolo.trim().is_empty() {
        progetto.metadati.titolo = opzioni
            .nome_file
            .clone()
            .unwrap_or_else(|| "Progetto importato da XPWE".into());
    }

    let mut computo = Computo::new(computo_id, opzioni.nome_computo.clone());
    match mis {
        Some(mis) => importa_misurazioni(
            &mut cx,
            mis,
            &format!("{percorso_radice}/{}", st::MISURAZIONI),
            &mut doc,
            &mut progetto,
            &mut computo,
            &capitoli,
            &classi,
        ),
        None => cx.anomalia(
            Gravita::Avviso,
            "xpwe.sezione_mancante",
            format!(
                "sezione «{}» assente: nessuna voce importata",
                st::MISURAZIONI
            ),
            radice.riga,
            &percorso_radice,
        ),
    }
    progetto.computi.push(computo);

    let (anomalie, statistiche) = cx.chiudi();
    let mut prov = Provenienza::new(FormatoOrigine::Xpwe);
    prov.file.clone_from(&opzioni.nome_file);
    prov.impronta_sha256 = Some(impronta(dati));
    prov.importato_il = opzioni.importato_il;
    prov.anomalie.clone_from(&anomalie);
    prov.dati_originali = doc;
    progetto.provenienza = Some(prov);

    Ok(RisultatoImport {
        progetto,
        computo: computo_id,
        rapporto: RapportoImport {
            anomalie,
            statistiche,
            codifica: documento.codifica.to_owned(),
        },
    })
}

/// Dati anagrafici mappati sui metadati del progetto.
const CAMPI_ANAGRAFICA: [&str; 4] = ["Comune", "Oggetto", "Committente", "Impresa"];

/// Impronta SHA-256 esadecimale.
#[must_use]
pub fn impronta(dati: &[u8]) -> String {
    let digest = Sha256::digest(dati);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn importa_dati_generali<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    dg: &Nodo,
    percorso_radice: &str,
    doc: &mut Dati,
    progetto: &mut Progetto,
    capitoli: &mut HashMap<(usize, String), CapitoloId>,
    classi: &mut HashMap<(usize, String), ClasseId>,
) {
    let percorso = format!("{percorso_radice}/{}", st::DATI_GENERALI);
    let prog = dg.figlio(st::DG_PROGETTO).or_else(|| {
        // Come in LeenO: in mancanza del nome atteso, il primo figlio che non sia
        // un'altra sezione nota.
        dg.figli.iter().find(|f| {
            ![
                st::DG_CAPITOLI_CATEGORIE,
                st::DG_MODULI,
                st::DG_CONFIGURAZIONE,
            ]
            .contains(&f.nome.as_str())
                && !f.is_foglia()
        })
    });
    let mappati_dg: Vec<&str> = [st::DG_CAPITOLI_CATEGORIE, st::DG_CONFIGURAZIONE]
        .into_iter()
        .chain(prog.map(|p| p.nome.as_str()))
        .collect();
    let esito = conservati::registra(
        doc,
        "dg:",
        dg,
        Regole {
            mappati: &mappati_dg,
            noti: &[st::DG_MODULI, st::DG_WBS, st::DG_WBS_CAP],
            testi: &[],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, &percorso);
    if let Some(m) = dg.figlio(st::DG_MODULI) {
        cx.anomalia(
            Gravita::Informazione,
            "xpwe.analisi_non_importata",
            "dati generali delle analisi (spese generali, utili, oneri accessori) conservati ma non importati",
            m.riga,
            &format!("{percorso}/{}", st::DG_MODULI),
        );
    }

    // Dati anagrafici.
    if let Some(prog) = prog {
        if prog.nome != st::DG_PROGETTO {
            doc.insert("dg:prog/_nome".into(), prog.nome.clone());
        }
        let anag = prog
            .figlio(st::DG_DATI_GENERALI)
            .or_else(|| prog.figli.first());
        let mappati_prog: Vec<&str> = anag.map(|a| a.nome.as_str()).into_iter().collect();
        let p_prog = format!("{percorso}/{}", prog.nome);
        let esito = conservati::registra(
            doc,
            "dg:prog/",
            prog,
            Regole {
                mappati: &mappati_prog,
                noti: &[],
                testi: &[],
                ripetibili: &[],
            },
        );
        cx.sconosciuti(esito, &p_prog);
        if let Some(anag) = anag {
            if anag.nome != st::DG_DATI_GENERALI {
                doc.insert("dg:anag/_nome".into(), anag.nome.clone());
            }
            let esito = conservati::registra(
                doc,
                "dg:anag/",
                anag,
                Regole {
                    mappati: &CAMPI_ANAGRAFICA,
                    noti: &["Provincia", "PercPrezzi", "ParteOpera"],
                    testi: &[],
                    ripetibili: &[],
                },
            );
            cx.sconosciuti(esito, &format!("{p_prog}/{}", anag.nome));
            let campo = |n: &str| {
                let t = testo_campo(anag, n);
                (!t.is_empty()).then(|| t.to_owned())
            };
            let oggetto = campo("Oggetto");
            progetto.metadati.titolo = oggetto.clone().unwrap_or_default();
            progetto.metadati.oggetto = oggetto;
            progetto.metadati.luogo = campo("Comune");
            progetto.metadati.committente = campo("Committente");
            progetto.metadati.impresa = campo("Impresa");
        }
    }

    if let Some(cc) = dg.figlio(st::DG_CAPITOLI_CATEGORIE) {
        importa_capitoli_categorie(cx, cc, &percorso, doc, progetto, capitoli, classi);
    }
    if let Some(cfg) = dg.figlio(st::DG_CONFIGURAZIONE) {
        importa_configurazione(cx, cfg, &percorso, doc, &mut progetto.regole);
    }
}

#[allow(clippy::too_many_arguments)]
fn importa_capitoli_categorie<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    cc: &Nodo,
    percorso_dg: &str,
    doc: &mut Dati,
    progetto: &mut Progetto,
    capitoli: &mut HashMap<(usize, String), CapitoloId>,
    classi: &mut HashMap<(usize, String), ClasseId>,
) {
    let percorso = format!("{percorso_dg}/{}", st::DG_CAPITOLI_CATEGORIE);
    let elenchi: Vec<&str> = st::ELENCHI_CAPITOLI
        .iter()
        .chain(st::ELENCHI_CATEGORIE.iter())
        .map(|(n, _)| *n)
        .collect();
    let esito = conservati::registra(
        doc,
        "cc:",
        cc,
        Regole {
            mappati: &elenchi,
            noti: &[],
            testi: &[],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, &percorso);

    for (livello, (nome_elenco, _)) in st::ELENCHI_CAPITOLI.iter().enumerate() {
        let Some(elenco) = cc.figlio(nome_elenco) else {
            continue;
        };
        let p_elenco = format!("{percorso}/{nome_elenco}");
        let attr_prefisso = format!("cc:{nome_elenco}/");
        for (k, v) in &elenco.attributi {
            doc.insert(format!("{attr_prefisso}@{k}"), v.clone());
        }
        for item in &elenco.figli {
            let Some((id_xpwe, codice, descrizione, dati)) =
                leggi_classe(cx, item, &p_elenco, livello)
            else {
                continue;
            };
            if capitoli.contains_key(&(livello, id_xpwe.clone())) {
                cx.anomalia(
                    Gravita::Errore,
                    "xpwe.id_duplicato",
                    format!("capitolo con ID {id_xpwe} duplicato: scartato"),
                    item.riga,
                    &p_elenco,
                );
                continue;
            }
            let id: CapitoloId = cx.generatore.nuovo();
            for (k, v) in dati {
                doc.insert(format!("capitolo:{id}/{k}"), v);
            }
            capitoli.insert((livello, id_xpwe), id);
            progetto.elenco_prezzi.capitoli.push(CapitoloPrezzario {
                id,
                padre: None,
                codice,
                descrizione,
            });
            cx.statistiche.capitoli += 1;
        }
    }

    for (livello, (nome_elenco, _)) in st::ELENCHI_CATEGORIE.iter().enumerate() {
        let Some(elenco) = cc.figlio(nome_elenco) else {
            continue;
        };
        let p_elenco = format!("{percorso}/{nome_elenco}");
        for (k, v) in &elenco.attributi {
            doc.insert(format!("cc:{nome_elenco}/@{k}"), v.clone());
        }
        let liv = LivelloCategoria::TUTTI
            .get(livello)
            .copied()
            .unwrap_or(LivelloCategoria::Categoria);
        for item in &elenco.figli {
            let Some((id_xpwe, codice, descrizione, dati)) =
                leggi_classe(cx, item, &p_elenco, livello)
            else {
                continue;
            };
            if classi.contains_key(&(livello, id_xpwe.clone())) {
                cx.anomalia(
                    Gravita::Errore,
                    "xpwe.id_duplicato",
                    format!("categoria con ID {id_xpwe} duplicata: scartata"),
                    item.riga,
                    &p_elenco,
                );
                continue;
            }
            let id: ClasseId = cx.generatore.nuovo();
            for (k, v) in dati {
                doc.insert(format!("classe:{id}/{k}"), v);
            }
            classi.insert((livello, id_xpwe), id);
            let elenco_modello = match liv {
                LivelloCategoria::SuperCategoria => &mut progetto.categorie.super_categorie,
                LivelloCategoria::Categoria => &mut progetto.categorie.categorie,
                LivelloCategoria::SottoCategoria => &mut progetto.categorie.sotto_categorie,
            };
            if elenco_modello.iter().any(|c| c.codice == codice) && !codice.is_empty() {
                cx.anomalia(
                    Gravita::Avviso,
                    "xpwe.codice_duplicato",
                    format!("{liv} con codice «{codice}» ripetuto"),
                    item.riga,
                    &p_elenco,
                );
            }
            elenco_modello.push(Classe {
                id,
                codice,
                descrizione,
            });
            cx.statistiche.categorie += 1;
        }
    }
}

/// Legge capitolo o categoria: (ID XPWE, codice, descrizione, dati conservati).
fn leggi_classe<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    item: &Nodo,
    percorso_elenco: &str,
    livello: usize,
) -> Option<(String, String, String, Dati)> {
    let Some(id_xpwe) = item
        .attributo("ID")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        cx.anomalia(
            Gravita::Errore,
            "xpwe.id_mancante",
            format!("elemento «{}» senza attributo ID: scartato", item.nome),
            item.riga,
            percorso_elenco,
        );
        return None;
    };
    let mut dati = Dati::new();
    dati.insert("_id".into(), id_xpwe.to_owned());
    dati.insert("_livello".into(), livello.to_string());
    dati.insert("_elemento".into(), item.nome.clone());
    let esito = conservati::registra(
        &mut dati,
        "",
        item,
        Regole {
            mappati: &["Codice", "DesSintetica"],
            noti: &["DesEstesa", "Percentuale"],
            testi: &[],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, &format!("{percorso_elenco}/{}", item.nome));
    Some((
        id_xpwe.to_owned(),
        testo_campo(item, "Codice").to_owned(),
        testo_campo(item, "DesSintetica").to_owned(),
        dati,
    ))
}

fn importa_configurazione<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    cfg: &Nodo,
    percorso_dg: &str,
    doc: &mut Dati,
    regole: &mut RegoleArrotondamento,
) {
    let percorso = format!("{percorso_dg}/{}", st::DG_CONFIGURAZIONE);
    let numeri = cfg
        .figlio(st::DG_CONFIG_NUMERI)
        .or_else(|| cfg.figli.first());
    let mappati: Vec<&str> = numeri.map(|n| n.nome.as_str()).into_iter().collect();
    let esito = conservati::registra(
        doc,
        "cfg:",
        cfg,
        Regole {
            mappati: &mappati,
            noti: &[],
            testi: &[],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, &percorso);
    let Some(numeri) = numeri else { return };
    if numeri.nome != st::DG_CONFIG_NUMERI {
        doc.insert("cfg:num/_nome".into(), numeri.nome.clone());
    }
    let campi = ["Quantita", "Prezzi", "PrezziTotale"];
    let _ = conservati::registra(
        doc,
        "cfg:num/",
        numeri,
        Regole {
            mappati: &campi,
            noti: &[],
            testi: &campi,
            ripetibili: &[],
        },
    );
    let p_num = format!("{percorso}/{}", numeri.nome);
    for (campo, destinazione) in [
        ("Quantita", &mut regole.decimali_quantita),
        ("Prezzi", &mut regole.decimali_prezzo),
        ("PrezziTotale", &mut regole.decimali_importo),
    ] {
        let Some(n) = numeri.figlio(campo) else {
            continue;
        };
        match testo::decimali_configurazione(&n.testo) {
            Some(d) => *destinazione = d,
            None => cx.anomalia(
                Gravita::Avviso,
                "xpwe.configurazione_numeri",
                format!(
                    "«{}» in {campo} non interpretabile: mantenuti {} decimali",
                    n.testo.trim(),
                    *destinazione
                ),
                n.riga,
                &p_num,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn importa_misurazioni<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    mis: &Nodo,
    percorso: &str,
    doc: &mut Dati,
    progetto: &mut Progetto,
    computo: &mut Computo,
    capitoli: &HashMap<(usize, String), CapitoloId>,
    classi: &HashMap<(usize, String), ClasseId>,
) {
    // In mancanza del nome atteso, la sezione è riconosciuta dal contenuto (figli
    // `EPItem` o `VCItem`), mai dalla sola posizione: PriMus omette l'elenco prezzi
    // quando è vuoto, e la prima sezione può essere quella delle voci di computo.
    let per_contenuto = |figli: &str, escluso: Option<&Nodo>| {
        mis.figli.iter().find(|n| {
            escluso.is_none_or(|e| !std::ptr::eq(*n, e)) && n.figli.iter().any(|f| f.nome == figli)
        })
    };
    let ep_nominato = mis.figlio(st::ELENCO_PREZZI);
    let vc_nominato = mis.figlio(st::VOCI_COMPUTO);
    let ep_nodo = ep_nominato.or_else(|| per_contenuto(st::EP_ITEM, vc_nominato));
    let vc_nodo = vc_nominato.or_else(|| per_contenuto(st::VC_ITEM, ep_nodo));
    for (nodo, nominato, atteso, cosa) in [
        (ep_nodo, ep_nominato, st::ELENCO_PREZZI, "elenco prezzi"),
        (vc_nodo, vc_nominato, st::VOCI_COMPUTO, "voci di computo"),
    ] {
        if let (Some(n), None) = (nodo, nominato) {
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.sezione_per_contenuto",
                format!(
                    "«{atteso}» assente: «{}» usato come {cosa} perché ne contiene le voci",
                    n.nome
                ),
                n.riga,
                percorso,
            );
        }
    }
    let mut mappati = Vec::new();
    if let Some(n) = ep_nodo {
        mappati.push(n.nome.as_str());
        if n.nome != st::ELENCO_PREZZI {
            doc.insert("mis:ep/_nome".into(), n.nome.clone());
        }
    }
    if let Some(n) = vc_nodo {
        mappati.push(n.nome.as_str());
        if n.nome != st::VOCI_COMPUTO {
            doc.insert("mis:vc/_nome".into(), n.nome.clone());
        }
    }
    let esito = conservati::registra(
        doc,
        "mis:",
        mis,
        Regole {
            mappati: &mappati,
            noti: &[],
            testi: &[],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, percorso);

    let mut voci_ep: HashMap<String, VocePrezzarioId> = HashMap::new();
    if let Some(ep) = ep_nodo {
        let p_ep = format!("{percorso}/{}", ep.nome);
        let esito = conservati::registra_contenitore(doc, "mis:ep/", ep, st::EP_ITEM);
        cx.sconosciuti(esito, &p_ep);
        for item in ep.figli_con_nome(st::EP_ITEM) {
            importa_voce_elenco(cx, item, &p_ep, progetto, capitoli, &mut voci_ep);
        }
        // Le analisi dopo tutte le voci: un componente può riferirsi a una voce che nel
        // file viene dopo.
        for item in ep.figli_con_nome(st::EP_ITEM) {
            importa_analisi(cx, item, &p_ep, progetto, &voci_ep);
        }
    } else if vc_nodo.is_some_and(|vc| vc.figli.iter().any(|f| f.nome == st::VC_ITEM)) {
        // Senza elenco nessuna voce di computo è risolvibile: ognuna sarà segnalata.
        cx.anomalia(
            Gravita::Avviso,
            "xpwe.sezione_mancante",
            "elenco prezzi assente ma voci di computo presenti",
            mis.riga,
            percorso,
        );
    }

    if let Some(vc) = vc_nodo {
        let p_vc = format!("{percorso}/{}", vc.nome);
        let esito = conservati::registra_contenitore(doc, "mis:vc/", vc, st::VC_ITEM);
        cx.sconosciuti(esito, &p_vc);
        let mut id_viste: HashMap<String, usize> = HashMap::new();
        for item in vc.figli_con_nome(st::VC_ITEM) {
            importa_voce_computo(
                cx,
                item,
                &p_vc,
                progetto,
                computo,
                classi,
                &voci_ep,
                &mut id_viste,
            );
        }
        collega_vedi_voce(cx, computo, &progetto.regole);
    }
}

/// Collega i righi «vedi voce» alle voci citate (IDVV = ID della voce). Il collegamento
/// resta solo se la quantità ricalcolata coincide con quella dichiarata nel file, con
/// i fattori del rigo come moltiplicatori o senza; altrimenti il rigo resta statico con
/// la quantità dichiarata. La convenzione di PriMus non è verificata (ADR 0019).
fn collega_vedi_voce<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    computo: &mut Computo,
    regole: &RegoleArrotondamento,
) {
    let pendenti = std::mem::take(&mut cx.vedi_voce);
    // (voce, rigo, rigo statico originale, variante: 0 con i fattori, 1 senza)
    let mut tentativi: Vec<(usize, usize, RigoMisura, &VediVoceDaCollegare, u8)> = Vec::new();
    for d in &pendenti {
        let Some((iv, ir)) = computo.voci.iter().enumerate().find_map(|(iv, v)| {
            v.righi
                .iter()
                .position(|r| r.id == d.rigo)
                .map(|ir| (iv, ir))
        }) else {
            continue;
        };
        let bersaglio = match cx.id_vc.get(&d.idvv) {
            Some(Some(id)) => *id,
            Some(None) => {
                cx.anomalia(
                    Gravita::Avviso,
                    "xpwe.vedi_voce_non_collegato",
                    format!("IDVV {} usato da più voci: rigo «vedi voce» importato con la quantità dichiarata (statico)", d.idvv),
                    d.riga,
                    &d.percorso,
                );
                continue;
            }
            None => {
                cx.anomalia(
                    Gravita::Avviso,
                    "xpwe.vedi_voce_non_collegato",
                    format!("nessuna voce con ID {}: rigo «vedi voce» importato con la quantità dichiarata (statico)", d.idvv),
                    d.riga,
                    &d.percorso,
                );
                continue;
            }
        };
        let Some(rigo) = computo.voci.get_mut(iv).and_then(|v| v.righi.get_mut(ir)) else {
            continue;
        };
        let originale = rigo.clone();
        imposta_collegamento(rigo, bersaglio, d, 0);
        tentativi.push((iv, ir, originale, d, 0));
    }
    // Si verifica a ogni giro: un collegamento tolto può cambiare le voci che dipendono
    // da quella voce.
    loop {
        let quantita = computo.quantita_riferite(regole);
        let mut cambiato = false;
        let mut restano = Vec::with_capacity(tentativi.len());
        for (iv, ir, originale, d, variante) in tentativi {
            let Some(rigo) = computo.voci.get_mut(iv).and_then(|v| v.righi.get_mut(ir)) else {
                continue;
            };
            let calcolata = rigo.calcola_con(regole, &quantita).ok().flatten();
            let torna = match (calcolata, d.dichiarata) {
                (Some(c), Some(q)) => regole.arrotonda_quantita(q) == c.quantita,
                (Some(c), None) => c.quantita.valore().is_zero(),
                (None, _) => false,
            };
            if torna {
                restano.push((iv, ir, originale, d, variante));
                continue;
            }
            cambiato = true;
            if variante == 0
                && d.fattori.iter().any(Option::is_some)
                && let Some(id) = rigo.vedi_voce.as_ref().and_then(VediVoce::voce)
            {
                imposta_collegamento(rigo, id, d, 1);
                restano.push((iv, ir, originale, d, 1));
                continue;
            }
            *rigo = originale;
            cx.aggregata(
                Gravita::Avviso,
                "xpwe.vedi_voce",
                "RGItem",
                "rigo «vedi voce» importato con la quantità dichiarata nel file (statico): la quantità ricalcolata dalla voce citata non coincide",
                d.riga,
                &d.percorso,
            );
        }
        tentativi = restano;
        if !cambiato {
            break;
        }
    }
    for (_, _, _, d, _) in &tentativi {
        cx.aggregata(
            Gravita::Informazione,
            "xpwe.vedi_voce_collegato",
            "RGItem",
            "rigo «vedi voce» collegato alla voce citata: la quantità segue la voce",
            d.riga,
            &d.percorso,
        );
    }
}

/// Rigo collegato alla voce `bersaglio`: con i fattori del file (`variante` 0) o senza
/// (1); deduzione se la quantità dichiarata è negativa.
fn imposta_collegamento(
    rigo: &mut RigoMisura,
    bersaglio: VoceComputoId,
    d: &VediVoceDaCollegare,
    variante: u8,
) {
    let [parti_uguali, lunghezza, larghezza, altezza_peso] = if variante == 0 {
        d.fattori.clone().map(|f| {
            f.map(|f| match f {
                Fattore::Valore(v) => Fattore::Valore(v.abs()),
                altro => altro,
            })
        })
    } else {
        [None, None, None, None]
    };
    rigo.parti_uguali = parti_uguali;
    rigo.lunghezza = lunghezza;
    rigo.larghezza = larghezza;
    rigo.altezza_peso = altezza_peso;
    rigo.formula = None;
    rigo.segno = if d.dichiarata.is_some_and(|q| q.is_sign_negative()) {
        SegnoRigo::Deduzione
    } else {
        SegnoRigo::Positivo
    };
    rigo.vedi_voce = Some(VediVoce::Collegata(bersaglio));
}

fn importa_voce_elenco<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    item: &Nodo,
    p_ep: &str,
    progetto: &mut Progetto,
    capitoli: &HashMap<(usize, String), CapitoloId>,
    voci_ep: &mut HashMap<String, VocePrezzarioId>,
) {
    let Some(id_xpwe) = item
        .attributo("ID")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        cx.anomalia(
            Gravita::Errore,
            "xpwe.id_mancante",
            format!(
                "voce di elenco «{}» senza attributo ID: scartata",
                testo_campo(item, "Tariffa")
            ),
            item.riga,
            p_ep,
        );
        cx.statistiche.voci_elenco_scartate += 1;
        return;
    };
    let percorso = format!("{p_ep}/{}[ID={id_xpwe}]", st::EP_ITEM);
    if voci_ep.contains_key(id_xpwe) {
        cx.anomalia(
            Gravita::Errore,
            "xpwe.id_duplicato",
            format!("voce di elenco con ID {id_xpwe} duplicata: scartata"),
            item.riga,
            &percorso,
        );
        cx.statistiche.voci_elenco_scartate += 1;
        return;
    }
    let codice = testo_campo(item, "Tariffa").to_owned();
    let prezzo = match testo::decimale(testo_campo(item, "Prezzo1")) {
        Ok(Some(p)) => p,
        Ok(None) => {
            cx.anomalia(
                Gravita::Errore,
                "xpwe.prezzo_mancante",
                format!("voce «{codice}» senza Prezzo1: scartata"),
                item.riga,
                &percorso,
            );
            cx.statistiche.voci_elenco_scartate += 1;
            return;
        }
        Err(e) => {
            cx.anomalia(
                Gravita::Errore,
                "xpwe.numero_non_valido",
                format!("voce «{codice}», Prezzo1: {e}; voce scartata"),
                item.riga,
                &percorso,
            );
            cx.statistiche.voci_elenco_scartate += 1;
            return;
        }
    };

    let des_breve = testo_campo(item, "DesBreve");
    let (breve, breve_da) = if des_breve.is_empty() {
        (testo_campo(item, "DesRidotta"), "DesRidotta")
    } else {
        (des_breve, "DesBreve")
    };
    let altra_breve = if breve_da == "DesBreve" {
        "DesRidotta"
    } else {
        "DesBreve"
    };
    let mappati = [
        "Tariffa",
        breve_da,
        "DesEstesa",
        "UnMisura",
        "Prezzo1",
        "Flags",
        "IncMDO",
        "IncSIC",
        "IDSpCap",
        "IDCap",
        "IDSbCap",
    ];
    let noti = [
        "TipoEP",
        "Articolo",
        altra_breve,
        "Prezzo2",
        "Prezzo3",
        "Prezzo4",
        "Prezzo5",
        "Data",
        "AdrInternet",
        "IncMAT",
        "IncATTR",
        st::EP_ANALISI,
    ];
    let mut dati = Dati::new();
    if breve_da != "DesBreve" {
        dati.insert("_breve_da".into(), breve_da.into());
    }
    let esito = conservati::registra(
        &mut dati,
        "",
        item,
        Regole {
            mappati: &mappati,
            noti: &noti,
            testi: &["Flags", "IDSpCap", "IDCap", "IDSbCap"],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, &format!("{p_ep}/{}", st::EP_ITEM));

    let flags_testo = testo_campo(item, "Flags");
    let flags = if flags_testo.is_empty() {
        0
    } else {
        testo::intero(flags_testo).unwrap_or_else(|| {
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.numero_non_valido",
                format!("Flags «{flags_testo}» non numerico: considerato 0"),
                item.riga,
                &percorso,
            );
            0
        })
    };
    let natura = if flags & st::FLAG_VOCE_SICUREZZA != 0 {
        NaturaVoce::OneriSicurezza
    } else {
        NaturaVoce::Lavoro
    };
    // Capitolo: il livello più profondo risolvibile.
    let mut capitolo = None;
    for (livello, campo) in ["IDSpCap", "IDCap", "IDSbCap"].into_iter().enumerate() {
        if let Some(r) = riferimento(item, campo) {
            match capitoli.get(&(livello, r.clone())) {
                Some(c) => capitolo = Some(*c),
                None => cx.anomalia(
                    Gravita::Avviso,
                    "xpwe.riferimento_non_risolto",
                    format!(
                        "{campo} = {r} non corrisponde ad alcun capitolo: riferimento conservato"
                    ),
                    item.riga,
                    &percorso,
                ),
            }
        }
    }

    if progetto
        .elenco_prezzi
        .voci
        .iter()
        .any(|v| v.codice == codice)
    {
        cx.anomalia(
            Gravita::Avviso,
            "xpwe.tariffa_duplicata",
            format!("tariffa «{codice}» già presente nell'elenco: importata comunque"),
            item.riga,
            &percorso,
        );
    }

    let incidenza_manodopera = percentuale_facoltativa(cx, item, "IncMDO", &percorso);
    let incidenza_sicurezza = percentuale_facoltativa(cx, item, "IncSIC", &percorso);
    let id: VocePrezzarioId = cx.generatore.nuovo();
    progetto.elenco_prezzi.voci.push(VocePrezzario {
        id,
        codice,
        descrizione_breve: breve.to_owned(),
        descrizione_estesa: testo_campo(item, "DesEstesa").to_owned(),
        unita_misura: testo_campo(item, "UnMisura").to_owned(),
        prezzo: Prezzo::new(prezzo),
        natura,
        incidenza_manodopera,
        incidenza_sicurezza,
        capitolo,
        analisi: None,
        nuovo_prezzo: None,
        provenienza: Some(provenienza(Some(id_xpwe.to_owned()), dati)),
    });
    voci_ep.insert(id_xpwe.to_owned(), id);
    cx.statistiche.voci_elenco += 1;
}

/// Testo su una riga sola: spazi e ritorni a capo ripetuti diventano uno spazio.
fn su_una_riga(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Importa l'analisi del prezzo di una voce di elenco, se ne ha una con componenti.
///
/// Il prezzo della voce resta quello dichiarato nel file; l'analisi gli viene collegata.
/// Spese generali, utile e tipi di risorsa non sono nel file: vedi il modulo `analisi`.
fn importa_analisi<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    item: &Nodo,
    p_ep: &str,
    progetto: &mut Progetto,
    voci_ep: &HashMap<String, VocePrezzarioId>,
) {
    let Some(contenitore) = item.figlio(st::EP_ANALISI) else {
        return;
    };
    let componenti: Vec<&Nodo> = contenitore
        .figli
        .iter()
        .flat_map(|f| f.figli.iter())
        .filter(|c| c.nome == st::EPAR_ITEM)
        .collect();
    if componenti.is_empty() {
        return;
    }
    let Some(id_xpwe) = item.attributo("ID").map(str::trim) else {
        return;
    };
    let Some(voce_id) = voci_ep.get(id_xpwe).copied() else {
        return;
    };
    let percorso = format!("{p_ep}/{}[ID={id_xpwe}]/{}", st::EP_ITEM, st::EP_ANALISI);
    let non_importata = |cx: &mut Contesto<'_, G>, perche: &str| {
        cx.aggregata(
            Gravita::Informazione,
            "xpwe.analisi_non_importata",
            perche,
            format!("analisi del prezzo conservata ma non importata ({perche}): vale il Prezzo1 dichiarato"),
            contenitore.riga,
            &percorso,
        );
    };

    let mut righe: Vec<RigaAnalisi> = Vec::new();
    for c in componenti {
        let numero = |campo: &str| testo::decimale(testo_campo(c, campo));
        let (quantita, prezzo) = match (numero("Qt"), numero("Prezzo")) {
            // Riga di solo testo (titolo di un gruppo): non è un componente.
            (Ok(None), Ok(None)) => continue,
            (Ok(Some(q)), Ok(Some(p))) if q >= Decimal::ZERO && p >= Decimal::ZERO => (q, p),
            _ => {
                non_importata(cx, "un componente ha quantità o prezzo non validi");
                return;
            }
        };
        // Componente che rimanda a un'altra voce dell'elenco: i campi vuoti vengono da lì.
        let riferita = riferimento(c, "IDEP")
            .and_then(|r| voci_ep.get(&r))
            .filter(|id| **id != voce_id)
            .and_then(|id| progetto.elenco_prezzi.voci.iter().find(|v| v.id == *id));
        let o = |proprio: String, della_voce: fn(&VocePrezzario) -> &str| {
            if proprio.is_empty() {
                riferita.map(della_voce).unwrap_or_default().to_owned()
            } else {
                proprio
            }
        };
        righe.push(RigaAnalisi {
            codice: riferita.map(|v| v.codice.clone()).unwrap_or_default(),
            descrizione: o(su_una_riga(testo_campo(c, "Descrizione")), |v| {
                &v.descrizione_breve
            }),
            unita_misura: o(testo_campo(c, "Misura").trim().to_owned(), |v| {
                &v.unita_misura
            }),
            tipo: opencomputo_core::analisi::TipoRisorsa::Altro,
            prezzo: Prezzo::new(prezzo),
            quantita: Quantita::new(quantita),
            espressione: None,
            origine: riferita.map(|v| OrigineRigaAnalisi::VocePrezzario(v.id)),
        });
    }
    if righe.is_empty() {
        non_importata(cx, "nessun componente con quantità e prezzo");
        return;
    }

    let regole = progetto.regole.clone();
    let Some(voce) = progetto.elenco_prezzi.voci.iter().find(|v| v.id == voce_id) else {
        return;
    };
    let dichiarato = voce.prezzo;

    // Tipi di risorsa: quelli che riproducono le incidenze dichiarate sul prezzo.
    let incidenza = |campo: &str| {
        testo::decimale(testo_campo(item, campo))
            .ok()
            .flatten()
            .unwrap_or(Decimal::ZERO)
    };
    let importi: Vec<Decimal> = righe
        .iter()
        .map(|r| {
            r.prezzo
                .per_quantita(r.quantita, &regole)
                .map_or(Decimal::ZERO, |i| i.valore())
        })
        .collect();
    let unita: Vec<&str> = righe.iter().map(|r| r.unita_misura.as_str()).collect();
    let (tipi, confermati) = deduzioni::classifica(
        &importi,
        &unita,
        dichiarato.valore(),
        [
            incidenza("IncMAT"),
            incidenza("IncMDO"),
            incidenza("IncATTR"),
        ],
    );
    for (r, t) in righe.iter_mut().zip(tipi) {
        r.tipo = t;
    }

    let mut analisi = AnalisiPrezzo {
        id: cx.generatore.nuovo(),
        codice: voce.codice.clone(),
        descrizione: voce.descrizione_breve.clone(),
        unita_misura: voce.unita_misura.clone(),
        quantita_riferimento: Quantita::new(Decimal::ONE),
        righe,
        spese_generali: Percentuale::ZERO,
        utile_impresa: Percentuale::ZERO,
        base_utile: BaseUtile::default(),
        variabili: Vec::new(),
        quota_oneri_sicurezza: None,
    };
    if analisi.calcola(&regole).is_err() {
        non_importata(cx, "il calcolo dei componenti non riesce");
        return;
    }
    match deduzioni::deduci_percentuali(&mut analisi, dichiarato, &regole) {
        Some(p) => {
            let (sg, utile) = (p.spese_generali.normalize(), p.utile.normalize());
            cx.aggregata(
                Gravita::Informazione,
                "xpwe.analisi_importata",
                &format!("{sg}/{utile}"),
                format!(
                    "analisi del prezzo importata con spese generali {sg} % e utile {utile} %: le percentuali non sono nel file, sono dedotte perché riproducono il prezzo dichiarato"
                ),
                contenitore.riga,
                &percorso,
            );
            if p.calcolato != dichiarato {
                cx.anomalia(
                    Gravita::Informazione,
                    "xpwe.analisi_arrotondamento",
                    format!(
                        "analisi di «{}»: arrotondando ogni componente OpenComputo calcola {}, il file dichiara {}; resta il prezzo dichiarato (si può riallineare dalla vista «Analisi prezzi»)",
                        analisi.codice,
                        p.calcolato.valore(),
                        dichiarato.valore()
                    ),
                    contenitore.riga,
                    &percorso,
                );
            }
        }
        None => {
            let calcolato = analisi
                .calcola(&regole)
                .map_or(Decimal::ZERO, |r| r.prezzo_unitario.valore());
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.analisi_prezzo_diverso",
                format!(
                    "analisi di «{}» importata senza spese generali e utile: i componenti danno {calcolato}, il file dichiara {}; resta il prezzo dichiarato",
                    analisi.codice,
                    dichiarato.valore()
                ),
                contenitore.riga,
                &percorso,
            );
        }
    }
    if !confermati {
        cx.aggregata(
            Gravita::Informazione,
            "xpwe.analisi_tipi_presunti",
            "EPItem",
            "tipo di risorsa dei componenti di analisi presunto dall'unità di misura (ore: manodopera, altrimenti materiale): il file non lo indica e le incidenze dichiarate non bastano a stabilirlo",
            contenitore.riga,
            &percorso,
        );
    }
    let id = analisi.id;
    progetto.elenco_prezzi.analisi.push(analisi);
    if let Some(v) = progetto
        .elenco_prezzi
        .voci
        .iter_mut()
        .find(|v| v.id == voce_id)
    {
        v.analisi = Some(id);
    }
    cx.statistiche.analisi += 1;
}

/// Contenitore dei righi di una voce: per nome o, in mancanza, l'ultimo figlio con `RGItem`.
fn contenitore_misure(item: &Nodo) -> Option<&Nodo> {
    item.figlio(st::VC_MISURE).or_else(|| {
        item.figli
            .iter()
            .rev()
            .find(|f| f.figli.iter().any(|g| g.nome == st::RG_ITEM))
    })
}

#[allow(clippy::too_many_arguments)]
fn importa_voce_computo<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    item: &Nodo,
    p_vc: &str,
    progetto: &Progetto,
    computo: &mut Computo,
    classi: &HashMap<(usize, String), ClasseId>,
    voci_ep: &HashMap<String, VocePrezzarioId>,
    id_viste: &mut HashMap<String, usize>,
) {
    let numero = computo.voci.len() + 1;
    let id_xpwe = item
        .attributo("ID")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let percorso = match &id_xpwe {
        Some(i) => format!("{p_vc}/{}[ID={i}]", st::VC_ITEM),
        None => format!("{p_vc}/{}[{numero}]", st::VC_ITEM),
    };
    if let Some(i) = &id_xpwe {
        if let Some(prima) = id_viste.insert(i.clone(), item.riga) {
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.id_duplicato",
                format!("voce di computo con ID {i} già usato alla riga {prima}: i riferimenti «vedi voce» potrebbero essere ambigui"),
                item.riga,
                &percorso,
            );
        }
    } else {
        cx.anomalia(
            Gravita::Avviso,
            "xpwe.id_mancante",
            "voce di computo senza attributo ID: importata, ma non referenziabile da «vedi voce»",
            item.riga,
            &percorso,
        );
    }

    let idep = testo_campo(item, "IDEP");
    let Some(voce_ep) = voci_ep
        .get(idep)
        .and_then(|id| progetto.elenco_prezzi.voce(*id))
    else {
        cx.anomalia(
            Gravita::Errore,
            "xpwe.idep_non_risolto",
            format!("IDEP «{idep}» non corrisponde ad alcuna voce di elenco importata: voce di computo scartata"),
            item.riga,
            &percorso,
        );
        cx.statistiche.voci_computo_scartate += 1;
        return;
    };

    let misure = contenitore_misure(item);
    let mut mappati = vec!["IDEP", "Quantita", "IDSpCat", "IDCat", "IDSbCat"];
    if let Some(m) = misure {
        mappati.push(m.nome.as_str());
    }
    let mut dati = Dati::new();
    if let Some(m) = misure
        && m.nome != st::VC_MISURE
    {
        dati.insert("_misure".into(), m.nome.clone());
    }
    let esito = conservati::registra(
        &mut dati,
        "",
        item,
        Regole {
            mappati: &mappati,
            noti: &["DataMis", "Flags"],
            testi: &["IDSpCat", "IDCat", "IDSbCat"],
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, &format!("{p_vc}/{}", st::VC_ITEM));

    let mut categorie = AssegnazioneCategorie::default();
    for (livello, campo) in ["IDSpCat", "IDCat", "IDSbCat"].into_iter().enumerate() {
        let Some(r) = riferimento(item, campo) else {
            continue;
        };
        match classi.get(&(livello, r.clone())) {
            Some(c) => match livello {
                0 => categorie.super_categoria = Some(*c),
                1 => categorie.categoria = Some(*c),
                _ => categorie.sotto_categoria = Some(*c),
            },
            None => cx.anomalia(
                Gravita::Avviso,
                "xpwe.riferimento_non_risolto",
                format!(
                    "{campo} = {r} non corrisponde ad alcuna categoria: riferimento conservato"
                ),
                item.riga,
                &percorso,
            ),
        }
    }

    let prezzo = PrezzoApplicato::da_voce(&progetto.elenco_prezzi, voce_ep);
    let mut voce = VoceComputo::new(cx.generatore.nuovo(), prezzo);
    voce.categorie = categorie;

    if let Some(m) = misure {
        let p_m = format!("{percorso}/{}", m.nome);
        let esito = conservati::registra_contenitore(&mut dati, "misure/", m, st::RG_ITEM);
        cx.sconosciuti(esito, &format!("{p_vc}/{}/{}", st::VC_ITEM, m.nome));
        let totale_voce = testo::decimale(testo_campo(item, "Quantita"))
            .ok()
            .flatten();
        for (i, rg) in m.figli_con_nome(st::RG_ITEM).enumerate() {
            let rigo = importa_rigo(
                cx,
                rg,
                &format!("{p_m}/{}[{}]", st::RG_ITEM, i + 1),
                &progetto.regole,
                &mut dati,
                totale_voce,
            );
            voce.righi.push(rigo);
            cx.statistiche.righi += 1;
        }
    }

    // Quantità della voce dichiarata nel file.
    match testo::decimale(testo_campo(item, "Quantita")) {
        Ok(Some(dichiarata)) => {
            if let Ok(calcolata) = quantita_totale(&voce.righi, &progetto.regole) {
                verifica_quantita(
                    cx,
                    dichiarata,
                    calcolata.valore(),
                    &progetto.regole,
                    item.riga,
                    &percorso,
                    "voce",
                );
            }
        }
        Ok(None) => {}
        Err(e) => cx.anomalia(
            Gravita::Avviso,
            "xpwe.numero_non_valido",
            format!("Quantita della voce: {e}; non verificata"),
            item.riga,
            &percorso,
        ),
    }

    if let Some(i) = &id_xpwe {
        cx.id_vc
            .entry(i.clone())
            .and_modify(|v| *v = None)
            .or_insert(Some(voce.id));
    }
    voce.provenienza = Some(provenienza(id_xpwe, dati));
    computo.voci.push(voce);
    cx.statistiche.voci_computo += 1;
}

fn verifica_quantita<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    dichiarata: Decimal,
    calcolata: Decimal,
    regole: &RegoleArrotondamento,
    riga: usize,
    percorso: &str,
    cosa: &str,
) -> bool {
    cx.statistiche.quantita_verificate += 1;
    let a = regole.arrotonda_quantita(dichiarata);
    let b = regole.arrotonda_quantita(calcolata);
    if a == b {
        return true;
    }
    cx.statistiche.quantita_diverse += 1;
    cx.anomalia(
        Gravita::Avviso,
        "xpwe.quantita_diversa",
        format!("quantità del {cosa}: il file dichiara {dichiarata}, il ricalcolo dà {b}"),
        riga,
        percorso,
    );
    false
}

const CAMPI_FATTORI: [&str; 4] = ["PartiUguali", "Lunghezza", "Larghezza", "HPeso"];

fn importa_rigo<G: GeneratoreId>(
    cx: &mut Contesto<'_, G>,
    rg: &Nodo,
    percorso: &str,
    regole: &RegoleArrotondamento,
    dati_voce: &mut Dati,
    totale_voce: Option<Decimal>,
) -> RigoMisura {
    let id: RigoMisuraId = cx.generatore.nuovo();
    let prefisso = format!("rigo:{id}/");
    if let Some(i) = rg.attributo("ID") {
        dati_voce.insert(format!("{prefisso}_id"), i.to_owned());
    }
    let flags_testo = testo_campo(rg, "Flags");
    let flags = if flags_testo.is_empty() {
        0
    } else {
        testo::intero(flags_testo).unwrap_or_else(|| {
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.numero_non_valido",
                format!("Flags «{flags_testo}» non numerico: considerato 0"),
                rg.riga,
                percorso,
            );
            0
        })
    };
    let idvv = testo_campo(rg, "IDVV");
    let vedi_voce = !idvv.is_empty() && idvv != st::IDVV_NESSUNO;
    let parziale = flags & st::FLAG_RIGO_PARZIALE != 0;
    let mut testi = vec!["Flags", "IDVV"];
    if vedi_voce || parziale {
        testi.extend(CAMPI_FATTORI);
        testi.push("Quantita");
    }
    let esito = conservati::registra(
        dati_voce,
        &prefisso,
        rg,
        Regole {
            mappati: &[
                "IDVV",
                "Descrizione",
                "PartiUguali",
                "Lunghezza",
                "Larghezza",
                "HPeso",
                "Quantita",
                "Flags",
            ],
            noti: &[],
            testi: &testi,
            ripetibili: &[],
        },
    );
    cx.sconosciuti(esito, "RGItem");
    if flags & !st::FLAG_RIGO_PARZIALE != 0 {
        cx.aggregata(
            Gravita::Informazione,
            "xpwe.flag_sconosciuti",
            "RGItem",
            "rigo con Flags di significato sconosciuto: valore conservato",
            rg.riga,
            percorso,
        );
    }

    let descrizione = testo_campo(rg, "Descrizione").to_owned();
    let dichiarata = match testo::decimale(testo_campo(rg, "Quantita")) {
        Ok(q) => q,
        Err(e) => {
            cx.anomalia(
                Gravita::Avviso,
                "xpwe.numero_non_valido",
                format!("Quantita del rigo: {e}; non verificata"),
                rg.riga,
                percorso,
            );
            None
        }
    };

    if parziale {
        cx.aggregata(
            Gravita::Informazione,
            "xpwe.rigo_parziale",
            "RGItem",
            "rigo «parziale» (subtotale) importato come commento: non altera la quantità",
            rg.riga,
            percorso,
        );
        return RigoMisura::commento(id, descrizione);
    }
    if vedi_voce {
        // Statico con la quantità dichiarata; a fine computo si prova il collegamento.
        let valore = dichiarata.unwrap_or(Decimal::ZERO);
        let fattori = CAMPI_FATTORI.map(|campo| {
            testo::fattore(testo_campo(rg, campo))
                .filter(|l| l.errore.is_none())
                .map(|l| l.fattore)
        });
        cx.vedi_voce.push(VediVoceDaCollegare {
            rigo: id,
            idvv: idvv.to_owned(),
            dichiarata,
            fattori,
            riga: rg.riga,
            percorso: percorso.to_owned(),
        });
        return RigoMisura::misura(id, descrizione).con_vedi_voce(VediVoce::Statica {
            quantita: valore,
            riferimento: format!("voce con ID {idvv} del file XPWE"),
        });
    }

    let mut fattori: [Option<Fattore>; 4] = [None, None, None, None];
    for (campo, destinazione) in CAMPI_FATTORI.iter().zip(fattori.iter_mut()) {
        let Some(letto) = testo::fattore(testo_campo(rg, campo)) else {
            continue;
        };
        if letto.normalizzato {
            cx.aggregata(
                Gravita::Informazione,
                "xpwe.espressione_normalizzata",
                "RGItem",
                "moltiplicazione implicita resa esplicita (es. «2(3+4)» → «2*(3+4)»)",
                rg.riga,
                percorso,
            );
        }
        if let Some(e) = &letto.errore {
            cx.anomalia(
                Gravita::Errore,
                "xpwe.espressione_non_valida",
                format!("{campo}: {e}; il computo non sarà calcolabile finché il rigo non viene corretto"),
                rg.riga,
                percorso,
            );
        }
        *destinazione = Some(letto.fattore);
    }

    let zero_dichiarato = dichiarata.is_none_or(|q| q.is_zero());
    let solo_zeri = fattori
        .iter()
        .flatten()
        .all(|f| matches!(f, Fattore::Valore(v) if v.is_zero()));
    if solo_zeri && zero_dichiarato {
        return RigoMisura {
            tipo: TipoRigo::Commento,
            ..RigoMisura::misura(id, descrizione)
        };
    }

    let mut rigo = costruisci_rigo(id, descrizione.clone(), fattori.clone());
    if let Some(q) = dichiarata
        && let Ok(Some(r)) = rigo.calcola(regole)
    {
        let diversa = regole.arrotonda_quantita(q) != regole.arrotonda_quantita(r.valore_esatto);
        // Fatto osservato in un export reale (PriMus-DCF, formato 5.01): la `Quantita` di
        // ogni rigo ripete il totale della voce, non la quantità del rigo. In quel caso
        // non c'è nulla da confrontare rigo per rigo: vale il controllo sul totale.
        if diversa
            && totale_voce
                .is_some_and(|t| regole.arrotonda_quantita(t) == regole.arrotonda_quantita(q))
        {
            return rigo;
        }
        if diversa {
            // Interpretazione alternativa: valori zero come campi vuoti.
            let senza_zeri = fattori.clone().map(|f| match f {
                Some(Fattore::Valore(v)) if v.is_zero() => None,
                altro => altro,
            });
            let alternativo = costruisci_rigo(id, descrizione, senza_zeri);
            if let Ok(Some(a)) = alternativo.calcola(regole)
                && regole.arrotonda_quantita(q) == regole.arrotonda_quantita(a.valore_esatto)
            {
                cx.aggregata(
                            Gravita::Informazione,
                            "xpwe.zeri_come_vuoti",
                            "RGItem",
                            "campi a zero interpretati come vuoti: così la quantità coincide con quella dichiarata nel file",
                            rg.riga,
                            percorso,
                        );
                rigo = alternativo;
            }
        }
        if let Ok(Some(r)) = rigo.calcola(regole) {
            verifica_quantita(cx, q, r.valore_esatto, regole, rg.riga, percorso, "rigo");
        }
    }
    rigo
}

/// Costruisce un rigo; un numero dispari di valori negativi diventa una deduzione
/// con valori assoluti (le espressioni restano invariate).
/// Contenuto di un'espressione nella forma `-(X)` con parentesi che racchiudono
/// tutto il resto (forma usata dall'export per le deduzioni).
fn negazione_avvolta(espressione: &str) -> Option<&str> {
    let interno = espressione.trim().strip_prefix("-(")?.strip_suffix(')')?;
    let mut profondita: i64 = 0;
    for c in interno.chars() {
        match c {
            '(' => profondita += 1,
            ')' => {
                profondita -= 1;
                if profondita < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    (profondita == 0).then_some(interno)
}

fn costruisci_rigo(
    id: RigoMisuraId,
    descrizione: String,
    mut fattori: [Option<Fattore>; 4],
) -> RigoMisura {
    let mut avvolte = 0usize;
    for f in fattori.iter_mut().flatten() {
        if let Fattore::Espressione(e) = f
            && let Some(interno) = negazione_avvolta(e)
        {
            *e = interno.to_owned();
            avvolte += 1;
        }
    }
    let negativi = avvolte
        + fattori
            .iter()
            .flatten()
            .filter(|f| matches!(f, Fattore::Valore(v) if v.is_sign_negative() && !v.is_zero()))
            .count();
    for f in fattori.iter_mut().flatten() {
        if let Fattore::Valore(v) = f
            && v.is_sign_negative()
        {
            *v = v.abs();
        }
    }
    let [parti_uguali, lunghezza, larghezza, altezza_peso] = fattori;
    RigoMisura {
        id,
        tipo: TipoRigo::Misura,
        descrizione,
        parti_uguali,
        lunghezza,
        larghezza,
        altezza_peso,
        formula: None,
        segno: if negativi % 2 == 1 {
            SegnoRigo::Deduzione
        } else {
            SegnoRigo::Positivo
        },
        vedi_voce: None,
    }
}
