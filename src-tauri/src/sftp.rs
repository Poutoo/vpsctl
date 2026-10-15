//! Opérations sur les fichiers distants via le sous-système SFTP (russh-sftp) :
//! lister, créer, renommer, supprimer, envoyer, télécharger.
//! Aucune commande shell.
//!
//! Toutes les opérations passent par la session partagée de la liaison
//! (`Connection::sftp`). Une erreur de transport l'invalide pour qu'elle soit
//! recréée à l'opération suivante ; une erreur de statut (fichier absent,
//! accès refusé) est une réponse normale du serveur et la conserve.

use std::sync::Arc;

use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, StatusCode};

use crate::error::AppError;
use crate::ssh::Connection;

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

/// Exécute une opération sur la session partagée et classe son échec.
async fn run<T, F, Fut>(connection: &Connection, path: &str, operation: F) -> Result<T, Failure>
where
    F: FnOnce(Arc<SftpSession>) -> Fut,
    Fut: std::future::Future<Output = Result<T, SftpError>>,
{
    let session = connection.sftp().await.map_err(Failure::App)?;
    match operation(session.clone()).await {
        Ok(value) => Ok(value),
        Err(SftpError::Status(status)) => Err(match status.status_code {
            StatusCode::NoSuchFile => Failure::NotFound,
            StatusCode::PermissionDenied => {
                Failure::App(AppError::io(format!("accès refusé : {path}")))
            }
            _ => Failure::App(AppError::io(format!("{path} : {}", status.error_message))),
        }),
        Err(error) => {
            connection.discard_sftp(&session).await;
            Err(Failure::App(AppError::network(format!(
                "session SFTP interrompue : {error}"
            ))))
        }
    }
}
