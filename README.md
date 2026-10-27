# OpenComputo

Software open source per computi metrici estimativi e contabilità dei lavori pubblici,
scritto in Rust, con l'obiettivo di interoperare con i formati documentali dell'ecosistema
ACCA PriMus (XPWE, e DCF in sola lettura per i contenuti non protetti) senza
riutilizzarne codice, risorse o marchi (vedi [ADR 0006](docs/adr/0006-formati-primus.md)).

> **Stato: Fase 6 completata, contabilità completa fino al conto finale, analisi prezzi e
> nuovi prezzi.** Esistono il modello di dominio, un
> import/export XPWE basato su una struttura verificata solo in parte con file PriMus
> (export sperimentale), la lettura dei DCF non protetti (elenco prezzi, capitoli,
> categorie), i report in PDF, XLSX e CSV, il salvataggio dei progetti su file, il binario
> `opencomputo` con uscita JSON per script e agenti AI e l'applicazione desktop (Tauri v2)
> per computo, contabilità (SAL, anticipazione, penali, certificati di pagamento con
> IVA, sommario del registro, conto finale e rata di saldo), analisi prezzi con
> fabbisogno di cantiere, nuovi prezzi con verbali di concordamento e report, con pacchetti installabili. L'applicazione è stata
> provata su Linux, non ancora su macOS e Windows. Guida per gli utenti:
> [docs/GUIDA.md](docs/GUIDA.md).
> Il progetto **non** è equivalente a PriMus e non va usato per documenti contabili
> ufficiali. Vedi [docs/STATO.md](docs/STATO.md) per l'elenco puntuale di cosa
> funziona e cosa manca.

## Contenuto del repository

```
Cargo.toml                 workspace
crates/
  opencomputo-core/        modello di dominio, calcoli e validazioni (puro, senza I/O)
  opencomputo-format-xpwe/ import ed export XPWE (puro, senza I/O)
  opencomputo-dcf-parser/  lettura dei DCF non protetti, export dell'elenco prezzi (puro, senza I/O)
  opencomputo-io/          report in PDF, XLSX e CSV (puro, senza I/O)
  opencomputo-storage/     file di progetto .ocp, salvataggio atomico, copie di sicurezza
  opencomputo-application/ casi d'uso: comandi, annulla/ripristina, prova, import/export
  opencomputo-agent-api/   binario `opencomputo`: CLI e servizio JSON per agenti
  opencomputo-gui/         applicazione desktop (Tauri v2): lato Rust, interfaccia in ui/,
                           test end-to-end in ui-tests/
strumenti/ci/               workflow di GitHub Actions (da copiare in .github/workflows/)
docs/
  ARCHITETTURA.md          confini dei moduli e crate previsti
  MODELLO_DATI.md          entità, regole di calcolo e di arrotondamento
  STATO.md                 cosa funziona, cosa manca, limiti noti
  GUIDA.md                 guida per gli utenti dell'applicazione
  RILASCIO.md              costruire e pubblicare i pacchetti
  DIPENDENZE.md            dipendenze e licenze
  adr/                     decisioni architetturali (ADR)
```

## Requisiti

* Rust stabile ≥ 1.88 (edizione 2024). Verificato con Rust 1.97; la CI compila anche
  con 1.88.

## Comandi

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo deny check                                    # licenze, vulnerabilità, sorgenti
cargo run -p opencomputo-core --example computo_esempio
cargo run -p opencomputo-io --example report -- \
  crates/opencomputo-core/tests/fixtures/progetto_esempio_v1.json report/
cargo install --path crates/opencomputo-agent-api   # installa il binario `opencomputo`
```

## Applicazione desktop

```sh
cargo run -p opencomputo-gui                    # facoltativo: -- progetto.ocp
cargo run -p opencomputo-gui --release          # più veloce sui progetti grandi
```

Su macOS e Windows non servono altre dipendenze. Su Linux servono i pacchetti di
sviluppo di WebKitGTK 4.1 (Debian/Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev
librsvg2-dev libsoup-3.0-dev`), richiesti anche da `cargo build --workspace`.

Pacchetti installabili (`.dmg`, `.msi`, `.deb`…): `cargo tauri build` nella cartella
`crates/opencomputo-gui`, vedi [docs/RILASCIO.md](docs/RILASCIO.md).

Test end-to-end dell'interfaccia (Node 18 o successivo, Chromium):

```sh
cargo build -p opencomputo-agent-api
cd crates/opencomputo-gui/ui-tests && npm install && npm test
```

## Uso da riga di comando e da agenti AI

Ogni comando risponde con un oggetto JSON (`{"ok": true, "risultato": …}` oppure
`{"ok": false, "errore": …}`) e un codice di uscita (0 eseguito, 1 rifiutato, 2
argomenti errati, 3 errore di file o di formato).

```sh
opencomputo nuovo lavori.ocp --titolo "Nuova scuola"
opencomputo schema-comandi            # regole ed esempi di tutti i comandi
echo '[{"comando": "aggiungi_voce_prezzario", "codice": "A.1", "unita_misura": "m³", "prezzo": "8.50"},
       {"comando": "aggiungi_voce_computo", "codice": "A.1",
        "righi": [{"lunghezza": 12, "larghezza": 8, "altezza_peso": "1,20"}]}]' \
  | opencomputo esegui lavori.ocp --prova    # totali e problemi, senza salvare
opencomputo report lavori.ocp computo --formato xlsx --uscita stampe/
opencomputo report lavori.ocp certificato --numero 2 --uscita stampe/   # certificato di pagamento del SAL 2
opencomputo report lavori.ocp computo --tradizionale --uscita stampe/   # modello tradizionale, con riporti e frontespizio
opencomputo report lavori.ocp cronoprogramma --gantt --uscita stampe/   # diagramma di Gantt (PDF A3)
opencomputo esporta lavori.ocp elenco.dcf   # elenco prezzi in DCF (sperimentale); con .xpwe il computo
opencomputo servizio lavori.ocp       # sessione con annulla/ripristina, una richiesta JSON per riga
```

Le modifiche passano tutte dagli stessi casi d'uso dell'applicazione desktop: richieste atomiche,
rifiuto dei comandi che introducono errori di validazione, SAL emessi immodificabili
([ADR 0008](docs/adr/0008-casi-uso-storage-cli.md)).

## Principi

* **Aritmetica decimale esatta** con `rust_decimal`: nessun `f32`/`f64` per prezzi,
  quantità o importi; il lint `clippy::float_arithmetic` è impostato a `deny`.
* **Arrotondamenti espliciti** e configurabili per progetto.
* **Prezzi fotografati**: aggiornare un prezzario non modifica i computi già redatti.
* **Errori espliciti** localizzati (computo › voce › rigo), mai risultati silenziosamente errati.
* **Contabilità con SAL congelati**: le registrazioni di un SAL emesso non sono modificabili,
  prospetto e certificato di pagamento vengono congelati all'emissione e le alterazioni
  esterne dei dati sono rilevate.

## Licenza

[EUPL-1.2](LICENSE) (European Union Public Licence), decisione in
[docs/adr/0005-licenza.md](docs/adr/0005-licenza.md).
