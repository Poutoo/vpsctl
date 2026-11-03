//! Fonctions pures : compte connecté, noms des propriétaires et raisons de
//! la lecture seule.
//!
//! Sans commande shell, le compte connecté (uid, groupe principal) est lu
//! dans `/etc/passwd` d'après le nom d'utilisateur du profil ; à défaut
//! (annuaire LDAP…), on suppose qu'il possède son dossier personnel. Ses
//! groupes secondaires restent inconnus : un dossier inscriptible par l'un
//! d'eux est laissé éditable, et l'enregistrement échoue avec une erreur
//! claire s'il ne l'est pas.

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use super::text::LineEnding;

/// Compte de la liaison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
}

/// Propriétaire et droits d'un élément, tels que lus par `lstat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Owned {
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub mode: Option<u32>,
}

/// Raison de la lecture seule, dans l'ordre d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReadOnlyReason {
    /// Le compte possède le fichier mais n'a pas le droit d'y écrire.
    Permissions,
    /// Le fichier appartient à un autre compte : le remplacer par renommage
    /// créerait un fichier au nom du compte connecté.
    Owner,
    /// Le temporaire ne peut pas être créé dans le dossier.
    Directory,
    /// Fins de ligne mélangées : impossibles à conserver fidèlement.
    MixedLineEndings,
}

/// Entrées utiles de `/etc/passwd` : nom → (uid, gid), uid → nom.
#[derive(Debug, Default)]
pub struct Users {
    by_name: HashMap<String, (u32, u32)>,
    names: HashMap<u32, String>,
}

impl Users {
    pub fn parse(passwd: &str) -> Self {
        let mut users = Self::default();
        for line in passwd.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            let (Some(name), Some(uid), Some(gid)) = (
                fields.first(),
                fields.get(2).and_then(|u| u.parse().ok()),
                fields.get(3).and_then(|g| g.parse().ok()),
            ) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            users.by_name.insert((*name).to_owned(), (uid, gid));
            users.names.entry(uid).or_insert_with(|| (*name).to_owned());
        }
        users
    }

    pub fn account(&self, name: &str) -> Option<Account> {
        self.by_name.get(name).map(|&(uid, gid)| Account {
            name: name.to_owned(),
            uid,
            gid,
        })
    }

    pub fn name(&self, uid: u32) -> Option<&str> {
        self.names.get(&uid).map(String::as_str)
    }
}

/// Noms des groupes de `/etc/group` : gid → nom.
pub fn parse_groups(group: &str) -> HashMap<u32, String> {
    let mut names = HashMap::new();
    for line in group.lines() {
        let mut fields = line.split(':');
        let (Some(name), Some(gid)) = (fields.next(), fields.nth(1).and_then(|g| g.parse().ok()))
        else {
            continue;
        };
        if !name.is_empty() {
            names.entry(gid).or_insert_with(|| name.to_owned());
        }
    }
    names
}

const USER_WRITE: u32 = 0o200;
const USER_EXEC: u32 = 0o100;
const GROUP_WX: u32 = 0o030;
const OTHER_WX: u32 = 0o003;

/// Le compte peut-il créer un fichier dans ce dossier ? Le propriétaire
/// n'est jugé que sur ses propres droits (règle POSIX). Un dossier
/// inscriptible par un groupe autre que le groupe principal est considéré
/// inscriptible : l'appartenance aux groupes secondaires est inconnue.
fn dir_writable(dir: &Owned, account: &Account) -> bool {
    let Some(mode) = dir.mode else {
        return true;
    };
    if dir.uid == Some(account.uid) {
        return mode & (USER_WRITE | USER_EXEC) == USER_WRITE | USER_EXEC;
    }
    mode & OTHER_WX == OTHER_WX || mode & GROUP_WX == GROUP_WX
}

/// Raisons de la lecture seule, dans l'ordre d'affichage ; vide si le
/// fichier est éditable.
pub fn read_only_reasons(
    file: &Owned,
    dir: &Owned,
    account: &Account,
    line_ending: LineEnding,
) -> Vec<ReadOnlyReason> {
    let mut reasons = Vec::new();
    let owned = file.uid == Some(account.uid);
    if owned && file.mode.is_some_and(|mode| mode & USER_WRITE == 0) {
        reasons.push(ReadOnlyReason::Permissions);
    }
    if !owned {
        reasons.push(ReadOnlyReason::Owner);
    }
    if !dir_writable(dir, account) {
        reasons.push(ReadOnlyReason::Directory);
    }
    if line_ending == LineEnding::Mixed {
        reasons.push(ReadOnlyReason::MixedLineEndings);
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deploy() -> Account {
        Account {
            name: "deploy".into(),
            uid: 1000,
            gid: 1000,
        }
    }

    fn owned(uid: u32, gid: u32, mode: u32) -> Owned {
        Owned {
            uid: Some(uid),
            gid: Some(gid),
            mode: Some(mode),
        }
    }

    const PASSWD: &str = "root:x:0:0:root:/root:/bin/bash\n\
        daemon:x:1:1::/usr/sbin:/usr/sbin/nologin\n\
        broken line\n\
        deploy:x:1000:1000:Deploy,,,:/home/deploy:/bin/bash\n";

    #[test]
    fn passwd_gives_the_account_and_owner_names() {
        let users = Users::parse(PASSWD);
        assert_eq!(users.account("deploy"), Some(deploy()));
        assert_eq!(users.account("absent"), None);
        assert_eq!(users.name(0), Some("root"));
        assert_eq!(users.name(4242), None);
    }

    #[test]
    fn group_names_are_parsed() {
        let groups = parse_groups("root:x:0:\ndocker:x:998:deploy\n:x:5:\nbad\n");
        assert_eq!(groups.get(&998).map(String::as_str), Some("docker"));
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn own_writable_file_in_own_directory_is_editable() {
        let reasons = read_only_reasons(
            &owned(1000, 1000, 0o100_600),
            &owned(1000, 1000, 0o040_755),
            &deploy(),
            LineEnding::Lf,
        );
        assert!(reasons.is_empty());
    }

    #[test]
    fn own_file_without_write_permission() {
        let reasons = read_only_reasons(
            &owned(1000, 1000, 0o100_444),
            &owned(1000, 1000, 0o040_755),
            &deploy(),
            LineEnding::Lf,
        );
        assert_eq!(reasons, [ReadOnlyReason::Permissions]);
    }

    #[test]
    fn file_of_another_owner_is_read_only_even_if_writable() {
        let reasons = read_only_reasons(
            &owned(0, 1000, 0o100_666),
            &owned(1000, 1000, 0o040_755),
            &deploy(),
            LineEnding::Lf,
        );
        assert_eq!(reasons, [ReadOnlyReason::Owner]);
    }

    #[test]
    fn unknown_owner_counts_as_another_owner() {
        let reasons = read_only_reasons(
            &Owned::default(),
            &owned(1000, 1000, 0o040_755),
            &deploy(),
            LineEnding::Lf,
        );
        assert_eq!(reasons, [ReadOnlyReason::Owner]);
    }

    #[test]
    fn directory_rules() {
        let check = |dir: Owned| {
            read_only_reasons(
                &owned(1000, 1000, 0o100_644),
                &dir,
                &deploy(),
                LineEnding::Lf,
            )
        };
        // Propriétaire : seuls ses droits comptent, même si les autres
        // pourraient écrire.
        assert_eq!(
            check(owned(1000, 1000, 0o040_577)),
            [ReadOnlyReason::Directory]
        );
        assert_eq!(
            check(owned(1000, 1000, 0o040_655)),
            [ReadOnlyReason::Directory]
        );
        assert!(check(owned(1000, 1000, 0o040_700)).is_empty());
        // Autre propriétaire.
        assert_eq!(check(owned(0, 0, 0o040_755)), [ReadOnlyReason::Directory]);
        assert!(check(owned(0, 0, 0o041_777)).is_empty());
        assert!(check(owned(0, 1000, 0o040_775)).is_empty());
        // Groupe secondaire possible : laissé éditable.
        assert!(check(owned(0, 998, 0o040_775)).is_empty());
        // Droits inconnus : laissé éditable, l'écriture tranchera.
        assert!(check(Owned::default()).is_empty());
    }

    #[test]
    fn mixed_line_endings_are_read_only_and_reasons_keep_their_order() {
        let reasons = read_only_reasons(
            &owned(1000, 1000, 0o100_400),
            &owned(0, 0, 0o040_755),
            &deploy(),
            LineEnding::Mixed,
        );
        assert_eq!(
            reasons,
            [
                ReadOnlyReason::Permissions,
                ReadOnlyReason::Directory,
                ReadOnlyReason::MixedLineEndings
            ]
        );
    }
}
