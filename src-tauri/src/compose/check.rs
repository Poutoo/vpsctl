//! Dossier et fichiers d'un projet compose, vérifiés par SFTP (aucune
//! commande shell) : avant chaque action, et périodiquement par l'interface.

use serde::Serialize;
use ts_rs::TS;

use super::labels::ProjectFiles;
use crate::error::AppError;
use crate::sftp::fs::RemoteFs;
use crate::sftp::EntryKind;

/// Résultat de la vérification, premier problème trouvé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum FilesStatus {
    Ok,
    /// Dossier absent (ou lien cassé).
    DirMissing,
    /// Le chemin du dossier désigne autre chose qu'un dossier.
    NotDir,
    Missing {
        path: String,
    },
    /// Autre chose qu'un fichier ordinaire (dossier, socket…).
    NotFile {
        path: String,
    },
    /// Présent mais illisible pour l'utilisateur connecté (ou dossier
    /// parent non traversable).
    Unreadable {
        path: String,
    },
    /// Le serveur a répondu par une erreur sans cause précise.
    Unchecked {
        message: String,
    },
}

impl FilesStatus {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// Vérification d'un projet, renvoyée par `compose_check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectCheck {
    pub project: String,
    pub status: FilesStatus,
}

/// Une erreur de transport arrête la vérification ; une autre erreur du
/// serveur devient `Unchecked`.
fn classify(error: AppError, path: &str) -> Result<FilesStatus, AppError> {
    match error {
        AppError::PermissionDenied { .. } => Ok(FilesStatus::Unreadable {
            path: path.to_owned(),
        }),
        AppError::NotFound { .. } => Ok(FilesStatus::Missing {
            path: path.to_owned(),
        }),
        AppError::Io { message } | AppError::InvalidInput { message } => {
            Ok(FilesStatus::Unchecked { message })
        }
        other => Err(other),
    }
}

/// Le dossier doit être un dossier ; chaque fichier de configuration et
/// d'environnement un fichier ordinaire (liens suivis) qui s'ouvre en
/// lecture. Le fichier ouvert est refermé aussitôt, sans rien lire.
pub async fn check_files<F: RemoteFs>(
    fs: &F,
    files: &ProjectFiles,
) -> Result<FilesStatus, AppError> {
    let dir = &files.working_dir;
    match fs.stat(dir).await {
        Ok(None) => return Ok(FilesStatus::DirMissing),
        Ok(Some(meta)) if meta.kind != EntryKind::Dir => return Ok(FilesStatus::NotDir),
        Ok(Some(_)) => {}
        Err(AppError::NotFound { .. }) => return Ok(FilesStatus::DirMissing),
        Err(error) => return classify(error, dir),
    }
    for path in files.config_files.iter().chain(&files.env_files) {
        match fs.stat(path).await {
            Ok(None) => return Ok(FilesStatus::Missing { path: path.clone() }),
            Ok(Some(meta)) if meta.kind != EntryKind::File => {
                return Ok(FilesStatus::NotFile { path: path.clone() })
            }
            Ok(Some(_)) => {}
            Err(error) => return classify(error, path),
        }
        if let Err(error) = fs.open_read(path).await {
            return classify(error, path);
        }
    }
    Ok(FilesStatus::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::fs::mem::MemFs;

    fn files(config: &[&str], env: &[&str]) -> ProjectFiles {
        ProjectFiles {
            working_dir: "/srv/app".into(),
            config_files: config.iter().map(|s| (*s).to_owned()).collect(),
            env_files: env.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn run(fs: &MemFs, files: &ProjectFiles) -> Result<FilesStatus, AppError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(check_files(fs, files))
    }

    fn check(fs: &MemFs, files: &ProjectFiles) -> Option<FilesStatus> {
        run(fs, files).ok()
    }

    fn server() -> std::sync::Arc<MemFs> {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/srv/app/compose.yaml", b"services: {}");
        fs.add_file("/srv/app/compose.prod.yaml", b"services: {}");
        fs.add_file("/srv/app/.env.prod", b"A=1");
        fs
    }

    #[test]
    fn present_and_readable_files_are_ok() {
        let fs = server();
        let all = files(
            &["/srv/app/compose.yaml", "/srv/app/compose.prod.yaml"],
            &["/srv/app/.env.prod"],
        );
        assert_eq!(check(&fs, &all), Some(FilesStatus::Ok));
    }

    #[test]
    fn a_followed_link_counts_as_its_target() {
        let fs = server();
        fs.add_link("/srv/app/docker-compose.yml", "/srv/app/compose.yaml");
        assert_eq!(
            check(&fs, &files(&["/srv/app/docker-compose.yml"], &[])),
            Some(FilesStatus::Ok)
        );
    }

    #[test]
    fn missing_directory_comes_first() {
        let fs = MemFs::new("/home/deploy");
        assert_eq!(
            check(&fs, &files(&["/srv/app/compose.yaml"], &[])),
            Some(FilesStatus::DirMissing)
        );
        fs.add_file("/srv/app", b"");
        assert_eq!(
            check(&fs, &files(&["/srv/app/compose.yaml"], &[])),
            Some(FilesStatus::NotDir)
        );
    }

    #[test]
    fn each_file_is_checked_in_order() {
        let fs = server();
        assert_eq!(
            check(
                &fs,
                &files(&["/srv/app/compose.yaml", "/srv/app/absent.yaml"], &[])
            ),
            Some(FilesStatus::Missing {
                path: "/srv/app/absent.yaml".into()
            })
        );
        assert_eq!(
            check(&fs, &files(&["/srv/app/compose.yaml"], &["/srv/app/.env"])),
            Some(FilesStatus::Missing {
                path: "/srv/app/.env".into()
            })
        );
        fs.mkdir_all("/srv/app/conf.d");
        assert_eq!(
            check(&fs, &files(&["/srv/app/conf.d"], &[])),
            Some(FilesStatus::NotFile {
                path: "/srv/app/conf.d".into()
            })
        );
    }

    #[test]
    fn unreadable_file_is_reported() {
        let fs = server();
        fs.faults
            .lock()
            .unwrap()
            .unreadable
            .insert("/srv/app/.env.prod".into());
        assert_eq!(
            check(
                &fs,
                &files(&["/srv/app/compose.yaml"], &["/srv/app/.env.prod"])
            ),
            Some(FilesStatus::Unreadable {
                path: "/srv/app/.env.prod".into()
            })
        );
    }

    #[test]
    fn a_lost_link_stops_the_check() {
        let fs = server();
        fs.faults.lock().unwrap().offline = true;
        assert!(matches!(
            run(&fs, &files(&["/srv/app/compose.yaml"], &[])),
            Err(AppError::Network { .. })
        ));
    }
}
