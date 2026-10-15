# Architecture

Statut : backend et frontend définis.

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

**sftp** : ouvre le sous-système SFTP sur un canal (`russh-sftp`) et réalise toutes les opérations sur les fichiers. Aucune commande shell. Une seule session SFTP est gardée ouverte sur la liaison (dans `Connection`) et recréée uniquement si elle s'est fermée ; toutes les opérations la partagent (lecture du dossier des unités systemd, explorateur).

**docker** : exécute `docker ps -a --no-trunc --format '{{json .}}'` et désérialise chaque ligne avec `serde`. Gère start, stop, restart, rm et le flux `docker logs -f`.

- `--no-trunc` donne l'ID complet (64 caractères hexadécimaux), utilisé pour les actions. Il a deux effets de bord, corrigés par des fonctions pures : `Names` contient aussi les anciens alias `--link` (`autre/alias`, séparés par des virgules) et on garde le premier nom sans `/` ; `Image` peut contenir un digest (`nginx@sha256:…`), retiré, ou être un identifiant d'image sans nom (`sha256:<64 hex>`), raccourci à 12 caractères.
- Le code de sortie est extrait du champ `Status` (`Exited (137) 4 days ago`, `Restarting (1) 5 seconds ago`).
- La durée est extraite du champ `Status` et traduite en français : « depuis 3 heures » pour un conteneur en cours, « il y a 4 jours » pour un conteneur sorti ou en redémarrage. Les formes possibles sont celles de `HumanDuration` (`docker/go-units`) : `Less than a second`, `1 second`, `N seconds`, `About a minute`, `N minutes`, `About an hour`, `N hours`, `N days`, `N weeks`, `N months`, `N years`. Une forme non reconnue est affichée telle quelle.
- Erreurs : commande introuvable (code 127) → `DockerNotInstalled` ; accès au socket refusé (utilisateur hors du groupe `docker`) → `DockerPermissionDenied` ; tout autre échec → `Remote`.

Correspondance des états Docker vers les états de l'interface (calculée dans le backend) :

| État Docker | État affiché |
|---|---|
| `running` | Nominal |
| `exited` avec code 0, 137 ou 143 | Arrêté (code affiché) |
| `exited` avec tout autre code, ou code illisible | En échec (code affiché) |
| `created`, `paused` | Arrêté |
| `restarting`, `removing` | En transition |
| `dead` | En échec |
| tout autre état | En échec (état brut en infobulle) |

Limite connue : un conteneur tué par manque de mémoire sort aussi avec le code 137 et apparaîtra « Arrêté ». Le détecter demanderait `docker inspect` (champ `OOMKilled`) : envisageable en v2.

**systemd** : `systemctl --user` en sortie JSON, actions sur les services, flux `journalctl --user -f`. Génère et supprime les fichiers d'unité dans `~/.config/systemd/user/`, suivis de `systemctl --user daemon-reload`.

Liste des services : `systemctl --user list-units --type=service --all --output=json --no-pager` (unités en mémoire) et lecture par SFTP du dossier `~/.config/systemd/user/` (fichiers d'unité), en parallèle. La sortie JSON ne donne pas le chemin du fichier d'unité et ne contient pas les unités que systemd n'a pas chargées : la lecture du dossier est donc nécessaire.

Règle de filtrage (fonction pure) :

- Seuls les services dont le fichier d'unité est dans `~/.config/systemd/user/` sont affichés. Les services fournis par le système (`dbus.service`, etc.) sont exclus.
- Un fichier `x.service` du dossier retient l'unité `x.service`. S'il n'est pas en mémoire (jamais démarré, désactivé), le service est tout de même affiché : « Arrêté », sans description.
- Un gabarit `bot@.service` du dossier retient ses instances en mémoire (`bot@1.service`). Le gabarit lui-même n'est pas affiché (il ne peut pas être démarré). Limite : une instance que systemd n'a pas en mémoire (jamais démarrée) n'apparaît pas.
- Une unité dont le chargement a échoué (`not-found`, `bad-setting`, `error`…) est exclue, sauf si son fichier est dans le dossier : elle est alors affichée « En échec ».
- Une unité masquée (lien vers `/dev/null` dans le dossier) est affichée « Arrêté » avec la mention « masqué », et n'est pas pilotable.
- Les sous-dossiers (`default.target.wants/`, `x.service.d/`) sont ignorés. Si le dossier n'existe pas, la liste est vide.

Correspondance des états systemd vers les états de l'interface (champ `active`, calculée dans le backend) :

| État systemd | État affiché |
|---|---|
| `active` (quel que soit `sub` : `running`, `exited`…) | Nominal |
| `inactive` | Arrêté |
| `failed` | En échec |
| `activating`, `deactivating`, `reloading`, `refreshing`, `maintenance` | En transition |
| chargement échoué (fichier dans le dossier) | En échec |
| tout autre état | En échec (`active`/`sub` bruts en infobulle) |

Un service dont le nom ne respecte pas `^[a-zA-Z0-9@._-]+\.service$` (ex. nom échappé `a\x2db.service`) est affiché mais marqué non pilotable : il ne reçoit aucune action.

**scripts** : charge les scripts depuis `config` et les exécute via `ssh` avec sortie en streaming.

**config** : lit et écrit les profils de connexion et les scripts en JSON dans le dossier de configuration de l'application.

**secrets** : surcouche de la crate `keyring` pour la passphrase de la clé SSH (Credential Manager sous Windows, Secret Service sous Linux).

**commands** : fonctions `#[tauri::command]` sans logique propre. Streaming via `tauri::ipc::Channel`.

## Points critiques

### Injection de commandes

Toute valeur saisie dans l'interface et insérée dans une commande shell est validée par une expression stricte, puis échappée. Exemple de risque : un service nommé `bot; rm -rf ~` exécuterait les deux commandes.

### Origine des lignes de logs

- `docker logs` et les scripts : SSH transmet séparément la sortie standard et la sortie d'erreur, on conserve donc l'origine `stdout` / `stderr`. Exception : un conteneur lancé avec un TTY mélange les deux flux.
- `journalctl` : le journal ne conserve pas, par défaut, de quel flux vient une ligne. On récupère à la place la priorité syslog de chaque message (`journalctl --user -o json`, champ `PRIORITY`). L'interface affiche `ERR` pour une priorité inférieure ou égale à 3 (err, crit, alert, emerg).
- Horodatages : `docker logs --timestamps` et le champ `__REALTIME_TIMESTAMP` du JSON de `journalctl`.

### Vérification après une action sur un service

Le code de retour d'une commande ne suffit pas à savoir si un service a démarré. Pour un service systemd de type `simple` (le type par défaut), `systemctl --user start` réussit dès que le processus est lancé, même s'il plante juste après. `docker start` se comporte de la même façon.

Après un démarrage ou un redémarrage :
1. Le service passe en état « En transition ».
2. Son état est relu après un court délai (2 à 3 secondes).
3. Seulement alors, l'état réel est affiché, avec une notification d'échec si besoin.

### Arrêt des flux

Chaque flux (logs, exécution de script) reçoit un identifiant et est enregistré dans `state.rs`. Une commande d'arrêt ferme le canal correspondant, ce qui termine le processus distant. Sans cela, les canaux s'accumulent.

### Vérification de la clé hôte en deux temps

1. Première connexion : l'empreinte est inconnue, `connect` renvoie `UnknownHostKey` avec l'empreinte reçue.
2. L'interface l'affiche, l'utilisateur la compare à celle notée sur le VPS.
3. Après validation, `trust_host_key` l'enregistre dans le profil.
4. Connexions suivantes : toute différence renvoie `HostKeyMismatch` et bloque la connexion.

### Fichiers d'unité systemd

Gabarit généré :

```ini
[Unit]
Description=<description>

[Service]
Type=simple
WorkingDirectory=<dossier>
ExecStart=<commande>
Restart=<no|on-failure|always>
RestartSec=5
Environment="CLE=valeur"

[Install]
WantedBy=default.target
```

- Pas de `After=network-online.target` : cette cible appartient au gestionnaire système, elle n'a pas d'effet dans un service utilisateur.
- Dossier de travail : `~` seul est accepté tel quel ; `~/chemin` est converti en `%h/chemin` (systemd n'accepte pas `~/` dans `WorkingDirectory`).
- Chaque variable d'environnement est écrite entre guillemets, avec les guillemets et barres obliques inverses échappés.

Les champs saisis dans le formulaire de création (`execStart`, `description`, etc.) sont écrits sans retour à la ligne, pour empêcher l'insertion de directives supplémentaires dans le fichier d'unité.

## Frontend

### Approche visuelle : design maison, comportements empruntés

- Le design est entièrement maison : aucune librairie de composants stylés.
- Les comportements complexes viennent de librairies **headless** (sans style) :
  - **Reka UI** (anciennement Radix Vue) : modale de confirmation, menus déroulants, menu contextuel (focus, clavier, positionnement).
  - **@tanstack/vue-virtual** : virtualisation de la console de logs.
- Les composants simples (boutons, champs, badges, tableaux, notifications, barre de progression) sont codés entièrement à la main.
- Style : variables CSS comme design tokens (couleurs, espacements, rayons, typographie), thème clair et sombre, CSS scoped dans chaque composant. Pas de Tailwind.

### Arborescence

```
src/
├── bindings/          types générés par ts-rs (jamais modifiés à la main)
├── api/               une fonction typée par commande Tauri, regroupées par domaine
├── stores/            Pinia : connection, profiles, files, services, scripts, ui
├── composables/       useLogStream, useConfirm, useAsyncAction
├── router/            routes + garde « connexion requise »
├── styles/            tokens.css, base.css
├── components/
│   ├── ui/            design system (Button, Modal, Console, Toast…)
│   └── features/      FileBrowser, ServiceList, ServiceForm, ScriptList…
└── views/             ConnectView, DashboardView, FilesView, ScriptsView, SettingsView
```

### Règles

- **Aucun `invoke()` en dehors de `api/`.** Stores et composants appellent des fonctions typées (`docker.list()`, etc.).
- **`useLogStream(source)`** gère tout le cycle de vie d'un flux : démarrage, mémoire tampon limitée (5 000 lignes par défaut, les plus anciennes sont supprimées), appel automatique de `logs_stop` au démontage du composant. Aucun flux ne doit rester ouvert après un changement d'écran. Utilisé aussi pour l'exécution des scripts.
- **Transferts** : file d'attente gérée dans le store `transfers`, un seul transfert actif à la fois. Chaque transfert a un identifiant et peut être annulé. « Afficher » ouvre le dossier local du fichier téléchargé via le plugin officiel opener de Tauri.
- **Rafraîchissement des services** : toutes les 5 secondes uniquement quand le tableau de bord est affiché et la fenêtre visible (`document.visibilityState`), plus un bouton d'actualisation manuelle. Le rafraîchissement suivant est programmé 5 secondes après la fin du précédent, et un rafraîchissement manuel pendant qu'un autre est en cours réutilise celui-ci : deux rafraîchissements ne se chevauchent jamais. Docker et systemd sont appelés indépendamment : l'échec de l'un n'empêche pas l'affichage de l'autre.
- **Sortie d'erreur ≠ erreur** : beaucoup de programmes écrivent tous leurs logs sur la sortie d'erreur (Caddy, le module `logging` de Python par défaut). Une ligne `stderr` est donc marquée de façon neutre (étiquette discrète, sans fond rouge). Le style d'erreur rouge est réservé aux messages systemd de priorité ≤ 3.
- **Console** : un seul composant console, utilisé à l'identique dans la vue plein écran, le panneau ancré du tableau de bord et l'exécution des scripts (défilement automatique, indicateur de nouvelles lignes, bannière LOS). Le panneau ancré réduit la zone du tableau au-dessus de lui, il ne la recouvre pas.
- **Barre de titre personnalisée** : la barre native est désactivée (`decorations: false`). La zone de déplacement utilise `data-tauri-drag-region`, les boutons réduire, agrandir et fermer appellent l'API de fenêtre de Tauri, avec les permissions correspondantes dans les capabilities. Le double-clic sur la barre agrandit la fenêtre. Limite connue : sous Windows, le menu d'ancrage (Snap Layouts) au survol du bouton agrandir n'est pas disponible ; sous Linux, le comportement est à tester selon le gestionnaire de fenêtres.
- **Barre d'état** : affiche `server_info` (système, Docker, systemd), récupéré une fois après la connexion.
- **Erreurs** : chaque `kind` de `AppError` est traduit en message clair. `HostKeyMismatch` redirige vers un écran d'alerte dédié, jamais une simple notification.

### Routes

| Route | Vue | Connexion requise |
|---|---|---|
| `/connect` | ConnectView | non |
| `/dashboard` | DashboardView (Docker + systemd) | oui |
| `/files/:path*` | FilesView | oui |
| `/scripts` | ScriptsView | oui |
| `/settings` | SettingsView (profils) | oui |
