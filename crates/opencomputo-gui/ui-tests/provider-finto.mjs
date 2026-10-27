// Server HTTP locale che imita le API di OpenAI (embedding) e Anthropic (messaggi con
// output JSON vincolato) per i test dell'interfaccia: nessuna chiamata reale. Il motore lo
// usa solo se compilato con la feature `banco-di-prova` e con la variabile
// OPENCOMPUTO_PROVIDER_DI_PROVA.
//
// Il «modello» finto:
// * analisi: una lavorazione di tinteggiatura; se la richiesta parla di «pavimento»
//   senza misure, chiede la superficie (esito `chiedi`);
// * selezione: sceglie, tra i candidati davvero ricevuti, quello con «idropittura» nella
//   descrizione, con un rigo rettangolo dai numeri della richiesta; con «perimetro di
//   N m» l'altezza diventa un'ipotesi dichiarata;
// * una richiesta che contiene «chiave sbagliata» riceve 401;
// * con lo strumento forzato risponde con un blocco `tool_use`.

import { createServer } from 'node:http';

function vettore(testo, dimensioni) {
  const v = new Array(dimensioni).fill(0);
  for (const parola of testo.toLowerCase().split(/[^a-zà-ù0-9]+/).filter((p) => p.length > 2)) {
    let h = 7;
    for (const c of parola) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    v[h % dimensioni] += 1;
  }
  v[0] += 1;
  return v;
}

function analisi(utente) {
  if (/pavimento/i.test(utente) && !/\d/.test(utente)) {
    return {
      esito: 'chiedi',
      lavorazioni: [{ id: 'L01', descrizione: 'Pavimento', query_ricerca: 'pavimento', dimensione_attesa: 'superficie' }],
      informazioni_mancanti: [{ domanda: 'Quanto misura la superficie del pavimento?', bloccante: true, lavorazioni: ['L01'] }],
    };
  }
  return {
    esito: 'procedi',
    lavorazioni: [{ id: 'L01', descrizione: 'Tinteggiatura della parete', query_ricerca: 'tinteggiatura idropittura pareti', dimensione_attesa: 'superficie' }],
    informazioni_mancanti: [],
  };
}

function selezione(utente) {
  const i = utente.indexOf('handle):\n') + 'handle):\n'.length;
  const candidati = JSON.parse(utente.slice(i));
  const c = candidati.find((x) => x.descrizione.toLowerCase().includes('idropittura'));
  const perimetro = utente.match(/perimetro di (\d+) m/);
  if (perimetro) {
    // Altezza non indicata: ipotesi dichiarata, esito da rivedere.
    return {
      esito: 'provvisorio',
      lavorazioni: [{
        id: 'L01', descrizione: 'Tinteggiatura delle pareti', candidato: c ? c.handle : 'NESSUNO', ruolo: 'principale',
        gruppo_alternativa: '', complementare_di: '', motivazione: 'idropittura su pareti interne',
        righe: [{ descrizione: 'pareti', forma: 'rettangolo', segno: 'positivo', parametri: [
          { nome: 'N', valore: '1', unita: 'n', provenienza: 'utente', rif_ipotesi: '' },
          { nome: 'L', valore: perimetro[1], unita: 'm', provenienza: 'utente', rif_ipotesi: '' },
          { nome: 'B', valore: '2.70', unita: 'm', provenienza: 'ipotesi', rif_ipotesi: 'I01' },
        ] }],
      }],
      ipotesi: [{ id: 'I01', testo: 'altezza dei locali 2,70 m', impatto: 'medio' }],
      informazioni_mancanti: [{ domanda: 'Qual è l\'altezza dei locali?', bloccante: false, lavorazioni: ['L01'] }],
    };
  }
  const numeri = (utente.match(/(\d+(?:[.,]\d+)?)\s*x\s*(\d+(?:[.,]\d+)?)/) ?? [null, '4', '3']).slice(1);
  return {
    esito: 'completo',
    lavorazioni: [{
      id: 'L01', descrizione: 'Tinteggiatura della parete', candidato: c ? c.handle : 'NESSUNO', ruolo: 'principale',
      gruppo_alternativa: '', complementare_di: '', motivazione: 'idropittura su pareti interne',
      righe: [{ descrizione: 'parete', forma: 'rettangolo', segno: 'positivo', parametri: [
        { nome: 'N', valore: '1', unita: 'n', provenienza: 'utente', rif_ipotesi: '' },
        { nome: 'L', valore: numeri[0], unita: 'm', provenienza: 'utente', rif_ipotesi: '' },
        { nome: 'B', valore: numeri[1], unita: 'm', provenienza: 'utente', rif_ipotesi: '' },
      ] }],
    }],
    ipotesi: [],
    informazioni_mancanti: [],
  };
}

/** Avvia il server: {url, ricevute, chiudi()}. */
export async function avviaProviderFinto() {
  const ricevute = [];
  const s = createServer((req, res) => {
    let corpo = '';
    req.on('data', (d) => { corpo += d; });
    req.on('end', () => {
      const json = JSON.parse(corpo || '{}');
      const chiave = req.headers['x-api-key'] ?? (req.headers.authorization ?? '').replace(/^Bearer /, '');
      ricevute.push({ percorso: req.url, chiave, corpo: json });
      const rispondi = (stato, dati) => {
        res.writeHead(stato, { 'content-type': 'application/json', 'request-id': 'req_finto' });
        res.end(JSON.stringify(dati));
      };
      if (req.url === '/v1/embeddings') {
        const dimensioni = json.dimensions ?? 1536;
        rispondi(200, { data: json.input.map((t, index) => ({ index, embedding: vettore(t, dimensioni) })) });
        return;
      }
      if (req.url === '/v1/messages') {
        const utente = json.messages?.[0]?.content ?? '';
        if (/chiave sbagliata/i.test(utente)) {
          rispondi(401, { type: 'error', error: { type: 'authentication_error', message: 'invalid x-api-key' } });
          return;
        }
        const risposta = utente.includes('handle):\n') ? selezione(utente) : analisi(utente);
        // Strumento forzato: la risposta è l'input del blocco `tool_use`.
        const strumento = json.tool_choice?.name;
        rispondi(200, {
          content: strumento
            ? [{ type: 'tool_use', id: 'toolu_finto', name: strumento, input: risposta }]
            : [{ type: 'text', text: JSON.stringify(risposta) }],
          stop_reason: 'end_turn',
          usage: { input_tokens: 120, output_tokens: 30 },
        });
        return;
      }
      rispondi(404, { error: 'sconosciuto' });
    });
  });
  await new Promise((ok) => s.listen(0, '127.0.0.1', ok));
  return { url: `http://127.0.0.1:${s.address().port}`, ricevute, chiudi: () => s.close() };
}
