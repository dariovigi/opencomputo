// Avvio dell'interfaccia: guscio (barra, navigazione, barra di stato), viste, scorciatoie.

import { h, sostituisci, euro, notifica, modale, chiediCampi, preferenza, salvaPreferenza } from './dom.js';
import { ascolta, finestra, scegliFile, scegliSalvataggio, esci, mac, titoloFinestra, apriFinestra } from './piattaforma.js';
import { stato, suCambio, aggiorna, aggiornaInfo, vai, agisci, chiedi, mostraErrore, computoCorrente } from './stato.js';
import * as generale from './viste/generale.js';
import * as elenco from './viste/elenco.js';
import * as computo from './viste/computo.js';
import * as classificazione from './viste/classificazione.js';
import * as libretto from './viste/libretto.js';
import * as sal from './viste/sal.js';
import * as conto from './viste/conto.js';
import * as analisi from './viste/analisi.js';
import * as aggiornaListino from './viste/aggiorna.js';
import * as nuovi from './viste/nuovi.js';
import * as report from './viste/report.js';
import * as cronoprogramma from './viste/cronoprogramma.js';
import * as problemi from './viste/problemi.js';
import * as assistente from './viste/assistente.js';

const VISTE = { generale, elenco, aggiorna: aggiornaListino, analisi, computo, classificazione, libretto, nuovi, sal, conto, cronoprogramma, report, problemi, assistente };

// Finestre secondarie: index.html#vista&computo=Nome
const indirizzo = new URLSearchParams(location.hash.slice(1));
const vistaIniziale = [...indirizzo.keys()].find((k) => k in VISTE);
const secondaria = finestra !== 'principale' && !!vistaIniziale;
const MOD = mac ? '⌘' : 'Ctrl+';

// ---------------------------------------------------------------------------
// Operazioni sui file

const FILTRO_PROGETTO = [{ name: 'Progetto OpenComputo', extensions: ['ocp', 'json'] }];
const FILTRO_IMPORT = [{ name: 'PriMus XPWE o DCF', extensions: ['xpwe', 'XPWE', 'dcf', 'DCF'] }];
const FILTRO_DCF = [{ name: 'PriMus DCF', extensions: ['dcf', 'DCF'] }];

/** Chiede se salvare le modifiche. `true` se si può procedere. */
async function salvaSeNecessario() {
  if (!stato.info?.modificato) return true;
  const r = await modale({
    titolo: 'Modifiche non salvate',
    contenuto: h('p', null, `Salvare le modifiche a «${stato.info.titolo}»?`),
    pulsanti: [{ testo: 'Annulla', valore: null }, { testo: 'Non salvare', valore: 'no', tipo: 'pericolo' }, { testo: 'Salva', valore: 'si', tipo: 'primario' }],
  });
  if (r === 'si') return salva();
  return r === 'no';
}

/**
 * Sostituisce il progetto aperto con quello della richiesta. Con `chiudiPrima = false`
 * il progetto aperto resta se la richiesta fallisce (il servizio lo sostituisce solo a
 * richiesta riuscita).
 */
async function sostituisciProgetto(richiesta, { chiudiPrima = true } = {}) {
  if (!(await salvaSeNecessario())) return null;
  if (chiudiPrima && stato.info) await chiedi({ richiesta: 'chiudi', forza: true }).catch(() => {});
  const r = await agisci(richiesta);
  if (r) vai('computo');
  return r;
}

export async function nuovo() {
  const v = await chiediCampi('Nuovo progetto', [{ nome: 'titolo', etichetta: 'Titolo dei lavori', valore: '', obbligatorio: true }], { ok: 'Crea' });
  if (v) await sostituisciProgetto({ richiesta: 'nuovo', titolo: v.titolo.trim() });
}

export async function apri() {
  const percorso = await scegliFile('Apri progetto', FILTRO_PROGETTO);
  if (percorso) await sostituisciProgetto({ richiesta: 'apri', percorso });
}

export async function importa() {
  const origine = await scegliFile('Importa da PriMus (XPWE o DCF)', FILTRO_IMPORT);
  if (!origine) return;
  const r = await sostituisciProgetto({ richiesta: 'importa', origine });
  if (!r) return;
  const imp = r.importazione;
  const errori = imp.anomalie.filter((a) => a.gravita === 'errore').length;
  modale({
    titolo: 'Importazione completata',
    contenuto: h('div', null,
      h('p', null, `Formato ${imp.formato.toUpperCase()}: ${imp.voci_elenco} voci di elenco prezzi, ${imp.voci_computo} voci di computo.`),
      imp.anomalie.length
        ? h('p', null, `${imp.anomalie.length} segnalazioni${errori ? `, di cui ${errori} con dati scartati` : ''}.`)
        : h('p', null, 'Nessuna segnalazione.'),
      imp.anomalie.length ? h('ul', { class: 'elenco-problemi' }, imp.anomalie.slice(0, 12).map((a) =>
        h('li', { class: a.gravita === 'errore' ? 'errore' : a.gravita === 'avviso' ? 'avviso' : 'informazione' }, a.messaggio))) : null,
      imp.anomalie.length > 12 ? h('p', { class: 'vuoto' }, `… e altre ${imp.anomalie.length - 12}.`) : null,
      h('p', { class: 'vuoto' }, 'Il progetto importato non è ancora salvato: usare «Salva con nome».')),
  });
}

/**
 * Convertitore DCF esterno (ADR 0027): mostra l'avviso e il programma configurato, e
 * permette di sceglierlo o toglierlo. Restituisce lo stato finale (o null se annullato).
 */
export async function configuraConvertitore() {
  let st;
  try { st = await chiedi({ richiesta: 'convertitore_stato' }); } catch (e) { mostraErrore(e); return null; }
  const pulsanti = [{ testo: 'Annulla', valore: null }];
  if (st.programma) pulsanti.push({ testo: 'Togli', valore: 'togli', tipo: 'pericolo' });
  pulsanti.push({ testo: st.programma ? 'Cambia programma…' : 'Scegli il programma…', valore: 'scegli', tipo: 'primario' });
  const scelta = await modale({
    titolo: 'Convertitore DCF esterno',
    contenuto: h('div', { class: 'convertitore' },
      h('p', null, st.avviso),
      h('p', null, 'Il programma viene lanciato come «programma convert <documento.dcf> <progetto.ocp> --report <resoconto.txt>».'),
      h('p', null, h('strong', null, 'Programma: '), st.programma ? h('code', null, st.programma) : 'nessuno'),
      st.problema ? h('p', { class: 'errore' }, st.problema) : null),
    pulsanti,
  });
  if (scelta === 'togli') {
    const r = await agisci({ richiesta: 'convertitore_imposta', programma: null });
    if (r) notifica('Convertitore tolto', 'info', null, 2000);
    return r;
  }
  if (scelta !== 'scegli') return null;
  const programma = await scegliFile('Scegli il convertitore DCF esterno', []);
  if (!programma) return null;
  const r = await agisci({ richiesta: 'convertitore_imposta', programma });
  if (r) notifica('Convertitore impostato', 'ok', null, 2000);
  return r;
}

/** Importa un DCF con il convertitore esterno; al primo uso chiede di configurarlo. */
export async function importaConConvertitore() {
  let st;
  try { st = await chiedi({ richiesta: 'convertitore_stato' }); } catch (e) { mostraErrore(e); return; }
  if (!st.utilizzabile) {
    st = await configuraConvertitore();
    if (!st?.utilizzabile) return;
  }
  const origine = await scegliFile('Importa DCF con il convertitore esterno', FILTRO_DCF);
  if (!origine) return;
  // Il progetto aperto resta se la conversione non riesce.
  const r = await sostituisciProgetto({ richiesta: 'importa_con_convertitore', origine }, { chiudiPrima: false });
  if (!r) return;
  const imp = r.importazione;
  const conv = r.convertitore;
  modale({
    titolo: 'Conversione completata',
    larga: true,
    contenuto: h('div', { class: 'convertitore' },
      h('p', null, `Convertito con «${conv.programma}»: ${imp.voci_elenco} voci di elenco prezzi, ${imp.voci_computo} voci di computo.`),
      h('p', null, 'Il progetto è stato prodotto da un programma esterno: controllarlo (vista «Problemi») prima di usarlo.'),
      h('h4', null, 'Resoconto del convertitore'),
      h('pre', { class: 'resoconto' }, conv.testo || '(nessun resoconto)'),
      conv.messaggi ? [h('h4', null, 'Messaggi'), h('pre', { class: 'resoconto' }, conv.messaggi)] : null,
      conv.troncato ? h('p', { class: 'vuoto' }, 'Il resoconto è stato troncato.') : null,
      h('p', { class: 'vuoto' }, 'Il progetto importato non è ancora salvato: usare «Salva con nome».')),
  });
}

/** Salva; `true` se riuscito. */
export async function salva() {
  if (!stato.info) return false;
  if (!stato.info.file) return salvaConNome();
  const r = await agisci({ richiesta: 'salva' });
  if (r) notifica('Progetto salvato', 'ok', null, 2000);
  return !!r;
}

export async function salvaConNome() {
  if (!stato.info) return false;
  const nome = (stato.info.titolo || 'progetto').replace(/[\\/:*?"<>|]+/g, ' ').trim().slice(0, 60) + '.ocp';
  let percorso = await scegliSalvataggio('Salva progetto', [{ name: 'Progetto OpenComputo', extensions: ['ocp'] }], nome);
  if (!percorso) return false;
  if (!/\.(ocp|json)$/i.test(percorso)) percorso += '.ocp';
  const r = await agisci({ richiesta: 'salva', percorso });
  if (r) notifica('Progetto salvato', 'ok', null, 2000);
  return !!r;
}

async function chiudiProgetto() {
  if (!(await salvaSeNecessario())) return;
  await agisci({ richiesta: 'chiudi', forza: true });
}

export async function annulla() {
  if (!stato.info?.annullabili?.length) return;
  const r = await agisci({ richiesta: 'annulla' });
  if (r) notifica(`Annullato: ${r.annullato}`, 'info', { testo: 'Ripristina', fn: ripristina }, 3000);
}

export async function ripristina() {
  if (!stato.info?.ripristinabili?.length) return;
  const r = await agisci({ richiesta: 'ripristina' });
  if (r) notifica(`Ripristinato: ${r.ripristinato}`, 'info', null, 2500);
}

// ---------------------------------------------------------------------------
// Guscio

const app = document.getElementById('app');
const elTitolo = h('div', { class: 'titolo-progetto' });
const pulsanteAnnulla = h('button', { class: 'pulsante icona', onclick: annulla, 'aria-label': 'Annulla' }, '↶ Annulla');
const pulsanteRipristina = h('button', { class: 'pulsante icona', onclick: ripristina, 'aria-label': 'Ripristina' }, '↷ Ripristina');
const pulsanteFinestra = secondaria ? null : costruisciPulsanteFinestra();
const pulsanteSalva = h('button', { class: 'pulsante', onclick: salva, title: `Salva (${MOD}S)` }, 'Salva');

function menuAltro() {
  const elenco_ = h('div', { class: 'menu-elenco', hidden: true, role: 'menu' });
  const voce = (testo, fn, tasto) => h('button', { role: 'menuitem', onclick: () => { chiudi(); fn(); } }, testo, tasto && h('span', { class: 'tasto' }, tasto));
  const chiudi = () => { elenco_.hidden = true; document.removeEventListener('mousedown', fuori, true); };
  const fuori = (e) => { if (!menu.contains(e.target)) chiudi(); };
  const menu = h('div', { class: 'menu' },
    h('button', {
      class: 'pulsante', 'aria-haspopup': 'menu',
      onclick: () => {
        sostituisci(elenco_,
          voce('Nuovo progetto…', nuovo, `${MOD}N`),
          voce('Apri…', apri, `${MOD}O`),
          voce('Importa da PriMus (XPWE, DCF)…', importa),
          voce('Importa DCF con convertitore esterno…', importaConConvertitore),
          voce('Convertitore DCF esterno…', configuraConvertitore),
          h('hr'),
          voce('Salva', salva, `${MOD}S`),
          voce('Salva con nome…', salvaConNome, `${MOD}⇧S`),
          h('hr'),
          voce('Chiudi progetto', chiudiProgetto));
        elenco_.hidden = !elenco_.hidden;
        if (!elenco_.hidden) document.addEventListener('mousedown', fuori, true);
      },
    }, 'File ▾'),
    elenco_);
  return menu;
}

const barra = h('header', { class: 'barra' },
  h('div', { class: 'marchio' }, h('img', { src: 'icona.png', alt: '' }), 'OpenComputo'),
  elTitolo,
  secondaria ? null : [menuAltro(), pulsanteSalva, h('span', { class: 'separatore' }), pulsanteAnnulla, pulsanteRipristina],
  secondaria ? null : pulsanteFinestra);

/** Apre una vista in una nuova finestra (per il computo, quello indicato). */
function apriInFinestra(v, computo = null) {
  const param = v === 'computo' && computo ? `&computo=${encodeURIComponent(computo)}` : '';
  const nome = v === 'computo' && computo ? computo : VISTE[v].titolo;
  apriFinestra(v + param, `${nome} — OpenComputo`).catch(mostraErrore);
}

function costruisciPulsanteFinestra() {
  return h('button', {
    class: 'pulsante icona', title: `Apri la vista corrente in una nuova finestra (anche ${MOD}clic o clic centrale su una voce del menu)`, 'aria-label': 'Nuova finestra',
    onclick: () => apriInFinestra(stato.vista.nome, computoCorrente()),
  }, '⧉');
}

const navigazione = h('nav', { class: 'navigazione', 'aria-label': 'Sezioni' });
const principale = h('main', { class: 'principale' });
const barraStato = h('footer', { class: 'stato-barra' });
const corpo = h('div', { class: ['corpo', secondaria && 'senza-navigazione'] }, secondaria ? null : navigazione, principale);
app.append(barra, corpo, barraStato);

function voceNav(nome, testo, parametri = {}, extra = null, classe = null) {
  const corrente = stato.vista.nome === nome && (nome !== 'computo' || computoCorrente() === parametri.computo);
  const inFinestra = () => apriInFinestra(nome, parametri.computo ?? null);
  return h('button', {
    class: [corrente && 'corrente', classe], 'aria-current': corrente ? 'page' : undefined,
    title: `${MOD}clic o clic centrale: apri in una nuova finestra`,
    onclick: (e) => ((mac ? e.metaKey : e.ctrlKey) ? inFinestra() : vai(nome, parametri)),
    onauxclick: (e) => { if (e.button === 1) { e.preventDefault(); inFinestra(); } },
    onmousedown: (e) => { if (e.button === 1) e.preventDefault(); },
  }, testo, extra);
}

function disegnaGuscio() {
  const i = stato.info;
  if (i?.file && !secondaria) ricordaRecente(i.file, i.titolo);
  const nomeFile = i?.file ? i.file.split(/[\\/]/).pop() : 'non salvato';
  const vistaSecondaria = secondaria && i ? (stato.vista.nome === 'computo' ? computoCorrente() : VISTE[stato.vista.nome]?.titolo) : null;
  sostituisci(elTitolo,
    h('strong', { class: i?.modificato && 'modificato' }, vistaSecondaria ?? (i ? i.titolo || '(senza titolo)' : 'Nessun progetto aperto')),
    i && h('small', { title: i.file || '' }, vistaSecondaria ? `${i.titolo} · ${nomeFile}` : nomeFile));
  if (secondaria && i) {
    const v = stato.vista.nome;
    const nomeVista = v === 'computo' ? computoCorrente() : VISTE[v]?.titolo;
    titoloFinestra(`${nomeVista} — ${i.titolo || 'OpenComputo'}`);
  } else titoloFinestra(i ? `${i.titolo} — OpenComputo` : 'OpenComputo');
  pulsanteSalva.disabled = !i;
  if (pulsanteFinestra) pulsanteFinestra.disabled = !i;
  pulsanteAnnulla.disabled = !i?.annullabili?.length;
  pulsanteRipristina.disabled = !i?.ripristinabili?.length;
  pulsanteAnnulla.title = i?.annullabili?.length ? `Annulla: ${i.annullabili.at(-1)} (${MOD}Z)` : 'Niente da annullare';
  pulsanteRipristina.title = i?.ripristinabili?.length ? `Ripristina: ${i.ripristinabili.at(-1)} (${mac ? '⌘⇧Z' : 'Ctrl+Y'})` : 'Niente da ripristinare';

  corpo.classList.toggle('vuoto', !i);
  if (!secondaria) {
    if (!i) sostituisci(navigazione);
    else {
      const p = i.problemi;
      const badge = p.errori ? h('span', { class: 'conteggio errore' }, String(p.errori)) : p.avvisi ? h('span', { class: 'conteggio avviso' }, String(p.avvisi)) : null;
      sostituisci(navigazione,
        h('h3', null, 'Progetto'),
        voceNav('generale', 'Dati generali'),
        voceNav('elenco', 'Elenco prezzi', {}, h('span', { class: 'conteggio' }, String(i.elenco_prezzi.voci))),
        voceNav('aggiorna', 'Aggiornamento da listino'),
        voceNav('analisi', 'Analisi prezzi', { voce: '' }, i.elenco_prezzi.analisi ? h('span', { class: 'conteggio' }, String(i.elenco_prezzi.analisi)) : null),
        voceNav('classificazione', 'Categorie, WBS, computi'),
        h('h3', null, 'Computi'),
        i.computi.map((c) => voceNav('computo', c.nome, { computo: c.nome }, h('span', { class: 'conteggio' }, String(c.voci)))),
        h('h3', null, 'Contabilità'),
        voceNav('libretto', 'Libretto delle misure', {}, i.contabilita.registrazioni ? h('span', { class: 'conteggio' }, String(i.contabilita.registrazioni)) : null),
        voceNav('nuovi', 'Nuovi prezzi', {}, i.elenco_prezzi.nuovi_prezzi ? h('span', { class: 'conteggio' }, String(i.elenco_prezzi.nuovi_prezzi)) : null),
        voceNav('sal', 'Stati di avanzamento', {}, i.contabilita.sal.length ? h('span', { class: 'conteggio' }, String(i.contabilita.sal.length)) : null),
        voceNav('conto', 'Conto finale', {}, i.contabilita.conto_finale ? h('span', { class: 'conteggio' }, i.contabilita.conto_finale === 'emesso' ? '✓' : 'bozza') : null),
        h('h3', null, 'Pianificazione'),
        voceNav('cronoprogramma', 'Cronoprogramma'),
        h('h3', null, 'Assistente'),
        voceNav('assistente', 'Assistente AI'),
        h('h3', null, 'Uscite'),
        voceNav('report', 'Report ed export'),
        voceNav('problemi', 'Controlli', {}, badge));
    }
  }

  if (!i) sostituisci(barraStato, h('span', null, 'Pronto'));
  else {
    const nome = computoCorrente();
    const c = i.computi.find((x) => x.nome === nome);
    sostituisci(barraStato,
      c && h('span', null, `${c.nome}: `, h('strong', null, c.errore ? 'non calcolabile' : euro(c.totale))),
      h('span', null, 'Errori ', h('strong', null, String(i.problemi.errori)), ' · avvisi ', h('strong', null, String(i.problemi.avvisi))),
      i.prezzari_caricati.length ? h('span', null, `Prezzari caricati: ${i.prezzari_caricati.map((p) => p.nome).join(', ')}`) : null,
      h('span', { class: 'spinge', title: i.file || '' }, i.file || 'Progetto non ancora salvato'));
  }
}

// File recenti: comodità di questo computer, conservata nel browser dell'applicazione.
const MAX_RECENTI = 8;

function recenti() {
  const r = preferenza('recenti', []);
  return Array.isArray(r) ? r.filter((x) => x && typeof x.percorso === 'string') : [];
}

function ricordaRecente(percorso, titolo) {
  const attuali = recenti();
  if (attuali[0]?.percorso === percorso && attuali[0]?.titolo === titolo) return;
  salvaPreferenza('recenti', [{ percorso, titolo }, ...attuali.filter((x) => x.percorso !== percorso)].slice(0, MAX_RECENTI));
}

function dimenticaRecente(percorso) {
  salvaPreferenza('recenti', recenti().filter((x) => x.percorso !== percorso));
}

function benvenuto() {
  return h('div', { class: 'benvenuto' }, h('div', null,
    h('img', { src: 'icona.png', alt: '' }),
    h('h1', null, 'OpenComputo'),
    h('p', null, 'Computi metrici estimativi e contabilità dei lavori pubblici.'),
    h('div', { class: 'azioni' },
      h('button', { onclick: nuovo }, h('strong', null, 'Nuovo progetto'), h('small', null, 'computo vuoto')),
      h('button', { onclick: apri }, h('strong', null, 'Apri progetto'), h('small', null, 'file .ocp')),
      h('button', { onclick: importa }, h('strong', null, 'Importa da PriMus'), h('small', null, 'XPWE o DCF non protetto'))),
    elencoRecenti(),
    h('p', { class: 'nota' }, 'I prezzari distribuiti da ACCA hanno condizioni d\'uso proprie: rispettarle è responsabilità di chi li importa.')));
}

function elencoRecenti() {
  const r = recenti();
  if (!r.length) return null;
  const el = h('section', { class: 'recenti', 'aria-label': 'File recenti' }, h('h2', null, 'Recenti'),
    r.map((x) => h('div', { style: { display: 'flex', alignItems: 'center' } },
      h('button', { onclick: () => sostituisciProgetto({ richiesta: 'apri', percorso: x.percorso }) },
        h('strong', null, x.titolo || x.percorso.split(/[\\/]/).pop()), h('small', { title: x.percorso }, x.percorso)),
      h('button', {
        class: 'link', title: 'Togli dall\'elenco', 'aria-label': `Togli ${x.percorso} dall'elenco`, style: { width: 'auto' },
        onclick: () => { dimenticaRecente(x.percorso); el.replaceWith(elencoRecenti() ?? h('span')); },
      }, '×'))));
  return el;
}

let vistaMostrata = null;
let disegnoInCorso = Promise.resolve();

async function disegnaVista(motivo) {
  if (!stato.info) {
    vistaMostrata = null;
    sostituisci(principale, secondaria ? h('div', { class: 'benvenuto' }, h('p', null, 'Nessun progetto aperto.')) : benvenuto());
    return;
  }
  const nome = stato.vista.nome in VISTE ? stato.vista.nome : 'generale';
  const vista = VISTE[nome];
  const cambiata = vistaMostrata !== nome || motivo === 'vista';
  if (cambiata) {
    vistaMostrata = nome;
    const el = h('section', { class: 'vista', 'aria-label': vista.titolo });
    sostituisci(principale, el);
    await vista.mostra(el, stato.vista.parametri, { nuova: true });
  } else {
    // Dopo un aggiornamento la vista viene ridisegnata: si conserva il campo attivo.
    const attivo = document.activeElement;
    const nome = principale.contains(attivo) && attivo.name ? attivo.name : null;
    await vista.mostra(principale.firstElementChild, stato.vista.parametri, { nuova: false });
    if (nome && !principale.contains(document.activeElement)) principale.querySelector(`[name="${CSS.escape(nome)}"]`)?.focus();
  }
}

// Disegni in corso (anche per i test: `data-disegno` torna a 0 quando l'interfaccia è ferma).
let inDisegno = 0;
document.body.dataset.disegno = '0';
suCambio((motivo) => {
  disegnaGuscio();
  inDisegno++;
  document.body.dataset.disegno = String(inDisegno);
  disegnoInCorso = disegnoInCorso.then(() => disegnaVista(motivo)).catch(mostraErrore).finally(() => {
    inDisegno--;
    document.body.dataset.disegno = String(inDisegno);
  });
});

// ---------------------------------------------------------------------------
// Scorciatoie da tastiera

document.addEventListener('keydown', (e) => {
  const mod = mac ? e.metaKey : e.ctrlKey;
  if (!mod || e.altKey || document.querySelector('.modale-sfondo')) return;
  const tasto = e.key.toLowerCase();
  const inTesto = e.target.matches('input, textarea, select') && !e.target.classList.contains('editor-cella');
  const azioni = {
    s: () => (e.shiftKey ? salvaConNome() : salva()),
    o: apri,
    n: nuovo,
  };
  if (!secondaria && azioni[tasto]) { e.preventDefault(); azioni[tasto](); return; }
  if (inTesto) return; // nei campi di testo vale l'annulla del campo
  if (tasto === 'z' && !e.shiftKey) { e.preventDefault(); annulla(); }
  else if ((tasto === 'z' && e.shiftKey) || tasto === 'y') { e.preventDefault(); ripristina(); }
});

// ---------------------------------------------------------------------------
// Eventi dell'applicazione

ascolta('progetto-aggiornato', (dato) => {
  if (dato?.origine !== finestra) aggiorna('dati').catch(mostraErrore);
});

// Doppio clic su un file .ocp con un progetto già aperto (macOS).
ascolta('apri-file', (dato) => {
  if (dato?.percorso) sostituisciProgetto({ richiesta: 'apri', percorso: dato.percorso });
});

ascolta('chiusura-richiesta', async () => {
  if (await salvaSeNecessario()) esci();
});

// ---------------------------------------------------------------------------
// Avvio

(async () => {
  if (vistaIniziale) stato.vista = { nome: vistaIniziale, parametri: Object.fromEntries(indirizzo) };
  try {
    await aggiornaInfo();
  } catch (e) {
    mostraErrore(e);
  }
  if (stato.info && !vistaIniziale) stato.vista = { nome: 'computo', parametri: {} };
  aggiorna('vista');
})();
