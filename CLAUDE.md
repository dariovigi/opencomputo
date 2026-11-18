# Istruzioni per le sessioni di sviluppo

* Progetto: OpenComputo, workspace Rust. Leggere `docs/STATO.md` prima di iniziare.
* Fase corrente: **Fase 6 completata** (2026-10-21), poi **contabilità completa**
  (anticipazione, penali, certificato di pagamento con IVA, sommario; ADR 0011) e
  **conto finale** (stato finale, collaudo, rata di saldo; ADR 0012) e **analisi prezzi e
  nuovi prezzi** (analisi dinamiche e annidate, fabbisogno, verbali; ADR 0013), poi
  variabili, oneri della sicurezza, aggiornamento prezzi, computo con analisi (ADR 0014,
  schema 5), stampa tradizionale di computo ed elenco prezzi (ADR 0015), export DCF
  sperimentale dell'elenco prezzi (ADR 0016, PriMus non lo apre), trascinamento di voci
  tra finestre e `copia_voci_computo` (ADR 0017), voci affiancate al computo
  con «>» e «<» (ADR 0018), «vedi voce» collegato nel computo e statico nel libretto
  (ADR 0019, schema 6), pannello dei filtri di computo ed elenco (ADR 0020),
  incidenze di manodopera e sicurezza nella stampa del computo (ADR 0021),
  aggiornamento dell'elenco da un nuovo listino (ADR 0022), modello
  tradizionale per tutti i documenti (ADR 0023), cronoprogramma dei lavori con Gantt
  (ADR 0024, schema 7, 57 comandi; sviluppato sul ramo `cronoprogramma`), assistente AI
  fasi 2–4: listini CSV/JSON, crate `opencomputo-assistente` e `opencomputo-rete`,
  provider OpenAI e Anthropic, portachiavi, consenso, servizio JSON e CLI, vista
  «Assistente AI» nell'interfaccia con voci alternative da spuntare (ADR 0025, ramo
  `assistente`).
  Nuovi lavori solo su richiesta dell'utente. Aperti: confronto con un export XPWE reale
  con voci, prova su macOS e Windows.
* `cargo deny check` deve restare pulito: nessuna eccezione per vulnerabilità; le
  eccezioni per crate non mantenuti vanno motivate in `deny.toml`. Versione minima di
  Rust 1.88: non usare funzionalità più recenti senza aggiornarla (ADR 0010).
* Le funzioni dell'applicazione vanno descritte in `docs/GUIDA.md` e le modifiche
  rilevanti in `CHANGELOG.md`.
* GUI (`crates/opencomputo-gui`): l'interfaccia parla con il motore solo con il comando
  `richiesta` (protocollo del servizio JSON). Il testo dei dati entra nel DOM solo con
  `h()`/`textContent`, mai come HTML; niente script o stili in linea (CSP). Ogni modifica
  dell'interfaccia va provata con `npm test` in `ui-tests` (dopo
  `cargo build -p opencomputo-agent-api --features banco-di-prova`: i test dell'assistente
  usano un provider finto locale e chiavi in memoria; la feature non va mai nei pacchetti).
  Su Linux `npm run vera` prova il binario Tauri vero con `tauri-driver` e `xvfb`
  (`cargo build -p opencomputo-gui --features opencomputo-agent-api/banco-di-prova`).
  Il comando `richiesta` gira sempre in `spawn_blocking`: niente codice bloccante nei
  comandi asincroni.
  Icone: la sorgente è `icons/opencomputo.svg`; PNG, ICO, ICNS e `ui/icona.png` si
  rigenerano con `npm run icone` in `ui-tests`, mai a mano.
* Ogni modifica al progetto passa da `opencomputo-application` (`Comando`, `Sessione`):
  GUI e CLI non scrivono il modello né chiamano i formati direttamente (ADR 0008).
  Aggiungendo una variante a `Comando` aggiornare gli esempi in
  `opencomputo-agent-api/src/schema.rs` (un test lo impone).
* Report: i valori vengono sempre dal core, mai ricalcolati negli scrittori (ADR 0007).
* Stampe di riferimento di programmi commerciali: solo per misurare l'impaginazione,
  mai nel repository; non riprodurre nomi, marchi o crediti (ADR 0015).
* DCF: mai decifrare flussi protetti né aggirare password (ADR 0006); in scrittura mai
  produrre protezioni, codici `Identity` o dichiarazioni di copyright di terzi
  (ADR 0016). I DCF reali usati
  per la verifica non vanno nel repository: il test `tests/file_reali.rs` li legge da
  `OPENCOMPUTO_DCF_REALI`.
* XPWE: la struttura è dedotta (crate `opencomputo-format-xpwe`, modulo `struttura`); non
  copiare codice da LeenO o da altri progetti, usare solo fatti sul formato.
* Le fixture XPWE sono sintetiche, tranne `primus_dcf_5_01_vuoto.xpwe` (export reale di un
  documento vuoto, con il percorso del file reso generico): non inserire nel repository
  file con dati reali di clienti o percorsi personali.
* Nessun `f32`/`f64` per valori contabili (lint `clippy::float_arithmetic = deny`).
* Date: `jiff::civil` attraverso `opencomputo_core::data` (`data_amg`, `OperazioniData`);
  niente `chrono` (non mantenuto, RUSTSEC-2026-0543).
* Assistente AI (ADR 0025): il modello propone handle di candidati e forme del catalogo
  chiuso, mai prezzi, codici o formule; i `Comando` nascono dalle scelte dell'utente e
  sono provati nella `Sessione`. I test non fanno chiamate AI reali. Chiavi solo nel
  portachiavi, mai in file, log, messaggi o `Debug`; nessuna chiamata senza consenso.
* Ogni funzionalità dichiarata completata deve avere test eseguiti e passati.
* Le fixture `tests/fixtures/progetto_esempio_v1.json`, `_v2.json`, `_v3.json`, `_v4.json`, `_v5.json`, `_v6.json` e `_v7.json`, una
  volta generate e verificate, non vanno mai rigenerate: le modifiche di schema
  richiedono una nuova versione con migrazione (e una nuova fixture).
* Documentazione e identificatori di dominio in italiano.
