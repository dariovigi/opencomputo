# ADR 0019 — «Vedi voce»: righi che citano un'altra voce

* Stato: accettato
* Data: 2026-10-22

## Contesto

Il titolare ha chiesto un rigo di misura che faccia riferimento a un'altra voce di
computo, come il «vedi voce» dei programmi di computo. Ha chiesto anche di selezionare
più voci e inserirle come «vedi voce» in righi diversi. Ha indicato di partire dal
computo e di trattare il «vedi voce» in modo statico nel libretto delle misure.

Prima di questa decisione il modello non aveva misure collegate. L'import XPWE portava i
righi «vedi voce» con la quantità dichiarata nel file, senza collegamento (ADR 0006).

## Decisioni

1. **Modello** (schema 6, migrazione 5 → 6 senza trasformazioni). `RigoMisura` ha il campo
   facoltativo `vedi_voce`, che può essere:
   * `collegata`: l'identificativo di una voce dello stesso computo. La quantità del
     rigo segue la voce;
   * `statica`: una quantità copiata e il testo del riferimento (per esempio «voce n. 1
     «A.01» del computo «Variante»»). Non cambia più.
2. **Calcolo** (`opencomputo-core`):
   * senza formula, la quantità citata è un fattore in più: rigo = quantità citata ×
     fattori valorizzati (solo la quantità citata se non ce ne sono);
   * con una formula, questa sostituisce il prodotto e usa la quantità citata con la
     variabile `vv`;
   * la quantità citata è quella della voce, arrotondata come la quantità di una voce;
   * il segno del rigo (deduzione) si applica come agli altri righi.

   `Computo::quantita_riferite` calcola le quantità di tutte le voci in ordine
   topologico: prima le voci citate, poi quelle che le citano, anche se vengono prima
   nel computo. L'algoritmo è iterativo (Kahn), quindi una catena lunga non consuma lo
   stack.
   * Una voce in un riferimento circolare, compresa una voce che cita sé stessa, ha un
     errore sul rigo che cita, con il numero della voce coinvolta. Ce l'hanno anche le
     voci che dipendono da essa.
   * La validazione lo riporta come `calcolo.errore`, quindi la sessione rifiuta il
     comando che lo introduce.
   * `RigoMisura::calcola` fuori da un computo rifiuta i collegamenti; `calcola_con`
     riceve le quantità.

   Tutti i calcoli di computo passano dalle quantità delle voci citate: totali, viste,
   validazione, fabbisogno, report, stampa tradizionale, export XPWE.
3. **Libretto: solo statico.** Un rigo del libretto non può avere un collegamento
   (errore di validazione `contabilita.vedi_voce_collegato`). Il comando copia la
   quantità attuale della voce indicata, per cui il libretto non cambia quando cambia il
   computo, coerentemente con il fatto che le misure sono già state eseguite.
4. **Comandi** (nessun comando nuovo: restano 49). Il rigo in ingresso (`NuovoRigo`) ha
   `vedi_voce`, che può essere:
   * numero d'ordine o identificativo: una voce dello stesso computo, collegata. Un
     identificativo di una voce di un altro computo, per esempio in un rigo copiato e
     incollato, diventa statico;
   * `{"computo", "voce"}`: collegata se il computo è lo stesso, altrimenti statica;
   * `{"quantita", "riferimento"}`: statica.

   Nel libretto ogni forma diventa statica; il solo numero vale se il progetto ha un solo
   computo. Più voci si citano con più `aggiungi_rigo` nella stessa richiesta, che resta
   atomica e si annulla in un passo.
5. **Il collegamento si conserva**:
   * spostare la voce citata non cambia nulla: il collegamento usa l'identificativo;
   * togliere la voce citata (`rimuovi_voce_computo`) rende statici i righi che la
     citano, con la quantità che avevano, e il messaggio lo dice;
   * copiare voci (`copia_voci_computo`):
     * tra le voci copiate i collegamenti seguono le copie;
     * verso le altre voci restano collegati nello stesso computo e diventano statici
       in un altro computo;
   * spostare voci in un altro computo rende statici i collegamenti che attraversano i
     due computi, nelle voci spostate e in quelle rimaste.
6. **XPWE**:
   * **Import.** Il rigo «vedi voce» (`IDVV` = ID della voce citata) entra statico con
     la quantità dichiarata. A fine computo si prova il collegamento: prima con i
     fattori del file come moltiplicatori, poi senza. Il collegamento resta solo se la
     quantità ricalcolata coincide con quella dichiarata, e si verifica a ogni giro
     perché un collegamento tolto cambia le voci a valle. Altrimenti il rigo resta
     statico, con un avviso.
     * La convenzione esatta di PriMus resta non verificata. Questo controllo evita di
       cambiare le quantità del documento se la convenzione è diversa.
   * **Export.**
     * I righi «vedi voce» importati riscrivono i campi originali, con `IDVV` della voce
       collegata e la quantità ricalcolata.
     * Quelli inseriti in OpenComputo si esportano con la quantità citata nei campi di
       misura e senza collegamento (`IDVV` = nessuno), con una segnalazione. La quantità
       in PriMus è così giusta qualunque sia la sua convenzione.
7. **Stampe.** Il rigo riporta «Vedi voce n. 12 [m² 125,40]», o il riferimento statico,
   prima della descrizione: nel report del computo (PDF, XLSX, CSV), nel libretto e nella
   stampa tradizionale.
8. **Interfaccia**:
   * **Inserimento.** «Vedi voce…» nel computo apre la scelta delle voci, con selezione
     multipla e ricerca. Inserisce un rigo per voce scelta sotto il rigo attivo, con
     l'opzione «in deduzione».
   * **Esclusioni.** La scelta non propone la voce stessa né le voci che la citano già,
     direttamente o indirettamente: sarebbero riferimenti circolari.
   * **Nel foglio.** Il riferimento appare come etichetta prima della descrizione; un
     clic porta alla voce citata. Il dettaglio della voce elenca le voci che la citano.
   * **Nel libretto.** Lo stesso pulsante sceglie il computo e le voci e copia le
     quantità.

## Conseguenze e limiti

* La quantità citata è sempre quella totale della voce. Non si possono citare un rigo o
  una parte dei righi; quando serve, si usano i fattori o la formula con `vv`.
* I collegamenti valgono solo nello stesso computo. Tra computi diversi il «vedi voce» è
  statico.
* Copiando e incollando righi tra computi o nel libretto, i collegamenti diventano
  statici.
* Per riallineare un «vedi voce» statico si reinserisce il rigo.
* Fixture `progetto_esempio_v6.json`: le fixture v1–v5 restano invariate e leggibili.
