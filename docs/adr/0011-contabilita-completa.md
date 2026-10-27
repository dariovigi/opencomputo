# ADR 0011 — Anticipazione, penali, certificato di pagamento e sommario del registro

* Stato: accettato
* Data: 2026-10-21

## Contesto

Dopo la Fase 6 il titolare ha chiesto di completare la contabilità: anticipazione del
prezzo e suo recupero, penali, certificato di pagamento con IVA, sommario del registro
per articolo. Riferimenti normativi verificati il 2026-10-21:

* **D.Lgs. 36/2023, art. 125**: anticipazione pari al 20 % del valore del contratto,
  elevabile fino al 30 % se previsto dai documenti di gara; il recupero avviene sui
  pagamenti successivi secondo quanto previsto dal contratto (la norma non fissa una
  formula).
* **D.L. 26 giugno 2026, n. 107, art. 1-bis** (inserito dalla legge di conversione
  7 agosto 2026, n. 152, G.U. n. 192 del 20 agosto 2026, in vigore dal 21 agosto): la
  stazione appaltante, su richiesta dell'esecutore e motivando, può sospendere il
  recupero dell'anticipazione nei contratti di lavori in corso per il tempo strettamente
  necessario e non oltre il 31 dicembre 2026; esclusi i lavori finanziati in tutto o in
  parte con il PNRR. L'anticipazione non recuperata resta dovuta.
* **D.Lgs. 36/2023, art. 126** (testo dopo il D.Lgs. 209/2024): penali per ritardo
  dallo 0,5 all'1,5 per mille giornaliero dell'ammontare netto contrattuale, in totale
  non oltre il 10 %.
* **Allegato II.14, art. 12**: il sommario del registro riporta, per ogni articolo di
  elenco, quantità e importi contabilizzati; il RUP emette il certificato di pagamento
  sulla base del SAL.

## Decisioni

1. **Recupero dell'anticipazione sul progressivo.** Per ogni certificato:
   `recupero = min(anticipazione, % recupero × importo netto progressivo) − già
   recuperato`, limitato tra zero e il residuo. Come per ribasso e ritenuta, gli
   arrotondamenti non si accumulano. La percentuale di recupero è un parametro
   (predefinita uguale a quella dell'anticipazione) perché la norma la rimette al
   contratto.
2. **Sospensione e recupero del residuo per SAL**: due opzioni del SAL in bozza.
   Con il recupero sospeso il certificato non recupera nulla e quello successivo si
   riallinea alla proporzione; con «recupera il residuo» il certificato recupera tutto
   quanto manca (di norma l'ultimo SAL). Il programma non verifica le condizioni dell'art.
   1-bis (richiesta, motivazione, PNRR, scadenza): restano responsabilità della stazione
   appaltante.
3. **Penali come importi datati.** Ogni penale ha data, motivo, importo e, se calcolata
   per ritardo, giorni e per mille. Va nel primo certificato il cui SAL arriva alla sua
   data (`lavori a tutto il`). Il calcolo non impone i limiti di legge: la validazione
   segnala per mille fuori dall'intervallo (avviso) e totale oltre il 10 % dell'importo
   contrattuale (errore).
4. **Certificato**: `imponibile = importo del SAL − recupero − penali`;
   `IVA = imponibile × aliquota` se il contratto la indica; `totale = imponibile + IVA`.
   Un imponibile negativo è segnalato come avviso, non rifiutato.
5. **Congelamento all'emissione**: il SAL emesso conserva il certificato e l'elenco delle
   penali applicate; le penali applicate non si possono rimuovere e la verifica di
   integrità confronta il certificato congelato con il ricalcolo
   (`contabilita.certificato_alterato`). I SAL emessi con lo schema 1 non hanno il
   certificato congelato: viene ricalcolato e l'interfaccia lo indica.
6. **Sommario del registro**: per articolo (codice e prezzo), quantità e importi lordi
   per ogni SAL, totali per SAL e complessivo; solo le registrazioni comprese in un SAL.
7. **Schema versione 2** con migrazione identità dalla 1 (i campi nuovi sono facoltativi).
   Nuova fixture `progetto_esempio_v2.json`, congelata come la v1.
8. **Comandi nuovi** (33 in tutto): `imposta_anticipazione`, `registra_penale`,
   `rimuovi_penale`, `imposta_sal`; `imposta_contratto` accetta `aliquota_iva` (`null` la
   rimuove). Report nuovi: `certificato` e `sommario`.

## Conseguenze e limiti

* Valori verificati a mano su un contratto di prova (100.000 €, anticipazione 20 %
  recuperata al 20 %, IVA 10 %, penale di 3 giorni allo 0,5 ‰) nei test del core.
* Non gestiti: ritenute fiscali o previdenziali sul certificato, split payment, IVA
  diverse per parti dell'opera, interessi sull'anticipazione, revoca dell'anticipazione
  per ritardo (art. 125, comma 1), premi di accelerazione, rata di saldo e svincolo della
  ritenuta di garanzia.
* Il programma non decide se e quanto applicare: anticipazione, recupero, penali e IVA
  sono dati del contratto e scelte del RUP e del direttore dei lavori.
