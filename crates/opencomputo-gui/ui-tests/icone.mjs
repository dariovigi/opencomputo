// Genera tutte le icone dell'applicazione da `icons/opencomputo.svg`.
//
// Ogni dimensione è disegnata direttamente dall'SVG (non rimpicciolita da un'immagine
// grande), su fondo trasparente. Produce:
//   icons/32x32.png, 128x128.png, 128x128@2x.png (256), icon.png (1024)
//   icons/icon.ico   (16, 24, 32, 48, 64, 128, 256)          per Windows
//   icons/icon.icns  (16, 32, 128, 256, 512 e i doppi @2x)   per macOS
//   ui/icona.png     (128)                                   per l'interfaccia
//
// Uso: npm run icone   (in ui-tests; serve Chromium come per i test)

import { chromium } from 'playwright-core';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const GUI = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const svg = await readFile(path.join(GUI, 'icons/opencomputo.svg'), 'utf8');

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
const pagina = await browser.newPage({ deviceScaleFactor: 1 });
const png = new Map();
const rgba = new Map();
for (const lato of [16, 24, 32, 48, 64, 128, 256, 512, 1024]) {
  await pagina.setViewportSize({ width: lato, height: lato });
  await pagina.setContent(`<!doctype html><style>html,body{margin:0;background:transparent}img{display:block;width:${lato}px;height:${lato}px}</style><img src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}">`);
  await pagina.waitForFunction(() => document.images[0].complete && document.images[0].naturalWidth > 0);
  png.set(lato, await pagina.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: lato, height: lato } }));
  // Punti RGBA non compressi, per le immagini piccole del file ICO.
  if (lato < 256) {
    rgba.set(lato, Buffer.from(await pagina.evaluate((n) => {
      const c = document.createElement('canvas');
      c.width = n; c.height = n;
      const x = c.getContext('2d');
      x.drawImage(document.images[0], 0, 0, n, n);
      return btoa(String.fromCharCode(...x.getImageData(0, 0, n, n).data));
    }, lato), 'base64'));
  }
}
await browser.close();

const scrivi = (nome, dati) => writeFile(path.join(GUI, nome), dati);
await scrivi('icons/32x32.png', png.get(32));
await scrivi('icons/128x128.png', png.get(128));
await scrivi('icons/128x128@2x.png', png.get(256));
await scrivi('icons/icon.png', png.get(1024));
await scrivi('ui/icona.png', png.get(128));

// ICO: intestazione, una voce di 16 byte per immagine, poi le immagini. Quella da 256 è
// in PNG; le più piccole sono bitmap non compresse a 32 bit (BGRA dal basso verso l'alto,
// seguite dalla maschera a 1 bit), che ogni versione di Windows legge.
{
  const bitmap = (lato) => {
    const d = rgba.get(lato);
    const testa = Buffer.alloc(40);
    testa.writeUInt32LE(40, 0);
    testa.writeInt32LE(lato, 4);
    testa.writeInt32LE(lato * 2, 8);
    testa.writeUInt16LE(1, 12);
    testa.writeUInt16LE(32, 14);
    const punti = Buffer.alloc(lato * lato * 4);
    for (let y = 0; y < lato; y += 1) {
      for (let x = 0; x < lato; x += 1) {
        const da = ((lato - 1 - y) * lato + x) * 4;
        const a = (y * lato + x) * 4;
        punti[a] = d[da + 2]; punti[a + 1] = d[da + 1]; punti[a + 2] = d[da]; punti[a + 3] = d[da + 3];
      }
    }
    const maschera = Buffer.alloc(Math.ceil(lato / 32) * 4 * lato);
    return Buffer.concat([testa, punti, maschera]);
  };
  const lati = [16, 24, 32, 48, 64, 128, 256];
  const immagini = lati.map((l) => (l === 256 ? png.get(l) : bitmap(l)));
  const testa = Buffer.alloc(6 + 16 * lati.length);
  testa.writeUInt16LE(1, 2);
  testa.writeUInt16LE(lati.length, 4);
  let posizione = testa.length;
  lati.forEach((lato, i) => {
    const o = 6 + 16 * i;
    testa.writeUInt8(lato === 256 ? 0 : lato, o);
    testa.writeUInt8(lato === 256 ? 0 : lato, o + 1);
    testa.writeUInt16LE(1, o + 4);
    testa.writeUInt16LE(32, o + 6);
    testa.writeUInt32LE(immagini[i].length, o + 8);
    testa.writeUInt32LE(posizione, o + 12);
    posizione += immagini[i].length;
  });
  await scrivi('icons/icon.ico', Buffer.concat([testa, ...immagini]));
}

// ICNS: «icns», lunghezza totale, poi blocchi tipo + lunghezza + PNG.
{
  const tipi = [['icp4', 16], ['icp5', 32], ['ic07', 128], ['ic08', 256], ['ic09', 512], ['ic10', 1024],
    ['ic11', 32], ['ic12', 64], ['ic13', 256], ['ic14', 512]];
  const blocchi = tipi.map(([tipo, lato]) => {
    const t = Buffer.alloc(8);
    t.write(tipo, 0, 'ascii');
    t.writeUInt32BE(8 + png.get(lato).length, 4);
    return Buffer.concat([t, png.get(lato)]);
  });
  const corpo = Buffer.concat(blocchi);
  const testa = Buffer.alloc(8);
  testa.write('icns', 0, 'ascii');
  testa.writeUInt32BE(8 + corpo.length, 4);
  await scrivi('icons/icon.icns', Buffer.concat([testa, corpo]));
}
console.log('Icone generate in', path.join(GUI, 'icons'), 'e', path.join(GUI, 'ui/icona.png'));
