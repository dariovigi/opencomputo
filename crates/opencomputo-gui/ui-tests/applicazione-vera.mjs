// Prova dell'applicazione vera (binario Tauri, finestra WebKitGTK) su Linux con
// `tauri-driver` e un display virtuale: non il banco in Chromium, ma il programma che
// l'utente avvia, con il comando `richiesta`, i lavori su thread separati e gli eventi
// di avanzamento. Provider finto locale, chiavi in memoria: nessuna chiamata reale.
//
// Uso (Linux, con tauri-driver, WebKitWebDriver e xvfb installati):
//   cargo build -p opencomputo-gui --features opencomputo-agent-api/banco-di-prova
//   npm run vera

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readdir, readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { RADICE } from './banco.mjs';
import { avviaProviderFinto } from './provider-finto.mjs';

const APP = process.env.OPENCOMPUTO_GUI || path.join(RADICE, 'target/debug/opencomputo-gui');
const CSV = path.join(RADICE, 'crates/opencomputo-assistente/tests/fixtures/prezzario_prova.csv');
const PORTA = 4444;

let finto;
let driver;
let sessione;
let config;
let cartella;

async function wd(metodo, percorso, corpo) {
  const r = await fetch(`http://127.0.0.1:${PORTA}${percorso}`, {
    method: metodo, headers: { 'content-type': 'application/json' }, body: corpo ? JSON.stringify(corpo) : undefined,
  });
  const j = await r.json();
  if (j.value?.error) throw new Error(`${j.value.error}: ${j.value.message}`);
  return j.value;
}

/** Esegue nella finestra una funzione asincrona (testo) e ne restituisce il risultato. */
async function nella(corpo, ...args) {
  const script = `const fine = arguments[arguments.length - 1];
    (async () => { ${corpo} })().then((v) => fine({ v: v ?? null }), (e) => fine({ e: String(e?.message ?? e) }));`;
  const r = await wd('POST', `/session/${sessione}/execute/async`, { script, args });
  if (r.e) throw new Error(r.e);
  return r.v;
}

const richiesta = (r) => nella('return await window.__TAURI__.core.invoke("richiesta", { richiesta: arguments[0] });', r);
const pausa = (ms) => new Promise((ok) => setTimeout(ok, ms));

async function finche(corpo, ...args) {
  const fine = Date.now() + 30000;
  for (;;) {
    const v = await nella(corpo, ...args);
    if (v) return v;
    if (Date.now() > fine) throw new Error(`tempo scaduto: ${corpo}`);
    await pausa(100);
  }
}

const clic = (selettore) => finche('const e = document.querySelector(arguments[0]); if (!e || e.disabled) return false; e.click(); return true;', selettore);
const scrivi = (selettore, testo) => finche(`const e = document.querySelector(arguments[0]); if (!e) return false;
  e.value = arguments[1]; e.dispatchEvent(new Event('input', { bubbles: true })); return true;`, selettore, testo);
const testo = (selettore) => nella('return document.querySelector(arguments[0])?.innerText ?? null;', selettore);

before(async () => {
  assert.ok(existsSync(APP), `manca ${APP}: cargo build -p opencomputo-gui --features opencomputo-agent-api/banco-di-prova`);
  finto = await avviaProviderFinto();
  config = await mkdtemp(path.join(tmpdir(), 'opencomputo-vera-config-'));
  cartella = await mkdtemp(path.join(tmpdir(), 'opencomputo-vera-'));
  driver = spawn('xvfb-run', ['-a', 'tauri-driver', '--port', String(PORTA)], {
    stdio: 'ignore', detached: true,
    env: { ...process.env, OPENCOMPUTO_CONFIG: config, OPENCOMPUTO_PROVIDER_DI_PROVA: finto.url, WEBKIT_DISABLE_COMPOSITING_MODE: '1' },
  });
  for (let i = 0; ; i += 1) {
    try { await wd('GET', '/status'); break; } catch (e) { if (i > 100) throw e; await pausa(100); }
  }
  const s = await wd('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'wry', 'tauri:options': { application: APP } } } });
  sessione = s.sessionId;
  await wd('POST', `/session/${sessione}/timeouts`, { script: 60000 });
  await finche('return !!window.__TAURI__ && document.readyState === "complete" && document.querySelectorAll("button").length > 0;');
});

after(async () => {
  try { if (sessione) await wd('DELETE', `/session/${sessione}`); } catch { /* finestra già chiusa */ }
  if (driver) { try { process.kill(-driver.pid); } catch { /* già terminato */ } }
  finto?.chiudi();
});

test('il motore dell\'applicazione usa il provider di prova e le chiavi in memoria', async () => {
  const st = await richiesta({ richiesta: 'assistente_stato' });
  assert.equal(st.ok, true);
  assert.match(st.risultato.chiavi.archivio, /^memoria/);
  assert.equal((await richiesta({ richiesta: 'nuovo', titolo: 'Applicazione vera', percorso: path.join(cartella, 'lavori.ocp') })).ok, true);
  assert.equal((await richiesta({ richiesta: 'carica_prezzario', nome: 'prezzario_prova', percorso: CSV })).ok, true);
  for (const [provider, chiave] of [['anthropic', 'sk-ant-prova'], ['openai', 'sk-openai-prova']]) {
    assert.equal((await richiesta({ richiesta: 'assistente_chiave', provider, chiave })).ok, true);
  }
  // La finestra rilegge lo stato del motore, come all'avvio con un progetto aperto.
  await wd('POST', `/session/${sessione}/refresh`, {});
  await finche('return !!window.__TAURI__ && !!document.querySelector("nav button");');
});

test('consenso e proposta dalla finestra vera, valori calcolati dal motore', async () => {
  await finche(`const b = [...document.querySelectorAll('nav button')].find((x) => x.textContent.includes('Assistente AI'));
    if (!b) return false; b.click(); return true;`);
  await finche('return !!document.querySelector(".ai-consenso");');
  assert.equal(finto.ricevute.length, 0);
  await clic('input[name="consenso-letto"]');
  await clic('button[name="accetta-consenso"]');
  await scrivi('textarea[name="richiesta-ai"]', 'Tinteggiatura di una parete di 4 x 3 m');
  await clic('button[name="proponi"]');
  await finche('return !!document.querySelector(".ai-risultato .ai-lavorazione");');
  const t = await testo('.ai-risultato');
  assert.match(t, /Alternative proposte/i);
  assert.match(t, /12,00/);
  assert.equal(finto.ricevute.filter((r) => r.percorso === '/v1/messages').length, 2);
});

test('risposta lenta: la finestra resta reattiva e «Annulla» ferma l\'attesa', async () => {
  await scrivi('textarea[name="richiesta-ai"]', 'Tinteggiatura di una parete di 4 x 3 m, risposta lenta');
  await clic('button[name="proponi"]');
  await finche('return !!document.querySelector(".ai-attesa");');
  await pausa(500);
  // Il lavoro gira fuori dal lucchetto del servizio: le altre richieste rispondono subito.
  const inizio = Date.now();
  assert.equal((await richiesta({ richiesta: 'info' })).ok, true);
  assert.equal((await richiesta({ richiesta: 'assistente_stato' })).ok, true);
  assert.ok(Date.now() - inizio < 3000, `richieste durante il lavoro: ${Date.now() - inizio} ms`);
  await clic('button[name="annulla-ai"]');
  await finche('return !document.querySelector(".ai-attesa");');
  assert.ok(Date.now() - inizio < 6000, `annullamento: ${Date.now() - inizio} ms`);
  assert.match(await finche('return document.querySelector(".notifica.info")?.innerText ?? null;'), /Operazione annullata/);
  // La proposta di prima è tornata visibile.
  assert.match(await testo('.ai-risultato'), /Alternative proposte/i);
});

test('indicizzazione con eventi di avanzamento veri, poi ricerca ibrida', async () => {
  const r = await nella(`const eventi = [];
    const smetti = await window.__TAURI__.event.listen('assistente-avanzamento', (e) => eventi.push(e.payload));
    const r = await window.__TAURI__.core.invoke('richiesta', { richiesta: { richiesta: 'assistente_indicizza', prezzario: 'prezzario_prova' } });
    await new Promise((ok) => setTimeout(ok, 200));
    smetti();
    return { r, eventi };`);
  assert.equal(r.r.ok, true, JSON.stringify(r.r));
  assert.equal(r.r.risultato.rapporto.calcolate, 26);
  assert.ok(r.eventi.length >= 1, 'nessun evento di avanzamento');
  const c = await richiesta({ richiesta: 'assistente_cerca', prezzario: 'prezzario_prova', testo: 'idropittura pareti' });
  assert.equal(c.risultato.ricerca, 'ibrida');
});

test('aggiunta al computo dalla finestra, salvataggio su file, nessuna chiave su disco', async () => {
  await clic('button[name="aggiungi-scelte"]');
  await finche(`const r = await window.__TAURI__.core.invoke('richiesta', { richiesta: { richiesta: 'computo' } });
    return r.ok && r.risultato.voci.length === 1;`);
  const c = (await richiesta({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci[0].quantita, '12.00');
  assert.equal((await richiesta({ richiesta: 'salva' })).ok, true);
  assert.ok(existsSync(path.join(cartella, 'lavori.ocp')));
  for (const d of [config, cartella]) {
    for (const f of await readdir(d, { recursive: true })) {
      const p = path.join(d, f);
      const b = await readFile(p).catch(() => null);
      if (b) assert.ok(!b.includes('sk-ant-prova') && !b.includes('sk-openai-prova'), `chiave in ${p}`);
    }
  }
});
