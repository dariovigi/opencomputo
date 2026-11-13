// Report: anteprima del documento (lo stesso da cui nascono PDF, XLSX e CSV) ed export.

import { h, sostituisci, oggi, notifica, preferenza, salvaPreferenza } from '../dom.js';
import { chiedi, mostraErrore, stato, computoCorrente } from '../stato.js';
import { scegliSalvataggio, scegliFile, apriFile } from '../piattaforma.js';

export const titolo = 'Report ed export';

const locale = {
  tipo: 'computo', computo: null, numero: null, data: oggi(), voce: null, sintetica: false, verbale: '', conAnalisi: false,
  // Modello tradizionale (solo PDF): ente e studio restano tra una sessione e l'altra.
  tradizionale: false, senzaPrezzi: false, frontespizio: true,
  // Cronoprogramma: diagramma di Gantt (PDF) o tabella delle attività.
  gantt: true,
  ente: preferenza('stampa.ente', ''), studio: preferenza('stampa.studio', ''),
  // Incidenze di manodopera e sicurezza nel computo (ADR 0021), ricordate come l'ente.
  incidenze: { manodopera: false, sicurezza: false, riepilogo: false, ...preferenza('stampa.incidenze', {}) },
};

/** Computo con le analisi delle voci (solo se l'elenco ha analisi). */
function conAnalisi() {
  return locale.tipo === 'computo' && locale.conAnalisi && stato.info.elenco_prezzi.analisi > 0;
}

/** Computo metrico senza prezzi (modello tradizionale del computo). */
function senzaPrezzi() {
  return locale.tradizionale && locale.tipo === 'computo' && !conAnalisi() && locale.senzaPrezzi;
}

/** Opzioni del modello tradizionale (tutti i documenti, ADR 0015 e 0023), se scelto. */
function stampaTradizionale() {
  if (!locale.tradizionale || gantt()) return undefined;
  return {
    ente: locale.ente.trim() || null,
    studio: locale.studio.trim() || null,
    frontespizio: locale.frontespizio,
    senza_prezzi: senzaPrezzi(),
  };
}

/** Diagramma di Gantt del cronoprogramma (solo PDF). */
function gantt() {
  return locale.tipo === 'cronoprogramma' && locale.gantt;
}

/** Le incidenze riguardano gli importi: non nel computo con le analisi né in quello senza prezzi. */
function incidenzeApplicabili() {
  return locale.tipo === 'computo' && !conAnalisi() && !senzaPrezzi();
}

/** Incidenze richieste, se applicabili e se ce n'è almeno una. */
function incidenzeRichieste() {
  const { manodopera, sicurezza, riepilogo } = locale.incidenze;
  if (!incidenzeApplicabili() || !(manodopera || sicurezza)) return undefined;
  return { manodopera, sicurezza, riepilogo };
}

const TIPI = [
  ['computo', 'Computo metrico estimativo'],
  ['elenco_prezzi', 'Elenco prezzi'],
  ['libretto', 'Libretto delle misure'],
  ['registro', 'Registro di contabilità'],
  ['sal', 'Stato di avanzamento lavori'],
  ['certificato', 'Certificato di pagamento'],
  ['sommario', 'Sommario del registro'],
  ['conto_finale', 'Conto finale e rata di saldo'],
  ['analisi_prezzi', 'Analisi dei prezzi'],
  ['fabbisogno', 'Fabbisogno di cantiere'],
  ['nuovi_prezzi', 'Nuovi prezzi e verbali'],
  ['cronoprogramma', 'Cronoprogramma dei lavori'],
];

const CON_SAL = ['sal', 'certificato', 'sommario'];
const CON_NUMERO = ['sal', 'certificato'];

function richiestaReport() {
  const r = documentoRichiesto();
  const tradizionale = stampaTradizionale();
  return tradizionale ? { ...r, tradizionale } : r;
}

function documentoRichiesto() {
  switch (locale.tipo) {
    case 'computo': {
      const r = { tipo: 'computo', computo: locale.computo, con_analisi: conAnalisi() };
      const incidenze = incidenzeRichieste();
      return incidenze ? { ...r, incidenze } : r;
    }
    case 'sal':
    case 'certificato': return { tipo: locale.tipo, numero: locale.numero };
    case 'fabbisogno': return { tipo: 'fabbisogno', computo: locale.computo };
    case 'analisi_prezzi': return { tipo: 'analisi_prezzi', voce: locale.voce, sintetica: locale.sintetica };
    case 'nuovi_prezzi': return { tipo: 'nuovi_prezzi', verbale: locale.verbale ? Number(locale.verbale) : null };
    case 'cronoprogramma': return { tipo: 'cronoprogramma', gantt: locale.gantt };
    default: return { tipo: locale.tipo };
  }
}

function nomeFile(base, estensione) {
  const titolo_ = (stato.info?.titolo || 'progetto').replace(/[\\/:*?"<>|]+/g, ' ').trim().slice(0, 40);
  return `${titolo_} - ${base}.${estensione}`;
}

export async function mostra(el, parametri) {
  const nuovoTipo = parametri.tipo;
  if (nuovoTipo) {
    locale.tipo = nuovoTipo;
    locale.voce = parametri.voce || null;
    if (nuovoTipo === 'nuovi_prezzi') { locale.verbale = parametri.numero ? String(parametri.numero) : ''; delete parametri.numero; }
    delete parametri.tipo;
    delete parametri.voce;
  }
  if (parametri.numero) { locale.numero = Number(parametri.numero); delete parametri.numero; }
  const verbali = stato.info.contabilita.verbali_nuovi_prezzi ?? [];
  if (locale.verbale && !verbali.includes(Number(locale.verbale))) locale.verbale = '';
  const computi = stato.info.computi.map((c) => c.nome);
  if (!computi.includes(locale.computo)) locale.computo = computoCorrente();
  const sal = stato.info.contabilita.sal.map((s) => s.numero);
  if (!sal.includes(locale.numero)) locale.numero = sal.at(-1) ?? null;

  const anteprima = h('div', { class: 'anteprima-sfondo', 'aria-label': 'Anteprima' }, h('p', { class: 'vuoto' }, 'Preparazione dell\'anteprima…'));
  const pannello = h('aside', { class: 'dettaglio', style: { borderLeft: '0', borderRight: '1px solid var(--bordo)' } });
  const disponibile = (tipo) => {
    if (tipo === 'conto_finale') return !!stato.info.contabilita.conto_finale;
    if (tipo === 'analisi_prezzi') return stato.info.elenco_prezzi.analisi > 0;
    if (tipo === 'nuovi_prezzi') return stato.info.elenco_prezzi.nuovi_prezzi > 0;
    return !CON_SAL.includes(tipo) || sal.length > 0;
  };
  const nonDisponibile = { conto_finale: ' (non redatto)', analisi_prezzi: ' (nessuna analisi)', nuovi_prezzi: ' (nessuno)' };

  const scelta = h('div', { class: 'scelta-report', role: 'radiogroup', 'aria-label': 'Tipo di report' }, TIPI.map(([k, t]) => {
    const radio = h('input', { type: 'radio', name: 'tipo-report', value: k, checked: locale.tipo === k, disabled: !disponibile(k) });
    radio.addEventListener('change', () => { locale.tipo = k; mostra(el, {}); });
    return h('label', { class: locale.tipo === k && 'corrente' }, radio, t, !disponibile(k) && h('small', { class: 'vuoto' }, nonDisponibile[k] ?? ' (nessun SAL)'));
  }));
  const selComputo = computi.length > 1 && (locale.tipo === 'computo' || locale.tipo === 'fabbisogno')
    ? h('select', { name: 'computo-report', onchange: (e) => { locale.computo = e.target.value; mostra(el, {}); } }, computi.map((c) => h('option', { value: c, selected: c === locale.computo }, c)))
    : null;
  const selSal = CON_NUMERO.includes(locale.tipo) && sal.length
    ? h('select', { name: 'sal-report', onchange: (e) => { locale.numero = Number(e.target.value); mostra(el, {}); } }, sal.map((n) => h('option', { value: n, selected: n === locale.numero }, `SAL n. ${n}`)))
    : null;
  const tradizionale = !!stampaTradizionale();
  const opzioniComputo = locale.tipo === 'computo' && stato.info.elenco_prezzi.analisi > 0 && h('label', { class: 'campo spunta' }, h('span', null, 'Con le analisi delle voci'),
    h('input', { type: 'checkbox', name: 'con-analisi', checked: locale.conAnalisi, onchange: (e) => { locale.conAnalisi = e.target.checked; mostra(el, {}); } }));
  const spunta = (nome, testo, chiave, disabilitata, suggerimento) => h('label', { class: 'campo spunta', title: suggerimento },
    h('span', null, testo),
    h('input', {
      type: 'checkbox', name: nome, checked: locale.incidenze[chiave] && !disabilitata, disabled: disabilitata,
      onchange: (e) => { locale.incidenze[chiave] = e.target.checked; salvaPreferenza('stampa.incidenze', locale.incidenze); mostra(el, {}); },
    }));
  const conQuote = locale.incidenze.manodopera || locale.incidenze.sicurezza;
  // Subito sotto il documento, prima delle opzioni del modello: le quattro stampe del
  // computo (senza incidenze, manodopera, sicurezza, entrambe) si scelgono qui.
  const opzioniIncidenze = incidenzeApplicabili() && h('div', { class: 'modulo', role: 'group', 'aria-label': 'Incidenze', style: { marginTop: '8px' } },
    h('h4', { class: 'titoletto' }, 'Incidenze'),
    spunta('incidenza-manodopera', 'Incidenza della manodopera', 'manodopera', false, tradizionale ? 'Colonne «MANODOPERA»: quota in euro e incidenza di ogni voce' : 'Percentuale e quota di manodopera di ogni voce'),
    spunta('incidenza-sicurezza', 'Incidenza della sicurezza', 'sicurezza', false, tradizionale ? 'Colonne «SICUREZZA»: quota in euro e incidenza di ogni voce' : 'Percentuale e quota di sicurezza compresa nel prezzo di ogni voce'),
    spunta('subtotali-incidenze', 'Subtotali nel riepilogo finale', 'riepilogo', !conQuote, 'Quote di manodopera e sicurezza per categorie e WBS nel riepilogo finale'));
  const opzioniAnalisi = locale.tipo === 'analisi_prezzi' && h('div', { class: 'modulo' },
    locale.voce && h('label', { class: 'campo spunta' }, h('span', null, `Solo ${locale.voce}`),
      h('input', { type: 'checkbox', name: 'solo-voce', checked: true, onchange: () => { locale.voce = null; mostra(el, {}); } })),
    h('label', { class: 'campo spunta' }, h('span', null, 'Quadro sintetico'),
      h('input', { type: 'checkbox', name: 'sintetica', checked: locale.sintetica, onchange: (e) => { locale.sintetica = e.target.checked; mostra(el, {}); } })));
  const testoPreferenza = (nome, chiave, segnaposto) => {
    const t = h('textarea', { name: nome, rows: 2, placeholder: segnaposto, value: locale[chiave] });
    // Cambia solo l'anteprima: il modulo resta com'è, con il cursore al suo posto.
    t.addEventListener('change', () => { locale[chiave] = t.value; salvaPreferenza('stampa.' + chiave, t.value); aggiornaAnteprima(); });
    return t;
  };
  const opzioniCrono = locale.tipo === 'cronoprogramma' && h('div', { class: 'modulo', style: { marginTop: '8px' } },
    h('h4', { class: 'titoletto' }, 'Cronoprogramma'),
    h('label', { class: 'campo spunta', title: 'A3 orizzontale: tabella delle attività e barre sul calendario, con legami, percorso critico e linea di base' },
      h('span', null, 'Diagramma di Gantt (PDF)'),
      h('input', { type: 'checkbox', name: 'gantt', checked: locale.gantt, onchange: (e) => { locale.gantt = e.target.checked; mostra(el, {}); } })),
    h('p', { class: 'nota' }, locale.gantt
      ? 'L\'anteprima mostra le attività; il diagramma è nel PDF.'
      : 'Tabella delle attività e produzione mensile: anche in XLSX e CSV.'));
  const opzioniModello = !gantt() && h('div', { class: 'modulo', style: { marginTop: '8px' } },
    h('h4', { class: 'titoletto' }, 'Modello di stampa'),
    h('label', { class: 'campo spunta', title: 'Impaginazione tradizionale: frontespizio, testata dello studio, riquadro verde; nel computo anche i riporti di pagina' },
      h('span', null, 'Modello tradizionale'),
      h('input', { type: 'checkbox', name: 'tradizionale', checked: locale.tradizionale, onchange: (e) => { locale.tradizionale = e.target.checked; mostra(el, {}); } })),
    tradizionale && [
      locale.tipo === 'computo' && !conAnalisi() && h('label', { class: 'campo spunta' }, h('span', null, 'Senza prezzi (computo metrico)'),
        h('input', { type: 'checkbox', name: 'senza-prezzi', checked: locale.senzaPrezzi, onchange: (e) => { locale.senzaPrezzi = e.target.checked; mostra(el, {}); } })),
      h('label', { class: 'campo spunta' }, h('span', null, 'Frontespizio'),
        h('input', { type: 'checkbox', name: 'frontespizio', checked: locale.frontespizio, onchange: (e) => { locale.frontespizio = e.target.checked; mostra(el, {}); } })),
      h('label', { class: 'campo' }, h('span', null, 'Ente (frontespizio)'), testoPreferenza('ente', 'ente', 'Comune di …\nProvincia di …')),
      h('label', { class: 'campo' }, h('span', null, 'Studio tecnico (testata)'), testoPreferenza('studio', 'studio', 'Studio tecnico …\nIndirizzo, telefono, e-mail')),

    ]);
  const selVerbale = locale.tipo === 'nuovi_prezzi' && h('select', { name: 'verbale-report', onchange: (e) => { locale.verbale = e.target.value; mostra(el, {}); } },
    h('option', { value: '', selected: !locale.verbale }, 'Elenco dei nuovi prezzi'),
    verbali.map((n) => h('option', { value: String(n), selected: String(n) === locale.verbale }, `Verbale di concordamento n. ${n}`)));
  const dataStampa = h('input', { type: 'date', name: 'data-stampa', value: locale.data });
  dataStampa.addEventListener('change', () => { locale.data = dataStampa.value; mostra(el, {}); });

  const esporta = async (formato) => {
    try {
      const report = richiestaReport();
      const nome = locale.tipo === 'sal' ? `SAL ${locale.numero}` : locale.tipo === 'certificato' ? `Certificato ${locale.numero}`
        : locale.tipo === 'nuovi_prezzi' && locale.verbale ? `Verbale nuovi prezzi ${locale.verbale}` : TIPI.find(([k]) => k === locale.tipo)[1];
      const base = stampaTradizionale() ? `${nome} (tradizionale)` : gantt() ? 'Diagramma di Gantt' : nome;
      let richiesta;
      if (formato === 'csv') {
        const cartella = await scegliFile('Cartella per i file CSV (uno per sezione)', null, { cartella: true });
        if (!cartella) return;
        richiesta = { richiesta: 'report', report, formato, cartella, data_stampa: locale.data || null };
      } else {
        const filtri = formato === 'pdf' ? [{ name: 'PDF', extensions: ['pdf'] }] : [{ name: 'Excel', extensions: ['xlsx'] }];
        let file = await scegliSalvataggio(`Esporta ${formato.toUpperCase()}`, filtri, nomeFile(base, formato));
        if (!file) return;
        if (!file.toLowerCase().endsWith('.' + formato)) file += '.' + formato;
        richiesta = { richiesta: 'report', report, formato, file, data_stampa: locale.data || null };
      }
      const r = await chiedi(richiesta);
      const primo = r.file[0]?.percorso;
      notifica(r.file.length === 1 ? `Esportato ${primo.split(/[\\/]/).pop()}` : `Esportati ${r.file.length} file CSV`, 'ok',
        r.file.length === 1 ? { testo: 'Apri', fn: () => apriFile(primo).catch(mostraErrore) } : null, 8000);
    } catch (e) { mostraErrore(e); }
  };
  const esportaXpwe = async () => {
    try {
      let percorso = await scegliSalvataggio('Esporta XPWE (sperimentale)', [{ name: 'PriMus XPWE', extensions: ['xpwe'] }], nomeFile(locale.computo || 'computo', 'xpwe'));
      if (!percorso) return;
      if (!/\.xpwe$/i.test(percorso)) percorso += '.xpwe';
      const r = await chiedi({ richiesta: 'esporta_xpwe', percorso, computo: locale.computo });
      notifica(`XPWE esportato${r.anomalie.length ? ` con ${r.anomalie.length} segnalazioni (dati approssimati o non esportati)` : ''}`, r.anomalie.length ? 'avviso' : 'ok');
    } catch (e) { mostraErrore(e); }
  };

  const esportaDcf = async () => {
    try {
      let percorso = await scegliSalvataggio('Esporta l\'elenco prezzi in DCF (sperimentale)', [{ name: 'PriMus DCF', extensions: ['dcf'] }], nomeFile('elenco prezzi', 'dcf'));
      if (!percorso) return;
      if (!/\.dcf$/i.test(percorso)) percorso += '.dcf';
      const r = await chiedi({ richiesta: 'esporta_dcf', percorso, data: locale.data || null });
      notifica(`DCF esportato: ${r.voci} voci, ${r.capitoli} capitoli. PriMus non apre questi file: per PriMus usare l'export XPWE`, 'avviso', null, 10000);
    } catch (e) { mostraErrore(e); }
  };

  sostituisci(pannello,
    h('h3', null, 'Documento'), scelta,
    selComputo && h('label', { class: 'campo' }, h('span', null, 'Computo'), selComputo),
    selSal && h('label', { class: 'campo' }, h('span', null, 'SAL'), selSal),
    opzioniComputo,
    opzioniIncidenze,
    opzioniCrono,
    opzioniModello,
    opzioniAnalisi,
    selVerbale && h('label', { class: 'campo' }, h('span', null, 'Documento'), selVerbale),
    h('label', { class: 'campo', style: { marginTop: '10px' } }, h('span', null, 'Data di stampa'), dataStampa),
    h('h3', null, 'Esporta'),
    h('div', { class: 'modulo' },
      h('button', { class: 'pulsante primario', onclick: () => esporta('pdf') }, 'PDF…'),
      h('button', { class: 'pulsante', disabled: tradizionale || gantt(), onclick: () => esporta('xlsx') }, 'Excel (XLSX)…'),
      h('button', { class: 'pulsante', disabled: tradizionale || gantt(), onclick: () => esporta('csv') }, 'CSV (una cartella)…'),
      locale.tipo === 'computo' && h('button', { class: 'pulsante', onclick: esportaXpwe, title: 'Struttura XPWE verificata solo in parte: non consegnare come file PriMus' }, 'PriMus XPWE (sperimentale)…'),
      locale.tipo === 'elenco_prezzi' && h('button', { class: 'pulsante', onclick: esportaDcf, title: 'Prezzario DCF in chiaro, senza analisi. PriMus non apre i DCF scritti da altri programmi: per PriMus usare XPWE' }, 'PriMus DCF (sperimentale)…')));

  sostituisci(el,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo)),
    h('div', { class: 'vista-corpo pieno' }, h('div', { class: 'dividi', style: { gridTemplateColumns: '290px minmax(0, 1fr)' } }, pannello, h('div', { class: 'zona' }, anteprima))));

  await aggiornaAnteprima();

  async function aggiornaAnteprima() {
    if (CON_SAL.includes(locale.tipo) && !locale.numero) { sostituisci(anteprima, h('p', { class: 'vuoto' }, 'Nessun SAL.')); return; }
    // Se nel frattempo parte un'altra richiesta, conta solo l'ultima.
    const giro = (anteprima.dataset.giro = String(Number(anteprima.dataset.giro || 0) + 1));
    try {
      const richiesta = { richiesta: 'anteprima', report: richiestaReport(), data_stampa: locale.data || null };
      const a = await chiedi(richiesta);
      if (anteprima.dataset.giro !== giro) return;
      // Modello tradizionale: le pagine del PDF, una immagine per pagina.
      if (a.pagine) pagineTradizionali(anteprima, a, richiesta);
      else sostituisci(anteprima, foglio(a));
    } catch (e) {
      if (anteprima.dataset.giro === giro) sostituisci(anteprima, h('div', { class: 'banner errore' }, e.message));
    }
  }
}

/** Pagine per volta nell'anteprima del modello tradizionale. */
const PAGINE_PER_VOLTA = 6;

/**
 * Anteprima del modello tradizionale: le stesse pagine del PDF, in SVG, mostrate come
 * immagini (un SVG in un'immagine non esegue script né carica risorse). Le pagine
 * successive si caricano a richiesta.
 */
function pagineTradizionali(contenitore, primo, richiesta) {
  const totale = primo.pagine_totali;
  const conteggio = h('p', { class: 'nota pagine-conteggio', role: 'status' });
  const fogli = h('div', { class: 'pagine-stampa' });
  const altre = h('button', { class: 'pulsante', name: 'altre-pagine' });
  let mostrate = 0;
  const aggiungi = (a) => {
    a.pagine.forEach((svg, i) => fogli.append(h('img', {
      class: 'pagina-stampa', alt: `Pagina ${a.prima + i} di ${totale}`,
      src: 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg),
    })));
    mostrate = a.prima - 1 + a.pagine.length;
    conteggio.textContent = mostrate < totale
      ? `Pagine 1–${mostrate} di ${totale}: l'anteprima è quella del PDF.`
      : `${totale} ${totale === 1 ? 'pagina' : 'pagine'}: l'anteprima è quella del PDF.`;
    if (mostrate >= totale) altre.remove();
    else altre.textContent = `Mostra altre ${Math.min(PAGINE_PER_VOLTA, totale - mostrate)} pagine`;
  };
  altre.addEventListener('click', async () => {
    altre.disabled = true;
    try {
      aggiungi(await chiedi({ ...richiesta, prima_pagina: mostrate + 1, pagine: PAGINE_PER_VOLTA }));
    } catch (e) { mostraErrore(e); }
    altre.disabled = false;
  });
  aggiungi(primo);
  sostituisci(contenitore, conteggio, fogli, h('div', { class: 'pagine-altre' }, altre));
}

/** Resa HTML del documento di report. */
function foglio(a) {
  return h('article', { class: 'foglio-stampa' },
    h('h1', null, a.titolo),
    a.intestazione.length ? h('dl', { class: 'intestazione' }, a.intestazione.map(([k, v]) => [h('dt', null, k), h('dd', null, v)])) : null,
    a.sezioni.map((s) => {
      const totalePesi = s.colonne.reduce((t, c) => t + c.peso, 0) || 1;
      return h('section', null,
        h('h2', null, s.titolo),
        s.note.map((n) => h('p', { class: 'note' }, n)),
        h('table', null,
          h('colgroup', null, s.colonne.map((c) => h('col', { style: { width: (100 * c.peso / totalePesi).toFixed(2) + '%' } }))),
          h('thead', null, h('tr', null, s.colonne.map((c) => h('th', { class: c.allineamento }, c.titolo)))),
          h('tbody', null, s.righe.map((r) => h('tr', { class: r.stile }, r.celle.map((testo, i) => h('td', { class: s.colonne[i]?.allineamento }, testo)))))));
    }),
    a.piede && h('div', { class: 'piede' }, a.piede));
}
