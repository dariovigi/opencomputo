# ADR 0002 — Aritmetica decimale e arrotondamenti

* Stato: accettato (Fase 1)
* Data: 2026-10-20

## Contesto

I valori contabili devono essere riproducibili al centesimo. `f64` non rappresenta
esattamente valori come 0,10 e accumula errori nelle somme.

## Opzioni valutate

| Opzione | Pro | Contro |
|---------|-----|--------|
| `f64` | veloce, nativo | inesatto: escluso |
| interi in centesimi (`i64`) | esatto, semplice | scala fissa: inadatto a quantità con 3+ decimali, prezzi di listino a 3-4 decimali e calcoli intermedi |
| `rust_decimal::Decimal` | decimale esatto a 28 cifre, scala variabile, arrotondamenti configurabili, serde come stringa, compatibile WASM | 96 bit: limiti di capacità (gestiti come errore) |
| `bigdecimal` | precisione arbitraria | allocazioni, API meno adatta, prestazioni |

## Decisione

* `rust_decimal::Decimal` per tutte le grandezze, incapsulato in tipi distinti
  (`Quantita`, `Prezzo`, `Importo`, `Percentuale`) per evitare confusioni tra grandezze.
* Solo operazioni controllate (`checked_*`) con errore `Overflow` esplicito.
* Lint `clippy::float_arithmetic = "deny"` sull'intero workspace.
* Arrotondamenti in punti definiti (rigo, quantità di voce, importo di voce, quote
  percentuali, prezzo da analisi) con regole configurabili per progetto; default:
  2 decimali, metà lontano da zero; totali come somme esatte di valori arrotondati.
* Gli ingressi non vengono arrotondati.

## Conseguenze e limiti

* Le divisioni non esatte (es. `1/3`) sono troncate a 28 cifre prima dell'arrotondamento
  finale: l'effetto è irrilevante a 2-4 decimali ma è documentato.
* Le regole predefinite sono un'assunzione ragionevole: **vanno confermate** con
  professionisti e con i documenti prodotti da PriMus (Fase 2), poiché software diversi
  possono arrotondare in punti diversi (per rigo o solo sul totale).
