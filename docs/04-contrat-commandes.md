# Contrat des commandes Tauri

Statut : validé.

## Conventions

- Commandes et arguments en `snake_case` côté Rust ; arguments en `camelCase` côté `invoke()` (conversion par défaut de Tauri). Exemple : `rename(from_path, to_path)` s'appelle `invoke('rename', { fromPath, toPath })`.
- Les types TypeScript sont générés depuis Rust avec `ts-rs`.
- Les conteneurs Docker sont identifiés par leur ID.

## Erreurs

```ts
type AppError =
  | { kind: "NotConnected" }
  | { kind: "LinkLost" }                   // échec dû à la perte de la liaison (voir « Perte de la liaison »)
  | { kind: "UnknownHostKey"; fingerprint: string }
  | { kind: "HostKeyMismatch"; expected: string; received: string }
  | { kind: "PassphraseRequired" }
  | { kind: "PassphraseIncorrect" }
  | { kind: "AuthFailed" }
  | { kind: "Cancelled" }                  // connexion interrompue par l'utilisateur
  | { kind: "Network"; message: string }   // y compris délai dépassé
  | { kind: "InvalidInput"; message: string }
  | { kind: "Remote"; exitCode: number; stderr: string }
  | { kind: "Io"; message: string }
  | { kind: "DockerNotInstalled" }         // commande docker introuvable sur le serveur
  | { kind: "DockerPermissionDenied" }     // utilisateur hors du groupe docker
  // Fichiers : `side` indique où se trouve l'élément (VPS ou machine locale).
  | { kind: "PermissionDenied"; path: string; side: FileSide }
  | { kind: "NotFound"; path: string; side: FileSide }
  | { kind: "AlreadyExists"; path: string; isDir: boolean }   // sur le VPS
  | { kind: "InsufficientSpace"; path: string; side: FileSide; needed: number | null; available: number | null }
  // Création ou suppression d'un service arrêtée en cours de séquence (voir « Systemd »).
  | { kind: "ServiceStepFailed"; unit: string; step: ServiceStep; state: string; cause: AppError }

type FileSide = "local" | "remote"

// Étape d'une séquence de création (reload, enable) ou de suppression (les autres, et reload).
type ServiceStep = "reload" | "enable" | "stop" | "disable" | "removeDropIn" | "removeFile" | "resetFailed"
```

`HostKeyMismatch` est présenté comme une alerte de sécurité, pas comme une erreur ordinaire.

Le serveur SFTP d'OpenSSH ne distingue que « introuvable » et « accès refusé » : un élément déjà existant, un disque plein ou un dossier non vide arrivent tous en échec générique. Après un tel échec, le backend vérifie la cause (`lstat` de la cible, espace libre via `statvfs@openssh.com`) avant de choisir l'erreur ; à défaut, `Io` avec le message du serveur.

## Connexion et profils

```ts
list_profiles(): Profile[]
save_profile(profile: Profile): Profile   // si l'hôte ou le port change, l'empreinte mémorisée est effacée ; un seul profil par défaut
delete_profile(id: string): void
set_key_passphrase(profileId: string, passphrase: string): void
trust_host_key(profileId: string, fingerprint: string): void
connect(profileId: string, onStep: Channel<ConnectStep>): ConnectionInfo
disconnect(): void      // ferme la liaison, ou interrompt une connexion en cours
connection_status(): ConnectionInfo | null
server_info(): ServerInfo      // appelé une fois après la connexion, affiché dans la barre d'état
```

`server_info` lit `PRETTY_NAME` dans `/etc/os-release`, `docker version --format '{{.Server.Version}}'` et la première ligne de `systemctl --version`. Si une information n'est pas disponible, le champ vaut `null` : l'échec d'une de ces commandes ne doit jamais bloquer la connexion.

### Séquence de connexion

`connect` émet une étape par phase réelle, affichée dans la télémétrie de liaison :

```ts
type ConnectStep =
  | { step: "tcp"; status: "ok" | "error"; durationMs: number | null; message: string | null }
  | { step: "ssh"; status: "ok" | "error"; message: string | null }
  | { step: "hostKey"; status: "ok" | "unknown" | "mismatch"; fingerprint: string }
  | { step: "auth"; status: "ok" | "passphraseRequired" | "error"; message: string | null }
  | { step: "aos" }
```

Clé hôte inconnue ou différente : la liaison est **fermée** avant toute authentification, et `connect` échoue avec `UnknownHostKey` ou `HostKeyMismatch`. Après validation par l'utilisateur (`trust_host_key`), une **nouvelle** connexion est lancée. On ne garde pas la négociation suspendue pendant que l'utilisateur compare l'empreinte : le serveur la couperait de lui-même passé son délai de connexion (`LoginGraceTime`, 120 secondes par défaut sur OpenSSH).

Les empreintes sont mémorisées dans le profil de connexion de l'application, jamais dans `~/.ssh/known_hosts`.

```ts
interface ConnectionInfo {
  profileId: string
  connectedAt: number        // horodatage
  latencyMs: number | null   // mesurée toutes les 30 secondes en chronométrant l'ouverture d'un canal de session (sans commande), refermé aussitôt
}
```

`connection_status` renvoie `null` dès que la liaison est fermée ou perdue.

### Perte de la liaison

Le backend détecte la perte et émet **un seul** événement Tauri `link-lost` par liaison perdue. Une déconnexion volontaire (`disconnect`, nouvelle `connect`, fermeture de l'application) n'en émet jamais.

```ts
// Événement "link-lost"
interface LinkLost {
  profileId: string
  cause: LossCause
  detail: string | null           // message du serveur (déconnexion) ou du système
  lastReceivedAt: number | null   // dernière réception de données du serveur (ms epoch) : « dernier échange »
  detectedAt: number
}

type LossCause =
  | "closed"    // fermée par le serveur : message de déconnexion SSH ou fin du flux TCP
  | "reset"     // connexion réinitialisée (RST) ou interrompue par le système
  | "timeout"   // le serveur ne répond plus : keepalive sans réponse, ou écriture bloquée
  | "network"   // réseau ou hôte injoignable
  | "unknown"
```

Détection (russh 0.63, `ssh/link.rs`) :

| Réglage | Valeur | Effet |
|---|---|---|
| `keepalive_interval` | 5 s | keepalive envoyé après 5 s sans rien recevoir du serveur |
| `keepalive_max` | 3 | perte constatée `(3 + 1) × 5 s` = 20 s après la dernière réception, soit 15 à 20 s après une coupure silencieuse ; une coupure de quelques secondes est absorbée |
| `inactivity_timeout` | 60 s | filet pour une écriture bloquée (envoi en cours sur une liaison coupée) : pendant une écriture, russh ne fait pas tourner le keepalive |

- Fermeture par le serveur (FIN, RST, message de déconnexion) : détection immédiate.
- Un serveur qui ferme la connexion alors que des données du client ne sont pas encore lues provoque un RST, qui peut faire perdre le message de déconnexion : la cause affichée est alors `reset` au lieu de `closed` (redémarrage du VPS, par exemple).
- Listes du tableau de bord (`docker_list`, `services_list`) : délai de 30 s pour la même raison. Une coupure plus courte que la détection retarde le rafraîchissement (« Interrogation · T+ ») sans afficher d'échec.
- Requêtes SFTP : délai de 30 s (10 s par défaut dans russh-sftp), plus long que la détection d'une perte silencieuse. Sur une liaison coupée, la perte est constatée avant que les requêtes n'expirent : leur échec devient `LinkLost`. Exception : une écriture bloquée (jusqu'à 60 s) peut encore expirer avant la détection ; l'interface suspend alors la file (voir `docs/03-architecture.md`).
- Liaison au repos : chaque réponse à un keepalive réarme aussi `inactivity_timeout`, qui ne se déclenche donc pas (vérifié dans `client/mod.rs` : le minuteur d'inactivité n'est réarmé qu'après un tour de boucle sans envoi de keepalive, c'est-à-dire à la réception de la réponse).

Après l'événement :

- `connection()` renvoie `LinkLost` jusqu'à la prochaine `connect` ou `disconnect` ; `connection_status` renvoie `null`.
- Toute commande qui échoue à cause de la perte renvoie `LinkLost`, quelle que soit l'erreur sous-jacente. L'interface ne la présente pas comme l'échec de l'opération.
- Les flux et les transferts coupés n'émettent aucun événement de fin (`error`, `cancelled`) : `link-lost` en tient lieu. Les registres de flux et de transferts sont vidés.
- Aucune reconnexion automatique. `connect` crée une nouvelle liaison, avec sa propre session SFTP.

## Fichiers

```ts
home_dir(): string                       // dossier personnel, chemin absolu (realpath de « . »)
list_dir(path: string): FileEntry[]
create_file(path: string): void
create_dir(path: string): void
rename(fromPath: string, toPath: string): void
dir_summary(path: string): DirSummary    // calculé avant le GO / NO GO d'une suppression
delete_path(path: string, recursive: boolean): void
upload(localPath: string, remotePath: string, overwrite: boolean, onProgress: Channel<Progress>): string  // transferId
download(remotePath: string, localPath: string, onProgress: Channel<Progress>): string                   // transferId
transfer_cancel(transferId: string): void
```

Toutes les opérations passent par la session SFTP partagée : aucune commande shell.

### Chemins et noms

- Un chemin distant est absolu, sans caractère nul, sans composant vide, `.` ou `..` (`/` seul désigne la racine), sinon `InvalidInput`. Les chemins sont construits par le backend (`FileEntry.path`) ou par l'interface en joignant un dossier et un nom ; ils ne sont jamais résolus ni normalisés.
- Un nom saisi (création, renommage) n'est ni vide, ni `.`, ni `..`, et ne contient ni `/` ni caractère nul, sinon `InvalidInput`.
- `delete_path` et `rename` refusent la racine, le dossier personnel et les dossiers qui le contiennent, y compris atteints par un chemin qui les résout (`InvalidInput`) : l'opération emporterait le dossier personnel. Un lien vers eux reste un lien ordinaire.

### Création, renommage, suppression

- `create_file` (`open` avec `CREATE | EXCLUDE`) et `create_dir` (`mkdir`) ne remplacent jamais un élément existant : `AlreadyExists`.
- `rename` refuse une destination existante (`AlreadyExists`), vérifiée par `lstat` avant l'opération. OpenSSH la refuse aussi de lui-même.
- `delete_path` lit l'élément avec `lstat` : un fichier ou un lien symbolique est supprimé directement (pour un lien, le lien seul, jamais sa cible). Un dossier :
  - `recursive = false` : `rmdir`, refusé s'il n'est pas vide ;
  - `recursive = true` : toute l'arborescence est d'abord lue, sans jamais suivre un lien symbolique ; si un sous-dossier est illisible, rien n'est supprimé. Puis les éléments sont supprimés (16 requêtes en parallèle), et les dossiers du plus profond au moins profond. Une erreur pendant cette phase arrête la suppression : ce qui a déjà été supprimé l'est définitivement.
- `dir_summary` fait le même parcours en lecture seule. Il s'arrête à 100 000 éléments (`truncated`).

### Transferts

- `upload` et `download` vérifient leurs arguments, puis renvoient aussitôt le `transferId` ; la copie se poursuit en tâche de fond et sa progression arrive par `onProgress` (au plus un événement toutes les 200 ms, puis un événement final). Une erreur détectée avant le début (fichier local absent, cible existante…) est renvoyée par la commande elle-même.
- Lecture et écriture par blocs de 256 Kio : un fichier n'est jamais chargé entier en mémoire.
- Fichiers uniquement : un dossier à envoyer ou à télécharger est refusé (`InvalidInput`).
- **Envoi** : écriture dans un temporaire `.<nom>.atlas-<8 hex>.part` du dossier de destination, renommé à la fin seulement.
  - Cible absente : `rename` du temporaire.
  - Cible existante : refusée (`AlreadyExists`) si `overwrite` vaut `false`. Avec `overwrite`, en trois étapes, pour que le contenu complet existe toujours sous un nom : la cible est renommée en `.<nom>.atlas-<hex>.old`, le temporaire prend sa place, puis le `.old` est supprimé. Si la deuxième étape échoue, le `.old` reprend sa place. Les droits du fichier remplacé sont recopiés sur le nouveau.
  - Cible dossier : `AlreadyExists` avec `isDir`. Cible lien symbolique : toujours refusée (`InvalidInput` indiquant la cible du lien) ; un lien n'est jamais remplacé par un fichier.
  - Espace libre du VPS (`statvfs@openssh.com`, si le serveur le propose) inférieur à la taille du fichier : `InsufficientSpace` avant de commencer.
- **Téléchargement** : même principe côté local, temporaire `.<nom>.atlas-<8 hex>.part` dans le dossier choisi, synchronisé sur disque puis renommé (un fichier existant est remplacé : la confirmation est celle de la fenêtre d'enregistrement du système). Un lien vers un fichier est suivi.
- **Annulation** (`transfer_cancel`) : la copie s'arrête entre deux blocs, le fichier ouvert est fermé (la session SFTP, partagée, reste ouverte), le temporaire est supprimé, puis l'événement `cancelled` est émis. La commande rend la main une fois ce nettoyage terminé, au plus tard après 5 s. Un identifiant inconnu (transfert terminé) est ignoré. La validation finale (renommages) n'est pas interruptible.
- **Erreur** pendant la copie : le temporaire est supprimé et le fichier existant reste intact, puis l'événement `error` est émis. Si la liaison est perdue, aucun événement n'est émis (`link-lost` en tient lieu) et le temporaire distant ne peut pas être supprimé : il reste sur le VPS, signalé dans la liste comme résidu de transfert (voir `FileEntry.residue`).
- Les transferts en cours sont annulés à la déconnexion, au rechargement de l'interface et à la fermeture de l'application.
- La file d'attente est gérée par l'interface (un seul transfert actif à la fois) ; les chemins locaux sont choisis côté Vue avec `tauri-plugin-dialog`.

## Docker

```ts
docker_list(): ContainerList    // délai maximal 30 s, docker inspect des conteneurs sortis en 137 compris
docker_action(id: string, action: "start" | "stop" | "restart"): void
docker_remove(id: string): void
```

`id` est l'ID complet : exactement 64 caractères hexadécimaux minuscules, sinon `InvalidInput`.

| Commande | Commande distante | Délai maximal |
|---|---|---|
| `docker_action` start | `docker start '<id>'` | 20 s |
| `docker_action` stop | `docker stop '<id>'` | 45 s |
| `docker_action` restart | `docker restart '<id>'` | 45 s |
| `docker_remove` | `docker rm '<id>'` | 20 s |

- `stop` et `restart` n'imposent pas de délai d'arrêt (`-t`) : celui du conteneur est respecté (10 s par défaut, `stop_grace_period` de compose). Un conteneur dont le délai dépasse 45 s fait échouer la commande (`Network`), mais le daemon termine l'arrêt.
- `docker_remove` n'utilise jamais `-f` ni `-v` : un conteneur en marche n'est pas supprimé (`Remote`), ses volumes et son image sont conservés.
- Erreurs : comme `docker_list` (`DockerNotInstalled`, `DockerPermissionDenied`, sinon `Remote`).

## Systemd

```ts
services_list(): Service[]      // délai maximal 30 s ; trié par nom d'unité
service_action(unit: string, action: "start" | "stop" | "restart"): void
service_unit_preview(def: ServiceDef): string   // texte du fichier d'unité, sans connexion
service_create(def: ServiceDef): Service
service_delete_info(unit: string): ServiceDeleteInfo
service_delete(unit: string): void
service_set_startup(unit: string, enabled: boolean): Service   // délai maximal 30 s
user_linger(): boolean | null   // null : état inconnu
```

Les deux listes sont des commandes indépendantes : l'interface les appelle séparément, et l'échec de l'une n'empêche pas l'affichage de l'autre. Les états d'interface (`RunState`), le code de sortie et la règle de filtrage des services sont calculés dans le backend (voir `docs/03-architecture.md`).

`service_action` (délai maximal 20 s pour l'ensemble) :

1. `unit` doit respecter `^[a-zA-Z0-9@._-]+\.service$`, sinon `InvalidInput`.
2. La liste des services est relue (mêmes commandes que `services_list`) : l'unité doit y figurer et être pilotable (`controllable`, donc non masquée), sinon `InvalidInput`.
3. `systemctl --user --no-block <start|stop|restart> -- '<unit>'` : la commande rend la main dès que la tâche est mise en file. Le résultat réel est vérifié ensuite par l'interface (voir « Vérification après une action » dans `docs/03-architecture.md`).

### Activation au démarrage

`services_list` renseigne `startup` (état de `systemctl --user list-unit-files --type=service --output=json --no-pager`) et `startupBlock` (voir `docs/03-architecture.md`).

`service_set_startup(unit, enabled)` (délai maximal 30 s pour l'ensemble) :

1. `unit` doit respecter `^[a-zA-Z0-9@._-]+\.service$`, sinon `InvalidInput`.
2. La liste des services est relue : l'unité doit y figurer avec `startupBlock` à `null` (état `enabled` ou `disabled`, fichier ordinaire du dossier), sinon `InvalidInput`.
3. `systemctl --user enable -- '<unit>'` ou `systemctl --user disable -- '<unit>'`, sans `--now` : rien n'est démarré ni arrêté ; `systemctl` recharge lui-même systemd. Échec : `Remote`.
4. La liste est relue et le `Service` relu est renvoyé.

### Fichier d'unité

`service_unit_preview` et `service_create` produisent le texte par la même fonction pure : l'aperçu du formulaire est identique au fichier écrit. Validation des champs, échappements et gabarit : voir « Fichiers d'unité systemd » dans `docs/03-architecture.md`. Un champ invalide donne `InvalidInput`, dont le message nomme le champ. `service_unit_preview` n'a pas besoin de la liaison.

Les chemins sont absolus, construits à partir du dossier personnel (`realpath` de `.` par SFTP) : `<home>/.config/systemd/user/<nom>.service`. Toutes les opérations sur les fichiers passent par SFTP, toutes les commandes par `systemctl --user`, sans `sudo`. Le nom, validé, est échappé par `shell::quote` et placé après `--`.

### Création

`service_create(def)`, dans l'ordre. Délais : 30 s pour les contrôles (étapes 1 et 2), 30 s pour l'écriture (étapes 3 et 4), 30 s pour le rechargement et l'activation (étapes 5 et 6).

1. Validation de `def` et production du texte. Erreur : `InvalidInput`.
2. Contrôles, sans rien écrire :
   - `lstat` de `<nom>.service` dans le dossier : s'il existe (fichier, lien ou dossier), `AlreadyExists`. La création n'écrase jamais rien.
   - `lstat` de `<nom>.service.d/` : s'il existe, `AlreadyExists` (`isDir`). Un dossier de surcharge oublié modifierait silencieusement le nouveau service.
   - `systemctl --user show --property=LoadState --property=FragmentPath -- '<nom>.service'` : si `LoadState` n'est pas `not-found`, le nom est déjà utilisé par une autre unité (fournie par le système dans `/usr/lib/systemd/user/`, par exemple) : `AlreadyExists` avec son `FragmentPath`, ou `InvalidInput` si celui-ci est vide. Sans ce contrôle, le fichier créé masquerait cette unité.
3. Création par SFTP des dossiers manquants parmi `~/.config`, `~/.config/systemd` et `~/.config/systemd/user`. Un élément existant qui n'est pas un dossier : `InvalidInput`.
4. Écriture atomique :
   - ouverture exclusive (`CREATE | EXCLUDE`) d'un temporaire `.<nom>.service.atlas-<8 hex>.part` du dossier ;
   - écriture du texte, fermeture, droits `0644` ;
   - renommage en `<nom>.service`, refusé par le serveur si la cible existe (`AlreadyExists`) ;
   - en cas d'échec, le temporaire est supprimé.
5. `systemctl --user daemon-reload`
6. `systemctl --user enable --now --no-block -- '<nom>.service'` : activation au démarrage (`WantedBy=default.target`) et démarrage mis en file, sans attendre son résultat.
7. Relecture de la liste ; le `Service` créé est renvoyé.

Une erreur aux étapes 1 à 4 ne laisse rien en place, à part les dossiers créés à l'étape 3. Une erreur aux étapes 5 ou 6 laisse le fichier en place : `ServiceStepFailed` (étape `reload` ou `enable`). Le résultat du démarrage est vérifié ensuite par l'interface, comme après un `service_action` : un service qui échoue au démarrage reste créé et apparaît « En échec ».

### Suppression

Uniquement pour un service dont `deleteBlock` est `null` : un fichier ordinaire dans `~/.config/systemd/user/` (pas une instance de gabarit, pas une unité masquée, pas un lien). Sinon `InvalidInput`.

`service_delete_info(unit)` fait les vérifications 1 et 2 ci-dessous, sans rien modifier, et renvoie les éléments affichés dans le GO / NO GO (`ServiceDeleteInfo`). Délai maximal 20 s.

`service_delete(unit)`, dans l'ordre. Délais : 100 s pour l'arrêt (plus que le `TimeoutStopSec` par défaut de 90 s, après lequel systemd envoie SIGKILL), 30 s pour chacune des autres étapes.

1. `unit` doit respecter `^[a-zA-Z0-9@._-]+\.service$`, sinon `InvalidInput`.
2. La liste des services est relue : l'unité doit y figurer avec `deleteBlock` à `null`, et son fichier (`lstat`) doit être un fichier ordinaire, sinon `InvalidInput`.
3. `stop` : `systemctl --user stop -- '<unit>'`, en attendant la fin de l'arrêt.
4. `disable` : `systemctl --user disable -- '<unit>'`.
5. `removeDropIn` : si `<unit>.d/` existe, suppression par SFTP de toute son arborescence, sans suivre les liens (même parcours que `delete_path` récursif). Un lien à cette place est supprimé seul.
6. `removeFile` : suppression par SFTP de `<unit>`.
7. `reload` : `systemctl --user daemon-reload`.
8. `resetFailed` : `if systemctl --user is-failed --quiet -- '<unit>'; then systemctl --user reset-failed -- '<unit>'; fi`. Une unité en échec reste en mémoire après le rechargement ; `reset-failed` seul échouerait sur une unité déjà déchargée.

Le dossier de surcharge est supprimé avant le fichier : si une suppression échoue, le fichier est encore là, le service reste dans la liste et la suppression peut être relancée depuis l'application.

Au premier échec, la séquence s'arrête et renvoie `ServiceStepFailed` : `step` est l'étape en échec, `state` décrit l'état du service, `cause` l'erreur de l'étape. Les erreurs des étapes 1 et 2 (rien n'a été modifié) sont renvoyées telles quelles.

| Étape en échec | État décrit (`state`) |
|---|---|
| `stop` | Rien n'a été modifié. L'arrêt est peut-être encore en cours. |
| `disable` | Service arrêté, toujours activé au démarrage, fichier en place. |
| `removeDropIn` | Service arrêté et désactivé, fichier en place, dossier de surcharge peut-être partiellement supprimé. |
| `removeFile` | Service arrêté et désactivé, dossier de surcharge supprimé, fichier en place. |
| `reload` | Fichier supprimé ; systemd garde l'ancienne définition en mémoire jusqu'au prochain `systemctl --user daemon-reload`. |
| `resetFailed` | Service supprimé ; systemd le liste encore en échec jusqu'à `systemctl --user reset-failed`. |

Création : `reload` → « Fichier créé, systemd non rechargé : service ni activé ni démarré. » ; `enable` → « Fichier créé et systemd rechargé : service ni activé ni démarré. »

### Lingering

`user_linger` : `loginctl show-user --property=Linger --value -- "$(id -u)"`, délai maximal 10 s. `yes` → `true`, `no` → `false`, toute autre réponse ou un échec → `null`. Sans lingering, les services utilisateur s'arrêtent à la fin de la dernière session du compte : le formulaire de création l'indique.

## Logs (Docker et systemd)

```ts
type LogSource = { docker: string } | { systemd: string }
logs_start(source: LogSource, tail: number, onEvent: Channel<LogEvent>): string  // streamId
logs_stop(streamId: string): void
```

- `source` : ID complet du conteneur (64 caractères hexadécimaux) ou nom d'unité respectant `^[a-zA-Z0-9@._-]+\.service$`, sinon `InvalidInput`. Une unité masquée est acceptée : ses logs restent consultables. Une unité dont le nom ne respecte pas la regex n'a donc pas de logs dans l'appli.
- `tail` : nombre de lignes d'historique, de 0 à 5 000 (taille de la mémoire tampon de la console), sinon `InvalidInput`.
- 4 flux simultanés au maximum (logs et scripts confondus) ; au-delà, `InvalidInput`. OpenSSH limite une connexion à 10 canaux par défaut (`MaxSessions`) : il en reste pour la session SFTP, la mesure de latence et les listes.
- `logs_start` : délai maximal de 10 s pour ouvrir le canal et lancer la commande ; le flux lui-même n'a pas d'échéance.
- `logs_stop` rend la main une fois le processus distant arrêté, au plus tard après 3 s. Un identifiant inconnu (flux déjà terminé) est ignoré.
- Commandes distantes :
  - Docker : `docker logs -f --tail <N> --timestamps '<id>'`
  - systemd : `journalctl --user --unit='<unit>' -f -n <N> -o json --output-fields=MESSAGE,PRIORITY --all --quiet --no-pager`

  Chacune est enveloppée dans un script de surveillance qui la termine quand le canal se ferme (voir « Arrêt des flux » dans `docs/03-architecture.md`).
- Événements : des lots de lignes (`lines`), puis au plus un événement final. `ended` n'est émis que si la commande se termine d'elle-même (conteneur arrêté, par exemple), `error` si le flux s'interrompt pour une autre raison. Liaison perdue : aucun événement final, `link-lost` en tient lieu. Après `logs_stop`, aucun événement n'est garanti : l'interface sait qu'elle a demandé l'arrêt.
- Regroupement : un lot part toutes les 50 ms, ou dès 500 lignes. Au-delà de 5 000 lignes par seconde, les lignes en excès sont écartées et comptées dans `dropped` du lot suivant.
- Toutes les lignes sont des `LogLine` : les lignes trop longues sont tronquées à 16 Kio, l'UTF-8 invalide est remplacé, et les séquences d'échappement ANSI (couleurs, styles, déplacements du curseur, titres, liens) sont retirées du texte, y compris une séquence incomplète en fin de ligne. Valable pour les logs Docker, le journal et la sortie des scripts.

## Scripts

```ts
scripts_list(): Script[]
scripts_save(script: Script): Script
scripts_delete(id: string): void
script_run(id: string, onEvent: Channel<LogEvent>): string  // runId
script_stop(runId: string): void
```

`script_run` réutilise `LogEvent` : un seul composant de console côté frontend pour les logs et les scripts.

### Stockage

- Scripts enregistrés localement dans `scripts.json` (dossier de configuration de l'application), avec la même écriture atomique que les profils (temporaire puis renommage).
- `scripts_list` renvoie les scripts dans l'ordre d'enregistrement.
- `scripts_save` : `id` vide pour créer (l'identifiant est attribué par le backend), sinon l'identifiant d'un script existant (`InvalidInput` sinon). Le `lastRun` reçu est ignoré : seul le backend l'écrit, la valeur enregistrée est conservée.
- Validation (`InvalidInput`) :
  - `name` : non vide après suppression des espaces, 64 caractères au plus, sans caractère de contrôle ;
  - `description` : facultative (chaîne vide), sans caractère nul ;
  - `command` : non vide après suppression des espaces, sans caractère nul, 64 Kio au plus. Les commandes sur plusieurs lignes sont autorisées ; les fins de ligne `\r\n` sont ramenées à `\n`.
- `scripts_delete` : identifiant inconnu → `InvalidInput`. Un script en cours d'exécution peut être supprimé : l'exécution continue, son `lastRun` n'est pas enregistré.

### Exécution

`script_run` lance la commande enregistrée (lue au moment du lancement : une modification ultérieure n'affecte pas l'exécution en cours) dans un **shell de connexion** de l'utilisateur, la commande étant transmise comme un seul argument échappé :

```sh
"${SHELL:-/bin/sh}" -l -c '<commande>'
```

- `SHELL` est toujours défini par sshd : c'est le shell de connexion du compte. `-l` charge `/etc/profile` et `~/.profile` (ou `~/.bash_profile`, `~/.zprofile` selon le shell) : le PATH qu'ils configurent est disponible.
- **Limite** : `~/.bashrc` n'est pas chargé utilement. Sous Ubuntu, `~/.profile` le source, mais il s'interrompt dès le début en mode non interactif (`case $- in *i*) ;; *) return;; esac`). Ce qu'il configure (nvm, alias, fonctions, PATH ajouté à la fin du fichier) n'est donc pas disponible : le script doit le charger lui-même (ex. `. "$HOME/.nvm/nvm.sh"`) ou utiliser des chemins complets. Le formulaire le rappelle sous le champ Commande.
- Tout ce que les fichiers de connexion écrivent (`/etc/profile`, `/etc/profile.d/*`, `~/.profile` et les fichiers qu'ils chargent : `echo`, message d'accueil, avertissement d'un outil) apparaît dans la sortie de **chaque** script, avant celle de la commande, sur `stdout` ou `stderr` selon le cas. Les séquences ANSI en sont retirées comme pour toute ligne. Une commande qui échoue dans un fichier de connexion n'arrête pas le shell (son message apparaît dans la sortie) ; un `exit` dans ce fichier empêche en revanche la commande de s'exécuter. Pour une sortie propre, ces fichiers ne doivent rien écrire en mode non interactif.
- Échappement par `shell::quote` (apostrophes, `'` → `'\''`) : plusieurs lignes, `$`, `"`, `;`, `` ` `` et accents arrivent intacts au shell.
- Dossier de départ : le dossier personnel (sshd s'y place avant d'exécuter la commande).
- Ni terminal ni entrée standard (`/dev/null`) : un script qui attend une saisie (`read`, mot de passe de `sudo`) lit une fin de fichier et échoue aussitôt au lieu de rester bloqué.
- Comme les logs, la commande est enveloppée dans le script de surveillance (voir « Arrêt des flux » dans `docs/03-architecture.md`) ; elle tourne dans le même groupe de processus que l'enveloppe. Limite : un processus qui ignore SIGTERM ou quitte le groupe (`setsid`, démon) survit à l'arrêt. C'est voulu pour `docker compose up -d` : les conteneurs appartiennent au daemon Docker.
- Délai maximal de 10 s pour ouvrir le canal et lancer la commande ; l'exécution elle-même n'a pas d'échéance.
- Limite de 4 flux simultanés, partagée avec les logs. Au-delà, `InvalidInput` avec le message « 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer ce script ».
- Un script déjà en cours d'exécution ne peut pas être relancé avant la fin de la première exécution (`InvalidInput`).
- Événements : comme les logs (lots de `lines`, puis `ended` avec le code de sortie, ou `error`). Origine `stdout` / `stderr` ; `timestamp` est l'heure de réception par le backend (le programme n'en fournit pas). `exitCode` vaut `null` si la commande a été tuée par un signal.
- `script_stop` : comme `logs_stop` (rend la main une fois le processus distant arrêté, 3 s au plus ; identifiant inconnu ignoré).
- `lastRun` est enregistré à la fin de chaque exécution : `at` est l'heure de **début** de l'exécution, `exitCode` le code de sortie de la fin naturelle, ou `null` pour une exécution interrompue (`script_stop`, déconnexion, liaison perdue, rechargement de l'interface, fermeture de l'application) ou tuée par un signal.

## Préférences

```ts
preferences_get(): Preferences
preferences_set_dimming(dimming: number): Preferences        // 0 à 100, sinon InvalidInput
preferences_set_original_colors(enabled: boolean): Preferences
```

- Fichier `preferences.json` du dossier de configuration, séparé des profils, écrit atomiquement (temporaire puis renommage) à chaque modification ; la valeur en mémoire ne change qu'après une écriture réussie (sinon `Io`).
- Lecture tolérante, une seule fois au démarrage : un fichier absent, illisible ou qui n'est pas un objet JSON donne les valeurs par défaut ; un champ manquant, d'un autre type ou hors limites reprend sa valeur par défaut sans toucher aux autres ; les champs inconnus et `version` sont ignorés. Le premier enregistrement remplace un fichier invalide.
- Format écrit : `{ "version": 1, "dimming": 85, "originalColors": false }`.

## Structures

```ts
interface Profile {
  id: string
  name: string
  host: string
  port: number
  user: string
  keyPath: string
  hostKeyFingerprint: string | null
  isDefault: boolean
}

interface ServerInfo {
  os: string | null        // ex. "Ubuntu 24.04.1 LTS"
  docker: string | null    // ex. "27.3.1"
  systemd: string | null   // ex. "255"
}

interface FileEntry {
  name: string
  path: string                  // dossier listé + nom
  kind: "file" | "dir" | "symlink" | "other"   // other : socket, fifo, périphérique
  size: number                  // octets (lstat : pour un lien, la longueur de sa cible)
  modified: number | null       // millisecondes depuis l'epoch Unix
  permissions: string           // ex. "drwxr-xr-x", "lrwxrwxrwx" ; "" si inconnu
  link: LinkInfo | null         // kind = "symlink" uniquement
  residue: Residue | null       // fichier temporaire laissé par un transfert interrompu
  sshAccess: boolean            // supprimer, renommer ou écraser peut couper l'accès SSH
}

interface LinkInfo {
  target: string                // cible telle qu'enregistrée (readlink, non résolue)
  targetKind: "file" | "dir" | "other" | "broken"   // stat (lien suivi) ; broken : cible absente
}

// Nom de la forme `.<nom>.atlas-<8 hex>.part` ou `.old`.
interface Residue {
  kind: "part" | "old"          // part : envoi ou téléchargement interrompu ; old : fichier d'origine pendant un écrasement
  original: string              // <nom>
  originalExists: boolean       // un élément <nom> existe dans le même dossier
}

interface DirSummary {
  dirs: number                  // sous-dossiers (le dossier lui-même non compté)
  files: number                 // fichiers, sockets, fifos, périphériques
  links: number                 // liens symboliques (supprimés, jamais suivis)
  bytes: number                 // taille cumulée des fichiers
  truncated: boolean            // parcours arrêté à 100 000 éléments
}

// État affiché dans l'interface, calculé par le backend.
type RunState = "nominal" | "stopped" | "failed" | "transition"

interface ContainerList {
  containers: Container[]   // triés par nom
  skipped: number           // lignes de docker ps illisibles (JSON invalide, ID non hexadécimal), exclues
}

interface Container {
  id: string                // ID complet (64 caractères hexadécimaux)
  name: string              // premier nom, sans les alias --link
  image: string             // sans digest ; image sans nom : 12 premiers caractères de son ID
  state: RunState
  dockerState: string       // état brut : running, exited, created…
  exitCode: number | null   // conteneur exited ou restarting, sinon null
  status: string            // texte brut de Docker, ex. "Exited (137) 4 days ago"
  duration: string | null   // "depuis 3 heures", "il y a 4 jours" ; null si non applicable
  oomKilled: boolean        // sorti en 137, tué par manque de mémoire (docker inspect) : state = "failed"
  memoryLimit: number | null  // limite mémoire en octets (HostConfig.Memory), lue avec oomKilled ; null : sans limite ou non lue
}

interface Service {
  unit: string
  load: string | null       // null : fichier présent mais unité non chargée par systemd
  active: string            // état brut : active, inactive, failed…
  sub: string               // sous-état brut : running, dead, exited…
  description: string | null  // null si l'unité n'est pas chargée
  state: RunState
  masked: boolean
  controllable: boolean     // false si masquée, ou si le nom ne respecte pas ^[a-zA-Z0-9@._-]+\.service$
  deleteBlock: DeleteBlock | null   // raison pour laquelle le service ne peut pas être supprimé ; null : suppression possible
  startup: string | null    // état de list-unit-files : enabled, disabled, static, masked… ; null : inconnu (instance, lecture impossible)
  startupBlock: StartupBlock | null // raison pour laquelle l'activation au démarrage ne peut pas être changée ; null : interrupteur actif
}

// Par ordre de priorité (voir docs/03-architecture.md).
type DeleteBlock = "invalidName" | "masked" | "templateInstance" | "linked"

// Par ordre de priorité (voir docs/03-architecture.md).
type StartupBlock = "invalidName" | "masked" | "templateInstance" | "linked" | "unknown" | "otherState"

interface ServiceDef {
  name: string                  // sans le suffixe .service : ^[a-z0-9][a-z0-9-]*$
  description: string           // "" : ligne Description= omise
  execStart: string
  workingDir: string            // ~, ~/chemin ou chemin absolu
  restart: "no" | "on-failure" | "always"
  env: EnvVar[]                 // dans l'ordre saisi ; clés sans doublon
}

interface EnvVar {
  key: string
  value: string
}

interface ServiceDeleteInfo {
  unit: string
  file: string                  // chemin absolu du fichier d'unité supprimé
  dropIn: string | null         // chemin absolu du dossier de surcharge <unit>.d/, s'il existe
  dropInEntries: number         // éléments qu'il contient (fichiers, liens, sous-dossiers)
  active: boolean               // le service n'est pas arrêté : il sera arrêté
}

interface Preferences {
  dimming: number             // assombrissement de l'image de fond, 0 (image nette) à 100 (masquée) ; 85 par défaut
  originalColors: boolean     // image en couleurs, sans la désaturation habituelle ; false par défaut
}

interface Script {
  id: string                     // "" à la création : attribué par le backend
  name: string
  description: string            // "" si absente
  command: string                // peut contenir plusieurs lignes
  requireConfirmation: boolean
  lastRun: LastRun | null        // écrit uniquement par le backend, à la fin de chaque exécution
}

interface LastRun {
  at: number                     // début de l'exécution, millisecondes depuis l'epoch Unix
  exitCode: number | null        // null : exécution interrompue ou tuée par un signal
}

interface LogLine {
  text: string
  timestamp: string | null          // ISO 8601 en UTC, à la milliseconde : 2026-10-16T09:13:46.900Z ; scripts : heure de réception
  origin: "stdout" | "stderr" | "journal"
  priority: number | null           // priorité syslog 0-7, uniquement pour origin = "journal"
  missing: boolean                  // entrée du journal sans MESSAGE : text vaut "[sans message]"
}

type LogEvent =
  | { type: "lines"; lines: LogLine[]; dropped: number }   // dropped : lignes écartées depuis le lot précédent (débit trop élevé)
  | { type: "ended"; exitCode: number | null }
  | { type: "error"; message: string }

type Progress =
  | { type: "progress"; transferred: number; total: number }
  | { type: "done" }
  | { type: "cancelled" }
  | { type: "error"; error: AppError }
```

`sshAccess` est vrai pour `~/.ssh`, pour ses parents jusqu'au dossier personnel exclu, et pour `~/.ssh/authorized_keys`. Le chemin comparé est celui du dossier listé résolu (`realpath`) suivi du nom : un dossier atteint par un lien symbolique est reconnu. L'interface affiche alors un avertissement dans le GO / NO GO et exige une case à cocher.
