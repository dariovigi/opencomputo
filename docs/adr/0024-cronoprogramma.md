# ADR 0024 — Cronoprogramma dei lavori

* Stato: accettato
* Data: 2026-10-22

## Contesto

Il titolare ha chiesto, su un ramo separato, un cronoprogramma dei lavori:
* durate e legami di causa ed effetto tra le lavorazioni suddivise nella WBS;
* dettaglio a scelta tra i livelli della WBS;
* dati salvati nel file del progetto ma senza effetti sugli export verso altri programmi;
* un editor completo, sul modello dei programmi di project management.

La WBS esisteva già (nodi ad albero, voci di computo assegnate ai nodi); mancavano
tempi, legami e calendario.

## Decisioni

1. **Le attività sono i nodi della WBS**. Ogni nodo è un'attività; i dati del
   cronoprogramma (durata, vincolo, avanzamento, note) si aggiungono a parte
   (`Cronoprogramma::attivita_wbs`), senza toccare la WBS. Per ciò che la WBS non ha
   (allestimento del cantiere, collaudi, pietre miliari) si aggiungono **attività
   proprie** (`AttivitaAggiunta`), sotto un nodo o alla radice. Rimuovere un nodo WBS
   rimuove i suoi dati e legami; le attività proprie passano al nodo padre.
2. **Livello di dettaglio**: si mostrano i nodi fino al livello scelto. Un nodo che a
   quel livello non mostra i figli diventa un'attività semplice:
   * con una durata propria vale quella (pianificazione sommaria, poi dettagliata);
   * senza, vale la durata del suo riepilogo nel piano completo.

   I legami di un nodo nascosto passano al primo antenato visibile; quelli interni a
   un'attività ridotta o tra un nodo e ciò che contiene si ignorano. Un legame su un
   riepilogo vale per tutte le attività che contiene (come nei programmi di project
   management). Un legame tra un riepilogo e ciò che contiene è rifiutato.
3. **Calcolo nel core** (`cronoprogramma::calcola`, metodo del percorso critico):
   * giorni lavorativi interi, intervalli semiaperti;
   * legami fine-inizio, inizio-inizio, fine-fine e inizio-fine con ritardo o
     anticipo in giorni lavorativi;
   * vincolo «inizio non prima del»;
   * passata in avanti e all'indietro: date al più presto e al più tardi, margine
     totale, attività critiche (margine nullo);
   * i riepiloghi prendono gli estremi delle attività contenute e l'avanzamento pesato
     sulle durate;
   * i cicli sono rifiutati con i nomi delle attività coinvolte. I comandi verificano
     i cicli anche al dettaglio completo, perché un ciclo può passare per un riepilogo.

   Una durata mancante vale un giorno ed è segnalata come «da stimare».
4. **Calendario**:
   * giorni lavorativi della settimana (lunedì–venerdì in modo predefinito);
   * festività nazionali italiane, con la Pasqua calcolata e il lunedì dell'Angelo;
   * chiusure del cantiere (dal–al, con descrizione).

   L'inizio è la data del cronoprogramma o, se manca, la consegna dei lavori del
   contratto; l'ultimazione contrattuale è la consegna più il tempo utile.
5. **Cronoprogramma economico**: gli importi vengono dal computo scelto (il primo se non
   indicato), cumulati sui nodi WBS. L'importo di ogni attività è distribuito sui suoi
   giorni lavorativi, mese per mese, con somme esatte (decimali, resto nell'ultimo
   mese). Ne vengono la produzione mensile, il cumulato e la percentuale (curva a S). Le
   voci senza WBS non si distribuiscono e sono indicate a parte.
6. **Linea di base**: fotografia delle date di tutte le attività, al dettaglio
   completo, con il giorno di salvataggio. Il piano riporta date di base e scostamento
   della fine in giorni lavorativi, a qualsiasi livello.
7. **Persistenza**: campo `cronoprogramma` del progetto, schema 7, omesso quando è
   vuoto. La migrazione da 6 a 7 è l'identità e la fixture v7 lo contiene. Gli export
   XPWE e DCF non lo leggono, quindi non cambiano.
8. **Comandi** (annullabili, ADR 0008), con le attività indicate come `{"wbs":
   "01.02"}` o `{"attivita": "<id>"}`:
   * `imposta_cronoprogramma`: inizio, livello, computo, calendario;
   * `imposta_attivita`, `aggiungi_attivita`, `rimuovi_attivita`;
   * `collega_attivita`, che aggiorna il legame se esiste già;
   * `scollega_attivita`, `linea_base_cronoprogramma`.

   La richiesta `cronoprogramma` restituisce dati e piano calcolato. La validazione
   segnala riferimenti orfani, avanzamenti oltre 100, legami su sé stessi, cicli,
   livello 0 e calendario senza giorni lavorativi.
9. **Report** (valori dal core, ADR 0007):
   * tipo `cronoprogramma`: tabella delle attività (WBS, durata, date, predecessori,
     margine, criticità, avanzamento, importo, scostamento) e produzione mensile;
     esce in PDF, XLSX e CSV, anche nel modello tradizionale;
   * con `gantt`: diagramma di Gantt in PDF A3 orizzontale, con la tabella a sinistra
     e le barre sul calendario. Riporta mesi e giorni o settimane, giorni non
     lavorativi, riepiloghi, pietre miliari, legami, percorso critico e linea di base,
     su più pagine se serve.
10. **Editor** (vista «Cronoprogramma» nella sezione «Pianificazione»):
    * tabella con modifica in linea (durata, inizio come vincolo, predecessori scritti
      come `3;4II+2;5FF-1`, avanzamento, nome delle attività proprie), tastiera,
      riepiloghi comprimibili;
    * Gantt in SVG con scorrimento sincronizzato. Si trascina una barra per spostarla
      (vincolo), il bordo destro per la durata, una barra in verticale su un'altra per
      collegarle. I legami si selezionano, si modificano e si tolgono;
    * scale giorni, settimane, mesi e «Adatta»; percorso critico, linea di base, oggi
      e ultimazione contrattuale;
    * pannello dei dettagli con predecessori e successori, pannello della produzione
      mensile con la curva a S;
    * calendario in una finestra; selettore del livello di dettaglio.

    Le date mostrate vengono sempre dal piano del core. Il calendario replicato
    nell'interfaccia serve solo all'anteprima del trascinamento.

## Conseguenze

* Il file del progetto passa allo schema 7; i file senza cronoprogramma restano
  identici a prima, salvo il numero di versione.
* Le risorse (squadre, mezzi) e il livellamento non ci sono: si possono aggiungere
  senza cambiare le attività.
* Il calcolo è deterministico e senza virgola mobile; le quote mensili sono in decimali
  esatti.
