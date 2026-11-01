//! Projets compose mémorisés par profil (`compose-projects.json`) : un projet
//! qui n'a plus aucun conteneur reste affiché (« Descendu ») et peut être
//! redémarré depuis son dossier. Rien n'est jamais écrit sur le serveur.
//!
//! Lecture tolérante, une fois au démarrage : un fichier absent ou invalide
//! donne une mémoire vide, une entrée invalide est ignorée sans toucher aux
//! autres. Chaque modification est écrite atomiquement avant d'être
//! appliquée en mémoire, et seulement si quelque chose change.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::labels::{checked_files, is_project_name, ProjectFiles};
use crate::config::write_json_atomic;
use crate::error::AppError;

const FILE_NAME: &str = "compose-projects.json";
const FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RememberedProject {
    pub name: String,
    #[serde(flatten)]
    pub files: ProjectFiles,
    /// Nombre de conteneurs à la dernière lecture : le N de « Descendu · 0/N ».
    pub containers: u32,
}

/// Projets par identifiant de profil.
type Profiles = BTreeMap<String, Vec<RememberedProject>>;

#[derive(Serialize)]
struct FileFormat<'a> {
    version: u32,
    profiles: &'a Profiles,
}

pub struct ProjectMemory {
    path: PathBuf,
    profiles: Mutex<Profiles>,
}

impl ProjectMemory {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(FILE_NAME);
        let profiles = fs::read_to_string(&path)
            .map(|content| parse(&content))
            .unwrap_or_default();
        Self {
            path,
            profiles: Mutex::new(profiles),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Profiles>, AppError> {
        self.profiles
            .lock()
            .map_err(|_| AppError::io("accès aux projets compose mémorisés impossible"))
    }

    pub fn projects(&self, profile: &str) -> Vec<RememberedProject> {
        self.lock()
            .ok()
            .and_then(|profiles| profiles.get(profile).cloned())
            .unwrap_or_default()
    }

    /// Ajoute ou met à jour les projets vus.
    pub fn remember(&self, profile: &str, seen: &[RememberedProject]) -> Result<(), AppError> {
        self.modify(|profiles| {
            let list = profiles.entry(profile.to_owned()).or_default();
            merge(list, seen)
        })
    }

    /// Oublie un projet ; `false` s'il n'était pas mémorisé.
    pub fn forget(&self, profile: &str, name: &str) -> Result<bool, AppError> {
        let mut found = false;
        self.modify(|profiles| {
            let Some(list) = profiles.get_mut(profile) else {
                return false;
            };
            let before = list.len();
            list.retain(|project| project.name != name);
            found = list.len() != before;
            if list.is_empty() {
                profiles.remove(profile);
            }
            found
        })?;
        Ok(found)
    }

    /// Profil supprimé : ses projets sont oubliés.
    pub fn forget_profile(&self, profile: &str) -> Result<(), AppError> {
        self.modify(|profiles| profiles.remove(profile).is_some())
    }

    /// Applique `change` à une copie ; si elle a changé, l'écrit puis la garde.
    fn modify(&self, change: impl FnOnce(&mut Profiles) -> bool) -> Result<(), AppError> {
        let mut current = self.lock()?;
        let mut next = current.clone();
        if !change(&mut next) {
            return Ok(());
        }
        write_json_atomic(
            &self.path,
            &FileFormat {
                version: FILE_VERSION,
                profiles: &next,
            },
            "des projets compose",
        )?;
        *current = next;
        Ok(())
    }
}

/// Ajoute ou remplace les projets vus, triés par nom. Renvoie `true` si la
/// liste a changé.
pub fn merge(list: &mut Vec<RememberedProject>, seen: &[RememberedProject]) -> bool {
    let mut changed = false;
    for project in seen {
        match list.iter_mut().find(|known| known.name == project.name) {
            Some(known) if known == project => {}
            Some(known) => {
                *known = project.clone();
                changed = true;
            }
            None => {
                list.push(project.clone());
                changed = true;
            }
        }
    }
    if changed {
        list.sort_by(|a, b| a.name.cmp(&b.name));
    }
    changed
}

/// Lecture tolérante : chaque entrée est relue et validée séparément.
pub fn parse(content: &str) -> Profiles {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(content) else {
        return Profiles::new();
    };
    let Some(Value::Object(profiles)) = root.get("profiles") else {
        return Profiles::new();
    };
    profiles
        .iter()
        .filter_map(|(profile, entries)| {
            let mut projects: Vec<RememberedProject> = entries
                .as_array()?
                .iter()
                .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                .filter(|project: &RememberedProject| {
                    is_project_name(&project.name) && checked_files(&project.files).is_ok()
                })
                .collect();
            projects.sort_by(|a, b| a.name.cmp(&b.name));
            projects.dedup_by(|a, b| a.name == b.name);
            (!projects.is_empty()).then(|| (profile.clone(), projects))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project(name: &str, containers: u32) -> RememberedProject {
        RememberedProject {
            name: name.into(),
            files: ProjectFiles {
                working_dir: format!("/srv/{name}"),
                config_files: vec![format!("/srv/{name}/compose.yaml")],
                env_files: vec![],
            },
            containers,
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-compose-{tag}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn merge_adds_and_updates_only_on_change() {
        let mut list = vec![project("b", 2)];
        assert!(!merge(&mut list, &[project("b", 2)]));
        assert!(merge(&mut list, &[project("a", 1), project("b", 3)]));
        assert_eq!(list, vec![project("a", 1), project("b", 3)]);
    }

    #[test]
    fn parse_is_tolerant_entry_by_entry() {
        let content = json!({
            "version": 1,
            "profiles": {
                "p1": [
                    { "name": "app", "workingDir": "/srv/app", "configFiles": ["/srv/app/compose.yaml"], "envFiles": [], "containers": 2 },
                    { "name": "Bad", "workingDir": "/srv/x", "configFiles": ["/srv/x/c.yaml"], "envFiles": [], "containers": 1 },
                    { "name": "rel", "workingDir": "srv", "configFiles": ["/srv/c.yaml"], "envFiles": [], "containers": 1 },
                    { "name": "nofile", "workingDir": "/srv", "configFiles": [], "envFiles": [], "containers": 1 },
                    { "name": "partial" },
                    "texte"
                ],
                "p2": "pas une liste"
            }
        })
        .to_string();
        let profiles = parse(&content);
        assert_eq!(profiles.len(), 1);
        assert_eq!(
            profiles["p1"],
            vec![RememberedProject {
                name: "app".into(),
                files: ProjectFiles {
                    working_dir: "/srv/app".into(),
                    config_files: vec!["/srv/app/compose.yaml".into()],
                    env_files: vec![],
                },
                containers: 2,
            }]
        );
        assert!(parse("pas du json").is_empty());
        assert!(parse("[]").is_empty());
    }

    #[test]
    fn memory_is_kept_per_profile_and_survives_a_reload() {
        let dir = temp_dir("reload");
        let memory = ProjectMemory::load(&dir);
        memory.remember("p1", &[project("app", 2)]).unwrap();
        memory.remember("p2", &[project("web", 1)]).unwrap();

        let reloaded = ProjectMemory::load(&dir);
        assert_eq!(reloaded.projects("p1"), vec![project("app", 2)]);
        assert_eq!(reloaded.projects("p2"), vec![project("web", 1)]);
        assert!(reloaded.projects("p3").is_empty());

        assert!(reloaded.forget("p1", "app").unwrap());
        assert!(!reloaded.forget("p1", "app").unwrap());
        reloaded.forget_profile("p2").unwrap();
        let again = ProjectMemory::load(&dir);
        assert!(again.projects("p1").is_empty());
        assert!(again.projects("p2").is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unchanged_list_writes_nothing() {
        let dir = temp_dir("unchanged");
        let memory = ProjectMemory::load(&dir);
        memory.remember("p1", &[]).unwrap();
        assert!(!dir.join(FILE_NAME).exists());
        memory.remember("p1", &[project("app", 2)]).unwrap();
        let written = fs::metadata(dir.join(FILE_NAME))
            .unwrap()
            .modified()
            .unwrap();
        memory.remember("p1", &[project("app", 2)]).unwrap();
        assert_eq!(
            fs::metadata(dir.join(FILE_NAME))
                .unwrap()
                .modified()
                .unwrap(),
            written
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
