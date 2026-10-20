//! Scripts personnalisés : stockage local en JSON (`scripts.json`, écriture
//! atomique fournie par `config`), validation, commande d'exécution et
//! enregistrement de la dernière exécution. L'exécution elle-même passe par
//! le registre des flux de `state` (même `LogEvent` que les logs).
//!
//! Règles d'enregistrement d'un script :
//! - l'identifiant est attribué par le backend à la création ;
//! - `lastRun` n'est jamais modifié par `scripts_save` : seule la fin d'une
//!   exécution l'écrit ;
//! - les fins de ligne `\r\n` de la commande sont ramenées à `\n`.
//!
//! Ce module ne dépend pas de Tauri.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::{self, has_control_chars};
use crate::error::AppError;
use crate::ssh::shell;
use crate::stream::{self, EventSink, LineParser, LogEvent, LogLine, Raw};

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

// ---------------------------------------------------------------------------
// Exécution
// ---------------------------------------------------------------------------

/// Commande distante d'un script : le shell de connexion de l'utilisateur
/// (`SHELL`, toujours défini par sshd), qui charge `/etc/profile` et
/// `~/.profile`, reçoit la commande en un seul argument échappé. Plusieurs
/// lignes, `$`, `"`, `;` et accents arrivent intacts.
///
/// `~/.bashrc` n'est pas chargé utilement : sous Ubuntu, il s'interrompt en
/// mode non interactif (nvm, alias… indisponibles).
///
/// À envelopper dans `stream::guarded_command` : c'est une commande simple.
pub fn run_command(command: &str) -> String {
    format!("\"${{SHELL:-/bin/sh}}\" -l -c {}", shell::quote(command))
}

/// Lignes de la sortie d'un script, horodatées à leur réception (le
/// programme n'en fournit pas).
pub struct ScriptParser {
    clock: fn() -> String,
}

impl ScriptParser {
    pub fn new() -> Self {
        Self { clock: now_iso }
    }
}

impl Default for ScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LineParser for ScriptParser {
    fn parse(&mut self, raw: Raw, line: String, out: &mut Vec<LogLine>) {
        out.push(LogLine {
            timestamp: Some((self.clock)()),
            ..LogLine::plain(raw, line)
        });
    }
}

/// Heure courante en ISO 8601 UTC, à la milliseconde.
fn now_iso() -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    stream::iso_from_micros(micros)
}

/// Enregistre `lastRun` une seule fois par exécution :
/// - à l'événement final, **avant** de le transmettre (l'interface relit la
///   liste à la réception de `ended` et doit y trouver la nouvelle valeur) :
///   `ended` donne le code de sortie, `error` (liaison perdue) `null` ;
/// - sinon à la destruction, avec `null` : arrêt demandé, interface
///   disparue, tâche abandonnée.
pub struct LastRunRecorder {
    store: Arc<ScriptStore>,
    id: String,
    at: u64,
    done: AtomicBool,
}

impl LastRunRecorder {
    pub fn new(store: Arc<ScriptStore>, id: String, at: u64) -> Arc<Self> {
        Arc::new(Self {
            store,
            id,
            at,
            done: AtomicBool::new(false),
        })
    }

    fn record(&self, exit_code: Option<i32>) {
        if self.done.swap(true, Ordering::SeqCst) {
            return;
        }
        // Échec d'écriture : l'exécution n'en dépend pas, rien à signaler.
        let _ = self.store.record_last_run(
            &self.id,
            LastRun {
                at: self.at,
                exit_code,
            },
        );
    }

    /// Le script n'a pas été lancé (ouverture refusée) : rien n'est enregistré.
    pub fn disarm(&self) {
        self.done.store(true, Ordering::SeqCst);
    }

    /// Enveloppe `sink` pour enregistrer `lastRun` à l'événement final.
    pub fn wrap(self: &Arc<Self>, sink: EventSink) -> EventSink {
        let recorder = self.clone();
        Arc::new(move |event: LogEvent| {
            match &event {
                LogEvent::Ended { exit_code } => recorder.record(*exit_code),
                LogEvent::Error { .. } => recorder.record(None),
                LogEvent::Lines { .. } => {}
            }
            sink(event)
        })
    }
}

impl Drop for LastRunRecorder {
    fn drop(&mut self) {
        self.record(None);
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

    // --- Commande d'exécution ---------------------------------------------

    /// Mot unique tel que le shell le lit : segments entre apostrophes
    /// (littéraux) et caractères échappés par `\` hors apostrophes.
    fn shell_word(word: &str) -> String {
        let mut out = String::new();
        let mut chars = word.chars();
        let mut quoted = false;
        while let Some(c) = chars.next() {
            match (quoted, c) {
                (_, '\'') => quoted = !quoted,
                (false, '\\') => out.push(chars.next().expect("échappement final")),
                (false, c) => panic!("caractère hors apostrophes : {c:?}"),
                (true, c) => out.push(c),
            }
        }
        assert!(!quoted, "apostrophe non refermée");
        out
    }

    const TRICKY: &str = "cd ~/app && git pull\n\
        echo \"fin : $(date) `id` $HOME\" ; echo 'cité' \\\n  suite\n\
        printf '%s\\n' \"l'apostrophe\" 'é ü — ; && | > <'\n\
        exit 3";

    #[test]
    fn run_command_uses_the_login_shell() {
        assert_eq!(
            run_command("~/scripts/deploy.sh"),
            "\"${SHELL:-/bin/sh}\" -l -c '~/scripts/deploy.sh'"
        );
    }

    #[test]
    fn multiline_command_is_passed_intact_as_a_single_argument() {
        let built = run_command(TRICKY);
        let argument = built
            .strip_prefix("\"${SHELL:-/bin/sh}\" -l -c ")
            .expect("préfixe du shell de connexion");
        // Un seul mot : aucun espace, retour à la ligne ou métacaractère
        // n'est interprété hors des apostrophes.
        assert_eq!(shell_word(argument), TRICKY);
    }

    #[test]
    fn script_lines_are_stamped_on_reception() {
        let mut parser = ScriptParser {
            clock: || "2026-10-17T09:13:46.900Z".to_owned(),
        };
        let mut out = Vec::new();
        parser.parse(Raw::Stdout, "==> déploiement".into(), &mut out);
        parser.parse(Raw::Stderr, "layer cache miss".into(), &mut out);
        let stamp = Some("2026-10-17T09:13:46.900Z".to_owned());
        assert_eq!(
            out,
            vec![
                LogLine {
                    timestamp: stamp.clone(),
                    ..LogLine::plain(Raw::Stdout, "==> déploiement".into())
                },
                LogLine {
                    timestamp: stamp,
                    ..LogLine::plain(Raw::Stderr, "layer cache miss".into())
                },
            ]
        );
        assert!(stream::normalize_utc_timestamp(&now_iso()).is_some());
    }

    // --- Enregistrement de lastRun ------------------------------------------

    /// Magasin sur disque avec un script, et son identifiant.
    fn store_with_script(dir: &TempDir) -> (Arc<ScriptStore>, String) {
        let store = Arc::new(ScriptStore::new(&dir.0));
        let id = store.save(script("")).unwrap().id;
        (store, id)
    }

    type Seen = Arc<Mutex<Vec<Option<LastRun>>>>;

    /// Destinataire qui relève, à chaque événement, la valeur enregistrée.
    fn spy(store: &Arc<ScriptStore>, id: &str) -> (EventSink, Seen) {
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let sink: EventSink = {
            let (store, id, seen) = (store.clone(), id.to_owned(), seen.clone());
            Arc::new(move |_| {
                seen.lock().unwrap().push(store.get(&id).unwrap().last_run);
                true
            })
        };
        (sink, seen)
    }

    const AT: u64 = 1_760_000_000_000;

    #[test]
    fn natural_end_is_recorded_before_the_event_is_forwarded() {
        let dir = TempDir::new();
        let (store, id) = store_with_script(&dir);
        let (sink, seen) = spy(&store, &id);
        let recorder = LastRunRecorder::new(store.clone(), id.clone(), AT);
        let sink = recorder.wrap(sink);
        drop(recorder);

        sink(LogEvent::Lines {
            lines: Vec::new(),
            dropped: 0,
        });
        sink(LogEvent::Ended { exit_code: Some(1) });
        drop(sink);

        let recorded = Some(LastRun {
            at: AT,
            exit_code: Some(1),
        });
        // Rien pendant l'exécution ; la valeur est déjà écrite quand
        // l'interface reçoit `ended`.
        assert_eq!(*seen.lock().unwrap(), vec![None, recorded]);
        // La destruction ne la remplace pas.
        assert_eq!(store.get(&id).unwrap().last_run, recorded);
    }

    #[test]
    fn lost_link_records_a_null_code() {
        let dir = TempDir::new();
        let (store, id) = store_with_script(&dir);
        let (sink, seen) = spy(&store, &id);
        let recorder = LastRunRecorder::new(store.clone(), id.clone(), AT);
        recorder.wrap(sink)(LogEvent::Error {
            message: "liaison perdue".into(),
        });
        let interrupted = Some(LastRun {
            at: AT,
            exit_code: None,
        });
        assert_eq!(*seen.lock().unwrap(), vec![interrupted]);
    }

    #[test]
    fn interrupted_run_records_a_null_code_when_dropped() {
        let dir = TempDir::new();
        let (store, id) = store_with_script(&dir);
        let (sink, _) = spy(&store, &id);
        // Arrêt demandé, interface disparue ou tâche abandonnée : aucun
        // événement final, le flux et son destinataire sont détruits.
        let recorder = LastRunRecorder::new(store.clone(), id.clone(), AT);
        let sink = recorder.wrap(sink);
        drop(recorder);
        assert_eq!(store.get(&id).unwrap().last_run, None);
        drop(sink);
        assert_eq!(
            store.get(&id).unwrap().last_run,
            Some(LastRun {
                at: AT,
                exit_code: None,
            })
        );
    }

    #[test]
    fn a_run_that_never_started_records_nothing() {
        let dir = TempDir::new();
        let (store, id) = store_with_script(&dir);
        let (sink, _) = spy(&store, &id);
        let recorder = LastRunRecorder::new(store.clone(), id.clone(), AT);
        let sink = recorder.wrap(sink);
        recorder.disarm();
        drop((recorder, sink));
        assert_eq!(store.get(&id).unwrap().last_run, None);
    }

    /// Exécution réelle de la commande par `sh`, enveloppée dans le script
    /// de surveillance, avec un dossier personnel temporaire.
    #[cfg(unix)]
    mod shell {
        use super::super::run_command;
        use super::{TempDir, TRICKY};
        use crate::stream::guarded_command;
        use std::process::{Command, Output, Stdio};

        fn run(home: &TempDir, command: &str) -> Output {
            let mut child = Command::new("sh")
                .arg("-c")
                .arg(guarded_command(&run_command(command)))
                .env("SHELL", "/bin/sh")
                .env("HOME", &home.0)
                .current_dir(&home.0)
                // Entrée gardée ouverte, comme le canal SSH : seule la
                // commande reçoit /dev/null.
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let _stdin = child.stdin.take();
            child.wait_with_output().unwrap()
        }

        #[test]
        fn multiline_command_runs_intact() {
            let home = TempDir::new();
            std::fs::create_dir(home.0.join("app")).unwrap();
            let command = TRICKY
                .replace("git pull", "echo pull")
                .replace("$(date)", "$(echo maintenant)")
                .replace("`id`", "`echo moi`");
            let output = run(&home, &command);
            assert_eq!(output.status.code(), Some(3));
            let home_path = home.0.display();
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!(
                    "pull\nfin : maintenant moi {home_path}\ncité suite\n\
                     l'apostrophe\né ü — ; && | > <\n"
                )
            );
        }

        #[test]
        fn login_profile_is_loaded() {
            let home = TempDir::new();
            std::fs::write(
                home.0.join(".profile"),
                "VPSCTL_PROFILE=chargé; export VPSCTL_PROFILE\n",
            )
            .unwrap();
            let output = run(&home, "echo \"$VPSCTL_PROFILE\"");
            assert_eq!(String::from_utf8(output.stdout).unwrap(), "chargé\n");
        }

        #[test]
        fn input_is_never_awaited() {
            let home = TempDir::new();
            // L'entrée du canal reste ouverte : sans /dev/null, `read`
            // attendrait indéfiniment.
            let output = run(&home, "read x; echo \"read $?\"; read y");
            assert_eq!(String::from_utf8(output.stdout).unwrap(), "read 1\n");
            assert_eq!(output.status.code(), Some(1));
        }
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

    #[test]
    fn shared_cases() {
        use crate::shared_cases::{check_valid, shared_cases};

        #[derive(Debug, serde::Deserialize)]
        struct Fields {
            name: String,
            command: String,
        }

        check_valid(shared_cases!("script-fields.json"), |f: &Fields| {
            normalize(Script {
                id: String::new(),
                name: f.name.clone(),
                description: String::new(),
                command: f.command.clone(),
                require_confirmation: false,
                last_run: None,
            })
            .is_ok()
        });
    }
}
