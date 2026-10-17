//! État partagé de l'application (`AppState`) : profils, scripts, liaison SSH
//! active (une seule à la fois), empreintes présentées par les serveurs,
//! registre des flux en cours (logs et exécutions de scripts) et registre des
//! transferts de fichiers, chacun identifié par un id pour pouvoir être
//! arrêté proprement.
//!
//! Ce module ne dépend pas de Tauri : la couche `commands` ne fait que
//! transmettre les appels.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::oneshot;
use tokio::task::{AbortHandle, JoinHandle, JoinSet};
use ts_rs::TS;
use zeroize::Zeroizing;

use crate::config::{Profile, ProfileStore};
use crate::error::AppError;
use crate::scripts::{Script, ScriptStore};
use crate::secrets;
use crate::sftp::fs::SftpFs;
use crate::sftp::transfer::{self, CancelSignal, ProgressSink};
use crate::ssh::{self, host_key, ConnectParams, Connection, Deadline, ServerInfo, StepSink};
use crate::stream::{self, EventSink, LineParser, LogSource};
use crate::{docker, systemd};

/// Intervalle de mesure de la latence.
const LATENCY_INTERVAL: Duration = Duration::from_secs(30);
/// Flux simultanés au maximum (logs et scripts). OpenSSH limite une
/// connexion à 10 canaux par défaut (`MaxSessions`) : il en reste pour la
/// session SFTP, la mesure de latence et les listes.
pub const MAX_STREAMS: usize = 4;
/// Délai d'ouverture d'un flux (canal et envoi de la commande).
const STREAM_OPEN_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai d'arrêt d'un flux : au-delà, sa tâche est abandonnée.
const STREAM_STOP_TIMEOUT: Duration = Duration::from_secs(3);
/// Délai d'annulation d'un transfert (nettoyage du temporaire compris) :
/// au-delà, sa tâche est abandonnée.
const TRANSFER_CANCEL_TIMEOUT: Duration = Duration::from_secs(5);
/// Délai de fermeture de la liaison à la sortie de l'application.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionInfo {
    pub profile_id: String,
    /// Horodatage de la connexion, en millisecondes depuis l'epoch Unix.
    #[ts(type = "number")]
    pub connected_at: u64,
    /// Mesurée toutes les 30 secondes en chronométrant l'ouverture d'un canal
    /// de session (sans commande), refermé aussitôt.
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

/// Flux ou transfert en cours : son signal d'arrêt et sa tâche.
struct StreamEntry {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

type StreamRegistry = Arc<Mutex<HashMap<String, StreamEntry>>>;

pub struct AppState {
    profiles: ProfileStore,
    /// Sérialise les lectures-modifications du fichier des profils.
    profiles_lock: Mutex<()>,
    /// Partagé avec les tâches d'exécution, qui enregistrent `lastRun`.
    scripts: Arc<ScriptStore>,
    link: tokio::sync::Mutex<Link>,
    attempts: AtomicU64,
    /// Dernière empreinte présentée par le serveur de chaque profil, lors
    /// d'une connexion refusée. `trust_host_key` n'accepte que celle-ci.
    presented: Mutex<HashMap<String, String>>,
    /// Flux en cours. Un flux terminé de lui-même s'en retire.
    streams: StreamRegistry,
    /// Sérialise les ouvertures de flux, pour respecter `MAX_STREAMS`.
    stream_start: tokio::sync::Mutex<()>,
    /// Transferts en cours. Un transfert terminé s'en retire.
    transfers: StreamRegistry,
}

impl AppState {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            profiles: ProfileStore::new(config_dir),
            profiles_lock: Mutex::new(()),
            scripts: Arc::new(ScriptStore::new(config_dir)),
            link: tokio::sync::Mutex::new(Link::Idle),
            attempts: AtomicU64::new(0),
            presented: Mutex::new(HashMap::new()),
            streams: Arc::new(Mutex::new(HashMap::new())),
            stream_start: tokio::sync::Mutex::new(()),
            transfers: Arc::new(Mutex::new(HashMap::new())),
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

    // --- Scripts -------------------------------------------------------------

    pub fn list_scripts(&self) -> Result<Vec<Script>, AppError> {
        self.scripts.list()
    }

    pub fn save_script(&self, script: Script) -> Result<Script, AppError> {
        self.scripts.save(script)
    }

    pub fn delete_script(&self, id: &str) -> Result<(), AppError> {
        self.scripts.delete(id)
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

    /// Ferme la liaison, ou interrompt une connexion en cours. Les flux et
    /// les transferts en cours sont d'abord arrêtés (les temporaires des
    /// transferts sont supprimés tant que la liaison est ouverte).
    pub async fn disconnect(&self) {
        tokio::join!(self.stop_all_streams(), self.cancel_all_transfers());
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
            drop(link);
            tokio::join!(self.stop_all_streams(), self.cancel_all_transfers());
            return None;
        }
        match &*link {
            Link::Connected(active) => Some(active.info()),
            _ => None,
        }
    }

    /// Liaison active, pour une opération distante. Le verrou de la liaison
    /// n'est pas gardé pendant l'opération.
    pub async fn connection(&self) -> Result<Arc<Connection>, AppError> {
        match &*self.link.lock().await {
            Link::Connected(active) => Ok(active.connection.clone()),
            _ => Err(AppError::NotConnected),
        }
    }

    pub async fn server_info(&self) -> Result<ServerInfo, AppError> {
        let connection = self.connection().await?;
        Ok(ssh::server_info(&connection).await)
    }

    /// Sortie de l'application : flux arrêtés et liaison fermée, en temps
    /// limité.
    pub async fn shutdown(&self) {
        let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, self.disconnect()).await;
    }

    // --- Flux ----------------------------------------------------------------

    /// Ouvre le flux de logs d'un conteneur ou d'un service. Les arguments
    /// sont validés et échappés par `docker` et `systemd`.
    pub async fn start_logs(
        &self,
        source: &LogSource,
        tail: u32,
        sink: EventSink,
    ) -> Result<String, AppError> {
        match source {
            LogSource::Docker(id) => {
                let command = docker::logs_command(id, tail)?;
                self.start_stream(&command, docker::DockerLogParser, sink)
                    .await
            }
            LogSource::Systemd(unit) => {
                let command = systemd::logs_command(unit, tail)?;
                self.start_stream(&command, systemd::JournalParser, sink)
                    .await
            }
        }
    }

    /// Lance `command` (enveloppée dans le script de surveillance) et lit sa
    /// sortie avec `parser`. Renvoie l'identifiant du flux.
    pub async fn start_stream(
        &self,
        command: &str,
        parser: impl LineParser,
        sink: EventSink,
    ) -> Result<String, AppError> {
        let _start = self.stream_start.lock().await;
        if self.stream_count() >= MAX_STREAMS {
            return Err(AppError::invalid_input(format!(
                "{MAX_STREAMS} flux au maximum : fermez-en un avant d'en ouvrir un autre"
            )));
        }
        let connection = self.connection().await?;
        let channel = connection
            .open_stream(
                &stream::guarded_command(command),
                Deadline::after(STREAM_OPEN_TIMEOUT),
            )
            .await?;

        let id = uuid::Uuid::new_v4().to_string();
        let (stop, stopped) = oneshot::channel();
        // Verrou tenu jusqu'à l'insertion : la tâche, qui se retire du
        // registre à sa fin, ne peut pas le faire avant d'y figurer.
        let mut streams = lock_streams(&self.streams);
        let task = {
            let registry = self.streams.clone();
            let id = id.clone();
            tokio::spawn(async move {
                stream::run(channel, parser, sink, stopped).await;
                lock_streams(&registry).remove(&id);
            })
        };
        streams.insert(id.clone(), StreamEntry { stop, task });
        Ok(id)
    }

    /// Arrête un flux et attend que la commande distante soit arrêtée (3 s
    /// au plus). Un identifiant inconnu (flux déjà terminé) est ignoré.
    pub async fn stop_stream(&self, id: &str) {
        let entry = lock_streams(&self.streams).remove(id);
        if let Some(entry) = entry {
            stop_entry(entry, STREAM_STOP_TIMEOUT).await;
        }
    }

    pub fn stream_count(&self) -> usize {
        lock_streams(&self.streams).len()
    }

    /// Arrête tous les flux en parallèle.
    pub async fn stop_all_streams(&self) {
        stop_all(&self.streams, STREAM_STOP_TIMEOUT).await;
    }

    // --- Transferts ----------------------------------------------------------

    /// Vérifie puis lance un envoi. Renvoie l'identifiant du transfert.
    pub async fn upload(
        &self,
        local: &str,
        remote: &str,
        overwrite: bool,
        sink: ProgressSink,
    ) -> Result<String, AppError> {
        let fs = SftpFs::shared(self.connection().await?);
        let upload = transfer::prepare_upload(&*fs, local, remote, overwrite).await?;
        Ok(self.spawn_transfer(move |cancel| transfer::run_upload(fs, upload, cancel, sink)))
    }

    /// Vérifie puis lance un téléchargement. Renvoie l'identifiant du transfert.
    pub async fn download(
        &self,
        remote: &str,
        local: &str,
        sink: ProgressSink,
    ) -> Result<String, AppError> {
        let fs = SftpFs::shared(self.connection().await?);
        let download = transfer::prepare_download(&*fs, remote, local).await?;
        Ok(self.spawn_transfer(move |cancel| transfer::run_download(fs, download, cancel, sink)))
    }

    fn spawn_transfer<Fut>(&self, start: impl FnOnce(CancelSignal) -> Fut) -> String
    where
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let id = uuid::Uuid::new_v4().to_string();
        let (stop, stopped) = oneshot::channel();
        // Verrou tenu jusqu'à l'insertion, comme pour les flux.
        let mut transfers = lock_streams(&self.transfers);
        let task = {
            let registry = self.transfers.clone();
            let id = id.clone();
            let run = start(stopped);
            tokio::spawn(async move {
                run.await;
                lock_streams(&registry).remove(&id);
            })
        };
        transfers.insert(id.clone(), StreamEntry { stop, task });
        id
    }

    /// Annule un transfert et attend la fin du nettoyage (5 s au plus). Un
    /// identifiant inconnu (transfert terminé) est ignoré.
    pub async fn cancel_transfer(&self, id: &str) {
        let entry = lock_streams(&self.transfers).remove(id);
        if let Some(entry) = entry {
            stop_entry(entry, TRANSFER_CANCEL_TIMEOUT).await;
        }
    }

    /// Annule tous les transferts en parallèle.
    pub async fn cancel_all_transfers(&self) {
        stop_all(&self.transfers, TRANSFER_CANCEL_TIMEOUT).await;
    }
}

/// Arrête toutes les tâches d'un registre en parallèle.
async fn stop_all(registry: &StreamRegistry, timeout: Duration) {
    let entries: Vec<StreamEntry> = lock_streams(registry)
        .drain()
        .map(|(_, entry)| entry)
        .collect();
    let mut stopping = JoinSet::new();
    for entry in entries {
        stopping.spawn(stop_entry(entry, timeout));
    }
    while stopping.join_next().await.is_some() {}
}

fn lock_streams(
    registry: &StreamRegistry,
) -> std::sync::MutexGuard<'_, HashMap<String, StreamEntry>> {
    // Un verrou empoisonné ne protège qu'une table : elle reste utilisable.
    registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Demande l'arrêt d'un flux ou d'un transfert et attend sa tâche ; au-delà
/// du délai, elle est abandonnée (pour un flux, le script de surveillance
/// termine alors la commande distante à la fermeture du canal).
async fn stop_entry(entry: StreamEntry, timeout: Duration) {
    let StreamEntry { stop, mut task } = entry;
    let _ = stop.send(());
    if tokio::time::timeout(timeout, &mut task).await.is_err() {
        task.abort();
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
