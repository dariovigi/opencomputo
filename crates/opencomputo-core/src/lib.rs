//! # opencomputo-core
//!
//! Modello di dominio, validazioni e calcoli puri di OpenComputo: prezzari,
//! analisi prezzi, categorie e WBS, computi metrici estimativi, misurazioni
//! analitiche, libretto delle misure, registro di contabilità e SAL.
//!
//! Il crate non dipende da GUI, filesystem, rete o servizi della piattaforma
//! ed è progettato per poter essere compilato anche in WebAssembly.
//!
//! ## Principi
//!
//! * **Aritmetica decimale esatta** ([`numeri`]): nessun `f32`/`f64` nei
//!   valori contabili; arrotondamenti espliciti e configurabili.
//! * **Prezzi fotografati** ([`partita::PrezzoApplicato`]): le voci di computo e
//!   di contabilità conservano una copia dei dati di prezzo; aggiornare un
//!   prezzario non modifica retroattivamente i documenti.
//! * **Errori espliciti**: ogni calcolo restituisce `Result` con il percorso
//!   dell'entità responsabile ([`errore::ErroreLocalizzato`]).
//! * **Identificativi stabili** ([`id`]) per i collegamenti tra documenti.
//! * **Schema versionato** ([`schema`]) con migrazioni esplicite.

pub mod aggiornamento;
pub mod analisi;
pub mod classificazione;
pub mod computo;
pub mod contabilita;
pub mod cronoprogramma;
pub mod data;
pub mod errore;
pub mod fabbisogno;
pub mod formula;
pub mod id;
pub mod misura;
pub mod numeri;
pub mod partita;
pub mod prezzario;
pub mod progetto;
pub mod provenienza;
pub mod riferimento;
pub mod schema;
pub mod validazione;

pub use rust_decimal::Decimal;

/// Importazioni di uso comune.
pub mod prelude {
    pub use crate::Decimal;
    pub use crate::analisi::{
        AnalisiPrezzo, BaseUtile, OrigineRigaAnalisi, ParametriAnalisi, RigaAnalisi,
        RisorsaElementare, TipoRisorsa,
    };
    pub use crate::classificazione::{
        AssegnazioneCategorie, Categorie, Classe, LivelloCategoria, NodoWbs, StrutturaWbs,
    };
    pub use crate::computo::{Computo, RiepilogoComputo, TipoComputo, VoceComputo};
    pub use crate::contabilita::{
        Anticipazione, CertificatoPagamento, Collaudo, Contabilita, ContoFinale,
        ParametriContabili, Penale, RegistrazioneLibretto, RiepilogoContoFinale, RiepilogoSal,
        StatoSal, TipoCollaudo, VerbaleNuoviPrezzi,
    };
    pub use crate::errore::{ErroreCalcolo, ErroreDominio, ErroreLocalizzato};
    pub use crate::id::*;
    pub use crate::misura::{Fattore, RigoMisura, SegnoRigo, TipoRigo};
    pub use crate::numeri::{
        Importo, ModalitaArrotondamento, Percentuale, Prezzo, Quantita, RegoleArrotondamento,
    };
    pub use crate::partita::{PrezzoApplicato, calcola_partita};
    pub use crate::prezzario::{
        CriterioNuovoPrezzo, NaturaVoce, NuovoPrezzo, Prezzario, VocePrezzario,
    };
    pub use crate::progetto::{DatiContratto, MetadatiProgetto, Progetto};
    pub use crate::validazione::{Gravita, Problema, RapportoValidazione, valida_progetto};
}
