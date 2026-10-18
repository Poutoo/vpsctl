//! Services systemd utilisateur (`systemctl --user`, `journalctl --user`) :
//! liste en JSON (parsing dans des fonctions pures), actions, flux de logs,
//! création et suppression des fichiers d'unité dans `~/.config/systemd/user/`
//! suivies de `daemon-reload`.
//! Noms de services validés par `^[a-zA-Z0-9@._-]+\.service$`.
//!
//! Liste : unités en mémoire (`list-units` en JSON) et fichiers d'unité du
//! dossier utilisateur (lus par SFTP), croisés par `select_user_services`.
//! Règle de filtrage et correspondance des états : `docs/03-architecture.md`.
//!
//! - `unit` : validation d'un service à créer et texte de son fichier d'unité ;
//! - `manage` : séquences de création et de suppression.

pub mod unit;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;
use crate::run_state::{Action, RunState};
use crate::sftp::{self, DirEntry, EntryKind};
use crate::ssh::{self, shell, Connection, Deadline};
use crate::stream::{self, LineParser, LogLine, Origin, Raw};

const LIST_COMMAND: &str =
    "systemctl --user list-units --type=service --all --output=json --no-pager";
/// Dossier des fichiers d'unité de l'utilisateur, relatif au dossier personnel
/// (point de départ des chemins SFTP).
pub const UNIT_DIR: &str = ".config/systemd/user";
/// Délai maximal de `services_list`.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai maximal de `service_action` (relecture de la liste et commande).
const ACTION_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Service {
    pub unit: String,
    /// `None` : fichier présent mais unité non chargée par systemd.
    pub load: Option<String>,
    /// État brut : active, inactive, failed…
    pub active: String,
    /// Sous-état brut : running, dead, exited…
    pub sub: String,
    /// `None` si l'unité n'est pas chargée.
    pub description: Option<String>,
    pub state: RunState,
    pub masked: bool,
    /// Faux si l'unité est masquée ou si son nom ne respecte pas la regex des
    /// services : elle ne reçoit alors aucune action.
    pub controllable: bool,
}

/// Une unité de `systemctl --user list-units --output=json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UnitRow {
    pub unit: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    #[serde(default)]
    pub description: String,
}

/// Fichier d'unité de service présent dans le dossier utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitFile {
    pub name: String,
    /// Lien vers `/dev/null` (`systemctl --user mask`).
    pub masked: bool,
}

/// Liste les services utilisateur, triés par nom d'unité.
/// Les deux lectures partagent la même échéance de 10 s.
pub async fn list(connection: &Connection) -> Result<Vec<Service>, AppError> {
    list_until(connection, Deadline::after(LIST_TIMEOUT)).await
}

/// Démarre, arrête ou redémarre un service pilotable, sans attendre le
/// résultat (`--no-block`). La liste est relue au préalable : l'unité doit
/// être un service utilisateur pilotable (règle de `select_user_services`).
/// Les deux étapes partagent une échéance de 20 s.
pub async fn action(connection: &Connection, unit: &str, action: Action) -> Result<(), AppError> {
    let command = action_command(unit, action)?;
    let deadline = Deadline::after(ACTION_TIMEOUT);
    let services = list_until(connection, deadline).await?;
    ensure_controllable(&services, unit)?;
    let output = connection.exec_until(&command, deadline).await?;
    if output.success() {
        Ok(())
    } else {
        Err(output.remote_error())
    }
}

/// L'unité figure dans la liste et peut recevoir une action.
pub fn ensure_controllable(services: &[Service], unit: &str) -> Result<(), AppError> {
    match services.iter().find(|s| s.unit == unit) {
        None => Err(AppError::invalid_input(format!(
            "{unit} n'est pas un service de ~/.config/systemd/user/"
        ))),
        Some(service) if service.masked => Err(AppError::invalid_input(format!(
            "{unit} est masqué : il ne peut pas être piloté"
        ))),
        Some(service) if !service.controllable => Err(AppError::invalid_input(format!(
            "{unit} n'est pas pilotable"
        ))),
        Some(_) => Ok(()),
    }
}

async fn list_until(connection: &Connection, deadline: Deadline) -> Result<Vec<Service>, AppError> {
    let (units, files) = tokio::join!(
        list_units(connection, deadline),
        ssh::within(deadline, unit_files(connection)),
    );
    Ok(select_user_services(units?, &files?))
}

async fn list_units(connection: &Connection, deadline: Deadline) -> Result<Vec<UnitRow>, AppError> {
    let output = connection.exec_until(LIST_COMMAND, deadline).await?;
    if !output.success() {
        return Err(output.remote_error());
    }
    parse_list_units(&output.stdout_text())
}

/// Fichiers d'unité du dossier utilisateur. Dossier absent : aucun fichier.
async fn unit_files(connection: &Connection) -> Result<Vec<UnitFile>, AppError> {
    let Some(entries) = sftp::read_dir(connection, UNIT_DIR).await? else {
        return Ok(Vec::new());
    };
    let mut files = Vec::new();
    for entry in unit_file_candidates(&entries) {
        let masked = if entry.kind == EntryKind::Symlink {
            let target = sftp::read_link(connection, &format!("{UNIT_DIR}/{}", entry.name)).await?;
            target.as_deref().is_some_and(is_mask_target)
        } else {
            false
        };
        files.push(UnitFile {
            name: entry.name.clone(),
            masked,
        });
    }
    Ok(files)
}

/// Entrées du dossier qui sont des fichiers d'unité de service : fichiers ou
/// liens nommés `*.service`. Les sous-dossiers (`*.wants/`, `*.service.d/`)
/// sont ignorés.
pub fn unit_file_candidates(entries: &[DirEntry]) -> Vec<&DirEntry> {
    entries
        .iter()
        .filter(|e| matches!(e.kind, EntryKind::File | EntryKind::Symlink))
        .filter(|e| e.name.len() > ".service".len() && e.name.ends_with(".service"))
        .collect()
}

/// Cible d'un lien de masquage.
pub fn is_mask_target(target: &str) -> bool {
    target == "/dev/null"
}

/// Analyse la sortie JSON de `list-units` (un tableau d'objets).
pub fn parse_list_units(output: &str) -> Result<Vec<UnitRow>, AppError> {
    let output = output.trim();
    if output.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(output)
        .map_err(|e| AppError::io(format!("réponse inattendue de systemctl : {e}")))
}

/// Gabarit d'une instance : `bot@1.service` → `bot@.service`. `None` pour
/// une unité qui n'est pas une instance (dont le gabarit lui-même).
pub fn template_of(unit: &str) -> Option<String> {
    let stem = unit.strip_suffix(".service")?;
    let (prefix, instance) = stem.split_once('@')?;
    (!prefix.is_empty() && !instance.is_empty()).then(|| format!("{prefix}@.service"))
}

/// Fichier gabarit (`bot@.service`).
pub fn is_template(name: &str) -> bool {
    name.strip_suffix("@.service")
        .is_some_and(|p| !p.is_empty())
}

/// Règle de filtrage : ne garde que les services dont le fichier d'unité est
/// dans le dossier utilisateur, plus les instances de ses gabarits. Les
/// fichiers que systemd n'a pas en mémoire sont ajoutés, « arrêtés ».
pub fn select_user_services(units: Vec<UnitRow>, files: &[UnitFile]) -> Vec<Service> {
    let plain: HashMap<&str, &UnitFile> = files
        .iter()
        .filter(|f| !is_template(&f.name))
        .map(|f| (f.name.as_str(), f))
        .collect();
    let templates: HashSet<&str> = files
        .iter()
        .filter(|f| is_template(&f.name))
        .map(|f| f.name.as_str())
        .collect();

    let mut seen = HashSet::new();
    let mut services = Vec::new();
    for row in units {
        let file = plain.get(row.unit.as_str());
        let from_template = template_of(&row.unit).is_some_and(|t| templates.contains(t.as_str()));
        if file.is_none() && !from_template {
            continue;
        }
        let masked = row.load == "masked" || file.is_some_and(|f| f.masked);
        seen.insert(row.unit.clone());
        services.push(loaded_service(row, masked));
    }
    for file in plain.values() {
        if !seen.contains(&file.name) {
            services.push(unloaded_service(file));
        }
    }
    services.sort_by(|a, b| a.unit.cmp(&b.unit));
    services
}

fn loaded_service(row: UnitRow, masked: bool) -> Service {
    let state = run_state(&row.load, &row.active);
    Service {
        controllable: !masked && is_valid_unit_name(&row.unit),
        description: Some(row.description).filter(|d| !d.is_empty()),
        unit: row.unit,
        load: Some(row.load),
        active: row.active,
        sub: row.sub,
        state,
        masked,
    }
}

fn unloaded_service(file: &UnitFile) -> Service {
    Service {
        unit: file.name.clone(),
        load: None,
        active: "inactive".into(),
        sub: "dead".into(),
        description: None,
        state: RunState::Stopped,
        masked: file.masked,
        controllable: !file.masked && is_valid_unit_name(&file.name),
    }
}

/// Correspondance des états systemd vers l'interface. Un chargement échoué
/// (`not-found`, `bad-setting`, `error`) est un échec ; sinon, l'état dépend
/// du champ `active`.
pub fn run_state(load: &str, active: &str) -> RunState {
    if matches!(load, "not-found" | "bad-setting" | "error") {
        return RunState::Failed;
    }
    match active {
        "active" => RunState::Nominal,
        "inactive" => RunState::Stopped,
        "failed" => RunState::Failed,
        "activating" | "deactivating" | "reloading" | "refreshing" | "maintenance" => {
            RunState::Transition
        }
        _ => RunState::Failed,
    }
}

/// Nom de service pilotable : `^[a-zA-Z0-9@._-]+\.service$`.
pub fn is_valid_unit_name(unit: &str) -> bool {
    unit.strip_suffix(".service").is_some_and(|stem| {
        !stem.is_empty()
            && stem
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'@' | b'.' | b'_' | b'-'))
    })
}

/// Le nom validé, à échapper avant de l'insérer dans une commande.
pub(crate) fn checked_unit(unit: &str) -> Result<&str, AppError> {
    if is_valid_unit_name(unit) {
        Ok(unit)
    } else {
        Err(AppError::invalid_input(format!(
            "nom de service invalide : {unit:?}"
        )))
    }
}

/// `journalctl --user --unit='<unit>' -f -n <N> -o json …`, à envelopper
/// dans le script de surveillance (`stream::guarded_command`, fait par
/// `state`). Le nom doit respecter la regex des services, mais une unité
/// masquée est acceptée : ses logs restent consultables.
///
/// - `--unit=` plutôt que `-u` : un nom autorisé peut commencer par `-`.
/// - `--output-fields` : seuls `MESSAGE` et `PRIORITY` (plus les champs
///   toujours présents, dont `__REALTIME_TIMESTAMP`).
/// - `--all` : sans lui, un champ de plus de 4 096 octets sort en `null`.
pub fn logs_command(unit: &str, tail: u32) -> Result<String, AppError> {
    let unit = checked_unit(unit)?;
    let tail = stream::checked_tail(tail)?;
    Ok(format!(
        "journalctl --user --unit={} -f -n {tail} -o json \
         --output-fields=MESSAGE,PRIORITY --all --quiet --no-pager",
        shell::quote(unit)
    ))
}

/// Texte affiché pour une entrée du journal sans `MESSAGE`.
pub const MISSING_MESSAGE: &str = "[sans message]";

/// Entrées de `journalctl -o json` (une par ligne de la sortie standard).
/// La sortie d'erreur de `journalctl` lui-même est affichée telle quelle.
pub struct JournalParser;

impl LineParser for JournalParser {
    /// Une entrée JSON peut dépasser largement la ligne affichée (message
    /// long, octets encodés en tableau) : elle est tronquée après analyse.
    fn max_raw_line(&self) -> usize {
        1024 * 1024
    }

    fn parse(&mut self, raw: Raw, line: String, out: &mut Vec<LogLine>) {
        match raw {
            Raw::Stdout => out.extend(parse_journal_entry(&line)),
            Raw::Stderr => out.push(LogLine::plain(raw, stream::truncate_line(line))),
        }
    }
}

/// Une entrée du journal en lignes de console : une par ligne du message.
///
/// `MESSAGE` peut être une chaîne, un tableau d'octets (message non UTF-8,
/// décodé avec remplacement), `null` (champ trop long sans `--all`) ou
/// absent : les deux derniers cas donnent « [sans message] ». `PRIORITY`
/// absente ou illisible donne `None`. Une ligne qui n'est pas du JSON est
/// affichée telle quelle.
pub fn parse_journal_entry(line: &str) -> Vec<LogLine> {
    let entry = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(serde_json::Value::Object(entry)) => entry,
        _ => {
            return vec![journal_line(
                stream::truncate_line(line.to_owned()),
                None,
                None,
            )]
        }
    };
    let timestamp = entry
        .get("__REALTIME_TIMESTAMP")
        .and_then(json_u64)
        .map(stream::iso_from_micros);
    let priority = entry
        .get("PRIORITY")
        .and_then(json_u64)
        .and_then(|p| u8::try_from(p).ok())
        .filter(|p| *p <= 7);

    let message = match entry.get("MESSAGE") {
        Some(serde_json::Value::String(text)) => Some(text.clone()),
        Some(serde_json::Value::Array(bytes)) => message_bytes(bytes),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    let Some(message) = message else {
        let mut line = journal_line(MISSING_MESSAGE.to_owned(), timestamp, priority);
        line.missing = true;
        return vec![line];
    };

    let message = message.strip_suffix('\n').unwrap_or(&message);
    message
        .split('\n')
        .map(|part| {
            let part = part.strip_suffix('\r').unwrap_or(part);
            journal_line(
                stream::truncate_line(part.to_owned()),
                timestamp.clone(),
                priority,
            )
        })
        .collect()
}

fn journal_line(text: String, timestamp: Option<String>, priority: Option<u8>) -> LogLine {
    LogLine {
        text,
        timestamp,
        origin: Origin::Journal,
        priority,
        missing: false,
    }
}

/// Le journal écrit les nombres en chaînes (`"6"`, `"1792142026900123"`).
fn json_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::String(text) => text.parse().ok(),
        other => other.as_u64(),
    }
}

/// Message non UTF-8 : tableau d'octets. `None` si un élément n'est pas un
/// octet.
fn message_bytes(values: &[serde_json::Value]) -> Option<String> {
    let bytes = values
        .iter()
        .map(|v| v.as_u64().and_then(|b| u8::try_from(b).ok()))
        .collect::<Option<Vec<u8>>>()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `systemctl --user --no-block start|stop|restart -- '<unit>'`.
/// `--no-block` : la commande rend la main dès que la tâche est mise en file,
/// l'état réel est vérifié ensuite par l'interface. `--` : un nom autorisé
/// peut commencer par `-`.
pub fn action_command(unit: &str, action: Action) -> Result<String, AppError> {
    let unit = checked_unit(unit)?;
    Ok(format!(
        "systemctl --user --no-block {} -- {}",
        action.verb(),
        shell::quote(unit)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Sortie de `systemctl --user list-units --type=service --all
    /// --output=json` (systemd 259) : services du système, services de
    /// l'utilisateur, instance de gabarit, unités introuvables et masquée.
    const LIST_UNITS: &str = r#"[
{"unit":"bot@1.service","load":"loaded","active":"active","sub":"running","description":"Bot instance 1"},
{"unit":"bot@2.service","load":"loaded","active":"failed","sub":"failed","description":"Bot instance 2"},
{"unit":"dbus.service","load":"loaded","active":"active","sub":"running","description":"D-Bus User Message Bus"},
{"unit":"api.service","load":"loaded","active":"active","sub":"running","description":"API"},
{"unit":"cron-like.service","load":"loaded","active":"activating","sub":"start","description":"Tâche"},
{"unit":"ghost.service","load":"not-found","active":"inactive","sub":"dead","description":"ghost.service"},
{"unit":"broken.service","load":"bad-setting","active":"inactive","sub":"dead","description":"broken.service"},
{"unit":"hidden.service","load":"masked","active":"inactive","sub":"dead","description":"hidden.service"},
{"unit":"pipewire.service","load":"loaded","active":"inactive","sub":"dead","description":"PipeWire Multimedia Service"},
{"unit":"worker@x.service","load":"loaded","active":"active","sub":"running","description":"Instance d'un gabarit système"}
]"#;

    fn file(name: &str) -> UnitFile {
        UnitFile {
            name: name.into(),
            masked: false,
        }
    }

    fn entry(name: &str, kind: EntryKind) -> DirEntry {
        DirEntry {
            name: name.into(),
            kind,
        }
    }

    fn user_files() -> Vec<UnitFile> {
        vec![
            file("api.service"),
            file("bot@.service"),
            file("cron-like.service"),
            file("broken.service"),
            file("never-started.service"),
            UnitFile {
                name: "hidden.service".into(),
                masked: true,
            },
        ]
    }

    fn units() -> Vec<String> {
        select_user_services(parse_list_units(LIST_UNITS).unwrap(), &user_files())
            .into_iter()
            .map(|s| s.unit)
            .collect()
    }

    fn service(unit: &str) -> Service {
        select_user_services(parse_list_units(LIST_UNITS).unwrap(), &user_files())
            .into_iter()
            .find(|s| s.unit == unit)
            .unwrap_or_else(|| panic!("{unit} absent"))
    }

    // --- Analyse du JSON -------------------------------------------------

    #[test]
    fn list_units_json_is_parsed() {
        let rows = parse_list_units(LIST_UNITS).unwrap();
        assert_eq!(rows.len(), 10);
        assert_eq!(
            rows[2],
            UnitRow {
                unit: "dbus.service".into(),
                load: "loaded".into(),
                active: "active".into(),
                sub: "running".into(),
                description: "D-Bus User Message Bus".into(),
            }
        );
    }

    #[test]
    fn empty_list_units_output_is_an_empty_list() {
        assert_eq!(parse_list_units("[]").unwrap(), vec![]);
        assert_eq!(parse_list_units("").unwrap(), vec![]);
    }

    #[test]
    fn invalid_list_units_output_is_an_error() {
        assert!(matches!(
            parse_list_units("UNIT LOAD ACTIVE SUB DESCRIPTION"),
            Err(AppError::Io { .. })
        ));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let rows = parse_list_units(
            r#"[{"unit":"a.service","load":"loaded","active":"active","sub":"running","description":"A","following":""}]"#,
        )
        .unwrap();
        assert_eq!(rows[0].unit, "a.service");
    }

    // --- Règle de filtrage ----------------------------------------------

    #[test]
    fn only_user_services_are_kept_sorted() {
        assert_eq!(
            units(),
            [
                "api.service",
                "bot@1.service",
                "bot@2.service",
                "broken.service",
                "cron-like.service",
                "hidden.service",
                "never-started.service",
            ]
        );
    }

    #[test]
    fn system_services_are_excluded() {
        let units = units();
        assert!(!units.contains(&"dbus.service".to_owned()));
        assert!(!units.contains(&"pipewire.service".to_owned()));
        // Instance d'un gabarit absent du dossier utilisateur.
        assert!(!units.contains(&"worker@x.service".to_owned()));
    }

    #[test]
    fn not_found_units_outside_the_folder_are_excluded() {
        assert!(!units().contains(&"ghost.service".to_owned()));
    }

    #[test]
    fn failed_load_inside_the_folder_is_shown_failed() {
        let broken = service("broken.service");
        assert_eq!(broken.state, RunState::Failed);
        assert_eq!(broken.load.as_deref(), Some("bad-setting"));

        // Fichier présent mais encore inconnu de systemd (avant daemon-reload).
        let rows = parse_list_units(
            r#"[{"unit":"ghost.service","load":"not-found","active":"inactive","sub":"dead","description":"ghost.service"}]"#,
        )
        .unwrap();
        let ghost = &select_user_services(rows, &[file("ghost.service")])[0];
        assert_eq!(ghost.state, RunState::Failed);
    }

    #[test]
    fn unloaded_unit_files_are_shown_stopped() {
        assert_eq!(
            service("never-started.service"),
            Service {
                unit: "never-started.service".into(),
                load: None,
                active: "inactive".into(),
                sub: "dead".into(),
                description: None,
                state: RunState::Stopped,
                masked: false,
                controllable: true,
            }
        );
    }

    #[test]
    fn template_instances_are_included_but_not_the_template() {
        let units = units();
        assert!(units.contains(&"bot@1.service".to_owned()));
        assert!(units.contains(&"bot@2.service".to_owned()));
        assert!(!units.contains(&"bot@.service".to_owned()));
        assert_eq!(service("bot@2.service").state, RunState::Failed);
    }

    #[test]
    fn instance_with_its_own_file_is_included() {
        let rows = parse_list_units(
            r#"[{"unit":"job@daily.service","load":"loaded","active":"inactive","sub":"dead","description":"Job"}]"#,
        )
        .unwrap();
        let services = select_user_services(rows, &[file("job@daily.service")]);
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].description.as_deref(), Some("Job"));
    }

    #[test]
    fn masked_units_are_stopped_and_not_controllable() {
        let hidden = service("hidden.service");
        assert!(hidden.masked);
        assert!(!hidden.controllable);
        assert_eq!(hidden.state, RunState::Stopped);

        // Masqué et absent de la mémoire : repéré grâce au lien.
        let masked_file = UnitFile {
            name: "off.service".into(),
            masked: true,
        };
        let off = &select_user_services(vec![], &[masked_file])[0];
        assert!(off.masked);
        assert!(!off.controllable);
        assert_eq!(off.state, RunState::Stopped);
    }

    #[test]
    fn empty_folder_gives_no_service() {
        let rows = parse_list_units(LIST_UNITS).unwrap();
        assert_eq!(select_user_services(rows, &[]), vec![]);
    }

    #[test]
    fn unit_file_candidates_skip_directories_and_other_files() {
        let entries = [
            entry("api.service", EntryKind::File),
            entry("linked.service", EntryKind::Symlink),
            entry("default.target.wants", EntryKind::Dir),
            entry("api.service.d", EntryKind::Dir),
            entry("backup.timer", EntryKind::File),
            entry("api.service~", EntryKind::File),
            entry(".service", EntryKind::File),
            entry("strange.service", EntryKind::Dir),
        ];
        let names: Vec<_> = unit_file_candidates(&entries)
            .into_iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["api.service", "linked.service"]);
    }

    #[test]
    fn only_links_to_dev_null_are_masks() {
        assert!(is_mask_target("/dev/null"));
        assert!(!is_mask_target("/home/deploy/units/linked.service"));
    }

    #[test]
    fn templates_are_recognized() {
        assert_eq!(
            template_of("bot@1.service").as_deref(),
            Some("bot@.service")
        );
        assert_eq!(
            template_of("bot@a.b.service").as_deref(),
            Some("bot@.service")
        );
        assert_eq!(template_of("bot@.service"), None);
        assert_eq!(template_of("api.service"), None);
        assert_eq!(template_of("@1.service"), None);
        assert!(is_template("bot@.service"));
        assert!(!is_template("bot@1.service"));
        assert!(!is_template("@.service"));
    }

    // --- Correspondance des états ---------------------------------------

    #[test]
    fn systemd_states_map_to_run_states() {
        assert_eq!(run_state("loaded", "active"), RunState::Nominal);
        assert_eq!(run_state("loaded", "inactive"), RunState::Stopped);
        assert_eq!(run_state("loaded", "failed"), RunState::Failed);
        for transition in [
            "activating",
            "deactivating",
            "reloading",
            "refreshing",
            "maintenance",
        ] {
            assert_eq!(run_state("loaded", transition), RunState::Transition);
        }
        assert_eq!(run_state("loaded", "inconnu"), RunState::Failed);
    }

    #[test]
    fn failed_loads_are_failures() {
        assert_eq!(run_state("not-found", "inactive"), RunState::Failed);
        assert_eq!(run_state("bad-setting", "inactive"), RunState::Failed);
        assert_eq!(run_state("error", "inactive"), RunState::Failed);
        assert_eq!(run_state("masked", "inactive"), RunState::Stopped);
    }

    // --- Noms pilotables ------------------------------------------------

    #[test]
    fn unit_names_are_validated() {
        for valid in [
            "api.service",
            "bot@1.service",
            "a_b-c.d.service",
            "A.service",
        ] {
            assert!(is_valid_unit_name(valid), "{valid}");
        }
        for invalid in [
            ".service",
            "api",
            "api.timer",
            "a\\x2db.service",
            "bot; rm -rf ~.service",
            "a b.service",
            "é.service",
            "api.service\n",
        ] {
            assert!(!is_valid_unit_name(invalid), "{invalid}");
        }
    }

    /// Valeurs hostiles qui ne doivent jamais atteindre une commande.
    const HOSTILE: &[&str] = &[
        "x; rm -rf ~",
        "x; rm -rf ~.service",
        "$(rm -rf ~).service",
        "`id`.service",
        "'; rm -rf ~; '.service",
        "\"; rm -rf ~; \".service",
        "a\nb.service",
        "bot.service\nrm -rf ~",
        "bot*.service",
        "bot?.service",
        "[ab].service",
        "a b.service",
        "~/x.service",
        "../x.service",
        "",
    ];

    #[test]
    fn hostile_unit_names_are_refused() {
        for hostile in HOSTILE {
            assert!(!is_valid_unit_name(hostile), "{hostile:?}");
            for action in [Action::Start, Action::Stop, Action::Restart] {
                assert!(
                    matches!(
                        action_command(hostile, action),
                        Err(AppError::InvalidInput { .. })
                    ),
                    "{hostile:?}"
                );
            }
        }
    }

    #[test]
    fn action_commands_are_non_blocking_and_quoted() {
        assert_eq!(
            action_command("bot@1.service", Action::Start).unwrap(),
            "systemctl --user --no-block start -- 'bot@1.service'"
        );
        assert_eq!(
            action_command("api.service", Action::Stop).unwrap(),
            "systemctl --user --no-block stop -- 'api.service'"
        );
        assert_eq!(
            action_command("api.service", Action::Restart).unwrap(),
            "systemctl --user --no-block restart -- 'api.service'"
        );
        // Un nom qui commence par « - » reste un argument, pas une option.
        assert_eq!(
            action_command("-x.service", Action::Start).unwrap(),
            "systemctl --user --no-block start -- '-x.service'"
        );
    }

    // --- Logs (journalctl -o json) --------------------------------------

    #[test]
    fn logs_command_reads_the_user_journal_in_json() {
        assert_eq!(
            logs_command("bot.service", 200).unwrap(),
            "journalctl --user --unit='bot.service' -f -n 200 -o json \
             --output-fields=MESSAGE,PRIORITY --all --quiet --no-pager"
        );
        assert_eq!(
            logs_command("-x.service", 0).unwrap(),
            "journalctl --user --unit='-x.service' -f -n 0 -o json \
             --output-fields=MESSAGE,PRIORITY --all --quiet --no-pager"
        );
    }

    #[test]
    fn logs_command_refuses_hostile_names_and_long_history() {
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
            logs_command("bot.service", stream::MAX_TAIL + 1),
            Err(AppError::InvalidInput { .. })
        ));
    }

    /// Entrée réelle de `journalctl --user -o json --output-fields=MESSAGE,PRIORITY`.
    /// `message` à `None` : champ absent.
    fn journal_json(message: Option<serde_json::Value>, priority: Option<&str>) -> String {
        let mut entry = json!({
            "__CURSOR": "s=4b1f;i=1a2;b=9c;m=1f;t=6413;x=7e",
            "__REALTIME_TIMESTAMP": "1792142026900123",
            "__MONOTONIC_TIMESTAMP": "123456789",
            "_BOOT_ID": "9c0f6d2e8b3a4f1c9e7d5b3a1f0e2d4c",
        });
        if let Some(message) = message {
            entry["MESSAGE"] = message;
        }
        if let Some(priority) = priority {
            entry["PRIORITY"] = json!(priority);
        }
        entry.to_string()
    }

    #[test]
    fn string_message_with_priority_and_timestamp() {
        let lines = parse_journal_entry(&journal_json(
            Some(json!("[bot] heartbeat ack · 41 ms")),
            Some("6"),
        ));
        assert_eq!(
            lines,
            vec![LogLine {
                text: "[bot] heartbeat ack · 41 ms".into(),
                timestamp: Some("2026-10-16T09:13:46.900Z".into()),
                origin: Origin::Journal,
                priority: Some(6),
                missing: false,
            }]
        );
    }

    #[test]
    fn byte_array_message_is_decoded_with_replacement() {
        // « caf\xe9 » : Latin-1, invalide en UTF-8.
        let lines = parse_journal_entry(&journal_json(Some(json!([99, 97, 102, 233])), Some("3")));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "caf\u{fffd}");
        assert_eq!(lines[0].priority, Some(3));
        assert!(!lines[0].missing);
        // UTF-8 valide transmis en octets : décodé normalement.
        let lines = parse_journal_entry(&journal_json(
            Some(json!([99, 97, 102, 195, 169])),
            Some("6"),
        ));
        assert_eq!(lines[0].text, "café");
    }

    #[test]
    fn missing_message_is_shown_as_such() {
        let lines = parse_journal_entry(&journal_json(None, Some("5")));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, MISSING_MESSAGE);
        assert!(lines[0].missing);
        assert_eq!(lines[0].priority, Some(5));
        assert!(lines[0].timestamp.is_some());
    }

    #[test]
    fn null_message_is_shown_as_missing() {
        let lines = parse_journal_entry(&journal_json(Some(serde_json::Value::Null), Some("6")));
        assert!(lines[0].missing);
        assert_eq!(lines[0].text, MISSING_MESSAGE);
    }

    #[test]
    fn invalid_byte_array_is_shown_as_missing() {
        let lines = parse_journal_entry(&journal_json(Some(json!([99, 300, "a"])), Some("6")));
        assert!(lines[0].missing);
    }

    #[test]
    fn absent_or_invalid_priority_is_none() {
        let lines = parse_journal_entry(&journal_json(Some(json!("sans priorité")), None));
        assert_eq!(lines[0].priority, None);
        assert_eq!(lines[0].text, "sans priorité");
        for invalid in ["9", "err", ""] {
            let lines = parse_journal_entry(&journal_json(Some(json!("x")), Some(invalid)));
            assert_eq!(lines[0].priority, None, "{invalid}");
        }
    }

    #[test]
    fn multiline_message_gives_one_line_per_line() {
        let lines = parse_journal_entry(&journal_json(
            Some(json!("Traceback:\r\n  File x\n  boom\n")),
            Some("3"),
        ));
        let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["Traceback:", "  File x", "  boom"]);
        assert!(lines
            .iter()
            .all(|l| l.priority == Some(3) && l.timestamp.is_some()));
    }

    #[test]
    fn empty_message_is_one_empty_line() {
        let lines = parse_journal_entry(&journal_json(Some(json!("")), Some("6")));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "");
        assert!(!lines[0].missing);
    }

    #[test]
    fn non_json_output_is_kept_as_is() {
        let lines = parse_journal_entry("-- No entries --");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "-- No entries --");
        assert_eq!(lines[0].timestamp, None);
        assert_eq!(lines[0].priority, None);
    }

    #[test]
    fn journalctl_stderr_is_shown_as_stderr() {
        let mut out = Vec::new();
        JournalParser.parse(Raw::Stderr, "No journal files were found.".into(), &mut out);
        assert_eq!(out[0].origin, Origin::Stderr);
        assert_eq!(out[0].text, "No journal files were found.");
    }

    #[test]
    fn only_listed_controllable_units_accept_actions() {
        let named = |unit: &str, masked: bool, controllable: bool| Service {
            unit: unit.into(),
            masked,
            controllable,
            ..service("api.service")
        };
        let services = [
            named("api.service", false, true),
            named("masked.service", true, false),
            named("a\\x2db.service", false, false),
        ];

        assert!(ensure_controllable(&services, "api.service").is_ok());
        for refused in ["masked.service", "a\\x2db.service", "dbus.service"] {
            assert!(
                matches!(
                    ensure_controllable(&services, refused),
                    Err(AppError::InvalidInput { .. })
                ),
                "{refused}"
            );
        }
    }

    #[test]
    fn invalid_names_are_shown_but_not_controllable() {
        let rows = parse_list_units(
            r#"[{"unit":"a\\x2db.service","load":"loaded","active":"active","sub":"running","description":"Échappé"}]"#,
        )
        .unwrap();
        let services = select_user_services(rows, &[file("a\\x2db.service")]);
        assert_eq!(services.len(), 1);
        assert!(!services[0].controllable);
        assert_eq!(services[0].state, RunState::Nominal);
    }

    #[test]
    fn service_serializes_in_camel_case() {
        assert_eq!(
            serde_json::to_value(service("api.service")).unwrap(),
            json!({
                "unit": "api.service",
                "load": "loaded",
                "active": "active",
                "sub": "running",
                "description": "API",
                "state": "nominal",
                "masked": false,
                "controllable": true,
            })
        );
    }
}
