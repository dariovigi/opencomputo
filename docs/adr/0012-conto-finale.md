# ADR 0012 — Stato finale, conto finale, collaudo e rata di saldo

* Stato: accettato
* Data: 2026-10-21

## Contesto

Con anticipazione, penali e certificati di pagamento (ADR 0011) mancava la chiusura
della contabilità. Il titolare ha scelto di completarla. Riferimenti verificati il
2026-10-21:

* **Allegato II.14 al D.Lgs. 36/2023**:
  * il direttore dei lavori certifica l'ultimazione dei lavori; dal certificato
    decorrono le penali per il ritardo e il termine per le lavorazioni di piccola
    entità;
  * il conto finale è compilato dal direttore dei lavori dopo l'ultimazione e
    trasmesso al RUP con una relazione (art. 12, comma 1, lett. e, e comma 5);
  * l'esecutore lo firma entro trenta giorni dall'invito del RUP. Non può iscrivere
    riserve nuove e deve confermare quelle già iscritte nel registro; se non firma,
    il conto si intende definitivamente accettato (art. 7, commi 3 e 4);
  * il certificato di regolare esecuzione è emesso dal direttore dei lavori entro
    tre mesi dall'ultimazione (art. 28, comma 3). Sostituisce il collaudo nei casi
    previsti, tra cui i lavori fino a 1 milione di euro (art. 28, comma 1);
  * la rata di saldo si paga con il certificato di collaudo o di regolare
    esecuzione (artt. 27 e 28).
* **D.Lgs. 36/2023, art. 116, comma 2**: collaudo entro sei mesi dall'ultimazione,
  fino a un anno nei casi di particolare complessità. Il certificato è provvisorio e
  diventa definitivo dopo due anni.
* **Art. 11, comma 6**: le ritenute dello 0,50 % si svincolano solo con la
  liquidazione finale, dopo l'approvazione del collaudo o della verifica di
  conformità e previo DURC.
* **Art. 117, comma 9**: il pagamento della rata di saldo prima dell'accettazione
  definitiva dell'opera richiede una garanzia pari alla rata più gli interessi legali
  (parere MIT n. 4150/2026: con il certificato di regolare esecuzione l'opera è già
  accettata).

## Decisioni

1. **Stato finale = SAL emessi + registrazioni non comprese nei SAL.** Le misurazioni
   eseguite dopo l'ultimo SAL non richiedono un SAL «finale»: confluiscono nel conto
   finale e quindi nella rata di saldo. Con il conto in bozza la composizione è
   dinamica; all'emissione l'elenco viene fissato.
2. **Calcolo** con le stesse formule dei SAL sui totali finali:
   * credito residuo = importo netto finale − importi dei SAL emessi; comprende le
     ritenute di garanzia, mostrate a parte come importo da svincolare;
   * rata di saldo = credito residuo − anticipazione non recuperata − penali non
     applicate nei certificati (tutte, qualunque sia la data);
   * IVA come nei certificati.

   Quadratura verificata nei test: anticipazione + imponibili di tutti i
   certificati = importo netto finale − penali.
3. **Procedura in due passi**:
   * *redazione in bozza*: data di ultimazione, data del conto, note. Richiede che
     nessun SAL sia in bozza e che l'ultimazione non preceda l'ultimo SAL; con il
     conto in bozza non si redigono altri SAL;
   * *emissione*: congela riepilogo, registrazioni dello stato finale e penali
     applicate, e **chiude la contabilità**. Niente più registrazioni, modifiche,
     SAL o penali; la verifica di integrità segnala le alterazioni
     (`contabilita.conto_finale_alterato`). L'annullamento resta possibile solo con
     «Annulla» della sessione.
4. **Firma e collaudo** si registrano dopo l'emissione, come dati del conto:
   * firma: data e testo delle riserve confermate;
   * collaudo: tipo (collaudo o regolare esecuzione), data e approvazione.

   Non modificano gli importi.
5. **Termini come avvisi o informazioni**, non come vincoli:
   * misurazioni successive all'ultimazione;
   * certificato di regolare esecuzione oltre tre mesi;
   * collaudo oltre sei mesi o un anno;
   * certificato di regolare esecuzione oltre 1 milione;
   * conto da firmare, riserve confermate, collaudo da approvare.
6. **Schema versione 3**, con migrazione identità dalla 2 e fixture
   `progetto_esempio_v3.json` congelata.
7. **Comandi nuovi** (38 in tutto): `redigi_conto_finale`, `emetti_conto_finale`,
   `annulla_conto_finale`, `firma_conto_finale`, `registra_collaudo`. Report nuovo:
   `conto_finale`. Il sommario del registro ha una colonna per lo stato finale.

## Conseguenze e limiti

* Valori verificati a mano su un contratto di prova di 13.000 € (rata di saldo
  764,48 €).
* Non gestiti:
  * relazione sul conto finale strutturata, oltre al testo libero delle note;
  * importi delle riserve e loro definizione (accordo bonario);
  * collaudi in corso d'opera;
  * interessi per ritardato pagamento;
  * garanzia della rata di saldo come dato;
  * DURC;
  * passaggio del collaudo da provvisorio a definitivo;
  * ritenute fiscali e split payment.
* La data dell'invito del RUP non è registrata, quindi il termine di trenta giorni
  per la firma non è verificato.
