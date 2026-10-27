// Pannello delle voci affiancato al computo (vista divisa, ADR 0018): a sinistra l'elenco
// prezzi del progetto o un prezzario di consultazione, con ricerca e selezione multipla;
// le voci scelte passano al computo con «>», con Invio o doppio clic, o trascinandole.
//
// Il pannello è uno solo per finestra e sopravvive ai ridisegni del computo: conserva
// fonte, ricerca, selezione e scorrimento.

import { h, numero, natura, rimanda, sostituisci, preferenza, salvaPreferenza } from './dom.js';
import { chiedi, stato, mostraErrore } from './stato.js';
import { griglia } from './griglia.js';
import { COLONNE_VOCE } from './selettore.js';

export const LARGHEZZA_MINIMA = 260;

/**
 * Crea il pannello.
 * opzioni: {
 *   suCopia: (voci) => void (voci scelte da aggiungere al computo),
 *   nelComputo: () => Set dei codici già usati nel computo (per evidenziarli),
 * }
 * Restituisce {el, aggiorna(), fuoco(), selezionate(), fonte()}.
 */
export function pannelloVoci({ suCopia, nelComputo }) {
  const locale = { fonte: null, testo: '', alto: 0, codice: null };
  let presenti = new Set(); // codici dell'elenco del progetto (per i prezzari)
  let usati = new Set();
  let richiesta = 0;

  const fonte = h('select', { 'aria-label': 'Voci da affiancare' });
  fonte.addEventListener('change', () => {
    locale.fonte = fonte.value || null;
    salvaPreferenza('affianca_fonte', locale.fonte);
    locale.alto = 0;
    locale.codice = null;
    carica();
  });
  const cerca = h('input', { type: 'search', placeholder: 'Cerca codice o parole…', 'aria-label': 'Cerca nelle voci affiancate' });
  cerca.addEventListener('input', rimanda(() => { locale.testo = cerca.value; locale.alto = 0; carica(); }, 180));
  cerca.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); g.fuoco(); if (!g.attiva()) g.seleziona(0); }
  });
  const conteggio = h('small', { class: 'vuoto' });
  const anteprima = h('div', { class: 'anteprima-voce', 'aria-live': 'polite' });

  const g = griglia({
    colonne: COLONNE_VOCE,
    multipla: true,
    etichetta: 'Voci da aggiungere al computo',
    vuoto: 'Nessuna voce',
    trascina: (voci) => (locale.fonte
      ? { tipo: 'prezzario', prezzario: locale.fonte, codici: voci.map((v) => v.codice) }
      : { tipo: 'elenco', codici: voci.map((v) => v.codice) }),
    classeRiga: (v) => [locale.fonte && presenti.has(v.codice) && 'presente', usati.has(v.codice) && 'nel-computo'].filter(Boolean).join(' ') || null,
    suSelezione: (_, v) => { locale.codice = v?.codice ?? null; mostraAnteprima(); },
    suAttiva: (v) => {
      const scelte = g.selezionate();
      suCopia(scelte.includes(v) ? scelte : [v]);
    },
  });
  const corpo = g.el.querySelector('.griglia-corpo');
  corpo.addEventListener('scroll', () => { if (corpo.isConnected) locale.alto = corpo.scrollTop; });

  const el = h('section', { class: 'pannello-voci', 'aria-label': 'Voci affiancate' },
    h('div', { class: 'pannello-voci-testata' }, fonte, cerca, conteggio),
    g.el, anteprima);

  function opzioniFonte() {
    const caricati = stato.info?.prezzari_caricati ?? [];
    if (locale.fonte === null) {
      const salvata = preferenza('affianca_fonte', null);
      if (salvata && caricati.some((p) => p.nome === salvata)) locale.fonte = salvata;
    }
    if (locale.fonte && !caricati.some((p) => p.nome === locale.fonte)) locale.fonte = null;
    const voci = [[null, 'Elenco prezzi del progetto'], ...caricati.map((p) => [p.nome, `Prezzario «${p.nome}» (${p.voci})`])];
    sostituisci(fonte, voci.map(([v, t]) => h('option', { value: v ?? '', selected: v === locale.fonte }, t)));
    fonte.value = locale.fonte ?? '';
    fonte.title = caricati.length ? '' : 'I prezzari di consultazione si caricano in «Elenco prezzi»';
  }

  async function carica() {
    const n = ++richiesta;
    try {
      const prezzario = locale.fonte;
      const [r, elenco] = await Promise.all([
        chiedi({ richiesta: 'elenco_prezzi', prezzario, testo: locale.testo }),
        prezzario ? chiedi({ richiesta: 'elenco_prezzi' }) : null,
      ]);
      if (n !== richiesta) return;
      presenti = new Set((elenco ?? r).voci.map((v) => v.codice));
      g.imposta(r.voci, (v) => v.codice);
      conteggio.textContent = locale.testo ? `${r.voci.length} di ${r.totale}` : `${r.totale} voci`;
      if (corpo.isConnected) corpo.scrollTop = locale.alto;
      mostraAnteprima();
    } catch (e) {
      if (n === richiesta) mostraErrore(e);
    }
  }

  async function mostraAnteprima() {
    const codice = locale.codice;
    if (!codice) {
      sostituisci(anteprima, h('span', { class: 'vuoto' }, 'Selezionare le voci (Maiusc o Ctrl per più voci) e aggiungerle al computo con «>», Invio o trascinandole.'));
      return;
    }
    try {
      const v = await chiedi({ richiesta: 'voce_prezzario', codice, prezzario: locale.fonte });
      if (codice !== locale.codice) return;
      sostituisci(anteprima,
        h('strong', { class: 'mono' }, v.codice), ' ',
        h('span', { class: 'etichetta neutra' }, `${numero(v.prezzo, 2)} € / ${v.unita_misura}`), ' ',
        v.natura !== 'lavoro' ? h('span', { class: 'etichetta neutra' }, natura(v.natura)) : null,
        usati.has(v.codice) ? h('span', { class: 'etichetta neutra' }, 'già nel computo') : null,
        h('div', null, v.descrizione_estesa || v.descrizione_breve));
    } catch { /* anteprima facoltativa */ }
  }

  return {
    el,
    /** Ricarica le voci (dopo un ridisegno del computo o una modifica). */
    async aggiorna() {
      usati = nelComputo();
      opzioniFonte();
      await carica();
    },
    fuoco: () => g.fuoco(),
    selezionate: () => g.selezionate(),
    fonte: () => locale.fonte,
  };
}

/**
 * Rende trascinabile il divisore tra pannello e computo: la larghezza del pannello
 * (in pixel) si conserva nelle preferenze.
 */
export function divisore(el, contenitore) {
  el.addEventListener('pointerdown', (e) => {
    if (e.target.closest('button') || e.button !== 0) return;
    e.preventDefault();
    el.setPointerCapture(e.pointerId);
    const sinistra = contenitore.getBoundingClientRect().left;
    const massima = () => Math.max(LARGHEZZA_MINIMA, contenitore.clientWidth - 420);
    const muovi = (m) => {
      const larghezza = Math.round(Math.min(massima(), Math.max(LARGHEZZA_MINIMA, m.clientX - sinistra)));
      contenitore.style.setProperty('--larghezza-voci', `${larghezza}px`);
    };
    const fine = () => {
      el.removeEventListener('pointermove', muovi);
      el.removeEventListener('pointerup', fine);
      el.removeEventListener('pointercancel', fine);
      const v = contenitore.style.getPropertyValue('--larghezza-voci');
      if (v) salvaPreferenza('affianca_larghezza', parseInt(v, 10));
    };
    el.addEventListener('pointermove', muovi);
    el.addEventListener('pointerup', fine);
    el.addEventListener('pointercancel', fine);
  });
}
