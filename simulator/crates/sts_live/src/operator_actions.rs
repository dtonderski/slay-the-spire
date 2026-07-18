use crate::model::{ActionId, Character, LegalAction, LegalActionKind, RunConfig};
use serde_json::json;

pub(super) fn start_run_action(config: &RunConfig) -> LegalAction {
    let character = match config.character {
        Character::Ironclad => "IRONCLAD",
    };
    let seed = config.seed.command_text();
    let command = format!("START {character} {} {seed}", config.ascension);
    LegalAction {
        id: ActionId("start-run".to_owned()),
        kind: LegalActionKind::StartRun,
        label: "Start run".to_owned(),
        enabled: true,
        command: json!({
            "transport": "communication_mod",
            "command": command,
        }),
        disabled_reason: None,
    }
}

pub(super) fn request_state_action() -> LegalAction {
    LegalAction {
        id: ActionId("request-state".to_owned()),
        kind: LegalActionKind::RequestState,
        label: "Request state".to_owned(),
        enabled: true,
        command: json!({
            "transport": "communication_mod",
            "command": "state",
        }),
        disabled_reason: None,
    }
}
