# ADR 0025 — Assistente AI per il computo e listini CSV/JSON

* Stato: accettato (fase 2 dell'assistente: parti locali, senza chiamate AI)
* Data: 2026-10-27

## Contesto

Il titolare ha chiesto un assistente che proponga un computo metrico a partire da una
descrizione in linguaggio naturale e da un prezzario importato. Il progetto è stato
impostato in un documento di analisi (fase 1, approvata il 2026-10-27) con un'architettura
pensata come applicazione autonoma: motore metrico, database SQLite, export XPWE e
frontend propri.

L'analisi del workspace ha mostrato che OpenComputo ha già quasi tutto:

* `opencomputo-core`: grandezze decimali esatte, righi di misura con fattori e formule
  sicure, arrotondamenti per progetto, prezzi fotografati, validazione con codici stabili;
* `opencomputo-application`: ogni modifica è un `Comando` serializzabile, eseguito in modo
  atomico dalla `Sessione`, con `prova` senza salvare e rifiuto dei nuovi errori;
* `opencomputo-dcf-parser` e `opencomputo-format-xpwe`: lettura dei prezzari DCF non
  protetti e import/export XPWE (export sperimentale).

Il titolare ha chiesto inoltre di poter importare prezzari in DCF.

## Decisioni

1. **Nessun motore metrico nuovo.** L'assistente produce righi di misura di `core`
   attraverso il comando esistente `aggiungi_voce_computo`. Le quantità e gli importi
   sono calcolati dal core, con le regole di arrotondamento del progetto.
2. **La risposta del modello diventa un elenco di `Comando`.** Il crate
   `opencomputo-assistente` valida la risposta e la traduce in `copia_da_prezzario` più
   `aggiungi_voce_computo`. I comandi vengono eseguiti prima con `Sessione::prova` e
   applicati solo con un'azione esplicita dell'utente. Così l'assistente passa dalle
   stesse validazioni della GUI, della CLI e degli agenti (ADR 0008) e ogni proposta si
   può annullare.
3. **Il modello non sceglie prezzi né codici.** Riceve solo i candidati trovati dalla
   ricerca, con handle brevi (`c01`, `c02`…) validi per una sola richiesta, e sceglie un
   handle o `NESSUNO`. Il prezzo arriva sempre dal prezzario attraverso
   `copia_da_prezzario` e viene fotografato nella voce (ADR 0003).
4. **Catalogo chiuso di forme di rigo.** Il modello non scrive espressioni: indica una
   forma (`conteggio`, `lunghezza`, `superficie`, `rettangolo`, `triangolo`, `trapezio`,
   `cerchio`, `volume`, `parallelepipedo`, `cilindro`, `prisma`, `massa_lineare`,
   `a_corpo`) e i suoi
   parametri. Rust controlla parametri e unità, converte in metri e chilogrammi, e scrive
   il rigo con i fattori di `core` (e, per le forme che non sono un prodotto, una formula
   generata da Rust con le variabili `par`, `lun`, `lar`, `alt`). La dimensione della forma
   deve corrispondere all'unità di misura della voce.
5. **Valori fondati.** I numeri presenti nella richiesta dell'utente sono estratti in modo
   deterministico. Un parametro che il modello dichiara «fornito dall'utente» ma che non
   compare nel testo viene riclassificato come ipotesi, con un avviso.
6. **Tracciabilità.** Ogni voce proposta conserva nelle note l'identificativo della
   lavorazione, le ipotesi e la provenienza dei parametri; la proposta completa
   (lavorazioni, ipotesi, informazioni mancanti, avvisi) è un dato serializzabile.
7. **DCF tramite il parser esistente.** L'import dei prezzari DCF passa da
   `casi_uso::leggi_prezzario` e da `opencomputo-dcf-parser`, con le regole dell'ADR 0006:
   solo contenuti non protetti, nessuna decifratura. Le condizioni d'uso dei prezzari DCF
   distribuiti da ACCA vietano di usarli con altri software; la scelta di importarli è del
   titolare (ADR 0006). L'assistente aggiunge un aspetto: le descrizioni delle voci
   verranno inviate a un provider di embedding, e la schermata di consenso lo dirà.
8. **Listini CSV e JSON** in un crate di formato nuovo, `opencomputo-format-listino`,
   puro come gli altri formati (dipende solo da `core`). Le colonne si riconoscono da
   nomi alternativi configurabili; ogni riga scartata o duplicata produce un'anomalia con
   il numero di riga; l'impronta SHA-256 del file identifica il listino. Nessuna nuova
   dipendenza esterna: il lettore CSV è scritto nel crate.
9. **Nessun database.** Progetti e prezzari restano nei file esistenti. L'indice di
   ricerca (testuale ora, vettoriale nella fase 3) sarà una cache separata per prezzario,
   legata all'impronta del file e ricostruibile.
10. **Ricerca testuale** nel crate dell'assistente: normalizzazione dell'italiano
    (minuscole, accenti, simboli di unità), parole vuote, radici leggere, punteggio BM25
    calcolato in interi a virgola fissa (nessuna aritmetica in virgola mobile, stessa
    classifica su ogni piattaforma), termini simili per trigrammi, testo arricchito con i
    capitoli antenati della voce. La ricerca non modifica il prezzario.
11. **Fasi.** Fase 2: parti locali (questo ADR). Fase 3: provider OpenAI (embedding) e
    Anthropic (tool use con `strict: true`), portachiavi, consenso. Fase 4: interfaccia,
    come viste JavaScript in `opencomputo-gui` (decisione del titolare del 2026-10-27).
12. **Vettori di embedding (fase 3).** Il lint `clippy::float_arithmetic = deny` resta.
    Il prodotto scalare tra vettori `f32` vivrà in un solo modulo con un `#[allow]`
    motivato; nessun valore contabile passa da quel modulo.

## Conseguenze

* Il motore e le validazioni restano uno solo per tutte le interfacce.
* Lo schema della risposta AI non ha campi facoltativi né unioni di tipi, per restare
  nei limiti degli structured outputs di Anthropic; i controlli che lo schema non può
  esprimere sono fatti in Rust.
* Il formato reale del prezzario regionale che il titolare userà non è ancora noto: il
  lettore CSV/JSON è configurabile e un prezzario in DCF non protetto si legge già.
* La compatibilità XPWE resta quella dichiarata nell'ADR 0006: export sperimentale.
