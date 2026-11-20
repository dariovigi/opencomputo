//! Import dei dati di un DCF non protetto in un [`Progetto`].
//!
//! Mappatura ricavata dai file analizzati (vedi `docs/adr/0006-formati-primus.md`):
//!
//! | Flusso | Elemento | Attributi usati |
//! |--------|----------|-----------------|
//! | `InfoDoc` | `DGDatiGenerali` | `PswOpen` (se valorizzato il file è rifiutato), `PswMod` |
//! | `DatiGenerali` | `DGDatiGenerali` | `Ogg`, `Cmn`, `Comm`, `Imprs` |
//! | `DatiGenerali` | `DGCollectionSpCap/Cap/SbCap` → `ItemSpCap/Cap/SbCap` | `Id` (0 = «nessuno»), `Cod`, `DesSnt`, `DesEst` |
//! | `DatiGenerali` | `DGCollectionSpCat/Cat/SbCat` → `ItemSpCat/Cat/SbCat` | come i capitoli |
//! | `CollectionEP` | `ItemEP` | `IdEP`, `Trf`, `DesBrv`/`DesRid`, `DesEst`, `UnMsr`, `Prz1`, `Fgs`, `MDOInc`, `SICInc`, `IdSpCap`, `IdCap`, `IdSbCap` |
//!
//! Gli altri attributi delle voci, se diversi dal valore predefinito, sono conservati
//! nella provenienza. Le incidenze sono scritte nei DCF con artefatti di virgola mobile
//! (es. `62.159999999999996600`): sono arrotondate a 6 decimali e il testo originale è
//! conservato. La natura «oneri della sicurezza» è dedotta dal bit `134217728` di
//! `Fgs`, come in XPWE: **ipotesi**, nessuna voce dei file analizzati usa quel bit.
//! Le voci di computo (`CollectionVC`) e le analisi non sono importate.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use jiff::civil::DateTime;
use opencomputo_core::classificazione::Classe;
use opencomputo_core::id::{CapitoloId, GeneratoreId, VocePrezzarioId};
use opencomputo_core::numeri::{Percentuale, Prezzo};
use opencomputo_core::prezzario::{CapitoloPrezzario, NaturaVoce, Prezzario, VocePrezzario};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::provenienza::{Anomalia, FormatoOrigine, Provenienza};
use opencomputo_core::validazione::Gravita;
use rust_decimal::{Decimal, RoundingStrategy};
use sha2::{Digest, Sha256};

use crate::contenitore::{self, Contenuto};
use crate::errore::ErroreDcf;
use crate::xml::{Elemento, per_elementi};

/// Bit di `Fgs` assunto per le voci della sicurezza (come `Flags` in XPWE).
pub const FLAG_VOCE_SICUREZZA: u64 = 1 << 27;
/// Decimali a cui sono arrotondate le incidenze.
pub const DECIMALI_INCIDENZE: u32 = 6;

/// Opzioni di import.
#[derive(Debug, Clone, Default)]
pub struct OpzioniImport {
    /// Nome del file di origine.
    pub nome_file: Option<String>,
    /// Momento dell'importazione.
    pub importato_il: Option<DateTime>,
    /// Nome da dare all'elenco prezzi (altrimenti l'oggetto del documento).
    pub nome_prezzario: Option<String>,
}

/// Conteggi dell'import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statistiche {
    /// Flussi del contenitore.
    pub flussi: usize,
    /// Flussi protetti (non letti).
    pub flussi_protetti: usize,
    /// Flussi non decomprimibili.
    pub flussi_illeggibili: usize,
    /// Voci di elenco lette nel file.
    pub voci_lette: usize,
    /// Voci di elenco importate.
    pub voci_elenco: usize,
    /// Voci senza tariffa (testi descrittivi) non importate.
    pub voci_senza_tariffa: usize,
    /// Voci scartate per dati non validi.
    pub voci_scartate: usize,
    /// Capitoli importati (tutti i livelli).
    pub capitoli: usize,
    /// Categorie importate (tutti i livelli).
    pub categorie: usize,
    /// Incidenze arrotondate per eliminare artefatti di virgola mobile.
    pub incidenze_arrotondate: usize,
}

/// Rapporto dell'import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportoImport {
    /// Anomalie.
    pub anomalie: Vec<Anomalia>,
    /// Conteggi.
    pub statistiche: Statistiche,
}

impl RapportoImport {
    /// `true` se almeno un'anomalia è un errore.
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
    /// Progetto con l'elenco prezzi importato (nessun computo).
    pub progetto: Progetto,
    /// Rapporto.
    pub rapporto: RapportoImport,
}

#[derive(Default)]
struct Anomalie {
    elenco: Vec<Anomalia>,
    aggregate: BTreeMap<(String, String), (Gravita, String, usize, String)>,
}

impl Anomalie {
    fn aggiungi(
        &mut self,
        gravita: Gravita,
        codice: &str,
        messaggio: impl Into<String>,
        posizione: Option<String>,
    ) {
        self.elenco.push(Anomalia {
            gravita,
            codice: codice.into(),
            messaggio: messaggio.into(),
            posizione,
        });
    }

    fn aggregata(
        &mut self,
        gravita: Gravita,
        codice: &str,
        chiave: &str,
        messaggio: &str,
        posizione: String,
    ) {
        self.aggregate
            .entry((codice.to_owned(), chiave.to_owned()))
            .and_modify(|a| a.2 += 1)
            .or_insert((gravita, messaggio.to_owned(), 1, posizione));
    }

    fn fine(mut self) -> Vec<Anomalia> {
        for ((codice, _), (gravita, messaggio, n, posizione)) in self.aggregate {
            self.elenco.push(Anomalia {
                gravita,
                codice,
                messaggio: if n > 1 {
                    format!("{messaggio} ({n} occorrenze, la prima in {posizione})")
                } else {
                    messaggio
                },
                posizione: Some(posizione),
            });
        }
        self.elenco
    }
}

fn decimale(testo: &str) -> Result<Option<Decimal>, String> {
    let t = testo.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.contains('.') && t.contains(',') {
        return Err(format!(
            "«{t}» è ambiguo: contiene sia il punto sia la virgola"
        ));
    }
    let n = t.replace(',', ".");
    Decimal::from_str_exact(&n)
        .or_else(|_| n.parse::<Decimal>())
        .map(Some)
        .map_err(|_| format!("«{t}» non è un numero"))
}

/// Valori che non vale la pena conservare tra i dati originali.
fn predefinito(valore: &str) -> bool {
    matches!(
        valore.trim(),
        "" | "0" | "false" | "30/12/1899" | "0.000000000000000000"
    )
}

/// Impronta SHA-256 esadecimale.
#[must_use]
pub fn impronta(dati: &[u8]) -> String {
    Sha256::digest(dati)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            s.push_str(&format!("{b:02x}"));
            s
        })
}

const ELENCHI_CAPITOLI: [(&str, &str); 3] = [
    ("DGCollectionSpCap", "ItemSpCap"),
    ("DGCollectionCap", "ItemCap"),
    ("DGCollectionSbCap", "ItemSbCap"),
];
const ELENCHI_CATEGORIE: [(&str, &str); 3] = [
    ("DGCollectionSpCat", "ItemSpCat"),
    ("DGCollectionCat", "ItemCat"),
    ("DGCollectionSbCat", "ItemSbCat"),
];
const MAPPATI_EP: [&str; 13] = [
    "IdEP", "Trf", "DesBrv", "DesRid", "DesEst", "UnMsr", "Prz1", "Fgs", "MDOInc", "SICInc",
    "IdSpCap", "IdCap", "IdSbCap",
];

/// Importa l'elenco prezzi (con capitoli, categorie e dati generali) di un DCF.
///
/// # Errors
/// File non DCF o danneggiato; documento protetto o con password di apertura
/// ([`ErroreDcf::Protetto`], [`ErroreDcf::ProtettoDaPassword`]); nessun dato
/// importabile.
pub fn importa<G: GeneratoreId>(
    dati: &[u8],
    generatore: &mut G,
    opzioni: &OpzioniImport,
) -> Result<RisultatoImport, ErroreDcf> {
    let c = contenitore::leggi(dati)?;
    let mut an = Anomalie::default();
    let mut st = Statistiche {
        flussi: c.flussi.len(),
        flussi_protetti: c.protetti(),
        ..Statistiche::default()
    };
    if c.settori_dichiarati != c.settori {
        an.aggiungi(
            Gravita::Avviso,
            "dcf.settori_incoerenti",
            format!(
                "la testata dichiara {} settori, il file ne contiene {}",
                c.settori_dichiarati, c.settori
            ),
            None,
        );
    }
    for f in &c.flussi {
        if let Contenuto::Illeggibile(motivo) = &f.contenuto {
            st.flussi_illeggibili += 1;
            an.aggiungi(
                Gravita::Avviso,
                "dcf.flusso_illeggibile",
                format!("flusso non decomprimibile: {motivo}"),
                Some(format!("settore {}", f.settore)),
            );
        }
    }

    // Password di apertura: rispettata, il file non viene letto.
    let mut doc = BTreeMap::new();
    for testo in c.xml("InfoDoc") {
        let mut password = false;
        per_elementi(testo, |e| {
            if e.nome == "DGDatiGenerali" {
                if e.attributo("PswOpen").is_some_and(|p| !p.is_empty()) {
                    password = true;
                }
                if e.attributo("PswMod").is_some_and(|p| !p.is_empty()) {
                    doc.insert("InfoDoc/PswMod".to_owned(), "presente".to_owned());
                }
                for k in ["TpDoc", "Vrsn", "SrcNm", "SrcVrsn"] {
                    if let Some(v) = e.attributo(k).filter(|v| !predefinito(v)) {
                        doc.insert(format!("InfoDoc/{k}"), v.to_owned());
                    }
                }
            }
        })
        .map_err(ErroreDcf::Danneggiato)?;
        if password {
            return Err(ErroreDcf::ProtettoDaPassword);
        }
    }
    if doc.contains_key("InfoDoc/PswMod") {
        an.aggiungi(
            Gravita::Informazione,
            "dcf.password_modifica",
            "il documento ha una password di modifica: OpenComputo lo legge senza modificarlo",
            None,
        );
    }

    let ha_elenco = c.xml("CollectionEP").next().is_some();
    let ha_generali = c.xml("DatiGenerali").next().is_some();
    if st.flussi_protetti > 0 {
        if !ha_elenco && !ha_generali {
            return Err(ErroreDcf::Protetto {
                flussi: st.flussi_protetti,
            });
        }
        an.aggiungi(
            Gravita::Avviso,
            "dcf.flussi_protetti",
            format!(
                "{} flussi protetti non letti: i dati importati potrebbero essere incompleti",
                st.flussi_protetti
            ),
            None,
        );
    }
    if !ha_elenco && !ha_generali {
        return Err(ErroreDcf::NessunDato(
            "mancano sia l'elenco prezzi sia i dati generali".into(),
        ));
    }

    let progetto_id = generatore.nuovo();
    let prezzario_id = generatore.nuovo();
    let mut progetto = Progetto::new(progetto_id, "", prezzario_id);

    // Dati generali, capitoli, categorie.
    let mut capitoli: HashMap<(usize, String), CapitoloId> = HashMap::new();
    for testo in c.xml("DatiGenerali") {
        let mut errore = None;
        per_elementi(testo, |e| {
            if e.nome == "DGDatiGenerali" && e.genitore() == Some("DatiGenerali") {
                let campo = |k: &str| {
                    e.attributo(k)
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_owned)
                };
                progetto.metadati.oggetto = campo("Ogg");
                progetto.metadati.titolo = campo("Ogg").unwrap_or_default();
                progetto.metadati.luogo = campo("Cmn");
                progetto.metadati.committente = campo("Comm");
                progetto.metadati.impresa = campo("Imprs");
                for (k, v) in e.attributi {
                    if !["Ogg", "Cmn", "Comm", "Imprs"].contains(&k.as_str()) && !predefinito(v) {
                        doc.insert(format!("DGDatiGenerali/{k}"), v.clone());
                    }
                }
                return;
            }
            let Some(genitore) = e.genitore() else { return };
            if let Some(livello) = ELENCHI_CAPITOLI
                .iter()
                .position(|(g, n)| *g == genitore && *n == e.nome)
            {
                if let Some((id, codice, descrizione)) = classe(e) {
                    let cid: CapitoloId = generatore.nuovo();
                    if capitoli.insert((livello, id.clone()), cid).is_some() {
                        errore = Some(format!("{} con Id {id} duplicato", e.nome));
                    }
                    doc.insert(format!("capitolo:{cid}/_livello"), livello.to_string());
                    doc.insert(format!("capitolo:{cid}/_id"), id);
                    progetto.elenco_prezzi.capitoli.push(CapitoloPrezzario {
                        id: cid,
                        padre: None,
                        codice,
                        descrizione,
                    });
                    st.capitoli += 1;
                }
            } else if let Some(livello) = ELENCHI_CATEGORIE
                .iter()
                .position(|(g, n)| *g == genitore && *n == e.nome)
                && let Some((id, codice, descrizione)) = classe(e)
            {
                let cid = generatore.nuovo();
                doc.insert(format!("classe:{cid}/_id"), id);
                let elenco = match livello {
                    0 => &mut progetto.categorie.super_categorie,
                    1 => &mut progetto.categorie.categorie,
                    _ => &mut progetto.categorie.sotto_categorie,
                };
                elenco.push(Classe {
                    id: cid,
                    codice,
                    descrizione,
                });
                st.categorie += 1;
            }
        })
        .map_err(ErroreDcf::Danneggiato)?;
        if let Some(e) = errore {
            an.aggiungi(Gravita::Avviso, "dcf.id_duplicato", e, None);
        }
    }

    let titolo = opzioni
        .nome_prezzario
        .clone()
        .or_else(|| progetto.metadati.oggetto.clone())
        .or_else(|| opzioni.nome_file.clone())
        .unwrap_or_else(|| "Prezzario DCF".into());
    if progetto.metadati.titolo.is_empty() {
        progetto.metadati.titolo.clone_from(&titolo);
    }
    progetto.elenco_prezzi = Prezzario {
        capitoli: std::mem::take(&mut progetto.elenco_prezzi.capitoli),
        ..Prezzario::new(prezzario_id, titolo)
    };
    let mut prov_prezzario = Provenienza::new(FormatoOrigine::Dcf);
    prov_prezzario.file.clone_from(&opzioni.nome_file);
    progetto.elenco_prezzi.provenienza = Some(prov_prezzario);

    // Elenco prezzi.
    let mut id_viste: HashMap<String, usize> = HashMap::new();
    let mut annidati = 0usize;
    for testo in c.xml("CollectionEP") {
        per_elementi(testo, |e| {
            if e.genitore() != Some("CollectionEP") {
                if e.genitori.iter().any(|g| g == "ItemEP") {
                    annidati += 1;
                }
                return;
            }
            if e.nome != "ItemEP" {
                return;
            }
            st.voci_lette += 1;
            let posizione = format!("CollectionEP, riga {}", e.riga);
            importa_voce(
                e,
                &posizione,
                generatore,
                &mut progetto,
                &capitoli,
                &mut id_viste,
                &mut an,
                &mut st,
            );
        })
        .map_err(ErroreDcf::Danneggiato)?;
    }
    if annidati > 0 {
        an.aggiungi(
            Gravita::Informazione,
            "dcf.analisi_non_importata",
            format!("{annidati} elementi annidati nelle voci (analisi) non importati: vale il prezzo dichiarato"),
            None,
        );
    }
    if st.voci_senza_tariffa > 0 {
        an.aggiungi(
            Gravita::Informazione,
            "dcf.voci_senza_tariffa",
            format!(
                "{} voci senza tariffa (testi descrittivi, es. avvertenze) non importate come voci di prezzo",
                st.voci_senza_tariffa
            ),
            None,
        );
    }
    if st.incidenze_arrotondate > 0 {
        an.aggiungi(
            Gravita::Informazione,
            "dcf.incidenze_arrotondate",
            format!(
                "{} incidenze arrotondate a {DECIMALI_INCIDENZE} decimali per eliminare artefatti di virgola mobile (testo originale conservato)",
                st.incidenze_arrotondate
            ),
            None,
        );
    }

    // Voci di computo: presenti ma non lette.
    for testo in c.xml("CollectionVC") {
        let mut voci = 0usize;
        per_elementi(testo, |e| {
            if e.genitore() == Some("CollectionVC") {
                voci += 1;
            }
        })
        .map_err(ErroreDcf::Danneggiato)?;
        if voci > 0 {
            an.aggiungi(
                Gravita::Avviso,
                "dcf.computo_non_importato",
                format!("{voci} voci di computo presenti ma non importate: per il computo usare l'export XPWE di PriMus"),
                None,
            );
        }
    }

    let mut altri: Vec<&str> = c
        .flussi
        .iter()
        .filter_map(|f| match &f.contenuto {
            Contenuto::Xml { radice, .. }
                if !["InfoDoc", "DatiGenerali", "CollectionEP", "CollectionVC"]
                    .contains(&radice.as_str()) =>
            {
                Some(radice.as_str())
            }
            _ => None,
        })
        .collect();
    altri.sort_unstable();
    altri.dedup();
    if !altri.is_empty() {
        an.aggiungi(
            Gravita::Informazione,
            "dcf.flussi_non_interpretati",
            format!("flussi non interpretati: {}", altri.join(", ")),
            None,
        );
    }

    let anomalie = an.fine();
    let mut prov = Provenienza::new(FormatoOrigine::Dcf);
    prov.file.clone_from(&opzioni.nome_file);
    prov.impronta_sha256 = Some(impronta(dati));
    prov.importato_il = opzioni.importato_il;
    prov.anomalie.clone_from(&anomalie);
    prov.dati_originali = doc;
    progetto.provenienza = Some(prov);
    Ok(RisultatoImport {
        progetto,
        rapporto: RapportoImport {
            anomalie,
            statistiche: st,
        },
    })
}

/// (Id, codice, descrizione) di capitoli e categorie; `Id = 0` è il segnaposto «nessuno».
fn classe(e: &Elemento<'_>) -> Option<(String, String, String)> {
    let id = e.attributo("Id")?.trim();
    if id.is_empty() || id == "0" {
        return None;
    }
    let sintetica = e.attributo("DesSnt").unwrap_or_default().trim();
    let descrizione = if sintetica.is_empty() {
        e.attributo("DesEst").unwrap_or_default().trim()
    } else {
        sintetica
    };
    Some((
        id.to_owned(),
        e.attributo("Cod").unwrap_or_default().trim().to_owned(),
        descrizione.to_owned(),
    ))
}

fn incidenza(
    e: &Elemento<'_>,
    nome: &str,
    dati: &mut BTreeMap<String, String>,
    st: &mut Statistiche,
    an: &mut Anomalie,
    posizione: &str,
) -> Option<Percentuale> {
    let testo = e.attributo(nome).unwrap_or_default();
    match decimale(testo) {
        Ok(Some(v)) if !v.is_zero() => {
            let arrotondato = v
                .round_dp_with_strategy(DECIMALI_INCIDENZE, RoundingStrategy::MidpointAwayFromZero)
                .normalize();
            if arrotondato != v {
                st.incidenze_arrotondate += 1;
                dati.insert(nome.to_owned(), testo.to_owned());
            }
            Some(Percentuale::new(arrotondato))
        }
        Ok(_) => None,
        Err(m) => {
            an.aggregata(
                Gravita::Avviso,
                "dcf.numero_non_valido",
                nome,
                &format!("{nome}: {m}; campo ignorato"),
                posizione.to_owned(),
            );
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn importa_voce<G: GeneratoreId>(
    e: &Elemento<'_>,
    posizione: &str,
    generatore: &mut G,
    progetto: &mut Progetto,
    capitoli: &HashMap<(usize, String), CapitoloId>,
    id_viste: &mut HashMap<String, usize>,
    an: &mut Anomalie,
    st: &mut Statistiche,
) {
    let codice = e.attributo("Trf").unwrap_or_default().trim().to_owned();
    if codice.is_empty() {
        st.voci_senza_tariffa += 1;
        return;
    }
    let id_dcf = e.attributo("IdEP").unwrap_or_default().trim().to_owned();
    if let Some(prima) = id_viste.insert(id_dcf.clone(), e.riga) {
        an.aggiungi(
            Gravita::Errore,
            "dcf.id_duplicato",
            format!("voce «{codice}» con IdEP {id_dcf} già usato alla riga {prima}: scartata"),
            Some(posizione.to_owned()),
        );
        st.voci_scartate += 1;
        return;
    }
    let prezzo = match decimale(e.attributo("Prz1").unwrap_or_default()) {
        Ok(Some(p)) => p,
        Ok(None) => {
            an.aggiungi(
                Gravita::Errore,
                "dcf.prezzo_mancante",
                format!("voce «{codice}» senza Prz1: scartata"),
                Some(posizione.to_owned()),
            );
            st.voci_scartate += 1;
            return;
        }
        Err(m) => {
            an.aggiungi(
                Gravita::Errore,
                "dcf.numero_non_valido",
                format!("voce «{codice}», Prz1: {m}; voce scartata"),
                Some(posizione.to_owned()),
            );
            st.voci_scartate += 1;
            return;
        }
    };
    let mut dati = BTreeMap::new();
    for (k, v) in e.attributi {
        if !MAPPATI_EP.contains(&k.as_str()) && !predefinito(v) {
            dati.insert(k.clone(), v.clone());
        }
    }
    let breve = {
        let b = e.attributo("DesBrv").unwrap_or_default().trim();
        if b.is_empty() {
            let r = e.attributo("DesRid").unwrap_or_default().trim();
            if !r.is_empty() {
                // Per l'export: la descrizione breve viene da `DesRid`.
                dati.insert("_breve_da".into(), "DesRid".into());
            }
            r
        } else {
            if let Some(r) = e
                .attributo("DesRid")
                .filter(|r| !r.trim().is_empty() && r.trim() != b)
            {
                dati.insert("DesRid".into(), r.to_owned());
            }
            b
        }
    };
    let fgs = e
        .attributo("Fgs")
        .and_then(|f| f.trim().parse::<u64>().ok())
        .unwrap_or(0);
    if fgs != 0 {
        dati.insert("Fgs".into(), fgs.to_string());
    }
    let natura = if fgs & FLAG_VOCE_SICUREZZA != 0 {
        NaturaVoce::OneriSicurezza
    } else {
        NaturaVoce::Lavoro
    };
    let mut capitolo = None;
    for (livello, campo) in ["IdSpCap", "IdCap", "IdSbCap"].into_iter().enumerate() {
        let r = e.attributo(campo).unwrap_or_default().trim();
        if r.is_empty() || r == "0" {
            continue;
        }
        dati.insert(campo.to_owned(), r.to_owned());
        match capitoli.get(&(livello, r.to_owned())) {
            Some(c) => capitolo = Some(*c),
            None => an.aggregata(
                Gravita::Avviso,
                "dcf.riferimento_non_risolto",
                campo,
                &format!("{campo} non corrisponde ad alcun capitolo: riferimento conservato"),
                posizione.to_owned(),
            ),
        }
    }
    if progetto
        .elenco_prezzi
        .voci
        .iter()
        .any(|v| v.codice == codice)
    {
        an.aggregata(
            Gravita::Avviso,
            "dcf.tariffa_duplicata",
            "Trf",
            "tariffa già presente nell'elenco: importata comunque",
            posizione.to_owned(),
        );
    }
    let incidenza_manodopera = incidenza(e, "MDOInc", &mut dati, st, an, posizione);
    let incidenza_sicurezza = incidenza(e, "SICInc", &mut dati, st, an, posizione);
    let mut prov = Provenienza::new(FormatoOrigine::Dcf);
    prov.identificativo_originale = (!id_dcf.is_empty()).then_some(id_dcf);
    prov.dati_originali = dati;
    let id: VocePrezzarioId = generatore.nuovo();
    progetto.elenco_prezzi.voci.push(Arc::new(VocePrezzario {
        id,
        codice,
        descrizione_breve: breve.to_owned(),
        descrizione_estesa: e.attributo("DesEst").unwrap_or_default().trim().to_owned(),
        unita_misura: e.attributo("UnMsr").unwrap_or_default().trim().to_owned(),
        prezzo: Prezzo::new(prezzo),
        natura,
        incidenza_manodopera,
        incidenza_sicurezza,
        capitolo,
        analisi: None,
        nuovo_prezzo: None,
        provenienza: Some(prov),
    }));
    st.voci_elenco += 1;
}
