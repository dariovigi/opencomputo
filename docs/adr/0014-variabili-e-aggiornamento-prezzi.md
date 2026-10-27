# ADR 0014 — Variabili nelle analisi, oneri aziendali della sicurezza, aggiornamento dei prezzi, computo con analisi

* Stato: accettato
* Data: 2026-10-21

## Contesto

Completamento delle analisi prezzi (ADR 0013), scelto dal titolare. Restavano fuori
quattro funzioni:
* variabili tra le righe di un'analisi;
* evidenza degli oneri aziendali della sicurezza;
* aggiornamento dei prezzi con un coefficiente;
* stampa del computo con le analisi delle voci.

## Decisioni

1. **Variabili per analisi** (`VariabileAnalisi`): nome, valore o espressione, descrizione.
   * Si valutano nel loro ordine e ognuna può usare le precedenti.
   * Le quantità delle righe con espressione le usano, insieme a `q1`, `q2`, … (quantità
     delle righe **precedenti**, così non ci sono cicli).
   * Nomi: lettere minuscole, cifre e `_`; `pi` e `q<n>` sono riservati.
   * Il valore calcolato è conservato anche nella riga: il comando lo aggiorna e il
     calcolo usa sempre l'espressione. Così una variabile modificata ricalcola tutte le
     righe che la usano.
2. **Oneri aziendali della sicurezza come quota delle spese generali**
   (`quota_oneri_sicurezza`, con un valore predefinito per l'elenco). L'importo è
   evidenziato nel risultato e nelle stampe e **non cambia il prezzo**: sono costi già
   compresi nelle spese generali. Sono distinti dai costi della sicurezza del PSC
   (`incidenza_sicurezza` delle voci), che non sono soggetti a ribasso.
3. **Aggiornamento dei prezzi** (`aggiorna_prezzi`): variazione percentuale, anche
   negativa, su:
   * risorse, tutte o di un tipo;
   * voci senza analisi, tutte o di un capitolo e dei suoi sottocapitoli;
   * in entrambi i casi eventualmente solo per codici indicati.

   Le voci analizzate non sono toccate: con `ricalcola_analisi` (predefinito) si
   aggiornano poi dalle analisi. Il nuovo prezzo è arrotondato ai decimali dei prezzi
   calcolati. Una variazione che azzera o rende negativi i prezzi è rifiutata. È
   un'operazione esplicita nella cronologia della sessione, annullabile come le altre.
4. **Computo con analisi** (`Progetto::analisi_computo`, report `computo` con
   `con_analisi`): per ogni voce del computo, gli elementi dell'analisi riportati alla
   quantità computata.
   * Elementi: quantità × quantità della voce / quantità di riferimento, arrotondata,
     per il prezzo corrente.
   * Spese generali e utile in proporzione.
   * Una riga di **differenza** riconcilia con l'importo della voce: comprende gli
     arrotondamenti e l'eventuale scostamento tra il prezzo fotografato e quello
     dell'analisi.

   I valori sono calcolati nel core (ADR 0007). Le sotto-analisi restano un elemento al
   loro prezzo; per le risorse complessive c'è il fabbisogno di cantiere.
5. **Schema versione 5** con migrazione identità dalla 4 e fixture
   `progetto_esempio_v5.json` congelata (comandi: 48).

## Conseguenze e limiti

* Valori verificati a mano:
  * intonaco con spessore e peso come variabili e il nolo pari a metà delle ore della
    riga 1: prezzo 11,30, oneri della sicurezza 0,27;
  * muratura di 100 m² nel computo con differenza di arrotondamento 0,30;
  * aggiornamento del 10 % della manodopera.
* Non gestiti:
  * tabelle di valori richiamate dalle analisi;
  * variabili comuni a più analisi;
  * storico degli aggiornamenti dei prezzi (resta la cronologia della sessione);
  * indici ufficiali di aggiornamento (si inserisce la percentuale).
