//! Conservazione dei dati originali non mappati sul modello.
//!
//! Per ogni elemento XPWE importato, la provenienza dell'entità corrispondente
//! conserva, sotto un prefisso:
//!
//! * `_ordine`: l'ordine dei figli nel file (chiavi separate da `|`; le ripetizioni
//!   dello stesso nome sono indicate come `Nome#2`, `Nome#3`…);
//! * `<chiave>`: il frammento XML di ogni figlio **non** mappato sul modello;
//! * `=<Nome>`: il testo originale di alcuni campi mappati, quando serve all'export
//!   per ricostruire valori che il modello non rappresenta (es. bit sconosciuti di
//!   `Flags`, riferimenti a capitoli);
//! * `@<attributo>`: gli attributi dell'elemento diversi da `ID`;
//! * `_nome`: il nome dell'elemento, se diverso da quello predefinito.
//!
//! In export l'ordine originale viene ripercorso: i campi mappati sono scritti dal
//! modello, quelli conservati sono reinseriti così com'erano.

use std::collections::{BTreeMap, BTreeSet};

use crate::xml::{Nodo, Scrittore};

/// Separatore delle chiavi in `_ordine`.
const SEPARATORE: char = '|';

/// Dati originali (chiave → valore), come in `Provenienza::dati_originali`.
pub type Dati = BTreeMap<String, String>;

/// Chiave di ogni figlio: il nome, con `#n` dalla seconda ripetizione.
fn chiavi_figli(nodo: &Nodo) -> Vec<(String, &Nodo)> {
    let mut conteggi: BTreeMap<&str, usize> = BTreeMap::new();
    nodo.figli
        .iter()
        .map(|f| {
            let n = conteggi.entry(f.nome.as_str()).or_insert(0);
            *n += 1;
            let chiave = if *n == 1 {
                f.nome.clone()
            } else {
                format!("{}#{n}", f.nome)
            };
            (chiave, f)
        })
        .collect()
}

/// Nome di base di una chiave (`Nome#2` → `Nome`).
#[must_use]
pub fn nome_base(chiave: &str) -> &str {
    chiave.split_once('#').map_or(chiave, |(n, _)| n)
}

/// Esito della registrazione: nomi dei figli né mappati né noti.
#[derive(Debug, Default)]
pub struct Registrazione {
    /// Figli sconosciuti (nome, riga).
    pub sconosciuti: Vec<(String, usize)>,
}

/// Regole di registrazione di un elemento.
#[derive(Debug, Clone, Copy)]
pub struct Regole<'a> {
    /// Figli mappati sul modello (scritti dal modello in export).
    pub mappati: &'a [&'a str],
    /// Figli noti ma non mappati (conservati senza segnalarli come sconosciuti).
    pub noti: &'a [&'a str],
    /// Campi mappati di cui conservare anche il testo originale.
    pub testi: &'a [&'a str],
    /// Campi mappati che possono ripetersi (elementi di un elenco): tutte le
    /// occorrenze sono mappate.
    pub ripetibili: &'a [&'a str],
}

/// Registra ordine, figli non mappati, testi richiesti e attributi di `nodo`.
pub fn registra(dati: &mut Dati, prefisso: &str, nodo: &Nodo, regole: Regole<'_>) -> Registrazione {
    let mut esito = Registrazione::default();
    let mut ordine = Vec::new();
    for (chiave, figlio) in chiavi_figli(nodo) {
        let base = nome_base(&chiave);
        let mappato =
            regole.mappati.contains(&base) && (chiave == base || regole.ripetibili.contains(&base));
        if mappato {
            if regole.ripetibili.contains(&base) && chiave != base {
                continue;
            }
            if regole.testi.contains(&base) {
                dati.insert(format!("{prefisso}={base}"), figlio.testo.clone());
            }
        } else if regole.mappati.contains(&base) {
            // Ripetizione di un campo mappato: conservata, il modello ne usa la prima.
            dati.insert(format!("{prefisso}{chiave}"), figlio.in_xml());
            esito.sconosciuti.push((chiave.clone(), figlio.riga));
        } else {
            dati.insert(format!("{prefisso}{chiave}"), figlio.in_xml());
            if !regole.noti.contains(&base) {
                esito.sconosciuti.push((chiave.clone(), figlio.riga));
            }
        }
        ordine.push(chiave);
    }
    dati.insert(
        format!("{prefisso}_ordine"),
        ordine.join(&SEPARATORE.to_string()),
    );
    for (k, v) in &nodo.attributi {
        if k != "ID" {
            dati.insert(format!("{prefisso}@{k}"), v.clone());
        }
    }
    esito
}

/// Registra solo gli attributi e i figli diversi da `contenuto` (contenitori di elenchi).
pub fn registra_contenitore(
    dati: &mut Dati,
    prefisso: &str,
    nodo: &Nodo,
    contenuto: &str,
) -> Registrazione {
    registra(
        dati,
        prefisso,
        nodo,
        Regole {
            mappati: &[contenuto],
            noti: &[],
            testi: &[],
            ripetibili: &[contenuto],
        },
    )
}

/// Testo originale di un campo mappato (`=<Nome>`).
#[must_use]
pub fn testo<'a>(dati: Option<&'a Dati>, prefisso: &str, nome: &str) -> Option<&'a str> {
    dati?.get(&format!("{prefisso}={nome}")).map(String::as_str)
}

/// Valore conservato con chiave esatta.
#[must_use]
pub fn valore<'a>(dati: Option<&'a Dati>, chiave: &str) -> Option<&'a str> {
    dati?.get(chiave).map(String::as_str)
}

/// Attributi conservati (diversi da `ID`).
#[must_use]
pub fn attributi(dati: Option<&Dati>, prefisso: &str) -> Vec<(String, String)> {
    let Some(d) = dati else { return Vec::new() };
    let iniziale = format!("{prefisso}@");
    d.range(iniziale.clone()..)
        .take_while(|(k, _)| k.starts_with(&iniziale))
        .filter_map(|(k, v)| {
            k.strip_prefix(&iniziale)
                .map(|nome| (nome.to_owned(), v.clone()))
        })
        .collect()
}

/// Scrive i figli di un elemento ripercorrendo l'ordine originale.
///
/// `scrivi_mappato(s, nome)` scrive un campo mappato e restituisce `true`, oppure
/// `false` se `nome` non è mappato. Ogni nome mappato è scritto una sola volta.
/// I campi di `obbligatori` assenti dall'ordine originale sono aggiunti in coda.
pub fn scrivi_figli(
    s: &mut Scrittore,
    dati: Option<&Dati>,
    prefisso: &str,
    predefinito: &[&str],
    obbligatori: &[&str],
    mut scrivi_mappato: impl FnMut(&mut Scrittore, &str) -> bool,
) {
    let ordine: Vec<String> = match valore(dati, &format!("{prefisso}_ordine")) {
        Some(o) if !o.is_empty() => o.split(SEPARATORE).map(str::to_owned).collect(),
        Some(_) => Vec::new(),
        None => predefinito.iter().map(|s| (*s).to_owned()).collect(),
    };
    let mut scritti: BTreeSet<String> = BTreeSet::new();
    for chiave in &ordine {
        let base = nome_base(chiave);
        if chiave == base && !scritti.contains(base) && scrivi_mappato(s, base) {
            scritti.insert(base.to_owned());
            continue;
        }
        if let Some(xml) = valore(dati, &format!("{prefisso}{chiave}")) {
            s.grezzo(xml);
        }
    }
    for nome in obbligatori {
        if !scritti.contains(*nome) && !ordine.iter().any(|k| k == nome) {
            scrivi_mappato(s, nome);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::analizza;

    #[test]
    fn ordine_e_figli_conservati() {
        let d =
            analizza(b"<V ID=\"3\" x=\"1\"><A>1</A><Z>z</Z><B>2</B><Z>w</Z><N>n</N></V>").unwrap();
        let mut dati = Dati::new();
        let r = registra(
            &mut dati,
            "p/",
            &d.radice,
            Regole {
                mappati: &["A", "B"],
                noti: &["N"],
                testi: &["B"],
                ripetibili: &[],
            },
        );
        assert_eq!(dati.get("p/_ordine").unwrap(), "A|Z|B|Z#2|N");
        assert_eq!(dati.get("p/Z").unwrap(), "<Z>z</Z>");
        assert_eq!(dati.get("p/Z#2").unwrap(), "<Z>w</Z>");
        assert_eq!(dati.get("p/=B").unwrap(), "2");
        assert!(!dati.contains_key("p/A"));
        assert_eq!(attributi(Some(&dati), "p/"), vec![("x".into(), "1".into())]);
        let nomi: Vec<_> = r.sconosciuti.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nomi, ["Z", "Z#2"]);

        let mut s = Scrittore::default();
        scrivi_figli(
            &mut s,
            Some(&dati),
            "p/",
            &[],
            &["C"],
            |s, nome| match nome {
                "A" | "B" | "C" => {
                    s.campo(nome, "m");
                    true
                }
                _ => false,
            },
        );
        assert_eq!(
            s.fine(),
            "<A>m</A>\r\n<Z>z</Z>\r\n<B>m</B>\r\n<Z>w</Z>\r\n<N>n</N>\r\n<C>m</C>\r\n"
        );
    }
}
