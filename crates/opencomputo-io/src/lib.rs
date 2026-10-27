//! # opencomputo-io
//!
//! Report del computo e della contabilità ed export in **PDF**, **XLSX** e **CSV**.
//!
//! I report sono costruiti una volta sola come [`documento::Documento`] dai valori
//! calcolati dal core ([`report`]) e poi scritti nei tre formati ([`pdf`], [`xlsx`],
//! [`csv`]): quantità, importi e totali coincidono in tutti i formati.
//!
//! Il crate non usa filesystem né rete: restituisce byte e testo.
//!
//! ```
//! use opencomputo_core::schema::importa_json;
//! use opencomputo_io::report::{self, OpzioniReport};
//! use opencomputo_io::{csv, pdf, xlsx};
//!
//! # let json = include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json");
//! let progetto = importa_json(json).unwrap();
//! let id = progetto.computi[0].id;
//! let doc = report::computo(&progetto, id, &OpzioniReport::default()).unwrap();
//!
//! let file_pdf = pdf::documento(&doc, &pdf::OpzioniPdf::default());
//! let file_xlsx = xlsx::documento(&doc, &xlsx::OpzioniXlsx::default()).unwrap();
//! let tabelle_csv = csv::documento(&doc, &csv::OpzioniCsv::default());
//! assert!(file_pdf.byte.starts_with(b"%PDF-1.4"));
//! assert!(file_xlsx.starts_with(b"PK"));
//! assert!(tabelle_csv[0].1.contains("14457,12"));
//! ```

pub mod csv;
pub mod documento;
pub mod formato;
mod metriche;
pub mod pdf;
pub mod report;
pub mod report_analisi;
pub mod report_cronoprogramma;
pub mod tradizionale;
mod vedi_voce;
pub mod xlsx;
