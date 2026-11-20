//! Report costruiti dalla fixture v1 del core: valori verificati a mano (vedi
//! `docs/STATO.md`, sezione «Fixture v1») e coerenza tra i formati.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use opencomputo_core::data::data_amg;
use opencomputo_core::id::{ComputoId, GeneratoreId, GeneratoreSequenziale, SalId};
use opencomputo_core::misura::Fattore;
use opencomputo_core::progetto::Progetto;
use opencomputo_core::schema::importa_json;
use opencomputo_io::documento::{Documento, StileRiga, Tabella, Valore};
use opencomputo_io::report::{self, ErroreReport, OpzioniReport};
use opencomputo_io::{csv, pdf, xlsx};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json");

fn progetto() -> Progetto {
    importa_json(FIXTURE).unwrap()
}

fn opzioni() -> OpzioniReport {
    OpzioniReport {
        data_stampa: data_amg(2026, 10, 20),
    }
}

fn computo_id(p: &Progetto) -> ComputoId {
    p.computi[0].id
}

/// Valore numerico della cella, se presente.
fn numero(v: &Valore) -> Option<Decimal> {
    match v {
        Valore::Numero { valore, .. } => Some(*valore),
        _ => None,
    }
}

fn testo(v: &Valore) -> String {
    v.in_testo()
}

/// Righe con lo stile indicato.
fn righe(t: &Tabella, stile: StileRiga) -> Vec<&opencomputo_io::documento::Riga> {
    t.righe.iter().filter(|r| r.stile == stile).collect()
}

/// Prima cella numerica (l'importo) della riga di totale con l'etichetta indicata.
fn totale(t: &Tabella, etichetta: &str) -> Decimal {
    t.righe
        .iter()
        .find(|r| r.stile == StileRiga::Totale && r.celle.iter().any(|c| testo(c) == etichetta))
        .and_then(|r| r.celle.iter().find_map(numero))
        .unwrap_or_else(|| panic!("totale «{etichetta}» assente"))
}

#[test]
fn computo_con_valori_verificati_a_mano() {
    let p = progetto();
    let doc = report::computo(&p, computo_id(&p), &opzioni()).unwrap();
    assert_eq!(doc.titolo, "Computo metrico estimativo");
    assert!(doc.orizzontale);
    assert_eq!(doc.piede.as_deref(), Some("Stampato il 20/10/2026"));
    assert_eq!(
        doc.intestazione[0],
        ("Oggetto".into(), "Nuova scuola primaria".into())
    );
    let titoli: Vec<_> = doc.sezioni.iter().map(|s| s.titolo.as_str()).collect();
    assert_eq!(
        titoli,
        [
            "Computo",
            "Riepilogo",
            "Riepilogo super categorie",
            "Riepilogo categorie",
            "Riepilogo WBS"
        ]
    );

    let t = &doc.sezioni[0].tabella;
    assert_eq!(t.colonne.len(), 10);
    let voci = righe(t, StileRiga::Voce);
    let codici: Vec<_> = voci.iter().map(|r| testo(&r.celle[1])).collect();
    assert_eq!(codici, ["A.01.001", "B.02.010", "C.03.020", "S.01.001"]);
    let sommano = righe(t, StileRiga::Sommano);
    let quantita: Vec<_> = sommano
        .iter()
        .map(|r| numero(&r.celle[7]).unwrap())
        .collect();
    let importi: Vec<_> = sommano
        .iter()
        .map(|r| numero(&r.celle[9]).unwrap())
        .collect();
    assert_eq!(
        quantita,
        [dec!(126.00), dec!(51.74), dec!(109.20), dec!(100.00)]
    );
    assert_eq!(
        importi,
        [dec!(1071.00), dec!(7517.82), dec!(4668.30), dec!(1200.00)]
    );
    assert_eq!(testo(&sommano[0].celle[2]), "SOMMANO m³");
    assert_eq!(totale(t, "Totale lavori"), dec!(13257.12));
    assert_eq!(totale(t, "Totale oneri della sicurezza"), dec!(1200.00));
    assert_eq!(totale(t, "TOTALE"), dec!(14457.12));

    // Righi: cordoli 4 × 7,80 × 0,30 × 0,40 = 3,744 → 3,74; deduzione negativa; commento.
    let cordoli = t
        .righe
        .iter()
        .find(|r| testo(&r.celle[2]) == "Cordoli")
        .unwrap();
    assert_eq!(numero(&cordoli.celle[4]), Some(dec!(7.80)));
    assert_eq!(numero(&cordoli.celle[7]), Some(dec!(3.74)));
    let finestre = t
        .righe
        .iter()
        .find(|r| testo(&r.celle[2]) == "A dedurre finestre")
        .unwrap();
    assert_eq!(numero(&finestre.celle[7]), Some(dec!(-10.80)));
    assert_eq!(righe(t, StileRiga::Commento).len(), 1);
    let rampa = t
        .righe
        .iter()
        .find(|r| testo(&r.celle[2]).starts_with("Rampa"))
        .unwrap();
    assert!(testo(&rampa.celle[2]).contains("[formula: 0,5*lun*lar*alt]"));
    assert_eq!(numero(&rampa.celle[7]), Some(dec!(10.80)));

    // Riepiloghi: incidenze, WBS cumulata.
    let cat = &doc.sezioni[3].tabella;
    let inc: Decimal = cat
        .righe
        .iter()
        .filter(|r| r.stile == StileRiga::Normale)
        .filter_map(|r| numero(&r.celle[3]))
        .sum();
    assert_eq!(inc, dec!(100.00));
    let wbs = &doc.sezioni[4].tabella;
    let edificio = wbs
        .righe
        .iter()
        .find(|r| testo(&r.celle[0]) == "01")
        .unwrap();
    assert_eq!(numero(&edificio.celle[2]), Some(dec!(13257.12)));
    let elevazioni = wbs
        .righe
        .iter()
        .find(|r| testo(&r.celle[0]) == "01.02")
        .unwrap();
    assert_eq!(numero(&elevazioni.celle[2]), Some(dec!(4668.30)));
    assert_eq!(totale(wbs, "TOTALE"), dec!(14457.12));
}

#[test]
fn libretto_senza_prezzi_e_registro_per_sal() {
    let p = progetto();
    let lib = report::libretto(&p, &opzioni()).unwrap();
    let t = &lib.sezioni[0].tabella;
    assert_eq!(
        t.colonne.len(),
        8,
        "il libretto non riporta prezzi e importi"
    );
    assert!(t.righe.iter().all(|r| r.celle.len() == 8));
    let voci = righe(t, StileRiga::Voce);
    assert_eq!(voci.len(), 4);
    assert!(testo(&voci[0].celle[2]).ends_with("10/03/2026, SAL n. 1"));
    assert!(testo(&voci[3].celle[2]).ends_with("SAL n. 2"));
    let q: Vec<_> = righe(t, StileRiga::Sommano)
        .iter()
        .map(|r| numero(&r.celle[7]).unwrap())
        .collect();
    assert_eq!(q, [dec!(115.20), dec!(48.00), dec!(100.00), dec!(72.00)]);

    let reg = report::registro(&p, &opzioni()).unwrap();
    let t = &reg.sezioni[0].tabella;
    let gruppi: Vec<_> = righe(t, StileRiga::Gruppo)
        .iter()
        .map(|r| testo(&r.celle[4]))
        .collect();
    assert_eq!(
        gruppi,
        [
            "SAL n. 1 — lavori a tutto il 31/03/2026",
            "SAL n. 2 — lavori a tutto il 30/04/2026"
        ]
    );
    let periodi: Vec<_> = t
        .righe
        .iter()
        .filter(|r| testo(&r.celle[4]) == "Importo del periodo")
        .map(|r| numero(&r.celle[8]).unwrap())
        .collect();
    let progressivi: Vec<_> = t
        .righe
        .iter()
        .filter(|r| testo(&r.celle[4]) == "Importo progressivo")
        .map(|r| numero(&r.celle[8]).unwrap())
        .collect();
    // SAL 1: 979,20 + 6.974,40 + 1.200,00; SAL 2: 3.078,00.
    assert_eq!(periodi, [dec!(9153.60), dec!(3078.00)]);
    assert_eq!(progressivi, [dec!(9153.60), dec!(12231.60)]);
    assert!(testo(&t.righe[3].celle[4]).ends_with("(sicurezza)"));
}

#[test]
fn sal_emessi_e_in_bozza() {
    let p = progetto();
    let sal: Vec<SalId> = p.contabilita.sal().iter().map(|s| s.id).collect();
    for (id, atteso) in sal.iter().zip([dec!(8118.60), dec!(2679.79)]) {
        let doc = report::stato_avanzamento(&p, *id, &opzioni()).unwrap();
        assert!(!doc.titolo.contains("BOZZA"));
        let prospetto = &doc.sezioni[1].tabella;
        assert_eq!(totale(prospetto, "IMPORTO DEL PRESENTE SAL"), atteso);
        assert!(
            doc.intestazione
                .iter()
                .any(|(k, v)| k == "Ribasso d'asta" && v == "12,5 %")
        );
    }
    let doc = report::stato_avanzamento(&p, sal[1], &opzioni()).unwrap();
    let voci: Vec<_> = doc.sezioni[1]
        .tabella
        .righe
        .iter()
        .map(|r| (testo(&r.celle[0]), numero(&r.celle[1]).unwrap()))
        .collect();
    assert_eq!(
        voci[0],
        ("Lavori a tutto il presente SAL".into(), dec!(11031.60))
    );
    assert!(voci.contains(&("a dedurre: ribasso d'asta".into(), dec!(1378.95))));
    assert!(voci.contains(&("a dedurre: certificati precedenti".into(), dec!(8118.60))));
    assert_eq!(righe(&doc.sezioni[0].tabella, StileRiga::Normale).len(), 1);

    // SAL in bozza: ricalcolato e marcato.
    let mut p = progetto();
    let mut g = GeneratoreSequenziale::new(70_000);
    let voce = p.elenco_prezzi.voci[0].clone();
    let prezzo = opencomputo_core::partita::PrezzoApplicato::da_voce(&p.elenco_prezzi, &voce);
    let mut reg = opencomputo_core::contabilita::RegistrazioneLibretto::new(
        g.nuovo(),
        5,
        data_amg(2026, 5, 20).unwrap(),
        prezzo,
    );
    reg.righi.push(
        opencomputo_core::misura::RigoMisura::misura(g.nuovo(), "Integrazione")
            .con_lunghezza(dec!(10)),
    );
    p.contabilita.registra(reg).unwrap();
    let nuovo: SalId = g.nuovo();
    p.contabilita
        .nuovo_sal(nuovo, data_amg(2026, 5, 31).unwrap())
        .unwrap_or_else(|e| panic!("{e}"));
    let doc = report::stato_avanzamento(&p, nuovo, &opzioni()).unwrap();
    assert!(doc.titolo.ends_with("— BOZZA"));
    assert!(doc.sezioni[1].note.iter().any(|n| n.contains("non emesso")));
}

#[test]
fn errori_espliciti() {
    let mut p = progetto();
    assert!(matches!(
        report::computo(&p, ComputoId::da_u128(424_242), &opzioni()),
        Err(ErroreReport::ComputoInesistente(_))
    ));
    assert!(matches!(
        report::stato_avanzamento(&p, SalId::da_u128(424_242), &opzioni()),
        Err(ErroreReport::SalInesistente(_))
    ));
    Arc::make_mut(&mut p.computi[0].voci[1]).righi[0].lunghezza =
        Some(Fattore::Espressione("2*(".into()));
    let id = computo_id(&p);
    assert!(matches!(
        report::computo(&p, id, &opzioni()),
        Err(ErroreReport::Calcolo(_))
    ));
}

#[test]
fn elenco_prezzi() {
    let p = progetto();
    let doc = report::elenco_prezzi(&p, &opzioni());
    assert!(
        doc.intestazione
            .iter()
            .any(|(k, v)| k == "Prezzario" && v == "Elenco prezzi di progetto (2026)")
    );
    let t = &doc.sezioni[0].tabella;
    assert_eq!(t.righe.len(), 4);
    assert_eq!(testo(&t.righe[1].celle[3]), "145,30");
    assert_eq!(testo(&t.righe[1].celle[4]), "18,50");
    assert_eq!(testo(&t.righe[3].celle[5]), "sicurezza");
}

#[test]
fn csv_coerente_con_il_report() {
    let p = progetto();
    let doc = report::computo(&p, computo_id(&p), &opzioni()).unwrap();
    let file = csv::documento(&doc, &csv::OpzioniCsv::default());
    assert_eq!(file.len(), doc.sezioni.len());
    let computo = &file[0].1;
    assert!(computo.starts_with("\u{FEFF}Tipo;N.;Codice;Designazione dei lavori;"));
    assert!(
        computo.contains("\r\nsommano;;;SOMMANO m³;;;;;126,00;8,50;1071,00\r\n"),
        "{computo}"
    );
    assert!(computo.contains("\r\ntotale;;;TOTALE;;;;;;;14457,12\r\n"));
    assert!(computo.contains("\r\nrigo;;;Cordoli;4,00;7,80;0,30;0,40;3,74;;\r\n"));
    // Righe CSV = righe della tabella + intestazione.
    assert_eq!(
        computo.matches("\r\n").count(),
        doc.sezioni[0].tabella.righe.len() + 1
    );
}

/// Controllo strutturale del PDF: tabella xref coerente con le posizioni degli oggetti.
fn verifica_pdf(byte: &[u8]) -> usize {
    let cerca = |ago: &[u8]| byte.windows(ago.len()).rposition(|w| w == ago);
    assert!(byte.starts_with(b"%PDF-1.4\n"));
    assert!(byte.ends_with(b"%%EOF\n"));
    let start = cerca(b"startxref\n").unwrap();
    let coda = std::str::from_utf8(&byte[start + 10..]).unwrap();
    let xref: usize = coda.lines().next().unwrap().parse().unwrap();
    assert_eq!(&byte[xref..xref + 4], b"xref");
    let tabella = std::str::from_utf8(&byte[xref..]).unwrap();
    let righe: Vec<&str> = tabella.lines().collect();
    let n: usize = righe[1].split(' ').nth(1).unwrap().parse().unwrap();
    for (i, r) in righe[3..3 + n - 1].iter().enumerate() {
        let pos: usize = r[..10].parse().unwrap();
        let atteso = format!("{} 0 obj", i + 1);
        assert_eq!(
            &byte[pos..pos + atteso.len()],
            atteso.as_bytes(),
            "oggetto {}",
            i + 1
        );
    }
    let pagine = b"/Type /Page ";
    byte.windows(pagine.len()).filter(|w| *w == pagine).count()
}

#[test]
fn pdf_valido_deterministico_e_con_i_valori() {
    let p = progetto();
    let doc = report::computo(&p, computo_id(&p), &opzioni()).unwrap();
    let a = pdf::documento(&doc, &pdf::OpzioniPdf::default());
    let b = pdf::documento(&doc, &pdf::OpzioniPdf::default());
    assert_eq!(a, b, "uscita deterministica");
    assert_eq!(verifica_pdf(&a.byte), a.pagine);
    assert_eq!(a.caratteri_sostituiti, 0);

    let chiaro = pdf::documento(&doc, &pdf::OpzioniPdf { comprimi: false });
    assert_eq!(verifica_pdf(&chiaro.byte), chiaro.pagine);
    let s = String::from_utf8_lossy(&chiaro.byte);
    for atteso in [
        "(14.457,12) Tj",
        "(SOMMANO m\\263) Tj",
        "(7.517,82) Tj",
        "(Pagina 1 di 2) Tj",
        "(OpenComputo \\227 Stampato il 20/10/2026) Tj",
    ] {
        assert!(s.contains(atteso), "manca {atteso}");
    }
}

#[test]
fn pdf_righe_lunghe_spezzate_tra_le_pagine() {
    let p = progetto();
    let mut doc = report::elenco_prezzi(&p, &opzioni());
    let parole: Vec<String> = (0..3000).map(|i| format!("parola{i}")).collect();
    doc.sezioni[0].tabella.righe[0].celle[1] = Valore::testo(parole.join(" "));
    let r = pdf::documento(&doc, &pdf::OpzioniPdf { comprimi: false });
    assert!(r.pagine >= 3, "{} pagine", r.pagine);
    assert_eq!(verifica_pdf(&r.byte), r.pagine);
    let s = String::from_utf8_lossy(&r.byte);
    for i in [0, 1500, 2999] {
        assert!(s.contains(&format!("parola{i}")), "parola{i} persa");
    }
    assert!(s.contains("(Elenco prezzi \\(segue\\)) Tj"));
}

#[test]
fn xlsx_un_foglio_per_sezione_e_riproducibile() {
    let p = progetto();
    let doc = report::computo(&p, computo_id(&p), &opzioni()).unwrap();
    let o = xlsx::OpzioniXlsx {
        data: data_amg(2026, 10, 20),
    };
    let a = xlsx::documento(&doc, &o).unwrap();
    assert_eq!(
        a,
        xlsx::documento(&doc, &o).unwrap(),
        "uscita riproducibile"
    );
    assert!(a.starts_with(b"PK"));
    // I nomi dei file nell'archivio ZIP sono in chiaro.
    for n in 1..=doc.sezioni.len() {
        let nome = format!("xl/worksheets/sheet{n}.xml");
        assert!(
            a.windows(nome.len()).any(|w| w == nome.as_bytes()),
            "manca {nome}"
        );
    }
    let oltre = format!("xl/worksheets/sheet{}.xml", doc.sezioni.len() + 1);
    assert!(!a.windows(oltre.len()).any(|w| w == oltre.as_bytes()));
}

#[test]
fn documento_vuoto_e_caratteri_non_rappresentabili() {
    let doc = Documento {
        titolo: "Prova Ж".into(),
        intestazione: Vec::new(),
        sezioni: Vec::new(),
        orizzontale: false,
        piede: None,
    };
    let r = pdf::documento(&doc, &pdf::OpzioniPdf::default());
    assert_eq!(r.pagine, 1);
    assert!(r.caratteri_sostituiti >= 1);
    assert_eq!(verifica_pdf(&r.byte), 1);
    assert!(xlsx::documento(&doc, &xlsx::OpzioniXlsx::default()).is_ok());
    assert!(csv::documento(&doc, &csv::OpzioniCsv::default()).is_empty());
}

#[test]
fn report_di_un_computo_importato_da_xpwe() {
    let xpwe =
        include_bytes!("../../opencomputo-format-xpwe/tests/fixtures/computo_sintetico.xpwe");
    let r = opencomputo_format_xpwe::importa(
        xpwe,
        &mut GeneratoreSequenziale::default(),
        &opencomputo_format_xpwe::OpzioniImport::default(),
    )
    .unwrap();
    let doc = report::computo(&r.progetto, r.computo, &opzioni()).unwrap();
    // Totali della fixture XPWE calcolati a mano.
    assert_eq!(
        totale(&doc.sezioni[0].tabella, "Totale lavori"),
        dec!(8566.84)
    );
    assert_eq!(totale(&doc.sezioni[0].tabella, "TOTALE"), dec!(9291.84));
    let pdf = pdf::documento(&doc, &pdf::OpzioniPdf::default());
    assert_eq!(pdf.caratteri_sostituiti, 0);
    assert_eq!(verifica_pdf(&pdf.byte), pdf.pagine);
}

#[test]
fn righi_vedi_voce_nel_computo() {
    let p = importa_json(include_str!(
        "../../opencomputo-core/tests/fixtures/progetto_esempio_v6.json"
    ))
    .unwrap();
    let d = report::computo(&p, computo_id(&p), &opzioni()).unwrap();
    let t = &d.sezioni[0].tabella;
    let testi: Vec<String> = righe(t, StileRiga::Rigo)
        .iter()
        .map(|r| testo(&r.celle[2]))
        .collect();
    assert!(
        testi.contains(&"Vedi voce n. 1 [m³ 126,00] — Metà dello scavo".to_string()),
        "{testi:?}"
    );
    assert!(testi.contains(&"Vedi voce n. 2 [m³ 51,74] — A dedurre il calcestruzzo".to_string()));
    assert!(
        testi.contains(
            &"Vedi voce n. 1 «E.01» del computo «Variante» [10,00] — Dal computo della variante"
                .to_string()
        )
    );
    let quantita: Vec<Decimal> = righe(t, StileRiga::Rigo)
        .iter()
        .rev()
        .take(3)
        .filter_map(|r| numero(&r.celle[7]))
        .collect();
    assert_eq!(quantita, [dec!(10.00), dec!(-51.74), dec!(63.00)]);
}

/// Fixture con l'incidenza della sicurezza su calcestruzzo (3 %) e murature (2,5 %).
fn progetto_con_sicurezza() -> Progetto {
    use opencomputo_core::numeri::Percentuale;
    let mut p = progetto();
    Arc::make_mut(&mut p.computi[0].voci[1])
        .prezzo
        .incidenza_sicurezza = Some(Percentuale::new(dec!(3)));
    Arc::make_mut(&mut p.computi[0].voci[2])
        .prezzo
        .incidenza_sicurezza = Some(Percentuale::new(dec!(2.5)));
    p
}

fn colonna(t: &Tabella, titolo: &str) -> usize {
    t.colonne
        .iter()
        .position(|c| c.titolo == titolo)
        .unwrap_or_else(|| panic!("colonna «{titolo}» assente"))
}

#[test]
fn computo_con_incidenze_e_subtotali() {
    let p = progetto_con_sicurezza();
    let tutte = report::Incidenze {
        manodopera: true,
        sicurezza: true,
        riepilogo: true,
    };
    let doc = report::computo_con(&p, computo_id(&p), tutte, &opzioni()).unwrap();
    let t = &doc.sezioni[0].tabella;
    let (mdo, mdo_pc) = (colonna(t, "Manodopera €"), colonna(t, "Mdo %"));
    let (sic, sic_pc) = (colonna(t, "Sicurezza €"), colonna(t, "Sic. %"));
    assert_eq!(colonna(t, "Importo €"), 9);
    let sommano = righe(t, StileRiga::Sommano);
    let celle = |i: usize, c: usize| numero(&sommano[i].celle[c]);
    // Quote e incidenze dal core e dal prezzo di ogni voce.
    assert_eq!(celle(0, mdo), Some(dec!(267.75)));
    assert_eq!(celle(0, mdo_pc), Some(dec!(25)));
    assert_eq!(testo(&sommano[0].celle[sic]), "—");
    assert_eq!(celle(1, mdo), Some(dec!(1390.80)));
    assert_eq!(celle(1, sic), Some(dec!(225.53)));
    assert_eq!(celle(1, sic_pc), Some(dec!(3)));
    assert_eq!(celle(2, sic), Some(dec!(116.71)));
    assert_eq!(testo(&sommano[3].celle[mdo]), "—");
    // Il totale resta nella colonna dell'importo, le quote nelle loro.
    let tot = t
        .righe
        .iter()
        .find(|r| r.stile == StileRiga::Totale && testo(&r.celle[2]) == "TOTALE")
        .unwrap();
    assert_eq!(numero(&tot.celle[9]), Some(dec!(14457.12)));
    assert_eq!(numero(&tot.celle[mdo]), Some(dec!(3759.29)));
    // 3 759,29 / 14 457,12 = 26,0029 %
    assert_eq!(numero(&tot.celle[mdo_pc]), Some(dec!(26.00)));
    assert_eq!(numero(&tot.celle[sic]), Some(dec!(342.24)));

    // Riepilogo generale con la sicurezza compresa nei prezzi e le incidenze.
    let rg = &doc.sezioni[1];
    let riga = rg
        .tabella
        .righe
        .iter()
        .find(|r| testo(&r.celle[0]).starts_with("di cui sicurezza"))
        .unwrap();
    assert_eq!(numero(&riga.celle[1]), Some(dec!(342.24)));
    assert_eq!(numero(&riga.celle[2]), Some(dec!(2.37)));
    assert!(
        rg.note
            .iter()
            .any(|n| n.starts_with("2 voci senza incidenza della sicurezza"))
    );

    // Subtotali per categoria: la quota e l'incidenza sull'importo del gruppo.
    let cat = &doc.sezioni[3];
    assert_eq!(cat.titolo, "Riepilogo categorie");
    let strutture = cat
        .tabella
        .righe
        .iter()
        .find(|r| numero(&r.celle[2]) == Some(dec!(7517.82)))
        .unwrap();
    let c = &cat.tabella;
    assert_eq!(
        numero(&strutture.celle[colonna(c, "Manodopera €")]),
        Some(dec!(1390.80))
    );
    assert_eq!(
        numero(&strutture.celle[colonna(c, "Mdo %")]),
        Some(dec!(18.50))
    );
    assert_eq!(
        numero(&strutture.celle[colonna(c, "Sicurezza €")]),
        Some(dec!(225.53))
    );
    let somma: Decimal = c
        .righe
        .iter()
        .filter(|r| r.stile == StileRiga::Normale)
        .filter_map(|r| numero(&r.celle[colonna(c, "Manodopera €")]))
        .sum();
    assert_eq!(somma, dec!(3759.29));
    // WBS cumulata: il nodo radice somma le quote dei figli.
    let wbs = &doc.sezioni[4].tabella;
    let edificio = wbs
        .righe
        .iter()
        .find(|r| testo(&r.celle[0]) == "01")
        .unwrap();
    assert_eq!(
        numero(&edificio.celle[colonna(wbs, "Sicurezza €")]),
        Some(dec!(342.24))
    );
    assert_eq!(
        numero(&edificio.celle[colonna(wbs, "Manodopera €")]),
        Some(dec!(3759.29))
    );

    // Coerenza tra i formati.
    assert!(
        pdf::documento(&doc, &pdf::OpzioniPdf::default())
            .byte
            .starts_with(b"%PDF")
    );
    assert!(xlsx::documento(&doc, &xlsx::OpzioniXlsx::default()).is_ok());
    assert!(
        csv::documento(&doc, &csv::OpzioniCsv::default())[0]
            .1
            .contains("3759,29")
    );
}

#[test]
fn incidenze_senza_subtotali_e_report_invariato_senza_incidenze() {
    let p = progetto_con_sicurezza();
    let base = report::computo(&p, computo_id(&p), &opzioni()).unwrap();
    let nessuna =
        report::computo_con(&p, computo_id(&p), report::Incidenze::default(), &opzioni()).unwrap();
    assert_eq!(base, nessuna);
    assert_eq!(base.sezioni[0].tabella.colonne.len(), 10);
    assert_eq!(base.sezioni[3].tabella.colonne.len(), 4);

    let solo_mdo = report::Incidenze {
        manodopera: true,
        ..report::Incidenze::default()
    };
    let doc = report::computo_con(&p, computo_id(&p), solo_mdo, &opzioni()).unwrap();
    assert_eq!(doc.sezioni[0].tabella.colonne.len(), 12);
    // Senza subtotali i riepiloghi restano come prima.
    assert_eq!(doc.sezioni[3].tabella, base.sezioni[3].tabella);
    assert_eq!(doc.sezioni[1].tabella.colonne.len(), 2);
}
