# Collaudo del motore

Script Python (solo libreria standard) che pilotano `opencomputo servizio` per cercare
difetti: panico, blocchi, invarianti violate. Prima: `cargo build -p opencomputo-agent-api`.

| Script | Cosa prova |
|--------|------------|
| `sequenze.py` | Sequenze casuali di comandi (validi e mutati) sul progetto di esempio. Dopo ogni comando accettato: totali = somma delle voci, quantità = somma dei righi, nessun errore di validazione, annulla/ripristina esatti. Ogni 80 passi: tutti i report in PDF, XLSX e CSV, salva e riapri, export e reimport XPWE con lo stesso totale. `SEED=1 PASSI=1500 python3 sequenze.py` |
| `corrotti.py` | File di progetto validi per lo schema ma incoerenti (cicli di capitoli e WBS, numeri enormi, identificativi duplicati, SAL alterati…): ogni richiesta risponde senza crash né blocchi. |
| `contabilita.py` | Scenari casuali di contabilità (ribasso, ritenuta, IVA, anticipazione, penali, nuovi prezzi, SAL, conto finale) con il ricalcolo indipendente di riepiloghi e certificati. `N=60 python3 contabilita.py` |
| `caratteri.py` | Caratteri speciali in PDF, XLSX, CSV, XPWE e DCF. |
| `robustezza.py` | Righe malformate al servizio, file vuoti, troncati, binari, gzip, versioni future, salvataggi impossibili. |
| `prestazioni.py` | Tempi con un computo grande (`N=3000` voci con catene di «vedi voce»). |

I collaudi lunghi in Rust sono test ignorati (`cargo test -- --ignored`): `collaudo_formule`,
`collaudo_dcf`, `collaudo_xpwe`. Quello dell'interfaccia: `npm run collaudo` in
`crates/opencomputo-gui/ui-tests`.
