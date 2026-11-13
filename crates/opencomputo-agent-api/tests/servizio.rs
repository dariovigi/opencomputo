//! Protocollo del servizio a righe JSON.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;

use opencomputo_agent_api::servizio::Servizio;
use serde_json::{Value, json};

fn cartella(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "opencomputo-servizio-{nome}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn chiedi(s: &mut Servizio, v: Value) -> Value {
    let r = s.rispondi(&v.to_string());
    assert_eq!(r["ok"], true, "{v}\n→ {r:#}");
    r["risultato"].clone()
}

fn rifiuta(s: &mut Servizio, v: Value) -> Value {
    let r = s.rispondi(&v.to_string());
    assert_eq!(r["ok"], false, "{v}\n→ {r:#}");
    r["errore"].clone()
}

#[test]
fn ciclo_di_lavoro_di_un_agente() {
    let d = cartella("ciclo");
    let file = d.join("lavori.ocp");
    let mut s = Servizio::new();
    let e = rifiuta(&mut s, json!({"richiesta": "info"}));
    assert_eq!(e["codice"], "richiesta");
    assert!(
        e["descrizione"]
            .as_str()
            .unwrap()
            .contains("nessun progetto aperto")
    );

    chiedi(
        &mut s,
        json!({"richiesta": "nuovo", "titolo": "Lavori", "percorso": file}),
    );
    let comandi = json!([
        {"comando": "aggiungi_voce_prezzario", "codice": "A.1", "descrizione_breve": "Scavo", "unita_misura": "m³", "prezzo": "8.50"},
        {"comando": "aggiungi_voce_computo", "codice": "A.1", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1,20"}]}
    ]);
    // Prima la prova, poi l'esecuzione.
    let p = chiedi(&mut s, json!({"richiesta": "prova", "comandi": comandi}));
    assert_eq!(p["applicato"], false);
    assert_eq!(p["totali"][0]["dopo"], "979.20");
    assert_eq!(
        chiedi(&mut s, json!({"richiesta": "info"}))["elenco_prezzi"]["voci"],
        0
    );
    let r = chiedi(&mut s, json!({"richiesta": "esegui", "comandi": comandi}));
    assert_eq!(r["applicato"], true);

    // Con modifiche non salvate non si apre un altro progetto.
    let e = rifiuta(&mut s, json!({"richiesta": "nuovo", "titolo": "Altro"}));
    assert!(
        e["descrizione"]
            .as_str()
            .unwrap()
            .contains("modifiche non salvate")
    );
    rifiuta(&mut s, json!({"richiesta": "chiudi"}));

    let r = chiedi(&mut s, json!({"richiesta": "salva", "gzip": true}));
    assert_eq!(r["gzip"], true);
    assert_eq!(std::fs::read(&file).unwrap()[..2], [0x1f, 0x8b]);
    let i = chiedi(&mut s, json!({"richiesta": "info"}));
    assert_eq!(i["modificato"], false);
    assert_eq!(i["computi"][0]["totale"], "979.20");
    assert_eq!(i["annullabili"], json!(["2 comandi"]));

    // Annulla, ripristina, risposta con id.
    let r = s.rispondi(r#"{"id": "a1", "richiesta": "annulla"}"#);
    assert_eq!(
        r,
        json!({"id": "a1", "ok": true, "risultato": {"annullato": "2 comandi"}})
    );
    chiedi(&mut s, json!({"richiesta": "ripristina"}));
    let e = rifiuta(&mut s, json!({"richiesta": "ripristina"}));
    assert_eq!(e["codice"], "niente_da_ripristinare");

    // Riapertura: la compressione del file si conserva al salvataggio successivo.
    chiedi(&mut s, json!({"richiesta": "chiudi", "forza": true}));
    let i = chiedi(&mut s, json!({"richiesta": "apri", "percorso": file}));
    assert_eq!(i["computi"][0]["totale"], "979.20");
    chiedi(
        &mut s,
        json!({"richiesta": "esegui", "comandi": [{"comando": "imposta_metadati", "committente": "Comune"}]}),
    );
    assert_eq!(chiedi(&mut s, json!({"richiesta": "salva"}))["gzip"], true);
    assert!(d.join("lavori.ocp.bak1").exists());

    // Report ed export.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "report", "report": {"tipo": "computo"}, "formato": "xlsx", "cartella": d.join("report")}),
    );
    assert!(
        r["file"][0]["percorso"]
            .as_str()
            .unwrap()
            .ends_with("computo.xlsx")
    );
    let r = chiedi(
        &mut s,
        json!({"richiesta": "esporta_xpwe", "percorso": d.join("lavori.xpwe")}),
    );
    assert!(r["byte"].as_u64().unwrap() > 0);
    let v = chiedi(&mut s, json!({"richiesta": "valida"}));
    assert_eq!(v["valido"], true);
}

#[test]
fn errori_di_protocollo_e_di_comando() {
    let mut s = Servizio::new();
    let r = s.rispondi("non json");
    assert_eq!(r["ok"], false);
    assert!(
        r["errore"]["descrizione"]
            .as_str()
            .unwrap()
            .starts_with("JSON non valido")
    );
    let e = rifiuta(&mut s, json!({"richiesta": "vola"}));
    assert!(
        e["descrizione"]
            .as_str()
            .unwrap()
            .starts_with("richiesta non valida")
    );
    let e = rifiuta(&mut s, json!({"richiesta": "info", "extra": 1}));
    assert!(e["descrizione"].as_str().unwrap().contains("extra"));
    chiedi(&mut s, json!({"richiesta": "nuovo", "titolo": "Prova"}));
    // Campo sconosciuto in un comando: la richiesta intera è rifiutata.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "esegui", "comandi": [{"comando": "nuovo_computo", "nome": "B", "colore": "rosso"}]}),
    );
    assert!(e["descrizione"].as_str().unwrap().contains("colore"));
    let e = rifiuta(&mut s, json!({"richiesta": "esegui", "comandi": []}));
    assert_eq!(e["codice"], "nessun_comando");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "esegui", "comandi": [
        {"comando": "nuovo_computo", "nome": "B"},
        {"comando": "rimuovi_voce_computo", "computo": "B", "voce": 1}]}),
    );
    assert_eq!(e["codice"], "comando");
    assert_eq!(e["indice"], 1);
    assert_eq!(
        chiedi(&mut s, json!({"richiesta": "info"}))["computi"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let e = rifiuta(&mut s, json!({"richiesta": "salva"}));
    assert!(
        e["descrizione"]
            .as_str()
            .unwrap()
            .contains("indicare `percorso`")
    );
    // Un progetto nuovo non salvato è una modifica da proteggere.
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "apri", "percorso": "/inesistente/x.ocp"}),
    );
    assert_eq!(e["codice"], "richiesta");
    chiedi(&mut s, json!({"richiesta": "chiudi", "forza": true}));
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "apri", "percorso": "/inesistente/x.ocp"}),
    );
    assert_eq!(e["codice"], "file");
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "importa", "origine": "/inesistente/x.xpwe"}),
    );
    assert_eq!(e["codice"], "file");
}

#[test]
fn prezzario_di_consultazione_e_ricerca() {
    let mut s = Servizio::new();
    chiedi(&mut s, json!({"richiesta": "nuovo", "titolo": "Prova"}));
    let xpwe = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../opencomputo-format-xpwe/tests/fixtures/computo_sintetico.xpwe"
    );
    let r = chiedi(
        &mut s,
        json!({"richiesta": "carica_prezzario", "nome": "reg", "percorso": xpwe}),
    );
    assert_eq!(r["formato"], "xpwe");
    let n = r["voci"].as_u64().unwrap();
    assert!(n >= 2);
    let c = chiedi(
        &mut s,
        json!({"richiesta": "cerca", "testo": "", "prezzario": "reg", "limite": 1}),
    );
    assert_eq!(c["trovate"], n);
    assert_eq!(c["mostrate"], 1);
    let codice = c["voci"][0]["codice"].as_str().unwrap().to_owned();
    chiedi(
        &mut s,
        json!({"richiesta": "esegui", "comandi": [{"comando": "copia_da_prezzario", "prezzario": "reg", "codici": [codice]}]}),
    );
    let c = chiedi(&mut s, json!({"richiesta": "cerca", "testo": codice}));
    assert_eq!(c["trovate"], 1);
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "cerca", "testo": "x", "prezzario": "nessuno"}),
    );
    assert!(e["descrizione"].as_str().unwrap().contains("«nessuno»"));
    // Import di un XPWE come progetto.
    chiedi(&mut s, json!({"richiesta": "chiudi", "forza": true}));
    let r = chiedi(&mut s, json!({"richiesta": "importa", "origine": xpwe}));
    assert_eq!(r["importazione"]["formato"], "xpwe");
    assert_eq!(r["progetto"]["computi"][0]["totale"], "9291.84");
}

#[test]
fn listino_csv_di_consultazione() {
    let mut s = Servizio::new();
    chiedi(&mut s, json!({"richiesta": "nuovo", "titolo": "Prova"}));
    let csv = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../opencomputo-format-listino/tests/fixtures/listino_esempio.csv"
    );
    let r = chiedi(
        &mut s,
        json!({"richiesta": "carica_prezzario", "nome": "listino", "percorso": csv}),
    );
    assert_eq!(r["formato"], "csv");
    assert_eq!(r["voci"], 7);
    let c = chiedi(
        &mut s,
        json!({"richiesta": "cerca", "testo": "muratura", "prezzario": "listino"}),
    );
    assert_eq!(c["trovate"], 1);
    assert_eq!(c["voci"][0]["codice"], "M.01.01");
}

#[test]
fn ciclo_su_flussi() {
    let mut s = Servizio::new();
    let ingresso =
        "{\"richiesta\":\"nuovo\",\"titolo\":\"A\"}\n\n{\"id\":2,\"richiesta\":\"annulla\"}\n";
    let mut uscita = Vec::new();
    s.servi(ingresso.as_bytes(), &mut uscita).unwrap();
    let righe: Vec<Value> = String::from_utf8(uscita)
        .unwrap()
        .lines()
        .map(|r| serde_json::from_str(r).unwrap())
        .collect();
    assert_eq!(righe.len(), 2);
    assert_eq!(righe[0]["ok"], true);
    assert_eq!(righe[1]["id"], 2);
    assert_eq!(righe[1]["errore"]["codice"], "niente_da_annullare");
}

#[test]
fn viste_per_l_interfaccia_grafica() {
    let d = cartella("viste");
    let file = d.join("esempio.ocp");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        ),
        &file,
    )
    .unwrap();
    let mut s = Servizio::new();
    chiedi(&mut s, json!({"richiesta": "apri", "percorso": file}));

    let p = chiedi(&mut s, json!({"richiesta": "progetto"}));
    assert_eq!(p["metadati"]["titolo"], "Nuova scuola primaria");
    assert_eq!(p["contratto"]["parametri"]["ribasso"], "12.5");
    assert_eq!(p["computi"][0]["voci"], 4);
    assert_eq!(p["wbs"][1]["codice_completo"], "01.01");
    assert_eq!(p["wbs"][1]["livello"], 1);

    let c = chiedi(&mut s, json!({"richiesta": "computo"}));
    assert_eq!(c["totale"], "14457.12");
    let v = &c["voci"][1];
    assert_eq!(v["codice"], "B.02.010");
    assert_eq!(v["quantita"], "51.74");
    assert_eq!(v["importo"], "7517.82");
    assert_eq!(v["stato_prezzo"], "allineato");
    assert_eq!(v["categorie"]["categoria"], "ST");
    assert_eq!(v["wbs"], "01.01");
    let cordoli = &v["righi"][1];
    assert_eq!(cordoli["lunghezza"], "7,50+0,30");
    assert_eq!(cordoli["larghezza"], "0,30");
    assert_eq!(cordoli["quantita"], "3.74");

    // Contratto della GUI: un rigo rimandato così come la vista lo mostra non cambia nulla.
    let mut rigo = serde_json::Map::new();
    for campo in [
        "descrizione",
        "parti_uguali",
        "lunghezza",
        "larghezza",
        "altezza_peso",
        "formula",
        "commento",
        "deduzione",
    ] {
        if !cordoli[campo].is_null() {
            rigo.insert(campo.into(), cordoli[campo].clone());
        }
    }
    let e = chiedi(
        &mut s,
        json!({"richiesta": "prova", "comandi": [{"comando": "modifica_rigo", "voce": 2, "numero": 2, "rigo": rigo}]}),
    );
    assert_eq!(e["totali"], json!([]), "{e:#}");
    assert_eq!(e["nuovi_problemi"], json!([]));

    let k = chiedi(&mut s, json!({"richiesta": "contabilita"}));
    assert_eq!(k["parametri"]["ritenuta_garanzia"], "0.5");
    assert_eq!(k["registrazioni"][0]["importo"], "979.20");
    assert_eq!(k["registrazioni"][0]["bloccata"], true);
    assert_eq!(k["registrazioni"][0]["sal"], 1);
    assert_eq!(k["sal"][1]["riepilogo"]["importo_sal"], "2679.79");
    assert_eq!(k["sal"][1]["registrazioni"], json!([4]));

    let el = chiedi(
        &mut s,
        json!({"richiesta": "elenco_prezzi", "testo": "scavo"}),
    );
    assert_eq!(el["totale"], 4);
    assert_eq!(el["voci"].as_array().unwrap().len(), 1);
    let vp = chiedi(
        &mut s,
        json!({"richiesta": "voce_prezzario", "codice": "A.01.001"}),
    );
    assert_eq!(vp["prezzo"], "8.50");
    assert!(vp["descrizione_estesa"].is_string());
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "voce_prezzario", "codice": "ZZ"}),
    );
    assert!(e["descrizione"].as_str().unwrap().contains("«ZZ»"));

    let a = chiedi(
        &mut s,
        json!({"richiesta": "anteprima", "report": {"tipo": "sal", "numero": 1}}),
    );
    assert_eq!(a["nome"], "sal_1");
    let ultima = a["sezioni"].as_array().unwrap().last().unwrap()["righe"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        ultima,
        json!({"stile": "totale", "celle": ["IMPORTO DEL PRESENTE SAL", "8.118,60"]})
    );

    // Modello tradizionale: l'anteprima sono le pagine del PDF, a blocchi.
    let report = json!({"tipo": "computo", "tradizionale": {"ente": "Comune di Esempio"}});
    let t = chiedi(&mut s, json!({"richiesta": "anteprima", "report": report}));
    assert_eq!(t["nome"], "computo_tradizionale");
    let totali = t["pagine_totali"].as_u64().unwrap();
    assert!(totali >= 2, "{totali}");
    assert_eq!(t["prima"], 1);
    let pagine = t["pagine"].as_array().unwrap();
    assert_eq!(pagine.len() as u64, totali.min(6));
    assert!(pagine[0].as_str().unwrap().contains("Comune di Esempio"));
    assert!(
        pagine[1]
            .as_str()
            .unwrap()
            .contains("DESIGNAZIONE DEI LAVORI")
    );
    let seconda = chiedi(
        &mut s,
        json!({"richiesta": "anteprima", "report": report, "prima_pagina": 2, "pagine": 1}),
    );
    assert_eq!(
        (
            seconda["prima"].as_u64(),
            seconda["pagine"].as_array().unwrap().len()
        ),
        (Some(2), 1)
    );
    assert_eq!(seconda["pagine"][0], pagine[1]);
    // Oltre la fine: nessuna pagina, nessun errore; limiti fuori misura ricondotti.
    let oltre = chiedi(
        &mut s,
        json!({"richiesta": "anteprima", "report": report, "prima_pagina": 999, "pagine": 100000}),
    );
    assert_eq!(oltre["pagine"], json!([]));
    let zero = chiedi(
        &mut s,
        json!({"richiesta": "anteprima", "report": report, "prima_pagina": 0, "pagine": 0}),
    );
    assert_eq!(
        (
            zero["prima"].as_u64(),
            zero["pagine"].as_array().unwrap().len()
        ),
        (Some(1), 1)
    );

    // Report su un file esatto; il CSV (più file) lo rifiuta.
    let r = chiedi(
        &mut s,
        json!({"richiesta": "report", "report": {"tipo": "elenco_prezzi"}, "formato": "pdf", "file": d.join("elenco.pdf")}),
    );
    assert_eq!(r["file"][0]["percorso"], json!(d.join("elenco.pdf")));
    let r = chiedi(
        &mut s,
        json!({"richiesta": "report", "report": {"tipo": "computo", "tradizionale": {"studio": "Studio tecnico"}}, "formato": "pdf", "file": d.join("computo tradizionale.pdf")}),
    );
    assert_eq!(
        r["file"][0]["percorso"],
        json!(d.join("computo tradizionale.pdf"))
    );
    assert!(
        std::fs::read(d.join("computo tradizionale.pdf"))
            .unwrap()
            .starts_with(b"%PDF")
    );
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "report", "report": {"tipo": "elenco_prezzi", "tradizionale": {}}, "formato": "xlsx", "file": d.join("e.xlsx")}),
    );
    assert!(e["descrizione"].as_str().unwrap().contains("solo in PDF"));
    let r = chiedi(
        &mut s,
        json!({"richiesta": "esporta_dcf", "percorso": d.join("elenco.dcf"), "data": "2026-10-21"}),
    );
    assert!(r["byte"].as_u64().unwrap() > 4096);
    assert!(
        r["anomalie"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["codice"] == "dcf.export_sperimentale")
    );
    let e = rifiuta(
        &mut s,
        json!({"richiesta": "report", "report": {"tipo": "computo"}, "formato": "csv", "file": d.join("x.csv")}),
    );
    assert!(e["descrizione"].as_str().unwrap().contains("cartella"));
    rifiuta(
        &mut s,
        json!({"richiesta": "report", "report": {"tipo": "computo"}}),
    );
}

#[test]
fn richieste_che_cambiano_lo_stato() {
    let mut s = Servizio::new();
    let (r, cambiato) = s.rispondi_valore(json!({"richiesta": "nuovo", "titolo": "A"}));
    assert_eq!((r["ok"].clone(), cambiato), (json!(true), true));
    let (_, cambiato) = s.rispondi_valore(json!({"richiesta": "info"}));
    assert!(!cambiato);
    let (_, cambiato) = s.rispondi_valore(json!({"richiesta": "annulla"}));
    assert!(!cambiato, "fallita: nessun cambiamento");
    let (_, cambiato) = s.rispondi_valore(
        json!({"richiesta": "esegui", "comandi": [{"comando": "nuovo_computo", "nome": "B"}]}),
    );
    assert!(cambiato);
}
