//! Flusso completo con provider sostituti deterministici: indicizzazione con ripresa,
//! ricerca ibrida, due chiamate al modello, proposta provata nella sessione.
//!
//! Il «modello» finto risponde leggendo i candidati davvero inviati nel messaggio: se il
//! flusso mandasse candidati sbagliati, il test se ne accorgerebbe.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // Solo i vettori dell'embedding finto: nessun valore contabile.
    clippy::float_arithmetic
)]

use std::sync::Mutex;

use opencomputo_application::Sessione;
use opencomputo_assistente::flusso::{
    ErroreFlusso, EsitoFlusso, Parametri, Richiesta, Strumenti, proponi,
};
use opencomputo_assistente::proposta::EsitoProposta;
use opencomputo_assistente::provider::anthropic::{Chiamata, RispostaModello};
use opencomputo_assistente::provider::openai::LimitiLotto;
use opencomputo_assistente::provider::{Annullamento, Embedding, ErroreAi, Generazione};
use opencomputo_assistente::ricerca::IndiceTestuale;
use opencomputo_assistente::schema::{STRUMENTO_ANALISI, STRUMENTO_SELEZIONE};
use opencomputo_assistente::testo::termini;
use opencomputo_assistente::verifica;
use opencomputo_assistente::vettori::{
    IndiceVettoriale, SpazioEmbedding, da_indicizzare, impronta_prezzario, indicizza,
};
use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::numeri::Importo;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_format_listino::{OpzioniListino, importa_csv};
use rust_decimal_macros::dec;
use serde_json::{Value, json};

const CSV: &[u8] = include_bytes!("fixtures/prezzario_prova.csv");
const DIM: u32 = 64;

fn prezzario() -> Prezzario {
    importa_csv(
        CSV,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap()
    .prezzario
}

/// Embedding finto: conteggio dei termini sparsi in 64 posizioni. Testi con parole
/// comuni hanno vettori vicini.
struct EmbeddingFinto {
    modello: &'static str,
    /// Fallisce dal lotto n (da 1).
    guasto_dal_lotto: Option<usize>,
    lotti: Mutex<usize>,
    testi: Mutex<usize>,
}

impl EmbeddingFinto {
    fn new() -> Self {
        Self {
            modello: "finto",
            guasto_dal_lotto: None,
            lotti: Mutex::new(0),
            testi: Mutex::new(0),
        }
    }
}

fn vettore(testo: &str) -> Vec<f32> {
    let mut v = vec![0.0f32; DIM as usize];
    for t in termini(testo) {
        let h = t
            .bytes()
            .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
        v[(h % DIM) as usize] += 1.0;
    }
    v
}

impl Embedding for EmbeddingFinto {
    fn spazio(&self) -> SpazioEmbedding {
        SpazioEmbedding::new(self.modello, DIM)
    }
    fn limiti(&self) -> LimitiLotto {
        LimitiLotto {
            testi: 5,
            caratteri: 100_000,
            caratteri_per_testo: 8_000,
        }
    }
    fn embed(&self, testi: &[String], _: &Annullamento) -> Result<Vec<Vec<f32>>, ErroreAi> {
        let mut n = self.lotti.lock().unwrap();
        *n += 1;
        if self.guasto_dal_lotto.is_some_and(|g| *n >= g) {
            return Err(ErroreAi::Rete {
                provider: opencomputo_assistente::provider::Provider::Openai,
                messaggio: "interrotta".into(),
            });
        }
        *self.testi.lock().unwrap() += testi.len();
        Ok(testi.iter().map(|t| vettore(t)).collect())
    }
}

/// Modello finto: analisi preparata; nella selezione propone come opzioni i candidati
/// la cui descrizione contiene la parola indicata (il primo con pertinenza alta).
struct ModelloFinto {
    analisi: Value,
    /// (parola nella descrizione dei candidati, lavorazione JSON senza le opzioni)
    scelte: Vec<(&'static str, Value)>,
    ipotesi: Vec<Value>,
    chiamate: Mutex<Vec<Chiamata>>,
}

fn candidati_inviati(messaggio: &str) -> Vec<Value> {
    let inizio = messaggio
        .find("handle):\n")
        .map(|i| i + "handle):\n".len())
        .unwrap();
    serde_json::from_str::<Vec<Value>>(&messaggio[inizio..]).unwrap()
}

impl Generazione for ModelloFinto {
    fn genera(&self, c: &Chiamata, _: &Annullamento) -> Result<RispostaModello, ErroreAi> {
        self.chiamate.lock().unwrap().push(c.clone());
        let json = if c.nome == STRUMENTO_ANALISI {
            self.analisi.clone()
        } else {
            assert_eq!(c.nome, STRUMENTO_SELEZIONE);
            let candidati = candidati_inviati(&c.utente);
            let lavorazioni: Vec<Value> = self
                .scelte
                .iter()
                .map(|(parola, l)| {
                    let opzioni: Vec<Value> = candidati
                        .iter()
                        .filter(|x| {
                            let d = x["descrizione"].as_str().unwrap().to_lowercase();
                            // «^parola»: solo all'inizio della descrizione.
                            match parola.strip_prefix('^') {
                                Some(p) => d.starts_with(p),
                                None => d.contains(parola),
                            }
                        })
                        .enumerate()
                        .map(|(i, x)| {
                            json!({
                                "candidato": x["handle"],
                                "pertinenza": if i == 0 { "alta" } else { "media" },
                                "motivazione": "contiene la parola cercata"
                            })
                        })
                        .collect();
                    let mut l = l.clone();
                    l["opzioni"] = json!(opzioni);
                    l
                })
                .collect();
            json!({"lavorazioni": lavorazioni, "ipotesi": self.ipotesi, "informazioni_mancanti": []})
        };
        Ok(RispostaModello {
            json,
            token_ingresso: 100,
            token_uscita: 20,
            id_richiesta: None,
        })
    }
}

fn analisi_parete() -> Value {
    json!({
        "esito": "procedi",
        "lavorazioni": [{"id": "L01", "descrizione": "Tinteggiatura della parete", "query_ricerca": "tinteggiatura idropittura pareti", "dimensione_attesa": "superficie"}],
        "informazioni_mancanti": []
    })
}

fn lavorazione_parete() -> Value {
    json!({
        "id": "L01", "descrizione": "Tinteggiatura della parete", "chiarezza": "chiara",
        "da_precisare": "", "opzioni": [],
        "righe": [{"descrizione": "parete", "forma": "rettangolo", "segno": "positivo", "parametri": [
            {"nome": "N", "valore": "1", "unita": "n", "provenienza": "utente", "rif_ipotesi": ""},
            {"nome": "L", "valore": "4", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""},
            {"nome": "B", "valore": "3", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""}
        ]}]
    })
}

fn indice_vettoriale(p: &Prezzario, e: &EmbeddingFinto) -> IndiceVettoriale {
    let da = da_indicizzare(p, 8_000);
    let mut iv = IndiceVettoriale::new(e.spazio(), &impronta_prezzario(&da));
    indicizza(e, &mut iv, &da, &Annullamento::new(), |_, _| {}).unwrap();
    iv
}

#[test]
fn indicizzazione_incrementale_e_ripresa() {
    let mut p = prezzario();
    let da = da_indicizzare(&p, 8_000);
    assert_eq!(da.len(), 26);

    // Interruzione al terzo lotto: i due lotti completati restano nell'indice salvato.
    let guasto = EmbeddingFinto {
        guasto_dal_lotto: Some(3),
        ..EmbeddingFinto::new()
    };
    let mut iv = IndiceVettoriale::new(guasto.spazio(), &impronta_prezzario(&da));
    let mut salvato: Option<Vec<u8>> = None;
    let mut avanzamenti = Vec::new();
    let e = indicizza(&guasto, &mut iv, &da, &Annullamento::new(), |i, a| {
        salvato = Some(i.codifica());
        avanzamenti.push((a.lotto, a.lotti, a.calcolate, a.da_calcolare));
    });
    assert_eq!(e.map_err(|e| e.codice()), Err("rete"));
    assert_eq!(avanzamenti, [(1, 6, 5, 26), (2, 6, 10, 26)]);

    // Ripresa dal file salvato: si calcolano solo le 16 voci mancanti.
    let ok = EmbeddingFinto::new();
    let mut iv = IndiceVettoriale::decodifica(&salvato.unwrap(), &ok.spazio()).unwrap();
    let r = indicizza(&ok, &mut iv, &da, &Annullamento::new(), |_, _| {}).unwrap();
    assert_eq!((r.voci, r.riusate, r.calcolate, r.lotti), (26, 10, 16, 4));
    assert!(iv.completo(&da));

    // Di nuovo: niente da calcolare.
    let r = indicizza(&ok, &mut iv, &da, &Annullamento::new(), |_, _| {}).unwrap();
    assert_eq!((r.riusate, r.calcolate, r.lotti), (26, 0, 0));

    // Cambia una descrizione e sparisce una voce: una sola voce ricalcolata, una rimossa.
    p.voci[0]
        .descrizione_estesa
        .push_str(" in terreno roccioso");
    p.voci.pop();
    let da = da_indicizzare(&p, 8_000);
    let r = indicizza(&ok, &mut iv, &da, &Annullamento::new(), |_, _| {}).unwrap();
    assert_eq!((r.voci, r.riusate, r.calcolate, r.rimosse), (25, 24, 1, 1));

    // Spazio diverso: rifiutato prima di qualsiasi chiamata.
    let altro = EmbeddingFinto {
        modello: "altro",
        ..EmbeddingFinto::new()
    };
    let e = indicizza(&altro, &mut iv, &da, &Annullamento::new(), |_, _| {});
    assert_eq!(e.map_err(|e| e.codice()), Err("modello_non_supportato"));
    assert_eq!(*altro.lotti.lock().unwrap(), 0);

    // Annullamento prima del primo lotto.
    let a = Annullamento::new();
    a.annulla();
    let mut nuovo = IndiceVettoriale::new(ok.spazio(), "");
    assert_eq!(
        indicizza(&ok, &mut nuovo, &da, &a, |_, _| {}).map_err(|e| e.codice()),
        Err("annullato")
    );
}

#[test]
fn flusso_ibrido_dalla_richiesta_alla_prova() {
    let p = prezzario();
    let e = EmbeddingFinto::new();
    let iv = indice_vettoriale(&p, &e);
    let it = IndiceTestuale::new(&p);
    let modello = ModelloFinto {
        analisi: analisi_parete(),
        scelte: vec![("idropittura", lavorazione_parete())],
        ipotesi: vec![],
        chiamate: Mutex::default(),
    };
    let mut s = Sessione::nuovo("Prova");
    s.carica_prezzario("prova", p.clone());
    let testi_prima = *e.testi.lock().unwrap();
    let esito = proponi(
        &Strumenti {
            generazione: &modello,
            embedding: Some(&e),
            indice_testo: &it,
            indice_vettori: Some(&iv),
        },
        &Richiesta {
            testo: "Tinteggiatura di una parete di 4 x 3 m",
            nome_prezzario: "prova",
            prezzario: &p,
            progetto: s.progetto(),
            computo: None,
        },
        &Parametri::default(),
        &Annullamento::new(),
    )
    .unwrap();
    let EsitoFlusso::Proposta {
        candidati,
        ricerca_ibrida,
        proposta,
        consumi,
        ..
    } = esito
    else {
        panic!("attesa una proposta");
    };
    assert!(ricerca_ibrida);
    assert_eq!(
        *e.testi.lock().unwrap() - testi_prima,
        1,
        "un solo testo per la domanda"
    );
    assert_eq!(
        (
            consumi.token_ingresso,
            consumi.token_uscita,
            consumi.testi_embedding
        ),
        (200, 40, 1)
    );
    assert_eq!(
        candidati.elenco.first().map(|c| c.codice.as_str()),
        Some("T.01")
    );
    assert_eq!(
        proposta.esito,
        EsitoProposta::Completo,
        "{:?}",
        proposta.avvisi
    );
    let prova = verifica::prova(&s, &proposta, &proposta.scelte_predefinite()).unwrap();
    assert_eq!(
        prova.totali.first().and_then(|t| t.dopo),
        Some(Importo::new(dec!(86.40)))
    );

    // Le due chiamate: istruzioni, richiesta, nessun codice né prezzo nei candidati.
    let chiamate = modello.chiamate.lock().unwrap();
    assert_eq!(chiamate.len(), 2);
    assert!(chiamate[0].sistema.contains("Non aggiungere lavorazioni"));
    assert!(chiamate[0].utente.contains("4 x 3 m"));
    assert!(
        chiamate[1]
            .sistema
            .contains("- rettangolo: N, L, B → superficie")
    );
    let inviati = candidati_inviati(&chiamate[1].utente);
    assert!(
        inviati
            .iter()
            .all(|c| c.get("codice").is_none() && c.get("prezzo").is_none())
    );
    assert!(!chiamate[1].utente.contains("7,20") && !chiamate[1].utente.contains("7.20"));
}

#[test]
fn senza_indice_vettoriale_la_ricerca_e_testuale() {
    let p = prezzario();
    let it = IndiceTestuale::new(&p);
    let e = EmbeddingFinto::new();
    let iv_altro = indice_vettoriale(
        &p,
        &EmbeddingFinto {
            modello: "altro",
            ..EmbeddingFinto::new()
        },
    );
    let modello = ModelloFinto {
        analisi: analisi_parete(),
        scelte: vec![("idropittura", lavorazione_parete())],
        ipotesi: vec![],
        chiamate: Mutex::default(),
    };
    let s = Sessione::nuovo("Prova");
    for vettori in [None, Some(&iv_altro)] {
        let esito = proponi(
            &Strumenti {
                generazione: &modello,
                embedding: Some(&e),
                indice_testo: &it,
                indice_vettori: vettori,
            },
            &Richiesta {
                testo: "parete di 4 x 3 m da tinteggiare",
                nome_prezzario: "prova",
                prezzario: &p,
                progetto: s.progetto(),
                computo: None,
            },
            &Parametri::default(),
            &Annullamento::new(),
        )
        .unwrap();
        let EsitoFlusso::Proposta { ricerca_ibrida, .. } = esito else {
            panic!()
        };
        assert!(!ricerca_ibrida);
    }
    assert_eq!(
        *e.lotti.lock().unwrap(),
        0,
        "spazio diverso: nessuna chiamata di embedding"
    );
}

#[test]
fn senza_lavorazioni_il_flusso_chiede_altrimenti_propone_opzioni() {
    let p = prezzario();
    let it = IndiceTestuale::new(&p);
    let s = Sessione::nuovo("Prova");
    let esegui = |m: &ModelloFinto| {
        proponi(
            &Strumenti {
                generazione: m,
                embedding: None,
                indice_testo: &it,
                indice_vettori: None,
            },
            &Richiesta {
                testo: "rifare il pavimento della stanza",
                nome_prezzario: "prova",
                prezzario: &p,
                progetto: s.progetto(),
                computo: None,
            },
            &Parametri::default(),
            &Annullamento::new(),
        )
        .unwrap()
    };
    let domanda =
        json!({"domanda": "Quanto misura la stanza?", "bloccante": true, "lavorazioni": ["L01"]});
    // Nessuna lavorazione riconoscibile: domande, nessuna seconda chiamata.
    let m = ModelloFinto {
        analisi: json!({"esito": "Chiedi", "lavorazioni": [], "informazioni_mancanti": [domanda]}),
        scelte: vec![],
        ipotesi: vec![],
        chiamate: Mutex::default(),
    };
    let EsitoFlusso::DaChiarire { analisi, .. } = esegui(&m) else {
        panic!("attese domande")
    };
    assert_eq!(
        analisi.informazioni_mancanti[0].domanda,
        "Quanto misura la stanza?"
    );
    assert_eq!(
        m.chiamate.lock().unwrap().len(),
        1,
        "nessuna seconda chiamata"
    );

    // La lavorazione si riconosce ma manca il materiale e la misura: opzioni da scegliere,
    // misure da completare, la domanda resta accanto alla proposta.
    let m = ModelloFinto {
        analisi: json!({
            "esito": "chiedi",
            "lavorazioni": [{"id": "L01", "descrizione": "Pavimento", "query_ricerca": "pavimento", "dimensione_attesa": "superficie"}],
            "informazioni_mancanti": [domanda]
        }),
        scelte: vec![(
            "^pavimento",
            json!({
                "id": "L01", "descrizione": "Pavimento", "chiarezza": "da_precisare",
                "da_precisare": "materiale non indicato", "opzioni": [],
                "righe": [{"descrizione": "stanza", "forma": "superficie", "segno": "positivo", "parametri": [
                    {"nome": "N", "valore": "1", "unita": "n", "provenienza": "utente", "rif_ipotesi": ""},
                    {"nome": "A", "valore": "", "unita": "m2", "provenienza": "mancante", "rif_ipotesi": ""}
                ]}]
            }),
        )],
        ipotesi: vec![],
        chiamate: Mutex::default(),
    };
    let EsitoFlusso::Proposta { proposta, .. } = esegui(&m) else {
        panic!("attesa una proposta")
    };
    assert_eq!(m.chiamate.lock().unwrap().len(), 2);
    let l = &proposta.lavorazioni[0];
    assert!(!l.chiara && l.da_completare());
    let mut codici: Vec<&str> = l.opzioni.iter().map(|o| o.codice.as_str()).collect();
    codici.sort_unstable();
    assert_eq!(codici, ["P.01", "P.04", "P.05"]);
    assert!(proposta.scelte_predefinite().is_empty());
    assert_eq!(
        proposta.informazioni_mancanti[0].domanda,
        "Quanto misura la stanza?"
    );
}

#[test]
fn errori_del_flusso() {
    let p = prezzario();
    let it = IndiceTestuale::new(&p);
    let s = Sessione::nuovo("Prova");
    let esegui = |analisi: Value| {
        let m = ModelloFinto {
            analisi,
            scelte: vec![],
            ipotesi: vec![],
            chiamate: Mutex::default(),
        };
        proponi(
            &Strumenti {
                generazione: &m,
                embedding: None,
                indice_testo: &it,
                indice_vettori: None,
            },
            &Richiesta {
                testo: "x",
                nome_prezzario: "prova",
                prezzario: &p,
                progetto: s.progetto(),
                computo: None,
            },
            &Parametri::default(),
            &Annullamento::new(),
        )
    };
    assert!(matches!(
        esegui(json!({"esito": "forse", "lavorazioni": [], "informazioni_mancanti": []})),
        Err(ErroreFlusso::Analisi { .. })
    ));
    assert!(matches!(
        esegui(json!({"esito": "procedi", "lavorazioni": [], "informazioni_mancanti": []})),
        Err(ErroreFlusso::Analisi { .. })
    ));
    assert!(matches!(
        esegui(json!({"esito": "procedi"})),
        Err(ErroreFlusso::Analisi { .. })
    ));
    let l = json!({"id": "L01", "descrizione": "x", "query_ricerca": "x", "dimensione_attesa": "qualsiasi"});
    assert!(matches!(
        esegui(
            json!({"esito": "procedi", "lavorazioni": [l.clone(), l], "informazioni_mancanti": []})
        ),
        Err(ErroreFlusso::Analisi { .. })
    ));
    assert_eq!(
        esegui(
            json!({"esito": "procedi", "lavorazioni": [{"id": "L01", "descrizione": "elicottero", "query_ricerca": "elicottero", "dimensione_attesa": "qualsiasi"}], "informazioni_mancanti": []})
        ),
        Err(ErroreFlusso::NessunCandidato)
    );

    // Errore del provider: propagato con il suo codice.
    struct Rifiuta;
    impl Generazione for Rifiuta {
        fn genera(&self, _: &Chiamata, _: &Annullamento) -> Result<RispostaModello, ErroreAi> {
            Err(ErroreAi::Rifiuto)
        }
    }
    let e = proponi(
        &Strumenti {
            generazione: &Rifiuta,
            embedding: None,
            indice_testo: &it,
            indice_vettori: None,
        },
        &Richiesta {
            testo: "x",
            nome_prezzario: "prova",
            prezzario: &p,
            progetto: s.progetto(),
            computo: None,
        },
        &Parametri::default(),
        &Annullamento::new(),
    );
    assert_eq!(e, Err(ErroreFlusso::Ai(ErroreAi::Rifiuto)));
}
