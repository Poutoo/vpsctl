//! Flux de lignes d'une commande longue : logs Docker et systemd, et plus
//! tard exécution des scripts (même `LogEvent`, même console).
//!
//! - Découpage des données reçues en lignes, une mémoire par flux de sortie :
//!   une ligne peut être coupée entre deux paquets SSH.
//! - Conversion de chaque ligne brute en `LogLine` par un `LineParser`
//!   propre à la source (`docker`, `systemd`).
//! - Regroupement par lots (50 ms ou 500 lignes) et limite de débit
//!   (5 000 lignes par seconde, l'excédent est compté dans `dropped`).
//! - Script de surveillance autour de la commande, pour qu'aucun processus ne
//!   survive à la fermeture du canal (`guarded_command`).
//!
//! Ce module ne dépend pas de Tauri : les événements partent vers une
//! fonction de rappel (`EventSink`). Le registre des flux est dans `state`.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tokio::time::{Instant, MissedTickBehavior};
use ts_rs::TS;

use crate::error::AppError;
use crate::ssh::{StreamChannel, StreamChunk};

/// Intervalle d'envoi des lots.
const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
/// Un lot part sans attendre l'intervalle dès qu'il atteint cette taille.
const BATCH_MAX: usize = 500;
/// Débit maximal transmis à l'interface, en lignes par seconde.
const RATE_LIMIT: u32 = 5_000;
/// Longueur maximale d'une ligne affichée, en octets.
pub const MAX_LINE_BYTES: usize = 16 * 1024;

// ---------------------------------------------------------------------------
// Événements (contrat : LogEvent)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Origin {
    Stdout,
    Stderr,
    Journal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogLine {
    pub text: String,
    /// ISO 8601 en UTC, à la milliseconde : `2026-10-16T09:13:46.900Z`.
    pub timestamp: Option<String>,
    pub origin: Origin,
    /// Priorité syslog 0-7, uniquement pour `origin = journal`.
    pub priority: Option<u8>,
    /// Entrée du journal sans `MESSAGE` : `text` vaut « [sans message] ».
    pub missing: bool,
}

impl LogLine {
    /// Ligne de la sortie standard ou d'erreur, sans horodatage.
    pub fn plain(raw: Raw, text: String) -> Self {
        Self {
            text,
            timestamp: None,
            origin: match raw {
                Raw::Stdout => Origin::Stdout,
                Raw::Stderr => Origin::Stderr,
            },
            priority: None,
            missing: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum LogEvent {
    /// Lot de lignes. `dropped` : lignes écartées depuis le lot précédent,
    /// le débit dépassant la limite.
    Lines { lines: Vec<LogLine>, dropped: u32 },
    /// La commande s'est terminée d'elle-même.
    Ended { exit_code: Option<i32> },
    /// Flux interrompu : liaison perdue, commande refusée.
    Error { message: String },
}

/// Destinataire des événements (le canal Tauri, côté commandes). Renvoie
/// `false` si plus personne n'écoute : le flux s'arrête alors de lui-même.
pub type EventSink = Arc<dyn Fn(LogEvent) -> bool + Send + Sync>;

// ---------------------------------------------------------------------------
// Script de surveillance
// ---------------------------------------------------------------------------

/// Enveloppe une commande simple (sans `|`, `;` ni `&&`) pour qu'elle se
/// termine quand le canal SSH se ferme, même si elle n'écrit plus rien et
/// même si le signal SSH n'a pas pu être envoyé (liaison coupée, application
/// tuée).
///
/// - La commande tourne en arrière-plan, entrée standard sur `/dev/null`.
/// - Le surveillant lit l'entrée standard du canal, gardée sur le
///   descripteur 3 (une commande en arrière-plan reçoit `/dev/null`). sshd
///   ferme ce tube avec le canal : la lecture se termine et `kill -TERM 0`
///   arrête tout le groupe de processus de la session, créé par sshd
///   (`setsid`). Viser le groupe plutôt qu'un PID : son identifiant ne peut
///   pas être réattribué tant que le surveillant en fait partie.
/// - Le surveillant est un seul processus : `read` est intégré au shell (pas
///   de `cat`). Quand la commande se termine d'elle-même, `kill "$w"` le
///   termine entièrement ; `$w` est un enfant pas encore attendu du shell,
///   son PID ne peut pas non plus être réattribué. Le code de sortie de la
///   commande est remonté.
pub fn guarded_command(command: &str) -> String {
    format!(
        "exec 3<&0 </dev/null; {command} 3<&- & p=$!; \
         {{ while read -r l; do :; done; kill -TERM 0; }} <&3 >/dev/null 2>&1 & w=$!; \
         exec 3<&-; wait \"$p\"; s=$?; kill \"$w\" 2>/dev/null; exit \"$s\""
    )
}

// ---------------------------------------------------------------------------
// Découpage en lignes
// ---------------------------------------------------------------------------

/// Flux de sortie d'où vient une ligne brute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raw {
    Stdout,
    Stderr,
}

/// Reconstitue les lignes d'un flux de sortie à partir de morceaux
/// quelconques. Retire le `\r` final, décode l'UTF-8 invalide avec
/// remplacement et tronque une ligne trop longue (marquée d'un `…`).
pub struct LineSplitter {
    buffer: Vec<u8>,
    max_bytes: usize,
    /// Ligne en cours déjà trop longue : la suite est ignorée jusqu'au `\n`.
    overflowed: bool,
}

impl LineSplitter {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_bytes,
            overflowed: false,
        }
    }

    /// Ajoute un morceau et renvoie les lignes complètes.
    pub fn push(&mut self, data: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for part in data.split_inclusive(|&b| b == b'\n') {
            let (content, complete) = match part.strip_suffix(b"\n") {
                Some(content) => (content, true),
                None => (part, false),
            };
            let room = self.max_bytes.saturating_sub(self.buffer.len());
            if content.len() > room {
                self.overflowed = true;
            }
            self.buffer
                .extend_from_slice(&content[..content.len().min(room)]);
            if complete {
                lines.push(self.take_line());
            }
        }
        lines
    }

    /// Dernière ligne sans `\n` final, à la fin du flux.
    pub fn finish(&mut self) -> Option<String> {
        (!self.buffer.is_empty() || self.overflowed).then(|| self.take_line())
    }

    fn take_line(&mut self) -> String {
        let mut bytes = std::mem::take(&mut self.buffer);
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        let mut line = String::from_utf8_lossy(&bytes).into_owned();
        if std::mem::take(&mut self.overflowed) {
            line.push('…');
        }
        line
    }
}

/// Convertit une ligne brute en lignes de console (zéro, une ou plusieurs).
pub trait LineParser: Send + 'static {
    /// Longueur maximale d'une ligne brute (au-delà, elle est tronquée).
    fn max_raw_line(&self) -> usize {
        MAX_LINE_BYTES
    }

    fn parse(&mut self, raw: Raw, line: String, out: &mut Vec<LogLine>);
}

/// Lignes telles quelles, origine `stdout` / `stderr`.
pub struct PlainParser;

impl LineParser for PlainParser {
    fn parse(&mut self, raw: Raw, line: String, out: &mut Vec<LogLine>) {
        out.push(LogLine::plain(raw, line));
    }
}

/// Tronque un texte à `MAX_LINE_BYTES` octets, sans couper un caractère.
pub fn truncate_line(mut text: String) -> String {
    if text.len() <= MAX_LINE_BYTES {
        return text;
    }
    let mut end = MAX_LINE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push('…');
    text
}

// ---------------------------------------------------------------------------
// Source, historique et horodatages
// ---------------------------------------------------------------------------

/// Source d'un flux de logs (contrat : `{ docker: id } | { systemd: unit }`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogSource {
    /// ID complet du conteneur.
    Docker(String),
    /// Nom de l'unité.
    Systemd(String),
}

/// Lignes d'historique au maximum : la taille de la mémoire tampon de la
/// console.
pub const MAX_TAIL: u32 = 5_000;

/// Nombre de lignes d'historique validé.
pub fn checked_tail(tail: u32) -> Result<u32, AppError> {
    if tail <= MAX_TAIL {
        Ok(tail)
    } else {
        Err(AppError::invalid_input(format!(
            "historique trop long : {MAX_TAIL} lignes au maximum"
        )))
    }
}

/// Horodatage RFC 3339 en UTC (`2026-10-16T09:13:46.900123456Z`, fraction
/// facultative et de longueur quelconque) ramené à la milliseconde :
/// `2026-10-16T09:13:46.900Z`. `None` pour toute autre forme.
pub fn normalize_utc_timestamp(text: &str) -> Option<String> {
    let rest = text.strip_suffix('Z')?;
    let (base, fraction) = match rest.split_once('.') {
        Some((base, fraction)) => (base, fraction),
        None => (rest, ""),
    };
    let shape_ok = base.len() == 19
        && base.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            10 => b == b'T',
            13 | 16 => b == b':',
            _ => b.is_ascii_digit(),
        });
    if !shape_ok || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if rest.contains('.') && fraction.is_empty() {
        return None;
    }
    let millis: String = fraction.chars().chain("000".chars()).take(3).collect();
    Some(format!("{base}.{millis}Z"))
}

/// Microsecondes depuis l'epoch Unix (`__REALTIME_TIMESTAMP` du journal)
/// en ISO 8601 UTC à la milliseconde.
pub fn iso_from_micros(micros: u64) -> String {
    let millis = micros / 1_000;
    let seconds = millis / 1_000;
    let (year, month, day) = civil_from_days(seconds / 86_400);
    let in_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        in_day / 3_600,
        in_day / 60 % 60,
        in_day % 60,
        millis % 1_000
    )
}

/// Date du calendrier grégorien d'un nombre de jours depuis le 1970-01-01
/// (algorithme « civil_from_days » de Howard Hinnant).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

// ---------------------------------------------------------------------------
// Lots et limite de débit
// ---------------------------------------------------------------------------

/// Accumule les lignes jusqu'au prochain envoi et applique la limite de débit
/// (fenêtres d'une seconde).
pub struct Batcher {
    lines: Vec<LogLine>,
    dropped: u32,
    window_start: Option<Instant>,
    in_window: u32,
}

impl Batcher {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            dropped: 0,
            window_start: None,
            in_window: 0,
        }
    }

    pub fn push(&mut self, line: LogLine, now: Instant) {
        let expired = self
            .window_start
            .is_none_or(|start| now.duration_since(start) >= Duration::from_secs(1));
        if expired {
            self.window_start = Some(now);
            self.in_window = 0;
        }
        if self.in_window >= RATE_LIMIT {
            self.dropped = self.dropped.saturating_add(1);
        } else {
            self.in_window += 1;
            self.lines.push(line);
        }
    }

    pub fn is_full(&self) -> bool {
        self.lines.len() >= BATCH_MAX
    }

    /// Lot à envoyer, ou `None` s'il n'y a rien de neuf.
    pub fn take(&mut self) -> Option<LogEvent> {
        if self.lines.is_empty() && self.dropped == 0 {
            return None;
        }
        Some(LogEvent::Lines {
            lines: std::mem::take(&mut self.lines),
            dropped: std::mem::take(&mut self.dropped),
        })
    }
}

impl Default for Batcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Boucle de lecture
// ---------------------------------------------------------------------------

/// Canal d'un flux. Abstrait pour tester `run` sans réseau.
pub trait StreamSource: Send + 'static {
    fn recv(&mut self) -> impl Future<Output = Option<StreamChunk>> + Send;
    /// Arrête la commande distante et ferme le canal.
    fn stop(self) -> impl Future<Output = ()> + Send;
}

impl StreamSource for StreamChannel {
    fn recv(&mut self) -> impl Future<Output = Option<StreamChunk>> + Send {
        StreamChannel::recv(self)
    }

    fn stop(self) -> impl Future<Output = ()> + Send {
        StreamChannel::stop(self)
    }
}

enum Outcome {
    /// Arrêt demandé (`logs_stop`, déconnexion, fermeture).
    Stopped,
    /// Plus personne n'écoute.
    Abandoned,
    /// Canal fermé par le serveur.
    Closed,
    Refused,
}

/// Lit le flux jusqu'à sa fin ou jusqu'à `stop` (signal envoyé, ou émetteur
/// abandonné). Un arrêt demandé n'émet aucun événement final ; une fin
/// d'elle-même émet `ended`, une liaison perdue ou un refus `error`.
pub async fn run<S: StreamSource, P: LineParser>(
    mut source: S,
    mut parser: P,
    sink: EventSink,
    mut stop: oneshot::Receiver<()>,
) {
    let mut stdout = LineSplitter::new(parser.max_raw_line());
    let mut stderr = LineSplitter::new(parser.max_raw_line());
    let mut batcher = Batcher::new();
    let mut parsed = Vec::new();
    let mut exit: Option<Option<i32>> = None;
    let mut tick = tokio::time::interval(FLUSH_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let outcome = loop {
        let flush = tokio::select! {
            biased;
            _ = &mut stop => break Outcome::Stopped,
            _ = tick.tick() => true,
            chunk = source.recv() => {
                match chunk {
                    Some(StreamChunk::Stdout(data)) => {
                        for line in stdout.push(&data) {
                            parser.parse(Raw::Stdout, line, &mut parsed);
                        }
                    }
                    Some(StreamChunk::Stderr(data)) => {
                        for line in stderr.push(&data) {
                            parser.parse(Raw::Stderr, line, &mut parsed);
                        }
                    }
                    Some(StreamChunk::Exited(code)) => exit = Some(code),
                    Some(StreamChunk::Refused) => break Outcome::Refused,
                    None => break Outcome::Closed,
                }
                let now = Instant::now();
                for line in parsed.drain(..) {
                    batcher.push(line, now);
                }
                batcher.is_full()
            }
        };
        if flush {
            if let Some(event) = batcher.take() {
                if !sink(event) {
                    break Outcome::Abandoned;
                }
            }
        }
    };

    match outcome {
        Outcome::Stopped | Outcome::Abandoned => source.stop().await,
        Outcome::Refused => {
            source.stop().await;
            sink(LogEvent::Error {
                message: "le serveur a refusé d'exécuter la commande".into(),
            });
        }
        Outcome::Closed => {
            if let Some(line) = stdout.finish() {
                parser.parse(Raw::Stdout, line, &mut parsed);
            }
            if let Some(line) = stderr.finish() {
                parser.parse(Raw::Stderr, line, &mut parsed);
            }
            let now = Instant::now();
            for line in parsed.drain(..) {
                batcher.push(line, now);
            }
            if let Some(event) = batcher.take() {
                sink(event);
            }
            sink(match exit {
                Some(exit_code) => LogEvent::Ended { exit_code },
                None => LogEvent::Error {
                    message: "liaison perdue : le flux s'est interrompu".into(),
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    // --- Événements ------------------------------------------------------

    #[test]
    fn events_serialize_per_contract() {
        let line = LogLine {
            text: "GET /".into(),
            timestamp: Some("2026-10-16T09:13:46.900Z".into()),
            origin: Origin::Journal,
            priority: Some(6),
            missing: false,
        };
        assert_eq!(
            serde_json::to_value(LogEvent::Lines {
                lines: vec![line],
                dropped: 0
            })
            .unwrap(),
            json!({
                "type": "lines",
                "lines": [{
                    "text": "GET /",
                    "timestamp": "2026-10-16T09:13:46.900Z",
                    "origin": "journal",
                    "priority": 6,
                    "missing": false,
                }],
                "dropped": 0,
            })
        );
        assert_eq!(
            serde_json::to_value(LogEvent::Ended { exit_code: Some(1) }).unwrap(),
            json!({ "type": "ended", "exitCode": 1 })
        );
        assert_eq!(
            serde_json::to_value(LogEvent::Error {
                message: "x".into()
            })
            .unwrap(),
            json!({ "type": "error", "message": "x" })
        );
    }

    // --- Script de surveillance ------------------------------------------

    const LOGS: &str = "docker logs -f --tail 200 --timestamps 'abc'";

    #[test]
    fn guarded_command_wraps_the_command() {
        assert_eq!(
            guarded_command(LOGS),
            "exec 3<&0 </dev/null; docker logs -f --tail 200 --timestamps 'abc' 3<&- & p=$!; \
             { while read -r l; do :; done; kill -TERM 0; } <&3 >/dev/null 2>&1 & w=$!; \
             exec 3<&-; wait \"$p\"; s=$?; kill \"$w\" 2>/dev/null; exit \"$s\""
        );
    }

    #[test]
    fn watchdog_never_targets_a_reusable_pid() {
        let script = guarded_command(LOGS);
        // Le surveillant arrête le groupe de processus de la session, jamais
        // le PID de la commande, qui peut avoir été réattribué.
        assert!(script.contains("kill -TERM 0;"));
        assert!(!script.contains("kill -TERM \"$p\""));
        assert!(!script.contains("kill \"$p\""));
        // Lecture intégrée au shell : pas de processus `cat` qui survivrait
        // au sous-shell.
        assert!(!script.contains("cat"));
        assert!(script.contains("while read -r l; do :; done"));
        // Commande terminée d'elle-même : le surveillant est tué après
        // l'attente, avant la sortie.
        let wait = script.find("wait \"$p\"").unwrap();
        let kill_watcher = script.find("kill \"$w\"").unwrap();
        let exit = script.find("exit \"$s\"").unwrap();
        assert!(wait < kill_watcher && kill_watcher < exit);
    }

    #[test]
    fn watchdog_only_stops_at_the_end_of_its_input() {
        let script = guarded_command(LOGS);
        // `read` seul se terminerait à la première ligne reçue : il est dans
        // une boucle qui ne sort qu'à la fin de l'entrée (fermeture du canal).
        // Variable nommée `l`, pas `_` : `_` est un paramètre spécial de bash
        // et de zsh, une erreur de `read` sortirait de la boucle à l'ouverture.
        let watcher = "{ while read -r l; do :; done; kill -TERM 0; }";
        assert!(script.contains(watcher));
        assert_eq!(script.matches("read").count(), 1);
        assert!(!script.contains("read -r _"));
    }

    #[test]
    fn watchdog_keeps_the_channel_away_from_the_command() {
        let script = guarded_command(LOGS);
        // La commande n'hérite ni de l'entrée du canal ni du descripteur 3.
        assert!(script.starts_with("exec 3<&0 </dev/null; "));
        assert!(script.contains(&format!("{LOGS} 3<&- &")));
        // Le surveillant ne garde pas les sorties du canal ouvertes.
        assert!(script.contains("<&3 >/dev/null 2>&1 &"));
    }

    /// Exécution réelle du script par `sh`, dans son propre groupe de
    /// processus comme sous sshd (`setsid`).
    #[cfg(unix)]
    mod shell {
        use super::super::guarded_command;
        use std::io::Write;
        use std::os::unix::process::CommandExt;
        use std::process::{Child, Command, ExitStatus, Stdio};
        use std::time::{Duration, Instant};

        fn spawn(command: &str) -> Child {
            Command::new("sh")
                .arg("-c")
                .arg(guarded_command(command))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .unwrap()
        }

        fn wait_exit(child: &mut Child) -> ExitStatus {
            let start = Instant::now();
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    return status;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "le script tourne encore"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        /// Plus aucun processus dans le groupe (au plus 5 s d'attente).
        fn assert_group_empty(pgid: u32) {
            let start = Instant::now();
            loop {
                let alive = Command::new("sh")
                    .arg("-c")
                    .arg(format!("kill -0 -{pgid} 2>/dev/null"))
                    .status()
                    .unwrap()
                    .success();
                if !alive {
                    return;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(5),
                    "processus restants"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        #[test]
        fn natural_end_reports_the_code_and_leaves_no_watcher() {
            let mut child = spawn("sh -c 'echo fin; exit 3'");
            let pgid = child.id();
            let mut stdin = child.stdin.take().unwrap();
            let status = wait_exit(&mut child);
            assert_eq!(status.code(), Some(3));
            let mut out = String::new();
            std::io::Read::read_to_string(&mut child.stdout.take().unwrap(), &mut out).unwrap();
            assert_eq!(out, "fin\n");
            // Plus aucun lecteur sur l'entrée du canal : un `cat` survivant
            // l'aurait gardée ouverte.
            assert_group_empty(pgid);
            assert!(stdin.write_all(b"x\n").and_then(|_| stdin.flush()).is_err());
        }

        #[test]
        fn received_lines_do_not_stop_the_command() {
            let mut child = spawn("sleep 30");
            let pgid = child.id();
            let mut stdin = child.stdin.take().unwrap();
            std::thread::sleep(Duration::from_millis(200));
            stdin.write_all(b"une ligne\nune autre\n").unwrap();
            stdin.flush().unwrap();
            std::thread::sleep(Duration::from_millis(500));
            assert!(child.try_wait().unwrap().is_none(), "arrêté par une ligne");
            // Seule la fermeture de l'entrée arrête la commande.
            drop(stdin);
            wait_exit(&mut child);
            assert_group_empty(pgid);
        }

        #[test]
        fn closing_the_channel_stops_a_silent_command() {
            let mut child = spawn("sleep 30");
            let pgid = child.id();
            std::thread::sleep(Duration::from_millis(200));
            // Fermeture du canal par sshd : l'entrée standard est fermée.
            drop(child.stdin.take());
            let status = wait_exit(&mut child);
            assert!(!status.success());
            assert_group_empty(pgid);
        }

        #[test]
        fn ssh_signal_stops_the_whole_group() {
            let mut child = spawn("sleep 30");
            let pgid = child.id();
            std::thread::sleep(Duration::from_millis(200));
            // Requête `signal` : sshd fait `killpg(pid, SIGTERM)`.
            Command::new("sh")
                .arg("-c")
                .arg(format!("kill -TERM -{pgid}"))
                .status()
                .unwrap();
            wait_exit(&mut child);
            assert_group_empty(pgid);
        }
    }

    // --- Découpage en lignes ---------------------------------------------

    #[test]
    fn lines_split_across_chunks_are_rebuilt() {
        let mut splitter = LineSplitter::new(MAX_LINE_BYTES);
        assert_eq!(splitter.push(b"premi"), Vec::<String>::new());
        assert_eq!(splitter.push(b"ere\ndeux"), vec!["premiere"]);
        assert_eq!(splitter.push(b"ieme\r\ntrois\n"), vec!["deuxieme", "trois"]);
        assert_eq!(splitter.finish(), None);
        splitter.push(b"sans fin");
        assert_eq!(splitter.finish().as_deref(), Some("sans fin"));
    }

    #[test]
    fn empty_lines_are_kept() {
        let mut splitter = LineSplitter::new(MAX_LINE_BYTES);
        assert_eq!(splitter.push(b"a\n\nb\n"), vec!["a", "", "b"]);
    }

    #[test]
    fn invalid_utf8_is_replaced() {
        let mut splitter = LineSplitter::new(MAX_LINE_BYTES);
        assert_eq!(splitter.push(b"caf\xe9\n"), vec!["caf\u{fffd}"]);
        // Caractère coupé entre deux paquets : reconstitué.
        assert_eq!(splitter.push(b"caf\xc3"), Vec::<String>::new());
        assert_eq!(splitter.push(b"\xa9\n"), vec!["café"]);
    }

    #[test]
    fn long_lines_are_truncated_and_marked() {
        let mut splitter = LineSplitter::new(8);
        assert_eq!(
            splitter.push(b"0123456789abc\nok\n"),
            vec!["01234567…", "ok"]
        );
        // Dépassement réparti sur plusieurs paquets.
        assert_eq!(splitter.push(b"0123"), Vec::<String>::new());
        assert_eq!(splitter.push(b"456789"), Vec::<String>::new());
        assert_eq!(splitter.push(b"\n"), vec!["01234567…"]);
    }

    #[test]
    fn truncate_line_respects_char_boundaries() {
        let text = "é".repeat(MAX_LINE_BYTES);
        let truncated = truncate_line(text);
        assert!(truncated.ends_with('…'));
        assert!(truncated.len() <= MAX_LINE_BYTES + '…'.len_utf8());
        assert_eq!(truncate_line("court".into()), "court");
    }

    // --- Source, historique, horodatages ---------------------------------

    #[test]
    fn log_source_deserializes_per_contract() {
        assert_eq!(
            serde_json::from_value::<LogSource>(json!({ "docker": "abc" })).unwrap(),
            LogSource::Docker("abc".into())
        );
        assert_eq!(
            serde_json::from_value::<LogSource>(json!({ "systemd": "bot.service" })).unwrap(),
            LogSource::Systemd("bot.service".into())
        );
        assert!(serde_json::from_value::<LogSource>(json!({ "shell": "id" })).is_err());
    }

    #[test]
    fn tail_is_bounded_by_the_console_buffer() {
        assert_eq!(checked_tail(0).unwrap(), 0);
        assert_eq!(checked_tail(MAX_TAIL).unwrap(), MAX_TAIL);
        assert!(matches!(
            checked_tail(MAX_TAIL + 1),
            Err(AppError::InvalidInput { .. })
        ));
    }

    #[test]
    fn utc_timestamps_are_cut_to_the_millisecond() {
        assert_eq!(
            normalize_utc_timestamp("2026-10-16T09:13:46.900123456Z").as_deref(),
            Some("2026-10-16T09:13:46.900Z")
        );
        assert_eq!(
            normalize_utc_timestamp("2026-10-16T09:13:46.9Z").as_deref(),
            Some("2026-10-16T09:13:46.900Z")
        );
        assert_eq!(
            normalize_utc_timestamp("2026-10-16T09:13:46Z").as_deref(),
            Some("2026-10-16T09:13:46.000Z")
        );
        for invalid in [
            "",
            "GET /api",
            "2026-10-16T09:13:46.900+02:00",
            "2026-10-16 09:13:46Z",
            "2026-10-16T09:13:46.Z",
            "2026-10-16T09:13:4x.900Z",
        ] {
            assert_eq!(normalize_utc_timestamp(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn journal_microseconds_become_iso_dates() {
        assert_eq!(iso_from_micros(0), "1970-01-01T00:00:00.000Z");
        // 2026-10-16T09:13:46.900123Z
        assert_eq!(
            iso_from_micros(1_792_142_026_900_123),
            "2026-10-16T09:13:46.900Z"
        );
        // Jour bissextile.
        assert_eq!(
            iso_from_micros(1_709_208_000_000_000),
            "2024-02-29T12:00:00.000Z"
        );
    }

    // --- Lots ------------------------------------------------------------

    fn line(n: usize) -> LogLine {
        LogLine::plain(Raw::Stdout, n.to_string())
    }

    #[test]
    fn batcher_counts_lines_over_the_rate_limit() {
        let mut batcher = Batcher::new();
        let start = Instant::now();
        for n in 0..(RATE_LIMIT as usize + 10) {
            batcher.push(line(n), start);
        }
        match batcher.take() {
            Some(LogEvent::Lines { lines, dropped }) => {
                assert_eq!(lines.len(), RATE_LIMIT as usize);
                assert_eq!(dropped, 10);
            }
            other => panic!("lot attendu, reçu {other:?}"),
        }
        // Nouvelle fenêtre une seconde plus tard.
        batcher.push(line(0), start + Duration::from_secs(1));
        assert!(matches!(
            batcher.take(),
            Some(LogEvent::Lines { ref lines, dropped: 0 }) if lines.len() == 1
        ));
        assert_eq!(batcher.take(), None);
    }

    #[test]
    fn batcher_is_full_at_the_batch_size() {
        let mut batcher = Batcher::new();
        let now = Instant::now();
        for n in 0..BATCH_MAX - 1 {
            batcher.push(line(n), now);
        }
        assert!(!batcher.is_full());
        batcher.push(line(0), now);
        assert!(batcher.is_full());
    }

    // --- Boucle de lecture (canal simulé) --------------------------------

    struct FakeSource {
        chunks: VecDeque<StreamChunk>,
        /// Après les morceaux : le canal reste ouvert et silencieux.
        hang: bool,
        stopped: Arc<AtomicBool>,
    }

    impl FakeSource {
        fn new(chunks: Vec<StreamChunk>, hang: bool) -> (Self, Arc<AtomicBool>) {
            let stopped = Arc::new(AtomicBool::new(false));
            let source = Self {
                chunks: chunks.into(),
                hang,
                stopped: stopped.clone(),
            };
            (source, stopped)
        }
    }

    impl StreamSource for FakeSource {
        async fn recv(&mut self) -> Option<StreamChunk> {
            match self.chunks.pop_front() {
                Some(chunk) => Some(chunk),
                None if self.hang => std::future::pending().await,
                None => None,
            }
        }

        async fn stop(self) {
            self.stopped.store(true, Ordering::SeqCst);
        }
    }

    fn recorder() -> (EventSink, Arc<Mutex<Vec<LogEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink: EventSink = {
            let events = events.clone();
            Arc::new(move |event| {
                events.lock().unwrap().push(event);
                true
            })
        };
        (sink, events)
    }

    fn texts(events: &[LogEvent]) -> Vec<(Origin, String)> {
        events
            .iter()
            .filter_map(|e| match e {
                LogEvent::Lines { lines, .. } => Some(lines),
                _ => None,
            })
            .flatten()
            .map(|l| (l.origin, l.text.clone()))
            .collect()
    }

    #[tokio::test]
    async fn natural_end_flushes_lines_then_reports_the_exit_code() {
        let (source, stopped) = FakeSource::new(
            vec![
                StreamChunk::Stdout(b"un\ndeu".to_vec()),
                StreamChunk::Stderr(b"erreur\n".to_vec()),
                StreamChunk::Stdout(b"x".to_vec()),
                StreamChunk::Exited(Some(0)),
            ],
            false,
        );
        let (sink, events) = recorder();
        let (_tx, rx) = oneshot::channel();
        run(source, PlainParser, sink, rx).await;

        let events = events.lock().unwrap();
        assert_eq!(
            texts(&events),
            vec![
                (Origin::Stdout, "un".into()),
                (Origin::Stderr, "erreur".into()),
                (Origin::Stdout, "deux".into()),
            ]
        );
        assert_eq!(events.last(), Some(&LogEvent::Ended { exit_code: Some(0) }));
        assert!(!stopped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn channel_closed_without_exit_is_a_lost_link() {
        let (source, _) = FakeSource::new(vec![StreamChunk::Stdout(b"a\n".to_vec())], false);
        let (sink, events) = recorder();
        let (_tx, rx) = oneshot::channel();
        run(source, PlainParser, sink, rx).await;
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(LogEvent::Error { .. })
        ));
    }

    #[tokio::test]
    async fn stop_request_stops_the_remote_command_without_final_event() {
        let (source, stopped) = FakeSource::new(vec![StreamChunk::Stdout(b"a\n".to_vec())], true);
        let (sink, events) = recorder();
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(run(source, PlainParser, sink, rx));
        tokio::time::sleep(Duration::from_millis(120)).await;
        tx.send(()).unwrap();
        task.await.unwrap();

        assert!(stopped.load(Ordering::SeqCst));
        let events = events.lock().unwrap();
        assert_eq!(texts(&events), vec![(Origin::Stdout, "a".into())]);
        assert!(events.iter().all(|e| matches!(e, LogEvent::Lines { .. })));
    }

    #[tokio::test]
    async fn dropped_stop_sender_also_stops_the_stream() {
        let (source, stopped) = FakeSource::new(vec![], true);
        let (sink, _) = recorder();
        let (tx, rx) = oneshot::channel::<()>();
        drop(tx);
        run(source, PlainParser, sink, rx).await;
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn stream_stops_when_nobody_listens() {
        let (source, stopped) = FakeSource::new(vec![StreamChunk::Stdout(b"a\n".to_vec())], true);
        let sink: EventSink = Arc::new(|_| false);
        let (_tx, rx) = oneshot::channel();
        run(source, PlainParser, sink, rx).await;
        assert!(stopped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn refused_command_is_an_error() {
        let (source, stopped) = FakeSource::new(vec![StreamChunk::Refused], false);
        let (sink, events) = recorder();
        let (_tx, rx) = oneshot::channel();
        run(source, PlainParser, sink, rx).await;
        assert!(stopped.load(Ordering::SeqCst));
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(LogEvent::Error { .. })
        ));
    }
}
