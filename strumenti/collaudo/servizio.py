"""Pilota `opencomputo servizio` con richieste JSON e rileva crash e blocchi."""
import json, os, select, shutil, subprocess, tempfile, time

RADICE = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
# Binario del motore: compilare prima con `cargo build -p opencomputo-agent-api`.
BIN = os.environ.get('OPENCOMPUTO_BIN', os.path.join(RADICE, 'target', 'debug', 'opencomputo'))
FIXTURE_V6 = os.path.join(RADICE, 'crates', 'opencomputo-core', 'tests', 'fixtures', 'progetto_esempio_v6.json')


def copia_v6():
    """Copia temporanea del progetto di esempio (schema 6, con contabilità e «vedi voce»)."""
    f = os.path.join(tempfile.mkdtemp(prefix='collaudo-'), 'v6.ocp')
    shutil.copy(FIXTURE_V6, f)
    return f


def esempi_comandi():
    """Esempi di tutti i comandi, dalla guida del servizio."""
    s = Servizio()
    try:
        return [c['esempio'] for c in s.req({'richiesta': 'schema_comandi'})['risultato']['comandi']]
    finally:
        s.chiudi()

class Morto(Exception):
    pass

class Bloccato(Exception):
    pass

class Servizio:
    def __init__(self, timeout=20):
        self.timeout = timeout
        self.p = subprocess.Popen([BIN, 'servizio'], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, bufsize=0)
        self.buf = b''
        self.ultime = []

    def req(self, obj):
        riga = json.dumps(obj, ensure_ascii=False)
        self.ultime = (self.ultime + [riga])[-30:]
        try:
            self.p.stdin.write(riga.encode() + b'\n')
            self.p.stdin.flush()
        except BrokenPipeError:
            raise Morto(self.stderr())
        fine = time.time() + self.timeout
        while b'\n' not in self.buf:
            resto = fine - time.time()
            if resto <= 0:
                raise Bloccato(riga[:300])
            r, _, _ = select.select([self.p.stdout], [], [], resto)
            if not r:
                continue
            pezzo = os.read(self.p.stdout.fileno(), 1 << 20)
            if not pezzo:
                raise Morto(self.stderr())
            self.buf += pezzo
        linea, self.buf = self.buf.split(b'\n', 1)
        return json.loads(linea)

    def stderr(self):
        try:
            self.p.wait(timeout=2)
        except Exception:
            self.p.kill()
        return self.p.stderr.read().decode(errors='replace')[-3000:]

    def chiudi(self):
        try:
            self.p.kill()
        except Exception:
            pass
