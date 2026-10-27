# ADR 0005 — Licenza

* Stato: **accettato** — approvato dal titolare del progetto il 2026-10-20
* Data: 2026-10-20

## Contesto

Il progetto si rivolge a professionisti e pubbliche amministrazioni italiane. Le Linee
guida AgID sul riuso del software della PA privilegiano licenze approvate da OSI.
Tutte le dipendenze attuali sono MIT o MIT/Apache-2.0, compatibili con ciascuna delle
opzioni seguenti.

## Opzioni

| Licenza | Effetto | Adatta se… |
|---------|---------|-----------|
| **EUPL-1.2** | copyleft «debole sulla rete»: chi distribuisce o offre come servizio versioni modificate deve pubblicarne il sorgente; testo ufficiale in italiano; compatibile con GPL/AGPL/MPL | si vuole che le migliorie restino aperte e il progetto sia riusabile dalla PA |
| AGPL-3.0-or-later | copyleft forte anche per uso via rete | si vuole impedire versioni SaaS chiuse |
| GPL-3.0-or-later | copyleft forte sulla distribuzione | applicazione desktop, uso SaaS non rilevante |
| MIT OR Apache-2.0 | permissiva (convenzione Rust) | si privilegia la massima adozione, anche in prodotti chiusi |

## Proposta

**EUPL-1.2** per l'applicazione. Se in futuro si vorrà favorire il riuso del solo
modello di dominio in altri prodotti, si potrà valutare `MIT OR Apache-2.0` per i crate
di formato e per `opencomputo-core`.

## Decisione

Approvata **EUPL-1.2** il 2026-10-20. Il testo ufficiale (versione inglese, fonte: SPDX
License List) è nel file `LICENSE`; tutte le versioni linguistiche della EUPL, inclusa
quella italiana, hanno pari valore. I manifest dichiarano `license = "EUPL-1.2"`.
`publish = false` resta finché non si decide di pubblicare i crate su crates.io.

Questo testo non costituisce consulenza legale.
