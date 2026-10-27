# ADR 0018 — Voci affiancate al computo (vista divisa)

* Stato: accettato
* Data: 2026-10-21

## Contesto

Il titolare vuole una vista divisa: il computo a destra, l'elenco prezzi o un prezzario a
sinistra, con i pulsanti «>» e «<» per copiare voci rapidamente.

Il trascinamento tra finestre (ADR 0017) copre già lo stesso bisogno con il mouse. Qui
serve un modo più rapido e utilizzabile anche da tastiera, nella stessa finestra.

## Decisioni

1. **Pannello nella vista del computo.** Il pulsante «Affianca voci» apre il pannello a
   sinistra del foglio delle misure, che resta al centro con il dettaglio a destra.
   L'interruttore è una preferenza dell'interfaccia (`affianca`), conservata tra le
   sessioni come quella del dettaglio. Non è un dato del progetto.
2. **Contenuto del pannello**:
   * scelta della fonte: l'elenco prezzi del progetto o uno dei prezzari di consultazione
     caricati (anche questa ricordata, finché il prezzario è caricato);
   * ricerca per codice o parole, come nell'elenco prezzi;
   * griglia con selezione multipla (Maiusc, Ctrl/⌘, Ctrl+A);
   * anteprima della voce attiva, con la descrizione estesa.

   Il pannello è uno per finestra e sopravvive ai ridisegni del computo: fonte, ricerca,
   selezione e scorrimento restano dopo ogni modifica.
3. **«>»** aggiunge le voci selezionate a sinistra dopo l'ultima voce selezionata nel
   computo, o in fondo se nessuna è selezionata. Ogni voce entra con un rigo vuoto.
   * Da un prezzario, le voci che mancano nell'elenco del progetto vengono prima copiate
     (`copia_da_prezzario`), nella stessa richiesta.
   * Le voci aggiunte restano selezionate nel computo, così il «>» successivo aggiunge
     dopo di loro e l'ordine segue quello delle scelte.
   * Il fuoco torna alla lista: si continua con frecce, ricerca e Invio.
   * Invio e doppio clic nella lista fanno lo stesso; il trascinamento resta possibile.
4. **«<»** toglie dal computo le voci selezionate (Ctrl/Maiusc+clic sulle testate per
   più voci). Lo fa senza conferma, perché è il contrario di «>»: la notifica offre
   «Annulla» e un solo annulla ripristina tutto.

   «<» non copia nulla verso sinistra. Le voci di computo sono già nell'elenco del
   progetto, e i prezzari di consultazione si leggono soltanto.
5. **Nessun comando nuovo.** «>» usa `copia_da_prezzario` e `aggiungi_voce_computo`,
   come il rilascio di un trascinamento (stessa funzione dell'interfaccia). «<» usa
   `rimuovi_voce_computo`. Ogni pressione è una sola richiesta atomica.
6. **Segni nella lista.** Le tariffe già usate nel computo hanno un pallino. Nei
   prezzari, quelle già nell'elenco del progetto sono attenuate.
7. **Larghezza.** La colonna dei pulsanti fa da divisore: trascinandola si allarga o si
   restringe il pannello. La larghezza resta nelle preferenze.
8. **Frecce disegnate.** «>» e «<» sono disegnati in vettoriale (SVG), non scritti come
   caratteri. L'altezza di un carattere nella riga dipende dal carattere del sistema:
   scritti come testo erano spostati in basso di circa 2 pixel. Disegnati, restano
   centrati nel pulsante su ogni sistema. Il nome per l'accessibilità è nel pulsante.

## Conseguenze e limiti

* Nel foglio delle misure, attivare una cella conserva la selezione di più voci se la
  comprende, ma solo quando lo chiede la vista. Serve dopo «>».
* Correzione nella griglia: un clic su una riga già selezionata da sola non la ridisegna
  più. Prima il secondo clic di un doppio clic cadeva su una riga appena sostituita e il
  doppio clic non arrivava alla griglia. Valeva anche per l'elenco prezzi e il selettore
  delle voci.
* Con il pannello e il dettaglio aperti il foglio si stringe: sotto i 1400 pixel circa
  conviene chiudere il dettaglio.
* Il pannello lavora sul computo della vista. Per copiare da un computo all'altro restano
  le finestre affiancate e il trascinamento (ADR 0017).
