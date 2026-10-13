# Configuration du VPS

Statut : fait.

## Environnement

- VPS Hetzner, Ubuntu 24.04
- Les services tournent sous Docker (projets docker compose) et en services systemd **utilisateur**

## Compte utilisé par l'application

L'application se connecte avec le **compte propriétaire des services utilisateur**. Il n'y a pas d'utilisateur dédié ni de règle sudoers.

Raisons :

- Les services utilisateur ne se pilotent que par leur propriétaire, sans droits root.
- Le compte doit appartenir au groupe `docker`, ce qui équivaut en pratique à un accès root. Un utilisateur séparé n'apportait donc pas de vraie isolation.
- Ce compte possède déjà les fichiers de ses services, ce qui évite les problèmes de droits dans l'explorateur SFTP.

Conséquence : la sécurité de l'ensemble repose sur les clés SSH. Les clés privées sont protégées par passphrase.

## Configuration en place

- Authentification par clé SSH ed25519, une clé distincte par machine cliente (Windows, laptop Linux), révocables individuellement
- Compte membre du groupe `docker`
- Lingering activé (`loginctl enable-linger`) : les services utilisateur tournent sans session ouverte
- Commandes testées en SSH non interactif (comme l'appli les enverra) :
  - `systemctl --user list-units --type=service --all`
  - `journalctl --user -u <service> -n 20`
  - `docker ps`
- Empreinte de la clé hôte ed25519 notée (obtenue via `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`), à comparer lors de la première connexion de l'appli

## Sortie JSON de systemd

`systemctl --user list-units --output=json` produit une sortie JSON exploitable directement, avec les champs `unit`, `load`, `active`, `sub`, `description`.

## Contraintes pour le code

- Aucune commande ne doit utiliser `sudo`.
- Toujours utiliser `systemctl --user` et `journalctl --user`.
- Docker : conteneurs identifiés par leur ID.