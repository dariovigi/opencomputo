//! Risposte del modello, misure, testi e file di cache alterati a caso (sequenza
//! pseudo-casuale fissa, ripetibile). Qualunque cosa risponda il modello:
//!
//! * nessun errore irreversibile (panic), nessun overflow;
//! * le voci proposte sono solo voci del prezzario inviate come candidati;
//! * i prezzi nel computo sono quelli del prezzario, mai influenzati dalla risposta;
//! * le quantità vengono solo da righi completi, calcolati dal core.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::float_arithmetic
)]

use std::collections::BTreeMap;

use opencomputo_application::{Comando, Sessione};
use opencomputo_assistente::candidati::Candidati;
use opencomputo_assistente::proposta::{
    Contesto, OPZIONI_MASSIME, leggi_selezione, valida_selezione,
};
use opencomputo_assistente::ricerca::{FiltroVoci, IndiceTestuale};
use opencomputo_assistente::scelta::{Scelta, comandi, tutte};
use opencomputo_assistente::valori::estrai;
use opencomputo_assistente::verifica;
use opencomputo_assistente::vettori::{IndiceVettoriale, SpazioEmbedding, da_indicizzare};
use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_format_listino::{OpzioniListino, importa_csv};
use rust_decimal::Decimal;
use serde_json::{Value, json};

const CSV: &[u8] = include_bytes!("fixtures/prezzario_prova.csv");
const NOME: &str = "prova";
const RICHIESTA: &str =
    "Bagno: pavimento di 2,50 x 2,40 m, rivestimento alto 2,20 m su 9,80 m di perimetro, 1 vaso";

struct Caso(u64);

impl Caso {
    fn prossimo(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn fino_a(&mut self, n: usize) -> usize {
        (self.prossimo() % n.max(1) as u64) as usize
    }
    fn scegli<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.fino_a(v.len())]
    }
}

fn prezzario() -> Prezzario {
    importa_csv(
        CSV,
        &mut GeneratoreSequenziale::default(),
        &OpzioniListino::default(),
    )
    .unwrap()
    .prezzario
}

const STRINGHE: &[&str] = &[
    "",
    " ",
    "0",
    "-1",
    "1",
    "2.5",
    "2,5",
    "1e9",
    "99999999999999999999999999999",
    "0.0000000000000000000000000001",
    "79228162514264337593543950335",
    "NaN",
    "inf",
    "null",
    "c01",
    "c02",
    "c03",
    "C04",
    "c99",
    "c00",
    "NESSUNO",
    "L01",
    "L02",
    "l01",
    "I01",
    "R01",
    "utente",
    "ipotesi",
    "mancante",
    "UTENTE",
    "chiara",
    "da_precisare",
    "alta",
    "media",
    "bassa",
    "altissima",
    "positivo",
    "detrazione",
    "rettangolo",
    "superficie",
    "volume",
    "conteggio",
    "lunghezza",
    "cilindro",
    "a_corpo",
    "esagono",
    "N",
    "L",
    "B",
    "H",
    "A",
    "V",
    "P",
    "D",
    "X",
    "n",
    "m",
    "cm",
    "mm",
    "m2",
    "m3",
    "kg/m",
    "km",
    "€",
    "7,20 €",
    "prezzo 0,01",
    "T.01",
    "'; DROP TABLE voci;--",
    "<script>alert(1)</script>",
    "\u{0}",
    "\u{202e}destra",
    "🚧🏗️",
    "à́̂̃",
    "\n\n\n",
    "\"",
    "\\",
];

fn valido(c: &Candidati) -> Value {
    let h = |codice: &str| {
        c.elenco
            .iter()
            .find(|x| x.codice == codice)
            .unwrap()
            .handle
            .clone()
    };
    let p = |n: &str, v: &str, u: &str, prov: &str, rif: &str| json!({"nome": n, "valore": v, "unita": u, "provenienza": prov, "rif_ipotesi": rif});
    let op = |codice: &str, pert: &str| json!({"candidato": h(codice), "pertinenza": pert, "motivazione": "m"});
    json!({
        "lavorazioni": [
            {"id": "L01", "descrizione": "Pavimento", "chiarezza": "da_precisare", "da_precisare": "materiale",
             "righe": [{"descrizione": "bagno", "forma": "rettangolo", "segno": "positivo", "parametri": [
                 p("N", "1", "n", "utente", ""), p("L", "2.50", "m", "utente", ""), p("B", "2.40", "m", "utente", "")]}],
             "opzioni": [op("P.01", "alta"), op("P.04", "media"), op("P.05", "bassa")]},
            {"id": "L02", "descrizione": "Rivestimento", "chiarezza": "chiara", "da_precisare": "",
             "righe": [
                 {"descrizione": "pareti", "forma": "rettangolo", "segno": "positivo", "parametri": [
                     p("N", "1", "n", "utente", ""), p("L", "9.80", "m", "utente", ""), p("B", "2.20", "m", "utente", "")]},
                 {"descrizione": "porta", "forma": "rettangolo", "segno": "detrazione", "parametri": [
                     p("N", "1", "n", "utente", ""), p("L", "0.80", "m", "ipotesi", "I01"), p("B", "", "m", "mancante", "")]}],
             "opzioni": [op("P.02", "alta"), op("T.01", "bassa")]},
            {"id": "L03", "descrizione": "Vaso", "chiarezza": "chiara", "da_precisare": "",
             "righe": [{"descrizione": "vaso", "forma": "conteggio", "segno": "positivo", "parametri": [p("N", "1", "n", "utente", "")]}],
             "opzioni": [op("I.01", "alta"), op("O.01", "bassa"), op("S.02", "bassa")]}
        ],
        "ipotesi": [{"id": "I01", "testo": "porta larga 0,80 m", "impatto": "medio"}],
        "informazioni_mancanti": [{"domanda": "Altezza della porta?", "bloccante": false, "lavorazioni": ["L02"]}]
    })
}

/// Percorsi di tutte le foglie e di tutti i contenitori del JSON.
fn percorsi(v: &Value, qui: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    out.push(qui.clone());
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                qui.push(k.clone());
                percorsi(x, qui, out);
                qui.pop();
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                qui.push(i.to_string());
                percorsi(x, qui, out);
                qui.pop();
            }
        }
        _ => {}
    }
}

fn a<'a>(v: &'a mut Value, percorso: &[String]) -> Option<&'a mut Value> {
    let mut corrente = v;
    for k in percorso {
        corrente = match corrente {
            Value::Object(m) => m.get_mut(k)?,
            Value::Array(x) => x.get_mut(k.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(corrente)
}

fn altera(c: &mut Caso, v: &mut Value) {
    for _ in 0..=c.fino_a(5) {
        let mut tutti = Vec::new();
        percorsi(v, &mut Vec::new(), &mut tutti);
        let percorso = c.scegli(&tutti).clone();
        let scelta = c.fino_a(10);
        let stringa = (*c.scegli(STRINGHE)).to_owned();
        let Some(x) = a(v, &percorso) else { continue };
        match (scelta, &mut *x) {
            // Le stringhe si sostituiscono quasi sempre con un'altra stringa: la risposta
            // resta conforme e arriva alla validazione.
            (0..=6, Value::String(s)) => *s = stringa,
            (7, Value::String(s)) => s.push_str(&stringa.repeat(50)),
            (_, Value::Array(arr)) if !arr.is_empty() => {
                let i = c.fino_a(arr.len());
                match scelta % 3 {
                    0 => {
                        let copia = arr[i].clone();
                        arr.push(copia);
                    }
                    1 => {
                        arr.remove(i);
                    }
                    _ => arr.reverse(),
                }
            }
            (8, x) => *x = Value::Null,
            (9, x) => *x = json!(c.prossimo() as f64),
            _ => {}
        }
    }
}

#[test]
fn risposte_del_modello_alterate() {
    let p = prezzario();
    let codici = [
        "P.01", "P.04", "P.05", "P.02", "T.01", "I.01", "O.01", "S.02", "M.01",
    ];
    let v: Vec<(String, Vec<String>)> = codici
        .iter()
        .map(|c| ((*c).to_owned(), Vec::new()))
        .collect();
    let candidati = Candidati::da_codici(&p, &v);
    let mut sessione = Sessione::nuovo("Casuali");
    sessione.carica_prezzario(NOME, p.clone());
    let base = valido(&candidati);
    let mut c = Caso(0xDEAD_BEEF_CAFE_F00D);
    let (mut non_conformi, mut proposte, mut provate, mut eseguite) = (0, 0, 0, 0);
    for n in 0..3000 {
        let mut json = base.clone();
        if n > 0 {
            altera(&mut c, &mut json);
        }
        let Ok(risposta) = leggi_selezione(&json) else {
            non_conformi += 1;
            continue;
        };
        let ctx = Contesto {
            richiesta: RICHIESTA,
            nome_prezzario: NOME,
            prezzario: &p,
            candidati: &candidati,
            progetto: sessione.progetto(),
            computo: None,
        };
        let Ok(proposta) = valida_selezione(&risposta, &ctx) else {
            continue;
        };
        proposte += 1;
        for l in &proposta.lavorazioni {
            assert!(
                !l.opzioni.is_empty() && l.opzioni.len() <= OPZIONI_MASSIME,
                "{json}"
            );
            let preselezionate = l.opzioni.iter().filter(|o| o.preselezionata).count();
            assert!(preselezionate <= usize::from(l.chiara), "{json}");
            for o in &l.opzioni {
                let voce = p.voce_per_codice(&o.codice).expect("voce del prezzario");
                assert!(
                    codici.contains(&o.codice.as_str()),
                    "voce non tra i candidati: {json}"
                );
                assert_eq!(
                    o.prezzo, voce.prezzo,
                    "prezzo diverso dal prezzario: {json}"
                );
                assert_eq!(
                    candidati.per_handle(&o.handle).map(|x| x.codice.as_str()),
                    Some(o.codice.as_str())
                );
            }
            for r in &l.righe {
                assert_eq!(r.parametri.len(), r.forma.parametri().len(), "{json}");
                assert!(
                    r.parametri
                        .iter()
                        .all(|x| x.valore.is_none_or(|v| v > Decimal::ZERO)),
                    "{json}"
                );
            }
        }
        // Tutte le opzioni, poi quelle preselezionate, anche con misure a caso.
        let mut insiemi = vec![tutte(&proposta, &[]), proposta.scelte_predefinite()];
        let mut con_misure = tutte(&proposta, &[]);
        for (s, l) in con_misure.iter_mut().zip(&proposta.lavorazioni) {
            s.misure = Some(
                l.righe
                    .iter()
                    .map(|r| {
                        r.parametri
                            .iter()
                            .map(|x| (format!("{:?}", x.nome), (*c.scegli(STRINGHE)).to_owned()))
                            .collect::<BTreeMap<_, _>>()
                    })
                    .collect(),
            );
        }
        insiemi.push(con_misure);
        for scelte in insiemi {
            let _ = verifica::anteprima(&sessione, &proposta, &scelte);
            let Ok((cmd, voci)) = comandi(&proposta, &scelte, sessione.progetto()) else {
                continue;
            };
            // Nei comandi non c'è nessun prezzo: solo codici di candidati e misure.
            for x in &cmd {
                match x {
                    Comando::CopiaDaPrezzario {
                        prezzario,
                        codici: cc,
                    } => {
                        assert_eq!(prezzario, NOME);
                        assert!(cc.iter().all(|k| codici.contains(&k.as_str())));
                    }
                    Comando::AggiungiVoceComputo { codice, .. } => {
                        assert!(codici.contains(&codice.as_str()))
                    }
                    altro => panic!("comando inatteso: {altro:?}"),
                }
            }
            assert!(!serde_json::to_string(&cmd).unwrap().contains("\"prezzo\""));
            let Ok(esito) = sessione.prova(&cmd) else {
                continue;
            };
            provate += 1;
            assert!(!esito.applicato);
            // Si esegue davvero, si controllano i prezzi e si annulla.
            if n % 10 == 0 {
                sessione.esegui(&cmd).unwrap();
                let computo = &sessione.progetto().computi[0];
                assert_eq!(computo.voci.len(), voci.len());
                for o in &sessione.progetto().elenco_prezzi.voci {
                    assert_eq!(
                        Some(o.prezzo),
                        p.voce_per_codice(&o.codice).map(|x| x.prezzo)
                    );
                }
                sessione.annulla().unwrap();
                assert!(sessione.progetto().computi[0].voci.is_empty());
                eseguite += 1;
            }
        }
    }
    // Il test deve attraversare davvero tutti i percorsi.
    assert!(non_conformi > 200, "non conformi {non_conformi}");
    assert!(proposte > 1000, "proposte {proposte}");
    assert!(
        provate > 1500 && eseguite > 50,
        "provate {provate}, eseguite {eseguite}"
    );
}

#[test]
fn scelte_qualsiasi() {
    let p = prezzario();
    let v: Vec<(String, Vec<String>)> = [
        "P.01", "P.04", "P.05", "P.02", "T.01", "I.01", "O.01", "S.02",
    ]
    .iter()
    .map(|c| ((*c).to_owned(), Vec::new()))
    .collect();
    let candidati = Candidati::da_codici(&p, &v);
    let mut sessione = Sessione::nuovo("Scelte");
    sessione.carica_prezzario(NOME, p.clone());
    let ctx = Contesto {
        richiesta: RICHIESTA,
        nome_prezzario: NOME,
        prezzario: &p,
        candidati: &candidati,
        progetto: sessione.progetto(),
        computo: None,
    };
    let proposta = valida_selezione(&leggi_selezione(&valido(&candidati)).unwrap(), &ctx).unwrap();
    let mut c = Caso(7);
    let mut valide = 0;
    for _ in 0..3000 {
        let scelte: Vec<Scelta> = (0..c.fino_a(5))
            .map(|_| Scelta {
                lavorazione: (*c.scegli(&["L01", "L02", "L03", " l01 ", "L09", "", "L02"]))
                    .to_owned(),
                codici: (0..c.fino_a(4))
                    .map(|_| {
                        (*c.scegli(&[
                            "P.01", "P.04", "P.02", "T.01", "I.01", "O.01", "S.02", "X", "",
                        ]))
                        .to_owned()
                    })
                    .collect(),
                misure: (c.fino_a(3) == 0).then(|| {
                    (0..c.fino_a(3))
                        .map(|_| {
                            (0..c.fino_a(4))
                                .map(|_| {
                                    (
                                        (*c.scegli(STRINGHE)).to_owned(),
                                        (*c.scegli(STRINGHE)).to_owned(),
                                    )
                                })
                                .collect()
                        })
                        .collect()
                }),
            })
            .collect();
        let _ = verifica::anteprima(&sessione, &proposta, &scelte);
        if let Ok((cmd, _)) = comandi(&proposta, &scelte, sessione.progetto()) {
            valide += 1;
            let _ = sessione.prova(&cmd);
        }
    }
    assert!(valide > 20, "valide {valide}");
}

#[test]
fn testi_qualsiasi_nella_ricerca_e_nei_valori() {
    let p = prezzario();
    let indice = IndiceTestuale::new(&p);
    let mut c = Caso(99);
    let pezzi = [
        "massetto",
        "PAVIMENTO",
        "gres",
        " ",
        "\t",
        "4x3",
        "4 x 3 m",
        "2,50",
        "1.234,56 m²",
        "m³",
        "cm",
        "kg/m",
        "ø",
        "é",
        "🚧",
        "\u{0}",
        "\u{feff}",
        "123456789012345678901234567890",
        "-",
        ",",
        ".",
        "x",
        "×",
        "per",
        "di",
        "mq",
        "ml",
        "n.",
        "°",
        "\u{202e}",
        "ｍ",
        "²",
    ];
    for _ in 0..5000 {
        let testo: String = (0..c.fino_a(12))
            .map(|_| *c.scegli(&pezzi))
            .collect::<Vec<_>>()
            .join(if c.fino_a(2) == 0 { " " } else { "" });
        for v in estrai(&testo) {
            assert!(
                testo.contains(&v.testo),
                "«{}» non è nel testo «{testo}»",
                v.testo
            );
        }
        let k = c.fino_a(10);
        let r = indice.cerca(&testo, k, &FiltroVoci::default());
        assert!(r.len() <= k);
        assert!(
            r.iter()
                .all(|x| p.voce_per_codice(&x.codice).is_some() && x.punteggio > 0)
        );
        assert!(
            r.windows(2).all(|w| w[0].punteggio >= w[1].punteggio),
            "ordine dei punteggi"
        );
    }
    // Un testo molto lungo non fa esplodere tempi né memoria.
    let lungo = "massetto in sabbia e cemento di 4 x 3 m ".repeat(20_000);
    assert!(!estrai(&lungo).is_empty());
    assert!(!indice.cerca(&lungo, 5, &FiltroVoci::default()).is_empty());
}

#[test]
fn file_di_cache_danneggiati() {
    let p = prezzario();
    let da = da_indicizzare(&p, 8_000);
    let spazio = SpazioEmbedding::new("modello", 4);
    let mut indice = IndiceVettoriale::new(spazio.clone(), "impronta");
    for (i, v) in da.iter().enumerate() {
        indice.registra(v, vec![i as f32, 1.0, -0.5, 0.25]).unwrap();
    }
    let buono = indice.codifica();
    assert_eq!(
        IndiceVettoriale::decodifica(&buono, &spazio).unwrap(),
        indice
    );
    // Lettura a flusso con pezzi piccoli: stesso risultato.
    struct APezzi<'a>(&'a [u8]);
    impl std::io::Read for APezzi<'_> {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            let n = b.len().min(3).min(self.0.len());
            b[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    assert_eq!(
        IndiceVettoriale::leggi(&mut APezzi(&buono), &spazio).unwrap(),
        indice
    );

    let mut c = Caso(2026);
    let mut letti = 0;
    // Ogni troncamento è rifiutato.
    for fine in (0..buono.len()).step_by(37) {
        assert!(
            IndiceVettoriale::decodifica(&buono[..fine], &spazio).is_err(),
            "troncato a {fine}"
        );
    }
    for _ in 0..3000 {
        let mut b = buono.clone();
        for _ in 0..=c.fino_a(3) {
            let i = c.fino_a(b.len().max(1));
            match c.fino_a(4) {
                0 if !b.is_empty() => b[i] = c.prossimo() as u8,
                1 => b.truncate(i),
                2 => b.extend((0..c.fino_a(64)).map(|_| 0xAB)),
                3 if !b.is_empty() => {
                    // Il numero di voci o di dimensioni dichiarato nella testata.
                    let fine = (i + 4).min(b.len());
                    b[i..fine].fill(0xFF);
                }
                _ => {}
            }
        }
        if let Ok(letto) = IndiceVettoriale::decodifica(&b, &spazio) {
            letti += 1;
            assert_eq!(letto.spazio(), &spazio);
            assert!(letto.len() <= da.len());
            // Un indice letto si può sempre usare: cercare, riscrivere, rileggere.
            let _ = letto.cerca(&[1.0, 0.0, 0.0, 0.0], 5, |_| true);
            assert_eq!(
                IndiceVettoriale::decodifica(&letto.codifica(), &spazio).unwrap(),
                letto
            );
        }
    }
    assert!(letti > 100, "letti {letti}");
    // Testata che dichiara milioni di voci in un file di pochi byte: rifiutata subito,
    // senza allocare la memoria dichiarata.
    let testata = br#"{"spazio":{"modello":"modello","dimensioni":4,"versione_testo":1},"impronta_prezzario":"","voci":1999999}"#;
    let mut finto = b"OCVEC".to_vec();
    finto.extend(1u16.to_le_bytes());
    finto.extend((testata.len() as u32).to_le_bytes());
    finto.extend_from_slice(testata);
    assert!(IndiceVettoriale::decodifica(&finto, &spazio).is_err());
    finto.splice(7..11, u32::MAX.to_le_bytes());
    assert!(IndiceVettoriale::decodifica(&finto, &spazio).is_err());
}
