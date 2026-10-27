// Banco di prova dell'interfaccia: la cartella `ui` servita in locale con la stessa
// Content Security Policy dell'applicazione, aperta in Chromium, con un `window.__TAURI__`
// simulato che inoltra ogni richiesta a un processo `opencomputo servizio` reale.
// I test vedono quindi gli stessi totali, errori e file dell'applicazione desktop.

import { createServer } from 'node:http';
import { readFile, mkdtemp } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';

const qui = path.dirname(fileURLToPath(import.meta.url));
export const RADICE = path.resolve(qui, '../../..');
const UI = path.resolve(qui, '../ui');
const MOTORE = process.env.OPENCOMPUTO_BIN || path.join(RADICE, 'target/debug', process.platform === 'win32' ? 'opencomputo.exe' : 'opencomputo');
// Chromium: variabile d'ambiente, poi quello preinstallato in questo ambiente di sviluppo,
// altrimenti quello scaricato da `npx playwright-core install chromium`.
const CHROMIUM = process.env.CHROMIUM_PATH
  || ['/opt/pw-browsers/chromium-1194/chrome-linux/chrome'].find((p) => existsSync(p));
// Come in tauri.conf.json, senza le sorgenti IPC di Tauri.
const CSP = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const TIPI = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.png': 'image/png' };

export const FIXTURE = path.join(RADICE, 'crates/opencomputo-core/tests/fixtures/progetto_esempio_v1.json');
export const XPWE = path.join(RADICE, 'crates/opencomputo-format-xpwe/tests/fixtures/computo_sintetico.xpwe');

function server() {
  const s = createServer(async (req, res) => {
    const url = new URL(req.url, 'http://x');
    const file = path.join(UI, url.pathname === '/' ? 'index.html' : decodeURIComponent(url.pathname));
    if (!file.startsWith(UI)) { res.writeHead(403).end(); return; }
    try {
      const corpo = await readFile(file);
      res.writeHead(200, { 'Content-Type': TIPI[path.extname(file)] || 'application/octet-stream', 'Content-Security-Policy': CSP });
      res.end(corpo);
    } catch {
      res.writeHead(404).end();
    }
  });
  return new Promise((ok) => s.listen(0, '127.0.0.1', () => ok(s)));
}

/**
 * Processo `opencomputo servizio` con richieste in fila (una risposta per riga). Le righe
 * di avanzamento dell'assistente (`{"avanzamento": …}`, senza `ok`) vanno a `suAvanzamento`.
 * La cartella di configurazione è sempre temporanea: i test non toccano quella dell'utente.
 */
function motore({ ambiente = {}, suAvanzamento = () => {} } = {}) {
  const p = spawn(MOTORE, ['servizio', '--avanzamento'], { stdio: ['pipe', 'pipe', 'inherit'], env: { ...process.env, ...ambiente } });
  const righe = createInterface({ input: p.stdout });
  const attese = [];
  righe.on('line', (l) => {
    if (l.startsWith('{"avanzamento"')) { suAvanzamento(JSON.parse(l)); return; }
    attese.shift()?.(l);
  });
  let coda = Promise.resolve();
  const invia = (json) => {
    const r = coda.then(() => new Promise((ok) => { attese.push(ok); p.stdin.write(json.replace(/\n/g, ' ') + '\n'); }));
    coda = r.catch(() => {});
    return r;
  };
  return { invia, chiudi: () => p.kill() };
}

const SIMULAZIONE = () => {
  window.__oc = { dialoghi: [], finestre: [], aperti: [], uscito: false, ascoltatori: {}, titoli: [] };
  const etichetta = new URLSearchParams(location.search).get('finestra') || 'principale';
  window.__ocEvento = (evento, dato) => { for (const fn of window.__oc.ascoltatori[evento] || []) fn({ payload: dato }); };
  window.__TAURI__ = {
    core: {
      invoke: async (comando, arg) => {
        switch (comando) {
          case 'richiesta':
            return JSON.parse(await window.__ocInvia(JSON.stringify(arg.richiesta), etichetta));
          case 'apri_finestra': window.__oc.finestre.push(arg); return 'vista-1';
          case 'trascinamento': return JSON.parse(await window.__ocTrascina(arg.azione, JSON.stringify(arg.dati ?? null)));
          case 'apri_file': window.__oc.aperti.push(arg.percorso); return null;
          case 'esci': window.__oc.uscito = true; return null;
          default: throw new Error(`comando sconosciuto ${comando}`);
        }
      },
    },
    dialog: {
      open: async () => window.__oc.dialoghi.shift() ?? null,
      save: async () => window.__oc.dialoghi.shift() ?? null,
    },
    event: {
      listen: async (evento, fn) => { (window.__oc.ascoltatori[evento] ||= []).push(fn); return () => {}; },
    },
    webviewWindow: {
      getCurrentWebviewWindow: () => ({ label: etichetta, setTitle: async (t) => { window.__oc.titoli.push(t); } }),
    },
  };
};

/** Apre il banco: {pagina, cartella (temporanea), errori (console e pagina), chiudi()}. */
export async function apriBanco({ larghezza = 1600, altezza = 900, provider = null } = {}) {
  const s = await server();
  const config = await mkdtemp(path.join(tmpdir(), 'opencomputo-config-'));
  const ambiente = { OPENCOMPUTO_CONFIG: config };
  if (provider) ambiente.OPENCOMPUTO_PROVIDER_DI_PROVA = provider;
  let avanzamento = () => {};
  const m = motore({ ambiente, suAvanzamento: (v) => avanzamento(v) });
  if (provider) {
    // Senza la feature `banco-di-prova` il motore userebbe il portachiavi vero e la rete.
    const st = JSON.parse(await m.invia('{"richiesta":"assistente_stato"}'));
    if (!st.ok || !st.risultato.chiavi.archivio.startsWith('memoria')) {
      m.chiudi();
      s.close();
      throw new Error('il motore non è compilato per il banco di prova: cargo build -p opencomputo-agent-api --features banco-di-prova');
    }
  }
  const browser = await chromium.launch({ executablePath: CHROMIUM || undefined, args: ['--lang=it-IT'] });
  const contesto = await browser.newContext({ viewport: { width: larghezza, height: altezza }, locale: 'it-IT' });
  const errori = [];
  contesto.on('page', (p) => {
    p.on('pageerror', (e) => errori.push(`pagina: ${e.message}`));
    p.on('console', (msg) => { if (msg.type() === 'error') errori.push(`console: ${msg.text()}`); });
  });
  // Come il lato Rust: dopo una richiesta riuscita che modifica il progetto, evento
  // `progetto-aggiornato` a tutte le finestre, con l'etichetta di quella d'origine.
  const MODIFICANO = new Set(['apri', 'nuovo', 'importa', 'salva', 'esegui', 'annulla', 'ripristina', 'carica_prezzario', 'chiudi', 'assistente_applica']);
  await contesto.exposeFunction('__ocInvia', async (json, origine) => {
    const risposta = await m.invia(json);
    const r = JSON.parse(risposta);
    if (r.ok && MODIFICANO.has(JSON.parse(json).richiesta)) {
      for (const p of contesto.pages()) p.evaluate((o) => window.__ocEvento?.('progetto-aggiornato', { origine: o }), origine).catch(() => {});
    }
    return risposta;
  });
  // Come il lato Rust: dati del trascinamento in corso, comuni a tutte le finestre.
  let trascinamento = null;
  await contesto.exposeFunction('__ocTrascina', async (azione, json) => {
    if (azione === 'inizia') trascinamento = JSON.parse(json);
    else if (azione === 'fine') trascinamento = null;
    return JSON.stringify(trascinamento);
  });
  // Come il lato Rust: avanzamento delle operazioni lunghe dell'assistente.
  avanzamento = (v) => {
    for (const p of contesto.pages()) p.evaluate((d) => window.__ocEvento?.('assistente-avanzamento', d), v).catch(() => {});
  };
  await contesto.addInitScript(SIMULAZIONE);
  const pagina = await contesto.newPage();
  const cartella = await mkdtemp(path.join(tmpdir(), 'opencomputo-gui-'));
  const indirizzo = `http://127.0.0.1:${s.address().port}/index.html`;
  await pagina.goto(indirizzo);
  return {
    pagina,
    cartella,
    config,
    errori,
    indirizzo,
    contesto,
    /** Apre una finestra secondaria sulla vista indicata (es. `elenco`). */
    async finestra(vista) {
      const p = await contesto.newPage();
      await p.goto(`${indirizzo}?finestra=vista-1#${vista}`);
      return p;
    },
    /**
     * Trascina dall'elemento `da` (nella pagina `sorgente`) al punto `a` della pagina
     * `destinazione`, con eventi di trascinamento HTML: tra due pagine i dati passano
     * come tra due finestre. `soloTesto` simula un sistema che non passa il formato
     * proprio tra finestre; `tasti`: {shiftKey, altKey, ctrlKey}; `sopra`: rilascio nella
     * metà superiore dell'elemento.
     */
    async trascina(sorgente, da, destinazione, a, { soloTesto = false, tasti = {}, sopra = false } = {}) {
      const dati = await sorgente.evaluate((sel) => {
        const el = document.querySelector(sel);
        const dt = new DataTransfer();
        el.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: dt }));
        return Object.fromEntries([...dt.types].map((t) => [t, dt.getData(t)]));
      }, da);
      await new Promise((ok) => setTimeout(ok, 50));
      await destinazione.evaluate(({ sel, dati, soloTesto, tasti, sopra }) => {
        const el = document.querySelector(sel);
        const r = el.getBoundingClientRect();
        const dt = new DataTransfer();
        for (const [t, v] of Object.entries(dati)) if (!soloTesto || t === 'text/plain') dt.setData(t, v);
        const opz = { bubbles: true, cancelable: true, dataTransfer: dt, clientX: r.left + 8, clientY: sopra ? r.top + 2 : r.bottom - 2, ...tasti };
        el.dispatchEvent(new DragEvent('dragenter', opz));
        el.dispatchEvent(new DragEvent('dragover', opz));
        el.dispatchEvent(new DragEvent('drop', opz));
      }, { sel: a, dati, soloTesto, tasti, sopra });
      await new Promise((ok) => setTimeout(ok, 50));
      await sorgente.evaluate((sel) => document.querySelector(sel)?.dispatchEvent(new DragEvent('dragend', { bubbles: true })) ?? document.dispatchEvent(new DragEvent('dragend')), da).catch(() => {});
      return dati;
    },
    /** Risposta del prossimo dialogo dei file. */
    dialogo: (percorso) => pagina.evaluate((p) => window.__oc.dialoghi.push(p), percorso),
    /** Stato della simulazione (finestre aperte, file aperti, uscita). */
    simulazione: () => pagina.evaluate(() => ({ ...window.__oc, ascoltatori: Object.keys(window.__oc.ascoltatori) })),
    /** Richiesta diretta al motore, per verificare lo stato dall'esterno dell'interfaccia. */
    motore: async (r) => JSON.parse(await m.invia(JSON.stringify(r))),
    async chiudi() {
      await browser.close();
      m.chiudi();
      s.close();
    },
  };
}
