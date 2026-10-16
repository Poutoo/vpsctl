//! Envoi et téléchargement d'un fichier, par blocs : un fichier n'est jamais
//! chargé entier en mémoire.
//!
//! - Préparation (`prepare_*`) : vérifications avant de rendre la main à
//!   l'interface (fichier local, cible, espace libre). Ses erreurs sont
//!   renvoyées par la commande elle-même.
//! - Exécution (`run_*`) : copie vers un temporaire `.<nom>.atlas-<jeton>.part`
//!   du dossier de destination, renommé à la fin seulement. Annulation ou
//!   erreur : le temporaire est supprimé et la cible reste intacte. La
//!   progression et l'événement final partent vers un `ProgressSink`.
//! - Écrasement d'un fichier distant, en trois étapes pour que le contenu
//!   complet existe toujours sous un nom : cible → `.old`, temporaire →
//!   cible, suppression du `.old`.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::Instant;
use ts_rs::TS;

use super::fs::RemoteFs;
use super::ops::explain_failure;
use super::path::{self, TempKind};
use super::EntryKind;
use crate::error::{AppError, FileSide};

/// Taille d'un bloc lu puis écrit.
pub const BLOCK_SIZE: usize = 256 * 1024;
/// Intervalle minimal entre deux événements de progression.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum Progress {
    Progress {
        #[ts(type = "number")]
        transferred: u64,
        #[ts(type = "number")]
        total: u64,
    },
    Done,
    Cancelled,
    Error {
        error: AppError,
    },
}

/// Destination des événements. Renvoie `false` si l'interface ne les reçoit
/// plus : le transfert est alors annulé.
pub type ProgressSink = Arc<dyn Fn(Progress) -> bool + Send + Sync>;

/// Signal d'annulation. L'abandon de l'émetteur vaut annulation.
pub type CancelSignal = oneshot::Receiver<()>;

/// Fin d'un transfert qui n'a pas abouti.
enum Stop {
    Cancelled,
    Failed(AppError),
}

impl From<AppError> for Stop {
    fn from(error: AppError) -> Self {
        Self::Failed(error)
    }
}

fn final_event(outcome: Result<(), Stop>) -> Progress {
    match outcome {
        Ok(()) => Progress::Done,
        Err(Stop::Cancelled) => Progress::Cancelled,
        Err(Stop::Failed(error)) => Progress::Error { error },
    }
}

/// Jeton des temporaires d'un transfert (8 caractères hexadécimaux).
fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..path::TOKEN_LEN].to_owned()
}

/// Erreur d'un fichier local.
fn local_error(file: &Path, error: &io::Error, needed: Option<u64>) -> AppError {
    let shown = file.display().to_string();
    match error.kind() {
        io::ErrorKind::NotFound => AppError::not_found(shown, FileSide::Local),
        io::ErrorKind::PermissionDenied => AppError::permission_denied(shown, FileSide::Local),
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => AppError::InsufficientSpace {
            path: shown,
            side: FileSide::Local,
            needed,
            available: None,
        },
        _ => AppError::io(format!("{shown} : {error}")),
    }
}

fn absolute_local(local: &str) -> Result<PathBuf, AppError> {
    let file = PathBuf::from(local);
    if !file.is_absolute() {
        return Err(AppError::invalid_input(format!(
            "chemin local non absolu : {local}"
        )));
    }
    Ok(file)
}

/// Erreur de la copie : annulée, ou échec de la lecture ou de l'écriture
/// (avec le nombre d'octets déjà copiés).
enum CopyError {
    Cancelled,
    Read(io::Error),
    Write(io::Error, u64),
}

/// Copie `reader` dans `writer` par blocs de `BLOCK_SIZE`, en émettant la
/// progression. S'arrête entre deux blocs, ou pendant une lecture ou une
/// écriture en attente, si l'annulation est demandée.
async fn copy<R, W>(
    reader: &mut R,
    writer: &mut W,
    total: u64,
    cancel: &mut CancelSignal,
    sink: &ProgressSink,
) -> Result<u64, CopyError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = vec![0_u8; BLOCK_SIZE];
    let mut transferred = 0_u64;
    if !sink(Progress::Progress { transferred, total }) {
        return Err(CopyError::Cancelled);
    }
    let mut reported = Instant::now();
    loop {
        let read = tokio::select! {
            biased;
            _ = &mut *cancel => return Err(CopyError::Cancelled),
            read = reader.read(&mut buffer) => read.map_err(CopyError::Read)?,
        };
        if read == 0 {
            return Ok(transferred);
        }
        tokio::select! {
            biased;
            _ = &mut *cancel => return Err(CopyError::Cancelled),
            written = writer.write_all(&buffer[..read]) => {
                written.map_err(|e| CopyError::Write(e, transferred))?
            }
        }
        transferred += read as u64;
        if reported.elapsed() >= PROGRESS_INTERVAL {
            reported = Instant::now();
            if !sink(Progress::Progress { transferred, total }) {
                return Err(CopyError::Cancelled);
            }
        }
    }
}

// --- Envoi ---------------------------------------------------------------------

/// Envoi vérifié, prêt à être lancé.
#[derive(Debug)]
pub struct Upload {
    local: PathBuf,
    remote: String,
    dir: String,
    temp: String,
    backup: String,
    size: u64,
    overwrite: bool,
}

/// Vérifie un envoi : fichier local ordinaire, cible absente (ou fichier
/// ordinaire avec `overwrite`), jamais un dossier ni un lien, espace libre
/// suffisant si le serveur l'indique.
pub async fn prepare_upload(
    fs: &impl RemoteFs,
    local: &str,
    remote: &str,
    overwrite: bool,
) -> Result<Upload, AppError> {
    let (dir, name) = path::validate_item(remote)?;
    let local = absolute_local(local)?;
    let metadata = tokio::fs::metadata(&local)
        .await
        .map_err(|e| local_error(&local, &e, None))?;
    if !metadata.is_file() {
        return Err(AppError::invalid_input(format!(
            "seuls les fichiers peuvent être envoyés : {}",
            local.display()
        )));
    }
    let size = metadata.len();
    check_target(fs, remote, overwrite).await?;
    if let Ok(Some(available)) = fs.available_space(dir).await {
        if available < size {
            return Err(AppError::InsufficientSpace {
                path: dir.to_owned(),
                side: FileSide::Remote,
                needed: Some(size),
                available: Some(available),
            });
        }
    }
    let token = new_token();
    Ok(Upload {
        temp: path::join(dir, &path::temp_name(name, &token, TempKind::Part)),
        backup: path::join(dir, &path::temp_name(name, &token, TempKind::Old)),
        dir: dir.to_owned(),
        remote: remote.to_owned(),
        local,
        size,
        overwrite,
    })
}

/// Cible d'un envoi. `Ok(true)` : un fichier ordinaire existe, à remplacer.
async fn check_target(fs: &impl RemoteFs, remote: &str, overwrite: bool) -> Result<bool, AppError> {
    let Some(meta) = fs.lstat(remote).await? else {
        return Ok(false);
    };
    match meta.kind {
        EntryKind::File if overwrite => Ok(true),
        EntryKind::File => Err(AppError::already_exists(remote, false)),
        EntryKind::Dir => Err(AppError::already_exists(remote, true)),
        EntryKind::Symlink => {
            let target = fs
                .read_link(remote)
                .await
                .unwrap_or_else(|_| "une cible illisible".to_owned());
            Err(AppError::invalid_input(format!(
                "{remote} est un lien symbolique vers {target} : un envoi ne remplace jamais un lien"
            )))
        }
        EntryKind::Other => Err(AppError::invalid_input(format!(
            "{remote} n'est pas un fichier ordinaire : il ne peut pas être remplacé"
        ))),
    }
}

/// Exécute un envoi préparé, puis émet l'événement final.
pub async fn run_upload<F: RemoteFs>(
    fs: Arc<F>,
    upload: Upload,
    mut cancel: CancelSignal,
    sink: ProgressSink,
) {
    let outcome = upload_file(&*fs, &upload, &mut cancel, &sink).await;
    sink(final_event(outcome));
}

async fn upload_file(
    fs: &impl RemoteFs,
    upload: &Upload,
    cancel: &mut CancelSignal,
    sink: &ProgressSink,
) -> Result<(), Stop> {
    let mut source = tokio::fs::File::open(&upload.local)
        .await
        .map_err(|e| local_error(&upload.local, &e, None))?;
    // Les erreurs du temporaire (caché) sont rapportées sur son dossier.
    let mut writer = fs
        .create_new(&upload.temp)
        .await
        .map_err(|error| match error {
            AppError::PermissionDenied { side, .. } => {
                AppError::permission_denied(&upload.dir, side)
            }
            AppError::NotFound { side, .. } => AppError::not_found(&upload.dir, side),
            other => other,
        })?;

    let copied = match copy(&mut source, &mut writer, upload.size, cancel, sink).await {
        // Fermeture du fichier distant : attend la confirmation des écritures.
        Ok(copied) => writer
            .shutdown()
            .await
            .map_err(|e| CopyError::Write(e, copied)),
        Err(error) => Err(error),
    };
    drop(writer);
    if let Err(error) = copied {
        let stop = match error {
            CopyError::Cancelled => Stop::Cancelled,
            CopyError::Read(e) => Stop::Failed(local_error(&upload.local, &e, None)),
            CopyError::Write(e, written) => {
                Stop::Failed(explain_write(fs, upload, written, &e).await)
            }
        };
        let _ = fs.remove_file(&upload.temp).await;
        return Err(stop);
    }

    if let Err(error) = commit_upload(fs, upload).await {
        let _ = fs.remove_file(&upload.temp).await;
        return Err(Stop::Failed(error));
    }
    Ok(())
}

/// Cause d'un échec d'écriture distante (le serveur ne renvoie qu'un échec
/// générique) : liaison perdue, ou espace disque épuisé.
async fn explain_write(
    fs: &impl RemoteFs,
    upload: &Upload,
    written: u64,
    error: &io::Error,
) -> AppError {
    let remaining = upload.size.saturating_sub(written);
    match fs.available_space(&upload.dir).await {
        Err(_) => AppError::network(format!("liaison perdue pendant l'envoi : {error}")),
        Ok(Some(available)) if available < remaining => AppError::InsufficientSpace {
            path: upload.dir.clone(),
            side: FileSide::Remote,
            needed: Some(upload.size),
            available: Some(available),
        },
        Ok(_) => AppError::io(format!(
            "écriture de {} impossible : {error}",
            upload.remote
        )),
    }
}

/// Met le temporaire complet à la place de la cible. La cible est relue :
/// elle a pu apparaître ou disparaître depuis la préparation.
async fn commit_upload(fs: &impl RemoteFs, upload: &Upload) -> Result<(), AppError> {
    let replace = check_target(fs, &upload.remote, upload.overwrite).await?;
    if !replace {
        return match fs.rename(&upload.temp, &upload.remote).await {
            Ok(()) => Ok(()),
            Err(error) => Err(explain_failure(fs, &upload.remote, error).await),
        };
    }
    // Droits du fichier remplacé, recopiés sur le nouveau (au mieux).
    if let Ok(Some(meta)) = fs.lstat(&upload.remote).await {
        if let Some(mode) = meta.mode {
            let _ = fs.set_mode(&upload.temp, mode).await;
        }
    }
    // 1. La cible est mise de côté : en cas d'échec, elle n'a pas bougé.
    fs.rename(&upload.remote, &upload.backup).await?;
    // 2. Le nouveau contenu prend sa place ; en cas d'échec, la cible revient.
    if let Err(error) = fs.rename(&upload.temp, &upload.remote).await {
        let _ = fs.rename(&upload.backup, &upload.remote).await;
        return Err(error);
    }
    // 3. L'ancien contenu est supprimé. En cas d'échec, il reste un `.old`,
    //    signalé dans la liste comme résidu de transfert.
    let _ = fs.remove_file(&upload.backup).await;
    Ok(())
}

// --- Téléchargement ------------------------------------------------------------

/// Téléchargement vérifié, prêt à être lancé.
#[derive(Debug)]
pub struct Download {
    remote: String,
    local: PathBuf,
    temp: PathBuf,
    size: u64,
}

/// Vérifie un téléchargement : fichier distant ordinaire (un lien vers un
/// fichier est suivi), dossier local existant. Un fichier local existant
/// sera remplacé (confirmé par la fenêtre d'enregistrement du système).
pub async fn prepare_download(
    fs: &impl RemoteFs,
    remote: &str,
    local: &str,
) -> Result<Download, AppError> {
    path::validate_item(remote)?;
    let local = absolute_local(local)?;
    let (Some(parent), Some(name)) = (local.parent(), local.file_name()) else {
        return Err(AppError::invalid_input(format!(
            "chemin local invalide : {}",
            local.display()
        )));
    };
    let parent_meta = tokio::fs::metadata(parent)
        .await
        .map_err(|e| local_error(parent, &e, None))?;
    if !parent_meta.is_dir() {
        return Err(AppError::invalid_input(format!(
            "{} n'est pas un dossier",
            parent.display()
        )));
    }
    if tokio::fs::metadata(&local)
        .await
        .is_ok_and(|meta| meta.is_dir())
    {
        return Err(AppError::invalid_input(format!(
            "un dossier porte déjà ce nom : {}",
            local.display()
        )));
    }
    let Some(meta) = fs.stat(remote).await? else {
        return Err(AppError::not_found(remote, FileSide::Remote));
    };
    if meta.kind != EntryKind::File {
        return Err(AppError::invalid_input(format!(
            "seuls les fichiers peuvent être téléchargés : {remote}"
        )));
    }
    let temp = parent.join(path::temp_name(
        &name.to_string_lossy(),
        &new_token(),
        TempKind::Part,
    ));
    Ok(Download {
        remote: remote.to_owned(),
        local,
        temp,
        size: meta.size,
    })
}

/// Exécute un téléchargement préparé, puis émet l'événement final.
pub async fn run_download<F: RemoteFs>(
    fs: Arc<F>,
    download: Download,
    mut cancel: CancelSignal,
    sink: ProgressSink,
) {
    let outcome = download_file(&*fs, &download, &mut cancel, &sink).await;
    sink(final_event(outcome));
}

async fn download_file(
    fs: &impl RemoteFs,
    download: &Download,
    cancel: &mut CancelSignal,
    sink: &ProgressSink,
) -> Result<(), Stop> {
    let mut reader = fs.open_read(&download.remote).await?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&download.temp)
        .await
        .map_err(|e| local_error(&download.local, &e, Some(download.size)))?;

    let copied = match copy(&mut reader, &mut file, download.size, cancel, sink).await {
        Ok(copied) => async {
            file.flush().await?;
            file.sync_all().await
        }
        .await
        .map_err(|e| CopyError::Write(e, copied)),
        Err(error) => Err(error),
    };
    // Attend une écriture éventuellement en cours avant de fermer : sous
    // Windows, un fichier ouvert ne peut pas être renommé.
    let _ = file.flush().await;
    drop(file);

    let outcome = match copied {
        Ok(_) => tokio::fs::rename(&download.temp, &download.local)
            .await
            .map_err(|e| Stop::Failed(local_error(&download.local, &e, Some(download.size)))),
        Err(CopyError::Cancelled) => Err(Stop::Cancelled),
        Err(CopyError::Read(e)) => Err(Stop::Failed(explain_read(fs, &download.remote, &e).await)),
        Err(CopyError::Write(e, _)) => Err(Stop::Failed(local_error(
            &download.local,
            &e,
            Some(download.size),
        ))),
    };
    if outcome.is_err() {
        let _ = tokio::fs::remove_file(&download.temp).await;
    }
    outcome
}

/// Cause d'un échec de lecture distante.
async fn explain_read(fs: &impl RemoteFs, remote: &str, error: &io::Error) -> AppError {
    match fs.lstat(remote).await {
        Err(_) => AppError::network(format!(
            "liaison perdue pendant le téléchargement : {error}"
        )),
        Ok(None) => AppError::not_found(remote, FileSide::Remote),
        Ok(Some(_)) => AppError::io(format!("lecture de {remote} impossible : {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::fs::mem::MemFs;
    use std::sync::Mutex;

    /// Dossier local temporaire, supprimé à la fin du test.
    struct LocalDir(PathBuf);

    impl LocalDir {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("vpsctl-transfer-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, name: &str, content: &[u8]) -> String {
            let file = self.0.join(name);
            std::fs::write(&file, content).unwrap();
            file.display().to_string()
        }

        fn path(&self, name: &str) -> String {
            self.0.join(name).display().to_string()
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.0)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for LocalDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Événements reçus.
    fn recorder() -> (ProgressSink, Arc<Mutex<Vec<Progress>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink: ProgressSink = {
            let events = events.clone();
            Arc::new(move |event| {
                events.lock().unwrap().push(event);
                true
            })
        };
        (sink, events)
    }

    fn last(events: &Mutex<Vec<Progress>>) -> String {
        format!("{:?}", events.lock().unwrap().last().unwrap())
    }

    /// Contenu de plusieurs blocs.
    fn content(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    /// Éléments du dossier distant, sans le chemin.
    fn remote_names(fs: &MemFs, dir: &str) -> Vec<String> {
        fs.paths()
            .iter()
            .filter_map(|p| path::split(p).filter(|(parent, _)| *parent == dir))
            .map(|(_, name)| name.to_owned())
            .collect()
    }

    async fn upload(
        fs: &Arc<MemFs>,
        local: &str,
        remote: &str,
        overwrite: bool,
        cancel: CancelSignal,
    ) -> Result<Arc<Mutex<Vec<Progress>>>, AppError> {
        let plan = prepare_upload(&**fs, local, remote, overwrite).await?;
        let (sink, events) = recorder();
        run_upload(fs.clone(), plan, cancel, sink).await;
        Ok(events)
    }

    fn never_cancelled() -> CancelSignal {
        let (sender, receiver) = oneshot::channel();
        std::mem::forget(sender);
        receiver
    }

    #[tokio::test]
    async fn upload_writes_a_temp_file_then_renames_it() {
        let fs = MemFs::new("/home/deploy");
        let local = LocalDir::new();
        let data = content(3 * BLOCK_SIZE + 17);
        let source = local.file("gros.bin", &data);

        let events = upload(
            &fs,
            &source,
            "/home/deploy/gros.bin",
            false,
            never_cancelled(),
        )
        .await
        .unwrap();
        assert_eq!(last(&events), "Done");
        assert!(matches!(
            events.lock().unwrap()[0],
            Progress::Progress { transferred: 0, total } if total == data.len() as u64
        ));
        assert_eq!(fs.content("/home/deploy/gros.bin").unwrap(), data);
        assert_eq!(remote_names(&fs, "/home/deploy"), ["gros.bin"]);
    }

    #[tokio::test]
    async fn existing_targets_are_refused_unless_overwritten() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/f.txt", b"ancien");
        fs.mkdir_all("/home/deploy/dossier");
        fs.add_link("/home/deploy/lien", "/etc/hosts");
        let local = LocalDir::new();
        let source = local.file("f.txt", b"nouveau");

        let refused = |remote: &'static str, overwrite: bool| {
            let fs = fs.clone();
            let source = source.clone();
            async move {
                prepare_upload(&*fs, &source, remote, overwrite)
                    .await
                    .unwrap_err()
            }
        };
        assert!(matches!(
            refused("/home/deploy/f.txt", false).await,
            AppError::AlreadyExists { is_dir: false, .. }
        ));
        assert!(matches!(
            refused("/home/deploy/dossier", true).await,
            AppError::AlreadyExists { is_dir: true, .. }
        ));
        match refused("/home/deploy/lien", true).await {
            AppError::InvalidInput { message } => assert!(message.contains("/etc/hosts")),
            other => panic!("attendu InvalidInput, reçu {other:?}"),
        }
        assert_eq!(fs.content("/home/deploy/f.txt").unwrap(), b"ancien");
    }

    #[tokio::test]
    async fn local_problems_are_reported_before_starting() {
        let fs = MemFs::new("/home/deploy");
        let local = LocalDir::new();
        assert!(matches!(
            prepare_upload(&*fs, &local.path("absent"), "/home/deploy/x", false).await,
            Err(AppError::NotFound {
                side: FileSide::Local,
                ..
            })
        ));
        assert!(matches!(
            prepare_upload(
                &*fs,
                &local.0.display().to_string(),
                "/home/deploy/x",
                false
            )
            .await,
            Err(AppError::InvalidInput { .. })
        ));
        assert!(matches!(
            prepare_upload(&*fs, "relatif.txt", "/home/deploy/x", false).await,
            Err(AppError::InvalidInput { .. })
        ));
    }

    #[tokio::test]
    async fn upload_checks_the_free_space() {
        let fs = MemFs::new("/home/deploy");
        fs.faults.lock().unwrap().space = Some(10);
        let local = LocalDir::new();
        let source = local.file("f", &content(100));
        assert!(matches!(
            prepare_upload(&*fs, &source, "/home/deploy/f", false).await,
            Err(AppError::InsufficientSpace {
                needed: Some(100),
                available: Some(10),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn overwrite_keeps_the_mode_and_leaves_no_residue() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/run.sh", b"ancien");
        fs.set_file_mode("/home/deploy/run.sh", 0o750);
        let local = LocalDir::new();
        let source = local.file("run.sh", b"nouveau contenu");

        let events = upload(&fs, &source, "/home/deploy/run.sh", true, never_cancelled())
            .await
            .unwrap();
        assert_eq!(last(&events), "Done");
        assert_eq!(
            fs.content("/home/deploy/run.sh").unwrap(),
            b"nouveau contenu"
        );
        assert_eq!(fs.mode("/home/deploy/run.sh"), Some(0o100_750));
        assert_eq!(remote_names(&fs, "/home/deploy"), ["run.sh"]);
    }

    #[tokio::test]
    async fn a_failed_swap_restores_the_original() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/f.txt", b"ancien");
        // Le temporaire ne peut pas prendre la place de la cible.
        fs.faults.lock().unwrap().rename_part_fails = true;
        let local = LocalDir::new();
        let source = local.file("f.txt", b"nouveau");

        let events = upload(&fs, &source, "/home/deploy/f.txt", true, never_cancelled())
            .await
            .unwrap();
        assert!(last(&events).starts_with("Error"));
        assert_eq!(fs.content("/home/deploy/f.txt").unwrap(), b"ancien");
        assert_eq!(remote_names(&fs, "/home/deploy"), ["f.txt"]);
    }

    #[tokio::test]
    async fn a_cancelled_upload_removes_its_temp_file() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/f.bin", b"original");
        fs.faults.lock().unwrap().write_stall = Some(BLOCK_SIZE);
        let local = LocalDir::new();
        let source = local.file("f.bin", &content(4 * BLOCK_SIZE));

        let (cancel, cancelled) = oneshot::channel();
        let plan = prepare_upload(&*fs, &source, "/home/deploy/f.bin", true)
            .await
            .unwrap();
        let (sink, events) = recorder();
        let task = tokio::spawn(run_upload(fs.clone(), plan, cancelled, sink));
        // La copie est bloquée sur l'écriture du deuxième bloc.
        while remote_names(&fs, "/home/deploy").len() < 2 {
            tokio::task::yield_now().await;
        }
        cancel.send(()).unwrap();
        task.await.unwrap();

        assert_eq!(last(&events), "Cancelled");
        assert_eq!(fs.content("/home/deploy/f.bin").unwrap(), b"original");
        assert_eq!(remote_names(&fs, "/home/deploy"), ["f.bin"]);
    }

    #[tokio::test]
    async fn an_interface_that_stops_listening_cancels_the_upload() {
        let fs = MemFs::new("/home/deploy");
        let local = LocalDir::new();
        let source = local.file("f", b"abc");
        let plan = prepare_upload(&*fs, &source, "/home/deploy/f", false)
            .await
            .unwrap();
        let sink: ProgressSink = Arc::new(|_| false);
        run_upload(fs.clone(), plan, never_cancelled(), sink).await;
        assert!(remote_names(&fs, "/home/deploy").is_empty());
    }

    #[tokio::test]
    async fn a_full_disk_is_reported_and_cleaned_up() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/f.bin", b"original");
        let local = LocalDir::new();
        let source = local.file("f.bin", &content(2 * BLOCK_SIZE));
        let plan = prepare_upload(&*fs, &source, "/home/deploy/f.bin", true)
            .await
            .unwrap();
        {
            let mut faults = fs.faults.lock().unwrap();
            faults.write_limit = Some(BLOCK_SIZE);
            faults.space = Some(0);
        }
        let (sink, events) = recorder();
        run_upload(fs.clone(), plan, never_cancelled(), sink).await;

        let event = events.lock().unwrap().pop().unwrap();
        assert!(matches!(
            event,
            Progress::Error {
                error: AppError::InsufficientSpace {
                    side: FileSide::Remote,
                    ..
                }
            }
        ));
        assert_eq!(fs.content("/home/deploy/f.bin").unwrap(), b"original");
        assert_eq!(remote_names(&fs, "/home/deploy"), ["f.bin"]);
    }

    #[tokio::test]
    async fn a_lost_link_leaves_the_original_untouched() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/f.bin", b"original");
        let local = LocalDir::new();
        let source = local.file("f.bin", &content(2 * BLOCK_SIZE));
        let plan = prepare_upload(&*fs, &source, "/home/deploy/f.bin", true)
            .await
            .unwrap();
        fs.faults.lock().unwrap().write_limit = Some(BLOCK_SIZE);
        let (sink, events) = recorder();
        let sink: ProgressSink = {
            let fs = fs.clone();
            Arc::new(move |event| {
                // La liaison tombe dès la première mesure de progression.
                fs.faults.lock().unwrap().offline = true;
                sink(event)
            })
        };
        run_upload(fs.clone(), plan, never_cancelled(), sink).await;

        assert!(matches!(
            events.lock().unwrap().last().unwrap(),
            Progress::Error {
                error: AppError::Network { .. }
            }
        ));
        fs.faults.lock().unwrap().offline = false;
        assert_eq!(fs.content("/home/deploy/f.bin").unwrap(), b"original");
        // Le temporaire n'a pas pu être supprimé : il reste un résidu.
        let names = remote_names(&fs, "/home/deploy");
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| path::residue_of(n).is_some()));
    }

    async fn download(
        fs: &Arc<MemFs>,
        remote: &str,
        local: &str,
        cancel: CancelSignal,
    ) -> Result<Arc<Mutex<Vec<Progress>>>, AppError> {
        let plan = prepare_download(&**fs, remote, local).await?;
        let (sink, events) = recorder();
        run_download(fs.clone(), plan, cancel, sink).await;
        Ok(events)
    }

    #[tokio::test]
    async fn download_writes_a_temp_file_then_renames_it() {
        let fs = MemFs::new("/home/deploy");
        let data = content(2 * BLOCK_SIZE + 5);
        fs.add_file("/srv/backup.sql.gz", &data);
        fs.add_link("/home/deploy/dernier", "/srv/backup.sql.gz");
        let local = LocalDir::new();
        local.file("backup.sql.gz", b"ancienne copie");

        let target = local.path("backup.sql.gz");
        let events = download(&fs, "/srv/backup.sql.gz", &target, never_cancelled())
            .await
            .unwrap();
        assert_eq!(last(&events), "Done");
        assert_eq!(std::fs::read(&target).unwrap(), data);
        // Un lien vers un fichier est suivi.
        let via_link = local.path("via-lien");
        download(&fs, "/home/deploy/dernier", &via_link, never_cancelled())
            .await
            .unwrap();
        assert_eq!(std::fs::read(&via_link).unwrap(), data);
        assert_eq!(local.names(), ["backup.sql.gz", "via-lien"]);
    }

    #[tokio::test]
    async fn a_cancelled_download_keeps_the_existing_local_file() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/srv/gros.bin", &content(4 * BLOCK_SIZE));
        fs.faults.lock().unwrap().read_stall = Some(BLOCK_SIZE);
        let local = LocalDir::new();
        let target = local.file("gros.bin", b"existant");

        let (cancel, cancelled) = oneshot::channel();
        let plan = prepare_download(&*fs, "/srv/gros.bin", &target)
            .await
            .unwrap();
        let (sink, events) = recorder();
        let task = tokio::spawn(run_download(fs.clone(), plan, cancelled, sink));
        while local.names().len() < 2 {
            tokio::task::yield_now().await;
        }
        cancel.send(()).unwrap();
        task.await.unwrap();

        assert_eq!(last(&events), "Cancelled");
        assert_eq!(std::fs::read(&target).unwrap(), b"existant");
        assert_eq!(local.names(), ["gros.bin"]);
    }

    #[tokio::test]
    async fn a_read_error_removes_the_local_temp_file() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/srv/f.bin", &content(3 * BLOCK_SIZE));
        fs.faults.lock().unwrap().read_limit = Some(BLOCK_SIZE);
        let local = LocalDir::new();
        let target = local.path("f.bin");

        let events = download(&fs, "/srv/f.bin", &target, never_cancelled())
            .await
            .unwrap();
        assert!(last(&events).starts_with("Error"));
        assert!(local.names().is_empty());
    }

    #[tokio::test]
    async fn only_files_are_downloaded() {
        let fs = MemFs::new("/home/deploy");
        fs.mkdir_all("/srv/dossier");
        let local = LocalDir::new();
        assert!(matches!(
            prepare_download(&*fs, "/srv/dossier", &local.path("d")).await,
            Err(AppError::InvalidInput { .. })
        ));
        assert!(matches!(
            prepare_download(&*fs, "/srv/absent", &local.path("a")).await,
            Err(AppError::NotFound {
                side: FileSide::Remote,
                ..
            })
        ));
        assert!(matches!(
            prepare_download(&*fs, "/srv/dossier", &local.path("absent/x")).await,
            Err(AppError::NotFound {
                side: FileSide::Local,
                ..
            })
        ));
    }
}
