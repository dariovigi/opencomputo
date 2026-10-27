// Controlli: esito della validazione completa del progetto.

import { h, sostituisci } from '../dom.js';
import { chiedi, mostraErrore, percorso, vai } from '../stato.js';

export const titolo = 'Controlli';

let filtro = 'tutti';

/** Vista da aprire per un problema, se c'è un'entità a cui portare. */
function destinazione(p, computi) {
  const rif = p.percorso.find((r) => r.tipo === 'computo');
  const computo = rif && computi.find((c) => c.id === rif.id)?.nome;
  const voce = p.percorso.find((r) => r.tipo === 'voce_computo');
  if (computo && voce) return () => vai('computo', { computo, voce: voce.numero });
  if (computo) return () => vai('computo', { computo });
  if (p.percorso.some((r) => r.tipo === 'conto_finale')) return () => vai('conto');
  if (p.percorso.some((r) => r.tipo === 'registrazione')) return () => vai('libretto');
  if (p.percorso.some((r) => r.tipo === 'sal')) return () => vai('sal');
  if (p.percorso.some((r) => r.tipo === 'voce_prezzario')) return () => vai('elenco');
  return null;
}

export async function mostra(el) {
  let r;
  let computi;
  try {
    [r, { computi }] = await Promise.all([chiedi({ richiesta: 'valida' }), chiedi({ richiesta: 'progetto' })]);
  } catch (e) { mostraErrore(e); return; }
  const disegna = () => {
    const elenco_ = r.problemi.filter((p) => filtro === 'tutti' || p.gravita === filtro);
    sostituisci(el,
      h('div', { class: 'vista-testata' },
        h('h2', null, titolo),
        h('div', { class: 'schede-tab', role: 'tablist' }, [['tutti', `Tutti (${r.problemi.length})`], ['errore', `Errori (${r.errori})`], ['avviso', `Avvisi (${r.avvisi})`], ['informazione', 'Informazioni']]
          .map(([k, t]) => h('button', { role: 'tab', class: filtro === k && 'corrente', onclick: () => { filtro = k; disegna(); } }, t)))),
      h('div', { class: 'vista-corpo' },
        r.valido
          ? h('div', { class: 'banner info' }, r.problemi.length ? 'Nessun errore: il progetto è calcolabile. Restano le segnalazioni qui sotto.' : 'Nessun problema rilevato.')
          : h('div', { class: 'banner errore' }, `${r.errori} errori: alcuni importi non sono calcolabili o i dati sono incoerenti.`),
        elenco_.length ? h('ul', { class: 'elenco-problemi scheda', style: { padding: 0 } }, elenco_.map((p) => {
          const vai_ = destinazione(p, computi);
          return h('li', { class: [p.gravita, vai_ && 'navigabile'], onclick: vai_ || undefined, title: vai_ ? 'Vai' : undefined },
            h('div', null, p.messaggio),
            h('small', null, percorso(p.percorso) || 'progetto', ' · ', h('code', null, p.codice)));
        })) : null));
  };
  disegna();
}
