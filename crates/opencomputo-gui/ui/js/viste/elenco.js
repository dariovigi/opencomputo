// Elenco prezzi del progetto e prezzari di consultazione (DCF, XPWE, .ocp).

import { h, sostituisci, numero, percento, natura, rimanda, decimale, opzionale, controllo, chiediCampi, conferma, notifica, preferenza, salvaPreferenza } from '../dom.js';
import { chiedi, esegui, mostraErrore, stato, vai, computoCorrente, agisci } from '../stato.js';
import { scegliFile } from '../piattaforma.js';
import { griglia } from '../griglia.js';
import * as trascina from '../trascina.js';
import { pannelloFiltri } from '../filtri.js';

export const titolo = 'Elenco prezzi';

const locale = { scheda: 'progetto', testo: '', prezzario: null, codice: null };

const COLONNE = [
  { titolo: 'Tariffa', larghezza: '140px', valore: (v) => v.codice, classe: () => 'mono' },
  { titolo: 'Descrizione', larghezza: 'minmax(240px, 1fr)', valore: (v) => v.descrizione_breve, suggerimento: true },
  { titolo: 'U.M.', larghezza: '64px', allinea: 'centro', valore: (v) => v.unita_misura },
  { titolo: 'Prezzo €', larghezza: '100px', allinea: 'destra', valore: (v) => numero(v.prezzo, 2) },
  { titolo: 'M.O.', larghezza: '76px', allinea: 'destra', valore: (v) => percento(v.incidenza_manodopera) },
  { titolo: 'Natura', larghezza: '84px', valore: (v) => natura(v.natura) },
  { titolo: 'Capitolo', larghezza: '100px', valore: (v) => v.capitolo ?? '', classe: () => 'mono' },
  { titolo: 'AP/NP', larghezza: '74px', valore: (v) => [v.nuovo_prezzo, v.analizzata && 'AP'].filter(Boolean).join(' ') },
];

const CAMPI_VOCE = [
  ['codice', 'Tariffa'],
  ['descrizione_breve', 'Descrizione breve'],
  ['descrizione_estesa', 'Descrizione estesa', 'textarea'],
  ['unita_misura', 'Unità di misura'],
  ['prezzo', 'Prezzo unitario (€)', 'numero'],
  ['natura', 'Natura', 'natura'],
  ['incidenza_manodopera', 'Incidenza manodopera (%)', 'numero'],
  ['incidenza_sicurezza', 'Incidenza sicurezza (%)', 'numero'],
];

function testoNumero(v) {
  return v === null || v === undefined ? '' : String(v).replace('.', ',');
}

let elVista = null;
let g = null;
let dettaglio = null;
let conteggio = null;
let codiciProgetto = new Set();
let barra = null; // indicatore del filtro attivo
let ultime = null; // ultima risposta del motore (voci prima dei filtri)
let contesto = null; // {progetto, computo}: categorie e WBS delle voci di computo
const filtri = {}; // un pannello per scheda (ADR 0020)

function filtriAperti() {
  return preferenza('filtri_elenco', false);
}

function pannello() {
  filtri[locale.scheda] ??= pannelloFiltri({ chiave: `elenco.${locale.scheda}`, suCambio: applicaFiltri });
  return filtri[locale.scheda];
}

export async function mostra(el, _parametri, { nuova }) {
  contesto = null; // i dati possono essere cambiati
  if (nuova || el !== elVista) costruisci(el);
  await carica();
}

function costruisci(el) {
  elVista = el;
  const cerca = h('input', { type: 'search', class: 'cerca', placeholder: 'Cerca codice o parole…', value: locale.testo, 'aria-label': 'Cerca nell\'elenco' });
  cerca.addEventListener('input', rimanda(() => { locale.testo = cerca.value; carica(); }, 180));
  const schede = h('div', { class: 'schede-tab', role: 'tablist' },
    ['progetto', 'consultazione'].map((s) => h('button', {
      role: 'tab', class: locale.scheda === s && 'corrente', 'aria-selected': locale.scheda === s ? 'true' : 'false',
      onclick: () => { locale.scheda = s; locale.codice = null; costruisci(el); carica(); },
    }, s === 'progetto' ? 'Elenco del progetto' : 'Prezzari di consultazione')));
  conteggio = h('span', { class: 'vuoto' });
  const progetto = locale.scheda === 'progetto';
  g = griglia({
    colonne: COLONNE,
    multipla: true,
    trascina: (voci) => (progetto
      ? { tipo: 'elenco', codici: voci.map((v) => v.codice) }
      : locale.prezzario && { tipo: 'prezzario', prezzario: locale.prezzario, codici: voci.map((v) => v.codice) }),
    rilascio: progetto ? rilascioNelProgetto : null,
    etichetta: progetto ? 'Elenco prezzi' : 'Prezzario di consultazione',
    vuoto: progetto ? 'Elenco prezzi vuoto: aggiungere una voce o copiarla da un prezzario.' : 'Caricare un prezzario di consultazione.',
    classeRiga: (v) => (!progetto && codiciProgetto.has(v.codice) ? 'presente' : null),
    suSelezione: (_, v) => { locale.codice = v?.codice ?? null; mostraDettaglio(); },
    suAttiva: (v) => (progetto ? aggiungiAlComputo([v.codice]) : copia([v])),
  });
  dettaglio = h('aside', { class: 'dettaglio', 'aria-label': 'Dettaglio voce' });

  const azioni = progetto
    ? [h('button', { class: 'pulsante primario', onclick: nuovaVoce }, '+ Nuova voce'),
      h('button', { class: 'pulsante', onclick: () => aggiungiAlComputo(g.selezionate().map((v) => v.codice)) }, 'Aggiungi al computo'),
      h('button', { class: 'pulsante', title: 'Confronta l\'elenco con un nuovo listino e aggiorna le voci scelte', onclick: () => vai('aggiorna') }, 'Aggiorna da listino…'),
      h('button', { class: 'pulsante pericolo', onclick: elimina }, 'Elimina')]
    : [selettorePrezzario(), h('button', { class: 'pulsante', onclick: caricaPrezzario }, 'Carica prezzario…'),
      h('button', { class: 'pulsante primario', onclick: () => copia(g.selezionate()) }, 'Copia nell\'elenco'),
      locale.prezzario && h('button', { class: 'pulsante', title: 'Confronta l\'elenco del progetto con questo prezzario e aggiorna le voci scelte', onclick: () => vai('aggiorna', { prezzario: locale.prezzario }) }, 'Aggiorna l\'elenco da questo…')];

  const n = pannello().attivi();
  const pulsanteFiltri = h('button', {
    class: ['pulsante', filtriAperti() && 'premuto', n && 'con-filtro'], 'aria-pressed': filtriAperti() ? 'true' : 'false',
    title: 'Filtra le voci per capitolo, natura, unità di misura, prezzo, incidenza della manodopera e uso nel computo (categorie e WBS)',
    onclick: () => { salvaPreferenza('filtri_elenco', !filtriAperti()); costruisci(el); carica(); },
  }, n ? `Filtri (${n})` : 'Filtri');
  barra = h('div', { class: 'barra-filtro', role: 'status', hidden: true });
  const dividi = h('div', { class: 'dividi' }, h('div', { class: 'zona' }, barra, g.el), dettaglio);
  sostituisci(el,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo), schede, cerca, conteggio, h('span', { class: 'spinge' }), pulsanteFiltri, azioni),
    h('div', { class: 'vista-corpo pieno' }, filtriAperti() ? h('div', { class: 'con-filtri' }, pannello().el, dividi) : dividi));
}

/** Categorie e WBS con cui ogni tariffa è usata nel computo corrente. */
async function caricaContesto() {
  if (contesto) return contesto;
  const nome = computoCorrente();
  const [prog, comp] = await Promise.all([
    chiedi({ richiesta: 'progetto' }),
    nome ? chiedi({ richiesta: 'computo', computo: nome }).catch(() => null) : null,
  ]);
  const uso = new Map(); // codice → {super_categoria:Set, …, wbs:Set}
  for (const v of comp?.voci ?? []) {
    if (!uso.has(v.codice)) uso.set(v.codice, { super_categoria: new Set(), categoria: new Set(), sotto_categoria: new Set(), wbs: new Set() });
    const u = uso.get(v.codice);
    for (const k of ['super_categoria', 'categoria', 'sotto_categoria']) u[k].add(v.categorie?.[k] ?? '');
    u.wbs.add(v.wbs ?? '');
  }
  contesto = { progetto: prog, computo: nome, uso };
  return contesto;
}

/** Sezioni del pannello dei filtri dell'elenco. */
function sezioniFiltro(r, cx) {
  const p = cx.progetto;
  const etichetta = (elenco, fmt = (c) => `${c.codice} — ${c.descrizione}`) => {
    const m = new Map(elenco.map((c) => [c.codice ?? c.codice_completo, fmt(c)]));
    return (k) => m.get(k) ?? k;
  };
  const usoDi = (v, k) => [...(cx.uso.get(v.codice)?.[k] ?? [''])];
  const nel = ' nel computo';
  const categoria = (id, titolo, elenco) => elenco.length && {
    id, titolo: `${titolo}${nel}`, tipo: 'scelte', valori: (v) => usoDi(v, id), etichetta: etichetta(elenco),
    ordine: elenco.map((c) => c.codice), vuoto: 'Non usata o non assegnata',
  };
  const tipi = { analisi: 'Con analisi', np: 'Nuovo prezzo', semplice: 'Senza analisi' };
  return [
    r.capitoli?.length && {
      id: 'capitolo', titolo: 'Capitolo', tipo: 'albero', valore: (v) => v.capitolo ?? '', vuoto: 'Senza capitolo',
      nodi: r.capitoli.map((c) => ({ codice: c.codice, padre: c.padre, descrizione: c.descrizione })),
    },
    { id: 'natura', titolo: 'Natura', tipo: 'scelte', valori: (v) => v.natura, etichetta: natura, ordine: ['lavoro', 'oneri_sicurezza'] },
    { id: 'unita', titolo: 'Unità di misura', tipo: 'scelte', valori: (v) => v.unita_misura || '', vuoto: 'Senza unità' },
    { id: 'prezzo', titolo: 'Prezzo unitario', tipo: 'intervallo', valore: (v) => v.prezzo, unita: '€' },
    { id: 'manodopera', titolo: 'Incidenza manodopera', tipo: 'intervallo', valore: (v) => v.incidenza_manodopera, unita: '%' },
    { id: 'tipo', titolo: 'Analisi e nuovi prezzi', tipo: 'scelte', ordine: Object.keys(tipi), etichetta: (k) => tipi[k] ?? k,
      valori: (v) => [v.analizzata && 'analisi', v.nuovo_prezzo && 'np'].filter(Boolean).concat(v.analizzata || v.nuovo_prezzo ? [] : ['semplice']) },
    cx.computo && { id: 'uso', titolo: `Uso nel computo «${cx.computo}»`, tipo: 'scelte', ordine: ['usata', 'non_usata'], etichetta: (k) => (k === 'usata' ? 'Usata nel computo' : 'Non usata'),
      valori: (v) => (cx.uso.has(v.codice) ? 'usata' : 'non_usata') },
    cx.computo && categoria('super_categoria', 'Super categorie', p.super_categorie),
    cx.computo && categoria('categoria', 'Categorie', p.categorie),
    cx.computo && categoria('sotto_categoria', 'Sotto categorie', p.sotto_categorie),
    cx.computo && p.wbs.length && {
      id: 'wbs', titolo: `WBS${nel}`, tipo: 'scelte', valori: (v) => usoDi(v, 'wbs'), vuoto: 'Non usata o non assegnata',
      ordine: p.wbs.map((n) => n.codice_completo), etichetta: etichetta(p.wbs, (n) => `${'\u00a0\u00a0'.repeat(n.livello)}${n.codice_completo} — ${n.descrizione}`),
    },
  ].filter(Boolean);
}

/** Applica i filtri all'ultima risposta del motore (senza nuove richieste). */
function applicaFiltri() {
  if (!ultime) return [];
  const f = pannello();
  const filtrate = f.filtra(ultime.voci);
  g.imposta(filtrate, (v) => v.codice);
  const attivi = f.attivi();
  conteggio.textContent = attivi || locale.testo ? `${filtrate.length} di ${ultime.totale}` : `${ultime.totale} voci`;
  barra.hidden = !attivi;
  if (attivi) {
    sostituisci(barra, h('span', null, `Filtro attivo: ${filtrate.length} di ${ultime.voci.length} voci`),
      h('button', { class: 'link', onclick: () => f.azzera() }, 'Azzera'),
      filtriAperti() ? null : h('button', { class: 'link', onclick: () => { salvaPreferenza('filtri_elenco', true); costruisci(elVista); carica(); } }, 'Mostra i filtri'));
  }
  const pulsante = elVista.querySelector('.vista-testata .pulsante[aria-pressed]');
  if (pulsante) { pulsante.textContent = attivi ? `Filtri (${attivi})` : 'Filtri'; pulsante.classList.toggle('con-filtro', !!attivi); }
  return filtrate;
}

function selettorePrezzario() {
  const caricati = stato.info?.prezzari_caricati ?? [];
  if (!caricati.some((p) => p.nome === locale.prezzario)) locale.prezzario = caricati[0]?.nome ?? null;
  if (!caricati.length) return null;
  const s = h('select', { 'aria-label': 'Prezzario', style: { width: 'auto' } }, caricati.map((p) => h('option', { value: p.nome, selected: p.nome === locale.prezzario }, `${p.nome} (${p.voci})`)));
  s.addEventListener('change', () => { locale.prezzario = s.value; carica(); });
  return s;
}

async function carica() {
  try {
    const progetto = locale.scheda === 'progetto';
    if (!progetto) {
      const tutti = await chiedi({ richiesta: 'elenco_prezzi' });
      codiciProgetto = new Set(tutti.voci.map((v) => v.codice));
      if (!locale.prezzario) { ultime = null; g.imposta([]); conteggio.textContent = ''; mostraDettaglio(); return; }
    }
    const [r, cx] = await Promise.all([
      chiedi({ richiesta: 'elenco_prezzi', prezzario: progetto ? null : locale.prezzario, testo: locale.testo }),
      caricaContesto(),
    ]);
    ultime = r;
    pannello().imposta(r.voci, sezioniFiltro(r, cx));
    const filtrate = applicaFiltri();
    if (!g.attiva() && locale.codice) {
      const i = filtrate.findIndex((v) => v.codice === locale.codice);
      if (i >= 0) g.seleziona(i); else locale.codice = null;
    }
    mostraDettaglio();
  } catch (e) {
    mostraErrore(e);
  }
}

async function mostraDettaglio() {
  const codice = locale.codice;
  if (!codice) {
    sostituisci(dettaglio, h('p', { class: 'vuoto' }, locale.scheda === 'progetto'
      ? 'Selezionare una voce per vederla e modificarla. Le voci selezionate (Maiusc o Ctrl per più voci) si trascinano in un computo, anche aperto in un\'altra finestra.'
      : 'Selezionare una o più voci (Maiusc o Ctrl per la selezione multipla) e copiarle nell\'elenco del progetto, oppure trascinarle in un computo o nell\'elenco del progetto aperto in un\'altra finestra.'));
    return;
  }
  const progetto = locale.scheda === 'progetto';
  let v;
  try {
    v = await chiedi({ richiesta: 'voce_prezzario', codice, prezzario: progetto ? null : locale.prezzario });
  } catch (e) { mostraErrore(e); return; }
  if (codice !== locale.codice) return;
  if (!progetto) {
    sostituisci(dettaglio,
      h('h3', null, v.codice), h('p', null, h('strong', null, v.descrizione_breve)),
      h('div', { class: 'descrizione' }, v.descrizione_estesa),
      h('h3', null, 'Prezzo'), h('p', null, `${numero(v.prezzo, 2)} € / ${v.unita_misura} — ${natura(v.natura)}`),
      v.incidenza_manodopera && h('p', null, `Manodopera ${percento(v.incidenza_manodopera)}`),
      codiciProgetto.has(v.codice) && h('p', { class: 'etichetta neutra' }, 'già nell\'elenco del progetto'));
    return;
  }
  const controlli = {};
  const modulo = h('div', { class: 'modulo' }, CAMPI_VOCE.map(([k, etichetta, tipo]) => {
    const valore = tipo === 'numero' ? testoNumero(v[k]) : v[k] ?? '';
    controlli[k] = tipo === 'natura'
      ? controllo({ tipo: 'select', nome: k, valore: v.natura, opzioni: [['lavoro', 'Lavoro'], ['oneri_sicurezza', 'Oneri della sicurezza']] })
      : controllo({ tipo: tipo === 'textarea' ? 'textarea' : 'text', nome: k, valore, righe: 7, numerico: tipo === 'numero' });
    return h('label', { class: 'campo' }, h('span', null, etichetta), controlli[k]);
  }));
  const applica = async () => {
    const c = { comando: 'modifica_voce_prezzario', codice: v.codice };
    try {
      for (const [k, , tipo] of CAMPI_VOCE) {
        const nuovo = controlli[k].value;
        const vecchio = tipo === 'numero' ? testoNumero(v[k]) : v[k] ?? '';
        if (String(nuovo).trim() === String(vecchio).trim()) continue;
        if (k === 'codice') c.nuovo_codice = nuovo.trim();
        else if (tipo === 'numero') c[k] = decimale(nuovo) ?? (k === 'prezzo' ? '0' : '0');
        else c[k] = k === 'descrizione_estesa' ? nuovo : nuovo.trim();
      }
    } catch (e) { mostraErrore(e); return; }
    if (Object.keys(c).length === 2) { notifica('Nessuna modifica', 'info', null, 1500); return; }
    if (c.nuovo_codice) locale.codice = c.nuovo_codice;
    const esito = await esegui([c]);
    if (esito) notifica('Voce aggiornata. Le voci di computo conservano il prezzo fotografato: riallinearle se serve.', 'ok');
  };
  const r = v.analisi?.risultato;
  sostituisci(dettaglio, h('h3', null, 'Voce'), modulo,
    v.capitolo && h('p', { class: 'vuoto' }, `Capitolo ${v.capitolo}`),
    v.fonte && h('p', { class: 'vuoto' }, `Origine: ${v.fonte}`),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '12px' } },
      h('button', { class: 'pulsante primario', onclick: applica }, 'Applica modifiche'),
      h('button', { class: 'pulsante', onclick: mostraDettaglio }, 'Annulla')),
    h('h3', null, 'Analisi del prezzo'),
    v.analisi
      ? h('p', null, r ? `${v.analisi.righe.length} elementi, prezzo da analisi ${numero(r.prezzo_unitario, 2)} €` : 'Analisi non calcolabile',
        v.analisi.allineata ? null : h('span', { class: 'etichetta avviso', style: { marginLeft: '6px' } }, 'da aggiornare'))
      : h('p', { class: 'vuoto' }, 'Prezzo senza analisi.'),
    h('button', { class: 'pulsante', onclick: () => vai('analisi', { voce: v.codice }) }, v.analisi ? 'Apri l\'analisi' : 'Crea l\'analisi'),
    h('h3', null, 'Nuovo prezzo'),
    v.nuovo_prezzo
      ? h('p', null, h('span', { class: 'etichetta neutra' }, `NP.${v.nuovo_prezzo.numero}`), ` del ${v.nuovo_prezzo.data.split('-').reverse().join('/')}${v.nuovo_prezzo.soggetto_a_ribasso ? '' : ', già netto (escluso dal ribasso)'}`)
      : h('p', { class: 'vuoto' }, 'Voce contrattuale.'),
    h('button', { class: 'pulsante', onclick: () => vai('nuovi', v.nuovo_prezzo ? {} : { voce: v.codice }) }, v.nuovo_prezzo ? 'Nuovi prezzi' : 'Segna come nuovo prezzo…'));
}

async function nuovaVoce() {
  const v = await chiediCampi('Nuova voce di elenco prezzi', [
    { nome: 'codice', etichetta: 'Tariffa', obbligatorio: true },
    { nome: 'descrizione_breve', etichetta: 'Descrizione breve', obbligatorio: true },
    { nome: 'descrizione_estesa', etichetta: 'Descrizione estesa', tipo: 'textarea' },
    { nome: 'unita_misura', etichetta: 'Unità di misura', obbligatorio: true, segnaposto: 'm³, m², kg, cad…' },
    { nome: 'prezzo', etichetta: 'Prezzo unitario (€)', obbligatorio: true, numerico: true },
    { nome: 'natura', etichetta: 'Natura', tipo: 'select', valore: 'lavoro', opzioni: [['lavoro', 'Lavoro'], ['oneri_sicurezza', 'Oneri della sicurezza']] },
    { nome: 'incidenza_manodopera', etichetta: 'Incidenza manodopera (%)', numerico: true },
  ], {
    ok: 'Aggiungi',
    convalida: (x) => { try { decimale(x.prezzo); decimale(x.incidenza_manodopera); return null; } catch (e) { return e.message; } },
  });
  if (!v) return;
  const c = {
    comando: 'aggiungi_voce_prezzario', codice: v.codice.trim(), descrizione_breve: v.descrizione_breve.trim(),
    unita_misura: v.unita_misura.trim(), prezzo: decimale(v.prezzo), natura: v.natura,
  };
  if (opzionale(v.descrizione_estesa)) c.descrizione_estesa = v.descrizione_estesa;
  if (decimale(v.incidenza_manodopera)) c.incidenza_manodopera = decimale(v.incidenza_manodopera);
  locale.codice = c.codice;
  locale.testo = '';
  await esegui([c]);
  costruisci(elVista);
  await carica();
}

async function elimina() {
  if (!locale.codice) return;
  if (!(await conferma('Elimina voce', `Eliminare la voce «${locale.codice}» dall'elenco prezzi? Le voci di computo che la usano restano con il prezzo fotografato.`, { ok: 'Elimina', pericolo: true }))) return;
  const codice = locale.codice;
  locale.codice = null;
  await esegui([{ comando: 'rimuovi_voce_prezzario', codice }]);
}

async function aggiungiAlComputo(codici) {
  const computo = computoCorrente();
  if (!computo) return;
  if (!codici.length) { notifica('Selezionare le voci da aggiungere', 'info', null, 1500); return; }
  const esito = await esegui(codici.map((codice) => ({ comando: 'aggiungi_voce_computo', computo, codice, righi: [{}] })));
  const testo = codici.length === 1 ? `Voce «${codici[0]}» aggiunta` : `${codici.length} voci aggiunte`;
  if (esito) notifica(`${testo} a «${computo}»`, 'ok', { testo: 'Vai al computo', fn: () => vai('computo', { computo, ultima: true }) });
}

async function caricaPrezzario() {
  const percorso = await scegliFile('Carica prezzario di consultazione', [{ name: 'Prezzari (DCF, XPWE, OpenComputo)', extensions: ['dcf', 'DCF', 'xpwe', 'XPWE', 'ocp', 'json'] }]);
  if (!percorso) return;
  const base = percorso.split(/[\\/]/).pop().replace(/\.[^.]+$/, '');
  const r = await agisci({ richiesta: 'carica_prezzario', nome: base, percorso });
  if (!r) return;
  notifica(`Caricato «${r.nome}»: ${r.voci} voci${r.anomalie ? `, ${r.anomalie} segnalazioni` : ''}`, 'ok');
  locale.prezzario = r.nome;
  locale.scheda = 'consultazione';
  costruisci(elVista);
  await carica();
}

async function copia(voci) {
  if (!locale.prezzario || !voci.length) { notifica('Selezionare le voci da copiare', 'info', null, 2000); return; }
  await copiaCodici(locale.prezzario, voci.map((v) => v.codice), codiciProgetto);
}

async function copiaCodici(prezzario, codici, presenti) {
  const nuove = codici.filter((k) => !presenti.has(k));
  const gia = codici.length - nuove.length;
  if (!nuove.length) { notifica('Le voci selezionate sono già nell\'elenco del progetto', 'info'); return; }
  const esito = await esegui([{ comando: 'copia_da_prezzario', prezzario, codici: nuove }]);
  if (esito) notifica(`${nuove.length} voci copiate nell'elenco prezzi${gia ? ` (${gia} già presenti)` : ''}`, 'ok');
}

/** Voci di un prezzario di consultazione rilasciate sull'elenco del progetto. */
async function rilascioNelProgetto(e) {
  const d = await trascina.leggi(e);
  if (!d) return;
  if (d.tipo !== 'prezzario') {
    if (d.tipo === 'computo') notifica('Le voci di computo sono già nell\'elenco prezzi: trascinarle in un computo', 'info', null, 2500);
    return;
  }
  const tutti = await chiedi({ richiesta: 'elenco_prezzi' });
  await copiaCodici(d.prezzario, d.codici, new Set(tutti.voci.map((v) => v.codice)));
}
