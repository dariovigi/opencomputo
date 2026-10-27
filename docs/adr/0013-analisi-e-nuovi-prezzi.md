# ADR 0013 — Analisi prezzi dinamiche, fabbisogno di cantiere e nuovi prezzi

* Stato: accettato
* Data: 2026-10-21

## Contesto

Il modello delle analisi prezzi esisteva dalla Fase 1, ma non era usabile: mancavano
comandi, interfaccia e stampe, e le righe usavano solo prezzi copiati. Non esistevano i
nuovi prezzi come istituto contrattuale. Il titolare li ha indicati come funzioni
importantissime e ha suggerito come spunto la descrizione del modulo di analisi prezzi
di un prodotto commerciale. Di quella descrizione si sono prese le funzioni, non
l'aspetto né il codice:
* analisi dinamiche;
* voci analizzate usate come elementi di altre analisi;
* spese generali e utile configurabili;
* incidenze di manodopera, materiali e noli, comprese quelle delle sotto-analisi;
* stampa in più impaginazioni;
* fabbisogno di cantiere.

Riferimenti verificati il 2026-10-21:

* **Allegato II.14 al D.Lgs. 36/2023, art. 5**: nuovi prezzi per lavorazioni non
  previste. Si formano dai prezzari, per analogia con prezzi contrattuali o con analisi
  dei prezzi elementari. Sono concordati tra direttore dei lavori ed esecutore e
  approvati dal RUP, o dalla stazione appaltante se comportano maggiori spese. Se
  l'esecutore non li accetta possono comunque essere ammessi in contabilità, e
  l'esecutore li contesta con riserva.
* **Ribasso sui nuovi prezzi**: il previgente DPR 207/2010 (art. 163) li assoggettava
  tutti al ribasso d'asta. Con il nuovo codice la questione è ancora oggetto di quesiti,
  in particolare per i prezzi formati con analisi a prezzi correnti.
* **Spese generali e utile**: i prezzari regionali usano spese generali del 15 % (di
  norma tra il 13 e il 17 %) e utile del 10 % su costo più spese generali (Allegato
  I.14; Allegato I.7, art. 31, per la composizione delle spese generali).

## Decisioni

1. **Analisi nel prezzario, dinamiche** (`Prezzario::calcola_analisi`):
   * le righe collegate a una risorsa o a una voce usano il prezzo **corrente** di
     quella risorsa o voce;
   * una riga che è una voce analizzata usa il prezzo unitario della sua analisi,
     calcolato ricorsivamente (sotto-analisi); i cicli sono un errore che indica la
     catena;
   * la copia dei dati nella riga serve per le righe libere e per le origini non più
     esistenti.

   `AnalisiPrezzo::calcola` resta come calcolo isolato sulle copie.
2. **Il prezzo della voce non cambia da solo.** Prezzo e incidenza della manodopera si
   aggiornano con un comando:
   * `imposta_analisi`, che di norma applica il prezzo, ma non con un'analisi ancora
     vuota;
   * `ricalcola_prezzi_analisi`, per tutte le voci analizzate.

   La validazione segnala con un'informazione le voci non allineate. I computi e il
   libretto conservano comunque i prezzi fotografati.
3. **Incidenze comprese le sotto-analisi**:
   * per una riga che è una voce analizzata, il costo è ripartito secondo le frazioni
     esatte di quella voce; la parte di spese generali e utile della sotto-analisi non
     è attribuita ad alcun tipo;
   * per una voce semplice vale la sua incidenza della manodopera;
   * le frazioni restano esatte e si arrotondano solo le percentuali finali.
4. **Parametri predefiniti per elenco** (`ParametriAnalisi`): spese generali 15 %, utile
   10 % su costo più spese generali. Ogni analisi ha i propri valori. Spese generali
   fuori dal 13–17 % producono un'informazione, non un errore.
5. **Quantità come espressione**: la riga conserva il testo (es. `4*3`) per la modifica,
   accanto al valore.
6. **Fabbisogno di cantiere** (`Progetto::fabbisogno`): espande le voci del computo nelle
   risorse delle analisi, sotto-analisi comprese, e somma le quantità esatte. Arrotonda
   alla fine e valuta ai prezzi correnti, senza spese generali né utile. Elenca a parte
   le voci senza analisi.
7. **Nuovi prezzi come dato della voce di elenco** (`NuovoPrezzo`):
   * numero (NP.n), data, motivo, criterio;
   * soggezione al ribasso, predefinita sì;
   * numero del verbale.

   I **verbali di concordamento** stanno nella contabilità (`VerbaleNuoviPrezzi`): data,
   firma e riserve dell'esecutore, approvazione con l'atto. Un verbale approvato non si
   rimuove e i suoi nuovi prezzi non si tolgono.
8. **Esclusione dal ribasso**:
   * un nuovo prezzo indicato come già netto passa nella fotografia del prezzo
     (`PrezzoApplicato::escluso_dal_ribasso`);
   * nei SAL e nel conto finale l'importo soggetto a ribasso è lavori − manodopera
     esclusa − lavori esclusi; la manodopera di un prezzo escluso non è sottratta due
     volte;
   * cambiando la soggezione, le registrazioni non ancora in un SAL emesso si
     allineano.
9. **Segnalazioni**:
   * nuovo prezzo usato nel libretto ma non approvato: avviso;
   * verbale non firmato, firmato con riserve o vuoto, prezzo già netto, criterio
     «analisi» senza analisi: informazioni;
   * numeri duplicati o verbali inesistenti: errori.
10. **Schema versione 4** con migrazione identità dalla 3 e fixture
    `progetto_esempio_v4.json` congelata. I riepiloghi dei SAL e del conto finale hanno
    il campo `lavori_esclusi_dal_ribasso`, omesso quando è zero: i file precedenti
    restano identici e la verifica di integrità non cambia.
11. **Comandi nuovi** (47 in tutto): `imposta_risorsa`, `rimuovi_risorsa`,
    `imposta_analisi`, `rimuovi_analisi`, `ricalcola_prezzi_analisi`,
    `imposta_parametri_analisi`, `imposta_nuovo_prezzo`, `imposta_verbale_nuovi_prezzi`,
    `rimuovi_verbale_nuovi_prezzi`.
12. **Report nuovi**:
    * `analisi_prezzi`: completa per voce, oppure quadro sintetico;
    * `fabbisogno`;
    * `nuovi_prezzi`: elenco, oppure verbale di concordamento con le firme e le analisi
      dei prezzi che ne derivano.

## Conseguenze e limiti

* Valori verificati a mano: una malta e una muratura che la usa come sotto-analisi, con
  incidenze ripartite e prezzi aggiornati dopo il cambio di una risorsa; un fabbisogno
  di cantiere; un SAL con un nuovo prezzo escluso dal ribasso.
* L'import e l'export delle analisi PriMus (XPWE, DCF) restano esclusi: senza un file
  reale con analisi la struttura non è verificabile. Le analisi importate restano
  conservate nel file di origine, come prima.
* Non gestiti:
  * variabili tra righe della stessa analisi;
  * tabelle di valori;
  * oneri della sicurezza aziendali separati dalle spese generali;
  * coefficienti di aggiornamento dei prezzi;
  * stampa del computo con le analisi delle voci.
* Il programma non decide se un nuovo prezzo è soggetto a ribasso: è un dato da indicare,
  con la scelta prudente (sì) come valore predefinito.
