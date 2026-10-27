//! Workflow B senza chiamate AI: ricerca, candidati, risposte del modello registrate,
//! validazione, prova ed esecuzione nella sessione. I totali attesi sono calcolati a
//! mano (quantità arrotondate a 2 decimali per rigo, importo = quantità × prezzo).
//!
//! Il prezzario `prezzario_prova.csv` è sintetico.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use opencomputo_application::{Comando, Sessione};
use opencomputo_assistente::candidati::Candidati;
use opencomputo_assistente::proposta::{
    Contesto, ErroreRisposta, EsitoProposta, Proposta, ProvenienzaValore, leggi_selezione,
    valida_selezione,
};
use opencomputo_assistente::ricerca::{FiltroVoci, IndiceTestuale};
use opencomputo_assistente::risposta::{LavorazioneAnalisi, RispostaAnalisi, RispostaSelezione};
use opencomputo_assistente::unita::Dimensione;
use opencomputo_assistente::verifica;
use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::numeri::Importo;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::validazione::Gravita;
use opencomputo_format_listino::{OpzioniListino, importa_csv};
use rust_decimal_macros::dec;
use serde_json::{Value, json};

const CSV: &[u8] = include_bytes!("fixtures/prezzario_prova.csv");
const NOME: &str = "prova";

fn prezzario() -> Prezzario {
    importa_csv(
        CSV,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap()
    .prezzario
}

fn sessione(p: &Prezzario) -> Sessione {
    let mut s = Sessione::nuovo("Prova assistente");
    s.carica_prezzario(NOME, p.clone());
    s
}

fn candidati(p: &Prezzario, codici: &[&str]) -> Candidati {
    let v: Vec<(String, Vec<String>)> = codici
        .iter()
        .map(|c| ((*c).to_owned(), Vec::new()))
        .collect();
    Candidati::da_codici(p, &v)
}

fn h(c: &Candidati, codice: &str) -> String {
    c.elenco
        .iter()
        .find(|x| x.codice == codice)
        .map(|x| x.handle.clone())
        .unwrap_or_else(|| panic!("{codice} non tra i candidati"))
}

fn par(nome: &str, valore: &str, unita: &str, provenienza: &str, rif: &str) -> Value {
    json!({"nome": nome, "valore": valore, "unita": unita, "provenienza": provenienza, "rif_ipotesi": rif})
}

fn u(nome: &str, valore: &str, unita: &str) -> Value {
    par(nome, valore, unita, "utente", "")
}

fn riga(descrizione: &str, forma: &str, segno: &str, parametri: Vec<Value>) -> Value {
    json!({"descrizione": descrizione, "forma": forma, "segno": segno, "parametri": parametri})
}

fn lav(id: &str, candidato: &str, ruolo: &str, gruppo: &str, di: &str, righe: Vec<Value>) -> Value {
    json!({
        "id": id, "descrizione": format!("lavorazione {id}"), "candidato": candidato,
        "ruolo": ruolo, "gruppo_alternativa": gruppo, "complementare_di": di,
        "righe": righe, "motivazione": "voce più pertinente"
    })
}

fn risposta(esito: &str, lavorazioni: Vec<Value>, ipotesi: Vec<Value>) -> RispostaSelezione {
    leggi_selezione(&json!({
        "esito": esito, "lavorazioni": lavorazioni, "ipotesi": ipotesi, "informazioni_mancanti": []
    }))
    .unwrap()
}

fn valida(
    s: &Sessione,
    p: &Prezzario,
    c: &Candidati,
    richiesta: &str,
    r: &RispostaSelezione,
) -> Proposta {
    valida_selezione(
        r,
        &Contesto {
            richiesta,
            nome_prezzario: NOME,
            prezzario: p,
            candidati: c,
            progetto: s.progetto(),
            computo: None,
        },
    )
    .unwrap()
}

fn totale(e: &opencomputo_application::Esito) -> Option<Importo> {
    e.totali.first().and_then(|t| t.dopo)
}

fn codici_avvisi(p: &Proposta) -> Vec<String> {
    p.avvisi.iter().map(|a| a.codice.clone()).collect()
}

// ---------------------------------------------------------------- ricerca

#[test]
fn ricerca_trova_le_voci_attese() {
    let p = prezzario();
    let indice = IndiceTestuale::new(&p);
    assert_eq!(indice.len(), 22);
    let casi = [
        ("massetto sabbia cemento", "M.01"),
        ("pavimento in gres", "P.01"),
        ("demolizione pavimento", "D.01"),
        ("rivestimento pareti ceramica", "P.02"),
        ("scavo di sbancamento", "E.01"),
        ("scavo per fondazioni", "E.02"),
        ("tinteggiatura idropittura", "T.01"),
        ("piatto doccia", "I.02"),
        ("calcestruzzo C25/30", "C.01"),
        ("armature in barre di acciaio", "C.02"),
        ("rimozione sanitari", "D.03"),
        // Errori di battitura: termini simili per trigrammi.
        ("masseto sabia", "M.01"),
        ("tingeggiatura", "T.01"),
    ];
    for (domanda, atteso) in casi {
        let r = indice.cerca(domanda, 5, &FiltroVoci::default());
        assert_eq!(
            r.first().map(|x| x.codice.as_str()),
            Some(atteso),
            "«{domanda}»: {r:?}"
        );
    }
}

#[test]
fn ricerca_con_filtri_e_senza_risultati() {
    let p = prezzario();
    let indice = IndiceTestuale::new(&p);
    // «gres» compare in P.01 (m²) e P.03 (m): il filtro per lunghezza lascia il battiscopa.
    let filtro = FiltroVoci {
        dimensioni: vec![Dimensione::Lunghezza],
        ..FiltroVoci::default()
    };
    let r = indice.cerca("gres porcellanato", 5, &filtro);
    assert_eq!(
        r.iter().map(|x| x.codice.as_str()).collect::<Vec<_>>(),
        ["P.03"]
    );
    let filtro = FiltroVoci {
        capitolo: Some("D".into()),
        ..FiltroVoci::default()
    };
    assert!(
        indice
            .cerca("pavimento", 5, &filtro)
            .iter()
            .all(|x| x.codice.starts_with('D'))
    );
    assert!(
        indice
            .cerca("elicottero", 5, &FiltroVoci::default())
            .is_empty()
    );
    assert!(indice.cerca("", 5, &FiltroVoci::default()).is_empty());
    // Ricerca deterministica e prezzario intatto.
    let a = indice.cerca("massetto", 10, &FiltroVoci::default());
    assert_eq!(a, indice.cerca("massetto", 10, &FiltroVoci::default()));
    assert_eq!(p, prezzario());
}

#[test]
fn candidati_dall_analisi() {
    let p = prezzario();
    let indice = IndiceTestuale::new(&p);
    let analisi = RispostaAnalisi {
        esito: "procedi".into(),
        lavorazioni: vec![
            LavorazioneAnalisi {
                id: "L01".into(),
                descrizione: "Massetto".into(),
                query_ricerca: "massetto sabbia cemento".into(),
                dimensione_attesa: "superficie".into(),
            },
            LavorazioneAnalisi {
                id: "L02".into(),
                descrizione: "Battiscopa".into(),
                query_ricerca: "battiscopa gres".into(),
                dimensione_attesa: "lunghezza".into(),
            },
        ],
        informazioni_mancanti: vec![],
    };
    let c = Candidati::da_analisi(&indice, &p, &analisi, 3, 40);
    // Alternanza: il primo di L01, poi il primo di L02.
    let codici: Vec<&str> = c.elenco.iter().map(|x| x.codice.as_str()).collect();
    assert_eq!(codici.first(), Some(&"M.01"));
    assert_eq!(codici.get(1), Some(&"P.03"));
    assert_eq!(c.elenco.first().map(|x| x.handle.as_str()), Some("c01"));
    assert_eq!(
        c.elenco.first().map(|x| x.capitoli.clone()),
        Some(vec!["Massetti e sottofondi".to_owned()])
    );
    // Il modello non vede codici né prezzi.
    let visto = c.per_modello().to_string();
    assert!(!visto.contains("M.01") && !visto.contains("18,40") && !visto.contains("18.40"));
    assert!(visto.contains("\"handle\":\"c01\""));
    // Limite complessivo.
    assert_eq!(
        Candidati::da_analisi(&indice, &p, &analisi, 3, 1)
            .elenco
            .len(),
        1
    );
}

// ---------------------------------------------------------------- proposta completa

const BAGNO: &str = "Rifacimento di un bagno: demolizione del pavimento esistente di 6 mq, \
    nuovo massetto e pavimento in gres su una superficie di 2,50 x 2,40 m, rivestimento \
    delle pareti alto 2,20 m su un perimetro di 9,80 m, sostituzione del vaso.";

fn risposta_bagno(c: &Candidati) -> RispostaSelezione {
    let n1 = || u("N", "1", "n");
    risposta(
        "provvisorio",
        vec![
            lav(
                "L01",
                &h(c, "D.01"),
                "principale",
                "",
                "",
                vec![riga(
                    "bagno",
                    "superficie",
                    "positivo",
                    vec![n1(), u("A", "6", "m2")],
                )],
            ),
            lav(
                "L02",
                &h(c, "M.01"),
                "principale",
                "",
                "",
                vec![riga(
                    "bagno",
                    "rettangolo",
                    "positivo",
                    vec![n1(), u("L", "2.50", "m"), u("B", "2.40", "m")],
                )],
            ),
            lav(
                "L03",
                &h(c, "P.01"),
                "principale",
                "",
                "",
                vec![riga(
                    "bagno",
                    "rettangolo",
                    "positivo",
                    vec![n1(), u("L", "2.5", "m"), u("B", "2.4", "m")],
                )],
            ),
            lav(
                "L04",
                &h(c, "P.02"),
                "principale",
                "",
                "",
                vec![
                    riga(
                        "pareti",
                        "rettangolo",
                        "positivo",
                        vec![n1(), u("L", "9.80", "m"), u("B", "2.20", "m")],
                    ),
                    riga(
                        "porta",
                        "rettangolo",
                        "detrazione",
                        vec![
                            n1(),
                            par("L", "0.80", "m", "ipotesi", "I01"),
                            par("B", "2.10", "m", "ipotesi", "I01"),
                        ],
                    ),
                ],
            ),
            lav(
                "L05",
                &h(c, "I.01"),
                "principale",
                "",
                "",
                vec![riga("vaso", "conteggio", "positivo", vec![n1()])],
            ),
            lav(
                "L06",
                &h(c, "D.03"),
                "complementare",
                "",
                "L05",
                vec![riga("vaso esistente", "conteggio", "positivo", vec![n1()])],
            ),
        ],
        vec![json!({"id": "I01", "testo": "porta di 0,80 x 2,10 m", "impatto": "medio"})],
    )
}

#[test]
fn bagno_dalla_risposta_al_computo() {
    let p = prezzario();
    let c = candidati(&p, &["D.01", "M.01", "P.01", "P.02", "I.01", "D.03"]);
    let mut s = sessione(&p);
    let proposta = valida(&s, &p, &c, BAGNO, &risposta_bagno(&c));

    assert_eq!(proposta.esito, EsitoProposta::Provvisorio); // c'è un'ipotesi
    assert!(proposta.escluse.is_empty(), "{:?}", proposta.escluse);
    assert_eq!(codici_avvisi(&proposta), Vec::<String>::new());
    assert_eq!(proposta.lavorazioni.len(), 6);
    assert_eq!(proposta.ipotesi.len(), 1);
    // Provenienze: L dalla richiesta, la porta da ipotesi.
    let l04 = proposta.lavorazioni.iter().find(|l| l.id == "L04").unwrap();
    assert_eq!(l04.ipotesi, ["I01"]);
    assert_eq!(
        l04.righe
            .first()
            .and_then(|r| r.parametri.get(1))
            .map(|p| p.provenienza.clone()),
        Some(ProvenienzaValore::Utente {
            testo: "9,80 m".into()
        })
    );
    // Comandi: una copia dal prezzario, poi una voce per lavorazione.
    assert_eq!(proposta.comandi.len(), 7);
    assert!(matches!(
        proposta.comandi.first(),
        Some(Comando::CopiaDaPrezzario { prezzario, codici }) if prezzario == NOME && codici.len() == 6
    ));

    // Prova: 84,00 + 110,40 + 312,00 + 924,42 + 280,00 + 35,00 = 1.745,82 €
    // (rivestimento: 9,80 × 2,20 = 21,56 − 0,80 × 2,10 = 1,68 → 19,88 m² × 46,50).
    let prova = verifica::prova(&s, &proposta).unwrap();
    assert!(!prova.applicato);
    assert_eq!(totale(&prova), Some(Importo::new(dec!(1745.82))));
    assert!(
        s.progetto().computi.first().unwrap().voci.is_empty(),
        "la prova non modifica"
    );

    // Esecuzione: i prezzi vengono dal prezzario, le note tracciano la lavorazione.
    let e = s.esegui(&proposta.comandi).unwrap();
    assert_eq!(totale(&e), Some(Importo::new(dec!(1745.82))));
    let computo = s.progetto().computi.first().unwrap();
    assert_eq!(computo.voci.len(), 6);
    let note = computo
        .voci
        .get(3)
        .and_then(|v| v.note.clone())
        .unwrap_or_default();
    assert!(note.starts_with("Assistente AI, L04"), "{note}");
    assert!(note.contains("I01 (porta di 0,80 x 2,10 m)"), "{note}");
    // Annullabile in un solo passo.
    s.annulla().unwrap();
    assert!(s.progetto().computi.first().unwrap().voci.is_empty());
    assert!(s.progetto().elenco_prezzi.voci.is_empty());
}

#[test]
fn proposta_completa_e_maiuscole_negli_enum() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = sessione(&p);
    let richiesta = "Tinteggiatura di una parete di 4 x 3 m";
    let r = risposta(
        "Completo",
        vec![lav(
            "L01",
            &h(&c, "T.01").to_uppercase(),
            "PRINCIPALE",
            "",
            "",
            vec![riga(
                "parete",
                "Rettangolo",
                "Positivo",
                vec![
                    par("n", "1", "N", "Utente", ""),
                    u("L", "4", "m"),
                    par("B", "300", "cm", "utente", ""),
                ],
            )],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, richiesta, &r);
    assert_eq!(
        proposta.esito,
        EsitoProposta::Completo,
        "{:?}",
        proposta.avvisi
    );
    // 300 cm = 3 m, trovato nella richiesta; 4 × 3 = 12,00 m² × 7,20 = 86,40 €.
    let prova = verifica::prova(&s, &proposta).unwrap();
    assert_eq!(totale(&prova), Some(Importo::new(dec!(86.40))));
}

#[test]
fn unita_non_base_e_forme_con_formula() {
    let p = prezzario();
    let c = candidati(&p, &["C.03", "C.04", "A.01"]);
    let s = sessione(&p);
    let richiesta = "Fornitura di 2 travi in acciaio lunghe 6 m da 30,6 kg/m; 8 pali di diametro 60 cm profondi 12 m; allestimento del cantiere";
    let r = risposta(
        "completo",
        vec![
            lav(
                "L01",
                &h(&c, "C.03"),
                "principale",
                "",
                "",
                vec![riga(
                    "travi",
                    "massa_lineare",
                    "positivo",
                    vec![u("N", "2", "n"), u("L", "6", "m"), u("P", "30.6", "kg/m")],
                )],
            ),
            lav(
                "L02",
                &h(&c, "C.04"),
                "principale",
                "",
                "",
                vec![riga(
                    "pali",
                    "cilindro",
                    "positivo",
                    vec![u("N", "8", "n"), u("D", "60", "cm"), u("H", "12", "m")],
                )],
            ),
            lav(
                "L03",
                &h(&c, "A.01"),
                "principale",
                "",
                "",
                vec![riga("cantiere", "a_corpo", "positivo", vec![])],
            ),
        ],
        vec![],
    );
    let proposta = valida(&s, &p, &c, richiesta, &r);
    assert_eq!(
        proposta.esito,
        EsitoProposta::Completo,
        "{:?}",
        proposta.avvisi
    );
    let Some(Comando::AggiungiVoceComputo { righi, .. }) = proposta.comandi.get(1) else {
        panic!("comando inatteso");
    };
    assert_eq!(
        righi.first().and_then(|r| r.formula.as_deref()),
        Some("(par*lun*alt)/1000")
    );
    // Travi: 2 × 6 × 30,6 / 1000 = 0,3672 → 0,37 t × 2.150,00 = 795,50
    // Pali: 8 × π × 0,60² / 4 × 12 = 27,1434 → 27,14 m³ × 210,00 = 5.699,40
    // Cantiere: 1 × 1.500,00
    let prova = verifica::prova(&s, &proposta).unwrap();
    assert_eq!(totale(&prova), Some(Importo::new(dec!(7994.90))));
}

// ---------------------------------------------------------------- controlli

/// Una lavorazione di tinteggiatura modificata da `f`; restituisce i codici degli avvisi
/// e se la lavorazione è stata esclusa.
fn controlla(f: impl FnOnce(&mut Value, &Candidati)) -> (Vec<String>, bool, Proposta) {
    let p = prezzario();
    let c = candidati(&p, &["T.01", "S.02", "O.01"]);
    let s = sessione(&p);
    let mut l = lav(
        "L01",
        &h(&c, "T.01"),
        "principale",
        "",
        "",
        vec![riga(
            "parete",
            "rettangolo",
            "positivo",
            vec![u("N", "1", "n"), u("L", "4", "m"), u("B", "3", "m")],
        )],
    );
    f(&mut l, &c);
    let r = leggi_selezione(&json!({
        "esito": "completo", "lavorazioni": [l],
        "ipotesi": [{"id": "I01", "testo": "altezza standard", "impatto": "basso"}],
        "informazioni_mancanti": []
    }))
    .unwrap();
    let proposta = valida(&s, &p, &c, "Tinteggiatura di una parete di 4 x 3 m", &r);
    let esclusa = !proposta.escluse.is_empty();
    (codici_avvisi(&proposta), esclusa, proposta)
}

fn parametri(l: &mut Value) -> &mut Vec<Value> {
    l["righe"][0]["parametri"].as_array_mut().unwrap()
}

/// Modifica di una lavorazione di prova.
type Modifica = Box<dyn FnOnce(&mut Value, &Candidati)>;

#[test]
fn ogni_controllo_ha_il_suo_codice() {
    let casi: Vec<(&str, Modifica, &str)> = vec![
        (
            "handle mai inviato",
            Box::new(|l, _| l["candidato"] = json!("c98")),
            "assistente.candidato_inesistente",
        ),
        (
            "forma di lunghezza su voce in m²",
            Box::new(|l, _| {
                l["righe"][0]["forma"] = json!("lunghezza");
                parametri(l).truncate(2);
            }),
            "assistente.unita_incompatibile",
        ),
        (
            "parametro mancante",
            Box::new(|l, _| {
                parametri(l).pop();
            }),
            "assistente.parametri_errati",
        ),
        (
            "parametro in più",
            Box::new(|l, _| parametri(l).push(u("H", "3", "m"))),
            "assistente.parametri_errati",
        ),
        (
            "parametro ripetuto",
            Box::new(|l, _| parametri(l).push(u("L", "4", "m"))),
            "assistente.parametri_errati",
        ),
        (
            "unità non ammessa",
            Box::new(|l, _| l["righe"][0]["parametri"][1]["unita"] = json!("m2")),
            "assistente.parametri_errati",
        ),
        (
            "valore zero",
            Box::new(|l, _| l["righe"][0]["parametri"][1]["valore"] = json!("0")),
            "assistente.valore_non_valido",
        ),
        (
            "N frazionario",
            Box::new(|l, _| l["righe"][0]["parametri"][0]["valore"] = json!("1.5")),
            "assistente.valore_non_valido",
        ),
        (
            "ipotesi inesistente",
            Box::new(|l, _| l["righe"][0]["parametri"][1] = par("L", "4", "m", "ipotesi", "I09")),
            "assistente.ipotesi_inesistente",
        ),
        (
            "principale con complementare_di",
            Box::new(|l, _| l["complementare_di"] = json!("L02")),
            "assistente.ruolo_incoerente",
        ),
        (
            "alternativa senza gruppo",
            Box::new(|l, _| l["ruolo"] = json!("alternativa")),
            "assistente.ruolo_incoerente",
        ),
        (
            "nessun rigo",
            Box::new(|l, _| l["righe"] = json!([])),
            "assistente.righe_assenti",
        ),
        (
            "unità m²/mese",
            Box::new(|l, c| l["candidato"] = json!(h(c, "S.02"))),
            "assistente.unita_non_supportata",
        ),
        (
            "unità oraria",
            Box::new(|l, c| l["candidato"] = json!(h(c, "O.01"))),
            "assistente.unita_non_supportata",
        ),
        (
            "forma sconosciuta",
            Box::new(|l, _| l["righe"][0]["forma"] = json!("sfera")),
            "assistente.parametri_errati",
        ),
    ];
    for (nome, modifica, atteso) in casi {
        let (codici, esclusa, proposta) = controlla(modifica);
        assert!(esclusa, "{nome}: non esclusa");
        assert!(codici.iter().any(|c| c == atteso), "{nome}: {codici:?}");
        assert!(proposta.comandi.is_empty(), "{nome}: nessun comando");
        assert_eq!(proposta.esito, EsitoProposta::Impossibile, "{nome}");
        assert!(
            proposta
                .avvisi
                .iter()
                .all(|a| a.lavorazione.as_deref() == Some("L01"))
        );
    }
}

#[test]
fn valore_non_fondato_diventa_ipotesi() {
    let (codici, esclusa, proposta) = controlla(|l, _| {
        l["righe"][0]["parametri"][1] = u("L", "5", "m"); // la richiesta dice 4 x 3
    });
    assert!(!esclusa);
    assert_eq!(codici, ["assistente.valore_non_fondato"]);
    assert_eq!(proposta.esito, EsitoProposta::Provvisorio);
    let r01 = proposta.ipotesi.iter().find(|i| i.id == "R01").unwrap();
    assert!(r01.dalla_validazione);
    let lav = proposta.lavorazioni.first().unwrap();
    assert_eq!(lav.ipotesi, ["R01"]);
    assert_eq!(
        lav.righe
            .first()
            .and_then(|r| r.parametri.get(1))
            .map(|p| p.provenienza.clone()),
        Some(ProvenienzaValore::Ipotesi { id: "R01".into() })
    );
    // N = 1 non è una dimensione inventata: non serve trovarlo nel testo.
    assert!(
        lav.righe
            .first()
            .and_then(|r| r.parametri.first())
            .is_some_and(|p| {
                p.provenienza
                    == ProvenienzaValore::Utente {
                        testo: String::new(),
                    }
            })
    );
}

#[test]
fn lavorazione_esclusa_non_lascia_ipotesi_orfane() {
    // Valore non fondato e parametro mancante nella stessa lavorazione: esclusa, e
    // nessuna ipotesi R01 né avviso di valore non fondato.
    let (codici, esclusa, proposta) = controlla(|l, _| {
        l["righe"][0]["parametri"][1] = u("L", "5", "m");
        parametri(l).pop();
    });
    assert!(esclusa);
    assert_eq!(codici, ["assistente.parametri_errati"]);
    assert!(proposta.ipotesi.iter().all(|i| !i.dalla_validazione));
}

#[test]
fn nessuna_voce_pertinente() {
    let (codici, esclusa, proposta) = controlla(|l, _| l["candidato"] = json!("NESSUNO"));
    assert!(esclusa);
    assert_eq!(codici, ["assistente.voce_non_trovata"]);
    assert_eq!(
        proposta.avvisi.first().map(|a| a.gravita),
        Some(Gravita::Avviso)
    );
    assert_eq!(proposta.esito, EsitoProposta::Impossibile);
}

#[test]
fn duplicati_alternative_e_complementari() {
    let p = prezzario();
    let c = candidati(&p, &["T.01", "P.01", "P.02"]);
    let s = sessione(&p);
    let parete = |l: &str| {
        riga(
            "parete",
            "rettangolo",
            "positivo",
            vec![u("N", "1", "n"), u("L", l, "m"), u("B", "3", "m")],
        )
    };
    let r = risposta(
        "completo",
        vec![
            lav(
                "L01",
                &h(&c, "T.01"),
                "principale",
                "",
                "",
                vec![parete("4")],
            ),
            lav(
                "L02",
                &h(&c, "T.01"),
                "principale",
                "",
                "",
                vec![parete("4.0")],
            ), // stesse misure
            lav(
                "L03",
                &h(&c, "T.01"),
                "principale",
                "",
                "",
                vec![parete("3")],
            ), // altre misure
            lav(
                "L04",
                &h(&c, "P.01"),
                "alternativa",
                "finitura",
                "",
                vec![parete("4")],
            ),
            lav(
                "L05",
                &h(&c, "P.02"),
                "alternativa",
                "Finitura",
                "",
                vec![parete("4")],
            ),
            lav(
                "L06",
                &h(&c, "T.01"),
                "complementare",
                "",
                "L09",
                vec![parete("2")],
            ),
        ],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "pareti di 4 x 3 m, 3 x 3 m e 2 x 3 m", &r);
    let per_lav: Vec<(String, Option<String>)> = proposta
        .avvisi
        .iter()
        .map(|a| (a.codice.clone(), a.lavorazione.clone()))
        .collect();
    let atteso = |c: &str, l: &str| (format!("assistente.{c}"), Some(l.to_owned()));
    assert!(per_lav.contains(&atteso("duplicato", "L02")), "{per_lav:?}");
    assert!(
        per_lav.contains(&atteso("possibile_doppio_conteggio", "L03")),
        "{per_lav:?}"
    );
    assert!(
        per_lav.contains(&atteso("alternativa_non_inclusa", "L05")),
        "{per_lav:?}"
    );
    assert!(
        per_lav.contains(&atteso("complementare_senza_principale", "L06")),
        "{per_lav:?}"
    );
    assert_eq!(
        proposta
            .escluse
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["L02"]
    );
    // L01, L03, L04, L06 nel computo; L05 resta come alternativa non inclusa.
    let aggiunte = proposta
        .comandi
        .iter()
        .filter(|c| matches!(c, Comando::AggiungiVoceComputo { .. }))
        .count();
    assert_eq!(aggiunte, 4);
    assert_eq!(
        proposta.lavorazioni.iter().filter(|l| !l.inclusa).count(),
        1
    );
    assert_eq!(proposta.esito, EsitoProposta::Provvisorio);
}

#[test]
fn prezzo_dell_elenco_prevale() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let mut s = sessione(&p);
    s.esegui(&serde_json::from_value::<Vec<Comando>>(json!([
        {"comando": "aggiungi_voce_prezzario", "codice": "T.01", "descrizione_breve": "Tinteggiatura", "unita_misura": "m²", "prezzo": "8.00"}
    ])).unwrap())
    .unwrap();
    let r = risposta(
        "completo",
        vec![lav(
            "L01",
            &h(&c, "T.01"),
            "principale",
            "",
            "",
            vec![riga(
                "parete",
                "rettangolo",
                "positivo",
                vec![u("N", "1", "n"), u("L", "4", "m"), u("B", "3", "m")],
            )],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "parete di 4 x 3 m", &r);
    assert_eq!(
        codici_avvisi(&proposta),
        ["assistente.prezzo_elenco_diverso"]
    );
    // Nessuna copia dal prezzario: la voce c'è già. 12,00 m² × 8,00 = 96,00 €.
    assert_eq!(proposta.comandi.len(), 1);
    let prova = verifica::prova(&s, &proposta).unwrap();
    assert_eq!(totale(&prova), Some(Importo::new(dec!(96.00))));
}

#[test]
fn risposte_inutilizzabili() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = sessione(&p);
    let ctx = Contesto {
        richiesta: "",
        nome_prezzario: NOME,
        prezzario: &p,
        candidati: &c,
        progetto: s.progetto(),
        computo: None,
    };
    let r = risposta("forse", vec![], vec![]);
    assert!(matches!(
        valida_selezione(&r, &ctx),
        Err(ErroreRisposta::EsitoSconosciuto { .. })
    ));
    let l = lav("L01", "c01", "principale", "", "", vec![]);
    let r = risposta("completo", vec![l.clone(), l], vec![]);
    assert!(matches!(
        valida_selezione(&r, &ctx),
        Err(ErroreRisposta::IdRipetuto { .. })
    ));
    let r = risposta(
        "completo",
        vec![],
        vec![
            json!({"id": "I01", "testo": "a", "impatto": "basso"}),
            json!({"id": "i01", "testo": "b", "impatto": "basso"}),
        ],
    );
    assert!(matches!(
        valida_selezione(&r, &ctx),
        Err(ErroreRisposta::IdRipetuto { .. })
    ));
    assert!(matches!(
        leggi_selezione(&json!({"esito": "completo"})),
        Err(ErroreRisposta::NonConforme { .. })
    ));
    // Nessuna lavorazione: impossibile, senza comandi.
    let vuota = valida_selezione(&risposta("completo", vec![], vec![]), &ctx).unwrap();
    assert_eq!(vuota.esito, EsitoProposta::Impossibile);
    assert!(vuota.comandi.is_empty());
    assert!(verifica::prova(&s, &vuota).is_err());
}

#[test]
fn prova_senza_prezzario_caricato_e_rifiutata() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = Sessione::nuovo("Senza prezzario");
    let r = risposta(
        "completo",
        vec![lav(
            "L01",
            &h(&c, "T.01"),
            "principale",
            "",
            "",
            vec![riga(
                "parete",
                "rettangolo",
                "positivo",
                vec![u("N", "1", "n"), u("L", "4", "m"), u("B", "3", "m")],
            )],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "4 x 3 m", &r);
    let e = verifica::prova(&s, &proposta).unwrap_err();
    assert!(
        e.to_string()
            .contains("prezzario caricato «prova» inesistente"),
        "{e}"
    );
}
