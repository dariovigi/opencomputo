# ADR 0008 — Casi d'uso, file di progetto e interfaccia per agenti

* Stato: accettato (Fase 4)
* Data: 2026-10-20

## Contesto

Fino alla Fase 3 il progetto esisteva solo in memoria: mancavano un file di progetto, un
modo uniforme di modificare i dati con annulla/ripristina e un'interfaccia utilizzabile
da script e agenti AI. GUI (Fase 5), CLI e agenti devono applicare le stesse regole: un
agente non deve poter fare ciò che l'interfaccia grafica impedisce.

## Decisioni

### File di progetto (`opencomputo-storage`)

1. **Contenuto = documento JSON versionato del core** ([ADR 0004](0004-formato-persistenza.md)),
   estensione consigliata `.ocp`, facoltativamente compresso con gzip. In lettura la
   compressione si riconosce dai byte iniziali, non dall'estensione; è tollerato un BOM
   UTF-8. Il documento decompresso è limitato a 2 GiB.
2. **Salvataggio atomico**: verifica del contenuto per rilettura e confronto con il
   progetto in memoria, scrittura in un file temporaneo nella stessa cartella,
   `fsync`, rinomina sopra la destinazione, `fsync` della cartella (Unix). Un errore o
   un'interruzione lasciano intatto il file precedente.
3. **Copie di sicurezza a rotazione**: prima di sostituire un file esistente se ne copia
   la versione precedente in `nome.ocp.bak1`, scalando le precedenti fino a `bakN`
   (3 per impostazione predefinita).
4. `codifica`/`decodifica` non usano il filesystem e compilano per WebAssembly;
   `salva`/`apri` usano `std::fs`.

### Casi d'uso (`opencomputo-application`)

5. **Ogni modifica è un `Comando` serializzabile** (JSON con il campo `comando`, campi
   sconosciuti rifiutati): metadati, contratto, regole, elenco prezzi, copia da un
   prezzario di consultazione, categorie, WBS, computi, voci, righi, libretto, SAL.
   I comandi usano i riferimenti che una persona userebbe (codici di tariffa, numero
   d'ordine della voce, nome del computo, codice WBS completo) e accettano i fattori di
   misura come numeri, testo con la virgola o espressioni. Le regole del dominio
   restano nel core: i comandi le invocano, non le duplicano.
6. **Richieste atomiche**: una richiesta è una lista di comandi applicata su una copia
   del progetto; la copia sostituisce l'originale solo se tutti i comandi riescono.
7. **Politica di validazione**: dopo i comandi il progetto è validato e confrontato con
   lo stato precedente. Per impostazione predefinita sono **rifiutate le richieste che
   introducono nuovi errori** (gli avvisi passano e sono riportati); gli errori già
   presenti non bloccano altre modifiche. La politica permissiva esiste per correggere
   progetti importati in più passi. Un SAL emesso non è alterabile: la verifica di
   integrità del core segnala come errore qualsiasi modifica che cambi il ricalcolo di un
   SAL emesso (registrazioni, ribasso, regole).
8. **Esito strutturato**: descrizione di ogni comando, problemi comparsi e risolti,
   totali dei computi prima e dopo. La **prova** (dry-run) produce lo stesso esito senza
   modificare il progetto.
9. **Annulla/ripristina per istantanee**: ogni richiesta applicata conserva il progetto
   precedente (fino a 50 passi). È più semplice e sicuro dei comandi inversi, che
   andrebbero scritti e verificati per ogni comando; il costo in memoria è accettabile
   per progetti di qualche MB. Da rivalutare se i progetti diventano molto grandi.
10. **Identificativi nuovi** generati in sequenza a partire dal valore massimo presente
    nel progetto, così non si scontrano con quelli esistenti e restano deterministici
    (utile per test e confronti).

### Interfaccia per persone e agenti (`opencomputo-agent-api`)

11. **Un binario, `opencomputo`**, con comandi singoli che leggono e scrivono file di
    progetto: `nuovo`, `importa`, `info`, `valida`, `cerca`, `esegui` (con `--prova`),
    `esporta`, `report`, `schema-comandi`, `servizio`. Libreria per gli argomenti: `clap`
    (MIT OR Apache-2.0).
12. **Uscita sempre JSON**: `{"ok": true, "risultato": …}` oppure
    `{"ok": false, "errore": {"codice": …, "descrizione": …}}`. Codici di uscita: 0
    eseguito, 1 rifiutato (o progetto non valido per `valida`), 2 argomenti errati, 3
    errore di file o di formato. Gli agenti non devono interpretare testo libero.
13. **Servizio a righe JSON** (`opencomputo servizio`): una richiesta per riga su stdin,
    una risposta per riga su stdout, con `id` facoltativo riportato nella risposta. Tiene
    aperto un progetto con annulla/ripristina e prezzari di consultazione caricati, e
    rifiuta di aprire un altro progetto o di chiudere se ci sono modifiche non salvate.
    I comandi singoli sono implementati come sequenze di richieste del servizio, quindi
    i due modi d'uso si comportano allo stesso modo.
14. **`schema-comandi`** restituisce regole ed esempi di tutti i comandi; un test verifica
    che ogni esempio sia un comando valido e che ogni comando abbia un esempio.
15. GUI e agenti passano entrambi da `opencomputo-application`: non scrivono il progetto
    né chiamano i formati direttamente.

## Conseguenze e limiti

* Nessun blocco del file tra processi: due processi che salvano lo stesso file si
  sovrascrivono (l'ultimo vince, il precedente resta in `bak1`).
* Annulla/ripristina vivono nella sessione: i comandi singoli della CLI non li
  conservano tra un'invocazione e l'altra (restano le copie di sicurezza). Un agente che
  vuole annullare usa il servizio.
* Il servizio non è un server di rete né un server MCP: un adattatore MCP potrà essere
  costruito sopra lo stesso protocollo.
* Le istantanee raddoppiano la memoria a ogni passo per il progetto intero.
* Mancano comandi per analisi prezzi, risorse elementari, modifica di capitoli e nodi WBS
  esistenti, e per spostare registrazioni tra SAL.
