# Costruire e pubblicare i pacchetti

Decisioni in [ADR 0010](adr/0010-rilascio-e-dipendenze.md).

## Prima del rilascio

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check                      # licenze, vulnerabilità, sorgenti
cargo build -p opencomputo-agent-api
(cd crates/opencomputo-gui/ui-tests && npm ci && npm test)
```

Aggiornare la versione in `Cargo.toml` (`[workspace.package] version`) e in
`crates/opencomputo-gui/tauri.conf.json`, e la sezione in `CHANGELOG.md`.

## Strumenti

```sh
cargo install tauri-cli --version "^2" --locked
cargo install cargo-deny --locked
```

## Pacchetti per sistema

Si costruiscono dalla cartella `crates/opencomputo-gui`; i file finiscono in
`target/release/bundle/`.

| Sistema | Comando | Risultato | Requisiti |
|---------|---------|-----------|-----------|
| macOS (Apple Silicon e Intel) | `cargo tauri build --target universal-apple-darwin` | `OpenComputo.app`, `.dmg` | Xcode Command Line Tools; `rustup target add aarch64-apple-darwin x86_64-apple-darwin` |
| macOS (solo questa macchina) | `cargo tauri build` | `.app`, `.dmg` | Xcode Command Line Tools |
| Windows | `cargo tauri build` | `.msi` (WiX), `-setup.exe` (NSIS) | Visual Studio Build Tools; WebView2 è già in Windows 10 e 11 |
| Linux | `cargo tauri build --bundles deb,rpm` | `.deb`, `.rpm` | pacchetti di sviluppo di WebKitGTK 4.1 (vedi README) |
| Linux | `cargo tauri build --bundles appimage` | `.AppImage` | scarica `linuxdeploy` da GitHub al primo uso |

Il workflow `rilascio.yml` (in `strumenti/ci/`, da copiare in `.github/workflows/`) costruisce tutti i pacchetti a ogni tag
`v*` (es. `git tag v0.2.0 && git push --tags`) e li allega a una release in bozza da
controllare prima di pubblicarla.

## Pacchetti non firmati

I pacchetti attuali non sono firmati:

* **macOS**: al primo avvio Gatekeeper avvisa che lo sviluppatore non è verificato.
  Aprire con clic destro → *Apri*, oppure Impostazioni di Sistema → Privacy e sicurezza
  → *Apri comunque*. Per distribuire senza avvisi servono un Apple Developer ID e la
  notarizzazione: si configurano con le variabili `APPLE_CERTIFICATE`,
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`,
  `APPLE_TEAM_ID` lette da `tauri build` (segreti del repository nella CI).
* **Windows**: SmartScreen mostra *Windows ha protetto il PC* → *Ulteriori informazioni*
  → *Esegui comunque*. Per evitarlo serve un certificato di firma del codice
  (`bundle.windows.certificateThumbprint` in `tauri.conf.json`, o un servizio di firma).
* **Linux**: nessun avviso; i pacchetti si possono firmare con `dpkg-sig`/`rpm --addsign`
  se distribuiti da un archivio.

## Verificato

* 2026-10-21, Linux (Ubuntu 24.04, WebKitGTK 2.52): `.deb` e `.rpm` costruiti; il
  `.deb` installato con `dpkg -i` registra programma, icone, voce di menu e tipo MIME
  (`gio info` riconosce un `.ocp` come `application/x-opencomputo`); l'avvio dalla voce
  di menu con un file apre il progetto.
* macOS e Windows: da provare.
