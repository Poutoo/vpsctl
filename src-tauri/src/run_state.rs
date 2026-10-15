//! État d'un conteneur ou d'un service tel qu'affiché dans l'interface.
//! La correspondance depuis les états Docker et systemd est faite dans
//! `docker` et `systemd` (voir `docs/03-architecture.md`).

use serde::Serialize;
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

#[cfg(test)]
mod tests {
    use super::RunState;
    use serde_json::{json, to_value};

    #[test]
    fn serializes_in_camel_case() {
        assert_eq!(to_value(RunState::Nominal).unwrap(), json!("nominal"));
        assert_eq!(to_value(RunState::Stopped).unwrap(), json!("stopped"));
        assert_eq!(to_value(RunState::Failed).unwrap(), json!("failed"));
        assert_eq!(to_value(RunState::Transition).unwrap(), json!("transition"));
    }
}
