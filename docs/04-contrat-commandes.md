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
  | { kind: "AuthFailed" }
  | { kind: "Network"; message: string }
  | { kind: "InvalidInput"; message: string }
  | { kind: "Remote"; exitCode: number; stderr: string }
  | { kind: "Io"; message: string }
```

`HostKeyMismatch` est présenté comme une alerte de sécurité, pas comme une erreur ordinaire.

## Connexion et profils

```ts
list_profiles(): Profile[]
save_profile(profile: Profile): Profile
delete_profile(id: string): void
set_key_passphrase(profileId: string, passphrase: string): void
trust_host_key(profileId: string, fingerprint: string): void
connect(profileId: string): ConnectionInfo
disconnect(): void
connection_status(): ConnectionInfo | null
server_info(): ServerInfo      // appelé une fois après la connexion, affiché dans la barre d'état
```

`server_info` lit `PRETTY_NAME` dans `/etc/os-release`, `docker version --format '{{.Server.Version}}'` et la première ligne de `systemctl --version`. Si une information n'est pas disponible, le champ vaut `null` : l'échec d'une de ces commandes ne doit jamais bloquer la connexion.

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
docker_list(): Container[]
docker_action(id: string, action: "start" | "stop" | "restart"): void
docker_remove(id: string): void
```

## Systemd

```ts
services_list(): Service[]
service_action(unit: string, action: "start" | "stop" | "restart"): void
service_create(def: ServiceDef): Service
service_delete(unit: string): void
```

## Logs (Docker et systemd)

```ts
type LogSource = { docker: string } | { systemd: string }
logs_start(source: LogSource, tail: number, onEvent: Channel<LogEvent>): string  // streamId
logs_stop(streamId: string): void
```

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

interface Container {
  id: string
  name: string
  image: string
  state: string
  status: string
}

interface Service {
  unit: string
  load: string
  active: string
  sub: string
  description: string
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
  command: string
  requireConfirmation: boolean
}

type LogEvent =
  | {
      type: "line"
      text: string
      timestamp: string | null          // ISO 8601
      origin: "stdout" | "stderr" | "journal"
      priority: number | null           // priorité syslog 0-7, uniquement pour origin = "journal"
    }
  | { type: "ended"; exitCode: number | null }
  | { type: "error"; message: string }

type Progress =
  | { type: "progress"; transferred: number; total: number }
  | { type: "done" }
  | { type: "cancelled" }
  | { type: "error"; message: string }
```

`ConnectionInfo` reste à préciser lors du jalon 1.
