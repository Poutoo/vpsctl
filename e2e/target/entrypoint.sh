#!/bin/sh
set -eu

# Docker dans Docker avec cgroup v2 : les processus existants passent dans un
# sous-groupe, sinon les contrôleurs (mémoire pour mem_limit) ne peuvent pas
# être délégués aux conteneurs imbriqués. Même manœuvre que l'image docker:dind.
if [ -f /sys/fs/cgroup/cgroup.controllers ]; then
  mkdir -p /sys/fs/cgroup/init
  xargs -rn1 < /sys/fs/cgroup/cgroup.procs > /sys/fs/cgroup/init/cgroup.procs || :
  sed -e 's/ / +/g' -e 's/^/+/' < /sys/fs/cgroup/cgroup.controllers \
    > /sys/fs/cgroup/cgroup.subtree_control
fi

install -d -m 0755 /run/sshd
install -d -o atlas -g atlas -m 0700 /home/atlas/.ssh
install -o atlas -g atlas -m 0600 /e2e/authorized_keys /home/atlas/.ssh/authorized_keys
install -d -o atlas -g atlas -m 0755 /home/atlas/atlas-tests

dockerd > /var/log/dockerd.log 2>&1 &

exec /usr/sbin/sshd -D -e
