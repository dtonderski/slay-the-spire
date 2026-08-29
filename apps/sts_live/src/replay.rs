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
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use sts_verify::{
    parse_trace_jsonl_line, verify_communication_mod_trace_reader, StartRunCommand, TraceAction,
    TraceLine, TraceMetadata, TraceProfile,
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

struct ReplaySourceRecords {
    metadata: Option<TraceMetadata>,
    actions: Vec<TraceAction>,
    recorded_failure: Option<String>,
}

fn extract_replay_source(
    source: &mut impl BufRead,
    path: &Path,
) -> LiveResult<ReplaySourceRecords> {
    let mut metadata = None;
    let mut actions = Vec::new();
    let mut recorded_failure = None;
    let mut encoded = String::new();
    let mut line_index = 0usize;
    loop {
        encoded.clear();
        if source.read_line(&mut encoded)? == 0 {
            break;
        }
        line_index += 1;
        let Some(line) = parse_trace_jsonl_line(&encoded).map_err(|error| {
            LiveError::InvalidAction(format!(
                "replay source changed or became invalid at {}:{line_index}: {error}",
                path.display()
            ))
        })?
        else {
            continue;
        };
        match line {
            TraceLine::Metadata(value) => metadata = Some(value),
            TraceLine::Action(action) => actions.push(action),
            TraceLine::Error(error) => {
                recorded_failure
                    .get_or_insert_with(|| format!("recorded bridge error at step {}", error.step));
            }
            TraceLine::CommandObservedTimeout(timeout) => {
                recorded_failure.get_or_insert_with(|| {
                    format!(
                        "recorded command timeout at step {} for {:?}",
                        timeout.step, timeout.command
                    )
                });
            }
            TraceLine::State(_)
            | TraceLine::ExternalRng(_)
            | TraceLine::CommandAccept(_)
            | TraceLine::Response(_)
            | TraceLine::SlayTheData(_)
            | TraceLine::Automation(_) => {}
        }
    }
    Ok(ReplaySourceRecords {
        metadata,
        actions,
        recorded_failure,
    })
}

fn load_replay_plan(path: &Path) -> LiveResult<ReplayPlan> {
    let mut source = BufReader::new(File::open(path)?);
    let initial_metadata = source.get_ref().metadata()?;
    let report = verify_communication_mod_trace_reader(&mut source)
        .map_err(|error| invalid_source(path, error.to_string()))?;
    source.seek(SeekFrom::Start(0))?;
    let records = extract_replay_source(&mut source, path)?;
    let final_metadata = source.get_ref().metadata()?;
    reject_changed_trace(path, &initial_metadata, &final_metadata)?;

    if records
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.boss_unlocks.is_some())
    {
        return Err(invalid_source(
            path,
            "explicit boss-unlock inputs cannot be asserted through the live bridge",
        ));
    }
    if let Some(error) = records.recorded_failure {
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
    let integrity = report
        .action_integrity
        .as_ref()
        .ok_or_else(|| invalid_source(path, "missing action-integrity report"))?;
    if integrity.applicable_actions != integrity.disposed_actions
        || integrity.duplicate_dispositions != 0
        || integrity.rejected_actions != 0
    {
        return Err(invalid_source(
            path,
            format!(
                "incomplete action accounting: applicable={}, disposed={}, duplicates={}, rejected={}",
                integrity.applicable_actions,
                integrity.disposed_actions,
                integrity.duplicate_dispositions,
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

    let all_actions = records.actions;
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

    let profile = records
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
    let mut source = BufReader::new(File::open(path)?);
    let initial_metadata = source.get_ref().metadata()?;
    let report = verify_communication_mod_trace_reader(&mut source)
        .map_err(|error| invalid_template_source(path, error.to_string()))?;
    source.seek(SeekFrom::Start(0))?;
    let records = extract_replay_source(&mut source, path)?;
    let final_metadata = source.get_ref().metadata()?;
    reject_changed_trace(path, &initial_metadata, &final_metadata)?;
    let seed_start = report
        .seed_start
        .as_ref()
        .ok_or_else(|| invalid_template_source(path, "missing START command"))?;

    let all_actions = records.actions;
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

fn reject_changed_trace(
    path: &Path,
    initial: &fs::Metadata,
    final_metadata: &fs::Metadata,
) -> LiveResult<()> {
    if initial.len() != final_metadata.len()
        || initial.modified().ok() != final_metadata.modified().ok()
    {
        return Err(LiveError::InvalidAction(format!(
            "trace changed while it was being verified: {}",
            path.display()
        )));
    }
    Ok(())
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
    use serde_json::json;

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
