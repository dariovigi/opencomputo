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
- informazioni_mancanti: i dati che servono e mancano (dimensioni, spessori, materiali). bloccante = true se senza quel dato non si può nemmeno impostare il computo.
- esito = chiedi solo se manca un'informazione bloccante; altrimenti procedi."
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

Compito: per ogni lavorazione scegli la voce del prezzario tra i candidati forniti e scrivi i righi di misura.

Voci:
- candidato: solo un handle tra quelli forniti (c01, c02…). Se nessun candidato è pertinente scrivi NESSUNO: non adattare voci sbagliate e non inventarne.
- Non indicare prezzi né codici: li prende il programma dal prezzario.
- L'unità di misura della voce deve corrispondere alla forma dei righi (una voce in m² vuole una forma che dà una superficie).

Righi di misura: usa solo queste forme e i loro parametri, tutti obbligatori.
{forme}
Parametri: N numero di parti uguali (intero, unità n); L, B, H, D, B1, B2 in mm, cm o m; A in m2; V in m3; P in kg/m. Valori con il punto decimale, maggiori di zero.
- segno = detrazione per i vuoti da togliere (porte, finestre).

Dati e ipotesi:
- provenienza = utente solo se il numero è scritto nella richiesta (anche in un'altra unità). Ogni altro valore è un'ipotesi: aggiungila in ipotesi (I01, I02…), con impatto, e indica il suo id in rif_ipotesi.
- Non inventare dimensioni, spessori, stratigrafie o sfridi. Se un dato mancante cambia molto il risultato mettilo in informazioni_mancanti; se usi comunque un'ipotesi, l'esito è provvisorio.

Lavorazioni:
- ruolo principale; complementare (con complementare_di = la lavorazione principale) per ciò che la completa; alternativa (con lo stesso gruppo_alternativa) per soluzioni tra cui scegliere: ne verrà computata una sola.
- Non contare due volte la stessa quantità con voci che si sovrappongono.
- motivazione: una frase breve sul perché della voce scelta.
- esito: completo solo senza ipotesi né informazioni mancanti; impossibile se nessuna voce è utilizzabile.",
        forme = catalogo_forme()
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
        "Richiesta dell'utente:\n<<<\n{}\n>>>\n\nLavorazioni individuate:\n{}\n\nCandidati del prezzario (scegli solo tra questi handle):\n{}",
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
