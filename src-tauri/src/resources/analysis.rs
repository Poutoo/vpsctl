//! Analyse Docker à la demande (`docker_analyze`) : `docker system df -v`
//! (volumes) ou `docker system df` (répartition par catégorie), lancée par
//! le registre des flux comme un script (script de surveillance, limite de
//! 4 flux, arrêt par TERM au groupe). La sortie n'est pas affichée : elle
//! est collectée puis analysée à la fin de la commande.

use std::future::Future;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::containers::{self, DockerUsage, VolumeSize};
use crate::error::AppError;
use crate::ssh::link::now_millis;
use crate::ssh::ExecOutput;
use crate::stream::{EventSink, LineParser, LogEvent, LogLine, Origin, Raw};

/// Sortie de `docker system df -v` : un seul objet JSON, sur une ligne.
const MAX_OUTPUT_LINE: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AnalysisKind {
    /// Répartition par catégorie (`docker system df`).
    Usage,
    /// Détail par volume (`docker system df -v`).
    Volumes,
}

impl AnalysisKind {
    pub fn command(self) -> &'static str {
        match self {
            Self::Usage => containers::USAGE_COMMAND,
            Self::Volumes => containers::VOLUMES_COMMAND,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeAnalysis {
    /// Fin de l'analyse (ms depuis l'epoch).
    #[ts(type = "number")]
    pub at: u64,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub volumes: Vec<VolumeSize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum AnalysisResult {
    Usage { usage: DockerUsage },
    Volumes { volumes: VolumeAnalysis },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum AnalysisEvent {
    Done { result: AnalysisResult },
    Failed { message: String },
}

/// Destinataire des événements de l'analyse (le canal Tauri).
pub type AnalysisSink = Arc<dyn Fn(AnalysisEvent) -> bool + Send + Sync>;

/// Lignes gardées entières (pas de limite de 16 Kio), sans traitement.
pub struct WholeLines;

impl LineParser for WholeLines {
    fn max_raw_line(&self) -> usize {
        MAX_OUTPUT_LINE
    }

    fn parse(&mut self, raw: Raw, line: String, out: &mut Vec<LogLine>) {
        out.push(LogLine::plain(raw, line));
    }
}

/// Résultat à partir de la sortie complète. `users` : sortie de
/// `VOLUME_USERS_COMMAND`, pour les volumes.
pub fn interpret(
    kind: AnalysisKind,
    stdout: &str,
    users: Option<&str>,
    started: u64,
    now: u64,
) -> Result<AnalysisResult, String> {
    match kind {
        AnalysisKind::Usage => Ok(AnalysisResult::Usage {
            usage: containers::parse_usage(stdout, now)?,
        }),
        AnalysisKind::Volumes => {
            let mut volumes = containers::parse_volumes(stdout)?;
            if let Some(users) = users {
                containers::attach_users(&mut volumes, users);
            }
            Ok(AnalysisResult::Volumes {
                volumes: VolumeAnalysis {
                    at: now,
                    duration_ms: now.saturating_sub(started),
                    volumes,
                },
            })
        }
    }
}

#[derive(Default)]
struct Collected {
    stdout: String,
    stderr: Vec<String>,
}

/// Collecte la sortie du flux et produit l'événement final. Une liaison
/// perdue n'en produit aucun (`link-lost` en tient lieu, l'erreur est déjà
/// écartée par le registre). `volume_users` lit les conteneurs de chaque
/// volume après une analyse des volumes réussie.
pub fn collector<F, Fut>(kind: AnalysisKind, volume_users: F, out: AnalysisSink) -> EventSink
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, AppError>> + Send + 'static,
{
    let started = now_millis();
    let collected = Arc::new(Mutex::new(Collected::default()));
    let volume_users = Arc::new(volume_users);
    Arc::new(move |event| {
        let mut buffer = collected.lock().unwrap_or_else(|p| p.into_inner());
        match event {
            LogEvent::Lines { lines, .. } => {
                for line in lines {
                    match line.origin {
                        Origin::Stdout => {
                            buffer.stdout.push_str(&line.text);
                            buffer.stdout.push('\n');
                        }
                        _ => buffer.stderr.push(line.text),
                    }
                }
                true
            }
            LogEvent::Ended { exit_code: Some(0) } => {
                let stdout = std::mem::take(&mut buffer.stdout);
                let (out, volume_users) = (out.clone(), volume_users.clone());
                tokio::spawn(async move {
                    let users = match kind {
                        AnalysisKind::Volumes => volume_users().await.ok(),
                        AnalysisKind::Usage => None,
                    };
                    let event =
                        match interpret(kind, &stdout, users.as_deref(), started, now_millis()) {
                            Ok(result) => AnalysisEvent::Done { result },
                            Err(message) => AnalysisEvent::Failed { message },
                        };
                    out(event);
                });
                true
            }
            LogEvent::Ended { exit_code } => {
                let output = ExecOutput {
                    exit_code: exit_code.and_then(|c| u32::try_from(c).ok()),
                    stdout: Vec::new(),
                    stderr: buffer.stderr.join("\n").into_bytes(),
                };
                out(AnalysisEvent::Failed {
                    message: super::reason(&crate::docker::failure(&output)),
                })
            }
            LogEvent::Error { message } => out(AnalysisEvent::Failed { message }),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(origin: Origin, text: &str) -> LogLine {
        LogLine {
            text: text.into(),
            timestamp: None,
            origin,
            priority: None,
            missing: false,
        }
    }

    fn capture() -> (AnalysisSink, Arc<Mutex<Vec<AnalysisEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink: AnalysisSink = {
            let events = events.clone();
            Arc::new(move |event| {
                events.lock().unwrap().push(event);
                true
            })
        };
        (sink, events)
    }

    #[tokio::test]
    async fn volumes_are_parsed_at_the_end_with_their_containers() {
        let (sink, events) = capture();
        let collect = collector(
            AnalysisKind::Volumes,
            || async { Ok("postgres-main|pg_data\n".to_owned()) },
            sink,
        );
        let json =
            r#"{"Volumes":[{"Name":"pg_data","Size":"46GB"},{"Name":"cache","Size":"2GB"}]}"#;
        assert!(collect(LogEvent::Lines {
            lines: vec![line(Origin::Stdout, json)],
            dropped: 0
        }));
        assert!(events.lock().unwrap().is_empty());
        collect(LogEvent::Ended { exit_code: Some(0) });
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let events = events.lock().unwrap();
        let AnalysisEvent::Done {
            result: AnalysisResult::Volumes { volumes },
        } = &events[0]
        else {
            panic!("{events:?}");
        };
        assert_eq!(volumes.volumes[0].name, "pg_data");
        assert_eq!(volumes.volumes[0].containers, ["postgres-main"]);
    }

    #[tokio::test]
    async fn failures_are_reported_with_a_readable_reason() {
        let (sink, events) = capture();
        let collect = collector(AnalysisKind::Usage, || async { Ok(String::new()) }, sink);
        collect(LogEvent::Lines {
            lines: vec![line(
                Origin::Stderr,
                "permission denied while trying to connect to the docker API at unix:///var/run/docker.sock",
            )],
            dropped: 0,
        });
        collect(LogEvent::Ended { exit_code: Some(1) });
        assert_eq!(
            events.lock().unwrap()[0],
            AnalysisEvent::Failed {
                message: "accès à Docker refusé".into()
            }
        );
    }

    #[test]
    fn interpret_usage_and_unreadable_output() {
        let usage = [
            r#"{"Type":"Images","Size":"1GB"}"#,
            r#"{"Type":"Containers","Size":"2GB"}"#,
            r#"{"Type":"Local Volumes","Size":"3GB"}"#,
            r#"{"Type":"Build Cache","Size":"4GB"}"#,
        ]
        .join("\n");
        let result = interpret(AnalysisKind::Usage, &usage, None, 0, 9).unwrap();
        assert!(
            matches!(result, AnalysisResult::Usage { usage } if usage.build_cache == 4_000_000_000 && usage.at == 9)
        );
        assert!(interpret(AnalysisKind::Volumes, "table", None, 0, 1).is_err());
        let volumes = interpret(
            AnalysisKind::Volumes,
            r#"{"Volumes":[]}"#,
            None,
            1_000,
            4_500,
        )
        .unwrap();
        assert!(
            matches!(volumes, AnalysisResult::Volumes { volumes } if volumes.duration_ms == 3_500)
        );
    }

    #[test]
    fn events_serialize_per_contract() {
        let event = AnalysisEvent::Failed {
            message: "x".into(),
        };
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            serde_json::json!({ "type": "failed", "message": "x" })
        );
    }
}
