# ADR 0027 — Convertitore DCF/OCP esterno

* Stato: accettato (rivisto il 2026-11-21 sulla seconda guida di integrazione)
* Data: 2026-11-21

## Contesto

Il titolare dispone di un convertitore DCF/OCP di terzi (CLI `dcf`, pacchetto
«primus-dcf 0.1.0», binari `dcf-linux` e `dcf.exe`) e vuole usarlo **dentro
OpenComputo**. Il programma non può essere caricato su piattaforme esterne (il titolare
non è autorizzato a farlo): non è stato visto né provato durante lo sviluppo, che si basa
solo sulla guida «Il convertitore DCF/OCP come plugin del vostro programma» e sul modulo
ponte di riferimento `dcf_bridge.py` (Python; il bytecode allegato coincide con il
sorgente). Contratto della CLI secondo la guida:

* `dcf convert <input> <output> [--report <file>] [--template <t.dcf>]`: conversione
  nei due sensi; la direzione è decisa dal **contenuto** dell'input (JSON → `.dcf`,
  altrimenti → `.ocp`), mai dall'estensione dell'output;
* `dcf confronta <a> <b>`: verifica di fedeltà tra un `.dcf` e un `.ocp` in qualunque
  ordine (prezzi, quantità, importi, SAL), verdetto «PREZZI PRESERVATI» o elenco delle
  differenze; due file dello stesso formato sono rifiutati;
* `dcf info <file>`: informazioni su un `.dcf`;
* uscita 0 riuscito, 1 errore; stderr `errore: <descrizione>` solo in caso di errore;
  stdout esito e resoconto in UTF-8; argomenti come lista, mai una shell;
* il ponte: percorsi assoluti, file fino a 200 MB, 120 secondi per invocazione,
  programma indicato da `DCF_BIN`, output accanto all'input con l'estensione opposta e
  resoconto in `<output>.report.txt`.

Il lettore DCF di OpenComputo non decifra i flussi protetti né aggira password
(ADR 0006) e lo scrittore DCF di OpenComputo non produce protezioni (ADR 0016).
OpenComputo non sa che cosa faccia il convertitore esterno con i documenti.

## Decisioni

1. **Il ponte riscritto in Rust, non incluso in Python.** OpenComputo non carica plugin
   Python: includere `dcf_bridge.py` richiederebbe Python sui computer degli utenti
   (anche Windows). La guida prevede di «replicarne il pattern nel linguaggio del
   programma»: `opencomputo-application::convertitore` lo fa funzione per funzione
   (`importa`/`esporta` per `converti`, `confronta` per `verifica`, `info`).
2. **Programma scelto dall'utente, mai incluso.** OpenComputo non contiene, non
   distribuisce e non scarica il convertitore. Il percorso assoluto del programma si
   imposta dall'interfaccia (richiesta `convertitore_imposta`, salvato in
   `convertitore.json` nella cartella di configurazione, ADR 0025); in sua assenza vale
   `DCF_BIN` della guida, solo se è un percorso assoluto. Prima di sceglierlo e a ogni
   conversione l'interfaccia mostra un avviso: programma di terzi, non verificato da
   OpenComputo, elaborazione locale, da usare solo se si è autorizzati a usarlo sui
   documenti che si convertono. ADR 0006 e 0016 restano validi per il codice di
   OpenComputo: la responsabilità di ciò che fa il programma esterno è di chi lo
   installa e lo usa.
3. **Import (`.dcf` → progetto).** Conversione in una cartella temporanea; il `.ocp`
   prodotto si legge come qualunque file (`opencomputo_storage::apri`: limiti,
   migrazioni) e la sessione lo valida (ADR 0026). Nessun file viene scritto accanto al
   documento d'origine: il progetto si apre non salvato. Un input che è già un progetto
   OpenComputo (JSON o gzip) è rifiutato: la CLI lo convertirebbe in `.dcf`. Il progetto
   aperto resta se la conversione fallisce.
4. **Export (progetto → `.dcf`).** Il progetto si scrive in JSON in chiaro (la CLI
   riconosce un `.ocp` dal primo byte `{`; schema 7, verificato da un test: se lo schema
   cambia l'export va rivisto) e si converte nella cartella temporanea; il risultato deve
   esistere e non essere JSON. Il `.dcf` e il resoconto `<nome>.report.txt` accanto (come
   il ponte) si scrivono in modo atomico solo a conversione riuscita: una destinazione
   esistente resta intatta se la conversione fallisce. L'opzione `--template` non è usata.
5. **Verifica sempre.** Dopo ogni conversione OpenComputo esegue `confronta` tra i due
   file e mostra il verdetto con il dettaglio (all'import anche come segnalazione del
   rapporto). Un confronto con differenze non annulla la conversione; un confronto
   impossibile è riportato con il motivo.
6. **Esecuzione prudente.** Oltre alle misure del ponte (lista di argomenti, percorsi
   assoluti, 200 MB, 120 secondi):
   * ambiente ridotto alle variabili indispensabili (`PATH`, cartella personale e
     temporanea, lingua, cartella di sistema su Windows): eventuali credenziali
     dell'ambiente di OpenComputo non arrivano al programma;
   * cartella di lavoro temporanea privata (0700 su Unix, nome nuovo a ogni invocazione),
     rimossa alla fine anche in caso di errore; su Windows nessuna finestra di console;
   * stdout, stderr e resoconto letti fino a 1 MB, riportati fino a 100.000 caratteri,
     senza caratteri di controllo; l'interfaccia li mostra solo come testo.
7. **Niente plugin MCP né microservizio.** L'integrazione passa dal servizio JSON, come
   ogni altra funzione dell'interfaccia (ADR 0009).

## Conseguenze

* Richieste del servizio: `convertitore_stato`, `convertitore_imposta`,
  `importa_con_convertitore`, `esporta_con_convertitore`, `convertitore_confronta`,
  `convertitore_info`. Menu File: «Importa DCF con convertitore esterno…», «Esporta DCF
  con convertitore esterno…», «Convertitore DCF esterno…».
* Prove senza il programma vero: il test `opencomputo-agent-api/tests/convertitore.rs`
  fa anche da convertitore finto (stesso eseguibile, senza l'harness standard: direzione
  dal contenuto, `confronta`, `info`), così la prova è uguale su Linux, macOS e Windows;
  l'interfaccia è provata con uno script Node. I casi del piano di collaudo della guida
  (§10) sono coperti con il finto; il funzionamento con il convertitore vero, il
  round-trip completo sui file campione e la prova su macOS e Windows vanno fatti dal
  titolare sul suo computer.
* Se il contratto cambia, cambia solo `convertitore.rs`.
