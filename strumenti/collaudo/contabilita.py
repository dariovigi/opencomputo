"""Scenari casuali di contabilità: invarianti di SAL, certificati e conto finale."""
import json, os, random, sys, tempfile
from decimal import Decimal as D, ROUND_HALF_UP, getcontext
getcontext().prec = 60
sys.path.insert(0, os.path.dirname(__file__))
from servizio import Servizio, copia_v6

def r2(x): return x.quantize(D('0.01'), ROUND_HALF_UP)
problemi = []
def verifica(cond, msg, ctx):
    if not cond: problemi.append((msg, ctx))

osservazioni = []
def segnala(cond, msg, ctx):
    """Comportamento da valutare, non un errore di calcolo."""
    if not cond: osservazioni.append((msg, ctx))

def scenario(seed):
    rnd = random.Random(seed)
    s = Servizio(timeout=60)
    s.req({'richiesta': 'apri', 'percorso': copia_v6()})
    # Contabilità nuova: si tolgono SAL e registrazioni del fixture ripartendo da un progetto nuovo con lo stesso elenco.
    s.req({'richiesta': 'nuovo', 'titolo': 'C'})
    voci = [('A.01', '8.50', 'lavoro', '25'), ('B.02', '145.30', 'lavoro', '18.5'), ('S.01', '12.00', 'oneri_sicurezza', None), ('NP.1', '17.71', 'lavoro', '40'), ('C.03', '0.37', 'lavoro', None)]
    cmds = []
    for c, p, n, mdo in voci:
        x = {'comando': 'aggiungi_voce_prezzario', 'codice': c, 'descrizione_breve': c, 'unita_misura': 'm', 'prezzo': p, 'natura': n}
        if mdo: x['incidenza_manodopera'] = mdo
        cmds.append(x)
    ribasso = rnd.choice(['0', '12.5', '27.333', '33.33', '9.99'])
    rit = rnd.choice(['0', '0.5', '1'])
    iva = rnd.choice([None, '10', '22', '4'])
    contr = {'comando': 'imposta_contratto', 'ribasso': ribasso, 'ritenuta_garanzia': rit, 'manodopera_esclusa_dal_ribasso': rnd.random() < 0.4,
             'importo_contrattuale': rnd.choice(['50000.00', '123456.78', '10000.00']), 'data_consegna_lavori': '2026-01-10'}
    if iva: contr['aliquota_iva'] = iva
    cmds.append(contr)
    if rnd.random() < 0.6:
        cmds.append({'comando': 'imposta_anticipazione', 'percentuale': rnd.choice(['20', '30', '10']), 'percentuale_recupero': rnd.choice(['20', '30', '15']), 'data_erogazione': '2026-01-15'})
    r = s.req({'richiesta': 'esegui', 'comandi': cmds})
    assert r['ok'], r
    if rnd.random() < 0.5:
        s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'imposta_nuovo_prezzo', 'voce': 'NP.1', 'criterio': 'analisi', 'data': '2026-02-01', 'motivo': 'x', 'soggetto_a_ribasso': rnd.random() < 0.5}]})
    mesi = rnd.randint(2, 6)
    for m in range(mesi):
        mese = 2 + m
        regs = []
        for _ in range(rnd.randint(1, 6)):
            c = rnd.choice(voci)[0]
            q = rnd.choice(['1', '2,5', '17,33', '0,07', '333', '1/3', '-2'])
            regs.append({'comando': 'registra', 'data': f'2026-{mese:02d}-{rnd.randint(1, 27):02d}', 'codice': c, 'righi': [{'lunghezza': q}, {'lunghezza': rnd.choice(['3', '1,25'])}]})
        r = s.req({'richiesta': 'esegui', 'comandi': regs})
        if not r['ok']: problemi.append(('registrazioni rifiutate', r['errore'])); continue
        if rnd.random() < 0.3:
            s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'registra_penale', 'data': f'2026-{mese:02d}-20', 'descrizione': 'r', 'giorni': rnd.randint(1, 30), 'per_mille': rnd.choice(['0.5', '1', '1.5'])}]})
        r = s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'nuovo_sal', 'lavori_a_tutto_il': f'2026-{mese:02d}-28'}]})
        if not r['ok']: problemi.append(('nuovo SAL rifiutato', r['errore'], seed)); continue
        n = m + 1
        if rnd.random() < 0.2:
            s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'imposta_sal', 'numero': n, 'recupero_anticipazione_sospeso': True}]})
        r = s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'emetti_sal', 'numero': n, 'data_emissione': f'2026-{mese + 1:02d}-03'}]})
        if not r['ok']: problemi.append(('emissione rifiutata', r['errore']['messaggio'], seed))
    cont = s.req({'richiesta': 'contabilita'})['risultato']
    par = cont['parametri']
    regs = {x['numero']: x for x in cont['registrazioni']}
    prec_credito = D(0); prec_lav = D(0); prec_on = D(0); recuperata = D(0)
    for sal in cont['sal']:
        ctx = (seed, sal['numero'])
        rp = sal['riepilogo']; ce = sal['certificato']
        if rp is None:
            problemi.append(('SAL senza riepilogo', sal.get('errore'), ctx)); continue
        lav = D(rp['lavori_lordi']); on = D(rp['oneri_sicurezza'])
        periodo = sum((D(regs[k]['importo']) for k in sal['registrazioni']), D(0))
        verifica(D(rp['lordo_periodo']) == periodo, f"lordo_periodo {rp['lordo_periodo']} ≠ Σ registrazioni {periodo}", ctx)
        verifica(lav + on - prec_lav - prec_on == periodo, 'progressivi incoerenti col periodo', ctx)
        verifica(D(rp['soggetto_a_ribasso']) == lav - D(rp.get('manodopera_esclusa', '0')) - D(rp.get('lavori_esclusi_dal_ribasso', '0')), 'soggetto a ribasso', ctx)
        verifica(D(rp['ribasso']) == r2(D(rp['soggetto_a_ribasso']) * D(par['ribasso']) / 100), f"ribasso {rp['ribasso']} vs {r2(D(rp['soggetto_a_ribasso']) * D(par['ribasso']) / 100)}", ctx)
        verifica(D(rp['importo_netto']) == lav + on - D(rp['ribasso']), 'importo netto', ctx)
        verifica(D(rp['ritenuta_garanzia']) == r2(D(rp['importo_netto']) * D(par['ritenuta_garanzia']) / 100), 'ritenuta', ctx)
        verifica(D(rp['credito_progressivo']) == D(rp['importo_netto']) - D(rp['ritenuta_garanzia']), 'credito progressivo', ctx)
        verifica(D(rp['certificati_precedenti']) == prec_credito, f"certificati precedenti {rp['certificati_precedenti']} ≠ {prec_credito}", ctx)
        verifica(D(rp['importo_sal']) == D(rp['credito_progressivo']) - D(rp['certificati_precedenti']), 'importo SAL', ctx)
        if ce:
            verifica(D(ce['importo_sal']) == D(rp['importo_sal']), 'certificato: importo SAL', ctx)
            verifica(D(ce['anticipazione_recuperata_prima']) == recuperata, f"anticipazione recuperata prima {ce['anticipazione_recuperata_prima']} ≠ {recuperata}", ctx)
            verifica(D(ce['anticipazione_residua']) == D(ce['anticipazione']) - recuperata - D(ce['recupero_anticipazione']), 'anticipazione residua', ctx)
            verifica(D(ce['anticipazione_residua']) >= 0, 'anticipazione residua negativa', ctx)
            verifica(D(ce['imponibile']) == D(ce['importo_sal']) - D(ce['recupero_anticipazione']) - D(ce['penali']), 'imponibile', ctx)
            aliq = D(par['aliquota_iva']) if par.get('aliquota_iva') else D(0)
            verifica(D(ce['iva']) == r2(D(ce['imponibile']) * aliq / 100), f"IVA {ce['iva']} vs {r2(D(ce['imponibile']) * aliq / 100)}", ctx)
            verifica(D(ce['totale']) == D(ce['imponibile']) + D(ce['iva']), 'totale certificato', ctx)
            if sal['recupero_anticipazione_sospeso']:
                verifica(D(ce['recupero_anticipazione']) == 0, 'recupero sospeso ma applicato', ctx)
            recuperata += D(ce['recupero_anticipazione'])
            segnala(D(ce['imponibile']) >= 0 or D(ce['importo_sal']) < 0, f"imponibile negativo {ce['imponibile']}: SAL {ce['importo_sal']}, recupero {ce['recupero_anticipazione']}, penali {ce['penali']}", ctx)
        prec_credito = D(rp['credito_progressivo']); prec_lav = lav; prec_on = on
    # Penali: al massimo il 10 % dell'importo contrattuale.
    tot_pen = sum((D(p['importo']) for p in cont['penali']), D(0))
    verifica(tot_pen <= D(cont['importo_contrattuale'] or '0') * D('0.1') + D('0.005'), f'penali {tot_pen} oltre il 10 %', (seed,))
    # Conto finale.
    r = s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'redigi_conto_finale', 'data': '2026-12-15', 'data_ultimazione': '2026-12-01'}]})
    if r['ok']:
        e = s.req({'richiesta': 'esegui', 'comandi': [{'comando': 'emetti_conto_finale', 'data': '2026-12-16'}]})
        cf = s.req({'richiesta': 'contabilita'})['risultato']['conto_finale']
        if cf and cf.get('riepilogo'):
            rf = cf['riepilogo']
            verifica(D(rf['credito_residuo']) == D(rf['importo_netto']) - D(rf['acconti']) if 'acconti' in rf else True, 'conto finale: credito residuo', (seed,))
        for rep in ('conto_finale', 'registro', 'sommario', 'libretto'):
            for fmt in ('pdf', 'xlsx', 'csv'):
                x = s.req({'richiesta': 'report', 'report': {'tipo': rep}, 'formato': fmt, 'cartella': tempfile.mkdtemp()})
                if not x['ok']: problemi.append(('report fallito', rep, fmt, x['errore']['messaggio'][:150], seed))
    else:
        problemi.append(('conto finale non redigibile', r['errore']['messaggio'][:200], seed))
    s.chiudi()

N = int(os.environ.get('N', '40'))
for seed in range(1, N + 1):
    try:
        scenario(seed)
    except Exception as ex:
        import traceback; problemi.append(('eccezione', traceback.format_exc()[-600:], seed))
from collections import Counter
print('scenari', N, 'problemi', len(problemi))
print(Counter(p[0] for p in problemi))
for p in problemi[:25]: print(' ', str(p)[:400])
print('osservazioni (certificati con imponibile negativo):', len(osservazioni))
for o in osservazioni[:5]: print(' ', o)
print('scenari con certificati negativi:', len({o[1][0] for o in osservazioni}), 'certificati:', len(osservazioni))
