# Registro delle modifiche

Formato ispirato a [Keep a Changelog](https://keepachangelog.com/it/1.1.0/); versioni
secondo il versionamento semantico.

## [0.1.0] — non ancora pubblicata

Prima versione: tutte le fasi di sviluppo 1–6. Dettagli in `docs/STATO.md`.

### Aggiunto (2026-11-13)

* Report: con il **modello tradizionale** l'anteprima mostra le pagine del PDF con la
  stessa impaginazione (prima mostrava la tabella del modello ordinario). Richiesta
  `anteprima` con `prima_pagina` e `pagine` (ADR 0015).
* Import XPWE: le **analisi dei prezzi** entrano come analisi di OpenComputo collegate
  alle voci. Spese generali, utile e tipo dei componenti, che il file non contiene, sono
  dedotti dal prezzo e dalle incidenze dichiarati e riportati nel riepilogo dell'import;
  il prezzo della voce resta quello del file (ADR 0006).

### Corretto (finestra e XPWE, 2026-11-13)

* Ridimensionamento della finestra: con un titolo dei lavori lungo la pagina diventava
  più larga della finestra e le colonne di quantità, prezzo e importo non si vedevano,
  a qualsiasi larghezza. Ora la pagina segue sempre la finestra.
* Finestre strette: sotto 1450 px il pannello di dettaglio passa sotto la tabella; sotto
  1250 px colonne dei numeri e barra laterale più compatte; nelle griglie (elenco
  prezzi, aggiornamento da listino) le colonne che non entrano si raggiungono scorrendo,
  con l'intestazione allineata.
* Colonna «Tariffa» più larga: i codici dei prezzari regionali si leggono per intero.
* Percentuali con molte cifre (incidenze lette da un XPWE) mostrate con due decimali.
* Import XPWE: niente più avvisi «quantità diversa» quando la `Quantita` dei righi
  ripete il totale della voce (osservato in un export reale); un contenitore di analisi
  vuoto non è più segnalato come analisi non importata.

### Prestazioni e robustezza dell'assistente AI (2026-11-12)

Misure su un listino di 30.000 voci con vettori di 1536 dimensioni (Linux, `--release`,
`cargo run --release -p opencomputo-agent-api --example misura_assistente`).

* Caricamento di un listino CSV: da 6,2 s a 0,1 s (il controllo dei codici doppi era
  quadratico).
* Ricerca per parole: prima ricerca da 0,97 s a 0,30 s, le successive da 88 a 18 ms;
  ricerca ibrida da 190 a 86 ms; proposta (escluso il modello) da 212 a 101 ms.
* Indicizzazione senza voci nuove: da 645 a 31 ms, senza riscrivere la cache.
* Memoria massima da 537 a 358 MB: la cache dei vettori si legge e si scrive a flusso.
* **Annulla** interrompe subito anche una richiesta già partita e le attese tra i
  tentativi (prima aspettava la risposta del provider o la fine dell'attesa), e ferma
  tutti i lavori in corso, non solo l'ultimo.
* Indicizzazione, ricerca e proposta sullo stesso prezzario non si bloccano più a
  vicenda; una seconda indicizzazione dello stesso prezzario è rifiutata
  (`indicizzazione_in_corso`).
* Un file `assistente.json` danneggiato non blocca più l'assistente: viene rinominato
  in `assistente.json.danneggiato`, tornano i valori predefiniti, il consenso va ridato
  e la vista lo segnala.
* Una risposta del provider interrotta a metà viene ritentata come un errore di rete.
* Servizio a righe JSON: `assistente_annulla` ha effetto anche mentre un lavoro è in
  corso (le risposte restano nell'ordine delle richieste).
* Richiesta all'assistente: al massimo 20.000 caratteri (`richiesta_troppo_lunga`).
* Interfaccia: se una nuova richiesta fallisce o viene annullata resta visibile la
  proposta precedente.
* Applicazione: tutte le richieste girano su thread per operazioni bloccanti. Nelle
  build di sviluppo (non `--release`) la prima richiesta all'assistente faceva fallire
  il comando; trovato provando il programma vero con `tauri-driver`.

### Corretto (interfaccia)

* Barra laterale: le voci dei computi avevano un rientro e un carattere più piccolo
  delle altre voci; ora sono allineate.

### Corretto (campagna di test del 2026-10-22)

* Fabbisogno di cantiere e relativo report: fallivano su ogni computo con un «vedi voce»
  collegato.
* Più finestre: una modifica in una finestra mentre un'altra eliminava o spostava voci
  finiva sulla voce sbagliata; l'interfaccia ora indica le voci per identificativo.
* CSV: i testi che sembrano formule (`=`, `+`, `@`, `-`) sono preceduti da un apostrofo.
* XPWE: i caratteri di controllo, non ammessi da XML, non sono più scritti nel file.
* Niente più «-0,00»: uno zero arrotondato è sempre positivo (JSON, viste, XPWE).
* Servizio a righe JSON: una riga non UTF-8 riceve un errore invece di terminare il
  processo.
* Lettura della struttura DCF: una testata di flusso danneggiata non chiede più
  allocazioni enormi (terminava il processo); i pezzi decompressi hanno un limite.
* Validazione: segnalati i capitoli dell'elenco con padre inesistente o in ciclo.

### Aggiunto

* **Assistente AI, voci alternative** (ADR 0025, decisioni 29–33): per ogni lavorazione
  fino a cinque voci in ordine di pertinenza, «Alternative proposte» (la più pertinente
  spuntata) o «Opzioni possibili» quando manca un'informazione per scegliere; misure
  modificabili, valori mancanti da completare; quantità e importi di ogni opzione
  calcolati dal motore e aggiornati a ogni scelta; «Aggiungi al computo» per le voci
  spuntate, anche più di una per lavorazione, con la proposta che resta aperta per
  continuare. Molte meno lavorazioni escluse e avvisi: i dettagli della verifica sono
  raccolti a parte. Richiesta `assistente_anteprima`; `assistente_applica` accetta le
  `scelte`.
* **Assistente AI nell'interfaccia** (ADR 0025, fase 4): vista «Assistente AI» con
  consenso, chiavi, modelli, indice dei prezzari con avanzamento e annullamento, e
  proposta con anteprima delle voci (quantità, prezzi e importi calcolati dal motore),
  ipotesi evidenziate, domande, alternative ed esclusioni; «Applica al computo» come
  passo annullabile. Le operazioni lunghe non bloccano le altre finestre. Richieste
  `assistente_indice` e `assistente_annulla`; `opencomputo servizio --avanzamento`.
  La finestra *Carica prezzario…* accetta anche i listini CSV.
* **Assistente AI, provider** (ADR 0025, fase 3; riga di comando e servizio JSON,
  l'interfaccia grafica arriverà dopo):
  * embedding OpenAI (`text-embedding-3-small` predefinito, modello e dimensioni
    configurabili) e generazione Anthropic (`claude-sonnet-5-5` predefinito) con
    risposte JSON vincolate a uno schema; uso forzato di uno strumento come alternativa,
    rifiutato in anticipo sui modelli che non lo accettano; modelli sconosciuti solo con
    la modalità indicata;
  * nuovi tentativi su sovraccarico, limiti di frequenza ed errori del server, con
    `retry-after`; nessun nuovo tentativo con chiave rifiutata o credito esaurito; errori
    con codice stabile (`motivo`) e identificativo della richiesta del provider;
  * ricerca ibrida (parole + significato, fusione delle classifiche in interi) e cache
    dei vettori per prezzario, salvata durante l'indicizzazione: ripresa dopo
    un'interruzione, ricalcolo delle sole voci cambiate;
  * chiavi nel portachiavi del sistema (crate `opencomputo-rete`), mai nei file né nei
    messaggi; da riga di comando lette da stdin;
  * consenso versionato all'invio dei dati, obbligatorio prima di ogni chiamata;
  * richieste `assistente_stato`, `assistente_configura`, `assistente_chiave`,
    `assistente_consenso`, `assistente_indicizza`, `assistente_cerca`,
    `assistente_proponi`, `assistente_applica`; comando `opencomputo assistente` con
    codice di uscita 4 per gli errori dell'assistente.
* **Assistente AI, parti locali** (ADR 0025; nessuna chiamata ai provider in questa
  fase):
  * listini prezzi **CSV e JSON** (crate `opencomputo-format-listino`): colonne
    riconosciute dall'intestazione, separatori riconosciuti dal contenuto, righe scartate
    e duplicati segnalati con il numero di riga, capitoli dal codice; caricabili come
    prezzari di consultazione come i DCF non protetti e gli XPWE;
  * crate `opencomputo-assistente`: ricerca testuale nei prezzari (BM25 deterministico,
    radici dell'italiano, termini simili), candidati con handle per il modello, schemi
    JSON delle risposte per il tool use con `strict: true`, catalogo chiuso di forme di
    rigo, controllo dei valori dichiarati dall'utente, validazione delle risposte con
    codici stabili e traduzione in comandi provati nella sessione.
* **Cronoprogramma dei lavori** sulla WBS (ADR 0024, schema 7):
  * durate in giorni lavorativi e legami FI, II, FF, IF con ritardo o anticipo,
    vincolo «inizio non prima del», avanzamento, note;
  * attività e pietre miliari aggiunte a quelle della WBS;
  * livello di dettaglio a scelta tra i livelli della WBS, con durate proprie dei nodi
    ridotti o, se mancano, la durata del loro riepilogo;
  * percorso critico con date al più presto e al più tardi e margine totale; cicli
    rifiutati con le attività coinvolte;
  * calendario con giorni lavorativi, festività nazionali (Pasqua calcolata) e
    chiusure del cantiere; ultimazione contrattuale;
  * produzione mensile dagli importi del computo, con somme esatte e curva a S;
  * linea di base con gli scostamenti;
  * editor in stile project management: tabella con modifica in linea e predecessori
    scritti come `3;4II+2`, Gantt con trascinamento di barre, durate e legami, scale,
    pannelli dei dettagli e della produzione;
  * report `cronoprogramma` (tabella e produzione mensile, anche nel modello
    tradizionale) e diagramma di Gantt in PDF A3 (`--gantt`);
  * sette comandi nuovi e richiesta `cronoprogramma`. I dati sono salvati nel progetto
    ma non entrano negli export XPWE e DCF.
* **Modello di dominio** (`opencomputo-core`): grandezze decimali esatte, arrotondamenti
  configurabili, formule nei righi di misura, prezzari e analisi, categorie e WBS,
  computi con prezzi fotografati, libretto delle misure, registro e SAL con prospetti
  congelati all'emissione, validazione, schema JSON versionato con migrazioni.
* **Formati PriMus**: import/export XPWE (export sperimentale), lettura dei DCF non
  protetti (elenchi prezzi, capitoli, categorie).
* **Contabilità completa**: anticipazione del prezzo con recupero sul netto progressivo
  (sospendibile o del residuo per singolo SAL), penali, certificato di pagamento con IVA
  congelato all'emissione, sommario del registro per articolo (ADR 0011).
* **Conto finale**: stato finale con le misurazioni non comprese nei SAL, conto finale
  congelato che chiude la contabilità, firma dell'esecutore con riserve, collaudo o
  certificato di regolare esecuzione, certificato della rata di saldo (ADR 0012).
* **Analisi prezzi e nuovi prezzi**: risorse elementari, analisi dinamiche e annidate
  con incidenze per tipo, spese generali e utile predefiniti, fabbisogno di cantiere;
  nuovi prezzi numerati con verbali di concordamento (firma, riserve, approvazione) ed
  esclusione dal ribasso per i prezzi già netti (ADR 0013); variabili nelle analisi,
  oneri aziendali della sicurezza, aggiornamento percentuale dei prezzi, computo con le
  analisi delle voci (ADR 0014). Schema del file versione 5 (poi 6, con il «vedi voce»),
  con migrazione automatica dalle precedenti.
* **Report** di computo, elenco prezzi, libretto, registro, SAL, certificato di
  pagamento, sommario del registro, conto finale, analisi dei prezzi, fabbisogno di
  cantiere, nuovi prezzi e verbali di concordamento in PDF, XLSX e CSV.
* **Stampa nel modello tradizionale** del computo (anche solo metrico) e dell'elenco
  prezzi, solo PDF. Ha frontespizio, riquadro verde e riporti di pagina. Nel computo
  i righi hanno le deduzioni in rosso e ogni voce chiude con «SOMMANO»; i parziali sono
  per super categoria. Nell'elenco il prezzo è anche in lettere (ADR 0015).
* **Export DCF dell'elenco prezzi** (sperimentale): contenitore ricostruito dall'analisi
  dei prezzari, con le somme di controllo e la mappa dei settori, contenuto in chiaro.
  Verificato rileggendo i file e confrontando 31.755 voci di due prezzari reali;
  l'apertura in PriMus è da verificare (ADR 0016). Computo e contabilità restano in
  XPWE.
* **Trascinamento di voci tra finestre**: dall'elenco prezzi o da un prezzario al
  computo, da un computo all'altro (copia, o spostamento con Maiusc), nello stesso
  computo per riordinare. Selezione multipla di voci, viste aperte in nuove finestre con
  Ctrl/⌘+clic sul menu, nuovo comando `copia_voci_computo` (ADR 0017).
* **Modello tradizionale (PDF) per tutti i documenti**:
  * libretto, registro, SAL, certificato, sommario, conto finale, analisi, fabbisogno,
    nuovi prezzi e computo con le analisi;
  * intestazione delle colonne ripetuta a ogni pagina;
  * firma del direttore dei lavori o del responsabile del procedimento;
  * campo `tradizionale` su ogni tipo di report e `--tradizionale` dalla CLI (ADR 0023).
* **Aggiornamento da listino**: confronto voce per voce dell'elenco prezzi e delle
  risorse con un nuovo prezzario, poi aggiornamento selettivo (ADR 0022):
  * accoppiamento per tariffa, anche con la sostituzione del prefisso;
  * differenze, variazione %, effetto sui computi;
  * scelta delle voci e dei campi;
  * ricalcolo delle analisi e riallineamento dei computi in un solo comando
    annullabile (`aggiorna_da_prezzario`, richiesta `confronto_prezzario`);
  * caselle nelle griglie, con Spazio per alternarle.
* **Incidenze di manodopera e sicurezza** nella stampa del computo:
  * per ogni voce, la percentuale e la quota, in colonne aggiuntive anche nel modello
    tradizionale (con i riporti di pagina);
  * quattro stampe a scelta con due caselle, nel riquadro «Incidenze» della pagina
    Report;
  * nel modello tradizionale i titoli delle colonne strette si rimpiccioliscono
    invece di sovrapporsi ai filetti;
  * a scelta, i subtotali nei riepiloghi per categorie e WBS (nel modello
    tradizionale, pagine di riepilogo finale);
  * nel core, il totale della sicurezza compresa nei prezzi e le quote raggruppate;
  * dalla CLI `--manodopera`, `--sicurezza`, `--subtotali` (ADR 0021).
* **Pannello dei filtri** nel computo e nell'elenco prezzi:
  * categorie, WBS e capitoli ad albero;
  * tariffa, descrizione, natura, unità di misura;
  * intervalli di prezzo, quantità, importo e incidenza della manodopera;
  * nell'elenco, l'uso nel computo per categoria e WBS;
  * conteggi e importi per valore;
  * le voci nuove ricevono la categoria filtrata (ADR 0020).
* **«Vedi voce»**: righi di misura che citano un'altra voce del computo e ne seguono la
  quantità (anche moltiplicata per i fattori o in una formula con `vv`); più voci in una
  volta, un rigo per voce. I riferimenti circolari sono rifiutati e la voce eliminata
  lascia i righi statici. Nel libretto il «vedi voce» è statico. L'import XPWE collega i
  righi «vedi voce» quando le quantità coincidono. Le stampe riportano «Vedi voce n.».
  Schema del file versione 6 (ADR 0019).
* **Voci affiancate al computo** (vista divisa): elenco prezzi o prezzario a sinistra,
  con ricerca e selezione multipla; «>» aggiunge le voci al computo dopo quella
  selezionata, «<» toglie le voci selezionate, Invio e doppio clic nella lista come «>»
  (ADR 0018).
* **File di progetto `.ocp`** con salvataggio atomico e copie di sicurezza.
* **Casi d'uso** con 49 comandi JSON, richieste atomiche, rifiuto dei nuovi errori,
  prova senza applicare, annulla/ripristina.
* **Binario `opencomputo`** (CLI con uscita JSON e servizio a righe JSON) per script e
  agenti AI.
* **Applicazione desktop** (Tauri v2): dati generali, elenco prezzi e prezzari di
  consultazione, computo con foglio delle misure da tastiera, copia/incolla di righi anche
  con i fogli di calcolo, categorie e WBS modificabili, libretto con annotazioni, SAL con
  certificato di pagamento, anticipazione e penali, conto finale, analisi prezzi, nuovi
  prezzi,
  report con anteprima, controlli, più finestre, file recenti, tema chiaro e scuro.
* **Pacchetti** `.deb`/`.rpm` (Linux, provati), configurazione per `.dmg` e `.msi`/NSIS;
  associazione dei file `.ocp`.
* **Verifiche**: CI su tre sistemi, `cargo deny` (licenze, vulnerabilità, sorgenti), test
  end-to-end dell'interfaccia con il motore reale.

### Sicurezza

* Aggiornate dipendenze indirette con vulnerabilità note: `zip` 9 (RUSTSEC-2026-0367,
  -0368, -0369), `quick-xml` 0.42 in Tauri (RUSTSEC-2026-0194, -0195), `time` 0.3.55
  (RUSTSEC-2026-0009). Versione minima di Rust portata a 1.88.
* `tokio` 1.54.1 nella GUI (RUSTSEC-2026-0384: corsa critica in `Interval`; la 1.54.0 è
  stata ritirata).
