# Architecture

Statut : backend défini, frontend à définir.

## Vue d'ensemble

```
Frontend Vue
   │  invoke() et Channel
commands/            adaptateurs Tauri, très fins
   │
Modules métier       sftp, docker, systemd, scripts (sans dépendance à Tauri)
   │
ssh                  connexion unique, canaux multiplexés
secrets, config      keyring, profils et scripts en JSON local
   │
VPS (SSH, port 22)
```

## Arborescence Rust

```
src-tauri/src/
├── lib.rs            point d'entrée Tauri 2 : enregistrement des commandes et de l'état
├── error.rs          enum AppError unique (thiserror), sérialisable vers le frontend
├── state.rs          AppState : connexion active + registre des flux en cours
├── ssh/
│   ├── mod.rs        connect, exec, open_stream
│   └── host_key.rs   vérification de l'empreinte de la clé hôte
├── sftp.rs
├── docker.rs
├── systemd.rs
├── scripts.rs
├── config.rs
├── secrets.rs
└── commands/
    ├── connection.rs
    ├── files.rs
    ├── docker.rs
    ├── systemd.rs
    └── scripts.rs
```

## Responsabilités

**ssh** : ouvre et maintient une seule connexion SSH. Les autres modules ouvrent des canaux dessus. Expose deux primitives : exécuter une commande et récupérer sa sortie, ouvrir un canal en streaming. Implémente la vérification de la clé hôte.

**sftp** : ouvre le sous-système SFTP sur un canal (`russh-sftp`) et réalise toutes les opérations sur les fichiers. Aucune commande shell.

**docker** : exécute `docker ps -a --format '{{json .}}'` et désérialise chaque ligne avec `serde`. Gère start, stop, restart, rm et le flux `docker logs -f`.

**systemd** : `systemctl --user` en sortie JSON, actions sur les services, flux `journalctl --user -f`. Génère et supprime les fichiers d'unité dans `~/.config/systemd/user/`, suivis de `systemctl --user daemon-reload`.

**scripts** : charge les scripts depuis `config` et les exécute via `ssh` avec sortie en streaming.

**config** : lit et écrit les profils de connexion et les scripts en JSON dans le dossier de configuration de l'application.

**secrets** : surcouche de la crate `keyring` pour la passphrase de la clé SSH (Credential Manager sous Windows, Secret Service sous Linux).

**commands** : fonctions `#[tauri::command]` sans logique propre. Streaming via `tauri::ipc::Channel`.

## Points critiques

### Injection de commandes

Toute valeur saisie dans l'interface et insérée dans une commande shell est validée par une expression stricte, puis échappée. Exemple de risque : un service nommé `bot; rm -rf ~` exécuterait les deux commandes.

### Arrêt des flux

Chaque flux (logs, exécution de script) reçoit un identifiant et est enregistré dans `state.rs`. Une commande d'arrêt ferme le canal correspondant, ce qui termine le processus distant. Sans cela, les canaux s'accumulent.

### Vérification de la clé hôte en deux temps

1. Première connexion : l'empreinte est inconnue, `connect` renvoie `UnknownHostKey` avec l'empreinte reçue.
2. L'interface l'affiche, l'utilisateur la compare à celle notée sur le VPS.
3. Après validation, `trust_host_key` l'enregistre dans le profil.
4. Connexions suivantes : toute différence renvoie `HostKeyMismatch` et bloque la connexion.

### Fichiers d'unité systemd

Les champs saisis dans le formulaire de création (`execStart`, `description`, etc.) sont écrits sans retour à la ligne, pour empêcher l'insertion de directives supplémentaires dans le fichier d'unité.

## Frontend

À définir : stores Pinia, routes, organisation des composants, librairie de composants ou design maison.