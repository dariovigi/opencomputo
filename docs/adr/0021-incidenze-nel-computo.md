# ADR 0021 — Incidenze di manodopera e sicurezza nella stampa del computo

* Stato: accettato
* Data: 2026-10-22

## Contesto

Il titolare ha chiesto di poter stampare, nel computo, l'incidenza della manodopera e
l'incidenza della sicurezza, e di poter indicare i subtotali di questi importi nel
riepilogo finale.

Le due incidenze erano già nel modello. Sono percentuali del prezzo, copiate nella
fotografia del prezzo di ogni voce di computo (`PrezzoApplicato`). Il core ne calcola
già la quota sull'importo di ogni voce (`CalcoloPartita.importo_manodopera` e
`importo_sicurezza`), arrotondata come gli importi. Quello che mancava:
* nel riepilogo del computo, il solo totale della manodopera;
* nessun totale della sicurezza compresa nei prezzi;
* nessun subtotale per categoria o WBS.

## Decisioni

1. **Core.** `RiepilogoComputo` aggiunge:
   * `totale_sicurezza` e `voci_senza_incidenza_sicurezza`;
   * `manodopera` e `sicurezza` (`QuoteRaggruppate`): le quote per super categoria,
     categoria, sotto categoria e nodo WBS.

   Ogni voce entra nei gruppi del suo importo, con quota zero se l'incidenza manca. Così
   le chiavi coincidono con quelle dei totali degli importi, e la somma dei gruppi dà il
   totale. I totali WBS cumulati valgono anche per le quote
   (`TotaliRaggruppati::cumulati`). I nuovi campi hanno `serde(default)`. Il progetto
   salvato non cambia: schema 6.
2. **Opzioni del report** (`Incidenze`): `manodopera`, `sicurezza`, `riepilogo` (i
   subtotali). Valgono sia per il modello ordinario sia per il tradizionale. Senza
   opzioni i report restano identici a prima (un test lo verifica).
3. **Modello ordinario** (PDF, XLSX, CSV):
   * il foglio del computo aggiunge, per ogni quota, le colonne «Manodopera €» e
     «Mdo %» o «Sicurezza €» e «Sic. %»;
   * i valori stanno sulla riga «SOMMANO»; «—» indica un'incidenza non indicata;
   * la riga «TOTALE» porta il totale delle quote e la loro incidenza sul totale.
   * con i subtotali:
     * il riepilogo generale aggiunge la colonna «Incidenza %»;
     * i riepiloghi per categorie e WBS aggiungono quota e incidenza sull'importo del
       gruppo.
   * Il riepilogo generale mostra la riga «di cui sicurezza compresa nei prezzi» se è
     scelta la sicurezza o se il totale non è zero, e una nota sulle voci senza
     incidenza.
4. **Modello tradizionale.**
   * Le quote sono **colonne aggiuntive** a destra degli importi: «MANODOPERA» e
     «SICUREZZA», ciascuna con euro e incid.%.
     * Storia: la prima versione metteva righe «di cui…» sotto ogni voce; la seconda
       queste colonne con euro e percentuale; la terza le sole percentuali. Il
       titolare il 2026-10-27 ha chiesto di tornare a importi e incidenze, cioè alla
       seconda.
   * La pagina resta A4 verticale. Le colonne si ridistribuiscono:
     * le misure si stringono a 30 pt l'una;
     * quantità e unitario a 40 pt, totale a 56 pt;
     * le quote prendono 46 pt per gli euro e 26 pt per la percentuale;
     * la designazione prende lo spazio che resta: circa 185 pt con una quota e 113
       pt con due (223 pt senza quote).

     Senza quote le colonne restano esattamente quelle delle stampe di riferimento.
   * I titoli delle colonne troppo larghi per lo spazio (per esempio «incid.%» sulla
     colonna di 26 pt) si rimpiccioliscono fino al 60 % del corpo, così non toccano i
     filetti. Vale per tutte le intestazioni centrate del modello tradizionale.
   * I valori stanno sulla riga «SOMMANO»; «—» se l'incidenza manca.
   * I parziali delle super categorie e il totale riportano le quote del core e la
     loro incidenza sull'importo.
   * Le quote hanno i loro progressivi nei riporti di pagina: sono somme esatte delle
     quote stampate e alla fine devono coincidere con i totali del core, altrimenti la
     stampa non viene prodotta, come per gli importi.
   * Con i subtotali, dopo il totale si aggiungono le pagine del riepilogo. Hanno
     un'intestazione propria: codice, descrizione, importo e, per ogni quota, euro e
     incidenza %. Non hanno riporti.
   * Il riepilogo contiene:
     * super categorie, categorie, sotto categorie e WBS, con i totali dei nodi
       comprensivi dei figli;
     * il «QUADRO GENERALE» (lavori, oneri della sicurezza, totale con le quote);
     * le note sulle voci senza incidenza.

     Luogo, data e firma passano in fondo al riepilogo.
5. **Valori dal core** (ADR 0007): quote, totali e subtotali vengono tutti dal
   riepilogo del core. La stampa calcola solo le incidenze percentuali dei gruppi,
   arrotondate a due decimali come le incidenze esistenti. I riepiloghi per i due
   modelli nascono da una sola funzione (`report::riepiloghi`).
6. **Combinazioni rifiutate** (errore della richiesta): incidenze con il computo con le
   analisi, e incidenze con il computo metrico senza prezzi. L'interfaccia non le offre.
7. **Interfacce.**
   * Nella richiesta: `incidenze: {manodopera, sicurezza, riepilogo}` sul report
     `computo`. Il campo è omesso quando è vuoto.
   * Dalla CLI: `--manodopera`, `--sicurezza`, `--subtotali`.
   * Nella pagina Report, tre caselle: «Incidenza della manodopera», «Incidenza della
     sicurezza», «Subtotali nel riepilogo finale». La terza si attiva solo con una
     delle prime due. Le scelte restano tra una sessione e l'altra.

## Conseguenze e limiti

* La «sicurezza» delle incidenze è la quota compresa nei prezzi. Gli oneri della
  sicurezza restano voci di natura propria, con il loro totale.
* Le quote dei lavori e degli oneri non sono separate: le righe «Totale lavori» e
  «Totale oneri della sicurezza» non riportano quote.
* Nel modello tradizionale con due quote la designazione è stretta (circa metà): le
  descrizioni vanno su più righe e le pagine aumentano.
