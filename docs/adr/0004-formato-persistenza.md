# ADR 0004 — Formato di persistenza interno

* Stato: accettato in via provvisoria (Fase 1)
* Data: 2026-10-20

## Decisione

* Il formato interno è indipendente da DCF e XPWE.
* Documento JSON con involucro `{ formato, versione_schema, progetto }`.
* Numeri come stringhe decimali esatte. Gli importi sono scritti con almeno 2 decimali
  (`"0.00"`, `"12.50"`; decisione del 2026-10-20): la scala viene solo estesa, mai
  ridotta, quindi nessun valore cambia. In lettura è accettata qualsiasi scala, per cui
  la fixture v1 (che contiene `"0"`) resta valida senza migrazione.
* Migrazioni esplicite `Value → Value`, una per versione; i documenti di versione futura
  vengono rifiutati.
* Una fixture per versione dello schema, mai rigenerata, protegge la retrocompatibilità.

## Da decidere in `opencomputo-storage` (Fase 4)

* Contenitore su disco: JSON singolo, archivio compresso o SQLite (utile per prezzari
  con decine di migliaia di voci e per il salvataggio incrementale).
* Salvataggio atomico (file temporaneo + rinomina) e copie di sicurezza.
* Eventuale cronologia delle versioni del documento.
