// Vista «Assistente AI» quando qualcosa va storto: configurazione danneggiata, provider
// sovraccarico, risposta del modello non valida, risposta lenta annullata, provider
// irraggiungibile. Motore reale e provider finto locale; nessuna chiamata reale.
//
// Uso: cargo build -p opencomputo-agent-api --features banco-di-prova && npm test

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { apriBanco, RADICE } from './banco.mjs';
import { avviaProviderFinto } from './provider-finto.mjs';

const CSV = path.join(RADICE, 'crates/opencomputo-assistente/tests/fixtures/prezzario_prova.csv');

let b;
let finto;

before(async () => {
  finto = await avviaProviderFinto();
  // File di configurazione troncato, come dopo una scrittura interrotta.
  b = await apriBanco({ provider: finto.url, configurazione: '{"consenso": {"versione": 1, "quan' });
  await b.motore({ richiesta: 'nuovo', titolo: 'Robustezza' });
  await b.motore({ richiesta: 'carica_prezzario', nome: 'prezzario_prova', percorso: CSV });
  await b.motore({ richiesta: 'assistente_chiave', provider: 'anthropic', chiave: 'sk-ant-prova' });
  await b.motore({ richiesta: 'assistente_chiave', provider: 'openai', chiave: 'sk-openai-prova' });
  await b.pagina.goto(b.indirizzo);
  await attendi();
});
after(async () => { await b?.chiudi(); finto?.chiudi(); });

const p = () => b.pagina;

async function attendi() {
  await p().waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
  await p().waitForTimeout(30);
  await p().waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
}

async function notifica(tipo) {
  const n = p().locator(`.notifica.${tipo}`).last();
  await n.waitFor();
  return (await n.innerText()).trim();
}

const chiudiNotifiche = () => p().evaluate(() => document.querySelectorAll('.notifica').forEach((n) => n.remove()));
const messaggi = () => finto.ricevute.filter((r) => r.percorso === '/v1/messages').length;

async function invia(testo) {
  await p().fill('textarea[name="richiesta-ai"]', testo);
  await p().click('button[name="proponi"]');
}

async function proponi(testo) {
  await invia(testo);
  await p().waitForFunction(() => !document.querySelector('.ai-attesa'), null, { timeout: 60000 });
  await attendi();
}

test('configurazione danneggiata: avviso, valori predefiniti, consenso da ridare', async () => {
  await p().locator('nav button', { hasText: 'Assistente AI' }).click();
  await attendi();
  const avviso = await p().locator('.ai-avviso-config').innerText();
  assert.match(avviso, /non era leggibile/);
  assert.match(avviso, /consenso va dato di nuovo/);
  assert.ok(existsSync(path.join(b.config, 'assistente.json.danneggiato')));
  // Nessuna proposta possibile finché il consenso non viene ridato.
  await p().waitForSelector('.ai-consenso');
  assert.equal(await p().locator('textarea[name="richiesta-ai"]').count(), 0);
  await p().check('input[name="consenso-letto"]');
  await p().click('button[name="accetta-consenso"]');
  await attendi();
  await p().waitForSelector('textarea[name="richiesta-ai"]');
  assert.equal(finto.ricevute.length, 0);
});

test('provider sovraccarico: nuovi tentativi automatici, poi la proposta', async () => {
  const prima = messaggi();
  await proponi('Tinteggiatura di una parete di 4 x 3 m (sovraccarico)');
  const r = p().locator('.ai-risultato');
  await r.waitFor();
  assert.match(await r.locator('.ai-lavorazione caption').innerText(), /Alternative proposte/i);
  // Due 529, poi analisi e selezione.
  assert.equal(messaggi() - prima, 4);
  assert.equal(await p().locator('.notifica.errore').count(), 0);
});

test('risposta del modello non valida: errore chiaro, la proposta precedente resta', async () => {
  await chiudiNotifiche();
  await proponi('Tinteggiatura di una parete di 4 x 3 m, risposta rotta');
  const n = await notifica('errore');
  assert.match(n, /risposta/i);
  assert.ok(!n.includes('sk-ant-prova'));
  // Torna visibile la proposta di prima, con le sue alternative.
  assert.match(await p().locator('.ai-risultato .ai-lavorazione caption').innerText(), /Alternative proposte/i);
  // La vista resta utilizzabile: richiesta ancora nel campo, pulsante attivo.
  assert.match(await p().inputValue('textarea[name="richiesta-ai"]'), /risposta rotta/);
  assert.ok(await p().locator('button[name="proponi"]').isEnabled());
  await chiudiNotifiche();
});

test('risposta lenta: «Annulla» ferma subito l\'attesa', async () => {
  await invia('Tinteggiatura di una parete di 4 x 3 m, risposta lenta');
  await p().waitForSelector('.ai-attesa');
  await p().waitForTimeout(400);
  const inizio = Date.now();
  await p().click('button[name="annulla-ai"]');
  assert.match(await notifica('info'), /Operazione annullata/);
  assert.ok(Date.now() - inizio < 2000, `annullamento in ${Date.now() - inizio} ms`);
  await p().waitForFunction(() => !document.querySelector('.ai-attesa'));
  await attendi();
  await chiudiNotifiche();
  // Subito dopo si può proporre di nuovo.
  await proponi('Tinteggiatura di una parete di 5 x 3 m');
  assert.match(await p().locator('.ai-risultato').innerText(), /15,00/);
});

test('provider irraggiungibile: tentativi annullabili, ricerca per parole e computo funzionanti', async () => {
  finto.chiudi();
  await invia('Tinteggiatura di una parete di 4 x 3 m');
  await p().waitForSelector('.ai-attesa');
  await p().waitForTimeout(600);
  await p().click('button[name="annulla-ai"]');
  assert.match(await notifica('info'), /Operazione annullata/);
  await p().waitForFunction(() => !document.querySelector('.ai-attesa'));
  await attendi();
  // La proposta di prima è ancora lì e si può aggiungere al computo senza rete.
  await p().click('button[name="aggiungi-scelte"]');
  await attendi();
  const c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci.length, 1);
  assert.equal(c.voci[0].quantita, '15.00');
  const r = (await b.motore({ richiesta: 'assistente_cerca', prezzario: 'prezzario_prova', testo: 'idropittura pareti' })).risultato;
  assert.equal(r.ricerca, 'testuale');
  assert.ok(r.voci.length > 0);
});

test('nessun errore JavaScript né violazione della Content Security Policy', () => {
  assert.deepEqual(b.errori, []);
});
