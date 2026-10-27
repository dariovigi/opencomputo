// Pannello dei filtri (ADR 0020), condiviso da computo ed elenco prezzi.
//
// Sezioni:
// * `scelte`: valori distinti (categorie, natura, unità di misura…), più voci in OR;
// * `albero`: nodi gerarchici (WBS, capitoli): un nodo comprende i suoi discendenti;
// * `testo`: tutte le parole contenute (tariffa, descrizione);
// * `intervallo`: da… a… su un numero decimale (prezzo, quantità, importo…).
// Le sezioni attive si combinano in AND. Accanto a ogni valore il numero di voci (e
// l'importo) che resterebbero scegliendolo, con gli altri filtri già applicati.
//
// I confronti numerici sono su decimali esatti (BigInt), mai in virgola mobile.

import { h, sostituisci, numero, sommaImporti, preferenza, salvaPreferenza, rimanda } from './dom.js';

/** Chiave dei valori «non assegnato». */
export const NESSUNO = '';

const SCALA = 6;

/** Decimale in stringa (punto o virgola) → BigInt scalato, o null. */
function scalato(testo) {
  if (testo === null || testo === undefined) return null;
  const t = String(testo).trim().replace(/\s/g, '').replace(',', '.');
  if (!/^-?\d*(\.\d*)?$/.test(t) || t === '' || t === '-' || t === '.') return null;
  const neg = t.startsWith('-');
  const [i, f = ''] = t.replace('-', '').split('.');
  const n = BigInt(i || '0') * 10n ** BigInt(SCALA) + BigInt((f + '0'.repeat(SCALA)).slice(0, SCALA) || '0');
  return neg ? -n : n;
}

/**
 * Crea il pannello. opzioni: {
 *   chiave: nome per le preferenze (filtri e sezioni chiuse conservati),
 *   importo: (riga) => importo in stringa (totali accanto ai valori), facoltativo,
 *   suCambio: () => void (i filtri sono cambiati),
 * }
 * Restituisce {el, imposta(righe, sezioni), filtra(righe), attivi(), azzera(),
 *   scelteUniche(), fuoco(), ripristinaFuoco(f)}.
 */
export function pannelloFiltri({ chiave, importo = null, suCambio }) {
  const salvato = preferenza(`filtri.${chiave}`, {}) || {};
  /** id → {scelte:Set} | {testo} | {da, a} */
  const stato = new Map(Object.entries(salvato).map(([id, s]) => [id, s.scelte ? { scelte: new Set(s.scelte) } : { ...s }]));
  const chiuse = new Set(preferenza(`filtri.${chiave}.chiuse`, []) || []);
  const aperti = new Map(); // id sezione albero → Set dei nodi espansi
  let righe = [];
  let sezioni = [];
  const dom = new Map(); // id → {el, lista, firma}

  const conteggio = h('span', { class: 'vuoto' });
  const azzera = h('button', { class: 'link', onclick: () => { azzeraTutto(); } }, 'Azzera');
  const corpo = h('div', { class: 'filtri-sezioni' });
  const el = h('section', { class: 'pannello-filtri', 'aria-label': 'Filtri' },
    h('div', { class: 'filtri-testata' }, h('strong', null, 'Filtri'), conteggio, h('span', { class: 'spinge' }), azzera),
    corpo);

  function salva() {
    const o = {};
    for (const [id, s] of stato) {
      if (s.scelte) { if (s.scelte.size) o[id] = { scelte: [...s.scelte] }; } else if ((s.testo ?? '').trim() || (s.da ?? '').trim() || (s.a ?? '').trim()) o[id] = s;
    }
    salvaPreferenza(`filtri.${chiave}`, o);
  }

  const cambiato = () => { salva(); aggiorna(); suCambio?.(); };
  const cambiatoRimandato = rimanda(cambiato, 220);

  function attiva(sez) {
    const s = stato.get(sez.id);
    if (!s) return false;
    if (sez.tipo === 'scelte' || sez.tipo === 'albero') return !!s.scelte?.size;
    if (sez.tipo === 'testo') return !!(s.testo ?? '').trim();
    return scalato(s.da) !== null || scalato(s.a) !== null;
  }

  /** Discendenti (compreso sé) di ogni nodo di un albero. */
  function discendenti(sez) {
    const figli = new Map();
    for (const n of sez.nodi) {
      const p = n.padre ?? NESSUNO;
      if (!figli.has(p)) figli.set(p, []);
      figli.get(p).push(n.codice);
    }
    const memo = new Map();
    const di = (c, visti = new Set()) => {
      if (memo.has(c)) return memo.get(c);
      const out = new Set([c]);
      if (!visti.has(c)) {
        visti.add(c);
        for (const f of figli.get(c) ?? []) for (const x of di(f, visti)) out.add(x);
      }
      memo.set(c, out);
      return out;
    };
    return { figli, di };
  }

  /** Predicato di una sezione attiva. */
  function predicato(sez) {
    const s = stato.get(sez.id);
    if (sez.tipo === 'scelte') {
      return (r) => { const v = valori(sez, r); return v.some((x) => s.scelte.has(x)); };
    }
    if (sez.tipo === 'albero') {
      const { di } = discendenti(sez);
      const ammessi = new Set();
      for (const c of s.scelte) for (const x of (c === NESSUNO ? [NESSUNO] : di(c))) ammessi.add(x);
      return (r) => ammessi.has(sez.valore(r) ?? NESSUNO);
    }
    if (sez.tipo === 'testo') {
      const parole = s.testo.toLowerCase().split(/\s+/).filter(Boolean);
      return (r) => { const t = String(sez.valore(r) ?? '').toLowerCase(); return parole.every((p) => t.includes(p)); };
    }
    const da = scalato(s.da);
    const a = scalato(s.a);
    return (r) => {
      const v = scalato(sez.valore(r));
      if (v === null) return false;
      return (da === null || v >= da) && (a === null || v <= a);
    };
  }

  function valori(sez, r) {
    const v = sez.valori(r);
    const arr = Array.isArray(v) ? v : [v];
    const pulito = arr.map((x) => x ?? NESSUNO);
    return pulito.length ? pulito : [NESSUNO];
  }

  function predicati(escludi = null) {
    return sezioni.filter((s) => s.id !== escludi && attiva(s)).map(predicato);
  }

  function filtra(rr) {
    const p = predicati();
    return p.length ? rr.filter((r) => p.every((f) => f(r))) : rr;
  }

  /** Conteggi e importi per valore, con gli altri filtri applicati. */
  function conteggi(sez) {
    const altri = predicati(sez.id);
    const n = new Map();
    const imp = new Map();
    const aggiungi = (k, r) => {
      n.set(k, (n.get(k) ?? 0) + 1);
      if (importo) { const v = importo(r); if (v !== null && v !== undefined) imp.set(k, sommaImporti([imp.get(k) ?? '0', v])); }
    };
    for (const r of righe) {
      if (!altri.every((f) => f(r))) continue;
      if (sez.tipo === 'scelte') for (const k of new Set(valori(sez, r))) aggiungi(k, r);
      else aggiungi(sez.valore(r) ?? NESSUNO, r);
    }
    if (sez.tipo === 'albero') {
      // Ogni nodo somma i suoi discendenti.
      const padri = new Map(sez.nodi.map((x) => [x.codice, x.padre ?? null]));
      const nTot = new Map();
      const impTot = new Map();
      for (const [k, c] of n) {
        let cur = k;
        const visti = new Set();
        while (cur !== null && cur !== undefined && !visti.has(cur)) {
          visti.add(cur);
          nTot.set(cur, (nTot.get(cur) ?? 0) + c);
          if (importo && imp.has(k)) impTot.set(cur, sommaImporti([impTot.get(cur) ?? '0', imp.get(k)]));
          cur = cur === NESSUNO ? null : padri.get(cur);
        }
      }
      return { n: nTot, imp: impTot };
    }
    return { n, imp };
  }

  function etichettaConteggio(c, k) {
    const n = c.n.get(k) ?? 0;
    const i = c.imp.get(k);
    return h('span', { class: 'filtri-conteggio' }, String(n), importo && i ? ` · ${numero(i, 2)}` : null);
  }

  function casella(sez, k, testo, c, livello = 0, extra = null) {
    const s = stato.get(sez.id) ?? { scelte: new Set() };
    const scelta = s.scelte?.has(k);
    const input = h('input', { type: 'checkbox', checked: !!scelta, dataset: { sezione: sez.id, chiave: k } });
    input.addEventListener('change', () => {
      const st = stato.get(sez.id) ?? { scelte: new Set() };
      if (!st.scelte) st.scelte = new Set();
      if (input.checked) st.scelte.add(k); else st.scelte.delete(k);
      stato.set(sez.id, st);
      cambiato();
    });
    const zero = !(c.n.get(k) ?? 0);
    return h('label', { class: ['filtri-voce', zero && 'zero', scelta && 'scelta'], style: livello ? { paddingLeft: `${4 + livello * 14}px` } : null, title: testo },
      extra, input, h('span', { class: 'filtri-etichetta' }, testo), etichettaConteggio(c, k));
  }

  function listaScelte(sez) {
    const c = conteggi(sez);
    const chiavi = new Set(sez.ordine ?? []);
    for (const r of righe) for (const k of valori(sez, r)) chiavi.add(k);
    const s = stato.get(sez.id);
    for (const k of s?.scelte ?? []) chiavi.add(k);
    let elenco = [...chiavi];
    if (!sez.ordine) elenco.sort((a, b) => String(sez.etichetta?.(a) ?? a).localeCompare(String(sez.etichetta?.(b) ?? b), 'it'));
    elenco = elenco.filter((k) => k !== NESSUNO).concat(chiavi.has(NESSUNO) ? [NESSUNO] : []);
    if (!elenco.length) return [h('p', { class: 'vuoto' }, 'Nessun valore')];
    return elenco.map((k) => casella(sez, k, k === NESSUNO ? (sez.vuoto ?? 'Non assegnato') : (sez.etichetta?.(k) ?? k), c));
  }

  function listaAlbero(sez) {
    const c = conteggi(sez);
    const { figli } = discendenti(sez);
    const descr = new Map(sez.nodi.map((n) => [n.codice, n.descrizione]));
    const noti = new Set(sez.nodi.map((n) => n.codice));
    // Alberi piccoli tutti aperti; quelli grandi (capitoli dei prezzari) chiusi, con
    // aperti solo i rami che portano ai nodi scelti.
    if (!aperti.has(sez.id)) aperti.set(sez.id, new Set(sez.nodi.length <= 40 ? sez.nodi.map((n) => n.codice) : []));
    const esp = aperti.get(sez.id);
    const padri = new Map(sez.nodi.map((n) => [n.codice, n.padre]));
    for (const k of stato.get(sez.id)?.scelte ?? []) {
      let cur = padri.get(k);
      const visti = new Set();
      while (cur && !visti.has(cur)) { visti.add(cur); esp.add(cur); cur = padri.get(cur); }
    }
    const out = [];
    const visti = new Set();
    const nodo = (k, livello) => {
      if (visti.has(k)) return;
      visti.add(k);
      const ff = figli.get(k) ?? [];
      const aperto = esp.has(k);
      const freccia = ff.length
        ? h('button', { type: 'button', class: 'filtri-freccia', 'aria-label': aperto ? 'Chiudi' : 'Apri', tabIndex: -1,
          onclick: (e) => { e.preventDefault(); if (aperto) esp.delete(k); else esp.add(k); sostituisci(dom.get(sez.id).lista, ...listaAlbero(sez)); } }, aperto ? '▾' : '▸')
        : h('span', { class: 'filtri-freccia' });
      out.push(casella(sez, k, descr.get(k) ? `${k} — ${descr.get(k)}` : k, c, livello, freccia));
      if (aperto) for (const f of ff) nodo(f, livello + 1);
    };
    // Radici: nodi senza padre o con padre sconosciuto (file incoerenti).
    for (const n of sez.nodi) if (!n.padre || !noti.has(n.padre)) nodo(n.codice, 0);
    if (righe.some((r) => (sez.valore(r) ?? NESSUNO) === NESSUNO) || stato.get(sez.id)?.scelte?.has(NESSUNO)) {
      out.push(casella(sez, NESSUNO, sez.vuoto ?? 'Non assegnato', c));
    }
    return out.length ? out : [h('p', { class: 'vuoto' }, 'Nessun valore')];
  }

  function campo(sez) {
    const s = stato.get(sez.id) ?? (sez.tipo === 'testo' ? { testo: '' } : { da: '', a: '' });
    stato.set(sez.id, s);
    if (sez.tipo === 'testo') {
      const i = h('input', { type: 'search', value: s.testo ?? '', placeholder: sez.segnaposto ?? 'Contiene…', 'aria-label': sez.titolo, dataset: { sezione: sez.id } });
      i.addEventListener('input', () => { s.testo = i.value; cambiatoRimandato(); });
      return [i];
    }
    const mk = (k, ph) => {
      const i = h('input', { type: 'text', inputMode: 'decimal', value: s[k] ?? '', placeholder: ph, 'aria-label': `${sez.titolo} ${ph}`, dataset: { sezione: sez.id, estremo: k } });
      i.addEventListener('input', () => {
        s[k] = i.value;
        i.classList.toggle('non-valido', i.value.trim() !== '' && scalato(i.value) === null);
        cambiatoRimandato();
      });
      return i;
    };
    return [h('div', { class: 'filtri-intervallo' }, mk('da', 'da'), h('span', null, '–'), mk('a', 'a'), sez.unita ? h('span', { class: 'vuoto' }, sez.unita) : null)];
  }

  function costruisci(sez) {
    const lista = h('div', { class: 'filtri-lista' });
    const titolo = h('summary', null, h('span', null, sez.titolo), h('span', { class: 'filtri-attivo' }));
    const el_ = h('details', { class: 'filtri-sezione', open: !chiuse.has(sez.id), dataset: { sezione: sez.id } }, titolo, lista);
    el_.addEventListener('toggle', () => {
      if (el_.open) chiuse.delete(sez.id); else chiuse.add(sez.id);
      salvaPreferenza(`filtri.${chiave}.chiuse`, [...chiuse]);
    });
    if (sez.tipo === 'testo' || sez.tipo === 'intervallo') sostituisci(lista, ...campo(sez));
    return { el: el_, lista, titolo };
  }

  /** Aggiorna liste, conteggi e indicatori (dopo un cambio o nuovi dati). */
  function aggiorna() {
    const fuoco = document.activeElement?.closest?.('.pannello-filtri') ? document.activeElement : null;
    const chiaveFuoco = fuoco?.dataset?.chiave;
    const sezFuoco = fuoco?.dataset?.sezione;
    let n = 0;
    for (const sez of sezioni) {
      const d = dom.get(sez.id);
      const on = attiva(sez);
      if (on) n++;
      d.el.classList.toggle('attiva', on);
      d.titolo.querySelector('.filtri-attivo').textContent = on ? '●' : '';
      if (sez.tipo === 'scelte') sostituisci(d.lista, ...listaScelte(sez));
      else if (sez.tipo === 'albero') sostituisci(d.lista, ...listaAlbero(sez));
    }
    conteggio.textContent = n ? `${n} attiv${n === 1 ? 'o' : 'i'}` : '';
    azzera.hidden = !n;
    if (chiaveFuoco !== undefined && sezFuoco) {
      el.querySelector(`input[data-sezione="${CSS.escape(sezFuoco)}"][data-chiave="${CSS.escape(chiaveFuoco)}"]`)?.focus();
    }
  }

  function azzeraTutto() {
    for (const [id, s] of stato) {
      if (s.scelte) s.scelte.clear(); else { s.testo = ''; s.da = ''; s.a = ''; }
      const sez = sezioni.find((x) => x.id === id);
      if (sez && (sez.tipo === 'testo' || sez.tipo === 'intervallo')) sostituisci(dom.get(id).lista, ...campo(sez));
    }
    cambiato();
  }

  return {
    el,
    /** Nuovi dati e definizione delle sezioni (le sezioni assenti spariscono). */
    imposta(nuove, defs) {
      righe = nuove;
      sezioni = defs;
      for (const sez of sezioni) {
        if (sez.tipo === 'scelte' || sez.tipo === 'albero') { if (!stato.has(sez.id)) stato.set(sez.id, { scelte: new Set() }); }
        if (!dom.has(sez.id)) dom.set(sez.id, costruisci(sez));
      }
      const ids = new Set(sezioni.map((s) => s.id));
      for (const id of [...dom.keys()]) if (!ids.has(id)) dom.delete(id);
      const voluti = sezioni.map((s) => dom.get(s.id).el);
      if (voluti.length !== corpo.children.length || voluti.some((x, i) => corpo.children[i] !== x)) sostituisci(corpo, ...voluti);
      aggiorna();
    },
    filtra,
    /** Numero di sezioni attive. */
    attivi: () => sezioni.filter(attiva).length,
    azzera: azzeraTutto,
    /** Sezioni con un solo valore scelto (diverso da «non assegnato»): id → chiave. */
    scelteUniche() {
      const out = {};
      for (const sez of sezioni) {
        const s = stato.get(sez.id);
        if ((sez.tipo === 'scelte' || sez.tipo === 'albero') && s?.scelte?.size === 1) {
          const [k] = s.scelte;
          if (k !== NESSUNO) out[sez.id] = k;
        }
      }
      return out;
    },
    /** Elemento con il fuoco dentro il pannello, con la selezione del testo. */
    fuoco() {
      const a = document.activeElement;
      if (!a || !el.contains(a)) return null;
      return { a, inizio: a.selectionStart ?? null, fine: a.selectionEnd ?? null };
    },
    ripristinaFuoco(f) {
      if (!f || !el.contains(f.a)) return;
      f.a.focus({ preventScroll: true });
      if (f.inizio !== null) { try { f.a.setSelectionRange(f.inizio, f.fine); } catch { /* campo senza selezione */ } }
    },
  };
}
