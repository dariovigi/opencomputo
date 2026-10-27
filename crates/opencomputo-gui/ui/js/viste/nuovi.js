// Nuovi prezzi (Allegato II.14 al D.Lgs. 36/2023, art. 5) e verbali di concordamento.

import { h, sostituisci, data, oggi, euro, decimale, chiediCampi, conferma } from '../dom.js';
import { chiedi, esegui, mostraErrore, vai } from '../stato.js';

export const titolo = 'Nuovi prezzi';

const CRITERI = [['analisi', 'Da analisi dei prezzi elementari'], ['analogia', 'Per analogia con prezzi contrattuali'], ['prezzario', 'Desunto dal prezzario']];
const nomeCriterio = (c) => CRITERI.find(([k]) => k === c)?.[1] ?? c;

export async function mostra(el, parametri) {
  let n; let elenco;
  try {
    [n, elenco] = await Promise.all([chiedi({ richiesta: 'nuovi_prezzi' }), chiedi({ richiesta: 'elenco_prezzi' })]);
  } catch (e) { mostraErrore(e); return; }
  if (parametri.voce) { const v = parametri.voce; delete parametri.voce; segnaVoce(n, elenco, v); }

  const nuovo = async () => {
    const v = await chiediCampi('Nuovo prezzo', [
      { nome: 'codice', etichetta: 'Tariffa', obbligatorio: true, valore: `NP.${String(n.nuovi_prezzi.length + 1).padStart(2, '0')}` },
      { nome: 'descrizione_breve', etichetta: 'Descrizione breve', obbligatorio: true },
      { nome: 'descrizione_estesa', etichetta: 'Descrizione estesa', tipo: 'textarea' },
      { nome: 'unita_misura', etichetta: 'Unità di misura', obbligatorio: true },
      { nome: 'criterio', etichetta: 'Criterio di formazione', tipo: 'select', valore: 'analisi', opzioni: CRITERI },
      { nome: 'prezzo', etichetta: 'Prezzo unitario (€)', numerico: true, aiuto: 'Con il criterio «analisi» si calcola dall\'analisi: si può lasciare vuoto' },
      { nome: 'data', etichetta: 'Data', tipo: 'date', valore: oggi(), obbligatorio: true },
      { nome: 'motivo', etichetta: 'Motivo', valore: 'Lavorazione non prevista nel contratto' },
      { nome: 'netto', etichetta: 'Prezzo già netto (escluso dal ribasso d\'asta)', tipo: 'checkbox' },
    ], {
      ok: 'Crea',
      convalida: (x) => {
        try {
          if (x.criterio !== 'analisi' && decimale(x.prezzo) === null) return 'Indicare il prezzo.';
          return null;
        } catch (e) { return e.message; }
      },
    });
    if (!v) return;
    const codice = v.codice.trim();
    const c = [{ comando: 'aggiungi_voce_prezzario', codice, descrizione_breve: v.descrizione_breve.trim(), unita_misura: v.unita_misura.trim(), prezzo: decimale(v.prezzo) ?? '0' },
      { comando: 'imposta_nuovo_prezzo', voce: codice, data: v.data, motivo: v.motivo.trim(), criterio: v.criterio, soggetto_a_ribasso: !v.netto }];
    if (v.descrizione_estesa.trim()) c[0].descrizione_estesa = v.descrizione_estesa;
    if (await esegui(c) && v.criterio === 'analisi') vai('analisi', { voce: codice });
  };

  const modifica = async (x) => {
    const v = await chiediCampi(`${x.sigla} — ${x.codice}`, [
      { nome: 'data', etichetta: 'Data', tipo: 'date', valore: x.data, obbligatorio: true },
      { nome: 'motivo', etichetta: 'Motivo', valore: x.motivo },
      { nome: 'criterio', etichetta: 'Criterio di formazione', tipo: 'select', valore: x.criterio, opzioni: CRITERI },
      { nome: 'netto', etichetta: 'Prezzo già netto (escluso dal ribasso d\'asta)', tipo: 'checkbox', valore: !x.soggetto_a_ribasso },
      ...(!x.approvato ? [{ nome: 'togli', etichetta: 'Non è più un nuovo prezzo', tipo: 'checkbox' }] : []),
    ], { ok: 'Salva' });
    if (!v) return;
    if (v.togli) { await esegui([{ comando: 'imposta_nuovo_prezzo', voce: x.codice, rimuovi: true }]); return; }
    await esegui([{ comando: 'imposta_nuovo_prezzo', voce: x.codice, data: v.data, motivo: v.motivo.trim(), criterio: v.criterio, soggetto_a_ribasso: !v.netto }]);
  };

  const schedaNp = h('section', { class: 'scheda larga' },
    h('h2', null, 'Nuovi prezzi',
      h('button', { class: 'pulsante primario', onclick: nuovo }, '+ Nuovo prezzo'),
      h('button', { class: 'pulsante', onclick: () => segnaVoce(n, elenco) }, 'Da una voce dell\'elenco…')),
    h('p', { class: 'nota' }, 'Prezzi di lavorazioni non previste nel contratto, concordati tra direttore dei lavori ed esecutore e approvati dal RUP. Un prezzo «già netto» è escluso dal ribasso d\'asta nei SAL.'),
    n.nuovi_prezzi.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Tariffa'), h('th', null, 'Descrizione'), h('th', null, 'U.M.'), h('th', { class: 'num' }, 'Prezzo'),
        h('th', null, 'Criterio'), h('th', null, 'Ribasso'), h('th', null, 'Verbale'), h('th', { class: 'num' }, 'Misure'), h('th'))),
      h('tbody', null, n.nuovi_prezzi.map((x) => h('tr', null,
        h('td', null, h('strong', null, x.sigla)), h('td', { class: 'mono' }, x.codice), h('td', null, x.descrizione), h('td', null, x.unita_misura),
        h('td', { class: 'num' }, euro(x.prezzo)),
        h('td', null, nomeCriterio(x.criterio), x.criterio === 'analisi' && h('button', { class: 'pulsante icona', style: { marginLeft: '6px' }, title: 'Apri l\'analisi', 'aria-label': `Analisi di ${x.codice}`, onclick: () => vai('analisi', { voce: x.codice }) }, x.analizzata ? 'analisi' : '+ analisi')),
        h('td', null, x.soggetto_a_ribasso ? 'soggetto' : h('span', { class: 'etichetta neutra' }, 'già netto')),
        h('td', null, x.verbale ? h('span', { class: ['etichetta', x.approvato ? 'ok' : 'avviso'] }, `n. ${x.verbale} ${x.approvato ? 'approvato' : 'da approvare'}`) : h('span', { class: 'vuoto' }, '—')),
        h('td', { class: 'num' }, String(x.registrazioni)),
        h('td', { class: 'azioni-riga' }, h('button', { class: 'pulsante', 'aria-label': `Modifica ${x.sigla}`, onclick: () => modifica(x) }, 'Modifica'))))))
      : h('p', { class: 'vuoto' }, 'Nessun nuovo prezzo.'));

  const schedaVerbali = h('section', { class: 'scheda larga' },
    h('h2', null, 'Verbali di concordamento',
      h('button', { class: 'pulsante', onclick: () => verbale(n, null), disabled: !n.nuovi_prezzi.length }, '+ Verbale')),
    n.verbali.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Data'), h('th', null, 'Nuovi prezzi'), h('th', null, 'Firma esecutore'), h('th', null, 'Approvazione'), h('th'))),
      h('tbody', null, n.verbali.map((v) => h('tr', null,
        h('td', null, String(v.numero)), h('td', null, data(v.data)),
        h('td', null, v.nuovi_prezzi.map((c) => n.nuovi_prezzi.find((x) => x.codice === c)?.sigla ?? c).join(', ') || h('span', { class: 'vuoto' }, 'nessuno')),
        h('td', null, v.firma_esecutore ? `${data(v.firma_esecutore)}${v.riserve ? ' (con riserve)' : ''}` : h('span', { class: 'etichetta avviso' }, 'non firmato')),
        h('td', null, v.approvato_il ? h('span', { class: 'etichetta ok' }, `approvato il ${data(v.approvato_il)}`) : h('span', { class: 'etichetta neutra' }, 'da approvare')),
        h('td', { class: 'azioni-riga' },
          h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'nuovi_prezzi', numero: v.numero }) }, 'Stampa'),
          h('button', { class: 'pulsante', 'aria-label': `Modifica il verbale ${v.numero}`, onclick: () => verbale(n, v) }, 'Modifica'))))))
      : h('p', { class: 'vuoto' }, 'Nessun verbale.'));

  sostituisci(el,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo), h('span', { class: 'spinge' }),
      h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'nuovi_prezzi' }), disabled: !n.nuovi_prezzi.length }, 'Stampa elenco')),
    h('div', { class: 'vista-corpo' }, schedaNp, schedaVerbali));
}

/** Segna come nuovo prezzo una voce già nell'elenco. */
async function segnaVoce(n, elenco, codice = null) {
  const candidate = elenco.voci.filter((v) => !v.nuovo_prezzo);
  if (!candidate.length) return;
  const v = await chiediCampi('Voce dell\'elenco come nuovo prezzo', [
    { nome: 'voce', etichetta: 'Voce', tipo: 'select', valore: codice ?? candidate[0].codice, opzioni: candidate.map((x) => [x.codice, `${x.codice} — ${x.descrizione_breve}`]) },
    { nome: 'criterio', etichetta: 'Criterio di formazione', tipo: 'select', valore: 'analogia', opzioni: CRITERI },
    { nome: 'data', etichetta: 'Data', tipo: 'date', valore: oggi(), obbligatorio: true },
    { nome: 'motivo', etichetta: 'Motivo' },
    { nome: 'netto', etichetta: 'Prezzo già netto (escluso dal ribasso d\'asta)', tipo: 'checkbox' },
  ], { ok: 'Segna come nuovo prezzo' });
  if (v) await esegui([{ comando: 'imposta_nuovo_prezzo', voce: v.voce, data: v.data, motivo: v.motivo.trim(), criterio: v.criterio, soggetto_a_ribasso: !v.netto }]);
}

/** Nuovo verbale o modifica: data, nuovi prezzi compresi, firma, riserve, approvazione. */
async function verbale(n, v) {
  const liberi = n.nuovi_prezzi.filter((x) => !x.verbale || x.verbale === v?.numero);
  const campi = [
    { nome: 'data', etichetta: 'Data del verbale', tipo: 'date', valore: v?.data ?? oggi(), obbligatorio: true },
    ...liberi.map((x) => ({ nome: `np_${x.codice}`, etichetta: `${x.sigla} ${x.codice} — ${x.descrizione}`, tipo: 'checkbox', valore: v ? v.nuovi_prezzi.includes(x.codice) : true })),
    { nome: 'firma', etichetta: 'Firma dell\'esecutore', tipo: 'date', valore: v?.firma_esecutore ?? '' },
    { nome: 'riserve', etichetta: 'Riserve dell\'esecutore', tipo: 'textarea', righe: 2, valore: v?.riserve ?? '' },
    { nome: 'approvato', etichetta: 'Approvato il', tipo: 'date', valore: v?.approvato_il ?? '', aiuto: 'Dal RUP, o dalla stazione appaltante se i nuovi prezzi comportano maggiori spese' },
    { nome: 'atto', etichetta: 'Atto di approvazione', valore: v?.atto_approvazione ?? '' },
    { nome: 'note', etichetta: 'Note', tipo: 'textarea', righe: 2, valore: v?.note ?? '' },
    ...(v && !v.approvato_il ? [{ nome: 'elimina', etichetta: 'Elimina il verbale', tipo: 'checkbox' }] : []),
  ];
  const x = await chiediCampi(v ? `Verbale n. ${v.numero}` : 'Nuovo verbale di concordamento', campi, {
    ok: v ? 'Salva' : 'Registra',
    convalida: (y) => (liberi.some((p) => y[`np_${p.codice}`]) || y.elimina ? null : 'Scegliere almeno un nuovo prezzo.'),
  });
  if (!x) return;
  if (x.elimina) {
    if (await conferma('Elimina verbale', `Eliminare il verbale n. ${v.numero}? I nuovi prezzi restano senza verbale.`, { ok: 'Elimina', pericolo: true })) {
      await esegui([{ comando: 'rimuovi_verbale_nuovi_prezzi', numero: v.numero }]);
    }
    return;
  }
  const c = {
    comando: 'imposta_verbale_nuovi_prezzi', data: x.data,
    nuovi_prezzi: liberi.filter((p) => x[`np_${p.codice}`]).map((p) => p.codice),
    firma_esecutore: x.firma || null, riserve: x.riserve.trim() || null,
    approvato_il: x.approvato || null, atto_approvazione: x.atto.trim() || null, note: x.note.trim() || null,
  };
  if (v) c.numero = v.numero;
  await esegui([c]);
}
