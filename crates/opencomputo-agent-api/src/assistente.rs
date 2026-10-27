//! Assistente AI nel servizio JSON (ADR 0025): configurazione, chiavi, consenso,
//! indicizzazione dei prezzari, ricerca e proposte di computo.
//!
//! * La configurazione (modelli, limiti, consenso) sta in `assistente.json` nella
//!   cartella di configurazione; le chiavi solo nel portachiavi del sistema.
//! * L'indice vettoriale di ogni prezzario è una cache in `cache/`, con nel nome
//!   l'impronta del contenuto del prezzario e lo spazio di embedding; viene salvato dopo
//!   ogni lotto, così un'indicizzazione interrotta riprende da dove era arrivata.
//! * Nessuna chiamata ai provider senza il consenso alla versione corrente del testo.
//! * Le proposte non modificano il progetto: si provano subito e si applicano solo con
//!   la richiesta `assistente_applica`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use opencomputo_application::{ErroreApplicazione, Sessione};
use opencomputo_assistente::candidati::{cerca_ibrida, dimensioni_attese};
use opencomputo_assistente::consenso::{self, Consenso};
use opencomputo_assistente::flusso::{self, ErroreFlusso, EsitoFlusso, Parametri, Strumenti};
use opencomputo_assistente::proposta::Proposta;
use opencomputo_assistente::provider::anthropic::{
    ClienteGenerazione, ConfigGenerazione, ModalitaOutput,
};
use opencomputo_assistente::provider::chiavi::{ArchivioChiavi, ChiaviInMemoria};
use opencomputo_assistente::provider::openai::{
    ClienteEmbedding, ConfigEmbedding, LimitiLotto, dimensioni_predefinite,
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
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Nome del file di configurazione.
pub const FILE_CONFIGURAZIONE: &str = "assistente.json";

/// Intervallo minimo tra due salvataggi della cache durante l'indicizzazione.
const SALVATAGGIO_CACHE: Duration = Duration::from_secs(30);

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
}

impl std::fmt::Debug for Ambiente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ambiente")
            .field("cartella", &self.cartella)
            .field("chiavi", &self.chiavi.descrizione())
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
    #[must_use]
    pub fn di_sistema() -> Self {
        let chiavi: Arc<dyn ArchivioChiavi> = match opencomputo_rete::PortachiaviSistema::new() {
            Ok(p) => avvolgi_chiavi(p),
            Err(_) => Arc::new(ChiaviInMemoria::default()),
        };
        let trasporto: Arc<dyn Trasporto> = match opencomputo_rete::TrasportoHttps::new() {
            Ok(t) => Arc::new(t),
            Err(e) => Arc::new(TrasportoAssente(e.to_string())),
        };
        Self {
            cartella: cartella_predefinita(),
            chiavi,
            trasporto,
            attesa: Arc::new(opencomputo_rete::AttesaReale),
        }
    }
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
            modello_embedding: opencomputo_assistente::provider::openai::MODELLO_PREDEFINITO.into(),
            dimensioni_embedding: None,
            modello_generazione: opencomputo_assistente::provider::anthropic::MODELLO_PREDEFINITO
                .into(),
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

/// Indici di un prezzario caricato.
struct Indici {
    impronta: String,
    da: Vec<VoceDaIndicizzare>,
    testo: IndiceTestuale,
    vettori: Option<IndiceVettoriale>,
}

/// Funzione che riceve gli avanzamenti delle operazioni lunghe.
pub type Avanzamento = Box<dyn Fn(&Value) + Send>;

/// Stato dell'assistente nel servizio.
pub struct Assistente {
    ambiente: Ambiente,
    config: Configurazione,
    indici: BTreeMap<String, Indici>,
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

fn errore(motivo: &str, messaggio: impl Into<String>) -> ErroreApplicazione {
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

    /// Stato: configurazione, chiavi presenti, consenso, testo del consenso. Un
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
        opencomputo_assistente::provider::anthropic::modalita_per(
            &c.modello_generazione,
            c.modalita_output,
        )
        .map_err(|e| da_ai(&e))?;
        let spazio_cambiato = c.modello_embedding != self.config.modello_embedding
            || c.dimensioni_embedding != self.config.dimensioni_embedding;
        self.config = c;
        self.salva_config()?;
        if spazio_cambiato {
            for i in self.indici.values_mut() {
                i.vettori = None;
            }
        }
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
                ..ConfigGenerazione::default()
            },
            chiave,
            self.ambiente.trasporto.clone(),
            self.ambiente.attesa.clone(),
        )
        .map_err(|e| da_ai(&e))
    }

    fn percorso_vettori(&self, impronta: &str, spazio: &SpazioEmbedding) -> PathBuf {
        let breve: String = impronta.chars().take(16).collect();
        self.ambiente
            .cartella
            .join("cache")
            .join(format!("vettori-{breve}-{}.bin", spazio.nome_file()))
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

    /// Indici del prezzario, ricostruiti se il contenuto è cambiato; l'indice vettoriale
    /// è letto dalla cache, se c'è, quando lo spazio della configurazione è noto.
    fn indici_di(&mut self, nome: &str, p: &Prezzario) -> Result<&mut Indici, ErroreApplicazione> {
        let da = da_indicizzare(p, LimitiLotto::default().caratteri_per_testo);
        let impronta = impronta_prezzario(&da);
        if !self
            .indici
            .get(nome)
            .is_some_and(|i| i.impronta == impronta)
        {
            self.indici.insert(
                nome.to_owned(),
                Indici {
                    impronta: impronta.clone(),
                    da,
                    testo: IndiceTestuale::new(p),
                    vettori: None,
                },
            );
        }
        let cache = self
            .spazio_configurato()
            .map(|s| (self.percorso_vettori(&impronta, &s), s));
        let i = self
            .indici
            .get_mut(nome)
            .ok_or_else(|| errore("interno", "indici assenti"))?;
        if i.vettori.is_none()
            && let Some((percorso, spazio)) = cache
            && let Ok(b) = std::fs::read(&percorso)
        {
            // Una cache illeggibile o di un altro spazio si ignora: sarà ricalcolata.
            i.vettori = IndiceVettoriale::decodifica(&b, &spazio).ok();
        }
        Ok(i)
    }

    /// Calcola (o completa) l'indice vettoriale di un prezzario caricato.
    ///
    /// # Errors
    /// Consenso mancante, chiave assente, errore del provider, cache non scrivibile.
    pub fn indicizza(
        &mut self,
        sessione: &Sessione,
        nome: &str,
        avanzamento: Option<&Avanzamento>,
    ) -> Result<Value, ErroreApplicazione> {
        self.richiedi_consenso()?;
        let p = prezzario(sessione, nome)?.clone();
        let cliente = self.cliente_embedding()?;
        let spazio = cliente.spazio();
        let percorso = {
            let i = self.indici_di(nome, &p)?;
            let impronta = i.impronta.clone();
            let da_rifare = i.vettori.as_ref().is_none_or(|v| v.spazio() != &spazio);
            let percorso = self.percorso_vettori(&impronta, &spazio);
            if da_rifare {
                // Prezzario cambiato (o nuovo): si parte dalla cache più recente dello stesso
                // spazio; le voci con lo stesso codice e lo stesso testo non si ricalcolano.
                let base = self
                    .cache_recente(&spazio)
                    .unwrap_or_else(|| IndiceVettoriale::new(spazio.clone(), &impronta));
                if let Some(i) = self.indici.get_mut(nome) {
                    i.vettori = Some(base);
                }
            }
            percorso
        };
        let annullamento = Annullamento::new();
        let i = self
            .indici
            .get_mut(nome)
            .ok_or_else(|| errore("interno", "indici assenti"))?;
        let Some(vettori) = i.vettori.as_mut() else {
            return Err(errore("interno", "indice vettoriale assente"));
        };
        // Il file può essere grande (decine di MB): si salva al più ogni 30 secondi e alla
        // fine, anche se l'indicizzazione si interrompe.
        let mut ultimo = Instant::now();
        let mut errore_salvataggio = None;
        let r = indicizza(&cliente, vettori, &i.da, &annullamento, |iv, a| {
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

    /// Indice della cache più recente dello spazio indicato, se leggibile.
    fn cache_recente(&self, spazio: &SpazioEmbedding) -> Option<IndiceVettoriale> {
        let fine = format!("-{}.bin", spazio.nome_file());
        let piu_recente = std::fs::read_dir(self.ambiente.cartella.join("cache"))
            .ok()?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(&fine))
            .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
            .max()?;
        let b = std::fs::read(piu_recente.1).ok()?;
        IndiceVettoriale::decodifica(&b, spazio).ok()
    }

    /// `true` se l'indice vettoriale c'è, è completo e si possono fare chiamate.
    fn embedding_usabile(&self, nome: &str) -> Option<ClienteEmbedding> {
        let i = self.indici.get(nome)?;
        let v = i.vettori.as_ref()?;
        if !v.completo(&i.da) || !self.config.consenso.valido() {
            return None;
        }
        self.cliente_embedding()
            .ok()
            .filter(|c| &c.spazio() == v.spazio())
    }

    /// Ricerca nel prezzario: ibrida se l'indice vettoriale è completo e utilizzabile,
    /// altrimenti solo testuale (anche senza rete).
    ///
    /// # Errors
    /// Prezzario non caricato; errore del provider nella ricerca ibrida.
    pub fn cerca(
        &mut self,
        sessione: &Sessione,
        nome: &str,
        testo: &str,
        limite: usize,
        dimensione: Option<&str>,
    ) -> Result<Value, ErroreApplicazione> {
        let p = prezzario(sessione, nome)?.clone();
        self.indici_di(nome, &p)?;
        let emb = self.embedding_usabile(nome);
        let i = self
            .indici
            .get(nome)
            .ok_or_else(|| errore("interno", "indici assenti"))?;
        let filtro = FiltroVoci {
            dimensioni: dimensione.map(dimensioni_attese).unwrap_or_default(),
            ..FiltroVoci::default()
        };
        let vettore = match (&emb, &i.vettori) {
            (Some(c), Some(_)) => c
                .embed(&[testo.to_owned()], &Annullamento::new())
                .map_err(|e| da_ai(&e))?
                .into_iter()
                .next(),
            _ => None,
        };
        let codici = cerca_ibrida(
            &i.testo,
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

    /// Proposta di computo dalla richiesta: due chiamate al modello, validazione e prova
    /// nella sessione. Il progetto non cambia.
    ///
    /// # Errors
    /// Consenso mancante, chiave assente, errore del provider o del flusso.
    pub fn proponi(
        &mut self,
        sessione: &Sessione,
        nome: &str,
        richiesta: &str,
        computo: Option<&str>,
    ) -> Result<Value, ErroreApplicazione> {
        if richiesta.trim().is_empty() {
            return Err(errore("richiesta_vuota", "la richiesta è vuota"));
        }
        self.richiedi_consenso()?;
        let generazione = self.cliente_generazione()?;
        let p = prezzario(sessione, nome)?.clone();
        self.indici_di(nome, &p)?;
        let emb = self.embedding_usabile(nome);
        let i = self
            .indici
            .get(nome)
            .ok_or_else(|| errore("interno", "indici assenti"))?;
        let avviso_vettori = match (&i.vettori, &emb) {
            (_, Some(_)) => None,
            (Some(v), None) if !v.completo(&i.da) => Some(
                "indice vettoriale incompleto: ricerca solo testuale (usare `assistente_indicizza`)",
            ),
            _ => Some("nessun indice vettoriale utilizzabile: ricerca solo testuale"),
        };
        let parametri = Parametri {
            max_token_analisi: self.config.max_token_analisi,
            max_token_selezione: self.config.max_token_selezione,
            ..Parametri::default()
        };
        let esito = flusso::proponi(
            &Strumenti {
                generazione: &generazione,
                embedding: emb.as_ref().map(|c| c as &dyn Embedding),
                indice_testo: &i.testo,
                indice_vettori: if emb.is_some() {
                    i.vettori.as_ref()
                } else {
                    None
                },
            },
            &flusso::Richiesta {
                testo: richiesta,
                nome_prezzario: nome,
                prezzario: &p,
                progetto: sessione.progetto(),
                computo,
            },
            &parametri,
            &Annullamento::new(),
        )
        .map_err(|e| da_flusso(&e))?;
        let mut v = serde_json::to_value(&esito).unwrap_or(Value::Null);
        if let EsitoFlusso::Proposta { proposta, .. } = &esito {
            let prova = match opencomputo_assistente::verifica::prova(sessione, proposta) {
                Ok(e) => json!({"ok": true, "esito": e}),
                Err(e) => json!({"ok": false, "errore": crate::servizio::errore_json(&e)}),
            };
            if let Value::Object(m) = &mut v {
                m.insert("prova".into(), prova);
            }
            self.ultima = Some((**proposta).clone());
        } else {
            self.ultima = None;
        }
        if let (Some(a), Value::Object(m)) = (avviso_vettori, &mut v) {
            m.insert("avviso_ricerca".into(), json!(a));
        }
        Ok(v)
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

fn prezzario<'a>(s: &'a Sessione, nome: &str) -> Result<&'a Prezzario, ErroreApplicazione> {
    s.prezzari()
        .iter()
        .find(|(n, _)| n == nome)
        .map(|(_, p)| p)
        .ok_or_else(|| ErroreApplicazione::Richiesta {
            messaggio: format!("prezzario caricato «{nome}» inesistente: usare `carica_prezzario`"),
        })
}
