# ADR 0015 — Stampa del computo e dell'elenco prezzi nel modello tradizionale

* Stato: accettato
* Data: 2026-10-21

## Contesto

Il titolare ha chiesto di poter stampare i report anche con un'impaginazione
graficamente uguale a quella del programma commerciale più diffuso, a cui sono abituati
committenti e uffici tecnici. Ha scelto il computo e l'elenco prezzi e ha fornito come
riferimento tre stampe PDF reali.

Le stampe sono servite solo a misurare l'impaginazione e non sono nel repository. Delle
stampe si riproducono la struttura e l'aspetto, che sono le convenzioni dei computi
italiani. Non si riproducono nomi, marchi, crediti né il codice del programma: il
frontespizio non ha righe di copyright e il produttore del PDF è OpenComputo.

Le misure sono state prese con `pdfplumber` sulle stampe di riferimento (A4, coordinate
in punti) e poi confrontate con le stampe prodotte:
* tabella in Times 7,94 pt con interlinea di 3,4 mm (9,64 pt);
* riquadro e intestazioni in verde (RGB 0; 0,502; 0), con un filetto spesso (1,13 pt) e
  uno sottile esterno;
* colonne a x = 17,57 | 64,05 | 287,68 | 322,26 | 356,84 | 395,38 | 433,93 | 476,44 |
  518,96 | 577,34 pt;
* frontespizio in Helvetica con la fascia gialla, l'ente in blu e il titolo in bordeaux.

## Decisioni

1. **Un modello di stampa in più, solo PDF** (`opencomputo_io::tradizionale`), accanto
   al report ordinario, che resta il predefinito e l'unico per XLSX e CSV. Chiedere il
   modello tradizionale con XLSX o CSV, o con il computo con le analisi, è un errore
   della richiesta.
2. **Il computo** ha:
   * l'intestazione delle colonne: «Num.Ord. TARIFFA», «DESIGNAZIONE DEI LAVORI»,
     «D I M E N S I O N I» (par.ug., lung., larg., H/peso), «Quantità», «I M P O R T I»
     (unitario, TOTALE);
   * le righe «R I P O R T O» e «A R I P O R T A R E» su ogni pagina;
   * per ogni voce: numero, tariffa, descrizione giustificata e righi di misura. Le
     deduzioni sono in rosso, con «Sommano positivi/negativi». La voce chiude con
     «SOMMANO» e i filetti neri delle quantità.
   * i fattori con 2, 2, 3 e 3 decimali e l'apice «´» per le migliaia;
   * le espressioni dei fattori nel testo del rigo, es. «(par.ug.=4*15*2)», e la
     formula del rigo come «(quantità=…)»;
   * per super categoria, se le voci ne hanno, un gruppo con il titolo sottolineato.
     Ogni gruppo comincia su una pagina nuova e chiude con «Parziale … euro»;
   * in fondo «T O T A L E euro», luogo e data, «Il Tecnico» con il nome del
     progettista;
   * le pagine chiuse completate con le righe di tratteggio;
   * le descrizioni oltre i 300 caratteri ridotte ai primi 180 e agli ultimi 120,
     separati da « ... », come nelle stampe di riferimento. Le descrizioni complete si
     possono chiedere con un'opzione, e restano comunque nell'elenco prezzi.
3. **Il computo metrico senza prezzi** (`senza_prezzi`) ha la stessa tabella senza
   unitario, importi, parziali e totale; il titolo è «COMPUTO METRICO».
4. **L'elenco prezzi** ha:
   * le colonne «Num.Ord. TARIFFA», «D E S C R I Z I O N E D E L L' A R T I C O L O»,
     «unità di misura», «P R E Z Z O UNITARIO»;
   * le voci numerate «Nr. n» per capitolo, con la descrizione completa giustificata;
   * il prezzo in lettere in grassetto, es. «euro (centodiciassette/53)», con l'unità di
     misura e il prezzo in cifre.

   Il prezzo in lettere (`formato::in_lettere`) segue l'uso degli assegni: parole unite,
   elisione (ventuno, centottanta), «tré» finale nei composti, decimali in cifre dopo la
   barra.
5. **Valori dal core** (ADR 0007):
   * quantità, importi e totali da `Progetto::calcola_computo`;
   * righi, progressivi e somme di positivi e deduzioni dalla nuova
     `misura::dettaglio_quantita`, che è anche la base di `quantita_totale`, con gli
     stessi arrotondamenti;
   * i parziali dei gruppi dai totali per super categoria del core.

   L'unica somma della stampa è il progressivo degli importi per i riporti di pagina.
   È una somma esatta, senza arrotondamenti, e alla fine deve coincidere con il totale
   del core, altrimenti la stampa non viene prodotta. La quantità riportata quando una
   voce passa alla pagina successiva è il progressivo del core all'ultimo rigo stampato.
6. **Dati di stampa non nel progetto**: le righe dell'ente (frontespizio) e dello studio
   tecnico (testata di ogni pagina e riquadro in basso del frontespizio) sono opzioni
   della richiesta. Così lo schema del file non cambia (resta la versione 5).
   * L'interfaccia ricorda le due righe tra una sessione e l'altra.
   * Senza ente il riquadro mostra il luogo dei lavori.
   * Oggetto, committente, luogo, data e progettista vengono dai dati generali.
7. **Interfacce**:
   * nella richiesta, `tradizionale: {ente, studio, frontespizio, senza_prezzi,
     descrizioni_ridotte}` sui report `computo` ed `elenco_prezzi`;
   * dalla CLI, `--tradizionale` con `--ente`, `--studio`, `--senza-prezzi`,
     `--senza-frontespizio` e `--descrizioni-complete`;
   * nell'interfaccia, una casella «Modello tradizionale (PDF)» nella pagina Report.

   L'anteprima mostra gli stessi dati nel documento ordinario.
8. **Font standard PDF** (Times-Roman, Times-Bold, Helvetica, Helvetica-Bold, codifica
   WinAnsi), non incorporati come nel report ordinario. Aggiunte le metriche Times alle
   tabelle delle larghezze. La giustificazione usa la spaziatura delle parole del PDF
   (`Tw`).

## Conseguenze e limiti

* Verificati con le misure delle stampe di riferimento:
  * posizioni dei testi dell'intestazione;
  * prima riga della tabella;
  * righe di riporto;
  * filetti delle colonne;
  * elementi del frontespizio.

  Gli scarti verticali sono sotto 0,1 pt; quelli orizzontali dei testi centrati sotto
  0,5 pt, perché le stampe di riferimento usano Times New Roman incorporato, con
  larghezze leggermente diverse dal Times standard.
* Il programma di riferimento stampa sulla seconda riga di ogni voce la data del
  prezzo; OpenComputo non ha una data per voce e la omette.
* Non gestiti:
  * il logo dell'ente e dello studio;
  * le «Parziale» intermedie dei righi;
  * i rimandi «Vedi voce n°» (aggiunti con l'ADR 0019: il rigo riporta «Vedi voce n. 12
    [m² 125,40]» prima della descrizione);
  * le stampe tradizionali di libretto, registro, SAL e analisi.
* Le stampe di riferimento non vanno nel repository: contengono dati reali.
