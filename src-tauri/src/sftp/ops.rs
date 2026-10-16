//! Opérations de l'explorateur : lister un dossier, dossier personnel,
//! créer, renommer. Toutes passent par `RemoteFs`, jamais par une commande
//! shell. Un nom ou un chemin invalide est refusé avant toute requête.

use std::collections::HashSet;
use std::sync::Arc;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use super::fs::{for_each_limited, Meta, RemoteFs, PARALLEL_REQUESTS};
use super::path::{self, TempKind};
use super::EntryKind;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileEntry {
    pub name: String,
    /// Dossier listé + nom.
    pub path: String,
    pub kind: EntryKind,
    /// Octets (`lstat` : pour un lien, la longueur de sa cible).
    #[ts(type = "number")]
    pub size: u64,
    /// Millisecondes depuis l'epoch Unix.
    #[ts(type = "number | null")]
    pub modified: Option<u64>,
    /// Au format de `ls -l` (`drwxr-xr-x`) ; vide si inconnu.
    pub permissions: String,
    /// Lien symbolique uniquement.
    pub link: Option<LinkInfo>,
    /// Temporaire laissé par un transfert interrompu.
    pub residue: Option<Residue>,
    /// Supprimer, renommer ou écraser cet élément peut couper l'accès SSH.
    pub ssh_access: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkInfo {
    /// Cible telle qu'enregistrée (non résolue).
    pub target: String,
    pub target_kind: TargetKind,
}

/// Genre de la cible d'un lien (lien suivi). `Broken` : cible absente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TargetKind {
    File,
    Dir,
    Other,
    Broken,
}

/// Résidu de transfert : `.<nom>.atlas-<jeton>.part` (envoi ou téléchargement
/// interrompu) ou `.old` (fichier d'origine mis de côté pendant un
/// écrasement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Residue {
    pub kind: ResidueKind,
    pub original: String,
    /// Un élément `original` existe dans le même dossier.
    pub original_exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ResidueKind {
    Part,
    Old,
}

impl From<TempKind> for ResidueKind {
    fn from(kind: TempKind) -> Self {
        match kind {
            TempKind::Part => Self::Part,
            TempKind::Old => Self::Old,
        }
    }
}

/// Dossier personnel : chemin absolu résolu.
pub async fn home_dir(fs: &impl RemoteFs) -> Result<String, AppError> {
    fs.real_path(".").await
}

/// Contenu d'un dossier, dans l'ordre du serveur (le tri est fait par
/// l'interface). La cible de chaque lien est lue en parallèle.
pub async fn list_dir<F: RemoteFs>(fs: &Arc<F>, dir: &str) -> Result<Vec<FileEntry>, AppError> {
    path::validate_path(dir)?;
    let (entries, resolved, home) =
        tokio::join!(fs.read_dir(dir), fs.real_path(dir), fs.real_path("."));
    let entries = entries?;
    let (resolved, home) = (resolved?, home?);

    let links: Vec<(usize, String)> = entries
        .iter()
        .enumerate()
        .filter(|(_, (_, meta))| meta.kind == EntryKind::Symlink)
        .map(|(index, (name, _))| (index, path::join(dir, name)))
        .collect();
    let infos = for_each_limited(links, PARALLEL_REQUESTS, |(index, link)| {
        let fs = fs.clone();
        async move { Ok((index, link_info(&*fs, &link).await)) }
    })
    .await?;

    let mut entries: Vec<(String, Meta, Option<LinkInfo>)> = entries
        .into_iter()
        .map(|(name, meta)| (name, meta, None))
        .collect();
    for (index, info) in infos {
        entries[index].2 = info;
    }
    Ok(build_entries(dir, &resolved, &home, entries))
}

/// Cible d'un lien et son genre. `None` si le lien n'a pas pu être lu.
async fn link_info(fs: &impl RemoteFs, link: &str) -> Option<LinkInfo> {
    let target = fs.read_link(link).await.ok()?;
    let target_kind = match fs.stat(link).await {
        Ok(Some(meta)) => match meta.kind {
            EntryKind::Dir => TargetKind::Dir,
            EntryKind::File => TargetKind::File,
            _ => TargetKind::Other,
        },
        Ok(None) => TargetKind::Broken,
        // Cible inaccessible (dossier parent illisible…).
        Err(_) => TargetKind::Other,
    };
    Some(LinkInfo {
        target,
        target_kind,
    })
}

/// Assemble les entrées d'un dossier : chemin, droits, résidus de transfert
/// (comparés aux autres noms du dossier) et accès SSH (d'après le dossier
/// résolu, pour reconnaître un dossier atteint par un lien).
pub fn build_entries(
    dir: &str,
    resolved_dir: &str,
    home: &str,
    entries: Vec<(String, Meta, Option<LinkInfo>)>,
) -> Vec<FileEntry> {
    let names: HashSet<&str> = entries.iter().map(|(name, _, _)| name.as_str()).collect();
    let residues: Vec<Option<Residue>> = entries
        .iter()
        .map(|(name, meta, _)| {
            let (kind, original) = path::residue_of(name)?;
            (meta.kind == EntryKind::File).then(|| Residue {
                kind: kind.into(),
                original: original.to_owned(),
                original_exists: names.contains(original),
            })
        })
        .collect();
    entries
        .into_iter()
        .zip(residues)
        .map(|((name, meta, link), residue)| FileEntry {
            path: path::join(dir, &name),
            ssh_access: path::ssh_access(home, &path::join(resolved_dir, &name)),
            kind: meta.kind,
            size: meta.size,
            modified: meta.modified.map(|s| s.saturating_mul(1000)),
            permissions: path::permissions_string(meta.mode),
            link: if meta.kind == EntryKind::Symlink {
                link
            } else {
                None
            },
            residue,
            name,
        })
        .collect()
}

/// Crée un fichier vide. Ne remplace jamais un élément existant.
pub async fn create_file(fs: &impl RemoteFs, file: &str) -> Result<(), AppError> {
    path::validate_item(file)?;
    let mut writer = match fs.create_new(file).await {
        Ok(writer) => writer,
        Err(error) => return Err(explain_failure(fs, file, error).await),
    };
    writer
        .shutdown()
        .await
        .map_err(|e| AppError::io(format!("{file} : {e}")))
}

/// Crée un dossier. Ne remplace jamais un élément existant.
pub async fn create_dir(fs: &impl RemoteFs, dir: &str) -> Result<(), AppError> {
    path::validate_item(dir)?;
    match fs.create_dir(dir).await {
        Ok(()) => Ok(()),
        Err(error) => Err(explain_failure(fs, dir, error).await),
    }
}

/// Renomme (ou déplace) un élément. La destination ne doit pas exister.
pub async fn rename(fs: &impl RemoteFs, from: &str, to: &str) -> Result<(), AppError> {
    path::validate_item(from)?;
    path::validate_item(to)?;
    let Some(meta) = fs.lstat(from).await? else {
        return Err(AppError::not_found(from, crate::error::FileSide::Remote));
    };
    refuse_home(fs, from, &meta, "renommé").await?;
    if let Some(existing) = fs.lstat(to).await? {
        return Err(AppError::already_exists(
            to,
            existing.kind == EntryKind::Dir,
        ));
    }
    match fs.rename(from, to).await {
        Ok(()) => Ok(()),
        Err(error) => Err(explain_failure(fs, to, error).await),
    }
}

/// Refuse d'agir sur le dossier personnel lui-même (y compris atteint par un
/// chemin qui le résout). Un lien vers lui reste un lien ordinaire.
pub(super) async fn refuse_home(
    fs: &impl RemoteFs,
    target: &str,
    meta: &Meta,
    action: &str,
) -> Result<(), AppError> {
    let home = fs.real_path(".").await?;
    let is_home = target == home
        || (meta.kind == EntryKind::Dir
            && fs.real_path(target).await.ok().as_deref() == Some(&home));
    if is_home {
        return Err(AppError::invalid_input(format!(
            "le dossier personnel ne peut pas être {action}"
        )));
    }
    Ok(())
}

/// Précise un échec générique du serveur : si l'élément existe désormais,
/// c'est qu'il existait déjà.
pub(super) async fn explain_failure(fs: &impl RemoteFs, target: &str, error: AppError) -> AppError {
    if matches!(error, AppError::Io { .. }) {
        if let Ok(Some(meta)) = fs.lstat(target).await {
            return AppError::already_exists(target, meta.kind == EntryKind::Dir);
        }
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FileSide;
    use crate::sftp::fs::mem::MemFs;

    fn meta(kind: EntryKind, mode: u32) -> Meta {
        Meta {
            kind,
            size: 12,
            mode: Some(mode),
            modified: Some(1_760_000_000),
        }
    }

    #[test]
    fn entries_carry_path_permissions_and_dates() {
        let entries = build_entries(
            "/home/deploy",
            "/home/deploy",
            "/home/deploy",
            vec![
                ("app".into(), meta(EntryKind::Dir, 0o040_755), None),
                (".env".into(), meta(EntryKind::File, 0o100_600), None),
            ],
        );
        assert_eq!(entries[0].path, "/home/deploy/app");
        assert_eq!(entries[0].permissions, "drwxr-xr-x");
        assert_eq!(entries[1].path, "/home/deploy/.env");
        assert_eq!(entries[1].permissions, "-rw-------");
        assert_eq!(entries[1].modified, Some(1_760_000_000_000));
        assert!(entries.iter().all(|e| !e.ssh_access && e.residue.is_none()));
    }

    #[test]
    fn residues_are_flagged_with_their_original() {
        let entries = build_entries(
            "/srv",
            "/srv",
            "/home/deploy",
            vec![
                ("db.sql".into(), meta(EntryKind::File, 0o100_644), None),
                (
                    ".db.sql.atlas-0123abcd.part".into(),
                    meta(EntryKind::File, 0o100_644),
                    None,
                ),
                (
                    ".site.zip.atlas-89abcdef.old".into(),
                    meta(EntryKind::File, 0o100_644),
                    None,
                ),
                // Un dossier qui porte ce nom n'est pas un résidu.
                (
                    ".x.atlas-00000000.part".into(),
                    meta(EntryKind::Dir, 0o040_755),
                    None,
                ),
            ],
        );
        assert_eq!(entries[0].residue, None);
        assert_eq!(
            entries[1].residue,
            Some(Residue {
                kind: ResidueKind::Part,
                original: "db.sql".into(),
                original_exists: true,
            })
        );
        assert_eq!(
            entries[2].residue,
            Some(Residue {
                kind: ResidueKind::Old,
                original: "site.zip".into(),
                original_exists: false,
            })
        );
        assert_eq!(entries[3].residue, None);
    }

    #[test]
    fn ssh_access_uses_the_resolved_directory() {
        // /srv/moi est un lien vers le dossier personnel.
        let entries = build_entries(
            "/srv/moi",
            "/home/deploy",
            "/home/deploy",
            vec![(".ssh".into(), meta(EntryKind::Dir, 0o040_700), None)],
        );
        assert_eq!(entries[0].path, "/srv/moi/.ssh");
        assert!(entries[0].ssh_access);
    }

    #[tokio::test]
    async fn listing_reads_link_targets() {
        let fs = MemFs::new("/home/deploy");
        fs.mkdir_all("/srv/data");
        fs.add_file("/home/deploy/notes.txt", b"abc");
        fs.add_link("/home/deploy/data", "/srv/data");
        fs.add_link("/home/deploy/notes", "notes.txt");
        fs.add_link("/home/deploy/perdu", "/nulle/part");
        fs.mkdir_all("/home/deploy/.ssh");
        fs.add_file("/home/deploy/.ssh/authorized_keys", b"ssh-ed25519 AAAA");

        let mut entries = list_dir(&fs, "/home/deploy").await.unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let by_name = |name: &str| entries.iter().find(|e| e.name == name).unwrap();

        let data = by_name("data");
        assert_eq!(data.kind, EntryKind::Symlink);
        assert_eq!(data.permissions, "lrwxrwxrwx");
        assert_eq!(
            data.link,
            Some(LinkInfo {
                target: "/srv/data".into(),
                target_kind: TargetKind::Dir,
            })
        );
        assert_eq!(
            by_name("notes").link.as_ref().unwrap().target_kind,
            TargetKind::File
        );
        assert_eq!(
            by_name("perdu").link.as_ref().unwrap().target_kind,
            TargetKind::Broken
        );
        assert_eq!(by_name("notes.txt").link, None);
        assert!(by_name(".ssh").ssh_access);

        let keys = list_dir(&fs, "/home/deploy/.ssh").await.unwrap();
        assert!(keys[0].ssh_access);
    }

    #[tokio::test]
    async fn listing_errors_are_distinct() {
        let fs = MemFs::new("/home/deploy");
        fs.mkdir_all("/root");
        fs.faults.lock().unwrap().unreadable.insert("/root".into());
        assert!(matches!(
            list_dir(&fs, "/root").await,
            Err(AppError::PermissionDenied {
                side: FileSide::Remote,
                ..
            })
        ));
        assert!(matches!(
            list_dir(&fs, "/absent").await,
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            list_dir(&fs, "/home/../root").await,
            Err(AppError::InvalidInput { .. })
        ));
    }

    #[tokio::test]
    async fn creation_never_replaces_an_existing_item() {
        let fs = MemFs::new("/home/deploy");
        create_file(&*fs, "/home/deploy/vide.txt").await.unwrap();
        assert_eq!(fs.content("/home/deploy/vide.txt"), Some(Vec::new()));
        create_dir(&*fs, "/home/deploy/neuf").await.unwrap();

        fs.add_file("/home/deploy/plein.txt", b"contenu");
        assert!(matches!(
            create_file(&*fs, "/home/deploy/plein.txt").await,
            Err(AppError::AlreadyExists { is_dir: false, .. })
        ));
        assert_eq!(fs.content("/home/deploy/plein.txt").unwrap(), b"contenu");
        assert!(matches!(
            create_dir(&*fs, "/home/deploy/neuf").await,
            Err(AppError::AlreadyExists { is_dir: true, .. })
        ));
        assert!(matches!(
            create_file(&*fs, "/home/deploy/absent/x").await,
            Err(AppError::NotFound { .. })
        ));
        for invalid in ["/home/deploy/..", "/home/deploy/a\0b", "/"] {
            assert!(matches!(
                create_dir(&*fs, invalid).await,
                Err(AppError::InvalidInput { .. })
            ));
        }
    }

    #[tokio::test]
    async fn rename_refuses_existing_destinations_and_the_home() {
        let fs = MemFs::new("/home/deploy");
        fs.add_file("/home/deploy/a.txt", b"a");
        fs.add_file("/home/deploy/b.txt", b"b");

        assert!(matches!(
            rename(&*fs, "/home/deploy/a.txt", "/home/deploy/b.txt").await,
            Err(AppError::AlreadyExists { .. })
        ));
        assert_eq!(fs.content("/home/deploy/b.txt").unwrap(), b"b");

        rename(&*fs, "/home/deploy/a.txt", "/home/deploy/c.txt")
            .await
            .unwrap();
        assert!(!fs.exists("/home/deploy/a.txt"));
        assert_eq!(fs.content("/home/deploy/c.txt").unwrap(), b"a");

        assert!(matches!(
            rename(&*fs, "/home/deploy/absent", "/home/deploy/x").await,
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            rename(&*fs, "/home/deploy", "/home/autre").await,
            Err(AppError::InvalidInput { .. })
        ));
        // Un lien vers le dossier personnel se renomme normalement.
        fs.add_link("/srv/moi", "/home/deploy");
        rename(&*fs, "/srv/moi", "/srv/toi").await.unwrap();
    }
}
