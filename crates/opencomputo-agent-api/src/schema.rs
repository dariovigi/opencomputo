//! Guida ai comandi per gli agenti: regole generali ed esempi validi di ogni comando.
//!
//! Gli esempi sono verificati dai test: ciascuno si deserializza in un
//! [`Comando`](opencomputo_application::Comando).

use serde_json::{Value, json};

/// Regole generali dei comandi.
pub const REGOLE: &[&str] = &[
    "Una richiesta è una lista di comandi JSON con il campo \"comando\"; i campi sconosciuti sono rifiutati.",
    "La richiesta è atomica: se un comando fallisce non viene applicato nessuno dei comandi.",
    "Sono rifiutate le richieste che introducono nuovi errori di validazione; gli avvisi sono riportati nell'esito.",
    "Prezzi, quantità e percentuali si scrivono preferibilmente come stringhe con il punto decimale (\"12.50\"); evitare i numeri JSON con decimali.",
    "I fattori dei righi (parti_uguali, lunghezza, larghezza, altezza_peso) accettano numeri, stringhe con la virgola (\"1,20\") o espressioni (\"2*(3,50+0,30)\").",
    "La formula di un rigo sostituisce il prodotto dei fattori e usa le variabili par, lun, lar, alt (e vv, la quantità del «vedi voce»).",
    "«Vedi voce» (campo vedi_voce del rigo): il numero o l'identificativo di una voce dello stesso computo collega il rigo alla sua quantità, che si moltiplica per i fattori presenti; {\"computo\", \"voce\"} di un altro computo e ogni «vedi voce» del libretto copiano la quantità attuale (statici); {\"quantita\", \"riferimento\"} è statico. I riferimenti circolari sono rifiutati; togliendo o spostando in un altro computo una voce citata, i righi che la citano diventano statici.",
    "Le voci di computo si indicano con il numero d'ordine (da 1) o con l'identificativo; il computo per nome, facoltativo se ce n'è uno solo.",
    "Il prezzo di una voce di computo è fotografato all'inserimento: modificare l'elenco prezzi non cambia il computo finché non si usa riallinea_prezzo.",
    "Le registrazioni di un SAL emesso non sono modificabili; un SAL emesso non può essere alterato da altri comandi.",
    "Usare prima la prova (opencomputo esegui --prova, o la richiesta \"prova\" del servizio) per vedere totali e problemi senza modificare il progetto.",
];

fn esempi() -> Vec<(&'static str, Value)> {
    vec![
        (
            "Imposta i metadati del progetto; i campi assenti restano invariati, la stringa vuota cancella.",
            json!({"comando": "imposta_metadati", "titolo": "Nuova scuola primaria", "committente": "Comune di Esempio", "luogo": "Via Roma 1", "cup": "", "direttore_lavori": "Ing. Rossi"}),
        ),
        (
            "Imposta i dati del contratto (necessari per la contabilità).",
            json!({"comando": "imposta_contratto", "ribasso": "12.5", "ritenuta_garanzia": "0.5", "manodopera_esclusa_dal_ribasso": false, "importo_contrattuale": "250000.00", "data_consegna_lavori": "2026-03-01", "aliquota_iva": "10"}),
        ),
        (
            "Imposta l'anticipazione (art. 125 D.Lgs. 36/2023): senza importo è la percentuale (predefinita 20) dell'importo contrattuale; recupero predefinito alla stessa percentuale dell'importo netto progressivo.",
            json!({"comando": "imposta_anticipazione", "percentuale": "20", "percentuale_recupero": "20", "data_erogazione": "2026-03-02"}),
        ),
        (
            "Registra una penale: importo, oppure giorni × per mille dell'importo contrattuale (art. 126: 0,5–1,5 per mille, al massimo 10 %).",
            json!({"comando": "registra_penale", "data": "2026-04-20", "descrizione": "Ritardo sulla scadenza intermedia", "giorni": 3, "per_mille": "0.5"}),
        ),
        (
            "Rimuove una penale non ancora applicata in un SAL emesso.",
            json!({"comando": "rimuovi_penale", "numero": 1}),
        ),
        (
            "Opzioni del certificato di un SAL in bozza: sospensione del recupero dell'anticipazione o recupero di tutto il residuo.",
            json!({"comando": "imposta_sal", "numero": 2, "recupero_anticipazione_sospeso": true}),
        ),
        (
            "Imposta le regole di arrotondamento (modalita: commerciale, meta_al_pari, troncamento).",
            json!({"comando": "imposta_regole", "decimali_quantita": 2, "decimali_importo": 2, "modalita": "commerciale"}),
        ),
        (
            "Aggiunge un capitolo all'elenco prezzi.",
            json!({"comando": "aggiungi_capitolo", "codice": "A.01", "descrizione": "Scavi", "padre": "A"}),
        ),
        (
            "Aggiunge una voce all'elenco prezzi (natura: lavoro o oneri_sicurezza).",
            json!({"comando": "aggiungi_voce_prezzario", "codice": "A.01.001", "descrizione_breve": "Scavo di sbancamento", "descrizione_estesa": "Scavo di sbancamento in terreno di qualsiasi natura.", "unita_misura": "m³", "prezzo": "8.50", "natura": "lavoro", "incidenza_manodopera": "25", "capitolo": "A.01"}),
        ),
        (
            "Modifica una voce dell'elenco prezzi (il computo conserva i prezzi fotografati).",
            json!({"comando": "modifica_voce_prezzario", "codice": "A.01.001", "prezzo": "9.00", "nuovo_codice": "A.01.001.a"}),
        ),
        (
            "Rimuove una voce dall'elenco prezzi.",
            json!({"comando": "rimuovi_voce_prezzario", "codice": "A.01.001"}),
        ),
        (
            "Copia voci, con i loro capitoli, da un prezzario caricato nella sessione.",
            json!({"comando": "copia_da_prezzario", "prezzario": "regionale", "codici": ["E.01.010", "E.02.020"]}),
        ),
        (
            "Aggiunge una categoria (livello: super_categoria, categoria, sotto_categoria).",
            json!({"comando": "aggiungi_categoria", "livello": "categoria", "codice": "ST", "descrizione": "Strutture"}),
        ),
        (
            "Modifica codice o descrizione di una categoria.",
            json!({"comando": "modifica_categoria", "livello": "categoria", "codice": "ST", "nuovo_codice": "STR", "descrizione": "Strutture in c.a."}),
        ),
        (
            "Rimuove una categoria non assegnata a voci o registrazioni.",
            json!({"comando": "rimuovi_categoria", "livello": "categoria", "codice": "STR"}),
        ),
        (
            "Modifica codice (relativo al padre) o descrizione di un nodo WBS indicato con il codice completo.",
            json!({"comando": "modifica_nodo_wbs", "codice": "01.02", "nuovo_codice": "03", "descrizione": "Fondazioni"}),
        ),
        (
            "Rimuove un nodo WBS senza figli e non assegnato.",
            json!({"comando": "rimuovi_nodo_wbs", "codice": "01.03"}),
        ),
        (
            "Aggiunge un nodo WBS sotto il nodo padre indicato con il codice completo.",
            json!({"comando": "aggiungi_nodo_wbs", "codice": "02", "descrizione": "Fondazioni", "padre": "01"}),
        ),
        (
            "Crea un computo (tipo: computo_metrico_estimativo, computo_metrico, preventivo, perizia_variante).",
            json!({"comando": "nuovo_computo", "nome": "Variante 1", "tipo": "perizia_variante"}),
        ),
        (
            "Aggiunge una voce al computo con il prezzo dell'elenco prezzi e i righi di misura.",
            json!({"comando": "aggiungi_voce_computo", "computo": "Computo metrico estimativo", "codice": "A.01.001", "categorie": {"categoria": "ST"}, "wbs": "01.02", "righi": [
                {"descrizione": "Piano terra", "commento": true},
                {"descrizione": "Platea", "lunghezza": "12.00", "larghezza": "8.00", "altezza_peso": "1,20"},
                {"descrizione": "Rampa", "lunghezza": 6, "larghezza": 3, "altezza_peso": "1.20", "formula": "0,5*lun*lar*alt"},
                {"descrizione": "A dedurre pozzetti", "parti_uguali": 2, "lunghezza": "1.00", "larghezza": "1.00", "altezza_peso": "1.20", "deduzione": true}
            ]}),
        ),
        (
            "Modifica categorie, WBS o note di una voce di computo (stringa vuota = nessuno).",
            json!({"comando": "modifica_voce_computo", "voce": 1, "categorie": {"categoria": ""}, "wbs": "01.03", "note": "verificare in cantiere"}),
        ),
        (
            "Rimuove una voce di computo.",
            json!({"comando": "rimuovi_voce_computo", "voce": 3}),
        ),
        (
            "Sposta una voce di computo nella posizione indicata (da 1).",
            json!({"comando": "sposta_voce_computo", "voce": 4, "posizione": 1}),
        ),
        (
            "Copia voci di computo (con righi, prezzo fotografato, categorie, WBS e note) nello stesso computo o in un altro; con `sposta` le sposta, conservandone l'identificativo.",
            json!({"comando": "copia_voci_computo", "da": "Computo metrico estimativo", "voci": [1, 3], "a": "Variante", "posizione": 1, "sposta": false}),
        ),
        (
            "Aggiorna il prezzo fotografato di una voce di computo all'elenco prezzi corrente.",
            json!({"comando": "riallinea_prezzo", "voce": 1}),
        ),
        (
            "Aggiunge un rigo di misura (in coda se manca la posizione); qui un «vedi voce»: metà della quantità della voce n. 1, che segue la voce.",
            json!({"comando": "aggiungi_rigo", "voce": 3, "rigo": {"descrizione": "Metà dello scavo", "vedi_voce": 1, "parti_uguali": "0.5"}, "posizione": 2}),
        ),
        (
            "Sostituisce il rigo indicato (numero da 1).",
            json!({"comando": "modifica_rigo", "voce": 1, "numero": 2, "rigo": {"descrizione": "Platea", "lunghezza": 12, "larghezza": 8, "altezza_peso": "1.10"}}),
        ),
        (
            "Rimuove il rigo indicato.",
            json!({"comando": "rimuovi_rigo", "voce": 1, "numero": 2}),
        ),
        (
            "Registra una misurazione nel libretto delle misure.",
            json!({"comando": "registra", "data": "2026-03-10", "codice": "A.01.001", "righi": [
                {"descrizione": "Platea", "lunghezza": 12, "larghezza": 8, "altezza_peso": "1.2"},
                {"descrizione": "Come da computo", "vedi_voce": {"computo": "Computo metrico estimativo", "voce": 2}}
            ], "annotazioni": "presente l'impresa"}),
        ),
        (
            "Sostituisce data, righi o annotazioni di una registrazione non ancora in un SAL emesso.",
            json!({"comando": "modifica_registrazione", "numero": 1, "data": "2026-03-11", "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1.25"}], "annotazioni": "presente il direttore dei lavori"}),
        ),
        (
            "Rimuove una registrazione non contabilizzata in un SAL.",
            json!({"comando": "rimuovi_registrazione", "numero": 1}),
        ),
        (
            "Crea un SAL in bozza con le registrazioni non contabilizzate fino alla data.",
            json!({"comando": "nuovo_sal", "lavori_a_tutto_il": "2026-03-31"}),
        ),
        (
            "Annulla il SAL in bozza.",
            json!({"comando": "annulla_bozza_sal", "numero": 1}),
        ),
        (
            "Emette il SAL: prospetto congelato, registrazioni bloccate.",
            json!({"comando": "emetti_sal", "numero": 1, "data_emissione": "2026-04-05"}),
        ),
        (
            "Redige il conto finale in bozza (le registrazioni non comprese nei SAL formano lo stato finale) o ne aggiorna date e note; nessun SAL deve essere in bozza.",
            json!({"comando": "redigi_conto_finale", "data_ultimazione": "2026-05-25", "data": "2026-06-15", "note": "Lavori ultimati senza varianti"}),
        ),
        (
            "Emette il conto finale: riepilogo e rata di saldo congelati, contabilità chiusa.",
            json!({"comando": "emetti_conto_finale", "data": "2026-06-15"}),
        ),
        (
            "Annulla il conto finale in bozza.",
            json!({"comando": "annulla_conto_finale"}),
        ),
        (
            "Firma dell'esecutore sul conto finale emesso, con le riserve confermate (senza data: firma rimossa).",
            json!({"comando": "firma_conto_finale", "data": "2026-06-30", "riserve": "Riserva n. 1 confermata"}),
        ),
        (
            "Registra il collaudo o il certificato di regolare esecuzione (tipo: collaudo, regolare_esecuzione) e la sua approvazione.",
            json!({"comando": "registra_collaudo", "tipo": "regolare_esecuzione", "data": "2026-07-20", "approvato_il": "2026-07-31"}),
        ),
        (
            "Aggiunge o modifica una risorsa elementare (tipo: manodopera, materiale, nolo, trasporto, altro). Le analisi usano subito il nuovo prezzo; le voci si aggiornano con ricalcola_prezzi_analisi.",
            json!({"comando": "imposta_risorsa", "codice": "MO.02", "descrizione": "Operaio comune", "unita_misura": "h", "tipo": "manodopera", "prezzo": "32.00"}),
        ),
        (
            "Rimuove una risorsa non usata nelle analisi.",
            json!({"comando": "rimuovi_risorsa", "codice": "MO.02"}),
        ),
        (
            "Crea o sostituisce l'analisi del prezzo di una voce: righe con risorsa, voce dell'elenco (anche analizzata) o libere; quantità come numero o espressione. Le quantità possono usare le variabili dell'analisi e q1, q2… (quantità delle righe precedenti); quota delle spese generali costituita dagli oneri aziendali della sicurezza. Spese generali e utile predefiniti dall'elenco (15 % e 10 %). Con applica_prezzo (predefinito) aggiorna prezzo e incidenza della manodopera della voce.",
            json!({"comando": "imposta_analisi", "voce": "NP.01", "quantita_riferimento": 1, "spese_generali": "15", "utile_impresa": "10", "quota_oneri_sicurezza": "20",
              "variabili": [{"nome": "spessore", "valore": "0,015", "descrizione": "spessore (m)"}, {"nome": "kg", "valore": "spessore*800"}],
              "righe": [
                {"risorsa": "MO.02", "quantita": "0,25"},
                {"risorsa": "MA.20", "quantita": "kg"},
                {"risorsa": "NO.01", "quantita": "q1/2"},
                {"voce": "B.02.010", "quantita": "0.01", "tipo": "materiale"},
                {"codice": "TR", "descrizione": "Trasporto a discarica", "unita_misura": "t", "tipo": "trasporto", "prezzo": "12.00", "quantita": "0.02"}
            ]}),
        ),
        (
            "Rimuove l'analisi di una voce (il prezzo resta quello attuale).",
            json!({"comando": "rimuovi_analisi", "voce": "NP.01"}),
        ),
        (
            "Applica a tutte le voci analizzate il prezzo calcolato con i prezzi correnti delle risorse (anche le sotto-analisi).",
            json!({"comando": "ricalcola_prezzi_analisi"}),
        ),
        (
            "Spese generali e utile predefiniti delle analisi (base_utile: costo_piu_spese_generali o solo_costo); applica_a_tutte li estende alle analisi esistenti.",
            json!({"comando": "imposta_parametri_analisi", "spese_generali": "15", "utile_impresa": "10", "base_utile": "costo_piu_spese_generali", "applica_a_tutte": false}),
        ),
        (
            "Aggiorna i prezzi di risorse (anche di un solo tipo) o di voci senza analisi (anche di un capitolo) con una variazione percentuale, anche negativa; con ricalcola_analisi (predefinito) aggiorna poi le voci analizzate.",
            json!({"comando": "aggiorna_prezzi", "oggetto": "risorse", "tipo": "materiale", "percentuale": "3.5"}),
        ),
        (
            "Cronoprogramma: inizio dei lavori, livello di dettaglio della WBS (null: tutti), computo degli importi, giorni lavorativi da lunedì a domenica, festività nazionali, chiusure del cantiere.",
            json!({"comando": "imposta_cronoprogramma", "inizio_lavori": "2026-03-02", "livello_dettaglio": 2, "computo": "Computo", "giorni_lavorativi": [true, true, true, true, true, false, false], "festivita_nazionali": true, "chiusure": [{"dal": "2026-08-10", "al": "2026-08-21", "descrizione": "Ferie"}]}),
        ),
        (
            "Dati di un'attività del cronoprogramma (nodo WBS per codice o attività aggiunta per id): durata in giorni lavorativi, inizio non prima del, avanzamento %, note; per le attività aggiunte anche nome, pietra_miliare, padre.",
            json!({"comando": "imposta_attivita", "attivita": {"wbs": "01.02"}, "durata": 15, "inizio_non_prima": "2026-03-16", "avanzamento": 40}),
        ),
        (
            "Aggiunge al cronoprogramma un'attività senza voci o una pietra miliare, sotto un nodo WBS (padre) o alla radice.",
            json!({"comando": "aggiungi_attivita", "nome": "Allestimento del cantiere", "padre": "01", "durata": 3}),
        ),
        (
            "Toglie un'attività aggiunta al cronoprogramma e i suoi legami.",
            json!({"comando": "rimuovi_attivita", "attivita": "00000000-0000-0000-0000-000000001f41"}),
        ),
        (
            "Collega due attività: tipo fine_inizio (predefinito), inizio_inizio, fine_fine, inizio_fine; ritardo in giorni lavorativi, anche negativo. I cicli sono rifiutati.",
            json!({"comando": "collega_attivita", "da": {"wbs": "01.01"}, "a": {"wbs": "01.02"}, "tipo": "fine_inizio", "ritardo": 2}),
        ),
        (
            "Toglie il legame tra due attività del cronoprogramma.",
            json!({"comando": "scollega_attivita", "da": {"wbs": "01.01"}, "a": {"wbs": "01.02"}}),
        ),
        (
            "Salva la linea di base del cronoprogramma con il giorno indicato, o la toglie con cancella.",
            json!({"comando": "linea_base_cronoprogramma", "data": "2026-02-25"}),
        ),
        (
            "Aggiorna voci e risorse dell'elenco da un prezzario caricato (nuovo listino), solo per i codici e i campi scelti (prezzo, incidenze, descrizioni, unita_misura, codice), anche con la sostituzione del prefisso dei codici; ricalcola le voci analizzate e riallinea le voci dei computi collegate (richiesta confronto_prezzario per il confronto).",
            json!({"comando": "aggiorna_da_prezzario", "prezzario": "regionale 2027", "voci": ["E.01.010"], "risorse": ["MO.01"], "campi": {"prezzo": true, "incidenze": true, "descrizioni": false}, "prefisso": {"da": "TOS23_", "a": "TOS24_"}}),
        ),
        (
            "Segna una voce dell'elenco come nuovo prezzo NP.n (criterio: prezzario, analogia, analisi) o ne modifica i dati; soggetto_a_ribasso false lo esclude dal ribasso nei SAL.",
            json!({"comando": "imposta_nuovo_prezzo", "voce": "NP.01", "data": "2026-04-08", "motivo": "Lavorazione non prevista", "criterio": "analisi", "soggetto_a_ribasso": true}),
        ),
        (
            "Crea (senza numero) o modifica un verbale di concordamento dei nuovi prezzi: voci comprese, firma e riserve dell'esecutore, approvazione del RUP (null toglie un campo).",
            json!({"comando": "imposta_verbale_nuovi_prezzi", "data": "2026-04-10", "nuovi_prezzi": ["NP.01", "NP.02"], "firma_esecutore": "2026-04-12", "approvato_il": "2026-04-15", "atto_approvazione": "Determina del RUP n. 12"}),
        ),
        (
            "Rimuove un verbale non approvato; i suoi nuovi prezzi restano senza verbale.",
            json!({"comando": "rimuovi_verbale_nuovi_prezzi", "numero": 1}),
        ),
    ]
}

/// Guida completa: regole ed esempi.
#[must_use]
pub fn schema_comandi() -> Value {
    let comandi: Vec<Value> = esempi()
        .into_iter()
        .map(|(descrizione, esempio)| {
            json!({
                "comando": esempio.get("comando").cloned().unwrap_or(Value::Null),
                "descrizione": descrizione,
                "esempio": esempio,
            })
        })
        .collect();
    json!({
        "regole": REGOLE,
        "comandi": comandi,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use std::collections::BTreeSet;

    use opencomputo_application::Comando;

    use super::*;

    #[test]
    fn ogni_esempio_e_un_comando_valido_e_tutti_i_comandi_hanno_un_esempio() {
        let mut nomi = BTreeSet::new();
        for (_, esempio) in esempi() {
            let c: Comando = serde_json::from_value(esempio.clone())
                .unwrap_or_else(|e| panic!("{e}: {esempio}"));
            // Andata e ritorno: nessun campo dell'esempio viene ignorato.
            let rilettura = serde_json::to_value(&c).unwrap();
            assert_eq!(rilettura["comando"], esempio["comando"]);
            assert!(
                nomi.insert(esempio["comando"].as_str().unwrap().to_owned()),
                "duplicato: {esempio}"
            );
        }
        // 57 varianti di `Comando`: aggiornare gli esempi quando se ne aggiunge una.
        assert_eq!(nomi.len(), 57);
        let s = schema_comandi();
        assert_eq!(s["comandi"].as_array().unwrap().len(), 57);
        assert_eq!(s["regole"].as_array().unwrap().len(), REGOLE.len());
    }
}
