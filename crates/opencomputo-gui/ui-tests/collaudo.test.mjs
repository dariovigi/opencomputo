// Collaudo dell'interfaccia (fuori dalla suite: `npm run collaudo`): dati ostili in tutte le
// viste, errori del motore, progetto vuoto, finestra stretta, scorciatoie con le finestre di
// dialogo aperte, doppi clic, tempi di un computo grande (stampati, non verificati).
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { copyFile } from 'node:fs/promises';
import path from 'node:path';
import { apriBanco, RADICE } from './banco.mjs';

const V6 = path.join(RADICE, 'crates/opencomputo-core/tests/fixtures/progetto_esempio_v6.json');
let b;
before(async () => { b = await apriBanco(); });
after(async () => { await b?.chiudi(); });
const p = () => b.pagina;
async function attendi() {
  await p().waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0', null, { timeout: 60000 });
  await p().waitForTimeout(30);
}
async function apri(file = V6) {
  await b.motore({ richiesta: 'chiudi', forza: true });
  await p().goto(b.indirizzo); await attendi();
  const f = path.join(b.cartella, `e-${Date.now()}.ocp`); await copyFile(file, f);
  await b.dialogo(f); await p().click('text=Apri progetto'); await p().waitForSelector('nav'); await attendi();
}
const aggiorna = async () => { await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' })); await attendi(); };
const VISTE = ['Dati generali', 'Elenco prezzi', 'Analisi prezzi', 'Categorie, WBS, computi', 'Computo metrico estimativo', 'Libretto delle misure', 'Nuovi prezzi', 'Stati di avanzamento', 'Conto finale', 'Report ed export', 'Controlli'];
const note = [];
const righi = async (n) => (await b.motore({ richiesta: 'computo' })).risultato.voci[n - 1].righi.length;

test('dati ostili in tutte le viste: nessun HTML interpretato', async () => {
  await apri(V6);
  const X = '<img src=x onerror="window.__xss=1"><b>grassetto</b>';
  const r = await b.motore({ richiesta: 'esegui', comandi: [
    { comando: 'imposta_metadati', titolo: `Titolo ${X}`, committente: X, luogo: X },
    { comando: 'aggiungi_voce_prezzario', codice: `Z${X}`.slice(0, 40), descrizione_breve: X, descrizione_estesa: X, unita_misura: '<u>m</u>', prezzo: '1' },
    { comando: 'aggiungi_voce_computo', codice: `Z${X}`.slice(0, 40), righi: [{ descrizione: X, lunghezza: '2' }, { descrizione: X, commento: true }] },
    { comando: 'aggiungi_categoria', codice: '<i>C</i>', descrizione: X, livello: 'categoria' },
    { comando: 'aggiungi_nodo_wbs', codice: '09', descrizione: X },
    { comando: 'nuovo_computo', nome: `Comp ${X}` },
    { comando: 'registra', data: '2026-06-01', codice: 'A.01.001', righi: [{ descrizione: X, lunghezza: 1 }], annotazioni: X },
  ] });
  assert.equal(r.ok, true, JSON.stringify(r));
  await aggiorna();
  for (const v of [...VISTE, `Comp ${X}`]) {
    await p().locator('nav button', { hasText: v.slice(0, 20) }).first().click();
    await attendi();
    const trovati = await p().evaluate(() => ({ img: document.querySelectorAll('main img, main b, nav b, .vista img').length, xss: !!window.__xss, onerror: document.querySelectorAll('[onerror]').length }));
    assert.deepEqual(trovati, { img: 0, xss: false, onerror: 0 }, v);
  }
  // Anteprima del report del computo con i testi ostili.
  await p().locator('nav button', { hasText: 'Report ed export' }).first().click(); await attendi();
  assert.equal(await p().evaluate(() => document.querySelectorAll('[onerror]').length), 0);
  assert.deepEqual(b.errori.filter((e) => !/Failed to load resource/.test(e)), []);
});

test('errori del motore mostrati e foglio ripristinato', async () => {
  await apri(V6);
  await p().locator('nav button', { hasText: 'Computo metrico estimativo' }).first().click(); await attendi();
  const cella = 'tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="lunghezza"]';
  const prima = (await p().locator(cella).innerText()).trim();
  await p().click(cella);
  await p().keyboard.type('2*(3');
  await p().keyboard.press('Enter');
  await attendi();
  const modale = await p().locator('.modale, .notifica.errore, .banner.errore').count();
  note.push(`espressione non valida: modale/notifica visibile=${modale > 0}; cella dopo=${(await p().locator(cella).innerText()).trim()} (prima ${prima})`);
  if (await p().locator('.modale').count()) { await p().keyboard.press('Escape'); await attendi(); }
  const dopo = (await p().locator(cella).innerText()).trim();
  note.push(`cella dopo la chiusura: «${dopo}»; quantità voce 1 dal motore: ${(await b.motore({ richiesta: 'computo' })).risultato.voci[0].quantita}`);
  // Valore che introdurrebbe un errore di validazione: lunghezza enorme → overflow.
  await p().click(cella); await p().keyboard.type('79228162514264337593543950335'); await p().keyboard.press('Enter'); await attendi();
  note.push(`overflow: modale=${await p().locator('.modale').count()}; motore q=${(await b.motore({ richiesta: 'computo' })).risultato.voci[0].quantita}`);
  if (await p().locator('.modale').count()) { await p().keyboard.press('Escape'); await attendi(); }
  assert.deepEqual(b.errori.filter((e) => !/Failed to load resource/.test(e)), []);
});

test('tutte le viste su un progetto nuovo e senza computi', async () => {
  await b.motore({ richiesta: 'chiudi', forza: true });
  await p().goto(b.indirizzo); await attendi();
  await p().click('text=Nuovo progetto'); await p().fill('.modale input', 'Vuoto'); await p().click('.modale button:has-text("Crea")'); await attendi();
  for (const v of VISTE) {
    const pulsante = p().locator('nav button', { hasText: v }).first();
    if (!(await pulsante.count())) { note.push(`vista assente nel menu: ${v}`); continue; }
    await pulsante.click(); await attendi();
  }
  // Un comando che toglie l'unico computo? Non esiste: si prova un progetto con computo vuoto e libretto vuoto.
  assert.deepEqual(b.errori.filter((e) => !/Failed to load resource/.test(e)), []);
});

test('computo grande: tempi di disegno e modifica', async () => {
  await b.motore({ richiesta: 'chiudi', forza: true });
  await p().goto(b.indirizzo); await attendi();
  await b.motore({ richiesta: 'nuovo', titolo: 'Grande' });
  const cmds = [{ comando: 'aggiungi_voce_prezzario', codice: 'V', descrizione_breve: 'Voce', unita_misura: 'm', prezzo: '1' }];
  for (let i = 0; i < 1500; i++) cmds.push({ comando: 'aggiungi_voce_computo', codice: 'V', righi: [1, 2, 3, 4].map((j) => ({ descrizione: `r${j}`, lunghezza: String(j), larghezza: '2' })) });
  const r = await b.motore({ richiesta: 'esegui', comandi: cmds });
  assert.equal(r.ok, true);
  await p().goto(b.indirizzo); await attendi();
  let t = Date.now();
  await p().locator('nav button', { hasText: 'Computo metrico estimativo' }).first().click(); await attendi();
  const apertura = Date.now() - t;
  await p().click('tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="lunghezza"]');
  t = Date.now();
  await p().keyboard.type('5'); await p().keyboard.press('Enter'); await attendi();
  const modifica = Date.now() - t;
  const righe = await p().locator('table.misure tr').count();
  note.push(`computo di 1500 voci (${righe} righe nel DOM): apertura ${apertura} ms, modifica di una cella ${modifica} ms`);
});

test('finestra stretta (900×700): nessuno scorrimento orizzontale della pagina', async () => {
  await p().setViewportSize({ width: 900, height: 700 });
  await apri(V6);
  for (const v of VISTE) {
    await p().locator('nav button', { hasText: v }).first().click(); await attendi();
    const larga = await p().evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
    if (larga) note.push(`vista «${v}»: la pagina scorre in orizzontale a 900 px`);
  }
  await p().setViewportSize({ width: 1600, height: 900 });
});

test('scorciatoie con una finestra di dialogo aperta', async () => {
  await apri(V6);
  await b.motore({ richiesta: 'esegui', comandi: [{ comando: 'imposta_metadati', luogo: 'Prova annulla' }] });
  await p().evaluate(() => window.__ocEvento('progetto-aggiornato', { origine: 'altra' })); await attendi();
  await p().click('tr.rigo[data-blocco="4"][data-rigo="1"] td[data-campo="descrizione"]');
  await p().click('text=Elimina voce');
  await p().waitForSelector('.modale');
  await p().keyboard.press('ControlOrMeta+z'); await attendi();
  const annullabili = (await b.motore({ richiesta: 'info' })).risultato.annullabili;
  note.push(`Ctrl+Z con la conferma aperta: annullabili dopo=${JSON.stringify(annullabili)}; modale ancora aperta=${await p().locator('.modale').count()}`);
  assert.deepEqual(annullabili, ['metadati aggiornati'], 'Ctrl+Z non deve agire sotto una finestra di dialogo');
  await p().keyboard.press('Escape'); await attendi();
  // Delete con la modale «Vedi voce» aperta: non deve svuotare celle sotto.
  await p().click('tr.rigo[data-blocco="1"][data-rigo="1"] td[data-campo="lunghezza"]');
  await p().click('button:has-text("Vedi voce…")'); await p().waitForSelector('.modale');
  await p().keyboard.press('Delete'); await p().keyboard.press('Backspace');
  await p().keyboard.press('Escape'); await attendi();
  const v1 = (await b.motore({ richiesta: 'computo' })).risultato.voci[0].righi[0].lunghezza;
  note.push(`Canc con «Vedi voce» aperta: lunghezza del rigo 1 = ${v1}`);
  assert.equal(v1, '12,00');
});

test('doppio clic veloce su «Inserisci» e Invio ripetuto', async () => {
  await apri(V6);
  await p().click('tr.rigo[data-blocco="4"][data-rigo="1"] td[data-campo="descrizione"]');
  await p().click('button:has-text("Vedi voce…")'); await p().waitForSelector('.modale .griglia-riga');
  await p().click('.modale .griglia-riga:has-text("A.01.001")');
  await p().dblclick('.modale button:has-text("Inserisci")'); await attendi();
  note.push(`doppio clic su Inserisci: righi della voce 4 = ${await righi(4)} (attesi 2)`);
  assert.equal(await righi(4), 2);
  // Invio due volte in rapida successione su «+ nuovo rigo».
  await p().click('tr.rigo[data-blocco="1"][data-rigo="nuovo"] td[data-campo="descrizione"]');
  await p().keyboard.type('X'); await p().keyboard.press('Enter'); await p().keyboard.press('Enter'); await attendi();
  note.push(`Invio ripetuto sul nuovo rigo: righi della voce 1 = ${await righi(1)} (attesi 3)`);
  // Tre Ctrl+Z rapidi.
  await p().keyboard.press('ControlOrMeta+z'); await p().keyboard.press('ControlOrMeta+z'); await p().keyboard.press('ControlOrMeta+z'); await attendi();
  note.push(`dopo tre Ctrl+Z rapidi: righi voce 1=${await righi(1)}, voce 4=${await righi(4)}`);
});

test('riepilogo note', () => { console.log('NOTE:\n' + note.map((n) => ' - ' + n).join('\n')); });
