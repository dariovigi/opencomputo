// Scelta di una voce di prezzario: dall'elenco prezzi del progetto o da un prezzario di
// consultazione caricato nella sessione (in quel caso la voce viene prima copiata
// nell'elenco prezzi, nella stessa richiesta atomica).

import { h, modale, numero, rimanda, sostituisci, natura } from './dom.js';
import { chiedi, stato, mostraErrore } from './stato.js';
import { griglia } from './griglia.js';

export const COLONNE_VOCE = [
  { titolo: 'Tariffa', larghezza: '130px', valore: (v) => v.codice, classe: () => 'mono' },
  { titolo: 'Descrizione', larghezza: 'minmax(200px, 1fr)', valore: (v) => v.descrizione_breve, suggerimento: true },
  { titolo: 'U.M.', larghezza: '60px', allinea: 'centro', valore: (v) => v.unita_misura },
  { titolo: 'Prezzo €', larghezza: '96px', allinea: 'destra', valore: (v) => numero(v.prezzo, 2) },
];

/**
 * Restituisce {codice, prezzario} della voce scelta, o null.
 * `prezzario` è null per l'elenco del progetto, altrimenti il nome del prezzario caricato.
 */
export async function scegliVoce(titolo = 'Scegli una voce') {
  const fonti = [[null, 'Elenco prezzi del progetto'], ...(stato.info?.prezzari_caricati ?? []).map((p) => [p.nome, `Prezzario «${p.nome}» (${p.voci} voci)`])];
  let fonte = null;
  let scelta = null;
  const anteprima = h('div', { class: 'anteprima-voce' });
  const conteggio = h('small', { class: 'vuoto' });
  const g = griglia({
    colonne: COLONNE_VOCE,
    etichetta: 'Voci',
    vuoto: 'Nessuna voce corrisponde alla ricerca',
    suSelezione: async (_, v) => {
      scelta = v;
      sostituisci(anteprima, h('strong', null, `${v.codice} — ${natura(v.natura)}`));
      try {
        const d = await chiedi({ richiesta: 'voce_prezzario', codice: v.codice, prezzario: fonte });
        if (scelta === v) sostituisci(anteprima, h('strong', null, `${v.codice}  `), d.descrizione_estesa || d.descrizione_breve);
      } catch { /* anteprima facoltativa */ }
    },
    suAttiva: (v) => { scelta = v; conferma(); },
  });
  const cerca = h('input', { type: 'search', placeholder: 'Cerca per codice o parole della descrizione…', 'aria-label': 'Cerca' });
  const selFonte = h('select', { 'aria-label': 'Prezzario' }, fonti.map(([v, t]) => h('option', { value: v ?? '' }, t)));
  let richiesta = 0;
  // Ricerca in attesa (digitazione o caricamento): la conferma la aspetta, così Invio
  // subito dopo aver scritto sceglie tra i risultati aggiornati.
  let pendente = Promise.resolve();
  let sblocca = null;
  const carica = async () => {
    const n = ++richiesta;
    try {
      const r = await chiedi({ richiesta: 'elenco_prezzi', prezzario: fonte, testo: cerca.value });
      if (n !== richiesta) return;
      g.imposta(r.voci);
      conteggio.textContent = `${r.voci.length} di ${r.totale} voci`;
      if (r.voci.length) g.seleziona(0); else scelta = null;
    } catch (e) { mostraErrore(e); }
  };
  const caricaRimandato = rimanda(async () => { await carica(); sblocca?.(); sblocca = null; }, 180);
  cerca.addEventListener('input', () => {
    if (!sblocca) pendente = new Promise((ok) => { sblocca = ok; });
    caricaRimandato();
  });
  cerca.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); g.fuoco(); }
  });
  selFonte.addEventListener('change', () => { fonte = selFonte.value || null; carica(); });
  let conferma = () => {};
  const promessa = modale({
    titolo,
    larga: true,
    contenuto: h('div', { class: 'selettore' },
      h('div', { class: 'riga-filtri' }, cerca, fonti.length > 1 ? selFonte : null),
      g.el, conteggio, anteprima),
    pulsanti: [{ testo: 'Annulla', valore: null }, { testo: 'Scegli', valore: 'ok', tipo: 'primario' }],
    prima: async () => { await pendente; return !!scelta; },
  });
  const pulsante = document.querySelector('.modale .modale-pulsanti .primario');
  conferma = () => pulsante?.click();
  pendente = carica();
  await pendente;
  cerca.focus();
  const r = await promessa;
  return r && scelta ? { codice: scelta.codice, prezzario: fonte } : null;
}

/** Comandi che rendono disponibile nell'elenco prezzi la voce scelta (copia se manca). */
export async function comandiPerVoce(v) {
  if (!v.prezzario) return [];
  try {
    await chiedi({ richiesta: 'voce_prezzario', codice: v.codice });
    return []; // già nell'elenco prezzi del progetto
  } catch {
    return [{ comando: 'copia_da_prezzario', prezzario: v.prezzario, codici: [v.codice] }];
  }
}
