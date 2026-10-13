//! Type d'erreur unique de l'application, sérialisé vers le frontend avec un
//! champ `kind` (voir `docs/04-contrat-commandes.md`).
//!
//! Le type TypeScript correspondant est généré par ts-rs dans `src/bindings/`.
//! Les messages `Display` servent aux logs ; l'interface traduit chaque `kind`.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, thiserror::Error, Serialize, TS)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum AppError {
    #[error("aucune connexion SSH active")]
    NotConnected,

    #[error("clé hôte inconnue : {fingerprint}")]
    UnknownHostKey { fingerprint: String },

    #[error("la clé hôte ne correspond pas (attendue {expected}, reçue {received})")]
    HostKeyMismatch { expected: String, received: String },

    #[error("passphrase de la clé SSH requise")]
    PassphraseRequired,

    #[error("échec de l'authentification SSH")]
    AuthFailed,

    #[error("erreur réseau : {message}")]
    Network { message: String },

    #[error("entrée invalide : {message}")]
    InvalidInput { message: String },

    #[error("la commande distante a échoué (code {exit_code}) : {stderr}")]
    Remote { exit_code: i32, stderr: String },

    #[error("erreur d'entrée/sortie : {message}")]
    Io { message: String },
}

#[cfg(test)]
mod tests {
    use super::AppError;
    use serde_json::{json, to_value};

    #[test]
    fn unit_variant_serializes_to_kind_only() {
        assert_eq!(
            to_value(AppError::NotConnected).unwrap(),
            json!({ "kind": "NotConnected" })
        );
    }

    #[test]
    fn host_key_mismatch_serializes_both_fingerprints() {
        let error = AppError::HostKeyMismatch {
            expected: "SHA256:attendue".into(),
            received: "SHA256:recue".into(),
        };
        assert_eq!(
            to_value(error).unwrap(),
            json!({
                "kind": "HostKeyMismatch",
                "expected": "SHA256:attendue",
                "received": "SHA256:recue",
            })
        );
    }

    #[test]
    fn remote_serializes_exit_code_in_camel_case() {
        let error = AppError::Remote {
            exit_code: 127,
            stderr: "command not found".into(),
        };
        assert_eq!(
            to_value(error).unwrap(),
            json!({
                "kind": "Remote",
                "exitCode": 127,
                "stderr": "command not found",
            })
        );
    }
}
