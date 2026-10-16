//! Fonctions pures sur les chemins distants (POSIX) et les noms : validation,
//! jointure, chaîne de droits, noms des temporaires de transfert, résidus et
//! chemins critiques pour l'accès SSH. Aucune requête au serveur.
//!
//! Les chemins ne sont jamais résolus ni normalisés : un chemin qui contient
//! `.`, `..` ou un composant vide est refusé, pas corrigé.

use super::EntryKind;
use crate::error::AppError;

/// Longueur maximale d'un nom (`NAME_MAX` de Linux), en octets.
pub const MAX_NAME_BYTES: usize = 255;
/// Longueur maximale d'un chemin (`PATH_MAX` de Linux), en octets.
const MAX_PATH_BYTES: usize = 4096;

/// Marque des temporaires de transfert : `.<nom>.atlas-<jeton>.<part|old>`.
const TEMP_MARK: &str = ".atlas-";
/// Longueur du jeton hexadécimal d'un temporaire.
pub const TOKEN_LEN: usize = 8;

/// Nom saisi dans l'interface (création, renommage) : ni vide, ni `.`, ni
/// `..`, sans `/` ni caractère nul.
pub fn validate_name(name: &str) -> Result<(), AppError> {
    let problem = if name.is_empty() {
        "le nom est vide"
    } else if name == "." || name == ".." {
        "« . » et « .. » ne sont pas des noms valides"
    } else if name.contains('/') {
        "le nom ne peut pas contenir « / »"
    } else if name.contains('\0') {
        "le nom ne peut pas contenir de caractère nul"
    } else if name.len() > MAX_NAME_BYTES {
        "le nom dépasse 255 octets"
    } else {
        return Ok(());
    };
    Err(AppError::invalid_input(problem))
}

/// Chemin distant : absolu, sans caractère nul, sans composant vide, `.` ou
/// `..`. `/` seul désigne la racine.
pub fn validate_path(path: &str) -> Result<(), AppError> {
    let invalid = |problem: &str| Err(AppError::invalid_input(format!("{problem} : {path}")));
    if !path.starts_with('/') {
        return invalid("chemin distant non absolu");
    }
    if path.contains('\0') {
        return invalid("caractère nul dans le chemin");
    }
    if path.len() > MAX_PATH_BYTES {
        return invalid("chemin trop long");
    }
    if path == "/" {
        return Ok(());
    }
    for component in path[1..].split('/') {
        match component {
            "" => return invalid("composant vide dans le chemin"),
            "." | ".." => return invalid("« . » et « .. » sont refusés dans un chemin"),
            _ => {}
        }
    }
    Ok(())
}

/// Chemin valide d'un élément (pas la racine), découpé en dossier parent et
/// nom.
pub fn validate_item(path: &str) -> Result<(&str, &str), AppError> {
    validate_path(path)?;
    let (parent, name) =
        split(path).ok_or_else(|| AppError::invalid_input("la racine n'est pas un élément"))?;
    validate_name(name)?;
    Ok((parent, name))
}

/// Chemin d'un élément du dossier `dir`.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// Dossier parent et nom d'un chemin absolu. `None` pour la racine.
pub fn split(path: &str) -> Option<(&str, &str)> {
    if path == "/" {
        return None;
    }
    let index = path.rfind('/')?;
    let parent = if index == 0 { "/" } else { &path[..index] };
    Some((parent, &path[index + 1..]))
}

/// `path` est `ancestor` ou se trouve dessous (comparaison des composants,
/// sans résolution).
pub fn is_within(path: &str, ancestor: &str) -> bool {
    if ancestor == "/" {
        return path.starts_with('/');
    }
    path == ancestor
        || path
            .strip_prefix(ancestor)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Genre d'un élément d'après son mode (bits de type de `st_mode`).
pub fn kind_of_mode(mode: Option<u32>) -> EntryKind {
    match mode.map(|m| m & 0o170_000) {
        Some(0o040_000) => EntryKind::Dir,
        Some(0o100_000) => EntryKind::File,
        Some(0o120_000) => EntryKind::Symlink,
        _ => EntryKind::Other,
    }
}

/// Droits au format de `ls -l` : `drwxr-xr-x`, avec `s`/`S` (setuid,
/// setgid) et `t`/`T` (sticky). Chaîne vide si le mode est inconnu.
pub fn permissions_string(mode: Option<u32>) -> String {
    let Some(mode) = mode else {
        return String::new();
    };
    let mut text = String::with_capacity(10);
    text.push(match mode & 0o170_000 {
        0o040_000 => 'd',
        0o120_000 => 'l',
        0o100_000 => '-',
        0o020_000 => 'c',
        0o060_000 => 'b',
        0o010_000 => 'p',
        0o140_000 => 's',
        _ => '?',
    });
    for (shift, special_bit, special) in [(6, 0o4000, 's'), (3, 0o2000, 's'), (0, 0o1000, 't')] {
        let bits = (mode >> shift) & 0o7;
        text.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        text.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        text.push(match (bits & 0o1 != 0, mode & special_bit != 0) {
            (true, true) => special,
            (false, true) => special.to_ascii_uppercase(),
            (true, false) => 'x',
            (false, false) => '-',
        });
    }
    text
}

/// Temporaire de transfert : en cours d'écriture, ou fichier d'origine mis
/// de côté pendant un écrasement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempKind {
    Part,
    Old,
}

impl TempKind {
    fn extension(self) -> &'static str {
        match self {
            Self::Part => "part",
            Self::Old => "old",
        }
    }
}

/// Nom d'un temporaire : `.<nom>.atlas-<jeton>.<part|old>`, caché et dans le
/// même dossier. Le nom d'origine est raccourci (à une frontière de
/// caractère) si l'ensemble dépasserait 255 octets.
pub fn temp_name(name: &str, token: &str, kind: TempKind) -> String {
    let extension = kind.extension();
    let overhead = 1 + TEMP_MARK.len() + token.len() + 1 + extension.len();
    let mut end = name.len().min(MAX_NAME_BYTES.saturating_sub(overhead));
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    format!(".{}{TEMP_MARK}{token}.{extension}", &name[..end])
}

/// Reconnaît un résidu de transfert et renvoie son genre et le nom d'origine.
pub fn residue_of(name: &str) -> Option<(TempKind, &str)> {
    let rest = name.strip_prefix('.')?;
    let (stem, kind) = match rest.strip_suffix(".part") {
        Some(stem) => (stem, TempKind::Part),
        None => (rest.strip_suffix(".old")?, TempKind::Old),
    };
    let split_at = stem.len().checked_sub(TOKEN_LEN)?;
    if !stem.is_char_boundary(split_at) {
        return None;
    }
    let (head, token) = stem.split_at(split_at);
    if !token
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let original = head.strip_suffix(TEMP_MARK)?;
    (!original.is_empty()).then_some((kind, original))
}

/// Supprimer, renommer ou écraser `path` peut couper l'accès SSH : c'est
/// `~/.ssh/authorized_keys`, `~/.ssh`, ou un parent de `~/.ssh` situé sous le
/// dossier personnel (le dossier personnel lui-même exclu). `home` et `path`
/// sont des chemins absolus résolus.
pub fn ssh_access(home: &str, path: &str) -> bool {
    let ssh_dir = join(home, ".ssh");
    let authorized_keys = join(&ssh_dir, "authorized_keys");
    path == authorized_keys || (is_within(&ssh_dir, path) && path != home && is_within(path, home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_validated() {
        for name in ["app", ".env", "nom avec espaces", "été.txt", "a\\b", "..."] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in ["", ".", "..", "a/b", "/", "a\0b"] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        assert!(validate_name(&"a".repeat(255)).is_ok());
        assert!(validate_name(&"a".repeat(256)).is_err());
        // 128 caractères de 2 octets : 256 octets.
        assert!(validate_name(&"é".repeat(128)).is_err());
    }

    #[test]
    fn paths_are_validated_without_normalization() {
        for path in [
            "/",
            "/home",
            "/home/deploy/app",
            "/home/deploy/.env",
            "/a b/c",
        ] {
            assert!(validate_path(path).is_ok(), "{path}");
        }
        for path in [
            "",
            "home",
            "~/app",
            "/home/",
            "//home",
            "/home//deploy",
            "/home/./deploy",
            "/home/../root",
            "/..",
            "/home/a\0b",
        ] {
            assert!(validate_path(path).is_err(), "{path:?}");
        }
        let long = format!("/{}", vec!["a"; 2100].join("/"));
        assert!(validate_path(&long).is_err());
    }

    #[test]
    fn an_item_is_a_valid_path_other_than_the_root() {
        assert_eq!(
            validate_item("/home/deploy/app").unwrap(),
            ("/home/deploy", "app")
        );
        assert_eq!(validate_item("/srv").unwrap(), ("/", "srv"));
        assert!(validate_item("/").is_err());
        assert!(validate_item("/home/..").is_err());
    }

    #[test]
    fn paths_are_joined_and_split() {
        assert_eq!(join("/", "srv"), "/srv");
        assert_eq!(join("/home/deploy", ".env"), "/home/deploy/.env");
        assert_eq!(split("/srv"), Some(("/", "srv")));
        assert_eq!(split("/home/deploy/app"), Some(("/home/deploy", "app")));
        assert_eq!(split("/"), None);
        for path in ["/srv", "/home/deploy/app"] {
            let (parent, name) = split(path).unwrap();
            assert_eq!(join(parent, name), path);
        }
    }

    #[test]
    fn within_compares_whole_components() {
        assert!(is_within("/home/deploy", "/home/deploy"));
        assert!(is_within("/home/deploy/.ssh", "/home/deploy"));
        assert!(is_within("/home", "/"));
        assert!(!is_within("/home/deployer", "/home/deploy"));
        assert!(!is_within("/home", "/home/deploy"));
    }

    #[test]
    fn modes_give_the_entry_kind() {
        assert_eq!(kind_of_mode(Some(0o040_755)), EntryKind::Dir);
        assert_eq!(kind_of_mode(Some(0o100_644)), EntryKind::File);
        assert_eq!(kind_of_mode(Some(0o120_777)), EntryKind::Symlink);
        assert_eq!(kind_of_mode(Some(0o010_644)), EntryKind::Other);
        assert_eq!(kind_of_mode(Some(0o140_755)), EntryKind::Other);
        assert_eq!(kind_of_mode(None), EntryKind::Other);
    }

    #[test]
    fn permissions_are_formatted_like_ls() {
        assert_eq!(permissions_string(Some(0o040_755)), "drwxr-xr-x");
        assert_eq!(permissions_string(Some(0o100_644)), "-rw-r--r--");
        assert_eq!(permissions_string(Some(0o100_600)), "-rw-------");
        assert_eq!(permissions_string(Some(0o120_777)), "lrwxrwxrwx");
        assert_eq!(permissions_string(Some(0o104_755)), "-rwsr-xr-x");
        assert_eq!(permissions_string(Some(0o102_644)), "-rw-r-Sr--");
        assert_eq!(permissions_string(Some(0o041_777)), "drwxrwxrwt");
        assert_eq!(permissions_string(Some(0o041_776)), "drwxrwxrwT");
        assert_eq!(permissions_string(Some(0o010_644)), "prw-r--r--");
        assert_eq!(permissions_string(None), "");
    }

    #[test]
    fn temp_names_are_hidden_and_recognized_as_residues() {
        let part = temp_name("backup.sql.gz", "0123abcd", TempKind::Part);
        assert_eq!(part, ".backup.sql.gz.atlas-0123abcd.part");
        assert_eq!(residue_of(&part), Some((TempKind::Part, "backup.sql.gz")));

        let old = temp_name(".env", "ffff0000", TempKind::Old);
        assert_eq!(old, "..env.atlas-ffff0000.old");
        assert_eq!(residue_of(&old), Some((TempKind::Old, ".env")));
    }

    #[test]
    fn long_names_are_shortened_to_fit() {
        let long = "é".repeat(200);
        let part = temp_name(&long, "0123abcd", TempKind::Part);
        assert!(part.len() <= MAX_NAME_BYTES);
        assert!(validate_name(&part).is_ok());
        let (kind, original) = residue_of(&part).unwrap();
        assert_eq!(kind, TempKind::Part);
        assert!(long.starts_with(original));
    }

    #[test]
    fn other_names_are_not_residues() {
        for name in [
            "backup.part",
            ".backup.part",
            ".backup.atlas-0123abcd",
            ".backup.atlas-0123ABCD.part",
            ".backup.atlas-0123abc.part",
            ".backup.atlas-0123abcz.old",
            "backup.atlas-0123abcd.part",
            "..atlas-0123abcd.part",
            ".atlas-0123abcd.old",
        ] {
            assert_eq!(residue_of(name), None, "{name}");
        }
    }

    #[test]
    fn ssh_access_paths_are_detected() {
        let home = "/home/deploy";
        for path in ["/home/deploy/.ssh", "/home/deploy/.ssh/authorized_keys"] {
            assert!(ssh_access(home, path), "{path}");
        }
        for path in [
            "/home/deploy",
            "/home",
            "/",
            "/home/deploy/.ssh/id_ed25519",
            "/home/deploy/.ssh/known_hosts",
            "/home/deploy/.sshx",
            "/home/deploy/.ssh2/authorized_keys",
            "/home/deploy/app/.ssh",
            "/home/deploy/app/.ssh/authorized_keys",
            "/home/deployer/.ssh",
        ] {
            assert!(!ssh_access(home, path), "{path}");
        }
        // Dossier personnel à la racine (compte root, cas limite).
        assert!(ssh_access("/", "/.ssh"));
        assert!(ssh_access("/", "/.ssh/authorized_keys"));
        assert!(!ssh_access("/", "/"));
    }
}
