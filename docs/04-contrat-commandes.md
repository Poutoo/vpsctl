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

type FileSide = "local" | "remote"
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
- Fichiers uniquement : un dossier, local ou distant, est refusé (`InvalidInput`).
- **Envoi** : écriture dans un temporaire `.<nom>.atlas-<8 hex>.part` du dossier de destination, renommé à la fin seulement.
  - Cible absente : `rename` du temporaire.
  - Cible existante : refusée (`AlreadyExists`) si `overwrite` vaut `false`. Avec `overwrite`, en trois étapes, pour que le contenu complet existe toujours sous un nom : la cible est renommée en `.<nom>.atlas-<hex>.old`, le temporaire prend sa place, puis le `.old` est supprimé. Si la deuxième étape échoue, le `.old` reprend sa place. Les droits du fichier remplacé sont recopiés sur le nouveau.
  - Cible dossier : `AlreadyExists` avec `isDir`. Cible lien symbolique : toujours refusée (`InvalidInput` indiquant la cible du lien) ; un lien n'est jamais remplacé par un fichier.
  - Espace libre du VPS (`statvfs@openssh.com`, si le serveur le propose) inférieur à la taille du fichier : `InsufficientSpace` avant de commencer.
- **Téléchargement** : même principe côté local, temporaire `.<nom>.atlas-<8 hex>.part` dans le dossier choisi, synchronisé sur disque puis renommé (un fichier existant est remplacé : la confirmation est celle de la fenêtre d'enregistrement du système). Un lien vers un fichier est suivi.
- **Annulation** (`transfer_cancel`) : la copie s'arrête entre deux blocs, le fichier ouvert est fermé (la session SFTP, partagée, reste ouverte), le temporaire est supprimé, puis l'événement `cancelled` est émis. La commande rend la main une fois ce nettoyage terminé, au plus tard après 5 s. Un identifiant inconnu (transfert terminé) est ignoré. La validation finale (renommages) n'est pas interruptible.
- **Erreur** pendant la copie : le temporaire est supprimé et le fichier existant reste intact, puis l'événement `error` est émis. Si la liaison est perdue, le temporaire distant ne peut pas être supprimé : il reste sur le VPS, signalé dans la liste comme résidu de transfert (voir `FileEntry.residue`).
- Les transferts en cours sont annulés à la déconnexion, au rechargement de l'interface et à la fermeture de l'application.
- La file d'attente est gérée par l'interface (un seul transfert actif à la fois) ; les chemins locaux sont choisis côté Vue avec `tauri-plugin-dialog`.

## Docker

```ts
docker_list(): ContainerList    // délai maximal 10 s
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
services_list(): Service[]      // délai maximal 10 s ; trié par nom d'unité
service_action(unit: string, action: "start" | "stop" | "restart"): void
service_create(def: ServiceDef): Service
service_delete(unit: string): void
```

Les deux listes sont des commandes indépendantes : l'interface les appelle séparément, et l'échec de l'une n'empêche pas l'affichage de l'autre. Les états d'interface (`RunState`), le code de sortie et la règle de filtrage des services sont calculés dans le backend (voir `docs/03-architecture.md`).

`service_action` (délai maximal 20 s pour l'ensemble) :

1. `unit` doit respecter `^[a-zA-Z0-9@._-]+\.service$`, sinon `InvalidInput`.
2. La liste des services est relue (mêmes commandes que `services_list`) : l'unité doit y figurer et être pilotable (`controllable`, donc non masquée), sinon `InvalidInput`.
3. `systemctl --user --no-block <start|stop|restart> -- '<unit>'` : la commande rend la main dès que la tâche est mise en file. Le résultat réel est vérifié ensuite par l'interface (voir « Vérification après une action » dans `docs/03-architecture.md`).

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
- Événements : des lots de lignes (`lines`), puis au plus un événement final. `ended` n'est émis que si la commande se termine d'elle-même (conteneur arrêté, par exemple), `error` si la liaison est perdue. Après `logs_stop`, aucun événement n'est garanti : l'interface sait qu'elle a demandé l'arrêt.
- Regroupement : un lot part toutes les 50 ms, ou dès 500 lignes. Au-delà de 5 000 lignes par seconde, les lignes en excès sont écartées et comptées dans `dropped` du lot suivant.
- Toutes les lignes sont des `LogLine` : les lignes trop longues sont tronquées à 16 Kio, l'UTF-8 invalide est remplacé.

## Scripts

```ts
scripts_list(): Script[]
scripts_save(script: Script): Script
scripts_delete(id: string): void
script_run(id: string, onEvent: Channel<LogEvent>): string  // runId
script_stop(runId: string): void
```

`script_run` réutilise `LogEvent` : un seul composant de console côté frontend pour les logs et les scripts.

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
}

interface ServiceDef {
  name: string
  description: string
  execStart: string
  workingDir: string
  restart: "no" | "on-failure" | "always"
  env: Record<string, string>
}

interface Script {
  id: string
  name: string
  description: string
  command: string
  requireConfirmation: boolean
  lastRun: { at: number; exitCode: number | null } | null   // mis à jour par le backend à la fin de chaque exécution
}

interface LogLine {
  text: string
  timestamp: string | null          // ISO 8601 en UTC, à la milliseconde : 2026-10-16T09:13:46.900Z
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
