# ADR 0017 — Più finestre e trascinamento di voci tra finestre

* Stato: accettato
* Data: 2026-10-21

## Contesto

Il titolare vuole aprire più viste come finestre separate e trascinare voci, una o più,
da una finestra all'altra: per esempio dall'elenco prezzi al computo, o da un computo a
un altro.

Le finestre separate esistevano già, con il pulsante ⧉ (ADR 0009). Tutte lavorano sullo
stesso progetto, tenuto dal lato Rust, e si aggiornano a ogni modifica.

## Decisioni

1. **Aprire le viste in finestre**:
   * ⧉ apre la vista corrente;
   * Ctrl+clic (⌘ su macOS) o clic centrale su una voce del menu apre quella vista senza
     lasciare quella corrente, compreso un computo qualsiasi.

   Le nuove finestre si aprono a cascata rispetto alla principale. Titolo e testata
   mostrano la vista o il nome del computo.
2. **Selezione multipla**:
   * nell'elenco prezzi del progetto e nei prezzari di consultazione, con Maiusc e Ctrl;
   * nel computo, con Ctrl/Maiusc+clic sulle testate delle voci. «Elimina voce» elimina
     tutte le voci selezionate.

   Un clic su una riga già selezionata insieme ad altre non riduce la selezione finché
   non si capisce se parte un trascinamento.
3. **Trascinamento HTML** tra finestre. Le finestre hanno già il gestore dei file del
   sistema disattivato (`dragDropEnabled: false`).
   * Il dato trascinato è un JSON con il formato proprio
     `application/x-opencomputo-voci`, con i campi `tipo`, `codici`, `computo`, `voci`,
     `prezzario` e `finestra`. Il testo semplice porta le tariffe, per chi rilascia in un
     altro programma.
   * **Riserva**: all'inizio del trascinamento la finestra d'origine affida i dati al
     lato Rust (comando `trascinamento`) e li ripulisce alla fine. Se il sistema non
     passa il formato proprio tra due finestre, la destinazione li legge da lì.
4. **Effetti del rilascio** (sempre con i comandi della sessione, quindi atomici e
   annullabili in un passo):
   * dall'elenco prezzi a un computo: `aggiungi_voce_computo` nel punto di rilascio, con
     un rigo vuoto;
   * da un prezzario di consultazione a un computo: `copia_da_prezzario` delle voci
     mancanti nell'elenco, poi come sopra;
   * da un prezzario all'elenco del progetto: `copia_da_prezzario`;
   * da un computo a un computo: il nuovo comando `copia_voci_computo` (49 in tutto).
     Tra computi diversi le voci si copiano, Maiusc le sposta; nello stesso computo si
     spostano, Alt o Ctrl le copia.
5. **`copia_voci_computo`** (`da`, `voci`, `a`, `posizione`, `sposta`):
   * la copia porta righi, prezzo fotografato, categorie, WBS e note, con identificativi
     nuovi;
   * lo spostamento conserva gli identificativi, e quindi i riferimenti del libretto: se
     uno spostamento lascia riferimenti non validi, la validazione lo rifiuta;
   * la posizione è contata prima della modifica;
   * il prezzo non viene ripreso dall'elenco: resta quello fotografato.

## Conseguenze e limiti

* Il punto di rilascio nel computo si sceglie tra una voce e l'altra: sopra la metà
  della testata si inserisce prima della voce, altrove dopo.
* I test end-to-end simulano due finestre con due pagine e gli eventi di trascinamento
  HTML: provano il formato proprio e la riserva del lato Rust. Il comportamento dei
  sistemi operativi con il trascinamento tra finestre (WebView2, WKWebView, WebKitGTK)
  va provato sulle macchine reali.
* Non gestiti: il trascinamento dei righi di misura (restano copia e incolla), delle voci
  del libretto e delle voci nell'elenco prezzi per riordinarle.
