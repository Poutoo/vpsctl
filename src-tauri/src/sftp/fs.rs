//! Trait `RemoteFs` : les opérations élémentaires sur le système de fichiers
//! distant dont ont besoin l'explorateur, le parcours d'arborescence et les
//! transferts. `SftpFs` l'implémente sur la session SFTP partagée ; les
//! tests utilisent un système de fichiers simulé en mémoire (`mem::MemFs`).
//!
//! Les erreurs sont déjà traduites : `NotFound` et `PermissionDenied` côté
//! VPS, `Io` pour un échec générique du serveur (que l'appelant peut préciser
//! par une vérification ciblée), `Network` pour une session perdue.

use std::future::Future;
use std::sync::Arc;

use russh_sftp::client::fs::File;
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::JoinSet;

use super::{path, run, EntryKind, Failure};
use crate::error::AppError;
use crate::ssh::Connection;

/// Requêtes simultanées au plus pour une série d'opérations indépendantes
/// (suppressions, lecture des liens d'un dossier).
pub const PARALLEL_REQUESTS: usize = 16;

/// Attributs d'un élément.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub kind: EntryKind,
    /// Octets. Pour un lien (`lstat`), la longueur de sa cible.
    pub size: u64,
    /// `st_mode` complet (type et droits), s'il est connu.
    pub mode: Option<u32>,
    /// Secondes depuis l'epoch Unix.
    pub modified: Option<u64>,
}

impl From<&FileAttributes> for Meta {
    fn from(attributes: &FileAttributes) -> Self {
        Self {
            kind: path::kind_of_mode(attributes.permissions),
            size: attributes.size.unwrap_or(0),
            mode: attributes.permissions,
            modified: attributes.mtime.map(u64::from),
        }
    }
}

pub trait RemoteFs: Send + Sync + 'static {
    type Reader: AsyncRead + Unpin + Send + 'static;
    type Writer: AsyncWrite + Unpin + Send + 'static;

    /// Attributs sans suivre un lien. `None` si l'élément n'existe pas.
    fn lstat(&self, path: &str) -> impl Future<Output = Result<Option<Meta>, AppError>> + Send;

    /// Attributs en suivant les liens. `None` si l'élément n'existe pas, ou
    /// si c'est un lien cassé.
    fn stat(&self, path: &str) -> impl Future<Output = Result<Option<Meta>, AppError>> + Send;

    /// Contenu d'un dossier (sans `.` ni `..`), avec les attributs de `lstat`.
    fn read_dir(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Vec<(String, Meta)>, AppError>> + Send;

    /// Cible d'un lien, telle qu'enregistrée.
    fn read_link(&self, path: &str) -> impl Future<Output = Result<String, AppError>> + Send;

    /// Chemin absolu résolu (`realpath`). `.` donne le dossier personnel.
    fn real_path(&self, path: &str) -> impl Future<Output = Result<String, AppError>> + Send;

    fn create_dir(&self, path: &str) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Crée un fichier vide et l'ouvre en écriture ; échoue s'il existe.
    fn create_new(&self, path: &str)
        -> impl Future<Output = Result<Self::Writer, AppError>> + Send;

    fn open_read(&self, path: &str) -> impl Future<Output = Result<Self::Reader, AppError>> + Send;

    /// Supprime un fichier ou un lien (le lien seul, jamais sa cible).
    fn remove_file(&self, path: &str) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Supprime un dossier vide.
    fn remove_dir(&self, path: &str) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Renomme ; échoue si la destination existe.
    fn rename(&self, from: &str, to: &str) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Applique les droits (12 bits bas de `mode`).
    fn set_mode(&self, path: &str, mode: u32) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Espace libre pour le compte, en octets. `None` si le serveur ne le
    /// fournit pas.
    fn available_space(
        &self,
        dir: &str,
    ) -> impl Future<Output = Result<Option<u64>, AppError>> + Send;
}

/// Exécute `task` sur chaque élément, `limit` à la fois. À la première
/// erreur, les tâches restantes sont abandonnées et l'erreur renvoyée. Les
/// résultats ne sont pas dans l'ordre des éléments.
pub async fn for_each_limited<T, R, Fut>(
    items: Vec<T>,
    limit: usize,
    task: impl Fn(T) -> Fut,
) -> Result<Vec<R>, AppError>
where
    T: Send + 'static,
    R: Send + 'static,
    Fut: Future<Output = Result<R, AppError>> + Send + 'static,
{
    let mut results = Vec::with_capacity(items.len());
    let mut running = JoinSet::new();
    let collect = |joined: Result<Result<R, AppError>, tokio::task::JoinError>,
                   results: &mut Vec<R>| match joined {
        Ok(Ok(result)) => {
            results.push(result);
            Ok(())
        }
        Ok(Err(error)) => Err(error),
        Err(_) => Err(AppError::io("opération interrompue")),
    };
    for item in items {
        if running.len() >= limit.max(1) {
            if let Some(joined) = running.join_next().await {
                collect(joined, &mut results)?;
            }
        }
        running.spawn(task(item));
    }
    while let Some(joined) = running.join_next().await {
        collect(joined, &mut results)?;
    }
    Ok(results)
}

/// `RemoteFs` sur la session SFTP partagée de la liaison.
pub struct SftpFs {
    connection: Arc<Connection>,
}

impl SftpFs {
    pub fn shared(connection: Arc<Connection>) -> Arc<Self> {
        Arc::new(Self { connection })
    }

    /// Opération sur la session partagée (reprise après une erreur de
    /// transport), erreurs traduites pour `path`.
    async fn call<T, F, Fut>(&self, path: &str, operation: F) -> Result<T, AppError>
    where
        F: Fn(Arc<russh_sftp::client::SftpSession>) -> Fut,
        Fut: Future<Output = Result<T, russh_sftp::client::error::Error>>,
    {
        run(&*self.connection, path, operation)
            .await
            .map_err(|failure| failure.into_error(path))
    }

    /// Comme `call`, mais un élément absent donne `None`.
    async fn call_optional<T, F, Fut>(
        &self,
        path: &str,
        operation: F,
    ) -> Result<Option<T>, AppError>
    where
        F: Fn(Arc<russh_sftp::client::SftpSession>) -> Fut,
        Fut: Future<Output = Result<T, russh_sftp::client::error::Error>>,
    {
        match run(&*self.connection, path, operation).await {
            Ok(value) => Ok(Some(value)),
            Err(Failure::NotFound) => Ok(None),
            Err(failure) => Err(failure.into_error(path)),
        }
    }
}

impl RemoteFs for SftpFs {
    type Reader = File;
    type Writer = File;

    async fn lstat(&self, path: &str) -> Result<Option<Meta>, AppError> {
        let attributes = self
            .call_optional(path, |s| {
                let path = path.to_owned();
                async move { s.symlink_metadata(path).await }
            })
            .await?;
        Ok(attributes.as_ref().map(Meta::from))
    }

    async fn stat(&self, path: &str) -> Result<Option<Meta>, AppError> {
        let attributes = self
            .call_optional(path, |s| {
                let path = path.to_owned();
                async move { s.metadata(path).await }
            })
            .await?;
        Ok(attributes.as_ref().map(Meta::from))
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<(String, Meta)>, AppError> {
        let entries = self
            .call(path, |s| {
                let path = path.to_owned();
                async move { s.read_dir(path).await }
            })
            .await?;
        Ok(entries
            .map(|entry| (entry.file_name(), Meta::from(&entry.metadata())))
            .collect())
    }

    async fn read_link(&self, path: &str) -> Result<String, AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move { s.read_link(path).await }
        })
        .await
    }

    async fn real_path(&self, path: &str) -> Result<String, AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move { s.canonicalize(path).await }
        })
        .await
    }

    async fn create_dir(&self, path: &str) -> Result<(), AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move { s.create_dir(path).await }
        })
        .await
    }

    async fn create_new(&self, path: &str) -> Result<File, AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move {
                s.open_with_flags(
                    path,
                    OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
                )
                .await
            }
        })
        .await
    }

    async fn open_read(&self, path: &str) -> Result<File, AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move { s.open(path).await }
        })
        .await
    }

    async fn remove_file(&self, path: &str) -> Result<(), AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move { s.remove_file(path).await }
        })
        .await
    }

    async fn remove_dir(&self, path: &str) -> Result<(), AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            async move { s.remove_dir(path).await }
        })
        .await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), AppError> {
        self.call(from, |s| {
            let (from, to) = (from.to_owned(), to.to_owned());
            async move { s.rename(from, to).await }
        })
        .await
    }

    async fn set_mode(&self, path: &str, mode: u32) -> Result<(), AppError> {
        self.call(path, |s| {
            let path = path.to_owned();
            let attributes = FileAttributes {
                permissions: Some(mode & 0o7777),
                ..FileAttributes::empty()
            };
            async move { s.set_metadata(path, attributes).await }
        })
        .await
    }

    async fn available_space(&self, dir: &str) -> Result<Option<u64>, AppError> {
        let info = self
            .call(dir, |s| {
                let dir = dir.to_owned();
                async move { s.fs_info(dir).await }
            })
            .await?;
        Ok(info.map(|i| i.blocks_avail.saturating_mul(i.fragment_size)))
    }
}

/// Système de fichiers simulé en mémoire, pour les tests du parcours, des
/// opérations et des transferts. Il reproduit les comportements d'OpenSSH
/// utiles ici : `readdir` donne les attributs de `lstat`, `rename` refuse
/// une destination existante, un élément existant ou un dossier non vide
/// donnent un échec générique (`Io "Failure"`).
#[cfg(test)]
pub mod mem {
    use std::collections::{BTreeMap, HashSet};
    use std::io;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};

    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    use super::{Meta, RemoteFs};
    use crate::error::{AppError, FileSide};
    use crate::sftp::{path, EntryKind};

    type Data = Arc<Mutex<Vec<u8>>>;

    #[derive(Clone)]
    pub enum Node {
        Dir { mode: u32 },
        File { data: Data, mode: u32 },
        Link { target: String },
        Other,
    }

    /// Pannes simulées.
    #[derive(Default)]
    pub struct Faults {
        /// Lecture refusée (`PermissionDenied`) de ces dossiers.
        pub unreadable: HashSet<String>,
        /// Suppression refusée de ces éléments.
        pub undeletable: HashSet<String>,
        /// Renommage d'un temporaire `.part` en échec.
        pub rename_part_fails: bool,
        /// Écriture en échec au-delà de ce nombre d'octets par fichier.
        pub write_limit: Option<usize>,
        /// Lecture en échec au-delà de ce nombre d'octets par fichier.
        pub read_limit: Option<usize>,
        /// Écriture bloquée (jamais terminée) au-delà de ce nombre d'octets.
        pub write_stall: Option<usize>,
        /// Lecture bloquée (jamais terminée) au-delà de ce nombre d'octets.
        pub read_stall: Option<usize>,
        /// Espace libre annoncé.
        pub space: Option<u64>,
        /// Liaison perdue : toute opération échoue en `Network`.
        pub offline: bool,
    }

    pub struct MemFs {
        pub home: String,
        nodes: Mutex<BTreeMap<String, Node>>,
        pub faults: Mutex<Faults>,
    }

    fn failure() -> AppError {
        AppError::io("Failure")
    }

    impl MemFs {
        pub fn new(home: &str) -> Arc<Self> {
            let fs = Self {
                home: home.to_owned(),
                nodes: Mutex::new(BTreeMap::from([(
                    "/".to_owned(),
                    Node::Dir { mode: 0o755 },
                )])),
                faults: Mutex::new(Faults::default()),
            };
            fs.mkdir_all(home);
            Arc::new(fs)
        }

        fn nodes(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Node>> {
            self.nodes.lock().unwrap()
        }

        fn check_online(&self) -> Result<(), AppError> {
            if self.faults.lock().unwrap().offline {
                Err(AppError::network("liaison perdue"))
            } else {
                Ok(())
            }
        }

        pub fn mkdir_all(&self, dir: &str) {
            let mut current = String::from("/");
            for component in dir.split('/').filter(|c| !c.is_empty()) {
                current = path::join(&current, component);
                self.nodes()
                    .entry(current.clone())
                    .or_insert(Node::Dir { mode: 0o755 });
            }
        }

        pub fn add_file(&self, file: &str, content: &[u8]) {
            self.mkdir_all(path::split(file).unwrap().0);
            self.nodes().insert(
                file.to_owned(),
                Node::File {
                    data: Arc::new(Mutex::new(content.to_vec())),
                    mode: 0o100_644,
                },
            );
        }

        pub fn add_link(&self, link: &str, target: &str) {
            self.mkdir_all(path::split(link).unwrap().0);
            self.nodes().insert(
                link.to_owned(),
                Node::Link {
                    target: target.to_owned(),
                },
            );
        }

        pub fn set_file_mode(&self, file: &str, mode: u32) {
            if let Some(Node::File { mode: m, .. }) = self.nodes().get_mut(file) {
                *m = 0o100_000 | mode;
            }
        }

        pub fn exists(&self, path: &str) -> bool {
            self.nodes().contains_key(path)
        }

        pub fn content(&self, file: &str) -> Option<Vec<u8>> {
            match self.nodes().get(file) {
                Some(Node::File { data, .. }) => Some(data.lock().unwrap().clone()),
                _ => None,
            }
        }

        pub fn mode(&self, file: &str) -> Option<u32> {
            match self.nodes().get(file) {
                Some(Node::File { mode, .. }) => Some(*mode),
                _ => None,
            }
        }

        /// Chemins existants, triés.
        pub fn paths(&self) -> Vec<String> {
            self.nodes().keys().cloned().collect()
        }

        fn meta(node: &Node) -> Meta {
            let (kind, size, mode) = match node {
                Node::Dir { mode } => (EntryKind::Dir, 4096, 0o040_000 | mode),
                Node::File { data, mode } => {
                    (EntryKind::File, data.lock().unwrap().len() as u64, *mode)
                }
                Node::Link { target } => (EntryKind::Symlink, target.len() as u64, 0o120_777),
                Node::Other => (EntryKind::Other, 0, 0o010_644),
            };
            Meta {
                kind,
                size,
                mode: Some(mode),
                modified: Some(1_760_000_000),
            }
        }

        /// Suit les liens du dernier composant (cibles absolues ou relatives).
        fn resolve(&self, path: &str) -> Option<String> {
            let nodes = self.nodes();
            let mut current = path.to_owned();
            for _ in 0..40 {
                match nodes.get(&current)? {
                    Node::Link { target } if target.starts_with('/') => current = target.clone(),
                    Node::Link { target } => {
                        current = path::join(path::split(&current)?.0, target);
                    }
                    _ => return Some(current),
                }
            }
            None
        }

        fn children(nodes: &BTreeMap<String, Node>, dir: &str) -> Vec<String> {
            nodes
                .keys()
                .filter(|p| {
                    p.as_str() != dir && path::split(p).is_some_and(|(parent, _)| parent == dir)
                })
                .cloned()
                .collect()
        }

        fn parent_is_dir(&self, path: &str) -> Result<(), AppError> {
            let parent = path::split(path).map(|(p, _)| p).unwrap_or("/");
            match self.nodes().get(parent) {
                Some(Node::Dir { .. }) => Ok(()),
                _ => Err(AppError::not_found(path, FileSide::Remote)),
            }
        }
    }

    pub struct MemWriter {
        data: Data,
        limit: Option<usize>,
        stall: Option<usize>,
    }

    impl AsyncWrite for MemWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let mut data = self.data.lock().unwrap();
            if self.stall.is_some_and(|stall| data.len() >= stall) {
                return Poll::Pending;
            }
            if self
                .limit
                .is_some_and(|limit| data.len() + buf.len() > limit)
            {
                return Poll::Ready(Err(io::Error::other("Failure")));
            }
            data.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    pub struct MemReader {
        data: Vec<u8>,
        position: usize,
        limit: Option<usize>,
        stall: Option<usize>,
    }

    impl AsyncRead for MemReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let mut end = (self.position + buf.remaining()).min(self.data.len());
            if let Some(stall) = self.stall {
                if self.position >= stall && self.position < self.data.len() {
                    return Poll::Pending;
                }
                end = end.min(stall.max(self.position));
            }
            if let Some(limit) = self.limit {
                if self.position >= limit && self.position < self.data.len() {
                    return Poll::Ready(Err(io::Error::other("connection reset")));
                }
                end = end.min(limit.max(self.position));
            }
            buf.put_slice(&self.data[self.position..end]);
            self.position = end;
            Poll::Ready(Ok(()))
        }
    }

    impl RemoteFs for MemFs {
        type Reader = MemReader;
        type Writer = MemWriter;

        async fn lstat(&self, path: &str) -> Result<Option<Meta>, AppError> {
            self.check_online()?;
            Ok(self.nodes().get(path).map(Self::meta))
        }

        async fn stat(&self, path: &str) -> Result<Option<Meta>, AppError> {
            self.check_online()?;
            let Some(resolved) = self.resolve(path) else {
                return Ok(None);
            };
            Ok(self.nodes().get(&resolved).map(Self::meta))
        }

        async fn read_dir(&self, dir: &str) -> Result<Vec<(String, Meta)>, AppError> {
            self.check_online()?;
            if self.faults.lock().unwrap().unreadable.contains(dir) {
                return Err(AppError::permission_denied(dir, FileSide::Remote));
            }
            let resolved = self
                .resolve(dir)
                .ok_or_else(|| AppError::not_found(dir, FileSide::Remote))?;
            let nodes = self.nodes();
            match nodes.get(&resolved) {
                Some(Node::Dir { .. }) => {}
                _ => return Err(AppError::not_found(dir, FileSide::Remote)),
            }
            Ok(Self::children(&nodes, &resolved)
                .into_iter()
                .map(|child| {
                    let name = path::split(&child).unwrap().1.to_owned();
                    let meta = Self::meta(&nodes[&child]);
                    (name, meta)
                })
                .collect())
        }

        async fn read_link(&self, link: &str) -> Result<String, AppError> {
            self.check_online()?;
            match self.nodes().get(link) {
                Some(Node::Link { target }) => Ok(target.clone()),
                Some(_) => Err(failure()),
                None => Err(AppError::not_found(link, FileSide::Remote)),
            }
        }

        async fn real_path(&self, path: &str) -> Result<String, AppError> {
            self.check_online()?;
            if path == "." {
                return Ok(self.home.clone());
            }
            self.resolve(path)
                .ok_or_else(|| AppError::not_found(path, FileSide::Remote))
        }

        async fn create_dir(&self, dir: &str) -> Result<(), AppError> {
            self.check_online()?;
            self.parent_is_dir(dir)?;
            let mut nodes = self.nodes();
            if nodes.contains_key(dir) {
                return Err(failure());
            }
            nodes.insert(dir.to_owned(), Node::Dir { mode: 0o755 });
            Ok(())
        }

        async fn create_new(&self, file: &str) -> Result<MemWriter, AppError> {
            self.check_online()?;
            self.parent_is_dir(file)?;
            let mut nodes = self.nodes();
            if nodes.contains_key(file) {
                return Err(failure());
            }
            let data = Data::default();
            nodes.insert(
                file.to_owned(),
                Node::File {
                    data: data.clone(),
                    mode: 0o100_664,
                },
            );
            let faults = self.faults.lock().unwrap();
            Ok(MemWriter {
                data,
                limit: faults.write_limit,
                stall: faults.write_stall,
            })
        }

        async fn open_read(&self, file: &str) -> Result<MemReader, AppError> {
            self.check_online()?;
            let resolved = self
                .resolve(file)
                .ok_or_else(|| AppError::not_found(file, FileSide::Remote))?;
            let (limit, stall) = {
                let faults = self.faults.lock().unwrap();
                (faults.read_limit, faults.read_stall)
            };
            match self.nodes().get(&resolved) {
                Some(Node::File { data, .. }) => Ok(MemReader {
                    data: data.lock().unwrap().clone(),
                    position: 0,
                    limit,
                    stall,
                }),
                Some(_) => Err(failure()),
                None => Err(AppError::not_found(file, FileSide::Remote)),
            }
        }

        async fn remove_file(&self, file: &str) -> Result<(), AppError> {
            self.check_online()?;
            if self.faults.lock().unwrap().undeletable.contains(file) {
                return Err(AppError::permission_denied(file, FileSide::Remote));
            }
            let mut nodes = self.nodes();
            match nodes.get(file) {
                None => Err(AppError::not_found(file, FileSide::Remote)),
                Some(Node::Dir { .. }) => Err(failure()),
                Some(_) => {
                    nodes.remove(file);
                    Ok(())
                }
            }
        }

        async fn remove_dir(&self, dir: &str) -> Result<(), AppError> {
            self.check_online()?;
            if self.faults.lock().unwrap().undeletable.contains(dir) {
                return Err(AppError::permission_denied(dir, FileSide::Remote));
            }
            let mut nodes = self.nodes();
            match nodes.get(dir) {
                None => Err(AppError::not_found(dir, FileSide::Remote)),
                Some(Node::Dir { .. }) if Self::children(&nodes, dir).is_empty() => {
                    nodes.remove(dir);
                    Ok(())
                }
                Some(_) => Err(failure()),
            }
        }

        async fn rename(&self, from: &str, to: &str) -> Result<(), AppError> {
            self.check_online()?;
            let is_part = path::split(from)
                .and_then(|(_, name)| path::residue_of(name))
                .is_some_and(|(kind, _)| kind == path::TempKind::Part);
            if is_part && self.faults.lock().unwrap().rename_part_fails {
                return Err(failure());
            }
            self.parent_is_dir(to)?;
            let mut nodes = self.nodes();
            if !nodes.contains_key(from) {
                return Err(AppError::not_found(from, FileSide::Remote));
            }
            if nodes.contains_key(to) {
                return Err(failure());
            }
            let moved: Vec<String> = nodes
                .keys()
                .filter(|p| path::is_within(p, from))
                .cloned()
                .collect();
            for old in moved {
                let node = nodes.remove(&old).unwrap();
                nodes.insert(format!("{to}{}", &old[from.len()..]), node);
            }
            Ok(())
        }

        async fn set_mode(&self, file: &str, mode: u32) -> Result<(), AppError> {
            self.check_online()?;
            match self.nodes().get_mut(file) {
                Some(Node::File { mode: m, .. }) => {
                    *m = 0o100_000 | (mode & 0o7777);
                    Ok(())
                }
                Some(_) => Ok(()),
                None => Err(AppError::not_found(file, FileSide::Remote)),
            }
        }

        async fn available_space(&self, _: &str) -> Result<Option<u64>, AppError> {
            self.check_online()?;
            Ok(self.faults.lock().unwrap().space)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn limited_tasks_never_exceed_the_limit() {
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let results = for_each_limited((0..50).collect(), 4, |n: u32| {
            let (running, peak) = (running.clone(), peak.clone());
            async move {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::task::yield_now().await;
                running.fetch_sub(1, Ordering::SeqCst);
                Ok(n * 2)
            }
        })
        .await
        .unwrap();
        let mut results = results;
        results.sort_unstable();
        assert_eq!(results, (0..50).map(|n| n * 2).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) <= 4);
    }

    #[tokio::test]
    async fn limited_tasks_stop_at_the_first_error() {
        let result = for_each_limited((0..10).collect(), 2, |n: u32| async move {
            if n == 3 {
                Err(AppError::io("échec"))
            } else {
                Ok(n)
            }
        })
        .await;
        assert!(matches!(result, Err(AppError::Io { .. })));
    }
}
