//! # opencomputo-rete
//!
//! Collegamento dell'assistente AI (ADR 0025) con il mondo esterno:
//!
//! * [`TrasportoHttps`]: richieste HTTPS bloccanti con `reqwest` e `rustls` (crittografia
//!   `ring`, certificati del sistema operativo), tempi massimi di connessione e di
//!   risposta;
//! * [`AttesaReale`]: pause tra i tentativi;
//! * [`PortachiaviSistema`]: chiavi API nel portachiavi del sistema operativo (Portachiavi
//!   di macOS, Gestione credenziali di Windows, keyring del kernel su Linux).
//!
//! Le chiamate sono bloccanti: un'interfaccia grafica deve eseguirle fuori dal thread
//! principale.

use std::sync::{Arc, Once};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use opencomputo_assistente::provider::chiavi::{ArchivioChiavi, ErroreChiavi};
use opencomputo_assistente::provider::{
    Attesa, ErroreTrasporto, Provider, RichiestaHttp, RispostaHttp, Segreto, Trasporto,
};

/// Dimensione massima di una risposta letta, in byte.
const RISPOSTA_MASSIMA: usize = 64 * 1024 * 1024;
/// Nome del servizio nel portachiavi.
pub const SERVIZIO_PORTACHIAVI: &str = "OpenComputo";

static FORNITORE_TLS: Once = Once::new();

/// Installa `ring` come fornitore crittografico di `rustls` (una volta per processo).
fn installa_tls() {
    FORNITORE_TLS.call_once(|| {
        // Se un'altra parte del programma ne ha già installato uno, resta quello.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Trasporto HTTPS.
#[derive(Debug, Clone)]
pub struct TrasportoHttps {
    client: reqwest::blocking::Client,
}

impl TrasportoHttps {
    /// Nuovo trasporto con tempo massimo di connessione di 10 secondi.
    ///
    /// # Errors
    /// Configurazione TLS non disponibile.
    pub fn new() -> Result<Self, ErroreTrasporto> {
        installa_tls();
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("OpenComputo/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ErroreTrasporto::Altro(format!("client HTTPS non disponibile: {e}")))?;
        Ok(Self { client })
    }
}

fn errore_reqwest(e: &reqwest::Error) -> ErroreTrasporto {
    // Il messaggio di reqwest può contenere l'URL, mai le intestazioni.
    if e.is_timeout() {
        ErroreTrasporto::Timeout
    } else if e.is_connect() || e.is_request() || e.is_body() {
        ErroreTrasporto::Connessione(e.to_string())
    } else {
        ErroreTrasporto::Altro(e.to_string())
    }
}

impl Trasporto for TrasportoHttps {
    fn invia(&self, richiesta: &RichiestaHttp) -> Result<RispostaHttp, ErroreTrasporto> {
        let mut b = self
            .client
            .post(&richiesta.url)
            .timeout(richiesta.timeout)
            .body(richiesta.corpo.clone());
        for (n, v) in &richiesta.intestazioni {
            b = b.header(n.as_str(), v.as_str());
        }
        let r = b.send().map_err(|e| errore_reqwest(&e))?;
        let stato = r.status().as_u16();
        let intestazioni = r
            .headers()
            .iter()
            .filter_map(|(n, v)| Some((n.as_str().to_lowercase(), v.to_str().ok()?.to_owned())))
            .collect();
        let corpo = r.bytes().map_err(|e| errore_reqwest(&e))?;
        if corpo.len() > RISPOSTA_MASSIMA {
            return Err(ErroreTrasporto::Altro(format!(
                "risposta oltre {RISPOSTA_MASSIMA} byte"
            )));
        }
        Ok(RispostaHttp {
            stato,
            intestazioni,
            corpo: corpo.to_vec(),
        })
    }
}

/// Attesa reale con variazione pseudo-casuale dall'orologio.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttesaReale;

impl Attesa for AttesaReale {
    fn attendi(&self, durata: Duration) {
        std::thread::sleep(durata);
    }

    fn variazione_ms(&self, massimo: u64) -> u64 {
        if massimo == 0 {
            return 0;
        }
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::from(d.subsec_nanos()));
        n % (massimo + 1)
    }
}

/// Portachiavi del sistema operativo.
pub struct PortachiaviSistema {
    descrizione: String,
    voci: Vec<(Provider, keyring_core::Entry)>,
}

impl std::fmt::Debug for PortachiaviSistema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortachiaviSistema")
            .field("descrizione", &self.descrizione)
            .finish_non_exhaustive()
    }
}

fn non_disponibile(e: &keyring_core::Error) -> ErroreChiavi {
    ErroreChiavi::NonDisponibile {
        messaggio: e.to_string(),
    }
}

fn utente(p: Provider) -> &'static str {
    match p {
        Provider::Openai => "openai",
        Provider::Anthropic => "anthropic",
    }
}

impl PortachiaviSistema {
    /// Portachiavi del sistema operativo corrente.
    ///
    /// # Errors
    /// Sistema senza portachiavi supportato o portachiavi non accessibile.
    pub fn new() -> Result<Self, ErroreChiavi> {
        #[cfg(target_os = "macos")]
        let (store, descrizione): (Arc<keyring_core::CredentialStore>, &str) = (
            apple_native_keyring_store::keychain::Store::new().map_err(|e| non_disponibile(&e))?,
            "Portachiavi di macOS",
        );
        #[cfg(target_os = "windows")]
        let (store, descrizione): (Arc<keyring_core::CredentialStore>, &str) = (
            windows_native_keyring_store::Store::new().map_err(|e| non_disponibile(&e))?,
            "Gestione credenziali di Windows",
        );
        #[cfg(target_os = "linux")]
        let (store, descrizione): (Arc<keyring_core::CredentialStore>, &str) = (
            linux_keyutils_keyring_store::Store::new().map_err(|e| non_disponibile(&e))?,
            "keyring del kernel Linux (non conservato dopo il riavvio)",
        );
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return Err(ErroreChiavi::NonDisponibile {
            messaggio: "sistema operativo senza portachiavi supportato".into(),
        });
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        Self::con_archivio(&store, descrizione)
    }

    /// Portachiavi su un archivio indicato (per i test, l'archivio finto di
    /// `keyring-core`).
    ///
    /// # Errors
    /// Voci non creabili nell'archivio.
    pub fn con_archivio(
        store: &Arc<keyring_core::CredentialStore>,
        descrizione: &str,
    ) -> Result<Self, ErroreChiavi> {
        let mut voci = Vec::new();
        for p in [Provider::Openai, Provider::Anthropic] {
            let e = store
                .build(SERVIZIO_PORTACHIAVI, utente(p), None)
                .map_err(|e| non_disponibile(&e))?;
            voci.push((p, e));
        }
        Ok(Self {
            descrizione: descrizione.to_owned(),
            voci,
        })
    }

    fn voce(&self, p: Provider) -> Result<&keyring_core::Entry, ErroreChiavi> {
        self.voci
            .iter()
            .find(|(x, _)| *x == p)
            .map(|(_, e)| e)
            .ok_or_else(|| ErroreChiavi::Altro {
                messaggio: format!("voce {} assente", p.nome()),
            })
    }
}

impl ArchivioChiavi for PortachiaviSistema {
    fn leggi(&self, provider: Provider) -> Result<Option<Segreto>, ErroreChiavi> {
        match self.voce(provider)?.get_password() {
            Ok(s) => Ok(Some(Segreto::new(&s)).filter(|s| !s.vuoto())),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(non_disponibile(&e)),
        }
    }

    fn scrivi(&self, provider: Provider, chiave: &Segreto) -> Result<(), ErroreChiavi> {
        if chiave.vuoto() {
            return self.elimina(provider);
        }
        self.voce(provider)?
            .set_password(chiave.esponi())
            .map_err(|e| non_disponibile(&e))
    }

    fn elimina(&self, provider: Provider) -> Result<(), ErroreChiavi> {
        match self.voce(provider)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(non_disponibile(&e)),
        }
    }

    fn descrizione(&self) -> String {
        self.descrizione.clone()
    }
}

/// Archivio che, se manca la chiave, la legge dalle variabili d'ambiente
/// `OPENAI_API_KEY` e `ANTHROPIC_API_KEY`. Solo per lo sviluppo (feature
/// `chiavi-da-ambiente`).
#[cfg(feature = "chiavi-da-ambiente")]
#[derive(Debug)]
pub struct ChiaviConAmbiente<A>(pub A);

#[cfg(feature = "chiavi-da-ambiente")]
impl<A: ArchivioChiavi> ArchivioChiavi for ChiaviConAmbiente<A> {
    fn leggi(&self, provider: Provider) -> Result<Option<Segreto>, ErroreChiavi> {
        if let Some(s) = self.0.leggi(provider)? {
            return Ok(Some(s));
        }
        let var = match provider {
            Provider::Openai => "OPENAI_API_KEY",
            Provider::Anthropic => "ANTHROPIC_API_KEY",
        };
        Ok(std::env::var(var)
            .ok()
            .map(|v| Segreto::new(&v))
            .filter(|s| !s.vuoto()))
    }
    fn scrivi(&self, provider: Provider, chiave: &Segreto) -> Result<(), ErroreChiavi> {
        self.0.scrivi(provider, chiave)
    }
    fn elimina(&self, provider: Provider) -> Result<(), ErroreChiavi> {
        self.0.elimina(provider)
    }
    fn descrizione(&self) -> String {
        format!("{} o variabili d'ambiente (sviluppo)", self.0.descrizione())
    }
}
