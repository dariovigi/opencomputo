# ADR 0020 — Pannello dei filtri del computo e dell'elenco prezzi

* Stato: accettato
* Data: 2026-10-22

## Contesto

Il titolare ha chiesto un pannello di filtro per le viste del computo e dell'elenco
prezzi. Deve filtrare per categorie e WBS e per gli altri parametri delle colonne del
listino, come il pannello di navigazione laterale dei programmi di computo.

Prima di questa decisione categorie e WBS servivano solo ad assegnare e a stampare: i
totali per categoria e per nodo WBS uscivano nei report, non nel computo.

## Decisioni

1. **Un pannello condiviso** (`ui/js/filtri.js`) a sinistra della vista, aperto e chiuso
   dal pulsante «Filtri». Il pulsante mostra quanti filtri sono attivi. Ha quattro tipi
   di sezione:
   * `scelte`: valori distinti con casella di spunta;
   * `albero`: nodi gerarchici (WBS, capitoli); un nodo comprende i suoi discendenti;
   * `testo`: tutte le parole, senza distinguere maiuscole e minuscole;
   * `intervallo`: da… a… su un numero, confrontato come decimale esatto (BigInt), mai in
     virgola mobile.

   I valori della stessa sezione si combinano in OR, le sezioni in AND. Accanto a ogni
   valore compaiono quante voci resterebbero (e il loro importo, nel computo) con gli
   altri filtri già applicati. Le sezioni si chiudono una per una.
2. **Computo.** Le sezioni sono:
   * super categorie, categorie e sotto categorie (ognuna con «non assegnata»);
   * WBS ad albero;
   * capitolo della voce dell'elenco da cui viene la voce di computo;
   * tariffa e descrizione (testo);
   * natura e unità di misura;
   * prezzo, quantità e importo (intervalli);
   * stato del prezzo rispetto all'elenco.

   Comportamento:
   * il foglio mostra solo le voci filtrate, con la numerazione del computo intero;
   * una barra sopra il foglio dice quante voci si vedono, con «Azzera»;
   * i totali aggiungono quello delle voci filtrate;
   * la selezione resta solo sulle voci visibili, così «Elimina voce» non tocca quelle
     nascoste;
   * ↑ e ↓ scavalcano la voce visibile vicina.
3. **Elenco prezzi** (del progetto e di consultazione, un filtro per scheda). Le sezioni
   sono:
   * capitolo ad albero;
   * natura e unità di misura;
   * prezzo e incidenza della manodopera (intervalli);
   * analisi e nuovi prezzi.

   Le voci dell'elenco non hanno categorie né WBS. Per filtrarle così, il pannello usa le
   voci di computo con la stessa tariffa nel computo corrente:
   * «Uso nel computo»: usata o non usata;
   * super categorie, categorie, sotto categorie e WBS con cui la tariffa è usata.

   La ricerca testuale resta quella del motore; i filtri si applicano al suo risultato.
4. **Voci nuove con un filtro attivo.** Una voce aggiunta con «+ Voce», «>» o un
   trascinamento dall'elenco riceve le categorie e il nodo WBS dei livelli in cui il
   filtro ha un solo valore scelto (come fa il programma di riferimento), e la notifica
   lo dice. Se la voce non rientra comunque nel filtro, una notifica lo segnala e offre
   «Azzera i filtri».
5. **Persistenza.** Filtri e sezioni chiuse restano nelle preferenze dell'interfaccia
   (`localStorage`), per vista. I filtri si applicano anche con il pannello chiuso e la
   barra lo ricorda, con «Mostra i filtri». Non sono dati del progetto.
6. **Motore.** Nessun comando nuovo; le viste espongono tre dati in più, senza cambiare
   lo schema:
   * `VistaVoceComputo.capitolo`, il capitolo della voce dell'elenco d'origine;
   * `VistaVoceComputo.id`, già aggiunto con la campagna di test;
   * `VistaElenco.capitoli`, l'albero dei capitoli anche dei prezzari di consultazione.

   La vista dell'elenco ora trova i capitoli per identificativo (mappa): prima la ricerca
   era lineare per ogni voce, lenta sui prezzari regionali.

## Conseguenze e limiti

* Il filtro lavora sui dati già caricati nella vista. Per un prezzario di 30.000 voci i
  conteggi si ricalcolano a ogni cambio: richiedono qualche decina di millisecondi.
* Le categorie dell'elenco riguardano il solo computo corrente.
* Il filtro non cambia le stampe: i report riguardano sempre il computo intero.
