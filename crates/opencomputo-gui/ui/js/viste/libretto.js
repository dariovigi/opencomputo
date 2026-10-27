// Libretto delle misure: registrazioni con righi modificabili. Le registrazioni di un
// SAL emesso sono bloccate dal motore e mostrate in sola lettura.

import { h, sostituisci, data, oggi, euro, numero, chiediCampi, conferma, notifica, sommaImporti , preferenza, salvaPreferenza } from '../dom.js';
import { chiedi, esegui, mostraErrore, stato, vai } from '../stato.js';
import { foglioMisure, rigoIngresso } from '../misure.js';
import { scegliVoce, comandiPerVoce } from '../selettore.js';
import { scegliVociDaCitare, quanteVoci } from '../vedivoce.js';

export const titolo = 'Libretto delle misure';

const locale = { numero: null, fuoco: null, scorrimento: 0 };
let elVista = null;
let foglio = null;
let dati = null;
let regole = null;
let dettaglio = null;

export async function mostra(el, _parametri, { nuova = false } = {}) {
  elVista = el;
  const s = el.querySelector('.foglio');
  if (s) locale.scorrimento = s.scrollTop;
  try {
    [dati, { regole }] = await Promise.all([chiedi({ richiesta: 'contabilita' }), chiedi({ richiesta: 'progetto' })]);
  } catch (e) { mostraErrore(e); return; }
  disegna(nuova);
}

const righiIngresso = (reg) => reg.righi.map((r) => rigoIngresso(r));

function disegna(nuova) {
  const blocchi = dati.registrazioni.map((r) => ({
    id: r.numero, numero: r.numero, codice: r.codice, titolo: `${data(r.data)} — ${r.descrizione_breve}`, um: r.unita_misura,
    quantita: r.quantita, prezzo: r.prezzo, importo: r.importo, righi: r.righi, errore: r.errore, bloccato: r.bloccata,
    selezionato: r.numero === locale.numero,
    etichetta: r.sal ? { tipo: r.bloccata ? 'ok' : 'neutra', testo: `SAL ${r.sal}${r.bloccata ? ' emesso' : ' in bozza'}` }
      : r.stato_finale ? { tipo: r.bloccata ? 'ok' : 'neutra', testo: r.bloccata ? 'conto finale emesso' : 'stato finale' } : null,
  }));
  const modificaRighi = (b, righi, fuoco, seRifiutato) =>
    comandi([{ comando: 'modifica_registrazione', numero: b.numero, righi }], fuoco, seRifiutato);
  foglio = foglioMisure({
    blocchi, decimali: regole.decimali_quantita, etichetta: 'Libretto delle misure',
    vuoto: 'Nessuna registrazione: «+ Registrazione» annota una misurazione eseguita in cantiere.',
    suSeleziona: (b) => { if (locale.numero !== b.numero) { locale.numero = b.numero; mostraDettaglio(); } },
    suModifica: (b, n, rigo, prossima) => {
      const reg = dati.registrazioni.find((r) => r.numero === b.numero);
      const righi = righiIngresso(reg);
      righi[n - 1] = rigo;
      return modificaRighi(b, righi, prossima, { blocco: b.id, rigo: n, c: prossima.c });
    },
    suAggiungi: (b, _pos, rigo, prossima) => {
      const reg = dati.registrazioni.find((r) => r.numero === b.numero);
      return modificaRighi(b, [...righiIngresso(reg), rigo], prossima, { blocco: b.id, rigo: 'nuovo', c: prossima.c });
    },
    suInserisci: () => alteraRighi((righi, i) => { righi.splice(i + 1, 0, {}); return i + 2; }),
    suElimina: () => alteraRighi((righi, i) => { righi.splice(i, 1); return Math.max(1, i); }),
    suIncolla: (b, posizione, nuovi, prossima) => {
      const reg = dati.registrazioni.find((r) => r.numero === b.numero);
      const righi = righiIngresso(reg);
      righi.splice(posizione - 1, 0, ...nuovi);
      return comandi([{ comando: 'modifica_registrazione', numero: b.numero, righi }], prossima);
    },
    suNotifica: (t) => notifica(t, 'info', null, 1500),
  });
  dettaglio = h('aside', { class: 'dettaglio', 'aria-label': 'Dettaglio registrazione' });
  const totaleLordo = sommaImporti(dati.registrazioni.map((r) => r.importo));
  const daContabilizzare = dati.registrazioni.filter((r) => !r.sal).length;
  const b = (testo, fn, t, classe = '') => h('button', { class: `pulsante ${classe}`, onclick: fn, title: t }, testo);
  sostituisci(elVista,
    h('div', { class: 'vista-testata' },
      h('h2', null, titolo),
      h('button', { class: 'pulsante primario', onclick: nuovaRegistrazione, disabled: dati.chiusa, title: dati.chiusa ? 'Contabilità chiusa dal conto finale' : 'Registra una misurazione' }, '+ Registrazione'),
      b('+ Rigo', () => alteraRighi((righi, i) => { righi.splice(i + 1, 0, {}); return i + 2; }), 'Inserisci un rigo (Ins)'),
      b('Deduzione', () => alterna('deduzione'), 'Rigo in deduzione (sì/no)'),
      b('Vedi voce…', vediVoce, 'Copia nei righi la quantità di una o più voci di computo (nel libretto il «vedi voce» è statico)'),
      b('Elimina rigo', () => alteraRighi((righi, i) => { righi.splice(i, 1); return Math.max(1, i); }), 'Elimina il rigo attivo'),
      h('span', { class: 'separatore' }),
      b('Cambia data…', cambiaData, 'Data della registrazione selezionata'),
      b('Elimina registrazione', elimina, 'Solo se non è in un SAL', 'pericolo'),
      h('span', { class: 'spinge' }),
      pulsanteDettaglio()),
    h('div', { class: 'vista-corpo pieno' }, h('div', { class: ['dividi', !preferenza('dettaglio', window.innerWidth >= 1500) && 'senza-dettaglio'] },
      h('div', { class: 'zona' }, foglio.el, h('div', { class: 'totali' },
        h('span', null, 'Registrazioni', h('strong', null, String(dati.registrazioni.length))),
        h('span', null, dati.conto_finale ? 'Nello stato finale' : 'Da contabilizzare', h('strong', null, String(daContabilizzare))),
        // Somma dei lordi delle registrazioni, solo indicativa: gli importi contabili sono nei SAL.
        h('span', { class: 'complessivo' }, 'Lordo registrato', h('strong', null, euro(totaleLordo))))),
      dettaglio)));
  foglio.el.scrollTop = locale.scorrimento;
  if (locale.fuoco) {
    const p = locale.fuoco;
    locale.fuoco = null;
    if (!foglio.focalizza(p) && p.rigo !== 'nuovo') foglio.focalizza({ ...p, rigo: 'nuovo' });
  } else if (nuova) foglio.el.focus({ preventScroll: true });
  mostraDettaglio();
}

async function comandi(c, fuoco, seRifiutato = null) {
  const esito = await esegui(c);
  if (!esito && seRifiutato) {
    locale.fuoco = seRifiutato;
    await mostra(elVista, stato.vista.parametri);
    return esito;
  }
  if (esito) locale.fuoco = fuoco;
  return esito;
}

/** Modifica i righi della registrazione attiva; `fn(righi, indice)` restituisce il rigo da attivare. */
async function alteraRighi(fn) {
  const a = foglio.attivo();
  if (!a || a.blocco.bloccato) { notifica('Attivare una cella di una registrazione modificabile', 'info', null, 1800); return; }
  const reg = dati.registrazioni.find((r) => r.numero === a.blocco.numero);
  const righi = righiIngresso(reg);
  const i = a.rigo ? a.rigo.numero - 1 : righi.length - 1;
  const prossimo = fn(righi, i);
  await comandi([{ comando: 'modifica_registrazione', numero: reg.numero, righi }], { blocco: reg.numero, rigo: prossimo, c: 0, oppureNuovo: true });
}

/** «Vedi voce…» nel libretto: un rigo per voce scelta, con la quantità copiata. */
async function vediVoce() {
  const a = foglio.attivo();
  if (!a || a.blocco.bloccato) { notifica('Attivare una cella di una registrazione modificabile', 'info', null, 1800); return; }
  const computi = stato.info?.computi?.map((c) => c.nome) ?? [];
  if (!computi.length) { notifica('Il progetto non ha computi', 'info', null, 1800); return; }
  const scelta = await scegliVociDaCitare({
    titolo: `Vedi voce — righi della registrazione n. ${a.blocco.numero}`,
    decimali: regole.decimali_quantita,
    computi,
    computo: computi[0],
    voci: async (nome) => (await chiedi({ richiesta: 'computo', computo: nome })).voci,
    nota: 'Nel libretto la quantità della voce viene copiata adesso e non cambia più.',
  });
  if (!scelta?.voci.length) return;
  const reg = dati.registrazioni.find((r) => r.numero === a.blocco.numero);
  const righi = righiIngresso(reg);
  const i = a.rigo ? a.rigo.numero : righi.length;
  righi.splice(i, 0, ...scelta.voci.map((v) => ({ descrizione: '', deduzione: scelta.deduzione, vedi_voce: { computo: scelta.computo, voce: v.numero } })));
  const n = scelta.voci.length;
  const esito = await comandi([{ comando: 'modifica_registrazione', numero: reg.numero, righi }], { blocco: reg.numero, rigo: i + n, c: 0 });
  if (esito) notifica(`${quanteVoci(n)} nella registrazione n. ${reg.numero}`, 'ok', null, 2500);
}

async function alterna(campo) {
  const a = foglio.attivo();
  if (!a?.rigo || a.blocco.bloccato) return;
  const reg = dati.registrazioni.find((r) => r.numero === a.blocco.numero);
  const righi = righiIngresso(reg);
  righi[a.rigo.numero - 1] = rigoIngresso(a.rigo, { [campo]: !a.rigo[campo] });
  await comandi([{ comando: 'modifica_registrazione', numero: reg.numero, righi }], { blocco: reg.numero, rigo: a.rigo.numero, c: a.c });
}

async function nuovaRegistrazione() {
  const d = await chiediCampi('Nuova registrazione', [{ nome: 'data', etichetta: 'Data della misurazione', tipo: 'date', valore: oggi(), obbligatorio: true }], { ok: 'Scegli la voce…' });
  if (!d) return;
  const v = await scegliVoce('Voce da registrare');
  if (!v) return;
  const prossimo = dati.registrazioni.reduce((m, r) => Math.max(m, r.numero), 0) + 1;
  locale.numero = prossimo;
  await comandi([...(await comandiPerVoce(v)), { comando: 'registra', data: d.data, codice: v.codice, righi: [{}] }], { blocco: prossimo, rigo: 1, c: 0 });
}

function scelta() {
  return dati.registrazioni.find((r) => r.numero === locale.numero) ?? null;
}

async function cambiaData() {
  const r = scelta();
  if (!r) return;
  const d = await chiediCampi(`Registrazione n. ${r.numero}`, [{ nome: 'data', etichetta: 'Data', tipo: 'date', valore: r.data, obbligatorio: true }]);
  if (d && d.data !== r.data) await comandi([{ comando: 'modifica_registrazione', numero: r.numero, data: d.data }], null);
}

async function elimina() {
  const r = scelta();
  if (!r) return;
  if (!(await conferma('Elimina registrazione', `Eliminare la registrazione n. ${r.numero} del ${data(r.data)}?`, { ok: 'Elimina', pericolo: true }))) return;
  locale.numero = null;
  await comandi([{ comando: 'rimuovi_registrazione', numero: r.numero }], null);
}

function mostraDettaglio() {
  const r = scelta();
  if (!r) {
    sostituisci(dettaglio, h('p', { class: 'vuoto' }, 'Selezionare una registrazione.'),
      h('p', { class: 'vuoto' }, 'Le registrazioni passano al registro di contabilità quando si crea un SAL che le comprende; con l\'emissione del SAL diventano immodificabili.'));
    return;
  }
  sostituisci(dettaglio,
    h('h3', null, `Registrazione n. ${r.numero}`),
    h('p', null, `Data: ${data(r.data)}`),
    h('p', null, h('strong', { class: 'mono' }, r.codice), ' ', r.descrizione_breve),
    h('p', null, `${numero(r.quantita, regole.decimali_quantita)} ${r.unita_misura} × ${numero(r.prezzo, 2)} € = ${euro(r.importo)}`),
    r.errore && h('p', { class: 'banner errore' }, r.errore),
    h('h3', null, 'Contabilizzazione'),
    r.sal
      ? h('p', null, `SAL n. ${r.sal} — ${r.bloccata ? 'emesso: la registrazione non è più modificabile' : 'in bozza'}`, ' ', h('button', { class: 'link', onclick: () => vai('sal', { numero: r.sal }) }, 'apri'))
      : h('p', { class: 'vuoto' }, 'Non ancora in un SAL.'),
    h('h3', null, 'Annotazioni'),
    r.bloccata
      ? (r.annotazioni ? h('div', { class: 'descrizione' }, r.annotazioni) : h('p', { class: 'vuoto' }, 'Nessuna.'))
      : annotazioni(r));
}

function annotazioni(r) {
  const t = h('textarea', { name: 'annotazioni', rows: 4, value: r.annotazioni ?? '', placeholder: 'Es. presenti in contraddittorio…' });
  t.addEventListener('change', () => comandi([{ comando: 'modifica_registrazione', numero: r.numero, annotazioni: t.value }], null));
  return t;
}

/** Mostra o nasconde il pannello di dettaglio (preferenza conservata). */
function pulsanteDettaglio() {
  return h('button', {
    class: 'pulsante', title: 'Mostra o nasconde il pannello di dettaglio',
    onclick: () => {
      const d = elVista.querySelector('.dividi');
      const nascosto = d.classList.toggle('senza-dettaglio');
      salvaPreferenza('dettaglio', !nascosto);
    },
  }, 'Dettaglio');
}
