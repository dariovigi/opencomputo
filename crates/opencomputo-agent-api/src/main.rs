//! Binario `opencomputo`: comandi singoli in JSON e servizio a righe JSON.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::NaiveDate;
use clap::{Parser, Subcommand, ValueEnum};
use opencomputo_agent_api::servizio::{Richiesta, Servizio, leggi, risposta};
use opencomputo_agent_api::uscita;
use opencomputo_application::casi_uso::{
    FormatoReport, IncidenzeReport, StampaTradizionale, TipoReport,
};
use opencomputo_application::{Comando, ErroreApplicazione, Politica};
use serde_json::Value;

/// OpenComputo: computi metrici estimativi e contabilità dei lavori pubblici.
///
/// Ogni comando stampa su stdout un oggetto JSON {"ok": true, "risultato": …} oppure
/// {"ok": false, "errore": …}. Codici di uscita: 0 eseguito, 1 rifiutato o progetto
/// non valido, 2 argomenti errati, 3 errore di file o di formato, 4 errore dell'assistente
/// AI.
#[derive(Debug, Parser)]
#[command(name = "opencomputo", version)]
struct Cli {
    /// JSON su una sola riga invece che indentato.
    #[arg(long, global = true)]
    compatto: bool,
    #[command(subcommand)]
    comando: Sotto,
}

#[derive(Debug, Subcommand)]
enum Sotto {
    /// Crea un file di progetto vuoto.
    Nuovo {
        /// File da creare (estensione .ocp).
        file: PathBuf,
        /// Titolo del progetto.
        #[arg(long, default_value = "Nuovo progetto")]
        titolo: String,
        /// Sovrascrive un file esistente (ne resta una copia di sicurezza).
        #[arg(long)]
        forza: bool,
        /// Salva compresso con gzip.
        #[arg(long)]
        gzip: bool,
    },
    /// Crea un file di progetto da un XPWE o da un DCF non protetto.
    Importa {
        /// File XPWE o DCF.
        origine: PathBuf,
        /// File di progetto da creare.
        file: PathBuf,
        /// Sovrascrive un file esistente.
        #[arg(long)]
        forza: bool,
        /// Salva compresso con gzip.
        #[arg(long)]
        gzip: bool,
    },
    /// Stato sintetico: totali dei computi, SAL, numero di problemi.
    Info {
        /// File di progetto.
        file: PathBuf,
    },
    /// Validazione completa (codice di uscita 1 se ci sono errori).
    Valida {
        /// File di progetto.
        file: PathBuf,
    },
    /// Cerca voci nell'elenco prezzi del progetto o in un prezzario esterno.
    Cerca {
        /// File di progetto.
        file: PathBuf,
        /// Parole da cercare (tutte devono comparire).
        #[arg(required = true)]
        testo: Vec<String>,
        /// Cerca in questo prezzario (DCF, XPWE, .ocp, listino CSV o JSON) invece che nel progetto.
        #[arg(long)]
        prezzario: Option<PathBuf>,
        /// Voci restituite al massimo.
        #[arg(long, default_value_t = opencomputo_agent_api::servizio::LIMITE_RICERCA)]
        limite: usize,
    },
    /// Esegue comandi JSON sul progetto e lo salva (richiesta atomica).
    ///
    /// I comandi si leggono da --comandi o da stdin: una lista, un singolo comando
    /// oppure un oggetto {"comandi": [...]}. Vedi `opencomputo schema-comandi`.
    Esegui {
        /// File di progetto.
        file: PathBuf,
        /// File JSON con i comandi ("-" per stdin, predefinito).
        #[arg(long)]
        comandi: Option<PathBuf>,
        /// Simula: riporta totali e problemi senza modificare il file.
        #[arg(long)]
        prova: bool,
        /// Prezzario di consultazione per `copia_da_prezzario`, come NOME=FILE.
        #[arg(long = "prezzario", value_name = "NOME=FILE")]
        prezzari: Vec<String>,
        /// Accetta comandi che introducono errori di validazione.
        #[arg(long)]
        permissivo: bool,
    },
    /// Esporta un computo in XPWE o l'elenco prezzi in DCF (sperimentali), secondo
    /// l'estensione del file di uscita.
    Esporta {
        /// File di progetto.
        file: PathBuf,
        /// File da scrivere: `.xpwe` (computo) o `.dcf` (elenco prezzi).
        uscita: PathBuf,
        /// Nome del computo (obbligatorio se ce n'è più di uno).
        #[arg(long)]
        computo: Option<String>,
    },
    /// Produce un report in PDF, XLSX o CSV.
    Report {
        /// File di progetto.
        file: PathBuf,
        /// Tipo di report.
        #[arg(value_enum)]
        tipo: TipoCli,
        /// Numero del SAL (per `sal` e `certificato`).
        #[arg(long)]
        numero: Option<u32>,
        /// Nome del computo (per `computo` e `fabbisogno`).
        #[arg(long)]
        computo: Option<String>,
        /// Codice della voce (per `analisi-prezzi`; assente: tutte).
        #[arg(long)]
        voce: Option<String>,
        /// Quadro sintetico (per `analisi-prezzi`).
        #[arg(long)]
        sintetica: bool,
        /// Con l'analisi di ogni voce (per `computo`).
        #[arg(long)]
        con_analisi: bool,
        /// Numero del verbale (per `nuovi-prezzi`; assente: elenco).
        #[arg(long)]
        verbale: Option<u32>,
        /// Incidenza e quota della manodopera di ogni voce (per `computo`).
        #[arg(long)]
        manodopera: bool,
        /// Incidenza e quota della sicurezza compresa nel prezzo (per `computo`).
        #[arg(long)]
        sicurezza: bool,
        /// Subtotali di manodopera e sicurezza nel riepilogo finale (per `computo`).
        #[arg(long)]
        subtotali: bool,
        /// Diagramma di Gantt in PDF (per `cronoprogramma`).
        #[arg(long)]
        gantt: bool,
        /// Modello tradizionale, solo PDF (tutti i documenti).
        #[arg(long)]
        tradizionale: bool,
        /// Computo metrico senza prezzi (modello tradizionale).
        #[arg(long, requires = "tradizionale")]
        senza_prezzi: bool,
        /// Senza frontespizio (modello tradizionale).
        #[arg(long, requires = "tradizionale")]
        senza_frontespizio: bool,
        /// Descrizioni complete anche nel computo (modello tradizionale).
        #[arg(long, requires = "tradizionale")]
        descrizioni_complete: bool,
        /// Righe dell'ente nel frontespizio, separate da `\n` (modello tradizionale).
        #[arg(long, requires = "tradizionale")]
        ente: Option<String>,
        /// Righe dello studio tecnico nella testata, separate da `\n` (modello
        /// tradizionale).
        #[arg(long, requires = "tradizionale")]
        studio: Option<String>,
        /// Formato.
        #[arg(long, value_enum, default_value = "pdf")]
        formato: FormatoCli,
        /// Cartella di uscita.
        #[arg(long, default_value = ".")]
        uscita: PathBuf,
        /// Data di stampa a piè di pagina (AAAA-MM-GG).
        #[arg(long)]
        data: Option<NaiveDate>,
    },
    /// Regole ed esempi dei comandi JSON, per agenti e script.
    SchemaComandi,
    /// Assistente AI (OpenAI per gli embedding, Anthropic per la generazione): i dati
    /// inviati escono dal computer; serve il consenso (`assistente consenso`).
    Assistente {
        #[command(subcommand)]
        azione: AzioneAssistente,
    },
    /// Servizio con stato: una richiesta JSON per riga su stdin, una risposta per riga.
    Servizio {
        /// File di progetto da aprire subito.
        file: Option<PathBuf>,
        /// Accetta comandi che introducono errori di validazione.
        #[arg(long)]
        permissivo: bool,
        /// Scrive su stdout anche righe di avanzamento delle operazioni lunghe
        /// dell'assistente (oggetti con il campo `avanzamento`, senza `ok`), prima della
        /// risposta.
        #[arg(long)]
        avanzamento: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AzioneAssistente {
    /// Configurazione, chiavi presenti, consenso e testo del consenso.
    Stato,
    /// Modifica la configurazione (i valori non indicati restano invariati).
    Configura {
        /// Modello di embedding OpenAI.
        #[arg(long)]
        modello_embedding: Option<String>,
        /// Dimensioni dei vettori (0: quelle del modello).
        #[arg(long)]
        dimensioni: Option<u32>,
        /// Modello di generazione Anthropic.
        #[arg(long)]
        modello_generazione: Option<String>,
        /// Modalità dell'output strutturato.
        #[arg(long, value_enum)]
        modalita: Option<ModalitaCli>,
        /// Limite di token della risposta di analisi.
        #[arg(long)]
        max_token_analisi: Option<u32>,
        /// Limite di token della risposta di selezione.
        #[arg(long)]
        max_token_selezione: Option<u32>,
    },
    /// Salva nel portachiavi del sistema la chiave letta da stdin (una riga); mai come
    /// argomento, che resterebbe nella cronologia della shell.
    Chiave {
        /// Provider.
        #[arg(value_enum)]
        provider: ProviderCli,
        /// Elimina la chiave invece di salvarla.
        #[arg(long)]
        elimina: bool,
    },
    /// Senza opzioni mostra il testo del consenso; `--accetto VERSIONE` lo accetta,
    /// `--revoca` lo revoca.
    Consenso {
        /// Versione del testo letto e accettato.
        #[arg(long, conflicts_with = "revoca")]
        accetto: Option<u32>,
        /// Revoca il consenso.
        #[arg(long)]
        revoca: bool,
    },
    /// Calcola gli embedding delle voci di un prezzario (invia le descrizioni a OpenAI);
    /// riprende un'indicizzazione interrotta. Avanzamento su stderr.
    Indicizza {
        /// Prezzario (DCF, XPWE, .ocp, listino CSV o JSON).
        prezzario: PathBuf,
    },
    /// Cerca nel prezzario: ibrida se è indicizzato, altrimenti solo testuale.
    Cerca {
        /// Prezzario.
        prezzario: PathBuf,
        /// Testo da cercare.
        #[arg(required = true)]
        testo: Vec<String>,
        /// Voci restituite al massimo.
        #[arg(long, default_value_t = opencomputo_agent_api::servizio::LIMITE_RICERCA)]
        limite: usize,
        /// Grandezza attesa (`m`, `m2`, `m3`, `kg`, …).
        #[arg(long)]
        unita: Option<String>,
    },
    /// Propone voci di computo dalla richiesta, con le opzioni per ogni lavorazione, e
    /// prova quelle preselezionate; con `--applica` le aggiunge e salva il file se la
    /// prova è riuscita.
    Proponi {
        /// File di progetto.
        file: PathBuf,
        /// Prezzario da cui scegliere le voci.
        #[arg(long)]
        prezzario: PathBuf,
        /// Computo di destinazione (obbligatorio se ce n'è più di uno).
        #[arg(long)]
        computo: Option<String>,
        /// Esegue la proposta e salva il progetto.
        #[arg(long)]
        applica: bool,
        /// Richiesta in linguaggio naturale.
        #[arg(required = true)]
        testo: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProviderCli {
    Openai,
    Anthropic,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModalitaCli {
    Automatica,
    Json,
    StrumentoForzato,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TipoCli {
    Computo,
    ElencoPrezzi,
    Libretto,
    Registro,
    Sal,
    Certificato,
    Sommario,
    ContoFinale,
    AnalisiPrezzi,
    Fabbisogno,
    NuoviPrezzi,
    Cronoprogramma,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatoCli {
    Pdf,
    Xlsx,
    Csv,
}

fn richiesta_errata(messaggio: impl Into<String>) -> ErroreApplicazione {
    ErroreApplicazione::Richiesta {
        messaggio: messaggio.into(),
    }
}

fn politica(permissivo: bool) -> Politica {
    Politica {
        rifiuta_nuovi_errori: !permissivo,
        ..Politica::default()
    }
}

fn apri(file: &Path, permissivo: bool) -> Result<Servizio, ErroreApplicazione> {
    let mut s = Servizio::new().con_politica(politica(permissivo));
    s.gestisci(Richiesta::Apri {
        percorso: file.to_path_buf(),
    })?;
    Ok(s)
}

fn non_sovrascrivere(file: &Path, forza: bool) -> Result<(), ErroreApplicazione> {
    if !forza && file.exists() {
        return Err(ErroreApplicazione::File {
            messaggio: format!("{}: il file esiste già (usare --forza)", file.display()),
        });
    }
    Ok(())
}

/// Comandi da file o da stdin.
fn leggi_comandi(origine: Option<&Path>) -> Result<Vec<Comando>, ErroreApplicazione> {
    let testo = match origine {
        Some(p) if p != Path::new("-") => String::from_utf8(leggi(p)?)
            .map_err(|_| richiesta_errata(format!("{}: non è testo UTF-8", p.display())))?,
        _ => {
            let mut t = String::new();
            io::stdin()
                .read_to_string(&mut t)
                .map_err(|e| ErroreApplicazione::File {
                    messaggio: format!("stdin: {e}"),
                })?;
            t
        }
    };
    let v: Value = serde_json::from_str(testo.trim_start_matches('\u{feff}'))
        .map_err(|e| richiesta_errata(format!("JSON non valido: {e}")))?;
    let lista = match v {
        Value::Array(_) => v,
        Value::Object(mut m) if m.contains_key("comandi") => {
            m.remove("comandi").unwrap_or_default()
        }
        altro => Value::Array(vec![altro]),
    };
    serde_json::from_value(lista).map_err(|e| richiesta_errata(format!("comandi non validi: {e}")))
}

fn esegui(cli: Sotto) -> Result<(Value, u8), ErroreApplicazione> {
    let ok = |v: Value| Ok((v, uscita::OK));
    match cli {
        Sotto::Nuovo {
            file,
            titolo,
            forza,
            gzip,
        } => {
            non_sovrascrivere(&file, forza)?;
            let mut s = Servizio::new();
            s.gestisci(Richiesta::Nuovo {
                titolo,
                percorso: Some(file),
            })?;
            ok(s.gestisci(Richiesta::Salva {
                percorso: None,
                gzip: Some(gzip),
            })?)
        }
        Sotto::Importa {
            origine,
            file,
            forza,
            gzip,
        } => {
            non_sovrascrivere(&file, forza)?;
            let mut s = Servizio::new();
            let mut r = s.gestisci(Richiesta::Importa { origine })?;
            let salvato = s.gestisci(Richiesta::Salva {
                percorso: Some(file),
                gzip: Some(gzip),
            })?;
            if let Value::Object(m) = &mut r {
                m.insert("salvataggio".into(), salvato);
            }
            ok(r)
        }
        Sotto::Info { file } => ok(apri(&file, false)?.gestisci(Richiesta::Info {})?),
        Sotto::Valida { file } => {
            let v = apri(&file, false)?.gestisci(Richiesta::Valida {})?;
            let codice = if v.get("valido") == Some(&Value::Bool(true)) {
                uscita::OK
            } else {
                uscita::RIFIUTATA
            };
            Ok((v, codice))
        }
        Sotto::Cerca {
            file,
            testo,
            prezzario,
            limite,
        } => {
            let mut s = apri(&file, false)?;
            let nome = match prezzario {
                Some(percorso) => {
                    s.gestisci(Richiesta::CaricaPrezzario {
                        nome: "consultazione".into(),
                        percorso,
                    })?;
                    Some("consultazione".to_owned())
                }
                None => None,
            };
            ok(s.gestisci(Richiesta::Cerca {
                testo: testo.join(" "),
                prezzario: nome,
                limite: Some(limite),
            })?)
        }
        Sotto::Esegui {
            file,
            comandi,
            prova,
            prezzari,
            permissivo,
        } => {
            let comandi = leggi_comandi(comandi.as_deref())?;
            let mut s = apri(&file, permissivo)?;
            for voce in prezzari {
                let (nome, percorso) = voce.split_once('=').ok_or_else(|| {
                    richiesta_errata(format!("--prezzario «{voce}»: usare NOME=FILE"))
                })?;
                s.gestisci(Richiesta::CaricaPrezzario {
                    nome: nome.to_owned(),
                    percorso: PathBuf::from(percorso),
                })?;
            }
            if prova {
                return ok(s.gestisci(Richiesta::Prova { comandi })?);
            }
            let mut esito = s.gestisci(Richiesta::Esegui { comandi })?;
            let salvato = s.gestisci(Richiesta::Salva {
                percorso: None,
                gzip: None,
            })?;
            if let Value::Object(m) = &mut esito {
                m.insert("salvataggio".into(), salvato);
            }
            ok(esito)
        }
        Sotto::Esporta {
            file,
            uscita: percorso,
            computo,
        } => {
            let dcf = percorso
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dcf"));
            if dcf {
                if computo.is_some() {
                    return Err(richiesta_errata(
                        "l'export DCF riguarda l'elenco prezzi: --computo non si usa",
                    ));
                }
                ok(apri(&file, false)?.gestisci(Richiesta::EsportaDcf {
                    percorso,
                    data: None,
                })?)
            } else {
                ok(apri(&file, false)?.gestisci(Richiesta::EsportaXpwe { percorso, computo })?)
            }
        }
        Sotto::Report {
            file,
            tipo,
            numero,
            computo,
            voce,
            sintetica,
            con_analisi,
            verbale,
            manodopera,
            sicurezza,
            subtotali,
            gantt,
            tradizionale,
            senza_prezzi,
            senza_frontespizio,
            descrizioni_complete,
            ente,
            studio,
            formato,
            uscita: cartella,
            data,
        } => {
            let a_capo = |t: Option<String>| t.map(|t| t.replace("\\n", "\n"));
            let stampa = tradizionale.then(|| StampaTradizionale {
                ente: a_capo(ente),
                studio: a_capo(studio),
                frontespizio: !senza_frontespizio,
                senza_prezzi,
                descrizioni_ridotte: !descrizioni_complete,
            });
            if stampa.as_ref().is_some_and(|s| s.senza_prezzi) && !matches!(tipo, TipoCli::Computo)
            {
                return Err(richiesta_errata("--senza-prezzi vale solo per computo"));
            }
            let incidenze = IncidenzeReport {
                manodopera,
                sicurezza,
                riepilogo: subtotali,
            };
            if !incidenze.nessuna() && !matches!(tipo, TipoCli::Computo) {
                return Err(richiesta_errata(
                    "--manodopera, --sicurezza e --subtotali valgono solo per computo",
                ));
            }
            let report = match tipo {
                TipoCli::Computo => TipoReport::Computo {
                    computo,
                    con_analisi,
                    tradizionale: stampa,
                    incidenze,
                },
                TipoCli::ElencoPrezzi => TipoReport::ElencoPrezzi {
                    tradizionale: stampa,
                },
                TipoCli::Libretto => TipoReport::Libretto {
                    tradizionale: stampa,
                },
                TipoCli::Registro => TipoReport::Registro {
                    tradizionale: stampa,
                },
                TipoCli::Sal => TipoReport::Sal {
                    numero: numero.ok_or_else(|| {
                        richiesta_errata("indicare --numero per il report del SAL")
                    })?,
                    tradizionale: stampa,
                },
                TipoCli::Certificato => TipoReport::Certificato {
                    numero: numero.ok_or_else(|| {
                        richiesta_errata("indicare --numero del SAL per il certificato")
                    })?,
                    tradizionale: stampa,
                },
                TipoCli::Sommario => TipoReport::Sommario {
                    tradizionale: stampa,
                },
                TipoCli::ContoFinale => TipoReport::ContoFinale {
                    tradizionale: stampa,
                },
                TipoCli::AnalisiPrezzi => TipoReport::AnalisiPrezzi {
                    voce,
                    sintetica,
                    tradizionale: stampa,
                },
                TipoCli::Fabbisogno => TipoReport::Fabbisogno {
                    computo,
                    tradizionale: stampa,
                },
                TipoCli::NuoviPrezzi => TipoReport::NuoviPrezzi {
                    verbale,
                    tradizionale: stampa,
                },
                TipoCli::Cronoprogramma => TipoReport::Cronoprogramma {
                    gantt,
                    tradizionale: stampa,
                },
            };
            if gantt && !matches!(report, TipoReport::Cronoprogramma { .. }) {
                return Err(richiesta_errata("--gantt vale solo per cronoprogramma"));
            }
            let formato = match formato {
                FormatoCli::Pdf => FormatoReport::Pdf,
                FormatoCli::Xlsx => FormatoReport::Xlsx,
                FormatoCli::Csv => FormatoReport::Csv,
            };
            ok(apri(&file, false)?.gestisci(Richiesta::Report {
                report,
                formato: Some(formato),
                cartella: Some(cartella),
                file: None,
                data_stampa: data,
            })?)
        }
        Sotto::SchemaComandi => ok(Servizio::new().gestisci(Richiesta::SchemaComandi {})?),
        Sotto::Assistente { azione } => assistente(azione),
        Sotto::Servizio { .. } => Err(richiesta_errata("servizio gestito a parte")),
    }
}

/// Nome con cui il prezzario indicato è caricato nella sessione.
const PREZZARIO_CLI: &str = "prezzario";

/// Servizio con un progetto vuoto in memoria e il prezzario caricato.
fn con_prezzario(mut s: Servizio, prezzario: PathBuf) -> Result<Servizio, ErroreApplicazione> {
    if s.sessione().is_none() {
        s.gestisci(Richiesta::Nuovo {
            titolo: "assistente".into(),
            percorso: None,
        })?;
    }
    s.gestisci(Richiesta::CaricaPrezzario {
        nome: PREZZARIO_CLI.into(),
        percorso: prezzario,
    })?;
    Ok(s)
}

fn leggi_chiave() -> Result<String, ErroreApplicazione> {
    let mut riga = String::new();
    io::stdin()
        .read_line(&mut riga)
        .map_err(|e| ErroreApplicazione::File {
            messaggio: format!("stdin: {e}"),
        })?;
    let chiave = riga.trim().to_owned();
    if chiave.is_empty() {
        return Err(richiesta_errata(
            "nessuna chiave su stdin (per eliminarla usare --elimina)",
        ));
    }
    Ok(chiave)
}

fn assistente(azione: AzioneAssistente) -> Result<(Value, u8), ErroreApplicazione> {
    use opencomputo_agent_api::assistente::{ChiaveApi, Modifiche};
    use opencomputo_assistente::provider::Provider;
    use opencomputo_assistente::provider::anthropic::ModalitaOutput;
    let ok = |v: Value| Ok((v, uscita::OK));
    let mut s = Servizio::new();
    match azione {
        AzioneAssistente::Stato => ok(s.gestisci(Richiesta::AssistenteStato {})?),
        AzioneAssistente::Configura {
            modello_embedding,
            dimensioni,
            modello_generazione,
            modalita,
            max_token_analisi,
            max_token_selezione,
        } => ok(s.gestisci(Richiesta::AssistenteConfigura {
            configurazione: Modifiche {
                modello_embedding,
                dimensioni_embedding: dimensioni,
                modello_generazione,
                modalita_output: modalita.map(|m| match m {
                    ModalitaCli::Automatica => ModalitaOutput::Automatica,
                    ModalitaCli::Json => ModalitaOutput::Json,
                    ModalitaCli::StrumentoForzato => ModalitaOutput::StrumentoForzato,
                }),
                max_token_analisi,
                max_token_selezione,
            },
        })?),
        AzioneAssistente::Chiave { provider, elimina } => {
            let chiave = if elimina {
                None
            } else {
                Some(ChiaveApi::from(leggi_chiave()?))
            };
            ok(s.gestisci(Richiesta::AssistenteChiave {
                provider: match provider {
                    ProviderCli::Openai => Provider::Openai,
                    ProviderCli::Anthropic => Provider::Anthropic,
                },
                chiave,
            })?)
        }
        AzioneAssistente::Consenso { accetto, revoca } => {
            if revoca {
                ok(s.gestisci(Richiesta::AssistenteConsenso {
                    accetto: false,
                    versione: None,
                })?)
            } else if let Some(versione) = accetto {
                ok(s.gestisci(Richiesta::AssistenteConsenso {
                    accetto: true,
                    versione: Some(versione),
                })?)
            } else {
                let stato = s.gestisci(Richiesta::AssistenteStato {})?;
                ok(stato.get("consenso").cloned().unwrap_or(Value::Null))
            }
        }
        AzioneAssistente::Indicizza { prezzario } => {
            let s = Servizio::new().con_avanzamento(Box::new(|v: &Value| {
                eprintln!("{v}");
            }));
            let mut s = con_prezzario(s, prezzario)?;
            ok(s.gestisci(Richiesta::AssistenteIndicizza {
                prezzario: PREZZARIO_CLI.into(),
            })?)
        }
        AzioneAssistente::Cerca {
            prezzario,
            testo,
            limite,
            unita,
        } => {
            let mut s = con_prezzario(s, prezzario)?;
            ok(s.gestisci(Richiesta::AssistenteCerca {
                prezzario: PREZZARIO_CLI.into(),
                testo: testo.join(" "),
                limite: Some(limite),
                unita,
            })?)
        }
        AzioneAssistente::Proponi {
            file,
            prezzario,
            computo,
            applica,
            testo,
        } => {
            s.gestisci(Richiesta::Apri { percorso: file })?;
            let mut s = con_prezzario(s, prezzario)?;
            let mut proposta = s.gestisci(Richiesta::AssistenteProponi {
                prezzario: PREZZARIO_CLI.into(),
                testo: testo.join(" "),
                computo,
            })?;
            // Applicabile: le scelte iniziali passano la prova.
            let provata = proposta
                .pointer("/anteprima/esito")
                .is_some_and(|e| !e.is_null());
            if !applica {
                return ok(proposta);
            }
            if !provata {
                return Ok((proposta, uscita::RIFIUTATA));
            }
            let esito = s.gestisci(Richiesta::AssistenteApplica { scelte: None })?;
            let salvato = s.gestisci(Richiesta::Salva {
                percorso: None,
                gzip: None,
            })?;
            if let Value::Object(m) = &mut proposta {
                m.insert("applicata".into(), esito);
                m.insert("salvataggio".into(), salvato);
            }
            ok(proposta)
        }
    }
}

fn stampa(v: &Value, compatto: bool) {
    let testo = if compatto {
        serde_json::to_string(v)
    } else {
        serde_json::to_string_pretty(v)
    };
    // Un flusso chiuso in anticipo (es. `| head`) non è un errore del programma.
    let mut uscita = io::stdout().lock();
    let _ = writeln!(uscita, "{}", testo.unwrap_or_default()).and_then(|()| uscita.flush());
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(if e.use_stderr() {
                uscita::USO
            } else {
                uscita::OK
            });
        }
    };
    if let Sotto::Servizio {
        file,
        permissivo,
        avanzamento,
    } = cli.comando
    {
        let mut s = Servizio::new().con_politica(politica(permissivo));
        if avanzamento {
            s = s.con_avanzamento(Box::new(|v: &Value| {
                let mut uscita = io::stdout().lock();
                let _ = writeln!(uscita, "{v}").and_then(|()| uscita.flush());
            }));
        }
        if let Some(percorso) = file {
            let r = s.gestisci(Richiesta::Apri { percorso });
            let fallito = r.is_err();
            stampa(&risposta(None, r), true);
            if fallito {
                return ExitCode::from(uscita::FILE);
            }
        }
        return match s.servi(io::stdin().lock(), io::stdout().lock()) {
            Ok(()) => ExitCode::from(uscita::OK),
            Err(e) => {
                eprintln!("opencomputo servizio: {e}");
                ExitCode::from(uscita::FILE)
            }
        };
    }
    let compatto = cli.compatto;
    match esegui(cli.comando) {
        Ok((v, codice)) => {
            stampa(&risposta(None, Ok(v)), compatto);
            ExitCode::from(codice)
        }
        Err(e) => {
            let codice = uscita::per_errore(&e);
            stampa(&risposta(None, Err(e)), compatto);
            ExitCode::from(codice)
        }
    }
}
