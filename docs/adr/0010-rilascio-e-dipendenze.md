# ADR 0010 — Rilascio, pacchetti e controllo delle dipendenze

* Stato: accettato (Fase 6)
* Data: 2026-10-21

## Contesto

Prima di distribuire l'applicazione servono: pacchetti installabili per macOS, Windows e
Linux, l'apertura dei progetti con un doppio clic, verifiche automatiche ripetibili (CI)
e il controllo delle dipendenze di terze parti, che con la GUI sono passate da circa 40
a circa 300 crate. Il primo `cargo deny check` ha trovato vulnerabilità reali in
dipendenze indirette (`zip`, `quick-xml` di Tauri, `time`).

## Decisioni

1. **Versione minima di Rust: 1.88** (era 1.85, mai verificata). Le correzioni di `zip`
   (tramite `rust_xlsxwriter` 0.100) e di `time` richiedono Rust 1.88, uscita a giugno
   2025. Il risolutore di Cargo (`resolver = "3"`) sceglie solo versioni compatibili con
   la versione dichiarata, e la CI compila con Rust 1.88. Il codice può usare le *let
   chains*: applicate le correzioni automatiche di Clippy.
2. **`cargo deny check` obbligatorio** (`deny.toml`): licenze ammesse in un elenco
   chiuso (permissive, MPL-2.0, EUPL-1.2), sorgenti solo crates.io, nessuna eccezione per
   le vulnerabilità. Le eccezioni per i crate non mantenuti sono motivate una per una nel
   file e vanno riviste a ogni aggiornamento di Tauri.
3. **Pacchetti con il bundler di Tauri** (`cargo tauri build`): `.dmg`/`.app` su macOS,
   `.msi` e installer NSIS su Windows (in italiano, installazione per l'utente), `.deb`,
   `.rpm` e AppImage su Linux. Ogni pacchetto si costruisce sul proprio sistema; il
   workflow `rilascio.yml` li costruisce tutti a ogni tag `v*` in una release in bozza.
4. **Associazione dei file `.ocp`** (tipo `application/x-opencomputo`): su Linux voce di
   menu con `Exec=… %F` e definizione del tipo MIME installate dal pacchetto; su
   Windows e Linux il file arriva come argomento; su macOS come evento
   `RunEvent::Opened`. Se un progetto è già aperto, l'interfaccia chiede se salvarlo
   prima di aprire l'altro.
5. **Pacchetti non firmati** in questa fase. La firma su Windows (certificato di firma
   del codice) e la firma con notarizzazione su macOS (Apple Developer ID) richiedono
   certificati intestati al titolare del progetto: senza, Windows SmartScreen e macOS
   Gatekeeper mostrano un avviso al primo avvio (istruzioni in `docs/RILASCIO.md`).
6. **CI** (`strumenti/ci/ci.yml`, da copiare in `.github/workflows/` alla pubblicazione su GitHub): formattazione, Clippy e test su Linux, macOS e
   Windows; compilazione con Rust 1.88; `cargo deny`; test end-to-end dell'interfaccia in
   Chromium. Il ramo di codice solo macOS (apertura dei file dal Finder) viene compilato
   solo lì: in questo ambiente di sviluppo manca il compilatore C di Apple.

## Conseguenze e limiti

* I workflow sono scritti ma non ancora eseguiti: il repository non è su GitHub.
* I pacchetti macOS e Windows non sono stati prodotti né provati (servono le rispettive
  macchine o la CI); su Linux `.deb` e `.rpm` sono stati costruiti, il `.deb`
  installato e provato.
* Nessun aggiornamento automatico dell'applicazione (il plugin *updater* di Tauri
  richiede una chiave di firma e un server dei rilasci).
* Il nome del pacchetto Linux deriva dal nome del prodotto: `open-computo`.
