//! Lecture et écriture des profils de connexion en JSON dans le dossier de
//! configuration de l'application (fourni par l'appelant : ce module ne
//! dépend pas de Tauri), et écriture atomique des fichiers JSON, partagée
//! avec les scripts.
//!
//! Règles d'enregistrement d'un profil :
//! - l'empreinte de la clé hôte n'est jamais modifiée par `save_profile` :
//!   seule la commande `trust_host_key` peut l'écrire ;
//! - si l'hôte ou le port change, l'empreinte mémorisée est effacée ;
//! - un seul profil peut être marqué par défaut.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;

const FILE_NAME: &str = "profiles.json";
const FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Profile {
    /// Vide à la création : l'identifiant est attribué par le backend.
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: String,
    pub host_key_fingerprint: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfilesFile {
    version: u32,
    profiles: Vec<Profile>,
}

/// Accès au fichier des profils. Les appels concurrents doivent être
/// sérialisés par l'appelant (verrou dans l'état partagé).
#[derive(Debug, Clone)]
pub struct ProfileStore {
    path: PathBuf,
}

impl ProfileStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join(FILE_NAME),
        }
    }

    pub fn list(&self) -> Result<Vec<Profile>, AppError> {
        let file: Option<ProfilesFile> = read_json(&self.path, "des profils")?;
        Ok(file.map(|f| f.profiles).unwrap_or_default())
    }

    pub fn get(&self, id: &str) -> Result<Profile, AppError> {
        self.list()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::invalid_input("profil introuvable"))
    }

    pub fn save(&self, incoming: Profile) -> Result<Profile, AppError> {
        let mut profiles = self.list()?;
        let saved = apply_save(&mut profiles, incoming, || uuid::Uuid::new_v4().to_string())?;
        self.write(&profiles)?;
        Ok(saved)
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let mut profiles = self.list()?;
        let before = profiles.len();
        profiles.retain(|p| p.id != id);
        if profiles.len() == before {
            return Err(AppError::invalid_input("profil introuvable"));
        }
        self.write(&profiles)
    }

    pub fn set_fingerprint(&self, id: &str, fingerprint: &str) -> Result<(), AppError> {
        let mut profiles = self.list()?;
        let profile = profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::invalid_input("profil introuvable"))?;
        profile.host_key_fingerprint = Some(fingerprint.to_owned());
        self.write(&profiles)
    }

    fn write(&self, profiles: &[Profile]) -> Result<(), AppError> {
        let file = ProfilesFile {
            version: FILE_VERSION,
            profiles: profiles.to_vec(),
        };
        write_json_atomic(&self.path, &file, "des profils")
    }
}

/// Lit un fichier JSON de configuration ; `None` s'il n'existe pas.
/// `what` complète les messages d'erreur (« des profils »).
pub fn read_json<T: DeserializeOwned>(path: &Path, what: &str) -> Result<Option<T>, AppError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .map_err(|e| AppError::io(format!("lecture {what} impossible : {e}")))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|e| AppError::io(format!("fichier {what} invalide : {e}")))
}

/// Écriture atomique d'un fichier JSON : fichier temporaire puis renommage.
/// Le dossier est créé au besoin.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T, what: &str) -> Result<(), AppError> {
    let to_io = |e: std::io::Error| AppError::io(format!("écriture {what} impossible : {e}"));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(to_io)?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| AppError::io(format!("sérialisation {what} impossible : {e}")))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(to_io)?;
    fs::rename(&tmp, path).map_err(to_io)
}

/// Applique les règles d'enregistrement sur la liste en mémoire.
fn apply_save(
    profiles: &mut Vec<Profile>,
    incoming: Profile,
    new_id: impl FnOnce() -> String,
) -> Result<Profile, AppError> {
    let mut profile = normalize(incoming)?;

    if profile.id.is_empty() {
        profile.id = new_id();
        profile.host_key_fingerprint = None;
    } else {
        let stored = profiles
            .iter()
            .find(|p| p.id == profile.id)
            .ok_or_else(|| AppError::invalid_input("profil introuvable"))?;
        let endpoint_changed = stored.host != profile.host || stored.port != profile.port;
        profile.host_key_fingerprint = if endpoint_changed {
            None
        } else {
            stored.host_key_fingerprint.clone()
        };
    }

    if profile.is_default {
        for other in profiles.iter_mut() {
            other.is_default = false;
        }
    }

    match profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(slot) => *slot = profile.clone(),
        None => profiles.push(profile.clone()),
    }
    Ok(profile)
}

/// Supprime les espaces superflus et valide chaque champ.
fn normalize(mut profile: Profile) -> Result<Profile, AppError> {
    profile.id = profile.id.trim().to_owned();
    profile.name = profile.name.trim().to_owned();
    profile.host = profile.host.trim().to_owned();
    profile.user = profile.user.trim().to_owned();
    profile.key_path = profile.key_path.trim().to_owned();

    if profile.name.is_empty() || profile.name.chars().count() > 64 {
        return Err(AppError::invalid_input(
            "le nom du profil est requis (64 caractères au plus)",
        ));
    }
    if has_control_chars(&profile.name) {
        return Err(AppError::invalid_input(
            "le nom du profil contient des caractères invalides",
        ));
    }
    if !is_valid_host(&profile.host) {
        return Err(AppError::invalid_input(
            "hôte invalide : nom de domaine ou adresse IP attendu",
        ));
    }
    if profile.port == 0 {
        return Err(AppError::invalid_input(
            "le port doit être compris entre 1 et 65535",
        ));
    }
    if !is_valid_user(&profile.user) {
        return Err(AppError::invalid_input(
            "utilisateur invalide : lettres, chiffres, « _ », « . » et « - » uniquement",
        ));
    }
    if profile.key_path.is_empty() || has_control_chars(&profile.key_path) {
        return Err(AppError::invalid_input(
            "le chemin de la clé privée est requis",
        ));
    }
    Ok(profile)
}

pub fn has_control_chars(value: &str) -> bool {
    value.chars().any(char::is_control)
}

/// Nom de domaine, IPv4 ou IPv6 (éventuellement entre crochets).
fn is_valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
}

fn is_valid_user(user: &str) -> bool {
    !user.is_empty()
        && user.len() <= 32
        && !user.starts_with('-')
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile(id: &str) -> Profile {
        Profile {
            id: id.to_owned(),
            name: "vps-prod".to_owned(),
            host: "vps.example".to_owned(),
            port: 22,
            user: "deploy".to_owned(),
            key_path: "~/.ssh/id_ed25519".to_owned(),
            host_key_fingerprint: None,
            is_default: false,
        }
    }

    fn trusted(id: &str) -> Profile {
        Profile {
            host_key_fingerprint: Some("SHA256:empreinte".to_owned()),
            ..profile(id)
        }
    }

    #[test]
    fn new_profile_gets_an_id_and_never_an_incoming_fingerprint() {
        let mut profiles = Vec::new();
        let saved = apply_save(&mut profiles, trusted(""), || "id-1".to_owned()).unwrap();
        assert_eq!(saved.id, "id-1");
        assert_eq!(saved.host_key_fingerprint, None);
        assert_eq!(profiles, vec![saved]);
    }

    #[test]
    fn host_change_clears_the_fingerprint() {
        let mut profiles = vec![trusted("a")];
        let edited = Profile {
            host: "autre.example".to_owned(),
            ..trusted("a")
        };
        let saved = apply_save(&mut profiles, edited, || unreachable!()).unwrap();
        assert_eq!(saved.host_key_fingerprint, None);
    }

    #[test]
    fn port_change_clears_the_fingerprint() {
        let mut profiles = vec![trusted("a")];
        let edited = Profile {
            port: 2222,
            ..trusted("a")
        };
        let saved = apply_save(&mut profiles, edited, || unreachable!()).unwrap();
        assert_eq!(saved.host_key_fingerprint, None);
    }

    #[test]
    fn other_changes_keep_the_stored_fingerprint() {
        let mut profiles = vec![trusted("a")];
        let edited = Profile {
            name: "renommé".to_owned(),
            user: "admin".to_owned(),
            host_key_fingerprint: None,
            ..trusted("a")
        };
        let saved = apply_save(&mut profiles, edited, || unreachable!()).unwrap();
        assert_eq!(
            saved.host_key_fingerprint.as_deref(),
            Some("SHA256:empreinte")
        );
    }

    #[test]
    fn incoming_fingerprint_is_ignored_on_update() {
        let mut profiles = vec![profile("a")];
        let edited = Profile {
            host_key_fingerprint: Some("SHA256:forgee".to_owned()),
            ..profile("a")
        };
        let saved = apply_save(&mut profiles, edited, || unreachable!()).unwrap();
        assert_eq!(saved.host_key_fingerprint, None);
    }

    #[test]
    fn only_one_default_profile() {
        let mut profiles = vec![
            Profile {
                is_default: true,
                ..profile("a")
            },
            profile("b"),
        ];
        let b_default = Profile {
            is_default: true,
            ..profile("b")
        };
        apply_save(&mut profiles, b_default, || unreachable!()).unwrap();
        let defaults: Vec<_> = profiles
            .iter()
            .filter(|p| p.is_default)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(defaults, vec!["b"]);
    }

    #[test]
    fn unknown_id_is_rejected() {
        let mut profiles = vec![profile("a")];
        let err = apply_save(&mut profiles, profile("zzz"), || unreachable!()).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput { .. }));
    }

    #[test]
    fn invalid_fields_are_rejected() {
        let cases = [
            Profile {
                name: "  ".to_owned(),
                ..profile("")
            },
            Profile {
                host: "vps example".to_owned(),
                ..profile("")
            },
            Profile {
                host: "vps;rm".to_owned(),
                ..profile("")
            },
            Profile {
                port: 0,
                ..profile("")
            },
            Profile {
                user: "-oProxyCommand".to_owned(),
                ..profile("")
            },
            Profile {
                user: "de ploy".to_owned(),
                ..profile("")
            },
            Profile {
                key_path: String::new(),
                ..profile("")
            },
        ];
        for case in cases {
            let mut profiles = Vec::new();
            let result = apply_save(&mut profiles, case.clone(), || "x".to_owned());
            assert!(
                matches!(result, Err(AppError::InvalidInput { .. })),
                "accepté à tort : {case:?}"
            );
        }
    }

    #[test]
    fn fields_are_trimmed() {
        let mut profiles = Vec::new();
        let saved = apply_save(
            &mut profiles,
            Profile {
                host: "  vps.example ".to_owned(),
                user: " deploy".to_owned(),
                ..profile("")
            },
            || "id".to_owned(),
        )
        .unwrap();
        assert_eq!(saved.host, "vps.example");
        assert_eq!(saved.user, "deploy");
    }

    #[test]
    fn profile_serializes_in_camel_case() {
        assert_eq!(
            serde_json::to_value(trusted("a")).unwrap(),
            json!({
                "id": "a",
                "name": "vps-prod",
                "host": "vps.example",
                "port": 22,
                "user": "deploy",
                "keyPath": "~/.ssh/id_ed25519",
                "hostKeyFingerprint": "SHA256:empreinte",
                "isDefault": false,
            })
        );
    }

    /// Dossier temporaire supprimé à la fin du test.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("vpsctl-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn store_round_trip_on_disk() {
        let dir = TempDir::new();
        let store = ProfileStore::new(&dir.0.join("sous-dossier"));

        assert_eq!(store.list().unwrap(), Vec::new());

        let saved = store.save(profile("")).unwrap();
        store.set_fingerprint(&saved.id, "SHA256:abc").unwrap();
        let reloaded = store.get(&saved.id).unwrap();
        assert_eq!(reloaded.host_key_fingerprint.as_deref(), Some("SHA256:abc"));

        store.delete(&saved.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(store.delete(&saved.id).is_err());
    }
}
