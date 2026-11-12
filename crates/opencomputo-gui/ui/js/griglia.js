// Griglia di sola lettura con righe virtuali: disegna solo le righe visibili, così anche
// un prezzario di decine di migliaia di voci scorre senza rallentamenti.

import { h } from './dom.js';
import * as trascina from './trascina.js';

const ALTEZZA = 28;

/**
 * Crea una griglia.
 * opzioni: {
 *   colonne: [{titolo, larghezza (css), allinea: 'destra'|'centro', valore: (riga) => testo, classe: (riga) => classe}],
 *   righe: [], multipla: bool,
 *   suSelezione: (indici:Set, riga) => void, suAttiva: (riga, indice) => void (doppio clic / Invio),
 *   vuoto: testo, etichetta: testo (accessibilità),
 *   trascina: (righe selezionate) => dati per `trascina.inizia` (righe trascinabili),
 *   rilascio: (evento) => void (la griglia accetta il rilascio di voci),
 *   spunte: {scelta: (riga) => bool, alterna: (righe, valore) => void, abilitata: (riga) => bool,
 *     etichetta: testo}: prima colonna con una casella per riga (clic o Spazio sulle righe
 *     selezionate), indipendente dalla selezione
 * }
 * Restituisce {el, imposta(righe), selezionate(), seleziona(indice), fuoco()}.
 */
export function griglia(opzioni) {
  const { colonne, multipla = false, suSelezione = () => {}, suAttiva = () => {}, vuoto = 'Nessun elemento', etichetta = '' } = opzioni;
  let righe = opzioni.righe || [];
  let selezione = new Set();
  let attiva = -1;
  let ancora = -1;

  const spunte = opzioni.spunte;
  const modello = [spunte && '32px', ...colonne.map((c) => (!c.larghezza || c.larghezza === '1fr' ? 'minmax(90px, 1fr)' : c.larghezza))].filter(Boolean).join(' ');
  const intestazione = h('div', { class: 'griglia-riga griglia-intestazione', role: 'row', style: { gridTemplateColumns: modello } },
    spunte && h('div', { class: 'cella centro', role: 'columnheader', 'aria-label': spunte.etichetta ?? 'Scelta' }, '✓'),
    colonne.map((c) => h('div', { class: ['cella', c.allinea], role: 'columnheader' }, c.titolo)));
  const spazio = h('div', { class: 'griglia-spazio' });
  const visibili = h('div', { class: 'griglia-visibili', role: 'rowgroup' });
  const messaggio = h('div', { class: 'griglia-vuota' });
  const corpo = h('div', { class: 'griglia-corpo', tabIndex: 0, role: 'grid', 'aria-label': etichetta, 'aria-multiselectable': multipla ? 'true' : 'false' },
    spazio, visibili, messaggio);
  const el = h('div', { class: 'griglia' }, intestazione, corpo);

  let richiesto = false;
  let minima = 0;
  function disegna() {
    richiesto = false;
    spazio.style.height = righe.length * ALTEZZA + 'px';
    // Finestra più stretta delle colonne: le righe prendono la larghezza
    // dell'intestazione (che non scende sotto il suo minimo) e si scorre in orizzontale.
    const larga = intestazione.offsetWidth > corpo.clientWidth + 20 ? intestazione.offsetWidth : 0;
    if (larga !== minima) {
      minima = larga;
      spazio.style.width = larga ? larga + 'px' : '';
      corpo.style.setProperty('--griglia-minima', larga + 'px');
    }
    messaggio.textContent = righe.length ? '' : vuoto;
    const alto = corpo.scrollTop;
    const altezza = corpo.clientHeight || 400;
    const da = Math.max(0, Math.floor(alto / ALTEZZA) - 5);
    const a = Math.min(righe.length, Math.ceil((alto + altezza) / ALTEZZA) + 5);
    const nodi = [];
    for (let i = da; i < a; i++) {
      const r = righe[i];
      const scelta = selezione.has(i);
      nodi.push(h('div', {
        class: ['griglia-riga', scelta && 'selezionata', i === attiva && 'attiva', opzioni.classeRiga?.(r)],
        role: 'row', 'aria-selected': scelta ? 'true' : 'false',
        draggable: opzioni.trascina ? 'true' : undefined,
        dataset: { indice: i },
        style: { top: i * ALTEZZA + 'px', gridTemplateColumns: modello },
      }, spunte && h('div', { class: 'cella centro', role: 'gridcell' },
        h('input', {
          type: 'checkbox', class: 'spunta', checked: spunte.scelta(r), disabled: spunte.abilitata ? !spunte.abilitata(r) : false,
          'aria-label': spunte.etichetta ?? 'Scelta', tabIndex: -1,
        })),
      colonne.map((c) => {
        const v = c.valore(r);
        return h('div', { class: ['cella', c.allinea, c.classe?.(r)], role: 'gridcell', title: c.suggerimento ? String(v ?? '') : undefined }, v);
      })));
    }
    visibili.replaceChildren(...nodi);
  }
  function ridisegna() {
    if (!richiesto) { richiesto = true; requestAnimationFrame(disegna); }
  }
  // L'intestazione segue lo scorrimento orizzontale delle righe: in una finestra stretta
  // le colonne si raggiungono scorrendo e i titoli restano allineati.
  let sinistra = 0;
  corpo.addEventListener('scroll', () => {
    if (corpo.scrollLeft !== sinistra) {
      sinistra = corpo.scrollLeft;
      intestazione.style.transform = sinistra ? `translateX(${-sinistra}px)` : '';
    }
    ridisegna();
  });
  new ResizeObserver(ridisegna).observe(corpo);

  function scegli(i, { estendi = false, aggiungiA = false } = {}) {
    if (i < 0 || i >= righe.length) return;
    if (multipla && estendi && ancora >= 0) {
      selezione = new Set();
      for (let k = Math.min(ancora, i); k <= Math.max(ancora, i); k++) selezione.add(k);
    } else if (multipla && aggiungiA) {
      selezione = new Set(selezione);
      if (selezione.has(i)) selezione.delete(i); else selezione.add(i);
      ancora = i;
    } else {
      selezione = new Set([i]);
      ancora = i;
    }
    attiva = i;
    mostra(i);
    disegna();
    suSelezione(selezione, righe[i], i);
  }

  function mostra(i) {
    const alto = i * ALTEZZA;
    if (alto < corpo.scrollTop) corpo.scrollTop = alto;
    else if (alto + ALTEZZA > corpo.scrollTop + corpo.clientHeight) corpo.scrollTop = alto + ALTEZZA - corpo.clientHeight;
  }

  // Clic su una riga già selezionata insieme ad altre: la selezione resta, così si può
  // trascinare tutta; se non parte un trascinamento, al rilascio resta solo quella riga.
  let inAttesa = -1;
  visibili.addEventListener('mousedown', (e) => {
    const riga = e.target.closest('.griglia-riga');
    if (!riga) return;
    if (spunte && e.target.matches('input.spunta')) return; // la casella non cambia la selezione
    const i = Number(riga.dataset.indice);
    const modificatori = e.shiftKey || e.ctrlKey || e.metaKey;
    if (opzioni.trascina && !modificatori && selezione.has(i) && selezione.size > 1) {
      inAttesa = i;
    } else if (!modificatori && attiva === i && selezione.size === 1 && selezione.has(i)) {
      // Già selezionata da sola: niente ridisegno, così il secondo clic di un doppio
      // clic resta sulla stessa riga e il doppio clic arriva alla griglia.
      inAttesa = -1;
    } else {
      inAttesa = -1;
      scegli(i, { estendi: e.shiftKey, aggiungiA: e.ctrlKey || e.metaKey });
    }
    corpo.focus({ preventScroll: true });
  });
  /** Alterna le caselle delle righe indicate (tutte al valore opposto della prima). */
  function alterna(indici) {
    const abilitate = indici.filter((i) => righe[i] && (!spunte.abilitata || spunte.abilitata(righe[i])));
    if (!abilitate.length) return;
    const valore = !spunte.scelta(righe[abilitate[0]]);
    spunte.alterna(abilitate.map((i) => righe[i]), valore);
    disegna();
  }
  visibili.addEventListener('click', (e) => {
    if (spunte && e.target.matches('input.spunta')) {
      e.preventDefault();
      alterna([Number(e.target.closest('.griglia-riga').dataset.indice)]);
      return;
    }
    const riga = e.target.closest('.griglia-riga');
    if (riga && Number(riga.dataset.indice) === inAttesa) scegli(inAttesa);
    inAttesa = -1;
  });
  // La riga premuta viene ridisegnata: il fuoco predefinito del clic finisce sul
  // documento, quindi torna alla griglia (per Spazio, frecce, Invio).
  visibili.addEventListener('mouseup', () => {
    if (!corpo.contains(document.activeElement)) corpo.focus({ preventScroll: true });
  });
  if (opzioni.trascina) {
    visibili.addEventListener('dragstart', (e) => {
      const riga = e.target.closest('.griglia-riga');
      if (!riga) return;
      inAttesa = -1;
      const i = Number(riga.dataset.indice);
      if (!selezione.has(i)) scegli(i);
      const dati = opzioni.trascina([...selezione].sort((a, b) => a - b).map((k) => righe[k]));
      if (!dati) { e.preventDefault(); return; }
      trascina.inizia(e, dati);
    });
    visibili.addEventListener('dragend', () => trascina.finisci());
  }
  if (opzioni.rilascio) {
    corpo.addEventListener('dragover', (e) => {
      if (!trascina.porta(e)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'copy';
      corpo.classList.add('rilascio-attivo');
    });
    corpo.addEventListener('dragleave', (e) => {
      if (!corpo.contains(e.relatedTarget)) corpo.classList.remove('rilascio-attivo');
    });
    corpo.addEventListener('drop', (e) => {
      e.preventDefault();
      corpo.classList.remove('rilascio-attivo');
      opzioni.rilascio(e);
    });
  }
  visibili.addEventListener('dblclick', (e) => {
    const riga = e.target.closest('.griglia-riga');
    if (riga) suAttiva(righe[Number(riga.dataset.indice)], Number(riga.dataset.indice));
  });
  corpo.addEventListener('keydown', (e) => {
    const pagina = Math.max(1, Math.floor(corpo.clientHeight / ALTEZZA) - 1);
    const passi = { ArrowDown: 1, ArrowUp: -1, PageDown: pagina, PageUp: -pagina };
    if (e.key in passi) {
      e.preventDefault();
      scegli(Math.max(0, Math.min(righe.length - 1, (attiva < 0 ? -1 : attiva) + passi[e.key])), { estendi: e.shiftKey });
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      scegli(e.key === 'Home' ? 0 : righe.length - 1, { estendi: e.shiftKey });
    } else if (spunte && e.key === ' ' && attiva >= 0) {
      e.preventDefault();
      alterna(selezione.size ? [...selezione].sort((a, b) => a - b) : [attiva]);
    } else if (e.key === 'Enter' && attiva >= 0) {
      e.preventDefault();
      suAttiva(righe[attiva], attiva);
    } else if (multipla && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      selezione = new Set(righe.map((_, i) => i));
      disegna();
      suSelezione(selezione, righe[attiva], attiva);
    }
  });

  return {
    el,
    /** Sostituisce le righe; con `chiave` conserva la selezione sulle righe equivalenti. */
    imposta(nuove, chiave = null) {
      let attivaChiave = null;
      if (chiave && attiva >= 0 && righe[attiva]) attivaChiave = chiave(righe[attiva]);
      const scelte = chiave ? new Set([...selezione].map((i) => righe[i] && chiave(righe[i]))) : new Set();
      righe = nuove;
      selezione = new Set();
      attiva = -1;
      if (chiave) {
        righe.forEach((r, i) => {
          const k = chiave(r);
          if (scelte.has(k)) selezione.add(i);
          if (k === attivaChiave) attiva = i;
        });
      }
      disegna();
    },
    selezionate: () => [...selezione].sort((a, b) => a - b).map((i) => righe[i]),
    attiva: () => (attiva >= 0 ? righe[attiva] : null),
    seleziona: (i) => scegli(i),
    fuoco: () => corpo.focus(),
    ridisegna: disegna,
  };
}
