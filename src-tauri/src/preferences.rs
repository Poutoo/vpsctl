//! Préférences de l'interface, dans `preferences.json` du dossier de
//! configuration, séparé des profils, et copie de l'image de fond
//! personnalisée (`background`, à côté).
//!
//! Lecture tolérante : un fichier absent, illisible ou invalide donne les
//! valeurs par défaut, et chaque champ manquant ou hors limites reprend la
//! sienne sans toucher aux autres. Le démarrage n'est jamais bloqué.
//!
//! Import d'une image en deux temps : `choose` contrôle le fichier et garde
//! ses octets en attente ; la webview le décode, puis `commit` écrit la copie
//! (ou `discard` l'abandonne). Le fond actuel n'est touché qu'à `commit`.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::background::{self, BackgroundInfo, Refusal};
use crate::config::{write_atomic, write_json_atomic};
use crate::error::AppError;

const FILE_NAME: &str = "preferences.json";
const FILE_VERSION: u32 = 1;
/// Copie de l'image de fond ; son format est relu sur son contenu.
const BACKGROUND_FILE: &str = "background";

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
    /// Image personnalisée ; `null` pour le fond fourni avec l'application.
    pub background: Option<BackgroundInfo>,
    /// Présence Discord, désactivée par défaut.
    pub discord_presence: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            dimming: DEFAULT_DIMMING,
            original_colors: false,
            background: None,
            discord_presence: false,
        }
    }
}

/// Issue du choix d'une image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BackgroundChoice {
    /// Fenêtre de sélection fermée sans choix.
    Cancelled,
    Refused {
        file_name: String,
        refusal: Refusal,
    },
    /// Contrôles réussis : la webview doit encore décoder l'image avant
    /// `background_commit`.
    Pending {
        token: String,
        background: BackgroundInfo,
    },
}

#[derive(Serialize)]
struct PreferencesFile<'a> {
    version: u32,
    #[serde(flatten)]
    preferences: &'a Preferences,
}

struct PendingImage {
    token: String,
    info: BackgroundInfo,
    data: Vec<u8>,
}

/// Préférences en mémoire et leur fichier. Chaque modification est écrite
/// aussitôt ; la valeur en mémoire ne change qu'une fois l'écriture réussie.
pub struct PreferenceStore {
    path: PathBuf,
    background_path: PathBuf,
    current: Mutex<Preferences>,
    pending: Mutex<Option<PendingImage>>,
    /// Copie introuvable ou illisible au démarrage, signalée une seule fois.
    background_lost: AtomicBool,
}

impl PreferenceStore {
    /// Lit les préférences et vérifie la copie de l'image de fond : si elle
    /// est absente ou illisible, le fond d'origine est rétabli.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(FILE_NAME);
        let current = fs::read_to_string(&path)
            .map(|content| parse(&content))
            .unwrap_or_default();
        let store = Self {
            path,
            background_path: config_dir.join(BACKGROUND_FILE),
            current: Mutex::new(current),
            pending: Mutex::new(None),
            background_lost: AtomicBool::new(false),
        };
        store.verify_background();
        store
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

    pub fn set_discord_presence(&self, enabled: bool) -> Result<Preferences, AppError> {
        self.update(|p| p.discord_presence = enabled)
    }

    /// Vrai une seule fois si la copie a été écartée au démarrage.
    pub fn take_background_lost(&self) -> bool {
        self.background_lost.swap(false, Ordering::SeqCst)
    }

    /// Contrôle l'image choisie et la garde en attente de décodage. Le
    /// chemin n'est pas conservé : seul le nom du fichier l'est.
    pub fn choose(&self, path: &Path) -> Result<BackgroundChoice, AppError> {
        let file_name = background::display_name(path);
        let refused = |refusal| {
            Ok(BackgroundChoice::Refused {
                file_name: file_name.clone(),
                refusal,
            })
        };
        let read_error =
            |e: std::io::Error| AppError::io(format!("lecture de l'image impossible : {e}"));

        let size = fs::metadata(path).map_err(read_error)?.len();
        if let Err(refusal) = background::check_size(size) {
            return refused(refusal);
        }
        // Lecture bornée : le fichier peut avoir grossi depuis `metadata`.
        let mut data = Vec::new();
        File::open(path)
            .and_then(|f| f.take(background::MAX_BYTES + 1).read_to_end(&mut data))
            .map_err(read_error)?;
        let probe = match background::inspect(&data) {
            Ok(probe) => probe,
            Err(refusal) => return refused(refusal),
        };
        let info = BackgroundInfo {
            file_name,
            format: probe.format,
            bytes: data.len() as u64,
            width: probe.width,
            height: probe.height,
        };
        let token = uuid::Uuid::new_v4().to_string();
        *self.pending_lock() = Some(PendingImage {
            token: token.clone(),
            info: info.clone(),
            data,
        });
        Ok(BackgroundChoice::Pending {
            token,
            background: info,
        })
    }

    /// Octets de l'image en attente, pour son décodage par la webview.
    pub fn pending_image(&self, token: &str) -> Result<Vec<u8>, AppError> {
        match self.pending_lock().as_ref() {
            Some(pending) if pending.token == token => Ok(pending.data.clone()),
            _ => Err(unknown_pending()),
        }
    }

    /// Écrit la copie de l'image en attente et l'enregistre comme fond.
    pub fn commit(&self, token: &str) -> Result<Preferences, AppError> {
        let pending = {
            let mut slot = self.pending_lock();
            match slot.take() {
                Some(pending) if pending.token == token => pending,
                other => {
                    *slot = other;
                    return Err(unknown_pending());
                }
            }
        };
        write_atomic(&self.background_path, &pending.data, "de l'image de fond")?;
        self.update(|p| p.background = Some(pending.info))
    }

    /// Abandonne l'image en attente (décodage refusé par la webview).
    pub fn discard(&self, token: &str) {
        let mut slot = self.pending_lock();
        if slot.as_ref().is_some_and(|p| p.token == token) {
            *slot = None;
        }
    }

    /// Octets de la copie enregistrée.
    pub fn background_image(&self) -> Result<Vec<u8>, AppError> {
        if self.lock().background.is_none() {
            return Err(AppError::invalid_input(
                "aucune image de fond personnalisée",
            ));
        }
        fs::read(&self.background_path)
            .map_err(|e| AppError::io(format!("lecture de l'image de fond impossible : {e}")))
    }

    /// Revient au fond d'origine et supprime la copie.
    pub fn reset(&self) -> Result<Preferences, AppError> {
        let preferences = self.update(|p| p.background = None)?;
        self.remove_copy();
        Ok(preferences)
    }

    fn verify_background(&self) {
        let Some(info) = self.lock().background.clone() else {
            return;
        };
        match self.read_copy() {
            // Format et dimensions relus sur la copie : ils font foi.
            Some(probe) => {
                let mut current = self.lock();
                current.background = Some(BackgroundInfo {
                    format: probe.0.format,
                    width: probe.0.width,
                    height: probe.0.height,
                    bytes: probe.1,
                    ..info
                });
            }
            None => {
                self.background_lost.store(true, Ordering::SeqCst);
                if self.update(|p| p.background = None).is_err() {
                    // Fichier non modifiable : l'application démarre quand
                    // même sur le fond d'origine.
                    self.lock().background = None;
                }
                self.remove_copy();
            }
        }
    }

    /// Contrôles rapides de la copie (taille, format, dimensions).
    fn read_copy(&self) -> Option<(background::Probe, u64)> {
        let size = fs::metadata(&self.background_path).ok()?.len();
        background::check_size(size).ok()?;
        let data = fs::read(&self.background_path).ok()?;
        let probe = background::inspect(&data).ok()?;
        Some((probe, data.len() as u64))
    }

    fn remove_copy(&self) {
        // Copie déjà absente ou impossible à supprimer : elle n'est plus
        // référencée, et sera remplacée au prochain import.
        let _ = fs::remove_file(&self.background_path);
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

    fn lock(&self) -> MutexGuard<'_, Preferences> {
        // Un verrou empoisonné garde une valeur cohérente : elle n'est
        // remplacée qu'en bloc, après une écriture réussie.
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn pending_lock(&self) -> MutexGuard<'_, Option<PendingImage>> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn unknown_pending() -> AppError {
    AppError::invalid_input("aucune image en attente : choisissez-la de nouveau")
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
    let background = fields
        .get("background")
        .and_then(|v| serde_json::from_value::<BackgroundInfo>(v.clone()).ok());
    let discord_presence = fields
        .get("discordPresence")
        .and_then(Value::as_bool)
        .unwrap_or(defaults.discord_presence);
    Preferences {
        dimming,
        original_colors,
        background,
        discord_presence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{samples, ImageFormat};

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-preferences-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Image choisie par l'utilisateur, ailleurs que dans la configuration.
    fn picked(dir: &Path, name: &str, data: &[u8]) -> PathBuf {
        let source = dir.join("source");
        fs::create_dir_all(&source).unwrap();
        let path = source.join(name);
        fs::write(&path, data).unwrap();
        path
    }

    fn token_of(choice: BackgroundChoice) -> String {
        match choice {
            BackgroundChoice::Pending { token, .. } => token,
            other => panic!("image refusée : {other:?}"),
        }
    }

    #[test]
    fn defaults_dim_by_85_percent_in_grayscale_on_the_original_background() {
        assert_eq!(
            Preferences::default(),
            Preferences {
                dimming: 85,
                original_colors: false,
                background: None,
                discord_presence: false,
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

        let prefs = parse(
            r#"{"dimming":40,"originalColors":"oui","background":{"fileName":1},"discordPresence":1}"#,
        );
        assert_eq!(prefs.dimming, 40);
        assert!(!prefs.original_colors);
        assert_eq!(prefs.background, None);
        assert!(!prefs.discord_presence);
        assert!(parse(r#"{"discordPresence":true}"#).discord_presence);

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
        let store = PreferenceStore::load(&dir);
        assert_eq!(store.get(), Preferences::default());
        assert!(!store.take_background_lost());
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
        assert_eq!(json["background"], Value::Null);

        let reloaded = PreferenceStore::load(&dir).get();
        assert_eq!(reloaded.dimming, 60);
        assert!(reloaded.original_colors);
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

    #[test]
    fn imported_image_is_copied_without_its_original_path() {
        let dir = temp_dir("import");
        let store = PreferenceStore::load(&dir);
        let image = samples::png(3840, 2160);
        let source = picked(&dir, "nebuleuse-nord.png", &image);

        let choice = store.choose(&source).unwrap();
        let BackgroundChoice::Pending { token, background } = choice.clone() else {
            panic!("{choice:?}");
        };
        assert_eq!(
            background,
            BackgroundInfo {
                file_name: "nebuleuse-nord.png".into(),
                format: ImageFormat::Png,
                bytes: image.len() as u64,
                width: 3840,
                height: 2160,
            }
        );
        // Rien n'est écrit avant le décodage par la webview.
        assert!(!dir.join(BACKGROUND_FILE).exists());
        assert_eq!(store.get().background, None);
        assert_eq!(store.pending_image(&token).unwrap(), image);

        let prefs = store.commit(&token).unwrap();
        assert_eq!(prefs.background, Some(background));
        assert_eq!(fs::read(dir.join(BACKGROUND_FILE)).unwrap(), image);
        assert_eq!(store.background_image().unwrap(), image);
        let written = fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(!written.contains("source"), "{written}");

        // L'original peut disparaître : la copie suffit.
        fs::remove_file(&source).unwrap();
        let reloaded = PreferenceStore::load(&dir);
        assert_eq!(reloaded.get().background, prefs.background);
        assert!(!reloaded.take_background_lost());
    }

    #[test]
    fn refused_images_keep_the_current_background() {
        let dir = temp_dir("refus-image");
        let store = PreferenceStore::load(&dir);
        let first = picked(&dir, "a.webp", &samples::webp_lossless(800, 600));
        store
            .commit(&token_of(store.choose(&first).unwrap()))
            .unwrap();
        let current = store.get();

        let gif = picked(&dir, "capture-ecran.gif", b"GIF89a\x01\x00\x01\x00");
        assert_eq!(
            store.choose(&gif).unwrap(),
            BackgroundChoice::Refused {
                file_name: "capture-ecran.gif".into(),
                refusal: Refusal::Format,
            }
        );
        let huge = picked(&dir, "panorama.png", &samples::png(12_000, 8_000));
        assert_eq!(
            store.choose(&huge).unwrap(),
            BackgroundChoice::Refused {
                file_name: "panorama.png".into(),
                refusal: Refusal::Dimensions {
                    width: 12_000,
                    height: 8_000
                },
            }
        );
        let mut heavy = samples::png(100, 100);
        heavy.resize(background::MAX_BYTES as usize + 1, 0);
        let heavy = picked(&dir, "lourd.png", &heavy);
        assert!(matches!(
            store.choose(&heavy).unwrap(),
            BackgroundChoice::Refused {
                refusal: Refusal::TooLarge { .. },
                ..
            }
        ));
        assert_eq!(store.get(), current);
    }

    #[test]
    fn discarded_or_replaced_pending_image_cannot_be_committed() {
        let dir = temp_dir("abandon");
        let store = PreferenceStore::load(&dir);
        let source = picked(&dir, "a.jpg", &samples::jpeg(640, 480));
        let token = token_of(store.choose(&source).unwrap());
        store.discard(&token);
        assert!(store.commit(&token).is_err());

        let old = token_of(store.choose(&source).unwrap());
        let new = token_of(store.choose(&source).unwrap());
        assert!(store.commit(&old).is_err());
        assert!(store.commit(&new).is_ok());
    }

    #[test]
    fn reset_removes_the_copy() {
        let dir = temp_dir("retablir");
        let store = PreferenceStore::load(&dir);
        let source = picked(&dir, "a.png", &samples::png(10, 10));
        store
            .commit(&token_of(store.choose(&source).unwrap()))
            .unwrap();

        let prefs = store.reset().unwrap();
        assert_eq!(prefs.background, None);
        assert!(!dir.join(BACKGROUND_FILE).exists());
        assert!(store.background_image().is_err());
        assert_eq!(PreferenceStore::load(&dir).get().background, None);
    }

    #[test]
    fn missing_or_unreadable_copy_falls_back_once_at_startup() {
        for damage in ["supprimée", "abîmée"] {
            let dir = temp_dir("perdue");
            let store = PreferenceStore::load(&dir);
            let source = picked(&dir, "a.png", &samples::png(10, 10));
            store
                .commit(&token_of(store.choose(&source).unwrap()))
                .unwrap();
            store.set_dimming(30).unwrap();
            match damage {
                "supprimée" => fs::remove_file(dir.join(BACKGROUND_FILE)).unwrap(),
                _ => fs::write(dir.join(BACKGROUND_FILE), b"plus une image").unwrap(),
            }

            let restarted = PreferenceStore::load(&dir);
            assert_eq!(restarted.get().background, None, "{damage}");
            assert_eq!(restarted.get().dimming, 30, "{damage}");
            assert!(restarted.take_background_lost(), "{damage}");
            assert!(!restarted.take_background_lost(), "{damage}");
            assert!(!dir.join(BACKGROUND_FILE).exists(), "{damage}");
            // Le retour au fond d'origine est enregistré : pas de second avis.
            assert!(!PreferenceStore::load(&dir).take_background_lost());
        }
    }

    #[test]
    fn copy_details_are_read_from_the_copy_at_startup() {
        let dir = temp_dir("relue");
        let store = PreferenceStore::load(&dir);
        let source = picked(&dir, "a.png", &samples::png(10, 10));
        store
            .commit(&token_of(store.choose(&source).unwrap()))
            .unwrap();
        fs::write(dir.join(BACKGROUND_FILE), samples::webp_extended(300, 200)).unwrap();

        let background = PreferenceStore::load(&dir).get().background.unwrap();
        assert_eq!(background.file_name, "a.png");
        assert_eq!(background.format, ImageFormat::Webp);
        assert_eq!((background.width, background.height), (300, 200));
    }
}
