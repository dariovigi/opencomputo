// Analisi prezzi: risorse elementari, spese generali e utile predefiniti, voci analizzate
// ed editor dell'analisi di una voce (righe di risorse, voci e sotto-analisi).

import { h, sostituisci, numero, percento, euro, decimale, chiediCampi, conferma, notifica, controllo, ErroreUtente } from '../dom.js';
import { chiedi, esegui, mostraErrore, vai } from '../stato.js';
import { scegliVoce, comandiPerVoce } from '../selettore.js';

export const titolo = 'Analisi prezzi';

const TIPI = [['manodopera', 'Manodopera'], ['materiale', 'Materiale'], ['nolo', 'Nolo'], ['trasporto', 'Trasporto'], ['altro', 'Altro']];
const nomeTipo = (t) => TIPI.find(([k]) => k === t)?.[1] ?? t;
const BASI = [['costo_piu_spese_generali', 'su costo e spese generali'], ['solo_costo', 'sul solo costo']];
const testoNum = (v) => (v === null || v === undefined ? '' : String(v).replace('.', ','));

const locale = { voce: null };

export async function mostra(el, parametri) {
  if (parametri.voce !== undefined) { locale.voce = parametri.voce || null; delete parametri.voce; }
  if (locale.voce) await editor(el, locale.voce);
  else await panoramica(el);
}

/** Incidenze come etichette. */
function etichetteIncidenze(incidenze) {
  return Object.entries(incidenze ?? {}).filter(([, v]) => Number(v) !== 0)
    .map(([t, v]) => h('span', { class: 'etichetta neutra' }, `${nomeTipo(t)} ${percento(v)}`));
}

// ---------------------------------------------------------------------------------------
// Panoramica

async function panoramica(el) {
  let a;
  try { a = await chiedi({ richiesta: 'analisi_prezzi' }); } catch (e) { mostraErrore(e); return; }
  const par = a.parametri;
  const sg = controllo({ nome: 'spese_generali', valore: testoNum(par.spese_generali), numerico: true });
  const utile = controllo({ nome: 'utile_impresa', valore: testoNum(par.utile_impresa), numerico: true });
  const base = controllo({ tipo: 'select', nome: 'base_utile', valore: par.base_utile, opzioni: BASI });
  const quota = controllo({ nome: 'quota_oneri_sicurezza', valore: testoNum(par.quota_oneri_sicurezza), numerico: true, segnaposto: 'non indicata' });
  const salvaParametri = async (applica_a_tutte = false) => {
    try {
      const c = { comando: 'imposta_parametri_analisi', spese_generali: decimale(sg.value) ?? '0', utile_impresa: decimale(utile.value) ?? '0', base_utile: base.value, quota_oneri_sicurezza: decimale(quota.value) };
      if (applica_a_tutte) c.applica_a_tutte = true;
      await esegui([c]);
    } catch (e) { mostraErrore(e); }
  };
  for (const c of [sg, utile, base, quota]) c.addEventListener('change', () => salvaParametri());
  const campo = (etichetta, c, aiuto) => h('label', { class: 'campo' }, h('span', null, etichetta), c, aiuto && h('small', null, aiuto));

  const nonAllineate = a.analisi.filter((x) => x.prezzo_calcolato && Number(x.prezzo_calcolato) !== Number(x.prezzo_voce));
  const nuova = async () => {
    const v = await scegliVoce('Voce da analizzare');
    if (!v) return;
    const prima = await comandiPerVoce(v);
    if (prima.length && !(await esegui(prima))) return;
    vai('analisi', { voce: v.codice });
  };

  const schedaAnalisi = h('section', { class: 'scheda' },
    h('h2', null, 'Voci analizzate',
      h('button', { class: 'pulsante', onclick: nuova }, '+ Analisi di una voce…'),
      nonAllineate.length > 0 && h('button', { class: 'pulsante primario', onclick: () => esegui([{ comando: 'ricalcola_prezzi_analisi' }]) },
        `Aggiorna ${nonAllineate.length === 1 ? 'il prezzo' : `${nonAllineate.length} prezzi`} dalle analisi`)),
    a.analisi.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'Voce'), h('th', null, 'Descrizione'), h('th', null, 'U.M.'),
        h('th', { class: 'num' }, 'Prezzo in elenco'), h('th', { class: 'num' }, 'Prezzo da analisi'), h('th', { class: 'num' }, 'Manodopera'))),
      h('tbody', null, a.analisi.map((x) => {
        const diverso = x.prezzo_calcolato && Number(x.prezzo_calcolato) !== Number(x.prezzo_voce);
        return h('tr', { class: 'selezionabile', onclick: () => vai('analisi', { voce: x.voce }) },
          h('td', { class: 'mono' }, x.voce, x.nuovo_prezzo && h('span', { class: 'etichetta neutra', style: { marginLeft: '6px' } }, x.nuovo_prezzo)),
          h('td', null, x.descrizione), h('td', null, x.unita_misura),
          h('td', { class: 'num' }, euro(x.prezzo_voce)),
          h('td', { class: ['num', diverso && 'differente'] }, x.errore ? h('span', { class: 'etichetta errore', title: x.errore }, 'errore') : euro(x.prezzo_calcolato)),
          h('td', { class: 'num' }, percento(x.incidenze?.manodopera)));
      }))) : h('p', { class: 'vuoto' }, 'Nessuna analisi: «+ Analisi di una voce…» ne crea una per una voce dell\'elenco prezzi.'),
    nonAllineate.length > 0 && h('p', { class: 'vuoto' }, 'I prezzi in elenco cambiano solo quando si aggiornano: computi e libretto conservano comunque i prezzi fotografati.'));

  const schedaParametri = h('section', { class: 'scheda' },
    h('h2', null, 'Spese generali e utile predefiniti'),
    h('p', { class: 'nota' }, 'Valori proposti per le nuove analisi; ogni analisi può averne di propri. Nei prezzari pubblici: spese generali tra il 13 e il 17 %, utile del 10 % su costo e spese generali.'),
    h('div', { class: 'modulo colonne' },
      campo('Spese generali (%)', sg), campo('Utile d\'impresa (%)', utile), campo('Base dell\'utile', base),
      campo('Oneri aziendali della sicurezza (% delle spese generali)', quota, 'Solo evidenziati nelle analisi: non cambiano il prezzo')),
    a.analisi.length > 0 && h('div', { style: { marginTop: '10px' } },
      h('button', { class: 'pulsante', onclick: async () => { if (await conferma('Applica a tutte', 'Usare questi valori in tutte le analisi esistenti? I prezzi in elenco si aggiornano poi con «Aggiorna prezzi dalle analisi».', { ok: 'Applica' })) salvaParametri(true); } }, 'Applica a tutte le analisi…')));

  const schedaRisorse = h('section', { class: 'scheda' },
    h('h2', null, 'Risorse elementari', h('button', { class: 'pulsante', onclick: () => risorsa(null) }, '+ Risorsa')),
    a.risorse.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'Codice'), h('th', null, 'Descrizione'), h('th', null, 'U.M.'), h('th', null, 'Tipo'), h('th', { class: 'num' }, 'Prezzo'), h('th', null, 'Usata in'))),
      h('tbody', null, a.risorse.map((r) => h('tr', { class: 'selezionabile', onclick: () => risorsa(r) },
        h('td', { class: 'mono' }, r.codice), h('td', null, r.descrizione), h('td', null, r.unita_misura), h('td', null, nomeTipo(r.tipo)),
        h('td', { class: 'num' }, euro(r.prezzo)), h('td', null, r.usata_in.length ? r.usata_in.join(', ') : h('span', { class: 'vuoto' }, '—'))))))
      : h('p', { class: 'vuoto' }, 'Nessuna risorsa: manodopera, materiali, noli e trasporti con il loro prezzo unitario.'));

  sostituisci(el,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo),
      h('button', { class: 'pulsante', onclick: aggiornaPrezzi }, 'Aggiorna prezzi…'),
      h('span', { class: 'spinge' }),
      h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'analisi_prezzi' }) }, 'Stampa analisi')),
    h('div', { class: 'vista-corpo' }, schedaAnalisi, schedaRisorse, schedaParametri));
}

/** Variazione percentuale dei prezzi di risorse o voci senza analisi. */
async function aggiornaPrezzi() {
  const v = await chiediCampi('Aggiorna prezzi', [
    { nome: 'oggetto', etichetta: 'Prezzi da aggiornare', tipo: 'select', valore: 'risorse', opzioni: [['risorse', 'Risorse elementari'], ['voci', 'Voci dell\'elenco senza analisi']] },
    { nome: 'tipo', etichetta: 'Solo le risorse di tipo', tipo: 'select', valore: '', opzioni: [['', 'Tutti i tipi'], ...TIPI] },
    { nome: 'capitolo', etichetta: 'Solo le voci del capitolo', segnaposto: 'codice, es. A.01 (vuoto: tutte)' },
    { nome: 'percentuale', etichetta: 'Variazione (%)', obbligatorio: true, numerico: true, aiuto: 'Anche negativa, es. 3,5 o -2' },
    { nome: 'ricalcola', etichetta: 'Aggiorna poi i prezzi delle voci analizzate', tipo: 'checkbox', valore: true },
  ], { ok: 'Aggiorna', convalida: (x) => { try { return decimale(x.percentuale) === null ? 'Indicare la variazione.' : null; } catch (e) { return e.message; } } });
  if (!v) return;
  const c = { comando: 'aggiorna_prezzi', oggetto: v.oggetto, percentuale: decimale(v.percentuale), ricalcola_analisi: v.ricalcola };
  if (v.oggetto === 'risorse' && v.tipo) c.tipo = v.tipo;
  if (v.oggetto === 'voci' && v.capitolo.trim()) c.capitolo = v.capitolo.trim();
  const esito = await esegui([c]);
  if (esito) notifica(esito.comandi[0].descrizione, 'ok', null, 6000);
}

async function risorsa(r) {
  const v = await chiediCampi(r ? `Risorsa ${r.codice}` : 'Nuova risorsa', [
    { nome: 'codice', etichetta: 'Codice', valore: r?.codice ?? '', obbligatorio: true },
    { nome: 'descrizione', etichetta: 'Descrizione', valore: r?.descrizione ?? '', obbligatorio: true },
    { nome: 'unita_misura', etichetta: 'Unità di misura', valore: r?.unita_misura ?? '', obbligatorio: true, segnaposto: 'h, kg, m³…' },
    { nome: 'tipo', etichetta: 'Tipo', tipo: 'select', valore: r?.tipo ?? 'materiale', opzioni: TIPI },
    { nome: 'prezzo', etichetta: 'Prezzo unitario (€)', valore: testoNum(r?.prezzo), obbligatorio: true, numerico: true },
    ...(r && !r.usata_in.length ? [{ nome: 'elimina', etichetta: 'Elimina la risorsa', tipo: 'checkbox' }] : []),
  ], { ok: r ? 'Salva' : 'Aggiungi', convalida: (x) => { try { decimale(x.prezzo); return null; } catch (e) { return e.message; } } });
  if (!v) return;
  if (v.elimina) { await esegui([{ comando: 'rimuovi_risorsa', codice: r.codice }]); return; }
  const c = { comando: 'imposta_risorsa', codice: r?.codice ?? v.codice.trim(), descrizione: v.descrizione.trim(), unita_misura: v.unita_misura.trim(), tipo: v.tipo, prezzo: decimale(v.prezzo) };
  if (r && v.codice.trim() !== r.codice) c.nuovo_codice = v.codice.trim();
  const esito = await esegui([c]);
  if (esito && r && r.usata_in.length) notifica('Le analisi usano subito il nuovo prezzo: aggiornare i prezzi delle voci se serve.', 'info');
}

// ---------------------------------------------------------------------------------------
// Editor dell'analisi di una voce

/** Riga della vista → riga in ingresso del comando `imposta_analisi`. */
function ingresso(r) {
  const base = { quantita: r.quantita };
  if (r.riferimento && r.origine === 'risorsa') return { ...base, risorsa: r.riferimento };
  if (r.riferimento && (r.origine === 'voce' || r.origine === 'sotto_analisi')) return { ...base, voce: r.riferimento, tipo: r.tipo };
  return { ...base, codice: r.codice, descrizione: r.descrizione, unita_misura: r.unita_misura, tipo: r.tipo, prezzo: r.prezzo ?? '0' };
}

async function editor(el, codice) {
  let v; let risorse;
  try {
    [v, { risorse }] = await Promise.all([chiedi({ richiesta: 'voce_prezzario', codice }), chiedi({ richiesta: 'analisi_prezzi' })]);
  } catch (e) { mostraErrore(e); locale.voce = null; return; }
  const a = v.analisi;
  const righe = a ? a.righe : [];
  const salva = (extra) => esegui([{ comando: 'imposta_analisi', voce: codice, ...extra }]);
  const salvaRighe = (nuove) => salva({ righe: nuove.map(ingresso) });

  const testata = h('div', { class: 'vista-testata' },
    h('button', { class: 'pulsante', onclick: () => vai('analisi', { voce: '' }) }, '← Analisi'),
    h('h2', null, `Analisi di ${codice}`),
    v.nuovo_prezzo && h('span', { class: 'etichetta neutra' }, `NP.${v.nuovo_prezzo.numero}`),
    h('span', { class: 'spinge' }),
    a && h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'analisi_prezzi', voce: codice }) }, 'Stampa'),
    a && h('button', {
      class: 'pulsante pericolo',
      onclick: async () => { if (await conferma('Rimuovi analisi', `Rimuovere l'analisi di «${codice}»? Il prezzo resta quello attuale.`, { ok: 'Rimuovi', pericolo: true })) { await esegui([{ comando: 'rimuovi_analisi', voce: codice }]); vai('analisi', { voce: '' }); } },
    }, 'Rimuovi analisi'));

  const intestazione = h('section', { class: 'scheda' },
    h('h2', null, `${v.descrizione_breve} (${v.unita_misura})`),
    h('p', { class: 'nota' }, v.descrizione_estesa || ''),
    h('p', null, `Prezzo in elenco: ${euro(v.prezzo)} / ${v.unita_misura}`,
      a?.risultato && (a.allineata ? h('span', { class: 'etichetta ok', style: { marginLeft: '8px' } }, 'allineato all\'analisi')
        : h('span', { class: 'etichetta avviso', style: { marginLeft: '8px' } }, `da analisi ${euro(a.risultato.prezzo_unitario)}`))));

  if (!a) {
    sostituisci(el, testata, h('div', { class: 'vista-corpo' }, intestazione,
      h('section', { class: 'scheda' }, h('p', null, 'La voce non ha ancora un\'analisi.'),
        h('button', { class: 'pulsante primario', onclick: () => salva({ righe: [], applica_prezzo: false }) }, 'Crea l\'analisi'))));
    return;
  }

  // Parametri dell'analisi.
  const q = controllo({ nome: 'quantita_riferimento', valore: testoNum(a.quantita_riferimento), numerico: true });
  const sg = controllo({ nome: 'sg_analisi', valore: testoNum(a.spese_generali), numerico: true });
  const ut = controllo({ nome: 'utile_analisi', valore: testoNum(a.utile_impresa), numerico: true });
  const base = controllo({ tipo: 'select', nome: 'base_analisi', valore: a.base_utile, opzioni: BASI });
  const quota = controllo({ nome: 'quota_analisi', valore: testoNum(a.quota_oneri_sicurezza), numerico: true, segnaposto: 'non indicata' });
  quota.addEventListener('change', () => { try { salva({ quota_oneri_sicurezza: decimale(quota.value) }); } catch (e) { mostraErrore(e); } });
  const campoNum = (c, chiave) => c.addEventListener('change', () => {
    try {
      const d = decimale(c.value);
      if (d === null) throw new ErroreUtente('Valore obbligatorio');
      salva({ [chiave]: d });
    } catch (e) { mostraErrore(e); }
  });
  campoNum(q, 'quantita_riferimento');
  campoNum(sg, 'spese_generali');
  campoNum(ut, 'utile_impresa');
  base.addEventListener('change', () => salva({ base_utile: base.value }));
  const campo = (etichetta, c, aiuto) => h('label', { class: 'campo' }, h('span', null, etichetta), c, aiuto && h('small', null, aiuto));

  // Righe.
  const sposta = (i, d) => { const n = [...righe]; const [x] = n.splice(i, 1); n.splice(i + d, 0, x); salvaRighe(n); };
  const corpo = righe.map((r, i) => {
    const quantita = h('input', { type: 'text', name: `quantita_${i + 1}`, value: r.quantita, inputMode: 'decimal', autocomplete: 'off', spellcheck: false, 'aria-label': `Quantità della riga ${i + 1}` });
    quantita.addEventListener('change', () => {
      if (!quantita.value.trim()) { mostraErrore(new ErroreUtente('Indicare la quantità')); quantita.value = r.quantita; return; }
      const n = righe.map((x) => ({ ...x }));
      n[i].quantita = quantita.value.trim();
      salvaRighe(n);
    });
    quantita.addEventListener('keydown', (e) => { if (e.key === 'Enter') quantita.blur(); });
    const origine = { risorsa: 'risorsa', voce: 'voce', sotto_analisi: 'sotto-analisi', libera: 'libera' }[r.origine];
    const calcolata = r.valore_quantita && /[^0-9,.\s]/.test(r.quantita) ? h('small', { class: 'vuoto' }, ` = ${numero(r.valore_quantita, 3).replace(/,?0+$/, '').replace(/,$/, '')}`) : null;
    return h('tr', null,
      h('td', null, String(r.numero)),
      h('td', null, h('span', { class: ['etichetta', r.orfana ? 'avviso' : 'neutra'], title: r.orfana ? 'L\'origine non esiste più: vale il prezzo copiato' : '' }, origine)),
      h('td', { class: 'mono' }, r.codice), h('td', null, r.descrizione), h('td', null, nomeTipo(r.tipo)), h('td', null, r.unita_misura),
      h('td', { class: 'num cella-input' }, quantita, calcolata),
      h('td', { class: 'num' }, r.prezzo ? numero(r.prezzo, 2) : '—'),
      h('td', { class: 'num' }, r.importo ? numero(r.importo, 2) : '—'),
      h('td', { class: 'azioni-riga' },
        h('button', { class: 'pulsante icona', title: 'Su', 'aria-label': `Sposta su la riga ${i + 1}`, disabled: i === 0, onclick: () => sposta(i, -1) }, '↑'),
        h('button', { class: 'pulsante icona', title: 'Giù', 'aria-label': `Sposta giù la riga ${i + 1}`, disabled: i === righe.length - 1, onclick: () => sposta(i, 1) }, '↓'),
        h('button', { class: 'pulsante icona pericolo', title: 'Rimuovi', 'aria-label': `Rimuovi la riga ${i + 1}`, onclick: () => salvaRighe(righe.filter((_, j) => j !== i)) }, '×')));
  });

  const aggiungiRisorsa = async () => {
    if (!risorse.length) { notifica('Nessuna risorsa: aggiungerle nella pagina Analisi prezzi.', 'info'); return; }
    const x = await chiediCampi('Aggiungi una risorsa', [
      { nome: 'risorsa', etichetta: 'Risorsa', tipo: 'select', valore: risorse[0].codice, opzioni: risorse.map((r) => [r.codice, `${r.codice} — ${r.descrizione} (${numero(r.prezzo, 2)} €/${r.unita_misura})`]) },
      { nome: 'quantita', etichetta: 'Quantità', obbligatorio: true, numerico: true, aiuto: 'Numero o espressione, es. 0,5*1,2' },
    ], { ok: 'Aggiungi' });
    if (x) salvaRighe([...righe, { origine: 'risorsa', riferimento: x.risorsa, quantita: x.quantita.trim() }]);
  };
  const aggiungiVoce = async () => {
    const s = await scegliVoce('Voce o analisi da usare come elemento');
    if (!s) return;
    const x = await chiediCampi(`Aggiungi ${s.codice}`, [
      { nome: 'quantita', etichetta: 'Quantità', obbligatorio: true, numerico: true, aiuto: 'Numero o espressione' },
      { nome: 'tipo', etichetta: 'Tipo', tipo: 'select', valore: 'materiale', opzioni: TIPI, aiuto: 'Per una voce analizzata le incidenze seguono le sue risorse' },
    ], { ok: 'Aggiungi' });
    if (!x) return;
    const prima = await comandiPerVoce(s);
    const nuove = [...righe, { origine: 'voce', riferimento: s.codice, tipo: x.tipo, quantita: x.quantita.trim() }];
    esegui([...prima, { comando: 'imposta_analisi', voce: codice, righe: nuove.map(ingresso) }]);
  };
  const aggiungiLibera = async () => {
    const x = await chiediCampi('Riga libera', [
      { nome: 'codice', etichetta: 'Codice' },
      { nome: 'descrizione', etichetta: 'Descrizione', obbligatorio: true },
      { nome: 'unita_misura', etichetta: 'Unità di misura', obbligatorio: true },
      { nome: 'tipo', etichetta: 'Tipo', tipo: 'select', valore: 'materiale', opzioni: TIPI },
      { nome: 'prezzo', etichetta: 'Prezzo unitario (€)', obbligatorio: true, numerico: true },
      { nome: 'quantita', etichetta: 'Quantità', obbligatorio: true, numerico: true },
    ], { ok: 'Aggiungi', convalida: (y) => { try { decimale(y.prezzo); return null; } catch (e) { return e.message; } } });
    if (x) salvaRighe([...righe, { origine: 'libera', codice: x.codice.trim(), descrizione: x.descrizione.trim(), unita_misura: x.unita_misura.trim(), tipo: x.tipo, prezzo: decimale(x.prezzo), quantita: x.quantita.trim() }]);
  };

  const r = a.risultato;
  const riga = (testo, valore, classe) => h('tr', { class: classe }, h('td', null, testo), h('td', { class: 'num' }, euro(valore)));
  const qRif = Number(a.quantita_riferimento);
  const schedaRighe = h('section', { class: 'scheda larga' },
    h('h2', null, 'Elementi dell\'analisi',
      h('button', { class: 'pulsante', onclick: aggiungiRisorsa }, '+ Risorsa'),
      h('button', { class: 'pulsante', onclick: aggiungiVoce }, '+ Voce o analisi'),
      h('button', { class: 'pulsante', onclick: aggiungiLibera }, '+ Riga libera')),
    righe.length ? h('table', { class: 'semplice analisi' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Origine'), h('th', null, 'Codice'), h('th', null, 'Elemento'), h('th', null, 'Tipo'), h('th', null, 'U.M.'),
        h('th', { class: 'num' }, 'Quantità'), h('th', { class: 'num' }, 'Prezzo €'), h('th', { class: 'num' }, 'Importo €'), h('th'))),
      h('tbody', null, corpo)) : h('p', { class: 'vuoto' }, 'Nessun elemento: aggiungere risorse, voci dell\'elenco (anche analizzate) o righe libere.'),
    a.errore && h('div', { class: 'banner errore' }, a.errore));

  const schedaRisultato = h('section', { class: 'scheda' },
    h('h2', null, 'Prezzo'),
    h('div', { class: 'modulo colonne' },
      campo(`Quantità di riferimento (${v.unita_misura})`, q), campo('Spese generali (%)', sg), campo('Utile d\'impresa (%)', ut), campo('Base dell\'utile', base),
      campo('Oneri aziendali della sicurezza (% delle spese generali)', quota)),
    r && h('table', { class: 'semplice prospetto', style: { marginTop: '12px' } }, h('tbody', null,
      riga('Costo diretto', r.costo_diretto, 'parziale'),
      riga(`Spese generali ${percento(a.spese_generali)}`, r.spese_generali),
      r.oneri_sicurezza_aziendali && riga(`— di cui oneri aziendali della sicurezza (${percento(a.quota_oneri_sicurezza)} delle spese generali)`, r.oneri_sicurezza_aziendali),
      riga(`Utile d'impresa ${percento(a.utile_impresa)}`, r.utile),
      riga(`Totale per ${numero(a.quantita_riferimento, Number.isInteger(qRif) ? 0 : 2)} ${v.unita_misura}`, r.totale, 'parziale'),
      riga(`Prezzo unitario (${v.unita_misura})`, r.prezzo_unitario, 'totale'))),
    r && h('div', { style: { display: 'flex', gap: '6px', flexWrap: 'wrap', marginTop: '10px' } }, etichetteIncidenze(r.incidenze)),
    h('p', { class: 'vuoto' }, 'Ogni modifica aggiorna il prezzo e l\'incidenza della manodopera della voce in elenco; computi e libretto conservano i prezzi fotografati.'));

  sostituisci(el, testata, h('div', { class: 'vista-corpo' }, intestazione, schedaVariabili(a, salva), schedaRighe, schedaRisultato));
}

/** Variabili dell'analisi: nome, valore o espressione, descrizione, valore calcolato. */
function schedaVariabili(a, salva) {
  const ingressi = (lista) => lista.map((x) => ({ nome: x.nome, valore: x.espressione, descrizione: x.descrizione }));
  const chiedi_ = async (x) => {
    const v = await chiediCampi(x ? `Variabile ${x.nome}` : 'Nuova variabile', [
      { nome: 'nome', etichetta: 'Nome', valore: x?.nome ?? '', obbligatorio: true, aiuto: 'Lettere minuscole, cifre e _ (non pi, q1, q2…)' },
      { nome: 'valore', etichetta: 'Valore o espressione', valore: x?.espressione ?? '', obbligatorio: true, aiuto: 'Può usare le variabili precedenti, es. spessore*1800' },
      { nome: 'descrizione', etichetta: 'Descrizione', valore: x?.descrizione ?? '' },
    ], { ok: x ? 'Salva' : 'Aggiungi' });
    if (!v) return;
    const nuova = { nome: v.nome.trim().toLowerCase(), valore: v.valore.trim(), descrizione: v.descrizione.trim() };
    const lista = ingressi(a.variabili);
    if (x) lista[a.variabili.indexOf(x)] = nuova; else lista.push(nuova);
    salva({ variabili: lista });
  };
  return h('section', { class: 'scheda' },
    h('h2', null, 'Variabili', h('button', { class: 'pulsante', onclick: () => chiedi_(null) }, '+ Variabile')),
    h('p', { class: 'nota' }, 'Le quantità degli elementi possono usare le variabili (es. spessore, peso) e q1, q2… per la quantità delle righe precedenti.'),
    a.variabili.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'Nome'), h('th', null, 'Valore o espressione'), h('th', { class: 'num' }, 'Valore'), h('th', null, 'Descrizione'), h('th'))),
      h('tbody', null, a.variabili.map((x, i) => h('tr', null,
        h('td', { class: 'mono' }, x.nome), h('td', { class: 'mono' }, x.espressione),
        h('td', { class: 'num' }, x.valore === null || x.valore === undefined ? '—' : numero(x.valore, 3).replace(/,?0+$/, '').replace(/,$/, '')),
        h('td', null, x.descrizione),
        h('td', { class: 'azioni-riga' },
          h('button', { class: 'pulsante', 'aria-label': `Modifica la variabile ${x.nome}`, onclick: () => chiedi_(x) }, 'Modifica'),
          h('button', { class: 'pulsante icona pericolo', 'aria-label': `Rimuovi la variabile ${x.nome}`, onclick: () => salva({ variabili: ingressi(a.variabili.filter((_, j) => j !== i)) }) }, '×'))))))
      : h('p', { class: 'vuoto' }, 'Nessuna variabile.'));
}
