// Assistente AI (ADR 0025): proposta di voci di computo da una richiesta in linguaggio
// naturale, indice dei prezzari, impostazioni (consenso, chiavi, modelli).
//
// Il modello sceglie solo tra voci del prezzario e forme di rigo; quantità, prezzi e
// importi arrivano dal motore (anteprima calcolata dal core), e la proposta entra nel
// computo solo con «Applica», come un passo annullabile. Le operazioni lunghe girano nel
// motore senza bloccare le altre finestre e si possono annullare.

import { h, sostituisci, numero, euro, notifica, conferma } from '../dom.js';
import { chiedi, agisci, stato, mostraErrore, vai, computoCorrente, ErroreServizio } from '../stato.js';
import { ascolta, scegliFile, FILTRO_PREZZARI } from '../piattaforma.js';

export const titolo = 'Assistente AI';

const SCHEDE = [['proposta', 'Proposta'], ['indice', 'Prezzari e indice'], ['impostazioni', 'Impostazioni']];

const locale = {
  scheda: 'proposta',
  prezzario: null,
  computo: null,
  testo: '',
  /** Risultato di assistente_stato. */
  ai: null,
  /** Stato dell'indice per prezzario: {nome: risultato di assistente_indice}. */
  indici: {},
  /** Ultimo risultato di assistente_proponi. */
  risultato: null,
  /** Operazione in corso: {tipo: 'proponi'|'indicizza', prezzario, inizio}. */
  inCorso: null,
  /** Ultimo avanzamento dell'indicizzazione. */
  avanzamento: null,
};

let elVista = null;
let orologio = null;

const UNITA = { mm: 'mm', cm: 'cm', m: 'm', m2: 'm²', m3: 'm³', kg: 'kg', kg_m: 'kg/m', n: '' };
const RUOLI = { principale: 'Principale', complementare: 'Complementare', alternativa: 'Alternativa' };
const IMPATTI = { basso: 'impatto basso', medio: 'impatto medio', alto: 'impatto alto' };
const ESITI = {
  completo: ['Completa', 'ok'],
  provvisorio: ['Da rivedere', 'avviso'],
  impossibile: ['Nessuna voce proposta', 'errore'],
};
const MODALITA = [
  ['automatica', 'Automatica (JSON vincolato a uno schema)'],
  ['json', 'JSON vincolato a uno schema'],
  ['strumento_forzato', 'Strumento forzato (tool use)'],
];

ascolta('assistente-avanzamento', (dato) => {
  if (dato?.avanzamento !== 'indicizzazione' || !locale.inCorso) return;
  locale.avanzamento = dato;
  aggiornaAvanzamento();
});

export async function mostra(el, parametri, { nuova }) {
  if (parametri.scheda) { locale.scheda = parametri.scheda; delete parametri.scheda; }
  elVista = el;
  const caricati = stato.info?.prezzari_caricati ?? [];
  if (!caricati.some((p) => p.nome === locale.prezzario)) locale.prezzario = caricati[0]?.nome ?? null;
  const computi = stato.info?.computi?.map((c) => c.nome) ?? [];
  if (!computi.includes(locale.computo)) locale.computo = computoCorrente();
  if (nuova || !locale.ai) await caricaStato();
  if (nuova || caricati.some((p) => !locale.indici[p.nome])) await caricaIndici();
  ridisegna();
}

async function caricaStato() {
  try {
    locale.ai = await chiedi({ richiesta: 'assistente_stato' });
  } catch (e) {
    mostraErrore(e);
  }
}

/** Stato dell'indice dei prezzari caricati (senza chiamate ai provider). */
async function caricaIndici() {
  const caricati = stato.info?.prezzari_caricati ?? [];
  const nuovi = {};
  for (const p of caricati) {
    try {
      nuovi[p.nome] = await chiedi({ richiesta: 'assistente_indice', prezzario: p.nome });
    } catch (e) {
      mostraErrore(e);
    }
  }
  locale.indici = nuovi;
}

function ridisegna() {
  if (!elVista || !locale.ai) {
    if (elVista) sostituisci(elVista, h('div', { class: 'vista-corpo' }, h('p', { class: 'vuoto' }, 'Assistente non disponibile.')));
    return;
  }
  const schede = h('div', { class: 'schede-tab', role: 'tablist' }, SCHEDE.map(([k, t]) => h('button', {
    role: 'tab', name: `scheda-${k}`, class: locale.scheda === k && 'corrente', 'aria-selected': locale.scheda === k ? 'true' : 'false',
    onclick: () => { locale.scheda = k; ridisegna(); },
  }, t)));
  const corpo = { proposta: schedaProposta, indice: schedaIndice, impostazioni: schedaImpostazioni }[locale.scheda]();
  sostituisci(elVista,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo), schede),
    h('div', { class: 'vista-corpo' },
      h('div', { class: 'banner avviso ai-fuori', role: 'note' },
        h('span', null, h('strong', null, 'I dati inviati escono dal computer. '),
          'La richiesta e le descrizioni delle voci candidate vanno ad Anthropic; il testo delle voci dei prezzari e delle ricerche va a OpenAI. Prezzi e importi non vengono inviati.'),
        locale.scheda !== 'impostazioni' && h('button', { class: 'link', onclick: () => { locale.scheda = 'impostazioni'; ridisegna(); } }, 'Dettagli')),
      corpo));
  aggiornaOrologio();
}

// ---------------------------------------------------------------------------
// Proposta

function schedaProposta() {
  const ai = locale.ai;
  if (!ai.consenso.valido) return pannelloConsenso();
  const caricati = stato.info?.prezzari_caricati ?? [];
  const occupato = !!locale.inCorso;
  const area = h('textarea', {
    name: 'richiesta-ai', rows: 5, value: locale.testo, disabled: occupato,
    placeholder: 'Es.: Tinteggiatura con idropittura di due pareti di 4 x 3 m e del soffitto di 4 x 5 m; rasatura delle pareti.',
    'aria-label': 'Richiesta',
    oninput: (e) => { locale.testo = e.target.value; aggiornaPulsanti(); },
    onkeydown: (e) => { if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); proponi(); } },
  });
  const selPrezzario = caricati.length
    ? h('select', { name: 'prezzario-ai', 'aria-label': 'Prezzario', disabled: occupato, onchange: (e) => { locale.prezzario = e.target.value; ridisegna(); } },
      caricati.map((p) => h('option', { value: p.nome, selected: p.nome === locale.prezzario }, `${p.nome} (${p.voci} voci)`)))
    : h('span', { class: 'vuoto' }, 'Nessun prezzario caricato');
  const computi = stato.info?.computi ?? [];
  const selComputo = h('select', { name: 'computo-ai', 'aria-label': 'Computo', disabled: occupato, onchange: (e) => { locale.computo = e.target.value; } },
    computi.map((c) => h('option', { value: c.nome, selected: c.nome === locale.computo }, c.nome)));
  const indice = locale.indici[locale.prezzario];
  const mancaChiave = !ai.chiavi.anthropic;
  return h('div', { class: 'assistente' },
    mancaChiave && h('div', { class: 'banner errore' }, h('span', null, 'Manca la chiave API di Anthropic: senza, l\'assistente non può proporre voci.'),
      h('button', { class: 'link', onclick: () => { locale.scheda = 'impostazioni'; ridisegna(); } }, 'Imposta la chiave')),
    h('section', { class: 'scheda ai-richiesta' },
      h('div', { class: 'ai-scelte' },
        h('label', { class: 'campo-riga' }, 'Prezzario', selPrezzario),
        h('button', { class: 'pulsante', name: 'carica-prezzario-ai', disabled: occupato, onclick: caricaPrezzario }, 'Carica prezzario…'),
        h('label', { class: 'campo-riga' }, 'Computo', selComputo),
        indice && h('span', { class: ['etichetta', indice.ricerca === 'ibrida' ? 'ok' : 'neutra'], title: indice.ricerca === 'ibrida' ? 'Ricerca per parole e per significato' : 'Ricerca solo per parole: indicizzare il prezzario per la ricerca per significato' },
          indice.ricerca === 'ibrida' ? 'Ricerca ibrida' : 'Ricerca per parole')),
      h('label', { class: 'campo' }, h('span', null, 'Richiesta'), area,
        h('small', null, 'Indicare le misure note: i valori mancanti diventano ipotesi dichiarate o domande. Ctrl+Invio per inviare.')),
      h('div', { class: 'ai-azioni' },
        h('button', { class: 'pulsante primario', name: 'proponi', onclick: proponi }, 'Proponi'),
        locale.inCorso?.tipo === 'proponi' && h('button', { class: 'pulsante', name: 'annulla-ai', onclick: annullaOperazione }, 'Annulla'),
        locale.inCorso?.tipo === 'proponi' && h('span', { class: 'ai-attesa', role: 'status' }, 'In attesa del modello… ', h('span', { class: 'ai-secondi' })))),
    locale.risultato && vistaRisultato(locale.risultato));
}

function aggiornaPulsanti() {
  const p = elVista?.querySelector('button[name="proponi"]');
  if (p) p.disabled = !puoProporre();
}

function puoProporre() {
  return !locale.inCorso && !!locale.prezzario && !!locale.testo.trim() && !!locale.ai?.chiavi.anthropic && !!locale.ai?.consenso.valido;
}

async function proponi() {
  if (!puoProporre()) { aggiornaPulsanti(); return; }
  locale.risultato = null;
  const r = await lavora({ tipo: 'proponi', prezzario: locale.prezzario }, {
    richiesta: 'assistente_proponi', prezzario: locale.prezzario, testo: locale.testo.trim(), computo: locale.computo,
  });
  if (r) locale.risultato = r;
  ridisegna();
}

function vistaRisultato(r) {
  if (r.esito === 'da_chiarire') {
    const domande = r.analisi.informazioni_mancanti;
    return h('section', { class: 'scheda ai-risultato' },
      h('h2', null, 'Servono altre informazioni'),
      h('p', null, 'Senza queste risposte il computo non sarebbe attendibile. Completare la richiesta e riprovare.'),
      h('ul', { class: 'elenco-problemi' }, domande.map((d) => h('li', { class: d.bloccante ? 'avviso' : 'informazione' }, d.domanda,
        d.lavorazioni.length ? h('small', null, ` — ${d.lavorazioni.join(', ')}`) : null))),
      h('button', { class: 'pulsante', name: 'aggiungi-risposte', onclick: () => aggiungiRisposte(domande) }, 'Aggiungi le domande alla richiesta'),
      consumi(r));
  }
  const p = r.proposta;
  const prova = r.prova ?? { ok: false, errore: { descrizione: 'prova non eseguita' } };
  const voci = new Map((prova.voci ?? []).map((v) => [v.lavorazione, v]));
  const [testoEsito, classeEsito] = ESITI[p.esito] ?? [p.esito, 'neutra'];
  const totali = prova.esito?.totali ?? [];
  const incluse = p.lavorazioni.filter((l) => l.inclusa);
  return h('section', { class: 'scheda ai-risultato', 'aria-label': 'Proposta' },
    h('h2', null, 'Proposta', h('span', { class: ['etichetta', classeEsito], name: 'esito-proposta' }, testoEsito),
      h('span', { class: 'etichetta neutra' }, r.ricerca_ibrida ? 'ricerca ibrida' : 'ricerca per parole')),
    r.avviso_ricerca && h('p', { class: 'nota' }, r.avviso_ricerca),
    prova.ok
      ? h('div', { class: 'ai-totali', role: 'status' }, totali.length
        ? totali.map((t) => h('span', null, `${t.computo}: ${euro(t.prima)} → `, h('strong', null, euro(t.dopo))))
        : h('span', { class: 'vuoto' }, 'Nessun totale cambia.'))
      : h('div', { class: 'banner errore' }, h('span', null, `La proposta non si può applicare: ${prova.errore?.descrizione ?? 'errore'}`)),
    incluse.length ? h('table', { class: 'ai-voci' },
      h('thead', null, h('tr', null,
        h('th', null, 'Lavorazione'), h('th', null, 'Voce'), h('th', { class: 'num' }, 'Quantità'), h('th', null, 'U.M.'),
        h('th', { class: 'num' }, 'Prezzo'), h('th', { class: 'num' }, 'Importo'))),
      h('tbody', null, incluse.map((l) => rigaLavorazione(l, voci.get(l.id), p.ipotesi)))) : null,
    elenco('Alternative non scelte', p.lavorazioni.filter((l) => !l.inclusa), (l) => h('li', { class: 'informazione' },
      `${l.id} — ${l.descrizione}: `, h('span', { class: 'mono' }, l.codice), l.motivazione ? ` (${l.motivazione})` : '')),
    elenco('Lavorazioni escluse', p.escluse, (l) => h('li', { class: 'avviso' }, h('strong', null, `${l.id} — ${l.descrizione}`), h('small', null, ` — ${l.motivi.join('; ')}`))),
    elenco('Ipotesi', p.ipotesi, (i) => h('li', { class: i.impatto === 'alto' ? 'avviso' : 'informazione' },
      h('strong', null, `${i.id}: `), i.testo, h('span', { class: ['etichetta', i.impatto === 'alto' ? 'avviso' : 'neutra'] }, IMPATTI[i.impatto] ?? i.impatto),
      i.dalla_validazione ? h('small', null, ' — valore non trovato nella richiesta') : null)),
    elenco('Informazioni mancanti', p.informazioni_mancanti, (d) => h('li', { class: d.bloccante ? 'avviso' : 'informazione' }, d.domanda)),
    elenco('Avvisi', p.avvisi, (a) => h('li', { class: a.gravita }, a.messaggio, a.lavorazione ? h('small', null, ` — ${a.lavorazione}`) : null)),
    h('div', { class: 'ai-azioni' },
      h('button', {
        class: 'pulsante primario', name: 'applica-proposta', disabled: !prova.ok || !incluse.length || !!locale.inCorso, onclick: applica,
        title: 'Aggiunge le voci al computo; si annulla come ogni altra modifica',
      }, `Applica al computo «${prova.computo ?? locale.computo ?? ''}»`),
      h('button', { class: 'pulsante', name: 'scarta-proposta', onclick: () => { locale.risultato = null; ridisegna(); } }, 'Scarta')),
    consumi(r));
}

function rigaLavorazione(l, v, ipotesi) {
  const righe = l.righe.map((r) => h('li', null,
    h('span', null, `${r.deduzione ? 'a dedurre: ' : ''}${r.descrizione || r.forma} `),
    h('span', { class: 'ai-forma' }, r.forma.replace(/_/g, ' ')), ' ',
    r.parametri.map((p) => h('span', {
      class: ['ai-parametro', p.provenienza.tipo === 'ipotesi' && 'ipotesi'],
      title: p.provenienza.tipo === 'ipotesi'
        ? `Ipotesi ${p.provenienza.id}: ${ipotesi.find((i) => i.id === p.provenienza.id)?.testo ?? ''}`
        : (p.provenienza.testo ? `Dalla richiesta: «${p.provenienza.testo}»` : 'Dalla richiesta'),
    }, `${p.nome} = ${numero(p.valore, decimali(p.valore, 0))}${UNITA[p.unita] ? ' ' + UNITA[p.unita] : ''}`,
    p.provenienza.tipo === 'ipotesi' ? ` (${p.provenienza.id})` : ''))));
  return [
    h('tr', { class: 'ai-voce', dataset: { lavorazione: l.id } },
      h('td', null, h('strong', null, l.id), ' ', l.descrizione, l.ruolo !== 'principale' ? h('span', { class: 'etichetta neutra' }, RUOLI[l.ruolo]) : null),
      h('td', null, h('span', { class: 'mono' }, l.codice), ' ', v?.descrizione ?? ''),
      h('td', { class: 'num' }, v?.quantita ? numero(v.quantita, decimali(v.quantita)) : '—'),
      h('td', null, v?.unita_misura ?? l.unita_misura),
      h('td', { class: 'num' }, v ? numero(v.prezzo, 2) : '—'),
      h('td', { class: 'num' }, v?.importo ? numero(v.importo, 2) : '—')),
    h('tr', { class: 'ai-dettaglio' }, h('td', { colSpan: 6 },
      h('ul', { class: 'ai-righe' }, righe),
      l.motivazione ? h('div', { class: 'ai-motivazione' }, 'Scelta: ', l.motivazione) : null)),
  ];
}

/** Decimali da mostrare: quelli del valore, da `minimo` a 4. */
function decimali(v, minimo = 2) {
  const f = String(v ?? '').split('.')[1] ?? '';
  return Math.max(minimo, Math.min(f.length, 4));
}

function elenco(titolo_, voci, riga) {
  if (!voci?.length) return null;
  return h('div', { class: 'ai-elenco' }, h('h3', null, titolo_), h('ul', { class: 'elenco-problemi' }, voci.map(riga)));
}

function consumi(r) {
  const c = r.consumi;
  if (!c) return null;
  return h('p', { class: 'nota ai-consumi' }, `Token: ${c.token_ingresso} in ingresso, ${c.token_uscita} in uscita; testi per gli embedding: ${c.testi_embedding}.`);
}

function aggiungiRisposte(domande) {
  const righe = domande.map((d) => `- ${d.domanda} `).join('\n');
  locale.testo = `${locale.testo.trim()}\n\nRisposte:\n${righe}`;
  locale.risultato = null;
  ridisegna();
  const area = elVista?.querySelector('textarea[name="richiesta-ai"]');
  area?.focus();
}

async function applica() {
  const n = locale.risultato?.prova?.voci?.length ?? 0;
  const computo = locale.risultato?.prova?.computo ?? locale.computo;
  const r = await agisci({ richiesta: 'assistente_applica' });
  if (!r) return;
  locale.risultato = null;
  notifica(`${n === 1 ? 'Una voce aggiunta' : `${n} voci aggiunte`} a «${computo}»`, 'ok', { testo: 'Vai al computo', fn: () => vai('computo', { computo, ultima: true }) }, 8000);
  ridisegna();
}

// ---------------------------------------------------------------------------
// Indice

function schedaIndice() {
  const caricati = stato.info?.prezzari_caricati ?? [];
  const ai = locale.ai;
  const occupato = !!locale.inCorso;
  return h('div', { class: 'assistente' },
    h('section', { class: 'scheda' },
      h('h2', null, 'Prezzari caricati',
        h('span', { class: 'spinge' }),
        h('button', { class: 'pulsante', name: 'carica-prezzario-ai', disabled: occupato, onclick: caricaPrezzario }, 'Carica prezzario…')),
      h('p', { class: 'nota' }, 'Indicizzare un prezzario abilita la ricerca per significato oltre che per parole. Il testo di ogni voce (codice, descrizioni, capitoli) viene inviato una volta a OpenAI; i vettori restano in una cache su questo computer e si ricalcolano solo per le voci cambiate.'),
      !ai.consenso.valido && h('div', { class: 'banner avviso' }, 'Per indicizzare serve il consenso all\'invio dei dati (scheda Impostazioni).'),
      ai.consenso.valido && !ai.chiavi.openai && h('div', { class: 'banner avviso' }, 'Per indicizzare serve la chiave API di OpenAI (scheda Impostazioni).'),
      caricati.length ? h('table', { class: 'ai-indici' },
        h('thead', null, h('tr', null, h('th', null, 'Prezzario'), h('th', { class: 'num' }, 'Voci'), h('th', { class: 'num' }, 'Indicizzate'), h('th', null, 'Ricerca'), h('th', null, ''))),
        h('tbody', null, caricati.map((p) => {
          const i = locale.indici[p.nome];
          const qui = locale.inCorso?.tipo === 'indicizza' && locale.inCorso.prezzario === p.nome;
          return h('tr', { dataset: { prezzario: p.nome } },
            h('td', null, p.nome),
            h('td', { class: 'num' }, String(i?.voci ?? p.voci)),
            h('td', { class: 'num' }, i ? String(i.indicizzate) : '—'),
            h('td', null, i ? h('span', { class: ['etichetta', i.ricerca === 'ibrida' ? 'ok' : 'neutra'] }, i.ricerca === 'ibrida' ? 'Ibrida' : 'Per parole') : null),
            h('td', null, qui
              ? h('span', { class: 'ai-progresso' }, h('progress', { max: 1, value: 0, 'aria-label': 'Avanzamento' }), h('span', { class: 'ai-progresso-testo' }, 'Avvio…'),
                h('button', { class: 'pulsante', name: 'annulla-ai', onclick: annullaOperazione }, 'Annulla'))
              : h('button', {
                class: 'pulsante', name: `indicizza-${p.nome}`, disabled: occupato || !ai.consenso.valido || !ai.chiavi.openai,
                onclick: () => indicizza(p.nome, i),
              }, i?.completo ? 'Aggiorna' : i?.indicizzate ? 'Completa' : 'Indicizza')));
        }))) : h('p', { class: 'vuoto' }, 'Nessun prezzario caricato: caricare un DCF, XPWE, listino CSV o JSON.')));
}

async function indicizza(nome, indice) {
  const mancanti = indice ? indice.voci - indice.indicizzate : null;
  const ok = await conferma('Indicizzare il prezzario?',
    `${mancanti === null ? 'Il testo delle voci' : `Il testo di ${mancanti} ${mancanti === 1 ? 'voce' : 'voci'}`} di «${nome}» (codice, descrizioni, capitoli) verrà inviato a OpenAI per calcolare gli embedding. Se il prezzario viene da un DCF distribuito da ACCA, verificare di avere il diritto di inviarne il contenuto.`,
    { ok: 'Invia e indicizza' });
  if (!ok) return;
  const r = await lavora({ tipo: 'indicizza', prezzario: nome }, { richiesta: 'assistente_indicizza', prezzario: nome });
  if (r) {
    const k = r.rapporto;
    notifica(`«${nome}»: ${k.calcolate} voci calcolate, ${k.riusate} riusate${k.rimosse ? `, ${k.rimosse} tolte` : ''}`, 'ok');
  }
  await caricaIndici();
  ridisegna();
}

function aggiornaAvanzamento() {
  const a = locale.avanzamento?.stato;
  const barra = elVista?.querySelector('.ai-progresso progress');
  const testo = elVista?.querySelector('.ai-progresso-testo');
  if (!a || !barra || !testo) return;
  barra.max = Math.max(1, a.da_calcolare);
  barra.value = a.calcolate;
  testo.textContent = `${a.calcolate} di ${a.da_calcolare} (lotto ${a.lotto} di ${a.lotti})`;
}

async function caricaPrezzario() {
  const percorso = await scegliFile('Carica prezzario', FILTRO_PREZZARI);
  if (!percorso) return;
  const base = percorso.split(/[\\/]/).pop().replace(/\.[^.]+$/, '');
  const r = await agisci({ richiesta: 'carica_prezzario', nome: base, percorso });
  if (!r) return;
  notifica(`Caricato «${r.nome}»: ${r.voci} voci${r.anomalie ? `, ${r.anomalie} segnalazioni` : ''}`, 'ok');
  locale.prezzario = r.nome;
  await caricaIndici();
  ridisegna();
}

// ---------------------------------------------------------------------------
// Impostazioni

function pannelloConsenso() {
  const c = locale.ai.consenso;
  const spunta = h('input', { type: 'checkbox', name: 'consenso-letto', onchange: (e) => { accetta.disabled = !e.target.checked; } });
  const accetta = h('button', { class: 'pulsante primario', name: 'accetta-consenso', disabled: true, onclick: () => impostaConsenso(true) }, 'Accetto');
  return h('section', { class: 'scheda ai-consenso', 'aria-label': 'Consenso all\'invio dei dati' },
    h('h2', null, 'Prima di usare l\'assistente'),
    testoConsenso(c.testo),
    h('label', { class: 'campo spunta' }, h('span', null, 'Ho letto: accetto che questi dati vengano inviati a OpenAI e Anthropic'), spunta),
    h('div', { class: 'ai-azioni' }, accetta, h('small', { class: 'vuoto' }, `Versione ${c.versione_corrente} del testo`)));
}

function testoConsenso(testo) {
  return h('div', { class: 'ai-testo-consenso' }, testo.split('\n\n').map((par) => {
    const righe = par.split('\n');
    return righe.every((r) => r.startsWith('•'))
      ? h('ul', null, righe.map((r) => h('li', null, r.replace(/^•\s*/, ''))))
      : h('p', null, par);
  }));
}

async function impostaConsenso(accetto) {
  const r = await agisci({ richiesta: 'assistente_consenso', accetto, versione: accetto ? locale.ai.consenso.versione_corrente : null });
  if (!r) return;
  notifica(accetto ? 'Consenso registrato' : 'Consenso revocato: l\'assistente non chiamerà più i provider', 'ok');
  await caricaStato();
  await caricaIndici();
  ridisegna();
}

function schedaImpostazioni() {
  const ai = locale.ai;
  const c = ai.configurazione;
  return h('div', { class: 'assistente' },
    h('section', { class: 'scheda', 'aria-label': 'Consenso' },
      h('h2', null, 'Consenso all\'invio dei dati',
        ai.consenso.valido ? h('span', { class: 'etichetta ok' }, 'accettato') : h('span', { class: 'etichetta avviso' }, 'non accettato')),
      ai.consenso.valido && c.consenso.data ? h('p', { class: 'nota' }, `Accettato il ${new Date(c.consenso.data).toLocaleString('it-IT')} (versione ${c.consenso.versione}).`) : null,
      testoConsenso(ai.consenso.testo),
      ai.consenso.valido
        ? h('button', { class: 'pulsante pericolo', name: 'revoca-consenso', onclick: async () => { if (await conferma('Revocare il consenso?', 'L\'assistente smetterà di chiamare OpenAI e Anthropic. La ricerca per parole resta disponibile.', { ok: 'Revoca', pericolo: true })) impostaConsenso(false); } }, 'Revoca il consenso')
        : h('button', { class: 'pulsante primario', name: 'accetta-consenso', onclick: () => impostaConsenso(true) }, 'Accetto l\'invio dei dati')),
    h('section', { class: 'scheda', 'aria-label': 'Chiavi API' },
      h('h2', null, 'Chiavi API'),
      h('p', { class: 'nota' }, `Le chiavi sono conservate in: ${ai.chiavi.archivio}. Non vengono mai scritte nei file di OpenComputo né mostrate.`),
      ai.chiavi.errore && h('div', { class: 'banner errore' }, `Portachiavi non accessibile: ${ai.chiavi.errore}`),
      rigaChiave('openai', 'OpenAI', 'embedding: ricerca per significato', ai.chiavi.openai),
      rigaChiave('anthropic', 'Anthropic', 'generazione: proposta delle voci', ai.chiavi.anthropic)),
    h('section', { class: 'scheda', 'aria-label': 'Modelli' },
      h('h2', null, 'Modelli'),
      moduloModelli(c, ai.modelli)),
    h('p', { class: 'nota' }, `Configurazione e cache dell'indice: ${ai.cartella}`));
}

function rigaChiave(provider, nome, uso, presente) {
  const input = h('input', { type: 'password', name: `chiave-${provider}`, autocomplete: 'off', spellcheck: false, placeholder: presente ? 'Nuova chiave (sostituisce quella salvata)' : 'Incollare la chiave' });
  const salva = async () => {
    const chiave = input.value.trim();
    if (!chiave) { input.focus(); return; }
    input.value = '';
    const r = await agisci({ richiesta: 'assistente_chiave', provider, chiave });
    if (!r) return;
    notifica(`Chiave di ${nome} salvata`, 'ok');
    await caricaStato();
    ridisegna();
  };
  const elimina = async () => {
    if (!(await conferma(`Eliminare la chiave di ${nome}?`, 'La chiave verrà tolta dal portachiavi del sistema.', { ok: 'Elimina', pericolo: true }))) return;
    const r = await agisci({ richiesta: 'assistente_chiave', provider, chiave: null });
    if (!r) return;
    await caricaStato();
    ridisegna();
  };
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); salva(); } });
  return h('div', { class: 'ai-chiave', dataset: { provider } },
    h('div', null, h('strong', null, nome), h('small', null, ` — ${uso}`),
      presente ? h('span', { class: 'etichetta ok' }, 'impostata') : h('span', { class: 'etichetta avviso' }, 'mancante')),
    input,
    h('button', { class: 'pulsante', name: `salva-chiave-${provider}`, onclick: salva }, 'Salva'),
    presente ? h('button', { class: 'pulsante pericolo', name: `elimina-chiave-${provider}`, onclick: elimina }, 'Elimina') : h('span'));
}

function moduloModelli(c, modelli) {
  const lista = (id, valori) => h('datalist', { id }, valori.map((v) => h('option', { value: v })));
  const generazione = h('input', { type: 'text', name: 'modello_generazione', value: c.modello_generazione, list: 'ai-modelli-generazione', autocomplete: 'off', spellcheck: false });
  const modalita = h('select', { name: 'modalita_output' }, MODALITA.map(([v, t]) => h('option', { value: v, selected: v === c.modalita_output }, t)));
  const embedding = h('input', { type: 'text', name: 'modello_embedding', value: c.modello_embedding, list: 'ai-modelli-embedding', autocomplete: 'off', spellcheck: false });
  const dimensioni = h('input', { type: 'text', inputMode: 'numeric', name: 'dimensioni_embedding', value: c.dimensioni_embedding ?? '', placeholder: modelli.dimensioni_predefinite ? `predefinite (${modelli.dimensioni_predefinite})` : 'predefinite' });
  const analisi = h('input', { type: 'text', inputMode: 'numeric', name: 'max_token_analisi', value: String(c.max_token_analisi) });
  const selezione = h('input', { type: 'text', inputMode: 'numeric', name: 'max_token_selezione', value: String(c.max_token_selezione) });
  const errore = h('div', { class: 'errore-modulo', role: 'alert' });
  const intero = (el, nome, vuoto = null) => {
    const t = el.value.trim();
    if (!t) return vuoto;
    if (!/^\d+$/.test(t)) throw new Error(`«${nome}»: indicare un numero intero`);
    return Number(t);
  };
  const salva = async () => {
    let configurazione;
    try {
      configurazione = {
        modello_generazione: generazione.value.trim(),
        modalita_output: modalita.value,
        modello_embedding: embedding.value.trim(),
        dimensioni_embedding: intero(dimensioni, 'Dimensioni', 0),
        max_token_analisi: intero(analisi, 'Token dell\'analisi', c.max_token_analisi),
        max_token_selezione: intero(selezione, 'Token della selezione', c.max_token_selezione),
      };
    } catch (e) {
      errore.textContent = e.message;
      return;
    }
    errore.textContent = '';
    const cambiaSpazio = configurazione.modello_embedding !== c.modello_embedding || (configurazione.dimensioni_embedding || null) !== (c.dimensioni_embedding ?? null);
    try {
      locale.ai = await chiedi({ richiesta: 'assistente_configura', configurazione });
    } catch (e) {
      errore.textContent = e instanceof ErroreServizio ? e.message : String(e);
      return;
    }
    notifica(cambiaSpazio ? 'Impostazioni salvate: con il nuovo modello di embedding i prezzari vanno indicizzati di nuovo' : 'Impostazioni salvate', 'ok');
    await caricaIndici();
    ridisegna();
  };
  return h('div', null,
    lista('ai-modelli-generazione', modelli.generazione), lista('ai-modelli-embedding', modelli.embedding),
    h('div', { class: 'modulo colonne' },
      h('label', { class: 'campo' }, h('span', null, 'Modello di generazione (Anthropic)'), generazione),
      h('label', { class: 'campo' }, h('span', null, 'Output strutturato'), modalita,
        h('small', null, `Non tutti i modelli supportano le stesse funzioni: lo strumento forzato non è accettato da ${modelli.senza_strumento_forzato.join(', ')}. Per un modello non in elenco indicare la modalità.`)),
      h('label', { class: 'campo' }, h('span', null, 'Modello di embedding (OpenAI)'), embedding),
      h('label', { class: 'campo' }, h('span', null, 'Dimensioni dei vettori'), dimensioni,
        h('small', null, 'Solo per i modelli text-embedding-3. Cambiando modello o dimensioni i prezzari vanno indicizzati di nuovo.')),
      h('label', { class: 'campo' }, h('span', null, 'Token massimi dell\'analisi'), analisi),
      h('label', { class: 'campo' }, h('span', null, 'Token massimi della selezione'), selezione)),
    errore,
    h('div', { class: 'ai-azioni' }, h('button', { class: 'pulsante primario', name: 'salva-modelli', onclick: salva }, 'Salva le impostazioni')));
}

// ---------------------------------------------------------------------------
// Operazioni lunghe

const MOTIVI = {
  consenso_mancante: 'Serve il consenso all\'invio dei dati.',
  chiave_assente: 'Manca la chiave API.',
  chiave_rifiutata: 'Il provider ha rifiutato la chiave API: controllarla nelle impostazioni.',
  quota: 'Credito o quota del provider esauriti.',
};

/** Esegue un'operazione lunga; restituisce il risultato o null (errore già mostrato). */
async function lavora(inCorso, richiesta) {
  locale.inCorso = { ...inCorso, inizio: Date.now() };
  locale.avanzamento = null;
  ridisegna();
  try {
    return await chiedi(richiesta);
  } catch (e) {
    const motivo = e instanceof ErroreServizio ? e.errore?.motivo : null;
    if (motivo === 'annullato') notifica('Operazione annullata', 'info');
    else if (MOTIVI[motivo]) {
      notifica(`${MOTIVI[motivo]} ${e.message}`, 'errore', { testo: 'Impostazioni', fn: () => { locale.scheda = 'impostazioni'; ridisegna(); } });
    } else mostraErrore(e);
    return null;
  } finally {
    locale.inCorso = null;
    if (orologio) { clearInterval(orologio); orologio = null; }
  }
}

async function annullaOperazione() {
  try {
    await chiedi({ richiesta: 'assistente_annulla' });
    const b = elVista?.querySelector('button[name="annulla-ai"]');
    if (b) { b.disabled = true; b.textContent = 'Annullamento…'; }
  } catch (e) {
    mostraErrore(e);
  }
}

function aggiornaOrologio() {
  aggiornaPulsanti();
  aggiornaAvanzamento();
  if (!locale.inCorso) return;
  const passa = () => {
    const s = elVista?.querySelector('.ai-secondi');
    if (s && locale.inCorso) s.textContent = `${Math.round((Date.now() - locale.inCorso.inizio) / 1000)} s`;
  };
  passa();
  if (!orologio) orologio = setInterval(passa, 1000);
}
