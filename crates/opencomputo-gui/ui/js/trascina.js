// Trascinamento di voci, anche tra finestre: dall'elenco prezzi o da un prezzario al
// computo, da un computo all'altro, da un prezzario all'elenco del progetto.
//
// I dati viaggiano nel trascinamento con un formato proprio (JSON); il testo semplice
// porta le tariffe, per chi rilascia in un altro programma. Se il sistema non passa il
// formato proprio tra finestre diverse, si usa la copia che la finestra d'origine ha
// lasciato al motore all'inizio del trascinamento (ripulita alla fine).
//
// Dati: {tipo: 'elenco', codici} | {tipo: 'prezzario', prezzario, codici} |
//       {tipo: 'computo', computo, voci: [identificativi], numeri, codici}, più `finestra`
//       d'origine (gli identificativi non cambiano se un'altra finestra riordina le voci).

import { h } from './dom.js';
import { finestra, trascinamento } from './piattaforma.js';

export const FORMATO_VOCI = 'application/x-opencomputo-voci';

let locale = null; // trascinamento partito da questa finestra

/** Inizia un trascinamento con i dati indicati. */
export function inizia(e, dati) {
  const completi = { ...dati, finestra };
  locale = completi;
  const dt = e.dataTransfer;
  dt.effectAllowed = 'copyMove';
  dt.setData(FORMATO_VOCI, JSON.stringify(completi));
  dt.setData('text/plain', (dati.codici || []).join('\n'));
  const n = (dati.codici || []).length;
  const etichetta = h('div', { class: 'trascina-etichetta' }, n === 1 ? dati.codici[0] : `${n} voci`);
  document.body.appendChild(etichetta);
  try { dt.setDragImage(etichetta, 12, 12); } catch { /* immagine predefinita */ }
  setTimeout(() => etichetta.remove(), 0);
  trascinamento('inizia', completi).catch(() => {});
}

/** Fine del trascinamento nella finestra d'origine. */
export function finisci() {
  locale = null;
  trascinamento('fine').catch(() => {});
}

/** `true` se il trascinamento può contenere voci di OpenComputo. */
export function porta(e) {
  const tipi = [...(e.dataTransfer?.types || [])];
  return tipi.includes(FORMATO_VOCI) || tipi.includes('text/plain') || !!locale;
}

/** Dati del trascinamento rilasciato, o null. */
export async function leggi(e) {
  const testo = e.dataTransfer?.getData(FORMATO_VOCI);
  if (testo) {
    try { return valida(JSON.parse(testo)); } catch { return null; }
  }
  if (locale) return valida(locale);
  try { return valida(await trascinamento('leggi')); } catch { return null; }
}

function valida(d) {
  if (!d || typeof d !== 'object' || !Array.isArray(d.codici)) return null;
  if (!['elenco', 'prezzario', 'computo'].includes(d.tipo)) return null;
  if (d.tipo === 'computo' && (!Array.isArray(d.voci) || !d.computo)) return null;
  if (d.tipo === 'prezzario' && !d.prezzario) return null;
  return d;
}

/**
 * Copia o spostamento per un rilascio di voci di computo: nello stesso computo si
 * sposta (Alt o Ctrl copia), tra computi diversi si copia (Maiusc sposta).
 */
export function sposta(e, stessoComputo) {
  if (stessoComputo) return !(e.altKey || e.ctrlKey);
  return e.shiftKey;
}
