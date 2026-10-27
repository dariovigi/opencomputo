//! # opencomputo-format-listino
//!
//! Lettura di listini prezzi in **CSV** e **JSON** verso il [`Prezzario`] di
//! `opencomputo-core` (ADR 0025).
//!
//! * Le colonne del CSV si riconoscono dall'intestazione, con nomi alternativi
//!   configurabili ([`MappaturaColonne`]): il formato reale dei prezzari regionali non è
//!   fissato in anticipo.
//! * Separatore di campo (`;`, `,`, tabulazione) e separatore decimale si riconoscono dal
//!   contenuto, oppure si impongono con [`OpzioniListino`].
//! * Ogni riga scartata (campo obbligatorio vuoto, prezzo non valido, codice duplicato)
//!   produce un'[`Anomalia`] con il numero di riga: nessun dato viene perso in silenzio.
//!   Le colonne non riconosciute restano nei dati originali della voce.
//! * L'impronta SHA-256 del file identifica il listino nella provenienza.
//!
//! Il crate non usa filesystem né rete: riceve i byte del file.
//!
//! ```
//! use opencomputo_core::id::GeneratoreSequenziale;
//! use opencomputo_format_listino::{OpzioniListino, importa_csv};
//!
//! let csv = "Codice;Descrizione;U.M.;Prezzo\nA.1;Scavo di sbancamento;m³;8,50\n";
//! let r = importa_csv(csv.as_bytes(), &mut GeneratoreSequenziale::default(),
//!                     &OpzioniListino::default()).unwrap();
//! assert_eq!(r.prezzario.voci.len(), 1);
//! assert_eq!(r.prezzario.voci[0].prezzo.to_string(), "8.50");
//! ```
//!
//! [`Prezzario`]: opencomputo_core::prezzario::Prezzario
//! [`Anomalia`]: opencomputo_core::provenienza::Anomalia

pub mod colonne;
pub mod csv;
pub mod errore;
pub mod importa;
pub mod numeri;

pub use colonne::MappaturaColonne;
pub use errore::ErroreListino;
pub use importa::{
    FormatoListino, OpzioniListino, RapportoListino, RisultatoListino, importa, importa_csv,
    importa_json,
};
pub use numeri::SeparatoreDecimale;
