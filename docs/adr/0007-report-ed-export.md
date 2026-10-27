# ADR 0007 — Report ed export in PDF, XLSX e CSV

* Stato: accettato (Fase 3)
* Data: 2026-10-20

## Contesto

La Fase 3 deve produrre i documenti di uso quotidiano (computo metrico estimativo,
elenco prezzi, libretto delle misure, registro di contabilità, stati di avanzamento) in
PDF per la stampa, in XLSX per la rielaborazione e in CSV per l'interscambio. I numeri
devono coincidere al centesimo con quelli del core in tutti i formati.

## Decisioni

1. **Un modello intermedio, tre scrittori.** Ogni report è costruito una volta sola come
   `Documento` (sezioni con tabelle di righe tipizzate: voce, rigo, commento, sommano,
   gruppo, totale) dai valori calcolati dal core. PDF, XLSX e CSV rappresentano lo stesso
   documento, quindi i numeri coincidono per costruzione. I report si verificano sul
   modello, senza leggere i file prodotti.
2. **I report non ricalcolano.** Quantità, importi, totali e riepiloghi vengono da
   `Computo::calcola`, dal libretto, dal registro e dai SAL. Fanno eccezione solo le
   incidenze percentuali dei riepiloghi (2 decimali, metà lontano da zero). Per un SAL
   emesso il prospetto è quello congelato all'emissione; un SAL in bozza è ricalcolato e
   marcato «BOZZA».
3. **XLSX con valori, non formule.** I numeri sono scritti come valori con il formato
   dei decimali corrispondenti: il foglio riproduce il report e non dipende dagli
   arrotondamenti di Excel, che differiscono da quelli del core. La conversione da
   decimale a virgola mobile è imposta dal formato XLSX e conserva le 15 cifre
   significative mostrate da Excel. Libreria: `rust_xlsxwriter` (MIT OR Apache-2.0).
4. **PDF scritto in proprio**, senza librerie di impaginazione. Pagine A4, font standard
   Helvetica con codifica WinAnsi, tabelle con intestazione ripetuta a ogni pagina, testo
   a capo, righe lunghe spezzate tra le pagine, «Pagina n di N». Motivi: nessuna
   dipendenza pesante, compilazione WebAssembly, uscita deterministica (stesso documento
   → stessi byte) e controllo completo dell'impaginazione. Le coordinate sono intere, in
   centesimi di punto, perché il workspace vieta l'aritmetica in virgola mobile. Le
   larghezze dei caratteri sono le metriche Adobe Core 14 (liberamente distribuibili).
5. **CSV per Excel in italiano**: separatore `;`, virgola decimale, nessun separatore
   delle migliaia, UTF-8 con BOM, righe CRLF, colonna «Tipo» con il ruolo della riga.
6. **Libretto senza prezzi**, come nella prassi: le quantità del libretto passano al
   registro, che riporta prezzi e importi.

## Conseguenze e limiti

* I font non sono incorporati: il PDF non è PDF/A. I caratteri fuori da Windows-1252
  sono trascritti se sono simboli tecnici frequenti (`≤` → `<=`, `λ` → `lambda`,
  `μ` → `µ`…); gli altri diventano `?` e sono contati nel risultato. Sui due prezzari
  regionali analizzati (31.755 voci ciascuno) resta un solo carattere sostituito per
  prezzario: un carattere di un'area d'uso privato Unicode.
* Il PDF/A e l'incorporamento di un font Unicode (es. DejaVu o Noto, licenze libere)
  richiedono il subsetting dei font: rinviati, da valutare se servono documenti da
  conservare a norma.
* I modelli di stampa non sono configurabili (colonne, intestazioni, logo): la
  configurazione è rinviata alla GUI (Fase 5).
* Mancano il sommario del registro per articolo, il certificato di pagamento, il quadro
  economico e la stima dell'incidenza della manodopera per voce, che dipendono da
  funzionalità del core non ancora presenti.
