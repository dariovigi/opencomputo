//! # opencomputo-dcf-parser
//!
//! Lettura dei file DCF dell'ecosistema PriMus, limitata ai contenuti **non
//! protetti**, ed **export sperimentale** dell'elenco prezzi in DCF.
//!
//! * Il contenitore binario è descritto in [`contenitore`] (lettura dei flussi) e in
//!   [`struttura`] (indice, somme di controllo), ricostruiti dall'analisi di file reali
//!   (vedi `docs/adr/0006-formati-primus.md` e `docs/adr/0016-export-dcf.md`).
//! * [`scrittura`] scrive il contenitore, [`esporta`] l'elenco prezzi in chiaro, nella
//!   forma dei prezzari distribuiti in DCF. Computo e contabilità non si esportano: nei
//!   documenti del programma di riferimento sono protetti.
//! * I flussi protetti sono riconosciuti e **mai decifrati**: se i dati del
//!   documento sono protetti, o se è impostata una password di apertura, l'import si
//!   ferma con un errore che suggerisce l'export XPWE da PriMus.
//! * Sono importati elenco prezzi, capitoli, categorie e dati generali. Le voci di
//!   computo e le analisi non sono importate.
//!
//! Il crate non usa filesystem né rete: riceve i byte del file.
//!
//! **Licenze dei contenuti**: leggere un formato di file è lecito, ma i documenti
//! possono essere soggetti a condizioni d'uso proprie (ad esempio i prezzari
//! distribuiti da ACCA vietano la conversione e l'uso con software di terzi). Il
//! rispetto di tali condizioni resta responsabilità di chi importa il file.

pub mod contenitore;
pub mod errore;
pub mod esporta;
pub mod importa;
pub mod scrittura;
pub mod struttura;
pub mod xml;

pub use errore::ErroreDcf;
pub use importa::{OpzioniImport, RapportoImport, RisultatoImport, Statistiche, importa};
