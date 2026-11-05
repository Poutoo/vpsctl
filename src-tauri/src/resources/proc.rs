//! Fonctions pures : fichiers du noyau (`/proc/stat`, `meminfo`, `loadavg`,
//! `net/dev`, `mounts`) et occupation d'un système de fichiers (`statvfs`).

use serde::Serialize;
use ts_rs::TS;

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

/// Compteurs cumulés de la ligne `cpu` de `/proc/stat` (en jiffies) et nombre
/// de cœurs (lignes `cpuN`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub total: u64,
    pub idle: u64,
    pub cores: u32,
}

pub fn parse_cpu_times(stat: &str) -> Option<CpuTimes> {
    let mut sums = None;
    let mut cores = 0u32;
    for line in stat.lines() {
        let mut fields = line.split_ascii_whitespace();
        match fields.next() {
            Some("cpu") => {
                let values = fields
                    .map(|f| f.parse::<u64>().ok())
                    .collect::<Option<Vec<_>>>()?;
                if values.len() < 4 {
                    return None;
                }
                // user nice system idle iowait irq softirq steal : guest et
                // guest_nice sont déjà comptés dans user et nice.
                let total = values.iter().take(8).sum();
                let idle = values[3] + values.get(4).copied().unwrap_or(0);
                sums = Some((total, idle));
            }
            Some(name) if is_core_line(name) => cores += 1,
            _ => {}
        }
    }
    let (total, idle) = sums?;
    (cores > 0).then_some(CpuTimes { total, idle, cores })
}

fn is_core_line(name: &str) -> bool {
    name.strip_prefix("cpu")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Utilisation en % entre deux relevés. `None` si un compteur a reculé
/// (redémarrage) ou n'a pas avancé.
pub fn cpu_usage(previous: CpuTimes, current: CpuTimes) -> Option<f64> {
    let total = current.total.checked_sub(previous.total)?;
    let idle = current.idle.checked_sub(previous.idle)?;
    if total == 0 || idle > total {
        return None;
    }
    Some(100.0 * (1.0 - idle as f64 / total as f64))
}

// ---------------------------------------------------------------------------
// Mémoire
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryUsage {
    #[ts(type = "number")]
    pub total: u64,
    /// `MemTotal − MemAvailable` : le cache récupérable n'est pas compté.
    #[ts(type = "number")]
    pub used: u64,
    pub percent: f64,
    /// `null` si `SwapTotal` ou `SwapFree` manque (« non mesurable »).
    pub swap: Option<SwapUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SwapUsage {
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub used: u64,
}

pub fn parse_meminfo(text: &str) -> Result<MemoryUsage, String> {
    let field = |name: &str| -> Option<u64> {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(name)?.strip_prefix(':')?;
            let kib = rest.split_ascii_whitespace().next()?.parse::<u64>().ok()?;
            kib.checked_mul(1024)
        })
    };
    let total = field("MemTotal").ok_or("MemTotal absent de /proc/meminfo")?;
    let available = field("MemAvailable").ok_or("MemAvailable absent de /proc/meminfo")?;
    if total == 0 {
        return Err("MemTotal nul dans /proc/meminfo".into());
    }
    let used = total.saturating_sub(available);
    let swap = match (field("SwapTotal"), field("SwapFree")) {
        (Some(total), Some(free)) => Some(SwapUsage {
            total,
            used: total.saturating_sub(free),
        }),
        _ => None,
    };
    Ok(MemoryUsage {
        total,
        used,
        percent: 100.0 * used as f64 / total as f64,
        swap,
    })
}

// ---------------------------------------------------------------------------
// Charge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoadAverage {
    /// 1, 5 et 15 minutes, telles que dans `/proc/loadavg`.
    pub raw: [f64; 3],
    /// Les mêmes, divisées par le nombre de cœurs.
    pub per_core: [f64; 3],
}

pub fn parse_loadavg(text: &str, cores: u32) -> Option<LoadAverage> {
    let mut fields = text.split_ascii_whitespace();
    let mut raw = [0.0; 3];
    for value in &mut raw {
        *value = fields
            .next()?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())?;
    }
    let cores = f64::from(cores.max(1));
    Some(LoadAverage {
        raw,
        per_core: raw.map(|v| v / cores),
    })
}

// ---------------------------------------------------------------------------
// Réseau
// ---------------------------------------------------------------------------

/// Octets cumulés des interfaces comptées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetCounters {
    pub interfaces: Vec<String>,
    pub received: u64,
    pub sent: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkRates {
    pub interfaces: Vec<String>,
    /// Octets par seconde.
    pub received: f64,
    pub sent: f64,
}

/// Interfaces comptées : toutes sauf la boucle locale et les ponts et paires
/// virtuelles de Docker.
pub fn counted_interface(name: &str) -> bool {
    name != "lo" && name != "docker0" && !name.starts_with("veth") && !name.starts_with("br-")
}

pub fn parse_net_dev(text: &str) -> Result<NetCounters, String> {
    let mut counters = NetCounters {
        interfaces: Vec::new(),
        received: 0,
        sent: 0,
    };
    // Deux lignes d'en-tête, puis « nom: reçus (8 champs) émis (8 champs) ».
    for line in text.lines().skip(2) {
        let Some((name, values)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !counted_interface(name) {
            continue;
        }
        let values: Vec<u64> = values
            .split_ascii_whitespace()
            .map(|v| v.parse().ok())
            .collect::<Option<_>>()
            .ok_or_else(|| format!("compteurs illisibles pour {name}"))?;
        if values.len() < 9 {
            return Err(format!("compteurs incomplets pour {name}"));
        }
        counters.interfaces.push(name.to_owned());
        counters.received = counters.received.saturating_add(values[0]);
        counters.sent = counters.sent.saturating_add(values[8]);
    }
    if counters.interfaces.is_empty() {
        return Err("aucune interface réseau à compter sur ce serveur".into());
    }
    Ok(counters)
}

/// Débits entre deux relevés. `None` si les interfaces ont changé ou qu'un
/// compteur a reculé.
pub fn net_rates(
    previous: &NetCounters,
    current: &NetCounters,
    seconds: f64,
) -> Option<NetworkRates> {
    if previous.interfaces != current.interfaces || seconds <= 0.0 {
        return None;
    }
    let received = current.received.checked_sub(previous.received)?;
    let sent = current.sent.checked_sub(previous.sent)?;
    Some(NetworkRates {
        interfaces: current.interfaces.clone(),
        received: received as f64 / seconds,
        sent: sent as f64 / seconds,
    })
}

// ---------------------------------------------------------------------------
// Points de montage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub device: String,
    pub point: String,
    pub fs_type: String,
}

/// Systèmes de fichiers sans espace disque propre.
const PSEUDO_FS: &[&str] = &[
    "tmpfs",
    "devtmpfs",
    "overlay",
    "squashfs",
    "proc",
    "sysfs",
    "cgroup",
    "cgroup2",
    "devpts",
    "mqueue",
    "debugfs",
    "tracefs",
    "securityfs",
    "pstore",
    "bpf",
    "configfs",
    "fusectl",
    "hugetlbfs",
    "autofs",
    "binfmt_misc",
    "rpc_pipefs",
    "nsfs",
    "ramfs",
    "efivarfs",
    "fuse.lxcfs",
    "fuse.portal",
    "nfsd",
];

pub fn parse_mounts(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            Some(Mount {
                device: unescape(fields.next()?),
                point: unescape(fields.next()?),
                fs_type: unescape(fields.next()?),
            })
        })
        .collect()
}

/// `/proc/mounts` écrit l'espace, la tabulation, le retour à la ligne et la
/// barre oblique inverse en octal (`\040`).
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes
            .get(i + 1..i + 4)
            .filter(|d| bytes[i] == b'\\' && d.iter().all(|b| (b'0'..=b'7').contains(b)));
        match octal {
            Some(digits) => {
                let value = digits
                    .iter()
                    .fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
                out.push(u8::try_from(value).unwrap_or(b'?'));
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Montages qui ont un espace disque propre (pseudo-systèmes exclus).
pub fn real_mounts(mounts: Vec<Mount>) -> Vec<Mount> {
    mounts
        .into_iter()
        .filter(|m| !PSEUDO_FS.contains(&m.fs_type.as_str()) && m.point.starts_with('/'))
        .collect()
}

/// Un seul point de montage par périphérique (montage lié, sous-volume) : le
/// plus court, à la place du premier rencontré.
pub fn dedupe_devices(mounts: Vec<Mount>) -> Vec<Mount> {
    let mut kept: Vec<Mount> = Vec::new();
    for mount in mounts {
        match kept.iter_mut().find(|k| k.device == mount.device) {
            Some(existing) if mount.point.len() < existing.point.len() => *existing = mount,
            Some(_) => {}
            None => kept.push(mount),
        }
    }
    kept
}

/// Point de montage qui contient `path` : le plus long préfixe, composant
/// par composant (`/var/lib` ne contient pas `/var/library`).
pub fn mount_containing<'a>(
    path: &str,
    points: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    points
        .into_iter()
        .filter(|point| {
            *point == "/"
                || path == *point
                || path
                    .strip_prefix(point)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|point| point.len())
}

// ---------------------------------------------------------------------------
// Occupation d'un système de fichiers
// ---------------------------------------------------------------------------

/// Champs utiles de `statvfs`, en blocs de `fragment_size` octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsStats {
    pub blocks: u64,
    pub blocks_free: u64,
    pub blocks_avail: u64,
    pub fragment_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiskSpace {
    #[ts(type = "number")]
    pub total: u64,
    /// Espace qu'un compte non root peut écrire (`bavail`).
    #[ts(type = "number")]
    pub free: u64,
    pub percent: f64,
}

/// Occupation calculée comme `df` : la réserve de root n'est ni libre ni
/// utilisée. `None` pour un système sans bloc.
pub fn disk_space(stats: FsStats) -> Option<DiskSpace> {
    if stats.blocks == 0 {
        return None;
    }
    let size = stats.fragment_size;
    let used = stats
        .blocks
        .saturating_sub(stats.blocks_free)
        .saturating_mul(size);
    let free = stats.blocks_avail.saturating_mul(size);
    let base = used.saturating_add(free);
    Some(DiskSpace {
        total: stats.blocks.saturating_mul(size),
        free,
        percent: if base == 0 {
            0.0
        } else {
            100.0 * used as f64 / base as f64
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  4705 356 584 3699 23 0 21 5 0 0\n\
cpu0 1393 280 290 1858 12 0 10 2 0 0\n\
cpu1 3312 76 294 1841 11 0 11 3 0 0\n\
intr 114930548 113199788 3 0 5\n\
ctxt 1990473\n";

    #[test]
    fn cpu_times_sum_eight_fields_and_count_cores() {
        let times = parse_cpu_times(STAT).unwrap();
        assert_eq!(times.total, 4705 + 356 + 584 + 3699 + 23 + 21 + 5);
        assert_eq!(times.idle, 3699 + 23);
        assert_eq!(times.cores, 2);
    }

    #[test]
    fn cpu_times_reject_unreadable_or_coreless_files() {
        assert_eq!(parse_cpu_times(""), None);
        assert_eq!(parse_cpu_times("cpu 1 2 x 4\ncpu0 1 2 3 4\n"), None);
        assert_eq!(parse_cpu_times("cpu 1 2 3 4\n"), None);
    }

    #[test]
    fn cpu_usage_by_difference() {
        let a = CpuTimes {
            total: 1000,
            idle: 800,
            cores: 4,
        };
        let b = CpuTimes {
            total: 1100,
            idle: 870,
            cores: 4,
        };
        assert!((cpu_usage(a, b).unwrap() - 30.0).abs() < 1e-9);
        // Compteurs revenus en arrière (redémarrage) ou figés.
        assert_eq!(cpu_usage(b, a), None);
        assert_eq!(cpu_usage(a, a), None);
    }

    #[test]
    fn memory_uses_mem_available_not_mem_free() {
        let text = "MemTotal:        8000000 kB\nMemFree:          500000 kB\n\
MemAvailable:    5000000 kB\nSwapTotal:       2000000 kB\nSwapFree:        1500000 kB\n";
        let memory = parse_meminfo(text).unwrap();
        assert_eq!(memory.total, 8_000_000 * 1024);
        assert_eq!(memory.used, 3_000_000 * 1024);
        assert!((memory.percent - 37.5).abs() < 1e-9);
        assert_eq!(
            memory.swap,
            Some(SwapUsage {
                total: 2_000_000 * 1024,
                used: 500_000 * 1024
            })
        );
    }

    #[test]
    fn memory_without_swap_fields_or_mem_available() {
        let memory = parse_meminfo("MemTotal: 1000 kB\nMemAvailable: 250 kB\n").unwrap();
        assert_eq!(memory.swap, None);
        assert!((memory.percent - 75.0).abs() < 1e-9);
        let no_swap = parse_meminfo(
            "MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n",
        )
        .unwrap();
        assert_eq!(no_swap.swap, Some(SwapUsage { total: 0, used: 0 }));
        assert!(parse_meminfo("MemTotal: 1000 kB\nMemFree: 250 kB\n").is_err());
        assert!(parse_meminfo("").is_err());
    }

    #[test]
    fn load_average_per_core() {
        let load = parse_loadavg("2.70 3.10 2.20 3/412 12345\n", 4).unwrap();
        assert_eq!(load.raw, [2.70, 3.10, 2.20]);
        assert!((load.per_core[1] - 0.775).abs() < 1e-9);
        assert_eq!(parse_loadavg("2.70 x", 4), None);
        assert_eq!(parse_loadavg("", 4), None);
    }

    const NET_DEV: &str = "Inter-|   Receive                                                |  Transmit\n \
face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n    \
lo: 1000 10 0 0 0 0 0 0 1000 10 0 0 0 0 0 0\n  \
eth0: 5000 50 0 0 0 0 0 0 7000 70 0 0 0 0 0 0\n  \
wg0: 100 1 0 0 0 0 0 0 200 2 0 0 0 0 0 0\n\
docker0: 9 1 0 0 0 0 0 0 9 1 0 0 0 0 0 0\n\
vethab12: 9 1 0 0 0 0 0 0 9 1 0 0 0 0 0 0\n\
br-1f2e: 9 1 0 0 0 0 0 0 9 1 0 0 0 0 0 0\n";

    #[test]
    fn network_sums_counted_interfaces_only() {
        let counters = parse_net_dev(NET_DEV).unwrap();
        assert_eq!(counters.interfaces, ["eth0", "wg0"]);
        assert_eq!(counters.received, 5100);
        assert_eq!(counters.sent, 7200);
    }

    #[test]
    fn network_without_counted_interface_is_unavailable() {
        let only_lo = "h\nh\n    lo: 1 1 0 0 0 0 0 0 1 1 0 0 0 0 0 0\n";
        assert!(parse_net_dev(only_lo).is_err());
        assert!(parse_net_dev("").is_err());
        assert!(parse_net_dev("h\nh\n  eth0: 1 2 3\n").is_err());
    }

    #[test]
    fn network_rates_by_difference() {
        let a = parse_net_dev(NET_DEV).unwrap();
        let mut b = a.clone();
        b.received += 10_000;
        b.sent += 5_000;
        let rates = net_rates(&a, &b, 5.0).unwrap();
        assert_eq!((rates.received, rates.sent), (2000.0, 1000.0));
        assert_eq!(net_rates(&b, &a, 5.0), None);
        let mut other = b.clone();
        other.interfaces.push("eth1".into());
        assert_eq!(net_rates(&a, &other, 5.0), None);
    }

    const MOUNTS: &str = "sysfs /sys sysfs rw 0 0\n\
proc /proc proc rw 0 0\n\
/dev/sda1 / ext4 rw,relatime 0 0\n\
tmpfs /run tmpfs rw 0 0\n\
/dev/sdb1 /var/lib/docker ext4 rw 0 0\n\
overlay /var/lib/docker/overlay2/abc/merged overlay rw 0 0\n\
/dev/sda15 /boot/efi vfat rw 0 0\n\
/dev/loop0 /snap/core/1 squashfs ro 0 0\n\
/dev/sda1 /srv/lien ext4 rw 0 0\n\
/dev/sdc1 /mnt/mes\\040donn\\303\\251es ext4 rw 0 0\n";

    #[test]
    fn mounts_keep_real_filesystems_once_per_device() {
        let mounts = dedupe_devices(real_mounts(parse_mounts(MOUNTS)));
        let points: Vec<&str> = mounts.iter().map(|m| m.point.as_str()).collect();
        assert_eq!(
            points,
            ["/", "/var/lib/docker", "/boot/efi", "/mnt/mes données"]
        );
    }

    #[test]
    fn dedupe_prefers_the_shortest_mount_point() {
        let mounts = parse_mounts("/dev/sdd /etc/hosts ext4 rw 0 0\n/dev/sdd /e ext4 rw 0 0\n");
        let kept = dedupe_devices(mounts);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].point, "/e");
    }

    #[test]
    fn mount_containing_compares_whole_components() {
        let points = ["/", "/var/lib", "/var/lib/docker"];
        assert_eq!(
            mount_containing("/var/lib/docker", points),
            Some("/var/lib/docker")
        );
        assert_eq!(
            mount_containing("/var/lib/docker/volumes", points),
            Some("/var/lib/docker")
        );
        assert_eq!(mount_containing("/var/library", points), Some("/"));
        assert_eq!(mount_containing("/var/lib/x", ["/var/lib/docker"]), None);
    }

    #[test]
    fn disk_space_like_df() {
        // 100 blocs de 4 Kio : 60 libres dont 50 pour un compte non root.
        let space = disk_space(FsStats {
            blocks: 100,
            blocks_free: 60,
            blocks_avail: 50,
            fragment_size: 4096,
        })
        .unwrap();
        assert_eq!(space.total, 409_600);
        assert_eq!(space.free, 50 * 4096);
        assert!((space.percent - 100.0 * 40.0 / 90.0).abs() < 1e-9);
        assert_eq!(
            disk_space(FsStats {
                blocks: 0,
                blocks_free: 0,
                blocks_avail: 0,
                fragment_size: 4096
            }),
            None
        );
    }
}
