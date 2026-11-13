# Workflow di GitHub Actions

`ci.yml` (verifiche a ogni push su `main`) e `rilascio.yml` (pacchetti a ogni tag `v*`).
Dal 2026-11-13 le copie attive sono in `.github/workflows/`: questa cartella resta come
copia di lavoro. Dopo una modifica, riallinearle:

```sh
cp strumenti/ci/*.yml .github/workflows/
```

Decisioni in `docs/adr/0010-rilascio-e-dipendenze.md`; pubblicazione in `docs/RILASCIO.md`.
