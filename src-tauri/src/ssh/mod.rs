//! Connexion SSH unique (russh) partagée par toute l'application.
//!
//! Expose deux primitives sur cette connexion : exécuter une commande et
//! récupérer sa sortie (`Connection::exec`), et plus tard ouvrir un canal en
//! streaming. Chaque opération ouvre son propre canal. La session SFTP, elle,
//! est partagée (`Connection::sftp`).
//!
//! Séquence de `connect`, émise étape par étape (`ConnectStep`) :
//! tcp → ssh (négociation) → hostKey → auth → aos.
//! Clé hôte inconnue ou différente : la liaison est fermée avant toute
//! authentification et `connect` échoue (`UnknownHostKey` / `HostKeyMismatch`).

pub mod host_key;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use russh::client::{self, AuthResult};
use russh::keys::{PrivateKey, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use russh_sftp::client::SftpSession;
use serde::Serialize;
use tokio::net::TcpStream;
use ts_rs::TS;
use zeroize::Zeroizing;

use crate::error::{AppError, NetworkFailure};
use host_key::HostKeyCheck;

/// Délai maximal pour résoudre le nom et établir la liaison TCP.
pub const TCP_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai maximal pour la négociation SSH (identification et échange de clés).
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
/// Délai maximal pour l'authentification.
const AUTH_TIMEOUT: Duration = Duration::from_secs(30);
/// Délai maximal d'une commande courte ou d'une mesure de latence.
const EXEC_TIMEOUT: Duration = Duration::from_secs(15);
/// Taille maximale conservée pour la sortie d'une commande courte.
const EXEC_OUTPUT_LIMIT: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// Étapes de connexion (contrat : ConnectStep)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HostKeyStatus {
    Ok,
    Unknown,
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuthStatus {
    Ok,
    PassphraseRequired,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ConnectStep {
    Tcp {
        status: StepStatus,
        duration_ms: Option<u32>,
        message: Option<String>,
    },
    Ssh {
        status: StepStatus,
        /// Algorithme d'échange de clés négocié (ex. `curve25519-sha256`).
        kex: Option<String>,
        /// Chiffrement négocié (ex. `chacha20-poly1305@openssh.com`).
        cipher: Option<String>,
        message: Option<String>,
    },
    HostKey {
        status: HostKeyStatus,
        fingerprint: String,
    },
    Auth {
        status: AuthStatus,
        message: Option<String>,
    },
    Aos,
}

/// Destinataire des étapes (le canal Tauri, côté commandes).
pub type StepSink = Arc<dyn Fn(ConnectStep) + Send + Sync>;

// ---------------------------------------------------------------------------
// Paramètres et chargement de la clé privée
// ---------------------------------------------------------------------------

pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: PathBuf,
    pub expected_fingerprint: Option<String>,
    pub passphrase: Option<Zeroizing<String>>,
}

/// Développe `~` en dossier personnel (`~`, `~/…`, `~\…`).
pub fn resolve_key_path(raw: &str, home: Option<&Path>) -> PathBuf {
    let rest = if raw == "~" {
        Some("")
    } else {
        raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
    };
    match (rest, home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(raw),
    }
}

/// Charge la clé privée. Opération locale et potentiellement lente (dérivation
/// de la passphrase) : à exécuter hors du fil asynchrone.
pub fn load_private_key(path: &Path, passphrase: Option<&str>) -> Result<PrivateKey, AppError> {
    use russh::keys::ssh_key;
    use russh::keys::Error as KeyError;

    russh::keys::load_secret_key(path, passphrase).map_err(|error| match error {
        KeyError::KeyIsEncrypted => AppError::PassphraseRequired,
        KeyError::IO(e) if e.kind() == std::io::ErrorKind::NotFound => {
            AppError::io(format!("Clé privée introuvable : {}.", path.display()))
        }
        KeyError::IO(e) => AppError::io(format!(
            "Lecture de la clé privée {} impossible : {e}.",
            path.display()
        )),
        // Déchiffrement raté : passphrase incorrecte.
        KeyError::SshKey(ssh_key::Error::Crypto)
        | KeyError::KeyIsCorrupt
        | KeyError::Pad(_)
        | KeyError::Unpad(_)
            if passphrase.is_some() =>
        {
            AppError::PassphraseIncorrect
        }
        other => AppError::io(format!(
            "Clé privée {} illisible : {other}.",
            path.display()
        )),
    })
}

/// Vérifie localement qu'une passphrase déchiffre bien la clé, sans réseau.
pub fn verify_passphrase(path: &Path, passphrase: &str) -> Result<(), AppError> {
    match load_private_key(path, None) {
        Ok(_) => Err(AppError::invalid_input(
            "cette clé privée n'est pas chiffrée : aucune passphrase n'est nécessaire",
        )),
        Err(AppError::PassphraseRequired) => load_private_key(path, Some(passphrase)).map(|_| ()),
        Err(other) => Err(other),
    }
}

// ---------------------------------------------------------------------------
// Handler russh : algorithmes négociés et vérification de la clé hôte
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum HostKeyOutcome {
    Trusted,
    Unknown(String),
    Mismatch { expected: String, received: String },
    Certificate,
    Cancelled,
}

#[derive(Default)]
struct Progress {
    ssh_reported: bool,
    host_key: Option<HostKeyOutcome>,
}

struct Handler {
    expected: Option<String>,
    on_step: StepSink,
    progress: Arc<Mutex<Progress>>,
    cancelled: Arc<AtomicBool>,
}

impl Handler {
    fn emit(&self, step: ConnectStep) {
        if !self.cancelled.load(Ordering::SeqCst) {
            (self.on_step)(step);
        }
    }

    fn record(&self, outcome: HostKeyOutcome) {
        if let Ok(mut progress) = self.progress.lock() {
            progress.host_key = Some(outcome);
        }
    }
}

impl client::Handler for Handler {
    type Error = russh::Error;

    async fn kex_done(
        &mut self,
        _shared_secret: Option<&[u8]>,
        names: &russh::Names,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // Appelé aussi à chaque renouvellement de clés : seule la première
        // négociation est une étape de connexion.
        let first = match self.progress.lock() {
            Ok(mut progress) => !std::mem::replace(&mut progress.ssh_reported, true),
            Err(_) => false,
        };
        if first {
            self.emit(ConnectStep::Ssh {
                status: StepStatus::Ok,
                kex: Some(names.kex.as_ref().to_owned()),
                cipher: Some(names.cipher.as_ref().to_owned()),
                message: None,
            });
        }
        Ok(())
    }

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        if self.cancelled.load(Ordering::SeqCst) {
            self.record(HostKeyOutcome::Cancelled);
            return Ok(false);
        }

        let key = match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key,
            // Seules des clés épinglées sont acceptées, pas des certificats.
            PublicKeyOrCertificate::Certificate(_) => {
                self.record(HostKeyOutcome::Certificate);
                return Ok(false);
            }
        };

        let received = host_key::fingerprint(key);
        let (status, outcome, accept) = match host_key::check(self.expected.as_deref(), &received) {
            HostKeyCheck::Trusted => (HostKeyStatus::Ok, HostKeyOutcome::Trusted, true),
            HostKeyCheck::Unknown => (
                HostKeyStatus::Unknown,
                HostKeyOutcome::Unknown(received.clone()),
                false,
            ),
            HostKeyCheck::Mismatch { expected } => (
                HostKeyStatus::Mismatch,
                HostKeyOutcome::Mismatch {
                    expected,
                    received: received.clone(),
                },
                false,
            ),
        };
        self.record(outcome);
        self.emit(ConnectStep::HostKey {
            status,
            fingerprint: received,
        });
        Ok(accept)
    }
}

// ---------------------------------------------------------------------------
// Connexion établie
// ---------------------------------------------------------------------------

/// Sortie d'une commande exécutée sur un canal.
#[derive(Debug, Default)]
pub struct ExecOutput {
    pub exit_code: Option<u32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ExecOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

/// Liaison SSH authentifiée. À la destruction, la liaison est fermée
/// (russh ne la ferme pas seul quand le `Handle` est abandonné).
pub struct Connection {
    handle: Option<client::Handle<Handler>>,
    /// Session SFTP partagée, ouverte à la première utilisation et recréée
    /// seulement après sa fermeture.
    sftp: tokio::sync::Mutex<Option<Arc<SftpSession>>>,
}

impl Connection {
    fn handle(&self) -> Result<&client::Handle<Handler>, AppError> {
        self.handle.as_ref().ok_or(AppError::NotConnected)
    }

    pub fn is_closed(&self) -> bool {
        self.handle.as_ref().is_none_or(|h| h.is_closed())
    }

    /// Exécute une commande fixe (jamais construite à partir d'une saisie non
    /// validée) et renvoie sa sortie complète.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput, AppError> {
        let run = async {
            let handle = self.handle()?;
            let mut channel = handle
                .channel_open_session()
                .await
                .map_err(|e| AppError::network(format!("ouverture d'un canal impossible : {e}")))?;
            channel
                .exec(true, command)
                .await
                .map_err(|e| AppError::network(format!("envoi de la commande impossible : {e}")))?;

            let mut output = ExecOutput::default();
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => append_limited(&mut output.stdout, &data),
                    ChannelMsg::ExtendedData { data, ext: 1 } => {
                        append_limited(&mut output.stderr, &data)
                    }
                    ChannelMsg::ExitStatus { exit_status } => output.exit_code = Some(exit_status),
                    _ => {}
                }
            }
            Ok(output)
        };
        tokio::time::timeout(EXEC_TIMEOUT, run)
            .await
            .map_err(|_| AppError::network("la commande n'a pas répondu à temps"))?
    }

    /// Durée d'un aller-retour SSH : ouverture d'un canal de session jusqu'à
    /// sa confirmation par le serveur, sans exécuter de commande. Le canal
    /// est refermé aussitôt.
    pub async fn measure_latency(&self) -> Result<u32, AppError> {
        let measure = async {
            let handle = self.handle()?;
            let start = Instant::now();
            let channel = handle
                .channel_open_session()
                .await
                .map_err(|e| AppError::network(format!("mesure de latence impossible : {e}")))?;
            let elapsed = start.elapsed();
            let _ = channel.close().await;
            Ok(millis(elapsed))
        };
        tokio::time::timeout(EXEC_TIMEOUT, measure)
            .await
            .map_err(|_| AppError::network("le serveur n'a pas répondu à la mesure de latence"))?
    }

    /// Session SFTP de la liaison : la session existante, ou une nouvelle
    /// ouverte sur un canal dédié si aucune n'est ouverte.
    pub async fn sftp(&self) -> Result<Arc<SftpSession>, AppError> {
        let mut slot = self.sftp.lock().await;
        if let Some(session) = slot.as_ref() {
            return Ok(session.clone());
        }
        let open =
            async {
                let channel = self.handle()?.channel_open_session().await.map_err(|e| {
                    AppError::network(format!("ouverture d'un canal impossible : {e}"))
                })?;
                channel.request_subsystem(true, "sftp").await.map_err(|e| {
                    AppError::network(format!("sous-système SFTP indisponible : {e}"))
                })?;
                SftpSession::new(channel.into_stream()).await.map_err(|e| {
                    AppError::network(format!("ouverture de la session SFTP impossible : {e}"))
                })
            };
        let session = Arc::new(
            tokio::time::timeout(EXEC_TIMEOUT, open)
                .await
                .map_err(|_| {
                    AppError::network("la session SFTP n'a pas pu être ouverte à temps")
                })??,
        );
        *slot = Some(session.clone());
        Ok(session)
    }

    /// Oublie la session SFTP après une erreur de transport : la suivante
    /// sera recréée. Sans effet si une autre session l'a déjà remplacée.
    pub async fn discard_sftp(&self, session: &Arc<SftpSession>) {
        let mut slot = self.sftp.lock().await;
        if slot.as_ref().is_some_and(|s| Arc::ptr_eq(s, session)) {
            *slot = None;
        }
    }

    /// Ferme proprement la liaison.
    pub async fn close(mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.disconnect(Disconnect::ByApplication, "", "fr").await;
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = handle.disconnect(Disconnect::ByApplication, "", "fr").await;
                });
            }
        }
    }
}

/// Limite la durée d'une opération distante. Au-delà, elle est abandonnée et
/// l'erreur est `Network`.
pub async fn within<T>(
    limit: Duration,
    operation: impl Future<Output = Result<T, AppError>>,
) -> Result<T, AppError> {
    tokio::time::timeout(limit, operation).await.map_err(|_| {
        AppError::network(format!(
            "le serveur n'a pas répondu en {} s",
            limit.as_secs()
        ))
    })?
}

fn append_limited(buffer: &mut Vec<u8>, data: &[u8]) {
    let room = EXEC_OUTPUT_LIMIT.saturating_sub(buffer.len());
    buffer.extend_from_slice(&data[..data.len().min(room)]);
}

fn millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------
// Séquence de connexion
// ---------------------------------------------------------------------------

/// Passe le drapeau d'annulation à vrai si la connexion est abandonnée
/// (future détruite) avant d'aboutir.
struct CancelOnDrop {
    flag: Arc<AtomicBool>,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(true, Ordering::SeqCst);
        }
    }
}

/// Établit une liaison authentifiée en émettant chaque étape.
pub async fn connect(params: ConnectParams, on_step: StepSink) -> Result<Connection, AppError> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let mut guard = CancelOnDrop {
        flag: cancelled.clone(),
        armed: true,
    };
    let emit = {
        let on_step = on_step.clone();
        let cancelled = cancelled.clone();
        move |step: ConnectStep| {
            if !cancelled.load(Ordering::SeqCst) {
                on_step(step);
            }
        }
    };

    // 1. TCP
    let started = Instant::now();
    let stream = match tokio::time::timeout(TCP_TIMEOUT, open_tcp(&params.host, params.port)).await
    {
        Ok(Ok(stream)) => stream,
        result => {
            let failure = match result {
                Ok(Err(failure)) => failure,
                _ => NetworkFailure::TimedOut {
                    seconds: TCP_TIMEOUT.as_secs(),
                },
            };
            let error = failure.into_error(&params.host, params.port);
            emit(ConnectStep::Tcp {
                status: StepStatus::Error,
                duration_ms: None,
                message: Some(error_message(&error)),
            });
            return Err(error);
        }
    };
    let _ = stream.set_nodelay(true);
    emit(ConnectStep::Tcp {
        status: StepStatus::Ok,
        duration_ms: Some(millis(started.elapsed())),
        message: None,
    });

    // 2 et 3. Négociation SSH et clé hôte (étapes émises par le handler)
    let progress = Arc::new(Mutex::new(Progress::default()));
    let handler = Handler {
        expected: params.expected_fingerprint.clone(),
        on_step: on_step.clone(),
        progress: progress.clone(),
        cancelled: cancelled.clone(),
    };
    let config = Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    });
    let handshake = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        client::connect_stream(config, stream, handler),
    )
    .await;

    let handle = match handshake {
        Ok(Ok(handle)) => handle,
        other => {
            let (ssh_reported, outcome) = match progress.lock() {
                Ok(p) => (p.ssh_reported, p.host_key.clone()),
                Err(_) => (false, None),
            };
            return Err(match outcome {
                Some(HostKeyOutcome::Unknown(fingerprint)) => {
                    AppError::UnknownHostKey { fingerprint }
                }
                Some(HostKeyOutcome::Mismatch { expected, received }) => {
                    AppError::HostKeyMismatch { expected, received }
                }
                Some(HostKeyOutcome::Cancelled) => AppError::Cancelled,
                Some(HostKeyOutcome::Certificate) => {
                    let message = "Le serveur présente un certificat d'hôte : seules les clés \
                                   d'hôte épinglées sont acceptées.";
                    emit(ConnectStep::Ssh {
                        status: StepStatus::Error,
                        kex: None,
                        cipher: None,
                        message: Some(message.to_owned()),
                    });
                    AppError::network(message)
                }
                Some(HostKeyOutcome::Trusted) | None => {
                    let detail = match other {
                        Ok(Err(e)) => e.to_string(),
                        _ => "pas de réponse".to_owned(),
                    };
                    let error = AppError::network(format!(
                        "Échec de la négociation SSH avec {}:{} ({detail}). \
                         Le port est-il bien celui du serveur SSH ?",
                        params.host, params.port
                    ));
                    if !ssh_reported {
                        emit(ConnectStep::Ssh {
                            status: StepStatus::Error,
                            kex: None,
                            cipher: None,
                            message: Some(error_message(&error)),
                        });
                    }
                    error
                }
            });
        }
    };
    let mut connection = Connection {
        handle: Some(handle),
        sftp: tokio::sync::Mutex::new(None),
    };

    // 4. Authentification par clé
    let auth_step =
        |status: AuthStatus, message: Option<String>| ConnectStep::Auth { status, message };
    let key = {
        let path = params.key_path.clone();
        let passphrase = params.passphrase.clone();
        tokio::task::spawn_blocking(move || {
            load_private_key(&path, passphrase.as_deref().map(|p| p.as_str()))
        })
        .await
        .map_err(|_| AppError::io("chargement de la clé privée interrompu"))?
    };
    let key = match key {
        Ok(key) => key,
        Err(error) => {
            let step = match &error {
                AppError::PassphraseRequired => auth_step(AuthStatus::PassphraseRequired, None),
                other => auth_step(AuthStatus::Error, Some(error_message(other))),
            };
            emit(step);
            connection.close().await;
            return Err(error);
        }
    };

    let auth = async {
        let handle = connection.handle.as_mut().ok_or(AppError::NotConnected)?;
        let hash_alg = if key.algorithm().is_rsa() {
            handle
                .best_supported_rsa_hash()
                .await
                .map_err(|e| AppError::network(e.to_string()))?
                .flatten()
        } else {
            None
        };
        handle
            .authenticate_publickey(
                params.user.clone(),
                PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg),
            )
            .await
            .map_err(|e| AppError::network(format!("échec de l'authentification : {e}")))
    };
    let auth = match tokio::time::timeout(AUTH_TIMEOUT, auth).await {
        Ok(result) => result,
        Err(_) => Err(AppError::network(
            "le serveur n'a pas répondu à l'authentification",
        )),
    };
    match auth {
        Ok(AuthResult::Success) => emit(auth_step(AuthStatus::Ok, None)),
        Ok(AuthResult::Failure { .. }) => {
            emit(auth_step(
                AuthStatus::Error,
                Some(format!(
                    "Clé refusée par le serveur pour l'utilisateur « {} ».",
                    params.user
                )),
            ));
            connection.close().await;
            return Err(AppError::AuthFailed);
        }
        Err(error) => {
            emit(auth_step(AuthStatus::Error, Some(error_message(&error))));
            connection.close().await;
            return Err(error);
        }
    }

    // 5. Signal acquis
    emit(ConnectStep::Aos);
    guard.armed = false;
    Ok(connection)
}

/// Résolution du nom puis connexion TCP à la première adresse joignable.
async fn open_tcp(host: &str, port: u16) -> Result<TcpStream, NetworkFailure> {
    let bare_host = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<_> = tokio::net::lookup_host((bare_host, port))
        .await
        .map_err(|_| NetworkFailure::HostNotFound)?
        .collect();
    if addrs.is_empty() {
        return Err(NetworkFailure::HostNotFound);
    }
    let mut last = NetworkFailure::HostNotFound;
    for addr in addrs {
        match TcpStream::connect(addr).await {
            Ok(stream) => return Ok(stream),
            Err(e) => last = NetworkFailure::from_connect_error(&e),
        }
    }
    Err(last)
}

fn error_message(error: &AppError) -> String {
    match error {
        AppError::Network { message }
        | AppError::Io { message }
        | AppError::InvalidInput { message } => message.clone(),
        AppError::PassphraseIncorrect => "Passphrase incorrecte.".to_owned(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Informations serveur (contrat : ServerInfo)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ServerInfo {
    pub os: Option<String>,
    pub docker: Option<String>,
    pub systemd: Option<String>,
}

/// Lit le système, Docker et systemd. Une information indisponible vaut
/// `None` : l'échec d'une commande ne bloque jamais la connexion.
pub async fn server_info(connection: &Connection) -> ServerInfo {
    let (os, docker, systemd) = tokio::join!(
        connection.exec("cat /etc/os-release"),
        connection.exec("docker version --format '{{.Server.Version}}'"),
        connection.exec("systemctl --version"),
    );
    let ok_text = |result: Result<ExecOutput, AppError>| {
        result
            .ok()
            .filter(ExecOutput::success)
            .map(|o| o.stdout_text())
    };
    ServerInfo {
        os: ok_text(os).as_deref().and_then(parse_os_release),
        docker: ok_text(docker).as_deref().and_then(parse_docker_version),
        systemd: ok_text(systemd).as_deref().and_then(parse_systemd_version),
    }
}

/// `PRETTY_NAME` de `/etc/os-release`, sans guillemets.
pub fn parse_os_release(content: &str) -> Option<String> {
    let value = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("PRETTY_NAME="))?
        .trim();
    let unquoted = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);
    let pretty = unquoted.replace("\\\"", "\"");
    (!pretty.is_empty()).then_some(pretty)
}

/// Version du serveur Docker (`docker version --format '{{.Server.Version}}'`).
pub fn parse_docker_version(output: &str) -> Option<String> {
    let version = output.trim();
    let plausible = !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '~' | '_'));
    plausible.then(|| version.to_owned())
}

/// Numéro de version depuis la première ligne de `systemctl --version`
/// (ex. `systemd 255 (255.4-1ubuntu8.4)` → `255`).
pub fn parse_systemd_version(output: &str) -> Option<String> {
    let mut words = output.lines().next()?.split_whitespace();
    if words.next()? != "systemd" {
        return None;
    }
    let version = words.next()?;
    version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.')
        .then(|| version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::private::Ed25519Keypair;
    use russh::keys::ssh_key::{Cipher, LineEnding};
    use serde_json::json;

    // --- Étapes ---------------------------------------------------------

    #[test]
    fn connect_steps_serialize_per_contract() {
        let cases = [
            (
                ConnectStep::Tcp {
                    status: StepStatus::Ok,
                    duration_ms: Some(38),
                    message: None,
                },
                json!({ "step": "tcp", "status": "ok", "durationMs": 38, "message": null }),
            ),
            (
                ConnectStep::Ssh {
                    status: StepStatus::Ok,
                    kex: Some("curve25519-sha256".into()),
                    cipher: Some("chacha20-poly1305@openssh.com".into()),
                    message: None,
                },
                json!({
                    "step": "ssh", "status": "ok",
                    "kex": "curve25519-sha256", "cipher": "chacha20-poly1305@openssh.com",
                    "message": null,
                }),
            ),
            (
                ConnectStep::HostKey {
                    status: HostKeyStatus::Mismatch,
                    fingerprint: "SHA256:abc".into(),
                },
                json!({ "step": "hostKey", "status": "mismatch", "fingerprint": "SHA256:abc" }),
            ),
            (
                ConnectStep::Auth {
                    status: AuthStatus::PassphraseRequired,
                    message: None,
                },
                json!({ "step": "auth", "status": "passphraseRequired", "message": null }),
            ),
            (ConnectStep::Aos, json!({ "step": "aos" })),
        ];
        for (step, expected) in cases {
            assert_eq!(serde_json::to_value(&step).unwrap(), expected);
        }
    }

    // --- Chemin de clé -------------------------------------------------

    #[test]
    fn tilde_is_expanded_to_home() {
        let home = Path::new("/home/deploy");
        assert_eq!(
            resolve_key_path("~", Some(home)),
            PathBuf::from("/home/deploy")
        );
        assert_eq!(
            resolve_key_path("~/.ssh/id_ed25519", Some(home)),
            home.join(".ssh/id_ed25519")
        );
        assert_eq!(
            resolve_key_path("~\\.ssh\\id_ed25519", Some(home)),
            home.join(".ssh\\id_ed25519")
        );
        assert_eq!(
            resolve_key_path("/etc/key", Some(home)),
            PathBuf::from("/etc/key")
        );
        assert_eq!(
            resolve_key_path("~autre/key", Some(home)),
            PathBuf::from("~autre/key")
        );
        assert_eq!(resolve_key_path("~/key", None), PathBuf::from("~/key"));
    }

    // --- Chargement de clé (clés générées pendant le test) --------------

    struct TempKeys {
        dir: PathBuf,
    }

    impl TempKeys {
        /// Écrit une clé ed25519 en clair et la même clé chiffrée.
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("vpsctl-keys-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let key = PrivateKey::from(Ed25519Keypair::from_seed(&[7; 32]));
            let plain = key.to_openssh(LineEnding::LF).unwrap();
            let encrypted = key
                .encrypt_with(
                    Cipher::Aes256Ctr,
                    russh::keys::ssh_key::Kdf::Bcrypt {
                        salt: vec![1; 16],
                        rounds: 4,
                    },
                    0x5eed_5eed,
                    "bonne passphrase",
                )
                .unwrap()
                .to_openssh(LineEnding::LF)
                .unwrap();
            std::fs::write(dir.join("plain"), plain.as_bytes()).unwrap();
            std::fs::write(dir.join("encrypted"), encrypted.as_bytes()).unwrap();
            Self { dir }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.dir.join(name)
        }
    }

    impl Drop for TempKeys {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn plain_key_loads_without_passphrase() {
        let keys = TempKeys::new();
        assert!(load_private_key(&keys.path("plain"), None).is_ok());
    }

    #[test]
    fn encrypted_key_without_passphrase_requires_one() {
        let keys = TempKeys::new();
        assert!(matches!(
            load_private_key(&keys.path("encrypted"), None),
            Err(AppError::PassphraseRequired)
        ));
    }

    #[test]
    fn encrypted_key_with_wrong_passphrase_is_incorrect() {
        let keys = TempKeys::new();
        assert!(matches!(
            load_private_key(&keys.path("encrypted"), Some("mauvaise")),
            Err(AppError::PassphraseIncorrect)
        ));
    }

    #[test]
    fn encrypted_key_with_right_passphrase_loads() {
        let keys = TempKeys::new();
        let key = load_private_key(&keys.path("encrypted"), Some("bonne passphrase")).unwrap();
        let reference = load_private_key(&keys.path("plain"), None).unwrap();
        assert_eq!(key.public_key(), reference.public_key());
    }

    #[test]
    fn missing_key_is_an_io_error() {
        let keys = TempKeys::new();
        assert!(matches!(
            load_private_key(&keys.path("absente"), None),
            Err(AppError::Io { .. })
        ));
    }

    #[test]
    fn verify_passphrase_checks_locally() {
        let keys = TempKeys::new();
        assert!(verify_passphrase(&keys.path("encrypted"), "bonne passphrase").is_ok());
        assert!(matches!(
            verify_passphrase(&keys.path("encrypted"), "mauvaise"),
            Err(AppError::PassphraseIncorrect)
        ));
        assert!(matches!(
            verify_passphrase(&keys.path("plain"), "inutile"),
            Err(AppError::InvalidInput { .. })
        ));
    }

    // --- Informations serveur -----------------------------------------

    #[test]
    fn os_release_pretty_name_is_extracted() {
        let content = "NAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\nID=ubuntu\n";
        assert_eq!(
            parse_os_release(content).as_deref(),
            Some("Ubuntu 24.04.1 LTS")
        );
        assert_eq!(
            parse_os_release("PRETTY_NAME=Alpine").as_deref(),
            Some("Alpine")
        );
        assert_eq!(parse_os_release("NAME=x\n"), None);
        assert_eq!(parse_os_release("PRETTY_NAME=\"\"\n"), None);
    }

    #[test]
    fn docker_version_is_trimmed_and_checked() {
        assert_eq!(parse_docker_version("27.3.1\n").as_deref(), Some("27.3.1"));
        assert_eq!(parse_docker_version(""), None);
        assert_eq!(
            parse_docker_version("Cannot connect to the Docker daemon"),
            None
        );
    }

    #[test]
    fn systemd_version_is_the_second_word() {
        let output = "systemd 255 (255.4-1ubuntu8.4)\n+PAM +AUDIT +SELINUX\n";
        assert_eq!(parse_systemd_version(output).as_deref(), Some("255"));
        assert_eq!(parse_systemd_version("bash: systemctl: not found"), None);
        assert_eq!(parse_systemd_version(""), None);
    }
}
