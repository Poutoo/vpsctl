//! Perte de la liaison : détection (keepalive SSH), cause et signal unique.
//!
//! russh 0.63 termine sa tâche de session sur un `SSH_MSG_DISCONNECT` du
//! serveur, une erreur de lecture ou d'écriture (EOF, RST…), l'expiration du
//! keepalive (`KeepaliveTimeout`) ou une écriture bloquée au-delà de
//! `inactivity_timeout` (`InactivityTimeout`). Il appelle alors
//! `Handler::disconnected`, puis détruit le handler. `LossReporter` publie la
//! cause une seule fois par liaison ; à défaut d'appel à `disconnected`, sa
//! destruction publie une cause inconnue.

use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::watch;
use ts_rs::TS;

/// Réglages de détection, passés à russh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkTiming {
    /// Keepalive envoyé après ce délai sans rien recevoir du serveur.
    pub keepalive_interval: Duration,
    /// Keepalives sans réponse tolérés : la perte est constatée
    /// `(keepalive_max + 1) × keepalive_interval` après la dernière réception.
    pub keepalive_max: usize,
    /// Filet pour une écriture bloquée (envoi en cours sur une liaison
    /// coupée) : pendant une écriture, russh ne déclenche pas le keepalive.
    pub inactivity_timeout: Duration,
}

impl Default for LinkTiming {
    /// 5 s × 3 : perte vue 15 à 20 s après la coupure ; une coupure de
    /// quelques secondes est absorbée. 60 s pour une écriture bloquée.
    fn default() -> Self {
        Self {
            keepalive_interval: Duration::from_secs(5),
            keepalive_max: 3,
            inactivity_timeout: Duration::from_secs(60),
        }
    }
}

/// Cause d'une perte de liaison, telle qu'affichée par l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LossCause {
    /// Fermée par le serveur : message de déconnexion ou fin du flux TCP.
    Closed,
    /// Connexion réinitialisée (RST) ou interrompue par le système.
    Reset,
    /// Le serveur ne répond plus (keepalive ou écriture bloquée).
    Timeout,
    /// Réseau ou hôte injoignable.
    Network,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkLoss {
    pub cause: LossCause,
    /// Message du serveur ou du système, s'il y en a un.
    pub detail: Option<String>,
    /// Dernière réception de données, en millisecondes depuis l'epoch Unix.
    pub last_received_at: Option<u64>,
}

/// Fin de la session telle que russh la transmet à `Handler::disconnected`.
pub enum Ending<'a> {
    Disconnected { message: &'a str },
    Failed(&'a russh::Error),
}

/// Cause et détail d'une fin de session.
pub fn classify(ending: Ending<'_>) -> (LossCause, Option<String>) {
    use io::ErrorKind as Kind;
    let error = match ending {
        Ending::Disconnected { message } => {
            let message = message.trim();
            return (
                LossCause::Closed,
                (!message.is_empty()).then(|| message.to_owned()),
            );
        }
        Ending::Failed(error) => error,
    };
    match error {
        russh::Error::KeepaliveTimeout | russh::Error::InactivityTimeout => {
            (LossCause::Timeout, None)
        }
        russh::Error::Disconnect => (LossCause::Closed, None),
        russh::Error::IO(e) => match e.kind() {
            Kind::UnexpectedEof => (LossCause::Closed, None),
            Kind::ConnectionReset | Kind::ConnectionAborted | Kind::BrokenPipe => {
                (LossCause::Reset, None)
            }
            Kind::NetworkUnreachable
            | Kind::HostUnreachable
            | Kind::NetworkDown
            | Kind::TimedOut
            | Kind::AddrNotAvailable => (LossCause::Network, Some(e.to_string())),
            _ => (LossCause::Unknown, Some(e.to_string())),
        },
        other => (LossCause::Unknown, Some(other.to_string())),
    }
}

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Publie la perte de la liaison, une seule fois.
pub struct LossReporter {
    sender: watch::Sender<Option<LinkLoss>>,
    last_received: Arc<AtomicU64>,
}

impl LossReporter {
    pub fn new() -> (Self, LossSignal, Arc<AtomicU64>) {
        let (sender, receiver) = watch::channel(None);
        let last_received = Arc::new(AtomicU64::new(0));
        let reporter = Self {
            sender,
            last_received: last_received.clone(),
        };
        (reporter, LossSignal { receiver }, last_received)
    }

    /// Sans effet si une perte a déjà été publiée.
    pub fn report(&self, cause: LossCause, detail: Option<String>) {
        let last = self.last_received.load(Ordering::SeqCst);
        self.sender.send_if_modified(|slot| {
            if slot.is_some() {
                return false;
            }
            *slot = Some(LinkLoss {
                cause,
                detail: detail.clone(),
                last_received_at: (last != 0).then_some(last),
            });
            true
        });
    }
}

impl Drop for LossReporter {
    fn drop(&mut self) {
        self.report(LossCause::Unknown, None);
    }
}

/// Perte de la liaison, observée par l'état de l'application.
#[derive(Clone)]
pub struct LossSignal {
    receiver: watch::Receiver<Option<LinkLoss>>,
}

impl LossSignal {
    pub fn is_lost(&self) -> bool {
        self.receiver.borrow().is_some()
    }

    /// Attend la perte ; immédiat si elle a déjà eu lieu.
    pub async fn lost(mut self) -> LinkLoss {
        let published = match self.receiver.wait_for(Option::is_some).await {
            Ok(loss) => loss.clone(),
            Err(_) => None,
        };
        published
            .or_else(|| self.receiver.borrow().clone())
            .unwrap_or_else(unknown)
    }
}

fn unknown() -> LinkLoss {
    LinkLoss {
        cause: LossCause::Unknown,
        detail: None,
        last_received_at: None,
    }
}

/// Flux TCP de la liaison : note l'heure de chaque réception et ignore
/// l'échec de sa fermeture. Sans cela, russh s'arrêterait sur cet échec
/// (fréquent après un RST) avant d'appeler `Handler::disconnected`.
pub struct TrackedStream<S> {
    inner: S,
    last_received: Arc<AtomicU64>,
}

impl<S> TrackedStream<S> {
    pub fn new(inner: S, last_received: Arc<AtomicU64>) -> Self {
        Self {
            inner,
            last_received,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for TrackedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() > before {
            self.last_received.store(now_millis(), Ordering::SeqCst);
        }
        result
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for TrackedStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx).map(|_| Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_error(kind: io::ErrorKind) -> russh::Error {
        russh::Error::IO(io::Error::from(kind))
    }

    #[test]
    fn server_disconnect_is_closed_with_its_message() {
        assert_eq!(
            classify(Ending::Disconnected {
                message: " maintenance "
            }),
            (LossCause::Closed, Some("maintenance".into()))
        );
        assert_eq!(
            classify(Ending::Disconnected { message: "" }),
            (LossCause::Closed, None)
        );
    }

    #[test]
    fn errors_are_classified() {
        let cases = [
            (russh::Error::KeepaliveTimeout, LossCause::Timeout),
            (russh::Error::InactivityTimeout, LossCause::Timeout),
            (russh::Error::Disconnect, LossCause::Closed),
            (io_error(io::ErrorKind::UnexpectedEof), LossCause::Closed),
            (io_error(io::ErrorKind::ConnectionReset), LossCause::Reset),
            (io_error(io::ErrorKind::ConnectionAborted), LossCause::Reset),
            (io_error(io::ErrorKind::BrokenPipe), LossCause::Reset),
            (
                io_error(io::ErrorKind::NetworkUnreachable),
                LossCause::Network,
            ),
            (io_error(io::ErrorKind::HostUnreachable), LossCause::Network),
            (io_error(io::ErrorKind::TimedOut), LossCause::Network),
            (
                io_error(io::ErrorKind::PermissionDenied),
                LossCause::Unknown,
            ),
            (russh::Error::SendError, LossCause::Unknown),
        ];
        for (error, cause) in cases {
            assert_eq!(classify(Ending::Failed(&error)).0, cause, "{error:?}");
        }
    }

    #[tokio::test]
    async fn only_the_first_loss_is_published() {
        let (reporter, signal, last_received) = LossReporter::new();
        assert!(!signal.is_lost());
        last_received.store(1_000, Ordering::SeqCst);
        reporter.report(LossCause::Reset, None);
        reporter.report(LossCause::Timeout, Some("ignorée".into()));
        drop(reporter);
        assert_eq!(
            signal.clone().lost().await,
            LinkLoss {
                cause: LossCause::Reset,
                detail: None,
                last_received_at: Some(1_000),
            }
        );
        assert!(signal.is_lost());
    }

    #[tokio::test]
    async fn a_dropped_reporter_publishes_an_unknown_loss() {
        let (reporter, signal, _) = LossReporter::new();
        let waiting = tokio::spawn(signal.lost());
        drop(reporter);
        let loss = waiting.await.unwrap();
        assert_eq!(loss.cause, LossCause::Unknown);
        assert_eq!(loss.last_received_at, None);
    }

    #[tokio::test]
    async fn tracked_stream_notes_reads_and_ignores_shutdown_errors() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        struct FailingShutdown(tokio::io::DuplexStream);
        impl AsyncRead for FailingShutdown {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut ReadBuf<'_>,
            ) -> Poll<io::Result<()>> {
                Pin::new(&mut self.0).poll_read(cx, buf)
            }
        }
        impl AsyncWrite for FailingShutdown {
            fn poll_write(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<io::Result<usize>> {
                Pin::new(&mut self.0).poll_write(cx, buf)
            }
            fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
                Pin::new(&mut self.0).poll_flush(cx)
            }
            fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
                Poll::Ready(Err(io::ErrorKind::NotConnected.into()))
            }
        }

        let (local, mut remote) = tokio::io::duplex(64);
        let last = Arc::new(AtomicU64::new(0));
        let mut stream = TrackedStream::new(FailingShutdown(local), last.clone());
        remote.write_all(b"ssh").await.unwrap();
        let mut buf = [0; 3];
        stream.read_exact(&mut buf).await.unwrap();
        assert!(last.load(Ordering::SeqCst) > 0);
        assert!(stream.shutdown().await.is_ok());
    }
}
