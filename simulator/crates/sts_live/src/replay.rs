//! Fail-closed replay of verified CommunicationMod traces against a live game.
//!
//! `ReplayMode::ActionTemplate` is an explicit recollection mode for traces
//! whose observations are known to be unusable. It reads only their command
//! stream, preserves the source file, and records a fresh trace from the live
//! bridge. Ordinary replay remains strict and refuses such sources.

use crate::{
    bridge::BridgeManager,
    fidelity::FidelityChecker,
    model::{
        ActionId, BridgeId, Character, FidelityKind, FidelityStatus, LegalAction, LegalActionKind,
        LiveError, LiveResult, RunConfig, RunSeed, SessionId, SessionLifecycle, SessionSnapshot,
    },
    session::SessionStore,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use sts_verify::{
    import_communication_mod_trace, verify_seed_start_communication_mod_trace, StartRunCommand,
    TraceAction, TraceLine, TraceProfile,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRequest {
    pub source_path: PathBuf,
    pub bridge_id: BridgeId,
    pub reset_bridge: bool,
    pub max_actions: Option<usize>,
    pub dry_run: bool,
    pub action_template: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayStatus {
    Validated,
    TemplateValidated,
    ActionLimit,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    Verified,
    ActionTemplate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub status: ReplayStatus,
    pub mode: ReplayMode,
    pub source_path: String,
    pub source_actions: usize,
    pub planned_actions: usize,
    pub replayed_actions: usize,
    pub last_replayed_source_step: Option<u32>,
    pub session_id: Option<SessionId>,
    pub replay_trace_path: Option<String>,
    pub reset_performed: bool,
    pub lifecycle: Option<SessionLifecycle>,
    pub fidelity: Option<FidelityStatus>,
    pub verification_starting_hp: Option<i32>,
    pub captured_profile: Option<TraceProfile>,
}

#[derive(Debug, Clone)]
struct ReplayPlan {
    mode: ReplayMode,
    source_path: String,
    source_actions: usize,
    config: RunConfig,
    start_command: StartRunCommand,
    actions: Vec<TraceAction>,
}

pub fn replay_existing_trace<B, F>(
    store: &mut SessionStore<B, F>,
    request: ReplayRequest,
) -> LiveResult<ReplayReport>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let plan = if request.action_template {
        load_action_template_plan(&request.source_path)?
    } else {
        load_replay_plan(&request.source_path)?
    };
    if request.dry_run {
        return Ok(ReplayReport {
            status: match plan.mode {
                ReplayMode::Verified => ReplayStatus::Validated,
                ReplayMode::ActionTemplate => ReplayStatus::TemplateValidated,
            },
            mode: plan.mode,
            source_path: plan.source_path,
            source_actions: plan.source_actions,
            planned_actions: plan.actions.len(),
            replayed_actions: 0,
            last_replayed_source_step: None,
            session_id: None,
            replay_trace_path: None,
            reset_performed: false,
            lifecycle: None,
            fidelity: None,
            verification_starting_hp: plan.start_command.verification_starting_hp,
            captured_profile: None,
        });
    }

    let reset_performed =
        prepare_bridge_for_replay(store, &request.bridge_id, request.reset_bridge)?;

    let source_profile = plan.config.profile.clone();
    let mut snapshot = match plan.start_command.verification_starting_hp {
        Some(starting_hp) => {
            store.start_verification_run(request.bridge_id, plan.config.clone(), starting_hp)?
        }
        None => store.start_run(request.bridge_id, plan.config.clone())?,
    };
    if plan.mode == ReplayMode::ActionTemplate {
        ensure_captured_profile(&snapshot)?;
    }
    ensure_source_profile(&snapshot, source_profile.as_ref())?;
    ensure_fidelity(&snapshot, None)?;
    let session_id = snapshot.session_id.clone();
    let mut replayed_actions = 0;
    let mut last_replayed_source_step = None;

    for (action_index, source_action) in plan.actions.iter().enumerate() {
        if request
            .max_actions
            .is_some_and(|limit| replayed_actions >= limit)
        {
            break;
        }

        let action_id = matching_live_action(
            &store.actions(&session_id)?,
            &source_action.command,
            &session_id,
            source_action.step,
        )?;
        snapshot = if command_is(&source_action.command, "STATE") {
            store.request_state(&session_id)?
        } else if command_is(&source_action.command, "ABANDON") {
            store.abandon_run(&session_id, "trace_replay")?
        } else {
            store.send_action(&session_id, &action_id)?
        };
        let next_action_will_run = action_index + 1 < plan.actions.len()
            && request
                .max_actions
                .is_none_or(|limit| replayed_actions + 1 < limit);
        ensure_replay_fidelity(&snapshot, source_action, plan.mode, next_action_will_run)?;
        replayed_actions += 1;
        last_replayed_source_step = Some(source_action.step);
    }

    let status = if replayed_actions == plan.actions.len() {
        ReplayStatus::Completed
    } else {
        ReplayStatus::ActionLimit
    };
    Ok(report_from_snapshot(
        status,
        plan,
        replayed_actions,
        last_replayed_source_step,
        reset_performed,
        snapshot,
    ))
}

fn load_replay_plan(path: &Path) -> LiveResult<ReplayPlan> {
    let content = fs::read_to_string(path)?;
    let trace = import_communication_mod_trace(&content)?;
    let report = verify_seed_start_communication_mod_trace(&content)
        .map_err(|error| invalid_source(path, error.to_string()))?;

    if trace
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.boss_unlocks.is_some())
    {
        return Err(invalid_source(
            path,
            "explicit boss-unlock inputs cannot be asserted through the live bridge",
        ));
    }
    if let Some(error) = trace.lines.iter().find_map(|line| match line {
        TraceLine::Error(error) => Some(format!("recorded bridge error at step {}", error.step)),
        TraceLine::CommandObservedTimeout(timeout) => Some(format!(
            "recorded command timeout at step {} for {:?}",
            timeout.step, timeout.command
        )),
        _ => None,
    }) {
        return Err(invalid_source(path, error));
    }
    if let Some(diff) = report.unexpected_diffs.first() {
        return Err(invalid_source(
            path,
            format!(
                "simulator diff at step {} for {:?}: {}",
                diff.action_step,
                diff.command,
                diff.diffs.join("; ")
            ),
        ));
    }
    if let Some(unsupported) = report.unsupported.first() {
        return Err(invalid_source(
            path,
            format!(
                "unsupported transition at step {} for {:?}: {}",
                unsupported.action_step, unsupported.command, unsupported.reason
            ),
        ));
    }
    if report.ignored_tail_actions != 0 {
        return Err(invalid_source(
            path,
            format!(
                "{} action(s) were not verified at the end of the trace",
                report.ignored_tail_actions
            ),
        ));
    }
    let integrity = report
        .action_integrity
        .as_ref()
        .ok_or_else(|| invalid_source(path, "missing action-integrity report"))?;
    if integrity.applicable_actions != integrity.disposed_actions
        || integrity.duplicate_dispositions != 0
        || integrity.unresolved_transient_assertions != 0
        || integrity.rejected_actions != 0
    {
        return Err(invalid_source(
            path,
            format!(
                "incomplete action accounting: applicable={}, disposed={}, duplicates={}, \
                 unresolved_transients={}, rejected={}",
                integrity.applicable_actions,
                integrity.disposed_actions,
                integrity.duplicate_dispositions,
                integrity.unresolved_transient_assertions,
                integrity.rejected_actions
            ),
        ));
    }
    let seed_start = report
        .seed_start
        .as_ref()
        .ok_or_else(|| invalid_source(path, "missing verified START command"))?;
    if seed_start.failed {
        return Err(invalid_source(
            path,
            format!(
                "seed-start verification stopped at {}: {}",
                seed_start.first_boundary.path, seed_start.first_boundary.reason
            ),
        ));
    }

    let all_actions = trace
        .lines
        .iter()
        .filter_map(|line| match line {
            TraceLine::Action(action) => Some(action.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let start_indexes = all_actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| command_is_start(&action.command).then_some(index))
        .collect::<Vec<_>>();
    if start_indexes.len() != 1 {
        return Err(invalid_source(
            path,
            format!(
                "expected exactly one START command, found {}",
                start_indexes.len()
            ),
        ));
    }
    let start_index = start_indexes[0];
    if let Some(action) = all_actions[..start_index]
        .iter()
        .find(|action| !command_is(&action.command, "STATE"))
    {
        return Err(invalid_source(
            path,
            format!(
                "non-observation command {:?} appears before START at step {}",
                action.command, action.step
            ),
        ));
    }
    if all_actions[start_index].step != seed_start.start_command.action_step {
        return Err(invalid_source(
            path,
            "verified START command does not match the replay action stream",
        ));
    }

    let profile = trace
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.run_config.as_ref())
        .and_then(|run_config| run_config.profile.clone());
    let config = replay_run_config(path, &seed_start.start_command, profile)?;
    let actions = all_actions.into_iter().skip(start_index + 1).collect();
    Ok(ReplayPlan {
        mode: ReplayMode::Verified,
        source_path: path.to_string_lossy().into_owned(),
        source_actions: report.total_actions,
        config,
        start_command: seed_start.start_command.clone(),
        actions,
    })
}

fn load_action_template_plan(path: &Path) -> LiveResult<ReplayPlan> {
    let content = fs::read_to_string(path)?;
    let trace = import_communication_mod_trace(&content)?;
    let report = verify_seed_start_communication_mod_trace(&content)
        .map_err(|error| invalid_template_source(path, error.to_string()))?;
    let seed_start = report
        .seed_start
        .as_ref()
        .ok_or_else(|| invalid_template_source(path, "missing START command"))?;

    let all_actions = trace
        .lines
        .iter()
        .filter_map(|line| match line {
            TraceLine::Action(action) => Some(action.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let start_indexes = all_actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| command_is_start(&action.command).then_some(index))
        .collect::<Vec<_>>();
    if start_indexes.len() != 1 {
        return Err(invalid_template_source(
            path,
            format!(
                "expected exactly one START or START_VERIFY command, found {}",
                start_indexes.len()
            ),
        ));
    }
    let start_index = start_indexes[0];
    if let Some(action) = all_actions[..start_index]
        .iter()
        .find(|action| !command_is(&action.command, "STATE"))
    {
        return Err(invalid_template_source(
            path,
            format!(
                "non-observation command {:?} appears before START at step {}",
                action.command, action.step
            ),
        ));
    }
    if all_actions[start_index].step != seed_start.start_command.action_step {
        return Err(invalid_template_source(
            path,
            "parsed START command does not match the action stream",
        ));
    }
    let config = action_template_run_config(path, &seed_start.start_command)?;
    let source_actions = all_actions.len();
    let actions = all_actions.into_iter().skip(start_index + 1).collect();
    Ok(ReplayPlan {
        mode: ReplayMode::ActionTemplate,
        source_path: path.to_string_lossy().into_owned(),
        source_actions,
        config,
        start_command: seed_start.start_command.clone(),
        actions,
    })
}

fn replay_run_config(
    path: &Path,
    start: &StartRunCommand,
    profile: Option<TraceProfile>,
) -> LiveResult<RunConfig> {
    if start.verification_starting_hp.is_some() {
        return Err(invalid_source(
            path,
            "START_VERIFY traces cannot be replayed by the normal live-game launcher",
        ));
    }
    if !start.character.eq_ignore_ascii_case("IRONCLAD") {
        return Err(invalid_source(
            path,
            format!("unsupported character {:?}", start.character),
        ));
    }
    Ok(RunConfig {
        character: Character::Ironclad,
        ascension: start.ascension,
        seed: RunSeed::External(start.external_seed.clone()),
        profile,
    })
}

fn action_template_run_config(path: &Path, start: &StartRunCommand) -> LiveResult<RunConfig> {
    if !start.character.eq_ignore_ascii_case("IRONCLAD") {
        return Err(invalid_template_source(
            path,
            format!("unsupported character {:?}", start.character),
        ));
    }
    Ok(RunConfig {
        character: Character::Ironclad,
        ascension: start.ascension,
        seed: RunSeed::External(start.external_seed.clone()),
        // The purpose of recollection is to capture the authoritative live
        // profile. Never copy or infer it from the broken source trace.
        profile: None,
    })
}

fn matching_live_action(
    actions: &[LegalAction],
    source_command: &str,
    session_id: &SessionId,
    source_step: u32,
) -> LiveResult<ActionId> {
    let matches = actions
        .iter()
        .filter(|action| {
            action
                .command
                .get("command")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|command| commands_match(command, source_command))
        })
        .collect::<Vec<_>>();
    let enabled = matches
        .iter()
        .copied()
        .filter(|action| action.enabled)
        .collect::<Vec<_>>();
    match enabled.as_slice() {
        [action] => Ok(action.id.clone()),
        [] if !matches.is_empty() => {
            let reason = matches
                .iter()
                .find_map(|action| action.disabled_reason.as_deref())
                .unwrap_or("matching live action is disabled");
            Err(replay_blocked(
                session_id,
                source_step,
                source_command,
                reason,
            ))
        }
        [] => {
            let legal = actions
                .iter()
                .filter(|action| action.enabled)
                .filter_map(|action| {
                    action
                        .command
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                })
                .collect::<Vec<_>>();
            Err(replay_blocked(
                session_id,
                source_step,
                source_command,
                &format!("command is not currently legal; live commands: {legal:?}"),
            ))
        }
        _ => Err(replay_blocked(
            session_id,
            source_step,
            source_command,
            "more than one enabled live action has the recorded command",
        )),
    }
}

fn ensure_fidelity(
    snapshot: &SessionSnapshot,
    source_action: Option<&TraceAction>,
) -> LiveResult<()> {
    if snapshot.fidelity.kind == FidelityKind::Ok {
        return Ok(());
    }
    let source = source_action.map_or_else(
        || "after START".to_owned(),
        |action| format!("after source step {} ({:?})", action.step, action.command),
    );
    let detail = if snapshot.fidelity.compact_diff.is_empty() {
        snapshot
            .fidelity
            .message
            .clone()
            .unwrap_or_else(|| format!("fidelity is {:?}", snapshot.fidelity.kind))
    } else {
        snapshot.fidelity.compact_diff.join("; ")
    };
    Err(LiveError::Blocked(format!(
        "replay session {} diverged {source}: {detail}",
        snapshot.session_id.0
    )))
}

fn ensure_replay_fidelity(
    snapshot: &SessionSnapshot,
    source_action: &TraceAction,
    mode: ReplayMode,
    next_action_will_run: bool,
) -> LiveResult<()> {
    if mode == ReplayMode::ActionTemplate
        && next_action_will_run
        && is_deferred_deck_boundary(&snapshot.fidelity)
    {
        return Ok(());
    }
    ensure_fidelity(snapshot, Some(source_action))
}

fn is_deferred_deck_boundary(fidelity: &FidelityStatus) -> bool {
    fidelity.kind == FidelityKind::Unknown
        && fidelity.message.as_deref().is_some_and(|message| {
            message.starts_with("seed-start replay reached boundary unreconciled_deck_frame:")
        })
        && fidelity.compact_diff.as_slice()
            == ["deferred deck mutation did not reach a captured settled frame"]
}

fn ensure_source_profile(
    snapshot: &SessionSnapshot,
    expected: Option<&TraceProfile>,
) -> LiveResult<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = snapshot
        .run_config
        .as_ref()
        .and_then(|config| config.profile.as_ref());
    if actual == Some(expected) {
        return Ok(());
    }
    Err(LiveError::Blocked(format!(
        "replay session {} stopped after START because the live profile {:?} does not match the \
         source profile {:?}",
        snapshot.session_id.0, actual, expected
    )))
}

fn ensure_captured_profile(snapshot: &SessionSnapshot) -> LiveResult<()> {
    if snapshot
        .run_config
        .as_ref()
        .and_then(|config| config.profile.as_ref())
        .is_some()
    {
        return Ok(());
    }
    Err(LiveError::Blocked(format!(
        "replay session {} stopped after START_VERIFY because CommunicationMod did not provide \
         an authoritative profile",
        snapshot.session_id.0
    )))
}

fn report_from_snapshot(
    status: ReplayStatus,
    plan: ReplayPlan,
    replayed_actions: usize,
    last_replayed_source_step: Option<u32>,
    reset_performed: bool,
    snapshot: SessionSnapshot,
) -> ReplayReport {
    ReplayReport {
        status,
        mode: plan.mode,
        source_path: plan.source_path,
        source_actions: plan.source_actions,
        planned_actions: plan.actions.len(),
        replayed_actions,
        last_replayed_source_step,
        session_id: Some(snapshot.session_id),
        replay_trace_path: Some(snapshot.trace_path),
        reset_performed,
        lifecycle: Some(snapshot.lifecycle),
        fidelity: Some(snapshot.fidelity),
        verification_starting_hp: plan.start_command.verification_starting_hp,
        captured_profile: snapshot
            .run_config
            .as_ref()
            .and_then(|config| config.profile.clone()),
    }
}

fn bridge_ready_for_start(state: &crate::model::LiveState) -> bool {
    state
        .legal_actions
        .iter()
        .any(|action| action.enabled && action.kind == crate::model::LegalActionKind::StartRun)
        || command_available(state, "start")
}

fn prepare_bridge_for_replay<B, F>(
    store: &mut SessionStore<B, F>,
    bridge_id: &BridgeId,
    reset_bridge: bool,
) -> LiveResult<bool>
where
    B: BridgeManager,
    F: FidelityChecker,
{
    let initial = store.request_bridge_state(bridge_id)?;
    if bridge_ready_for_start(&initial) {
        return Ok(false);
    }
    if !reset_bridge {
        return Err(LiveError::Blocked(format!(
            "bridge {} is in phase {:?}; replay requires a start-ready state or explicit \
             --reset-bridge",
            bridge_id.0, initial.phase
        )));
    }

    let mut reset_performed = false;
    let mut state = initial;
    for _ in 0..40 {
        if bridge_ready_for_start(&state) {
            return Ok(reset_performed);
        }
        if command_available(&state, "proceed") {
            state = store.send_bridge_command(
                bridge_id,
                &state,
                "PROCEED",
                LegalActionKind::Confirm,
                "Proceed",
            )?;
            reset_performed = true;
        } else if command_available(&state, "abandon") {
            state = store.abandon_bridge_run(bridge_id)?;
            reset_performed = true;
        } else {
            thread::sleep(Duration::from_millis(250));
            state = store.request_bridge_state(bridge_id)?;
        }
    }
    Err(LiveError::Bridge(format!(
        "bridge {} did not return to a start-ready state after reset",
        bridge_id.0
    )))
}

fn command_available(state: &crate::model::LiveState, expected: &str) -> bool {
    commands_contain(state.raw.pointer("/summary/available_commands"), expected)
        || commands_contain(
            state
                .raw
                .pointer("/current_state/message/available_commands"),
            expected,
        )
}

fn commands_contain(commands: Option<&Value>, expected: &str) -> bool {
    match commands {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .any(|command| command.eq_ignore_ascii_case(expected)),
        Some(Value::String(commands)) => commands
            .split_whitespace()
            .any(|command| command.eq_ignore_ascii_case(expected)),
        _ => false,
    }
}

fn command_is_start(command: &str) -> bool {
    command.split_whitespace().next().is_some_and(|head| {
        head.eq_ignore_ascii_case("START") || head.eq_ignore_ascii_case("START_VERIFY")
    })
}

fn command_is(command: &str, expected: &str) -> bool {
    normalize_command(command) == normalize_command(expected)
}

fn commands_match(left: &str, right: &str) -> bool {
    normalize_command(left) == normalize_command(right)
}

fn normalize_command(command: &str) -> String {
    command
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn invalid_source(path: &Path, reason: impl Into<String>) -> LiveError {
    LiveError::InvalidAction(format!(
        "replay source {} is not safe to execute: {}",
        path.display(),
        reason.into()
    ))
}

fn invalid_template_source(path: &Path, reason: impl Into<String>) -> LiveError {
    LiveError::InvalidAction(format!(
        "action-template source {} cannot be recollected: {}",
        path.display(),
        reason.into()
    ))
}

fn replay_blocked(
    session_id: &SessionId,
    source_step: u32,
    source_command: &str,
    reason: &str,
) -> LiveError {
    LiveError::Blocked(format!(
        "replay session {} stopped before source step {} ({source_command:?}): {reason}",
        session_id.0, source_step
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LegalActionKind, LivePhase, LiveState};
    use crate::{FakeBridgeManager, TraceFidelityChecker};
    use serde_json::json;

    fn fixture_path() -> PathBuf {
        sts_verify::simulator_root()
            .join("verification/corpus/permanent_traces/trace-2026-07-03T20-12-12-408Z.jsonl")
    }

    #[test]
    fn verified_partial_trace_builds_a_replay_plan() {
        let plan = load_replay_plan(&fixture_path()).unwrap();

        assert_eq!(plan.source_actions, 3);
        assert_eq!(plan.actions.len(), 2);
        assert_eq!(plan.actions[0].command, "CHOOSE 0");
        assert_eq!(plan.config.ascension, 0);
        assert_eq!(
            plan.config.seed,
            RunSeed::External("4CAD260DLFGRM".to_owned())
        );
    }

    #[test]
    fn dry_run_validates_without_touching_the_bridge() {
        let root =
            std::env::temp_dir().join(format!("sts-live-replay-dry-run-{}", std::process::id()));
        let mut store = SessionStore::new(
            FakeBridgeManager::with_default_bridge(),
            TraceFidelityChecker,
            &root,
        );

        let report = replay_existing_trace(
            &mut store,
            ReplayRequest {
                source_path: fixture_path(),
                bridge_id: BridgeId("missing-bridge-is-not-read".to_owned()),
                reset_bridge: false,
                max_actions: None,
                dry_run: true,
                action_template: false,
            },
        )
        .unwrap();

        assert_eq!(report.status, ReplayStatus::Validated);
        assert_eq!(report.mode, ReplayMode::Verified);
        assert_eq!(report.planned_actions, 2);
        assert!(report.session_id.is_none());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn quarantined_start_verify_trace_builds_an_action_template_plan() {
        let source = sts_verify::simulator_root().join(
            "verification/corpus/quarantined_traces/note_profile_missing/\
             random-fidelity-573e04950e8c3758.jsonl",
        );

        let plan = load_action_template_plan(&source).unwrap();

        assert_eq!(plan.mode, ReplayMode::ActionTemplate);
        assert_eq!(plan.start_command.external_seed, "FIDL00055");
        assert_eq!(plan.start_command.verification_starting_hp, Some(10_000));
        assert_eq!(plan.config.profile, None);
        assert_eq!(plan.actions.first().unwrap().command, "STATE");
        assert!(plan
            .actions
            .iter()
            .any(|action| action.command == "PLAY 1 0"));
    }

    #[test]
    fn normal_replay_still_rejects_quarantined_start_verify_trace() {
        let source = sts_verify::simulator_root().join(
            "verification/corpus/quarantined_traces/note_profile_missing/\
             random-fidelity-573e04950e8c3758.jsonl",
        );

        let error = load_replay_plan(&source).unwrap_err();

        assert!(error.to_string().contains("not safe to execute"));
    }

    #[test]
    fn command_matching_ignores_case_and_whitespace() {
        let action = LegalAction {
            id: ActionId("choose-0".to_owned()),
            kind: LegalActionKind::ChooseNeow,
            label: "Talk".to_owned(),
            enabled: true,
            command: json!({"command": "CHOOSE 0"}),
            disabled_reason: None,
        };

        let matched = matching_live_action(
            &[action],
            "  choose   0 ",
            &SessionId("session-1".to_owned()),
            2,
        )
        .unwrap();

        assert_eq!(matched, ActionId("choose-0".to_owned()));
    }

    #[test]
    fn unavailable_recorded_command_stops_before_sending() {
        let state_action = LegalAction {
            id: ActionId("request-state".to_owned()),
            kind: LegalActionKind::RequestState,
            label: "Request state".to_owned(),
            enabled: true,
            command: json!({"command": "STATE"}),
            disabled_reason: None,
        };

        let error = matching_live_action(
            &[state_action],
            "CHOOSE 3",
            &SessionId("session-9".to_owned()),
            17,
        )
        .unwrap_err();

        assert!(matches!(error, LiveError::Blocked(_)));
        assert!(error.to_string().contains("before source step 17"));
        assert!(error.to_string().contains("CHOOSE 3"));
    }

    #[test]
    fn action_template_can_cross_known_deferred_deck_boundary_with_a_next_action() {
        let fidelity = FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: vec![
                "deferred deck mutation did not reach a captured settled frame".to_owned(),
            ],
            message: Some(
                "seed-start replay reached boundary unreconciled_deck_frame: deferred deck \
                 mutation did not reach a captured settled frame"
                    .to_owned(),
            ),
        };

        assert!(is_deferred_deck_boundary(&fidelity));
    }

    #[test]
    fn action_template_does_not_cross_other_unknown_fidelity_boundaries() {
        let fidelity = FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: vec!["unsupported event".to_owned()],
            message: Some(
                "seed-start replay reached boundary unsupported_event: unsupported event"
                    .to_owned(),
            ),
        };

        assert!(!is_deferred_deck_boundary(&fidelity));
    }

    #[test]
    fn active_bridge_requires_explicit_reset() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({}),
        };

        assert!(!bridge_ready_for_start(&state));
    }

    #[test]
    fn protocol_start_command_is_start_ready_even_with_unknown_phase() {
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Unknown,
            legal_actions: Vec::new(),
            raw: json!({
                "current_state": {
                    "message": {
                        "available_commands": ["start", "state"],
                        "in_game": false,
                        "ready_for_command": true
                    }
                }
            }),
        };

        assert!(bridge_ready_for_start(&state));
    }
}
