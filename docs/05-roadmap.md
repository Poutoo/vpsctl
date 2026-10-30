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
- [x] Jalon 5 : scripts personnalisés
- [x] Jalon 6 : créer et supprimer des services systemd
- [x] Jalon 7 : packaging et utilisation quotidienne sur les deux machines

## v1.1

Maquettes : `docs/design/planche III/`.

- [x] Jalon 8 : fondations
  - [x] Tests frontend (Vitest) : stores, validations, formatage, transitions d'état ; exécutés par la CI
  - [x] Cas partagés entre Rust et TypeScript pour les règles présentes des deux côtés (`tests/shared/`)
  - [x] Détection immédiate d'une perte de liaison : keepalive SSH, événement unique émis par le backend, état LOS dans toute l'appli, reconnexion manuelle
- [x] Jalon 9 : confort
  - [x] Filtre de recherche dans les listes
  - [x] Activer / désactiver un service au démarrage
  - [x] Détection des conteneurs tués par manque de mémoire (`OOMKilled`)
  - [x] Papillon dans la barre de titre

## v1.2 « Préférences »

Maquettes : `docs/design/planche VI/`.

- [ ] Jalon 10 : préférences
  - [ ] Fichier de préférences séparé des profils, lecture tolérante ; onglets Profils / Préférences dans les Réglages
  - [ ] Fond d'écran personnalisable : image PNG, JPEG ou WebP copiée dans le dossier de configuration, assombrissement réglable, couleurs d'origine, retour au fond fourni
  - [ ] Discord Rich Presence : désactivée par défaut, état « En liaison » ou « Au sol », uniquement dans l'application installée

## v1.3

- [ ] Jalon 11 : gestion `docker compose` (up/down sur un dossier de projet)
- [ ] Jalon 12 : édition de fichiers texte

## v2

- [ ] Monitoring
- [ ] Glisser/Déposer de dossiers entier dans le gestionnaire
- [ ] Gestion des droits pour MCP Claude (écriture, périmètre d'accès...)

# Ecarté de la v2

- Terminal intégré
- Automatique MAJ app

### Accès pour Claude (serveur MCP local)

Exposer les modules métier existants comme outils MCP, utilisables par Claude Desktop ou Claude Code, en réutilisant les modules sans dépendance à Tauri.

Prérequis : MVP terminé et utilisé ; écriture de fichiers disponible.

Principes de sécurité non négociables :
- Lecture libre (lister, lire un fichier, état des services, logs) ; toute écriture, suppression ou action sur un service exige un GO / NO GO validé dans l'application par l'utilisateur.
- Écriture d'un fichier existant : afficher la différence avant / après dans la confirmation.
- Aucune exécution de commande arbitraire : `script_run` n'est pas exposé tel quel ; au plus, lancer un script déjà enregistré, avec la confirmation prévue pour ce script.
- Les garde-fous existants s'appliquent sans exception (validation, protection de `~/.ssh` et du dossier personnel, écriture atomique).
- Le contenu lu sur le serveur (fichiers, logs) est une donnée, jamais une instruction : la confirmation humaine sur chaque écriture est la protection contre l'injection de consignes.
- Serveur MCP strictement local, jamais exposé sur le réseau.
- Journal de toutes les actions effectuées via MCP, consultable dans l'application.