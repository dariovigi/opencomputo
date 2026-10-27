//! Risposte del modello così come arrivano, prima della validazione.
//!
//! I campi a valori chiusi (esito, ruolo, forma, unità…) sono letti come testo: gli
//! structured outputs non garantiscono maiuscole e minuscole dei valori `enum`, quindi il
//! confronto avviene in [`crate::proposta`] senza distinzione di caso. Nessun campo è
//! facoltativo: un campo «vuoto» è una stringa vuota o un elenco vuoto.

use serde::{Deserialize, Serialize};

/// Risposta della prima chiamata: analisi della richiesta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RispostaAnalisi {
    /// `procedi` oppure `chiedi` (mancano dati bloccanti).
    pub esito: String,
    /// Lavorazioni ipotizzate, ciascuna con la sua domanda di ricerca.
    pub lavorazioni: Vec<LavorazioneAnalisi>,
    /// Informazioni mancanti.
    pub informazioni_mancanti: Vec<InformazioneMancante>,
}

/// Lavorazione ipotizzata nell'analisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LavorazioneAnalisi {
    /// Identificativo (`L01`…).
    pub id: String,
    /// Descrizione della lavorazione.
    pub descrizione: String,
    /// Testo da cercare nel prezzario.
    pub query_ricerca: String,
    /// Dimensione attesa dell'unità di misura (`superficie`, `volume`… o `qualsiasi`).
    pub dimensione_attesa: String,
}

/// Informazione che manca per completare il computo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InformazioneMancante {
    /// Domanda da porre all'utente.
    pub domanda: String,
    /// `true` se senza la risposta il computo non è attendibile.
    pub bloccante: bool,
    /// Lavorazioni interessate.
    pub lavorazioni: Vec<String>,
}

/// Risposta della seconda chiamata: scelta delle voci e righi di misura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RispostaSelezione {
    /// `completo`, `provvisorio` o `impossibile`.
    pub esito: String,
    /// Lavorazioni con la voce scelta.
    pub lavorazioni: Vec<LavorazioneSelezione>,
    /// Ipotesi fatte dal modello.
    pub ipotesi: Vec<IpotesiModello>,
    /// Informazioni mancanti.
    pub informazioni_mancanti: Vec<InformazioneMancante>,
}

/// Lavorazione con la voce scelta tra i candidati.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LavorazioneSelezione {
    /// Identificativo (`L01`…).
    pub id: String,
    /// Descrizione della lavorazione.
    pub descrizione: String,
    /// Handle del candidato (`c01`…) oppure `NESSUNO`.
    pub candidato: String,
    /// `principale`, `complementare` o `alternativa`.
    pub ruolo: String,
    /// Gruppo delle alternative (vuoto se non è un'alternativa).
    pub gruppo_alternativa: String,
    /// Lavorazione di cui è complementare (vuoto se non è complementare).
    pub complementare_di: String,
    /// Righi di misura.
    pub righe: Vec<RigaModello>,
    /// Motivo della scelta, in una frase.
    pub motivazione: String,
}

/// Rigo di misura proposto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigaModello {
    /// Descrizione del rigo.
    pub descrizione: String,
    /// Forma del catalogo chiuso.
    pub forma: String,
    /// `positivo` o `detrazione`.
    pub segno: String,
    /// Parametri della forma.
    pub parametri: Vec<ParametroModello>,
}

/// Parametro di un rigo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParametroModello {
    /// Nome (`N`, `L`, `B`, `H`, `D`, `B1`, `B2`, `A`, `V`, `P`).
    pub nome: String,
    /// Valore decimale con il punto, come testo.
    pub valore: String,
    /// Unità (`n`, `mm`, `cm`, `m`, `m2`, `m3`, `kg/m`).
    pub unita: String,
    /// `utente` o `ipotesi`.
    pub provenienza: String,
    /// Identificativo dell'ipotesi (vuoto se fornito dall'utente).
    pub rif_ipotesi: String,
}

/// Ipotesi del modello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpotesiModello {
    /// Identificativo (`I01`…).
    pub id: String,
    /// Testo dell'ipotesi.
    pub testo: String,
    /// `basso`, `medio` o `alto`.
    pub impatto: String,
}
