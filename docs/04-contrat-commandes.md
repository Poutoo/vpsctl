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
```

`HostKeyMismatch` est présenté comme une alerte de sécurité, pas comme une erreur ordinaire.

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
list_dir(path: string): FileEntry[]
create_file(path: string): void
create_dir(path: string): void
rename(fromPath: string, toPath: string): void
delete_path(path: string, recursive: boolean): void
upload(localPath: string, remotePath: string, onProgress: Channel<Progress>): string    // transferId
download(remotePath: string, localPath: string, onProgress: Channel<Progress>): string  // transferId
transfer_cancel(transferId: string): void
```

Une annulation ferme le canal SFTP ; un envoi annulé laisse un fichier partiel sur le serveur, qui doit être supprimé.

Les chemins locaux sont choisis côté Vue avec `tauri-plugin-dialog`.

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

- `source` : ID complet du conteneur (64 caractères hexadécimaux) ou nom d'unité pilotable, validés comme pour les actions.
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
  path: string
  kind: "file" | "dir" | "symlink"
  size: number
  modified: number
  permissions: string
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
  | { type: "error"; message: string }
```
