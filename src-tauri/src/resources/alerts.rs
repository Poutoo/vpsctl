//! Fonctions pures : niveaux d'alerte avec hystérésis.
//!
//! Les valeurs comparées sont arrondies comme à l'affichage (pourcentage
//! entier, charge par cœur au centième) : une alerte à 80 % se lève quand
//! l'écran affiche 80 %.

use serde::Serialize;
use ts_rs::TS;

use super::settings::{ThresholdPair, Thresholds};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Level {
    #[default]
    Nominal,
    Attention,
    Critical,
}

/// Marge de retombée : 3 points pour le disque et la mémoire.
pub const PERCENT_MARGIN: f64 = 3.0;
/// Marge de retombée de la charge, par cœur.
pub const LOAD_MARGIN: f64 = 0.1;
/// Tolérance des comparaisons (0,7 ou 0,1 ne tombent pas juste en binaire).
const EPSILON: f64 = 1e-9;

/// Le niveau monte dès que la valeur atteint un seuil ; il ne redescend que
/// lorsqu'elle passe à `seuil − marge` ou en dessous.
pub fn next_level(previous: Level, value: f64, pair: ThresholdPair, margin: f64) -> Level {
    let reached = |threshold: f64| value >= threshold - EPSILON;
    let held = |threshold: f64| value > threshold - margin + EPSILON;
    if reached(pair.critical) || (previous == Level::Critical && held(pair.critical)) {
        Level::Critical
    } else if reached(pair.attention) || (previous >= Level::Attention && held(pair.attention)) {
        Level::Attention
    } else {
        Level::Nominal
    }
}

/// Pourcentage tel qu'affiché.
pub fn displayed_percent(percent: f64) -> f64 {
    percent.round()
}

/// Charge par cœur telle qu'affichée.
pub fn displayed_load(per_core: f64) -> f64 {
    (per_core * 100.0).round() / 100.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MountLevel {
    pub mount_point: String,
    pub level: Level,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Levels {
    pub disks: Vec<MountLevel>,
    pub memory: Level,
    pub load: Level,
}

/// Valeurs mesurées d'un relevé ; `None` : non mesurée (le niveau précédent
/// est gardé).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Values {
    /// Pourcentage par point de montage mesuré.
    pub disks: Vec<(String, f64)>,
    /// Points de montage présents mais non mesurés (`statvfs` en échec).
    pub unmeasured_disks: Vec<String>,
    /// La liste des points de montage a été lue.
    pub mounts_read: bool,
    pub memory: Option<f64>,
    pub load_per_core: Option<f64>,
}

/// Niveaux d'un relevé à partir des précédents. Un point de montage qui
/// n'existe plus perd son niveau ; un point non mesuré garde le sien.
pub fn compute(previous: &Levels, values: &Values, thresholds: &Thresholds) -> Levels {
    let before = |point: &str| {
        previous
            .disks
            .iter()
            .find(|m| m.mount_point == point)
            .map_or(Level::Nominal, |m| m.level)
    };
    let disks = if values.mounts_read {
        let measured = values.disks.iter().map(|(point, percent)| MountLevel {
            mount_point: point.clone(),
            level: next_level(
                before(point),
                displayed_percent(*percent),
                thresholds.disk,
                PERCENT_MARGIN,
            ),
        });
        let kept = values.unmeasured_disks.iter().map(|point| MountLevel {
            mount_point: point.clone(),
            level: before(point),
        });
        measured.chain(kept).collect()
    } else {
        previous.disks.clone()
    };
    Levels {
        disks,
        memory: values.memory.map_or(previous.memory, |percent| {
            next_level(
                previous.memory,
                displayed_percent(percent),
                thresholds.memory,
                PERCENT_MARGIN,
            )
        }),
        load: values.load_per_core.map_or(previous.load, |load| {
            next_level(
                previous.load,
                displayed_load(load),
                thresholds.load,
                LOAD_MARGIN,
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISK: ThresholdPair = ThresholdPair {
        attention: 80.0,
        critical: 90.0,
    };
    const LOAD: ThresholdPair = ThresholdPair {
        attention: 1.0,
        critical: 2.0,
    };

    fn run(values: &[f64], pair: ThresholdPair, margin: f64) -> Vec<Level> {
        let mut level = Level::Nominal;
        values
            .iter()
            .map(|&v| {
                level = next_level(level, v, pair, margin);
                level
            })
            .collect()
    }

    #[test]
    fn raises_at_the_threshold() {
        use Level::*;
        assert_eq!(
            run(&[79.0, 80.0, 89.0, 90.0], DISK, PERCENT_MARGIN),
            [Nominal, Attention, Attention, Critical]
        );
        // Un saut direct au-delà du critique.
        assert_eq!(run(&[95.0], DISK, PERCENT_MARGIN), [Critical]);
    }

    #[test]
    fn falls_only_three_points_below() {
        use Level::*;
        assert_eq!(
            run(&[80.0, 78.0, 77.5, 77.0, 79.0, 80.0], DISK, PERCENT_MARGIN),
            [Attention, Attention, Attention, Nominal, Nominal, Attention]
        );
        assert_eq!(
            run(&[90.0, 88.0, 87.0, 76.0], DISK, PERCENT_MARGIN),
            [Critical, Critical, Attention, Nominal]
        );
    }

    #[test]
    fn no_flapping_around_the_threshold() {
        use Level::*;
        let oscillating = [80.0, 79.0, 80.0, 78.0, 79.5, 80.0, 79.0];
        assert!(run(&oscillating, DISK, PERCENT_MARGIN)
            .iter()
            .all(|&l| l == Attention));
        let below = [79.0, 78.0, 79.9, 77.0];
        assert!(run(&below, DISK, PERCENT_MARGIN)
            .iter()
            .all(|&l| l == Nominal));
    }

    #[test]
    fn load_margin_is_a_tenth_per_core() {
        use Level::*;
        assert_eq!(
            run(&[1.0, 0.95, 0.91, 0.9], LOAD, LOAD_MARGIN),
            [Attention, Attention, Attention, Nominal]
        );
        let tenths = ThresholdPair {
            attention: 0.7,
            critical: 1.3,
        };
        assert_eq!(run(&[0.7, 0.6], tenths, LOAD_MARGIN), [Attention, Nominal]);
        assert_eq!(run(&[0.69], tenths, LOAD_MARGIN), [Nominal]);
    }

    #[test]
    fn raised_threshold_keeps_or_drops_the_alert() {
        // Disque à 41 % : seuil à 41 (alerte), puis 43 (reste), puis 45 (retombe).
        let mut level = next_level(
            Level::Nominal,
            41.0,
            ThresholdPair {
                attention: 41.0,
                critical: 90.0,
            },
            PERCENT_MARGIN,
        );
        assert_eq!(level, Level::Attention);
        level = next_level(
            level,
            41.0,
            ThresholdPair {
                attention: 43.0,
                critical: 90.0,
            },
            PERCENT_MARGIN,
        );
        assert_eq!(level, Level::Attention);
        level = next_level(
            level,
            41.0,
            ThresholdPair {
                attention: 45.0,
                critical: 90.0,
            },
            PERCENT_MARGIN,
        );
        assert_eq!(level, Level::Nominal);
    }

    #[test]
    fn compares_displayed_values() {
        assert_eq!(displayed_percent(79.6), 80.0);
        assert_eq!(displayed_load(0.996), 1.0);
    }

    #[test]
    fn compute_keeps_levels_of_unmeasured_values() {
        let thresholds = Thresholds::default();
        let first = compute(
            &Levels::default(),
            &Values {
                disks: vec![("/".into(), 92.0), ("/boot".into(), 10.0)],
                mounts_read: true,
                memory: Some(86.0),
                load_per_core: Some(2.5),
                ..Values::default()
            },
            &thresholds,
        );
        assert_eq!(first.disks[0].level, Level::Critical);
        assert_eq!(first.memory, Level::Attention);
        assert_eq!(first.load, Level::Critical);

        // Arrière-plan : charge non relevée, /boot non mesuré, / retombé.
        let second = compute(
            &first,
            &Values {
                disks: vec![("/".into(), 50.0)],
                unmeasured_disks: vec!["/boot".into()],
                mounts_read: true,
                memory: Some(83.0),
                load_per_core: None,
            },
            &thresholds,
        );
        assert_eq!(
            second.disks[0],
            MountLevel {
                mount_point: "/".into(),
                level: Level::Nominal
            }
        );
        assert_eq!(second.disks[1].level, Level::Nominal);
        assert_eq!(second.memory, Level::Attention);
        assert_eq!(second.load, Level::Critical);

        // Points de montage illisibles : niveaux gardés.
        let third = compute(&first, &Values::default(), &thresholds);
        assert_eq!(third.disks, first.disks);
    }
}
