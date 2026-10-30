//! Présence Discord (Rich Presence), désactivée par défaut.
//!
//! Une tâche unique tient la connexion locale à Discord (`socket`) tant que
//! l'interrupteur est actif : poignée de main, puis une activité à chaque
//! changement de phase de la liaison (`activity`, seul contenu envoyé).
//! Discord absent ou fermé : nouvelle tentative silencieuse toutes les 15 s,
//! ou tout de suite sur « Réessayer ». Désactivation et fermeture de
//! l'application effacent l'activité.
//!
//! Limite de Discord : 5 mises à jour d'activité par 20 secondes
//! (documentation du Game SDK, `UpdateActivity`, et de l'objet Activity).
//! Les envois sont comptés sur une fenêtre glissante, effacements compris ;
//! une phase qui change plus vite est regroupée et seule la dernière part.
//! Seul l'effacement à la fermeture de l'application passe outre.
//!
//! Ce module ne dépend pas de Tauri : le statut part vers une fonction de
//! rappel, que `lib.rs` relie à l'événement `presence-status`.

mod activity;
mod frame;
mod socket;

pub use activity::{LinkPhase, PhaseState};

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite, WriteHalf};
use tokio::sync::{mpsc, oneshot, watch, Notify};
use tokio::time::{sleep, timeout, Instant};
use ts_rs::TS;

use activity::{handshake, set_activity};
use frame::{OP_CLOSE, OP_FRAME, OP_HANDSHAKE, OP_PING, OP_PONG};

/// Identifiant public de l'application sur le portail Discord (17 à 20
/// chiffres). Vide ou invalide : présence indisponible.
pub const CLIENT_ID: &str = "";

/// Nom de l'événement émis vers l'interface.
pub const STATUS_EVENT: &str = "presence-status";

const RETRY_INTERVAL: Duration = Duration::from_secs(15);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const RATE_WINDOW: Duration = Duration::from_secs(20);
const RATE_MAX: usize = 5;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PresenceState {
    /// Interrupteur éteint : rien n'est envoyé.
    Off,
    /// Interrupteur actif, Discord absent ou fermé : nouvelles tentatives.
    Searching,
    /// Activité publiée.
    Connected,
    /// Application lancée par `tauri dev` ou compilée en mode debug.
    DevUnavailable,
    /// `CLIENT_ID` absent de cette version.
    NotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresenceStatus {
    pub state: PresenceState,
    /// Phase de la liaison, pour l'aperçu de la carte dans l'interface.
    pub phase: PhaseState,
}

pub type StatusSink = Arc<dyn Fn(PresenceStatus) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available { client_id: String },
    DevMode,
    NotConfigured,
}

/// La présence n'existe que dans l'application installée (build release
/// hors `tauri dev`), avec un identifiant d'application valide.
pub fn availability(release: bool, client_id: &str) -> Availability {
    let valid =
        (17..=20).contains(&client_id.len()) && client_id.bytes().all(|b| b.is_ascii_digit());
    if !release {
        Availability::DevMode
    } else if !valid {
        Availability::NotConfigured
    } else {
        Availability::Available {
            client_id: client_id.to_owned(),
        }
    }
}

/// Ouverture de la connexion locale ; simulée dans les tests.
pub trait Connector: Send + Sync + 'static {
    type Stream: AsyncRead + AsyncWrite + Send + Unpin + 'static;
    fn connect(&self) -> impl Future<Output = io::Result<Self::Stream>> + Send;
}

/// Discord installé sur cet ordinateur.
pub struct LocalDiscord;

impl Connector for LocalDiscord {
    type Stream = socket::Stream;

    fn connect(&self) -> impl Future<Output = io::Result<Self::Stream>> + Send {
        socket::connect()
    }
}

/// Poignée de la tâche de présence, gardée dans l'état de l'application.
pub struct Presence {
    enabled: watch::Sender<bool>,
    retry: Arc<Notify>,
    status: Arc<Mutex<PresenceStatus>>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    done: tokio::sync::Mutex<Option<oneshot::Receiver<()>>>,
}

impl Presence {
    /// Crée la poignée et la tâche à lancer sur le runtime asynchrone.
    pub fn new<C: Connector>(
        availability: Availability,
        enabled: bool,
        phase: watch::Receiver<PhaseState>,
        connector: C,
        on_status: StatusSink,
    ) -> (Self, impl Future<Output = ()> + Send + 'static) {
        let initial = PresenceStatus {
            state: match availability {
                Availability::DevMode => PresenceState::DevUnavailable,
                Availability::NotConfigured => PresenceState::NotConfigured,
                Availability::Available { .. } => PresenceState::Off,
            },
            phase: *phase.borrow(),
        };
        let (enabled_tx, enabled_rx) = watch::channel(enabled);
        let (stop_tx, stop_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let retry = Arc::new(Notify::new());
        let status = Arc::new(Mutex::new(initial));
        let task = Task {
            enabled: enabled_rx,
            phase,
            retry: retry.clone(),
            stop: stop_rx,
            connector,
            publisher: Publisher {
                status: status.clone(),
                sink: on_status,
            },
            limiter: RateLimiter::default(),
        };
        let presence = Self {
            enabled: enabled_tx,
            retry,
            status,
            stop: Mutex::new(Some(stop_tx)),
            done: tokio::sync::Mutex::new(Some(done_rx)),
        };
        let future = async move {
            run(task, availability).await;
            let _ = done_tx.send(());
        };
        (presence, future)
    }

    pub fn status(&self) -> PresenceStatus {
        *self.status.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.send_replace(enabled);
    }

    /// Nouvelle tentative immédiate (bouton « Réessayer »).
    pub fn retry(&self) {
        self.retry.notify_one();
    }

    /// Fermeture de l'application : activité effacée, en 1 s au plus.
    pub async fn shutdown(&self) {
        let stop = self.stop.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(stop) = stop {
            let _ = stop.send(());
        }
        if let Some(done) = self.done.lock().await.take() {
            let _ = timeout(SHUTDOWN_TIMEOUT, done).await;
        }
    }
}

struct Publisher {
    status: Arc<Mutex<PresenceStatus>>,
    sink: StatusSink,
}

impl Publisher {
    fn publish(&self, state: PresenceState, phase: PhaseState) {
        let next = PresenceStatus { state, phase };
        let changed = {
            let mut current = self.status.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *current != next;
            *current = next;
            changed
        };
        if changed {
            (self.sink)(next);
        }
    }
}

/// Fenêtre glissante de `RATE_MAX` envois par `RATE_WINDOW`.
#[derive(Default)]
struct RateLimiter {
    sent: VecDeque<Instant>,
}

impl RateLimiter {
    fn delay(&mut self, now: Instant) -> Duration {
        while self
            .sent
            .front()
            .is_some_and(|t| now.duration_since(*t) >= RATE_WINDOW)
        {
            self.sent.pop_front();
        }
        match self.sent.front() {
            Some(oldest) if self.sent.len() >= RATE_MAX => (*oldest + RATE_WINDOW) - now,
            _ => Duration::ZERO,
        }
    }

    fn record(&mut self, now: Instant) {
        self.sent.push_back(now);
    }
}

struct Task<C> {
    enabled: watch::Receiver<bool>,
    phase: watch::Receiver<PhaseState>,
    retry: Arc<Notify>,
    stop: oneshot::Receiver<()>,
    connector: C,
    publisher: Publisher,
    limiter: RateLimiter,
}

impl<C> Task<C> {
    fn publish(&self, state: PresenceState) {
        self.publisher.publish(state, *self.phase.borrow());
    }
}

/// Fin d'une session avec Discord.
enum Ending {
    Disabled,
    Lost,
    Stop,
}

/// Attend un changement ; un émetteur disparu n'en produira plus aucun.
async fn changed<T>(rx: &mut watch::Receiver<T>) {
    if rx.changed().await.is_err() {
        std::future::pending::<()>().await;
    }
}

async fn run<C: Connector>(mut task: Task<C>, availability: Availability) {
    let client_id = match availability {
        Availability::Available { client_id } => client_id,
        Availability::DevMode => return idle(task, PresenceState::DevUnavailable).await,
        Availability::NotConfigured => return idle(task, PresenceState::NotConfigured).await,
    };
    loop {
        if !*task.enabled.borrow_and_update() {
            task.publish(PresenceState::Off);
            tokio::select! {
                _ = changed(&mut task.enabled) => {}
                _ = changed(&mut task.phase) => {}
                _ = &mut task.stop => return,
            }
            continue;
        }
        task.publish(PresenceState::Searching);
        let opened = tokio::select! {
            opened = timeout(HANDSHAKE_TIMEOUT, open(&task.connector, &client_id)) => opened.ok().and_then(Result::ok),
            _ = &mut task.stop => return,
        };
        let Some(stream) = opened else {
            tokio::select! {
                _ = sleep(RETRY_INTERVAL) => {}
                _ = task.retry.notified() => {}
                _ = changed(&mut task.enabled) => {}
                _ = changed(&mut task.phase) => {}
                _ = &mut task.stop => return,
            }
            continue;
        };
        if let Ending::Stop = session(&mut task, stream).await {
            return;
        }
    }
}

/// Présence indisponible : seul le statut (et sa phase) est tenu à jour.
async fn idle<C>(mut task: Task<C>, state: PresenceState) {
    loop {
        task.publish(state);
        tokio::select! {
            _ = changed(&mut task.phase) => {}
            _ = &mut task.stop => return,
        }
    }
}

/// Connexion et poignée de main : Discord répond par l'événement READY.
async fn open<C: Connector>(connector: &C, client_id: &str) -> io::Result<C::Stream> {
    let mut stream = connector.connect().await?;
    frame::write(&mut stream, OP_HANDSHAKE, &handshake(client_id)).await?;
    loop {
        match frame::read(&mut stream).await? {
            (OP_FRAME, payload) if payload["evt"] == "READY" => return Ok(stream),
            (OP_CLOSE, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "connexion refusée par Discord",
                ))
            }
            (OP_PING, payload) => frame::write(&mut stream, OP_PONG, &payload).await?,
            _ => {}
        }
    }
}

/// Session établie : publie l'activité à chaque changement de phase, dans
/// la limite de fréquence, jusqu'à la désactivation, la perte de Discord ou
/// la fermeture de l'application.
async fn session<C: Connector>(task: &mut Task<C>, stream: C::Stream) -> Ending {
    let (mut reader, mut writer) = tokio::io::split(stream);
    // Lecture dans une tâche à part : une trame lue à moitié n'est jamais
    // perdue quand une autre branche de `select!` l'emporte.
    let (frames_tx, mut frames) = mpsc::channel(8);
    let reader_task = tokio::spawn(async move {
        while let Ok(frame) = frame::read(&mut reader).await {
            if frames_tx.send(frame).await.is_err() {
                break;
            }
        }
    });

    task.publish(PresenceState::Connected);
    task.phase.borrow_and_update();
    let mut pending = true;
    let ending = loop {
        let delay = task.limiter.delay(Instant::now());
        tokio::select! {
            _ = changed(&mut task.enabled) => {
                if !*task.enabled.borrow_and_update() {
                    break Ending::Disabled;
                }
            }
            _ = changed(&mut task.phase) => {
                task.phase.borrow_and_update();
                task.publish(PresenceState::Connected);
                pending = true;
            }
            incoming = frames.recv() => match incoming {
                Some((OP_PING, payload)) => {
                    if frame::write(&mut writer, OP_PONG, &payload).await.is_err() {
                        break Ending::Lost;
                    }
                }
                Some((OP_CLOSE, _)) | None => break Ending::Lost,
                Some(_) => {}
            },
            _ = sleep(delay), if pending => {
                let current = activity::activity(*task.phase.borrow());
                if send_activity(&mut writer, Some(current)).await.is_err() {
                    break Ending::Lost;
                }
                task.limiter.record(Instant::now());
                pending = false;
            }
            _ = &mut task.stop => break Ending::Stop,
        }
    };
    reader_task.abort();

    match ending {
        Ending::Disabled => {
            let delay = task.limiter.delay(Instant::now());
            let stopped = tokio::select! {
                _ = sleep(delay) => false,
                _ = &mut task.stop => true,
            };
            let _ = send_activity(&mut writer, None).await;
            task.limiter.record(Instant::now());
            let _ = frame::write(&mut writer, OP_CLOSE, &json!({})).await;
            if stopped {
                return Ending::Stop;
            }
        }
        Ending::Stop => {
            let _ = send_activity(&mut writer, None).await;
            let _ = frame::write(&mut writer, OP_CLOSE, &json!({})).await;
        }
        Ending::Lost => {}
    }
    ending
}

async fn send_activity<S: AsyncWrite>(
    writer: &mut WriteHalf<S>,
    current: Option<Value>,
) -> io::Result<()> {
    let nonce = uuid::Uuid::new_v4().to_string();
    let payload = set_activity(std::process::id(), &nonce, current);
    frame::write(writer, OP_FRAME, &payload).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::DuplexStream;

    const ID: &str = "123456789012345678";

    #[derive(Clone, Default)]
    struct FakeDiscord {
        /// Côtés « application » des connexions que Discord acceptera.
        streams: Arc<Mutex<VecDeque<DuplexStream>>>,
        attempts: Arc<AtomicUsize>,
    }

    impl FakeDiscord {
        /// Discord lancé : la prochaine tentative réussit. Renvoie le côté
        /// Discord de la connexion.
        fn launch(&self) -> DuplexStream {
            let (app, discord) = tokio::io::duplex(64 * 1024);
            self.streams.lock().unwrap().push_back(app);
            discord
        }

        fn attempts(&self) -> usize {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    impl Connector for FakeDiscord {
        type Stream = DuplexStream;

        fn connect(&self) -> impl Future<Output = io::Result<DuplexStream>> + Send {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            let next = self.streams.lock().unwrap().pop_front();
            async move { next.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "absent")) }
        }
    }

    struct Harness {
        presence: Presence,
        discord: FakeDiscord,
        phase: watch::Sender<PhaseState>,
        statuses: Arc<Mutex<Vec<PresenceStatus>>>,
    }

    fn ground(since: u64) -> PhaseState {
        PhaseState {
            phase: LinkPhase::Ground,
            since,
        }
    }

    fn linked(since: u64) -> PhaseState {
        PhaseState {
            phase: LinkPhase::Linked,
            since,
        }
    }

    fn start(availability: Availability, enabled: bool) -> Harness {
        let discord = FakeDiscord::default();
        let (phase, phase_rx) = watch::channel(ground(1_000_000));
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let sink_statuses = statuses.clone();
        let (presence, future) = Presence::new(
            availability,
            enabled,
            phase_rx,
            discord.clone(),
            Arc::new(move |s| sink_statuses.lock().unwrap().push(s)),
        );
        tokio::spawn(future);
        Harness {
            presence,
            discord,
            phase,
            statuses,
        }
    }

    fn available() -> Availability {
        Availability::Available {
            client_id: ID.into(),
        }
    }

    /// Laisse tourner les tâches (temps virtuel) jusqu'à la condition.
    async fn until(mut condition: impl FnMut() -> bool) {
        for _ in 0..10_000 {
            if condition() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("condition jamais remplie");
    }

    /// Côté Discord : lit la poignée de main et répond READY.
    async fn accept(discord: &mut DuplexStream) -> Value {
        let (op, payload) = frame::read(discord).await.unwrap();
        assert_eq!(op, OP_HANDSHAKE);
        frame::write(
            discord,
            OP_FRAME,
            &json!({"cmd": "DISPATCH", "evt": "READY"}),
        )
        .await
        .unwrap();
        payload
    }

    /// Côté Discord : prochaine activité reçue (`null` pour un effacement).
    async fn next_activity(discord: &mut DuplexStream) -> Value {
        loop {
            let (op, payload) = frame::read(discord).await.unwrap();
            if op == OP_FRAME && payload["cmd"] == "SET_ACTIVITY" {
                return payload["args"]["activity"].clone();
            }
        }
    }

    #[test]
    fn available_only_in_release_with_a_valid_id() {
        assert_eq!(availability(false, ID), Availability::DevMode);
        assert_eq!(availability(true, ""), Availability::NotConfigured);
        assert_eq!(availability(true, "12345"), Availability::NotConfigured);
        assert_eq!(
            availability(true, "12345678901234567a"),
            Availability::NotConfigured
        );
        assert_eq!(availability(true, ID), available());
    }

    #[tokio::test(start_paused = true)]
    async fn off_by_default_never_contacts_discord() {
        let h = start(available(), false);
        sleep(Duration::from_secs(120)).await;
        assert_eq!(h.discord.attempts(), 0);
        assert_eq!(h.presence.status().state, PresenceState::Off);
    }

    #[tokio::test(start_paused = true)]
    async fn development_build_never_contacts_discord() {
        let h = start(Availability::DevMode, true);
        h.presence.set_enabled(true);
        h.presence.retry();
        sleep(Duration::from_secs(60)).await;
        assert_eq!(h.discord.attempts(), 0);
        assert_eq!(h.presence.status().state, PresenceState::DevUnavailable);
        h.phase.send_replace(linked(5_000));
        until(|| h.presence.status().phase.phase == LinkPhase::Linked).await;
    }

    #[tokio::test(start_paused = true)]
    async fn publishes_the_phase_and_follows_the_link() {
        let h = start(available(), false);
        let mut discord = h.discord.launch();
        h.presence.set_enabled(true);
        assert_eq!(accept(&mut discord).await, json!({"v": 1, "client_id": ID}));
        let first = next_activity(&mut discord).await;
        assert_eq!(first["details"], "Au sol");
        assert_eq!(first["timestamps"]["start"], 1_000);
        until(|| h.presence.status().state == PresenceState::Connected).await;

        h.phase.send_replace(linked(2_000_000));
        let second = next_activity(&mut discord).await;
        assert_eq!(second["details"], "En liaison");
        assert_eq!(second["timestamps"]["start"], 2_000);
        assert_eq!(h.presence.status().phase, linked(2_000_000));
    }

    #[tokio::test(start_paused = true)]
    async fn never_more_than_5_updates_in_20_seconds() {
        let h = start(available(), true);
        let mut discord = h.discord.launch();
        accept(&mut discord).await;
        next_activity(&mut discord).await;
        let begin = Instant::now();
        let mut sent = vec![begin];
        for i in 0..9u64 {
            let phase = if i % 2 == 0 {
                linked(i * 1000)
            } else {
                ground(i * 1000)
            };
            h.phase.send_replace(phase);
            sleep(Duration::from_millis(100)).await;
        }
        // La dernière phase finit par partir ; aucune fenêtre de 20 s ne
        // contient plus de 5 envois.
        loop {
            let activity = next_activity(&mut discord).await;
            sent.push(Instant::now());
            if activity["timestamps"]["start"] == 8 {
                break;
            }
        }
        for (i, t) in sent.iter().enumerate() {
            let in_window = sent[i..]
                .iter()
                .filter(|u| u.duration_since(*t) < RATE_WINDOW)
                .count();
            assert!(in_window <= RATE_MAX, "{in_window} envois en 20 s");
        }
        assert!(sent.len() < 10, "les changements rapides sont regroupés");
    }

    #[tokio::test(start_paused = true)]
    async fn disabling_clears_the_activity_then_closes() {
        let h = start(available(), true);
        let mut discord = h.discord.launch();
        accept(&mut discord).await;
        next_activity(&mut discord).await;
        h.presence.set_enabled(false);
        assert_eq!(next_activity(&mut discord).await, Value::Null);
        let (op, _) = frame::read(&mut discord).await.unwrap();
        assert_eq!(op, OP_CLOSE);
        until(|| h.presence.status().state == PresenceState::Off).await;
    }

    #[tokio::test(start_paused = true)]
    async fn absent_discord_is_retried_silently_every_15_seconds() {
        let h = start(available(), true);
        until(|| h.discord.attempts() == 1).await;
        assert_eq!(h.presence.status().state, PresenceState::Searching);
        sleep(Duration::from_secs(10)).await;
        assert_eq!(h.discord.attempts(), 1);
        sleep(Duration::from_secs(6)).await;
        assert_eq!(h.discord.attempts(), 2);

        // « Réessayer » : tentative immédiate, qui réussit si Discord est lancé.
        let mut discord = h.discord.launch();
        h.presence.retry();
        accept(&mut discord).await;
        assert_eq!(h.discord.attempts(), 3);
        next_activity(&mut discord).await;
        until(|| h.presence.status().state == PresenceState::Connected).await;
        let states: Vec<_> = h.statuses.lock().unwrap().iter().map(|s| s.state).collect();
        assert_eq!(states, [PresenceState::Searching, PresenceState::Connected]);
    }

    #[tokio::test(start_paused = true)]
    async fn reconnects_after_discord_is_closed() {
        let h = start(available(), true);
        let mut discord = h.discord.launch();
        accept(&mut discord).await;
        next_activity(&mut discord).await;
        drop(discord);
        until(|| h.presence.status().state == PresenceState::Searching).await;

        let mut relaunched = h.discord.launch();
        sleep(RETRY_INTERVAL).await;
        accept(&mut relaunched).await;
        assert_eq!(next_activity(&mut relaunched).await["details"], "Au sol");
    }

    #[tokio::test(start_paused = true)]
    async fn handshake_refused_counts_as_absent() {
        let h = start(available(), true);
        let mut discord = h.discord.launch();
        frame::read(&mut discord).await.unwrap();
        frame::write(&mut discord, OP_CLOSE, &json!({"code": 4000}))
            .await
            .unwrap();
        until(|| h.discord.attempts() == 1).await;
        sleep(Duration::from_millis(50)).await;
        assert_eq!(h.presence.status().state, PresenceState::Searching);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_clears_the_activity_at_once() {
        let h = start(available(), true);
        let mut discord = h.discord.launch();
        accept(&mut discord).await;
        next_activity(&mut discord).await;
        // Limite atteinte : l'effacement de fermeture part quand même.
        for i in 0..6u64 {
            h.phase
                .send_replace(if i % 2 == 0 { linked(i) } else { ground(i) });
            sleep(Duration::from_millis(10)).await;
        }
        let shutdown = tokio::spawn(async move {
            h.presence.shutdown().await;
            h
        });
        loop {
            if next_activity(&mut discord).await == Value::Null {
                break;
            }
        }
        shutdown.await.unwrap();
    }
}
