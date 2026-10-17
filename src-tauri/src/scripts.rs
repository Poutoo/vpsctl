//! Scripts personnalisés : stockage local en JSON (`scripts.json`, écriture
//! atomique fournie par `config`), validation et enregistrement de la
//! dernière exécution.
//!
//! Règles d'enregistrement d'un script :
//! - l'identifiant est attribué par le backend à la création ;
//! - `lastRun` n'est jamais modifié par `scripts_save` : seule la fin d'une
//!   exécution l'écrit ;
//! - les fins de ligne `\r\n` de la commande sont ramenées à `\n`.
//!
//! Ce module ne dépend pas de Tauri.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::{self, has_control_chars};
use crate::error::AppError;

const FILE_NAME: &str = "scripts.json";
const FILE_VERSION: u32 = 1;
/// Longueur maximale du nom, en caractères.
const MAX_NAME_CHARS: usize = 64;
/// Taille maximale de la commande, en octets.
const MAX_COMMAND_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Script {
    /// Vide à la création : l'identifiant est attribué par le backend.
    pub id: String,
    pub name: String,
    /// Chaîne vide si absente.
    pub description: String,
    /// Peut contenir plusieurs lignes.
    pub command: String,
    pub require_confirmation: bool,
    /// Écrit uniquement par le backend, à la fin de chaque exécution.
    pub last_run: Option<LastRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LastRun {
    /// Début de l'exécution, en millisecondes depuis l'epoch Unix.
    #[ts(type = "number")]
    pub at: u64,
    /// `None` : exécution interrompue ou tuée par un signal.
    pub exit_code: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScriptsFile {
    version: u32,
    scripts: Vec<Script>,
}

/// Accès au fichier des scripts. Les lectures-modifications sont sérialisées
/// par un verrou interne : la fin d'une exécution (tâche de fond) et
/// l'interface peuvent écrire en même temps.
#[derive(Debug)]
pub struct ScriptStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl ScriptStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join(FILE_NAME),
            lock: Mutex::new(()),
        }
    }

    fn locked<T>(&self, f: impl FnOnce() -> Result<T, AppError>) -> Result<T, AppError> {
        // Un verrou empoisonné ne protège que l'accès au fichier : il reste utilisable.
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        f()
    }

    fn read(&self) -> Result<Vec<Script>, AppError> {
        let file: Option<ScriptsFile> = config::read_json(&self.path, "des scripts")?;
        Ok(file.map(|f| f.scripts).unwrap_or_default())
    }

    fn write(&self, scripts: Vec<Script>) -> Result<(), AppError> {
        let file = ScriptsFile {
            version: FILE_VERSION,
            scripts,
        };
        config::write_json_atomic(&self.path, &file, "des scripts")
    }

    pub fn list(&self) -> Result<Vec<Script>, AppError> {
        self.locked(|| self.read())
    }

    pub fn get(&self, id: &str) -> Result<Script, AppError> {
        self.list()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AppError::invalid_input("script introuvable"))
    }

    pub fn save(&self, incoming: Script) -> Result<Script, AppError> {
        self.locked(|| {
            let mut scripts = self.read()?;
            let saved = apply_save(&mut scripts, incoming, || uuid::Uuid::new_v4().to_string())?;
            self.write(scripts)?;
            Ok(saved)
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        self.locked(|| {
            let mut scripts = self.read()?;
            let before = scripts.len();
            scripts.retain(|s| s.id != id);
            if scripts.len() == before {
                return Err(AppError::invalid_input("script introuvable"));
            }
            self.write(scripts)
        })
    }

    /// Enregistre la dernière exécution d'un script. Script supprimé
    /// entre-temps : rien n'est écrit (`Ok(false)`).
    pub fn record_last_run(&self, id: &str, run: LastRun) -> Result<bool, AppError> {
        self.locked(|| {
            let mut scripts = self.read()?;
            if !apply_last_run(&mut scripts, id, run) {
                return Ok(false);
            }
            self.write(scripts)?;
            Ok(true)
        })
    }
}

/// Applique les règles d'enregistrement sur la liste en mémoire.
fn apply_save(
    scripts: &mut Vec<Script>,
    incoming: Script,
    new_id: impl FnOnce() -> String,
) -> Result<Script, AppError> {
    let mut script = normalize(incoming)?;

    if script.id.is_empty() {
        script.id = new_id();
        script.last_run = None;
    } else {
        let stored = scripts
            .iter()
            .find(|s| s.id == script.id)
            .ok_or_else(|| AppError::invalid_input("script introuvable"))?;
        script.last_run = stored.last_run;
    }

    match scripts.iter_mut().find(|s| s.id == script.id) {
        Some(slot) => *slot = script.clone(),
        None => scripts.push(script.clone()),
    }
    Ok(script)
}

/// Renvoie `false` si le script n'existe plus.
fn apply_last_run(scripts: &mut [Script], id: &str, run: LastRun) -> bool {
    match scripts.iter_mut().find(|s| s.id == id) {
        Some(script) => {
            script.last_run = Some(run);
            true
        }
        None => false,
    }
}

/// Supprime les espaces superflus et valide chaque champ.
fn normalize(mut script: Script) -> Result<Script, AppError> {
    script.id = script.id.trim().to_owned();
    script.name = script.name.trim().to_owned();
    script.description = script.description.trim().to_owned();
    script.command = script.command.replace("\r\n", "\n");

    if script.name.is_empty() || script.name.chars().count() > MAX_NAME_CHARS {
        return Err(AppError::invalid_input(format!(
            "le nom du script est requis ({MAX_NAME_CHARS} caractères au plus)"
        )));
    }
    if has_control_chars(&script.name) {
        return Err(AppError::invalid_input(
            "le nom du script contient des caractères invalides",
        ));
    }
    if script.description.contains('\0') {
        return Err(AppError::invalid_input(
            "la description contient un caractère nul",
        ));
    }
    if script.command.trim().is_empty() {
        return Err(AppError::invalid_input("la commande est requise"));
    }
    if script.command.contains('\0') {
        return Err(AppError::invalid_input(
            "la commande contient un caractère nul",
        ));
    }
    if script.command.len() > MAX_COMMAND_BYTES {
        return Err(AppError::invalid_input(format!(
            "commande trop longue : {} Kio au plus",
            MAX_COMMAND_BYTES / 1024
        )));
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn script(id: &str) -> Script {
        Script {
            id: id.to_owned(),
            name: "deploy.sh".to_owned(),
            description: "Récupère main.".to_owned(),
            command: "~/scripts/deploy.sh".to_owned(),
            require_confirmation: true,
            last_run: None,
        }
    }

    const RUN: LastRun = LastRun {
        at: 1_760_000_000_000,
        exit_code: Some(0),
    };

    // --- Validation --------------------------------------------------------

    #[test]
    fn invalid_fields_are_rejected() {
        let cases = [
            Script {
                name: "   ".to_owned(),
                ..script("")
            },
            Script {
                name: "x".repeat(65),
                ..script("")
            },
            Script {
                name: "deploy\n.sh".to_owned(),
                ..script("")
            },
            Script {
                name: "deploy\0".to_owned(),
                ..script("")
            },
            Script {
                description: "a\0b".to_owned(),
                ..script("")
            },
            Script {
                command: String::new(),
                ..script("")
            },
            Script {
                command: " \n\t\r\n ".to_owned(),
                ..script("")
            },
            Script {
                command: "echo a\0b".to_owned(),
                ..script("")
            },
            Script {
                command: "x".repeat(MAX_COMMAND_BYTES + 1),
                ..script("")
            },
        ];
        for case in cases {
            let mut scripts = Vec::new();
            let result = apply_save(&mut scripts, case.clone(), || "x".to_owned());
            assert!(
                matches!(result, Err(AppError::InvalidInput { .. })),
                "accepté à tort : {case:?}"
            );
            assert!(scripts.is_empty());
        }
    }

    #[test]
    fn valid_limits_are_accepted() {
        let mut scripts = Vec::new();
        let saved = apply_save(
            &mut scripts,
            Script {
                name: "é".repeat(MAX_NAME_CHARS),
                description: String::new(),
                command: "x".repeat(MAX_COMMAND_BYTES),
                ..script("")
            },
            || "id".to_owned(),
        )
        .unwrap();
        assert_eq!(saved.name.chars().count(), MAX_NAME_CHARS);
        assert_eq!(saved.description, "");
    }

    #[test]
    fn multiline_command_is_kept_and_crlf_normalized() {
        let mut scripts = Vec::new();
        let command = "cd ~/app &&\r\n  git pull\r\necho \"fini : $(date)\"\r\n";
        let saved = apply_save(
            &mut scripts,
            Script {
                command: command.to_owned(),
                ..script("")
            },
            || "id".to_owned(),
        )
        .unwrap();
        assert_eq!(
            saved.command,
            "cd ~/app &&\n  git pull\necho \"fini : $(date)\"\n"
        );
    }

    #[test]
    fn name_and_description_are_trimmed_but_not_the_command() {
        let mut scripts = Vec::new();
        let saved = apply_save(
            &mut scripts,
            Script {
                name: "  deploy.sh ".to_owned(),
                description: " Rappel ".to_owned(),
                command: "  echo  ".to_owned(),
                ..script("")
            },
            || "id".to_owned(),
        )
        .unwrap();
        assert_eq!(saved.name, "deploy.sh");
        assert_eq!(saved.description, "Rappel");
        assert_eq!(saved.command, "  echo  ");
    }

    // --- Enregistrement ----------------------------------------------------

    #[test]
    fn new_script_gets_an_id_and_never_an_incoming_last_run() {
        let mut scripts = Vec::new();
        let saved = apply_save(
            &mut scripts,
            Script {
                last_run: Some(RUN),
                ..script("")
            },
            || "id-1".to_owned(),
        )
        .unwrap();
        assert_eq!(saved.id, "id-1");
        assert_eq!(saved.last_run, None);
        assert_eq!(scripts, vec![saved]);
    }

    #[test]
    fn update_keeps_the_stored_last_run() {
        let mut scripts = vec![Script {
            last_run: Some(RUN),
            ..script("a")
        }];
        let edited = Script {
            name: "renommé".to_owned(),
            last_run: None,
            ..script("a")
        };
        let saved = apply_save(&mut scripts, edited, || unreachable!()).unwrap();
        assert_eq!(saved.last_run, Some(RUN));
        assert_eq!(scripts[0].name, "renommé");

        // Une valeur forgée est ignorée elle aussi.
        let forged = Script {
            last_run: Some(LastRun {
                at: 1,
                exit_code: Some(42),
            }),
            ..script("a")
        };
        let saved = apply_save(&mut scripts, forged, || unreachable!()).unwrap();
        assert_eq!(saved.last_run, Some(RUN));
    }

    #[test]
    fn unknown_id_is_rejected() {
        let mut scripts = vec![script("a")];
        let err = apply_save(&mut scripts, script("zzz"), || unreachable!()).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput { .. }));
    }

    #[test]
    fn last_run_is_recorded_on_the_right_script() {
        let mut scripts = vec![script("a"), script("b")];
        let interrupted = LastRun {
            at: 1_760_000_000_000,
            exit_code: None,
        };
        assert!(apply_last_run(&mut scripts, "b", interrupted));
        assert_eq!(scripts[0].last_run, None);
        assert_eq!(scripts[1].last_run, Some(interrupted));

        // Nouvelle exécution : la précédente est remplacée.
        assert!(apply_last_run(&mut scripts, "b", RUN));
        assert_eq!(scripts[1].last_run, Some(RUN));

        // Script supprimé pendant l'exécution : rien n'est écrit.
        assert!(!apply_last_run(&mut scripts, "zzz", RUN));
    }

    // --- Sérialisation -----------------------------------------------------

    #[test]
    fn script_serializes_in_camel_case() {
        let value = Script {
            last_run: Some(RUN),
            ..script("a")
        };
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            json!({
                "id": "a",
                "name": "deploy.sh",
                "description": "Récupère main.",
                "command": "~/scripts/deploy.sh",
                "requireConfirmation": true,
                "lastRun": { "at": 1_760_000_000_000_u64, "exitCode": 0 },
            })
        );
        let interrupted = LastRun {
            at: 5,
            exit_code: None,
        };
        assert_eq!(
            serde_json::to_value(interrupted).unwrap(),
            json!({ "at": 5, "exitCode": null })
        );
    }

    #[test]
    fn script_deserializes_from_the_frontend() {
        let parsed: Script = serde_json::from_value(json!({
            "id": "",
            "name": "backup",
            "description": "",
            "command": "pg_dump app\nexit 0",
            "requireConfirmation": false,
            "lastRun": null,
        }))
        .unwrap();
        assert_eq!(parsed.command, "pg_dump app\nexit 0");
        assert!(!parsed.require_confirmation);
    }

    // --- Fichier -----------------------------------------------------------

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
        let store = ScriptStore::new(&dir.0.join("sous-dossier"));

        assert_eq!(store.list().unwrap(), Vec::new());

        let saved = store.save(script("")).unwrap();
        assert!(store.record_last_run(&saved.id, RUN).unwrap());
        let reloaded = store.get(&saved.id).unwrap();
        assert_eq!(reloaded.last_run, Some(RUN));

        // Modifier le script conserve sa dernière exécution.
        let edited = store
            .save(Script {
                command: "echo un\necho 'deux'".to_owned(),
                ..reloaded
            })
            .unwrap();
        assert_eq!(edited.last_run, Some(RUN));
        assert_eq!(store.list().unwrap(), vec![edited]);

        store.delete(&saved.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(store.delete(&saved.id).is_err());
        assert!(!store.record_last_run(&saved.id, RUN).unwrap());
        // Pas de temporaire laissé par l'écriture atomique.
        let names: Vec<_> = fs::read_dir(dir.0.join("sous-dossier"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(FILE_NAME)]);
    }
}
