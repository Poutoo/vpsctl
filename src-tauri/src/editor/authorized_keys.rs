//! Fonctions pures : lecture de `~/.ssh/authorized_keys` et vérifications
//! faites avant chaque enregistrement (clé de la liaison, clés reconnues,
//! ajoutées, retirées, droits).
//!
//! La lecture suit celle de sshd (`auth2-pubkeyfile.c`) : lignes vides et
//! commentaires ignorés ; une clé `type base64 [commentaire]`, sinon des
//! options (séparées par des virgules, valeurs entre guillemets avec `\"`)
//! puis la clé. La clé est validée par `ssh-key` (type annoncé conforme au
//! contenu). Une option inconnue, un type non pris en charge ou une clé
//! illisible rendent la ligne « non reconnue par Atlas » : on n'affirme
//! jamais ce que sshd en fera.

use russh::keys::ssh_key::public::KeyData;
use russh::keys::ssh_key::{Certificate, PublicKey};
use serde::Serialize;
use ts_rs::TS;

/// Options admises par sshd sans valeur, puis avec une valeur entre
/// guillemets (`sshd(8)`, AUTHORIZED_KEYS FILE FORMAT).
const FLAG_OPTIONS: &[&str] = &[
    "agent-forwarding",
    "cert-authority",
    "no-agent-forwarding",
    "no-port-forwarding",
    "no-pty",
    "no-touch-required",
    "no-user-rc",
    "no-x11-forwarding",
    "port-forwarding",
    "pty",
    "restrict",
    "touch-required",
    "user-rc",
    "verify-required",
    "x11-forwarding",
];
const VALUE_OPTIONS: &[&str] = &[
    "command",
    "environment",
    "expiry-time",
    "from",
    "permitlisten",
    "permitopen",
    "principals",
    "tunnel",
];

const CERT_SUFFIX: &str = "-cert-v01@openssh.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "AuthorizedKeyKind")]
pub enum EntryKind {
    /// Clé de connexion.
    Key,
    /// Certificat (`…-cert-v01@openssh.com`).
    Certificate,
    /// Autorité de certification (option `cert-authority`).
    Authority,
}

/// Ligne reconnue.
#[derive(Debug, Clone)]
pub struct Entry {
    pub line: u32,
    pub kind: EntryKind,
    /// Commentaire, ou fin du base64 s'il n'y en a pas.
    pub label: String,
    /// Noms des options, en minuscules.
    pub options: Vec<String>,
    /// Type et base64, pour comparer deux versions du fichier.
    identity: String,
    key: Option<KeyData>,
}

/// Lecture d'un fichier : lignes reconnues et numéros des autres.
#[derive(Debug, Default)]
pub struct Parsed {
    pub entries: Vec<Entry>,
    pub unrecognized: Vec<u32>,
}

/// Fin du base64 d'une clé, pour la désigner sans commentaire.
fn tail(base64: &str) -> String {
    let start = base64
        .char_indices()
        .rev()
        .nth(7)
        .map_or(0, |(index, _)| index);
    format!("…{}", &base64[start..])
}

pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    for (index, raw) in text.split('\n').enumerate() {
        let line = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let content = raw.trim_end_matches('\r').trim_start_matches([' ', '\t']);
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        match parse_entry(content, line) {
            Some(entry) => parsed.entries.push(entry),
            None => parsed.unrecognized.push(line),
        }
    }
    parsed
}

fn parse_entry(content: &str, line: u32) -> Option<Entry> {
    if let Some(entry) = parse_key(content, line, Vec::new()) {
        return Some(entry);
    }
    let (options, rest) = split_options(content)?;
    let options = parse_options(options)?;
    parse_key(rest.trim_start_matches([' ', '\t']), line, options)
}

/// Sépare les options (jusqu'au premier blanc hors guillemets) du reste.
fn split_options(content: &str) -> Option<(&str, &str)> {
    let bytes = content.as_bytes();
    let mut quoted = false;
    let mut i = 0;
    while i < bytes.len() && (quoted || (bytes[i] != b' ' && bytes[i] != b'\t')) {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'"') {
            i += 1;
        } else if bytes[i] == b'"' {
            quoted = !quoted;
        }
        i += 1;
    }
    (!quoted && i > 0).then(|| content.split_at(i))
}

/// Noms des options, ou `None` si l'une est inconnue ou mal formée.
fn parse_options(options: &str) -> Option<Vec<String>> {
    let mut names = Vec::new();
    let mut rest = options;
    loop {
        let name_end = rest.find(['=', ',']).unwrap_or(rest.len());
        let name = rest[..name_end].to_ascii_lowercase();
        rest = &rest[name_end..];
        if let Some(value) = rest.strip_prefix('=') {
            if !VALUE_OPTIONS.contains(&name.as_str()) {
                return None;
            }
            let end = closing_quote(value.strip_prefix('"')?)?;
            rest = &value[1 + end + 1..];
        } else if !FLAG_OPTIONS.contains(&name.as_str()) {
            return None;
        }
        names.push(name);
        match rest.strip_prefix(',') {
            Some(next) => rest = next,
            None if rest.is_empty() => return Some(names),
            None => return None,
        }
    }
}

/// Position du guillemet fermant (`\"` est un guillemet échappé).
fn closing_quote(value: &str) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if bytes.get(i + 1) == Some(&b'"') => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn parse_key(content: &str, line: u32, options: Vec<String>) -> Option<Entry> {
    let mut words = content.split_whitespace();
    let algorithm = words.next()?;
    let base64 = words.next()?;
    let identity = format!("{algorithm} {base64}");
    if algorithm.ends_with(CERT_SUFFIX) {
        let certificate = Certificate::from_openssh(content).ok()?;
        let label = non_empty(certificate.comment()).unwrap_or_else(|| tail(base64));
        return Some(Entry {
            line,
            kind: EntryKind::Certificate,
            label,
            options,
            identity,
            key: None,
        });
    }
    let key = PublicKey::from_openssh(content).ok()?;
    if key.key_data().is_other() {
        return None;
    }
    let kind = if options.iter().any(|o| o == "cert-authority") {
        EntryKind::Authority
    } else {
        EntryKind::Key
    };
    let label = non_empty(key.comment().as_str_lossy()).unwrap_or_else(|| tail(base64));
    Some(Entry {
        line,
        kind,
        label,
        options,
        identity,
        key: (kind == EntryKind::Key).then(|| key.key_data().clone()),
    })
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// Clé de la liaison en cours, telle qu'affichée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkKey {
    /// `ED25519`, `RSA`, `ECDSA P-256`…
    pub algorithm: String,
    pub comment: Option<String>,
    /// Fin du base64, comme on la voit dans le fichier.
    pub tail: String,
}

/// Présence de la clé de la liaison dans une version du fichier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeyPresence {
    pub line: u32,
    /// Options de cette ligne (`from`, `command`…), qui peuvent restreindre
    /// la liaison.
    pub options: Vec<String>,
}

/// Vérifications de `authorized_keys`, entre la version ouverte et la
/// vôtre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorizedKeysCheck {
    /// `None` si la clé de la liaison n'est pas connue.
    pub link_key: Option<LinkKey>,
    pub before: Option<KeyPresence>,
    pub after: Option<KeyPresence>,
    /// Lignes reconnues dans votre version : clés, certificats, autorités.
    pub keys: u32,
    pub certificates: u32,
    pub authorities: u32,
    /// Numéros des lignes non reconnues par Atlas, dans votre version.
    pub unrecognized: Vec<u32>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// Droits du fichier, conservés à l'enregistrement : `600`.
    pub mode: Option<String>,
    /// Le groupe ou les autres peuvent écrire : sshd refuse un tel fichier
    /// (`StrictModes`).
    pub too_open: bool,
}

fn algorithm_label(key: &PublicKey) -> String {
    match key.algorithm().as_str() {
        "ssh-ed25519" => "ED25519".to_owned(),
        "ssh-rsa" => "RSA".to_owned(),
        "ecdsa-sha2-nistp256" => "ECDSA P-256".to_owned(),
        "ecdsa-sha2-nistp384" => "ECDSA P-384".to_owned(),
        "ecdsa-sha2-nistp521" => "ECDSA P-521".to_owned(),
        "sk-ssh-ed25519@openssh.com" => "ED25519-SK".to_owned(),
        "sk-ecdsa-sha2-nistp256@openssh.com" => "ECDSA-SK".to_owned(),
        other => other.to_owned(),
    }
}

/// Description de la clé de la liaison.
pub fn link_key(key: &PublicKey) -> LinkKey {
    let openssh = key.to_openssh().unwrap_or_default();
    let base64 = openssh.split_whitespace().nth(1).unwrap_or_default();
    LinkKey {
        algorithm: algorithm_label(key),
        comment: non_empty(key.comment().as_str_lossy()),
        tail: tail(base64),
    }
}

fn presence(parsed: &Parsed, key: Option<&PublicKey>) -> Option<KeyPresence> {
    let data = key?.key_data();
    parsed
        .entries
        .iter()
        .find(|e| e.key.as_ref() == Some(data))
        .map(|e| KeyPresence {
            line: e.line,
            options: e.options.clone(),
        })
}

/// Libellés des lignes de `from` absentes de `to` (en tenant compte des
/// doublons).
fn missing(from: &Parsed, to: &Parsed) -> Vec<String> {
    let mut remaining: Vec<&str> = to.entries.iter().map(|e| e.identity.as_str()).collect();
    from.entries
        .iter()
        .filter(|entry| {
            match remaining
                .iter()
                .position(|identity| *identity == entry.identity)
            {
                Some(index) => {
                    remaining.swap_remove(index);
                    false
                }
                None => true,
            }
        })
        .map(|entry| entry.label.clone())
        .collect()
}

pub fn check(
    before: &str,
    after: &str,
    key: Option<&PublicKey>,
    mode: Option<u32>,
) -> AuthorizedKeysCheck {
    let (old, new) = (parse(before), parse(after));
    let count = |kind| {
        new.entries
            .iter()
            .filter(|e| e.kind == kind)
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    };
    AuthorizedKeysCheck {
        link_key: key.map(link_key),
        before: presence(&old, key),
        after: presence(&new, key),
        keys: count(EntryKind::Key),
        certificates: count(EntryKind::Certificate),
        authorities: count(EntryKind::Authority),
        unrecognized: new.unrecognized.clone(),
        added: missing(&new, &old),
        removed: missing(&old, &new),
        mode: mode.map(|m| format!("{:o}", m & 0o7777)),
        too_open: mode.is_some_and(|m| m & 0o022 != 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::private::Ed25519Keypair;
    use russh::keys::PrivateKey;

    fn key(seed: u8, comment: &str) -> PublicKey {
        let mut key = PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]))
            .public_key()
            .clone();
        key.set_comment(comment);
        key
    }

    fn line(key: &PublicKey) -> String {
        key.to_openssh().unwrap()
    }

    /// Certificat produit par `ssh-keygen -s ca -I test -n deploy` (clé et
    /// autorité jetables).
    const CERT: &str = "ssh-ed25519-cert-v01@openssh.com AAAAIHNzaC1lZDI1NTE5LWNlcnQtdjAxQG9wZW5zc2guY29tAAAAIBfbwpG6XCnJ9V3aFC/TNfJaESFfCHj5siWczluzfz0lAAAAIAK0tF2zUITQA+h413F1Sgynz3lkPnCps+7ZCaGTT5OdAAAAAAAAAAAAAAABAAAABHRlc3QAAAAKAAAABmRlcGxveQAAAAAAAAAA//////////8AAAAAAAAAggAAABVwZXJtaXQtWDExLWZvcndhcmRpbmcAAAAAAAAAF3Blcm1pdC1hZ2VudC1mb3J3YXJkaW5nAAAAAAAAABZwZXJtaXQtcG9ydC1mb3J3YXJkaW5nAAAAAAAAAApwZXJtaXQtcHR5AAAAAAAAAA5wZXJtaXQtdXNlci1yYwAAAAAAAAAAAAAAMwAAAAtzc2gtZWQyNTUxOQAAACAWALT51Mh1qu/E813ipwlZIdY9wgVljMAFWTqb1pI+7QAAAFMAAAALc3NoLWVkMjU1MTkAAABADEWH9kuTASa6827XaITD/JtOg+Sob/ayz7F0qpsg2tcM+219I7kYACc3pUpwpAbnKh6ocPSA3vfzDUYx23JrBQ== ci-cert";

    #[test]
    fn plain_keys_comments_and_blank_lines() {
        let a = key(1, "deploy@atlas-poste");
        let text = format!(
            "# compte deploy\n\n{}\n  \t{}\n",
            line(&a),
            line(&key(2, ""))
        );
        let parsed = parse(&text);
        assert_eq!(parsed.unrecognized, Vec::<u32>::new());
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].line, 3);
        assert_eq!(parsed.entries[0].label, "deploy@atlas-poste");
        assert!(parsed.entries[1].label.starts_with('…'));
        assert_eq!(parsed.entries[1].label.chars().count(), 9);
    }

    #[test]
    fn options_are_read() {
        let a = key(1, "ci");
        let text = format!(
            "from=\"10.0.0.1,192.168.1.*\",command=\"echo \\\"hi there\\\"\",no-pty,NO-X11-FORWARDING {}\nrestrict,cert-authority {}\n",
            line(&a),
            line(&key(3, "ca"))
        );
        let parsed = parse(&text);
        assert_eq!(parsed.unrecognized, Vec::<u32>::new());
        assert_eq!(
            parsed.entries[0].options,
            ["from", "command", "no-pty", "no-x11-forwarding"]
        );
        assert_eq!(parsed.entries[0].kind, EntryKind::Key);
        assert_eq!(parsed.entries[1].kind, EntryKind::Authority);
    }

    #[test]
    fn certificates_are_read() {
        let parsed = parse(&format!("{CERT}\nno-pty {CERT}\n"));
        assert_eq!(parsed.unrecognized, Vec::<u32>::new());
        assert_eq!(parsed.entries[0].kind, EntryKind::Certificate);
        assert_eq!(parsed.entries[0].label, "ci-cert");
        assert_eq!(parsed.entries[1].options, ["no-pty"]);
    }

    #[test]
    fn unreadable_lines_are_unrecognized() {
        let a = line(&key(1, "a"));
        let (algorithm, rest) = a.split_once(' ').unwrap();
        let text = [
            "ssh-ed25519 AAAA tronquée".to_owned(),
            format!("unknown-option {a}"),
            format!("from=10.0.0.1 {a}"),
            format!("from=\"10.0.0.1 {a}"),
            format!("ssh-rsa {rest}"),
            format!("{algorithm}  "),
            "n'importe quoi".to_owned(),
            "ssh-dss AAAAB3NzaC1kc3M=".to_owned(),
        ]
        .join("\n");
        assert_eq!(parse(&text).unrecognized, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn crlf_line_endings_are_tolerated() {
        let text = format!("{}\r\n", line(&key(1, "a")));
        assert_eq!(parse(&text).entries.len(), 1);
    }

    #[test]
    fn link_key_kept_and_counts() {
        let link = key(1, "deploy@atlas-poste");
        let ci = key(2, "ci@github-actions");
        let old = key(4, "ancien-portable");
        let before = format!("{}\n{}\n{}\n", line(&link), line(&ci), line(&old));
        let after = format!("# vps\n{}\n{}\n{CERT}\nbad line\n", line(&link), line(&ci));
        let check = check(&before, &after, Some(&link), Some(0o100_600));
        assert_eq!(
            check.before,
            Some(KeyPresence {
                line: 1,
                options: vec![]
            })
        );
        assert_eq!(check.after.as_ref().unwrap().line, 2);
        assert_eq!(
            (check.keys, check.certificates, check.authorities),
            (2, 1, 0)
        );
        assert_eq!(check.unrecognized, [5]);
        assert_eq!(check.added, ["ci-cert"]);
        assert_eq!(check.removed, ["ancien-portable"]);
        assert_eq!(check.mode.as_deref(), Some("600"));
        assert!(!check.too_open);
        let link_key = check.link_key.unwrap();
        assert_eq!(link_key.algorithm, "ED25519");
        assert_eq!(link_key.comment.as_deref(), Some("deploy@atlas-poste"));
        assert!(line(&link).ends_with(&format!(
            "{} deploy@atlas-poste",
            link_key.tail.trim_start_matches('…')
        )));
    }

    #[test]
    fn link_key_removed() {
        let link = key(1, "deploy@atlas-poste");
        let ci = key(2, "ci");
        let check = check(
            &format!("{}\n{}\n", line(&ci), line(&link)),
            &format!("{}\n", line(&ci)),
            Some(&link),
            Some(0o600),
        );
        assert_eq!(check.before.unwrap().line, 2);
        assert_eq!(check.after, None);
        assert_eq!(check.removed, ["deploy@atlas-poste"]);
    }

    #[test]
    fn link_key_absent_before_and_after() {
        let link = key(1, "deploy");
        let check = check(
            &format!("{}\n", line(&key(2, "ci"))),
            &format!("{}\n{}\n", line(&key(2, "ci")), line(&key(5, "backup-ops"))),
            Some(&link),
            Some(0o600),
        );
        assert_eq!((check.before, check.after), (None, None));
        assert_eq!(check.added, ["backup-ops"]);
        assert!(check.removed.is_empty());
    }

    #[test]
    fn link_key_with_a_new_comment_or_options_is_still_the_link_key() {
        let link = key(1, "deploy");
        let renamed = key(1, "autre-commentaire");
        let check = check(
            &line(&link),
            &format!("from=\"127.0.0.1\" {}", line(&renamed)),
            Some(&link),
            None,
        );
        assert_eq!(
            check.after,
            Some(KeyPresence {
                line: 1,
                options: vec!["from".into()]
            })
        );
    }

    #[test]
    fn link_key_as_an_authority_does_not_count() {
        let link = key(1, "deploy");
        let check = check(
            "",
            &format!("cert-authority {}", line(&link)),
            Some(&link),
            None,
        );
        assert_eq!(check.after, None);
        assert_eq!(check.authorities, 1);
    }

    #[test]
    fn duplicates_are_counted_once_each() {
        let a = line(&key(1, "a"));
        let check = check(&format!("{a}\n{a}\n"), &format!("{a}\n"), None, None);
        assert_eq!(check.removed, ["a"]);
        assert!(check.link_key.is_none());
    }

    #[test]
    fn group_or_other_write_is_too_open() {
        assert!(check("", "", None, Some(0o100_620)).too_open);
        assert!(check("", "", None, Some(0o602)).too_open);
        assert!(!check("", "", None, Some(0o644)).too_open);
    }
}
