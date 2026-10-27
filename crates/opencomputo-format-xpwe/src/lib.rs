//! # opencomputo-format-xpwe
//!
//! Import ed export del formato **XPWE**, il formato XML di interscambio
//! dell'ecosistema PriMus, per il modello di [`opencomputo_core`].
//!
//! ## Stato
//!
//! Non esiste una specifica pubblica di XPWE: la struttura usata è **dedotta** da
//! fonti secondarie e documentata in [`struttura`] (vedi
//! `docs/adr/0006-formati-primus.md`). Di conseguenza:
//!
//! * l'**import** è tollerante: conserva tutto ciò che non interpreta, segnala ogni
//!   anomalia con la posizione nel file e confronta le quantità dichiarate con
//!   quelle ricalcolate;
//! * l'**export** è **sperimentale**: l'andata e ritorno con questo crate è
//!   verificata dai test, l'apertura in PriMus no.
//!
//! Il crate non usa filesystem né rete: riceve e restituisce byte e testo, ed è
//! compilabile in WebAssembly.
//!
//! ## Uso
//!
//! ```
//! use opencomputo_core::id::GeneratoreSequenziale;
//! use opencomputo_format_xpwe::{OpzioniImport, esporta, importa};
//!
//! let xpwe = br#"<PweDocumento>
//!   <PweDatiGenerali/>
//!   <PweMisurazioni>
//!     <PweElencoPrezzi>
//!       <EPItem ID="1"><Tariffa>A.1</Tariffa><DesBreve>Scavo</DesBreve>
//!         <DesEstesa>Scavo</DesEstesa><UnMisura>m3</UnMisura><Prezzo1>8.50</Prezzo1></EPItem>
//!     </PweElencoPrezzi>
//!     <PweVociComputo>
//!       <VCItem ID="1"><IDEP>1</IDEP><Quantita>10.00</Quantita>
//!         <PweVCMisure><RGItem ID="1"><Descrizione>Platea</Descrizione>
//!           <Lunghezza>5</Lunghezza><Larghezza>2</Larghezza><Quantita>10.00</Quantita></RGItem>
//!         </PweVCMisure></VCItem>
//!     </PweVociComputo>
//!   </PweMisurazioni>
//! </PweDocumento>"#;
//!
//! let mut generatore = GeneratoreSequenziale::default();
//! let r = importa(xpwe, &mut generatore, &OpzioniImport::default())?;
//! assert!(!r.rapporto.ha_errori());
//! let totale = r.progetto.calcola_computo(r.computo).map(|c| c.totale.to_string());
//! assert_eq!(totale.as_deref(), Ok("85.00"));
//!
//! let export = esporta(&r.progetto, r.computo)?;
//! assert!(export.xml.contains("<Tariffa>A.1</Tariffa>"));
//! # Ok::<(), opencomputo_format_xpwe::ErroreXpwe>(())
//! ```

pub mod conservati;
pub mod errore;
pub mod esporta;
pub mod importa;
pub mod struttura;
pub mod testo;
pub mod xml;

pub use errore::ErroreXpwe;
pub use esporta::{RisultatoExport, esporta};
pub use importa::{OpzioniImport, RapportoImport, RisultatoImport, Statistiche, importa};
