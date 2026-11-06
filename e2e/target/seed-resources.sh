#!/bin/bash
# Préparation de docs/tests-jalon-13.md (section 1), lancée en root dans la
# cible par les specs du jalon 13 (`seedResources()`) :
#   - volume atlas-gros aux nombreux petits fichiers, qui allonge l'analyse
#     des volumes (rempli une fois, gardé d'un lancement à l'autre) ;
#   - conteneurs atlas-limite (256 Mo, monte atlas-gros) et atlas-libre (sans
#     limite) ;
#   - état normal rétabli après un test : accès au socket Docker, disque de
#     test démonté, conteneur de mémoire et processus de charge arrêtés.
set -euo pipefail

FILES_PER_DIR=1000
DIRS=${ATLAS_GROS_DIRS:-300}

as_atlas() { runuser -u atlas -- "$@"; }

# Démon suspendu par un test (holdDocker) : relancé.
pkill -CONT -x dockerd 2>/dev/null || :
chgrp docker /var/run/docker.sock && chmod 660 /var/run/docker.sock
umount /srv/prive/disque 2>/dev/null || :
pkill -x yes 2>/dev/null || :
as_atlas docker rm -f atlas-limite atlas-libre atlas-memoire >/dev/null 2>&1 || :

if ! as_atlas docker run --rm -v atlas-gros:/d busybox:1.37 test -e /d/.complet 2>/dev/null; then
  as_atlas docker volume create atlas-gros >/dev/null
  as_atlas docker run --rm -v atlas-gros:/d busybox:1.37 sh -c "
    for i in \$(seq 1 $DIRS); do
      mkdir -p /d/\$i && cd /d/\$i && for j in \$(seq 1 $FILES_PER_DIR); do : > \$j; done
    done
    touch /d/.complet"
fi

as_atlas docker run -d --name atlas-limite --memory 256m -v atlas-gros:/d busybox:1.37 sleep 1d >/dev/null
as_atlas docker run -d --name atlas-libre busybox:1.37 sleep 1d >/dev/null
