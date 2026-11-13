//! Stampa del computo e dell'elenco prezzi nel modello tradizionale (ADR 0015): testi,
//! riporti di pagina e totali letti dal PDF non compresso.
//!
//! Con la variabile `OPENCOMPUTO_STAMPE` impostata a una cartella, i PDF prodotti sono
//! scritti lì per il confronto visivo.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use chrono::NaiveDate;
use opencomputo_core::classificazione::AssegnazioneCategorie;
use opencomputo_core::id::{GeneratoreId, GeneratoreSequenziale};
use opencomputo_core::progetto::Progetto;
use opencomputo_core::schema::importa_json;
use opencomputo_io::formato;
use opencomputo_io::pdf::RisultatoPdf;
use opencomputo_io::tradizionale::{self, OpzioniTradizionale};
use rust_decimal::Decimal;

const FIXTURE: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v1.json");

fn progetto() -> Progetto {
    let mut p = importa_json(FIXTURE).unwrap();
    p.metadati.oggetto = Some("Costruzione della nuova scuola primaria".into());
    p.metadati.progettista = Some("Ing. Maria Bianchi".into());
    p.metadati.luogo = Some("Esempio".into());
    p.metadati.data = NaiveDate::from_ymd_opt(2026, 10, 21);
    p.elenco_prezzi.voci[0].descrizione_estesa = DESCRIZIONE_LUNGA.into();
    p.computi[0].voci[0].prezzo.descrizione_estesa = DESCRIZIONE_LUNGA.into();
    p
}

const DESCRIZIONE_LUNGA: &str = "Scavo di sbancamento a sezione aperta eseguito con mezzi \
    meccanici, anche in presenza di acqua, in terreni di qualsiasi natura e consistenza, \
    compresi i trovanti rocciosi fino a 0,50 m³, il carico sui mezzi di trasporto, la \
    regolarizzazione delle pareti e del fondo, il deflusso delle acque e ogni altro onere \
    per dare il lavoro finito a regola d'arte; escluso il trasporto a discarica e gli \
    oneri di conferimento, compensati a parte.";

/// Progetto con molte voci, per avere più pagine e voci spezzate tra le pagine.
fn progetto_lungo() -> Progetto {
    let mut p = progetto();
    let mut g = GeneratoreSequenziale::new(10_000);
    let base = p.computi[0].voci.clone();
    for i in 0..12 {
        for v in &base {
            let mut v = v.clone();
            v.id = g.nuovo();
            for r in &mut v.righi {
                r.id = g.nuovo();
            }
            if i % 2 == 1 {
                v.categorie = AssegnazioneCategorie::default();
            }
            p.computi[0].voci.push(v);
        }
    }
    p
}

fn opzioni() -> OpzioniTradizionale {
    OpzioniTradizionale {
        ente: vec!["Comune di Esempio".into(), "Provincia di Esempio".into()],
        studio: vec![
            "Studio tecnico di progettazione".into(),
            "Via Roma 1 - Esempio".into(),
        ],
        comprimi: false,
        ..OpzioniTradizionale::default()
    }
}

fn salva(nome: &str, r: &RisultatoPdf) {
    if let Ok(cartella) = std::env::var("OPENCOMPUTO_STAMPE") {
        std::fs::write(std::path::Path::new(&cartella).join(nome), &r.byte).unwrap();
    }
}

/// Testi di ogni pagina, nell'ordine in cui sono scritti (PDF non compresso).
fn testi(r: &RisultatoPdf) -> Vec<Vec<String>> {
    let dati = &r.byte;
    let mut pagine = Vec::new();
    let mut i = 0;
    while let Some(p) = trova(dati, b"stream\n", i) {
        let inizio = p + 7;
        let fine = trova(dati, b"\nendstream", inizio).unwrap();
        pagine.push(stringhe(&dati[inizio..fine]));
        i = fine + b"\nendstream".len();
    }
    pagine
}

fn trova(dati: &[u8], cosa: &[u8], da: usize) -> Option<usize> {
    dati.get(da..)?
        .windows(cosa.len())
        .position(|w| w == cosa)
        .map(|p| p + da)
}

/// Stringhe `(...) Tj` decodificate da WinAnsi.
fn stringhe(contenuto: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < contenuto.len() {
        if contenuto[k] != b'(' {
            k += 1;
            continue;
        }
        let mut byte = Vec::new();
        k += 1;
        while contenuto[k] != b')' {
            if contenuto[k] == b'\\' {
                let ottale = &contenuto[k + 1..k + 4];
                if ottale.iter().all(u8::is_ascii_digit) {
                    let s = std::str::from_utf8(ottale).unwrap();
                    byte.push(u8::from_str_radix(s, 8).unwrap());
                    k += 4;
                } else {
                    byte.push(contenuto[k + 1]);
                    k += 2;
                }
            } else {
                byte.push(contenuto[k]);
                k += 1;
            }
        }
        k += 1;
        out.push(
            byte.iter()
                .map(|b| match b {
                    0x80 => '€',
                    0x96 => '–',
                    0x97 => '—',
                    0xB4 => '´',
                    0xB2 => '²',
                    0xB3 => '³',
                    0xE0 => 'à',
                    0xE8 => 'è',
                    0xE9 => 'é',
                    0xEC => 'ì',
                    0xF2 => 'ò',
                    0xF9 => 'ù',
                    b => char::from(*b),
                })
                .collect(),
        );
    }
    out
}

fn contiene(pagina: &[String], testo: &str) -> bool {
    pagina.iter().any(|t| t == testo)
}

fn dopo<'a>(pagina: &'a [String], etichetta: &str, quanti: usize) -> Vec<&'a str> {
    let i = pagina.iter().position(|t| t == etichetta).unwrap();
    pagina[i + 1..=i + quanti]
        .iter()
        .map(String::as_str)
        .collect()
}

#[test]
fn computo_estimativo_con_frontespizio_e_totali_del_core() {
    let p = progetto();
    let id = p.computi[0].id;
    let r = tradizionale::computo(&p, id, &opzioni()).unwrap();
    salva("computo_tradizionale.pdf", &r);
    assert!(r.byte.starts_with(b"%PDF-1.4"));
    assert_eq!(r.caratteri_sostituiti, 0);
    let pagine = testi(&r);
    assert_eq!(pagine.len(), r.pagine);
    // Frontespizio e una pagina per ciascuna delle due super categorie.
    assert_eq!(r.pagine, 3);

    // Frontespizio.
    let f = &pagine[0];
    for t in [
        "Comune di Esempio",
        "Provincia di Esempio",
        "pag. 1",
        "COMPUTO METRICO ESTIMATIVO",
        "OGGETTO:",
        "Costruzione della nuova scuola primaria",
        "COMMITTENTE:",
        "Comune di Esempio",
        "Esempio, 21/10/2026",
        "IL TECNICO",
        "Ing. Maria Bianchi",
        "Studio tecnico di progettazione",
    ] {
        assert!(contiene(f, t), "frontespizio senza «{t}»: {f:?}");
    }

    // Tabella.
    let t = &pagine[1..].concat();
    for s in [
        "Studio tecnico di progettazione",
        "Via Roma 1 - Esempio",
        "pag. 2",
        "D I M E N S I O N I",
        "I M P O R T I",
        "Num.Ord.",
        "TARIFFA",
        "DESIGNAZIONE DEI LAVORI",
        "Quantità",
        "par.ug.",
        "H/peso",
        "unitario",
        "TOTALE",
        "R I P O R T O ",
        "A   R I P O R T A R E ",
        "Opere edili",
        "SENZA SUPER CATEGORIA",
        "1",
        "A.01.001",
        "SOMMANO m³",
        "Sommano positivi m²",
        "Sommano negativi m²",
        "Parziale Opere edili euro",
        "Parziale SENZA SUPER CATEGORIA euro",
        "T O T A L E   euro",
        "Esempio, 21/10/2026",
        "Il Tecnico",
        "Ing. Maria Bianchi",
        "COMMITTENTE: Comune di Esempio",
    ] {
        assert!(contiene(t, s), "tabella senza «{s}»: {t:?}");
    }
    // Descrizione lunga ridotta nel computo, completa nell'elenco.
    assert!(t.iter().any(|s| s.contains(" ... ")));
    let elenco = testi(&tradizionale::elenco_prezzi(&p, &opzioni())).concat();
    assert!(!elenco.iter().any(|s| s.contains(" ... ")));
    assert!(elenco.iter().any(|s| s.ends_with("compensati a parte.")));
    // Totale del computo dal core, con l'apice per le migliaia.
    let c = p.calcola_computo(id).unwrap();
    let totale = formato::numero_apice(c.totale.valore(), 2);
    assert_eq!(totale, "14´457,12");
    assert_eq!(dopo(t, "T O T A L E   euro", 1), vec![totale.as_str()]);
    // Ogni «SOMMANO» riporta quantità, prezzo e importo della voce.
    let v = &c.voci[0];
    assert_eq!(
        dopo(t, "SOMMANO m³", 3),
        vec![
            formato::numero_apice(v.calcolo.quantita.valore(), 2).as_str(),
            formato::numero_apice(v.prezzo.valore(), 2).as_str(),
            formato::numero_apice(v.calcolo.importo.valore(), 2).as_str(),
        ]
    );
    // Pagina finale: completata con il tratteggio, piè senza valori.
    let ultima = pagine.last().unwrap();
    assert!(contiene(ultima, &"-".repeat(40)));
    let i = ultima
        .iter()
        .position(|s| s == "A   R I P O R T A R E ")
        .unwrap();
    assert!(!ultima[i + 1].contains(','));
    // La prima pagina della tabella passa il progressivo alla seconda.
    let parziale = formato::numero_apice(
        c.per_super_categoria
            .per_chiave
            .values()
            .next()
            .unwrap()
            .valore(),
        2,
    );
    assert_eq!(
        dopo(&pagine[1], "A   R I P O R T A R E ", 1),
        vec![parziale.as_str()]
    );
    assert_eq!(
        dopo(&pagine[2], "R I P O R T O ", 1),
        vec![parziale.as_str()]
    );
}

#[test]
fn riporti_di_pagina_coerenti() {
    let p = progetto_lungo();
    let id = p.computi[0].id;
    let r = tradizionale::computo(&p, id, &opzioni()).unwrap();
    salva("computo_tradizionale_lungo.pdf", &r);
    let pagine = testi(&r);
    assert!(pagine.len() >= 5, "pagine: {}", pagine.len());
    // Il valore «A RIPORTARE» di una pagina è il «RIPORTO» della successiva.
    let valori = |pagina: &[String], etichetta: &str| -> Vec<String> {
        let i = pagina.iter().position(|t| t == etichetta).unwrap();
        pagina[i + 1..]
            .iter()
            .take_while(|t| {
                t.contains(',') && t.chars().all(|c| c.is_ascii_digit() || ",´-".contains(c))
            })
            .cloned()
            .collect()
    };
    let mut riporti = 0;
    for coppia in pagine[1..].windows(2) {
        let fine = valori(&coppia[0], "A   R I P O R T A R E ");
        let inizio = valori(&coppia[1], "R I P O R T O ");
        assert_eq!(fine, inizio);
        assert!(!fine.is_empty());
        riporti += 1;
    }
    assert!(riporti >= 4);
    // La prima pagina della tabella non ha riporto; l'ultima chiude con il totale del core.
    assert!(valori(&pagine[1], "R I P O R T O ").is_empty());
    let totale = formato::numero_apice(p.calcola_computo(id).unwrap().totale.valore(), 2);
    assert_eq!(
        dopo(&pagine.concat(), "T O T A L E   euro", 1),
        vec![totale.as_str()]
    );
    // Ogni gruppo di super categoria comincia su una pagina nuova.
    let pagina_gruppo = pagine
        .iter()
        .position(|p| contiene(p, "SENZA SUPER CATEGORIA"))
        .unwrap();
    assert!(pagina_gruppo > 1);
    assert!(!contiene(&pagine[pagina_gruppo], "Opere edili"));
    // Uscita deterministica.
    assert_eq!(
        tradizionale::computo(&p, id, &opzioni()).unwrap().byte,
        r.byte
    );
}

#[test]
fn computo_metrico_senza_prezzi_e_senza_frontespizio() {
    let p = progetto();
    let id = p.computi[0].id;
    let o = OpzioniTradizionale {
        senza_prezzi: true,
        frontespizio: false,
        studio: Vec::new(),
        ..opzioni()
    };
    let r = tradizionale::computo(&p, id, &o).unwrap();
    salva("computo_metrico_tradizionale.pdf", &r);
    let pagine = testi(&r);
    assert_eq!(pagine.len(), 2);
    assert!(contiene(&pagine[0], "pag. 1"));
    let t = &pagine.concat();
    assert!(!contiene(t, "COMPUTO METRICO"));
    assert!(!contiene(t, "T O T A L E   euro"));
    assert!(!t.iter().any(|s| s.starts_with("Parziale")));
    let c = p.calcola_computo(id).unwrap();
    // Dopo «SOMMANO» solo la quantità.
    let i = t.iter().position(|s| s == "SOMMANO m³").unwrap();
    assert_eq!(
        t[i + 1],
        formato::numero_apice(c.voci[0].calcolo.quantita.valore(), 2)
    );
    assert!(
        !t.iter()
            .any(|s| s == &formato::numero_apice(c.voci[0].calcolo.importo.valore(), 2))
    );
}

#[test]
fn elenco_prezzi_con_prezzo_in_lettere() {
    let p = progetto();
    let r = tradizionale::elenco_prezzi(&p, &opzioni());
    salva("elenco_tradizionale.pdf", &r);
    let pagine = testi(&r);
    assert_eq!(pagine.len(), 2);
    assert!(contiene(&pagine[0], "ELENCO PREZZI"));
    let t = &pagine[1];
    for s in [
        "pag. 2",
        "D E S C R I Z I O N E   D E L L' A R T I C O L O",
        "unità",
        "misura",
        "P R E Z Z O",
        "UNITARIO",
        "Nr. 1",
        "Nr. 4",
        "A.01.001",
        "Il Tecnico",
    ] {
        assert!(contiene(t, s), "elenco senza «{s}»: {t:?}");
    }
    for (n, v) in p.elenco_prezzi.voci.iter().enumerate() {
        let lettere = format!("euro ({})", formato::in_lettere(v.prezzo.valore(), 2));
        let i = t
            .iter()
            .position(|s| s == &lettere)
            .unwrap_or_else(|| panic!("voce {n} senza «{lettere}»"));
        assert_eq!(t[i + 1], v.unita_misura);
        assert_eq!(t[i + 2], formato::numero_apice(v.prezzo.valore(), 2));
    }
    assert!(contiene(t, &"-".repeat(78)));
    let zero = Decimal::ZERO;
    assert_eq!(formato::in_lettere(zero, 2), "zero/00");
}

#[test]
fn righi_vedi_voce_con_il_rimando() {
    let p = importa_json(include_str!(
        "../../opencomputo-core/tests/fixtures/progetto_esempio_v6.json"
    ))
    .unwrap();
    let r = tradizionale::computo(&p, p.computi[0].id, &opzioni()).unwrap();
    salva("computo_vedi_voce", &r);
    let tutte: Vec<String> = testi(&r).into_iter().flatten().collect();
    // Il trattino lungo è il byte 0x97 della codifica WinAnsi del PDF.
    let unito = tutte.join(" ").replace('\u{97}', "—");
    assert!(
        unito.contains("Vedi voce n. 1 [m³ 126,00] — Metà dello scavo"),
        "{tutte:?}"
    );
    assert!(unito.contains("Vedi voce n. 2 [m³ 51,74] — A dedurre il calcestruzzo"));
    assert!(unito.contains("21,26"));
}

#[test]
fn computo_inesistente() {
    let p = progetto();
    let id = opencomputo_core::id::ComputoId::da_u128(999);
    assert!(tradizionale::computo(&p, id, &opzioni()).is_err());
}

#[test]
fn voce_spezzata_riporta_la_quantita_progressiva() {
    let mut p = progetto();
    let mut g = GeneratoreSequenziale::new(50_000);
    let voce = &mut p.computi[0].voci[0];
    let base = voce.righi.clone();
    for _ in 0..60 {
        for r in &base {
            let mut r = r.clone();
            r.id = g.nuovo();
            voce.righi.push(r);
        }
    }
    let id = p.computi[0].id;
    let r = tradizionale::computo(&p, id, &opzioni()).unwrap();
    salva("computo_tradizionale_voce_lunga.pdf", &r);
    let pagine = testi(&r);
    let dettaglio =
        opencomputo_core::misura::dettaglio_quantita(&p.computi[0].voci[0].righi, &p.regole)
            .unwrap();
    let progressivi: Vec<String> = dettaglio
        .progressivi
        .iter()
        .map(|q| formato::numero_apice(q.valore(), 2))
        .collect();
    // La prima voce occupa più di una pagina: in calce alla prima pagina della tabella
    // c'è la quantità progressiva (uno dei progressivi del core), senza importo.
    let fine = dopo(&pagine[1], "A   R I P O R T A R E ", 1);
    assert!(progressivi.contains(&fine[0].to_owned()), "{fine:?}");
    assert_eq!(dopo(&pagine[2], "R I P O R T O ", 1), fine);
    // La quantità totale della voce è quella del core.
    let c = p.calcola_computo(id).unwrap();
    assert_eq!(c.voci[0].calcolo.quantita, dettaglio.totale);
}

#[test]
fn computo_con_incidenze_e_riepilogo_finale() {
    use opencomputo_core::numeri::Percentuale;
    use rust_decimal_macros::dec;
    let mut p = progetto();
    p.computi[0].voci[1].prezzo.incidenza_sicurezza = Some(Percentuale::new(dec!(3)));
    p.computi[0].voci[2].prezzo.incidenza_sicurezza = Some(Percentuale::new(dec!(2.5)));
    let id = p.computi[0].id;
    let base = tradizionale::computo(&p, id, &opzioni()).unwrap();
    let o = OpzioniTradizionale {
        incidenze: tradizionale::Incidenze {
            manodopera: true,
            sicurezza: true,
            riepilogo: true,
        },
        ..opzioni()
    };
    let r = tradizionale::computo(&p, id, &o).unwrap();
    salva("computo_tradizionale_incidenze.pdf", &r);
    assert_eq!(r.caratteri_sostituiti, 0);
    let pagine = testi(&r);
    // Le pagine del computo (la designazione è più stretta), più una di riepilogo.
    assert!(r.pagine > base.pagine, "{} pagine", r.pagine);
    let corpo = pagine[1..r.pagine - 1].concat();
    for s in ["MANODOPERA", "SICUREZZA", "incid.%", "euro"] {
        assert!(contiene(&corpo, s), "intestazione senza «{s}»");
    }
    // Quote e incidenze nelle colonne aggiuntive della riga «SOMMANO»; «—» se
    // l'incidenza manca.
    assert_eq!(
        dopo(&corpo, "SOMMANO m³", 6),
        vec!["126,00", "8,50", "1´071,00", "267,75", "25,00", "—"]
    );
    let i = corpo.iter().position(|t| t == "7´517,82").unwrap();
    assert_eq!(
        corpo[i + 1..=i + 4],
        ["1´390,80", "18,50", "225,53", "3,00"]
    );
    assert_eq!(
        dopo(&corpo, "SOMMANO m", 5),
        vec!["100,00", "12,00", "1´200,00", "—", "—"]
    );
    // Parziale e totale con le quote del core e le incidenze sul loro importo.
    assert_eq!(
        dopo(&corpo, "Parziale Opere edili euro", 5),
        vec!["13´257,12", "3´759,29", "28,36", "342,24", "2,58"]
    );
    assert_eq!(
        dopo(&corpo, "T O T A L E   euro", 5),
        vec!["14´457,12", "3´759,29", "26,00", "342,24", "2,37"]
    );
    assert!(!corpo.iter().any(|s| s.starts_with("di cui")));
    // La firma passa in fondo al riepilogo.
    assert!(!contiene(&corpo, "Il Tecnico"));
    let riepilogo = pagine.last().unwrap();
    let numero_pagina = format!("pag. {}", r.pagine);
    for s in [
        numero_pagina.as_str(),
        "D E S C R I Z I O N E",
        "I M P O R T O",
        "M A N O D O P E R A",
        "S I C U R E Z Z A",
        "incid. %",
        "R I E P I L O G O",
        "SUPER CATEGORIE",
        "CATEGORIE",
        "WBS",
        "QUADRO GENERALE",
        "Opere edili",
        "Non assegnato",
        "1 voce senza incidenza della manodopera: la quota di manodopera è",
        "Il Tecnico",
        "Ing. Maria Bianchi",
    ] {
        assert!(
            contiene(riepilogo, s),
            "riepilogo senza «{s}»: {riepilogo:?}"
        );
    }
    // Quadro generale: totale, quote e incidenze sul totale, dal core.
    assert_eq!(
        dopo(riepilogo, "T O T A L E   euro", 5),
        vec!["14´457,12", "3´759,29", "26,00", "342,24", "2,37"]
    );
    // Super categoria «Opere edili»: importo, quote e incidenze sul gruppo.
    assert_eq!(
        dopo(riepilogo, "Opere edili", 5),
        vec!["13´257,12", "3´759,29", "28,36", "342,24", "2,58"]
    );

    // Solo la manodopera, senza riepilogo: nessuna pagina in più.
    let o = OpzioniTradizionale {
        incidenze: tradizionale::Incidenze {
            manodopera: true,
            ..tradizionale::Incidenze::default()
        },
        ..opzioni()
    };
    let r = tradizionale::computo(&p, id, &o).unwrap();
    salva("computo_tradizionale_manodopera.pdf", &r);
    let tutto = testi(&r).concat();
    assert_eq!(
        dopo(&tutto, "SOMMANO m²", 5),
        vec!["109,20", "42,75", "4´668,30", "2´100,74", "45,00"]
    );
    assert!(contiene(&tutto, "MANODOPERA") && !contiene(&tutto, "SICUREZZA"));
    assert!(!contiene(&tutto, "R I E P I L O G O"));
    assert!(contiene(&tutto, "Il Tecnico"));

    // Il computo metrico senza prezzi non stampa le incidenze.
    let o = OpzioniTradizionale {
        senza_prezzi: true,
        incidenze: tradizionale::Incidenze {
            manodopera: true,
            sicurezza: true,
            riepilogo: true,
        },
        ..opzioni()
    };
    let senza = testi(&tradizionale::computo(&p, id, &o).unwrap()).concat();
    assert!(!contiene(&senza, "MANODOPERA"));
    assert!(!contiene(&senza, "R I E P I L O G O"));
}

/// Fixture con la contabilità (libretto e due SAL).
const FIXTURE_CONTABILITA: &str =
    include_str!("../../opencomputo-core/tests/fixtures/progetto_esempio_v6.json");

#[test]
fn documenti_di_cantiere_nel_modello_tradizionale() {
    use opencomputo_io::report::{self, OpzioniReport};
    let mut p = importa_json(FIXTURE_CONTABILITA).unwrap();
    p.metadati.direttore_lavori = Some("Arch. Paolo Verdi".into());
    let o = OpzioniReport {
        data_stampa: NaiveDate::from_ymd_opt(2026, 5, 2),
    };
    let doc = report::registro(&p, &o).unwrap();
    let opzioni = OpzioniTradizionale {
        firma: Some(tradizionale::Firma {
            ruolo: "Il Direttore dei Lavori".into(),
            nome: p.metadati.direttore_lavori.clone(),
        }),
        data_stampa: o.data_stampa,
        ..opzioni()
    };
    let r = tradizionale::documento(&p, &doc, &doc.titolo, &opzioni);
    salva("registro_tradizionale.pdf", &r);
    assert_eq!(r.caratteri_sostituiti, 0);
    let pagine = testi(&r);
    assert_eq!(pagine.len(), r.pagine);
    // Frontespizio con il titolo e chi firma.
    for s in [
        "REGISTRO DI CONTABILITÀ",
        "IL DIRETTORE DEI LAVORI",
        "Arch. Paolo Verdi",
    ] {
        assert!(
            contiene(&pagine[0], s),
            "frontespizio senza «{s}»: {:?}",
            pagine[0]
        );
    }
    // Tabella: intestazione delle colonne, valori del documento ordinario, firma.
    let corpo = pagine[1..].concat();
    for s in [
        "Studio tecnico di progettazione",
        "pag. 2",
        "REGISTRO DI CONTABILITÀ",
        "Descrizione",
        "Importo €",
        "Il Direttore dei Lavori",
        "Arch. Paolo Verdi",
        "COMMITTENTE: Comune di Esempio",
    ] {
        assert!(contiene(&corpo, s), "registro senza «{s}»: {corpo:?}");
    }
    // Ogni importo del documento ordinario compare con l'apice per le migliaia.
    for riga in &doc.sezioni[0].tabella.righe {
        if let Some(opencomputo_io::documento::Valore::Numero { valore, decimali }) =
            riga.celle.last()
        {
            let atteso = formato::numero_apice(*valore, *decimali);
            assert!(contiene(&corpo, &atteso), "importo {atteso} assente");
        }
    }
}

#[test]
fn intestazione_ripetuta_quando_la_tabella_continua() {
    use opencomputo_io::documento::{Colonna, Documento, Sezione, StileRiga, Tabella, Valore};
    let p = progetto();
    let mut t = Tabella::new(vec![
        Colonna::centrata("N.", 4),
        Colonna::testo("Descrizione", 40),
        Colonna::numero("Importo €", 10),
    ]);
    for i in 1..=150 {
        t.riga(
            StileRiga::Normale,
            vec![
                Valore::testo(i.to_string()),
                Valore::testo(format!("Riga di prova {i}")),
                Valore::numero(Decimal::new(i * 100_000, 2), 2),
            ],
        );
    }
    t.riga(
        StileRiga::Totale,
        vec![
            Valore::Vuoto,
            Valore::testo("TOTALE"),
            Valore::numero(Decimal::new(113_250_000, 2), 2),
        ],
    );
    let doc = Documento {
        titolo: "Prova".into(),
        intestazione: Vec::new(),
        sezioni: vec![Sezione {
            titolo: "Tabella lunga".into(),
            note: vec!["Nota della sezione.".into()],
            tabella: t,
        }],
        orizzontale: false,
        piede: None,
    };
    let o = OpzioniTradizionale {
        frontespizio: false,
        ..opzioni()
    };
    let r = tradizionale::documento(&p, &doc, &doc.titolo, &o);
    salva("documento_lungo_tradizionale.pdf", &r);
    let pagine = testi(&r);
    assert!(pagine.len() >= 2, "pagine: {}", pagine.len());
    for (i, pagina) in pagine.iter().enumerate() {
        // L'intestazione delle colonne in cima a ogni pagina della tabella.
        assert!(
            contiene(pagina, "Importo €"),
            "pagina {} senza intestazione",
            i + 1
        );
        assert!(contiene(pagina, &format!("pag. {}", i + 1)));
    }
    let tutto = pagine.concat();
    assert!(contiene(&tutto, "PROVA"));
    assert!(contiene(&tutto, "Nota della sezione."));
    assert!(contiene(&tutto, "Riga di prova 150"));
    assert_eq!(dopo(&tutto, "TOTALE", 1), vec!["1´132´500,00"]);
    // Senza frontespizio, la firma è quella del progettista.
    assert!(contiene(&tutto, "Il Tecnico"));
    assert!(contiene(&tutto, "Ing. Maria Bianchi"));
}

#[test]
fn computo_tradizionale_con_la_sola_incidenza_della_sicurezza() {
    use opencomputo_core::numeri::Percentuale;
    use rust_decimal_macros::dec;
    let mut p = progetto();
    p.computi[0].voci[1].prezzo.incidenza_sicurezza = Some(Percentuale::new(dec!(3)));
    let id = p.computi[0].id;
    let o = OpzioniTradizionale {
        incidenze: tradizionale::Incidenze {
            sicurezza: true,
            ..tradizionale::Incidenze::default()
        },
        ..opzioni()
    };
    let tutto = testi(&tradizionale::computo(&p, id, &o).unwrap()).concat();
    // Colonne «SICUREZZA» con quota in euro e incidenza (ADR 0021).
    assert!(contiene(&tutto, "SICUREZZA") && contiene(&tutto, "incid.%"));
    assert!(!contiene(&tutto, "MANODOPERA"));
    assert_eq!(
        dopo(&tutto, "SOMMANO m³", 4),
        vec!["126,00", "8,50", "1´071,00", "—"]
    );
    let i = tutto.iter().position(|t| t == "7´517,82").unwrap();
    assert_eq!(tutto[i + 1..i + 3], ["225,53", "3,00"]);
    // La quota ha il suo progressivo nei riporti di pagina.
    assert!(
        tutto
            .windows(3)
            .any(|w| w == ["R I P O R T O ", "13´257,12", "225,53"])
    );
    // Incidenza sul totale: 225,53 / 14 457,12 = 1,56 %.
    assert_eq!(
        dopo(&tutto, "T O T A L E   euro", 3),
        vec!["14´457,12", "225,53", "1,56"]
    );
}

/// Testi di una pagina SVG dell'anteprima, nell'ordine di disegno.
fn testi_svg(svg: &str) -> Vec<String> {
    svg.split("<text ")
        .skip(1)
        .map(|t| {
            let inizio = t.find('>').unwrap() + 1;
            let fine = t.find("</text>").unwrap();
            t[inizio..fine]
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&amp;", "&")
        })
        .collect()
}

/// L'anteprima è il PDF: stesse pagine, stessi testi nello stesso ordine, stesso numero di
/// filetti; un SVG ben formato fatto solo di testo, linee e rettangoli.
#[test]
fn anteprima_svg_uguale_al_pdf() {
    let p = progetto();
    let id = p.computi[0].id;
    for r in [
        tradizionale::computo(&p, id, &opzioni()).unwrap(),
        tradizionale::elenco_prezzi(&p, &opzioni()),
    ] {
        assert_eq!(r.anteprima.len(), r.pagine);
        let pdf = testi(&r);
        let contenuti = String::from_utf8_lossy(&r.byte);
        let mut filetti = 0;
        for (svg, testi_pdf) in r.anteprima.iter().zip(&pdf) {
            assert!(svg.starts_with(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 595.32 841.92\""
            ));
            assert!(svg.ends_with("</svg>"));
            assert_eq!(&testi_svg(svg), testi_pdf);
            // Solo elementi di disegno: niente script, collegamenti o immagini esterne.
            for elemento in svg.split('<').skip(1) {
                let nome = elemento
                    .split([' ', '>', '/'])
                    .find(|s| !s.is_empty())
                    .unwrap();
                assert!(["svg", "rect", "line", "text"].contains(&nome), "{nome}");
            }
            assert!(!svg.contains("href"));
            filetti += svg.matches("<line ").count() + svg.matches("fill=\"none\"").count();
        }
        // Ogni tratto del PDF (`S`) ha il suo elemento nell'anteprima.
        assert_eq!(filetti, contenuti.matches(" S\n").count());
    }
}

/// Testi con caratteri speciali di XML restano testo.
#[test]
fn anteprima_svg_con_caratteri_speciali() {
    let mut p = progetto();
    p.elenco_prezzi.voci[0].descrizione_breve =
        "Tubo Ø 20 <prova> & \"altro\" </text><script>".into();
    p.elenco_prezzi.voci[0].descrizione_estesa = p.elenco_prezzi.voci[0].descrizione_breve.clone();
    let r = tradizionale::elenco_prezzi(&p, &opzioni());
    let tutto = r.anteprima.concat();
    assert!(!tutto.contains("<script") && !tutto.contains("<prova>"));
    assert!(
        tutto.contains("&lt;prova&gt; &amp; &quot;altro&quot;"),
        "testo non trovato"
    );
}
