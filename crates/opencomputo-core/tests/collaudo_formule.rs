//! Collaudo lungo (ignorato): 50.000 formule casuali confrontate con un calcolo indipendente e
//! 50.000 testi casuali senza panico. `cargo test -p opencomputo-core --test collaudo_formule -- --ignored --nocapture`
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    missing_docs
)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use opencomputo_core::formula::valuta;
use rust_decimal::Decimal;

struct Rng(u64);
impl Rng {
    fn n(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn sotto(&mut self, m: usize) -> usize {
        (self.n() % m.max(1) as u64) as usize
    }
}

enum E {
    N(Decimal, String),
    Neg(Box<E>),
    B(char, Box<E>, Box<E>),
    P(Box<E>),
    Pow(Box<E>, i64),
}

fn genera(r: &mut Rng, d: usize) -> E {
    if d == 0 || r.sotto(3) == 0 {
        let intero = r.sotto(1000) as i64;
        let dec = r.sotto(100) as i64;
        let con_dec = r.sotto(2) == 0;
        let v = if con_dec {
            Decimal::new(intero * 100 + dec, 2)
        } else {
            Decimal::from(intero)
        };
        let sep = if r.sotto(2) == 0 { "," } else { "." };
        let s = if con_dec {
            format!("{intero}{sep}{dec:02}")
        } else {
            format!("{intero}")
        };
        return E::N(v, s);
    }
    match r.sotto(6) {
        0 => E::Neg(Box::new(genera(r, d - 1))),
        1 => E::P(Box::new(genera(r, d - 1))),
        2 => E::Pow(
            Box::new(E::P(Box::new(genera(r, d - 1)))),
            r.sotto(4) as i64,
        ),
        _ => E::B(
            ['+', '-', '*', '/'][r.sotto(4)],
            Box::new(genera(r, d - 1)),
            Box::new(genera(r, d - 1)),
        ),
    }
}

// Stampa sempre con parentesi esplicite per gli operandi binari: così il valore atteso
// non dipende dalla precedenza; la precedenza si prova a parte.
fn stampa(e: &E, r: &mut Rng) -> String {
    let sp = |r: &mut Rng| if r.sotto(4) == 0 { " " } else { "" };
    match e {
        E::N(_, s) => s.clone(),
        E::Neg(x) => format!("-({})", stampa(x, r)),
        E::P(x) => format!("({})", stampa(x, r)),
        E::Pow(x, k) => format!("{}^{k}", stampa(x, r)),
        E::B(o, a, b) => {
            let (s1, s2) = (sp(r), sp(r));
            format!("({}){s1}{o}{s2}({})", stampa(a, r), stampa(b, r))
        }
    }
}

fn valore(e: &E) -> Option<Decimal> {
    match e {
        E::N(v, _) => Some(*v),
        E::Neg(x) => valore(x).map(|v| -v),
        E::P(x) => valore(x),
        E::Pow(x, k) => {
            let b = valore(x)?;
            let mut acc = Decimal::ONE;
            for _ in 0..*k {
                acc = acc.checked_mul(b)?;
            }
            Some(acc)
        }
        E::B(o, a, b) => {
            let (a, b) = (valore(a)?, valore(b)?);
            match o {
                '+' => a.checked_add(b),
                '-' => a.checked_sub(b),
                '*' => a.checked_mul(b),
                _ => {
                    if b.is_zero() {
                        None
                    } else {
                        a.checked_div(b)
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "collaudo lungo: cargo test -- --ignored"]
fn esplora_formule() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut r = Rng(0x1234_5678_9ABC_DEF1);
    let mut diversi = Vec::new();
    let mut panici = 0;
    let n = 50_000;
    for _ in 0..n {
        let e = genera(&mut r, 5);
        let s = stampa(&e, &mut r);
        if s.chars().count() > 1024 {
            continue;
        }
        let atteso = valore(&e);
        match catch_unwind(AssertUnwindSafe(|| valuta(&s))) {
            Err(_) => {
                panici += 1;
                if diversi.len() < 10 {
                    diversi.push(format!("PANICO {s}"));
                }
            }
            Ok(Ok(v)) => {
                match atteso {
                    Some(a) if a == v => {}
                    // Tolleranza: la divisione può arrotondare l'ultima cifra in modo diverso secondo l'ordine.
                    Some(a) if (a - v).abs() <= Decimal::new(1, 20) * (Decimal::ONE + a.abs()) => {}
                    _ => {
                        if diversi.len() < 10 {
                            diversi.push(format!("{s} = {v}, atteso {atteso:?}"));
                        }
                    }
                }
            }
            Ok(Err(err)) => {
                if atteso.is_some() && diversi.len() < 10 {
                    diversi.push(format!("{s}: errore «{err}», atteso {atteso:?}"));
                }
            }
        }
    }
    // Testi casuali: mai panico.
    let alfabeto: Vec<char> =
        "0123456789,.;+-*/^()× ÷ e piabsminmaxsqrtradqarrotonda par lun vv\u{0}😀"
            .chars()
            .collect();
    for _ in 0..50_000 {
        let l = 1 + r.sotto(40);
        let s: String = (0..l).map(|_| alfabeto[r.sotto(alfabeto.len())]).collect();
        if catch_unwind(AssertUnwindSafe(|| valuta(&s))).is_err() {
            panici += 1;
            if diversi.len() < 20 {
                diversi.push(format!("PANICO testo «{s}»"));
            }
        }
    }
    let _ = std::panic::take_hook();
    // Precedenze e casi noti.
    let casi = [
        ("2+3*4", "14"),
        ("2*3^2", "18"),
        ("-2^2", "-4"),
        ("2^3^2", "512"),
        ("10-4-3", "3"),
        ("100/10/5", "2"),
        ("2(3)", "err"),
        ("1,5+1.5", "3.0"),
        ("--2", "2"),
        ("+2", "2"),
        ("2^-1", "0.5"),
        ("0^0", "1"),
        ("min(3;1;2)", "1"),
        ("max(2,5; 3)", "3"),
        ("abs(-0,5)", "0.5"),
        ("sqrt(2)^2", "2"),
        ("arrotonda(2,345;2)", "2.35"),
        ("arrotonda(-2,345;2)", "-2.35"),
        ("1/3*3", "1"),
        ("pi", "3.14159"),
        ("2^64", "18446744073709551616"),
        ("2^65", "err"),
        ("1e3", "err"),
        (" ", "err"),
        ("()", "err"),
        ("3%", "err"),
    ];
    for (f, a) in casi {
        println!("  {f:>22} → {:?}   (atteso {a})", valuta(f));
    }
    println!("DIVERSI/PANICI: {} panici; {:#?}", panici, diversi);
    assert!(panici == 0 && diversi.is_empty());
}
