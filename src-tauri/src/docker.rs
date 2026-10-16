//! Conteneurs Docker : liste (`docker ps -a --format '{{json .}}'`, parsing
//! dans des fonctions pures), start, stop, restart, rm et flux `docker logs -f`.
//! Les conteneurs sont manipulés par ID (hexadécimal validé), jamais par nom.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;
use crate::run_state::{Action, RunState};
use crate::ssh::{shell, Connection, Deadline, ExecOutput};
use crate::stream::{self, LineParser, LogLine, Raw};

/// `--no-trunc` : ID complet, utilisé pour les actions.
const LIST_COMMAND: &str = "docker ps -a --no-trunc --format '{{json .}}'";
/// Délai maximal de `docker_list`.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai maximal de `docker start`.
const START_TIMEOUT: Duration = Duration::from_secs(20);
/// Délai maximal de `docker stop` et `docker restart`.
const STOP_TIMEOUT: Duration = Duration::from_secs(45);
/// Délai maximal de `docker rm`.
const REMOVE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Container {
    /// ID complet (64 caractères hexadécimaux).
    pub id: String,
    /// Premier nom, sans les alias `--link`.
    pub name: String,
    /// Référence de l'image, sans digest.
    pub image: String,
    pub state: RunState,
    /// État brut de Docker : running, exited, created…
    pub docker_state: String,
    /// Code de sortie d'un conteneur `exited` ou `restarting`.
    pub exit_code: Option<i32>,
    /// Texte brut de Docker, ex. « Exited (137) 4 days ago ».
    pub status: String,
    /// « depuis 3 heures », « il y a 4 jours ».
    pub duration: Option<String>,
}

/// Résultat de `docker_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerList {
    /// Triés par nom.
    pub containers: Vec<Container>,
    /// Lignes de `docker ps` illisibles (JSON invalide, ID non hexadécimal),
    /// exclues de la liste.
    pub skipped: u32,
}

/// Une ligne de `docker ps --format '{{json .}}'` (champs utiles seulement).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PsLine {
    #[serde(rename = "ID")]
    id: String,
    names: String,
    image: String,
    state: String,
    status: String,
}

/// Liste tous les conteneurs, y compris arrêtés, triés par nom.
pub async fn list(connection: &Connection) -> Result<ContainerList, AppError> {
    let output = connection
        .exec_until(LIST_COMMAND, Deadline::after(LIST_TIMEOUT))
        .await?;
    interpret_list(&output)
}

/// Résultat de `docker ps` : conteneurs, ou erreur compréhensible.
pub fn interpret_list(output: &ExecOutput) -> Result<ContainerList, AppError> {
    if output.success() {
        return Ok(parse_ps_output(&output.stdout_text()));
    }
    Err(failure(output))
}

/// Démarre, arrête ou redémarre un conteneur (ID complet).
pub async fn action(connection: &Connection, id: &str, action: Action) -> Result<(), AppError> {
    let command = action_command(id, action)?;
    let output = connection
        .exec_until(&command, Deadline::after(action_timeout(action)))
        .await?;
    interpret_done(&output)
}

/// Supprime un conteneur arrêté (ID complet). Ses volumes et son image sont
/// conservés ; un conteneur en marche est refusé par Docker (`Remote`).
pub async fn remove(connection: &Connection, id: &str) -> Result<(), AppError> {
    let command = remove_command(id)?;
    let output = connection
        .exec_until(&command, Deadline::after(REMOVE_TIMEOUT))
        .await?;
    interpret_done(&output)
}

/// Délai d'une action. `docker stop` attend lui-même le délai d'arrêt du
/// conteneur (10 s par défaut) avant de forcer : le nôtre est nettement
/// supérieur, et `restart` enchaîne un arrêt et un démarrage.
pub fn action_timeout(action: Action) -> Duration {
    match action {
        Action::Start => START_TIMEOUT,
        Action::Stop | Action::Restart => STOP_TIMEOUT,
    }
}

/// Résultat d'une action ou d'une suppression : rien, ou erreur compréhensible.
pub fn interpret_done(output: &ExecOutput) -> Result<(), AppError> {
    if output.success() {
        Ok(())
    } else {
        Err(failure(output))
    }
}

/// Échec d'une commande `docker` : commande introuvable, accès au socket
/// refusé, ou toute autre erreur (`Remote`, avec la sortie d'erreur).
fn failure(output: &ExecOutput) -> AppError {
    match output.exit_code {
        Some(127) => AppError::DockerNotInstalled,
        _ if is_permission_denied(&output.stderr_text()) => AppError::DockerPermissionDenied,
        _ => output.remote_error(),
    }
}

/// Accès au socket refusé : l'utilisateur n'est pas dans le groupe `docker`.
/// Docker 28 et suivants : « permission denied while trying to connect to the
/// docker API at unix:///var/run/docker.sock » ; versions précédentes : « Got
/// permission denied while trying to connect to the Docker daemon socket… ».
fn is_permission_denied(stderr: &str) -> bool {
    let stderr = stderr.to_lowercase();
    stderr.contains("permission denied") && stderr.contains("docker")
}

/// Une ligne JSON par conteneur. Les lignes vides sont ignorées ; une ligne
/// illisible est exclue et comptée, sans invalider les autres.
pub fn parse_ps_output(output: &str) -> ContainerList {
    let mut containers = Vec::new();
    let mut skipped = 0;
    for line in output.lines().map(str::trim).filter(|l| !l.is_empty()) {
        match parse_ps_line(line) {
            Some(container) => containers.push(container),
            None => skipped += 1,
        }
    }
    containers.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    ContainerList {
        containers,
        skipped,
    }
}

/// Un conteneur, ou `None` si la ligne est illisible ou son ID n'est pas
/// hexadécimal (il ne pourrait pas être manipulé sans risque).
fn parse_ps_line(line: &str) -> Option<Container> {
    let raw: PsLine = serde_json::from_str(line).ok()?;
    if !is_container_id(&raw.id) {
        return None;
    }
    let exit_code = parse_exit_code(&raw.status);
    Some(Container {
        name: primary_name(&raw.names),
        image: display_image(&raw.image),
        state: run_state(&raw.state, exit_code),
        duration: duration(&raw.status),
        exit_code,
        id: raw.id,
        docker_state: raw.state,
        status: raw.status,
    })
}

/// ID de conteneur : hexadécimal uniquement.
pub fn is_container_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// ID complet, seul accepté pour une action ou un flux de logs : exactement
/// 64 caractères hexadécimaux minuscules, comme les donne `docker ps --no-trunc`.
pub fn is_full_container_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// L'ID validé, à échapper avant de l'insérer dans une commande.
fn checked_id(id: &str) -> Result<&str, AppError> {
    if is_full_container_id(id) {
        Ok(id)
    } else {
        Err(AppError::invalid_input(
            "identifiant de conteneur invalide : 64 caractères hexadécimaux attendus",
        ))
    }
}

/// `docker start|stop|restart '<id>'`. Pas de `-t` : le délai d'arrêt propre
/// au conteneur est respecté.
pub fn action_command(id: &str, action: Action) -> Result<String, AppError> {
    let id = checked_id(id)?;
    Ok(format!("docker {} {}", action.verb(), shell::quote(id)))
}

/// `docker rm '<id>'` : jamais `-f` (un conteneur en marche est refusé) ni
/// `-v` (les volumes sont conservés).
pub fn remove_command(id: &str) -> Result<String, AppError> {
    let id = checked_id(id)?;
    Ok(format!("docker rm {}", shell::quote(id)))
}

/// `docker logs -f --tail <N> --timestamps '<id>'`, à envelopper dans le
/// script de surveillance (`stream::guarded_command`, fait par `state`).
pub fn logs_command(id: &str, tail: u32) -> Result<String, AppError> {
    let id = checked_id(id)?;
    let tail = stream::checked_tail(tail)?;
    Ok(format!(
        "docker logs -f --tail {tail} --timestamps {}",
        shell::quote(id)
    ))
}

/// Lignes de `docker logs --timestamps` : `<RFC 3339> <message>`. L'origine
/// vient du flux du canal SSH (un conteneur lancé avec un TTY mélange les
/// deux sur la sortie standard).
pub struct DockerLogParser;

impl LineParser for DockerLogParser {
    /// Ligne affichée plus l'horodatage qui la précède.
    fn max_raw_line(&self) -> usize {
        stream::MAX_LINE_BYTES + 64
    }

    fn parse(&mut self, raw: Raw, line: String, out: &mut Vec<LogLine>) {
        out.push(parse_log_line(raw, line));
    }
}

/// Sépare l'horodatage du message. Sans horodatage reconnu (message
/// d'erreur de `docker logs` lui-même, par exemple), la ligne est gardée
/// entière.
pub fn parse_log_line(raw: Raw, line: String) -> LogLine {
    let (stamp, text) = match line.split_once(' ') {
        Some((stamp, text)) => (stamp, text),
        None => (line.as_str(), ""),
    };
    let mut parsed = LogLine::plain(raw, String::new());
    match stream::normalize_utc_timestamp(stamp) {
        Some(timestamp) => {
            parsed.timestamp = Some(timestamp);
            parsed.text = stream::truncate_line(text.to_owned());
        }
        None => parsed.text = stream::truncate_line(line),
    }
    parsed
}

/// Premier nom qui n'est pas un alias `--link` (`autre/alias`). Avec
/// `--no-trunc`, Docker les liste tous, séparés par des virgules.
pub fn primary_name(names: &str) -> String {
    names
        .split(',')
        .map(str::trim)
        .find(|name| !name.is_empty() && !name.contains('/'))
        .unwrap_or(names)
        .to_owned()
}

/// Image sans digest (`nginx:1.27@sha256:…` → `nginx:1.27`) ; une image sans
/// nom (`sha256:<64 hex>`) est raccourcie à 12 caractères.
pub fn display_image(image: &str) -> String {
    if let Some(hex) = image.strip_prefix("sha256:") {
        return hex.chars().take(12).collect();
    }
    match image.split_once('@') {
        Some((name, _)) if !name.is_empty() => name.to_owned(),
        _ => image.to_owned(),
    }
}

/// Code de sortie lu dans `Status` : `Exited (137) 4 days ago` → 137,
/// `Restarting (1) 5 seconds ago` → 1. `None` pour les autres états.
pub fn parse_exit_code(status: &str) -> Option<i32> {
    let rest = status
        .strip_prefix("Exited (")
        .or_else(|| status.strip_prefix("Restarting ("))?;
    let (code, _) = rest.split_once(')')?;
    code.parse().ok()
}

/// Correspondance des états Docker vers l'interface
/// (tableau de `docs/03-architecture.md`).
pub fn run_state(docker_state: &str, exit_code: Option<i32>) -> RunState {
    match docker_state {
        "running" => RunState::Nominal,
        "exited" => match exit_code {
            Some(0 | 137 | 143) => RunState::Stopped,
            _ => RunState::Failed,
        },
        "created" | "paused" => RunState::Stopped,
        "restarting" | "removing" => RunState::Transition,
        // `dead` et tout état inconnu.
        _ => RunState::Failed,
    }
}

/// Durée lue dans `Status`, en français : « depuis 3 heures » (en cours),
/// « il y a 4 jours » (sorti, en redémarrage). Une durée non reconnue renvoie
/// le texte brut de Docker. `None` sans durée (`Created`, `Dead`…).
pub fn duration(status: &str) -> Option<String> {
    let (prefix, text) = if let Some(rest) = status.strip_prefix("Up ") {
        // Suffixe éventuel : « (healthy) », « (health: starting) », « (Paused) ».
        let text = rest.split_once(" (").map_or(rest, |(text, _)| text);
        ("depuis", text)
    } else {
        let rest = status
            .strip_prefix("Exited (")
            .or_else(|| status.strip_prefix("Restarting ("))?;
        let text = rest.split_once(") ")?.1.strip_suffix(" ago")?;
        ("il y a", text)
    };
    Some(match human_duration_fr(text) {
        Some(french) => format!("{prefix} {french}"),
        None => status.to_owned(),
    })
}

/// Traduit une durée produite par `HumanDuration` (`docker/go-units`) :
/// « Less than a second », « 1 second », « N seconds », « About a minute »,
/// « N minutes », « About an hour », « N hours », « N days », « N weeks »,
/// « N months », « N years ».
pub fn human_duration_fr(text: &str) -> Option<String> {
    let fixed = match text {
        "Less than a second" => Some("moins d'une seconde"),
        "About a minute" => Some("environ une minute"),
        "About an hour" => Some("environ une heure"),
        _ => None,
    };
    if let Some(fixed) = fixed {
        return Some(fixed.to_owned());
    }
    let (count, unit) = text.split_once(' ')?;
    let count: u64 = count.parse().ok()?;
    let (one, many) = match unit {
        "second" | "seconds" => ("seconde", "secondes"),
        "minute" | "minutes" => ("minute", "minutes"),
        "hour" | "hours" => ("heure", "heures"),
        "day" | "days" => ("jour", "jours"),
        "week" | "weeks" => ("semaine", "semaines"),
        "month" | "months" => ("mois", "mois"),
        "year" | "years" => ("an", "ans"),
        _ => return None,
    };
    Some(format!("{count} {}", if count == 1 { one } else { many }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID_WEB: &str = "3f4e8a1c9b2d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f";
    const ID_DB: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f9";

    /// Sortie réelle de `docker ps -a --no-trunc --format '{{json .}}'`
    /// (Docker 29), raccourcie aux champs habituels.
    fn ps_output() -> String {
        [
            json!({
                "Command": "\"/docker-entrypoint.sh nginx -g 'daemon off;'\"",
                "CreatedAt": "2026-10-11 09:12:44 +0000 UTC",
                "ID": ID_WEB,
                "Image": "nginx:1.27@sha256:0d17b565c37bcbd895e9d92315a05c1c3c9a29f762b011a10c54a66cd53c9b31",
                "Labels": "com.docker.compose.project=web",
                "LocalVolumes": "0",
                "Mounts": "",
                "Names": "web-nginx-1",
                "Networks": "web_default",
                "Ports": "0.0.0.0:80->80/tcp",
                "RunningFor": "4 days ago",
                "Size": "0B",
                "State": "running",
                "Status": "Up 3 hours (healthy)",
            }),
            json!({
                "Command": "\"docker-entrypoint.sh postgres\"",
                "CreatedAt": "2026-10-01 18:02:10 +0000 UTC",
                "ID": ID_DB,
                "Image": "postgres:17",
                "Labels": "",
                "LocalVolumes": "1",
                "Mounts": "pgdata",
                "Names": "db,web-nginx-1/db",
                "Networks": "bridge",
                "Ports": "",
                "RunningFor": "2 weeks ago",
                "Size": "0B",
                "State": "exited",
                "Status": "Exited (137) 4 days ago",
            }),
        ]
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
            + "\n"
    }

    fn exec_output(exit_code: u32, stdout: &str, stderr: &str) -> ExecOutput {
        ExecOutput {
            exit_code: Some(exit_code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    // --- Analyse du JSON -------------------------------------------------

    #[test]
    fn ps_output_is_parsed_and_sorted_by_name() {
        let list = parse_ps_output(&ps_output());
        assert_eq!(list.skipped, 0);
        assert_eq!(
            list.containers,
            vec![
                Container {
                    id: ID_DB.into(),
                    name: "db".into(),
                    image: "postgres:17".into(),
                    state: RunState::Stopped,
                    docker_state: "exited".into(),
                    exit_code: Some(137),
                    status: "Exited (137) 4 days ago".into(),
                    duration: Some("il y a 4 jours".into()),
                },
                Container {
                    id: ID_WEB.into(),
                    name: "web-nginx-1".into(),
                    image: "nginx:1.27".into(),
                    state: RunState::Nominal,
                    docker_state: "running".into(),
                    exit_code: None,
                    status: "Up 3 hours (healthy)".into(),
                    duration: Some("depuis 3 heures".into()),
                },
            ]
        );
    }

    #[test]
    fn empty_output_is_an_empty_list() {
        let empty = ContainerList {
            containers: vec![],
            skipped: 0,
        };
        assert_eq!(parse_ps_output(""), empty);
        assert_eq!(parse_ps_output("\n\n"), empty);
    }

    #[test]
    fn unreadable_lines_are_skipped_and_counted() {
        let wrong_id = json!({
            "ID": "web; rm -rf ~", "Names": "web", "Image": "nginx",
            "State": "running", "Status": "Up 3 hours",
        });
        let missing_field = json!({ "ID": ID_WEB, "Names": "web" });
        let output = format!(
            "pas du json\n{}{wrong_id}\n{missing_field}\n{{\"ID\":",
            ps_output()
        );
        let list = parse_ps_output(&output);
        assert_eq!(list.skipped, 4);
        let names: Vec<_> = list.containers.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["db", "web-nginx-1"]);
    }

    #[test]
    fn container_list_serializes_in_camel_case() {
        let value = serde_json::to_value(parse_ps_output("pas du json")).unwrap();
        assert_eq!(value, json!({ "containers": [], "skipped": 1 }));
    }

    #[test]
    fn container_serializes_in_camel_case() {
        let container = &parse_ps_output(&ps_output()).containers[0];
        assert_eq!(
            serde_json::to_value(container).unwrap(),
            json!({
                "id": ID_DB,
                "name": "db",
                "image": "postgres:17",
                "state": "stopped",
                "dockerState": "exited",
                "exitCode": 137,
                "status": "Exited (137) 4 days ago",
                "duration": "il y a 4 jours",
            })
        );
    }

    #[test]
    fn container_ids_are_hexadecimal() {
        assert!(is_container_id(ID_WEB));
        assert!(is_container_id("3f4e8a1c9b2d"));
        assert!(!is_container_id(""));
        assert!(!is_container_id("web-nginx-1"));
        assert!(!is_container_id(&format!("{ID_WEB}0")));
    }

    /// Valeurs hostiles qui ne doivent jamais atteindre une commande.
    const HOSTILE: &[&str] = &[
        "x; rm -rf ~",
        "$(rm -rf ~)",
        "`id`",
        "'; rm -rf ~; '",
        "\"; rm -rf ~; \"",
        "abc\nrm -rf ~",
        "-f",
        "web-nginx-1",
        "",
    ];

    #[test]
    fn full_container_ids_are_64_lowercase_hex() {
        assert!(is_full_container_id(ID_WEB));
        assert!(!is_full_container_id("3f4e8a1c9b2d"));
        assert!(!is_full_container_id(&ID_WEB.to_uppercase()));
        assert!(!is_full_container_id(&format!("{ID_WEB}0")));
        assert!(!is_full_container_id(&format!("{}\n", &ID_WEB[..63])));
        assert!(!is_full_container_id(&format!("{}g", &ID_WEB[..63])));
        for hostile in HOSTILE {
            assert!(!is_full_container_id(hostile), "{hostile:?}");
        }
    }

    #[test]
    fn action_commands_quote_the_validated_id() {
        let quoted = format!("'{ID_WEB}'");
        assert_eq!(
            action_command(ID_WEB, Action::Start).unwrap(),
            format!("docker start {quoted}")
        );
        assert_eq!(
            action_command(ID_WEB, Action::Stop).unwrap(),
            format!("docker stop {quoted}")
        );
        assert_eq!(
            action_command(ID_WEB, Action::Restart).unwrap(),
            format!("docker restart {quoted}")
        );
        assert_eq!(
            remove_command(ID_WEB).unwrap(),
            format!("docker rm {quoted}")
        );
    }

    #[test]
    fn hostile_ids_are_refused_before_any_command() {
        let shortened = &ID_WEB[..12];
        for hostile in HOSTILE.iter().copied().chain([shortened]) {
            for action in [Action::Start, Action::Stop, Action::Restart] {
                assert!(
                    matches!(
                        action_command(hostile, action),
                        Err(AppError::InvalidInput { .. })
                    ),
                    "{hostile:?}"
                );
            }
            assert!(
                matches!(remove_command(hostile), Err(AppError::InvalidInput { .. })),
                "{hostile:?}"
            );
        }
    }

    // --- Logs -------------------------------------------------------------

    #[test]
    fn logs_command_follows_with_timestamps() {
        assert_eq!(
            logs_command(ID_WEB, 200).unwrap(),
            format!("docker logs -f --tail 200 --timestamps '{ID_WEB}'")
        );
        assert_eq!(
            logs_command(ID_WEB, 0).unwrap(),
            format!("docker logs -f --tail 0 --timestamps '{ID_WEB}'")
        );
    }

    #[test]
    fn logs_command_refuses_hostile_ids_and_long_history() {
        for hostile in HOSTILE {
            assert!(
                matches!(
                    logs_command(hostile, 200),
                    Err(AppError::InvalidInput { .. })
                ),
                "{hostile:?}"
            );
        }
        assert!(matches!(
            logs_command(ID_WEB, stream::MAX_TAIL + 1),
            Err(AppError::InvalidInput { .. })
        ));
    }

    #[test]
    fn log_lines_are_split_into_timestamp_and_message() {
        use crate::stream::Origin;

        let line = parse_log_line(
            Raw::Stdout,
            "2026-10-16T09:13:46.900123456Z GET /api/health 200 · 3 ms".into(),
        );
        assert_eq!(line.timestamp.as_deref(), Some("2026-10-16T09:13:46.900Z"));
        assert_eq!(line.text, "GET /api/health 200 · 3 ms");
        assert_eq!(line.origin, Origin::Stdout);
        assert_eq!(line.priority, None);
        assert!(!line.missing);

        let err = parse_log_line(
            Raw::Stderr,
            "2026-10-16T09:13:47.000000000Z slow query  812 ms".into(),
        );
        assert_eq!(err.origin, Origin::Stderr);
        // Espaces du message conservés.
        assert_eq!(err.text, "slow query  812 ms");
    }

    #[test]
    fn empty_messages_keep_their_timestamp() {
        let line = parse_log_line(Raw::Stdout, "2026-10-16T09:13:46.900123456Z ".into());
        assert_eq!(line.timestamp.as_deref(), Some("2026-10-16T09:13:46.900Z"));
        assert_eq!(line.text, "");
        let bare = parse_log_line(Raw::Stdout, "2026-10-16T09:13:46.900123456Z".into());
        assert_eq!(bare.text, "");
        assert!(bare.timestamp.is_some());
    }

    #[test]
    fn lines_without_timestamp_are_kept_whole() {
        let line = parse_log_line(
            Raw::Stderr,
            "Error response from daemon: No such container: abc".into(),
        );
        assert_eq!(line.timestamp, None);
        assert_eq!(
            line.text,
            "Error response from daemon: No such container: abc"
        );
    }

    #[test]
    fn long_log_messages_are_truncated() {
        let line = parse_log_line(
            Raw::Stdout,
            format!(
                "2026-10-16T09:13:46.900123456Z {}",
                "x".repeat(stream::MAX_LINE_BYTES + 10)
            ),
        );
        assert!(line.text.ends_with('…'));
        assert!(line.text.len() <= stream::MAX_LINE_BYTES + '…'.len_utf8());
    }

    #[test]
    fn remove_never_forces_nor_deletes_volumes() {
        let command = remove_command(ID_WEB).unwrap();
        assert!(!command.contains(" -f") && !command.contains("--force"));
        assert!(!command.contains(" -v") && !command.contains("--volumes"));
    }

    #[test]
    fn primary_name_skips_link_aliases() {
        assert_eq!(primary_name("web"), "web");
        assert_eq!(primary_name("db,web/db"), "db");
        assert_eq!(primary_name("web/db,db"), "db");
    }

    #[test]
    fn image_digest_is_removed() {
        assert_eq!(display_image("nginx:1.27"), "nginx:1.27");
        assert_eq!(
            display_image("ghcr.io/org/app:2@sha256:0d17b565c37bcbd895e9d92315a05c1c"),
            "ghcr.io/org/app:2"
        );
        assert_eq!(display_image(&format!("sha256:{ID_WEB}")), "3f4e8a1c9b2d");
        assert_eq!(display_image("localhost:5000/app"), "localhost:5000/app");
    }

    // --- Code de sortie -------------------------------------------------

    #[test]
    fn exit_code_is_read_from_status() {
        assert_eq!(parse_exit_code("Exited (0) 2 minutes ago"), Some(0));
        assert_eq!(parse_exit_code("Exited (1) About an hour ago"), Some(1));
        assert_eq!(parse_exit_code("Exited (137) 4 days ago"), Some(137));
        assert_eq!(parse_exit_code("Exited (143) 3 weeks ago"), Some(143));
        assert_eq!(parse_exit_code("Exited (-1) 5 seconds ago"), Some(-1));
        assert_eq!(parse_exit_code("Restarting (1) 5 seconds ago"), Some(1));
    }

    #[test]
    fn running_and_other_containers_have_no_exit_code() {
        assert_eq!(parse_exit_code("Up 3 hours"), None);
        assert_eq!(parse_exit_code("Up 2 days (Paused)"), None);
        assert_eq!(parse_exit_code("Created"), None);
        assert_eq!(parse_exit_code("Dead"), None);
        assert_eq!(parse_exit_code("Removal In Progress"), None);
        assert_eq!(parse_exit_code(""), None);
        assert_eq!(parse_exit_code("Exited (x) 2 minutes ago"), None);
    }

    // --- Correspondance des états ---------------------------------------

    #[test]
    fn docker_states_map_to_run_states() {
        assert_eq!(run_state("running", None), RunState::Nominal);
        assert_eq!(run_state("exited", Some(0)), RunState::Stopped);
        assert_eq!(run_state("exited", Some(137)), RunState::Stopped);
        assert_eq!(run_state("exited", Some(143)), RunState::Stopped);
        assert_eq!(run_state("exited", Some(1)), RunState::Failed);
        assert_eq!(run_state("exited", Some(2)), RunState::Failed);
        assert_eq!(run_state("exited", None), RunState::Failed);
        assert_eq!(run_state("created", None), RunState::Stopped);
        assert_eq!(run_state("paused", None), RunState::Stopped);
        assert_eq!(run_state("restarting", Some(1)), RunState::Transition);
        assert_eq!(run_state("removing", None), RunState::Transition);
        assert_eq!(run_state("dead", None), RunState::Failed);
        assert_eq!(run_state("inconnu", None), RunState::Failed);
    }

    // --- Durées ---------------------------------------------------------

    #[test]
    fn every_human_duration_form_is_translated() {
        let cases = [
            ("Less than a second", "moins d'une seconde"),
            ("1 second", "1 seconde"),
            ("2 seconds", "2 secondes"),
            ("59 seconds", "59 secondes"),
            ("About a minute", "environ une minute"),
            ("2 minutes", "2 minutes"),
            ("59 minutes", "59 minutes"),
            ("About an hour", "environ une heure"),
            ("2 hours", "2 heures"),
            ("47 hours", "47 heures"),
            ("2 days", "2 jours"),
            ("13 days", "13 jours"),
            ("2 weeks", "2 semaines"),
            ("8 weeks", "8 semaines"),
            ("2 months", "2 mois"),
            ("24 months", "24 mois"),
            ("2 years", "2 ans"),
            ("10 years", "10 ans"),
        ];
        for (english, french) in cases {
            assert_eq!(
                human_duration_fr(english).as_deref(),
                Some(french),
                "{english}"
            );
        }
    }

    #[test]
    fn singular_units_are_handled() {
        assert_eq!(human_duration_fr("1 minute").as_deref(), Some("1 minute"));
        assert_eq!(human_duration_fr("1 hours").as_deref(), Some("1 heure"));
        assert_eq!(human_duration_fr("1 day").as_deref(), Some("1 jour"));
        assert_eq!(human_duration_fr("1 years").as_deref(), Some("1 an"));
    }

    #[test]
    fn unknown_durations_are_not_translated() {
        assert_eq!(human_duration_fr(""), None);
        assert_eq!(human_duration_fr("About a fortnight"), None);
        assert_eq!(human_duration_fr("3 fortnights"), None);
        assert_eq!(human_duration_fr("many hours"), None);
    }

    #[test]
    fn duration_is_extracted_from_status() {
        let cases = [
            ("Up Less than a second", Some("depuis moins d'une seconde")),
            ("Up About a minute", Some("depuis environ une minute")),
            ("Up 3 hours (healthy)", Some("depuis 3 heures")),
            ("Up 5 minutes (unhealthy)", Some("depuis 5 minutes")),
            (
                "Up 12 seconds (health: starting)",
                Some("depuis 12 secondes"),
            ),
            ("Up 2 days (Paused)", Some("depuis 2 jours")),
            (
                "Exited (0) Less than a second ago",
                Some("il y a moins d'une seconde"),
            ),
            ("Exited (137) 4 days ago", Some("il y a 4 jours")),
            (
                "Exited (1) About an hour ago",
                Some("il y a environ une heure"),
            ),
            ("Restarting (1) 5 seconds ago", Some("il y a 5 secondes")),
            ("Created", None),
            ("Dead", None),
            ("Removal In Progress", None),
            ("", None),
        ];
        for (status, expected) in cases {
            assert_eq!(duration(status).as_deref(), expected, "{status}");
        }
    }

    #[test]
    fn unknown_duration_keeps_the_raw_status() {
        assert_eq!(
            duration("Up 3 fortnights").as_deref(),
            Some("Up 3 fortnights")
        );
    }

    // --- Erreurs --------------------------------------------------------

    #[test]
    fn missing_docker_is_reported() {
        let output = exec_output(127, "", "bash: line 1: docker: command not found\n");
        assert!(matches!(
            interpret_list(&output),
            Err(AppError::DockerNotInstalled)
        ));
    }

    #[test]
    fn permission_denied_is_reported() {
        for stderr in [
            "permission denied while trying to connect to the docker API at unix:///var/run/docker.sock",
            "Got permission denied while trying to connect to the Docker daemon socket at \
             unix:///var/run/docker.sock: Get \"http://%2Fvar%2Frun%2Fdocker.sock/v1.47/containers/json\": \
             dial unix /var/run/docker.sock: connect: permission denied",
        ] {
            assert!(matches!(
                interpret_list(&exec_output(1, "", stderr)),
                Err(AppError::DockerPermissionDenied)
            ));
        }
    }

    #[test]
    fn other_failures_are_remote_errors() {
        let output = exec_output(
            1,
            "",
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?\n",
        );
        match interpret_list(&output) {
            Err(AppError::Remote { exit_code, stderr }) => {
                assert_eq!(exit_code, 1);
                assert!(stderr.starts_with("Cannot connect"));
                assert!(!stderr.ends_with('\n'));
            }
            other => panic!("attendu Remote, reçu {other:?}"),
        }
    }

    #[test]
    fn successful_output_is_parsed() {
        let output = exec_output(0, &ps_output(), "");
        assert_eq!(interpret_list(&output).unwrap().containers.len(), 2);
    }

    // --- Actions et suppression ------------------------------------------

    #[test]
    fn successful_action_returns_nothing() {
        // `docker start` réécrit l'ID sur la sortie standard.
        assert!(interpret_done(&exec_output(0, &format!("{ID_WEB}\n"), "")).is_ok());
    }

    #[test]
    fn removing_a_running_container_is_a_remote_error() {
        let stderr = format!(
            "Error response from daemon: cannot remove container \"{ID_WEB}\": container is running: \
             stop the container before removing or force remove\n"
        );
        match interpret_done(&exec_output(1, "", &stderr)) {
            Err(AppError::Remote { exit_code, stderr }) => {
                assert_eq!(exit_code, 1);
                assert!(stderr.contains("container is running"));
            }
            other => panic!("attendu Remote, reçu {other:?}"),
        }
    }

    #[test]
    fn action_failures_are_classified_like_the_list() {
        assert!(matches!(
            interpret_done(&exec_output(127, "", "sh: 1: docker: not found")),
            Err(AppError::DockerNotInstalled)
        ));
        assert!(matches!(
            interpret_done(&exec_output(
                1,
                "",
                "permission denied while trying to connect to the docker API at unix:///var/run/docker.sock"
            )),
            Err(AppError::DockerPermissionDenied)
        ));
    }

    #[test]
    fn stop_timeout_is_well_above_the_docker_grace_period() {
        // `docker stop` attend jusqu'à 10 s avant de forcer l'arrêt.
        assert!(action_timeout(Action::Stop) >= Duration::from_secs(40));
        assert!(action_timeout(Action::Restart) >= Duration::from_secs(40));
        assert_eq!(action_timeout(Action::Start), Duration::from_secs(20));
    }
}
