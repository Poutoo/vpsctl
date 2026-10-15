//! État partagé de l'application (`AppState`) : profils, liaison SSH active
//! (une seule à la fois) et empreintes présentées par les serveurs.
//!
//! Plus tard : registre des flux en cours (logs, exécutions de scripts),
//! chacun identifié par un id pour pouvoir être arrêté proprement.
//!
//! Ce module ne dépend pas de Tauri : la couche `commands` ne fait que
//! transmettre les appels.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::task::{AbortHandle, JoinHandle};
use ts_rs::TS;
use zeroize::Zeroizing;

use crate::config::{Profile, ProfileStore};
use crate::error::AppError;
use crate::secrets;
use crate::ssh::{self, host_key, ConnectParams, Connection, ServerInfo, StepSink};

/// Intervalle de mesure de la latence.
const LATENCY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionInfo {
    pub profile_id: String,
    /// Horodatage de la connexion, en millisecondes depuis l'epoch Unix.
    #[ts(type = "number")]
    pub connected_at: u64,
    /// Mesurée en chronométrant une commande vide toutes les 30 secondes.
    pub latency_ms: Option<u32>,
}

struct Active {
    connection: Arc<Connection>,
    profile_id: String,
    connected_at: u64,
    latency: Arc<Mutex<Option<u32>>>,
    latency_task: JoinHandle<()>,
}

impl Active {
    fn info(&self) -> ConnectionInfo {
        ConnectionInfo {
            profile_id: self.profile_id.clone(),
            connected_at: self.connected_at,
            latency_ms: self.latency.lock().ok().and_then(|l| *l),
        }
    }
}

enum Link {
    Idle,
    Connecting {
        attempt: u64,
        profile_id: String,
        abort: AbortHandle,
    },
    Connected(Active),
}

pub struct AppState {
    profiles: ProfileStore,
    /// Sérialise les lectures-modifications du fichier des profils.
    profiles_lock: Mutex<()>,
    link: tokio::sync::Mutex<Link>,
    attempts: AtomicU64,
    /// Dernière empreinte présentée par le serveur de chaque profil, lors
    /// d'une connexion refusée. `trust_host_key` n'accepte que celle-ci.
    presented: Mutex<HashMap<String, String>>,
}

impl AppState {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            profiles: ProfileStore::new(config_dir),
            profiles_lock: Mutex::new(()),
            link: tokio::sync::Mutex::new(Link::Idle),
            attempts: AtomicU64::new(0),
            presented: Mutex::new(HashMap::new()),
        }
    }

    fn with_profiles<T>(
        &self,
        f: impl FnOnce(&ProfileStore) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let _guard = self
            .profiles_lock
            .lock()
            .map_err(|_| AppError::io("accès aux profils impossible"))?;
        f(&self.profiles)
    }

    // --- Profils -----------------------------------------------------------

    pub fn list_profiles(&self) -> Result<Vec<Profile>, AppError> {
        self.with_profiles(|store| store.list())
    }

    pub fn save_profile(&self, profile: Profile) -> Result<Profile, AppError> {
        self.with_profiles(|store| store.save(profile))
    }

    /// Supprime un profil et sa passphrase. Refusé pour le profil connecté.
    pub async fn delete_profile(&self, id: &str) -> Result<(), AppError> {
        if self.linked_profile().await.as_deref() == Some(id) {
            return Err(AppError::invalid_input(
                "impossible de supprimer le profil de la liaison en cours",
            ));
        }
        self.with_profiles(|store| store.delete(id))?;
        if let Ok(mut presented) = self.presented.lock() {
            presented.remove(id);
        }
        let id = id.to_owned();
        blocking(move || secrets::delete_passphrase(&id)).await
    }

    /// Vérifie localement la passphrase puis la mémorise dans le trousseau.
    pub async fn set_key_passphrase(
        &self,
        profile_id: &str,
        passphrase: Zeroizing<String>,
    ) -> Result<(), AppError> {
        let profile = self.with_profiles(|store| store.get(profile_id))?;
        let key_path = key_path(&profile);
        let id = profile.id.clone();
        blocking(move || {
            ssh::verify_passphrase(&key_path, &passphrase)?;
            secrets::set_passphrase(&id, &passphrase)
        })
        .await
    }

    /// Mémorise l'empreinte, seulement si c'est celle que le serveur vient de
    /// présenter pour ce profil.
    pub fn trust_host_key(&self, profile_id: &str, fingerprint: &str) -> Result<(), AppError> {
        if !host_key::is_valid_fingerprint(fingerprint) {
            return Err(AppError::invalid_input("empreinte invalide"));
        }
        let mut presented = self
            .presented
            .lock()
            .map_err(|_| AppError::io("accès aux empreintes impossible"))?;
        if presented.get(profile_id).map(String::as_str) != Some(fingerprint) {
            return Err(AppError::invalid_input(
                "cette empreinte n'a pas été présentée par le serveur : relancez la connexion",
            ));
        }
        self.with_profiles(|store| store.set_fingerprint(profile_id, fingerprint))?;
        presented.remove(profile_id);
        Ok(())
    }

    // --- Liaison -------------------------------------------------------------

    async fn linked_profile(&self) -> Option<String> {
        match &*self.link.lock().await {
            Link::Idle => None,
            Link::Connecting { profile_id, .. } => Some(profile_id.clone()),
            Link::Connected(active) => Some(active.profile_id.clone()),
        }
    }

    /// Établit la liaison. Une liaison existante est d'abord fermée.
    pub async fn connect(
        &self,
        profile_id: &str,
        passphrase: Option<Zeroizing<String>>,
        on_step: StepSink,
    ) -> Result<ConnectionInfo, AppError> {
        let profile = self.with_profiles(|store| store.get(profile_id))?;
        self.disconnect().await;

        let passphrase = match passphrase {
            Some(passphrase) => Some(passphrase),
            None => {
                let id = profile.id.clone();
                // Trousseau indisponible : on se comporte comme sans passphrase.
                blocking(move || secrets::get_passphrase(&id))
                    .await
                    .unwrap_or(None)
            }
        };
        let params = ConnectParams {
            host: profile.host.clone(),
            port: profile.port,
            user: profile.user.clone(),
            key_path: key_path(&profile),
            expected_fingerprint: profile.host_key_fingerprint.clone(),
            passphrase,
        };

        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        let task = {
            let mut link = self.link.lock().await;
            let task = tokio::spawn(ssh::connect(params, on_step));
            *link = Link::Connecting {
                attempt,
                profile_id: profile.id.clone(),
                abort: task.abort_handle(),
            };
            task
        };
        let result = match task.await {
            Ok(result) => result,
            Err(_) => Err(AppError::Cancelled),
        };

        let mut link = self.link.lock().await;
        let still_ours = matches!(&*link, Link::Connecting { attempt: a, .. } if *a == attempt);
        if !still_ours {
            // Interrompue entre-temps : la liaison éventuelle est refermée.
            if let Ok(connection) = result {
                connection.close().await;
            }
            return Err(AppError::Cancelled);
        }

        match result {
            Ok(connection) => {
                let active = start_active(profile.id.clone(), connection);
                let info = active.info();
                *link = Link::Connected(active);
                Ok(info)
            }
            Err(error) => {
                *link = Link::Idle;
                let presented = match &error {
                    AppError::UnknownHostKey { fingerprint } => Some(fingerprint.clone()),
                    AppError::HostKeyMismatch { received, .. } => Some(received.clone()),
                    _ => None,
                };
                if let (Some(fingerprint), Ok(mut map)) = (presented, self.presented.lock()) {
                    map.insert(profile.id.clone(), fingerprint);
                }
                Err(error)
            }
        }
    }

    /// Ferme la liaison, ou interrompt une connexion en cours.
    pub async fn disconnect(&self) {
        let previous = std::mem::replace(&mut *self.link.lock().await, Link::Idle);
        match previous {
            Link::Idle => {}
            Link::Connecting { abort, .. } => abort.abort(),
            Link::Connected(active) => close_active(active).await,
        }
    }

    /// Liaison active, ou `None`. Une liaison tombée est oubliée.
    pub async fn connection_status(&self) -> Option<ConnectionInfo> {
        let mut link = self.link.lock().await;
        let closed = match &*link {
            Link::Connected(active) => active.connection.is_closed(),
            _ => return None,
        };
        if closed {
            if let Link::Connected(active) = std::mem::replace(&mut *link, Link::Idle) {
                close_active(active).await;
            }
            return None;
        }
        match &*link {
            Link::Connected(active) => Some(active.info()),
            _ => None,
        }
    }

    pub async fn server_info(&self) -> Result<ServerInfo, AppError> {
        let connection = match &*self.link.lock().await {
            Link::Connected(active) => active.connection.clone(),
            _ => return Err(AppError::NotConnected),
        };
        Ok(ssh::server_info(&connection).await)
    }
}

fn key_path(profile: &Profile) -> std::path::PathBuf {
    ssh::resolve_key_path(&profile.key_path, std::env::home_dir().as_deref())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Démarre la mesure périodique de la latence (une mesure immédiate).
fn start_active(profile_id: String, connection: Connection) -> Active {
    let connection = Arc::new(connection);
    let latency = Arc::new(Mutex::new(None));
    let latency_task = {
        let connection = connection.clone();
        let latency = latency.clone();
        tokio::spawn(async move {
            loop {
                let measured = connection.measure_latency().await.ok();
                if let Ok(mut slot) = latency.lock() {
                    *slot = measured;
                }
                tokio::time::sleep(LATENCY_INTERVAL).await;
            }
        })
    };
    Active {
        connection,
        profile_id,
        connected_at: now_millis(),
        latency,
        latency_task,
    }
}

async fn close_active(active: Active) {
    active.latency_task.abort();
    let _ = active.latency_task.await;
    if let Ok(connection) = Arc::try_unwrap(active.connection) {
        connection.close().await;
    }
    // Sinon, la dernière référence fermera la liaison à sa destruction.
}

/// Exécute un appel synchrone (trousseau, déchiffrement de clé) hors du fil
/// asynchrone.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| AppError::io("opération interrompue"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn connection_info_serializes_in_camel_case() {
        let info = ConnectionInfo {
            profile_id: "p1".into(),
            connected_at: 1_760_000_000_000,
            latency_ms: Some(42),
        };
        assert_eq!(
            serde_json::to_value(info).unwrap(),
            json!({ "profileId": "p1", "connectedAt": 1_760_000_000_000_u64, "latencyMs": 42 })
        );
    }

    fn state() -> (AppState, PathBuf) {
        let dir = std::env::temp_dir().join(format!("vpsctl-state-{}", uuid::Uuid::new_v4()));
        (AppState::new(&dir), dir)
    }

    use std::path::PathBuf;

    #[test]
    fn trust_host_key_only_accepts_the_presented_fingerprint() {
        let (state, dir) = state();
        let profile = state
            .save_profile(Profile {
                id: String::new(),
                name: "vps".into(),
                host: "vps.example".into(),
                port: 22,
                user: "deploy".into(),
                key_path: "~/.ssh/id_ed25519".into(),
                host_key_fingerprint: None,
                is_default: false,
            })
            .unwrap();
        let presented = include_str!("../tests/fixtures/host_ed25519.fingerprint");
        let other = include_str!("../tests/fixtures/host_rsa.fingerprint");

        // Rien n'a été présenté : refus.
        assert!(state.trust_host_key(&profile.id, presented).is_err());

        state
            .presented
            .lock()
            .unwrap()
            .insert(profile.id.clone(), presented.to_owned());

        // Format invalide ou autre empreinte : refus.
        assert!(state.trust_host_key(&profile.id, "SHA256:court").is_err());
        assert!(state.trust_host_key(&profile.id, other).is_err());

        // Empreinte présentée : acceptée une seule fois.
        state.trust_host_key(&profile.id, presented).unwrap();
        assert_eq!(
            state.list_profiles().unwrap()[0]
                .host_key_fingerprint
                .as_deref(),
            Some(presented)
        );
        assert!(state.trust_host_key(&profile.id, presented).is_err());

        let _ = std::fs::remove_dir_all(dir);
    }
}
