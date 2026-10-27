import json, os, sys, tempfile, zipfile, re, zlib
sys.path.insert(0, os.path.dirname(__file__))
from servizio import Servizio, copia_v6
T = tempfile.mkdtemp(prefix='car-')
s = Servizio()
TESTI = ['=1+1', '@SUM(A1)', '+41 cmd', '-2+3', 'Δ≤≥ ² ³ ½ € ° Ø æ ő ł', '中文 漢字', '😀 emoji', 'ctrl\u0001\u0008x', 'a&b<c>"d\'e', 'riga1\nriga2', '‮RTL']
print(s.req({'richiesta': 'nuovo', 'titolo': 'Prova caratteri =1+1 😀'})['ok'])
cmds = [{'comando': 'nuovo_computo', 'nome': 'C'}]
for i, t in enumerate(TESTI):
    cmds.append({'comando': 'aggiungi_voce_prezzario', 'codice': f'V.{i:02d}', 'descrizione_breve': t, 'descrizione_estesa': t + ' estesa', 'unita_misura': 'm²', 'prezzo': '10'})
    cmds.append({'comando': 'aggiungi_voce_computo', 'computo': 'C', 'codice': f'V.{i:02d}', 'righi': [{'descrizione': t, 'lunghezza': '2'}]})
r = s.req({'richiesta': 'esegui', 'comandi': cmds})
print('esegui', r['ok'], r.get('errore'))
for rep in [{'tipo': 'computo', 'computo': 'C'}, {'tipo': 'elenco_prezzi'}, {'tipo': 'computo', 'computo': 'C', 'tradizionale': {}}, {'tipo': 'elenco_prezzi', 'tradizionale': {}}]:
    for f in ('pdf', 'xlsx', 'csv'):
        r = s.req({'richiesta': 'report', 'report': rep, 'formato': f, 'cartella': T})
        print(rep, f, r['ok'], (r.get('errore') or {}).get('messaggio', '')[:100])
print('xpwe', s.req({'richiesta': 'esporta_xpwe', 'percorso': T + '/o.xpwe', 'computo': 'C'})['ok'])
print('dcf', s.req({'richiesta': 'esporta_dcf', 'percorso': T + '/o.dcf'})['ok'])
s.req({'richiesta': 'salva', 'percorso': T + '/p.ocp'})
s.chiudi()
print(sorted(os.listdir(T)))
# CSV: celle che iniziano con = + - @
for f in os.listdir(T):
    if f.endswith('.csv'):
        txt = open(os.path.join(T, f), encoding='utf-8-sig', errors='replace').read()
        celle = [c for riga in txt.splitlines() for c in riga.split(';')]
        peric = [c for c in celle if c[:1] in '=+@' or (c[:1] == '-' and not re.match(r'^-[\d.,]+$', c))]
        print('CSV', f, 'celle pericolose:', peric[:6])
for f in os.listdir(T):
    if f.endswith('.xlsx'):
        z = zipfile.ZipFile(os.path.join(T, f))
        fogli = [n for n in z.namelist() if n.startswith('xl/worksheets/')]
        x = ''.join(z.read(n).decode() for n in fogli)
        ss = z.read('xl/sharedStrings.xml').decode() if 'xl/sharedStrings.xml' in z.namelist() else ''
        print('XLSX', f, 'formule <f>:', x.count('<f>'), 'inlineStr/sharedStrings con =:', ('>=1+1<' in x) or ('>=1+1<' in ss), 'caratteri di controllo:', bool(re.search('[\x00-\x08\x0b\x0c\x0e-\x1f]', x + ss)))
x = open(T + '/o.xpwe', encoding='utf-8', errors='replace').read()
print('XPWE caratteri di controllo non validi in XML:', bool(re.search('[\x00-\x08\x0b\x0c\x0e-\x1f]', x)), '| emoji presenti:', '😀' in x)
s2 = Servizio()
r = s2.req({'richiesta': 'importa', 'origine': T + '/o.xpwe'})
print('reimport XPWE', r['ok'], (r.get('errore') or {}).get('messaggio', '')[:200])
if r['ok']:
    el = s2.req({'richiesta': 'elenco_prezzi'})['risultato']['voci']
    for v, t in zip(el, TESTI):
        if v['descrizione_breve'] != t:
            print('   DIVERSO', repr(t), '→', repr(v['descrizione_breve']))
s2.chiudi()
# PDF: i caratteri non rappresentabili
for f in os.listdir(T):
    if f.endswith('.pdf'):
        b = open(os.path.join(T, f), 'rb').read()
        print('PDF', f, len(b), 'byte; contiene ToUnicode:', b'ToUnicode' in b, 'font:', set(re.findall(rb'/BaseFont\s*/([A-Za-z-]+)', b)))
print(T)
