// Stato dell'interfaccia e accesso al servizio.
//
// Ogni modifica passa da `esegui()`, che invia comandi JSON alla sessione (gli stessi
// che usano gli agenti): richieste atomiche, rifiuto dei nuovi errori di validazione,
// annulla/ripristina gestiti dal motore.

import { inviaRichiesta } from './piattaforma.js';
import { h, modale, notifica, ErroreUtente } from './dom.js';

/** Errore restituito dal servizio. */
export class ErroreServizio extends Error {
  constructor(errore) {
    super(errore?.descrizione || 'errore');
    this.errore = errore;
  }
}

export const stato = {
  /** Risultato di `info` (null se nessun progetto aperto). */
  info: null,
  /** Vista corrente: {nome, parametri}. */
  vista: { nome: 'generale', parametri: {} },
  /** Richieste in corso. */
  occupato: 0,
};

const ascoltatori = new Set();

/** Registra una funzione chiamata a ogni cambiamento dello stato. */
export function suCambio(fn) {
  ascoltatori.add(fn);
  return () => ascoltatori.delete(fn);
}

function avvisa(motivo) {
  for (const fn of ascoltatori) fn(motivo);
}

function occupato(delta) {
  stato.occupato += delta;
  document.body.classList.toggle('occupato', stato.occupato > 0);
}

/** Invia una richiesta e restituisce il risultato; lancia ErroreServizio. */
export async function chiedi(richiesta) {
  occupato(1);
  try {
    const r = await inviaRichiesta(richiesta);
    if (!r.ok) throw new ErroreServizio(r.errore);
    return r.risultato;
  } finally {
    occupato(-1);
  }
}

/** Aggiorna `stato.info` (null se nessun progetto è aperto). */
export async function aggiornaInfo() {
  try {
    stato.info = await chiedi({ richiesta: 'info' });
  } catch (e) {
    if (e instanceof ErroreServizio && e.errore.codice === 'richiesta') stato.info = null;
    else throw e;
  }
  return stato.info;
}

/** Aggiorna info e interfaccia (dopo una modifica, anche da un'altra finestra). */
export async function aggiorna(motivo = 'dati') {
  await aggiornaInfo();
  avvisa(motivo);
}

/** Cambia vista. */
export function vai(nome, parametri = {}) {
  stato.vista = { nome, parametri };
  avvisa('vista');
}

function descriviProblemi(problemi) {
  return h('ul', { class: 'elenco-problemi' }, problemi.map((p) =>
    h('li', { class: p.gravita }, h('strong', null, p.messaggio), percorso(p.percorso) && h('small', null, ' — ' + percorso(p.percorso)))));
}

/** Descrizione leggibile del percorso di un problema. */
export function percorso(p = []) {
  return p.map((r) => {
    switch (r.tipo) {
      case 'computo': return null;
      case 'voce_computo': return `voce n. ${r.numero}${r.codice ? ` «${r.codice}»` : ''}`;
      case 'rigo': return `rigo ${r.numero}`;
      case 'voce_prezzario': return `voce «${r.codice ?? ''}»`;
      case 'registrazione': return `registrazione n. ${r.numero}`;
      case 'sal': return `SAL n. ${r.numero}`;
      case 'conto_finale': return 'conto finale';
      case 'prezzario': return null;
      default: return r.codice ?? r.nome ?? null;
    }
  }).filter(Boolean).join(' › ');
}

/** Mostra un errore del servizio o dell'interfaccia. */
export function mostraErrore(e) {
  if (e instanceof ErroreServizio && e.errore.codice === 'nuovi_errori') {
    modale({
      titolo: 'Modifica non applicata',
      contenuto: h('div', null,
        h('p', null, 'La modifica introdurrebbe questi errori nel progetto, quindi non è stata applicata:'),
        descriviProblemi(e.errore.problemi)),
    });
  } else {
    notifica(e.message || String(e), 'errore');
    if (!(e instanceof ErroreServizio) && !(e instanceof ErroreUtente)) console.error(e);
  }
}

/**
 * Esegue comandi sul progetto. Restituisce l'esito o null se la richiesta è stata
 * rifiutata (l'errore è già stato mostrato).
 */
export async function esegui(comandi, { silenzioso = false } = {}) {
  try {
    const esito = await chiedi({ richiesta: 'esegui', comandi });
    // Un rigo appena aggiunto è vuoto per costruzione: il suo avviso non va notificato.
    const avvisi = esito.nuovi_problemi.filter((p) => p.gravita !== 'informazione' && p.codice !== 'misura.rigo_vuoto');
    if (avvisi.length && !silenzioso) {
      notifica(avvisi.length === 1 ? `Avviso: ${avvisi[0].messaggio}` : `${avvisi.length} nuovi avvisi nel progetto`, 'avviso',
        avvisi.length > 1 ? { testo: 'Vedi', fn: () => vai('problemi') } : null);
    }
    await aggiorna('dati');
    return esito;
  } catch (e) {
    mostraErrore(e);
    return null;
  }
}

/** Esegue un'altra richiesta che modifica lo stato (apri, salva, annulla…). */
export async function agisci(richiesta) {
  try {
    const r = await chiedi(richiesta);
    await aggiorna('dati');
    return r;
  } catch (e) {
    mostraErrore(e);
    return null;
  }
}

/** Esegue una funzione asincrona mostrando eventuali errori. */
export async function prova(fn) {
  try {
    return await fn();
  } catch (e) {
    mostraErrore(e);
    return undefined;
  }
}

/** Nome del computo corrente (il primo se non indicato). */
export function computoCorrente() {
  const nomi = stato.info?.computi?.map((c) => c.nome) ?? [];
  const richiesto = stato.vista.parametri.computo;
  return nomi.includes(richiesto) ? richiesto : nomi[0] ?? null;
}
