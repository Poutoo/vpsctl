//! Services systemd utilisateur (`systemctl --user`, `journalctl --user`) :
//! liste en JSON (parsing dans des fonctions pures), actions, flux de logs,
//! création et suppression des fichiers d'unité dans `~/.config/systemd/user/`
//! suivies de `daemon-reload`.
//! Noms de services validés par `^[a-zA-Z0-9@._-]+\.service$`.
//!
//! Liste : unités en mémoire (`list-units` en JSON) et fichiers d'unité du
//! dossier utilisateur (lus par SFTP), croisés par `select_user_services`.
//! Règle de filtrage et correspondance des états : `docs/03-architecture.md`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;
use crate::run_state::{Action, RunState};
use crate::sftp::{self, DirEntry, EntryKind};
use crate::ssh::{self, shell, Connection, Deadline};

const LIST_COMMAND: &str =
    "systemctl --user list-units --type=service --all --output=json --no-pager";
/// Dossier des fichiers d'unité de l'utilisateur, relatif au dossier personnel
/// (point de départ des chemins SFTP).
pub const UNIT_DIR: &str = ".config/systemd/user";
/// Délai maximal de `services_list`.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(10);

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
    let deadline = Deadline::after(LIST_TIMEOUT);
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
