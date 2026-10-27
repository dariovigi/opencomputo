# Stato del progetto

Ultimo aggiornamento: 2026-10-27. Ultimo lavoro, sul ramo `assistente`: le **voci
alternative** dell'assistente AI (opzioni in ordine di pertinenza da spuntare, misure
modificabili; ADR 0025, decisioni 29–33), dopo la fase 4 (vista «Assistente AI»
nell'interfaccia), la fase 3
(provider OpenAI e Anthropic, portachiavi, consenso, ricerca ibrida, servizio JSON e riga
di comando) e la fase 2 (listini CSV e JSON,
ricerca nei prezzari, validazione delle risposte del modello e traduzione in comandi),
dopo il
**cronoprogramma dei lavori** con editor in stile project management e diagramma di
Gantt (ADR 0024, schema 7), dopo il **modello tradizionale per tutti i
documenti** (ADR 0023), dopo l'**aggiornamento da listino** (confronto
dell'elenco prezzi con un nuovo prezzario e aggiornamento delle voci scelte; ADR 0022),
dopo le **incidenze di manodopera e sicurezza** nella stampa del computo, con i subtotali nel riepilogo finale (ADR 0021), dopo il
**pannello dei filtri** del computo e dell'elenco prezzi (categorie, WBS, capitoli,
colonne; ADR 0020) e la **campagna di test** (fuzzing dei
formati, sequenze casuali di comandi con invarianti, contabilità ricalcolata, interfaccia
con dati ostili; 9 difetti corretti), dopo il **«vedi voce»** (righi collegati a
un'altra voce del computo, statici nel libretto; schema 6), dopo le voci affiancate al
computo (vista divisa con «>» e «<»), il trascinamento di voci tra finestre, l'export sperimentale
dell'elenco prezzi in DCF, la stampa nel modello tradizionale e il completamento delle
analisi prezzi.

Restano aperti:
* la prova in PriMus dell'XPWE del computo (file di prova consegnati al titolare);
* il trascinamento tra finestre sui sistemi reali (WebView2, WKWebView, WebKitGTK);
* il confronto con documenti PriMus reali (serve un export XPWE con voci);
* la prova su macOS e Windows.

## Assistente AI: voci alternative da scegliere

Chiesto dal titolare il 2026-10-27 dopo la prova sul Mac: troppe lavorazioni escluse per
informazioni mancanti e troppi avvisi; per ogni lavorazione le alternative in ordine di
probabilità, da spuntare e aggiungere al computo, ripetendo. Scelte del titolare: misure
modificabili, fino a 5 opzioni con pertinenza, preselezionata la più pertinente.

### Cosa è cambiato

* **Risposta del modello** (schema, prompt): per lavorazione `chiarezza` (`chiara` o
  `da_precisare` con che cosa manca), righi di misura comuni, fino a 5 `opzioni` con
  pertinenza e motivazione; valori `mancante` ammessi. Niente più ruoli né `NESSUNO`.
* **Validazione** (`proposta`) riscritta: opzioni ordinate e controllate, valori non
  validi da completare invece di escludere la lavorazione, lavorazioni senza voce solo
  se non resta nessuna opzione, dettagli tecnici con gravità «informazione».
* **Nuovo modulo `scelta`**: dalle scelte dell'utente (codici spuntati, misure corrette)
  ai comandi; righi con valori mancanti vuoti, mai quantità parziali.
* **`verifica::anteprima`**: valori di tutte le opzioni con le misure correnti e prova
  delle scelte.
* **Flusso**: si ferma con le domande solo se non riconosce nessuna lavorazione.
* **Servizio**: `assistente_anteprima {scelte}`; `assistente_applica {scelte}` (senza:
  le preselezionate) lascia la proposta disponibile; la proposta riporta `anteprima`
  invece di `prova`.
* **Interfaccia**: per lavorazione misure in campi modificabili (ipotesi in arancione,
  mancanti tratteggiate), tabella «Alternative proposte» o «Opzioni possibili» con
  spunte, pertinenza, quantità e importi aggiornati dal motore; barra fissa con le voci
  spuntate, il totale prima e dopo e «Aggiungi al computo»; voci aggiunte segnate;
  dettagli della verifica chiusi.
* **Prezzario di prova**: 4 voci in più (parquet, laminato, pittura ai silicati, smalto)
  per avere alternative vere; 26 voci.

### Verifica (Rust 1.97.0, Linux, Chromium 1243)

* `cargo test --workspace`: 391 passati, 0 falliti, 9 ignorati. Nuovi o riscritti i test
  della proposta: bagno con pavimento da precisare (1.433,82 € con le voci preselezionate,
  1.745,82 € scegliendo il gres, due opzioni sulla stessa lavorazione 519,00 €), opzioni
  ordinate, ripetute, inventate, di altra grandezza o a misura manuale, limite di 5,
  valori da completare e misure corrette (311,04 €), scelte non valide, righi scartati e
  lavorazioni senza voce, flusso che continua con la lavorazione riconosciuta.
* `cargo clippy` con e senza `--all-features`, `cargo fmt --check`, `cargo deny check`:
  puliti.
* `npm test`: 41 scenari passati, tra cui alternative con spunta di due voci, misura
  corretta (168,00 €), valore non valido che blocca l'aggiunta, proposta che resta aperta
  e terza voce aggiunta; ipotesi modificata; pavimento da precisare senza preselezione,
  superficie indicata (parquet, 1.560,00 €) o lasciata vuota (rigo da completare).
* **Non verificato**: la qualità delle opzioni con il modello vero (dipende dal prompt e
  dal prezzario reale): da provare sul Mac.

## Assistente AI, fase 4: interfaccia

Approvata dal titolare il 2026-10-27 («Approvo, passa alla Fase 4»). Decisioni 23–28
dell'[ADR 0025](adr/0025-assistente-ai.md).

### Cosa è cambiato

* **Interfaccia** (`ui/js/viste/assistente.js`, voce «Assistente AI» nella barra
  laterale), tre schede:
  * *Proposta*: consenso da accettare prima di tutto; prezzario (con «Carica
    prezzario…»), computo, richiesta, «Proponi» con attesa e «Annulla»; risultato con
    esito, voci con quantità, prezzo e importo calcolati dal motore, righi con forma e
    parametri (ipotesi evidenziate), alternative, escluse, ipotesi, informazioni
    mancanti, avvisi, totale prima e dopo, token; «Applica al computo» (un passo
    annullabile) e «Scarta»; per le domande bloccanti, «Aggiungi le domande alla
    richiesta»;
  * *Prezzari e indice*: voci indicizzate e tipo di ricerca per prezzario;
    indicizzazione con conferma esplicita dell'invio, barra di avanzamento e annullamento;
  * *Impostazioni*: consenso (data, revoca), chiavi (campi password, mai rimostrate),
    modelli con l'elenco di quelli noti, modalità dell'output, dimensioni, limiti di token.
  * avviso sempre visibile che i dati escono dal computer.
* **Listini CSV** nella finestra *Carica prezzario…* (elenco prezzi, aggiornamento da
  listino, assistente): un solo filtro comune.
* **agent-api**:
  * operazioni lunghe come `Lavoro` (`Servizio::avvia`, `Lavoro::esegui`,
    `Servizio::completa`), indici con un lucchetto per prezzario, `Interruttore` per
    annullare, generazione del progetto per scartare le proposte di un progetto
    chiuso;
  * richieste `assistente_indice` (stato dell'indice, senza chiamate) e
    `assistente_annulla`; `assistente_stato` riporta anche i modelli noti;
  * la proposta riporta `prova.voci` (anteprima del core) e `prova.computo`;
  * controllo delle dimensioni per i modelli di embedding che non le accettano;
  * `opencomputo servizio --avanzamento`;
  * feature di sviluppo `banco-di-prova` (provider locale finto, chiavi in memoria).
* **assistente**: `verifica::anteprima` (esecuzione su una copia del progetto, valori
  dalla vista del computo).
* **GUI (Rust)**: il comando `richiesta` esegue i lavori con `spawn_blocking` senza
  tenere il servizio ed emette `assistente-avanzamento` alla finestra che li ha chiesti.
* **Test dell'interfaccia**: provider finto (`ui-tests/provider-finto.mjs`), cartella di
  configurazione sempre temporanea, righe di avanzamento del motore inoltrate come
  eventi; `npm test` richiede `cargo build -p opencomputo-agent-api --features
  banco-di-prova` (README, RILASCIO, CI e CLAUDE.md aggiornati).

### Verifica (Rust 1.97.0, Linux, Chromium 1243)

* `cargo test --workspace` (ora compreso il crate grafico, dopo aver installato qui
  WebKitGTK): 391 passati, 0 falliti, 9 ignorati. Nuovi test Rust: 4 del servizio
  (lavoro eseguito in un altro thread mentre il servizio risponde, con anteprima 12,00
  m² × 7,20 € = 86,40 €; progetto cambiato durante la proposta; annullamento
  dall'interruttore prima di ogni chiamata; stato dell'indice e modelli).
* `cargo clippy --workspace --all-targets` con e senza `--all-features`, `-D warnings`:
  pulito, crate grafico compreso. `cargo fmt --check`, `cargo deny check`: puliti.
* `npm test`: 40 scenari passati (30 esistenti + 10 dell'assistente): consenso prima di
  qualsiasi chiamata; chiavi salvate e mai presenti nella pagina, nella memoria del
  browser o nei file; listino CSV caricato dalla finestra; proposta con anteprima,
  nessun prezzo né codice nei messaggi, applicazione e annullamento dalla barra;
  ipotesi evidenziate (16 m × 2,70 m ipotizzati = 43,20 m², 311,04 €); domande bloccanti;
  chiave rifiutata con collegamento alle impostazioni; indicizzazione con conferma,
  avanzamento e ricerca ibrida; modalità rifiutata per Sonnet 5.5 e strumento forzato
  davvero usato con Haiku 5.5; revoca del consenso; nessun errore JavaScript né
  violazione della CSP. Il banco rifiuta di partire con un motore senza la feature.
* **Non verificato**: l'applicazione Tauri vera (finestre, `spawn_blocking`, eventi,
  annullamento a metà di una chiamata) è compilata ma non avviata qui: i test usano il
  banco con il motore a righe, che esegue un lavoro per volta (l'annullamento a metà è
  provato solo nei test Rust); chiamate reali ai provider; macOS e Windows.

### Aperti

* Prova sul Mac con le chiavi vere: indicizzazione di un prezzario reale e qualche
  richiesta tipica dalla nuova vista.

## Assistente AI, fase 3: provider, portachiavi, consenso

Approvata dal titolare il 2026-10-27 («Approvo, passa alla Fase 3»). Decisioni 13–22
dell'[ADR 0025](adr/0025-assistente-ai.md). L'interfaccia grafica è la fase 4, da
approvare.

### Cosa è cambiato

* **`opencomputo-assistente`**:
  * modulo `provider`: tratti `Trasporto` e `Attesa` (la rete e l'orologio restano
    fuori dal crate), `Embedding`, `Generazione`, `Segreto` (mai in `Debug`), errori
    tipizzati con codice stabile, `Annullamento`;
  * client OpenAI degli embedding: lotti, controllo di numero, ordine, dimensioni e
    finitezza dei vettori; `dimensions` solo ai modelli che lo accettano;
  * client Anthropic: modalità JSON (`output_config.format`) predefinita, uso forzato
    di uno strumento rifiutato prima della chiamata sui quattro modelli che non lo
    accettano, modelli fuori elenco solo con modalità esplicita; `refusal` e
    `max_tokens` come errori;
  * nuovi tentativi (`riprova`): 5 tentativi, attesa esponenziale con variazione,
    `retry-after` fino a 120 s; 401/403/402 e `insufficient_quota` non si ripetono;
  * `vettori`: spazio di embedding versionato, indice con impronta SHA-256 del testo di
    ogni voce, formato binario `OCVEC`, indicizzazione a lotti con avanzamento e
    ripresa; coseno nell'unico modulo con aritmetica `f32`;
  * ricerca ibrida con *reciprocal rank fusion* in interi; ricerca testuale resa
    lineare (mappa dei codici e dei capitoli, prima quadratica);
  * `prompt`, `consenso` (testo versione 1), `flusso` (analisi → ricerca → selezione →
    validazione).
* **Nuovo crate `opencomputo-rete`**: `TrasportoHttps` (`reqwest` bloccante, `rustls`
  con `ring`, certificati del sistema, connessione entro 10 s, risposte fino a 64 MB),
  `AttesaReale`, `PortachiaviSistema` (macOS, Windows, Linux), `ChiaviConAmbiente` solo
  con la feature di sviluppo `chiavi-da-ambiente`.
* **application**: errore `Assistente { motivo, messaggio }`.
* **agent-api**: modulo `assistente` (configurazione in `assistente.json`, cache dei
  vettori salvata al più ogni 30 s e alla fine, ripartenza dalla cache più recente dello
  stesso spazio quando il prezzario cambia), otto richieste `assistente_*`, comando
  `opencomputo assistente` (`stato`, `configura`, `chiave`, `consenso`, `indicizza`,
  `cerca`, `proponi [--applica]`), codice di uscita 4. La proposta si applica solo con
  `assistente_applica` (passo annullabile); aprire o chiudere un progetto la dimentica.
* **deny.toml**: licenza ISC ammessa; eccezione per `webpki-root-certs`
  (CDLA-Permissive-2.0).

### Verifica (Rust 1.97.0, Linux)

* Nessuna chiamata reale ai provider: i test usano trasporti finti che imitano le
  risposte e gli errori di OpenAI e Anthropic, un server HTTP locale (127.0.0.1) per il
  trasporto reale e l'archivio finto di `keyring-core` per il portachiavi.
* Nuovi test (44):
  * assistente, unità (+24): corpo delle richieste nelle due modalità, scelta della
    modalità per modello, estrazione della risposta, `refusal`, `max_tokens`,
    classificazione degli errori e nuovi tentativi con `retry-after`, quota esaurita,
    annullamento, lotti, controllo dei vettori, formato della cache e suoi rifiuti
    (spazio diverso, file troncato o con byte in più), coseno, chiavi mai in `Debug`;
  * assistente, integrazione (+6): indicizzazione interrotta e ripresa, voci cambiate e
    rimosse, flusso completo con embedding e modello finti (il modello finto legge i
    candidati davvero inviati; nessun codice né prezzo nei messaggi), ricerca solo
    testuale senza indice valido, dati bloccanti mancanti, ricerca ibrida; prestazioni su 30 000 voci
    (indice testuale 4,5 s e ricerca ibrida 0,1 s in debug);
  * rete (5): intestazioni e corpo, tempo scaduto, connessione rifiutata, 529 e 429 con
    `retry-after` reale, 401 senza nuovi tentativi, portachiavi; nessuna chiave nei
    messaggi d'errore;
  * servizio (8) e CLI (1): consenso (versione sbagliata, revoca), nessuna chiamata senza
    consenso o senza chiave, configurazione persistente e validata, chiavi mai nelle
    risposte né nel file di configurazione, cache riusata da un nuovo processo, nuovo
    spazio che invalida la cache, prezzario modificato con una sola voce ricalcolata,
    proposta provata, applicata (86,40 €) e annullata, proposta dimenticata cambiando
    progetto, servizio trasferibile tra thread.
* `cargo test --workspace --exclude opencomputo-gui`: 382 passati, 0 falliti, 9
  ignorati (prima della fase: 338, 0, 9). `cargo clippy --all-targets --all-features`
  (con e senza la feature di sviluppo), `cargo fmt --check`, `cargo deny check`: puliti.
  `cargo doc`: nessun avviso nei crate toccati (restano due avvisi preesistenti di link
  in `core` e `application`).
* **Non verificato**: chiamate reali a OpenAI e Anthropic (nessuna chiave qui, e la rete
  di questo ambiente non le raggiunge); portachiavi di macOS e Windows (codice non
  compilato qui: mancano i target); compilazione del crate grafico, che ora include le
  nuove dipendenze (su Linux richiede WebKitGTK).

### Aperti

* Prova sul Mac del titolare con le sue chiavi: `assistente chiave`, `consenso`,
  `indicizza` su un prezzario reale, `proponi` su qualche richiesta tipica.
* Su Linux il keyring del kernel non conserva le chiavi dopo il riavvio; un archivio
  Secret Service è un possibile seguito.
* Annullamento dall'interfaccia e chiamate fuori dal thread principale: fase 4.
* I file della cache non vengono mai cancellati automaticamente.

## Assistente AI, fase 2: parti locali

Chiesto dal titolare il 2026-10-27: un assistente che proponga il computo da una
descrizione in linguaggio naturale e da un prezzario importato. Fase 1 (analisi e
progetto) approvata lo stesso giorno, con la richiesta di importare prezzari anche in DCF.
Decisioni in [ADR 0025](adr/0025-assistente-ai.md). **Nessuna chiamata AI in questa
fase**: i provider (OpenAI per gli embedding, Anthropic per la generazione), il
portachiavi e il consenso all'invio dei dati arrivano nella fase 3; l'interfaccia nella
fase 4.

### Cosa è cambiato

* **Nuovo crate `opencomputo-format-listino`**: listini CSV e JSON → `Prezzario`.
  * colonne riconosciute da nomi alternativi configurabili; separatore di campo e
    decimale riconosciuti dal contenuto; UTF-8 o Windows-1252;
  * righe scartate (codice, descrizione o unità vuoti; prezzo non valido o negativo;
    codice duplicato; campi in eccesso) segnalate con il numero di riga; prezzo zero,
    incidenze fuori da 0–100 e natura sconosciuta solo segnalati;
  * capitoli figli dal prefisso del codice; colonne non riconosciute nei dati originali
    della voce; impronta SHA-256 nella provenienza.
* **application**: `casi_uso::leggi_listino`; `leggi_prezzario` riconosce anche CSV e
  JSON (dopo DCF, file di progetto e XPWE). Ne beneficiano `carica_prezzario`, `cerca
  --prezzario` ed `esegui --prezzario`. I DCF passano dal parser esistente, con le regole
  dell'ADR 0006.
* **Nuovo crate `opencomputo-assistente`**:
  * ricerca testuale BM25 in interi a virgola fissa, con radici leggere dell'italiano,
    capitoli antenati nel testo delle voci, termini simili per trigrammi, filtri per
    dimensione dell'unità, capitolo e natura;
  * candidati con handle validi per una richiesta; il modello non vede codici né prezzi;
  * schemi JSON delle due risposte del modello (analisi e selezione) per il tool use con
    `strict: true`, senza campi facoltativi né unioni;
  * catalogo chiuso di 13 forme di rigo tradotte nei righi di misura del core, con
    conversione delle unità (cm, mm → m; kg → t con la formula del rigo);
  * estrazione dei valori numerici della richiesta: un valore che il modello dichiara
    dell'utente ma che non compare diventa un'ipotesi;
  * validazione della risposta con 15 codici di controllo (tabella nel modulo
    `proposta`), esito deciso da Rust, comandi `copia_da_prezzario` e
    `aggiungi_voce_computo` con la lavorazione, la motivazione e le ipotesi nelle note;
  * prova dei comandi nella `Sessione`.
* **CLI**: testo d'aiuto di `cerca --prezzario` aggiornato.

### Verifica (Rust 1.97.0, Linux)

* I test usano solo fixture sintetiche e risposte del modello scritte a mano: nessuna
  chiamata AI.
* Nuovi test:
  * listini: 16 test di unità, 9 d'integrazione e un esempio nella documentazione con risultati attesi riga per riga
    (CSV con 12 righe di dati, 7 importate, 5 scartate; JSON con 8 voci, 4 importate);
  * assistente: 16 test di unità, 14 d'integrazione: 13 domande di ricerca con la voce
    attesa al primo posto (anche con errori di battitura), un bagno completo dalla
    risposta al computo (1.745,82 € calcolati a mano, poi esecuzione e annullamento),
    travi in tonnellate e pali cilindrici (7.994,90 €), una tabella di 15 risposte
    sbagliate con il codice atteso, duplicati, alternative, complementari, prezzo
    dell'elenco diverso, nessuna ipotesi orfana da una lavorazione esclusa;
  * application e servizio: caricamento di listini CSV e JSON.
* `cargo test --workspace --exclude opencomputo-gui`: 338 test passati, 0 falliti, 9
  ignorati (prima della fase: 280, 0, 9). `cargo clippy … -- -D warnings`, `cargo fmt
  --check` e `cargo doc` dei due crate nuovi: puliti.
* Il crate grafico non è stato compilato (nessuna modifica; su Linux richiede WebKitGTK).
* Non verificato: compilazione e test su macOS e Windows; l'interfaccia (nessuna
  modifica); qualità della ricerca su un prezzario reale.

### Aperti

* Formato reale del prezzario regionale che si userà: il lettore è configurabile, ma
  le soglie della ricerca vanno ricalibrate sui dati veri.
* La finestra *Carica prezzario…* non mostra ancora i file `.csv` (fase 4).
* Compatibilità XPWE: invariata, export sperimentale (ADR 0006).

## Cronoprogramma dei lavori

Chiesto dal titolare il 2026-10-22, su un ramo separato (`cronoprogramma`). Decisioni in
[ADR 0024](adr/0024-cronoprogramma.md).

### Cosa è cambiato

* **core**:
  * `cronoprogramma.rs`: dati (attività dei nodi WBS e aggiunte, legami, calendario,
    linea di base) e calcolo del piano (percorso critico, livelli di dettaglio,
    produzione mensile, scostamenti);
  * campo `cronoprogramma` del progetto, schema 7 con migrazione identità e fixture v7;
  * validazione del cronoprogramma; rimozione di un nodo WBS a cascata.
* **Applicazione**:
  * sette comandi (`imposta_cronoprogramma`, `imposta_attivita`, `aggiungi_attivita`,
    `rimuovi_attivita`, `collega_attivita`, `scollega_attivita`,
    `linea_base_cronoprogramma`), con la verifica dei cicli a tutti i livelli;
  * vista `cronoprogramma`; tipo di report `cronoprogramma` con `gantt`.
* **io**: `report_cronoprogramma.rs` (tabella delle attività e produzione mensile;
  diagramma di Gantt in PDF A3 orizzontale).
* **CLI**: `report … cronoprogramma [--gantt]`; servizio: richiesta `cronoprogramma`.
* **Interfaccia**: vista «Cronoprogramma» nella sezione «Pianificazione»:
  * tabella con modifica in linea e tastiera;
  * Gantt SVG con trascinamento di barre, durate e legami;
  * scale, percorso critico, linea di base;
  * pannelli dei dettagli e della produzione mensile, finestra del calendario;
  * opzione «Diagramma di Gantt (PDF)» nella pagina dei report.

### Verifica

* `cargo test --workspace`: 285 test passati, 0 falliti, 9 ignorati.
* Interfaccia: 30 scenari passati, compreso il nuovo scenario del cronoprogramma
  (inizio, durate da tastiera, predecessori, ciclo rifiutato, livelli, pietra miliare,
  riepilogo compresso, trascinamento, note, linea di base, produzione, annulla, Gantt
  PDF, XPWE senza cronoprogramma). Collaudo: 8 scenari passati.
* Nuovi test:
  * calcolo verificato a mano sul calendario 2026: date, margini, percorso critico,
    livelli, festività e chiusure, produzione mensile, linea di base;
  * comandi e cascata;
  * fixture v7;
  * CLI del Gantt.
* PDF del Gantt guardato sulla fixture v7.

## Modello tradizionale per tutti i documenti

Chiesto dal titolare il 2026-10-22. Decisioni in
[ADR 0023](adr/0023-modello-tradizionale-per-tutti-i-documenti.md).

### Cosa è cambiato

* **io**:
  * `tradizionale/generico.rs`: il documento ordinario nella veste tradizionale
    (intestazione ripetuta, colonne dai contenuti, firma);
  * filetti verdi e verticali per riga;
  * ruolo di chi firma nel frontespizio.
* **Applicazione**:
  * `tradizionale` su ogni tipo di report e `TipoReport::tradizionale()`;
  * firma per documento;
  * il computo con le analisi accetta il modello tradizionale.
* **CLI**: `--tradizionale` per tutti i documenti; `--senza-prezzi` solo per il
  computo.
* **Interfaccia**: casella del modello tradizionale per tutti i documenti; «Con le
  analisi» anche nel modello tradizionale; le incidenze offerte quando valgono.

### Verifica

* `cargo test --workspace`: 272 test passati, 0 falliti, 9 ignorati.
* Interfaccia: 28 scenari passati (il modello tradizionale ora prova anche SAL,
  registro e computo con le incidenze); collaudo: 8 scenari passati.
* Nuovi test:
  * registro tradizionale con valori e firma;
  * tabella lunga con l'intestazione su ogni pagina;
  * tutti i tipi dal JSON, solo PDF, combinazioni rifiutate;
  * CLI.
* PDF guardati pagina per pagina: registro, SAL, libretto, sommario, certificato,
  verbale, analisi sintetica.

## Aggiornamento da listino

Chiesto dal titolare il 2026-10-22. Decisioni in
[ADR 0022](adr/0022-aggiornamento-da-listino.md).

### Cosa è cambiato

* **Core** (`aggiornamento.rs`):
  * `confronta_listino`: esiti, differenze, variazioni, importi dei computi, risorse;
  * `aggiorna_da_listino`: codici e campi scelti, ricalcolo delle analisi,
    riallineamento dei computi; tutto o niente.
* **Applicazione**:
  * comando `aggiorna_da_prezzario` (50 comandi);
  * vista `confronto_prezzario` e richiesta del servizio.
* **Interfaccia**:
  * vista «Aggiornamento da listino» con filtri, confronto della riga ed effetto sui
    computi dalla `prova`;
  * caselle nelle griglie (`spunte`) e ritorno del fuoco alla griglia dopo il clic;
  * pulsanti nell'elenco prezzi.

### Verifica

* `cargo test --workspace`: 269 test passati, 0 falliti, 9 ignorati.
* Interfaccia: 28 scenari passati (nuovo: aggiornamento da listino); collaudo: 8
  scenari passati.
* Nuovi test:
  * core: confronto con importi verificati a mano; aggiornamento con riallineamento;
    nessuna modifica in caso di errore; prefisso e tariffa; risorse e analisi
    ricalcolate (18,35 verificato a mano); libretto invariato;
  * sessione: confronto, `prova` dei totali, esecuzione e annulla;
  * interfaccia: esiti, scelte iniziali, campi, caselle e Spazio, filtro, effetto
    sui computi, aggiornamento e annulla.

## Incidenze di manodopera e sicurezza nella stampa del computo

Chiesto dal titolare il 2026-10-22. Decisioni in
[ADR 0021](adr/0021-incidenze-nel-computo.md).

### Cosa è cambiato

* **Core**: `RiepilogoComputo` con il totale della sicurezza compresa nei prezzi, le
  voci senza incidenza della sicurezza e le quote di manodopera e sicurezza per
  categorie e WBS (`QuoteRaggruppate`, `TotaliRaggruppati::cumulati`).
* **Report ordinario**:
  * colonne di quota e incidenza sulla riga «SOMMANO» e sul totale;
  * con i subtotali, quote e incidenze nei riepiloghi.
* **Modello tradizionale**:
  * colonne «MANODOPERA» e «SICUREZZA» (euro e incid.%) a destra degli importi, con
    le colonne ridistribuite (`Griglia`), i riporti e le quote nei parziali e nel
    totale (prima erano righe «di cui…» sotto ogni voce; per qualche giorno solo le
    percentuali, poi il titolare ha chiesto di nuovo importi e incidenze);
  * con i subtotali, pagine di riepilogo con intestazione propria e quadro generale
    (`tradizionale/riepilogo.rs`).
* **Richiesta e interfacce**:
  * `incidenze` sul report del computo;
  * dalla CLI, `--manodopera`, `--sicurezza`, `--subtotali`;
  * nella pagina Report, tre caselle.

### Verifica

* `cargo test --workspace`: 263 test passati, 0 falliti, 9 ignorati.
* Interfaccia: 27 scenari passati (nuovo: incidenze); collaudo: 8 scenari passati.
* Nuovi test:
  * core: quote per categorie e WBS con valori verificati a mano;
  * report ordinario: colonne, totali, riepiloghi, invarianza senza opzioni;
  * modello tradizionale: righe delle voci, pagina di riepilogo, firma, combinazioni;
  * richiesta JSON e combinazioni rifiutate; CLI.
* PDF dei due modelli guardati pagina per pagina.

## Pannello dei filtri

Chiesto dal titolare il 2026-10-22. Decisioni in
[ADR 0020](adr/0020-pannello-dei-filtri.md).

### Cosa è cambiato

* **Interfaccia**:
  * `filtri.js`, con sezioni a scelte, ad albero, a testo e a intervallo e conteggi che
    tengono conto degli altri filtri;
  * pulsante «Filtri» nel computo e nell'elenco;
  * barra «Filtro attivo» e totale delle voci filtrate;
  * assegnazione di categorie e WBS alle voci nuove;
  * ↑ e ↓ sulle voci visibili.
* **Viste del motore**: il capitolo della voce di computo; i capitoli nella vista
  dell'elenco, anche per i prezzari di consultazione. Capitoli cercati per identificativo
  (prima la ricerca era lineare per voce). Nessun comando nuovo, schema invariato.

### Verifica

Vedi la tabella della campagna di test qui sotto, aggiornata: i test Rust sono 259 e gli
scenari dell'interfaccia 26. Il nuovo scenario prova:
* categorie in OR, conteggi della WBS con il filtro;
* nodo WBS con i discendenti;
* importo da 2.000 €, tariffa assente;
* «Azzera»;
* voce aggiunta con la categoria del filtro;
* elenco filtrato per natura, per categoria d'uso nel computo e per prezzo massimo.

## Campagna di test (2026-10-22)

Chiesta dal titolare: quanti più test di funzionamento possibili, analisi delle funzioni
a rischio, report completo. Strumenti nel repository: `strumenti/collaudo/` (script
Python sul servizio), test Rust ignorati `collaudo_formule`, `collaudo_dcf`,
`collaudo_xpwe` (`cargo test -- --ignored`), `npm run collaudo` per l'interfaccia.

### Cosa è stato provato

| Prova | Volume | Esito |
|-------|--------|-------|
| Formule: confronto con un calcolo indipendente | 50.000 espressioni + 50.000 testi casuali | nessuna differenza, nessun panico |
| XPWE mutati: import → validazione → calcolo → export → reimport | 20.000 file (8.155 importati) | nessun panico; stesso totale dopo il giro |
| DCF mutati: contenitore, import, struttura, decompressione | 3.000 file × 5 funzioni + casi mirati | 1 abort (difetto 1) |
| Sequenze casuali di comandi con invarianti, salva/riapri, report, XPWE | 8 × 1.500 passi, ~5.700 comandi accettati | invarianti rispettate (oltre 10^26 € vedi limiti) |
| File di progetto incoerenti (cicli, duplicati, numeri enormi, SAL alterati) | 23 varianti × ~70 richieste | nessun crash né blocco; capitoli in ciclo non segnalati (difetto 9) |
| Contabilità ricalcolata a parte (SAL, certificati, conto finale) | 60 scenari | nessuna differenza; certificati con imponibile negativo (da decidere) |
| Caratteri speciali in PDF, XLSX, CSV, XPWE, DCF | 11 testi | difetti 5 e 6; limite dei font PDF |
| Servizio: righe malformate, file vuoti/troncati/binari/gzip/futuri, salvataggi | 40 casi | difetto 7 |
| Prestazioni | 3.000 voci (motore), 1.500 voci (interfaccia) | motore rapido; interfaccia lenta (vedi sotto) |
| Interfaccia: HTML nei dati, errori, progetto vuoto, finestra stretta, dialoghi, doppi clic, due finestre | 9 scenari | difetto 8 |

### Difetti corretti (con test di regressione)

1. Struttura DCF: una testata di flusso danneggiata chiedeva un'allocazione di 137 GB e
   terminava il processo (`struttura::pezzi`).
2. Struttura DCF: decompressione senza limite («bomba» zlib).
3. Regressione del «vedi voce»: fabbisogno di cantiere in errore su ogni computo con un
   collegamento. Tolta `VoceComputo::calcola`, che calcolava la voce da sola.
4. «-0,00» negli arrotondamenti (JSON, viste, XPWE).
5. CSV: testi interpretati come formule dai fogli di calcolo.
6. XPWE: caratteri di controllo scritti nel file (XML non ben formato).
7. Servizio a righe JSON terminato da una riga non UTF-8.
8. Più finestre: modifica applicata alla voce sbagliata dopo una rinumerazione
   nell'altra finestra (ora i comandi dell'interfaccia usano l'identificativo).
9. Validazione: capitoli con padre inesistente o in ciclo non segnalati.

### Aperti (da decidere o da fare)

* **Certificati con imponibile negativo**: penali o recupero dell'anticipazione oltre
  l'importo del SAL danno un certificato negativo. Va deciso se limitarli all'importo del
  SAL e riportare il resto.
* **Prestazioni dell'interfaccia** con computi grandi: 1.500 voci, 3,8–5 s per modificare
  una cella (ridisegno di tutto il foglio). Serve un ridisegno incrementale.
* **Font PDF**: lettere fuori dal Latin-1 stampate come «?».
* **«1.234»** digitato vale 1,234; incollato da un foglio di calcolo vale 1234.
* Importi oltre 10^26 €: le somme perdono i decimali in silenzio (irrealistico).

### Verifica

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **259 test passati** (con il pannello dei filtri), 0 falliti, 9 ignorati (5 di prima + 4 collaudi lunghi) |
| collaudi lunghi (`-- --ignored` dei tre file `collaudo_*`) | passati |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **26 scenari passati** (nuovi: modifica con due finestre e rinumerazione, pannello dei filtri); `npm run collaudo`: 8 scenari passati |

## «Vedi voce»

Chiesto dal titolare il 2026-10-22: partire dal computo, statico nel libretto, più voci
in righi diversi. Decisioni in [ADR 0019](adr/0019-vedi-voce.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **251 test passati**, 0 falliti, 5 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **24 scenari passati**. Il nuovo scenario prova: due voci citate in una volta, quantità che segue la voce citata, clic sul riferimento, esclusione delle voci in ciclo, voce citata eliminata (rigo statico), «vedi voce» statico nel libretto. |

Test nuovi:
* core `tests/vedi_voce.rs`: ordine dei collegamenti anche in avanti, formula con `vv`,
  statico, cicli (anche con sé stessa) localizzati sul rigo, voce assente o non
  calcolabile, libretto;
* fixture v6 riletta e verificata a mano: 63,00 − 51,74 + 10,00 = 21,26; libretto
  126,00 / 2 = 63,00;
* sessione: comandi, cicli rifiutati, spostamento, copia nello stesso computo e in un
  altro, rimozione, libretto, identificativo di un altro computo;
* XPWE: collegamento all'import, fallback statico (quantità diversa, ID assente), fattori
  come moltiplicatori, export dei righi inseriti in OpenComputo e rilettura;
* report e stampa tradizionale con il rimando «Vedi voce n.».

### Cosa è cambiato

* **Core**: `VediVoce` (collegata o statica) nei righi, `QuantitaRiferite`,
  `Computo::quantita_riferite` (ordine topologico, cicli), `VoceComputo::calcola_in`,
  `calcola_partita_con`, validazione del libretto. Schema 6 con migrazione e fixture v6.
* **Applicazione**: `vedi_voce` nei righi dei comandi (numero, id, computo + voce,
  statico); rimozione, copia e spostamento conservano le quantità; vista del rigo con il
  riferimento. Nessun comando nuovo (49).
* **XPWE**: import collegato con verifica delle quantità, export della quantità citata.
* **Stampe**: rimando «Vedi voce n. 12 [m² 125,40]».
* **Interfaccia**: «Vedi voce…» nel computo e nel libretto (`vedivoce.js`), etichetta
  cliccabile nel foglio, «citata da» nel dettaglio.

## Voci affiancate al computo

Chiesto dal titolare il 2026-10-21. Decisioni in [ADR 0018](adr/0018-voci-affiancate.md).

### Verifica

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **238 test passati**, 0 falliti, 5 ignorati (nessuna modifica Rust) |
| `cargo fmt --all -- --check` | pulito |
| Test end-to-end dell'interfaccia | **23 scenari passati**. Il nuovo scenario prova: apertura del pannello, due voci con «>» dopo la voce selezionata, fuoco che resta nella lista, ricerca + ↓ + Invio, «<» su una voce con «Annulla» dalla notifica, «<» su due voci, doppio clic su una voce di un prezzario non ancora nell'elenco (copiata nell'elenco e aggiunta), chiusura del pannello e preferenza. |

### Cosa è cambiato

* **Interfaccia**:
  * pulsante «Affianca voci» nel computo, con il pannello `affianca.js`;
  * pulsanti «>» e «<» nella colonna che fa da divisore trascinabile;
  * il foglio delle misure può conservare la selezione di più voci attivando una cella;
  * corretto il doppio clic nelle griglie (elenco prezzi, selettore delle voci): il
    secondo clic non ridisegna più la riga già selezionata.
* Nessun comando nuovo (49) e nessuna modifica dello schema (versione 5).

## Trascinamento di voci tra finestre

Chiesto dal titolare il 2026-10-21. Decisioni in
[ADR 0017](adr/0017-trascinamento-tra-finestre.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **238 test passati**, 0 falliti, 5 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **22 scenari passati**. Il nuovo scenario usa due pagine come due finestre e prova: elenco → computo con tre voci; due voci da un computo all'altro con la sola riserva del motore; spostamento con Maiusc; riordino nello stesso computo annullato in un passo; apertura con Ctrl+clic sul menu. |

### Cosa è cambiato

* **Comandi**: `copia_voci_computo` (49 in tutto). Copia o sposta voci nello stesso
  computo o in un altro, con righi, prezzo fotografato, categorie, WBS e note; lo
  spostamento conserva gli identificativi.
* **Applicazione desktop**:
  * comando `trascinamento` (dati del trascinamento in corso, per le altre finestre);
  * finestre nuove a cascata.
* **Interfaccia**:
  * trascinamento delle voci da griglie e testate del computo;
  * selezione multipla nell'elenco del progetto e nel computo;
  * eliminazione di più voci insieme;
  * «Aggiungi al computo» per più voci;
  * Ctrl/⌘+clic e clic centrale sul menu aprono la vista in una nuova finestra;
  * titoli delle finestre secondarie con la vista o il computo.
* **DCF**: la prova in PriMus del titolare non è riuscita: nessuno dei file si apre,
  nemmeno il prezzario originale riscritto (ADR 0016, esito in fondo). Messaggi e guida
  rimandano all'XPWE.
* Lo schema del file non cambia (versione 5).

## Export DCF dell'elenco prezzi

Esito della prova in PriMus (2026-10-21): **nessuno dei file si apre**, nemmeno il
prezzario originale riscritto da OpenComputo con la firma di ACCA intatta. Per PriMus
si usa l'XPWE; la firma non si riproduce (ADR 0016).

Chiesto dal titolare il 2026-10-21. Gli sono stati esposti i limiti: computo e
contabilità sono protetti nei documenti PriMus, e l'apertura in PriMus non è
verificabile qui. Ha scelto l'elenco prezzi in DCF più un XPWE di prova, che proverà lui
in PriMus. Decisioni in [ADR 0016](adr/0016-export-dcf.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **236 test passati**, 0 falliti, 5 ignorati (2 nuovi sui DCF reali, non nel repository) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **21 scenari passati** (export DCF aggiunto allo scenario del modello tradizionale) |
| Test ignorati sui tre DCF reali | eseguiti: struttura coerente in tutti; riscrittura riletta uguale; export dei due prezzari con le 31.755 voci identiche all'originale in tutti i 59 attributi |
| Apertura in PriMus | **da verificare** dal titolare |

### Cosa è cambiato

* **DCF**:
  * moduli `struttura` (indice, elenchi dei settori, somme di controllo XOR, CRC-16 dei
    pezzi, verifica), `scrittura` (contenitore 2,6) ed `esporta` (prezzario in chiaro);
  * il lettore ricorda quando la descrizione breve viene da `DesRid`.
* **Casi d'uso**: `esporta_dcf`.
* **Servizio**: richiesta `esporta_dcf`.
* **CLI**: `opencomputo esporta progetto.ocp elenco.dcf` (il formato segue
  l'estensione).
* **Interfaccia**: pulsante «PriMus DCF (sperimentale)…» nel report dell'elenco prezzi.
* Lo schema del file non cambia (versione 5).

## Stampa nel modello tradizionale

Chiesta dal titolare il 2026-10-21 per il computo e l'elenco prezzi, con tre stampe di
riferimento (non nel repository). Decisioni in
[ADR 0015](adr/0015-stampa-tradizionale.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **229 test passati**, 0 falliti, 3 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **21 scenari passati** (1 nuovo: modello tradizionale del computo e dell'elenco, opzioni memorizzate, XLSX e CSV disattivati) |
| Build `wasm32-unknown-unknown` | **non ripetuta**: in questo ambiente il target non è installabile (download bloccato). Il codice nuovo è puro, senza I/O né dipendenze nuove. |
| Confronto con le stampe di riferimento | misure di testi e filetti con `pdfplumber`: scarti verticali sotto 0,1 pt, orizzontali sotto 0,5 pt (font Times standard al posto di Times New Roman incorporato) |

### Cosa è cambiato

* **Core**: `misura::dettaglio_quantita`, con i risultati dei righi, i progressivi e le
  somme di positivi e deduzioni. `quantita_totale` ora lo usa: stesso risultato.
* **IO**:
  * modulo `tradizionale` con `computo` ed `elenco_prezzi` e le metriche Times;
  * `formato::numero_apice` («14´457,12») e `formato::in_lettere`
    («centodiciassette/53»);
  * `pdf::assembla` condivisa dai due scrittori PDF. Il PDF ordinario è identico byte
    per byte.
* **Casi d'uso**:
  * `TipoReport::Computo` ed `ElencoPrezzi` hanno `tradizionale`
    (`StampaTradizionale`);
  * il JSON `{"tipo": "elenco_prezzi"}` resta valido;
  * il modello tradizionale con XLSX o CSV, o con il computo con le analisi, è
    rifiutato.
* **CLI**: `--tradizionale`, `--senza-prezzi`, `--senza-frontespizio`,
  `--descrizioni-complete`, `--ente`, `--studio`.
* **Interfaccia**: casella «Modello tradizionale (PDF)» con le sue opzioni; ente e
  studio sono ricordati tra una sessione e l'altra.
* Lo schema del file non cambia (versione 5).

### Valori verificati

* Fixture v1:
  * totale stampato 14´457,12, uguale al core;
  * primo «SOMMANO m³»: 126,00 × 8,50 = 1´071,00;
  * parziale «Opere edili» 13´257,12, riportato dalla prima alla seconda pagina.
* Computo di 52 voci su 9 pagine: ogni «A RIPORTARE» coincide con il «RIPORTO» della
  pagina successiva.
* Voce di 122 righi spezzata tra due pagine: la quantità riportata è il progressivo
  del core.
* Prezzo in lettere:
  * 117,53 → centodiciassette/53; 99,24 → novantanove/24;
  * 2.323,45 → duemilatrecentoventitré/45;
  * 1.250.000 → unmilioneduecentocinquantamila.

## Completamento delle analisi

Scelto dal titolare il 2026-10-21. Decisioni in
[ADR 0014](adr/0014-variabili-e-aggiornamento-prezzi.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **219 test passati**, 0 falliti, 3 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **20 scenari passati** (1 nuovo: variabili da interfaccia, quantità con variabili e `q1`, oneri della sicurezza, aggiornamento del 10 % della manodopera, anteprima del computo con analisi) |
| Build `wasm32-unknown-unknown` di core, formati, io, storage e application | riuscita |
| Fixture v1–v4 lette con la migrazione fino alla 5; nuova fixture v5 riletta e riscritta identica | test passati |

### Cosa è cambiato

* **Core**:
  * `VariabileAnalisi` e valutazione delle quantità (`AnalisiPrezzo::quantita_righe`)
    con variabili e `q1`, `q2`… delle righe precedenti;
  * `quota_oneri_sicurezza` nell'analisi e nei parametri predefiniti, con l'importo nel
    risultato;
  * `Progetto::analisi_computo` (analisi riportate alle quantità del computo);
  * il fabbisogno usa le quantità calcolate.
* **Comandi**:
  * `imposta_analisi` accetta `variabili` e `quota_oneri_sicurezza`;
  * `imposta_parametri_analisi` accetta `quota_oneri_sicurezza`;
  * nuovo `aggiorna_prezzi` (48 comandi in tutto).
* **Report**:
  * `computo` con `con_analisi` (anche `--con-analisi` dalla CLI);
  * nelle analisi stampate, le variabili e la riga degli oneri aziendali della sicurezza.
* **Interfaccia**:
  * scheda *Variabili* nell'editor dell'analisi;
  * valore calcolato accanto alle quantità con espressione;
  * campo degli oneri della sicurezza;
  * *Aggiorna prezzi…* nella pagina Analisi prezzi;
  * casella «con le analisi delle voci» nel report del computo.
* **Schema versione 5**; nuova fixture `progetto_esempio_v5.json`, congelata.

### Valori verificati a mano

* **Intonaco**: spessore 0,015 e kg = spessore × 1400 = 21.
  * Righe: operaio `0,02+spessore*10` = 0,17 h → 5,44; cemento `kg/100` = 0,21 q → 3,15;
    betoniera `q1/2` = 0,085 h → 0,34.
  * Prezzo: costo 8,93 + spese generali 1,34 + utile 1,03 = **11,30**.
  * Oneri aziendali della sicurezza: 20 % delle spese generali = **0,27**.
* **Aggiornamento del 10 % della manodopera**: l'operaio passa da 32,00 a 35,20 e
  l'intonaco a 11,98.
* **Muratura di 100 m² nel computo con le analisi**:
  * elementi 8.742,70 + spese generali 1.311,00 + utile 1.005,00 = 11.058,70;
  * l'importo della voce è 11.059,00: differenza di arrotondamento 0,30.

### Limiti

* Mancano le tabelle di valori e le variabili comuni a più analisi.
* L'aggiornamento dei prezzi richiede la percentuale, senza indici ufficiali, e non
  tiene uno storico oltre alla cronologia della sessione.

## Analisi prezzi e nuovi prezzi

Richiesti dal titolare il 2026-10-21 («funzioni importantissime»). Decisioni e
riferimenti in [ADR 0013](adr/0013-analisi-e-nuovi-prezzi.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **214 test passati**, 0 falliti, 3 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **19 scenari passati** (1 nuovo, in sequenza: risorse; nuovo prezzo già netto; analisi con espressione; modifica di una quantità; cambio del prezzo di una risorsa e aggiornamento; verbale approvato; misura nel SAL esclusa dal ribasso; stampe di verbale, analisi, quadro sintetico e fabbisogno) |
| Build `wasm32-unknown-unknown` di core, formati, io, storage e application | riuscita |
| Fixture v1, v2 e v3 lette con la migrazione fino alla 4; nuova fixture v4 riletta e riscritta identica | test passati |

### Cosa è cambiato

* **Core, analisi**:
  * `Prezzario::calcola_analisi` usa i prezzi correnti, con sotto-analisi ricorsive e
    rilevamento dei cicli;
  * incidenze per tipo di risorsa, comprese le sotto-analisi;
  * `ParametriAnalisi` predefiniti (15 % e 10 %) ed espressione delle quantità;
  * aggiornamento esplicito dei prezzi (`ricalcola_prezzi_da_analisi`) e verifica
    dell'allineamento.
* **Core, fabbisogno**: modulo `fabbisogno`, che espande il computo nelle risorse delle
  analisi.
* **Core, nuovi prezzi**:
  * `NuovoPrezzo` sulla voce di elenco e `VerbaleNuoviPrezzi` nella contabilità;
  * `escluso_dal_ribasso` nel prezzo applicato;
  * campo `lavori_esclusi_dal_ribasso` nei riepiloghi dei SAL e del conto finale,
    omesso se zero.
* **Validazione**:
  * analisi cicliche o non calcolabili, righe orfane;
  * prezzi non allineati all'analisi, spese generali fuori dal 13–17 %, risorse
    duplicate o con prezzo negativo;
  * nuovi prezzi non approvati ma usati nel libretto, verbali non firmati, con riserve,
    vuoti o inesistenti, numeri duplicati.
* **Schema versione 4** (migrazione identità dalla 3); nuova fixture
  `progetto_esempio_v4.json`, congelata.
* **Comandi nuovi** (47 in tutto): `imposta_risorsa`, `rimuovi_risorsa`,
  `imposta_analisi`, `rimuovi_analisi`, `ricalcola_prezzi_analisi`,
  `imposta_parametri_analisi`, `imposta_nuovo_prezzo`, `imposta_verbale_nuovi_prezzi`,
  `rimuovi_verbale_nuovi_prezzi`. Nuove richieste di sola lettura nel servizio:
  `analisi_prezzi`, `nuovi_prezzi`, `fabbisogno`; `voce_prezzario` restituisce anche
  l'analisi calcolata e i dati del nuovo prezzo.
* **Report**: `analisi_prezzi` (completa per voce o sintetica), `fabbisogno`,
  `nuovi_prezzi` (elenco o verbale con firme e analisi), in PDF, XLSX e CSV, anche dalla
  CLI (`--voce`, `--sintetica`, `--verbale`).
* **Interfaccia**:
  * nuove pagine *Analisi prezzi* (risorse, parametri, voci analizzate, editor
    dell'analisi) e *Nuovi prezzi* (nuovi prezzi e verbali);
  * colonna AP/NP e collegamenti nell'elenco prezzi;
  * riga dei nuovi prezzi esclusi dal ribasso nei prospetti;
  * nuovi tipi di report.

### Valori verificati a mano

* **Malta** (1 m³), con risorse a prezzo corrente:
  * costo 143,00 + spese generali 21,45 + utile 16,45 = **180,90**;
  * incidenze: manodopera 35,38 %, materiali 40,35 %, noli 3,32 %.
* **Muratura** con 0,03 m³ di malta come sotto-analisi:
  * costo 87,43 + spese generali 13,11 + utile 10,05 = **110,59**;
  * manodopera 50,57 %, comprese le ore della malta.
* **Aggiornamento**: con il cemento da 15,00 a 16,00 la malta va a 184,69 e la muratura
  a 110,74, ma solo dopo l'aggiornamento esplicito.
* **Fabbisogno** di 100 m² di muratura e 2 m³ di malta: manodopera 5.720,00, materiali
  3.065,00, noli 30,00, trasporti 100,00, totale 8.915,00.
* **SAL 2 con il NP.1 già netto** (analisi 17,71 €/m² × 10 m² = 177,10):
  * soggetto a ribasso 11.081,60 su lavori 11.258,70;
  * ribasso 1.385,20, netto 11.073,50;
  * importo del SAL 2.899,53.

### Limiti

* **Import ed export delle analisi PriMus** (XPWE, DCF) non fatti: senza un file reale
  con analisi la struttura non è verificabile.
* **Non gestiti**:
  * variabili tra righe della stessa analisi e tabelle di valori;
  * oneri della sicurezza aziendali separati dalle spese generali;
  * coefficienti di aggiornamento dei prezzi;
  * stampa del computo con le analisi.
* **Ribasso dei nuovi prezzi**: la scelta è un dato inserito, perché la regola applicabile
  con il nuovo codice è ancora discussa. Il valore predefinito, prudente, è «soggetto».

## Conto finale e rata di saldo

Richiesto dal titolare il 2026-10-21. Decisioni e riferimenti normativi in
[ADR 0012](adr/0012-conto-finale.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **201 test passati**, 0 falliti, 3 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **18 scenari passati** (1 nuovo: stato finale, redazione, blocchi, emissione, firma, regolare esecuzione, anteprima) |
| Build `wasm32-unknown-unknown` di core, formati, io, storage e application | riuscita |
| Fixture v1 e v2 lette con la migrazione fino alla 3; nuova fixture v3 riletta e riscritta identica | test passati |

### Cosa è cambiato

* **Core** (modulo `contabilita::conto_finale`): `ContoFinale`, `RiepilogoContoFinale`,
  `Collaudo`. Le registrazioni non comprese nei SAL formano lo stato finale. Il conto
  in bozza si ricalcola. L'emissione congela il riepilogo, le registrazioni dello stato
  finale e le penali della rata di saldo, e chiude la contabilità: niente più
  registrazioni, SAL o penali. La verifica di integrità rileva le alterazioni.
* **Validazione**:
  * misurazioni successive all'ultimazione;
  * certificato di regolare esecuzione oltre tre mesi;
  * collaudo oltre sei mesi o un anno;
  * certificato di regolare esecuzione oltre 1 milione;
  * conto da firmare, riserve confermate, collaudo da approvare;
  * rata di saldo negativa.
* **Schema versione 3** (migrazione identità dalla 2); nuova fixture
  `progetto_esempio_v3.json`, congelata.
* **Comandi nuovi** (38 in tutto): `redigi_conto_finale`, `emetti_conto_finale`,
  `annulla_conto_finale`, `firma_conto_finale`, `registra_collaudo`.
* **Report**: `conto_finale` in PDF, XLSX e CSV, anche dalla CLI. Contiene stato
  finale, certificati dei SAL, conto finale, rata di saldo, lavori dello stato finale,
  penali, firma e collaudo. Il sommario del registro ha la colonna dello stato finale.
* **Interfaccia**:
  * nuova schermata *Conto finale* (redazione, emissione, importi, lavori dello stato
    finale, firma e collaudo);
  * libretto, SAL e penali si bloccano a contabilità chiusa;
  * il report del conto finale è tra quelli disponibili.

### Valori verificati a mano

Contratto di 13.000,00 €, ribasso 12,5 %, ritenuta 0,5 %, anticipazione 2.600,00 €
recuperata al 20 %, IVA 10 %. Penali: 19,50 € nel SAL 2 e 52,00 € per l'ultimazione.
Dopo il SAL 2 c'è una misurazione di 1.282,50 €.

| | Importo |
|---|---:|
| Importo netto finale | 11.974,84 |
| Acconti (SAL 1 + SAL 2) | 10.798,39 |
| Credito residuo (di cui ritenute 59,87) | 1.176,45 |
| Anticipazione non recuperata | 429,47 |
| Penali | 52,00 |
| Imponibile della rata di saldo | 694,98 |
| IVA 10 % | 69,50 |
| Totale della rata di saldo | 764,48 |

Quadratura: anticipazione 2.600,00 più imponibili dei certificati (6.486,72, 2.121,64 e
694,98) fa 11.903,34. È uguale al netto finale (11.974,84) meno le penali (71,50).

### Limiti

* Non gestiti:
  * importi e definizione delle riserve;
  * collaudi in corso d'opera;
  * garanzia della rata di saldo e DURC come dati;
  * passaggio del collaudo a definitivo;
  * interessi per ritardato pagamento.
* La data dell'invito del RUP non è registrata, quindi il termine di trenta giorni per
  la firma non è verificato.

## Contabilità completa (dopo la Fase 6)

Richiesta dal titolare il 2026-10-21. Decisioni e riferimenti normativi in
[ADR 0011](adr/0011-contabilita-completa.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo test --workspace` | **191 test passati**, 0 falliti, 3 ignorati (gli stessi) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| Test end-to-end dell'interfaccia | **17 scenari passati** (1 nuovo: IVA, anticipazione, penale, certificato, sospensione e recupero del residuo, emissione, anteprime) |
| Build `wasm32-unknown-unknown` di core, formati, io, storage e application | riuscita |
| Lettura della fixture v1 (migrazione 1 → 2) e della nuova fixture v2 | test passati |

### Cosa è cambiato

* **Core**: `Anticipazione`, `Penale`, `CertificatoPagamento` (modulo
  `contabilita::certificato`) e `SommarioRegistro` (`contabilita::sommario`). Il
  certificato di ogni SAL deduce dall'importo del SAL il recupero dell'anticipazione
  (calcolato sul netto progressivo) e le penali, poi aggiunge l'IVA. All'emissione
  certificato e penali applicate vengono congelati; la verifica di integrità li ricalcola.
* **Validazione**: percentuali dell'anticipazione e dell'IVA, anticipazione oltre il 30 %,
  per mille fuori dallo 0,5–1,5, penali oltre il 10 % dell'importo contrattuale,
  certificato con imponibile negativo, IVA non indicata.
* **Schema versione 2** (migrazione identità dalla 1); nuova fixture
  `progetto_esempio_v2.json`, congelata.
* **Comandi nuovi** (33 in tutto): `imposta_anticipazione`, `registra_penale`,
  `rimuovi_penale`, `imposta_sal`; `aliquota_iva` in `imposta_contratto`. Report
  `certificato` (con `numero`) e `sommario` in PDF, XLSX e CSV, anche dalla CLI.
* **Interfaccia**: IVA e anticipazione nei *Dati generali*; nella schermata dei SAL il
  certificato di pagamento (congelato o ricalcolato), le opzioni di recupero per la bozza
  e le penali; certificato e sommario tra i report.

### Valori verificati a mano

Contratto di 100.000,00 €, ribasso 12,5 %, ritenuta 0,5 %, anticipazione 20 % (20.000,00 €)
recuperata al 20 %, IVA 10 %, penale di 3 giorni allo 0,5 ‰ (150,00 €):

| | SAL 1 | SAL 2 |
|---|---:|---:|
| Recupero dell'anticipazione | 1.631,88 | 538,65 |
| Penali | — | 150,00 |
| Imponibile | 6.486,72 | 1.991,14 |
| IVA | 648,67 | 199,11 |
| Totale del certificato | 7.135,39 | 2.190,25 |
| Anticipazione residua | 18.368,12 | 17.829,47 |

Sommario del registro: 9.153,60 € (SAL 1) e 3.078,00 € (SAL 2), totale 12.231,60 €.

### Limiti

* Non gestiti: ritenute fiscali e previdenziali, split payment, aliquote IVA diverse
  nello stesso contratto, revoca dell'anticipazione, premi di accelerazione, rata di
  saldo e svincolo della ritenuta di garanzia.
* Le condizioni per sospendere il recupero (D.L. 107/2026, art. 1-bis: richiesta
  dell'esecutore, motivazione, esclusione del PNRR, non oltre il 31/12/2026) non sono
  verificate dal programma.

## Fase 6 — rifinitura, dipendenze, pacchetti

Avviata con l'approvazione del titolare il 2026-10-20. Decisioni in
[ADR 0010](adr/0010-rilascio-e-dipendenze.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo build --workspace --all-targets` | 0 errori, 0 avvisi |
| `cargo test --workspace` | **181 test passati**, 0 falliti, 3 ignorati (gli stessi delle fasi precedenti) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| `npm audit` (test dell'interfaccia) | 0 vulnerabilità |
| Test end-to-end dell'interfaccia | **16 scenari passati** (3 nuovi) |
| Build `wasm32-unknown-unknown` delle librerie | riuscita |
| Pacchetti Linux (`cargo tauri build`) | `.deb` e `.rpm` costruiti; `.deb` installato, avvio dalla voce di menu con un file `.ocp` riuscito |
| Ramo di codice solo macOS (file aperti dal Finder) | **non compilato** qui (manca il compilatore C di Apple): lo compila la CI su macOS |

### Cosa è cambiato

* **Dipendenze**: `cargo deny` ha trovato vulnerabilità note in `zip` 7.3 (estrazione di
  archivi), nel `quick-xml` 0.38 interno a Tauri e in `time` 0.3.45: corrette
  aggiornando a `rust_xlsxwriter` 0.100 (`zip` 9), `plist` 1.10 (`quick-xml` 0.42) e
  `time` 0.3.55. Versione minima di Rust **1.88** (da 1.85). Nessuna era sfruttabile
  nell'uso di OpenComputo (il `quick-xml` che legge XPWE e DCF era già corretto), ma ora
  il controllo è pulito e automatico. Dettagli in [DIPENDENZE.md](DIPENDENZE.md).
* **Comandi nuovi** (29 in tutto): `modifica_categoria`, `rimuovi_categoria`,
  `modifica_nodo_wbs`, `rimuovi_nodo_wbs` (rifiutati se il codice è già usato o se la voce è
  assegnata), `annotazioni` in `modifica_registrazione`.
* **Interfaccia**: selezione di più righi (Maiusc+frecce o Maiusc+clic), copia e incolla
  di righi anche con Excel e LibreOffice (testo separato da tabulazioni, separatori delle
  migliaia riconosciuti, formato interno senza perdite di deduzioni e commenti),
  eliminazione di più righi, modifica e rimozione di categorie e nodi WBS, annotazioni
  delle registrazioni, file recenti nella schermata iniziale.
* **Pacchetti**: configurazione del bundler di Tauri per `.dmg`, `.msi`, NSIS (italiano),
  `.deb`, `.rpm`, AppImage; associazione dei file `.ocp` (tipo MIME
  `application/x-opencomputo`, voce di menu Linux con `%F`, evento di apertura su macOS
  con richiesta di salvare il progetto aperto). Istruzioni in [RILASCIO.md](RILASCIO.md).
* **CI** (`strumenti/ci/`, da copiare in `.github/workflows/` alla pubblicazione): `ci.yml` con test su Linux, macOS e Windows, Rust 1.88,
  `cargo deny`, test dell'interfaccia; `rilascio.yml` con i pacchetti dei tre sistemi a
  ogni tag `v*`.
* **Documentazione**: [GUIDA.md](GUIDA.md) per gli utenti, `CHANGELOG.md`, istruzioni di
  rilascio.
* Il banco di prova dell'interfaccia trova da solo Chromium e il binario anche su altri
  sistemi (necessario per la CI).

### Limiti della Fase 6

* Pacchetti **non firmati**: Gatekeeper (macOS) e SmartScreen (Windows) avvisano al primo
  avvio. La firma richiede certificati del titolare.
* I workflow GitHub non sono stati eseguiti (il repository non è su GitHub); pacchetti
  macOS e Windows non prodotti.
* **Confronto con PriMus non fatto**: senza un export XPWE con voci, la struttura delle
  voci e gli arrotondamenti restano non confrontati con documenti reali.
* Nessun aggiornamento automatico dell'applicazione.

## Fase 5 — applicazione desktop

Avviata con l'approvazione del titolare il 2026-10-20, che ha scelto Tauri v2 e tutte e
quattro le aree (progetto ed elenco prezzi, computo, contabilità e SAL, report). Decisioni
in [ADR 0009](adr/0009-gui-tauri.md).

### Verifica (Rust 1.97.0, Chromium 141, WebKitGTK 2.52)

| Controllo | Esito |
|-----------|-------|
| `cargo build --workspace --all-targets` | 0 errori, 0 avvisi |
| `cargo test --workspace` | **178 test passati**, 0 falliti, 3 ignorati (gli stessi delle fasi precedenti) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| Build `wasm32-unknown-unknown` di `opencomputo-application` (con le viste) | riuscita |
| Test end-to-end dell'interfaccia (`npm test` in `crates/opencomputo-gui/ui-tests`) | **13 scenari passati** in Chromium con il motore reale e la stessa Content Security Policy dell'applicazione, nessun errore JavaScript |
| Applicazione compilata su Linux (WebKitGTK) sotto Xvfb | avvio con un file da argomento, modifica di un rigo da tastiera e mouse simulati, salvataggio con copia di sicurezza, seconda finestra: riusciti, schermate controllate a vista |
| Prova di carico: elenco prezzi sintetico di 30.000 voci (build di debug) | apertura del progetto 1,4 s, elenco 1,3 s, ricerca 0,3 s, scorrimento a metà elenco 24 ms |

Nuovi test Rust (5): 2 del servizio (viste per la GUI, richieste che cambiano lo stato),
3 dell'applicazione (instradamento delle richieste, apertura dal primo argomento, viste
ammesse nelle finestre). Gli scenari end-to-end coprono: progetto nuovo con voce di
elenco, voce di computo e righi scritti da tastiera, annulla/ripristina e salva con nome;
modifica di un'espressione e rifiuto di un'espressione non valida; inserimento,
spostamento ed eliminazione di voci; categorie, note, prezzo cambiato in elenco e
riallineamento; prezzario di consultazione (caricamento, copia, uso nel computo); dati
generali (campi, titolo obbligatorio, ribasso bloccato da un SAL emesso); registrazione
nel libretto, nuovo SAL, emissione e annullamento; anteprima e export PDF, XLSX, CSV,
XPWE; import XPWE; controlli con navigazione alla voce; finestra secondaria aggiornata;
chiusura con modifiche non salvate.

### Cosa fa `opencomputo-gui`

* **Avvio**: `cargo run -p opencomputo-gui` (facoltativo: `-- progetto.ocp`). Schermata
  iniziale con nuovo progetto, apertura, import da PriMus.
* **Progetto**: dati generali (metadati, contratto, arrotondamenti), elenco prezzi con
  griglia virtuale, ricerca, modifica e nuova voce, prezzari di consultazione (DCF, XPWE,
  `.ocp`) con selezione multipla e copia nell'elenco, categorie, WBS e computi.
* **Computo**: foglio delle misure con voci e righi modificabili da tastiera come in un
  foglio di calcolo (frecce, Invio, Tab, Esc, Ins, Canc), espressioni nei fattori,
  formule, deduzioni e commenti, riga vuota per il nuovo rigo; voci da elenco o da un
  prezzario caricato; spostamento ed eliminazione; pannello di dettaglio con descrizione
  estesa, categorie, WBS, note, avviso di prezzo cambiato e riallineamento; totali
  lavori, sicurezza e complessivo.
* **Contabilità**: libretto delle misure con lo stesso foglio (registrazioni dei SAL
  emessi in sola lettura), cambio data ed eliminazione; SAL in bozza con prospetto
  ricalcolato, emissione con data, annullamento della bozza, prospetto congelato dei SAL
  emessi.
* **Report**: anteprima HTML dello stesso documento da cui nascono i file, export PDF ed
  Excel in un file scelto, CSV in una cartella, XPWE sperimentale; apertura del file
  prodotto con l'applicazione di sistema.
* **Controlli**: validazione completa filtrabile per gravità, con navigazione alla voce.
* **Generale**: annulla/ripristina con la descrizione del passo, scorciatoie (Ctrl/⌘+S,
  O, N, Z, Y), conferma delle modifiche non salvate alla chiusura, nuova finestra sulla
  vista corrente aggiornata dalle modifiche delle altre, tema chiaro e scuro secondo il
  sistema.
* Nel servizio JSON (quindi anche per gli agenti) nuove richieste di sola lettura:
  `progetto`, `elenco_prezzi`, `voce_prezzario`, `computo`, `contabilita`, `anteprima`;
  `report` accetta un `file` esatto.

### Limiti della Fase 5

* Provata a vista solo su Linux (WebKitGTK) e in Chromium: **macOS e Windows vanno
  provati sulle macchine reali** (la compilazione su macOS non richiede dipendenze
  aggiuntive).
* Un progetto per istanza dell'applicazione; nessun elenco dei file recenti.
* Nessun pacchetto installabile, firma o notarizzazione; nessuna associazione del tipo di
  file `.ocp`.
* Mancano: copia/incolla di righi e voci tra il computo e i fogli di calcolo,
  trascinamento delle voci, modifica di categorie e nodi WBS esistenti, annotazioni
  delle registrazioni, configurazione dei modelli di stampa, menu nativo.
* La chiusura con modifiche non salvate è gestita dal lato Rust; la sua resa è provata
  nel banco di prova, non nell'applicazione compilata (in Xvfb manca un gestore di
  finestre che invii la richiesta di chiusura).
* Il foglio delle misure disegna tutte le righe: adatto a computi di qualche migliaia di
  righi; l'elenco prezzi invece è virtuale.

## Fase 4 — casi d'uso, file di progetto, CLI per agenti

Avviata con l'approvazione del titolare il 2026-10-20. Decisioni in
[ADR 0008](adr/0008-casi-uso-storage-cli.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo build --workspace --all-targets` | 0 errori, 0 avvisi |
| `cargo test --workspace` | **173 test passati**, 0 falliti, 3 ignorati (gli stessi della Fase 3) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| Build `wasm32-unknown-unknown` di core, formati, io, storage e application | riuscita (std compilata dai sorgenti) |

Nuovi test (30): `opencomputo-storage` 6 (andata e ritorno semplice e gzip, rotazione
delle copie, file precedente intatto dopo un errore, file danneggiati o di versione
futura); `opencomputo-application` 14 (2 unitari, 11 di integrazione, 1 esempio);
`opencomputo-agent-api` 10 (verifica degli esempi dei comandi, 4 sul servizio, 4 sul
binario eseguito come processo, 1 esempio).

Il test principale di `opencomputo-application` ricostruisce **con soli comandi JSON** il
progetto della fixture v1 (elenco prezzi, computo, contratto, quattro registrazioni, due
SAL emessi) e ottiene gli stessi valori controllati a mano nella Fase 1: totale
14.457,12 €, SAL 8.118,60 € e 2.679,79 €.

### Cosa fa `opencomputo-storage`

* File `.ocp` = documento JSON versionato del core, leggibile con un editor;
  facoltativamente gzip (riconosciuto dai byte iniziali). Limite di 2 GiB decompressi.
* Salvataggio atomico (verifica per rilettura, file temporaneo, `fsync`, rinomina) e
  copie di sicurezza a rotazione `nome.ocp.bak1…bak3`.
* Errori distinti: I/O con operazione e percorso, schema (versione futura, JSON
  invalido), contenuto, verifica fallita.

### Cosa fa `opencomputo-application`

* **25 comandi** serializzabili in JSON: metadati, contratto, regole di arrotondamento,
  capitoli e voci dell'elenco prezzi, copia da un prezzario di consultazione (DCF, XPWE
  o `.ocp`), categorie, WBS, computi, voci (aggiungi, modifica, rimuovi, sposta,
  riallinea il prezzo), righi, registrazioni del libretto, SAL (nuovo, annulla bozza,
  emetti).
* **Sessione**: richieste atomiche, rifiuto delle richieste che introducono nuovi errori
  di validazione (politica configurabile), esito con problemi comparsi e risolti e
  totali prima/dopo, prova senza applicare, annulla/ripristina fino a 50 passi.
* Casi d'uso sui formati: import XPWE e DCF, lettura di prezzari, export XPWE, report
  nei tre formati, tutti con errori serializzabili.

### Cosa fa `opencomputo-agent-api` (binario `opencomputo`)

```sh
opencomputo nuovo lavori.ocp --titolo "Nuova scuola"
opencomputo importa computo.xpwe lavori.ocp
opencomputo schema-comandi                      # regole ed esempi per agenti
opencomputo cerca lavori.ocp scavo sbancamento --prezzario regionale.dcf
opencomputo esegui lavori.ocp --comandi modifiche.json --prova
opencomputo esegui lavori.ocp --prezzario reg=regionale.dcf < modifiche.json
opencomputo info lavori.ocp
opencomputo valida lavori.ocp
opencomputo report lavori.ocp sal --numero 2 --formato pdf --uscita stampe/
opencomputo esporta lavori.ocp lavori.xpwe
opencomputo servizio lavori.ocp                 # una richiesta JSON per riga
```

* Uscita sempre JSON (`ok`, `risultato` o `errore` con `codice` e `descrizione`);
  codici di uscita 0/1/2/3 (eseguito, rifiutato, argomenti errati, file o formato).
* Il servizio accetta le richieste `apri`, `nuovo`, `importa`, `salva`, `info`,
  `valida`, `esegui`, `prova`, `annulla`, `ripristina`, `cerca`, `carica_prezzario`,
  `esporta_xpwe`, `report`, `schema_comandi`, `chiudi`, con `id` facoltativo; protegge
  le modifiche non salvate.

### Limiti della Fase 4

* Nessun blocco del file tra processi; annulla/ripristina solo nel servizio, non tra
  invocazioni successive della CLI (restano le copie di sicurezza).
* Non c'è un server MCP né un'API di rete: il servizio a righe JSON è la base su cui
  costruirli.
* Comandi mancanti: analisi prezzi e risorse, modifica o rimozione di capitoli, categorie
  e nodi WBS esistenti, note e campi minori della contabilità.
* Le istantanee di annullamento copiano l'intero progetto: adeguate a progetti di
  qualche MB, da rivedere per progetti molto grandi.
* La CLI non è stata provata su Windows e macOS (solo Linux); i percorsi e la
  rinomina atomica usano solo `std`.

## Fase 3 — report ed export

Avviata con l'approvazione del titolare il 2026-10-20. Decisioni in
[ADR 0007](adr/0007-report-ed-export.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo build --workspace --all-targets` | 0 errori, 0 avvisi |
| `cargo test --workspace` | **143 test passati**, 0 falliti, 3 ignorati (`scrivi_fixture`, `dcf_reali`, `elenco_prezzi_di_un_prezzario_regionale`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| Build `wasm32-unknown-unknown` dei quattro crate, clippy di `opencomputo-io` | riuscite (std compilata dai sorgenti) |
| PDF generati: `qpdf --check`, resa in immagine controllata a vista | nessun errore |
| XLSX generati: lettura con openpyxl e conversione con LibreOffice | valori, formati e impostazioni di stampa corretti |
| Prova di carico sui due prezzari regionali (31.755 voci ciascuno, non nel repository) | elenco prezzi in PDF di circa 2.050 pagine (4 MB) in meno di 2 s, XLSX e CSV coerenti |

Test di `opencomputo-io` (18): 6 unitari (numeri all'italiana, CSV, codifica WinAnsi e
trascrizioni, misure e a capo, nomi dei fogli), 11 di integrazione sulla fixture v1 e sulla
fixture XPWE, 1 esempio nella documentazione, più la prova di carico ignorata. La
struttura dei PDF (tabella xref e posizioni degli oggetti) è verificata nei test.

### Cosa fa `opencomputo-io`

* **Report** costruiti dai valori del core, con intestazione del progetto e data di stampa:
  * *computo metrico estimativo*: voci con righi di misura (fattori valutati, formule
    indicate, deduzioni, commenti), «sommano», totali lavori/sicurezza; riepilogo con
    manodopera; riepiloghi per super categorie, categorie, sotto categorie con incidenze;
    riepilogo WBS con totali cumulati;
  * *elenco prezzi* raggruppato per capitolo;
  * *libretto delle misure*, senza prezzi, con data e SAL di ogni registrazione;
  * *registro di contabilità* per SAL, con importi del periodo e progressivi;
  * *stato di avanzamento*: lavorazioni del periodo e prospetto (lavori, ribasso,
    sicurezza, netto, ritenuta, credito, certificati precedenti, importo del SAL),
    congelato per i SAL emessi e marcato «BOZZA» per quelli in preparazione.
* **PDF** A4 scritto in proprio: tabelle con intestazione ripetuta, testo a capo, righe
  lunghe spezzate tra le pagine, «Pagina n di N», uscita deterministica.
* **XLSX**: un foglio per sezione, valori numerici (non formule) con formato dei decimali,
  stili per voci/righi/totali, riquadri bloccati, stampa A4 adattata in larghezza con
  intestazione ripetuta e numeri di pagina.
* **CSV** per Excel in italiano (`;`, virgola decimale, BOM, colonna «Tipo»).
* Esempio: `cargo run -p opencomputo-io --example report -- progetto.json cartella [aaaa-mm-gg]`
  genera tutti i report di un progetto nei tre formati.

### Limiti della Fase 3

* PDF con font standard non incorporati: non è PDF/A. Simboli tecnici fuori da
  Windows-1252 trascritti (`≤` → `<=`, `λ` → `lambda`…), gli altri caratteri sostituiti
  con `?` e contati.
* Modelli di stampa non configurabili (colonne, logo, intestazioni personalizzate).
* Nessuna importazione da XLSX o CSV (prezzari o computi): rinviata.
* Mancano sommario del registro per articolo, certificato di pagamento, quadro economico
  e stima della manodopera per voce: richiedono prima le funzionalità nel core.
* I numeri di voce sono scritti come testo in XLSX.

## Fase 2 — formati PriMus

Avviata con l'approvazione del titolare il 2026-10-20. Decisioni e fonti in
[ADR 0006](adr/0006-formati-primus.md).

### Verifica (Rust 1.97.0)

| Controllo | Esito |
|-----------|-------|
| `cargo build --workspace --all-targets` | 0 errori, 0 avvisi |
| `cargo test --workspace` (anche `--all-features`) | **125 test passati**, 0 falliti, 2 ignorati (`scrivi_fixture`, `dcf_reali`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pulito |
| `cargo fmt --all -- --check` | pulito |
| Build e clippy `wasm32-unknown-unknown` di `opencomputo-format-xpwe` e `opencomputo-dcf-parser` | riuscite (std compilata dai sorgenti, come nella Fase 1) |
| Test `dcf_reali` sui tre DCF forniti (non nel repository) | riuscito: vedi sotto |

Test di `opencomputo-format-xpwe` (38): 12 unitari (XML, codifiche, numeri, conservazione
dei dati, sostituzione delle formule), 6 di import, 5 di andata e ritorno, 10 di
robustezza, 4 sull'export reale di PriMus-DCF, 1 esempio nella documentazione.
L'esempio `cargo run -p opencomputo-format-xpwe --example analizza_xpwe -- file.xpwe`
stampa rapporto ed export di qualsiasi file XPWE. Quelli di `opencomputo-dcf-parser` sono 13
(3 unitari, 10 su DCF sintetici) più il test su file reali. Quelli del core sono 74 (uno
in più della Fase 1: la serializzazione degli importi).

### Cosa fa `opencomputo-dcf-parser`

* **Contenitore** a settori ricostruito dall'analisi di tre file reali: flussi `AACS`,
  pezzi zlib da 1 MiB, XML in UTF-8 o UTF-16. Limiti contro i file «bomba».
* **Protezioni rispettate**: i flussi protetti (marcatore di copyright ACCA seguito da
  dati cifrati) sono contati e mai decifrati. Se contengono i dati del documento,
  l'import si ferma e suggerisce l'export XPWE. Una password di apertura blocca la
  lettura.
* **Import** di elenco prezzi (tariffa, descrizioni, unità di misura, prezzo,
  incidenze di manodopera e sicurezza, capitolo), capitoli e categorie (3 livelli) e
  dati generali. Gli attributi non mappati sono conservati nella provenienza.
* Le incidenze, scritte nei DCF con artefatti di virgola mobile
  (`62.159999999999996600`), sono arrotondate a 6 decimali; il testo originale è
  conservato.
* **Verificato su file reali** (2026-10-20): due prezzari regionali, ciascuno con
  31.755 voci importate, 770 capitoli, 3 categorie, 0 errori e 0 problemi di
  validazione. La somma dei prezzi coincide al centesimo con un controllo indipendente
  in Python. Il documento di contabilità di esempio è rifiutato perché protetto.
* Non importati: voci di computo (`CollectionVC`, che nei documenti di esempio è
  protetta), analisi (136.256 elementi nel prezzario «Analisi»), modelli di stampa e
  opzioni dell'interfaccia.
* **Condizioni d'uso**: i prezzari distribuiti da ACCA vietano di convertirli e di usarli
  con altri software. Rispettarle è responsabilità di chi importa (ADR 0006).

### Cosa fa `opencomputo-format-xpwe`

* **Import** di dati generali, capitoli e categorie (3 livelli ciascuno), elenco prezzi
  (prezzo, natura sicurezza da `Flags`, incidenze manodopera e sicurezza, capitolo),
  voci di computo con categorie e righi di misura (valori, espressioni, deduzioni,
  commenti, righi parziali, «vedi voce»), decimali dalla configurazione dei numeri.
* **Tutto ciò che non è mappato è conservato** (ordine dei campi, frammenti XML,
  attributi) e **reinserito in export**. Gli elementi sconosciuti sono segnalati.
* **Verifica delle quantità**: ogni quantità dichiarata nel file è confrontata con il
  ricalcolo e ogni differenza è segnalata; vale sempre il ricalcolo.
* **Errori espliciti con posizione** (riga e percorso XML): XML malformato, `IDEP` non
  risolto, prezzo non valido, identificativi duplicati, espressioni non valide. Le
  entità con dati essenziali non validi sono scartate e segnalate come «errore».
* **Codifiche**: UTF-8 (anche con BOM), codifica dichiarata, ripiego su Windows-1252.
* **Export sperimentale** di un computo: scrive le quantità ricalcolate e riusa gli
  identificativi originali. Segnala ogni approssimazione: formule dei righi scritte
  come espressioni, prezzi applicati non allineati all'elenco, dati non esportati
  (contabilità, WBS, note, analisi, CIG/CUP).
* **Verificato dai test**: XPWE → modello → XPWE → modello senza perdite e secondo export
  identico al primo; progetto nativo → XPWE → modello con stesse quantità e importi.
  Fixture sintetica `tests/fixtures/computo_sintetico.xpwe` con totali calcolati a mano
  (lavori 8.566,84 €, sicurezza 725,00 €, totale 9.291,84 €).
* Nessun panico su file troncati (verificati i prefissi della fixture, uno ogni 7 byte).

### Limiti della Fase 2

* **Struttura XPWE verificata solo in parte.** Un export reale di PriMus-DCF (formato
  5.01, documento vuoto; fixture `primus_dcf_5_01_vuoto.xpwe` con il solo percorso del
  file reso generico) conferma testata, dati generali e configurazione. Import ed export
  di quel file riproducono la stessa struttura e i documenti nuovi seguono lo stesso
  schema. **Voci di elenco, voci di computo e righi di misura restano dedotti da LeenO
  e non verificati**: finché non arriva un export con voci, l'export non va consegnato
  come compatibile con PriMus.
* Non importati, ma conservati e reinseriti in export: analisi prezzi (`PweEPAnalisi`),
  dati generali delle analisi (`PweDGModuli`), prezzi 2–5, incidenze materiali e
  attrezzature, date.
* «Vedi voce»: collegato alla voce citata se la quantità ricalcolata coincide con quella
  dichiarata, altrimenti statico con la quantità del file (ADR 0019). La convenzione di
  PriMus non è verificata.
* Contabilità e WBS non sono lette né scritte da XPWE.
* Capitoli: la gerarchia del modello viene esportata come tre elenchi piatti.
* Incidenze a zero importate come «non indicate».
* DCF: la natura «oneri della sicurezza» usa un'ipotesi (bit di `Fgs` come in XPWE) non
  verificabile sui file analizzati, dove nessuna voce imposta quel bit.

### Decisioni applicate il 2026-10-20

* Licenza **EUPL-1.2**: file `LICENSE`, `license = "EUPL-1.2"` nei manifest, ADR 0005
  accettato.
* **Importi a 2 decimali nel JSON**: gli importi sono scritti con almeno 2 decimali
  (`"0.00"`). La fixture v1, che contiene `"0"`, resta leggibile senza migrazione.

## Fase 1 — verifica del modello di dominio

Eseguita il 2026-10-20 con Rust 1.97.0 (x86_64 Linux) e dipendenze scaricate da crates.io.

| Controllo | Esito |
|-----------|-------|
| `cargo build --workspace --all-targets` | riuscito al primo tentativo, 0 errori, 0 avvisi |
| `cargo test --workspace` | **73 test passati**, 0 falliti (1 ignorato: `scrivi_fixture`) |
| `cargo test --workspace --all-features` | 73 passati |
| `cargo clippy --workspace --all-targets -- -D warnings` | pulito (anche con `--all-features`) |
| `cargo fmt --all -- --check` | pulito |
| `cargo run -p opencomputo-core --example computo_esempio` | totale 6.972,13 €, validazione senza problemi, andata e ritorno JSON verificata |
| Build `wasm32-unknown-unknown` (debug, release, clippy) | riuscita con le feature di default (vedi nota) |
| Versione minima Rust 1.85 | **non verificata** (toolchain 1.85 non scaricabile) |

Ripartizione dei test: 43 unitari (numeri 11, formule 8 funzioni con ≈ 50 casi, misure 7,
analisi 4, classificazione 4, partite 4, prezzario 3, id 2); integrazione: computo 8,
contabilità 10, schema 6, validazione 6.

### Fixture v1

`crates/opencomputo-core/tests/fixtures/progetto_esempio_v1.json` è stata generata il
2026-10-20 e **controllata a mano** contro i valori attesi:

* computo: scavo 126,00 m³ × 8,50 = 1.071,00; calcestruzzo 51,74 m³ × 145,30 = 7.517,82
  (cordoli 3,744 → 3,74 per rigo); muratura 109,20 m² × 42,75 = 4.668,30; recinzione
  100 m × 12,00 = 1.200,00; **totale 14.457,12 €**;
* SAL 1: lavori 7.953,60, ribasso 12,5 % 994,20, sicurezza 1.200,00, netto 8.159,40,
  ritenuta 0,5 % 40,80, **importo 8.118,60 €**;
* SAL 2 (progressivo): lavori 11.031,60, ribasso 1.378,95, netto 10.852,65, ritenuta
  54,26, credito 10.798,39, meno precedenti 8.118,60, **importo 2.679,79 €**.

Da ora la fixture è congelata: il test ignorato `scrivi_fixture` si rifiuta di
sovrascrivere una fixture esistente. Le modifiche di schema richiedono una migrazione e
una nuova fixture `v2`.

Osservazione, risolta nella Fase 2 (importi scritti con almeno 2 decimali): nel JSON gli zeri compaiono come `"0"` anziché `"0.00"` e le
misure del libretto di prova hanno la scala con cui sono state scritte (`"12"`, `"1.2"`).
Il valore numerico è corretto e il confronto lo tratta come uguale; l'eventuale
normalizzazione della scala in serializzazione andrà decisa prima della Fase 2, tenendo
conto che la fixture v1 deve restare leggibile.

### Correzioni apportate durante la verifica

* Test di integrazione: aggiunto `#![allow(clippy::unwrap_used, expect_used, panic,
  indexing_slicing)]` in testa a ciascun file, perché le opzioni `allow-*-in-tests` di
  `clippy.toml` valgono solo dentro le funzioni `#[test]` e non negli helper condivisi
  (`tests/common`). Il codice di libreria non è cambiato.
* `scrivi_fixture`: protezione contro la rigenerazione di una fixture esistente.
* `clippy.toml`: tutte le chiavi sono riconosciute da Clippy 1.97, nessuna rimossa.
* Aggiunto `Cargo.lock` (dipendenze risolte su versioni compatibili con Rust 1.85).

### Nota su WebAssembly

`static.rust-lang.org` non era raggiungibile, quindi `rustup target add` non è riuscito.
La libreria standard per `wasm32-unknown-unknown` è stata compilata dai sorgenti ufficiali
di Rust 1.97.0 (commit `2d8144b78`, identico al compilatore usato) con
`RUSTC_BOOTSTRAP=1 cargo build -Zbuild-std=std,panic_abort`. Il risultato è equivalente
al target preinstallato, ma conviene ripetere la build con `rustup target add` appena
possibile. Limiti:

* la feature `id-casuali` non compila su wasm finché non si sceglie la sorgente di
  casualità di `uuid` (es. `uuid/js`): decisione rinviata alla Fase 2;
* i test non sono stati eseguiti in un runtime WASM (servirà `wasm-bindgen-test` o simile).

## Implementato e verificato

| Area | Contenuto | Test |
|------|-----------|------|
| Grandezze e arrotondamenti | `Quantita`, `Prezzo`, `Importo`, `Percentuale`; 3 modalità; regole per progetto | unitari |
| Identificativi | id tipizzati, generatore deterministico, UUID v4 opzionale | unitari |
| Formule | parser e valutatore decimale, funzioni, variabili, limiti di sicurezza, errori con posizione | unitari (≈ 50 casi) |
| Righi di misura | parti uguali, L, B, H/peso, espressioni, formula, deduzioni, commenti | unitari |
| Prezzario | capitoli, voci, natura, incidenze, ricerca, unicità | unitari |
| Analisi prezzi | risorse, righe, spese generali, utile, incidenze; analisi dinamiche e annidate, fabbisogno di cantiere, comandi, interfaccia e stampe | unitari e integrazione, valori calcolati a mano, end-to-end |
| Nuovi prezzi | numerazione NP, criterio, verbali di concordamento con firma, riserve e approvazione, esclusione dal ribasso nei SAL | integrazione, valori calcolati a mano, end-to-end |
| Categorie e WBS | 3 livelli di categorie; albero WBS con percorsi, cicli, totali cumulati | unitari + integrazione |
| Computo | voci con prezzo fotografato, inserimento/spostamento/rimozione, riepiloghi per natura, categorie, WBS | integrazione, valori calcolati a mano |
| Contabilità | libretto, registro, SAL con ribasso, sicurezza, manodopera esclusa, ritenuta; blocco dei SAL emessi; rilevamento alterazioni | integrazione, valori calcolati a mano |
| Contabilità completa | anticipazione e recupero (anche sospeso o del residuo), penali, certificato di pagamento con IVA congelato all'emissione, sommario del registro | integrazione, valori calcolati a mano, end-to-end |
| Conto finale | stato finale, conto finale congelato con chiusura della contabilità, firma e riserve, collaudo o regolare esecuzione, rata di saldo | integrazione, valori calcolati a mano, end-to-end |
| Validazione | ≈ 30 controlli con gravità e codice stabile | integrazione |
| Schema | involucro JSON versionato, migrazioni, rifiuto versioni future, fixture v1–v5 | integrazione |
| XPWE (Fase 2) | import tollerante con verifica delle quantità; export sperimentale; andata e ritorno | unitari + integrazione, fixture sintetica calcolata a mano |
| DCF (Fase 2) | lettura dei contenuti non protetti: elenco prezzi, capitoli, categorie | unitari + integrazione, verificato su file reali |
| Report (Fase 3) | computo, elenco prezzi, libretto, registro, SAL (poi certificato di pagamento e sommario del registro) in PDF, XLSX, CSV | unitari + integrazione, valori della fixture v1 |
| File di progetto (Fase 4) | `.ocp` JSON o gzip, salvataggio atomico, copie di sicurezza | integrazione |
| Casi d'uso (Fase 4) | 25 comandi, richieste atomiche, politica di validazione, prova, annulla/ripristina | integrazione, fixture v1 ricostruita con comandi |
| CLI e servizio (Fase 4) | binario `opencomputo`, uscita JSON, servizio a righe JSON | integrazione, binario eseguito come processo |
| Applicazione desktop (Fase 5) | Tauri v2: progetto, elenco prezzi, computo, contabilità, SAL, report | end-to-end in Chromium con il motore reale, prova dell'applicazione compilata su Linux |
| Rifinitura e rilascio (Fase 6) | copia/incolla, categorie e WBS modificabili, pacchetti, `cargo deny`, CI | end-to-end, `.deb` installato e provato |

Tutti i test sono stati eseguiti e passati il 2026-10-21. «Verificato» significa verificato dai test: per XPWE non c'è ancora un confronto con PriMus.

## Non implementato

* Import da XLSX e CSV; PDF/A; modelli di stampa configurabili; server MCP o API di
  rete; firma dei pacchetti e aggiornamento automatico.
* Scrittura DCF; lettura dei DCF protetti (esclusa per scelta, ADR 0006); voci di computo
  e analisi da DCF; import dei prezzari regionali in XML aperto.
* Contabilità: lavori a corpo a percentuale, revisione prezzi, giornale dei lavori,
  riserve con importi e accordo bonario, collaudi in corso d'opera, ritenute fiscali e
  split payment nei certificati.
* Computo: prezzi multipli per voce, voci a corpo, misure collegate tra voci, riepiloghi
  per capitolo di prezzario, quadro economico.
* Import ed export delle analisi PriMus; tabelle di valori nelle analisi.
* Valute diverse dall'euro.

## Limiti noti e assunzioni

* **Arrotondamento**: default 2 decimali, metà lontano da zero, arrotondamento per rigo.
  È un'assunzione ragionevole da confrontare con documenti reali prodotti da PriMus
  prima di dichiarare la compatibilità dei risultati.
* **Normativa**: ribasso, oneri della sicurezza, esclusione della manodopera, ritenuta di
  garanzia, anticipazione, penali e IVA sono parametri; il core non verifica la conformità alla normativa vigente
  (D.Lgs. 36/2023 e allegati), che resta responsabilità del professionista.
* **Categorie**: il modello a tre elenchi piatti riflette l'organizzazione attesa nei
  formati di interscambio; anche nella struttura XPWE dedotta categorie e capitoli
  sono tre elenchi piatti, da confermare con file PriMus reali.
* **Precisione**: 28 cifre significative; i quozienti non esatti sono troncati a 28 cifre.
* **Formule**: separatore decimale `,` o `.`, argomenti separati da `;`; niente separatore
  delle migliaia.
* **Licenza**: EUPL-1.2 ([ADR 0005](adr/0005-licenza.md)).

## Decisioni in attesa

1. **File XPWE reali** per verificare la struttura dedotta: un computo di prova esportato
   da PriMus, anche senza dati riservati, è il passo che più riduce il rischio.
2. Eventuale import dei prezzari regionali in XML aperto (Allegato I.14), fonte priva
   dei vincoli d'uso dei prezzari ACCA.
3. Sorgente di casualità per `id-casuali` su WASM (`uuid/js` o generazione lato host).
4. Prova dell'applicazione su macOS e Windows (sul Mac: `cargo run -p opencomputo-gui
   --release`, oppure `cargo tauri build` per il `.dmg`).
5. Pubblicazione del repository (per esempio su GitHub) per far girare CI e rilasci.
6. Certificati per firmare i pacchetti, se si vuole distribuirli senza avvisi.
7. Eventuale adattatore MCP sopra il servizio a righe JSON.
