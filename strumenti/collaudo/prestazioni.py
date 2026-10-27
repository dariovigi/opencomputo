import json, os, sys, time, tempfile
sys.path.insert(0, os.path.dirname(__file__))
from servizio import Servizio, copia_v6
N = int(os.environ.get('N', '3000'))
s = Servizio(timeout=600)
s.req({'richiesta': 'nuovo', 'titolo': 'Grande'})
cmds = [{'comando': 'aggiungi_voce_prezzario', 'codice': f'V.{i:04d}', 'descrizione_breve': f'Voce {i} ' + 'descrizione lunga ' * 5, 'unita_misura': 'm²', 'prezzo': '12.34'} for i in range(200)]
t = time.time(); r = s.req({'richiesta': 'esegui', 'comandi': cmds}); print('200 voci elenco', r['ok'], round(time.time() - t, 2), 's')
blocco = 500
for k in range(0, N, blocco):
    cmds = []
    for i in range(k, min(N, k + blocco)):
        righi = [{'descrizione': f'r{j}', 'parti_uguali': '2', 'lunghezza': f'{j+1},5', 'larghezza': '0,30'} for j in range(4)]
        if i > 0 and i % 3 == 0:
            righi.append({'vedi_voce': i, 'parti_uguali': '0,1'})  # cita la voce precedente (catena)
        cmds.append({'comando': 'aggiungi_voce_computo', 'codice': f'V.{i % 200:04d}', 'righi': righi})
    t = time.time(); r = s.req({'richiesta': 'esegui', 'comandi': cmds}); print(f'voci {k}..{k+len(cmds)}', r['ok'], (r.get('errore') or {}).get('messaggio','')[:100], round(time.time() - t, 2), 's')
for nome, rq in [('computo', {'richiesta': 'computo'}), ('valida', {'richiesta': 'valida'}), ('info', {'richiesta': 'info'}),
                 ('un rigo', {'richiesta': 'esegui', 'comandi': [{'comando': 'modifica_rigo', 'voce': 1, 'numero': 1, 'rigo': {'lunghezza': '3'}}]}),
                 ('rimuovi voce citata', {'richiesta': 'esegui', 'comandi': [{'comando': 'rimuovi_voce_computo', 'voce': 3}]}),
                 ('copia 300 voci', {'richiesta': 'esegui', 'comandi': [{'comando': 'copia_voci_computo', 'voci': list(range(1, 301)), 'a': None}]}),
                 ('annulla', {'richiesta': 'annulla'}),
                 ('report pdf', {'richiesta': 'report', 'report': {'tipo': 'computo'}, 'formato': 'pdf', 'cartella': tempfile.mkdtemp()}),
                 ('report tradizionale', {'richiesta': 'report', 'report': {'tipo': 'computo', 'tradizionale': {}}, 'formato': 'pdf', 'cartella': tempfile.mkdtemp()}),
                 ('report xlsx', {'richiesta': 'report', 'report': {'tipo': 'computo'}, 'formato': 'xlsx', 'cartella': tempfile.mkdtemp()}),
                 ('xpwe', {'richiesta': 'esporta_xpwe', 'percorso': tempfile.mkdtemp() + '/x.xpwe'}),
                 ('salva', {'richiesta': 'salva', 'percorso': tempfile.mkdtemp() + '/g.ocp'})]:
    rq = {k: v for k, v in rq.items()}
    if nome == 'copia 300 voci':
        rq['comandi'][0].pop('a')
    t = time.time(); r = s.req(rq); dt = time.time() - t
    print(f'{nome:22s} {r["ok"]} {round(dt, 2)} s', (r.get('errore') or {}).get('messaggio', '')[:100], len(json.dumps(r)) // 1024, 'KB')
s.chiudi()
