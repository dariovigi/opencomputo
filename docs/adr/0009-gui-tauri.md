# ADR 0009 — Interfaccia grafica con Tauri v2

* Stato: accettato (Fase 5)
* Data: 2026-10-20
* Decisione del titolare: Tauri v2, preferito a egui (2026-10-20)

## Contesto

La Fase 5 aggiunge l'applicazione desktop: progetto ed elenco prezzi, computo con righi
di misura, contabilità e SAL, report ed export. È un'applicazione di inserimento dati:
tabelle lunghe e modificabili, testo descrittivo esteso, copia e incolla con i fogli di
calcolo, anteprime di stampa. Le alternative valutate erano Tauri v2 (finestre native
con interfaccia web) ed egui/eframe (interfaccia disegnata in Rust).

## Decisioni

1. **Tauri v2** (MIT OR Apache-2.0). Rispetto a egui offre resa del testo e dei font,
   campi di testo e selezione nativi, tabelle grandi, accessibilità e copia/incolla
   migliori, con finestre native su macOS (WKWebView), Windows (WebView2) e Linux
   (WebKitGTK). Il prezzo è un secondo linguaggio (JavaScript) e motori web diversi per
   sistema.
2. **Un solo canale con il motore: il protocollo del servizio JSON**
   ([ADR 0008](0008-casi-uso-storage-cli.md)). Il lato Rust dell'applicazione tiene un
   `Servizio` di `opencomputo-agent-api` ed espone un solo comando Tauri, `richiesta`,
   che riceve la stessa richiesta JSON di `opencomputo servizio` e ne restituisce la
   risposta. Interfaccia grafica e agenti AI passano quindi dagli stessi casi d'uso, con
   le stesse validazioni, e ogni funzione della GUI è disponibile anche agli agenti.
   Per la GUI il servizio ha richieste di sola lettura («viste») su progetto, elenco
   prezzi, computo, contabilità e anteprima dei report.
3. **JavaScript senza strumenti di build**: moduli ES serviti così come sono, nessun
   framework, nessun `npm` per compilare l'applicazione. Basta `cargo run`, anche sul
   Mac. Un piccolo modulo di adattamento (`piattaforma.js`) isola le API di Tauri
   (richieste, dialoghi dei file, eventi, chiusura della finestra), così l'interfaccia
   può essere provata in un browser e in futuro servita sul web con il core compilato
   in WebAssembly.
4. **Sicurezza**:
   * il testo proveniente dai file (descrizioni, codici) entra nella pagina solo come
     `textContent`, mai come HTML: un file XPWE o DCF malevolo non può eseguire codice
     in una finestra che ha accesso ai file;
   * Content Security Policy restrittiva (solo risorse locali, nessuno script in linea,
     nessun accesso di rete);
   * permessi Tauri minimi: dialoghi dei file, eventi, finestre; nessun accesso diretto
     al filesystem dal JavaScript, che passa sempre dal servizio.
5. **Più finestre su un progetto**: elenco prezzi, computo o anteprima possono essere
   aperti in finestre separate che condividono lo stesso progetto; dopo ogni modifica
   il lato Rust emette l'evento `progetto-aggiornato` e le altre finestre si aggiornano.
   Un progetto per istanza dell'applicazione.
6. **Test**:
   * il lato Rust (instradamento delle richieste, eventi) con test Rust;
   * l'interfaccia con test end-to-end in Chromium (Playwright) che usano il motore
     **reale**: le richieste della pagina sono inoltrate a un processo
     `opencomputo servizio`, quindi i test vedono gli stessi totali e gli stessi errori
     dell'applicazione;
   * una prova dell'applicazione compilata su Linux (WebKitGTK) sotto Xvfb, con
     schermate controllate a vista.

## Conseguenze e limiti

* Su Linux la compilazione richiede le librerie di sviluppo di WebKitGTK 4.1; su macOS e
  Windows nessuna dipendenza aggiuntiva. `cargo build --workspace` su Linux le richiede
  perché la GUI è un membro del workspace.
* Senza framework il codice dell'interfaccia è più esplicito; se crescerà molto si
  valuterà un framework leggero con una fase di build.
* I motori web differiscono (WebKit, Chromium): i test automatici girano in Chromium e
  la prova a vista in WebKitGTK; macOS e Windows vanno provati sulle macchine reali.
* Il pacchetto installabile (firma, notarizzazione macOS, installer Windows) non fa parte
  della Fase 5.
