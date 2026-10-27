"""File di progetto validi per lo schema ma incoerenti: nessun crash né blocco."""
import copy, json, os, sys, tempfile
sys.path.insert(0, os.path.dirname(__file__))
from servizio import Servizio, Morto, Bloccato, copia_v6, esempi_comandi
BASE = json.load(open(copia_v6()))
LAV = tempfile.mkdtemp(prefix='corr-')
REPORT = [{'tipo': 'computo'}, {'tipo': 'computo', 'con_analisi': True}, {'tipo': 'computo', 'tradizionale': {}}, {'tipo': 'elenco_prezzi'},
          {'tipo': 'libretto'}, {'tipo': 'registro'}, {'tipo': 'sal', 'numero': 1}, {'tipo': 'certificato', 'numero': 2}, {'tipo': 'sommario'},
          {'tipo': 'conto_finale'}, {'tipo': 'analisi_prezzi'}, {'tipo': 'fabbisogno'}, {'tipo': 'nuovi_prezzi'}]
U = lambda n: '00000000-0000-0000-0000-%012x' % n

def varianti():
    p = BASE['progetto']
    def v(nome, f):
        d = copy.deepcopy(BASE); f(d['progetto']); return nome, d
    yield v('capitoli in ciclo', lambda p: p['elenco_prezzi']['capitoli'].extend([
        {'id': U(0xC1), 'padre': U(0xC2), 'codice': 'X', 'descrizione': 'X'},
        {'id': U(0xC2), 'padre': U(0xC1), 'codice': 'Y', 'descrizione': 'Y'}]) or p['elenco_prezzi']['voci'][0].__setitem__('capitolo', U(0xC1)))
    yield v('capitolo padre di sé stesso', lambda p: p['elenco_prezzi']['capitoli'].append({'id': U(0xC3), 'padre': U(0xC3), 'codice': 'Z', 'descrizione': 'Z'}) or p['elenco_prezzi']['voci'][1].__setitem__('capitolo', U(0xC3)))
    yield v('capitolo padre inesistente', lambda p: p['elenco_prezzi']['capitoli'].append({'id': U(0xC4), 'padre': U(0xDEAD), 'codice': 'Q', 'descrizione': 'Q'}))
    yield v('wbs in ciclo', lambda p: p['wbs']['nodi'][0].__setitem__('padre', p['wbs']['nodi'][1]['id']))
    yield v('wbs padre di sé', lambda p: p['wbs']['nodi'][0].__setitem__('padre', p['wbs']['nodi'][0]['id']))
    yield v('prezzo enorme', lambda p: p['computi'][0]['voci'][0]['prezzo'].__setitem__('prezzo', '79228162514264337593543950335'))
    yield v('fattore enorme', lambda p: p['computi'][0]['voci'][0]['righi'][0].__setitem__('lunghezza', {'valore': '79228162514264337593543950335'}))
    yield v('quantità statica enorme', lambda p: p['computi'][0]['voci'][4]['righi'][2]['vedi_voce'].__setitem__('statica', {'quantita': '79228162514264337593543950335', 'riferimento': 'x'}))
    yield v('decimali 28', lambda p: p['regole'].update({'decimali_quantita': 28, 'decimali_importo': 28, 'decimali_prezzo': 28}))
    yield v('decimali 255', lambda p: p['regole'].update({'decimali_quantita': 255}))
    yield v('voce id duplicato', lambda p: p['computi'][0]['voci'][1].__setitem__('id', p['computi'][0]['voci'][0]['id']))
    yield v('rigo id duplicato', lambda p: p['computi'][0]['voci'][0]['righi'][1].__setitem__('id', p['computi'][0]['voci'][0]['righi'][0]['id']))
    yield v('vedi voce di altro computo', lambda p: p['computi'].append({'id': U(0xE1), 'nome': 'Altro', 'voci': [dict(copy.deepcopy(p['computi'][0]['voci'][0]), id=U(0xE2), righi=[{'id': U(0xE3), 'vedi_voce': {'collegata': p['computi'][0]['voci'][1]['id']}}])]}))
    yield v('vedi voce a sé con duplicati', lambda p: p['computi'][0]['voci'][0]['righi'].append({'id': U(0xE4), 'vedi_voce': {'collegata': p['computi'][0]['voci'][0]['id']}}))
    yield v('SAL con registrazione inesistente', lambda p: p['contabilita']['sal'][1]['registrazioni'].append(U(0xBEEF)))
    yield v('due SAL stesso numero', lambda p: p['contabilita']['sal'][1].__setitem__('numero', 1))
    yield v('registrazione in due SAL', lambda p: p['contabilita']['sal'][1]['registrazioni'].append(p['contabilita']['sal'][0]['registrazioni'][0]))
    yield v('analisi con sé stessa', lambda p: p['elenco_prezzi']['analisi'][0]['righe'].append({'origine': {'voce': p['elenco_prezzi']['voci'][-2]['id']}, 'descrizione': 'x', 'unita_misura': 'm', 'quantita': '1', 'prezzo': '1'}) if p['elenco_prezzi']['analisi'] else None)
    yield v('testi con controlli', lambda p: p['metadati'].update({'titolo': 'a\u0000b‮c퟿' + 'z' * 20000}))
    yield v('nessun computo', lambda p: p.__setitem__('computi', []))
    yield v('computo vuoto', lambda p: p['computi'][0].__setitem__('voci', []))
    yield v('percentuali negative', lambda p: p['contratto']['parametri'].update({'ribasso': '-50', 'ritenuta_garanzia': '150'}))
    yield v('date invertite SAL', lambda p: p['contabilita']['sal'][1].update({'lavori_a_tutto_il': '2020-01-01'}))

def tutte_le_richieste(s, cart):
    out = []
    for r in [{'richiesta': x} for x in ('info', 'valida', 'progetto', 'contabilita', 'analisi_prezzi', 'nuovi_prezzi')] + [
            {'richiesta': 'computo'}, {'richiesta': 'computo', 'computo': 'Altro'}, {'richiesta': 'elenco_prezzi'}, {'richiesta': 'fabbisogno'},
            {'richiesta': 'cerca', 'testo': 'a'}, {'richiesta': 'esporta_xpwe', 'percorso': os.path.join(cart, 'o.xpwe')},
            {'richiesta': 'esporta_dcf', 'percorso': os.path.join(cart, 'o.dcf')}, {'richiesta': 'salva', 'percorso': os.path.join(cart, 's.ocp')}]:
        out.append((r, s.req(r)))
    for rep in REPORT:
        for formato in ('pdf', 'xlsx', 'csv'):
            r = {'richiesta': 'report', 'report': rep, 'formato': formato, 'cartella': cart}
            out.append((r, s.req(r)))
        r = {'richiesta': 'anteprima', 'report': rep}
        out.append((r, s.req(r)))
    # Un comando qualsiasi, poi annulla.
    out.append(('esegui', s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'aggiungi_rigo', 'voce': 1, 'rigo': {'lunghezza': 1}}]})))
    out.append(('annulla', s.req({'richiesta': 'annulla'})))
    return out

for nome, d in varianti():
    f = os.path.join(LAV, nome.replace(' ', '_') + '.ocp')
    json.dump(d, open(f, 'w'), ensure_ascii=False)
    cart = f + '.u'; os.makedirs(cart, exist_ok=True)
    s = Servizio(timeout=15)
    try:
        a = s.req({'richiesta': 'apri', 'percorso': f})
        if not a.get('ok'):
            print(f'[{nome}] apertura rifiutata: {a["errore"]["messaggio"][:150]}')
            continue
        esiti = tutte_le_richieste(s, cart)
        falliti = [(r if isinstance(r, str) else (r.get('richiesta'), r.get('report', {}).get('tipo') if isinstance(r, dict) else '', r.get('formato', '')), str(e['errore'].get('messaggio', e['errore']))[:160]) for r, e in esiti if not e.get('ok') and 'solo in PDF' not in str(e) and 'Altro' not in str(e)]
        val = s.req({'richiesta': 'valida'})['risultato']
        print(f'[{nome}] aperto; validazione: {val["errori"]} errori ({", ".join(sorted(set(p["codice"] for p in val["problemi"] if p["gravita"]=="errore")))}); richieste fallite: {len(falliti)}')
        for x in falliti[:6]:
            print('      ', x)
    except Morto as e:
        print(f'[{nome}] *** CRASH: {str(e)[-800:]}')
    except Bloccato as e:
        print(f'[{nome}] *** BLOCCO su {e}')
    finally:
        s.chiudi()
