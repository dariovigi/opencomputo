//! Validazione del modello.
//!
//! Le operazioni del modello impediscono molte incoerenze, ma i dati possono
//! arrivare da file, importazioni o modifiche dirette ai campi pubblici.
//! [`valida_progetto`] esamina l'intero progetto e restituisce un elenco di
//! problemi con gravità, codice stabile (adatto a filtri e API), messaggio e
//! percorso dell'entità coinvolta. La validazione non modifica mai i dati.

use std::collections::BTreeSet;

use rust_decimal::Decimal;

use serde::{Deserialize, Serialize};

use crate::analisi::OrigineRigaAnalisi;
use crate::classificazione::{AssegnazioneCategorie, Categorie, LivelloCategoria, StrutturaWbs};
use crate::contabilita::{ParametriContabili, TipoCollaudo};
use crate::errore::ErroreLocalizzato;
use crate::id::{VoceComputoId, WbsNodoId};
use crate::misura::{
    QuantitaRiferite, RigoMisura, SegnoRigo, SenzaRiferimenti, TipoRigo, VediVoce,
};
use crate::numeri::{Importo, Percentuale, RegoleArrotondamento};
use crate::partita::{PrezzoApplicato, calcola_partita_con};
use crate::prezzario::{CriterioNuovoPrezzo, NaturaVoce, Prezzario};
use crate::progetto::Progetto;
use crate::riferimento::Riferimento;

/// Gravità di un problema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gravita {
    /// Informazione.
    Informazione,
    /// Avviso: il dato è calcolabile ma probabilmente incompleto o anomalo.
    Avviso,
    /// Errore: il dato è incoerente o non calcolabile.
    Errore,
}

/// Problema rilevato dalla validazione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problema {
    /// Gravità.
    pub gravita: Gravita,
    /// Codice stabile (es. `prezzario.codice_duplicato`).
    pub codice: String,
    /// Messaggio leggibile.
    pub messaggio: String,
    /// Percorso dell'entità coinvolta.
    pub percorso: Vec<Riferimento>,
}

impl Problema {
    /// Nuovo problema.
    #[must_use]
    pub fn new(
        gravita: Gravita,
        codice: impl Into<String>,
        messaggio: impl Into<String>,
        percorso: Vec<Riferimento>,
    ) -> Self {
        Self {
            gravita,
            codice: codice.into(),
            messaggio: messaggio.into(),
            percorso,
        }
    }

    /// Nuovo errore.
    #[must_use]
    pub fn errore(
        codice: impl Into<String>,
        messaggio: impl Into<String>,
        percorso: Vec<Riferimento>,
    ) -> Self {
        Self::new(Gravita::Errore, codice, messaggio, percorso)
    }

    /// Nuovo avviso.
    #[must_use]
    pub fn avviso(
        codice: impl Into<String>,
        messaggio: impl Into<String>,
        percorso: Vec<Riferimento>,
    ) -> Self {
        Self::new(Gravita::Avviso, codice, messaggio, percorso)
    }

    /// Nuova informazione.
    #[must_use]
    pub fn informazione(
        codice: impl Into<String>,
        messaggio: impl Into<String>,
        percorso: Vec<Riferimento>,
    ) -> Self {
        Self::new(Gravita::Informazione, codice, messaggio, percorso)
    }
}

/// Esito della validazione.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RapportoValidazione {
    /// Problemi rilevati.
    pub problemi: Vec<Problema>,
}

impl RapportoValidazione {
    /// `true` se è presente almeno un errore.
    #[must_use]
    pub fn ha_errori(&self) -> bool {
        self.problemi.iter().any(|p| p.gravita == Gravita::Errore)
    }

    /// Problemi con il codice indicato.
    pub fn con_codice<'a>(&'a self, codice: &'a str) -> impl Iterator<Item = &'a Problema> + 'a {
        self.problemi.iter().filter(move |p| p.codice == codice)
    }
}

/// Valida l'intero progetto.
#[must_use]
pub fn valida_progetto(progetto: &Progetto) -> RapportoValidazione {
    let mut v = Validatore {
        problemi: Vec::new(),
    };
    let regole_valide = match progetto.regole.valida() {
        Ok(()) => true,
        Err(e) => {
            v.errore(
                "regole.non_valide",
                e.to_string(),
                vec![Riferimento::Regole],
            );
            false
        }
    };
    if let Some(c) = &progetto.contratto {
        for (nome, p) in [
            ("ribasso", c.parametri.ribasso),
            ("ritenuta di garanzia", c.parametri.ritenuta_garanzia),
        ] {
            if !p.nell_intervallo_0_100() || p == Percentuale::CENTO {
                v.errore(
                    "contratto.percentuale_non_valida",
                    format!("{nome} del {p} % fuori dall'intervallo [0, 100)"),
                    vec![Riferimento::Contratto],
                );
            }
        }
    }
    v.prezzario(&progetto.elenco_prezzi);
    if regole_valide {
        v.analisi_prezzi(&progetto.elenco_prezzi, &progetto.regole);
    }
    v.categorie(&progetto.categorie);
    if let Err(e) = progetto.wbs.verifica() {
        v.errore("wbs.non_valida", e.to_string(), vec![]);
    }
    v.cronoprogramma(progetto);

    let mut computi_visti = BTreeSet::new();
    let mut voci_computo: BTreeSet<VoceComputoId> = BTreeSet::new();
    for computo in &progetto.computi {
        let rif_computo = Riferimento::Computo { id: computo.id };
        if !computi_visti.insert(computo.id) {
            v.errore(
                "computo.id_duplicato",
                "identificativo duplicato",
                vec![rif_computo.clone()],
            );
        }
        let mut voci_viste = BTreeSet::new();
        let quantita = regole_valide.then(|| computo.quantita_riferite(&progetto.regole));
        let riferite: &dyn QuantitaRiferite = match &quantita {
            Some(q) => q,
            None => &SenzaRiferimenti,
        };
        for (i, voce) in computo.voci.iter().enumerate() {
            let percorso = vec![
                rif_computo.clone(),
                Riferimento::VoceComputo {
                    numero: i + 1,
                    id: voce.id,
                },
            ];
            if !voci_viste.insert(voce.id) {
                v.errore(
                    "computo.voce_id_duplicato",
                    "identificativo duplicato",
                    percorso.clone(),
                );
            }
            voci_computo.insert(voce.id);
            // Una voce in un riferimento circolare: l'errore che lo descrive.
            let circolare = quantita
                .as_ref()
                .and_then(|q| q.di(voce.id))
                .and_then(|r| r.as_ref().err());
            v.partita(
                &voce.prezzo,
                &voce.righi,
                &progetto.regole,
                regole_valide.then_some(circolare),
                riferite,
                &percorso,
            );
            v.assegnazioni(
                &progetto.categorie,
                &progetto.wbs,
                &voce.categorie,
                voce.wbs,
                &percorso,
            );
            v.allineamento_elenco(&progetto.elenco_prezzi, &voce.prezzo, &percorso);
        }
    }

    let parametri = progetto
        .contratto
        .as_ref()
        .map(|c| c.parametri.clone())
        .unwrap_or_default();
    for reg in &progetto.contabilita.libretto().registrazioni {
        let percorso = vec![reg.riferimento()];
        for (i, rigo) in reg.righi.iter().enumerate() {
            if rigo.vedi_voce.as_ref().and_then(VediVoce::voce).is_some() {
                v.errore(
                    "contabilita.vedi_voce_collegato",
                    "nel libretto il «vedi voce» è statico: la quantità citata va copiata",
                    vec![
                        reg.riferimento(),
                        Riferimento::Rigo {
                            numero: i + 1,
                            id: rigo.id,
                        },
                    ],
                );
            }
        }
        v.partita(
            &reg.prezzo,
            &reg.righi,
            &progetto.regole,
            regole_valide.then_some(None),
            &SenzaRiferimenti,
            &percorso,
        );
        v.assegnazioni(
            &progetto.categorie,
            &progetto.wbs,
            &reg.categorie,
            reg.wbs,
            &percorso,
        );
        if let Some(vc) = reg.voce_computo.filter(|vc| !voci_computo.contains(vc)) {
            v.avviso(
                "contabilita.voce_computo_inesistente",
                format!("la voce di computo collegata {vc} non esiste"),
                percorso.clone(),
            );
        }
        if parametri.manodopera_esclusa_dal_ribasso
            && reg.prezzo.natura == NaturaVoce::Lavoro
            && reg.prezzo.incidenza_manodopera.is_none()
        {
            v.avviso(
                "contabilita.incidenza_manodopera_mancante",
                "manodopera esclusa dal ribasso ma incidenza non indicata: quota considerata nulla",
                percorso,
            );
        }
    }
    v.anticipazione_penali_iva(progetto, &parametri);
    v.conto_finale(progetto);
    v.nuovi_prezzi(progetto);
    if regole_valide {
        v.problemi.extend(
            progetto
                .contabilita
                .verifica_integrita(&parametri, &progetto.regole),
        );
    }
    RapportoValidazione {
        problemi: v.problemi,
    }
}

struct Validatore {
    problemi: Vec<Problema>,
}

impl Validatore {
    /// Limiti di anticipazione (art. 125), penali (art. 126) e IVA dei certificati.
    fn anticipazione_penali_iva(&mut self, progetto: &Progetto, parametri: &ParametriContabili) {
        let contratto = vec![Riferimento::Contratto];
        let netto_contrattuale = progetto
            .contratto
            .as_ref()
            .and_then(|c| c.importo_contrattuale)
            .filter(|i| !i.is_zero());
        let trenta = Decimal::new(30, 0);
        if let Some(a) = &parametri.anticipazione {
            if !a.percentuale_recupero.nell_intervallo_0_100()
                || !a.percentuale.nell_intervallo_0_100()
            {
                self.errore(
                    "contabilita.anticipazione_percentuale",
                    "le percentuali dell'anticipazione devono essere comprese tra 0 e 100",
                    contratto.clone(),
                );
            }
            if a.importo.is_negativo() {
                self.errore(
                    "contabilita.anticipazione_negativa",
                    "l'importo dell'anticipazione non può essere negativo",
                    contratto.clone(),
                );
            }
            let per = |x: Decimal, y: Decimal| x.checked_mul(y).unwrap_or(Decimal::MAX);
            let oltre = match netto_contrattuale {
                Some(n) => per(a.importo.valore(), Decimal::ONE_HUNDRED) > per(n.valore(), trenta),
                None => a.percentuale.valore() > trenta,
            };
            if oltre {
                self.avviso(
                    "contabilita.anticipazione_oltre_30",
                    "anticipazione oltre il 30 % del valore del contratto (art. 125, D.Lgs. 36/2023)",
                    contratto.clone(),
                );
            }
        }
        let penali = progetto.contabilita.penali();
        for p in penali {
            if let Some(pm) = p.per_mille
                && (pm < Decimal::new(5, 1) || pm > Decimal::new(15, 1))
            {
                self.avviso(
                    "contabilita.penale_fuori_misura",
                    format!(
                        "penale n. {}: {pm} per mille fuori dall'intervallo 0,5–1,5 per mille \
                             dell'art. 126 (D.Lgs. 36/2023 come modificato dal D.Lgs. 209/2024)",
                        p.numero
                    ),
                    contratto.clone(),
                );
            }
        }
        if !penali.is_empty() {
            let totale = penali
                .iter()
                .try_fold(Decimal::ZERO, |t, p| t.checked_add(p.importo.valore()))
                .unwrap_or(Decimal::MAX);
            let dieci_volte = totale.checked_mul(Decimal::TEN).unwrap_or(Decimal::MAX);
            match netto_contrattuale {
                Some(n) if dieci_volte > n.valore() => self.errore(
                    "contabilita.penali_oltre_limite",
                    format!(
                        "le penali ({totale} €) superano il 10 % dell'ammontare netto contrattuale \
                         (art. 126, D.Lgs. 36/2023)"
                    ),
                    contratto.clone(),
                ),
                Some(_) => {}
                None => self.informazione(
                    "contabilita.penali_limite_non_verificabile",
                    "importo contrattuale non indicato: il limite del 10 % delle penali non è verificabile",
                    contratto.clone(),
                ),
            }
        }
        match parametri.aliquota_iva {
            Some(a) if !a.nell_intervallo_0_100() => self.errore(
                "contabilita.aliquota_iva",
                "l'aliquota IVA deve essere compresa tra 0 e 100",
                contratto,
            ),
            None if !progetto.contabilita.sal().is_empty() => self.informazione(
                "contabilita.iva_non_indicata",
                "aliquota IVA non indicata nel contratto: i certificati di pagamento sono senza IVA",
                contratto,
            ),
            _ => {}
        }
    }

    /// Nuovi prezzi e verbali di concordamento (Allegato II.14, art. 5).
    fn nuovi_prezzi(&mut self, progetto: &Progetto) {
        let elenco = &progetto.elenco_prezzi;
        let rif_p = Riferimento::Prezzario { id: elenco.id };
        let contabilita = &progetto.contabilita;
        let mut numeri = BTreeSet::new();
        let mut per_verbale: std::collections::BTreeMap<u32, usize> =
            std::collections::BTreeMap::new();
        for voce in &elenco.voci {
            let Some(np) = &voce.nuovo_prezzo else {
                continue;
            };
            let percorso = vec![
                rif_p.clone(),
                Riferimento::VocePrezzario {
                    id: voce.id,
                    codice: voce.codice.clone(),
                },
            ];
            if np.numero == 0 || !numeri.insert(np.numero) {
                self.errore(
                    "prezzario.nuovo_prezzo_numero",
                    format!("numero {} del nuovo prezzo nullo o duplicato", np.sigla()),
                    percorso.clone(),
                );
            }
            if np.criterio == CriterioNuovoPrezzo::Analisi && voce.analisi.is_none() {
                self.informazione(
                    "prezzario.nuovo_prezzo_senza_analisi",
                    format!(
                        "{} formato da analisi: l'analisi non è ancora stata redatta",
                        np.sigla()
                    ),
                    percorso.clone(),
                );
            }
            if !np.soggetto_a_ribasso {
                self.informazione(
                    "prezzario.nuovo_prezzo_escluso_dal_ribasso",
                    format!(
                        "{} indicato come già netto: nei SAL è escluso dal ribasso d'asta",
                        np.sigla()
                    ),
                    percorso.clone(),
                );
            }
            let verbale = np.verbale.and_then(|n| contabilita.verbale_nuovi_prezzi(n));
            if let Some(n) = np.verbale {
                *per_verbale.entry(n).or_insert(0) += 1;
                if verbale.is_none() {
                    self.errore(
                        "prezzario.verbale_inesistente",
                        format!("{} riferisce il verbale n. {n} inesistente", np.sigla()),
                        percorso.clone(),
                    );
                }
            }
            let usato = contabilita.libretto().registrazioni.iter().any(|r| {
                r.prezzo
                    .origine
                    .map_or(r.prezzo.codice == voce.codice, |o| {
                        o.prezzario == elenco.id && o.voce == voce.id
                    })
            });
            if usato && !verbale.is_some_and(|v| v.approvato()) {
                self.avviso(
                    "contabilita.nuovo_prezzo_non_approvato",
                    format!(
                        "{} usato nel libretto delle misure ma non ancora approvato in un verbale \
                         di concordamento",
                        np.sigla()
                    ),
                    percorso,
                );
            }
        }
        let mut visti = BTreeSet::new();
        for v in contabilita.verbali_nuovi_prezzi() {
            let percorso = vec![Riferimento::Contratto];
            let nome = format!("verbale nuovi prezzi n. {}", v.numero);
            if !visti.insert(v.numero) {
                self.errore(
                    "contabilita.verbale_numero_duplicato",
                    format!("{nome} duplicato"),
                    percorso.clone(),
                );
            }
            if v.firma_esecutore.is_some_and(|d| d < v.data)
                || v.approvato_il.is_some_and(|d| d < v.data)
            {
                self.errore(
                    "contabilita.verbale_date",
                    format!("{nome}: firma o approvazione anteriori al verbale"),
                    percorso.clone(),
                );
            }
            if !per_verbale.contains_key(&v.numero) {
                self.avviso(
                    "contabilita.verbale_vuoto",
                    format!("{nome} senza nuovi prezzi"),
                    percorso.clone(),
                );
            }
            match (&v.firma_esecutore, &v.riserve) {
                (None, _) => self.informazione(
                    "contabilita.verbale_non_firmato",
                    format!(
                        "{nome} non firmato dall'esecutore: i prezzi possono essere ammessi in \
                         contabilità, l'esecutore può iscrivere riserva"
                    ),
                    percorso,
                ),
                (Some(_), Some(_)) => self.informazione(
                    "contabilita.verbale_con_riserve",
                    format!("{nome} firmato con riserve"),
                    percorso,
                ),
                (Some(_), None) => {}
            }
        }
    }

    /// Termini del conto finale, della firma e del collaudo (Allegato II.14, art. 116).
    fn conto_finale(&mut self, progetto: &Progetto) {
        let Some(c) = progetto.contabilita.conto_finale() else {
            return;
        };
        let conto = || vec![Riferimento::ContoFinale];
        for r in &progetto.contabilita.libretto().registrazioni {
            if r.data > c.data_ultimazione {
                self.avviso(
                    "contabilita.registrazione_dopo_ultimazione",
                    format!(
                        "misurazione del {} successiva all'ultimazione dei lavori ({})",
                        r.data, c.data_ultimazione
                    ),
                    vec![Riferimento::ContoFinale, r.riferimento()],
                );
            }
        }
        if c.stato == crate::contabilita::StatoSal::Emesso {
            match (&c.firma_esecutore, &c.riserve) {
                (None, _) => self.informazione(
                    "contabilita.conto_finale_da_firmare",
                    "conto finale in attesa della firma dell'esecutore: entro trenta giorni \
                     dall'invito del RUP, altrimenti si intende accettato",
                    conto(),
                ),
                (Some(_), Some(_)) => self.informazione(
                    "contabilita.conto_finale_riserve",
                    "l'esecutore ha firmato il conto finale confermando delle riserve",
                    conto(),
                ),
                (Some(_), None) => {}
            }
        }
        let Some(collaudo) = &c.collaudo else {
            return;
        };
        let entro = |mesi: u32| {
            c.data_ultimazione
                .checked_add_months(chrono::Months::new(mesi))
        };
        match collaudo.tipo {
            TipoCollaudo::RegolareEsecuzione => {
                if entro(3).is_some_and(|limite| collaudo.data > limite) {
                    self.avviso(
                        "contabilita.cre_oltre_tre_mesi",
                        "certificato di regolare esecuzione emesso oltre tre mesi \
                         dall'ultimazione dei lavori (Allegato II.14, art. 28)",
                        conto(),
                    );
                }
                let milione = Importo::new(Decimal::new(1_000_000, 0));
                if progetto
                    .contratto
                    .as_ref()
                    .and_then(|x| x.importo_contrattuale)
                    .is_some_and(|i| i.valore() > milione.valore())
                {
                    self.informazione(
                        "contabilita.cre_oltre_un_milione",
                        "certificato di regolare esecuzione per lavori oltre 1 milione di euro: \
                         verificare che l'Allegato II.14 lo consenta in luogo del collaudo",
                        conto(),
                    );
                }
            }
            TipoCollaudo::Collaudo => {
                if entro(12).is_some_and(|limite| collaudo.data > limite) {
                    self.avviso(
                        "contabilita.collaudo_oltre_sei_mesi",
                        "collaudo concluso oltre un anno dall'ultimazione dei lavori \
                         (art. 116, comma 2)",
                        conto(),
                    );
                } else if entro(6).is_some_and(|limite| collaudo.data > limite) {
                    self.avviso(
                        "contabilita.collaudo_oltre_sei_mesi",
                        "collaudo concluso oltre sei mesi dall'ultimazione dei lavori: \
                         ammesso fino a un anno solo nei casi di particolare complessità \
                         (art. 116, comma 2)",
                        conto(),
                    );
                }
            }
        }
        if collaudo.approvato_il.is_none() {
            self.informazione(
                "contabilita.collaudo_da_approvare",
                format!(
                    "{} da approvare: la rata di saldo e lo svincolo delle ritenute seguono \
                     l'approvazione",
                    collaudo.tipo.nome()
                ),
                conto(),
            );
        }
    }

    /// Cronoprogramma: riferimenti a nodi e attività esistenti, avanzamenti, legami
    /// senza cicli (ADR 0024).
    fn cronoprogramma(&mut self, p: &Progetto) {
        use crate::cronoprogramma::RifAttivita;
        let c = &p.cronoprogramma;
        let percorso = || vec![Riferimento::Cronoprogramma];
        let esiste = |r: RifAttivita| match r {
            RifAttivita::Wbs(n) => p.wbs.nodo(n).is_some(),
            RifAttivita::Attivita(a) => c.attivita_aggiunta(a).is_some(),
        };
        for a in &c.attivita_wbs {
            if p.wbs.nodo(a.nodo).is_none() {
                self.avviso(
                    "cronoprogramma.nodo_inesistente",
                    format!(
                        "dati del cronoprogramma per il nodo WBS inesistente {}",
                        a.nodo
                    ),
                    percorso(),
                );
            }
        }
        for a in &c.attivita {
            if a.padre.is_some_and(|n| p.wbs.nodo(n).is_none()) {
                self.avviso(
                    "cronoprogramma.padre_inesistente",
                    format!("l'attività «{}» è sotto un nodo WBS inesistente", a.nome),
                    percorso(),
                );
            }
        }
        let tutte = c
            .attivita_wbs
            .iter()
            .map(|a| (a.dati.avanzamento, a.nodo.to_string()))
            .chain(
                c.attivita
                    .iter()
                    .map(|a| (a.dati.avanzamento, a.nome.clone())),
            );
        for (avanzamento, nome) in tutte {
            if avanzamento > 100 {
                self.avviso(
                    "cronoprogramma.avanzamento_non_valido",
                    format!("avanzamento di {nome} oltre il 100 %"),
                    percorso(),
                );
            }
        }
        for l in &c.legami {
            if !esiste(l.da) || !esiste(l.a) {
                self.avviso(
                    "cronoprogramma.legame_orfano",
                    "legame del cronoprogramma con un'attività inesistente",
                    percorso(),
                );
            } else if l.da == l.a {
                self.avviso(
                    "cronoprogramma.legame_su_se_stesso",
                    "legame del cronoprogramma di un'attività con sé stessa",
                    percorso(),
                );
            }
        }
        if let Err(e) = crate::cronoprogramma::verifica_legami(p) {
            self.errore("cronoprogramma.ciclo", e.to_string(), percorso());
        }
        if c.livello_dettaglio == Some(0) {
            self.avviso(
                "cronoprogramma.livello_non_valido",
                "livello di dettaglio 0: si usano tutti i livelli",
                percorso(),
            );
        }
        if !c.calendario.ha_giorni_lavorativi() {
            self.errore(
                "cronoprogramma.calendario_vuoto",
                "il calendario del cronoprogramma non ha giorni lavorativi",
                percorso(),
            );
        }
    }

    fn errore(&mut self, codice: &str, messaggio: impl Into<String>, percorso: Vec<Riferimento>) {
        self.problemi
            .push(Problema::errore(codice, messaggio, percorso));
    }

    fn avviso(&mut self, codice: &str, messaggio: impl Into<String>, percorso: Vec<Riferimento>) {
        self.problemi
            .push(Problema::avviso(codice, messaggio, percorso));
    }

    fn informazione(
        &mut self,
        codice: &str,
        messaggio: impl Into<String>,
        percorso: Vec<Riferimento>,
    ) {
        self.problemi
            .push(Problema::informazione(codice, messaggio, percorso));
    }

    fn percentuale(&mut self, nome: &str, p: Option<Percentuale>, percorso: &[Riferimento]) {
        if let Some(p) = p.filter(|p| !p.nell_intervallo_0_100()) {
            self.errore(
                "percentuale_fuori_intervallo",
                format!("{nome} del {p} % fuori dall'intervallo [0, 100]"),
                percorso.to_vec(),
            );
        }
    }

    fn prezzario(&mut self, p: &Prezzario) {
        let rif_p = Riferimento::Prezzario { id: p.id };
        let mut id = BTreeSet::new();
        let mut codici = BTreeSet::new();
        for voce in &p.voci {
            let percorso = vec![
                rif_p.clone(),
                Riferimento::VocePrezzario {
                    id: voce.id,
                    codice: voce.codice.clone(),
                },
            ];
            if !id.insert(voce.id) {
                self.errore(
                    "prezzario.id_duplicato",
                    "identificativo duplicato",
                    percorso.clone(),
                );
            }
            if voce.codice.trim().is_empty() {
                self.errore(
                    "prezzario.codice_vuoto",
                    "codice mancante",
                    percorso.clone(),
                );
            } else if !codici.insert(voce.codice.as_str()) {
                self.errore(
                    "prezzario.codice_duplicato",
                    "codice duplicato",
                    percorso.clone(),
                );
            }
            if voce.unita_misura.trim().is_empty() {
                self.errore(
                    "prezzario.unita_misura_vuota",
                    "unità di misura mancante",
                    percorso.clone(),
                );
            }
            if voce.descrizione_breve.trim().is_empty() && voce.descrizione_estesa.trim().is_empty()
            {
                self.avviso(
                    "prezzario.descrizione_vuota",
                    "descrizione mancante",
                    percorso.clone(),
                );
            }
            if voce.prezzo.is_negativo() {
                self.avviso(
                    "prezzario.prezzo_negativo",
                    "prezzo unitario negativo",
                    percorso.clone(),
                );
            }
            self.percentuale("incidenza manodopera", voce.incidenza_manodopera, &percorso);
            self.percentuale("incidenza sicurezza", voce.incidenza_sicurezza, &percorso);
            if let Some(c) = voce
                .capitolo
                .filter(|c| !p.capitoli.iter().any(|x| x.id == *c))
            {
                self.errore(
                    "prezzario.capitolo_inesistente",
                    format!("capitolo {c} inesistente"),
                    percorso.clone(),
                );
            }
            if let Some(a) = voce.analisi.filter(|a| p.analisi(*a).is_none()) {
                self.errore(
                    "prezzario.analisi_inesistente",
                    format!("analisi {a} inesistente"),
                    percorso,
                );
            }
        }
        // Albero dei capitoli: padre esistente e nessun ciclo (un file danneggiato non
        // deve dare un albero senza radice).
        let padri: std::collections::HashMap<_, _> =
            p.capitoli.iter().map(|c| (c.id, c.padre)).collect();
        for c in &p.capitoli {
            let percorso = vec![rif_p.clone()];
            let Some(padre) = c.padre else { continue };
            if !padri.contains_key(&padre) {
                self.avviso(
                    "prezzario.capitolo_padre_inesistente",
                    format!(
                        "capitolo «{}»: capitolo padre {padre} inesistente",
                        c.codice
                    ),
                    percorso,
                );
                continue;
            }
            let mut visti = BTreeSet::from([c.id]);
            let mut corrente = Some(padre);
            while let Some(id) = corrente {
                if !visti.insert(id) {
                    self.avviso(
                        "prezzario.capitoli_in_ciclo",
                        format!("capitolo «{}»: i capitoli padre formano un ciclo", c.codice),
                        percorso,
                    );
                    break;
                }
                corrente = padri.get(&id).copied().flatten();
            }
        }
        for a in &p.analisi {
            let percorso = vec![
                rif_p.clone(),
                Riferimento::Analisi {
                    id: a.id,
                    codice: a.codice.clone(),
                },
            ];
            self.percentuale("spese generali", Some(a.spese_generali), &percorso);
            self.percentuale("utile d'impresa", Some(a.utile_impresa), &percorso);
            self.percentuale(
                "quota degli oneri aziendali della sicurezza",
                a.quota_oneri_sicurezza,
                &percorso,
            );
        }
    }

    /// Analisi del prezzario: calcolabilità (anche delle sotto-analisi), righe con
    /// origine inesistente, prezzi non allineati, risorse e percentuali.
    fn analisi_prezzi(&mut self, p: &Prezzario, regole: &RegoleArrotondamento) {
        let rif_p = Riferimento::Prezzario { id: p.id };
        let mut codici = BTreeSet::new();
        for r in &p.risorse {
            let percorso = vec![
                rif_p.clone(),
                Riferimento::Risorsa {
                    id: r.id,
                    codice: r.codice.clone(),
                },
            ];
            if !codici.insert(r.codice.as_str()) {
                self.avviso(
                    "prezzario.risorsa_codice_duplicato",
                    "codice di risorsa duplicato",
                    percorso.clone(),
                );
            }
            if r.prezzo.valore().is_sign_negative() {
                self.errore(
                    "prezzario.risorsa_prezzo_negativo",
                    "prezzo della risorsa negativo",
                    percorso,
                );
            }
        }
        let tredici = Decimal::new(13, 0);
        let diciassette = Decimal::new(17, 0);
        for a in &p.analisi {
            let percorso = vec![
                rif_p.clone(),
                Riferimento::Analisi {
                    id: a.id,
                    codice: a.codice.clone(),
                },
            ];
            for (i, riga) in a.righe.iter().enumerate() {
                let orfana = match riga.origine {
                    Some(OrigineRigaAnalisi::Risorsa(r)) => p.risorsa(r).is_none(),
                    Some(OrigineRigaAnalisi::VocePrezzario(v)) => p.voce(v).is_none(),
                    None => false,
                };
                if orfana {
                    let mut percorso = percorso.clone();
                    percorso.push(Riferimento::RigaAnalisi { numero: i + 1 });
                    self.avviso(
                        "prezzario.riga_analisi_orfana",
                        "la risorsa o la voce della riga non esiste più: vale il prezzo copiato",
                        percorso,
                    );
                }
            }
            let sg = a.spese_generali.valore();
            if sg < tredici || sg > diciassette {
                self.informazione(
                    "prezzario.spese_generali_insolite",
                    format!(
                        "spese generali del {} %: nei prezzari pubblici sono di norma tra il 13 \
                         e il 17 %",
                        a.spese_generali
                    ),
                    percorso.clone(),
                );
            }
            if let Err(e) = p.calcola_analisi(a.id, regole) {
                let ciclica = e.to_string().contains("ciclica");
                self.errore(
                    if ciclica {
                        "prezzario.analisi_ciclica"
                    } else {
                        "prezzario.analisi_non_calcolabile"
                    },
                    e.to_string(),
                    percorso,
                );
            }
        }
        if let Ok(diversi) = p.prezzi_non_allineati(regole) {
            for (id, prezzo, calcolato) in diversi {
                let codice = p.voce(id).map(|v| v.codice.clone()).unwrap_or_default();
                self.informazione(
                    "prezzario.prezzo_analisi_non_allineato",
                    format!(
                        "prezzo {prezzo} diverso da quello dell'analisi ({calcolato}): applicare \
                         l'analisi per aggiornarlo"
                    ),
                    vec![rif_p.clone(), Riferimento::VocePrezzario { id, codice }],
                );
            }
        }
    }

    fn categorie(&mut self, c: &Categorie) {
        for livello in LivelloCategoria::TUTTI {
            let mut id = BTreeSet::new();
            let mut codici = BTreeSet::new();
            for classe in c.elenco(livello) {
                let percorso = vec![Riferimento::Classe {
                    livello,
                    id: classe.id,
                }];
                if !id.insert(classe.id) {
                    self.errore(
                        "categorie.id_duplicato",
                        "identificativo duplicato",
                        percorso.clone(),
                    );
                }
                if !codici.insert(classe.codice.as_str()) {
                    self.avviso("categorie.codice_duplicato", "codice duplicato", percorso);
                }
            }
        }
    }

    fn assegnazioni(
        &mut self,
        categorie: &Categorie,
        wbs: &StrutturaWbs,
        a: &AssegnazioneCategorie,
        nodo: Option<WbsNodoId>,
        percorso: &[Riferimento],
    ) {
        for livello in LivelloCategoria::TUTTI {
            if let Some(id) = a
                .di(livello)
                .filter(|id| categorie.trova(livello, *id).is_none())
            {
                self.errore(
                    "categorie.riferimento_inesistente",
                    format!("{livello} {id} inesistente"),
                    percorso.to_vec(),
                );
            }
        }
        if let Some(n) = nodo.filter(|n| wbs.nodo(*n).is_none()) {
            self.errore(
                "wbs.riferimento_inesistente",
                format!("nodo WBS {n} inesistente"),
                percorso.to_vec(),
            );
        }
    }

    fn allineamento_elenco(
        &mut self,
        elenco: &Prezzario,
        prezzo: &PrezzoApplicato,
        percorso: &[Riferimento],
    ) {
        let Some(origine) = prezzo.origine.filter(|o| o.prezzario == elenco.id) else {
            return;
        };
        match elenco.voce(origine.voce) {
            None => self.informazione(
                "prezzo.origine_rimossa",
                format!(
                    "la voce di origine «{}» non è più nell'elenco prezzi",
                    prezzo.codice
                ),
                percorso.to_vec(),
            ),
            Some(v) if !prezzo.confronta(v).is_empty() => self.informazione(
                "prezzo.non_allineato",
                format!(
                    "il prezzo applicato «{}» differisce dalla voce attuale dell'elenco prezzi",
                    prezzo.codice
                ),
                percorso.to_vec(),
            ),
            Some(_) => {}
        }
    }

    fn partita(
        &mut self,
        prezzo: &PrezzoApplicato,
        righi: &[RigoMisura],
        regole: &RegoleArrotondamento,
        // `None` con regole non valide (niente calcolo), altrimenti l'eventuale errore
        // già noto della voce (riferimento circolare).
        calcolo: Option<Option<&ErroreLocalizzato>>,
        riferite: &dyn QuantitaRiferite,
        percorso: &[Riferimento],
    ) {
        if prezzo.codice.trim().is_empty() {
            self.avviso("prezzo.codice_vuoto", "codice mancante", percorso.to_vec());
        }
        if prezzo.unita_misura.trim().is_empty() {
            self.errore(
                "prezzo.unita_misura_vuota",
                "unità di misura mancante",
                percorso.to_vec(),
            );
        }
        if prezzo.prezzo.is_negativo() {
            self.avviso(
                "prezzo.negativo",
                "prezzo unitario negativo",
                percorso.to_vec(),
            );
        }
        self.percentuale(
            "incidenza manodopera",
            prezzo.incidenza_manodopera,
            percorso,
        );
        self.percentuale("incidenza sicurezza", prezzo.incidenza_sicurezza, percorso);
        if righi.is_empty() {
            self.avviso(
                "misura.nessun_rigo",
                "voce priva di righi di misura",
                percorso.to_vec(),
            );
        }
        let mut id = BTreeSet::new();
        for (i, rigo) in righi.iter().enumerate() {
            let mut p = percorso.to_vec();
            p.push(Riferimento::Rigo {
                numero: i + 1,
                id: rigo.id,
            });
            if !id.insert(rigo.id) {
                self.errore("misura.id_duplicato", "identificativo duplicato", p.clone());
            }
            if rigo.tipo == TipoRigo::Misura && !rigo.ha_valori() {
                self.avviso(
                    "misura.rigo_vuoto",
                    "rigo di misura senza valori",
                    p.clone(),
                );
            }
            if rigo.segno == SegnoRigo::Deduzione
                && let Ok(Some(r)) = rigo.calcola_con(regole, riferite)
                && r.valore_esatto > rust_decimal::Decimal::ZERO
            {
                self.avviso(
                    "misura.deduzione_positiva",
                    "deduzione con valori negativi: il contributo risulta positivo",
                    p,
                );
            }
        }
        let Some(noto) = calcolo else {
            return;
        };
        let esito = match noto {
            Some(e) => Err(e.clone()),
            None => calcola_partita_con(prezzo, righi, regole, riferite),
        };
        match esito {
            Ok(c) if c.quantita.is_negativo() => self.avviso(
                "misura.quantita_negativa",
                format!("quantità totale negativa ({})", c.quantita),
                percorso.to_vec(),
            ),
            Ok(_) => {}
            Err(e) => {
                let mut p = percorso.to_vec();
                p.extend(e.percorso);
                self.errore("calcolo.errore", e.errore.to_string(), p);
            }
        }
    }
}
