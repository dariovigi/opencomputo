// Aggiornamento dell'elenco prezzi da un nuovo listino (ADR 0022): confronto completo
// voce per voce con un prezzario caricato, scelta delle voci e dei campi, effetto sui
// computi calcolato dal motore (prova), poi un solo comando annullabile.

import { h, sostituisci, numero, percento, notifica, rimanda, preferenza, salvaPreferenza, sommaImporti } from '../dom.js';
import { chiedi, esegui, agisci, stato, mostraErrore, vai } from '../stato.js';
import { scegliFile } from '../piattaforma.js';
import { griglia } from '../griglia.js';
import { pannelloFiltri } from '../filtri.js';

export const titolo = 'Aggiornamento da listino';

const CAMPI = [
  ['prezzo', 'Prezzo'],
  ['incidenze', 'Incidenze (manodopera, sicurezza)'],
  ['descrizioni', 'Descrizioni'],
  ['unita_misura', 'Unità di misura'],
  ['codice', 'Tariffa (con la sostituzione del prefisso)'],
];

/** Campo del confronto → campo da aggiornare. */
const CAMPO_DI = {
  prezzo: 'prezzo',
  incidenza_manodopera: 'incidenze',
  incidenza_sicurezza: 'incidenze',
  descrizione_breve: 'descrizioni',
  descrizione_estesa: 'descrizioni',
  unita_misura: 'unita_misura',
  codice: 'codice',
};

const NOMI_CAMPO = {
  codice: 'Tariffa',
  descrizione_breve: 'Descrizione breve',
  descrizione_estesa: 'Descrizione estesa',
  unita_misura: 'Unità di misura',
  prezzo: 'Prezzo',
  incidenza_manodopera: 'Incidenza manodopera %',
  incidenza_sicurezza: 'Incidenza sicurezza %',
};

const SIGLE = { prezzo: 'P', incidenze: 'I', descrizioni: 'D', unita_misura: 'UM', codice: 'T' };

const ESITI = { variata: 'Variata', invariata: 'Invariata', non_trovata: 'Non trovata', analizzata: 'Con analisi' };

const locale = {
  prezzario: null,
  scheda: 'voci',
  da: '', a: '',
  campi: { prezzo: true, incidenze: true, descrizioni: false, unita_misura: false, codice: false, ...preferenza('aggiorna.campi', {}) },
  ricalcola: true,
  riallinea: true,
};

let elVista = null;
let dati = null; // risposta di confronto_prezzario
let chiaveDati = null; // prezzario e prefisso dei dati
const scelte = { voci: new Set(), risorse: new Set() };
let g = null;
let dettaglio = null;
let riepilogo = null;
let conteggio = null;
const filtri = {
  voci: pannelloFiltri({ chiave: 'aggiorna.voci', suCambio: () => applica() }),
  risorse: pannelloFiltri({ chiave: 'aggiorna.risorse', suCambio: () => applica() }),
};

function prefisso() {
  return locale.da ? { da: locale.da, a: locale.a } : null;
}

/** Il campo `d.campo` del confronto è tra quelli scelti. */
function campoScelto(campo) {
  const k = CAMPO_DI[campo];
  return !!k && locale.campi[k] && (k !== 'codice' || !!locale.da);
}

/** La riga ha differenze nei campi scelti (aggiornarla cambia qualcosa). */
function rilevante(r) {
  return r.esito === 'variata' && r.differenze.some((d) => campoScelto(d.campo));
}

function aggiornabile(r) {
  return r.esito === 'variata' || r.esito === 'invariata';
}

function campiDiversi(r) {
  return [...new Set(r.differenze.map((d) => CAMPO_DI[d.campo]).filter(Boolean))];
}

export async function mostra(el, parametri, { nuova }) {
  if (parametri.prezzario) { locale.prezzario = parametri.prezzario; delete parametri.prezzario; }
  const caricati = stato.info?.prezzari_caricati ?? [];
  if (!caricati.some((p) => p.nome === locale.prezzario)) locale.prezzario = caricati[0]?.nome ?? null;
  if (nuova || el !== elVista) costruisci(el);
  await carica(true);
}

function costruisci(el) {
  elVista = el;
  const caricati = stato.info?.prezzari_caricati ?? [];
  const sel = caricati.length
    ? h('select', { name: 'listino', 'aria-label': 'Listino', onchange: (e) => { locale.prezzario = e.target.value; carica(true); } },
      caricati.map((p) => h('option', { value: p.nome, selected: p.nome === locale.prezzario }, `${p.nome} (${p.voci} voci)`)))
    : h('span', { class: 'vuoto' }, 'Nessun listino caricato');
  const testoPrefisso = (nome, chiave, segnaposto) => {
    const i = h('input', { type: 'text', name: nome, value: locale[chiave], placeholder: segnaposto, class: 'mono corto', 'aria-label': nome === 'prefisso-da' ? 'Prefisso nell\'elenco' : 'Prefisso nel listino' });
    i.addEventListener('input', rimanda(() => { locale[chiave] = i.value.trim(); costruisciOpzioni(); carica(true); }, 400));
    return i;
  };
  const schede = h('div', { class: 'schede-tab', role: 'tablist' },
    [['voci', 'Voci dell\'elenco'], ['risorse', 'Risorse delle analisi']].map(([k, t]) => h('button', {
      role: 'tab', name: `scheda-${k}`, class: locale.scheda === k && 'corrente', 'aria-selected': locale.scheda === k ? 'true' : 'false',
      onclick: () => { locale.scheda = k; costruisci(el); carica(false); },
    }, t)));
  conteggio = h('span', { class: 'vuoto' });
  g = griglia({
    colonne: locale.scheda === 'voci' ? colonneVoci() : colonneRisorse(),
    multipla: true,
    etichetta: locale.scheda === 'voci' ? 'Confronto delle voci' : 'Confronto delle risorse',
    vuoto: locale.prezzario ? 'Nessuna voce corrisponde ai filtri.' : 'Caricare il nuovo listino (DCF, XPWE o progetto OpenComputo).',
    classeRiga: (r) => `esito-${r.esito}`,
    spunte: {
      etichetta: 'Da aggiornare',
      scelta: (r) => scelte[locale.scheda].has(r.codice),
      abilitata: aggiornabile,
      alterna: (righe, valore) => {
        for (const r of righe) { if (valore) scelte[locale.scheda].add(r.codice); else scelte[locale.scheda].delete(r.codice); }
        aggiornaRiepilogo();
      },
    },
    suSelezione: (_, r) => mostraDettaglio(r),
  });
  dettaglio = h('aside', { class: 'dettaglio', 'aria-label': 'Confronto della voce' });
  riepilogo = h('div', { class: 'riepilogo-aggiornamento', role: 'status' });
  const opzioni = h('div', { class: 'opzioni-aggiornamento' });
  const zona = h('div', { class: 'zona' }, opzioni, g.el, riepilogo);
  sostituisci(el,
    h('div', { class: 'vista-testata' },
      h('h2', null, titolo),
      h('label', { class: 'campo-riga' }, 'Listino', sel),
      h('button', { class: 'pulsante', onclick: caricaListino }, 'Carica listino…'),
      h('label', { class: 'campo-riga', title: 'Per i prezzari che cambiano il prefisso dei codici a ogni edizione, es. TOS23_ → TOS24_' },
        'Prefisso', testoPrefisso('prefisso-da', 'da', 'elenco'), '→', testoPrefisso('prefisso-a', 'a', 'listino')),
      h('span', { class: 'spinge' }),
      h('button', { class: 'pulsante', onclick: () => vai('elenco') }, 'Elenco prezzi')),
    h('div', { class: 'vista-corpo pieno' },
      h('div', { class: 'con-filtri' }, filtri[locale.scheda].el,
        h('div', { class: 'dividi' }, h('div', { class: 'zona' }, h('div', { class: 'barra-schede' }, schede, conteggio), zona), dettaglio))));
  costruisciOpzioni();
}

function costruisciOpzioni() {
  const opzioni = elVista?.querySelector('.opzioni-aggiornamento');
  if (!opzioni) return;
  const spunta = (nome, testo, valore, cambia, disabilitata = false, titolo = null) => h('label', { class: 'campo-riga', title: titolo },
    h('input', { type: 'checkbox', name: nome, checked: valore && !disabilitata, disabled: disabilitata, onchange: (e) => cambia(e.target.checked) }), testo);
  sostituisci(opzioni,
    h('strong', null, 'Aggiorna:'),
    CAMPI.filter(([k]) => locale.scheda === 'voci' || k !== 'incidenze').map(([k, t]) => spunta(`campo-${k}`, t, locale.campi[k], (v) => {
      locale.campi[k] = v;
      salvaPreferenza('aggiorna.campi', locale.campi);
      sceltePredefinite();
      applica();
    }, k === 'codice' && !locale.da)),
    h('span', { class: 'separatore' }),
    spunta('ricalcola-analisi', 'Ricalcola le analisi', locale.ricalcola, (v) => { locale.ricalcola = v; aggiornaRiepilogo(); }, false,
      'Le voci con analisi seguono i nuovi prezzi delle risorse'),
    spunta('riallinea-computi', 'Riallinea i computi', locale.riallinea, (v) => { locale.riallinea = v; aggiornaRiepilogo(); }, false,
      'Le voci dei computi collegate alle voci aggiornate prendono i nuovi dati'));
}

function colonneVoci() {
  return [
    { titolo: 'Tariffa', larghezza: '104px', valore: (v) => v.codice, classe: () => 'mono' },
    { titolo: 'Descrizione', larghezza: 'minmax(120px, 1fr)', valore: (v) => v.descrizione_breve, suggerimento: true },
    { titolo: 'U.M.', larghezza: '44px', allinea: 'centro', valore: (v) => v.unita_misura },
    { titolo: 'Prezzo €', larghezza: '74px', allinea: 'destra', valore: (v) => numero(v.prezzo, 2) },
    { titolo: 'Listino €', larghezza: '74px', allinea: 'destra', valore: (v) => numero(v.prezzo_listino, 2) },
    { titolo: 'Var. %', larghezza: '60px', allinea: 'destra', valore: (v) => variazione(v.variazione), classe: (v) => segno(v.variazione) },
    { titolo: 'Diff.', larghezza: '44px', valore: (v) => campiDiversi(v).map((k) => SIGLE[k]).join(' '), suggerimento: true },
    { titolo: 'Δ computi €', larghezza: '92px', allinea: 'destra', valore: effettoComputi, classe: (v) => segno(deltaComputi(v)) },
    { titolo: 'Esito', larghezza: '92px', valore: (v) => ESITI[v.esito] },
  ];
}

function colonneRisorse() {
  return [
    { titolo: 'Codice', larghezza: '96px', valore: (r) => r.codice, classe: () => 'mono' },
    { titolo: 'Descrizione', larghezza: 'minmax(120px, 1fr)', valore: (r) => r.descrizione, suggerimento: true },
    { titolo: 'Tipo', larghezza: '84px', valore: (r) => r.tipo },
    { titolo: 'U.M.', larghezza: '44px', allinea: 'centro', valore: (r) => r.unita_misura },
    { titolo: 'Prezzo €', larghezza: '74px', allinea: 'destra', valore: (r) => numero(r.prezzo, 2) },
    { titolo: 'Listino €', larghezza: '74px', allinea: 'destra', valore: (r) => numero(r.prezzo_listino, 2) },
    { titolo: 'Var. %', larghezza: '60px', allinea: 'destra', valore: (r) => variazione(r.variazione), classe: (r) => segno(r.variazione) },
    { titolo: 'Diff.', larghezza: '44px', valore: (r) => campiDiversi(r).map((k) => SIGLE[k]).join(' ') },
    { titolo: 'Analisi', larghezza: '58px', allinea: 'destra', valore: (r) => (r.analisi ? String(r.analisi) : '') },
    { titolo: 'Esito', larghezza: '92px', valore: (r) => ESITI[r.esito] },
  ];
}

/** Differenza degli importi nei computi con il prezzo del listino (decimale esatto). */
function deltaComputi(v) {
  if (!v.voci_computo || v.importo_computi_listino === null) return null;
  return sommaImporti([v.importo_computi_listino, opposto(v.importo_computi)]);
}

function effettoComputi(v) {
  const d = deltaComputi(v);
  return d === null ? '' : variazione(d);
}

function opposto(x) {
  return x.startsWith('-') ? x.slice(1) : `-${x}`;
}

function variazione(v) {
  if (v === null || v === undefined) return '';
  const t = numero(v, 2);
  return v.startsWith('-') || /^0(\.0*)?$/.test(v) ? t : `+${t}`;
}

function segno(v) {
  if (!v || /^-?0(\.0*)?$/.test(v)) return null;
  return v.startsWith('-') ? 'diminuzione' : 'aumento';
}

/** Sezioni del pannello dei filtri. */
function sezioni(righe) {
  const comuni = [
    { id: 'esito', titolo: 'Esito', tipo: 'scelte', valori: (r) => r.esito, etichetta: (k) => ESITI[k] ?? k, ordine: Object.keys(ESITI) },
    { id: 'campi', titolo: 'Dati diversi', tipo: 'scelte', valori: (r) => campiDiversi(r), etichetta: (k) => CAMPI.find(([c]) => c === k)?.[1] ?? k, vuoto: 'Nessuno' },
    { id: 'scelta', titolo: 'Da aggiornare', tipo: 'scelte', valori: (r) => (scelte[locale.scheda].has(r.codice) ? 'si' : 'no'), etichetta: (k) => (k === 'si' ? 'Scelte' : 'Non scelte') },
    { id: 'variazione', titolo: 'Variazione del prezzo', tipo: 'intervallo', valore: (r) => r.variazione, unita: '%' },
    { id: 'testo', titolo: 'Tariffa o descrizione', tipo: 'testo', valore: (r) => `${r.codice} ${r.descrizione_breve ?? r.descrizione}`, segnaposto: 'Cerca…' },
  ];
  if (locale.scheda === 'voci') {
    comuni.push(
      { id: 'capitolo', titolo: 'Capitolo', tipo: 'scelte', valori: (v) => v.capitolo ?? '', vuoto: 'senza capitolo' },
      { id: 'uso', titolo: 'Uso nei computi', tipo: 'scelte', valori: (v) => (v.voci_computo ? 'usata' : 'non_usata'), etichetta: (k) => (k === 'usata' ? 'Usata' : 'Non usata') },
      { id: 'prezzo', titolo: 'Prezzo attuale', tipo: 'intervallo', valore: (v) => v.prezzo, unita: '€' },
    );
  } else {
    comuni.push({ id: 'tipo', titolo: 'Tipo', tipo: 'scelte', valori: (r) => r.tipo });
  }
  return comuni.filter((s) => s.id !== 'capitolo' || righe.some((v) => v.capitolo));
}

async function caricaListino() {
  const percorso = await scegliFile('Carica il nuovo listino', [{ name: 'Prezzari (DCF, XPWE, OpenComputo)', extensions: ['dcf', 'DCF', 'xpwe', 'XPWE', 'ocp', 'json'] }]);
  if (!percorso) return;
  const base = percorso.split(/[\\/]/).pop().replace(/\.[^.]+$/, '');
  const r = await agisci({ richiesta: 'carica_prezzario', nome: base, percorso });
  if (!r) return;
  notifica(`Caricato «${r.nome}»: ${r.voci} voci${r.anomalie ? `, ${r.anomalie} segnalazioni` : ''}`, 'ok');
  locale.prezzario = r.nome;
  costruisci(elVista);
  await carica(true);
}

/** Scelte iniziali: le righe con differenze nei campi da aggiornare. */
function sceltePredefinite() {
  if (!dati) return;
  scelte.voci = new Set(dati.voci.filter(rilevante).map((v) => v.codice));
  scelte.risorse = new Set(dati.risorse.filter(rilevante).map((r) => r.codice));
}

/** Rilegge il confronto dal motore; `daCapo` ricalcola anche le scelte iniziali. */
async function carica(daCapo) {
  if (!locale.prezzario) { dati = null; chiaveDati = null; applica(); return; }
  const chiave = JSON.stringify([locale.prezzario, prefisso()]);
  try {
    if (daCapo || chiave !== chiaveDati || !dati) {
      const precedenti = chiave === chiaveDati ? scelte : null;
      dati = await chiedi({ richiesta: 'confronto_prezzario', prezzario: locale.prezzario, prefisso: prefisso() });
      const nuova = chiave !== chiaveDati;
      chiaveDati = chiave;
      if (nuova || !precedenti) sceltePredefinite();
      else {
        // Dopo un aggiornamento le scelte restano solo sulle righe ancora da aggiornare.
        const ancora = (righe, set) => new Set(righe.filter((r) => set.has(r.codice) && rilevante(r)).map((r) => r.codice));
        scelte.voci = ancora(dati.voci, scelte.voci);
        scelte.risorse = ancora(dati.risorse, scelte.risorse);
      }
    }
  } catch (e) {
    mostraErrore(e);
    dati = null;
  }
  applica();
}

function righe() {
  if (!dati) return [];
  return locale.scheda === 'voci' ? dati.voci : dati.risorse;
}

function applica() {
  if (!g) return;
  const tutte = righe();
  const f = filtri[locale.scheda];
  f.imposta(tutte, sezioni(tutte));
  const visibili = f.filtra(tutte);
  g.imposta(visibili, (r) => r.codice);
  if (conteggio) conteggio.textContent = dati ? (f.attivi() ? `${visibili.length} di ${tutte.length}` : `${tutte.length}`) : '';
  mostraDettaglio(g.attiva());
  aggiornaRiepilogo();
}

function mostraDettaglio(r) {
  if (!dettaglio) return;
  if (!r) {
    sostituisci(dettaglio, dati
      ? [h('h3', null, 'Listino'), h('p', null, dati.fonte), h('p', { class: 'vuoto' }, `${dati.voci_listino} voci nel listino, ${dati.voci_solo_nel_listino} senza corrispondenza nell'elenco.`),
        dati.codici_ripetuti.length ? h('p', { class: 'avviso' }, `Codici ripetuti nel listino (vale la prima voce): ${dati.codici_ripetuti.slice(0, 10).join(', ')}${dati.codici_ripetuti.length > 10 ? '…' : ''}`) : null,
        dati.computi_non_calcolabili.length ? h('p', { class: 'avviso' }, `Computi non calcolabili, esclusi dagli importi: ${dati.computi_non_calcolabili.join(', ')}`) : null,
        h('p', { class: 'vuoto' }, 'Selezionare una riga per il confronto dei dati.')]
      : h('p', { class: 'vuoto' }, 'Caricare il nuovo listino e scegliere le voci da aggiornare.'));
    return;
  }
  const differenze = r.differenze;
  sostituisci(dettaglio,
    h('h3', null, `${r.codice}${r.codice_listino !== r.codice ? ` → ${r.codice_listino}` : ''}`),
    h('p', null, r.descrizione_breve ?? r.descrizione),
    h('p', { class: 'vuoto' }, {
      variata: 'Dati diversi nel listino:',
      invariata: 'Stessi dati nel listino.',
      non_trovata: `Tariffa «${r.codice_listino}» non presente nel listino.`,
      analizzata: 'Voce con analisi: il prezzo segue l\'analisi. Aggiornare le risorse dalla scheda «Risorse delle analisi».',
    }[r.esito]),
    differenze.length ? h('table', { class: 'confronto-campi' },
      h('thead', null, h('tr', null, h('th', null, 'Dato'), h('th', null, 'Elenco'), h('th', null, 'Listino'))),
      h('tbody', null, differenze.map((d) => h('tr', { class: campoScelto(d.campo) ? null : 'non-aggiornato', title: campoScelto(d.campo) ? null : 'Campo non scelto per l\'aggiornamento' },
        h('th', null, NOMI_CAMPO[d.campo] ?? d.campo),
        h('td', null, formatta(d.campo, d.applicato)),
        h('td', null, formatta(d.campo, d.prezzario)))))) : null,
    r.voci_computo ? [h('h3', null, 'Nei computi'),
      h('p', null, `${r.voci_computo} ${r.voci_computo === 1 ? 'voce' : 'voci'}: ${numero(r.importo_computi, 2)} € oggi`,
        r.importo_computi_listino !== null ? `, ${numero(r.importo_computi_listino, 2)} € con il prezzo del listino (${differenza(r.importo_computi_listino, r.importo_computi)} €)` : '')] : null,
    r.analisi ? h('p', null, `Usata in ${r.analisi} ${r.analisi === 1 ? 'analisi' : 'analisi'}.`) : null);
}

function formatta(campo, valore) {
  if (campo === 'prezzo') return numero(valore, 2);
  if (campo.startsWith('incidenza')) return valore ? percento(valore) : '—';
  return valore || '—';
}

function differenza(a, b) {
  const d = sommaImporti([a, opposto(b)]);
  return d.startsWith('-') ? numero(d, 2) : `+${numero(d, 2)}`;
}

/** Comando di aggiornamento per le scelte correnti, o null se non c'è nulla da fare. */
function comando() {
  if (!dati || !locale.prezzario) return null;
  const voci = dati.voci.filter((v) => scelte.voci.has(v.codice) && aggiornabile(v)).map((v) => v.codice);
  const risorse = dati.risorse.filter((r) => scelte.risorse.has(r.codice) && aggiornabile(r)).map((r) => r.codice);
  if (!voci.length && !risorse.length) return null;
  const campi = { ...locale.campi, codice: locale.campi.codice && !!locale.da };
  if (!Object.values(campi).some(Boolean)) return null;
  return {
    comando: 'aggiorna_da_prezzario', prezzario: locale.prezzario, voci, risorse, campi,
    prefisso: prefisso(), ricalcola_analisi: locale.ricalcola, riallinea_computi: locale.riallinea,
  };
}

let provaInCorso = 0;
const provaRimandata = rimanda(async () => {
  const c = comando();
  const n = ++provaInCorso;
  const effetto = riepilogo?.querySelector('.effetto');
  if (!effetto) return;
  if (!c) { sostituisci(effetto); return; }
  try {
    const esito = await chiedi({ richiesta: 'prova', comandi: [c] });
    if (n !== provaInCorso) return;
    sostituisci(effetto, esito.totali.length
      ? esito.totali.map((t) => h('span', { class: 'totale-computo' }, `${t.computo}: ${numero(t.prima, 2)} → `, h('strong', null, numero(t.dopo, 2)),
        t.prima !== null && t.dopo !== null ? ` (${differenza(t.dopo, t.prima)} €)` : ''))
      : h('span', { class: 'vuoto' }, locale.riallinea ? 'Nessun totale dei computi cambia.' : 'I computi non cambiano (riallineamento escluso).'));
  } catch (e) {
    if (n === provaInCorso) sostituisci(effetto, h('span', { class: 'errore' }, e.message));
  }
}, 250);

function aggiornaRiepilogo() {
  if (!riepilogo) return;
  const nv = dati ? dati.voci.filter((v) => scelte.voci.has(v.codice) && aggiornabile(v)).length : 0;
  const nr = dati ? dati.risorse.filter((r) => scelte.risorse.has(r.codice) && aggiornabile(r)).length : 0;
  const visibili = () => filtri[locale.scheda].filtra(righe()).filter(aggiornabile);
  const tutte = (valore) => () => {
    for (const r of visibili()) { if (valore) scelte[locale.scheda].add(r.codice); else scelte[locale.scheda].delete(r.codice); }
    g.ridisegna();
    aggiornaRiepilogo();
  };
  const variate = () => {
    for (const r of visibili()) { if (rilevante(r)) scelte[locale.scheda].add(r.codice); else scelte[locale.scheda].delete(r.codice); }
    g.ridisegna();
    aggiornaRiepilogo();
  };
  const c = comando();
  sostituisci(riepilogo,
    h('div', { class: 'scelta-rapida' },
      h('button', { class: 'link', name: 'scegli-visibili', onclick: tutte(true) }, 'Scegli le visibili'),
      h('button', { class: 'link', name: 'scegli-variate', onclick: variate }, 'Solo le variate'),
      h('button', { class: 'link', name: 'scegli-nessuna', onclick: tutte(false) }, 'Nessuna')),
    h('span', { class: 'conteggio-scelte' }, `Scelte: ${nv} ${nv === 1 ? 'voce' : 'voci'}, ${nr} ${nr === 1 ? 'risorsa' : 'risorse'}`),
    h('span', { class: 'effetto' }),
    h('span', { class: 'spinge' }),
    h('button', { class: 'pulsante primario', name: 'aggiorna-scelte', disabled: !c, onclick: aggiornaScelte }, 'Aggiorna le voci scelte'));
  provaRimandata();
}

async function aggiornaScelte() {
  const c = comando();
  if (!c) return;
  const esito = await esegui([c]);
  if (!esito) return;
  notifica(`${esito.comandi[0].descrizione[0].toUpperCase()}${esito.comandi[0].descrizione.slice(1)}`, 'ok', { testo: 'Elenco prezzi', fn: () => vai('elenco') }, 8000);
}
