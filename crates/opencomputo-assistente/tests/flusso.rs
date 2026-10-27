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

use std::collections::BTreeMap;

use opencomputo_application::{Comando, Sessione};
use opencomputo_assistente::candidati::Candidati;
use opencomputo_assistente::proposta::{
    Contesto, ErroreRisposta, EsitoProposta, Pertinenza, Proposta, ProvenienzaValore,
    leggi_selezione, valida_selezione,
};
use opencomputo_assistente::ricerca::{FiltroVoci, IndiceTestuale};
use opencomputo_assistente::risposta::{LavorazioneAnalisi, RispostaAnalisi, RispostaSelezione};
use opencomputo_assistente::scelta::{ErroreScelta, Scelta, comandi};
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

/// Opzione: handle del codice, pertinenza.
fn op(c: &Candidati, codice: &str, pertinenza: &str) -> Value {
    json!({"candidato": h(c, codice), "pertinenza": pertinenza, "motivazione": format!("voce {codice}")})
}

/// Lavorazione chiara con le opzioni e i righi indicati.
fn lav(id: &str, opzioni: Vec<Value>, righe: Vec<Value>) -> Value {
    json!({
        "id": id, "descrizione": format!("lavorazione {id}"), "chiarezza": "chiara",
        "da_precisare": "", "righe": righe, "opzioni": opzioni
    })
}

/// Lavorazione da precisare.
fn da_precisare(id: &str, cosa: &str, opzioni: Vec<Value>, righe: Vec<Value>) -> Value {
    let mut l = lav(id, opzioni, righe);
    l["chiarezza"] = json!("da_precisare");
    l["da_precisare"] = json!(cosa);
    l
}

fn risposta(lavorazioni: Vec<Value>, ipotesi: Vec<Value>) -> RispostaSelezione {
    leggi_selezione(&json!({
        "lavorazioni": lavorazioni, "ipotesi": ipotesi, "informazioni_mancanti": []
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

fn scelta(lavorazione: &str, codici: &[&str]) -> Scelta {
    Scelta {
        lavorazione: lavorazione.into(),
        codici: codici.iter().map(|c| (*c).to_owned()).collect(),
        misure: None,
    }
}

fn prova(s: &Sessione, p: &Proposta, scelte: &[Scelta]) -> Option<Importo> {
    totale(&verifica::prova(s, p, scelte).unwrap())
}

fn rettangolo(l: &str, b: &str) -> Value {
    riga(
        "parete",
        "rettangolo",
        "positivo",
        vec![u("N", "1", "n"), u("L", l, "m"), u("B", b, "m")],
    )
}

fn codici_avvisi(p: &Proposta) -> Vec<String> {
    p.avvisi.iter().map(|a| a.codice.clone()).collect()
}

// ---------------------------------------------------------------- ricerca

#[test]
fn ricerca_trova_le_voci_attese() {
    let p = prezzario();
    let indice = IndiceTestuale::new(&p);
    assert_eq!(indice.len(), 26);
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
    nuovo massetto e pavimento su una superficie di 2,50 x 2,40 m, rivestimento \
    delle pareti alto 2,20 m su un perimetro di 9,80 m, sostituzione del vaso.";

fn risposta_bagno(c: &Candidati) -> RispostaSelezione {
    let n1 = || u("N", "1", "n");
    let bagno = || {
        riga(
            "bagno",
            "rettangolo",
            "positivo",
            vec![n1(), u("L", "2.50", "m"), u("B", "2.40", "m")],
        )
    };
    risposta(
        vec![
            lav(
                "L01",
                vec![op(c, "D.01", "alta")],
                vec![riga(
                    "bagno",
                    "superficie",
                    "positivo",
                    vec![n1(), u("A", "6", "m2")],
                )],
            ),
            lav("L02", vec![op(c, "M.01", "alta")], vec![bagno()]),
            // Materiale non indicato: tre opzioni, nessuna preselezionata.
            da_precisare(
                "L03",
                "materiale del pavimento non indicato",
                vec![
                    op(c, "P.05", "bassa"),
                    op(c, "P.01", "alta"),
                    op(c, "P.04", "media"),
                ],
                vec![bagno()],
            ),
            lav(
                "L04",
                vec![op(c, "P.02", "alta")],
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
                vec![op(c, "I.01", "alta")],
                vec![riga("vaso", "conteggio", "positivo", vec![n1()])],
            ),
            lav(
                "L06",
                vec![op(c, "D.03", "alta")],
                vec![riga("vaso esistente", "conteggio", "positivo", vec![n1()])],
            ),
        ],
        vec![json!({"id": "I01", "testo": "porta di 0,80 x 2,10 m", "impatto": "medio"})],
    )
}

#[test]
fn bagno_con_opzioni_scelte_e_computo() {
    let p = prezzario();
    let c = candidati(
        &p,
        &[
            "D.01", "M.01", "P.01", "P.04", "P.05", "P.02", "I.01", "D.03",
        ],
    );
    let mut s = sessione(&p);
    let proposta = valida(&s, &p, &c, BAGNO, &risposta_bagno(&c));

    assert_eq!(proposta.esito, EsitoProposta::DaRivedere); // pavimento da precisare, ipotesi
    assert!(proposta.senza_voce.is_empty());
    assert_eq!(codici_avvisi(&proposta), Vec::<String>::new());
    assert_eq!(proposta.lavorazioni.len(), 6);
    // Opzioni in ordine di pertinenza; nessuna preselezionata dove manca il materiale.
    let l03 = proposta.lavorazione("L03").unwrap();
    assert!(!l03.chiara);
    assert_eq!(l03.da_precisare, "materiale del pavimento non indicato");
    let codici: Vec<&str> = l03.opzioni.iter().map(|o| o.codice.as_str()).collect();
    assert_eq!(codici, ["P.01", "P.04", "P.05"]);
    assert_eq!(
        l03.opzioni.first().map(|o| o.pertinenza),
        Some(Pertinenza::Alta)
    );
    assert!(l03.opzioni.iter().all(|o| !o.preselezionata));
    assert!(proposta.lavorazione("L01").unwrap().opzioni[0].preselezionata);
    // Provenienze: L dalla richiesta, la porta da ipotesi.
    let l04 = proposta.lavorazione("L04").unwrap();
    assert_eq!(l04.ipotesi, ["I01"]);
    assert_eq!(
        l04.righe[0].parametri[1].provenienza,
        ProvenienzaValore::Utente {
            testo: "9,80 m".into()
        }
    );

    // Scelte iniziali: cinque voci, il pavimento escluso.
    // 84,00 + 110,40 + 924,42 + 280,00 + 35,00 = 1.433,82 € (rivestimento: 9,80 × 2,20 =
    // 21,56 − 0,80 × 2,10 = 1,68 → 19,88 m² × 46,50).
    let predefinite = proposta.scelte_predefinite();
    assert_eq!(predefinite.len(), 5);
    assert_eq!(
        prova(&s, &proposta, &predefinite),
        Some(Importo::new(dec!(1433.82)))
    );
    // L'utente sceglie il gres: + 6,00 m² × 52,00 = 312,00 → 1.745,82 €.
    let mut scelte = predefinite.clone();
    scelte.push(scelta("L03", &["P.01"]));
    let (cmd, voci) = comandi(&proposta, &scelte, s.progetto()).unwrap();
    assert_eq!(cmd.len(), 7);
    assert!(matches!(
        cmd.first(),
        Some(Comando::CopiaDaPrezzario { prezzario, codici }) if prezzario == NOME && codici.len() == 6
    ));
    // Nell'ordine delle lavorazioni, qualunque sia l'ordine delle scelte.
    assert_eq!(
        voci.iter().map(|v| v.codice.as_str()).collect::<Vec<_>>(),
        ["D.01", "M.01", "P.01", "P.02", "I.01", "D.03"]
    );
    let prova_e = verifica::prova(&s, &proposta, &scelte).unwrap();
    assert!(!prova_e.applicato);
    assert_eq!(totale(&prova_e), Some(Importo::new(dec!(1745.82))));
    assert!(
        s.progetto().computi[0].voci.is_empty(),
        "la prova non modifica"
    );

    let e = s.esegui(&cmd).unwrap();
    assert_eq!(totale(&e), Some(Importo::new(dec!(1745.82))));
    let computo = &s.progetto().computi[0];
    assert_eq!(computo.voci.len(), 6);
    let note = computo.voci[3].note.clone().unwrap_or_default();
    assert!(note.starts_with("Assistente AI, L04"), "{note}");
    assert!(note.contains("I01 (porta di 0,80 x 2,10 m)"), "{note}");
    // Annullabile in un solo passo.
    s.annulla().unwrap();
    assert!(s.progetto().computi[0].voci.is_empty());
    assert!(s.progetto().elenco_prezzi.voci.is_empty());

    // Due opzioni spuntate per la stessa lavorazione: due voci con le stesse misure.
    let doppie = [scelta("L03", &["P.01", "P.05"])];
    // 6,00 × 52,00 + 6,00 × 34,50 = 519,00 €.
    assert_eq!(
        prova(&s, &proposta, &doppie),
        Some(Importo::new(dec!(519.00)))
    );
}

#[test]
fn proposta_completa_e_maiuscole_negli_enum() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = sessione(&p);
    let r = leggi_selezione(&json!({
        "lavorazioni": [{
            "id": "L01", "descrizione": "parete", "chiarezza": "Chiara", "da_precisare": "",
            "righe": [riga("parete", "Rettangolo", "Positivo", vec![
                par("n", "1", "N", "Utente", ""), u("L", "4", "m"), par("B", "300", "cm", "utente", "")
            ])],
            "opzioni": [{"candidato": h(&c, "T.01").to_uppercase(), "pertinenza": "ALTA", "motivazione": "idropittura"}]
        }],
        "ipotesi": [], "informazioni_mancanti": []
    }))
    .unwrap();
    let proposta = valida(&s, &p, &c, "Tinteggiatura di una parete di 4 x 3 m", &r);
    assert_eq!(
        proposta.esito,
        EsitoProposta::Completo,
        "{:?}",
        proposta.avvisi
    );
    // 300 cm = 3 m, trovato nella richiesta; 4 × 3 = 12,00 m² × 7,20 = 86,40 €.
    assert_eq!(
        prova(&s, &proposta, &proposta.scelte_predefinite()),
        Some(Importo::new(dec!(86.40)))
    );
}

#[test]
fn unita_non_base_e_forme_con_formula() {
    let p = prezzario();
    let c = candidati(&p, &["C.03", "C.04", "A.01"]);
    let s = sessione(&p);
    let richiesta = "Fornitura di 2 travi in acciaio lunghe 6 m da 30,6 kg/m; 8 pali di diametro 60 cm profondi 12 m; allestimento del cantiere";
    let r = risposta(
        vec![
            lav(
                "L01",
                vec![op(&c, "C.03", "alta")],
                vec![riga(
                    "travi",
                    "massa_lineare",
                    "positivo",
                    vec![u("N", "2", "n"), u("L", "6", "m"), u("P", "30.6", "kg/m")],
                )],
            ),
            lav(
                "L02",
                vec![op(&c, "C.04", "alta")],
                vec![riga(
                    "pali",
                    "cilindro",
                    "positivo",
                    vec![u("N", "8", "n"), u("D", "60", "cm"), u("H", "12", "m")],
                )],
            ),
            lav(
                "L03",
                vec![op(&c, "A.01", "alta")],
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
    let (cmd, _) = comandi(&proposta, &proposta.scelte_predefinite(), s.progetto()).unwrap();
    let Some(Comando::AggiungiVoceComputo { righi, .. }) = cmd.get(1) else {
        panic!("comando inatteso");
    };
    assert_eq!(righi[0].formula.as_deref(), Some("(par*lun*alt)/1000"));
    // Travi: 2 × 6 × 30,6 / 1000 = 0,3672 → 0,37 t × 2.150,00 = 795,50
    // Pali: 8 × π × 0,60² / 4 × 12 = 27,1434 → 27,14 m³ × 210,00 = 5.699,40
    // Cantiere: 1 × 1.500,00
    assert_eq!(
        prova(&s, &proposta, &proposta.scelte_predefinite()),
        Some(Importo::new(dec!(7994.90)))
    );
}

// ---------------------------------------------------------------- controlli

#[test]
fn opzioni_ordinate_controllate_e_limitate() {
    let p = prezzario();
    let c = candidati(
        &p,
        &[
            "T.01", "T.02", "T.03", "P.02", "P.03", "S.02", "P.01", "P.05", "M.01",
        ],
    );
    let s = sessione(&p);
    let r = risposta(
        vec![lav(
            "L01",
            vec![
                op(&c, "T.03", "media"),
                json!({"candidato": "c99", "pertinenza": "alta", "motivazione": "inventato"}),
                op(&c, "T.01", "alta"),
                op(&c, "T.01", "alta"),
                op(&c, "P.03", "alta"), // in m: le misure danno una superficie
                op(&c, "S.02", "bassa"), // m²/mese: nessuna forma, misura manuale
                op(&c, "T.02", "bassa"),
                op(&c, "P.02", "bassa"),
                op(&c, "P.01", "bassa"),
                op(&c, "P.05", "bassa"),
            ],
            vec![rettangolo("4", "3")],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "parete 4 x 3 m", &r);
    let l = proposta.lavorazione("L01").unwrap();
    // Ordine: pertinenza, poi l'ordine del modello; al massimo 5.
    let codici: Vec<&str> = l.opzioni.iter().map(|o| o.codice.as_str()).collect();
    assert_eq!(codici, ["T.01", "T.03", "S.02", "T.02", "P.02"]);
    assert!(l.opzioni[0].preselezionata && l.opzioni.iter().skip(1).all(|o| !o.preselezionata));
    assert!(l.opzioni[2].misura_manuale);
    let mut avvisi = codici_avvisi(&proposta);
    avvisi.sort();
    avvisi.dedup();
    assert_eq!(
        avvisi,
        [
            "assistente.candidato_inesistente",
            "assistente.misura_manuale",
            "assistente.opzione_ripetuta",
            "assistente.unita_incompatibile"
        ]
    );
    // Sono dettagli: la proposta resta completa.
    assert!(
        proposta
            .avvisi
            .iter()
            .all(|a| a.gravita == Gravita::Informazione)
    );
    assert_eq!(proposta.esito, EsitoProposta::Completo);
    // La voce in m²/mese entra con un rigo vuoto che riporta le misure.
    let (cmd, voci) = comandi(&proposta, &[scelta("L01", &["S.02"])], s.progetto()).unwrap();
    assert!(voci[0].da_completare);
    let Some(Comando::AggiungiVoceComputo { righi, note, .. }) = cmd.last() else {
        panic!()
    };
    assert_eq!(righi.len(), 1);
    assert!(righi[0].parti_uguali.is_none() && righi[0].lunghezza.is_none());
    assert!(
        righi[0].descrizione.contains("L = 4 m, B = 3 m"),
        "{}",
        righi[0].descrizione
    );
    assert!(
        note.as_deref()
            .unwrap_or_default()
            .ends_with("Misure da completare.")
    );
}

#[test]
fn valori_da_completare_e_misure_corrette() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = sessione(&p);
    let r = risposta(
        vec![lav(
            "L01",
            vec![op(&c, "T.01", "alta")],
            vec![riga(
                "pareti",
                "rettangolo",
                "positivo",
                vec![
                    u("N", "1", "n"),
                    par("L", "", "m", "mancante", ""),
                    par("H", "3", "m", "utente", ""), // non della forma: scartato
                ],
            )],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "pareti alte 3 m", &r);
    assert_eq!(proposta.esito, EsitoProposta::DaRivedere);
    let l = proposta.lavorazione("L01").unwrap();
    assert!(l.da_completare());
    let par = &l.righe[0].parametri;
    assert_eq!(
        par.iter()
            .map(|p| format!("{:?}", p.nome))
            .collect::<Vec<_>>(),
        ["N", "L", "B"]
    );
    assert!(par[1].valore.is_none() && par[2].valore.is_none());
    assert_eq!(par[2].provenienza, ProvenienzaValore::Mancante);
    assert_eq!(codici_avvisi(&proposta), ["assistente.parametro_scartato"]);

    // Senza correzioni: rigo vuoto, nessuna quantità parziale.
    let (cmd, voci) = comandi(&proposta, &proposta.scelte_predefinite(), s.progetto()).unwrap();
    assert!(voci[0].da_completare);
    let Some(Comando::AggiungiVoceComputo { righi, .. }) = cmd.last() else {
        panic!()
    };
    assert_eq!(
        righi[0].descrizione,
        "pareti — da completare: N = 1, L = ?, B = ?"
    );
    assert!(righi[0].lunghezza.is_none());
    let a = verifica::anteprima(&s, &proposta, &proposta.scelte_predefinite()).unwrap();
    assert_eq!(a.opzioni[0].importo, Some(Importo::new(dec!(0.00))));
    assert!(a.opzioni[0].da_completare);

    // L'utente completa le misure: 16 × 2,70 = 43,20 m² × 7,20 = 311,04 €.
    let misure = vec![BTreeMap::from([
        ("L".to_owned(), "16".to_owned()),
        ("B".to_owned(), "2,70".to_owned()),
    ])];
    let scelte = [Scelta {
        lavorazione: "L01".into(),
        codici: vec!["T.01".into()],
        misure: Some(misure.clone()),
    }];
    assert_eq!(
        prova(&s, &proposta, &scelte),
        Some(Importo::new(dec!(311.04)))
    );
    let (cmd, voci) = comandi(&proposta, &scelte, s.progetto()).unwrap();
    assert!(!voci[0].da_completare);
    let Some(Comando::AggiungiVoceComputo { note, .. }) = cmd.last() else {
        panic!()
    };
    assert!(
        !note
            .as_deref()
            .unwrap_or_default()
            .contains("da completare")
    );
    // Anteprima con le misure corrette anche senza opzioni spuntate.
    let a = verifica::anteprima(
        &s,
        &proposta,
        &[Scelta {
            codici: vec![],
            ..scelte[0].clone()
        }],
    )
    .unwrap();
    assert_eq!(a.opzioni[0].importo, Some(Importo::new(dec!(311.04))));
    assert!(a.esito.is_none() && a.scelte.is_empty());
    assert!(
        a.errore
            .is_some_and(|e| e.to_string() == "nessuna voce scelta")
    );
}

#[test]
fn scelte_non_valide() {
    let p = prezzario();
    let c = candidati(&p, &["T.01", "T.03"]);
    let s = sessione(&p);
    let r = risposta(
        vec![lav(
            "L01",
            vec![op(&c, "T.01", "alta"), op(&c, "T.03", "media")],
            vec![rettangolo("4", "3")],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "parete 4 x 3 m", &r);
    let errore = |scelte: Vec<Scelta>| comandi(&proposta, &scelte, s.progetto()).unwrap_err();
    assert_eq!(errore(vec![]), ErroreScelta::NessunaScelta);
    assert_eq!(
        errore(vec![scelta("L01", &[])]),
        ErroreScelta::NessunaScelta
    );
    assert!(matches!(
        errore(vec![scelta("L09", &["T.01"])]),
        ErroreScelta::Lavorazione { .. }
    ));
    assert!(matches!(
        errore(vec![scelta("L01", &["P.01"])]),
        ErroreScelta::Opzione { .. }
    ));
    let con_misure = |m: Vec<BTreeMap<String, String>>| Scelta {
        misure: Some(m),
        ..scelta("L01", &["T.01"])
    };
    let m = |k: &str, v: &str| BTreeMap::from([(k.to_owned(), v.to_owned())]);
    for (misure, atteso) in [
        (vec![], "0 righi di misure invece di 1"),
        (vec![m("X", "1")], "parametro «X» non previsto"),
        (vec![m("H", "1")], "parametro «H» non previsto"),
        (vec![m("L", "abc")], "non è un numero maggiore di zero"),
        (vec![m("L", "0")], "non è un numero maggiore di zero"),
        (vec![m("N", "1.5")], "N deve essere intero"),
    ] {
        let e = errore(vec![con_misure(misure)]);
        assert!(
            matches!(e, ErroreScelta::Misure { .. }) && e.to_string().contains(atteso),
            "{e}"
        );
    }
    // Una misura svuotata rende il rigo da completare.
    let (_, voci) = comandi(&proposta, &[con_misure(vec![m("B", "")])], s.progetto()).unwrap();
    assert!(voci[0].da_completare);
}

#[test]
fn valore_non_fondato_diventa_ipotesi() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = sessione(&p);
    let r = risposta(
        vec![lav(
            "L01",
            vec![op(&c, "T.01", "alta")],
            vec![rettangolo("4", "3.5")],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "parete lunga 4 m", &r);
    assert_eq!(codici_avvisi(&proposta), ["assistente.valore_non_fondato"]);
    assert_eq!(proposta.esito, EsitoProposta::DaRivedere);
    assert_eq!(proposta.ipotesi.len(), 1);
    assert!(proposta.ipotesi[0].dalla_validazione);
    assert_eq!(
        proposta.lavorazioni[0].righe[0].parametri[2].provenienza,
        ProvenienzaValore::Ipotesi { id: "R01".into() }
    );
}

#[test]
fn righi_scartati_e_lavorazioni_senza_voce() {
    let p = prezzario();
    let c = candidati(&p, &["T.01", "P.03"]);
    let s = sessione(&p);
    let r = risposta(
        vec![
            lav(
                "L01",
                vec![op(&c, "T.01", "alta")],
                vec![
                    rettangolo("4", "3"),
                    riga(
                        "cornice",
                        "lunghezza",
                        "positivo",
                        vec![u("N", "1", "n"), u("L", "4", "m")],
                    ),
                    riga("strana", "esagono", "positivo", vec![]),
                ],
            ),
            lav("L02", vec![], vec![rettangolo("4", "3")]),
            lav(
                "L03",
                vec![op(&c, "P.03", "alta")],
                vec![rettangolo("4", "3")],
            ),
        ],
        vec![json!({"id": "I09", "testo": "mai usata", "impatto": "basso"})],
    );
    let proposta = valida(&s, &p, &c, "parete 4 x 3 m", &r);
    assert_eq!(proposta.lavorazione("L01").unwrap().righe.len(), 1);
    assert_eq!(proposta.senza_voce.len(), 2);
    assert!(
        proposta.senza_voce[0]
            .motivo
            .starts_with("nessuna voce pertinente")
    );
    assert!(
        proposta.senza_voce[1].motivo.contains("P.03 è in «m»"),
        "{}",
        proposta.senza_voce[1].motivo
    );
    assert!(
        proposta.ipotesi.is_empty(),
        "le ipotesi non usate non restano"
    );
    assert_eq!(proposta.esito, EsitoProposta::DaRivedere);
    // Nessuna opzione da nessuna parte: proposta vuota.
    let vuota = valida(
        &s,
        &p,
        &c,
        "",
        &risposta(vec![lav("L01", vec![], vec![])], vec![]),
    );
    assert_eq!(vuota.esito, EsitoProposta::Vuoto);
    assert!(vuota.scelte_predefinite().is_empty());
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
        vec![lav(
            "L01",
            vec![op(&c, "T.01", "alta")],
            vec![rettangolo("4", "3")],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "parete di 4 x 3 m", &r);
    assert_eq!(
        codici_avvisi(&proposta),
        ["assistente.prezzo_elenco_diverso"]
    );
    let o = &proposta.lavorazioni[0].opzioni[0];
    assert_eq!(o.prezzo.to_string(), "8.00");
    assert_eq!(
        o.prezzo_prezzario.map(|p| p.to_string()),
        Some("7.20".into())
    );
    // Nessuna copia dal prezzario: la voce c'è già. 12,00 m² × 8,00 = 96,00 €.
    let (cmd, _) = comandi(&proposta, &proposta.scelte_predefinite(), s.progetto()).unwrap();
    assert_eq!(cmd.len(), 1);
    assert_eq!(
        prova(&s, &proposta, &proposta.scelte_predefinite()),
        Some(Importo::new(dec!(96.00)))
    );
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
    let l = lav("L01", vec![], vec![]);
    let r = risposta(vec![l.clone(), l], vec![]);
    assert!(matches!(
        valida_selezione(&r, &ctx),
        Err(ErroreRisposta::IdRipetuto { .. })
    ));
    let r = risposta(
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
        leggi_selezione(&json!({"lavorazioni": []})),
        Err(ErroreRisposta::NonConforme { .. })
    ));
}

#[test]
fn prova_senza_prezzario_caricato_e_rifiutata() {
    let p = prezzario();
    let c = candidati(&p, &["T.01"]);
    let s = Sessione::nuovo("Senza prezzario");
    let r = risposta(
        vec![lav(
            "L01",
            vec![op(&c, "T.01", "alta")],
            vec![rettangolo("4", "3")],
        )],
        vec![],
    );
    let proposta = valida(&s, &p, &c, "4 x 3 m", &r);
    let e = verifica::prova(&s, &proposta, &proposta.scelte_predefinite()).unwrap_err();
    assert!(
        e.to_string()
            .contains("prezzario caricato «prova» inesistente"),
        "{e}"
    );
}
