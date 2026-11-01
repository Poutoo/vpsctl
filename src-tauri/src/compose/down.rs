//! Contenu du GO / NO GO de « Arrêter et supprimer » (`down`) : conteneurs,
//! réseaux, volumes et dossiers montés (fonctions pures pour les commandes de
//! lecture et leurs sorties).

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::check::FilesStatus;
use super::labels::{is_project_name, ProjectFiles};
use crate::config::has_control_chars;
use crate::docker::is_full_container_id;
use crate::error::AppError;
use crate::ssh::shell;

/// Volumes anonymes : Docker leur pose l'étiquette `com.docker.volume.anonymous`
/// à la création (`AnonymousLabel`, `daemon/volume/service/service.go` de
/// moby), jamais aux volumes nommés.
pub const ANONYMOUS_VOLUMES_COMMAND: &str =
    "docker volume ls --filter label=com.docker.volume.anonymous --format '{{.Name}}'";

/// Longueur maximale d'un nom ou d'un chemin affiché.
const MAX_SHOWN_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownContainer {
    pub name: String,
    pub service: Option<String>,
    pub running: bool,
}

/// Ce que `down` supprime et ce qu'il conserve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownInfo {
    pub project: String,
    /// Commande exacte de `down`, affichée dans le GO / NO GO.
    pub command: String,
    /// Conteneurs arrêtés puis supprimés, `compose run` compris.
    pub containers: Vec<DownContainer>,
    /// Réseaux du projet, supprimés.
    pub networks: Vec<String>,
    /// Volumes nommés montés, conservés.
    pub named_volumes: Vec<String>,
    /// Volumes anonymes montés : conservés, mais plus rattachés au projet
    /// après le `down` (un `up` suivant en crée de nouveaux).
    pub anonymous_volumes: Vec<String>,
    /// Dossiers du serveur montés (bind mounts), conservés.
    pub binds: Vec<String>,
    /// Fichiers du projet, pour la relance par Démarrer.
    pub files: Option<ProjectFiles>,
    /// Leur vérification ; `None` sans fichiers connus.
    pub files_status: Option<FilesStatus>,
}

/// Un point de montage de `docker inspect` (`MountPoint` de l'API Engine).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Mount {
    #[serde(rename = "Type", default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    /// Volume : emplacement sous `/var/lib/docker/volumes/` ; bind : chemin
    /// du serveur.
    #[serde(default)]
    pub source: String,
}

/// `docker inspect --type container --format '{{json .Mounts}}' -- '<id>'…` ;
/// `None` sans conteneur. Seuls des ID complets validés y entrent, échappés.
pub fn mounts_command(ids: &[&str]) -> Option<String> {
    let quoted: Vec<String> = ids
        .iter()
        .filter(|id| is_full_container_id(id))
        .map(|id| shell::quote(id))
        .collect();
    (!quoted.is_empty()).then(|| {
        format!(
            "docker inspect --type container --format {} -- {}",
            shell::quote("{{json .Mounts}}"),
            quoted.join(" ")
        )
    })
}

/// `docker network ls --filter 'label=com.docker.compose.project=<nom>'`.
pub fn networks_command(project: &str) -> Result<String, AppError> {
    if !is_project_name(project) {
        return Err(AppError::invalid_input("nom de projet compose invalide"));
    }
    Ok(format!(
        "docker network ls --filter {} --format {}",
        shell::quote(&format!("label=com.docker.compose.project={project}")),
        shell::quote("{{.Name}}")
    ))
}

/// Une ligne JSON par conteneur ; une ligne illisible est ignorée (le code
/// de retour n'est pas consulté : un conteneur supprimé entre-temps fait
/// échouer la commande, qui répond quand même pour les autres).
pub fn parse_mounts(output: &str) -> Vec<Mount> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Vec<Mount>>(line.trim()).ok())
        .flatten()
        .collect()
}

/// Valeur affichable : non vide, sans caractère de contrôle, longueur bornée.
fn shown(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= MAX_SHOWN_BYTES && !has_control_chars(value))
        .then(|| value.to_owned())
}

/// Noms, un par ligne, triés et sans doublon.
pub fn parse_names(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(shown)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Volume anonyme : étiquette `com.docker.volume.anonymous` (`labelled`), ou à
/// défaut nom de 64 caractères hexadécimaux, la forme que Docker donne aux
/// volumes sans nom (volume créé avant l'étiquette, étiquette illisible).
/// Le doute va vers « anonyme » : le GO / NO GO ne présente jamais comme
/// rattaché un volume que le `down` détachera.
pub fn is_anonymous_volume(name: &str, labelled: &HashSet<String>) -> bool {
    labelled.contains(name) || (name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Volumes nommés, volumes anonymes et dossiers montés, triés et sans
/// doublon. Les autres montages (`tmpfs`, `npipe`…) ne survivent pas au
/// conteneur et ne sont pas listés.
pub fn classify_mounts(
    mounts: &[Mount],
    anonymous: &HashSet<String>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut named = BTreeSet::new();
    let mut anon = BTreeSet::new();
    let mut binds = BTreeSet::new();
    for mount in mounts {
        match mount.kind.as_str() {
            "volume" => {
                if let Some(name) = shown(&mount.name) {
                    if is_anonymous_volume(&name, anonymous) {
                        anon.insert(name);
                    } else {
                        named.insert(name);
                    }
                }
            }
            "bind" => {
                if let Some(source) = shown(&mount.source) {
                    binds.insert(source);
                }
            }
            _ => {}
        }
    }
    (
        named.into_iter().collect(),
        anon.into_iter().collect(),
        binds.into_iter().collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "3f4e8a1c9b2d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f";
    const ANON: &str = "9b1d4c3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c";

    #[test]
    fn read_commands_quote_every_value() {
        assert_eq!(
            mounts_command(&[ID, "pas-un-id"]).unwrap(),
            format!("docker inspect --type container --format '{{{{json .Mounts}}}}' -- '{ID}'")
        );
        assert_eq!(mounts_command(&["pas-un-id"]), None);
        assert_eq!(
            networks_command("urbanflow-mobility").unwrap(),
            "docker network ls --filter 'label=com.docker.compose.project=urbanflow-mobility' \
             --format '{{.Name}}'"
        );
        assert!(networks_command("x';rm -rf ~;'").is_err());
    }

    /// Sortie de `docker inspect --format '{{json .Mounts}}'` (Docker 29).
    fn inspect_output() -> String {
        [
            serde_json::json!([
                { "Type": "volume", "Name": "urbanflow-mobility_pgdata",
                  "Source": "/var/lib/docker/volumes/urbanflow-mobility_pgdata/_data",
                  "Destination": "/var/lib/postgresql/data", "Driver": "local",
                  "Mode": "z", "RW": true, "Propagation": "" },
                { "Type": "volume", "Name": ANON,
                  "Source": format!("/var/lib/docker/volumes/{ANON}/_data"),
                  "Destination": "/cache", "Driver": "local", "Mode": "", "RW": true, "Propagation": "" },
            ])
            .to_string(),
            serde_json::json!([
                { "Type": "bind", "Source": "/srv/urbanflow-mobility/uploads",
                  "Destination": "/app/uploads", "Mode": "rw", "RW": true, "Propagation": "rprivate" },
                { "Type": "tmpfs", "Source": "", "Destination": "/tmp", "Mode": "", "RW": true, "Propagation": "" },
                { "Type": "volume", "Name": "urbanflow-mobility_pgdata",
                  "Source": "/var/lib/docker/volumes/urbanflow-mobility_pgdata/_data",
                  "Destination": "/backup", "Driver": "local", "Mode": "z", "RW": false, "Propagation": "" },
            ])
            .to_string(),
            "[]".to_owned(),
            "Error: No such container: abc".to_owned(),
        ]
        .join("\n")
    }

    #[test]
    fn mounts_are_split_by_kind() {
        let mounts = parse_mounts(&inspect_output());
        assert_eq!(mounts.len(), 5);
        let anonymous = HashSet::from([ANON.to_owned(), "autre".to_owned()]);
        let (named, anon, binds) = classify_mounts(&mounts, &anonymous);
        assert_eq!(named, ["urbanflow-mobility_pgdata"]);
        assert_eq!(anon, [ANON]);
        assert_eq!(binds, ["/srv/urbanflow-mobility/uploads"]);
    }

    #[test]
    fn an_unlabelled_hexadecimal_volume_counts_as_anonymous() {
        let mounts = parse_mounts(&inspect_output());
        let (named, anon, _) = classify_mounts(&mounts, &HashSet::new());
        assert_eq!(named, ["urbanflow-mobility_pgdata"]);
        assert_eq!(anon, [ANON]);
    }

    #[test]
    fn anonymity_follows_the_label_then_the_name() {
        let labelled = HashSet::from(["cache".to_owned()]);
        assert!(is_anonymous_volume("cache", &labelled));
        assert!(is_anonymous_volume(ANON, &HashSet::new()));
        assert!(is_anonymous_volume(&ANON.to_uppercase(), &HashSet::new()));
        assert!(!is_anonymous_volume("urbanflow-mobility_pgdata", &labelled));
        assert!(!is_anonymous_volume(&ANON[..63], &HashSet::new()));
        assert!(!is_anonymous_volume(
            &format!("{}g", &ANON[..63]),
            &HashSet::new()
        ));
    }

    #[test]
    fn names_are_sorted_and_cleaned() {
        assert_eq!(
            parse_names("urbanflow-mobility_default\n\nb_net\nb_net\nx\u{1b}[31m\n"),
            ["b_net", "urbanflow-mobility_default"]
        );
    }
}
