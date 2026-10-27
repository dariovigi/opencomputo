# Workflow di GitHub Actions

`ci.yml` (verifiche a ogni push) e `rilascio.yml` (pacchetti a ogni tag `v*`). Vanno
copiati in `.github/workflows/` quando il repository viene pubblicato su GitHub:

```sh
mkdir -p .github/workflows && cp strumenti/ci/*.yml .github/workflows/
```

Sono tenuti qui perché la cartella `.github/workflows` non è scrivibile dagli strumenti
remoti usati durante lo sviluppo. Decisioni in `docs/adr/0010-rilascio-e-dipendenze.md`.
