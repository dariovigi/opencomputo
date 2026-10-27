//! Export **sperimentale** dell'elenco prezzi in DCF, nella forma dei prezzari
//! distribuiti in DCF (ADR 0016).
//!
//! Il file contiene, in chiaro:
//!
//! | Flusso | Contenuto |
//! |--------|-----------|
//! | `Documento/CollectionEP.xml` | una `ItemEP` per voce, con i 59 attributi nell'ordine dei prezzari |
//! | `Documento/DatiGenerali.xml` | dati generali, capitoli e categorie su tre livelli, parametri delle analisi e dei decimali |
//! | `Documento/InfoDoc.xml` | tipo di documento e formato (5.01), programma che lo ha scritto |
//! | `Documento/CollectionGP.xml`, `CollectionOD.xml`, `CollectionVC.xml`, `InfoMRR.xml` | raccolte vuote |
//! | `Versione`, `VersioneFix` | 5.01 e 1.07 (f64) |
//!
//! Non contiene:
//! * `Identity`: nei file del programma di riferimento è un codice di autenticazione che
//!   OpenComputo non può né deve produrre;
//! * gli stili di stampa (`CollectionST`) e le opzioni dell'interfaccia;
//! * le analisi prezzi;
//! * il computo e la contabilità: nei documenti del programma di riferimento sono
//!   protetti, e riprodurre la protezione significherebbe aggirarla (ADR 0006).
//!
//! Le voci importate da un DCF riprendono gli attributi originali conservati nella
//! provenienza (articolo, prezzi alternativi, `Fgs`…), così l'andata e ritorno non perde
//! dati. L'apertura nel programma di riferimento **non è verificata** da OpenComputo.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use chrono::NaiveDateTime;
use opencomputo_core::classificazione::Classe;
use opencomputo_core::id::CapitoloId;
use opencomputo_core::prezzario::{CapitoloPrezzario, NaturaVoce, VocePrezzario};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::provenienza::{Anomalia, FormatoOrigine};
use opencomputo_core::validazione::Gravita;
use rust_decimal::Decimal;

use crate::errore::ErroreDcf;
use crate::importa::FLAG_VOCE_SICUREZZA;
use crate::scrittura::{self, Elemento};

/// Versione del formato dei dati (come `Vrsn` dei prezzari).
pub const VERSIONE_FORMATO: &str = "5.01";
/// Versione di correzione del formato (`VrsnFix`).
pub const VERSIONE_FIX: &str = "1.07";
/// 5,01 in IEEE 754 doppia precisione, little endian.
const VERSIONE_F64: [u8; 8] = [0x0A, 0xD7, 0xA3, 0x70, 0x3D, 0x0A, 0x14, 0x40];
/// 1,07 in IEEE 754 doppia precisione, little endian.
const VERSIONE_FIX_F64: [u8; 8] = [0x1F, 0x85, 0xEB, 0x51, 0xB8, 0x1E, 0xF1, 0x3F];
/// Data «nessuna» dei campi data.
const DATA_NULLA: &str = "30/12/1899";
/// Decimali delle incidenze nel file.
const DECIMALI_INCIDENZE: u32 = 18;
/// Primo `IdEP` delle voci (come nei prezzari).
const PRIMO_ID: usize = 2;

/// Attributi di `ItemEP` nell'ordine dei prezzari, con il valore predefinito.
const ATTRIBUTI_EP: [(&str, &str); 59] = [
    ("IdEP", ""),
    ("IdSrc", "0"),
    ("Trf", ""),
    ("Art", ""),
    ("DesRid", ""),
    ("DesEst", ""),
    ("DesBrv", ""),
    ("UnMsr", ""),
    ("Prz1", "0"),
    ("Prz2", "0"),
    ("Prz3", "0"),
    ("Prz4", "0"),
    ("Prz5", "0"),
    ("CnfQt", ""),
    ("QtPrg", "0"),
    ("CodFase", ""),
    ("TipoLvr", "0"),
    ("IdSpCap", "0"),
    ("IdCap", "0"),
    ("IdSbCap", "0"),
    ("Rbs", "0"),
    ("Fgs", "0"),
    ("Rclc", "0"),
    ("PrNetto", "0"),
    ("Fabbis", "0"),
    ("Data", DATA_NULLA),
    ("IdNrm1", "0"),
    ("IdNrm2", "0"),
    ("IdNrm3", "0"),
    ("IdNrm4", "0"),
    ("IdNrm5", "0"),
    ("IdNrm6", "0"),
    ("IdNrm7", "0"),
    ("IdNrm8", "0"),
    ("Prcntl", "0"),
    ("PrcntlTrf", ""),
    ("AdrInet", ""),
    ("IdOD", "0"),
    ("ICodBridge", ""),
    ("ITmpEsec", "0"),
    ("ISglDitta", ""),
    ("IFamSconto", ""),
    ("Dsp1a", ""),
    ("Dsp1n", "0"),
    ("Dsp1s", "0"),
    ("Dsp1i", "0"),
    ("Blocco", "0"),
    ("TipoRsrs", "0"),
    ("Aliq", "0"),
    ("PrzPrg", "0"),
    ("MDOPrz", "0"),
    ("MDOPrzNetto", "0"),
    ("SICInc", "0.000000000000000000"),
    ("MDOInc", "0.000000000000000000"),
    ("ExtDt", ""),
    ("IncMt", "0.000000000000000000"),
    ("IncAttr", "0.000000000000000000"),
    ("TagBIM", ""),
    ("IdWBS", ""),
];

/// Attributi calcolati da OpenComputo (mai ripresi dai dati originali).
const CALCOLATI_EP: [&str; 12] = [
    "IdEP", "Trf", "DesRid", "DesEst", "DesBrv", "UnMsr", "Prz1", "IdSpCap", "IdCap", "IdSbCap",
    "SICInc", "MDOInc",
];

/// Opzioni dell'export.
#[derive(Debug, Clone)]
pub struct OpzioniExport {
    /// Data e ora del documento (date dell'indice del contenitore).
    pub data: NaiveDateTime,
    /// Versione di OpenComputo, scritta come programma di origine.
    pub versione_programma: String,
}

/// Risultato dell'export.
#[derive(Debug, Clone)]
pub struct RisultatoExport {
    /// Byte del file DCF.
    pub byte: Vec<u8>,
    /// Voci esportate.
    pub voci: usize,
    /// Capitoli esportati (tutti i livelli).
    pub capitoli: usize,
    /// Segnalazioni: dati non esportati o approssimati.
    pub anomalie: Vec<Anomalia>,
}

/// Testo per un attributo XML: entità per `& < > " '`, a capo come CR LF, senza i
/// caratteri non ammessi in XML.
fn attributo(testo: &str) -> String {
    let mut s = String::with_capacity(testo.len());
    let mut precedente_cr = false;
    for c in testo.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&apos;"),
            '\r' => s.push_str("\r\n"),
            '\n' if precedente_cr => {}
            '\n' => s.push_str("\r\n"),
            '\t' => s.push(' '),
            // Caratteri non ammessi in XML 1.0 (controlli C0, U+FFFE, U+FFFF).
            c if u32::from(c) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => s.push(c),
        }
        precedente_cr = c == '\r';
    }
    s
}

/// Descrizione ridotta come `DesRid`: primi 57 caratteri, « ... », ultimi 58.
fn ridotta(testo: &str) -> String {
    let t: Vec<char> = testo.trim().chars().collect();
    if t.len() <= 120 {
        return t.iter().collect();
    }
    let inizio: String = t.iter().take(57).collect();
    let fine: String = t.iter().skip(t.len() - 58).collect();
    format!("{inizio} ... {fine}")
}

/// Numero con il punto decimale, scala estesa ai decimali indicati.
fn numero(v: Decimal, decimali: u32) -> String {
    let mut v = v;
    if v.scale() < decimali {
        v.rescale(decimali);
    }
    v.to_string()
}

fn elemento(nome: &str, attributi: &[(&str, String)], rientro: usize) -> String {
    let mut s = "\t".repeat(rientro);
    s.push('<');
    s.push_str(nome);
    for (k, v) in attributi {
        let _ = write!(s, " {k}=\"{}\"", attributo(v));
    }
    s.push_str(" />\r\n");
    s
}

fn raccolta_vuota(nome: &str) -> Vec<u8> {
    format!("\t<{nome} CountId=\"1\" />\r\n").into_bytes()
}

/// Voce di capitolo o categoria nella forma dei prezzari (`Id="0"` = «nessuna»).
fn voce_classe(tag: &str, id: &str, codice: &str, descrizione: &str) -> String {
    let attributi = [
        ("Id", id.to_owned()),
        ("DesSnt", descrizione.to_owned()),
        ("DesEst", String::new()),
        ("DataInit", DATA_NULLA.to_owned()),
        ("Drt", "0".to_owned()),
        ("CodFs", String::new()),
        ("Prcntl", "0".to_owned()),
        ("Cod", codice.to_owned()),
        ("SICInc", "0".to_owned()),
        ("MDOInc", "0".to_owned()),
        ("SICImp", "0".to_owned()),
        ("MDOImp", "0".to_owned()),
        ("BIMImp", "0".to_owned()),
        ("Grp", String::new()),
        ("IdxOGOS", "0".to_owned()),
    ];
    elemento(tag, &attributi, 2)
}

fn raccolta_classi(raccolta: &str, tag: &str, voci: &[(String, String, String)]) -> String {
    let mut s = format!("\t<{raccolta}>\r\n");
    s.push_str(&voce_classe(tag, "0", "", "<nessuna>"));
    for (id, codice, descrizione) in voci {
        s.push_str(&voce_classe(tag, id, codice, descrizione));
    }
    let _ = write!(s, "\t</{raccolta}>\r\n");
    s
}

/// Livello e identificativo DCF dei capitoli.
struct Capitoli {
    oltre_tre_livelli: usize,
    livello: HashMap<CapitoloId, usize>,
    id: HashMap<CapitoloId, String>,
    per_livello: [Vec<(String, String, String)>; 3],
}

fn capitoli(p: &Progetto) -> Capitoli {
    let ep = &p.elenco_prezzi;
    let originali: BTreeMap<String, String> = p
        .provenienza
        .as_ref()
        .filter(|pr| pr.formato == FormatoOrigine::Dcf)
        .map(|pr| pr.dati_originali.clone())
        .unwrap_or_default();
    let per_id: HashMap<CapitoloId, &CapitoloPrezzario> =
        ep.capitoli.iter().map(|c| (c.id, c)).collect();
    let profondita = |c: &CapitoloPrezzario| {
        let mut d = 0usize;
        let mut corrente = c.padre;
        while let Some(pid) = corrente {
            d += 1;
            corrente = per_id.get(&pid).and_then(|x| x.padre);
            if d > ep.capitoli.len() {
                break;
            }
        }
        d
    };
    let mut livello = HashMap::new();
    let mut oltre_tre_livelli = 0;
    for c in &ep.capitoli {
        let originale = originali
            .get(&format!("capitolo:{}/_livello", c.id))
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|l| *l <= 2);
        let l = originale.unwrap_or_else(|| {
            let d = profondita(c);
            if d > 2 {
                oltre_tre_livelli += 1;
            }
            d.min(2)
        });
        livello.insert(c.id, l);
    }
    let mut id = HashMap::new();
    let mut per_livello: [Vec<(String, String, String)>; 3] = Default::default();
    for (l, elenco) in per_livello.iter_mut().enumerate() {
        let del_livello: Vec<&CapitoloPrezzario> = ep
            .capitoli
            .iter()
            .filter(|c| livello.get(&c.id) == Some(&l))
            .collect();
        // Identificativi originali se tutti presenti, numerici e distinti.
        let orig: Vec<Option<String>> = del_livello
            .iter()
            .map(|c| {
                originali
                    .get(&format!("capitolo:{}/_id", c.id))
                    .filter(|v| v.parse::<u64>().is_ok_and(|n| n > 0))
                    .cloned()
            })
            .collect();
        let mut visti = std::collections::HashSet::new();
        let usa_orig = orig
            .iter()
            .all(|o| o.as_ref().is_some_and(|v| visti.insert(v.clone())));
        for (k, c) in del_livello.iter().enumerate() {
            let valore = if usa_orig {
                orig.get(k).cloned().flatten().unwrap_or_default()
            } else {
                (k + 1).to_string()
            };
            id.insert(c.id, valore.clone());
            elenco.push((valore, c.codice.clone(), c.descrizione.clone()));
        }
    }
    Capitoli {
        oltre_tre_livelli,
        livello,
        id,
        per_livello,
    }
}

impl Capitoli {
    /// `IdSpCap`, `IdCap`, `IdSbCap` della voce.
    fn della_voce(&self, p: &Progetto, v: &VocePrezzario) -> [String; 3] {
        let mut ids = [String::from("0"), String::from("0"), String::from("0")];
        let mut corrente = v.capitolo;
        let mut passi = 0;
        while let Some(cid) = corrente {
            if let (Some(l), Some(i)) = (self.livello.get(&cid), self.id.get(&cid))
                && let Some(slot) = ids.get_mut(*l)
                && slot == "0"
            {
                slot.clone_from(i);
            }
            corrente = p
                .elenco_prezzi
                .capitoli
                .iter()
                .find(|c| c.id == cid)
                .and_then(|c| c.padre);
            passi += 1;
            if passi > p.elenco_prezzi.capitoli.len() {
                break;
            }
        }
        // Voci importate da un DCF: i livelli superiori originali, se esistono ancora.
        if let Some(orig) = v
            .provenienza
            .as_ref()
            .filter(|pr| pr.formato == FormatoOrigine::Dcf)
        {
            for (l, campo) in ["IdSpCap", "IdCap", "IdSbCap"].into_iter().enumerate() {
                if let (Some(slot), Some(valore), Some(elenco)) = (
                    ids.get_mut(l),
                    orig.dati_originali.get(campo),
                    self.per_livello.get(l),
                ) && slot == "0"
                    && elenco.iter().any(|(i, _, _)| i == valore)
                {
                    slot.clone_from(valore);
                }
            }
        }
        ids
    }
}

fn collection_ep(p: &Progetto, cap: &Capitoli) -> String {
    let ep = &p.elenco_prezzi;
    // `IdEP` originali se tutte le voci vengono da un DCF con identificativi distinti.
    let originali: Option<Vec<u64>> = ep
        .voci
        .iter()
        .map(|v| {
            v.provenienza
                .as_ref()
                .filter(|pr| pr.formato == FormatoOrigine::Dcf)
                .and_then(|pr| pr.identificativo_originale.as_deref())
                .and_then(|i| i.parse::<u64>().ok())
                .filter(|i| *i > 0)
        })
        .collect();
    let ids: Vec<u64> = match originali {
        Some(o) if o.iter().collect::<std::collections::HashSet<_>>().len() == o.len() => o,
        _ => (0..ep.voci.len())
            .map(|k| u64::try_from(PRIMO_ID + k).unwrap_or(u64::MAX))
            .collect(),
    };
    let prossimo = ids.iter().max().map_or(1, |m| m.saturating_add(1));
    let mut s = format!("\t<CollectionEP CountId=\"{prossimo}\" IsIncAnls=\"false\" >\r\n");
    for (v, id_ep) in ep.voci.iter().zip(&ids) {
        let originali = v
            .provenienza
            .as_ref()
            .filter(|pr| pr.formato == FormatoOrigine::Dcf)
            .map(|pr| &pr.dati_originali);
        let estesa = if v.descrizione_estesa.trim().is_empty() {
            v.descrizione_breve.clone()
        } else {
            v.descrizione_estesa.clone()
        };
        // Voci lette da un DCF senza descrizione breve: il lettore ha preso `DesRid`.
        let breve_da_rid = originali
            .and_then(|o| o.get("_breve_da"))
            .is_some_and(|b| b == "DesRid");
        let (rid, brv) = if breve_da_rid {
            (v.descrizione_breve.clone(), String::new())
        } else if v.descrizione_breve.trim().is_empty() {
            (ridotta(&estesa), String::new())
        } else {
            (
                originali
                    .and_then(|o| o.get("DesRid"))
                    .cloned()
                    .unwrap_or_else(|| ridotta(&v.descrizione_breve)),
                v.descrizione_breve.clone(),
            )
        };
        let [sp, c, sb] = cap.della_voce(p, v);
        let incidenza = |x: Option<opencomputo_core::numeri::Percentuale>| {
            numero(x.map_or(Decimal::ZERO, |p| p.valore()), DECIMALI_INCIDENZE)
        };
        let mut fgs = originali
            .and_then(|o| o.get("Fgs"))
            .and_then(|f| f.parse::<u64>().ok())
            .unwrap_or(0);
        if v.natura == NaturaVoce::OneriSicurezza {
            fgs |= FLAG_VOCE_SICUREZZA;
        } else {
            fgs &= !FLAG_VOCE_SICUREZZA;
        }
        let calcolati: HashMap<&str, String> = [
            ("IdEP", id_ep.to_string()),
            ("Trf", v.codice.clone()),
            ("DesRid", rid),
            ("DesEst", estesa),
            ("DesBrv", brv),
            ("UnMsr", v.unita_misura.clone()),
            ("Prz1", v.prezzo.valore().to_string()),
            ("IdSpCap", sp),
            ("IdCap", c),
            ("IdSbCap", sb),
            ("Fgs", fgs.to_string()),
            ("SICInc", incidenza(v.incidenza_sicurezza)),
            ("MDOInc", incidenza(v.incidenza_manodopera)),
        ]
        .into_iter()
        .collect();
        let attributi: Vec<(&str, String)> = ATTRIBUTI_EP
            .iter()
            .map(|(nome, predefinito)| {
                let valore = calcolati.get(nome).cloned().unwrap_or_else(|| {
                    originali
                        .filter(|_| !CALCOLATI_EP.contains(nome))
                        .and_then(|o| o.get(*nome))
                        .cloned()
                        .unwrap_or_else(|| (*predefinito).to_owned())
                });
                (*nome, valore)
            })
            .collect();
        s.push_str(&elemento("ItemEP", &attributi, 2));
    }
    s.push_str("\t</CollectionEP>\r\n");
    s
}

fn dati_generali(p: &Progetto, cap: &Capitoli) -> String {
    let m = &p.metadati;
    let ep = &p.elenco_prezzi;
    let campo = |v: &Option<String>| v.as_deref().unwrap_or_default().trim().to_owned();
    let oggetto = m
        .oggetto
        .as_deref()
        .filter(|o| !o.trim().is_empty())
        .unwrap_or(&m.titolo)
        .trim()
        .to_owned();
    let mut s = String::from("\t<DatiGenerali> \r\n");
    let generali = [
        ("Oprtr", String::new()),
        ("PrcPrz", "0".to_owned()),
        ("Cmn", campo(&m.luogo)),
        ("Prvnc", String::new()),
        ("Ogg", oggetto),
        ("Comm", campo(&m.committente)),
        ("Imprs", campo(&m.impresa)),
        ("PrtOpr", String::new()),
        ("DsTrf", ep.fonte()),
    ];
    let mut riga = elemento("DGDatiGenerali", &generali, 1);
    riga.truncate(riga.len() - 5);
    s.push_str(&riga);
    s.push_str(" >\r\n\t<DGCollectionCncsn/>\r\n\t</DGDatiGenerali>\r\n");
    for (l, (raccolta, tag)) in [
        ("DGCollectionSpCap", "ItemSpCap"),
        ("DGCollectionCap", "ItemCap"),
        ("DGCollectionSbCap", "ItemSbCap"),
    ]
    .into_iter()
    .enumerate()
    {
        s.push_str(&raccolta_classi(
            raccolta,
            tag,
            cap.per_livello.get(l).map_or(&[], Vec::as_slice),
        ));
    }
    let classi = |elenco: &[Classe]| -> Vec<(String, String, String)> {
        elenco
            .iter()
            .enumerate()
            .map(|(k, c)| ((k + 1).to_string(), c.codice.clone(), c.descrizione.clone()))
            .collect()
    };
    s.push_str(&raccolta_classi(
        "DGCollectionSpCat",
        "ItemSpCat",
        &classi(&p.categorie.super_categorie),
    ));
    s.push_str(&raccolta_classi(
        "DGCollectionCat",
        "ItemCat",
        &classi(&p.categorie.categorie),
    ));
    s.push_str(&raccolta_classi(
        "DGCollectionSbCat",
        "ItemSbCat",
        &classi(&p.categorie.sotto_categorie),
    ));
    let pa = ep.parametri_analisi.unwrap_or_default();
    let percento = |x: Decimal| x.normalize().to_string();
    s.push_str(&elemento(
        "DGAnalisi",
        &[
            ("SpsUtl", "true".to_owned()),
            ("SpsGnl", percento(pa.spese_generali.valore())),
            ("UtlImprs", percento(pa.utile_impresa.valore())),
            ("OnrAccSC", "0".to_owned()),
        ],
        1,
    ));
    s.push_str(&elemento(
        "DGPassword",
        &[
            ("PswOpen", String::new()),
            ("PswMod", String::new()),
            ("BlcEp", String::new()),
            ("BlcMis", String::new()),
            ("BlcNrm", String::new()),
            ("BlcStmp", String::new()),
        ],
        1,
    ));
    let r = &p.regole;
    s.push_str(&elemento(
        "DGConfigurazione",
        &[
            ("Pu", "8.2|0".to_owned()),
            ("Lu", "8.2|0".to_owned()),
            ("La", "9.3|0".to_owned()),
            ("Hp", "9.3|0".to_owned()),
            ("TotQt", format!("10.{}|1", r.decimali_quantita)),
            ("PrzUn", format!("10.{}|1", r.decimali_prezzo)),
            ("PrzTot", format!("10.{}|1", r.decimali_importo)),
            ("CnvPrzUn", "11.0|1".to_owned()),
            ("CnvImpTot", "15.0|1".to_owned()),
            ("IncPrc", "7.3|0".to_owned()),
            ("Aliq", "7.3|0".to_owned()),
            ("Dvs", "euro".to_owned()),
        ],
        1,
    ));
    s.push_str("\t</DatiGenerali>\r\n");
    s
}

fn info_doc(p: &Progetto, o: &OpzioniExport) -> String {
    let m = &p.metadati;
    let campo = |v: &Option<String>| v.as_deref().unwrap_or_default().trim().to_owned();
    let mut s = elemento(
        "InfoDoc",
        &[
            ("CopyRight", String::new()),
            ("CopyRightFUPP", String::new()),
            ("TpDoc", "0".to_owned()),
            ("TpFrmt", "XMLPwe".to_owned()),
            ("Vrsn", VERSIONE_FORMATO.to_owned()),
            ("VrsnFix", VERSIONE_FIX.to_owned()),
            ("SrcNm", "OpenComputo".to_owned()),
            ("SrcVrsn", o.versione_programma.clone()),
            ("Cmd", "0".to_owned()),
            ("FlnmDoc", String::new()),
            ("Fgs", "4".to_owned()),
            ("TpCtDoc", "0".to_owned()),
            ("SystmPltfrm", String::new()),
            ("SrcLng", "IT".to_owned()),
            ("CdOff", String::new()),
            ("TpOFS", "4".to_owned()),
        ],
        2,
    );
    s.push_str(&elemento(
        "DGDatiGenerali",
        &[
            ("Cmn", campo(&m.luogo)),
            ("Prvnc", String::new()),
            ("Ogg", m.oggetto.clone().unwrap_or_else(|| m.titolo.clone())),
            ("Comm", campo(&m.committente)),
            ("Imprs", campo(&m.impresa)),
            ("PrtOpr", String::new()),
        ],
        1,
    ));
    s.push_str(&elemento(
        "DGPassword",
        &[("PswOpen", String::new()), ("PswMod", String::new())],
        1,
    ));
    s
}

/// Esporta l'elenco prezzi del progetto in un DCF in chiaro.
///
/// # Errors
/// Elenco vuoto o file oltre i limiti del contenitore.
pub fn esporta(p: &Progetto, o: &OpzioniExport) -> Result<RisultatoExport, ErroreDcf> {
    let ep = &p.elenco_prezzi;
    if ep.voci.is_empty() {
        return Err(ErroreDcf::NessunDato("l'elenco prezzi è vuoto".into()));
    }
    let cap = capitoli(p);
    let mut anomalie = Vec::new();
    let mut segnala = |gravita, codice: &str, messaggio: String| {
        anomalie.push(Anomalia {
            gravita,
            codice: codice.to_owned(),
            messaggio,
            posizione: None,
        });
    };
    segnala(
        Gravita::Avviso,
        "dcf.export_sperimentale",
        "export DCF sperimentale: PriMus non apre i file scritti da OpenComputo (prova del \
         2026-10-21); per PriMus usare l'export XPWE"
            .into(),
    );
    let analizzate = ep.voci.iter().filter(|v| v.analisi.is_some()).count();
    if analizzate > 0 {
        segnala(
            Gravita::Informazione,
            "dcf.analisi_non_esportate",
            format!("{analizzate} voci con analisi: esportato il prezzo, non l'analisi"),
        );
    }
    if !p.computi.iter().all(|c| c.voci.is_empty()) {
        segnala(
            Gravita::Informazione,
            "dcf.computo_non_esportato",
            "il computo non è esportato in DCF: per il computo usare l'export XPWE".into(),
        );
    }
    let gerarchia_ridotta = cap.oltre_tre_livelli;
    if gerarchia_ridotta > 0 {
        segnala(
            Gravita::Avviso,
            "dcf.capitoli_oltre_tre_livelli",
            format!(
                "{gerarchia_ridotta} capitoli oltre il terzo livello scritti come terzo livello"
            ),
        );
    }

    let mut elementi = vec![Elemento::Cartella {
        nome: "Documento".into(),
        genitore: None,
    }];
    let mut file = |nome: &str, contenuto: Vec<u8>| {
        elementi.push(Elemento::File {
            nome: nome.into(),
            genitore: Some(0),
            contenuto,
        });
    };
    file("CollectionEP.xml", collection_ep(p, &cap).into_bytes());
    file("CollectionGP.xml", raccolta_vuota("CollectionGP"));
    file("CollectionOD.xml", raccolta_vuota("CollectionOD"));
    file("CollectionVC.xml", raccolta_vuota("CollectionVC"));
    file("DatiGenerali.xml", dati_generali(p, &cap).into_bytes());
    file("InfoDoc.xml", info_doc(p, o).into_bytes());
    file(
        "InfoMRR.xml",
        b"\t<InfoMRR PsTtlStrtt=\"0\" PsRcctStrtt=\"0\" PsTtlNoStrtt=\"0\" PsRccltNoStrtt=\"0\" Fgs=\"0\" />\r\n"
            .to_vec(),
    );
    elementi.push(Elemento::File {
        nome: "Versione".into(),
        genitore: None,
        contenuto: VERSIONE_F64.to_vec(),
    });
    elementi.push(Elemento::File {
        nome: "VersioneFix".into(),
        genitore: None,
        contenuto: VERSIONE_FIX_F64.to_vec(),
    });
    let byte = scrittura::scrivi(&elementi, o.data)?;
    Ok(RisultatoExport {
        byte,
        voci: ep.voci.len(),
        capitoli: ep.capitoli.len(),
        anomalie,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testi_e_numeri() {
        assert_eq!(attributo("a&b<c>\"d'"), "a&amp;b&lt;c&gt;&quot;d&apos;");
        assert_eq!(attributo("uno\ndue\r\ntre\tq\u{1}"), "uno\r\ndue\r\ntre q");
        assert_eq!(ridotta("breve"), "breve");
        let lunga: String = (0..200)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let r = ridotta(&lunga);
        assert_eq!(r.chars().count(), 57 + 5 + 58);
        assert_eq!(numero(Decimal::new(7076, 2), 18), "70.760000000000000000");
        assert_eq!(numero(Decimal::new(850, 2), 0), "8.50");
        // 5,01 e 1,07 in doppia precisione.
        assert_eq!(u64::from_le_bytes(VERSIONE_F64), 0x4014_0A3D_70A3_D70A);
        assert_eq!(u64::from_le_bytes(VERSIONE_FIX_F64), 0x3FF1_1EB8_51EB_851F);
    }
}
