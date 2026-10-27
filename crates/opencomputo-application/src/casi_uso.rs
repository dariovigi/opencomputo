//! Casi d'uso che coinvolgono i formati esterni: import XPWE, DCF e listini CSV/JSON, export XPWE,
//! report. Restituiscono byte e rapporti: la scrittura su disco spetta al chiamante.

use opencomputo_core::id::{ComputoId, GeneratoreSequenziale};
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::provenienza::Anomalia;
use opencomputo_io::documento::Documento;
use opencomputo_io::report::{self, OpzioniReport};
use opencomputo_io::tradizionale::{self, OpzioniTradizionale};
use opencomputo_io::{csv, pdf, report_analisi, xlsx};
use serde::{Deserialize, Serialize};

use crate::errore::ErroreApplicazione;
use crate::sessione::Sessione;

/// Rapporto sintetico di un'importazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RapportoImportazione {
    /// Formato di origine.
    pub formato: String,
    /// Voci di elenco prezzi importate.
    pub voci_elenco: usize,
    /// Voci di computo importate.
    pub voci_computo: usize,
    /// Anomalie (le gravità «errore» indicano dati scartati).
    pub anomalie: Vec<Anomalia>,
}

fn errore_import(e: impl std::fmt::Display) -> ErroreApplicazione {
    ErroreApplicazione::Importazione {
        messaggio: e.to_string(),
    }
}

/// Nuova sessione da un file XPWE.
///
/// # Errors
/// File non leggibile come XPWE.
pub fn importa_xpwe(
    byte: &[u8],
    nome_file: Option<&str>,
) -> Result<(Sessione, RapportoImportazione), ErroreApplicazione> {
    let r = opencomputo_format_xpwe::importa(
        byte,
        &mut GeneratoreSequenziale::default(),
        &opencomputo_format_xpwe::OpzioniImport {
            nome_file: nome_file.map(str::to_owned),
            ..Default::default()
        },
    )
    .map_err(errore_import)?;
    let rapporto = RapportoImportazione {
        formato: "xpwe".into(),
        voci_elenco: r.rapporto.statistiche.voci_elenco,
        voci_computo: r.rapporto.statistiche.voci_computo,
        anomalie: r.rapporto.anomalie,
    };
    Ok((Sessione::new(r.progetto), rapporto))
}

/// Progetto con l'elenco prezzi di un DCF non protetto.
fn progetto_da_dcf(
    byte: &[u8],
    nome_file: Option<&str>,
) -> Result<(Progetto, RapportoImportazione), ErroreApplicazione> {
    let r = opencomputo_dcf_parser::importa(
        byte,
        &mut GeneratoreSequenziale::default(),
        &opencomputo_dcf_parser::OpzioniImport {
            nome_file: nome_file.map(str::to_owned),
            ..Default::default()
        },
    )
    .map_err(errore_import)?;
    let rapporto = RapportoImportazione {
        formato: "dcf".into(),
        voci_elenco: r.rapporto.statistiche.voci_elenco,
        voci_computo: 0,
        anomalie: r.rapporto.anomalie,
    };
    Ok((r.progetto, rapporto))
}

/// Nuova sessione con l'elenco prezzi di un DCF non protetto.
///
/// # Errors
/// File non DCF, protetto o senza dati importabili.
pub fn importa_dcf(
    byte: &[u8],
    nome_file: Option<&str>,
) -> Result<(Sessione, RapportoImportazione), ErroreApplicazione> {
    let (p, r) = progetto_da_dcf(byte, nome_file)?;
    Ok((Sessione::new(p), r))
}

/// `true` se il primo carattere significativo (dopo BOM e spazi) è `<`.
fn sembra_xml(byte: &[u8]) -> bool {
    let senza_bom = byte.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(byte);
    senza_bom.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<')
}

/// Prezzario da un listino CSV o JSON (ADR 0025).
///
/// # Errors
/// Listino vuoto, colonne obbligatorie mancanti, nessuna voce valida.
pub fn leggi_listino(
    byte: &[u8],
    nome_file: Option<&str>,
) -> Result<(Prezzario, RapportoImportazione), ErroreApplicazione> {
    let r = opencomputo_format_listino::importa(
        byte,
        &mut GeneratoreSequenziale::default(),
        &opencomputo_format_listino::OpzioniListino {
            nome_file: nome_file.map(str::to_owned),
            ..Default::default()
        },
    )
    .map_err(errore_import)?;
    let formato = match r.rapporto.formato {
        opencomputo_format_listino::FormatoListino::Csv => "csv",
        opencomputo_format_listino::FormatoListino::Json => "json",
    };
    let rapporto = RapportoImportazione {
        formato: formato.into(),
        voci_elenco: r.rapporto.voci_importate,
        voci_computo: 0,
        anomalie: r.rapporto.anomalie,
    };
    Ok((r.prezzario, rapporto))
}

/// Prezzario di consultazione da un file DCF (solo contenuti non protetti), XPWE, di
/// progetto o da un listino CSV o JSON, riconosciuto dal contenuto: DCF dalla firma,
/// poi file di progetto, poi XPWE se il testo inizia con `<`, altrimenti listino.
///
/// # Errors
/// Formato non riconosciuto o file non leggibile.
pub fn leggi_prezzario(
    byte: &[u8],
    nome_file: Option<&str>,
) -> Result<(Prezzario, RapportoImportazione), ErroreApplicazione> {
    if byte.starts_with(b"AAMVHFSS") {
        let (p, r) = progetto_da_dcf(byte, nome_file)?;
        return Ok((p.elenco_prezzi, r));
    }
    if let Ok(p) = opencomputo_storage::decodifica(byte) {
        let r = RapportoImportazione {
            formato: "opencomputo".into(),
            voci_elenco: p.elenco_prezzi.voci.len(),
            voci_computo: 0,
            anomalie: Vec::new(),
        };
        return Ok((p.elenco_prezzi, r));
    }
    if !sembra_xml(byte) {
        return leggi_listino(byte, nome_file);
    }
    let (s, r) = importa_xpwe(byte, nome_file)?;
    Ok((s.progetto().elenco_prezzi.clone(), r))
}

/// Computo per nome; senza nome, l'unico computo del progetto.
///
/// # Errors
/// Computo inesistente o ambiguo.
pub fn trova_computo(p: &Progetto, nome: Option<&str>) -> Result<ComputoId, ErroreApplicazione> {
    let richiesta = |messaggio: String| ErroreApplicazione::Richiesta { messaggio };
    match nome {
        Some(n) => p
            .computi
            .iter()
            .find(|c| c.nome == n)
            .map(|c| c.id)
            .ok_or_else(|| richiesta(format!("computo «{n}» inesistente"))),
        None => match p.computi.as_slice() {
            [c] => Ok(c.id),
            [] => Err(richiesta("il progetto non ha computi".into())),
            molti => Err(richiesta(format!(
                "il progetto ha {} computi: indicarne il nome",
                molti.len()
            ))),
        },
    }
}

/// Esporta un computo in XPWE (sperimentale, vedi ADR 0006).
///
/// # Errors
/// Computo inesistente o non calcolabile.
pub fn esporta_xpwe(
    p: &Progetto,
    computo: Option<&str>,
) -> Result<opencomputo_format_xpwe::RisultatoExport, ErroreApplicazione> {
    let id = trova_computo(p, computo)?;
    opencomputo_format_xpwe::esporta(p, id).map_err(|e| ErroreApplicazione::Esportazione {
        messaggio: e.to_string(),
    })
}

fn vero() -> bool {
    true
}

/// Esporta l'elenco prezzi in DCF (sperimentale, ADR 0016). Le date del contenitore
/// sono quella del progetto o, se manca, quella indicata.
///
/// # Errors
/// Elenco prezzi vuoto o oltre i limiti del contenitore.
pub fn esporta_dcf(
    p: &Progetto,
    data: Option<chrono::NaiveDate>,
) -> Result<opencomputo_dcf_parser::esporta::RisultatoExport, ErroreApplicazione> {
    let giorno = p
        .metadati
        .data
        .or(data)
        .or_else(|| chrono::NaiveDate::from_ymd_opt(2000, 1, 1))
        .unwrap_or_default();
    let opzioni = opencomputo_dcf_parser::esporta::OpzioniExport {
        data: giorno.and_time(chrono::NaiveTime::MIN),
        versione_programma: env!("CARGO_PKG_VERSION").to_owned(),
    };
    opencomputo_dcf_parser::esporta::esporta(p, &opzioni).map_err(|e| {
        ErroreApplicazione::Esportazione {
            messaggio: e.to_string(),
        }
    })
}

/// Stampa nel modello tradizionale (ADR 0015): impaginazione dei computi in uso negli
/// studi tecnici, solo PDF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StampaTradizionale {
    /// Righe del riquadro in alto del frontespizio (es. «Comune di …»), separate da a
    /// capo; assente: il luogo dei lavori.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ente: Option<String>,
    /// Righe dello studio tecnico, in testa a ogni pagina e in basso nel frontespizio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub studio: Option<String>,
    /// Con il frontespizio.
    #[serde(default = "vero")]
    pub frontespizio: bool,
    /// Computo metrico senza prezzi né importi (solo per il computo).
    #[serde(default)]
    pub senza_prezzi: bool,
    /// Descrizioni lunghe ridotte nel computo (inizio « ... » fine).
    #[serde(default = "vero")]
    pub descrizioni_ridotte: bool,
}

impl Default for StampaTradizionale {
    fn default() -> Self {
        Self {
            ente: None,
            studio: None,
            frontespizio: true,
            senza_prezzi: false,
            descrizioni_ridotte: true,
        }
    }
}

impl StampaTradizionale {
    fn opzioni(&self, o: &OpzioniReport) -> OpzioniTradizionale {
        let righe = |t: &Option<String>| -> Vec<String> {
            t.as_deref()
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_owned)
                .collect()
        };
        OpzioniTradizionale {
            data_stampa: o.data_stampa,
            ente: righe(&self.ente),
            studio: righe(&self.studio),
            frontespizio: self.frontespizio,
            senza_prezzi: self.senza_prezzi,
            descrizioni_ridotte: self.descrizioni_ridotte,
            comprimi: true,
            incidenze: report::Incidenze::default(),
            firma: None,
        }
    }
}

/// Incidenze da stampare nel computo (ADR 0021).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidenzeReport {
    /// Incidenza e quota della manodopera di ogni voce.
    #[serde(default)]
    pub manodopera: bool,
    /// Incidenza e quota della sicurezza compresa nel prezzo di ogni voce.
    #[serde(default)]
    pub sicurezza: bool,
    /// Subtotali delle quote scelte nel riepilogo finale (nel modello tradizionale,
    /// pagine di riepilogo per categorie e WBS).
    #[serde(default)]
    pub riepilogo: bool,
}

impl IncidenzeReport {
    /// `true` se non è richiesto nulla.
    #[must_use]
    pub fn nessuna(&self) -> bool {
        self.in_report().nessuna()
    }

    fn in_report(self) -> report::Incidenze {
        report::Incidenze {
            manodopera: self.manodopera,
            sicurezza: self.sicurezza,
            riepilogo: self.riepilogo,
        }
    }
}

/// Tipo di report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum TipoReport {
    /// Computo metrico estimativo.
    Computo {
        /// Nome del computo.
        #[serde(default)]
        computo: Option<String>,
        /// Con l'analisi di ogni voce riportata alla quantità computata.
        #[serde(default)]
        con_analisi: bool,
        /// Stampa nel modello tradizionale (solo PDF).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
        /// Incidenze di manodopera e sicurezza e loro subtotali nel riepilogo.
        #[serde(default, skip_serializing_if = "IncidenzeReport::nessuna")]
        incidenze: IncidenzeReport,
    },
    /// Elenco prezzi.
    ElencoPrezzi {
        /// Stampa nel modello tradizionale (solo PDF).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Libretto delle misure.
    Libretto {
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Registro di contabilità.
    Registro {
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Stato di avanzamento lavori.
    Sal {
        /// Numero del SAL.
        numero: u32,
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Certificato di pagamento di un SAL.
    Certificato {
        /// Numero del SAL.
        numero: u32,
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Sommario del registro di contabilità.
    Sommario {
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Conto finale e rata di saldo.
    ContoFinale {
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Analisi dei prezzi: tutte le voci analizzate o una sola, completa o sintetica.
    AnalisiPrezzi {
        /// Codice della voce (assente: tutte).
        #[serde(default)]
        voce: Option<String>,
        /// Quadro sintetico (una riga per voce).
        #[serde(default)]
        sintetica: bool,
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Fabbisogno di cantiere di un computo.
    Fabbisogno {
        /// Nome del computo (facoltativo se ce n'è uno solo).
        #[serde(default)]
        computo: Option<String>,
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Cronoprogramma dei lavori: attività e produzione mensile, oppure il diagramma di
    /// Gantt (solo PDF).
    Cronoprogramma {
        /// Diagramma di Gantt (PDF A3 orizzontale) invece della tabella.
        #[serde(default)]
        gantt: bool,
        /// Stampa nel modello tradizionale (solo PDF, tabella).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
    /// Elenco dei nuovi prezzi o verbale di concordamento.
    NuoviPrezzi {
        /// Numero del verbale (assente: elenco).
        #[serde(default)]
        verbale: Option<u32>,
        /// Stampa nel modello tradizionale (solo PDF, ADR 0023).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tradizionale: Option<StampaTradizionale>,
    },
}

/// Formato di uscita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatoReport {
    /// PDF.
    Pdf,
    /// Excel.
    Xlsx,
    /// CSV (un file per sezione).
    Csv,
}

/// File prodotto da un report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    /// Nome suggerito (senza cartella).
    pub nome: String,
    /// Contenuto.
    pub byte: Vec<u8>,
}

fn errore_export(e: impl std::fmt::Display) -> ErroreApplicazione {
    ErroreApplicazione::Esportazione {
        messaggio: e.to_string(),
    }
}

fn sal_per_numero(
    p: &Progetto,
    numero: u32,
) -> Result<opencomputo_core::id::SalId, ErroreApplicazione> {
    p.contabilita
        .sal()
        .iter()
        .find(|s| s.numero == numero)
        .map(|s| s.id)
        .ok_or_else(|| ErroreApplicazione::Richiesta {
            messaggio: format!("SAL n. {numero} inesistente"),
        })
}

/// Costruisce il documento di un report. Per il modello tradizionale è il documento
/// ordinario con gli stessi dati (serve all'anteprima): la stampa tradizionale si
/// ottiene con [`report`].
///
/// # Errors
/// Riferimenti inesistenti o dati non calcolabili.
pub fn documento(
    p: &Progetto,
    tipo: &TipoReport,
    o: &OpzioniReport,
) -> Result<(String, Documento), ErroreApplicazione> {
    Ok(match tipo {
        TipoReport::Computo {
            computo,
            con_analisi: false,
            incidenze,
            tradizionale,
        } => {
            controlla_incidenze(*incidenze, tradizionale.as_ref())?;
            let id = trova_computo(p, computo.as_deref())?;
            (
                "computo".into(),
                report::computo_con(p, id, incidenze.in_report(), o).map_err(errore_export)?,
            )
        }
        TipoReport::Computo {
            computo,
            con_analisi: true,
            incidenze,
            ..
        } => {
            if !incidenze.nessuna() {
                return Err(ErroreApplicazione::Richiesta {
                    messaggio: "il computo con le analisi non stampa le incidenze".into(),
                });
            }
            let id = trova_computo(p, computo.as_deref())?;
            (
                "computo_con_analisi".into(),
                report_analisi::computo_con_analisi(p, id, o).map_err(errore_export)?,
            )
        }
        TipoReport::ElencoPrezzi { .. } => ("elenco_prezzi".into(), report::elenco_prezzi(p, o)),
        TipoReport::Libretto { .. } => (
            "libretto".into(),
            report::libretto(p, o).map_err(errore_export)?,
        ),
        TipoReport::Registro { .. } => (
            "registro".into(),
            report::registro(p, o).map_err(errore_export)?,
        ),
        TipoReport::Sal { numero, .. } => {
            let id = sal_per_numero(p, *numero)?;
            (
                format!("sal_{numero}"),
                report::stato_avanzamento(p, id, o).map_err(errore_export)?,
            )
        }
        TipoReport::Certificato { numero, .. } => {
            let id = sal_per_numero(p, *numero)?;
            (
                format!("certificato_{numero}"),
                report::certificato_pagamento(p, id, o).map_err(errore_export)?,
            )
        }
        TipoReport::Sommario { .. } => (
            "sommario_registro".into(),
            report::sommario_registro(p, o).map_err(errore_export)?,
        ),
        TipoReport::ContoFinale { .. } => (
            "conto_finale".into(),
            report::conto_finale(p, o).map_err(errore_export)?,
        ),
        TipoReport::AnalisiPrezzi {
            voce, sintetica, ..
        } => (
            if *sintetica {
                "analisi_prezzi_sintetica".into()
            } else {
                "analisi_prezzi".into()
            },
            report_analisi::analisi_prezzi(p, voce.as_deref(), *sintetica, o)
                .map_err(errore_export)?,
        ),
        TipoReport::Fabbisogno { computo, .. } => {
            let id = trova_computo(p, computo.as_deref())?;
            (
                "fabbisogno".into(),
                report_analisi::fabbisogno(p, id, o).map_err(errore_export)?,
            )
        }
        TipoReport::Cronoprogramma { .. } => (
            "cronoprogramma".into(),
            opencomputo_io::report_cronoprogramma::cronoprogramma(p, o).map_err(errore_export)?,
        ),
        TipoReport::NuoviPrezzi { verbale, .. } => (
            verbale.map_or_else(
                || "nuovi_prezzi".into(),
                |n| format!("verbale_nuovi_prezzi_{n}"),
            ),
            report_analisi::nuovi_prezzi(p, *verbale, o).map_err(errore_export)?,
        ),
    })
}

/// Le incidenze riguardano gli importi: non si stampano nel computo metrico senza
/// prezzi.
fn controlla_incidenze(
    incidenze: IncidenzeReport,
    tradizionale: Option<&StampaTradizionale>,
) -> Result<(), ErroreApplicazione> {
    if !incidenze.nessuna() && tradizionale.is_some_and(|t| t.senza_prezzi) {
        return Err(ErroreApplicazione::Richiesta {
            messaggio: "il computo metrico senza prezzi non stampa le incidenze".into(),
        });
    }
    Ok(())
}

impl TipoReport {
    /// Opzioni del modello tradizionale, se richiesto.
    #[must_use]
    pub fn tradizionale(&self) -> Option<&StampaTradizionale> {
        match self {
            Self::Computo { tradizionale, .. }
            | Self::ElencoPrezzi { tradizionale }
            | Self::Libretto { tradizionale }
            | Self::Registro { tradizionale }
            | Self::Sal { tradizionale, .. }
            | Self::Certificato { tradizionale, .. }
            | Self::Sommario { tradizionale }
            | Self::ContoFinale { tradizionale }
            | Self::AnalisiPrezzi { tradizionale, .. }
            | Self::Fabbisogno { tradizionale, .. }
            | Self::NuoviPrezzi { tradizionale, .. }
            | Self::Cronoprogramma { tradizionale, .. } => tradizionale.as_ref(),
        }
    }
}

/// Chi firma un documento nel modello tradizionale: il direttore dei lavori per i
/// documenti contabili, il responsabile del procedimento per il certificato di
/// pagamento, altrimenti il progettista.
fn firma(p: &Progetto, tipo: &TipoReport) -> Option<tradizionale::Firma> {
    let m = &p.metadati;
    let (ruolo, nome) = match tipo {
        TipoReport::Libretto { .. }
        | TipoReport::Registro { .. }
        | TipoReport::Sal { .. }
        | TipoReport::Sommario { .. }
        | TipoReport::ContoFinale { .. }
        | TipoReport::NuoviPrezzi { .. } => ("Il Direttore dei Lavori", &m.direttore_lavori),
        TipoReport::Certificato { .. } => (
            "Il Responsabile del Procedimento",
            &m.responsabile_procedimento,
        ),
        _ => return None,
    };
    Some(tradizionale::Firma {
        ruolo: ruolo.into(),
        nome: nome.clone(),
    })
}

/// Stampa nel modello tradizionale, se richiesta (solo PDF): il computo e l'elenco
/// prezzi con le loro impaginazioni (ADR 0015), gli altri documenti con la resa
/// tradizionale del documento ordinario (ADR 0023).
fn report_tradizionale(
    p: &Progetto,
    tipo: &TipoReport,
    formato: FormatoReport,
    o: &OpzioniReport,
) -> Result<Option<FileReport>, ErroreApplicazione> {
    let richiesta = |messaggio: &str| ErroreApplicazione::Richiesta {
        messaggio: messaggio.to_owned(),
    };
    let Some(stampa) = tipo.tradizionale() else {
        return Ok(None);
    };
    if formato != FormatoReport::Pdf {
        return Err(richiesta(
            "il modello tradizionale è disponibile solo in PDF",
        ));
    }
    let opzioni = stampa.opzioni(o);
    Ok(Some(match tipo {
        TipoReport::Computo {
            computo,
            con_analisi: false,
            incidenze,
            ..
        } => {
            controlla_incidenze(*incidenze, Some(stampa))?;
            let id = trova_computo(p, computo.as_deref())?;
            let base = if stampa.senza_prezzi {
                "computo_metrico_tradizionale"
            } else {
                "computo_tradizionale"
            };
            let opzioni = OpzioniTradizionale {
                incidenze: incidenze.in_report(),
                ..opzioni
            };
            FileReport {
                nome: format!("{base}.pdf"),
                byte: tradizionale::computo(p, id, &opzioni)
                    .map_err(errore_export)?
                    .byte,
            }
        }
        TipoReport::ElencoPrezzi { .. } => FileReport {
            nome: "elenco_prezzi_tradizionale.pdf".into(),
            byte: tradizionale::elenco_prezzi(p, &opzioni).byte,
        },
        _ => {
            if stampa.senza_prezzi {
                return Err(richiesta(
                    "«senza prezzi» vale solo per il computo nel modello tradizionale",
                ));
            }
            let (base, doc) = documento(p, tipo, o)?;
            let opzioni = OpzioniTradizionale {
                firma: firma(p, tipo),
                ..opzioni
            };
            FileReport {
                nome: format!("{base}_tradizionale.pdf"),
                byte: tradizionale::documento(p, &doc, &doc.titolo, &opzioni).byte,
            }
        }
    }))
}

/// Produce un report nel formato richiesto.
///
/// # Errors
/// Come [`documento`], più gli errori di scrittura XLSX; il modello tradizionale è
/// solo PDF.
pub fn report(
    p: &Progetto,
    tipo: &TipoReport,
    formato: FormatoReport,
    o: &OpzioniReport,
) -> Result<Vec<FileReport>, ErroreApplicazione> {
    if let TipoReport::Cronoprogramma { gantt: true, .. } = tipo {
        if formato != FormatoReport::Pdf {
            return Err(ErroreApplicazione::Richiesta {
                messaggio: "il diagramma di Gantt è disponibile solo in PDF".into(),
            });
        }
        let opzioni = opencomputo_io::report_cronoprogramma::OpzioniGantt {
            data_stampa: o.data_stampa,
            percorso_critico: true,
            linea_base: p.cronoprogramma.linea_base.is_some(),
            comprimi: true,
        };
        return Ok(vec![FileReport {
            nome: "cronoprogramma_gantt.pdf".into(),
            byte: opencomputo_io::report_cronoprogramma::gantt(p, &opzioni)
                .map_err(errore_export)?
                .byte,
        }]);
    }
    if let Some(f) = report_tradizionale(p, tipo, formato, o)? {
        return Ok(vec![f]);
    }
    let (base, doc) = documento(p, tipo, o)?;
    Ok(match formato {
        FormatoReport::Pdf => vec![FileReport {
            nome: format!("{base}.pdf"),
            byte: pdf::documento(&doc, &pdf::OpzioniPdf::default()).byte,
        }],
        FormatoReport::Xlsx => vec![FileReport {
            nome: format!("{base}.xlsx"),
            byte: xlsx::documento(
                &doc,
                &xlsx::OpzioniXlsx {
                    data: o.data_stampa,
                },
            )
            .map_err(errore_export)?,
        }],
        FormatoReport::Csv => csv::documento(&doc, &csv::OpzioniCsv::default())
            .into_iter()
            .enumerate()
            .map(|(i, (titolo, testo))| {
                let sezione: String = titolo
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() {
                            c.to_ascii_lowercase()
                        } else {
                            '_'
                        }
                    })
                    .collect();
                FileReport {
                    nome: format!("{base}_{}_{sezione}.csv", i + 1),
                    byte: testo.into_bytes(),
                }
            })
            .collect(),
    })
}
