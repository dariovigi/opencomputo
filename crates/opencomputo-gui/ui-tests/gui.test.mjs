// Test end-to-end dell'interfaccia: Chromium + motore reale (`opencomputo servizio`).
// Eseguire dopo `cargo build -p opencomputo-agent-api`: `npm test` in questa cartella.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { copyFile, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { apriBanco, FIXTURE, XPWE } from './banco.mjs';

const SCHERMATE = process.env.OPENCOMPUTO_SCHERMATE || null;
let b;

before(async () => { b = await apriBanco(); });
after(async () => { await b?.chiudi(); });

const p = () => b.pagina;
const mod = process.platform === 'darwin' ? 'Meta' : 'Control';

/** Attende che richieste e disegni siano terminati. */
async function attendi(pagina = p()) {
  await pagina.waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
  await pagina.waitForTimeout(30);
  await pagina.waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
}

async function schermata(nome, pagina = p()) {
  if (SCHERMATE) await pagina.screenshot({ path: path.join(SCHERMATE, `${nome}.png`) });
}

const testo = async (sel, pagina = p()) => (await pagina.locator(sel).first().innerText()).trim();
const totale = () => testo('.totali .complessivo strong');

/** Scrive in una cella attiva del foglio e conferma con il tasto indicato. */
async function scrivi(valore, tasto = 'Tab') {
  await p().keyboard.type(valore);
  await p().keyboard.press(tasto);
  await attendi();
}

/** Sceglie una voce nel selettore aperto: cerca, attende il risultato unico, conferma. */
async function scegliNelSelettore(cerca, codice) {
  await p().fill('.modale input[type=search]', cerca);
  await p().waitForFunction((c) => {
    const righe = [...document.querySelectorAll('.modale .griglia-visibili .griglia-riga')];
    return righe.length > 0 && righe[0].textContent.includes(c) && righe.every((r) => r.textContent.includes(c));
  }, codice);
  await p().keyboard.press('Enter');
  await p().waitForSelector('.modale', { state: 'detached' });
  await attendi();
}

async function vai(voce) {
  await p().locator('nav button', { hasText: voce }).first().click();
  await attendi();
}

/** Chiude il progetto (scartando le modifiche) per ripartire dalla schermata iniziale. */
async function ricomincia() {
  await b.motore({ richiesta: 'chiudi', forza: true });
  await p().goto(b.indirizzo);
  await attendi();
  await p().waitForSelector('.benvenuto');
}

async function apriEsempio() {
  await ricomincia();
  const f = path.join(b.cartella, `esempio-${Date.now()}.ocp`);
  await copyFile(FIXTURE, f);
  await b.dialogo(f);
  await p().click('text=Apri progetto');
  await p().waitForSelector('table.misure');
  await attendi();
  return f;
}

test('computo nuovo: elenco prezzi, voce, righi da tastiera, annulla e salva', async () => {
  await p().waitForSelector('.benvenuto');
  await schermata('01-benvenuto');
  await p().click('text=Nuovo progetto');
  await p().fill('.modale input[name=titolo]', 'Scuola di prova');
  await p().keyboard.press('Enter');
  await attendi();
  assert.equal(await testo('.titolo-progetto strong'), 'Scuola di prova');

  // Voce di elenco prezzi dal modulo.
  await vai('Elenco prezzi');
  await p().click('text=+ Nuova voce');
  await p().fill('.modale input[name=codice]', 'A.01');
  await p().fill('.modale input[name=descrizione_breve]', 'Scavo di sbancamento');
  await p().fill('.modale input[name=unita_misura]', 'm³');
  await p().fill('.modale input[name=prezzo]', '8,50');
  await p().click('.modale button:has-text("Aggiungi")');
  await attendi();
  await p().waitForSelector('.griglia-riga[data-indice="0"]:has-text("A.01")');

  // Voce di computo dal selettore, righi scritti da tastiera.
  await vai('Computo metrico estimativo');
  await p().click('text=+ Voce');
  await scegliNelSelettore('scavo', 'A.01');
  await p().waitForSelector('table.misure tr.testata:has-text("A.01")');
  await scrivi('Platea');       // descrizione → parti uguali
  await p().keyboard.press('Tab'); // parti uguali vuoto → lunghezza
  await scrivi('12');
  await scrivi('8');
  await scrivi('1,20', 'Enter');
  assert.equal(await testo('tr.testata[data-blocco="1"] td.c-q'), '115,20 m³');
  assert.equal(await totale(), '979,20 €');
  await schermata('02-computo-nuovo');

  // Annulla l'ultima modifica (l'altezza) e ripristina.
  await p().locator('table.misure').click({ position: { x: 5, y: 5 } }).catch(() => {});
  await p().keyboard.press(`${mod}+z`);
  await attendi();
  assert.equal(await totale(), '816,00 €');
  await p().keyboard.press(`${mod}+y`);
  await attendi();
  assert.equal(await totale(), '979,20 €');

  // Salva con nome (nessun file ancora).
  const f = path.join(b.cartella, 'scuola.ocp');
  await b.dialogo(f);
  await p().keyboard.press(`${mod}+s`);
  await attendi();
  assert.ok((await stat(f)).size > 0);
  assert.equal(await p().locator('.titolo-progetto strong.modificato').count(), 0);
  const info = await b.motore({ richiesta: 'info' });
  assert.equal(info.risultato.file, f);
  assert.equal(info.risultato.computi[0].totale, '979.20');
});

test('modifica di un rigo, espressioni, rifiuto dei nuovi errori', async () => {
  await apriEsempio();
  assert.equal(await totale(), '14.457,12 €');
  const cella = 'tr.rigo[data-blocco="2"][data-rigo="2"] td[data-campo="lunghezza"]';
  assert.equal(await testo(cella), '7,50+0,30');
  await p().click(cella);
  await p().keyboard.press('Enter');
  await p().fill('.editor-cella', '7,50+0,50');
  await p().keyboard.press('Enter');
  await attendi();
  // Cordoli: 4 × 8,00 × 0,30 × 0,40 = 3,84; calcestruzzo 51,84 × 145,30 = 7.532,35.
  assert.equal(await testo('tr.rigo[data-blocco="2"][data-rigo="2"] td.c-q'), '3,84');
  assert.equal(await totale(), '14.471,65 €');

  // Espressione non valida: la modifica è rifiutata e il valore resta.
  await p().click(cella);
  await p().keyboard.press('Enter');
  await p().fill('.editor-cella', '2*(');
  await p().keyboard.press('Enter');
  await p().waitForSelector('.modale:has-text("Modifica non applicata")');
  await schermata('03-modifica-rifiutata');
  await p().keyboard.press('Escape');
  await attendi();
  assert.equal(await testo(cella), '7,50+0,50');
  assert.equal(await totale(), '14.471,65 €');

  // Deduzione dal pulsante e annulla dal pulsante.
  await p().click('tr.rigo[data-blocco="1"][data-rigo="2"] td[data-campo="descrizione"]');
  await p().click('.vista-testata button:has-text("Deduzione")');
  await attendi();
  assert.equal(await testo('tr.rigo[data-blocco="1"][data-rigo="2"] td.c-q'), '-10,80');
  await p().click('button[aria-label=Annulla]');
  await attendi();
  await p().click('button[aria-label=Annulla]');
  await attendi();
  assert.equal(await totale(), '14.457,12 €');
});

test('voci: inserimento, spostamento, eliminazione con conferma', async () => {
  await apriEsempio();
  await p().click('tr.testata[data-blocco="4"] td.c-descr');
  await p().click('.vista-testata button[title="Sposta la voce in alto"]');
  await attendi();
  assert.match(await testo('tr.testata[data-blocco="3"]'), /S\.01\.001/);
  await p().click('tr.testata[data-blocco="3"] td.c-descr');
  await p().click('text=Elimina voce');
  await p().click('.modale button:has-text("Elimina")');
  await attendi();
  assert.equal(await p().locator('tr.testata').count(), 3);
  assert.equal(await totale(), '13.257,12 €');
  // Nuovo rigo con Ins sotto il rigo attivo.
  await p().click('tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="descrizione"]');
  await p().keyboard.press('Insert');
  await attendi();
  assert.equal(await p().locator('tr.rigo[data-blocco="1"]:not(.nuovo)').count(), 3);
  await scrivi('Rampa bis', 'Enter');
  assert.equal(await testo('tr.rigo[data-blocco="1"][data-rigo="2"] td[data-campo="descrizione"]'), 'Rampa bis');
});

test('dettaglio della voce: categorie, note, prezzo non allineato e riallineamento', async () => {
  await apriEsempio();
  await p().click('tr.testata[data-blocco="1"] td.c-descr');
  await attendi();
  await p().selectOption('.dettaglio select[name="cat-categoria"]', 'MU');
  await attendi();
  let c = await b.motore({ richiesta: 'computo' });
  assert.equal(c.risultato.voci[0].categorie.categoria, 'MU');
  await p().fill('.dettaglio textarea[name=note]', 'verificare quote');
  await p().locator('.dettaglio textarea[name=note]').blur();
  await attendi();
  c = await b.motore({ richiesta: 'computo' });
  assert.equal(c.risultato.voci[0].note, 'verificare quote');

  // Prezzo modificato in elenco: il computo conserva quello fotografato.
  await vai('Elenco prezzi');
  await p().click('.griglia-riga:has-text("A.01.001")');
  await attendi();
  await p().fill('.dettaglio input[name=prezzo]', '9,00');
  await p().click('.dettaglio button:has-text("Applica modifiche")');
  await attendi();
  await vai('Computo metrico estimativo');
  assert.equal(await totale(), '14.457,12 €');
  await p().waitForSelector('tr.testata[data-blocco="1"] .etichetta:has-text("prezzo cambiato")');
  await p().click('tr.testata[data-blocco="1"] td.c-descr');
  await p().click('.dettaglio button:has-text("Riallinea")');
  await attendi();
  // 126,00 m³ × 0,50 € in più = 63,00 €.
  assert.equal(await totale(), '14.520,12 €');
  await schermata('04-dettaglio-voce');
});

test('prezzario di consultazione: carica, cerca, copia nell\'elenco e usa nel computo', async () => {
  await apriEsempio();
  await vai('Elenco prezzi');
  await p().click('text=Prezzari di consultazione');
  await b.dialogo(XPWE);
  await p().click('text=Carica prezzario…');
  await attendi();
  await p().waitForSelector('.griglia-riga[data-indice="0"]');
  const righe = await p().locator('.griglia-visibili .griglia-riga').count();
  assert.ok(righe >= 2);
  await p().click('.griglia-riga[data-indice="0"]');
  await p().click('.griglia-riga[data-indice="1"]', { modifiers: ['Shift'] });
  await p().click('text=Copia nell\'elenco');
  await attendi();
  const info = await b.motore({ richiesta: 'info' });
  assert.equal(info.risultato.elenco_prezzi.voci, 6);
  assert.equal(info.risultato.prezzari_caricati[0].nome, 'computo_sintetico');
  await schermata('05-consultazione');

  // Selettore del computo: una voce presa direttamente dal prezzario caricato.
  const terza = await p().locator('.griglia-riga[data-indice="2"] .cella').first().innerText();
  await vai('Computo metrico estimativo');
  await p().click('text=+ Voce');
  await p().selectOption('.modale select', 'computo_sintetico');
  await scegliNelSelettore(terza, terza);
  const c = await b.motore({ richiesta: 'computo' });
  assert.equal(c.risultato.voci.length, 5);
  assert.ok(c.risultato.voci.some((v) => v.codice === terza));
});

test('dati generali: campi applicati all\'uscita, titolo obbligatorio', async () => {
  await apriEsempio();
  await vai('Dati generali');
  await p().fill('input[name=committente]', 'Provincia di Esempio');
  await p().keyboard.press('Tab');
  await attendi();
  let pr = await b.motore({ richiesta: 'progetto' });
  assert.equal(pr.risultato.metadati.committente, 'Provincia di Esempio');
  // Il fuoco resta sul campo successivo dopo il ridisegno.
  assert.equal(await p().evaluate(() => document.activeElement?.name), 'impresa');
  await p().fill('input[name=titolo]', '');
  await p().keyboard.press('Tab');
  await p().waitForSelector('.notifica.errore');
  assert.equal(await p().inputValue('input[name=titolo]'), 'Nuova scuola primaria');
  await p().fill('input[name=ribasso]', '10');
  await p().keyboard.press('Tab');
  await p().waitForSelector('.modale:has-text("Modifica non applicata")');
  await p().keyboard.press('Escape');
  await attendi();
  pr = await b.motore({ richiesta: 'progetto' });
  assert.equal(pr.risultato.contratto.parametri.ribasso, '12.5', 'un SAL emesso blocca il ribasso');
  await schermata('06-dati-generali');
});

test('contabilità: registrazione da tastiera, nuovo SAL, emissione e annulla', async () => {
  await apriEsempio();
  await vai('Libretto delle misure');
  assert.equal(await p().locator('tr.testata.bloccata').count(), 4);
  assert.equal(await p().locator('tr.rigo.nuovo').count(), 0, 'registrazioni di SAL emessi in sola lettura');
  await p().click('text=+ Registrazione');
  await p().fill('.modale input[name=data]', '2026-05-20');
  await p().keyboard.press('Enter');
  await scegliNelSelettore('muratura', 'C.03.020');
  await p().waitForSelector('tr.testata[data-blocco="5"]');
  await scrivi('Parete nord');
  await p().keyboard.press('Tab');
  await scrivi('10');
  await p().keyboard.press('Tab');
  await scrivi('3', 'Enter');
  assert.equal(await testo('tr.testata[data-blocco="5"] td.c-q'), '30,00 m²');
  assert.equal(await testo('tr.testata[data-blocco="5"] td.c-importo'), '1.282,50');
  await schermata('07-libretto');

  await vai('Stati di avanzamento');
  await p().click('text=+ Nuovo SAL');
  await p().fill('.modale input[name=data]', '2026-05-31');
  await p().click('.modale button:has-text("Crea in bozza")');
  await attendi();
  await p().waitForSelector('.scheda h2:has-text("SAL n. 3")');
  const k = await b.motore({ richiesta: 'contabilita' });
  const atteso = k.risultato.sal[2].riepilogo.importo_sal;
  const mostrato = await testo('table.prospetto tr.totale td.num');
  assert.equal(mostrato.replace(/\./g, '').replace(',', '.').replace(' €', ''), atteso);
  assert.equal(await testo('table.prospetto tr:first-child td.num'), '1.282,50 €');
  await p().click('text=Emetti SAL…');
  await p().fill('.modale input[name=data]', '2026-06-05');
  await p().click('.modale button:has-text("Emetti")');
  await attendi();
  await p().waitForSelector('.scheda h2 .etichetta:has-text("emesso il 05/06/2026")');
  await schermata('08-sal');
  await p().keyboard.press(`${mod}+z`);
  await attendi();
  await p().waitForSelector('.scheda h2 .etichetta:has-text("bozza")');
});

test('contabilità completa: IVA, anticipazione, penali, certificato e sommario', async () => {
  await apriEsempio();
  const importoDi = (t) => t.replace(/\./g, '').replace(',', '.').replace(' €', '');
  await vai('Dati generali');
  await p().fill('input[name=importo_contrattuale]', '100000');
  await p().keyboard.press('Tab');
  await attendi();
  await p().selectOption('select[name=aliquota_iva]', '10');
  await attendi();
  await p().fill('input[name=ant_recupero]', '20');
  await p().click('button:has-text("Imposta anticipazione")');
  await attendi();
  await p().waitForSelector('.scheda h2:has-text("Anticipazione del prezzo") .etichetta:has-text("20.000,00 €")');
  let pr = await b.motore({ richiesta: 'progetto' });
  const par = pr.risultato.contratto.parametri;
  assert.equal(pr.risultato.contratto.importo_contrattuale, '100000.00');
  assert.equal(Number(par.aliquota_iva), 10);
  assert.equal(par.anticipazione.importo, '20000.00');
  assert.equal(Number(par.anticipazione.percentuale), 20);
  assert.equal(Number(par.anticipazione.percentuale_recupero), 20);

  // Certificato di un SAL emesso con una versione precedente: ricalcolato.
  await vai('Stati di avanzamento');
  await p().waitForSelector('.scheda h2:has-text("Certificato di pagamento n. 2") .etichetta:has-text("ricalcolato")');
  let k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  assert.equal(importoDi(await testo('.scheda:has(h2:has-text("Certificato di pagamento")) tr.totale td.num')), k.sal[1].certificato.totale);
  assert.equal(await p().locator('input[name=recupero_anticipazione_sospeso]').count(), 0, 'SAL emesso: nessuna opzione');

  // Penale per ritardo: 3 giorni allo 0,5 per mille di 100.000 €.
  await p().click('button:has-text("+ Penale")');
  await p().fill('.modale input[name=data]', '2026-05-15');
  await p().fill('.modale input[name=giorni]', '3');
  await p().fill('.modale input[name=per_mille]', '0,5');
  await p().click('.modale button:has-text("Registra")');
  await attendi();
  assert.equal(await testo('.scheda:has(h2:has-text("Penali")) tbody tr td.num'), '150,00 €');
  assert.match(await testo('.scheda:has(h2:has-text("Penali"))'), /10\.000,00 €/);

  // Nuovo SAL in bozza: la penale e il recupero vanno nel certificato.
  await vai('Libretto delle misure');
  await p().click('text=+ Registrazione');
  await p().fill('.modale input[name=data]', '2026-05-20');
  await p().keyboard.press('Enter');
  await scegliNelSelettore('muratura', 'C.03.020');
  await p().waitForSelector('tr.testata[data-blocco="5"]');
  await scrivi('Parete nord');
  await p().keyboard.press('Tab');
  await scrivi('10');
  await p().keyboard.press('Tab');
  await scrivi('3', 'Enter');
  await vai('Stati di avanzamento');
  await p().click('text=+ Nuovo SAL');
  await p().fill('.modale input[name=data]', '2026-05-31');
  await p().click('.modale button:has-text("Crea in bozza")');
  await attendi();
  await p().waitForSelector('.scheda h2:has-text("Certificato di pagamento n. 3") .etichetta:has-text("bozza")');
  const scheda = '.scheda:has(h2:has-text("Certificato di pagamento n. 3"))';
  k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  let c = k.sal[2].certificato;
  assert.equal(c.penali, '150.00');
  assert.ok(Number(c.recupero_anticipazione) > 0);
  assert.equal(importoDi(await testo(`${scheda} tr:has-text("penali") td.num`)), '150.00');
  assert.equal(importoDi(await testo(`${scheda} tr.totale td.num`)), c.totale);
  assert.match(await testo(`${scheda} tr:has-text("IVA")`), /IVA 10 %/);
  await p().locator('.scheda:has(h2:has-text("Penali"))').scrollIntoViewIfNeeded();
  await schermata('14-certificato');

  // Recupero sospeso, poi recupero di tutto il residuo.
  await p().check(`${scheda} input[name=recupero_anticipazione_sospeso]`);
  await attendi();
  k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  c = k.sal[2].certificato;
  assert.equal(c.recupero_anticipazione, '0.00');
  assert.equal(c.recupero_sospeso, true);
  assert.match(await testo(`${scheda} table`), /recupero dell'anticipazione \(sospeso\)/);
  await p().uncheck(`${scheda} input[name=recupero_anticipazione_sospeso]`);
  await attendi();
  await p().check(`${scheda} input[name=recupera_residuo_anticipazione]`);
  await attendi();
  k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  c = k.sal[2].certificato;
  assert.equal(c.anticipazione_residua, '0.00');
  assert.equal(c.recupero_anticipazione, sommaCentesimi('20000.00', '-' + c.anticipazione_recuperata_prima));

  // Emissione: il certificato si congela e la penale non si può più rimuovere.
  await p().click('text=Emetti SAL…');
  await p().fill('.modale input[name=data]', '2026-06-05');
  await p().click('.modale button:has-text("Emetti")');
  await attendi();
  await p().waitForSelector(`${scheda} .etichetta:has-text("congelato")`);
  assert.equal(await testo('.scheda:has(h2:has-text("Penali")) tbody tr td:nth-child(6)'), 'SAL n. 3');
  assert.equal(await p().locator('button[aria-label="Rimuovi la penale 1"]').count(), 0);

  // Anteprima del certificato e del sommario del registro.
  await p().click('button:has-text("Anteprima e stampa del certificato")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Certificato di pagamento n. 3")');
  assert.match(await testo('.foglio-stampa'), /TOTALE DEL CERTIFICATO DI PAGAMENTO/);
  assert.ok((await testo('.foglio-stampa')).includes(numeroIt(c.totale)));
  await p().click('label:has-text("Sommario del registro")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Sommario del registro di contabilità")');
  assert.match(await testo('.foglio-stampa'), /C\.03\.020/);
  await schermata('15-sommario');
});

/** Somma di importi in forma di stringa (centesimi interi). */
function sommaCentesimi(...valori) {
  const c = valori.reduce((t, v) => t + BigInt(v.replace('.', '')), 0n);
  return `${c / 100n}.${String(c % 100n).padStart(2, '0')}`;
}

/** `2190.25` → `2.190,25`. */
function numeroIt(v) {
  const [i, f] = v.split('.');
  return i.replace(/\B(?=(\d{3})+(?!\d))/g, '.') + ',' + f;
}

test('conto finale: stato finale, emissione, firma, collaudo e rata di saldo', async () => {
  await apriEsempio();
  const importoDi = (t) => t.replace(/\./g, '').replace(',', '.').replace(' €', '');
  // Una misurazione dopo l'ultimo SAL: entra nello stato finale.
  await vai('Libretto delle misure');
  await p().click('text=+ Registrazione');
  await p().fill('.modale input[name=data]', '2026-05-20');
  await p().keyboard.press('Enter');
  await scegliNelSelettore('muratura', 'C.03.020');
  await p().waitForSelector('tr.testata[data-blocco="5"]');
  await scrivi('Parete nord');
  await p().keyboard.press('Tab');
  await scrivi('10');
  await p().keyboard.press('Tab');
  await scrivi('3', 'Enter');

  await vai('Conto finale');
  await p().click('button:has-text("Redigi il conto finale")');
  await p().fill('.modale input[name=data_ultimazione]', '2026-05-25');
  await p().fill('.modale input[name=data]', '2026-06-15');
  await p().click('.modale button:has-text("Redigi in bozza")');
  await attendi();
  await p().waitForSelector('.scheda h2:has-text("Conto finale dei lavori") .etichetta:has-text("bozza")');
  let k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  let r = k.conto_finale.riepilogo;
  assert.deepEqual(k.conto_finale.registrazioni, [5]);
  assert.equal(r.importo_netto, '11974.84');
  assert.equal(r.credito_residuo, '1176.45');
  assert.equal(r.totale, '1176.45', 'senza anticipazione, penali né IVA');
  const importi = '.scheda:has(h2:has-text("Stato finale e conto"))';
  assert.equal(importoDi(await testo(`${importi} tr:has-text("Credito residuo") td.num`)), r.credito_residuo);
  assert.equal(importoDi(await testo('.scheda:has(h2:has-text("rata di saldo")) tr.totale td.num')), r.totale);
  assert.match(await testo('.scheda:has(h2:has-text("Lavori dello stato finale"))'), /C\.03\.020/);

  // Note salvate all'uscita dal campo; i SAL sono bloccati finché c'è il conto.
  await p().fill('textarea[name=note]', 'Lavori ultimati senza varianti');
  await p().keyboard.press('Tab');
  await attendi();
  k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  assert.equal(k.conto_finale.note, 'Lavori ultimati senza varianti');
  await vai('Stati di avanzamento');
  assert.equal(await p().locator('button:has-text("+ Nuovo SAL")').isDisabled(), true);
  await p().waitForSelector('.banner:has-text("conto finale è in bozza")');
  await vai('Libretto delle misure');
  assert.match(await testo('tr.testata[data-blocco="5"]'), /stato finale/);

  // Emissione: contabilità chiusa.
  await vai('Conto finale');
  await p().click('button:has-text("Emetti conto finale")');
  await p().click('.modale button:has-text("Emetti")');
  await attendi();
  await p().waitForSelector('.scheda h2 .etichetta:has-text("emesso il 15/06/2026")');
  await p().waitForSelector('.banner:has-text("la contabilità è chiusa")');
  k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  assert.equal(k.chiusa, true);

  // Firma con riserve e certificato di regolare esecuzione approvato.
  await p().fill('input[name=firma]', '2026-06-30');
  await p().fill('textarea[name=riserve]', 'Riserva n. 1 confermata');
  await p().click('button:has-text("Registra firma")');
  await attendi();
  await p().waitForSelector('h3 .etichetta:has-text("firmato il 30/06/2026")');
  await p().selectOption('select[name=tipo_collaudo]', 'regolare_esecuzione');
  await p().fill('input[name=data_collaudo]', '2026-07-20');
  await p().fill('input[name=approvato_il]', '2026-07-31');
  await p().click('.scheda:has(h2:has-text("Firma e collaudo")) button:text-is("Registra")');
  await attendi();
  await p().waitForSelector('h3 .etichetta:has-text("CRE del 20/07/2026")');
  await p().waitForSelector('.scheda h2:has-text("rata di saldo") .etichetta:has-text("pagabile")');
  k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  assert.equal(k.conto_finale.firma_esecutore, '2026-06-30');
  assert.equal(k.conto_finale.riserve, 'Riserva n. 1 confermata');
  assert.deepEqual(k.conto_finale.collaudo, { tipo: 'regolare_esecuzione', data: '2026-07-20', approvato_il: '2026-07-31' });
  await p().locator('.scheda:has(h2:has-text("Firma e collaudo"))').scrollIntoViewIfNeeded();
  await schermata('16-conto-finale');

  await vai('Libretto delle misure');
  assert.equal(await p().locator('button:has-text("+ Registrazione")').isDisabled(), true);

  // Anteprima del conto finale.
  await vai('Conto finale');
  await p().click('button:has-text("Anteprima e stampa")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Conto finale dei lavori")');
  const foglio = await testo('.foglio-stampa');
  assert.match(foglio, /TOTALE DELLA RATA DI SALDO/);
  assert.match(foglio, /1\.176,45/);
  assert.match(foglio, /firmato il 30\/06\/2026/);
  await schermata('17-report-conto-finale');
});

test('analisi prezzi e nuovi prezzi: risorse, analisi, verbale, ribasso, stampe', async () => {
  await apriEsempio();
  const voce = async (codice) => (await b.motore({ richiesta: 'voce_prezzario', codice })).risultato;
  const risorsa = async (codice, descrizione, um, tipo, prezzo) => {
    await p().click('button:has-text("+ Risorsa")');
    await p().fill('.modale input[name=codice]', codice);
    await p().fill('.modale input[name=descrizione]', descrizione);
    await p().fill('.modale input[name=unita_misura]', um);
    await p().selectOption('.modale select[name=tipo]', tipo);
    await p().fill('.modale input[name=prezzo]', prezzo);
    await p().click('.modale button:has-text("Aggiungi")');
    await attendi();
  };
  await vai('Analisi prezzi');
  await risorsa('MO.02', 'Operaio comune', 'h', 'manodopera', '32');
  await risorsa('MA.20', 'Intonaco premiscelato in sacchi', 'kg', 'materiale', '0,50');
  assert.equal(await p().locator('.scheda:has(h2:has-text("Risorse elementari")) tbody tr').count(), 2);

  // Nuovo prezzo da analisi, già netto: si apre l'editor dell'analisi.
  await vai('Nuovi prezzi');
  await p().click('button:has-text("+ Nuovo prezzo")');
  await p().fill('.modale input[name=codice]', 'NP.01');
  await p().fill('.modale input[name=descrizione_breve]', 'Intonaco premiscelato');
  await p().fill('.modale input[name=unita_misura]', 'm²');
  await p().fill('.modale input[name=data]', '2026-04-08');
  await p().check('.modale input[name=netto]');
  await p().click('.modale button:has-text("Crea")');
  await attendi();
  await p().waitForSelector('.vista-testata h2:has-text("Analisi di NP.01")');
  await p().click('button:has-text("Crea l\'analisi")');
  await attendi();
  const aggiungi = async (codice, quantita) => {
    await p().click('.scheda button:has-text("+ Risorsa")');
    await p().selectOption('.modale select[name=risorsa]', codice);
    await p().fill('.modale input[name=quantita]', quantita);
    await p().click('.modale button:has-text("Aggiungi")');
    await attendi();
  };
  await aggiungi('MO.02', '0,25');
  await aggiungi('MA.20', '4*3');
  const prezzo = '.scheda:has(h2:has-text("Prezzo")) tr.totale td.num';
  assert.equal(await testo(prezzo), '17,71 €');
  assert.match(await testo('.scheda:has(h2:has-text("Prezzo"))'), /Manodopera 45,17 %/);
  let v = await voce('NP.01');
  assert.equal(v.prezzo, '17.71');
  assert.equal(v.incidenza_manodopera, '45.17');
  assert.equal(v.analisi.righe[1].quantita, '4*3');
  // Quantità modificata nella tabella: 0,5 h → 16,00 + 6,00 + 3,30 + 2,53 = 27,83.
  await p().fill('input[name=quantita_1]', '0,5');
  await p().keyboard.press('Enter');
  await attendi();
  assert.equal(await testo(prezzo), '27,83 €');
  await schermata('18-analisi');

  // Prezzo della risorsa cambiato: l'analisi lo usa subito, la voce si aggiorna a richiesta.
  await p().click('button:has-text("← Analisi")');
  await attendi();
  await p().click('.scheda:has(h2:has-text("Risorse elementari")) tr:has-text("MO.02")');
  await p().fill('.modale input[name=prezzo]', '33');
  await p().click('.modale button:has-text("Salva")');
  await attendi();
  await p().waitForSelector('td.differente:has-text("28,47")');
  await p().click('button:has-text("Aggiorna il prezzo dalle analisi")');
  await attendi();
  assert.equal(await p().locator('td.differente').count(), 0);
  assert.equal((await voce('NP.01')).prezzo, '28.47');
  await schermata('19-analisi-panoramica');

  // Verbale di concordamento approvato.
  await vai('Nuovi prezzi');
  await p().click('button:has-text("+ Verbale")');
  await p().fill('.modale input[name=data]', '2026-04-10');
  await p().fill('.modale input[name=firma]', '2026-04-12');
  await p().fill('.modale input[name=approvato]', '2026-04-15');
  await p().fill('.modale input[name=atto]', 'Determina RUP n. 12');
  await p().click('.modale button:has-text("Registra")');
  await attendi();
  await p().waitForSelector('td .etichetta:has-text("n. 1 approvato")');
  const np = (await b.motore({ richiesta: 'nuovi_prezzi' })).risultato;
  assert.equal(np.nuovi_prezzi[0].sigla, 'NP.1');
  assert.equal(np.nuovi_prezzi[0].soggetto_a_ribasso, false);
  assert.deepEqual(np.verbali[0].nuovi_prezzi, ['NP.01']);
  await schermata('20-nuovi-prezzi');

  // Misura con il nuovo prezzo: nel SAL è esclusa dal ribasso.
  await vai('Libretto delle misure');
  await p().click('text=+ Registrazione');
  await p().fill('.modale input[name=data]', '2026-05-20');
  await p().keyboard.press('Enter');
  await scegliNelSelettore('intonaco', 'NP.01');
  await p().waitForSelector('tr.testata[data-blocco="5"]');
  await scrivi('Pareti');
  await p().keyboard.press('Tab');
  await scrivi('10', 'Enter');
  await vai('Stati di avanzamento');
  await p().click('text=+ Nuovo SAL');
  await p().fill('.modale input[name=data]', '2026-05-31');
  await p().click('.modale button:has-text("Crea in bozza")');
  await attendi();
  assert.equal(await testo('table.prospetto tr:has-text("nuovi prezzi esclusi") td.num'), '284,70 €');
  const k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  assert.equal(k.sal[2].riepilogo.lavori_esclusi_dal_ribasso, '284.70');

  // Stampe: verbale con l'analisi, analisi completa e sintetica, fabbisogno.
  await vai('Nuovi prezzi');
  await p().click('.scheda:has(h2:has-text("Verbali")) button:has-text("Stampa")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Verbale di concordamento nuovi prezzi n. 1")');
  let foglio = await testo('.foglio-stampa');
  assert.match(foglio, /Determina RUP n\. 12/);
  assert.match(foglio, /Analisi del prezzo NP\.1 NP\.01/);
  await p().click('label:has-text("Analisi dei prezzi")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:text-is("Analisi dei prezzi")');
  assert.match(await testo('.foglio-stampa'), /PREZZO UNITARIO \(m²\)[\s\S]*28,47/);
  await p().check('input[name=sintetica]');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("quadro sintetico")');
  await p().click('label:has-text("Fabbisogno di cantiere")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Fabbisogno di cantiere")');
  assert.match(await testo('.foglio-stampa'), /Voci senza analisi/);

  // Elenco prezzi: colonna AP/NP e collegamenti dal dettaglio.
  await vai('Elenco prezzi');
  assert.match(await testo('.griglia-visibili'), /NP\.1 AP/);
});

test('analisi: variabili, oneri della sicurezza, aggiornamento prezzi, computo con analisi', async () => {
  await apriEsempio();
  const r = await b.motore({ richiesta: 'esegui', comandi: [
    { comando: 'imposta_risorsa', codice: 'MO.02', descrizione: 'Operaio comune', unita_misura: 'h', tipo: 'manodopera', prezzo: '32.00' },
    { comando: 'imposta_risorsa', codice: 'MA.01', descrizione: 'Cemento', unita_misura: 'q', tipo: 'materiale', prezzo: '15.00' },
    { comando: 'imposta_risorsa', codice: 'NO.01', descrizione: 'Betoniera', unita_misura: 'h', tipo: 'nolo', prezzo: '4.00' },
    { comando: 'aggiungi_voce_prezzario', codice: 'NP.INT', descrizione_breve: 'Intonaco', unita_misura: 'm²', prezzo: '0' },
    { comando: 'imposta_analisi', voce: 'NP.INT', righe: [
      { risorsa: 'MO.02', quantita: '0,17' }, { risorsa: 'MA.01', quantita: '0,21' }, { risorsa: 'NO.01', quantita: 'q1/2' }] },
    { comando: 'aggiungi_voce_computo', codice: 'NP.INT', righi: [{ lunghezza: 10 }] }] });
  assert.ok(r.ok, JSON.stringify(r));
  await vai('Analisi prezzi');
  await p().click('tr:has-text("NP.INT")');
  await attendi();
  const prezzo = '.scheda:has(h2:has-text("Prezzo")) tr.totale td.num';
  assert.equal(await testo(prezzo), '11,30 €');
  const variabile = async (nome, valore) => {
    await p().click('button:has-text("+ Variabile")');
    await p().fill('.modale input[name=nome]', nome);
    await p().fill('.modale input[name=valore]', valore);
    await p().click('.modale button:has-text("Aggiungi")');
    await attendi();
  };
  await variabile('spessore', '0,015');
  await variabile('kg', 'spessore*1400');
  assert.equal(await testo('.scheda:has(h2:has-text("Variabili")) tr:has-text("kg") td.num'), '21');
  for (const [n, q] of [[1, '0,02+spessore*10'], [2, 'kg/100']]) {
    await p().fill(`input[name=quantita_${n}]`, q);
    await p().keyboard.press('Enter');
    await attendi();
  }
  assert.match(await testo('tr:has(input[name=quantita_1])'), /= 0,17/);
  assert.equal(await testo(prezzo), '11,30 €');
  await p().fill('input[name=quota_analisi]', '20');
  await p().keyboard.press('Tab');
  await attendi();
  assert.equal(await testo('tr:has-text("oneri aziendali della sicurezza") td.num'), '0,27 €');
  await schermata('21-analisi-variabili');

  // Aggiornamento del 10 % della manodopera: 0,17 h × 35,20 = 5,98; prezzo 11,98.
  await p().click('button:has-text("← Analisi")');
  await attendi();
  await p().click('button:has-text("Aggiorna prezzi…")');
  await p().selectOption('.modale select[name=tipo]', 'manodopera');
  await p().fill('.modale input[name=percentuale]', '10');
  await p().click('.modale button:has-text("Aggiorna")');
  await attendi();
  await p().waitForSelector('.notifica.ok:has-text("prezzi di 1 risorse")');
  const v = (await b.motore({ richiesta: 'voce_prezzario', codice: 'NP.INT' })).risultato;
  assert.equal(v.prezzo, '11.98');
  assert.equal(v.analisi.variabili[0].espressione, '0,015');

  // Computo con le analisi delle voci.
  await vai('Report ed export');
  await p().check('input[name=con-analisi]');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("con analisi dei prezzi")');
  const foglio = await testo('.foglio-stampa');
  assert.match(foglio, /Spese generali/);
  assert.match(foglio, /voce senza analisi/);
});

test('report: anteprima, export PDF, XLSX, CSV e XPWE', async () => {
  await apriEsempio();
  await vai('Report ed export');
  await p().waitForSelector('.foglio-stampa h1:has-text("Computo metrico estimativo")');
  assert.match(await testo('.foglio-stampa'), /14\.457,12/);
  await p().click('label:has-text("Stato di avanzamento lavori")');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Stato di avanzamento lavori n. 2")');
  assert.equal(await testo('.foglio-stampa section:last-of-type tbody tr:last-child td:last-child'), '2.679,79');
  await schermata('09-report');

  const pdf = path.join(b.cartella, 'sal2');
  await b.dialogo(pdf);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("sal2.pdf")');
  assert.equal((await readFile(pdf + '.pdf')).subarray(0, 4).toString(), '%PDF');
  await p().click('.notifica.ok button:has-text("Apri")');
  assert.deepEqual((await b.simulazione()).aperti, [pdf + '.pdf']);

  const xlsx = path.join(b.cartella, 'sal2.xlsx');
  await b.dialogo(xlsx);
  await p().click('.dettaglio button:has-text("Excel")');
  await p().waitForSelector('.notifica.ok:has-text("sal2.xlsx")');
  assert.equal((await readFile(xlsx)).subarray(0, 2).toString(), 'PK');

  const csv = path.join(b.cartella, 'csv');
  await b.dialogo(csv);
  await p().click('.dettaglio button:has-text("CSV")');
  await p().waitForSelector('.notifica.ok:has-text("CSV")');
  assert.ok((await readdir(csv)).length >= 2);

  await p().click('label:has-text("Computo metrico estimativo")');
  await attendi();
  const xpwe = path.join(b.cartella, 'computo.xpwe');
  await b.dialogo(xpwe);
  await p().click('text=PriMus XPWE (sperimentale)…');
  await p().waitForSelector('.notifica:has-text("XPWE esportato")');
  assert.match(await readFile(xpwe, 'utf8'), /PweDGProgetto/);
});

test('report: modello tradizionale del computo e dell\'elenco prezzi', async () => {
  await apriEsempio();
  await vai('Report ed export');
  await p().waitForSelector('.foglio-stampa h1:has-text("Computo metrico estimativo")');
  await p().check('input[name=tradizionale]');
  await attendi();
  await p().waitForSelector('textarea[name=ente]');
  assert.ok(await p().isDisabled('.dettaglio button:has-text("Excel")'));
  assert.ok(await p().isDisabled('.dettaglio button:has-text("CSV")'));
  await p().fill('textarea[name=ente]', 'Comune di Esempio\nProvincia di Esempio');
  await p().locator('textarea[name=ente]').dispatchEvent('change');
  await p().fill('textarea[name=studio]', 'Studio tecnico di prova');
  await p().locator('textarea[name=studio]').dispatchEvent('change');
  await schermata('21-report-tradizionale');

  const computo = path.join(b.cartella, 'computo-tradizionale.pdf');
  await b.dialogo(computo);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("computo-tradizionale.pdf")');
  const byte = await readFile(computo);
  assert.equal(byte.subarray(0, 8).toString(), '%PDF-1.4');
  assert.match(byte.toString('latin1'), /Times-Roman/);

  // Elenco prezzi: la scelta del modello e i dati dello studio restano.
  await p().click('label:has-text("Elenco prezzi")');
  await attendi();
  assert.ok(await p().isChecked('input[name=tradizionale]'));
  assert.equal(await p().inputValue('textarea[name=ente]'), 'Comune di Esempio\nProvincia di Esempio');
  assert.equal(await p().locator('input[name=senza-prezzi]').count(), 0);
  const elenco = path.join(b.cartella, 'elenco-tradizionale.pdf');
  await b.dialogo(elenco);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("elenco-tradizionale.pdf")');
  assert.equal((await readFile(elenco)).subarray(0, 4).toString(), '%PDF');

  // Anche gli altri documenti di cantiere (ADR 0023): SAL e registro.
  await p().click('label:has-text("Stato di avanzamento lavori")');
  await attendi();
  assert.ok(await p().isChecked('input[name=tradizionale]'));
  const sal = path.join(b.cartella, 'sal-tradizionale.pdf');
  await b.dialogo(sal);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("sal-tradizionale.pdf")');
  const pdfSal = await readFile(sal);
  assert.equal(pdfSal.subarray(0, 8).toString(), '%PDF-1.4');
  assert.match(pdfSal.toString('latin1'), /Times-Roman/);
  await p().click('label:has-text("Registro di contabilità")');
  await attendi();
  const registro = path.join(b.cartella, 'registro-tradizionale.pdf');
  await b.dialogo(registro);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("registro-tradizionale.pdf")');
  assert.equal((await readFile(registro)).subarray(0, 4).toString(), '%PDF');
  // Computo con le incidenze nel modello tradizionale.
  await p().click('label:has-text("Computo metrico estimativo")');
  await attendi();
  await p().check('input[name=incidenza-manodopera]');
  await attendi();
  const conIncidenze = path.join(b.cartella, 'computo-incidenze-tradizionale.pdf');
  await b.dialogo(conIncidenze);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("computo-incidenze-tradizionale.pdf")');
  await p().uncheck('input[name=incidenza-manodopera]');
  await attendi();

  // Tornando al modello ordinario si riattivano XLSX e CSV.
  await p().uncheck('input[name=tradizionale]');
  await attendi();
  assert.ok(!(await p().isDisabled('.dettaglio button:has-text("Excel")')));

  // Elenco prezzi in DCF (sperimentale).
  await p().click('label:has-text("Elenco prezzi")');
  await attendi();
  const dcf = path.join(b.cartella, 'elenco');
  await b.dialogo(dcf);
  await p().click('text=PriMus DCF (sperimentale)…');
  await p().waitForSelector('.notifica:has-text("DCF esportato: 4 voci")');
  assert.equal((await readFile(dcf + '.dcf')).subarray(0, 8).toString(), 'AAMVHFSS');
});

test('report: incidenze di manodopera e sicurezza e subtotali nel riepilogo', async () => {
  await apriEsempio();
  await vai('Report ed export');
  await p().waitForSelector('.foglio-stampa h1:has-text("Computo metrico estimativo")');
  assert.ok(await p().isDisabled('input[name=subtotali-incidenze]'));
  await p().check('input[name=incidenza-manodopera]');
  await attendi();
  await p().waitForSelector('.foglio-stampa th:has-text("Manodopera €")');
  assert.equal(await p().locator('.foglio-stampa th:has-text("Sicurezza €")').count(), 0);
  // Quota della prima voce: 1.071,00 × 25 % (dal core), totale della manodopera.
  assert.match(await testo('.foglio-stampa section:first-of-type tr.sommano'), /267,75\s*25,00/);
  assert.match(await testo('.foglio-stampa section:first-of-type tr.totale:last-child'), /3\.759,29\s*26,00/);
  // Senza subtotali i riepiloghi per categoria non cambiano.
  assert.equal(await p().locator('.foglio-stampa section:nth-of-type(3) th:has-text("Manodopera €")').count(), 0);
  await p().check('input[name=incidenza-sicurezza]');
  await attendi();
  await p().check('input[name=subtotali-incidenze]');
  await attendi();
  await p().waitForSelector('.foglio-stampa section:nth-of-type(3) th:has-text("Manodopera €")');
  assert.match(await testo('.foglio-stampa section:nth-of-type(3)'), /Sicurezza €/);
  assert.match(await testo('.foglio-stampa section:nth-of-type(2)'), /voci senza incidenza della sicurezza/);
  await schermata('27-report-incidenze');

  // Modello tradizionale: una pagina di riepilogo in più.
  const pagine = async (nome) => {
    const file = path.join(b.cartella, nome);
    await b.dialogo(file);
    await p().click('.dettaglio button:has-text("PDF…")');
    await p().waitForSelector(`.notifica.ok:has-text("${nome}")`);
    return Number(/\/Type\s*\/Pages[^>]*\/Count (\d+)/.exec((await readFile(file)).toString('latin1'))[1]);
  };
  await p().check('input[name=tradizionale]');
  await attendi();
  const conRiepilogo = await pagine('computo-incidenze.pdf');
  await p().uncheck('input[name=subtotali-incidenze]');
  await attendi();
  assert.equal(await pagine('computo-senza-riepilogo.pdf'), conRiepilogo - 1);
  // Nel computo metrico senza prezzi le incidenze non si offrono.
  await p().check('input[name=senza-prezzi]');
  await attendi();
  assert.equal(await p().locator('input[name=incidenza-manodopera]').count(), 0);
  await p().uncheck('input[name=senza-prezzi]');
  await attendi();
  // Le scelte restano tra una visita e l'altra; si ripuliscono per i test successivi.
  assert.ok(await p().isChecked('input[name=incidenza-manodopera]'));
  await p().uncheck('input[name=incidenza-manodopera]');
  await attendi();
  await p().uncheck('input[name=incidenza-sicurezza]');
  await attendi();
  await p().uncheck('input[name=tradizionale]');
  await attendi();
});

test('aggiornamento da listino: confronto, scelta delle voci, effetto sui computi, annulla', async () => {
  await apriEsempio();
  // Nuovo listino: scavo a 9,00, descrizione nuova della muratura, recinzione assente.
  const doc = JSON.parse(await readFile(FIXTURE, 'utf8'));
  const voci = doc.progetto.elenco_prezzi.voci;
  voci.find((v) => v.codice === 'A.01.001').prezzo = '9.00';
  voci.find((v) => v.codice === 'C.03.020').descrizione_breve = 'Muratura in blocchi di laterizio porizzato';
  doc.progetto.elenco_prezzi.voci = voci.filter((v) => v.codice !== 'S.01.001');
  const listino = path.join(b.cartella, 'regionale-2027.json');
  await writeFile(listino, JSON.stringify(doc));

  await vai('Elenco prezzi');
  await p().click('text=Aggiorna da listino…');
  await attendi();
  await p().waitForSelector('.vista[aria-label="Aggiornamento da listino"]');
  await b.dialogo(listino);
  await p().click('text=Carica listino…');
  await attendi();
  await p().waitForSelector('.griglia-riga[data-indice="3"]');
  const esito = async (i) => (await testo(`.griglia-riga[data-indice="${i}"] .cella:last-child`));
  assert.deepEqual([await esito(0), await esito(1), await esito(2), await esito(3)], ['Variata', 'Invariata', 'Variata', 'Non trovata']);
  // Variazione e importi nei computi dal motore: 126,00 m³ × 9,00 = 1.134,00 (+63,00).
  assert.equal(await testo('.griglia-riga[data-indice="0"] .aumento'), '+5,88');
  assert.equal(await testo('.griglia-riga[data-indice="0"] .cella:nth-child(9)'), '+63,00');
  // Scelta iniziale: solo le voci con differenze nei campi da aggiornare (il prezzo).
  const scelta = (i) => p().isChecked(`.griglia-riga[data-indice="${i}"] input.spunta`);
  assert.deepEqual([await scelta(0), await scelta(1), await scelta(2)], [true, false, false]);
  assert.ok(await p().isDisabled('.griglia-riga[data-indice="3"] input.spunta'));
  await p().waitForSelector('.riepilogo-aggiornamento .totale-computo');
  assert.match(await testo('.riepilogo-aggiornamento .totale-computo'), /14\.457,12 → 14\.520,12 \(\+63,00 €\)/);
  // Confronto completo della voce selezionata.
  await p().click('.griglia-riga[data-indice="2"] .cella:nth-child(3)');
  await p().waitForSelector('.dettaglio .confronto-campi');
  assert.match(await testo('.dettaglio .confronto-campi'), /porizzato/);
  assert.equal(await p().locator('.dettaglio .confronto-campi tr.non-aggiornato').count(), 1);
  // Con le descrizioni anche la muratura è scelta; senza, torna fuori.
  await p().check('input[name=campo-descrizioni]');
  assert.ok(await scelta(2));
  await schermata('28-aggiornamento-listino');
  await p().uncheck('input[name=campo-descrizioni]');
  assert.ok(!(await scelta(2)));
  // La casella toglie e rimette la voce; senza scelte il pulsante si disattiva.
  await p().click('.griglia-riga[data-indice="0"] input.spunta');
  assert.match(await testo('.riepilogo-aggiornamento .conteggio-scelte'), /Scelte: 0 voci/);
  assert.ok(await p().isDisabled('button[name=aggiorna-scelte]'));
  await p().click('.griglia-riga[data-indice="0"] .cella:nth-child(2)');
  await p().keyboard.press('Space');
  assert.ok(await scelta(0));
  // Filtro per esito.
  await p().click('.pannello-filtri label:has-text("Variata") input');
  await attendi();
  assert.equal(await p().locator('.griglia-visibili .griglia-riga').count(), 2);
  await p().click('.pannello-filtri .filtri-testata button:has-text("Azzera")');
  await attendi();

  await p().click('button[name=aggiorna-scelte]');
  await p().waitForSelector('.notifica.ok:has-text("1 voce aggiornata, 1 voce di computo riallineata")');
  await attendi();
  const c = await b.motore({ richiesta: 'computo' });
  assert.equal(c.risultato.totale, '14520.12');
  assert.equal(await esito(0), 'Invariata');
  await p().waitForSelector('.riepilogo-aggiornamento:has-text("Scelte: 0 voci")');
  await p().click('header button:has-text("Annulla")');
  await attendi();
  assert.equal((await b.motore({ richiesta: 'computo' })).risultato.totale, '14457.12');
  assert.equal(await esito(0), 'Variata');
});

test('report: le quattro stampe del computo tradizionale (senza, manodopera, sicurezza, entrambe)', async () => {
  await apriEsempio();
  await vai('Report ed export');
  await p().waitForSelector('.foglio-stampa h1:has-text("Computo metrico estimativo")');
  await p().check('input[name=tradizionale]');
  await attendi();
  // Le caselle restano disponibili con il modello tradizionale, in qualsiasi combinazione.
  assert.ok(await p().isVisible('input[name=incidenza-manodopera]'));
  assert.ok(await p().isVisible('input[name=incidenza-sicurezza]'));
  const stampe = new Map();
  for (const [nome, mdo, sic] of [['senza', false, false], ['manodopera', true, false], ['sicurezza', false, true], ['entrambe', true, true]]) {
    await p().setChecked('input[name=incidenza-manodopera]', mdo);
    await attendi();
    await p().setChecked('input[name=incidenza-sicurezza]', sic);
    await attendi();
    if (nome === 'entrambe') await schermata('29-report-tradizionale-incidenze');
    const file = path.join(b.cartella, `computo-${nome}.pdf`);
    await b.dialogo(file);
    await p().click('.dettaglio button:has-text("PDF…")');
    await p().waitForSelector(`.notifica.ok:has-text("computo-${nome}.pdf")`);
    stampe.set(nome, (await readFile(file)).toString('latin1'));
  }
  // Quattro stampe diverse.
  assert.equal(new Set(stampe.values()).size, 4);
  await p().setChecked('input[name=incidenza-manodopera]', false);
  await attendi();
  await p().setChecked('input[name=incidenza-sicurezza]', false);
  await attendi();
  await p().uncheck('input[name=tradizionale]');
  await attendi();
});

test('import XPWE dalla schermata iniziale', async () => {
  await ricomincia();
  await b.dialogo(XPWE);
  await p().click('text=Importa da PriMus');
  await p().waitForSelector('.modale:has-text("Importazione completata")');
  await p().keyboard.press('Escape');
  await attendi();
  assert.equal(await totale(), '9.291,84 €');
  assert.match(await testo('.titolo-progetto small'), /non salvato/);
});

test('controlli: avvisi e navigazione alla voce', async () => {
  await apriEsempio();
  await b.motore({ richiesta: 'esegui', comandi: [{ comando: 'modifica_voce_prezzario', codice: 'C.03.020', prezzo: '43.00' }] });
  await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' }));
  await attendi();
  await vai('Controlli');
  await p().click('.schede-tab button:has-text("Informazioni")');
  await p().waitForSelector('.elenco-problemi li.informazione:has-text("C.03.020")');
  await schermata('10-controlli');
  await p().click('.elenco-problemi li.navigabile:has-text("C.03.020")');
  await attendi();
  await p().waitForSelector('table.misure tr.testata.scelta[data-blocco="3"]');
});

test('finestra secondaria aggiornata dalle modifiche della principale', async () => {
  await apriEsempio();
  await p().click('button[aria-label="Nuova finestra"]');
  assert.equal((await b.simulazione()).finestre[0].vista, 'computo&computo=Computo%20metrico%20estimativo');
  const seconda = await b.finestra('elenco');
  await attendi(seconda);
  assert.equal(await seconda.locator('nav').count(), 0, 'senza navigazione');
  await seconda.waitForSelector('.griglia-riga:has-text("A.01.001")');
  // Modifica nella principale: la secondaria si aggiorna.
  await vai('Elenco prezzi');
  await p().click('text=+ Nuova voce');
  await p().fill('.modale input[name=codice]', 'Z.99');
  await p().fill('.modale input[name=descrizione_breve]', 'Voce nuova');
  await p().fill('.modale input[name=unita_misura]', 'cad');
  await p().fill('.modale input[name=prezzo]', '1');
  await p().click('.modale button:has-text("Aggiungi")');
  await attendi();
  await seconda.waitForSelector('.griglia-riga:has-text("Z.99")');
  await schermata('11-finestra-secondaria', seconda);
  await seconda.close();
});

test('trascinamento di voci tra finestre: elenco → computo, computo → computo', async () => {
  await apriEsempio();
  const codiciDi = async (nome) => (await b.motore({ richiesta: 'computo', computo: nome })).risultato.voci.map((v) => v.codice);
  const principale = 'Computo metrico estimativo';
  await b.motore({ richiesta: 'esegui', comandi: [{ comando: 'nuovo_computo', nome: 'Variante' }] });
  await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' }));
  await attendi();

  // Ctrl/⌘+clic su una voce del menu apre la vista in una nuova finestra.
  await p().click('nav button:has-text("Variante")', { modifiers: ['ControlOrMeta'] });
  const fin = (await b.simulazione()).finestre;
  assert.equal(fin.at(-1).vista, 'computo&computo=Variante');
  assert.equal(fin.at(-1).titolo, 'Variante — OpenComputo');

  // Elenco prezzi nella principale, computo «Variante» in una seconda finestra.
  const variante = await b.finestra('computo&computo=Variante');
  await attendi(variante);
  await variante.waitForSelector('.vista-testata h2:has-text("Variante")');
  await vai('Elenco prezzi');
  await p().click('.griglia-riga:has-text("A.01.001")');
  await p().click('.griglia-riga:has-text("C.03.020")', { modifiers: ['Shift'] });
  assert.equal(await p().locator('.griglia-riga.selezionata').count(), 3);
  const dati = await b.trascina(p(), '.griglia-riga.selezionata', variante, '.foglio');
  assert.equal(dati['text/plain'], 'A.01.001\nB.02.010\nC.03.020');
  await variante.waitForSelector('.notifica:has-text("3 voci aggiunte")');
  assert.deepEqual(await codiciDi('Variante'), ['A.01.001', 'B.02.010', 'C.03.020']);
  await attendi(variante);
  await variante.waitForSelector('table.misure tr.testata[data-blocco="3"]');

  // Dal computo principale (due voci selezionate) alla variante, prima della voce 1:
  // copia; il sistema passa solo il testo, i dati arrivano dal motore.
  await vai(principale);
  await p().click('table.misure tr.testata[data-blocco="2"] td.c-codice');
  await p().click('table.misure tr.testata[data-blocco="4"] td.c-codice', { modifiers: ['ControlOrMeta'] });
  assert.equal(await p().locator('table.misure tr.testata.scelta').count(), 2);
  await b.trascina(p(), 'table.misure tr.testata[data-blocco="4"]', variante, 'table.misure tr.testata[data-blocco="1"]', { soloTesto: true, sopra: true });
  await variante.waitForSelector('.notifica:has-text("2 voci copiate da")');
  assert.deepEqual(await codiciDi('Variante'), ['B.02.010', 'S.01.001', 'A.01.001', 'B.02.010', 'C.03.020']);
  assert.equal((await codiciDi(principale)).length, 4, 'la copia lascia l\'origine invariata');
  const totale = async (nome) => (await b.motore({ richiesta: 'computo', computo: nome })).risultato.totale;
  assert.equal(await totale(principale), '14457.12');

  // Con Maiusc si sposta: la voce lascia il computo d'origine.
  await attendi(variante);
  await b.trascina(variante, 'table.misure tr.testata[data-blocco="5"]', p(), '.foglio', { tasti: { shiftKey: true } });
  await p().waitForSelector('.notifica:has-text("spostata da «Variante»")');
  assert.deepEqual(await codiciDi(principale), ['A.01.001', 'B.02.010', 'C.03.020', 'S.01.001', 'C.03.020']);
  assert.equal((await codiciDi('Variante')).length, 4);

  // Nello stesso computo si sposta: la voce 1 dopo la 3; un solo annulla.
  await attendi();
  await b.trascina(p(), 'table.misure tr.testata[data-blocco="1"]', p(), 'table.misure tr.testata[data-blocco="3"]');
  await p().waitForSelector('.notifica:has-text("Voce spostata")');
  assert.deepEqual(await codiciDi(principale), ['B.02.010', 'C.03.020', 'A.01.001', 'S.01.001', 'C.03.020']);
  await attendi();
  await p().waitForSelector('table.misure tr.testata.scelta[data-blocco="3"]');
  await schermata('22-trascinamento', variante);
  await p().keyboard.press('ControlOrMeta+z');
  await attendi();
  assert.deepEqual(await codiciDi(principale), ['A.01.001', 'B.02.010', 'C.03.020', 'S.01.001', 'C.03.020']);
  // Il trascinamento finito non lascia dati nel motore.
  assert.equal(await p().evaluate(() => window.__ocTrascina('leggi', 'null')), 'null');
  await variante.close();
});

test('voci affiancate al computo: «>» e «<», tastiera, prezzario', async () => {
  await apriEsempio();
  const codici = async () => (await b.motore({ richiesta: 'computo' })).risultato.voci.map((v) => v.codice);
  const pannello = '.pannello-voci';
  await p().click('button:has-text("Affianca voci")');
  await attendi();
  await p().waitForSelector(`${pannello} .griglia-riga[data-indice="3"]`);
  assert.equal(await p().getAttribute('button:has-text("Affianca voci")', 'aria-pressed'), 'true');
  // Le tariffe già usate nel computo sono segnate.
  assert.equal(await p().locator(`${pannello} .griglia-riga.nel-computo`).count(), 4);

  // Due voci a sinistra, «>»: entrano dopo la voce selezionata nel computo.
  await p().click('table.misure tr.testata[data-blocco="2"] td.c-codice');
  await p().click(`${pannello} .griglia-riga:has-text("A.01.001")`);
  await p().click(`${pannello} .griglia-riga:has-text("C.03.020")`, { modifiers: ['ControlOrMeta'] });
  await p().click('.frecce button[aria-label="Aggiungi al computo"]');
  await p().waitForSelector('.notifica:has-text("2 voci aggiunte")');
  await attendi();
  assert.deepEqual(await codici(), ['A.01.001', 'B.02.010', 'A.01.001', 'C.03.020', 'C.03.020', 'S.01.001']);
  assert.equal(await p().locator('table.misure tr.testata.scelta').count(), 2);
  // Il fuoco resta nella lista, per continuare.
  assert.ok(await p().evaluate((s) => document.activeElement === document.querySelector(`${s} .griglia-corpo`), pannello));

  // Da tastiera: ricerca, freccia giù, Invio; la voce va dopo quelle appena aggiunte.
  await p().fill(`${pannello} input[type=search]`, 'S.01');
  await p().waitForFunction((s) => {
    const r = [...document.querySelectorAll(`${s} .griglia-visibili .griglia-riga`)];
    return r.length === 1 && r[0].textContent.includes('S.01.001');
  }, pannello);
  await p().press(`${pannello} input[type=search]`, 'ArrowDown');
  await p().keyboard.press('Enter');
  await p().waitForSelector('.notifica:has-text("Voce «S.01.001» aggiunta")');
  await attendi();
  assert.deepEqual(await codici(), ['A.01.001', 'B.02.010', 'A.01.001', 'C.03.020', 'S.01.001', 'C.03.020', 'S.01.001']);
  await schermata('23-affianca');

  // «<» toglie la voce selezionata; un solo annulla la ripristina.
  await p().click('.frecce button[aria-label="Togli dal computo"]');
  await p().waitForSelector('.notifica:has-text("Voce tolta dal computo")');
  await attendi();
  assert.deepEqual(await codici(), ['A.01.001', 'B.02.010', 'A.01.001', 'C.03.020', 'C.03.020', 'S.01.001']);
  await p().click('.notifica button:has-text("Annulla")');
  await attendi();
  assert.equal((await codici()).length, 7);
  // Più voci: Ctrl+clic sulle testate, poi «<».
  await p().click('table.misure tr.testata[data-blocco="3"] td.c-codice');
  await p().click('table.misure tr.testata[data-blocco="5"] td.c-codice', { modifiers: ['ControlOrMeta'] });
  await p().click('.frecce button[aria-label="Togli dal computo"]');
  await p().waitForSelector('.notifica:has-text("2 voci tolte dal computo")');
  await attendi();
  assert.deepEqual(await codici(), ['A.01.001', 'B.02.010', 'C.03.020', 'C.03.020', 'S.01.001']);

  // Da un prezzario di consultazione: la voce entra anche nell'elenco del progetto.
  await b.motore({ richiesta: 'carica_prezzario', nome: 'sintetico', percorso: XPWE });
  await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' }));
  await attendi();
  await p().fill(`${pannello} input[type=search]`, '');
  await p().selectOption(`${pannello} select`, 'sintetico');
  await p().waitForSelector(`${pannello} small:text-is("6 voci")`);
  await attendi();
  // Le voci già nell'elenco del progetto sono attenuate.
  await p().waitForSelector(`${pannello} .griglia-riga.presente:has-text("S.01.001")`);
  const nuova = (await p().locator(`${pannello} .griglia-visibili .griglia-riga:not(.presente) .cella`).first().innerText()).trim();
  await p().click('table.misure tr.testata[data-blocco="5"] td.c-codice');
  await p().dblclick(`${pannello} .griglia-visibili .griglia-riga:not(.presente)`);
  await p().waitForSelector('.notifica:has-text("da «sintetico»")');
  await attendi();
  assert.equal((await codici()).at(-1), nuova);
  const info = await b.motore({ richiesta: 'info' });
  assert.equal(info.risultato.elenco_prezzi.voci, 5);
  await p().waitForSelector(`${pannello} .griglia-riga.presente.nel-computo:has-text("${nuova}")`);

  // Il pannello si chiude e la scelta resta nelle preferenze.
  await p().click('button:has-text("Affianca voci")');
  await attendi();
  assert.equal(await p().locator(pannello).count(), 0);
  assert.equal(await p().evaluate(() => localStorage.getItem('opencomputo.affianca')), 'false');
});

test('vedi voce: più voci citate in righi diversi, collegamento, libretto statico', async () => {
  await apriEsempio();
  const computo = async () => (await b.motore({ richiesta: 'computo' })).risultato;
  // Rigo 1 della voce 4 (recinzione, 100,00) attivo, poi «Vedi voce…».
  await p().click('tr.rigo[data-blocco="4"][data-rigo="1"] td[data-campo="descrizione"]');
  await p().click('button:has-text("Vedi voce…")');
  await p().waitForSelector('.modale .griglia-visibili .griglia-riga');
  // La voce stessa non è proposta.
  assert.equal(await p().locator('.modale .griglia-visibili .griglia-riga').count(), 3);
  await p().click('.modale .griglia-riga:has-text("A.01.001")');
  await p().click('.modale .griglia-riga:has-text("B.02.010")', { modifiers: ['ControlOrMeta'] });
  await p().click('.modale button:has-text("Inserisci")');
  await p().waitForSelector('.notifica:has-text("2 righi «vedi voce» nella voce n. 4")');
  await attendi();
  let c = await computo();
  assert.equal(c.voci[3].righi.length, 3);
  assert.equal(c.voci[3].righi[1].vedi_voce.testo, 'Vedi voce n. 1');
  assert.equal(c.voci[3].righi[2].vedi_voce.numero, 2);
  assert.equal(c.voci[3].quantita, '277.74'); // 100 + 126,00 + 51,74
  assert.equal(await testo('tr.rigo[data-blocco="4"][data-rigo="2"] .vedi-voce'), 'Vedi voce n. 1 [m³ 126,00]');
  await schermata('24-vedi-voce');

  // Cambia la voce citata: cambia la quantità della voce 4.
  await p().click('tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="altezza_peso"]');
  await scrivi('1,70', 'Enter');
  c = await computo();
  assert.equal(c.voci[0].quantita, '174.00');
  assert.equal(c.voci[3].quantita, '325.74');

  // Clic sul riferimento: si va alla voce citata.
  await p().click('tr.rigo[data-blocco="4"][data-rigo="3"] .vedi-voce');
  await p().waitForSelector('table.misure tr.testata.scelta[data-blocco="2"]');
  await p().waitForSelector('.dettaglio .citata-da:has-text("n. 4")');

  // Dalla voce 1 non si propongono la voce stessa né la 4, che la cita già.
  await p().click('tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="descrizione"]');
  await p().click('button:has-text("Vedi voce…")');
  await p().waitForSelector('.modale .griglia-visibili .griglia-riga');
  assert.equal(await p().locator('.modale .griglia-visibili .griglia-riga').count(), 2);
  assert.equal(await p().locator('.modale .griglia-riga:has-text("S.01.001")').count(), 0);
  await p().keyboard.press('Escape');
  await p().waitForSelector('.modale', { state: 'detached' });

  // Eliminata la voce 2, il rigo che la cita conserva la quantità (statico).
  await p().click('table.misure tr.testata[data-blocco="2"] td.c-codice');
  await p().click('text=Elimina voce');
  await p().click('.modale button:has-text("Elimina")');
  await attendi();
  c = await computo();
  assert.equal(c.voci[2].righi[2].vedi_voce.statica, true);
  assert.equal(c.voci[2].quantita, '325.74');
  await p().waitForSelector('tr.rigo[data-blocco="3"][data-rigo="3"] .vedi-voce.statica');

  // Libretto: una registrazione modificabile, «Vedi voce…» copia la quantità.
  await b.motore({ richiesta: 'esegui', comandi: [{ comando: 'registra', data: '2026-06-10', codice: 'A.01.001', righi: [{ descrizione: 'Platea', lunghezza: 10 }] }] });
  await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' }));
  await vai('Libretto delle misure');
  const ultima = (await b.motore({ richiesta: 'contabilita' })).risultato.registrazioni.at(-1).numero;
  await p().click(`tr.rigo[data-blocco="${ultima}"][data-rigo="1"] td[data-campo="descrizione"]`);
  await p().click('button:has-text("Vedi voce…")');
  await p().waitForSelector('.modale .griglia-riga:has-text("A.01.001")');
  await p().click('.modale .griglia-riga:has-text("A.01.001")');
  await p().click('.modale button:has-text("Inserisci")');
  await p().waitForSelector('.notifica:has-text("1 rigo «vedi voce» nella registrazione")');
  await attendi();
  const reg = (await b.motore({ richiesta: 'contabilita' })).risultato.registrazioni.at(-1);
  assert.equal(reg.righi[1].vedi_voce.statica, true);
  assert.equal(reg.righi[1].quantita, '174.00');
  assert.equal(reg.quantita, '184.00');
  await p().waitForSelector(`tr.rigo[data-blocco="${ultima}"][data-rigo="2"] .vedi-voce.statica`);
});

test('due finestre: la modifica va alla voce giusta anche se l\'altra finestra rinumera', async () => {
  // Regressione (campagna di test del 2026-10-22): i comandi indicavano la voce per
  // numero, e un'eliminazione nell'altra finestra spostava la modifica sulla voce dopo.
  await apriEsempio();
  await p().click('nav button:has-text("Computo metrico estimativo")', { modifiers: ['ControlOrMeta'] });
  const altra = await b.finestra('computo&computo=Computo%20metrico%20estimativo');
  await altra.waitForSelector('table.misure');
  await attendi(altra);
  await altra.click('tr.rigo[data-blocco="2"][data-rigo="1"] td[data-campo="descrizione"]');
  await altra.keyboard.press('F2');
  await altra.keyboard.type('Platea bis');
  await p().click('table.misure tr.testata[data-blocco="1"] td.c-codice');
  await p().click('text=Elimina voce');
  await p().click('.modale button:has-text("Elimina")');
  await attendi();
  await altra.keyboard.press('Enter');
  await attendi(altra);
  const c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci[0].codice, 'B.02.010');
  assert.equal(c.voci[0].righi[0].descrizione, 'Platea bis');
  assert.equal(c.voci[1].righi[0].descrizione, 'Piano terra');
  await altra.close();
});

test('pannello dei filtri: computo ed elenco prezzi', async () => {
  await apriEsempio();
  const visibili = async () => (await p().locator('table.misure tr.testata td.c-codice').allInnerTexts()).map((t) => t.trim());
  const casella = (sezione, chiave) => `.pannello-filtri input[data-sezione="${sezione}"][data-chiave="${chiave}"]`;
  await p().click('.vista-testata button:has-text("Filtri")');
  await attendi();
  await p().waitForSelector('.pannello-filtri');
  // Categorie: Scavi o Strutture.
  await p().check(casella('categoria', 'SC'));
  await p().check(casella('categoria', 'ST'));
  await attendi();
  assert.deepEqual(await visibili(), ['A.01.001', 'B.02.010']);
  assert.equal(await testo('.barra-filtro span'), 'Filtro attivo: 2 di 4 voci');
  assert.equal(await testo('.totali .filtrato strong'), '8.588,82 €');
  // I conteggi della WBS tengono conto del filtro: 2 voci sotto «01».
  assert.match(await testo('label:has(input[data-sezione="wbs"][data-chiave="01"]) .filtri-conteggio'), /^2 · 8\.588,82$/);
  // WBS: un nodo comprende i discendenti.
  await p().uncheck(casella('categoria', 'SC'));
  await p().uncheck(casella('categoria', 'ST'));
  await p().check(casella('wbs', '01.02'));
  await attendi();
  assert.deepEqual(await visibili(), ['C.03.020']);
  await p().uncheck(casella('wbs', '01.02'));
  await p().check(casella('wbs', '01'));
  await attendi();
  assert.deepEqual(await visibili(), ['A.01.001', 'B.02.010', 'C.03.020']);
  // Colonne: importo da 2.000 €, poi una tariffa che non c'è.
  await p().fill('.pannello-filtri input[data-sezione="importo"][data-estremo="da"]', '2000');
  await p().waitForFunction(() => document.querySelectorAll('table.misure tr.testata').length === 2);
  await attendi();
  assert.deepEqual(await visibili(), ['B.02.010', 'C.03.020']);
  await p().fill('.pannello-filtri input[data-sezione="tariffa"]', 'S.01');
  await p().waitForSelector('.foglio-vuoto:has-text("Nessuna voce corrisponde ai filtri")');
  await p().click('.filtri-testata button:has-text("Azzera")');
  await attendi();
  assert.equal((await visibili()).length, 4);
  // Una voce aggiunta con una sola categoria scelta la riceve.
  await p().check(casella('categoria', 'MU'));
  await attendi();
  await p().click('table.misure tr.testata[data-blocco="3"] td.c-codice');
  await p().click('text=+ Voce');
  await scegliNelSelettore('B.02.010', 'B.02.010');
  await p().waitForSelector('.notifica:has-text("assegnata a MU dal filtro")');
  const c = await b.motore({ richiesta: 'computo' });
  assert.equal(c.risultato.voci[3].codice, 'B.02.010');
  assert.equal(c.risultato.voci[3].categorie.categoria, 'MU');
  assert.deepEqual(await visibili(), ['C.03.020', 'B.02.010']);
  await schermata('25-filtri-computo');
  await p().click('.filtri-testata button:has-text("Azzera")');
  await p().click('.vista-testata button:has-text("Filtri")');
  await attendi();

  // Elenco prezzi: natura, uso nel computo per categoria, prezzo.
  await vai('Elenco prezzi');
  await p().click('.vista-testata button:has-text("Filtri")');
  await attendi();
  const righe = async () => (await p().locator('.zona .griglia-visibili .griglia-riga .cella:first-child').allInnerTexts()).map((t) => t.trim());
  await p().check(casella('natura', 'oneri_sicurezza'));
  await p().waitForFunction(() => document.querySelectorAll('.zona .griglia-visibili .griglia-riga').length === 1);
  assert.deepEqual(await righe(), ['S.01.001']);
  await p().uncheck(casella('natura', 'oneri_sicurezza'));
  await p().check(casella('categoria', 'MU'));
  await p().waitForFunction(() => document.querySelectorAll('.zona .griglia-visibili .griglia-riga').length === 2);
  assert.deepEqual(await righe(), ['B.02.010', 'C.03.020']);
  await p().uncheck(casella('categoria', 'MU'));
  await p().fill('.pannello-filtri input[data-sezione="prezzo"][data-estremo="a"]', '10');
  await p().waitForFunction(() => document.querySelectorAll('.zona .griglia-visibili .griglia-riga').length === 1);
  assert.deepEqual(await righe(), ['A.01.001']);
  assert.match(await testo('.vista-testata .vuoto'), /^1 di 4/);
  await schermata('26-filtri-elenco');
  await p().click('.filtri-testata button:has-text("Azzera")');
  await p().click('.vista-testata button:has-text("Filtri")');
  await attendi();
});

test('chiusura con modifiche non salvate', async () => {
  await apriEsempio();
  await b.motore({ richiesta: 'esegui', comandi: [{ comando: 'imposta_metadati', luogo: 'Altrove' }] });
  await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' }));
  await attendi();
  await p().evaluate(() => window.__ocEvento('chiusura-richiesta', null));
  await p().waitForSelector('.modale:has-text("Salvare le modifiche")');
  await p().click('.modale button:has-text("Annulla")');
  assert.equal((await b.simulazione()).uscito, false);
  await p().evaluate(() => window.__ocEvento('chiusura-richiesta', null));
  await p().click('.modale button:has-text("Non salvare")');
  await p().waitForFunction(() => window.__oc.uscito === true);
});

/** Simula un evento degli appunti sul foglio delle misure; restituisce i dati scritti. */
async function appunti(tipo, dati = {}) {
  return p().evaluate(([t, d]) => {
    const dt = new DataTransfer();
    for (const [k, v] of Object.entries(d)) dt.setData(k, v);
    document.querySelector('.foglio').dispatchEvent(new ClipboardEvent(t, { clipboardData: dt, bubbles: true, cancelable: true }));
    return { testo: dt.getData('text/plain'), righi: dt.getData('application/x-opencomputo-righi') };
  }, [tipo, dati]);
}

test('copia e incolla di righi, anche da un foglio di calcolo', async () => {
  await apriEsempio();
  // Due righi selezionati con Maiusc+Giù, copiati in formato per Excel e interno.
  await p().click('tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="descrizione"]');
  await p().keyboard.press('Shift+ArrowDown');
  assert.equal(await p().locator('tr.rigo.nella-selezione').count(), 2);
  const copiati = await appunti('copy');
  assert.equal(copiati.testo, 'Platea\t\t12,00\t8,00\t1,20\t\r\nRampa di accesso\t\t6,00\t3,00\t1,20\t0,5*lun*lar*alt\r\n');
  // Incollati sotto il rigo 3 della voce 3: la formula si conserva.
  await p().click('tr.rigo[data-blocco="3"][data-rigo="3"] td[data-campo="descrizione"]');
  await appunti('paste', { 'text/plain': copiati.testo, 'application/x-opencomputo-righi': copiati.righi });
  await attendi();
  let c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.deepEqual(c.voci[2].righi.slice(3, 5).map((r) => [r.descrizione, r.quantita, r.formula]),
    [['Platea', '115.20', null], ['Rampa di accesso', '10.80', '0,5*lun*lar*alt']]);
  // Da Excel: colonne a partire da quella attiva, separatori delle migliaia tolti.
  await p().click('tr.rigo[data-blocco="4"][data-rigo="nuovo"] td[data-campo="descrizione"]');
  await appunti('paste', { 'text/plain': 'Lato A\t2\t5,5\t\t3\r\nLato B\t1\t1.234,5\t\t2\r\n' });
  await attendi();
  c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.deepEqual(c.voci[3].righi.slice(1).map((r) => [r.descrizione, r.quantita]), [['Lato A', '33.00'], ['Lato B', '2469.00']]);
  // Un valore solo sostituisce la cella attiva.
  await p().click('tr.rigo[data-blocco="4"][data-rigo="2"] td[data-campo="altezza_peso"]');
  await appunti('paste', { 'text/plain': '4' });
  await attendi();
  c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci[3].righi[1].quantita, '44.00');
  // Selezione eliminata con Ctrl+Canc.
  await p().click('tr.rigo[data-blocco="4"][data-rigo="2"] td[data-campo="descrizione"]');
  await p().keyboard.press('Shift+ArrowDown');
  await p().keyboard.press(`${mod}+Delete`);
  await attendi();
  c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci[3].righi.length, 1);
  await schermata('12-incolla');
});

test('categorie e WBS modificabili, annotazioni del libretto', async () => {
  await apriEsempio();
  await vai('Categorie, WBS, computi');
  await p().click('button[aria-label="Modifica ST"]');
  await p().fill('.modale input[name=descrizione]', 'Strutture in c.a.');
  await p().click('.modale button:has-text("OK")');
  await attendi();
  const pr = (await b.motore({ richiesta: 'progetto' })).risultato;
  assert.equal(pr.categorie.find((c) => c.codice === 'ST').descrizione, 'Strutture in c.a.');
  await p().click('button[aria-label="Rimuovi 01.01"]');
  await p().click('.modale button:has-text("Rimuovi")');
  await p().waitForSelector('.notifica.errore:has-text("assegnato a")');
  await schermata('13-categorie');

  await vai('Libretto delle misure');
  await p().click('text=+ Registrazione');
  await p().fill('.modale input[name=data]', '2026-05-20');
  await p().keyboard.press('Enter');
  await scegliNelSelettore('scavo', 'A.01.001');
  await p().fill('.dettaglio textarea[name=annotazioni]', 'Presente l\'impresa');
  await p().locator('.dettaglio textarea[name=annotazioni]').blur();
  await attendi();
  const k = (await b.motore({ richiesta: 'contabilita' })).risultato;
  assert.equal(k.registrazioni[4].annotazioni, 'Presente l\'impresa');
});

test('file recenti nella schermata iniziale', async () => {
  const ultimo = await apriEsempio();
  await ricomincia();
  await p().waitForSelector('.recenti');
  // Al massimo 8, il più recente in cima.
  assert.ok((await p().locator('.recenti small').count()) <= 8);
  assert.equal(await p().locator('.recenti small').first().innerText(), ultimo);
  await p().locator('.recenti button:has-text("Nuova scuola primaria")').first().click();
  await attendi();
  assert.equal(await testo('.titolo-progetto small'), path.basename(ultimo));
  assert.equal(await totale(), '14.457,12 €');
});

test('cronoprogramma: inizio, durate, legami, livelli, pietra miliare, trascinamento, linea di base, Gantt', async () => {
  await apriEsempio();
  await vai('Cronoprogramma');
  // Senza inizio dei lavori il piano non si calcola: il banner porta al calendario.
  await p().click('button[name=imposta-inizio]');
  await p().fill('.modale input[name=inizio-lavori]', '2026-03-02');
  await p().click('.modale button:has-text("Applica")');
  await attendi();
  const righe = () => p().locator('.crono-righe .crono-riga');
  assert.equal(await righe().count(), 4);
  const cella = (n, col) => p().locator(`.crono-righe .crono-riga[data-indice="${n - 1}"] [data-col="${col}"]`);
  assert.equal((await cella(2, 'durata').innerText()).trim(), '1 g?');

  // Durate da tastiera: una cifra apre la modifica, Invio conferma e scende.
  await cella(2, 'wbs').click();
  await p().keyboard.type('10');
  await p().keyboard.press('Enter');
  await attendi();
  await p().keyboard.type('15');
  await p().keyboard.press('Enter');
  await attendi();
  // Predecessori scritti come in un programma di project management.
  await cella(3, 'predecessori').dblclick();
  await p().keyboard.type('2');
  await p().keyboard.press('Enter');
  await attendi();
  assert.equal((await cella(2, 'fine').innerText()).trim(), '13/03/26');
  assert.equal((await cella(3, 'inizio').innerText()).trim(), '16/03/26');
  assert.equal((await cella(3, 'fine').innerText()).trim(), '03/04/26');
  assert.equal((await cella(1, 'durata').innerText()).trim(), '25 g');
  assert.equal(await p().locator('.crono-gantt .legame').count(), 1);
  let c = (await b.motore({ richiesta: 'cronoprogramma' })).risultato;
  assert.equal(c.piano.fine, '2026-04-03');
  assert.equal(c.piano.legami[0].tipo, 'fine_inizio');
  await schermata('30-cronoprogramma');

  // Un ciclo è rifiutato.
  await cella(2, 'predecessori').dblclick();
  await p().keyboard.type('3');
  await p().keyboard.press('Enter');
  await p().waitForSelector('.notifica.errore:has-text("ciclo")');
  await p().keyboard.press('Escape');
  await attendi();

  // Livello di dettaglio: solo il primo livello della WBS.
  await p().selectOption('select[name=livello-dettaglio]', '1');
  await attendi();
  assert.equal(await righe().count(), 2);
  assert.equal((await cella(1, 'durata').innerText()).trim(), '25 g');
  await p().selectOption('select[name=livello-dettaglio]', '');
  await attendi();
  assert.equal(await righe().count(), 4);

  // Pietra miliare dopo le elevazioni.
  await cella(3, 'wbs').click();
  await p().click('button[name=nuova-pietra]');
  await p().selectOption('.modale select[name=padre]', '');
  await p().click('.modale button:has-text("Aggiungi")');
  await attendi();
  assert.equal(await righe().count(), 5);
  const pietra = p().locator('.crono-righe .crono-riga:has-text("Ultimazione dei lavori")');
  assert.equal((await pietra.locator('[data-col="fine"]').innerText()).trim(), '03/04/26');
  assert.equal(await p().locator('.crono-gantt .barra.pietra').count(), 1);

  // Riepilogo compresso: le attività contenute spariscono dalla tabella e dal Gantt.
  await p().click('.crono-righe .crono-riga[data-indice="0"] .crono-apri');
  await attendi();
  assert.equal(await righe().count(), 3);
  await p().click('.crono-righe .crono-riga[data-indice="0"] .crono-apri');
  await attendi();
  assert.equal(await righe().count(), 5);

  // Trascinamento della barra della sicurezza di una settimana (24 px al giorno).
  const barra = p().locator('.crono-gantt .barra[data-indice="3"] rect.forma');
  await barra.scrollIntoViewIfNeeded();
  const r = await barra.boundingBox();
  await p().mouse.move(r.x + 4, r.y + r.height / 2);
  await p().mouse.down();
  await p().mouse.move(r.x + 4 + 84, r.y + r.height / 2, { steps: 5 });
  await p().mouse.move(r.x + 4 + 168, r.y + r.height / 2, { steps: 5 });
  await p().mouse.up();
  await attendi();
  c = (await b.motore({ richiesta: 'cronoprogramma' })).risultato;
  const sicurezza = c.piano.righe.find((x) => x.codice === '02');
  assert.equal(sicurezza.inizio_non_prima, '2026-03-09');
  assert.equal((await cella(4, 'inizio').innerText()).trim(), '09/03/26');

  // Dettagli: avanzamento e note dal pannello inferiore.
  await cella(2, 'wbs').click();
  await p().fill('.crono-pannello textarea[name=dettaglio-note]', 'Getto in due fasi');
  await p().locator('.crono-pannello textarea[name=dettaglio-note]').blur();
  await attendi();
  c = (await b.motore({ richiesta: 'cronoprogramma' })).risultato;
  assert.equal(c.piano.righe[1].note, 'Getto in due fasi');

  // Linea di base, poi produzione mensile.
  await p().click('button[name=linea-base]');
  await attendi();
  c = (await b.motore({ richiesta: 'cronoprogramma' })).risultato;
  assert.ok(c.linea_base);
  assert.ok(await p().locator('.crono-gantt .barra rect.base').count() >= 3);
  await p().click('button[name=pannello-produzione]');
  await attendi();
  assert.ok(await p().locator('.crono-mesi tbody tr').count() >= 1);
  await p().waitForSelector('.crono-curva polyline.cumulata');
  await schermata('31-cronoprogramma-produzione');
  await p().click('button[name=pannello-attivita]');
  await attendi();

  // Annulla torna indietro di un comando.
  await p().keyboard.press(`${mod}+z`);
  await attendi();
  c = (await b.motore({ richiesta: 'cronoprogramma' })).risultato;
  assert.equal(c.linea_base, null);

  // Stampa: diagramma di Gantt in PDF; niente XLSX/CSV per il diagramma.
  await p().click('button[name=stampa-crono]');
  await attendi();
  await p().waitForSelector('.foglio-stampa h1:has-text("Cronoprogramma")');
  assert.ok(await p().isChecked('input[name=gantt]'));
  assert.ok(await p().isDisabled('.dettaglio button:has-text("Excel (XLSX)…")'));
  const file = path.join(b.cartella, 'gantt.pdf');
  await b.dialogo(file);
  await p().click('.dettaglio button:has-text("PDF…")');
  await p().waitForSelector('.notifica.ok:has-text("gantt.pdf")');
  assert.ok((await readFile(file)).subarray(0, 5).toString() === '%PDF-');
  await p().uncheck('input[name=gantt]');
  await attendi();
  assert.ok(!(await p().isDisabled('.dettaglio button:has-text("Excel (XLSX)…")')));

  // Il cronoprogramma è nel file ma non nell'export XPWE.
  const xpwe = path.join(b.cartella, 'crono.xpwe');
  await b.motore({ richiesta: 'esporta_xpwe', percorso: xpwe, computo: null });
  const testoXpwe = await readFile(xpwe, 'utf8');
  assert.ok(!/Ultimazione dei lavori/.test(testoXpwe));
});

test('nessun errore JavaScript né violazione della Content Security Policy', () => {
  assert.deepEqual(b.errori, []);
});
