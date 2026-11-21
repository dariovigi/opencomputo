# Architettura

## Workspace

Il progetto è un workspace Cargo. Esistono `opencomputo-core` (Fase 1),
`opencomputo-format-xpwe` e `opencomputo-dcf-parser` (Fase 2), `opencomputo-io`
(Fase 3), `opencomputo-storage`, `opencomputo-application` e `opencomputo-agent-api`
(Fase 4), `opencomputo-gui` (Fase 5), `opencomputo-format-listino` e
`opencomputo-assistente` (assistente AI, fase 2 dell'ADR 0025), `opencomputo-rete`
(fase 3). Nessun crate è stato
creato vuoto: ognuno è stato aggiunto nella fase in cui ha ricevuto un'implementazione
reale.

| Crate | Responsabilità | Fase | Stato |
|-------|----------------|------|-------|
| `opencomputo-core` | modello di dominio, calcoli puri, validazioni, schema versionato | 1 | **presente** |
| `opencomputo-format-xpwe` | import/export XPWE, test di andata e ritorno | 2 | **presente** (export sperimentale) |
| `opencomputo-dcf-parser` | lettura dei DCF non protetti (elenco prezzi, capitoli, categorie) ed export dell'elenco prezzi | 2 | **presente** ([ADR 0006](adr/0006-formati-primus.md); export sperimentale, [ADR 0016](adr/0016-export-dcf.md)) |
| `opencomputo-io` | report in PDF, XLSX e CSV | 3 | **presente** |
| `opencomputo-application` | casi d'uso condivisi da GUI, CLI e API (comandi, annulla/ripristina, prova) | 4 | **presente** |
| `opencomputo-storage` | file di progetto, salvataggi atomici, copie di sicurezza | 4 | **presente** |
| `opencomputo-agent-api` | binario `opencomputo`: CLI e servizio JSON per agenti AI | 4 | **presente** |
| `opencomputo-gui` | applicazione desktop multifinestra (Tauri v2) | 5 | **presente** ([ADR 0009](adr/0009-gui-tauri.md)) |
| `opencomputo-format-listino` | lettura di listini prezzi CSV e JSON | assistente 2 | **presente** ([ADR 0025](adr/0025-assistente-ai.md)) |
| `opencomputo-assistente` | assistente AI: ricerca testuale e vettoriale, client dei provider (senza rete propria), consenso, validazione delle risposte, traduzione in comandi | assistente 2–3 | **presente** ([ADR 0025](adr/0025-assistente-ai.md)) |
| `opencomputo-rete` | trasporto HTTPS e portachiavi del sistema per l'assistente | assistente 3 | **presente** ([ADR 0025](adr/0025-assistente-ai.md)) |

## Dipendenze consentite tra i crate

```
gui ──► agent-api (servizio JSON) ──► application ──► core
                                          │
                                          ├──► storage ──► core
                                          ├──► format-xpwe ──► core
                                          ├──► dcf-parser ──► core
                                          ├──► format-listino ──► core
                                          └──► io ──► core

agent-api ──► rete ──► assistente ──► application ──► core
         └──► assistente
```

L'assistente costruisce comandi di `application` e li prova nella `Sessione`: non scrive
il modello e non calcola quantità né importi (ADR 0025). I client dei provider stanno nel
modulo `provider`, separato dalla validazione, e spediscono le richieste attraverso il
tratto `Trasporto`: l'unica implementazione con la rete è in `opencomputo-rete`, insieme
al portachiavi del sistema. Il servizio JSON (`agent-api`, modulo `assistente`) collega i
pezzi: configurazione, consenso, cache dei vettori. Le operazioni lunghe sono `Lavoro`:
preparate con il servizio, eseguite senza (la GUI le esegue con `spawn_blocking`, così le
altre finestre restano libere), concluse con il servizio.

La GUI usa lo stesso servizio JSON della CLI e degli agenti (ADR 0009): non esiste una
via di modifica riservata all'interfaccia grafica.

* `core` non dipende da nessun altro crate del workspace, né da filesystem, rete,
  orologio di sistema, sorgenti di entropia o GUI.
* I formati dipendono solo da `core`: producono e consumano tipi del dominio.
* GUI e API non chiamano direttamente i formati o lo storage: passano dai casi
  d'uso di `application`, così che gli agenti AI non possano aggirare le
  validazioni applicate all'interfaccia grafica.

## Organizzazione di `opencomputo-core`

| Modulo | Contenuto |
|--------|-----------|
| `numeri` | `Quantita`, `Prezzo`, `Importo`, `Percentuale` (newtype su `Decimal`); `RegoleArrotondamento` |
| `id` | identificativi tipizzati (UUID), `GeneratoreId` deterministico |
| `formula` | valutatore di espressioni in aritmetica decimale, con limiti di sicurezza |
| `misura` | righi di misurazione (parti uguali, lunghezza, larghezza, altezza/peso, formula) |
| `prezzario` | prezzari, capitoli, voci, natura (lavori / oneri della sicurezza) |
| `analisi` | risorse elementari e analisi prezzi |
| `classificazione` | super categorie, categorie, sotto categorie e WBS |
| `partita` | `PrezzoApplicato` (fotografia del prezzo) e calcolo prezzo × misure |
| `computo` | computi, voci di computo, riepiloghi e totali raggruppati |
| `contabilita` | libretto delle misure, registro di contabilità, SAL |
| `progetto` | radice del modello: metadati, contratto, regole, elenco prezzi, computi, contabilità |
| `provenienza` | formato e file di origine, anomalie, dati originali non mappati |
| `validazione` | `valida_progetto` → elenco di problemi con gravità e codice stabile |
| `schema` | involucro JSON versionato e migrazioni |
| `riferimento`, `errore` | percorsi delle entità ed errori tipizzati |

## Organizzazione di `opencomputo-format-xpwe`

| Modulo | Contenuto |
|--------|-----------|
| `xml` | albero XML minimo con righe di origine, decodifica (BOM, dichiarazione, ripiego Windows-1252), limiti di sicurezza, scrittore con rientri |
| `struttura` | nomi di elementi e flag di XPWE **dedotti** da LeenO, con il grado di certezza |
| `testo` | numeri con punto o virgola, fattori di misura, moltiplicazione implicita, decimali di configurazione |
| `conservati` | conservazione dei dati non mappati (ordine dei campi, frammenti XML, testi originali) |
| `importa` | XPWE → `Progetto` con rapporto di anomalie e verifica delle quantità dichiarate |
| `esporta` | computo → XPWE (sperimentale), ripercorre i dati conservati |

Il crate dipende solo da `opencomputo-core` e non usa filesystem né rete: riceve byte e
restituisce testo. Il parser XML non risolve entità esterne né DTD.

## Organizzazione di `opencomputo-dcf-parser`

| Modulo | Contenuto |
|--------|-----------|
| `contenitore` | settori, flussi `AACS`, pezzi zlib; riconoscimento dei flussi protetti; limiti contro i file «bomba» |
| `xml` | lettura in streaming degli elementi (flussi fino a decine di MB) |
| `importa` | DCF → `Progetto` con elenco prezzi, capitoli, categorie, dati generali e rapporto di anomalie |
| `struttura` | indice delle voci, elenchi dei settori, somme di controllo, verifica di coerenza |
| `scrittura` | scrittura del contenitore (indice, mappa dei settori, flussi compressi con CRC) |
| `esporta` | elenco prezzi → DCF in chiaro, nella forma dei prezzari |

Il crate non decifra i flussi protetti, rispetta la password di apertura e non scrive
protezioni né codici di autenticazione.

## Organizzazione di `opencomputo-format-listino`

| Modulo | Contenuto |
|--------|-----------|
| `csv` | lettore CSV (RFC 4180) con riconoscimento del separatore di campo |
| `numeri` | numeri all'italiana o con il punto; riconoscimento del separatore decimale per file |
| `colonne` | nomi alternativi delle colonne, configurabili, e colonne obbligatorie |
| `importa` | CSV/JSON → `Prezzario` con capitoli, rapporto di righe importate e scartate, impronta SHA-256 |

Dipende solo da `opencomputo-core`; nessuna dipendenza esterna nuova.

## Organizzazione di `opencomputo-assistente`

| Modulo | Contenuto |
|--------|-----------|
| `testo` | normalizzazione dell'italiano, parole vuote, radici leggere, trigrammi |
| `unita` | dimensione e fattore delle unità di misura dei prezzari |
| `ricerca` | indice testuale BM25 in interi a virgola fissa, termini simili per trigrammi, filtri |
| `candidati` | candidati con handle per una sola richiesta; vista per il modello senza codici né prezzi |
| `valori` | valori numerici della richiesta («valori fondati») e confronto nell'unità base |
| `forme` | catalogo chiuso delle forme di rigo e traduzione in righi di misura del core |
| `risposta`, `schema` | risposte del modello e schemi JSON per il tool use con `strict: true` |
| `proposta` | validazione della risposta, proposta tracciata, comandi |
| `verifica` | prova dei comandi nella `Sessione` e anteprima delle voci su una copia del progetto |
| `provider` | tratti `Trasporto`, `Attesa`, `Embedding`, `Generazione`; `Segreto`; errori tipizzati |
| `provider::openai`, `provider::anthropic` | client degli embedding e della generazione con output strutturato |
| `provider::riprova` | classificazione degli errori HTTP e nuovi tentativi con `retry-after` |
| `provider::chiavi` | tratto `ArchivioChiavi` e archivio in memoria per i test |
| `vettori` | spazio di embedding, indice vettoriale con cache `OCVEC`, indicizzazione a lotti; coseno nell'unico modulo con aritmetica `f32` |
| `prompt` | istruzioni e messaggi delle due chiamate al modello |
| `consenso` | testo versionato del consenso all'invio dei dati |
| `flusso` | dalla richiesta alla proposta: analisi, ricerca ibrida, selezione, validazione |

## Organizzazione di `opencomputo-io`

| Modulo | Contenuto |
|--------|-----------|
| `documento` | modello intermedio: sezioni, tabelle, righe tipizzate, valori |
| `report` | costruzione dei report dal `Progetto` (computo, elenco prezzi, libretto, registro, SAL) |
| `formato` | numeri e date all'italiana, senza arrotondamenti |
| `report_analisi` | report delle analisi prezzi, del fabbisogno, dei nuovi prezzi e del computo con analisi |
| `pdf` | impaginazione e scrittura PDF (font standard, WinAnsi, coordinate intere) |
| `tradizionale` | stampa PDF di computo ed elenco prezzi nel modello tradizionale, con riporti di pagina e frontespizio (ADR 0015) |
| `xlsx` | scrittura XLSX con `rust_xlsxwriter` |
| `csv` | scrittura CSV |

Dipende solo da `opencomputo-core`. Gli altri crate di formato sono dipendenze di sviluppo,
usate solo nei test.

## Organizzazione di `opencomputo-storage`

Un solo modulo: `codifica`/`decodifica` (byte ↔ `Progetto`, senza filesystem), `salva`
(atomico, con verifica e copie di sicurezza) e `apri`. Dipende da `core` e `flate2`.

## Organizzazione di `opencomputo-application`

| Modulo | Contenuto |
|--------|-----------|
| `comandi` | `Comando` (JSON), riferimenti per codice, numero e nome, `applica` sulle API del core |
| `sessione` | `Sessione`: richieste atomiche, politica di validazione, `Esito`, prova, annulla/ripristina, prezzari di consultazione, generatore di identificativi |
| `casi_uso` | import XPWE/DCF, lettura di prezzari, export XPWE, report |
| `convertitore` | convertitore DCF/OCP esterno scelto dall'utente (il ponte `dcf_bridge.py` in Rust): import, export, verifica `confronta`, `info`; ambiente ridotto, cartella temporanea, durata massima, scrittura atomica (ADR 0027) |
| `errore` | `ErroreApplicazione`, serializzabile con un codice stabile |

Dipende da `core`, `storage`, i formati e `io`. Non contiene regole di dominio proprie.

## Organizzazione di `opencomputo-agent-api`

| Modulo | Contenuto |
|--------|-----------|
| `servizio` | `Richiesta`, `Servizio` con stato, risposte JSON, ciclo a righe |
| `riepilogo` | `info`, `valida`, `cerca` in JSON |
| `convertitore` | configurazione del convertitore DCF/OCP esterno (`convertitore.json`, poi `DCF_BIN`) e avviso (ADR 0027) |
| `schema` | regole ed esempi dei comandi (verificati dai test) |
| `main.rs` | binario `opencomputo` (`clap`): ogni sottocomando è una sequenza di richieste del servizio |

## Organizzazione di `opencomputo-gui`

Il lato Rust (`src/lib.rs`) tiene un `Servizio` di `opencomputo-agent-api` ed espone il
comando Tauri `richiesta` (la stessa richiesta JSON di `opencomputo servizio`), più
`apri_finestra`, `apri_file` ed `esci`; emette `progetto-aggiornato` dopo ogni modifica e
`chiusura-richiesta` quando si chiude con modifiche non salvate. L'interfaccia è nella
cartella `ui`, JavaScript senza build:

| File | Contenuto |
|------|-----------|
| `ui/js/piattaforma.js` | unico accesso alle API di Tauri (richieste, dialoghi, eventi, finestre) |
| `ui/js/stato.js` | stato dell'interfaccia, `esegui()` dei comandi, errori, aggiornamento |
| `ui/js/dom.js` | `h()` (solo testo, mai HTML), numeri all'italiana, modali, notifiche |
| `ui/js/griglia.js` | griglia virtuale di sola lettura (elenchi prezzi grandi) |
| `ui/js/misure.js` | foglio delle misure modificabile da tastiera (computo e libretto) |
| `ui/js/selettore.js` | scelta di una voce dall'elenco o da un prezzario caricato |
| `ui/js/app.js` | guscio, navigazione, operazioni sui file, scorciatoie, eventi |
| `ui/js/viste/*.js` | dati generali, elenco prezzi, computo, categorie e WBS, libretto, SAL, report, controlli |

I test dell'interfaccia (`ui-tests`, Playwright con Chromium) inoltrano le richieste della
pagina a un processo `opencomputo servizio` reale.

## WebAssembly

Il core è progettato per `wasm32-unknown-unknown`: nessuna dipendenza da I/O,
nessuna generazione casuale di default (la feature `id-casuali` abilita UUID v4 e
richiederà `getrandom` con supporto JS), `jiff` solo per le date civili (senza fusi orari né
lettura dell'orologio; non ancora riprovato per wasm dopo il passaggio da `chrono`).
**Verificato** il 2026-10-20: con le feature di default il core compila (debug e
release) e passa clippy per `wasm32-unknown-unknown`. Con `id-casuali` la compilazione
fallisce finché non si sceglie la sorgente di casualità di `uuid` (es. feature `js`):
decisione ancora aperta. Dalla Fase 4 compilano per `wasm32-unknown-unknown` anche i
formati, `io`, `storage` e `application` (in WebAssembly `salva` e `apri` restituiscono
errore; `codifica` e `decodifica` funzionano). Il binario `opencomputo-agent-api` è solo
nativo. I test non sono stati eseguiti in un runtime WASM (vedi [STATO.md](STATO.md)).

## Decisioni

Le decisioni sono registrate in [`adr/`](adr/):

* [0001 — Workspace modulare e core puro](adr/0001-workspace-modulare.md)
* [0002 — Aritmetica decimale e arrotondamenti](adr/0002-aritmetica-decimale.md)
* [0003 — Prezzi fotografati nelle voci](adr/0003-prezzi-fotografati.md)
* [0004 — Formato di persistenza interno](adr/0004-formato-persistenza.md)
* [0005 — Licenza: EUPL-1.2](adr/0005-licenza.md)
* [0006 — Formati PriMus: XPWE e DCF non protetti](adr/0006-formati-primus.md)
* [0007 — Report ed export in PDF, XLSX e CSV](adr/0007-report-ed-export.md)
* [0008 — Casi d'uso, file di progetto e interfaccia per agenti](adr/0008-casi-uso-storage-cli.md)

* [0009 — Interfaccia grafica con Tauri v2](adr/0009-gui-tauri.md)
* [0010 — Rilascio, pacchetti e controllo delle dipendenze](adr/0010-rilascio-e-dipendenze.md)
* [0011 — Anticipazione, penali, certificato di pagamento e sommario del registro](adr/0011-contabilita-completa.md)
* [0012 — Stato finale, conto finale, collaudo e rata di saldo](adr/0012-conto-finale.md)
* [0013 — Analisi prezzi dinamiche, fabbisogno di cantiere e nuovi prezzi](adr/0013-analisi-e-nuovi-prezzi.md)
* [0014 — Variabili nelle analisi, oneri aziendali della sicurezza, aggiornamento dei prezzi, computo con analisi](adr/0014-variabili-e-aggiornamento-prezzi.md)
* Le decisioni dalla 0015 alla 0024 sono nella cartella [`adr/`](adr/).
* [0025 — Assistente AI per il computo e listini CSV/JSON](adr/0025-assistente-ai.md)
* [0026 — Prestazioni con progetti grandi e limiti dei file](adr/0026-prestazioni-e-limiti.md)
* [0027 — Convertitore DCF/OCP esterno](adr/0027-convertitore-dcf-esterno.md)
