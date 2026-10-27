//! Test di integrazione: serializzazione, schema versionato e migrazioni.

// Le opzioni `allow-*-in-tests` di clippy.toml coprono solo le funzioni `#[test]`:
// negli helper condivisi (`common`) e nei costruttori di dati di prova un errore
// deve far fallire il test immediatamente.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::prelude::*;
use opencomputo_core::schema::{
    self, ErroreSchema, FORMATO, Migrazione, VERSIONE_SCHEMA_CORRENTE, esporta_json, importa_json,
};
use rust_decimal_macros::dec;
use serde_json::{Value, json};

/// Fixture prodotta da `progetto_con_contabilita()` con i due SAL emessi.
/// Se lo schema cambia, la fixture NON va rigenerata: va scritta una migrazione.
const FIXTURE_V1: &str = include_str!("fixtures/progetto_esempio_v1.json");

/// Fixture della versione 2: come la v1, con importo contrattuale, anticipazione, IVA e
/// una penale; i SAL emessi hanno il certificato congelato.
const FIXTURE_V2: &str = include_str!("fixtures/progetto_esempio_v2.json");

/// Fixture della versione 3: contratto di 13.000 € con conto finale emesso, firmato con
/// riserve e certificato di regolare esecuzione approvato (valori in
/// `tests/conto_finale.rs`).
const FIXTURE_V3: &str = include_str!("fixtures/progetto_esempio_v3.json");

/// Fixture della versione 4: nuovi prezzi (uno da analisi e già netto), verbale di
/// concordamento approvato, SAL 1 e 2 emessi (valori in `tests/nuovi_prezzi.rs`),
/// parametri predefiniti delle analisi ed espressione in una riga.
const FIXTURE_V4: &str = include_str!("fixtures/progetto_esempio_v4.json");

/// Fixture della versione 5: come la v4, con variabili e quota degli oneri aziendali
/// della sicurezza nell'analisi del NP.1 (prezzo invariato).
const FIXTURE_V5: &str = include_str!("fixtures/progetto_esempio_v5.json");

/// Fixture della versione 6: come la v5, con una quinta voce di computo fatta di righi
/// «vedi voce» (collegati alle voci 1 e 2, uno statico) e una registrazione del libretto
/// fuori dai SAL con un «vedi voce» statico.
const FIXTURE_V6: &str = include_str!("fixtures/progetto_esempio_v6.json");

/// Fixture della versione 7: come la v6, con il cronoprogramma (inizio, calendario con
/// una chiusura, durate, un'attività aggiunta, una pietra miliare, legami di tre tipi,
/// linea di base e computo degli importi).
const FIXTURE_V7: &str = include_str!("fixtures/progetto_esempio_v7.json");

/// Il progetto riscritto nella versione corrente, con il numero di versione indicato al
/// posto di quello corrente: per i file di versioni precedenti cambia solo quello.
fn riscritta_come(p: &Progetto, versione: u32) -> String {
    let mut json = esporta_json(p).unwrap();
    json.push('\n');
    json.replacen(
        &format!("\"versione_schema\": {VERSIONE_SCHEMA_CORRENTE}"),
        &format!("\"versione_schema\": {versione}"),
        1,
    )
}

fn progetto_completo() -> Progetto {
    let mut p = progetto_con_contabilita();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    p
}

/// Progetto come lo produceva la versione 1: SAL emessi senza certificato congelato.
fn come_in_v1(mut p: Progetto) -> Progetto {
    let mut c = serde_json::to_value(&p.contabilita).unwrap();
    for s in c["sal"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("certificato_emesso");
    }
    p.contabilita = serde_json::from_value(c).unwrap();
    p
}

/// Progetto della fixture v2 (valori verificati a mano in `tests/certificato.rs`).
fn progetto_completo_v2() -> Progetto {
    let mut p = progetto_con_contabilita();
    let c = p.contratto.as_mut().unwrap();
    c.importo_contrattuale = Some(Importo::new(dec!(100000.00)));
    c.parametri.aliquota_iva = Some(Percentuale::new(dec!(10)));
    c.parametri.anticipazione = Some(Anticipazione {
        percentuale: Percentuale::new(dec!(20)),
        importo: Importo::new(dec!(20000.00)),
        percentuale_recupero: Percentuale::new(dec!(20)),
        data_erogazione: Some(data(2026, 3, 2)),
    });
    p.contabilita
        .registra_penale(Penale {
            id: PenaleId::da_u128(7001),
            numero: 1,
            data: data(2026, 4, 20),
            descrizione: "Ritardo sulla scadenza intermedia".into(),
            importo: Importo::new(dec!(150.00)),
            giorni: Some(3),
            per_mille: Some(dec!(0.5)),
        })
        .unwrap();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    p
}

/// Progetto della fixture v3.
fn progetto_completo_v3() -> Progetto {
    let mut p = progetto_per_conto_finale();
    p.emetti_conto_finale(data(2026, 6, 15)).unwrap();
    p.contabilita
        .firma_conto_finale(
            Some(data(2026, 6, 30)),
            Some("Riserva n. 1 iscritta nel registro il 30/04/2026, confermata".into()),
        )
        .unwrap();
    p.contabilita
        .registra_collaudo(Some(Collaudo {
            tipo: TipoCollaudo::RegolareEsecuzione,
            data: data(2026, 7, 20),
            approvato_il: Some(data(2026, 7, 31)),
        }))
        .unwrap();
    p
}

/// Progetto della fixture v4.
fn progetto_completo_v4() -> Progetto {
    let mut p = progetto_con_nuovi_prezzi();
    let mut v = p.contabilita.verbale_nuovi_prezzi(1).unwrap().clone();
    v.approvato_il = Some(data(2026, 4, 15));
    v.atto_approvazione = Some("Determina del RUP n. 12".into());
    p.contabilita.imposta_verbale_nuovi_prezzi(v).unwrap();
    p.elenco_prezzi.parametri_analisi = Some(ParametriAnalisi {
        spese_generali: Percentuale::new(dec!(15)),
        utile_impresa: Percentuale::new(dec!(10)),
        base_utile: BaseUtile::CostoPiuSpeseGenerali,
        quota_oneri_sicurezza: None,
    });
    p.elenco_prezzi.analisi[0].righe[1].espressione = Some("4*3".into());
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    p
}

/// Progetto della fixture v5.
fn progetto_completo_v5() -> Progetto {
    use opencomputo_core::analisi::VariabileAnalisi;
    let mut p = progetto_completo_v4();
    let a = &mut p.elenco_prezzi.analisi[0];
    a.variabili = vec![VariabileAnalisi {
        nome: "sacchi".into(),
        espressione: "12".into(),
        descrizione: "kg di premiscelato per m²".into(),
    }];
    a.righe[1].espressione = Some("sacchi".into());
    a.quota_oneri_sicurezza = Some(Percentuale::new(dec!(20)));
    p
}

/// Voce della fixture v6 con i righi «vedi voce».
const VC_VEDI_VOCE: VoceComputoId = VoceComputoId::da_u128(4005);

/// Progetto della fixture v6.
fn progetto_completo_v6() -> Progetto {
    use opencomputo_core::misura::VediVoce;
    let mut p = progetto_completo_v5();
    let elenco = p.elenco_prezzi.clone();
    let c = p.computo_mut(COMPUTO).unwrap();
    let v = c
        .aggiungi_da_prezzario(VC_VEDI_VOCE, &elenco, SCAVO, None)
        .unwrap();
    v.righi = vec![
        // Metà dello scavo: 0,5 × 126,00 = 63,00.
        RigoMisura::misura(RigoMisuraId::da_u128(5041), "Metà dello scavo")
            .con_vedi_voce(VediVoce::Collegata(VC_SCAVO))
            .con_parti(dec!(0.5)),
        // A dedurre il calcestruzzo: −51,74.
        RigoMisura::misura(RigoMisuraId::da_u128(5042), "A dedurre il calcestruzzo")
            .con_vedi_voce(VediVoce::Collegata(VC_CLS))
            .come_deduzione(),
        // Quantità copiata da un altro computo: 10,00.
        RigoMisura::misura(RigoMisuraId::da_u128(5043), "Dal computo della variante")
            .con_vedi_voce(VediVoce::Statica {
                quantita: dec!(10),
                riferimento: "voce n. 1 «E.01» del computo «Variante»".into(),
            }),
    ];
    let mut r = registrazione(&p, 7, 7, data(2026, 5, 20), SCAVO, &[]);
    r.righi = vec![
        RigoMisura::misura(RigoMisuraId::da_u128(71), "Metà dello scavo del computo")
            .con_vedi_voce(VediVoce::Statica {
                quantita: dec!(126.00),
                riferimento: "voce n. 1 «E.01» del computo «Computo»".into(),
            })
            .con_formula("vv/2"),
    ];
    p.contabilita.registra(r).unwrap();
    p
}

#[test]
fn andata_e_ritorno_senza_perdite() {
    let p = progetto_completo();
    let json = esporta_json(&p).unwrap();
    let letto = importa_json(&json).unwrap();
    assert_eq!(letto, p);
    assert_eq!(esporta_json(&letto).unwrap(), json);
}

#[test]
fn i_numeri_sono_stringhe_decimali() {
    let json = esporta_json(&progetto_completo()).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["formato"], FORMATO);
    assert_eq!(v["versione_schema"], VERSIONE_SCHEMA_CORRENTE);
    assert_eq!(
        v["progetto"]["elenco_prezzi"]["voci"][1]["prezzo"],
        json!("145.30")
    );
    assert_eq!(
        v["progetto"]["contabilita"]["sal"][0]["riepilogo_emesso"]["importo_sal"],
        json!("8118.60")
    );
}

#[test]
fn la_fixture_v1_resta_leggibile_e_coerente() {
    let p = importa_json(FIXTURE_V1).unwrap();
    assert_eq!(p, come_in_v1(progetto_completo()));
    // Senza certificato congelato, il certificato dei SAL v1 è ricalcolato.
    assert_eq!(
        p.calcola_certificato(SAL2).unwrap().totale,
        Importo::new(dec!(2679.79))
    );
    let r = p.calcola_computo(COMPUTO).unwrap();
    assert_eq!(r.totale, Importo::new(dec!(14457.12)));
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
}

#[test]
fn la_fixture_v2_resta_leggibile_e_coerente() {
    let p = importa_json(FIXTURE_V2).unwrap();
    assert_eq!(p, progetto_completo_v2());
    let c2 = p.contabilita.sal()[1].certificato_emesso.clone().unwrap();
    assert_eq!(c2.recupero_anticipazione, Importo::new(dec!(538.65)));
    assert_eq!(c2.penali, Importo::new(dec!(150.00)));
    assert_eq!(c2.totale, Importo::new(dec!(2190.25)));
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
}

#[test]
fn la_fixture_v3_resta_leggibile_e_coerente() {
    let p = importa_json(FIXTURE_V3).unwrap();
    assert_eq!(p, progetto_completo_v3());
    let conto = p.contabilita.conto_finale().unwrap();
    let r = conto.riepilogo_emesso.clone().unwrap();
    assert_eq!(r.credito_residuo, Importo::new(dec!(1176.45)));
    assert_eq!(r.totale, Importo::new(dec!(764.48)));
    assert!(p.contabilita.chiusa());
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
    // Riscritto nella versione corrente cambia solo il numero di versione.
    assert_eq!(riscritta_come(&p, 3), FIXTURE_V3);
}

#[test]
fn la_fixture_v4_resta_leggibile_e_coerente() {
    let p = importa_json(FIXTURE_V4).unwrap();
    assert_eq!(p, progetto_completo_v4());
    let s2 = p.contabilita.sal()[1].riepilogo_emesso.clone().unwrap();
    assert_eq!(s2.lavori_esclusi_dal_ribasso, Importo::new(dec!(177.10)));
    assert_eq!(s2.importo_sal, Importo::new(dec!(2899.53)));
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
    assert_eq!(riscritta_come(&p, 4), FIXTURE_V4);
}

#[test]
fn la_fixture_v5_resta_leggibile_e_coerente() {
    let p = importa_json(FIXTURE_V5).unwrap();
    assert_eq!(p, progetto_completo_v5());
    let a = p.elenco_prezzi.calcola_analisi(AN_NP1, &p.regole).unwrap();
    assert_eq!(a.prezzo_unitario, Prezzo::new(dec!(17.71)));
    // 20 % delle spese generali di 2,10.
    assert_eq!(a.oneri_sicurezza_aziendali, Some(Importo::new(dec!(0.42))));
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
    assert_eq!(riscritta_come(&p, 5), FIXTURE_V5);
}

#[test]
fn la_fixture_v6_resta_leggibile_e_coerente() {
    let p = importa_json(FIXTURE_V6).unwrap();
    assert_eq!(p, progetto_completo_v6());
    let c = p.computo(COMPUTO).unwrap();
    // 63,00 − 51,74 + 10,00 = 21,26
    assert_eq!(
        c.quantita_voce(VC_VEDI_VOCE, &p.regole).unwrap(),
        Quantita::new(dec!(21.26))
    );
    let r = c.calcola(&p.regole).unwrap();
    assert_eq!(r.voci[4].calcolo.quantita, Quantita::new(dec!(21.26)));
    let reg = p.contabilita.libretto().registrazioni.last().unwrap();
    assert_eq!(
        reg.calcola(&p.regole).unwrap().quantita,
        Quantita::new(dec!(63.00))
    );
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
    assert_eq!(riscritta_come(&p, 6), FIXTURE_V6);
}

/// Progetto della fixture v7.
fn progetto_completo_v7() -> Progetto {
    use opencomputo_core::cronoprogramma::{
        AttivitaAggiunta, Chiusura, DatiAttivita, Legame, LineaBase, RifAttivita, TipoLegame,
        VoceLineaBase,
    };
    let mut p = progetto_completo_v6();
    let c = &mut p.cronoprogramma;
    c.inizio_lavori = Some(data(2026, 3, 2));
    c.calendario.chiusure.push(Chiusura {
        dal: data(2026, 8, 10),
        al: data(2026, 8, 21),
        descrizione: "Ferie estive".into(),
    });
    c.computo = Some(COMPUTO);
    c.imposta_dati_wbs(
        WBS_FONDAZIONI,
        DatiAttivita {
            durata: Some(10),
            avanzamento: 40,
            ..DatiAttivita::default()
        },
    );
    c.imposta_dati_wbs(
        WBS_ELEVAZIONI,
        DatiAttivita {
            durata: Some(15),
            inizio_non_prima: Some(data(2026, 3, 16)),
            note: Some("Dopo la maturazione dei getti".into()),
            ..DatiAttivita::default()
        },
    );
    c.imposta_dati_wbs(
        WBS_SICUREZZA,
        DatiAttivita {
            durata: Some(20),
            ..DatiAttivita::default()
        },
    );
    let allestimento = AttivitaId::da_u128(8101);
    let fine = AttivitaId::da_u128(8102);
    c.attivita = vec![
        AttivitaAggiunta {
            id: allestimento,
            nome: "Allestimento del cantiere".into(),
            padre: Some(WBS_EDIFICIO),
            pietra_miliare: false,
            dati: DatiAttivita {
                durata: Some(3),
                ..DatiAttivita::default()
            },
        },
        AttivitaAggiunta {
            id: fine,
            nome: "Ultimazione dei lavori".into(),
            padre: None,
            pietra_miliare: true,
            dati: DatiAttivita::default(),
        },
    ];
    c.legami = vec![
        Legame {
            da: RifAttivita::Attivita(allestimento),
            a: RifAttivita::Wbs(WBS_FONDAZIONI),
            tipo: TipoLegame::FineInizio,
            ritardo: 0,
        },
        Legame {
            da: RifAttivita::Wbs(WBS_FONDAZIONI),
            a: RifAttivita::Wbs(WBS_ELEVAZIONI),
            tipo: TipoLegame::FineInizio,
            ritardo: 2,
        },
        Legame {
            da: RifAttivita::Wbs(WBS_FONDAZIONI),
            a: RifAttivita::Wbs(WBS_SICUREZZA),
            tipo: TipoLegame::InizioInizio,
            ritardo: 0,
        },
        Legame {
            da: RifAttivita::Wbs(WBS_ELEVAZIONI),
            a: RifAttivita::Attivita(fine),
            tipo: TipoLegame::FineFine,
            ritardo: 0,
        },
    ];
    c.linea_base = Some(LineaBase {
        salvata_il: data(2026, 2, 25),
        attivita: vec![VoceLineaBase {
            attivita: RifAttivita::Wbs(WBS_FONDAZIONI),
            inizio: data(2026, 3, 5),
            fine: data(2026, 3, 18),
        }],
    });
    p
}

#[test]
fn la_fixture_v7_resta_leggibile_e_coerente() {
    use opencomputo_core::cronoprogramma::{RifAttivita, calcola};
    let p = importa_json(FIXTURE_V7).unwrap();
    assert_eq!(p, progetto_completo_v7());
    let rapporto = valida_progetto(&p);
    assert!(!rapporto.ha_errori(), "{:#?}", rapporto.problemi);
    let piano = calcola(&p, None).unwrap();
    let fond = piano
        .righe
        .iter()
        .find(|r| r.attivita == RifAttivita::Wbs(WBS_FONDAZIONI))
        .unwrap();
    // Allestimento 2-4 marzo, fondazioni dal 5 al 18 marzo.
    assert_eq!(
        (fond.inizio, fond.fine),
        (data(2026, 3, 5), data(2026, 3, 18))
    );
    assert_eq!(fond.scostamento, Some(0));
    assert_eq!(riscritta_come(&p, 7), FIXTURE_V7);
}

#[test]
fn versioni_e_formati_non_supportati() {
    let json = esporta_json(&progetto_di_esempio()).unwrap();
    let mut v: Value = serde_json::from_str(&json).unwrap();

    v["versione_schema"] = json!(VERSIONE_SCHEMA_CORRENTE + 1);
    assert_eq!(
        importa_json(&v.to_string()),
        Err(ErroreSchema::VersioneFutura {
            trovata: VERSIONE_SCHEMA_CORRENTE + 1,
            supportata: VERSIONE_SCHEMA_CORRENTE
        })
    );

    v["versione_schema"] = json!("1");
    assert_eq!(
        importa_json(&v.to_string()),
        Err(ErroreSchema::VersioneMancante)
    );
    v.as_object_mut().unwrap().remove("versione_schema");
    assert_eq!(
        importa_json(&v.to_string()),
        Err(ErroreSchema::VersioneMancante)
    );

    v["versione_schema"] = json!(VERSIONE_SCHEMA_CORRENTE);
    v["formato"] = json!("altro.formato");
    assert!(matches!(
        importa_json(&v.to_string()),
        Err(ErroreSchema::FormatoSconosciuto(_))
    ));

    assert!(matches!(
        importa_json("{ non json"),
        Err(ErroreSchema::Json(_))
    ));

    // Versione precedente senza migrazioni registrate.
    v["formato"] = json!(FORMATO);
    v["versione_schema"] = json!(0);
    assert_eq!(
        importa_json(&v.to_string()),
        Err(ErroreSchema::MigrazioneMancante(0))
    );
}

#[test]
fn un_prezzo_non_numerico_e_rifiutato() {
    let json = esporta_json(&progetto_di_esempio()).unwrap();
    let mut v: Value = serde_json::from_str(&json).unwrap();
    v["progetto"]["elenco_prezzi"]["voci"][0]["prezzo"] = json!("otto euro");
    assert!(matches!(
        importa_json(&v.to_string()),
        Err(ErroreSchema::Json(_))
    ));
}

/// Migrazione d'esempio da un ipotetico schema 0 in cui il titolo era un
/// campo di primo livello del progetto anziché dei metadati.
fn sposta_titolo(mut progetto: Value) -> Result<Value, String> {
    let oggetto = progetto
        .as_object_mut()
        .ok_or("il progetto non è un oggetto")?;
    let titolo = oggetto.remove("titolo").ok_or("campo «titolo» assente")?;
    oggetto.insert("metadati".into(), json!({ "titolo": titolo }));
    Ok(progetto)
}

#[test]
fn migrazione_esplicita_da_una_versione_precedente() {
    let migrazioni = [Migrazione {
        da: 0,
        descrizione: "titolo spostato nei metadati",
        applica: sposta_titolo,
    }];
    let corrente = esporta_json(&progetto_di_esempio()).unwrap();
    let mut v: Value = serde_json::from_str(&corrente).unwrap();
    v["versione_schema"] = json!(0);
    let titolo = v["progetto"]["metadati"]["titolo"].clone();
    v["progetto"].as_object_mut().unwrap().remove("metadati");
    v["progetto"]["titolo"] = titolo;

    let p = schema::importa_json_con(&v.to_string(), &migrazioni, 1).unwrap();
    assert_eq!(p.metadati.titolo, "Nuova scuola primaria");
    assert_eq!(
        p.calcola_computo(COMPUTO).unwrap().totale,
        Importo::new(dec!(14457.12))
    );

    // Una migrazione che fallisce produce un errore esplicito.
    v["progetto"].as_object_mut().unwrap().remove("titolo");
    assert!(matches!(
        schema::importa_json_con(&v.to_string(), &migrazioni, 1),
        Err(ErroreSchema::Migrazione { da: 0, .. })
    ));
}

/// Scrive la fixture della versione corrente dello schema.
/// Eseguire solo per creare la fixture di una **nuova** versione:
/// `cargo test -p opencomputo-core --test schema -- --ignored scrivi_fixture`
#[test]
#[ignore = "genera la fixture dello schema corrente; non fa parte della suite"]
fn scrivi_fixture() {
    let percorso = format!(
        "{}/tests/fixtures/progetto_esempio_v{VERSIONE_SCHEMA_CORRENTE}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    // Una fixture già generata è congelata: non va mai sovrascritta.
    let esistente = std::fs::read_to_string(&percorso).unwrap_or_default();
    assert!(
        esistente.trim().is_empty(),
        "{percorso} esiste già: le fixture non si rigenerano, serve una migrazione"
    );
    let mut json = esporta_json(&progetto_completo_v7()).unwrap();
    json.push('\n');
    std::fs::write(percorso, json).unwrap();
}
