//! Projets docker compose : détection du plugin, projets lus dans les
//! étiquettes des conteneurs et mémorisés par profil.

pub mod labels;
pub mod memory;

use serde::Serialize;
use ts_rs::TS;

pub use labels::{ComposeProject, ProjectFiles, ProjectProblem, ProjectState};
pub use memory::ProjectMemory;

use crate::docker::{self, ContainerList};
use crate::error::AppError;
use crate::ssh::{Connection, ExecOutput};

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
}
