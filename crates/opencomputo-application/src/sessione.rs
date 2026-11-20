//! Sessione di lavoro su un progetto: comandi, annulla/ripristina, prova senza applicare.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

use opencomputo_core::computo::RiepilogoComputo;
use opencomputo_core::id::{ComputoId, GeneratoreId, GeneratoreSequenziale};
use opencomputo_core::numeri::Importo;
use opencomputo_core::prezzario::Prezzario;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::validazione::{Gravita, Problema, valida_progetto};
use opencomputo_storage::{EsitoSalvataggio, OpzioniSalvataggio};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::comandi::{Comando, EsitoComando, applica};
use crate::errore::ErroreApplicazione;

/// Passi di annullamento conservati per impostazione predefinita.
pub const LIMITE_ANNULLA: usize = 50;

/// Totale di un computo prima e dopo i comandi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariazioneTotale {
    /// Nome del computo.
    pub computo: String,
    /// Totale prima (assente se il computo non esisteva o non era calcolabile).
    pub prima: Option<Importo>,
    /// Totale dopo (assente se il computo non esiste più o non è calcolabile).
    pub dopo: Option<Importo>,
}

/// Esito di una richiesta di comandi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Esito {
    /// `false` per una prova: il progetto non è stato modificato.
    pub applicato: bool,
    /// Esito di ciascun comando.
    pub comandi: Vec<EsitoComando>,
    /// Problemi di validazione comparsi.
    pub nuovi_problemi: Vec<Problema>,
    /// Problemi di validazione scomparsi.
    pub problemi_risolti: Vec<Problema>,
    /// Totali dei computi che sono cambiati.
    pub totali: Vec<VariazioneTotale>,
}

/// Regole applicate ai comandi.
#[derive(Debug, Clone)]
pub struct Politica {
    /// Rifiuta le richieste che introducono nuovi errori di validazione (predefinito).
    /// Gli avvisi sono sempre consentiti e riportati nell'esito.
    pub rifiuta_nuovi_errori: bool,
    /// Passi di annullamento conservati.
    pub limite_annulla: usize,
}

impl Default for Politica {
    fn default() -> Self {
        Self {
            rifiuta_nuovi_errori: true,
            limite_annulla: LIMITE_ANNULLA,
        }
    }
}

struct Passo {
    descrizione: String,
    progetto: Progetto,
    stato: OnceLock<Stato>,
}

/// Totali di un computo (senza il dettaglio delle voci).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotaliComputo {
    /// Computo.
    pub computo: ComputoId,
    /// Nome del computo.
    pub nome: String,
    /// Totali, o il messaggio dell'errore che impedisce il calcolo.
    pub totali: Result<ImportiComputo, String>,
}

/// Importi totali di un computo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportiComputo {
    /// Totale.
    pub totale: Importo,
    /// Lavori.
    pub totale_lavori: Importo,
    /// Oneri della sicurezza.
    pub totale_oneri_sicurezza: Importo,
    /// Manodopera.
    pub totale_manodopera: Importo,
}

impl From<&RiepilogoComputo> for ImportiComputo {
    fn from(r: &RiepilogoComputo) -> Self {
        Self {
            totale: r.totale,
            totale_lavori: r.totale_lavori,
            totale_oneri_sicurezza: r.totale_oneri_sicurezza,
            totale_manodopera: r.totale_manodopera,
        }
    }
}

/// Validazione e totali di uno stato del progetto, calcolati una volta sola: il
/// progetto di una sessione cambia solo con [`Sessione::esegui`], `annulla` e
/// `ripristina`, che portano con sé lo stato del progetto che rendono corrente.
#[derive(Debug, Clone)]
struct Stato {
    problemi: Vec<Problema>,
    computi: Vec<TotaliComputo>,
}

impl Stato {
    fn di(p: &Progetto) -> Self {
        Self {
            problemi: valida_progetto(p).problemi,
            computi: p
                .computi
                .iter()
                .map(|c| TotaliComputo {
                    computo: c.id,
                    nome: c.nome.clone(),
                    totali: p
                        .calcola_computo(c.id)
                        .map(|r| ImportiComputo::from(&r))
                        .map_err(|e| e.to_string()),
                })
                .collect(),
        }
    }

    fn totali(&self) -> Vec<(String, Option<Importo>)> {
        self.computi
            .iter()
            .map(|c| (c.nome.clone(), c.totali.as_ref().ok().map(|t| t.totale)))
            .collect()
    }
}

fn gia_calcolato(stato: Stato) -> OnceLock<Stato> {
    let s = OnceLock::new();
    let _ = s.set(stato);
    s
}

/// Sessione di lavoro.
///
/// Ogni richiesta (uno o più comandi) è **atomica**: i comandi sono applicati su una
/// copia del progetto e la copia sostituisce l'originale solo se tutti riescono e la
/// validazione lo consente. Ogni richiesta applicata è un passo di annullamento.
pub struct Sessione {
    progetto: Progetto,
    generatore: GeneratoreSequenziale,
    annulla: Vec<Passo>,
    ripristina: Vec<Passo>,
    politica: Politica,
    modificato: bool,
    prezzari: Vec<(String, Prezzario)>,
    stato: OnceLock<Stato>,
}

impl std::fmt::Debug for Sessione {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessione")
            .field("titolo", &self.progetto.metadati.titolo)
            .field("annulla", &self.annulla.len())
            .field("ripristina", &self.ripristina.len())
            .field("modificato", &self.modificato)
            .field("prezzari", &self.prezzari.len())
            .finish_non_exhaustive()
    }
}

/// Identificativo più alto usato nel progetto (tutti gli UUID del documento).
fn id_massimo(p: &Progetto) -> u128 {
    use std::cell::{Cell, RefCell};
    use std::io;

    /// Formatter JSON che non scrive nulla e annota le stringhe di 36 caratteri che
    /// sono UUID: valori e chiavi delle mappe, senza costruire il documento in memoria.
    /// Una stringa che contiene un UUID con caratteri di escape non conta (un UUID non
    /// ne ha); un testo che per caso è un UUID alza il massimo, il che non nuoce.
    struct Cerca<'a> {
        stringa: &'a RefCell<String>,
        valida: &'a Cell<bool>,
        massimo: &'a Cell<u128>,
    }
    impl serde_json::ser::Formatter for Cerca<'_> {
        fn begin_string<W: ?Sized + io::Write>(&mut self, _: &mut W) -> io::Result<()> {
            self.stringa.borrow_mut().clear();
            self.valida.set(true);
            Ok(())
        }
        fn write_string_fragment<W: ?Sized + io::Write>(
            &mut self,
            _: &mut W,
            frammento: &str,
        ) -> io::Result<()> {
            let mut s = self.stringa.borrow_mut();
            if s.len().saturating_add(frammento.len()) <= 36 {
                s.push_str(frammento);
            } else {
                self.valida.set(false);
            }
            Ok(())
        }
        fn write_char_escape<W: ?Sized + io::Write>(
            &mut self,
            _: &mut W,
            _: serde_json::ser::CharEscape,
        ) -> io::Result<()> {
            self.valida.set(false);
            Ok(())
        }
        fn end_string<W: ?Sized + io::Write>(&mut self, _: &mut W) -> io::Result<()> {
            let s = self.stringa.borrow();
            if self.valida.get()
                && s.len() == 36
                && let Ok(u) = Uuid::parse_str(&s)
            {
                self.massimo.set(self.massimo.get().max(u.as_u128()));
            }
            Ok(())
        }
    }

    let (stringa, valida, massimo) = (RefCell::new(String::new()), Cell::new(false), Cell::new(0));
    let f = Cerca {
        stringa: &stringa,
        valida: &valida,
        massimo: &massimo,
    };
    let mut ser = serde_json::Serializer::with_formatter(io::sink(), f);
    let _ = serde::Serialize::serialize(p, &mut ser);
    massimo.get()
}

/// Chiave di confronto di un problema: indipendente dai numeri d'ordine, che cambiano
/// quando si inseriscono o spostano voci.
fn chiave(p: &Problema) -> String {
    fn senza_numeri(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("numero");
                o.values_mut().for_each(senza_numeri);
            }
            Value::Array(a) => a.iter_mut().for_each(senza_numeri),
            _ => {}
        }
    }
    let mut percorso = serde_json::to_value(&p.percorso).unwrap_or(Value::Null);
    senza_numeri(&mut percorso);
    format!("{:?}|{}|{}", p.gravita, p.codice, percorso)
}

impl Sessione {
    /// Sessione su un progetto esistente.
    #[must_use]
    pub fn new(progetto: Progetto) -> Self {
        let inizio = id_massimo(&progetto).wrapping_add(1);
        Self {
            progetto,
            generatore: GeneratoreSequenziale::new(inizio),
            annulla: Vec::new(),
            ripristina: Vec::new(),
            politica: Politica::default(),
            modificato: false,
            prezzari: Vec::new(),
            stato: OnceLock::new(),
        }
    }

    /// Sessione su un progetto nuovo con un computo vuoto.
    #[must_use]
    pub fn nuovo(titolo: &str) -> Self {
        let mut g = GeneratoreSequenziale::default();
        let mut p = Progetto::new(g.nuovo(), titolo, g.nuovo());
        p.elenco_prezzi.nome = "Elenco prezzi di progetto".into();
        let c = opencomputo_core::computo::Computo::new(g.nuovo(), "Computo metrico estimativo");
        p.computi.push(c);
        let mut s = Self::new(p);
        s.modificato = true;
        s
    }

    /// Apre un file di progetto.
    ///
    /// # Errors
    /// Errori di lettura del file.
    pub fn apri(percorso: &Path) -> Result<Self, ErroreApplicazione> {
        Ok(Self::new(opencomputo_storage::apri(percorso)?))
    }

    /// Salva il progetto e segna la sessione come non modificata.
    ///
    /// # Errors
    /// Errori di scrittura (il file precedente resta intatto).
    pub fn salva(
        &mut self,
        percorso: &Path,
        opzioni: &OpzioniSalvataggio,
    ) -> Result<EsitoSalvataggio, ErroreApplicazione> {
        let e = opencomputo_storage::salva(percorso, &self.progetto, opzioni)?;
        self.modificato = false;
        Ok(e)
    }

    /// Imposta la politica.
    pub fn con_politica(mut self, politica: Politica) -> Self {
        self.politica = politica;
        self
    }

    /// Progetto corrente (sola lettura: le modifiche passano dai comandi).
    #[must_use]
    pub fn progetto(&self) -> &Progetto {
        &self.progetto
    }

    /// `true` se ci sono modifiche non salvate.
    #[must_use]
    pub fn modificato(&self) -> bool {
        self.modificato
    }

    /// Descrizioni dei passi annullabili, dal più recente.
    #[must_use]
    pub fn passi_annullabili(&self) -> Vec<&str> {
        self.annulla
            .iter()
            .rev()
            .map(|p| p.descrizione.as_str())
            .collect()
    }

    /// Descrizioni dei passi ripristinabili, dal più recente.
    #[must_use]
    pub fn passi_ripristinabili(&self) -> Vec<&str> {
        self.ripristina
            .iter()
            .rev()
            .map(|p| p.descrizione.as_str())
            .collect()
    }

    /// Carica un prezzario di consultazione (non salvato nel progetto), da cui copiare
    /// voci con il comando `copia_da_prezzario`. Un prezzario con lo stesso nome è sostituito.
    pub fn carica_prezzario(&mut self, nome: &str, prezzario: Prezzario) {
        self.prezzari.retain(|(n, _)| n != nome);
        self.prezzari.push((nome.to_owned(), prezzario));
    }

    /// Prezzari di consultazione caricati.
    #[must_use]
    pub fn prezzari(&self) -> &[(String, Prezzario)] {
        &self.prezzari
    }

    fn stato(&self) -> &Stato {
        self.stato.get_or_init(|| Stato::di(&self.progetto))
    }

    /// Problemi di validazione del progetto corrente (calcolati una volta per ogni
    /// stato del progetto).
    #[must_use]
    pub fn problemi(&self) -> &[Problema] {
        &self.stato().problemi
    }

    /// Totali dei computi del progetto corrente, nell'ordine dei computi (calcolati una
    /// volta per ogni stato del progetto).
    #[must_use]
    pub fn totali_computi(&self) -> &[TotaliComputo] {
        &self.stato().computi
    }

    fn calcola(
        &self,
        comandi: &[Comando],
    ) -> Result<(Progetto, GeneratoreSequenziale, Esito, Stato), ErroreApplicazione> {
        if comandi.is_empty() {
            return Err(ErroreApplicazione::NessunComando);
        }
        let mut copia = self.progetto.clone();
        let mut g = self.generatore.clone();
        let mut esiti = Vec::with_capacity(comandi.len());
        for (indice, c) in comandi.iter().enumerate() {
            let descrizione = applica(&mut copia, c, &mut g, &self.prezzari)
                .map_err(|messaggio| ErroreApplicazione::Comando { indice, messaggio })?;
            esiti.push(EsitoComando {
                indice,
                descrizione,
            });
        }
        let prima_stato = self.stato();
        let dopo_stato = Stato::di(&copia);
        let (prima, dopo) = (&prima_stato.problemi, &dopo_stato.problemi);
        let chiavi_prima: BTreeSet<String> = prima.iter().map(chiave).collect();
        let chiavi_dopo: BTreeSet<String> = dopo.iter().map(chiave).collect();
        let nuovi: Vec<Problema> = dopo
            .iter()
            .filter(|p| !chiavi_prima.contains(&chiave(p)))
            .cloned()
            .collect();
        let risolti: Vec<Problema> = prima
            .iter()
            .filter(|p| !chiavi_dopo.contains(&chiave(p)))
            .cloned()
            .collect();
        if self.politica.rifiuta_nuovi_errori {
            let errori: Vec<Problema> = nuovi
                .iter()
                .filter(|p| p.gravita == Gravita::Errore)
                .cloned()
                .collect();
            if !errori.is_empty() {
                return Err(ErroreApplicazione::NuoviErrori { problemi: errori });
            }
        }
        let t_prima = prima_stato.totali();
        let t_dopo = dopo_stato.totali();
        let mut variazioni = Vec::new();
        for (nome, dopo) in &t_dopo {
            let prima = t_prima
                .iter()
                .find(|(n, _)| n == nome)
                .and_then(|(_, t)| *t);
            if prima != *dopo {
                variazioni.push(VariazioneTotale {
                    computo: nome.clone(),
                    prima,
                    dopo: *dopo,
                });
            }
        }
        for (nome, prima) in &t_prima {
            if !t_dopo.iter().any(|(n, _)| n == nome) {
                variazioni.push(VariazioneTotale {
                    computo: nome.clone(),
                    prima: *prima,
                    dopo: None,
                });
            }
        }
        Ok((
            copia,
            g,
            Esito {
                applicato: false,
                comandi: esiti,
                nuovi_problemi: nuovi,
                problemi_risolti: risolti,
                totali: variazioni,
            },
            dopo_stato,
        ))
    }

    /// Prova i comandi senza modificare il progetto.
    ///
    /// # Errors
    /// Come [`Sessione::esegui`].
    pub fn prova(&self, comandi: &[Comando]) -> Result<Esito, ErroreApplicazione> {
        self.calcola(comandi).map(|(_, _, e, _)| e)
    }

    /// Esegue i comandi come un unico passo atomico.
    ///
    /// # Errors
    /// Nessun comando, comando non applicabile (con il suo indice), nuovi errori di
    /// validazione: in tutti i casi il progetto resta invariato.
    pub fn esegui(&mut self, comandi: &[Comando]) -> Result<Esito, ErroreApplicazione> {
        let (nuovo, g, mut esito, stato) = self.calcola(comandi)?;
        let descrizione = match esito.comandi.as_slice() {
            [unico] => unico.descrizione.clone(),
            molti => format!("{} comandi", molti.len()),
        };
        let precedente = std::mem::replace(&mut self.progetto, nuovo);
        let stato_precedente = std::mem::replace(&mut self.stato, gia_calcolato(stato));
        self.annulla.push(Passo {
            descrizione,
            progetto: precedente,
            stato: stato_precedente,
        });
        if self.annulla.len() > self.politica.limite_annulla {
            self.annulla.remove(0);
        }
        self.ripristina.clear();
        self.generatore = g;
        self.modificato = true;
        esito.applicato = true;
        Ok(esito)
    }

    /// Annulla l'ultimo passo e ne restituisce la descrizione.
    ///
    /// # Errors
    /// Nessun passo da annullare.
    pub fn annulla(&mut self) -> Result<String, ErroreApplicazione> {
        let passo = self
            .annulla
            .pop()
            .ok_or(ErroreApplicazione::NienteDaAnnullare)?;
        let attuale = std::mem::replace(&mut self.progetto, passo.progetto);
        let stato = std::mem::replace(&mut self.stato, passo.stato);
        self.ripristina.push(Passo {
            descrizione: passo.descrizione.clone(),
            progetto: attuale,
            stato,
        });
        self.modificato = true;
        Ok(passo.descrizione)
    }

    /// Ripristina l'ultimo passo annullato e ne restituisce la descrizione.
    ///
    /// # Errors
    /// Nessun passo da ripristinare.
    pub fn ripristina(&mut self) -> Result<String, ErroreApplicazione> {
        let passo = self
            .ripristina
            .pop()
            .ok_or(ErroreApplicazione::NienteDaRipristinare)?;
        let attuale = std::mem::replace(&mut self.progetto, passo.progetto);
        let stato = std::mem::replace(&mut self.stato, passo.stato);
        self.annulla.push(Passo {
            descrizione: passo.descrizione.clone(),
            progetto: attuale,
            stato,
        });
        self.modificato = true;
        Ok(passo.descrizione)
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Il calcolo di prima: tutto il progetto come albero JSON, solo i valori.
    fn id_massimo_con_albero(p: &Progetto) -> u128 {
        fn visita(v: &Value, max: &mut u128) {
            match v {
                Value::String(s) => {
                    if let Ok(u) = Uuid::parse_str(s).map(|u| u.as_u128())
                        && s.len() == 36
                    {
                        *max = (*max).max(u);
                    }
                }
                Value::Array(a) => a.iter().for_each(|x| visita(x, max)),
                Value::Object(o) => o.iter().for_each(|(k, x)| {
                    if let Ok(u) = Uuid::parse_str(k)
                        && k.len() == 36
                    {
                        *max = (*max).max(u.as_u128());
                    }
                    visita(x, max);
                }),
                _ => {}
            }
        }
        let mut max = 0;
        visita(&serde_json::to_value(p).unwrap(), &mut max);
        max
    }

    #[test]
    fn identificativo_massimo_come_con_l_albero_json() {
        for testo in [
            include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json"),
            include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v4.json"),
            include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v7.json"),
        ] {
            let mut p = opencomputo_core::schema::importa_json(testo).unwrap();
            let base = id_massimo(&p);
            assert_eq!(base, id_massimo_con_albero(&p));
            // Un testo che contiene un UUID tra altri caratteri non conta.
            p.metadati.titolo = "\"ffffffff-ffff-ffff-ffff-ffffffffffff\"".into();
            assert_eq!(id_massimo(&p), base);
            // Un testo che è esattamente un UUID conta (alza il massimo: non nuoce).
            p.metadati.titolo = "ffffffff-ffff-ffff-ffff-fffffffffff0".into();
            assert_eq!(id_massimo(&p), u128::MAX - 15);
        }
    }

    /// La validazione e i totali in memoria sono quelli del progetto corrente dopo ogni
    /// modifica, annullamento e ripristino.
    #[test]
    fn stato_in_memoria_sempre_del_progetto_corrente() {
        let p = opencomputo_core::schema::importa_json(include_str!(
            "../../opencomputo-core/tests/fixtures/progetto_esempio_v7.json"
        ))
        .unwrap();
        let mut s = Sessione::new(p);
        let verifica = |s: &Sessione| {
            let atteso = Stato::di(s.progetto());
            assert_eq!(s.problemi(), atteso.problemi.as_slice());
            assert_eq!(s.totali_computi(), atteso.computi.as_slice());
        };
        verifica(&s);
        let modifica: Vec<Comando> = serde_json::from_value(serde_json::json!([
            {"comando": "modifica_rigo", "voce": 1, "numero": 1,
             "rigo": {"descrizione": "Nuovo", "lunghezza": "7", "larghezza": "3", "altezza_peso": "2"}}
        ]))
        .unwrap();
        let prima = s.totali_computi().to_vec();
        s.esegui(&modifica).unwrap();
        verifica(&s);
        assert_ne!(s.totali_computi(), prima.as_slice());
        s.annulla().unwrap();
        verifica(&s);
        assert_eq!(s.totali_computi(), prima.as_slice());
        s.ripristina().unwrap();
        verifica(&s);
        // Una prova non cambia nulla.
        let _ = s.prova(&modifica);
        verifica(&s);
    }
}
