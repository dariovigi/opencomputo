// Categorie (tre livelli), nodi WBS e computi del progetto.

import { h, sostituisci, chiediCampi, conferma } from '../dom.js';
import { chiedi, esegui, mostraErrore, vai } from '../stato.js';

export const titolo = 'Categorie, WBS e computi';

const LIVELLI = [
  ['super_categoria', 'Super categorie', 'super_categorie'],
  ['categoria', 'Categorie', 'categorie'],
  ['sotto_categoria', 'Sotto categorie', 'sotto_categorie'],
];

const TIPI = [
  ['computo_metrico_estimativo', 'Computo metrico estimativo'],
  ['computo_metrico', 'Computo metrico'],
  ['preventivo', 'Preventivo'],
  ['perizia_variante', 'Perizia di variante'],
];

export async function mostra(el) {
  let p;
  try { p = await chiedi({ richiesta: 'progetto' }); } catch (e) { mostraErrore(e); return; }

  const azioni = (modifica, rimuovi, nome) => h('td', { class: 'azioni-riga' },
    h('button', { class: 'pulsante icona', title: `Modifica ${nome}`, 'aria-label': `Modifica ${nome}`, onclick: modifica }, '✎'),
    h('button', { class: 'pulsante icona pericolo', title: `Rimuovi ${nome}`, 'aria-label': `Rimuovi ${nome}`, onclick: rimuovi }, '×'));

  const modificaClasse = async (livello, c) => {
    const v = await chiediCampi(`Modifica «${c.codice}»`, [
      { nome: 'codice', etichetta: 'Codice', valore: c.codice, obbligatorio: true },
      { nome: 'descrizione', etichetta: 'Descrizione', valore: c.descrizione, obbligatorio: true },
    ]);
    if (!v) return;
    const cmd = { comando: 'modifica_categoria', livello, codice: c.codice, descrizione: v.descrizione.trim() };
    if (v.codice.trim() !== c.codice) cmd.nuovo_codice = v.codice.trim();
    esegui([cmd]);
  };
  const rimuoviClasse = async (livello, c) => {
    if (await conferma('Rimuovi', `Rimuovere «${c.codice} — ${c.descrizione}»? È possibile solo se non è assegnata a nessuna voce.`, { ok: 'Rimuovi', pericolo: true })) {
      esegui([{ comando: 'rimuovi_categoria', livello, codice: c.codice }]);
    }
  };

  const tabellaClassi = (livello, elenco_) => elenco_.length
    ? h('table', { class: 'semplice' }, h('thead', null, h('tr', null, h('th', { style: { width: '120px' } }, 'Codice'), h('th', null, 'Descrizione'), h('th', { style: { width: '90px' } }))),
      h('tbody', null, elenco_.map((c) => h('tr', null, h('td', { class: 'mono' }, c.codice), h('td', null, c.descrizione),
        azioni(() => modificaClasse(livello, c), () => rimuoviClasse(livello, c), c.codice)))))
    : h('p', { class: 'vuoto' }, 'Nessuna.');

  const modificaNodo = async (n) => {
    const v = await chiediCampi(`Modifica il nodo «${n.codice_completo}»`, [
      { nome: 'codice', etichetta: 'Codice (relativo al nodo padre)', valore: n.codice, obbligatorio: true },
      { nome: 'descrizione', etichetta: 'Descrizione', valore: n.descrizione, obbligatorio: true },
    ]);
    if (!v) return;
    const cmd = { comando: 'modifica_nodo_wbs', codice: n.codice_completo, descrizione: v.descrizione.trim() };
    if (v.codice.trim() !== n.codice) cmd.nuovo_codice = v.codice.trim();
    esegui([cmd]);
  };
  const rimuoviNodo = async (n) => {
    if (await conferma('Rimuovi nodo', `Rimuovere il nodo «${n.codice_completo} — ${n.descrizione}»? È possibile solo se non ha figli e non è assegnato.`, { ok: 'Rimuovi', pericolo: true })) {
      esegui([{ comando: 'rimuovi_nodo_wbs', codice: n.codice_completo }]);
    }
  };

  const nuovaClasse = async (livello, nome) => {
    const v = await chiediCampi(`Nuova voce in «${nome}»`, [
      { nome: 'codice', etichetta: 'Codice', obbligatorio: true },
      { nome: 'descrizione', etichetta: 'Descrizione', obbligatorio: true },
    ], { ok: 'Aggiungi' });
    if (v) esegui([{ comando: 'aggiungi_categoria', livello, codice: v.codice.trim(), descrizione: v.descrizione.trim() }]);
  };

  const nuovoNodo = async () => {
    const v = await chiediCampi('Nuovo nodo WBS', [
      { nome: 'padre', etichetta: 'Sotto il nodo', tipo: 'select', valore: '', opzioni: [['', '(radice)'], ...p.wbs.map((n) => [n.codice_completo, `${n.codice_completo} — ${n.descrizione}`])] },
      { nome: 'codice', etichetta: 'Codice (relativo al nodo padre)', obbligatorio: true, segnaposto: '01' },
      { nome: 'descrizione', etichetta: 'Descrizione', obbligatorio: true },
    ], { ok: 'Aggiungi' });
    if (!v) return;
    const c = { comando: 'aggiungi_nodo_wbs', codice: v.codice.trim(), descrizione: v.descrizione.trim() };
    if (v.padre) c.padre = v.padre;
    esegui([c]);
  };

  const nuovoComputo = async () => {
    const v = await chiediCampi('Nuovo computo', [
      { nome: 'nome', etichetta: 'Nome', obbligatorio: true },
      { nome: 'tipo', etichetta: 'Tipo', tipo: 'select', valore: 'computo_metrico_estimativo', opzioni: TIPI },
    ], { ok: 'Crea' });
    if (!v) return;
    const esito = await esegui([{ comando: 'nuovo_computo', nome: v.nome.trim(), tipo: v.tipo }]);
    if (esito) vai('computo', { computo: v.nome.trim() });
  };

  sostituisci(el,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo)),
    h('div', { class: 'vista-corpo' },
      h('section', { class: 'scheda' },
        h('h2', null, 'Computi', h('button', { class: 'pulsante', onclick: nuovoComputo }, '+ Nuovo computo')),
        h('table', { class: 'semplice' }, h('thead', null, h('tr', null, h('th', null, 'Nome'), h('th', null, 'Tipo'), h('th', { class: 'num' }, 'Voci'))),
          h('tbody', null, p.computi.map((c) => h('tr', { class: 'selezionabile', onclick: () => vai('computo', { computo: c.nome }) },
            h('td', null, c.nome), h('td', null, TIPI.find(([k]) => k === c.tipo)?.[1] ?? c.tipo), h('td', { class: 'num' }, String(c.voci))))))),
      LIVELLI.map(([livello, nome, chiave]) => h('section', { class: 'scheda' },
        h('h2', null, nome, h('button', { class: 'pulsante', onclick: () => nuovaClasse(livello, nome) }, '+ Aggiungi')),
        tabellaClassi(livello, p[chiave]))),
      h('section', { class: 'scheda' },
        h('h2', null, 'WBS (struttura di scomposizione del lavoro)', h('button', { class: 'pulsante', onclick: nuovoNodo }, '+ Aggiungi nodo')),
        p.wbs.length
          ? h('table', { class: 'semplice' }, h('thead', null, h('tr', null, h('th', { style: { width: '160px' } }, 'Codice'), h('th', null, 'Descrizione'), h('th', { style: { width: '90px' } }))),
            h('tbody', null, p.wbs.map((n) => h('tr', null,
              h('td', { class: 'mono', style: { paddingLeft: 10 + n.livello * 18 + 'px' } }, n.codice_completo),
              h('td', null, n.descrizione),
              azioni(() => modificaNodo(n), () => rimuoviNodo(n), n.codice_completo)))))
          : h('p', { class: 'vuoto' }, 'Nessun nodo.')),
      p.capitoli.length ? h('section', { class: 'scheda' },
        h('h2', null, `Capitoli dell'elenco prezzi (${p.capitoli.length})`),
        h('table', { class: 'semplice' }, h('tbody', null, p.capitoli.slice(0, 300).map((c) => h('tr', null, h('td', { class: 'mono', style: { width: '160px' } }, c.codice), h('td', null, c.descrizione))))),
        p.capitoli.length > 300 ? h('p', { class: 'vuoto' }, `… e altri ${p.capitoli.length - 300}.`) : null) : null));
}
