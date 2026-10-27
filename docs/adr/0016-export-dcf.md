# ADR 0016 — Export sperimentale dell'elenco prezzi in DCF

* Stato: accettato, sperimentale. **PriMus non apre i file prodotti** (prova del
  titolare del 2026-10-21, vedi in fondo).
* Data: 2026-10-21
* Modifica: [ADR 0006](0006-formati-primus.md), che limitava il DCF alla lettura

## Contesto

Il titolare ha chiesto l'esportazione in DCF per l'interoperabilità. Prima di
cominciare gli sono stati esposti due limiti, e ha scelto di procedere con l'elenco
prezzi in DCF e con un file XPWE di prova, da verificare entrambi in PriMus.

1. **Computo e contabilità.** Nei documenti DCF salvati da PriMus questi dati stanno
   in flussi protetti. Nel documento di contabilità analizzato sono protetti tutti i
   flussi della cartella `Documento`, elenco prezzi compreso. Un DCF che PriMus apra come documento di computo
   richiederebbe di riprodurre quella protezione, cioè di aggirarla: è escluso
   dall'ADR 0006.
2. **Elenco prezzi.** I prezzari distribuiti in DCF sono in chiaro. Si può scrivere un
   file con la stessa struttura, ma OpenComputo non può eseguire PriMus per provarlo.

Per scambiare il computo con PriMus resta l'export XPWE, il formato di interscambio che
PriMus importa.

## Analisi del contenitore (2026-10-21)

Fonti: i due prezzari e il documento di contabilità già usati per l'ADR 0006. I file
non sono nel repository. Del documento di contabilità si sono letti solo il contenitore
e i flussi in chiaro.

* **Indice.** È un elenco piatto di voci da 512 byte, sette per settore, in una catena
  di settori. Ogni voce ha:
  * il settore con l'elenco dei settori del flusso;
  * la lunghezza;
  * tre date FILETIME;
  * gli attributi (0x10 per le cartelle);
  * la voce genitrice (0xFFFFFFFF per la radice);
  * il nome.

  Nei prezzari la cartella `Documento` contiene nove flussi; `Identity`, `Versione` e
  `VersioneFix` sono nella radice.
* **Somme di controllo.** Ogni settore ha nella testata lo XOR delle parole a 32 bit
  dei suoi 4064 byte di dati. Ogni pezzo compresso ha il CRC-16/ARC dei byte
  decompressi e la posizione della sua fine. La fine è assoluta nei contenitori 2,6
  (i prezzari) e contata dopo i primi 0x80 byte nei contenitori 8,0 (il documento).
* **Mappa dei settori liberi.** Nel settore 1 un bit per settore (1 = libero). Nei
  settori 2 e 3 un bit per byte della mappa. Il totale dei settori, escluso lo 0, è un
  multiplo di 8.
* **Campi variabili.** Gli altri campi delle testate cambiano da file a file e
  contengono anche indirizzi di memoria del programma che ha scritto il file. Nel
  documento salvato da PriMus sono a zero nei settori di dati: non sono controlli.

Verifica del modello, con il test ignorato `tests/file_reali.rs`:
* `struttura::verifica` controlla tutti i settori, la mappa e i CRC di tutti i pezzi;
  i tre file la superano senza segnalazioni;
* riscritti con lo scrittore di OpenComputo, i tre file si rileggono con gli stessi
  nomi, cartelle e contenuti.

## Decisioni

1. **Scrittura del contenitore** (`opencomputo_dcf_parser::scrittura`) nella forma dei
   prezzari:
   * contenitore 2,6;
   * indice dal settore 4, poi ogni flusso con il suo elenco di settori;
   * tutti i controlli noti calcolati;
   * i campi senza significato noto a zero.

   L'uscita è deterministica: le date sono quella del progetto o, se manca, quella
   indicata.
2. **Contenuto: un prezzario in chiaro** (`opencomputo_dcf_parser::esporta`):
   * `CollectionEP`: le voci con i 59 attributi nell'ordine dei prezzari;
   * `DatiGenerali`: dati generali, capitoli e categorie su tre livelli, spese generali
     e utile delle analisi, decimali;
   * `InfoDoc`: tipo di documento e formato 5.01;
   * le raccolte vuote `CollectionGP`, `CollectionOD`, `CollectionVC` e `InfoMRR`;
   * `Versione` 5.01 e `VersioneFix` 1.07.

   Livelli dei capitoli:
   * i capitoli importati da un DCF riprendono livello e identificativo originali;
   * gli altri prendono il livello dalla loro profondità;
   * oltre il terzo livello si scrivono come terzo, con una segnalazione.
3. **Fedeltà sulle voci importate da DCF.** Gli attributi originali conservati nella
   provenienza (articolo, prezzi alternativi, `Fgs`, percentuali…) sono riscritti, e
   così l'`IdEP` originale. Il lettore ora ricorda quando la descrizione breve viene da
   `DesRid`.
4. **Cosa non si scrive:**
   * nessuna dichiarazione di copyright di terzi: `CopyRight=""`, `SrcNm="OpenComputo"`;
   * nessun flusso `Identity`: nei file di PriMus è un codice di autenticazione che
     OpenComputo non può né deve produrre;
   * niente stili di stampa e opzioni dell'interfaccia (contenuti propri del
     programma);
   * niente analisi, computo e contabilità.
5. **Interfacce:**
   * il caso d'uso `esporta_dcf`;
   * la richiesta `esporta_dcf` del servizio;
   * `opencomputo esporta progetto.ocp elenco.dcf`: il formato segue l'estensione;
   * nell'interfaccia, il pulsante «PriMus DCF (sperimentale)…» nel report dell'elenco
     prezzi.

   Il rapporto di export segnala sempre che il file è sperimentale.

## Conseguenze e limiti

* **Verifiche fatte:**
  * i file prodotti superano gli stessi controlli dei file di PriMus;
  * si rileggono con il lettore DCF;
  * esportando i due prezzari reali importati, tutti gli attributi delle 31.755 voci
    coincidono con l'originale (test ignorato, file non nel repository).
* **Non verificato: l'apertura in PriMus.** Le ragioni possibili di rifiuto sono:
  * l'assenza di `Identity`;
  * l'attributo `CopyRight` vuoto;
  * le sezioni di `DatiGenerali` omesse.

  OpenComputo non aggiungerà marchi, codici di autenticazione o protezioni di terzi per
  farlo accettare. Se PriMus rifiuta il file per uno di questi motivi, l'export DCF
  resta utile solo per scambiare elenchi tra programmi che leggono il formato in
  chiaro, e per PriMus resta XPWE.
* I bit di `Fgs` restano un'ipotesi: la natura «sicurezza» usa lo stesso bit di XPWE.
* Gli a capo nelle descrizioni sono scritti come nei file di PriMus (CR LF nel valore
  dell'attributo). Il lettore li normalizza in spazi, come prevede XML.
* Un flusso può occupare al massimo 1016 settori (circa 4 MB compressi): bastano per
  i prezzari regionali completi.
* **Licenze.** Esportare i propri elenchi è interoperabilità di formato. Riconvertire in
  DCF prezzari di terzi può essere vietato dalle loro condizioni d'uso (ADR 0006): la
  responsabilità resta di chi esporta. Questo testo non costituisce consulenza legale.

## Esito della prova in PriMus (2026-10-21)

Il titolare ha provato in PriMus il file di prova e tre file di diagnosi:
* il prezzario «Articoli» originale con il contenitore riscritto da OpenComputo e tutti
  i flussi identici, compreso `Identity`;
* lo stesso senza `Identity`;
* il file di prova con le raccolte mancanti aggiunte vuote.

PriMus non ha aperto nessuno dei quattro. Il primo esclude che la causa sia solo il
contenuto: PriMus rifiuta il file anche con i flussi originali. Le ragioni possibili
sono due:
* nel contenitore c'è un controllo non riprodotto;
* il codice di autenticazione copre anche i byte del contenitore.

Il titolare ha chiesto se si potesse riprodurre la firma di PriMus. La risposta è no:
generarla per file non prodotti da ACCA vorrebbe dire falsificare un'autenticazione,
cosa esclusa come la decifratura dei flussi protetti.

L'export resta, segnalato come non apribile in PriMus, per scambiare elenchi con
programmi che leggono il DCF in chiaro, compreso OpenComputo. Per PriMus si usa l'export
XPWE.
