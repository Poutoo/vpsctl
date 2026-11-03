//! Édition de fichiers texte distants, entièrement par SFTP (aucune
//! commande shell).
//!
//! - `text` : refus, BOM, fins de ligne, encodage, empreinte ;
//! - `access` : compte connecté, propriétaires, raisons de la lecture seule ;
//! - `authorized_keys` : vérifications avant d'enregistrer ce fichier.
//!
//! Enregistrement (`save`) : relecture du fichier et comparaison de son
//! empreinte SHA-256 avec celle de l'ouverture (différente : conflit, rien
//! n'est écrit), écriture dans un temporaire du même dossier, droits
//! appliqués, remplacement atomique (`posix-rename`), puis relecture pour
//! vérifier le résultat. Limite : sans verrou, une écriture faite sur le
//! serveur entre la relecture et le remplacement est perdue.

use std::collections::HashMap;

use russh::keys::PublicKey;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use ts_rs::TS;

use crate::error::{AppError, FileSide};
use crate::sftp::fs::{Meta, RemoteFs};
use crate::sftp::path::{self, TempKind};
use crate::sftp::transfer::new_token;
use crate::sftp::EntryKind;
use crate::ssh::link::now_millis;

pub mod access;
pub mod authorized_keys;
pub mod text;

use access::{Account, Owned, ReadOnlyReason, Users};
use authorized_keys::AuthorizedKeysCheck;
use text::{LineEnding, Refusal, MAX_SIZE};

/// État du fichier à un instant : ce que l'enregistrement compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snapshot {
    /// SHA-256 des octets du fichier (BOM et fins de ligne compris).
    pub sha256: String,
    #[ts(type = "number")]
    pub size: u64,
    /// Millisecondes depuis l'epoch Unix.
    #[ts(type = "number | null")]
    pub modified: Option<u64>,
}

/// Propriétaire ou groupe : identifiants et noms (lus dans `/etc/passwd`
/// et `/etc/group`, `null` s'ils n'y sont pas).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Ownership {
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub user: Option<String>,
    pub group: Option<String>,
}

/// Faits affichés sur un fichier ouvert ou refusé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileFacts {
    pub path: String,
    pub name: String,
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number | null")]
    pub modified: Option<u64>,
    /// `-rw-------`.
    pub permissions: String,
    /// Droits en octal : `600`.
    pub mode: Option<String>,
    pub owner: Ownership,
    pub dir: String,
    pub dir_permissions: String,
    pub dir_owner: Ownership,
    /// Compte de la liaison.
    pub account: String,
}

/// Fichier sous `~/.ssh` : bandeau permanent ; `authorizedKeys` passe en
/// plus par les vérifications et le GO / NO GO avant chaque enregistrement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SshFile {
    AuthorizedKeys,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TextDocument {
    pub file: FileFacts,
    /// Texte avec des fins de ligne LF.
    pub text: String,
    pub bom: bool,
    pub line_ending: LineEnding,
    pub snapshot: Snapshot,
    /// Vide : le fichier est éditable.
    pub read_only: Vec<ReadOnlyReason>,
    pub ssh_file: Option<SshFile>,
    #[ts(type = "number")]
    pub opened_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RefusedFile {
    pub file: FileFacts,
    pub refusal: Refusal,
    pub downloadable: bool,
    pub ssh_file: Option<SshFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum OpenedFile {
    Text(TextDocument),
    Refused(RefusedFile),
}

/// Version trouvée sur le serveur lors d'un conflit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerVersion {
    /// `None` : le fichier a été supprimé.
    pub snapshot: Option<Snapshot>,
    /// Texte (fins de ligne LF), s'il est lisible par l'éditeur.
    pub text: Option<String>,
    /// Raison pour laquelle il ne l'est plus (devenu binaire…).
    pub refusal: Option<Refusal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "outcome",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SaveOutcome {
    Saved {
        snapshot: Snapshot,
        /// La relecture après le remplacement donne exactement ce qui a été
        /// écrit : « identique au serveur ».
        verified: bool,
        #[ts(type = "number")]
        saved_at: u64,
    },
    /// Rien n'a été écrit.
    Conflict { server: ServerVersion },
}

/// Contenu à enregistrer, avec ce que l'ouverture a constaté.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SaveRequest {
    pub path: String,
    pub text: String,
    /// Version que l'on remplace ; `None` : le fichier doit être absent
    /// (recréation après une suppression sur le serveur).
    pub expected: Option<Snapshot>,
    pub bom: bool,
    pub line_ending: LineEnding,
    /// Droits à donner si le fichier doit être recréé ; sinon, ceux du
    /// fichier remplacé sont conservés.
    pub mode: u32,
}

/// Compte connecté et noms des propriétaires.
struct Identity {
    account: Account,
    users: Users,
    groups: HashMap<u32, String>,
}

impl Identity {
    fn ownership(&self, meta: Option<&Meta>) -> Ownership {
        let uid = meta.and_then(|m| m.uid);
        let gid = meta.and_then(|m| m.gid);
        Ownership {
            uid,
            gid,
            user: uid.and_then(|u| self.users.name(u)).map(str::to_owned),
            group: gid.and_then(|g| self.groups.get(&g)).cloned(),
        }
    }
}

fn owned(meta: Option<&Meta>) -> Owned {
    meta.map_or_else(Owned::default, |m| Owned {
        uid: m.uid,
        gid: m.gid,
        mode: m.mode,
    })
}

/// Lit au plus `cap` octets.
async fn read_capped(fs: &impl RemoteFs, file: &str, cap: u64) -> Result<Vec<u8>, AppError> {
    let reader = fs.open_read(file).await?;
    let mut bytes = Vec::new();
    reader
        .take(cap)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| AppError::io(format!("{file} : {e}")))?;
    Ok(bytes)
}

/// Fichier système lisible par tous, lu sans faire échouer l'opération.
async fn read_system_file(fs: &impl RemoteFs, file: &str) -> String {
    match read_capped(fs, file, MAX_SIZE).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

async fn identity(fs: &impl RemoteFs, user: &str, home: &str) -> Result<Identity, AppError> {
    let (passwd, group) = tokio::join!(
        read_system_file(fs, "/etc/passwd"),
        read_system_file(fs, "/etc/group")
    );
    let users = Users::parse(&passwd);
    let account = match users.account(user) {
        Some(account) => account,
        // Compte hors de /etc/passwd (annuaire) : propriétaire du dossier
        // personnel.
        None => {
            let meta = fs.lstat(home).await?;
            Account {
                name: user.to_owned(),
                uid: meta.as_ref().and_then(|m| m.uid).unwrap_or(u32::MAX),
                gid: meta.as_ref().and_then(|m| m.gid).unwrap_or(u32::MAX),
            }
        }
    };
    Ok(Identity {
        account,
        users,
        groups: access::parse_groups(&group),
    })
}

fn snapshot(bytes: &[u8], meta: &Meta) -> Snapshot {
    Snapshot {
        sha256: text::sha256_hex(bytes),
        size: bytes.len() as u64,
        modified: meta.modified.map(|s| s.saturating_mul(1000)),
    }
}

/// Fichier de `~/.ssh` (chemins résolus).
pub fn ssh_file(home: &str, resolved: &str) -> Option<SshFile> {
    let ssh_dir = path::join(home, ".ssh");
    if resolved == path::join(&ssh_dir, "authorized_keys") {
        Some(SshFile::AuthorizedKeys)
    } else if resolved != ssh_dir && path::is_within(resolved, &ssh_dir) {
        Some(SshFile::Other)
    } else {
        None
    }
}

/// Chemin résolu du fichier (dossier résolu suivi du nom) et dossier
/// personnel.
async fn resolve(fs: &impl RemoteFs, dir: &str, name: &str) -> Result<(String, String), AppError> {
    let (resolved_dir, home) = tokio::join!(fs.real_path(dir), fs.real_path("."));
    Ok((path::join(&resolved_dir?, name), home?))
}

/// Ouvre un fichier : son texte et ce qu'il faut en savoir, ou la raison du
/// refus. Un lien symbolique n'est jamais suivi.
pub async fn open(fs: &impl RemoteFs, file: &str, user: &str) -> Result<OpenedFile, AppError> {
    let (dir, name) = path::validate_item(file)?;
    let meta = fs
        .lstat(file)
        .await?
        .ok_or_else(|| AppError::not_found(file, FileSide::Remote))?;
    let (resolved, home) = resolve(fs, dir, name).await?;
    let (dir_meta, identity) = tokio::join!(fs.lstat(dir), identity(fs, user, &home));
    let (dir_meta, identity) = (dir_meta?, identity?);
    let ssh_file = ssh_file(&home, &resolved);
    let facts = FileFacts {
        path: file.to_owned(),
        name: name.to_owned(),
        size: meta.size,
        modified: meta.modified.map(|s| s.saturating_mul(1000)),
        permissions: path::permissions_string(meta.mode),
        mode: meta.mode.map(|m| format!("{:o}", m & 0o7777)),
        owner: identity.ownership(Some(&meta)),
        dir: dir.to_owned(),
        dir_permissions: path::permissions_string(dir_meta.as_ref().and_then(|m| m.mode)),
        dir_owner: identity.ownership(dir_meta.as_ref()),
        account: identity.account.name.clone(),
    };
    let refused = |refusal: Refusal, facts: FileFacts| {
        Ok(OpenedFile::Refused(RefusedFile {
            downloadable: refusal.downloadable(),
            refusal,
            file: facts,
            ssh_file,
        }))
    };
    match meta.kind {
        EntryKind::File => {}
        EntryKind::Symlink => {
            let target = fs.read_link(file).await.ok();
            return refused(Refusal::Symlink { target }, facts);
        }
        _ => return refused(Refusal::NotAFile, facts),
    }
    if meta.size > MAX_SIZE {
        return refused(
            Refusal::TooLarge {
                size: meta.size,
                limit: MAX_SIZE,
            },
            facts,
        );
    }
    let bytes = read_capped(fs, file, MAX_SIZE + 1).await?;
    if bytes.len() as u64 > MAX_SIZE {
        return refused(
            Refusal::TooLarge {
                size: bytes.len() as u64,
                limit: MAX_SIZE,
            },
            facts,
        );
    }
    let decoded = match text::decode(&bytes) {
        Ok(decoded) => decoded,
        Err(refusal) => return refused(refusal, facts),
    };
    let read_only = access::read_only_reasons(
        &owned(Some(&meta)),
        &owned(dir_meta.as_ref()),
        &identity.account,
        decoded.line_ending,
    );
    Ok(OpenedFile::Text(TextDocument {
        snapshot: snapshot(&bytes, &meta),
        file: facts,
        text: decoded.text,
        bom: decoded.bom,
        line_ending: decoded.line_ending,
        read_only,
        ssh_file,
        opened_at: now_millis(),
    }))
}

/// Version du serveur, pour la fenêtre de conflit.
fn server_version(bytes: &[u8], meta: &Meta) -> ServerVersion {
    let (text, refusal) = if bytes.len() as u64 > MAX_SIZE {
        (
            None,
            Some(Refusal::TooLarge {
                size: meta.size.max(bytes.len() as u64),
                limit: MAX_SIZE,
            }),
        )
    } else {
        match text::decode(bytes) {
            Ok(decoded) => (Some(decoded.text), None),
            Err(refusal) => (None, Some(refusal)),
        }
    };
    ServerVersion {
        snapshot: Some(snapshot(bytes, meta)),
        text,
        refusal,
    }
}

/// Le fichier, toujours présent, peut-il encore être remplacé ? (Droits ou
/// propriétaire changés sur le serveur depuis l'ouverture : l'empreinte ne
/// le voit pas.)
fn still_editable(file: &str, meta: &Meta, account: &Account) -> Result<(), AppError> {
    if meta.kind != EntryKind::File {
        return Err(AppError::invalid_input(format!(
            "{file} n'est plus un fichier ordinaire sur le serveur : rien n'a été écrit"
        )));
    }
    if meta.uid != Some(account.uid) {
        return Err(AppError::invalid_input(format!(
            "{file} appartient désormais à un autre compte : rien n'a été écrit"
        )));
    }
    if meta.mode.is_some_and(|m| m & 0o200 == 0) {
        return Err(AppError::invalid_input(format!(
            "{file} n'est plus inscriptible : rien n'a été écrit"
        )));
    }
    Ok(())
}

/// Écrit `bytes` dans le temporaire (créé exclusivement, droits 0600
/// pendant l'écriture), le synchronise sur disque puis lui donne `mode`.
async fn write_temp<F: RemoteFs>(
    fs: &F,
    temp: &str,
    dir: &str,
    bytes: &[u8],
    mode: u32,
) -> Result<(), AppError> {
    let mut writer = match fs.create_new(temp).await {
        Ok(writer) => writer,
        Err(AppError::PermissionDenied { .. }) => {
            return Err(AppError::permission_denied(dir, FileSide::Remote));
        }
        Err(error) => return Err(error),
    };
    let io = |e: std::io::Error| AppError::io(format!("{temp} : {e}"));
    fs.set_mode(temp, 0o600).await?;
    writer.write_all(bytes).await.map_err(io)?;
    fs.sync(&mut writer).await?;
    writer.shutdown().await.map_err(io)?;
    fs.set_mode(temp, mode).await
}

/// Enregistre le texte si le fichier n'a pas changé depuis `expected`.
pub async fn save<F: RemoteFs>(
    fs: &F,
    request: &SaveRequest,
    user: &str,
) -> Result<SaveOutcome, AppError> {
    let file = request.path.as_str();
    let (dir, name) = path::validate_item(file)?;
    let bytes = text::encode(&request.text, request.bom, request.line_ending)?;
    let current = fs.lstat(file).await?;

    let mode = match (&current, &request.expected) {
        (None, None) => request.mode & 0o7777,
        (None, Some(_)) => {
            return Ok(SaveOutcome::Conflict {
                server: ServerVersion {
                    snapshot: None,
                    text: None,
                    refusal: None,
                },
            })
        }
        (Some(meta), expected) => {
            if meta.kind == EntryKind::Symlink {
                return Err(AppError::invalid_input(format!(
                    "{file} est devenu un lien symbolique sur le serveur : rien n'a été écrit"
                )));
            }
            let server = if meta.kind == EntryKind::File {
                read_capped(fs, file, MAX_SIZE + 1).await?
            } else {
                Vec::new()
            };
            let unchanged = expected.as_ref().is_some_and(|e| {
                meta.kind == EntryKind::File && e.sha256 == text::sha256_hex(&server)
            });
            if !unchanged {
                return Ok(SaveOutcome::Conflict {
                    server: server_version(&server, meta),
                });
            }
            let home = fs.real_path(".").await?;
            let identity = identity(fs, user, &home).await?;
            still_editable(file, meta, &identity.account)?;
            meta.mode.map_or(0o644, |m| m & 0o7777)
        }
    };

    let temp = path::join(dir, &path::temp_name(name, &new_token(), TempKind::Part));
    let written = async {
        write_temp(fs, &temp, dir, &bytes, mode).await?;
        fs.replace(&temp, file).await
    }
    .await;
    if let Err(error) = written {
        // Le fichier d'origine est intact ; le temporaire est retiré (s'il
        // reste, l'explorateur le signale comme résidu).
        let _ = fs.remove_file(&temp).await;
        return Err(error);
    }

    let meta = fs
        .lstat(file)
        .await?
        .ok_or_else(|| AppError::not_found(file, FileSide::Remote))?;
    let reread = read_capped(fs, file, MAX_SIZE + 1).await?;
    Ok(SaveOutcome::Saved {
        verified: reread == bytes,
        snapshot: snapshot(&reread, &meta),
        saved_at: now_millis(),
    })
}

/// Vérifications de `~/.ssh/authorized_keys` entre la version ouverte et
/// celle à enregistrer.
pub async fn check_authorized_keys(
    fs: &impl RemoteFs,
    file: &str,
    before: &str,
    after: &str,
    key: Option<&PublicKey>,
) -> Result<AuthorizedKeysCheck, AppError> {
    let (dir, name) = path::validate_item(file)?;
    let (resolved, home) = resolve(fs, dir, name).await?;
    if ssh_file(&home, &resolved) != Some(SshFile::AuthorizedKeys) {
        return Err(AppError::invalid_input(format!(
            "{file} n'est pas ~/.ssh/authorized_keys"
        )));
    }
    let mode = fs.lstat(file).await?.and_then(|m| m.mode);
    Ok(authorized_keys::check(before, after, key, mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::fs::mem::MemFs;
    use std::sync::Arc;

    const HOME: &str = "/home/deploy";
    const ENV: &str = "/home/deploy/app/.env";

    fn fs() -> Arc<MemFs> {
        let fs = MemFs::new(HOME);
        fs.add_file(
            "/etc/passwd",
            b"root:x:0:0::/root:/bin/bash\ndeploy:x:1000:1000::/home/deploy:/bin/bash\n",
        );
        fs.add_file("/etc/group", b"root:x:0:\ndeploy:x:1000:\n");
        fs.set_owner("/etc/passwd", 0, 0);
        fs.add_file(ENV, b"PORT=8080\r\nLOG=info\r\n");
        fs.set_file_mode(ENV, 0o640);
        fs
    }

    async fn open_text(fs: &MemFs, file: &str) -> TextDocument {
        match open(fs, file, "deploy").await.unwrap() {
            OpenedFile::Text(document) => document,
            OpenedFile::Refused(refused) => panic!("refusé : {:?}", refused.refusal),
        }
    }

    async fn open_refused(fs: &MemFs, file: &str) -> RefusedFile {
        match open(fs, file, "deploy").await.unwrap() {
            OpenedFile::Refused(refused) => refused,
            OpenedFile::Text(_) => panic!("ouvert"),
        }
    }

    fn request(document: &TextDocument, text: &str) -> SaveRequest {
        SaveRequest {
            path: document.file.path.clone(),
            text: text.to_owned(),
            expected: Some(document.snapshot.clone()),
            bom: document.bom,
            line_ending: document.line_ending,
            mode: 0o644,
        }
    }

    fn temporaries(fs: &MemFs) -> Vec<String> {
        fs.paths()
            .into_iter()
            .filter(|p| p.contains(".atlas-"))
            .collect()
    }

    #[tokio::test]
    async fn opens_a_text_file_with_its_facts() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        assert_eq!(document.text, "PORT=8080\nLOG=info\n");
        assert_eq!(document.line_ending, LineEnding::Crlf);
        assert!(document.read_only.is_empty());
        assert_eq!(document.ssh_file, None);
        assert_eq!(document.file.mode.as_deref(), Some("640"));
        assert_eq!(document.file.permissions, "-rw-r-----");
        assert_eq!(document.file.owner.user.as_deref(), Some("deploy"));
        assert_eq!(document.file.dir, "/home/deploy/app");
        assert_eq!(document.file.account, "deploy");
        assert_eq!(
            document.snapshot.sha256,
            text::sha256_hex(b"PORT=8080\r\nLOG=info\r\n")
        );
        assert_eq!(document.snapshot.size, 21);
    }

    #[tokio::test]
    async fn refusals() {
        let fs = fs();
        fs.add_link("/home/deploy/lien", ENV);
        fs.add_file("/home/deploy/app.db", b"SQLite\0");
        fs.add_file("/home/deploy/gros.log", &vec![b'a'; MAX_SIZE as usize + 1]);
        fs.add_file("/srv/cle.pem", b"-----BEGIN OPENSSH PRIVATE KEY-----\n");
        let link = open_refused(&fs, "/home/deploy/lien").await;
        assert_eq!(
            link.refusal,
            Refusal::Symlink {
                target: Some(ENV.to_owned())
            }
        );
        assert!(!link.downloadable);
        assert_eq!(
            open_refused(&fs, "/home/deploy/app.db").await.refusal,
            Refusal::Binary { offset: 6 }
        );
        let large = open_refused(&fs, "/home/deploy/gros.log").await;
        assert!(matches!(large.refusal, Refusal::TooLarge { .. }));
        assert!(large.downloadable);
        let key = open_refused(&fs, "/srv/cle.pem").await;
        assert!(matches!(key.refusal, Refusal::PrivateKey { line: 1, .. }));
        assert!(!key.downloadable);
        assert_eq!(
            open_refused(&fs, "/home/deploy/app").await.refusal,
            Refusal::NotAFile
        );
    }

    #[tokio::test]
    async fn read_only_reasons_and_owner_names() {
        let fs = fs();
        fs.add_file("/etc/caddy/Caddyfile", b"{\n}\n");
        fs.set_owner("/etc/caddy/Caddyfile", 0, 0);
        fs.set_owner("/etc/caddy", 0, 0);
        let document = open_text(&fs, "/etc/caddy/Caddyfile").await;
        assert_eq!(
            document.read_only,
            [ReadOnlyReason::Owner, ReadOnlyReason::Directory]
        );
        assert_eq!(document.file.owner.user.as_deref(), Some("root"));
        assert_eq!(document.file.dir_owner.group.as_deref(), Some("root"));
    }

    #[tokio::test]
    async fn account_falls_back_to_the_home_owner() {
        let fs = fs();
        fs.write_file("/etc/passwd", b"");
        let document = open_text(&fs, ENV).await;
        assert!(document.read_only.is_empty());
        assert_eq!(document.file.owner.user, None);
    }

    #[tokio::test]
    async fn ssh_files_are_recognized() {
        let fs = fs();
        fs.add_file("/home/deploy/.ssh/authorized_keys", b"");
        fs.add_file("/home/deploy/.ssh/config", b"");
        assert_eq!(
            open_text(&fs, "/home/deploy/.ssh/authorized_keys")
                .await
                .ssh_file,
            Some(SshFile::AuthorizedKeys)
        );
        assert_eq!(
            open_text(&fs, "/home/deploy/.ssh/config").await.ssh_file,
            Some(SshFile::Other)
        );
    }

    #[test]
    fn ssh_file_rules_use_resolved_paths() {
        let home = "/home/deploy";
        assert_eq!(
            ssh_file(home, "/home/deploy/.ssh/authorized_keys"),
            Some(SshFile::AuthorizedKeys)
        );
        assert_eq!(
            ssh_file(home, "/home/deploy/.ssh/known_hosts"),
            Some(SshFile::Other)
        );
        assert_eq!(
            ssh_file(home, "/home/deploy/.ssh/sub/authorized_keys"),
            Some(SshFile::Other)
        );
        for other in [
            "/home/deploy/.ssh",
            "/home/deploy/.ssh2/authorized_keys",
            "/home/deploy/app/.ssh/authorized_keys",
        ] {
            assert_eq!(ssh_file(home, other), None, "{other}");
        }
    }

    #[tokio::test]
    async fn save_keeps_crlf_and_permissions_and_leaves_no_temporary() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        let outcome = save(&*fs, &request(&document, "PORT=9090\nLOG=info\n"), "deploy")
            .await
            .unwrap();
        let SaveOutcome::Saved {
            snapshot, verified, ..
        } = outcome
        else {
            panic!("conflit")
        };
        assert!(verified);
        assert_eq!(fs.content(ENV).unwrap(), b"PORT=9090\r\nLOG=info\r\n");
        assert_eq!(fs.mode(ENV), Some(0o100_640));
        assert_eq!(
            snapshot.sha256,
            text::sha256_hex(b"PORT=9090\r\nLOG=info\r\n")
        );
        assert!(temporaries(&fs).is_empty());
    }

    #[tokio::test]
    async fn save_keeps_the_bom() {
        let fs = fs();
        fs.add_file("/home/deploy/bom.txt", b"\xEF\xBB\xBFa\n");
        let document = open_text(&fs, "/home/deploy/bom.txt").await;
        save(&*fs, &request(&document, "b\n"), "deploy")
            .await
            .unwrap();
        assert_eq!(
            fs.content("/home/deploy/bom.txt").unwrap(),
            b"\xEF\xBB\xBFb\n"
        );
    }

    #[tokio::test]
    async fn changed_file_is_a_conflict_and_nothing_is_written() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.write_file(ENV, b"PORT=8080\r\nLOG=debug\r\nNEW=1\r\n");
        let outcome = save(&*fs, &request(&document, "PORT=1\n"), "deploy")
            .await
            .unwrap();
        let SaveOutcome::Conflict { server } = outcome else {
            panic!("enregistré")
        };
        assert_eq!(
            server.text.as_deref(),
            Some("PORT=8080\nLOG=debug\nNEW=1\n")
        );
        assert_eq!(server.snapshot.unwrap().size, 29);
        assert_eq!(
            fs.content(ENV).unwrap(),
            b"PORT=8080\r\nLOG=debug\r\nNEW=1\r\n"
        );
        assert!(temporaries(&fs).is_empty());
    }

    #[tokio::test]
    async fn overwrite_uses_the_server_snapshot_seen_in_the_conflict() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.write_file(ENV, b"X=1\r\n");
        let SaveOutcome::Conflict { server } = save(&*fs, &request(&document, "Y=2\n"), "deploy")
            .await
            .unwrap()
        else {
            panic!()
        };
        let mut overwrite = request(&document, "Y=2\n");
        overwrite.expected = server.snapshot;
        assert!(matches!(
            save(&*fs, &overwrite, "deploy").await.unwrap(),
            SaveOutcome::Saved { .. }
        ));
        assert_eq!(fs.content(ENV).unwrap(), b"Y=2\r\n");
    }

    #[tokio::test]
    async fn conflict_with_a_file_that_became_binary_or_was_deleted() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.write_file(ENV, b"\0\0");
        let SaveOutcome::Conflict { server } = save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(server.text, None);
        assert_eq!(server.refusal, Some(Refusal::Binary { offset: 0 }));

        fs.remove_file(ENV).await.unwrap();
        let SaveOutcome::Conflict { server } = save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(server.snapshot, None);

        // Recréation confirmée : droits de l'ouverture.
        let mut recreate = request(&document, "a\n");
        recreate.expected = None;
        recreate.mode = 0o640;
        save(&*fs, &recreate, "deploy").await.unwrap();
        assert_eq!(fs.content(ENV).unwrap(), b"a\r\n");
        assert_eq!(fs.mode(ENV), Some(0o100_640));
    }

    #[tokio::test]
    async fn permissions_or_owner_changed_on_the_server_block_the_save() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.set_file_mode(ENV, 0o440);
        let error = save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::InvalidInput { .. }));
        fs.set_file_mode(ENV, 0o640);
        fs.set_owner(ENV, 0, 0);
        assert!(save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .is_err());
        assert_eq!(fs.content(ENV).unwrap(), b"PORT=8080\r\nLOG=info\r\n");
    }

    #[tokio::test]
    async fn read_only_directory_gives_a_clear_error() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.faults
            .lock()
            .unwrap()
            .read_only_dirs
            .insert("/home/deploy/app".into());
        let error = save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .unwrap_err();
        assert!(
            matches!(&error, AppError::PermissionDenied { path, .. } if path == "/home/deploy/app")
        );
    }

    #[tokio::test]
    async fn failed_replace_keeps_the_original_and_removes_the_temporary() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.faults.lock().unwrap().replace_fails = true;
        assert!(save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .is_err());
        assert_eq!(fs.content(ENV).unwrap(), b"PORT=8080\r\nLOG=info\r\n");
        assert!(temporaries(&fs).is_empty());
    }

    #[tokio::test]
    async fn server_without_posix_rename_refuses_the_save() {
        let fs = fs();
        let document = open_text(&fs, ENV).await;
        fs.faults.lock().unwrap().no_posix_rename = true;
        assert!(save(&*fs, &request(&document, "a\n"), "deploy")
            .await
            .is_err());
        assert_eq!(fs.content(ENV).unwrap(), b"PORT=8080\r\nLOG=info\r\n");
    }

    #[tokio::test]
    async fn mixed_line_endings_are_never_saved() {
        let fs = fs();
        fs.add_file("/home/deploy/mixte.txt", b"a\r\nb\n");
        let document = open_text(&fs, "/home/deploy/mixte.txt").await;
        assert_eq!(document.read_only, [ReadOnlyReason::MixedLineEndings]);
        assert!(save(&*fs, &request(&document, "a\nb\n"), "deploy")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn authorized_keys_check_is_limited_to_that_file() {
        let fs = fs();
        fs.add_file("/home/deploy/.ssh/authorized_keys", b"");
        fs.set_file_mode("/home/deploy/.ssh/authorized_keys", 0o600);
        let check = check_authorized_keys(&*fs, "/home/deploy/.ssh/authorized_keys", "", "", None)
            .await
            .unwrap();
        assert_eq!(check.mode.as_deref(), Some("600"));
        assert!(check_authorized_keys(&*fs, ENV, "", "", None)
            .await
            .is_err());
    }
}
