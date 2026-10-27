// Computo metrico estimativo: voci e righi di misura modificabili, totali, dettaglio
// della voce (descrizione estesa, categorie, WBS, note). Con «Affianca» l'elenco prezzi o
// un prezzario si apre a sinistra e le voci passano al computo con «>» (ADR 0018).

import { h, sostituisci, euro, numero, natura, conferma, controllo, notifica, preferenza, salvaPreferenza, sommaImporti } from '../dom.js';
import { chiedi, esegui, agisci, mostraErrore, computoCorrente, stato } from '../stato.js';
import { foglioMisure, rigoIngresso } from '../misure.js';
import { scegliVoce, comandiPerVoce } from '../selettore.js';
import * as trascina from '../trascina.js';
import { pannelloVoci, divisore, LARGHEZZA_MINIMA } from '../affianca.js';
import { scegliVociDaCitare, comandiVediVoce, vociCheCitano, quanteVoci } from '../vedivoce.js';
import { pannelloFiltri } from '../filtri.js';

export const titolo = 'Computo';

const locale = { computo: null, voce: null, scelte: [], fuoco: null, fuocoPannello: false, scorrimento: 0, avviso: null };

let elVista = null;
let foglio = null;
let dati = null;
let progetto = null;
let dettaglio = null;
let attivo = null; // {blocco, rigo}
let pannello = null; // voci affiancate (creato alla prima apertura)
let filtri = null; // pannello dei filtri (ADR 0020)
let visibili = []; // voci che passano i filtri

export async function mostra(el, parametri, { nuova = false } = {}) {
  const nome = computoCorrente();
  if (!nome) {
    sostituisci(el, h('div', { class: 'vista-corpo' }, h('p', { class: 'vuoto' }, 'Il progetto non ha computi.')));
    return;
  }
  if (locale.computo !== nome) { locale.computo = nome; locale.voce = null; locale.scelte = []; locale.scorrimento = 0; locale.fuoco = null; }
  elVista = el;
  const scorrevole = el.querySelector('.foglio');
  if (scorrevole) locale.scorrimento = scorrevole.scrollTop;
  try {
    [dati, progetto] = await Promise.all([
      chiedi({ richiesta: 'computo', computo: nome }),
      chiedi({ richiesta: 'progetto' }),
    ]);
  } catch (e) { mostraErrore(e); return; }
  if (parametri.voce && dati.voci.some((v) => v.numero === Number(parametri.voce))) {
    locale.voce = Number(parametri.voce);
    locale.fuoco = { blocco: locale.voce, rigo: 1, c: 0, oppureNuovo: true };
    delete parametri.voce;
  }
  if (parametri.ultima && dati.voci.length) {
    locale.voce = dati.voci.length;
    locale.fuoco = { blocco: dati.voci.length, rigo: 1, c: 2 };
    delete parametri.ultima;
  }
  disegna(nuova);
}

function disegna(nuova) {
  const decimali = progetto.regole.decimali_quantita;
  filtri ??= pannelloFiltri({ chiave: 'computo', importo: (v) => v.importo, suCambio: ridisegnaFiltrato });
  filtri.imposta(dati.voci, sezioniFiltro());
  visibili = filtri.filtra(dati.voci);
  const filtrato = visibili.length !== dati.voci.length || filtri.attivi() > 0;
  const blocchi = visibili.map((v) => ({
    id: v.numero, uid: v.id, numero: v.numero, codice: v.codice, titolo: v.descrizione_breve, um: v.unita_misura,
    quantita: v.quantita, prezzo: v.prezzo, importo: v.importo, righi: v.righi, errore: v.errore,
    selezionato: v.numero === locale.voce,
    etichetta: v.stato_prezzo === 'non_allineato' ? { tipo: 'avviso', testo: 'prezzo cambiato in elenco' }
      : v.stato_prezzo === 'assente_in_elenco' ? { tipo: 'neutra', testo: 'non in elenco' }
        : v.natura === 'oneri_sicurezza' ? { tipo: 'neutra', testo: 'sicurezza' } : null,
  }));
  // Solo le voci visibili restano selezionate: «Elimina voce» non tocca quelle nascoste.
  const valide = new Set(visibili.map((v) => v.numero));
  locale.scelte = locale.scelte.filter((n) => valide.has(n));
  if (!locale.scelte.length && locale.voce) locale.scelte = [locale.voce];
  foglio = foglioMisure({
    blocchi, decimali, etichetta: `Misure del computo ${dati.nome}`,
    scelti: locale.scelte,
    suSelezioneVoci: (bs) => { locale.scelte = bs.map((b) => b.numero); },
    trascinaVoci: (bs) => (bs.length ? { tipo: 'computo', computo: dati.nome, voci: bs.map((b) => b.uid), numeri: bs.map((b) => b.numero), codici: bs.map((b) => b.codice) } : null),
    rilascio: rilascia,
    vuoto: filtrato ? 'Nessuna voce corrisponde ai filtri.' : 'Computo vuoto: «+ Voce» aggiunge una voce dall\'elenco prezzi o da un prezzario caricato.',
    suSeleziona: (b, rigo) => {
      attivo = { blocco: b, rigo };
      if (locale.voce !== b.numero) { locale.voce = b.numero; mostraDettaglio(); }
    },
    suModifica: (b, n, rigo, prossima) => comandi([{ comando: 'modifica_rigo', computo: dati.nome, voce: b.uid, numero: n, rigo }], prossima, { blocco: b.id, rigo: n, c: prossima.c }),
    suAggiungi: (b, posizione, rigo, prossima) => comandi([{ comando: 'aggiungi_rigo', computo: dati.nome, voce: b.uid, rigo, ...(posizione ? { posizione } : {}) }], prossima, { blocco: b.id, rigo: 'nuovo', c: prossima.c }),
    suInserisci: inserisciRigo,
    suElimina: eliminaRigo,
    suIncolla: (b, posizione, righi, prossima) => comandi(
      righi.map((rigo, i) => ({ comando: 'aggiungi_rigo', computo: dati.nome, voce: b.uid, rigo, posizione: posizione + i })), prossima),
    suNotifica: (t) => notifica(t, 'info', null, 1500),
    suVediVoce: (vv) => vaiAllaVoce(vv.numero),
  });
  dettaglio = h('aside', { class: 'dettaglio', 'aria-label': 'Dettaglio voce' });
  const totali = h('div', { class: 'totali' },
    dati.errore ? h('span', { class: 'etichetta errore' }, `Totali non calcolabili: ${dati.errore}`) : [
      h('span', null, 'Lavori', h('strong', null, euro(dati.totale_lavori))),
      h('span', null, 'Oneri della sicurezza', h('strong', null, euro(dati.totale_oneri_sicurezza))),
      filtrato ? h('span', { class: 'filtrato' }, `Filtrate (${visibili.length})`, h('strong', null, euro(sommaImporti(visibili.map((v) => v.importo))))) : null,
      h('span', { class: 'complessivo' }, 'Totale', h('strong', null, euro(dati.totale))),
    ]);
  const barraFiltro = filtrato && h('div', { class: 'barra-filtro', role: 'status' },
    h('span', null, `Filtro attivo: ${visibili.length} di ${dati.voci.length} voci`),
    h('button', { class: 'link', onclick: () => filtri.azzera() }, 'Azzera'),
    filtriAperti() ? null : h('button', { class: 'link', onclick: () => alternaFiltri() }, 'Mostra i filtri'));
  const b = (testo, fn, titoloP, classe = '') => h('button', { class: `pulsante ${classe}`, onclick: fn, title: titoloP }, testo);
  sostituisci(elVista,
    h('div', { class: 'vista-testata' },
      h('h2', null, dati.nome),
      b('+ Voce', aggiungiVoce, 'Aggiungi una voce dopo quella selezionata', 'primario'),
      b('+ Rigo', inserisciRigo, 'Inserisci un rigo sotto quello attivo (Ins)'),
      b('Deduzione', () => alterna('deduzione'), 'Rigo in deduzione (sì/no)'),
      b('Commento', () => alterna('commento'), 'Rigo di solo commento (sì/no)'),
      b('Vedi voce…', vediVoce, 'Cita una o più voci del computo, una per rigo: la quantità del rigo segue la voce citata'),
      b('Elimina rigo', eliminaRigo, 'Elimina il rigo attivo (Ctrl+Canc)'),
      h('span', { class: 'separatore' }),
      b('↑', () => sposta(-1), 'Sposta la voce in alto', 'icona'),
      b('↓', () => sposta(1), 'Sposta la voce in basso', 'icona'),
      b('Elimina voce', eliminaVoce, 'Elimina la voce selezionata (o le voci selezionate con Ctrl/Maiusc+clic sulla testata)', 'pericolo'),
      h('span', { class: 'spinge' }),
      pulsanteFiltri(),
      pulsanteAffianca(),
      pulsanteDettaglio()),
    h('div', { class: 'vista-corpo pieno' }, corpoVista(conFiltri(
      h('div', { class: ['dividi', !preferenza('dettaglio', window.innerWidth >= 1500) && 'senza-dettaglio'] },
        h('div', { class: 'zona' }, barraFiltro, foglio.el, totali), dettaglio)))));
  foglio.el.scrollTop = locale.scorrimento;
  if (locale.fuoco) {
    const p = locale.fuoco;
    locale.fuoco = null;
    if (!foglio.focalizza(p) && p.rigo !== 'nuovo') foglio.focalizza({ ...p, rigo: 'nuovo' });
  } else if (nuova) {
    foglio.el.focus({ preventScroll: true });
  }
  if (affiancato()) {
    const fuoco = locale.fuocoPannello;
    locale.fuocoPannello = false;
    if (fuoco) pannello.fuoco();
    pannello.aggiorna();
  }
  mostraDettaglio();
  avvisaDopoAggiunta();
}

function affiancato() {
  return preferenza('affianca', false);
}

function filtriAperti() {
  return preferenza('filtri_computo', false);
}

/** Il computo con il pannello dei filtri a sinistra, se aperto. */
function conFiltri(computo) {
  return filtriAperti() ? h('div', { class: 'con-filtri' }, filtri.el, computo) : computo;
}

function alternaFiltri() {
  salvaPreferenza('filtri_computo', !filtriAperti());
  locale.scorrimento = elVista.querySelector('.foglio')?.scrollTop ?? 0;
  disegna(false);
}

/** Apre o chiude il pannello dei filtri (preferenza conservata). */
function pulsanteFiltri() {
  const acceso = filtriAperti();
  const n = filtri?.attivi() ?? 0;
  return h('button', {
    class: ['pulsante', acceso && 'premuto', n && 'con-filtro'], 'aria-pressed': acceso ? 'true' : 'false',
    title: 'Filtra le voci per categorie, WBS, capitolo dell\'elenco e colonne',
    onclick: alternaFiltri,
  }, n ? `Filtri (${n})` : 'Filtri');
}

/** Ridisegno dopo un cambio dei filtri, conservando il fuoco nel pannello. */
function ridisegnaFiltrato() {
  const f = filtri.fuoco();
  locale.scorrimento = 0;
  disegna(false);
  filtri.ripristinaFuoco(f);
}

/** Sezioni del pannello dei filtri del computo. */
function sezioniFiltro() {
  const etichetta = (elenco) => {
    const m = new Map(elenco.map((c) => [c.codice, `${c.codice} — ${c.descrizione}`]));
    return (k) => m.get(k) ?? k;
  };
  const categoria = (id, titolo, elenco) => elenco.length && {
    id, titolo, tipo: 'scelte', valori: (v) => v.categorie?.[id] ?? '', etichetta: etichetta(elenco),
    ordine: elenco.map((c) => c.codice), vuoto: 'Non assegnata',
  };
  const padreWbs = (n) => (n.codice_completo.length > n.codice.length ? n.codice_completo.slice(0, -n.codice.length - 1) : null);
  const stati = { allineato: 'Allineato all\'elenco', non_allineato: 'Prezzo cambiato in elenco', assente_in_elenco: 'Non più in elenco' };
  return [
    categoria('super_categoria', 'Super categorie', progetto.super_categorie),
    categoria('categoria', 'Categorie', progetto.categorie),
    categoria('sotto_categoria', 'Sotto categorie', progetto.sotto_categorie),
    progetto.wbs.length && {
      id: 'wbs', titolo: 'WBS', tipo: 'albero', valore: (v) => v.wbs ?? '', vuoto: 'Non assegnata',
      nodi: progetto.wbs.map((n) => ({ codice: n.codice_completo, padre: padreWbs(n), descrizione: n.descrizione })),
    },
    progetto.capitoli.length && {
      id: 'capitolo', titolo: 'Capitolo dell\'elenco', tipo: 'albero', valore: (v) => v.capitolo ?? '', vuoto: 'Senza capitolo',
      nodi: progetto.capitoli.map((c) => ({ codice: c.codice, padre: c.padre, descrizione: c.descrizione })),
    },
    { id: 'tariffa', titolo: 'Tariffa', tipo: 'testo', valore: (v) => v.codice, segnaposto: 'Contiene… (es. A.01)' },
    { id: 'descrizione', titolo: 'Descrizione', tipo: 'testo', valore: (v) => `${v.descrizione_breve} ${v.descrizione_estesa}`, segnaposto: 'Tutte le parole…' },
    { id: 'natura', titolo: 'Natura', tipo: 'scelte', valori: (v) => v.natura, etichetta: natura, ordine: ['lavoro', 'oneri_sicurezza'] },
    { id: 'unita', titolo: 'Unità di misura', tipo: 'scelte', valori: (v) => v.unita_misura || '', vuoto: 'Senza unità' },
    { id: 'prezzo', titolo: 'Prezzo unitario', tipo: 'intervallo', valore: (v) => v.prezzo, unita: '€' },
    { id: 'quantita', titolo: 'Quantità', tipo: 'intervallo', valore: (v) => v.quantita },
    { id: 'importo', titolo: 'Importo', tipo: 'intervallo', valore: (v) => v.importo, unita: '€' },
    { id: 'stato', titolo: 'Prezzo rispetto all\'elenco', tipo: 'scelte', valori: (v) => v.stato_prezzo, etichetta: (k) => stati[k] ?? k, ordine: Object.keys(stati) },
  ].filter(Boolean);
}

/**
 * Categorie e WBS da dare a una voce nuova con un filtro attivo: i livelli con un solo
 * valore scelto (come fa il programma di riferimento), così la voce resta visibile.
 */
function assegnazioneFiltro() {
  const u = filtri?.scelteUniche() ?? {};
  const out = {};
  const categorie = {};
  for (const k of ['super_categoria', 'categoria', 'sotto_categoria']) if (u[k]) categorie[k] = u[k];
  if (Object.keys(categorie).length) out.categorie = categorie;
  if (u.wbs) out.wbs = u.wbs;
  return out;
}

/** Testo della notifica per una voce assegnata dal filtro. */
function testoAssegnazione(a) {
  const parti = [...Object.values(a.categorie ?? {}), a.wbs && `WBS ${a.wbs}`].filter(Boolean);
  return parti.length ? ` (assegnata a ${parti.join(', ')} dal filtro)` : '';
}

/** Corpo della vista: il computo, con il pannello delle voci a sinistra se affiancato. */
function corpoVista(computo) {
  if (!affiancato()) return computo;
  pannello ??= pannelloVoci({
    suCopia: (voci) => aggiungiDalPannello(voci),
    nelComputo: () => new Set(dati?.voci.map((v) => v.codice) ?? []),
  });
  const frecce = h('div', { class: 'frecce', role: 'toolbar', 'aria-orientation': 'vertical', 'aria-label': 'Copia di voci', title: 'Trascinare per allargare o restringere il pannello' },
    h('button', { class: 'pulsante primario', 'aria-label': 'Aggiungi al computo', title: 'Aggiungi al computo le voci selezionate a sinistra, dopo la voce selezionata (Invio o doppio clic nella lista)', onclick: () => aggiungiDalPannello(pannello.selezionate()) }, freccia('>')),
    h('button', { class: 'pulsante', 'aria-label': 'Togli dal computo', title: 'Togli dal computo le voci selezionate (Ctrl/Maiusc+clic sulle testate per più voci; si può annullare)', onclick: togliVoci }, freccia('<')));
  const larghezza = preferenza('affianca_larghezza', null);
  const contenitore = h('div', { class: 'affianca' }, pannello.el, frecce, computo);
  if (larghezza) contenitore.style.setProperty('--larghezza-voci', `${Math.max(LARGHEZZA_MINIMA, larghezza)}px`);
  divisore(frecce, contenitore);
  return contenitore;
}

/**
 * «>» o «<» disegnati in vettoriale: un carattere di testo si centra solo in orizzontale
 * (la sua altezza nella riga dipende dal carattere del sistema), il disegno anche in
 * verticale.
 */
function freccia(verso) {
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('class', 'freccia');
  svg.setAttribute('aria-hidden', 'true');
  const linea = document.createElementNS(ns, 'polyline');
  linea.setAttribute('points', verso === '>' ? '4.5,4 11.5,8 4.5,12' : '11.5,4 4.5,8 11.5,12');
  svg.appendChild(linea);
  return svg;
}

/** Apre o chiude il pannello delle voci affiancato (preferenza conservata). */
function pulsanteAffianca() {
  const acceso = affiancato();
  return h('button', {
    class: ['pulsante', acceso && 'premuto'], 'aria-pressed': acceso ? 'true' : 'false',
    title: 'Affianca a sinistra l\'elenco prezzi o un prezzario, per aggiungere voci al computo con «>»',
    onclick: () => {
      salvaPreferenza('affianca', !affiancato());
      locale.scorrimento = elVista.querySelector('.foglio')?.scrollTop ?? 0;
      disegna(false);
    },
  }, 'Affianca voci');
}

/** Posizione (da 1) dopo l'ultima voce selezionata del computo, o in fondo. */
function posizioneDopo() {
  const scelte = locale.scelte.length ? locale.scelte : locale.voce ? [locale.voce] : [];
  return scelte.length ? Math.max(...scelte) + 1 : dati.voci.length + 1;
}

/**
 * Comandi che aggiungono al computo le tariffe indicate dalla posizione data; da un
 * prezzario, prima copia nell'elenco del progetto le voci che mancano.
 */
async function comandiAggiunta(prezzario, codici, posizione) {
  const c = [];
  if (prezzario) {
    const elenco = await chiedi({ richiesta: 'elenco_prezzi' }).catch(() => ({ voci: [] }));
    const presenti = new Set(elenco.voci.map((v) => v.codice));
    const mancanti = [...new Set(codici.filter((k) => !presenti.has(k)))];
    if (mancanti.length) c.push({ comando: 'copia_da_prezzario', prezzario, codici: mancanti });
  }
  const a = assegnazioneFiltro();
  codici.forEach((codice, i) => c.push({ comando: 'aggiungi_voce_computo', computo: dati.nome, codice, posizione: posizione + i, righi: [{}], ...a }));
  return c;
}

/** «>»: le voci scelte nel pannello entrano nel computo dopo la voce selezionata. */
async function aggiungiDalPannello(voci) {
  if (!voci.length) { notifica('Selezionare a sinistra le voci da aggiungere', 'info', null, 1800); return; }
  const prezzario = pannello.fonte();
  const codici = voci.map((v) => v.codice);
  const posizione = posizioneDopo();
  const c = await comandiAggiunta(prezzario, codici, posizione);
  const ultima = posizione + codici.length - 1;
  // Le voci aggiunte restano selezionate: il prossimo «>» aggiunge dopo di loro.
  locale.voce = ultima;
  locale.scelte = codici.map((_, i) => posizione + i);
  locale.fuocoPannello = true;
  let messaggio = codici.length === 1 ? `Voce «${codici[0]}» aggiunta` : `${codici.length} voci aggiunte`;
  if (prezzario) messaggio += ` da «${prezzario}»`;
  locale.avviso = { numeri: [...locale.scelte], testo: messaggio + testoAssegnazione(assegnazioneFiltro()) };
  const esito = await comandi(c, { blocco: ultima, rigo: 1, c: 0, oppureNuovo: true, mantieniScelte: true });
  if (!esito) { locale.scelte = []; locale.fuocoPannello = false; locale.avviso = null; }
}

/** «<»: toglie dal computo le voci selezionate (un solo passo da annullare). */
async function togliVoci() {
  const numeri = [...(locale.scelte.length ? locale.scelte : locale.voce ? [locale.voce] : [])].sort((a, b) => b - a);
  if (!numeri.length) { notifica('Selezionare nel computo le voci da togliere', 'info', null, 1800); return; }
  const primo = numeri.at(-1);
  const restano = dati.voci.length - numeri.length;
  locale.voce = restano ? Math.min(primo, restano) : null;
  locale.scelte = [];
  // Dal fondo, così i numeri delle voci restanti non cambiano.
  const esito = await comandi(numeri.map((n) => ({ comando: 'rimuovi_voce_computo', computo: dati.nome, voce: idDi(n) })), null);
  if (!esito) return;
  const n = numeri.length;
  notifica(n === 1 ? 'Voce tolta dal computo' : `${n} voci tolte dal computo`, 'ok', { testo: 'Annulla', fn: () => agisci({ richiesta: 'annulla' }) }, 4000);
}

/** Porta alla voce indicata (numero d'ordine), dal riferimento di un rigo «vedi voce». */
function vaiAllaVoce(numero) {
  if (!dati.voci.some((v) => v.numero === numero)) return;
  locale.voce = numero;
  locale.scelte = [numero];
  foglio.focalizza({ blocco: numero, rigo: 1, c: 0, oppureNuovo: true });
  mostraDettaglio();
}

/**
 * «Vedi voce…»: le voci scelte entrano come righi della voce attiva, sotto il rigo
 * attivo (o in fondo), collegati alle voci citate.
 */
async function vediVoce() {
  const a = foglio.attivo();
  const numeroVoce = a?.blocco?.numero ?? locale.voce;
  const voce = dati.voci.find((v) => v.numero === numeroVoce);
  if (!voce) { notifica('Selezionare la voce in cui inserire i righi «vedi voce»', 'info', null, 2000); return; }
  const posizione = a?.rigo && a.blocco.numero === voce.numero ? a.rigo.numero + 1 : voce.righi.length + 1;
  const escluse = vociCheCitano(dati.voci, voce.numero);
  const scelta = await scegliVociDaCitare({
    titolo: `Vedi voce — righi della voce n. ${voce.numero} «${voce.codice}»`,
    decimali: progetto.regole.decimali_quantita,
    computo: dati.nome,
    voci: async () => dati.voci,
    escluse: () => new Set([voce.numero, ...escluse]),
    nota: escluse.size ? `Non sono proposte le voci che citano già questa voce (${[...escluse].sort((x, y) => x - y).map((n) => `n. ${n}`).join(', ')}): sarebbe un riferimento circolare.` : null,
  });
  if (!scelta?.voci.length) return;
  const n = scelta.voci.length;
  const esito = await comandi(comandiVediVoce({ computo: dati.nome, voce: voce.id, posizione, voci: scelta.voci, deduzione: scelta.deduzione }),
    { blocco: voce.numero, rigo: posizione + n - 1, c: 0 });
  if (esito) notifica(`${quanteVoci(n)} nella voce n. ${voce.numero}`, 'ok', null, 2500);
}

/** Esegue comandi e prepara la cella da riattivare (`seRifiutato` se la richiesta fallisce). */
async function comandi(c, fuoco, seRifiutato = null) {
  const esito = await esegui(c);
  if (!esito && seRifiutato) {
    locale.fuoco = seRifiutato;
    // La vista non è stata aggiornata: si ridisegna per ripristinare i valori.
    await mostra(elVista, stato.vista.parametri);
    return esito;
  }
  if (!esito) return esito;
  locale.fuoco = fuoco;
  return esito;
}

/** Identificativo della voce con il numero indicato (nella vista attuale). */
function idDi(numero) {
  return dati.voci.find((v) => v.numero === numero)?.id ?? numero;
}

function voceScelta() {
  return dati?.voci.find((v) => v.numero === locale.voce) ?? null;
}

async function aggiungiVoce() {
  const v = await scegliVoce('Aggiungi una voce al computo');
  if (!v) return;
  const posizione = locale.voce ? locale.voce + 1 : dati.voci.length + 1;
  locale.voce = posizione;
  const a = assegnazioneFiltro();
  locale.avviso = { numeri: [posizione], testo: `Voce «${v.codice}» aggiunta${testoAssegnazione(a)}` };
  const esito = await comandi([...(await comandiPerVoce(v)), { comando: 'aggiungi_voce_computo', computo: dati.nome, codice: v.codice, posizione, righi: [{}], ...a }],
    { blocco: posizione, rigo: 1, c: 0 });
  if (!esito) locale.avviso = null;
}

/**
 * Dopo il ridisegno che segue un'aggiunta: la notifica, o l'avviso che le voci aggiunte
 * non rientrano nei filtri.
 */
function avvisaDopoAggiunta() {
  const a = locale.avviso;
  locale.avviso = null;
  if (!a) return;
  const nascoste = a.numeri.filter((n) => !visibili.some((v) => v.numero === n)).length;
  if (nascoste) notifica(`${nascoste === 1 ? 'La voce aggiunta non rientra' : `${nascoste} voci aggiunte non rientrano`} nei filtri attivi`, 'info', { testo: 'Azzera i filtri', fn: () => filtri.azzera() }, 5000);
  else if (a.testo) notifica(a.testo, 'ok', null, 3000);
}

async function inserisciRigo() {
  const a = foglio.attivo() ?? (attivo && { ...attivo, c: 0 });
  const b = a?.blocco ?? (voceScelta() && { numero: locale.voce, id: locale.voce, righi: voceScelta().righi });
  if (!b) { notifica('Selezionare una voce', 'info', null, 1500); return; }
  const posizione = a?.rigo ? a.rigo.numero + 1 : b.righi.length + 1;
  await comandi([{ comando: 'aggiungi_rigo', computo: dati.nome, voce: b.uid ?? voceScelta()?.id, rigo: {}, posizione }], { blocco: b.id, rigo: posizione, c: 0 });
}

async function eliminaRigo() {
  const a = foglio.attivo();
  const sel = foglio.selezionati();
  if (!a?.rigo || !sel?.righi.length) { notifica('Attivare una cella del rigo da eliminare', 'info', null, 1800); return; }
  // Dal basso verso l'alto, così i numeri dei righi restanti non cambiano.
  const numeri = sel.righi.map((r) => r.numero).sort((x, y) => y - x);
  const primo = numeri.at(-1);
  await comandi(numeri.map((numero) => ({ comando: 'rimuovi_rigo', computo: dati.nome, voce: a.blocco.uid, numero })),
    { blocco: a.blocco.id, rigo: Math.max(1, Math.min(primo, a.blocco.righi.length - numeri.length)), c: a.c, oppureNuovo: true });
}

async function alterna(campo) {
  const a = foglio.attivo();
  if (!a?.rigo) { notifica('Attivare una cella del rigo', 'info', null, 1800); return; }
  await comandi([{ comando: 'modifica_rigo', computo: dati.nome, voce: a.blocco.uid, numero: a.rigo.numero, rigo: rigoIngresso(a.rigo, { [campo]: !a.rigo[campo] }) }],
    { blocco: a.blocco.id, rigo: a.rigo.numero, c: campo === 'commento' ? 0 : a.c });
}

async function sposta(delta) {
  const v = voceScelta();
  if (!v) return;
  // Con un filtro si scavalca la voce visibile precedente o successiva.
  const i = visibili.findIndex((x) => x.numero === v.numero);
  const vicina = visibili[i + delta];
  if (i < 0 || !vicina) return;
  const posizione = vicina.numero;
  locale.voce = posizione;
  await comandi([{ comando: 'sposta_voce_computo', computo: dati.nome, voce: v.id, posizione }], null);
}

async function eliminaVoce() {
  const numeri = locale.scelte.length > 1 ? [...locale.scelte].sort((a, b) => b - a) : null;
  if (numeri) {
    if (!(await conferma('Elimina voci', `Eliminare le ${numeri.length} voci selezionate con i loro righi?`, { ok: 'Elimina', pericolo: true }))) return;
    const primo = numeri.at(-1);
    locale.voce = primo > 1 ? primo - 1 : null;
    locale.scelte = [];
    // Dal fondo, così i numeri delle voci restanti non cambiano.
    await comandi(numeri.map((n) => ({ comando: 'rimuovi_voce_computo', computo: dati.nome, voce: idDi(n) })), null);
    return;
  }
  const v = voceScelta();
  if (!v) { notifica('Selezionare una voce', 'info', null, 1500); return; }
  if (!(await conferma('Elimina voce', `Eliminare la voce n. ${v.numero} «${v.codice}» con i suoi ${v.righi.length} righi?`, { ok: 'Elimina', pericolo: true }))) return;
  locale.voce = v.numero > 1 ? v.numero - 1 : null;
  locale.scelte = [];
  await comandi([{ comando: 'rimuovi_voce_computo', computo: dati.nome, voce: v.id }], null);
}

/**
 * Voci rilasciate nel foglio alla posizione indicata (da 1): dall'elenco prezzi o da un
 * prezzario si aggiungono, da un computo si copiano o si spostano.
 */
async function rilascia(e, posizione) {
  const d = await trascina.leggi(e);
  if (!d) { notifica('Si possono rilasciare qui voci dell\'elenco prezzi, di un prezzario o di un computo', 'info', null, 2500); return; }
  const nome = dati.nome;
  let c = [];
  let messaggio;
  let prima = posizione;
  if (d.tipo === 'computo') {
    const stesso = d.computo === nome;
    const sposta = trascina.sposta(e, stesso);
    if (stesso && sposta) prima = posizione - (d.numeri ?? d.voci).filter((n) => n < posizione).length;
    c = [{ comando: 'copia_voci_computo', da: d.computo, voci: d.voci, a: nome, posizione, sposta }];
    const n = d.voci.length;
    const quante = n === 1 ? 'Voce' : `${n} voci`;
    messaggio = stesso
      ? (sposta ? `${quante} spostat${n === 1 ? 'a' : 'e'} (Alt o Ctrl per copiare)` : `${quante} copiat${n === 1 ? 'a' : 'e'}`)
      : `${quante} ${sposta ? 'spostat' : 'copiat'}${n === 1 ? 'a' : 'e'} da «${d.computo}»${sposta ? '' : ' (Maiusc per spostare)'}`;
  } else {
    c = await comandiAggiunta(d.tipo === 'prezzario' ? d.prezzario : null, d.codici, posizione);
    messaggio = d.codici.length === 1 ? `Voce «${d.codici[0]}» aggiunta` : `${d.codici.length} voci aggiunte`;
    if (d.tipo === 'prezzario') messaggio += ` da «${d.prezzario}»`;
  }
  const n = d.tipo === 'computo' ? d.voci.length : d.codici.length;
  locale.voce = prima;
  locale.scelte = Array.from({ length: n }, (_, i) => prima + i);
  if (d.tipo !== 'computo') messaggio += testoAssegnazione(assegnazioneFiltro());
  locale.avviso = { numeri: [...locale.scelte], testo: messaggio };
  const esito = await comandi(c, { blocco: prima, rigo: 1, c: 0, oppureNuovo: true });
  if (!esito) { locale.scelte = []; locale.avviso = null; }
}

function mostraDettaglio() {
  const v = voceScelta();
  if (!v) {
    sostituisci(dettaglio, h('p', { class: 'vuoto' }, 'Selezionare una voce per vederne la descrizione e assegnare categorie e WBS.'),
      h('h3', null, 'Tastiera'),
      h('p', { class: 'vuoto' }, 'Frecce per spostarsi, Invio o F2 per modificare, Tab per passare alla cella successiva, Esc per annullare, Ins per un nuovo rigo. Nei fattori si possono scrivere espressioni come 2*(3,50+0,30).'));
    return;
  }
  const modifica = (campi) => esegui([{ comando: 'modifica_voce_computo', computo: dati.nome, voce: v.id, ...campi }]);
  const sel = (livello, elenco_) => {
    const s = controllo({ tipo: 'select', nome: `cat-${livello}`, valore: v.categorie[livello] ?? '', opzioni: [['', '—'], ...elenco_.map((c) => [c.codice, `${c.codice} — ${c.descrizione}`])] });
    s.addEventListener('change', () => modifica({ categorie: { [livello]: s.value } }));
    return s;
  };
  const wbs = controllo({ tipo: 'select', nome: 'wbs', valore: v.wbs ?? '', opzioni: [['', '—'], ...progetto.wbs.map((n) => [n.codice_completo, `${'  '.repeat(n.livello)}${n.codice_completo} — ${n.descrizione}`])] });
  wbs.addEventListener('change', () => modifica({ wbs: wbs.value }));
  const note = controllo({ tipo: 'textarea', nome: 'note', valore: v.note ?? '', righe: 3 });
  note.addEventListener('change', () => modifica({ note: note.value }));
  const campo = (etichetta, el) => h('label', { class: 'campo' }, h('span', null, etichetta), el);
  sostituisci(dettaglio,
    h('h3', null, `Voce n. ${v.numero}`),
    h('p', null, h('strong', { class: 'mono' }, v.codice), ' ', h('span', { class: 'etichetta neutra' }, natura(v.natura))),
    h('p', null, h('strong', null, v.descrizione_breve)),
    v.descrizione_estesa && v.descrizione_estesa !== v.descrizione_breve ? h('div', { class: 'descrizione' }, v.descrizione_estesa) : null,
    h('h3', null, 'Prezzo'),
    h('p', null, `${numero(v.prezzo, 2)} € / ${v.unita_misura}`),
    v.quantita !== null && h('p', null, `Quantità ${numero(v.quantita, progetto.regole.decimali_quantita)} — importo ${euro(v.importo)}`),
    v.errore && h('p', { class: 'banner errore' }, v.errore),
    v.stato_prezzo === 'non_allineato' && h('div', { class: 'banner avviso' },
      h('span', null, 'Il prezzo in elenco è cambiato. La voce conserva quello applicato all\'inserimento.'),
      h('button', { class: 'pulsante', onclick: () => esegui([{ comando: 'riallinea_prezzo', computo: dati.nome, voce: v.id }]) }, 'Riallinea')),
    v.stato_prezzo === 'assente_in_elenco' && h('p', { class: 'banner info' }, 'La tariffa non è più nell\'elenco prezzi del progetto.'),
    citataDa(v.numero),
    h('h3', null, 'Classificazione'),
    h('div', { class: 'modulo' },
      progetto.super_categorie.length ? campo('Super categoria', sel('super_categoria', progetto.super_categorie)) : null,
      progetto.categorie.length ? campo('Categoria', sel('categoria', progetto.categorie)) : null,
      progetto.sotto_categorie.length ? campo('Sotto categoria', sel('sotto_categoria', progetto.sotto_categorie)) : null,
      progetto.wbs.length ? campo('WBS', wbs) : null,
      !progetto.super_categorie.length && !progetto.categorie.length && !progetto.sotto_categorie.length && !progetto.wbs.length
        ? h('p', { class: 'vuoto' }, 'Nessuna categoria o WBS definita: si aggiungono in «Categorie, WBS, computi».') : null,
      campo('Note', note)));
}

/** Voci i cui righi «vedi voce» citano la voce indicata, con un clic per andarci. */
function citataDa(numero) {
  const da = dati.voci.filter((x) => x.righi.some((r) => r.vedi_voce && !r.vedi_voce.statica && r.vedi_voce.numero === numero));
  if (!da.length) return null;
  return h('p', { class: 'citata-da' }, 'Citata da «vedi voce» nelle voci ',
    da.map((x, i) => [i ? ', ' : '', h('button', { class: 'link', onclick: () => vaiAllaVoce(x.numero) }, `n. ${x.numero}`)]), '.');
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
