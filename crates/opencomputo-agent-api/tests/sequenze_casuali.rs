//! Sequenze casuali di comandi (generatore fisso, quindi ripetibili) sulla `Sessione`:
//! gli esempi documentati di tutti i comandi e quelli di un progetto completo, con valori
//! cambiati a caso (numeri limite, testi vuoti, date impossibili, codici inesistenti),
//! alternati ad annulla e ripristina.
//!
//! Dopo ogni passo si controlla che:
//! * nessun comando termini il processo (panic);
//! * una richiesta rifiutata lasci il progetto com'era (atomicità) e la prova dia lo
//!   stesso esito dell'esecuzione;
//! * problemi e totali conservati dalla sessione siano quelli del progetto corrente;
//! * annulla e ripristina riportino esattamente agli stati precedenti;
//! * il progetto si salvi e si rilegga uguale, e viste e report si producano.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use opencomputo_agent_api::schema::schema_comandi;
use opencomputo_application::casi_uso::{self, FormatoReport, TipoReport};
use opencomputo_application::sessione::ImportiComputo;
use opencomputo_application::{Comando, Politica, Sessione, viste};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::schema::{esporta_json, importa_json};
use opencomputo_core::validazione::valida_progetto;
use opencomputo_io::report::OpzioniReport;
use serde_json::{Value, json};

/// Generatore xorshift: stessa sequenza a ogni esecuzione.
struct Caso(u64);

impl Caso {
    fn prossimo(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn fino_a(&mut self, n: usize) -> usize {
        usize::try_from(self.prossimo() % u64::try_from(n.max(1)).unwrap()).unwrap()
    }
    fn uno<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.fino_a(v.len())]
    }
    fn su(&mut self, n: usize) -> bool {
        self.fino_a(n) == 0
    }
}

/// Comandi di un progetto completo: elenco, computo con formule e «vedi voce»,
/// contabilità con SAL.
const BASE: &str = r#"[
  {"comando": "imposta_metadati", "titolo": "Nuova scuola", "committente": "Comune di Esempio"},
  {"comando": "aggiungi_voce_prezzario", "codice": "A.01.001", "descrizione_breve": "Scavo di sbancamento", "unita_misura": "m³", "prezzo": "8.50", "incidenza_manodopera": "25"},
  {"comando": "aggiungi_voce_prezzario", "codice": "B.02.010", "descrizione_breve": "Calcestruzzo", "unita_misura": "m³", "prezzo": "145.30", "incidenza_manodopera": "18.5"},
  {"comando": "aggiungi_voce_prezzario", "codice": "C.03.020", "descrizione_breve": "Muratura", "unita_misura": "m²", "prezzo": "42.75"},
  {"comando": "aggiungi_voce_prezzario", "codice": "S.01.001", "descrizione_breve": "Recinzione", "unita_misura": "m", "prezzo": "12.00", "natura": "oneri_sicurezza"},
  {"comando": "aggiungi_voce_computo", "codice": "A.01.001", "righi": [
    {"descrizione": "Platea", "lunghezza": "12.00", "larghezza": "8.00", "altezza_peso": "1.20"},
    {"descrizione": "Rampa", "lunghezza": 6, "larghezza": 3, "altezza_peso": "1,20", "formula": "0,5*lun*lar*alt"}]},
  {"comando": "aggiungi_voce_computo", "codice": "B.02.010", "righi": [
    {"descrizione": "Cordoli", "parti_uguali": 4, "lunghezza": "7,50+0,30", "larghezza": "0.30", "altezza_peso": "0.40"},
    {"descrizione": "Come la platea", "vedi_voce": 1, "altezza_peso": "0.5"}]},
  {"comando": "aggiungi_voce_computo", "codice": "C.03.020", "righi": [
    {"descrizione": "Piano terra", "commento": true},
    {"descrizione": "Pareti", "parti_uguali": 2, "lunghezza": 12, "altezza_peso": 3},
    {"descrizione": "A dedurre", "parti_uguali": 6, "lunghezza": "1.20", "altezza_peso": "1.50", "deduzione": true}]},
  {"comando": "aggiungi_voce_computo", "codice": "S.01.001", "righi": [{"lunghezza": "2*(30+20)"}]},
  {"comando": "imposta_contratto", "ribasso": "12.5", "ritenuta_garanzia": "0.5", "importo_contrattuale": "100000", "data_consegna_lavori": "2026-03-01"},
  {"comando": "registra", "data": "2026-03-10", "codice": "A.01.001", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1.2"}]},
  {"comando": "registra", "data": "2026-03-25", "codice": "S.01.001", "righi": [{"lunghezza": 100}]},
  {"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-03-31"},
  {"comando": "aggiungi_capitolo", "codice": "A", "descrizione": "Opere"},
  {"comando": "aggiungi_capitolo", "codice": "A.01", "descrizione": "Scavi", "padre": "A"},
  {"comando": "aggiungi_categoria", "livello": "categoria", "codice": "ST", "descrizione": "Strutture"},
  {"comando": "aggiungi_nodo_wbs", "codice": "01", "descrizione": "Edificio"},
  {"comando": "aggiungi_nodo_wbs", "codice": "01", "descrizione": "Scavi", "padre": "01"},
  {"comando": "aggiungi_nodo_wbs", "codice": "02", "descrizione": "Fondazioni", "padre": "01"},
  {"comando": "aggiungi_nodo_wbs", "codice": "03", "descrizione": "Elevazioni", "padre": "01"},
  {"comando": "imposta_risorsa", "codice": "MA.20", "descrizione": "Acciaio", "unita_misura": "kg", "tipo": "materiale", "prezzo": "1.10"},
  {"comando": "imposta_risorsa", "codice": "MO.01", "descrizione": "Operaio specializzato", "unita_misura": "h", "tipo": "manodopera", "prezzo": "36.00"},
  {"comando": "imposta_risorsa", "codice": "MO.02", "descrizione": "Operaio comune", "unita_misura": "h", "tipo": "manodopera", "prezzo": "32.00"},
  {"comando": "aggiungi_voce_prezzario", "codice": "NP.01", "descrizione_breve": "Nuovo prezzo 1", "unita_misura": "m²", "prezzo": "20.00", "capitolo": "A.01"},
  {"comando": "aggiungi_voce_prezzario", "codice": "NP.02", "descrizione_breve": "Nuovo prezzo 2", "unita_misura": "kg", "prezzo": "3.10"}
]"#;

/// Valori con cui sostituire i numeri.
const INTERI: &[i64] = &[0, 1, 2, 3, 4, 5, 7, -1, 1000, 1_000_000_000, i64::MAX];

/// Valori con cui sostituire i testi: numeri limite, espressioni sbagliate, date
/// impossibili, codici presenti e assenti.
const TESTI: &[&str] = &[
    "",
    " ",
    "0",
    "-5",
    "100",
    "0.001",
    "1,5",
    "1e30",
    "99999999999999999999999999.99",
    "79228162514264337593543950335",
    "-79228162514264337593543950335",
    "0.0000000000000000000000000001",
    "2*(3+",
    "1/0",
    "lun*lar*",
    "2026-02-30",
    "2026-13-01",
    "9999-12-31",
    "-0001-01-01",
    "2026-03-15",
    "2027-01-01",
    "A.01.001",
    "B.02.010",
    "S.01.001",
    "XYZ",
    "A",
    "A.01",
    "ST",
    "01",
    "01.02",
    "Computo metrico estimativo",
    "regionale",
    "<script>alert(1)</script>",
    "Ä\u{0}\u{202e}ž",
];

/// Cambia a caso alcuni valori (anche annidati) di un comando.
fn altera(c: &mut Caso, v: &mut Value, profondita: usize) {
    match v {
        Value::Object(m) => {
            let chiavi: Vec<String> = m.keys().filter(|k| *k != "comando").cloned().collect();
            for k in chiavi {
                if c.su(12) {
                    m.remove(&k);
                } else if let Some(x) = m.get_mut(&k) {
                    altera(c, x, profondita + 1);
                }
            }
        }
        Value::Array(a) => {
            if !a.is_empty() && c.su(8) {
                let i = c.fino_a(a.len());
                a.remove(i);
            }
            if !a.is_empty() && c.su(8) && profondita < 6 {
                let i = c.fino_a(a.len());
                let copia = a[i].clone();
                a.push(copia);
            }
            for x in a {
                altera(c, x, profondita + 1);
            }
        }
        Value::Number(_) if c.su(4) => *v = json!(*c.uno(INTERI)),
        Value::String(_) if c.su(4) => *v = json!(*c.uno(TESTI)),
        Value::Bool(b) if c.su(4) => *b = !*b,
        _ => {}
    }
}

fn importi(p: &Progetto, i: usize) -> Result<ImportiComputo, String> {
    p.calcola_computo(p.computi[i].id)
        .map(|r| ImportiComputo::from(&r))
        .map_err(|e| e.to_string())
}

/// Prezzari di consultazione citati dagli esempi: le voci dell'elenco con altri codici.
fn carica_prezzari(s: &mut Sessione) {
    for (nome, codici) in [
        ("regionale", ["E.01.010", "E.02.020"]),
        ("regionale 2027", ["A.01.001", "E.01.010"]),
    ] {
        let mut r = s.progetto().elenco_prezzi.clone();
        r.capitoli.clear();
        for (v, codice) in r.voci.iter_mut().zip(codici) {
            let v = Arc::make_mut(v);
            v.codice = codice.into();
            v.capitolo = None;
        }
        for v in r.voci.iter_mut().skip(2) {
            Arc::make_mut(v).capitolo = None;
        }
        s.carica_prezzario(nome, r);
    }
}

/// Problemi e totali conservati dalla sessione = quelli calcolati da capo.
fn controlla_stato(s: &Sessione, passo: &str) {
    let p = s.progetto();
    assert_eq!(
        s.problemi(),
        valida_progetto(p).problemi.as_slice(),
        "problemi non aggiornati dopo {passo}"
    );
    let totali = s.totali_computi();
    assert_eq!(totali.len(), p.computi.len(), "{passo}");
    for (i, t) in totali.iter().enumerate() {
        assert_eq!(t.computo, p.computi[i].id, "{passo}");
        assert_eq!(
            t.totali,
            importi(p, i),
            "totali non aggiornati dopo {passo}"
        );
    }
}

/// Andata e ritorno nel formato del file, viste e un report.
fn controlla_uscite(p: &Progetto, passo: &str, report: bool) {
    let testo = esporta_json(p).unwrap();
    assert_eq!(
        &importa_json(&testo).unwrap(),
        p,
        "rilettura diversa dopo {passo}"
    );
    let _ = viste::progetto(p);
    let _ = viste::elenco(&p.elenco_prezzi, None);
    let _ = viste::contabilita(p);
    let _ = viste::analisi_prezzi(p);
    let _ = viste::nuovi_prezzi(p);
    let _ = viste::cronoprogramma(p);
    for c in &p.computi {
        let _ = viste::computo(p, Some(&c.nome));
        let _ = viste::fabbisogno(p, Some(&c.nome));
    }
    if report {
        let o = OpzioniReport::default();
        for tipo in [
            json!({"tipo": "computo"}),
            json!({"tipo": "computo", "tradizionale": {}}),
            json!({"tipo": "elenco_prezzi"}),
            json!({"tipo": "libretto"}),
        ] {
            let tipo: TipoReport = serde_json::from_value(tipo).unwrap();
            let _ = casi_uso::report(p, &tipo, FormatoReport::Pdf, &o);
            let _ = casi_uso::report(p, &tipo, FormatoReport::Xlsx, &o);
        }
    }
}

fn sequenza(seme: u64, passi: usize, permissiva: bool) {
    let mut c = Caso(seme);
    let esempi: Vec<Value> = schema_comandi()["comandi"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["esempio"].clone())
        .collect();
    let base: Vec<Value> = serde_json::from_str(BASE).unwrap();
    let pool: Vec<Value> = esempi.iter().chain(&base).cloned().collect();

    let politica = Politica {
        rifiuta_nuovi_errori: !permissiva,
        limite_annulla: 10_000,
    };
    let mut s = Sessione::nuovo("Prova casuale").con_politica(politica);
    let comandi: Vec<Comando> = serde_json::from_value(Value::Array(base)).unwrap();
    // Stati dopo ogni richiesta eseguita (per annulla) e stati annullati (per ripristina):
    // il primo è il progetto vuoto.
    let mut storia = vec![s.progetto().clone()];
    s.esegui(&comandi).unwrap();
    storia.push(s.progetto().clone());
    carica_prezzari(&mut s);
    let mut annullati: Vec<Progetto> = Vec::new();
    let mut registro: Vec<String> = Vec::new();
    let (mut eseguiti, mut rifiutati) = (0, 0);

    for n in 0..passi {
        let esito = catch_unwind(AssertUnwindSafe(|| {
            match c.fino_a(14) {
                0 => {
                    registro.push("annulla".into());
                    let r = s.annulla();
                    if storia.len() > 1 {
                        r.unwrap();
                        annullati.push(storia.pop().unwrap());
                        assert_eq!(s.progetto(), storia.last().unwrap(), "annulla");
                    } else {
                        assert!(r.is_err());
                    }
                }
                1 => {
                    registro.push("ripristina".into());
                    let r = s.ripristina();
                    if let Some(p) = annullati.pop() {
                        r.unwrap();
                        assert_eq!(s.progetto(), &p, "ripristina");
                        storia.push(p);
                    } else {
                        assert!(r.is_err());
                    }
                }
                _ => {
                    let quanti = if c.su(4) { 2 + c.fino_a(3) } else { 1 };
                    let lotto: Vec<Value> = (0..quanti)
                        .map(|_| {
                            let mut v = c.uno(&pool).clone();
                            if c.su(2) {
                                altera(&mut c, &mut v, 0);
                            }
                            v
                        })
                        .collect();
                    let testo = Value::Array(lotto.clone()).to_string();
                    registro.push(testo.clone());
                    let Ok(comandi) = serde_json::from_value::<Vec<Comando>>(Value::Array(lotto))
                    else {
                        rifiutati += 1;
                        return;
                    };
                    let prima = s.progetto().clone();
                    let prova = s.prova(&comandi);
                    match s.esegui(&comandi) {
                        Ok(e) => {
                            eseguiti += 1;
                            let p = prova.expect("la prova rifiuta ciò che l'esecuzione accetta");
                            assert_eq!(p.nuovi_problemi, e.nuovi_problemi, "prova diversa");
                            assert_eq!(p.problemi_risolti, e.problemi_risolti, "prova diversa");
                            assert_eq!(p.totali, e.totali, "prova diversa");
                            storia.push(s.progetto().clone());
                            annullati.clear();
                        }
                        Err(_) => {
                            rifiutati += 1;
                            assert!(
                                prova.is_err(),
                                "la prova accetta ciò che l'esecuzione rifiuta"
                            );
                            assert_eq!(s.progetto(), &prima, "richiesta rifiutata ma applicata");
                        }
                    }
                }
            }
            let ultimo = registro.last().cloned().unwrap_or_default();
            controlla_stato(&s, &ultimo);
            if n % 10 == 9 {
                controlla_uscite(s.progetto(), &ultimo, n % 50 == 49);
            }
        }));
        if let Err(e) = esito {
            let messaggio = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_default();
            let ultimi: Vec<&String> = registro.iter().rev().take(5).collect();
            panic!(
                "seme {seme}, passo {n}: {messaggio}\nultimi passi (dal più recente):\n{ultimi:#?}"
            );
        }
    }
    controlla_uscite(s.progetto(), "fine", true);
    eprintln!(
        "seme {seme}: {eseguiti} richieste eseguite, {rifiutati} rifiutate, {} voci, {} registrazioni",
        s.progetto()
            .computi
            .iter()
            .map(|c| c.voci.len())
            .sum::<usize>(),
        s.progetto().contabilita.libretto().registrazioni.len()
    );
    // La sequenza deve aver fatto davvero qualcosa in entrambi i sensi.
    assert!(
        eseguiti > passi / 20,
        "seme {seme}: solo {eseguiti} richieste eseguite"
    );
    assert!(rifiutati > 0, "seme {seme}: nessuna richiesta rifiutata");
}

/// Semi da provare: 4, o quanti indicati in `OPENCOMPUTO_SEMI` per una campagna lunga.
fn semi(primo: u64) -> impl Iterator<Item = u64> {
    let quanti = std::env::var("OPENCOMPUTO_SEMI")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(4);
    (primo..primo + quanti).map(|s| 0x9E37_79B9_7F4A_7C15 ^ s.wrapping_mul(0x2545_F491))
}

#[test]
fn sequenze_con_politica_predefinita() {
    for seme in semi(1) {
        sequenza(seme, 250, false);
    }
}

#[test]
fn sequenze_con_politica_permissiva() {
    for seme in semi(1_000_000) {
        sequenza(seme, 250, true);
    }
}
