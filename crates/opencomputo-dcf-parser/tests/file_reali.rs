//! Verifica su DCF reali, che **non** sono nel repository.
//!
//! Eseguire con:
//! `OPENCOMPUTO_DCF_REALI=/percorso/cartella cargo test -p opencomputo-dcf-parser --release --test file_reali -- --ignored --nocapture`
//!
//! Il test controlla coerenze interne (nessuna voce persa, riferimenti risolti,
//! documenti protetti rifiutati) senza contenere dati dei file.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_dcf_parser::contenitore::{self, Contenuto};
use opencomputo_dcf_parser::{ErroreDcf, OpzioniImport, importa};

#[test]
#[ignore = "richiede DCF reali indicati da OPENCOMPUTO_DCF_REALI"]
fn dcf_reali() {
    let Ok(cartella) = std::env::var("OPENCOMPUTO_DCF_REALI") else {
        eprintln!("OPENCOMPUTO_DCF_REALI non impostata: nessuna verifica");
        return;
    };
    let mut file: Vec<_> = std::fs::read_dir(&cartella)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dcf")))
        .collect();
    file.sort();
    assert!(!file.is_empty(), "nessun .dcf in {cartella}");
    for percorso in file {
        let dati = std::fs::read(&percorso).unwrap();
        let nome = percorso.file_name().unwrap().to_string_lossy().into_owned();
        let c = contenitore::leggi(&dati).unwrap();
        assert_eq!(c.settori_dichiarati, c.settori, "{nome}: geometria");
        let illeggibili = c
            .flussi
            .iter()
            .filter(|f| matches!(f.contenuto, Contenuto::Illeggibile(_)))
            .count();
        let item_ep: usize = c
            .xml("CollectionEP")
            .map(|t| t.matches("<ItemEP ").count())
            .sum();
        println!(
            "{nome}: {} settori, {} flussi ({} protetti, {} illeggibili), {} ItemEP nel testo",
            c.settori,
            c.flussi.len(),
            c.protetti(),
            illeggibili,
            item_ep
        );
        assert_eq!(illeggibili, 0, "{nome}: flussi non decomprimibili");
        match importa(
            &dati,
            &mut GeneratoreSequenziale::default(),
            &OpzioniImport::default(),
        ) {
            Ok(r) => {
                let s = &r.rapporto.statistiche;
                println!("  importato: {s:?}");
                for a in &r.rapporto.anomalie {
                    println!("  [{:?}] {}: {}", a.gravita, a.codice, a.messaggio);
                }
                assert!(!r.rapporto.ha_errori(), "{nome}: errori");
                assert_eq!(s.voci_lette, item_ep, "{nome}: voci lette");
                assert_eq!(
                    s.voci_elenco + s.voci_senza_tariffa + s.voci_scartate,
                    s.voci_lette
                );
                assert_eq!(r.progetto.elenco_prezzi.voci.len(), s.voci_elenco);
                assert!(
                    r.rapporto
                        .con_codice("dcf.riferimento_non_risolto")
                        .next()
                        .is_none()
                );
                let con_capitolo = r
                    .progetto
                    .elenco_prezzi
                    .voci
                    .iter()
                    .filter(|v| v.capitolo.is_some())
                    .count();
                let negativi = r
                    .progetto
                    .elenco_prezzi
                    .voci
                    .iter()
                    .filter(|v| v.prezzo.is_negativo())
                    .count();
                let totale: rust_decimal::Decimal = r
                    .progetto
                    .elenco_prezzi
                    .voci
                    .iter()
                    .map(|v| v.prezzo.valore())
                    .sum();
                println!(
                    "  voci con capitolo: {con_capitolo}, prezzi negativi: {negativi}, somma dei prezzi: {totale}"
                );
                let v = opencomputo_core::validazione::valida_progetto(&r.progetto);
                let errori: Vec<_> = v
                    .problemi
                    .iter()
                    .filter(|p| p.gravita == opencomputo_core::validazione::Gravita::Errore)
                    .map(|p| p.codice.as_str())
                    .collect();
                println!(
                    "  validazione: {} problemi, errori {:?}",
                    v.problemi.len(),
                    errori.iter().take(5).collect::<Vec<_>>()
                );
            }
            Err(e @ ErroreDcf::Protetto { .. }) => {
                println!("  rifiutato: {e}");
                assert!(c.protetti() > 0);
                assert_eq!(item_ep, 0, "{nome}: protetto ma con elenco leggibile");
            }
            Err(e) => panic!("{nome}: {e}"),
        }
    }
}

/// Struttura dei DCF reali: indice, somme di controllo, mappa, CRC dei pezzi. La
/// riscrittura dei flussi in chiaro con lo scrittore di OpenComputo è riletta uguale.
#[test]
#[ignore = "richiede DCF reali indicati da OPENCOMPUTO_DCF_REALI"]
fn struttura_dei_dcf_reali_e_riscrittura() {
    use opencomputo_dcf_parser::scrittura::{self, Elemento};
    use opencomputo_dcf_parser::struttura;
    let Ok(cartella) = std::env::var("OPENCOMPUTO_DCF_REALI") else {
        eprintln!("OPENCOMPUTO_DCF_REALI non impostata: nessuna verifica");
        return;
    };
    let mut file: Vec<_> = std::fs::read_dir(&cartella)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dcf")))
        .collect();
    file.sort();
    for percorso in file {
        let dati = std::fs::read(&percorso).unwrap();
        let nome = percorso.file_name().unwrap().to_string_lossy().into_owned();
        let s = struttura::leggi(&dati).unwrap();
        let problemi = struttura::verifica(&dati).unwrap();
        println!(
            "{nome}: {} settori, {} voci nell'indice in {} settori, problemi: {problemi:?}",
            s.settori,
            s.voci.len(),
            s.settori_indice.len()
        );
        assert!(problemi.is_empty(), "{nome}: {problemi:?}");
        // Riscrittura: stessi nomi, stesse cartelle, stessi contenuti decompressi.
        let elementi: Vec<Elemento> = s
            .voci
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if v.cartella {
                    Elemento::Cartella {
                        nome: v.nome.clone(),
                        genitore: v.genitore,
                    }
                } else {
                    Elemento::File {
                        nome: v.nome.clone(),
                        genitore: v.genitore,
                        contenuto: struttura::decomprimi(&s.flusso(&dati, i)).unwrap(),
                    }
                }
            })
            .collect();
        let data = chrono::NaiveDate::from_ymd_opt(2026, 10, 21)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let riscritto = scrittura::scrivi(&elementi, data).unwrap();
        assert!(struttura::verifica(&riscritto).unwrap().is_empty());
        let r = struttura::leggi(&riscritto).unwrap();
        assert_eq!(r.voci.len(), s.voci.len());
        for (i, (a, b)) in s.voci.iter().zip(&r.voci).enumerate() {
            assert_eq!(
                (a.nome.as_str(), a.cartella, a.genitore),
                (b.nome.as_str(), b.cartella, b.genitore)
            );
            if !a.cartella {
                assert_eq!(
                    struttura::decomprimi(&s.flusso(&dati, i)).unwrap(),
                    struttura::decomprimi(&r.flusso(&riscritto, i)).unwrap(),
                    "{nome}: {}",
                    s.percorso(i)
                );
            }
        }
    }
}

/// Prezzari reali: import, export DCF e confronto degli attributi di ogni voce con
/// l'originale.
#[test]
#[ignore = "richiede DCF reali indicati da OPENCOMPUTO_DCF_REALI"]
fn export_dei_prezzari_reali_fedele() {
    use std::collections::BTreeMap;

    use opencomputo_dcf_parser::esporta::{OpzioniExport, esporta};
    use opencomputo_dcf_parser::struttura;
    use opencomputo_dcf_parser::xml::per_elementi;
    let Ok(cartella) = std::env::var("OPENCOMPUTO_DCF_REALI") else {
        eprintln!("OPENCOMPUTO_DCF_REALI non impostata: nessuna verifica");
        return;
    };
    let voci = |testo: &str| -> BTreeMap<String, BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        per_elementi(testo, |e| {
            if e.nome == "ItemEP"
                && let Some(t) = e.attributo("Trf").filter(|t| !t.trim().is_empty())
            {
                out.insert(t.trim().to_owned(), e.attributi.iter().cloned().collect());
            }
        })
        .unwrap();
        out
    };
    let mut file: Vec<_> = std::fs::read_dir(&cartella)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dcf")))
        .collect();
    file.sort();
    for percorso in file {
        let dati = std::fs::read(&percorso).unwrap();
        let nome = percorso.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(imp) = importa(
            &dati,
            &mut GeneratoreSequenziale::default(),
            &OpzioniImport::default(),
        ) else {
            continue;
        };
        let o = OpzioniExport {
            data: chrono::NaiveDate::from_ymd_opt(2026, 10, 21)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            versione_programma: "prova".into(),
        };
        let r = esporta(&imp.progetto, &o).unwrap();
        assert!(struttura::verifica(&r.byte).unwrap().is_empty());
        let orig = contenitore::leggi(&dati).unwrap();
        let nuovo = contenitore::leggi(&r.byte).unwrap();
        let a = voci(orig.xml("CollectionEP").next().unwrap());
        let b = voci(nuovo.xml("CollectionEP").next().unwrap());
        assert_eq!(a.len(), b.len(), "{nome}: voci");
        let mut diverse: BTreeMap<String, usize> = BTreeMap::new();
        for (trf, va) in &a {
            let vb = &b[trf];
            for (k, x) in va {
                let y = vb.get(k).cloned().unwrap_or_default();
                let uguali = match k.as_str() {
                    // Numeri: stesso valore (le incidenze originali hanno artefatti di
                    // virgola mobile, arrotondati a 6 decimali dal lettore).
                    "Prz1" | "Prz2" | "Prz3" | "Prz4" | "Prz5" => {
                        x.parse::<rust_decimal::Decimal>().ok() == y.parse().ok()
                    }
                    "MDOInc" | "SICInc" => {
                        let r = |s: &str| {
                            s.parse::<rust_decimal::Decimal>()
                                .ok()
                                .map(|d| d.round_dp(6).normalize())
                        };
                        r(x) == r(&y)
                    }
                    // Testi: confronto dopo la normalizzazione XML degli a capo.
                    _ => x.split_whitespace().eq(y.split_whitespace()),
                };
                if !uguali {
                    let n = diverse.entry(k.clone()).or_default();
                    *n += 1;
                    if *n == 1 && k == "DesEst" {
                        let corto = |t: &str| t.chars().take(160).collect::<String>();
                        println!(
                            "  esempio {k}: {:?}\n        nuovo: {:?}",
                            corto(x),
                            corto(&y)
                        );
                    }
                }
            }
        }
        println!(
            "{nome}: {} voci esportate, attributi diversi: {diverse:?}",
            b.len()
        );
        for k in [
            "Trf", "Art", "DesEst", "DesBrv", "UnMsr", "Prz1", "Prz2", "IdSpCap", "IdCap",
            "IdSbCap", "Fgs", "MDOInc", "SICInc",
        ] {
            assert!(!diverse.contains_key(k), "{nome}: {k} diverso");
        }
        let riletto = importa(
            &r.byte,
            &mut GeneratoreSequenziale::default(),
            &OpzioniImport::default(),
        )
        .unwrap();
        assert_eq!(
            riletto.rapporto.statistiche.voci_elenco,
            imp.rapporto.statistiche.voci_elenco
        );
        assert_eq!(
            riletto.rapporto.statistiche.capitoli,
            imp.rapporto.statistiche.capitoli
        );
    }
}
