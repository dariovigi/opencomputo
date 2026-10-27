// Dati generali: metadati, contratto, regole di arrotondamento. Ogni campo si applica
// quando si lascia il campo (un passo di annullamento per modifica).

import { h, sostituisci, decimale, controllo, euro, ErroreUtente } from '../dom.js';
import { chiedi, esegui, mostraErrore } from '../stato.js';

export const titolo = 'Dati generali';

const METADATI = [
  ['titolo', 'Titolo dei lavori', true],
  ['oggetto', 'Oggetto'],
  ['committente', 'Committente'],
  ['impresa', 'Impresa'],
  ['luogo', 'Luogo'],
  ['cig', 'CIG'],
  ['cup', 'CUP'],
  ['responsabile_procedimento', 'Responsabile del procedimento'],
  ['progettista', 'Progettista'],
  ['direttore_lavori', 'Direttore dei lavori'],
];

function campo(etichetta, el, { largo = false, aiuto = null } = {}) {
  return h('label', { class: ['campo', largo && 'largo', el.type === 'checkbox' && 'spunta'] }, h('span', null, etichetta), el, aiuto && h('small', null, aiuto));
}

/** Applica un comando quando il controllo cambia; ripristina il valore se rifiutato. */
function applica(el, originale, comando) {
  el.addEventListener('change', async () => {
    let c;
    try {
      c = comando(el.type === 'checkbox' ? el.checked : el.value);
    } catch (e) {
      mostraErrore(e);
      if (el.type === 'checkbox') el.checked = originale; else el.value = originale ?? '';
      return;
    }
    if (!c) return;
    const esito = await esegui([c]);
    if (!esito) { if (el.type === 'checkbox') el.checked = originale; else el.value = originale ?? ''; }
  });
  return el;
}

export async function mostra(el) {
  const p = await chiedi({ richiesta: 'progetto' });
  const m = p.metadati;
  const contratto = p.contratto;
  const par = contratto?.parametri;
  const r = p.regole;

  const schedaProgetto = h('section', { class: 'scheda' },
    h('h2', null, 'Progetto'),
    h('div', { class: 'modulo colonne' }, METADATI.map(([chiave, etichetta, obbligatorio]) => {
      const input = controllo({ nome: chiave, valore: m[chiave] ?? '' });
      applica(input, m[chiave] ?? '', (v) => {
        if (obbligatorio && !v.trim()) throw new ErroreUtente(`Il campo «${etichetta}» non può essere vuoto.`);
        return { comando: 'imposta_metadati', [chiave]: v.trim() };
      });
      return campo(etichetta, input, { largo: chiave === 'titolo' || chiave === 'oggetto' });
    })));

  const percent = (chiave, etichetta, valore, aiuto) => {
    const input = controllo({ nome: chiave, valore: valore ? String(valore).replace('.', ',') : '', numerico: true, segnaposto: '0,00' });
    applica(input, valore ? String(valore).replace('.', ',') : '', (v) => ({ comando: 'imposta_contratto', [chiave]: decimale(v) ?? '0' }));
    return campo(etichetta, input, { aiuto });
  };
  const importoContr = controllo({ nome: 'importo_contrattuale', valore: contratto?.importo_contrattuale ? String(contratto.importo_contrattuale).replace('.', ',') : '', numerico: true });
  applica(importoContr, importoContr.value, (v) => {
    const d = decimale(v);
    return d === null ? null : { comando: 'imposta_contratto', importo_contrattuale: d };
  });
  const consegna = h('input', { type: 'date', name: 'data_consegna_lavori', value: contratto?.data_consegna_lavori ?? '' });
  applica(consegna, consegna.value, (v) => (v ? { comando: 'imposta_contratto', data_consegna_lavori: v } : null));
  const ivaAttuale = par?.aliquota_iva ? String(Number(par.aliquota_iva)) : '';
  const iva = controllo({
    tipo: 'select', nome: 'aliquota_iva', valore: ivaAttuale,
    opzioni: [['', 'Non indicata'], ['4', '4 %'], ['5', '5 %'], ['10', '10 %'], ['22', '22 %'],
      ...(ivaAttuale && !['4', '5', '10', '22'].includes(ivaAttuale) ? [[ivaAttuale, `${ivaAttuale.replace('.', ',')} %`]] : [])],
  });
  applica(iva, ivaAttuale, (v) => ({ comando: 'imposta_contratto', aliquota_iva: v === '' ? null : v }));
  const esclusa = controllo({ tipo: 'checkbox', nome: 'mo', valore: par?.manodopera_esclusa_dal_ribasso });
  applica(esclusa, !!par?.manodopera_esclusa_dal_ribasso, (v) => ({ comando: 'imposta_contratto', manodopera_esclusa_dal_ribasso: v }));

  const schedaContratto = h('section', { class: 'scheda' },
    h('h2', null, 'Contratto', !contratto && h('span', { class: 'etichetta avviso' }, 'non impostato')),
    h('p', { class: 'nota' }, 'Parametri usati da libretto, registro e SAL. Non modificabili se alterano un SAL già emesso.'),
    h('div', { class: 'modulo colonne' },
      percent('ribasso', 'Ribasso d\'asta (%)', par?.ribasso),
      percent('ritenuta_garanzia', 'Ritenuta di garanzia (%)', par?.ritenuta_garanzia, 'Di norma 0,50 %'),
      campo('Importo contrattuale (€)', importoContr),
      campo('Consegna dei lavori', consegna),
      campo('IVA dei certificati di pagamento', iva, { aiuto: 'Dipende dal tipo di opera: verificare il contratto' }),
      campo('Manodopera esclusa dal ribasso', esclusa)));

  // Anticipazione del prezzo (art. 125 D.Lgs. 36/2023).
  const ant = par?.anticipazione;
  const testoNum = (v) => (v === null || v === undefined ? '' : String(v).replace('.', ','));
  const campiAnt = {
    percentuale: controllo({ nome: 'ant_percentuale', valore: testoNum(ant?.percentuale ?? '20'), numerico: true }),
    importo: controllo({ nome: 'ant_importo', valore: testoNum(ant?.importo), numerico: true, segnaposto: 'calcolato sull\'importo contrattuale' }),
    percentuale_recupero: controllo({ nome: 'ant_recupero', valore: testoNum(ant?.percentuale_recupero), numerico: true, segnaposto: 'uguale alla percentuale' }),
    data_erogazione: h('input', { type: 'date', name: 'ant_data', value: ant?.data_erogazione ?? '' }),
  };
  const impostaAnticipazione = async () => {
    try {
      const c = { comando: 'imposta_anticipazione' };
      const perc = decimale(campiAnt.percentuale.value);
      if (perc !== null) c.percentuale = perc;
      // L'importo scritto a mano vale così com'è; altrimenti si calcola dalla percentuale.
      const importo = decimale(campiAnt.importo.value);
      if (importo !== null && campiAnt.importo.value.trim() !== testoNum(ant?.importo)) c.importo = importo;
      const rec = decimale(campiAnt.percentuale_recupero.value);
      if (rec !== null) c.percentuale_recupero = rec;
      if (campiAnt.data_erogazione.value) c.data_erogazione = campiAnt.data_erogazione.value;
      await esegui([c]);
    } catch (e) { mostraErrore(e); }
  };
  const schedaAnticipazione = h('section', { class: 'scheda' },
    h('h2', null, 'Anticipazione del prezzo', ant ? h('span', { class: 'etichetta ok' }, euro(ant.importo)) : h('span', { class: 'etichetta neutra' }, 'non prevista')),
    h('p', { class: 'nota' }, 'Di norma il 20 % del valore del contratto (fino al 30 % se previsto dai documenti di gara). Viene recuperata nei certificati di pagamento in proporzione all\'importo netto progressivo; il contratto può prevedere una percentuale diversa.'),
    h('div', { class: 'modulo colonne' },
      campo('Percentuale (%)', campiAnt.percentuale),
      campo('Importo erogato (€)', campiAnt.importo, { aiuto: 'Vuoto: percentuale dell\'importo contrattuale' }),
      campo('Recupero sui SAL (%)', campiAnt.percentuale_recupero),
      campo('Data di erogazione', campiAnt.data_erogazione)),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '12px' } },
      h('button', { class: 'pulsante primario', onclick: impostaAnticipazione }, ant ? 'Aggiorna anticipazione' : 'Imposta anticipazione'),
      ant && h('button', { class: 'pulsante pericolo', onclick: () => esegui([{ comando: 'imposta_anticipazione', rimuovi: true }]) }, 'Rimuovi')));

  const dec = (chiave, etichetta, valore) => {
    const s = controllo({ tipo: 'select', nome: chiave, valore, opzioni: [0, 1, 2, 3, 4, 5, 6].map((n) => [n, String(n)]) });
    applica(s, String(valore), (v) => ({ comando: 'imposta_regole', [chiave]: Number(v) }));
    return campo(etichetta, s);
  };
  const modalita = controllo({ tipo: 'select', nome: 'modalita', valore: r.modalita, opzioni: [['commerciale', 'Commerciale (metà per eccesso)'], ['meta_al_pari', 'Metà al pari (bancario)'], ['troncamento', 'Troncamento']] });
  applica(modalita, r.modalita, (v) => ({ comando: 'imposta_regole', modalita: v }));
  const schedaRegole = h('section', { class: 'scheda' },
    h('h2', null, 'Arrotondamenti'),
    h('p', { class: 'nota' }, 'Le quantità sono arrotondate per rigo, gli importi per voce.'),
    h('div', { class: 'modulo colonne' },
      dec('decimali_quantita', 'Decimali delle quantità', r.decimali_quantita),
      dec('decimali_prezzo', 'Decimali dei prezzi calcolati', r.decimali_prezzo),
      dec('decimali_importo', 'Decimali degli importi', r.decimali_importo),
      campo('Modalità', modalita)));

  sostituisci(el,
    h('div', { class: 'vista-testata' }, h('h2', null, titolo)),
    h('div', { class: 'vista-corpo' }, schedaProgetto, schedaContratto, schedaAnticipazione, schedaRegole));
}

