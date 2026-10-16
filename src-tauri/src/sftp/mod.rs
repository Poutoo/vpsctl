//! Opérations sur les fichiers distants via le sous-système SFTP (russh-sftp) :
//! lister, créer, renommer, supprimer, envoyer, télécharger.
//! Aucune commande shell.
//!
//! Toutes les opérations passent par la session partagée de la liaison
//! (`Connection::sftp`). Une erreur de transport l'invalide : elle est
//! recréée et l'opération retentée une seule fois. Une erreur de statut
//! (fichier absent, accès refusé) est une réponse normale du serveur : la
//! session est conservée et l'opération n'est pas retentée.

use std::sync::Arc;

use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, StatusCode};

use crate::error::{AppError, FileSide};
use crate::ssh::Connection;

pub mod path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

impl From<FileType> for EntryKind {
    fn from(file_type: FileType) -> Self {
        match file_type {
            FileType::File => Self::File,
            FileType::Dir => Self::Dir,
            FileType::Symlink => Self::Symlink,
            FileType::Other => Self::Other,
        }
    }
}

/// Entrée d'un dossier distant (un lien symbolique n'est pas suivi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
}

/// Résultat d'une opération SFTP, avant traduction en `AppError`.
enum Failure {
    NotFound,
    App(AppError),
}

/// Lit le contenu d'un dossier (sans `.` ni `..`). `Ok(None)` s'il n'existe
/// pas. Un chemin relatif part du dossier personnel.
pub async fn read_dir(
    connection: &Connection,
    path: &str,
) -> Result<Option<Vec<DirEntry>>, AppError> {
    let result = run(connection, path, |session| {
        let path = path.to_owned();
        async move { session.read_dir(path).await }
    })
    .await;
    match result {
        Ok(entries) => Ok(Some(
            entries
                .filter(|e| e.file_name() != "." && e.file_name() != "..")
                .map(|e| DirEntry {
                    name: e.file_name(),
                    kind: e.file_type().into(),
                })
                .collect(),
        )),
        Err(Failure::NotFound) => Ok(None),
        Err(Failure::App(error)) => Err(error),
    }
}

/// Cible d'un lien symbolique, telle qu'enregistrée (non résolue).
/// `Ok(None)` si le lien n'existe pas.
pub async fn read_link(connection: &Connection, path: &str) -> Result<Option<String>, AppError> {
    let result = run(connection, path, |session| {
        let path = path.to_owned();
        async move { session.read_link(path).await }
    })
    .await;
    match result {
        Ok(target) => Ok(Some(target)),
        Err(Failure::NotFound) => Ok(None),
        Err(Failure::App(error)) => Err(error),
    }
}

/// Source de la session partagée. Abstraite pour tester la reprise sans réseau.
trait SessionSource<S> {
    async fn session(&self) -> Result<Arc<S>, AppError>;
    /// Oublie la session après une erreur de transport.
    async fn discard(&self, session: &Arc<S>);
}

impl SessionSource<SftpSession> for Connection {
    async fn session(&self) -> Result<Arc<SftpSession>, AppError> {
        self.sftp().await
    }

    async fn discard(&self, session: &Arc<SftpSession>) {
        self.discard_sftp(session).await
    }
}

/// Exécute une opération sur la session partagée et classe son échec. Après
/// une erreur de transport, la session est recréée et l'opération retentée
/// une seule fois.
async fn run<S, T, F, Fut>(
    source: &impl SessionSource<S>,
    path: &str,
    operation: F,
) -> Result<T, Failure>
where
    F: Fn(Arc<S>) -> Fut,
    Fut: std::future::Future<Output = Result<T, SftpError>>,
{
    let mut retried = false;
    loop {
        let session = source.session().await.map_err(Failure::App)?;
        match operation(session.clone()).await {
            Ok(value) => return Ok(value),
            Err(SftpError::Status(status)) => {
                return Err(match status.status_code {
                    StatusCode::NoSuchFile => Failure::NotFound,
                    StatusCode::PermissionDenied => {
                        Failure::App(AppError::permission_denied(path, FileSide::Remote))
                    }
                    _ => Failure::App(AppError::io(format!("{path} : {}", status.error_message))),
                })
            }
            Err(error) => {
                source.discard(&session).await;
                if retried {
                    return Err(Failure::App(AppError::network(format!(
                        "session SFTP interrompue : {error}"
                    ))));
                }
                retried = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh_sftp::protocol::Status;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Source simulée : comme `Connection`, garde la session jusqu'à ce
    /// qu'elle soit oubliée. Chaque session est numérotée à partir de 1.
    #[derive(Default)]
    struct FakeSource {
        current: Mutex<Option<Arc<usize>>>,
        opened: AtomicUsize,
        discarded: AtomicUsize,
    }

    impl SessionSource<usize> for FakeSource {
        async fn session(&self) -> Result<Arc<usize>, AppError> {
            let mut current = self.current.lock().unwrap();
            let session = current
                .get_or_insert_with(|| Arc::new(self.opened.fetch_add(1, Ordering::SeqCst) + 1));
            Ok(session.clone())
        }

        async fn discard(&self, session: &Arc<usize>) {
            let mut current = self.current.lock().unwrap();
            if current.as_ref().is_some_and(|s| Arc::ptr_eq(s, session)) {
                *current = None;
                self.discarded.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    fn transport_error() -> SftpError {
        SftpError::IO("channel closed".into())
    }

    fn status_error(status_code: StatusCode) -> SftpError {
        SftpError::Status(Status {
            id: 1,
            status_code,
            error_message: "message du serveur".into(),
            language_tag: "en".into(),
        })
    }

    #[tokio::test]
    async fn success_uses_the_shared_session() {
        let source = FakeSource::default();
        for _ in 0..3 {
            let value = run(&source, "x", |s| async move { Ok::<_, SftpError>(*s) }).await;
            assert!(matches!(value, Ok(1)));
        }
        assert_eq!(source.opened.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn transport_error_recreates_the_session_and_retries_once() {
        let source = FakeSource::default();
        let result = run(&source, "x", |s| async move {
            if *s == 1 {
                Err(transport_error())
            } else {
                Ok(*s)
            }
        })
        .await;
        assert!(matches!(result, Ok(2)));
        assert_eq!(source.opened.load(Ordering::SeqCst), 2);
        assert_eq!(source.discarded.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn second_transport_error_is_reported() {
        let source = FakeSource::default();
        let attempts = AtomicUsize::new(0);
        let result = run(&source, "x", |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(transport_error()) }
        })
        .await;
        assert!(matches!(
            result,
            Err(Failure::App(AppError::Network { .. }))
        ));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        // La session fautive est oubliée : la prochaine opération en ouvrira une neuve.
        assert_eq!(source.discarded.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn status_errors_keep_the_session_and_are_not_retried() {
        let source = FakeSource::default();
        let attempts = AtomicUsize::new(0);
        let missing = run(&source, "absent", |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(status_error(StatusCode::NoSuchFile)) }
        })
        .await;
        assert!(matches!(missing, Err(Failure::NotFound)));

        let denied = run(&source, "secret", |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(status_error(StatusCode::PermissionDenied)) }
        })
        .await;
        match denied {
            Err(Failure::App(AppError::PermissionDenied { path, side })) => {
                assert_eq!(path, "secret");
                assert_eq!(side, FileSide::Remote);
            }
            _ => panic!("attendu PermissionDenied"),
        }

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(source.opened.load(Ordering::SeqCst), 1);
        assert_eq!(source.discarded.load(Ordering::SeqCst), 0);
    }
}
