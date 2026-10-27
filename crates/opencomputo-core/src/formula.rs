//! Valutatore di formule aritmetiche in aritmetica decimale esatta.
//!
//! Le formule sono usate nei righi di misurazione (es. `2*(3,50+0,30)` o
//! `par*lun*lar`). Il valutatore è volutamente ristretto: non esegue codice,
//! non accede a risorse esterne e restituisce errori espliciti con la posizione
//! (carattere, a base 1) del problema.
//!
//! ## Sintassi
//!
//! * numeri con separatore decimale `,` oppure `.` (`2,5` = `2.5`); il
//!   separatore delle migliaia **non** è ammesso;
//! * operatori `+ - * / ^` (anche `×`, `÷`), parentesi tonde;
//! * `^` accetta solo esponenti interi (|esponente| ≤ 64) ed è associativo a destra;
//!   `-2^2` vale `-4`;
//! * argomenti delle funzioni separati da `;` (convenzione dei fogli di
//!   calcolo in italiano): `min(2,5; 3)`;
//! * funzioni: `abs(x)`, `min(x; …)`, `max(x; …)`, `sqrt(x)`/`radq(x)`,
//!   `arrotonda(x; n)` (commerciale, 0 ≤ n ≤ 10);
//! * costante `pi`;
//! * variabili fornite dal contesto (es. `par`, `lun`, `lar`, `alt` nei righi
//!   di misura). I nomi non distinguono maiuscole e minuscole.
//!
//! La precisione è quella di [`Decimal`] (28 cifre significative): `1/3*3`
//! vale `0,9999999999999999999999999999`, che arrotondato alle quantità
//! diventa `1,00`.

use std::collections::BTreeSet;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps, RoundingStrategy};

/// Lunghezza massima di una formula, in caratteri.
pub const LUNGHEZZA_MASSIMA: usize = 1024;
/// Profondità massima di annidamento.
pub const ANNIDAMENTO_MASSIMO: usize = 100;
const ESPONENTE_MASSIMO: i64 = 64;
const DECIMALI_MASSIMI_ARROTONDA: i64 = 10;

/// Errore di analisi o di valutazione di una formula.
/// Le posizioni sono indici di carattere a base 1.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErroreFormula {
    /// La formula è vuota.
    #[error("la formula è vuota")]
    Vuota,
    /// La formula supera [`LUNGHEZZA_MASSIMA`] caratteri.
    #[error("la formula supera {LUNGHEZZA_MASSIMA} caratteri")]
    TroppoLunga,
    /// Carattere non ammesso.
    #[error("carattere «{carattere}» non ammesso (posizione {posizione})")]
    CarattereInatteso {
        /// Carattere trovato.
        carattere: char,
        /// Posizione.
        posizione: usize,
    },
    /// Numero mal formato.
    #[error("numero «{testo}» non valido (posizione {posizione})")]
    NumeroNonValido {
        /// Testo del numero.
        testo: String,
        /// Posizione.
        posizione: usize,
    },
    /// Elemento inatteso.
    #[error("«{trovato}» inatteso (posizione {posizione})")]
    ElementoInatteso {
        /// Elemento trovato.
        trovato: String,
        /// Posizione.
        posizione: usize,
    },
    /// La formula termina prima del previsto.
    #[error("la formula è incompleta")]
    FineInattesa,
    /// Annidamento oltre [`ANNIDAMENTO_MASSIMO`].
    #[error("annidamento eccessivo (oltre {ANNIDAMENTO_MASSIMO} livelli)")]
    AnnidamentoEccessivo,
    /// Variabile non prevista dal contesto.
    #[error("variabile «{nome}» sconosciuta")]
    VariabileSconosciuta {
        /// Nome.
        nome: String,
    },
    /// Variabile prevista ma priva di valore.
    #[error("la variabile «{nome}» non ha un valore")]
    VariabileNonValorizzata {
        /// Nome.
        nome: String,
    },
    /// Funzione sconosciuta.
    #[error("funzione «{nome}» sconosciuta (posizione {posizione})")]
    FunzioneSconosciuta {
        /// Nome.
        nome: String,
        /// Posizione.
        posizione: usize,
    },
    /// Numero di argomenti errato.
    #[error("la funzione «{funzione}» richiede {attesi} argomenti, trovati {trovati}")]
    NumeroArgomenti {
        /// Funzione.
        funzione: String,
        /// Descrizione degli argomenti attesi.
        attesi: &'static str,
        /// Argomenti trovati.
        trovati: usize,
    },
    /// Argomento non ammesso.
    #[error("argomento non valido per «{funzione}»: {motivo}")]
    ArgomentoNonValido {
        /// Funzione.
        funzione: String,
        /// Motivo.
        motivo: String,
    },
    /// Divisione per zero.
    #[error("divisione per zero")]
    DivisionePerZero,
    /// Superamento della capacità numerica.
    #[error("superamento della capacità numerica")]
    Overflow,
    /// Esponente non intero o fuori intervallo.
    #[error("l'esponente deve essere un intero compreso tra -64 e 64")]
    EsponenteNonValido,
    /// Radice quadrata di un numero negativo.
    #[error("radice quadrata di un numero negativo")]
    RadiceNegativa,
}

/// Contesto che fornisce i valori delle variabili.
pub trait ContestoFormula {
    /// Valore della variabile `nome` (già in minuscolo).
    ///
    /// # Errors
    /// [`ErroreFormula::VariabileSconosciuta`] o [`ErroreFormula::VariabileNonValorizzata`].
    fn variabile(&self, nome: &str) -> Result<Decimal, ErroreFormula>;
}

/// Contesto privo di variabili.
#[derive(Debug, Clone, Copy, Default)]
pub struct SenzaVariabili;

impl ContestoFormula for SenzaVariabili {
    fn variabile(&self, nome: &str) -> Result<Decimal, ErroreFormula> {
        Err(ErroreFormula::VariabileSconosciuta {
            nome: nome.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Numero(Decimal),
    Identificatore(String),
    Piu,
    Meno,
    Per,
    Diviso,
    Potenza,
    Aperta,
    Chiusa,
    PuntoEVirgola,
}

impl Token {
    fn testo(&self) -> String {
        match self {
            Self::Numero(n) => n.to_string(),
            Self::Identificatore(s) => s.clone(),
            Self::Piu => "+".into(),
            Self::Meno => "-".into(),
            Self::Per => "*".into(),
            Self::Diviso => "/".into(),
            Self::Potenza => "^".into(),
            Self::Aperta => "(".into(),
            Self::Chiusa => ")".into(),
            Self::PuntoEVirgola => ";".into(),
        }
    }
}

#[derive(Debug, Clone)]
struct TokenPos {
    token: Token,
    posizione: usize,
}

fn tokenizza(sorgente: &str) -> Result<Vec<TokenPos>, ErroreFormula> {
    let caratteri: Vec<char> = sorgente.chars().collect();
    let mut token = Vec::new();
    let mut i = 0;
    while let Some(&c) = caratteri.get(i) {
        let posizione = i + 1;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' || c == ',' {
            let inizio = i;
            while caratteri
                .get(i)
                .is_some_and(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
            {
                i += 1;
            }
            let testo: String = caratteri
                .get(inizio..i)
                .unwrap_or_default()
                .iter()
                .collect();
            token.push(TokenPos {
                token: Token::Numero(interpreta_numero(&testo, posizione)?),
                posizione,
            });
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let inizio = i;
            while caratteri
                .get(i)
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                i += 1;
            }
            let testo: String = caratteri
                .get(inizio..i)
                .unwrap_or_default()
                .iter()
                .collect();
            token.push(TokenPos {
                token: Token::Identificatore(testo.to_lowercase()),
                posizione,
            });
            continue;
        }
        let t = match c {
            '+' => Token::Piu,
            '-' | '−' => Token::Meno,
            '*' | '×' => Token::Per,
            '/' | '÷' => Token::Diviso,
            '^' => Token::Potenza,
            '(' => Token::Aperta,
            ')' => Token::Chiusa,
            ';' => Token::PuntoEVirgola,
            altro => {
                return Err(ErroreFormula::CarattereInatteso {
                    carattere: altro,
                    posizione,
                });
            }
        };
        token.push(TokenPos {
            token: t,
            posizione,
        });
        i += 1;
    }
    Ok(token)
}

fn interpreta_numero(testo: &str, posizione: usize) -> Result<Decimal, ErroreFormula> {
    let errore = || ErroreFormula::NumeroNonValido {
        testo: testo.to_owned(),
        posizione,
    };
    let separatori = testo.chars().filter(|c| *c == '.' || *c == ',').count();
    let cifre = testo.chars().filter(char::is_ascii_digit).count();
    if separatori > 1 || cifre == 0 {
        return Err(errore());
    }
    let mut normalizzato = testo.replace(',', ".");
    if normalizzato.starts_with('.') {
        normalizzato.insert(0, '0');
    }
    if normalizzato.ends_with('.') {
        normalizzato.push('0');
    }
    Decimal::from_str_exact(&normalizzato).map_err(|_| errore())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operatore {
    Somma,
    Differenza,
    Prodotto,
    Quoziente,
    Potenza,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Nodo {
    Numero(Decimal),
    Variabile(String),
    Opposto(Box<Nodo>),
    Binario(Operatore, Box<Nodo>, Box<Nodo>),
    Funzione {
        nome: String,
        argomenti: Vec<Nodo>,
        posizione: usize,
    },
}

struct Parser<'a> {
    token: &'a [TokenPos],
    indice: usize,
    profondita: usize,
}

impl Parser<'_> {
    fn corrente(&self) -> Option<&Token> {
        self.token.get(self.indice).map(|t| &t.token)
    }

    fn avanza(&mut self) -> Option<&TokenPos> {
        let t = self.token.get(self.indice);
        self.indice += 1;
        t
    }

    fn inatteso(&self) -> ErroreFormula {
        match self.token.get(self.indice) {
            Some(t) => ErroreFormula::ElementoInatteso {
                trovato: t.token.testo(),
                posizione: t.posizione,
            },
            None => ErroreFormula::FineInattesa,
        }
    }

    fn espressione(&mut self) -> Result<Nodo, ErroreFormula> {
        let mut sinistro = self.termine()?;
        loop {
            let op = match self.corrente() {
                Some(Token::Piu) => Operatore::Somma,
                Some(Token::Meno) => Operatore::Differenza,
                _ => break,
            };
            self.avanza();
            let destro = self.termine()?;
            sinistro = Nodo::Binario(op, Box::new(sinistro), Box::new(destro));
        }
        Ok(sinistro)
    }

    fn termine(&mut self) -> Result<Nodo, ErroreFormula> {
        let mut sinistro = self.unario()?;
        loop {
            let op = match self.corrente() {
                Some(Token::Per) => Operatore::Prodotto,
                Some(Token::Diviso) => Operatore::Quoziente,
                _ => break,
            };
            self.avanza();
            let destro = self.unario()?;
            sinistro = Nodo::Binario(op, Box::new(sinistro), Box::new(destro));
        }
        Ok(sinistro)
    }

    /// Tutti i cicli ricorsivi della grammatica passano da qui: il controllo
    /// della profondità protegge dall'esaurimento dello stack.
    fn unario(&mut self) -> Result<Nodo, ErroreFormula> {
        self.profondita += 1;
        if self.profondita > ANNIDAMENTO_MASSIMO {
            return Err(ErroreFormula::AnnidamentoEccessivo);
        }
        let risultato = match self.corrente() {
            Some(Token::Meno) => {
                self.avanza();
                self.unario().map(|n| Nodo::Opposto(Box::new(n)))
            }
            Some(Token::Piu) => {
                self.avanza();
                self.unario()
            }
            _ => self.potenza(),
        };
        self.profondita -= 1;
        risultato
    }

    fn potenza(&mut self) -> Result<Nodo, ErroreFormula> {
        let base = self.primario()?;
        if self.corrente() == Some(&Token::Potenza) {
            self.avanza();
            let esponente = self.unario()?;
            return Ok(Nodo::Binario(
                Operatore::Potenza,
                Box::new(base),
                Box::new(esponente),
            ));
        }
        Ok(base)
    }

    fn primario(&mut self) -> Result<Nodo, ErroreFormula> {
        let Some(tp) = self.token.get(self.indice).cloned() else {
            return Err(ErroreFormula::FineInattesa);
        };
        match tp.token {
            Token::Numero(n) => {
                self.avanza();
                Ok(Nodo::Numero(n))
            }
            Token::Identificatore(nome) => {
                self.avanza();
                if self.corrente() == Some(&Token::Aperta) {
                    self.avanza();
                    let argomenti = self.argomenti()?;
                    Ok(Nodo::Funzione {
                        nome,
                        argomenti,
                        posizione: tp.posizione,
                    })
                } else {
                    Ok(Nodo::Variabile(nome))
                }
            }
            Token::Aperta => {
                self.avanza();
                let interno = self.espressione()?;
                self.chiusa()?;
                Ok(interno)
            }
            _ => Err(self.inatteso()),
        }
    }

    fn argomenti(&mut self) -> Result<Vec<Nodo>, ErroreFormula> {
        let mut argomenti = Vec::new();
        if self.corrente() == Some(&Token::Chiusa) {
            self.avanza();
            return Ok(argomenti);
        }
        loop {
            argomenti.push(self.espressione()?);
            match self.corrente() {
                Some(Token::PuntoEVirgola) => {
                    self.avanza();
                }
                _ => break,
            }
        }
        self.chiusa()?;
        Ok(argomenti)
    }

    fn chiusa(&mut self) -> Result<(), ErroreFormula> {
        if self.corrente() == Some(&Token::Chiusa) {
            self.avanza();
            Ok(())
        } else {
            Err(self.inatteso())
        }
    }
}

/// Formula analizzata, valutabile più volte con contesti diversi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formula {
    radice: Nodo,
}

impl Formula {
    /// Analizza il testo della formula.
    ///
    /// # Errors
    /// Errore di sintassi con la relativa posizione.
    pub fn analizza(sorgente: &str) -> Result<Self, ErroreFormula> {
        if sorgente.chars().count() > LUNGHEZZA_MASSIMA {
            return Err(ErroreFormula::TroppoLunga);
        }
        let token = tokenizza(sorgente)?;
        if token.is_empty() {
            return Err(ErroreFormula::Vuota);
        }
        let mut parser = Parser {
            token: &token,
            indice: 0,
            profondita: 0,
        };
        let radice = parser.espressione()?;
        if parser.indice < token.len() {
            return Err(parser.inatteso());
        }
        Ok(Self { radice })
    }

    /// Valuta la formula nel contesto dato.
    ///
    /// # Errors
    /// Errori di valutazione (divisione per zero, variabile mancante, overflow…).
    pub fn valuta(&self, contesto: &dyn ContestoFormula) -> Result<Decimal, ErroreFormula> {
        valuta_nodo(&self.radice, contesto)
    }

    /// Nomi delle variabili (in minuscolo) usate nella formula, esclusa `pi`.
    #[must_use]
    pub fn variabili(&self) -> BTreeSet<String> {
        let mut insieme = BTreeSet::new();
        raccogli_variabili(&self.radice, &mut insieme);
        insieme
    }
}

fn raccogli_variabili(nodo: &Nodo, insieme: &mut BTreeSet<String>) {
    match nodo {
        Nodo::Numero(_) => {}
        Nodo::Variabile(n) => {
            if n != "pi" {
                insieme.insert(n.clone());
            }
        }
        Nodo::Opposto(n) => raccogli_variabili(n, insieme),
        Nodo::Binario(_, a, b) => {
            raccogli_variabili(a, insieme);
            raccogli_variabili(b, insieme);
        }
        Nodo::Funzione { argomenti, .. } => {
            for a in argomenti {
                raccogli_variabili(a, insieme);
            }
        }
    }
}

/// Valuta una formula priva di variabili.
///
/// # Errors
/// Errori di sintassi o di valutazione.
pub fn valuta(sorgente: &str) -> Result<Decimal, ErroreFormula> {
    Formula::analizza(sorgente)?.valuta(&SenzaVariabili)
}

/// Valuta una formula con le variabili fornite dal contesto.
///
/// # Errors
/// Errori di sintassi o di valutazione.
pub fn valuta_con(
    sorgente: &str,
    contesto: &dyn ContestoFormula,
) -> Result<Decimal, ErroreFormula> {
    Formula::analizza(sorgente)?.valuta(contesto)
}

/// π con 28 cifre decimali.
fn pi_greco() -> Decimal {
    Decimal::from_i128_with_scale(31_415_926_535_897_932_384_626_433_833, 28)
}

fn valuta_nodo(nodo: &Nodo, contesto: &dyn ContestoFormula) -> Result<Decimal, ErroreFormula> {
    match nodo {
        Nodo::Numero(n) => Ok(*n),
        Nodo::Variabile(nome) if nome == "pi" => Ok(pi_greco()),
        Nodo::Variabile(nome) => contesto.variabile(nome),
        Nodo::Opposto(n) => Ok(-valuta_nodo(n, contesto)?),
        Nodo::Binario(op, a, b) => {
            let a = valuta_nodo(a, contesto)?;
            let b = valuta_nodo(b, contesto)?;
            match op {
                Operatore::Somma => a.checked_add(b).ok_or(ErroreFormula::Overflow),
                Operatore::Differenza => a.checked_sub(b).ok_or(ErroreFormula::Overflow),
                Operatore::Prodotto => a.checked_mul(b).ok_or(ErroreFormula::Overflow),
                Operatore::Quoziente => {
                    if b.is_zero() {
                        Err(ErroreFormula::DivisionePerZero)
                    } else {
                        a.checked_div(b).ok_or(ErroreFormula::Overflow)
                    }
                }
                Operatore::Potenza => potenza(a, b),
            }
        }
        Nodo::Funzione {
            nome,
            argomenti,
            posizione,
        } => {
            let valori = argomenti
                .iter()
                .map(|a| valuta_nodo(a, contesto))
                .collect::<Result<Vec<_>, _>>()?;
            funzione(nome, &valori, *posizione)
        }
    }
}

fn potenza(base: Decimal, esponente: Decimal) -> Result<Decimal, ErroreFormula> {
    if !esponente.fract().is_zero() {
        return Err(ErroreFormula::EsponenteNonValido);
    }
    let e = esponente
        .to_i64()
        .filter(|e| e.abs() <= ESPONENTE_MASSIMO)
        .ok_or(ErroreFormula::EsponenteNonValido)?;
    if base.is_zero() && e < 0 {
        return Err(ErroreFormula::DivisionePerZero);
    }
    base.checked_powi(e).ok_or(ErroreFormula::Overflow)
}

fn funzione(nome: &str, argomenti: &[Decimal], posizione: usize) -> Result<Decimal, ErroreFormula> {
    let numero_errato = |attesi: &'static str| ErroreFormula::NumeroArgomenti {
        funzione: nome.to_owned(),
        attesi,
        trovati: argomenti.len(),
    };
    match nome {
        "abs" => match argomenti {
            [x] => Ok(x.abs()),
            _ => Err(numero_errato("1")),
        },
        "min" => argomenti
            .iter()
            .copied()
            .min()
            .ok_or_else(|| numero_errato("almeno 1")),
        "max" => argomenti
            .iter()
            .copied()
            .max()
            .ok_or_else(|| numero_errato("almeno 1")),
        "sqrt" | "radq" => match argomenti {
            [x] if x.is_sign_negative() && !x.is_zero() => Err(ErroreFormula::RadiceNegativa),
            [x] => x.sqrt().ok_or(ErroreFormula::Overflow),
            _ => Err(numero_errato("1")),
        },
        "arrotonda" => match argomenti {
            [x, n] => {
                let decimali = (n.fract().is_zero())
                    .then(|| n.to_i64())
                    .flatten()
                    .filter(|d| (0..=DECIMALI_MASSIMI_ARROTONDA).contains(d))
                    .and_then(|d| u32::try_from(d).ok())
                    .ok_or_else(|| ErroreFormula::ArgomentoNonValido {
                        funzione: nome.to_owned(),
                        motivo: format!(
                            "i decimali devono essere un intero tra 0 e {DECIMALI_MASSIMI_ARROTONDA}"
                        ),
                    })?;
                Ok(x.round_dp_with_strategy(decimali, RoundingStrategy::MidpointAwayFromZero))
            }
            _ => Err(numero_errato("2")),
        },
        _ => Err(ErroreFormula::FunzioneSconosciuta {
            nome: nome.to_owned(),
            posizione,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn aritmetica_di_base() {
        assert_eq!(valuta("2,5*4").unwrap(), dec!(10));
        assert_eq!(valuta("2.5 * 4").unwrap(), dec!(10));
        assert_eq!(valuta("(1+2)*3").unwrap(), dec!(9));
        assert_eq!(valuta("1+2*3").unwrap(), dec!(7));
        assert_eq!(valuta("10/4").unwrap(), dec!(2.5));
        assert_eq!(valuta("10-2-3").unwrap(), dec!(5));
        assert_eq!(valuta("2 × 3 ÷ 4").unwrap(), dec!(1.5));
        assert_eq!(valuta(",5+1.").unwrap(), dec!(1.5));
    }

    #[test]
    fn precisione_decimale_esatta() {
        // In virgola mobile 0.1 + 0.2 = 0.30000000000000004.
        assert_eq!(valuta("0,1+0,2").unwrap(), dec!(0.3));
        assert_eq!(valuta("1/3*3").unwrap().round_dp(2), dec!(1.00));
    }

    #[test]
    fn segni_e_potenze() {
        assert_eq!(valuta("-2^2").unwrap(), dec!(-4));
        assert_eq!(valuta("(-2)^2").unwrap(), dec!(4));
        assert_eq!(valuta("2^-1").unwrap(), dec!(0.5));
        assert_eq!(valuta("2^3^2").unwrap(), dec!(512));
        assert_eq!(valuta("--3").unwrap(), dec!(3));
        assert_eq!(valuta("+3").unwrap(), dec!(3));
    }

    #[test]
    fn funzioni_e_costanti() {
        assert_eq!(valuta("min(3;1;2)").unwrap(), dec!(1));
        assert_eq!(valuta("max(3;1;2)").unwrap(), dec!(3));
        assert_eq!(valuta("min(2,5)").unwrap(), dec!(2.5));
        assert_eq!(valuta("abs(-7,25)").unwrap(), dec!(7.25));
        assert_eq!(valuta("arrotonda(2,345;2)").unwrap(), dec!(2.35));
        assert_eq!(valuta("ARROTONDA(-2,345;2)").unwrap(), dec!(-2.35));
        assert_eq!(valuta("sqrt(9)").unwrap(), dec!(3));
        assert_eq!(valuta("radq(2,25)").unwrap(), dec!(1.5));
        assert_eq!(valuta("arrotonda(pi*0,5^2;4)").unwrap(), dec!(0.7854));
    }

    #[test]
    fn errori_di_sintassi() {
        assert_eq!(valuta(""), Err(ErroreFormula::Vuota));
        assert_eq!(valuta("   "), Err(ErroreFormula::Vuota));
        assert_eq!(valuta("2*"), Err(ErroreFormula::FineInattesa));
        assert_eq!(valuta("(1+2"), Err(ErroreFormula::FineInattesa));
        assert_eq!(
            valuta("1+2)"),
            Err(ErroreFormula::ElementoInatteso {
                trovato: ")".into(),
                posizione: 4
            })
        );
        assert_eq!(
            valuta("1.234,5"),
            Err(ErroreFormula::NumeroNonValido {
                testo: "1.234,5".into(),
                posizione: 1
            })
        );
        assert_eq!(
            valuta("2 # 3"),
            Err(ErroreFormula::CarattereInatteso {
                carattere: '#',
                posizione: 3
            })
        );
        assert_eq!(
            valuta("2 3"),
            Err(ErroreFormula::ElementoInatteso {
                trovato: "3".into(),
                posizione: 3
            })
        );
    }

    #[test]
    fn errori_di_valutazione() {
        assert_eq!(valuta("1/0"), Err(ErroreFormula::DivisionePerZero));
        assert_eq!(valuta("1/(2-2)"), Err(ErroreFormula::DivisionePerZero));
        assert_eq!(valuta("0^-1"), Err(ErroreFormula::DivisionePerZero));
        assert_eq!(valuta("2^0,5"), Err(ErroreFormula::EsponenteNonValido));
        assert_eq!(valuta("2^100"), Err(ErroreFormula::EsponenteNonValido));
        assert_eq!(valuta("sqrt(-1)"), Err(ErroreFormula::RadiceNegativa));
        assert_eq!(
            valuta("abc"),
            Err(ErroreFormula::VariabileSconosciuta { nome: "abc".into() })
        );
        assert!(matches!(
            valuta("foo(1)"),
            Err(ErroreFormula::FunzioneSconosciuta { .. })
        ));
        assert!(matches!(
            valuta("max()"),
            Err(ErroreFormula::NumeroArgomenti { .. })
        ));
        assert!(matches!(
            valuta("abs(1;2)"),
            Err(ErroreFormula::NumeroArgomenti { .. })
        ));
        assert!(matches!(
            valuta("arrotonda(1;1,5)"),
            Err(ErroreFormula::ArgomentoNonValido { .. })
        ));
        assert_eq!(
            valuta("79228162514264337593543950335*10"),
            Err(ErroreFormula::Overflow)
        );
    }

    #[test]
    fn limiti_di_sicurezza() {
        let annidata = format!("{}1{}", "(".repeat(500), ")".repeat(500));
        assert_eq!(valuta(&annidata), Err(ErroreFormula::AnnidamentoEccessivo));
        let segni = format!("{}1", "-".repeat(500));
        assert_eq!(valuta(&segni), Err(ErroreFormula::AnnidamentoEccessivo));
        let lunga = "1+".repeat(600) + "1";
        assert_eq!(valuta(&lunga), Err(ErroreFormula::TroppoLunga));
        let ok = format!("{}1{}", "(".repeat(50), ")".repeat(50));
        assert_eq!(valuta(&ok).unwrap(), dec!(1));
    }

    struct Ctx;
    impl ContestoFormula for Ctx {
        fn variabile(&self, nome: &str) -> Result<Decimal, ErroreFormula> {
            match nome {
                "lun" => Ok(dec!(3.5)),
                "lar" => Err(ErroreFormula::VariabileNonValorizzata { nome: nome.into() }),
                _ => Err(ErroreFormula::VariabileSconosciuta { nome: nome.into() }),
            }
        }
    }

    #[test]
    fn variabili_dal_contesto() {
        assert_eq!(valuta_con("2*LUN", &Ctx).unwrap(), dec!(7));
        assert_eq!(
            valuta_con("lun*lar", &Ctx),
            Err(ErroreFormula::VariabileNonValorizzata { nome: "lar".into() })
        );
        let f = Formula::analizza("par*lun + pi*max(lar;1)").unwrap();
        let attese: BTreeSet<String> = ["lar", "lun", "par"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(f.variabili(), attese);
    }
}
