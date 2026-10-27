//! Schemi JSON delle risposte del modello e definizioni degli strumenti.
//!
//! Gli schemi sono pensati per il tool use di Anthropic con `strict: true` (e valgono
//! anche per `output_config.format`), che accetta un sottoinsieme di JSON Schema:
//! ogni oggetto ha `additionalProperties: false`; niente vincoli numerici o di
//! lunghezza; al massimo 24 parametri facoltativi e 16 con unioni di tipo per richiesta.
//! Questi schemi non hanno **nessun** parametro facoltativo e **nessuna** unione: un
//! test lo verifica. I numeri viaggiano come testo con un `pattern`, così Rust li legge
//! in decimale esatto.
//!
//! Gli elenchi dei valori ammessi vengono dal codice ([`Forma::TUTTE`], parametri,
//! unità): schema e validazione non possono divergere.

use serde_json::{Value, json};

use crate::forme::Forma;

/// Nome dello strumento della prima chiamata.
pub const STRUMENTO_ANALISI: &str = "registra_analisi";
/// Nome dello strumento della seconda chiamata.
pub const STRUMENTO_SELEZIONE: &str = "registra_selezione";

/// Nomi dei parametri delle forme.
pub const PARAMETRI: [&str; 10] = ["N", "L", "B", "H", "D", "B1", "B2", "A", "V", "P"];
/// Unità dei parametri.
pub const UNITA: [&str; 7] = ["n", "mm", "cm", "m", "m2", "m3", "kg/m"];
/// Dimensioni attese nell'analisi.
pub const DIMENSIONI_ATTESE: [&str; 7] = [
    "lunghezza",
    "superficie",
    "volume",
    "massa",
    "numero",
    "a_corpo",
    "qualsiasi",
];
/// Pattern dei valori decimali (vuoto se il valore manca).
pub const PATTERN_VALORE: &str = "^([0-9]+([.][0-9]+)?)?$";
/// Pattern degli handle dei candidati.
pub const PATTERN_CANDIDATO: &str = "^c[0-9]{2}$";
/// Opzioni al massimo per lavorazione.
pub const OPZIONI_MASSIME: usize = 5;
/// Pattern degli identificativi delle lavorazioni.
pub const PATTERN_LAVORAZIONE: &str = "^L[0-9]{2}$";
/// Pattern degli identificativi delle ipotesi.
pub const PATTERN_IPOTESI: &str = "^I[0-9]{2}$";

/// Oggetto con tutti i campi obbligatori e nessun campo aggiuntivo.
fn oggetto(campi: &[(&str, Value)]) -> Value {
    let properties: serde_json::Map<String, Value> = campi
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    let required: Vec<&str> = campi.iter().map(|(k, _)| *k).collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties,
    })
}

fn testo(descrizione: &str) -> Value {
    json!({"type": "string", "description": descrizione})
}

fn scelta(valori: &[&str], descrizione: &str) -> Value {
    json!({"type": "string", "enum": valori, "description": descrizione})
}

fn elenco(elementi: Value) -> Value {
    json!({"type": "array", "items": elementi})
}

fn informazioni_mancanti() -> Value {
    elenco(oggetto(&[
        ("domanda", testo("Domanda da porre all'utente, in italiano")),
        (
            "bloccante",
            json!({"type": "boolean", "description": "true se senza risposta il computo non è attendibile"}),
        ),
        (
            "lavorazioni",
            elenco(json!({"type": "string", "pattern": PATTERN_LAVORAZIONE})),
        ),
    ]))
}

/// Schema della risposta di analisi.
#[must_use]
pub fn schema_analisi() -> Value {
    oggetto(&[
        (
            "esito",
            scelta(&["procedi", "chiedi"], "chiedi se mancano dati bloccanti"),
        ),
        (
            "lavorazioni",
            elenco(oggetto(&[
                (
                    "id",
                    json!({"type": "string", "pattern": PATTERN_LAVORAZIONE}),
                ),
                (
                    "descrizione",
                    testo("Lavorazione richiesta, senza inventare dettagli"),
                ),
                (
                    "query_ricerca",
                    testo("Parole chiave per cercare la voce nel prezzario"),
                ),
                (
                    "dimensione_attesa",
                    scelta(
                        &DIMENSIONI_ATTESE,
                        "Grandezza con cui la lavorazione si misura",
                    ),
                ),
            ])),
        ),
        ("informazioni_mancanti", informazioni_mancanti()),
    ])
}

/// Schema della risposta di selezione.
#[must_use]
pub fn schema_selezione() -> Value {
    let forme: Vec<&str> = Forma::TUTTE.iter().map(|f| f.nome()).collect();
    let parametro = oggetto(&[
        ("nome", scelta(&PARAMETRI, "Parametro della forma")),
        (
            "valore",
            json!({"type": "string", "pattern": PATTERN_VALORE, "description": "Numero con il punto decimale; vuoto se il valore manca"}),
        ),
        ("unita", scelta(&UNITA, "Unità del valore")),
        (
            "provenienza",
            scelta(
                &["utente", "ipotesi", "mancante"],
                "utente solo se il valore è scritto nella richiesta; mancante se non è indicato e non è ragionevole ipotizzarlo",
            ),
        ),
        (
            "rif_ipotesi",
            testo("Identificativo dell'ipotesi se la provenienza è ipotesi, altrimenti vuoto"),
        ),
    ]);
    let riga = oggetto(&[
        ("descrizione", testo("Descrizione del rigo di misura")),
        ("forma", scelta(&forme, "Forma geometrica del rigo")),
        (
            "segno",
            scelta(
                &["positivo", "detrazione"],
                "detrazione per vuoti da togliere",
            ),
        ),
        ("parametri", elenco(parametro)),
    ]);
    let opzione = oggetto(&[
        (
            "candidato",
            json!({"type": "string", "pattern": PATTERN_CANDIDATO, "description": "Handle di un candidato fornito"}),
        ),
        (
            "pertinenza",
            scelta(
                &["alta", "media", "bassa"],
                "Quanto la voce corrisponde alla lavorazione",
            ),
        ),
        (
            "motivazione",
            testo("Perché la voce è adatta, in una frase breve"),
        ),
    ]);
    oggetto(&[
        (
            "lavorazioni",
            elenco(oggetto(&[
                (
                    "id",
                    json!({"type": "string", "pattern": PATTERN_LAVORAZIONE}),
                ),
                ("descrizione", testo("Lavorazione")),
                (
                    "chiarezza",
                    scelta(
                        &["chiara", "da_precisare"],
                        "da_precisare se manca un'informazione per scegliere tra le voci",
                    ),
                ),
                (
                    "da_precisare",
                    testo("Cosa manca per scegliere la voce (vuoto se chiara)"),
                ),
                ("righe", elenco(riga)),
                ("opzioni", elenco(opzione)),
            ])),
        ),
        (
            "ipotesi",
            elenco(oggetto(&[
                ("id", json!({"type": "string", "pattern": PATTERN_IPOTESI})),
                ("testo", testo("Ipotesi fatta per un dato mancante")),
                (
                    "impatto",
                    scelta(&["basso", "medio", "alto"], "Effetto sul risultato"),
                ),
            ])),
        ),
        ("informazioni_mancanti", informazioni_mancanti()),
    ])
}

fn strumento(nome: &str, descrizione: &str, schema: Value) -> Value {
    json!({
        "name": nome,
        "description": descrizione,
        "strict": true,
        "input_schema": schema,
    })
}

/// Definizione dello strumento di analisi per l'API Messages di Anthropic.
#[must_use]
pub fn strumento_analisi() -> Value {
    strumento(
        STRUMENTO_ANALISI,
        "Registra le lavorazioni richieste, le parole da cercare nel prezzario e le informazioni mancanti.",
        schema_analisi(),
    )
}

/// Definizione dello strumento di selezione per l'API Messages di Anthropic.
#[must_use]
pub fn strumento_selezione() -> Value {
    strumento(
        STRUMENTO_SELEZIONE,
        "Registra per ogni lavorazione i righi di misura e le voci possibili in ordine di pertinenza, le ipotesi e le informazioni mancanti.",
        schema_selezione(),
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::risposta::{RispostaAnalisi, RispostaSelezione};

    /// Conta campi facoltativi e unioni, e verifica le regole degli oggetti.
    fn visita(v: &Value, percorso: &str, facoltativi: &mut usize, unioni: &mut usize) {
        let Value::Object(o) = v else { return };
        for vietata in [
            "anyOf",
            "oneOf",
            "allOf",
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "$ref",
        ] {
            assert!(
                !o.contains_key(vietata),
                "{percorso}: «{vietata}» non ammesso"
            );
        }
        if o.get("type").is_some_and(Value::is_array) {
            *unioni += 1;
        }
        if o.get("type") == Some(&json!("object")) {
            assert_eq!(
                o.get("additionalProperties"),
                Some(&json!(false)),
                "{percorso}"
            );
            let props = o.get("properties").and_then(Value::as_object).unwrap();
            let req: Vec<&str> = o
                .get("required")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect();
            *facoltativi += props.keys().filter(|k| !req.contains(&k.as_str())).count();
            for (k, figlio) in props {
                visita(figlio, &format!("{percorso}.{k}"), facoltativi, unioni);
            }
        }
        if let Some(items) = o.get("items") {
            visita(items, &format!("{percorso}[]"), facoltativi, unioni);
        }
    }

    #[test]
    fn schemi_nei_limiti_degli_structured_outputs() {
        let (mut f, mut u) = (0, 0);
        visita(&schema_analisi(), "analisi", &mut f, &mut u);
        visita(&schema_selezione(), "selezione", &mut f, &mut u);
        assert_eq!((f, u), (0, 0));
        assert_eq!(strumento_selezione()["strict"], json!(true));
        assert_eq!(strumento_analisi()["name"], json!("registra_analisi"));
    }

    #[test]
    fn esempi_conformi_si_leggono() {
        let a: RispostaAnalisi = serde_json::from_value(json!({
            "esito": "procedi",
            "lavorazioni": [{"id": "L01", "descrizione": "Massetto", "query_ricerca": "massetto sabbia cemento", "dimensione_attesa": "superficie"}],
            "informazioni_mancanti": []
        }))
        .unwrap();
        assert_eq!(a.lavorazioni.len(), 1);
        let s: RispostaSelezione = serde_json::from_value(json!({
            "lavorazioni": [{
                "id": "L01", "descrizione": "Massetto", "chiarezza": "chiara", "da_precisare": "",
                "righe": [{"descrizione": "soggiorno", "forma": "rettangolo", "segno": "positivo", "parametri": [
                    {"nome": "N", "valore": "1", "unita": "n", "provenienza": "utente", "rif_ipotesi": ""},
                    {"nome": "L", "valore": "5", "unita": "m", "provenienza": "utente", "rif_ipotesi": ""},
                    {"nome": "B", "valore": "", "unita": "m", "provenienza": "mancante", "rif_ipotesi": ""}
                ]}],
                "opzioni": [{"candidato": "c01", "pertinenza": "alta", "motivazione": "voce del capitolo massetti"}]
            }],
            "ipotesi": [],
            "informazioni_mancanti": []
        }))
        .unwrap();
        assert_eq!(s.lavorazioni.first().map(|l| l.righe.len()), Some(1));
        // Un campo in più è rifiutato anche lato Rust.
        assert!(
            serde_json::from_value::<RispostaAnalisi>(json!({
                "esito": "procedi", "lavorazioni": [], "informazioni_mancanti": [], "extra": 1
            }))
            .is_err()
        );
    }
}
