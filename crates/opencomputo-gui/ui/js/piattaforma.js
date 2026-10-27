// Adattatore verso Tauri. È l'unico modulo che usa `window.__TAURI__`: nei test
// end-to-end (e in un'eventuale versione web) basta fornire un oggetto con la stessa forma.

const T = window.__TAURI__;

if (!T) {
  document.body.textContent = 'OpenComputo va avviato come applicazione desktop.';
  throw new Error('API di Tauri non disponibili');
}

/** Etichetta della finestra corrente. */
export const finestra = T.webviewWindow?.getCurrentWebviewWindow?.().label
  ?? window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label
  ?? 'principale';

/** Invia una richiesta al servizio e restituisce la risposta {ok, risultato|errore}. */
export function inviaRichiesta(richiesta) {
  return T.core.invoke('richiesta', { richiesta });
}

/** Sceglie un file da aprire. `filtri`: [{name, extensions}]. Restituisce il percorso o null. */
export async function scegliFile(titolo, filtri, { cartella = false } = {}) {
  const r = await T.dialog.open({ title: titolo, multiple: false, directory: cartella, filters: cartella ? undefined : filtri });
  return Array.isArray(r) ? r[0] ?? null : r ?? null;
}

/** Sceglie dove salvare un file. Restituisce il percorso o null. */
export async function scegliSalvataggio(titolo, filtri, nome) {
  return (await T.dialog.save({ title: titolo, filters: filtri, defaultPath: nome })) ?? null;
}

/** Ascolta un evento dell'applicazione. */
export function ascolta(evento, fn) {
  return T.event.listen(evento, (e) => fn(e.payload));
}

/** Apre una vista in una nuova finestra. */
export function apriFinestra(vista, titolo) {
  return T.core.invoke('apri_finestra', { vista, titolo });
}

/** Dati del trascinamento di voci in corso, tenuti dal motore per le altre finestre. */
export function trascinamento(azione, dati = null) {
  return T.core.invoke('trascinamento', { azione, dati });
}

/** Apre un file con l'applicazione predefinita del sistema. */
export function apriFile(percorso) {
  return T.core.invoke('apri_file', { percorso });
}

/** Esce dall'applicazione (dopo la conferma delle modifiche non salvate). */
export function esci() {
  return T.core.invoke('esci');
}

/** Cambia il titolo della finestra, se consentito. */
export function titoloFinestra(titolo) {
  document.title = titolo;
  try {
    T.webviewWindow?.getCurrentWebviewWindow?.().setTitle?.(titolo)?.catch?.(() => {});
  } catch { /* titolo non modificabile: resta quello del documento */ }
}

/** `true` su macOS (scorciatoie con ⌘). */
export const mac = /Mac/.test(navigator.platform || navigator.userAgent);
