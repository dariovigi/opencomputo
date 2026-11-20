// Convertitore DCF esterno (ADR 0027) dall'interfaccia: configurazione al primo uso con
// l'avviso, conversione con il resoconto, errore del convertitore, rimozione.
// Il convertitore è finto: uno script Node che rispetta il contratto della CLI esterna
// (`convert <origine> <uscita> --report <resoconto>`) e copia una fixture del progetto.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { writeFile, chmod, readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { apriBanco, RADICE } from './banco.mjs';

let b;
const p = () => b.pagina;
const FIXTURE = path.join(RADICE, 'crates/opencomputo-core/tests/fixtures/progetto_esempio_v7.json');

const FINTO = `#!/usr/bin/env node
const fs = require('node:fs');
const [, , comando, origine, uscita, opzione, resoconto] = process.argv;
if (comando !== 'convert' || opzione !== '--report') { process.stderr.write('errore: argomenti\\n'); process.exit(2); }
if (origine.includes('errore')) { process.stderr.write('errore: formato non riconosciuto\\n'); process.exit(1); }
fs.copyFileSync(${JSON.stringify(FIXTURE)}, uscita);
fs.writeFileSync(resoconto, 'trasferito (3):\\n  + elenco prezzi: 6 voci\\n<b>non HTML</b>\\n');
process.stdout.write('convertito\\n');
`;

async function attendi() {
  await p().waitForFunction(() => !document.body.classList.contains('occupato'));
  await p().waitForTimeout(60);
}

async function menu(voce) {
  await p().click('button:has-text("File ▾")');
  await p().click(`[role=menuitem]:has-text("${voce}")`);
}

let programma;
before(async () => {
  b = await apriBanco();
  programma = path.join(b.cartella, 'convertitore finto.js');
  await writeFile(programma, FINTO);
  await chmod(programma, 0o755);
  for (const n of ['lavori.dcf', 'errore.dcf']) await writeFile(path.join(b.cartella, n), 'AAMVHFSS finto');
});
after(async () => { await b?.chiudi(); });

test('primo uso: avviso, scelta del programma, conversione con resoconto', async () => {
  // Due dialoghi: il programma, poi il documento.
  await b.dialogo(programma);
  await b.dialogo(path.join(b.cartella, 'lavori.dcf'));
  await menu('Importa DCF con convertitore esterno');
  const modale = p().locator('.modale', { hasText: 'Convertitore DCF esterno' });
  await modale.waitFor();
  assert.match(await modale.textContent(), /non ne verifica il funzionamento/);
  assert.match(await modale.textContent(), /Programma: nessuno/);
  await modale.locator('button', { hasText: 'Scegli il programma' }).click();
  const esito = p().locator('.modale', { hasText: 'Conversione completata' });
  await esito.waitFor();
  await attendi();
  const resoconto = await esito.locator('pre.resoconto').first().textContent();
  assert.match(resoconto, /trasferito \(3\)/);
  // Il testo del convertitore entra come testo, mai come HTML.
  assert.match(resoconto, /<b>non HTML<\/b>/);
  assert.equal(await esito.locator('pre.resoconto b').count(), 0);
  await esito.locator('button', { hasText: 'OK' }).click();
  assert.match(await p().locator('.titolo-progetto').textContent(), /Nuova scuola primaria/);
  // Configurazione salvata nella cartella di configurazione.
  const config = JSON.parse(await readFile(path.join(b.config, 'convertitore.json'), 'utf8'));
  assert.equal(config.programma, programma);
  assert.deepEqual(b.errori, []);
});

test('errore del convertitore: messaggio, progetto invariato', async () => {
  await b.dialogo(path.join(b.cartella, 'errore.dcf'));
  await menu('Importa DCF con convertitore esterno');
  // Già configurato: si passa subito alla scelta del documento.
  await p().waitForSelector('.notifica.errore:has-text("formato non riconosciuto")');
  // Il progetto aperto resta quello di prima, nel motore e nell'interfaccia.
  const info = await b.motore({ richiesta: 'info' });
  assert.equal(info.ok, true, JSON.stringify(info));
  assert.equal(info.risultato.titolo, 'Nuova scuola primaria');
  assert.match(await p().locator('.titolo-progetto').textContent(), /Nuova scuola primaria/);
});

test('il convertitore si può togliere', async () => {
  await menu('Convertitore DCF esterno…');
  const modale = p().locator('.modale', { hasText: 'Convertitore DCF esterno' });
  await modale.waitFor();
  assert.match(await modale.textContent(), /convertitore finto\.js/);
  await modale.locator('button', { hasText: 'Togli' }).click();
  await p().waitForSelector('.notifica:has-text("Convertitore tolto")');
  const st = await b.motore({ richiesta: 'convertitore_stato' });
  assert.equal(st.risultato.programma, null);
  assert.ok(existsSync(path.join(b.config, 'convertitore.json')));
  assert.deepEqual(b.errori, []);
});
