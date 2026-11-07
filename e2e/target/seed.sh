#!/bin/bash
# Remet les projets de test dans leur état de départ (tableau « Préparation »
# de docs/tests-jalon-11.md). Lancé en root dans la cible par les tests :
#   /e2e/seed.sh               projets changés depuis leur semis, plus le
#                              conteneur « kuma »
#   /e2e/seed.sh --full        tous les projets, plus « kuma » (lancement)
#   /e2e/seed.sh nominal anon  ces projets, sans vérification
# Un projet semé est descendu, ses volumes supprimés, son dossier recopié
# depuis /e2e/projects, puis lancé comme sur le VPS. Son empreinte est alors
# gardée : le semis suivant ne recrée que les projets dont l'empreinte a
# changé (voir `state`).
set -euo pipefail
# Une erreur dans le relevé de l'empreinte arrête le semis.
shopt -s inherit_errexit

ALL=(nominal partial failed oom loop multi envfile anon profiled slow)
TESTS=/home/atlas/atlas-tests
PLUGIN=/usr/libexec/docker/cli-plugins/docker-compose
# Hors du volume Docker : perdues avec le conteneur, donc semis complet.
PRINTS=/var/tmp/atlas-e2e-seed

as_atlas() { runuser -u atlas -- "$@"; }
compose_in() { (cd "$TESTS/$1" && shift && as_atlas docker compose --progress quiet "$@"); }

# Ce qu'un test peut changer, une ligne par élément, préfixée par le projet :
# conteneurs (y compris ceux de compose run), réseaux, volumes, dossier du
# projet et sa copie .bak (chemins, types, droits, propriétaires, contenus).
# L'état d'un conteneur à redémarrage automatique (loop-crash) varie seul :
# seul son identifiant compte. Un montage par ligne : Docker ne les rend pas
# toujours dans le même ordre, les lignes sont triées ensuite.
state() {
  docker ps -aq | xargs -r docker inspect --format \
    '{{$p := index .Config.Labels "com.docker.compose.project"}}{{$p}} c {{.Id}} {{.Name}} {{.Image}} {{if eq .HostConfig.RestartPolicy.Name "always"}}-{{else}}{{.State.Status}} {{.State.ExitCode}} {{.State.OOMKilled}} {{.State.StartedAt}}{{end}}{{$id := .Id}}{{range .Mounts}}{{"\n"}}{{$p}} k {{$id}} {{.Type}}:{{.Source}}{{end}}'
  docker network ls -q | xargs -r docker network inspect --format \
    '{{index .Labels "com.docker.compose.project"}} n {{.Id}}'
  docker volume ls -q | xargs -r docker volume inspect --format \
    '{{index .Labels "com.docker.compose.project"}} v {{.Name}} {{.CreatedAt}}'
  for p in "${ALL[@]}"; do
    for dir in "$TESTS/$p" "$TESTS/$p.bak"; do
      [ -e "$dir" ] || continue
      find "$dir" -printf "$p f %p %y %m %u:%g %s %l\n"
      find "$dir" -type f -exec md5sum {} + | sed "s|^|$p m |"
    done
  done
}

print_of() { { grep "^$1 " <<<"$2" || :; } | sort | md5sum | cut -d' ' -f1; }
kuma_print() { docker inspect --format '{{.Id}} {{.State.Status}}' kuma 2>/dev/null || :; }

# Un test a pu masquer le plugin (section 3) ou modifier une image.
[ -e "$PLUGIN" ] || mv "$PLUGIN.off" "$PLUGIN"

# Semis complet ou vérifié : nettoyage avant le relevé des empreintes.
if [ "${1:-}" = "" ] || [ "${1:-}" = "--full" ]; then
  # Volumes anonymes détachés par les tests de down (section 8) : la
  # section 0 attend seulement ceux du projet anon.
  as_atlas docker volume prune -f >/dev/null
  # Conteneurs du jalon 13 (seed-resources.sh) : hors compose, ils
  # fausseraient le décompte de la section 4 du jalon 11.
  as_atlas docker rm -f atlas-limite atlas-libre atlas-memoire >/dev/null 2>&1 || :
fi

full=0
verify=0
case "${1:-}" in
  --full) full=1; projects=("${ALL[@]}") ;;
  "")
    verify=1
    projects=()
    current=$(state)
    for p in "${ALL[@]}"; do
      [ "$(print_of "$p" "$current")" = "$(cat "$PRINTS/$p" 2>/dev/null)" ] || projects+=("$p")
    done
    ;;
  *) projects=("$@") ;;
esac

for p in "${projects[@]}"; do
  as_atlas docker compose -p "$p" --profile '*' down --volumes --remove-orphans -t 0 >/dev/null 2>&1 || :
  # Conteneurs de compose run, que down peut laisser.
  docker ps -aq --filter "label=com.docker.compose.project=$p" | xargs -r docker rm -f >/dev/null
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

if [ $full -eq 1 ] || { [ $verify -eq 1 ] && [ "$(kuma_print)" != "$(cat "$PRINTS/kuma" 2>/dev/null)" ]; }; then
  as_atlas docker rm -f kuma >/dev/null 2>&1 || :
  as_atlas docker run -d --name kuma busybox:1.37 sleep 1d >/dev/null
fi

# Conteneurs qui doivent sortir d'eux-mêmes (migrate, job, hog) : attendus
# avant de rendre la main, et avant de relever les empreintes.
exits=()
for p in "${projects[@]}"; do
  case "$p" in
    partial) exits+=(partial-migrate-1) ;;
    failed)  exits+=(failed-job-1) ;;
    oom)     exits+=(oom-hog-1) ;;
  esac
done
[ ${#exits[@]} -eq 0 ] || timeout 60 docker wait "${exits[@]}" >/dev/null

mkdir -p "$PRINTS"
[ $full -eq 0 ] || rm -f "$PRINTS"/*
if [ ${#projects[@]} -gt 0 ]; then
  current=$(state)
  for p in "${projects[@]}"; do print_of "$p" "$current" > "$PRINTS/$p"; done
fi
[ $full -eq 0 ] && [ $verify -eq 0 ] || kuma_print > "$PRINTS/kuma"

echo "semés : ${projects[*]:-aucun}"
