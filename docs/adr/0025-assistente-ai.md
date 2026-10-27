# ADR 0025 — Assistente AI per il computo e listini CSV/JSON

* Stato: accettato (fase 2: parti locali; fase 3: provider, portachiavi, consenso; fase 4: interfaccia)
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
   ricerca, con handle brevi (`c01`, `c02`…) validi per una sola richiesta, e propone
   handle (decisione 29). Il prezzo arriva sempre dal prezzario attraverso
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
11. **Fasi.** Fase 2: parti locali. Fase 3: provider OpenAI (embedding) e Anthropic
    (output strutturato), portachiavi, consenso, esposti dal servizio JSON e dalla riga
    di comando. Fase 4: interfaccia, come viste JavaScript in `opencomputo-gui`
    (decisione del titolare del 2026-10-27).
12. **Vettori di embedding.** Il lint `clippy::float_arithmetic = deny` resta. La
    similarità del coseno tra vettori `f32` vive solo in `vettori::calcolo`, con un
    `#[allow]` motivato; il punteggio esce come intero scalato e nessun valore contabile
    passa da quel modulo. La ricerca ibrida unisce le classifiche testuale e vettoriale
    con la *reciprocal rank fusion* in interi (k = 60).

### Fase 3 (2026-10-27)

13. **Due crate.** `opencomputo-assistente` contiene i client dei provider come codice
    puro: costruiscono le richieste, interpretano risposte ed errori, decidono i nuovi
    tentativi, ma spediscono attraverso il tratto `Trasporto` e aspettano attraverso
    `Attesa`. Il nuovo crate `opencomputo-rete` fornisce le implementazioni reali
    (`reqwest` bloccante con `rustls` e crittografia `ring`, certificati del sistema;
    portachiavi del sistema operativo con `keyring-core`). Così tutti i test dei client
    girano con trasporti finti, senza rete e senza chiavi.
14. **Output strutturato di Anthropic.** Predefinita la modalità JSON
    (`output_config.format` con `json_schema`), supportata da tutti i modelli dell'elenco
    verificato sulla documentazione. L'uso forzato di uno strumento con `strict: true`
    resta disponibile, ma è rifiutato prima della chiamata per Claude Opus 5.5, Sonnet
    5.5, Fable 5.1 e Mythos 5.1, che non lo accettano (errore 400). Un modello fuori
    elenco si usa solo indicando esplicitamente la modalità: non si presume che ogni
    modello supporti le stesse funzioni. `stop_reason` `refusal` e `max_tokens` sono
    errori tipizzati.
15. **Errori e nuovi tentativi.** 401/403 (chiave rifiutata), 402 e 429 con
    `insufficient_quota` (credito) non si ripetono. 408, 409, 429, 5xx e 529 si ripetono
    fino a 5 volte con attesa esponenziale e variazione casuale, rispettando
    `retry-after` (al massimo 120 s). L'identificativo della richiesta del provider è
    conservato nell'errore. Chiavi e corpi delle richieste non compaiono mai in `Debug`,
    nei messaggi o nei log.
16. **Embedding.** OpenAI `text-embedding-3-small` predefinito, modello e dimensioni
    configurabili (`dimensions` solo per i modelli `text-embedding-3`). Lo **spazio** di
    un indice è modello + dimensioni + versione del testo indicizzato: un indice di un
    altro spazio non si usa mai. Lotti di al massimo 256 testi e 200 000 caratteri;
    testi accorciati a 8 000 caratteri.
17. **Cache dei vettori** in `cache/vettori-<impronta>-<spazio>.bin` nella cartella di
    configurazione, formato binario versionato `OCVEC` con l'impronta SHA-256 del testo
    di ogni voce. Salvata (scrittura atomica) dopo ogni lotto: un'indicizzazione
    interrotta riprende dalle voci mancanti; voci cambiate o rimosse si ricalcolano o si
    tolgono. Senza un indice completo dello spazio configurato la ricerca è solo
    testuale, e funziona senza rete.
18. **Configurazione** in `assistente.json` nella cartella di configurazione
    (`OPENCOMPUTO_CONFIG`, oppure `~/Library/Application Support/OpenComputo`,
    `%APPDATA%\OpenComputo`, `$XDG_CONFIG_HOME/opencomputo`): modelli, dimensioni,
    modalità, limiti di token, consenso. **Mai le chiavi.**
19. **Chiavi** solo nel portachiavi del sistema operativo (servizio `OpenComputo`, utenti
    `openai` e `anthropic`): Portachiavi di macOS, Gestione credenziali di Windows,
    keyring del kernel su Linux (non conservato dopo il riavvio; un archivio Secret
    Service è un possibile seguito). Le variabili `OPENAI_API_KEY` e `ANTHROPIC_API_KEY`
    si leggono solo nelle build di sviluppo con la feature `chiavi-da-ambiente`. Dalla
    riga di comando la chiave si legge da stdin, mai da un argomento.
20. **Consenso versionato.** Il testo (`consenso::TESTO`, versione 1) dice che i dati
    escono dal computer, quali dati vanno a quale provider, che le chiavi restano nel
    portachiavi e ricorda le condizioni d'uso dei DCF di ACCA. Va accettato indicando la
    versione letta; se il testo cambia, il consenso precedente non vale più. Senza
    consenso nessuna chiamata ai provider; la ricerca testuale resta.
21. **Servizio JSON.** Richieste `assistente_stato`, `assistente_configura`,
    `assistente_chiave`, `assistente_consenso`, `assistente_indicizza`,
    `assistente_cerca`, `assistente_proponi`, `assistente_applica`. La proposta è provata
    nella sessione e conservata; solo `assistente_applica` la esegue, come un normale
    passo annullabile. Aprire o chiudere un progetto la dimentica. Gli errori hanno codice
    `assistente` e un `motivo` stabile (`consenso_mancante`, `chiave_assente`,
    `chiave_rifiutata`, `quota`, `rete`, `modello_non_supportato`, …); dalla riga di
    comando escono con codice 4.
22. **Chiamate bloccanti.** I client sono sincroni: l'interfaccia li esegue fuori dal
    thread principale (decisione 24).

### Fase 4 (2026-10-27)

23. **Una vista JavaScript** («Assistente AI») in `opencomputo-gui`, come le altre: parla
    con il motore solo con il comando `richiesta`; testo dei dati solo con `h()`. Tre
    schede: proposta, prezzari e indice, impostazioni. L'avviso che i dati escono dal
    computer è sempre visibile; senza consenso la scheda della proposta mostra solo il
    testo da accettare; l'indicizzazione chiede conferma dicendo quante voci verranno
    inviate. Le chiavi si inseriscono in campi password, vanno al portachiavi e non
    tornano mai all'interfaccia (lo stato dice solo se ci sono).
24. **Operazioni lunghe senza bloccare il servizio.** Indice, indicizzazione, ricerca e
    proposta sono un `Lavoro`: `Servizio::avvia` lo prepara (controlli di consenso e
    chiavi, copia del prezzario e del progetto), il comando Tauri lo esegue con
    `spawn_blocking` senza tenere il servizio, `Servizio::completa` lo conclude. Le altre
    finestre restano utilizzabili; gli indici di ogni prezzario hanno un proprio
    lucchetto. `assistente_annulla` ferma il lavoro in corso attraverso un
    `Interruttore` condiviso, senza attendere il servizio. Il servizio a righe e la CLI
    eseguono gli stessi lavori in linea.
25. **Progetto cambiato durante la proposta.** Ogni cambio di progetto (apri, nuovo,
    importa, chiudi) aumenta una generazione: una proposta conclusa su un altro progetto
    è rifiutata (`progetto_cambiato`) e non lascia nulla da applicare.
26. **Anteprima calcolata dal core.** Prima di mostrare la proposta il motore la prova
    con la politica della sessione e la esegue su una copia del progetto: quantità,
    prezzo e importo di ogni voce vengono dalla vista del computo del core
    (`verifica::anteprima`), non dall'interfaccia né dal modello.
27. **Avanzamento** dell'indicizzazione: evento Tauri `assistente-avanzamento` alla
    finestra che l'ha chiesta; nel servizio a righe, con `--avanzamento`, righe
    `{"avanzamento": …}` prima della risposta.
28. **Banco di prova.** I test dell'interfaccia usano un server locale che imita OpenAI e
    Anthropic. Il motore lo usa solo se compilato con la feature di sviluppo
    `banco-di-prova` e con `OPENCOMPUTO_PROVIDER_DI_PROVA=http://127.0.0.1:…`; in quel
    caso le chiavi restano in memoria. La feature non entra nei pacchetti; il banco
    rifiuta di partire se il motore non la ha, per non toccare il portachiavi vero.

### Voci alternative scelte dall'utente (2026-10-27)

Dopo la prova sul Mac il titolare ha chiesto meno esclusioni e avvisi, e più scelta: per
ogni lavorazione le voci possibili in ordine di probabilità, da spuntare, aggiungere al
computo e ripetere. Scelte del titolare: misure modificabili, fino a 5 opzioni con
pertinenza, preselezionata la più pertinente delle lavorazioni chiare.

29. **Opzioni invece di una voce.** La seconda chiamata restituisce per ogni lavorazione
    i righi di misura e fino a 5 opzioni (handle, pertinenza alta/media/bassa,
    motivazione), più `chiarezza`: `chiara` o `da_precisare` con che cosa manca. Non ci
    sono più ruoli (principale, complementare, alternativa) né `NESSUNO`: le alternative
    sono le opzioni, una lavorazione senza opzioni è «senza voce». Rust ordina per
    pertinenza (a parità l'ordine del modello) e preseleziona la prima solo se la
    lavorazione è chiara.
30. **Validazione che scarta il meno possibile.** Un valore non valido diventa «da
    completare», un parametro estraneo è ignorato, un rigo di grandezza diversa dagli
    altri è tolto, un'opzione incoerente (handle non inviato, unità di un'altra grandezza)
    è tolta: dettagli di gravità «informazione», mostrati chiusi. Solo un valore
    dichiarato «dell'utente» ma assente dalla richiesta resta un avviso (diventa
    ipotesi). L'analisi ferma il flusso solo se non riconosce nessuna lavorazione; le
    altre informazioni mancanti diventano domande accanto alla proposta.
31. **I comandi nascono dalle scelte** (`scelta`): per lavorazione i codici spuntati e le
    misure eventualmente corrette dall'utente; un `aggiungi_voce_computo` per opzione
    spuntata, nell'ordine della proposta. Un rigo con valori mancanti entra **vuoto**,
    con i valori noti nella descrizione: mai una quantità parziale. Le voci con unità
    senza forme (h, m²/mese) entrano allo stesso modo.
32. **Anteprima di tutte le opzioni.** `assistente_anteprima` esegue su una copia del
    progetto tutte le opzioni di tutte le lavorazioni con le misure correnti, e prova
    quelle scelte con la politica della sessione: quantità, importi e totali vengono dal
    core e l'interfaccia li aggiorna a ogni spunta o misura cambiata.
33. **La proposta resta** dopo `assistente_applica`: si possono aggiungere altre opzioni
    (ogni aggiunta è un passo annullabile). Si dimentica solo cambiando progetto o
    chiedendo una nuova proposta.

## Conseguenze

* Il motore e le validazioni restano uno solo per tutte le interfacce.
* Lo schema della risposta AI non ha campi facoltativi né unioni di tipi, per restare
  nei limiti degli structured outputs di Anthropic; i controlli che lo schema non può
  esprimere sono fatti in Rust.
* Il formato reale del prezzario regionale che il titolare userà non è ancora noto: il
  lettore CSV/JSON è configurabile e un prezzario in DCF non protetto si legge già.
* La compatibilità XPWE resta quella dichiarata nell'ADR 0006: export sperimentale.
