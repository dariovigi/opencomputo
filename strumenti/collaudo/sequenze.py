"""Sequenze casuali di comandi sul servizio con controllo di invarianti."""
import copy, json, os, random, shutil, sys, tempfile, traceback
from decimal import Decimal, ROUND_HALF_UP, getcontext
getcontext().prec = 80
sys.path.insert(0, os.path.dirname(__file__))
from servizio import Servizio, Morto, Bloccato, copia_v6, esempi_comandi

SEED = int(os.environ.get('SEED', '1'))
PASSI = int(os.environ.get('PASSI', '400'))
rnd = random.Random(SEED)
ESEMPI = esempi_comandi()
BASE = copia_v6()
LAVORO = tempfile.mkdtemp(prefix='seq-')
reperti = []

def reperto(tipo, dettaglio, s=None):
    reperti.append({'seed': SEED, 'tipo': tipo, 'dettaglio': str(dettaglio)[:1500],
                    'ultime': (s.ultime[-6:] if s else [])})

CODICI = ['A.01.001', 'B.02.010', 'C.03.020', 'S.01.001', 'NP.01', 'NP.02', 'ZZ.99', '']
NOMI = ['Computo metrico estimativo', 'Variante', 'Ж', '']
STRANI = ['', ' ', 'x' * 3000, '<img src=x onerror=alert(1)>', '=SUM(A1:A9)', '😀\u0000‮', 'a"b\\c', '\n\t']
NUMERI = [0, -1, 1, 2, 3, 4, 5, 6, 7, 9, 99, 10**12, 2**64, '1', '-0', '1,5', '1.5', 'abc', '1e9', '99999999999999999999999999999', None, True, 0.1]

def muta_valore(chiave, v):
    r = rnd.random()
    if r < 0.65:
        return v
    if isinstance(v, bool):
        return not v if r < 0.9 else 'sì'
    if isinstance(v, (int, float)) or (isinstance(v, str) and v.replace('.', '').replace(',', '').replace('-', '').isdigit()):
        return rnd.choice(NUMERI)
    if isinstance(v, str):
        if chiave in ('codice', 'voce', 'nuovo_codice'):
            return rnd.choice(CODICI + STRANI[:2])
        if chiave in ('computo', 'da', 'a', 'nome'):
            return rnd.choice(NOMI)
        if len(v) == 10 and v[4] == '-':
            return rnd.choice(['2026-02-30', '1900-01-01', '2026-13-01', '2099-12-31', '2026-05-%02d' % rnd.randint(1, 28), 'ieri'])
        return rnd.choice(STRANI + [v])
    if isinstance(v, list):
        v = [muta_valore(chiave, x) for x in v]
        if v and rnd.random() < 0.3:
            v = v + v[:1]
        if rnd.random() < 0.15:
            v = []
        return v
    if isinstance(v, dict):
        return muta_dict(v)
    return v

def muta_dict(d):
    d = {k: muta_valore(k, v) for k, v in d.items()}
    if d and rnd.random() < 0.1:
        d.pop(rnd.choice([k for k in d if k != 'comando'] or ['x']), None)
    return d

def comando_valido(stato):
    """Comandi plausibili sulla struttura attuale."""
    n_voci = stato.get('n_voci', 5)
    k = rnd.randint(0, 11)
    v = rnd.randint(1, max(1, n_voci))
    if k == 0:
        return {'comando': 'aggiungi_voce_computo', 'codice': rnd.choice(CODICI[:6]), 'posizione': rnd.randint(1, n_voci + 1),
                'righi': [{'lunghezza': str(rnd.randint(1, 50)), 'larghezza': '%d,%02d' % (rnd.randint(0, 9), rnd.randint(0, 99))}]}
    if k == 1:
        return {'comando': 'aggiungi_rigo', 'voce': v, 'rigo': {'vedi_voce': rnd.randint(1, max(1, n_voci)), 'parti_uguali': rnd.choice(['0,5', '2', '-1', '1/3']), 'deduzione': rnd.random() < 0.3}}
    if k == 2:
        return {'comando': 'rimuovi_voce_computo', 'voce': v}
    if k == 3:
        return {'comando': 'sposta_voce_computo', 'voce': v, 'posizione': rnd.randint(1, n_voci)}
    if k == 4:
        voci = sorted(set(rnd.randint(1, n_voci) for _ in range(rnd.randint(1, 3))))
        return {'comando': 'copia_voci_computo', 'voci': voci, 'posizione': rnd.randint(1, n_voci + 1), 'sposta': rnd.random() < 0.5}
    if k == 5:
        return {'comando': 'modifica_rigo', 'voce': v, 'numero': rnd.randint(1, 3), 'rigo': {'descrizione': rnd.choice(STRANI), 'lunghezza': rnd.choice(['2*(3+4)', '1/3', '0', '-5', '1e3', '1000000000000'])}}
    if k == 6:
        return {'comando': 'registra', 'data': '2026-%02d-%02d' % (rnd.randint(5, 9), rnd.randint(1, 28)), 'codice': rnd.choice(CODICI[:6]),
                'righi': [{'lunghezza': str(rnd.randint(1, 30))}, {'vedi_voce': {'computo': 'Computo metrico estimativo', 'voce': v}}]}
    if k == 7:
        return {'comando': 'nuovo_sal', 'lavori_a_tutto_il': '2026-%02d-28' % rnd.randint(5, 10)}
    if k == 8:
        return {'comando': 'emetti_sal', 'numero': rnd.randint(1, 4), 'data_emissione': '2026-%02d-15' % rnd.randint(5, 11)}
    if k == 9:
        return {'comando': 'aggiorna_prezzi', 'oggetto': 'voci', 'percentuale': rnd.choice(['5', '-10', '0', '-100', '1000'])}
    if k == 10:
        return {'comando': 'aggiungi_rigo', 'voce': v, 'rigo': {'lunghezza': rnd.choice(['2', '3,5', '1/3']), 'vedi_voce': {'quantita': '7', 'riferimento': 'x'}}}
    return {'comando': 'imposta_regole', 'decimali_quantita': rnd.choice([0, 1, 2, 3, 6, 9]), 'decimali_importo': rnd.choice([0, 2, 3])}

def istantanea(s):
    info = s.req({'richiesta': 'info'})['risultato']
    comp = []
    for c in info['computi']:
        r = s.req({'richiesta': 'computo', 'computo': c['nome']})
        comp.append(r.get('risultato'))
    cont = s.req({'richiesta': 'contabilita'}).get('risultato')
    el = s.req({'richiesta': 'elenco_prezzi'}).get('risultato')
    prog = s.req({'richiesta': 'progetto'}).get('risultato')
    return {'computi': comp, 'contabilita': cont, 'elenco': el, 'progetto': prog}

def D(x):
    return Decimal(str(x)) if x is not None else None

def controlla(s, ist):
    for c in ist['computi']:
        if not c or c.get('totale') is None:
            continue
        lav = sum((D(v['importo']) for v in c['voci'] if v['natura'] == 'lavoro' and v['importo'] is not None), Decimal(0))
        sic = sum((D(v['importo']) for v in c['voci'] if v['natura'] != 'lavoro' and v['importo'] is not None), Decimal(0))
        if any(v['importo'] is None for v in c['voci']):
            reperto('totale con voci non calcolabili', c['nome'], s)
        if lav != D(c['totale_lavori']) or sic != D(c['totale_oneri_sicurezza']) or lav + sic != D(c['totale']):
            reperto('totali incoerenti', (c['nome'], lav, sic, c['totale_lavori'], c['totale_oneri_sicurezza'], c['totale']), s)
        regole = ist['progetto']['regole']
        for v in c['voci']:
            if v['quantita'] is None:
                continue
            somma = sum((D(r['quantita']) for r in v['righi'] if r['quantita'] is not None and not r['commento']), Decimal(0))
            dq = regole['decimali_quantita']
            if regole.get('arrotonda_quantita_parziali', True) and somma.quantize(Decimal(1).scaleb(-dq), ROUND_HALF_UP) != D(v['quantita']):
                reperto('quantità della voce ≠ somma dei righi', (c['nome'], v['numero'], somma, v['quantita']), s)
    val = s.req({'richiesta': 'valida'})['risultato']
    if val['errori']:
        reperto('errori di validazione dopo comandi accettati', [p for p in val['problemi'] if p['gravita'] == 'errore'][:3], s)

def esegui_passo(s, stato):
    if rnd.random() < 0.8:
        c = comando_valido(stato)
    else:
        c = muta_dict(copy.deepcopy(rnd.choice([e for e in ESEMPI if e['comando'] != 'nuovo_computo'])))
    if rnd.random() < 0.1:
        cmds = [c, comando_valido(stato)]
    else:
        cmds = [c]
    prima = istantanea(s)
    r = s.req({'richiesta': 'esegui', 'comandi': cmds})
    if not r.get('ok'):
        msg = json.dumps(r.get('errore'), ensure_ascii=False)
        for sospetto in ('panick', 'index out', 'unwrap', 'rust_backtrace', 'internal'):
            if sospetto in msg.lower():
                reperto('errore interno', msg, s)
        dopo = istantanea(s)
        if dopo != prima:
            reperto('comando rifiutato ma stato cambiato', cmds, s)
        return False
    dopo = istantanea(s)
    controlla(s, dopo)
    stato['n_voci'] = len(dopo['computi'][0]['voci']) if dopo['computi'] and dopo['computi'][0] else 0
    # Annulla e ripristina.
    if rnd.random() < 0.3:
        a = s.req({'richiesta': 'annulla'})
        if a.get('ok') and istantanea(s) != prima:
            reperto('annulla non ripristina lo stato precedente', cmds, s)
        rp = s.req({'richiesta': 'ripristina'})
        if rp.get('ok') and istantanea(s) != dopo:
            reperto('ripristina non riporta lo stato successivo', cmds, s)
    return True

REPORT = [{'tipo': 'computo'}, {'tipo': 'computo', 'con_analisi': True}, {'tipo': 'computo', 'tradizionale': {}},
          {'tipo': 'elenco_prezzi'}, {'tipo': 'elenco_prezzi', 'tradizionale': {}}, {'tipo': 'libretto'}, {'tipo': 'registro'},
          {'tipo': 'sal', 'numero': 1}, {'tipo': 'certificato', 'numero': 2}, {'tipo': 'sommario'}, {'tipo': 'conto_finale'},
          {'tipo': 'analisi_prezzi'}, {'tipo': 'analisi_prezzi', 'sintetica': True}, {'tipo': 'fabbisogno'}, {'tipo': 'nuovi_prezzi'},
          {'tipo': 'nuovi_prezzi', 'verbale': 1}]

def prova_uscite(s, k):
    cart = os.path.join(LAVORO, f'u{k}')
    os.makedirs(cart, exist_ok=True)
    for rep in REPORT:
        for formato in ('pdf', 'xlsx', 'csv'):
            r = s.req({'richiesta': 'report', 'report': rep, 'formato': formato, 'cartella': cart})
            if not r.get('ok'):
                msg = json.dumps(r['errore'], ensure_ascii=False)
                if any(x in msg.lower() for x in ('panick', 'index out', 'unwrap')):
                    reperto('errore interno nel report', (rep, formato, msg), s)
    for x in ('esporta_xpwe', 'esporta_dcf'):
        est = '.xpwe' if x == 'esporta_xpwe' else '.dcf'
        s.req({'richiesta': x, 'percorso': os.path.join(cart, 'out' + est)})
    # Salvataggio e riapertura.
    f = os.path.join(cart, 'p.ocp')
    r = s.req({'richiesta': 'salva', 'percorso': f})
    if not r.get('ok'):
        reperto('salvataggio fallito', r, s)
        return
    ist = istantanea(s)
    s2 = Servizio()
    try:
        r2 = s2.req({'richiesta': 'apri', 'percorso': f})
        if not r2.get('ok'):
            reperto('riapertura fallita', r2, s)
        else:
            i2 = istantanea(s2)
            for k_ in ('computi', 'contabilita', 'elenco'):
                if i2[k_] != ist[k_]:
                    reperto('dati diversi dopo salva e riapri', k_, s)
        # Reimport XPWE: totale del computo.
        xp = os.path.join(cart, 'out.xpwe')
        if os.path.exists(xp):
            r3 = Servizio()
            try:
                ri = r3.req({'richiesta': 'importa', 'origine': xp})
                if ri.get('ok'):
                    c1 = ist['computi'][0]
                    c2 = r3.req({'richiesta': 'computo'}).get('risultato')
                    if c1 and c2 and c1.get('totale') != c2.get('totale'):
                        reperto('totale diverso dopo export e import XPWE', (c1.get('totale'), c2.get('totale')), s)
                else:
                    reperto('XPWE esportato non reimportabile', ri.get('errore'), s)
            finally:
                r3.chiudi()
    finally:
        s2.chiudi()

def main():
    lavoro = os.path.join(LAVORO, 'base.ocp')
    shutil.copy(BASE, lavoro)
    s = Servizio()
    s.req({'richiesta': 'apri', 'percorso': lavoro})
    stato = {'n_voci': 5}
    accettati = 0
    try:
        for passo in range(PASSI):
            if esegui_passo(s, stato):
                accettati += 1
            if passo % 80 == 79:
                prova_uscite(s, passo)
    except Morto as e:
        reperto('CRASH del servizio', e, s)
    except Bloccato as e:
        reperto('BLOCCO del servizio (timeout)', e, s)
    except Exception:
        reperto('errore dello script', traceback.format_exc(), s)
    finally:
        s.chiudi()
    print(json.dumps({'seed': SEED, 'accettati': accettati, 'reperti': reperti}, ensure_ascii=False))

main()
