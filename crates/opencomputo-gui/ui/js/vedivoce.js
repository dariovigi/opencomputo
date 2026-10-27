// «Vedi voce» (ADR 0019): scelta di una o più voci di computo da citare, ognuna in un
// rigo di misura. Nel computo il rigo resta collegato alla voce citata; nel libretto
// delle misure la quantità viene copiata (statica).

import { h, modale, numero, rimanda } from './dom.js';
import { griglia } from './griglia.js';

const COLONNE = (decimali) => [
  { titolo: 'N.', larghezza: '48px', allinea: 'destra', valore: (v) => String(v.numero) },
  { titolo: 'Tariffa', larghezza: '120px', valore: (v) => v.codice, classe: () => 'mono' },
  { titolo: 'Descrizione', larghezza: 'minmax(200px, 1fr)', valore: (v) => v.descrizione_breve, suggerimento: true },
  { titolo: 'Quantità', larghezza: '120px', allinea: 'destra', valore: (v) => (v.quantita === null ? 'errore' : `${numero(v.quantita, decimali)} ${v.unita_misura}`) },
];

/**
 * Voci che citano (anche indirettamente) la voce `numero`: citarle da quella voce
 * creerebbe un riferimento circolare.
 */
export function vociCheCitano(voci, numero) {
  const citataDa = new Map(); // numero → voci che lo citano
  for (const v of voci) {
    for (const r of v.righi) {
      const n = r.vedi_voce && !r.vedi_voce.statica ? r.vedi_voce.numero : null;
      if (!n) continue;
      if (!citataDa.has(n)) citataDa.set(n, new Set());
      citataDa.get(n).add(v.numero);
    }
  }
  const trovate = new Set();
  const da = [numero];
  while (da.length) {
    for (const k of citataDa.get(da.pop()) ?? []) {
      if (!trovate.has(k)) { trovate.add(k); da.push(k); }
    }
  }
  return trovate;
}

/**
 * Modale di scelta. opzioni: {
 *   titolo, decimali,
 *   computi: [nomi] (scelta del computo, nel libretto), computo: nome iniziale,
 *   voci: async (nome computo) => voci della vista del computo,
 *   escluse: (voci, nome computo) => Set dei numeri da non proporre,
 *   nota: testo sotto la lista,
 * }
 * Restituisce {computo, voci: [voci scelte nell'ordine del computo], deduzione} o null.
 */
export async function scegliVociDaCitare(opzioni) {
  const { titolo = 'Vedi voce', decimali = 2, computi = [] } = opzioni;
  let computo = opzioni.computo;
  let tutte = [];
  let visibili = [];
  const g = griglia({
    colonne: COLONNE(decimali),
    multipla: true,
    etichetta: 'Voci di computo da citare',
    vuoto: 'Nessuna voce da citare',
    suAttiva: () => conferma(),
  });
  const cerca = h('input', { type: 'search', placeholder: 'Cerca numero, tariffa o descrizione…', 'aria-label': 'Cerca' });
  const conteggio = h('small', { class: 'vuoto' });
  const deduzione = h('input', { type: 'checkbox', name: 'deduzione' });
  const sel = computi.length > 1
    ? h('select', { 'aria-label': 'Computo' }, computi.map((n) => h('option', { value: n, selected: n === computo }, n)))
    : null;
  const filtra = () => {
    const t = cerca.value.trim().toLowerCase();
    visibili = t ? tutte.filter((v) => `${v.numero} ${v.codice} ${v.descrizione_breve}`.toLowerCase().includes(t)) : tutte;
    g.imposta(visibili, (v) => v.numero);
    conteggio.textContent = `${visibili.length} voci — Maiusc o Ctrl/⌘ per sceglierne più d'una: un rigo per voce`;
  };
  const carica = async () => {
    const voci = await opzioni.voci(computo);
    const escluse = opzioni.escluse?.(voci, computo) ?? new Set();
    tutte = voci.filter((v) => !escluse.has(v.numero));
    filtra();
    if (tutte.length && !g.selezionate().length) g.seleziona(0);
  };
  cerca.addEventListener('input', rimanda(filtra, 120));
  cerca.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); g.fuoco(); if (!g.attiva()) g.seleziona(0); }
  });
  sel?.addEventListener('change', () => { computo = sel.value; carica(); });
  let conferma = () => {};
  const promessa = modale({
    titolo,
    larga: true,
    contenuto: h('div', { class: 'selettore' },
      h('div', { class: 'riga-filtri' }, cerca, sel),
      g.el, conteggio,
      opzioni.nota ? h('p', { class: 'vuoto' }, opzioni.nota) : null,
      h('label', { class: 'campo-riga' }, deduzione, ' Righi in deduzione')),
    pulsanti: [{ testo: 'Annulla', valore: null }, { testo: 'Inserisci', valore: 'ok', tipo: 'primario' }],
    prima: async () => g.selezionate().length > 0,
  });
  const pulsante = document.querySelector('.modale .modale-pulsanti .primario');
  conferma = () => pulsante?.click();
  await carica();
  cerca.focus();
  const r = await promessa;
  if (!r) return null;
  return { computo, voci: g.selezionate(), deduzione: deduzione.checked };
}

/** Comandi `aggiungi_rigo` per citare le voci scelte dalla posizione indicata (da 1). */
export function comandiVediVoce({ computo, voce, posizione, voci, deduzione }) {
  return voci.map((v, i) => ({
    comando: 'aggiungi_rigo', computo, voce, posizione: posizione + i,
    rigo: { descrizione: '', deduzione, vedi_voce: v.id ?? v.numero },
  }));
}

/** Etichetta breve per le notifiche. */
export function quanteVoci(n) {
  return n === 1 ? '1 rigo «vedi voce»' : `${n} righi «vedi voce»`;
}
