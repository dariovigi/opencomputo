//! Dati di prova condivisi dai test di integrazione.
//!
//! Tutti i valori attesi citati nei test sono stati calcolati a mano e sono
//! riportati nei commenti accanto ai dati.

#![allow(dead_code, missing_docs)]

use chrono::NaiveDate;
use opencomputo_core::prelude::*;
use rust_decimal_macros::dec;

pub const ELENCO: PrezzarioId = PrezzarioId::da_u128(1000);
pub const SCAVO: VocePrezzarioId = VocePrezzarioId::da_u128(1001);
pub const CLS: VocePrezzarioId = VocePrezzarioId::da_u128(1002);
pub const MURATURA: VocePrezzarioId = VocePrezzarioId::da_u128(1003);
pub const RECINZIONE: VocePrezzarioId = VocePrezzarioId::da_u128(1004);

pub const SC_EDILI: ClasseId = ClasseId::da_u128(2001);
pub const CAT_SCAVI: ClasseId = ClasseId::da_u128(2101);
pub const CAT_STRUTTURE: ClasseId = ClasseId::da_u128(2102);
pub const CAT_MURATURE: ClasseId = ClasseId::da_u128(2103);

pub const WBS_EDIFICIO: WbsNodoId = WbsNodoId::da_u128(3001);
pub const WBS_FONDAZIONI: WbsNodoId = WbsNodoId::da_u128(3002);
pub const WBS_ELEVAZIONI: WbsNodoId = WbsNodoId::da_u128(3003);
pub const WBS_SICUREZZA: WbsNodoId = WbsNodoId::da_u128(3004);

pub const COMPUTO: ComputoId = ComputoId::da_u128(4000);
pub const VC_SCAVO: VoceComputoId = VoceComputoId::da_u128(4001);
pub const VC_CLS: VoceComputoId = VoceComputoId::da_u128(4002);
pub const VC_MURATURA: VoceComputoId = VoceComputoId::da_u128(4003);
pub const VC_RECINZIONE: VoceComputoId = VoceComputoId::da_u128(4004);

pub fn data(a: i32, m: u32, g: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, g).expect("data valida")
}

fn voce(
    id: VocePrezzarioId,
    codice: &str,
    descrizione: &str,
    um: &str,
    prezzo: Decimal,
    mdo: Option<Decimal>,
) -> VocePrezzario {
    let mut v = VocePrezzario::new(id, codice, descrizione, um, Prezzo::new(prezzo));
    v.incidenza_manodopera = mdo.map(Percentuale::new);
    v
}

pub fn elenco_prezzi() -> Prezzario {
    let mut p = Prezzario::new(ELENCO, "Elenco prezzi di progetto");
    p.edizione = Some("2026".into());
    p.aggiungi_voce(voce(
        SCAVO,
        "A.01.001",
        "Scavo di sbancamento",
        "m³",
        dec!(8.50),
        Some(dec!(25)),
    ))
    .unwrap();
    p.aggiungi_voce(voce(
        CLS,
        "B.02.010",
        "Calcestruzzo per fondazioni C25/30",
        "m³",
        dec!(145.30),
        Some(dec!(18.5)),
    ))
    .unwrap();
    p.aggiungi_voce(voce(
        MURATURA,
        "C.03.020",
        "Muratura in blocchi di laterizio",
        "m²",
        dec!(42.75),
        Some(dec!(45)),
    ))
    .unwrap();
    let mut recinzione = voce(
        RECINZIONE,
        "S.01.001",
        "Recinzione provvisoria di cantiere",
        "m",
        dec!(12.00),
        None,
    );
    recinzione.natura = NaturaVoce::OneriSicurezza;
    p.aggiungi_voce(recinzione).unwrap();
    p
}

fn classe(id: ClasseId, codice: &str, descrizione: &str) -> Classe {
    Classe {
        id,
        codice: codice.into(),
        descrizione: descrizione.into(),
    }
}

fn nodo(id: WbsNodoId, padre: Option<WbsNodoId>, codice: &str, descrizione: &str) -> NodoWbs {
    NodoWbs {
        id,
        padre,
        codice: codice.into(),
        descrizione: descrizione.into(),
    }
}

fn rigo(n: u128) -> RigoMisura {
    RigoMisura::misura(RigoMisuraId::da_u128(n), "")
}

/// Progetto di esempio con un computo di quattro voci.
///
/// | voce | quantità | prezzo | importo | manodopera |
/// |------|---------:|-------:|--------:|-----------:|
/// | scavo      | 126,00 | 8,50   | 1 071,00 | 267,75 |
/// | cls        |  51,74 | 145,30 | 7 517,82 | 1 390,80 |
/// | muratura   | 109,20 | 42,75  | 4 668,30 | 2 100,74 |
/// | recinzione | 100,00 | 12,00  | 1 200,00 (sicurezza) | — |
///
/// Totale lavori 13 257,12; sicurezza 1 200,00; totale 14 457,12.
pub fn progetto_di_esempio() -> Progetto {
    let mut p = Progetto::new(ProgettoId::da_u128(1), "Nuova scuola primaria", ELENCO);
    p.metadati.committente = Some("Comune di Esempio".into());
    p.metadati.luogo = Some("Via Roma 1".into());
    p.elenco_prezzi = elenco_prezzi();

    p.categorie
        .aggiungi(
            LivelloCategoria::SuperCategoria,
            classe(SC_EDILI, "OE", "Opere edili"),
        )
        .unwrap();
    p.categorie
        .aggiungi(
            LivelloCategoria::Categoria,
            classe(CAT_SCAVI, "SC", "Scavi"),
        )
        .unwrap();
    p.categorie
        .aggiungi(
            LivelloCategoria::Categoria,
            classe(CAT_STRUTTURE, "ST", "Strutture"),
        )
        .unwrap();
    p.categorie
        .aggiungi(
            LivelloCategoria::Categoria,
            classe(CAT_MURATURE, "MU", "Murature"),
        )
        .unwrap();

    p.wbs
        .aggiungi(nodo(WBS_EDIFICIO, None, "01", "Edificio A"))
        .unwrap();
    p.wbs
        .aggiungi(nodo(WBS_FONDAZIONI, Some(WBS_EDIFICIO), "01", "Fondazioni"))
        .unwrap();
    p.wbs
        .aggiungi(nodo(WBS_ELEVAZIONI, Some(WBS_EDIFICIO), "02", "Elevazioni"))
        .unwrap();
    p.wbs
        .aggiungi(nodo(WBS_SICUREZZA, None, "02", "Sicurezza"))
        .unwrap();

    let mut c = Computo::new(COMPUTO, "Computo metrico estimativo");
    let elenco = &p.elenco_prezzi;

    // 1. Scavo: 12 × 8 × 1,20 = 115,20; rampa 0,5 × 6 × 3 × 1,20 = 10,80 → 126,00
    let v = c
        .aggiungi_da_prezzario(VC_SCAVO, elenco, SCAVO, None)
        .unwrap();
    v.categorie.super_categoria = Some(SC_EDILI);
    v.categorie.categoria = Some(CAT_SCAVI);
    v.wbs = Some(WBS_FONDAZIONI);
    v.righi = vec![
        RigoMisura::misura(RigoMisuraId::da_u128(5001), "Platea")
            .con_lunghezza(dec!(12.00))
            .con_larghezza(dec!(8.00))
            .con_altezza(dec!(1.20)),
        RigoMisura::misura(RigoMisuraId::da_u128(5002), "Rampa di accesso")
            .con_lunghezza(dec!(6.00))
            .con_larghezza(dec!(3.00))
            .con_altezza(dec!(1.20))
            .con_formula("0,5*lun*lar*alt"),
    ];

    // 2. Calcestruzzo: 12 × 8 × 0,50 = 48,00; cordoli 4 × 7,80 × 0,30 × 0,40 = 3,744 → 3,74
    let v = c.aggiungi_da_prezzario(VC_CLS, elenco, CLS, None).unwrap();
    v.categorie.super_categoria = Some(SC_EDILI);
    v.categorie.categoria = Some(CAT_STRUTTURE);
    v.wbs = Some(WBS_FONDAZIONI);
    v.righi = vec![
        RigoMisura::misura(RigoMisuraId::da_u128(5011), "Platea")
            .con_lunghezza(dec!(12.00))
            .con_larghezza(dec!(8.00))
            .con_altezza(dec!(0.50)),
        RigoMisura::misura(RigoMisuraId::da_u128(5012), "Cordoli")
            .con_parti(dec!(4))
            .con_lunghezza("7,50+0,30")
            .con_larghezza(dec!(0.30))
            .con_altezza(dec!(0.40)),
    ];

    // 3. Muratura: 2 × 12 × 3 = 72; 2 × 8 × 3 = 48; finestre −6 × 1,20 × 1,50 = −10,80 → 109,20
    let v = c
        .aggiungi_da_prezzario(VC_MURATURA, elenco, MURATURA, None)
        .unwrap();
    v.categorie.super_categoria = Some(SC_EDILI);
    v.categorie.categoria = Some(CAT_MURATURE);
    v.wbs = Some(WBS_ELEVAZIONI);
    v.righi = vec![
        RigoMisura::commento(RigoMisuraId::da_u128(5020), "Piano terra"),
        RigoMisura::misura(RigoMisuraId::da_u128(5021), "Pareti lato lungo")
            .con_parti(dec!(2))
            .con_lunghezza(dec!(12.00))
            .con_altezza(dec!(3.00)),
        RigoMisura::misura(RigoMisuraId::da_u128(5022), "Pareti lato corto")
            .con_parti(dec!(2))
            .con_lunghezza(dec!(8.00))
            .con_altezza(dec!(3.00)),
        RigoMisura::misura(RigoMisuraId::da_u128(5023), "A dedurre finestre")
            .con_parti(dec!(6))
            .con_lunghezza(dec!(1.20))
            .con_altezza(dec!(1.50))
            .come_deduzione(),
    ];

    // 4. Recinzione (oneri della sicurezza): 2 × (30 + 20) = 100,00
    let v = c
        .aggiungi_da_prezzario(VC_RECINZIONE, elenco, RECINZIONE, None)
        .unwrap();
    v.wbs = Some(WBS_SICUREZZA);
    v.righi = vec![rigo(5031).con_lunghezza("2*(30+20)")];

    p.aggiungi_computo(c).unwrap();
    p
}

/// Registrazione del libretto con un solo rigo `lun × lar × alt` (fattori opzionali).
pub fn registrazione(
    progetto: &Progetto,
    id: u128,
    numero: u32,
    giorno: NaiveDate,
    voce: VocePrezzarioId,
    fattori: &[Decimal],
) -> RegistrazioneLibretto {
    let elenco = &progetto.elenco_prezzi;
    let prezzo = PrezzoApplicato::da_voce(elenco, elenco.voce(voce).unwrap());
    let mut r = RegistrazioneLibretto::new(RegistrazioneId::da_u128(id), numero, giorno, prezzo);
    let mut rigo = rigo(id * 10);
    if let Some(v) = fattori.first() {
        rigo = rigo.con_lunghezza(*v);
    }
    if let Some(v) = fattori.get(1) {
        rigo = rigo.con_larghezza(*v);
    }
    if let Some(v) = fattori.get(2) {
        rigo = rigo.con_altezza(*v);
    }
    r.righi.push(rigo);
    r
}

pub const SAL1: SalId = SalId::da_u128(9001);
pub const SAL2: SalId = SalId::da_u128(9002);

/// Progetto con contratto (ribasso 12,5 %, ritenuta 0,5 %) e quattro registrazioni.
///
/// | n. | data       | voce       | quantità | importo  | manodopera |
/// |----|------------|------------|---------:|---------:|-----------:|
/// | 1  | 10/03/2026 | scavo      | 115,20   |   979,20 |   244,80   |
/// | 2  | 20/03/2026 | cls        |  48,00   | 6 974,40 | 1 290,26   |
/// | 3  | 25/03/2026 | recinzione | 100,00   | 1 200,00 (sicurezza) | — |
/// | 4  | 15/04/2026 | muratura   |  72,00   | 3 078,00 | 1 385,10   |
pub fn progetto_con_contabilita() -> Progetto {
    let mut p = progetto_di_esempio();
    p.contratto = Some(DatiContratto {
        parametri: ParametriContabili {
            ribasso: Percentuale::new(dec!(12.5)),
            ritenuta_garanzia: Percentuale::new(dec!(0.5)),
            manodopera_esclusa_dal_ribasso: false,
            aliquota_iva: None,
            anticipazione: None,
        },
        ..DatiContratto::default()
    });
    let registrazioni = [
        registrazione(
            &p,
            1,
            1,
            data(2026, 3, 10),
            SCAVO,
            &[dec!(12), dec!(8), dec!(1.2)],
        ),
        registrazione(
            &p,
            2,
            2,
            data(2026, 3, 20),
            CLS,
            &[dec!(12), dec!(8), dec!(0.5)],
        ),
        registrazione(&p, 3, 3, data(2026, 3, 25), RECINZIONE, &[dec!(100)]),
        registrazione(&p, 4, 4, data(2026, 4, 15), MURATURA, &[dec!(24), dec!(3)]),
    ];
    for r in registrazioni {
        p.contabilita.registra(r).unwrap();
    }
    p
}

pub const PENALE_1: PenaleId = PenaleId::da_u128(7101);
pub const PENALE_2: PenaleId = PenaleId::da_u128(7102);

/// Contratto di 13.000,00 € con IVA 10 %, anticipazione 20 % (2.600,00 €) recuperata al
/// 20 % e una penale di 3 giorni allo 0,5 ‰ (19,50 €, data nel periodo del SAL 2); SAL 1
/// e 2 emessi; una registrazione dopo il SAL 2 (n. 5, muratura 30 m² = 1.282,50 €) e una
/// penale per l'ultimazione (4 giorni all'1 ‰ = 52,00 €). Conto finale in bozza
/// (ultimazione 25/05/2026, conto del 15/06/2026). Valori in `tests/conto_finale.rs`.
pub fn progetto_per_conto_finale() -> Progetto {
    let mut p = progetto_con_contabilita();
    let c = p.contratto.as_mut().unwrap();
    c.importo_contrattuale = Some(Importo::new(dec!(13000.00)));
    c.parametri.aliquota_iva = Some(Percentuale::new(dec!(10)));
    c.parametri.anticipazione = Some(Anticipazione {
        percentuale: Percentuale::new(dec!(20)),
        importo: Importo::new(dec!(2600.00)),
        percentuale_recupero: Percentuale::new(dec!(20)),
        data_erogazione: Some(data(2026, 3, 2)),
    });
    p.contabilita
        .registra_penale(Penale {
            id: PENALE_1,
            numero: 1,
            data: data(2026, 4, 20),
            descrizione: "Ritardo sulla scadenza intermedia".into(),
            importo: Importo::new(dec!(19.50)),
            giorni: Some(3),
            per_mille: Some(dec!(0.5)),
        })
        .unwrap();
    p.contabilita.nuovo_sal(SAL1, data(2026, 3, 31)).unwrap();
    p.emetti_sal(SAL1, data(2026, 4, 5)).unwrap();
    p.contabilita.nuovo_sal(SAL2, data(2026, 4, 30)).unwrap();
    p.emetti_sal(SAL2, data(2026, 5, 6)).unwrap();
    let r5 = registrazione(&p, 5, 5, data(2026, 5, 20), MURATURA, &[dec!(10), dec!(3)]);
    p.contabilita.registra(r5).unwrap();
    p.contabilita
        .registra_penale(Penale {
            id: PENALE_2,
            numero: 2,
            data: data(2026, 5, 25),
            descrizione: "Ritardo nell'ultimazione dei lavori".into(),
            importo: Importo::new(dec!(52.00)),
            giorni: Some(4),
            per_mille: Some(dec!(1)),
        })
        .unwrap();
    p.contabilita
        .redigi_conto_finale(data(2026, 5, 25), data(2026, 6, 15), None)
        .unwrap();
    p
}

pub const NP1: VocePrezzarioId = VocePrezzarioId::da_u128(1101);
pub const NP2: VocePrezzarioId = VocePrezzarioId::da_u128(1102);
pub const AN_NP1: AnalisiId = AnalisiId::da_u128(1201);

/// Progetto con due nuovi prezzi e un verbale di concordamento, senza SAL.
///
/// * NP.1 «Intonaco premiscelato» (m²), formato con analisi e già netto (non soggetto a
///   ribasso): operaio comune 0,25 h × 32,00 = 8,00; premiscelato 12 kg × 0,50 = 6,00;
///   costo 14,00; spese generali 15 % = 2,10; utile 10 % di 16,10 = 1,61; prezzo 17,71;
///   incidenza manodopera 8 / 17,71 = 45,17 %.
/// * NP.2 «Sigillatura giunti» (m), desunto dal prezzario, 10,00 €, soggetto a ribasso.
/// * Verbale n. 1 del 10/04/2026 con entrambi, firmato il 12/04 (approvazione assente).
/// * Registrazioni n. 5 (NP.1, 10 m² = 177,10, 20/04/2026) e n. 6 (NP.2, 5 m = 50,00,
///   21/04/2026).
pub fn progetto_con_nuovi_prezzi() -> Progetto {
    let mut p = progetto_con_contabilita();
    let e = &mut p.elenco_prezzi;
    let comune = RisorsaElementare {
        id: RisorsaId::da_u128(1301),
        codice: "MO.02".into(),
        descrizione: "Operaio comune".into(),
        unita_misura: "h".into(),
        tipo: TipoRisorsa::Manodopera,
        prezzo: Prezzo::new(dec!(32.00)),
    };
    let premiscelato = RisorsaElementare {
        id: RisorsaId::da_u128(1302),
        codice: "MA.20".into(),
        descrizione: "Intonaco premiscelato in sacchi".into(),
        unita_misura: "kg".into(),
        tipo: TipoRisorsa::Materiale,
        prezzo: Prezzo::new(dec!(0.50)),
    };
    e.analisi.push(AnalisiPrezzo {
        id: AN_NP1,
        codice: "NP.01".into(),
        descrizione: "Intonaco premiscelato".into(),
        unita_misura: "m²".into(),
        quantita_riferimento: Quantita::new(dec!(1)),
        righe: vec![
            RigaAnalisi::da_risorsa(&comune, Quantita::new(dec!(0.25))),
            RigaAnalisi::da_risorsa(&premiscelato, Quantita::new(dec!(12))),
        ],
        spese_generali: Percentuale::new(dec!(15)),
        utile_impresa: Percentuale::new(dec!(10)),
        base_utile: BaseUtile::CostoPiuSpeseGenerali,
        variabili: Vec::new(),
        quota_oneri_sicurezza: None,
    });
    e.risorse = vec![comune, premiscelato];
    let mut np1 = VocePrezzario::new(
        NP1,
        "NP.01",
        "Intonaco premiscelato",
        "m²",
        Prezzo::new(dec!(17.71)),
    );
    np1.analisi = Some(AN_NP1);
    np1.incidenza_manodopera = Some(Percentuale::new(dec!(45.17)));
    np1.nuovo_prezzo = Some(NuovoPrezzo {
        numero: 1,
        data: data(2026, 4, 8),
        motivo: "Lavorazione non prevista nel progetto".into(),
        criterio: CriterioNuovoPrezzo::Analisi,
        soggetto_a_ribasso: false,
        verbale: Some(1),
    });
    let mut np2 = VocePrezzario::new(
        NP2,
        "NP.02",
        "Sigillatura giunti",
        "m",
        Prezzo::new(dec!(10.00)),
    );
    np2.nuovo_prezzo = Some(NuovoPrezzo {
        numero: 2,
        data: data(2026, 4, 8),
        motivo: "Variante".into(),
        criterio: CriterioNuovoPrezzo::Prezzario,
        soggetto_a_ribasso: true,
        verbale: Some(1),
    });
    e.aggiungi_voce(np1).unwrap();
    e.aggiungi_voce(np2).unwrap();
    p.contabilita
        .imposta_verbale_nuovi_prezzi(VerbaleNuoviPrezzi {
            numero: 1,
            data: data(2026, 4, 10),
            firma_esecutore: Some(data(2026, 4, 12)),
            riserve: None,
            approvato_il: None,
            atto_approvazione: None,
            note: None,
        })
        .unwrap();
    let r5 = registrazione(&p, 5, 5, data(2026, 4, 20), NP1, &[dec!(10)]);
    let r6 = registrazione(&p, 6, 6, data(2026, 4, 21), NP2, &[dec!(5)]);
    p.contabilita.registra(r5).unwrap();
    p.contabilita.registra(r6).unwrap();
    p
}
