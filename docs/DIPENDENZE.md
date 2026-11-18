# Dipendenze

Dipendenze dirette di `opencomputo-core` (licenze dichiarate dai rispettivi progetti,
verificate automaticamente da `cargo deny check`, vedi sotto).

| Crate | Uso | Licenza |
|-------|-----|---------|
| `rust_decimal` (feature `std`, `serde`, `maths`) | aritmetica decimale, arrotondamenti, `sqrt`/potenze nelle formule | MIT |
| `serde` | serializzazione | MIT OR Apache-2.0 |
| `serde_json` | formato di persistenza, migrazioni | MIT OR Apache-2.0 |
| `thiserror` | tipi di errore | MIT OR Apache-2.0 |
| `uuid` (feature `std`, `serde`; `v4` opzionale) | identificativi stabili | MIT OR Apache-2.0 |
| `jiff` (solo `std`, `serde`: niente fusi orari) | date civili, modulo `data` del core | Unlicense OR MIT |
| `rust_decimal_macros` (solo test) | letterali decimali `dec!()` | MIT |

Dipendenze dirette aggiuntive di `opencomputo-format-xpwe`:

| Crate | Uso | Licenza |
|-------|-----|---------|
| `quick-xml` | lettura XML (senza risoluzione di entità esterne) | MIT |
| `encoding_rs` | decodifica di UTF-8, Windows-1252 e altre codifiche dichiarate | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| `sha2` | impronta SHA-256 del file importato | MIT OR Apache-2.0 |

Dipendenze dirette aggiuntive di `opencomputo-dcf-parser` (oltre a `quick-xml`,
`encoding_rs`, `sha2`):

| Crate | Uso | Licenza |
|-------|-----|---------|
| `flate2` (backend `miniz_oxide`, Rust puro) | decompressione zlib dei flussi | MIT OR Apache-2.0 |

Indirette: `miniz_oxide` (MIT OR Zlib OR Apache-2.0), `adler2` (0BSD OR MIT OR
Apache-2.0), `crc32fast` (MIT OR Apache-2.0).

Dipendenze dirette aggiuntive di `opencomputo-io` (oltre a `flate2`, già usata):

| Crate | Uso | Licenza |
|-------|-----|---------|
| `rust_xlsxwriter` 0.100 (senza feature facoltative) | scrittura XLSX | MIT OR Apache-2.0 |

Indirette: `zip` 9 (MIT), `zopfli` (Apache-2.0), `zlib-rs` (Zlib), `typed-path` (MIT OR
Apache-2.0), `simd-adler32` (MIT), `log` (MIT OR Apache-2.0). Le larghezze dei caratteri
del PDF sono le metriche Adobe Core 14, liberamente distribuibili.

`opencomputo-storage` e `opencomputo-application` non aggiungono dipendenze esterne
(usano `flate2`, `serde`, `serde_json`, `thiserror`, `uuid`, `jiff`, `rust_decimal`).

`opencomputo-format-listino` e `opencomputo-assistente` (ADR 0025) non aggiungono
dipendenze esterne: usano `encoding_rs`, `sha2`, `serde`, `serde_json`, `thiserror`,
`jiff` e `rust_decimal`, già presenti. Il lettore CSV è scritto nel crate.

Dipendenze dirette di `opencomputo-rete` (ADR 0025, fase 3: collegamento ai provider AI
e portachiavi):

| Crate | Uso | Licenza |
|-------|-----|---------|
| `reqwest` 0.13 (solo `blocking` e `rustls-no-provider`) | client HTTPS bloccante | MIT OR Apache-2.0 |
| `rustls` 0.23 (feature `ring`, `std`, `tls12`) | TLS; crittografia `ring`, nessun OpenSSL | Apache-2.0 OR ISC OR MIT |
| `keyring-core` 1.0 | interfaccia comune ai portachiavi | MIT OR Apache-2.0 |
| `apple-native-keyring-store` 1.0 (feature `keychain`; solo macOS) | Portachiavi di macOS | MIT OR Apache-2.0 |
| `windows-native-keyring-store` 1.1 (solo Windows) | Gestione credenziali di Windows | MIT OR Apache-2.0 |
| `linux-keyutils-keyring-store` 1.0 (solo Linux) | keyring del kernel | MIT OR Apache-2.0 |

Indirette principali: `ring` (Apache-2.0 AND ISC), `rustls-webpki` (ISC),
`rustls-platform-verifier` e `rustls-native-certs` (certificati del sistema operativo),
`hyper`, `hyper-util`, `hyper-rustls`, `http`, `tokio` (usato internamente da `reqwest`
anche in modalità bloccante), `tower`, `url` e `idna` con i dati ICU (Unicode-3.0). Circa
80 crate esterni in più sul solo Linux. Per questo `deny.toml` ammette ora la licenza
**ISC** (permissiva, equivalente alla MIT) e un'eccezione puntuale per
`webpki-root-certs` (CDLA-Permissive-2.0, licenza permissiva dei dati dei certificati
radice; compare solo nel grafo delle piattaforme che non hanno certificati di sistema e
non viene compilato su Linux, macOS e Windows). `cargo deny check`: pulito.

`opencomputo-assistente` aggiunge `sha2` (impronte dei testi indicizzati), già presente.
Con la fase 4 anche `opencomputo-gui` include `opencomputo-rete` (attraverso
`opencomputo-agent-api`); l'interfaccia dell'assistente non aggiunge dipendenze né
JavaScript esterno.

Dipendenze dirette aggiuntive di `opencomputo-agent-api`:

| Crate | Uso | Licenza |
|-------|-----|---------|
| `clap` (feature `std`, `derive`, `help`, `usage`, `error-context`; senza colori) | argomenti della riga di comando | MIT OR Apache-2.0 |

Indirette: `clap_builder`, `clap_derive`, `clap_lex`, `anstyle`, `heck` (tutte MIT OR
Apache-2.0); in compilazione `proc-macro2`, `quote`, `syn` (MIT OR Apache-2.0),
`unicode-ident` ((MIT OR Apache-2.0) AND Unicode-3.0).

Dipendenze dirette aggiuntive di `opencomputo-gui`:

| Crate | Uso | Licenza |
|-------|-----|---------|
| `tauri` 2.11 (feature `wry`, `compression`, `common-controls-v6`, `custom-protocol`) | finestre native e webview | Apache-2.0 OR MIT |
| `tauri-build` 2.6 (compilazione) | contesto, permessi, icone | Apache-2.0 OR MIT |
| `tauri-plugin-dialog` 2.7 | dialoghi per aprire e salvare i file | Apache-2.0 OR MIT |

L'albero di `opencomputo-gui` conta circa 300 crate. Le licenze sono permissive (MIT,
Apache-2.0, BSD, ISC, Zlib, Unicode-3.0) tranne cinque crate sotto **MPL-2.0**
(`cssparser`, `cssparser-macros`, `selectors`, `dtoa-short`, `option-ext`): copyleft
debole a livello di file, compatibile con la EUPL-1.2 (la MPL-2.0 è tra le licenze
compatibili elencate nell'appendice della EUPL) purché i file di quei crate restino sotto
MPL. Su Linux l'applicazione usa le librerie di sistema WebKitGTK 4.1 e GTK 3 (LGPL),
collegate dinamicamente; per compilarla servono i pacchetti di sviluppo
(`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `librsvg2-dev`, `libsoup-3.0-dev`). Su macOS e
Windows si usano le webview del sistema (WKWebView, WebView2).

Test dell'interfaccia (cartella `crates/opencomputo-gui/ui-tests`, non fa parte del
prodotto): `playwright-core` 1.56.0 (Apache-2.0), con `package-lock.json`.

## Verifica automatica: `cargo deny`

`deny.toml` alla radice del workspace; `cargo deny check` verifica:

* **licenze**: solo quelle elencate (permissive, MPL-2.0, EUPL-1.2 per i crate del
  progetto); qualsiasi altra licenza fa fallire il controllo;
* **vulnerabilità** note nel database RustSec, crate ritirati da crates.io (*yanked*) e
  crate non più mantenuti. Nessuna eccezione per le vulnerabilità; sei eccezioni
  motivate per crate non mantenuti senza vulnerabilità note, interni a Tauri e GTK
  (`proc-macro-error`, cinque crate `unic-*` usati da `urlpattern`), da rivedere a ogni
  aggiornamento di Tauri;
* **sorgenti**: solo crates.io, nessuna dipendenza da repository git.

Esito al 2026-10-21: `advisories ok, bans ok, licenses ok, sources ok`. Il primo controllo
aveva trovato vulnerabilità reali, corrette aggiornando le dipendenze ([ADR 0010](adr/0010-rilascio-e-dipendenze.md)):

| Avviso | Crate | Correzione |
|--------|-------|------------|
| RUSTSEC-2026-0367, -0368, -0369 (estrazione di archivi: link simbolici, permessi) | `zip` 7.3 via `rust_xlsxwriter` | `rust_xlsxwriter` 0.100 → `zip` 9.0.5. OpenComputo scrive XLSX e non estrae archivi: non sfruttabile, ma corretto |
| RUSTSEC-2026-0194, -0195 (attributi duplicati in tempo quadratico, allocazione illimitata) | `quick-xml` 0.38 via `plist` (Tauri) | `plist` 1.10 → `quick-xml` 0.42. Il `quick-xml` che legge XPWE e DCF era già la 0.41, corretta |
| RUSTSEC-2026-0009 (esaurimento dello stack) | `time` 0.3.45 via `cookie` (Tauri) | `time` 0.3.55 |

I test dell'interfaccia (`npm audit` in `ui-tests`): 0 vulnerabilità.

## Versioni

Le versioni esatte sono fissate in `Cargo.lock`, incluso nel repository perché il
workspace produce applicazioni. Aggiornate il 2026-10-21 con il risolutore che rispetta
la versione minima di Rust dichiarata (**1.88**): `rust_decimal` 1.43.0, `serde`
1.0.229, `serde_json` 1.0.154, `thiserror` 2.0.21, `uuid` 1.26.1, `jiff` 0.2.38 (al posto di `chrono` dal 2026-11-18),
`quick-xml` 0.41.0, `encoding_rs` 0.8.42, `sha2` 0.11.1, `flate2` 1.1.10,
`rust_xlsxwriter` 0.100.0, `zip` 9.0.5, `clap` 4.6.7, `tauri` 2.11.6, `tauri-build`
2.6.3, `tauri-plugin-dialog` 2.7.3.
