#!/bin/bash
# Remet les projets de test dans leur état de départ (tableau « Préparation »
# de docs/tests-jalon-11.md). Lancé en root dans la cible par les tests :
#   /e2e/seed.sh               tous les projets, plus le conteneur « kuma »
#   /e2e/seed.sh nominal anon  seulement ceux-là
# Chaque projet est descendu, ses volumes supprimés, son dossier recopié
# depuis /e2e/projects, puis lancé comme sur le VPS.
set -euo pipefail

ALL=(nominal partial failed oom loop multi envfile anon profiled slow)
TESTS=/home/atlas/atlas-tests
PLUGIN=/usr/libexec/docker/cli-plugins/docker-compose

as_atlas() { runuser -u atlas -- "$@"; }
compose_in() { (cd "$TESTS/$1" && shift && as_atlas docker compose --progress quiet "$@"); }

# Un test a pu masquer le plugin (section 3) ou modifier une image.
[ -e "$PLUGIN" ] || mv "$PLUGIN.off" "$PLUGIN"

projects=("$@")
[ ${#projects[@]} -gt 0 ] || projects=("${ALL[@]}")

for p in "${projects[@]}"; do
  as_atlas docker compose -p "$p" --profile '*' down --volumes --remove-orphans -t 0 >/dev/null 2>&1 || :
  rm -rf "${TESTS:?}/$p" "${TESTS:?}/$p.bak"
  cp -r "/e2e/projects/$p" "$TESTS/$p"
  chown -R atlas:atlas "$TESTS/$p"
done

for p in "${projects[@]}"; do
  case "$p" in
    multi)    compose_in "$p" -f compose.yaml -f compose.prod.yaml up -d ;;
    envfile)  compose_in "$p" --env-file .env.prod up -d ;;
    profiled) compose_in "$p" --profile debug up -d ;;
    # Créé sans démarrer : le test lance lui-même le up long.
    slow)     compose_in "$p" create ;;
    *)        compose_in "$p" up -d ;;
  esac
done

if [ $# -eq 0 ]; then
  # Volumes anonymes détachés par les tests de down (section 8) : la
  # section 0 attend seulement ceux du projet anon.
  as_atlas docker volume prune -f >/dev/null
  as_atlas docker rm -f kuma >/dev/null 2>&1 || :
  # Conteneurs du jalon 13 (seed-resources.sh) : hors compose, ils
  # fausseraient le décompte de la section 4 du jalon 11.
  as_atlas docker rm -f atlas-limite atlas-libre atlas-memoire >/dev/null 2>&1 || :
  as_atlas docker run -d --name kuma busybox:1.37 sleep 1d >/dev/null
fi

# Conteneurs qui doivent sortir d'eux-mêmes (migrate, job, hog) : attendus
# avant de rendre la main.
exits=()
for p in "${projects[@]}"; do
  case "$p" in
    partial) exits+=(partial-migrate-1) ;;
    failed)  exits+=(failed-job-1) ;;
    oom)     exits+=(oom-hog-1) ;;
  esac
done
[ ${#exits[@]} -eq 0 ] || timeout 60 docker wait "${exits[@]}" >/dev/null
