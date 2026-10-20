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

    /// L'opération a échoué parce que la liaison est perdue. L'événement
    /// `link-lost` en donne la cause ; l'interface ne signale pas cet échec
    /// comme celui de l'opération.
    #[error("liaison perdue")]
    LinkLost,

    #[error("clé hôte inconnue : {fingerprint}")]
    UnknownHostKey { fingerprint: String },

    #[error("la clé hôte ne correspond pas (attendue {expected}, reçue {received})")]
    HostKeyMismatch { expected: String, received: String },

    #[error("passphrase de la clé SSH requise")]
    PassphraseRequired,

    #[error("passphrase de la clé SSH incorrecte")]
    PassphraseIncorrect,

    #[error("échec de l'authentification SSH")]
    AuthFailed,

    #[error("connexion interrompue")]
    Cancelled,

    #[error("erreur réseau : {message}")]
    Network { message: String },

    #[error("entrée invalide : {message}")]
    InvalidInput { message: String },

    #[error("la commande distante a échoué (code {exit_code}) : {stderr}")]
    Remote { exit_code: i32, stderr: String },

    #[error("erreur d'entrée/sortie : {message}")]
    Io { message: String },

    #[error("Docker n'est pas installé sur le serveur")]
    DockerNotInstalled,

    #[error("accès à Docker refusé (utilisateur hors du groupe docker)")]
    DockerPermissionDenied,

    #[error("accès refusé ({side}) : {path}")]
    PermissionDenied { path: String, side: FileSide },

    #[error("élément introuvable ({side}) : {path}")]
    NotFound { path: String, side: FileSide },

    /// Sur le VPS uniquement.
    #[error("un élément porte déjà ce nom : {path}")]
    AlreadyExists { path: String, is_dir: bool },

    #[error("espace disque insuffisant ({side}) : {path}")]
    InsufficientSpace {
        path: String,
        side: FileSide,
        #[ts(type = "number | null")]
        needed: Option<u64>,
        #[ts(type = "number | null")]
        available: Option<u64>,
    },

    /// Création ou suppression d'un service arrêtée en cours de séquence :
    /// `step` est l'étape en échec, `state` décrit où en est le service.
    #[error("{unit} : échec à l'étape {step} ({state}) : {cause}")]
    ServiceStepFailed {
        unit: String,
        step: ServiceStep,
        state: String,
        cause: Box<AppError>,
    },
}

/// Étape d'une séquence de création (`Reload`, `Enable`) ou de suppression
/// d'un service (voir `docs/04-contrat-commandes.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ServiceStep {
    Reload,
    Enable,
    Stop,
    Disable,
    RemoveDropIn,
    RemoveFile,
    ResetFailed,
}

impl std::fmt::Display for ServiceStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Reload => "daemon-reload",
            Self::Enable => "enable",
            Self::Stop => "stop",
            Self::Disable => "disable",
            Self::RemoveDropIn => "suppression du dossier de surcharge",
            Self::RemoveFile => "suppression du fichier",
            Self::ResetFailed => "reset-failed",
        })
    }
}

/// Côté d'un élément de fichier : le VPS ou la machine locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileSide {
    Local,
    Remote,
}

impl std::fmt::Display for FileSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Local => "local",
            Self::Remote => "VPS",
        })
    }
}

impl AppError {
    pub fn permission_denied(path: impl Into<String>, side: FileSide) -> Self {
        Self::PermissionDenied {
            path: path.into(),
            side,
        }
    }

    pub fn not_found(path: impl Into<String>, side: FileSide) -> Self {
        Self::NotFound {
            path: path.into(),
            side,
        }
    }

    pub fn already_exists(path: impl Into<String>, is_dir: bool) -> Self {
        Self::AlreadyExists {
            path: path.into(),
            is_dir,
        }
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }
}

/// Cause d'un échec avant l'établissement de la liaison TCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkFailure {
    HostNotFound,
    Refused,
    TimedOut { seconds: u64 },
    Other(String),
}

impl NetworkFailure {
    /// Classe une erreur de `TcpStream::connect`.
    pub fn from_connect_error(error: &std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::ConnectionRefused => Self::Refused,
            std::io::ErrorKind::TimedOut => Self::TimedOut { seconds: 0 },
            _ => Self::Other(error.to_string()),
        }
    }

    /// Message compréhensible, porté par `AppError::Network`.
    pub fn into_error(self, host: &str, port: u16) -> AppError {
        let message = match self {
            Self::HostNotFound => format!("Nom d'hôte introuvable : {host}."),
            Self::Refused => format!(
                "Connexion refusée par {host}:{port}. Aucun service SSH n'écoute sur ce port."
            ),
            Self::TimedOut { seconds: 0 } => format!("Aucune réponse de {host}:{port}."),
            Self::TimedOut { seconds } => {
                format!("Aucune réponse de {host}:{port} après {seconds} s.")
            }
            Self::Other(detail) => format!("Impossible de joindre {host}:{port} : {detail}."),
        };
        AppError::network(message)
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, NetworkFailure};
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

    #[test]
    fn passphrase_and_cancel_variants_serialize_to_kind_only() {
        assert_eq!(
            to_value(AppError::PassphraseIncorrect).unwrap(),
            json!({ "kind": "PassphraseIncorrect" })
        );
        assert_eq!(
            to_value(AppError::Cancelled).unwrap(),
            json!({ "kind": "Cancelled" })
        );
    }

    #[test]
    fn file_errors_serialize_their_path_and_side() {
        use super::FileSide;
        assert_eq!(
            to_value(AppError::permission_denied("/root", FileSide::Remote)).unwrap(),
            json!({ "kind": "PermissionDenied", "path": "/root", "side": "remote" })
        );
        assert_eq!(
            to_value(AppError::already_exists("/srv/app", true)).unwrap(),
            json!({ "kind": "AlreadyExists", "path": "/srv/app", "isDir": true })
        );
        assert_eq!(
            to_value(AppError::InsufficientSpace {
                path: "C:\\dl".into(),
                side: FileSide::Local,
                needed: Some(10),
                available: None,
            })
            .unwrap(),
            json!({
                "kind": "InsufficientSpace",
                "path": "C:\\dl",
                "side": "local",
                "needed": 10,
                "available": null,
            })
        );
    }

    #[test]
    fn service_step_failure_serializes_its_cause() {
        use super::ServiceStep;
        let error = AppError::ServiceStepFailed {
            unit: "bot.service".into(),
            step: ServiceStep::RemoveDropIn,
            state: "Service arrêté et désactivé.".into(),
            cause: Box::new(AppError::Remote {
                exit_code: 1,
                stderr: "boom".into(),
            }),
        };
        assert_eq!(
            to_value(error).unwrap(),
            json!({
                "kind": "ServiceStepFailed",
                "unit": "bot.service",
                "step": "removeDropIn",
                "state": "Service arrêté et désactivé.",
                "cause": { "kind": "Remote", "exitCode": 1, "stderr": "boom" },
            })
        );
    }

    #[test]
    fn network_failures_become_readable_network_errors() {
        let message = |failure: NetworkFailure| match failure.into_error("vps.example", 2222) {
            AppError::Network { message } => message,
            other => panic!("attendu Network, reçu {other:?}"),
        };

        assert_eq!(
            message(NetworkFailure::HostNotFound),
            "Nom d'hôte introuvable : vps.example."
        );
        assert!(
            message(NetworkFailure::Refused).starts_with("Connexion refusée par vps.example:2222.")
        );
        assert_eq!(
            message(NetworkFailure::TimedOut { seconds: 10 }),
            "Aucune réponse de vps.example:2222 après 10 s."
        );
    }

    #[test]
    fn connect_errors_are_classified_by_kind() {
        use std::io::{Error, ErrorKind};

        assert_eq!(
            NetworkFailure::from_connect_error(&Error::from(ErrorKind::ConnectionRefused)),
            NetworkFailure::Refused
        );
        assert!(matches!(
            NetworkFailure::from_connect_error(&Error::from(ErrorKind::TimedOut)),
            NetworkFailure::TimedOut { .. }
        ));
        assert!(matches!(
            NetworkFailure::from_connect_error(&Error::from(ErrorKind::PermissionDenied)),
            NetworkFailure::Other(_)
        ));
    }
}
