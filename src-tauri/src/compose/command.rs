//! Commandes docker compose (fonctions pures) : construction de la ligne de
//! commande à partir de valeurs validées, chacune échappée.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::labels::{checked_files, is_project_name, ProjectFiles};
use crate::error::AppError;
use crate::ssh::shell;

/// Sortie en texte simple : `--ansi never` choisit déjà le rendu plain
/// (`selectEventProcessor` de compose) ; `--progress plain` l'impose même
/// si `COMPOSE_PROGRESS` vaut `tty` sur le serveur.
const GLOBAL_FLAGS: &str = "--ansi never --progress plain";

/// Actions de projet. Jamais d'option qui supprime des volumes ou des
/// images (`-v`, `--rmi`), ni `--remove-orphans`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ComposeAction {
    /// `up -d`
    Up,
    Pull,
    Restart,
    Down,
}

impl ComposeAction {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Pull => "pull",
            Self::Restart => "restart",
            Self::Down => "down",
        }
    }

    /// Démarrer et Récupérer lisent les fichiers du projet. Redémarrer et
    /// Arrêter passent toujours par le seul nom : avec `-f`, un service retiré
    /// du fichier depuis le dernier `up` laisserait un conteneur orphelin
    /// (`--remove-orphans` est exclu) ; sans fichier, compose reconstruit le
    /// projet depuis les conteneurs (`projectOrName`).
    pub fn uses_files(self) -> bool {
        matches!(self, Self::Up | Self::Pull)
    }

    fn arguments(self) -> &'static str {
        match self {
            Self::Up => "up -d",
            other => other.verb(),
        }
    }
}

/// Ligne de commande complète. `files` est exigé pour Démarrer et
/// Récupérer, ignoré pour Redémarrer et Arrêter. Le nom et les chemins sont
/// revalidés ici, puis échappés.
pub fn command_line(
    project: &str,
    action: ComposeAction,
    files: Option<&ProjectFiles>,
) -> Result<String, AppError> {
    if !is_project_name(project) {
        return Err(AppError::invalid_input("nom de projet compose invalide"));
    }
    let mut line = format!("docker compose {GLOBAL_FLAGS} -p {}", shell::quote(project));
    if action.uses_files() {
        let files = files
            .ok_or_else(|| AppError::invalid_input("dossier et fichiers du projet inconnus"))?;
        checked_files(files).map_err(|_| AppError::invalid_input("chemin du projet refusé"))?;
        line.push_str(" --project-directory ");
        line.push_str(&shell::quote(&files.working_dir));
        for file in &files.config_files {
            line.push_str(" -f ");
            line.push_str(&shell::quote(file));
        }
        for env in &files.env_files {
            line.push_str(" --env-file ");
            line.push_str(&shell::quote(env));
        }
    }
    line.push(' ');
    line.push_str(action.arguments());
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn files() -> ProjectFiles {
        ProjectFiles {
            working_dir: "/srv/urbanflow-mobility".into(),
            config_files: vec![
                "/srv/urbanflow-mobility/compose.yaml".into(),
                "/srv/urbanflow-mobility/compose.prod.yaml".into(),
            ],
            env_files: vec!["/srv/urbanflow-mobility/.env.prod".into()],
        }
    }

    #[test]
    fn up_and_pull_pass_every_file_in_order() {
        assert_eq!(
            command_line("urbanflow-mobility", ComposeAction::Up, Some(&files())).unwrap(),
            "docker compose --ansi never --progress plain -p 'urbanflow-mobility' \
             --project-directory '/srv/urbanflow-mobility' \
             -f '/srv/urbanflow-mobility/compose.yaml' \
             -f '/srv/urbanflow-mobility/compose.prod.yaml' \
             --env-file '/srv/urbanflow-mobility/.env.prod' up -d"
        );
        assert!(command_line("app", ComposeAction::Pull, Some(&files()))
            .unwrap()
            .ends_with("--env-file '/srv/urbanflow-mobility/.env.prod' pull"));
    }

    #[test]
    fn restart_and_down_use_the_name_only() {
        assert_eq!(
            command_line("app", ComposeAction::Restart, Some(&files())).unwrap(),
            "docker compose --ansi never --progress plain -p 'app' restart"
        );
        assert_eq!(
            command_line("app", ComposeAction::Down, None).unwrap(),
            "docker compose --ansi never --progress plain -p 'app' down"
        );
    }

    #[test]
    fn nothing_removes_volumes_or_images() {
        for action in [
            ComposeAction::Up,
            ComposeAction::Pull,
            ComposeAction::Restart,
            ComposeAction::Down,
        ] {
            let line = command_line("app", action, Some(&files())).unwrap();
            for forbidden in [" -v", "--volumes", "--rmi", "--remove-orphans"] {
                assert!(!line.contains(forbidden), "{line}");
            }
        }
    }

    #[test]
    fn shell_characters_in_paths_are_quoted() {
        let tricky = ProjectFiles {
            working_dir: "/srv/a b'$(rm -rf ~);`x`".into(),
            config_files: vec!["/srv/a b'$(rm -rf ~);`x`/compose.yaml".into()],
            env_files: vec![],
        };
        let line = command_line("app", ComposeAction::Up, Some(&tricky)).unwrap();
        assert!(line.contains(r"--project-directory '/srv/a b'\''$(rm -rf ~);`x`'"));
    }

    #[test]
    fn invalid_values_are_refused() {
        assert!(command_line("App;rm", ComposeAction::Down, None).is_err());
        assert!(command_line("-p", ComposeAction::Down, None).is_err());
        assert!(command_line("app", ComposeAction::Up, None).is_err());
        let mut relative = files();
        relative.config_files.push("relative.yaml".into());
        assert!(command_line("app", ComposeAction::Up, Some(&relative)).is_err());
        let mut newline = files();
        newline.working_dir = "/srv/a\nb".into();
        assert!(command_line("app", ComposeAction::Pull, Some(&newline)).is_err());
    }

    #[test]
    fn actions_use_the_contract_names() {
        assert_eq!(
            serde_json::to_value(ComposeAction::Up).unwrap(),
            json!("up")
        );
        assert_eq!(
            serde_json::from_value::<ComposeAction>(json!("down")).unwrap(),
            ComposeAction::Down
        );
    }
}
