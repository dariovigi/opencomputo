# ADR 0026 — Prestazioni con progetti grandi e limiti dei file

* Stato: accettato
* Data: 2026-11-20

## Contesto

Il titolare ha chiesto di ottimizzare l'intero programma e di cercare difetti e
vulnerabilità. Le misure su un computo sintetico grande (5.000 voci di elenco, 20.000 voci
di computo, 100.000 righi; XPWE di 26 MB) attraverso il servizio JSON, la stessa strada
dell'interfaccia, hanno mostrato:

* ogni modifica copiava l'intero progetto per l'annullamento: 460 ms per un rigo e, con
  50 passi conservati, oltre 4 GB di memoria (2,7 GB già dopo 20 modifiche);
* `info` e `valida` ricalcolavano validazione e totali a ogni richiesta (245 e 125 ms);
* le ricerche di voci di elenco per codice o identificativo erano lineari;
* l'apertura di un `.ocp` passava sempre dall'albero JSON generico (2,2 s);
* l'anteprima tradizionale di 6 pagine impaginava, disegnava e assemblava in PDF tutte
  le 3.127 pagine (2 s, 750 MB);
* nessun limite alla dimensione dei file aperti o importati: un `.ocp` compresso di pochi
  KB poteva espandersi fino a esaurire la memoria.

Le sequenze casuali di comandi aggiunte in questa revisione hanno trovato inoltre che
`imposta_regole` accettava un miliardo di decimali: con la politica permissiva (o da un
file costruito apposta) il report Excel chiedeva 1 GB per un formato numerico.

## Decisioni

1. **Voci condivise tra gli stati.** `Computo.voci`, `Prezzario.voci` e
   `LibrettoMisure.registrazioni` sono `Vec<Arc<…>>` (feature `rc` di serde: il formato
   del file non cambia). Copiare il progetto per l'annullamento copia solo i puntatori;
   una voce si modifica con `Arc::make_mut`, che la duplica solo se è condivisa con un
   altro stato. I comandi toccano solo le voci che cambiano davvero (per esempio il
   «vedi voce» rinumera solo le voci che citano quelle spostate).
2. **Stato calcolato una volta per progetto.** La `Sessione` conserva, per lo stato
   corrente e per ogni passo di annulla/ripristina, problemi di validazione e totali dei
   computi (`problemi()`, `totali_computi()`), calcolati al più una volta (`OnceLock`).
   `info` e `valida` del servizio li leggono senza ricalcolare.
3. **Indice delle voci di elenco.** `Prezzario` ha un indice per codice e identificativo
   (`IndiceVoci`, non serializzato) costruito alla prima ricerca. Ogni risultato è
   verificato sulla voce; se l'indice è vecchio si ricostruisce, e in caso di dubbio si
   torna alla scansione: il risultato è sempre quello della scansione lineare (test
   `prove_indice`). Sotto le 64 voci si usa la scansione.
4. **Lettura diretta dei file correnti.** Un documento della versione corrente dello
   schema, con `formato` e `versione_schema` prima di `progetto` (come li scrive
   OpenComputo), si legge direttamente nel modello; altrimenti si passa dall'albero JSON
   e dalle migrazioni come prima. Un test verifica che le due strade diano lo stesso
   progetto.
5. **Limiti dei file.** `.ocp` fino a 512 MB, controllati prima della lettura e, per i
   file compressi, durante la decompressione (`DIMENSIONE_MASSIMA`); documenti da
   migrare fino a 256 MB (`TESTO_MASSIMO_MIGRAZIONI`); file importati dal servizio
   (XPWE, DCF, listini) fino a 256 MB (`IMPORT_MASSIMO`). Oltre: errore, senza leggere.
6. **Anteprima tradizionale solo delle pagine richieste.** La stampa tradizionale ha
   un'uscita (`Uscita`): solo PDF (report), solo l'anteprima SVG di un intervallo di
   pagine (anteprima; l'impaginazione resta quella dell'intero documento, quindi il
   numero delle pagine è esatto) o entrambi (prove di coerenza). Un test verifica che le
   tre uscite coincidano.
7. **Regole di arrotondamento entro i limiti.** `imposta_regole` rifiuta più di 10
   decimali con qualunque politica; report, anteprime ed export rifiutano un progetto
   con regole fuori dai limiti (possibile solo da un file costruito apposta), con un
   messaggio chiaro.

## Conseguenze

* Misure dopo le modifiche (stesso computo, Linux, `--release`): modifica di un rigo
  190 ms con memoria stabile (515 MB dopo 20 modifiche invece di 2,7 GB); `info` e
  `valida` sotto 1 ms; import XPWE 1,0 s e 359 MB (erano 2,0 s e 718 MB); apertura
  `.ocp` compresso 0,57 s (2,2 s); salvataggio compresso 0,9 s (1,8 s); anteprima
  tradizionale 0,4 s e nessuna memoria in più (2 s, +240 MB); PDF tradizionale 1,2 s
  (2,1 s).
* Codice che modifica voci: usare `voce_mut`/`Arc::make_mut`, mai sostituire l'intero
  vettore; leggere con `&**v` o `Arc::as_ref` dove serve un `&Voce`.
* Restano lenti, e sono candidati per il futuro: la vista del computo intero (0,7 s e
  32 MB di JSON, ricaricata dall'interfaccia dopo ogni modifica), i report completi
  (1,2–1,8 s), la validazione completa dopo ogni modifica (gran parte dei 190 ms).
