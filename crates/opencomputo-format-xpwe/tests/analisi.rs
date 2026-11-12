//! Import delle analisi dei prezzi (`PweEPAnalisi`): dati sintetici con la struttura
//! osservata in un export reale.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::*;
use opencomputo_core::analisi::{OrigineRigaAnalisi, TipoRisorsa};
use opencomputo_core::id::GeneratoreSequenziale;
use opencomputo_core::prelude::*;
use opencomputo_format_xpwe::{OpzioniImport, RisultatoImport, esporta, importa};
use rust_decimal_macros::dec;

fn componente(idep: &str, descrizione: &str, misura: &str, qt: &str, prezzo: &str) -> String {
    format!(
        "<EPARItem ID=\"1\"><Tipo>0</Tipo><IDEP>{idep}</IDEP><Descrizione>{descrizione}</Descrizione><Misura>{misura}</Misura><Qt>{qt}</Qt><Prezzo>{prezzo}</Prezzo><FieldCTL>0</FieldCTL></EPARItem>"
    )
}

/// Voce di elenco con analisi e incidenze dichiarate (materiali, manodopera, attrezzature).
fn voce(id: &str, tariffa: &str, prezzo: &str, inc: [&str; 3], componenti: &str) -> String {
    format!(
        "<EPItem ID=\"{id}\"><Tariffa>{tariffa}</Tariffa><DesRidotta>Voce {tariffa}</DesRidotta><DesEstesa>Voce {tariffa}</DesEstesa><UnMisura>mc</UnMisura><Prezzo1>{prezzo}</Prezzo1><IncMAT>{}</IncMAT><IncMDO>{}</IncMDO><IncATTR>{}</IncATTR><PweEPAnalisi><PweEPAR>{componenti}</PweEPAR></PweEPAnalisi></EPItem>",
        inc[0], inc[1], inc[2]
    )
}

fn prova(elenco: &str) -> RisultatoImport {
    importa(
        &documento(elenco, ""),
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
    .unwrap()
}

fn analisi_di<'a>(
    r: &'a RisultatoImport,
    codice: &str,
) -> &'a opencomputo_core::analisi::AnalisiPrezzo {
    let p = &r.progetto.elenco_prezzi;
    let v = p.voci.iter().find(|v| v.codice == codice).unwrap();
    p.analisi(v.analisi.expect("voce senza analisi")).unwrap()
}

#[test]
fn analisi_importata_con_percentuali_e_tipi_dedotti() {
    // Costo 383,47 + 26,54 + 319,10 = 729,11; × 1,17 × 1,10 = 938,37.
    let componenti = format!(
        "{}{}{}",
        componente("-2", "Legno massello\n   di abete", "mc", "1,05", "365,21"),
        componente("-2", "Operaio comune", "h", "1", "26,54"),
        componente("-2", "Operaio specializzato", "h", "10", "31,91"),
    );
    let r = prova(&voce(
        "1",
        "NP.01",
        "938.37",
        ["40.8658", "36.8343", "0"],
        &componenti,
    ));
    assert_eq!(r.rapporto.statistiche.analisi, 1);
    let a = analisi_di(&r, "NP.01");
    assert_eq!(
        (a.spese_generali, a.utile_impresa),
        (Percentuale::new(dec!(17)), Percentuale::new(dec!(10)))
    );
    assert_eq!(a.unita_misura, "mc");
    assert_eq!(a.righe.len(), 3);
    assert_eq!(a.righe[0].descrizione, "Legno massello di abete");
    assert_eq!(
        (a.righe[0].quantita, a.righe[0].prezzo),
        (Quantita::new(dec!(1.05)), Prezzo::new(dec!(365.21)))
    );
    assert_eq!(
        a.righe.iter().map(|x| x.tipo).collect::<Vec<_>>(),
        [
            TipoRisorsa::Materiale,
            TipoRisorsa::Manodopera,
            TipoRisorsa::Manodopera
        ]
    );
    // Il calcolo del core con l'analisi importata dà il prezzo del file: nulla da allineare.
    let p = &r.progetto.elenco_prezzi;
    assert_eq!(
        a.calcola(&r.progetto.regole).unwrap().prezzo_unitario,
        Prezzo::new(dec!(938.37))
    );
    assert!(
        p.prezzi_non_allineati(&r.progetto.regole)
            .unwrap()
            .is_empty()
    );
    let c = codici(&r);
    assert!(c.contains(&"xpwe.analisi_importata".to_owned()), "{c:?}");
    assert!(
        !c.iter()
            .any(|x| x == "xpwe.analisi_non_importata" || x == "xpwe.analisi_tipi_presunti"),
        "{c:?}"
    );
    let m = &r
        .rapporto
        .con_codice("xpwe.analisi_importata")
        .next()
        .unwrap()
        .messaggio;
    assert!(
        m.contains("17 %") && m.contains("10 %") && m.contains("non sono nel file"),
        "{m}"
    );
    assert!(!valida_progetto(&r.progetto).ha_errori());
    let json = opencomputo_core::schema::esporta_json(&r.progetto).unwrap();
    assert_eq!(
        opencomputo_core::schema::importa_json(&json).unwrap(),
        r.progetto
    );
}

#[test]
fn tipi_stabiliti_dalle_incidenze_e_costi_della_sicurezza_senza_utile() {
    // Un tecnico a ore che le incidenze dichiarate non contano come manodopera, e
    // un'attrezzatura. Costo 185,77 × 1,17 × 1,10 = 239,09 (senza arrotondamenti intermedi).
    let componenti = format!(
        "{}{}{}{}",
        componente("-2", "Tecnico per la prova", "h", "1,5", "75"),
        componente("-2", "Operaio comune", "h", "0,5", "26,54"),
        componente("-2", "Attrezzatura di prova", "", "1", "50"),
        componente("-2", "Consumabili", "", "1", "10"),
    );
    let sicurezza = format!(
        "{}{}",
        componente("-2", "Cartelli", "", "1", "170"),
        componente("-2", "Operaio comune", "h", "1", "26,54"),
    );
    let r = prova(&format!(
        "{}{}",
        voce(
            "1",
            "NP.05",
            "239.09",
            ["4.1826", "5.5503", "20.913"],
            &componenti
        ),
        voce(
            "2",
            "NP.SIC.03",
            "229.95",
            ["73.9285", "11.5415", "0"],
            &sicurezza
        ),
    ));
    assert_eq!(
        analisi_di(&r, "NP.05")
            .righe
            .iter()
            .map(|x| x.tipo)
            .collect::<Vec<_>>(),
        [
            TipoRisorsa::Altro,
            TipoRisorsa::Manodopera,
            TipoRisorsa::Nolo,
            TipoRisorsa::Materiale
        ]
    );
    let s = analisi_di(&r, "NP.SIC.03");
    assert_eq!(
        (s.spese_generali, s.utile_impresa),
        (Percentuale::new(dec!(17)), Percentuale::ZERO)
    );
    // Una segnalazione per ogni coppia di percentuali.
    assert_eq!(r.rapporto.con_codice("xpwe.analisi_importata").count(), 2);
}

#[test]
fn differenza_di_arrotondamento_dichiarata_e_prezzo_del_file_conservato() {
    // 160 + 40 + 26,54 + 15,955 = 242,495; × 1,287 = 312,09 senza arrotondare i componenti.
    // Arrotondando 15,955 a 15,96 il core dà 312,10.
    let componenti = format!(
        "{}{}{}{}",
        componente("-2", "Zavorre", "", "1", "160"),
        componente("-2", "Movimentazione", "", "1", "40"),
        componente("-2", "Operaio comune", "h", "1", "26,54"),
        componente("-2", "Operaio specializzato", "h", "0,5", "31,91"),
    );
    let r = prova(&voce(
        "1",
        "NP.03",
        "312.09",
        ["51.2671", "13.6162", "12.8168"],
        &componenti,
    ));
    let a = analisi_di(&r, "NP.03");
    assert_eq!(
        (a.spese_generali, a.utile_impresa),
        (Percentuale::new(dec!(17)), Percentuale::new(dec!(10)))
    );
    let p = &r.progetto.elenco_prezzi;
    assert_eq!(
        p.voci[0].prezzo,
        Prezzo::new(dec!(312.09)),
        "resta il prezzo dichiarato"
    );
    let m: Vec<_> = r
        .rapporto
        .con_codice("xpwe.analisi_arrotondamento")
        .collect();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].gravita, Gravita::Informazione);
    assert!(
        m[0].messaggio.contains("312.10") && m[0].messaggio.contains("312.09"),
        "{}",
        m[0].messaggio
    );
    assert_eq!(p.prezzi_non_allineati(&r.progetto.regole).unwrap().len(), 1);
}

#[test]
fn percentuali_non_deducibili_avviso_e_nessuna_maggiorazione() {
    let r = prova(&voce(
        "1",
        "X.01",
        "777",
        ["0", "0", "0"],
        &componente("-2", "Materiale", "kg", "2", "100"),
    ));
    let a = analisi_di(&r, "X.01");
    assert_eq!(
        (a.spese_generali, a.utile_impresa),
        (Percentuale::ZERO, Percentuale::ZERO)
    );
    let m: Vec<_> = r
        .rapporto
        .con_codice("xpwe.analisi_prezzo_diverso")
        .collect();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].gravita, Gravita::Avviso);
    assert!(
        m[0].messaggio.contains("200") && m[0].messaggio.contains("777"),
        "{}",
        m[0].messaggio
    );
    assert_eq!(
        r.progetto.elenco_prezzi.voci[0].prezzo,
        Prezzo::new(dec!(777))
    );
    // Senza incidenze dichiarate il tipo è presunto, e lo si dice.
    assert_eq!(a.righe[0].tipo, TipoRisorsa::Materiale);
    assert_eq!(
        r.rapporto.con_codice("xpwe.analisi_tipi_presunti").count(),
        1
    );
}

#[test]
fn analisi_non_importabili_restano_conservate() {
    for (componenti, motivo) in [
        (
            componente("-1", "MATERIALI", "", "", ""),
            "nessun componente",
        ),
        (
            componente("-2", "Materiale", "kg", "due", "100"),
            "non validi",
        ),
        (
            componente("-2", "Materiale", "kg", "-1", "100"),
            "non validi",
        ),
        (componente("-2", "Materiale", "kg", "1", ""), "non validi"),
    ] {
        let r = prova(&voce("1", "X.01", "100", ["0", "0", "0"], &componenti));
        assert_eq!(r.rapporto.statistiche.analisi, 0, "{motivo}");
        assert!(r.progetto.elenco_prezzi.analisi.is_empty());
        assert_eq!(r.progetto.elenco_prezzi.voci[0].analisi, None);
        let m = &r
            .rapporto
            .con_codice("xpwe.analisi_non_importata")
            .next()
            .unwrap()
            .messaggio;
        assert!(m.contains(motivo), "{m}");
    }
    // Contenitore vuoto e voce senza analisi: nessuna segnalazione.
    let r = prova(&voce("1", "X.01", "100", ["0", "0", "0"], ""));
    assert_eq!(codici(&r), Vec::<String>::new());
}

#[test]
fn titolo_di_gruppo_saltato_e_componente_riferito_a_un_altra_voce() {
    // Il componente rimanda alla voce 2, che nel file viene dopo: codice, descrizione e
    // unità mancanti vengono da lì. 2 × 50 = 100; × 1,15 × 1,10 = 126,50.
    let componenti = format!(
        "{}{}",
        componente("-1", "MATERIALI", "", "", ""),
        componente("2", "", "", "2", "50"),
    );
    let r = prova(&format!(
        "{}{}",
        voce("1", "A.01", "126.50", ["0", "0", "0"], &componenti),
        ep("2", "M.01", "50"),
    ));
    let a = analisi_di(&r, "A.01");
    assert_eq!(a.righe.len(), 1);
    let riferita = r
        .progetto
        .elenco_prezzi
        .voci
        .iter()
        .find(|v| v.codice == "M.01")
        .unwrap();
    assert_eq!(a.righe[0].codice, "M.01");
    assert_eq!(a.righe[0].unita_misura, "m");
    assert_eq!(
        a.righe[0].origine,
        Some(OrigineRigaAnalisi::VocePrezzario(riferita.id))
    );
    assert_eq!(
        (a.spese_generali, a.utile_impresa),
        (Percentuale::new(dec!(15)), Percentuale::new(dec!(10)))
    );
    assert!(
        r.progetto
            .elenco_prezzi
            .prezzi_non_allineati(&r.progetto.regole)
            .unwrap()
            .is_empty()
    );
    // Un componente che rimanda alla propria voce non crea un ciclo.
    let r = prova(&voce(
        "1",
        "A.01",
        "115",
        ["0", "0", "0"],
        &componente("1", "Sé stessa", "m", "1", "100"),
    ));
    assert_eq!(analisi_di(&r, "A.01").righe[0].origine, None);
    assert!(
        r.progetto
            .elenco_prezzi
            .prezzi_non_allineati(&r.progetto.regole)
            .is_ok()
    );
}

#[test]
fn l_export_riporta_l_analisi_originale_e_il_reimport_la_ritrova() {
    let componenti = format!(
        "{}{}",
        componente("-2", "Legno", "mc", "1,05", "365,21"),
        componente("-2", "Operaio", "h", "11", "31,42"),
    );
    let r = prova(&voce("1", "NP.01", "938.37", ["0", "0", "0"], &componenti));
    let prima = analisi_di(&r, "NP.01").clone();
    let xml = esporta(&r.progetto, r.computo).unwrap().xml;
    assert!(
        xml.contains("<Qt>1,05</Qt>") && xml.contains("<Prezzo>365,21</Prezzo>"),
        "analisi non riesportata"
    );
    let r2 = importa(
        xml.as_bytes(),
        &mut GeneratoreSequenziale::default(),
        &OpzioniImport::default(),
    )
    .unwrap();
    let dopo = analisi_di(&r2, "NP.01");
    assert_eq!(dopo.righe, prima.righe);
    assert_eq!(
        (dopo.spese_generali, dopo.utile_impresa),
        (prima.spese_generali, prima.utile_impresa)
    );
}
