# Roadmap

## Préparation

- [x] Cadrer le périmètre du MVP
- [x] Préparer le VPS
- [x] Découper les modules Rust
- [x] Écrire le contrat des commandes Tauri
- [x] Structurer le frontend Vue (stores, routes, composants, approche UI)
- [x] Maquetter les écrans : connexion, tableau de bord des services, logs, explorateur, scripts

## Initialisation

- [x] Créer le projet avec `create-tauri-app` (Tauri 2, Vue, TypeScript)
- [x] Configurer ESLint et Prettier côté Vue, rustfmt et clippy côté Rust
- [x] Mettre en place `ts-rs`
- [x] Installer Reka UI et @tanstack/vue-virtual
- [x] CI GitHub Actions : build Windows et Linux via `tauri-action`
- [x] Lancer le squelette sur les deux machines

## Jalons de développement

Chaque jalon doit produire quelque chose d'utilisable.

- [x] Jalon 1 : connexion SSH avec vérification de la clé hôte et passphrase dans le keyring
- [x] Jalon 2 : lister les services Docker et systemd (lecture seule)
- [x] Jalon 3 : piloter les services (start/stop/restart avec confirmation) et logs en direct
- [x] Jalon 4 : explorateur SFTP
- [ ] Jalon 5 : scripts personnalisés
- [ ] Jalon 6 : créer et supprimer des services systemd
- [ ] Jalon 7 : packaging et utilisation quotidienne sur les deux machines

## v2

- Gestion `docker compose` (up/down sur un dossier de projet)
- Activer / désactiver un service au démarrage
- Filtre de recherche dans les listes
- Détection des conteneurs tués par manque de mémoire (`OOMKilled`)
- Édition de fichiers texte
- Terminal intégré
- Monitoring
- Détection immédiate d'une perte de liaison (événement émis par le backend au lieu du polling de 30 s)
- Glisser/Déposer de dossiers entier dans le gestionnaire