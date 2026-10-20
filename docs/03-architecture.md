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
├── state.rs          AppState : connexion active + registres des flux et des transferts en cours
├── run_state.rs      RunState : état affiché d'un conteneur ou d'un service
├── ssh/
│   ├── mod.rs        connect, exec, open_stream
│   └── host_key.rs   vérification de l'empreinte de la clé hôte
├── sftp/
│   ├── mod.rs        session partagée, reprise après erreur de transport, classement des erreurs
│   ├── path.rs       fonctions pures : chemins, noms, droits, temporaires, résidus, accès SSH
│   ├── fs.rs         trait RemoteFs et son implémentation SFTP
│   ├── ops.rs        lister, créer, renommer, supprimer
│   ├── walk.rs       parcours d'une arborescence sans suivre les liens : décompte et suppression
│   └── transfer.rs   envoi et téléchargement par blocs, temporaires, annulation
├── stream.rs         flux de lignes : LogEvent, découpage, lots, boucle de lecture, arrêt
├── docker.rs
├── systemd/
│   ├── mod.rs        liste, filtrage, états, actions, logs
│   ├── unit.rs       fonctions pures : validation de ServiceDef et texte du fichier d'unité
│   └── manage.rs     séquences de création et de suppression d'un service
├── scripts.rs
├── config.rs
├── secrets.rs
└── commands/
    ├── connection.rs
    ├── files.rs
    ├── docker.rs
    ├── systemd.rs
    ├── logs.rs
    └── scripts.rs
```

## Responsabilités

**ssh** : ouvre et maintient une seule connexion SSH. Les autres modules ouvrent des canaux dessus. Expose deux primitives : exécuter une commande et récupérer sa sortie, ouvrir un canal en streaming. Implémente la vérification de la clé hôte. Une commande a une échéance : au-delà, elle est interrompue (SIGTERM si le serveur accepte les signaux) et son canal est fermé, pour qu'aucun canal abandonné ne s'accumule.

**stream** : flux de lignes, partagé par les logs et les scripts. Types `LogEvent` et `LogLine`, découpage des données reçues en lignes (une mémoire par flux de sortie, une ligne pouvant être coupée entre deux paquets SSH), retrait des séquences d'échappement ANSI du texte de chaque ligne (fonction pure `strip_ansi`, appliquée après l'analyse propre à la source), regroupement par lots, limite de débit, boucle de lecture d'un canal et séquence d'arrêt. Construit le script de surveillance qui enveloppe la commande (voir « Arrêt des flux »). Le registre des flux en cours est dans `state.rs`. Sans dépendance à Tauri : les événements partent vers une fonction de rappel, que la couche `commands` relie au `Channel`.

**sftp** : ouvre le sous-système SFTP sur un canal (`russh-sftp`) et réalise toutes les opérations sur les fichiers. Aucune commande shell. Une seule session SFTP est gardée ouverte sur la liaison (dans `Connection`) et recréée uniquement si elle s'est fermée : après une erreur de transport, la session est recréée et l'opération retentée une seule fois avant de remonter l'erreur. Toutes les opérations la partagent (lecture du dossier des unités systemd, explorateur, transferts).

- Les opérations de l'explorateur et des transferts passent par le trait `RemoteFs` (`lstat`, `read_dir`, `rename`…), implémenté sur la session SFTP. Le parcours d'arborescence et les transferts sont ainsi testés sur un système de fichiers simulé en mémoire, sans réseau.
- Les fonctions pures de `path.rs` (validation des chemins et des noms, jointure, chaîne de droits `drwxr-xr-x`, noms des temporaires, reconnaissance des résidus, chemins critiques pour l'accès SSH) sont couvertes par des tests unitaires.
- Le parcours (`walk.rs`) lit toute l'arborescence avant de supprimer quoi que ce soit, n'entre jamais dans un lien symbolique (les attributs de `readdir` sont ceux de `lstat`), et limite les requêtes en parallèle à 16.
- Les transferts (`transfer.rs`) copient par blocs entre un `AsyncRead` et un `AsyncWrite` (le `File` de russh-sftp envoie déjà plusieurs requêtes à la fois), avec un temporaire renommé à la fin. Chaque transfert est enregistré dans `state.rs` avec son signal d'annulation, comme un flux.

**docker** : exécute `docker ps -a --no-trunc --format '{{json .}}'` et désérialise chaque ligne avec `serde`. Gère start, stop, restart, rm et le flux `docker logs -f`.

- `--no-trunc` donne l'ID complet (64 caractères hexadécimaux), utilisé pour les actions. Il a deux effets de bord, corrigés par des fonctions pures : `Names` contient aussi les anciens alias `--link` (`autre/alias`, séparés par des virgules) et on garde le premier nom sans `/` ; `Image` peut contenir un digest (`nginx@sha256:…`), retiré, ou être un identifiant d'image sans nom (`sha256:<64 hex>`), raccourci à 12 caractères.
- Une ligne illisible (JSON invalide, ID non hexadécimal) est exclue sans invalider les autres ; leur nombre est renvoyé (`skipped`) et affiché en avertissement dans la section Docker.
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
- Une unité masquée (`load` = `masked`, ou lien vers `/dev/null` dans le dossier, lu par SFTP) est affichée avec la mention « masqué » et n'est pas pilotable. Son état suit le champ `active` : « Arrêté » en pratique, sauf si elle a été masquée pendant qu'elle tournait.
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

Suppression possible (`Service.deleteBlock`, fonction pure) : uniquement pour un service dont le fichier d'unité est un fichier ordinaire du dossier utilisateur. Sinon, la raison est donnée, dans cet ordre de priorité :

| Cas | `deleteBlock` |
|---|---|
| Nom hors de `^[a-zA-Z0-9@._-]+\.service$` | `invalidName` |
| Unité masquée | `masked` |
| Instance d'un gabarit, sans fichier propre | `templateInstance` |
| Lien symbolique vers autre chose que `/dev/null` (`systemctl link`) : supprimer le lien ne supprimerait pas le vrai fichier | `linked` |

L'état d'exécution n'entre pas en compte : un service « en transition » (qui redémarre en boucle, par exemple) peut être supprimé.

Création et suppression (`manage.rs`) : séquences décrites dans `docs/04-contrat-commandes.md`. Elles sont génériques sur un petit trait qui fournit l'exécution d'une commande et un `RemoteFs`, pour tester l'ordre des étapes et l'arrêt au premier échec sans réseau (`MemFs` et une exécution simulée qui enregistre les commandes).

**scripts** : type `Script`, validation et règles d'enregistrement (fonctions pures), fichier `scripts.json` (écriture atomique fournie par `config`), construction de la commande d'exécution et enregistrement de `lastRun`. L'exécution passe par le registre des flux de `state.rs`, comme les logs.

- La commande est lancée dans le shell de connexion de l'utilisateur : `"${SHELL:-/bin/sh}" -l -c '<commande>'`, la commande échappée par `shell::quote` en un seul argument, le tout enveloppé dans le script de surveillance. Détail et limites (`~/.bashrc` non chargé) : voir `docs/04-contrat-commandes.md`.
- Chaque ligne est horodatée à sa réception par le backend.
- `stream::run` renvoie l'issue du flux ; à la fin de la tâche, une fonction enregistre `lastRun` (début de l'exécution, code de sortie ou `null`).

**config** : lit et écrit les profils de connexion en JSON dans le dossier de configuration de l'application, et fournit l'écriture atomique (temporaire puis renommage) utilisée aussi pour les scripts.

**secrets** : surcouche de la crate `keyring` pour la passphrase de la clé SSH (Credential Manager sous Windows, Secret Service sous Linux).

**commands** : fonctions `#[tauri::command]` sans logique propre. Streaming via `tauri::ipc::Channel`.

## Points critiques

### Injection de commandes

Toute valeur saisie dans l'interface et insérée dans une commande shell est validée par une expression stricte, puis échappée. Exemple de risque : un service nommé `bot; rm -rf ~` exécuterait les deux commandes.

### Origine des lignes de logs

- `docker logs` et les scripts : SSH transmet séparément la sortie standard et la sortie d'erreur, on conserve donc l'origine `stdout` / `stderr`. Exception : un conteneur lancé avec un TTY mélange les deux flux.
- `journalctl` : le journal ne conserve pas, par défaut, de quel flux vient une ligne. On récupère à la place la priorité syslog de chaque message (`journalctl --user -o json`, champ `PRIORITY`). L'interface affiche `ERR` pour une priorité inférieure ou égale à 3 (err, crit, alert, emerg).
- Horodatages : `docker logs --timestamps` et le champ `__REALTIME_TIMESTAMP` du JSON de `journalctl`. Tous deux sont normalisés en ISO 8601 UTC à la milliseconde (`2026-10-16T09:13:46.900Z`) : Docker en donne 9 décimales, que `Date.parse` ne garantit pas sous WebKitGTK.
- Champ `MESSAGE` de `journalctl -o json` : chaîne, tableau d'octets (message non UTF-8, décodé avec remplacement), `null` (champ trop long sans `--all` ; l'option est passée) ou absent (« [sans message] », affiché en texte discret). Un message sur plusieurs lignes donne plusieurs lignes de console.

### Vérification après une action sur un service

Le code de retour d'une commande ne suffit pas à savoir si un service a démarré. Pour un service systemd de type `simple` (le type par défaut), `systemctl --user start` réussit dès que le processus est lancé, même s'il plante juste après. `docker start` se comporte de la même façon.

Après un démarrage ou un redémarrage :
1. Le service passe en état « En transition ».
2. Son état est relu après un court délai (2 à 3 secondes).
3. Seulement alors, l'état réel est affiché, avec une notification d'échec si besoin.

Mise en œuvre (store `services`) :

- Pendant l'action, la ligne est marquée « en attente » dans le store. Elle s'affiche « En transition » avec le chrono de l'action, quel que soit l'état renvoyé par les rafraîchissements automatiques : seule la vérification lève cette marque.
- Relecture à 2,5 s après toute action systemd (`--no-block` rend la main avant le résultat) et après un démarrage ou redémarrage Docker. L'arrêt Docker est synchrone : relecture immédiate. La suppression retire la ligne à la relecture suivante.
- Démarrage ou redémarrage : « Nominal » à la relecture donne une notification de succès ; « Arrêté » ou « En échec » une notification d'erreur persistante avec « Voir la télémétrie », qui ouvre les logs du service. Encore « En transition » (service lent) : l'état réel est affiché sans verdict.
- L'animation de changement d'état n'est jouée qu'à cette relecture, jamais lors d'un rafraîchissement automatique.

### Arrêt des flux

Chaque flux (logs, exécution de script) reçoit un identifiant et est enregistré dans `state.rs` (4 au maximum). Il est arrêté par `logs_stop` ou `script_stop`, à la déconnexion, à la fermeture de l'application, au chargement d'une page (rechargement de l'interface : les `Channel` de l'ancienne page sont morts, sans que leur envoi échoue forcément), et de lui-même si un envoi vers son `Channel` échoue.

Fermer le canal ne suffit pas à terminer le processus distant. Dans OpenSSH (`session.c`), une session sans terminal n'est pas tuée à la fermeture de son canal : sshd ferme seulement ses tubes, et le processus ne meurt qu'à sa prochaine écriture (SIGPIPE). `docker logs -f` sur un conteneur silencieux ou `journalctl -f` sur un service calme survivraient indéfiniment. Un terminal (PTY) réglerait la question par SIGHUP, mais il fusionne stdout et stderr : exclu.

Deux mécanismes complémentaires :

1. **Arrêt demandé** : requête SSH `signal` TERM (`Channel::signal` de russh), puis fin d'émission et fermeture du canal, en 3 s au plus. sshd crée une nouvelle session pour la commande (`setsid()` dans `do_exec_no_pty`) et transmet le signal à tout son groupe de processus (`killpg`) : le shell et la commande le reçoivent.
2. **Filet de sécurité côté serveur**, pour un signal refusé, une liaison coupée ou une application tuée. La commande est enveloppée dans un script POSIX dont le texte est fixe (seuls les arguments validés et échappés y entrent) :

   ```sh
   exec 3<&0 </dev/null; <commande> 3<&- & p=$!; { while read -r l; do :; done; kill -TERM 0; } <&3 >/dev/null 2>&1 & w=$!; exec 3<&-; wait "$p"; s=$?; kill "$w" 2>/dev/null; exit "$s"
   ```

   - La commande tourne en arrière-plan, l'entrée standard sur `/dev/null`.
   - Le surveillant lit l'entrée standard du canal (gardée sur le descripteur 3, car une commande en arrière-plan reçoit `/dev/null`). Quand sshd ferme le canal, il ferme ce tube : la lecture se termine et `kill -TERM 0` arrête tout le groupe de processus de la session. `read` est dans une boucle : une ligne reçue sur l'entrée relance la lecture, seule la fin de l'entrée déclenche l'arrêt. La variable s'appelle `l` et non `_`, paramètre spécial de bash et de zsh.
   - `kill 0` vise le groupe de processus, pas un PID : ce groupe existe tant que le surveillant en fait partie, son identifiant ne peut donc pas être réattribué. Aucun processus étranger ne peut être atteint.
   - Le surveillant est un seul processus : la lecture utilise `read`, intégré au shell, et non `cat`. Quand la commande se termine d'elle-même, `kill "$w"` le termine entièrement ; `$w` est un enfant non encore attendu du shell principal, son PID ne peut pas être réattribué. Le code de sortie de la commande est remonté.
   - Application tuée : le système ferme la connexion TCP, sshd ferme le canal aussitôt et le surveillant se déclenche. Après une coupure réseau sans fermeture, le délai dépend de `ClientAliveInterval` côté serveur.
   - Hypothèse : le shell de connexion du compte est compatible POSIX (bash, dash, zsh).

### Vérification de la clé hôte en deux temps

1. Première connexion : l'empreinte est inconnue, `connect` renvoie `UnknownHostKey` avec l'empreinte reçue.
2. L'interface l'affiche, l'utilisateur la compare à celle notée sur le VPS.
3. Après validation, `trust_host_key` l'enregistre dans le profil.
4. Connexions suivantes : toute différence renvoie `HostKeyMismatch` et bloque la connexion.

### Fichiers d'unité systemd

Le texte du fichier est produit uniquement par le backend, par une fonction pure (`systemd/unit.rs`). L'aperçu du formulaire l'obtient par `service_unit_preview` : il est toujours identique au fichier écrit.

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
- La ligne `Description=` est omise si la description est vide (systemd affiche alors le nom de l'unité). Une ligne `Environment=` par variable, dans l'ordre saisi ; aucune s'il n'y en a pas.
- Le fichier se termine par un retour à la ligne.

Règles de lecture de systemd prises en compte (vérifiées dans le source de systemd 259 : `conf-parser.c`, `load-fragment.c`, `extract-word.c`) :

- Une ligne se termine par `\n`, `\r` ou un caractère nul ; une ligne qui finit par un nombre impair de `\` est jointe à la suivante. Les espaces au début et à la fin d'une valeur sont retirés.
- **Spécificateurs `%`** (`%h`, `%n`…) : résolus dans `Description=`, `WorkingDirectory=`, `ExecStart=` et `Environment=`. Un spécificateur inconnu (`%Y`) fait ignorer la directive (`Description=`, `Environment=`) ou met l'unité en erreur (`ExecStart=`, `WorkingDirectory=`). `%%` donne `%`.
- **`ExecStart=`** : les mots sont séparés par des espaces, `'…'` et `"…"` les regroupent, les échappements C (`\n`, `\x2d`, `\\`…) sont traités, y compris entre apostrophes. Puis les spécificateurs sont résolus. Préfixes reconnus en tête du premier mot, après retrait des guillemets : `-` (échec ignoré), `@` (argv[0] séparé), `:` (pas de substitution), `+`, `!` (privilèges), `|` (lancé par le shell) ; `!!` est ignoré depuis la v258. Un `;` isolé hors guillemets sépare deux commandes. Le programme doit être un chemin absolu ou un simple nom (cherché dans un PATH fixe) ; `~` n'est pas développé. À l'exécution, `$VAR` et `${VAR}` sont remplacés (variable inconnue : chaîne vide), `$$` donne `$`.
- **`Environment=`** : retrait des guillemets et échappements C, puis spécificateurs. `$` n'a aucun sens particulier.
- **`WorkingDirectory=`** : ni guillemets ni échappements, spécificateurs seulement. `~` désigne le dossier personnel ; un préfixe `-` tolère un dossier absent ; le chemin doit être absolu et normalisé (sans `.` ni `..`).

Validation (`InvalidInput`, le message nomme le champ) et écriture de chaque champ. Pour tous : aucun caractère de contrôle (retours à la ligne, `\r`, caractère nul, tabulation…), ce qui empêche d'insérer des directives supplémentaires.

| Champ | Validation | Écriture |
|---|---|---|
| `name` | Saisi sans `.service` : `^[a-z0-9][a-z0-9-]*$`, 64 caractères au plus | `<name>.service` |
| `description` | Facultative ; espaces de bord retirés ; 256 caractères au plus ; ne finit pas par `\` | `%` → `%%` |
| `execStart` | Requise ; espaces de bord retirés ; 4 096 octets au plus ; ne finit pas par `\` ; guillemets équilibrés ; aucun `;` isolé hors guillemets ; le premier mot n'est pas vide, ne commence pas par `-`, `@`, `:`, `+`, `!`, `\|`, `~` ni `;`, ne contient pas `\`, et est un chemin absolu ou un nom sans `/` | `%` → `%%`, `$` → `$$` |
| `workingDir` | Requis (pré-rempli avec `~`) ; espaces de bord retirés ; `~`, `~/chemin` ou chemin absolu ; sans `\` ; sans composant `.` ou `..` ; 4 096 octets au plus | `~` tel quel ; `~/chemin` → `%h/chemin` ; `%` → `%%` (le `%h` de la conversion est le seul spécificateur produit) |
| `restart` | `no`, `on-failure` ou `always` | tel quel |
| `env` | 32 variables au plus ; clé `^[A-Za-z_][A-Za-z0-9_]*$`, 128 caractères au plus, sans doublon ; valeur de 4 096 octets au plus (espaces conservés) | `Environment="CLE=valeur"` avec `\` → `\\`, `"` → `\"`, `%` → `%%` |

Choix pour `ExecStart=` : la syntaxe de systemd (guillemets, échappements) est conservée, mais aucune substitution n'a lieu : `%` et `$` arrivent tels quels au programme. Les variables d'environnement restent transmises au processus ; pour les utiliser dans les arguments, passer par un shell (`/bin/sh -c '… $VAR …'`), qui les développe. Les préfixes spéciaux sont refusés : `-` neutraliserait `Restart=on-failure`, `@` et `|` changent l'interprétation de la ligne, `+` et `!` n'ont pas d'usage dans un service utilisateur.

Exemple :

```ini
[Unit]
Description=Sauvegarde 100%% locale

[Service]
Type=simple
WorkingDirectory=%h/backup
ExecStart=/bin/sh -c 'tar czf "a-$$(date +%%Y).tgz" data'
Restart=on-failure
RestartSec=5
Environment="TOKEN=a\"b\\c"

[Install]
WantedBy=default.target
```

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
├── stores/            Pinia : connection, profiles, files, transfers, services, scripts, telemetry
├── composables/       logSession, useDockedStream, useConfirm, useNotify, useVisiblePolling
├── router/            routes + garde « connexion requise »
├── styles/            tokens.css, base.css
├── components/
│   ├── ui/            design system (Button, Modal, Console, Toast…)
│   └── features/      FileBrowser, ServiceList, ServiceForm, ScriptList…
└── views/             ConnectView, DashboardView, FilesView, ScriptsView, SettingsView
```

### Règles

- **Aucun `invoke()` en dehors de `api/`.** Stores et composants appellent des fonctions typées (`docker.list()`, etc.).
- **Sessions de flux** : `createLogSession` porte l'état d'un flux (logs ou script) : démarrage, mémoire tampon limitée (5 000 lignes par défaut, les plus anciennes sont supprimées), compteurs, arrêt, relance. Elle ne dépend d'aucun composant : son `Channel` reste vivant tant que la page existe. Toutes les sessions appartiennent au store `telemetry` :
  - **onglets de l'écran Télémétrie** (sortie de script ou logs) : un flux vit jusqu'à la fermeture de son onglet, la déconnexion ou la fermeture de l'application. Quitter l'écran, changer d'onglet ou de section ne l'interrompt pas, et sa sortie continue d'être collectée. Fermer l'onglet d'un script en cours, l'interrompre, couper la liaison ou fermer l'application pendant un script demande un GO / NO GO ;
  - **panneau ancré du tableau de bord** : `useDockedStream` arrête son flux quand on quitte le tableau de bord (démontage) et le relance au retour, la source choisie étant gardée ; Fermer l'arrête. « Plein écran » transfère la session vers un nouvel onglet, sans redémarrer le flux ni perdre la mémoire tampon.
- **Indicateur de télémétrie** : hors de l'écran Télémétrie, la navigation signale les procédures en cours (forme « En transition » et chrono T+), ou à défaut le nombre de flux ouverts, avec un lien vers l'onglet.
- **Transferts** : file d'attente gérée dans le store `transfers`, un seul transfert actif à la fois. Chaque transfert a un identifiant et peut être annulé. « Afficher » ouvre le dossier local du fichier téléchargé via le plugin officiel opener de Tauri (`revealItemInDir`). Le store vit au-delà de l'écran Fichiers : un transfert continue pendant la navigation vers une autre section.
- **Explorateur** : la liste est virtualisée (`@tanstack/vue-virtual`), triée côté Vue, dossiers (et liens vers un dossier) en premier. Les fenêtres de sélection de fichiers viennent du plugin officiel dialog (`open`, `save`). Avant une action qui peut couper l'accès SSH (`FileEntry.sshAccess`), le GO / NO GO affiche un avertissement et exige une case à cocher. Les résidus de transfert (`FileEntry.residue`) sont signalés dans la liste.
- **Création d'un service** (écran ouvert depuis le tableau de bord) : validation instantanée de chaque champ avec les mêmes règles que le backend (`src/serviceRules.ts`, voir « Fichiers d'unité systemd ») ; aperçu du fichier obtenu du backend (`service_unit_preview`), 300 ms après la dernière saisie ; avertissement si le lingering n'est pas actif (`user_linger`). Après la création, retour au tableau de bord : la ligne est « en transition » puis vérifiée à 2,5 s, comme après un démarrage.
- **Suppression d'un service** : action isolée par un séparateur, désactivée avec sa raison (`deleteBlock`) en infobulle. Le GO / NO GO liste les conséquences d'après `service_delete_info`. La liste est relue dès la fin de la séquence.
- **Rafraîchissement des services** : toutes les 5 secondes uniquement quand le tableau de bord est affiché et la fenêtre visible : document visible (`document.visibilityState`) et fenêtre non réduite (`isMinimized` de Tauri, relu à chaque redimensionnement et changement de focus, car WebView2 ne masque pas le document quand la fenêtre est réduite), plus un bouton d'actualisation manuelle. Le rafraîchissement suivant est programmé 5 secondes après la fin du précédent, et un rafraîchissement manuel pendant qu'un autre est en cours réutilise celui-ci : deux rafraîchissements ne se chevauchent jamais. Docker et systemd sont appelés indépendamment : l'échec de l'un n'empêche pas l'affichage de l'autre.
- **Sortie d'erreur ≠ erreur** : beaucoup de programmes écrivent tous leurs logs sur la sortie d'erreur (Caddy, le module `logging` de Python par défaut). Une ligne `stderr` est donc marquée de façon neutre (étiquette discrète, sans fond rouge). Le style d'erreur rouge est réservé aux messages systemd de priorité ≤ 3.
- **Console** : un seul composant console, utilisé à l'identique dans l'écran Télémétrie (logs et sortie des scripts) et le panneau ancré du tableau de bord (défilement automatique, indicateur de nouvelles lignes, bannière LOS). Le panneau ancré réduit la zone du tableau au-dessus de lui, il ne la recouvre pas.
- **Barre de titre personnalisée** : la barre native est désactivée (`decorations: false`). La zone de déplacement utilise `data-tauri-drag-region`, les boutons réduire, agrandir et fermer appellent l'API de fenêtre de Tauri, avec les permissions correspondantes dans les capabilities. Le double-clic sur la barre agrandit la fenêtre. Limite connue : sous Windows, le menu d'ancrage (Snap Layouts) au survol du bouton agrandir n'est pas disponible ; sous Linux, le comportement est à tester selon le gestionnaire de fenêtres.
- **Build de production** (`productionGuards.ts`) : F5, Ctrl+R et Ctrl+Maj+R sont bloqués (un rechargement interromprait sans confirmation les scripts en cours), ainsi que le menu contextuel du moteur web, sauf dans un champ de saisie (couper, copier, coller). Les menus de l'application ne sont pas touchés. En développement, rien ne change.
- **Barre d'état** : affiche `server_info` (système, Docker, systemd), récupéré une fois après la connexion.
- **Erreurs** : chaque `kind` de `AppError` est traduit en message clair. `HostKeyMismatch` redirige vers un écran d'alerte dédié, jamais une simple notification.

### Permissions (capabilities)

`src-tauri/capabilities/default.json` n'accorde que ce que le frontend appelle ; pas de `core:default` (menus, barre système, images, émission d'événements… inutilisés). Les commandes de l'application ne sont pas soumises aux permissions (pas de manifeste d'application), ni la récupération des messages d'un `Channel` (exception explicite de Tauri). Le presse-papiers passe par `navigator.clipboard`, hors permissions.

| Permission | Usage |
|---|---|
| `core:app:allow-version` | version dans la barre d'état (`getVersion`) |
| `core:event:allow-listen`, `core:event:allow-unlisten` | `onResized`, `onFocusChanged`, `onCloseRequested` |
| `core:path:allow-resolve-directory`, `core:path:allow-join` | dossier Téléchargements proposé à l'enregistrement (`downloadDir`, `join`) |
| `core:window:allow-is-minimized`, `core:window:allow-is-maximized` | visibilité réelle de la fenêtre, état agrandi |
| `core:window:allow-start-dragging`, `core:window:allow-internal-toggle-maximize` | `data-tauri-drag-region` : déplacement et double-clic |
| `core:window:allow-minimize`, `core:window:allow-toggle-maximize`, `core:window:allow-close` | boutons de la barre de titre |
| `core:window:allow-destroy` | fermeture après le GO / NO GO (`onCloseRequested` ferme par `destroy()`) |
| `core:webview:allow-internal-toggle-devtools` | Ctrl+Maj+I en développement (sous Linux, l'inspecteur n'a pas d'autre raccourci) ; sans effet en production, où l'inspecteur n'est pas compilé |
| `dialog:allow-open`, `dialog:allow-save` | fichiers à envoyer, emplacement d'un téléchargement |
| `opener:allow-reveal-item-in-dir` | « Afficher » après un téléchargement |

Une nouvelle API de Tauri utilisée dans `src/api/` demande d'ajouter sa permission ici et dans le fichier.

### Politique de sécurité du contenu (CSP)

Définie dans `tauri.conf.json` (`app.security.csp`) : tout vient de l'application elle-même, rien d'autre n'est autorisé.

| Directive | Valeur | Raison |
|---|---|---|
| `default-src`, `script-src` | `'self'` | modules JS du build, aucun script en ligne |
| `style-src` | `'self'` | feuilles CSS du build ; pas de `'unsafe-inline'` |
| `img-src`, `font-src` | `'self'` | image de fond et polices embarquées |
| `connect-src` | `ipc: http://ipc.localhost` | IPC de Tauri (`ipc:` sous Linux, `http://ipc.localhost` sous Windows) |
| `object-src`, `base-uri`, `form-action`, `frame-src` | `'none'` | inutilisés |

- Les styles calculés (`:style` de Vue : console virtualisée, radar, orbite) passent par l'API de style du DOM, que la CSP ne bloque pas. Un `<style>` ou un attribut `style="…"` inséré dans le HTML serait bloqué : à éviter (Reka UI n'en insère que dans son Splitter, non utilisé).
- Vite intègre en URI `data:` les fichiers importés de moins de 4 Ko : un tel import serait bloqué par `img-src` ou `font-src`. Aujourd'hui, aucun fichier n'est dans ce cas.
- Pas de `frame-ancestors` : ignorée dans une balise meta, forme sous laquelle Tauri injecte la CSP sous Linux.
- **La CSP n'est pas appliquée en `tauri dev`** sur ordinateur : la webview charge directement le serveur de Vite (le relais de Tauri n'existe que sur mobile). Pour la vérifier avec l'inspecteur : `npm run tauri build -- --debug --no-bundle`, puis lancer `src-tauri/target/debug/atlas-vps` (fichiers embarqués, vraie CSP, inspecteur disponible).

### Routes

| Route | Vue | Connexion requise |
|---|---|---|
| `/connect` | ConnectView | non |
| `/dashboard` | DashboardView (Docker + systemd) | oui |
| `/dashboard/new-service` | ServiceCreateView (création d'un service ; Tableau de bord surligné) | oui |
| `/files/:path*` | FilesView (chemin absolu ; `/files` = racine) | oui |
| `/scripts` | ScriptsView | oui |
| `/telemetry/:tab?` | TelemetryView (onglets des flux ; section d'origine surlignée : Scripts ou Tableau de bord) | oui |
| `/settings` | SettingsView (profils) | oui |

## Tests

- **Backend** : `cargo test`, sans réseau ni VPS (fonctions pures, `MemFs`, exécutions simulées). `tests/ssh_integration.rs` vise un vrai serveur et reste ignoré par défaut.
- **Frontend** : Vitest (`npm test`), environnement `happy-dom`, fichiers `src/**/*.test.ts`. Les modules de `src/api/` sont remplacés par des simulations (`vi.mock`) : aucun `invoke` réel. Ciblent la logique : stores, `logSession`, validations, formats. `@vue/test-utils` sert uniquement aux composables liés au cycle de vie d'un composant (`useDockedStream`). Pas de tests d'instantané.

### Cas partagés Rust / TypeScript

`tests/shared/*.json` : entrée, verdict ou résultat attendu, justification (format dans `tests/shared/README.md`). Chaque fichier est lu par `cargo test` (`include_str!`, module `shared_cases`) et par `npm test` (`src/sharedCases.test.ts`). Le backend fait foi.

| Fichier | Rust | TypeScript |
|---|---|---|
| `service-name.json` | `systemd::unit::unit_name` | `serviceRules.nameProblem` |
| `service-description.json`, `service-working-dir.json`, `service-exec-start.json`, `service-env.json` | `systemd::unit::render_unit` (un champ varié) | `serviceRules.*Problem`, `envProblems` |
| `remote-name.json` | `sftp::path::validate_name` | `remoteFiles.nameProblem` |
| `remote-join.json`, `remote-parent.json` | `sftp::path::join`, `split` | `remoteFiles.joinPath`, `parentPath` |
| `script-fields.json` | `scripts::normalize` (nom requis et longueur, commande requise) | `scriptRules.scriptProblems` |
| `profile-fields.json` | `config::normalize` (champs requis, port) | `profileRules.profileProblems` |

- Espaces de bord et longueurs : `src/text.ts` reproduit `str::trim` (propriété Unicode White_Space, différente de `String.prototype.trim` pour U+0085 et U+FEFF) et `chars().count()` (points de code, pas unités UTF-16).
- Règles présentes d'un seul côté, donc sans cas partagés :
  - backend seul : regex large des unités (`^[a-zA-Z0-9@._-]+\.service$`, l'interface reçoit `controllable`), états Docker et systemd (`RunState`), texte du fichier d'unité (échappements de `ExecStart=`, `WorkingDirectory=`, `Environment=` ; l'aperçu vient de `service_unit_preview`), IDs Docker, résidus, `sshAccess`, séquences ANSI, durées Docker, contrôles des profils et scripts au-delà des champs requis ;
  - interface seule : tri de l'explorateur, formats d'affichage, ligne d'erreur du journal (priorité ≤ 3).

## Packaging

Installateurs produits par `npm run tauri build` (`bundle.targets`) : NSIS sous Windows, .deb et AppImage sous Linux. Publication par `.github/workflows/release.yml` (voir README).

| | Valeur | Où |
|---|---|---|
| Nom affiché | « Atlas » (fenêtre, menu Démarrer, menu des applications) | `productName`, `Name=` du modèle `src-tauri/linux/atlas-vps.desktop` |
| Exécutable | `atlas-vps` (`atlas-vps.exe`) | `mainBinaryName` |
| Paquet .deb, fichiers .deb et AppImage, `.desktop`, icônes `hicolor` | `atlas-vps` | `src-tauri/tauri.linux.conf.json` (`productName`, fusionné sous Linux) |
| Identifiant, dossier de configuration, service du trousseau | `dev.poutoo.atlas` | `identifier`, `secrets::SERVICE` (test qui les compare) |

- Le bundler de Tauri dérive le nom du paquet .deb, des fichiers produits et du `.desktop` de `productName`, sans option dédiée : d'où la surcharge de `productName` sous Linux, et le modèle `.desktop` qui garde « Atlas » comme nom affiché. Le reste de l'application n'utilise pas `productName` (titre de fenêtre fixé dans `app.windows`, dossiers et trousseau tirés de l'identifiant).
- Sous Linux, la fenêtre est rattachée à son entrée de menu par le nom de l'exécutable (GTK sans identifiant d'application, `StartupWMClass=atlas-vps`).
- NSIS : installation pour l'utilisateur courant (`%LOCALAPPDATA%\Atlas`), en français, icône de l'installateur et du désinstalleur `icons/icon.ico`.
- .deb : Tauri déclare `libwebkit2gtk-4.1-0` et `libgtk-3-0` ; `libdbus-1-3` (lié par `tao`) est ajouté. `gnome-keyring | org.freedesktop.secrets` est recommandé : sans service Secret Service, la passphrase ne peut pas être mémorisée.
- Icônes : `npm run icons` (voir `scripts/icons.js`) ; l'entrée 16 × 16 du .ico et `16x16.png` viennent de `atlas-icone-16.svg`. Le .deb installe les PNG de `bundle.icon` dans `hicolor/<taille>/apps/atlas-vps.png` ; `icon.png` (512 px) y figure car Tauri range `128x128@2x.png` dans `256x256@2`, dossier que les thèmes d'icônes ne lisent pas.
- Profil `release` : `opt-level = 3`, LTO, une seule unité de compilation, symboles retirés ; pas de `panic = "abort"` (une panique n'arrête que sa tâche).
