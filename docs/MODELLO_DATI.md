# Modello dati (schema versione 7)

## Mappa delle entità

```
Progetto
├── MetadatiProgetto            titolo, committente, impresa, CIG, CUP, RUP, DL…
├── DatiContratto?              repertorio, date, importo, ParametriContabili (con IVA e Anticipazione?)
├── RegoleArrotondamento        decimali e modalità di arrotondamento
├── elenco_prezzi: Prezzario
│   ├── CapitoloPrezzario[]     albero dei codici (padre opzionale)
│   ├── VocePrezzario[]         codice, descrizioni, U.M., prezzo, natura, incidenze, analisi, NuovoPrezzo?
│   ├── RisorsaElementare[]     manodopera, materiali, noli, trasporti
│   ├── AnalisiPrezzo[]         righe (risorsa, voce o sotto-analisi, libera; quantità ed espressione),
│   │                           VariabileAnalisi[], spese generali, utile, quota oneri sicurezza
│   └── ParametriAnalisi?       spese generali e utile predefiniti (15 %, 10 %)
├── Categorie                   super categorie / categorie / sotto categorie (elenchi piatti)
├── StrutturaWbs                albero di nodi
├── Computo[]
│   └── VoceComputo[]           PrezzoApplicato + RigoMisura[] (anche «vedi voce») + categorie + nodo WBS
├── Contabilita
│   ├── LibrettoMisure
│   │   └── RegistrazioneLibretto[]   numero, data, PrezzoApplicato, RigoMisura[], voce di computo collegata
│   ├── StatoAvanzamentoLavori[]      numero, data, stato, registrazioni, riepilogo e certificato congelati,
│   │                                 penali applicate, opzioni di recupero dell'anticipazione
│   ├── Penale[]                      numero, data, motivo, importo (giorni e per mille facoltativi)
│   ├── VerbaleNuoviPrezzi[]          numero, data, firma e riserve dell'esecutore, approvazione e atto
│   └── ContoFinale?                  ultimazione, data, stato, registrazioni dello stato finale,
│                                     riepilogo e penali congelati, firma e riserve, Collaudo?
└── Provenienza?                formato/file di origine, anomalie, dati non mappati
```

Ogni entità ha un identificativo stabile e tipizzato (`VoceComputoId`, `SalId`…).
I collegamenti tra documenti usano solo identificativi:

* `PrezzoApplicato.origine` → (`PrezzarioId`, `VocePrezzarioId`), solo per tracciabilità;
* `VoceComputo.categorie` / `.wbs` → `ClasseId` / `WbsNodoId`;
* `RegistrazioneLibretto.voce_computo` → `VoceComputoId`;
* `RigoMisura.vedi_voce` → `VoceComputoId` della stessa voce di computo (collegato),
  oppure quantità e testo copiati (statico, l'unico ammesso nel libretto);
* `StatoAvanzamentoLavori.registrazioni` → `RegistrazioneId[]`;
* `StatoAvanzamentoLavori.penali` → `PenaleId[]` (solo nei SAL emessi);
* `RigaAnalisi.origine` → `RisorsaId` o `VocePrezzarioId`; `VocePrezzario.analisi` →
  `AnalisiId`; `NuovoPrezzo.verbale` → numero del `VerbaleNuoviPrezzi`;
* `ContoFinale.registrazioni` / `.penali` → `RegistrazioneId[]` / `PenaleId[]` (fissate
  all'emissione).

## Grandezze numeriche

| Tipo | Significato | Esempio |
|------|-------------|---------|
| `Quantita` | quantità con segno | `126.00` m³ |
| `Prezzo` | prezzo unitario in € | `145.30` €/m³ |
| `Importo` | importo in € | `7517.82` € |
| `Percentuale` | percentuale in centesimi | `12.5` = 12,5 % |

Tutte usano `rust_decimal::Decimal` (96 bit, 28 cifre significative). Le operazioni
sono controllate e restituiscono `Overflow` invece di andare in panico. In JSON i
valori sono stringhe (`"145.30"`).

## Regole di calcolo

### Rigo di misura

1. Rigo di commento → nessuna quantità.
2. Ciascun fattore (parti uguali, lunghezza, larghezza, altezza/peso) è un valore o
   un'espressione (`"7,50+0,30"`).
3. Con una formula, il risultato è la formula, che può usare `par`, `lun`, `lar`, `alt`;
   una variabile senza valore è un errore.
4. Senza formula, il risultato è il prodotto dei soli fattori valorizzati; nessun fattore → 0
   (la validazione lo segnala).
5. Deduzione → segno negativo.
6. Se `arrotonda_quantita_parziali` (default), il rigo è arrotondato a `decimali_quantita`.
7. «Vedi voce»: la quantità citata (della voce collegata, arrotondata, o quella copiata)
   è un fattore in più; con una formula entra solo come variabile `vv`. Nel computo le
   voci si calcolano nell'ordine dei collegamenti; i riferimenti circolari sono un
   errore sul rigo che cita ([ADR 0019](adr/0019-vedi-voce.md)).

### Voce (computo o registrazione)

```
quantità = arrotonda_q( Σ righi )
importo  = arrotonda_€( quantità × prezzo )
manodopera = arrotonda_€( importo × incidenza manodopera / 100 )   se l'incidenza è nota
```

### Totali

Somme esatte degli importi già arrotondati: il totale coincide sempre con la somma
delle righe stampate. Totali separati per natura (lavori / oneri della sicurezza),
per super categoria, categoria, sotto categoria, nodo WBS (diretti e cumulati sugli antenati).

### Analisi prezzi

Nel prezzario (`Prezzario::calcola_analisi`) il prezzo di una riga è quello corrente della
risorsa o della voce collegata; per una voce analizzata è il prezzo unitario della sua
analisi (sotto-analisi, calcolata ricorsivamente; i cicli sono un errore). Vedi
[ADR 0013](adr/0013-analisi-e-nuovi-prezzi.md).

```
riga        = arrotonda_€( quantità × prezzo corrente )
costo       = Σ righe                         (anche suddiviso per tipo di risorsa)
spese gen.  = arrotonda_€( costo × SG% )
utile       = arrotonda_€( (costo + spese gen.) × U% )     oppure sul solo costo
totale      = costo + spese gen. + utile
prezzo      = arrotonda_prezzo( totale / quantità di riferimento )
incidenza    = arrotonda_%( Σ quote del tipo / totale × 100 )   per manodopera, materiali, noli…
               (il costo di una sotto-analisi è ripartito con le sue frazioni esatte)
```

Le percentuali sono parametri dell'analisi: il core non impone valori normativi. Il
prezzo della voce di elenco si aggiorna solo con un'operazione esplicita.

Quantità con espressione: valutata con le variabili dell'analisi (nel loro ordine) e con
`q1`, `q2`… (quantità delle righe precedenti). Oneri aziendali della sicurezza =
arrotonda_€( spese generali × quota ), solo evidenziati
([ADR 0014](adr/0014-variabili-e-aggiornamento-prezzi.md)).

### Fabbisogno di cantiere

```
quantità della risorsa = Σ quantità della voce di computo × quantità nella riga
                         / quantità di riferimento     (ricorsivo nelle sotto-analisi)
importo                = quantità arrotondata × prezzo corrente   (senza SG né utile)
```

### Nuovi prezzi

`VocePrezzario.nuovo_prezzo` (numero NP.n, data, motivo, criterio, `soggetto_a_ribasso`,
verbale). Un nuovo prezzo non soggetto a ribasso è copiato nel `PrezzoApplicato` come
`escluso_dal_ribasso`; nei SAL e nel conto finale:

```
soggetto a ribasso = lavori lordi − manodopera esclusa − lavori esclusi dal ribasso
```

### SAL

Calcolo sui valori progressivi (vedi documentazione del modulo `contabilita::sal`):

```
soggetto a ribasso = lavori lordi − manodopera esclusa (se l'opzione è attiva)
ribasso            = arrotonda_€( soggetto × ribasso% )
netto              = lavori lordi + oneri sicurezza − ribasso
ritenuta           = arrotonda_€( netto × ritenuta% )        default 0,50 %, da verificare sul contratto
credito            = netto − ritenuta
importo SAL        = credito − credito del SAL precedente (valore congelato)
```

### Certificato di pagamento

Vedi documentazione del modulo `contabilita::certificato` e
[ADR 0011](adr/0011-contabilita-completa.md):

```
recupero    = min(anticipazione, % recupero × netto progressivo) − già recuperato
              (0 se sospeso; tutto il residuo se richiesto; sempre tra 0 e il residuo)
penali      = Σ penali con data ≤ «lavori a tutto il» non applicate in SAL precedenti
imponibile  = importo SAL − recupero − penali
IVA         = arrotonda_€( imponibile × aliquota )           se l'aliquota è indicata
totale      = imponibile + IVA
```

### Conto finale e rata di saldo

Vedi documentazione del modulo `contabilita::conto_finale` e
[ADR 0012](adr/0012-conto-finale.md):

```
stato finale     = registrazioni dei SAL emessi + registrazioni non comprese nei SAL
netto finale     = lavori + sicurezza − ribasso (come nei SAL, sui totali)
ritenute         = arrotonda_€( netto finale × ritenuta% )   (da svincolare)
credito residuo  = netto finale − Σ importi dei SAL emessi
rata di saldo    = credito residuo − anticipazione non recuperata − penali non applicate
IVA, totale      come nei certificati
```

## Arrotondamento

`RegoleArrotondamento` (per progetto):

| Campo | Default |
|-------|---------|
| `decimali_quantita` | 2 |
| `decimali_prezzo` | 2 (solo prezzi *calcolati*, es. da analisi) |
| `decimali_importo` | 2 |
| `decimali_percentuale` | 2 |
| `modalita` | `commerciale` (metà lontano da zero); alternative `meta_al_pari`, `troncamento` |
| `arrotonda_quantita_parziali` | `true` |

I dati d'ingresso (dimensioni, prezzi di listino) non vengono mai arrotondati.

## Fotografia dei prezzi

La voce di computo e la registrazione contabile contengono un `PrezzoApplicato`,
copia dei dati della voce di prezzario al momento dell'inserimento. Modificare il
prezzario non modifica i documenti; la validazione segnala con gravità
*informazione* le voci non più allineate (`prezzo.non_allineato`) e
`PrezzoApplicato::riallinea` aggiorna esplicitamente la copia restituendo le differenze.

## Cronoprogramma

Campo `cronoprogramma` del progetto (schema 7, omesso se vuoto; ADR 0024):

* `inizio_lavori` (se manca: `contratto.data_consegna_lavori`), `livello_dettaglio`
  (assente: tutti i livelli), `computo` degli importi (assente: il primo);
* `calendario`: `giorni_lavorativi` (7 valori da lunedì), `festivita_nazionali`,
  `chiusure` (`dal`, `al`, `descrizione`);
* `attivita_wbs`: dati delle attività dei nodi WBS (`nodo`, `durata` in giorni
  lavorativi, `inizio_non_prima`, `avanzamento` 0–100, `note`);
* `attivita`: attività aggiunte (`id`, `nome`, `padre` nodo WBS o radice,
  `pietra_miliare`, stessi dati);
* `legami`: `da`, `a` (`{"tipo": "wbs"|"attivita", "id": …}`), `tipo` (`fine_inizio`,
  `inizio_inizio`, `fine_fine`, `inizio_fine`), `ritardo` in giorni lavorativi (anche
  negativo);
* `linea_base`: `salvata_il` e le date di inizio e fine di ogni attività.

Regole del calcolo (`cronoprogramma::calcola`):
* indici di giorno lavorativo, intervalli semiaperti [inizio, fine);
* FI: inizio ≥ fine del predecessore + ritardo; II: inizio ≥ inizio + ritardo;
  FF: fine ≥ fine + ritardo; IF: fine ≥ inizio + ritardo;
* durata mancante: 1 giorno, «da stimare»; pietra miliare: durata 0, alla fine del giorno
  lavorativo precedente;
* margine totale = inizio al più tardi − inizio al più presto; critica se ≤ 0;
* riepiloghi: estremi dei figli, avanzamento pesato sulle durate;
* importi: totali WBS cumulati del computo, distribuiti per giorno lavorativo e
  arrotondati per mese con il resto nell'ultimo mese.

## Contabilità: invarianti

* registrazioni con id e numero univoci (numero ≥ 1);
* ogni registrazione in al più un SAL; SAL numerati 1…n con date crescenti;
* un solo SAL in bozza, l'ultimo; emissione solo dopo i precedenti;
* le registrazioni di un SAL emesso non sono modificabili né rimovibili tramite il modello;
* `verifica_integrita` ricalcola i SAL emessi e segnala `contabilita.sal_alterato` se il
  risultato non coincide con il riepilogo congelato (dati o parametri alterati), e
  `contabilita.certificato_alterato` per il certificato congelato;
* penali con id e numero univoci, ciascuna applicata in al più un SAL emesso; le penali
  applicate non sono rimovibili;
* il conto finale si redige senza SAL in bozza e con l'ultimazione non anteriore
  all'ultimo SAL; con il conto in bozza non si redigono SAL; dopo l'emissione la
  contabilità è chiusa e `contabilita.conto_finale_alterato` segnala le differenze dal
  ricalcolo.

## Provenienza

`Provenienza` registra formato e file di origine, impronta SHA-256, identificativo
originale, anomalie (`Anomalia` con gravità e codice) e i dati originali non mappati,
per poter ricostruire il documento di origine quando possibile.

## Persistenza

```json
{ "formato": "opencomputo.progetto", "versione_schema": 7, "progetto": { … } }
```

Gli importi sono scritti con almeno 2 decimali (`"0.00"`); le altre grandezze
conservano la scala con cui sono state inserite. In lettura qualsiasi scala è accettata.

Versioni precedenti sono migrate con funzioni esplicite (`schema::MIGRAZIONI`),
versioni successive sono rifiutate. La fixture
`crates/opencomputo-core/tests/fixtures/progetto_esempio_v1.json` garantisce che i
documenti della versione 1 restino leggibili: se lo schema cambia si scrive una
migrazione, non si rigenera la fixture.
