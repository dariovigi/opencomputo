//! Costruzione dei report dal modello di dominio.
//!
//! Ogni funzione produce un [`Documento`] con i valori calcolati dal core: i report
//! non ricalcolano nulla per conto proprio. Quantità, importi e totali sono quelli di
//! [`Computo::calcola`](opencomputo_core::computo::Computo::calcola), del libretto,
//! del registro e dei SAL, con gli stessi arrotondamenti.

use chrono::NaiveDate;
use std::collections::BTreeMap;

use opencomputo_core::classificazione::LivelloCategoria;
use opencomputo_core::computo::{Computo, QuoteRaggruppate, RiepilogoComputo, TotaliRaggruppati};
use opencomputo_core::contabilita::{RiepilogoSal, StatoSal};
use opencomputo_core::id::{ClasseId, ComputoId, SalId, WbsNodoId};
use opencomputo_core::misura::{QuantitaRiferite, RigoMisura, SenzaRiferimenti, TipoRigo};
use opencomputo_core::numeri::{Importo, RegoleArrotondamento};
use opencomputo_core::partita::{CalcoloPartita, PrezzoApplicato};
use opencomputo_core::prezzario::NaturaVoce;
use opencomputo_core::progetto::Progetto;
use rust_decimal::Decimal;
use thiserror::Error;

use crate::documento::{Colonna, Documento, Sezione, StileRiga, Tabella, Valore};
use crate::formato;

/// Errore nella costruzione di un report.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreReport {
    /// Il computo richiesto non esiste.
    #[error("computo inesistente: {0}")]
    ComputoInesistente(String),
    /// Il SAL richiesto non esiste.
    #[error("SAL inesistente: {0}")]
    SalInesistente(String),
    /// I dati non sono calcolabili: il report non viene prodotto con valori parziali.
    #[error("dati non calcolabili: {0}")]
    Calcolo(String),
}

/// Opzioni comuni dei report.
#[derive(Debug, Clone, Default)]
pub struct OpzioniReport {
    /// Data di stampa, riportata a piè di pagina (il core non legge l'orologio).
    pub data_stampa: Option<NaiveDate>,
}

/// Decimali minimi dei fattori di misura.
const DECIMALI_FATTORI: u32 = 2;
/// Decimali delle percentuali di incidenza.
const DECIMALI_INCIDENZE: u32 = 2;

fn intestazione(p: &Progetto) -> Vec<(String, String)> {
    let m = &p.metadati;
    let mut v = Vec::new();
    let mut aggiungi = |etichetta: &str, valore: Option<&str>| {
        if let Some(x) = valore.map(str::trim).filter(|x| !x.is_empty()) {
            v.push((etichetta.to_owned(), x.to_owned()));
        }
    };
    let oggetto = m.oggetto.as_deref().unwrap_or(&m.titolo);
    aggiungi("Oggetto", Some(oggetto));
    aggiungi("Committente", m.committente.as_deref());
    aggiungi("Impresa", m.impresa.as_deref());
    aggiungi("Luogo", m.luogo.as_deref());
    aggiungi("CIG", m.cig.as_deref());
    aggiungi("CUP", m.cup.as_deref());
    aggiungi(
        "Responsabile del procedimento",
        m.responsabile_procedimento.as_deref(),
    );
    aggiungi("Progettista", m.progettista.as_deref());
    aggiungi("Direttore dei lavori", m.direttore_lavori.as_deref());
    v
}

fn piede(o: &OpzioniReport) -> Option<String> {
    o.data_stampa
        .map(|d| format!("Stampato il {}", formato::data(d)))
}

pub(crate) fn documento(
    p: &Progetto,
    titolo: &str,
    orizzontale: bool,
    o: &OpzioniReport,
) -> Documento {
    Documento {
        titolo: titolo.to_owned(),
        intestazione: intestazione(p),
        sezioni: Vec::new(),
        orizzontale,
        piede: piede(o),
    }
}

fn descrizione(prezzo: &PrezzoApplicato) -> String {
    if prezzo.descrizione_estesa.trim().is_empty() {
        prezzo.descrizione_breve.clone()
    } else {
        prezzo.descrizione_estesa.clone()
    }
}

fn natura(n: NaturaVoce) -> &'static str {
    match n {
        NaturaVoce::Lavoro => "lavori",
        NaturaVoce::OneriSicurezza => "sicurezza",
    }
}

pub(crate) fn errore_calcolo(e: impl std::fmt::Display) -> ErroreReport {
    ErroreReport::Calcolo(e.to_string())
}

/// Colonne del foglio di misura: N., codice, designazione, quattro fattori,
/// quantità e, se richiesti, prezzo e importo (il libretto delle misure non li riporta).
fn colonne_misure(con_prezzi: bool) -> Vec<Colonna> {
    let mut c = vec![
        Colonna::centrata("N.", 5),
        Colonna::testo("Codice", 8),
        Colonna::testo("Designazione dei lavori", 50),
        Colonna::numero("Par. ug.", 7),
        Colonna::numero("Lung.", 7),
        Colonna::numero("Larg.", 7),
        Colonna::numero("H/peso", 7),
        Colonna::numero("Quantità", 9),
    ];
    if con_prezzi {
        c.push(Colonna::numero("Prezzo €", 9));
        c.push(Colonna::numero("Importo €", 11));
    }
    c
}

/// Voci citate dai «vedi voce»: il computo (per i numeri d'ordine) e le quantità.
#[derive(Clone, Copy)]
pub(crate) struct Citate<'a> {
    pub(crate) computo: Option<&'a Computo>,
    pub(crate) quantita: &'a dyn QuantitaRiferite,
}

impl Citate<'_> {
    /// Nessuna voce citabile (libretto): solo «vedi voce» statici.
    pub(crate) const NESSUNA: Citate<'static> = Citate {
        computo: None,
        quantita: &SenzaRiferimenti,
    };
}

/// Righi di misura con fattori valutati e quantità parziale.
fn righi_misura(
    t: &mut Tabella,
    righi: &[RigoMisura],
    regole: &RegoleArrotondamento,
    citate: Citate<'_>,
) -> Result<(), ErroreReport> {
    for rigo in righi {
        if rigo.tipo == TipoRigo::Commento {
            t.riga_testo(StileRiga::Commento, 2, rigo.descrizione.clone());
            continue;
        }
        let c = rigo.contesto().map_err(errore_calcolo)?;
        let r = rigo
            .calcola_con(regole, citate.quantita)
            .map_err(errore_calcolo)?;
        let vedi = rigo.vedi_voce.as_ref().map(|v| {
            crate::vedi_voce::testo(v, citate.computo, citate.quantita, regole.decimali_quantita)
        });
        let mut testo = crate::vedi_voce::con_descrizione(vedi, &rigo.descrizione);
        if let Some(f) = rigo.formula.as_deref().filter(|f| !f.trim().is_empty()) {
            if !testo.is_empty() {
                testo.push(' ');
            }
            testo.push_str(&format!("[formula: {f}]"));
        }
        t.riga(
            StileRiga::Rigo,
            vec![
                Valore::Vuoto,
                Valore::Vuoto,
                Valore::testo(testo),
                Valore::numero_opz(c.parti_uguali, DECIMALI_FATTORI),
                Valore::numero_opz(c.lunghezza, DECIMALI_FATTORI),
                Valore::numero_opz(c.larghezza, DECIMALI_FATTORI),
                Valore::numero_opz(c.altezza_peso, DECIMALI_FATTORI),
                Valore::numero_opz(r.map(|x| x.quantita.valore()), regole.decimali_quantita),
            ],
        );
    }
    Ok(())
}

/// Voce completa: intestazione, righi e riga «sommano».
fn partita(
    t: &mut Tabella,
    numero: String,
    prezzo: &PrezzoApplicato,
    righi: &[RigoMisura],
    calcolo: &CalcoloPartita,
    regole: &RegoleArrotondamento,
    citate: Citate<'_>,
) -> Result<(), ErroreReport> {
    t.riga(
        StileRiga::Voce,
        vec![
            Valore::testo(numero),
            Valore::testo(prezzo.codice.clone()),
            Valore::testo(descrizione(prezzo)),
        ],
    );
    righi_misura(t, righi, regole, citate)?;
    let sommano = if prezzo.unita_misura.trim().is_empty() {
        "SOMMANO".to_owned()
    } else {
        format!("SOMMANO {}", prezzo.unita_misura.trim())
    };
    t.riga(
        StileRiga::Sommano,
        vec![
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::testo(sommano),
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::Vuoto,
            Valore::numero(calcolo.quantita.valore(), regole.decimali_quantita),
            Valore::numero(prezzo.prezzo.valore(), regole.decimali_importo),
            Valore::numero(calcolo.importo.valore(), regole.decimali_importo),
        ],
    );
    Ok(())
}

fn riga_totale(t: &mut Tabella, colonna_testo: usize, testo: &str, valore: Decimal, decimali: u32) {
    let n = t.colonne.len();
    let mut celle = vec![Valore::Vuoto; n];
    if let Some(c) = celle.get_mut(colonna_testo) {
        *c = Valore::testo(testo);
    }
    if let Some(c) = celle.last_mut() {
        *c = Valore::numero(valore, decimali);
    }
    t.riga(StileRiga::Totale, celle);
}

/// Incidenze da stampare nel computo (ADR 0021): quote di manodopera e di sicurezza
/// di ogni voce e, a scelta, i loro subtotali nel riepilogo finale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Incidenze {
    /// Incidenza e quota della manodopera di ogni voce.
    pub manodopera: bool,
    /// Incidenza e quota della sicurezza compresa nel prezzo di ogni voce.
    pub sicurezza: bool,
    /// Subtotali delle quote scelte nei riepiloghi finali.
    pub riepilogo: bool,
}

impl Incidenze {
    /// `true` se non è richiesta nessuna incidenza né subtotale.
    #[must_use]
    pub fn nessuna(&self) -> bool {
        !self.manodopera && !self.sicurezza && !self.riepilogo
    }

    /// Quote scelte, nell'ordine di stampa.
    pub(crate) fn quote(self) -> Vec<Quota> {
        let mut q = Vec::new();
        if self.manodopera {
            q.push(Quota::Manodopera);
        }
        if self.sicurezza {
            q.push(Quota::Sicurezza);
        }
        q
    }

    /// Quote con i subtotali nei riepiloghi.
    pub(crate) fn quote_riepilogo(self) -> Vec<Quota> {
        if self.riepilogo {
            self.quote()
        } else {
            Vec::new()
        }
    }
}

/// Quota di un importo: manodopera o sicurezza compresa nel prezzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Quota {
    Manodopera,
    Sicurezza,
}

impl Quota {
    /// Nome per esteso, minuscolo.
    pub(crate) fn nome(self) -> &'static str {
        match self {
            Self::Manodopera => "manodopera",
            Self::Sicurezza => "sicurezza",
        }
    }

    /// Colonne della quota: importo ed incidenza percentuale.
    fn colonne(self) -> [Colonna; 2] {
        match self {
            Self::Manodopera => [
                Colonna::numero("Manodopera €", 12),
                Colonna::numero("Mdo %", 6),
            ],
            Self::Sicurezza => [
                Colonna::numero("Sicurezza €", 12),
                Colonna::numero("Sic. %", 6),
            ],
        }
    }

    /// Incidenza percentuale della voce.
    pub(crate) fn incidenza(self, prezzo: &PrezzoApplicato) -> Option<Decimal> {
        match self {
            Self::Manodopera => prezzo.incidenza_manodopera,
            Self::Sicurezza => prezzo.incidenza_sicurezza,
        }
        .map(|p| p.valore())
    }

    /// Quota della voce calcolata dal core.
    pub(crate) fn importo(self, c: &CalcoloPartita) -> Option<Decimal> {
        match self {
            Self::Manodopera => c.importo_manodopera,
            Self::Sicurezza => c.importo_sicurezza,
        }
        .map(|i| i.valore())
    }

    /// Totale del computo.
    pub(crate) fn totale(self, r: &RiepilogoComputo) -> Decimal {
        match self {
            Self::Manodopera => r.totale_manodopera,
            Self::Sicurezza => r.totale_sicurezza,
        }
        .valore()
    }

    /// Quote raggruppate per categorie e WBS.
    pub(crate) fn raggruppate(self, r: &RiepilogoComputo) -> &QuoteRaggruppate {
        match self {
            Self::Manodopera => &r.manodopera,
            Self::Sicurezza => &r.sicurezza,
        }
    }

    /// Voci prive dell'incidenza.
    pub(crate) fn voci_senza(self, r: &RiepilogoComputo) -> usize {
        match self {
            Self::Manodopera => r.voci_senza_incidenza_manodopera,
            Self::Sicurezza => r.voci_senza_incidenza_sicurezza,
        }
    }

    /// Nota sulle voci prive dell'incidenza, se ce ne sono.
    pub(crate) fn nota(self, r: &RiepilogoComputo) -> Option<String> {
        let nome = self.nome();
        match self.voci_senza(r) {
            0 => None,
            1 => Some(format!(
                "1 voce senza incidenza della {nome}: la quota di {nome} è sottostimata."
            )),
            n => Some(format!(
                "{n} voci senza incidenza della {nome}: la quota di {nome} è sottostimata."
            )),
        }
    }
}

/// Riga di un riepilogo: gruppo, importo e quote nell'ordine richiesto.
#[derive(Debug, Clone)]
pub(crate) struct RigaRiepilogo {
    pub(crate) codice: String,
    pub(crate) descrizione: String,
    /// Profondità nell'albero (WBS), zero per le categorie.
    pub(crate) livello: usize,
    /// Riga di primo livello da evidenziare (nodi WBS radice).
    pub(crate) evidenza: bool,
    pub(crate) importo: Decimal,
    pub(crate) quote: Vec<Decimal>,
}

/// Riepilogo per un livello di categorie o per WBS. Il totale è quello del computo.
#[derive(Debug, Clone)]
pub(crate) struct Riepilogo {
    pub(crate) titolo: &'static str,
    pub(crate) righe: Vec<RigaRiepilogo>,
    pub(crate) nota: Option<&'static str>,
}

/// Riepiloghi del computo per super categorie, categorie, sotto categorie e WBS (solo
/// i livelli usati), con le quote richieste. Tutti i valori vengono dal core.
///
/// # Errors
/// Struttura WBS non valida.
pub(crate) fn riepiloghi(
    p: &Progetto,
    r: &RiepilogoComputo,
    quote: &[Quota],
) -> Result<Vec<Riepilogo>, ErroreReport> {
    let mut out = Vec::new();
    for (livello, totali, titolo, scegli) in [
        (
            LivelloCategoria::SuperCategoria,
            &r.per_super_categoria,
            "Super categorie",
            (|q: &QuoteRaggruppate| &q.per_super_categoria)
                as fn(&QuoteRaggruppate) -> &TotaliRaggruppati<ClasseId>,
        ),
        (
            LivelloCategoria::Categoria,
            &r.per_categoria,
            "Categorie",
            |q: &QuoteRaggruppate| &q.per_categoria,
        ),
        (
            LivelloCategoria::SottoCategoria,
            &r.per_sotto_categoria,
            "Sotto categorie",
            |q: &QuoteRaggruppate| &q.per_sotto_categoria,
        ),
    ] {
        if totali.per_chiave.is_empty() {
            continue;
        }
        let elenco = p.categorie.elenco(livello);
        let quote_di = |chiave: Option<ClasseId>| -> Vec<Decimal> {
            quote
                .iter()
                .map(|q| {
                    let t = scegli(q.raggruppate(r));
                    match chiave {
                        Some(k) => t.per_chiave.get(&k).copied().unwrap_or_default(),
                        None => t.non_assegnato,
                    }
                    .valore()
                })
                .collect()
        };
        let riga = |codice: String, descrizione: String, importo: Decimal, chiave| RigaRiepilogo {
            codice,
            descrizione,
            livello: 0,
            evidenza: false,
            importo,
            quote: quote_di(chiave),
        };
        let mut righe = Vec::new();
        // Nell'ordine dell'elenco delle classi, poi eventuali identificativi non trovati.
        for classe in elenco {
            if let Some(i) = totali.per_chiave.get(&classe.id) {
                righe.push(riga(
                    classe.codice.clone(),
                    classe.descrizione.clone(),
                    i.valore(),
                    Some(classe.id),
                ));
            }
        }
        for (id, i) in &totali.per_chiave {
            if !elenco.iter().any(|c| c.id == *id) {
                righe.push(riga(
                    "?".into(),
                    format!("classe inesistente {id}"),
                    i.valore(),
                    Some(*id),
                ));
            }
        }
        if !totali.non_assegnato.is_zero() {
            righe.push(riga(
                String::new(),
                "Non assegnato".into(),
                totali.non_assegnato.valore(),
                None,
            ));
        }
        out.push(Riepilogo {
            titolo,
            righe,
            nota: None,
        });
    }

    // WBS, con totali cumulati sugli antenati.
    if !r.per_wbs.per_chiave.is_empty() {
        let cumulati = r.per_wbs.cumulati(&p.wbs).map_err(errore_calcolo)?;
        let quote_cumulate = quote
            .iter()
            .map(|q| q.raggruppate(r).per_wbs.cumulati(&p.wbs))
            .collect::<Result<Vec<_>, _>>()
            .map_err(errore_calcolo)?;
        struct Visita<'a> {
            p: &'a Progetto,
            cumulati: &'a BTreeMap<WbsNodoId, Importo>,
            quote: &'a [BTreeMap<WbsNodoId, Importo>],
            righe: Vec<RigaRiepilogo>,
        }
        impl Visita<'_> {
            fn nodi(
                &mut self,
                padre: Option<WbsNodoId>,
                livello: usize,
            ) -> Result<(), ErroreReport> {
                for nodo in self.p.wbs.figli(padre) {
                    let Some(importo) = self.cumulati.get(&nodo.id) else {
                        continue;
                    };
                    let codice = self
                        .p
                        .wbs
                        .codice_completo(nodo.id)
                        .map_err(errore_calcolo)?;
                    self.righe.push(RigaRiepilogo {
                        codice,
                        descrizione: nodo.descrizione.clone(),
                        livello,
                        evidenza: livello == 0,
                        importo: importo.valore(),
                        quote: self
                            .quote
                            .iter()
                            .map(|q| q.get(&nodo.id).copied().unwrap_or_default().valore())
                            .collect(),
                    });
                    self.nodi(Some(nodo.id), livello + 1)?;
                }
                Ok(())
            }
        }
        let mut v = Visita {
            p,
            cumulati: &cumulati,
            quote: &quote_cumulate,
            righe: Vec::new(),
        };
        v.nodi(None, 0)?;
        let mut righe = v.righe;
        if !r.per_wbs.non_assegnato.is_zero() {
            righe.push(RigaRiepilogo {
                codice: String::new(),
                descrizione: "Non assegnato".into(),
                livello: 0,
                evidenza: false,
                importo: r.per_wbs.non_assegnato.valore(),
                quote: quote
                    .iter()
                    .map(|q| q.raggruppate(r).per_wbs.non_assegnato.valore())
                    .collect(),
            });
        }
        out.push(Riepilogo {
            titolo: "WBS",
            righe,
            nota: Some("Gli importi dei nodi comprendono quelli dei nodi figli."),
        });
    }
    Ok(out)
}

/// Celle di una quota: importo e incidenza sull'importo di riferimento.
fn celle_quota(quota: Decimal, riferimento: Decimal, di: u32) -> [Valore; 2] {
    [
        Valore::numero(quota, di),
        Valore::numero_opz(incidenza(quota, riferimento), DECIMALI_INCIDENZE),
    ]
}

/// Computo metrico estimativo: voci con righi di misura, totali, riepiloghi per
/// categorie e WBS.
///
/// # Errors
/// Computo inesistente o non calcolabile.
pub fn computo(p: &Progetto, id: ComputoId, o: &OpzioniReport) -> Result<Documento, ErroreReport> {
    computo_con(p, id, Incidenze::default(), o)
}

/// Come [`computo`], con le incidenze di manodopera e sicurezza richieste: colonne
/// della quota e dell'incidenza su ogni voce e, a scelta, i subtotali nei riepiloghi.
///
/// # Errors
/// Computo inesistente o non calcolabile.
pub fn computo_con(
    p: &Progetto,
    id: ComputoId,
    incidenze: Incidenze,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let c = p
        .computo(id)
        .ok_or_else(|| ErroreReport::ComputoInesistente(id.to_string()))?;
    let r = p.calcola_computo(id).map_err(errore_calcolo)?;
    let regole = &p.regole;
    let di = regole.decimali_importo;
    let mut doc = documento(p, &c.nome, true, o);
    let quote = incidenze.quote();

    let mut colonne = colonne_misure(true);
    let colonna_importo = colonne.len() - 1;
    for q in &quote {
        colonne.extend(q.colonne());
    }
    let mut t = Tabella::new(colonne);
    let quantita = c.quantita_riferite(regole);
    let citate = Citate {
        computo: Some(c),
        quantita: &quantita,
    };
    for (voce, risultato) in c.voci.iter().zip(&r.voci) {
        partita(
            &mut t,
            risultato.numero.to_string(),
            &voce.prezzo,
            &voce.righi,
            &risultato.calcolo,
            regole,
            citate,
        )?;
        if !quote.is_empty()
            && let Some(sommano) = t.righe.last_mut()
        {
            sommano.celle.truncate(colonna_importo + 1);
            for q in &quote {
                match (q.importo(&risultato.calcolo), q.incidenza(&voce.prezzo)) {
                    (Some(i), Some(pc)) => {
                        sommano.celle.push(Valore::numero(i, di));
                        sommano.celle.push(Valore::numero(pc, DECIMALI_INCIDENZE));
                    }
                    _ => {
                        sommano.celle.push(Valore::testo("—"));
                        sommano.celle.push(Valore::testo("—"));
                    }
                }
            }
        }
    }
    let totale = |t: &mut Tabella, testo: &str, valore: Decimal, quote_totali: &[Quota]| {
        let mut celle = vec![Valore::Vuoto; colonna_importo + 1];
        if let Some(c) = celle.get_mut(2) {
            *c = Valore::testo(testo);
        }
        if let Some(c) = celle.get_mut(colonna_importo) {
            *c = Valore::numero(valore, di);
        }
        for q in &quote {
            if quote_totali.contains(q) {
                celle.extend(celle_quota(q.totale(&r), valore, di));
            } else {
                celle.extend([Valore::Vuoto, Valore::Vuoto]);
            }
        }
        t.riga(StileRiga::Totale, celle);
    };
    totale(&mut t, "Totale lavori", r.totale_lavori.valore(), &[]);
    totale(
        &mut t,
        "Totale oneri della sicurezza",
        r.totale_oneri_sicurezza.valore(),
        &[],
    );
    totale(&mut t, "TOTALE", r.totale.valore(), &quote);
    let mut note = Vec::new();
    if !quote.is_empty() {
        note.push(
            "Le quote di manodopera e di sicurezza sono calcolate sull'importo di ogni voce \
             con l'incidenza del suo prezzo; «—»: incidenza non indicata."
                .to_owned(),
        );
    }
    doc.sezioni.push(Sezione {
        titolo: "Computo".into(),
        note,
        tabella: t,
    });

    // Riepilogo generale.
    let con_incidenza = incidenze.riepilogo;
    let mut colonne = vec![Colonna::testo("Voce", 60), Colonna::numero("Importo €", 20)];
    if con_incidenza {
        colonne.push(Colonna::numero("Incidenza %", 10));
    }
    let mut t = Tabella::new(colonne);
    let totale_computo = r.totale.valore();
    let mut riga = |stile: StileRiga, testo: &str, v: Decimal| {
        t.riga(
            stile,
            vec![
                Valore::testo(testo),
                Valore::numero(v, di),
                Valore::numero_opz(incidenza(v, totale_computo), DECIMALI_INCIDENZE),
            ],
        );
    };
    riga(StileRiga::Normale, "Lavori", r.totale_lavori.valore());
    riga(
        StileRiga::Normale,
        "Oneri della sicurezza",
        r.totale_oneri_sicurezza.valore(),
    );
    riga(
        StileRiga::Normale,
        "di cui manodopera (voci con incidenza nota)",
        r.totale_manodopera.valore(),
    );
    if incidenze.sicurezza || !r.totale_sicurezza.is_zero() {
        riga(
            StileRiga::Normale,
            "di cui sicurezza compresa nei prezzi (voci con incidenza nota)",
            r.totale_sicurezza.valore(),
        );
    }
    riga(StileRiga::Totale, "TOTALE", totale_computo);
    let mut note: Vec<String> = Quota::Manodopera.nota(&r).into_iter().collect();
    if incidenze.sicurezza {
        note.extend(Quota::Sicurezza.nota(&r));
    }
    doc.sezioni.push(Sezione {
        titolo: "Riepilogo".into(),
        note,
        tabella: t,
    });

    // Riepiloghi per categorie e WBS, con i subtotali delle quote se richiesti.
    let quote_riepilogo = incidenze.quote_riepilogo();
    for riepilogo in riepiloghi(p, &r, &quote_riepilogo)? {
        let mut colonne = vec![
            Colonna::testo("Codice", 8),
            Colonna::testo("Descrizione", 50),
            Colonna::numero("Importo €", 16),
            Colonna::numero("Incidenza %", 10),
        ];
        for q in &quote_riepilogo {
            colonne.extend(q.colonne());
        }
        let mut t = Tabella::new(colonne);
        let celle = |codice: Valore, descrizione: String, importo: Decimal, quote: &[Decimal]| {
            let mut c = vec![
                codice,
                Valore::testo(descrizione),
                Valore::numero(importo, di),
                Valore::numero_opz(incidenza(importo, totale_computo), DECIMALI_INCIDENZE),
            ];
            for q in quote {
                c.extend(celle_quota(*q, importo, di));
            }
            c
        };
        for riga in &riepilogo.righe {
            let codice = if riga.codice.is_empty() {
                Valore::Vuoto
            } else {
                Valore::testo(riga.codice.clone())
            };
            t.riga(
                if riga.evidenza {
                    StileRiga::Voce
                } else {
                    StileRiga::Normale
                },
                celle(
                    codice,
                    format!("{}{}", "  ".repeat(riga.livello), riga.descrizione),
                    riga.importo,
                    &riga.quote,
                ),
            );
        }
        let totali_quote: Vec<Decimal> = quote_riepilogo.iter().map(|q| q.totale(&r)).collect();
        t.riga(
            StileRiga::Totale,
            celle(
                Valore::Vuoto,
                "TOTALE".into(),
                totale_computo,
                &totali_quote,
            ),
        );
        doc.sezioni.push(Sezione {
            titolo: if riepilogo.titolo == "WBS" {
                "Riepilogo WBS".to_owned()
            } else {
                format!("Riepilogo {}", riepilogo.titolo.to_lowercase())
            },
            note: riepilogo.nota.map(str::to_owned).into_iter().collect(),
            tabella: t,
        });
    }
    Ok(doc)
}

/// Incidenza percentuale arrotondata a 2 decimali (metà lontano da zero).
pub(crate) fn incidenza(parte: Decimal, totale: Decimal) -> Option<Decimal> {
    if totale.is_zero() {
        return None;
    }
    let q = parte
        .checked_mul(Decimal::ONE_HUNDRED)?
        .checked_div(totale)?;
    Some(q.round_dp_with_strategy(
        DECIMALI_INCIDENZE,
        rust_decimal::RoundingStrategy::MidpointAwayFromZero,
    ))
}

/// Elenco prezzi, raggruppato per capitolo nell'ordine dei capitoli.
#[must_use]
pub fn elenco_prezzi(p: &Progetto, o: &OpzioniReport) -> Documento {
    let ep = &p.elenco_prezzi;
    let mut doc = documento(p, "Elenco prezzi", false, o);
    doc.intestazione.push(("Prezzario".into(), ep.fonte()));
    let mut t = Tabella::new(vec![
        Colonna::testo("Codice", 14),
        Colonna::testo("Descrizione", 56),
        Colonna::centrata("U.M.", 7),
        Colonna::numero("Prezzo €", 11),
        Colonna::numero("Manod. %", 8),
        Colonna::centrata("Natura", 9),
    ]);
    let voce = |t: &mut Tabella, v: &opencomputo_core::prezzario::VocePrezzario| {
        let descr = if v.descrizione_estesa.trim().is_empty() {
            v.descrizione_breve.clone()
        } else {
            v.descrizione_estesa.clone()
        };
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::testo(v.codice.clone()),
                Valore::testo(descr),
                Valore::testo(v.unita_misura.clone()),
                Valore::numero(v.prezzo.valore(), p.regole.decimali_importo),
                Valore::numero_opz(
                    v.incidenza_manodopera.map(|x| x.valore()),
                    DECIMALI_INCIDENZE,
                ),
                Valore::testo(natura(v.natura)),
            ],
        );
    };
    let mut scritte = 0usize;
    for cap in &ep.capitoli {
        let mut voci = ep
            .voci
            .iter()
            .filter(|v| v.capitolo == Some(cap.id))
            .peekable();
        if voci.peek().is_none() {
            continue;
        }
        let titolo = if cap.codice.is_empty() {
            cap.descrizione.clone()
        } else {
            format!("{} — {}", cap.codice, cap.descrizione)
        };
        t.riga_testo(StileRiga::Gruppo, 1, titolo);
        for v in voci {
            voce(&mut t, v);
            scritte += 1;
        }
    }
    let senza: Vec<_> = ep
        .voci
        .iter()
        .filter(|v| {
            v.capitolo
                .is_none_or(|c| !ep.capitoli.iter().any(|x| x.id == c))
        })
        .collect();
    if !senza.is_empty() {
        if scritte > 0 {
            t.riga_testo(StileRiga::Gruppo, 1, "Voci senza capitolo");
        }
        for v in senza {
            voce(&mut t, v);
        }
    }
    doc.sezioni.push(Sezione {
        titolo: "Elenco prezzi".into(),
        note: vec![format!("{} voci.", ep.voci.len())],
        tabella: t,
    });
    doc
}

/// Libretto delle misure: registrazioni con righi, data e SAL di appartenenza.
///
/// # Errors
/// Registrazioni non calcolabili.
pub fn libretto(p: &Progetto, o: &OpzioniReport) -> Result<Documento, ErroreReport> {
    let regole = &p.regole;
    let mut doc = documento(p, "Libretto delle misure", true, o);
    // Senza prezzi: le celle di prezzo e importo di `partita` sono scartate.
    let mut t = Tabella::new(colonne_misure(false));
    let contabilita = &p.contabilita;
    for reg in &contabilita.libretto().registrazioni {
        let calcolo = reg.calcola(regole).map_err(errore_calcolo)?;
        let mut prezzo = reg.prezzo.clone();
        let sal = contabilita.sal_di(reg.id).map_or_else(
            || "non contabilizzata".to_owned(),
            |s| format!("SAL n. {}", s.numero),
        );
        prezzo.descrizione_estesa = format!(
            "{} — {}, {}",
            descrizione(&reg.prezzo),
            formato::data(reg.data),
            sal
        );
        partita(
            &mut t,
            reg.numero.to_string(),
            &prezzo,
            &reg.righi,
            &calcolo,
            regole,
            Citate::NESSUNA,
        )?;
    }
    doc.sezioni.push(Sezione {
        titolo: "Libretto".into(),
        note: Vec::new(),
        tabella: t,
    });
    Ok(doc)
}

fn colonne_registro() -> Vec<Colonna> {
    vec![
        Colonna::centrata("N.", 5),
        Colonna::centrata("Libr.", 5),
        Colonna::centrata("Data", 9),
        Colonna::testo("Codice", 12),
        Colonna::testo("Descrizione", 38),
        Colonna::centrata("U.M.", 6),
        Colonna::numero("Quantità", 10),
        Colonna::numero("Prezzo €", 10),
        Colonna::numero("Importo €", 12),
    ]
}

/// Registro di contabilità, raggruppato per SAL con totali del periodo e progressivi.
///
/// # Errors
/// Registro non calcolabile.
pub fn registro(p: &Progetto, o: &OpzioniReport) -> Result<Documento, ErroreReport> {
    let regole = &p.regole;
    let di = regole.decimali_importo;
    let reg = p.contabilita.registro(regole).map_err(errore_calcolo)?;
    let mut doc = documento(p, "Registro di contabilità", true, o);
    let mut t = Tabella::new(colonne_registro());
    for tot in &reg.totali {
        let sal = p.contabilita.sal_per_id(tot.sal);
        let titolo = match sal {
            Some(s) => format!(
                "SAL n. {} — lavori a tutto il {}{}",
                s.numero,
                formato::data(s.lavori_a_tutto_il),
                if s.stato == StatoSal::Bozza {
                    " (bozza)"
                } else {
                    ""
                }
            ),
            None => format!("SAL n. {}", tot.numero_sal),
        };
        t.riga_testo(StileRiga::Gruppo, 4, titolo);
        for r in reg.righe.iter().filter(|r| r.sal == tot.sal) {
            t.riga(
                StileRiga::Normale,
                vec![
                    Valore::testo(r.numero.to_string()),
                    Valore::testo(r.numero_libretto.to_string()),
                    Valore::testo(formato::data(r.data)),
                    Valore::testo(r.codice.clone()),
                    Valore::testo(if r.natura == NaturaVoce::OneriSicurezza {
                        format!("{} (sicurezza)", r.descrizione)
                    } else {
                        r.descrizione.clone()
                    }),
                    Valore::testo(r.unita_misura.clone()),
                    Valore::numero(r.quantita.valore(), regole.decimali_quantita),
                    Valore::numero(r.prezzo.valore(), di),
                    Valore::numero(r.importo.valore(), di),
                ],
            );
        }
        riga_totale(
            &mut t,
            4,
            "Importo del periodo",
            tot.importo_periodo.valore(),
            di,
        );
        riga_totale(
            &mut t,
            4,
            "Importo progressivo",
            tot.importo_progressivo.valore(),
            di,
        );
    }
    doc.sezioni.push(Sezione {
        titolo: "Registro".into(),
        note: vec!["Importi lordi, prima del ribasso d'asta.".into()],
        tabella: t,
    });
    Ok(doc)
}

/// Stato di avanzamento lavori: lavorazioni del periodo e prospetto economico.
///
/// Per un SAL emesso il prospetto è quello **congelato** all'emissione; per un SAL in
/// bozza è ricalcolato e il documento è marcato come bozza.
///
/// # Errors
/// SAL inesistente o non calcolabile.
pub fn stato_avanzamento(
    p: &Progetto,
    id: SalId,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let regole = &p.regole;
    let di = regole.decimali_importo;
    let sal = p
        .contabilita
        .sal_per_id(id)
        .ok_or_else(|| ErroreReport::SalInesistente(id.to_string()))?;
    let (riepilogo, bozza): (RiepilogoSal, bool) = match (&sal.stato, &sal.riepilogo_emesso) {
        (StatoSal::Emesso, Some(r)) => (r.clone(), false),
        _ => (p.calcola_sal(id).map_err(errore_calcolo)?, true),
    };
    let mut doc = documento(
        p,
        &format!(
            "Stato di avanzamento lavori n. {}{}",
            sal.numero,
            if bozza { " — BOZZA" } else { "" }
        ),
        false,
        o,
    );
    doc.intestazione.push((
        "Lavori a tutto il".into(),
        formato::data(sal.lavori_a_tutto_il),
    ));
    if let Some(d) = sal.data_emissione {
        doc.intestazione
            .push(("Emesso il".into(), formato::data(d)));
    }
    if let Some(c) = &p.contratto {
        doc.intestazione.push((
            "Ribasso d'asta".into(),
            format!("{} %", formato::numero(c.parametri.ribasso.valore(), 0)),
        ));
    }

    // Lavorazioni del periodo, dal registro.
    let reg = p.contabilita.registro(regole).map_err(errore_calcolo)?;
    let mut t = Tabella::new(colonne_registro());
    for r in reg.righe.iter().filter(|r| r.sal == id) {
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::testo(r.numero.to_string()),
                Valore::testo(r.numero_libretto.to_string()),
                Valore::testo(formato::data(r.data)),
                Valore::testo(r.codice.clone()),
                Valore::testo(r.descrizione.clone()),
                Valore::testo(r.unita_misura.clone()),
                Valore::numero(r.quantita.valore(), regole.decimali_quantita),
                Valore::numero(r.prezzo.valore(), di),
                Valore::numero(r.importo.valore(), di),
            ],
        );
    }
    riga_totale(
        &mut t,
        4,
        "Importo lordo del periodo",
        riepilogo.lordo_periodo.valore(),
        di,
    );
    doc.sezioni.push(Sezione {
        titolo: "Lavorazioni del periodo".into(),
        note: Vec::new(),
        tabella: t,
    });

    let mut t = Tabella::new(vec![
        Colonna::testo("Voce", 60),
        Colonna::numero("Importo €", 20),
    ]);
    let mut riga = |stile: StileRiga, testo: &str, v: Decimal| {
        t.riga(stile, vec![Valore::testo(testo), Valore::numero(v, di)]);
    };
    let n = StileRiga::Normale;
    riga(
        n,
        "Lavori a tutto il presente SAL",
        riepilogo.lavori_lordi.valore(),
    );
    if !riepilogo.manodopera_esclusa.is_zero() {
        riga(
            n,
            "di cui manodopera esclusa dal ribasso",
            riepilogo.manodopera_esclusa.valore(),
        );
    }
    if !riepilogo.lavori_esclusi_dal_ribasso.is_zero() {
        riga(
            n,
            "di cui nuovi prezzi esclusi dal ribasso",
            riepilogo.lavori_esclusi_dal_ribasso.valore(),
        );
    }
    riga(
        n,
        "Importo soggetto a ribasso",
        riepilogo.soggetto_a_ribasso.valore(),
    );
    riga(n, "a dedurre: ribasso d'asta", riepilogo.ribasso.valore());
    riga(
        n,
        "Oneri della sicurezza (non soggetti a ribasso)",
        riepilogo.oneri_sicurezza.valore(),
    );
    riga(
        StileRiga::Sommano,
        "Importo netto (lavori − ribasso + sicurezza)",
        riepilogo.importo_netto.valore(),
    );
    riga(
        n,
        "a dedurre: ritenuta di garanzia",
        riepilogo.ritenuta_garanzia.valore(),
    );
    riga(
        StileRiga::Sommano,
        "Credito dell'impresa",
        riepilogo.credito_progressivo.valore(),
    );
    riga(
        n,
        "a dedurre: certificati precedenti",
        riepilogo.certificati_precedenti.valore(),
    );
    riga(
        StileRiga::Totale,
        "IMPORTO DEL PRESENTE SAL",
        riepilogo.importo_sal.valore(),
    );
    let mut note = vec!["Importi progressivi, salvo l'importo del presente SAL.".to_owned()];
    if bozza {
        note.push("SAL non emesso: prospetto ricalcolato, può ancora cambiare.".into());
    }
    doc.sezioni.push(Sezione {
        titolo: "Prospetto".into(),
        note,
        tabella: t,
    });
    Ok(doc)
}

/// Certificato di pagamento di un SAL: importo del SAL, recupero dell'anticipazione,
/// penali, IVA e totale. Congelato se il SAL è emesso con questa versione, altrimenti
/// ricalcolato (bozza o SAL emesso con una versione precedente).
///
/// # Errors
/// SAL inesistente, contratto mancante o dati non calcolabili.
pub fn certificato_pagamento(
    p: &Progetto,
    id: SalId,
    o: &OpzioniReport,
) -> Result<Documento, ErroreReport> {
    let di = p.regole.decimali_importo;
    let sal = p
        .contabilita
        .sal_per_id(id)
        .ok_or_else(|| ErroreReport::SalInesistente(id.to_string()))?;
    let c = p.calcola_certificato(id).map_err(errore_calcolo)?;
    let bozza = sal.stato != StatoSal::Emesso;
    let congelato = !bozza && sal.certificato_emesso.is_some();
    let mut doc = documento(
        p,
        &format!(
            "Certificato di pagamento n. {}{}",
            c.numero,
            if bozza { " — BOZZA" } else { "" }
        ),
        false,
        o,
    );
    doc.intestazione.push((
        "Stato di avanzamento".into(),
        format!(
            "n. {} — lavori a tutto il {}",
            sal.numero,
            formato::data(sal.lavori_a_tutto_il)
        ),
    ));
    if let Some(d) = sal.data_emissione {
        doc.intestazione
            .push(("SAL emesso il".into(), formato::data(d)));
    }

    let mut t = Tabella::new(vec![
        Colonna::testo("Voce", 60),
        Colonna::numero("Importo €", 20),
    ]);
    let mut riga = |stile: StileRiga, testo: &str, v: Decimal| {
        t.riga(stile, vec![Valore::testo(testo), Valore::numero(v, di)]);
    };
    let n = StileRiga::Normale;
    riga(
        n,
        "Importo del presente stato di avanzamento (al netto della ritenuta di garanzia)",
        c.importo_sal.valore(),
    );
    if !c.anticipazione.is_zero() {
        riga(
            n,
            if c.recupero_sospeso {
                "a dedurre: recupero dell'anticipazione (sospeso)"
            } else {
                "a dedurre: recupero dell'anticipazione"
            },
            c.recupero_anticipazione.valore(),
        );
    }
    if !c.penali.is_zero() {
        riga(n, "a dedurre: penali", c.penali.valore());
    }
    riga(StileRiga::Sommano, "Imponibile", c.imponibile.valore());
    if let Some(a) = c.aliquota_iva {
        riga(
            n,
            &format!("IVA {} %", formato::numero(a.valore(), 0)),
            c.iva.valore(),
        );
    }
    riga(
        StileRiga::Totale,
        "TOTALE DEL CERTIFICATO DI PAGAMENTO",
        c.totale.valore(),
    );
    let mut note = Vec::new();
    if c.aliquota_iva.is_none() {
        note.push("Aliquota IVA non indicata nel contratto: importi al netto dell'IVA.".into());
    }
    if bozza {
        note.push("SAL non emesso: certificato ricalcolato, può ancora cambiare.".into());
    } else if !congelato {
        note.push(
            "SAL emesso con una versione precedente: certificato ricalcolato con i parametri attuali."
                .into(),
        );
    }
    doc.sezioni.push(Sezione {
        titolo: "Certificato".into(),
        note,
        tabella: t,
    });

    if !c.anticipazione.is_zero() {
        let mut t = Tabella::new(vec![
            Colonna::testo("Anticipazione", 60),
            Colonna::numero("Importo €", 20),
        ]);
        for (testo, v) in [
            ("Anticipazione erogata", c.anticipazione),
            (
                "Recuperata nei certificati precedenti",
                c.anticipazione_recuperata_prima,
            ),
            (
                "Recuperata nel presente certificato",
                c.recupero_anticipazione,
            ),
            ("Ancora da recuperare", c.anticipazione_residua),
        ] {
            t.riga(
                n,
                vec![Valore::testo(testo), Valore::numero(v.valore(), di)],
            );
        }
        doc.sezioni.push(Sezione {
            titolo: "Anticipazione".into(),
            note: Vec::new(),
            tabella: t,
        });
    }

    let penali: Vec<_> = p
        .contabilita
        .penali()
        .iter()
        .filter(|x| {
            if bozza {
                x.data <= sal.lavori_a_tutto_il && p.contabilita.sal_della_penale(x.id).is_none()
            } else {
                sal.penali.contains(&x.id)
            }
        })
        .collect();
    if !penali.is_empty() {
        let mut t = Tabella::new(vec![
            Colonna::centrata("N.", 6),
            Colonna::testo("Data", 12),
            Colonna::testo("Motivo", 50),
            Colonna::numero("Importo €", 16),
        ]);
        for x in &penali {
            let motivo = match (x.giorni, x.per_mille) {
                (Some(g), Some(pm)) => format!(
                    "{} ({g} giorni × {} ‰)",
                    x.descrizione,
                    formato::numero(pm, 1)
                ),
                _ => x.descrizione.clone(),
            };
            t.riga(
                n,
                vec![
                    Valore::testo(x.numero.to_string()),
                    Valore::testo(formato::data(x.data)),
                    Valore::testo(motivo),
                    Valore::numero(x.importo.valore(), di),
                ],
            );
        }
        doc.sezioni.push(Sezione {
            titolo: "Penali applicate".into(),
            note: Vec::new(),
            tabella: t,
        });
    }
    Ok(doc)
}

/// Sommario del registro di contabilità: per articolo di elenco, quantità e importi di
/// ogni SAL (Allegato II.14 al D.Lgs. 36/2023, art. 12, comma 2).
///
/// # Errors
/// Registrazioni non calcolabili.
pub fn sommario_registro(p: &Progetto, o: &OpzioniReport) -> Result<Documento, ErroreReport> {
    let regole = &p.regole;
    let (dq, di) = (regole.decimali_quantita, regole.decimali_importo);
    let s = p.contabilita.sommario(regole).map_err(errore_calcolo)?;
    let mut doc = documento(p, "Sommario del registro di contabilità", true, o);
    let mut colonne = vec![
        Colonna::testo("Tariffa", 14),
        Colonna::testo("Designazione", 34),
        Colonna::centrata("U.M.", 6),
        Colonna::numero("Prezzo €", 10),
    ];
    let mut etichette: Vec<String> = s.sal.iter().map(|n| format!("SAL {n}")).collect();
    if s.stato_finale {
        etichette.push("Stato finale".into());
    }
    for e in &etichette {
        colonne.push(Colonna::numero(&format!("{e} quantità"), 11));
        colonne.push(Colonna::numero(&format!("{e} importo €"), 12));
    }
    colonne.push(Colonna::numero("Quantità totale", 11));
    colonne.push(Colonna::numero("Importo totale €", 12));
    let mut t = Tabella::new(colonne);
    for a in &s.articoli {
        let mut celle = vec![
            Valore::testo(a.codice.clone()),
            Valore::testo(if a.natura == NaturaVoce::OneriSicurezza {
                format!("{} (sicurezza)", a.descrizione_breve)
            } else {
                a.descrizione_breve.clone()
            }),
            Valore::testo(a.unita_misura.clone()),
            Valore::numero(a.prezzo.valore(), di),
        ];
        for (q, i) in &a.per_sal {
            if i.is_zero() && q.valore().is_zero() {
                celle.extend([Valore::Vuoto, Valore::Vuoto]);
            } else {
                celle.push(Valore::numero(q.valore(), dq));
                celle.push(Valore::numero(i.valore(), di));
            }
        }
        celle.push(Valore::numero(a.quantita.valore(), dq));
        celle.push(Valore::numero(a.importo.valore(), di));
        t.riga(StileRiga::Normale, celle);
    }
    let mut totali = vec![
        Valore::Vuoto,
        Valore::testo("Totale lordo"),
        Valore::Vuoto,
        Valore::Vuoto,
    ];
    for v in &s.totali_per_sal {
        totali.push(Valore::Vuoto);
        totali.push(Valore::numero(v.valore(), di));
    }
    totali.push(Valore::Vuoto);
    totali.push(Valore::numero(s.totale.valore(), di));
    t.riga(StileRiga::Totale, totali);
    doc.sezioni.push(Sezione {
        titolo: "Sommario".into(),
        note: vec![if s.stato_finale {
            "Importi lordi (prima del ribasso) delle registrazioni contabilizzate in ciascun SAL \
             e nello stato finale."
                .into()
        } else {
            "Importi lordi (prima del ribasso) delle registrazioni contabilizzate in ciascun SAL."
                .into()
        }],
        tabella: t,
    });
    Ok(doc)
}

/// Tabella a due colonne «voce / importo».
fn tabella_importi(righe: &[(StileRiga, String, Decimal)], di: u32) -> Tabella {
    let mut t = Tabella::new(vec![
        Colonna::testo("Voce", 60),
        Colonna::numero("Importo €", 20),
    ]);
    for (stile, testo, v) in righe {
        t.riga(
            *stile,
            vec![Valore::testo(testo.clone()), Valore::numero(*v, di)],
        );
    }
    t
}

/// Conto finale dei lavori: stato finale, certificati emessi, credito residuo e
/// certificato di pagamento della rata di saldo (Allegato II.14 al D.Lgs. 36/2023).
///
/// # Errors
/// Conto finale non redatto o dati non calcolabili.
#[allow(clippy::too_many_lines)]
pub fn conto_finale(p: &Progetto, o: &OpzioniReport) -> Result<Documento, ErroreReport> {
    let regole = &p.regole;
    let (dq, di) = (regole.decimali_quantita, regole.decimali_importo);
    let k = p
        .contabilita
        .conto_finale()
        .ok_or_else(|| ErroreReport::Calcolo("il conto finale non è stato redatto".into()))?;
    let r = p.calcola_conto_finale().map_err(errore_calcolo)?;
    let bozza = k.stato != StatoSal::Emesso;
    let mut doc = documento(
        p,
        &format!(
            "Conto finale dei lavori{}",
            if bozza { " — BOZZA" } else { "" }
        ),
        false,
        o,
    );
    doc.intestazione.push((
        "Ultimazione dei lavori".into(),
        formato::data(k.data_ultimazione),
    ));
    doc.intestazione.push((
        if bozza {
            "Data del conto".into()
        } else {
            "Conto emesso il".into()
        },
        formato::data(k.data),
    ));
    let n = StileRiga::Normale;
    let v = |i: opencomputo_core::numeri::Importo| i.valore();

    // Stato finale.
    let mut righe = vec![(
        n,
        "Lavori a tutto lo stato finale (lordo)".to_owned(),
        v(r.lavori_lordi),
    )];
    if !r.lordo_stato_finale.is_zero() {
        righe.push((
            n,
            "— di cui lavori non compresi nei SAL".into(),
            v(r.lordo_stato_finale),
        ));
    }
    righe.push((
        n,
        "Oneri della sicurezza (non soggetti a ribasso)".into(),
        v(r.oneri_sicurezza),
    ));
    if !r.manodopera_esclusa.is_zero() {
        righe.push((
            n,
            "Manodopera esclusa dal ribasso".into(),
            v(r.manodopera_esclusa),
        ));
    }
    if !r.lavori_esclusi_dal_ribasso.is_zero() {
        righe.push((
            n,
            "Nuovi prezzi esclusi dal ribasso".into(),
            v(r.lavori_esclusi_dal_ribasso),
        ));
    }
    righe.push((
        n,
        "Importo soggetto a ribasso".into(),
        v(r.soggetto_a_ribasso),
    ));
    righe.push((n, "Ribasso d'asta".into(), v(r.ribasso)));
    righe.push((
        StileRiga::Totale,
        "IMPORTO NETTO FINALE DEI LAVORI".into(),
        v(r.importo_netto),
    ));
    doc.sezioni.push(Sezione {
        titolo: "Stato finale".into(),
        note: Vec::new(),
        tabella: tabella_importi(&righe, di),
    });

    // Certificati emessi con i SAL.
    let mut t = Tabella::new(vec![
        Colonna::centrata("SAL", 6),
        Colonna::testo("Lavori a tutto il", 12),
        Colonna::testo("Emesso il", 12),
        Colonna::numero("Importo SAL €", 14),
        Colonna::numero("Recupero anticip. €", 14),
        Colonna::numero("Penali €", 11),
        Colonna::numero("Imponibile €", 14),
        Colonna::numero("IVA €", 11),
        Colonna::numero("Totale €", 14),
    ]);
    let mut somme = [opencomputo_core::numeri::Importo::ZERO; 6];
    for sal in p.contabilita.sal() {
        let c = p.calcola_certificato(sal.id).map_err(errore_calcolo)?;
        let valori = [
            c.importo_sal,
            c.recupero_anticipazione,
            c.penali,
            c.imponibile,
            c.iva,
            c.totale,
        ];
        for (s, x) in somme.iter_mut().zip(valori) {
            *s = s.somma(x).map_err(errore_calcolo)?;
        }
        let mut celle = vec![
            Valore::testo(sal.numero.to_string()),
            Valore::testo(formato::data(sal.lavori_a_tutto_il)),
            Valore::testo(sal.data_emissione.map(formato::data).unwrap_or_default()),
        ];
        celle.extend(valori.iter().map(|x| Valore::numero(x.valore(), di)));
        t.riga(n, celle);
    }
    let mut celle = vec![Valore::Vuoto, Valore::testo("Totale"), Valore::Vuoto];
    celle.extend(somme.iter().map(|x| Valore::numero(x.valore(), di)));
    t.riga(StileRiga::Totale, celle);
    doc.sezioni.push(Sezione {
        titolo: "Stati di avanzamento e certificati di pagamento".into(),
        note: Vec::new(),
        tabella: t,
    });

    // Conto finale.
    let righe = vec![
        (
            n,
            "Importo netto finale dei lavori".to_owned(),
            v(r.importo_netto),
        ),
        (
            n,
            "a dedurre: importi dei SAL emessi (acconti)".into(),
            v(r.acconti),
        ),
        (
            StileRiga::Sommano,
            "Credito residuo dell'esecutore".into(),
            v(r.credito_residuo),
        ),
        (
            n,
            "— di cui ritenute di garanzia da svincolare".into(),
            v(r.ritenute_garanzia),
        ),
    ];
    doc.sezioni.push(Sezione {
        titolo: "Conto finale".into(),
        note: Vec::new(),
        tabella: tabella_importi(&righe, di),
    });

    // Rata di saldo.
    let mut righe = vec![(
        n,
        "Credito residuo dell'esecutore".to_owned(),
        v(r.credito_residuo),
    )];
    if !r.anticipazione.is_zero() {
        righe.push((
            n,
            "a dedurre: anticipazione non ancora recuperata".into(),
            v(r.anticipazione_residua),
        ));
    }
    if !r.penali.is_zero() {
        righe.push((
            n,
            "a dedurre: penali non applicate nei certificati".into(),
            v(r.penali),
        ));
    }
    righe.push((StileRiga::Sommano, "Imponibile".into(), v(r.imponibile)));
    if let Some(a) = r.aliquota_iva {
        righe.push((
            n,
            format!("IVA {} %", formato::numero(a.valore(), 0)),
            v(r.iva),
        ));
    }
    righe.push((
        StileRiga::Totale,
        "TOTALE DELLA RATA DI SALDO".into(),
        v(r.totale),
    ));
    let mut note = vec![
        "La rata di saldo si paga dopo il collaudo o il certificato di regolare esecuzione; \
         se pagata prima dell'accettazione definitiva dell'opera serve la garanzia dell'art. 117, \
         comma 9. Le ritenute si svincolano previo DURC regolare (art. 11, comma 6)."
            .to_owned(),
    ];
    if r.aliquota_iva.is_none() {
        note.push("Aliquota IVA non indicata nel contratto: importi al netto dell'IVA.".into());
    }
    if bozza {
        note.push("Conto finale non emesso: valori ricalcolati, possono ancora cambiare.".into());
    }
    doc.sezioni.push(Sezione {
        titolo: "Certificato di pagamento della rata di saldo".into(),
        note,
        tabella: tabella_importi(&righe, di),
    });

    // Lavori dello stato finale.
    let finali = p.contabilita.registrazioni_stato_finale();
    if !finali.is_empty() {
        let mut t = Tabella::new(vec![
            Colonna::centrata("N.", 6),
            Colonna::testo("Data", 11),
            Colonna::testo("Tariffa", 12),
            Colonna::testo("Designazione", 34),
            Colonna::centrata("U.M.", 6),
            Colonna::numero("Quantità", 11),
            Colonna::numero("Prezzo €", 10),
            Colonna::numero("Importo €", 12),
        ]);
        for reg in finali {
            let c = reg
                .calcola(regole)
                .map_err(|e| ErroreReport::Calcolo(e.to_string()))?;
            t.riga(
                n,
                vec![
                    Valore::testo(reg.numero.to_string()),
                    Valore::testo(formato::data(reg.data)),
                    Valore::testo(reg.prezzo.codice.clone()),
                    Valore::testo(reg.prezzo.descrizione_breve.clone()),
                    Valore::testo(reg.prezzo.unita_misura.clone()),
                    Valore::numero(c.quantita.valore(), dq),
                    Valore::numero(reg.prezzo.prezzo.valore(), di),
                    Valore::numero(c.importo.valore(), di),
                ],
            );
        }
        doc.sezioni.push(Sezione {
            titolo: "Lavori dello stato finale non compresi nei SAL".into(),
            note: Vec::new(),
            tabella: t,
        });
    }

    let penali = p.contabilita.penali_conto_finale();
    if !penali.is_empty() {
        let mut t = Tabella::new(vec![
            Colonna::centrata("N.", 6),
            Colonna::testo("Data", 12),
            Colonna::testo("Motivo", 50),
            Colonna::numero("Importo €", 16),
        ]);
        for x in penali {
            t.riga(
                n,
                vec![
                    Valore::testo(x.numero.to_string()),
                    Valore::testo(formato::data(x.data)),
                    Valore::testo(x.descrizione.clone()),
                    Valore::numero(x.importo.valore(), di),
                ],
            );
        }
        doc.sezioni.push(Sezione {
            titolo: "Penali della rata di saldo".into(),
            note: Vec::new(),
            tabella: t,
        });
    }

    // Firma e collaudo.
    let mut t = Tabella::new(vec![Colonna::testo("Atto", 30), Colonna::testo("", 50)]);
    let mut atto = |a: &str, b: String| t.riga(n, vec![Valore::testo(a), Valore::testo(b)]);
    if !bozza {
        atto(
            "Firma dell'esecutore",
            k.firma_esecutore.map_or_else(
                || "non ancora firmato".into(),
                |d| format!("firmato il {}", formato::data(d)),
            ),
        );
        if k.firma_esecutore.is_some() {
            atto(
                "Riserve confermate",
                k.riserve.clone().unwrap_or_else(|| "nessuna".into()),
            );
        }
    }
    match &k.collaudo {
        Some(c) => {
            let mut testo = format!("emesso il {}", formato::data(c.data));
            if let Some(a) = c.approvato_il {
                testo.push_str(&format!(", approvato il {}", formato::data(a)));
            }
            let mut nome = c.tipo.nome().to_owned();
            if let Some(prima) = nome.get_mut(..1) {
                prima.make_ascii_uppercase();
            }
            atto(&nome, testo);
        }
        None => atto("Collaudo", "non ancora eseguito".into()),
    }
    doc.sezioni.push(Sezione {
        titolo: "Firma e collaudo".into(),
        note: k.note.iter().cloned().collect(),
        tabella: t,
    });
    Ok(doc)
}
