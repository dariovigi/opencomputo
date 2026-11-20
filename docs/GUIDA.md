# Guida all'uso di OpenComputo

OpenComputo serve a redigere computi metrici estimativi e a tenere la contabilità dei
lavori pubblici: elenco prezzi, computo con righi di misura, libretto delle misure, stati
di avanzamento (SAL), report in PDF, Excel e CSV. Importa i documenti di PriMus in formato
XPWE e gli elenchi prezzi dai file DCF non protetti.

> OpenComputo non è affiliato ad ACCA software e non è equivalente a PriMus. I risultati
> vanno controllati prima di usarli in documenti ufficiali (vedi [Limiti](#limiti)).

## Installazione

* **macOS**: aprire il `.dmg` e trascinare OpenComputo in Applicazioni. Al primo avvio,
  se macOS avvisa che lo sviluppatore non è verificato: clic destro sull'applicazione →
  *Apri*.
* **Windows**: eseguire l'installer `-setup.exe`, che installa per il solo utente e non
  chiede privilegi di amministratore (il `.msi` installa per tutti e li chiede). In
  alternativa `OpenComputo-portatile-x64.exe` si avvia senza installazione, da qualsiasi
  cartella. Se compare *Windows ha protetto il PC*: *Ulteriori informazioni* → *Esegui
  comunque*.
* **Linux**: `sudo apt install ./OpenComputo_…_amd64.deb` (o il `.rpm`).
* **Dai sorgenti**: `cargo run -p opencomputo-gui --release` (vedi README).

I file di progetto hanno estensione **`.ocp`**: dopo l'installazione si aprono con un
doppio clic.

## Primo avvio

La schermata iniziale offre:

* **Nuovo progetto**: chiede il titolo e crea un progetto con un computo vuoto;
* **Apri progetto**: un file `.ocp`;
* **Importa da PriMus**: un file XPWE o DCF (vedi [PriMus](#importare-da-primus));
* l'elenco dei **file recenti** di questo computer (la × lo toglie dall'elenco).

A sinistra la navigazione: *Dati generali*, *Elenco prezzi*, *Categorie, WBS, computi*,
i computi del progetto, *Libretto delle misure*, *Stati di avanzamento*,
*Cronoprogramma*, *Report ed export*, *Controlli*. In basso la barra di stato con il totale del computo corrente e il
numero di errori e avvisi.

## Salvare

* **Salva** (Ctrl+S, ⌘S su Mac); **Salva con nome** dal menu *File* (Ctrl+Maiusc+S).
* Il salvataggio è sicuro: il file viene scritto per intero accanto all'originale e solo
  alla fine lo sostituisce, così un'interruzione non lascia mai un file rovinato.
* Prima di sostituire un file ne viene conservata la versione precedente:
  `progetto.ocp.bak1`, `.bak2`, `.bak3` (la più vecchia viene eliminata). Per recuperarla
  basta rinominarla in `.ocp`.
* Il pallino accanto al titolo indica modifiche non salvate; chiudendo l'applicazione
  viene chiesto se salvarle.

## Annullare

**Annulla** (Ctrl+Z) e **Ripristina** (Ctrl+Y o Ctrl+Maiusc+Z) valgono per qualsiasi
modifica, con fino a 50 passi; passando il mouse sul pulsante si vede cosa verrà
annullato. Dentro un campo di testo Ctrl+Z annulla la digitazione nel campo.

## Dati generali

Titolo, oggetto, committente, impresa, luogo, CIG, CUP, responsabile del procedimento,
progettista, direttore dei lavori; dati del contratto (ribasso d'asta, ritenuta di
garanzia, importo contrattuale, consegna dei lavori, IVA dei certificati di pagamento,
manodopera esclusa dal ribasso); anticipazione del prezzo;
arrotondamenti (decimali di quantità, prezzi e importi, al massimo 10; modalità). Ogni
campo si applica quando lo si lascia.

Dopo l'emissione di un SAL i parametri che lo modificherebbero (per esempio il ribasso)
non sono più modificabili: OpenComputo rifiuta la modifica e lo spiega.

**Anticipazione del prezzo**: percentuale (di norma 20 %, fino al 30 % se previsto dai
documenti di gara), importo erogato (se vuoto si calcola sull'importo contrattuale),
percentuale di recupero sui SAL (se vuota, uguale a quella dell'anticipazione) e data di
erogazione. **Imposta anticipazione** la registra, **Rimuovi** la toglie.

## Elenco prezzi

* La griglia mostra tariffa, descrizione, unità di misura, prezzo, incidenza della
  manodopera, natura (lavoro o oneri della sicurezza) e capitolo. La casella di ricerca
  trova le voci che contengono tutte le parole scritte, nel codice o nelle descrizioni.
* Selezionando una voce, a destra si modificano tutti i campi; **Applica modifiche**.
* **+ Nuova voce**, **Elimina**, **Aggiungi al computo** (anche con doppio clic).
* **Prezzi fotografati**: una voce di computo conserva il prezzo che aveva quando è stata
  inserita. Cambiare il prezzo in elenco non cambia il computo: la voce mostra l'avviso
  *prezzo cambiato in elenco* e il pulsante **Riallinea** nel pannello di dettaglio.

Nel dettaglio di una voce, **Apri l'analisi** (o **Crea l'analisi**) porta all'analisi del
prezzo e **Segna come nuovo prezzo…** ai nuovi prezzi. La colonna *AP/NP* indica le voci
analizzate (AP) e i nuovi prezzi (NP.1, NP.2…).

### Analisi prezzi

La pagina *Analisi prezzi* raccoglie:

* le **risorse elementari** (manodopera, materiali, noli, trasporti, altro) con il loro
  prezzo: **+ Risorsa**, oppure clic su una riga per modificarla o eliminarla se non è
  usata;
* le **voci analizzate**, con il prezzo in elenco e quello che risulta dall'analisi con i
  prezzi correnti. Quando una risorsa cambia prezzo le analisi lo usano subito, ma il
  prezzo in elenco cambia solo con **Aggiorna i prezzi dalle analisi**;
* **Aggiorna prezzi…** applica una variazione percentuale, anche negativa, alle risorse
  (tutte o di un tipo, per esempio solo la manodopera) o alle voci senza analisi (tutte o
  di un capitolo). Le voci analizzate seguono poi le loro analisi. Si annulla con Annulla;
* **spese generali e utile predefiniti** (15 % e 10 % su costo più spese generali, come
  nei prezzari pubblici), proposti alle nuove analisi; **Applica a tutte le analisi…** li
  estende a quelle esistenti.

L'analisi di una voce (**+ Analisi di una voce…**, oppure clic su una voce analizzata) ha
gli elementi in tabella:

* **+ Risorsa**: una risorsa elementare con la sua quantità;
* **+ Voce o analisi**: una voce dell'elenco, o di un prezzario caricato, che viene
  copiata. Se la voce ha a sua volta un'analisi diventa una *sotto-analisi*: il suo prezzo
  segue l'analisi e le incidenze ne comprendono le risorse;
* **+ Riga libera**: codice, descrizione, unità di misura, tipo e prezzo scritti a mano.

La quantità si modifica direttamente nella tabella, anche come espressione (`0,5*1,2`),
e accanto compare il valore calcolato. Nella scheda **Variabili** (**+ Variabile**) si
definiscono valori con un nome, per esempio `spessore = 0,015` e `kg = spessore*1400`; le
quantità possono usarli insieme a `q1`, `q2`…, la quantità delle righe precedenti: il nolo
di una betoniera pari a metà delle ore della riga 1 si scrive `q1/2`. Cambiando una
variabile si ricalcolano tutte le righe che la usano.
Le frecce cambiano l'ordine e × toglie la riga. Sotto si impostano la quantità di
riferimento (es. 100 m²), le spese generali, l'utile e la sua base, e quale parte delle
spese generali è costituita dagli **oneri aziendali della sicurezza**: l'importo è
evidenziato nell'analisi e nelle stampe senza cambiare il prezzo. Il riquadro *Prezzo*
mostra costo diretto, spese generali, utile, totale, prezzo unitario e le incidenze per
tipo di risorsa. Ogni modifica aggiorna il prezzo e l'incidenza della manodopera della
voce in elenco; computi e libretto conservano i prezzi fotografati. Un'analisi che usa
sé stessa, anche indirettamente, viene rifiutata.

### Filtri dell'elenco

**Filtri** apre il pannello dei filtri anche nell'elenco del progetto e nei prezzari di
consultazione, con un filtro per scheda. Le sezioni sono:
* capitolo, ad albero;
* natura e unità di misura;
* prezzo e incidenza della manodopera;
* analisi e nuovi prezzi;
* **uso nel computo** corrente, insieme a super categorie, categorie, sotto categorie e
  WBS con cui la tariffa vi è usata. Esempio: le voci dell'elenco usate nel computo per
  le murature.

I filtri si aggiungono alla ricerca.

### Prezzari di consultazione

Nella scheda *Prezzari di consultazione*, **Carica prezzario…** apre un prezzario
regionale (DCF non protetto, XPWE o un altro progetto `.ocp`) senza copiarlo nel
progetto. Si cercano le voci, se ne selezionano una o più (Maiusc o Ctrl per la selezione
multipla) e **Copia nell'elenco** le aggiunge all'elenco prezzi del progetto con i loro
capitoli. Le voci già presenti sono mostrate in grigio. Anche il selettore delle voci del
computo può pescare direttamente da un prezzario caricato.

I prezzari distribuiti da ACCA hanno condizioni d'uso proprie: rispettarle è
responsabilità di chi li importa.

### Listini CSV e JSON

Un prezzario si può caricare anche da un listino **CSV** o **JSON**, per esempio
esportato da un foglio di calcolo, dalla finestra *Carica prezzario…* o dalla riga di
comando:

```sh
opencomputo cerca progetto.ocp "massetto" --prezzario listino.csv
opencomputo esegui progetto.ocp --prezzario listino=listino.csv < comandi.json
```

* **CSV**: la prima riga è l'intestazione. Servono le colonne codice (o tariffa),
  descrizione, unità di misura (anche «U.M.» o «UM») e prezzo; facoltative descrizione
  breve, capitolo e descrizione del capitolo, natura («oneri della sicurezza»), incidenza
  della manodopera e della sicurezza. Il separatore (`;`, `,` o tabulazione) e la virgola
  o il punto decimale si riconoscono da soli. Testo in UTF-8 o Windows-1252.
* **JSON**: un oggetto con `voci` (e, facoltativi, `nome`, `ente`, `edizione`,
  `data_pubblicazione`, `capitoli`) oppure un elenco di voci. Ogni voce ha `codice`,
  `descrizione`, `unita_misura`, `prezzo`.
* Le righe con codice, descrizione o unità vuoti, prezzo non valido o negativo, o un
  codice già presente vengono **scartate e segnalate** con il numero di riga; un prezzo
  zero o un'incidenza fuori da 0–100 sono segnalati ma la voce resta. I capitoli figli
  (`M.01` sotto `M`) si riconoscono dal codice.

### Aggiornamento da listino

Per portare l'elenco prezzi a un listino nuovo, per esempio il prezzario regionale
dell'anno successivo (ADR 0022), si apre **Aggiornamento da listino**: dal menu, da
**Aggiorna da listino…** nell'elenco prezzi o da **Aggiorna l'elenco da questo…** in un
prezzario di consultazione.

1. **Listino**: si sceglie un prezzario caricato o si carica il nuovo con **Carica
   listino…** (DCF, XPWE o progetto).
2. **Prefisso**: se il listino cambia il prefisso dei codici a ogni edizione, si indica
   la sostituzione, es. `TOS23_` → `TOS24_`. Le voci si accoppiano per tariffa.
3. **Confronto**. Per ogni voce dell'elenco si vedono:
   * il prezzo attuale e quello del listino, con la variazione % (rossa se aumenta,
     verde se diminuisce);
   * i dati diversi: P prezzo, I incidenze, D descrizioni, UM unità di misura, T
     tariffa;
   * la differenza degli importi nei computi (Δ computi);
   * l'esito: variata, invariata, non trovata, con analisi.

   Selezionando una riga, a destra compare il confronto campo per campo, con il testo
   delle descrizioni. La scheda **Risorse delle analisi** confronta le risorse con le
   risorse o le voci del listino.
4. **Scelta**: la casella di ogni riga (o Spazio sulle righe selezionate) decide cosa
   aggiornare. All'inizio sono scelte le voci con differenze nei campi da aggiornare.
   * I filtri restringono la lista, es. le voci variate più del 5 % o quelle usate nei
     computi. **Scegli le visibili**, **Solo le variate** e **Nessuna** agiscono sulle
     righe filtrate.
   * Le voci non trovate e quelle con analisi non si possono scegliere: il prezzo di
     una voce analizzata segue l'analisi, quindi si aggiornano le sue risorse.
5. **Aggiorna**: si scelgono i campi (prezzo e incidenze in modo predefinito;
   descrizioni, unità di misura e, con il prefisso, la tariffa). Si sceglie anche se
   ricalcolare le analisi e riallineare i computi.

   In basso compaiono i totali dei computi prima e dopo, calcolati dal motore senza
   applicare nulla. **Aggiorna le voci scelte** applica tutto in un solo passo,
   annullabile. La contabilità non cambia: le registrazioni conservano il loro prezzo.

## Computo

Il computo è un foglio delle misure: per ogni voce (tariffa, descrizione, quantità,
prezzo, importo) i righi di misura con descrizione, parti uguali, lunghezza, larghezza,
altezza o peso, formula e quantità.

### Inserire

* **+ Voce** apre il selettore: si scrive parte del codice o della descrizione, si sceglie
  con le frecce e Invio. La voce viene inserita dopo quella selezionata, con un rigo vuoto
  già pronto.
* Sotto ogni voce la riga *+ nuovo rigo*: basta scriverci per aggiungere un rigo.

### Tastiera

| Tasto | Effetto |
|-------|---------|
| Frecce | spostarsi tra le celle |
| Invio o F2 | modificare la cella; Invio conferma e scende |
| Tab / Maiusc+Tab | confermare e passare alla cella successiva / precedente |
| Esc | annullare la modifica in corso |
| un carattere qualsiasi | iniziare a scrivere nella cella sostituendone il contenuto |
| Canc | svuotare la cella |
| Ins o Ctrl+Invio | nuovo rigo sotto quello attivo |
| Ctrl+Canc | eliminare il rigo (o i righi selezionati) |
| Maiusc+Su/Giù, Maiusc+clic | selezionare più righi della stessa voce |
| Ctrl+C / Ctrl+V | copiare / incollare righi (vedi sotto) |

### Fattori, espressioni e formule

* I fattori accettano numeri con la virgola (`1,20`) e **espressioni**: `7,50+0,30`,
  `2*(3,50+0,30)`. Il valore calcolato compare nella quantità del rigo.
* La **formula** sostituisce il prodotto dei fattori e usa le variabili `par`, `lun`,
  `lar`, `alt`: per esempio `0,5*lun*lar*alt` per un volume triangolare.
* **Deduzione**: il rigo viene sottratto (in rosso, quantità negativa).
  **Commento**: il rigo è solo descrittivo.
* Le quantità sono arrotondate per rigo, gli importi per voce, secondo i decimali dei
  dati generali.

### Copiare e incollare

* **Ctrl+C** copia il rigo attivo o i righi selezionati. Incollati in Excel o LibreOffice
  diventano colonne (descrizione, parti uguali, lunghezza, larghezza, altezza, formula).
* **Ctrl+V** incolla sotto il rigo attivo, a partire dalla colonna attiva: righi copiati in
  OpenComputo (con deduzioni e commenti) oppure celle copiate da un foglio di calcolo. I
  numeri con il punto delle migliaia (`1.234,5`) vengono riconosciuti.
* Un singolo valore incollato sostituisce il contenuto della cella attiva.

### Voci

**↑ ↓** spostano la voce selezionata, **Elimina voce** la rimuove (con conferma). Il
pannello **Dettaglio** mostra la descrizione estesa, il prezzo, e permette di assegnare
super categoria, categoria, sotto categoria, nodo WBS e note.

### Più finestre

Il pulsante ⧉ in alto apre la vista corrente in una nuova finestra (per esempio l'elenco
prezzi accanto al computo). Ctrl+clic (⌘+clic su Mac) o clic centrale su una voce del
menu apre direttamente quella vista, per esempio un secondo computo. Tutte le finestre
lavorano sullo stesso progetto e si aggiornano a ogni modifica.

### Trascinare voci

Le voci si trascinano con il mouse, anche da una finestra all'altra.

* **Dall'elenco prezzi a un computo**: le voci selezionate si aggiungono nel punto di
  rilascio, ognuna con un rigo vuoto. Nell'elenco si selezionano più voci con Maiusc
  (intervallo) o Ctrl/⌘ (una alla volta).
* **Da un prezzario di consultazione a un computo**: come sopra. Le voci che mancano
  nell'elenco prezzi vengono prima copiate nell'elenco.
* **Da un prezzario all'elenco del progetto** (aperto in un'altra finestra): le voci
  vengono copiate nell'elenco.
* **Da un computo all'altro**: si trascina la testata della voce. Più voci si
  selezionano con Ctrl/⌘+clic o Maiusc+clic sulle testate.
  * Le voci si copiano con righi, prezzo applicato, categorie, WBS e note; con Maiusc
    premuto al rilascio si spostano.
* **Nello stesso computo**: le voci si spostano; con Alt o Ctrl premuto al rilascio si
  copiano.

Una linea indica dove finiranno le voci: sopra la metà della testata di una voce si
inseriscono prima di quella voce, altrove dopo. Ogni trascinamento si annulla con un
solo «Annulla». Le voci selezionate nel computo si eliminano insieme con «Elimina voce».

### Filtri

**Filtri** apre a sinistra del computo il pannello dei filtri:
* **super categorie, categorie e sotto categorie**, ognuna con «non assegnata»;
* **WBS** e **capitolo dell'elenco**, ad albero: un nodo comprende quelli sotto;
* **tariffa** e **descrizione**: scrivendo, restano le voci che contengono tutte le parole;
* **natura**, **unità di misura**, **stato del prezzo** rispetto all'elenco;
* **prezzo**, **quantità** e **importo**: da… a….

Comportamento:
* **Combinazione.** Più valori della stessa sezione si sommano (una qualsiasi), sezioni
  diverse si combinano (tutte).
* **Conteggi.** Accanto a ogni valore ci sono il numero di voci e l'importo che
  resterebbero scegliendolo.
* **Foglio.** Mostra solo le voci filtrate, con i loro numeri d'ordine. Sopra compare la
  barra «Filtro attivo» con **Azzera**; in basso, il totale delle voci filtrate.
* **Voci nuove.** Con una sola categoria (o un solo nodo WBS) scelta, una voce aggiunta
  la riceve, e la notifica lo dice.
* **Persistenza.** I filtri restano attivi anche chiudendo il pannello, finché non si
  azzerano.

### Vedi voce

Un rigo può partire dalla quantità di un'altra voce dello stesso computo, e la segue:
se cambia la voce citata, cambia anche il rigo.

* Si attiva una cella della voce in cui inserire i righi e si preme **Vedi voce…**.
  Si scelgono una o più voci con Maiusc o Ctrl/⌘ (c'è la ricerca); con **Inserisci**
  ogni voce scelta diventa un rigo, sotto il rigo attivo. **Righi in deduzione** li
  sottrae.
* Non sono proposte la voce stessa e le voci che la citano già: sarebbe un riferimento
  circolare.
* Nel rigo il riferimento compare prima della descrizione, per esempio «Vedi voce n. 1
  [m³ 126,00]». Un clic porta alla voce citata; il dettaglio di una voce elenca le voci
  che la citano.
* I fattori del rigo moltiplicano la quantità citata: con parti uguali 0,5 il rigo vale
  metà della voce. Una formula usa la quantità citata con `vv`, per esempio `vv/2`.
* Spostare la voce citata non cambia nulla. Se la si elimina, o si copia il rigo in un
  altro computo, il rigo conserva la quantità che aveva e diventa statico (bordo
  tratteggiato).
* Copiando insieme la voce citata e quella che la cita, la copia cita la copia.
* **Nel libretto delle misure** «Vedi voce…» chiede anche il computo e copia la
  quantità attuale delle voci scelte: nel libretto il «vedi voce» è sempre statico.
* Nelle stampe il rigo riporta il riferimento, con la quantità citata.

### Voci affiancate

**Affianca voci** apre a sinistra del computo l'elenco prezzi del progetto o un
prezzario di consultazione caricato (si sceglie in alto nel pannello), con la ricerca.

* **>** aggiunge al computo le voci selezionate a sinistra, ognuna con un rigo vuoto,
  dopo la voce selezionata nel computo (o in fondo). Le voci aggiunte restano
  selezionate: il «>» successivo aggiunge dopo di loro.
* Nella lista **Invio** o il doppio clic fanno lo stesso. Il fuoco resta nella lista: si
  cerca, si scende con ↓, si sceglie con Invio, e così via. Le voci si possono anche
  trascinare nel foglio.
* Da un prezzario, le voci che mancano vengono prima copiate nell'elenco prezzi.
* **<** toglie dal computo le voci selezionate (Ctrl/⌘+clic o Maiusc+clic sulle testate
  per più voci), senza conferma: «Annulla» nella notifica le rimette.
* Un pallino segna le tariffe già usate nel computo. Nei prezzari le voci già
  nell'elenco del progetto sono in grigio.
* La colonna dei pulsanti si trascina per allargare o restringere il pannello. Il
  pannello resta aperto, con la sua larghezza, finché non si ripreme «Affianca voci».

## Categorie, WBS e computi

Si aggiungono, modificano (✎) e rimuovono (×) super categorie, categorie, sotto categorie
e nodi della WBS. Una categoria o un nodo assegnati a qualche voce non si possono
rimuovere finché non li si toglie dalle voci. Da qui si creano anche altri computi (per
esempio una perizia di variante).

## Contabilità

### Nuovi prezzi

Per le lavorazioni non previste nel contratto (Allegato II.14 al D.Lgs. 36/2023, art. 5):

1. **+ Nuovo prezzo** crea la voce in elenco e la numera NP.1, NP.2… Chiede tariffa,
   descrizione, unità di misura, criterio di formazione (da analisi, per analogia con
   prezzi contrattuali, dal prezzario), data, motivo e se il prezzo è **già netto**. Con il
   criterio «analisi» si apre subito l'editor dell'analisi.
2. **Da una voce dell'elenco…** segna come nuovo prezzo una voce già presente.
3. **+ Verbale** registra il verbale di concordamento:
   * data e nuovi prezzi compresi;
   * firma dell'esecutore ed eventuali riserve;
   * approvazione (RUP o stazione appaltante) con l'atto.

   **Stampa** produce il verbale con le firme e le analisi dei prezzi che ne derivano.
4. **Ribasso**: di norma un nuovo prezzo è soggetto al ribasso d'asta. Un prezzo «già
   netto» è escluso dal ribasso nei SAL e nel conto finale (riga «di cui nuovi prezzi
   esclusi dal ribasso»). Cambiando l'opzione, le misure non ancora in un SAL emesso si
   adeguano.

I *Controlli* segnalano i nuovi prezzi usati nel libretto ma non ancora approvati, e i
verbali non firmati o con riserve. Un verbale approvato non si elimina e i suoi nuovi
prezzi non si tolgono.

### Libretto delle misure

**+ Registrazione** chiede la data e la voce, poi si scrivono i righi come nel computo.
Nel dettaglio si scrivono le annotazioni. **Cambia data…** ed **Elimina registrazione**
agiscono sulla registrazione selezionata.

Le registrazioni comprese in un SAL emesso sono in sola lettura: OpenComputo impedisce di
modificarle.

### Stati di avanzamento

1. Impostare il contratto nei *Dati generali* (ribasso, ritenuta).
2. **+ Nuovo SAL** con la data *lavori a tutto il*: il SAL in bozza comprende le
   registrazioni non ancora contabilizzate fino a quella data e mostra il prospetto
   (lavori, sicurezza, ribasso, netto, ritenuta, credito, certificati precedenti, importo).
3. **Emetti SAL…** con la data di emissione: il prospetto viene congelato e le
   registrazioni bloccate. Subito dopo, Annulla può ancora tornare indietro.
4. **Annulla bozza** elimina un SAL non emesso.

### Certificato di pagamento

Sotto il prospetto del SAL scelto c'è il certificato di pagamento: importo del SAL, a
dedurre il recupero dell'anticipazione e le penali, imponibile, IVA (se indicata nel
contratto) e totale. Il recupero è calcolato sull'importo netto progressivo, quindi i
certificati si riallineano da soli. Per il SAL in bozza si può:

* **sospendere il recupero** in questo certificato (per esempio quando la stazione
  appaltante lo concede: D.L. 107/2026, art. 1-bis, fino al 31/12/2026, esclusi i lavori
  PNRR); il certificato successivo recupera anche la parte non recuperata;
* **recuperare tutta l'anticipazione residua**, di norma nell'ultimo SAL.

All'emissione del SAL il certificato viene congelato. I SAL emessi con versioni
precedenti di OpenComputo mostrano il certificato ricalcolato. **Anteprima e stampa del
certificato** apre il report.

### Penali

**+ Penale** chiede data, motivo e giorni di ritardo con il per mille giornaliero
(art. 126: dallo 0,5 all'1,5 per mille dell'importo contrattuale), oppure direttamente
l'importo. La penale va nel primo certificato il cui SAL arriva alla sua data. Sotto
l'elenco compare il totale rispetto al limite del 10 %; i *Controlli* segnalano per mille
fuori misura e penali oltre il limite. Una penale si può rimuovere finché il suo SAL non
è emesso.

### Conto finale

Dopo l'ultimazione dei lavori, la schermata *Conto finale* chiude la contabilità.

1. **Redigi il conto finale…** chiede la data di ultimazione e la data del conto. Prima
   bisogna emettere o annullare l'eventuale SAL in bozza. Le registrazioni del libretto
   non comprese in un SAL formano lo *stato finale* e nel libretto sono segnate
   «stato finale». Finché il conto è in bozza non si redigono altri SAL, ma si possono
   ancora aggiungere o correggere misurazioni.
2. La schermata mostra:
   * lo stato finale (lavori, sicurezza, ribasso, importo netto finale);
   * gli acconti già certificati con i SAL;
   * il credito residuo dell'esecutore, con le ritenute di garanzia da svincolare;
   * il certificato della rata di saldo. Dal credito residuo si deducono
     l'anticipazione non ancora recuperata e le penali non applicate nei
     certificati, poi si aggiunge l'IVA.
3. **Emetti conto finale…** congela gli importi e chiude la contabilità: non si possono
   più aggiungere misurazioni, SAL o penali. Subito dopo, Annulla può ancora tornare
   indietro.
4. **Firma e collaudo**:
   * firma: data della firma dell'esecutore e testo delle riserve confermate.
     L'esecutore firma entro trenta giorni dall'invito del RUP e non può aggiungere
     riserve nuove;
   * collaudo: tipo (certificato di regolare esecuzione o di collaudo), data e
     approvazione. Con l'approvazione la rata di saldo risulta «pagabile».

   I *Controlli* segnalano i termini: certificato di regolare esecuzione entro tre
   mesi dall'ultimazione, collaudo entro sei mesi (un anno nei casi complessi).

**Anteprima e stampa** apre il report del conto finale.

## Cronoprogramma

Nella sezione **Pianificazione** della barra laterale, **Cronoprogramma** pianifica i
tempi dei lavori sulla WBS: ogni nodo della WBS è un'attività. I dati sono salvati nel
file del progetto, ma non entrano negli export XPWE e DCF.

### Inizio e calendario

**Calendario e inizio…** apre una finestra con:
* l'inizio dei lavori (se manca vale la consegna dei lavori dei dati generali);
* i giorni lavorativi della settimana (in modo predefinito da lunedì a venerdì);
* le festività nazionali (con Pasqua e il lunedì dell'Angelo), lavorative o no;
* le chiusure del cantiere (ferie, fermo invernale), dal giorno al giorno;
* il computo da cui prendere gli importi della produzione mensile.

Le durate e i ritardi sono in giorni lavorativi. In alto si leggono inizio, fine,
durata in giorni lavorativi e naturali e l'ultimazione contrattuale (consegna più tempo
utile). Se la fine prevista la supera compare un avviso.

### Tabella delle attività

A sinistra la tabella: numero, WBS, nome, durata, inizio, fine, predecessori,
avanzamento e margine. I riepiloghi (nodi con figli) sono in grassetto e si comprimono
con ▾ o con le frecce ← e →. Il divisore tra tabella e diagramma si trascina.

Per modificare una cella: doppio clic, oppure F2 o Invio sulla durata. Si può anche
scrivere direttamente una cifra per la durata. Invio conferma e passa alla riga sotto,
Esc annulla.
* **Durata**: giorni lavorativi. Vuota significa «da stimare»: vale un giorno e
  appare come «1 g?».
* **Inizio**: vincolo «inizio non prima del» (gg/mm/aa); vuoto, l'attività parte appena
  possibile. Le attività vincolate hanno la bandierina ⚑.
* **Predecessori**: i numeri delle righe separati da punto e virgola, con il tipo e il
  ritardo facoltativi. Per esempio `3;4II+2;5FF-1` significa: dopo la fine della 3;
  2 giorni dopo l'inizio della 4; finire un giorno prima della fine della 5. I tipi sono
  FI fine-inizio (predefinito), II inizio-inizio, FF fine-fine, IF inizio-fine.
* **%**: avanzamento dei lavori.

Un legame che chiuderebbe un ciclo è rifiutato, con le attività coinvolte. Un legame su
un riepilogo vale per tutte le attività che contiene.

La barra degli strumenti offre:
* **+ Attività** e **+ Pietra miliare** (o il tasto Ins) per le attività che la WBS non
  ha, come l'allestimento del cantiere o l'ultimazione dei lavori. Si aggiungono sotto un
  nodo o alla radice e, a scelta, dopo un'altra attività. **Elimina** (Canc) le toglie;
  i nodi WBS si tolgono da «Categorie, WBS, computi».
* **Collega**: con più righe selezionate (Ctrl/⌘+clic, Maiusc+clic) le collega in
  sequenza, fine-inizio. **Scollega** toglie i legami tra le righe selezionate.

### Livello di dettaglio

**Dettaglio** sceglie fino a quale livello della WBS pianificare. A un livello ridotto i
nodi che contengono altro diventano attività semplici:
* si può dare loro una durata propria, per un cronoprogramma sommario;
* senza durata propria valgono quanto il loro riepilogo nel piano dettagliato.

I legami delle attività nascoste passano al nodo visibile. **Tutti i livelli** torna al
dettaglio completo; i dati di ogni livello restano.

### Diagramma di Gantt

A destra il diagramma, con scorrimento allineato alla tabella:
* le barre blu sono le attività, quelle rosse il percorso critico (attività senza
  margine). La linea tratteggiata dopo una barra è il margine;
* le barre nere con i becchi sono i riepiloghi, i rombi le pietre miliari;
* le barre grigie sottili sono la linea di base. La parte scura di una barra è
  l'avanzamento;
* le colonne grigie sono i giorni non lavorativi. La linea verde è oggi, quella rossa
  tratteggiata l'ultimazione contrattuale.

Con il mouse:
* trascinando una barra in orizzontale si sposta l'attività (diventa il vincolo
  «inizio non prima del»);
* trascinando il bordo destro si cambia la durata;
* trascinando una barra in verticale su un'altra si crea un legame fine-inizio;
* un clic su una freccia seleziona il legame: Canc lo toglie, il doppio clic ne cambia
  tipo e ritardo;
* il doppio clic su una barra apre i dettagli.

**Giorni**, **Settimane**, **Mesi** e **Adatta** cambiano la scala; **Oggi** porta il
diagramma alla data odierna. **Percorso critico** e **Linea di base** mostrano o
nascondono queste informazioni.

### Dettagli e produzione mensile

Sotto il diagramma, due schede:
* **Dettagli dell'attività**: nome (per le attività aggiunte), durata, vincolo,
  avanzamento, note, pietra miliare; date al più presto e al più tardi, margine, linea
  di base con lo scostamento, importo; predecessori e successori con tipo e ritardo
  modificabili, e l'aggiunta di un predecessore.
* **Produzione mensile**: importi del computo distribuiti sui giorni lavorativi di ogni
  attività, mese per mese, con il cumulato e la percentuale. Il grafico mostra le
  colonne mensili e la curva cumulata (curva a S). Le voci senza WBS non si
  distribuiscono e sono indicate a parte.

### Linea di base

**Salva linea di base** fotografa le date attuali di tutte le attività. Da quel momento
la tabella di stampa riporta lo scostamento della fine e il diagramma le barre di base.
Il pulsante, che riporta la data della linea di base, permette di salvarne una nuova o
di toglierla.

**Stampa…** apre la pagina dei report sul cronoprogramma. Ogni modifica si annulla con
Ctrl/⌘+Z.

## Report ed export

Si sceglie il documento (computo, elenco prezzi, libretto, registro di contabilità, SAL,
certificato di pagamento, sommario del registro con quantità e importi per articolo e per SAL,
conto finale e rata di saldo, computo con le analisi delle voci riportate alle quantità
computate, analisi dei prezzi completa o in quadro sintetico,
fabbisogno di cantiere del computo con le quantità di manodopera, materiali e noli
ricavate dalle analisi, elenco dei nuovi prezzi o verbale di concordamento,
cronoprogramma dei lavori)
e la data di stampa; a destra l'anteprima mostra esattamente il contenuto dei file.

* **PDF…** e **Excel (XLSX)…** chiedono dove salvare il file; la notifica offre **Apri**.
* **CSV…** chiede una cartella: un file per sezione, con il punto e virgola e la virgola
  decimale (si apre direttamente in Excel in italiano). Un testo che comincia con `=`,
  `+`, `@` o `-` (non numerico) è preceduto da un apostrofo, perché il foglio di calcolo
  non lo esegua come formula.
* Le stampe PDF usano i caratteri standard: le lettere fuori dall'alfabeto latino
  occidentale (per esempio ő, ł, č, greco, cinese) e gli emoji escono come «?».
* **PriMus XPWE (sperimentale)…** esporta il computo per PriMus (vedi sotto).
* **PriMus DCF (sperimentale)…**, con l'elenco prezzi scelto, esporta l'elenco prezzi in
  DCF (vedi sotto).
* Per il **cronoprogramma** la casella **Diagramma di Gantt (PDF)** produce il diagramma
  in A3 orizzontale: tabella delle attività, barre sul calendario, legami, percorso
  critico e linea di base, su più pagine se le attività sono molte. Senza la casella si
  ottiene la tabella delle attività con la produzione mensile, anche in XLSX e CSV e
  nel modello tradizionale. Dalla riga di comando: `opencomputo report progetto.ocp
  cronoprogramma --gantt`.

### Incidenze di manodopera e sicurezza

Per il computo, nel riquadro **Incidenze** subito sotto la scelta del documento, tre
caselle aggiungono le incidenze della manodopera e della sicurezza compresa nei prezzi
(ADR 0021). Con le prime due si ottengono le quattro stampe: senza incidenze, con la
manodopera, con la sicurezza, con entrambe, sia nel modello ordinario sia nel
tradizionale.
* **Incidenza della manodopera** e **Incidenza della sicurezza**: per ogni voce, la
  percentuale del prezzo e la quota dell'importo, in colonne aggiuntive sulla riga
  «SOMMANO».
  * Nel modello tradizionale le colonne «MANODOPERA» e «SICUREZZA» (euro e incid.%)
    stanno a destra degli importi.
  * Le misure e la designazione si stringono: con una quota la designazione è larga
    circa i due terzi, con due circa la metà.
  * Le quote hanno i loro riporti di pagina e compaiono anche nei parziali e nel totale.

  Se una voce non ha l'incidenza compare «—», e una nota avverte che la quota è
  sottostimata.
* **Subtotali nel riepilogo finale** (con almeno una delle due):
  * nel modello ordinario, i riepiloghi per super categorie, categorie, sotto
    categorie e WBS aggiungono la quota e la sua incidenza sull'importo del gruppo;
  * nel modello tradizionale, dopo il totale si aggiungono le pagine «RIEPILOGO» con
    gli stessi gruppi e il quadro generale. La firma va in fondo al riepilogo.

Le incidenze non si offrono per il computo con le analisi e per il computo metrico
senza prezzi. Le scelte restano memorizzate. Dalla riga di comando: `--manodopera`,
`--sicurezza`, `--subtotali`.

### Modello tradizionale

Per il computo e l'elenco prezzi la casella **Modello tradizionale (PDF)** produce la
stampa nell'impaginazione in uso negli studi tecnici (ADR 0015):
* frontespizio con l'ente, il titolo, l'oggetto, il committente, luogo e data e il
  tecnico (il progettista dei dati generali);
* tabella in un riquadro verde, con le righe «R I P O R T O» e «A R I P O R T A R E»
  su ogni pagina. Quando una voce continua nella pagina successiva si riporta anche la
  sua quantità parziale.
* nel computo, deduzioni in rosso con «Sommano positivi/negativi», «SOMMANO» per ogni
  voce, un gruppo per super categoria con il suo parziale, il totale e la firma;
* nell'elenco prezzi, il prezzo anche in lettere, es. «euro (otto/50)».

Opzioni:
* **Senza prezzi (computo metrico)**: solo le quantità.
* **Frontespizio**: si può togliere.
* **Ente** e **Studio tecnico**: due o tre righe ciascuno. L'ente va nel riquadro in
  alto del frontespizio; se manca, compare il luogo dei lavori. Lo studio va in testa a
  ogni pagina e in basso nel frontespizio. Restano memorizzati per le stampe successive.

Nel computo le descrizioni oltre i 300 caratteri sono ridotte all'inizio e alla fine,
separati da « ... »; dalla CLI `--descrizioni-complete` le stampa per intero.

Il modello tradizionale è solo PDF: XLSX e CSV restano disponibili con il modello
ordinario. Con il modello tradizionale **l'anteprima mostra le pagine del PDF**, con la
stessa impaginazione (frontespizio, riquadro verde, riporti): le prime sei subito, le
altre con «Mostra altre pagine». L'anteprima si aggiorna cambiando le opzioni, l'ente e
lo studio. A video il carattere può essere leggermente diverso da quello del PDF (dipende
da quelli installati), ma righe, colonne e a capo sono gli stessi.

**Altri documenti** (ADR 0023). La casella c'è per tutti i documenti: libretto,
registro, SAL, certificato di pagamento, sommario, conto finale, analisi dei prezzi,
fabbisogno, nuovi prezzi e verbali, computo con le analisi. Hanno la stessa veste:
* frontespizio, testata dello studio, riquadro verde;
* una tabella per sezione, con l'intestazione delle colonne ripetuta a ogni pagina;
* la firma in fondo: il direttore dei lavori per i documenti contabili e i nuovi
  prezzi, il responsabile del procedimento per il certificato, altrimenti il
  progettista. Il nome viene dai dati generali.

Le incidenze di manodopera e sicurezza del computo si stampano anche nel modello
tradizionale.

Dalla riga di comando, per esempio:

```
opencomputo report progetto.ocp computo --tradizionale --ente 'Comune di …\nProvincia di …' --studio 'Studio tecnico …'
opencomputo report progetto.ocp elenco-prezzi --tradizionale --senza-frontespizio
opencomputo report progetto.ocp computo --tradizionale --manodopera --sicurezza --subtotali
opencomputo report progetto.ocp sal --numero 2 --tradizionale
```

## Importare da PriMus

* **XPWE**: elenco prezzi, capitoli, categorie, computo con righi di misura. Quello che
  OpenComputo non usa viene conservato e riscritto nell'export. Dopo l'import un
  riepilogo indica le voci lette e le eventuali segnalazioni.
  Le **analisi dei prezzi** del file entrano nella vista «Analisi prezzi», collegate alle
  loro voci. Il file XPWE non contiene le percentuali di spese generali e utile né il tipo
  di ogni componente: OpenComputo li deduce (le percentuali che riproducono il prezzo del
  file, provando prima spese generali dal 13 al 17 % con utile del 10 % e poi senza utile;
  i tipi che riproducono le incidenze di materiali, manodopera e attrezzature del file) e
  il riepilogo dice che cosa ha dedotto: conviene controllarlo. Il prezzo della voce resta
  sempre quello del file; se OpenComputo, che arrotonda ogni componente, ottiene qualche
  centesimo di differenza, la voce è segnata «da allineare» e si decide dalla vista
  «Analisi prezzi» se applicare il prezzo dell'analisi. L'export XPWE riporta l'analisi
  com'era nel file, non le modifiche fatte in OpenComputo.
* **DCF**: solo i contenuti non protetti (di norma elenchi prezzi, capitoli e categorie).
  I file protetti o con password non vengono letti: in quel caso esportare in XPWE da
  PriMus.
* **Export XPWE**: sperimentale. La struttura delle voci è ricostruita da documentazione
  pubblica e non è ancora stata confrontata con un export reale di PriMus: controllare il
  file in PriMus prima di consegnarlo.
* **Export DCF dell'elenco prezzi**: sperimentale. Il file ha la struttura dei prezzari
  distribuiti in DCF, in chiaro. Contiene voci, capitoli su tre livelli, categorie e
  dati generali; non contiene le analisi.
  * Il computo e la contabilità non si esportano in DCF: nei documenti di PriMus sono
    protetti. Per il computo usare l'export XPWE.
  * **PriMus non apre i file prodotti**: alla prova del 2026-10-21 ha rifiutato anche
    un prezzario originale riscritto da OpenComputo. Per portare i dati in PriMus usare
    l'export XPWE.
  * Dalla riga di comando: `opencomputo esporta progetto.ocp elenco.dcf`.

## Controlli

La sezione *Controlli* elenca errori, avvisi e informazioni del progetto (per esempio un
rigo senza valori, un prezzo cambiato in elenco). Un clic porta alla voce interessata.
OpenComputo rifiuta in ogni caso le modifiche che introdurrebbero un errore e ne mostra
il motivo, lasciando il progetto com'era.

## Assistente AI

L'assistente propone voci di computo a partire da una descrizione in linguaggio naturale
(«tinteggiatura di due pareti di 4 x 3 m»), scegliendo solo voci del prezzario caricato.
Si usa dalla sezione **Assistente AI** della barra laterale, dalla riga di comando e dal
servizio JSON.

**I dati inviati escono dal computer.** L'assistente usa due servizi esterni:

* **OpenAI** (embedding) riceve il testo delle voci del prezzario (codice, descrizioni,
  capitoli), per la ricerca per significato, e il testo delle ricerche;
* **Anthropic** (generazione) riceve la richiesta e la descrizione, l'unità di misura e i
  capitoli delle voci candidate, senza codici né prezzi.

Non vengono inviati prezzi, importi, dati del committente né il file del progetto.

Prezzi, quantità e importi non li sceglie né li calcola il modello: li calcola
OpenComputo, con le stesse regole e gli stessi controlli di sempre. Se il prezzario
viene da un DCF distribuito da ACCA, le sue condizioni d'uso ne vietano l'uso con altri
software: verificare di avere il diritto di inviarne il contenuto.

### Nell'applicazione

La vista **Assistente AI** ha tre schede.

* **Proposta**. Al primo uso mostra il testo del consenso: va letto e accettato
  (casella «Ho letto…», poi **Accetto**); senza consenso l'assistente non chiama nessun
  servizio. Poi si sceglie il prezzario (o lo si carica con **Carica prezzario…**:
  DCF non protetto, XPWE, listino CSV o JSON) e il computo di destinazione, si scrive la
  richiesta e si preme **Proponi** (o Ctrl+Invio). Durante l'attesa si può continuare a
  lavorare nelle altre viste e finestre, oppure premere **Annulla**.
* **Prezzari e indice**: per ogni prezzario caricato il numero di voci indicizzate e il
  tipo di ricerca (per parole, o ibrida per parole e significato). **Indicizza** chiede
  conferma, dicendo quante voci verranno inviate a OpenAI, e mostra l'avanzamento; si
  può annullare e riprendere in seguito.
* **Impostazioni**: consenso (con la data e la possibilità di revocarlo), chiavi API di
  OpenAI e Anthropic (si incollano nel campo e si salvano nel portachiavi del sistema;
  dopo il salvataggio non sono più visibili), modelli e limiti di token.

**Annulla** ferma subito l'operazione in corso, anche se il provider non ha ancora
risposto; l'indicizzazione annullata o interrotta riprende dal punto in cui era. Durante
l'indicizzazione di un prezzario si può continuare a cercare e a chiedere proposte: usano
la ricerca per parole finché l'indice non è pronto. Se una richiesta fallisce o viene
annullata, resta la proposta precedente. Se il provider è sovraccarico o la rete cade,
l'assistente ritenta da solo alcune volte prima di mostrare l'errore. Una richiesta può
avere al massimo 20.000 caratteri.

Se il file delle impostazioni dell'assistente risulta danneggiato, viene messo da parte
(`assistente.json.danneggiato`), tornano le impostazioni predefinite e un avviso nella
vista chiede di ridare il consenso; le chiavi nel portachiavi non vengono toccate.

La proposta elenca le **lavorazioni** riconosciute nella richiesta. Per ciascuna:

* le **misure** (righi con forma e parametri), comuni a tutte le sue voci, in campi
  modificabili: i valori presi dalla richiesta, quelli **ipotizzati** dall'assistente (in
  arancione: il suggerimento dice quale ipotesi) e quelli **da completare** (bordo
  tratteggiato rosso), che si possono scrivere o correggere;
* le voci del prezzario possibili, al massimo cinque, **dalla più alla meno pertinente**
  (pertinenza alta, media, bassa) con il motivo di ognuna e quantità, prezzo e importo
  **calcolati da OpenComputo** con le misure correnti:
  * **Alternative proposte**, quando la richiesta basta a capire la voce: la più
    pertinente è già spuntata;
  * **Opzioni possibili**, quando manca un'informazione per scegliere (materiale, tipo,
    finitura…): la lavorazione è segnata «Da precisare», con che cosa manca, e nessuna
    voce è spuntata.

Si spuntano le voci da aggiungere, anche più di una per lavorazione (entrano tutte, con le
stesse misure), e la barra in fondo mostra quante sono e il totale del computo prima e
dopo. **Aggiungi al computo** le aggiunge in un solo passo, annullabile come ogni altra
modifica, con nelle note la lavorazione, il motivo della scelta e le ipotesi. La proposta
resta aperta: le voci aggiunte sono segnate «aggiunta» e si può continuare a scegliere,
oppure cambiare la richiesta e chiedere una nuova proposta.

Se una misura resta vuota, la voce entra con un rigo **da completare** (quantità zero, i
valori noti nella descrizione del rigo): mai con una quantità parziale. Lo stesso per le
voci con un'unità di misura senza forme (ore, m²/mese…).

Sotto le lavorazioni compaiono quelle **senza voce nel prezzario**, le ipotesi con il
loro impatto, le **domande per precisare** (**Aggiungi le domande alla richiesta** le
copia nel testo, da completare con le risposte), ciò che è **da verificare** e, chiusi,
i dettagli tecnici della verifica, poi i token consumati. Solo se nella richiesta non si
riconosce nessuna lavorazione l'assistente fa le domande invece di proporre.

Gli errori dei servizi (chiave rifiutata, credito esaurito, rete) compaiono come
notifiche, con il collegamento alle impostazioni quando serve.

### Dalla riga di comando

#### Consenso

Senza consenso l'assistente non chiama nessun servizio; la ricerca testuale resta
disponibile. Per leggere il testo e accettarlo:

```sh
opencomputo assistente consenso            # mostra il testo e la sua versione
opencomputo assistente consenso --accetto 1
opencomputo assistente consenso --revoca
```

Se il testo cambia, cambia la versione e va accettato di nuovo.

#### Chiavi

Servono una chiave API di OpenAI e una di Anthropic. Si salvano nel portachiavi del
sistema (Portachiavi di macOS, Gestione credenziali di Windows; su Linux il keyring del
kernel, che **non** sopravvive al riavvio), mai nei file di OpenComputo. La chiave si
scrive su stdin, non come argomento, così non resta nella cronologia della shell:

```sh
opencomputo assistente chiave openai       # incollare la chiave e premere Invio
opencomputo assistente chiave anthropic
opencomputo assistente chiave openai --elimina
opencomputo assistente stato               # quali chiavi ci sono (mai il loro valore)
```

#### Modelli e configurazione

Predefiniti: `text-embedding-3-small` per gli embedding e `claude-sonnet-5-5` per la
generazione, con risposte in JSON vincolate a uno schema. Si possono cambiare:

```sh
opencomputo assistente configura --modello-generazione claude-opus-5-5
opencomputo assistente configura --modello-embedding text-embedding-3-large --dimensioni 1024
opencomputo assistente configura --max-token-selezione 24000
```

Un modello di generazione che OpenComputo non conosce si accetta solo indicando
`--modalita json` o `--modalita strumento-forzato`: non tutti i modelli supportano le
stesse funzioni. Claude Opus 5.5, Sonnet 5.5, Fable 5.1 e Mythos 5.1 non accettano la
modalità `strumento-forzato`.

La configurazione sta in `assistente.json`, nella cartella `~/Library/Application
Support/OpenComputo` (macOS), `%APPDATA%\OpenComputo` (Windows) o
`~/.config/opencomputo` (Linux); la variabile `OPENCOMPUTO_CONFIG` ne indica un'altra.

#### Indicizzare un prezzario

Per la ricerca per significato le voci del prezzario vanno indicizzate una volta:

```sh
opencomputo assistente indicizza prezzario-regionale.dcf
```

L'avanzamento compare su stderr. I vettori si conservano nella sottocartella `cache`
della configurazione; un'indicizzazione interrotta riprende dalle voci mancanti, e quando
il prezzario cambia si ricalcolano solo le voci con una descrizione diversa. Cambiando
modello o dimensioni degli embedding serve una nuova indicizzazione. I file della cache
si possono cancellare in qualsiasi momento.

```sh
opencomputo assistente cerca prezzario-regionale.dcf "tinteggiatura pareti interne" --unita m2
```

La ricerca è «ibrida» (parole e significato) se il prezzario è indicizzato con il modello
configurato, altrimenti solo per parole, senza rete.

#### Proporre il computo

```sh
opencomputo assistente proponi lavori.ocp --prezzario prezzario-regionale.dcf \
    "Tinteggiatura di due pareti di 4 x 3 m e di un soffitto di 4 x 5 m"
```

La risposta contiene le lavorazioni riconosciute, ognuna con le misure e le voci possibili
in ordine di pertinenza (`proposta.lavorazioni[].opzioni`, con `preselezionata` per la più
pertinente delle lavorazioni chiare), le **ipotesi**, le **informazioni mancanti**, gli
avvisi e l'anteprima: valori di ogni opzione e prova delle voci preselezionate sul
progetto. Il file non cambia. Se non si riconosce nessuna lavorazione l'esito è
`da_chiarire`, con le domande da cui ripartire.

Con `--applica` le voci preselezionate, se la prova è riuscita, vengono aggiunte e il
progetto salvato. Per scegliere altre voci o correggere le misure si usa il servizio JSON
(`assistente_anteprima` e `assistente_applica` con le `scelte`: per ogni lavorazione
`{"lavorazione": "L01", "codici": [...], "misure": [{"L": "4", "B": "2,70"}]}`).

Codice di uscita 4: errore dell'assistente (consenso mancante, chiave assente o rifiutata,
credito esaurito, rete, risposta del modello inutilizzabile). Il campo `motivo`
dell'errore lo indica in modo stabile.

## Uso da script e da agenti AI

Il programma `opencomputo` (riga di comando) lavora sugli stessi file con gli stessi
controlli: `opencomputo schema-comandi` elenca i comandi disponibili. Vedi il README.

## Limiti

* Il cronoprogramma non gestisce risorse (squadre, mezzi) né il loro livellamento; le
  attività sono pianificate al più presto, con i soli vincoli «inizio non prima del».
* Gli arrotondamenti predefiniti (2 decimali, metà per eccesso, per rigo) sono
  un'assunzione ragionevole ma non ancora confrontata con documenti prodotti da PriMus.
* Dimensioni e valori massimi: file di progetto fino a 512 MB (anche compressi: conta il
  contenuto), file da importare (XPWE, DCF, listini) fino a 256 MB; quantità e importi
  di una voce o di una registrazione fino a un milione di miliardi (10^15) in valore
  assoluto. Oltre, il file non viene letto o la voce è segnalata come non calcolabile.
  Un progetto con regole di arrotondamento oltre i 10 decimali (possibile solo da un
  file modificato a mano) si apre, ma report ed export sono rifiutati finché non si
  correggono.
* Il calcolo non verifica la conformità alla normativa (D.Lgs. 36/2023): ribasso,
  sicurezza, manodopera e ritenuta sono parametri impostati dall'utente.
* Certificati e rata di saldo non comprendono ritenute fiscali né split payment; il
  programma non verifica le condizioni per sospendere il recupero dell'anticipazione;
  le riserve sono solo testo, senza importi; la garanzia della rata di saldo e il DURC
  non sono registrati.
* Mancano: lavori a corpo, revisione prezzi, quadro economico, modelli di stampa personalizzabili. Un progetto
  per volta in ogni finestra principale: per lavorare su due progetti si avvia
  l'applicazione due volte.
* L'applicazione è stata provata su Linux; macOS e Windows sono da provare.
* Assistente AI: le chiamate reali a OpenAI e Anthropic e i portachiavi di macOS e
  Windows non sono ancora stati provati; su Linux le chiavi si perdono al riavvio.
  Errori di rete, annullamenti e file danneggiati sono provati con provider locali
  finti, su Linux.
