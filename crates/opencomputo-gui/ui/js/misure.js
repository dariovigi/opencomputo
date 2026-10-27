// Foglio delle misure: voci (o registrazioni) con i righi di misura modificabili come in
// un foglio di calcolo. Usato dal computo e dal libretto delle misure.
//
// Tastiera: frecce per spostarsi, Invio o F2 per modificare (Invio conferma e scende),
// Tab conferma e va a destra, Esc annulla, Canc svuota la cella, Ins o Ctrl+Invio
// aggiunge un rigo sotto. Sotto ogni voce c'è una riga vuota per scrivere un nuovo rigo.
// Maiusc+frecce seleziona più righi; Ctrl+C li copia (anche per Excel, separati da
// tabulazioni), Ctrl+V incolla righi copiati qui o da un foglio di calcolo sotto il rigo
// attivo, a partire dalla colonna attiva.
//
// Voci: clic sulla testata seleziona la voce, Ctrl (⌘) o Maiusc+clic ne seleziona più
// d'una. Con `trascinaVoci` le testate si trascinano (anche in un'altra finestra); con
// `rilascio` il foglio accetta voci rilasciate tra una voce e l'altra.
//
// Righi «vedi voce»: il riferimento compare prima della descrizione; con `suVediVoce`
// un clic porta alla voce citata.

import { h, numero } from './dom.js';
import * as trascina from './trascina.js';

/** Campi modificabili di un rigo, nell'ordine delle colonne. */
export const CAMPI = ['descrizione', 'parti_uguali', 'lunghezza', 'larghezza', 'altezza_peso', 'formula'];
const FATTORI = new Set(['parti_uguali', 'lunghezza', 'larghezza', 'altezza_peso', 'formula']);

/** Formato degli appunti per copiare righi tra finestre di OpenComputo senza perdite. */
const FORMATO_RIGHI = 'application/x-opencomputo-righi';

function cellaTsv(v) {
  const t = String(v ?? '');
  return /[\t\n"]/.test(t) ? `"${t.replace(/"/g, '""')}"` : t;
}

/** Testo separato da tabulazioni (come lo copiano Excel e LibreOffice) → righe di celle. */
export function leggiTsv(testo) {
  const righe = [];
  let riga = [];
  let cella = '';
  let virgolette = false;
  for (let i = 0; i < testo.length; i++) {
    const ch = testo[i];
    if (virgolette) {
      if (ch === '"' && testo[i + 1] === '"') { cella += '"'; i++; } else if (ch === '"') virgolette = false; else cella += ch;
    } else if (ch === '"' && cella === '') virgolette = true;
    else if (ch === '\t') { riga.push(cella); cella = ''; }
    else if (ch === '\n' || ch === '\r') {
      if (ch === '\r' && testo[i + 1] === '\n') i++;
      riga.push(cella); righe.push(riga); riga = []; cella = '';
    } else cella += ch;
  }
  if (cella !== '' || riga.length) { riga.push(cella); righe.push(riga); }
  return righe.filter((r) => r.some((c) => c.trim() !== ''));
}

/** Numero all'italiana con i separatori delle migliaia (`1.234,5`) → senza (`1234,5`). */
function numeroIncollato(t) {
  const s = t.trim();
  return /^-?\d{1,3}(\.\d{3})+(,\d+)?$/.test(s) ? s.replace(/\./g, '') : s;
}

/** Righi di misura da righe di celle, a partire dalla colonna `c` (0 = descrizione). */
export function righiDaCelle(righe, c = 0) {
  return righe.map((celle) => {
    const modifiche = {};
    celle.forEach((v, j) => {
      const campo = CAMPI[c + j];
      if (campo) modifiche[campo] = campo === 'descrizione' ? v.trim() : numeroIncollato(v);
    });
    return rigoIngresso(null, modifiche);
  });
}

/** Rigo di misura per un comando, dai valori della vista con le modifiche indicate. */
export function rigoIngresso(vista, modifiche = {}) {
  const base = vista || { descrizione: '', commento: false, deduzione: false };
  const r = {};
  for (const c of CAMPI) {
    const v = c in modifiche ? modifiche[c] : base[c];
    if (c === 'descrizione') r.descrizione = v ?? '';
    else if (v !== null && v !== undefined && String(v).trim() !== '') r[c] = String(v).trim();
  }
  r.commento = 'commento' in modifiche ? modifiche.commento : !!base.commento;
  r.deduzione = 'deduzione' in modifiche ? modifiche.deduzione : !!base.deduzione;
  const vv = 'vedi_voce' in modifiche ? modifiche.vedi_voce : base.vedi_voce;
  if (vv !== null && vv !== undefined) r.vedi_voce = vediVoceIngresso(vv);
  if (r.commento) { for (const c of FATTORI) delete r[c]; delete r.vedi_voce; }
  return r;
}

/**
 * «Vedi voce» per un comando: dalla vista (collegato → identificativo della voce;
 * statico → quantità e riferimento) o già in forma di comando.
 */
function vediVoceIngresso(vv) {
  if (typeof vv !== 'object' || !('statica' in vv)) return vv;
  if (vv.statica) return { quantita: String(vv.quantita ?? '0'), riferimento: vv.riferimento ?? '' };
  return vv.id;
}

/** Riferimento di un rigo «vedi voce», cliccabile se la voce citata è nel foglio. */
function etichettaVediVoce(vv, decimali, vai) {
  const q = vv.quantita === null || vv.quantita === undefined ? ''
    : ` [${vv.unita_misura ? `${vv.unita_misura} ` : ''}${numero(vv.quantita, decimali)}]`;
  const titolo = vv.statica
    ? 'Quantità copiata: non segue più la voce citata'
    : vv.numero ? `${vv.codice} — ${vv.descrizione}\nClic per andare alla voce n. ${vv.numero}` : 'La voce citata non esiste più';
  return h('button', {
    type: 'button', class: ['vedi-voce', vv.statica && 'statica'], tabIndex: -1, title: titolo,
    onclick: vai && vv.numero ? (e) => { e.stopPropagation(); vai(vv); } : undefined,
  }, vv.testo + q);
}

/**
 * opzioni: {
 *   blocchi: [{id, numero, codice, titolo, um, quantita, prezzo, importo, righi, bloccato, errore, avviso, etichetta}],
 *   prezzi: bool (colonne prezzo e importo), decimali: decimali delle quantità,
 *   suModifica(blocco, numero, rigo), suAggiungi(blocco, posizione|null, rigo), suSeleziona(blocco, rigo|null),
 *   attiva: {blocco, rigo, c} (cella da attivare), vuoto: testo,
 *   scelti: [id] (voci selezionate), suSelezioneVoci(blocchi),
 *   trascinaVoci(blocchi) => dati per `trascina.inizia`,
 *   rilascio(evento, posizione da 1) (voci rilasciate nel foglio)
 * }
 */
export function foglioMisure(opzioni) {
  const { blocchi, prezzi = true, decimali = 2 } = opzioni;
  let attiva = null; // td
  let editor = null; // {td, input, originale}
  let occupato = false;
  let ancora = null; // rigo da cui parte la selezione con Maiusc
  let scelti = new Set((opzioni.scelti ?? blocchi.filter((b) => b.selezionato).map((b) => b.id)).map(String));
  let ancoraVoce = null; // voce da cui parte la selezione di voci con Maiusc
  let inAttesa = null; // voce da selezionare da sola al clic, se non parte un trascinamento

  const intestazione = h('thead', null, h('tr', null,
    h('th', { class: 'c-num' }, 'N.'),
    h('th', { class: 'c-codice' }, 'Tariffa'),
    h('th', { class: 'c-descr' }, 'Designazione dei lavori'),
    h('th', { class: 'c-fatt' }, 'Par. ug.'),
    h('th', { class: 'c-fatt' }, 'Lung.'),
    h('th', { class: 'c-fatt' }, 'Larg.'),
    h('th', { class: 'c-fatt' }, 'H/peso'),
    h('th', { class: 'c-formula' }, 'Formula'),
    h('th', { class: 'c-q' }, 'Quantità'),
    prezzi && h('th', { class: 'c-prezzo' }, 'Prezzo'),
    prezzi && h('th', { class: 'c-importo' }, 'Importo')));

  const corpo = h('tbody');
  let r = 0;
  for (const b of blocchi) {
    corpo.appendChild(h('tr', {
      class: ['testata', b.bloccato && 'bloccata', b.errore && 'con-errore', b.avviso && 'con-avviso', scelti.has(String(b.id)) && 'scelta'],
      dataset: { blocco: b.id },
      draggable: opzioni.trascinaVoci && !b.bloccato ? 'true' : undefined,
      title: opzioni.trascinaVoci && !b.bloccato ? 'Trascinare per spostare o copiare la voce (anche in un\'altra finestra)' : undefined,
    },
    h('td', { class: 'c-num' }, String(b.numero)),
    h('td', { class: 'c-codice', title: b.codice }, b.codice),
    h('td', { class: 'c-descr', colSpan: 6 },
      h('span', { class: 'titolo-voce' }, b.titolo || '(senza descrizione)'),
      b.etichetta && h('span', { class: ['etichetta', b.etichetta.tipo] }, b.etichetta.testo)),
    h('td', { class: ['c-q', 'num'], title: b.errore || undefined }, b.errore ? 'errore' : `${numero(b.quantita, decimali)} ${b.um}`),
    prezzi && h('td', { class: ['c-prezzo', 'num'] }, numero(b.prezzo, 2)),
    prezzi && h('td', { class: ['c-importo', 'num'] }, b.errore ? '' : numero(b.importo, 2))));

    const righi = [...b.righi, null];
    for (const rigo of righi) {
      const nuovo = rigo === null;
      if (nuovo && b.bloccato) continue;
      const riga = h('tr', {
        class: ['rigo', nuovo && 'nuovo', rigo?.commento && 'commento', rigo?.deduzione && 'deduzione', rigo?.errore && 'con-errore'],
        dataset: { blocco: b.id, rigo: nuovo ? 'nuovo' : rigo.numero },
      });
      riga.appendChild(h('td', { class: 'c-num' }));
      riga.appendChild(h('td', { class: ['c-codice', 'piccolo'] }, nuovo ? '' : String(rigo.numero)));
      CAMPI.forEach((campo, c) => {
        const testo = nuovo ? '' : rigo[campo] ?? '';
        const modificabile = !b.bloccato && !(rigo?.commento && FATTORI.has(campo));
        const td = h('td', {
          class: [campo === 'descrizione' ? 'c-descr' : campo === 'formula' ? 'c-formula' : 'c-fatt', modificabile && 'ed', campo !== 'descrizione' && 'num'],
          dataset: modificabile ? { r, c, campo } : { campo },
          title: campo !== 'descrizione' && testo ? testo : undefined,
        }, nuovo && c === 0 ? h('span', { class: 'segnaposto' }, '+ nuovo rigo') : contenuto(rigo, campo, testo));
        riga.appendChild(td);
      });
      const q = nuovo || rigo.commento ? '' : rigo.errore ? 'errore' : numero(rigo.quantita, decimali);
      riga.appendChild(h('td', { class: ['c-q', 'num'], title: rigo?.errore || undefined }, q));
      if (prezzi) riga.appendChild(h('td', { colSpan: 2 }));
      corpo.appendChild(riga);
      if (!b.bloccato) r++;
    }
  }

  const tabella = h('table', { class: 'misure' }, h('colgroup', null,
    h('col', { class: 'c-num' }), h('col', { class: 'c-codice' }), h('col', { class: 'c-descr' }),
    h('col', { class: 'c-fatt' }), h('col', { class: 'c-fatt' }), h('col', { class: 'c-fatt' }), h('col', { class: 'c-fatt' }),
    h('col', { class: 'c-formula' }), h('col', { class: 'c-q' }), prezzi && h('col', { class: 'c-prezzo' }), prezzi && h('col', { class: 'c-importo' })),
  intestazione, corpo);
  const el = h('div', { class: 'foglio', tabIndex: 0, role: 'grid', 'aria-label': opzioni.etichetta || 'Misure' }, tabella,
    blocchi.length ? null : h('div', { class: 'foglio-vuoto' }, opzioni.vuoto || 'Nessuna voce.'));

  /** Contenuto di una cella: il testo, preceduto dal «vedi voce» nella descrizione. */
  function contenuto(rigo, campo, testo) {
    if (campo !== 'descrizione' || !rigo?.vedi_voce) return testo;
    return [etichettaVediVoce(rigo.vedi_voce, decimali, opzioni.suVediVoce), testo ? ` ${testo}` : null];
  }

  const bloccoDi = (id) => blocchi.find((b) => String(b.id) === String(id));
  const rigoDi = (tr) => {
    const b = bloccoDi(tr.dataset.blocco);
    const n = tr.dataset.rigo;
    return { blocco: b, rigo: n === 'nuovo' || n === undefined ? null : b.righi[Number(n) - 1] };
  };

  function attiva_(td, { scorri = true, mantieni = false } = {}) {
    if (!td) return;
    attiva?.classList.remove('attiva');
    attiva = td;
    td.classList.add('attiva');
    if (scorri) td.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const tr = td.closest('tr');
    const { blocco, rigo } = rigoDi(tr);
    if (!(mantieni && scelti.has(String(blocco.id)))) scegliVoci([blocco.id]);
    opzioni.suSeleziona?.(blocco, rigo);
  }

  /** Seleziona le voci indicate (id) e aggiorna le testate. */
  function scegliVoci(ids) {
    scelti = new Set(ids.map(String));
    corpo.querySelectorAll('tr.testata').forEach((x) => x.classList.toggle('scelta', scelti.has(x.dataset.blocco)));
    opzioni.suSelezioneVoci?.(vociScelte());
  }

  function vociScelte() {
    return blocchi.filter((b) => scelti.has(String(b.id)));
  }

  function cella(rr, c) {
    return corpo.querySelector(`td.ed[data-r="${rr}"][data-c="${c}"]`);
  }

  function sposta(dr, dc) {
    if (!attiva) { attiva_(corpo.querySelector('td.ed')); return; }
    let rr = Number(attiva.dataset.r);
    let c = Number(attiva.dataset.c) + dc;
    if (c > 5) { c = 0; rr++; } else if (c < 0) { c = 5; rr--; }
    rr += dr;
    let td = cella(rr, c);
    // Righi di commento: solo la descrizione è modificabile.
    if (!td && dr !== 0) td = cella(rr, 0);
    if (!td && dc !== 0) td = cella(rr + Math.sign(dc), dc > 0 ? 0 : 5) || cella(rr, 0);
    if (td) attiva_(td);
  }

  function inizia(td, testoIniziale = null) {
    if (occupato || !td || !td.classList.contains('ed')) return;
    const tr = td.closest('tr');
    const { rigo } = rigoDi(tr);
    const campo = td.dataset.campo;
    const originale = rigo ? rigo[campo] ?? '' : '';
    const input = h('input', {
      class: 'editor-cella', value: testoIniziale ?? originale, spellcheck: false, autocomplete: 'off',
      inputMode: campo === 'descrizione' || campo === 'formula' ? 'text' : 'decimal',
      'aria-label': campo,
    });
    td.replaceChildren(input);
    td.classList.add('in-modifica');
    editor = { td, input, originale };
    input.focus();
    if (testoIniziale === null) input.select();
    else input.setSelectionRange(input.value.length, input.value.length);
    input.addEventListener('blur', () => { if (editor?.input === input) conferma(null); });
  }

  function annulla() {
    if (!editor) return;
    const { td, originale } = editor;
    editor = null;
    td.classList.remove('in-modifica');
    td.replaceChildren(...[contenuto(rigoDi(td.closest('tr')).rigo, td.dataset.campo, originale)].flat().filter((x) => x !== null));
    el.focus({ preventScroll: true });
  }

  async function conferma(movimento) {
    if (!editor) return;
    const { td, input, originale } = editor;
    editor = null;
    const valore = input.value;
    td.classList.remove('in-modifica');
    td.replaceChildren(valore);
    const tr = td.closest('tr');
    const { blocco, rigo } = rigoDi(tr);
    const campo = td.dataset.campo;
    const c = Number(td.dataset.c);
    if (valore.trim() === String(originale).trim()) {
      el.focus({ preventScroll: true });
      if (movimento) sposta(...movimento);
      return;
    }
    if (!rigo && valore.trim() === '') { el.focus({ preventScroll: true }); return; }
    // Cella da attivare dopo l'aggiornamento della vista.
    const numeroRigo = rigo ? rigo.numero : blocco.righi.length + 1;
    let prossima = { blocco: blocco.id, rigo: numeroRigo, c };
    if (movimento?.[0] === 1) prossima = rigo ? { blocco: blocco.id, rigo: numeroRigo + 1, c, oppureNuovo: true } : { blocco: blocco.id, rigo: 'nuovo', c };
    if (movimento?.[1] === 1) prossima = { blocco: blocco.id, rigo: numeroRigo, c: Math.min(5, c + 1) };
    occupato = true;
    try {
      if (rigo) await opzioni.suModifica(blocco, rigo.numero, rigoIngresso(rigo, { [campo]: valore }), prossima);
      else await opzioni.suAggiungi(blocco, null, rigoIngresso(null, { [campo]: valore }), prossima);
    } finally {
      occupato = false;
    }
  }

  el.addEventListener('mousedown', (e) => {
    const td = e.target.closest('td');
    if (!td || editor?.td === td) return;
    if (td.classList.contains('ed')) {
      e.preventDefault();
      if (editor) conferma(null);
      if (e.shiftKey && attiva) {
        const a = rigoDi(attiva.closest('tr'));
        if (a.rigo && (ancora === null || ancora.blocco !== a.blocco.id)) ancora = { blocco: a.blocco.id, numero: a.rigo.numero };
      } else ancora = null;
      attiva_(td, { scorri: false });
      evidenzia();
      el.focus({ preventScroll: true });
    } else {
      const tr = td.closest('tr');
      if (tr?.dataset.blocco) {
        const { blocco } = rigoDi(tr);
        const id = String(blocco.id);
        inAttesa = null;
        if (tr.classList.contains('testata') && (e.ctrlKey || e.metaKey)) {
          const nuovi = new Set(scelti);
          if (nuovi.has(id)) nuovi.delete(id); else nuovi.add(id);
          ancoraVoce = id;
          scegliVoci([...nuovi]);
        } else if (tr.classList.contains('testata') && e.shiftKey && ancoraVoce !== null) {
          const ids = blocchi.map((b) => String(b.id));
          const [da, a] = [ids.indexOf(ancoraVoce), ids.indexOf(id)].sort((x, y) => x - y);
          scegliVoci(da < 0 ? [id] : ids.slice(da, a + 1));
        } else if (tr.classList.contains('testata') && scelti.has(id) && scelti.size > 1) {
          inAttesa = id; // si decide al clic: trascinamento della selezione o voce singola
        } else {
          ancoraVoce = id;
          scegliVoci([id]);
        }
        opzioni.suSeleziona?.(blocco, rigoDi(tr).rigo);
      }
    }
  });
  el.addEventListener('click', (e) => {
    const tr = e.target.closest('tr.testata');
    if (tr && inAttesa !== null && tr.dataset.blocco === inAttesa) { ancoraVoce = inAttesa; scegliVoci([inAttesa]); }
    inAttesa = null;
  });

  if (opzioni.trascinaVoci) {
    el.addEventListener('dragstart', (e) => {
      const tr = e.target.closest?.('tr.testata');
      if (!tr || editor) { e.preventDefault(); return; }
      inAttesa = null;
      if (!scelti.has(tr.dataset.blocco)) { ancoraVoce = tr.dataset.blocco; scegliVoci([tr.dataset.blocco]); }
      const dati = opzioni.trascinaVoci(vociScelte().filter((b) => !b.bloccato));
      if (!dati) { e.preventDefault(); return; }
      trascina.inizia(e, dati);
    });
    el.addEventListener('dragend', () => { trascina.finisci(); pulisciRilascio(); });
  }

  /** Posizione (da 1) in cui finirebbero le voci rilasciate sul punto indicato. */
  function posizioneRilascio(e) {
    const tr = e.target.closest?.('tr[data-blocco]');
    if (!tr) return blocchi.length + 1;
    const i = blocchi.findIndex((b) => String(b.id) === tr.dataset.blocco);
    if (i < 0) return blocchi.length + 1;
    if (tr.classList.contains('testata')) {
      const r = tr.getBoundingClientRect();
      if (e.clientY < r.top + r.height / 2) return i + 1;
    }
    return i + 2;
  }

  function pulisciRilascio() {
    corpo.querySelectorAll('tr.rilascio-sopra').forEach((x) => x.classList.remove('rilascio-sopra'));
    el.classList.remove('rilascio-in-fondo');
  }

  if (opzioni.rilascio) {
    el.addEventListener('dragover', (e) => {
      if (!trascina.porta(e)) return;
      e.preventDefault();
      const p = posizioneRilascio(e);
      pulisciRilascio();
      const b = blocchi[p - 1];
      if (b) corpo.querySelector(`tr.testata[data-blocco="${CSS.escape(String(b.id))}"]`)?.classList.add('rilascio-sopra');
      else el.classList.add('rilascio-in-fondo');
    });
    el.addEventListener('dragleave', (e) => { if (!el.contains(e.relatedTarget)) pulisciRilascio(); });
    el.addEventListener('drop', (e) => {
      e.preventDefault();
      const p = posizioneRilascio(e);
      pulisciRilascio();
      opzioni.rilascio(e, p);
    });
  }
  el.addEventListener('dblclick', (e) => {
    const td = e.target.closest('td.ed');
    if (td) { attiva_(td, { scorri: false }); inizia(td); }
  });

  el.addEventListener('keydown', (e) => {
    if (editor) {
      if (e.key === 'Enter') { e.preventDefault(); conferma([1, 0]); }
      else if (e.key === 'Tab') { e.preventDefault(); conferma(e.shiftKey ? [0, -1] : [0, 1]); }
      else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); annulla(); }
      else if ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && !e.shiftKey) { e.preventDefault(); conferma(e.key === 'ArrowUp' ? [-1, 0] : [1, 0]); }
      return;
    }
    if (e.target !== el) return;
    const mod = e.ctrlKey || e.metaKey;
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && e.shiftKey && attiva) {
      // Selezione di più righi della stessa voce.
      e.preventDefault();
      const { blocco, rigo } = rigoDi(attiva.closest('tr'));
      if (!rigo) return;
      if (ancora === null || ancora.blocco !== blocco.id) ancora = { blocco: blocco.id, numero: rigo.numero };
      const prossimo = blocco.righi[rigo.numero - 1 + (e.key === 'ArrowDown' ? 1 : -1)];
      if (prossimo) {
        const td = corpo.querySelector(`tr.rigo[data-blocco="${CSS.escape(String(blocco.id))}"][data-rigo="${prossimo.numero}"] td.ed[data-c="${attiva.dataset.c}"]`)
          || corpo.querySelector(`tr.rigo[data-blocco="${CSS.escape(String(blocco.id))}"][data-rigo="${prossimo.numero}"] td.ed`);
        if (td) attiva_(td);
      }
      evidenzia();
      return;
    }
    if (!e.shiftKey && !mod && e.key.startsWith('Arrow') && ancora !== null) { ancora = null; evidenzia(); }
    switch (e.key) {
      case 'ArrowDown': e.preventDefault(); sposta(1, 0); break;
      case 'ArrowUp': e.preventDefault(); sposta(-1, 0); break;
      case 'ArrowRight': e.preventDefault(); sposta(0, 1); break;
      case 'ArrowLeft': e.preventDefault(); sposta(0, -1); break;
      case 'Tab': e.preventDefault(); sposta(0, e.shiftKey ? -1 : 1); break;
      case 'F2': e.preventDefault(); inizia(attiva); break;
      case 'Enter':
        e.preventDefault();
        if (mod) opzioni.suInserisci?.(); else inizia(attiva);
        break;
      case 'Insert': e.preventDefault(); opzioni.suInserisci?.(); break;
      case 'Delete':
      case 'Backspace':
        if (attiva && !mod) {
          e.preventDefault();
          const { rigo } = rigoDi(attiva.closest('tr'));
          if (rigo && (rigo[attiva.dataset.campo] ?? '') !== '') { inizia(attiva, ''); conferma(null); }
        } else if (attiva && mod && e.key === 'Delete') {
          e.preventDefault();
          opzioni.suElimina?.();
        }
        break;
      default:
        if (e.key.length === 1 && !mod && !e.altKey && attiva) { e.preventDefault(); inizia(attiva, e.key); }
    }
  });

  /** Righi selezionati (Maiusc+frecce) o il rigo attivo: {blocco, righi}. */
  function selezionati() {
    if (!attiva) return null;
    const { blocco, rigo } = rigoDi(attiva.closest('tr'));
    if (!rigo) return { blocco, righi: [] };
    if (ancora === null || ancora.blocco !== blocco.id) return { blocco, righi: [rigo] };
    const [da, a] = [Math.min(ancora.numero, rigo.numero), Math.max(ancora.numero, rigo.numero)];
    return { blocco, righi: blocco.righi.filter((x) => x.numero >= da && x.numero <= a) };
  }

  function evidenzia() {
    corpo.querySelectorAll('tr.nella-selezione').forEach((x) => x.classList.remove('nella-selezione'));
    const sel = selezionati();
    if (!sel || sel.righi.length < 2) return;
    for (const r of sel.righi) {
      corpo.querySelector(`tr.rigo[data-blocco="${CSS.escape(String(sel.blocco.id))}"][data-rigo="${r.numero}"]`)?.classList.add('nella-selezione');
    }
  }

  el.addEventListener('copy', (e) => {
    if (editor) return;
    const sel = selezionati();
    if (!sel?.righi.length) return;
    e.preventDefault();
    const tsv = sel.righi.map((r) => CAMPI.map((c) => cellaTsv(r[c])).join('\t')).join('\r\n');
    e.clipboardData.setData('text/plain', tsv + '\r\n');
    e.clipboardData.setData(FORMATO_RIGHI, JSON.stringify(sel.righi.map((r) => rigoIngresso(r))));
    opzioni.suNotifica?.(sel.righi.length === 1 ? 'Rigo copiato' : `${sel.righi.length} righi copiati`);
  });

  el.addEventListener('paste', (e) => {
    if (editor || !attiva || occupato) return;
    const { blocco, rigo } = rigoDi(attiva.closest('tr'));
    if (blocco.bloccato) return;
    e.preventDefault();
    const c = Number(attiva.dataset.c);
    let righi = null;
    try {
      const interni = e.clipboardData.getData(FORMATO_RIGHI);
      if (interni) righi = JSON.parse(interni);
    } catch { righi = null; }
    if (!righi) {
      const celle = leggiTsv(e.clipboardData.getData('text/plain') || '');
      if (!celle.length) return;
      if (celle.length === 1 && celle[0].length === 1 && rigo) {
        // Un solo valore: sostituisce il contenuto della cella attiva.
        inizia(attiva, c === 0 ? celle[0][0].trim() : numeroIncollato(celle[0][0]));
        conferma(null);
        return;
      }
      righi = righiDaCelle(celle, c);
    }
    const posizione = rigo ? rigo.numero + 1 : blocco.righi.length + 1;
    occupato = true;
    Promise.resolve(opzioni.suIncolla?.(blocco, posizione, righi, { blocco: blocco.id, rigo: posizione + righi.length - 1, c }))
      .finally(() => { occupato = false; });
  });

  /** Attiva la cella indicata ({blocco, rigo, c}); `rigo` può essere 'nuovo'. */
  function focalizza(p) {
    if (!p) return false;
    const trova = (n) => corpo.querySelector(`tr[data-blocco="${CSS.escape(String(p.blocco))}"][data-rigo="${n}"]`);
    let tr = trova(p.rigo);
    if (!tr && p.oppureNuovo) tr = trova('nuovo');
    if (!tr) return false;
    const td = tr.querySelector(`td.ed[data-c="${p.c ?? 0}"]`) || tr.querySelector('td.ed');
    if (!td) return false;
    // `mantieniScelte`: la selezione di più voci resta, se comprende quella attivata.
    attiva_(td, { mantieni: !!p.mantieniScelte });
    el.focus({ preventScroll: true });
    return true;
  }

  return {
    el,
    focalizza,
    /** Voce e rigo attivi. */
    attivo() {
      if (!attiva) return null;
      return { ...rigoDi(attiva.closest('tr')), c: Number(attiva.dataset.c) };
    },
    selezionati,
    /** Voci selezionate (testate), nell'ordine del foglio. */
    vociScelte,
  };
}
