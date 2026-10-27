//! Test di integrazione: computo metrico estimativo.

// Le opzioni `allow-*-in-tests` di clippy.toml coprono solo le funzioni `#[test]`:
// negli helper condivisi (`common`) e nei costruttori di dati di prova un errore
// deve far fallire il test immediatamente.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::prelude::*;
use opencomputo_core::riferimento::Riferimento;
use rust_decimal_macros::dec;

fn imp(v: Decimal) -> Importo {
    Importo::new(v)
}

#[test]
fn totali_del_computo_verificati_a_mano() {
    let p = progetto_di_esempio();
    let r = p.calcola_computo(COMPUTO).unwrap();

    let quantita: Vec<Quantita> = r.voci.iter().map(|v| v.calcolo.quantita).collect();
    assert_eq!(
        quantita,
        vec![
            Quantita::new(dec!(126.00)),
            Quantita::new(dec!(51.74)),
            Quantita::new(dec!(109.20)),
            Quantita::new(dec!(100.00)),
        ]
    );
    let importi: Vec<Importo> = r.voci.iter().map(|v| v.calcolo.importo).collect();
    assert_eq!(
        importi,
        vec![
            imp(dec!(1071.00)),
            imp(dec!(7517.82)),
            imp(dec!(4668.30)),
            imp(dec!(1200.00))
        ]
    );
    assert_eq!(r.totale_lavori, imp(dec!(13257.12)));
    assert_eq!(r.totale_oneri_sicurezza, imp(dec!(1200.00)));
    assert_eq!(r.totale, imp(dec!(14457.12)));
    // 267,75 + 1 390,80 + 2 100,74
    assert_eq!(r.totale_manodopera, imp(dec!(3759.29)));
    assert_eq!(r.voci_senza_incidenza_manodopera, 1);
    // Numerazione per posizione.
    assert_eq!(
        r.voci.iter().map(|v| v.numero).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
}

#[test]
fn il_totale_coincide_con_la_somma_delle_voci() {
    let p = progetto_di_esempio();
    let r = p.calcola_computo(COMPUTO).unwrap();
    let somma = Importo::totale(r.voci.iter().map(|v| v.calcolo.importo)).unwrap();
    assert_eq!(somma, r.totale);
    assert_eq!(
        r.totale_lavori.somma(r.totale_oneri_sicurezza).unwrap(),
        r.totale
    );
}

#[test]
fn totali_per_categoria_e_wbs() {
    let p = progetto_di_esempio();
    let r = p.calcola_computo(COMPUTO).unwrap();

    assert_eq!(r.per_categoria.per_chiave[&CAT_SCAVI], imp(dec!(1071.00)));
    assert_eq!(
        r.per_categoria.per_chiave[&CAT_STRUTTURE],
        imp(dec!(7517.82))
    );
    assert_eq!(
        r.per_categoria.per_chiave[&CAT_MURATURE],
        imp(dec!(4668.30))
    );
    assert_eq!(r.per_categoria.non_assegnato, imp(dec!(1200.00)));
    assert_eq!(
        r.per_super_categoria.per_chiave[&SC_EDILI],
        imp(dec!(13257.12))
    );
    assert_eq!(r.per_sotto_categoria.non_assegnato, r.totale);

    assert_eq!(r.per_wbs.per_chiave[&WBS_FONDAZIONI], imp(dec!(8588.82)));
    assert_eq!(r.per_wbs.per_chiave.get(&WBS_EDIFICIO), None);
    let cumulati = r.totali_wbs_cumulati(&p.wbs).unwrap();
    assert_eq!(cumulati[&WBS_EDIFICIO], imp(dec!(13257.12)));
    assert_eq!(cumulati[&WBS_FONDAZIONI], imp(dec!(8588.82)));
    assert_eq!(cumulati[&WBS_ELEVAZIONI], imp(dec!(4668.30)));
    assert_eq!(cumulati[&WBS_SICUREZZA], imp(dec!(1200.00)));
}

#[test]
fn quote_di_manodopera_e_sicurezza_per_categorie_e_wbs() {
    let mut p = progetto_di_esempio();
    let c = p.computi.iter_mut().find(|c| c.id == COMPUTO).unwrap();
    c.voci[1].prezzo.incidenza_sicurezza = Some(Percentuale::new(dec!(3)));
    c.voci[2].prezzo.incidenza_sicurezza = Some(Percentuale::new(dec!(2.5)));
    let r = p.calcola_computo(COMPUTO).unwrap();

    // 7 517,82 × 3 % = 225,5346 → 225,53; 4 668,30 × 2,5 % = 116,7075 → 116,71.
    assert_eq!(r.totale_sicurezza, imp(dec!(342.24)));
    assert_eq!(r.voci_senza_incidenza_sicurezza, 2);
    // Le quote raggruppate sono le somme delle quote arrotondate delle voci.
    let m = &r.manodopera;
    assert_eq!(m.per_categoria.per_chiave[&CAT_SCAVI], imp(dec!(267.75)));
    assert_eq!(
        m.per_categoria.per_chiave[&CAT_STRUTTURE],
        imp(dec!(1390.80))
    );
    assert_eq!(
        m.per_categoria.per_chiave[&CAT_MURATURE],
        imp(dec!(2100.74))
    );
    // La voce degli oneri non ha incidenza: entra nel suo gruppo con quota zero.
    assert_eq!(m.per_categoria.non_assegnato, Importo::ZERO);
    assert_eq!(
        m.per_super_categoria.per_chiave[&SC_EDILI],
        r.totale_manodopera
    );
    assert_eq!(m.per_sotto_categoria.non_assegnato, r.totale_manodopera);
    // Stesse chiavi dei totali degli importi.
    assert_eq!(
        m.per_categoria.per_chiave.keys().collect::<Vec<_>>(),
        r.per_categoria.per_chiave.keys().collect::<Vec<_>>()
    );
    let s = &r.sicurezza;
    assert_eq!(s.per_categoria.per_chiave[&CAT_SCAVI], Importo::ZERO);
    assert_eq!(
        s.per_categoria.per_chiave[&CAT_STRUTTURE],
        imp(dec!(225.53))
    );
    assert_eq!(s.per_wbs.per_chiave[&WBS_FONDAZIONI], imp(dec!(225.53)));
    assert_eq!(s.per_wbs.per_chiave[&WBS_ELEVAZIONI], imp(dec!(116.71)));
    let cumulati = s.per_wbs.cumulati(&p.wbs).unwrap();
    assert_eq!(cumulati[&WBS_EDIFICIO], imp(dec!(342.24)));
    assert_eq!(cumulati[&WBS_SICUREZZA], Importo::ZERO);
    let cumulati = m.per_wbs.cumulati(&p.wbs).unwrap();
    assert_eq!(cumulati[&WBS_EDIFICIO], r.totale_manodopera);
}

#[test]
fn aggiornare_il_prezzario_non_modifica_il_computo() {
    let mut p = progetto_di_esempio();
    let prima = p.calcola_computo(COMPUTO).unwrap();

    // Aggiornamento del listino: +10 % sul calcestruzzo.
    p.elenco_prezzi.voce_mut(CLS).unwrap().prezzo = Prezzo::new(dec!(159.83));
    let dopo = p.calcola_computo(COMPUTO).unwrap();
    assert_eq!(prima, dopo);

    // La validazione segnala il disallineamento come informazione.
    let rapporto = valida_progetto(&p);
    let non_allineati: Vec<_> = rapporto.con_codice("prezzo.non_allineato").collect();
    assert_eq!(non_allineati.len(), 1);
    assert_eq!(non_allineati[0].gravita, Gravita::Informazione);

    // Il riallineamento è esplicito e riporta le differenze.
    let elenco = p.elenco_prezzi.clone();
    let voce = p.computi[0].voce_mut(VC_CLS).unwrap();
    let differenze = voce.prezzo.riallinea(&elenco, elenco.voce(CLS).unwrap());
    assert_eq!(differenze.len(), 1);
    let riallineato = p.calcola_computo(COMPUTO).unwrap();
    // 51,74 × 159,83 = 8 269,6042 → 8 269,60
    assert_eq!(riallineato.voci[1].calcolo.importo, imp(dec!(8269.60)));
}

#[test]
fn la_copia_dal_prezzario_lascia_intatta_la_sorgente() {
    let p = progetto_di_esempio();
    let sorgente = p.elenco_prezzi.clone();
    let mut c = Computo::new(ComputoId::da_u128(77), "Variante");
    c.aggiungi_da_prezzario(VoceComputoId::da_u128(1), &p.elenco_prezzi, SCAVO, None)
        .unwrap();
    c.aggiungi_da_prezzario(VoceComputoId::da_u128(2), &p.elenco_prezzi, SCAVO, Some(0))
        .unwrap();
    assert_eq!(p.elenco_prezzi, sorgente);
    assert_eq!(c.voci.len(), 2);
    assert_eq!(c.voci[0].id, VoceComputoId::da_u128(2));
    let errore = c.aggiungi_da_prezzario(VoceComputoId::da_u128(1), &p.elenco_prezzi, SCAVO, None);
    assert!(matches!(errore, Err(ErroreDominio::IdDuplicato { .. })));
    let errore = c.aggiungi_da_prezzario(
        VoceComputoId::da_u128(3),
        &p.elenco_prezzi,
        VocePrezzarioId::da_u128(9999),
        None,
    );
    assert!(matches!(errore, Err(ErroreDominio::NonTrovato { .. })));
}

#[test]
fn il_totale_non_dipende_dall_ordine_delle_voci() {
    let p = progetto_di_esempio();
    let atteso = p.calcola_computo(COMPUTO).unwrap().totale;
    let base = p.computo(COMPUTO).unwrap().clone();
    let ids: Vec<VoceComputoId> = base.voci.iter().map(|v| v.id).collect();
    // Tutte le rotazioni e alcuni spostamenti.
    for (k, id) in ids.iter().enumerate() {
        let mut c = base.clone();
        c.sposta_voce(*id, (k + 2) % ids.len()).unwrap();
        assert_eq!(c.calcola(&p.regole).unwrap().totale, atteso);
        c.voci.reverse();
        assert_eq!(c.calcola(&p.regole).unwrap().totale, atteso);
    }
}

#[test]
fn errori_localizzati_su_computo_voce_e_rigo() {
    let mut p = progetto_di_esempio();
    let voce = p.computi[0].voce_mut(VC_MURATURA).unwrap();
    voce.righi[2].lunghezza = Some(Fattore::Espressione("12/(3-3)".into()));
    let e = p.calcola_computo(COMPUTO).unwrap_err();
    let ErroreDominio::Calcolo(e) = e else {
        panic!("atteso errore di calcolo, trovato {e:?}");
    };
    assert_eq!(
        e.percorso,
        vec![
            Riferimento::Computo { id: COMPUTO },
            Riferimento::VoceComputo {
                numero: 3,
                id: VC_MURATURA
            },
            Riferimento::Rigo {
                numero: 3,
                id: RigoMisuraId::da_u128(5022)
            },
        ]
    );
    assert_eq!(
        e.to_string(),
        format!(
            "computo {COMPUTO} › voce n. 3 › rigo n. 3: formula non valida: divisione per zero"
        )
    );
}

#[test]
fn regole_di_arrotondamento_alternative() {
    let mut p = progetto_di_esempio();
    // Senza arrotondamento dei parziali i cordoli valgono 3,744 e la voce 51,744 → 51,74:
    // il risultato non cambia; con 3 decimali sulle quantità sì.
    p.regole.decimali_quantita = 3;
    let r = p.calcola_computo(COMPUTO).unwrap();
    assert_eq!(r.voci[1].calcolo.quantita, Quantita::new(dec!(51.744)));
    // 51,744 × 145,30 = 7 518,4032 → 7 518,40
    assert_eq!(r.voci[1].calcolo.importo, imp(dec!(7518.40)));

    p.regole.decimali_quantita = 11;
    assert!(p.calcola_computo(COMPUTO).is_err());
}
