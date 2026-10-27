# ADR 0001 — Workspace modulare e core puro

* Stato: accettato (Fase 1)
* Data: 2026-10-20

## Contesto

OpenComputo deve offrire GUI, CLI, API per agenti AI, import/export di più formati e
reportistica, con calcoli contabili identici in tutti i casi. Il prompt di progetto
propone otto crate.

## Decisione

* Workspace Cargo con un crate per confine logico, come da proposta iniziale.
* Il crate `opencomputo-core` contiene **solo** dominio, calcoli puri e validazioni: nessun
  I/O, orologio, entropia, rete o GUI. Le operazioni sono deterministiche.
* I crate vengono creati quando ricevono un'implementazione reale: nella Fase 1 esiste
  solo `opencomputo-core`. Gli altri sono descritti in `docs/ARCHITETTURA.md`.
* Lo schema di serializzazione (involucro JSON e migrazioni) vive nel core perché
  dipende solo dai tipi del dominio ed è puro; file, salvataggi atomici e copie di
  sicurezza spetteranno a `opencomputo-storage`.
* Identificativi forniti dal chiamante tramite `GeneratoreId`; UUID v4 dietro la
  feature opzionale `id-casuali`.

## Differenze dalla proposta iniziale

1. Crate futuri non creati vuoti (principio «nessuna implementazione fittizia»).
2. Modulo `schema` nel core anziché in `opencomputo-storage` (motivazione sopra).

## Conseguenze

* I calcoli sono testabili senza GUI e riutilizzabili in WebAssembly.
* Le API pubbliche del core saranno consumate da `opencomputo-application`, che
  aggiungerà transazioni, annulla/ripristina e dry-run.
