//! Comandi che modificano un progetto.
//!
//! I comandi sono **dati serializzabili** (JSON): la GUI, la riga di comando e gli
//! agenti AI li costruiscono allo stesso modo e passano tutti dalla stessa
//! [`Sessione`](crate::Sessione), con le stesse validazioni.
//!
//! Le entità sono indicate con riferimenti leggibili, stabili per chi scrive i comandi:
//!
//! * voci di elenco prezzi, capitoli, categorie per **codice**;
//! * nodi WBS per **codice completo** (es. `01.02`);
//! * computi per **nome** (facoltativo se il progetto ne ha uno solo);
//! * voci di computo per **numero d'ordine** (da 1) oppure per **identificativo**;
//! * righi per numero d'ordine nella voce; registrazioni e SAL per numero.
//!
//! I numeri si scrivono come stringhe (`"12.50"`) o come numeri JSON; i fattori di
//! misura accettano anche espressioni (`"2*(3,50+0,30)"`).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use jiff::civil::Date;
use opencomputo_core::aggiornamento::{
    CampiAggiornamento, SceltaAggiornamento, SostituzionePrefisso, aggiorna_da_listino,
};
use opencomputo_core::analisi::{BaseUtile, TipoRisorsa};
use opencomputo_core::classificazione::{
    AssegnazioneCategorie, Categorie, Classe, LivelloCategoria, NodoWbs,
};
use opencomputo_core::computo::{Computo, TipoComputo, VoceComputo};
use opencomputo_core::contabilita::{
    Anticipazione, Collaudo, Penale, RegistrazioneLibretto, TipoCollaudo,
};
use opencomputo_core::cronoprogramma::{Chiusura, TipoLegame};
use opencomputo_core::id::{AttivitaId, ClasseId, GeneratoreId, VoceComputoId};
use opencomputo_core::misura::{Fattore, RigoMisura, SegnoRigo, TipoRigo, VediVoce};
use opencomputo_core::numeri::{Importo, ModalitaArrotondamento, Percentuale, Prezzo};
use opencomputo_core::partita::PrezzoApplicato;
use opencomputo_core::prezzario::{
    CapitoloPrezzario, CriterioNuovoPrezzo, NaturaVoce, Prezzario, VocePrezzario,
};
use opencomputo_core::progetto::{DatiContratto, Progetto};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Fattore di misura in ingresso: numero, oppure testo con un numero (anche con la
/// virgola) o un'espressione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FattoreIngresso {
    /// Numero.
    Numero(Decimal),
    /// Numero in forma di testo o espressione.
    Testo(String),
}

impl FattoreIngresso {
    pub(crate) fn in_fattore(&self) -> Fattore {
        match self {
            Self::Numero(d) => Fattore::Valore(*d),
            Self::Testo(t) => {
                let n = t.trim().replace(',', ".");
                match Decimal::from_str_exact(&n) {
                    Ok(d) if !t.contains('.') || !t.contains(',') => Fattore::Valore(d),
                    _ => Fattore::Espressione(t.trim().to_owned()),
                }
            }
        }
    }
}

/// Rigo di misura in ingresso.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NuovoRigo {
    /// Descrizione.
    #[serde(default)]
    pub descrizione: String,
    /// Rigo di solo commento (nessuna quantità).
    #[serde(default)]
    pub commento: bool,
    /// Parti uguali.
    #[serde(default)]
    pub parti_uguali: Option<FattoreIngresso>,
    /// Lunghezza.
    #[serde(default)]
    pub lunghezza: Option<FattoreIngresso>,
    /// Larghezza.
    #[serde(default)]
    pub larghezza: Option<FattoreIngresso>,
    /// Altezza o peso.
    #[serde(default)]
    pub altezza_peso: Option<FattoreIngresso>,
    /// Formula che sostituisce il prodotto (variabili `par`, `lun`, `lar`, `alt`).
    #[serde(default)]
    pub formula: Option<String>,
    /// Rigo in deduzione.
    #[serde(default)]
    pub deduzione: bool,
    /// «Vedi voce»: il rigo parte dalla quantità di un'altra voce.
    #[serde(default)]
    pub vedi_voce: Option<VediVoceIngresso>,
}

/// «Vedi voce» in ingresso.
///
/// * numero d'ordine o identificativo: voce dello stesso computo, collegata (la
///   quantità segue la voce);
/// * `{"computo", "voce"}`: voce di un computo indicato; nello stesso computo è un
///   collegamento, altrove e nel libretto la quantità viene copiata (statica);
/// * `{"quantita", "riferimento"}`: quantità statica con il testo del riferimento.
///
/// Nel libretto delle misure il «vedi voce» è sempre statico (la voce va cercata nel
/// computo indicato, o nell'unico computo del progetto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VediVoceIngresso {
    /// Voce dello stesso computo.
    Voce(RifVoce),
    /// Voce di un computo indicato.
    DaComputo {
        /// Nome del computo.
        computo: String,
        /// Voce.
        voce: RifVoce,
    },
    /// Quantità statica.
    Statica {
        /// Quantità della voce citata.
        quantita: FattoreIngresso,
        /// Voce citata, per le stampe.
        riferimento: String,
    },
}

/// Testo del riferimento a una voce per i «vedi voce» statici.
pub(crate) fn riferimento_voce(c: &Computo, indice: usize) -> String {
    let codice = c.voci.get(indice).map_or("", |v| v.prezzo.codice.as_str());
    format!("voce n. {} «{codice}» del computo «{}»", indice + 1, c.nome)
}

/// «Vedi voce» statico con la quantità attuale della voce indicata.
pub(crate) fn vedi_voce_statico(
    c: &Computo,
    indice: usize,
    regole: &opencomputo_core::numeri::RegoleArrotondamento,
) -> Result<VediVoce, String> {
    let v = c.voci.get(indice).ok_or("voce inesistente")?;
    let quantita = c
        .quantita_voce(v.id, regole)
        .map_err(|e| format!("«vedi voce»: la voce citata non è calcolabile: {e}"))?;
    Ok(VediVoce::Statica {
        quantita: quantita.valore(),
        riferimento: riferimento_voce(c, indice),
    })
}

/// «Vedi voce» statici con la quantità attuale delle voci `citate` di `c`.
fn statici_per(
    c: &Computo,
    citate: &BTreeSet<VoceComputoId>,
    regole: &opencomputo_core::numeri::RegoleArrotondamento,
) -> Result<HashMap<VoceComputoId, VediVoce>, String> {
    citate
        .iter()
        .filter_map(|id| c.voci.iter().position(|v| v.id == *id).map(|i| (*id, i)))
        .map(|(id, i)| Ok((id, vedi_voce_statico(c, i, regole)?)))
        .collect()
}

/// Sostituisce nei righi i collegamenti alle voci di `statici` con la quantità copiata;
/// restituisce quanti righi sono cambiati.
fn sostituisci_collegamenti<'a>(
    voci: impl Iterator<Item = &'a mut VoceComputo>,
    statici: &HashMap<VoceComputoId, VediVoce>,
) -> usize {
    let mut n = 0;
    for v in voci {
        for r in &mut v.righi {
            if let Some(nuovo) = r
                .vedi_voce
                .as_ref()
                .and_then(VediVoce::voce)
                .and_then(|id| statici.get(&id))
            {
                r.vedi_voce = Some(nuovo.clone());
                n += 1;
            }
        }
    }
    n
}

/// Rende statici, nelle voci di `c` fuori da `voci`, i righi che citano le voci `voci`:
/// servono prima di toglierle dal computo. Restituisce quanti righi sono cambiati.
fn rendi_statici_i_riferimenti(
    c: &mut Computo,
    voci: &BTreeSet<VoceComputoId>,
    regole: &opencomputo_core::numeri::RegoleArrotondamento,
) -> Result<usize, String> {
    let citate: BTreeSet<VoceComputoId> = c
        .voci
        .iter()
        .filter(|v| !voci.contains(&v.id))
        .flat_map(|v| v.voci_citate())
        .filter(|id| voci.contains(id))
        .collect();
    if citate.is_empty() {
        return Ok(0);
    }
    let statici = statici_per(c, &citate, regole)?;
    // Solo le voci che citano davvero: le altre restano condivise con le copie del
    // progetto (cronologia di «Annulla»).
    Ok(sostituisci_collegamenti(
        c.voci
            .iter_mut()
            .filter(|v| !voci.contains(&v.id) && v.voci_citate().any(|id| citate.contains(&id)))
            .map(Arc::make_mut),
        &statici,
    ))
}

/// Frase del messaggio per i righi resi statici.
fn resi_statici(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => "; 1 rigo «vedi voce» reso statico".into(),
        n => format!("; {n} righi «vedi voce» resi statici"),
    }
}

impl VediVoceIngresso {
    /// Risolve il riferimento per un rigo del computo `computo`, o del libretto se
    /// `None`.
    fn risolvi(&self, p: &Progetto, computo: Option<&str>) -> Result<VediVoce, String> {
        match self {
            Self::Statica {
                quantita,
                riferimento,
            } => Ok(VediVoce::Statica {
                quantita: quantita
                    .in_fattore()
                    .valuta()
                    .map_err(|e| format!("quantità del «vedi voce»: {e}"))?,
                riferimento: riferimento.trim().to_owned(),
            }),
            Self::Voce(voce) => {
                // Un identificativo di una voce di un altro computo (righi copiati e
                // incollati altrove, anche nel libretto): la quantità viene copiata.
                if let RifVoce::Id(id) = voce {
                    let altrove = p
                        .computi
                        .iter()
                        .find_map(|c| c.voci.iter().position(|v| v.id == *id).map(|i| (c, i)));
                    if let Some((c, i)) = altrove
                        && (computo.is_none() || computo != Some(c.nome.as_str()))
                    {
                        return vedi_voce_statico(c, i, &p.regole);
                    }
                }
                let c = computo_rif(p, computo)?;
                let i = indice_voce(c, *voce)?;
                match computo {
                    Some(_) => Ok(VediVoce::Collegata(
                        c.voci.get(i).map(|v| v.id).ok_or("voce inesistente")?,
                    )),
                    None => vedi_voce_statico(c, i, &p.regole),
                }
            }
            Self::DaComputo {
                computo: nome,
                voce,
            } => {
                let c = computo_rif(p, Some(nome))?;
                let i = indice_voce(c, *voce)?;
                if computo == Some(nome.as_str()) {
                    Ok(VediVoce::Collegata(
                        c.voci.get(i).map(|v| v.id).ok_or("voce inesistente")?,
                    ))
                } else {
                    vedi_voce_statico(c, i, &p.regole)
                }
            }
        }
    }
}

impl NuovoRigo {
    /// Rigo del computo indicato (o del libretto, con `None`).
    fn in_rigo_di<G: GeneratoreId>(
        &self,
        g: &mut G,
        p: &Progetto,
        computo: Option<&str>,
    ) -> Result<RigoMisura, String> {
        let mut r = self.in_rigo(g);
        r.vedi_voce = self
            .vedi_voce
            .as_ref()
            .map(|v| v.risolvi(p, computo))
            .transpose()?;
        Ok(r)
    }

    fn in_rigo<G: GeneratoreId>(&self, g: &mut G) -> RigoMisura {
        let f = |x: &Option<FattoreIngresso>| x.as_ref().map(FattoreIngresso::in_fattore);
        RigoMisura {
            id: g.nuovo(),
            tipo: if self.commento {
                TipoRigo::Commento
            } else {
                TipoRigo::Misura
            },
            descrizione: self.descrizione.clone(),
            parti_uguali: f(&self.parti_uguali),
            lunghezza: f(&self.lunghezza),
            larghezza: f(&self.larghezza),
            altezza_peso: f(&self.altezza_peso),
            formula: self.formula.clone().filter(|x| !x.trim().is_empty()),
            segno: if self.deduzione {
                SegnoRigo::Deduzione
            } else {
                SegnoRigo::Positivo
            },
            vedi_voce: None,
        }
    }
}

/// Riferimento a una voce di computo: numero d'ordine (da 1) o identificativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RifVoce {
    /// Numero d'ordine, da 1.
    Numero(usize),
    /// Identificativo.
    Id(VoceComputoId),
}

/// Categorie per codice (ogni livello facoltativo; stringa vuota = nessuna).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategorieIngresso {
    /// Super categoria.
    #[serde(default)]
    pub super_categoria: Option<String>,
    /// Categoria.
    #[serde(default)]
    pub categoria: Option<String>,
    /// Sotto categoria.
    #[serde(default)]
    pub sotto_categoria: Option<String>,
}

/// Attività del cronoprogramma nei comandi: un nodo WBS per codice completo o
/// un'attività aggiunta per identificativo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RifAttivitaComando {
    /// Nodo WBS, es. `{"wbs": "01.02"}`.
    Wbs(String),
    /// Attività aggiunta, es. `{"attivita": "…"}`.
    Attivita(AttivitaId),
}

/// Comando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "comando", rename_all = "snake_case", deny_unknown_fields)]
pub enum Comando {
    /// Imposta i metadati (i campi assenti restano invariati; stringa vuota = cancella).
    ImpostaMetadati {
        /// Titolo.
        #[serde(default)]
        titolo: Option<String>,
        /// Oggetto dei lavori.
        #[serde(default)]
        oggetto: Option<String>,
        /// Committente.
        #[serde(default)]
        committente: Option<String>,
        /// Impresa.
        #[serde(default)]
        impresa: Option<String>,
        /// Luogo.
        #[serde(default)]
        luogo: Option<String>,
        /// CIG.
        #[serde(default)]
        cig: Option<String>,
        /// CUP.
        #[serde(default)]
        cup: Option<String>,
        /// Responsabile del procedimento.
        #[serde(default)]
        responsabile_procedimento: Option<String>,
        /// Progettista.
        #[serde(default)]
        progettista: Option<String>,
        /// Direttore dei lavori.
        #[serde(default)]
        direttore_lavori: Option<String>,
    },
    /// Imposta i dati contrattuali (crea il contratto se assente).
    ImpostaContratto {
        /// Ribasso d'asta in percentuale (es. `"12.5"`).
        #[serde(default)]
        ribasso: Option<Percentuale>,
        /// Ritenuta di garanzia in percentuale.
        #[serde(default)]
        ritenuta_garanzia: Option<Percentuale>,
        /// Manodopera esclusa dal ribasso.
        #[serde(default)]
        manodopera_esclusa_dal_ribasso: Option<bool>,
        /// Importo contrattuale.
        #[serde(default)]
        importo_contrattuale: Option<Importo>,
        /// Data di consegna dei lavori.
        #[serde(default)]
        data_consegna_lavori: Option<Date>,
        /// Aliquota IVA dei certificati di pagamento (`null` = nessuna).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        aliquota_iva: Option<Option<Percentuale>>,
    },
    /// Imposta l'anticipazione del prezzo (art. 125 D.Lgs. 36/2023). Senza `importo`,
    /// è calcolata sull'importo contrattuale con la percentuale (predefinita 20 %).
    ImpostaAnticipazione {
        /// Percentuale del valore del contratto.
        #[serde(default)]
        percentuale: Option<Percentuale>,
        /// Importo erogato (se diverso dalla percentuale dell'importo contrattuale).
        #[serde(default)]
        importo: Option<Importo>,
        /// Percentuale di recupero sull'importo netto progressivo (predefinita uguale
        /// alla percentuale dell'anticipazione).
        #[serde(default)]
        percentuale_recupero: Option<Percentuale>,
        /// Data di erogazione.
        #[serde(default)]
        data_erogazione: Option<Date>,
        /// Rimuove l'anticipazione.
        #[serde(default)]
        rimuovi: bool,
    },
    /// Registra una penale: importo indicato oppure giorni di ritardo × per mille
    /// dell'importo contrattuale (art. 126: 0,5–1,5 per mille, al massimo 10 %).
    RegistraPenale {
        /// Data: la penale va nel primo certificato il cui SAL arriva a questa data.
        data: Date,
        /// Motivo.
        descrizione: String,
        /// Importo.
        #[serde(default)]
        importo: Option<Importo>,
        /// Giorni di ritardo.
        #[serde(default)]
        giorni: Option<u32>,
        /// Misura giornaliera in millesimi dell'importo contrattuale (es. `"1"`).
        #[serde(default)]
        per_mille: Option<Decimal>,
    },
    /// Rimuove una penale non ancora applicata in un SAL emesso.
    RimuoviPenale {
        /// Numero della penale.
        numero: u32,
    },
    /// Opzioni del certificato di un SAL in bozza.
    ImpostaSal {
        /// Numero del SAL.
        numero: u32,
        /// Sospende il recupero dell'anticipazione in questo certificato.
        #[serde(default)]
        recupero_anticipazione_sospeso: Option<bool>,
        /// Recupera tutta l'anticipazione residua (di norma nell'ultimo SAL).
        #[serde(default)]
        recupera_residuo_anticipazione: Option<bool>,
    },
    /// Imposta le regole di arrotondamento.
    ImpostaRegole {
        /// Decimali delle quantità.
        #[serde(default)]
        decimali_quantita: Option<u32>,
        /// Decimali dei prezzi calcolati.
        #[serde(default)]
        decimali_prezzo: Option<u32>,
        /// Decimali degli importi.
        #[serde(default)]
        decimali_importo: Option<u32>,
        /// Modalità di arrotondamento.
        #[serde(default)]
        modalita: Option<ModalitaArrotondamento>,
    },
    /// Aggiunge un capitolo all'elenco prezzi.
    AggiungiCapitolo {
        /// Codice.
        codice: String,
        /// Descrizione.
        descrizione: String,
        /// Codice del capitolo padre.
        #[serde(default)]
        padre: Option<String>,
    },
    /// Aggiunge una voce all'elenco prezzi.
    AggiungiVocePrezzario {
        /// Codice (tariffa).
        codice: String,
        /// Descrizione breve.
        #[serde(default)]
        descrizione_breve: Option<String>,
        /// Descrizione estesa.
        #[serde(default)]
        descrizione_estesa: Option<String>,
        /// Unità di misura.
        unita_misura: String,
        /// Prezzo unitario.
        prezzo: Prezzo,
        /// Natura (lavoro o oneri della sicurezza).
        #[serde(default)]
        natura: Option<NaturaVoce>,
        /// Incidenza della manodopera in percentuale.
        #[serde(default)]
        incidenza_manodopera: Option<Percentuale>,
        /// Incidenza della sicurezza in percentuale.
        #[serde(default)]
        incidenza_sicurezza: Option<Percentuale>,
        /// Codice del capitolo.
        #[serde(default)]
        capitolo: Option<String>,
    },
    /// Modifica una voce dell'elenco prezzi. Le voci di computo già inserite
    /// conservano il prezzo fotografato: si aggiornano con `riallinea_prezzo`.
    ModificaVocePrezzario {
        /// Codice della voce.
        codice: String,
        /// Nuovo codice.
        #[serde(default)]
        nuovo_codice: Option<String>,
        /// Descrizione breve.
        #[serde(default)]
        descrizione_breve: Option<String>,
        /// Descrizione estesa.
        #[serde(default)]
        descrizione_estesa: Option<String>,
        /// Unità di misura.
        #[serde(default)]
        unita_misura: Option<String>,
        /// Prezzo unitario.
        #[serde(default)]
        prezzo: Option<Prezzo>,
        /// Natura.
        #[serde(default)]
        natura: Option<NaturaVoce>,
        /// Incidenza della manodopera.
        #[serde(default)]
        incidenza_manodopera: Option<Percentuale>,
        /// Incidenza della sicurezza.
        #[serde(default)]
        incidenza_sicurezza: Option<Percentuale>,
    },
    /// Rimuove una voce dall'elenco prezzi (le voci di computo che la usano restano,
    /// con il prezzo fotografato; la validazione lo segnala).
    RimuoviVocePrezzario {
        /// Codice.
        codice: String,
    },
    /// Copia voci da un prezzario caricato nella sessione all'elenco prezzi,
    /// con i rispettivi capitoli.
    CopiaDaPrezzario {
        /// Nome del prezzario caricato.
        prezzario: String,
        /// Codici delle voci.
        codici: Vec<String>,
    },
    /// Aggiunge una categoria.
    AggiungiCategoria {
        /// Livello.
        livello: LivelloCategoria,
        /// Codice.
        codice: String,
        /// Descrizione.
        descrizione: String,
    },
    /// Aggiunge un nodo WBS.
    AggiungiNodoWbs {
        /// Codice del nodo (relativo al padre).
        codice: String,
        /// Descrizione.
        descrizione: String,
        /// Codice completo del padre (es. `01`).
        #[serde(default)]
        padre: Option<String>,
    },
    /// Modifica codice o descrizione di una categoria.
    ModificaCategoria {
        /// Livello.
        livello: LivelloCategoria,
        /// Codice attuale.
        codice: String,
        /// Nuovo codice.
        #[serde(default)]
        nuovo_codice: Option<String>,
        /// Nuova descrizione.
        #[serde(default)]
        descrizione: Option<String>,
    },
    /// Rimuove una categoria non assegnata a voci o registrazioni.
    RimuoviCategoria {
        /// Livello.
        livello: LivelloCategoria,
        /// Codice.
        codice: String,
    },
    /// Modifica codice (relativo al padre) o descrizione di un nodo WBS.
    ModificaNodoWbs {
        /// Codice completo attuale (es. `01.02`).
        codice: String,
        /// Nuovo codice relativo al padre.
        #[serde(default)]
        nuovo_codice: Option<String>,
        /// Nuova descrizione.
        #[serde(default)]
        descrizione: Option<String>,
    },
    /// Rimuove un nodo WBS senza figli e non assegnato a voci o registrazioni.
    RimuoviNodoWbs {
        /// Codice completo.
        codice: String,
    },
    /// Crea un computo.
    NuovoComputo {
        /// Nome (univoco).
        nome: String,
        /// Tipo.
        #[serde(default)]
        tipo: Option<TipoComputo>,
    },
    /// Aggiunge una voce al computo prendendo il prezzo dall'elenco prezzi.
    AggiungiVoceComputo {
        /// Computo (facoltativo se ce n'è uno solo).
        #[serde(default)]
        computo: Option<String>,
        /// Codice della voce di elenco prezzi.
        codice: String,
        /// Posizione (da 1); in coda se assente.
        #[serde(default)]
        posizione: Option<usize>,
        /// Righi di misura.
        #[serde(default)]
        righi: Vec<NuovoRigo>,
        /// Categorie.
        #[serde(default)]
        categorie: Option<CategorieIngresso>,
        /// Codice completo del nodo WBS.
        #[serde(default)]
        wbs: Option<String>,
        /// Note.
        #[serde(default)]
        note: Option<String>,
    },
    /// Modifica categorie, WBS o note di una voce di computo.
    ModificaVoceComputo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
        /// Categorie.
        #[serde(default)]
        categorie: Option<CategorieIngresso>,
        /// Codice completo del nodo WBS (stringa vuota = nessuno).
        #[serde(default)]
        wbs: Option<String>,
        /// Note (stringa vuota = nessuna).
        #[serde(default)]
        note: Option<String>,
    },
    /// Rimuove una voce di computo.
    RimuoviVoceComputo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
    },
    /// Sposta una voce di computo nella posizione indicata (da 1).
    SpostaVoceComputo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
        /// Nuova posizione, da 1.
        posizione: usize,
    },
    /// Copia (o sposta) voci di computo nello stesso computo o in un altro, con righi,
    /// prezzo fotografato, categorie, WBS e note.
    CopiaVociComputo {
        /// Computo di origine (facoltativo se ce n'è uno solo).
        #[serde(default)]
        da: Option<String>,
        /// Voci da copiare, nell'ordine del computo di origine.
        voci: Vec<RifVoce>,
        /// Computo di destinazione (assente: quello di origine).
        #[serde(default)]
        a: Option<String>,
        /// Posizione nella destinazione (da 1, contata prima della copia); in coda se
        /// assente.
        #[serde(default)]
        posizione: Option<usize>,
        /// Sposta invece di copiare: le voci lasciano l'origine e conservano il loro
        /// identificativo (e quindi i riferimenti del libretto).
        #[serde(default)]
        sposta: bool,
    },
    /// Aggiorna il prezzo fotografato di una voce di computo dall'elenco prezzi.
    RiallineaPrezzo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
    },
    /// Aggiunge un rigo di misura a una voce di computo.
    AggiungiRigo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
        /// Rigo.
        rigo: NuovoRigo,
        /// Posizione (da 1); in coda se assente.
        #[serde(default)]
        posizione: Option<usize>,
    },
    /// Sostituisce un rigo di misura.
    ModificaRigo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
        /// Numero del rigo, da 1.
        numero: usize,
        /// Nuovo contenuto.
        rigo: NuovoRigo,
    },
    /// Rimuove un rigo di misura.
    RimuoviRigo {
        /// Computo.
        #[serde(default)]
        computo: Option<String>,
        /// Voce.
        voce: RifVoce,
        /// Numero del rigo, da 1.
        numero: usize,
    },
    /// Registra una misurazione nel libretto delle misure.
    Registra {
        /// Data della misurazione.
        data: Date,
        /// Codice della voce di elenco prezzi.
        codice: String,
        /// Righi di misura.
        #[serde(default)]
        righi: Vec<NuovoRigo>,
        /// Annotazioni.
        #[serde(default)]
        annotazioni: Option<String>,
    },
    /// Sostituisce data e righi di una registrazione (non consentito se il SAL è emesso).
    ModificaRegistrazione {
        /// Numero della registrazione.
        numero: u32,
        /// Data.
        #[serde(default)]
        data: Option<Date>,
        /// Righi (sostituiscono quelli esistenti).
        #[serde(default)]
        righi: Option<Vec<NuovoRigo>>,
        /// Annotazioni (stringa vuota = nessuna).
        #[serde(default)]
        annotazioni: Option<String>,
    },
    /// Rimuove una registrazione (non consentito se appartiene a un SAL).
    RimuoviRegistrazione {
        /// Numero.
        numero: u32,
    },
    /// Crea un SAL in bozza con le registrazioni non ancora contabilizzate fino alla data.
    NuovoSal {
        /// Lavori a tutto il.
        lavori_a_tutto_il: Date,
    },
    /// Annulla un SAL in bozza.
    AnnullaBozzaSal {
        /// Numero del SAL.
        numero: u32,
    },
    /// Emette un SAL: il prospetto viene congelato e le registrazioni bloccate.
    EmettiSal {
        /// Numero del SAL.
        numero: u32,
        /// Data di emissione.
        data_emissione: Date,
    },
    /// Redige il conto finale in bozza (con le registrazioni non comprese nei SAL come
    /// stato finale) o ne aggiorna date e note.
    RedigiContoFinale {
        /// Data di ultimazione dei lavori (obbligatoria alla prima redazione).
        #[serde(default)]
        data_ultimazione: Option<Date>,
        /// Data del conto finale (predefinita: quella di ultimazione).
        #[serde(default)]
        data: Option<Date>,
        /// Note o relazione (`null` le rimuove).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        note: Option<Option<String>>,
    },
    /// Emette il conto finale: riepilogo e rata di saldo congelati, contabilità chiusa.
    EmettiContoFinale {
        /// Data di emissione (predefinita: quella del conto).
        #[serde(default)]
        data: Option<Date>,
    },
    /// Annulla il conto finale in bozza.
    AnnullaContoFinale {},
    /// Firma dell'esecutore sul conto finale emesso (`data` assente: firma rimossa).
    FirmaContoFinale {
        /// Data della firma.
        #[serde(default)]
        data: Option<Date>,
        /// Riserve confermate.
        #[serde(default)]
        riserve: Option<String>,
    },
    /// Registra il collaudo o il certificato di regolare esecuzione.
    RegistraCollaudo {
        /// Tipo (`collaudo` o `regolare_esecuzione`).
        #[serde(default)]
        tipo: Option<TipoCollaudo>,
        /// Data del certificato.
        #[serde(default)]
        data: Option<Date>,
        /// Data di approvazione della stazione appaltante.
        #[serde(default)]
        approvato_il: Option<Date>,
        /// Rimuove il collaudo registrato.
        #[serde(default)]
        rimuovi: bool,
    },
    /// Aggiunge una risorsa elementare all'elenco prezzi o modifica quella con il codice
    /// indicato (le analisi usano subito il nuovo prezzo; le voci si aggiornano con
    /// `ricalcola_prezzi_analisi`).
    ImpostaRisorsa {
        /// Codice.
        codice: String,
        /// Nuovo codice (solo modifica).
        #[serde(default)]
        nuovo_codice: Option<String>,
        /// Descrizione.
        #[serde(default)]
        descrizione: Option<String>,
        /// Unità di misura (obbligatoria per una risorsa nuova).
        #[serde(default)]
        unita_misura: Option<String>,
        /// Tipo: manodopera, materiale, nolo, trasporto, altro.
        #[serde(default)]
        tipo: Option<TipoRisorsa>,
        /// Prezzo unitario (obbligatorio per una risorsa nuova).
        #[serde(default)]
        prezzo: Option<Prezzo>,
    },
    /// Rimuove una risorsa non usata nelle analisi.
    RimuoviRisorsa {
        /// Codice.
        codice: String,
    },
    /// Crea o modifica l'analisi del prezzo di una voce dell'elenco. Le righe indicate
    /// sostituiscono tutte le precedenti; con `applica_prezzo` (predefinito) il prezzo e
    /// l'incidenza della manodopera della voce sono aggiornati.
    ImpostaAnalisi {
        /// Codice della voce analizzata.
        voce: String,
        /// Quantità a cui si riferisce l'analisi (predefinita 1).
        #[serde(default)]
        quantita_riferimento: Option<FattoreIngresso>,
        /// Spese generali in percentuale (predefinite quelle dell'elenco).
        #[serde(default)]
        spese_generali: Option<Percentuale>,
        /// Utile d'impresa in percentuale (predefinito quello dell'elenco).
        #[serde(default)]
        utile_impresa: Option<Percentuale>,
        /// Base dell'utile: costo_piu_spese_generali o solo_costo.
        #[serde(default)]
        base_utile: Option<BaseUtile>,
        /// Variabili usabili nelle quantità (sostituiscono le precedenti).
        #[serde(default)]
        variabili: Option<Vec<VariabileIngresso>>,
        /// Quota delle spese generali costituita dagli oneri aziendali della sicurezza
        /// (`null` la toglie).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        quota_oneri_sicurezza: Option<Option<Percentuale>>,
        /// Righe: risorsa, voce dell'elenco o riga libera, con la quantità (numero o
        /// espressione con le variabili e `q1`, `q2`… delle righe precedenti).
        #[serde(default)]
        righe: Option<Vec<RigaAnalisiIngresso>>,
        /// Aggiorna il prezzo della voce (predefinito `true`).
        #[serde(default = "vero")]
        applica_prezzo: bool,
    },
    /// Rimuove l'analisi di una voce (il prezzo resta quello attuale).
    RimuoviAnalisi {
        /// Codice della voce.
        voce: String,
    },
    /// Applica a tutte le voci analizzate il prezzo calcolato con i prezzi correnti.
    RicalcolaPrezziAnalisi {},
    /// Spese generali e utile predefiniti per le analisi dell'elenco.
    ImpostaParametriAnalisi {
        /// Spese generali in percentuale.
        #[serde(default)]
        spese_generali: Option<Percentuale>,
        /// Utile d'impresa in percentuale.
        #[serde(default)]
        utile_impresa: Option<Percentuale>,
        /// Base dell'utile.
        #[serde(default)]
        base_utile: Option<BaseUtile>,
        /// Quota degli oneri aziendali della sicurezza nelle spese generali (`null` la
        /// toglie).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        quota_oneri_sicurezza: Option<Option<Percentuale>>,
        /// Applica i valori anche a tutte le analisi esistenti.
        #[serde(default)]
        applica_a_tutte: bool,
    },
    /// Aggiorna i prezzi di risorse o voci dell'elenco con una variazione percentuale
    /// (adeguamento a un prezzario più recente, a un indice…). Le voci con analisi non
    /// sono toccate: con `ricalcola_analisi` (predefinito) seguono le risorse.
    AggiornaPrezzi {
        /// `risorse` o `voci`.
        oggetto: OggettoAggiornamento,
        /// Variazione in percentuale, anche negativa (es. `"3.5"`).
        percentuale: Decimal,
        /// Solo le risorse di questo tipo.
        #[serde(default)]
        tipo: Option<TipoRisorsa>,
        /// Solo questi codici.
        #[serde(default)]
        codici: Option<Vec<String>>,
        /// Solo le voci di questo capitolo (e dei suoi sottocapitoli).
        #[serde(default)]
        capitolo: Option<String>,
        /// Aggiorna poi i prezzi delle voci analizzate (predefinito `true`).
        #[serde(default = "vero")]
        ricalcola_analisi: bool,
    },
    /// Imposta i dati generali del cronoprogramma (ADR 0024): inizio dei lavori, livello
    /// di dettaglio della WBS, computo degli importi, calendario. `null` toglie un campo
    /// facoltativo.
    ImpostaCronoprogramma {
        /// Inizio dei lavori (`null`: la consegna dei lavori del contratto).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        inizio_lavori: Option<Option<Date>>,
        /// Livello di dettaglio della WBS (1 = radici; `null`: tutti i livelli).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        livello_dettaglio: Option<Option<u32>>,
        /// Computo degli importi (`null`: il primo).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        computo: Option<Option<String>>,
        /// Giorni lavorativi da lunedì a domenica.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        giorni_lavorativi: Option<[bool; 7]>,
        /// Festività nazionali non lavorative.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        festivita_nazionali: Option<bool>,
        /// Chiusure del cantiere (sostituiscono le precedenti).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chiusure: Option<Vec<Chiusura>>,
    },
    /// Imposta i dati di un'attività del cronoprogramma: un nodo WBS (per codice) o
    /// un'attività aggiunta (per identificativo). `null` toglie un campo facoltativo.
    ImpostaAttivita {
        /// Attività.
        attivita: RifAttivitaComando,
        /// Durata in giorni lavorativi (`null`: da stimare, vale 1 giorno).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        durata: Option<Option<u32>>,
        /// Vincolo «inizio non prima del».
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        inizio_non_prima: Option<Option<Date>>,
        /// Avanzamento in percentuale (0–100).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avanzamento: Option<u8>,
        /// Note.
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        note: Option<Option<String>>,
        /// Nome (solo attività aggiunte).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nome: Option<String>,
        /// Pietra miliare (solo attività aggiunte).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pietra_miliare: Option<bool>,
        /// Nodo WBS sotto cui sta (solo attività aggiunte; `null`: alla radice).
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        padre: Option<Option<String>>,
    },
    /// Aggiunge al cronoprogramma un'attività senza voci (fase, allestimento,
    /// collaudo…) o una pietra miliare, sotto un nodo WBS o alla radice.
    AggiungiAttivita {
        /// Nome.
        nome: String,
        /// Codice del nodo WBS sotto cui sta (assente: alla radice).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        padre: Option<String>,
        /// Pietra miliare (durata zero).
        #[serde(default)]
        pietra_miliare: bool,
        /// Durata in giorni lavorativi.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        durata: Option<u32>,
    },
    /// Toglie un'attività aggiunta e i suoi legami.
    RimuoviAttivita {
        /// Identificativo.
        attivita: AttivitaId,
    },
    /// Collega due attività (se il legame c'è già, ne cambia tipo e ritardo). I legami
    /// che formerebbero un ciclo sono rifiutati.
    CollegaAttivita {
        /// Predecessore.
        da: RifAttivitaComando,
        /// Successore.
        a: RifAttivitaComando,
        /// Tipo (predefinito `fine_inizio`).
        #[serde(default)]
        tipo: TipoLegame,
        /// Ritardo in giorni lavorativi, anche negativo.
        #[serde(default)]
        ritardo: i32,
    },
    /// Toglie il legame tra due attività.
    ScollegaAttivita {
        /// Predecessore.
        da: RifAttivitaComando,
        /// Successore.
        a: RifAttivitaComando,
    },
    /// Salva la linea di base del cronoprogramma (date correnti di tutte le attività
    /// visibili) o la toglie.
    LineaBaseCronoprogramma {
        /// Giorno del salvataggio (obbligatorio per salvare).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<Date>,
        /// Toglie la linea di base.
        #[serde(default)]
        cancella: bool,
    },
    /// Aggiorna voci e risorse dell'elenco da un prezzario caricato (nuovo listino):
    /// solo i codici e i campi scelti dopo il confronto (richiesta
    /// `confronto_prezzario`). Ricalcola le voci analizzate e riallinea le voci dei
    /// computi collegate alle voci cambiate (ADR 0022).
    AggiornaDaPrezzario {
        /// Nome del prezzario caricato.
        prezzario: String,
        /// Codici (nell'elenco) delle voci da aggiornare.
        #[serde(default)]
        voci: Vec<String>,
        /// Codici delle risorse da aggiornare.
        #[serde(default)]
        risorse: Vec<String>,
        /// Campi da aggiornare (predefiniti: prezzo e incidenze).
        #[serde(default)]
        campi: CampiAggiornamento,
        /// Sostituzione del prefisso dei codici nel listino (es. `TOS23_` → `TOS24_`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prefisso: Option<SostituzionePrefisso>,
        /// Ricalcola le voci analizzate (predefinito `true`).
        #[serde(default = "vero")]
        ricalcola_analisi: bool,
        /// Riallinea le voci dei computi collegate alle voci cambiate (predefinito
        /// `true`).
        #[serde(default = "vero")]
        riallinea_computi: bool,
    },
    /// Segna una voce dell'elenco come nuovo prezzo (numerato NP.1, NP.2…) o ne modifica
    /// i dati. Le registrazioni non ancora in un SAL emesso seguono la soggezione al
    /// ribasso.
    ImpostaNuovoPrezzo {
        /// Codice della voce.
        voce: String,
        /// Data di formazione (obbligatoria per un nuovo prezzo nuovo).
        #[serde(default)]
        data: Option<Date>,
        /// Motivo.
        #[serde(default)]
        motivo: Option<String>,
        /// Criterio: prezzario, analogia, analisi (predefinito: analisi se la voce ha
        /// un'analisi, altrimenti prezzario).
        #[serde(default)]
        criterio: Option<CriterioNuovoPrezzo>,
        /// Soggetto al ribasso d'asta (predefinito `true`).
        #[serde(default)]
        soggetto_a_ribasso: Option<bool>,
        /// Toglie l'indicazione di nuovo prezzo.
        #[serde(default)]
        rimuovi: bool,
    },
    /// Crea (senza numero) o modifica un verbale di concordamento dei nuovi prezzi.
    /// `nuovi_prezzi` indica i codici delle voci compresi nel verbale; `null` toglie un
    /// campo facoltativo.
    ImpostaVerbaleNuoviPrezzi {
        /// Numero del verbale da modificare.
        #[serde(default)]
        numero: Option<u32>,
        /// Data del verbale (obbligatoria per un verbale nuovo).
        #[serde(default)]
        data: Option<Date>,
        /// Codici delle voci (nuovi prezzi) compresi.
        #[serde(default)]
        nuovi_prezzi: Option<Vec<String>>,
        /// Data della firma dell'esecutore.
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        firma_esecutore: Option<Option<Date>>,
        /// Riserve dell'esecutore.
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        riserve: Option<Option<String>>,
        /// Data dell'approvazione.
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        approvato_il: Option<Option<Date>>,
        /// Estremi dell'atto di approvazione.
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        atto_approvazione: Option<Option<String>>,
        /// Note.
        #[serde(
            default,
            deserialize_with = "opzione_doppia",
            skip_serializing_if = "Option::is_none"
        )]
        note: Option<Option<String>>,
    },
    /// Rimuove un verbale non approvato; i suoi nuovi prezzi restano senza verbale.
    RimuoviVerbaleNuoviPrezzi {
        /// Numero.
        numero: u32,
    },
}

/// Oggetto dell'aggiornamento dei prezzi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OggettoAggiornamento {
    /// Risorse elementari.
    Risorse,
    /// Voci dell'elenco prezzi senza analisi.
    Voci,
}

/// Variabile di un'analisi in ingresso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariabileIngresso {
    /// Nome (lettere minuscole, cifre e `_`).
    pub nome: String,
    /// Valore o espressione (può usare le variabili precedenti).
    pub valore: FattoreIngresso,
    /// Descrizione.
    #[serde(default)]
    pub descrizione: String,
}

/// Riga di analisi in ingresso: `risorsa` (codice), `voce` (codice dell'elenco) oppure
/// riga libera con codice, descrizione, unità di misura, tipo e prezzo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigaAnalisiIngresso {
    /// Codice di una risorsa elementare.
    #[serde(default)]
    pub risorsa: Option<String>,
    /// Codice di una voce dell'elenco (anche analizzata: sotto-analisi).
    #[serde(default)]
    pub voce: Option<String>,
    /// Codice della riga libera.
    #[serde(default)]
    pub codice: Option<String>,
    /// Descrizione (riga libera, o per sostituire quella copiata).
    #[serde(default)]
    pub descrizione: Option<String>,
    /// Unità di misura della riga libera.
    #[serde(default)]
    pub unita_misura: Option<String>,
    /// Tipo (predefinito: quello della risorsa, materiale per voci e righe libere).
    #[serde(default)]
    pub tipo: Option<TipoRisorsa>,
    /// Prezzo della riga libera.
    #[serde(default)]
    pub prezzo: Option<Prezzo>,
    /// Quantità: numero o espressione (es. `"0,5*1,2"`).
    pub quantita: FattoreIngresso,
}

fn vero() -> bool {
    true
}

/// Esito di un singolo comando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EsitoComando {
    /// Indice del comando nella richiesta (da 0).
    pub indice: usize,
    /// Descrizione di ciò che è stato fatto.
    pub descrizione: String,
}

pub(crate) fn non_trovato(cosa: &str, rif: &str) -> String {
    format!("{cosa} «{rif}» inesistente")
}

/// Computo per nome; senza nome, l'unico computo del progetto.
fn computo_mut<'a>(p: &'a mut Progetto, nome: Option<&str>) -> Result<&'a mut Computo, String> {
    match nome {
        Some(n) => p
            .computi
            .iter_mut()
            .find(|c| c.nome == n)
            .ok_or_else(|| non_trovato("computo", n)),
        None => match p.computi.len() {
            1 => p
                .computi
                .first_mut()
                .ok_or_else(|| "nessun computo".to_owned()),
            0 => Err("il progetto non ha computi: crearne uno con `nuovo_computo`".into()),
            n => Err(format!(
                "il progetto ha {n} computi: indicare `computo` per nome"
            )),
        },
    }
}

/// Computo per nome (facoltativo se il progetto ne ha uno solo), in lettura.
fn computo_rif<'a>(p: &'a Progetto, nome: Option<&str>) -> Result<&'a Computo, String> {
    match nome {
        Some(n) => p
            .computi
            .iter()
            .find(|c| c.nome == n)
            .ok_or_else(|| non_trovato("computo", n)),
        None => match p.computi.as_slice() {
            [c] => Ok(c),
            [] => Err("il progetto non ha computi: crearne uno con `nuovo_computo`".into()),
            molti => Err(format!(
                "il progetto ha {} computi: indicare `computo` per nome",
                molti.len()
            )),
        },
    }
}

fn indice_voce(c: &Computo, voce: RifVoce) -> Result<usize, String> {
    match voce {
        RifVoce::Numero(n) if n >= 1 && n <= c.voci.len() => Ok(n - 1),
        RifVoce::Numero(n) => Err(format!(
            "voce n. {n} inesistente nel computo «{}» ({} voci)",
            c.nome,
            c.voci.len()
        )),
        RifVoce::Id(id) => c
            .voci
            .iter()
            .position(|v| v.id == id)
            .ok_or_else(|| format!("voce {id} inesistente nel computo «{}»", c.nome)),
    }
}

fn voce_mut(c: &mut Computo, voce: RifVoce) -> Result<&mut VoceComputo, String> {
    let i = indice_voce(c, voce)?;
    c.voci
        .get_mut(i)
        .map(Arc::make_mut)
        .ok_or_else(|| "voce inesistente".to_owned())
}

fn assegna_categorie(
    p: &Progetto,
    ingresso: &CategorieIngresso,
    base: AssegnazioneCategorie,
) -> Result<AssegnazioneCategorie, String> {
    let mut a = base;
    for (livello, codice) in [
        (LivelloCategoria::SuperCategoria, &ingresso.super_categoria),
        (LivelloCategoria::Categoria, &ingresso.categoria),
        (LivelloCategoria::SottoCategoria, &ingresso.sotto_categoria),
    ] {
        let Some(codice) = codice else { continue };
        let id = if codice.is_empty() {
            None
        } else {
            Some(
                p.categorie
                    .elenco(livello)
                    .iter()
                    .find(|c| &c.codice == codice)
                    .map(|c| c.id)
                    .ok_or_else(|| non_trovato(&livello.to_string(), codice))?,
            )
        };
        match livello {
            LivelloCategoria::SuperCategoria => a.super_categoria = id,
            LivelloCategoria::Categoria => a.categoria = id,
            LivelloCategoria::SottoCategoria => a.sotto_categoria = id,
        }
    }
    Ok(a)
}

fn classi_mut(c: &mut Categorie, livello: LivelloCategoria) -> &mut Vec<Classe> {
    match livello {
        LivelloCategoria::SuperCategoria => &mut c.super_categorie,
        LivelloCategoria::Categoria => &mut c.categorie,
        LivelloCategoria::SottoCategoria => &mut c.sotto_categorie,
    }
}

/// Voci di computo e registrazioni a cui è assegnata la categoria.
fn usi_categoria(p: &Progetto, livello: LivelloCategoria, id: ClasseId) -> usize {
    let voci = p
        .computi
        .iter()
        .flat_map(|c| &c.voci)
        .filter(|v| v.categorie.di(livello) == Some(id))
        .count();
    let registrazioni = p
        .contabilita
        .libretto()
        .registrazioni
        .iter()
        .filter(|r| r.categorie.di(livello) == Some(id))
        .count();
    voci + registrazioni
}

pub(crate) fn nodo_wbs(
    p: &Progetto,
    codice: &str,
) -> Result<Option<opencomputo_core::id::WbsNodoId>, String> {
    if codice.is_empty() {
        return Ok(None);
    }
    for n in &p.wbs.nodi {
        if p.wbs.codice_completo(n.id).map_err(|e| e.to_string())? == codice {
            return Ok(Some(n.id));
        }
    }
    Err(non_trovato("nodo WBS", codice))
}

fn testo_opz(v: &Option<String>) -> Option<Option<String>> {
    v.as_ref().map(|s| {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.trim().to_owned())
        }
    })
}

fn posizione_righi(n: Option<usize>, len: usize) -> Result<usize, String> {
    match n {
        None => Ok(len),
        Some(p) if p >= 1 && p <= len + 1 => Ok(p - 1),
        Some(p) => Err(format!("posizione {p} non valida (righi: {len})")),
    }
}

pub(crate) fn voce_elenco<'a>(p: &'a Progetto, codice: &str) -> Result<&'a VocePrezzario, String> {
    p.elenco_prezzi
        .voce_per_codice(codice)
        .ok_or_else(|| non_trovato("voce di elenco prezzi", codice))
}

/// Applica un comando al progetto. In caso di errore il progetto può essere
/// parzialmente modificato: la [`Sessione`](crate::Sessione) applica sempre i comandi
/// su una copia.
///
/// # Errors
/// Messaggio leggibile che spiega perché il comando non è applicabile.
#[allow(clippy::too_many_lines)]
pub fn applica<G: GeneratoreId>(
    p: &mut Progetto,
    comando: &Comando,
    g: &mut G,
    prezzari: &[(String, Prezzario)],
) -> Result<String, String> {
    match comando {
        Comando::ImpostaMetadati {
            titolo,
            oggetto,
            committente,
            impresa,
            luogo,
            cig,
            cup,
            responsabile_procedimento,
            progettista,
            direttore_lavori,
        } => {
            let m = &mut p.metadati;
            if let Some(t) = titolo {
                if t.trim().is_empty() {
                    return Err("il titolo non può essere vuoto".into());
                }
                m.titolo = t.trim().to_owned();
            }
            for (campo, valore) in [
                (&mut m.oggetto, oggetto),
                (&mut m.committente, committente),
                (&mut m.impresa, impresa),
                (&mut m.luogo, luogo),
                (&mut m.cig, cig),
                (&mut m.cup, cup),
                (&mut m.responsabile_procedimento, responsabile_procedimento),
                (&mut m.progettista, progettista),
                (&mut m.direttore_lavori, direttore_lavori),
            ] {
                if let Some(v) = testo_opz(valore) {
                    *campo = v;
                }
            }
            Ok("metadati aggiornati".into())
        }
        Comando::ImpostaContratto {
            ribasso,
            ritenuta_garanzia,
            manodopera_esclusa_dal_ribasso,
            importo_contrattuale,
            data_consegna_lavori,
            aliquota_iva,
        } => {
            let c = p.contratto.get_or_insert_with(DatiContratto::default);
            if let Some(a) = aliquota_iva {
                c.parametri.aliquota_iva = *a;
            }
            if let Some(r) = ribasso {
                c.parametri.ribasso = *r;
            }
            if let Some(r) = ritenuta_garanzia {
                c.parametri.ritenuta_garanzia = *r;
            }
            if let Some(m) = manodopera_esclusa_dal_ribasso {
                c.parametri.manodopera_esclusa_dal_ribasso = *m;
            }
            if let Some(i) = importo_contrattuale {
                c.importo_contrattuale = Some(*i);
            }
            if let Some(d) = data_consegna_lavori {
                c.data_consegna_lavori = Some(*d);
            }
            Ok("dati contrattuali aggiornati".into())
        }
        Comando::ImpostaAnticipazione {
            percentuale,
            importo,
            percentuale_recupero,
            data_erogazione,
            rimuovi,
        } => {
            let regole = p.regole.clone();
            let c = p.contratto.get_or_insert_with(DatiContratto::default);
            if *rimuovi {
                c.parametri.anticipazione = None;
                return Ok("anticipazione rimossa".into());
            }
            let attuale = c.parametri.anticipazione.clone();
            let perc = percentuale
                .or(attuale.as_ref().map(|a| a.percentuale))
                .unwrap_or_else(|| Percentuale::new(Decimal::new(20, 0)));
            let importo = match (importo, &attuale) {
                (Some(i), _) => *i,
                (None, Some(a)) if percentuale.is_none() => a.importo,
                _ => {
                    let contrattuale = c.importo_contrattuale.ok_or(
                        "indicare l'importo dell'anticipazione oppure l'importo contrattuale",
                    )?;
                    perc.quota_di(contrattuale, &regole)
                        .map_err(|e| e.to_string())?
                }
            };
            let recupero = percentuale_recupero
                .or(attuale.as_ref().map(|a| a.percentuale_recupero))
                .unwrap_or(perc);
            c.parametri.anticipazione = Some(Anticipazione {
                percentuale: perc,
                importo,
                percentuale_recupero: recupero,
                data_erogazione: data_erogazione.or(attuale.and_then(|a| a.data_erogazione)),
            });
            Ok(format!(
                "anticipazione di {} € impostata",
                crate::formato_importo(importo)
            ))
        }
        Comando::RegistraPenale {
            data,
            descrizione,
            importo,
            giorni,
            per_mille,
        } => {
            let importo = match (importo, giorni, per_mille) {
                (Some(i), _, _) => *i,
                (None, Some(g), Some(pm)) => {
                    let contrattuale = p
                        .contratto
                        .as_ref()
                        .and_then(|c| c.importo_contrattuale)
                        .ok_or("per calcolare la penale serve l'importo contrattuale")?;
                    let valore = contrattuale
                        .valore()
                        .checked_mul(*pm)
                        .and_then(|v| v.checked_mul(Decimal::from(*g)))
                        .and_then(|v| v.checked_div(Decimal::ONE_THOUSAND))
                        .ok_or("importo della penale fuori scala")?;
                    p.regole.arrotonda_importo(valore)
                }
                _ => return Err("indicare l'importo oppure giorni e per mille".into()),
            };
            let numero = p.contabilita.prossimo_numero_penale();
            p.contabilita
                .registra_penale(Penale {
                    id: g.nuovo(),
                    numero,
                    data: *data,
                    descrizione: descrizione.clone(),
                    importo,
                    giorni: *giorni,
                    per_mille: *per_mille,
                })
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "penale n. {numero} di {} € registrata",
                crate::formato_importo(importo)
            ))
        }
        Comando::RimuoviPenale { numero } => {
            let id = p
                .contabilita
                .penali()
                .iter()
                .find(|x| x.numero == *numero)
                .map(|x| x.id)
                .ok_or_else(|| format!("penale n. {numero} inesistente"))?;
            p.contabilita
                .rimuovi_penale(id)
                .map_err(|e| e.to_string())?;
            Ok(format!("penale n. {numero} rimossa"))
        }
        Comando::ImpostaSal {
            numero,
            recupero_anticipazione_sospeso,
            recupera_residuo_anticipazione,
        } => {
            let id = sal(p, *numero)?;
            p.contabilita
                .imposta_recupero_anticipazione(
                    id,
                    *recupero_anticipazione_sospeso,
                    *recupera_residuo_anticipazione,
                )
                .map_err(|e| e.to_string())?;
            Ok(format!("certificato del SAL n. {numero} aggiornato"))
        }
        Comando::ImpostaRegole {
            decimali_quantita,
            decimali_prezzo,
            decimali_importo,
            modalita,
        } => {
            let r = &mut p.regole;
            for (campo, valore) in [
                (&mut r.decimali_quantita, decimali_quantita),
                (&mut r.decimali_prezzo, decimali_prezzo),
                (&mut r.decimali_importo, decimali_importo),
            ] {
                if let Some(v) = valore {
                    *campo = *v;
                }
            }
            if let Some(m) = modalita {
                r.modalita = *m;
            }
            // Fuori dai limiti nessun importo sarebbe calcolabile: rifiutato con ogni
            // politica.
            r.valida().map_err(|e| e.to_string())?;
            Ok("regole di arrotondamento aggiornate".into())
        }
        Comando::AggiungiCapitolo {
            codice,
            descrizione,
            padre,
        } => {
            let ep = &mut p.elenco_prezzi;
            if ep.capitoli.iter().any(|c| &c.codice == codice) {
                return Err(format!("capitolo «{codice}» già presente"));
            }
            let padre = match padre {
                Some(pc) => Some(
                    ep.capitoli
                        .iter()
                        .find(|c| &c.codice == pc)
                        .map(|c| c.id)
                        .ok_or_else(|| non_trovato("capitolo", pc))?,
                ),
                None => None,
            };
            ep.aggiungi_capitolo(CapitoloPrezzario {
                id: g.nuovo(),
                padre,
                codice: codice.clone(),
                descrizione: descrizione.clone(),
            })
            .map_err(|e| e.to_string())?;
            Ok(format!("capitolo «{codice}» aggiunto"))
        }
        Comando::AggiungiVocePrezzario {
            codice,
            descrizione_breve,
            descrizione_estesa,
            unita_misura,
            prezzo,
            natura,
            incidenza_manodopera,
            incidenza_sicurezza,
            capitolo,
        } => {
            let breve = descrizione_breve
                .clone()
                .or_else(|| descrizione_estesa.clone())
                .unwrap_or_default();
            let mut v = VocePrezzario::new(
                g.nuovo(),
                codice.clone(),
                breve.clone(),
                unita_misura.clone(),
                *prezzo,
            );
            v.descrizione_estesa = descrizione_estesa.clone().unwrap_or(breve);
            v.natura = natura.unwrap_or_default();
            v.incidenza_manodopera = *incidenza_manodopera;
            v.incidenza_sicurezza = *incidenza_sicurezza;
            if let Some(c) = capitolo {
                v.capitolo = Some(
                    p.elenco_prezzi
                        .capitoli
                        .iter()
                        .find(|x| &x.codice == c)
                        .map(|x| x.id)
                        .ok_or_else(|| non_trovato("capitolo", c))?,
                );
            }
            p.elenco_prezzi
                .aggiungi_voce(v)
                .map_err(|e| e.to_string())?;
            Ok(format!("voce «{codice}» aggiunta all'elenco prezzi"))
        }
        Comando::ModificaVocePrezzario {
            codice,
            nuovo_codice,
            descrizione_breve,
            descrizione_estesa,
            unita_misura,
            prezzo,
            natura,
            incidenza_manodopera,
            incidenza_sicurezza,
        } => {
            if let Some(n) = nuovo_codice
                && n != codice
                && p.elenco_prezzi.voce_per_codice(n).is_some()
            {
                return Err(format!("codice «{n}» già usato da un'altra voce"));
            }
            let id = voce_elenco(p, codice)?.id;
            let v = p
                .elenco_prezzi
                .voce_mut(id)
                .ok_or_else(|| non_trovato("voce di elenco prezzi", codice))?;
            if let Some(n) = nuovo_codice {
                v.codice.clone_from(n);
            }
            if let Some(d) = descrizione_breve {
                v.descrizione_breve.clone_from(d);
            }
            if let Some(d) = descrizione_estesa {
                v.descrizione_estesa.clone_from(d);
            }
            if let Some(u) = unita_misura {
                v.unita_misura.clone_from(u);
            }
            if let Some(x) = prezzo {
                v.prezzo = *x;
            }
            if let Some(x) = natura {
                v.natura = *x;
            }
            if let Some(x) = incidenza_manodopera {
                v.incidenza_manodopera = Some(*x);
            }
            if let Some(x) = incidenza_sicurezza {
                v.incidenza_sicurezza = Some(*x);
            }
            Ok(format!(
                "voce «{codice}» modificata nell'elenco prezzi (le voci di computo restano con il prezzo fotografato)"
            ))
        }
        Comando::RimuoviVocePrezzario { codice } => {
            let v = voce_elenco(p, codice)?;
            let (id, analisi) = (v.id, v.analisi);
            if let Some(a) = analisi {
                p.elenco_prezzi.analisi.retain(|x| x.id != a);
            }
            p.elenco_prezzi.voci.retain(|v| v.id != id);
            Ok(format!("voce «{codice}» rimossa dall'elenco prezzi"))
        }
        Comando::AggiornaDaPrezzario {
            prezzario,
            voci,
            risorse,
            campi,
            prefisso,
            ricalcola_analisi,
            riallinea_computi,
        } => {
            let (_, listino) = prezzari
                .iter()
                .find(|(n, _)| n == prezzario)
                .ok_or_else(|| non_trovato("prezzario caricato", prezzario))?;
            let esito = aggiorna_da_listino(
                p,
                listino,
                &SceltaAggiornamento {
                    voci,
                    risorse,
                    campi: *campi,
                    prefisso: prefisso.as_ref(),
                    ricalcola_analisi: *ricalcola_analisi,
                    riallinea_computi: *riallinea_computi,
                },
            )
            .map_err(|e| e.to_string())?;
            let quante = |n: usize, uno: &str, piu: &str| {
                if n == 1 {
                    format!("1 {uno}")
                } else {
                    format!("{n} {piu}")
                }
            };
            let mut parti = Vec::new();
            if esito.voci > 0 {
                parti.push(quante(esito.voci, "voce aggiornata", "voci aggiornate"));
            }
            if esito.risorse > 0 {
                parti.push(quante(
                    esito.risorse,
                    "risorsa aggiornata",
                    "risorse aggiornate",
                ));
            }
            if esito.analisi_ricalcolate > 0 {
                parti.push(quante(
                    esito.analisi_ricalcolate,
                    "voce analizzata ricalcolata",
                    "voci analizzate ricalcolate",
                ));
            }
            let riallineate: usize = esito.riallineate.values().sum();
            if riallineate > 0 {
                parti.push(quante(
                    riallineate,
                    "voce di computo riallineata",
                    "voci di computo riallineate",
                ));
            }
            Ok(format!("dal prezzario «{prezzario}»: {}", parti.join(", ")))
        }
        Comando::CopiaDaPrezzario { prezzario, codici } => {
            let (_, sorgente) = prezzari
                .iter()
                .find(|(n, _)| n == prezzario)
                .ok_or_else(|| non_trovato("prezzario caricato", prezzario))?;
            let mut copiate = 0usize;
            for codice in codici {
                let v = sorgente.voce_per_codice(codice).ok_or_else(|| {
                    format!("voce «{codice}» assente nel prezzario «{prezzario}»")
                })?;
                if p.elenco_prezzi.voce_per_codice(codice).is_some() {
                    return Err(format!("voce «{codice}» già presente nell'elenco prezzi"));
                }
                // Capitolo con lo stesso codice, creato se manca.
                let capitolo = match v
                    .capitolo
                    .and_then(|c| sorgente.capitoli.iter().find(|x| x.id == c))
                {
                    Some(cap) => match p
                        .elenco_prezzi
                        .capitoli
                        .iter()
                        .find(|x| x.codice == cap.codice)
                    {
                        Some(x) => Some(x.id),
                        None => {
                            let id = g.nuovo();
                            p.elenco_prezzi
                                .aggiungi_capitolo(CapitoloPrezzario {
                                    id,
                                    padre: None,
                                    codice: cap.codice.clone(),
                                    descrizione: cap.descrizione.clone(),
                                })
                                .map_err(|e| e.to_string())?;
                            Some(id)
                        }
                    },
                    None => None,
                };
                let mut nuova = v.clone();
                nuova.id = g.nuovo();
                nuova.capitolo = capitolo;
                nuova.analisi = None;
                p.elenco_prezzi
                    .aggiungi_voce(nuova)
                    .map_err(|e| e.to_string())?;
                copiate += 1;
            }
            Ok(format!(
                "{copiate} voci copiate dal prezzario «{prezzario}»"
            ))
        }
        Comando::AggiungiCategoria {
            livello,
            codice,
            descrizione,
        } => {
            p.categorie
                .aggiungi(
                    *livello,
                    Classe {
                        id: g.nuovo(),
                        codice: codice.clone(),
                        descrizione: descrizione.clone(),
                    },
                )
                .map_err(|e| e.to_string())?;
            Ok(format!("{livello} «{codice}» aggiunta"))
        }
        Comando::AggiungiNodoWbs {
            codice,
            descrizione,
            padre,
        } => {
            let padre = match padre {
                Some(c) => nodo_wbs(p, c)?,
                None => None,
            };
            p.wbs
                .aggiungi(NodoWbs {
                    id: g.nuovo(),
                    padre,
                    codice: codice.clone(),
                    descrizione: descrizione.clone(),
                })
                .map_err(|e| e.to_string())?;
            Ok(format!("nodo WBS «{codice}» aggiunto"))
        }
        Comando::ModificaCategoria {
            livello,
            codice,
            nuovo_codice,
            descrizione,
        } => {
            let elenco = classi_mut(&mut p.categorie, *livello);
            if let Some(n) = nuovo_codice {
                if n.trim().is_empty() {
                    return Err("il codice non può essere vuoto".into());
                }
                if n != codice && elenco.iter().any(|c| &c.codice == n) {
                    return Err(format!("{livello} «{n}» già presente"));
                }
            }
            let c = elenco
                .iter_mut()
                .find(|c| &c.codice == codice)
                .ok_or_else(|| format!("{livello} «{codice}» inesistente"))?;
            if let Some(n) = nuovo_codice {
                c.codice = n.trim().to_owned();
            }
            if let Some(d) = descrizione {
                c.descrizione = d.clone();
            }
            Ok(format!("{livello} «{codice}» modificata"))
        }
        Comando::RimuoviCategoria { livello, codice } => {
            let id = p
                .categorie
                .elenco(*livello)
                .iter()
                .find(|c| &c.codice == codice)
                .map(|c| c.id)
                .ok_or_else(|| format!("{livello} «{codice}» inesistente"))?;
            let usi = usi_categoria(p, *livello, id);
            if usi > 0 {
                return Err(format!(
                    "{livello} «{codice}» assegnata a {usi} voci o registrazioni: toglierla prima"
                ));
            }
            classi_mut(&mut p.categorie, *livello).retain(|c| c.id != id);
            Ok(format!("{livello} «{codice}» rimossa"))
        }
        Comando::ModificaNodoWbs {
            codice,
            nuovo_codice,
            descrizione,
        } => {
            let id = nodo_wbs(p, codice)?.ok_or("indicare il codice del nodo WBS")?;
            let padre = p.wbs.nodo(id).and_then(|n| n.padre);
            if let Some(n) = nuovo_codice {
                if n.trim().is_empty() || n.contains('.') {
                    return Err("il codice del nodo non può essere vuoto né contenere punti".into());
                }
                if p.wbs
                    .nodi
                    .iter()
                    .any(|x| x.id != id && x.padre == padre && &x.codice == n)
                {
                    return Err(format!("nodo WBS «{n}» già presente allo stesso livello"));
                }
            }
            let nodo = p
                .wbs
                .nodi
                .iter_mut()
                .find(|n| n.id == id)
                .ok_or_else(|| format!("nodo WBS «{codice}» inesistente"))?;
            if let Some(n) = nuovo_codice {
                nodo.codice = n.trim().to_owned();
            }
            if let Some(d) = descrizione {
                nodo.descrizione = d.clone();
            }
            Ok(format!("nodo WBS «{codice}» modificato"))
        }
        Comando::RimuoviNodoWbs { codice } => {
            let id = nodo_wbs(p, codice)?.ok_or("indicare il codice del nodo WBS")?;
            if p.wbs.nodi.iter().any(|n| n.padre == Some(id)) {
                return Err(format!(
                    "il nodo WBS «{codice}» ha nodi figli: rimuoverli prima"
                ));
            }
            let usi = p
                .computi
                .iter()
                .flat_map(|c| &c.voci)
                .filter(|v| v.wbs == Some(id))
                .count()
                + p.contabilita
                    .libretto()
                    .registrazioni
                    .iter()
                    .filter(|r| r.wbs == Some(id))
                    .count();
            if usi > 0 {
                return Err(format!(
                    "nodo WBS «{codice}» assegnato a {usi} voci o registrazioni: toglierlo prima"
                ));
            }
            let padre = p.wbs.nodo(id).and_then(|n| n.padre);
            p.wbs.nodi.retain(|n| n.id != id);
            p.cronoprogramma.rimuovi_nodo(id, padre);
            Ok(format!("nodo WBS «{codice}» rimosso"))
        }
        Comando::NuovoComputo { nome, tipo } => {
            if nome.trim().is_empty() {
                return Err("il nome del computo non può essere vuoto".into());
            }
            if p.computi.iter().any(|c| &c.nome == nome) {
                return Err(format!("computo «{nome}» già presente"));
            }
            let mut c = Computo::new(g.nuovo(), nome.clone());
            c.tipo = tipo.unwrap_or_default();
            p.aggiungi_computo(c).map_err(|e| e.to_string())?;
            Ok(format!("computo «{nome}» creato"))
        }
        Comando::AggiungiVoceComputo {
            computo,
            codice,
            posizione,
            righi,
            categorie,
            wbs,
            note,
        } => {
            let sorgente = voce_elenco(p, codice)?.clone();
            let assegnazione = match categorie {
                Some(c) => assegna_categorie(p, c, AssegnazioneCategorie::default())?,
                None => AssegnazioneCategorie::default(),
            };
            let nodo = match wbs {
                Some(w) => nodo_wbs(p, w)?,
                None => None,
            };
            let prezzo = PrezzoApplicato::da_voce(&p.elenco_prezzi, &sorgente);
            let mut voce = VoceComputo::new(g.nuovo(), prezzo);
            let nome = computo_rif(p, computo.as_deref())?.nome.clone();
            voce.righi = righi
                .iter()
                .map(|r| r.in_rigo_di(g, p, Some(&nome)))
                .collect::<Result<_, _>>()?;
            voce.categorie = assegnazione;
            voce.wbs = nodo;
            voce.note = note.clone().filter(|n| !n.trim().is_empty());
            let c = computo_mut(p, computo.as_deref())?;
            let pos = match posizione {
                None => None,
                Some(n) if *n >= 1 && *n <= c.voci.len() + 1 => Some(n - 1),
                Some(n) => {
                    return Err(format!("posizione {n} non valida (voci: {})", c.voci.len()));
                }
            };
            c.inserisci_voce(pos, voce).map_err(|e| e.to_string())?;
            let numero = pos.map_or(c.voci.len(), |x| x + 1);
            Ok(format!(
                "voce n. {numero} «{codice}» aggiunta al computo «{}»",
                c.nome
            ))
        }
        Comando::ModificaVoceComputo {
            computo,
            voce,
            categorie,
            wbs,
            note,
        } => {
            let nodo = match wbs {
                Some(w) => Some(nodo_wbs(p, w)?),
                None => None,
            };
            let attuale = {
                let c = computo_mut(p, computo.as_deref())?;
                voce_mut(c, *voce)?.categorie
            };
            let assegnazione = match categorie {
                Some(cat) => Some(assegna_categorie(p, cat, attuale)?),
                None => None,
            };
            let c = computo_mut(p, computo.as_deref())?;
            let v = voce_mut(c, *voce)?;
            if let Some(a) = assegnazione {
                v.categorie = a;
            }
            if let Some(n) = nodo {
                v.wbs = n;
            }
            if let Some(n) = testo_opz(note) {
                v.note = n;
            }
            Ok("voce di computo modificata".into())
        }
        Comando::RimuoviVoceComputo { computo, voce } => {
            let regole = p.regole.clone();
            let c = computo_mut(p, computo.as_deref())?;
            let i = indice_voce(c, *voce)?;
            let id = c.voci.get(i).map(|v| v.id).ok_or("voce inesistente")?;
            // I righi che la citano conservano la quantità attuale.
            let statici = rendi_statici_i_riferimenti(c, &BTreeSet::from([id]), &regole)?;
            let v = c.rimuovi_voce(id).map_err(|e| e.to_string())?;
            Ok(format!(
                "voce n. {} «{}» rimossa{}",
                i + 1,
                v.prezzo.codice,
                resi_statici(statici)
            ))
        }
        Comando::SpostaVoceComputo {
            computo,
            voce,
            posizione,
        } => {
            let c = computo_mut(p, computo.as_deref())?;
            let i = indice_voce(c, *voce)?;
            if *posizione < 1 || *posizione > c.voci.len() {
                return Err(format!(
                    "posizione {posizione} non valida (voci: {})",
                    c.voci.len()
                ));
            }
            let id = c.voci.get(i).map(|v| v.id).ok_or("voce inesistente")?;
            c.sposta_voce(id, posizione - 1)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "voce spostata dalla posizione {} alla {posizione}",
                i + 1
            ))
        }
        Comando::CopiaVociComputo {
            da,
            voci,
            a,
            posizione,
            sposta,
        } => {
            if voci.is_empty() {
                return Err("nessuna voce indicata".into());
            }
            let regole = p.regole.clone();
            let origine = computo_mut(p, da.as_deref())?;
            let nome_origine = origine.nome.clone();
            let mut indici: Vec<usize> = voci
                .iter()
                .map(|v| indice_voce(origine, *v))
                .collect::<Result<_, _>>()?;
            indici.sort_unstable();
            indici.dedup();
            let destinazione = a.clone().unwrap_or_else(|| nome_origine.clone());
            let stesso = destinazione == nome_origine;
            let lunghezza_dest = if stesso {
                origine.voci.len()
            } else {
                computo_mut(p, Some(&destinazione))?.voci.len()
            };
            let mut pos = match posizione {
                None => lunghezza_dest,
                Some(n) if *n >= 1 && *n <= lunghezza_dest + 1 => n - 1,
                Some(n) => {
                    return Err(format!("posizione {n} non valida (voci: {lunghezza_dest})"));
                }
            };
            let origine = computo_mut(p, Some(&nome_origine))?;
            // «Vedi voce»: tra le voci scelte i collegamenti seguono le voci (nella copia,
            // le nuove); verso le altre voci restano collegati nello stesso computo,
            // altrimenti diventano statici. Le quantità si copiano prima di modificare.
            let scelte: BTreeSet<VoceComputoId> = indici
                .iter()
                .filter_map(|i| origine.voci.get(*i).map(|v| v.id))
                .collect();
            let esterni = if stesso {
                HashMap::new()
            } else {
                let citate: BTreeSet<VoceComputoId> = scelte
                    .iter()
                    .filter_map(|id| origine.voce(*id))
                    .flat_map(VoceComputo::voci_citate)
                    .filter(|id| !scelte.contains(id))
                    .collect();
                statici_per(origine, &citate, &regole)?
            };
            let mut statici = 0;
            if *sposta && !stesso {
                statici += rendi_statici_i_riferimenti(origine, &scelte, &regole)?;
            }
            let mut nuovi_id: HashMap<VoceComputoId, VoceComputoId> = HashMap::new();
            let mut copiate: Vec<VoceComputo> = Vec::with_capacity(indici.len());
            if *sposta {
                // Dal fondo, così gli indici restanti non cambiano.
                for i in indici.iter().rev() {
                    let id = origine
                        .voci
                        .get(*i)
                        .map(|v| v.id)
                        .ok_or("voce inesistente")?;
                    copiate.push(origine.rimuovi_voce(id).map_err(|e| e.to_string())?);
                }
                copiate.reverse();
                if stesso {
                    pos -= indici.iter().filter(|i| **i < pos).count();
                }
            } else {
                for i in &indici {
                    let mut v = origine
                        .voci
                        .get(*i)
                        .map(|v| VoceComputo::clone(v))
                        .ok_or("voce inesistente")?;
                    let nuovo = g.nuovo();
                    nuovi_id.insert(v.id, nuovo);
                    v.id = nuovo;
                    for r in &mut v.righi {
                        r.id = g.nuovo();
                    }
                    copiate.push(v);
                }
                for r in copiate.iter_mut().flat_map(|v| v.righi.iter_mut()) {
                    if let Some(id) = r.vedi_voce.as_ref().and_then(VediVoce::voce)
                        && let Some(nuovo) = nuovi_id.get(&id)
                    {
                        r.vedi_voce = Some(VediVoce::Collegata(*nuovo));
                    }
                }
            }
            statici += sostituisci_collegamenti(copiate.iter_mut(), &esterni);
            let n = copiate.len();
            let dest = computo_mut(p, Some(&destinazione))?;
            for (k, v) in copiate.into_iter().enumerate() {
                dest.inserisci_voce(Some(pos + k), v)
                    .map_err(|e| e.to_string())?;
            }
            let azione = if *sposta { "spostate" } else { "copiate" };
            let statici = resi_statici(statici);
            Ok(if stesso {
                format!(
                    "{n} voci {azione} alla posizione {} di «{destinazione}»{statici}",
                    pos + 1
                )
            } else {
                format!(
                    "{n} voci {azione} da «{nome_origine}» a «{destinazione}» (posizione {}){statici}",
                    pos + 1
                )
            })
        }
        Comando::RiallineaPrezzo { computo, voce } => {
            let elenco = p.elenco_prezzi.clone();
            let c = computo_mut(p, computo.as_deref())?;
            let v = voce_mut(c, *voce)?;
            let sorgente = v
                .prezzo
                .origine
                .filter(|o| o.prezzario == elenco.id)
                .and_then(|o| elenco.voce(o.voce))
                .or_else(|| elenco.voce_per_codice(&v.prezzo.codice))
                .ok_or_else(|| {
                    format!(
                        "voce «{}» non più presente nell'elenco prezzi",
                        v.prezzo.codice
                    )
                })?;
            let differenze = v.prezzo.riallinea(&elenco, sorgente);
            Ok(if differenze.is_empty() {
                "prezzo già allineato all'elenco".into()
            } else {
                format!(
                    "prezzo riallineato all'elenco ({} campi aggiornati)",
                    differenze.len()
                )
            })
        }
        Comando::AggiungiRigo {
            computo,
            voce,
            rigo,
            posizione,
        } => {
            let nome = computo_rif(p, computo.as_deref())?.nome.clone();
            let nuovo = rigo.in_rigo_di(g, p, Some(&nome))?;
            let c = computo_mut(p, computo.as_deref())?;
            let v = voce_mut(c, *voce)?;
            let pos = posizione_righi(*posizione, v.righi.len())?;
            v.righi.insert(pos, nuovo);
            Ok(format!("rigo n. {} aggiunto", pos + 1))
        }
        Comando::ModificaRigo {
            computo,
            voce,
            numero,
            rigo,
        } => {
            let nome = computo_rif(p, computo.as_deref())?.nome.clone();
            let mut nuovo = rigo.in_rigo_di(g, p, Some(&nome))?;
            let c = computo_mut(p, computo.as_deref())?;
            let v = voce_mut(c, *voce)?;
            let n = v.righi.len();
            let r = numero
                .checked_sub(1)
                .and_then(|i| v.righi.get_mut(i))
                .ok_or_else(|| format!("rigo n. {numero} inesistente (righi: {n})"))?;
            nuovo.id = r.id;
            *r = nuovo;
            Ok(format!("rigo n. {numero} modificato"))
        }
        Comando::RimuoviRigo {
            computo,
            voce,
            numero,
        } => {
            let c = computo_mut(p, computo.as_deref())?;
            let v = voce_mut(c, *voce)?;
            if *numero < 1 || *numero > v.righi.len() {
                return Err(format!(
                    "rigo n. {numero} inesistente (righi: {})",
                    v.righi.len()
                ));
            }
            v.righi.remove(numero - 1);
            Ok(format!("rigo n. {numero} rimosso"))
        }
        Comando::Registra {
            data,
            codice,
            righi,
            annotazioni,
        } => {
            let sorgente = voce_elenco(p, codice)?.clone();
            let prezzo = PrezzoApplicato::da_voce(&p.elenco_prezzi, &sorgente);
            let numero = p
                .contabilita
                .libretto()
                .registrazioni
                .iter()
                .map(|r| r.numero)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            let mut r = RegistrazioneLibretto::new(g.nuovo(), numero, *data, prezzo);
            r.righi = righi
                .iter()
                .map(|x| x.in_rigo_di(g, p, None))
                .collect::<Result<_, _>>()?;
            r.annotazioni = annotazioni.clone().filter(|a| !a.trim().is_empty());
            p.contabilita.registra(r).map_err(|e| e.to_string())?;
            Ok(format!(
                "registrazione n. {numero} «{codice}» aggiunta al libretto"
            ))
        }
        Comando::ModificaRegistrazione {
            numero,
            data,
            righi,
            annotazioni,
        } => {
            let id = registrazione(p, *numero)?;
            let nuovi: Option<Vec<RigoMisura>> = righi
                .as_ref()
                .map(|rr| {
                    rr.iter()
                        .map(|x| x.in_rigo_di(g, p, None))
                        .collect::<Result<_, _>>()
                })
                .transpose()?;
            p.contabilita
                .modifica_registrazione(id, |r| {
                    if let Some(d) = data {
                        r.data = *d;
                    }
                    if let Some(n) = nuovi {
                        r.righi = n;
                    }
                    if let Some(a) = annotazioni {
                        r.annotazioni = Some(a.clone()).filter(|a| !a.trim().is_empty());
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(format!("registrazione n. {numero} modificata"))
        }
        Comando::RimuoviRegistrazione { numero } => {
            let id = registrazione(p, *numero)?;
            p.contabilita
                .rimuovi_registrazione(id)
                .map_err(|e| e.to_string())?;
            Ok(format!("registrazione n. {numero} rimossa"))
        }
        Comando::NuovoSal { lavori_a_tutto_il } => {
            let s = p
                .contabilita
                .nuovo_sal(g.nuovo(), *lavori_a_tutto_il)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "SAL n. {} creato in bozza con {} registrazioni",
                s.numero,
                s.registrazioni.len()
            ))
        }
        Comando::AnnullaBozzaSal { numero } => {
            let id = sal(p, *numero)?;
            p.contabilita
                .annulla_bozza_sal(id)
                .map_err(|e| e.to_string())?;
            Ok(format!("SAL n. {numero} in bozza annullato"))
        }
        Comando::EmettiSal {
            numero,
            data_emissione,
        } => {
            let id = sal(p, *numero)?;
            let r = p
                .emetti_sal(id, *data_emissione)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "SAL n. {numero} emesso: importo {} €",
                crate::formato_importo(r.importo_sal)
            ))
        }
        Comando::RedigiContoFinale {
            data_ultimazione,
            data,
            note,
        } => {
            let attuale = p.contabilita.conto_finale();
            let ultimazione = data_ultimazione
                .or(attuale.map(|c| c.data_ultimazione))
                .ok_or("indicare la data di ultimazione dei lavori")?;
            let data = data.or(attuale.map(|c| c.data)).unwrap_or(ultimazione);
            let note = match note {
                Some(n) => n.clone().filter(|t| !t.trim().is_empty()),
                None => attuale.and_then(|c| c.note.clone()),
            };
            let nuovo = attuale.is_none();
            p.contabilita
                .redigi_conto_finale(ultimazione, data, note)
                .map_err(|e| e.to_string())?;
            let n = p.contabilita.registrazioni_stato_finale().len();
            Ok(if nuovo {
                format!("conto finale redatto in bozza ({n} registrazioni nello stato finale)")
            } else {
                "conto finale aggiornato".into()
            })
        }
        Comando::EmettiContoFinale { data } => {
            let data = data
                .or(p.contabilita.conto_finale().map(|c| c.data))
                .ok_or("il conto finale non è stato redatto")?;
            let r = p.emetti_conto_finale(data).map_err(|e| e.to_string())?;
            Ok(format!(
                "conto finale emesso: rata di saldo {} €",
                crate::formato_importo(r.totale)
            ))
        }
        Comando::AnnullaContoFinale {} => {
            p.contabilita
                .annulla_conto_finale()
                .map_err(|e| e.to_string())?;
            Ok("conto finale in bozza annullato".into())
        }
        Comando::FirmaContoFinale { data, riserve } => {
            p.contabilita
                .firma_conto_finale(*data, riserve.clone())
                .map_err(|e| e.to_string())?;
            Ok(if data.is_some() {
                "firma dell'esecutore registrata".into()
            } else {
                "firma dell'esecutore rimossa".into()
            })
        }
        c @ (Comando::ImpostaCronoprogramma { .. }
        | Comando::ImpostaAttivita { .. }
        | Comando::AggiungiAttivita { .. }
        | Comando::RimuoviAttivita { .. }
        | Comando::CollegaAttivita { .. }
        | Comando::ScollegaAttivita { .. }
        | Comando::LineaBaseCronoprogramma { .. }) => crate::comandi_crono::applica(p, c, g),
        c @ (Comando::ImpostaRisorsa { .. }
        | Comando::RimuoviRisorsa { .. }
        | Comando::ImpostaAnalisi { .. }
        | Comando::RimuoviAnalisi { .. }
        | Comando::RicalcolaPrezziAnalisi {}
        | Comando::ImpostaParametriAnalisi { .. }
        | Comando::AggiornaPrezzi { .. }
        | Comando::ImpostaNuovoPrezzo { .. }
        | Comando::ImpostaVerbaleNuoviPrezzi { .. }
        | Comando::RimuoviVerbaleNuoviPrezzi { .. }) => crate::comandi_analisi::applica(p, c, g),
        Comando::RegistraCollaudo {
            tipo,
            data,
            approvato_il,
            rimuovi,
        } => {
            if *rimuovi {
                p.contabilita
                    .registra_collaudo(None)
                    .map_err(|e| e.to_string())?;
                return Ok("collaudo rimosso".into());
            }
            let attuale = p
                .contabilita
                .conto_finale()
                .and_then(|c| c.collaudo.clone());
            let collaudo = Collaudo {
                tipo: tipo
                    .or(attuale.as_ref().map(|c| c.tipo))
                    .ok_or("indicare il tipo: collaudo o regolare_esecuzione")?,
                data: data
                    .or(attuale.as_ref().map(|c| c.data))
                    .ok_or("indicare la data del certificato")?,
                approvato_il: approvato_il.or(attuale.and_then(|c| c.approvato_il)),
            };
            let nome = collaudo.tipo.nome();
            p.contabilita
                .registra_collaudo(Some(collaudo))
                .map_err(|e| e.to_string())?;
            Ok(format!("{nome} registrato"))
        }
    }
}

fn registrazione(
    p: &Progetto,
    numero: u32,
) -> Result<opencomputo_core::id::RegistrazioneId, String> {
    p.contabilita
        .libretto()
        .registrazioni
        .iter()
        .find(|r| r.numero == numero)
        .map(|r| r.id)
        .ok_or_else(|| format!("registrazione n. {numero} inesistente"))
}

/// Distingue un campo assente (`None`) da un campo `null` (`Some(None)`).
pub(crate) fn opzione_doppia<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

fn sal(p: &Progetto, numero: u32) -> Result<opencomputo_core::id::SalId, String> {
    p.contabilita
        .sal()
        .iter()
        .find(|s| s.numero == numero)
        .map(|s| s.id)
        .ok_or_else(|| format!("SAL n. {numero} inesistente"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn fattori_in_ingresso() {
        let f = |j: &str| {
            serde_json::from_str::<FattoreIngresso>(j)
                .unwrap()
                .in_fattore()
        };
        assert_eq!(f("12.5"), Fattore::Valore(dec!(12.5)));
        assert_eq!(f("\"12,50\""), Fattore::Valore(dec!(12.50)));
        assert_eq!(f("\"7.80\""), Fattore::Valore(dec!(7.80)));
        assert_eq!(f("\"2*(3+4)\""), Fattore::Espressione("2*(3+4)".into()));
        assert_eq!(f("\"1.234,5\""), Fattore::Espressione("1.234,5".into()));
    }

    #[test]
    fn comandi_in_json() {
        let c: Comando = serde_json::from_str(
            r#"{"comando": "aggiungi_voce_computo", "codice": "A.1", "righi": [{"descrizione": "Platea", "lunghezza": 12, "larghezza": "8,00"}]}"#,
        )
        .unwrap();
        assert!(matches!(c, Comando::AggiungiVoceComputo { ref righi, .. } if righi.len() == 1));
        let r: Result<Comando, _> =
            serde_json::from_str(r#"{"comando": "nuovo_computo", "nome": "X", "sconosciuto": 1}"#);
        assert!(r.is_err(), "campi sconosciuti rifiutati");
        let r: Result<Comando, _> = serde_json::from_str(r#"{"comando": "inesistente"}"#);
        assert!(r.is_err());
        let v: RifVoce = serde_json::from_str("3").unwrap();
        assert_eq!(v, RifVoce::Numero(3));
    }
}
