use crate::model::{ActionId, LegalAction, LegalActionKind, LivePhase, LiveState};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};

use super::files::BridgeFiles;

pub(crate) fn live_state_from_protocol_state(value: &Value) -> LiveState {
    let mut summary = value.get("summary").cloned().unwrap_or(Value::Null);
    if summary.is_null()
        && value
            .get("status")
            .and_then(|status| status.get("status"))
            .and_then(Value::as_str)
            == Some("ready")
    {
        summary = json!({
            "step": value.get("step").and_then(Value::as_u64).unwrap_or_default(),
            "state_seq": value.get("state_seq").and_then(Value::as_u64).unwrap_or_default(),
            "available_commands": ["start", "state"],
            "ready_for_command": true,
            "in_game": false,
            "screen_type": "MENU",
            "client_pid": value.get("client_pid"),
            "trace_path": value.get("trace_path"),
        });
    }
    let files = BridgeFiles {
        status: protocol_status(value),
        summary,
        current_state: value.get("state").cloned().unwrap_or(Value::Null),
        status_age: Some(Duration::ZERO),
        summary_age: Some(Duration::ZERO),
    };
    live_state_from_files(&files)
}

pub(crate) fn bridge_files_from_protocol_state(value: &Value) -> BridgeFiles {
    let state = live_state_from_protocol_state(value);
    BridgeFiles {
        status: protocol_status(value),
        summary: state.raw.get("summary").cloned().unwrap_or(Value::Null),
        current_state: state
            .raw
            .get("current_state")
            .cloned()
            .unwrap_or(Value::Null),
        status_age: Some(Duration::ZERO),
        summary_age: Some(Duration::ZERO),
    }
}

fn protocol_status(value: &Value) -> Value {
    let mut status = value.get("status").cloned().unwrap_or(Value::Null);
    if let Some(object) = status.as_object_mut() {
        if let Some(pending) = value.get("pending_command") {
            object.insert("pending_command".to_owned(), pending.clone());
        }
        if let Some(in_flight) = value.get("command_in_flight") {
            object.insert("command_in_flight".to_owned(), in_flight.clone());
        }
    }
    status
}

pub(crate) fn live_state_from_files(files: &BridgeFiles) -> LiveState {
    let sequence = files
        .summary
        .get("state_seq")
        .or_else(|| files.current_state.get("state_seq"))
        .or_else(|| files.summary.get("step"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let source_state_id = files
        .summary
        .get("state_id")
        .or_else(|| files.current_state.get("state_id"))
        .and_then(Value::as_str);
    LiveState {
        sequence,
        phase: phase_from_summary(&files.summary),
        legal_actions: actions_from_summary(&files.summary, source_state_id),
        raw: json!({
            "status": files.status,
            "summary": files.summary,
            "current_state": files.current_state,
        }),
    }
}

pub(crate) fn available_commands(summary: &Value) -> HashSet<String> {
    summary
        .get("available_commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|command| command.to_ascii_lowercase())
        .collect()
}

fn actions_from_summary(summary: &Value, source_state_id: Option<&str>) -> Vec<LegalAction> {
    let available = available_commands(summary);
    let disabled = disabled_reason(summary);
    let phase = phase_from_summary(summary);
    let mut actions = vec![bridge_action(
        "request-state",
        LegalActionKind::RequestState,
        "Request state",
        "STATE",
        None,
        None,
    )];

    add_choice_actions(
        &mut actions,
        summary,
        &available,
        phase,
        disabled.clone(),
        source_state_id,
    );
    add_card_actions(
        &mut actions,
        summary,
        &available,
        disabled.clone(),
        source_state_id,
    );
    add_operator_actions(&mut actions, summary, disabled.clone(), source_state_id);
    add_simple_actions(&mut actions, &available, disabled, source_state_id);
    actions
}

fn add_choice_actions(
    actions: &mut Vec<LegalAction>,
    summary: &Value,
    available: &HashSet<String>,
    phase: LivePhase,
    disabled: Option<String>,
    source_state_id: Option<&str>,
) {
    if !available.contains("choose") {
        return;
    }
    let choose_kind = match phase {
        LivePhase::Neow => LegalActionKind::ChooseNeow,
        LivePhase::Map => LegalActionKind::ChooseMapNode,
        LivePhase::Reward => LegalActionKind::ChooseReward,
        LivePhase::Event => LegalActionKind::EventChoice,
        LivePhase::Shop => LegalActionKind::ShopBuy,
        LivePhase::Rest => LegalActionKind::RestSite,
        _ => LegalActionKind::Confirm,
    };
    if let Some(choices) = summary.get("choices").and_then(Value::as_array) {
        for (index, choice) in choices.iter().enumerate() {
            if should_hide_reward_choice(summary, &phase, choice) {
                continue;
            }
            actions.push(bridge_action(
                &format!("choose-{index}"),
                choose_kind.clone(),
                choice.as_str().unwrap_or("Choose"),
                &format!("CHOOSE {index}"),
                disabled.clone(),
                source_state_id,
            ));
        }
    }
}

fn should_hide_reward_choice(summary: &Value, phase: &LivePhase, choice: &Value) -> bool {
    if phase != &LivePhase::Reward {
        return false;
    }
    let Some(label) = choice.as_str() else {
        return false;
    };
    label.eq_ignore_ascii_case("potion")
        && summary
            .get("open_potion_slots")
            .and_then(Value::as_i64)
            .is_some_and(|slots| slots <= 0)
}

fn add_card_actions(
    actions: &mut Vec<LegalAction>,
    summary: &Value,
    available: &HashSet<String>,
    disabled: Option<String>,
    source_state_id: Option<&str>,
) {
    if !available.contains("play") {
        return;
    }
    let Some(hand) = summary.pointer("/combat/hand").and_then(Value::as_array) else {
        return;
    };
    let monsters = summary
        .pointer("/combat/monsters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for card in hand {
        add_card_action(actions, card, &monsters, disabled.clone(), source_state_id);
    }
}

fn add_card_action(
    actions: &mut Vec<LegalAction>,
    card: &Value,
    monsters: &[Value],
    disabled: Option<String>,
    source_state_id: Option<&str>,
) {
    if card.get("playable").and_then(Value::as_bool) == Some(false) {
        return;
    }
    let Some(hand_slot) = card.get("index").and_then(Value::as_u64) else {
        return;
    };
    let card_label = card
        .get("name")
        .or_else(|| card.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("Card");
    if card
        .get("has_target")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        for monster in monsters
            .iter()
            .filter(|m| m.get("gone").and_then(Value::as_bool) != Some(true))
        {
            let Some(target_slot) = monster.get("index").and_then(Value::as_u64) else {
                continue;
            };
            let monster_label = monster
                .get("name")
                .or_else(|| monster.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("Monster");
            actions.push(bridge_action(
                &format!("play-{hand_slot}-{target_slot}"),
                LegalActionKind::PlayCard,
                &format!("Play {card_label} -> {monster_label}"),
                &format!("PLAY {hand_slot} {target_slot}"),
                disabled.clone(),
                source_state_id,
            ));
        }
    } else {
        actions.push(bridge_action(
            &format!("play-{hand_slot}"),
            LegalActionKind::PlayCard,
            &format!("Play {card_label}"),
            &format!("PLAY {hand_slot}"),
            disabled,
            source_state_id,
        ));
    }
}

fn add_simple_actions(
    actions: &mut Vec<LegalAction>,
    available: &HashSet<String>,
    disabled: Option<String>,
    source_state_id: Option<&str>,
) {
    for (verb, label, kind) in [
        ("end", "End turn", LegalActionKind::EndTurn),
        ("proceed", "Proceed", LegalActionKind::Confirm),
        ("confirm", "Confirm", LegalActionKind::Confirm),
        ("skip", "Skip", LegalActionKind::SkipReward),
    ] {
        if available.contains(verb) {
            actions.push(bridge_action(
                verb,
                kind,
                label,
                &verb.to_ascii_uppercase(),
                disabled.clone(),
                source_state_id,
            ));
        }
    }
}

fn add_operator_actions(
    actions: &mut Vec<LegalAction>,
    summary: &Value,
    disabled: Option<String>,
    source_state_id: Option<&str>,
) {
    if summary.get("in_game").and_then(Value::as_bool) != Some(true) {
        return;
    }
    actions.push(bridge_action(
        "abandon-run",
        LegalActionKind::AbandonRun,
        "Abandon run",
        "ABANDON",
        disabled,
        source_state_id,
    ));
}

fn bridge_action(
    id: &str,
    kind: LegalActionKind,
    label: &str,
    command: &str,
    disabled_reason: Option<String>,
    source_state_id: Option<&str>,
) -> LegalAction {
    LegalAction {
        id: ActionId(id.to_owned()),
        kind,
        label: label.to_owned(),
        enabled: disabled_reason.is_none(),
        command: json!({
            "transport": "communication_mod",
            "command": command,
            "source_state_id": source_state_id,
        }),
        disabled_reason,
    }
}

fn phase_from_summary(summary: &Value) -> LivePhase {
    let screen = summary
        .get("screen_type")
        .or_else(|| summary.get("screen_name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    let room_phase = summary
        .get("room_phase")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    let room_type = summary
        .get("room_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match screen.as_str() {
        "MENU" => LivePhase::Menu,
        "MAP" => LivePhase::Map,
        "COMBAT" => LivePhase::Combat,
        "COMBAT_REWARD" | "CARD_REWARD" | "GRID" | "BOSS_REWARD" => LivePhase::Reward,
        "SHOP" => LivePhase::Shop,
        "REST" => LivePhase::Rest,
        "GAME_OVER" => LivePhase::GameOver,
        "EVENT" if room_type == "NeowRoom" => LivePhase::Neow,
        "EVENT" => LivePhase::Event,
        _ if room_phase == "COMBAT" || summary.get("combat").is_some() => LivePhase::Combat,
        _ if room_type == "NeowRoom" => LivePhase::Neow,
        _ => LivePhase::Unknown,
    }
}

fn disabled_reason(summary: &Value) -> Option<String> {
    if summary.get("ready_for_command").and_then(Value::as_bool) == Some(true) {
        None
    } else {
        Some("bridge is not ready for a command".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::bridge_files_from_protocol_state;
    use serde_json::json;

    #[test]
    fn protocol_state_status_uses_fresh_top_level_pending_fields() {
        let files = bridge_files_from_protocol_state(&json!({
            "ok": true,
            "client_pid": 1234,
            "step": 9,
            "state_seq": 84,
            "state_id": "state-84",
            "pending_command": false,
            "command_in_flight": null,
            "summary": {
                "state_seq": 84,
                "state_id": "state-84",
                "available_commands": ["choose", "state"],
                "ready_for_command": true,
                "in_game": true
            },
            "state": {
                "message": {
                    "available_commands": ["choose", "state"],
                    "ready_for_command": true
                }
            },
            "status": {
                "pending_command": true,
                "command_in_flight": {
                    "command": "CHOOSE 0",
                    "accepted_state_seq": 84
                }
            }
        }));

        assert_eq!(
            files
                .status
                .get("pending_command")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert!(files.status.get("command_in_flight").unwrap().is_null());
    }
}
