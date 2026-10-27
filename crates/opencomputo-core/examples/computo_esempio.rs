//! Esempio d'uso del core: elenco prezzi, computo con righi di misura,
//! riepilogo e serializzazione.
//!
//! ```sh
//! cargo run -p opencomputo-core --example computo_esempio
//! ```

use opencomputo_core::prelude::*;
use opencomputo_core::schema;
use rust_decimal_macros::dec;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = GeneratoreSequenziale::default();

    let mut progetto = Progetto::new(g.nuovo(), "Manutenzione marciapiedi", g.nuovo());
    let mut demolizione = VocePrezzario::new(
        g.nuovo(),
        "D.01",
        "Demolizione di pavimentazione in conglomerato",
        "m²",
        Prezzo::new(dec!(6.80)),
    );
    demolizione.incidenza_manodopera = Some(Percentuale::new(dec!(38)));
    let cordolo = VocePrezzario::new(
        g.nuovo(),
        "P.12",
        "Cordolo in calcestruzzo 12×25 cm, posa compresa",
        "m",
        Prezzo::new(dec!(24.15)),
    );
    let (id_demolizione, id_cordolo) = (demolizione.id, cordolo.id);
    progetto.elenco_prezzi.aggiungi_voce(demolizione)?;
    progetto.elenco_prezzi.aggiungi_voce(cordolo)?;

    let mut computo = Computo::new(g.nuovo(), "Computo metrico estimativo");
    let v =
        computo.aggiungi_da_prezzario(g.nuovo(), &progetto.elenco_prezzi, id_demolizione, None)?;
    v.righi = vec![
        RigoMisura::misura(g.nuovo(), "Via Garibaldi")
            .con_lunghezza(dec!(120.00))
            .con_larghezza(dec!(1.50)),
        RigoMisura::misura(g.nuovo(), "Piazza del Mercato, a dedurre aiuola")
            .con_parti(dec!(2))
            .con_lunghezza(dec!(3.20))
            .con_larghezza(dec!(1.10))
            .come_deduzione(),
    ];
    let v = computo.aggiungi_da_prezzario(g.nuovo(), &progetto.elenco_prezzi, id_cordolo, None)?;
    v.righi = vec![RigoMisura::misura(g.nuovo(), "Due lati").con_lunghezza("2*120")];
    let id_computo = computo.id;
    progetto.aggiungi_computo(computo)?;

    let riepilogo = progetto.calcola_computo(id_computo)?;
    println!(
        "{:>3}  {:<6} {:>4} {:>12} {:>10} {:>12}",
        "N.", "Codice", "U.M.", "Quantità", "Prezzo", "Importo"
    );
    for v in &riepilogo.voci {
        println!(
            "{:>3}  {:<6} {:>4} {:>12} {:>10} {:>12}",
            v.numero, v.codice, v.unita_misura, v.calcolo.quantita, v.prezzo, v.calcolo.importo
        );
    }
    println!("Totale lavori: {} €", riepilogo.totale);
    println!(
        "di cui manodopera (voci con incidenza nota): {} €",
        riepilogo.totale_manodopera
    );

    let rapporto = valida_progetto(&progetto);
    println!("Problemi di validazione: {}", rapporto.problemi.len());
    for p in &rapporto.problemi {
        println!("  [{:?}] {}: {}", p.gravita, p.codice, p.messaggio);
    }

    let json = schema::esporta_json(&progetto)?;
    let riletto = schema::importa_json(&json)?;
    assert_eq!(riletto, progetto);
    println!(
        "Serializzazione: {} byte, andata e ritorno verificata.",
        json.len()
    );
    Ok(())
}
