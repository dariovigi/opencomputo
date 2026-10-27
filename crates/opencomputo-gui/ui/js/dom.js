// Utilità per il DOM e la formattazione.

/** Errore di inserimento dell'utente (mostrato, non registrato in console). */
export class ErroreUtente extends Error {}
//
// Regola di sicurezza (ADR 0009): il testo che arriva dai dati entra nella pagina solo
// come nodo di testo o come `value`, mai come HTML. `h()` non accetta HTML.

/** Crea un elemento. `attr` accetta class, dataset, on<evento>, style (oggetto) e proprietà. */
export function h(tag, attr, ...figli) {
  const el = document.createElement(tag);
  if (attr) {
    for (const [k, v] of Object.entries(attr)) {
      if (v === undefined || v === null || v === false) continue;
      if (k === 'class') el.className = Array.isArray(v) ? v.filter(Boolean).join(' ') : v;
      else if (k === 'dataset') Object.assign(el.dataset, v);
      else if (k === 'style') Object.assign(el.style, v);
      else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
      else if (k in el && k !== 'list' && k !== 'form') el[k] = v;
      else el.setAttribute(k, v === true ? '' : String(v));
    }
  }
  aggiungi(el, figli);
  return el;
}

function aggiungi(el, figli) {
  for (const f of figli) {
    if (f === null || f === undefined || f === false) continue;
    if (Array.isArray(f)) aggiungi(el, f);
    else if (f instanceof Node) el.appendChild(f);
    else el.appendChild(document.createTextNode(String(f)));
  }
}

/** Sostituisce il contenuto di un elemento. */
export function sostituisci(el, ...figli) {
  el.replaceChildren();
  aggiungi(el, figli);
  return el;
}

/** Numero decimale (stringa con il punto) all'italiana: `14457.1` → `14.457,10`. */
export function numero(valore, decimali = 2) {
  if (valore === null || valore === undefined || valore === '') return '';
  let s = String(valore).trim();
  let segno = '';
  if (s.startsWith('-')) { segno = '-'; s = s.slice(1); }
  let [intera, frazione = ''] = s.split('.');
  while (frazione.length < decimali) frazione += '0';
  intera = intera.replace(/^0+(?=\d)/, '');
  const gruppi = [];
  for (let i = intera.length; i > 0; i -= 3) gruppi.unshift(intera.slice(Math.max(0, i - 3), i));
  const testo = gruppi.join('.') + (frazione ? ',' + frazione : '');
  return (segno && /[1-9]/.test(testo) ? '-' : '') + testo;
}

/** Somma esatta di decimali in forma di stringa (centesimi interi, senza virgola mobile). */
export function sommaImporti(valori, decimali = 2) {
  const scala = 10n ** BigInt(decimali);
  let tot = 0n;
  for (const v of valori) {
    if (v === null || v === undefined || v === '') continue;
    const s = String(v);
    const neg = s.startsWith('-');
    const [i, f = ''] = s.replace('-', '').split('.');
    const n = BigInt(i || '0') * scala + BigInt((f + '0'.repeat(decimali)).slice(0, decimali) || '0');
    tot += neg ? -n : n;
  }
  const neg = tot < 0n;
  if (neg) tot = -tot;
  const i = tot / scala;
  const f = String(tot % scala).padStart(decimali, '0');
  return (neg ? '-' : '') + i + (decimali ? '.' + f : '');
}

/** Importo con il simbolo dell'euro. */
export function euro(valore) {
  return valore === null || valore === undefined ? '—' : numero(valore, 2) + ' €';
}

/** Percentuale: `12.5` → `12,50 %`. */
export function percento(valore) {
  return valore === null || valore === undefined ? '' : numero(valore, 2) + ' %';
}

/** Data ISO → gg/mm/aaaa. */
export function data(iso) {
  if (!iso) return '';
  const [a, m, g] = iso.split('-');
  return `${g}/${m}/${a}`;
}

/** Data di oggi in formato ISO (ora locale). */
export function oggi() {
  const d = new Date();
  const z = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}`;
}

/**
 * Testo numerico inserito dall'utente → decimale con il punto, o `null` se vuoto.
 * Accetta `12,5`, `12.5`, `1.234,56`. Lancia un errore se non è un numero.
 */
export function decimale(testo) {
  let s = String(testo ?? '').trim().replace(/\s/g, '').replace('€', '').replace('%', '');
  if (s === '') return null;
  if (s.includes(',')) s = s.replace(/\./g, '').replace(',', '.');
  if (!/^-?\d+(\.\d+)?$/.test(s)) throw new ErroreUtente(`«${testo}» non è un numero`);
  return s;
}

/** Testo vuoto → null. */
export function opzionale(testo) {
  const t = String(testo ?? '').trim();
  return t === '' ? null : t;
}

/** Ritarda l'esecuzione finché le chiamate non si fermano. */
export function rimanda(fn, ms = 200) {
  let t = null;
  return (...a) => {
    clearTimeout(t);
    t = setTimeout(() => fn(...a), ms);
  };
}

// ---------------------------------------------------------------------------
// Notifiche

let contenitoreNotifiche = null;

/** Notifica temporanea. `tipo`: info, ok, avviso, errore. `azione`: {testo, fn}. */
export function notifica(testo, tipo = 'info', azione = null, durata = 4500) {
  if (!contenitoreNotifiche) {
    contenitoreNotifiche = h('div', { class: 'notifiche', role: 'status', 'aria-live': 'polite' });
    document.body.appendChild(contenitoreNotifiche);
  }
  const el = h('div', { class: ['notifica', tipo] },
    h('span', null, testo),
    azione && h('button', { class: 'link', onclick: () => { azione.fn(); chiudi(); } }, azione.testo),
    h('button', { class: 'chiudi', title: 'Chiudi', 'aria-label': 'Chiudi', onclick: () => chiudi() }, '×'));
  const chiudi = () => el.remove();
  contenitoreNotifiche.appendChild(el);
  if (durata) setTimeout(chiudi, tipo === 'errore' ? durata * 2 : durata);
  return el;
}

// ---------------------------------------------------------------------------
// Finestre modali

/**
 * Finestra modale. `pulsanti`: [{testo, valore, tipo: 'primario'|'pericolo'}].
 * Restituisce una promessa con il valore del pulsante premuto (null con Esc).
 * `prima(valore)` può rifiutare la chiusura restituendo false (anche asincrono).
 */
export function modale({ titolo, contenuto, pulsanti = [{ testo: 'OK', valore: true, tipo: 'primario' }], larga = false, prima = null }) {
  return new Promise((risolvi) => {
    const precedente = document.activeElement;
    const sfondo = h('div', { class: 'modale-sfondo' });
    const corpo = h('div', { class: 'modale-corpo' }, contenuto);
    const barra = h('div', { class: 'modale-pulsanti' });
    const finestra = h('div', { class: ['modale', larga && 'larga'], role: 'dialog', 'aria-modal': 'true', 'aria-label': titolo },
      h('div', { class: 'modale-titolo' }, titolo), corpo, barra);
    const chiudi = async (valore) => {
      if (prima && valore !== null && (await prima(valore)) === false) return;
      sfondo.remove();
      document.removeEventListener('keydown', tasti, true);
      precedente?.focus?.();
      risolvi(valore);
    };
    for (const p of pulsanti) {
      barra.appendChild(h('button', { class: ['pulsante', p.tipo], onclick: () => chiudi(p.valore) }, p.testo));
    }
    const tasti = (e) => {
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); chiudi(null); }
      else if (e.key === 'Enter' && e.target.tagName !== 'TEXTAREA' && e.target.tagName !== 'BUTTON') {
        const p = pulsanti.find((x) => x.tipo === 'primario');
        if (p) { e.preventDefault(); e.stopPropagation(); chiudi(p.valore); }
      }
    };
    document.addEventListener('keydown', tasti, true);
    sfondo.appendChild(finestra);
    document.body.appendChild(sfondo);
    const primo = corpo.querySelector('input:not([type=checkbox]), select, textarea') || barra.querySelector('.primario') || barra.lastChild;
    primo?.focus();
    primo?.select?.();
  });
}

/** Conferma sì/no. */
export async function conferma(titolo, testo, { ok = 'Conferma', pericolo = false } = {}) {
  const r = await modale({
    titolo,
    contenuto: h('p', null, testo),
    pulsanti: [{ testo: 'Annulla', valore: false }, { testo: ok, valore: true, tipo: pericolo ? 'pericolo' : 'primario' }],
  });
  return r === true;
}

/**
 * Modulo in una modale. `campi`: [{nome, etichetta, tipo, valore, opzioni, obbligatorio, aiuto}].
 * Restituisce l'oggetto dei valori (testi) o null se annullato.
 */
export async function chiediCampi(titolo, campi, { ok = 'OK', convalida = null } = {}) {
  const controlli = {};
  const errore = h('div', { class: 'errore-modulo', role: 'alert' });
  const modulo = h('div', { class: 'modulo' }, campi.map((c) => {
    controlli[c.nome] = controllo(c);
    return h('label', { class: ['campo', c.tipo === 'checkbox' && 'spunta'] },
      h('span', null, c.etichetta), controlli[c.nome], c.aiuto && h('small', null, c.aiuto));
  }), errore);
  const valori = () => Object.fromEntries(Object.entries(controlli).map(([k, el]) => [k, el.type === 'checkbox' ? el.checked : el.value]));
  const r = await modale({
    titolo,
    contenuto: modulo,
    pulsanti: [{ testo: 'Annulla', valore: false }, { testo: ok, valore: true, tipo: 'primario' }],
    prima: async () => {
      const v = valori();
      for (const c of campi) {
        if (c.obbligatorio && !String(v[c.nome]).trim()) { errore.textContent = `Indicare «${c.etichetta}».`; controlli[c.nome].focus(); return false; }
      }
      if (convalida) {
        const msg = await convalida(v);
        if (msg) { errore.textContent = msg; return false; }
      }
      return true;
    },
  });
  return r === true ? valori() : null;
}

/** Controllo di un campo di modulo. */
export function controllo(c) {
  if (c.tipo === 'select') {
    return h('select', { name: c.nome }, c.opzioni.map(([v, t]) => h('option', { value: v, selected: String(v) === String(c.valore ?? '') }, t)));
  }
  if (c.tipo === 'textarea') return h('textarea', { name: c.nome, rows: c.righe || 4, value: c.valore ?? '' });
  if (c.tipo === 'checkbox') return h('input', { type: 'checkbox', name: c.nome, checked: !!c.valore });
  return h('input', {
    type: c.tipo || 'text', name: c.nome, value: c.valore ?? '', placeholder: c.segnaposto || '',
    inputMode: c.numerico ? 'decimal' : undefined, autocomplete: 'off', spellcheck: false,
  });
}

/** Etichetta della natura di una voce. */
export function natura(n) {
  return n === 'oneri_sicurezza' ? 'Sicurezza' : 'Lavoro';
}

/** Preferenza dell'interfaccia conservata nel browser (facoltativa: può mancare). */
export function preferenza(chiave, predefinito) {
  try {
    const v = localStorage.getItem('opencomputo.' + chiave);
    return v === null ? predefinito : JSON.parse(v);
  } catch {
    return predefinito;
  }
}

/** Salva una preferenza dell'interfaccia (ignorando gli errori di archiviazione). */
export function salvaPreferenza(chiave, valore) {
  try { localStorage.setItem('opencomputo.' + chiave, JSON.stringify(valore)); } catch { /* facoltativo */ }
}
