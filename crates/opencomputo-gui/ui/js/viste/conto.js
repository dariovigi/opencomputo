// Conto finale: redazione ed emissione, stato finale, rata di saldo, firma e collaudo.

import { h, sostituisci, data, oggi, euro, numero, chiediCampi, conferma, controllo, ErroreUtente } from '../dom.js';
import { chiedi, esegui, mostraErrore, vai } from '../stato.js';

export const titolo = 'Conto finale';

const RIGHE_STATO_FINALE = [
  ['lavori_lordi', 'Lavori a tutto lo stato finale (lordo)'],
  ['lordo_stato_finale', '— di cui lavori non compresi nei SAL'],
  ['oneri_sicurezza', 'Oneri della sicurezza (non soggetti a ribasso)'],
  ['manodopera_esclusa', 'Manodopera esclusa dal ribasso'],
  ['lavori_esclusi_dal_ribasso', 'Nuovi prezzi esclusi dal ribasso'],
  ['soggetto_a_ribasso', 'Importo soggetto a ribasso'],
  ['ribasso', 'Ribasso d\'asta'],
  ['importo_netto', 'Importo netto finale dei lavori', 'parziale'],
  ['acconti', 'A dedurre: importi dei SAL emessi (acconti)'],
  ['credito_residuo', 'Credito residuo dell\'esecutore', 'parziale'],
  ['ritenute_garanzia', '— di cui ritenute di garanzia da svincolare'],
];

const zero = (v) => !v || Number(v) === 0;

export async function mostra(el) {
  let k;
  try { k = await chiedi({ richiesta: 'contabilita' }); } catch (e) { mostraErrore(e); return; }
  const c = k.conto_finale;
  const bozzaSal = k.sal.find((s) => s.stato === 'bozza');
  const emesso = c?.stato === 'emesso';

  const redigi = async () => {
    const ultimo = k.sal.at(-1)?.lavori_a_tutto_il;
    const v = await chiediCampi('Conto finale', [
      { nome: 'data_ultimazione', etichetta: 'Ultimazione dei lavori', tipo: 'date', valore: ultimo && ultimo > oggi() ? ultimo : oggi(), obbligatorio: true, aiuto: 'Data del certificato di ultimazione dei lavori.' },
      { nome: 'data', etichetta: 'Data del conto finale', tipo: 'date', valore: oggi(), obbligatorio: true },
    ], { ok: 'Redigi in bozza' });
    if (v) await esegui([{ comando: 'redigi_conto_finale', data_ultimazione: v.data_ultimazione, data: v.data }]);
  };
  const emetti = async () => {
    const v = await chiediCampi('Emissione del conto finale', [
      { nome: 'data', etichetta: 'Data di emissione', tipo: 'date', valore: c.data, obbligatorio: true, aiuto: 'Con l\'emissione la contabilità si chiude: niente più registrazioni, SAL o penali. Si può tornare indietro solo con «Annulla» subito dopo.' },
    ], { ok: 'Emetti' });
    if (v) await esegui([{ comando: 'emetti_conto_finale', data: v.data }]);
  };
  const annulla = async () => {
    if (await conferma('Annulla bozza', 'Annullare la bozza del conto finale?', { ok: 'Annulla bozza', pericolo: true })) {
      await esegui([{ comando: 'annulla_conto_finale' }]);
    }
  };

  const testata = h('div', { class: 'vista-testata' },
    h('h2', null, titolo),
    !c && h('button', {
      class: 'pulsante primario', onclick: redigi,
      disabled: !k.parametri || !!bozzaSal || !k.registrazioni.length,
      title: bozzaSal ? 'Emettere o annullare prima il SAL in bozza' : '',
    }, 'Redigi il conto finale…'),
    h('span', { class: 'spinge' }),
    c && h('button', { class: 'pulsante', onclick: () => vai('report', { tipo: 'conto_finale' }) }, 'Anteprima e stampa'));

  if (!c) {
    sostituisci(el, testata, h('div', { class: 'vista-corpo' },
      !k.parametri && h('div', { class: 'banner avviso' }, 'Per il conto finale servono i dati del contratto.', h('button', { class: 'pulsante', onclick: () => vai('generale') }, 'Imposta il contratto')),
      bozzaSal && h('div', { class: 'banner info' }, `Il SAL n. ${bozzaSal.numero} è in bozza: emetterlo o annullarlo prima di redigere il conto finale.`),
      h('section', { class: 'scheda' },
        h('h2', null, 'Chiusura della contabilità'),
        h('p', null, 'Dopo l\'ultimazione dei lavori il direttore dei lavori redige il conto finale. Le registrazioni del libretto non comprese in un SAL formano lo stato finale; il credito residuo, con le ritenute di garanzia, diventa la rata di saldo, al netto dell\'anticipazione non recuperata e delle penali non ancora applicate.'),
        h('p', { class: 'vuoto' }, 'Il conto si redige in bozza (i valori si ricalcolano), poi si emette: la contabilità si chiude. Seguono la firma dell\'esecutore e il collaudo o il certificato di regolare esecuzione.'))));
    return;
  }

  // Dati del conto (modificabili in bozza).
  const campo = (etichetta, el_, aiuto) => h('label', { class: 'campo' }, h('span', null, etichetta), el_, aiuto && h('small', null, aiuto));
  const ultimazione = h('input', { type: 'date', name: 'data_ultimazione', value: c.data_ultimazione, disabled: emesso });
  const dataConto = h('input', { type: 'date', name: 'data_conto', value: c.data, disabled: emesso });
  const note = controllo({ tipo: 'textarea', nome: 'note', valore: c.note ?? '', righe: 3 });
  note.disabled = emesso;
  const aggiorna = (comando) => esegui([{ comando: 'redigi_conto_finale', ...comando }]);
  ultimazione.addEventListener('change', () => ultimazione.value && aggiorna({ data_ultimazione: ultimazione.value }));
  dataConto.addEventListener('change', () => dataConto.value && aggiorna({ data: dataConto.value }));
  note.addEventListener('change', () => aggiorna({ note: note.value.trim() || null }));

  const schedaConto = h('section', { class: 'scheda' },
    h('h2', null, 'Conto finale dei lavori',
      emesso ? h('span', { class: 'etichetta ok' }, `emesso il ${data(c.data)}`) : h('span', { class: 'etichetta avviso' }, 'bozza: valori ricalcolati')),
    h('div', { class: 'modulo colonne' },
      campo('Ultimazione dei lavori', ultimazione),
      campo('Data del conto', dataConto),
      h('label', { class: 'campo largo' }, h('span', null, 'Note e relazione'), note)),
    !emesso && h('div', { style: { display: 'flex', gap: '8px', marginTop: '12px' } },
      h('button', { class: 'pulsante primario', onclick: emetti }, 'Emetti conto finale…'),
      h('button', { class: 'pulsante pericolo', onclick: annulla }, 'Annulla bozza')));

  const r = c.riepilogo;
  const riga = (testo, valore, classe) => h('tr', { class: classe }, h('td', null, testo), h('td', { class: 'num' }, euro(valore)));
  const schedaImporti = h('section', { class: 'scheda' },
    h('h2', null, 'Stato finale e conto'),
    c.errore ? h('div', { class: 'banner errore' }, c.errore) : h('table', { class: 'semplice prospetto' }, h('tbody', null,
      RIGHE_STATO_FINALE.filter(([chiave]) => !(['lordo_stato_finale', 'manodopera_esclusa', 'lavori_esclusi_dal_ribasso'].includes(chiave) && zero(r[chiave])))
        .map(([chiave, etichetta, classe]) => riga(etichetta, r[chiave], classe)))));

  const schedaSaldo = r && h('section', { class: 'scheda' },
    h('h2', null, 'Certificato di pagamento della rata di saldo',
      c.collaudo?.approvato_il ? h('span', { class: 'etichetta ok' }, 'pagabile') : h('span', { class: 'etichetta neutra' }, 'dopo il collaudo')),
    h('table', { class: 'semplice prospetto' }, h('tbody', null,
      riga('Credito residuo dell\'esecutore', r.credito_residuo),
      !zero(r.anticipazione) && riga('A dedurre: anticipazione non ancora recuperata', r.anticipazione_residua),
      !zero(r.penali) && riga('A dedurre: penali non applicate nei certificati', r.penali),
      riga('Imponibile', r.imponibile, 'parziale'),
      r.aliquota_iva && riga(`IVA ${numero(r.aliquota_iva, 0)} %`, r.iva),
      riga('Totale della rata di saldo', r.totale, 'totale'))),
    h('p', { class: 'vuoto' }, 'Si paga dopo il collaudo o il certificato di regolare esecuzione; prima dell\'accettazione definitiva dell\'opera serve la garanzia dell\'art. 117, comma 9. Le ritenute si svincolano previo DURC regolare.'));

  const finali = k.registrazioni.filter((x) => c.registrazioni.includes(x.numero));
  const schedaLavori = h('section', { class: 'scheda' },
    h('h2', null, 'Lavori dello stato finale non compresi nei SAL'),
    finali.length ? h('table', { class: 'semplice' },
      h('thead', null, h('tr', null, h('th', null, 'N.'), h('th', null, 'Data'), h('th', null, 'Tariffa'), h('th', null, 'Designazione'), h('th', { class: 'num' }, 'Importo lordo'))),
      h('tbody', null, finali.map((x) => h('tr', { class: 'selezionabile', onclick: () => vai('libretto') },
        h('td', null, String(x.numero)), h('td', null, data(x.data)), h('td', { class: 'mono' }, x.codice), h('td', null, x.descrizione_breve),
        h('td', { class: 'num' }, x.importo ? euro(x.importo) : '—')))))
      : h('p', { class: 'vuoto' }, 'Nessuna: tutte le registrazioni sono comprese nei SAL.'),
    c.penali.length ? h('p', { class: 'vuoto' }, `Penali della rata di saldo: ${c.penali.map((n) => `n. ${n}`).join(', ')}.`) : null);

  sostituisci(el, testata, h('div', { class: 'vista-corpo' },
    emesso && h('div', { class: 'banner info' }, 'Conto finale emesso: la contabilità è chiusa.'),
    schedaConto, schedaImporti, schedaSaldo, schedaLavori,
    emesso && schedaFirmaCollaudo(c)));
}

/** Firma dell'esecutore e collaudo, dopo l'emissione. */
function schedaFirmaCollaudo(c) {
  const firma = h('input', { type: 'date', name: 'firma', value: c.firma_esecutore ?? '' });
  const riserve = controllo({ tipo: 'textarea', nome: 'riserve', valore: c.riserve ?? '', righe: 2 });
  const tipo = controllo({ tipo: 'select', nome: 'tipo_collaudo', valore: c.collaudo?.tipo ?? 'regolare_esecuzione', opzioni: [['regolare_esecuzione', 'Certificato di regolare esecuzione'], ['collaudo', 'Certificato di collaudo']] });
  const dataCollaudo = h('input', { type: 'date', name: 'data_collaudo', value: c.collaudo?.data ?? '' });
  const approvato = h('input', { type: 'date', name: 'approvato_il', value: c.collaudo?.approvato_il ?? '' });
  const campo = (etichetta, el, aiuto, largo) => h('label', { class: ['campo', largo && 'largo'] }, h('span', null, etichetta), el, aiuto && h('small', null, aiuto));

  const registraFirma = () => {
    if (!firma.value) { mostraErrore(new ErroreUtente('Indicare la data della firma.')); return; }
    esegui([{ comando: 'firma_conto_finale', data: firma.value, riserve: riserve.value.trim() || null }]);
  };
  const registraCollaudo = () => {
    if (!dataCollaudo.value) { mostraErrore(new ErroreUtente('Indicare la data del certificato.')); return; }
    const comando = { comando: 'registra_collaudo', tipo: tipo.value, data: dataCollaudo.value };
    if (approvato.value) comando.approvato_il = approvato.value;
    esegui([comando]);
  };
  return h('section', { class: 'scheda' },
    h('h2', null, 'Firma e collaudo'),
    h('h3', null, 'Firma dell\'esecutore',
      c.firma_esecutore ? h('span', { class: 'etichetta ok' }, `firmato il ${data(c.firma_esecutore)}`) : h('span', { class: 'etichetta avviso' }, 'da firmare')),
    h('p', { class: 'nota' }, 'Entro trenta giorni dall\'invito del RUP. L\'esecutore conferma le riserve già iscritte nel registro (non può aggiungerne); se non firma, il conto si intende accettato.'),
    h('div', { class: 'modulo colonne' },
      campo('Data della firma', firma),
      campo('Riserve confermate', riserve, 'Vuoto: firmato senza riserve', true)),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '8px' } },
      h('button', { class: 'pulsante primario', onclick: registraFirma }, c.firma_esecutore ? 'Aggiorna firma' : 'Registra firma'),
      c.firma_esecutore && h('button', { class: 'pulsante', onclick: () => esegui([{ comando: 'firma_conto_finale' }]) }, 'Rimuovi firma')),
    h('h3', { style: { marginTop: '18px' } }, 'Collaudo o regolare esecuzione',
      c.collaudo ? h('span', { class: 'etichetta ok' }, `${c.collaudo.tipo === 'collaudo' ? 'collaudo' : 'CRE'} del ${data(c.collaudo.data)}`) : h('span', { class: 'etichetta neutra' }, 'non eseguito')),
    h('p', { class: 'nota' }, 'Certificato di regolare esecuzione entro tre mesi dall\'ultimazione; collaudo entro sei mesi (fino a un anno nei casi complessi). La rata di saldo e lo svincolo delle ritenute seguono l\'approvazione.'),
    h('div', { class: 'modulo colonne' },
      campo('Tipo', tipo),
      campo('Data del certificato', dataCollaudo),
      campo('Approvato il', approvato, 'Dalla stazione appaltante')),
    h('div', { style: { display: 'flex', gap: '8px', marginTop: '8px' } },
      h('button', { class: 'pulsante primario', onclick: registraCollaudo }, c.collaudo ? 'Aggiorna' : 'Registra'),
      c.collaudo && h('button', { class: 'pulsante', onclick: () => esegui([{ comando: 'registra_collaudo', rimuovi: true }]) }, 'Rimuovi')));
}
