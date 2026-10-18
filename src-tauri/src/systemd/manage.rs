//! Création et suppression d'un service utilisateur : séquences exactes de
//! `docs/04-contrat-commandes.md` (« Systemd »).
//!
//! Les fichiers passent par `RemoteFs` (SFTP), les commandes par
//! `systemctl --user`, sans `sudo`. Le nom d'unité est validé par la regex des
//! services, échappé par `shell::quote` et placé après `--`.
//!
//! Les séquences sont génériques sur `UnitHost` : les tests vérifient l'ordre
//! des étapes et l'arrêt au premier échec sur un système de fichiers simulé,
//! avec une exécution qui enregistre les commandes.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use super::unit::{self, ServiceDef};
use super::{checked_unit, list_until, unloaded_service, Service, UnitFile, LIST_TIMEOUT};
use crate::error::{AppError, ServiceStep};
use crate::run_state::RunState;
use crate::sftp::fs::{RemoteFs, SftpFs};
use crate::sftp::ops::explain_failure;
use crate::sftp::path::{self, TempKind};
use crate::sftp::transfer::new_token;
use crate::sftp::walk;
use crate::sftp::EntryKind;
use crate::ssh::{self, shell, Connection, Deadline, ExecOutput};

/// Délai de chaque groupe d'étapes (contrôles, écriture, activation) et de
/// chaque étape de la suppression, sauf l'arrêt.
pub const STEP_TIMEOUT: Duration = Duration::from_secs(30);
/// Délai de l'arrêt : plus que le `TimeoutStopSec` par défaut (90 s), après
/// lequel systemd envoie SIGKILL.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(100);
/// Délai de `service_delete_info` et de `user_linger`.
pub const INFO_TIMEOUT: Duration = Duration::from_secs(20);
pub const LINGER_TIMEOUT: Duration = Duration::from_secs(10);

/// Droits du fichier d'unité écrit.
const UNIT_FILE_MODE: u32 = 0o644;

pub const RELOAD_COMMAND: &str = "systemctl --user daemon-reload";
/// Lingering du compte, identifié par son UID (sans saisie de l'interface).
pub const LINGER_COMMAND: &str = r#"loginctl show-user --property=Linger --value -- "$(id -u)""#;

/// Ce dont les séquences ont besoin sur le serveur.
pub trait UnitHost: Send + Sync {
    type Fs: RemoteFs;

    fn fs(&self) -> &Arc<Self::Fs>;

    /// Exécute une commande fixe ou construite à partir de valeurs validées
    /// et échappées.
    fn run(
        &self,
        command: &str,
        deadline: Deadline,
    ) -> impl Future<Output = Result<ExecOutput, AppError>> + Send;

    /// Liste des services utilisateur (comme `services_list`).
    fn services(
        &self,
        deadline: Deadline,
    ) -> impl Future<Output = Result<Vec<Service>, AppError>> + Send;
}

/// `UnitHost` sur la liaison SSH et sa session SFTP.
pub struct SshHost {
    connection: Arc<Connection>,
    fs: Arc<SftpFs>,
}

impl SshHost {
    pub fn new(connection: Arc<Connection>) -> Self {
        Self {
            fs: SftpFs::shared(connection.clone()),
            connection,
        }
    }
}

impl UnitHost for SshHost {
    type Fs = SftpFs;

    fn fs(&self) -> &Arc<SftpFs> {
        &self.fs
    }

    async fn run(&self, command: &str, deadline: Deadline) -> Result<ExecOutput, AppError> {
        self.connection.exec_until(command, deadline).await
    }

    async fn services(&self, deadline: Deadline) -> Result<Vec<Service>, AppError> {
        list_until(&self.connection, deadline).await
    }
}

/// Éléments affichés dans le GO / NO GO d'une suppression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceDeleteInfo {
    pub unit: String,
    /// Chemin absolu du fichier d'unité supprimé.
    pub file: String,
    /// Chemin absolu du dossier de surcharge `<unit>.d/`, s'il existe.
    pub drop_in: Option<String>,
    /// Éléments qu'il contient (fichiers, liens, sous-dossiers).
    #[ts(type = "number")]
    pub drop_in_entries: u64,
    /// Le service n'est pas arrêté : il sera arrêté.
    pub active: bool,
}

// --- Commandes (fonctions pures) -------------------------------------------

fn unit_command(template: &str, unit: &str) -> Result<String, AppError> {
    let unit = checked_unit(unit)?;
    Ok(template.replace("{unit}", &shell::quote(unit)))
}

/// `LoadState` et `FragmentPath` d'une unité, chargée ou non.
pub fn show_command(unit: &str) -> Result<String, AppError> {
    unit_command(
        "systemctl --user show --property=LoadState --property=FragmentPath -- {unit}",
        unit,
    )
}

/// Activation au démarrage et démarrage mis en file, sans attendre son
/// résultat (vérifié ensuite par l'interface).
pub fn enable_command(unit: &str) -> Result<String, AppError> {
    unit_command("systemctl --user enable --now --no-block -- {unit}", unit)
}

/// Arrêt, en attendant sa fin.
pub fn stop_command(unit: &str) -> Result<String, AppError> {
    unit_command("systemctl --user stop -- {unit}", unit)
}

pub fn disable_command(unit: &str) -> Result<String, AppError> {
    unit_command("systemctl --user disable -- {unit}", unit)
}

/// `reset-failed` seulement si l'unité est restée en échec en mémoire : il
/// échouerait sur une unité déjà déchargée.
pub fn reset_failed_command(unit: &str) -> Result<String, AppError> {
    unit_command(
        "if systemctl --user is-failed --quiet -- {unit}; \
         then systemctl --user reset-failed -- {unit}; fi",
        unit,
    )
}

/// Le nom est libre pour systemd : `LoadState=not-found`. Sinon, une autre
/// unité porte déjà ce nom (fournie par le système, par exemple).
pub fn check_load_state(unit: &str, show_output: &str) -> Result<(), AppError> {
    let property = |name: &str| {
        show_output
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .map(str::trim)
    };
    let Some(load_state) = property("LoadState") else {
        return Err(AppError::io(format!(
            "réponse inattendue de systemctl show : {}",
            show_output.trim()
        )));
    };
    match load_state {
        "not-found" => Ok(()),
        "masked" => Err(AppError::invalid_input(format!(
            "{unit} est masqué ailleurs sur le système : ce nom n'est pas disponible"
        ))),
        state => match property("FragmentPath").filter(|p| !p.is_empty()) {
            Some(fragment) => Err(AppError::already_exists(fragment, false)),
            None => Err(AppError::invalid_input(format!(
                "{unit} est déjà connu de systemd (état {state}) : ce nom n'est pas disponible"
            ))),
        },
    }
}

/// Réponse de `LINGER_COMMAND` : `None` si elle n'est ni `yes` ni `no`.
pub fn parse_linger(output: &str) -> Option<bool> {
    match output.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// Séquence à laquelle appartient une étape en échec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sequence {
    Create,
    Delete,
}

/// État du service après l'échec d'une étape.
pub fn failure_state(sequence: Sequence, step: ServiceStep) -> &'static str {
    match (sequence, step) {
        (Sequence::Create, ServiceStep::Reload) => {
            "Fichier créé, systemd non rechargé : service ni activé ni démarré."
        }
        (Sequence::Create, _) => "Fichier créé et systemd rechargé : service ni activé ni démarré.",
        (Sequence::Delete, ServiceStep::Stop) => {
            "Rien n'a été modifié. L'arrêt est peut-être encore en cours."
        }
        (Sequence::Delete, ServiceStep::Disable) => {
            "Service arrêté, toujours activé au démarrage, fichier en place."
        }
        (Sequence::Delete, ServiceStep::RemoveDropIn) => {
            "Service arrêté et désactivé, fichier en place, dossier de surcharge \
             peut-être partiellement supprimé."
        }
        (Sequence::Delete, ServiceStep::RemoveFile) => {
            "Service arrêté et désactivé, dossier de surcharge supprimé, fichier en place."
        }
        (Sequence::Delete, ServiceStep::Reload) => {
            "Fichier supprimé ; systemd garde l'ancienne définition en mémoire jusqu'au \
             prochain systemctl --user daemon-reload."
        }
        (Sequence::Delete, _) => {
            "Service supprimé ; systemd le liste encore en échec jusqu'à \
             systemctl --user reset-failed."
        }
    }
}

// --- Chemins ---------------------------------------------------------------

/// Chemins absolus d'une unité dans le dossier utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitPaths {
    home: String,
    dir: String,
    file: String,
    drop_in: String,
}

impl UnitPaths {
    fn new(home: &str, unit: &str) -> Self {
        let dir = path::join(home, super::UNIT_DIR);
        Self {
            file: path::join(&dir, unit),
            drop_in: path::join(&dir, &format!("{unit}.d")),
            home: home.to_owned(),
            dir,
        }
    }

    async fn resolve(fs: &impl RemoteFs, unit: &str) -> Result<Self, AppError> {
        Ok(Self::new(&fs.real_path(".").await?, unit))
    }
}

// --- Étapes ----------------------------------------------------------------

/// Exécute une commande ; un code de sortie non nul est une erreur `Remote`.
async fn run_ok(host: &impl UnitHost, command: &str, deadline: Deadline) -> Result<(), AppError> {
    let output = host.run(command, deadline).await?;
    if output.success() {
        Ok(())
    } else {
        Err(output.remote_error())
    }
}

/// Erreur d'une étape après laquelle la séquence s'arrête.
fn step_failed(unit: &str, sequence: Sequence, step: ServiceStep, cause: AppError) -> AppError {
    AppError::ServiceStepFailed {
        unit: unit.to_owned(),
        step,
        state: failure_state(sequence, step).to_owned(),
        cause: Box::new(cause),
    }
}

/// Le dossier existe (un lien vers un dossier convient), ou il est créé.
async fn ensure_dir(fs: &impl RemoteFs, dir: &str) -> Result<(), AppError> {
    match fs.stat(dir).await? {
        Some(meta) if meta.kind == EntryKind::Dir => Ok(()),
        Some(_) => Err(AppError::invalid_input(format!(
            "{dir} existe et n'est pas un dossier"
        ))),
        None => match fs.create_dir(dir).await {
            Ok(()) => Ok(()),
            // Créé entre-temps : il suffit qu'il soit là.
            Err(error) => match fs.stat(dir).await {
                Ok(Some(meta)) if meta.kind == EntryKind::Dir => Ok(()),
                _ => Err(error),
            },
        },
    }
}

/// Crée les dossiers manquants parmi `~/.config`, `~/.config/systemd` et
/// `~/.config/systemd/user`.
async fn ensure_unit_dir(fs: &impl RemoteFs, home: &str) -> Result<(), AppError> {
    let mut dir = home.to_owned();
    for component in super::UNIT_DIR.split('/') {
        dir = path::join(&dir, component);
        ensure_dir(fs, &dir).await?;
    }
    Ok(())
}

/// Rien ne porte déjà ce nom dans le dossier : ni fichier (ou lien), ni
/// dossier de surcharge, qui modifierait silencieusement le nouveau service.
async fn ensure_free(fs: &impl RemoteFs, paths: &UnitPaths) -> Result<(), AppError> {
    for target in [&paths.file, &paths.drop_in] {
        if let Some(meta) = fs.lstat(target).await? {
            return Err(AppError::already_exists(
                target,
                meta.kind == EntryKind::Dir,
            ));
        }
    }
    Ok(())
}

/// Écriture atomique : temporaire `.<unit>.atlas-<jeton>.part` créé en
/// exclusivité, écrit, droits `0644`, puis renommé. Le renommage échoue si
/// la cible existe : rien n'est jamais écrasé. En cas d'échec, le temporaire
/// est supprimé.
async fn write_unit_file(
    fs: &impl RemoteFs,
    paths: &UnitPaths,
    unit: &str,
    text: &str,
) -> Result<(), AppError> {
    let temp = path::join(
        &paths.dir,
        &path::temp_name(unit, &new_token(), TempKind::Part),
    );
    let mut writer = fs.create_new(&temp).await?;
    let written = async {
        writer.write_all(text.as_bytes()).await?;
        writer.shutdown().await
    }
    .await;
    drop(writer);
    let result = match written {
        Err(error) => Err(AppError::io(format!(
            "écriture de {} impossible : {error}",
            paths.file
        ))),
        Ok(()) => match fs.set_mode(&temp, UNIT_FILE_MODE).await {
            Err(error) => Err(error),
            Ok(()) => match fs.rename(&temp, &paths.file).await {
                Ok(()) => Ok(()),
                Err(error) => Err(explain_failure(fs, &paths.file, error).await),
            },
        },
    };
    if result.is_err() {
        let _ = fs.remove_file(&temp).await;
    }
    result
}

// --- Création --------------------------------------------------------------

/// Crée, active et démarre un service (démarrage mis en file : son résultat
/// est vérifié par l'interface). Renvoie le service tel que listé.
pub async fn create(host: &impl UnitHost, def: &ServiceDef) -> Result<Service, AppError> {
    let text = unit::render_unit(def)?;
    let unit = unit::unit_name(&def.name)?;
    let fs = host.fs();

    // 1-2. Contrôles, sans rien écrire.
    let checks = Deadline::after(STEP_TIMEOUT);
    let paths = ssh::within(checks, async {
        let paths = UnitPaths::resolve(&**fs, &unit).await?;
        ensure_free(&**fs, &paths).await?;
        Ok(paths)
    })
    .await?;
    let shown = host.run(&show_command(&unit)?, checks).await?;
    if !shown.success() {
        return Err(shown.remote_error());
    }
    check_load_state(&unit, &shown.stdout_text())?;

    // 3-4. Dossiers manquants, écriture atomique.
    let write = Deadline::after(STEP_TIMEOUT);
    ssh::within(write, async {
        ensure_unit_dir(&**fs, &paths.home).await?;
        write_unit_file(&**fs, &paths, &unit, &text).await
    })
    .await?;

    // 5-6. Le fichier est en place : un échec n'efface rien.
    let activate = Deadline::after(STEP_TIMEOUT);
    run_ok(host, RELOAD_COMMAND, activate)
        .await
        .map_err(|e| step_failed(&unit, Sequence::Create, ServiceStep::Reload, e))?;
    run_ok(host, &enable_command(&unit)?, activate)
        .await
        .map_err(|e| step_failed(&unit, Sequence::Create, ServiceStep::Enable, e))?;

    // 7. Le service est créé : une relecture impossible n'est pas un échec.
    let listed = host
        .services(Deadline::after(LIST_TIMEOUT))
        .await
        .ok()
        .and_then(|services| services.into_iter().find(|s| s.unit == unit));
    Ok(listed.unwrap_or_else(|| {
        unloaded_service(&UnitFile {
            name: unit,
            masked: false,
            linked: false,
        })
    }))
}

// --- Suppression -----------------------------------------------------------

/// Suppression vérifiée : service listé, supprimable, fichier ordinaire.
struct Deletion {
    unit: String,
    paths: UnitPaths,
    active: bool,
}

async fn prepare_deletion(
    host: &impl UnitHost,
    unit: &str,
    deadline: Deadline,
) -> Result<Deletion, AppError> {
    let unit = checked_unit(unit)?;
    let services = host.services(deadline).await?;
    let Some(service) = services.iter().find(|s| s.unit == unit) else {
        return Err(AppError::invalid_input(format!(
            "{unit} n'est pas un service de ~/.config/systemd/user/"
        )));
    };
    if let Some(block) = service.delete_block {
        return Err(AppError::invalid_input(format!(
            "{unit} ne peut pas être supprimé ({})",
            block_reason(block)
        )));
    }
    let fs = host.fs();
    let paths = ssh::within(deadline, UnitPaths::resolve(&**fs, unit)).await?;
    match ssh::within(deadline, fs.lstat(&paths.file)).await? {
        Some(meta) if meta.kind == EntryKind::File => {}
        _ => {
            return Err(AppError::invalid_input(format!(
                "{} n'est pas un fichier ordinaire",
                paths.file
            )))
        }
    }
    Ok(Deletion {
        unit: unit.to_owned(),
        paths,
        active: matches!(service.state, RunState::Nominal | RunState::Transition),
    })
}

fn block_reason(block: super::DeleteBlock) -> &'static str {
    match block {
        super::DeleteBlock::InvalidName => "nom hors du format accepté",
        super::DeleteBlock::Masked => "unité masquée",
        super::DeleteBlock::TemplateInstance => "instance d'un gabarit, sans fichier propre",
        super::DeleteBlock::Linked => "lien vers un fichier situé ailleurs",
    }
}

/// Fichier et dossier de surcharge qui seraient supprimés, sans rien modifier.
pub async fn delete_info(host: &impl UnitHost, unit: &str) -> Result<ServiceDeleteInfo, AppError> {
    let deadline = Deadline::after(INFO_TIMEOUT);
    let deletion = prepare_deletion(host, unit, deadline).await?;
    let fs = host.fs();
    let drop_in = &deletion.paths.drop_in;
    let (drop_in, drop_in_entries) = match ssh::within(deadline, fs.lstat(drop_in)).await? {
        Some(meta) if meta.kind == EntryKind::Dir => {
            let summary = ssh::within(deadline, walk::summarize(fs, drop_in, 100_000)).await?;
            (
                Some(drop_in.clone()),
                summary.dirs + summary.files + summary.links,
            )
        }
        Some(meta) if meta.kind == EntryKind::Symlink => (Some(drop_in.clone()), 0),
        _ => (None, 0),
    };
    Ok(ServiceDeleteInfo {
        unit: deletion.unit,
        file: deletion.paths.file,
        drop_in,
        drop_in_entries,
        active: deletion.active,
    })
}

/// Dossier de surcharge : toute l'arborescence, sans suivre les liens. Un
/// lien à cette place est supprimé seul ; absent, rien à faire.
async fn remove_drop_in<F: RemoteFs>(fs: &Arc<F>, drop_in: &str) -> Result<(), AppError> {
    match fs.lstat(drop_in).await? {
        Some(meta) if meta.kind == EntryKind::Dir => walk::delete_tree(fs, drop_in).await,
        Some(meta) if meta.kind == EntryKind::Symlink => fs.remove_file(drop_in).await,
        _ => Ok(()),
    }
}

/// Arrête, désactive et supprime un service, puis recharge systemd. Au
/// premier échec, la séquence s'arrête (`ServiceStepFailed`).
pub async fn delete(host: &impl UnitHost, unit: &str) -> Result<(), AppError> {
    let deletion = prepare_deletion(host, unit, Deadline::after(STEP_TIMEOUT)).await?;
    let unit = deletion.unit.as_str();
    let paths = &deletion.paths;
    let fs = host.fs();
    let failed =
        |step: ServiceStep| move |e: AppError| step_failed(unit, Sequence::Delete, step, e);

    run_ok(host, &stop_command(unit)?, Deadline::after(STOP_TIMEOUT))
        .await
        .map_err(failed(ServiceStep::Stop))?;
    run_ok(host, &disable_command(unit)?, Deadline::after(STEP_TIMEOUT))
        .await
        .map_err(failed(ServiceStep::Disable))?;
    ssh::within(
        Deadline::after(STEP_TIMEOUT),
        remove_drop_in(fs, &paths.drop_in),
    )
    .await
    .map_err(failed(ServiceStep::RemoveDropIn))?;
    ssh::within(Deadline::after(STEP_TIMEOUT), fs.remove_file(&paths.file))
        .await
        .or_else(|e| match e {
            AppError::NotFound { .. } => Ok(()),
            other => Err(other),
        })
        .map_err(failed(ServiceStep::RemoveFile))?;
    run_ok(host, RELOAD_COMMAND, Deadline::after(STEP_TIMEOUT))
        .await
        .map_err(failed(ServiceStep::Reload))?;
    run_ok(
        host,
        &reset_failed_command(unit)?,
        Deadline::after(STEP_TIMEOUT),
    )
    .await
    .map_err(failed(ServiceStep::ResetFailed))?;
    Ok(())
}

/// Lingering du compte : `None` si l'état n'a pas pu être lu.
pub async fn linger(connection: &Connection) -> Option<bool> {
    let output = connection
        .exec_until(LINGER_COMMAND, Deadline::after(LINGER_TIMEOUT))
        .await
        .ok()?;
    if output.success() {
        parse_linger(&output.stdout_text())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::fs::mem::MemFs;
    use crate::systemd::unit::{EnvVar, RestartPolicy};
    use crate::systemd::DeleteBlock;
    use serde_json::json;
    use std::sync::Mutex;

    const HOME: &str = "/home/deploy";
    const DIR: &str = "/home/deploy/.config/systemd/user";

    struct FakeHost {
        fs: Arc<MemFs>,
        commands: Mutex<Vec<String>>,
        /// Commande (début) qui échoue avec le code 1.
        fail: Option<&'static str>,
        /// Sortie de `systemctl show`.
        show: String,
        services: Vec<Service>,
    }

    impl FakeHost {
        fn new(fs: Arc<MemFs>) -> Self {
            Self {
                fs,
                commands: Mutex::default(),
                fail: None,
                show: "LoadState=not-found\nFragmentPath=\n".into(),
                services: Vec::new(),
            }
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }
    }

    impl UnitHost for FakeHost {
        type Fs = MemFs;

        fn fs(&self) -> &Arc<MemFs> {
            &self.fs
        }

        async fn run(&self, command: &str, _: Deadline) -> Result<ExecOutput, AppError> {
            self.commands.lock().unwrap().push(command.to_owned());
            if self.fail.is_some_and(|f| command.starts_with(f)) {
                return Ok(ExecOutput {
                    exit_code: Some(1),
                    stdout: Vec::new(),
                    stderr: b"Failed".to_vec(),
                });
            }
            let stdout = if command.starts_with("systemctl --user show") {
                self.show.clone().into_bytes()
            } else {
                Vec::new()
            };
            Ok(ExecOutput {
                exit_code: Some(0),
                stdout,
                stderr: Vec::new(),
            })
        }

        async fn services(&self, _: Deadline) -> Result<Vec<Service>, AppError> {
            Ok(self.services.clone())
        }
    }

    fn def() -> ServiceDef {
        ServiceDef {
            name: "bot".into(),
            description: "Bot".into(),
            exec_start: "/usr/bin/node app.js".into(),
            working_dir: "~/bot".into(),
            restart: RestartPolicy::OnFailure,
            env: vec![EnvVar {
                key: "NODE_ENV".into(),
                value: "production".into(),
            }],
        }
    }

    fn service(unit: &str, state: RunState, delete_block: Option<DeleteBlock>) -> Service {
        Service {
            unit: unit.into(),
            load: Some("loaded".into()),
            active: "active".into(),
            sub: "running".into(),
            description: None,
            state,
            masked: delete_block == Some(DeleteBlock::Masked),
            controllable: true,
            delete_block,
        }
    }

    fn step_of(error: AppError) -> (ServiceStep, String) {
        match error {
            AppError::ServiceStepFailed { step, state, .. } => (step, state),
            other => panic!("attendu ServiceStepFailed, reçu {other:?}"),
        }
    }

    fn temp_files(fs: &MemFs) -> Vec<String> {
        fs.paths()
            .into_iter()
            .filter(|p| p.ends_with(".part"))
            .collect()
    }

    // --- Commandes ---------------------------------------------------------

    #[test]
    fn commands_are_quoted_after_double_dash() {
        assert_eq!(
            show_command("bot.service").unwrap(),
            "systemctl --user show --property=LoadState --property=FragmentPath -- 'bot.service'"
        );
        assert_eq!(
            enable_command("bot.service").unwrap(),
            "systemctl --user enable --now --no-block -- 'bot.service'"
        );
        assert_eq!(
            stop_command("bot.service").unwrap(),
            "systemctl --user stop -- 'bot.service'"
        );
        assert_eq!(
            disable_command("bot.service").unwrap(),
            "systemctl --user disable -- 'bot.service'"
        );
        assert_eq!(
            reset_failed_command("bot.service").unwrap(),
            "if systemctl --user is-failed --quiet -- 'bot.service'; \
             then systemctl --user reset-failed -- 'bot.service'; fi"
        );
    }

    #[test]
    fn commands_refuse_hostile_names() {
        for hostile in ["x; rm -rf ~.service", "$(id).service", "a\nb.service", ""] {
            for command in [
                show_command,
                enable_command,
                stop_command,
                disable_command,
                reset_failed_command,
            ] {
                assert!(
                    matches!(command(hostile), Err(AppError::InvalidInput { .. })),
                    "{hostile:?}"
                );
            }
        }
    }

    #[test]
    fn load_state_tells_if_the_name_is_free() {
        assert!(check_load_state("bot.service", "LoadState=not-found\nFragmentPath=\n").is_ok());
        assert!(matches!(
            check_load_state(
                "dbus.service",
                "LoadState=loaded\nFragmentPath=/usr/lib/systemd/user/dbus.service\n"
            ),
            Err(AppError::AlreadyExists { path, is_dir: false })
                if path == "/usr/lib/systemd/user/dbus.service"
        ));
        assert!(matches!(
            check_load_state("x.service", "LoadState=masked\nFragmentPath=/dev/null\n"),
            Err(AppError::InvalidInput { .. })
        ));
        assert!(matches!(
            check_load_state("x.service", "LoadState=loaded\nFragmentPath=\n"),
            Err(AppError::InvalidInput { .. })
        ));
        assert!(matches!(
            check_load_state("x.service", "Unknown"),
            Err(AppError::Io { .. })
        ));
        // Ordre des propriétés indifférent.
        assert!(check_load_state("x.service", "FragmentPath=\nLoadState=not-found").is_ok());
    }

    #[test]
    fn linger_answers_are_parsed() {
        assert_eq!(parse_linger("yes\n"), Some(true));
        assert_eq!(parse_linger("no"), Some(false));
        assert_eq!(parse_linger(""), None);
        assert_eq!(parse_linger("Failed to get user"), None);
    }

    #[test]
    fn every_failure_has_a_state() {
        for step in [ServiceStep::Reload, ServiceStep::Enable] {
            assert!(failure_state(Sequence::Create, step).starts_with("Fichier créé"));
        }
        let states: std::collections::HashSet<_> = [
            ServiceStep::Stop,
            ServiceStep::Disable,
            ServiceStep::RemoveDropIn,
            ServiceStep::RemoveFile,
            ServiceStep::Reload,
            ServiceStep::ResetFailed,
        ]
        .into_iter()
        .map(|step| failure_state(Sequence::Delete, step))
        .collect();
        assert_eq!(states.len(), 6);
    }

    // --- Création ----------------------------------------------------------

    #[tokio::test]
    async fn create_writes_the_unit_then_reloads_and_enables() {
        let host = FakeHost {
            services: vec![service("bot.service", RunState::Transition, None)],
            ..FakeHost::new(MemFs::new(HOME))
        };
        let created = create(&host, &def()).await.unwrap();
        assert_eq!(created.unit, "bot.service");

        let file = format!("{DIR}/bot.service");
        assert_eq!(
            String::from_utf8(host.fs.content(&file).unwrap()).unwrap(),
            unit::render_unit(&def()).unwrap()
        );
        assert_eq!(host.fs.mode(&file), Some(0o100_644));
        assert!(temp_files(&host.fs).is_empty());
        assert_eq!(
            host.commands(),
            [
                show_command("bot.service").unwrap(),
                RELOAD_COMMAND.to_owned(),
                enable_command("bot.service").unwrap(),
            ]
        );
    }

    #[tokio::test]
    async fn create_makes_missing_directories() {
        let fs = MemFs::new(HOME);
        assert!(!fs.exists(&format!("{HOME}/.config")));
        let host = FakeHost::new(fs);
        create(&host, &def()).await.unwrap();
        for dir in [".config", ".config/systemd", ".config/systemd/user"] {
            assert!(host.fs.exists(&format!("{HOME}/{dir}")), "{dir}");
        }
    }

    #[tokio::test]
    async fn create_keeps_existing_directories_and_files() {
        let fs = MemFs::new(HOME);
        fs.add_file(&format!("{DIR}/api.service"), b"[Unit]\n");
        let host = FakeHost::new(fs);
        create(&host, &def()).await.unwrap();
        assert_eq!(
            host.fs.content(&format!("{DIR}/api.service")).unwrap(),
            b"[Unit]\n"
        );
    }

    #[tokio::test]
    async fn create_returns_an_unloaded_service_if_the_list_lacks_it() {
        let host = FakeHost::new(MemFs::new(HOME));
        let created = create(&host, &def()).await.unwrap();
        assert_eq!(created.unit, "bot.service");
        assert_eq!(created.delete_block, None);
    }

    #[tokio::test]
    async fn create_never_overwrites_an_existing_file() {
        for existing in ["bot.service", "bot.service.d/override.conf"] {
            let fs = MemFs::new(HOME);
            fs.add_file(&format!("{DIR}/{existing}"), b"ancien");
            let host = FakeHost::new(fs);
            let error = create(&host, &def()).await.unwrap_err();
            assert!(
                matches!(error, AppError::AlreadyExists { .. }),
                "{existing} : {error:?}"
            );
            assert_eq!(
                host.fs.content(&format!("{DIR}/{existing}")).unwrap(),
                b"ancien"
            );
            assert!(host.commands().is_empty(), "{existing}");
        }
        // Un lien (même masqué) porte aussi le nom.
        let fs = MemFs::new(HOME);
        fs.add_link(&format!("{DIR}/bot.service"), "/dev/null");
        let host = FakeHost::new(fs);
        assert!(matches!(
            create(&host, &def()).await,
            Err(AppError::AlreadyExists { .. })
        ));
    }

    #[tokio::test]
    async fn create_refuses_a_name_used_by_a_system_unit() {
        let host = FakeHost {
            show: "LoadState=loaded\nFragmentPath=/usr/lib/systemd/user/bot.service\n".into(),
            ..FakeHost::new(MemFs::new(HOME))
        };
        assert!(matches!(
            create(&host, &def()).await,
            Err(AppError::AlreadyExists { path, .. }) if path == "/usr/lib/systemd/user/bot.service"
        ));
        assert!(!host.fs.exists(&format!("{DIR}/bot.service")));
        assert_eq!(host.commands().len(), 1);
    }

    #[tokio::test]
    async fn create_refuses_invalid_definitions_before_anything() {
        let host = FakeHost::new(MemFs::new(HOME));
        let invalid = ServiceDef {
            exec_start: "/bin/echo a\nExecStartPre=/bin/rm -rf ~".into(),
            ..def()
        };
        assert!(matches!(
            create(&host, &invalid).await,
            Err(AppError::InvalidInput { .. })
        ));
        assert!(host.commands().is_empty());
        assert!(!host.fs.exists(&format!("{HOME}/.config")));
    }

    #[tokio::test]
    async fn create_refuses_a_config_path_that_is_not_a_directory() {
        let fs = MemFs::new(HOME);
        fs.add_file(&format!("{HOME}/.config"), b"");
        let host = FakeHost::new(fs);
        assert!(matches!(
            create(&host, &def()).await,
            Err(AppError::InvalidInput { .. })
        ));
        assert!(temp_files(&host.fs).is_empty());
    }

    #[tokio::test]
    async fn failed_reload_or_enable_keeps_the_file() {
        for (fail, expected) in [
            (RELOAD_COMMAND, ServiceStep::Reload),
            ("systemctl --user enable", ServiceStep::Enable),
        ] {
            let host = FakeHost {
                fail: Some(fail),
                ..FakeHost::new(MemFs::new(HOME))
            };
            let (step, state) = step_of(create(&host, &def()).await.unwrap_err());
            assert_eq!(step, expected);
            assert!(state.starts_with("Fichier créé"));
            assert!(host.fs.exists(&format!("{DIR}/bot.service")));
        }
    }

    #[tokio::test]
    async fn atomic_write_refuses_a_target_that_appeared_and_cleans_up() {
        let fs = MemFs::new(HOME);
        fs.mkdir_all(DIR);
        let paths = UnitPaths::new(HOME, "bot.service");
        // La cible apparaît entre les contrôles et le renommage.
        fs.add_file(&paths.file, b"concurrent");
        let error = write_unit_file(&*fs, &paths, "bot.service", "[Unit]\n")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::AlreadyExists { .. }), "{error:?}");
        assert_eq!(fs.content(&paths.file).unwrap(), b"concurrent");
        assert!(temp_files(&fs).is_empty());
    }

    // --- Suppression ---------------------------------------------------------

    /// Dossier avec `bot.service`, son dossier de surcharge et un autre service.
    fn deletion_fs() -> Arc<MemFs> {
        let fs = MemFs::new(HOME);
        fs.add_file(&format!("{DIR}/bot.service"), b"[Unit]\n");
        fs.add_file(
            &format!("{DIR}/bot.service.d/override.conf"),
            b"[Service]\n",
        );
        fs.add_file(&format!("{DIR}/bot.service.d/sub/x.conf"), b"");
        fs.add_link(
            &format!("{DIR}/bot.service.d/lien.conf"),
            "/srv/garder.conf",
        );
        fs.add_file("/srv/garder.conf", b"cible");
        fs.add_file(&format!("{DIR}/api.service"), b"[Unit]\n");
        fs.add_file(&format!("{DIR}/default.target.wants/api.service"), b"");
        fs
    }

    fn deletion_host() -> FakeHost {
        FakeHost {
            services: vec![
                service("bot.service", RunState::Nominal, None),
                service("api.service", RunState::Nominal, None),
            ],
            ..FakeHost::new(deletion_fs())
        }
    }

    #[tokio::test]
    async fn delete_runs_the_full_sequence_in_order() {
        let host = deletion_host();
        delete(&host, "bot.service").await.unwrap();
        assert_eq!(
            host.commands(),
            [
                stop_command("bot.service").unwrap(),
                disable_command("bot.service").unwrap(),
                RELOAD_COMMAND.to_owned(),
                reset_failed_command("bot.service").unwrap(),
            ]
        );
        assert!(!host.fs.exists(&format!("{DIR}/bot.service")));
        assert!(!host.fs.exists(&format!("{DIR}/bot.service.d")));
        // Le lien du dossier de surcharge est supprimé seul, jamais sa cible.
        assert_eq!(host.fs.content("/srv/garder.conf").unwrap(), b"cible");
        assert!(host.fs.exists(&format!("{DIR}/api.service")));
        assert!(host
            .fs
            .exists(&format!("{DIR}/default.target.wants/api.service")));
    }

    #[tokio::test]
    async fn delete_without_drop_in_or_with_a_drop_in_link() {
        let fs = MemFs::new(HOME);
        fs.add_file(&format!("{DIR}/bot.service"), b"");
        let host = FakeHost {
            services: vec![service("bot.service", RunState::Stopped, None)],
            ..FakeHost::new(fs)
        };
        delete(&host, "bot.service").await.unwrap();
        assert!(!host.fs.exists(&format!("{DIR}/bot.service")));

        let fs = MemFs::new(HOME);
        fs.add_file(&format!("{DIR}/bot.service"), b"");
        fs.add_file("/srv/surcharges/a.conf", b"garder");
        fs.add_link(&format!("{DIR}/bot.service.d"), "/srv/surcharges");
        let host = FakeHost {
            services: vec![service("bot.service", RunState::Stopped, None)],
            ..FakeHost::new(fs)
        };
        delete(&host, "bot.service").await.unwrap();
        assert!(!host.fs.exists(&format!("{DIR}/bot.service.d")));
        assert_eq!(
            host.fs.content("/srv/surcharges/a.conf").unwrap(),
            b"garder"
        );
    }

    #[tokio::test]
    async fn delete_refuses_blocked_unknown_and_non_file_units() {
        let fs = deletion_fs();
        fs.add_link(&format!("{DIR}/linked.service"), "/srv/linked.service");
        let host = FakeHost {
            services: vec![
                service(
                    "bot@1.service",
                    RunState::Nominal,
                    Some(DeleteBlock::TemplateInstance),
                ),
                service("off.service", RunState::Stopped, Some(DeleteBlock::Masked)),
                service(
                    "linked.service",
                    RunState::Stopped,
                    Some(DeleteBlock::Linked),
                ),
                // Fichier remplacé par un lien depuis la liste.
                service("ghost.service", RunState::Stopped, None),
            ],
            ..FakeHost::new(fs)
        };
        host.fs
            .add_link(&format!("{DIR}/ghost.service"), "/dev/null");
        for unit in [
            "bot@1.service",
            "off.service",
            "linked.service",
            "ghost.service",
            "dbus.service",
            "x; rm -rf ~.service",
        ] {
            assert!(
                matches!(
                    delete(&host, unit).await,
                    Err(AppError::InvalidInput { .. })
                ),
                "{unit}"
            );
            assert!(
                matches!(
                    delete_info(&host, unit).await,
                    Err(AppError::InvalidInput { .. })
                ),
                "{unit}"
            );
        }
        assert!(host.commands().is_empty());
        assert!(host.fs.exists(&format!("{DIR}/linked.service")));
    }

    #[tokio::test]
    async fn delete_stops_at_the_first_failed_step() {
        let bot = format!("{DIR}/bot.service");
        let drop_in = format!("{DIR}/bot.service.d");

        // Arrêt en échec : rien n'est modifié.
        let host = FakeHost {
            fail: Some("systemctl --user stop"),
            ..deletion_host()
        };
        let (step, state) = step_of(delete(&host, "bot.service").await.unwrap_err());
        assert_eq!(step, ServiceStep::Stop);
        assert!(state.starts_with("Rien n'a été modifié"));
        assert_eq!(host.commands().len(), 1);
        assert!(host.fs.exists(&bot) && host.fs.exists(&drop_in));

        // Désactivation en échec : arrêté, fichiers en place.
        let host = FakeHost {
            fail: Some("systemctl --user disable"),
            ..deletion_host()
        };
        let (step, _) = step_of(delete(&host, "bot.service").await.unwrap_err());
        assert_eq!(step, ServiceStep::Disable);
        assert_eq!(host.commands().len(), 2);
        assert!(host.fs.exists(&bot) && host.fs.exists(&drop_in));

        // Dossier de surcharge en échec : le fichier reste, rien n'est rechargé.
        let host = deletion_host();
        host.fs
            .faults
            .lock()
            .unwrap()
            .undeletable
            .insert(format!("{drop_in}/override.conf"));
        let (step, _) = step_of(delete(&host, "bot.service").await.unwrap_err());
        assert_eq!(step, ServiceStep::RemoveDropIn);
        assert!(host.fs.exists(&bot));
        assert_eq!(host.commands().len(), 2);

        // Fichier en échec : dossier de surcharge supprimé, fichier en place.
        let host = deletion_host();
        host.fs
            .faults
            .lock()
            .unwrap()
            .undeletable
            .insert(bot.clone());
        let (step, state) = step_of(delete(&host, "bot.service").await.unwrap_err());
        assert_eq!(step, ServiceStep::RemoveFile);
        assert!(state.contains("fichier en place"));
        assert!(host.fs.exists(&bot) && !host.fs.exists(&drop_in));
        assert_eq!(host.commands().len(), 2);

        // Rechargement en échec : fichier supprimé, pas de reset-failed.
        let host = FakeHost {
            fail: Some(RELOAD_COMMAND),
            ..deletion_host()
        };
        let (step, _) = step_of(delete(&host, "bot.service").await.unwrap_err());
        assert_eq!(step, ServiceStep::Reload);
        assert!(!host.fs.exists(&bot));
        assert_eq!(host.commands().len(), 3);

        // reset-failed en échec.
        let host = FakeHost {
            fail: Some("if systemctl"),
            ..deletion_host()
        };
        let (step, _) = step_of(delete(&host, "bot.service").await.unwrap_err());
        assert_eq!(step, ServiceStep::ResetFailed);
    }

    #[tokio::test]
    async fn delete_info_lists_what_will_be_removed() {
        let host = deletion_host();
        let info = delete_info(&host, "bot.service").await.unwrap();
        assert_eq!(
            info,
            ServiceDeleteInfo {
                unit: "bot.service".into(),
                file: format!("{DIR}/bot.service"),
                drop_in: Some(format!("{DIR}/bot.service.d")),
                // override.conf, lien.conf, sub/ et sub/x.conf.
                drop_in_entries: 4,
                active: true,
            }
        );
        assert!(host.commands().is_empty());
        assert!(host.fs.exists(&format!("{DIR}/bot.service.d")));

        let fs = MemFs::new(HOME);
        fs.add_file(&format!("{DIR}/bot.service"), b"");
        let host = FakeHost {
            services: vec![service("bot.service", RunState::Failed, None)],
            ..FakeHost::new(fs)
        };
        let info = delete_info(&host, "bot.service").await.unwrap();
        assert_eq!(info.drop_in, None);
        assert!(!info.active);
    }

    #[test]
    fn delete_info_serializes_in_camel_case() {
        let info = ServiceDeleteInfo {
            unit: "bot.service".into(),
            file: format!("{DIR}/bot.service"),
            drop_in: None,
            drop_in_entries: 0,
            active: false,
        };
        assert_eq!(
            serde_json::to_value(info).unwrap(),
            json!({
                "unit": "bot.service",
                "file": format!("{DIR}/bot.service"),
                "dropIn": null,
                "dropInEntries": 0,
                "active": false,
            })
        );
    }
}
