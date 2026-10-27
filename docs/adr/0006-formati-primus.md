# ADR 0006 — Formati dell'ecosistema PriMus: XPWE e DCF non protetti

* Stato: **accettato** (Fase 2). XPWE in lettura e scrittura. DCF in sola lettura e solo per i
  contenuti non protetti, su decisione del titolare del progetto del 2026-10-20.
  Modificato dall'[ADR 0016](0016-export-dcf.md) (2026-10-21): export sperimentale
  dell'elenco prezzi in DCF in chiaro; computo e contabilità restano esclusi.
* Data: 2026-10-20

## Contesto

Il piano prevedeva per la Fase 2 due crate: `opencomputo-format-xpwe` (import/export
XPWE) e `opencomputo-dcf-parser` (lettura dei DCF). Non esiste una specifica pubblica di
nessuno dei due formati e il titolare del progetto non dispone di file di esempio.

Ricerca svolta il 2026-10-20:

* **Prezzari regionali**: le Regioni li pubblicano in XML aperto, ODS, XLS e PDF, come
  previsto dall'Allegato I.14 al D.Lgs. 36/2023 (verificati Veneto, Lombardia,
  Toscana). Nessuna Regione verificata pubblica XPWE.
* **ACCA** distribuisce i prezzari regionali in formato DCF. Le condizioni d'uso
  (<https://www.acca.it/prezzari-condizioni-uso>) dicono: «È ESPRESSAMENTE VIETATO
  convertire i suddetti Prezzari, prodotti da ACCA nel formato standard DCF» e «È
  ESPRESSAMENTE VIETATO usare i suddetti Prezzari … con altri software di Computo metrico
  e Contabilità dei lavori».
* **LeenO** (<https://github.com/giuserpe/leeno>, LGPL-2.1), estensione open source di
  LibreOffice, importa ed esporta XPWE. Il suo codice è l'unica fonte pubblica
  dettagliata sulla struttura di XPWE. LeenO afferma inoltre che «i file DCF sono
  protetti da DRM».

## Decisione: XPWE

1. La struttura di XPWE è **dedotta** dal codice di importazione di LeenO (commit
   `40966e3`, file `LeenoImport_XPWE.py`): nomi degli elementi, gerarchia, significato di
   alcuni campi (`Flags`, `IDVV`, incidenze in percentuale). Da LeenO si ricavano solo
   fatti sul formato, che non sono oggetto di diritto d'autore. L'implementazione in Rust è
   scritta da zero e non deriva dal codice di LeenO; per questo non si applica la LGPL.
2. **Verifica parziale con un export reale** (2026-10-20): un documento vuoto esportato
   da PriMus-DCF (formato 5.01) conferma radice, testata (`TipoFormato` = `XMLPwe`,
   `Versione` = `5.01`), dati generali, configurazione dei numeri nella forma `L.D|0`,
   l'assenza dell'elenco prezzi quando è vuoto e i fine riga CRLF. Ha anche fatto
   emergere un errore: l'import riconosceva le sezioni per posizione e scambiava le voci
   di computo per l'elenco prezzi. Ora le riconosce dal contenuto. Restano **non
   verificate** voci di elenco, voci di computo e righi di misura, che il file non
   contiene. Il crate lo dichiara: ogni elemento interpretato in modo
   incerto è documentato nel modulo `struttura` e ogni dato non riconosciuto è
   conservato e segnalato, mai scartato in silenzio.
3. **Import tollerante**: gli elementi sconosciuti finiscono nei `dati_originali` della
   provenienza e sono segnalati come anomalie. Le quantità dichiarate nel file sono
   confrontate con quelle ricalcolate e ogni differenza viene segnalata. I riferimenti
   non risolvibili (es. `IDEP` inesistente) producono un'anomalia di gravità
   «errore» con la posizione nel file.
4. **Export sperimentale**: genera un XPWE con la struttura dedotta e reinserisce i dati
   originali conservati. L'andata e ritorno (OpenComputo → XPWE → OpenComputo) è
   verificata dai test, mentre **l'apertura in PriMus non è verificata**. Il rapporto di
   export segnala ogni approssimazione (es. formule dei righi riscritte come
   espressioni numeriche).
5. Le fixture XPWE del repository sono **sintetiche**, scritte a mano, senza dati reali
   di clienti o progetti.

## Decisione: DCF in sola lettura, solo contenuti non protetti

Prima proposta (2026-10-20): non implementare il DCF, perché LeenO lo descrive come
«protetto da DRM». Il titolare del progetto ha deciso di supportarlo per compatibilità
e ha fornito tre file per l'analisi: due prezzari regionali distribuiti da ACCA e un
documento di esempio di contabilità. I file **non** sono nel repository.

### Risultato dell'analisi (2026-10-20)

* Il DCF è un **contenitore a settori** da 4096 byte (firma `AAMVHFSS`, testata
  `AASFSSGN`). I flussi di dati iniziano con `AACS` e sono divisi in pezzi zlib da
  1 MiB al massimo. La struttura completa è documentata nel modulo `contenitore` del
  crate `opencomputo-dcf-parser`.
* **I prezzari non sono cifrati**: contengono XML in chiaro (`CollectionEP`,
  `DatiGenerali`, …). Leggerli richiede solo di interpretare il formato.
* **Nel documento di contabilità i dati sono protetti**: 10 flussi iniziano con
  «Copyright ACCA software S.p.A.» seguito da dati con entropia di circa 7,9 bit/byte,
  cioè cifrati, e contengono proprio i dati del documento. Gli altri flussi sono dati
  accessori.
* `InfoDoc` può contenere una password di apertura (`PswOpen`) e una di modifica
  (`PswMod`).

### Regole del lettore

1. Il lettore interpreta il formato del contenitore e l'XML in chiaro. **Non decifra
   mai** i flussi protetti e non contiene alcun codice di decifratura.
2. Se i dati del documento sono solo in flussi protetti, l'import si ferma
   (`ErroreDcf::Protetto`) e suggerisce l'export XPWE da PriMus. Se sono protette
   solo alcune parti, l'import prosegue e segnala quanti flussi non ha letto.
3. Una password di apertura è rispettata: il file viene rifiutato
   (`ErroreDcf::ProtettoDaPassword`). La sola password di modifica non impedisce la
   lettura, perché OpenComputo non scrive DCF.
4. Sono importati elenco prezzi, capitoli, categorie e dati generali. Voci di computo
   e analisi non sono importate: non c'è un documento non protetto su cui verificarle.
5. Limiti contro i file «bomba»: pezzi fino a 16 MiB, al massimo 4096 pezzi per
   flusso, 1 GiB decompresso per file.

### Verifica sui file reali (non inclusi nel repository)

Test ignorato `tests/file_reali.rs`, eseguito il 2026-10-20 sui tre file:

| File | Esito |
|------|-------|
| prezzario «Articoli» (2 MB, 513 settori) | 31.755 voci, 770 capitoli, 3 categorie, 0 errori, 0 problemi di validazione |
| prezzario «Analisi» (3 MB, 769 settori) | 31.755 voci, 770 capitoli, 3 categorie, 136.256 elementi di analisi non importati |
| documento di contabilità (0,8 MB) | rifiutato: 10 flussi protetti |

Controllo indipendente: la somma dei prezzi `Prz1` calcolata da uno script Python
separato, con aritmetica decimale, coincide al centesimo con quella del crate Rust.

### Condizioni d'uso dei contenuti

Le condizioni d'uso dei prezzari ACCA (<https://www.acca.it/prezzari-condizioni-uso>)
vietano di convertirli e di usarli con software di terzi. Il lettore è uno strumento di
interoperabilità di formato: come qualsiasi programma che legge un formato di file, non
decide quali documenti l'utente può importare. Rispettare le condizioni d'uso dei
singoli documenti resta responsabilità di chi li importa. La documentazione del crate lo
dice esplicitamente. Le fonti alternative senza questi vincoli sono i prezzari pubblicati
direttamente dalle Regioni in XML aperto (Allegato I.14 al D.Lgs. 36/2023).

Questo testo non costituisce consulenza legale.

## Da verificare con file PriMus reali

* nome di `PweElencoPrezzi`, `PweDGAnalisi` e `PweVCMisure` e struttura di `EPItem`,
  `VCItem`, `RGItem` (verificati invece radice, testata, `PweDGProgetto`,
  `PweDGDatiGenerali`, `PweDGConfigNumeri`, `PweVociComputo`);
* significato completo dei `Flags` (noti: voce di sicurezza `134217728` sulle voci di
  elenco, rigo parziale `2` sui righi di misura), di `TipoEP` e di `TipoDocumento`;
* formato dei numeri nelle voci (punto o virgola decimale) e codifica dei caratteri non
  ASCII (l'export osservato è solo ASCII, senza dichiarazione di codifica);
* struttura delle analisi prezzi (`PweEPAnalisi`) e della contabilità;
* DCF: significato dei bit di `Fgs` (la natura «sicurezza» usa per ipotesi lo stesso bit
  di XPWE, ma nessuna voce dei file analizzati lo imposta); struttura di `CollectionVC`
  in un documento non protetto;
* comportamento di PriMus con espressioni nei campi di misura e con tariffe duplicate.
