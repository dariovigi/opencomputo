//! Riepiloghi in JSON del progetto: stato, validazione, ricerca nei prezzari.

use std::path::Path;

use opencomputo_application::Sessione;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::validazione::{Gravita, valida_progetto};
use serde_json::{Value, json};

/// Stato sintetico della sessione: totali dei computi, SAL, conteggio dei problemi.
#[must_use]
pub fn info(s: &Sessione, percorso: Option<&Path>) -> Value {
    let p = s.progetto();
    let rapporto = valida_progetto(p);
    let conta = |g: Gravita| rapporto.problemi.iter().filter(|x| x.gravita == g).count();
    let computi: Vec<Value> = p
        .computi
        .iter()
        .map(|c| match p.calcola_computo(c.id) {
            Ok(r) => json!({
                "nome": c.nome,
                "tipo": c.tipo,
                "voci": c.voci.len(),
                "totale": r.totale,
                "totale_lavori": r.totale_lavori,
                "totale_oneri_sicurezza": r.totale_oneri_sicurezza,
                "totale_manodopera": r.totale_manodopera,
            }),
            Err(e) => json!({
                "nome": c.nome,
                "tipo": c.tipo,
                "voci": c.voci.len(),
                "errore": e.to_string(),
            }),
        })
        .collect();
    let sal: Vec<Value> = p
        .contabilita
        .sal()
        .iter()
        .map(|x| {
            json!({
                "numero": x.numero,
                "stato": x.stato,
                "lavori_a_tutto_il": x.lavori_a_tutto_il,
                "data_emissione": x.data_emissione,
                "registrazioni": x.registrazioni.len(),
                "importo_sal": x.riepilogo_emesso.as_ref().map(|r| r.importo_sal),
            })
        })
        .collect();
    let prezzari: Vec<Value> = s
        .prezzari()
        .iter()
        .map(|(nome, pr)| json!({"nome": nome, "voci": pr.voci.len()}))
        .collect();
    json!({
        "file": percorso,
        "titolo": p.metadati.titolo,
        "committente": p.metadati.committente,
        "modificato": s.modificato(),
        "elenco_prezzi": {
            "voci": p.elenco_prezzi.voci.len(),
            "capitoli": p.elenco_prezzi.capitoli.len(),
            "analisi": p.elenco_prezzi.analisi.len(),
            "risorse": p.elenco_prezzi.risorse.len(),
            "nuovi_prezzi": p.elenco_prezzi.voci.iter().filter(|v| v.nuovo_prezzo.is_some()).count(),
        },
        "computi": computi,
        "contabilita": {
            "contratto": p.contratto.is_some(),
            "registrazioni": p.contabilita.libretto().registrazioni.len(),
            "sal": sal,
            "conto_finale": p.contabilita.conto_finale().map(|c| c.stato),
            "verbali_nuovi_prezzi": p
                .contabilita
                .verbali_nuovi_prezzi()
                .iter()
                .map(|v| v.numero)
                .collect::<Vec<_>>(),
        },
        "problemi": {
            "errori": conta(Gravita::Errore),
            "avvisi": conta(Gravita::Avviso),
            "informazioni": conta(Gravita::Informazione),
        },
        "annullabili": s.passi_annullabili(),
        "ripristinabili": s.passi_ripristinabili(),
        "prezzari_caricati": prezzari,
    })
}

/// Validazione completa.
#[must_use]
pub fn valida(s: &Sessione) -> Value {
    let rapporto = valida_progetto(s.progetto());
    let conta = |g: Gravita| rapporto.problemi.iter().filter(|x| x.gravita == g).count();
    json!({
        "valido": !rapporto.ha_errori(),
        "errori": conta(Gravita::Errore),
        "avvisi": conta(Gravita::Avviso),
        "problemi": rapporto.problemi,
    })
}

/// Ricerca testuale (tutte le parole, senza distinzione di maiuscole) su codice e
/// descrizioni.
#[must_use]
pub fn cerca(prezzario: &Prezzario, testo: &str, limite: usize) -> Value {
    let trovate: Vec<_> = prezzario.cerca(testo).collect();
    let voci: Vec<Value> = trovate
        .iter()
        .take(limite)
        .map(|v| {
            let capitolo = v
                .capitolo
                .and_then(|id| prezzario.capitoli.iter().find(|c| c.id == id))
                .map(|c| c.codice.as_str());
            json!({
                "codice": v.codice,
                "descrizione_breve": v.descrizione_breve,
                "descrizione_estesa": v.descrizione_estesa,
                "unita_misura": v.unita_misura,
                "prezzo": v.prezzo,
                "natura": v.natura,
                "incidenza_manodopera": v.incidenza_manodopera,
                "capitolo": capitolo,
            })
        })
        .collect();
    json!({
        "prezzario": prezzario.nome,
        "trovate": trovate.len(),
        "mostrate": voci.len(),
        "voci": voci,
    })
}
