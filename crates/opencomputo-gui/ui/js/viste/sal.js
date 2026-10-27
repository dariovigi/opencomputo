// Stati di avanzamento lavori: creazione in bozza, prospetto, emissione, annullamento.

import { h, sostituisci, data, oggi, euro, percento, numero, sommaImporti, chiediCampi, conferma, decimale } from '../dom.js';
import { chiedi, esegui, mostraErrore, vai } from '../stato.js';

export const titolo = 'Stati di avanzamento lavori';

let scelto = null;

const RIGHE_PROSPETTO = [
  ['lordo_periodo', 'Lavori del periodo (lordo)'],
  ['lavori_lordi', 'Lavori a tutto il SAL (lordo)', 'parziale'],
  ['oneri_sicurezza', '— di cui oneri della sicurezza (non soggetti a ribasso)'],
  ['manodopera_esclusa', '— di cui manodopera esclusa dal ribasso'],
  ['lavori_esclusi_dal_ribasso', '— di cui nuovi prezzi esclusi dal ribasso'],
  ['soggetto_a_ribasso', 'Importo soggetto a ribasso'],
  ['ribasso', 'Ribasso d\'asta'],
  ['importo_netto', 'Importo netto dei lavori', 'parziale'],
  ['ritenuta_garanzia', 'Ritenuta di garanzia'],
  ['credito_progressivo', 'Credito dell\'impresa a tutto il SAL', 'parziale'],
  ['certificati_precedenti', 'A dedurre: certificati precedenti'],
  ['importo_sal', 'Importo del presente SAL', 'totale'],
];

export async function mostra(el, parametri) {
  let k;
  try { k = await chiedi({ richiesta: 'contabilita' }); } catch (e) { mostraErrore(e); return; }
  if (parametri.numero) { scelto = Number(parametri.numero); delete parametri.numero; }
  if (!k.sal.some((s) => s.numero === scelto)) scelto = k.sal.at(-1)?.numero ?? null;
  const s = k.sal.find((x) => x.numero === scelto);
  const bozza = k.sal.find((x) => x.stato === 'bozza');
  const daContabilizzare = k.registrazioni.filter((r) => !r.sal);

  const nuovoSal = async () => {
    const v = await chiediCampi('Nuovo SAL', [
      { nome: 'data', etichetta: 'Lavori eseguiti a tutto il', tipo: 'date', valore: oggi(), obbligatorio: true, aiuto: 'Il SAL comprende le registrazioni non ancora contabilizzate fino a questa data.' },
    ], { ok: 'Crea in bozza' });
    if (!v) return;
    scelto = (k.sal.at(-1)?.numero ?? 0) + 1;
    await esegui([{ comando: 'nuovo_sal', lavori_a_tutto_il: v.data }]);
  };
  const emetti = async () => {
    const v = await chiediCampi(`Emissione del SAL n. ${s.numero}`, [
      { nome: 'data', etichetta: 'Data di emissione', tipo: 'date', valore: oggi(), obbligatorio: true, aiuto: 'Dopo l\'emissione il prospetto è congelato e le registrazioni comprese non sono più modificabili. L\'operazione si può annullare solo con «Annulla» subito dopo.' },
    ], { ok: 'Emetti' });
    if (v) await esegui([{ comando: 'emetti_sal', numero: s.numero, data_emissione: v.data }]);
  };
  const annullaBozza = async () => {
    if (await conferma('Annulla bozza', `Annullare la bozza del SAL n. ${s.numero}? Le registrazioni tornano da contabilizzare.`, { ok: 'Annulla bozza', pericolo: true })) {
      await esegui([{ comando: 'annulla_bozza_sal', numero: s.numero }]);
    }
  };

  const elenco_ = k.sal.length
    ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Lavori a tutto il'), h('th', null, 'Stato'), h('th', null, 'Emesso il'), h('th', { class: 'num' }, 'Registrazioni'), h('th', { class: 'num' }, 'Importo SAL'))),
      h('tbody', null, k.sal.map((x) => h('tr', { class: ['selezionabile', x.numero === scelto && 'scelta'], onclick: () => { scelto = x.numero; mostra(el, {}); } },
        h('td', null, String(x.numero)), h('td', null, data(x.lavori_a_tutto_il)),
        h('td', null, h('span', { class: ['etichetta', x.stato === 'emesso' ? 'ok' : 'avviso'] }, x.stato === 'emesso' ? 'emesso' : 'bozza')),
        h('td', null, data(x.data_emissione)), h('td', { class: 'num' }, String(x.registrazioni.length)),
        h('td', { class: 'num' }, x.riepilogo ? euro(x.riepilogo.importo_sal) : '—')))))
    : h('p', { class: 'vuoto' }, 'Nessun SAL.');

  const prospetto = s && h('section', { class: 'scheda' },
    h('h2', null, `SAL n. ${s.numero} — lavori a tutto il ${data(s.lavori_a_tutto_il)}`,
      h('span', { class: ['etichetta', s.stato === 'emesso' ? 'ok' : 'avviso'] }, s.stato === 'emesso' ? `emesso il ${data(s.data_emissione)}` : 'bozza: valori ricalcolati')),
    s.errore ? h('div', { class: 'banner errore' }, s.errore) : h('table', { class: 'semplice prospetto' },
      h('tbody', null, RIGHE_PROSPETTO.filter(([chiave]) => chiave !== 'lavori_esclusi_dal_ribasso' || Number(s.riepilogo[chiave] ?? 0) !== 0).map(([chiave, etichetta, classe]) => h('tr', { class: classe },
        h('td', null, etichetta),
        h('td', { class: 'num' }, euro(s.riepilogo[chiave])))))),
    h('p', { class: 'vuoto' }, `Registrazioni comprese: ${s.registrazioni.length ? s.registrazioni.map((n) => `n. ${n}`).join(', ') : 'nessuna'}.`),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '10px' } },
      s.stato === 'bozza' && h('button', { class: 'pulsante primario', onclick: emetti }, 'Emetti SAL…'),
      s.stato === 'bozza' && h('button', { class: 'pulsante pericolo', onclick: annullaBozza }, 'Annulla bozza'),
      h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'sal', numero: s.numero }) }, 'Anteprima e stampa')));

  const certificato = s && schedaCertificato(s, k);
  const penali = k.parametri && schedaPenali(k);

  sostituisci(el,
    h('div', { class: 'vista-testata' },
      h('h2', null, titolo),
      h('button', {
        class: 'pulsante primario', onclick: nuovoSal, disabled: !k.parametri || !!bozza || !!k.conto_finale,
        title: bozza ? 'Emettere o annullare prima la bozza in corso' : k.conto_finale ? 'Il conto finale è redatto' : '',
      }, '+ Nuovo SAL'),
      h('span', { class: 'spinge' }),
      k.parametri && h('span', { class: 'vuoto' }, `Ribasso ${percento(k.parametri.ribasso)} · ritenuta ${percento(k.parametri.ritenuta_garanzia)}`)),
    h('div', { class: 'vista-corpo' },
      !k.parametri && h('div', { class: 'banner avviso' }, 'Per i SAL servono i dati del contratto (ribasso, ritenuta).', h('button', { class: 'pulsante', onclick: () => vai('generale') }, 'Imposta il contratto')),
      k.conto_finale && h('div', { class: 'banner info' }, k.chiusa ? 'Conto finale emesso: la contabilità è chiusa.' : 'Il conto finale è in bozza: per redigere altri SAL va annullato.', h('button', { class: 'pulsante', onclick: () => vai('conto') }, 'Conto finale')),
      k.parametri && !k.conto_finale && daContabilizzare.length > 0 && !bozza && h('div', { class: 'banner info' }, `${daContabilizzare.length} registrazioni del libretto non ancora contabilizzate.`),
      h('section', { class: 'scheda' }, h('h2', null, 'Elenco'), elenco_),
      prospetto,
      certificato,
      penali));
}

/** Certificato di pagamento del SAL scelto, con le opzioni di recupero per le bozze. */
function schedaCertificato(s, k) {
  const c = s.certificato;
  const bozza = s.stato === 'bozza';
  const anticipazione = !!k.parametri?.anticipazione;
  const opzione = (campo, etichetta, valore) => {
    const box = h('input', { type: 'checkbox', name: campo, checked: valore });
    box.addEventListener('change', () => esegui([{ comando: 'imposta_sal', numero: s.numero, [campo]: box.checked }]));
    return h('label', { class: 'campo spunta' }, h('span', null, etichetta), box);
  };
  const riga = (testo, valore, classe) => h('tr', { class: classe }, h('td', null, testo), h('td', { class: 'num' }, euro(valore)));
  return h('section', { class: 'scheda' },
    h('h2', null, `Certificato di pagamento n. ${s.numero}`,
      s.certificato_congelato ? h('span', { class: 'etichetta ok' }, 'congelato all\'emissione')
        : bozza ? h('span', { class: 'etichetta avviso' }, 'bozza: ricalcolato')
          : h('span', { class: 'etichetta neutra' }, 'ricalcolato (SAL di una versione precedente)')),
    s.errore_certificato ? h('div', { class: 'banner errore' }, s.errore_certificato) : h('table', { class: 'semplice prospetto' }, h('tbody', null,
      riga('Importo del presente SAL (al netto della ritenuta)', c.importo_sal),
      anticipazione && riga(c.recupero_sospeso ? 'A dedurre: recupero dell\'anticipazione (sospeso)' : 'A dedurre: recupero dell\'anticipazione', c.recupero_anticipazione),
      Number(c.penali) !== 0 && riga('A dedurre: penali', c.penali),
      riga('Imponibile', c.imponibile, 'parziale'),
      c.aliquota_iva && riga(`IVA ${numero(c.aliquota_iva, 0)} %`, c.iva),
      riga('Totale del certificato', c.totale, 'totale'))),
    c && anticipazione && h('p', { class: 'vuoto' },
      `Anticipazione ${euro(c.anticipazione)}: recuperati prima ${euro(c.anticipazione_recuperata_prima)}, residuo dopo questo certificato ${euro(c.anticipazione_residua)}.`),
    c && !c.aliquota_iva && h('p', { class: 'vuoto' }, 'Aliquota IVA non indicata nel contratto (Dati generali): importi senza IVA.'),
    bozza && anticipazione && h('div', { class: 'modulo', style: { marginTop: '8px' } },
      opzione('recupero_anticipazione_sospeso', 'Sospendi il recupero dell\'anticipazione in questo certificato', s.recupero_anticipazione_sospeso),
      opzione('recupera_residuo_anticipazione', 'Recupera tutta l\'anticipazione residua (ultimo SAL)', s.recupera_residuo_anticipazione)),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '10px' } },
      h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'certificato', numero: s.numero }) }, 'Anteprima e stampa del certificato')));
}

/** Penali: elenco, nuova penale, rimozione di quelle non ancora applicate. */
function schedaPenali(k) {
  const nuova = async () => {
    const v = await chiediCampi('Nuova penale', [
      { nome: 'data', etichetta: 'Data', tipo: 'date', valore: oggi(), obbligatorio: true, aiuto: 'Va nel primo certificato il cui SAL arriva a questa data.' },
      { nome: 'descrizione', etichetta: 'Motivo', obbligatorio: true, valore: 'Ritardo' },
      { nome: 'giorni', etichetta: 'Giorni di ritardo', numerico: true },
      { nome: 'per_mille', etichetta: 'Per mille giornaliero', numerico: true, valore: '1', aiuto: 'Art. 126: dallo 0,5 all\'1,5 per mille dell\'ammontare netto contrattuale' },
      { nome: 'importo', etichetta: 'oppure importo (€)', numerico: true, aiuto: 'Se indicato, sostituisce il calcolo da giorni e per mille' },
    ], {
      ok: 'Registra',
      convalida: (x) => {
        try {
          const imp = decimale(x.importo);
          if (imp === null && (!/^\d+$/.test(x.giorni.trim()) || decimale(x.per_mille) === null)) return 'Indicare giorni e per mille, oppure l\'importo.';
          if (imp === null && !k.importo_contrattuale) return 'Per calcolare la penale serve l\'importo contrattuale (Dati generali): in alternativa indicare l\'importo.';
          return null;
        } catch (e) { return e.message; }
      },
    });
    if (!v) return;
    const c = { comando: 'registra_penale', data: v.data, descrizione: v.descrizione.trim() };
    const imp = decimale(v.importo);
    if (imp !== null) c.importo = imp;
    if (v.giorni.trim()) c.giorni = Number(v.giorni.trim());
    if (decimale(v.per_mille) !== null) c.per_mille = decimale(v.per_mille);
    esegui([c]);
  };
  const totale = sommaImporti(k.penali.map((p) => p.importo));
  return h('section', { class: 'scheda' },
    h('h2', null, 'Penali', h('button', { class: 'pulsante', onclick: nuova, disabled: k.chiusa }, '+ Penale')),
    k.penali.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Data'), h('th', null, 'Motivo'), h('th', null, 'Calcolo'), h('th', { class: 'num' }, 'Importo'), h('th', null, 'Certificato'), h('th'))),
      h('tbody', null, k.penali.map((p) => h('tr', null,
        h('td', null, String(p.numero)), h('td', null, data(p.data)), h('td', null, p.descrizione),
        h('td', null, p.giorni && p.per_mille ? `${p.giorni} gg × ${numero(p.per_mille, 1)} ‰` : '—'),
        h('td', { class: 'num' }, euro(p.importo)),
        h('td', null, p.sal ? `SAL n. ${p.sal}` : p.conto_finale ? 'rata di saldo' : h('span', { class: 'vuoto' }, 'da applicare')),
        h('td', { class: 'azioni-riga' }, !p.sal && !k.chiusa && h('button', {
          class: 'pulsante icona pericolo', 'aria-label': `Rimuovi la penale ${p.numero}`, title: 'Rimuovi',
          onclick: async () => { if (await conferma('Rimuovi penale', `Rimuovere la penale n. ${p.numero}?`, { ok: 'Rimuovi', pericolo: true })) esegui([{ comando: 'rimuovi_penale', numero: p.numero }]); },
        }, '×')))))) : h('p', { class: 'vuoto' }, 'Nessuna penale.'),
    k.importo_contrattuale && k.penali.length ? h('p', { class: 'vuoto' },
      `Totale ${euro(totale)} su un limite di ${euro(decimo(k.importo_contrattuale))} (10 % dell'importo contrattuale, art. 126).`) : null);
}

/** Un decimo di un importo, troncato al centesimo (solo per l'indicazione del limite). */
function decimo(importo) {
  const centesimi = BigInt(sommaImporti([importo]).replace('.', '')) / 10n;
  const neg = centesimi < 0n;
  const a = neg ? -centesimi : centesimi;
  return `${neg ? '-' : ''}${a / 100n}.${String(a % 100n).padStart(2, '0')}`;
}
