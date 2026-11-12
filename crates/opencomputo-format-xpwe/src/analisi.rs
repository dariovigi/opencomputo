//! Analisi dei prezzi di una voce di elenco (`PweEPAnalisi`).
//!
//! Fatti osservati in un export reale (formato 5.01): ogni componente è un `EPARItem`
//! con `Tipo`, `IDEP` (`-2` per un componente libero), `Descrizione`, `Misura`, `Qt` e
//! `Prezzo` (con la virgola decimale). Il file **non** contiene le percentuali di spese
//! generali e utile, né il tipo di risorsa di ogni componente: contiene il prezzo
//! risultante (`Prezzo1`) e le incidenze di materiali, manodopera e attrezzature
//! (`IncMAT`, `IncMDO`, `IncATTR`) sul prezzo.
//!
//! Qui stanno le due deduzioni, separate dalla lettura del file: le percentuali, cercate
//! tra i valori d'uso finché il calcolo del core riproduce il prezzo dichiarato, e i tipi
//! di risorsa, scelti in modo che le incidenze coincidano con quelle dichiarate. Quando
//! una deduzione non riesce l'import lo dice: il prezzo della voce resta sempre quello
//! dichiarato nel file.

use opencomputo_core::analisi::{AnalisiPrezzo, TipoRisorsa};
use opencomputo_core::numeri::{Percentuale, Prezzo, RegoleArrotondamento};
use rust_decimal::Decimal;

/// Coppie (spese generali, utile) provate nell'ordine: prima i valori d'uso per i lavori
/// (spese generali dal 13 al 17 %, utile 10 %), poi senza utile (costi della sicurezza),
/// poi nessuna maggiorazione, infine una griglia più ampia a passi di mezzo punto.
fn candidate() -> Vec<(Decimal, Decimal)> {
    let dieci = Decimal::TEN;
    let mut c: Vec<(Decimal, Decimal)> = Vec::new();
    for utile in [dieci, Decimal::ZERO] {
        for sg in [15, 17, 13, 14, 16] {
            c.push((Decimal::from(sg), utile));
        }
    }
    c.push((Decimal::ZERO, Decimal::ZERO));
    for utile in [dieci, Decimal::ZERO] {
        for mezzi in 0..=60 {
            let sg = Decimal::new(mezzi * 5, 1);
            if !c.contains(&(sg, utile)) {
                c.push((sg, utile));
            }
        }
    }
    c
}

/// Esito della ricerca di spese generali e utile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Percentuali {
    /// Spese generali.
    pub spese_generali: Decimal,
    /// Utile d'impresa.
    pub utile: Decimal,
    /// Prezzo che il core calcola con queste percentuali (arrotondando ogni componente
    /// e ogni passaggio): può differire di qualche centesimo da quello dichiarato.
    pub calcolato: Prezzo,
}

/// Cerca spese generali e utile che riproducono il prezzo dichiarato e li lascia
/// impostati nell'analisi.
///
/// Una coppia è accettata se il calcolo del core dà il prezzo dichiarato, oppure se lo dà
/// il calcolo senza arrotondamenti intermedi (costo × (1 + spese generali) × (1 + utile),
/// arrotondato solo alla fine): fatto osservato in un file reale, dove i due calcoli
/// differiscono di un centesimo.
pub(crate) fn deduci_percentuali(
    analisi: &mut AnalisiPrezzo,
    dichiarato: Prezzo,
    regole: &RegoleArrotondamento,
) -> Option<Percentuali> {
    let costo: Decimal = analisi
        .righe
        .iter()
        .filter_map(|r| r.quantita.valore().checked_mul(r.prezzo.valore()))
        .sum();
    let cento = Decimal::ONE_HUNDRED;
    let decimali = dichiarato.valore().scale().max(2);
    for (sg, utile) in candidate() {
        analisi.spese_generali = Percentuale::new(sg);
        analisi.utile_impresa = Percentuale::new(utile);
        let Ok(r) = analisi.calcola(regole) else {
            continue;
        };
        let senza_arrotondamenti = costo
            .checked_mul((cento + sg) / cento)
            .and_then(|x| x.checked_mul((cento + utile) / cento))
            .map(|x| {
                x.round_dp_with_strategy(
                    decimali,
                    rust_decimal::RoundingStrategy::MidpointAwayFromZero,
                )
            });
        if r.prezzo_unitario == dichiarato || senza_arrotondamenti == Some(dichiarato.valore()) {
            return Some(Percentuali {
                spese_generali: sg,
                utile,
                calcolato: r.prezzo_unitario,
            });
        }
    }
    analisi.spese_generali = Percentuale::ZERO;
    analisi.utile_impresa = Percentuale::ZERO;
    None
}

/// Unità di misura di tempo: la riga è con ogni probabilità manodopera.
fn a_ore(unita: &str) -> bool {
    matches!(
        unita.trim().to_lowercase().as_str(),
        "h" | "ora" | "ore" | "hh"
    )
}

/// Tipo più probabile di una riga senza altre informazioni.
fn tipo_probabile(unita: &str) -> TipoRisorsa {
    if a_ore(unita) {
        TipoRisorsa::Manodopera
    } else {
        TipoRisorsa::Materiale
    }
}

/// Oltre questo numero di righe non si provano tutte le combinazioni.
const RIGHE_MASSIME: usize = 8;

/// Scostamento ammesso tra incidenza calcolata e dichiarata, in punti percentuali.
fn tolleranza() -> Decimal {
    Decimal::new(2, 2)
}

/// Sceglie il tipo di risorsa di ogni riga.
///
/// `importi` e `unita` sono per riga; `prezzo` è il prezzo dichiarato della voce;
/// `dichiarate` sono le incidenze percentuali sul prezzo di materiali, manodopera e
/// attrezzature. Restituisce i tipi e `true` se riproducono le incidenze dichiarate.
/// Senza incidenze, o se nessuna combinazione le riproduce, vale il tipo più probabile
/// (ore → manodopera, altrimenti materiale) e il secondo valore è `false`.
pub(crate) fn classifica(
    importi: &[Decimal],
    unita: &[&str],
    prezzo: Decimal,
    dichiarate: [Decimal; 3],
) -> (Vec<TipoRisorsa>, bool) {
    const TIPI: [TipoRisorsa; 4] = [
        TipoRisorsa::Materiale,
        TipoRisorsa::Manodopera,
        TipoRisorsa::Nolo,
        TipoRisorsa::Altro,
    ];
    let probabili: Vec<TipoRisorsa> = unita.iter().map(|u| tipo_probabile(u)).collect();
    let n = importi.len();
    if n == 0
        || n > RIGHE_MASSIME
        || prezzo <= Decimal::ZERO
        || dichiarate.iter().all(Decimal::is_zero)
    {
        return (probabili, false);
    }
    let mut migliore: Option<(usize, Vec<TipoRisorsa>)> = None;
    let combinazioni = 4usize.pow(u32::try_from(n).unwrap_or(0));
    for mut k in 0..combinazioni {
        let mut somme = [Decimal::ZERO; 3];
        let mut tipi = Vec::with_capacity(n);
        for importo in importi {
            let t = k % 4;
            k /= 4;
            if let Some(s) = somme.get_mut(t) {
                *s += *importo;
            }
            tipi.push(TIPI.get(t).copied().unwrap_or(TipoRisorsa::Altro));
        }
        let coincide = somme
            .iter()
            .zip(dichiarate)
            .all(|(s, d)| (*s * Decimal::ONE_HUNDRED / prezzo - d).abs() <= tolleranza());
        if !coincide {
            continue;
        }
        // Tra le combinazioni che tornano, quella più vicina ai tipi probabili.
        let accordo = tipi.iter().zip(&probabili).filter(|(a, b)| a == b).count();
        if migliore.as_ref().is_none_or(|(m, _)| accordo > *m) {
            migliore = Some((accordo, tipi));
        }
    }
    match migliore {
        Some((_, tipi)) => (tipi, true),
        None => (probabili, false),
    }
}

#[cfg(test)]
mod prove {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn tipi_dalle_incidenze_dichiarate() {
        // Tecnico a ore che non è manodopera d'opera: lo dicono le incidenze.
        let importi = [dec!(112.50), dec!(13.27), dec!(50), dec!(10)];
        let unita = ["h", "h", "", ""];
        let (tipi, ok) = classifica(
            &importi,
            &unita,
            dec!(239.09),
            [dec!(4.1826), dec!(5.5503), dec!(20.913)],
        );
        assert!(ok);
        assert_eq!(
            tipi,
            [
                TipoRisorsa::Altro,
                TipoRisorsa::Manodopera,
                TipoRisorsa::Nolo,
                TipoRisorsa::Materiale
            ]
        );
    }

    #[test]
    fn senza_incidenze_vale_il_tipo_probabile() {
        let (tipi, ok) = classifica(
            &[dec!(10), dec!(20)],
            &["mc", " H "],
            dec!(40),
            [Decimal::ZERO; 3],
        );
        assert!(!ok);
        assert_eq!(tipi, [TipoRisorsa::Materiale, TipoRisorsa::Manodopera]);
        // Incidenze che nessuna combinazione riproduce.
        let (tipi, ok) = classifica(
            &[dec!(10), dec!(20)],
            &["mc", "h"],
            dec!(40),
            [dec!(99), dec!(1), Decimal::ZERO],
        );
        assert!(!ok);
        assert_eq!(tipi, [TipoRisorsa::Materiale, TipoRisorsa::Manodopera]);
    }

    #[test]
    fn candidate_senza_ripetizioni_e_valori_d_uso_per_primi() {
        let c = candidate();
        assert_eq!(c.first(), Some(&(dec!(15), dec!(10))));
        let mut unici = c.clone();
        unici.sort();
        unici.dedup();
        assert_eq!(unici.len(), c.len());
    }
}
