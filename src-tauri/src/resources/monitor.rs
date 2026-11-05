//! Tâche de relevé d'une liaison.
//!
//! Une seule boucle « relevé, puis attente » : deux relevés ne se
//! chevauchent jamais. Avant-plan (écran affiché, fenêtre visible) : relevé
//! complet, le suivant 5 s après la fin du précédent. Arrière-plan : mémoire
//! et disque toutes les 60 s, par SFTP uniquement. Passer en avant-plan
//! pendant l'attente lance un relevé tout de suite.
//!
//! Les accès au serveur passent par le trait `Probe` : la boucle est testée
//! avec une sonde simulée, en temps virtuel.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::task::JoinSet;
use tokio::time::Instant;

use super::containers::{self, DockerUsage};
use super::proc::{self, CpuTimes, FsStats, Mount, NetCounters};
use super::{docker_mount_level, DiskUsage, Measure, ResourceHub, Survey, DOCKER_USAGE_AUTOMATIC};
use crate::error::AppError;
use crate::sftp::fs::{RemoteFs, SftpFs};
use crate::ssh::link::now_millis;
use crate::ssh::{Connection, Deadline, ExecOutput};

pub const FOREGROUND_INTERVAL: Duration = Duration::from_secs(5);
pub const BACKGROUND_INTERVAL: Duration = Duration::from_secs(60);
/// Échéance de chaque mesure d'un relevé (elles tournent en parallèle).
/// Plus longue que la détection d'une perte silencieuse (20 s au plus) : sur
/// une liaison coupée, la perte est constatée avant, la tâche est abandonnée
/// et les valeurs restent figées au lieu de passer « Indisponible ».
pub const MEASURE_TIMEOUT: Duration = Duration::from_secs(25);
const STATS_TIMEOUT: Duration = Duration::from_secs(25);
const ROOT_DIR_TIMEOUT: Duration = Duration::from_secs(10);
/// Racine de Docker illisible : nouvelle tentative au plus tôt après ce délai.
const ROOT_DIR_RETRY: Duration = Duration::from_secs(300);
/// Répartition automatique de l'espace Docker : au plus une fois par période.
pub const USAGE_INTERVAL: Duration = Duration::from_secs(300);
const USAGE_TIMEOUT: Duration = Duration::from_secs(60);
/// Au-delà, le relevé précédent ne sert plus de base aux mesures par
/// différence (retour sur l'écran) : la mesure repart de `pending`. Couvre
/// un relevé lent (25 s) suivi de l'attente de 5 s.
const BASELINE_MAX_AGE: Duration = Duration::from_secs(40);
/// Taille lue au plus dans un fichier de `/proc`.
const PROC_FILE_LIMIT: u64 = 1024 * 1024;

/// Accès au serveur dont la boucle a besoin.
pub trait Probe: Send + Sync + 'static {
    /// Contenu d'un fichier (de `/proc`), lu jusqu'à la fin.
    fn read_file(&self, path: &str) -> impl Future<Output = Result<String, AppError>> + Send;
    fn fs_stats(&self, path: &str) -> impl Future<Output = Result<FsStats, AppError>> + Send;
    /// Le chemin (suivi s'il est un lien) est un dossier.
    fn is_dir(&self, path: &str) -> impl Future<Output = Result<bool, AppError>> + Send;
    fn real_path(&self, path: &str) -> impl Future<Output = Result<String, AppError>> + Send;
    /// Commande fixe, interrompue à l'échéance.
    fn exec(
        &self,
        command: &'static str,
        timeout: Duration,
    ) -> impl Future<Output = Result<ExecOutput, AppError>> + Send;
}

enum DockerRoot {
    Unknown,
    Known(String),
    Failed(Instant),
}

/// Ce que la boucle garde d'un relevé à l'autre.
struct State {
    cpu: Option<(Instant, CpuTimes)>,
    net: Option<(Instant, NetCounters)>,
    cores: Option<u32>,
    docker_root: DockerRoot,
    usage_attempt: Option<Instant>,
}

/// Relève les ressources de la liaison de `profile_id` jusqu'à être
/// abandonnée (perte, déconnexion) ou que la session change de profil.
pub async fn run<P: Probe>(hub: Arc<ResourceHub>, probe: Arc<P>, profile_id: String) {
    let mut foreground = hub.watch_foreground();
    let mut state = State {
        cpu: None,
        net: None,
        cores: None,
        docker_root: DockerRoot::Unknown,
        usage_attempt: None,
    };
    // Répartition automatique : tâche à part, abandonnée avec la boucle.
    let mut usage: JoinSet<()> = JoinSet::new();
    loop {
        let surveyed_foreground = *foreground.borrow_and_update();
        let survey = survey(&probe, surveyed_foreground, &mut state).await;
        let Some(update) = hub.publish(&profile_id, survey) else {
            return;
        };
        // Première tentative de la liaison : la répartition d'une liaison
        // précédente (avant une perte) compte, par son heure.
        let usage_due = match state.usage_attempt {
            Some(attempt) => attempt.elapsed() >= USAGE_INTERVAL,
            None => hub.docker_usage().is_none_or(|u| {
                now_millis().saturating_sub(u.at) >= USAGE_INTERVAL.as_millis() as u64
            }),
        };
        if DOCKER_USAGE_AUTOMATIC
            && surveyed_foreground
            && usage.is_empty()
            && usage_due
            && docker_mount_level(&update) >= super::alerts::Level::Attention
        {
            state.usage_attempt = Some(Instant::now());
            let (hub, probe, profile_id) = (hub.clone(), probe.clone(), profile_id.clone());
            usage.spawn(async move {
                if let Ok(usage) = read_usage(&*probe).await {
                    hub.set_docker_usage(&profile_id, usage);
                }
            });
        }

        let finished = Instant::now();
        loop {
            let now_foreground = *foreground.borrow();
            if now_foreground && !surveyed_foreground {
                break;
            }
            let wait = if now_foreground {
                FOREGROUND_INTERVAL
            } else {
                BACKGROUND_INTERVAL
            };
            tokio::select! {
                _ = tokio::time::sleep_until(finished + wait) => break,
                changed = foreground.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                Some(_) = usage.join_next(), if !usage.is_empty() => {}
            }
        }
    }
}

/// Une mesure, bornée par `MEASURE_TIMEOUT`.
async fn timed<T>(future: impl Future<Output = Result<T, AppError>>) -> Result<T, AppError> {
    tokio::time::timeout(MEASURE_TIMEOUT, future)
        .await
        .unwrap_or_else(|_| Err(AppError::network("délai dépassé")))
}

async fn survey<P: Probe>(probe: &Arc<P>, foreground: bool, state: &mut State) -> Survey {
    if !foreground {
        let (memory, disks) = tokio::join!(memory(&**probe), disks(probe.clone()));
        return Survey {
            at: now_millis(),
            foreground,
            cores: state.cores,
            cpu: Measure::NotRead,
            docker_mount: docker_mount(state, &disks),
            memory,
            load: Measure::NotRead,
            network: Measure::NotRead,
            disks,
            containers: Measure::NotRead,
        };
    }
    let (stat, loadavg, net_dev, memory, disks, stats, ()) = tokio::join!(
        timed(probe.read_file("/proc/stat")),
        timed(probe.read_file("/proc/loadavg")),
        timed(probe.read_file("/proc/net/dev")),
        memory(&**probe),
        disks(probe.clone()),
        timed(probe.exec(containers::STATS_COMMAND, STATS_TIMEOUT)),
        resolve_docker_root(&**probe, state),
    );
    let now = Instant::now();
    let cpu = match stat.map(|text| proc::parse_cpu_times(&text)) {
        Ok(Some(times)) => {
            state.cores = Some(times.cores);
            let usage = fresh(state.cpu, now).and_then(|previous| proc::cpu_usage(previous, times));
            state.cpu = Some((now, times));
            usage.map_or(Measure::Pending, |value| Measure::Ok { value })
        }
        Ok(None) => Measure::Unavailable {
            reason: "/proc/stat illisible".into(),
        },
        Err(error) => Measure::unavailable(&error),
    };
    let load = match (loadavg, state.cores) {
        (Ok(text), Some(cores)) => proc::parse_loadavg(&text, cores).map_or(
            Measure::Unavailable {
                reason: "/proc/loadavg illisible".into(),
            },
            |value| Measure::Ok { value },
        ),
        (Ok(_), None) => Measure::Unavailable {
            reason: "nombre de cœurs inconnu".into(),
        },
        (Err(error), _) => Measure::unavailable(&error),
    };
    let network = match net_dev.map(|text| proc::parse_net_dev(&text)) {
        Ok(Ok(counters)) => {
            let rates = state
                .net
                .as_ref()
                .filter(|(at, _)| now - *at <= BASELINE_MAX_AGE)
                .and_then(|(at, previous)| {
                    proc::net_rates(previous, &counters, (now - *at).as_secs_f64())
                });
            state.net = Some((now, counters));
            rates.map_or(Measure::Pending, |value| Measure::Ok { value })
        }
        Ok(Err(reason)) => Measure::Unavailable { reason },
        Err(error) => Measure::unavailable(&error),
    };
    let memory_total = memory.value().map(|m| m.total);
    let containers = match stats {
        Ok(output) if output.success() => Measure::Ok {
            value: containers::parse_stats(&output.stdout_text(), memory_total),
        },
        Ok(output) => Measure::unavailable(&crate::docker::failure(&output)),
        Err(error) => Measure::unavailable(&error),
    };
    Survey {
        at: now_millis(),
        foreground,
        cores: state.cores,
        cpu,
        docker_mount: docker_mount(state, &disks),
        memory,
        load,
        network,
        disks,
        containers,
    }
}

/// Base d'une mesure par différence, si elle est récente.
fn fresh(previous: Option<(Instant, CpuTimes)>, now: Instant) -> Option<CpuTimes> {
    previous
        .filter(|(at, _)| now - *at <= BASELINE_MAX_AGE)
        .map(|(_, times)| times)
}

async fn memory<P: Probe>(probe: &P) -> Measure<proc::MemoryUsage> {
    match timed(probe.read_file("/proc/meminfo")).await {
        Ok(text) => match proc::parse_meminfo(&text) {
            Ok(value) => Measure::Ok { value },
            Err(reason) => Measure::Unavailable { reason },
        },
        Err(error) => Measure::unavailable(&error),
    }
}

/// Exécute `task` sur chaque élément en parallèle ; résultats dans l'ordre.
async fn in_order<T, R, Fut>(items: Vec<T>, task: impl Fn(T) -> Fut) -> Vec<R>
where
    R: Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
{
    let mut running = JoinSet::new();
    let count = items.len();
    for (index, item) in items.into_iter().enumerate() {
        let future = task(item);
        running.spawn(async move { (index, future.await) });
    }
    let mut results: Vec<Option<R>> = (0..count).map(|_| None).collect();
    while let Some(joined) = running.join_next().await {
        if let Ok((index, result)) = joined {
            results[index] = Some(result);
        }
    }
    results.into_iter().flatten().collect()
}

async fn disks<P: Probe>(probe: Arc<P>) -> Measure<Vec<DiskUsage>> {
    let text = match timed(probe.read_file("/proc/mounts")).await {
        Ok(text) => text,
        Err(error) => return Measure::unavailable(&error),
    };
    let candidates = proc::real_mounts(proc::parse_mounts(&text));
    // Fichier monté par un montage lié (`/etc/hosts` dans un conteneur) :
    // écarté. Un `stat` refusé laisse le point de montage, que `statvfs`
    // signalera.
    let checked = in_order(candidates, |mount: Mount| {
        let probe = probe.clone();
        async move {
            let is_dir = timed(probe.is_dir(&mount.point)).await.unwrap_or(true);
            is_dir.then_some(mount)
        }
    })
    .await;
    let mounts = proc::dedupe_devices(checked.into_iter().flatten().collect());
    let usages = in_order(mounts, |mount: Mount| {
        let probe = probe.clone();
        async move {
            let space = match timed(probe.fs_stats(&mount.point)).await {
                Ok(stats) => match proc::disk_space(stats) {
                    Some(value) => Measure::Ok { value },
                    // Système sans bloc : pas un disque.
                    None => return None,
                },
                Err(error) => Measure::unavailable(&error),
            };
            Some(DiskUsage {
                mount_point: mount.point,
                device: mount.device,
                fs_type: mount.fs_type,
                space,
            })
        }
    })
    .await;
    Measure::Ok {
        value: usages.into_iter().flatten().collect(),
    }
}

/// Lit la racine de Docker une fois par liaison (nouvelle tentative après
/// un échec, au plus tôt 5 minutes plus tard).
async fn resolve_docker_root<P: Probe>(probe: &P, state: &mut State) {
    let due = match state.docker_root {
        DockerRoot::Unknown => true,
        DockerRoot::Failed(at) => at.elapsed() >= ROOT_DIR_RETRY,
        DockerRoot::Known(_) => false,
    };
    if !due {
        return;
    }
    let root = async {
        let output = probe
            .exec(containers::ROOT_DIR_COMMAND, ROOT_DIR_TIMEOUT)
            .await
            .ok()?;
        let path = output
            .success()
            .then(|| containers::parse_root_dir(&output.stdout_text()))??;
        // Un lien vers un autre disque est suivi.
        Some(timed(probe.real_path(&path)).await.unwrap_or(path))
    };
    state.docker_root = match root.await {
        Some(path) => DockerRoot::Known(path),
        None => DockerRoot::Failed(Instant::now()),
    };
}

fn docker_mount(state: &State, disks: &Measure<Vec<DiskUsage>>) -> Option<String> {
    let DockerRoot::Known(root) = &state.docker_root else {
        return None;
    };
    let disks = disks.value()?;
    proc::mount_containing(root, disks.iter().map(|d| d.mount_point.as_str())).map(str::to_owned)
}

async fn read_usage<P: Probe>(probe: &P) -> Result<DockerUsage, AppError> {
    let output = probe.exec(containers::USAGE_COMMAND, USAGE_TIMEOUT).await?;
    if !output.success() {
        return Err(crate::docker::failure(&output));
    }
    containers::parse_usage(&output.stdout_text(), now_millis()).map_err(AppError::io)
}

// ---------------------------------------------------------------------------
// Sonde sur la liaison
// ---------------------------------------------------------------------------

/// `Probe` sur la session SFTP partagée et les canaux de la liaison.
pub struct RemoteProbe {
    connection: Arc<Connection>,
    fs: Arc<SftpFs>,
}

impl RemoteProbe {
    pub fn new(connection: Arc<Connection>) -> Arc<Self> {
        Arc::new(Self {
            fs: SftpFs::shared(connection.clone()),
            connection,
        })
    }
}

impl Probe for RemoteProbe {
    async fn read_file(&self, path: &str) -> Result<String, AppError> {
        let file = self.fs.open_read(path).await?;
        let mut content = Vec::new();
        file.take(PROC_FILE_LIMIT)
            .read_to_end(&mut content)
            .await
            .map_err(|e| AppError::io(format!("{path} : lecture impossible ({e})")))?;
        Ok(String::from_utf8_lossy(&content).into_owned())
    }

    async fn fs_stats(&self, path: &str) -> Result<FsStats, AppError> {
        crate::sftp::fs_stats(&self.connection, path).await
    }

    async fn is_dir(&self, path: &str) -> Result<bool, AppError> {
        Ok(self
            .fs
            .stat(path)
            .await?
            .is_some_and(|meta| meta.kind == crate::sftp::EntryKind::Dir))
    }

    async fn real_path(&self, path: &str) -> Result<String, AppError> {
        self.fs.real_path(path).await
    }

    async fn exec(&self, command: &'static str, timeout: Duration) -> Result<ExecOutput, AppError> {
        self.connection
            .exec_until(command, Deadline::after(timeout))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::alerts::Level;
    use crate::resources::settings::Thresholds;
    use crate::resources::ResourceUpdate;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    const STATS_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// Sonde simulée : chaque lecture et commande dure `latency` ; compte les
    /// opérations en cours pour vérifier l'absence de chevauchement.
    struct FakeProbe {
        latency: Duration,
        files: Mutex<HashMap<String, String>>,
        stat_ticks: AtomicUsize,
        commands: Mutex<Vec<&'static str>>,
        /// Lectures de /proc/meminfo en cours : une par relevé, quel que
        /// soit le mode. Deux à la fois prouveraient un chevauchement.
        meminfo_running: AtomicUsize,
        max_meminfo_running: AtomicUsize,
        disk_percent_used: Mutex<u64>,
    }

    impl FakeProbe {
        fn new(latency: Duration) -> Arc<Self> {
            let files = HashMap::from([
                ("/proc/meminfo".to_owned(), "MemTotal: 1000 kB\nMemAvailable: 400 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n".to_owned()),
                ("/proc/loadavg".to_owned(), "0.40 0.80 0.30 1/100 1\n".to_owned()),
                ("/proc/mounts".to_owned(), "/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /var/lib/docker ext4 rw 0 0\n/dev/sdb1 /etc/hosts ext4 rw 0 0\ntmpfs /run tmpfs rw 0 0\n".to_owned()),
            ]);
            Arc::new(Self {
                latency,
                files: Mutex::new(files),
                stat_ticks: AtomicUsize::new(0),
                commands: Mutex::new(Vec::new()),
                meminfo_running: AtomicUsize::new(0),
                max_meminfo_running: AtomicUsize::new(0),
                disk_percent_used: Mutex::new(50),
            })
        }

        fn commands(&self, command: &str) -> usize {
            self.commands
                .lock()
                .unwrap()
                .iter()
                .filter(|c| **c == command)
                .count()
        }

        async fn operation(&self) {
            tokio::time::sleep(self.latency).await;
        }
    }

    impl Probe for FakeProbe {
        async fn read_file(&self, path: &str) -> Result<String, AppError> {
            if path == "/proc/stat" {
                // Lecture de /proc/stat : marque un relevé complet.
                let tick = self.stat_ticks.fetch_add(1, Ordering::SeqCst) as u64 + 1;
                self.operation().await;
                return Ok(format!(
                    "cpu {} 0 0 {} 0 0 0 0 0 0\ncpu0 1 0 0 1 0 0 0 0 0 0\ncpu1 1 0 0 1 0 0 0 0 0 0\n",
                    tick * 25,
                    tick * 75
                ));
            }
            if path == "/proc/net/dev" {
                let tick = self.stat_ticks.load(Ordering::SeqCst) as u64;
                return Ok(format!(
                    "h\nh\n  eth0: {} 0 0 0 0 0 0 0 {} 0 0 0 0 0 0 0\n",
                    tick * 5000,
                    tick * 1000
                ));
            }
            let meminfo = path == "/proc/meminfo";
            if meminfo {
                let running = self.meminfo_running.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_meminfo_running
                    .fetch_max(running, Ordering::SeqCst);
            }
            self.operation().await;
            if meminfo {
                self.meminfo_running.fetch_sub(1, Ordering::SeqCst);
            }
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| AppError::not_found(path, crate::error::FileSide::Remote))
        }

        async fn fs_stats(&self, path: &str) -> Result<FsStats, AppError> {
            let used = if path == "/var/lib/docker" {
                *self.disk_percent_used.lock().unwrap()
            } else {
                10
            };
            Ok(FsStats {
                blocks: 100,
                blocks_free: 100 - used,
                blocks_avail: 100 - used,
                fragment_size: 1024,
            })
        }

        async fn is_dir(&self, path: &str) -> Result<bool, AppError> {
            Ok(path != "/etc/hosts")
        }

        async fn real_path(&self, path: &str) -> Result<String, AppError> {
            Ok(path.to_owned())
        }

        async fn exec(&self, command: &'static str, _: Duration) -> Result<ExecOutput, AppError> {
            self.commands.lock().unwrap().push(command);
            self.operation().await;
            let stdout = match command {
                containers::ROOT_DIR_COMMAND => "/var/lib/docker\n".to_owned(),
                containers::STATS_COMMAND => format!(
                    r#"{{"ID":"{STATS_ID}","Name":"web","CPUPerc":"1.00%","MemUsage":"100KiB / 200KiB"}}"#
                ),
                containers::USAGE_COMMAND => [
                    r#"{"Type":"Images","Size":"1GB"}"#,
                    r#"{"Type":"Containers","Size":"2GB"}"#,
                    r#"{"Type":"Local Volumes","Size":"3GB"}"#,
                    r#"{"Type":"Build Cache","Size":"0B"}"#,
                ]
                .join("\n"),
                _ => String::new(),
            };
            Ok(ExecOutput {
                exit_code: Some(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
            })
        }
    }

    struct Harness {
        hub: Arc<ResourceHub>,
        probe: Arc<FakeProbe>,
        /// Relevés reçus, avec l'instant (virtuel) de leur réception.
        updates: Arc<Mutex<Vec<(Instant, ResourceUpdate)>>>,
        task: tokio::task::JoinHandle<()>,
        dir: std::path::PathBuf,
    }

    impl Harness {
        fn start(latency: Duration) -> Self {
            let dir = std::env::temp_dir().join(format!("vpsctl-monitor-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let updates = Arc::new(Mutex::new(Vec::new()));
            let sink = {
                let updates = updates.clone();
                Arc::new(move |update| updates.lock().unwrap().push((Instant::now(), update)))
            };
            let hub = Arc::new(ResourceHub::new(&dir, sink));
            hub.begin("p");
            let probe = FakeProbe::new(latency);
            let task = tokio::spawn(run(hub.clone(), probe.clone(), "p".into()));
            Self {
                hub,
                probe,
                updates,
                task,
                dir,
            }
        }

        /// Relevés (sans les réémissions de la répartition Docker).
        fn surveys(&self) -> Vec<(Instant, bool)> {
            let mut surveys: Vec<(Instant, bool)> = Vec::new();
            let updates = self.updates.lock().unwrap();
            let mut previous_at = None;
            for (received, update) in updates.iter() {
                if previous_at != Some(update.at) || update.docker_usage.is_none() {
                    surveys.push((*received, update.foreground));
                }
                previous_at = Some(update.at);
            }
            surveys
        }

        fn last(&self) -> ResourceUpdate {
            self.updates.lock().unwrap().last().cloned().unwrap().1
        }

        fn stop(self) {
            self.task.abort();
            let _ = std::fs::remove_dir_all(self.dir);
        }
    }

    async fn advance(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    #[tokio::test(start_paused = true)]
    async fn background_reads_memory_and_disk_every_minute_without_docker() {
        let harness = Harness::start(Duration::from_millis(100));
        advance(Duration::from_secs(125)).await;
        let surveys = harness.surveys();
        assert_eq!(surveys.len(), 3, "à 0, 60 et 120 s");
        assert!(surveys.iter().all(|(_, foreground)| !foreground));
        let last = harness.last();
        assert!(matches!(last.cpu, Measure::NotRead));
        assert!(matches!(last.containers, Measure::NotRead));
        assert!(last.memory.value().is_some());
        let points: Vec<String> = last
            .disks
            .value()
            .unwrap()
            .iter()
            .map(|d| d.mount_point.clone())
            .collect();
        assert_eq!(points, ["/", "/var/lib/docker"]);
        assert!(
            harness.probe.commands.lock().unwrap().is_empty(),
            "aucune commande docker"
        );
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn foreground_surveys_every_five_seconds_after_the_previous_end() {
        let harness = Harness::start(Duration::from_secs(2));
        harness.hub.set_foreground(true);
        advance(Duration::from_secs(30)).await;
        let surveys = harness.surveys();
        // Un relevé dure 2 s (opérations en parallèle), puis 5 s d'attente.
        let foreground: Vec<Instant> = surveys
            .iter()
            .filter(|(_, f)| *f)
            .map(|(at, _)| *at)
            .collect();
        assert!(foreground.len() >= 3);
        for pair in foreground.windows(2) {
            assert_eq!(pair[1] - pair[0], Duration::from_secs(7), "{pair:?}");
        }
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn surveys_never_overlap_even_when_slow() {
        // Chaque opération dure 12 s : plus que l'intervalle de 5 s.
        let harness = Harness::start(Duration::from_secs(12));
        harness.hub.set_foreground(true);
        for _ in 0..10 {
            advance(Duration::from_secs(3)).await;
            harness.hub.set_foreground(false);
            harness.hub.set_foreground(true);
        }
        advance(Duration::from_secs(60)).await;
        assert_eq!(harness.probe.max_meminfo_running.load(Ordering::SeqCst), 1);
        // Le dernier relevé complet peut être encore en cours.
        let ticks = harness.probe.stat_ticks.load(Ordering::SeqCst);
        let foreground = harness.surveys().iter().filter(|(_, f)| *f).count();
        assert!(
            ticks == foreground || ticks == foreground + 1,
            "{ticks} {foreground}"
        );
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn switching_to_foreground_surveys_at_once() {
        let harness = Harness::start(Duration::from_millis(100));
        advance(Duration::from_secs(10)).await;
        assert_eq!(harness.surveys().len(), 1);
        harness.hub.set_foreground(true);
        advance(Duration::from_secs(1)).await;
        let surveys = harness.surveys();
        assert_eq!(surveys.len(), 2);
        assert!(surveys[1].1);
        // Retour à l'arrière-plan : plus de relevé complet ni de docker stats.
        harness.hub.set_foreground(false);
        advance(Duration::from_secs(1)).await;
        let stats_before = harness.probe.commands(containers::STATS_COMMAND);
        advance(Duration::from_secs(55)).await;
        assert_eq!(
            harness.probe.commands(containers::STATS_COMMAND),
            stats_before
        );
        advance(Duration::from_secs(10)).await;
        assert!(!harness.last().foreground);
        assert_eq!(
            harness.probe.commands(containers::STATS_COMMAND),
            stats_before
        );
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn differential_measures_need_two_surveys() {
        let harness = Harness::start(Duration::from_millis(100));
        harness.hub.set_foreground(true);
        advance(Duration::from_secs(1)).await;
        let first = harness.last();
        assert!(matches!(first.cpu, Measure::Pending));
        assert!(matches!(first.network, Measure::Pending));
        assert!(first.load.value().is_some());
        assert_eq!(first.docker_mount.as_deref(), Some("/var/lib/docker"));
        assert_eq!(first.containers.value().unwrap()[0].name, "web");
        advance(Duration::from_secs(5)).await;
        let second = harness.last();
        // 25 jiffies actifs sur 100 entre deux relevés.
        assert!((second.cpu.value().unwrap() - 25.0).abs() < 1e-9);
        assert!(second.network.value().unwrap().received > 0.0);

        // Retour après une longue absence : de nouveau en attente.
        harness.hub.set_foreground(false);
        advance(Duration::from_secs(70)).await;
        harness.hub.set_foreground(true);
        advance(Duration::from_secs(1)).await;
        assert!(matches!(harness.last().cpu, Measure::Pending));
        assert_eq!(
            harness.probe.commands(containers::ROOT_DIR_COMMAND),
            1,
            "une fois par liaison"
        );
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn docker_usage_runs_at_the_alert_at_most_every_five_minutes() {
        let harness = Harness::start(Duration::from_millis(100));
        harness.hub.set_foreground(true);
        advance(Duration::from_secs(20)).await;
        assert_eq!(
            harness.probe.commands(containers::USAGE_COMMAND),
            0,
            "pas d'alerte"
        );

        *harness.probe.disk_percent_used.lock().unwrap() = 92;
        advance(Duration::from_secs(6)).await;
        assert_eq!(harness.last().levels.disks[1].level, Level::Critical);
        advance(Duration::from_secs(2)).await;
        assert_eq!(harness.probe.commands(containers::USAGE_COMMAND), 1);
        assert_eq!(harness.last().docker_usage.unwrap().volumes, 3_000_000_000);

        advance(Duration::from_secs(240)).await;
        assert_eq!(harness.probe.commands(containers::USAGE_COMMAND), 1);
        advance(Duration::from_secs(70)).await;
        assert_eq!(harness.probe.commands(containers::USAGE_COMMAND), 2);
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn another_mount_in_alert_runs_no_docker_usage() {
        let harness = Harness::start(Duration::from_millis(100));
        *harness.probe.disk_percent_used.lock().unwrap() = 5;
        let mut thresholds = Thresholds::default();
        // `/` à 10 % est en alerte, `/var/lib/docker` à 5 % reste nominal.
        thresholds.disk.attention = 8.0;
        thresholds.disk.critical = 60.0;
        harness.hub.save_thresholds("p", thresholds).unwrap();
        harness.hub.set_foreground(true);
        advance(Duration::from_secs(20)).await;
        let last = harness.last();
        assert_eq!(last.levels.disks[0].level, Level::Attention);
        assert_eq!(last.levels.disks[1].level, Level::Nominal);
        assert_eq!(harness.probe.commands(containers::USAGE_COMMAND), 0);
        harness.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_the_session_changes_profile() {
        let harness = Harness::start(Duration::from_millis(100));
        advance(Duration::from_secs(1)).await;
        harness.hub.begin("autre");
        advance(Duration::from_secs(61)).await;
        assert!(harness.task.is_finished());
        let _ = std::fs::remove_dir_all(&harness.dir);
    }
}
