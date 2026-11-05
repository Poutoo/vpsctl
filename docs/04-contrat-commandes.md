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

Plugin compose : `docker compose version --short`, qui affiche la version sans « v » (`cmd/compose/version.go`). Réponse lisible : `present` avec la version ; code de sortie non nul (le CLI docker sort avec le code 1 sur une commande inconnue, 127 sans docker) : `absent` ; pas de réponse ou sortie inattendue : `unknown`. Seul le code de sortie est consulté, jamais le texte du message. `absent` masque les actions de projet (voir « Docker compose ») ; `unknown` les laisse proposées, une commande sans plugin échouant dans sa console.

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

## Édition de fichiers

```ts
file_open(path: string): OpenedFile
file_save(request: SaveRequest): SaveOutcome
authorized_keys_check(path: string, before: string, after: string): AuthorizedKeysCheck
```

Tout passe par SFTP, comme l'explorateur : aucune commande shell. Les chemins suivent les règles de « Chemins et noms ».

### Ouverture

`file_open` lit l'élément avec `lstat` (un lien n'est jamais suivi), puis le fichier entier. Refus (`OpenedFile.kind = "refused"`), dans cet ordre ; le contenu d'un fichier refusé n'est jamais envoyé à l'interface :

| `OpenRefusal.reason` | Cas | Téléchargement proposé |
|---|---|---|
| `symlink` | lien symbolique (cible lue par `readlink`, affichée) | non |
| `notAFile` | dossier, socket, périphérique… | non |
| `tooLarge` | plus de 1 Mio (1 048 576 octets), d'après `lstat` puis la lecture | oui |
| `privateKey` | une ligne `-----BEGIN <MOTS EN MAJUSCULES> PRIVATE KEY-----` (espaces de bord et BOM ignorés), n'importe où dans le fichier et quel que soit son dossier | **non** : une clé privée ne doit pas transiter par l'application |
| `binary` | octet nul parmi les 8 192 premiers ; sa position (`offset`) est donnée, le format n'est pas identifié | oui |
| `invalidUtf8` | UTF-8 validé sur tout le fichier : position et ligne de la première séquence invalide | oui |

Texte (`kind = "text"`) :

- `text` : fins de ligne converties en LF (une zone de texte HTML le ferait de toute façon). `bom` : le fichier commence par `EF BB BF` (retiré du texte, remis à l'enregistrement). `lineEnding` : `lf` (aucun CR, fichier sans fin de ligne compris), `crlf` (uniquement des CRLF), `mixed` (CRLF et LF, ou un CR seul).
- `snapshot` : SHA-256 des octets bruts (BOM et CR compris), taille, date de modification. L'interface le renvoie tel quel à l'enregistrement ; elle ne calcule jamais d'empreinte.
- `readOnly` : raisons de la lecture seule, dans l'ordre d'affichage. Vide : le fichier est éditable.

| `ReadOnlyReason` | Cas |
|---|---|
| `permissions` | le compte possède le fichier mais n'a pas le droit d'écriture |
| `owner` | le fichier appartient à un autre compte (ou son propriétaire est inconnu) : le remplacer par renommage créerait un fichier au nom du compte connecté, et Atlas ne change jamais le propriétaire d'un fichier |
| `directory` | le dossier n'est pas inscriptible : le temporaire ne pourrait pas y être créé |
| `mixedLineEndings` | fins de ligne mélangées, impossibles à conserver fidèlement |

- Compte connecté, sans shell : entrée de `/etc/passwd` (lu par SFTP) au nom d'utilisateur du profil, qui donne l'uid et le groupe principal ; à défaut (annuaire LDAP…), propriétaire du dossier personnel. Les noms affichés (`root:root`) viennent de `/etc/passwd` et `/etc/group`, sinon les identifiants numériques.
- Dossier : son propriétaire n'est jugé que sur ses propres droits (règle POSIX). Pour un autre propriétaire, le dossier est inscriptible si les autres ont `wx`, ou si le groupe a `wx` : l'appartenance aux groupes secondaires est invisible sans shell, un dossier inscriptible par un groupe est donc laissé éditable. **Limite** : si le compte n'est pas dans ce groupe, l'enregistrement échoue à la création du temporaire (`PermissionDenied` sur le dossier), sans rien toucher.
- `sshFile` (chemin résolu : dossier `realpath` suivi du nom) : `authorizedKeys` pour `~/.ssh/authorized_keys` (même fichier que `FileEntry.sshAccess`), `other` pour tout autre élément sous `~/.ssh/`.

### Enregistrement

`file_save` reçoit le texte (LF), l'instantané de l'ouverture (`expected`), `bom`, `lineEnding` et les droits de l'ouverture (`mode`, utilisés seulement pour recréer un fichier supprimé). Séquence :

1. Encodage : LF → CRLF si `lineEnding = crlf`, BOM remis. Refusé (`InvalidInput`) : style `mixed`, texte avec un CR ou un caractère nul, plus de 1 Mio.
2. `lstat` de la cible. Devenue un lien symbolique ou autre chose qu'un fichier : `InvalidInput`, rien n'est écrit.
3. Relecture complète et SHA-256. Différente de `expected` (ou fichier supprimé alors qu'`expected` existe) : `SaveOutcome.outcome = "conflict"` avec la version du serveur (instantané, texte s'il est lisible, sinon la raison) ; **rien n'est écrit**. `expected = null` exige un fichier absent (recréation confirmée après une suppression sur le serveur).
4. Propriétaire et droits relus : si le fichier appartient désormais à un autre compte ou n'est plus inscriptible (changements que l'empreinte ne voit pas), `InvalidInput`, rien n'est écrit.
5. Temporaire `.<nom>.atlas-<8 hex>.part` dans le même dossier (nom des temporaires de transfert : un temporaire abandonné est signalé comme résidu dans l'explorateur), créé exclusivement, droits 0600 pendant l'écriture, écrit en entier, synchronisé sur disque (`fsync@openssh.com`, si le serveur le propose), fermé.
6. Droits du fichier remplacé appliqués au temporaire (douze bits bas). Le propriétaire n'a pas à être recopié : les autres cas sont en lecture seule.
7. Remplacement atomique par `posix-rename@openssh.com` (`rename(2)` : la cible désigne toujours l'ancien ou le nouveau contenu complet, jamais un fichier à moitié écrit ni absent). russh-sftp n'exposant pas cette extension sur la session partagée, une session SFTP brute est ouverte sur un canal le temps de la requête. **Serveur sans cette extension : l'enregistrement est refusé.**
8. Relecture : `verified` vaut `true` si le fichier relu est exactement celui écrit (« identique au serveur »).

En cas d'échec aux étapes 5 à 7, le temporaire est supprimé et la cible reste intacte. Liaison perdue pendant ce temps : le temporaire peut rester, signalé comme résidu.

**Limite, inévitable sans verrou** : une écriture faite sur le serveur entre la relecture (3) et le remplacement (7) est perdue. Le remplacement crée aussi un nouveau fichier : un processus qui garde l'ancien ouvert continue d'écrire dans celui-ci, un lien physique vers l'ancien est rompu, les ACL et attributs étendus ne sont pas recopiés.

Écraser après un conflit : l'interface renvoie le texte avec `expected` = l'instantané du serveur vu dans la fenêtre de conflit. Si le serveur a encore changé entre-temps, nouveau conflit.

### authorized_keys

`authorized_keys_check(path, before, after)` : `before` est le texte à l'ouverture, `after` celui à enregistrer ; refusé (`InvalidInput`) pour tout autre fichier que `~/.ssh/authorized_keys`. Appelé avant chaque enregistrement de ce fichier.

- Clé de la liaison : clé publique de la clé privée du profil, gardée à la connexion (jamais la partie privée). Elle est cherchée par ses données (ni le commentaire ni les options ne comptent), seulement parmi les clés de connexion (pas une autorité `cert-authority`).
- Lecture des lignes comme sshd (`auth2-pubkeyfile.c`) : lignes vides et `#` ignorées ; `type base64 [commentaire]`, sinon options (séparées par des virgules hors guillemets, valeurs entre guillemets avec `\"`) puis la clé. Clé validée par `ssh-key` (type annoncé conforme au contenu) ; certificats `*-cert-v01@openssh.com` reconnus. Une option inconnue, un type non pris en charge ou une clé illisible : ligne « non reconnue par Atlas » (jamais « ignorée par sshd »).
- `added` / `removed` : lignes reconnues comparées par type et base64 (doublons comptés), désignées par leur commentaire ou la fin de leur base64.
- `mode` : droits du fichier, conservés par l'enregistrement ; `tooOpen` si le groupe ou les autres peuvent écrire (sshd refuse alors le fichier, `StrictModes`).

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

## Docker compose

```ts
compose_check(): ProjectCheck[]         // dossier et fichiers de chaque projet, par SFTP
compose_run(project: string, action: ComposeAction, onEvent: Channel<LogEvent>): ComposeRun
compose_stop(runId: string): void
compose_down_info(project: string): DownInfo   // contenu du GO / NO GO de down, délai 20 s après la liste
compose_forget(project: string): void   // projet descendu uniquement ; rien n'est touché sur le serveur
```

### Projets

Un projet compose n'existe sur le serveur que par les étiquettes de ses conteneurs. `docker_list` les lit **dans le même appel** que la liste : un objet JSON par conteneur, qui reprend la ligne de `{{json .}}` et lit à part chaque étiquette avec `.Label`, encodée par la fonction `json` du gabarit (`templates.go` du CLI docker) :

```sh
docker ps -a --no-trunc --format '{"ps":{{json .}},"project":{{json (.Label "com.docker.compose.project")}},"workingDir":{{json (.Label "com.docker.compose.project.working_dir")}},"configFiles":{{json (.Label "com.docker.compose.project.config_files")}},"envFiles":{{json (.Label "com.docker.compose.project.environment_file")}},"service":{{json (.Label "com.docker.compose.service")}},"oneoff":{{json (.Label "com.docker.compose.oneoff")}}}'
```

Le champ `Labels` de `{{json .}}` ne convient pas : les étiquettes y sont séparées par des virgules, et celle des fichiers de configuration en contient elle-même. Une étiquette absente vaut `""`.

| Étiquette | Usage |
|---|---|
| `com.docker.compose.project` | nom du groupe, `-p` |
| `com.docker.compose.project.working_dir` | dossier, `--project-directory` |
| `com.docker.compose.project.config_files` | fichiers de configuration, `-f` dans l'ordre de l'étiquette |
| `com.docker.compose.project.environment_file` | fichiers d'environnement, `--env-file` ; posée seulement avec `--env-file` (ou `COMPOSE_ENV_FILES`), jamais pour le `.env` par défaut du dossier, que compose relit de lui-même |
| `com.docker.compose.service` | nom court du service |
| `com.docker.compose.oneoff` | `True` pour un conteneur de `docker compose run` |

Code source de compose (v5, `internal/projectloader/loader.go`) : `ConfigFilesLabel` et `EnvironmentFileLabel` valent `strings.Join(…, ",")`, chemins absolus.

Validation (fonctions pures, `compose/labels.rs`). Les valeurs sont des données externes : validées ici, puis échappées avant d'entrer dans une commande.

- Nom : `^[a-z0-9][a-z0-9_-]*$` (règle de compose), 255 octets au plus. Sinon `invalidName` : le groupe est affiché, aucune commande possible.
- Dossier et chaque fichier : chemin absolu, sans composant vide, `.` ou `..`, sans caractère de contrôle, 4 096 octets au plus. Sinon `invalidPath`.
- Listes séparées par des virgules : un fragment qui n'est pas absolu (ou vide) révèle une virgule dans un chemin, la liste est refusée (`ambiguousPath`). Limite : `/a,/b.yml`, où `/a` et `/b.yml` sont deux fichiers existants, ne se distingue pas d'un chemin `/a,/b.yml`.
- Étiquette du dossier ou des fichiers absente : `incomplete`. Conteneurs du même projet aux étiquettes différentes : `divergent`.
- Les étiquettes de référence sont celles des conteneurs de service ; un projet qui n'a que des conteneurs `compose run` prend les leurs.
- Un projet aux étiquettes inutilisables est affiché, avec la raison, mais jamais mémorisé.

### État agrégé

Calculé par le backend sur les conteneurs du projet, conteneurs `compose run` exclus (affichés dans le groupe sans être comptés). `running` compte les conteneurs `running`, `total` les autres.

| État | Règle, par priorité |
|---|---|
| `failed` (En échec) | au moins un conteneur en échec (code de sortie autre que 0, 137 ou 143, `dead`, tué par manque de mémoire même sorti en 137), ou en boucle de redémarrage : `restarting` avec un dernier code non nul ou illisible, ou `running` depuis moins de 10 s après un redémarrage par sa politique (`RestartCount` non nul, lu par `docker inspect`) |
| `transition` (En transition) | au moins un conteneur `removing`, ou `restarting` avec le code 0 |
| `nominal` | tous en marche |
| `stopped` (Arrêté · 0/N) | aucun en marche, tous arrêtés proprement |
| `partial` (Partiel · n/N) | les autres cas : au moins un en marche et au moins un arrêté proprement |
| `down` (Descendu · 0/N) | aucun conteneur ; projet connu par la mémoire du profil, N mémorisé |

### Vérification du dossier et des fichiers

`compose_check` relit la liste (mêmes commandes que `docker_list`, délai de 30 s) puis vérifie, **par SFTP** et sans aucune commande shell, chaque projet dont le dossier et les fichiers sont connus (étiquettes valides ou mémoire), un projet après l'autre :

1. le dossier : `stat` (liens suivis) ; absent → `dirMissing`, autre chose qu'un dossier → `notDir` ;
2. chaque fichier de configuration puis d'environnement, dans l'ordre : `stat` (absent → `missing`, autre chose qu'un fichier ordinaire → `notFile`), puis ouverture en lecture, refermée aussitôt sans rien lire (refusée → `unreadable`). Un dossier s'ouvre en lecture sur Linux : le `stat` le rattrape avant.

Le premier problème trouvé est renvoyé. Accès refusé au `stat` (dossier parent non traversable) → `unreadable`. Une autre erreur du serveur → `unchecked` (ne bloque rien) ; une erreur de transport fait échouer la commande.

- Fréquence : toutes les 30 s (une liste sur six) tant que le tableau de bord est affiché et la fenêtre visible, à l'apparition d'un projet non encore vérifié, sur « Actualiser », et après chaque commande compose. Jamais à chaque rafraîchissement de la liste. Pendant un LOS, aucune vérification ; la dernière reste affichée.
- Démarrer et Récupérer les images refont la vérification avant de partir et sont refusés si elle échoue (voir « Commandes »).
- Affichage : en-tête teinté, « dossier introuvable », « fichier introuvable », « fichier illisible »… dans la colonne Détail, chemin du dossier barré s'il manque, et une ligne d'explication sous l'en-tête.

### Commandes

Actions de la maquette uniquement. Jamais d'option qui supprime des volumes ou des images (`-v`, `--volumes`, `--rmi`), ni `--remove-orphans`.

| Action | Interface | Arguments |
|---|---|---|
| `up` | Démarrer | `up -d` |
| `pull` | Récupérer les images | `pull` |
| `restart` | Redémarrer | `restart` |
| `down` | Arrêter et supprimer (GO / NO GO) | `down` |

Construction (fonction pure `compose/command.rs`), chaque valeur validée (règles des étiquettes) puis échappée par `shell::quote` :

```sh
# Démarrer, Récupérer les images
docker compose --ansi never --progress plain -p '<projet>' --project-directory '<dossier>' -f '<fichier 1>' -f '<fichier 2>' [--env-file '<fichier>'…] up -d | pull
# Redémarrer, Arrêter et supprimer
docker compose --ansi never --progress plain -p '<projet>' restart | down
```

- Fichiers de configuration dans l'ordre de l'étiquette ; `--env-file` seulement si l'étiquette `environment_file` existe (sinon compose relit le `.env` du dossier, grâce à `--project-directory`).
- Sortie en texte simple : `--ansi never` choisit déjà le rendu `plain` de compose (`selectEventProcessor`, v5.3.1) ; `--progress plain` l'impose même si `COMPOSE_PROGRESS` vaut `tty` sur le serveur. Le rendu `plain` écrit une ligne par événement, sans déplacement du curseur (`cmd/display/plain.go`) ; les séquences ANSI restantes sont retirées comme pour toute ligne. Les deux options sont des options globales de `docker compose` (v5.3.1 vérifiée). compose écrit sa progression sur la sortie d'erreur.
- **Redémarrer et Arrêter passent toujours par le seul nom**, même quand les fichiers sont présents : avec `-f`, un service retiré du fichier depuis le dernier `up` laisserait un conteneur orphelin, puisque `--remove-orphans` est exclu. Sans `-f` ni `--project-directory`, `projectOrName` (`cmd/compose/compose.go`) ne charge aucun fichier : `restart` et `down` relisent le projet depuis les conteneurs, réseaux et volumes étiquetés (`projectOrFromResources`). Ils fonctionnent donc aussi quand le dossier a disparu.
- Limite : les profils compose (`--profile`) ne sont enregistrés dans aucune étiquette. Démarrer ne lance que les services sans profil ; un service lancé avec un profil n'est pas recréé par Atlas.

`compose_run` (délai de 30 s pour la préparation, puis 10 s pour ouvrir le canal ; la commande elle-même n'a pas d'échéance) :

1. Nom validé, liste relue (mêmes commandes que `docker_list`) : le projet doit y figurer (conteneurs ou mémoire), sinon `InvalidInput`. Le frontend ne transmet que le nom et l'action, jamais un chemin.
2. Démarrer et Récupérer : étiquettes utilisables, puis vérification SFTP du dossier et des fichiers (voir ci-dessus) ; un dossier ou un fichier manquant, qui n'est pas un fichier, ou illisible → `InvalidInput` avec la raison. Redémarrer et Arrêter : au moins un conteneur, sinon `InvalidInput`.
3. Commande enveloppée dans le script de surveillance et lancée par le registre des flux, comme un script : sortie horodatée à la réception, lots, ANSI retiré, `ended` avec le code de sortie. **Seul le code de sortie juge le résultat** : compose écrit toute sa progression sur la sortie d'erreur.
4. Une seule commande à la fois par projet (`InvalidInput` : « une commande compose est déjà en cours sur ce projet… »). Les commandes compose comptent dans la limite de 4 flux, avec les logs et les scripts (« 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer cette commande ») : un quota séparé dépasserait les 10 canaux d'OpenSSH.

```ts
interface ComposeRun {
  runId: string
  command: string   // commande exacte, sans le script de surveillance
}
```

- `compose_stop` : comme `script_stop` (TERM au groupe de processus, 3 s au plus ; identifiant inconnu ignoré). Ce qui est déjà fait n'est pas annulé : un `up` interrompu peut laisser le projet à moitié démarré. L'interface demande un GO / NO GO.
- Liaison perdue pendant une commande : même traitement que les scripts, aucun événement final (`link-lost` en tient lieu), aucune relance automatique. Le script de surveillance termine la commande quand sshd ferme le canal : l'issue est inconnue.

`compose_down_info` : liste relue (au moins un conteneur, sinon `InvalidInput`), puis, dans un délai de 20 s :

```sh
docker inspect --type container --format '{{json .Mounts}}' -- '<id>' '<id>'…
docker network ls --filter 'label=com.docker.compose.project=<projet>' --format '{{.Name}}'
docker volume ls --filter label=com.docker.volume.anonymous --format '{{.Name}}'
```

- Montages (`MountPoint` de l'API Engine) : `volume` → nommé ou anonyme ; `bind` → dossier du serveur (`Source`) ; les autres (`tmpfs`…) ne survivent pas au conteneur. Le code de retour d'`inspect` n'est pas consulté (un conteneur supprimé entre-temps ne masque pas les autres).
- Volumes anonymes : un volume est anonyme s'il porte l'étiquette `com.docker.volume.anonymous` (Docker la pose à la création, `AnonymousLabel`, `daemon/volume/service/service.go` de moby, jamais aux volumes nommés), ou à défaut si son nom fait 64 caractères hexadécimaux, la forme des volumes sans nom (volume créé par une version de Docker antérieure à l'étiquette, par exemple). Le doute va toujours vers « anonyme » : le GO / NO GO ne présente jamais comme rattaché un volume que le `down` détachera. Fonction pure `is_anonymous_volume`.
- Les fichiers du projet sont vérifiés (relance possible ou non par Démarrer).
- Sans `-v`, `down` conserve les volumes nommés **et** anonymes (documentation de `docker compose down`) ; un volume anonyme n'est simplement plus monté par le `up` suivant.

### Mémorisation

- Fichier `compose-projects.json` du dossier de configuration, par identifiant de profil : `{ "version": 1, "profiles": { "<id>": [ { "name", "workingDir", "configFiles", "envFiles", "containers" } ] } }`. Rien n'est écrit sur le serveur.
- À chaque `docker_list`, les projets aux étiquettes valides et avec au moins un conteneur sont ajoutés ou mis à jour (`containers` : leur nombre). Écriture atomique, uniquement si quelque chose change ; un échec d'écriture n'empêche pas l'affichage.
- Un projet mémorisé sans aucun conteneur est renvoyé « Descendu », avec ses fichiers mémorisés.
- Lecture tolérante au démarrage : fichier absent ou invalide → mémoire vide ; entrée invalide (mêmes règles que les étiquettes) ignorée sans toucher aux autres.
- `compose_forget` : la liste est relue ; un projet qui a encore des conteneurs est refusé (`InvalidInput`, il réapparaîtrait aussitôt), un projet inconnu aussi. L'interface demande un GO / NO GO simple.
- Supprimer un profil oublie ses projets.

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
- 4 flux simultanés au maximum (logs, scripts et commandes compose confondus) ; au-delà, `InvalidInput`. OpenSSH limite une connexion à 10 canaux par défaut (`MaxSessions`) : il en reste pour la session SFTP, la mesure de latence et les listes.
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
- Limite de 4 flux simultanés, partagée avec les logs et les commandes compose. Au-delà, `InvalidInput` avec le message « 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer ce script ».
- Un script déjà en cours d'exécution ne peut pas être relancé avant la fin de la première exécution (`InvalidInput`).
- Événements : comme les logs (lots de `lines`, puis `ended` avec le code de sortie, ou `error`). Origine `stdout` / `stderr` ; `timestamp` est l'heure de réception par le backend (le programme n'en fournit pas). `exitCode` vaut `null` si la commande a été tuée par un signal.
- `script_stop` : comme `logs_stop` (rend la main une fois le processus distant arrêté, 3 s au plus ; identifiant inconnu ignoré).
- `lastRun` est enregistré à la fin de chaque exécution : `at` est l'heure de **début** de l'exécution, `exitCode` le code de sortie de la fin naturelle, ou `null` pour une exécution interrompue (`script_stop`, déconnexion, liaison perdue, rechargement de l'interface, fermeture de l'application) ou tuée par un signal.

## Ressources du serveur

Maquettes : `docs/design/planche VII/`. Rien n'est installé ni stocké sur le serveur : les mesures viennent de fichiers du noyau lus par SFTP et de commandes `docker` fixes (aucune valeur de l'interface n'y entre).

```ts
resources_watch(foreground: boolean): void     // écran Ressources affiché et fenêtre visible
resources_state(): ResourceUpdate | null       // dernier relevé de la session (rechargement de la page)
resource_thresholds_get(profileId: string): Thresholds
resource_thresholds_save(profileId: string, thresholds: Thresholds): Thresholds
resource_thresholds_reset(profileId: string): Thresholds    // « Valeurs par défaut » : ce profil seulement
docker_analyze(kind: AnalysisKind, onEvent: Channel<AnalysisEvent>): string   // runId
docker_analyze_stop(runId: string): void
// Événement `resources-update` : ResourceUpdate, à chaque relevé et à chaque changement de seuils.
```

### Mesures

| Mesure | Source | Voie | Calcul |
|---|---|---|---|
| CPU | `/proc/stat`, ligne `cpu` | SFTP | Par différence entre deux relevés : `1 − Δ(idle + iowait) / Δtotal` (`steal` compris dans le total). Cœurs : lignes `cpuN`. |
| Mémoire | `/proc/meminfo` | SFTP | Utilisée = `MemTotal − MemAvailable` (jamais `MemFree` : le cache est récupérable). Swap = `SwapTotal − SwapFree`. |
| Charge | `/proc/loadavg` | SFTP | 1, 5 et 15 min, divisées par le nombre de cœurs. La moyenne sur 5 min par cœur sert pour la valeur, l'alerte, le seuil et la courbe. |
| Disque | `/proc/mounts`, puis `statvfs` de chaque point de montage | SFTP (`statvfs@openssh.com`, `SftpSession::fs_info`) | Voir « Points de montage ». |
| Réseau | `/proc/net/dev` | SFTP | Par différence : octets reçus et émis par seconde, sommés sur les interfaces comptées (toutes sauf `lo`, `docker0`, `veth*` et `br-*`). Pas de choix des interfaces. |
| Conteneurs | `docker stats --no-stream --no-trunc --format '{{json .}}'` | exec, 25 s au plus | Voir « Conteneurs ». |
| Racine Docker | `docker info --format '{{.DockerRootDir}}'` | exec, une fois par liaison | Chemin résolu par `realpath` SFTP. |
| Répartition Docker | `docker system df --format '{{json .}}'` | exec (60 s au plus) ou flux, voir « Cause probable du disque » | |
| Volumes | `docker system df -v --format '{{json .}}'` | flux | puis `docker ps -a --no-trunc --format '{{.Names}}\t{{.Mounts}}'` (exec) pour rattacher chaque volume à ses conteneurs. |

- Fichiers de `/proc` : lus jusqu'à la fin (le serveur SFTP les lit malgré leur taille annoncée de 0), 1 Mio au plus. Une seule session SFTP, celle de la liaison : aucun canal ouvert pour ces lectures.
- Mesure impossible (fichier illisible, champ manquant, commande en échec, délai dépassé) : `unavailable` avec sa raison ; les autres mesures du relevé ne sont pas touchées.
- Mesures par différence (CPU, réseau) : `pending` au premier relevé, et quand le relevé précédent a plus de 40 s (retour sur l'écran ; un relevé lent dure 25 s au plus, suivi de 5 s d'attente) ou qu'un compteur a reculé (redémarrage du serveur). Elles ne sont pas relevées hors de l'écran.
- Swap : `null` si `SwapTotal` ou `SwapFree` manque (« non mesurable ») ; `total` à 0 sans swap.

#### Points de montage

Fonctions pures `parse_mounts`, `keep_mounts` :

- Échappements octaux de `/proc/mounts` décodés (`\040` pour une espace).
- Exclus : types `tmpfs`, `devtmpfs`, `overlay`, `squashfs`, `proc`, `sysfs`, `cgroup`, `cgroup2`, `devpts`, `mqueue`, `debugfs`, `tracefs`, `securityfs`, `pstore`, `bpf`, `configfs`, `fusectl`, `hugetlbfs`, `autofs`, `binfmt_misc`, `rpc_pipefs`, `nsfs`, `ramfs`, `efivarfs`, `fuse.lxcfs`, `fuse.portal`, `nfsd`.
- Doublons d'un même périphérique (montage lié, sous-volume) : le point de montage le plus court est gardé.
- Un point de montage qui n'est pas un dossier (fichier monté par un montage lié, `/etc/hosts` dans un conteneur) est écarté, d'après un `stat` SFTP.
- Occupation comme `df` : `utilisé = (blocks − bfree) × frsize`, `pourcentage = utilisé / (utilisé + bavail × frsize)` ; espace libre affiché `bavail × frsize` (ce qu'un compte non root peut écrire), taille `blocks × frsize`. `statvfs` refusé ou en échec : la ligne passe `unavailable`, les autres restent.

#### Conteneurs

- Uniquement quand l'écran est affiché (relevé d'avant-plan) : c'est la mesure la plus coûteuse. `--no-stream` attend deux échantillons du daemon (1 à 2 s) puis rend la main.
- Ligne JSON par conteneur en marche : `ID` (64 caractères hexadécimaux, sinon ligne écartée), `Name`, `CPUPerc` (`"4.80%"`, en % d'un cœur), `MemUsage` (`"1.2GiB / 3GiB"`, unités binaires de `go-units`). Triés par mémoire décroissante.
- Sans limite : Docker donne alors la mémoire du serveur comme limite. Une limite d'au moins 99 % de `MemTotal` est affichée « sans limite » (pas de `docker inspect` supplémentaire).
- Erreurs : comme `docker_list` (`DockerNotInstalled`, `DockerPermissionDenied`), traduites en raison de `unavailable`.

### Cadences

Une tâche par liaison (`resources::monitor`), lancée à l'établissement, arrêtée à la perte et à la déconnexion :

- **Avant-plan** (`resources_watch(true)` : écran affiché et fenêtre ni masquée ni réduite) : relevé complet, le suivant **5 s après la fin** du précédent.
- **Arrière-plan** : mémoire et disque (`/proc/meminfo`, `/proc/mounts`, `statvfs`) toutes les 60 s, par SFTP uniquement : aucun canal, aucune commande `docker`.
- **Jamais deux relevés à la fois** : une seule boucle, relevé puis attente. Passer en avant-plan pendant l'attente lance un relevé tout de suite ; pendant un relevé, le suivant part dès la fin. Chaque mesure a une échéance de 25 s : ce qui n'a pas répondu passe `unavailable` (« délai dépassé »). 25 s dépasse la détection d'une perte silencieuse (20 s au plus, voir « Perte de la liaison ») : sur une liaison coupée, la perte est constatée avant, la tâche est abandonnée et rien n'est publié ; l'interface garde les valeurs figées.
- **Canaux** (OpenSSH : `MaxSessions` 10) : un relevé n'ouvre qu'un canal exec à la fois (`docker stats`), plus au plus un pour la répartition automatique. L'analyse Docker passe par le registre des flux et compte dans la limite de 4. En pointe : SFTP 1, SFTP brute 1 (éditeur), latence 1, flux 4, relevé 2 = 9 ; les listes du tableau de bord ne tournent pas quand l'écran Ressources est affiché.

### Seuils

```ts
interface Thresholds { disk: ThresholdPair; memory: ThresholdPair; load: ThresholdPair }
interface ThresholdPair { attention: number; critical: number }
```

- Par défaut : disque 80 / 90 %, mémoire 85 / 95 %, charge 1,0 / 2,0 par cœur.
- Fichier `resource-thresholds.json` du dossier de configuration, une entrée par profil (`{ "version": 1, "profiles": { "<id>": Thresholds } }`). Lecture tolérante (fichier ou entrée invalide : valeurs par défaut), écriture atomique avant la mise à jour en mémoire, rien d'écrit si rien ne change. Entrée retirée à la suppression du profil et par `resource_thresholds_reset`.
- Validation (`InvalidInput`, cas partagés `tests/shared/resource-thresholds.json`) : disque et mémoire entiers de 1 à 100 ; charge de 0,1 à 64 au dixième ; `attention < critical`.
- Le profil est passé par l'interface : les seuils restent modifiables pendant un LOS. Ceux du profil de la liaison s'appliquent aussitôt : les niveaux sont recalculés sur le dernier relevé et `resources-update` est émis, sans nouveau relevé.

### Alertes

```ts
type Level = "nominal" | "attention" | "critical"
interface Levels { disks: MountLevel[]; memory: Level; load: Level }
interface MountLevel { mountPoint: string; level: Level }
```

- Hystérésis (fonction pure `next_level`) : le niveau monte dès que la valeur atteint un seuil ; il ne redescend que lorsque la valeur passe à `seuil − marge` ou en dessous. Marge : 3 points pour le disque et la mémoire, 0,1 par cœur pour la charge. Les valeurs comparées sont arrondies comme à l'affichage (pourcentage entier, charge par cœur au centième) : une alerte à 80 % se lève quand l'écran affiche 80 %.
- Un niveau par point de montage, un pour la mémoire, un pour la charge. Une mesure absente (`pending`, `unavailable`, `notRead`) garde le niveau précédent. Niveaux gardés pendant un LOS ; vidés à la déconnexion et au changement de profil.
- Bandeaux par ordre de priorité : disque, mémoire, charge. Repère de navigation : le pire niveau du disque et de la mémoire, plus la charge quand l'écran est affiché (hors de l'écran, elle n'est pas relevée).
- Uniquement dans l'application : aucune notification système.

### Cause probable du disque

- Seulement pour le point de montage qui contient la racine Docker (préfixe le plus long, composant par composant, après `realpath`) : `dockerMount`. Docker absent ou injoignable : aucune cause. L'alerte d'un autre point de montage n'affiche rien et ne renvoie jamais vers un conteneur.
- `DOCKER_USAGE_AUTOMATIC` (`resources/mod.rs`) choisit le déclenchement de la répartition par catégorie (`docker system df`, sans `-v`) :
  - `true` : lancée par le relevé d'avant-plan quand le point de montage de Docker est en attention ou critique, au plus une fois toutes les **5 minutes** (relancée en arrivant sur l'écran si elle a plus de 5 minutes), en parallèle du relevé, 60 s au plus. Résultat dans `ResourceUpdate.dockerUsage` ;
  - `false` : rien d'automatique ; l'interface propose « Analyser l'espace Docker » (`docker_analyze("usage")`).

  Règle de choix : si `docker system df` coûte presque autant que `docker system df -v` (mesure de `docs/tests-jalon-13.md`, section 0), `false`.
- `docker_analyze(kind)` : `"volumes"` (`docker system df -v`) ou `"usage"` (`docker system df`), à la demande, sans délai fixe. Par le registre des flux (script de surveillance, limite de 4 flux, refus « 4 flux déjà ouverts : fermez un onglet de télémétrie pour lancer l'analyse »), une seule analyse à la fois (`InvalidInput`). La sortie JSON tient sur une ligne : elle est lue sans la limite de 16 Kio des consoles (32 Mio au plus) puis analysée à la fin.
- `docker_analyze_stop` : comme `logs_stop` (TERM au groupe de processus, puis fermeture du canal, 3 s au plus). **Limite** : le processus `docker` s'arrête, mais `dockerd` peut finir son parcours des volumes après l'arrêt de la commande (la requête de l'API n'interrompt pas forcément le calcul des tailles).
- Liaison perdue : aucun événement, `link-lost` en tient lieu ; l'analyse est à relancer.

```ts
type AnalysisKind = "usage" | "volumes"
type AnalysisEvent =
  | { type: "done"; result: AnalysisResult }
  | { type: "failed"; message: string }
type AnalysisResult =
  | { kind: "usage"; usage: DockerUsage }
  | { kind: "volumes"; volumes: VolumeAnalysis }
interface DockerUsage { at: number; images: number; containers: number; volumes: number; buildCache: number }   // octets
interface VolumeAnalysis { at: number; durationMs: number; volumes: VolumeSize[] }   // triés par taille décroissante
interface VolumeSize { name: string; size: number | null; containers: string[] }
```

Tailles de `docker system df` : unités décimales de `go-units` (`"14.2GB"`, `"0B"`, `"1.5kB"`), converties en octets.

### Relevé

```ts
type Measure<T> =
  | { status: "ok"; value: T }
  | { status: "pending" }                      // mesure par différence : premier relevé
  | { status: "unavailable"; reason: string }
  | { status: "notRead" }                      // non relevée hors de l'écran

interface ResourceUpdate {
  profileId: string
  at: number               // fin du relevé (ms epoch)
  foreground: boolean      // relevé complet
  cores: number | null
  cpu: Measure<number>     // % de 0 à 100
  memory: Measure<MemoryUsage>
  load: Measure<LoadAverage>
  network: Measure<NetworkRates>
  disks: Measure<DiskUsage[]>
  containers: Measure<ContainerUsage[]>
  dockerMount: string | null
  dockerUsage: DockerUsage | null
  automaticDockerUsage: boolean   // DOCKER_USAGE_AUTOMATIC
  levels: Levels
  thresholds: Thresholds   // du profil de la liaison
}
interface MemoryUsage { total: number; used: number; percent: number; swap: { total: number; used: number } | null }
interface LoadAverage { raw: [number, number, number]; perCore: [number, number, number] }
interface NetworkRates { interfaces: string[]; received: number; sent: number }   // octets par seconde
interface DiskUsage { mountPoint: string; device: string; fsType: string; space: Measure<DiskSpace> }
interface DiskSpace { total: number; free: number; percent: number }
interface ContainerUsage { id: string; name: string; cpuPercent: number; memoryUsed: number; memoryLimit: number | null }
```

Historique : 30 minutes dans le store `resources` de l'interface, séries bornées, perdues à la fermeture, à la déconnexion et au changement de profil (voir `docs/03-architecture.md`).

## Préférences

```ts
preferences_get(): Preferences
preferences_set_dimming(dimming: number): Preferences        // 0 à 100, sinon InvalidInput
preferences_set_original_colors(enabled: boolean): Preferences
```

- Fichier `preferences.json` du dossier de configuration, séparé des profils, écrit atomiquement (temporaire puis renommage) à chaque modification ; la valeur en mémoire ne change qu'après une écriture réussie (sinon `Io`).
- Lecture tolérante, une seule fois au démarrage : un fichier absent, illisible ou qui n'est pas un objet JSON donne les valeurs par défaut ; un champ manquant, d'un autre type ou hors limites reprend sa valeur par défaut sans toucher aux autres ; les champs inconnus et `version` sont ignorés. Le premier enregistrement remplace un fichier invalide.
- Format écrit : `{ "version": 1, "dimming": 85, "originalColors": false, "background": null, "discordPresence": false }` ; `background` reprend `BackgroundInfo` pour une image personnalisée.

### Image de fond

```ts
background_choose(): BackgroundChoice           // fenêtre de sélection du système, puis contrôles
background_pending_image(token: string): ArrayBuffer   // octets bruts de l'image en attente
background_commit(token: string): Preferences   // après décodage réussi par la webview
background_discard(token: string): void         // décodage refusé : l'image en attente est oubliée
background_image(): ArrayBuffer                 // octets bruts de la copie enregistrée ; InvalidInput sans image personnalisée
background_reset(): Preferences                 // fond d'origine, copie supprimée
background_lost_at_startup(): boolean           // vrai une seule fois si la copie a été écartée au démarrage
```

- **Choix** : la fenêtre de sélection est ouverte par le backend (plugin dialog, API Rust), avec un filtre « Images PNG, JPEG ou WebP » et un filtre « Tous les fichiers » (un fichier renommé doit pouvoir être choisi pour être refusé). Le chemin ne quitte jamais le backend et n'est pas conservé : seul le nom du fichier l'est (dernier composant, caractères de contrôle retirés, 128 caractères au plus), pour l'affichage.
- **Contrôles**, dans cet ordre, chacun donnant un refus (`Refusal`) qui laisse le fond actuel en place :
  1. taille du fichier au plus 10 Mo (10 × 1024 × 1024 octets), vérifiée avant la lecture, puis lecture bornée à cette taille → `tooLarge` ;
  2. format sur les premiers octets, jamais sur l'extension : PNG (`89 50 4E 47 0D 0A 1A 0A`), JPEG (`FF D8 FF`), WebP (`RIFF` … `WEBP`) → `format` ;
  3. dimensions lues dans l'en-tête (analyseur maison, `background.rs`) : bloc `IHDR` du PNG ; premier marqueur SOFn du JPEG, octets de bourrage acceptés ; bloc `VP8 ` (code de synchronisation vérifié, bits d'échelle ignorés), `VP8L` (signature `0x2F`) ou `VP8X` du WebP. En-tête incohérent ou dimension nulle → `unreadable` ;
  4. fin du fichier présente → `unreadable` : blocs du PNG entiers jusqu'au bloc `IEND` (parcours depuis la signature ; les données placées après `IEND` sont tolérées) ; marqueur `FF D9` après le marqueur SOFn du JPEG (celui d'une vignette EXIF, placée avant, ne compte pas) ; taille annoncée par l'en-tête RIFF du WebP disponible. Le moteur web décode sans erreur une partie d'un fichier tronqué : ce contrôle le rattrape ;
  5. 8 192 px de côté et 40 mégapixels au plus → `dimensions`. Une image compressée mais énorme saturerait la mémoire une fois décodée (40 Mpx ≈ 160 Mo en RGBA).
- **Décodage** : le backend ne décode pas l'image. Les contrôles réussis, `background_choose` garde les octets en attente (une seule image à la fois, désignée par `token`) ; l'interface les obtient par `background_pending_image`, les décode avec `img.decode()` puis appelle `background_commit`, ou `background_discard` et affiche le refus `unreadable`. Un `token` inconnu ou remplacé par un choix plus récent → `InvalidInput`.
- **Copie** : `background_commit` écrit les octets dans `background` (dossier de configuration, sans extension : le format est relu sur le contenu) par un temporaire renommé, puis enregistre `BackgroundInfo` dans les préférences. `background_reset` enregistre `background: null` puis supprime la copie.
- **Démarrage** : une copie absente, trop lourde, d'un format non reconnu ou dont l'en-tête est illisible est écartée (préférence remise à `null`, copie supprimée) et `background_lost_at_startup` le signale une fois. Format et dimensions sont relus sur la copie. L'interface décode ensuite la copie ; si le moteur web la refuse, elle appelle `background_reset` et affiche le même message.
- Les octets passent en binaire (`tauri::ipc::Response`), sans JSON. L'interface les affiche par une URL `blob:`, seule source d'image ajoutée à la CSP (voir `docs/03-architecture.md`).

## Présence Discord

```ts
presence_status(): PresenceStatus
presence_set_enabled(enabled: boolean): void   // enregistre discordPresence puis l'applique ; Io si l'enregistrement échoue
presence_retry(): void                         // nouvelle tentative immédiate (bouton « Réessayer »)
// Événement `presence-status` : PresenceStatus, à chaque changement d'état ou de phase.
```

- **Disponibilité** : uniquement dans un build release lancé hors `tauri dev` (`!tauri::is_dev() && !cfg!(debug_assertions)`), sinon `devUnavailable` ; avec un identifiant d'application valide (`presence::CLIENT_ID`, 17 à 20 chiffres), sinon `notConfigured`. Dans ces deux cas, aucune connexion à Discord n'est jamais tentée.
- **Désactivée par défaut** (`discordPresence: false`). Interrupteur éteint : aucune connexion, rien n'est envoyé.
- **Connexion locale uniquement** : tube nommé `\\?\pipe\discord-ipc-N` sous Windows ; socket Unix `discord-ipc-N` sous Linux, dans `$XDG_RUNTIME_DIR`, `$TMPDIR`, `$TMP`, `$TEMP` puis `/tmp`, et dans leurs sous-dossiers `app/com.discordapp.Discord/` (Flatpak) et `snap.discord/` (Snap), N de 0 à 9. Les dossiers de base viennent de la documentation RPC de Discord ; les sous-dossiers Flatpak et Snap sont ceux des crates `discord-rich-presence` et `discord-presence`, qui concordent.
- **Protocole** (implémentation maison, `presence/`) : trames `opcode` (u32 LE), longueur (u32 LE), JSON. Poignée de main `{"v": 1, "client_id": …}`, attente de l'événement READY (5 s au plus), puis `SET_ACTIVITY` avec `pid` (Discord efface la carte si le processus disparaît). Les PING reçoivent un PONG ; une trame CLOSE ou une lecture en erreur signale Discord fermé.
- **Contenu** : `{"details": "En liaison" | "Au sol", "timestamps": {"start": <lancement d'Atlas, secondes>}, "assets": {"large_image": "papillon", "large_text": "Atlas · VPS Control"}, "buttons": [{"label": "Source Code", "url": "https://github.com/Poutoo/vpsctl"}]}`. Le bouton (libellé de 32 caractères au plus, URL de 512 au plus) est visible des autres utilisateurs ; Discord ne le montre pas à soi-même. Le nom affiché en titre de la carte est celui de l'application sur le portail Discord. « En liaison » : de l'établissement de la liaison (AOS) à sa perte ou à la déconnexion ; « Au sol » : tous les autres cas, connexion en cours et LOS compris. La durée part du lancement d'Atlas et ne repart jamais de zéro : le même départ accompagne chaque mise à jour, que l'état passe d'« En liaison » à « Au sol » ou l'inverse. La phase vient de `AppState::link_phase`. La fonction qui construit l'activité ne reçoit que la phase et son début : aucune donnée d'un profil, d'un serveur, d'un fichier ou d'une commande ne peut y entrer (test `no_profile_file_or_command_data_reaches_discord`).
- **Discord absent ou fermé** : état `searching`, nouvelle tentative toutes les 15 s tant que l'interrupteur est actif, ou tout de suite par `presence_retry`. Aucune erreur n'est remontée ni notifiée.
- **Limite de fréquence** : 5 mises à jour d'activité par 20 secondes (documentation de Discord : Game SDK, `UpdateActivity` ; objet Activity des événements du gateway). Fenêtre glissante comptée sur tous les envois, effacements compris ; une phase qui change plus vite est regroupée et seule la dernière part.
- **Effacement** : `SET_ACTIVITY` avec `activity: null` (la documentation RPC ne décrit pas l'effacement ; c'est la forme employée par les deux crates citées), puis trame CLOSE. À la désactivation (dans la limite de fréquence) et à la fermeture de l'application (aussitôt, en 1 s au plus).

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
  compose: ComposeSupport
}

type ComposeSupport =
  | { status: "present"; version: string }   // ex. "5.3.1"
  | { status: "absent" }
  | { status: "unknown" }

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
  projects: ComposeProject[] // triés par nom, descendus compris (voir « Docker compose »)
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
  project: string | null    // étiquette com.docker.compose.project, brute ; null si absente
  service: string | null    // étiquette com.docker.compose.service, brute
  oneoff: boolean           // conteneur de docker compose run
}

interface ComposeProject {
  name: string
  files: ProjectFiles | null       // null si les étiquettes sont inutilisables
  problem: ProjectProblem | null
  state: ProjectState
  running: number                  // conteneurs en marche, compose run exclus
  total: number                    // conteneurs, compose run exclus ; projet descendu : nombre mémorisé
}

interface ProjectCheck {
  project: string
  status: FilesStatus
}

type FilesStatus =
  | { kind: "ok" }
  | { kind: "dirMissing" }
  | { kind: "notDir" }
  | { kind: "missing"; path: string }
  | { kind: "notFile"; path: string }
  | { kind: "unreadable"; path: string }     // illisible pour le compte connecté
  | { kind: "unchecked"; message: string }   // erreur du serveur sans cause précise

interface ProjectFiles {
  workingDir: string
  configFiles: string[]            // ordre de l'étiquette
  envFiles: string[]               // vide sans --env-file
}

type ProjectProblem =
  | { kind: "invalidName" }
  | { kind: "incomplete" }
  | { kind: "invalidPath"; path: string }       // tronqué à 256 caractères
  | { kind: "ambiguousPath"; label: string }    // config_files ou environment_file
  | { kind: "divergent" }

type ProjectState = "nominal" | "partial" | "failed" | "transition" | "stopped" | "down"

type ComposeAction = "up" | "pull" | "restart" | "down"

interface DownInfo {
  project: string
  command: string                  // commande exacte de down
  containers: DownContainer[]      // arrêtés puis supprimés, compose run compris
  networks: string[]               // supprimés
  namedVolumes: string[]           // conservés
  anonymousVolumes: string[]       // conservés, plus rattachés au projet après le down
  binds: string[]                  // dossiers du serveur montés, conservés
  files: ProjectFiles | null
  filesStatus: FilesStatus | null  // relance possible par Démarrer
}

interface DownContainer {
  name: string
  service: string | null
  running: boolean
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
  background: BackgroundInfo | null   // null : fond fourni avec l'application
  discordPresence: boolean    // présence Discord ; false par défaut
}

interface BackgroundInfo {
  fileName: string            // nom seul, jamais le chemin d'origine
  format: "png" | "jpeg" | "webp"
  bytes: number
  width: number
  height: number
}

type BackgroundChoice =
  | { kind: "cancelled" }
  | { kind: "refused"; fileName: string; refusal: Refusal }
  | { kind: "pending"; token: string; background: BackgroundInfo }

type Refusal =
  | { kind: "format" }
  | { kind: "tooLarge"; bytes: number }
  | { kind: "dimensions"; width: number; height: number }
  | { kind: "unreadable" }                  // en-tête incohérent, ou décodage refusé par la webview

interface PresenceStatus {
  state: "off" | "searching" | "connected" | "devUnavailable" | "notConfigured"
  phase: PhaseState             // pour l'aperçu de la carte
  startedAt: number             // lancement d'Atlas, millisecondes depuis l'epoch Unix : départ de la durée affichée
}

interface PhaseState {
  phase: "linked" | "ground"    // En liaison (AOS) ou Au sol
  since: number                 // début de la phase, millisecondes depuis l'epoch Unix
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

// Édition de fichiers
type OpenedFile = ({ kind: "text" } & TextDocument) | ({ kind: "refused" } & RefusedFile)

interface TextDocument {
  file: FileFacts
  text: string                   // fins de ligne LF
  bom: boolean
  lineEnding: "lf" | "crlf" | "mixed"
  snapshot: Snapshot
  readOnly: ReadOnlyReason[]     // vide : éditable
  sshFile: "authorizedKeys" | "other" | null
  openedAt: number
}

interface RefusedFile {
  file: FileFacts
  refusal: OpenRefusal
  downloadable: boolean
  sshFile: "authorizedKeys" | "other" | null
}

interface FileFacts {
  path: string
  name: string
  size: number
  modified: number | null        // millisecondes depuis l'epoch Unix
  permissions: string            // "-rw-------"
  mode: string | null            // "600"
  owner: Ownership
  dir: string
  dirPermissions: string
  dirOwner: Ownership
  account: string                // compte de la liaison
}

interface Ownership { uid: number | null; gid: number | null; user: string | null; group: string | null }

interface Snapshot { sha256: string; size: number; modified: number | null }

type ReadOnlyReason = "permissions" | "owner" | "directory" | "mixedLineEndings"

type OpenRefusal =
  | { reason: "tooLarge"; size: number; limit: number }
  | { reason: "binary"; offset: number }
  | { reason: "invalidUtf8"; offset: number; line: number }
  | { reason: "privateKey"; line: number; header: string }
  | { reason: "symlink"; target: string | null }
  | { reason: "notAFile" }

interface SaveRequest {
  path: string
  text: string
  expected: Snapshot | null      // null : le fichier doit être absent (recréation)
  bom: boolean
  lineEnding: "lf" | "crlf" | "mixed"
  mode: number                   // droits de l'ouverture, pour une recréation
}

type SaveOutcome =
  | { outcome: "saved"; snapshot: Snapshot; verified: boolean; savedAt: number }
  | { outcome: "conflict"; server: ServerVersion }   // rien n'a été écrit

interface ServerVersion {
  snapshot: Snapshot | null      // null : supprimé sur le serveur
  text: string | null
  refusal: OpenRefusal | null
}

interface AuthorizedKeysCheck {
  linkKey: LinkKey | null        // clé de la liaison en cours
  before: KeyPresence | null     // à l'ouverture
  after: KeyPresence | null      // dans la version à enregistrer
  keys: number                   // lignes reconnues de la version à enregistrer
  certificates: number
  authorities: number
  unrecognized: number[]         // numéros des lignes non reconnues par Atlas
  added: string[]
  removed: string[]
  mode: string | null            // "600", conservé
  tooOpen: boolean
}

interface LinkKey { algorithm: string; comment: string | null; tail: string }   // tail : « …V5gT0hPq »
interface KeyPresence { line: number; options: string[] }
```

`sshAccess` est vrai pour `~/.ssh`, pour ses parents jusqu'au dossier personnel exclu, et pour `~/.ssh/authorized_keys`. Le chemin comparé est celui du dossier listé résolu (`realpath`) suivi du nom : un dossier atteint par un lien symbolique est reconnu. L'interface affiche alors un avertissement dans le GO / NO GO et exige une case à cocher.
