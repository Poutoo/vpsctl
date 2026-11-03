#!/bin/bash
# Fichiers des tests de l'éditeur (docs/tests-jalon-12.md, « Préparation »),
# remis à zéro par les tests. Lancé en root dans la cible :
#   /e2e/seed-editor.sh            dossier editeur/ et authorized_keys
#   /e2e/seed-editor.sh keys       authorized_keys seul
# Les fichiers appartiennent au compte atlas, sauf autre.conf (compte autre).
set -euo pipefail

HOME_DIR=/home/atlas
DIR=$HOME_DIR/atlas-tests/editeur
KEYS=$HOME_DIR/.ssh/authorized_keys
WORK=/tmp/atlas-e2e-keys

as_atlas() { runuser -u atlas -- "$@"; }

# authorized_keys : la clé de la liaison de test, une seconde clé, une ligne
# avec options, un certificat signé par une autorité de test et une ligne
# illisible. Clés et autorité jetables, générées une fois.
seed_keys() {
  if [ ! -f "$WORK/ci.pub" ]; then
    rm -rf "$WORK" && mkdir -p "$WORK"
    ssh-keygen -q -t ed25519 -N '' -C ci@e2e -f "$WORK/ci"
    ssh-keygen -q -t ed25519 -N '' -C restreinte@e2e -f "$WORK/restreinte"
    ssh-keygen -q -t ed25519 -N '' -C ca@e2e -f "$WORK/ca"
    ssh-keygen -q -t ed25519 -N '' -C cert@e2e -f "$WORK/cert"
    ssh-keygen -q -s "$WORK/ca" -I e2e -n atlas "$WORK/cert.pub"
  fi
  {
    echo "# atlas-e2e · compte atlas"
    cat /e2e/authorized_keys
    cat "$WORK/ci.pub"
    printf 'from="127.0.0.1",no-pty %s\n' "$(cat "$WORK/restreinte.pub")"
    cat "$WORK/cert-cert.pub"
    echo "ssh-ed25519 AAAA tronquée"
  } > "$KEYS"
  chown atlas:atlas "$KEYS"
  chmod 600 "$KEYS"
}

seed_files() {
  if [ -d "$DIR" ]; then chmod -R u+w "$DIR"; fi
  rm -rf "$DIR"
  as_atlas mkdir -p "$DIR"
  cd "$DIR"
  as_atlas bash -c '
    printf "NODE_ENV=production\r\nPORT=8080\r\nLOG_LEVEL=info\r\n" > crlf.env
    printf "\xef\xbb\xbfnom=été\n" > bom.txt
    printf "a\r\nb\nc\r\n" > mixte.txt
    { head -c 10000 /dev/zero | tr "\0" a; printf "\n\xc3\x28 fin\n"; } > utf8-invalide.txt
    printf "SQLite format 3\0" > binaire.db
    head -c 1200000 /dev/zero | tr "\0" x > gros.log
    ssh-keygen -q -t ed25519 -N "" -C jetable -f cle.pem && rm cle.pem.pub
    ln -s crlf.env lien.env
    printf "version: 1\n" > lecture-seule.txt && chmod 444 lecture-seule.txt
    mkdir verrouille && printf "A=1\n" > verrouille/reglages.env && chmod 555 verrouille
    printf "A=1\nB=2\nC=3\n" > droits.conf && chmod 640 droits.conf
    printf "A=1\nB=2\nC=3\nD=4\nE=5\n" > conflit.env
    printf "A=1\nB=2\nC=3\nD=4\nE=5\nF=6\nG=7\n" > diff.env
    printf "un\ndeux\ntrois\nquatre\ncinq\n" > notes.txt
  '
  printf 'owner=autre\n' > autre.conf
  chown autre:autre autre.conf
  chmod 644 autre.conf
}

case "${1:-all}" in
  keys) seed_keys ;;
  *) seed_files; seed_keys ;;
esac
