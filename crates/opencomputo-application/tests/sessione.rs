//! Flussi completi attraverso la sessione: il progetto della fixture v1 ricostruito
//! con soli comandi JSON deve dare gli stessi totali verificati a mano.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use opencomputo_application::casi_uso::{self, FormatoReport, TipoReport};
use opencomputo_application::{Comando, ErroreApplicazione, Politica, Sessione, viste};
use opencomputo_core::data::data_amg;
use opencomputo_core::numeri::Importo;
use opencomputo_core::schema::importa_json;
use opencomputo_core::validazione::valida_progetto;
use opencomputo_io::report::OpzioniReport;
use opencomputo_storage::OpzioniSalvataggio;
use rust_decimal_macros::dec;
use std::sync::Arc;

fn comandi(json: &str) -> Vec<Comando> {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}\n{json}"))
}

const ELENCO: &str = r#"[
  {"comando": "imposta_metadati", "titolo": "Nuova scuola primaria", "committente": "Comune di Esempio", "luogo": "Via Roma 1"},
  {"comando": "aggiungi_voce_prezzario", "codice": "A.01.001", "descrizione_breve": "Scavo di sbancamento", "unita_misura": "m³", "prezzo": "8.50", "incidenza_manodopera": "25"},
  {"comando": "aggiungi_voce_prezzario", "codice": "B.02.010", "descrizione_breve": "Calcestruzzo per fondazioni C25/30", "unita_misura": "m³", "prezzo": "145.30", "incidenza_manodopera": "18.5"},
  {"comando": "aggiungi_voce_prezzario", "codice": "C.03.020", "descrizione_breve": "Muratura in blocchi di laterizio", "unita_misura": "m²", "prezzo": "42.75", "incidenza_manodopera": "45"},
  {"comando": "aggiungi_voce_prezzario", "codice": "S.01.001", "descrizione_breve": "Recinzione provvisoria di cantiere", "unita_misura": "m", "prezzo": "12.00", "natura": "oneri_sicurezza"}
]"#;

const COMPUTO: &str = r#"[
  {"comando": "aggiungi_voce_computo", "codice": "A.01.001", "righi": [
    {"descrizione": "Platea", "lunghezza": "12.00", "larghezza": "8.00", "altezza_peso": "1.20"},
    {"descrizione": "Rampa di accesso", "lunghezza": 6, "larghezza": 3, "altezza_peso": "1,20", "formula": "0,5*lun*lar*alt"}]},
  {"comando": "aggiungi_voce_computo", "codice": "B.02.010", "righi": [
    {"descrizione": "Platea", "lunghezza": 12, "larghezza": 8, "altezza_peso": "0.50"},
    {"descrizione": "Cordoli", "parti_uguali": 4, "lunghezza": "7,50+0,30", "larghezza": "0.30", "altezza_peso": "0.40"}]},
  {"comando": "aggiungi_voce_computo", "codice": "C.03.020", "righi": [
    {"descrizione": "Piano terra", "commento": true},
    {"descrizione": "Pareti lato lungo", "parti_uguali": 2, "lunghezza": 12, "altezza_peso": 3},
    {"descrizione": "Pareti lato corto", "parti_uguali": 2, "lunghezza": 8, "altezza_peso": 3},
    {"descrizione": "A dedurre finestre", "parti_uguali": 6, "lunghezza": "1.20", "altezza_peso": "1.50", "deduzione": true}]},
  {"comando": "aggiungi_voce_computo", "codice": "S.01.001", "righi": [{"lunghezza": "2*(30+20)"}]}
]"#;

const CONTABILITA: &str = r#"[
  {"comando": "imposta_contratto", "ribasso": "12.5", "ritenuta_garanzia": "0.5"},
  {"comando": "registra", "data": "2026-03-10", "codice": "A.01.001", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1.2"}]},
  {"comando": "registra", "data": "2026-03-20", "codice": "B.02.010", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "0.5"}]},
  {"comando": "registra", "data": "2026-03-25", "codice": "S.01.001", "righi": [{"lunghezza": 100}]},
  {"comando": "registra", "data": "2026-04-15", "codice": "C.03.020", "righi": [{"lunghezza": 24, "larghezza": 3}]},
  {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-03-31"},
  {"comando": "emetti_sal", "numero": 1, "data_emissione": "2026-04-05"},
  {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-04-30"},
  {"comando": "emetti_sal", "numero": 2, "data_emissione": "2026-05-06"}
]"#;

fn sessione_completa() -> Sessione {
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    s.esegui(&comandi(COMPUTO)).unwrap();
    s.esegui(&comandi(CONTABILITA)).unwrap();
    s
}

fn totale(s: &Sessione) -> Importo {
    let p = s.progetto();
    p.calcola_computo(p.computi[0].id).unwrap().totale
}

#[test]
fn progetto_della_fixture_ricostruito_con_comandi() {
    let s = sessione_completa();
    let p = s.progetto();
    assert_eq!(totale(&s), Importo::new(dec!(14457.12)));
    let r = p.calcola_computo(p.computi[0].id).unwrap();
    assert_eq!(r.totale_oneri_sicurezza, Importo::new(dec!(1200.00)));
    let sal: Vec<_> = p
        .contabilita
        .sal()
        .iter()
        .map(|s| s.riepilogo_emesso.as_ref().unwrap().importo_sal)
        .collect();
    assert_eq!(
        sal,
        [Importo::new(dec!(8118.60)), Importo::new(dec!(2679.79))]
    );
    assert!(!valida_progetto(p).ha_errori());
    // Stessi totali del progetto della fixture v1 del core.
    let fixture = importa_json(include_str!(
        "../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
    ))
    .unwrap();
    let rf = fixture.calcola_computo(fixture.computi[0].id).unwrap();
    assert_eq!(rf.totale, r.totale);
    assert_eq!(s.passi_annullabili().len(), 3);
    assert_eq!(s.passi_annullabili()[0], "9 comandi");
}

#[test]
fn esito_con_totali_e_problemi() {
    let mut s = Sessione::nuovo("Prova");
    let e = s.esegui(&comandi(ELENCO)).unwrap();
    assert!(e.applicato);
    assert_eq!(e.comandi.len(), 5);
    assert_eq!(
        e.comandi[1].descrizione,
        "voce «A.01.001» aggiunta all'elenco prezzi"
    );
    assert!(e.totali.is_empty());
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "aggiungi_voce_computo", "codice": "A.01.001"}]"#,
        ))
        .unwrap();
    // Voce senza righi: avviso, non errore; totale invariato a zero.
    assert!(
        e.nuovi_problemi
            .iter()
            .any(|p| p.codice == "misura.nessun_rigo")
    );
    assert_eq!(
        e.comandi[0].descrizione,
        "voce n. 1 «A.01.001» aggiunta al computo «Computo metrico estimativo»"
    );
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "aggiungi_rigo", "voce": 1, "rigo": {"lunghezza": 10}}]"#,
        ))
        .unwrap();
    assert!(
        e.problemi_risolti
            .iter()
            .any(|p| p.codice == "misura.nessun_rigo")
    );
    assert_eq!(e.totali[0].prima, Some(Importo::new(dec!(0.00))));
    assert_eq!(e.totali[0].dopo, Some(Importo::new(dec!(85.00))));
}

#[test]
fn richieste_atomiche_e_prova() {
    let mut s = sessione_completa();
    let prima = s.progetto().clone();
    // Il secondo comando fallisce: nemmeno il primo viene applicato.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_metadati", "titolo": "Cambiato"},
                {"comando": "aggiungi_voce_computo", "codice": "Z.99"}]"#,
        ))
        .unwrap_err();
    assert_eq!(
        e,
        ErroreApplicazione::Comando {
            indice: 1,
            messaggio: "voce di elenco prezzi «Z.99» inesistente".into()
        }
    );
    assert_eq!(
        e.to_string(),
        "comando n. 2 non applicabile: voce di elenco prezzi «Z.99» inesistente"
    );
    assert_eq!(s.progetto(), &prima);

    // Prova: esito completo, progetto invariato, nessun passo aggiunto.
    let passi = s.passi_annullabili().len();
    let e = s
        .prova(&comandi(
            r#"[{"comando": "modifica_rigo", "voce": 4, "numero": 1, "rigo": {"lunghezza": 150}}]"#,
        ))
        .unwrap();
    assert!(!e.applicato);
    assert_eq!(e.totali[0].dopo, Some(Importo::new(dec!(15057.12))));
    assert_eq!(s.progetto(), &prima);
    assert_eq!(s.passi_annullabili().len(), passi);
    assert_eq!(s.prova(&[]), Err(ErroreApplicazione::NessunComando));
}

#[test]
fn nuovi_errori_rifiutati_salvo_politica_permissiva() {
    let mut s = sessione_completa();
    let rotto =
        comandi(r#"[{"comando": "aggiungi_rigo", "voce": 1, "rigo": {"lunghezza": "2*("}}]"#);
    let e = s.esegui(&rotto).unwrap_err();
    let ErroreApplicazione::NuoviErrori { problemi } = &e else {
        panic!("{e:?}")
    };
    assert!(
        problemi.iter().all(|p| p.codice == "calcolo.errore"),
        "{problemi:#?}"
    );
    assert_eq!(totale(&s), Importo::new(dec!(14457.12)));
    // Errori preesistenti non bloccano altri comandi; con la politica permissiva l'errore passa.
    let mut s = Sessione::new(s.progetto().clone()).con_politica(Politica {
        rifiuta_nuovi_errori: false,
        ..Politica::default()
    });
    let e = s.esegui(&rotto).unwrap();
    assert!(
        e.nuovi_problemi
            .iter()
            .any(|p| p.codice == "calcolo.errore")
    );
    assert_eq!(e.totali[0].dopo, None, "computo non più calcolabile");
}

#[test]
fn le_regole_della_contabilita_restano_in_vigore() {
    let mut s = sessione_completa();
    // Registrazione in un SAL emesso: bloccata dal core.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "modifica_registrazione", "numero": 1, "righi": [{"lunghezza": 1}]}]"#,
        ))
        .unwrap_err();
    assert!(
        matches!(e, ErroreApplicazione::Comando { indice: 0, .. }),
        "{e}"
    );
    // Cambiare il ribasso altererebbe i SAL emessi: rifiutato dalla validazione.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_contratto", "ribasso": "10"}]"#,
        ))
        .unwrap_err();
    assert!(matches!(e, ErroreApplicazione::NuoviErrori { .. }), "{e}");
    assert!(e.to_string().contains("errori di validazione"));
    let ErroreApplicazione::NuoviErrori { problemi } = &e else {
        unreachable!()
    };
    assert!(
        problemi
            .iter()
            .any(|p| p.codice == "contabilita.sal_alterato")
    );
    // Le regole possono cambiare se i SAL emessi restano identici al ricalcolo.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_regole", "decimali_quantita": 3}]"#,
        ))
        .unwrap();
    assert!(e.nuovi_problemi.is_empty());
}

#[test]
fn annulla_e_ripristina() {
    let mut s = Sessione::nuovo("Prova");
    assert_eq!(s.annulla(), Err(ErroreApplicazione::NienteDaAnnullare));
    s.esegui(&comandi(ELENCO)).unwrap();
    s.esegui(&comandi(COMPUTO)).unwrap();
    let completo = s.progetto().clone();
    assert_eq!(s.annulla().unwrap(), "4 comandi");
    assert!(s.progetto().computi[0].voci.is_empty());
    assert_eq!(s.passi_ripristinabili(), ["4 comandi"]);
    assert_eq!(s.ripristina().unwrap(), "4 comandi");
    assert_eq!(s.progetto(), &completo);
    assert_eq!(
        s.ripristina(),
        Err(ErroreApplicazione::NienteDaRipristinare)
    );
    // Un nuovo comando dopo un annullamento cancella i ripristini.
    s.annulla().unwrap();
    s.esegui(&comandi(
        r#"[{"comando": "nuovo_computo", "nome": "Variante"}]"#,
    ))
    .unwrap();
    assert!(s.passi_ripristinabili().is_empty());
    // Limite dei passi.
    let mut s = Sessione::nuovo("Prova").con_politica(Politica {
        limite_annulla: 2,
        ..Politica::default()
    });
    for i in 0..4 {
        s.esegui(&comandi(&format!(
            r#"[{{"comando": "imposta_metadati", "titolo": "v{i}"}}]"#
        )))
        .unwrap();
    }
    assert_eq!(s.passi_annullabili().len(), 2);
}

#[test]
fn riferimenti_voci_e_computi() {
    let mut s = sessione_completa();
    s.esegui(&comandi(
        r#"[{"comando": "sposta_voce_computo", "voce": 4, "posizione": 1}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let codici: Vec<_> = p.computi[0]
        .voci
        .iter()
        .map(|v| v.prezzo.codice.as_str())
        .collect();
    assert_eq!(codici, ["S.01.001", "A.01.001", "B.02.010", "C.03.020"]);
    // Per identificativo.
    let id = p.computi[0].voci[1].id;
    s.esegui(&[Comando::RimuoviVoceComputo {
        computo: None,
        voce: opencomputo_application::RifVoce::Id(id),
    }])
    .unwrap();
    assert_eq!(totale(&s), Importo::new(dec!(13386.12)));
    // Con due computi il nome diventa obbligatorio.
    s.esegui(&comandi(
        r#"[{"comando": "nuovo_computo", "nome": "Variante"}]"#,
    ))
    .unwrap();
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "aggiungi_voce_computo", "codice": "A.01.001"}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("indicare `computo` per nome"), "{e}");
    s.esegui(&comandi(
        r#"[{"comando": "aggiungi_voce_computo", "computo": "Variante", "codice": "A.01.001", "righi": [{"lunghezza": 2}]}]"#,
    ))
    .unwrap();
    assert_eq!(s.progetto().computi[1].voci.len(), 1);
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "rimuovi_voce_computo", "computo": "Variante", "voce": 5}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("voce n. 5 inesistente"), "{e}");
}

#[test]
fn prezzi_fotografati_e_riallineamento() {
    let mut s = sessione_completa();
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "modifica_voce_prezzario", "codice": "A.01.001", "prezzo": "9.00"}]"#,
        ))
        .unwrap();
    assert!(e.totali.is_empty(), "il computo non cambia");
    assert!(
        e.nuovi_problemi
            .iter()
            .any(|p| p.codice == "prezzo.non_allineato")
    );
    let e = s
        .esegui(&comandi(r#"[{"comando": "riallinea_prezzo", "voce": 1}]"#))
        .unwrap();
    // 126,00 m³ × 0,50 € in più = 63,00 €.
    assert_eq!(e.totali[0].dopo, Some(Importo::new(dec!(14520.12))));
    assert!(
        e.problemi_risolti
            .iter()
            .any(|p| p.codice == "prezzo.non_allineato")
    );
}

#[test]
fn categorie_wbs_e_identificativi_univoci() {
    let mut s = Sessione::new(
        importa_json(include_str!(
            "../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        ))
        .unwrap(),
    );
    s.esegui(&comandi(
        r#"[{"comando": "aggiungi_categoria", "livello": "categoria", "codice": "IM", "descrizione": "Impianti"},
            {"comando": "aggiungi_nodo_wbs", "codice": "03", "descrizione": "Impianti", "padre": "01"},
            {"comando": "aggiungi_voce_computo", "codice": "A.01.001", "categorie": {"categoria": "IM"}, "wbs": "01.03", "righi": [{"lunghezza": 1}]},
            {"comando": "modifica_voce_computo", "voce": 1, "categorie": {"categoria": ""}, "note": "verificare"}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let nuova = p.computi[0].voci.last().unwrap();
    let im = p
        .categorie
        .categorie
        .iter()
        .find(|c| c.codice == "IM")
        .unwrap();
    assert_eq!(nuova.categorie.categoria, Some(im.id));
    assert_eq!(p.wbs.codice_completo(nuova.wbs.unwrap()).unwrap(), "01.03");
    assert_eq!(p.computi[0].voci[0].categorie.categoria, None);
    assert_eq!(p.computi[0].voci[0].note.as_deref(), Some("verificare"));
    // Nessun identificativo duplicato con quelli già presenti nella fixture.
    assert!(
        !valida_progetto(p)
            .problemi
            .iter()
            .any(|x| x.codice.ends_with("id_duplicato"))
    );
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "aggiungi_voce_computo", "codice": "A.01.001", "wbs": "09"}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("nodo WBS «09» inesistente"));
}

#[test]
fn copia_da_un_prezzario_di_consultazione() {
    let xpwe =
        include_bytes!("../../opencomputo-format-xpwe/tests/fixtures/computo_sintetico.xpwe");
    let (prezzario, rapporto) = casi_uso::leggi_prezzario(xpwe, Some("sintetico.xpwe")).unwrap();
    assert_eq!(rapporto.formato, "xpwe");
    let mut s = Sessione::nuovo("Prova");
    s.carica_prezzario("regionale", prezzario);
    s.esegui(&comandi(
        r#"[{"comando": "copia_da_prezzario", "prezzario": "regionale", "codici": ["E.01.010", "E.02.020"]},
            {"comando": "aggiungi_voce_computo", "codice": "E.02.020", "righi": [{"lunghezza": 10}]}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    assert_eq!(p.elenco_prezzi.voci.len(), 2);
    let capitoli: Vec<_> = p
        .elenco_prezzi
        .capitoli
        .iter()
        .map(|c| c.codice.as_str())
        .collect();
    assert_eq!(capitoli, ["E.01", "E.02"]);
    assert_eq!(totale(&s), Importo::new(dec!(1524.00)));
    let e = s
        .esegui(&comandi(r#"[{"comando": "copia_da_prezzario", "prezzario": "regionale", "codici": ["E.01.010"]}]"#))
        .unwrap_err();
    assert!(e.to_string().contains("già presente"));
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "copia_da_prezzario", "prezzario": "altro", "codici": []}]"#,
        ))
        .unwrap_err();
    assert!(
        e.to_string()
            .contains("prezzario caricato «altro» inesistente")
    );
}

#[test]
fn salvataggio_import_export_e_report() {
    let dir = std::env::temp_dir().join(format!("opencomputo-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let percorso = dir.join("scuola.ocp");
    let mut s = sessione_completa();
    assert!(s.modificato());
    s.salva(&percorso, &OpzioniSalvataggio::default()).unwrap();
    assert!(!s.modificato());
    let riaperta = Sessione::apri(&percorso).unwrap();
    assert_eq!(riaperta.progetto(), s.progetto());

    // Export XPWE e reimport: stesso totale.
    let x = casi_uso::esporta_xpwe(s.progetto(), None).unwrap();
    let (s2, r) = casi_uso::importa_xpwe(x.xml.as_bytes(), Some("scuola.xpwe")).unwrap();
    assert_eq!(r.voci_computo, 4);
    assert_eq!(totale(&s2), Importo::new(dec!(14457.12)));

    // Report nei tre formati.
    let o = OpzioniReport::default();
    let pdf = casi_uso::report(
        s.progetto(),
        &TipoReport::Sal {
            numero: 2,
            tradizionale: None,
        },
        FormatoReport::Pdf,
        &o,
    )
    .unwrap();
    assert_eq!(pdf[0].nome, "sal_2.pdf");
    assert!(pdf[0].byte.starts_with(b"%PDF"));
    let csv = casi_uso::report(
        s.progetto(),
        &TipoReport::Computo {
            computo: None,
            con_analisi: false,
            tradizionale: None,
            incidenze: Default::default(),
        },
        FormatoReport::Csv,
        &o,
    )
    .unwrap();
    assert_eq!(csv[0].nome, "computo_1_computo.csv");
    assert!(
        String::from_utf8(csv[0].byte.clone())
            .unwrap()
            .contains(";14457,12\r\n")
    );
    let xlsx = casi_uso::report(
        s.progetto(),
        &TipoReport::Registro { tradizionale: None },
        FormatoReport::Xlsx,
        &o,
    )
    .unwrap();
    assert!(xlsx[0].byte.starts_with(b"PK"));
    let e = casi_uso::report(
        s.progetto(),
        &TipoReport::Sal {
            numero: 9,
            tradizionale: None,
        },
        FormatoReport::Pdf,
        &o,
    )
    .unwrap_err();
    assert_eq!(e.to_string(), "SAL n. 9 inesistente");

    // Modello tradizionale: solo PDF, dal JSON delle richieste.
    let tipo: TipoReport = serde_json::from_str(
        r#"{"tipo": "computo", "tradizionale": {"ente": "Comune di Esempio\nProvincia di Esempio", "senza_prezzi": true}}"#,
    )
    .unwrap();
    let TipoReport::Computo {
        tradizionale: Some(t),
        ..
    } = &tipo
    else {
        panic!("{tipo:?}");
    };
    assert!(t.frontespizio && t.descrizioni_ridotte && t.senza_prezzi);
    let f = casi_uso::report(s.progetto(), &tipo, FormatoReport::Pdf, &o).unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].nome, "computo_metrico_tradizionale.pdf");
    assert!(f[0].byte.starts_with(b"%PDF-1.4"));
    let e = casi_uso::report(s.progetto(), &tipo, FormatoReport::Xlsx, &o).unwrap_err();
    assert!(e.to_string().contains("solo in PDF"), "{e}");
    // L'anteprima mostra gli stessi dati con il documento ordinario.
    let (nome, _) = casi_uso::documento(s.progetto(), &tipo, &o).unwrap();
    assert_eq!(nome, "computo");

    // Incidenze (ADR 0021): dal JSON, nel modello ordinario e nel tradizionale; il
    // computo metrico senza prezzi le rifiuta.
    let incidenze: TipoReport = serde_json::from_str(
        r#"{"tipo": "computo", "incidenze": {"manodopera": true, "sicurezza": true, "riepilogo": true}}"#,
    )
    .unwrap();
    let (_, d) = casi_uso::documento(s.progetto(), &incidenze, &o).unwrap();
    assert!(
        d.sezioni[0]
            .tabella
            .colonne
            .iter()
            .any(|c| c.titolo == "Manodopera €")
    );
    assert!(
        d.sezioni[0]
            .tabella
            .colonne
            .iter()
            .any(|c| c.titolo == "Sicurezza €")
    );
    assert!(
        d.sezioni[1]
            .tabella
            .colonne
            .iter()
            .any(|c| c.titolo == "Incidenza %")
    );
    // Senza incidenze il campo non compare nella serializzazione.
    let senza = serde_json::to_value(TipoReport::Computo {
        computo: None,
        con_analisi: false,
        tradizionale: None,
        incidenze: Default::default(),
    })
    .unwrap();
    assert!(senza.get("incidenze").is_none(), "{senza}");
    let tradizionale: TipoReport = serde_json::from_str(
        r#"{"tipo": "computo", "tradizionale": {}, "incidenze": {"manodopera": true, "riepilogo": true}}"#,
    )
    .unwrap();
    let f = casi_uso::report(s.progetto(), &tradizionale, FormatoReport::Pdf, &o).unwrap();
    assert_eq!(f[0].nome, "computo_tradizionale.pdf");
    let e = casi_uso::report(
        s.progetto(),
        &tipo_con_incidenze(&tipo),
        FormatoReport::Pdf,
        &o,
    )
    .unwrap_err();
    assert!(
        e.to_string()
            .contains("senza prezzi non stampa le incidenze"),
        "{e}"
    );
    let elenco: TipoReport =
        serde_json::from_str(r#"{"tipo": "elenco_prezzi", "tradizionale": {}}"#).unwrap();
    let f = casi_uso::report(s.progetto(), &elenco, FormatoReport::Pdf, &o).unwrap();
    assert_eq!(f[0].nome, "elenco_prezzi_tradizionale.pdf");
    let ordinario: TipoReport = serde_json::from_str(r#"{"tipo": "elenco_prezzi"}"#).unwrap();
    assert_eq!(ordinario, TipoReport::ElencoPrezzi { tradizionale: None });
    assert_eq!(
        serde_json::to_string(&ordinario).unwrap(),
        r#"{"tipo":"elenco_prezzi"}"#
    );
    let con_analisi: TipoReport =
        serde_json::from_str(r#"{"tipo": "computo", "con_analisi": true, "tradizionale": {}}"#)
            .unwrap();
    // Dal 2026-10-22 anche il computo con le analisi ha il modello tradizionale.
    let f = casi_uso::report(s.progetto(), &con_analisi, FormatoReport::Pdf, &o).unwrap();
    assert_eq!(f[0].nome, "computo_con_analisi_tradizionale.pdf");
    // Errori serializzabili per le risposte JSON.
    let j = serde_json::to_value(&e).unwrap();
    assert_eq!(j["codice"], "richiesta");
}

#[test]
fn modifica_e_rimozione_di_categorie_e_wbs() {
    let mut s = Sessione::new(
        importa_json(include_str!(
            "../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        ))
        .unwrap(),
    );
    s.esegui(&comandi(
        r#"[{"comando": "modifica_categoria", "livello": "categoria", "codice": "ST", "nuovo_codice": "STR", "descrizione": "Strutture in c.a."},
            {"comando": "aggiungi_categoria", "livello": "categoria", "codice": "IM", "descrizione": "Impianti"},
            {"comando": "rimuovi_categoria", "livello": "categoria", "codice": "IM"},
            {"comando": "aggiungi_nodo_wbs", "codice": "09", "descrizione": "Da togliere", "padre": "01"},
            {"comando": "modifica_nodo_wbs", "codice": "01.09", "nuovo_codice": "08", "descrizione": "Rinominato"},
            {"comando": "rimuovi_nodo_wbs", "codice": "01.08"}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let st = p
        .categorie
        .categorie
        .iter()
        .find(|c| c.codice == "STR")
        .unwrap();
    assert_eq!(st.descrizione, "Strutture in c.a.");
    assert!(!p.categorie.categorie.iter().any(|c| c.codice == "IM"));
    // La voce 2 resta assegnata alla categoria rinominata.
    assert_eq!(p.computi[0].voci[1].categorie.categoria, Some(st.id));
    assert!(
        !p.wbs
            .nodi
            .iter()
            .any(|n| n.codice == "08" || n.codice == "09")
    );

    let rifiuti = [
        (
            r#"{"comando": "modifica_categoria", "livello": "categoria", "codice": "STR", "nuovo_codice": "MU"}"#,
            "già presente",
        ),
        (
            r#"{"comando": "rimuovi_categoria", "livello": "categoria", "codice": "STR"}"#,
            "assegnata a",
        ),
        (
            r#"{"comando": "rimuovi_nodo_wbs", "codice": "01"}"#,
            "nodi figli",
        ),
        (
            r#"{"comando": "rimuovi_nodo_wbs", "codice": "01.01"}"#,
            "assegnato a",
        ),
        (
            r#"{"comando": "modifica_nodo_wbs", "codice": "01.01", "nuovo_codice": "a.b"}"#,
            "punti",
        ),
        (
            r#"{"comando": "modifica_categoria", "livello": "categoria", "codice": "ZZ", "descrizione": "x"}"#,
            "inesistente",
        ),
    ];
    for (c, atteso) in rifiuti {
        let e = s.esegui(&comandi(&format!("[{c}]"))).unwrap_err();
        assert!(e.to_string().contains(atteso), "{c}: {e}");
    }
}

#[test]
fn annotazioni_delle_registrazioni() {
    let mut s = sessione_completa();
    // SAL emessi: la registrazione 1 è bloccata anche per le annotazioni.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "modifica_registrazione", "numero": 1, "annotazioni": "x"}]"#,
        ))
        .unwrap_err();
    assert!(matches!(e, ErroreApplicazione::Comando { .. }), "{e}");
    s.esegui(&comandi(
        r#"[{"comando": "registra", "data": "2026-05-10", "codice": "A.01.001", "righi": [{"lunghezza": 1}]},
            {"comando": "modifica_registrazione", "numero": 5, "annotazioni": "presente l'impresa"}]"#,
    ))
    .unwrap();
    let r = &s.progetto().contabilita.libretto().registrazioni[4];
    assert_eq!(r.annotazioni.as_deref(), Some("presente l'impresa"));
    s.esegui(&comandi(
        r#"[{"comando": "modifica_registrazione", "numero": 5, "annotazioni": ""}]"#,
    ))
    .unwrap();
    assert_eq!(
        s.progetto().contabilita.libretto().registrazioni[4].annotazioni,
        None
    );
}

/// Gli stessi valori di `opencomputo-core/tests/certificato.rs`, ottenuti con i comandi.
#[test]
fn anticipazione_penali_e_certificati_con_i_comandi() {
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    s.esegui(&comandi(
        r#"[{"comando": "imposta_contratto", "ribasso": "12.5", "ritenuta_garanzia": "0.5", "importo_contrattuale": "100000.00", "aliquota_iva": "10"},
            {"comando": "imposta_anticipazione", "data_erogazione": "2026-03-02"},
            {"comando": "registra", "data": "2026-03-10", "codice": "A.01.001", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1.2"}]},
            {"comando": "registra", "data": "2026-03-20", "codice": "B.02.010", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "0.5"}]},
            {"comando": "registra", "data": "2026-03-25", "codice": "S.01.001", "righi": [{"lunghezza": 100}]},
            {"comando": "registra", "data": "2026-04-15", "codice": "C.03.020", "righi": [{"lunghezza": 24, "larghezza": 3}]},
            {"comando": "registra_penale", "data": "2026-04-20", "descrizione": "Ritardo", "giorni": 3, "per_mille": "0.5"},
            {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-03-31"},
            {"comando": "emetti_sal", "numero": 1, "data_emissione": "2026-04-05"},
            {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-04-30"},
            {"comando": "emetti_sal", "numero": 2, "data_emissione": "2026-05-06"}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let a = p
        .contratto
        .as_ref()
        .unwrap()
        .parametri
        .anticipazione
        .clone()
        .unwrap();
    assert_eq!(
        a.importo,
        Importo::new(dec!(20000.00)),
        "20 % di 100.000,00"
    );
    let c: Vec<_> = p
        .contabilita
        .sal()
        .iter()
        .map(|x| x.certificato_emesso.clone().unwrap())
        .collect();
    assert_eq!(c[0].totale, Importo::new(dec!(7135.39)));
    assert_eq!(c[1].penali, Importo::new(dec!(150.00)));
    assert_eq!(c[1].totale, Importo::new(dec!(2190.25)));
    assert!(!valida_progetto(p).ha_errori());

    // Dopo l'emissione: penale applicata non rimovibile, IVA bloccata dai certificati.
    let e = s
        .esegui(&comandi(r#"[{"comando": "rimuovi_penale", "numero": 1}]"#))
        .unwrap_err();
    assert!(e.to_string().contains("SAL n. 2"), "{e}");
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_contratto", "aliquota_iva": "22"}]"#,
        ))
        .unwrap_err();
    let ErroreApplicazione::NuoviErrori { problemi } = &e else {
        panic!("{e}")
    };
    assert!(
        problemi
            .iter()
            .all(|p| p.codice == "contabilita.certificato_alterato")
    );
    // Penale oltre il 10 %: rifiutata dalla validazione.
    let e = s
        .esegui(&comandi(r#"[{"comando": "registra_penale", "data": "2026-06-01", "descrizione": "x", "importo": "9850.01"}]"#))
        .unwrap_err();
    assert!(matches!(e, ErroreApplicazione::NuoviErrori { .. }), "{e}");
    // Opzioni del certificato su un SAL in bozza; rimozione di IVA con null su progetto nuovo.
    s.esegui(&comandi(
        r#"[{"comando": "registra", "data": "2026-05-10", "codice": "A.01.001", "righi": [{"lunghezza": 1}]},
            {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-05-31"},
            {"comando": "imposta_sal", "numero": 3, "recupera_residuo_anticipazione": true}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let id = p.contabilita.sal()[2].id;
    assert_eq!(
        p.calcola_certificato(id).unwrap().anticipazione_residua,
        Importo::new(dec!(0))
    );
    let mut n = Sessione::nuovo("Altro");
    n.esegui(&comandi(r#"[{"comando": "imposta_contratto", "aliquota_iva": "22"}, {"comando": "imposta_contratto", "aliquota_iva": null}]"#)).unwrap();
    assert_eq!(
        n.progetto()
            .contratto
            .as_ref()
            .unwrap()
            .parametri
            .aliquota_iva,
        None
    );
    let e = n
        .esegui(&comandi(r#"[{"comando": "imposta_anticipazione"}]"#))
        .unwrap_err();
    assert!(e.to_string().contains("importo contrattuale"), "{e}");
}

#[test]
fn conto_finale_firma_e_collaudo_con_i_comandi() {
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    // Stessi dati di `progetto_per_conto_finale` nei test del core.
    s.esegui(&comandi(
        r#"[{"comando": "imposta_contratto", "ribasso": "12.5", "ritenuta_garanzia": "0.5", "importo_contrattuale": "13000.00", "aliquota_iva": "10"},
            {"comando": "imposta_anticipazione", "percentuale_recupero": "20"},
            {"comando": "registra", "data": "2026-03-10", "codice": "A.01.001", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1.2"}]},
            {"comando": "registra", "data": "2026-03-20", "codice": "B.02.010", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "0.5"}]},
            {"comando": "registra", "data": "2026-03-25", "codice": "S.01.001", "righi": [{"lunghezza": 100}]},
            {"comando": "registra", "data": "2026-04-15", "codice": "C.03.020", "righi": [{"lunghezza": 24, "larghezza": 3}]},
            {"comando": "registra_penale", "data": "2026-04-20", "descrizione": "Ritardo", "giorni": 3, "per_mille": "0.5"},
            {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-03-31"},
            {"comando": "emetti_sal", "numero": 1, "data_emissione": "2026-04-05"},
            {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-04-30"},
            {"comando": "emetti_sal", "numero": 2, "data_emissione": "2026-05-06"},
            {"comando": "registra", "data": "2026-05-20", "codice": "C.03.020", "righi": [{"lunghezza": 10, "larghezza": 3}]},
            {"comando": "registra_penale", "data": "2026-05-25", "descrizione": "Ritardo nell'ultimazione", "giorni": 4, "per_mille": "1"}]"#,
    ))
    .unwrap();
    // Prima redazione senza data di ultimazione: rifiutata.
    let e = s
        .esegui(&comandi(r#"[{"comando": "redigi_conto_finale"}]"#))
        .unwrap_err();
    assert!(e.to_string().contains("ultimazione"), "{e}");
    s.esegui(&comandi(
        r#"[{"comando": "redigi_conto_finale", "data_ultimazione": "2026-05-25"},
            {"comando": "redigi_conto_finale", "data": "2026-06-15", "note": "Nessuna variante"}]"#,
    ))
    .unwrap();
    let k = s.progetto().contabilita.conto_finale().unwrap().clone();
    assert_eq!(k.data.to_string(), "2026-06-15");
    assert_eq!(k.note.as_deref(), Some("Nessuna variante"));
    assert_eq!(
        s.progetto().calcola_conto_finale().unwrap().totale,
        Importo::new(dec!(764.48))
    );
    // Con il conto in bozza non si redigono altri SAL.
    assert!(
        s.esegui(&comandi(
            r#"[{"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-05-22"}]"#
        ))
        .is_err()
    );
    let esito = s
        .esegui(&comandi(r#"[{"comando": "emetti_conto_finale"}]"#))
        .unwrap();
    assert!(
        esito.comandi[0].descrizione.contains("764,48"),
        "{:?}",
        esito.comandi
    );
    // Contabilità chiusa.
    for c in [
        r#"[{"comando": "registra", "data": "2026-05-21", "codice": "A.01.001", "righi": [{"lunghezza": 1}]}]"#,
        r#"[{"comando": "rimuovi_penale", "numero": 2}]"#,
        r#"[{"comando": "annulla_conto_finale"}]"#,
    ] {
        let e = s.esegui(&comandi(c)).unwrap_err();
        assert!(e.to_string().contains("chiusa"), "{c}: {e}");
    }
    // IVA modificata dopo l'emissione: rifiutata perché altera il conto congelato.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_contratto", "aliquota_iva": "22"}]"#,
        ))
        .unwrap_err();
    let ErroreApplicazione::NuoviErrori { problemi } = &e else {
        panic!("{e}")
    };
    assert!(
        problemi
            .iter()
            .any(|p| p.codice == "contabilita.conto_finale_alterato")
    );
    s.esegui(&comandi(
        r#"[{"comando": "firma_conto_finale", "data": "2026-06-30", "riserve": "Riserva n. 1 confermata"},
            {"comando": "registra_collaudo", "tipo": "regolare_esecuzione", "data": "2026-07-20"},
            {"comando": "registra_collaudo", "approvato_il": "2026-07-31"}]"#,
    ))
    .unwrap();
    let k = s.progetto().contabilita.conto_finale().unwrap();
    let collaudo = k.collaudo.clone().unwrap();
    assert_eq!(collaudo.data.to_string(), "2026-07-20");
    assert_eq!(collaudo.approvato_il.unwrap().to_string(), "2026-07-31");
    assert!(!valida_progetto(s.progetto()).ha_errori());

    // Report del conto finale e sommario con la colonna dello stato finale.
    let o = OpzioniReport::default();
    let pdf = casi_uso::report(
        s.progetto(),
        &TipoReport::ContoFinale { tradizionale: None },
        FormatoReport::Pdf,
        &o,
    )
    .unwrap();
    assert_eq!(pdf[0].nome, "conto_finale.pdf");
    assert!(pdf[0].byte.starts_with(b"%PDF"));
    let (_, doc) = casi_uso::documento(
        s.progetto(),
        &TipoReport::Sommario { tradizionale: None },
        &o,
    )
    .unwrap();
    let intestazioni: Vec<&str> = doc.sezioni[0]
        .tabella
        .colonne
        .iter()
        .map(|c| c.titolo.as_str())
        .collect();
    assert!(
        intestazioni.contains(&"Stato finale importo €"),
        "{intestazioni:?}"
    );

    // Annulla (una richiesta per passo): firma e collaudo, poi l'emissione.
    s.annulla().unwrap();
    assert!(s.progetto().contabilita.chiusa());
    assert!(
        s.progetto()
            .contabilita
            .conto_finale()
            .unwrap()
            .collaudo
            .is_none()
    );
    s.annulla().unwrap();
    assert!(!s.progetto().contabilita.chiusa());
}

/// Risorse e analisi del test `analisi_prezzi` del core (malta 180,90 €, muratura con la
/// malta come sotto-analisi 110,59 €), più due nuovi prezzi nella contabilità.
const ANALISI: &str = r#"[
  {"comando": "imposta_risorsa", "codice": "MA.01", "descrizione": "Cemento", "unita_misura": "q", "tipo": "materiale", "prezzo": "15.00"},
  {"comando": "imposta_risorsa", "codice": "MA.02", "descrizione": "Sabbia", "unita_misura": "m³", "tipo": "materiale", "prezzo": "28.00"},
  {"comando": "imposta_risorsa", "codice": "MO.01", "descrizione": "Operaio qualificato", "unita_misura": "h", "tipo": "manodopera", "prezzo": "35.50"},
  {"comando": "imposta_risorsa", "codice": "MO.02", "descrizione": "Operaio comune", "unita_misura": "h", "tipo": "manodopera", "prezzo": "32.00"},
  {"comando": "imposta_risorsa", "codice": "NO.01", "descrizione": "Betoniera", "unita_misura": "h", "tipo": "nolo", "prezzo": "4.00"},
  {"comando": "imposta_risorsa", "codice": "TR.01", "descrizione": "Autocarro", "unita_misura": "h", "tipo": "trasporto", "prezzo": "50.00"},
  {"comando": "aggiungi_voce_prezzario", "codice": "MAT.01", "descrizione_breve": "Mattoni pieni", "unita_misura": "cad", "prezzo": "0.45"},
  {"comando": "aggiungi_voce_prezzario", "codice": "NP.MALTA", "descrizione_breve": "Malta cementizia", "unita_misura": "m³", "prezzo": "0"},
  {"comando": "aggiungi_voce_prezzario", "codice": "NP.MUR", "descrizione_breve": "Muratura in mattoni pieni", "unita_misura": "m²", "prezzo": "0"},
  {"comando": "imposta_analisi", "voce": "NP.MALTA", "righe": [
    {"risorsa": "MA.01", "quantita": 3}, {"risorsa": "MA.02", "quantita": 1},
    {"risorsa": "MO.02", "quantita": 2}, {"risorsa": "NO.01", "quantita": "1,5"}]},
  {"comando": "imposta_analisi", "voce": "NP.MUR", "righe": [
    {"voce": "NP.MALTA", "quantita": "0,03"}, {"voce": "MAT.01", "quantita": "6*10"},
    {"risorsa": "MO.01", "quantita": "0,8"}, {"risorsa": "MO.02", "quantita": "0,8"},
    {"risorsa": "TR.01", "quantita": "0,02"}]}
]"#;

#[test]
fn analisi_e_nuovi_prezzi_con_i_comandi() {
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    let esito = s.esegui(&comandi(ANALISI)).unwrap();
    assert!(
        esito.comandi[9].descrizione.contains("180,90"),
        "{:?}",
        esito.comandi
    );
    assert!(
        esito.comandi[10].descrizione.contains("110,59"),
        "{:?}",
        esito.comandi
    );
    let p = s.progetto();
    let mur = p.elenco_prezzi.voce_per_codice("NP.MUR").unwrap();
    assert_eq!(mur.prezzo.to_string(), "110.59");
    assert_eq!(mur.incidenza_manodopera.unwrap().to_string(), "50.57");
    let a = p.elenco_prezzi.analisi(mur.analisi.unwrap()).unwrap();
    assert_eq!(a.righe[1].espressione.as_deref(), Some("6*10"));
    assert_eq!(a.spese_generali.to_string(), "15");

    // Una risorsa usata non si rimuove; il nuovo prezzo si applica con il ricalcolo.
    assert!(
        s.esegui(&comandi(
            r#"[{"comando": "rimuovi_risorsa", "codice": "MA.01"}]"#
        ))
        .is_err()
    );
    let esito = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_risorsa", "codice": "MA.01", "prezzo": "16.00"},
                {"comando": "ricalcola_prezzi_analisi"}]"#,
        ))
        .unwrap();
    assert!(
        esito.comandi[1].descrizione.contains("2 voci"),
        "{:?}",
        esito.comandi
    );
    let p = s.progetto();
    assert_eq!(
        p.elenco_prezzi
            .voce_per_codice("NP.MUR")
            .unwrap()
            .prezzo
            .to_string(),
        "110.74"
    );
    // Analisi ciclica: rifiutata.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_analisi", "voce": "NP.MALTA", "righe": [{"voce": "NP.MUR", "quantita": 1}]}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("ciclica"), "{e}");

    // Nuovi prezzi e verbale; la muratura è già netta (esclusa dal ribasso).
    s.esegui(&comandi(
        r#"[{"comando": "imposta_contratto", "ribasso": "10", "ritenuta_garanzia": "0.5"},
            {"comando": "imposta_nuovo_prezzo", "voce": "NP.MALTA", "data": "2026-04-08", "motivo": "Variante"},
            {"comando": "imposta_nuovo_prezzo", "voce": "NP.MUR", "data": "2026-04-08", "criterio": "analisi"},
            {"comando": "registra", "data": "2026-04-20", "codice": "NP.MUR", "righi": [{"lunghezza": 10}]},
            {"comando": "imposta_verbale_nuovi_prezzi", "data": "2026-04-10", "nuovi_prezzi": ["NP.MALTA", "NP.MUR"]}]"#,
    ))
    .unwrap();
    let np = viste::nuovi_prezzi(s.progetto());
    assert_eq!(np.nuovi_prezzi.len(), 2);
    assert_eq!(np.nuovi_prezzi[0].sigla, "NP.1");
    assert_eq!(
        np.nuovi_prezzi[0].criterio,
        opencomputo_core::prezzario::CriterioNuovoPrezzo::Analisi
    );
    assert_eq!(np.nuovi_prezzi[1].registrazioni, 1);
    assert_eq!(np.verbali[0].nuovi_prezzi, vec!["NP.MALTA", "NP.MUR"]);
    let r = |s: &Sessione| {
        s.progetto().contabilita.libretto().registrazioni[0]
            .prezzo
            .escluso_dal_ribasso
    };
    assert!(!r(&s));
    let esito = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_nuovo_prezzo", "voce": "NP.MUR", "soggetto_a_ribasso": false}]"#,
        ))
        .unwrap();
    assert!(
        esito.comandi[0].descrizione.contains("1 registrazioni"),
        "{:?}",
        esito.comandi
    );
    assert!(r(&s));
    s.esegui(&comandi(
        r#"[{"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-04-30"}]"#,
    ))
    .unwrap();
    let id = s.progetto().contabilita.sal()[0].id;
    let sal = s.progetto().calcola_sal(id).unwrap();
    // 10 m² × 110,74 = 1.107,40 interamente esclusi dal ribasso.
    assert_eq!(sal.lavori_esclusi_dal_ribasso.to_string(), "1107.40");
    assert!(sal.ribasso.is_zero());
    // Approvazione con atto, poi l'approvazione impedisce di togliere il verbale.
    s.esegui(&comandi(
        r#"[{"comando": "imposta_verbale_nuovi_prezzi", "numero": 1, "firma_esecutore": "2026-04-12", "approvato_il": "2026-04-15", "atto_approvazione": "Determina RUP n. 12"}]"#,
    ))
    .unwrap();
    assert!(
        s.esegui(&comandi(
            r#"[{"comando": "rimuovi_verbale_nuovi_prezzi", "numero": 1}]"#
        ))
        .is_err()
    );
    assert!(
        s.esegui(&comandi(
            r#"[{"comando": "imposta_nuovo_prezzo", "voce": "NP.MUR", "rimuovi": true}]"#
        ))
        .is_err()
    );

    // Report.
    let o = OpzioniReport::default();
    for (tipo, nome) in [
        (
            TipoReport::AnalisiPrezzi {
                voce: None,
                sintetica: false,
                tradizionale: None,
            },
            "analisi_prezzi.pdf",
        ),
        (
            TipoReport::AnalisiPrezzi {
                voce: Some("NP.MUR".into()),
                sintetica: true,
                tradizionale: None,
            },
            "analisi_prezzi_sintetica.pdf",
        ),
        (
            TipoReport::NuoviPrezzi {
                verbale: None,
                tradizionale: None,
            },
            "nuovi_prezzi.pdf",
        ),
        (
            TipoReport::NuoviPrezzi {
                verbale: Some(1),
                tradizionale: None,
            },
            "verbale_nuovi_prezzi_1.pdf",
        ),
    ] {
        let f = casi_uso::report(s.progetto(), &tipo, FormatoReport::Pdf, &o).unwrap();
        assert_eq!(f[0].nome, nome);
        assert!(f[0].byte.starts_with(b"%PDF"));
    }
    let (_, verbale) = casi_uso::documento(
        s.progetto(),
        &TipoReport::NuoviPrezzi {
            verbale: Some(1),
            tradizionale: None,
        },
        &o,
    )
    .unwrap();
    let titoli: Vec<&str> = verbale.sezioni.iter().map(|x| x.titolo.as_str()).collect();
    assert_eq!(
        titoli,
        [
            "Nuovi prezzi concordati",
            "Firme",
            "Analisi del prezzo NP.1 NP.MALTA — Malta cementizia",
            "Analisi del prezzo NP.2 NP.MUR — Muratura in mattoni pieni",
        ]
    );
    assert!(
        verbale.sezioni[1]
            .note
            .join(" ")
            .contains("Determina RUP n. 12")
    );
}

#[test]
fn variabili_aggiornamento_prezzi_e_computo_con_analisi() {
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    s.esegui(&comandi(ANALISI)).unwrap();
    // Variabili e quantità dalle righe precedenti (valori del test `analisi` del core:
    // prezzo 11,30, oneri aziendali della sicurezza 0,27).
    s.esegui(&comandi(
        r#"[{"comando": "aggiungi_voce_prezzario", "codice": "NP.INT", "descrizione_breve": "Intonaco", "unita_misura": "m²", "prezzo": "0"},
            {"comando": "imposta_analisi", "voce": "NP.INT", "quota_oneri_sicurezza": "20",
             "variabili": [{"nome": "spessore", "valore": "0,015"}, {"nome": "kg", "valore": "spessore*1400"}],
             "righe": [{"risorsa": "MO.02", "quantita": "0,02+spessore*10"},
                       {"risorsa": "MA.01", "quantita": "kg/100"},
                       {"risorsa": "NO.01", "quantita": "q1/2"}]}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let v = p.elenco_prezzi.voce_per_codice("NP.INT").unwrap();
    assert_eq!(v.prezzo.to_string(), "11.30");
    let a = p.elenco_prezzi.analisi(v.analisi.unwrap()).unwrap();
    assert_eq!(a.righe[2].quantita.to_string(), "0.085");
    let d = viste::voce(&p.elenco_prezzi, "NP.INT", &p.regole).unwrap();
    let va = d.analisi.unwrap();
    assert_eq!(va.variabili[1].valore.unwrap().to_string(), "21.000");
    assert_eq!(
        va.risultato
            .unwrap()
            .oneri_sicurezza_aziendali
            .unwrap()
            .to_string(),
        "0.27"
    );
    // Variabile sconosciuta: rifiutata.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "imposta_analisi", "voce": "NP.INT", "righe": [{"risorsa": "MO.02", "quantita": "altezza*2"}]}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("altezza"), "{e}");

    // Aggiornamento del 10 % delle sole risorse di manodopera: 32,00 → 35,20; le voci
    // analizzate seguono.
    let esito = s
        .esegui(&comandi(
            r#"[{"comando": "aggiorna_prezzi", "oggetto": "risorse", "tipo": "manodopera", "percentuale": "10"}]"#,
        ))
        .unwrap();
    assert!(
        esito.comandi[0].descrizione.contains("2 risorse"),
        "{:?}",
        esito.comandi
    );
    let p = s.progetto();
    let mo = p
        .elenco_prezzi
        .risorse
        .iter()
        .find(|r| r.codice == "MO.02")
        .unwrap();
    assert_eq!(mo.prezzo.to_string(), "35.20");
    assert!(
        p.elenco_prezzi
            .prezzi_non_allineati(&p.regole)
            .unwrap()
            .is_empty()
    );
    // Voci: quelle analizzate non sono toccate; una diminuzione del 100 % è rifiutata.
    let esito = s
        .esegui(&comandi(
            r#"[{"comando": "aggiorna_prezzi", "oggetto": "voci", "codici": ["A.01.001", "NP.MUR"], "percentuale": "-2"}]"#,
        ))
        .unwrap();
    assert!(
        esito.comandi[0].descrizione.contains("1 voci"),
        "{:?}",
        esito.comandi
    );
    assert!(
        esito.comandi[0].descrizione.contains("1 voci analizzate"),
        "{:?}",
        esito.comandi
    );
    assert_eq!(
        s.progetto()
            .elenco_prezzi
            .voce_per_codice("A.01.001")
            .unwrap()
            .prezzo
            .to_string(),
        "8.33"
    );
    assert!(
        s.esegui(&comandi(
            r#"[{"comando": "aggiorna_prezzi", "oggetto": "voci", "percentuale": "-100"}]"#
        ))
        .is_err()
    );

    // Computo con le analisi.
    s.esegui(&comandi(
        r#"[{"comando": "aggiungi_voce_computo", "codice": "NP.MUR", "righi": [{"lunghezza": 100}]},
            {"comando": "aggiungi_voce_computo", "codice": "A.01.001", "righi": [{"lunghezza": 10}]}]"#,
    ))
    .unwrap();
    let o = OpzioniReport::default();
    let tipo = TipoReport::Computo {
        computo: None,
        con_analisi: true,
        tradizionale: None,
        incidenze: Default::default(),
    };
    let f = casi_uso::report(s.progetto(), &tipo, FormatoReport::Pdf, &o).unwrap();
    assert_eq!(f[0].nome, "computo_con_analisi.pdf");
    // Il computo con le analisi non stampa le incidenze.
    let con_incidenze: TipoReport = serde_json::from_str(
        r#"{"tipo": "computo", "con_analisi": true, "incidenze": {"manodopera": true}}"#,
    )
    .unwrap();
    let e = casi_uso::report(s.progetto(), &con_incidenze, FormatoReport::Pdf, &o).unwrap_err();
    assert!(e.to_string().contains("non stampa le incidenze"), "{e}");
    let (_, d) = casi_uso::documento(s.progetto(), &tipo, &o).unwrap();
    let testi: Vec<String> = d.sezioni[0]
        .tabella
        .righe
        .iter()
        .map(|r| r.celle[2].in_testo())
        .collect();
    assert!(testi.contains(&"Spese generali".to_string()), "{testi:?}");
    assert!(
        testi.contains(&"voce senza analisi".to_string()),
        "{testi:?}"
    );
}

#[test]
fn copia_e_sposta_voci_tra_computi() {
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    s.esegui(&comandi(COMPUTO)).unwrap();
    let principale = s.progetto().computi[0].nome.clone();
    s.esegui(&comandi(
        r#"[{"comando": "nuovo_computo", "nome": "Variante"}]"#,
    ))
    .unwrap();
    let codici = |s: &Sessione, nome: &str| -> Vec<String> {
        s.progetto()
            .computi
            .iter()
            .find(|c| c.nome == nome)
            .unwrap()
            .voci
            .iter()
            .map(|v| v.prezzo.codice.clone())
            .collect()
    };
    let totale_di = |s: &Sessione, nome: &str| {
        let id = s
            .progetto()
            .computi
            .iter()
            .find(|c| c.nome == nome)
            .unwrap()
            .id;
        s.progetto().calcola_computo(id).unwrap().totale
    };
    let prima = totale_di(&s, &principale);

    // Copia delle voci 3 e 1 (in ordine di computo) in testa alla variante.
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "copia_voci_computo", "da": "{principale}", "voci": [3, 1], "a": "Variante"}}]"#
    )))
    .unwrap();
    assert_eq!(codici(&s, "Variante"), ["A.01.001", "C.03.020"]);
    assert_eq!(codici(&s, &principale).len(), 4);
    assert_eq!(totale_di(&s, &principale), prima);
    let p = s.progetto();
    let orig = &p.computi[0].voci[2];
    let copia = &p
        .computi
        .iter()
        .find(|c| c.nome == "Variante")
        .unwrap()
        .voci[1];
    assert_ne!(orig.id, copia.id);
    assert_eq!(orig.righi.len(), copia.righi.len());
    assert!(
        orig.righi
            .iter()
            .zip(&copia.righi)
            .all(|(a, b)| a.id != b.id && a.descrizione == b.descrizione)
    );
    assert_eq!(orig.prezzo, copia.prezzo);
    // 1.071,00 + 4.668,30.
    assert_eq!(totale_di(&s, "Variante").valore(), dec!(5739.30));
    assert!(!valida_progetto(s.progetto()).ha_errori());

    // Spostamento nello stesso computo: voci 1 e 2 prima della 4 → 3, 1, 2, 4.
    let id_1 = s.progetto().computi[0].voci[0].id;
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "copia_voci_computo", "da": "{principale}", "voci": [1, 2], "posizione": 4, "sposta": true}}]"#
    )))
    .unwrap();
    assert_eq!(
        codici(&s, &principale),
        ["C.03.020", "A.01.001", "B.02.010", "S.01.001"]
    );
    assert_eq!(
        s.progetto().computi[0].voci[1].id,
        id_1,
        "lo spostamento conserva l'identificativo"
    );
    assert_eq!(totale_di(&s, &principale), prima);

    // Spostamento in un altro computo, in coda; poi annulla in un passo.
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "copia_voci_computo", "da": "{principale}", "voci": [4], "a": "Variante", "sposta": true}}]"#
    )))
    .unwrap();
    assert_eq!(codici(&s, &principale).len(), 3);
    assert_eq!(codici(&s, "Variante"), ["A.01.001", "C.03.020", "S.01.001"]);
    s.annulla().unwrap();
    assert_eq!(codici(&s, &principale).len(), 4);
    assert_eq!(codici(&s, "Variante").len(), 2);

    // Errori: posizione oltre la fine, voce inesistente, elenco vuoto.
    for c in [
        r#"{"comando": "copia_voci_computo", "da": "Variante", "voci": [1], "posizione": 9}"#,
        r#"{"comando": "copia_voci_computo", "da": "Variante", "voci": [7]}"#,
        r#"{"comando": "copia_voci_computo", "da": "Variante", "voci": []}"#,
    ] {
        assert!(s.esegui(&comandi(&format!("[{c}]"))).is_err(), "{c}");
    }
}

#[test]
fn vedi_voce_collegato_statico_e_conservato() {
    use opencomputo_core::misura::VediVoce;
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(ELENCO)).unwrap();
    s.esegui(&comandi(COMPUTO)).unwrap();
    let principale = s.progetto().computi[0].nome.clone();
    let quantita = |s: &Sessione, nome: &str, n: usize| {
        let p = s.progetto();
        let c = p.computi.iter().find(|c| c.nome == nome).unwrap();
        c.quantita_voce(c.voci[n - 1].id, &p.regole)
            .unwrap()
            .valore()
    };
    let vedi = |s: &Sessione, nome: &str, n: usize, r: usize| {
        let c = s
            .progetto()
            .computi
            .iter()
            .find(|c| c.nome == nome)
            .unwrap();
        c.voci[n - 1].righi[r - 1].vedi_voce.clone()
    };

    // Due righi «vedi voce» nella voce 4: metà della voce 1, e la voce 2 in deduzione.
    s.esegui(&comandi(
        r#"[{"comando": "aggiungi_rigo", "voce": 4, "rigo": {"descrizione": "Metà scavo", "vedi_voce": 1, "parti_uguali": "0,5"}},
            {"comando": "aggiungi_rigo", "voce": 4, "rigo": {"vedi_voce": 2, "deduzione": true}}]"#,
    ))
    .unwrap();
    // 100 + 63 − 51,74
    assert_eq!(quantita(&s, &principale, 4), dec!(111.26));
    let id_1 = s.progetto().computi[0].voci[0].id;
    assert_eq!(vedi(&s, &principale, 4, 2), Some(VediVoce::Collegata(id_1)));
    // Cambia la voce 1, cambia la 4.
    s.esegui(&comandi(
        r#"[{"comando": "modifica_rigo", "voce": 1, "numero": 1, "rigo": {"lunghezza": 12, "larghezza": 8, "altezza_peso": "1.70"}}]"#,
    ))
    .unwrap();
    // voce 1: 163,20 + 10,80 = 174,00 → metà 87,00; 100 + 87 − 51,74
    assert_eq!(quantita(&s, &principale, 4), dec!(135.26));

    // Riferimento circolare e voce inesistente: rifiutati.
    for c in [
        r#"{"comando": "aggiungi_rigo", "voce": 1, "rigo": {"vedi_voce": 4}}"#,
        r#"{"comando": "aggiungi_rigo", "voce": 2, "rigo": {"vedi_voce": 2}}"#,
        r#"{"comando": "aggiungi_rigo", "voce": 2, "rigo": {"vedi_voce": 9}}"#,
    ] {
        let e = s.esegui(&comandi(&format!("[{c}]"))).unwrap_err();
        assert!(!e.to_string().is_empty(), "{c}");
    }
    assert_eq!(s.progetto().computi[0].voci[0].righi.len(), 2);

    // Spostare la voce citata non rompe il collegamento.
    s.esegui(&comandi(
        r#"[{"comando": "sposta_voce_computo", "voce": 1, "posizione": 3}]"#,
    ))
    .unwrap();
    assert_eq!(quantita(&s, &principale, 4), dec!(135.26));

    // Copia nello stesso computo della voce 4 con la voce citata (ora n. 3): il
    // collegamento della copia segue la copia della voce citata; quello verso la voce 2
    // (non copiata) resta.
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "copia_voci_computo", "da": "{principale}", "voci": [3, 4]}}]"#
    )))
    .unwrap();
    let c = &s.progetto().computi[0];
    let (copia_citata, copia) = (&c.voci[4], &c.voci[5]);
    assert_eq!(
        copia.righi[1].vedi_voce,
        Some(VediVoce::Collegata(copia_citata.id))
    );
    assert_eq!(
        copia.righi[2].vedi_voce,
        Some(VediVoce::Collegata(c.voci[0].id))
    );
    assert_eq!(quantita(&s, &principale, 6), dec!(135.26));
    s.annulla().unwrap();

    // Copia in un altro computo della sola voce 4: i collegamenti diventano statici.
    s.esegui(&comandi(
        r#"[{"comando": "nuovo_computo", "nome": "Variante"}]"#,
    ))
    .unwrap();
    let esito = s
        .esegui(&comandi(&format!(
            r#"[{{"comando": "copia_voci_computo", "da": "{principale}", "voci": [4], "a": "Variante"}}]"#
        )))
        .unwrap();
    assert!(
        esito.comandi[0]
            .descrizione
            .contains("2 righi «vedi voce» resi statici"),
        "{esito:?}"
    );
    assert_eq!(
        vedi(&s, "Variante", 1, 2),
        Some(VediVoce::Statica {
            quantita: dec!(174.00),
            riferimento: format!("voce n. 3 «A.01.001» del computo «{principale}»"),
        })
    );
    assert_eq!(quantita(&s, "Variante", 1), dec!(135.26));

    // Togliere la voce citata (il calcestruzzo, ora n. 1): i righi che la citano
    // diventano statici.
    let esito = s
        .esegui(&comandi(&format!(
            r#"[{{"comando": "rimuovi_voce_computo", "computo": "{principale}", "voce": 1}}]"#
        )))
        .unwrap();
    assert!(
        esito.comandi[0]
            .descrizione
            .contains("1 rigo «vedi voce» reso statico")
    );
    assert!(matches!(
        vedi(&s, &principale, 3, 3),
        Some(VediVoce::Statica { quantita, .. }) if quantita == dec!(51.74)
    ));
    assert_eq!(quantita(&s, &principale, 3), dec!(135.26));
    assert!(!valida_progetto(s.progetto()).ha_errori());

    // Libretto: il «vedi voce» copia la quantità attuale della voce indicata.
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "registra", "data": "2026-03-10", "codice": "A.01.001", "righi": [
            {{"vedi_voce": {{"computo": "{principale}", "voce": 2}}, "formula": "vv/2"}}]}}]"#
    )))
    .unwrap();
    let reg = &s.progetto().contabilita.libretto().registrazioni[0];
    assert!(
        matches!(&reg.righi[0].vedi_voce, Some(VediVoce::Statica { quantita, .. }) if *quantita == dec!(174.00))
    );
    assert_eq!(
        reg.calcola(&s.progetto().regole).unwrap().quantita.valore(),
        dec!(87.00)
    );
    // Un rigo collegato copiato altrove (identificativo di una voce di un altro computo,
    // o nel libretto) diventa statico.
    let id_a = s.progetto().computi[0].voci[1].id;
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "aggiungi_rigo", "computo": "Variante", "voce": 1, "rigo": {{"vedi_voce": "{id_a}"}}}},
            {{"comando": "registra", "data": "2026-03-12", "codice": "A.01.001", "righi": [{{"vedi_voce": "{id_a}"}}]}}]"#
    )))
    .unwrap();
    assert!(matches!(
        vedi(&s, "Variante", 1, 4),
        Some(VediVoce::Statica { quantita, .. }) if quantita == dec!(174.00)
    ));
    assert!(matches!(
        &s.progetto().contabilita.libretto().registrazioni[1].righi[0].vedi_voce,
        Some(VediVoce::Statica { quantita, .. }) if *quantita == dec!(174.00)
    ));
    // Con più computi il numero da solo è ambiguo nel libretto.
    assert!(
        s.esegui(&comandi(
            r#"[{"comando": "registra", "data": "2026-03-11", "codice": "A.01.001", "righi": [{"vedi_voce": 1}]}]"#
        ))
        .is_err()
    );
}

#[test]
fn viste_con_i_dati_per_i_filtri() {
    use opencomputo_application::viste;
    let mut s = Sessione::nuovo("Prova");
    s.esegui(&comandi(
        r#"[{"comando": "aggiungi_capitolo", "codice": "A", "descrizione": "Scavi"},
            {"comando": "aggiungi_capitolo", "codice": "A.01", "descrizione": "Sbancamenti", "padre": "A"},
            {"comando": "aggiungi_voce_prezzario", "codice": "A.01.001", "capitolo": "A.01", "descrizione_breve": "Scavo", "unita_misura": "m³", "prezzo": "8.50"},
            {"comando": "aggiungi_voce_prezzario", "codice": "B.01", "descrizione_breve": "Getto", "unita_misura": "m³", "prezzo": "100"},
            {"comando": "aggiungi_voce_computo", "codice": "A.01.001", "righi": [{"lunghezza": 2}]},
            {"comando": "aggiungi_voce_computo", "codice": "B.01", "righi": [{"lunghezza": 1}]}]"#,
    ))
    .unwrap();
    let p = s.progetto();
    let e = viste::elenco(&p.elenco_prezzi, None);
    assert_eq!(e.capitoli.len(), 2);
    assert_eq!(e.capitoli[1].padre.as_deref(), Some("A"));
    assert_eq!(e.voci[0].capitolo.as_deref(), Some("A.01"));
    let c = viste::computo(p, None).unwrap();
    assert_eq!(c.voci[0].capitolo.as_deref(), Some("A.01"));
    assert_eq!(c.voci[1].capitolo, None);
    assert_eq!(c.voci[0].id, p.computi[0].voci[0].id);
}

/// Lo stesso report con l'incidenza della manodopera.
fn tipo_con_incidenze(tipo: &TipoReport) -> TipoReport {
    let mut t = tipo.clone();
    if let TipoReport::Computo { incidenze, .. } = &mut t {
        incidenze.manodopera = true;
    }
    t
}

#[test]
fn aggiornamento_da_un_nuovo_listino_con_confronto_e_prova() {
    let mut s = Sessione::new(
        importa_json(include_str!(
            "../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        ))
        .unwrap(),
    );
    // Nuovo listino: lo stesso elenco con i codici «2027.» e due prezzi cambiati.
    let mut listino = s.progetto().elenco_prezzi.clone();
    listino.nome = "Regionale".into();
    listino.edizione = Some("2027".into());
    for v in listino.voci.iter_mut().map(Arc::make_mut) {
        v.codice = format!("2027.{}", v.codice);
    }
    Arc::make_mut(&mut listino.voci[0]).prezzo = opencomputo_core::numeri::Prezzo::new(dec!(9.00));
    Arc::make_mut(&mut listino.voci[1]).prezzo =
        opencomputo_core::numeri::Prezzo::new(dec!(150.00));
    s.carica_prezzario("regionale 2027", listino.clone());
    let codice_scavo = s.progetto().elenco_prezzi.voci[0].codice.clone();

    // Confronto: senza prefisso nessuna voce si trova; con il prefisso due variate.
    let p = s.progetto();
    let senza = viste::confronto_prezzario(p, "regionale 2027", &listino, None);
    assert!(senza.voci.iter().all(|v| v.confronto.listino.is_none()));
    assert_eq!(senza.voci_solo_nel_listino, listino.voci.len());
    let prefisso = opencomputo_core::aggiornamento::SostituzionePrefisso {
        da: String::new(),
        a: "2027.".into(),
    };
    // Un prefisso «da» vuoto non sostituisce nulla.
    let vuoto = viste::confronto_prezzario(p, "regionale 2027", &listino, Some(&prefisso));
    assert!(vuoto.voci.iter().all(|v| v.confronto.listino.is_none()));
    let json = serde_json::to_value(&senza).unwrap();
    assert_eq!(json["fonte"], "Regionale (2027)");
    assert!(json["voci"][0]["capitolo"].is_string() || json["voci"][0]["capitolo"].is_null());
    assert_eq!(json["voci"][0]["esito"], "non_trovata");

    let comando = |voci: &str| {
        comandi(&format!(
            r#"[{{"comando": "aggiorna_da_prezzario", "prezzario": "regionale 2027", "voci": {voci}, "prefisso": {{"da": "", "a": ""}}}}]"#
        ))
    };
    // Senza prefisso valido il codice non è nel listino.
    let e = s
        .prova(&comando(&format!(r#"["{codice_scavo}"]"#)))
        .unwrap_err();
    assert!(e.to_string().contains("non trovato nel listino"), "{e}");

    // La prova mostra l'effetto sul totale senza applicare.
    let mut lettere = codice_scavo.chars();
    let primo = lettere.next().unwrap();
    let c = comandi(&format!(
        r#"[{{"comando": "aggiorna_da_prezzario", "prezzario": "regionale 2027", "voci": ["{codice_scavo}"], "prefisso": {{"da": "{primo}", "a": "2027.{primo}"}}}}]"#
    ));
    let prova = s.prova(&c).unwrap();
    assert!(!prova.applicato);
    assert_eq!(prova.totali.len(), 1);
    // Scavo 126,00 m³: da 8,50 a 9,00, +63,00.
    assert_eq!(prova.totali[0].prima, Some(Importo::new(dec!(14457.12))));
    assert_eq!(prova.totali[0].dopo, Some(Importo::new(dec!(14520.12))));
    let esito = s.esegui(&c).unwrap();
    assert!(
        esito.comandi[0]
            .descrizione
            .contains("1 voce aggiornata, 1 voce di computo riallineata"),
        "{}",
        esito.comandi[0].descrizione
    );
    assert_eq!(
        s.progetto().elenco_prezzi.voci[0].prezzo,
        opencomputo_core::numeri::Prezzo::new(dec!(9.00))
    );
    s.annulla().unwrap();
    assert_eq!(
        s.progetto()
            .calcola_computo(s.progetto().computi[0].id)
            .unwrap()
            .totale,
        Importo::new(dec!(14457.12))
    );
}

#[test]
fn modello_tradizionale_per_tutti_i_documenti() {
    let s = Sessione::new(
        importa_json(include_str!(
            "../../opencomputo-core/tests/fixtures/progetto_esempio_v6.json"
        ))
        .unwrap(),
    );
    let o = OpzioniReport::default();
    for (json, nome) in [
        (
            r#"{"tipo": "libretto", "tradizionale": {}}"#,
            "libretto_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "registro", "tradizionale": {}}"#,
            "registro_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "sal", "numero": 2, "tradizionale": {}}"#,
            "sal_2_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "certificato", "numero": 2, "tradizionale": {"frontespizio": false}}"#,
            "certificato_2_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "sommario", "tradizionale": {}}"#,
            "sommario_registro_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "analisi_prezzi", "sintetica": true, "tradizionale": {}}"#,
            "analisi_prezzi_sintetica_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "nuovi_prezzi", "verbale": 1, "tradizionale": {}}"#,
            "verbale_nuovi_prezzi_1_tradizionale.pdf",
        ),
        (
            r#"{"tipo": "computo", "con_analisi": true, "tradizionale": {}}"#,
            "computo_con_analisi_tradizionale.pdf",
        ),
    ] {
        let tipo: TipoReport = serde_json::from_str(json).unwrap();
        assert!(tipo.tradizionale().is_some());
        let f = casi_uso::report(s.progetto(), &tipo, FormatoReport::Pdf, &o)
            .unwrap_or_else(|e| panic!("{json}: {e}"));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].nome, nome);
        assert!(f[0].byte.starts_with(b"%PDF-1.4"));
        // Solo PDF.
        let e = casi_uso::report(s.progetto(), &tipo, FormatoReport::Csv, &o).unwrap_err();
        assert!(e.to_string().contains("solo in PDF"), "{e}");
    }
    // «Senza prezzi» resta del computo; le incidenze restano escluse dal computo con
    // le analisi anche nel modello tradizionale.
    let tipo: TipoReport =
        serde_json::from_str(r#"{"tipo": "registro", "tradizionale": {"senza_prezzi": true}}"#)
            .unwrap();
    let e = casi_uso::report(s.progetto(), &tipo, FormatoReport::Pdf, &o).unwrap_err();
    assert!(e.to_string().contains("senza prezzi"), "{e}");
    let tipo: TipoReport = serde_json::from_str(
        r#"{"tipo": "computo", "con_analisi": true, "tradizionale": {}, "incidenze": {"manodopera": true}}"#,
    )
    .unwrap();
    let e = casi_uso::report(s.progetto(), &tipo, FormatoReport::Pdf, &o).unwrap_err();
    assert!(e.to_string().contains("non stampa le incidenze"), "{e}");
    // Un tipo senza modello tradizionale si serializza come prima.
    let v = serde_json::to_value(TipoReport::Libretto { tradizionale: None }).unwrap();
    assert_eq!(v, serde_json::json!({"tipo": "libretto"}));
}

#[test]
fn cronoprogramma_con_i_comandi() {
    use opencomputo_core::cronoprogramma::RifAttivita;
    let mut s = Sessione::new(
        importa_json(include_str!(
            "../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"
        ))
        .unwrap(),
    );
    // Senza inizio il piano non si calcola, ma la vista risponde.
    let v = viste::cronoprogramma(s.progetto());
    assert!(v.piano.is_none());
    assert!(v.errore.unwrap().contains("inizio dei lavori"));

    s.esegui(&comandi(
        r#"[{"comando": "imposta_cronoprogramma", "inizio_lavori": "2026-03-02"},
            {"comando": "imposta_attivita", "attivita": {"wbs": "01.01"}, "durata": 10},
            {"comando": "imposta_attivita", "attivita": {"wbs": "01.02"}, "durata": 15, "avanzamento": 20},
            {"comando": "imposta_attivita", "attivita": {"wbs": "02"}, "durata": 20},
            {"comando": "aggiungi_attivita", "nome": "Fine dei lavori", "pietra_miliare": true},
            {"comando": "collega_attivita", "da": {"wbs": "01.01"}, "a": {"wbs": "01.02"}},
            {"comando": "collega_attivita", "da": {"wbs": "01.01"}, "a": {"wbs": "02"}, "tipo": "inizio_inizio"}]"#,
    ))
    .unwrap();
    let fine = s.progetto().cronoprogramma.attivita[0].id;
    let e = s
        .esegui(&comandi(&format!(
            r#"[{{"comando": "collega_attivita", "da": {{"wbs": "01.02"}}, "a": {{"attivita": "{fine}"}}}}]"#
        )))
        .unwrap();
    assert!(
        e.comandi[0].descrizione.contains("(FI)"),
        "{}",
        e.comandi[0].descrizione
    );
    let v = viste::cronoprogramma(s.progetto());
    let piano = v.piano.unwrap();
    assert_eq!(piano.fine, data_amg(2026, 4, 3).unwrap());
    assert_eq!(piano.legami.len(), 3);
    let json = serde_json::to_value(viste::cronoprogramma(s.progetto())).unwrap();
    assert_eq!(json["piano"]["righe"][1]["attivita"]["tipo"], "wbs");
    assert_eq!(json["piano"]["righe"][2]["avanzamento"], 20);

    // Ciclo e legame con il riepilogo che contiene l'attività: rifiutati.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "collega_attivita", "da": {"wbs": "01.02"}, "a": {"wbs": "01.01"}}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("ciclo"), "{e}");
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "collega_attivita", "da": {"wbs": "01"}, "a": {"wbs": "01.02"}}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("contiene"), "{e}");
    // Ciclo che compare solo con i riepiloghi: 02 → 01, e 01.01 II 02 già c'è.
    let e = s
        .esegui(&comandi(
            r#"[{"comando": "collega_attivita", "da": {"wbs": "02"}, "a": {"wbs": "01"}}]"#,
        ))
        .unwrap_err();
    assert!(e.to_string().contains("ciclo"), "{e}");

    // Livello di dettaglio e linea di base.
    s.esegui(&comandi(
        r#"[{"comando": "imposta_cronoprogramma", "livello_dettaglio": 1},
            {"comando": "linea_base_cronoprogramma", "data": "2026-02-25"}]"#,
    ))
    .unwrap();
    let c = &s.progetto().cronoprogramma;
    assert_eq!(c.livello_dettaglio, Some(1));
    // La linea di base comprende tutti i livelli, anche quelli nascosti.
    assert_eq!(c.linea_base.as_ref().unwrap().attivita.len(), 5);
    let v = viste::cronoprogramma(s.progetto());
    assert_eq!(v.piano.unwrap().righe.len(), 3);

    // Nodo WBS nuovo collegato e poi rimosso: dati e legami spariscono con lui.
    s.esegui(&comandi(
        r#"[{"comando": "imposta_cronoprogramma", "livello_dettaglio": null},
            {"comando": "aggiungi_nodo_wbs", "codice": "03", "descrizione": "Sistemazioni esterne"},
            {"comando": "imposta_attivita", "attivita": {"wbs": "03"}, "durata": 5},
            {"comando": "collega_attivita", "da": {"wbs": "02"}, "a": {"wbs": "03"}},
            {"comando": "aggiungi_attivita", "nome": "Collaudo", "padre": "03", "durata": 2}]"#,
    ))
    .unwrap();
    assert_eq!(s.progetto().cronoprogramma.legami.len(), 4);
    s.esegui(&comandi(
        r#"[{"comando": "rimuovi_nodo_wbs", "codice": "03"}]"#,
    ))
    .unwrap();
    let c = &s.progetto().cronoprogramma;
    assert_eq!(c.legami.len(), 3);
    assert_eq!(c.attivita_wbs.len(), 3);
    // Le attività aggiunte sotto il nodo passano al padre (qui la radice).
    let collaudo = c.attivita.iter().find(|a| a.nome == "Collaudo").unwrap();
    assert_eq!(collaudo.padre, None);
    assert!(
        c.legami
            .iter()
            .all(|l| l.da != RifAttivita::Wbs(opencomputo_core::id::WbsNodoId::da_u128(0)))
    );

    // Togliere l'attività aggiunta toglie i suoi legami; annulla la rimette.
    s.esegui(&comandi(&format!(
        r#"[{{"comando": "rimuovi_attivita", "attivita": "{fine}"}}]"#
    )))
    .unwrap();
    assert_eq!(s.progetto().cronoprogramma.legami.len(), 2);
    s.annulla().unwrap();
    assert_eq!(s.progetto().cronoprogramma.legami.len(), 3);
    // Il cronoprogramma non entra nell'export XPWE.
    let x = casi_uso::esporta_xpwe(s.progetto(), None).unwrap();
    assert!(!x.xml.contains("Fine dei lavori"));
}

#[test]
fn copia_da_un_listino_csv_e_json() {
    let csv = include_bytes!("../../opencomputo-format-listino/tests/fixtures/listino_esempio.csv");
    let (prezzario, rapporto) = casi_uso::leggi_prezzario(csv, Some("listino.csv")).unwrap();
    assert_eq!(rapporto.formato, "csv");
    assert_eq!(rapporto.voci_elenco, 7);
    assert_eq!(
        rapporto
            .anomalie
            .iter()
            .filter(|a| a.gravita == opencomputo_core::validazione::Gravita::Errore)
            .count(),
        5
    );
    let mut s = Sessione::nuovo("Prova");
    s.carica_prezzario("listino", prezzario);
    s.esegui(&comandi(
        r#"[{"comando": "copia_da_prezzario", "prezzario": "listino", "codici": ["M.01.01"]},
            {"comando": "aggiungi_voce_computo", "codice": "M.01.01", "righi": [{"lunghezza": 10, "altezza_peso": "3"}]}]"#,
    ))
    .unwrap();
    // 10 × 3 = 30 m² × 1.234,56 = 37.036,80 €
    assert_eq!(totale(&s), Importo::new(dec!(37036.80)));

    let json =
        include_bytes!("../../opencomputo-format-listino/tests/fixtures/listino_esempio.json");
    let (prezzario, rapporto) = casi_uso::leggi_prezzario(json, Some("listino.json")).unwrap();
    assert_eq!(rapporto.formato, "json");
    assert_eq!(prezzario.voci.len(), 4);
    assert_eq!(prezzario.nome, "Listino di prova");

    let e = casi_uso::leggi_prezzario(b"Codice;Prezzo\nA;1\n", Some("x.csv")).unwrap_err();
    assert!(matches!(e, ErroreApplicazione::Importazione { .. }));
    assert!(e.to_string().contains("colonne obbligatorie mancanti"));
}
