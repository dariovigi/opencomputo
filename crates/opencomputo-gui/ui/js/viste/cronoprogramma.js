// Cronoprogramma dei lavori (ADR 0024): editor in stile project management.
//
// A sinistra la tabella delle attività (WBS, nome, durata, date, predecessori,
// avanzamento, margine) con modifica in linea; a destra il diagramma di Gantt con
// barre, riepiloghi, pietre miliari, legami, percorso critico e linea di base. Le barre
// si spostano e si allungano trascinandole; trascinando una barra in verticale su
// un'altra si crea un legame fine-inizio. In basso i dettagli dell'attività e la
// produzione mensile con la curva a S.
//
// Ogni modifica è un comando del motore (annullabile): date, margini e importi
// vengono dal piano calcolato dal core. Il calendario replicato qui serve solo
// all'anteprima del trascinamento.

import { h, sostituisci, numero, notifica, modale, conferma, chiediCampi, preferenza, salvaPreferenza, oggi, data as dataIt } from '../dom.js';
import { chiedi, esegui, vai, mostraErrore } from '../stato.js';

export const titolo = 'Cronoprogramma';

const RIGA = 26;
const SCALE = { giorni: 24, settimane: 8, mesi: 2.4 };
const NOMI_SCALA = { giorni: 'Giorni', settimane: 'Settimane', mesi: 'Mesi' };
const TIPI = { fine_inizio: 'FI', inizio_inizio: 'II', fine_fine: 'FF', inizio_fine: 'IF' };
const DA_SIGLA = { FI: 'fine_inizio', II: 'inizio_inizio', FF: 'fine_fine', IF: 'inizio_fine' };
const NOMI_TIPI = {
  fine_inizio: 'Fine-inizio (FI)', inizio_inizio: 'Inizio-inizio (II)', fine_fine: 'Fine-fine (FF)', inizio_fine: 'Inizio-fine (IF)',
};
const MESI = ['gennaio', 'febbraio', 'marzo', 'aprile', 'maggio', 'giugno', 'luglio', 'agosto', 'settembre', 'ottobre', 'novembre', 'dicembre'];
const GIORNI = ['Lun', 'Mar', 'Mer', 'Gio', 'Ven', 'Sab', 'Dom'];
const NS = 'http://www.w3.org/2000/svg';

const locale = {
  zoom: preferenza('crono.zoom', 'giorni'),
  px: null, // scala personalizzata («Adatta»)
  critico: preferenza('crono.critico', true),
  base: preferenza('crono.base', true),
  pannello: preferenza('crono.pannello', 'attivita'), // 'attivita' | 'produzione' | null
  tabella: preferenza('crono.tabella', 700),
  chiusi: new Set(preferenza('crono.chiusi', [])),
  selezione: new Set(), // chiavi delle righe
  attiva: null, // chiave della riga attiva
  legame: null, // legame selezionato {da, a}
};

let elVista = null;
let dati = null;
let righe = []; // righe del piano
let visibili = []; // indici delle righe mostrate
let dom = {};

// ---------------------------------------------------------------------------
// Utilità

/** Elemento SVG con attributi (niente stili in linea: CSP). */
function s(tag, attr = {}, ...figli) {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attr)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') el.setAttribute('class', Array.isArray(v) ? v.filter(Boolean).join(' ') : v);
    else if (k === 'dataset') Object.entries(v).forEach(([a, b]) => el.setAttribute(`data-${a}`, b));
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, String(v));
  }
  for (const f of figli.flat()) {
    if (f === null || f === undefined || f === false) continue;
    el.appendChild(f instanceof Node ? f : document.createTextNode(String(f)));
  }
  return el;
}

const GIORNO = 86_400_000;
/** 'AAAA-MM-GG' → numero del giorno (UTC). */
function giorno(iso) {
  const [a, m, g] = iso.split('-').map(Number);
  return Math.round(Date.UTC(a, m - 1, g) / GIORNO);
}
/** Numero del giorno → 'AAAA-MM-GG'. */
function iso(n) {
  return new Date(n * GIORNO).toISOString().slice(0, 10);
}
function dataBreve(isoData) {
  if (!isoData) return '';
  const [a, m, g] = isoData.split('-');
  return `${g}/${m}/${a.slice(2)}`;
}
/** Data scritta dall'utente (gg/mm/aa, gg/mm/aaaa, aaaa-mm-gg) → ISO, o null. */
function leggiData(t) {
  t = t.trim();
  if (!t) return null;
  let m = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(t);
  if (m) return iso(giorno(`${m[1]}-${m[2].padStart(2, '0')}-${m[3].padStart(2, '0')}`));
  m = /^(\d{1,2})[/.-](\d{1,2})[/.-](\d{2,4})$/.exec(t);
  if (!m) throw new Error(`data non valida: «${t}»`);
  const anno = m[3].length === 2 ? 2000 + Number(m[3]) : Number(m[3]);
  const d = new Date(Date.UTC(anno, Number(m[2]) - 1, Number(m[1])));
  if (d.getUTCDate() !== Number(m[1])) throw new Error(`data non valida: «${t}»`);
  return d.toISOString().slice(0, 10);
}

function chiave(r) {
  return `${r.attivita.tipo}:${r.attivita.id}`;
}
function rifComando(r) {
  return r.attivita.tipo === 'wbs' ? { wbs: r.codice } : { attivita: r.attivita.id };
}
function rifDaLegame(rif) {
  if (rif.tipo === 'attivita') return { attivita: rif.id };
  const r = righe.find((x) => x.attivita.tipo === 'wbs' && x.attivita.id === rif.id);
  if (r) return { wbs: r.codice };
  return { wbs: dati?.codici_wbs?.[rif.id] ?? '' };
}

// Calendario replicato dal core (solo per l'anteprima del trascinamento).
function pasqua(anno) {
  const a = anno % 19; const b = Math.floor(anno / 100); const c = anno % 100;
  const d = Math.floor(b / 4); const e = b % 4; const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3); const hh = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4); const k = c % 4; const l = (32 + 2 * e + 2 * i - hh - k) % 7;
  const m = Math.floor((a + 11 * hh + 22 * l) / 451);
  const mese = Math.floor((hh + l - 7 * m + 114) / 31); const gg = ((hh + l - 7 * m + 114) % 31) + 1;
  return giorno(`${anno}-${String(mese).padStart(2, '0')}-${String(gg).padStart(2, '0')}`);
}
const FESTE = ['01-01', '01-06', '04-25', '05-01', '06-02', '08-15', '11-01', '12-08', '12-25', '12-26'];
function lavorativo(n) {
  const c = dati?.calendario;
  if (!c) return true;
  const dow = (new Date(n * GIORNO).getUTCDay() + 6) % 7; // 0 = lunedì
  if (!c.giorni_lavorativi[dow]) return false;
  const t = iso(n);
  if (c.festivita_nazionali && (FESTE.includes(t.slice(5)) || n === pasqua(Number(t.slice(0, 4))) + 1)) return false;
  return !c.chiusure.some((x) => giorno(x.dal) <= n && n <= giorno(x.al));
}
/** Giorni lavorativi da `a` a `b` compresi. */
function lavorativiTra(a, b) {
  let n = 0;
  for (let d = a; d <= b; d++) if (lavorativo(d)) n++;
  return n;
}
function primoLavorativo(n) {
  for (let i = 0; i < 3660 && !lavorativo(n); i++) n++;
  return n;
}

// ---------------------------------------------------------------------------
// Vista

export async function mostra(el, _parametri, { nuova }) {
  try {
    dati = await chiedi({ richiesta: 'cronoprogramma' });
  } catch (e) {
    mostraErrore(e);
    return;
  }
  dati.calendario.chiusure ??= [];
  righe = dati.piano?.righe ?? [];
  if (nuova || el !== elVista || !dom.corpo) costruisci(el);
  disegna();
}

function costruisci(el) {
  elVista = el;
  dom = {};
  dom.sintesi = h('span', { class: 'crono-sintesi' });
  dom.livello = h('select', { name: 'livello-dettaglio', 'aria-label': 'Livello di dettaglio della WBS', onchange: (e) => imposta({ livello_dettaglio: e.target.value ? Number(e.target.value) : null }) });
  dom.zoom = h('div', { class: 'gruppo-pulsanti', role: 'group', 'aria-label': 'Scala del diagramma' },
    Object.keys(SCALE).map((k) => h('button', { class: 'pulsante', name: `scala-${k}`, onclick: () => { locale.zoom = k; locale.px = null; salvaPreferenza('crono.zoom', k); disegna(); } }, NOMI_SCALA[k])),
    h('button', { class: 'pulsante', name: 'scala-adatta', title: 'Tutto il piano nella larghezza del diagramma', onclick: adatta }, 'Adatta'));
  const interruttore = (nome, testo, chiaveLocale, suggerimento) => h('button', {
    class: ['pulsante', locale[chiaveLocale] && 'premuto'], name: nome, 'aria-pressed': String(!!locale[chiaveLocale]), title: suggerimento,
    onclick: (e) => { locale[chiaveLocale] = !locale[chiaveLocale]; salvaPreferenza(`crono.${chiaveLocale}`, locale[chiaveLocale]); e.currentTarget.classList.toggle('premuto', locale[chiaveLocale]); e.currentTarget.setAttribute('aria-pressed', String(locale[chiaveLocale])); disegna(); },
  }, testo);
  dom.elimina = h('button', { class: 'pulsante pericolo', name: 'elimina-attivita', title: 'Elimina l\'attività aggiunta selezionata (Canc)', onclick: eliminaSelezionata }, 'Elimina');
  dom.collega = h('button', { class: 'pulsante', name: 'collega', title: 'Collega le attività selezionate in sequenza, fine-inizio', onclick: collegaSelezionate }, 'Collega');
  dom.scollega = h('button', { class: 'pulsante', name: 'scollega', title: 'Toglie i legami tra le attività selezionate', onclick: scollegaSelezionate }, 'Scollega');
  dom.lineaBase = h('button', { class: 'pulsante', name: 'linea-base', onclick: lineaBase }, 'Linea di base…');

  const testata = h('div', { class: 'vista-testata' },
    h('h2', null, titolo), dom.sintesi, h('span', { class: 'spinge' }),
    h('button', { class: 'pulsante', name: 'calendario', title: 'Inizio dei lavori, giorni lavorativi, festività, chiusure, computo degli importi', onclick: apriCalendario }, 'Calendario e inizio…'),
    dom.lineaBase,
    h('button', { class: 'pulsante', name: 'stampa-crono', onclick: () => vai('report', { tipo: 'cronoprogramma' }) }, 'Stampa…'));

  const strumenti = h('div', { class: 'crono-strumenti', role: 'toolbar', 'aria-label': 'Strumenti del cronoprogramma' },
    h('label', { class: 'campo-riga' }, 'Dettaglio', dom.livello),
    h('span', { class: 'separatore' }),
    h('button', { class: 'pulsante', name: 'nuova-attivita', title: 'Nuova attività senza voci (Ins)', onclick: () => nuovaAttivita(false) }, '+ Attività'),
    h('button', { class: 'pulsante', name: 'nuova-pietra', title: 'Nuova pietra miliare', onclick: () => nuovaAttivita(true) }, '+ Pietra miliare'),
    dom.elimina,
    h('span', { class: 'separatore' }),
    dom.collega, dom.scollega,
    h('span', { class: 'separatore' }),
    dom.zoom,
    h('button', { class: 'pulsante', name: 'vai-oggi', title: 'Porta il diagramma a oggi', onclick: vaiOggi }, 'Oggi'),
    h('span', { class: 'separatore' }),
    interruttore('percorso-critico', 'Percorso critico', 'critico', 'Attività senza margine in rosso, margine totale come linea sottile'),
    interruttore('mostra-linea-base', 'Linea di base', 'base', 'Barre della linea di base sotto quelle del piano'));
  dom.schede = h('div', { class: 'crono-schede' },
    h('div', { class: 'schede-tab', role: 'tablist', 'aria-label': 'Pannello inferiore' },
      [['attivita', 'Dettagli dell\'attività'], ['produzione', 'Produzione mensile']].map(([k, t]) => h('button', {
        role: 'tab', name: `pannello-${k}`, class: locale.pannello === k && 'corrente', 'aria-selected': String(locale.pannello === k),
        onclick: () => apriPannello(locale.pannello === k ? null : k),
      }, t))),
    h('span', { class: 'spinge' }),
    h('button', { class: 'pulsante icona', name: 'pannello-chiudi', title: locale.pannello ? 'Chiudi il pannello' : 'Apri il pannello', onclick: () => apriPannello(locale.pannello ? null : 'attivita') }, locale.pannello ? '▾' : '▴'));

  dom.banner = h('div', { class: 'crono-banner' });
  dom.testataTabella = h('div', { class: 'crono-testata-tabella' });
  dom.righeTabella = h('div', { class: 'crono-righe', role: 'rowgroup' });
  dom.tabella = h('div', { class: 'crono-tabella', tabIndex: 0, role: 'grid', 'aria-label': 'Attività del cronoprogramma', onkeydown: tasti },
    dom.testataTabella, dom.righeTabella);
  dom.testataGantt = h('div', { class: 'crono-testata-gantt' });
  dom.areaGantt = h('div', { class: 'crono-area-gantt' });
  dom.gantt = h('div', { class: 'crono-gantt', tabIndex: -1, onkeydown: tasti }, dom.testataGantt, dom.areaGantt);
  const divisore = h('div', { class: 'crono-divisore', role: 'separator', 'aria-orientation': 'vertical', title: 'Trascina per cambiare la larghezza della tabella' });
  divisore.addEventListener('pointerdown', (e) => trascinaDivisore(e, divisore));
  dom.corpo = h('div', { class: 'crono-corpo' }, dom.tabella, divisore, dom.gantt);
  dom.tabella.style.width = `${locale.tabella}px`;
  // Scorrimento verticale sincronizzato.
  let sincronizza = false;
  dom.tabella.addEventListener('scroll', () => { if (sincronizza) return; sincronizza = true; dom.gantt.scrollTop = dom.tabella.scrollTop; sincronizza = false; });
  dom.gantt.addEventListener('scroll', () => { if (sincronizza) return; sincronizza = true; dom.tabella.scrollTop = dom.gantt.scrollTop; sincronizza = false; });
  dom.pannello = h('div', { class: ['crono-pannello', !locale.pannello && 'chiuso'] });

  sostituisci(el, testata, strumenti,
    h('div', { class: 'vista-corpo pieno crono' }, h('div', { class: 'crono-colonna' }, dom.banner, dom.corpo, dom.schede, dom.pannello)));
}

/** Apre (o chiude, con null) il pannello inferiore. */
function apriPannello(k) {
  locale.pannello = k;
  salvaPreferenza('crono.pannello', k);
  const scorri = [dom.tabella.scrollTop, dom.gantt.scrollLeft];
  costruisci(elVista);
  disegna();
  [dom.tabella.scrollTop, dom.gantt.scrollLeft] = scorri;
  dom.gantt.scrollTop = scorri[0];
}

function trascinaDivisore(e, divisore) {
  e.preventDefault();
  divisore.setPointerCapture(e.pointerId);
  const x0 = e.clientX;
  const w0 = dom.tabella.offsetWidth;
  const muovi = (ev) => { locale.tabella = Math.max(240, Math.min(1100, w0 + ev.clientX - x0)); dom.tabella.style.width = `${locale.tabella}px`; };
  const fine = () => { divisore.removeEventListener('pointermove', muovi); salvaPreferenza('crono.tabella', locale.tabella); };
  divisore.addEventListener('pointermove', muovi);
  divisore.addEventListener('pointerup', fine, { once: true });
}

/** Righe mostrate (i riepiloghi chiusi nascondono ciò che contengono). */
function calcolaVisibili() {
  visibili = [];
  const chiusi = new Set();
  righe.forEach((r, i) => {
    if (r.padre !== null && r.padre !== undefined && chiusi.has(r.padre)) { chiusi.add(i); return; }
    if (r.riepilogo && locale.chiusi.has(chiave(r))) chiusi.add(i);
    visibili.push(i);
  });
  // Selezione solo su righe esistenti.
  const chiavi = new Set(righe.map(chiave));
  locale.selezione = new Set([...locale.selezione].filter((k) => chiavi.has(k)));
  if (locale.attiva && !chiavi.has(locale.attiva)) locale.attiva = null;
}

function indiceAttivo() {
  return righe.findIndex((r) => chiave(r) === locale.attiva);
}

function disegna() {
  if (!dom.corpo) return;
  calcolaVisibili();
  disegnaSintesi();
  // Livello di dettaglio.
  const livelli = dati.piano?.livelli_wbs ?? 0;
  sostituisci(dom.livello,
    Array.from({ length: livelli }, (_, i) => h('option', { value: String(i + 1), selected: dati.livello_dettaglio === i + 1 }, `Livello ${i + 1}`)),
    h('option', { value: '', selected: !dati.livello_dettaglio }, 'Tutti i livelli'));
  dom.livello.disabled = livelli === 0;
  dom.zoom.querySelectorAll('button[name^="scala-"]').forEach((b) => b.classList.toggle('premuto', !locale.px && b.name === `scala-${locale.zoom}`));
  dom.lineaBase.textContent = dati.linea_base ? `Linea di base (${dataIt(dati.linea_base)})…` : 'Salva linea di base';
  aggiornaPulsanti();

  // Avvisi.
  const avvisi = [];
  if (dati.errore) {
    const inizio = /inizio dei lavori/.test(dati.errore);
    avvisi.push(h('div', { class: 'banner avviso' }, h('span', null, inizio ? 'Per calcolare il piano serve la data di inizio dei lavori.' : dati.errore),
      inizio && h('button', { class: 'pulsante primario', name: 'imposta-inizio', onclick: apriCalendario }, 'Imposta l\'inizio dei lavori…')));
  } else if (!righe.length) {
    avvisi.push(h('div', { class: 'banner info' },
      h('span', null, 'Il cronoprogramma si costruisce sulla WBS: ogni nodo è un\'attività. Definire i nodi, oppure aggiungere attività e pietre miliari.'),
      h('button', { class: 'pulsante', onclick: () => vai('classificazione') }, 'Categorie, WBS, computi')));
  }
  const piano = dati.piano;
  if (piano?.scadenza_contrattuale && piano.fine > piano.scadenza_contrattuale) {
    avvisi.push(h('div', { class: 'banner errore' }, `La fine prevista (${dataIt(piano.fine)}) supera l'ultimazione contrattuale (${dataIt(piano.scadenza_contrattuale)}).`));
  }
  sostituisci(dom.banner, avvisi);
  dom.corpo.hidden = !piano;
  if (piano) {
    disegnaTabella();
    disegnaGantt();
  }
  disegnaPannello();
}

function disegnaSintesi() {
  const p = dati.piano;
  if (!p) { sostituisci(dom.sintesi); return; }
  sostituisci(dom.sintesi,
    h('span', null, 'Inizio ', h('strong', null, dataIt(p.inizio))),
    h('span', null, 'Fine ', h('strong', null, dataIt(p.fine))),
    h('span', null, h('strong', null, String(p.durata)), ' gg lavorativi (', String(p.durata_naturale), ' naturali)'),
    p.scadenza_contrattuale && h('span', { class: p.fine > p.scadenza_contrattuale ? 'fuori' : null }, 'Ultimazione contrattuale ', h('strong', null, dataIt(p.scadenza_contrattuale))));
}

function aggiornaPulsanti() {
  const sel = selezionate();
  const attiva = righe[indiceAttivo()];
  dom.elimina.disabled = !(attiva && attiva.attivita.tipo === 'attivita') && !locale.legame;
  dom.elimina.textContent = locale.legame ? 'Elimina legame' : 'Elimina';
  dom.collega.disabled = sel.length < 2;
  dom.scollega.disabled = sel.length < 2 || !(dati.piano?.legami ?? []).some((l) => sel.includes(l.riga_da) && sel.includes(l.riga_a));
}

/** Indici delle righe selezionate, nell'ordine della tabella. */
function selezionate() {
  return righe.map((r, i) => (locale.selezione.has(chiave(r)) ? i : -1)).filter((i) => i >= 0);
}

// ---------------------------------------------------------------------------
// Tabella

const COLONNE = [
  { id: 'n', titolo: 'N.', larghezza: '34px', allinea: 'destra' },
  { id: 'wbs', titolo: 'WBS', larghezza: '64px' },
  { id: 'nome', titolo: 'Nome attività', larghezza: 'minmax(160px, 1fr)' },
  { id: 'durata', titolo: 'Durata', larghezza: '58px', allinea: 'destra', modificabile: true },
  { id: 'inizio', titolo: 'Inizio', larghezza: '72px', allinea: 'centro', modificabile: true },
  { id: 'fine', titolo: 'Fine', larghezza: '72px', allinea: 'centro' },
  { id: 'predecessori', titolo: 'Predecessori', larghezza: '92px', modificabile: true },
  { id: 'avanzamento', titolo: '%', larghezza: '42px', allinea: 'destra', modificabile: true },
  { id: 'margine', titolo: 'Margine', larghezza: '64px', allinea: 'destra' },
];
const MODELLO = COLONNE.map((c) => c.larghezza).join(' ');

function testoPredecessori(r) {
  return r.predecessori.map((p) => {
    const tipo = p.tipo === 'fine_inizio' && p.ritardo === 0 ? '' : TIPI[p.tipo];
    const rit = p.ritardo > 0 ? `+${p.ritardo}` : p.ritardo < 0 ? String(p.ritardo) : '';
    return `${p.riga + 1}${tipo}${rit}`;
  }).join(';');
}

function valoreCella(r, i, col) {
  switch (col) {
    case 'n': return String(i + 1);
    case 'wbs': return r.codice;
    case 'durata': return r.pietra_miliare && !r.riepilogo ? '0 g' : `${r.durata} g${r.durata_stimata ? '?' : ''}`;
    case 'inizio': return dataBreve(r.inizio);
    case 'fine': return dataBreve(r.fine);
    case 'predecessori': return testoPredecessori(r);
    case 'avanzamento': return `${r.avanzamento}%`;
    case 'margine': return r.riepilogo ? '' : `${r.margine} g`;
    default: return '';
  }
}

function modificabile(r, col) {
  if (col === 'nome') return r.attivita.tipo === 'attivita';
  if (r.riepilogo) return col === 'predecessori';
  if (col === 'durata') return !r.pietra_miliare || r.attivita.tipo === 'wbs';
  return COLONNE.find((c) => c.id === col)?.modificabile ?? false;
}

function disegnaTabella() {
  sostituisci(dom.testataTabella, h('div', { class: 'crono-riga intestazione', role: 'row', style: { gridTemplateColumns: MODELLO } },
    COLONNE.map((c) => h('div', { class: ['cella', c.allinea], role: 'columnheader' }, c.titolo))));
  const nodi = visibili.map((i) => {
    const r = righe[i];
    const k = chiave(r);
    const celle = COLONNE.map((c) => {
      if (c.id === 'nome') {
        const freccia = r.riepilogo
          ? h('button', { class: 'crono-apri', tabIndex: -1, 'aria-label': locale.chiusi.has(k) ? 'Espandi' : 'Comprimi', onclick: (e) => { e.stopPropagation(); alternaRiepilogo(r); } }, locale.chiusi.has(k) ? '▸' : '▾')
          : h('span', { class: 'crono-apri vuoto' });
        return h('div', { class: ['cella', 'nome', r.riepilogo && 'riepilogo', r.pietra_miliare && 'pietra'], role: 'gridcell', dataset: { col: 'nome' }, title: r.note || r.nome },
          h('span', { class: 'rientro', style: { width: `${(r.livello - 1) * 14}px` } }), freccia,
          r.pietra_miliare && h('span', { class: 'segno-pietra', 'aria-hidden': 'true' }, '◆'),
          h('span', { class: 'testo' }, r.nome),
          r.inizio_non_prima && h('span', { class: 'segno-vincolo', title: `Inizio non prima del ${dataIt(r.inizio_non_prima)}` }, '⚑'));
      }
      return h('div', {
        class: ['cella', c.allinea, c.id === 'durata' && r.durata_stimata && 'stimata', c.id === 'margine' && locale.critico && r.critica && 'critica', !modificabile(r, c.id) && 'calcolata'],
        role: 'gridcell', dataset: { col: c.id },
      }, valoreCella(r, i, c.id));
    });
    return h('div', {
      class: ['crono-riga', r.riepilogo && 'riepilogo', locale.selezione.has(k) && 'selezionata', locale.attiva === k && 'attiva', locale.critico && r.critica && 'critica'],
      role: 'row', 'aria-selected': String(locale.selezione.has(k)), dataset: { indice: String(i) }, style: { gridTemplateColumns: MODELLO },
      onmousedown: (e) => { if (!e.target.closest('input')) seleziona(i, e); },
      ondblclick: (e) => { const col = e.target.closest('[data-col]')?.dataset.col; if (col && modificabile(r, col)) modificaCella(i, col); else if (locale.pannello !== 'attivita') apriPannello('attivita'); },
    }, celle);
  });
  sostituisci(dom.righeTabella, nodi);
}

function alternaRiepilogo(r) {
  const k = chiave(r);
  if (locale.chiusi.has(k)) locale.chiusi.delete(k); else locale.chiusi.add(k);
  salvaPreferenza('crono.chiusi', [...locale.chiusi]);
  disegna();
}

function seleziona(i, e = {}) {
  const k = chiave(righe[i]);
  locale.legame = null;
  if (e.shiftKey && locale.attiva) {
    const a = visibili.indexOf(indiceAttivo());
    const b = visibili.indexOf(i);
    locale.selezione = new Set(visibili.slice(Math.min(a, b), Math.max(a, b) + 1).map((x) => chiave(righe[x])));
  } else if (e.ctrlKey || e.metaKey) {
    if (locale.selezione.has(k)) locale.selezione.delete(k); else locale.selezione.add(k);
    locale.attiva = k;
  } else {
    locale.selezione = new Set([k]);
    locale.attiva = k;
  }
  if (!e.shiftKey) locale.attiva = k;
  aggiornaSelezione();
  dom.tabella.focus({ preventScroll: true });
}

/** Aggiorna gli evidenziamenti senza ridisegnare tutto. */
function aggiornaSelezione() {
  dom.righeTabella.querySelectorAll('.crono-riga').forEach((el) => {
    const r = righe[Number(el.dataset.indice)];
    const k = chiave(r);
    el.classList.toggle('selezionata', locale.selezione.has(k));
    el.classList.toggle('attiva', locale.attiva === k);
    el.setAttribute('aria-selected', String(locale.selezione.has(k)));
  });
  dom.areaGantt.querySelectorAll('.crono-sfondo-riga').forEach((el) => {
    const r = righe[Number(el.dataset.indice)];
    el.classList.toggle('selezionata', locale.selezione.has(chiave(r)));
  });
  dom.areaGantt.querySelectorAll('.legame').forEach((el) => {
    el.classList.toggle('selezionato', !!locale.legame && el.dataset.da === locale.legame.da && el.dataset.a === locale.legame.a);
  });
  aggiornaPulsanti();
  disegnaPannello();
}

function mostraRiga(i) {
  const pos = visibili.indexOf(i);
  if (pos < 0) return;
  const alto = pos * RIGA;
  const area = dom.tabella;
  const testata = dom.testataTabella.offsetHeight;
  if (alto < area.scrollTop) area.scrollTop = alto;
  else if (alto + RIGA > area.scrollTop + area.clientHeight - testata) area.scrollTop = alto + RIGA - area.clientHeight + testata;
}

function tasti(e) {
  if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT' || e.target.tagName === 'TEXTAREA') return;
  const i = indiceAttivo();
  const pos = visibili.indexOf(i);
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    const nuova = visibili[Math.max(0, Math.min(visibili.length - 1, (pos < 0 ? -1 : pos) + (e.key === 'ArrowDown' ? 1 : -1)))];
    if (nuova !== undefined) { seleziona(nuova, { shiftKey: e.shiftKey }); mostraRiga(nuova); }
  } else if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && i >= 0 && righe[i].riepilogo) {
    e.preventDefault();
    const chiuso = locale.chiusi.has(chiave(righe[i]));
    if ((e.key === 'ArrowLeft') !== chiuso) alternaRiepilogo(righe[i]);
  } else if ((e.key === 'Enter' || e.key === 'F2') && i >= 0) {
    e.preventDefault();
    if (modificabile(righe[i], 'durata')) modificaCella(i, 'durata');
  } else if (e.key === 'Delete' || e.key === 'Backspace') {
    if (locale.legame || (i >= 0 && righe[i].attivita.tipo === 'attivita')) { e.preventDefault(); eliminaSelezionata(); }
  } else if (e.key === 'Insert') {
    e.preventDefault();
    nuovaAttivita(false);
  } else if (/^[0-9]$/.test(e.key) && i >= 0 && modificabile(righe[i], 'durata') && !e.ctrlKey && !e.metaKey) {
    e.preventDefault();
    modificaCella(i, 'durata', e.key);
  }
}

/** Modifica in linea di una cella. */
function modificaCella(i, col, iniziale = null) {
  const r = righe[i];
  const riga = dom.righeTabella.querySelector(`.crono-riga[data-indice="${i}"]`);
  const cella = riga?.querySelector(`[data-col="${col}"]`);
  if (!cella) return;
  const valori = {
    nome: r.nome,
    durata: r.durata_stimata ? '' : String(r.durata),
    inizio: r.inizio_non_prima ? dataBreve(r.inizio_non_prima) : '',
    predecessori: testoPredecessori(r),
    avanzamento: String(r.avanzamento),
  };
  const input = h('input', {
    class: 'crono-modifica', name: `modifica-${col}`, value: iniziale ?? valori[col] ?? '',
    placeholder: col === 'inizio' ? dataBreve(r.inizio) : col === 'durata' ? 'giorni' : '', spellcheck: false, autocomplete: 'off',
    'aria-label': COLONNE.find((c) => c.id === col)?.titolo ?? 'Nome',
  });
  let finito = false;
  const chiudi = async (salva, sposta = 0) => {
    if (finito) return;
    finito = true;
    if (salva && input.value !== (valori[col] ?? '')) {
      try {
        const ok = await salvaCella(r, col, input.value);
        if (!ok) { finito = false; input.focus(); return; }
      } catch (err) {
        notifica(err.message, 'errore');
        finito = false;
        input.focus();
        return;
      }
    } else {
      disegnaTabella();
    }
    dom.tabella.focus({ preventScroll: true });
    if (sposta) {
      const pos = visibili.indexOf(i) + sposta;
      const nuova = visibili[pos];
      if (nuova !== undefined) { seleziona(nuova); mostraRiga(nuova); }
    }
  };
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') { e.preventDefault(); chiudi(true, 1); }
    else if (e.key === 'Escape') { e.preventDefault(); chiudi(false); }
    else if (e.key === 'Tab') { e.preventDefault(); chiudi(true); }
  });
  input.addEventListener('blur', () => chiudi(true));
  sostituisci(cella, input);
  cella.classList.add('in-modifica');
  input.focus();
  if (iniziale === null) input.select();
}

/** Comandi per il valore scritto in una cella. Restituisce false se rifiutato. */
async function salvaCella(r, col, testo) {
  const attivita = rifComando(r);
  let comandi;
  if (col === 'durata') {
    const t = testo.trim().replace(/\s*(g|gg|giorni)$/i, '');
    if (t && !/^\d+$/.test(t)) throw new Error('durata in giorni lavorativi: un numero intero');
    comandi = [{ comando: 'imposta_attivita', attivita, durata: t ? Number(t) : null }];
  } else if (col === 'inizio') {
    comandi = [{ comando: 'imposta_attivita', attivita, inizio_non_prima: leggiData(testo) }];
  } else if (col === 'avanzamento') {
    const t = testo.trim().replace('%', '');
    if (!/^\d+$/.test(t) || Number(t) > 100) throw new Error('avanzamento: da 0 a 100');
    comandi = [{ comando: 'imposta_attivita', attivita, avanzamento: Number(t) }];
  } else if (col === 'nome') {
    if (!testo.trim()) throw new Error('il nome non può essere vuoto');
    comandi = [{ comando: 'imposta_attivita', attivita, nome: testo.trim() }];
  } else if (col === 'predecessori') {
    comandi = comandiPredecessori(r, testo);
    if (!comandi.length) { disegnaTabella(); return true; }
  }
  return !!(await esegui(comandi));
}

/** Predecessori scritti come «2;3II+2;5FF-1» → comandi di collegamento. */
function comandiPredecessori(r, testo) {
  const i = righe.indexOf(r);
  const voluti = new Map();
  for (const parte of testo.split(/[;,]/).map((x) => x.trim()).filter(Boolean)) {
    const m = /^(\d+)\s*(FI|II|FF|IF)?\s*(?:([+-])\s*(\d+)\s*(?:g|gg)?)?$/i.exec(parte);
    if (!m) throw new Error(`predecessore non valido: «${parte}» (es. 3, 4II+2, 5FF-1)`);
    const n = Number(m[1]) - 1;
    if (!righe[n]) throw new Error(`non c'è l'attività n. ${m[1]}`);
    if (n === i) throw new Error('un\'attività non può precedere sé stessa');
    voluti.set(n, { tipo: DA_SIGLA[(m[2] || 'FI').toUpperCase()], ritardo: m[3] ? Number(`${m[3]}${m[4]}`) : 0 });
  }
  const comandi = [];
  for (const l of dati.piano.legami.filter((x) => x.riga_a === i)) {
    const v = voluti.get(l.riga_da);
    if (!v) comandi.push({ comando: 'scollega_attivita', da: rifDaLegame(l.da), a: rifDaLegame(l.a) });
  }
  for (const [n, v] of voluti) {
    const attuale = r.predecessori.find((p) => p.riga === n);
    if (attuale && attuale.tipo === v.tipo && attuale.ritardo === v.ritardo) continue;
    comandi.push({ comando: 'collega_attivita', da: rifComando(righe[n]), a: rifComando(r), tipo: v.tipo, ritardo: v.ritardo });
  }
  return comandi;
}

// ---------------------------------------------------------------------------
// Diagramma di Gantt

function scala() {
  return locale.px ?? SCALE[locale.zoom] ?? SCALE.giorni;
}

/** Intervallo dell'asse dei tempi: dal lunedì prima dell'inizio a un mese dopo la fine. */
function intervallo() {
  const p = dati.piano;
  let a = giorno(p.inizio);
  let b = giorno(p.fine);
  for (const r of righe) {
    if (r.inizio_base) a = Math.min(a, giorno(r.inizio_base));
    if (r.fine_base) b = Math.max(b, giorno(r.fine_base));
    if (r.fine_tardi) b = Math.max(b, giorno(r.fine_tardi));
  }
  if (p.scadenza_contrattuale) b = Math.max(b, giorno(p.scadenza_contrattuale));
  a -= 7;
  while ((new Date(a * GIORNO).getUTCDay() + 6) % 7 !== 0) a--;
  b += 35;
  return [a, b];
}

function adatta() {
  if (!dati.piano) return;
  const p = dati.piano;
  const giorni = giorno(p.fine) - giorno(p.inizio) + 14;
  locale.px = Math.max(0.5, (dom.gantt.clientWidth - 40) / giorni);
  disegna();
  dom.gantt.scrollLeft = Math.max(0, (giorno(p.inizio) - intervallo()[0] - 7) * scala());
}

function vaiOggi() {
  if (!dati.piano) return;
  const [a] = intervallo();
  dom.gantt.scrollLeft = Math.max(0, (giorno(oggi()) - a) * scala() - dom.gantt.clientWidth / 3);
}

function disegnaGantt() {
  const [a, b] = intervallo();
  const px = scala();
  const larghezza = Math.ceil((b - a) * px);
  const altezza = visibili.length * RIGA;
  const x = (n) => (n - a) * px;
  const xIso = (d) => x(giorno(d));

  // Testata dei tempi: mesi (o anni) sopra, giorni o settimane (o mesi) sotto.
  const testata = s('svg', { width: larghezza, height: 44, class: 'crono-svg-testata' });
  const alto = [];
  const basso = [];
  if (px >= 4) {
    for (let d = a; d < b;) {
      const dt = new Date(d * GIORNO);
      const prossimo = Math.round(Date.UTC(dt.getUTCFullYear(), dt.getUTCMonth() + 1, 1) / GIORNO);
      const fine = Math.min(prossimo, b);
      alto.push(s('line', { x1: x(d), y1: 0, x2: x(d), y2: 44, class: 'griglia-mese' }));
      const lungo = `${MESI[dt.getUTCMonth()]} ${dt.getUTCFullYear()}`;
      const breve = `${MESI[dt.getUTCMonth()].slice(0, 3)} ${String(dt.getUTCFullYear()).slice(2)}`;
      const spazio = (fine - d) * px - 10;
      const etichetta = spazio > lungo.length * 7 ? lungo : spazio > breve.length * 7 ? breve : null;
      if (etichetta) alto.push(s('text', { x: x(d) + 6, y: 15, class: 'testo-mese' }, etichetta));
      d = fine;
    }
    if (px >= 14) {
      for (let d = a; d < b; d++) {
        const dt = new Date(d * GIORNO);
        basso.push(s('text', { x: x(d) + px / 2, y: 37, class: ['testo-giorno', !lavorativo(d) && 'festivo'], 'text-anchor': 'middle' }, String(dt.getUTCDate())));
      }
    } else {
      for (let d = a; d < b; d += 7) {
        basso.push(s('line', { x1: x(d), y1: 22, x2: x(d), y2: 44, class: 'griglia-settimana' }));
        if (7 * px > 26) basso.push(s('text', { x: x(d) + 3, y: 37, class: 'testo-giorno' }, dataBreve(iso(d)).slice(0, 5)));
      }
    }
  } else {
    for (let d = a; d < b;) {
      const dt = new Date(d * GIORNO);
      const prossimo = Math.round(Date.UTC(dt.getUTCFullYear(), dt.getUTCMonth() + 1, 1) / GIORNO);
      const fine = Math.min(prossimo, b);
      basso.push(s('line', { x1: x(d), y1: 22, x2: x(d), y2: 44, class: 'griglia-settimana' }));
      if ((fine - d) * px > 14) basso.push(s('text', { x: x(d) + 2, y: 37, class: 'testo-giorno' }, MESI[dt.getUTCMonth()].slice(0, 3)));
      if (dt.getUTCMonth() === 0 || d === a) alto.push(s('text', { x: x(d) + 4, y: 15, class: 'testo-mese' }, String(dt.getUTCFullYear())));
      d = fine;
    }
  }
  testata.append(s('line', { x1: 0, y1: 22, x2: larghezza, y2: 22, class: 'griglia-mese' }), ...alto, ...basso);
  sostituisci(dom.testataGantt, testata);

  const svg = s('svg', { width: larghezza, height: Math.max(altezza, 10), class: 'crono-svg' },
    s('defs', {},
      s('marker', { id: 'freccia', viewBox: '0 0 8 8', refX: 7, refY: 4, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' },
        s('path', { d: 'M0,0 L8,4 L0,8 z', class: 'punta' })),
      s('marker', { id: 'freccia-sel', viewBox: '0 0 8 8', refX: 7, refY: 4, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' },
        s('path', { d: 'M0,0 L8,4 L0,8 z', class: 'punta selezionata' }))));
  // Righe di sfondo (selezione, clic).
  visibili.forEach((i, pos) => {
    svg.append(s('rect', {
      x: 0, y: pos * RIGA, width: larghezza, height: RIGA,
      class: ['crono-sfondo-riga', pos % 2 && 'pari', locale.selezione.has(chiave(righe[i])) && 'selezionata'],
      dataset: { indice: i }, onmousedown: (e) => seleziona(i, e),
    }));
  });
  // Giorni non lavorativi.
  if (px >= 4) {
    for (let d = a; d < b; d++) {
      if (!lavorativo(d)) svg.append(s('rect', { x: x(d), y: 0, width: Math.max(px, 1), height: altezza, class: 'non-lavorativo' }));
    }
  }
  // Oggi e ultimazione contrattuale.
  const oggiN = giorno(oggi());
  if (oggiN >= a && oggiN <= b) svg.append(s('line', { x1: x(oggiN), y1: 0, x2: x(oggiN), y2: altezza, class: 'linea-oggi' }));
  if (dati.piano.scadenza_contrattuale) {
    const xs = x(giorno(dati.piano.scadenza_contrattuale) + 1);
    svg.append(s('line', { x1: xs, y1: 0, x2: xs, y2: altezza, class: 'linea-scadenza' }, s('title', {}, `Ultimazione contrattuale ${dataIt(dati.piano.scadenza_contrattuale)}`)));
  }

  // Barre.
  const posizione = new Map(visibili.map((i, pos) => [i, pos]));
  const geometria = new Map();
  for (const i of visibili) {
    const r = righe[i];
    const pos = posizione.get(i);
    const y = pos * RIGA;
    const xa = xIso(r.inizio);
    const xb = xIso(r.fine) + px;
    geometria.set(i, { xa, xb, y });
    const g = s('g', { class: ['barra', r.riepilogo && 'riepilogo', r.pietra_miliare && 'pietra', locale.critico && r.critica && 'critica'], dataset: { indice: i } });
    if (locale.base && r.inizio_base && r.fine_base && !r.riepilogo) {
      const ba = xIso(r.inizio_base);
      const bb = r.pietra_miliare ? ba + 2 : xIso(r.fine_base) + px;
      g.append(s('rect', { x: ba, y: y + 19, width: Math.max(bb - ba, 2), height: 4, rx: 1, class: 'base' }));
    }
    const titoloBarra = `${r.codice ? `${r.codice} ` : ''}${r.nome}\n${dataIt(r.inizio)} – ${dataIt(r.fine)} · ${r.durata} gg lavorativi${r.riepilogo ? '' : ` · margine ${r.margine} g`}${r.avanzamento ? ` · ${r.avanzamento}%` : ''}`;
    if (r.pietra_miliare && !r.riepilogo) {
      const cx = xb;
      const cy = y + RIGA / 2;
      g.append(s('path', { d: `M${cx},${cy - 7} L${cx + 7},${cy} L${cx},${cy + 7} L${cx - 7},${cy} z`, class: 'rombo' }, s('title', {}, titoloBarra)));
      g.append(s('text', { x: cx + 10, y: cy + 4, class: 'etichetta' }, dataBreve(r.fine)));
    } else if (r.riepilogo) {
      const w = Math.max(xb - xa, 2);
      g.append(s('path', { d: `M${xa},${y + 8} H${xa + w} V${y + 17} L${xa + w - 5},${y + 12} H${xa + 5} L${xa},${y + 17} z`, class: 'forma' }, s('title', {}, titoloBarra)));
    } else {
      const w = Math.max(xb - xa, 3);
      if (locale.critico && r.margine > 0) {
        const xt = xIso(r.fine_tardi) + px;
        g.append(s('line', { x1: xb, y1: y + 13, x2: xt, y2: y + 13, class: 'margine' }));
      }
      g.append(s('rect', { x: xa, y: y + 6, width: w, height: 13, rx: 2, class: ['forma', r.durata_stimata && 'stimata'] }, s('title', {}, titoloBarra)));
      if (r.avanzamento > 0) g.append(s('rect', { x: xa, y: y + 10, width: (w * Math.min(r.avanzamento, 100)) / 100, height: 5, class: 'avanzamento' }));
      g.append(s('rect', { x: xa + w - 5, y: y + 6, width: 7, height: 13, class: 'maniglia', dataset: { maniglia: 'fine' } }));
    }
    if (!r.pietra_miliare && xb - xa < 300 && px >= 4) g.append(s('text', { x: xb + 6, y: y + 17, class: 'etichetta' }, r.nome.length > 40 ? `${r.nome.slice(0, 39)}…` : r.nome));
    g.addEventListener('pointerdown', (e) => trascinaBarra(e, i, { xa, xb, y, px, a }));
    g.addEventListener('dblclick', () => { if (locale.pannello !== 'attivita') apriPannello('attivita'); });
    svg.append(g);
  }

  // Legami.
  for (const l of dati.piano.legami) {
    const pa = geometria.get(l.riga_da);
    const pb = geometria.get(l.riga_a);
    if (!pa || !pb) continue;
    const rda = righe[l.riga_da];
    const ra = righe[l.riga_a];
    const dallaFine = l.tipo === 'fine_inizio' || l.tipo === 'fine_fine';
    const sullInizio = l.tipo === 'fine_inizio' || l.tipo === 'inizio_inizio';
    const x1 = dallaFine ? (rda.pietra_miliare && !rda.riepilogo ? pa.xb + 7 : pa.xb) : pa.xa;
    const x2 = sullInizio ? (ra.pietra_miliare && !ra.riepilogo ? pb.xb - 7 : pb.xa) : (ra.pietra_miliare && !ra.riepilogo ? pb.xb + 7 : pb.xb);
    const y1 = pa.y + RIGA / 2;
    const y2 = pb.y + RIGA / 2;
    const uscita = dallaFine ? x1 + 8 : x1 - 8;
    const accosto = sullInizio ? x2 - 8 : x2 + 8;
    const diretto = sullInizio ? uscita <= accosto : uscita >= accosto;
    const ym = y2 > y1 ? pb.y - 1 : pb.y + RIGA + 1;
    const d = diretto
      ? `M${x1},${y1} H${uscita} V${y2} H${x2}`
      : `M${x1},${y1} H${uscita} V${ym} H${accosto} V${y2} H${x2}`;
    const kda = `${l.da.tipo}:${l.da.id}`;
    const ka = `${l.a.tipo}:${l.a.id}`;
    const selezionato = locale.legame && locale.legame.da === kda && locale.legame.a === ka;
    const etichetta = `${rda.nome} → ${ra.nome} (${TIPI[l.tipo]}${l.ritardo > 0 ? `+${l.ritardo}` : l.ritardo < 0 ? l.ritardo : ''})`;
    const gruppo = s('g', { class: ['legame', selezionato && 'selezionato', locale.critico && rda.critica && ra.critica && 'critico'], dataset: { da: kda, a: ka } },
      s('path', { d, class: 'tratto', 'marker-end': selezionato ? 'url(#freccia-sel)' : 'url(#freccia)' }),
      s('path', { d, class: 'presa' }, s('title', {}, `${etichetta}\nClic per selezionare, doppio clic per modificare, Canc per togliere`)));
    gruppo.addEventListener('mousedown', (e) => { e.stopPropagation(); locale.legame = { da: kda, a: ka, l }; aggiornaSelezione(); dom.tabella.focus({ preventScroll: true }); });
    gruppo.addEventListener('dblclick', (e) => { e.stopPropagation(); modificaLegame(l); });
    svg.append(gruppo);
  }
  sostituisci(dom.areaGantt, svg);
  dom.areaGantt.style.width = `${larghezza}px`;
  dom.areaGantt.style.height = `${altezza}px`;
}

/**
 * Trascinamento di una barra: in orizzontale sposta l'attività (vincolo «inizio non
 * prima del»), dalla maniglia destra ne cambia la durata, in verticale su un'altra
 * barra crea un legame fine-inizio.
 */
function trascinaBarra(e, i, geo) {
  if (e.button !== 0) return;
  const r = righe[i];
  seleziona(i, e);
  if (e.shiftKey || e.ctrlKey || e.metaKey) return;
  const svg = dom.areaGantt.querySelector('svg');
  const maniglia = e.target.dataset?.maniglia === 'fine';
  let modo = null; // 'sposta' | 'durata' | 'collega'
  const x0 = e.clientX;
  const y0 = e.clientY;
  let fantasma = null;
  let linea = null;
  const g = e.currentTarget;
  g.setPointerCapture(e.pointerId);
  const muovi = (ev) => {
    const dx = ev.clientX - x0;
    const dy = ev.clientY - y0;
    if (!modo) {
      if (Math.abs(dy) > 10 && Math.abs(dy) > Math.abs(dx)) modo = 'collega';
      else if (Math.abs(dx) > 3) modo = maniglia && !r.riepilogo && !r.pietra_miliare ? 'durata' : (r.riepilogo ? null : 'sposta');
      if (!modo) return;
    }
    const rect = svg.getBoundingClientRect();
    if (modo === 'collega') {
      if (!linea) { linea = s('line', { class: 'collegamento-in-corso' }); svg.append(linea); }
      linea.setAttribute('x1', geo.xb); linea.setAttribute('y1', geo.y + RIGA / 2);
      linea.setAttribute('x2', ev.clientX - rect.left); linea.setAttribute('y2', ev.clientY - rect.top);
      return;
    }
    const giorni = Math.round(dx / geo.px);
    if (!fantasma) { fantasma = s('rect', { class: 'fantasma', height: 13, y: geo.y + 6, rx: 2 }); svg.append(fantasma); }
    if (modo === 'sposta') {
      fantasma.setAttribute('x', geo.xa + giorni * geo.px);
      fantasma.setAttribute('width', Math.max(geo.xb - geo.xa, 3));
    } else {
      fantasma.setAttribute('x', geo.xa);
      fantasma.setAttribute('width', Math.max(geo.xb - geo.xa + giorni * geo.px, geo.px));
    }
    fantasma.dataset.giorni = String(giorni);
  };
  const fine = async (ev) => {
    g.removeEventListener('pointermove', muovi);
    linea?.remove();
    fantasma?.remove();
    if (modo === 'collega') {
      const bersaglio = document.elementFromPoint(ev.clientX, ev.clientY)?.closest('[data-indice]');
      const j = bersaglio ? Number(bersaglio.dataset.indice) : -1;
      if (j >= 0 && j !== i) await esegui([{ comando: 'collega_attivita', da: rifComando(r), a: rifComando(righe[j]) }]);
      return;
    }
    const giorni = Number(fantasma?.dataset.giorni ?? 0);
    if (!giorni) return;
    if (modo === 'sposta') {
      const nuovo = primoLavorativo(giorno(r.inizio) + giorni);
      await esegui([{ comando: 'imposta_attivita', attivita: rifComando(r), inizio_non_prima: iso(nuovo) }]);
    } else if (modo === 'durata') {
      const fineNuova = Math.max(giorno(r.inizio), giorno(r.fine) + giorni);
      const durata = Math.max(1, lavorativiTra(giorno(r.inizio), fineNuova));
      await esegui([{ comando: 'imposta_attivita', attivita: rifComando(r), durata }]);
    }
  };
  g.addEventListener('pointermove', muovi);
  g.addEventListener('pointerup', fine, { once: true });
  g.addEventListener('pointercancel', () => { g.removeEventListener('pointermove', muovi); linea?.remove(); fantasma?.remove(); }, { once: true });
}

async function modificaLegame(l) {
  const v = await chiediCampi('Legame', [
    { nome: 'tipo', etichetta: 'Tipo', tipo: 'select', valore: l.tipo, opzioni: Object.entries(NOMI_TIPI) },
    { nome: 'ritardo', etichetta: 'Ritardo (giorni lavorativi, negativo = anticipo)', valore: String(l.ritardo), numerico: true },
  ], {
    ok: 'Applica',
    convalida: (x) => (/^-?\d+$/.test(String(x.ritardo).trim() || '0') ? null : 'Ritardo: un numero intero'),
  });
  if (!v) return;
  await esegui([{ comando: 'collega_attivita', da: rifDaLegame(l.da), a: rifDaLegame(l.a), tipo: v.tipo, ritardo: Number(String(v.ritardo).trim() || 0) }]);
}

// ---------------------------------------------------------------------------
// Azioni

async function imposta(campi) {
  await esegui([{ comando: 'imposta_cronoprogramma', ...campi }]);
}

/** Nodi WBS tra le righe (tutti quelli mostrati dal piano). */
function nodiWbs() {
  return righe.filter((r) => r.attivita.tipo === 'wbs').map((r) => [r.codice, `${r.codice} — ${r.nome}`]);
}

async function nuovaAttivita(pietra) {
  const attiva = righe[indiceAttivo()];
  let padre = '';
  if (attiva) {
    if (attiva.riepilogo && attiva.attivita.tipo === 'wbs') padre = attiva.codice;
    else if (attiva.padre !== null && attiva.padre !== undefined) padre = righe[attiva.padre].codice;
  }
  const campi = [
    { nome: 'nome', etichetta: pietra ? 'Nome della pietra miliare' : 'Nome dell\'attività', obbligatorio: true, valore: pietra ? 'Ultimazione dei lavori' : '' },
    { nome: 'padre', etichetta: 'Sotto il nodo WBS', tipo: 'select', valore: padre, opzioni: [['', '— radice del piano —'], ...nodiWbs()] },
  ];
  if (!pietra) campi.push({ nome: 'durata', etichetta: 'Durata (giorni lavorativi)', valore: '5', numerico: true });
  campi.push({ nome: 'predecessore', etichetta: 'Dopo l\'attività', tipo: 'select', valore: attiva ? String(indiceAttivo()) : '', opzioni: [['', '— nessun legame —'], ...righe.map((r, i) => [String(i), `${i + 1}. ${r.nome}`])] });
  const v = await chiediCampi(pietra ? 'Nuova pietra miliare' : 'Nuova attività', campi, {
    ok: 'Aggiungi',
    convalida: (x) => (!pietra && x.durata.trim() && !/^\d+$/.test(x.durata.trim()) ? 'Durata: un numero intero di giorni' : null),
  });
  if (!v) return;
  const prima = new Set((dati.piano?.righe ?? []).map(chiave));
  const esito = await esegui([{ comando: 'aggiungi_attivita', nome: v.nome, padre: v.padre || null, pietra_miliare: pietra, durata: pietra || !v.durata.trim() ? null : Number(v.durata) }]);
  if (!esito) return;
  // La nuova attività è l'unica riga nuova del piano: la si seleziona e la si collega.
  const nuova = (await chiedi({ richiesta: 'cronoprogramma' })).piano?.righe.find((r) => !prima.has(chiave(r)));
  if (nuova) {
    locale.selezione = new Set([chiave(nuova)]);
    locale.attiva = chiave(nuova);
    if (v.predecessore !== '' && righe[Number(v.predecessore)]) {
      await esegui([{ comando: 'collega_attivita', da: rifComando(righe[Number(v.predecessore)]), a: { attivita: nuova.attivita.id } }]);
    } else {
      disegna();
    }
  }
}

async function eliminaSelezionata() {
  if (locale.legame) {
    const l = locale.legame.l;
    locale.legame = null;
    await esegui([{ comando: 'scollega_attivita', da: rifDaLegame(l.da), a: rifDaLegame(l.a) }]);
    return;
  }
  const r = righe[indiceAttivo()];
  if (!r || r.attivita.tipo !== 'attivita') {
    notifica('Si eliminano solo le attività aggiunte: i nodi WBS si tolgono in «Categorie, WBS, computi»', 'info');
    return;
  }
  if (!(await conferma('Elimina attività', `Eliminare «${r.nome}» e i suoi legami?`, { ok: 'Elimina', pericolo: true }))) return;
  await esegui([{ comando: 'rimuovi_attivita', attivita: r.attivita.id }]);
}

async function collegaSelezionate() {
  const sel = selezionate();
  if (sel.length < 2) return;
  const comandi = [];
  for (let k = 1; k < sel.length; k++) comandi.push({ comando: 'collega_attivita', da: rifComando(righe[sel[k - 1]]), a: rifComando(righe[sel[k]]) });
  await esegui(comandi);
}

async function scollegaSelezionate() {
  const sel = new Set(selezionate());
  const comandi = dati.piano.legami.filter((l) => sel.has(l.riga_da) && sel.has(l.riga_a))
    .map((l) => ({ comando: 'scollega_attivita', da: rifDaLegame(l.da), a: rifDaLegame(l.a) }));
  if (comandi.length) await esegui(comandi);
}

async function lineaBase() {
  if (!dati.linea_base) {
    const esito = await esegui([{ comando: 'linea_base_cronoprogramma', data: oggi() }]);
    if (esito) notifica('Linea di base salvata: gli scostamenti si misurano da oggi', 'ok');
    return;
  }
  const r = await modale({
    titolo: 'Linea di base',
    contenuto: h('p', null, `La linea di base è del ${dataIt(dati.linea_base)}. Salvarne una nuova con le date attuali sostituisce la precedente.`),
    pulsanti: [{ testo: 'Annulla', valore: null }, { testo: 'Togli la linea di base', valore: 'togli', tipo: 'pericolo' }, { testo: 'Salva una nuova linea di base', valore: 'salva', tipo: 'primario' }],
  });
  if (r === 'salva') await esegui([{ comando: 'linea_base_cronoprogramma', data: oggi() }]);
  else if (r === 'togli') await esegui([{ comando: 'linea_base_cronoprogramma', cancella: true }]);
}

async function apriCalendario() {
  const c = dati.calendario;
  const inizio = h('input', { type: 'date', name: 'inizio-lavori', value: dati.inizio_lavori ?? '' });
  const giorni = c.giorni_lavorativi.map((v, i) => h('input', { type: 'checkbox', name: `giorno-${i}`, checked: v }));
  const feste = h('input', { type: 'checkbox', name: 'festivita', checked: c.festivita_nazionali });
  const computo = h('select', { name: 'computo-importi' },
    h('option', { value: '', selected: !dati.computo }, dati.computi.length ? `Il primo (${dati.computi[0]})` : 'Nessun computo'),
    dati.computi.map((n) => h('option', { value: n, selected: dati.computo === n }, n)));
  const chiusure = h('div', { class: 'crono-chiusure' });
  const rigaChiusura = (ch = { dal: '', al: '', descrizione: '' }) => {
    const r = h('div', { class: 'crono-chiusura' },
      h('input', { type: 'date', name: 'chiusura-dal', value: ch.dal, 'aria-label': 'Dal' }),
      h('input', { type: 'date', name: 'chiusura-al', value: ch.al, 'aria-label': 'Al' }),
      h('input', { type: 'text', name: 'chiusura-descrizione', value: ch.descrizione, placeholder: 'Ferie, fermo invernale…', 'aria-label': 'Descrizione' }),
      h('button', { class: 'pulsante icona', title: 'Togli', onclick: () => r.remove() }, '✕'));
    chiusure.append(r);
  };
  c.chiusure.forEach(rigaChiusura);
  const errore = h('div', { class: 'errore-modulo', role: 'alert' });
  const contenuto = h('div', { class: 'modulo crono-calendario' },
    h('label', { class: 'campo' }, h('span', null, 'Inizio dei lavori'), inizio,
      h('small', null, dati.consegna_lavori ? `Se vuoto: la consegna dei lavori del contratto (${dataIt(dati.consegna_lavori)}).` : 'Primo giorno di lavoro; se non è lavorativo si parte dal successivo.')),
    h('fieldset', { class: 'campo' }, h('legend', null, 'Giorni lavorativi'),
      h('div', { class: 'crono-giorni' }, giorni.map((g, i) => h('label', { class: 'campo-riga' }, g, GIORNI[i])))),
    h('label', { class: 'campo-riga' }, feste, 'Festività nazionali non lavorative (con il lunedì dell\'Angelo)'),
    h('fieldset', { class: 'campo' }, h('legend', null, 'Chiusure del cantiere'), chiusure,
      h('button', { class: 'pulsante', name: 'aggiungi-chiusura', onclick: () => rigaChiusura() }, '+ Chiusura')),
    h('label', { class: 'campo' }, h('span', null, 'Importi della produzione mensile dal computo'), computo),
    errore);
  const leggi = () => ({
    inizio_lavori: inizio.value || null,
    giorni_lavorativi: giorni.map((g) => g.checked),
    festivita_nazionali: feste.checked,
    chiusure: [...chiusure.children].map((r) => ({
      dal: r.querySelector('[name=chiusura-dal]').value,
      al: r.querySelector('[name=chiusura-al]').value,
      descrizione: r.querySelector('[name=chiusura-descrizione]').value.trim(),
    })).filter((x) => x.dal || x.al),
    computo: computo.value || null,
  });
  const r = await modale({
    titolo: 'Calendario e inizio dei lavori', larga: true, contenuto,
    pulsanti: [{ testo: 'Annulla', valore: null }, { testo: 'Applica', valore: 'ok', tipo: 'primario' }],
    prima: () => {
      const v = leggi();
      if (!v.giorni_lavorativi.some(Boolean)) { errore.textContent = 'Indicare almeno un giorno lavorativo.'; return false; }
      const sbagliata = v.chiusure.find((x) => !x.dal || !x.al || x.al < x.dal);
      if (sbagliata) { errore.textContent = 'Ogni chiusura ha un inizio e una fine, con la fine non prima dell\'inizio.'; return false; }
      return true;
    },
  });
  if (r !== 'ok') return;
  await imposta(leggi());
}

// ---------------------------------------------------------------------------
// Pannello inferiore

function disegnaPannello() {
  if (!dom.pannello) return;
  dom.pannello.classList.toggle('chiuso', !locale.pannello);
  if (!locale.pannello) { sostituisci(dom.pannello); return; }
  if (locale.pannello === 'produzione') { disegnaProduzione(); return; }
  const i = indiceAttivo();
  const r = righe[i];
  if (!r) { sostituisci(dom.pannello, h('p', { class: 'vuoto' }, 'Selezionare un\'attività per vederne e modificarne i dati.')); return; }
  const attivita = rifComando(r);
  const comando = (campi) => esegui([{ comando: 'imposta_attivita', attivita, ...campi }]);
  const campo = (etichetta, el, aiuto) => h('label', { class: 'campo' }, h('span', null, etichetta), el, aiuto && h('small', null, aiuto));
  const aggiunta = r.attivita.tipo === 'attivita';
  const foglia = !r.riepilogo;

  const nome = h('input', { name: 'dettaglio-nome', value: r.nome, disabled: !aggiunta, title: aggiunta ? '' : 'Il nome dei nodi WBS si cambia in «Categorie, WBS, computi»' });
  nome.addEventListener('change', () => { if (nome.value.trim()) comando({ nome: nome.value.trim() }); });
  const durata = h('input', { name: 'dettaglio-durata', type: 'number', min: 0, step: 1, value: r.durata_stimata ? '' : String(r.durata), placeholder: '1 (da stimare)', disabled: !foglia || (r.pietra_miliare && aggiunta) });
  durata.addEventListener('change', () => comando({ durata: durata.value === '' ? null : Math.max(0, Math.round(Number(durata.value))) }));
  const vincolo = h('input', { name: 'dettaglio-vincolo', type: 'date', value: r.inizio_non_prima ?? '', disabled: !foglia });
  vincolo.addEventListener('change', () => comando({ inizio_non_prima: vincolo.value || null }));
  const avanzamento = h('input', { name: 'dettaglio-avanzamento', type: 'range', min: 0, max: 100, step: 5, value: String(r.avanzamento), disabled: !foglia });
  const etichettaAv = h('output', null, `${r.avanzamento}%`);
  avanzamento.addEventListener('input', () => { etichettaAv.textContent = `${avanzamento.value}%`; });
  avanzamento.addEventListener('change', () => comando({ avanzamento: Number(avanzamento.value) }));
  const note = h('textarea', { name: 'dettaglio-note', rows: 2, value: r.note ?? '', placeholder: 'Annotazioni sull\'attività' });
  note.addEventListener('change', () => comando({ note: note.value.trim() || null }));
  const pietra = aggiunta && h('label', { class: 'campo-riga' },
    h('input', { type: 'checkbox', name: 'dettaglio-pietra', checked: r.pietra_miliare, onchange: (e) => comando({ pietra_miliare: e.target.checked }) }), 'Pietra miliare (durata zero)');

  // Predecessori e successori.
  const predecessori = dati.piano.legami.filter((l) => l.riga_a === i);
  const successori = dati.piano.legami.filter((l) => l.riga_da === i);
  const rigaLegame = (l, altro) => {
    const tipo = h('select', { name: 'legame-tipo', 'aria-label': 'Tipo di legame' }, Object.entries(NOMI_TIPI).map(([k, t]) => h('option', { value: k, selected: l.tipo === k }, t)));
    const ritardo = h('input', { name: 'legame-ritardo', type: 'number', step: 1, value: String(l.ritardo), 'aria-label': 'Ritardo in giorni lavorativi', class: 'corto' });
    const aggiorna = () => esegui([{ comando: 'collega_attivita', da: rifDaLegame(l.da), a: rifDaLegame(l.a), tipo: tipo.value, ritardo: Math.round(Number(ritardo.value) || 0) }]);
    tipo.addEventListener('change', aggiorna);
    ritardo.addEventListener('change', aggiorna);
    return h('tr', null,
      h('td', { class: 'destra' }, String(altro + 1)),
      h('td', null, h('button', { class: 'link', onclick: () => { seleziona(altro); mostraRiga(altro); } }, righe[altro].nome)),
      h('td', null, tipo), h('td', null, ritardo),
      h('td', null, h('button', { class: 'pulsante icona', title: 'Togli il legame', onclick: () => esegui([{ comando: 'scollega_attivita', da: rifDaLegame(l.da), a: rifDaLegame(l.a) }]) }, '✕')));
  };
  const nuovoPred = h('select', { name: 'nuovo-predecessore', 'aria-label': 'Nuovo predecessore' },
    h('option', { value: '' }, '+ Aggiungi predecessore…'),
    righe.map((x, j) => j !== i && h('option', { value: String(j) }, `${j + 1}. ${x.nome}`)));
  nuovoPred.addEventListener('change', () => {
    if (nuovoPred.value === '') return;
    esegui([{ comando: 'collega_attivita', da: rifComando(righe[Number(nuovoPred.value)]), a: attivita }]);
  });
  const tabellaLegami = (elenco, chiaveAltro, vuoto) => (elenco.length
    ? h('table', { class: 'crono-legami' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Attività'), h('th', null, 'Tipo'), h('th', null, 'Ritardo gg'), h('th', null, ''))),
      h('tbody', null, elenco.map((l) => rigaLegame(l, l[chiaveAltro]))))
    : h('p', { class: 'vuoto' }, vuoto));

  const scostamento = r.scostamento === null || r.scostamento === undefined ? null
    : r.scostamento === 0 ? 'in linea' : r.scostamento > 0 ? `${r.scostamento} g di ritardo` : `${-r.scostamento} g di anticipo`;
  const date = h('dl', { class: 'crono-date' },
    h('dt', null, 'Inizio / fine'), h('dd', null, `${dataIt(r.inizio)} – ${dataIt(r.fine)}`),
    foglia && [h('dt', null, 'Al più tardi'), h('dd', null, `${dataIt(r.inizio_tardi)} – ${dataIt(r.fine_tardi)}`)],
    h('dt', null, 'Margine totale'), h('dd', { class: r.critica ? 'critica' : null }, foglia ? `${r.margine} g${r.critica ? ' — critica' : ''}` : `${r.margine} g (minimo delle attività contenute)`),
    r.inizio_base && [h('dt', null, 'Linea di base'), h('dd', null, `${dataIt(r.inizio_base)} – ${dataIt(r.fine_base)}${scostamento ? ` (${scostamento})` : ''}`)],
    r.importo !== null && r.importo !== undefined && [h('dt', null, 'Importo'), h('dd', null, `${numero(r.importo, 2)} €`)]);

  sostituisci(dom.pannello,
    h('div', { class: 'crono-dettagli' },
      h('section', null,
        h('h3', null, `${i + 1}. ${r.codice ? `${r.codice} ` : ''}${r.riepilogo ? 'Riepilogo' : r.pietra_miliare ? 'Pietra miliare' : 'Attività'}`),
        h('div', { class: 'modulo due-colonne' },
          campo('Nome', nome),
          campo('Durata (giorni lavorativi)', durata, r.riepilogo ? 'Dalle attività contenute' : r.durata_stimata ? 'Da stimare: vale 1 giorno' : null),
          campo('Inizio non prima del', vincolo, foglia ? 'Vincolo; vuoto: il più presto possibile' : null),
          h('label', { class: 'campo' }, h('span', null, 'Avanzamento'), h('div', { class: 'campo-riga' }, avanzamento, etichettaAv)),
          pietra,
          h('label', { class: 'campo largo' }, h('span', null, 'Note'), note))),
      h('section', null, h('h3', null, 'Date calcolate'), date),
      h('section', null,
        h('h3', null, 'Predecessori'), tabellaLegami(predecessori, 'riga_da', 'Nessun predecessore: inizia appena possibile.'), nuovoPred,
        h('h3', null, 'Successori'), tabellaLegami(successori, 'riga_a', 'Nessun successore.'))));
}

function disegnaProduzione() {
  const p = dati.piano;
  if (!p) { sostituisci(dom.pannello, h('p', { class: 'vuoto' }, 'Il piano non è calcolabile.')); return; }
  const computo = h('select', { name: 'computo-produzione', 'aria-label': 'Computo degli importi' },
    h('option', { value: '', selected: !dati.computo }, dati.computi.length ? `Il primo (${dati.computi[0]})` : 'Nessun computo'),
    dati.computi.map((n) => h('option', { value: n, selected: dati.computo === n }, n)));
  computo.addEventListener('change', () => imposta({ computo: computo.value || null }));
  if (!p.mesi.length) {
    sostituisci(dom.pannello, h('div', { class: 'crono-produzione' },
      h('div', { class: 'campo-riga' }, 'Importi dal computo', computo),
      h('p', { class: 'vuoto' }, 'Nessun importo da distribuire: assegnare le voci del computo ai nodi WBS.')));
    return;
  }
  // Grafico: istogramma mensile e curva cumulata (curva a S).
  const W = Math.max(360, p.mesi.length * 56);
  const H = 170;
  const M = { s: 54, d: 46, a: 12, b: 26 };
  const larg = W - M.s - M.d;
  const alt = H - M.a - M.b;
  const massimo = Math.max(...p.mesi.map((m) => Number(m.importo)), 1);
  const totale = Number(p.importo_pianificato) || 1;
  const bw = larg / p.mesi.length;
  const grafico = s('svg', { width: W, height: H, class: 'crono-curva', role: 'img', 'aria-label': 'Produzione mensile e curva cumulata' });
  for (let k = 0; k <= 4; k++) {
    const y = M.a + alt - (alt * k) / 4;
    grafico.append(s('line', { x1: M.s, x2: M.s + larg, y1: y, y2: y, class: 'griglia' }),
      s('text', { x: M.s - 6, y: y + 4, 'text-anchor': 'end', class: 'asse' }, numero(String(Math.round((massimo * k) / 4)), 0)),
      s('text', { x: M.s + larg + 6, y: y + 4, class: 'asse' }, `${k * 25}%`));
  }
  const punti = [];
  p.mesi.forEach((m, k) => {
    const v = Number(m.importo);
    const hgt = (alt * v) / massimo;
    const x0 = M.s + k * bw;
    grafico.append(s('rect', { x: x0 + bw * 0.18, y: M.a + alt - hgt, width: bw * 0.64, height: hgt, class: 'colonna' }, s('title', {}, `${MESI[m.mese - 1]} ${m.anno}: ${numero(m.importo, 2)} €`)));
    grafico.append(s('text', { x: x0 + bw / 2, y: H - 8, 'text-anchor': 'middle', class: 'asse' }, `${MESI[m.mese - 1].slice(0, 3)} ${String(m.anno).slice(2)}`));
    punti.push([x0 + bw, M.a + alt - (alt * Number(m.cumulato)) / totale]);
  });
  grafico.append(s('polyline', { points: [[M.s, M.a + alt], ...punti].map((q) => q.join(',')).join(' '), class: 'cumulata' }));
  punti.forEach(([px, py], k) => grafico.append(s('circle', { cx: px, cy: py, r: 3, class: 'punto' }, s('title', {}, `${p.mesi[k].percentuale}% cumulato`))));

  sostituisci(dom.pannello, h('div', { class: 'crono-produzione' },
    h('div', { class: 'crono-produzione-testata' },
      h('label', { class: 'campo-riga' }, 'Importi dal computo', computo),
      h('span', null, `Pianificato ${numero(p.importo_pianificato, 2)} €`),
      Number(p.importo_non_assegnato) ? h('span', { class: 'avviso' }, `Voci senza WBS, non distribuite: ${numero(p.importo_non_assegnato, 2)} €`) : null),
    h('div', { class: 'crono-produzione-corpo' },
      h('div', { class: 'crono-curva-contenitore' }, grafico),
      h('table', { class: 'crono-mesi' },
        h('thead', null, h('tr', null, h('th', null, 'Mese'), h('th', { class: 'destra' }, 'Importo €'), h('th', { class: 'destra' }, 'Cumulato €'), h('th', { class: 'destra' }, '%'))),
        h('tbody', null, p.mesi.map((m) => h('tr', null,
          h('td', null, `${MESI[m.mese - 1]} ${m.anno}`),
          h('td', { class: 'destra' }, numero(m.importo, 2)),
          h('td', { class: 'destra' }, numero(m.cumulato, 2)),
          h('td', { class: 'destra' }, numero(m.percentuale, 2)))))))));
}
