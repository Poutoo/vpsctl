//! État d'un conteneur ou d'un service tel qu'affiché dans l'interface, et
//! actions qui le font changer.
//! La correspondance depuis les états Docker et systemd est faite dans
//! `docker` et `systemd` (voir `docs/03-architecture.md`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RunState {
    Nominal,
    Stopped,
    Failed,
    Transition,
}

/// Action sur un conteneur ou un service (`docker_action`, `service_action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    /// Verbe de `docker` et de `systemctl`.
    pub fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, RunState};
    use serde_json::{json, to_value};

    #[test]
    fn serializes_in_camel_case() {
        assert_eq!(to_value(RunState::Nominal).unwrap(), json!("nominal"));
        assert_eq!(to_value(RunState::Stopped).unwrap(), json!("stopped"));
        assert_eq!(to_value(RunState::Failed).unwrap(), json!("failed"));
        assert_eq!(to_value(RunState::Transition).unwrap(), json!("transition"));
    }

    #[test]
    fn actions_deserialize_from_the_contract_names() {
        for (name, action) in [
            ("start", Action::Start),
            ("stop", Action::Stop),
            ("restart", Action::Restart),
        ] {
            assert_eq!(
                serde_json::from_value::<Action>(json!(name)).unwrap(),
                action
            );
            assert_eq!(action.verb(), name);
        }
        assert!(serde_json::from_value::<Action>(json!("kill")).is_err());
    }
}
