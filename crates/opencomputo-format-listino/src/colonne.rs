//! Riconoscimento delle colonne del CSV dall'intestazione.
//!
//! I nomi si confrontano dopo la normalizzazione: minuscole, senza accenti, senza spazi,
//! punti, trattini e sottolineature («U.M.» = «um», «Unità di misura» = «unitadimisura»).

use serde::{Deserialize, Serialize};

use crate::errore::ErroreListino;

/// Nomi alternativi di ciascun campo. Il primo nome dell'intestazione che corrisponde
/// a uno degli alternativi viene usato per quel campo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MappaturaColonne {
    /// Codice (tariffa). Obbligatorio.
    pub codice: Vec<String>,
    /// Descrizione estesa. Obbligatoria questa o la breve.
    pub descrizione: Vec<String>,
    /// Descrizione breve.
    pub descrizione_breve: Vec<String>,
    /// Unità di misura. Obbligatoria.
    pub unita_misura: Vec<String>,
    /// Prezzo unitario. Obbligatorio.
    pub prezzo: Vec<String>,
    /// Codice del capitolo.
    pub capitolo: Vec<String>,
    /// Descrizione del capitolo.
    pub descrizione_capitolo: Vec<String>,
    /// Natura (lavoro od oneri della sicurezza).
    pub natura: Vec<String>,
    /// Incidenza percentuale della manodopera.
    pub incidenza_manodopera: Vec<String>,
    /// Incidenza percentuale della sicurezza compresa nel prezzo.
    pub incidenza_sicurezza: Vec<String>,
}

fn nomi(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

impl Default for MappaturaColonne {
    fn default() -> Self {
        Self {
            codice: nomi(&[
                "codice",
                "tariffa",
                "codice voce",
                "cod",
                "articolo",
                "codice articolo",
            ]),
            descrizione: nomi(&[
                "descrizione",
                "descrizione estesa",
                "descrizione voce",
                "desc",
                "declaratoria",
            ]),
            descrizione_breve: nomi(&[
                "descrizione breve",
                "titolo",
                "sintesi",
                "descrizione sintetica",
            ]),
            unita_misura: nomi(&[
                "unità di misura",
                "unita misura",
                "u.m.",
                "udm",
                "unità",
                "unit",
            ]),
            prezzo: nomi(&[
                "prezzo",
                "prezzo unitario",
                "prezzo €",
                "euro",
                "prezzo euro",
                "importo",
            ]),
            capitolo: nomi(&["capitolo", "codice capitolo", "cap"]),
            descrizione_capitolo: nomi(&["descrizione capitolo", "titolo capitolo"]),
            natura: nomi(&["natura", "tipo voce"]),
            incidenza_manodopera: nomi(&[
                "incidenza manodopera",
                "manodopera",
                "% manodopera",
                "mdo",
                "incidenza mdo",
            ]),
            incidenza_sicurezza: nomi(&[
                "incidenza sicurezza",
                "sicurezza",
                "% sicurezza",
                "oneri sicurezza",
            ]),
        }
    }
}

/// Campi riconosciuti.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Campo {
    /// Codice.
    Codice,
    /// Descrizione estesa.
    Descrizione,
    /// Descrizione breve.
    DescrizioneBreve,
    /// Unità di misura.
    UnitaMisura,
    /// Prezzo.
    Prezzo,
    /// Codice del capitolo.
    Capitolo,
    /// Descrizione del capitolo.
    DescrizioneCapitolo,
    /// Natura.
    Natura,
    /// Incidenza della manodopera.
    IncidenzaManodopera,
    /// Incidenza della sicurezza.
    IncidenzaSicurezza,
}

impl Campo {
    /// Nome del campo nei messaggi.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Self::Codice => "codice",
            Self::Descrizione => "descrizione",
            Self::DescrizioneBreve => "descrizione breve",
            Self::UnitaMisura => "unità di misura",
            Self::Prezzo => "prezzo",
            Self::Capitolo => "capitolo",
            Self::DescrizioneCapitolo => "descrizione capitolo",
            Self::Natura => "natura",
            Self::IncidenzaManodopera => "incidenza manodopera",
            Self::IncidenzaSicurezza => "incidenza sicurezza",
        }
    }
}

/// Nome normalizzato per il confronto.
#[must_use]
pub fn normalizza(nome: &str) -> String {
    nome.trim_start_matches('\u{feff}')
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            'à' | 'á' | 'â' => Some('a'),
            'è' | 'é' | 'ê' => Some('e'),
            'ì' | 'í' | 'î' => Some('i'),
            'ò' | 'ó' | 'ô' => Some('o'),
            'ù' | 'ú' | 'û' => Some('u'),
            c if c.is_alphanumeric() || c == '%' || c == '€' => Some(c),
            _ => None,
        })
        .collect()
}

/// Posizione delle colonne riconosciute nell'intestazione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colonne {
    /// Campo → indice della colonna.
    pub posizioni: Vec<(Campo, usize)>,
    /// Colonne non riconosciute: indice e nome originale.
    pub altre: Vec<(usize, String)>,
}

impl Colonne {
    /// Indice della colonna del campo.
    #[must_use]
    pub fn indice(&self, campo: Campo) -> Option<usize> {
        self.posizioni
            .iter()
            .find(|(c, _)| *c == campo)
            .map(|(_, i)| *i)
    }
}

impl MappaturaColonne {
    fn alternative(&self) -> [(Campo, &[String]); 10] {
        [
            (Campo::Codice, &self.codice),
            (Campo::Descrizione, &self.descrizione),
            (Campo::DescrizioneBreve, &self.descrizione_breve),
            (Campo::UnitaMisura, &self.unita_misura),
            (Campo::Prezzo, &self.prezzo),
            (Campo::Capitolo, &self.capitolo),
            (Campo::DescrizioneCapitolo, &self.descrizione_capitolo),
            (Campo::Natura, &self.natura),
            (Campo::IncidenzaManodopera, &self.incidenza_manodopera),
            (Campo::IncidenzaSicurezza, &self.incidenza_sicurezza),
        ]
    }

    /// Riconosce le colonne dell'intestazione.
    ///
    /// # Errors
    /// Colonne obbligatorie mancanti (codice, una descrizione, unità di misura, prezzo)
    /// o una colonna che corrisponde a due campi diversi.
    pub fn riconosci(&self, intestazione: &[String]) -> Result<Colonne, ErroreListino> {
        let mut posizioni: Vec<(Campo, usize)> = Vec::new();
        let mut altre = Vec::new();
        for (i, nome) in intestazione.iter().enumerate() {
            let n = normalizza(nome);
            let campo = self
                .alternative()
                .into_iter()
                .find(|(_, alt)| alt.iter().any(|a| normalizza(a) == n))
                .map(|(c, _)| c);
            match campo {
                Some(c) if posizioni.iter().any(|(x, _)| *x == c) => {
                    return Err(ErroreListino::ColonnaAmbigua {
                        colonna: nome.trim().to_owned(),
                    });
                }
                Some(c) => posizioni.push((c, i)),
                None => altre.push((i, nome.trim().to_owned())),
            }
        }
        let presente = |c: Campo| posizioni.iter().any(|(x, _)| *x == c);
        let mut mancanti = Vec::new();
        for c in [Campo::Codice, Campo::UnitaMisura, Campo::Prezzo] {
            if !presente(c) {
                mancanti.push(c.nome().to_owned());
            }
        }
        if !presente(Campo::Descrizione) && !presente(Campo::DescrizioneBreve) {
            mancanti.push("descrizione (o descrizione breve)".to_owned());
        }
        if !mancanti.is_empty() {
            return Err(ErroreListino::ColonneMancanti {
                mancanti,
                trovate: intestazione.iter().map(|s| s.trim().to_owned()).collect(),
            });
        }
        Ok(Colonne { posizioni, altre })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn intestazione(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn nomi_alternativi_e_normalizzazione() {
        let c = MappaturaColonne::default()
            .riconosci(&intestazione(&[
                "\u{feff}Tariffa",
                "Descrizione",
                "U.M.",
                "Prezzo €",
                "Note",
            ]))
            .unwrap();
        assert_eq!(c.indice(Campo::Codice), Some(0));
        assert_eq!(c.indice(Campo::UnitaMisura), Some(2));
        assert_eq!(c.indice(Campo::Prezzo), Some(3));
        assert_eq!(c.altre, vec![(4, "Note".to_owned())]);
        assert_eq!(normalizza("Unità di misura"), "unitadimisura");
    }

    #[test]
    fn colonne_mancanti() {
        let e = MappaturaColonne::default()
            .riconosci(&intestazione(&["Codice", "Prezzo"]))
            .unwrap_err();
        let ErroreListino::ColonneMancanti { mancanti, .. } = e else {
            panic!("errore inatteso");
        };
        assert_eq!(
            mancanti,
            vec!["unità di misura", "descrizione (o descrizione breve)"]
        );
    }

    #[test]
    fn colonna_ripetuta() {
        let e = MappaturaColonne::default()
            .riconosci(&intestazione(&[
                "Codice",
                "Tariffa",
                "Descrizione",
                "UM",
                "Prezzo",
            ]))
            .unwrap_err();
        assert!(matches!(e, ErroreListino::ColonnaAmbigua { .. }));
    }
}
