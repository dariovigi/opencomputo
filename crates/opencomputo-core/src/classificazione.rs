//! Categorie e struttura WBS.
//!
//! Sono previsti due sistemi di classificazione indipendenti e combinabili:
//!
//! * [`Categorie`]: tre elenchi piatti (super categorie, categorie, sotto
//!   categorie). Ogni voce può essere assegnata a un elemento di ciascun
//!   livello in modo indipendente. Questa organizzazione corrisponde a quella
//!   diffusa nei software di computo italiani e nei relativi formati di
//!   interscambio (da verificare sui formati nella Fase 2).
//! * [`StrutturaWbs`]: albero gerarchico di profondità arbitraria (Work
//!   Breakdown Structure), con totali cumulati sui nodi antenati.

use core::fmt;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::errore::ErroreDominio;
use crate::id::{ClasseId, WbsNodoId};

/// Livello di categoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LivelloCategoria {
    /// Super categoria.
    SuperCategoria,
    /// Categoria.
    Categoria,
    /// Sotto categoria.
    SottoCategoria,
}

impl LivelloCategoria {
    /// Tutti i livelli, dal più generale al più specifico.
    pub const TUTTI: [Self; 3] = [Self::SuperCategoria, Self::Categoria, Self::SottoCategoria];
}

impl fmt::Display for LivelloCategoria {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SuperCategoria => "super categoria",
            Self::Categoria => "categoria",
            Self::SottoCategoria => "sotto categoria",
        })
    }
}

/// Elemento di un elenco di categorie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Classe {
    /// Identificativo.
    pub id: ClasseId,
    /// Codice.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
}

/// Elenchi delle categorie di un progetto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Categorie {
    /// Super categorie.
    #[serde(default)]
    pub super_categorie: Vec<Classe>,
    /// Categorie.
    #[serde(default)]
    pub categorie: Vec<Classe>,
    /// Sotto categorie.
    #[serde(default)]
    pub sotto_categorie: Vec<Classe>,
}

impl Categorie {
    /// Elenco di un livello.
    #[must_use]
    pub fn elenco(&self, livello: LivelloCategoria) -> &[Classe] {
        match livello {
            LivelloCategoria::SuperCategoria => &self.super_categorie,
            LivelloCategoria::Categoria => &self.categorie,
            LivelloCategoria::SottoCategoria => &self.sotto_categorie,
        }
    }

    fn elenco_mut(&mut self, livello: LivelloCategoria) -> &mut Vec<Classe> {
        match livello {
            LivelloCategoria::SuperCategoria => &mut self.super_categorie,
            LivelloCategoria::Categoria => &mut self.categorie,
            LivelloCategoria::SottoCategoria => &mut self.sotto_categorie,
        }
    }

    /// Classe di un livello per identificativo.
    #[must_use]
    pub fn trova(&self, livello: LivelloCategoria, id: ClasseId) -> Option<&Classe> {
        self.elenco(livello).iter().find(|c| c.id == id)
    }

    /// Aggiunge una classe al livello indicato.
    ///
    /// # Errors
    /// Identificativo o codice già presenti nel livello.
    pub fn aggiungi(
        &mut self,
        livello: LivelloCategoria,
        classe: Classe,
    ) -> Result<(), ErroreDominio> {
        let elenco = self.elenco_mut(livello);
        if elenco.iter().any(|c| c.id == classe.id) {
            return Err(ErroreDominio::IdDuplicato {
                entita: "categoria",
                id: classe.id.to_string(),
            });
        }
        if elenco.iter().any(|c| c.codice == classe.codice) {
            return Err(ErroreDominio::CodiceDuplicato {
                entita: "categoria",
                codice: classe.codice,
            });
        }
        elenco.push(classe);
        Ok(())
    }
}

/// Assegnazione di una voce alle categorie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssegnazioneCategorie {
    /// Super categoria.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub super_categoria: Option<ClasseId>,
    /// Categoria.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categoria: Option<ClasseId>,
    /// Sotto categoria.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sotto_categoria: Option<ClasseId>,
}

impl AssegnazioneCategorie {
    /// Classe assegnata per il livello indicato.
    #[must_use]
    pub fn di(&self, livello: LivelloCategoria) -> Option<ClasseId> {
        match livello {
            LivelloCategoria::SuperCategoria => self.super_categoria,
            LivelloCategoria::Categoria => self.categoria,
            LivelloCategoria::SottoCategoria => self.sotto_categoria,
        }
    }
}

/// Nodo della WBS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodoWbs {
    /// Identificativo.
    pub id: WbsNodoId,
    /// Nodo padre (`None` per i nodi radice).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padre: Option<WbsNodoId>,
    /// Codice locale (es. `02`); il codice completo si ottiene risalendo l'albero.
    pub codice: String,
    /// Descrizione.
    pub descrizione: String,
}

/// Struttura WBS: albero di nodi. L'ordine del vettore è l'ordine di presentazione.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrutturaWbs {
    /// Nodi.
    #[serde(default)]
    pub nodi: Vec<NodoWbs>,
}

impl StrutturaWbs {
    /// Nodo per identificativo.
    #[must_use]
    pub fn nodo(&self, id: WbsNodoId) -> Option<&NodoWbs> {
        self.nodi.iter().find(|n| n.id == id)
    }

    /// Figli di un nodo (`None` = nodi radice), nell'ordine di presentazione.
    #[must_use]
    pub fn figli(&self, padre: Option<WbsNodoId>) -> Vec<&NodoWbs> {
        self.nodi.iter().filter(|n| n.padre == padre).collect()
    }

    /// Aggiunge un nodo.
    ///
    /// # Errors
    /// Identificativo duplicato, padre inesistente o codice duplicato tra fratelli.
    pub fn aggiungi(&mut self, nodo: NodoWbs) -> Result<(), ErroreDominio> {
        if self.nodo(nodo.id).is_some() {
            return Err(ErroreDominio::IdDuplicato {
                entita: "nodo WBS",
                id: nodo.id.to_string(),
            });
        }
        if let Some(p) = nodo.padre
            && self.nodo(p).is_none()
        {
            return Err(ErroreDominio::RiferimentoNonValido(format!(
                "il nodo WBS padre {p} non esiste"
            )));
        }
        if self
            .nodi
            .iter()
            .any(|n| n.padre == nodo.padre && n.codice == nodo.codice)
        {
            return Err(ErroreDominio::CodiceDuplicato {
                entita: "nodo WBS",
                codice: nodo.codice,
            });
        }
        self.nodi.push(nodo);
        Ok(())
    }

    /// Percorso dalla radice al nodo indicato (incluso).
    ///
    /// # Errors
    /// Nodo o antenato inesistente, oppure ciclo nella struttura.
    pub fn percorso(&self, id: WbsNodoId) -> Result<Vec<WbsNodoId>, ErroreDominio> {
        let mut percorso = Vec::new();
        let mut corrente = Some(id);
        while let Some(c) = corrente {
            if percorso.len() > self.nodi.len() || percorso.contains(&c) {
                return Err(ErroreDominio::RiferimentoNonValido(format!(
                    "ciclo nella struttura WBS in corrispondenza del nodo {c}"
                )));
            }
            let nodo = self.nodo(c).ok_or_else(|| ErroreDominio::NonTrovato {
                entita: "nodo WBS",
                id: c.to_string(),
            })?;
            percorso.push(c);
            corrente = nodo.padre;
        }
        percorso.reverse();
        Ok(percorso)
    }

    /// Codice completo del nodo (codici del percorso uniti da `.`).
    ///
    /// # Errors
    /// Come [`Self::percorso`].
    pub fn codice_completo(&self, id: WbsNodoId) -> Result<String, ErroreDominio> {
        let codici: Vec<&str> = self
            .percorso(id)?
            .into_iter()
            .filter_map(|n| self.nodo(n).map(|x| x.codice.as_str()))
            .collect();
        Ok(codici.join("."))
    }

    /// Verifica la coerenza della struttura: id univoci, padri esistenti, assenza di cicli.
    ///
    /// # Errors
    /// Il primo problema rilevato.
    pub fn verifica(&self) -> Result<(), ErroreDominio> {
        let mut visti = BTreeMap::new();
        for n in &self.nodi {
            if visti.insert(n.id, ()).is_some() {
                return Err(ErroreDominio::IdDuplicato {
                    entita: "nodo WBS",
                    id: n.id.to_string(),
                });
            }
        }
        for n in &self.nodi {
            self.percorso(n.id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodo(id: u128, padre: Option<u128>, codice: &str) -> NodoWbs {
        NodoWbs {
            id: WbsNodoId::da_u128(id),
            padre: padre.map(WbsNodoId::da_u128),
            codice: codice.into(),
            descrizione: codice.into(),
        }
    }

    fn wbs() -> StrutturaWbs {
        let mut w = StrutturaWbs::default();
        w.aggiungi(nodo(1, None, "01")).unwrap();
        w.aggiungi(nodo(2, Some(1), "01")).unwrap();
        w.aggiungi(nodo(3, Some(2), "03")).unwrap();
        w.aggiungi(nodo(4, None, "02")).unwrap();
        w
    }

    #[test]
    fn percorso_e_codice_completo() {
        let w = wbs();
        assert_eq!(
            w.percorso(WbsNodoId::da_u128(3)).unwrap(),
            vec![
                WbsNodoId::da_u128(1),
                WbsNodoId::da_u128(2),
                WbsNodoId::da_u128(3)
            ]
        );
        assert_eq!(
            w.codice_completo(WbsNodoId::da_u128(3)).unwrap(),
            "01.01.03"
        );
        assert_eq!(w.figli(None).len(), 2);
        assert!(w.verifica().is_ok());
    }

    #[test]
    fn vincoli_di_inserimento() {
        let mut w = wbs();
        assert!(matches!(
            w.aggiungi(nodo(9, Some(99), "x")),
            Err(ErroreDominio::RiferimentoNonValido(_))
        ));
        assert!(matches!(
            w.aggiungi(nodo(9, None, "01")),
            Err(ErroreDominio::CodiceDuplicato { .. })
        ));
        assert!(matches!(
            w.aggiungi(nodo(1, None, "zz")),
            Err(ErroreDominio::IdDuplicato { .. })
        ));
    }

    #[test]
    fn ciclo_rilevato() {
        // Dati incoerenti possono arrivare da un file: la verifica li rileva.
        let w = StrutturaWbs {
            nodi: vec![nodo(1, Some(2), "a"), nodo(2, Some(1), "b")],
        };
        assert!(matches!(
            w.verifica(),
            Err(ErroreDominio::RiferimentoNonValido(_))
        ));
    }

    #[test]
    fn categorie_per_livello() {
        let mut c = Categorie::default();
        let classe = Classe {
            id: ClasseId::da_u128(1),
            codice: "OS".into(),
            descrizione: "Opere strutturali".into(),
        };
        c.aggiungi(LivelloCategoria::Categoria, classe.clone())
            .unwrap();
        // Lo stesso codice è ammesso in un livello diverso.
        c.aggiungi(LivelloCategoria::SuperCategoria, classe.clone())
            .unwrap();
        assert!(c.aggiungi(LivelloCategoria::Categoria, classe).is_err());
        assert!(
            c.trova(LivelloCategoria::Categoria, ClasseId::da_u128(1))
                .is_some()
        );
        assert!(
            c.trova(LivelloCategoria::SottoCategoria, ClasseId::da_u128(1))
                .is_none()
        );
    }
}
