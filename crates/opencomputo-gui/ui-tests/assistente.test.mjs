// Assistente AI nell'interfaccia: consenso, chiavi, caricamento del prezzario, proposta
// con anteprima, applicazione e annullamento, domande, errori dei provider, indice,
// impostazioni dei modelli. I provider sono un server locale finto (provider-finto.mjs):
// nessuna chiamata reale e nessuna chiave vera. Il motore va compilato con
// `cargo build -p opencomputo-agent-api --features banco-di-prova`.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { apriBanco, RADICE } from './banco.mjs';
import { avviaProviderFinto } from './provider-finto.mjs';

const SCHERMATE = process.env.OPENCOMPUTO_SCHERMATE || null;
const CSV = path.join(RADICE, 'crates/opencomputo-assistente/tests/fixtures/prezzario_prova.csv');

let b;
let finto;

before(async () => {
  finto = await avviaProviderFinto();
  b = await apriBanco({ provider: finto.url });
  await b.motore({ richiesta: 'nuovo', titolo: 'Tinteggiature' });
  await b.pagina.goto(b.indirizzo);
  await attendi();
});
after(async () => { await b?.chiudi(); finto?.chiudi(); });

const p = () => b.pagina;

async function attendi(pagina = p()) {
  await pagina.waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
  await pagina.waitForTimeout(30);
  await pagina.waitForFunction(() => !document.body.classList.contains('occupato') && document.body.dataset.disegno === '0');
}

async function schermata(nome) {
  if (SCHERMATE) await p().screenshot({ path: path.join(SCHERMATE, `${nome}.png`) });
}

const scheda = async (nome) => { await p().click(`button[name="scheda-${nome}"]`); await attendi(); };
const stato = async () => (await b.motore({ richiesta: 'assistente_stato' })).risultato;
const messaggi = () => finto.ricevute.filter((r) => r.percorso === '/v1/messages');
const embedding = () => finto.ricevute.filter((r) => r.percorso === '/v1/embeddings');

async function ultimaNotifica(tipo) {
  const n = p().locator(`.notifica.${tipo}`).last();
  await n.waitFor();
  return (await n.innerText()).trim();
}

async function chiudiNotifiche() {
  await p().evaluate(() => document.querySelectorAll('.notifica').forEach((n) => n.remove()));
}

async function proponi(testo) {
  await p().fill('textarea[name="richiesta-ai"]', testo);
  await p().click('button[name="proponi"]');
  await p().waitForFunction(() => !document.querySelector('.ai-attesa'));
  await attendi();
}

test('consenso prima di qualsiasi chiamata', async () => {
  await p().locator('nav button', { hasText: 'Assistente AI' }).click();
  await attendi();
  // Avviso sempre visibile: i dati escono dal computer.
  assert.match(await p().locator('.ai-fuori').innerText(), /escono dal computer/);
  // Senza consenso la proposta non è disponibile: c'è solo il testo da accettare.
  await p().waitForSelector('.ai-consenso');
  assert.equal(await p().locator('textarea[name="richiesta-ai"]').count(), 0);
  assert.match(await p().locator('.ai-testo-consenso').innerText(), /A OpenAI/);
  assert.ok(await p().locator('button[name="accetta-consenso"]').isDisabled());
  await schermata('ai-01-consenso');
  await p().check('input[name="consenso-letto"]');
  await p().click('button[name="accetta-consenso"]');
  await attendi();
  await p().waitForSelector('textarea[name="richiesta-ai"]');
  assert.equal((await stato()).consenso.valido, true);
  assert.equal(finto.ricevute.length, 0);
});

test('chiavi nel portachiavi, mai mostrate né salvate nei file', async () => {
  // Senza la chiave di Anthropic: avviso e «Proponi» disattivato.
  await p().waitForSelector('.banner.errore', { hasText: 'chiave API di Anthropic' });
  await p().fill('textarea[name="richiesta-ai"]', 'parete 4 x 3 m');
  assert.ok(await p().locator('button[name="proponi"]').isDisabled());
  await p().click('text=Imposta la chiave');
  await attendi();
  assert.match(await p().locator('.ai-chiave[data-provider="anthropic"]').innerText(), /mancante/);
  await p().fill('input[name="chiave-anthropic"]', 'sk-ant-prova');
  await p().click('button[name="salva-chiave-anthropic"]');
  await attendi();
  assert.match(await p().locator('.ai-chiave[data-provider="anthropic"]').innerText(), /impostata/);
  assert.equal(await p().inputValue('input[name="chiave-anthropic"]'), '');
  const st = await stato();
  assert.equal(st.chiavi.anthropic, true);
  assert.equal(st.chiavi.openai, false);
  // La chiave non compare nella pagina, nella memoria del browser né nei file.
  const html = await p().content();
  assert.ok(!html.includes('sk-ant-prova'));
  assert.ok(!(await p().evaluate(() => JSON.stringify(localStorage))).includes('sk-ant-prova'));
  for (const f of await readdir(b.config)) {
    if (f.endsWith('.json')) assert.ok(!(await readFile(path.join(b.config, f), 'utf8')).includes('sk-'));
  }
  await schermata('ai-02-chiavi');
});

/** Celle di un'opzione: [spunta, pertinenza, voce, quantità, U.M., prezzo, importo]. */
const opzione = (l, codice) => p().locator(`tr.ai-opzione[data-lavorazione="${l}"][data-codice="${codice}"]`);
const celle = async (l, codice) => (await opzione(l, codice).locator('td').allInnerTexts()).map((t) => t.trim());
/** Attende che l'anteprima mostri l'importo atteso per l'opzione. */
async function importo(l, codice, atteso) {
  await p().waitForFunction(([l, c, a]) => document.querySelector(`tr.ai-opzione[data-lavorazione="${l}"][data-codice="${c}"] .ai-importo`)?.textContent === a, [l, codice, atteso]);
}
const barra = () => p().locator('.ai-barra-scelte');

test('alternative proposte in ordine di pertinenza, spunta, aggiunta e annullamento', async () => {
  await scheda('proposta');
  // Il selettore dei file ora accetta anche i listini CSV.
  await b.dialogo(CSV);
  await p().click('button[name="carica-prezzario-ai"]');
  await attendi();
  assert.match(await ultimaNotifica('ok'), /26 voci/);
  assert.match(await p().locator('select[name="prezzario-ai"]').innerText(), /prezzario_prova \(26 voci\)/);
  assert.match(await p().locator('.ai-scelte').innerText(), /Ricerca per parole/);

  await proponi('Tinteggiatura di una parete di 4 x 3 m');
  const r = p().locator('.ai-risultato');
  await r.waitFor();
  assert.match(await r.locator('h2').innerText(), /Completa/);
  const l = r.locator('.ai-lavorazione[data-lavorazione="L01"]');
  assert.match(await l.locator('caption').innerText(), /Alternative proposte/i);
  // Tre voci, dalla più pertinente; la prima spuntata; valori di tutte calcolati dal motore.
  const ordine = await l.locator('tr.ai-opzione').evaluateAll((tr) => tr.map((t) => t.dataset.codice));
  assert.deepEqual(ordine, ['T.01', 'T.03', 'T.02']);
  await importo('L01', 'T.03', '115,20');
  assert.deepEqual(await celle('L01', 'T.01'), ['', 'alta', 'T.01 Tinteggiatura con idropittura lavabile, due mani\nidropittura su pareti interne', '12,00', 'm²', '7,20', '86,40']);
  assert.equal((await celle('L01', 'T.02'))[6], '136,80');
  assert.ok(await p().isChecked('input[name="opzione-L01-T.01"]'));
  assert.ok(!(await p().isChecked('input[name="opzione-L01-T.03"]')));
  // Misure modificabili, dalla richiesta.
  assert.equal(await p().inputValue('input[name="misura-L01-0-L"]'), '4');
  assert.match(await p().getAttribute('input[name="misura-L01-0-L"]', 'title'), /Dalla richiesta/);
  assert.match(await barra().innerText(), /1 voce spuntata[\s\S]*0,00 € → 86,40 €/);
  await schermata('ai-03-proposta');

  // Due chiamate ad Anthropic con la chiave salvata; nessun prezzo nei messaggi.
  const m = messaggi();
  assert.equal(m.length, 2);
  assert.ok(m.every((x) => x.chiave === 'sk-ant-prova'));
  assert.ok(m.every((x) => x.corpo.output_config?.format?.type === 'json_schema'));
  assert.ok(!JSON.stringify(m[1].corpo).includes('7,20') && !JSON.stringify(m[1].corpo).includes('T.01'));

  // Si spunta anche lo smalto e si corregge l'altezza: i valori si aggiornano.
  await p().check('input[name="opzione-L01-T.03"]');
  await p().fill('input[name="misura-L01-0-B"]', '2,5');
  await p().press('input[name="misura-L01-0-B"]', 'Tab');
  await importo('L01', 'T.03', '96,00');
  await importo('L01', 'T.01', '72,00');
  assert.match(await barra().innerText(), /2 voci spuntate[\s\S]*0,00 € → 168,00 €/);
  assert.equal(await p().locator('button[name="aggiungi-scelte"]').innerText(), 'Aggiungi al computo (2)');
  // Un valore non valido blocca l'aggiunta.
  await p().fill('input[name="misura-L01-0-L"]', 'quattro');
  await p().press('input[name="misura-L01-0-L"]', 'Tab');
  await p().waitForFunction(() => document.querySelector('.ai-totali-scelte .errore'));
  assert.match(await barra().innerText(), /non è un numero/);
  assert.ok(await p().locator('button[name="aggiungi-scelte"]').isDisabled());
  await p().fill('input[name="misura-L01-0-L"]', '4');
  await p().press('input[name="misura-L01-0-L"]', 'Tab');
  await p().waitForFunction(() => !document.querySelector('.ai-totali-scelte .errore'));

  // Il progetto non cambia finché non si aggiunge.
  let c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci.length, 0);
  await p().click('button[name="aggiungi-scelte"]');
  await attendi();
  assert.match(await ultimaNotifica('ok'), /2 voci aggiunte/);
  c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.deepEqual(c.voci.map((v) => v.codice), ['T.01', 'T.03']);
  assert.equal(c.totale, '168.00');
  assert.match(c.voci[0].note, /Assistente AI, L01/);
  // La proposta resta: le voci aggiunte sono segnate e non più spuntate, si può continuare.
  assert.match(await opzione('L01', 'T.01').innerText(), /aggiunta/);
  assert.ok(!(await p().isChecked('input[name="opzione-L01-T.01"]')));
  assert.match(await barra().innerText(), /Nessuna voce spuntata/);
  await p().check('input[name="opzione-L01-T.02"]');
  await p().waitForFunction(() => /168,00 € → 282,00 €/.test(document.querySelector('.ai-totali-scelte')?.textContent ?? ''));
  await p().click('button[name="aggiungi-scelte"]');
  await attendi();
  c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci.length, 3);
  // Ogni aggiunta è un passo: si annulla dalla barra.
  await p().click('button[aria-label="Annulla"]');
  await attendi();
  await p().click('button[aria-label="Annulla"]');
  await attendi();
  c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.equal(c.voci.length, 0);
  await chiudiNotifiche();
});

test('ipotesi evidenziate e modificabili', async () => {
  await proponi('Tinteggiatura delle pareti di una stanza con perimetro di 16 m');
  const r = p().locator('.ai-risultato');
  await r.waitFor();
  assert.match(await r.locator('h2').innerText(), /Da rivedere/);
  const b2 = p().locator('input[name="misura-L01-0-B"]');
  assert.equal(await b2.inputValue(), '2,70');
  assert.match(await b2.getAttribute('class'), /ipotesi/);
  assert.match(await b2.getAttribute('title'), /Ipotesi I01: altezza dei locali 2,70 m/);
  const elenchi = await r.locator('.ai-elenco').allInnerTexts();
  assert.ok(elenchi.some((t) => /Ipotesi[\s\S]*I01: altezza dei locali 2,70 m[\s\S]*impatto medio/.test(t)), elenchi.join('|'));
  assert.ok(elenchi.some((t) => /Domande per precisare[\s\S]*altezza dei locali\?/.test(t)));
  // 16 × 2,70 = 43,20 m² a 7,20 €: calcolati dal motore.
  await importo('L01', 'T.01', '311,04');
  assert.equal((await celle('L01', 'T.01'))[3], '43,20');
  await schermata('ai-03b-ipotesi');
  // L'altezza vera è 3 m: il valore non è più un'ipotesi.
  await b2.fill('3');
  await b2.press('Tab');
  await importo('L01', 'T.01', '345,60');
  assert.match(await b2.getAttribute('class'), /modificato/);
  await p().click('button[name="scarta-proposta"]');
  await attendi();
  assert.equal(await p().locator('.ai-risultato').count(), 0);
});

test('materiale non indicato: opzioni possibili, nessuna preselezionata, misura da completare', async () => {
  await proponi('rifare il pavimento della camera');
  const l = p().locator('.ai-lavorazione[data-lavorazione="L01"]');
  await l.waitFor();
  assert.match(await l.locator('h3').innerText(), /Pavimento[\s\S]*Da precisare/);
  assert.match(await l.locator('.ai-precisare').innerText(), /Materiale del pavimento non indicato/);
  assert.match(await l.locator('caption').innerText(), /Opzioni possibili/i);
  const ordine = await l.locator('tr.ai-opzione').evaluateAll((tr) => tr.map((t) => t.dataset.codice));
  assert.deepEqual(ordine, ['P.01', 'P.04', 'P.05']);
  assert.equal(await l.locator('input[type=checkbox]:checked').count(), 0);
  assert.match(await barra().innerText(), /Nessuna voce spuntata/);
  assert.ok(await p().locator('button[name="aggiungi-scelte"]').isDisabled());
  // Superficie non indicata: campo da completare, quantità zero finché manca.
  const area = p().locator('input[name="misura-L01-0-A"]');
  assert.equal(await area.inputValue(), '');
  assert.match(await area.getAttribute('class'), /mancante/);
  await importo('L01', 'P.04', '0,00');
  await schermata('ai-03c-opzioni');
  // Si sceglie il parquet e si indica la superficie: 20 m² × 78,00 = 1.560,00 €.
  await p().check('input[name="opzione-L01-P.04"]');
  await area.fill('20');
  await area.press('Tab');
  await importo('L01', 'P.04', '1.560,00');
  await p().waitForFunction(() => /1\.560,00 €/.test(document.querySelector('.ai-totali-scelte')?.textContent ?? ''));
  await p().click('button[name="aggiungi-scelte"]');
  await attendi();
  const c = (await b.motore({ richiesta: 'computo' })).risultato;
  assert.deepEqual(c.voci.map((v) => [v.codice, v.importo]), [['P.04', '1560.00']]);
  await p().click('button[aria-label="Annulla"]');
  await attendi();
  // Senza la superficie la voce entrerebbe con il rigo da completare.
  await p().check('input[name="opzione-L01-P.05"]');
  await p().uncheck('input[name="opzione-L01-P.04"]');
  await area.fill('');
  await area.press('Tab');
  await importo('L01', 'P.05', '0,00');
  await p().click('button[name="aggiungi-scelte"]');
  await attendi();
  assert.match(await ultimaNotifica('ok'), /1 con misure da completare/);
  const v = (await b.motore({ richiesta: 'computo' })).risultato.voci[0];
  assert.equal(v.codice, 'P.05');
  assert.match(v.righi[0].descrizione, /da completare: N = 1, A = \?/);
  await p().click('button[aria-label="Annulla"]');
  await attendi();
  await chiudiNotifiche();
});

test('nessuna lavorazione riconoscibile: domande da aggiungere alla richiesta', async () => {
  await proponi('ciao');
  const r = p().locator('.ai-risultato');
  await r.waitFor();
  assert.match(await r.innerText(), /Servono altre informazioni/);
  assert.match(await r.innerText(), /Quali lavori servono\?/);
  assert.equal(await p().locator('button[name="aggiungi-scelte"]').count(), 0);
  await p().click('button[name="aggiungi-risposte"]');
  await attendi();
  const testo = await p().inputValue('textarea[name="richiesta-ai"]');
  assert.match(testo, /^ciao\n\nRisposte:\n- Quali lavori servono\? $/);
});

test('errore del provider: messaggio chiaro e collegamento alle impostazioni', async () => {
  await chiudiNotifiche();
  await proponi('parete 4 x 3 m con chiave sbagliata');
  const n = await ultimaNotifica('errore');
  assert.match(n, /rifiutato la chiave/);
  assert.ok(!n.includes('sk-ant-prova'));
  await p().locator('.notifica.errore button.link', { hasText: 'Impostazioni' }).click();
  await attendi();
  await p().waitForSelector('.ai-chiave');
  await chiudiNotifiche();
});

test('indice: conferma dell\'invio, avanzamento, ricerca ibrida', async () => {
  await scheda('indice');
  const riga = p().locator('tr[data-prezzario="prezzario_prova"]');
  assert.ok(await riga.locator('button[name="indicizza-prezzario_prova"]').isDisabled(), 'senza chiave di OpenAI');
  await scheda('impostazioni');
  await p().fill('input[name="chiave-openai"]', 'sk-openai-prova');
  await p().keyboard.press('Enter');
  await attendi();
  await scheda('indice');
  await p().evaluate(() => {
    window.__avanzamenti = [];
    window.__oc.ascoltatori['assistente-avanzamento'].push((e) => window.__avanzamenti.push(e.payload));
  });
  await riga.locator('button[name="indicizza-prezzario_prova"]').click();
  // Conferma esplicita prima di inviare le voci.
  const modale = p().locator('.modale');
  assert.match(await modale.innerText(), /26 voci di «prezzario_prova».*OpenAI/s);
  await modale.locator('button', { hasText: 'Annulla' }).click();
  assert.equal(embedding().length, 0);
  await riga.locator('button[name="indicizza-prezzario_prova"]').click();
  await p().locator('.modale button', { hasText: 'Invia e indicizza' }).click();
  await p().waitForFunction(() => !document.querySelector('.ai-progresso'));
  await attendi();
  assert.match(await ultimaNotifica('ok'), /26 voci calcolate/);
  const celle = await riga.locator('td').allInnerTexts();
  assert.equal(celle[2], '26');
  assert.match(celle[3], /Ibrida/);
  assert.equal(await riga.locator('button').innerText(), 'Aggiorna');
  const avanzamenti = await p().evaluate(() => window.__avanzamenti);
  assert.ok(avanzamenti.length >= 1);
  assert.equal(avanzamenti.at(-1).stato.calcolate, 26);
  const e = embedding();
  assert.equal(e.length, 1);
  assert.equal(e[0].chiave, 'sk-openai-prova');
  assert.equal(e[0].corpo.input.length, 26);
  assert.ok(e[0].corpo.input.every((t) => !t.includes('€')));
  // La cache dei vettori è nella cartella di configurazione.
  assert.ok((await readdir(path.join(b.config, 'cache'))).some((f) => f.startsWith('vettori-')));

  await scheda('proposta');
  assert.match(await p().locator('.ai-scelte').innerText(), /Ricerca ibrida/);
  await proponi('Tinteggiatura di una parete di 5 x 3 m');
  assert.match(await p().locator('.ai-risultato h2').innerText(), /ricerca ibrida/);
  await importo('L01', 'T.01', '108,00');
  await schermata('ai-04-ibrida');
  await chiudiNotifiche();
});

test('impostazioni dei modelli: controlli e modalità per modello', async () => {
  await scheda('impostazioni');
  // Lo strumento forzato non è accettato da Sonnet 5.5: rifiutato prima di ogni chiamata.
  await p().selectOption('select[name="modalita_output"]', 'strumento_forzato');
  await p().click('button[name="salva-modelli"]');
  await attendi();
  assert.match(await p().locator('.errore-modulo').innerText(), /claude-sonnet-5-5/);
  await p().fill('input[name="dimensioni_embedding"]', 'tante');
  await p().click('button[name="salva-modelli"]');
  assert.match(await p().locator('.errore-modulo').innerText(), /numero intero/);
  await p().fill('input[name="dimensioni_embedding"]', '');
  // Con un modello che lo accetta, lo strumento forzato va bene e si usa davvero.
  await p().fill('input[name="modello_generazione"]', 'claude-haiku-5-5');
  await p().click('button[name="salva-modelli"]');
  await attendi();
  assert.equal(await p().locator('.errore-modulo').innerText(), '');
  const c = (await stato()).configurazione;
  assert.equal(c.modello_generazione, 'claude-haiku-5-5');
  assert.equal(c.modalita_output, 'strumento_forzato');
  await scheda('proposta');
  const prima = messaggi().length;
  await proponi('Tinteggiatura di una parete di 4 x 3 m');
  const m = messaggi().slice(prima);
  assert.equal(m.length, 2);
  assert.ok(m.every((x) => x.corpo.model === 'claude-haiku-5-5' && x.corpo.tool_choice?.type === 'tool'));
  assert.match(await p().locator('.ai-risultato h2').innerText(), /Completa/);
  await schermata('ai-05-impostazioni');
});

test('revoca del consenso: di nuovo nessuna chiamata possibile', async () => {
  await scheda('impostazioni');
  await p().click('button[name="revoca-consenso"]');
  await p().locator('.modale button', { hasText: 'Revoca' }).click();
  await attendi();
  assert.equal((await stato()).consenso.valido, false);
  await scheda('proposta');
  await p().waitForSelector('.ai-consenso');
  const r = await b.motore({ richiesta: 'assistente_proponi', prezzario: 'prezzario_prova', testo: 'parete 4 x 3 m' });
  assert.equal(r.errore.motivo, 'consenso_mancante');
});

test('nessun errore JavaScript né violazione della Content Security Policy', () => {
  assert.deepEqual(b.errori, []);
});
