//! Ressources du serveur (planche VII) : CPU, mémoire, charge, réseau,
//! disque et conteneurs, sans agent ni stockage sur le serveur.
//!
//! - `proc`, `containers`, `alerts` : fonctions pures (lecture des sorties,
//!   hystérésis) ;
//! - `settings` : seuils par profil ;
//! - `monitor` : tâche de relevé d'une liaison ;
//! - `analysis` : analyse Docker à la demande, sur le registre des flux.
//!
//! `ResourceHub` garde ce qui dure au-delà d'une liaison : mode demandé par
//! l'interface, dernier relevé, niveaux d'alerte et seuils. Ce module ne
//! dépend pas de Tauri : les relevés partent vers une fonction de rappel.

pub mod alerts;
pub mod analysis;
pub mod containers;
pub mod monitor;
pub mod proc;
pub mod settings;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tokio::sync::watch;
use ts_rs::TS;

use crate::error::AppError;
use alerts::{Level, Levels, Values};
use containers::{ContainerUsage, DockerUsage};
use proc::{DiskSpace, LoadAverage, MemoryUsage, NetworkRates};
use settings::{ThresholdStore, Thresholds};

/// Répartition de l'espace Docker lancée d'elle-même à l'alerte disque
/// (`true`), ou seulement à la demande (« Analyser l'espace Docker »). À
/// passer à `false` si `docker system df` coûte presque autant que
/// `docker system df -v` (mesure de `docs/tests-jalon-13.md`, section 0).
pub const DOCKER_USAGE_AUTOMATIC: bool = true;

/// Nom de l'événement émis vers l'interface.
pub const UPDATE_EVENT: &str = "resources-update";

/// Valeur d'une mesure dans un relevé.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Measure<T> {
    Ok {
        value: T,
    },
    /// Mesure par différence : premier relevé.
    Pending,
    Unavailable {
        reason: String,
    },
    /// Non relevée hors de l'écran.
    NotRead,
}

impl<T> Measure<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Ok { value } => Some(value),
            _ => None,
        }
    }

    pub fn unavailable(error: &AppError) -> Self {
        Self::Unavailable {
            reason: reason(error),
        }
    }
}

/// Raison d'une mesure impossible, pour l'affichage.
pub fn reason(error: &AppError) -> String {
    match error {
        AppError::DockerNotInstalled => "Docker n'est pas installé".into(),
        AppError::DockerPermissionDenied => "accès à Docker refusé".into(),
        AppError::PermissionDenied { .. } => "accès refusé".into(),
        AppError::NotFound { .. } => "introuvable sur ce serveur".into(),
        AppError::Network { message } | AppError::Io { message } => message.clone(),
        AppError::Remote { stderr, exit_code } if stderr.is_empty() => {
            format!("la commande a échoué (code {exit_code})")
        }
        AppError::Remote { stderr, .. } => stderr.clone(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiskUsage {
    pub mount_point: String,
    pub device: String,
    pub fs_type: String,
    pub space: Measure<DiskSpace>,
}

/// Résultat d'un relevé, avant le calcul des alertes.
#[derive(Debug, Clone, PartialEq)]
pub struct Survey {
    pub at: u64,
    pub foreground: bool,
    pub cores: Option<u32>,
    pub cpu: Measure<f64>,
    pub memory: Measure<MemoryUsage>,
    pub load: Measure<LoadAverage>,
    pub network: Measure<NetworkRates>,
    pub disks: Measure<Vec<DiskUsage>>,
    pub containers: Measure<Vec<ContainerUsage>>,
    pub docker_mount: Option<String>,
}

/// Événement `resources-update`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResourceUpdate {
    pub profile_id: String,
    /// Fin du relevé (ms depuis l'epoch).
    #[ts(type = "number")]
    pub at: u64,
    /// Relevé complet (écran affiché).
    pub foreground: bool,
    pub cores: Option<u32>,
    /// En %, de 0 à 100.
    pub cpu: Measure<f64>,
    pub memory: Measure<MemoryUsage>,
    pub load: Measure<LoadAverage>,
    pub network: Measure<NetworkRates>,
    pub disks: Measure<Vec<DiskUsage>>,
    pub containers: Measure<Vec<ContainerUsage>>,
    /// Point de montage qui contient la racine de Docker.
    pub docker_mount: Option<String>,
    /// Dernière répartition automatique de l'espace Docker.
    pub docker_usage: Option<DockerUsage>,
    pub automatic_docker_usage: bool,
    pub levels: Levels,
    pub thresholds: Thresholds,
}

/// Destinataire des relevés (l'émission Tauri, côté `lib.rs`).
pub type UpdateSink = Arc<dyn Fn(ResourceUpdate) + Send + Sync>;

#[derive(Default)]
struct Session {
    profile_id: Option<String>,
    last: Option<ResourceUpdate>,
    levels: Levels,
    docker_usage: Option<DockerUsage>,
}

pub struct ResourceHub {
    thresholds: ThresholdStore,
    foreground: watch::Sender<bool>,
    session: Mutex<Session>,
    sink: UpdateSink,
}

impl ResourceHub {
    pub fn new(config_dir: &Path, sink: UpdateSink) -> Self {
        Self {
            thresholds: ThresholdStore::load(config_dir),
            foreground: watch::Sender::new(false),
            session: Mutex::new(Session::default()),
            sink,
        }
    }

    fn session(&self) -> MutexGuard<'_, Session> {
        // Un verrou empoisonné ne protège qu'un état d'affichage.
        self.session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Écran Ressources affiché et fenêtre visible : relevé complet.
    pub fn set_foreground(&self, foreground: bool) {
        self.foreground.send_if_modified(|current| {
            let changed = *current != foreground;
            *current = foreground;
            changed
        });
    }

    pub fn watch_foreground(&self) -> watch::Receiver<bool> {
        self.foreground.subscribe()
    }

    /// Liaison établie. Après une perte de la même liaison, la session
    /// (dernier relevé, niveaux) est gardée ; pour un autre profil, vidée.
    pub fn begin(&self, profile_id: &str) {
        let mut session = self.session();
        if session.profile_id.as_deref() != Some(profile_id) {
            *session = Session {
                profile_id: Some(profile_id.to_owned()),
                ..Session::default()
            };
        }
    }

    /// Déconnexion volontaire : tout est oublié.
    pub fn end(&self) {
        *self.session() = Session::default();
    }

    pub fn last(&self) -> Option<ResourceUpdate> {
        self.session().last.clone()
    }

    pub fn thresholds(&self, profile_id: &str) -> Thresholds {
        self.thresholds.get(profile_id)
    }

    pub fn save_thresholds(
        &self,
        profile_id: &str,
        thresholds: Thresholds,
    ) -> Result<Thresholds, AppError> {
        let saved = self.thresholds.save(profile_id, thresholds)?;
        self.refresh(profile_id);
        Ok(saved)
    }

    pub fn reset_thresholds(&self, profile_id: &str) -> Result<Thresholds, AppError> {
        let thresholds = self.thresholds.reset(profile_id)?;
        self.refresh(profile_id);
        Ok(thresholds)
    }

    pub fn forget_profile(&self, profile_id: &str) -> Result<(), AppError> {
        self.thresholds.forget_profile(profile_id)
    }

    /// Seuils changés : niveaux recalculés sur le dernier relevé, sans en
    /// attendre un nouveau.
    fn refresh(&self, profile_id: &str) {
        let update = {
            let mut session = self.session();
            if session.profile_id.as_deref() != Some(profile_id) {
                return;
            }
            let Some(mut update) = session.last.clone() else {
                return;
            };
            let thresholds = self.thresholds.get(profile_id);
            let levels = alerts::compute(&session.levels, &values_of(&update), &thresholds);
            session.levels = levels.clone();
            update.levels = levels;
            update.thresholds = thresholds;
            session.last = Some(update.clone());
            update
        };
        (self.sink)(update);
    }

    /// Calcule les alertes d'un relevé, le garde et l'émet. `None` si la
    /// session appartient à un autre profil (relevé périmé).
    pub fn publish(&self, profile_id: &str, survey: Survey) -> Option<ResourceUpdate> {
        let update = {
            let mut session = self.session();
            if session.profile_id.as_deref() != Some(profile_id) {
                return None;
            }
            let thresholds = self.thresholds.get(profile_id);
            let mut update = ResourceUpdate {
                profile_id: profile_id.to_owned(),
                at: survey.at,
                foreground: survey.foreground,
                cores: survey.cores,
                cpu: survey.cpu,
                memory: survey.memory,
                load: survey.load,
                network: survey.network,
                disks: survey.disks,
                containers: survey.containers,
                docker_mount: survey.docker_mount,
                docker_usage: session.docker_usage,
                automatic_docker_usage: DOCKER_USAGE_AUTOMATIC,
                levels: Levels::default(),
                thresholds,
            };
            let levels = alerts::compute(&session.levels, &values_of(&update), &thresholds);
            session.levels = levels.clone();
            update.levels = levels;
            session.last = Some(update.clone());
            update
        };
        (self.sink)(update.clone());
        Some(update)
    }

    pub fn docker_usage(&self) -> Option<DockerUsage> {
        self.session().docker_usage
    }

    /// Répartition automatique terminée : gardée et émise avec le dernier
    /// relevé.
    pub fn set_docker_usage(&self, profile_id: &str, usage: DockerUsage) {
        let update = {
            let mut session = self.session();
            if session.profile_id.as_deref() != Some(profile_id) {
                return;
            }
            session.docker_usage = Some(usage);
            let Some(last) = session.last.as_mut() else {
                return;
            };
            last.docker_usage = Some(usage);
            last.clone()
        };
        (self.sink)(update);
    }
}

/// Niveau du point de montage de Docker dans un relevé.
pub fn docker_mount_level(update: &ResourceUpdate) -> Level {
    update
        .docker_mount
        .as_deref()
        .and_then(|point| update.levels.disks.iter().find(|m| m.mount_point == point))
        .map_or(Level::Nominal, |m| m.level)
}

/// Valeurs comparées aux seuils.
fn values_of(update: &ResourceUpdate) -> Values {
    let mut values = Values {
        memory: update.memory.value().map(|m| m.percent),
        load_per_core: update.load.value().map(|l| l.per_core[1]),
        ..Values::default()
    };
    if let Some(disks) = update.disks.value() {
        values.mounts_read = true;
        for disk in disks {
            match disk.space.value() {
                Some(space) => values.disks.push((disk.mount_point.clone(), space.percent)),
                None => values.unmeasured_disks.push(disk.mount_point.clone()),
            }
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn survey(memory_percent: f64) -> Survey {
        Survey {
            at: 1_000,
            foreground: false,
            cores: Some(4),
            cpu: Measure::NotRead,
            memory: Measure::Ok {
                value: MemoryUsage {
                    total: 100,
                    used: memory_percent as u64,
                    percent: memory_percent,
                    swap: None,
                },
            },
            load: Measure::NotRead,
            network: Measure::NotRead,
            disks: Measure::Ok {
                value: vec![DiskUsage {
                    mount_point: "/".into(),
                    device: "/dev/sda1".into(),
                    fs_type: "ext4".into(),
                    space: Measure::Ok {
                        value: DiskSpace {
                            total: 100,
                            free: 59,
                            percent: 41.0,
                        },
                    },
                }],
            },
            containers: Measure::NotRead,
            docker_mount: Some("/".into()),
        }
    }

    fn hub() -> (
        Arc<ResourceHub>,
        Arc<Mutex<Vec<ResourceUpdate>>>,
        std::path::PathBuf,
    ) {
        let dir = std::env::temp_dir().join(format!("vpsctl-hub-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let sent = sent.clone();
            Arc::new(move |update| sent.lock().unwrap().push(update))
        };
        (Arc::new(ResourceHub::new(&dir, sink)), sent, dir)
    }

    #[test]
    fn measures_serialize_with_a_status() {
        assert_eq!(
            serde_json::to_value(Measure::Ok { value: 1.5 }).unwrap(),
            json!({ "status": "ok", "value": 1.5 })
        );
        assert_eq!(
            serde_json::to_value(Measure::<f64>::NotRead).unwrap(),
            json!({ "status": "notRead" })
        );
        assert_eq!(
            serde_json::to_value(Measure::<f64>::Unavailable { reason: "x".into() }).unwrap(),
            json!({ "status": "unavailable", "reason": "x" })
        );
    }

    #[test]
    fn threshold_change_recomputes_levels_without_a_new_survey() {
        let (hub, sent, dir) = hub();
        hub.begin("a");
        hub.publish("a", survey(50.0)).unwrap();
        assert_eq!(hub.last().unwrap().levels.disks[0].level, Level::Nominal);

        // Disque à 41 % : seuil 41 (alerte), 43 (reste), 45 (retombe).
        let mut thresholds = Thresholds::default();
        for (attention, expected) in [
            (41.0, Level::Attention),
            (43.0, Level::Attention),
            (45.0, Level::Nominal),
        ] {
            thresholds.disk.attention = attention;
            hub.save_thresholds("a", thresholds).unwrap();
            let last = sent.lock().unwrap().last().cloned().unwrap();
            assert_eq!(last.levels.disks[0].level, expected, "seuil {attention}");
            assert_eq!(last.thresholds.disk.attention, attention);
        }
        assert_eq!(sent.lock().unwrap().len(), 4);

        // Seuils d'un autre profil : rien n'est émis.
        hub.save_thresholds("b", thresholds).unwrap();
        assert_eq!(sent.lock().unwrap().len(), 4);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn session_survives_a_restore_but_not_another_profile() {
        let (hub, _, dir) = hub();
        hub.begin("a");
        hub.publish("a", survey(90.0)).unwrap();
        hub.begin("a");
        assert!(hub.last().is_some());
        assert_eq!(hub.last().unwrap().levels.memory, Level::Attention);
        hub.begin("b");
        assert!(hub.last().is_none());
        // Relevé d'une liaison précédente : ignoré.
        assert!(hub.publish("a", survey(90.0)).is_none());
        hub.end();
        assert!(hub.last().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
