//! Services systemd utilisateur (`systemctl --user`, `journalctl --user`) :
//! liste en JSON (parsing dans des fonctions pures), actions, flux de logs,
//! création et suppression des fichiers d'unité dans `~/.config/systemd/user/`
//! suivies de `daemon-reload`.
//! Noms de services validés par `^[a-zA-Z0-9@._-]+\.service$`.
