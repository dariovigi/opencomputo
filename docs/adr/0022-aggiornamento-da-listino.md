# ADR 0022 — Aggiornamento dell'elenco prezzi a un nuovo listino

* Stato: accettato
* Data: 2026-10-22

## Contesto

Il titolare ha chiesto una funzione per aggiornare le voci di prezzo ai nuovi listini o
prezzari, come nel programma di riferimento. Prima di aggiornare vuole una visione
comparativa completa e la possibilità di scegliere quali voci aggiornare.

C'erano già:
* l'aggiornamento con un coefficiente percentuale (`aggiorna_prezzi`, ADR 0014);
* il riallineamento di una voce di computo all'elenco (`riallinea_prezzo`);
* i prezzari di consultazione caricati nella sessione, da cui si copiano le voci.

Mancava il confronto voce per voce con un listino nuovo, con l'aggiornamento selettivo.

## Decisioni

1. **Il listino è un prezzario di consultazione**: DCF non protetto, XPWE o progetto,
   caricato come gli altri e non salvato nel progetto. Si aggiorna l'elenco prezzi del
   progetto, cioè le voci e le risorse delle analisi. I computi seguono con il
   riallineamento.
2. **Corrispondenza per tariffa**:
   * il codice è esatto, senza normalizzazioni che potrebbero accoppiare voci diverse;
   * per i prezzari che cambiano il prefisso a ogni edizione (`TOS23_` → `TOS24_`) si
     indica una **sostituzione del prefisso**;
   * se un codice compare più volte nel listino vale la prima voce, e il confronto lo
     segnala;
   * le risorse si cercano tra le risorse del listino e poi tra le sue voci (molti
     prezzari pubblicano la manodopera e i materiali come voci).
3. **Confronto nel core** (`aggiornamento::confronta_listino`). Per ogni voce
   dell'elenco riporta:
   * l'esito: variata, invariata, non trovata, con analisi;
   * le differenze campo per campo, con il valore dell'elenco e quello del listino:
     tariffa, descrizioni, unità di misura, prezzo, incidenze;
   * la variazione percentuale del prezzo;
   * le voci dei computi che la usano e la somma dei loro importi, oggi e con il
     prezzo del listino (quantità × prezzo, arrotondati come gli importi).

   Per le risorse riporta le stesse informazioni e le analisi che le usano. Riporta
   anche le voci del listino senza corrispondenza e i computi non calcolabili. La
   richiesta del servizio è `confronto_prezzario`, in sola lettura.
4. **Aggiornamento** (`aggiorna_da_prezzario`, un solo comando annullabile):
   * si applica ai codici scelti e ai **campi scelti**: prezzo e incidenze in modo
     predefinito; a richiesta descrizioni, unità di misura e tariffa (quest'ultima solo
     con la sostituzione del prefisso);
   * poi ricalcola le voci analizzate. Le voci con analisi non si aggiornano dal
     listino: il loro prezzo segue l'analisi, quindi si aggiornano le risorse.
   * infine riallinea le voci dei computi collegate alle voci il cui stato è cambiato,
     anche per effetto del ricalcolo. Le collega per identificativo d'origine o per la
     tariffa di prima dell'aggiornamento.

   Se una voce non è nel listino, ha un'analisi o prenderebbe un codice già usato, il
   comando è rifiutato per intero e il progetto non cambia. Le registrazioni del libretto
   conservano il prezzo fotografato: la contabilità non cambia.
5. **Effetto sui computi dal motore**: l'interfaccia manda lo stesso comando alla
   richiesta `prova`, che restituisce i totali dei computi prima e dopo senza applicare
   nulla. Così il riepilogo «14.457,12 → 14.520,12 (+63,00 €)» viene dal core (ADR
   0007). La colonna «Δ computi» è la differenza esatta dei due importi del confronto.
6. **Interfaccia** (vista «Aggiornamento da listino»).
   * Vi si arriva dal menu, da «Aggiorna da listino…» nell'elenco prezzi e da
     «Aggiorna l'elenco da questo…» in un prezzario di consultazione.
   * In alto: listino, «Carica listino…», sostituzione del prefisso.
   * Due schede, voci e risorse, con le stesse colonne: prezzo, listino, variazione %,
     dati diversi, Δ computi, esito. Ogni riga ha una casella; Spazio la alterna sulle
     righe selezionate.
   * Pannello dei filtri (ADR 0020): esito, dati diversi, scelte, variazione, testo,
     capitolo, uso nei computi, prezzo o tipo di risorsa.
   * A destra, il confronto completo della riga: campo, elenco, listino. I campi non
     scelti sono barrati.
   * Le righe scelte all'inizio sono quelle con differenze nei campi da aggiornare.
     Restano scelte solo righe variate o invariate.
   * In basso:
     * «Scegli le visibili», «Solo le variate», «Nessuna»;
     * il numero delle scelte e l'effetto sui computi;
     * «Aggiorna le voci scelte».
7. **Comandi**: 50 (era 49). Lo schema del file non cambia.

## Conseguenze e limiti

* La corrispondenza è solo per tariffa (con prefisso). Le voci che cambiano codice in
  modo diverso non si trovano: restano «non trovate» e si possono aggiornare a mano.
* Il riallineamento copia nella voce di computo tutti i dati della voce dell'elenco,
  come «Riallinea». Se in quella voce erano stati modificati a mano, le modifiche si
  perdono, ma l'aggiornamento resta annullabile.
* Le voci del listino senza corrispondenza non si aggiungono: si copiano dai prezzari
  di consultazione come prima.
