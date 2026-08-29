use crate::{
    bridge::BridgeManager,
    cli::run_cli_with_events,
    cli_output::format_cli_error,
    fidelity::FidelityChecker,
    model::{LiveResult, SessionId, SessionSnapshot},
    SessionStore,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const AGENT_PROTOCOL_SCHEMA: u32 = 1;

#[derive(Debug, Deserialize)]
struct AgentRequest {
    #[serde(default)]
    request_id: Option<Value>,
    command: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    action_id: Option<String>,
    #[serde(default)]
    target_floor: Option<u32>,
}

pub fn run_slaythedata_agent<B, F, R, W>(
    store: &mut SessionStore<B, F>,
    collect_options: Vec<String>,
    reader: R,
    mut writer: W,
) -> LiveResult<()>
where
    B: BridgeManager,
    F: FidelityChecker,
    R: BufRead,
    W: Write,
{
    let collect_args = one_run_collect_args(&collect_options);
    let default_target_floor = option_value(&collect_options, "--target-floor")
        .and_then(|value| value.parse().ok())
        .unwrap_or(60);
    let resume_output_args = resume_output_args(&collect_options);
    write_event(
        &mut writer,
        &json!({
            "type": "agent_ready",
            "schema": AGENT_PROTOCOL_SCHEMA,
            "pid": std::process::id(),
            "executable": std::env::current_exe().ok(),
            "commands": ["inspect", "act", "skip_shop", "resume", "next_run", "abandon", "ping", "stop"],
        }),
    )?;

    execute(
        store,
        &mut writer,
        None,
        "initial_collect",
        collect_args.clone(),
    )?;

    for (line_index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: AgentRequest = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_event(
                    &mut writer,
                    &json!({
                        "type": "protocol_error",
                        "schema": AGENT_PROTOCOL_SCHEMA,
                        "line": line_index + 1,
                        "message": error.to_string(),
                    }),
                )?;
                continue;
            }
        };
        let request_id = request.request_id.clone();
        match request.command.as_str() {
            "ping" => write_event(
                &mut writer,
                &json!({
                    "type": "pong",
                    "schema": AGENT_PROTOCOL_SCHEMA,
                    "request_id": request_id,
                }),
            )?,
            "stop" => {
                write_event(
                    &mut writer,
                    &json!({
                        "type": "agent_stopped",
                        "schema": AGENT_PROTOCOL_SCHEMA,
                        "request_id": request_id,
                    }),
                )?;
                return Ok(());
            }
            "next_run" => execute(
                store,
                &mut writer,
                request_id,
                "next_run",
                collect_args.clone(),
            )?,
            "inspect" => {
                let Some(session_id) = request.session_id.as_deref() else {
                    write_missing_field(&mut writer, request_id, "session_id")?;
                    continue;
                };
                execute(
                    store,
                    &mut writer,
                    request_id,
                    "inspect",
                    strings(["sessions", "request-state", session_id]),
                )?;
            }
            "act" => {
                let Some(session_id) = request.session_id.as_deref() else {
                    write_missing_field(&mut writer, request_id, "session_id")?;
                    continue;
                };
                let Some(action_id) = request.action_id.as_deref() else {
                    write_missing_field(&mut writer, request_id, "action_id")?;
                    continue;
                };
                execute(
                    store,
                    &mut writer,
                    request_id,
                    "act",
                    strings(["actions", "send", session_id, action_id]),
                )?;
            }
            "skip_shop" => {
                let Some(session_id) = request.session_id.as_deref() else {
                    write_missing_field(&mut writer, request_id, "session_id")?;
                    continue;
                };
                execute(
                    store,
                    &mut writer,
                    request_id,
                    "skip_shop",
                    strings(["slaythedata", "skip-shop", session_id]),
                )?;
            }
            "resume" => {
                let Some(session_id) = request.session_id.as_deref() else {
                    write_missing_field(&mut writer, request_id, "session_id")?;
                    continue;
                };
                let mut args = strings([
                    "slaythedata",
                    "resume",
                    session_id,
                    "--target-floor",
                    &request
                        .target_floor
                        .unwrap_or(default_target_floor)
                        .to_string(),
                ]);
                args.extend(resume_output_args.clone());
                execute(store, &mut writer, request_id, "resume", args)?;
            }
            "abandon" => {
                let Some(session_id) = request.session_id.as_deref() else {
                    write_missing_field(&mut writer, request_id, "session_id")?;
                    continue;
                };
                execute(
                    store,
                    &mut writer,
                    request_id,
                    "abandon",
                    strings(["sessions", "abandon", session_id]),
                )?;
            }
            other => write_event(
                &mut writer,
                &json!({
                    "type": "protocol_error",
                    "schema": AGENT_PROTOCOL_SCHEMA,
                    "request_id": request_id,
                    "message": format!("unsupported agent command {other}"),
                }),
            )?,
        }
    }
    Ok(())
}

fn execute<B, F, W>(
    store: &mut SessionStore<B, F>,
    writer: &mut W,
    request_id: Option<Value>,
    command: &str,
    args: Vec<String>,
) -> LiveResult<()>
where
    B: BridgeManager,
    F: FidelityChecker,
    W: Write,
{
    write_event(
        writer,
        &json!({
            "type": "command_started",
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": request_id,
            "command": command,
        }),
    )?;
    let mut write_error = None;
    let result = {
        let mut emit = |mut event: Value| {
            event["schema"] = json!(AGENT_PROTOCOL_SCHEMA);
            event["request_id"] = request_id.clone().unwrap_or(Value::Null);
            if let Err(error) = write_event(writer, &event) {
                write_error = Some(error);
            }
        };
        run_cli_with_events(store, args, &mut emit)
    };
    if let Some(error) = write_error {
        return Err(error);
    }
    match result {
        Ok(value) => {
            write_event(
                writer,
                &json!({
                    "type": "command_finished",
                    "schema": AGENT_PROTOCOL_SCHEMA,
                    "request_id": request_id,
                    "command": command,
                    "result": compact_result(&value),
                }),
            )?;
            if let Some(session_id) = result_session_id(&value) {
                if let Ok(snapshot) = store.session_snapshot(&SessionId(session_id.to_owned())) {
                    let mut packet = decision_packet(&snapshot);
                    packet["request_id"] = request_id.unwrap_or(Value::Null);
                    write_event(writer, &packet)?;
                }
            }
        }
        Err(error) => {
            let payload = serde_json::from_str::<Value>(&format_cli_error(&error))
                .unwrap_or_else(|_| json!({"error": {"message": error.to_string()}}));
            write_event(
                writer,
                &json!({
                    "type": "command_error",
                    "schema": AGENT_PROTOCOL_SCHEMA,
                    "request_id": request_id,
                    "command": command,
                    "error": payload.get("error"),
                }),
            )?;
        }
    }
    Ok(())
}

fn compact_result(value: &Value) -> Value {
    let attempt = value
        .get("attempts")
        .and_then(Value::as_array)
        .and_then(|attempts| attempts.last())
        .unwrap_or(value);
    json!({
        "status": attempt.get("status").or_else(|| value.get("status")),
        "reason": attempt.get("reason").or_else(|| value.get("reason")),
        "blocker_kind": attempt.get("blocker_kind").or_else(|| value.get("blocker_kind")),
        "run_id": attempt.get("run_id").or_else(|| value.get("run_id")),
        "session_id": attempt.get("session_id").or_else(|| value.get("session_id")),
        "floor": attempt.get("floor").or_else(|| value.get("floor")),
        "phase": attempt.get("phase").or_else(|| value.get("phase")),
        "fidelity": attempt.get("fidelity").or_else(|| value.get("fidelity")),
        "strict_verification": attempt.get("strict_verification").or_else(|| value.get("strict_verification")),
        "promotion": attempt.get("promotion").or_else(|| value.get("promotion")),
        "marked_broken": attempt.get("marked_broken").or_else(|| value.get("marked_broken")),
        "processed_runs": value.get("processed_runs"),
        "promoted_traces": value.get("promoted_traces"),
        "message": attempt.get("message").or_else(|| value.get("message")),
    })
}

fn result_session_id(value: &Value) -> Option<&str> {
    value
        .get("session_id")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("attempts")
                .and_then(Value::as_array)
                .and_then(|attempts| attempts.last())
                .and_then(|attempt| attempt.get("session_id"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            value
                .pointer("/repair_packet/session_id")
                .and_then(Value::as_str)
        })
}

fn decision_packet(snapshot: &SessionSnapshot) -> Value {
    let state = snapshot.latest_state.as_ref();
    let raw = state.map(|state| &state.raw);
    let summary = raw.and_then(|raw| raw.pointer("/summary"));
    let game = raw.and_then(|raw| raw.pointer("/current_state/message/game_state"));
    let screen = game.and_then(|game| game.get("screen_state"));
    let is_shop = summary
        .and_then(|summary| summary.get("screen_type"))
        .and_then(Value::as_str)
        == Some("SHOP_SCREEN");
    json!({
        "type": "decision",
        "schema": AGENT_PROTOCOL_SCHEMA,
        "session_id": snapshot.session_id,
        "run_id": snapshot.slaythedata.attached_run.as_ref().map(|run| run.id),
        "floor": summary.and_then(|summary| summary.get("floor")),
        "phase": state.map(|state| format!("{:?}", state.phase).to_lowercase()),
        "room_type": summary.and_then(|summary| summary.get("room_type")),
        "screen_type": summary.and_then(|summary| summary.get("screen_type")),
        "hp": {
            "current": summary.and_then(|summary| summary.get("current_hp")),
            "max": summary.and_then(|summary| summary.get("max_hp")),
        },
        "gold": summary.and_then(|summary| summary.get("gold")),
        "deck": game.and_then(|game| game.get("deck")),
        "relics": game.and_then(|game| game.get("relics")),
        "potions": game.and_then(|game| game.get("potions")),
        "shop": screen.filter(|_| is_shop),
        "screen": screen.filter(|_| !is_shop),
        "route": {
            "act": game.and_then(|game| game.get("act")),
            "boss": game.and_then(|game| game.get("act_boss")),
            "floor": game.and_then(|game| game.get("floor")),
            "current_node": screen.and_then(|screen| screen.get("current_node")),
            "next_nodes": screen.and_then(|screen| screen.get("next_nodes")),
            "boss_available": screen.and_then(|screen| screen.get("boss_available")),
        },
        "combat": game.and_then(|game| game.get("combat_state")),
        "legal_actions": state.map(|state| &state.legal_actions),
        "slaythedata": snapshot.slaythedata,
        "automation": {
            "state": &snapshot.automation.state,
            "policy": &snapshot.automation.policy,
            "config": &snapshot.automation.config,
            "planned_action": &snapshot.automation.planned_action,
            "plan": &snapshot.automation.plan,
            "blocked": &snapshot.automation.blocked,
            "last_message": &snapshot.automation.last_message,
            "executed_action_count": snapshot.automation.executed_actions.len(),
        },
        "fidelity": snapshot.fidelity,
    })
}

fn one_run_collect_args(options: &[String]) -> Vec<String> {
    let mut args = strings(["slaythedata", "collect"]);
    let mut index = 0;
    while index < options.len() {
        if options[index] == "--limit" {
            index += 2;
            continue;
        }
        args.push(options[index].clone());
        index += 1;
    }
    args.extend(strings(["--limit", "1"]));
    args
}

fn resume_output_args(options: &[String]) -> Vec<String> {
    let mut output = Vec::new();
    let mut index = 0;
    while index < options.len() {
        match options[index].as_str() {
            "--journal" | "--permanent-root" | "--promote-floor" => {
                if let Some(value) = options.get(index + 1) {
                    output.push(options[index].clone());
                    output.push(value.clone());
                }
                index += 2;
            }
            "--no-promote" => {
                output.push(options[index].clone());
                index += 1;
            }
            _ => index += 1,
        }
    }
    output
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == option)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn write_missing_field(
    writer: &mut impl Write,
    request_id: Option<Value>,
    field: &str,
) -> LiveResult<()> {
    write_event(
        writer,
        &json!({
            "type": "protocol_error",
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": request_id,
            "message": format!("agent request requires field {field}"),
        }),
    )
}

fn write_event(writer: &mut impl Write, value: &Value) -> LiveResult<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn strings<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ActionId, AutomationJobSnapshot, BridgeId, FidelityStatus, LegalAction, LegalActionKind,
        LivePhase, LiveState, SessionLifecycle, SlayTheDataSessionSnapshot,
    };

    #[test]
    fn agent_forces_one_run_per_decision_cycle() {
        assert_eq!(
            one_run_collect_args(&strings([
                "--ascension",
                "0",
                "--victory",
                "--limit",
                "20",
                "--target-floor",
                "60",
            ])),
            strings([
                "slaythedata",
                "collect",
                "--ascension",
                "0",
                "--victory",
                "--target-floor",
                "60",
                "--limit",
                "1",
            ])
        );
    }

    #[test]
    fn resume_preserves_collection_output_contract() {
        assert_eq!(
            resume_output_args(&strings([
                "--journal",
                "journal.jsonl",
                "--repair-packet",
                "repair.json",
                "--permanent-root",
                "corpus",
                "--promote-floor",
                "11",
                "--no-promote",
            ])),
            strings([
                "--journal",
                "journal.jsonl",
                "--permanent-root",
                "corpus",
                "--promote-floor",
                "11",
                "--no-promote",
            ])
        );
    }

    #[test]
    fn decision_packet_contains_shop_and_run_context() {
        let snapshot = SessionSnapshot {
            session_id: SessionId("session-7".to_owned()),
            bridge_id: BridgeId("bridge-1".to_owned()),
            lifecycle: SessionLifecycle::Blocked,
            trace_path: "session-7.jsonl".to_owned(),
            run_config: None,
            latest_state: Some(LiveState {
                sequence: 9,
                phase: LivePhase::Shop,
                legal_actions: vec![LegalAction {
                    id: ActionId("choose-1".to_owned()),
                    kind: LegalActionKind::ShopBuy,
                    label: "Feed".to_owned(),
                    enabled: true,
                    command: json!({"command": "CHOOSE 1"}),
                    disabled_reason: None,
                }],
                raw: json!({
                    "summary": {
                        "floor": 11,
                        "current_hp": 42,
                        "max_hp": 80,
                        "gold": 103,
                        "room_type": "ShopRoom",
                        "screen_type": "SHOP_SCREEN"
                    },
                    "current_state": {"message": {"game_state": {
                        "act": 1,
                        "act_boss": "Hexaghost",
                        "floor": 11,
                        "deck": [{"name": "Strike"}],
                        "relics": [{"name": "Burning Blood"}],
                        "potions": [],
                        "map": [{"x": 2, "y": 10, "symbol": "$"}],
                        "screen_state": {
                            "cards": [{"name": "Feed", "price": 77}],
                            "relics": [{"name": "Lantern", "price": 149}],
                            "purge_available": true,
                            "purge_cost": 75
                        }
                    }}}
                }),
            }),
            fidelity: FidelityStatus::unknown(),
            blocked: None,
            automation: AutomationJobSnapshot::default(),
            slaythedata: SlayTheDataSessionSnapshot::default(),
        };

        let packet = decision_packet(&snapshot);

        assert_eq!(packet["type"], "decision");
        assert_eq!(packet["hp"]["current"], 42);
        assert_eq!(packet["shop"]["cards"][0]["price"], 77);
        assert_eq!(packet["shop"]["purge_cost"], 75);
        assert!(packet["screen"].is_null());
        assert_eq!(packet["deck"][0]["name"], "Strike");
        assert_eq!(packet["route"]["boss"], "Hexaghost");
        assert!(packet["route"].get("map").is_none());
        assert_eq!(packet["legal_actions"][0]["id"], "choose-1");
        assert!(packet["automation"].get("executed_actions").is_none());
        assert_eq!(packet["automation"]["executed_action_count"], 0);
    }

    #[test]
    fn terminal_result_is_compact_but_keeps_verification() {
        let compact = compact_result(&json!({
            "status": "blocked",
            "processed_runs": 1,
            "attempts": [{
                "status": "simulator_mismatch",
                "reason": "fidelity_lost",
                "run_id": 42,
                "session_id": "session-9",
                "strict_verification": {"clean": false, "max_floor": 7},
                "repair_packet": {"large": [1, 2, 3]}
            }],
            "repair_packet": {"very_large": true}
        }));

        assert_eq!(compact["status"], "simulator_mismatch");
        assert_eq!(compact["run_id"], 42);
        assert_eq!(compact["strict_verification"]["max_floor"], 7);
        assert!(compact.get("repair_packet").is_none());
    }
}
