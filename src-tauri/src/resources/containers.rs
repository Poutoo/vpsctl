//! Fonctions pures : sorties de `docker stats`, `docker info` et
//! `docker system df` (avec et sans `-v`). Les commandes sont fixes : aucune
//! valeur de l'interface n'y entre.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::docker::is_container_id;

pub const STATS_COMMAND: &str = "docker stats --no-stream --no-trunc --format '{{json .}}'";
pub const ROOT_DIR_COMMAND: &str = "docker info --format '{{.DockerRootDir}}'";
pub const USAGE_COMMAND: &str = "docker system df --format '{{json .}}'";
pub const VOLUMES_COMMAND: &str = "docker system df -v --format '{{json .}}'";
/// Conteneurs et ce qu'ils montent (noms de volumes, chemins des montages
/// liés), séparés par des virgules. Un nom de conteneur ne contient jamais `|`.
pub const VOLUME_USERS_COMMAND: &str = "docker ps -a --no-trunc --format '{{.Names}}|{{.Mounts}}'";

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerUsage {
    pub id: String,
    pub name: String,
    /// En % d'un cœur (dépasse 100 sur plusieurs cœurs).
    pub cpu_percent: f64,
    #[ts(type = "number")]
    pub memory_used: u64,
    /// `null` : sans limite (Docker donne alors la mémoire du serveur).
    #[ts(type = "number | null")]
    pub memory_limit: Option<u64>,
}

/// Une limite d'au moins 99 % de la mémoire du serveur est l'absence de
/// limite : Docker la remplace par la mémoire de l'hôte, arrondie à
/// l'affichage.
const NO_LIMIT_RATIO: f64 = 0.99;

/// Lignes de `docker stats`, triées par mémoire décroissante. Une ligne
/// illisible est écartée.
pub fn parse_stats(output: &str, memory_total: Option<u64>) -> Vec<ContainerUsage> {
    let mut containers: Vec<ContainerUsage> = output
        .lines()
        .filter_map(|line| parse_stats_line(line, memory_total))
        .collect();
    containers.sort_by(|a, b| b.memory_used.cmp(&a.memory_used).then(a.name.cmp(&b.name)));
    containers
}

fn parse_stats_line(line: &str, memory_total: Option<u64>) -> Option<ContainerUsage> {
    let value: Value = serde_json::from_str(line).ok()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str);
    let id = text("ID")?;
    if !is_container_id(id) {
        return None;
    }
    let cpu_percent = text("CPUPerc")
        .and_then(|p| p.trim().trim_end_matches('%').parse::<f64>().ok())
        .filter(|p| p.is_finite())
        .unwrap_or(0.0);
    let (used, limit) = text("MemUsage")?.split_once(" / ")?;
    let limit = parse_size(limit).filter(|&l| l > 0);
    let no_limit = match (limit, memory_total) {
        (None, _) => true,
        (Some(limit), Some(total)) => limit as f64 >= total as f64 * NO_LIMIT_RATIO,
        (Some(_), None) => false,
    };
    Some(ContainerUsage {
        id: id.to_owned(),
        name: text("Name").unwrap_or(id).to_owned(),
        cpu_percent,
        memory_used: parse_size(used)?,
        memory_limit: if no_limit { None } else { limit },
    })
}

/// Taille écrite par `go-units` : décimale (`"14.2GB"`, `"1.5kB"`) pour
/// `docker system df`, binaire (`"1.2GiB"`) pour `docker stats`.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number.parse().ok()?;
    let factor: f64 = match unit.trim() {
        "B" | "" => 1.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        "PB" => 1e15,
        "KiB" => 1024.0,
        "MiB" => 1024f64.powi(2),
        "GiB" => 1024f64.powi(3),
        "TiB" => 1024f64.powi(4),
        "PiB" => 1024f64.powi(5),
        _ => return None,
    };
    let bytes = (number * factor).round();
    (bytes.is_finite() && bytes >= 0.0).then_some(bytes as u64)
}

/// Racine de Docker : un chemin absolu sur une ligne.
pub fn parse_root_dir(output: &str) -> Option<String> {
    let path = output.trim();
    (path.starts_with('/') && !path.contains('\n')).then(|| path.to_owned())
}

// ---------------------------------------------------------------------------
// docker system df
// ---------------------------------------------------------------------------

/// Répartition de l'espace de Docker, en octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockerUsage {
    /// Fin de la lecture (ms depuis l'epoch).
    #[ts(type = "number")]
    pub at: u64,
    #[ts(type = "number")]
    pub images: u64,
    #[ts(type = "number")]
    pub containers: u64,
    #[ts(type = "number")]
    pub volumes: u64,
    #[ts(type = "number")]
    pub build_cache: u64,
}

/// Quatre lignes JSON (`Images`, `Containers`, `Local Volumes`,
/// `Build Cache`). Une catégorie manquante ou illisible rend la sortie
/// illisible.
pub fn parse_usage(output: &str, at: u64) -> Result<DockerUsage, String> {
    let mut sizes: HashMap<String, u64> = HashMap::new();
    for line in output.lines().filter(|l| !l.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)
            .map_err(|_| "sortie de docker system df illisible".to_owned())?;
        if let (Some(kind), Some(size)) = (
            value.get("Type").and_then(Value::as_str),
            value
                .get("Size")
                .and_then(Value::as_str)
                .and_then(parse_size),
        ) {
            sizes.insert(kind.to_owned(), size);
        }
    }
    let size = |kind: &str| {
        sizes
            .get(kind)
            .copied()
            .ok_or_else(|| format!("catégorie « {kind} » absente de docker system df"))
    };
    Ok(DockerUsage {
        at,
        images: size("Images")?,
        containers: size("Containers")?,
        volumes: size("Local Volumes")?,
        build_cache: size("Build Cache")?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeSize {
    pub name: String,
    /// `null` : taille non calculée par Docker (`N/A`).
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    pub containers: Vec<String>,
}

/// Tableau `Volumes` de `docker system df -v --format '{{json .}}'` (un seul
/// objet JSON), trié par taille décroissante.
pub fn parse_volumes(output: &str) -> Result<Vec<VolumeSize>, String> {
    let value: Value = serde_json::from_str(output.trim())
        .map_err(|_| "sortie de docker system df -v illisible".to_owned())?;
    let volumes = value
        .get("Volumes")
        .and_then(Value::as_array)
        .ok_or("liste des volumes absente de docker system df -v")?;
    let mut sizes: Vec<VolumeSize> = volumes
        .iter()
        .filter_map(|volume| {
            let name = volume.get("Name")?.as_str()?.to_owned();
            let size = match volume.get("Size") {
                Some(Value::String(text)) => parse_size(text),
                Some(Value::Number(n)) => n.as_u64(),
                _ => None,
            };
            Some(VolumeSize {
                name,
                size,
                containers: Vec::new(),
            })
        })
        .collect();
    sizes.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));
    Ok(sizes)
}

/// Rattache chaque volume aux conteneurs qui le montent (sortie de
/// `VOLUME_USERS_COMMAND`).
pub fn attach_users(volumes: &mut [VolumeSize], users: &str) {
    for line in users.lines() {
        let Some((container, mounts)) = line.split_once('|') else {
            continue;
        };
        for mount in mounts.split(',').map(str::trim) {
            if let Some(volume) = volumes.iter_mut().find(|v| v.name == mount) {
                if !volume.containers.iter().any(|c| c == container) {
                    volume.containers.push(container.to_owned());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn stats_line(id: &str, name: &str, cpu: &str, mem: &str) -> String {
        format!(
            r#"{{"BlockIO":"0B / 0B","CPUPerc":"{cpu}","Container":"{id}","ID":"{id}","MemPerc":"1.00%","MemUsage":"{mem}","Name":"{name}","NetIO":"0B / 0B","PIDs":"1"}}"#
        )
    }

    #[test]
    fn stats_sorted_by_memory_with_limits() {
        let output = [
            stats_line(ID_A, "caddy", "0.60%", "41MiB / 256MiB"),
            stats_line(ID_B, "postgres-main", "22.00%", "2.4GiB / 3GiB"),
            "illisible".into(),
            stats_line("pas-un-id", "x", "1%", "1MiB / 2MiB"),
        ]
        .join("\n");
        let containers = parse_stats(&output, Some(8 * 1024u64.pow(3)));
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].name, "postgres-main");
        assert_eq!(containers[0].memory_limit, Some(3 * 1024u64.pow(3)));
        assert!((containers[0].cpu_percent - 22.0).abs() < 1e-9);
        assert_eq!(containers[1].memory_used, 41 * 1024 * 1024);
    }

    #[test]
    fn limit_equal_to_server_memory_means_no_limit() {
        let total = 8_138_000 * 1024; // MemTotal
        let output = stats_line(ID_A, "worker", "31%", "1.4GiB / 7.761GiB");
        assert_eq!(parse_stats(&output, Some(total))[0].memory_limit, None);
        // Mémoire du serveur inconnue : la limite affichée reste celle de Docker.
        assert!(parse_stats(&output, None)[0].memory_limit.is_some());
        let zero = stats_line(ID_A, "worker", "0%", "0B / 0B");
        assert_eq!(parse_stats(&zero, Some(total))[0].memory_limit, None);
    }

    #[test]
    fn sizes_in_decimal_and_binary_units() {
        assert_eq!(parse_size("14.2GB"), Some(14_200_000_000));
        assert_eq!(parse_size("1.5kB"), Some(1500));
        assert_eq!(parse_size("0B"), Some(0));
        assert_eq!(parse_size("1.2GiB"), Some(1_288_490_189));
        assert_eq!(parse_size("512MiB"), Some(512 * 1024 * 1024));
        assert_eq!(parse_size("N/A"), None);
        assert_eq!(parse_size("3 parsecs"), None);
    }

    #[test]
    fn root_dir_is_an_absolute_path() {
        assert_eq!(
            parse_root_dir("/var/lib/docker\n").as_deref(),
            Some("/var/lib/docker")
        );
        assert_eq!(parse_root_dir("erreur"), None);
        assert_eq!(parse_root_dir(""), None);
    }

    #[test]
    fn usage_reads_the_four_categories() {
        let output = r#"{"Active":"5","Reclaimable":"1.2GB (8%)","Size":"14.2GB","TotalCount":"12","Type":"Images"}
{"Active":"5","Reclaimable":"0B (0%)","Size":"3GB","TotalCount":"5","Type":"Containers"}
{"Active":"4","Reclaimable":"0B","Size":"61GB","TotalCount":"6","Type":"Local Volumes"}
{"Active":"0","Reclaimable":"14GB","Size":"14GB","TotalCount":"80","Type":"Build Cache"}
"#;
        let usage = parse_usage(output, 7).unwrap();
        assert_eq!(usage.images, 14_200_000_000);
        assert_eq!(usage.volumes, 61_000_000_000);
        assert_eq!(usage.build_cache, 14_000_000_000);
        assert_eq!(usage.at, 7);
        assert!(parse_usage("", 0).is_err());
        assert!(parse_usage("TYPE TOTAL\nImages 1", 0).is_err());
    }

    #[test]
    fn volumes_sorted_by_size_with_their_containers() {
        let output = r#"{"BuildCache":[],"Containers":[],"Images":[],"Volumes":[
{"Driver":"local","Links":"1","Name":"uploads","Size":"9GB"},
{"Driver":"local","Links":"1","Name":"pg_data","Size":"46GB"},
{"Driver":"local","Links":"0","Name":"orphelin","Size":"N/A"}]}"#
            .replace('\n', "");
        let mut volumes = parse_volumes(&output).unwrap();
        attach_users(
            &mut volumes,
            "postgres-main|pg_data,/srv/conf\napp-api|uploads\nbackup|pg_data,uploads\nsans-montage|\n",
        );
        let names: Vec<&str> = volumes.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["pg_data", "uploads", "orphelin"]);
        assert_eq!(volumes[0].containers, ["postgres-main", "backup"]);
        assert_eq!(volumes[2].size, None);
        assert!(parse_volumes("Images space usage:").is_err());
        assert!(parse_volumes("{}").is_err());
    }
}
