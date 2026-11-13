// Ridimensionamento della finestra: con un titolo dei lavori molto lungo, tariffe lunghe
// e percentuali con molte cifre (come in un XPWE reale) la pagina non deve mai essere più
// larga della finestra e le colonne dei numeri devono restare raggiungibili.
// I dati sono sintetici, scritti qui.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { apriBanco } from './banco.mjs';

let b;
const p = () => b.pagina;

const TITOLO = 'Stima di opere provvisionali per la messa in sicurezza di un portale storico mediante struttura metallica indipendente, sistemi di ripartizione, protezione degli elementi lapidei, ritegni, zavorre e apprestamenti di sicurezza';
const voceElenco = (id, tariffa, prezzo, mdo) => `<EPItem ID="${id}"><TipoEP>0</TipoEP><Tariffa>${tariffa}</Tariffa><Articolo>${tariffa}</Articolo>
  <DesRidotta>Carpenteria metallica in profilati tubolari per travi e pilastri, completi di piastre di base e di attacco, compresi tagli e sfridi ${id}</DesRidotta>
  <DesEstesa>Carpenteria metallica in profilati tubolari per travi e pilastri, completi di piastre di base e di attacco, compresi tagli e sfridi ${id}</DesEstesa>
  <UnMisura>kg</UnMisura><Prezzo1>${prezzo}</Prezzo1><IncMDO>${mdo}</IncMDO><IncSIC>0.660418935033961</IncSIC></EPItem>`;
const rigo = (d, pu, l, h) => `<RGItem><IDVV>-2</IDVV><Descrizione>${d}</Descrizione><PartiUguali>${pu}</PartiUguali><Lunghezza>${l}</Lunghezza><Larghezza></Larghezza><HPeso>${h}</HPeso><Quantita></Quantita><Flags>0</Flags></RGItem>`;
const XPWE = `<?xml version="1.0"?><PweDocumento><PweDatiGenerali><PweDGProgetto><PweDGDatiGenerali><Oggetto>${TITOLO}</Oggetto></PweDGDatiGenerali></PweDGProgetto></PweDatiGenerali>
<PweMisurazioni><PweElencoPrezzi>${voceElenco(1, 'REG26_E19.040.030.A', '1234.56', '28.6200495049505')}${voceElenco(2, 'REG26_E19.010.030.R', '7.69', '31.9456')}</PweElencoPrezzi>
<PweVociComputo><VCItem ID="1"><IDEP>1</IDEP><PweVCMisure>${rigo('montanti tubolari del telaio principale lato strada', '8', '4.1', '6.437')}${rigo('correnti', '1', '28.8', '3.559')}${rigo('piastre e bulloneria', '', '', '184.32')}</PweVCMisure></VCItem>
<VCItem ID="2"><IDEP>2</IDEP><PweVCMisure>${rigo('trave principale', '1', '3.6', '51.2')}</PweVCMisure></VCItem></PweVociComputo></PweMisurazioni></PweDocumento>`;

async function attendi() {
  await p().waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
  await p().waitForTimeout(60);
}

async function vai(voce) {
  await p().locator('nav button', { hasText: voce }).first().click();
  await attendi();
}

/** Larghezza della pagina e posizione (bordo destro) delle intestazioni indicate. */
const misure = (selettore, titoli) => p().evaluate(([sel, titoli]) => {
  const d = document.documentElement;
  const teste = [...document.querySelectorAll(sel)];
  const destra = {};
  for (const t of titoli) {
    const e = teste.find((x) => x.textContent.trim().startsWith(t));
    destra[t] = e ? Math.round(e.getBoundingClientRect().right) : null;
  }
  return { pagina: d.scrollWidth, finestra: d.clientWidth, destra };
}, [selettore, titoli]);

before(async () => {
  b = await apriBanco();
  const file = path.join(b.cartella, 'largo.xpwe');
  await writeFile(file, XPWE);
  const r = await b.motore({ richiesta: 'importa', origine: file });
  assert.equal(r.ok, true, JSON.stringify(r));
  await p().goto(b.indirizzo);
  await attendi();
});
after(async () => { await b?.chiudi(); });

for (const larghezza of [1600, 1320, 1100, 960]) {
  test(`finestra larga ${larghezza}: la pagina non esce dalla finestra, importi visibili`, async () => {
    await p().setViewportSize({ width: larghezza, height: 800 });
    await vai('Computo metrico estimativo');
    await p().locator('table.misure tr.testata').first().click();
    await attendi();
    let m = await misure('table.misure thead th', ['Quantità', 'Prezzo', 'Importo']);
    assert.equal(m.pagina, m.finestra, 'la pagina è più larga della finestra');
    // A 1100 e oltre le colonne dei numeri sono visibili senza scorrere, anche con una
    // voce selezionata (pannello di dettaglio aperto).
    if (larghezza >= 1100) assert.ok(m.destra.Importo <= larghezza, `Importo a ${m.destra.Importo} px`);
    // La tariffa intera e i fattori si leggono.
    const celle = await p().evaluate(() => [...document.querySelectorAll('table.misure td.c-codice, table.misure tr.rigo td.c-fatt')]
      .filter((c) => c.textContent.trim() && c.scrollWidth > c.clientWidth + 1).map((c) => c.textContent.trim()));
    assert.deepEqual(celle, [], 'celle troncate');
    // I titoli della barra restano raggiungibili.
    const salva = await p().locator('.barra button', { hasText: 'Salva' }).first().boundingBox();
    assert.ok(salva && salva.x + salva.width <= larghezza, 'pulsante Salva fuori dalla finestra');

    await vai('Elenco prezzi');
    m = await misure('.griglia-intestazione .cella', ['Prezzo', 'M.O.']);
    assert.equal(m.pagina, m.finestra);
    assert.ok(m.destra.Prezzo <= larghezza && m.destra['M.O.'] <= larghezza, JSON.stringify(m.destra));
    // Percentuali con molte cifre: mostrate con due decimali.
    const riga = await p().locator('.griglia-visibili .griglia-riga').first().innerText();
    assert.match(riga, /28,62 %/);
    assert.ok(!riga.includes('28,6200'));
  });
}

test('finestra secondaria stretta (640): le colonne si raggiungono scorrendo, intestazione allineata', async () => {
  await p().setViewportSize({ width: 640, height: 600 });
  await vai('Elenco prezzi');
  const m = await p().evaluate(async () => {
    const corpo = document.querySelector('.griglia-corpo');
    corpo.scrollLeft = corpo.scrollWidth;
    await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
    const sin = (e) => Math.round(e.getBoundingClientRect().left);
    const teste = [...document.querySelectorAll('.griglia-intestazione .cella')];
    const celle = [...document.querySelector('.griglia-visibili .griglia-riga').children];
    const i = teste.findIndex((t) => t.textContent.trim().startsWith('Prezzo'));
    return { pagina: document.documentElement.scrollWidth, finestra: document.documentElement.clientWidth, scorre: corpo.scrollLeft > 0,
      testa: sin(teste[i]), cella: sin(celle[i]), destra: Math.round(celle[i].getBoundingClientRect().right) };
  });
  assert.equal(m.pagina, m.finestra);
  assert.ok(m.scorre, 'la griglia non scorre in orizzontale');
  assert.equal(m.testa, m.cella, 'intestazione non allineata alle righe');
  assert.ok(m.destra <= 640, 'colonna del prezzo non raggiungibile');
  await vai('Computo metrico estimativo');
  const f = await p().evaluate(() => { const f = document.querySelector('.foglio'); return { pagina: document.documentElement.scrollWidth, scorre: f.scrollWidth > f.clientWidth }; });
  assert.equal(f.pagina, 640);
  assert.ok(f.scorre);
});

test('anteprima del modello tradizionale: pagine del PDF, le successive a richiesta', async () => {
  // Un computo lungo: più pagine di quelle mostrate subito.
  const voci = Array.from({ length: 70 }, (_, i) => `<VCItem ID="${i + 1}"><IDEP>${(i % 2) + 1}</IDEP><PweVCMisure>${rigo(`parte ${i + 1}`, '2', '3.5', '1.2')}${rigo('altra parte', '1', '2', '')}</PweVCMisure></VCItem>`).join('');
  const file = path.join(b.cartella, 'lungo.xpwe');
  await writeFile(file, XPWE.replace(/<PweVociComputo>.*<\/PweVociComputo>/s, `<PweVociComputo>${voci}</PweVociComputo>`));
  assert.equal((await b.motore({ richiesta: 'importa', origine: file })).ok, true);
  await p().setViewportSize({ width: 1320, height: 800 });
  await p().goto(b.indirizzo);
  await attendi();
  await vai('Report ed export');
  await p().waitForSelector('.foglio-stampa');
  await p().check('input[name=tradizionale]');
  const caricate = () => p().waitForFunction(() => { const i = [...document.querySelectorAll('img.pagina-stampa')]; return i.length && i.every((x) => x.complete && x.naturalWidth > 0) && !document.querySelector('button[name=altre-pagine]:disabled'); });
  await caricate();
  const stato = () => p().evaluate(() => ({ pagine: document.querySelectorAll('img.pagina-stampa').length, testo: document.querySelector('.pagine-conteggio').textContent,
    pagina: document.documentElement.scrollWidth - document.documentElement.clientWidth, larga: Math.round(document.querySelector('img.pagina-stampa').getBoundingClientRect().width) }));
  let s = await stato();
  const totale = Number(/di (\d+)/.exec(s.testo)[1]);
  assert.ok(totale > 6, `pagine: ${totale}`);
  assert.equal(s.pagine, 6);
  assert.equal(s.pagina, 0, 'la pagina esce dalla finestra');
  assert.ok(s.larga > 500, `pagina larga ${s.larga} px`);
  while (await p().locator('button[name=altre-pagine]').count()) {
    const prima = (await stato()).pagine;
    await p().click('button[name=altre-pagine]');
    await p().waitForFunction((n) => document.querySelectorAll('img.pagina-stampa').length > n, prima);
    await caricate();
  }
  s = await stato();
  assert.equal(s.pagine, totale);
  assert.match(s.testo, new RegExp(`^${totale} pagine`));
  // L'ultima pagina porta il totale, la penultima il riporto.
  const ultima = await p().evaluate(() => decodeURIComponent([...document.querySelectorAll('img.pagina-stampa')].at(-1).src));
  assert.match(ultima, /T O T A L E|TOTALE/);
  // Finestra stretta: le pagine si rimpiccioliscono, non escono.
  await p().setViewportSize({ width: 960, height: 700 });
  await p().waitForTimeout(150);
  s = await stato();
  assert.equal(s.pagina, 0);
  assert.ok(s.larga <= 960 - 290 - 150);
});

test('nessun errore JavaScript né violazione della Content Security Policy', () => {
  assert.deepEqual(b.errori, []);
});
