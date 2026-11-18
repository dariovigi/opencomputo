//! Date civili: giorni del calendario, senza fuso orario.
//!
//! Il progetto usa `jiff` (`civil::Date`, `civil::DateTime`). Questo modulo raccoglie
//! le poche operazioni che servono, con la regola di sempre: una data che non esiste o
//! un calcolo fuori dall'intervallo ammesso (anni da -9999 a 9999) danno `None`, mai un
//! panic.
//!
//! Formato nei file e nel servizio JSON: `AAAA-MM-GG` per le date e `AAAA-MM-GGThh:mm:ss`
//! per data e ora, identico a quello scritto fino alla versione 0.1 (che usava `chrono`).

pub use jiff::civil::{Date, DateTime, Weekday};

/// Data da anno, mese e giorno. `None` se non esiste (31 aprile, 29 febbraio di un anno
/// non bisestile) o è fuori dagli anni da -9999 a 9999.
#[must_use]
pub fn data_amg(anno: i32, mese: u32, giorno: u32) -> Option<Date> {
    Date::new(
        i16::try_from(anno).ok()?,
        i8::try_from(mese).ok()?,
        i8::try_from(giorno).ok()?,
    )
    .ok()
}

/// Operazioni sulle date usate dal programma.
pub trait OperazioniData: Sized {
    /// Anno.
    fn anno(self) -> i32;
    /// Mese, da 1 a 12.
    fn mese(self) -> u32;
    /// Giorno del mese, da 1 a 31.
    fn giorno(self) -> u32;
    /// Giorno della settimana: 0 lunedì … 6 domenica.
    fn giorno_settimana(self) -> u32;
    /// Il giorno seguente.
    fn giorno_dopo(self) -> Option<Date>;
    /// Il giorno precedente.
    fn giorno_prima(self) -> Option<Date>;
    /// Data spostata di `giorni` (anche negativi).
    fn piu_giorni(self, giorni: i64) -> Option<Date>;
    /// Data spostata di `mesi` in avanti; il giorno oltre la fine del mese diventa
    /// l'ultimo del mese (31 gennaio + 1 mese = 28 o 29 febbraio).
    fn piu_mesi(self, mesi: u32) -> Option<Date>;
    /// Giorni da `altra` a questa data: positivo se questa viene dopo.
    fn giorni_da(self, altra: Date) -> i64;
    /// Data e ora indicate.
    fn alle(self, ora: u32, minuti: u32, secondi: u32) -> Option<DateTime>;
    /// Data a mezzanotte.
    fn mezzanotte(self) -> DateTime;
}

impl OperazioniData for Date {
    fn anno(self) -> i32 {
        i32::from(self.year())
    }

    fn mese(self) -> u32 {
        u32::try_from(self.month()).unwrap_or(1)
    }

    fn giorno(self) -> u32 {
        u32::try_from(self.day()).unwrap_or(1)
    }

    fn giorno_settimana(self) -> u32 {
        u32::try_from(self.weekday().to_monday_zero_offset()).unwrap_or(0)
    }

    fn giorno_dopo(self) -> Option<Date> {
        self.tomorrow().ok()
    }

    fn giorno_prima(self) -> Option<Date> {
        self.yesterday().ok()
    }

    fn piu_giorni(self, giorni: i64) -> Option<Date> {
        let span = jiff::Span::new().try_days(giorni).ok()?;
        self.checked_add(span).ok()
    }

    fn piu_mesi(self, mesi: u32) -> Option<Date> {
        let span = jiff::Span::new().try_months(i64::from(mesi)).ok()?;
        self.checked_add(span).ok()
    }

    fn giorni_da(self, altra: Date) -> i64 {
        // Tra due date valide la differenza in giorni esiste sempre.
        altra
            .until(self)
            .map_or(0, |span| i64::from(span.get_days()))
    }

    fn alle(self, ora: u32, minuti: u32, secondi: u32) -> Option<DateTime> {
        let t = jiff::civil::Time::new(
            i8::try_from(ora).ok()?,
            i8::try_from(minuti).ok()?,
            i8::try_from(secondi).ok()?,
            0,
        )
        .ok()?;
        Some(self.to_datetime(t))
    }

    fn mezzanotte(self) -> DateTime {
        self.to_datetime(jiff::civil::Time::midnight())
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    fn d(a: i32, m: u32, g: u32) -> Date {
        data_amg(a, m, g).expect("data valida")
    }

    #[test]
    fn date_non_valide_o_fuori_intervallo() {
        assert_eq!(data_amg(2026, 2, 29), None);
        assert_eq!(data_amg(2024, 2, 29).map(OperazioniData::giorno), Some(29));
        assert_eq!(data_amg(2026, 4, 31), None);
        assert_eq!(data_amg(2026, 13, 1), None);
        assert_eq!(data_amg(2026, 0, 1), None);
        assert_eq!(data_amg(10_000, 1, 1), None);
        assert_eq!(data_amg(-10_000, 1, 1), None);
        assert_eq!(data_amg(i32::MAX, u32::MAX, u32::MAX), None);
        assert_eq!(d(9999, 12, 31).giorno_dopo(), None);
        assert_eq!(d(-9999, 1, 1).giorno_prima(), None);
        assert_eq!(d(2026, 1, 1).piu_giorni(i64::MAX), None);
        assert_eq!(d(9999, 12, 1).piu_mesi(1), None);
        assert_eq!(d(2026, 1, 1).alle(24, 0, 0), None);
    }

    #[test]
    fn calcoli() {
        let x = d(2026, 1, 31);
        assert_eq!((x.anno(), x.mese(), x.giorno()), (2026, 1, 31));
        assert_eq!(x.piu_mesi(1), Some(d(2026, 2, 28)));
        assert_eq!(d(2024, 1, 31).piu_mesi(1), Some(d(2024, 2, 29)));
        assert_eq!(x.piu_giorni(1), Some(d(2026, 2, 1)));
        assert_eq!(x.piu_giorni(-31), Some(d(2025, 12, 31)));
        assert_eq!(x.giorno_dopo(), Some(d(2026, 2, 1)));
        assert_eq!(x.giorno_prima(), Some(d(2026, 1, 30)));
        assert_eq!(d(2026, 12, 31).giorni_da(d(2026, 1, 1)), 364);
        assert_eq!(d(2026, 1, 1).giorni_da(d(2026, 12, 31)), -364);
        assert_eq!(d(2030, 1, 1).giorni_da(d(2026, 1, 1)), 1461);
        // 19 novembre 2026: giovedì.
        assert_eq!(d(2026, 11, 19).giorno_settimana(), 3);
        assert_eq!(d(2026, 11, 16).giorno_settimana(), 0);
        assert_eq!(d(2026, 11, 22).giorno_settimana(), 6);
        assert_eq!(x.mezzanotte().to_string(), "2026-01-31T00:00:00");
        assert_eq!(
            x.alle(13, 5, 9).map(|t| t.to_string()).as_deref(),
            Some("2026-01-31T13:05:09")
        );
    }

    #[test]
    fn formato_nei_file() {
        // Identico al formato scritto con chrono: i progetti salvati restano leggibili.
        let x = d(2026, 4, 5);
        assert_eq!(serde_json::to_string(&x).unwrap(), "\"2026-04-05\"");
        assert_eq!(serde_json::from_str::<Date>("\"2026-04-05\"").unwrap(), x);
        let t = x.alle(13, 5, 9).unwrap();
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            "\"2026-04-05T13:05:09\""
        );
        assert_eq!(
            serde_json::from_str::<DateTime>("\"2026-04-05T13:05:09\"").unwrap(),
            t
        );
        // Secondi con decimali, come li scriveva chrono.
        assert!(serde_json::from_str::<DateTime>("\"2026-04-05T13:05:09.120\"").is_ok());
        assert_eq!(
            serde_json::to_string(&d(1, 1, 1)).unwrap(),
            "\"0001-01-01\""
        );
        assert_eq!(
            serde_json::to_string(&d(9999, 12, 31)).unwrap(),
            "\"9999-12-31\""
        );
        // Testi non validi rifiutati.
        for t in ["2026-02-30", "26-04-05", "05/04/2026", "", "2026-04-05x"] {
            assert!(
                serde_json::from_str::<Date>(&format!("\"{t}\"")).is_err(),
                "{t}"
            );
        }
    }
}
