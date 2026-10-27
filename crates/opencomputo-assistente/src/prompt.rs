//! Istruzioni e messaggi per le due chiamate al modello.
//!
//! Le regole del catalogo delle forme e degli elenchi chiusi sono generate dal codice
//! ([`Forma::TUTTE`], [`crate::schema`]): istruzioni, schema e validazione non possono
//! divergere. Le regole ripetono ciò che la validazione controlla comunque: servono a
//! ottenere risposte valide al primo colpo, non a garantirle.

use serde_json::{Value, json};

use crate::candidati::Candidati;
use crate::forme::Forma;
use crate::risposta::RispostaAnalisi;

const RUOLO: &str = "Sei l'assistente di un programma italiano per computi metrici estimativi di lavori pubblici. \
I prezzi, i codici e i calcoli sono responsabilità del programma: tu interpreti la richiesta dell'utente.";

/// Istruzioni della chiamata di analisi.
#[must_use]
pub fn sistema_analisi() -> String {
    format!(
        "{RUOLO}

Compito: individuare le lavorazioni richieste e preparare la ricerca nel prezzario.

Regole:
- Elenca solo le lavorazioni chieste o descritte dall'utente. Non aggiungere lavorazioni, stratigrafie, sfridi o finiture non nominati.
- Identificativi L01, L02… nell'ordine in cui le lavorazioni compaiono.
- query_ricerca: poche parole tecniche come compaiono nei prezzari regionali (lavorazione, materiale, elemento), senza misure né quantità.
- dimensione_attesa: la grandezza con cui la lavorazione si misura (lunghezza, superficie, volume, massa, numero, a_corpo); qualsiasi se non è chiaro.
- Se la lavorazione è riconoscibile ma mancano dettagli (materiale, tipo, spessore), elencala comunque: la scelta tra le voci possibili la farà l'utente.
- informazioni_mancanti: i dati che servono e mancano (dimensioni, spessori, materiali). bloccante = true solo se senza quel dato non si capisce nemmeno quale lavorazione serve.
- esito = chiedi solo se non riesci a individuare nessuna lavorazione; altrimenti procedi."
    )
}

/// Messaggio della chiamata di analisi.
#[must_use]
pub fn utente_analisi(richiesta: &str) -> String {
    format!("Richiesta dell'utente:\n<<<\n{}\n>>>", richiesta.trim())
}

/// Descrizione del catalogo delle forme, dal codice.
#[must_use]
pub fn catalogo_forme() -> String {
    Forma::TUTTE
        .iter()
        .map(|f| {
            let p: Vec<String> = f.parametri().iter().map(|p| format!("{p:?}")).collect();
            let p = if p.is_empty() {
                "nessun parametro".to_owned()
            } else {
                p.join(", ")
            };
            format!("- {}: {p} → {}", f.nome(), f.dimensione().nome())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Istruzioni della chiamata di selezione.
#[must_use]
pub fn sistema_selezione() -> String {
    format!(
        "{RUOLO}

Compito: per ogni lavorazione proponi le voci possibili tra i candidati forniti, dalla più alla meno pertinente, e scrivi i righi di misura. L'utente sceglierà quali voci usare.

Opzioni:
- opzioni: fino a {massime} handle tra quelli forniti (c01, c02…), ordinati per pertinenza (alta, media, bassa). Solo voci davvero adatte alla lavorazione: meglio due buone opzioni che cinque forzate. Se nessun candidato è pertinente, nessuna opzione.
- Le opzioni di una lavorazione condividono i righi di misura: proponi solo voci con un'unità di misura coerente con quei righi (una voce in m² vuole righi che danno una superficie).
- chiarezza = chiara se la richiesta basta a scegliere la voce (la prima opzione è quella giusta); da_precisare se manca un'informazione per scegliere (materiale, tipo, finitura, spessore): in da_precisare scrivi in poche parole cosa manca (es. «materiale del pavimento non indicato») e proponi le opzioni più plausibili.
- motivazione: una frase breve sul perché della voce (es. «gres porcellanato, posa a colla»).
- Non indicare prezzi né codici: li prende il programma dal prezzario.

Righi di misura: usa solo queste forme e i loro parametri, tutti presenti.
{forme}
Parametri: N numero di parti uguali (intero, unità n); L, B, H, D, B1, B2 in mm, cm o m; A in m2; V in m3; P in kg/m. Valori con il punto decimale, maggiori di zero.
- segno = detrazione per i vuoti da togliere (porte, finestre).

Dati e ipotesi:
- provenienza = utente solo se il numero è scritto nella richiesta (anche in un'altra unità).
- Se un dato manca ma un valore usuale è ragionevole (altezza di un piano, spessore corrente), usalo come ipotesi: aggiungila in ipotesi (I01, I02…) con l'impatto e indica il suo id in rif_ipotesi.
- Se un dato manca e non è ragionevole ipotizzarlo, valore vuoto e provenienza mancante: l'utente lo completerà. Non inventare dimensioni, spessori, stratigrafie o sfridi.
- informazioni_mancanti: le domande utili per completare o precisare il computo.

Lavorazioni:
- Una voce per lavorazione nei righi: non contare due volte la stessa quantità in lavorazioni diverse.",
        forme = catalogo_forme(),
        massime = crate::schema::OPZIONI_MASSIME,
    )
}

/// Messaggio della chiamata di selezione: richiesta, lavorazioni dell'analisi, candidati.
#[must_use]
pub fn utente_selezione(
    richiesta: &str,
    analisi: &RispostaAnalisi,
    candidati: &Candidati,
) -> String {
    let lavorazioni: Vec<Value> = analisi
        .lavorazioni
        .iter()
        .map(|l| json!({"id": l.id, "descrizione": l.descrizione, "dimensione_attesa": l.dimensione_attesa}))
        .collect();
    format!(
        "Richiesta dell'utente:\n<<<\n{}\n>>>\n\nLavorazioni individuate:\n{}\n\nCandidati del prezzario (proponi solo questi handle):\n{}",
        richiesta.trim(),
        Value::Array(lavorazioni),
        candidati.per_modello()
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn catalogo_dal_codice() {
        let c = catalogo_forme();
        assert_eq!(c.lines().count(), Forma::TUTTE.len());
        assert!(c.contains("- rettangolo: N, L, B → superficie"));
        assert!(c.contains("- a_corpo: nessun parametro → a corpo"));
        assert!(sistema_selezione().contains("- cilindro: N, D, H → volume"));
    }
}
