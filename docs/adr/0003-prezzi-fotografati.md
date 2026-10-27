# ADR 0003 — Prezzi fotografati nelle voci

* Stato: accettato (Fase 1)
* Data: 2026-10-20

## Contesto

Requisito: l'aggiornamento di un prezzario non deve modificare retroattivamente le voci
già inserite nei computi e nei documenti contabili. Nei formati di interscambio diffusi
le voci di computo tendono invece a riferirsi a una voce di elenco prezzi per
identificativo (da verificare nella Fase 2 su XPWE), per cui una modifica dell'elenco si
riflette sul computo.

## Decisione

* Ogni voce di computo e ogni registrazione contabile contiene un `PrezzoApplicato`:
  copia di codice, descrizioni, unità di misura, prezzo, natura e incidenze, più un
  riferimento all'origine usato solo per tracciabilità.
* Il riallineamento è un'operazione esplicita (`PrezzoApplicato::riallinea`) che
  restituisce le differenze applicate; la validazione segnala le voci disallineate con
  gravità *informazione*.
* Copiare una voce dal prezzario al computo (`Computo::aggiungi_da_prezzario`) lascia
  la sorgente invariata.

## Conseguenze

* Nessuna modifica silenziosa degli importi.
* All'esportazione XPWE occorrerà ricostruire un elenco prezzi dalle fotografie,
  gestendo i casi in cui lo stesso codice ha dati diversi in voci diverse (da progettare
  nella Fase 2).
* Maggiore occupazione di memoria/spazio, trascurabile per i volumi attesi.
