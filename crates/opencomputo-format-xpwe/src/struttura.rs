//! Struttura del formato XPWE, in parte **verificata** e in parte **dedotta**.
//!
//! Non esiste una specifica pubblica di XPWE. Le fonti sono:
//!
//! * un **export reale** di PriMus-DCF, versione del formato 5.01, di un documento
//!   vuoto (2026-10-20, fixture `tests/fixtures/primus_dcf_5_01_vuoto.xpwe`):
//!   conferma testata, dati generali, configurazione dei numeri e il fatto che
//!   l'elenco prezzi vuoto è omesso;
//! * il codice di importazione di LeenO (LGPL-2.1, commit `40966e3`), usato come fonte
//!   di fatti sul formato e non come codice (vedi `docs/adr/0006-formati-primus.md`).
//!
//! Grado di certezza:
//!
//! * **verificato**: osservato nell'export reale di PriMus-DCF;
//! * **letto da LeenO**: nomi che LeenO cerca esplicitamente, non ancora osservati;
//! * **ipotesi**: nomi che LeenO non usa; l'import riconosce la sezione anche dal
//!   contenuto, l'export usa i nomi osservati nel file di origine se presenti.
//!
//! Voci di elenco, voci di computo e righi di misura **non sono ancora verificati**:
//! l'export reale disponibile non ne contiene.

/// Radice del documento (**verificato**).
pub const RADICE: &str = "PweDocumento";

/// Campi di testata del documento, nell'ordine osservato (**verificato**, PriMus-DCF 5.01).
pub const TESTATA: [&str; 7] = [
    "CopyRight",
    "TipoDocumento",
    "TipoFormato",
    "Versione",
    "SourceVersione",
    "SourceNome",
    "FileNameDocumento",
];
/// Versione del formato osservata e scritta dall'export.
pub const VERSIONE_FORMATO: &str = "5.01";

/// Dati generali (**verificato**).
pub const DATI_GENERALI: &str = "PweDatiGenerali";
/// Contenitore dei dati anagrafici (**verificato**).
pub const DG_PROGETTO: &str = "PweDGProgetto";
/// Dati anagrafici (**verificato**).
pub const DG_DATI_GENERALI: &str = "PweDGDatiGenerali";
/// Capitoli e categorie (**verificato** il contenitore, vuoto nell'export osservato).
pub const DG_CAPITOLI_CATEGORIE: &str = "PweDGCapitoliCategorie";
/// Moduli, contiene i dati generali delle analisi (letto da LeenO).
pub const DG_MODULI: &str = "PweDGModuli";
/// WBS (**verificato**, conservato e non interpretato).
pub const DG_WBS: &str = "PweDGWBS";
/// WBS dei capitoli (**verificato**, conservato e non interpretato).
pub const DG_WBS_CAP: &str = "PweDGWBSCAP";
/// Configurazione (**verificato**).
pub const DG_CONFIGURAZIONE: &str = "PweDGConfigurazione";
/// Configurazione dei numeri (**verificato**). I campi hanno la forma `L.D|0`, dove
/// `D` sono i decimali (**verificato**: `Quantita` `11.2|0`).
pub const DG_CONFIG_NUMERI: &str = "PweDGConfigNumeri";
/// Configurazione dei numeri dell'export osservato, scritta nei documenti nuovi
/// (i decimali di quantità, prezzi e importi sono sostituiti da quelli del progetto).
pub const CONFIG_NUMERI_PREDEFINITA: [(&str, &str); 15] = [
    ("Divisa", "eur"),
    ("ConversioniIN", "lire"),
    ("FattoreConversione", "1936.27"),
    ("Cambio", "1"),
    ("PartiUguali", "8.3|0"),
    ("Lunghezza", "9.3|0"),
    ("Larghezza", "9.3|0"),
    ("HPeso", "10.3|0"),
    ("Quantita", "11.2|0"),
    ("Prezzi", "10.2|0"),
    ("PrezziTotale", "14.2|0"),
    ("ConvPrezzi", "11.2|0"),
    ("ConvPrezziTotale", "15.2|0"),
    ("IncidenzaPercentuale", "7.4|0"),
    ("Aliquote", "7.4|0"),
];

/// Misurazioni: elenco prezzi e voci di computo (**verificato**).
pub const MISURAZIONI: &str = "PweMisurazioni";
/// Elenco prezzi (**ipotesi** sul nome; **verificato** che è omesso quando è vuoto).
pub const ELENCO_PREZZI: &str = "PweElencoPrezzi";
/// Voce di elenco prezzi (letto da LeenO).
pub const EP_ITEM: &str = "EPItem";
/// Voci di computo (**verificato**).
pub const VOCI_COMPUTO: &str = "PweVociComputo";
/// Voce di computo (letto da LeenO).
pub const VC_ITEM: &str = "VCItem";
/// Righi di misura di una voce (**ipotesi**: LeenO usa l'ultimo figlio della voce).
pub const VC_MISURE: &str = "PweVCMisure";
/// Rigo di misura (letto da LeenO).
pub const RG_ITEM: &str = "RGItem";
/// Analisi del prezzo di una voce di elenco (letto da LeenO; non importata).
pub const EP_ANALISI: &str = "PweEPAnalisi";

/// Elenchi di capitoli e categorie: (nome dell'elenco, nome predefinito degli
/// elementi). I nomi degli elenchi sono letti da LeenO; i nomi degli elementi sono
/// **ipotesi** (LeenO legge tutti i figli senza controllarne il nome).
pub const ELENCHI_CAPITOLI: [(&str, &str); 3] = [
    ("PweDGSuperCapitoli", "DGSuperCapitoliItem"),
    ("PweDGCapitoli", "DGCapitoliItem"),
    ("PweDGSubCapitoli", "DGSubCapitoliItem"),
];
/// Elenchi delle categorie, nell'ordine super / categoria / sotto.
pub const ELENCHI_CATEGORIE: [(&str, &str); 3] = [
    ("PweDGSuperCategorie", "DGSuperCategorieItem"),
    ("PweDGCategorie", "DGCategorieItem"),
    ("PweDGSubCategorie", "DGSubCategorieItem"),
];

/// Bit di `Flags` di una voce di elenco che indica una voce della sicurezza
/// (letto da LeenO: valore `134217728`, bit 27).
pub const FLAG_VOCE_SICUREZZA: u64 = 1 << 27;
/// Bit di `Flags` di un rigo di misura che indica un rigo «parziale» (subtotale)
/// (letto da LeenO: valore `2`).
pub const FLAG_RIGO_PARZIALE: u64 = 2;
/// Valore di `IDVV` che indica l'assenza di un «vedi voce» (letto da LeenO).
pub const IDVV_NESSUNO: &str = "-2";

/// Ordine predefinito dei campi di una voce di elenco prezzi (**ipotesi**:
/// ordine dei campi letti da LeenO).
pub const ORDINE_EP: [&str; 23] = [
    "TipoEP",
    "Tariffa",
    "Articolo",
    "DesRidotta",
    "DesEstesa",
    "DesBreve",
    "UnMisura",
    "Prezzo1",
    "Prezzo2",
    "Prezzo3",
    "Prezzo4",
    "Prezzo5",
    "IDSpCap",
    "IDCap",
    "IDSbCap",
    "Flags",
    "Data",
    "AdrInternet",
    "IncSIC",
    "IncMDO",
    "IncMAT",
    "IncATTR",
    EP_ANALISI,
];
/// Ordine predefinito dei campi di una voce di computo (**ipotesi**).
pub const ORDINE_VC: [&str; 8] = [
    "IDEP", "Quantita", "DataMis", "Flags", "IDSpCat", "IDCat", "IDSbCat", VC_MISURE,
];
/// Ordine predefinito dei campi di un rigo di misura (**ipotesi**).
pub const ORDINE_RG: [&str; 8] = [
    "IDVV",
    "Descrizione",
    "PartiUguali",
    "Lunghezza",
    "Larghezza",
    "HPeso",
    "Quantita",
    "Flags",
];
/// Ordine predefinito dei campi di capitoli e categorie (letto da LeenO).
pub const ORDINE_CLASSE: [&str; 4] = ["Codice", "DesSintetica", "DesEstesa", "Percentuale"];
/// Ordine dei dati anagrafici (**verificato**).
pub const ORDINE_DATI_GENERALI: [&str; 7] = [
    "PercPrezzi",
    "Comune",
    "Provincia",
    "Oggetto",
    "Committente",
    "Impresa",
    "ParteOpera",
];
