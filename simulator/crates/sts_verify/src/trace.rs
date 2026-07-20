//! Trace JSONL formats for verification corpora.

use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sts_core::content::encounters::BossUnlockState;

/// One line from a CommunicationMod-style trace file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceLine {
    Metadata(TraceMetadata),
    State(TraceState),
    Action(TraceAction),
    Error(TraceError),
    CommandAccept(TraceCommandAccept),
    Response(TraceResponse),
    SlayTheData(TraceSlayTheData),
    Automation(TraceAutomation),
    CommandObservedTimeout(TraceCommandObservedTimeout),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceCommandAccept {
    pub step: u32,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceResponse {
    pub sequence: u64,
    pub response: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceSlayTheData {
    pub sequence: u64,
    pub event: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceAutomation {
    pub sequence: u64,
    pub event: String,
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceCommandObservedTimeout {
    pub step: u32,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceError {
    pub step: u32,
    pub message: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceMetadata {
    #[serde(default)]
    pub schema: u32,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Profile state supplied as a pre-run input. Boss selection is not a pure
    /// function of the seed while a profile still has unseen bosses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_unlocks: Option<BossUnlockState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceState {
    pub step: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    pub message: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceAction {
    pub step: u32,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<String>,
    /// Explicit non-seeded run timer input used by time-gated target logic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playtime_seconds: Option<u32>,
}

/// Hand-authored manual corpus fixture (one JSON object per file or line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualFixture {
    pub name: String,
    pub fixture: String,
    pub actions: Vec<Value>,
    pub rng_draws: u32,
}

/// Parsed CommunicationMod trace with metadata, states, and actions in order.
#[derive(Debug, Clone, PartialEq)]
pub struct CommunicationModTrace {
    pub metadata: Option<TraceMetadata>,
    pub lines: Vec<TraceLine>,
}

/// Parse every nonblank JSONL record into one known typed trace line.
pub fn parse_trace_jsonl(content: &str) -> Result<Vec<TraceLine>, serde_json::Error> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        let parsed = match value.get("type").and_then(Value::as_str) {
            Some("metadata") => TraceLine::Metadata(serde_json::from_value(value)?),
            Some("state") => TraceLine::State(parse_state_line(value)?),
            Some("action") => TraceLine::Action(parse_action_line(value)?),
            Some("error") => TraceLine::Error(serde_json::from_value(value)?),
            Some("command_accept") => TraceLine::CommandAccept(parse_command_accept_line(value)?),
            Some("response") => TraceLine::Response(parse_response_line(value)?),
            Some("slay_the_data") => TraceLine::SlayTheData(parse_slay_the_data_line(value)?),
            Some("automation") => TraceLine::Automation(parse_automation_line(value)?),
            Some("command_observed_timeout") => {
                TraceLine::CommandObservedTimeout(parse_command_observed_timeout_line(value)?)
            }
            _ => serde_json::from_value::<TraceLine>(value)?,
        };
        lines.push(parsed);
    }
    Ok(lines)
}

fn parse_command_accept_line(value: Value) -> Result<TraceCommandAccept, serde_json::Error> {
    let accepted: TraceCommandAccept = serde_json::from_value(value)?;
    if accepted.command.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "trace command acceptance must name a command",
        ));
    }
    Ok(accepted)
}

fn parse_response_line(value: Value) -> Result<TraceResponse, serde_json::Error> {
    let response: TraceResponse = serde_json::from_value(value)?;
    if !response.response.is_object() {
        return Err(serde_json::Error::custom(
            "trace response payload must be a JSON object",
        ));
    }
    Ok(response)
}

fn parse_slay_the_data_line(value: Value) -> Result<TraceSlayTheData, serde_json::Error> {
    let guidance: TraceSlayTheData = serde_json::from_value(value)?;
    if guidance.event.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "SlayTheData trace guidance must name an event",
        ));
    }
    if !guidance.details.is_object() {
        return Err(serde_json::Error::custom(
            "SlayTheData trace guidance details must be a JSON object",
        ));
    }
    Ok(guidance)
}

fn parse_automation_line(value: Value) -> Result<TraceAutomation, serde_json::Error> {
    let automation: TraceAutomation = serde_json::from_value(value)?;
    if automation.event.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "automation trace telemetry must name an event",
        ));
    }
    if !automation.details.is_object() {
        return Err(serde_json::Error::custom(
            "automation trace telemetry details must be a JSON object",
        ));
    }
    Ok(automation)
}

fn parse_command_observed_timeout_line(
    value: Value,
) -> Result<TraceCommandObservedTimeout, serde_json::Error> {
    let timeout: TraceCommandObservedTimeout = serde_json::from_value(value)?;
    if timeout.command.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "trace command observation timeout must name a command",
        ));
    }
    Ok(timeout)
}

fn parse_state_line(value: Value) -> Result<TraceState, serde_json::Error> {
    let state: TraceState = if value.get("step").is_some() && value.get("message").is_some() {
        serde_json::from_value(value)?
    } else {
        let step = value
            .pointer("/state/raw/current_state/step")
            .or_else(|| value.pointer("/state/raw/status/step"))
            .or_else(|| value.get("sequence"))
            .cloned()
            .unwrap_or(Value::Null);
        let message = value
            .pointer("/state/raw/current_state/message")
            .cloned()
            .unwrap_or(Value::Null);
        let received_at = value
            .pointer("/state/raw/current_state/received_at")
            .cloned()
            .unwrap_or(Value::Null);

        serde_json::from_value(serde_json::json!({
            "step": step,
            "received_at": received_at,
            "message": message,
        }))?
    };
    if !state.message.is_object() {
        return Err(serde_json::Error::custom(
            "trace state message must be a JSON object",
        ));
    }
    validate_game_state_schema(state.step, &state.message)?;
    Ok(state)
}

fn validate_game_state_schema(step: u32, message: &Value) -> Result<(), serde_json::Error> {
    let Some(game_value) = message.get("game_state") else {
        return Ok(());
    };
    let game = game_value.as_object().ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} game_state must be a JSON object"
        ))
    })?;
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .filter(|screen_type| !screen_type.trim().is_empty())
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} game_state.screen_type must be a string"
            ))
        })?;
    if screen_type.eq_ignore_ascii_case("MENU") {
        return Ok(());
    }

    let ascension = required_unsigned_game_field(step, game, "ascension_level")?;
    if u8::try_from(ascension).is_err() {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} game_state.ascension_level is out of range"
        )));
    }
    let floor = required_unsigned_game_field(step, game, "floor")?;
    if u32::try_from(floor).is_err() {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} game_state.floor is out of range"
        )));
    }
    for field in ["gold", "current_hp", "max_hp"] {
        let Some(value) = game.get(field).and_then(Value::as_i64) else {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} game_state.{field} must be an integer"
            )));
        };
        if i32::try_from(value).is_err() {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} game_state.{field} is out of range"
            )));
        }
    }
    for field in ["deck", "relics", "potions"] {
        let entries = game.get(field).and_then(Value::as_array).ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} game_state.{field} must be an array"
            ))
        })?;
        if entries.iter().any(|entry| {
            !entry.as_object().is_some_and(|entry| {
                entry
                    .get("id")
                    .and_then(Value::as_str)
                    .or_else(|| entry.get("name").and_then(Value::as_str))
                    .is_some_and(|identity| !identity.trim().is_empty())
            })
        }) {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} game_state.{field} entries must name an id or name"
            )));
        }
    }
    if game.get("choice_list").is_some_and(|choices| {
        choices
            .as_array()
            .is_none_or(|choices| choices.iter().any(|choice| !choice.is_string()))
    }) {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} game_state.choice_list must be an array of strings"
        )));
    }
    validate_optional_combat_state(step, game)?;
    validate_visible_screen_schema(step, screen_type, game)?;
    Ok(())
}

fn validate_optional_combat_state(
    step: u32,
    game: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let Some(value) = game.get("combat_state").filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let combat = value.as_object().ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} game_state.combat_state must be a JSON object"
        ))
    })?;
    let player = combat
        .get("player")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} game_state.combat_state.player must be a JSON object"
            ))
        })?;
    for field in ["current_hp", "block", "energy"] {
        validate_i32_field(step, "game_state.combat_state.player", player, field)?;
    }
    for pile in ["hand", "draw_pile", "discard_pile"] {
        validate_combat_card_array(step, combat, pile)?;
    }
    validate_combat_monsters(step, combat)
}

fn validate_combat_card_array(
    step: u32,
    combat: &serde_json::Map<String, Value>,
    pile: &str,
) -> Result<(), serde_json::Error> {
    let path = format!("game_state.combat_state.{pile}");
    let cards = combat.get(pile).and_then(Value::as_array).ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} {path} must be an array"
        ))
    })?;
    for card in cards {
        let card = card.as_object().ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {path} entries must be objects"
            ))
        })?;
        if card
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(|id| id.trim().is_empty())
        {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} {path} entries require a string id"
            )));
        }
        if let Some(upgrades) = card.get("upgrades") {
            let valid = upgrades
                .as_u64()
                .and_then(|upgrades| u8::try_from(upgrades).ok())
                .is_some();
            if !valid {
                return Err(serde_json::Error::custom(format!(
                    "trace state at step {step} {path} entry upgrades must be a non-negative u8"
                )));
            }
        }
    }
    Ok(())
}

fn validate_combat_monsters(
    step: u32,
    combat: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    const PATH: &str = "game_state.combat_state.monsters";
    let monsters = combat
        .get("monsters")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} must be an array"
            ))
        })?;
    for monster in monsters {
        let monster = monster.as_object().ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} entries must be objects"
            ))
        })?;
        if monster
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| monster.get("name").and_then(Value::as_str))
            .is_none_or(|identity| identity.trim().is_empty())
        {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} entries require an id or name"
            )));
        }
        for field in ["current_hp", "max_hp", "block"] {
            validate_i32_field(step, PATH, monster, field)?;
        }
        if monster
            .get("intent")
            .and_then(Value::as_str)
            .is_none_or(|intent| intent.trim().is_empty())
        {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} entries require a string intent"
            )));
        }
        validate_combat_powers(step, monster)?;
    }
    Ok(())
}

fn validate_combat_powers(
    step: u32,
    monster: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    const PATH: &str = "game_state.combat_state.monsters[].powers";
    let powers = monster
        .get("powers")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} must be an array"
            ))
        })?;
    for power in powers {
        let power = power.as_object().ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} entries must be objects"
            ))
        })?;
        if power
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| power.get("name").and_then(Value::as_str))
            .is_none_or(|identity| identity.trim().is_empty())
        {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} {PATH} entries require an id or name"
            )));
        }
        validate_i32_field(step, PATH, power, "amount")?;
    }
    Ok(())
}

fn validate_i32_field(
    step: u32,
    path: &str,
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), serde_json::Error> {
    let value = object.get(field).and_then(Value::as_i64).ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} {path}.{field} must be an integer"
        ))
    })?;
    if i32::try_from(value).is_err() {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} {path}.{field} is out of range"
        )));
    }
    Ok(())
}

fn validate_visible_screen_schema(
    step: u32,
    screen_type: &str,
    game: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let required_collection = match screen_type {
        "CARD_REWARD" => Some("cards"),
        "COMBAT_REWARD" => Some("rewards"),
        "EVENT" => return validate_event_screen_schema(step, game),
        "MAP" => return validate_map_screen_schema(step, game),
        "GRID" => return validate_grid_screen_schema(step, game),
        _ => return validate_optional_screen_state(step, game),
    };
    let screen = game
        .get("screen_state")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {screen_type} screen requires an object game_state.screen_state"
            ))
        })?;
    if let Some(field) = required_collection {
        if screen.get(field).and_then(Value::as_array).is_none() {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} {screen_type} screen requires an array game_state.screen_state.{field}"
            )));
        }
    }
    validate_screen_state_collections(step, screen)
}

fn validate_grid_screen_schema(
    step: u32,
    game: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let screen = game
        .get("screen_state")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} GRID screen requires an object game_state.screen_state"
            ))
        })?;
    validate_identity_array(
        step,
        "game_state.screen_state.cards",
        screen.get("cards").unwrap_or(&Value::Null),
    )?;
    validate_identity_array(
        step,
        "game_state.screen_state.selected_cards",
        screen.get("selected_cards").unwrap_or(&Value::Null),
    )?;
    for field in [
        "confirm_up",
        "for_purge",
        "for_transform",
        "for_upgrade",
        "any_number",
    ] {
        if screen.get(field).and_then(Value::as_bool).is_none() {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} game_state.screen_state.{field} must be a boolean"
            )));
        }
    }
    let purpose_count = ["for_purge", "for_transform", "for_upgrade"]
        .iter()
        .filter(|field| screen.get(**field).and_then(Value::as_bool) == Some(true))
        .count();
    if purpose_count > 1 {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} GRID screen declares multiple selection purposes"
        )));
    }
    if screen
        .get("num_cards")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && u32::try_from(*count).is_ok())
        .is_none()
    {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} game_state.screen_state.num_cards must be a positive u32"
        )));
    }
    if screen.get("confirm_up").and_then(Value::as_bool) == Some(false)
        || game.get("choice_list").is_some()
    {
        validate_nonblank_string_array(step, game, "choice_list", "game_state.choice_list")?;
    }
    validate_screen_state_collections(step, screen)
}

fn validate_map_screen_schema(
    step: u32,
    game: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let screen = game
        .get("screen_state")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} MAP screen requires an object game_state.screen_state"
            ))
        })?;
    let first_node_chosen = screen
        .get("first_node_chosen")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} MAP screen requires boolean game_state.screen_state.first_node_chosen"
            ))
        })?;
    let current_node = screen
        .get("current_node")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} MAP screen requires object game_state.screen_state.current_node"
            ))
        })?;
    for field in ["x", "y"] {
        if current_node.get(field).and_then(Value::as_i64).is_none() {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} MAP current_node.{field} must be an integer"
            )));
        }
    }
    let symbol = current_node.get("symbol");
    if first_node_chosen {
        if symbol
            .and_then(Value::as_str)
            .is_none_or(|symbol| symbol.trim().is_empty())
        {
            return Err(serde_json::Error::custom(format!(
                "trace state at step {step} MAP chosen current_node requires a string symbol"
            )));
        }
    } else if symbol.is_some_and(|symbol| {
        symbol
            .as_str()
            .is_none_or(|symbol| !symbol.trim().is_empty())
    }) {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} MAP unchosen current_node must omit symbol"
        )));
    }
    validate_nonblank_string_array(step, game, "choice_list", "game_state.choice_list")?;
    if screen.get("next_nodes").and_then(Value::as_array).is_none() {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} MAP screen requires an array game_state.screen_state.next_nodes"
        )));
    }
    validate_screen_state_collections(step, screen)
}

fn validate_nonblank_string_array(
    step: u32,
    object: &serde_json::Map<String, Value>,
    field: &str,
    path: &str,
) -> Result<(), serde_json::Error> {
    let entries = object.get(field).and_then(Value::as_array).ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} {path} must be an array"
        ))
    })?;
    if entries
        .iter()
        .any(|entry| entry.as_str().is_none_or(|entry| entry.trim().is_empty()))
    {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} {path} entries must be nonblank strings"
        )));
    }
    Ok(())
}

fn validate_event_screen_schema(
    step: u32,
    game: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let screen = game
        .get("screen_state")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} EVENT screen requires an object game_state.screen_state"
            ))
        })?;
    if screen
        .get("event_id")
        .and_then(Value::as_str)
        .or_else(|| screen.get("event_name").and_then(Value::as_str))
        .is_none_or(|identity| identity.trim().is_empty())
    {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} EVENT screen requires an event_id or event_name"
        )));
    }
    let choices = game
        .get("choice_list")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} EVENT screen requires an array game_state.choice_list"
            ))
        })?;
    if choices.iter().any(|choice| {
        choice
            .as_str()
            .is_none_or(|choice| choice.trim().is_empty())
    }) {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} EVENT game_state.choice_list entries must be nonblank strings"
        )));
    }
    if screen.get("options").and_then(Value::as_array).is_none() {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} EVENT screen requires an array game_state.screen_state.options"
        )));
    }
    validate_screen_state_collections(step, screen)
}

fn validate_optional_screen_state(
    step: u32,
    game: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let Some(value) = game.get("screen_state") else {
        return Ok(());
    };
    let screen = value.as_object().ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} game_state.screen_state must be a JSON object"
        ))
    })?;
    validate_screen_state_collections(step, screen)
}

fn validate_screen_state_collections(
    step: u32,
    screen: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    if let Some(cards) = screen.get("cards") {
        validate_identity_array(step, "game_state.screen_state.cards", cards)?;
    }
    if let Some(rewards) = screen.get("rewards") {
        validate_object_array_field(
            step,
            "game_state.screen_state.rewards",
            rewards,
            "reward_type",
        )?;
        validate_reward_payloads(step, rewards)?;
    }
    if let Some(options) = screen.get("options") {
        validate_object_array_field(step, "game_state.screen_state.options", options, "text")?;
    }
    if let Some(nodes) = screen.get("next_nodes") {
        let nodes = nodes.as_array().ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} game_state.screen_state.next_nodes must be an array"
            ))
        })?;
        for node in nodes {
            let node = node.as_object().ok_or_else(|| {
                serde_json::Error::custom(format!(
                    "trace state at step {step} game_state.screen_state.next_nodes entries must be objects"
                ))
            })?;
            if node
                .get("symbol")
                .and_then(Value::as_str)
                .filter(|symbol| !symbol.trim().is_empty())
                .is_none()
                || node.get("x").and_then(Value::as_i64).is_none()
                || node.get("y").and_then(Value::as_i64).is_none()
            {
                return Err(serde_json::Error::custom(format!(
                    "trace state at step {step} game_state.screen_state.next_nodes entries require symbol, x, and y"
                )));
            }
        }
    }
    Ok(())
}

fn validate_reward_payloads(step: u32, value: &Value) -> Result<(), serde_json::Error> {
    let rewards = value
        .as_array()
        .expect("reward collection was validated as an array");
    for reward in rewards {
        let reward = reward
            .as_object()
            .expect("reward entry was validated as an object");
        match reward
            .get("reward_type")
            .and_then(Value::as_str)
            .expect("reward type was validated as a string")
        {
            "GOLD" | "STOLEN_GOLD" => {
                validate_i32_field(step, "game_state.screen_state.rewards[]", reward, "gold")?;
            }
            "POTION" => validate_reward_identity_payload(step, reward, "potion")?,
            "RELIC" => validate_reward_identity_payload(step, reward, "relic")?,
            _ => {}
        }
    }
    Ok(())
}

fn validate_reward_identity_payload(
    step: u32,
    reward: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), serde_json::Error> {
    let payload = reward
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| {
            serde_json::Error::custom(format!(
                "trace state at step {step} {field} reward requires an object {field} payload"
            ))
        })?;
    if payload
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("name").and_then(Value::as_str))
        .is_none_or(|identity| identity.trim().is_empty())
    {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} {field} reward payload requires an id or name"
        )));
    }
    Ok(())
}

fn validate_identity_array(step: u32, path: &str, value: &Value) -> Result<(), serde_json::Error> {
    let entries = value.as_array().ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} {path} must be an array"
        ))
    })?;
    if entries.iter().any(|entry| {
        !entry.as_object().is_some_and(|entry| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| entry.get("name").and_then(Value::as_str))
                .is_some_and(|identity| !identity.trim().is_empty())
        })
    }) {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} {path} entries must name an id or name"
        )));
    }
    Ok(())
}

fn validate_object_array_field(
    step: u32,
    path: &str,
    value: &Value,
    required_field: &str,
) -> Result<(), serde_json::Error> {
    let entries = value.as_array().ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} {path} must be an array"
        ))
    })?;
    if entries.iter().any(|entry| {
        entry
            .get(required_field)
            .and_then(Value::as_str)
            .is_none_or(|field| field.trim().is_empty())
    }) {
        return Err(serde_json::Error::custom(format!(
            "trace state at step {step} {path} entries require string field {required_field}"
        )));
    }
    Ok(())
}

fn required_unsigned_game_field(
    step: u32,
    game: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u64, serde_json::Error> {
    game.get(field).and_then(Value::as_u64).ok_or_else(|| {
        serde_json::Error::custom(format!(
            "trace state at step {step} game_state.{field} must be a non-negative integer"
        ))
    })
}

fn parse_action_line(value: Value) -> Result<TraceAction, serde_json::Error> {
    let action: TraceAction = if value.get("step").is_some() && value.get("command").is_some() {
        serde_json::from_value(value)?
    } else {
        let step = value
            .pointer("/action/command/source_state_seq")
            .or_else(|| value.get("sequence"))
            .cloned()
            .unwrap_or(Value::Null);
        let command = value
            .pointer("/action/command/command")
            .cloned()
            .unwrap_or(Value::Null);
        let playtime_seconds = value
            .pointer("/action/playtime_seconds")
            .or_else(|| value.pointer("/action/command/playtime_seconds"))
            .cloned()
            .unwrap_or(Value::Null);

        serde_json::from_value(serde_json::json!({
            "step": step,
            "command": command,
            "playtime_seconds": playtime_seconds,
        }))?
    };
    if action.command.trim().is_empty() {
        return Err(serde_json::Error::custom(
            "trace action command must not be empty",
        ));
    }
    Ok(action)
}

/// Import a CommunicationMod trace, collecting metadata and ordered lines.
pub fn import_communication_mod_trace(
    content: &str,
) -> Result<CommunicationModTrace, serde_json::Error> {
    let lines = parse_trace_jsonl(content)?;
    let metadata = lines.iter().find_map(|line| {
        if let TraceLine::Metadata(metadata) = line {
            Some(metadata.clone())
        } else {
            None
        }
    });
    Ok(CommunicationModTrace { metadata, lines })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trace_rejects_unknown_line_types() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"state","step":0,"message":{}}
{"type":"action","step":1,"command":"PLAY 1 0"}
{"type":"exit","ended_at":"now"}"#;

        let error = parse_trace_jsonl(content).expect_err("unknown record type is invalid");
        assert!(error.to_string().contains("unknown variant `exit`"));
    }

    #[test]
    fn parse_trace_preserves_known_auxiliary_records() {
        let content = r#"{"type":"command_accept","step":1,"command":"CHOOSE 0"}
{"type":"response","sequence":2,"response":{"kind":"bridge_command_result"}}
{"type":"slay_the_data","sequence":3,"event":"send_action","details":{"step_index":1}}
{"type":"automation","sequence":4,"event":"plan_ready","details":{"state":"ready_to_send"}}
{"type":"command_observed_timeout","step":5,"command":"END"}"#;

        let lines = parse_trace_jsonl(content).expect("known auxiliary records parse");
        assert!(matches!(
            &lines[0],
            TraceLine::CommandAccept(accepted)
                if accepted.step == 1 && accepted.command == "CHOOSE 0"
        ));
        assert!(matches!(
            &lines[1],
            TraceLine::Response(response)
                if response.sequence == 2
                    && response.response["kind"] == "bridge_command_result"
        ));
        assert!(matches!(
            &lines[2],
            TraceLine::SlayTheData(guidance)
                if guidance.sequence == 3
                    && guidance.event == "send_action"
                    && guidance.details["step_index"] == 1
        ));
        assert!(matches!(
            &lines[3],
            TraceLine::Automation(automation)
                if automation.sequence == 4
                    && automation.event == "plan_ready"
                    && automation.details["state"] == "ready_to_send"
        ));
        assert!(matches!(
            &lines[4],
            TraceLine::CommandObservedTimeout(timeout)
                if timeout.step == 5 && timeout.command == "END"
        ));
    }

    #[test]
    fn parse_trace_rejects_missing_line_type() {
        let error = parse_trace_jsonl(r#"{"step":1,"command":"PLAY 1 0"}"#)
            .expect_err("missing record type is invalid");

        assert!(error.to_string().contains("missing field `type`"));
    }

    #[test]
    fn parse_trace_rejects_null_state_message() {
        let error = parse_trace_jsonl(r#"{"type":"state","step":1,"message":null}"#)
            .expect_err("null state message is invalid");

        assert!(error
            .to_string()
            .contains("trace state message must be a JSON object"));
    }

    #[test]
    fn parse_trace_rejects_incomplete_non_menu_game_state() {
        let content = r#"{"type":"state","step":7,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"relics":[]}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing deck is invalid input");
        assert!(error
            .to_string()
            .contains("trace state at step 7 game_state.deck must be an array"));
    }

    #[test]
    fn parse_trace_rejects_unnamed_authoritative_entries() {
        let content = r#"{"type":"state","step":8,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[{}],"relics":[]}}}"#;

        let error = parse_trace_jsonl(content).expect_err("unnamed deck entry is invalid input");
        assert!(error
            .to_string()
            .contains("trace state at step 8 game_state.deck entries must name an id or name"));
    }

    #[test]
    fn parse_trace_rejects_missing_potion_authority() {
        let content = r#"{"type":"state","step":9,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[]}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing potions are invalid input");
        assert!(error
            .to_string()
            .contains("trace state at step 9 game_state.potions must be an array"));
    }

    #[test]
    fn parse_trace_rejects_missing_card_reward_choices() {
        let content = r#"{"type":"state","step":10,"message":{"game_state":{"screen_type":"CARD_REWARD","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"screen_state":{}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing visible cards are invalid");
        assert!(error.to_string().contains(
            "trace state at step 10 CARD_REWARD screen requires an array game_state.screen_state.cards"
        ));
    }

    #[test]
    fn parse_trace_rejects_malformed_visible_map_nodes() {
        let content = r#"{"type":"state","step":11,"message":{"game_state":{"screen_type":"MAP","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["x=0"],"screen_state":{"first_node_chosen":false,"current_node":{"x":0,"y":-1},"next_nodes":[{"symbol":"M","x":0}]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("incomplete visible node is invalid");
        assert!(error.to_string().contains(
            "trace state at step 11 game_state.screen_state.next_nodes entries require symbol, x, and y"
        ));
    }

    #[test]
    fn parse_trace_rejects_missing_combat_player() {
        let content = r#"{"type":"state","step":12,"message":{"game_state":{"screen_type":"NONE","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"combat_state":{"hand":[],"draw_pile":[],"discard_pile":[],"monsters":[]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing combat player is invalid");
        assert!(error.to_string().contains(
            "trace state at step 12 game_state.combat_state.player must be a JSON object"
        ));
    }

    #[test]
    fn parse_trace_rejects_combat_card_without_id() {
        let content = r#"{"type":"state","step":13,"message":{"game_state":{"screen_type":"NONE","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"combat_state":{"hand":[{"name":"Strike"}],"draw_pile":[],"discard_pile":[],"player":{"current_hp":80,"block":0,"energy":3},"monsters":[]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("unprojectable combat card is invalid");
        assert!(error.to_string().contains(
            "trace state at step 13 game_state.combat_state.hand entries require a string id"
        ));
    }

    #[test]
    fn parse_trace_rejects_monster_without_observable_hp() {
        let content = r#"{"type":"state","step":14,"message":{"game_state":{"screen_type":"NONE","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"combat_state":{"hand":[],"draw_pile":[],"discard_pile":[],"player":{"current_hp":80,"block":0,"energy":3},"monsters":[{"name":"Cultist","max_hp":50,"block":0,"intent":"BUFF","powers":[]}]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing monster hp is invalid");
        assert!(error.to_string().contains(
            "trace state at step 14 game_state.combat_state.monsters.current_hp must be an integer"
        ));
    }

    #[test]
    fn parse_trace_rejects_event_without_identity() {
        let content = r#"{"type":"state","step":15,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":2,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["Leave"],"screen_state":{"options":[{"text":"Leave"}]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing event identity is invalid");
        assert!(error
            .to_string()
            .contains("trace state at step 15 EVENT screen requires an event_id or event_name"));
    }

    #[test]
    fn parse_trace_rejects_event_without_choice_authority() {
        let content = r#"{"type":"state","step":16,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":2,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"screen_state":{"event_id":"Golden Shrine","options":[]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing event choices are invalid");
        assert!(error.to_string().contains(
            "trace state at step 16 EVENT screen requires an array game_state.choice_list"
        ));
    }

    #[test]
    fn parse_trace_rejects_map_without_current_node_authority() {
        let content = r#"{"type":"state","step":17,"message":{"game_state":{"screen_type":"MAP","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["x=0"],"screen_state":{"first_node_chosen":true,"next_nodes":[]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing current map node is invalid");
        assert!(error.to_string().contains(
            "trace state at step 17 MAP screen requires object game_state.screen_state.current_node"
        ));
    }

    #[test]
    fn parse_trace_rejects_chosen_map_node_without_symbol() {
        let content = r#"{"type":"state","step":18,"message":{"game_state":{"screen_type":"MAP","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["x=1"],"screen_state":{"first_node_chosen":true,"current_node":{"x":0,"y":0},"next_nodes":[]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("chosen node identity is required");
        assert!(error
            .to_string()
            .contains("trace state at step 18 MAP chosen current_node requires a string symbol"));
    }

    #[test]
    fn parse_trace_accepts_unselected_map_sentinel() {
        let content = r#"{"type":"state","step":19,"message":{"game_state":{"screen_type":"MAP","ascension_level":0,"floor":0,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["x=0"],"screen_state":{"first_node_chosen":false,"current_node":{"x":0,"y":-1},"next_nodes":[{"symbol":"M","x":0,"y":0}]}}}}"#;

        parse_trace_jsonl(content).expect("pre-first-node sentinel is authoritative");
    }

    #[test]
    fn parse_trace_rejects_grid_without_selection_mode() {
        let content = r#"{"type":"state","step":20,"message":{"game_state":{"screen_type":"GRID","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["Strike"],"screen_state":{"cards":[{"id":"Strike_R"}],"selected_cards":[],"for_purge":true,"for_transform":false,"for_upgrade":false,"any_number":false,"num_cards":1}}}}"#;

        let error =
            parse_trace_jsonl(content).expect_err("missing grid confirmation mode is invalid");
        assert!(error.to_string().contains(
            "trace state at step 20 game_state.screen_state.confirm_up must be a boolean"
        ));
    }

    #[test]
    fn parse_trace_accepts_grid_confirmation_without_choice_list() {
        let content = r#"{"type":"state","step":21,"message":{"game_state":{"screen_type":"GRID","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"screen_state":{"cards":[{"id":"Strike_R"}],"selected_cards":[],"confirm_up":true,"for_purge":true,"for_transform":false,"for_upgrade":false,"any_number":false,"num_cards":1}}}}"#;

        parse_trace_jsonl(content).expect("confirmation overlay omits ordinary choices");
    }

    #[test]
    fn parse_trace_rejects_gold_reward_without_amount() {
        let content = r#"{"type":"state","step":22,"message":{"game_state":{"screen_type":"COMBAT_REWARD","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"screen_state":{"rewards":[{"reward_type":"GOLD"}]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing gold amount is invalid");
        assert!(error.to_string().contains(
            "trace state at step 22 game_state.screen_state.rewards[].gold must be an integer"
        ));
    }

    #[test]
    fn parse_trace_rejects_relic_reward_without_identity() {
        let content = r#"{"type":"state","step":23,"message":{"game_state":{"screen_type":"COMBAT_REWARD","ascension_level":0,"floor":1,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"screen_state":{"rewards":[{"reward_type":"RELIC","relic":{}}]}}}}"#;

        let error = parse_trace_jsonl(content).expect_err("missing relic identity is invalid");
        assert!(error
            .to_string()
            .contains("trace state at step 23 relic reward payload requires an id or name"));
    }

    #[test]
    fn parse_trace_allows_partial_menu_game_state() {
        let lines = parse_trace_jsonl(
            r#"{"type":"state","step":0,"message":{"game_state":{"screen_type":"MENU"}}}"#,
        )
        .expect("menu state does not represent an active run");

        assert!(matches!(&lines[0], TraceLine::State(state) if state.step == 0));
    }

    #[test]
    fn parse_trace_rejects_empty_action_command() {
        let error = parse_trace_jsonl(r#"{"type":"action","step":1,"command":"  "}"#)
            .expect_err("empty action command is invalid");

        assert!(error
            .to_string()
            .contains("trace action command must not be empty"));
    }

    #[test]
    fn parse_trace_accepts_live_trace_session_records() {
        let content = r#"{"type":"state","sequence":7,"state":{"raw":{"current_state":{"step":6,"received_at":"now","message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":0,"gold":99,"current_hp":80,"max_hp":80,"deck":[],"relics":[],"potions":[],"choice_list":["Leave"],"screen_state":{"event_id":"Golden Shrine","options":[{"text":"Leave"}]}}}}}}}
{"type":"action","sequence":7,"action":{"command":{"command":"CHOOSE 0","source_state_seq":6},"playtime_seconds":812}}"#;

        let lines = parse_trace_jsonl(content).expect("parses");
        assert_eq!(lines.len(), 2);
        assert!(matches!(
            &lines[0],
            TraceLine::State(state)
                if state.step == 6
                    && state.received_at.as_deref() == Some("now")
                    && state.message["game_state"]["floor"] == 0
        ));
        assert!(matches!(
            &lines[1],
            TraceLine::Action(action)
                if action.step == 6
                    && action.command == "CHOOSE 0"
                    && action.playtime_seconds == Some(812)
        ));
    }

    #[test]
    fn parse_trace_preserves_target_command_errors() {
        let content = r#"{"type":"action","step":7,"command":"POTION USE 1"}
{"type":"error","step":7,"message":{"error":"Potion cannot be used"}}"#;

        let lines = parse_trace_jsonl(content).expect("parses");
        assert!(matches!(
            &lines[1],
            TraceLine::Error(error)
                if error.step == 7 && error.message["error"] == "Potion cannot be used"
        ));
    }

    #[test]
    fn parse_trace_preserves_explicit_boss_unlock_inputs() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod","boss_unlocks":{"guardian_seen":false,"hexaghost_seen":true,"slime_boss_seen":true,"champ_seen":true,"automaton_seen":true,"collector_seen":true,"awakened_one_seen":true,"donu_deca_seen":true,"time_eater_seen":true}}"#;

        let trace = import_communication_mod_trace(content).expect("parses");
        let unlocks = trace
            .metadata
            .and_then(|metadata| metadata.boss_unlocks)
            .expect("boss unlock inputs");
        assert!(!unlocks.guardian_seen);
        assert!(unlocks.hexaghost_seen);
    }
}
