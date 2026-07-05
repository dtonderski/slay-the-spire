use crate::{
    bridge::BridgeManager,
    fidelity::FidelityChecker,
    model::{
        AutomationConfig, AutomationPolicy, BridgeId, Character, LiveError, LiveResult, RunConfig,
        RunSeed, SessionId,
    },
    session::SessionStore,
};
use serde_json::{json, Value};

pub fn run_cli<B, F>(
    store: &mut SessionStore<B, F>,
    args: impl IntoIterator<Item = String>,
) -> LiveResult<Value>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [area, command] if area == "bridges" && command == "list" => {
            Ok(json!({"bridges": store.list_bridges()?}))
        }
        [area, command, flag] if area == "bridges" && command == "kill" && flag == "--all" => {
            Ok(json!({"killed": store.kill_all_bridges()?}))
        }
        [area, command, bridge_id]
            if area == "bridges" && command == "kill" && bridge_id != "--all" =>
        {
            store.kill_bridge(&BridgeId(bridge_id.clone()))?;
            Ok(json!({"killed": 1, "bridge_id": bridge_id}))
        }
        [area, command] if area == "sessions" && command == "list" => {
            Ok(json!({"sessions": store.list_sessions()}))
        }
        [area, command, rest @ ..] if area == "sessions" && command == "start" => {
            let request = parse_start_args(rest)?;
            Ok(serde_json::to_value(
                store.start_run(request.bridge_id, request.config)?,
            )?)
        }
        [area, command, session_id] if area == "sessions" && command == "state" => Ok(
            serde_json::to_value(store.session_snapshot(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "sessions" && command == "request-state" => Ok(
            serde_json::to_value(store.request_state(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "sessions" && command == "abandon" => Ok(
            serde_json::to_value(
                store.abandon_run(&SessionId(session_id.clone()), "operator_cli")?,
            )?,
        ),
        [area, command, session_id] if area == "actions" && command == "list" => Ok(json!({
            "session_id": session_id,
            "legal_actions": store.actions(&SessionId(session_id.clone()))?,
        })),
        [area, command, session_id, action_id] if area == "actions" && command == "send" => {
            Ok(serde_json::to_value(store.send_action(
                &SessionId(session_id.clone()),
                &crate::model::ActionId(action_id.clone()),
            )?)?)
        }
        [area, command, session_id] if area == "automation" && command == "status" => Ok(
            serde_json::to_value(store.automation_status(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id, rest @ ..]
            if area == "automation" && command == "configure" =>
        {
            let config = parse_automation_config(rest)?;
            Ok(serde_json::to_value(store.configure_automation(
                &SessionId(session_id.clone()),
                config,
            )?)?)
        }
        [area, command, session_id] if area == "automation" && command == "plan" => Ok(
            serde_json::to_value(store.automation_plan(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "send-ready" => Ok(
            serde_json::to_value(store.automation_send_ready(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id]
            if area == "automation" && (command == "step" || command == "run-one") =>
        {
            Ok(serde_json::to_value(
                store.automation_step(&SessionId(session_id.clone()))?,
            )?)
        }
        [area, command, session_id] if area == "automation" && command == "auto-play" => Ok(
            serde_json::to_value(store.automation_auto_play(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "pause" => Ok(
            serde_json::to_value(store.automation_pause(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "resume" => Ok(
            serde_json::to_value(store.automation_resume(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "automation" && command == "cancel" => Ok(
            serde_json::to_value(store.automation_cancel(&SessionId(session_id.clone()))?)?,
        ),
        [area, command, session_id] if area == "fidelity" && command == "status" => {
            let snapshot = store.session_snapshot(&SessionId(session_id.clone()))?;
            Ok(serde_json::to_value(snapshot.fidelity)?)
        }
        [area, command, session_id] if area == "trace" && command == "path" => {
            let snapshot = store.session_snapshot(&SessionId(session_id.clone()))?;
            Ok(json!({"session_id": session_id, "trace_path": snapshot.trace_path}))
        }
        _ => Err(LiveError::InvalidAction(usage())),
    }
}

struct StartRequest {
    bridge_id: BridgeId,
    config: RunConfig,
}

fn parse_start_args(args: &[String]) -> LiveResult<StartRequest> {
    let mut bridge_id = BridgeId("fake-bridge-1".to_owned());
    let mut character = Character::Ironclad;
    let mut ascension = 0;
    let mut seed: Option<RunSeed> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bridge" => {
                index += 1;
                bridge_id = BridgeId(required(args, index, "--bridge")?.to_owned());
            }
            "--character" => {
                index += 1;
                character = match required(args, index, "--character")?
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "ironclad" => Character::Ironclad,
                    other => {
                        return Err(LiveError::InvalidAction(format!(
                            "unsupported character {other}"
                        )))
                    }
                };
            }
            "--ascension" => {
                index += 1;
                ascension = required(args, index, "--ascension")?
                    .parse()
                    .map_err(|err| LiveError::InvalidAction(format!("invalid ascension: {err}")))?;
            }
            "--seed" => {
                index += 1;
                let value = required(args, index, "--seed")?;
                seed = Some(
                    value
                        .parse::<i64>()
                        .map_or_else(|_| RunSeed::External(value.to_owned()), RunSeed::Numeric),
                );
            }
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    let seed = seed.ok_or_else(|| LiveError::InvalidAction("--seed is required".to_owned()))?;
    Ok(StartRequest {
        bridge_id,
        config: RunConfig {
            character,
            ascension,
            seed,
        },
    })
}

fn parse_automation_config(args: &[String]) -> LiveResult<AutomationConfig> {
    let mut config = AutomationConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--policy" => {
                index += 1;
                config.policy = parse_automation_policy(required(args, index, "--policy")?)?;
            }
            "--depth" => {
                index += 1;
                config.depth = parse_usize(required(args, index, "--depth")?, "--depth")?;
            }
            "--width" => {
                index += 1;
                config.width = parse_usize(required(args, index, "--width")?, "--width")?;
            }
            "--auto-action-limit" => {
                index += 1;
                config.auto_action_limit = parse_usize(
                    required(args, index, "--auto-action-limit")?,
                    "--auto-action-limit",
                )?;
            }
            "--potions" | "--potion-slots" => {
                index += 1;
                config.allowed_potion_slots =
                    parse_potion_slots(required(args, index, "--potions")?)?;
            }
            other => return Err(LiveError::InvalidAction(format!("unknown flag {other}"))),
        }
        index += 1;
    }
    Ok(config)
}

fn parse_automation_policy(value: &str) -> LiveResult<AutomationPolicy> {
    match value {
        "fake-play-first-card" | "fake_play_first_card" => Ok(AutomationPolicy::FakePlayFirstCard),
        "greedy-search" | "greedy_search" | "greedy" => Ok(AutomationPolicy::GreedySearch),
        "beam-search" | "beam_search" | "beam" => Ok(AutomationPolicy::BeamSearch),
        other => Err(LiveError::InvalidAction(format!(
            "unsupported automation policy {other}"
        ))),
    }
}

fn parse_usize(value: &str, flag: &str) -> LiveResult<usize> {
    value
        .parse()
        .map_err(|err| LiveError::InvalidAction(format!("invalid {flag}: {err}")))
}

fn parse_potion_slots(value: &str) -> LiveResult<Vec<usize>> {
    if value.trim().is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|slot| {
            slot.trim()
                .parse()
                .map_err(|err| LiveError::InvalidAction(format!("invalid potion slot: {err}")))
        })
        .collect()
}

fn required<'a>(args: &'a [String], index: usize, flag: &str) -> LiveResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| LiveError::InvalidAction(format!("{flag} requires a value")))
}

fn usage() -> String {
    "usage: live-trace bridges list|kill [--all|bridge-id]; live-trace sessions list|start|state|request-state|abandon; live-trace actions list SESSION; live-trace actions send SESSION ACTION; live-trace automation status|configure|plan|send-ready|step|run-one|auto-play|pause|resume|cancel SESSION; live-trace fidelity status SESSION; live-trace trace path SESSION".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bridge::FakeBridgeManager, fidelity::TraceFidelityChecker};
    use std::{fs, time::SystemTime};

    #[test]
    fn cli_lists_bridges_and_starts_run() {
        let root = temp_dir("cli-start");
        let mut store = fake_store(&root);
        let bridges = run_cli(&mut store, strings(["bridges", "list"])).unwrap();
        assert_eq!(bridges["bridges"].as_array().unwrap().len(), 1);

        let session = run_cli(
            &mut store,
            strings([
                "sessions",
                "start",
                "--character",
                "ironclad",
                "--ascension",
                "0",
                "--seed",
                "CODEX04",
            ]),
        )
        .unwrap();
        assert_eq!(session["session_id"], "session-1");

        let actions = run_cli(&mut store, strings(["actions", "list", "session-1"])).unwrap();
        assert!(!actions["legal_actions"].as_array().unwrap().is_empty());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_sends_action_with_same_backend_store() {
        let root = temp_dir("cli-send");
        let mut store = fake_store(&root);
        run_cli(
            &mut store,
            strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
        )
        .unwrap();
        let sent = run_cli(
            &mut store,
            strings(["actions", "send", "session-1", "talk"]),
        )
        .unwrap();
        assert_eq!(sent["latest_state"]["phase"], "combat");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_can_continue_recovered_trace_session() {
        let root = temp_dir("cli-recover");
        {
            let mut store = fake_store(&root);
            run_cli(
                &mut store,
                strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
            )
            .unwrap();
        }

        let mut recovered = fake_store(&root);
        recovered.recover_existing_sessions().unwrap();
        let sessions = run_cli(&mut recovered, strings(["sessions", "list"])).unwrap();
        assert_eq!(sessions["sessions"][0]["session_id"], "session-1");

        let actions = run_cli(&mut recovered, strings(["actions", "list", "session-1"])).unwrap();
        assert!(actions["legal_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["id"] == "talk"));

        let sent = run_cli(
            &mut recovered,
            strings(["actions", "send", "session-1", "talk"]),
        )
        .unwrap();
        assert_eq!(sent["latest_state"]["phase"], "combat");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn cli_abandons_active_session() {
        let root = temp_dir("cli-abandon");
        let mut store = fake_store(&root);
        run_cli(
            &mut store,
            strings(["sessions", "start", "--seed", "123", "--ascension", "0"]),
        )
        .unwrap();
        let abandoned = run_cli(&mut store, strings(["sessions", "abandon", "session-1"])).unwrap();
        assert_eq!(abandoned["lifecycle"], "ended");
        assert_eq!(abandoned["latest_state"]["phase"], "menu");
        fs::remove_dir_all(root).ok();
    }

    fn fake_store(root: &std::path::Path) -> SessionStore<FakeBridgeManager, TraceFidelityChecker> {
        SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            root,
        )
    }

    fn strings<const N: usize>(items: [&str; N]) -> Vec<String> {
        items.into_iter().map(str::to_owned).collect()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sts-live-cli-{name}-{nonce}"))
    }
}
