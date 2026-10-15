//! Vérification de l'empreinte de la clé hôte SSH.
//!
//! Empreinte inconnue : `Unknown`, la liaison est fermée avant toute
//! authentification et l'utilisateur doit comparer l'empreinte. Empreinte
//! différente de celle du profil : `Mismatch`, connexion bloquée.
//! La vérification n'est jamais désactivée ni contournée, et rien n'est écrit
//! dans `~/.ssh/known_hosts`.

use russh::keys::{HashAlg, PublicKey};

/// Préfixe et longueur d'une empreinte SHA-256 au format de `ssh-keygen -lf`.
const PREFIX: &str = "SHA256:";
const BASE64_LEN: usize = 43;

/// Empreinte au format exact de `ssh-keygen -lf` : `SHA256:` suivi du
/// condensé en base64 sans remplissage.
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// Vérifie qu'une chaîne a la forme d'une empreinte SHA-256.
pub fn is_valid_fingerprint(value: &str) -> bool {
    value.strip_prefix(PREFIX).is_some_and(|b64| {
        b64.len() == BASE64_LEN
            && b64
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyCheck {
    /// L'empreinte reçue est celle mémorisée dans le profil.
    Trusted,
    /// Aucune empreinte mémorisée : première connexion.
    Unknown,
    /// L'empreinte reçue diffère de celle mémorisée.
    Mismatch { expected: String },
}

/// Compare l'empreinte reçue à celle mémorisée. Comparaison exacte : pas de
/// normalisation, une seule forme est acceptée.
pub fn check(expected: Option<&str>, received: &str) -> HostKeyCheck {
    match expected {
        None => HostKeyCheck::Unknown,
        Some(expected) if expected == received => HostKeyCheck::Trusted,
        Some(expected) => HostKeyCheck::Mismatch {
            expected: expected.to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_key(openssh: &str) -> PublicKey {
        PublicKey::from_openssh(openssh.trim()).unwrap()
    }

    #[test]
    fn ed25519_fingerprint_matches_ssh_keygen() {
        let key = public_key(include_str!("../../tests/fixtures/host_ed25519.pub"));
        assert_eq!(
            fingerprint(&key),
            include_str!("../../tests/fixtures/host_ed25519.fingerprint")
        );
    }

    #[test]
    fn rsa_fingerprint_matches_ssh_keygen() {
        let key = public_key(include_str!("../../tests/fixtures/host_rsa.pub"));
        assert_eq!(
            fingerprint(&key),
            include_str!("../../tests/fixtures/host_rsa.fingerprint")
        );
    }

    #[test]
    fn generated_fingerprints_are_valid() {
        let key = public_key(include_str!("../../tests/fixtures/host_ed25519.pub"));
        let fp = fingerprint(&key);
        assert!(is_valid_fingerprint(&fp));
        assert!(!fp.ends_with('='), "pas de remplissage base64");
    }

    #[test]
    fn malformed_fingerprints_are_rejected() {
        let valid = include_str!("../../tests/fixtures/host_ed25519.fingerprint");
        assert!(is_valid_fingerprint(valid));
        for invalid in [
            "",
            "SHA256:",
            &valid.replace("SHA256:", "MD5:"),
            &valid.replace("SHA256:", "sha256:"),
            &format!("{valid}="),
            &valid[..valid.len() - 1],
            &format!(" {valid}"),
            &valid.replacen('/', "_", 1),
        ] {
            assert!(
                !is_valid_fingerprint(invalid),
                "accepté à tort : {invalid:?}"
            );
        }
    }

    #[test]
    fn check_without_stored_fingerprint_is_unknown() {
        assert_eq!(check(None, "SHA256:abc"), HostKeyCheck::Unknown);
    }

    #[test]
    fn check_with_same_fingerprint_is_trusted() {
        assert_eq!(
            check(Some("SHA256:abc"), "SHA256:abc"),
            HostKeyCheck::Trusted
        );
    }

    #[test]
    fn check_with_other_fingerprint_is_mismatch() {
        assert_eq!(
            check(Some("SHA256:abc"), "SHA256:abd"),
            HostKeyCheck::Mismatch {
                expected: "SHA256:abc".to_owned()
            }
        );
    }

    #[test]
    fn check_is_exact_and_case_sensitive() {
        assert!(matches!(
            check(Some("SHA256:abc"), "SHA256:ABC"),
            HostKeyCheck::Mismatch { .. }
        ));
        assert!(matches!(
            check(Some("SHA256:abc "), "SHA256:abc"),
            HostKeyCheck::Mismatch { .. }
        ));
    }
}
