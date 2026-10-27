import json, os, sys, tempfile, gzip, stat
sys.path.insert(0, os.path.dirname(__file__))
from servizio import Servizio, Morto, Bloccato, copia_v6, esempi_comandi
T = tempfile.mkdtemp(prefix='rob-')
def prova(nome, righe, s=None):
    s = s or Servizio(timeout=30)
    try:
        for r in righe:
            if isinstance(r, (bytes, str)):
                b = r.encode() if isinstance(r, str) else r
                s.p.stdin.write(b + b'\n'); s.p.stdin.flush()
                out = s.req({'richiesta': 'info'}) if False else None
                import select, os as o
                select.select([s.p.stdout], [], [], 10)
                linea = b''
                while not linea.endswith(b'\n'):
                    pezzo = o.read(s.p.stdout.fileno(), 1 << 16)
                    if not pezzo: raise Morto(s.stderr())
                    linea += pezzo
                ris = json.loads(linea)
            else:
                ris = s.req(r)
            print(f'[{nome}] →', json.dumps(ris, ensure_ascii=False)[:180])
    except (Morto, Bloccato) as e:
        print(f'[{nome}] *** {type(e).__name__}: {str(e)[-500:]}')
    finally:
        s.chiudi()

prova('json malformato', ['{', 'null', '[]', '"x"', '{"richiesta": 5}', '{"richiesta": "inesistente"}', '{"richiesta":"info","extra":1}'])
prova('byte non UTF-8', [b'\xff\xfe{"richiesta":"info"}'])
prova('riga enorme', ['{"richiesta":"cerca","testo":"' + 'a' * 20_000_000 + '"}'])
prova('nul', ['{"richiesta":"nuovo","titolo":"a\\u0000b"}'])
open(T + '/vuoto.ocp', 'w').close()
open(T + '/troncato.ocp', 'w').write(open(copia_v6()).read()[:5000])
open(T + '/bom.ocp', 'w', encoding='utf-8-sig').write(open(copia_v6()).read())
with gzip.open(T + '/gz.ocp', 'wt') as f: f.write(open(copia_v6()).read())
g = open(T + '/gz.ocp', 'rb').read(); open(T + '/gztroncato.ocp', 'wb').write(g[:len(g)//2])
open(T + '/futuro.ocp', 'w').write(open(copia_v6()).read().replace('"versione_schema": 6', '"versione_schema": 99'))
open(T + '/binario.ocp', 'wb').write(os.urandom(4096))
os.makedirs(T + '/cartella.ocp')
for f in ['vuoto', 'troncato', 'bom', 'gz', 'gztroncato', 'futuro', 'binario', 'cartella', 'inesistente']:
    prova('apri ' + f, [{'richiesta': 'apri', 'percorso': f'{T}/{f}.ocp'}])
prova('importa file non xpwe', [{'richiesta': 'importa', 'origine': T + '/binario.ocp'}])
prova('importa cartella', [{'richiesta': 'importa', 'origine': T}])
prova('carica prezzario binario', [{'richiesta': 'nuovo', 'titolo': 'x'}, {'richiesta': 'carica_prezzario', 'nome': 'b', 'percorso': T + '/binario.ocp'}])
# Salvataggi problematici.
os.makedirs(T + '/solalettura'); os.chmod(T + '/solalettura', stat.S_IRUSR | stat.S_IXUSR)
prova('salvataggi', [{'richiesta': 'nuovo', 'titolo': 'x'},
                     {'richiesta': 'salva', 'percorso': T + '/nonesiste/sotto/p.ocp'},
                     {'richiesta': 'salva', 'percorso': T + '/solalettura/p.ocp'},
                     {'richiesta': 'salva', 'percorso': T + '/cartella.ocp'},
                     {'richiesta': 'salva', 'percorso': T + '/èà ü 😀.ocp'},
                     {'richiesta': 'salva', 'percorso': T + '/senza_estensione'},
                     {'richiesta': 'salva'},
                     {'richiesta': 'salva', 'percorso': T + '/g.ocp', 'gzip': True},
                     {'richiesta': 'apri', 'percorso': T + '/g.ocp'}])
prova('chiudi con modifiche', [{'richiesta': 'nuovo', 'titolo': 'x'}, {'richiesta': 'esegui', 'comandi': [{'comando': 'imposta_metadati', 'titolo': 'y'}]}, {'richiesta': 'chiudi'}, {'richiesta': 'info'}, {'richiesta': 'chiudi', 'forza': True}, {'richiesta': 'info'}])
prova('report in cartella inesistente', [{'richiesta': 'apri', 'percorso': copia_v6()}, {'richiesta': 'report', 'report': {'tipo': 'computo'}, 'formato': 'pdf', 'cartella': T + '/no/no'},
     {'richiesta': 'report', 'report': {'tipo': 'computo'}, 'formato': 'pdf', 'file': T + '/solalettura/x.pdf'}, {'richiesta': 'report', 'report': {'tipo': 'sal', 'numero': 99}}, {'richiesta': 'report', 'report': {'tipo': 'certificato', 'numero': 0}}, {'richiesta': 'anteprima', 'report': {'tipo': 'nuovi_prezzi', 'verbale': 99}}])
print(sorted(os.listdir(T)))
