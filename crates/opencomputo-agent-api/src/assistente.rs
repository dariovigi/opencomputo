//! Assistente AI nel servizio JSON (ADR 0025): configurazione, chiavi, consenso,
//! indicizzazione dei prezzari, ricerca e proposte di computo.
//!
//! * La configurazione (modelli, limiti, consenso) sta in `assistente.json` nella
//!   cartella di configurazione; le chiavi solo nel portachiavi del sistema.
//! * L'indice vettoriale di ogni prezzario è una cache in `cache/`, con nel nome
//!   l'impronta del contenuto del prezzario e lo spazio di embedding; viene salvato
//!   durante l'indicizzazione, così un'indicizzazione interrotta riprende da dove era
//!   arrivata.
//! * Nessuna chiamata ai provider senza il consenso alla versione corrente del testo.
//! * Le proposte non modificano il progetto: si provano subito e si applicano solo con
//!   la richiesta `assistente_applica`.
//!
//! Le operazioni lunghe (indice, indicizzazione, ricerca, proposta) sono [`Lavoro`]:
//! preparate con il servizio, eseguite senza il servizio (l'interfaccia resta libera di
//! fare altre richieste), concluse di nuovo con il servizio. Gli indici di ogni
//! prezzario hanno un proprio lucchetto.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use opencomputo_application::ErroreApplicazione;
use opencomputo_assistente::candidati::{cerca_ibrida, dimensioni_attese};
use opencomputo_assistente::consenso::{self, Consenso};
use opencomputo_assistente::flusso::{self, ErroreFlusso, EsitoFlusso, Parametri, Strumenti};
use opencomputo_assistente::proposta::Proposta;
use opencomputo_assistente::provider::anthropic::{
    self, ClienteGenerazione, ConfigGenerazione, ModalitaOutput,
};
use opencomputo_assistente::provider::chiavi::{ArchivioChiavi, ChiaviInMemoria};
use opencomputo_assistente::provider::openai::{
    self, ClienteEmbedding, ConfigEmbedding, LimitiLotto, dimensioni_predefinite,
};
use opencomputo_assistente::provider::{
    Annullamento, Attesa, Embedding, ErroreAi, ErroreTrasporto, Provider, RichiestaHttp,
    RispostaHttp, Segreto, Trasporto,
};
use opencomputo_assistente::ricerca::{FiltroVoci, IndiceTestuale};
use opencomputo_assistente::vettori::{
    IndiceVettoriale, SpazioEmbedding, VoceDaIndicizzare, da_indicizzare, impronta_prezzario,
    indicizza,
};
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Nome del file di configurazione.
pub const FILE_CONFIGURAZIONE: &str = "assistente.json";

/// Intervallo minimo tra due salvataggi della cache durante l'indicizzazione.
const SALVATAGGIO_CACHE: Duration = Duration::from_secs(30);

/// Indirizzi delle API dei provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indirizzi {
    /// Embedding (OpenAI).
    pub embedding: String,
    /// Messaggi (Anthropic).
    pub generazione: String,
}

impl Default for Indirizzi {
    fn default() -> Self {
        Self {
            embedding: openai::URL_EMBEDDING.into(),
            generazione: anthropic::URL_MESSAGGI.into(),
        }
    }
}

/// Dipendenze esterne dell'assistente, sostituibili nei test.
pub struct Ambiente {
    /// Cartella di configurazione e cache.
    pub cartella: PathBuf,
    /// Archivio delle chiavi.
    pub chiavi: Arc<dyn ArchivioChiavi>,
    /// Trasporto HTTP.
    pub trasporto: Arc<dyn Trasporto>,
    /// Attesa tra i tentativi.
    pub attesa: Arc<dyn Attesa>,
    /// Indirizzi delle API.
    pub indirizzi: Indirizzi,
}

impl std::fmt::Debug for Ambiente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ambiente")
            .field("cartella", &self.cartella)
            .field("chiavi", &self.chiavi.descrizione())
            .field("indirizzi", &self.indirizzi)
            .finish_non_exhaustive()
    }
}

/// Trasporto che rifiuta ogni richiesta (client HTTPS non disponibile).
struct TrasportoAssente(String);

impl Trasporto for TrasportoAssente {
    fn invia(&self, _: &RichiestaHttp) -> Result<RispostaHttp, ErroreTrasporto> {
        Err(ErroreTrasporto::Altro(self.0.clone()))
    }
}

/// Cartella di configurazione predefinita: `OPENCOMPUTO_CONFIG` se impostata, altrimenti
/// `~/Library/Application Support/OpenComputo` (macOS), `%APPDATA%\OpenComputo`
/// (Windows), `$XDG_CONFIG_HOME/opencomputo` o `~/.config/opencomputo` (altri).
#[must_use]
pub fn cartella_predefinita() -> PathBuf {
    let var = |n: &str| {
        std::env::var_os(n)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(c) = var("OPENCOMPUTO_CONFIG") {
        return c;
    }
    if cfg!(target_os = "macos") {
        if let Some(h) = var("HOME") {
            return h.join("Library/Application Support/OpenComputo");
        }
    } else if cfg!(target_os = "windows") {
        if let Some(a) = var("APPDATA") {
            return a.join("OpenComputo");
        }
    } else if let Some(x) = var("XDG_CONFIG_HOME") {
        return x.join("opencomputo");
    } else if let Some(h) = var("HOME") {
        return h.join(".config/opencomputo");
    }
    PathBuf::from(".opencomputo")
}

impl Ambiente {
    /// Ambiente reale: portachiavi del sistema (o memoria se non disponibile), HTTPS.
    ///
    /// Solo con la feature di sviluppo `banco-di-prova`: se la variabile
    /// `OPENCOMPUTO_PROVIDER_DI_PROVA` contiene un indirizzo (`http://127.0.0.1:…`), le
    /// chiamate vanno lì e le chiavi restano in memoria. Serve ai test dell'interfaccia.
    #[must_use]
    pub fn di_sistema() -> Self {
        let trasporto: Arc<dyn Trasporto> = match opencomputo_rete::TrasportoHttps::new() {
            Ok(t) => Arc::new(t),
            Err(e) => Arc::new(TrasportoAssente(e.to_string())),
        };
        let mut a = Self {
            cartella: cartella_predefinita(),
            chiavi: Arc::new(ChiaviInMemoria::default()),
            trasporto,
            attesa: Arc::new(opencomputo_rete::AttesaReale),
            indirizzi: Indirizzi::default(),
        };
        if banco_di_prova(&mut a) {
            return a;
        }
        if let Ok(p) = opencomputo_rete::PortachiaviSistema::new() {
            a.chiavi = avvolgi_chiavi(p);
        }
        a
    }
}

#[cfg(feature = "banco-di-prova")]
fn banco_di_prova(a: &mut Ambiente) -> bool {
    let Some(base) = std::env::var("OPENCOMPUTO_PROVIDER_DI_PROVA")
        .ok()
        .filter(|b| b.starts_with("http://127.0.0.1:"))
    else {
        return false;
    };
    let base = base.trim_end_matches('/');
    a.indirizzi = Indirizzi {
        embedding: format!("{base}/v1/embeddings"),
        generazione: format!("{base}/v1/messages"),
    };
    a.chiavi = Arc::new(ChiaviInMemoria::default());
    true
}

#[cfg(not(feature = "banco-di-prova"))]
fn banco_di_prova(_: &mut Ambiente) -> bool {
    false
}

#[cfg(feature = "chiavi-da-ambiente")]
fn avvolgi_chiavi(p: opencomputo_rete::PortachiaviSistema) -> Arc<dyn ArchivioChiavi> {
    Arc::new(opencomputo_rete::ChiaviConAmbiente(p))
}

#[cfg(not(feature = "chiavi-da-ambiente"))]
fn avvolgi_chiavi(p: opencomputo_rete::PortachiaviSistema) -> Arc<dyn ArchivioChiavi> {
    Arc::new(p)
}

/// Configurazione salvata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Configurazione {
    /// Modello di embedding (OpenAI).
    pub modello_embedding: String,
    /// Dimensioni dei vettori (assenti: quelle del modello).
    pub dimensioni_embedding: Option<u32>,
    /// Modello di generazione (Anthropic).
    pub modello_generazione: String,
    /// Modalità di output strutturato.
    pub modalita_output: ModalitaOutput,
    /// Limite di token dell'analisi.
    pub max_token_analisi: u32,
    /// Limite di token della selezione.
    pub max_token_selezione: u32,
    /// Consenso all'invio dei dati.
    pub consenso: Consenso,
}

impl Default for Configurazione {
    fn default() -> Self {
        let p = Parametri::default();
        Self {
            modello_embedding: openai::MODELLO_PREDEFINITO.into(),
            dimensioni_embedding: None,
            modello_generazione: anthropic::MODELLO_PREDEFINITO.into(),
            modalita_output: ModalitaOutput::Automatica,
            max_token_analisi: p.max_token_analisi,
            max_token_selezione: p.max_token_selezione,
            consenso: Consenso::default(),
        }
    }
}

/// Modifiche alla configurazione (campi assenti invariati).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modifiche {
    /// Modello di embedding.
    #[serde(default)]
    pub modello_embedding: Option<String>,
    /// Dimensioni (0: quelle del modello).
    #[serde(default)]
    pub dimensioni_embedding: Option<u32>,
    /// Modello di generazione.
    #[serde(default)]
    pub modello_generazione: Option<String>,
    /// Modalità di output.
    #[serde(default)]
    pub modalita_output: Option<ModalitaOutput>,
    /// Limite di token dell'analisi.
    #[serde(default)]
    pub max_token_analisi: Option<u32>,
    /// Limite di token della selezione.
    #[serde(default)]
    pub max_token_selezione: Option<u32>,
}

/// Chiave API ricevuta in una richiesta: non compare in `Debug`.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct ChiaveApi(String);

impl ChiaveApi {
    /// Valore.
    #[must_use]
    pub fn valore(&self) -> &str {
        &self.0
    }
}

impl From<String> for ChiaveApi {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Debug for ChiaveApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChiaveApi(***)")
    }
}

/// Interruttore delle operazioni lunghe: annulla quella in corso senza passare dal
/// servizio (che durante il lavoro resta libero per altre richieste).
#[derive(Debug, Clone, Default)]
pub struct Interruttore(Arc<Mutex<Annullamento>>);

impl Interruttore {
    fn segnale(&self) -> MutexGuard<'_, Annullamento> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Segnale per una nuova operazione, che diventa quella corrente.
    pub(crate) fn nuovo(&self) -> Annullamento {
        let a = Annullamento::new();
        *self.segnale() = a.clone();
        a
    }

    /// Chiede l'annullamento dell'operazione corrente: si ferma prima della prossima
    /// chiamata a un provider (o del prossimo nuovo tentativo).
    pub fn annulla(&self) {
        self.segnale().annulla();
    }
}

/// Indici di un prezzario caricato.
struct Indici {
    impronta: String,
    da: Vec<VoceDaIndicizzare>,
    testo: Option<IndiceTestuale>,
    vettori: Option<IndiceVettoriale>,
}

/// Indici di un prezzario, con il proprio lucchetto.
type Posto = Arc<Mutex<Option<Indici>>>;

/// Funzione che riceve gli avanzamenti delle operazioni lunghe.
pub type Avanzamento = Box<dyn Fn(&Value) + Send>;

/// Stato dell'assistente nel servizio.
pub struct Assistente {
    ambiente: Ambiente,
    config: Configurazione,
    indici: BTreeMap<String, Posto>,
    ultima: Option<Proposta>,
}

impl std::fmt::Debug for Assistente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Assistente")
            .field("ambiente", &self.ambiente)
            .field("config", &self.config)
            .field(
                "prezzari_indicizzati",
                &self.indici.keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

pub(crate) fn errore(motivo: &str, messaggio: impl Into<String>) -> ErroreApplicazione {
    ErroreApplicazione::Assistente {
        motivo: motivo.to_owned(),
        messaggio: messaggio.into(),
    }
}

fn da_ai(e: &ErroreAi) -> ErroreApplicazione {
    errore(e.codice(), e.to_string())
}

fn da_flusso(e: &ErroreFlusso) -> ErroreApplicazione {
    match e {
        ErroreFlusso::Ai(a) => da_ai(a),
        ErroreFlusso::Analisi { .. } => errore("analisi_non_valida", e.to_string()),
        ErroreFlusso::Selezione(_) => errore("selezione_non_valida", e.to_string()),
        ErroreFlusso::NessunCandidato => errore("nessun_candidato", e.to_string()),
    }
}

fn file_errore(percorso: &Path, e: &std::io::Error) -> ErroreApplicazione {
    ErroreApplicazione::File {
        messaggio: format!("{}: {e}", percorso.display()),
    }
}

/// Scrive un file sostituendo il precedente solo a scrittura completata.
fn scrivi_atomico(percorso: &Path, byte: &[u8]) -> Result<(), ErroreApplicazione> {
    if let Some(c) = percorso.parent() {
        std::fs::create_dir_all(c).map_err(|e| file_errore(c, &e))?;
    }
    let mut tmp = percorso.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, byte).map_err(|e| file_errore(&tmp, &e))?;
    std::fs::rename(&tmp, percorso).map_err(|e| file_errore(percorso, &e))
}

fn adesso() -> String {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    chrono::DateTime::from_timestamp(i64::try_from(s).unwrap_or(0), 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

/// Ciò che un lavoro richiede ai provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bisogno {
    /// Nessuna chiamata obbligatoria (gli embedding si usano se disponibili).
    Niente,
    /// Embedding obbligatori (indicizzazione).
    Embedding,
    /// Generazione obbligatoria (proposta); embedding se disponibili.
    Generazione,
}

/// Strumenti di un lavoro, presi dal servizio e usati senza di esso.
pub(crate) struct Attrezzi {
    cartella: PathBuf,
    spazio: Option<SpazioEmbedding>,
    embedding: Option<ClienteEmbedding>,
    generazione: Option<ClienteGenerazione>,
    parametri: Parametri,
    posto: Posto,
}

impl Assistente {
    /// Assistente con l'ambiente indicato; legge la configurazione se esiste.
    ///
    /// # Errors
    /// File di configurazione presente ma non leggibile.
    pub fn new(ambiente: Ambiente) -> Result<Self, ErroreApplicazione> {
        let p = ambiente.cartella.join(FILE_CONFIGURAZIONE);
        let config = match std::fs::read(&p) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| ErroreApplicazione::File {
                messaggio: format!("{}: {e}", p.display()),
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Configurazione::default(),
            Err(e) => return Err(file_errore(&p, &e)),
        };
        Ok(Self {
            ambiente,
            config,
            indici: BTreeMap::new(),
            ultima: None,
        })
    }

    /// Dimentica l'ultima proposta (il progetto aperto è cambiato).
    pub fn dimentica_proposta(&mut self) {
        self.ultima = None;
    }

    /// Conserva la proposta da applicare (o nessuna).
    pub(crate) fn conserva_proposta(&mut self, p: Option<Proposta>) {
        self.ultima = p;
    }

    fn salva_config(&self) -> Result<(), ErroreApplicazione> {
        let b = serde_json::to_vec_pretty(&self.config).unwrap_or_default();
        scrivi_atomico(&self.ambiente.cartella.join(FILE_CONFIGURAZIONE), &b)
    }

    fn chiave(&self, p: Provider) -> Result<Option<Segreto>, ErroreApplicazione> {
        self.ambiente
            .chiavi
            .leggi(p)
            .map_err(|e| errore("portachiavi", e.to_string()))
    }

    fn richiedi_consenso(&self) -> Result<(), ErroreApplicazione> {
        if self.config.consenso.valido() {
            Ok(())
        } else {
            Err(errore(
                "consenso_mancante",
                format!(
                    "serve il consenso all'invio dei dati (versione {}): mostrare il testo e usare `assistente_consenso`",
                    consenso::VERSIONE
                ),
            ))
        }
    }

    /// Stato: configurazione, chiavi presenti, consenso e suo testo, modelli noti. Un
    /// portachiavi non accessibile è riportato in `chiavi.errore`, senza fallire.
    ///
    /// # Errors
    /// Nessuno per ora; il tipo resta quello delle altre richieste.
    pub fn stato(&self) -> Result<Value, ErroreApplicazione> {
        let mut errore_chiavi = None;
        let mut presente = |p: Provider| match self.ambiente.chiavi.leggi(p) {
            Ok(c) => c.is_some(),
            Err(e) => {
                errore_chiavi = Some(e.to_string());
                false
            }
        };
        let openai = presente(Provider::Openai);
        let anthropic = presente(Provider::Anthropic);
        let mut chiavi = json!({
            "openai": openai,
            "anthropic": anthropic,
            "archivio": self.ambiente.chiavi.descrizione(),
        });
        if let (Some(e), Value::Object(m)) = (errore_chiavi, &mut chiavi) {
            m.insert("errore".into(), json!(e));
        }
        Ok(json!({
            "configurazione": self.config,
            "chiavi": chiavi,
            "consenso": {
                "valido": self.config.consenso.valido(),
                "versione_corrente": consenso::VERSIONE,
                "testo": consenso::TESTO,
            },
            "modelli": {
                "embedding": ["text-embedding-3-small", "text-embedding-3-large", "text-embedding-ada-002"],
                "generazione": anthropic::MODELLI_CON_OUTPUT_STRUTTURATI,
                "senza_strumento_forzato": anthropic::MODELLI_SENZA_STRUMENTO_FORZATO,
                "dimensioni_predefinite": dimensioni_predefinite(&self.config.modello_embedding),
            },
            "cartella": self.ambiente.cartella,
        }))
    }

    /// Modifica la configurazione e la salva.
    ///
    /// # Errors
    /// Valori non validi; file non scrivibile.
    pub fn configura(&mut self, m: Modifiche) -> Result<Value, ErroreApplicazione> {
        let mut c = self.config.clone();
        if let Some(x) = m.modello_embedding {
            c.modello_embedding = x.trim().to_owned();
        }
        if let Some(d) = m.dimensioni_embedding {
            c.dimensioni_embedding = (d > 0).then_some(d);
        }
        if let Some(x) = m.modello_generazione {
            c.modello_generazione = x.trim().to_owned();
        }
        if let Some(x) = m.modalita_output {
            c.modalita_output = x;
        }
        if let Some(x) = m.max_token_analisi {
            c.max_token_analisi = x;
        }
        if let Some(x) = m.max_token_selezione {
            c.max_token_selezione = x;
        }
        if c.modello_embedding.is_empty() || c.modello_generazione.is_empty() {
            return Err(errore(
                "configurazione",
                "il nome del modello non può essere vuoto",
            ));
        }
        if c.max_token_analisi < 1_000 || c.max_token_selezione < 1_000 {
            return Err(errore(
                "configurazione",
                "limiti di token troppo bassi (minimo 1000)",
            ));
        }
        if c.dimensioni_embedding.is_some() && !openai::accetta_dimensioni(&c.modello_embedding) {
            return Err(errore(
                "configurazione",
                format!(
                    "il modello «{}» non accetta un numero di dimensioni diverso dal suo",
                    c.modello_embedding
                ),
            ));
        }
        anthropic::modalita_per(&c.modello_generazione, c.modalita_output)
            .map_err(|e| da_ai(&e))?;
        self.config = c;
        self.salva_config()?;
        self.stato()
    }

    /// Imposta o elimina (chiave vuota o assente) la chiave di un provider.
    ///
    /// # Errors
    /// Portachiavi non scrivibile.
    pub fn imposta_chiave(
        &self,
        p: Provider,
        chiave: Option<&str>,
    ) -> Result<Value, ErroreApplicazione> {
        let s = Segreto::new(chiave.unwrap_or(""));
        let r = if s.vuoto() {
            self.ambiente.chiavi.elimina(p)
        } else {
            self.ambiente.chiavi.scrivi(p, &s)
        };
        r.map_err(|e| errore("portachiavi", e.to_string()))?;
        Ok(
            json!({"provider": p, "impostata": !s.vuoto(), "archivio": self.ambiente.chiavi.descrizione()}),
        )
    }

    /// Registra o revoca il consenso.
    ///
    /// # Errors
    /// Versione accettata diversa da quella corrente; file non scrivibile.
    pub fn consenso(
        &mut self,
        accetto: bool,
        versione: Option<u32>,
    ) -> Result<Value, ErroreApplicazione> {
        if accetto {
            if versione != Some(consenso::VERSIONE) {
                return Err(errore(
                    "consenso_versione",
                    format!(
                        "il testo corrente è la versione {}: mostrarlo e accettare quella",
                        consenso::VERSIONE
                    ),
                ));
            }
            self.config.consenso = Consenso {
                versione: consenso::VERSIONE,
                data: Some(adesso()),
            };
        } else {
            self.config.consenso = Consenso::default();
        }
        self.salva_config()?;
        Ok(json!({"valido": self.config.consenso.valido(), "data": self.config.consenso.data}))
    }

    fn cliente_embedding(&self) -> Result<ClienteEmbedding, ErroreApplicazione> {
        let chiave = self.chiave(Provider::Openai)?.ok_or_else(|| {
            da_ai(&ErroreAi::ChiaveAssente {
                provider: Provider::Openai,
            })
        })?;
        ClienteEmbedding::new(
            ConfigEmbedding {
                modello: self.config.modello_embedding.clone(),
                dimensioni: self.config.dimensioni_embedding,
                url: self.ambiente.indirizzi.embedding.clone(),
                ..ConfigEmbedding::default()
            },
            chiave,
            self.ambiente.trasporto.clone(),
            self.ambiente.attesa.clone(),
        )
        .map_err(|e| da_ai(&e))
    }

    fn cliente_generazione(&self) -> Result<ClienteGenerazione, ErroreApplicazione> {
        let chiave = self.chiave(Provider::Anthropic)?.ok_or_else(|| {
            da_ai(&ErroreAi::ChiaveAssente {
                provider: Provider::Anthropic,
            })
        })?;
        ClienteGenerazione::new(
            ConfigGenerazione {
                modello: self.config.modello_generazione.clone(),
                modalita: self.config.modalita_output,
                url: self.ambiente.indirizzi.generazione.clone(),
                ..ConfigGenerazione::default()
            },
            chiave,
            self.ambiente.trasporto.clone(),
            self.ambiente.attesa.clone(),
        )
        .map_err(|e| da_ai(&e))
    }

    /// Spazio di embedding della configurazione, se determinabile senza chiamate.
    fn spazio_configurato(&self) -> Option<SpazioEmbedding> {
        let m = &self.config.modello_embedding;
        let d = self
            .config
            .dimensioni_embedding
            .or_else(|| dimensioni_predefinite(m))?;
        Some(SpazioEmbedding::new(m, d))
    }

    /// Strumenti per un lavoro sul prezzario `nome`. Consenso e chiavi si controllano qui,
    /// prima di qualsiasi lavoro.
    ///
    /// # Errors
    /// Consenso mancante o chiave assente per ciò che il lavoro richiede.
    pub(crate) fn attrezzi(
        &mut self,
        nome: &str,
        bisogno: Bisogno,
    ) -> Result<Attrezzi, ErroreApplicazione> {
        if bisogno != Bisogno::Niente {
            self.richiedi_consenso()?;
        }
        let consenso = self.config.consenso.valido();
        let embedding = match bisogno {
            Bisogno::Embedding => Some(self.cliente_embedding()?),
            _ if consenso => self.cliente_embedding().ok(),
            _ => None,
        };
        let generazione = match bisogno {
            Bisogno::Generazione => Some(self.cliente_generazione()?),
            _ => None,
        };
        Ok(Attrezzi {
            cartella: self.ambiente.cartella.clone(),
            spazio: self.spazio_configurato(),
            embedding,
            generazione,
            parametri: Parametri {
                max_token_analisi: self.config.max_token_analisi,
                max_token_selezione: self.config.max_token_selezione,
                ..Parametri::default()
            },
            posto: self.indici.entry(nome.to_owned()).or_default().clone(),
        })
    }

    /// Comandi dell'ultima proposta, da eseguire nella sessione; la proposta è consumata.
    ///
    /// # Errors
    /// Nessuna proposta da applicare.
    pub fn da_applicare(
        &mut self,
    ) -> Result<Vec<opencomputo_application::Comando>, ErroreApplicazione> {
        let p = self.ultima.take().ok_or_else(|| {
            errore(
                "nessuna_proposta",
                "nessuna proposta da applicare: usare `assistente_proponi`",
            )
        })?;
        if p.comandi.is_empty() {
            return Err(errore(
                "nessuna_proposta",
                "la proposta non contiene voci da aggiungere",
            ));
        }
        Ok(p.comandi)
    }
}

impl Attrezzi {
    fn percorso_vettori(&self, impronta: &str, spazio: &SpazioEmbedding) -> PathBuf {
        let breve: String = impronta.chars().take(16).collect();
        self.cartella
            .join("cache")
            .join(format!("vettori-{breve}-{}.bin", spazio.nome_file()))
    }

    fn posto(&self) -> MutexGuard<'_, Option<Indici>> {
        self.posto
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Indici del prezzario, ricostruiti se il contenuto è cambiato. L'indice testuale si
    /// costruisce solo se `testo`; l'indice vettoriale è quello dello spazio configurato,
    /// letto dalla cache se c'è.
    fn indici<'a>(
        &self,
        posto: &'a mut Option<Indici>,
        p: &Prezzario,
        testo: bool,
    ) -> &'a mut Indici {
        let da = da_indicizzare(p, LimitiLotto::default().caratteri_per_testo);
        let impronta = impronta_prezzario(&da);
        if posto.as_ref().is_some_and(|i| i.impronta != impronta) {
            *posto = None;
        }
        let i = posto.get_or_insert_with(|| Indici {
            impronta,
            da,
            testo: None,
            vettori: None,
        });
        if testo && i.testo.is_none() {
            i.testo = Some(IndiceTestuale::new(p));
        }
        if i.vettori
            .as_ref()
            .is_some_and(|v| Some(v.spazio()) != self.spazio.as_ref())
        {
            i.vettori = None;
        }
        if i.vettori.is_none()
            && let Some(spazio) = &self.spazio
            && let Ok(b) = std::fs::read(self.percorso_vettori(&i.impronta, spazio))
        {
            // Una cache illeggibile o di un altro spazio si ignora: sarà ricalcolata.
            i.vettori = IndiceVettoriale::decodifica(&b, spazio).ok();
        }
        i
    }

    /// Indice della cache più recente dello spazio indicato, se leggibile.
    fn cache_recente(&self, spazio: &SpazioEmbedding) -> Option<IndiceVettoriale> {
        let fine = format!("-{}.bin", spazio.nome_file());
        let piu_recente = std::fs::read_dir(self.cartella.join("cache"))
            .ok()?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(&fine))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .max()?;
        let b = std::fs::read(piu_recente.1).ok()?;
        IndiceVettoriale::decodifica(&b, spazio).ok()
    }

    /// Client degli embedding utilizzabile con l'indice: completo e dello stesso spazio.
    fn embedding_per(&self, i: &Indici) -> Option<&ClienteEmbedding> {
        let v = i.vettori.as_ref()?;
        self.embedding
            .as_ref()
            .filter(|c| v.completo(&i.da) && &c.spazio() == v.spazio())
    }

    /// Stato dell'indice di un prezzario, senza chiamate.
    pub(crate) fn indice(&self, nome: &str, p: &Prezzario) -> Value {
        let mut posto = self.posto();
        let i = self.indici(&mut posto, p, false);
        let aggiornate = i
            .vettori
            .as_ref()
            .map_or(0, |v| i.da.len() - v.mancanti(&i.da).len());
        json!({
            "prezzario": nome,
            "voci": i.da.len(),
            "indicizzate": aggiornate,
            "completo": !i.da.is_empty() && aggiornate == i.da.len(),
            "spazio": self.spazio,
            "ricerca": if self.embedding_per(i).is_some() { "ibrida" } else { "testuale" },
        })
    }

    /// Calcola (o completa) l'indice vettoriale di un prezzario.
    pub(crate) fn indicizza(
        &self,
        nome: &str,
        p: &Prezzario,
        annullamento: &Annullamento,
        avanzamento: Option<&dyn Fn(&Value)>,
    ) -> Result<Value, ErroreApplicazione> {
        let cliente = self
            .embedding
            .as_ref()
            .ok_or_else(|| errore("interno", "client degli embedding assente"))?;
        let spazio = cliente.spazio();
        let mut posto = self.posto();
        let i = self.indici(&mut posto, p, false);
        if i.vettori.as_ref().is_none_or(|v| v.spazio() != &spazio) {
            // Prezzario cambiato (o nuovo): si parte dalla cache più recente dello stesso
            // spazio; le voci con lo stesso codice e lo stesso testo non si ricalcolano.
            i.vettori = Some(
                self.cache_recente(&spazio)
                    .unwrap_or_else(|| IndiceVettoriale::new(spazio.clone(), &i.impronta)),
            );
        }
        let percorso = self.percorso_vettori(&i.impronta, &spazio);
        let Some(vettori) = i.vettori.as_mut() else {
            return Err(errore("interno", "indice vettoriale assente"));
        };
        // Il file può essere grande (decine di MB): si salva al più ogni 30 secondi e alla
        // fine, anche se l'indicizzazione si interrompe.
        let mut ultimo = Instant::now();
        let mut errore_salvataggio = None;
        let r = indicizza(cliente, vettori, &i.da, annullamento, |iv, a| {
            if ultimo.elapsed() >= SALVATAGGIO_CACHE {
                if let Err(e) = scrivi_atomico(&percorso, &iv.codifica()) {
                    errore_salvataggio = Some(e);
                }
                ultimo = Instant::now();
            }
            if let Some(f) = avanzamento {
                f(&json!({"avanzamento": "indicizzazione", "prezzario": nome, "stato": a}));
            }
        });
        let salvato = scrivi_atomico(&percorso, &vettori.codifica());
        let r = r.map_err(|e| da_ai(&e))?;
        if let Some(e) = errore_salvataggio {
            return Err(e);
        }
        salvato?;
        Ok(json!({"prezzario": nome, "spazio": spazio, "rapporto": r, "file": percorso}))
    }

    /// Ricerca nel prezzario: ibrida se l'indice vettoriale è completo e utilizzabile,
    /// altrimenti solo testuale (anche senza rete).
    pub(crate) fn cerca(
        &self,
        p: &Prezzario,
        testo: &str,
        limite: usize,
        dimensione: Option<&str>,
        annullamento: &Annullamento,
    ) -> Result<Value, ErroreApplicazione> {
        let mut posto = self.posto();
        let i = self.indici(&mut posto, p, true);
        let filtro = FiltroVoci {
            dimensioni: dimensione.map(dimensioni_attese).unwrap_or_default(),
            ..FiltroVoci::default()
        };
        let vettore = match self.embedding_per(i) {
            Some(c) => c
                .embed(&[testo.to_owned()], annullamento)
                .map_err(|e| da_ai(&e))?
                .into_iter()
                .next(),
            None => None,
        };
        let Some(indice_testo) = i.testo.as_ref() else {
            return Err(errore("interno", "indice testuale assente"));
        };
        let codici = cerca_ibrida(
            indice_testo,
            i.vettori.as_ref().zip(vettore.as_deref()),
            testo,
            limite,
            &filtro,
        );
        let voci: Vec<Value> = codici
            .iter()
            .filter_map(|c| p.voce_per_codice(c))
            .map(|v| {
                json!({
                    "codice": v.codice,
                    "descrizione": v.descrizione_breve,
                    "unita_misura": v.unita_misura,
                    "prezzo": v.prezzo,
                })
            })
            .collect();
        Ok(json!({"ricerca": if vettore.is_some() { "ibrida" } else { "testuale" }, "voci": voci}))
    }

    /// Proposta di computo dalla richiesta: due chiamate al modello e validazione. La
    /// prova nella sessione la fa chi conclude il lavoro.
    pub(crate) fn proponi(
        &self,
        nome: &str,
        p: &Prezzario,
        progetto: &Progetto,
        richiesta: &str,
        computo: Option<&str>,
        annullamento: &Annullamento,
    ) -> Result<(Value, Option<Proposta>), ErroreApplicazione> {
        let generazione = self
            .generazione
            .as_ref()
            .ok_or_else(|| errore("interno", "client della generazione assente"))?;
        let mut posto = self.posto();
        let i = self.indici(&mut posto, p, true);
        let emb = self.embedding_per(i);
        let avviso_vettori = match (&i.vettori, emb) {
            (_, Some(_)) => None,
            (Some(v), None) if !v.completo(&i.da) => Some(
                "indice vettoriale incompleto: ricerca solo testuale (indicizzare il prezzario)",
            ),
            _ => Some("nessun indice vettoriale utilizzabile: ricerca solo testuale"),
        };
        let Some(indice_testo) = i.testo.as_ref() else {
            return Err(errore("interno", "indice testuale assente"));
        };
        let esito = flusso::proponi(
            &Strumenti {
                generazione,
                embedding: emb.map(|c| c as &dyn Embedding),
                indice_testo,
                indice_vettori: if emb.is_some() {
                    i.vettori.as_ref()
                } else {
                    None
                },
            },
            &flusso::Richiesta {
                testo: richiesta,
                nome_prezzario: nome,
                prezzario: p,
                progetto,
                computo,
            },
            &self.parametri,
            annullamento,
        )
        .map_err(|e| da_flusso(&e))?;
        let mut v = serde_json::to_value(&esito).unwrap_or(Value::Null);
        if let (Some(a), Value::Object(m)) = (avviso_vettori, &mut v) {
            m.insert("avviso_ricerca".into(), json!(a));
        }
        let proposta = match esito {
            EsitoFlusso::Proposta { proposta, .. } => Some(*proposta),
            EsitoFlusso::DaChiarire { .. } => None,
        };
        Ok((v, proposta))
    }
}

/// Tipo di lavoro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TipoLavoro {
    /// Stato dell'indice.
    Indice,
    /// Indicizzazione.
    Indicizza,
    /// Ricerca.
    Cerca {
        testo: String,
        limite: usize,
        unita: Option<String>,
    },
    /// Proposta.
    Proponi {
        testo: String,
        computo: Option<String>,
    },
}

/// Operazione lunga dell'assistente, eseguibile senza tenere il servizio: si ottiene da
/// [`crate::servizio::Servizio::avvia`], si esegue con [`Lavoro::esegui`] (anche in un
/// altro thread) e si conclude con [`crate::servizio::Servizio::completa`].
pub struct Lavoro {
    pub(crate) id: Option<Value>,
    pub(crate) generazione: u64,
    pub(crate) tipo: TipoLavoro,
    pub(crate) attrezzi: Attrezzi,
    pub(crate) nome: String,
    pub(crate) prezzario: Prezzario,
    pub(crate) progetto: Option<Progetto>,
    pub(crate) annullamento: Annullamento,
}

impl std::fmt::Debug for Lavoro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lavoro")
            .field("tipo", &self.tipo)
            .field("prezzario", &self.nome)
            .finish_non_exhaustive()
    }
}

/// Risultato di un [`Lavoro`], da concludere con il servizio.
pub struct EsitoLavoro {
    pub(crate) id: Option<Value>,
    pub(crate) generazione: u64,
    pub(crate) proposta_attesa: bool,
    pub(crate) risultato: Result<Value, ErroreApplicazione>,
    pub(crate) proposta: Option<Proposta>,
}

impl std::fmt::Debug for EsitoLavoro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EsitoLavoro")
            .field("ok", &self.risultato.is_ok())
            .finish_non_exhaustive()
    }
}

impl Lavoro {
    /// `true` se il lavoro chiama (o può chiamare) i provider.
    #[must_use]
    pub fn in_rete(&self) -> bool {
        !matches!(self.tipo, TipoLavoro::Indice)
    }

    /// Esegue il lavoro. `avanzamento` riceve gli oggetti di avanzamento
    /// dell'indicizzazione.
    #[must_use]
    pub fn esegui(self, avanzamento: Option<&dyn Fn(&Value)>) -> EsitoLavoro {
        let a = &self.attrezzi;
        let mut proposta = None;
        let risultato = match &self.tipo {
            TipoLavoro::Indice => Ok(a.indice(&self.nome, &self.prezzario)),
            TipoLavoro::Indicizza => {
                a.indicizza(&self.nome, &self.prezzario, &self.annullamento, avanzamento)
            }
            TipoLavoro::Cerca {
                testo,
                limite,
                unita,
            } => a.cerca(
                &self.prezzario,
                testo,
                *limite,
                unita.as_deref(),
                &self.annullamento,
            ),
            TipoLavoro::Proponi { testo, computo } => match &self.progetto {
                Some(prog) => a
                    .proponi(
                        &self.nome,
                        &self.prezzario,
                        prog,
                        testo,
                        computo.as_deref(),
                        &self.annullamento,
                    )
                    .map(|(v, p)| {
                        proposta = p;
                        v
                    }),
                None => Err(errore("interno", "progetto assente")),
            },
        };
        EsitoLavoro {
            id: self.id,
            generazione: self.generazione,
            proposta_attesa: matches!(self.tipo, TipoLavoro::Proponi { .. }),
            risultato,
            proposta,
        }
    }
}
