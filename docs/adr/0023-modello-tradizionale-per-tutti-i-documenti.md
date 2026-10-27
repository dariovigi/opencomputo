# ADR 0023 — Modello tradizionale per tutti i documenti

* Stato: accettato
* Data: 2026-10-22
* Estende: [ADR 0015](0015-stampa-tradizionale.md)

## Contesto

Il titolare ha chiesto di poter stampare nel modello tradizionale (PDF) anche gli altri
documenti di cantiere, e anche con le incidenze di manodopera e sicurezza attive.

Prima il modello tradizionale c'era solo per:
* il computo, anche con le incidenze (ADR 0021);
* l'elenco prezzi.

Il computo con le analisi lo rifiutava. Le impaginazioni di computo ed elenco sono
state misurate su stampe di riferimento; per gli altri documenti stampe di riferimento
non ce ne sono.

## Decisioni

1. **Resa tradizionale del documento ordinario** (`tradizionale::documento`). Vale per
   libretto, registro, SAL, certificato di pagamento, sommario, conto finale, analisi
   dei prezzi (complete e sintetiche), fabbisogno, nuovi prezzi e verbali, computo con
   le analisi. Disegna il `Documento` ordinario con l'aspetto delle stampe
   tradizionali:
   * frontespizio, testata dello studio con il numero di pagina, riquadro verde,
     committente in calce, caratteri Times;
   * per ogni sezione, una tabella con l'intestazione delle colonne in verde tra due
     filetti spessi e i filetti verticali delle colonne. L'intestazione si ripete in
     cima a ogni pagina in cui la tabella continua.
   * le colonne nascono da quelle del documento: le numeriche prendono la larghezza dei
     loro valori, quelle di testo si dividono il resto secondo il peso;
   * i valori alla destra della descrizione stanno sull'ultima riga del testo, numero
     e codice sulla prima;
   * filetto sopra «SOMMANO» e i totali; gruppi in grassetto sottolineato;
   * una riga che va a capo resta intera nella stessa pagina;
   * l'ultima pagina è completata con il tratteggio.

   I valori sono quelli del documento ordinario (ADR 0007): la resa non calcola nulla.
2. **Chi firma** (`OpzioniTradizionale::firma`, nel frontespizio e in fondo):
   * il direttore dei lavori per libretto, registro, SAL, sommario, conto finale e
     nuovi prezzi;
   * il responsabile del procedimento per il certificato di pagamento;
   * il progettista («Il Tecnico») per gli altri documenti.
3. **Computo ed elenco prezzi** mantengono le loro impaginazioni (ADR 0015), con i
   riporti di pagina, le incidenze e il riepilogo finale (ADR 0021).
   * Il computo con le analisi ora usa la resa del punto 1.
   * Le incidenze restano escluse dal computo con le analisi e da quello senza prezzi,
     in entrambi i modelli: l'interfaccia non le offre in quei casi.
   * «Senza prezzi» resta un'opzione del solo computo.
4. **Richiesta**: ogni tipo di report accetta `tradizionale`. I tipi che prima non
   avevano campi (libretto, registro, sommario, conto finale) diventano oggetti con il
   solo campo facoltativo: il JSON senza `tradizionale` è identico a prima. Il file si
   chiama come quello ordinario con `_tradizionale`, es. `sal_2_tradizionale.pdf`.
   Dalla CLI `--tradizionale` vale per tutti i documenti.
5. **Interfaccia**: la casella «Modello tradizionale (PDF)» c'è per tutti i documenti,
   e la scelta resta passando dall'uno all'altro. Ente e studio restano memorizzati.

## Conseguenze e limiti

* La resa dei documenti contabili non è misurata su stampe di riferimento: segue lo
  stile del computo tradizionale, non l'impaginazione di un programma specifico.
* Registro e libretto non hanno le righe «R I P O R T O» e «A R I P O R T A R E»: i
  progressivi del registro restano nelle righe «Importo progressivo» del documento.
* Le tabelle sono in verticale (A4): le dieci colonne del sommario entrano, ma tabelle
  molto più larghe avrebbero colonne strette.
