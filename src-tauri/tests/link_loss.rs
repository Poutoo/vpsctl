//! Perte de la liaison contre un serveur SSH local (russh) placé derrière un
//! relais TCP qu'on peut couper : silence, fermeture, RST. Aucun réseau
//! extérieur. Délais de détection courts (keepalive 200 ms) mais marges
//! larges (10 s) pour rester stable en CI.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::ssh_key::LineEnding;
use russh::keys::PrivateKey;
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, Disconnect};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use vpsctl_lib::config::Profile;
use vpsctl_lib::error::AppError;
use vpsctl_lib::scripts::ScriptParser;
use vpsctl_lib::ssh::host_key;
use vpsctl_lib::ssh::link::{LinkTiming, LossCause};
use vpsctl_lib::state::{AppState, LinkLost};
use vpsctl_lib::stream::{EventSink, LogEvent};
use zeroize::Zeroizing;

const FAST: LinkTiming = LinkTiming {
    keepalive_interval: Duration::from_millis(200),
    keepalive_max: 3,
    inactivity_timeout: Duration::from_secs(1),
};
const MARGIN: Duration = Duration::from_secs(10);

// --- Serveur ----------------------------------------------------------------

#[derive(Clone)]
struct TestServer;

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        _: &str,
        _: &russh::keys::ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    /// Toute commande reçoit une ligne puis reste ouverte, comme un flux.
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, &b"ligne\n"[..])?;
        Ok(())
    }
}

async fn start_server(key: PrivateKey) -> (SocketAddr, mpsc::UnboundedReceiver<server::Handle>) {
    let config = Arc::new(server::Config {
        keys: vec![key],
        inactivity_timeout: None,
        auth_rejection_time: Duration::from_millis(10),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (handles, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let config = config.clone();
            let handles = handles.clone();
            tokio::spawn(async move {
                if let Ok(session) = server::run_stream(config, socket, TestServer).await {
                    let _ = handles.send(session.handle());
                    let _ = session.await;
                }
            });
        }
    });
    (addr, receiver)
}

// --- Relais -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cut {
    None,
    /// Plus rien ne passe, les connexions restent ouvertes. Les octets en
    /// attente restent dans les tampons TCP, comme lors d'une coupure réelle
    /// : ils passent si la liaison revient.
    Silent,
    /// Fermeture propre (FIN) côté client.
    Close,
    /// Réinitialisation (RST) côté client.
    Reset,
}

/// Relaie chaque connexion vers le serveur ; `cut` agit sur la dernière.
struct Relay {
    addr: SocketAddr,
    current: Arc<Mutex<Option<watch::Sender<Cut>>>>,
}

impl Relay {
    async fn start(server: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let current = Arc::new(Mutex::new(None));
        let slot = current.clone();
        tokio::spawn(async move {
            loop {
                let Ok((client, _)) = listener.accept().await else {
                    return;
                };
                let (cut, receiver) = watch::channel(Cut::None);
                *slot.lock().unwrap() = Some(cut);
                tokio::spawn(forward(client, server, receiver));
            }
        });
        Self { addr, current }
    }

    fn cut(&self, mode: Cut) {
        if let Some(cut) = self.current.lock().unwrap().as_ref() {
            let _ = cut.send(mode);
        }
    }
}

async fn forward(mut client: TcpStream, server: SocketAddr, mut cut: watch::Receiver<Cut>) {
    let Ok(mut server) = TcpStream::connect(server).await else {
        return;
    };
    let mut from_client = [0u8; 16 * 1024];
    let mut from_server = [0u8; 16 * 1024];
    let mut silent = false;
    // Demi-fermeture propagée comme sur un vrai réseau : la fin de flux d'un
    // côté ferme seulement le sens d'écriture vers l'autre, et le relais
    // continue de lire jusqu'à ce que l'autre ferme à son tour. Fermer les
    // deux sockets dès la première fin laisserait des octets non lus, donc un
    // RST, qui peut faire perdre le message de déconnexion du serveur.
    let mut client_done = false;
    let mut server_done = false;
    loop {
        tokio::select! {
            changed = cut.changed() => {
                if changed.is_err() {
                    return;
                }
                let mode = *cut.borrow();
                match mode {
                    Cut::None => silent = false,
                    Cut::Silent => silent = true,
                    Cut::Close => return close_cleanly(client).await,
                    Cut::Reset => {
                        let _ = client.set_zero_linger();
                        return;
                    }
                }
            }
            read = client.read(&mut from_client), if !silent && !client_done => match read {
                Ok(0) => {
                    let _ = server.shutdown().await;
                    client_done = true;
                    if server_done {
                        return;
                    }
                }
                Err(_) => return,
                Ok(n) => {
                    if server.write_all(&from_client[..n]).await.is_err() {
                        return;
                    }
                }
            },
            read = server.read(&mut from_server), if !silent && !server_done => match read {
                Ok(0) => {
                    let _ = client.shutdown().await;
                    server_done = true;
                    if client_done {
                        return;
                    }
                }
                Err(_) => return,
                Ok(n) => {
                    if client.write_all(&from_server[..n]).await.is_err() {
                        return;
                    }
                }
            },
        }
    }
}

/// FIN sans RST : un socket fermé avec des octets non lus enverrait un RST.
async fn close_cleanly(mut client: TcpStream) {
    let _ = client.shutdown().await;
    let mut sink = [0u8; 4096];
    let _ = tokio::time::timeout(MARGIN, async {
        while matches!(client.read(&mut sink).await, Ok(n) if n > 0) {}
    })
    .await;
}

// --- Banc de test -----------------------------------------------------------

struct Bench {
    state: AppState,
    profile_id: String,
    relay: Relay,
    lost: mpsc::UnboundedReceiver<LinkLost>,
    server_handles: mpsc::UnboundedReceiver<server::Handle>,
    dir: PathBuf,
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Bench {
    async fn new(timing: LinkTiming) -> Self {
        let host_key = PrivateKey::from(Ed25519Keypair::from_seed(&[3; 32]));
        let fingerprint = host_key::fingerprint(host_key.public_key());
        let (server, server_handles) = start_server(host_key).await;
        let relay = Relay::start(server).await;

        let dir = std::env::temp_dir().join(format!("vpsctl-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let client_key = PrivateKey::from(Ed25519Keypair::from_seed(&[9; 32]));
        let key_path = dir.join("id_ed25519");
        std::fs::write(&key_path, client_key.to_openssh(LineEnding::LF).unwrap()).unwrap();

        let (events, lost) = mpsc::unbounded_channel();
        let state = AppState::new(
            &dir,
            Arc::new(move |event| {
                let _ = events.send(event);
            }),
        )
        .with_link_timing(timing);
        let profile = state
            .save_profile(Profile {
                id: String::new(),
                name: "local".into(),
                host: "127.0.0.1".into(),
                port: relay.addr.port(),
                user: "deploy".into(),
                key_path: key_path.to_string_lossy().into_owned(),
                host_key_fingerprint: None,
                is_default: false,
            })
            .unwrap();

        let bench = Self {
            state,
            profile_id: profile.id,
            relay,
            lost,
            server_handles,
            dir,
        };
        match bench.try_connect().await {
            Err(AppError::UnknownHostKey { fingerprint: shown }) => assert_eq!(shown, fingerprint),
            other => panic!("UnknownHostKey attendu, reçu {other:?}"),
        }
        bench
            .state
            .trust_host_key(&bench.profile_id, &fingerprint)
            .unwrap();
        bench
    }

    /// Une passphrase fournie évite de lire le trousseau ; la clé, non
    /// chiffrée, l'ignore.
    async fn try_connect(&self) -> Result<(), AppError> {
        self.state
            .connect(
                &self.profile_id,
                Some(Zeroizing::new("inutile".into())),
                Arc::new(|_| {}),
            )
            .await
            .map(|_| ())
    }

    /// Se connecte et renvoie la session serveur de cette connexion.
    async fn connect(&mut self) -> server::Handle {
        while self.server_handles.try_recv().is_ok() {}
        self.try_connect().await.expect("connexion attendue");
        tokio::time::timeout(MARGIN, self.server_handles.recv())
            .await
            .expect("session serveur attendue")
            .expect("canal fermé")
    }

    async fn next_loss(&mut self) -> LinkLost {
        tokio::time::timeout(MARGIN, self.lost.recv())
            .await
            .expect("perte non détectée à temps")
            .expect("canal fermé")
    }

    async fn assert_no_loss_for(&mut self, duration: Duration) {
        if let Ok(event) = tokio::time::timeout(duration, self.lost.recv()).await {
            panic!("aucune perte attendue, reçu {event:?}");
        }
    }

    async fn assert_lost_state(&self) {
        assert!(self.state.connection_status().await.is_none());
        assert!(matches!(
            self.state.connection().await,
            Err(AppError::LinkLost)
        ));
    }
}

// --- Tests ------------------------------------------------------------------

#[tokio::test]
async fn silent_cut_is_detected_by_keepalive_once() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;
    bench.relay.cut(Cut::Silent);

    let loss = bench.next_loss().await;
    assert_eq!(loss.cause, LossCause::Timeout);
    assert_eq!(loss.profile_id, bench.profile_id);
    assert!(loss.last_received_at.is_some());
    bench.assert_lost_state().await;
    bench.assert_no_loss_for(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn closed_connection_is_reported_as_closed() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;
    bench.relay.cut(Cut::Close);

    assert_eq!(bench.next_loss().await.cause, LossCause::Closed);
    bench.assert_lost_state().await;
}

#[tokio::test]
async fn reset_connection_is_reported_as_reset() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;
    bench.relay.cut(Cut::Reset);

    assert_eq!(bench.next_loss().await.cause, LossCause::Reset);
}

#[tokio::test]
async fn server_disconnect_carries_its_message() {
    // Sans course : après le message, russh ne ferme que son sens d'écriture
    // et lit jusqu'à la fermeture du client (`server/session.rs`), et le
    // relais propage cette demi-fermeture ; aucun RST ne peut faire perdre le
    // message. Keepalives espacés : aucun ne part pendant le test.
    let mut bench = Bench::new(LinkTiming {
        keepalive_interval: Duration::from_secs(30),
        keepalive_max: 3,
        inactivity_timeout: Duration::from_secs(60),
    })
    .await;
    let handle = bench.connect().await;
    handle
        .disconnect(Disconnect::ByApplication, "maintenance".into(), "fr".into())
        .await
        .unwrap();

    let loss = bench.next_loss().await;
    assert_eq!(loss.cause, LossCause::Closed);
    assert_eq!(loss.detail.as_deref(), Some("maintenance"));
}

#[tokio::test]
async fn voluntary_disconnect_emits_nothing() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;
    bench.state.disconnect().await;

    bench.assert_no_loss_for(Duration::from_secs(2)).await;
    assert!(matches!(
        bench.state.connection().await,
        Err(AppError::NotConnected)
    ));
}

/// Liaison au repos : les réponses aux keepalives réarment aussi
/// `inactivity_timeout` (1 s ici), qui ne se déclenche donc pas.
#[tokio::test]
async fn idle_link_survives_with_keepalive() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;

    bench.assert_no_loss_for(Duration::from_secs(3)).await;
    assert!(bench.state.connection_status().await.is_some());
    assert!(bench.state.server_info().await.is_ok());
}

/// Une coupure plus courte que le délai de détection ne provoque aucun LOS.
#[tokio::test]
async fn short_cut_is_absorbed() {
    let timing = LinkTiming {
        keepalive_interval: Duration::from_millis(500),
        keepalive_max: 3,
        inactivity_timeout: Duration::from_secs(10),
    };
    let mut bench = Bench::new(timing).await;
    bench.connect().await;
    bench.relay.cut(Cut::Silent);
    tokio::time::sleep(Duration::from_millis(600)).await;
    bench.relay.cut(Cut::None);

    // Le relais garde les octets non transmis : reprise sans perte.
    bench.assert_no_loss_for(Duration::from_secs(3)).await;
}

#[tokio::test]
async fn stream_cut_by_loss_ends_silently_and_registry_empties() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;

    let (events, mut received) = mpsc::unbounded_channel();
    let sink: EventSink = Arc::new(move |event| events.send(event).is_ok());
    bench
        .state
        .start_stream("tail -f /dev/null", ScriptParser::new(), sink, None)
        .await
        .unwrap();
    let first = tokio::time::timeout(MARGIN, received.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(first, LogEvent::Lines { .. }));
    assert_eq!(bench.state.stream_count(), 1);

    bench.relay.cut(Cut::Silent);
    bench.next_loss().await;
    tokio::time::timeout(MARGIN, async {
        while bench.state.stream_count() > 0 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("registre des flux non vidé");

    while let Ok(event) = received.try_recv() {
        assert!(
            !matches!(event, LogEvent::Error { .. }),
            "aucune erreur de flux attendue après la perte : {event:?}"
        );
    }
}

#[tokio::test]
async fn reconnection_after_loss_starts_clean() {
    let mut bench = Bench::new(FAST).await;
    bench.connect().await;
    bench.relay.cut(Cut::Close);
    bench.next_loss().await;

    bench.connect().await;
    assert!(bench.state.connection().await.is_ok());
    assert_eq!(bench.state.stream_count(), 0);
    assert!(bench.state.server_info().await.is_ok());

    bench.relay.cut(Cut::Close);
    assert_eq!(bench.next_loss().await.cause, LossCause::Closed);
}
