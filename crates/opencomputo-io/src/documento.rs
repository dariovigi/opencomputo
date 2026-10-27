//! Modello intermedio dei report.
//!
//! I report (computo, elenco prezzi, libretto, registro, SAL) sono costruiti una
//! volta sola come [`Documento`]: sezioni con tabelle di righe tipizzate. Gli
//! scrittori PDF, XLSX e CSV rappresentano lo stesso documento, così i numeri di
//! ogni formato coincidono per costruzione e i report si verificano sul modello,
//! senza dover leggere i file prodotti.

use rust_decimal::Decimal;

use crate::formato;

/// Allineamento di una colonna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allineamento {
    /// A sinistra.
    Sinistra,
    /// Al centro.
    Centro,
    /// A destra.
    Destra,
}

/// Colonna di una tabella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colonna {
    /// Intestazione.
    pub titolo: String,
    /// Larghezza relativa (le colonne si dividono lo spazio in proporzione).
    pub peso: u16,
    /// Allineamento.
    pub allineamento: Allineamento,
}

impl Colonna {
    /// Colonna di testo allineata a sinistra.
    #[must_use]
    pub fn testo(titolo: &str, peso: u16) -> Self {
        Self {
            titolo: titolo.to_owned(),
            peso,
            allineamento: Allineamento::Sinistra,
        }
    }

    /// Colonna numerica allineata a destra.
    #[must_use]
    pub fn numero(titolo: &str, peso: u16) -> Self {
        Self {
            titolo: titolo.to_owned(),
            peso,
            allineamento: Allineamento::Destra,
        }
    }

    /// Colonna centrata.
    #[must_use]
    pub fn centrata(titolo: &str, peso: u16) -> Self {
        Self {
            titolo: titolo.to_owned(),
            peso,
            allineamento: Allineamento::Centro,
        }
    }
}

/// Contenuto di una cella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Valore {
    /// Cella vuota.
    Vuoto,
    /// Testo.
    Testo(String),
    /// Numero con i decimali minimi da mostrare.
    Numero {
        /// Valore esatto.
        valore: Decimal,
        /// Decimali minimi (la scala viene estesa, mai ridotta).
        decimali: u32,
    },
}

impl Valore {
    /// Testo.
    #[must_use]
    pub fn testo(t: impl Into<String>) -> Self {
        Self::Testo(t.into())
    }

    /// Numero.
    #[must_use]
    pub fn numero(valore: Decimal, decimali: u32) -> Self {
        Self::Numero { valore, decimali }
    }

    /// Numero facoltativo (vuoto se assente).
    #[must_use]
    pub fn numero_opz(valore: Option<Decimal>, decimali: u32) -> Self {
        valore.map_or(Self::Vuoto, |v| Self::numero(v, decimali))
    }

    /// Rappresentazione per la stampa (numeri all'italiana).
    #[must_use]
    pub fn in_testo(&self) -> String {
        match self {
            Self::Vuoto => String::new(),
            Self::Testo(t) => t.clone(),
            Self::Numero { valore, decimali } => formato::numero(*valore, *decimali),
        }
    }

    /// `true` se la cella è vuota.
    #[must_use]
    pub fn is_vuoto(&self) -> bool {
        match self {
            Self::Vuoto => true,
            Self::Testo(t) => t.is_empty(),
            Self::Numero { .. } => false,
        }
    }
}

/// Ruolo di una riga, che ne determina la resa grafica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StileRiga {
    /// Riga ordinaria.
    Normale,
    /// Intestazione di una voce (numero, codice, descrizione).
    Voce,
    /// Rigo di misura o dettaglio.
    Rigo,
    /// Commento (rigo descrittivo senza quantità).
    Commento,
    /// Riga «sommano» di chiusura di una voce.
    Sommano,
    /// Titolo di gruppo (capitolo, SAL…).
    Gruppo,
    /// Totale.
    Totale,
}

impl StileRiga {
    /// Codice stabile, usato nella colonna «Tipo» dei CSV.
    #[must_use]
    pub fn codice(self) -> &'static str {
        match self {
            Self::Normale => "riga",
            Self::Voce => "voce",
            Self::Rigo => "rigo",
            Self::Commento => "commento",
            Self::Sommano => "sommano",
            Self::Gruppo => "gruppo",
            Self::Totale => "totale",
        }
    }
}

/// Riga di una tabella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Riga {
    /// Stile.
    pub stile: StileRiga,
    /// Celle, una per colonna.
    pub celle: Vec<Valore>,
}

/// Tabella.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tabella {
    /// Colonne.
    pub colonne: Vec<Colonna>,
    /// Righe.
    pub righe: Vec<Riga>,
}

impl Tabella {
    /// Nuova tabella con le colonne indicate.
    #[must_use]
    pub fn new(colonne: Vec<Colonna>) -> Self {
        Self {
            colonne,
            righe: Vec::new(),
        }
    }

    /// Aggiunge una riga, completando con celle vuote o troncando alle colonne.
    pub fn riga(&mut self, stile: StileRiga, mut celle: Vec<Valore>) {
        celle.resize(self.colonne.len(), Valore::Vuoto);
        self.righe.push(Riga { stile, celle });
    }

    /// Riga con un solo testo nella colonna indicata.
    pub fn riga_testo(&mut self, stile: StileRiga, colonna: usize, testo: impl Into<String>) {
        let mut celle = vec![Valore::Vuoto; self.colonne.len()];
        if let Some(c) = celle.get_mut(colonna) {
            *c = Valore::testo(testo);
        }
        self.righe.push(Riga { stile, celle });
    }
}

/// Sezione di un documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sezione {
    /// Titolo (anche nome del foglio XLSX e del file CSV).
    pub titolo: String,
    /// Note sotto il titolo.
    pub note: Vec<String>,
    /// Tabella.
    pub tabella: Tabella,
}

/// Documento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Documento {
    /// Titolo (es. «Computo metrico estimativo»).
    pub titolo: String,
    /// Dati di intestazione (etichetta, valore).
    pub intestazione: Vec<(String, String)>,
    /// Sezioni.
    pub sezioni: Vec<Sezione>,
    /// Pagina orizzontale nel PDF e nella stampa XLSX.
    pub orizzontale: bool,
    /// Nota a piè di pagina (es. data di stampa).
    pub piede: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn righe_completate_alle_colonne() {
        let mut t = Tabella::new(vec![Colonna::testo("A", 1), Colonna::numero("B", 1)]);
        t.riga(StileRiga::Normale, vec![Valore::testo("x")]);
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::Vuoto,
                Valore::numero(dec!(1), 2),
                Valore::testo("in più"),
            ],
        );
        t.riga_testo(StileRiga::Gruppo, 1, "titolo");
        assert!(t.righe.iter().all(|r| r.celle.len() == 2));
        assert_eq!(t.righe[1].celle[1].in_testo(), "1,00");
        assert_eq!(t.righe[2].celle[1], Valore::testo("titolo"));
        assert!(t.righe[0].celle[1].is_vuoto());
    }
}
