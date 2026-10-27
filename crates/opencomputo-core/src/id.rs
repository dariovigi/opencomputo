//! Identificativi stabili delle entità.
//!
//! Ogni tipo di entità ha un proprio identificativo (UUID incapsulato in un
//! tipo distinto), così che non sia possibile, ad esempio, usare l'id di una
//! voce di prezzario dove è richiesto l'id di una voce di computo.
//!
//! Il core non genera identificativi casuali per impostazione predefinita:
//! li riceve dal chiamante tramite un [`GeneratoreId`]. Questo mantiene i
//! calcoli deterministici, i test riproducibili e il core indipendente dalla
//! sorgente di entropia della piattaforma (rilevante per WebAssembly).
//! La feature `id-casuali` abilita [`GeneratoreCasuale`] (UUID v4).

use core::fmt;
use core::hash::Hash;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Comportamento comune a tutti gli identificativi.
pub trait Identificativo: Copy + Eq + Ord + Hash + fmt::Debug + fmt::Display {
    /// Costruisce l'identificativo da un UUID.
    fn da_uuid(uuid: Uuid) -> Self;
    /// UUID sottostante.
    fn uuid(self) -> Uuid;
}

macro_rules! identificativo {
    ($(#[$meta:meta])* $nome:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $nome(Uuid);

        impl $nome {
            /// Identificativo deterministico costruito da un intero (utile per test e import).
            #[must_use]
            pub const fn da_u128(valore: u128) -> Self {
                Self(Uuid::from_u128(valore))
            }

            /// Nuovo identificativo casuale (UUID v4).
            #[cfg(feature = "id-casuali")]
            #[must_use]
            pub fn casuale() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Identificativo for $nome {
            fn da_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }
            fn uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $nome {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $nome {
            type Err = uuid::Error;
            fn from_str(testo: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(testo).map(Self)
            }
        }
    };
}

identificativo!(
    /// Identificativo di un progetto.
    ProgettoId
);
identificativo!(
    /// Identificativo di un prezzario o elenco prezzi.
    PrezzarioId
);
identificativo!(
    /// Identificativo di una voce di prezzario.
    VocePrezzarioId
);
identificativo!(
    /// Identificativo di un capitolo di prezzario.
    CapitoloId
);
identificativo!(
    /// Identificativo di una risorsa elementare (manodopera, materiale, nolo…).
    RisorsaId
);
identificativo!(
    /// Identificativo di un'analisi prezzi.
    AnalisiId
);
identificativo!(
    /// Identificativo di una classe di categoria (super categoria, categoria, sotto categoria).
    ClasseId
);
identificativo!(
    /// Identificativo di un nodo della WBS.
    WbsNodoId
);
identificativo!(
    /// Identificativo di un computo.
    ComputoId
);
identificativo!(
    /// Identificativo di una voce di computo.
    VoceComputoId
);
identificativo!(
    /// Identificativo di un rigo di misurazione.
    RigoMisuraId
);
identificativo!(
    /// Identificativo di una registrazione del libretto delle misure.
    RegistrazioneId
);
identificativo!(
    /// Identificativo di uno stato di avanzamento lavori.
    SalId
);
identificativo!(
    /// Identificativo di una penale.
    PenaleId
);
identificativo!(
    /// Identificativo di un'attività del cronoprogramma aggiunta ai nodi WBS (fase,
    /// attività libera o pietra miliare).
    AttivitaId
);

/// Sorgente di nuovi identificativi.
pub trait GeneratoreId {
    /// Prossimo UUID.
    fn prossimo_uuid(&mut self) -> Uuid;

    /// Nuovo identificativo del tipo richiesto.
    fn nuovo<T: Identificativo>(&mut self) -> T
    where
        Self: Sized,
    {
        T::da_uuid(self.prossimo_uuid())
    }
}

/// Generatore deterministico: produce UUID consecutivi a partire da un valore iniziale.
///
/// Adatto a test, import riproducibili e documenti generati automaticamente.
#[derive(Debug, Clone)]
pub struct GeneratoreSequenziale {
    prossimo: u128,
}

impl GeneratoreSequenziale {
    /// Nuovo generatore che parte da `inizio`.
    #[must_use]
    pub const fn new(inizio: u128) -> Self {
        Self { prossimo: inizio }
    }
}

impl Default for GeneratoreSequenziale {
    fn default() -> Self {
        Self::new(1)
    }
}

impl GeneratoreId for GeneratoreSequenziale {
    fn prossimo_uuid(&mut self) -> Uuid {
        let uuid = Uuid::from_u128(self.prossimo);
        self.prossimo = self.prossimo.wrapping_add(1);
        uuid
    }
}

/// Generatore di UUID v4 casuali.
#[cfg(feature = "id-casuali")]
#[derive(Debug, Clone, Copy, Default)]
pub struct GeneratoreCasuale;

#[cfg(feature = "id-casuali")]
impl GeneratoreId for GeneratoreCasuale {
    fn prossimo_uuid(&mut self) -> Uuid {
        Uuid::new_v4()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generatore_sequenziale_deterministico() {
        let mut g = GeneratoreSequenziale::new(10);
        let a: VoceComputoId = g.nuovo();
        let b: VoceComputoId = g.nuovo();
        assert_eq!(a, VoceComputoId::da_u128(10));
        assert_eq!(b, VoceComputoId::da_u128(11));
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_testuale_e_json() {
        let id = SalId::da_u128(0xABCD);
        let testo = id.to_string();
        assert_eq!(testo, "00000000-0000-0000-0000-00000000abcd");
        assert_eq!(testo.parse::<SalId>().unwrap(), id);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{testo}\""));
        assert_eq!(serde_json::from_str::<SalId>(&json).unwrap(), id);
    }
}
