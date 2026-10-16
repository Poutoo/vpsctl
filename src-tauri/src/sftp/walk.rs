//! Parcours d'une arborescence distante, sans jamais suivre un lien
//! symbolique (les attributs de `readdir` sont ceux de `lstat` : un lien
//! vers un dossier reste un lien). Sert au décompte affiché avant une
//! suppression (`summarize`) et à la suppression récursive (`delete_tree`).
//!
//! L'arborescence est entièrement lue avant toute suppression : un
//! sous-dossier illisible fait échouer l'opération sans rien supprimer.

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use super::fs::{for_each_limited, RemoteFs, PARALLEL_REQUESTS};
use super::{path, EntryKind};
use crate::error::AppError;

/// Le décompte s'arrête au-delà de ce nombre d'éléments.
pub const SUMMARY_LIMIT: u64 = 100_000;

/// Contenu d'un dossier, affiché dans le GO / NO GO d'une suppression.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DirSummary {
    /// Sous-dossiers (le dossier lui-même n'est pas compté).
    #[ts(type = "number")]
    pub dirs: u64,
    /// Fichiers, sockets, fifos, périphériques.
    #[ts(type = "number")]
    pub files: u64,
    /// Liens symboliques (supprimés, jamais suivis).
    #[ts(type = "number")]
    pub links: u64,
    /// Taille cumulée des fichiers.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Parcours arrêté à `SUMMARY_LIMIT` éléments.
    pub truncated: bool,
}

/// Élément qui n'est pas un dossier.
struct Leaf {
    path: String,
    kind: EntryKind,
    size: u64,
}

/// Arborescence lue : dossiers par profondeur (le dossier de départ seul au
/// niveau 0) et éléments qui ne sont pas des dossiers.
struct Tree {
    levels: Vec<Vec<String>>,
    leaves: Vec<Leaf>,
    truncated: bool,
}

/// Lit l'arborescence de `root` niveau par niveau, les dossiers d'un même
/// niveau en parallèle. Avec `limit`, s'arrête dès que ce nombre
/// d'éléments est atteint (les dossiers trouvés sont comptés, pas lus).
async fn scan<F: RemoteFs>(fs: &Arc<F>, root: &str, limit: Option<u64>) -> Result<Tree, AppError> {
    let mut levels = vec![vec![root.to_owned()]];
    let mut leaves = Vec::new();
    let mut count = 0_u64;
    loop {
        let current = levels.last().cloned().unwrap_or_default();
        let listed = for_each_limited(current, PARALLEL_REQUESTS, |dir| {
            let fs = fs.clone();
            async move {
                let entries = fs.read_dir(&dir).await?;
                Ok((dir, entries))
            }
        })
        .await?;
        let mut next = Vec::new();
        for (dir, entries) in listed {
            for (name, meta) in entries {
                count += 1;
                let child = path::join(&dir, &name);
                if meta.kind == EntryKind::Dir {
                    next.push(child);
                } else {
                    leaves.push(Leaf {
                        path: child,
                        kind: meta.kind,
                        size: meta.size,
                    });
                }
            }
        }
        if next.is_empty() {
            return Ok(Tree {
                levels,
                leaves,
                truncated: false,
            });
        }
        levels.push(next);
        if limit.is_some_and(|limit| count >= limit) {
            return Ok(Tree {
                levels,
                leaves,
                truncated: true,
            });
        }
    }
}

/// Décompte du contenu de `dir` (un dossier), `limit` éléments au plus.
pub async fn summarize<F: RemoteFs>(
    fs: &Arc<F>,
    dir: &str,
    limit: u64,
) -> Result<DirSummary, AppError> {
    let tree = scan(fs, dir, Some(limit)).await?;
    let mut summary = DirSummary {
        dirs: tree.levels.iter().skip(1).map(|l| l.len() as u64).sum(),
        truncated: tree.truncated,
        ..DirSummary::default()
    };
    for leaf in &tree.leaves {
        match leaf.kind {
            EntryKind::Symlink => summary.links += 1,
            EntryKind::File => {
                summary.files += 1;
                summary.bytes = summary.bytes.saturating_add(leaf.size);
            }
            _ => summary.files += 1,
        }
    }
    Ok(summary)
}

/// Supprime `dir` et tout son contenu. Les liens sont supprimés comme des
/// fichiers (le lien seul) : aucune cible n'est jamais atteinte. Une erreur
/// arrête la suppression ; ce qui est déjà supprimé l'est définitivement.
pub async fn delete_tree<F: RemoteFs>(fs: &Arc<F>, dir: &str) -> Result<(), AppError> {
    let tree = scan(fs, dir, None).await?;
    let leaves = tree.leaves.into_iter().map(|leaf| leaf.path).collect();
    for_each_limited(leaves, PARALLEL_REQUESTS, |leaf| {
        let fs = fs.clone();
        async move { fs.remove_file(&leaf).await }
    })
    .await?;
    // Du plus profond au moins profond : un dossier est vide quand vient son tour.
    for level in tree.levels.into_iter().rev() {
        for_each_limited(level, PARALLEL_REQUESTS, |dir| {
            let fs = fs.clone();
            async move {
                fs.remove_dir(&dir).await.map_err(|error| match error {
                    AppError::Io { .. } => AppError::io(format!(
                        "{dir} : le dossier n'a pas pu être supprimé (un élément y a peut-être été ajouté entre-temps)"
                    )),
                    other => other,
                })
            }
        })
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::fs::mem::MemFs;

    /// Dossier de test : sous-dossiers, fichiers cachés, liens vers
    /// l'extérieur (dossier et fichier) et lien cassé.
    fn sample() -> Arc<MemFs> {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/srv/garder/precieux.txt", b"ne pas toucher");
        fs.add_file("/srv/seul.txt", b"cible");
        let root = "/home/deploy/essai";
        fs.add_file(&format!("{root}/a/b/f.txt"), b"12345");
        fs.add_file(&format!("{root}/a/.cache/x"), b"xx");
        fs.add_file(&format!("{root}/.cache"), b"abc");
        fs.mkdir_all(&format!("{root}/vide"));
        fs.add_link(&format!("{root}/vers-dossier"), "/srv/garder");
        fs.add_link(&format!("{root}/a/vers-fichier"), "/srv/seul.txt");
        fs.add_link(&format!("{root}/casse"), "/nulle/part");
        fs
    }

    #[tokio::test]
    async fn summary_counts_without_following_links() {
        let fs = sample();
        let summary = summarize(&fs, "/home/deploy/essai", SUMMARY_LIMIT)
            .await
            .unwrap();
        assert_eq!(
            summary,
            DirSummary {
                dirs: 4, // a, a/b, a/.cache, vide
                files: 3,
                links: 3,
                bytes: 10,
                truncated: false,
            }
        );
    }

    #[tokio::test]
    async fn summary_stops_at_the_limit() {
        let fs = MemFs::new("/home/deploy");
        for n in 0..10 {
            fs.add_file(&format!("/home/deploy/gros/d{n}/f"), b"1");
        }
        let summary = summarize(&fs, "/home/deploy/gros", 5).await.unwrap();
        assert!(summary.truncated);
        assert_eq!(summary.dirs, 10);
        assert_eq!(summary.files, 0);
    }

    #[tokio::test]
    async fn deletion_removes_the_tree_but_never_a_link_target() {
        let fs = sample();
        delete_tree(&fs, "/home/deploy/essai").await.unwrap();
        assert!(fs
            .paths()
            .iter()
            .all(|p| !path::is_within(p, "/home/deploy/essai")));
        assert_eq!(
            fs.content("/srv/garder/precieux.txt").unwrap(),
            b"ne pas toucher"
        );
        assert_eq!(fs.content("/srv/seul.txt").unwrap(), b"cible");
        assert!(fs.exists("/home/deploy"));
    }

    #[tokio::test]
    async fn an_unreadable_subdirectory_stops_everything_before_deleting() {
        let fs = sample();
        fs.faults
            .lock()
            .unwrap()
            .unreadable
            .insert("/home/deploy/essai/a/b".into());
        let before = fs.paths();
        let result = delete_tree(&fs, "/home/deploy/essai").await;
        assert!(matches!(result, Err(AppError::PermissionDenied { .. })));
        assert_eq!(fs.paths(), before);
    }

    #[tokio::test]
    async fn a_deletion_error_is_reported() {
        let fs = sample();
        fs.faults
            .lock()
            .unwrap()
            .undeletable
            .insert("/home/deploy/essai/a/b/f.txt".into());
        let result = delete_tree(&fs, "/home/deploy/essai").await;
        assert!(matches!(result, Err(AppError::PermissionDenied { .. })));
        assert!(fs.exists("/home/deploy/essai/a/b/f.txt"));
        assert!(fs.exists("/srv/garder/precieux.txt"));
    }
}
