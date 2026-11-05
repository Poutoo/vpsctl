//! Seuils d'alerte propres à chaque profil (`resource-thresholds.json`).
//!
//! Lecture tolérante, une fois au démarrage : un fichier absent ou invalide,
//! ou l'entrée invalide d'un profil, donne les valeurs par défaut. Chaque
//! modification est écrite atomiquement avant d'être appliquée en mémoire,
//! et seulement si quelque chose change.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::config::write_json_atomic;
use crate::error::AppError;

const FILE_NAME: &str = "resource-thresholds.json";
const FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThresholdPair {
    pub attention: f64,
    pub critical: f64,
}

/// Disque et mémoire en %, charge par cœur.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Thresholds {
    pub disk: ThresholdPair,
    pub memory: ThresholdPair,
    pub load: ThresholdPair,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            disk: ThresholdPair {
                attention: 80.0,
                critical: 90.0,
            },
            memory: ThresholdPair {
                attention: 85.0,
                critical: 95.0,
            },
            load: ThresholdPair {
                attention: 1.0,
                critical: 2.0,
            },
        }
    }
}

const LOAD_MAX: f64 = 64.0;

/// Disque et mémoire : entiers de 1 à 100 ; charge : de 0,1 à 64 au dixième ;
/// attention strictement inférieure à critique. Le message nomme le champ.
pub fn validate(thresholds: &Thresholds) -> Result<(), AppError> {
    let percent = |value: f64, field: &str| -> Result<(), AppError> {
        if value.fract() != 0.0 || !(1.0..=100.0).contains(&value) {
            return Err(AppError::invalid_input(format!(
                "{field} : un nombre entier de 1 à 100 est attendu"
            )));
        }
        Ok(())
    };
    let load = |value: f64, field: &str| -> Result<(), AppError> {
        let tenths = value * 10.0;
        if !value.is_finite()
            || (tenths - tenths.round()).abs() > 1e-6
            || !(0.1 - 1e-9..=LOAD_MAX).contains(&value)
        {
            return Err(AppError::invalid_input(format!(
                "{field} : une valeur de 0,1 à 64 par cœur, au dixième, est attendue"
            )));
        }
        Ok(())
    };
    let ordered = |pair: ThresholdPair, name: &str| -> Result<(), AppError> {
        if pair.attention >= pair.critical {
            return Err(AppError::invalid_input(format!(
                "{name} : le seuil d'attention doit être inférieur au seuil critique"
            )));
        }
        Ok(())
    };
    percent(thresholds.disk.attention, "disque, attention")?;
    percent(thresholds.disk.critical, "disque, critique")?;
    percent(thresholds.memory.attention, "mémoire, attention")?;
    percent(thresholds.memory.critical, "mémoire, critique")?;
    load(thresholds.load.attention, "charge, attention")?;
    load(thresholds.load.critical, "charge, critique")?;
    ordered(thresholds.disk, "disque")?;
    ordered(thresholds.memory, "mémoire")?;
    ordered(thresholds.load, "charge")
}

type Profiles = BTreeMap<String, Thresholds>;

#[derive(Serialize)]
struct FileFormat<'a> {
    version: u32,
    profiles: &'a Profiles,
}

pub struct ThresholdStore {
    path: PathBuf,
    profiles: Mutex<Profiles>,
}

impl ThresholdStore {
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
            .map_err(|_| AppError::io("accès aux seuils d'alerte impossible"))
    }

    /// Seuils d'un profil, ou les valeurs par défaut.
    pub fn get(&self, profile: &str) -> Thresholds {
        self.lock()
            .ok()
            .and_then(|profiles| profiles.get(profile).copied())
            .unwrap_or_default()
    }

    pub fn save(&self, profile: &str, thresholds: Thresholds) -> Result<Thresholds, AppError> {
        validate(&thresholds)?;
        self.modify(|profiles| {
            profiles.insert(profile.to_owned(), thresholds) != Some(thresholds)
        })?;
        Ok(thresholds)
    }

    /// « Valeurs par défaut » : l'entrée du profil est retirée.
    pub fn reset(&self, profile: &str) -> Result<Thresholds, AppError> {
        self.modify(|profiles| profiles.remove(profile).is_some())?;
        Ok(Thresholds::default())
    }

    /// Profil supprimé.
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
            "des seuils d'alerte",
        )?;
        *current = next;
        Ok(())
    }
}

/// Entrées valides du fichier ; les autres sont ignorées.
fn parse(content: &str) -> Profiles {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(content) else {
        return Profiles::new();
    };
    let Some(Value::Object(profiles)) = root.get("profiles") else {
        return Profiles::new();
    };
    profiles
        .iter()
        .filter_map(|(id, value)| {
            let thresholds: Thresholds = serde_json::from_value(value.clone()).ok()?;
            validate(&thresholds).ok()?;
            Some((id.clone(), thresholds))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_cases::{check_valid, shared_cases};

    fn dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vpsctl-thresholds-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn shared_validation_cases() {
        check_valid::<Thresholds>(shared_cases!("resource-thresholds.json"), |t| {
            validate(t).is_ok()
        });
    }

    #[test]
    fn defaults_follow_the_specification() {
        let t = Thresholds::default();
        assert_eq!((t.disk.attention, t.disk.critical), (80.0, 90.0));
        assert_eq!((t.memory.attention, t.memory.critical), (85.0, 95.0));
        assert_eq!((t.load.attention, t.load.critical), (1.0, 2.0));
        assert!(validate(&t).is_ok());
    }

    #[test]
    fn thresholds_are_per_profile_and_reset_alone() {
        let dir = dir();
        let store = ThresholdStore::load(&dir);
        let custom = Thresholds {
            disk: ThresholdPair {
                attention: 50.0,
                critical: 60.0,
            },
            ..Thresholds::default()
        };
        store.save("a", custom).unwrap();
        store.save("b", Thresholds::default()).unwrap();
        assert_eq!(store.get("a"), custom);
        assert_eq!(store.get("b"), Thresholds::default());
        assert_eq!(store.get("inconnu"), Thresholds::default());

        // Relu depuis le fichier.
        assert_eq!(ThresholdStore::load(&dir).get("a"), custom);

        store.reset("a").unwrap();
        assert_eq!(store.get("a"), Thresholds::default());
        let reloaded = ThresholdStore::load(&dir);
        assert_eq!(reloaded.get("a"), Thresholds::default());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_thresholds_are_refused_and_not_written() {
        let dir = dir();
        let store = ThresholdStore::load(&dir);
        let mut bad = Thresholds::default();
        bad.memory.attention = 96.0;
        assert!(store.save("a", bad).is_err());
        assert!(!dir.join(FILE_NAME).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tolerant_reading() {
        assert!(parse("pas du json").is_empty());
        assert!(parse("[]").is_empty());
        let content = r#"{"version":1,"profiles":{
            "ok":{"disk":{"attention":70,"critical":80},"memory":{"attention":85,"critical":95},"load":{"attention":1.5,"critical":3}},
            "invalide":{"disk":{"attention":90,"critical":80},"memory":{"attention":85,"critical":95},"load":{"attention":1,"critical":2}},
            "incomplet":{"disk":{"attention":70}}}}"#;
        let profiles = parse(content);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles["ok"].load.attention, 1.5);
    }
}
