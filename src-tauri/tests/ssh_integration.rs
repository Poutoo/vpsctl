//! Tests d'intégration SSH contre un vrai serveur. Ignorés par défaut (la CI
//! ne peut pas joindre le VPS) ; à lancer manuellement :
//!
//! ```text
//! VPSCTL_TEST_HOST=…            hôte du VPS
//! VPSCTL_TEST_PORT=22           facultatif
//! VPSCTL_TEST_USER=…            compte SSH
//! VPSCTL_TEST_KEY=…             chemin de la clé privée
//! VPSCTL_TEST_FINGERPRINT=…     sortie de ssh-keygen -lf (SHA256:…)
//! VPSCTL_TEST_PASSPHRASE=…      facultatif, si la clé est chiffrée
//! cargo test --test ssh_integration -- --ignored --test-threads=1
//! ```

use std::sync::{Arc, Mutex};

use vpsctl_lib::error::AppError;
use vpsctl_lib::ssh::{self, ConnectParams, ConnectStep};
use zeroize::Zeroizing;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("variable d'environnement {name} requise"))
}

fn params(expected_fingerprint: Option<String>) -> ConnectParams {
    let key = env("VPSCTL_TEST_KEY");
    ConnectParams {
        host: env("VPSCTL_TEST_HOST"),
        port: std::env::var("VPSCTL_TEST_PORT")
            .ok()
            .map(|p| p.parse().expect("VPSCTL_TEST_PORT invalide"))
            .unwrap_or(22),
        user: env("VPSCTL_TEST_USER"),
        key_path: ssh::resolve_key_path(&key, std::env::home_dir().as_deref()),
        expected_fingerprint,
        passphrase: std::env::var("VPSCTL_TEST_PASSPHRASE")
            .ok()
            .map(Zeroizing::new),
        timing: ssh::link::LinkTiming::default(),
    }
}

/// Lance `connect` en collectant les étapes émises.
async fn connect_collecting(
    params: ConnectParams,
) -> (Result<ssh::Connection, AppError>, Vec<ConnectStep>) {
    let steps = Arc::new(Mutex::new(Vec::new()));
    let sink = {
        let steps = steps.clone();
        Arc::new(move |step: ConnectStep| steps.lock().unwrap().push(step))
    };
    let result = ssh::connect(params, sink).await;
    let steps = steps.lock().unwrap().clone();
    (result, steps)
}

fn has_auth_step(steps: &[ConnectStep]) -> bool {
    steps.iter().any(|s| matches!(s, ConnectStep::Auth { .. }))
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH (variables VPSCTL_TEST_*)"]
async fn connects_with_pinned_fingerprint() {
    let (result, steps) = connect_collecting(params(Some(env("VPSCTL_TEST_FINGERPRINT")))).await;
    let connection = result.expect("connexion attendue");
    assert!(
        matches!(steps.last(), Some(ConnectStep::Aos)),
        "étapes : {steps:?}"
    );

    let output = connection.exec("true").await.unwrap();
    assert!(output.success());
    assert!(connection.measure_latency().await.is_ok());

    let info = ssh::server_info(&connection).await;
    assert!(info.os.is_some(), "système non détecté : {info:?}");
    connection.close().await;
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH (variables VPSCTL_TEST_*)"]
async fn unknown_host_key_stops_before_authentication() {
    let (result, steps) = connect_collecting(params(None)).await;
    match result {
        Err(AppError::UnknownHostKey { fingerprint }) => {
            assert_eq!(fingerprint, env("VPSCTL_TEST_FINGERPRINT"));
        }
        Err(other) => panic!("UnknownHostKey attendu, reçu {other:?}"),
        Ok(_) => panic!("UnknownHostKey attendu, connexion établie"),
    }
    assert!(
        !has_auth_step(&steps),
        "aucune authentification attendue : {steps:?}"
    );
}

#[tokio::test]
#[ignore = "nécessite un serveur SSH (variables VPSCTL_TEST_*)"]
async fn different_host_key_is_blocked_before_authentication() {
    let wrong = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned();
    let (result, steps) = connect_collecting(params(Some(wrong.clone()))).await;
    match result {
        Err(AppError::HostKeyMismatch { expected, received }) => {
            assert_eq!(expected, wrong);
            assert_eq!(received, env("VPSCTL_TEST_FINGERPRINT"));
        }
        Err(other) => panic!("HostKeyMismatch attendu, reçu {other:?}"),
        Ok(_) => panic!("HostKeyMismatch attendu, connexion établie"),
    }
    assert!(
        !has_auth_step(&steps),
        "aucune authentification attendue : {steps:?}"
    );
}
