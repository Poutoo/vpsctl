//! Contenu envoyé à Discord. Fonctions pures, qui ne reçoivent que la phase
//! de la liaison et un horodatage : aucune donnée d'un profil, d'un serveur,
//! d'un fichier ou d'une commande ne peut y entrer.

use serde::Serialize;
use serde_json::{json, Value};
use ts_rs::TS;

/// État partagé : « En liaison » (AOS) ou « Au sol » (tous les autres cas,
/// perte de liaison comprise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkPhase {
    Linked,
    Ground,
}

/// Phase en cours et son début.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PhaseState {
    pub phase: LinkPhase,
    /// Début de la phase, en millisecondes depuis l'epoch Unix.
    #[ts(type = "number")]
    pub since: u64,
}

/// Nom de l'application sur le portail Discord, repris en infobulle.
pub const APP_NAME: &str = "Atlas · VPS Control";
pub const LINKED_LABEL: &str = "En liaison";
pub const GROUND_LABEL: &str = "Au sol";
/// Clé de l'image (papillon) déposée sur le portail Discord.
pub const IMAGE_KEY: &str = "papillon";

/// Activité affichée : l'état et la durée écoulée depuis le début de la
/// phase (en secondes, comme l'exemple de la documentation RPC).
pub fn activity(state: PhaseState) -> Value {
    let details = match state.phase {
        LinkPhase::Linked => LINKED_LABEL,
        LinkPhase::Ground => GROUND_LABEL,
    };
    json!({
        "details": details,
        "timestamps": { "start": state.since / 1000 },
        "assets": { "large_image": IMAGE_KEY, "large_text": APP_NAME },
    })
}

/// Commande `SET_ACTIVITY` ; `None` efface l'activité. `pid` permet à
/// Discord d'effacer la carte si l'application disparaît sans le faire.
pub fn set_activity(pid: u32, nonce: &str, activity: Option<Value>) -> Value {
    json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": pid, "activity": activity },
        "nonce": nonce,
    })
}

/// Poignée de main de l'IPC (version 1 du protocole).
pub fn handshake(client_id: &str) -> Value {
    json!({ "v": 1, "client_id": client_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;

    /// Toutes les chaînes et toutes les clés d'une valeur JSON.
    fn strings_and_keys(value: &Value, strings: &mut Vec<String>, keys: &mut Vec<String>) {
        match value {
            Value::String(s) => strings.push(s.clone()),
            Value::Array(items) => items
                .iter()
                .for_each(|v| strings_and_keys(v, strings, keys)),
            Value::Object(map) => {
                for (key, v) in map {
                    keys.push(key.clone());
                    strings_and_keys(v, strings, keys);
                }
            }
            _ => {}
        }
    }

    fn sent_frames(state: PhaseState) -> Vec<Value> {
        vec![
            handshake("123456789012345678"),
            set_activity(4242, "nonce-1", Some(activity(state))),
            set_activity(4242, "nonce-2", None),
        ]
    }

    #[test]
    fn activity_shows_the_phase_and_its_start() {
        let linked = PhaseState {
            phase: LinkPhase::Linked,
            since: 1_760_000_000_999,
        };
        assert_eq!(
            activity(linked),
            json!({
                "details": "En liaison",
                "timestamps": { "start": 1_760_000_000 },
                "assets": { "large_image": "papillon", "large_text": "Atlas · VPS Control" },
            })
        );
        let ground = PhaseState {
            phase: LinkPhase::Ground,
            since: 0,
        };
        assert_eq!(activity(ground)["details"], "Au sol");
    }

    #[test]
    fn clearing_sends_a_null_activity() {
        let frame = set_activity(1, "n", None);
        assert_eq!(frame["cmd"], "SET_ACTIVITY");
        assert_eq!(frame["args"]["activity"], Value::Null);
        assert_eq!(frame["args"]["pid"], 1);
    }

    /// Seuls le nom, l'état, la durée et le papillon peuvent être envoyés :
    /// toutes les chaînes et toutes les clés des trames appartiennent à une
    /// liste fixe, quelle que soit la phase.
    #[test]
    fn only_fixed_strings_and_keys_can_be_sent() {
        let allowed_strings = [
            APP_NAME,
            LINKED_LABEL,
            GROUND_LABEL,
            IMAGE_KEY,
            "SET_ACTIVITY",
            "123456789012345678",
            "nonce-1",
            "nonce-2",
        ];
        let allowed_keys = [
            "v",
            "client_id",
            "cmd",
            "args",
            "pid",
            "activity",
            "nonce",
            "details",
            "timestamps",
            "start",
            "assets",
            "large_image",
            "large_text",
        ];
        for phase in [LinkPhase::Linked, LinkPhase::Ground] {
            let (mut strings, mut keys) = (Vec::new(), Vec::new());
            for frame in sent_frames(PhaseState {
                phase,
                since: 5_000,
            }) {
                strings_and_keys(&frame, &mut strings, &mut keys);
            }
            for s in &strings {
                assert!(
                    allowed_strings.contains(&s.as_str()),
                    "chaîne inattendue : {s}"
                );
            }
            for k in &keys {
                assert!(allowed_keys.contains(&k.as_str()), "clé inattendue : {k}");
            }
        }
    }

    /// Aucune donnée d'un profil (nom, hôte, adresse, utilisateur, clé) ni
    /// un nom de fichier ou une commande n'apparaît dans ce qui est envoyé.
    #[test]
    fn no_profile_file_or_command_data_reaches_discord() {
        let profile = Profile {
            id: "f2c1a9e0-profil".into(),
            name: "vps-prod-01".into(),
            host: "51.178.42.17".into(),
            port: 2222,
            user: "deploy".into(),
            key_path: "~/.ssh/id_ed25519".into(),
            host_key_fingerprint: Some("SHA256:k3Qm9vZc1uR7xH2bT0yLw4NfA8sEj6dPoXgVtYiCz5U".into()),
            is_default: true,
        };
        let sensitive = [
            profile.id.as_str(),
            profile.name.as_str(),
            profile.host.as_str(),
            profile.user.as_str(),
            profile.key_path.as_str(),
            profile.host_key_fingerprint.as_deref().unwrap(),
            "2222",
            "Caddyfile",
            "docker-compose.yml",
            "systemctl",
            "docker",
        ];
        for phase in [LinkPhase::Linked, LinkPhase::Ground] {
            for frame in sent_frames(PhaseState {
                phase,
                since: 5_000,
            }) {
                let text = frame.to_string();
                for value in sensitive {
                    assert!(!text.contains(value), "{value} envoyé : {text}");
                }
            }
        }
    }
}
