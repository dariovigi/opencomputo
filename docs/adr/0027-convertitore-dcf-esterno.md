# ADR 0027 — Convertitore DCF esterno

* Stato: accettato
* Data: 2026-11-21

## Contesto

Il titolare dispone di un convertitore DCF/OCP di terzi (CLI `dcf`, pacchetto
«primus-dcf 0.1.0»), con una guida di integrazione come plugin MCP di Claude, e vuole
usarlo **dentro OpenComputo** per importare i documenti `.dcf`. Il programma non può
essere caricato su piattaforme esterne (non è autorizzato a farlo): non è stato visto né
provato durante lo sviluppo, che si basa solo sul contratto della riga di comando
descritto nella guida:

* `dcf convert <origine> <uscita> [--report <file>]`, direzione rilevata dal contenuto;
* uscita 0 se riuscito, 1 in caso di errore; stderr `errore: <descrizione>` solo in caso
  di errore; stdout il resoconto in UTF-8;
* il risultato è un progetto OpenComputo in JSON (schema 7).

Il lettore DCF di OpenComputo non decifra i flussi protetti né aggira password
(ADR 0006). OpenComputo non sa cosa faccia il convertitore esterno con i documenti.

## Decisioni

1. **Programma scelto dall'utente, mai incluso.** OpenComputo non contiene, non
   distribuisce e non scarica il convertitore. L'utente indica il percorso assoluto del
   programma (richiesta `convertitore_imposta`), salvato in `convertitore.json` nella
   cartella di configurazione (la stessa dell'assistente, ADR 0025). Prima di sceglierlo
   e a ogni conversione l'interfaccia mostra un avviso: programma di terzi, non verificato
   da OpenComputo, elaborazione locale, da usare solo se si è autorizzati a usarlo sui
   documenti che si convertono. ADR 0006 resta valido per il codice di OpenComputo: la
   responsabilità di ciò che fa il programma esterno è di chi lo installa e lo usa.
2. **Solo il contratto, nessun formato.** La conversione passa da
   `opencomputo-application::convertitore`, che lancia
   `programma convert <origine> <cartella temporanea>/convertito.ocp --report <…>`.
3. **Il risultato è un file non fidato.** Il `.ocp` prodotto si legge come qualunque
   file (`opencomputo_storage::apri`: limiti di dimensione, migrazioni dello schema) e
   la sessione lo valida come ogni progetto (ADR 0026). Il rapporto d'importazione
   segnala sempre che il progetto viene da un programma esterno.
4. **Esecuzione prudente.**
   * Argomenti come lista, mai una shell: spazi e caratteri speciali dei percorsi non
     vengono interpretati.
   * Ambiente ridotto alle variabili indispensabili (`PATH`, cartella personale e
     temporanea, lingua, cartella di sistema su Windows): eventuali credenziali presenti
     nell'ambiente di OpenComputo non arrivano al programma.
   * Cartella di lavoro temporanea privata (permessi 0700 su Unix, nome nuovo a ogni
     conversione), rimossa alla fine anche in caso di errore.
   * Durata massima di 120 secondi, poi il processo viene terminato; su Windows nessuna
     finestra di console.
   * Documento d'origine fino a 256 MB (come gli altri import); stdout, stderr e
     resoconto letti fino a 1 MB, riportati fino a 100.000 caratteri, senza caratteri
     di controllo; l'interfaccia li mostra solo come testo.
5. **Il progetto aperto resta se la conversione fallisce.** Il servizio sostituisce la
   sessione solo a conversione riuscita, e l'interfaccia non chiude il progetto prima.
6. **Niente plugin MCP.** Il server MCP della guida (per Claude Desktop e Claude Code)
   non fa parte di OpenComputo: l'integrazione passa dal servizio JSON, come ogni altra
   funzione dell'interfaccia (ADR 0009).

## Conseguenze

* Richieste del servizio: `convertitore_stato`, `convertitore_imposta`,
  `importa_con_convertitore`; nel menu File: «Importa DCF con convertitore esterno…» e
  «Convertitore DCF esterno…».
* Prove senza il programma vero: il test `opencomputo-agent-api/tests/convertitore.rs`
  fa anche da convertitore finto (stesso eseguibile, senza l'harness standard), così la
  prova è uguale su Linux, macOS e Windows; l'interfaccia è provata con uno script Node.
  Il funzionamento con il convertitore vero va verificato dal titolare sul suo computer.
* Se il formato del resoconto o il contratto del programma cambiano, cambia solo
  `convertitore.rs`.
