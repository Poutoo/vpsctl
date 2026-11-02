# Cible des tests E2E : un « faux VPS » isolé, aussi proche que possible du
# vrai (docs/02-preparation-vps.md) : Ubuntu 26.04, Docker 29.6.1, plugin
# compose 5.3.1, compte non root membre du groupe docker, clé SSH uniquement.
# Docker tourne dans le conteneur (privilégié) : les projets de test ne
# touchent jamais au Docker du poste.
FROM ubuntu:26.04

ARG DOCKER_VERSION=5:29.6.1-1~ubuntu.26.04~resolute
ARG COMPOSE_VERSION=5.3.1-1~ubuntu.26.04~resolute

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl openssh-server iptables procps \
 && install -m 0755 -d /etc/apt/keyrings \
 && curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc \
 && echo "deb [arch=amd64 signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu resolute stable" \
      > /etc/apt/sources.list.d/docker.list \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      docker-ce="$DOCKER_VERSION" docker-ce-cli="$DOCKER_VERSION" containerd.io \
      docker-compose-plugin="$COMPOSE_VERSION" \
 && rm -rf /var/lib/apt/lists/*

# Une seule clé hôte (ed25519) : l'empreinte attendue par les tests est lue
# dans le conteneur, comme on la lit sur la console de l'hébergeur.
RUN rm -f /etc/ssh/ssh_host_* \
 && ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key

# Compte de l'application : pas de mot de passe utilisable, pas de sudo.
RUN useradd -m -s /bin/bash -G docker atlas \
 && usermod -p '*' atlas

COPY sshd_config /etc/ssh/sshd_config
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 0755 /usr/local/bin/entrypoint.sh

EXPOSE 22
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
