//! Cronoprogramma dei lavori (ADR 0024).
//!
//! Le attività sono i nodi della WBS, più eventuali attività aggiunte (fasi senza voci,
//! pietre miliari) sotto un nodo o alla radice. Il **livello di dettaglio** decide fin
//! dove scendere nella WBS: i nodi al livello scelto, o le foglie, sono le attività da
//! pianificare con una durata; i nodi sopra sono riepiloghi, con le date dei figli.
//! Durate e legami definiti ai livelli più profondi restano memorizzati: con un livello
//! meno dettagliato i legami passano al nodo visibile che li contiene.
//!
//! Le durate sono in **giorni lavorativi** secondo il [`Calendario`] (giorni della
//! settimana, festività nazionali, chiusure). Il calcolo è il metodo del percorso
//! critico: date al più presto (passata in avanti), date al più tardi (passata
//! all'indietro), margine totale, attività critiche. I legami sono fine-inizio,
//! inizio-inizio, fine-fine e inizio-fine, con un ritardo anche negativo.
//!
//! Con un computo scelto, ogni attività riceve l'importo dei suoi nodi WBS e il piano
//! dà la produzione mensile (cronoprogramma economico e curva a S): gli importi sono
//! quelli del core, ripartiti in modo uniforme sui giorni lavorativi delle attività,
//! con somme esatte.
//!
//! Il cronoprogramma è salvato nel progetto ma non entra negli export verso altri
//! programmi (XPWE, DCF).

use std::collections::{BTreeMap, HashMap};

use crate::data::{Date, OperazioniData, data_amg};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::classificazione::StrutturaWbs;
use crate::id::{AttivitaId, ComputoId, WbsNodoId};
use crate::numeri::Importo;
use crate::progetto::Progetto;

/// Riferimento a un'attività del cronoprogramma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "id", rename_all = "snake_case")]
pub enum RifAttivita {
    /// Nodo della WBS.
    Wbs(WbsNodoId),
    /// Attività aggiunta.
    Attivita(AttivitaId),
}

/// Tipo di legame tra due attività.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoLegame {
    /// Il successore inizia dopo la fine del predecessore (FI).
    #[default]
    FineInizio,
    /// Il successore inizia dopo l'inizio del predecessore (II).
    InizioInizio,
    /// Il successore finisce dopo la fine del predecessore (FF).
    FineFine,
    /// Il successore finisce dopo l'inizio del predecessore (IF).
    InizioFine,
}

impl TipoLegame {
    /// Sigla: FI, II, FF, IF.
    #[must_use]
    pub fn sigla(self) -> &'static str {
        match self {
            Self::FineInizio => "FI",
            Self::InizioInizio => "II",
            Self::FineFine => "FF",
            Self::InizioFine => "IF",
        }
    }

    /// Il legame dipende dalla fine del predecessore.
    #[must_use]
    pub fn dalla_fine(self) -> bool {
        matches!(self, Self::FineInizio | Self::FineFine)
    }

    /// Il legame vincola l'inizio del successore.
    #[must_use]
    pub fn sull_inizio(self) -> bool {
        matches!(self, Self::FineInizio | Self::InizioInizio)
    }
}

/// Legame di precedenza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Legame {
    /// Predecessore.
    pub da: RifAttivita,
    /// Successore.
    pub a: RifAttivita,
    /// Tipo.
    #[serde(default)]
    pub tipo: TipoLegame,
    /// Ritardo in giorni lavorativi (negativo: anticipo).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ritardo: i32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &i32) -> bool {
    *v == 0
}

/// Dati di pianificazione di un'attività.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatiAttivita {
    /// Durata in giorni lavorativi (assente: 1 giorno, da stimare; 0 per una pietra
    /// miliare).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durata: Option<u32>,
    /// Vincolo: inizio non prima del giorno indicato.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inizio_non_prima: Option<Date>,
    /// Avanzamento in percentuale (0–100).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub avanzamento: u8,
    /// Note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

impl DatiAttivita {
    fn vuoti(&self) -> bool {
        *self == Self::default()
    }
}

/// Dati di pianificazione di un nodo WBS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttivitaWbs {
    /// Nodo.
    pub nodo: WbsNodoId,
    /// Dati.
    #[serde(flatten)]
    pub dati: DatiAttivita,
}

/// Attività aggiunta ai nodi WBS: una fase senza voci di computo (allestimento del
/// cantiere, collaudi…) o una pietra miliare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttivitaAggiunta {
    /// Identificativo.
    pub id: AttivitaId,
    /// Nome.
    pub nome: String,
    /// Nodo WBS sotto cui sta (assente: alla radice).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padre: Option<WbsNodoId>,
    /// Pietra miliare (durata zero).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pietra_miliare: bool,
    /// Dati.
    #[serde(flatten)]
    pub dati: DatiAttivita,
}

/// Periodo di chiusura del cantiere (ferie, fermo invernale…), estremi compresi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chiusura {
    /// Primo giorno.
    pub dal: Date,
    /// Ultimo giorno.
    pub al: Date,
    /// Descrizione.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub descrizione: String,
}

/// Calendario dei giorni lavorativi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calendario {
    /// Giorni lavorativi della settimana, da lunedì a domenica.
    #[serde(default = "settimana_corta")]
    pub giorni_lavorativi: [bool; 7],
    /// Festività nazionali italiane non lavorative (con il lunedì dell'Angelo).
    #[serde(default = "vero")]
    pub festivita_nazionali: bool,
    /// Chiusure del cantiere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chiusure: Vec<Chiusura>,
}

fn settimana_corta() -> [bool; 7] {
    [true, true, true, true, true, false, false]
}

fn vero() -> bool {
    true
}

impl Default for Calendario {
    fn default() -> Self {
        Self {
            giorni_lavorativi: settimana_corta(),
            festivita_nazionali: true,
            chiusure: Vec::new(),
        }
    }
}

/// Domenica di Pasqua (calendario gregoriano, algoritmo di Meeus/Jones/Butcher).
#[must_use]
pub fn pasqua(anno: i32) -> Option<Date> {
    let a = anno.rem_euclid(19);
    let b = anno.div_euclid(100);
    let c = anno.rem_euclid(100);
    let d = b.div_euclid(4);
    let e = b.rem_euclid(4);
    let f = (b + 8).div_euclid(25);
    let g = (b - f + 1).div_euclid(3);
    let h = (19 * a + b - d - g + 15).rem_euclid(30);
    let i = c.div_euclid(4);
    let k = c.rem_euclid(4);
    let l = (32 + 2 * e + 2 * i - h - k).rem_euclid(7);
    let m = (a + 11 * h + 22 * l).div_euclid(451);
    let mese = (h + l - 7 * m + 114).div_euclid(31);
    let giorno = (h + l - 7 * m + 114).rem_euclid(31) + 1;
    data_amg(anno, u32::try_from(mese).ok()?, u32::try_from(giorno).ok()?)
}

/// Festività nazionali italiane di un giorno: nome, se il giorno è festivo.
#[must_use]
pub fn festivita_nazionale(d: Date) -> Option<&'static str> {
    let nome = match (d.mese(), d.giorno()) {
        (1, 1) => "Capodanno",
        (1, 6) => "Epifania",
        (4, 25) => "Festa della Liberazione",
        (5, 1) => "Festa del lavoro",
        (6, 2) => "Festa della Repubblica",
        (8, 15) => "Ferragosto",
        (11, 1) => "Ognissanti",
        (12, 8) => "Immacolata Concezione",
        (12, 25) => "Natale",
        (12, 26) => "Santo Stefano",
        _ => "",
    };
    if !nome.is_empty() {
        return Some(nome);
    }
    let pasquetta = pasqua(d.anno()).and_then(|p| p.piu_giorni(1));
    (pasquetta == Some(d)).then_some("Lunedì dell'Angelo")
}

impl Calendario {
    /// `true` se nel calendario c'è almeno un giorno lavorativo alla settimana.
    #[must_use]
    pub fn ha_giorni_lavorativi(&self) -> bool {
        self.giorni_lavorativi.iter().any(|g| *g)
    }

    /// `true` se il giorno è lavorativo.
    #[must_use]
    pub fn lavorativo(&self, d: Date) -> bool {
        let indice = usize::try_from(d.giorno_settimana()).unwrap_or(0);
        if !self.giorni_lavorativi.get(indice).copied().unwrap_or(false) {
            return false;
        }
        if self.festivita_nazionali && festivita_nazionale(d).is_some() {
            return false;
        }
        !self.chiusure.iter().any(|c| c.dal <= d && d <= c.al)
    }
}

/// Giorno della linea di base di un'attività.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoceLineaBase {
    /// Attività.
    pub attivita: RifAttivita,
    /// Inizio.
    pub inizio: Date,
    /// Fine.
    pub fine: Date,
}

/// Linea di base: le date del piano in un momento dato, per misurare gli scostamenti.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineaBase {
    /// Giorno in cui è stata salvata.
    pub salvata_il: Date,
    /// Date delle attività visibili in quel momento.
    pub attivita: Vec<VoceLineaBase>,
}

/// Cronoprogramma dei lavori.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cronoprogramma {
    /// Inizio dei lavori (assente: la consegna dei lavori del contratto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inizio_lavori: Option<Date>,
    /// Calendario.
    #[serde(default, skip_serializing_if = "Calendario::predefinito")]
    pub calendario: Calendario,
    /// Livello di dettaglio della WBS (1 = solo i nodi radice; assente: tutti).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub livello_dettaglio: Option<u32>,
    /// Computo da cui prendere gli importi delle attività (assente: il primo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computo: Option<ComputoId>,
    /// Dati dei nodi WBS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attivita_wbs: Vec<AttivitaWbs>,
    /// Attività aggiunte.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attivita: Vec<AttivitaAggiunta>,
    /// Legami.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legami: Vec<Legame>,
    /// Linea di base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linea_base: Option<LineaBase>,
}

impl Calendario {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn predefinito(&self) -> bool {
        *self == Self::default()
    }
}

impl Cronoprogramma {
    /// `true` se non c'è nulla di pianificato (non si salva nel file).
    #[must_use]
    pub fn vuoto(&self) -> bool {
        *self == Self::default()
    }

    /// Dati di un nodo WBS (vuoti se mancano).
    #[must_use]
    pub fn dati_wbs(&self, nodo: WbsNodoId) -> DatiAttivita {
        self.attivita_wbs
            .iter()
            .find(|a| a.nodo == nodo)
            .map(|a| a.dati.clone())
            .unwrap_or_default()
    }

    /// Imposta i dati di un nodo WBS (toglie la voce se tornano vuoti).
    pub fn imposta_dati_wbs(&mut self, nodo: WbsNodoId, dati: DatiAttivita) {
        self.attivita_wbs.retain(|a| a.nodo != nodo);
        if !dati.vuoti() {
            self.attivita_wbs.push(AttivitaWbs { nodo, dati });
        }
    }

    /// Attività aggiunta per identificativo.
    #[must_use]
    pub fn attivita_aggiunta(&self, id: AttivitaId) -> Option<&AttivitaAggiunta> {
        self.attivita.iter().find(|a| a.id == id)
    }

    /// Toglie tutto ciò che riguarda un nodo WBS rimosso: i suoi dati e i legami; le
    /// attività aggiunte sotto di esso passano al nodo padre.
    pub fn rimuovi_nodo(&mut self, nodo: WbsNodoId, padre: Option<WbsNodoId>) {
        self.attivita_wbs.retain(|a| a.nodo != nodo);
        let rif = RifAttivita::Wbs(nodo);
        self.legami.retain(|l| l.da != rif && l.a != rif);
        for a in &mut self.attivita {
            if a.padre == Some(nodo) {
                a.padre = padre;
            }
        }
        if let Some(b) = &mut self.linea_base {
            b.attivita.retain(|v| v.attivita != rif);
        }
    }

    /// Toglie un'attività aggiunta e i suoi legami.
    pub fn rimuovi_attivita(&mut self, id: AttivitaId) {
        self.attivita.retain(|a| a.id != id);
        let rif = RifAttivita::Attivita(id);
        self.legami.retain(|l| l.da != rif && l.a != rif);
        if let Some(b) = &mut self.linea_base {
            b.attivita.retain(|v| v.attivita != rif);
        }
    }
}

/// Errore del calcolo del cronoprogramma.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErroreCronoprogramma {
    /// Manca la data di inizio dei lavori.
    #[error("indicare la data di inizio dei lavori (o la consegna dei lavori nel contratto)")]
    InizioMancante,
    /// Il calendario non ha giorni lavorativi.
    #[error("il calendario non ha giorni lavorativi")]
    CalendarioVuoto,
    /// I legami formano un ciclo.
    #[error("i legami formano un ciclo tra: {0}")]
    Ciclo(String),
    /// Il piano supera i limiti del calendario (oltre 200 anni).
    #[error("il piano supera i limiti del calendario")]
    FuoriScala,
    /// Errore nel calcolo degli importi.
    #[error("importi del computo non calcolabili: {0}")]
    Importi(String),
}

/// Scala dei giorni lavorativi a partire dal primo giorno lavorativo dall'inizio.
struct Scala<'a> {
    calendario: &'a Calendario,
    giorni: Vec<Date>,
}

/// Limite di sicurezza: circa 200 anni di giorni di calendario.
const LIMITE_GIORNI: i64 = 73_000;

impl<'a> Scala<'a> {
    fn new(calendario: &'a Calendario, inizio: Date) -> Result<Self, ErroreCronoprogramma> {
        if !calendario.ha_giorni_lavorativi() {
            return Err(ErroreCronoprogramma::CalendarioVuoto);
        }
        let mut d = inizio;
        let mut passi = 0;
        while !calendario.lavorativo(d) {
            d = d.giorno_dopo().ok_or(ErroreCronoprogramma::FuoriScala)?;
            passi += 1;
            if passi > LIMITE_GIORNI {
                return Err(ErroreCronoprogramma::CalendarioVuoto);
            }
        }
        Ok(Self {
            calendario,
            giorni: vec![d],
        })
    }

    fn primo(&self) -> Date {
        self.giorni.first().copied().unwrap_or_default()
    }

    /// Giorno lavorativo di indice `i` (negativo: prima dell'inizio).
    fn data(&mut self, i: i64) -> Result<Date, ErroreCronoprogramma> {
        if i < 0 {
            let mut d = self.primo();
            let mut contati = 0;
            let mut passi = 0;
            while contati < -i {
                d = d.giorno_prima().ok_or(ErroreCronoprogramma::FuoriScala)?;
                passi += 1;
                if passi > LIMITE_GIORNI {
                    return Err(ErroreCronoprogramma::FuoriScala);
                }
                if self.calendario.lavorativo(d) {
                    contati += 1;
                }
            }
            return Ok(d);
        }
        let i = usize::try_from(i).map_err(|_| ErroreCronoprogramma::FuoriScala)?;
        while self.giorni.len() <= i {
            let mut d = self.giorni.last().copied().unwrap_or_default();
            let mut passi = 0;
            loop {
                d = d.giorno_dopo().ok_or(ErroreCronoprogramma::FuoriScala)?;
                passi += 1;
                if passi > LIMITE_GIORNI || self.giorni.len() > 60_000 {
                    return Err(ErroreCronoprogramma::FuoriScala);
                }
                if self.calendario.lavorativo(d) {
                    break;
                }
            }
            self.giorni.push(d);
        }
        self.giorni
            .get(i)
            .copied()
            .ok_or(ErroreCronoprogramma::FuoriScala)
    }

    /// Indice del primo giorno lavorativo non precedente a `d` (negativo se `d` viene
    /// prima dell'inizio).
    fn indice(&mut self, d: Date) -> Result<i64, ErroreCronoprogramma> {
        let primo = self.primo();
        if d < primo {
            let mut i = 0i64;
            let mut g = primo;
            while g > d {
                g = g.giorno_prima().ok_or(ErroreCronoprogramma::FuoriScala)?;
                if g < d {
                    break;
                }
                if self.calendario.lavorativo(g) {
                    i -= 1;
                }
                if -i > LIMITE_GIORNI {
                    return Err(ErroreCronoprogramma::FuoriScala);
                }
            }
            return Ok(i);
        }
        let mut i = 0i64;
        loop {
            let g = self.data(i)?;
            if g >= d {
                return Ok(i);
            }
            i += 1;
        }
    }
}

/// Predecessore di una riga del piano, tra le righe visibili.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Predecessore {
    /// Indice della riga del predecessore.
    pub riga: usize,
    /// Tipo.
    pub tipo: TipoLegame,
    /// Ritardo.
    pub ritardo: i32,
}

/// Riga del piano calcolato, nell'ordine della WBS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RigaPiano {
    /// Attività.
    pub attivita: RifAttivita,
    /// Codice WBS completo (vuoto per le attività aggiunte).
    pub codice: String,
    /// Nome.
    pub nome: String,
    /// Profondità (1 = radice).
    pub livello: u32,
    /// Riga del riepilogo che la contiene.
    pub padre: Option<usize>,
    /// Riepilogo (date dalle attività contenute).
    pub riepilogo: bool,
    /// Pietra miliare.
    pub pietra_miliare: bool,
    /// Durata in giorni lavorativi (per i riepiloghi, dal primo inizio all'ultima fine).
    pub durata: u32,
    /// Durata non indicata (vale 1 giorno).
    pub durata_stimata: bool,
    /// Inizio al più presto.
    pub inizio: Date,
    /// Fine al più presto.
    pub fine: Date,
    /// Inizio al più tardi.
    pub inizio_tardi: Date,
    /// Fine al più tardi.
    pub fine_tardi: Date,
    /// Margine totale in giorni lavorativi.
    pub margine: i64,
    /// Sul percorso critico (margine nullo o negativo).
    pub critica: bool,
    /// Avanzamento in percentuale (per i riepiloghi, pesato sulle durate).
    pub avanzamento: u8,
    /// Vincolo «inizio non prima del».
    pub inizio_non_prima: Option<Date>,
    /// Importo dal computo (nodi WBS).
    pub importo: Option<Importo>,
    /// Inizio nella linea di base.
    pub inizio_base: Option<Date>,
    /// Fine nella linea di base.
    pub fine_base: Option<Date>,
    /// Scostamento della fine dalla linea di base, in giorni lavorativi.
    pub scostamento: Option<i64>,
    /// Predecessori visibili.
    pub predecessori: Vec<Predecessore>,
    /// Note.
    pub note: Option<String>,
    /// Indici dei giorni lavorativi (inizio compreso, fine esclusa), per il disegno.
    pub giorno_inizio: i64,
    /// Indice del giorno dopo la fine.
    pub giorno_fine: i64,
}

/// Legame tra due righe visibili del piano (per il disegno e la modifica).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegamePiano {
    /// Predecessore come memorizzato (anche un nodo non visibile a questo livello).
    pub da: RifAttivita,
    /// Successore come memorizzato.
    pub a: RifAttivita,
    /// Tipo.
    pub tipo: TipoLegame,
    /// Ritardo.
    pub ritardo: i32,
    /// Riga visibile del predecessore.
    pub riga_da: usize,
    /// Riga visibile del successore.
    pub riga_a: usize,
}

/// Produzione di un mese (cronoprogramma economico).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProduzioneMese {
    /// Anno.
    pub anno: i32,
    /// Mese (1–12).
    pub mese: u32,
    /// Importo del mese.
    pub importo: Importo,
    /// Importo cumulato.
    pub cumulato: Importo,
    /// Percentuale cumulata sull'importo pianificato (due decimali).
    pub percentuale: Decimal,
}

/// Piano calcolato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Piano {
    /// Righe nell'ordine della WBS.
    pub righe: Vec<RigaPiano>,
    /// Legami memorizzati con le righe visibili che collegano.
    pub legami: Vec<LegamePiano>,
    /// Primo giorno lavorativo.
    pub inizio: Date,
    /// Ultimo giorno di lavoro.
    pub fine: Date,
    /// Durata complessiva in giorni lavorativi.
    pub durata: u32,
    /// Durata complessiva in giorni naturali e consecutivi.
    pub durata_naturale: i64,
    /// Ultimazione prevista dal contratto (consegna + tempo utile), se nota.
    pub scadenza_contrattuale: Option<Date>,
    /// Livello di dettaglio applicato.
    pub livello_dettaglio: Option<u32>,
    /// Profondità massima della WBS.
    pub livelli_wbs: u32,
    /// Computo degli importi.
    pub computo: Option<String>,
    /// Importo distribuito nel tempo.
    pub importo_pianificato: Importo,
    /// Importo delle voci senza nodo WBS (non distribuito).
    pub importo_non_assegnato: Importo,
    /// Produzione mensile.
    pub mesi: Vec<ProduzioneMese>,
    /// Giorni non lavorativi tra inizio e fine (per l'ombreggiatura del diagramma).
    pub giorni_non_lavorativi: Vec<Date>,
}

/// Elemento del piano in costruzione.
struct Elemento {
    attivita: RifAttivita,
    codice: String,
    nome: String,
    livello: u32,
    padre: Option<usize>,
    figli: Vec<usize>,
    pietra_miliare: bool,
    dati: DatiAttivita,
    importo_cumulato: Option<Decimal>,
}

/// Profondità di un nodo WBS (1 = radice).
fn profondita(wbs: &StrutturaWbs, nodo: WbsNodoId) -> u32 {
    wbs.percorso(nodo)
        .map(|p| u32::try_from(p.len()).unwrap_or(u32::MAX))
        .unwrap_or(1)
}

/// Struttura visibile del piano e corrispondenza dei riferimenti nascosti.
struct Struttura {
    elementi: Vec<Elemento>,
    per_rif: HashMap<RifAttivita, usize>,
}

fn struttura(
    p: &Progetto,
    livello: Option<u32>,
    importi: &BTreeMap<WbsNodoId, Decimal>,
) -> Struttura {
    let c = &p.cronoprogramma;
    let visibile = |prof: u32| livello.is_none_or(|l| prof <= l);
    let mut s = Struttura {
        elementi: Vec::new(),
        per_rif: HashMap::new(),
    };
    fn aggiungi_attivita(
        s: &mut Struttura,
        c: &Cronoprogramma,
        padre_wbs: Option<WbsNodoId>,
        padre: Option<usize>,
        prof: u32,
    ) {
        for a in c.attivita.iter().filter(|a| a.padre == padre_wbs) {
            let i = s.elementi.len();
            s.elementi.push(Elemento {
                attivita: RifAttivita::Attivita(a.id),
                codice: String::new(),
                nome: a.nome.clone(),
                livello: prof,
                padre,
                figli: Vec::new(),
                pietra_miliare: a.pietra_miliare,
                dati: a.dati.clone(),
                importo_cumulato: None,
            });
            s.per_rif.insert(RifAttivita::Attivita(a.id), i);
            if let Some(pp) = padre.and_then(|pp| s.elementi.get_mut(pp)) {
                pp.figli.push(i);
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn visita(
        p: &Progetto,
        s: &mut Struttura,
        padre_wbs: Option<WbsNodoId>,
        padre: Option<usize>,
        prof: u32,
        visibile: &dyn Fn(u32) -> bool,
        importi: &BTreeMap<WbsNodoId, Decimal>,
    ) {
        if !visibile(prof) {
            return;
        }
        for n in p.wbs.figli(padre_wbs) {
            let i = s.elementi.len();
            s.elementi.push(Elemento {
                attivita: RifAttivita::Wbs(n.id),
                codice: p.wbs.codice_completo(n.id).unwrap_or_default(),
                nome: n.descrizione.clone(),
                livello: prof,
                padre,
                figli: Vec::new(),
                pietra_miliare: false,
                dati: p.cronoprogramma.dati_wbs(n.id),
                importo_cumulato: importi.get(&n.id).copied().or(Some(Decimal::ZERO)),
            });
            s.per_rif.insert(RifAttivita::Wbs(n.id), i);
            if let Some(pp) = padre.and_then(|pp| s.elementi.get_mut(pp)) {
                pp.figli.push(i);
            }
            visita(p, s, Some(n.id), Some(i), prof + 1, visibile, importi);
            if visibile(prof + 1) {
                aggiungi_attivita(s, &p.cronoprogramma, Some(n.id), Some(i), prof + 1);
            }
        }
    }
    visita(p, &mut s, None, None, 1, &visibile, importi);
    aggiungi_attivita(&mut s, c, None, None, 1);
    s
}

impl Struttura {
    /// Elemento visibile che rappresenta un riferimento (sé stesso o l'antenato visibile).
    fn visibile(&self, p: &Progetto, rif: RifAttivita) -> Option<usize> {
        if let Some(i) = self.per_rif.get(&rif) {
            return Some(*i);
        }
        let mut nodo = match rif {
            RifAttivita::Wbs(n) => p.wbs.nodo(n).and_then(|x| x.padre),
            RifAttivita::Attivita(a) => p.cronoprogramma.attivita_aggiunta(a)?.padre,
        };
        let mut passi = 0;
        while let Some(n) = nodo {
            if let Some(i) = self.per_rif.get(&RifAttivita::Wbs(n)) {
                return Some(*i);
            }
            nodo = p.wbs.nodo(n).and_then(|x| x.padre);
            passi += 1;
            if passi > p.wbs.nodi.len() {
                return None;
            }
        }
        None
    }

    fn foglie_di(&self, i: usize, out: &mut Vec<usize>) {
        match self.elementi.get(i) {
            Some(e) if e.figli.is_empty() => out.push(i),
            Some(e) => {
                for f in &e.figli {
                    self.foglie_di(*f, out);
                }
            }
            None => {}
        }
    }

    fn antenato(&self, a: usize, di: usize) -> bool {
        let mut c = self.elementi.get(di).and_then(|e| e.padre);
        while let Some(x) = c {
            if x == a {
                return true;
            }
            c = self.elementi.get(x).and_then(|e| e.padre);
        }
        false
    }
}

/// Il nodo WBS ha nodi figli o attività aggiunte (nascoste a un livello ridotto).
fn nasconde_attivita(p: &Progetto, rif: RifAttivita) -> bool {
    let RifAttivita::Wbs(n) = rif else {
        return false;
    };
    p.wbs.nodi.iter().any(|x| x.padre == Some(n))
        || p.cronoprogramma.attivita.iter().any(|a| a.padre == Some(n))
}

fn durata(e: &Elemento) -> (i64, bool) {
    if e.pietra_miliare {
        return (0, false);
    }
    match e.dati.durata {
        Some(d) => (i64::from(d), false),
        None => (1, true),
    }
}

/// Inizio dei lavori: quello del cronoprogramma o la consegna del contratto.
#[must_use]
pub fn inizio_lavori(p: &Progetto) -> Option<Date> {
    p.cronoprogramma
        .inizio_lavori
        .or_else(|| p.contratto.as_ref().and_then(|c| c.data_consegna_lavori))
}

/// Calcola il piano del cronoprogramma con il livello di dettaglio indicato (assente:
/// quello del cronoprogramma).
///
/// # Errors
/// Inizio mancante, calendario senza giorni lavorativi, legami in ciclo, piano fuori
/// scala, computo non calcolabile.
#[allow(clippy::too_many_lines)]
pub fn calcola(p: &Progetto, livello: Option<Option<u32>>) -> Result<Piano, ErroreCronoprogramma> {
    let c = &p.cronoprogramma;
    let livello = livello.unwrap_or(c.livello_dettaglio).filter(|l| *l > 0);
    let inizio = inizio_lavori(p).ok_or(ErroreCronoprogramma::InizioMancante)?;
    let mut scala = Scala::new(&c.calendario, inizio)?;

    // Importi del computo per nodo WBS (cumulati sui discendenti).
    let computo = c
        .computo
        .and_then(|id| p.computo(id))
        .or_else(|| p.computi.first());
    let (importi, non_assegnato) = match computo {
        Some(comp) => {
            let r = p
                .calcola_computo(comp.id)
                .map_err(|e| ErroreCronoprogramma::Importi(e.to_string()))?;
            let cumulati = r
                .totali_wbs_cumulati(&p.wbs)
                .map_err(|e| ErroreCronoprogramma::Importi(e.to_string()))?;
            (
                cumulati.into_iter().map(|(k, v)| (k, v.valore())).collect(),
                r.per_wbs.non_assegnato.valore(),
            )
        }
        None => (BTreeMap::new(), Decimal::ZERO),
    };

    let s = struttura(p, livello, &importi);
    let n = s.elementi.len();

    // A un livello ridotto, un nodo che nasconde attività e non ha una durata propria
    // dura quanto il suo riepilogo nel piano completo.
    let durate_dettaglio: HashMap<RifAttivita, i64> = if livello.is_some()
        && s.elementi.iter().any(|e| {
            e.figli.is_empty()
                && e.dati.durata.is_none()
                && !e.pietra_miliare
                && nasconde_attivita(p, e.attivita)
        }) {
        calcola(p, Some(None))?
            .righe
            .iter()
            .filter(|r| r.riepilogo)
            .map(|r| (r.attivita, i64::from(r.durata)))
            .collect()
    } else {
        HashMap::new()
    };
    let foglie: Vec<usize> = (0..n)
        .filter(|i| s.elementi.get(*i).is_some_and(|e| e.figli.is_empty()))
        .collect();

    // Legami tra le righe visibili e vincoli tra le foglie.
    let mut legami_visibili: Vec<(usize, usize, TipoLegame, i32)> = Vec::new();
    let mut legami_piano = Vec::new();
    for l in &c.legami {
        let (Some(da), Some(a)) = (s.visibile(p, l.da), s.visibile(p, l.a)) else {
            continue;
        };
        if da == a || s.antenato(da, a) || s.antenato(a, da) {
            continue;
        }
        legami_piano.push(LegamePiano {
            da: l.da,
            a: l.a,
            tipo: l.tipo,
            ritardo: l.ritardo,
            riga_da: da,
            riga_a: a,
        });
        if !legami_visibili
            .iter()
            .any(|(x, y, t, r)| *x == da && *y == a && *t == l.tipo && *r == l.ritardo)
        {
            legami_visibili.push((da, a, l.tipo, l.ritardo));
        }
    }
    let mut entranti: Vec<Vec<(usize, TipoLegame, i64)>> = vec![Vec::new(); n];
    let mut uscenti: Vec<Vec<(usize, TipoLegame, i64)>> = vec![Vec::new(); n];
    for (da, a, tipo, ritardo) in &legami_visibili {
        let mut fda = Vec::new();
        let mut fa = Vec::new();
        s.foglie_di(*da, &mut fda);
        s.foglie_di(*a, &mut fa);
        for x in &fda {
            for y in &fa {
                if x == y {
                    continue;
                }
                if let Some(v) = entranti.get_mut(*y) {
                    v.push((*x, *tipo, i64::from(*ritardo)));
                }
                if let Some(v) = uscenti.get_mut(*x) {
                    v.push((*y, *tipo, i64::from(*ritardo)));
                }
            }
        }
    }

    // Ordine topologico delle foglie.
    let mut grado: Vec<usize> = (0..n)
        .map(|i| entranti.get(i).map_or(0, Vec::len))
        .collect();
    let mut coda: Vec<usize> = foglie
        .iter()
        .copied()
        .filter(|i| grado.get(*i) == Some(&0))
        .collect();
    coda.reverse();
    let mut ordine = Vec::with_capacity(foglie.len());
    while let Some(i) = coda.pop() {
        ordine.push(i);
        for (j, _, _) in uscenti.get(i).map(Vec::as_slice).unwrap_or_default() {
            if let Some(g) = grado.get_mut(*j) {
                *g -= 1;
                if *g == 0 {
                    coda.push(*j);
                }
            }
        }
    }
    if ordine.len() < foglie.len() {
        let in_ciclo: Vec<String> = foglie
            .iter()
            .filter(|i| !ordine.contains(i))
            .filter_map(|i| s.elementi.get(*i))
            .map(|e| {
                if e.codice.is_empty() {
                    e.nome.clone()
                } else {
                    format!("{} {}", e.codice, e.nome)
                }
            })
            .collect();
        return Err(ErroreCronoprogramma::Ciclo(in_ciclo.join(", ")));
    }

    // Passata in avanti.
    let mut inizio_presto = vec![0i64; n];
    let mut fine_presto = vec![0i64; n];
    let mut durate = vec![0i64; n];
    let mut stimate = vec![false; n];
    for i in &ordine {
        let Some(e) = s.elementi.get(*i) else {
            continue;
        };
        let (d, stimata) = match (e.dati.durata, durate_dettaglio.get(&e.attivita)) {
            (None, Some(dd)) if !e.pietra_miliare => (*dd, false),
            _ => durata(e),
        };
        let mut es = 0i64;
        if let Some(v) = e.dati.inizio_non_prima {
            es = es.max(scala.indice(v)?);
        }
        for (j, tipo, rit) in entranti.get(*i).map(Vec::as_slice).unwrap_or_default() {
            let sj = inizio_presto.get(*j).copied().unwrap_or(0);
            let ej = fine_presto.get(*j).copied().unwrap_or(0);
            let vincolo = match tipo {
                TipoLegame::FineInizio => ej + rit,
                TipoLegame::InizioInizio => sj + rit,
                TipoLegame::FineFine => ej + rit - d,
                TipoLegame::InizioFine => sj + rit - d,
            };
            es = es.max(vincolo);
        }
        if let (Some(a), Some(b), Some(dd), Some(st)) = (
            inizio_presto.get_mut(*i),
            fine_presto.get_mut(*i),
            durate.get_mut(*i),
            stimate.get_mut(*i),
        ) {
            *a = es;
            *b = es + d;
            *dd = d;
            *st = stimata;
        }
    }
    let fine_progetto = foglie
        .iter()
        .filter_map(|i| fine_presto.get(*i))
        .copied()
        .max()
        .unwrap_or(0)
        .max(
            foglie
                .iter()
                .filter_map(|i| inizio_presto.get(*i))
                .copied()
                .max()
                .unwrap_or(0),
        );

    // Passata all'indietro.
    let mut fine_tardi = vec![fine_progetto; n];
    for i in ordine.iter().rev() {
        let d = durate.get(*i).copied().unwrap_or(0);
        let mut lf = fine_progetto;
        for (j, tipo, rit) in uscenti.get(*i).map(Vec::as_slice).unwrap_or_default() {
            let fj = fine_tardi.get(*j).copied().unwrap_or(fine_progetto);
            let dj = durate.get(*j).copied().unwrap_or(0);
            let sj = fj - dj;
            let vincolo = match tipo {
                TipoLegame::FineInizio => sj - rit,
                TipoLegame::InizioInizio => sj - rit + d,
                TipoLegame::FineFine => fj - rit,
                TipoLegame::InizioFine => fj - rit + d,
            };
            lf = lf.min(vincolo);
        }
        if let Some(x) = fine_tardi.get_mut(*i) {
            *x = lf;
        }
    }

    // Riepiloghi dal basso: estremi dei figli.
    let mut inizio_t = vec![0i64; n];
    for i in &foglie {
        let d = durate.get(*i).copied().unwrap_or(0);
        if let (Some(x), Some(lf)) = (inizio_t.get_mut(*i), fine_tardi.get(*i)) {
            *x = lf - d;
        }
    }
    let mut margine = vec![0i64; n];
    for i in &foglie {
        if let (Some(m), Some(lt), Some(es)) =
            (margine.get_mut(*i), inizio_t.get(*i), inizio_presto.get(*i))
        {
            *m = lt - es;
        }
    }
    for i in (0..n).rev() {
        let Some(e) = s.elementi.get(i) else { continue };
        if e.figli.is_empty() {
            continue;
        }
        let es = e
            .figli
            .iter()
            .filter_map(|f| inizio_presto.get(*f))
            .min()
            .copied();
        let ef = e
            .figli
            .iter()
            .filter_map(|f| fine_presto.get(*f))
            .max()
            .copied();
        let ls = e
            .figli
            .iter()
            .filter_map(|f| inizio_t.get(*f))
            .min()
            .copied();
        let lf = e
            .figli
            .iter()
            .filter_map(|f| fine_tardi.get(*f))
            .max()
            .copied();
        let m = e
            .figli
            .iter()
            .filter_map(|f| margine.get(*f))
            .min()
            .copied();
        if let (Some(es), Some(ef), Some(ls), Some(lf), Some(m)) = (es, ef, ls, lf, m) {
            if let Some(x) = inizio_presto.get_mut(i) {
                *x = es;
            }
            if let Some(x) = fine_presto.get_mut(i) {
                *x = ef;
            }
            if let Some(x) = inizio_t.get_mut(i) {
                *x = ls;
            }
            if let Some(x) = fine_tardi.get_mut(i) {
                *x = lf;
            }
            if let Some(x) = margine.get_mut(i) {
                *x = m;
            }
        }
    }

    // Avanzamento dei riepiloghi pesato sulle durate delle foglie.
    let avanzamento_di = |i: usize| -> u8 {
        let mut f = Vec::new();
        s.foglie_di(i, &mut f);
        let peso: i64 = f
            .iter()
            .map(|x| durate.get(*x).copied().unwrap_or(0).max(0))
            .sum();
        let somma: i64 = f
            .iter()
            .map(|x| {
                let a = s
                    .elementi
                    .get(*x)
                    .map_or(0, |e| i64::from(e.dati.avanzamento.min(100)));
                a * durate.get(*x).copied().unwrap_or(0).max(0)
            })
            .sum();
        if peso == 0 {
            let n = i64::try_from(f.len().max(1)).unwrap_or(1);
            let media: i64 = f
                .iter()
                .filter_map(|x| s.elementi.get(*x))
                .map(|e| i64::from(e.dati.avanzamento.min(100)))
                .sum::<i64>()
                / n;
            return u8::try_from(media).unwrap_or(100);
        }
        u8::try_from((somma + peso / 2) / peso).unwrap_or(100)
    };

    // Linea di base.
    let base: HashMap<RifAttivita, (Date, Date)> = c
        .linea_base
        .as_ref()
        .map(|b| {
            b.attivita
                .iter()
                .map(|v| (v.attivita, (v.inizio, v.fine)))
                .collect()
        })
        .unwrap_or_default();

    let mut righe = Vec::with_capacity(n);
    for (i, e) in s.elementi.iter().enumerate() {
        let es = inizio_presto.get(i).copied().unwrap_or(0);
        let ef = fine_presto.get(i).copied().unwrap_or(0);
        let ls = inizio_t.get(i).copied().unwrap_or(0);
        let lf = fine_tardi.get(i).copied().unwrap_or(0);
        let riepilogo = !e.figli.is_empty();
        let pietra = e.pietra_miliare || (!riepilogo && durate.get(i) == Some(&0));
        // La fine è l'ultimo giorno di lavoro; una pietra miliare cade alla fine del
        // giorno lavorativo che la precede (come la fine del suo predecessore).
        let fine_data = |scala: &mut Scala<'_>, a: i64, b: i64| {
            if b > a {
                scala.data(b - 1)
            } else {
                scala.data((a - 1).max(0))
            }
        };
        let fine = fine_data(&mut scala, es, ef)?;
        let inizio_riga = if pietra { fine } else { scala.data(es)? };
        let (inizio_base, fine_base) = base
            .get(&e.attivita)
            .map_or((None, None), |(a, b)| (Some(*a), Some(*b)));
        let scostamento = match fine_base {
            Some(fb) => Some(scala.indice(fine)? - scala.indice(fb)?),
            None => None,
        };
        let predecessori = legami_visibili
            .iter()
            .filter(|(_, a, _, _)| *a == i)
            .map(|(da, _, tipo, ritardo)| Predecessore {
                riga: *da,
                tipo: *tipo,
                ritardo: *ritardo,
            })
            .collect();
        let importo = if matches!(e.attivita, RifAttivita::Wbs(_)) {
            e.importo_cumulato.map(Importo::new)
        } else {
            None
        };
        righe.push(RigaPiano {
            attivita: e.attivita,
            codice: e.codice.clone(),
            nome: e.nome.clone(),
            livello: e.livello,
            padre: e.padre,
            riepilogo,
            pietra_miliare: pietra,
            durata: u32::try_from((ef - es).max(0)).unwrap_or(u32::MAX),
            durata_stimata: !riepilogo && stimate.get(i).copied().unwrap_or(false),
            inizio: inizio_riga,
            fine,
            inizio_tardi: if pietra {
                fine_data(&mut scala, ls, lf)?
            } else {
                scala.data(ls)?
            },
            fine_tardi: fine_data(&mut scala, ls, lf)?,
            margine: margine.get(i).copied().unwrap_or(0),
            critica: !riepilogo && margine.get(i).copied().unwrap_or(0) <= 0,
            avanzamento: if riepilogo {
                avanzamento_di(i)
            } else {
                e.dati.avanzamento.min(100)
            },
            inizio_non_prima: e.dati.inizio_non_prima,
            importo,
            inizio_base,
            fine_base,
            scostamento,
            predecessori,
            note: e.dati.note.clone(),
            giorno_inizio: es,
            giorno_fine: ef,
        });
    }

    // Produzione mensile: importi delle foglie WBS e quote dirette dei riepiloghi,
    // ripartiti sui giorni lavorativi con somme esatte.
    let mut per_mese: BTreeMap<(i32, u32), Decimal> = BTreeMap::new();
    let mut pianificato = Decimal::ZERO;
    for (i, e) in s.elementi.iter().enumerate() {
        let Some(cumulato) = e.importo_cumulato else {
            continue;
        };
        let figli_wbs: Decimal = e
            .figli
            .iter()
            .filter_map(|f| s.elementi.get(*f))
            .filter_map(|f| f.importo_cumulato)
            .sum();
        let proprio = cumulato - figli_wbs;
        if proprio.is_zero() {
            continue;
        }
        pianificato += proprio;
        let es = inizio_presto.get(i).copied().unwrap_or(0);
        let ef = fine_presto.get(i).copied().unwrap_or(0).max(es + 1);
        let mut giorni: BTreeMap<(i32, u32), i64> = BTreeMap::new();
        for g in es..ef {
            let d = scala.data(g)?;
            *giorni.entry((d.anno(), d.mese())).or_default() += 1;
        }
        let totale = Decimal::from(ef - es);
        let mut assegnato = Decimal::ZERO;
        let k = giorni.len();
        for (j, (mese, cnt)) in giorni.into_iter().enumerate() {
            let quota = if j + 1 == k {
                proprio - assegnato
            } else {
                p.regole
                    .arrotonda_importo(proprio * Decimal::from(cnt) / totale)
                    .valore()
            };
            assegnato += quota;
            *per_mese.entry(mese).or_default() += quota;
        }
    }
    let inizio_piano = scala.data(0)?;
    let fine_piano = if fine_progetto > 0 {
        scala.data(fine_progetto - 1)?
    } else {
        inizio_piano
    };
    let mut mesi = Vec::new();
    if !per_mese.is_empty() {
        let mut cumulato = Decimal::ZERO;
        let (mut a, mut m) = (inizio_piano.anno(), inizio_piano.mese());
        let ultimo = per_mese
            .keys()
            .next_back()
            .copied()
            .unwrap_or((fine_piano.anno(), fine_piano.mese()))
            .max((fine_piano.anno(), fine_piano.mese()));
        let primo = per_mese
            .keys()
            .next()
            .copied()
            .unwrap_or((a, m))
            .min((a, m));
        (a, m) = primo;
        while (a, m) <= ultimo {
            let importo = per_mese.get(&(a, m)).copied().unwrap_or_default();
            cumulato += importo;
            let percentuale = if pianificato.is_zero() {
                Decimal::ZERO
            } else {
                (cumulato * Decimal::ONE_HUNDRED / pianificato)
                    .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
            };
            mesi.push(ProduzioneMese {
                anno: a,
                mese: m,
                importo: Importo::new(importo),
                cumulato: Importo::new(cumulato),
                percentuale,
            });
            if m == 12 {
                a += 1;
                m = 1;
            } else {
                m += 1;
            }
        }
    }

    let scadenza_contrattuale = p.contratto.as_ref().and_then(|ct| {
        let consegna = ct.data_consegna_lavori?;
        let giorni = ct.tempo_utile_giorni?;
        consegna.piu_giorni(i64::from(giorni) - 1)
    });
    let mut non_lavorativi = Vec::new();
    let mut d = inizio_piano;
    while d <= fine_piano {
        if !c.calendario.lavorativo(d) {
            non_lavorativi.push(d);
        }
        match d.giorno_dopo() {
            Some(x) => d = x,
            None => break,
        }
    }
    let livelli_wbs = p
        .wbs
        .nodi
        .iter()
        .map(|nodo| profondita(&p.wbs, nodo.id))
        .max()
        .unwrap_or(0);
    Ok(Piano {
        righe,
        legami: legami_piano,
        inizio: inizio_piano,
        fine: fine_piano,
        durata: u32::try_from(fine_progetto.max(0)).unwrap_or(u32::MAX),
        durata_naturale: fine_piano.giorni_da(inizio_piano) + 1,
        scadenza_contrattuale,
        livello_dettaglio: livello,
        livelli_wbs,
        computo: computo.map(|c| c.nome.clone()),
        importo_pianificato: Importo::new(pianificato),
        importo_non_assegnato: Importo::new(non_assegnato),
        mesi,
        giorni_non_lavorativi: non_lavorativi,
    })
}

/// Verifica che i legami non formino cicli (con tutti i livelli della WBS).
///
/// # Errors
/// Il ciclo trovato.
pub fn verifica_legami(p: &Progetto) -> Result<(), ErroreCronoprogramma> {
    // Basta il grafo dei riferimenti: un ciclo tra attività visibili a un livello è un
    // ciclo anche al livello più dettagliato, salvo legami tra un nodo e i suoi
    // discendenti, che il calcolo ignora.
    let mut adiacenti: HashMap<RifAttivita, Vec<RifAttivita>> = HashMap::new();
    for l in &p.cronoprogramma.legami {
        adiacenti.entry(l.da).or_default().push(l.a);
    }
    let mut stato: HashMap<RifAttivita, u8> = HashMap::new();
    /// Visita in profondità; restituisce il ciclo trovato (dal primo nodo ripetuto).
    fn dfs(
        r: RifAttivita,
        adiacenti: &HashMap<RifAttivita, Vec<RifAttivita>>,
        stato: &mut HashMap<RifAttivita, u8>,
        pila: &mut Vec<RifAttivita>,
    ) -> Option<Vec<RifAttivita>> {
        stato.insert(r, 1);
        pila.push(r);
        for s in adiacenti.get(&r).map(Vec::as_slice).unwrap_or_default() {
            match stato.get(s) {
                Some(1) => {
                    let da = pila.iter().position(|x| x == s).unwrap_or(0);
                    return Some(pila.get(da..).map(<[_]>::to_vec).unwrap_or_default());
                }
                Some(_) => {}
                None => {
                    if let Some(c) = dfs(*s, adiacenti, stato, pila) {
                        return Some(c);
                    }
                }
            }
        }
        pila.pop();
        stato.insert(r, 2);
        None
    }
    let mut nodi: Vec<RifAttivita> = adiacenti.keys().copied().collect();
    nodi.sort_by_key(|r| format!("{r:?}"));
    for r in nodi {
        if stato.contains_key(&r) {
            continue;
        }
        let mut pila = Vec::new();
        if let Some(ciclo) = dfs(r, &adiacenti, &mut stato, &mut pila) {
            let nomi: Vec<String> = ciclo.iter().map(|x| nome_attivita(p, *x)).collect();
            return Err(ErroreCronoprogramma::Ciclo(nomi.join(" → ")));
        }
    }
    Ok(())
}

/// Nome di un'attività per i messaggi: codice WBS e descrizione, o nome.
fn nome_attivita(p: &Progetto, r: RifAttivita) -> String {
    match r {
        RifAttivita::Wbs(n) => {
            let codice = p.wbs.codice_completo(n).unwrap_or_default();
            let descrizione = p.wbs.nodo(n).map(|x| x.descrizione.as_str()).unwrap_or("");
            format!("{codice} {descrizione}").trim().to_owned()
        }
        RifAttivita::Attivita(a) => p
            .cronoprogramma
            .attivita_aggiunta(a)
            .map(|x| x.nome.clone())
            .unwrap_or_default(),
    }
}

/// Linea di base dalle date di un piano calcolato.
#[must_use]
pub fn linea_base(piano: &Piano, salvata_il: Date) -> LineaBase {
    LineaBase {
        salvata_il,
        attivita: piano
            .righe
            .iter()
            .map(|r| VoceLineaBase {
                attivita: r.attivita,
                inizio: r.inizio,
                fine: r.fine,
            })
            .collect(),
    }
}
