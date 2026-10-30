//! Préférences de l'interface, dans `preferences.json` du dossier de
//! configuration, séparé des profils.
//!
//! Lecture tolérante : un fichier absent, illisible ou invalide donne les
//! valeurs par défaut, et chaque champ manquant ou hors limites reprend la
//! sienne sans toucher aux autres. Le démarrage n'est jamais bloqué.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::config::write_json_atomic;
use crate::error::AppError;

const FILE_NAME: &str = "preferences.json";
const FILE_VERSION: u32 = 1;

/// Assombrissement de l'image de fond par défaut, en pourcentage.
pub const DEFAULT_DIMMING: u8 = 85;
pub const MAX_DIMMING: u8 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Preferences {
    /// Assombrissement de l'image de fond, de 0 (image nette) à 100 (masquée).
    pub dimming: u8,
    /// Image de fond en couleurs, sans la désaturation habituelle.
    pub original_colors: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            dimming: DEFAULT_DIMMING,
            original_colors: false,
        }
    }
}

#[derive(Serialize)]
struct PreferencesFile<'a> {
    version: u32,
    #[serde(flatten)]
    preferences: &'a Preferences,
}

/// Préférences en mémoire et leur fichier. Chaque modification est écrite
/// aussitôt ; la valeur en mémoire ne change qu'une fois l'écriture réussie.
#[derive(Debug)]
pub struct PreferenceStore {
    path: PathBuf,
    current: Mutex<Preferences>,
}

impl PreferenceStore {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(FILE_NAME);
        let current = fs::read_to_string(&path)
            .map(|content| parse(&content))
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Preferences {
        self.lock().clone()
    }

    pub fn set_dimming(&self, dimming: u8) -> Result<Preferences, AppError> {
        if dimming > MAX_DIMMING {
            return Err(AppError::invalid_input(
                "l'assombrissement doit être compris entre 0 et 100",
            ));
        }
        self.update(|p| p.dimming = dimming)
    }

    pub fn set_original_colors(&self, enabled: bool) -> Result<Preferences, AppError> {
        self.update(|p| p.original_colors = enabled)
    }

    fn update(&self, change: impl FnOnce(&mut Preferences)) -> Result<Preferences, AppError> {
        let mut current = self.lock();
        let mut next = current.clone();
        change(&mut next);
        let file = PreferencesFile {
            version: FILE_VERSION,
            preferences: &next,
        };
        write_json_atomic(&self.path, &file, "des préférences")?;
        *current = next.clone();
        Ok(next)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Preferences> {
        // Un verrou empoisonné garde une valeur cohérente : elle n'est
        // remplacée qu'en bloc, après une écriture réussie.
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Lit le contenu du fichier champ par champ (fonction pure).
fn parse(content: &str) -> Preferences {
    let defaults = Preferences::default();
    let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(content) else {
        return defaults;
    };
    let dimming = fields
        .get("dimming")
        .and_then(Value::as_u64)
        .filter(|d| *d <= u64::from(MAX_DIMMING))
        .map_or(defaults.dimming, |d| d as u8);
    let original_colors = fields
        .get("originalColors")
        .and_then(Value::as_bool)
        .unwrap_or(defaults.original_colors);
    Preferences {
        dimming,
        original_colors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-preferences-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_dim_by_85_percent_in_grayscale() {
        assert_eq!(
            Preferences::default(),
            Preferences {
                dimming: 85,
                original_colors: false
            }
        );
    }

    #[test]
    fn invalid_content_gives_defaults() {
        for content in ["", "{", "null", "[]", "42", "\"texte\""] {
            assert_eq!(parse(content), Preferences::default(), "{content:?}");
        }
    }

    #[test]
    fn each_field_falls_back_on_its_own() {
        let prefs = parse(r#"{"version":1,"dimming":150,"originalColors":true}"#);
        assert_eq!(prefs.dimming, DEFAULT_DIMMING);
        assert!(prefs.original_colors);

        let prefs = parse(r#"{"dimming":40,"originalColors":"oui"}"#);
        assert_eq!(prefs.dimming, 40);
        assert!(!prefs.original_colors);

        for dimming in ["-1", "12.5", "\"50\"", "null"] {
            let prefs = parse(&format!(r#"{{"dimming":{dimming}}}"#));
            assert_eq!(prefs.dimming, DEFAULT_DIMMING, "{dimming}");
        }
    }

    #[test]
    fn unknown_fields_and_versions_are_ignored() {
        let prefs = parse(r#"{"version":9,"dimming":0,"inconnu":{"a":1}}"#);
        assert_eq!(prefs.dimming, 0);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = temp_dir("absent");
        assert_eq!(PreferenceStore::load(&dir).get(), Preferences::default());
    }

    #[test]
    fn changes_are_written_and_read_back() {
        let dir = temp_dir("aller-retour");
        let store = PreferenceStore::load(&dir);
        store.set_dimming(60).unwrap();
        store.set_original_colors(true).unwrap();

        let written = fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        let json: Value = serde_json::from_str(&written).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["dimming"], 60);
        assert_eq!(json["originalColors"], true);

        let reloaded = PreferenceStore::load(&dir).get();
        assert_eq!(
            reloaded,
            Preferences {
                dimming: 60,
                original_colors: true
            }
        );
    }

    #[test]
    fn invalid_file_is_replaced_on_first_change() {
        let dir = temp_dir("remplace");
        fs::write(dir.join(FILE_NAME), "pas du json").unwrap();
        let store = PreferenceStore::load(&dir);
        assert_eq!(store.get(), Preferences::default());
        store.set_original_colors(true).unwrap();
        assert!(PreferenceStore::load(&dir).get().original_colors);
    }

    #[test]
    fn dimming_above_100_is_refused_and_nothing_changes() {
        let dir = temp_dir("refus");
        let store = PreferenceStore::load(&dir);
        assert!(matches!(
            store.set_dimming(101),
            Err(AppError::InvalidInput { .. })
        ));
        assert_eq!(store.get(), Preferences::default());
        assert!(!dir.join(FILE_NAME).exists());
    }
}
