//! Projets docker compose : détection du plugin, projets lus dans les
//! étiquettes des conteneurs et mémorisés par profil.

pub mod check;
pub mod command;
pub mod down;
pub mod labels;
pub mod memory;

use serde::Serialize;
use ts_rs::TS;

pub use check::{FilesStatus, ProjectCheck};
pub use command::ComposeAction;
pub use down::DownInfo;
pub use labels::{ComposeProject, ProjectFiles, ProjectProblem, ProjectState};
pub use memory::ProjectMemory;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use crate::docker::{self, ContainerList};
use crate::error::AppError;
use crate::sftp::fs::SftpFs;
use crate::ssh::{Connection, Deadline, ExecOutput};

/// Délai des lectures du GO / NO GO de `down` (montages, réseaux, volumes
/// anonymes), après la liste.
const DOWN_INFO_TIMEOUT: Duration = Duration::from_secs(20);

/// Exécution lancée par `compose_run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeRun {
    pub run_id: String,
    /// Commande exacte, sans le script de surveillance qui l'enveloppe.
    pub command: String,
}

/// Conteneurs et projets compose du profil connecté. Les projets vus sont
/// mémorisés ; un échec d'écriture n'empêche pas l'affichage, la mémoire
/// sera mise à jour à la lecture suivante.
pub async fn list(
    connection: &Connection,
    memory: &ProjectMemory,
    profile: &str,
) -> Result<ContainerList, AppError> {
    let mut list = docker::list(connection).await?;
    let grouping = labels::group(&list.containers, &memory.projects(profile));
    let _ = memory.remember(profile, &grouping.seen);
    list.projects = grouping.projects;
    Ok(list)
}

/// Vérifie par SFTP le dossier et les fichiers de chaque projet dont les
/// étiquettes (ou la mémoire) sont utilisables, un projet après l'autre.
pub async fn check(
    connection: Arc<Connection>,
    memory: &ProjectMemory,
    profile: &str,
) -> Result<Vec<ProjectCheck>, AppError> {
    let list = docker::list(&connection).await?;
    let grouping = labels::group(&list.containers, &memory.projects(profile));
    let fs = SftpFs::shared(connection);
    let mut checks = Vec::new();
    for project in grouping.projects {
        if let Some(files) = &project.files {
            checks.push(ProjectCheck {
                status: check::check_files(&*fs, files).await?,
                project: project.name,
            });
        }
    }
    Ok(checks)
}

/// Pourquoi les étiquettes d'un projet empêchent Démarrer et Récupérer.
pub fn problem_message(problem: &ProjectProblem) -> String {
    match problem {
        ProjectProblem::InvalidName => "nom de projet hors du format de compose".to_owned(),
        ProjectProblem::Incomplete => {
            "étiquettes du dossier ou des fichiers absentes sur les conteneurs".to_owned()
        }
        ProjectProblem::InvalidPath { path } => format!("chemin refusé : {path}"),
        ProjectProblem::AmbiguousPath { label } => {
            format!("virgule dans un chemin de l'étiquette {label}")
        }
        ProjectProblem::Divergent => "étiquettes différentes d'un conteneur à l'autre".to_owned(),
    }
}

/// Pourquoi la vérification des fichiers empêche Démarrer et Récupérer ;
/// `None` si elle ne bloque rien.
pub fn status_message(status: &FilesStatus, working_dir: &str) -> Option<String> {
    match status {
        FilesStatus::Ok | FilesStatus::Unchecked { .. } => None,
        FilesStatus::DirMissing => Some(format!("dossier introuvable : {working_dir}")),
        FilesStatus::NotDir => Some(format!("{working_dir} n'est pas un dossier")),
        FilesStatus::Missing { path } => Some(format!("fichier introuvable : {path}")),
        FilesStatus::NotFile { path } => Some(format!("{path} n'est pas un fichier")),
        FilesStatus::Unreadable { path } => Some(format!(
            "fichier illisible pour le compte connecté : {path}"
        )),
    }
}

/// Projet relu sur le serveur (étiquettes, ou mémoire s'il est descendu).
async fn find_project(
    connection: &Connection,
    memory: &ProjectMemory,
    profile: &str,
    project: &str,
) -> Result<(ContainerList, ComposeProject), AppError> {
    if !labels::is_project_name(project) {
        return Err(AppError::invalid_input("nom de projet compose invalide"));
    }
    let list = docker::list(connection).await?;
    let found = labels::group(&list.containers, &memory.projects(profile))
        .projects
        .into_iter()
        .find(|p| p.name == project)
        .ok_or_else(|| AppError::invalid_input("projet compose inconnu sur ce serveur"))?;
    Ok((list, found))
}

/// Relit le projet et construit la commande d'une action. Démarrer et
/// Récupérer exigent des étiquettes utilisables et des fichiers vérifiés
/// (SFTP) ; Redémarrer et Arrêter passent par le seul nom et exigent au
/// moins un conteneur.
pub async fn prepare(
    connection: Arc<Connection>,
    memory: &ProjectMemory,
    profile: &str,
    project: &str,
    action: ComposeAction,
) -> Result<String, AppError> {
    let (_, found) = find_project(&connection, memory, profile, project).await?;
    if !action.uses_files() {
        if found.state == ProjectState::Down {
            return Err(AppError::invalid_input(
                "aucun conteneur : Démarrer recrée le projet",
            ));
        }
        return command::command_line(project, action, None);
    }
    if let Some(problem) = &found.problem {
        return Err(AppError::invalid_input(problem_message(problem)));
    }
    let files = found
        .files
        .ok_or_else(|| AppError::invalid_input("dossier et fichiers du projet inconnus"))?;
    let status = check::check_files(&*SftpFs::shared(connection), &files).await?;
    if let Some(message) = status_message(&status, &files.working_dir) {
        return Err(AppError::invalid_input(message));
    }
    command::command_line(project, action, Some(&files))
}

/// Ce que `down` supprimerait et conserverait (GO / NO GO).
pub async fn down_info(
    connection: Arc<Connection>,
    memory: &ProjectMemory,
    profile: &str,
    project: &str,
) -> Result<DownInfo, AppError> {
    let (list, found) = find_project(&connection, memory, profile, project).await?;
    let members: Vec<&docker::Container> = list
        .containers
        .iter()
        .filter(|c| c.project.as_deref() == Some(project))
        .collect();
    if members.is_empty() {
        return Err(AppError::invalid_input("aucun conteneur à supprimer"));
    }
    let deadline = Deadline::after(DOWN_INFO_TIMEOUT);
    let ids: Vec<&str> = members.iter().map(|c| c.id.as_str()).collect();
    let mounts = match down::mounts_command(&ids) {
        Some(command) => down::parse_mounts(
            &connection
                .exec_until(&command, deadline)
                .await?
                .stdout_text(),
        ),
        None => Vec::new(),
    };
    let networks = docker::checked_stdout(
        &connection
            .exec_until(&down::networks_command(project)?, deadline)
            .await?,
    )?;
    let anonymous = docker::checked_stdout(
        &connection
            .exec_until(down::ANONYMOUS_VOLUMES_COMMAND, deadline)
            .await?,
    )?;
    let anonymous: HashSet<String> = down::parse_names(&anonymous).into_iter().collect();
    let (named_volumes, anonymous_volumes, binds) = down::classify_mounts(&mounts, &anonymous);
    let files_status = match &found.files {
        Some(files) => Some(check::check_files(&*SftpFs::shared(connection), files).await?),
        None => None,
    };
    Ok(DownInfo {
        project: project.to_owned(),
        command: command::command_line(project, ComposeAction::Down, None)?,
        containers: members
            .iter()
            .map(|c| down::DownContainer {
                name: c.name.clone(),
                service: c.service.clone(),
                running: c.docker_state == "running",
            })
            .collect(),
        networks: down::parse_names(&networks),
        named_volumes,
        anonymous_volumes,
        binds,
        files: found.files,
        files_status,
    })
}

/// Oublie un projet descendu, sans rien toucher sur le serveur. Refusé si le
/// projet a encore des conteneurs (il réapparaîtrait aussitôt).
pub async fn forget(
    connection: &Connection,
    memory: &ProjectMemory,
    profile: &str,
    project: &str,
) -> Result<(), AppError> {
    let list = docker::list(connection).await?;
    if list
        .containers
        .iter()
        .any(|c| c.project.as_deref() == Some(project))
    {
        return Err(AppError::invalid_input(
            "ce projet a encore des conteneurs : il ne peut pas être oublié",
        ));
    }
    if !memory.forget(profile, project)? {
        return Err(AppError::invalid_input("projet inconnu de ce profil"));
    }
    Ok(())
}

/// Version du plugin, sans le « v » initial (`cmd/compose/version.go`).
pub const VERSION_COMMAND: &str = "docker compose version --short";

/// Présence du plugin compose sur le serveur.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum ComposeSupport {
    Present {
        version: String,
    },
    /// La commande a répondu par un échec : plugin (ou docker) absent.
    Absent,
    /// Pas de réponse exploitable (liaison, sortie inattendue) : les actions
    /// restent proposées.
    #[default]
    Unknown,
}

/// Interprète `docker compose version --short`. Le CLI docker sort avec le
/// code 1 sur une commande inconnue ; seul le code est consulté, pas le
/// texte du message.
pub fn interpret_version(result: &Result<ExecOutput, AppError>) -> ComposeSupport {
    match result {
        Ok(output) if output.success() => match parse_version(&output.stdout_text()) {
            Some(version) => ComposeSupport::Present { version },
            None => ComposeSupport::Unknown,
        },
        Ok(output) if output.exit_code.is_some() => ComposeSupport::Absent,
        _ => ComposeSupport::Unknown,
    }
}

/// `5.3.1` ; un éventuel « v » est retiré.
pub fn parse_version(output: &str) -> Option<String> {
    let version = output.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    let plausible = !version.is_empty()
        && version.len() <= 64
        && version.starts_with(|c: char| c.is_ascii_digit())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'));
    plausible.then(|| version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn output(exit_code: Option<u32>, stdout: &str) -> Result<ExecOutput, AppError> {
        Ok(ExecOutput {
            exit_code,
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    #[test]
    fn version_is_read_without_prefix() {
        assert_eq!(parse_version("5.3.1\n").as_deref(), Some("5.3.1"));
        assert_eq!(parse_version("v2.29.7").as_deref(), Some("2.29.7"));
        assert_eq!(
            parse_version("2.40.0-desktop.1").as_deref(),
            Some("2.40.0-desktop.1")
        );
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("Docker Compose version v5.3.1"), None);
        assert_eq!(parse_version("unknown command"), None);
    }

    #[test]
    fn support_follows_the_exit_code() {
        assert_eq!(
            interpret_version(&output(Some(0), "5.3.1\n")),
            ComposeSupport::Present {
                version: "5.3.1".into()
            }
        );
        assert_eq!(
            interpret_version(&output(Some(1), "")),
            ComposeSupport::Absent
        );
        assert_eq!(
            interpret_version(&output(Some(127), "")),
            ComposeSupport::Absent
        );
        assert_eq!(
            interpret_version(&output(Some(0), "???")),
            ComposeSupport::Unknown
        );
        assert_eq!(
            interpret_version(&output(None, "")),
            ComposeSupport::Unknown
        );
        assert_eq!(
            interpret_version(&Err(AppError::LinkLost)),
            ComposeSupport::Unknown
        );
    }

    #[test]
    fn support_serializes_with_a_status() {
        assert_eq!(
            serde_json::to_value(ComposeSupport::Present {
                version: "5.3.1".into()
            })
            .unwrap(),
            json!({ "status": "present", "version": "5.3.1" })
        );
        assert_eq!(
            serde_json::to_value(ComposeSupport::Absent).unwrap(),
            json!({ "status": "absent" })
        );
    }

    #[test]
    fn only_a_missing_or_unreadable_file_blocks_up_and_pull() {
        assert_eq!(status_message(&FilesStatus::Ok, "/srv/app"), None);
        assert_eq!(
            status_message(
                &FilesStatus::Unchecked {
                    message: "Failure".into()
                },
                "/srv/app"
            ),
            None
        );
        assert_eq!(
            status_message(&FilesStatus::DirMissing, "/srv/app").as_deref(),
            Some("dossier introuvable : /srv/app")
        );
        assert_eq!(
            status_message(
                &FilesStatus::Unreadable {
                    path: "/srv/app/.env".into()
                },
                "/srv/app"
            )
            .as_deref(),
            Some("fichier illisible pour le compte connecté : /srv/app/.env")
        );
    }

    #[test]
    fn compose_run_serializes_in_camel_case() {
        assert_eq!(
            serde_json::to_value(ComposeRun {
                run_id: "r1".into(),
                command: "docker compose -p 'app' down".into(),
            })
            .unwrap(),
            json!({ "runId": "r1", "command": "docker compose -p 'app' down" })
        );
    }
}
