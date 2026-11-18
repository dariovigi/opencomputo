//! Import di listini CSV e JSON nel [`Prezzario`] del core.
//!
//! Le righe del CSV e gli elementi del JSON passano dalla stessa validazione:
//!
//! * scartate (anomalia di gravità «errore»): codice, descrizione o unità di misura
//!   vuoti; prezzo non numerico o negativo; codice già presente in una riga precedente;
//!   campi oltre quelli dell'intestazione;
//! * importate con un avviso: prezzo zero; incidenza non numerica o fuori da 0–100 (il
//!   valore originale resta nei dati originali della voce); natura non riconosciuta;
//!   stesso capitolo con descrizioni diverse.
//!
//! Vale sempre: righe di dati = voci importate + righe scartate.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::{Date, DateTime};
use opencomputo_core::id::GeneratoreId;
use opencomputo_core::numeri::{Percentuale, Prezzo};
use opencomputo_core::prezzario::{CapitoloPrezzario, NaturaVoce, Prezzario, VocePrezzario};
use opencomputo_core::provenienza::{Anomalia, FormatoOrigine, Provenienza};
use opencomputo_core::validazione::Gravita;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::colonne::{Campo, MappaturaColonne};
use crate::csv;
use crate::errore::ErroreListino;
use crate::numeri::{SeparatoreDecimale, interpreta, risolvi};

/// Dimensione massima del file, in byte.
pub const DIMENSIONE_MASSIMA: usize = 128 * 1024 * 1024;
/// Numero massimo di righe (CSV) o di voci (JSON).
pub const RIGHE_MASSIME: usize = 1_000_000;

/// Formato del listino.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatoListino {
    /// CSV con intestazione.
    Csv,
    /// JSON (oggetto con `voci`, oppure elenco di voci).
    Json,
}

/// Opzioni di import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpzioniListino {
    /// Nome del file, per la provenienza e come nome predefinito del prezzario.
    pub nome_file: Option<String>,
    /// Nome del prezzario (prevale su quello del file JSON e sul nome del file).
    pub nome: Option<String>,
    /// Ente emittente.
    pub ente: Option<String>,
    /// Edizione o versione.
    pub edizione: Option<String>,
    /// Momento dell'importazione, dichiarato dal chiamante.
    pub importato_il: Option<DateTime>,
    /// Separatore di campo del CSV; se assente si riconosce dall'intestazione.
    pub separatore_campi: Option<char>,
    /// Separatore decimale.
    pub separatore_decimale: SeparatoreDecimale,
    /// Nomi alternativi delle colonne del CSV.
    pub colonne: MappaturaColonne,
}

/// Rapporto di import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RapportoListino {
    /// Formato letto.
    pub formato: FormatoListino,
    /// Codifica del testo (`utf-8` o `windows-1252`).
    pub codifica: String,
    /// Impronta SHA-256 del file.
    pub impronta_sha256: String,
    /// Separatore di campo usato (solo CSV).
    pub separatore_campi: Option<char>,
    /// Separatore decimale usato, dopo il riconoscimento automatico.
    pub separatore_decimale: SeparatoreDecimale,
    /// Righe o elementi con dati (esclusi intestazione e righe vuote).
    pub righe_dati: usize,
    /// Righe vuote ignorate (solo CSV).
    pub righe_vuote: usize,
    /// Voci importate.
    pub voci_importate: usize,
    /// Righe scartate.
    pub righe_scartate: usize,
    /// Capitoli creati.
    pub capitoli: usize,
    /// Anomalie, nell'ordine del file.
    pub anomalie: Vec<Anomalia>,
}

/// Risultato dell'import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RisultatoListino {
    /// Prezzario letto.
    pub prezzario: Prezzario,
    /// Rapporto.
    pub rapporto: RapportoListino,
}

/// Voce letta dal file prima della validazione.
#[derive(Debug, Default)]
struct VoceGrezza {
    /// Posizione nel file (`riga 5`, `voci[3]`).
    posizione: String,
    /// Campi riconosciuti.
    campi: BTreeMap<Campo, String>,
    /// Campi arrivati come numeri JSON (sempre con il punto decimale).
    numerici: BTreeSet<Campo>,
    /// Campi non riconosciuti.
    altri: BTreeMap<String, String>,
    /// Problemi strutturali già noti (la riga va scartata).
    difetti: Vec<(String, String)>,
}

impl VoceGrezza {
    fn campo(&self, c: Campo) -> &str {
        self.campi.get(&c).map_or("", |s| s.trim())
    }
}

/// Metadati del prezzario letti dal file.
#[derive(Debug, Default)]
struct Metadati {
    nome: Option<String>,
    ente: Option<String>,
    edizione: Option<String>,
    data: Option<Date>,
    /// Capitoli dichiarati: codice, descrizione, codice del padre, posizione.
    capitoli: Vec<(String, String, Option<String>, String)>,
}

/// Impronta SHA-256 esadecimale.
#[must_use]
pub fn impronta(dati: &[u8]) -> String {
    Sha256::digest(dati)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            s.push_str(&format!("{b:02x}"));
            s
        })
}

fn anomalia(gravita: Gravita, codice: &str, messaggio: String, posizione: &str) -> Anomalia {
    Anomalia {
        gravita,
        codice: format!("listino.{codice}"),
        messaggio,
        posizione: (!posizione.is_empty()).then(|| posizione.to_owned()),
    }
}

/// Testo dei byte: UTF-8 (con o senza BOM), altrimenti Windows-1252.
fn decodifica(byte: &[u8]) -> (String, &'static str) {
    let senza_bom = byte.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(byte);
    match std::str::from_utf8(senza_bom) {
        Ok(t) => (t.to_owned(), "utf-8"),
        Err(_) => {
            let (t, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(senza_bom);
            (t.into_owned(), "windows-1252")
        }
    }
}

fn controlla_dimensione(byte: &[u8]) -> Result<(), ErroreListino> {
    if byte.len() > DIMENSIONE_MASSIMA {
        return Err(ErroreListino::TroppoGrande {
            massimo: DIMENSIONE_MASSIMA,
        });
    }
    Ok(())
}

/// Importa un listino riconoscendo il formato dal contenuto: JSON se il primo carattere
/// significativo è `{` o `[`, altrimenti CSV.
///
/// # Errors
/// Vedi [`importa_csv`] e [`importa_json`].
pub fn importa<G: GeneratoreId>(
    byte: &[u8],
    g: &mut G,
    opzioni: &OpzioniListino,
) -> Result<RisultatoListino, ErroreListino> {
    if sembra_json(byte) {
        importa_json(byte, g, opzioni)
    } else {
        importa_csv(byte, g, opzioni)
    }
}

/// `true` se il primo carattere significativo (dopo BOM e spazi) è `{` o `[`.
#[must_use]
pub fn sembra_json(byte: &[u8]) -> bool {
    let senza_bom = byte.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(byte);
    matches!(
        senza_bom.iter().find(|b| !b.is_ascii_whitespace()),
        Some(b'{' | b'[')
    )
}

/// Importa un listino CSV con intestazione.
///
/// # Errors
/// File vuoto o troppo grande, CSV non valido, colonne obbligatorie mancanti, nessuna
/// voce valida.
pub fn importa_csv<G: GeneratoreId>(
    byte: &[u8],
    g: &mut G,
    opzioni: &OpzioniListino,
) -> Result<RisultatoListino, ErroreListino> {
    controlla_dimensione(byte)?;
    let (testo, codifica) = decodifica(byte);
    let separatore = opzioni
        .separatore_campi
        .unwrap_or_else(|| csv::riconosci_separatore(&testo));
    let righe = csv::leggi(&testo, separatore, RIGHE_MASSIME)?;
    let mut iter = righe.into_iter().skip_while(csv::RigaCsv::vuota);
    let intestazione = iter.next().ok_or(ErroreListino::Vuoto)?;
    let colonne = opzioni.colonne.riconosci(&intestazione.campi)?;
    let attesi = intestazione.campi.len();

    let mut grezze = Vec::new();
    let mut vuote = 0usize;
    for riga in iter {
        if riga.vuota() {
            vuote += 1;
            continue;
        }
        let mut v = VoceGrezza {
            posizione: format!("riga {}", riga.numero),
            ..VoceGrezza::default()
        };
        if riga.campi.iter().skip(attesi).any(|c| !c.trim().is_empty()) {
            v.difetti.push((
                "campi_in_eccesso".into(),
                format!(
                    "la riga ha {} campi, l'intestazione {attesi}: possibile separatore dentro un campo senza virgolette",
                    riga.campi.len()
                ),
            ));
        }
        for &(campo, i) in &colonne.posizioni {
            if let Some(t) = riga.campi.get(i) {
                v.campi.insert(campo, t.clone());
            }
        }
        for (i, nome) in &colonne.altre {
            if let Some(t) = riga.campi.get(*i)
                && !t.trim().is_empty()
            {
                v.altri.insert(nome.clone(), t.clone());
            }
        }
        grezze.push(v);
    }
    let decimale = risolvi(
        opzioni.separatore_decimale,
        grezze.iter().map(|v| v.campo(Campo::Prezzo)),
    );
    costruisci(
        grezze,
        Metadati::default(),
        Contesto {
            formato: FormatoListino::Csv,
            codifica,
            impronta: impronta(byte),
            separatore_campi: Some(separatore),
            decimale,
            vuote,
        },
        g,
        opzioni,
    )
}

/// Testo di un valore JSON scalare; `None` per oggetti ed elenchi.
fn testo_json(v: &Value) -> Option<(String, bool)> {
    match v {
        Value::String(s) => Some((s.clone(), false)),
        Value::Number(n) => Some((n.to_string(), true)),
        Value::Bool(b) => Some((b.to_string(), false)),
        Value::Null => Some((String::new(), false)),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn campo_json(nome: &str) -> Option<Campo> {
    Some(match nome {
        "codice" | "tariffa" => Campo::Codice,
        "descrizione" | "descrizione_estesa" => Campo::Descrizione,
        "descrizione_breve" => Campo::DescrizioneBreve,
        "unita_misura" | "um" => Campo::UnitaMisura,
        "prezzo" => Campo::Prezzo,
        "capitolo" => Campo::Capitolo,
        "descrizione_capitolo" => Campo::DescrizioneCapitolo,
        "natura" => Campo::Natura,
        "incidenza_manodopera" => Campo::IncidenzaManodopera,
        "incidenza_sicurezza" => Campo::IncidenzaSicurezza,
        _ => return None,
    })
}

fn stringa(o: &Map<String, Value>, k: &str) -> Option<String> {
    o.get(k)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Importa un listino JSON.
///
/// Forme ammesse: un elenco di voci, oppure un oggetto con `voci` e, facoltativi, `nome`,
/// `ente`, `edizione`, `data_pubblicazione` (`AAAA-MM-GG`) e `capitoli` (`codice`,
/// `descrizione`, `padre`). Ogni voce è un oggetto con `codice`, `descrizione` (o
/// `descrizione_breve`), `unita_misura`, `prezzo` e, facoltativi, `capitolo`,
/// `descrizione_capitolo`, `natura`, `incidenza_manodopera`, `incidenza_sicurezza`. I
/// numeri si possono scrivere come numeri JSON o come testo (anche con la virgola). Gli
/// altri campi restano nei dati originali della voce.
///
/// # Errors
/// File troppo grande, JSON non valido o di forma diversa, nessuna voce valida.
pub fn importa_json<G: GeneratoreId>(
    byte: &[u8],
    g: &mut G,
    opzioni: &OpzioniListino,
) -> Result<RisultatoListino, ErroreListino> {
    controlla_dimensione(byte)?;
    let (testo, codifica) = decodifica(byte);
    let radice: Value = serde_json::from_str(&testo).map_err(|e| ErroreListino::Json {
        motivo: e.to_string(),
    })?;
    let mut meta = Metadati::default();
    let mut extra = Vec::new();
    let voci = match &radice {
        Value::Array(v) => v,
        Value::Object(o) => {
            meta.nome = stringa(o, "nome");
            meta.ente = stringa(o, "ente");
            meta.edizione = stringa(o, "edizione");
            if let Some(d) = stringa(o, "data_pubblicazione") {
                match Date::strptime("%Y-%m-%d", &d) {
                    Ok(d) => meta.data = Some(d),
                    Err(_) => extra.push(anomalia(
                        Gravita::Avviso,
                        "data_non_valida",
                        format!(
                            "data di pubblicazione «{d}» non valida (atteso AAAA-MM-GG): ignorata"
                        ),
                        "data_pubblicazione",
                    )),
                }
            }
            if let Some(caps) = o.get("capitoli") {
                let Value::Array(caps) = caps else {
                    return Err(ErroreListino::Json {
                        motivo: "«capitoli» deve essere un elenco".into(),
                    });
                };
                for (i, c) in caps.iter().enumerate() {
                    let pos = format!("capitoli[{i}]");
                    let codice = c.as_object().and_then(|o| stringa(o, "codice"));
                    match codice {
                        Some(codice) => meta.capitoli.push((
                            codice,
                            c.as_object()
                                .and_then(|o| stringa(o, "descrizione"))
                                .unwrap_or_default(),
                            c.as_object().and_then(|o| stringa(o, "padre")),
                            pos,
                        )),
                        None => extra.push(anomalia(
                            Gravita::Avviso,
                            "capitolo_senza_codice",
                            "capitolo senza codice: ignorato".into(),
                            &pos,
                        )),
                    }
                }
            }
            for k in o.keys() {
                if !matches!(
                    k.as_str(),
                    "voci"
                        | "nome"
                        | "ente"
                        | "edizione"
                        | "data_pubblicazione"
                        | "capitoli"
                        | "formato"
                        | "versione"
                ) {
                    extra.push(anomalia(
                        Gravita::Informazione,
                        "campo_sconosciuto",
                        format!("campo «{k}» non riconosciuto: ignorato"),
                        k,
                    ));
                }
            }
            match o.get("voci") {
                Some(Value::Array(v)) => v,
                _ => {
                    return Err(ErroreListino::Json {
                        motivo: "manca l'elenco «voci»".into(),
                    });
                }
            }
        }
        _ => {
            return Err(ErroreListino::Json {
                motivo: "atteso un oggetto con «voci» o un elenco di voci".into(),
            });
        }
    };
    if voci.len() > RIGHE_MASSIME {
        return Err(ErroreListino::TroppeRighe {
            massimo: RIGHE_MASSIME,
        });
    }
    let mut grezze = Vec::with_capacity(voci.len());
    for (i, elemento) in voci.iter().enumerate() {
        let mut v = VoceGrezza {
            posizione: format!("voci[{i}]"),
            ..VoceGrezza::default()
        };
        match elemento {
            Value::Object(o) => {
                for (k, valore) in o {
                    match (campo_json(k), testo_json(valore)) {
                        (Some(c), Some((t, numero))) => {
                            if numero {
                                v.numerici.insert(c);
                            }
                            v.campi.insert(c, t);
                        }
                        (Some(c), None) => v.difetti.push((
                            "campo_non_scalare".into(),
                            format!("il campo «{}» deve essere un testo o un numero", c.nome()),
                        )),
                        (None, Some((t, _))) => {
                            if !t.is_empty() {
                                v.altri.insert(k.clone(), t);
                            }
                        }
                        (None, None) => {
                            v.altri.insert(k.clone(), valore.to_string());
                        }
                    }
                }
            }
            _ => v.difetti.push((
                "elemento_non_oggetto".into(),
                "la voce non è un oggetto JSON".into(),
            )),
        }
        grezze.push(v);
    }
    let decimale = risolvi(
        opzioni.separatore_decimale,
        grezze
            .iter()
            .filter(|v| !v.numerici.contains(&Campo::Prezzo))
            .map(|v| v.campo(Campo::Prezzo)),
    );
    let mut r = costruisci(
        grezze,
        meta,
        Contesto {
            formato: FormatoListino::Json,
            codifica,
            impronta: impronta(byte),
            separatore_campi: None,
            decimale,
            vuote: 0,
        },
        g,
        opzioni,
    )?;
    if !extra.is_empty() {
        extra.append(&mut r.rapporto.anomalie);
        r.rapporto.anomalie = extra;
        if let Some(p) = &mut r.prezzario.provenienza {
            p.anomalie.clone_from(&r.rapporto.anomalie);
        }
    }
    Ok(r)
}

/// Riga valida, in attesa della creazione dei capitoli.
struct Accettata {
    grezza: VoceGrezza,
    prezzo: Prezzo,
    natura: NaturaVoce,
    manodopera: Option<Percentuale>,
    sicurezza: Option<Percentuale>,
}

struct Contesto {
    formato: FormatoListino,
    codifica: &'static str,
    impronta: String,
    separatore_campi: Option<char>,
    decimale: SeparatoreDecimale,
    vuote: usize,
}

fn numero_campo(
    v: &VoceGrezza,
    c: Campo,
    decimale: SeparatoreDecimale,
    percentuale: bool,
) -> Option<Decimal> {
    let sep = if v.numerici.contains(&c) {
        SeparatoreDecimale::Punto
    } else {
        decimale
    };
    interpreta(v.campo(c), sep, percentuale)
}

fn natura(testo: &str) -> Option<NaturaVoce> {
    let t = testo.trim().to_lowercase();
    if t.is_empty() || matches!(t.as_str(), "l" | "lavoro" | "lavori" | "lavorazione") {
        Some(NaturaVoce::Lavoro)
    } else if t.contains("sicurezza") || t.contains("oneri") || matches!(t.as_str(), "os" | "s") {
        Some(NaturaVoce::OneriSicurezza)
    } else {
        None
    }
}

/// Capitolo padre di `codice` tra i codici noti: il più lungo che ne è prefisso seguito
/// da un separatore (`.`, `-`, `/`, `_`, spazio).
fn padre_per_prefisso<'a>(codice: &str, noti: impl Iterator<Item = &'a String>) -> Option<String> {
    noti.filter(|p| {
        p.len() < codice.len()
            && codice.starts_with(p.as_str())
            && codice
                .get(p.len()..)
                .is_some_and(|resto| resto.starts_with(['.', '-', '/', '_', ' ']))
    })
    .max_by_key(|p| p.len())
    .cloned()
}

#[allow(clippy::too_many_lines)]
fn costruisci<G: GeneratoreId>(
    grezze: Vec<VoceGrezza>,
    meta: Metadati,
    ctx: Contesto,
    g: &mut G,
    opzioni: &OpzioniListino,
) -> Result<RisultatoListino, ErroreListino> {
    let mut anomalie = Vec::new();
    if ctx.codifica != "utf-8" {
        anomalie.push(anomalia(
            Gravita::Informazione,
            "codifica",
            format!("il file non è UTF-8: letto come {}", ctx.codifica),
            "",
        ));
    }
    let righe_dati = grezze.len();
    let nome = opzioni
        .nome
        .clone()
        .or(meta.nome.clone())
        .or_else(|| {
            opzioni.nome_file.as_deref().map(|f| {
                std::path::Path::new(f)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(f)
                    .to_owned()
            })
        })
        .unwrap_or_else(|| "Listino".to_owned());
    let mut prezzario = Prezzario::new(g.nuovo(), nome);
    prezzario.ente = opzioni.ente.clone().or(meta.ente.clone());
    prezzario.edizione = opzioni.edizione.clone().or(meta.edizione.clone());
    prezzario.data_pubblicazione = meta.data;

    // Capitoli dichiarati (JSON): codice → (descrizione, padre).
    let mut capitoli: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for (codice, descrizione, padre, pos) in meta.capitoli {
        if capitoli.contains_key(&codice) {
            anomalie.push(anomalia(
                Gravita::Avviso,
                "capitolo_duplicato",
                format!("capitolo «{codice}» dichiarato più volte: vale il primo"),
                &pos,
            ));
            continue;
        }
        capitoli.insert(codice, (descrizione, padre));
    }
    let dichiarati = !capitoli.is_empty();

    let mut accettate: Vec<Accettata> = Vec::new();
    let mut visti: BTreeMap<String, String> = BTreeMap::new();
    let mut scartate = 0usize;
    for mut v in grezze {
        let pos = v.posizione.clone();
        let mut errori: Vec<(String, String)> = std::mem::take(&mut v.difetti);
        if errori.iter().any(|(c, _)| c == "elemento_non_oggetto") {
            scartate += 1;
            for (c, m) in errori {
                anomalie.push(anomalia(Gravita::Errore, &c, m, &pos));
            }
            continue;
        }
        let codice = v.campo(Campo::Codice).to_owned();
        if codice.is_empty() {
            errori.push(("codice_vuoto".into(), "codice vuoto".into()));
        }
        if v.campo(Campo::Descrizione).is_empty() && v.campo(Campo::DescrizioneBreve).is_empty() {
            errori.push((
                "descrizione_vuota".into(),
                format!("voce «{codice}»: descrizione vuota"),
            ));
        }
        if v.campo(Campo::UnitaMisura).is_empty() {
            errori.push((
                "unita_vuota".into(),
                format!("voce «{codice}»: unità di misura vuota"),
            ));
        }
        let prezzo = numero_campo(&v, Campo::Prezzo, ctx.decimale, false);
        match prezzo {
            None => errori.push((
                "prezzo_non_valido".into(),
                format!(
                    "voce «{codice}»: prezzo «{}» non valido",
                    v.campo(Campo::Prezzo)
                ),
            )),
            Some(p) if p.is_sign_negative() && !p.is_zero() => errori.push((
                "prezzo_negativo".into(),
                format!("voce «{codice}»: prezzo negativo ({p})"),
            )),
            Some(_) => {}
        }
        if !codice.is_empty()
            && let Some(prima) = visti.get(&codice)
        {
            errori.push((
                "codice_duplicato".into(),
                format!("codice «{codice}» già presente ({prima}): riga scartata"),
            ));
        }
        if !errori.is_empty() {
            scartate += 1;
            for (c, m) in errori {
                anomalie.push(anomalia(Gravita::Errore, &c, m, &pos));
            }
            continue;
        }
        let prezzo = prezzo.unwrap_or_default();
        visti.insert(codice.clone(), pos.clone());
        if prezzo.is_zero() {
            anomalie.push(anomalia(
                Gravita::Avviso,
                "prezzo_zero",
                format!("voce «{codice}»: prezzo zero"),
                &pos,
            ));
        }
        let mut incidenza = |c: Campo, v: &mut VoceGrezza| -> Option<Percentuale> {
            let testo = v.campo(c).to_owned();
            if testo.is_empty() {
                return None;
            }
            match numero_campo(v, c, ctx.decimale, true) {
                Some(d) if d >= Decimal::ZERO && d <= Decimal::ONE_HUNDRED => {
                    Some(Percentuale::new(d))
                }
                _ => {
                    anomalie.push(anomalia(
                        Gravita::Avviso,
                        "incidenza_non_valida",
                        format!(
                            "voce «{codice}»: {} «{testo}» non valida (0–100): ignorata",
                            c.nome()
                        ),
                        &pos,
                    ));
                    v.altri.insert(c.nome().to_owned(), testo);
                    None
                }
            }
        };
        let manodopera = incidenza(Campo::IncidenzaManodopera, &mut v);
        let sicurezza = incidenza(Campo::IncidenzaSicurezza, &mut v);
        let nat = match natura(v.campo(Campo::Natura)) {
            Some(n) => n,
            None => {
                anomalie.push(anomalia(
                    Gravita::Avviso,
                    "natura_sconosciuta",
                    format!(
                        "voce «{codice}»: natura «{}» non riconosciuta, considerata lavoro",
                        v.campo(Campo::Natura)
                    ),
                    &pos,
                ));
                let t = v.campo(Campo::Natura).to_owned();
                v.altri.insert("natura".into(), t);
                NaturaVoce::Lavoro
            }
        };
        let cap = v.campo(Campo::Capitolo).to_owned();
        if !cap.is_empty() {
            let desc = v.campo(Campo::DescrizioneCapitolo).to_owned();
            match capitoli.get_mut(&cap) {
                Some((d, _)) => {
                    if d.is_empty() {
                        d.clone_from(&desc);
                    } else if !desc.is_empty() && *d != desc {
                        anomalie.push(anomalia(
                            Gravita::Avviso,
                            "capitolo_descrizioni_diverse",
                            format!("capitolo «{cap}»: descrizione «{desc}» diversa da «{d}», vale la prima"),
                            &pos,
                        ));
                    }
                }
                None => {
                    if dichiarati {
                        anomalie.push(anomalia(
                            Gravita::Avviso,
                            "capitolo_non_dichiarato",
                            format!("voce «{codice}»: capitolo «{cap}» non dichiarato in «capitoli»: creato"),
                            &pos,
                        ));
                    }
                    capitoli.insert(cap, (desc, None));
                }
            }
        }
        accettate.push(Accettata {
            grezza: v,
            prezzo: Prezzo::new(prezzo),
            natura: nat,
            manodopera,
            sicurezza,
        });
    }

    if accettate.is_empty() {
        if righe_dati == 0 {
            return Err(ErroreListino::Vuoto);
        }
        return Err(ErroreListino::NessunaVoceValida {
            scartate,
            prima: anomalie
                .iter()
                .find(|a| a.gravita == Gravita::Errore)
                .map(|a| a.messaggio.clone())
                .unwrap_or_default(),
        });
    }

    // Capitoli: padre dichiarato o, se assente, per prefisso del codice. I padri sono
    // inseriti prima dei figli; un padre inesistente o in un ciclo viene tolto.
    let codici: Vec<String> = capitoli.keys().cloned().collect();
    let mut padri: BTreeMap<String, Option<String>> = BTreeMap::new();
    for c in &codici {
        let dichiarato = capitoli.get(c).and_then(|(_, p)| p.clone());
        let padre = match dichiarato {
            Some(p) if capitoli.contains_key(&p) && p != *c => Some(p),
            Some(p) => {
                anomalie.push(anomalia(
                    Gravita::Avviso,
                    "capitolo_padre_inesistente",
                    format!("capitolo «{c}»: padre «{p}» inesistente, ignorato"),
                    "capitoli",
                ));
                None
            }
            None if dichiarati => None,
            None => padre_per_prefisso(c, codici.iter()),
        };
        padri.insert(c.clone(), padre);
    }
    let mut ids: BTreeMap<String, opencomputo_core::id::CapitoloId> = BTreeMap::new();
    let mut in_attesa: Vec<String> = codici;
    while !in_attesa.is_empty() {
        let prima = in_attesa.len();
        let mut resto = Vec::new();
        for c in in_attesa {
            let padre = padri.get(&c).cloned().flatten();
            let pronto = padre.as_ref().is_none_or(|p| ids.contains_key(p));
            if !pronto {
                resto.push(c);
                continue;
            }
            let id = g.nuovo();
            let descrizione = capitoli.get(&c).map(|(d, _)| d.clone()).unwrap_or_default();
            let capitolo = CapitoloPrezzario {
                id,
                padre: padre.and_then(|p| ids.get(&p).copied()),
                codice: c.clone(),
                descrizione,
            };
            // Il padre è già inserito e l'identificativo è nuovo: non può fallire.
            if prezzario.aggiungi_capitolo(capitolo).is_ok() {
                ids.insert(c, id);
            }
        }
        if resto.len() == prima {
            // Ciclo tra padri dichiarati: i capitoli rimasti diventano di primo livello.
            for c in &resto {
                anomalie.push(anomalia(
                    Gravita::Avviso,
                    "capitolo_in_ciclo",
                    format!("capitolo «{c}»: i padri formano un ciclo, reso di primo livello"),
                    "capitoli",
                ));
                padri.insert(c.clone(), None);
            }
        }
        in_attesa = resto;
    }

    let formato_origine = match ctx.formato {
        FormatoListino::Csv => FormatoOrigine::Csv,
        FormatoListino::Json => FormatoOrigine::Altro("json".into()),
    };
    let mut codici_inseriti: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(accettate.len());
    prezzario.voci.reserve(accettate.len());
    for Accettata {
        grezza: v,
        prezzo,
        natura: nat,
        manodopera,
        sicurezza,
    } in accettate
    {
        let estesa = match v.campo(Campo::Descrizione) {
            "" => v.campo(Campo::DescrizioneBreve),
            d => d,
        };
        let mut voce = VocePrezzario::new(
            g.nuovo(),
            v.campo(Campo::Codice),
            estesa,
            v.campo(Campo::UnitaMisura),
            prezzo,
        );
        if !v.campo(Campo::DescrizioneBreve).is_empty() {
            v.campo(Campo::DescrizioneBreve)
                .clone_into(&mut voce.descrizione_breve);
        }
        voce.natura = nat;
        voce.incidenza_manodopera = manodopera;
        voce.incidenza_sicurezza = sicurezza;
        voce.capitolo = ids.get(v.campo(Campo::Capitolo)).copied();
        let mut prov = Provenienza::new(formato_origine.clone());
        prov.identificativo_originale = Some(v.posizione.clone());
        prov.dati_originali = v.altri.clone();
        voce.provenienza = Some(prov);
        // Identificativo nuovo, capitolo appena creato; il codice si ricontrolla qui con un
        // insieme: `Prezzario::aggiungi_voce` scorre tutte le voci a ogni inserimento, e
        // su un listino di decine di migliaia di voci impiega secondi.
        if codici_inseriti.insert(voce.codice.clone()) {
            prezzario.voci.push(voce);
        } else {
            anomalie.push(anomalia(
                Gravita::Errore,
                "voce_rifiutata",
                format!("codice «{}» già presente", voce.codice),
                &v.posizione,
            ));
            scartate += 1;
        }
    }

    let mut prov = Provenienza::new(formato_origine);
    prov.file.clone_from(&opzioni.nome_file);
    prov.impronta_sha256 = Some(ctx.impronta.clone());
    prov.importato_il = opzioni.importato_il;
    prov.anomalie.clone_from(&anomalie);
    prezzario.provenienza = Some(prov);

    let rapporto = RapportoListino {
        formato: ctx.formato,
        codifica: ctx.codifica.to_owned(),
        impronta_sha256: ctx.impronta,
        separatore_campi: ctx.separatore_campi,
        separatore_decimale: ctx.decimale,
        righe_dati,
        righe_vuote: ctx.vuote,
        voci_importate: prezzario.voci.len(),
        righe_scartate: scartate,
        capitoli: prezzario.capitoli.len(),
        anomalie,
    };
    Ok(RisultatoListino {
        prezzario,
        rapporto,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn natura_riconosciuta() {
        assert_eq!(natura(""), Some(NaturaVoce::Lavoro));
        assert_eq!(natura("Lavori"), Some(NaturaVoce::Lavoro));
        assert_eq!(
            natura("Oneri della sicurezza"),
            Some(NaturaVoce::OneriSicurezza)
        );
        assert_eq!(natura("OS"), Some(NaturaVoce::OneriSicurezza));
        assert_eq!(natura("fornitura"), None);
    }

    #[test]
    fn padre_dal_prefisso() {
        let noti: Vec<String> = ["A", "A.01", "A.01.02", "AB"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            padre_per_prefisso("A.01.02", noti.iter()),
            Some("A.01".into())
        );
        assert_eq!(padre_per_prefisso("A.01", noti.iter()), Some("A".into()));
        assert_eq!(padre_per_prefisso("AB", noti.iter()), None);
        assert_eq!(padre_per_prefisso("A", noti.iter()), None);
    }

    #[test]
    fn riconoscimento_json() {
        assert!(sembra_json(b"\xEF\xBB\xBF  {\"voci\": []}"));
        assert!(sembra_json(b"\n[ ]"));
        assert!(!sembra_json(b"Codice;Prezzo"));
    }
}
