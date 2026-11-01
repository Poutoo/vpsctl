//! Étiquettes compose des conteneurs (fonctions pures) : validation des
//! valeurs, regroupement par projet et état agrégé.
//!
//! Les valeurs des étiquettes viennent du serveur : ce sont des données
//! externes. Elles sont validées ici, puis échappées avant d'entrer dans une
//! commande (`command.rs`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory::RememberedProject;
use crate::config::has_control_chars;
use crate::docker::{ComposePaths, Container};
use crate::run_state::RunState;
use crate::sftp::path::validate_path;

/// Longueur maximale d'un nom de projet.
const MAX_NAME_BYTES: usize = 255;
/// Chemin refusé : affiché tronqué à cette longueur.
const MAX_SHOWN_CHARS: usize = 256;

/// Dossier et fichiers d'un projet, tels que les étiquettes les donnent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectFiles {
    /// `com.docker.compose.project.working_dir`.
    pub working_dir: String,
    /// `com.docker.compose.project.config_files`, dans l'ordre de l'étiquette.
    pub config_files: Vec<String>,
    /// `com.docker.compose.project.environment_file`, posée seulement avec
    /// `--env-file` : vide pour le `.env` par défaut du dossier.
    pub env_files: Vec<String>,
}

/// Raison pour laquelle les étiquettes d'un projet sont inutilisables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ProjectProblem {
    /// Nom hors de `^[a-z0-9][a-z0-9_-]*$` : aucune commande possible.
    InvalidName,
    /// Étiquette du dossier ou des fichiers absente.
    Incomplete,
    /// Chemin non absolu, avec `.`, `..`, un composant vide ou un caractère
    /// de contrôle, ou trop long.
    InvalidPath { path: String },
    /// Un fragment de la liste n'est pas absolu : un chemin contient une
    /// virgule, la liste ne peut pas être découpée sans ambiguïté.
    AmbiguousPath { label: String },
    /// Conteneurs du projet avec des étiquettes différentes.
    Divergent,
}

/// État agrégé d'un projet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProjectState {
    Nominal,
    Partial,
    Failed,
    Transition,
    Stopped,
    /// Aucun conteneur : projet connu seulement par la mémoire du profil.
    Down,
}

/// Un projet compose, affiché comme en-tête de groupe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeProject {
    pub name: String,
    /// `None` si les étiquettes sont inutilisables (`problem`).
    pub files: Option<ProjectFiles>,
    pub problem: Option<ProjectProblem>,
    pub state: ProjectState,
    /// Conteneurs en marche, conteneurs `compose run` exclus.
    pub running: u32,
    /// Conteneurs du projet, `compose run` exclus ; pour un projet descendu,
    /// nombre mémorisé.
    pub total: u32,
}

/// Nom de projet tel que compose le normalise.
pub fn is_project_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_NAME_BYTES
        && matches!(bytes[0], b'a'..=b'z' | b'0'..=b'9')
        && bytes
            .iter()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

/// Chemin d'étiquette : absolu, sans composant vide, `.` ou `..`, sans
/// caractère de contrôle, 4 096 octets au plus.
pub fn checked_path(path: &str) -> Result<String, ProjectProblem> {
    if validate_path(path).is_ok() && !has_control_chars(path) {
        Ok(path.to_owned())
    } else {
        Err(ProjectProblem::InvalidPath {
            path: path.chars().take(MAX_SHOWN_CHARS).collect(),
        })
    }
}

/// Liste jointe par des virgules (`strings.Join(…, ",")` de compose). Tous
/// les chemins y sont absolus : un fragment qui ne l'est pas révèle une
/// virgule dans un chemin, et la liste est refusée.
fn checked_list(value: &str, label: &str) -> Result<Vec<String>, ProjectProblem> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let fragments: Vec<&str> = value.split(',').collect();
    if fragments.iter().any(|fragment| !fragment.starts_with('/')) {
        return Err(ProjectProblem::AmbiguousPath {
            label: label.to_owned(),
        });
    }
    fragments.into_iter().map(checked_path).collect()
}

/// Dossier et fichiers d'un conteneur, validés.
pub fn project_files(paths: &ComposePaths) -> Result<ProjectFiles, ProjectProblem> {
    if paths.working_dir.is_empty() || paths.config_files.is_empty() {
        return Err(ProjectProblem::Incomplete);
    }
    Ok(ProjectFiles {
        working_dir: checked_path(&paths.working_dir)?,
        config_files: checked_list(&paths.config_files, "config_files")?,
        env_files: checked_list(&paths.env_files, "environment_file")?,
    })
}

/// Dossier et fichiers relus après une mémorisation (fichier modifiable à la
/// main) : mêmes règles que les étiquettes.
pub fn checked_files(files: &ProjectFiles) -> Result<(), ProjectProblem> {
    if files.config_files.is_empty() {
        return Err(ProjectProblem::Incomplete);
    }
    checked_path(&files.working_dir)?;
    for path in files.config_files.iter().chain(&files.env_files) {
        checked_path(path)?;
    }
    Ok(())
}

/// Étiquettes communes à un projet. `members` n'est jamais vide.
fn files_of(name: &str, members: &[&Container]) -> Result<ProjectFiles, ProjectProblem> {
    if !is_project_name(name) {
        return Err(ProjectProblem::InvalidName);
    }
    let reference = &members[0].compose_paths;
    if members.iter().any(|c| &c.compose_paths != reference) {
        return Err(ProjectProblem::Divergent);
    }
    project_files(reference)
}

/// Boucle de redémarrage : un conteneur `restarting` dont le dernier code de
/// sortie est non nul (ou illisible).
fn restart_loop(container: &Container) -> bool {
    container.docker_state == "restarting" && container.exit_code != Some(0)
}

/// État agrégé des conteneurs d'un projet (`compose run` exclus), par
/// priorité : En échec (un conteneur en échec, tué par manque de mémoire
/// compris, ou en boucle de redémarrage), En transition, puis Nominal (tous
/// en marche), Arrêté (aucun) ou Partiel. Renvoie aussi le nombre en marche
/// et le total.
pub fn aggregate(services: &[&Container]) -> (ProjectState, u32, u32) {
    let total = services.len() as u32;
    let running = services
        .iter()
        .filter(|c| c.docker_state == "running")
        .count() as u32;
    let state = if total == 0 {
        ProjectState::Down
    } else if services
        .iter()
        .any(|c| c.state == RunState::Failed || restart_loop(c))
    {
        ProjectState::Failed
    } else if services.iter().any(|c| c.state == RunState::Transition) {
        ProjectState::Transition
    } else if running == total {
        ProjectState::Nominal
    } else if running == 0 {
        ProjectState::Stopped
    } else {
        ProjectState::Partial
    };
    (state, running, total)
}

/// Résultat du regroupement.
#[derive(Debug, Default)]
pub struct Grouping {
    /// Triés par nom, projets descendus compris.
    pub projects: Vec<ComposeProject>,
    /// Projets vus avec des étiquettes valides : à mémoriser.
    pub seen: Vec<RememberedProject>,
}

/// Regroupe les conteneurs par projet et ajoute les projets mémorisés qui
/// n'ont plus aucun conteneur (« Descendu »). Les étiquettes de référence
/// sont celles des conteneurs de service ; un projet qui n'a que des
/// conteneurs `compose run` prend les leurs.
pub fn group(containers: &[Container], remembered: &[RememberedProject]) -> Grouping {
    let mut by_name: BTreeMap<&str, Vec<&Container>> = BTreeMap::new();
    for container in containers {
        if let Some(project) = container.project.as_deref() {
            by_name.entry(project).or_default().push(container);
        }
    }

    let mut grouping = Grouping::default();
    for (name, members) in &by_name {
        let services: Vec<&Container> = members.iter().copied().filter(|c| !c.oneoff).collect();
        let reference = if services.is_empty() {
            members
        } else {
            &services
        };
        let files = files_of(name, reference);
        let (state, running, total) = aggregate(&services);
        if let Ok(files) = &files {
            if total > 0 {
                grouping.seen.push(RememberedProject {
                    name: (*name).to_owned(),
                    files: files.clone(),
                    containers: total,
                });
            }
        }
        let (files, problem) = match files {
            Ok(files) => (Some(files), None),
            Err(problem) => (None, Some(problem)),
        };
        grouping.projects.push(ComposeProject {
            name: (*name).to_owned(),
            files,
            problem,
            state,
            running,
            total,
        });
    }

    for project in remembered {
        if !by_name.contains_key(project.name.as_str()) {
            grouping.projects.push(ComposeProject {
                name: project.name.clone(),
                files: Some(project.files.clone()),
                problem: None,
                state: ProjectState::Down,
                running: 0,
                total: project.containers,
            });
        }
    }
    grouping.projects.sort_by(|a, b| a.name.cmp(&b.name));
    grouping
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &str, files: &str, env: &str) -> ComposePaths {
        ComposePaths {
            working_dir: dir.into(),
            config_files: files.into(),
            env_files: env.into(),
        }
    }

    fn container(name: &str, project: Option<&str>, state: &str, exit: Option<i32>) -> Container {
        let state_run = crate::docker::run_state(state, exit);
        Container {
            id: format!("{:0>64}", name.len()),
            name: name.into(),
            image: "img".into(),
            state: state_run,
            docker_state: state.into(),
            exit_code: exit,
            status: String::new(),
            duration: None,
            oom_killed: false,
            memory_limit: None,
            project: project.map(str::to_owned),
            service: Some(name.into()),
            oneoff: false,
            compose_paths: paths("/srv/app", "/srv/app/compose.yaml", ""),
        }
    }

    fn files() -> ProjectFiles {
        ProjectFiles {
            working_dir: "/srv/app".into(),
            config_files: vec!["/srv/app/compose.yaml".into()],
            env_files: vec![],
        }
    }

    // --- Validation -----------------------------------------------------

    #[test]
    fn project_names_follow_compose_normalization() {
        for ok in ["app", "urbanflow-mobility", "a_b", "0app"] {
            assert!(is_project_name(ok), "{ok}");
        }
        for bad in ["", "App", "-app", "_app", "a b", "a;b", "a'b", "a/b", "é"] {
            assert!(!is_project_name(bad), "{bad:?}");
        }
        assert!(!is_project_name(&"a".repeat(256)));
    }

    #[test]
    fn labels_give_files_in_order() {
        let found = project_files(&paths(
            "/srv/app",
            "/srv/app/compose.yaml,/srv/app/compose.prod.yaml",
            "/srv/app/.env.prod",
        ))
        .unwrap();
        assert_eq!(
            found,
            ProjectFiles {
                working_dir: "/srv/app".into(),
                config_files: vec![
                    "/srv/app/compose.yaml".into(),
                    "/srv/app/compose.prod.yaml".into()
                ],
                env_files: vec!["/srv/app/.env.prod".into()],
            }
        );
    }

    #[test]
    fn missing_labels_are_incomplete() {
        assert_eq!(
            project_files(&paths("", "/srv/app/compose.yaml", "")),
            Err(ProjectProblem::Incomplete)
        );
        assert_eq!(
            project_files(&paths("/srv/app", "", "")),
            Err(ProjectProblem::Incomplete)
        );
    }

    #[test]
    fn a_comma_inside_a_path_is_refused() {
        assert_eq!(
            project_files(&paths("/srv/app", "/srv/a,b/compose.yaml", "")),
            Err(ProjectProblem::AmbiguousPath {
                label: "config_files".into()
            })
        );
        assert_eq!(
            project_files(&paths("/srv/app", "/srv/app/compose.yaml,", "")),
            Err(ProjectProblem::AmbiguousPath {
                label: "config_files".into()
            })
        );
        assert_eq!(
            project_files(&paths("/srv/app", "/srv/app/compose.yaml", "/srv/x,y.env")),
            Err(ProjectProblem::AmbiguousPath {
                label: "environment_file".into()
            })
        );
    }

    #[test]
    fn unsafe_paths_are_refused() {
        for bad in [
            "relative/dir",
            "/srv/../etc",
            "/srv//app",
            "/srv/app/",
            "/srv/a\nb",
            "/srv/a\0b",
        ] {
            assert!(
                matches!(
                    project_files(&paths(bad, "/srv/app/compose.yaml", "")),
                    Err(ProjectProblem::InvalidPath { .. })
                ),
                "{bad:?}"
            );
        }
        // Les caractères spéciaux du shell sont acceptés : ils sont échappés.
        assert!(project_files(&paths("/srv/a b'$(x);`", "/srv/c.yaml", "")).is_ok());
    }

    // --- État agrégé ----------------------------------------------------

    fn state_of(containers: &[Container]) -> (ProjectState, u32, u32) {
        aggregate(&containers.iter().collect::<Vec<_>>())
    }

    #[test]
    fn aggregate_state_follows_priorities() {
        let up = container("a", Some("p"), "running", None);
        let clean = container("b", Some("p"), "exited", Some(0));
        let sigterm = container("c", Some("p"), "exited", Some(143));
        let crashed = container("d", Some("p"), "exited", Some(1));
        let removing = container("e", Some("p"), "removing", None);
        let created = container("f", Some("p"), "created", None);

        assert_eq!(
            state_of(&[up.clone(), up.clone()]),
            (ProjectState::Nominal, 2, 2)
        );
        assert_eq!(
            state_of(&[up.clone(), clean.clone()]),
            (ProjectState::Partial, 1, 2)
        );
        assert_eq!(
            state_of(&[clean.clone(), sigterm.clone(), created]),
            (ProjectState::Stopped, 0, 3)
        );
        assert_eq!(
            state_of(&[up.clone(), clean.clone(), crashed.clone()]),
            (ProjectState::Failed, 1, 3)
        );
        assert_eq!(
            state_of(&[up.clone(), removing.clone()]),
            (ProjectState::Transition, 1, 2)
        );
        assert_eq!(state_of(&[removing, crashed]), (ProjectState::Failed, 0, 2));
        assert_eq!(state_of(&[]), (ProjectState::Down, 0, 0));
    }

    #[test]
    fn oom_killed_container_fails_the_project_despite_code_137() {
        let mut killed = container("w", Some("p"), "exited", Some(137));
        assert_eq!(killed.state, RunState::Stopped);
        assert_eq!(
            state_of(&[container("a", Some("p"), "running", None), killed.clone()]),
            (ProjectState::Partial, 1, 2)
        );
        killed.oom_killed = true;
        killed.state = RunState::Failed;
        assert_eq!(
            state_of(&[container("a", Some("p"), "running", None), killed]),
            (ProjectState::Failed, 1, 2)
        );
    }

    #[test]
    fn restart_loop_is_a_failure_but_a_clean_restart_is_a_transition() {
        let looping = container("l", Some("p"), "restarting", Some(1));
        let unreadable = container("u", Some("p"), "restarting", None);
        let clean = container("c", Some("p"), "restarting", Some(0));
        let up = container("a", Some("p"), "running", None);
        assert_eq!(state_of(&[up.clone(), looping]).0, ProjectState::Failed);
        assert_eq!(state_of(&[up.clone(), unreadable]).0, ProjectState::Failed);
        assert_eq!(state_of(&[up, clean]).0, ProjectState::Transition);
    }

    // --- Regroupement ---------------------------------------------------

    #[test]
    fn containers_are_grouped_and_remembered() {
        let mut run = container("run", Some("app"), "exited", Some(0));
        run.oneoff = true;
        let containers = vec![
            container("web", Some("app"), "running", None),
            container("db", Some("app"), "running", None),
            run,
            container("kuma", None, "running", None),
        ];
        let grouping = group(&containers, &[]);
        assert_eq!(
            grouping.projects,
            vec![ComposeProject {
                name: "app".into(),
                files: Some(files()),
                problem: None,
                state: ProjectState::Nominal,
                running: 2,
                total: 2,
            }]
        );
        assert_eq!(
            grouping.seen,
            vec![RememberedProject {
                name: "app".into(),
                files: files(),
                containers: 2,
            }]
        );
    }

    #[test]
    fn remembered_project_without_container_is_down() {
        let remembered = [RememberedProject {
            name: "old".into(),
            files: files(),
            containers: 4,
        }];
        let grouping = group(&[container("kuma", None, "running", None)], &remembered);
        assert_eq!(
            grouping.projects,
            vec![ComposeProject {
                name: "old".into(),
                files: Some(files()),
                problem: None,
                state: ProjectState::Down,
                running: 0,
                total: 4,
            }]
        );
        assert!(grouping.seen.is_empty());
    }

    #[test]
    fn unusable_labels_are_shown_but_not_remembered() {
        let mut other = container("db", Some("app"), "running", None);
        other.compose_paths = paths("/srv/other", "/srv/other/compose.yaml", "");
        let grouping = group(
            &[container("web", Some("app"), "running", None), other],
            &[],
        );
        assert_eq!(
            grouping.projects[0].problem,
            Some(ProjectProblem::Divergent)
        );
        assert_eq!(grouping.projects[0].files, None);
        assert_eq!(grouping.projects[0].state, ProjectState::Nominal);
        assert!(grouping.seen.is_empty());

        let grouping = group(&[container("x", Some("Bad Name"), "running", None)], &[]);
        assert_eq!(
            grouping.projects[0].problem,
            Some(ProjectProblem::InvalidName)
        );
        assert!(grouping.seen.is_empty());
    }

    #[test]
    fn project_with_only_run_containers_uses_their_labels() {
        let mut run = container("run", Some("app"), "exited", Some(0));
        run.oneoff = true;
        let grouping = group(&[run], &[]);
        assert_eq!(grouping.projects[0].files, Some(files()));
        assert_eq!(grouping.projects[0].state, ProjectState::Down);
        assert_eq!(grouping.projects[0].total, 0);
        assert!(grouping.seen.is_empty());
    }
}
