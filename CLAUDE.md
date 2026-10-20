# Appli de contrôle VPS

Application desktop personnelle pour gérer un VPS Linux via SSH : explorateur de fichiers (SFTP), conteneurs Docker, services systemd utilisateur et scripts personnalisés.

Cibles : Windows et Linux (le développeur utilise les deux au quotidien).

## Stack

- Tauri 2 (application desktop native, pas de serveur, pas d'hébergement)
- Frontend : Vue 3 + TypeScript + Pinia + Vue Router, design maison en CSS (tokens en variables CSS), Reka UI (headless) pour modale et menus, @tanstack/vue-virtual pour la console de logs
- Backend : Rust, SSH via `russh`, SFTP via `russh-sftp`, secrets via `keyring`, erreurs via `thiserror`, types TS générés via `ts-rs`

## Documentation du projet

Lire le document concerné avant de travailler sur une partie :

- `docs/01-perimetre-mvp.md` : ce qui est dans le MVP et ce qui n'y est pas
- `docs/02-preparation-vps.md` : configuration du VPS et contraintes côté serveur
- `docs/03-architecture.md` : découpage des modules Rust et responsabilités
- `docs/04-contrat-commandes.md` : contrat complet des commandes Tauri, types et erreurs
- `docs/05-roadmap.md` : jalons de développement et état d'avancement

## Règles impératives

### Sécurité

- Ne jamais désactiver ni contourner la vérification de la clé hôte SSH. Ne jamais écrire un `check_server_key` qui renvoie `true` sans comparer l'empreinte.
- Toute valeur venant de l'interface qui finit dans une commande shell doit être validée strictement puis échappée. Pas d'exception.
  - Noms de services systemd existants (affichage, actions) : `^[a-zA-Z0-9@._-]+\.service$`
  - Noms de services créés depuis l'appli (formulaire de création) : `^[a-z0-9][a-z0-9-]*\.service$`
  - IDs de conteneurs Docker : hexadécimal uniquement. On manipule les conteneurs par ID, jamais par nom.
- Les opérations sur les fichiers passent par SFTP, jamais par des commandes shell (`rm`, `mv`, etc.).
- Les secrets (passphrase de clé) vont dans le keyring système via la crate `keyring`, jamais dans un fichier.
- Aucune commande ne doit utiliser `sudo`. Tout fonctionne avec les droits du compte SSH (voir `docs/02-preparation-vps.md`).

### Architecture

- La couche `commands/` reste très fine : récupérer l'état, appeler un module métier, renvoyer le résultat.
- Les modules métier (`ssh`, `sftp`, `docker`, `systemd`, `scripts`, `config`, `secrets`) ne dépendent pas de Tauri. Ils doivent être testables avec `cargo test` sans lancer l'application ni se connecter au VPS.
- Le parsing des sorties (JSON de Docker et systemd) est isolé dans des fonctions pures, couvertes par des tests unitaires avec des sorties d'exemple en dur.
- Une seule connexion SSH partagée ; chaque opération ouvre un canal sur cette connexion.
- Tout flux (logs, exécution de script) est enregistré dans l'état partagé avec un identifiant et doit pouvoir être arrêté proprement.
- Un seul type d'erreur `AppError`, sérialisé avec un champ `kind` (voir `docs/04-contrat-commandes.md`).
- Toute structure échangée avec le frontend dérive `ts-rs` ; ne jamais écrire à la main un type TypeScript qui duplique une structure Rust.

### Frontend

- Design entièrement maison : ne jamais ajouter de librairie de composants stylés (PrimeVue, Vuetify, Naive UI, shadcn-vue…) ni Tailwind. Reka UI est utilisé uniquement pour ses comportements, sans ses éventuels styles.
- Toutes les valeurs visuelles (couleurs, espacements, rayons, tailles de texte) viennent des tokens de `src/styles/tokens.css`. Pas de valeur en dur dans les composants.
- Aucun appel à `invoke()` en dehors du dossier `src/api/`.
- Tout flux de logs ou de script passe par une session `createLogSession` détenue par le store `telemetry`. Le panneau ancré du tableau de bord arrête son flux quand on quitte le tableau de bord ; un onglet de l'écran Télémétrie vit jusqu'à sa fermeture, la déconnexion ou la fermeture de l'application (un script en cours n'est jamais interrompu par un changement d'écran).
- Arguments de commandes : `snake_case` côté Rust, `camelCase` côté `invoke()` (conversion par défaut de Tauri).
- Les actions destructrices (suppression de fichier, de conteneur, de service, script marqué `requireConfirmation`) passent toujours par une confirmation explicite.
- Tester régulièrement le rendu sur Windows (WebView2) et Linux (WebKitGTK).

### Versions et changelog

- Versionnage sémantique, règles en tête de `CHANGELOG.md` : correctif pour des corrections seules, mineur pour une nouvelle fonction, majeur pour un changement incompatible pour l'utilisateur.
- Tout changement visible par l'utilisateur (fonction, correction, comportement, sécurité, installateurs) ajoute une ligne sous `## [Non publié]` de `CHANGELOG.md`, **dans le même commit**, dans la bonne catégorie (Ajouté, Modifié, Corrigé, Retiré, Sécurité). Une ligne décrit l'effet pour l'utilisateur, pas le code. Pas d'entrée pour la documentation, les tests, la CI ou une réorganisation interne sans effet visible.
- La version est identique dans `package.json`, `src-tauri/Cargo.toml` et `src-tauri/tauri.conf.json` (vérifié par `src-tauri/tests/version.rs` et par le workflow de publication).
- Préparer une version uniquement sur demande explicite : choisir le numéro d'après les entrées de « Non publié », renommer cette section en `## [X.Y.Z] - AAAA-MM-JJ` et rouvrir une section « Non publié » vide, mettre à jour les liens de comparaison en bas du fichier, changer la version dans les trois fichiers (et les fichiers de verrouillage), en un commit. Ne jamais créer de tag ni pousser : la publication (tag `vX.Y.Z`, release en brouillon publiée à la main) reste à l'utilisateur.

### Autres
- Le code doit être auto commenté. Limiter le nombre de commentaire
- Mettre un commentaire sur une longue fonctionnalité ou non explicite par son code

## Périmètre

Ne rien ajouter qui soit listé « hors MVP » dans `docs/01-perimetre-mvp.md` sans demande explicite.