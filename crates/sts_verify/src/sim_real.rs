//! CommunicationMod trace replay against the simulator for supported fields.

use crate::canonical_json::{canonical_json_bytes, sha256_hex};
use crate::{
    canonical_value_diff, parse_trace_jsonl_line, TraceAction, TraceLine, TraceProfile, TraceState,
    VerificationIntegrity,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::{BufRead, Cursor},
};
use sts_core::card::CardType;
use sts_core::combat::{ExhaustSelectPurpose, HandSelectPurpose};
use sts_core::content::cards::{card_instance_is_upgradeable, card_type_and_rarity};
use sts_core::content::encounters::BossUnlockState;
use sts_core::content::monsters::{target_move_byte, target_move_byte_for_monster};
use sts_core::potion::Potion;
use sts_core::{
    affordable_shop_picks, apply_run_decision_action, legal_run_decision_actions,
    try_sts_seed_string_to_long, CardGridScreen, CardId, CardInstance, CombatAction,
    CombatDecisionState, CombatPhase, CombatState, ContentId, Event, EventScreen, GridPurpose,
    MapAction, MonsterId, MonsterIntent, MonsterState, Relic, RelicKey, RestAction,
    RewardContinuation, RewardScreen, RoomKind, RunAction, RunDecisionAction, RunPhase, RunState,
    ShopPick,
};
use sts_core::{Snapshot, SNAPSHOT_SCHEMA_VERSION};

mod replay;

use replay::{
    finish_streaming_seed_start_replay, verify_seed_start_transition, SeedStartReplayInputs,
    StreamingSeedStartReplay,
};

fn legal_map_decisions(run: &RunState) -> sts_core::SimResult<Vec<MapAction>> {
    Ok(legal_run_decision_actions(run)?
        .into_iter()
        .filter_map(|action| match action {
            RunDecisionAction::Map(action) => Some(action),
            _ => None,
        })
        .collect())
}

fn legal_rest_decisions(run: &RunState) -> sts_core::SimResult<Vec<RestAction>> {
    Ok(legal_run_decision_actions(run)?
        .into_iter()
        .filter_map(|action| match action {
            RunDecisionAction::Rest(action) => Some(action),
            _ => None,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimRealReport {
    pub total_actions: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_dispositions: Vec<ActionDisposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_integrity: Option<VerificationIntegrity>,
    pub verified: Vec<VerifiedTransition>,
    pub unsupported: Vec<UnsupportedTransition>,
    pub unexpected_diffs: Vec<UnexpectedDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_start: Option<SeedStartReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionDisposition {
    pub action_ordinal: usize,
    pub action_step: u32,
    pub command: String,
    pub disposition: ActionDispositionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionDispositionKind {
    Verified,
    Unsupported,
    UnexpectedDiff,
    TargetRejected,
    Boundary,
    BeyondBoundary,
    Unclassified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedTransition {
    pub action_step: u32,
    pub command: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedTransition {
    pub action_step: u32,
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnexpectedDiff {
    pub action_step: u32,
    pub command: String,
    pub label: String,
    pub diffs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartReport {
    pub start_command: StartRunCommand,
    pub failed: bool,
    pub first_boundary: SeedStartBoundary,
    #[serde(skip)]
    pub sim_run_state: Option<RunState>,
}

/// A lightweight checkpoint produced by an explicit trace replay.
///
/// Observation polls and target-only UI confirmations remain in the timeline,
/// but their state hash always comes from the authoritative simulator state.
/// Full snapshots are retained only for the selected checkpoint and replay
/// endpoint so long traces do not duplicate their entire run state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCheckpoint {
    pub action_step: u32,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCheckpointState {
    pub action_step: u32,
    pub command: String,
    pub snapshot: Snapshot<RunState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayResult {
    pub report: SimRealReport,
    pub checkpoints: Vec<ReplayCheckpoint>,
    pub final_snapshot: Option<Snapshot<RunState>>,
    pub selected_checkpoint: Option<ReplayCheckpointState>,
}

pub const REPLAY_ARTIFACT_SCHEMA: u32 = 1;

#[derive(Default)]
pub(crate) struct ReplayCapture {
    pub(crate) requested_step: Option<u32>,
    pub(crate) checkpoints: Vec<ReplayCheckpoint>,
    pub(crate) selected_checkpoint: Option<ReplayCheckpointState>,
    pub(crate) capture_roots: bool,
    pub(crate) roots: Vec<ReplayCombatRoot>,
    pub(crate) previous_state_was_actionable: bool,
    pub(crate) next_combat_ordinal: u32,
    pub(crate) capture_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCombatRoot {
    pub combat_ordinal: u32,
    pub action_step: u32,
    pub snapshot: Snapshot<RunState>,
}

#[derive(Debug)]
pub struct TraceRootCapture {
    pub report: SimRealReport,
    pub roots: Vec<ReplayCombatRoot>,
    pub capture_error: Option<String>,
}

pub const ROOT_ENCODING: &str = "snapshot_canonical_json_v1";
pub const ACTIONABLE_PREDICATE: &str =
    "combat_waiting_for_player_with_nonempty_public_legal_actions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRunCommand {
    pub action_step: u32,
    pub character: String,
    pub ascension: u8,
    pub external_seed: String,
    pub numeric_seed: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_starting_hp: Option<i32>,
}

impl StartRunCommand {
    fn matches_command(&self, command: &str) -> bool {
        let expected = match self.verification_starting_hp {
            Some(hp) => format!(
                "START_VERIFY {} {} {} {hp}",
                self.character, self.ascension, self.external_seed
            ),
            None => format!(
                "START {} {} {}",
                self.character, self.ascension, self.external_seed
            ),
        };
        command.eq_ignore_ascii_case(&expected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartBoundary {
    pub path: String,
    pub category: String,
    pub reason: String,
}

#[derive(Debug)]
pub enum SimRealError {
    Trace(serde_json::Error),
    Io(std::io::Error),
    MissingStartCommand,
    MalformedStartCommand(String),
    MalformedChooseCommand { step: u32, command: String },
    InvalidProfileInput(String),
    UnsupportedSchema { boundary_schema: Option<u32> },
    InvalidBoundaryContract { step: u32, reason: String },
    OrphanExternalRng { step: u32 },
}

impl std::fmt::Display for SimRealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::MissingStartCommand => write!(f, "trace does not contain START command"),
            Self::MalformedStartCommand(command) => {
                write!(f, "malformed START command: {command}")
            }
            Self::MalformedChooseCommand { step, command } => write!(
                f,
                "malformed CHOOSE command at step {step}: {command}; expected exactly `CHOOSE <non-negative index>`"
            ),
            Self::InvalidProfileInput(reason) => write!(f, "invalid pre-run profile input: {reason}"),
            Self::UnsupportedSchema { boundary_schema } => match boundary_schema {
                Some(schema) => write!(f, "unsupported CommunicationMod boundary schema {schema}; supported schemas are 1, 2, 3, 4, 5, 6, and 7"),
                None => write!(f, "unsupported CommunicationMod boundary schema: explicit metadata boundary_schema of 1, 2, 3, 4, 5, 6, or 7 is required"),
            },
            Self::InvalidBoundaryContract { step, reason } => {
                write!(f, "invalid boundary contract at step {step}: {reason}")
            }
            Self::OrphanExternalRng { step } => {
                write!(f, "external RNG metadata at step {step} has no matching pending action")
            }
        }
    }
}

impl std::error::Error for SimRealError {}

impl From<serde_json::Error> for SimRealError {
    fn from(value: serde_json::Error) -> Self {
        Self::Trace(value)
    }
}

impl From<std::io::Error> for SimRealError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationReadMode {
    Strict,
    DiagnosticEarlyExit,
}

pub fn verify_communication_mod_trace(content: &str) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_communication_mod_trace(content)
}

pub fn verify_seed_start_communication_mod_trace(
    content: &str,
) -> Result<SimRealReport, SimRealError> {
    verify_communication_mod_trace_reader(Cursor::new(content.as_bytes()))
}

pub fn verify_communication_mod_trace_reader<R: BufRead>(
    reader: R,
) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_reader(reader, VerificationReadMode::Strict, None)
}

pub fn verify_communication_mod_trace_diagnostic_reader<R: BufRead>(
    reader: R,
) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_reader(reader, VerificationReadMode::DiagnosticEarlyExit, None)
}

/// Replays a CommunicationMod trace and returns the authoritative simulator
/// endpoint plus lightweight state checkpoints.
///
/// `requested_step` selects the latest checkpoint at or before that trace
/// action. The observed trace state is used only for verification; it never
/// supplies simulator state.
pub fn replay_communication_mod_trace(
    content: &str,
    requested_step: Option<u32>,
) -> Result<ReplayResult, SimRealError> {
    replay_communication_mod_trace_reader(Cursor::new(content.as_bytes()), requested_step)
}

pub fn replay_communication_mod_trace_reader<R: BufRead>(
    reader: R,
    requested_step: Option<u32>,
) -> Result<ReplayResult, SimRealError> {
    let mut capture = ReplayCapture {
        requested_step,
        ..ReplayCapture::default()
    };
    let report =
        verify_seed_start_reader(reader, VerificationReadMode::Strict, Some(&mut capture))?;
    let final_snapshot = report
        .seed_start
        .as_ref()
        .and_then(|seed_start| seed_start.sim_run_state.as_ref())
        .map(|state| Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: state.clone(),
        });
    Ok(ReplayResult {
        report,
        checkpoints: capture.checkpoints,
        final_snapshot,
        selected_checkpoint: capture.selected_checkpoint,
    })
}

/// Replays one CommunicationMod trace and captures the first actionable
/// authoritative simulator state of every distinct combat.
///
/// Observed trace state remains comparison evidence only. Candidates are kept
/// in memory here; callers must publish them only after a complete-pass
/// assessment.
pub fn extract_communication_mod_trace_reader<R: BufRead>(
    reader: R,
) -> Result<TraceRootCapture, SimRealError> {
    let mut capture = ReplayCapture {
        capture_roots: true,
        ..ReplayCapture::default()
    };
    let report =
        verify_seed_start_reader(reader, VerificationReadMode::Strict, Some(&mut capture))?;
    Ok(TraceRootCapture {
        report,
        roots: capture.roots,
        capture_error: capture.capture_error,
    })
}

pub(crate) fn is_actionable_combat_state(run: &RunState) -> Result<bool, String> {
    if run.phase != RunPhase::Combat {
        return Ok(false);
    }
    let Some(combat) = run.combat.as_ref() else {
        return Ok(false);
    };
    if combat.phase != CombatPhase::WaitingForPlayer {
        return Ok(false);
    }
    let actions = legal_run_decision_actions(run).map_err(|error| error.to_string())?;
    if actions.is_empty() {
        return Ok(false);
    }
    Ok(!actions
        .iter()
        .all(|action| matches!(action, RunDecisionAction::Run(RunAction::Proceed))))
}

pub(crate) fn encode_root_snapshot(snapshot: &Snapshot<RunState>) -> Result<Vec<u8>, String> {
    if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(format!(
            "root snapshot schema {} is not {SNAPSHOT_SCHEMA_VERSION}",
            snapshot.schema_version
        ));
    }
    let value = serde_json::to_value(snapshot).map_err(|error| error.to_string())?;
    Ok(canonical_json_bytes(&value))
}

pub(crate) fn validate_encoded_root_snapshot(
    snapshot: &Snapshot<RunState>,
) -> Result<Vec<u8>, String> {
    snapshot
        .state
        .validate()
        .map_err(|error| error.to_string())?;
    if !is_actionable_combat_state(&snapshot.state)? {
        return Err("root snapshot is not an actionable combat decision".to_owned());
    }
    let bytes = encode_root_snapshot(snapshot)?;
    let json = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
    let restored = sts_core::restore_run_snapshot_json(json).map_err(|error| error.to_string())?;
    if restored.schema_version != SNAPSHOT_SCHEMA_VERSION {
        return Err(format!(
            "restored root snapshot schema {} is not {SNAPSHOT_SCHEMA_VERSION}",
            restored.schema_version
        ));
    }
    restored
        .state
        .validate()
        .map_err(|error| error.to_string())?;
    if restored.state != snapshot.state {
        return Err("restored root snapshot does not equal the captured state".to_owned());
    }
    if !is_actionable_combat_state(&restored.state)? {
        return Err("restored root snapshot is not an actionable combat decision".to_owned());
    }
    let reemitted = encode_root_snapshot(&restored)?;
    if reemitted != bytes {
        return Err("root snapshot re-encode is not byte-identical".to_owned());
    }
    Ok(bytes)
}

pub(crate) fn capture_actionable_root(
    capture: &mut ReplayCapture,
    action: &TraceAction,
    state: &RunState,
) {
    if !capture.capture_roots || capture.capture_error.is_some() {
        return;
    }
    let actionable = match is_actionable_combat_state(state) {
        Ok(actionable) => actionable,
        Err(error) => {
            capture.capture_error = Some(error);
            return;
        }
    };
    if !actionable {
        capture.previous_state_was_actionable = false;
        return;
    }
    if capture.previous_state_was_actionable {
        return;
    }
    let snapshot = Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        state: state.clone(),
    };
    if let Err(error) = validate_encoded_root_snapshot(&snapshot) {
        capture.capture_error = Some(error);
        return;
    }
    let Some(next_ordinal) = capture.next_combat_ordinal.checked_add(1) else {
        capture.capture_error = Some("combat root ordinal overflow".to_owned());
        return;
    };
    capture.next_combat_ordinal = next_ordinal;
    capture.roots.push(ReplayCombatRoot {
        combat_ordinal: capture.next_combat_ordinal,
        action_step: action.step,
        snapshot,
    });
    capture.previous_state_was_actionable = true;
}

pub(crate) fn root_id_for_bytes(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

struct PendingStreamingAction {
    action: TraceAction,
    action_ordinal: usize,
    external_rng: Vec<sts_core::ExternalRngInput>,
}

fn streaming_disposition(
    pending: &PendingStreamingAction,
    disposition: ActionDispositionKind,
    detail: Option<String>,
) -> ActionDisposition {
    ActionDisposition {
        action_ordinal: pending.action_ordinal,
        action_step: pending.action.step,
        command: pending.action.command.clone(),
        disposition,
        detail,
    }
}

fn trace_state_is_terminal(state: &TraceState) -> bool {
    state.message.get("in_game").and_then(Value::as_bool) == Some(false)
        || screen_type(&state.message) == Some("GAME_OVER")
}

fn verify_seed_start_reader<R: BufRead>(
    reader: R,
    mode: VerificationReadMode,
    replay_capture: Option<&mut ReplayCapture>,
) -> Result<SimRealReport, SimRealError> {
    let mut report = SimRealReport {
        total_actions: 0,
        action_dispositions: Vec::new(),
        action_integrity: None,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };
    let mut replay_capture = replay_capture;
    let mut replay = StreamingSeedStartReplay::default();
    let mut last_observed_playtime_seconds = None;
    let mut metadata_seen = false;
    let mut metadata_boundary_schema = None;
    let mut profile = None;
    let mut boss_unlocks = BossUnlockState::default();
    let mut start = None;
    let mut saw_start_completion = false;
    let mut pending: Option<PendingStreamingAction> = None;
    let mut last_action_step: Option<u32> = None;
    let mut terminal_state_observed = false;
    let mut rejected_actions = 0usize;
    let mut first_boundary = None;
    let mut last_command_execution_seq = None;
    let mut last_command_settlement_seq = None;
    let mut seen_command_ids = BTreeSet::new();
    let mut diagnostic_stopped = false;
    let mut record_count = 0usize;

    for raw_line in reader.lines() {
        let raw_line = raw_line?;
        let Some(line) = parse_trace_jsonl_line(&raw_line)? else {
            continue;
        };
        record_count += 1;

        if let TraceLine::Metadata(metadata) = line {
            if record_count != 1 || metadata_seen {
                return Err(SimRealError::InvalidBoundaryContract {
                    step: 0,
                    reason: "schema-v1 requires exactly one leading metadata record".to_owned(),
                });
            }
            metadata_seen = true;
            if !matches!(metadata.boundary_schema, Some(1..=7)) {
                return Err(SimRealError::UnsupportedSchema {
                    boundary_schema: metadata.boundary_schema,
                });
            }
            metadata_boundary_schema = metadata.boundary_schema;
            if metadata.schema != 1 || metadata.source != "communication_mod" {
                return Err(SimRealError::InvalidBoundaryContract {
                    step: 0,
                    reason: "schema-v1 metadata must declare schema=1 and source=communication_mod"
                        .to_owned(),
                });
            }
            let explicit_profile = metadata
                .run_config
                .as_ref()
                .and_then(|run_config| run_config.profile.clone())
                .ok_or_else(|| {
                    SimRealError::InvalidProfileInput(
                        "schema-v1 metadata.run_config.profile is required".to_owned(),
                    )
                })?;
            validate_trace_profile(&explicit_profile)?;
            profile = Some(explicit_profile);
            boss_unlocks = metadata.boss_unlocks.unwrap_or_default();
            continue;
        }

        if !metadata_seen {
            return Err(SimRealError::UnsupportedSchema {
                boundary_schema: None,
            });
        }

        match line {
            TraceLine::Action(action) => {
                if metadata_boundary_schema.is_some_and(|schema| schema >= 7) {
                    let command_id = action_command_id(&action).ok_or_else(|| {
                        SimRealError::InvalidBoundaryContract {
                            step: action.step,
                            reason: "schema-7 action requires command_meta.command_id".to_owned(),
                        }
                    })?;
                    if !seen_command_ids.insert(command_id.to_owned()) {
                        return Err(SimRealError::InvalidBoundaryContract {
                            step: action.step,
                            reason: format!("duplicate schema-7 command_id {command_id:?}"),
                        });
                    }
                }
                if pending.is_some() {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: action.step,
                        reason: "action has no immediate completing state".to_owned(),
                    });
                }
                if last_action_step.is_none() && action.step != 1 {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: action.step,
                        reason: "first action step must be integer 1".to_owned(),
                    });
                }
                if let Some(previous_step) = last_action_step {
                    let expected = previous_step.checked_add(1).ok_or_else(|| {
                        SimRealError::InvalidBoundaryContract {
                            step: action.step,
                            reason: "action step overflow".to_owned(),
                        }
                    })?;
                    if action.step != expected {
                        return Err(SimRealError::InvalidBoundaryContract {
                            step: action.step,
                            reason: format!(
                                "action step {} is not contiguous after {}",
                                action.step, previous_step
                            ),
                        });
                    }
                }
                if command_head_eq(&action.command, "CHOOSE")
                    && command_choose_index(&action.command).is_none()
                {
                    return Err(SimRealError::MalformedChooseCommand {
                        step: action.step,
                        command: action.command,
                    });
                }
                if last_action_step.is_none() && parse_start_command(&action).is_none() {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: action.step,
                        reason: "first action is not START and has no pre-state".to_owned(),
                    });
                }
                if start.is_none() {
                    if let Some(parsed) = parse_start_command(&action) {
                        start = Some(parsed?);
                    }
                }
                last_action_step = Some(action.step);
                let action_ordinal = report.total_actions;
                report.total_actions += 1;
                report.action_dispositions.push(ActionDisposition {
                    action_ordinal,
                    action_step: action.step,
                    command: action.command.clone(),
                    disposition: ActionDispositionKind::Unclassified,
                    detail: None,
                });
                pending = Some(PendingStreamingAction {
                    action,
                    action_ordinal,
                    external_rng: Vec::new(),
                });
            }
            TraceLine::ExternalRng(capture) => {
                let Some(pending_action) = pending.as_mut() else {
                    return Err(SimRealError::OrphanExternalRng { step: capture.step });
                };
                if pending_action.action.step != capture.step {
                    return Err(SimRealError::OrphanExternalRng { step: capture.step });
                }
                pending_action.external_rng.extend(capture.draws);
            }
            TraceLine::State(state) => {
                let kind = validate_boundary_state(
                    &state,
                    metadata_boundary_schema.expect("validated leading metadata schema"),
                )?;
                let Some(pending_action) = pending.take() else {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: state.step,
                        reason: "state has no immediately preceding action".to_owned(),
                    });
                };
                if state.step != pending_action.action.step {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: state.step,
                        reason: format!(
                            "state step {} does not match pending action step {}",
                            state.step, pending_action.action.step
                        ),
                    });
                }
                let state_command = command_head_eq(&pending_action.action.command, "STATE");
                if metadata_boundary_schema.is_some_and(|schema| schema >= 7) {
                    validate_schema7_completion(
                        &pending_action.action,
                        &state.message,
                        kind,
                        state_command,
                        last_command_execution_seq,
                        last_command_settlement_seq,
                    )?;
                    last_command_execution_seq = state
                        .message
                        .get("command_execution_seq")
                        .and_then(Value::as_u64);
                    last_command_settlement_seq = state
                        .message
                        .get("command_settlement_seq")
                        .and_then(Value::as_u64);
                } else if metadata_boundary_schema.is_some_and(|schema| schema >= 5) {
                    let execution_seq = state
                        .message
                        .get("command_execution_seq")
                        .and_then(Value::as_u64)
                        .expect("schema-5 sequence validated");
                    let source_execution_seq =
                        action_source_command_execution_seq(&pending_action.action);
                    if !state_command && source_execution_seq.is_none() {
                        return Err(SimRealError::InvalidBoundaryContract {
                            step: state.step,
                            reason: format!(
                                "schema-5+ gameplay action {} requires command_meta.source_command_execution_seq",
                                pending_action.action.command
                            ),
                        });
                    }
                    if let (Some(source), Some(previous)) =
                        (source_execution_seq, last_command_execution_seq)
                    {
                        if source != previous {
                            return Err(SimRealError::InvalidBoundaryContract {
                                step: state.step,
                                reason: format!(
                                    "source_command_execution_seq {source} does not match preceding completion sequence {previous}"
                                ),
                            });
                        }
                    }
                    let fence_advanced = if state_command {
                        source_execution_seq.map_or_else(
                            || {
                                last_command_execution_seq
                                    .is_none_or(|previous| execution_seq >= previous)
                            },
                            |source| execution_seq >= source,
                        )
                    } else {
                        execution_seq
                            > source_execution_seq.expect("schema-5 gameplay source validated")
                    };
                    if !fence_advanced {
                        return Err(SimRealError::InvalidBoundaryContract {
                            step: state.step,
                            reason: format!(
                                "command_execution_seq {execution_seq} did not advance beyond the action source fence for {}",
                                pending_action.action.command
                            ),
                        });
                    }
                    last_command_execution_seq = Some(execution_seq);
                }
                let valid = if state_command {
                    kind == "poll"
                } else {
                    matches!(kind, "interaction_ready" | "quiescent" | "terminal")
                };
                if !valid {
                    let expected = if state_command {
                        "poll"
                    } else {
                        "interaction_ready, quiescent, or terminal"
                    };
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: state.step,
                        reason: format!(
                            "{} completed on {kind}; expected {expected}",
                            pending_action.action.command
                        ),
                    });
                }
                terminal_state_observed = trace_state_is_terminal(&state);

                let disposition = if first_boundary.is_some() {
                    streaming_disposition(
                        &pending_action,
                        ActionDispositionKind::BeyondBoundary,
                        Some("action follows the verifier boundary".to_owned()),
                    )
                } else {
                    let start = start.as_ref().ok_or(SimRealError::MissingStartCommand)?;
                    if start.matches_command(&pending_action.action.command) {
                        saw_start_completion = true;
                    }
                    let verified_before = report.verified.len();
                    let unsupported_before = report.unsupported.len();
                    let boundary = verify_seed_start_transition(
                        &mut replay,
                        &pending_action.action,
                        &state,
                        &pending_action.external_rng,
                        &mut report,
                        SeedStartReplayInputs {
                            start,
                            boss_unlocks,
                            profile: profile.as_ref().expect("metadata profile was validated"),
                            source_playtime_seconds: last_observed_playtime_seconds,
                        },
                        &mut replay_capture,
                    );
                    last_observed_playtime_seconds =
                        observed_playtime_seconds(&state).or(last_observed_playtime_seconds);
                    if let Some(boundary) = boundary.as_ref() {
                        first_boundary = Some(boundary.clone());
                    }
                    if boundary
                        .as_ref()
                        .is_some_and(|boundary| boundary.category == "unexpected_sim_real_diff")
                    {
                        let boundary = boundary.expect("unexpected diff boundary exists");
                        streaming_disposition(
                            &pending_action,
                            ActionDispositionKind::UnexpectedDiff,
                            Some(boundary.reason),
                        )
                    } else if report.verified.len() > verified_before {
                        let verified = report.verified.last().expect("new verified transition");
                        streaming_disposition(
                            &pending_action,
                            ActionDispositionKind::Verified,
                            Some(verified.label.clone()),
                        )
                    } else if report.unsupported.len() > unsupported_before {
                        let unsupported = report
                            .unsupported
                            .last()
                            .expect("new unsupported transition");
                        streaming_disposition(
                            &pending_action,
                            ActionDispositionKind::Unsupported,
                            Some(unsupported.reason.clone()),
                        )
                    } else {
                        let boundary = first_boundary
                            .as_ref()
                            .expect("transition without verification has a boundary");
                        streaming_disposition(
                            &pending_action,
                            ActionDispositionKind::Boundary,
                            Some(format!("{}: {}", boundary.category, boundary.reason)),
                        )
                    }
                };
                report.action_dispositions[pending_action.action_ordinal] = disposition;
                if first_boundary.is_some() && mode == VerificationReadMode::DiagnosticEarlyExit {
                    diagnostic_stopped = true;
                    break;
                }
            }
            TraceLine::Error(error) => {
                let Some(pending_action) = pending.take() else {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: error.step,
                        reason: "error has no immediately preceding action".to_owned(),
                    });
                };
                if pending_action.action.step != error.step {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: error.step,
                        reason: "error does not match the pending action".to_owned(),
                    });
                }
                if metadata_boundary_schema.is_some_and(|schema| schema >= 7) {
                    validate_schema7_rejection(
                        &pending_action.action,
                        &error.message,
                        last_command_execution_seq,
                        last_command_settlement_seq,
                    )?;
                    last_command_execution_seq = error
                        .message
                        .get("command_execution_seq")
                        .and_then(Value::as_u64);
                    last_command_settlement_seq = error
                        .message
                        .get("command_settlement_seq")
                        .and_then(Value::as_u64);
                } else if metadata_boundary_schema.is_some_and(|schema| schema >= 5) {
                    let source_execution_seq = action_source_command_execution_seq(
                        &pending_action.action,
                    )
                    .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                        step: error.step,
                        reason: "schema-5+ command error requires command_meta.source_command_execution_seq"
                            .to_owned(),
                    })?;
                    if let Some(previous) = last_command_execution_seq {
                        if source_execution_seq != previous {
                            return Err(SimRealError::InvalidBoundaryContract {
                                step: error.step,
                                reason: format!(
                                    "source_command_execution_seq {source_execution_seq} does not match preceding completion sequence {previous}"
                                ),
                            });
                        }
                    }
                    let execution_seq = error
                        .message
                        .get("command_execution_seq")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                            step: error.step,
                            reason: "schema-5+ command error requires command_execution_seq"
                                .to_owned(),
                        })?;
                    if execution_seq <= source_execution_seq {
                        return Err(SimRealError::InvalidBoundaryContract {
                            step: error.step,
                            reason: format!(
                                "error command_execution_seq {execution_seq} did not advance beyond source fence {source_execution_seq}"
                            ),
                        });
                    }
                    last_command_execution_seq = Some(execution_seq);
                }
                // A rejected command can still let the game's queued Time Warp
                // EndTurnAction drain between the command error and the next
                // observed state. Preserve that source-backed asynchronous
                // boundary instead of leaving the accepted pre-END candidate
                // frozen forever (FIDL01358).
                replay
                    .settle_time_warp_after_rejected_command()
                    .map_err(|settle_error| SimRealError::InvalidBoundaryContract {
                        step: error.step,
                        reason: format!(
                            "failed to settle queued Time Warp after rejected command: {settle_error}"
                        ),
                    })?;
                rejected_actions += 1;
                report.action_dispositions[pending_action.action_ordinal] = streaming_disposition(
                    &pending_action,
                    ActionDispositionKind::TargetRejected,
                    Some(
                        serde_json::to_string(&error.message)
                            .unwrap_or_else(|_| "target rejected command".to_owned()),
                    ),
                );
            }
            TraceLine::CommandAccept(_)
            | TraceLine::Response(_)
            | TraceLine::SlayTheData(_)
            | TraceLine::Automation(_)
            | TraceLine::CommandObservedTimeout(_) => {
                if let Some(pending_action) = pending.as_ref() {
                    return Err(SimRealError::InvalidBoundaryContract {
                        step: pending_action.action.step,
                        reason: "auxiliary record interrupts immediate action completion"
                            .to_owned(),
                    });
                }
            }
            TraceLine::Metadata(_) => unreachable!("metadata handled above"),
        }
    }

    if let Some(pending_action) = pending {
        return Err(SimRealError::InvalidBoundaryContract {
            step: pending_action.action.step,
            reason: "action has no immediate completing state at end of trace".to_owned(),
        });
    }
    if !metadata_seen {
        return Err(SimRealError::UnsupportedSchema {
            boundary_schema: None,
        });
    }
    if !saw_start_completion {
        return Err(SimRealError::MissingStartCommand);
    }

    let final_run_state = finish_streaming_seed_start_replay(&mut replay, &mut replay_capture);
    let boundary = first_boundary.unwrap_or_else(|| SeedStartBoundary {
        path: "$.actions[verified]".to_owned(),
        category: "none".to_owned(),
        reason: "schema-v1 verifier checked every direct simulator transition".to_owned(),
    });
    let start = start.expect("completed START was parsed");
    report.seed_start = Some(SeedStartReport {
        failed: boundary.category != "none",
        start_command: start,
        first_boundary: boundary,
        sim_run_state: final_run_state,
    });

    let disposed_actions = report
        .action_dispositions
        .iter()
        .filter(|entry| {
            !matches!(
                entry.disposition,
                ActionDispositionKind::Unclassified | ActionDispositionKind::TargetRejected
            )
        })
        .count();
    report.action_integrity = Some(VerificationIntegrity {
        eof_validated: mode == VerificationReadMode::Strict && !diagnostic_stopped,
        applicable_actions: report.total_actions - rejected_actions,
        disposed_actions,
        duplicate_dispositions: 0,
        terminal_state_observed,
        rejected_actions,
    });
    Ok(report)
}

fn validate_boundary_state(
    state: &TraceState,
    metadata_boundary_schema: u32,
) -> Result<&str, SimRealError> {
    let message = &state.message;
    let state_boundary_schema = message.get("boundary_schema").and_then(Value::as_u64);
    if state_boundary_schema != Some(u64::from(metadata_boundary_schema)) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: format!(
                "state boundary_schema must match metadata boundary_schema={metadata_boundary_schema}, received {}",
                state_boundary_schema
                    .map(|schema| schema.to_string())
                    .unwrap_or_else(|| "missing".to_owned())
            ),
        });
    }
    if metadata_boundary_schema >= 2
        && message
            .get("end_turn_queued")
            .and_then(Value::as_bool)
            .is_none()
    {
        return Err(SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: format!(
                "boundary schema {metadata_boundary_schema} requires boolean end_turn_queued"
            ),
        });
    }
    if metadata_boundary_schema >= 5
        && message
            .get("command_execution_seq")
            .and_then(Value::as_u64)
            .is_none()
    {
        return Err(SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: format!(
                "boundary schema {metadata_boundary_schema} requires non-negative integer command_execution_seq"
            ),
        });
    }
    if metadata_boundary_schema >= 6 {
        for field in [
            "effects_size",
            "top_level_effects_size",
            "queued_top_level_effects_size",
        ] {
            if message.get(field).and_then(Value::as_u64) != Some(0) {
                return Err(SimRealError::InvalidBoundaryContract {
                    step: state.step,
                    reason: format!("boundary schema 6 requires {field}=0"),
                });
            }
        }
    }
    if metadata_boundary_schema >= 7 {
        if message
            .get("command_settlement_seq")
            .and_then(Value::as_u64)
            .is_none()
        {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: "boundary schema 7 requires non-negative integer command_settlement_seq"
                    .to_owned(),
            });
        }
        let response_kind = message
            .get("command_response_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: "boundary schema 7 requires command_response_kind".to_owned(),
            })?;
        if !matches!(response_kind, "settled" | "poll" | "unsolicited") {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: format!("unsupported state command_response_kind {response_kind}"),
            });
        }
        let response_id = match message.get("command_response_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(id)) if !id.is_empty() => Some(id.as_str()),
            _ => {
                return Err(SimRealError::InvalidBoundaryContract {
                    step: state.step,
                    reason: "boundary schema 7 requires command_response_id to be a nonempty string or null"
                        .to_owned(),
                });
            }
        };
        let transaction_pending = message
            .get("transaction_pending")
            .and_then(Value::as_bool)
            .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: "boundary schema 7 requires boolean transaction_pending".to_owned(),
            })?;
        if matches!(response_kind, "settled" | "poll")
            && (response_id.is_none() || transaction_pending)
        {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: format!(
                    "schema-7 {response_kind} state requires a response ID and transaction_pending=false"
                ),
            });
        }
        if response_kind == "unsolicited" && response_id.is_some() {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: "schema-7 unsolicited state must not name a response ID".to_owned(),
            });
        }
    }
    let kind = message
        .get("boundary_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: "boundary_kind must be a string".to_owned(),
        })?;
    if !matches!(
        kind,
        "interaction_ready" | "quiescent" | "terminal" | "poll"
    ) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: format!("unsupported boundary_kind {kind}"),
        });
    }
    for field in [
        "game_update_seq",
        "dungeon_update_seq",
        "actions_queued",
        "card_queue_size",
        "pre_turn_actions_size",
    ] {
        if message.get(field).and_then(Value::as_u64).is_none() {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: format!("{field} must be a non-negative JSON integer"),
            });
        }
    }
    if message
        .get("current_action")
        .is_some_and(|value| !value.is_null())
    {
        if message
            .get("current_action")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: "current_action must be a string or null".to_owned(),
            });
        }
        for field in ["current_action_instance", "current_action_update_count"] {
            if message.get(field).and_then(Value::as_u64).is_none() {
                return Err(SimRealError::InvalidBoundaryContract {
                    step: state.step,
                    reason: format!("{field} must be a non-negative JSON integer"),
                });
            }
        }
    }
    let queue_is_empty = ["actions_queued", "card_queue_size", "pre_turn_actions_size"]
        .iter()
        .all(|field| message.get(field).and_then(Value::as_u64) == Some(0));
    let has_current_action = message
        .get("current_action")
        .is_some_and(|value| !value.is_null());
    if kind == "quiescent" && (has_current_action || !queue_is_empty) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: "quiescent boundary has active or queued work".to_owned(),
        });
    }
    if kind == "interaction_ready" && !has_current_action && queue_is_empty {
        return Err(SimRealError::InvalidBoundaryContract {
            step: state.step,
            reason: "interaction_ready boundary has no active or queued work".to_owned(),
        });
    }
    if matches!(kind, "interaction_ready" | "quiescent" | "terminal") {
        if message.get("ready_for_command").and_then(Value::as_bool) != Some(true) {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: format!("{kind} boundary is not ready for input"),
            });
        }
        let terminal_death_with_residual_end_turn = kind == "terminal"
            && message.get("in_game").and_then(Value::as_bool) == Some(true)
            && screen_type(message) == Some("GAME_OVER");
        if metadata_boundary_schema >= 2
            && kind != "interaction_ready"
            && !terminal_death_with_residual_end_turn
            && message.get("end_turn_queued").and_then(Value::as_bool) != Some(false)
        {
            return Err(SimRealError::InvalidBoundaryContract {
                step: state.step,
                reason: format!(
                    "{kind} schema-{metadata_boundary_schema} boundary cannot have an end turn queued"
                ),
            });
        }
    }
    Ok(kind)
}

fn seed_start_take_first_diff_boundary(report: &mut SimRealReport) -> Option<SeedStartBoundary> {
    let diffs = std::mem::take(&mut report.unexpected_diffs);
    let first = diffs.first()?;
    let reason = diffs
        .iter()
        .filter(|diff| diff.action_step == first.action_step)
        .flat_map(|diff| {
            diff.diffs
                .iter()
                .map(move |detail| format!("{}: {detail}", diff.label))
        })
        .collect::<Vec<_>>()
        .join("; ");
    report.unsupported.push(UnsupportedTransition {
        action_step: first.action_step,
        command: first.command.clone(),
        reason: reason.clone(),
    });
    Some(SeedStartBoundary {
        path: format!("$.actions[step={}].command", first.action_step),
        category: "unexpected_sim_real_diff".to_owned(),
        reason,
    })
}

fn validate_trace_profile(profile: &TraceProfile) -> Result<(), SimRealError> {
    let content_id = profile
        .note_card
        .as_deref()
        .map(|note_card| {
            content_id_from_key(note_card).ok_or_else(|| {
                SimRealError::InvalidProfileInput(format!("unknown Note card {note_card:?}"))
            })
        })
        .transpose()?;
    let mut run = RunState::seeded_ironclad(0, 0);
    run.note_card_content_id = content_id;
    run.note_card_upgrades = profile.note_upgrades;
    run.set_final_act_available(profile.final_act_available)
        .map_err(|error| {
            SimRealError::InvalidProfileInput(format!("invalid final-act profile input: {error}"))
        })?;
    run.validate().map_err(|error| {
        SimRealError::InvalidProfileInput(format!(
            "Note card {:?} with {} upgrade(s) is invalid: {error}",
            profile.note_card, profile.note_upgrades
        ))
    })
}

fn parse_start_command(action: &TraceAction) -> Option<Result<StartRunCommand, SimRealError>> {
    let parts: Vec<_> = action.command.split_whitespace().collect();
    let is_start = parts
        .first()
        .is_some_and(|command| command.eq_ignore_ascii_case("START"));
    let is_start_verify = parts
        .first()
        .is_some_and(|command| command.eq_ignore_ascii_case("START_VERIFY"));
    if !is_start && !is_start_verify {
        return None;
    }
    if (is_start && parts.len() != 4) || (is_start_verify && parts.len() != 5) {
        return Some(Err(SimRealError::MalformedStartCommand(
            action.command.clone(),
        )));
    }
    let ascension = match parts[2].parse::<u8>() {
        Ok(ascension) => ascension,
        Err(_) => {
            return Some(Err(SimRealError::MalformedStartCommand(
                action.command.clone(),
            )))
        }
    };
    let numeric_seed = match seed_text_to_long(parts[3]) {
        Ok(seed) => seed,
        Err(_) => {
            return Some(Err(SimRealError::MalformedStartCommand(
                action.command.clone(),
            )))
        }
    };
    let verification_starting_hp = if is_start_verify {
        match parts[4].parse::<i32>() {
            Ok(hp @ 1..=1_000_000) => Some(hp),
            _ => {
                return Some(Err(SimRealError::MalformedStartCommand(
                    action.command.clone(),
                )))
            }
        }
    } else {
        None
    };
    Some(Ok(StartRunCommand {
        action_step: action.step,
        character: parts[1].to_owned(),
        ascension,
        external_seed: parts[3].to_owned(),
        numeric_seed,
        verification_starting_hp,
    }))
}

fn seed_text_to_long(seed: &str) -> Result<i64, String> {
    let trimmed = seed.trim();
    if trimmed.starts_with('-') && trimmed[1..].chars().all(|ch| ch.is_ascii_digit()) {
        return trimmed
            .parse::<i64>()
            .map_err(|error| format!("invalid numeric seed {trimmed:?}: {error}"));
    }
    try_sts_seed_string_to_long(trimmed).map_err(|error| error.to_string())
}

fn command_choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("CHOOSE") {
        parts[1].parse::<usize>().ok()
    } else {
        None
    }
}

fn observed_playtime_seconds(state: &TraceState) -> Option<u32> {
    state
        .message
        .pointer("/game_state/playtime_seconds")
        .and_then(|value| {
            value.as_f64().or_else(|| {
                value.as_u64().map(|seconds| seconds as f64).or_else(|| {
                    value
                        .as_i64()
                        .and_then(|seconds| (seconds >= 0).then_some(seconds as f64))
                })
            })
        })
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map(|seconds| seconds.min(u32::MAX as f64).floor() as u32)
}

fn seed_start_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_combat_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    if game
        .get("screen_type")
        .and_then(Value::as_str)
        .is_some_and(|screen| screen == "COMBAT_REWARD")
    {
        return json!({
            "screen_type": "COMBAT_REWARD",
            "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
            "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
            "gold": int(game, "gold"),
            "current_hp": int(game, "current_hp"),
            "max_hp": int(game, "max_hp"),
            "deck_ids": deck_keys_from_value(game.get("deck")),
            "relic_ids": relic_keys_from_value(game.get("relics")),
            "potion_ids": potion_keys_from_value(game.get("potions")),
            "choices": choice_list_from_value(game.get("choice_list")),
            "reward_types": reward_types_from_value(game.get("screen_state").and_then(|state| state.get("rewards"))),
            "gold_offer": reward_gold_offer(game),
            "unobservable": {
                "reward_gold_rng_draws": true,
                "card_reward_rng_draws": true,
                "reward_screen_internal_ids": true,
            },
        });
    }

    let combat = game.get("combat_state");
    let player = combat.and_then(|combat| combat.get("player"));
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let current_hp = if screen_type == "CARD_REWARD" {
        int(game, "current_hp")
    } else {
        player
            .map(|p| int(p, "current_hp"))
            .unwrap_or_else(|| int(game, "current_hp"))
    };
    let monster_intents_visible =
        screen_type != "GAME_OVER" && observed_monster_intents_visible(game);
    let mut subset = json!({
        "screen_type": screen_type,
        "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": current_hp,
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "potion_ids": potion_keys_from_value(game.get("potions")),
        "combat_player_hp": if screen_type == "CARD_REWARD" { current_hp } else { player.map(|p| int(p, "current_hp")).unwrap_or(0) },
        "combat_player_block": player.map(|p| int(p, "block")).unwrap_or(0),
        "combat_player_energy": player.map(|p| int(p, "energy")).unwrap_or(0),
        "combat_player_frail": power_amount(player.and_then(|p| p.get("powers")), "Frail"),
        "combat_player_weak": power_amount(player.and_then(|p| p.get("powers")), "Weakened"),
        "combat_player_vulnerable": power_amount(player.and_then(|p| p.get("powers")), "Vulnerable"),
        "combat_player_artifact": power_amount(player.and_then(|p| p.get("powers")), "Artifact"),
        "hand_ids": combat_card_ids(combat.and_then(|combat| combat.get("hand"))),
        "draw_ids": combat_card_ids(combat.and_then(|combat| combat.get("draw_pile"))),
        "discard_ids": combat_card_ids(combat.and_then(|combat| combat.get("discard_pile"))),
        "monster_intents_visible": monster_intents_visible,
        "monsters": seed_start_monsters_from_value(
            combat.and_then(|combat| combat.get("monsters")),
            monster_intents_visible,
        ),
        "unobservable": {
            "shuffle_rng_draws": combat.and_then(|combat| combat.get("draw_pile")).and_then(Value::as_array).is_some_and(|draw| draw.len() == 5)
                && combat.and_then(|combat| combat.get("discard_pile")).and_then(Value::as_array).is_some_and(Vec::is_empty),
            "card_uuids": true,
            "card_reward_uuids": true,
        },
    });
    if screen_type == "CARD_REWARD" {
        if let Value::Object(map) = &mut subset {
            map.insert(
                "card_reward_ids".to_owned(),
                json!(card_reward_ids_from_value(
                    game.get("screen_state")
                        .and_then(|state| state.get("cards")),
                )),
            );
        }
    }
    subset
}

fn seed_start_reward_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut choices = choice_list_from_value(game.get("choice_list"));
    if screen_type == "CARD_REWARD" {
        // Pool key FOLLOW_UP and CommunicationMod display name Follow-Up are
        // the same card choice; card reward labels are presentation, while
        // card_reward_ids below remain the authoritative identity comparison.
        for choice in &mut choices {
            if choice.eq_ignore_ascii_case("follow-up") {
                *choice = "follow up".to_owned();
            }
        }
    }
    let mut out = json!({
        "screen_type": screen_type,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choices,
    });

    if let Value::Object(map) = &mut out {
        match screen_type {
            "CARD_REWARD" => {
                insert(
                    map,
                    "card_reward_ids",
                    card_reward_ids_from_value(
                        game.get("screen_state")
                            .and_then(|state| state.get("cards")),
                    ),
                );
                insert(
                    map,
                    "unobservable",
                    json!({
                        "card_reward_rng_draws": true,
                        "card_reward_uuids": true,
                    }),
                );
            }
            "COMBAT_REWARD" => {
                let reward_types = reward_types_from_value(
                    game.get("screen_state")
                        .and_then(|state| state.get("rewards")),
                );
                insert(map, "reward_types", reward_types.clone());
                let gold_offer = reward_gold_offer(game);
                if gold_offer > 0 {
                    insert(map, "gold_offer", gold_offer);
                }
                let stolen_gold_offer = reward_gold_at_reward_type_from_game(game, "STOLEN_GOLD");
                if stolen_gold_offer > 0 {
                    insert(map, "stolen_gold_offer", stolen_gold_offer);
                }
                insert(
                    map,
                    "choices",
                    reward_types
                        .iter()
                        .map(|reward_type| reward_type.to_ascii_lowercase())
                        .collect::<Vec<_>>(),
                );
                insert(
                    map,
                    "relic_offer_ids",
                    observed_reward_relic_offer_ids(game),
                );
                let unobservable = if reward_types.is_empty() {
                    json!({
                        "picked_card_uuid": true,
                    })
                } else {
                    json!({
                        "reward_gold_rng_draws": true,
                        "reward_screen_internal_ids": true,
                    })
                };
                insert(map, "unobservable", unobservable);
            }
            _ => {}
        }
    }
    out
}

fn seed_start_map_return_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let screen_state = game.get("screen_state");
    let current_node = screen_state.and_then(|state| state.get("current_node"));
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
        "first_node_chosen": screen_state
            .and_then(|state| state.get("first_node_chosen"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "current_node": {
            "symbol": current_node
                .and_then(|node| node.get("symbol"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            "x": current_node.and_then(|node| node.get("x")).and_then(Value::as_i64).unwrap_or(0),
            "y": current_node.and_then(|node| node.get("y")).and_then(Value::as_i64).unwrap_or(0),
        },
        "next_nodes": map_nodes_from_value(screen_state.and_then(|state| state.get("next_nodes"))),
    })
}

fn observed_monster_intents_visible(game: &Value) -> bool {
    !relic_keys_from_value(game.get("relics"))
        .iter()
        .any(|relic| relic.eq_ignore_ascii_case("Runic Dome"))
}

fn seed_start_monsters_from_value(value: Option<&Value>, intents_visible: bool) -> Vec<Value> {
    let Some(monsters) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    monsters
        .iter()
        .map(|monster| {
            let current_hp = int(monster, "current_hp");
            let is_gone = monster
                .get("is_gone")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut projected = json!({
                "name": monster.get("name").and_then(Value::as_str).unwrap_or(""),
                "current_hp": current_hp,
                "max_hp": int(monster, "max_hp"),
                "block": int(monster, "block"),
                "intent": monster.get("intent").and_then(Value::as_str).unwrap_or(""),
                "move_id": int(monster, "move_id"),
                "strength": power_amount(monster.get("powers"), "Strength"),
                "ritual": power_amount(monster.get("powers"), "Ritual"),
                "vulnerable": power_amount(monster.get("powers"), "Vulnerable"),
                "metallicize": power_amount(monster.get("powers"), "Metallicize"),
                "regeneration": power_amount(monster.get("powers"), "Regenerate"),
            });
            // CommunicationMod exports corpse powers inconsistently while the
            // target death animation settles. They are not gameplay state once
            // current HP is zero, so the documented visibility contract omits
            // only these three powers for dead monsters; living powers remain
            // strict and directly compared.
            if is_gone || current_hp <= 0 {
                let fields = projected
                    .as_object_mut()
                    .expect("projected monster is an object");
                fields.remove("strength");
                fields.remove("ritual");
                fields.remove("vulnerable");
                fields.remove("metallicize");
                fields.remove("regeneration");
            }
            if monster.get("move_id").and_then(Value::as_i64).is_none() {
                projected
                    .as_object_mut()
                    .expect("projected monster is an object")
                    .remove("move_id");
            }
            if !intents_visible {
                let fields = projected
                    .as_object_mut()
                    .expect("projected monster is an object");
                fields.remove("intent");
                fields.remove("move_id");
            }
            projected
        })
        .collect()
}

fn combat_card_ids(value: Option<&Value>) -> Vec<String> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    // Preserve the authoritative boundary's exact visible sequence, including
    // duplicates, for ordinary pile projections.
    cards
        .iter()
        .map(|card| {
            observed_card_projection_key(card)
                .expect("trace combat card schema was validated before projection")
        })
        .collect()
}

fn cards_to_comm_mod_visible_order<'a>(
    cards: impl IntoIterator<Item = &'a CardInstance>,
) -> Vec<String> {
    cards
        .into_iter()
        .map(simulated_card_projection_key)
        .collect()
}

fn any_color_card_projection_key(pool_key: &str, upgrades: u8) -> String {
    let mut label: String = pool_key
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    format!(
                        "{}{}",
                        first.to_ascii_uppercase(),
                        chars.as_str().to_ascii_lowercase()
                    )
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if upgrades > 0 && !label.ends_with('+') {
        label.push('+');
    }
    label
}

fn simulated_card_projection_key(card: &CardInstance) -> String {
    // Prismatic any-color deck cards must project their pool name (FIDL00288),
    // not the fallback "unknown" from the modeled Ironclad registry.
    if sts_core::content::cards::get_card_definition(card.content_id).is_none() {
        if let Some(key) = sts_core::run::reward::any_color_reward_card_key(card.content_id) {
            // Some CM deck ids omit spaces (HandOfGreed); prefer spaced Title Case
            // matching reward-screen names like "Blasphemy" / "Empty Body".
            return any_color_card_projection_key(key, card.upgrades);
        }
    }
    let key = modeled_card_projection_key(card.content_id);
    if card.searing_blow_upgrades > 1 {
        return format!(
            "{}+{}",
            key.trim_end_matches('+'),
            card.searing_blow_upgrades
        );
    }
    if card.upgrades > 0 && !key.ends_with('+') {
        return format!("{key}+");
    }
    key
}

fn draw_pile_to_comm_mod_visible_order(cards: &[CardInstance]) -> Vec<String> {
    cards_to_comm_mod_visible_order(cards.iter())
}

fn discard_pile_to_comm_mod_visible_order(cards: &[CardInstance]) -> Vec<String> {
    cards_to_comm_mod_visible_order(cards.iter())
}

fn power_amount(value: Option<&Value>, id: &str) -> i32 {
    let Some(powers) = value.and_then(Value::as_array) else {
        return 0;
    };
    powers
        .iter()
        .find(|power| {
            power
                .get("id")
                .or_else(|| power.get("name"))
                .and_then(Value::as_str)
                == Some(id)
        })
        .map(|power| int(power, "amount"))
        .unwrap_or(0)
}

fn seed_start_treasure_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_boss_reward_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let boss_relic_ids = observed_boss_relic_choice_ids(game);
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": boss_relic_ids.iter().map(|key| key.to_ascii_lowercase()).collect::<Vec<_>>(),
        "boss_relic_ids": boss_relic_ids,
    })
}

fn observed_boss_relic_choice_ids(game: &Value) -> Vec<String> {
    if game
        .get("screen_type")
        .and_then(Value::as_str)
        .is_none_or(|screen| screen != "BOSS_REWARD")
    {
        return Vec::new();
    }
    game.get("screen_state")
        .and_then(|screen| screen.get("relics"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|relic| {
            relic
                .get("name")
                .or_else(|| relic.get("id"))
                .and_then(Value::as_str)
        })
        .map(|identity| {
            relic_key_from_trace_name(identity)
                .map(relic_key_trace_name)
                .unwrap_or(identity)
                .to_owned()
        })
        .collect()
}

fn seed_start_rest_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_rest_simulated_subset(run: &RunState) -> Value {
    let choices = if run.phase == RunPhase::Rest && !run.rest_room_complete {
        let actions = match seed_start_rest_screen_actions(run) {
            Ok(actions) => actions,
            Err(error) => {
                return json!({
                    "simulator_error": format!(
                        "core legal-action boundary rejected rest state: {error}"
                    )
                });
            }
        };
        actions
            .into_iter()
            .filter_map(|action| match action {
                RestAction::Heal => Some("rest".to_owned()),
                RestAction::OpenSmith => Some("smith".to_owned()),
                RestAction::OpenRemove => Some("toke".to_owned()),
                RestAction::Lift => Some("lift".to_owned()),
                RestAction::Dig => Some("dig".to_owned()),
                RestAction::Recall => Some("recall".to_owned()),
                RestAction::Smith { .. } | RestAction::RemoveCard { .. } | RestAction::Proceed => {
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let screen_type = if run.card_grid.is_some() {
        "GRID"
    } else {
        "REST"
    };
    json!({
        "screen_type": screen_type,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": choices,
    })
}

fn seed_start_rest_screen_actions(run: &RunState) -> sts_core::SimResult<Vec<RestAction>> {
    Ok(legal_rest_decisions(run)?
        .into_iter()
        .filter(|action| {
            matches!(
                action,
                RestAction::Heal
                    | RestAction::OpenSmith
                    | RestAction::OpenRemove
                    | RestAction::Lift
                    | RestAction::Dig
                    | RestAction::Recall
            )
        })
        .collect())
}

fn seed_start_treasure_simulated_subset(run: &RunState) -> Value {
    let choices = if run.current_room_kind() == Some(RoomKind::Boss) && run.boss_chest_opened {
        Vec::new()
    } else {
        vec!["open".to_owned()]
    };
    json!({
        "screen_type": "CHEST",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": choices,
    })
}

fn seed_start_boss_reward_simulated_subset(run: &RunState) -> Value {
    let boss_relic_ids = run
        .reward
        .as_ref()
        .map(|reward| {
            reward
                .boss_relic_choices
                .iter()
                .map(|key| relic_key_trace_name(*key).to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "screen_type": "BOSS_REWARD",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": boss_relic_ids.iter().map(|key| key.to_ascii_lowercase()).collect::<Vec<_>>(),
        "boss_relic_ids": boss_relic_ids,
    })
}

fn seed_start_shop_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_shop_room_simulated_subset(run: &RunState) -> Value {
    json!({
        "screen_type": "SHOP_ROOM",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": ["shop"],
    })
}

fn seed_start_shop_trace_choice_labels(run: &RunState) -> Vec<String> {
    let Some(shop) = run.shop.as_ref() else {
        return Vec::new();
    };
    if run.card_grid.is_some() {
        return Vec::new();
    }

    let mut choices = affordable_shop_picks(run)
        .into_iter()
        .filter(|pick| !matches!(pick, ShopPick::BuyPotion(_)))
        .map(|pick| match pick {
            ShopPick::Purge => "purge".to_owned(),
            ShopPick::BuyCard(slot) => shop_card_trace_label(run, &shop.cards[slot].card),
            ShopPick::BuyRelic(slot) => {
                relic_key_trace_name(shop.relics[slot].relic_key).to_ascii_lowercase()
            }
            ShopPick::BuyPotion(_) => unreachable!("potions are projected below"),
        })
        .collect::<Vec<_>>();

    for offer in &shop.potions {
        // The target shop still advertises potion offers while Sozu prevents
        // acquiring them; `choice_list` is the UI's offer list, not the set of
        // actions that the player can successfully bind.
        if !offer.sold && run.gold >= offer.price {
            choices.push(potion_trace_label(offer.potion));
        }
    }

    choices
}

fn potion_trace_label(potion: Potion) -> String {
    potion_trace_name(potion).to_ascii_lowercase()
}

fn potion_trace_name(potion: Potion) -> &'static str {
    match potion {
        Potion::Fire => "Fire Potion",
        Potion::Block => "Block Potion",
        Potion::Fear => "Fear Potion",
        Potion::GamblersBrew => "Gambler's Brew",
        Potion::Blood => "Blood Potion",
        Potion::Elixir => "Elixir",
        Potion::HeartOfIron => "Heart of Iron",
        Potion::Dexterity => "Dexterity Potion",
        Potion::Energy => "Energy Potion",
        Potion::Explosive => "Explosive Potion",
        Potion::Strength => "Strength Potion",
        Potion::Swift => "Swift Potion",
        Potion::Weak => "Weak Potion",
        Potion::Attack => "Attack Potion",
        Potion::Skill => "Skill Potion",
        Potion::Power => "Power Potion",
        Potion::Colorless => "Colorless Potion",
        Potion::Flex => "Flex Potion",
        Potion::Speed => "Speed Potion",
        Potion::BlessingOfTheForge => "Blessing of the Forge",
        Potion::Regen => "Regen Potion",
        Potion::Ancient => "Ancient Potion",
        Potion::LiquidBronze => "Liquid Bronze",
        Potion::EssenceOfSteel => "Essence of Steel",
        Potion::Duplication => "Duplication Potion",
        Potion::DistilledChaos => "Distilled Chaos",
        Potion::LiquidMemories => "Liquid Memories",
        Potion::Cultist => "Cultist Potion",
        Potion::FruitJuice => "Fruit Juice",
        Potion::SneckoOil => "Snecko Oil",
        Potion::Fairy => "Fairy in a Bottle",
        Potion::SmokeBomb => "Smoke Bomb",
        Potion::EntropicBrew => "Entropic Brew",
    }
}

fn shop_card_trace_label(run: &RunState, card: &CardInstance) -> String {
    if card.searing_blow_upgrades > 0 {
        return format!("searing blow+{}", card.searing_blow_upgrades);
    }
    if egg_preview_upgrade(run, card.content_id)
        == Some(sts_core::content::cards::SEARING_BLOW_PLUS_ID)
    {
        return "searing blow+1".to_owned();
    }
    shop_card_display_key(run, card.content_id).to_ascii_lowercase()
}

fn shop_card_display_key(run: &RunState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::INFLAME_ID;
    if let Some(upgraded) = egg_preview_upgrade(run, content_id) {
        return content_key(upgraded);
    }
    if let Some(name) = shop_pool_trace_name(content_id) {
        return name;
    }
    if run_has_relic_key(run, RelicKey::FrozenEgg) && content_id == INFLAME_ID {
        return "Inflame+";
    }
    content_key(content_id)
}

fn shop_pool_trace_name(content_id: ContentId) -> Option<&'static str> {
    use sts_core::content::shop_pool::shop_card_content_id;
    const NAMES: &[(&str, &str)] = &[
        ("MIND_BLAST", "Mind Blast"),
        ("THINKING_AHEAD", "Thinking Ahead"),
    ];
    for (pool_name, trace_name) in NAMES {
        if shop_card_content_id(pool_name) == content_id {
            return Some(trace_name);
        }
    }
    None
}

fn seed_start_shop_screen_simulated_subset(run: &RunState) -> Value {
    json!({
        "screen_type": if run.card_grid.is_some() { "GRID" } else { "SHOP_SCREEN" },
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": seed_start_shop_trace_choice_labels(run),
    })
}

fn grid_trace_choice_label(_run: &RunState, card: &CardInstance) -> String {
    use sts_core::content::cards::{
        CURSE_OF_THE_BELL_ID, DEFEND_R_ID, RITUAL_DAGGER_ID, SEARING_BLOW_PLUS_ID, STRIKE_R_ID,
    };
    if card.content_id == RITUAL_DAGGER_ID && card.upgrades > 0 {
        return "ritual dagger+".to_owned();
    }
    if card.content_id == SEARING_BLOW_PLUS_ID && card.searing_blow_upgrades > 0 {
        return format!("searing blow+{}", card.searing_blow_upgrades);
    }
    if sts_core::content::cards::is_synthetic_any_color_content_id(card.content_id) {
        if let Some(key) = sts_core::run::reward::any_color_reward_card_key(card.content_id) {
            let mut label = if key == "JUDGEMENT" {
                "judgment".to_owned()
            } else {
                key.replace('_', " ").to_ascii_lowercase()
            };
            if card.upgrades > 0 {
                label.push('+');
            }
            return label;
        }
    }
    match card.content_id {
        id if id == STRIKE_R_ID => "strike".to_owned(),
        id if id == DEFEND_R_ID => "defend".to_owned(),
        id if id == CURSE_OF_THE_BELL_ID => "curse of the bell".to_owned(),
        _ => content_key(card.content_id).to_ascii_lowercase(),
    }
}

fn seed_start_grid_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let mut choices = choice_list_from_value(game.get("choice_list"));
    let grid_confirm_up = game
        .get("screen_state")
        .and_then(|state| state.get("confirm_up"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let selected_cards_visible = game
        .get("screen_state")
        .and_then(|state| state.get("selected_cards"))
        .and_then(Value::as_array)
        .is_some_and(|cards| !cards.is_empty());
    let card_selection_confirm = selected_cards_visible
        || ["for_purge", "for_transform", "for_upgrade"]
            .iter()
            .any(|key| {
                game.get("screen_state")
                    .and_then(|state| state.get(*key))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
    if choices.is_empty() && !(grid_confirm_up && card_selection_confirm) {
        choices = grid_card_choices_from_value(game);
    }
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choices,
    })
}

fn grid_card_choices_from_value(game: &Value) -> Vec<String> {
    game.get("screen_state")
        .and_then(|state| state.get("cards"))
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(|card| {
                    card.get("name")
                        .and_then(Value::as_str)
                        .or_else(|| card.get("id").and_then(Value::as_str))
                        .map(str::to_ascii_lowercase)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn seed_start_grid_simulated_subset(run: &RunState) -> Value {
    let choices = run
        .card_grid
        .as_ref()
        .map(|grid| {
            if grid_selection_ready_for_confirm(grid) {
                Vec::new()
            } else if grid.purpose == GridPurpose::CallingBellCurse {
                vec!["curse of the bell".to_owned()]
            } else if grid.purpose == GridPurpose::PandorasBox {
                grid.cards
                    .iter()
                    .rev()
                    .map(|card| grid_trace_choice_label(run, card))
                    .collect::<Vec<_>>()
            } else {
                grid.cards
                    .iter()
                    .map(|card| grid_trace_choice_label(run, card))
                    .collect::<Vec<_>>()
            }
        })
        .unwrap_or_default();
    json!({
        "screen_type": "GRID",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": choices,
    })
}

fn grid_selection_ready_for_confirm(grid: &CardGridScreen) -> bool {
    if grid.selected.is_some() {
        return true;
    }
    let required = match grid.purpose {
        GridPurpose::Astrolabe => Some(3),
        GridPurpose::EmptyCage { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowRemove { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowTransform { count }
        | GridPurpose::EventTransform { count }
        | GridPurpose::EventTransformReturnToEvent { count, .. } => Some(usize::from(count)),
        _ => None,
    };
    required.is_some_and(|required| grid.selected_indices.len() >= required)
}

fn room_kind_symbol(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Combat => "M",
        RoomKind::Event => "?",
        RoomKind::Shop => "$",
        RoomKind::Rest => "R",
        RoomKind::Elite => "E",
        RoomKind::Treasure => "T",
        RoomKind::Boss => "B",
        RoomKind::Victory => "V",
    }
}

fn seed_start_simulated_map_return(run: &RunState) -> Result<Value, String> {
    let map_state = run
        .map
        .as_ref()
        .ok_or_else(|| "core run state has no authoritative map".to_owned())?;
    let first_node_chosen = map_state.current_node.get() != 0;
    let (current_x, current_y, current_symbol) = if first_node_chosen {
        let current = map_state
            .map
            .node(map_state.current_node)
            .ok_or_else(|| "core map current node is missing".to_owned())?;
        let (x, y) = seed_start_map_node_xy(map_state.current_node);
        (x, y, room_kind_symbol(current.room_kind))
    } else if run.current_act >= 3 {
        // CommunicationMod exports the pre-first-room Act 3 anchor as (-1, 15).
        // Acts 1-2 use the ordinary (0, -1) pre-map anchor.
        (-1, 15, "")
    } else {
        (0, -1, "")
    };

    let mut map_action_run = run.clone();
    map_action_run.phase = RunPhase::Idle;
    // This is a deterministic completed-room projection, not authoritative replay state.
    // Remove simulator-owned overlays from the temporary copy before asking the core for
    // map decisions; no observed post-state participates in this normalization.
    map_action_run.combat = None;
    map_action_run.reward = None;
    map_action_run.event = None;
    map_action_run.shop = None;
    map_action_run.shop_merchant_open = false;
    map_action_run.card_grid = None;
    let legal_actions = legal_map_decisions(&map_action_run)
        .map_err(|error| format!("core legal-action boundary rejected map state: {error}"))?;
    let next_node_ids = legal_actions
        .into_iter()
        .map(|action| match action {
            sts_core::MapAction::ChooseNode { node_id } => node_id,
        })
        .collect::<Vec<_>>();
    if next_node_ids.is_empty() {
        return Err("core map state exposes no legal destination".to_owned());
    }

    let boss_available = next_node_ids.len() == 1
        && next_node_ids
            .first()
            .and_then(|id| map_state.map.node(*id))
            .is_some_and(|node| node.room_kind == RoomKind::Boss);
    let choices = if boss_available {
        vec!["boss".to_owned()]
    } else {
        next_node_ids
            .iter()
            .map(|id| {
                let (x, _) = seed_start_map_node_xy(*id);
                format!("x={x}")
            })
            .collect()
    };
    let next_nodes = if boss_available {
        Vec::new()
    } else {
        next_node_ids
            .iter()
            .map(|id| {
                let node = map_state
                    .map
                    .node(*id)
                    .ok_or_else(|| format!("core map destination {} is missing", id.get()))?;
                let (x, y) = seed_start_map_node_xy(*id);
                Ok(json!({
                    "symbol": room_kind_symbol(node.room_kind),
                    "x": x,
                    "y": y,
                }))
            })
            .collect::<Result<Vec<_>, String>>()?
    };
    Ok(json!({
        "screen_type": "MAP",
        "floor": run.current_floor.max(0) as u64,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": choices,
        "first_node_chosen": first_node_chosen,
        "current_node": {
            "symbol": current_symbol,
            "x": current_x,
            "y": current_y,
        },
        "next_nodes": next_nodes,
    }))
}

fn seed_start_map_node_xy(node_id: sts_core::MapNodeId) -> (i32, i64) {
    if node_id.get() == 0 {
        return (0, -1);
    }
    let index = node_id.get() - 1;
    ((index % 7) as i32, (index / 7) as i64)
}

fn seed_start_trace_monster_name(monster: &MonsterState) -> String {
    use sts_core::combat::SlimeSize;
    use sts_core::content::monsters::{
        get_monster_definition, ACID_SLIME_ID, BRONZE_ORB_ID, GREEN_LOUSE_ID, GREMLIN_FAT_ID,
        GREMLIN_THIEF_ID, GREMLIN_TSUNDERE_ID, GREMLIN_WARRIOR_ID, GUARDIAN_ID, RED_LOUSE_ID,
        SLAVER_BLUE_ID, SLAVER_RED_ID, SPIKE_SLIME_ID,
    };

    let slime_size = || match monster.slime_size.unwrap_or(match monster.max_hp {
        ..=19 => SlimeSize::Small,
        20..=49 => SlimeSize::Medium,
        _ => SlimeSize::Large,
    }) {
        SlimeSize::Small => "S",
        SlimeSize::Medium => "M",
        SlimeSize::Large => "L",
    };
    match monster.content_id {
        id if id == SPIKE_SLIME_ID => format!("Spike Slime ({})", slime_size()),
        id if id == ACID_SLIME_ID => format!("Acid Slime ({})", slime_size()),
        id if id == GREEN_LOUSE_ID || id == RED_LOUSE_ID => "Louse".to_owned(),
        id if id == SLAVER_BLUE_ID || id == SLAVER_RED_ID => "Slaver".to_owned(),
        id if id == GREMLIN_WARRIOR_ID => "Mad Gremlin".to_owned(),
        id if id == GREMLIN_THIEF_ID => "Sneaky Gremlin".to_owned(),
        id if id == GREMLIN_FAT_ID => "Fat Gremlin".to_owned(),
        id if id == GREMLIN_TSUNDERE_ID => "Shield Gremlin".to_owned(),
        id if id == BRONZE_ORB_ID => "Orb".to_owned(),
        id if id == GUARDIAN_ID => "The Guardian".to_owned(),
        content_id => get_monster_definition(content_id)
            .map(|definition| definition.name.to_owned())
            .unwrap_or_else(|| format!("Unknown monster content {}", content_id.get())),
    }
}

fn seed_start_trace_intent(monster: &MonsterState) -> String {
    use sts_core::content::monsters::{GREEN_LOUSE_ID, RED_LOUSE_ID};

    match monster.intent {
        MonsterIntent::Block { .. }
            if matches!(monster.content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) =>
        {
            "ATTACK".to_owned()
        }
        _ => intent_key(monster),
    }
}

fn reward_types_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(rewards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    rewards
        .iter()
        .filter_map(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn card_reward_ids_from_value(value: Option<&Value>) -> Vec<Value> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    cards
        .iter()
        .map(|card| {
            let identity = observed_display_card_identity(card)
                .or_else(|| card.get("id").and_then(Value::as_str).map(str::to_owned));
            if let Some(content_id) = content_id_from_card_value(card) {
                if sts_core::content::cards::is_synthetic_any_color_content_id(content_id) {
                    if let Some(pool_key) =
                        sts_core::run::reward::any_color_reward_card_key(content_id)
                    {
                        return json!(normalize_card_identity(pool_key));
                    }
                }
                // Some target card IDs differ from their pool identities (for
                // example Steam Barrier uses cardID `Steam`). When an observed
                // card is not in the modeled registry, canonicalize its display
                // identity through the complete any-color pool before falling
                // back to the raw target ID.
                if sts_core::content::cards::get_card_definition(content_id).is_none() {
                    if let Some(identity) = identity.as_deref() {
                        if let Some(pool_key) =
                            sts_core::run::reward::any_color_reward_card_key_from_identity(identity)
                        {
                            return json!(normalize_card_identity(pool_key));
                        }
                    }
                }
                return json!(content_id.get());
            }
            if let Some(identity) = identity.as_deref() {
                if let Some(pool_key) =
                    sts_core::run::reward::any_color_reward_card_key_from_identity(identity)
                {
                    return json!(normalize_card_identity(pool_key));
                }
            }
            json!(identity.expect("trace card reward schema was validated before projection"))
        })
        .collect()
}

fn normalize_card_identity(value: &str) -> String {
    let mut normalized: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    // CommunicationMod cardID is `Judgement`; some choice_list labels are `judgment`.
    if normalized == "judgment" {
        normalized = "judgement".to_owned();
    }
    normalized
}

fn simulated_card_reward_identity(content_id: ContentId) -> Value {
    if !sts_core::content::cards::is_synthetic_any_color_content_id(content_id)
        && sts_core::content::cards::get_card_definition(content_id).is_some()
    {
        return json!(content_id.get());
    }
    sts_core::run::reward::any_color_reward_card_key(content_id)
        .map(normalize_card_identity)
        .map_or_else(|| json!(content_id.get()), Value::String)
}

fn map_nodes_from_value(value: Option<&Value>) -> Vec<Value> {
    let Some(nodes) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    nodes
        .iter()
        .map(|node| {
            json!({
                "symbol": node.get("symbol").and_then(Value::as_str),
                "x": node.get("x").and_then(Value::as_i64),
                "y": node.get("y").and_then(Value::as_i64),
            })
        })
        .collect()
}

fn relic_keys_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(relics) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    relics
        .iter()
        .filter_map(|relic| {
            relic
                .get("name")
                .or_else(|| relic.get("id"))
                .and_then(Value::as_str)
                .map(|name| {
                    relic_key_from_trace_name(name)
                        .map(relic_key_trace_name)
                        .unwrap_or(name)
                        .to_owned()
                })
        })
        .collect()
}

fn choice_list_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(choices) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    choices
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn relic_key_trace_name(key: RelicKey) -> &'static str {
    key.trace_name()
}

fn relic_key_from_trace_name(name: &str) -> Option<RelicKey> {
    Relic::from_trace_name(name)
}

fn normalized_trace_relic_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn potion_from_trace_name(name: &str) -> Option<Potion> {
    match name {
        "Attack Potion" => Some(Potion::Attack),
        "Blessing of the Forge" => Some(Potion::BlessingOfTheForge),
        "Blood Potion" => Some(Potion::Blood),
        "Colorless Potion" => Some(Potion::Colorless),
        "Cultist Potion" => Some(Potion::Cultist),
        "Dexterity Potion" => Some(Potion::Dexterity),
        "Distilled Chaos" => Some(Potion::DistilledChaos),
        "Duplication Potion" => Some(Potion::Duplication),
        "Elixir" => Some(Potion::Elixir),
        "Energy Potion" => Some(Potion::Energy),
        "Entropic Brew" => Some(Potion::EntropicBrew),
        "Essence of Steel" => Some(Potion::EssenceOfSteel),
        "Explosive Potion" => Some(Potion::Explosive),
        "Fairy in a Bottle" => Some(Potion::Fairy),
        "Fear Potion" => Some(Potion::Fear),
        "Fire Potion" => Some(Potion::Fire),
        "Flex Potion" => Some(Potion::Flex),
        "Fruit Juice" => Some(Potion::FruitJuice),
        "Gamblers Brew" | "Gambler's Brew" | "GamblersBrew" => Some(Potion::GamblersBrew),
        "Heart of Iron" => Some(Potion::HeartOfIron),
        "Liquid Bronze" => Some(Potion::LiquidBronze),
        "Liquid Memories" => Some(Potion::LiquidMemories),
        "Power Potion" => Some(Potion::Power),
        "Regen Potion" => Some(Potion::Regen),
        "Skill Potion" => Some(Potion::Skill),
        "Smoke Bomb" => Some(Potion::SmokeBomb),
        "Snecko Oil" => Some(Potion::SneckoOil),
        "Speed Potion" => Some(Potion::Speed),
        "Strength Potion" => Some(Potion::Strength),
        "Swift Potion" => Some(Potion::Swift),
        "Weak Potion" => Some(Potion::Weak),
        "Block Potion" => Some(Potion::Block),
        "Ancient Potion" => Some(Potion::Ancient),
        _ => None,
    }
}

fn potion_keys_from_value(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|potions| potions.iter().filter_map(potion_key_from_value).collect())
        .unwrap_or_default()
}

fn potion_key_from_value(potion: &Value) -> Option<String> {
    let name = potion
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| potion.get("id").and_then(Value::as_str))?;
    if name.eq_ignore_ascii_case("Potion Slot") {
        return None;
    }
    Some(
        potion_from_trace_name(name)
            .map(|potion| potion_trace_name(potion).to_owned())
            .unwrap_or_else(|| name.to_owned()),
    )
}

fn relic_ids_for_simulated_subset(run: &RunState) -> Vec<String> {
    run.relics
        .iter()
        .map(|relic| relic_key_trace_name(relic.key()).to_owned())
        .filter(|name| name != "Unknown Relic")
        .collect()
}

fn run_has_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relics.iter().any(|relic| relic.key() == key)
}

fn seed_start_event_observed_subset(message: &Value) -> Value {
    let mut value = seed_start_observed_subset(message);
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "event_id".to_owned(),
            json!(seed_start_observed_event_key(message).unwrap_or_default()),
        );
        let choices = message
            .get("game_state")
            .and_then(|game| game.get("choice_list"))
            .map(|choices| choice_list_from_value(Some(choices)))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|choice| seed_start_visible_event_choice_label(&choice))
            .collect::<Vec<_>>();
        object.insert("choices".to_owned(), json!(choices));
    }
    value
}

fn seed_start_event_simulated_subset(run: &RunState) -> Value {
    seed_start_event_simulated_subset_with_deck(run, deck_content_keys(&run.deck))
}

fn seed_start_event_simulated_subset_with_deck(run: &RunState, deck_ids: Vec<String>) -> Value {
    let choices = run
        .event
        .as_ref()
        .map(|event| {
            seed_start_event_choice_presentations(run, event)
                .into_iter()
                .filter_map(|presentation| {
                    seed_start_event_choice_label(event.event, event.stage, presentation)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let event_id = run
        .event
        .as_ref()
        .map(|event| match event.event {
            sts_core::Event::Neow => "neowevent".to_owned(),
            sts_core::Event::TheSsssserpent => "liarsgame".to_owned(),
            sts_core::Event::HypnotizingColoredMushrooms => "mushrooms".to_owned(),
            // CommunicationMod id is `The Moai Head` → themoaihead (FIDL00232).
            sts_core::Event::MoaiHead => "themoaihead".to_owned(),
            _ => normalized_trace_relic_name(&format!("{:?}", event.event)),
        })
        .unwrap_or_default();
    let screen_type = if run.phase == RunPhase::Event {
        "EVENT"
    } else {
        "MAP"
    };
    json!({
        "screen_type": screen_type,
        "event_id": event_id,
        "ascension": run.ascension as u64,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_ids,
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": choices,
    })
}

fn seed_start_observed_event_key(message: &Value) -> Option<String> {
    let screen = message
        .get("game_state")
        .and_then(|game| game.get("screen_state"))?;
    let event = screen
        .get("event_id")
        .or_else(|| screen.get("event_name"))
        .and_then(Value::as_str)?;
    let key = normalized_trace_relic_name(event);
    Some(match key.as_str() {
        "goldenwing" => "wingstatue".to_owned(),
        _ => key,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartEventChoicePresentation<'a> {
    Text(&'a str),
    Card(ContentId),
    CardSlot(usize),
}

fn seed_start_event_choice_presentations<'a>(
    run: &'a RunState,
    event: &'a EventScreen,
) -> Vec<SeedStartEventChoicePresentation<'a>> {
    if event.event == Event::MatchAndKeep && event.stage == 2 {
        // CommunicationMod's GremlinMatchGame branch emits AbstractCard.cardID
        // for revealed cards and `card{position}` for hidden cards. Derive both
        // labels from the simulator-owned board, rather than from EventChoice
        // text or the observed choice_list.
        let state = run
            .match_and_keep
            .as_ref()
            .expect("validated Match and Keep choice projection has state");
        let card_count = state.cards.len();
        let mut presentations = Vec::new();
        for label_index in 0..card_count {
            let group_index =
                sts_core::match_and_keep_group_index_for_label(label_index, card_count)
                    .expect("validated Match and Keep label has a board slot");
            let card = state
                .cards
                .get(group_index)
                .expect("validated Match and Keep board slot exists");
            let currently_flipped = state.first_flipped_index == Some(group_index)
                || state.second_flipped_index == Some(group_index);
            if card.matched || currently_flipped {
                continue;
            }
            presentations.push(if card.revealed {
                SeedStartEventChoicePresentation::Card(card.content_id)
            } else {
                SeedStartEventChoicePresentation::CardSlot(label_index)
            });
        }
        return presentations;
    }

    event
        .choices
        .iter()
        .map(|choice| SeedStartEventChoicePresentation::Text(&choice.label))
        .collect()
}

fn seed_start_event_choice_label(
    event: Event,
    stage: u32,
    presentation: SeedStartEventChoicePresentation<'_>,
) -> Option<String> {
    match presentation {
        SeedStartEventChoicePresentation::Text(label) => {
            seed_start_visible_event_choice_label_for_event(event, stage, label)
        }
        SeedStartEventChoicePresentation::Card(content_id) => {
            Some(seed_start_communication_mod_card_id(content_id))
        }
        SeedStartEventChoicePresentation::CardSlot(label_index) => {
            Some(format!("card{label_index}"))
        }
    }
}

fn seed_start_communication_mod_card_id(content_id: ContentId) -> String {
    let source_card_id = sts_core::content::cards::communication_mod_card_id(content_id)
        .expect("validated card choice has a known card definition");
    // The surrounding event projection lowercases observed choice_list labels.
    // Fold case only after obtaining the exact source cardID; source spaces are
    // significant for ordinary IDs such as `Bandage Up`.
    source_card_id.to_ascii_lowercase()
}

fn seed_start_visible_event_choice_label_for_event(
    event: Event,
    stage: u32,
    label: &str,
) -> Option<String> {
    let normalized = seed_start_visible_event_choice_label(label)?;
    match (event, stage, normalized.as_str()) {
        // Colosseum's CommunicationMod choice IDs are the event's internal
        // outcome names, while the simulator's canonical labels are the
        // visible button text. Keep that protocol distinction at the trace
        // projection boundary; event mechanics continue to use Flee/Fight Nobs.
        (Event::Colosseum, 2, "flee") => Some("cowardice".to_owned()),
        (Event::Colosseum, 2, "fight nobs") => Some("victory".to_owned()),
        // CommunicationMod exposes Designer's fourth service as `punch`,
        // while the game's button text includes the passive `Get punched`.
        (Event::Designer, 1, "get punched") => Some("punch".to_owned()),
        // CommunicationMod exposes Vampires' Blood Vial option as `lose blood vial`.
        (Event::Vampires, 0, "give blood vial") => Some("lose blood vial".to_owned()),
        // CommunicationMod exposes Secret Portal accept as `enter the portal`,
        // then republishes the follow-up as `leave` instead of `continue`.
        (Event::SecretPortal, 0, "take the portal") => Some("enter the portal".to_owned()),
        (Event::SecretPortal, 1, "continue") => Some("leave".to_owned()),
        // CommunicationMod exposes Red Mask accept as `don the red mask`.
        (Event::TombOfLordRedMask, 0, "wear mask") => Some("don the red mask".to_owned()),
        // Moai Head CommMod choice_list uses short action tags, not the full
        // effect sentences (FIDL00232: jump inside / offer: golden idol).
        (Event::MoaiHead, 0, label) if label.starts_with("lose ") && label.contains("max hp") => {
            Some("jump inside".to_owned())
        }
        (Event::MoaiHead, 0, label) if label.contains("golden idol") => {
            Some("offer: golden idol".to_owned())
        }
        // Forgotten Altar's source button is `[Offer: Golden Idol]`; the core
        // choice is intentionally the semantic `Offer` action and is present
        // only when Golden Idol is owned. CommunicationMod's getOptionName
        // exposes the bracketed relic-specific label in choice_list.
        (Event::ForgottenAltar, 0, "offer") => Some("offer: golden idol".to_owned()),
        _ => Some(normalized),
    }
}

fn seed_start_visible_event_choice_label(label: &str) -> Option<String> {
    let mut label = label.to_ascii_lowercase();
    if let Some((visible, rest)) = label.split_once(" (") {
        let rest = rest.trim_end_matches(')').trim();
        if rest == "locked" {
            return None;
        }
        label = visible.to_owned();
    }
    label = label.trim_end_matches(['!', '?', '.', ':', ';']).to_owned();
    match label.as_str() {
        "locked" => None,
        "enter the light" => Some("enter".to_owned()),
        _ => Some(label),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartCombatDecision {
    CardReward,
    HandSelect,
    DrawSelect,
    DiscardSelect,
    ExhaustSelect,
}

fn seed_start_active_combat_decision(
    run: &RunState,
) -> Result<Option<SeedStartCombatDecision>, String> {
    let Some(combat) = run.combat.as_ref() else {
        return Ok(None);
    };
    Ok(combat.decision.as_ref().map(|decision| match decision {
        CombatDecisionState::PotionCardReward { .. }
        | CombatDecisionState::ToolboxCardReward { .. }
        | CombatDecisionState::DiscoveryCardReward { .. }
        | CombatDecisionState::NilrysCodexCardReward { .. } => SeedStartCombatDecision::CardReward,
        CombatDecisionState::HandSelect { .. } => SeedStartCombatDecision::HandSelect,
        CombatDecisionState::DrawSelect { .. } => SeedStartCombatDecision::DrawSelect,
        CombatDecisionState::DiscardSelect { .. } => SeedStartCombatDecision::DiscardSelect,
        CombatDecisionState::ExhaustSelect { .. } => SeedStartCombatDecision::ExhaustSelect,
    }))
}

fn seed_start_bind_combat_decision_command(
    decision: SeedStartCombatDecision,
    command: &str,
) -> Result<(RunAction, &'static str), String> {
    if command_head_eq(command, "CHOOSE") {
        let index = choose_index(command)
            .ok_or_else(|| format!("malformed combat decision command {command:?}"))?;
        return Ok(match decision {
            SeedStartCombatDecision::CardReward => (
                RunAction::ChooseCombatCardReward { index },
                "combat potion card reward",
            ),
            SeedStartCombatDecision::HandSelect => {
                (RunAction::ChooseHandSelect { index }, "hand select")
            }
            SeedStartCombatDecision::DrawSelect => {
                (RunAction::ChooseDrawSelect { index }, "draw select")
            }
            SeedStartCombatDecision::DiscardSelect => {
                (RunAction::ChooseDiscardSelect { index }, "discard select")
            }
            SeedStartCombatDecision::ExhaustSelect => {
                (RunAction::ChooseExhaustSelect { index }, "exhaust select")
            }
        });
    }
    if command.eq_ignore_ascii_case("CONFIRM") {
        return match decision {
            SeedStartCombatDecision::HandSelect => {
                Ok((RunAction::ConfirmHandSelect, "hand select confirm"))
            }
            SeedStartCombatDecision::DrawSelect => {
                Ok((RunAction::ConfirmDrawSelect, "draw select confirm"))
            }
            SeedStartCombatDecision::DiscardSelect => {
                Ok((RunAction::ConfirmDiscardSelect, "discard select confirm"))
            }
            SeedStartCombatDecision::ExhaustSelect => {
                Ok((RunAction::ConfirmExhaustSelect, "exhaust select confirm"))
            }
            SeedStartCombatDecision::CardReward => Err(
                "combat card rewards do not accept a CONFIRM command; choose or skip the offer"
                    .to_owned(),
            ),
        };
    }
    if command.eq_ignore_ascii_case("SKIP") && decision == SeedStartCombatDecision::CardReward {
        return Ok((
            RunAction::SkipCombatCardReward,
            "combat potion card reward skip",
        ));
    }
    Err(format!(
        "command {command:?} is not valid for active combat decision {decision:?}"
    ))
}

fn seed_start_simulated_combat_subset(run: &RunState) -> Value {
    let Some(combat) = run.combat.as_ref() else {
        return json!({
            "screen_type": "NO_COMBAT",
            "ascension": run.ascension,
            "floor": run.current_floor,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck_ids": deck_content_keys(&run.deck),
            "relic_ids": relic_ids_for_simulated_subset(run),
            "potion_ids": run.potions.iter().map(|potion| potion_trace_name(*potion)).collect::<Vec<_>>(),
            "combat_player_hp": run.player_hp,
            "combat_player_block": 0,
            "combat_player_energy": 0,
            "combat_player_frail": 0,
            "combat_player_weak": 0,
            "combat_player_vulnerable": 0,
            "combat_player_artifact": 0,
            "hand_ids": Vec::<String>::new(),
            "draw_ids": Vec::<String>::new(),
            "discard_ids": Vec::<String>::new(),
            "monsters": Vec::<Value>::new(),
            "unobservable": {
                "shuffle_rng_draws": false,
                "card_uuids": true,
                "card_reward_uuids": true,
            },
        });
    };
    let screen_type = seed_start_simulated_combat_screen_type(combat);
    let monster_intents_visible =
        screen_type != "GAME_OVER" && !run_has_relic_key(run, RelicKey::RunicDome);
    let combat_player_energy = combat.player.energy;
    let mut subset = json!({
        "screen_type": screen_type,
        "ascension": run.ascension,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": combat.player.hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "potion_ids": run.potions.iter().map(|potion| potion_trace_name(*potion)).collect::<Vec<_>>(),
        "combat_player_hp": combat.player.hp,
        "combat_player_block": combat.player.block,
        "combat_player_energy": combat_player_energy,
        "combat_player_frail": combat.player.powers.frail,
        "combat_player_weak": combat.player.powers.weak,
        "combat_player_vulnerable": combat.player.powers.vulnerable,
        "combat_player_artifact": combat.player.powers.artifact,
        "hand_ids": cards_to_comm_mod_visible_order(
            combat
                .piles
                .hand
                .iter()
                .enumerate()
                .filter(|(index, card)| {
                    let hidden_by_hand_select = combat.hand_select().is_some_and(|hand_select| {
                        card.id == hand_select.source_card_id
                            || hand_select.selected_hand_index == Some(*index)
                            || (matches!(
                                hand_select.purpose,
                                HandSelectPurpose::ForethoughtPutAnyOnDraw
                                    | HandSelectPurpose::PreparedDiscard
                            ) && hand_select.selected_hand_indices.contains(index))
                            || (hand_select.purpose == HandSelectPurpose::ArmamentsUpgrade
                                && !card_instance_is_upgradeable(card))
                            || (hand_select.purpose == HandSelectPurpose::DualWieldCopy
                                && card_type_and_rarity(card.content_id).is_none_or(|(card_type, _)| {
                                    !matches!(card_type, CardType::Attack | CardType::Power)
                                }))
                    });
                    let hidden_by_exhaust_select = combat
                        .exhaust_select()
                        .is_some_and(|exhaust_select| {
                            Some(card.id) == exhaust_select.source_card_id
                                || exhaust_select.selected_hand_indices.contains(index)
                        });
                    let hidden_after_interrupted_exhaust = combat
                        .pending_hidden_hand_card_until_end_turn
                        .iter()
                        .any(|hidden_card| hidden_card.id == card.id);
                    !hidden_by_hand_select
                        && !hidden_by_exhaust_select
                        && !hidden_after_interrupted_exhaust
                })
                .map(|(_, card)| card),
        ),
        "draw_ids": draw_pile_to_comm_mod_visible_order(&combat.piles.draw_pile),
        "discard_ids": discard_pile_to_comm_mod_visible_order(&combat.piles.discard_pile),
        "monster_intents_visible": monster_intents_visible,
        "monsters": seed_start_monsters_from_sim(combat, monster_intents_visible),
        "unobservable": {
            // Marker-only: CommunicationMod exposes pile order, not the shuffle draws
            // or card instance UUIDs used to build it. Keep this aligned with
            // seed_start_combat_observed_subset so combat entry is not rejected on
            // the markers alone.
            "shuffle_rng_draws": combat.piles.draw_pile.len() == 5
                && combat.piles.discard_pile.is_empty(),
            "card_uuids": true,
            "card_reward_uuids": true,
        },
    });
    if let Some(choices) = combat.combat_card_reward_choices() {
        if let Value::Object(map) = &mut subset {
            map.insert(
                "card_reward_ids".to_owned(),
                json!(choices
                    .iter()
                    .map(|card| card.content_id.get())
                    .collect::<Vec<_>>()),
            );
        }
    }
    subset
}

fn seed_start_simulated_combat_screen_type(combat: &CombatState) -> &'static str {
    if combat.phase == CombatPhase::Lost {
        "GAME_OVER"
    } else if combat.combat_card_reward_choices().is_some() {
        "CARD_REWARD"
    } else if combat.hand_select().is_some()
        || combat
            .exhaust_select()
            .is_some_and(|select| select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand)
    {
        "HAND_SELECT"
    } else if combat.draw_select().is_some()
        || combat.discard_select().is_some()
        || combat
            .exhaust_select()
            .is_some_and(|select| select.purpose == ExhaustSelectPurpose::ExhumeReturnToHand)
    {
        "GRID"
    } else {
        "NONE"
    }
}

fn seed_start_victory_observed_subset(message: &Value) -> Value {
    // Death GAME_OVER proceed ends with a terminal frame that has no game_state.
    if message.get("in_game").and_then(Value::as_bool) == Some(false)
        || message.get("game_state").is_none()
    {
        return json!({ "run_over": true });
    }
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
    })
}

fn seed_start_victory_simulated_subset(run: &RunState) -> Value {
    // RunPhase::Victory is the typed final-boss COMPLETE boundary before the
    // presentation-only PROCEED into the Spire Heart event.
    debug_assert_eq!(run.phase, RunPhase::Victory);
    json!({
        "screen_type": "COMPLETE",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
    })
}

fn seed_start_complete_simulated_subset(run: &RunState) -> Value {
    debug_assert_eq!(run.phase, RunPhase::Complete);
    // A positive-HP Complete run is the terminal Spire Heart outcome. The
    // Heart event has already advanced the room to Victory, so phase—not the
    // prior room kind—owns the GAME_OVER presentation.
    if run.player_hp <= 0 {
        return json!({ "run_over": true });
    }
    json!({
        "screen_type": "GAME_OVER",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
    })
}

fn sim_reward_combat_choices(run: &RunState, reward: &RewardScreen) -> Vec<String> {
    let mut choices = Vec::new();
    let has_relic = reward.relic_offer.is_some();
    let has_pending_relic = reward.pending_relic_offer.is_some();
    let event_map_reward = run.current_room_kind() == Some(RoomKind::Event)
        && reward.continuation == RewardContinuation::Map;
    let treasure_reward = run.current_room_kind() == Some(RoomKind::Treasure);
    let relic_before_gold = run
        .treasure_room
        .as_ref()
        .is_some_and(|treasure| treasure.relic_before_gold);
    if treasure_reward && has_relic {
        // An ordinary chest appends its gold reward before its own relic.
        // Matryoshka's onChestOpen hook inserts its bonus relic first, so its
        // two-relic chest remains [relic, gold, relic]. After the trailing
        // chest relic is claimed, treasure_room.relic_before_gold keeps the
        // residual order as [relic, gold] instead of flipping to [gold, relic].
        let matryoshka_style = has_pending_relic || relic_before_gold;
        if matryoshka_style {
            choices.push("relic".to_owned());
            if reward.gold_offer > 0 {
                choices.push("gold".to_owned());
            }
            if has_pending_relic {
                choices.push("relic".to_owned());
            }
        } else {
            if reward.gold_offer > 0 {
                choices.push("gold".to_owned());
            }
            choices.push("relic".to_owned());
        }
        choices.extend(std::iter::repeat_n(
            "relic".to_owned(),
            reward.queued_relic_offers.len(),
        ));
    } else if event_map_reward && has_relic {
        choices.push("relic".to_owned());
        if has_pending_relic {
            choices.push("relic".to_owned());
        }
        choices.extend(std::iter::repeat_n(
            "relic".to_owned(),
            reward.queued_relic_offers.len(),
        ));
        if reward.gold_offer > 0 {
            choices.push("gold".to_owned());
        }
    } else {
        if reward.stolen_gold_offer > 0 {
            choices.push("stolen_gold".to_owned());
        }
        if reward.gold_offer > 0 {
            choices.push("gold".to_owned());
        }
        if has_relic {
            choices.push("relic".to_owned());
            if has_pending_relic {
                choices.push("relic".to_owned());
            }
            choices.extend(std::iter::repeat_n(
                "relic".to_owned(),
                reward.queued_relic_offers.len(),
            ));
        }
    }
    if run.emerald_key_reward_available {
        choices.push("emerald_key".to_owned());
    }
    if !reward.potion_offers.is_empty() {
        choices.extend(std::iter::repeat_n(
            "potion".to_owned(),
            reward.potion_offers.len(),
        ));
    } else if reward.potion_offer.is_some() {
        choices.push("potion".to_owned());
    }
    // CombatRewardScreen keeps the card RewardItem in its outer list while an
    // opened CardRewardScreen is closed. The simulator deliberately preserves
    // the opened choices for reopening, so the pending count must take
    // precedence over that non-empty choice buffer when projecting the outer
    // reward list.
    if reward.card_reward_is_pending() {
        choices.extend(std::iter::repeat_n(
            "card".to_owned(),
            reward.remaining_card_reward_count() as usize,
        ));
    } else if !reward.choices.is_empty() && !reward.card_reward_is_active() {
        choices.push("card".to_owned());
    }
    if run
        .treasure_room
        .as_ref()
        .is_some_and(|treasure| treasure.sapphire_key_relic_offer.is_some())
    {
        choices.push("sapphire_key".to_owned());
    }
    choices
}

/// Bind CommunicationMod `CHOOSE <index>` on a reward boundary to the matching
/// simulator action.
///
/// Choice indices follow the projected CommunicationMod `choice_list`
/// (`sim_reward_combat_choices` / open card-reward labels), not the denser
/// `legal_run_decision_actions` vector that also contains Proceed/Skip/etc.
fn seed_start_bind_reward_choose_action(run: &RunState, index: usize) -> Result<RunAction, String> {
    let reward = run
        .reward
        .as_ref()
        .ok_or_else(|| "reward phase has no reward screen".to_owned())?;

    if reward.card_reward_is_active() {
        if let Some(card) = reward.choices.get(index) {
            return Ok(RunAction::TakeCardReward { card_id: card.id });
        }
        if run.relics.contains(&Relic::SingingBowl) && index == reward.choices.len() {
            return Ok(RunAction::TakeSingingBowlReward);
        }
        return Err(format!(
            "CHOOSE {index} is out of range for {} card-reward choices",
            reward.choices.len()
        ));
    }

    if !reward.boss_relic_choices.is_empty() {
        if index >= reward.boss_relic_choices.len() {
            return Err(format!(
                "CHOOSE {index} is out of range for {} boss relic choices",
                reward.boss_relic_choices.len()
            ));
        }
        return Ok(RunAction::ChooseBossRelicReward { index });
    }

    let choices = sim_reward_combat_choices(run, reward);
    let Some(label) = choices.get(index).map(String::as_str) else {
        return Err(format!(
            "CHOOSE {index} is out of range for reward choices {choices:?}"
        ));
    };
    let prior = |name: &str| {
        choices[..index]
            .iter()
            .filter(|choice| choice.as_str() == name)
            .count()
    };

    match label {
        "gold" => Ok(RunAction::TakeGoldReward),
        "stolen_gold" => Ok(RunAction::TakeStolenGoldReward),
        "card" => {
            let card_index = prior("card");
            // Prayer Wheel eagerly queues every card reward item; CHOOSE slots map
            // 1:1 onto that queue. Ordinary combat rewards use OpenCardReward for
            // the single pending item (and any non-queued fallback).
            if !reward.queued_card_rewards.is_empty()
                && reward.queued_card_rewards.len() == reward.remaining_card_reward_count() as usize
            {
                if card_index >= reward.queued_card_rewards.len() {
                    return Err(format!(
                        "CHOOSE {index} card slot {card_index} exceeds {} queued card rewards",
                        reward.queued_card_rewards.len()
                    ));
                }
                Ok(RunAction::OpenQueuedCardReward { index: card_index })
            } else if card_index == 0 {
                Ok(RunAction::OpenCardReward)
            } else if card_index - 1 < reward.queued_card_rewards.len() {
                Ok(RunAction::OpenQueuedCardReward {
                    index: card_index - 1,
                })
            } else {
                Ok(RunAction::OpenCardReward)
            }
        }
        "relic" => {
            let relic_index = prior("relic");
            let relic_count = usize::from(reward.relic_offer.is_some())
                + usize::from(reward.pending_relic_offer.is_some())
                + reward.queued_relic_offers.len();
            if relic_count <= 1 && relic_index == 0 {
                Ok(RunAction::TakeRelicReward)
            } else {
                Ok(RunAction::TakeRelicRewardAt { index: relic_index })
            }
        }
        "potion" => Ok(RunAction::TakePotionReward {
            index: prior("potion"),
        }),
        "sapphire_key" => Ok(RunAction::TakeSapphireKey),
        "emerald_key" => Ok(RunAction::TakeEmeraldKey),
        other => Err(format!(
            "unsupported combat-reward choice label {other:?} at CHOOSE {index}"
        )),
    }
}

fn seed_start_reward_simulated_subset(run: &RunState) -> Value {
    if run.card_grid.is_some() {
        return seed_start_grid_simulated_subset(run);
    }
    let floor = run.current_floor;
    let relic_ids = relic_ids_for_simulated_subset(run);

    if run
        .reward
        .as_ref()
        .is_some_and(RewardScreen::card_reward_is_active)
    {
        let reward = run.reward.as_ref().expect("card reward active");
        let mut card_choices = reward
            .choices
            .iter()
            .map(|card| reward_card_choice_display_key(run, card).to_ascii_lowercase())
            .collect::<Vec<_>>();
        if run.relics.contains(&Relic::SingingBowl) {
            card_choices.push("bowl".to_owned());
        }
        return json!({
            "screen_type": "CARD_REWARD",
            "floor": floor,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck_ids": deck_content_keys(&run.deck),
            "relic_ids": relic_ids_for_simulated_subset(run),
            "choices": card_choices,
            "card_reward_ids": reward
                .choices
                .iter()
                .map(|card| simulated_card_reward_identity(card.content_id))
                .collect::<Vec<_>>(),
            "unobservable": {
                "card_reward_rng_draws": true,
                "card_reward_uuids": true,
            },
        });
    }

    let reward = run.reward.as_ref();
    let combat_choices = reward
        .map(|reward| sim_reward_combat_choices(run, reward))
        .unwrap_or_default();
    let relic_offer_ids = reward
        .into_iter()
        .flat_map(|reward| {
            reward
                .relic_offer
                .iter()
                .chain(reward.pending_relic_offer.iter())
                .chain(reward.queued_relic_offers.iter())
        })
        .map(|relic| relic_key_trace_name(relic.key()))
        .collect::<Vec<_>>();
    let reward_types: Vec<String> = combat_choices
        .iter()
        .map(|choice| match choice.as_str() {
            "gold" => "GOLD",
            "stolen_gold" => "STOLEN_GOLD",
            "potion" => "POTION",
            "card" => "CARD",
            "relic" => "RELIC",
            "sapphire_key" => "SAPPHIRE_KEY",
            "emerald_key" => "EMERALD_KEY",
            _ => "UNKNOWN",
        })
        .map(str::to_owned)
        .collect();

    let mut out = json!({
        "screen_type": "COMBAT_REWARD",
        "floor": floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids,
        "choices": combat_choices,
        "reward_types": reward_types,
        "relic_offer_ids": relic_offer_ids,
    });

    if let Value::Object(map) = &mut out {
        if let Some(gold_offer) = reward
            .map(|reward| reward.gold_offer)
            .filter(|offer| *offer > 0)
        {
            insert(map, "gold_offer", gold_offer);
        }
        if let Some(stolen_gold_offer) = reward
            .map(|reward| reward.stolen_gold_offer)
            .filter(|offer| *offer > 0)
        {
            insert(map, "stolen_gold_offer", stolen_gold_offer);
        }
    }

    if let Value::Object(map) = &mut out {
        if reward_types.is_empty() {
            insert(
                map,
                "unobservable",
                json!({
                    "picked_card_uuid": true,
                }),
            );
        } else {
            insert(
                map,
                "unobservable",
                json!({
                    "reward_gold_rng_draws": true,
                    "reward_screen_internal_ids": true,
                }),
            );
        }
    }
    out
}

fn seed_start_monsters_from_sim(combat: &CombatState, intents_visible: bool) -> Vec<Value> {
    combat
        .monsters
        .iter()
        .enumerate()
        .map(|(index, monster)| {
            let name = seed_start_trace_monster_name(monster);
            let opening_move_id = combat
                .opening_turn_pending
                .then(|| combat.pending_opening_monster_intents.get(index))
                .flatten()
                .and_then(|intent| target_move_byte(monster.content_id, *intent));
            let move_id = opening_move_id
                .or_else(|| target_move_byte_for_monster(monster))
                .map(i32::from)
                .unwrap_or(-1);
            let mut projected = json!({
                "name": name,
                "current_hp": monster.hp.max(0),
                "max_hp": monster.max_hp,
                "block": monster.block,
                "intent": seed_start_trace_intent(monster),
                "move_id": move_id,
                "strength": monster.powers.strength,
                "ritual": monster.powers.ritual,
                "vulnerable": monster.powers.vulnerable,
                "metallicize": monster.powers.metallicize,
                "regeneration": monster.powers.regeneration,
            });
            // Keep the same explicit visibility boundary as the observed
            // projector. Dead powers are animation residue, not authoritative
            // gameplay state; living powers remain strict.
            if !monster.alive {
                let fields = projected
                    .as_object_mut()
                    .expect("projected monster is an object");
                fields.remove("strength");
                fields.remove("ritual");
                fields.remove("vulnerable");
                fields.remove("metallicize");
                fields.remove("regeneration");
            }
            if !intents_visible {
                let fields = projected
                    .as_object_mut()
                    .expect("projected monster is an object");
                fields.remove("intent");
                fields.remove("move_id");
            }
            projected
        })
        .collect()
}

fn unsupported_seed_start_combat_command(combat: &CombatState, command: &str) -> Option<String> {
    let parts: Vec<_> = command.split_whitespace().collect();
    let [cmd, hand_index, ..] = parts.as_slice() else {
        return None;
    };
    if !cmd.eq_ignore_ascii_case("PLAY") {
        return None;
    }
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    let card = combat.piles.hand.get(index)?;
    let key = content_key(card.content_id);
    if key != "unknown" {
        return None;
    }
    Some(format!(
        "card at hand index {} is not mapped in the verifier, so this combat command is unsupported",
        index + 1
    ))
}

fn observed_reward_relic_offer_ids(game: &Value) -> Vec<String> {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("RELIC"))
        })
        .filter_map(|reward| reward.get("relic"))
        .filter_map(|relic| relic.get("name").or_else(|| relic.get("id")))
        .filter_map(Value::as_str)
        .map(|identity| {
            relic_key_from_trace_name(identity)
                .map(relic_key_trace_name)
                .unwrap_or(identity)
                .to_owned()
        })
        .collect()
}

fn sole_living_enemy_target_if_required(
    combat: &CombatState,
    card_id: CardId,
) -> Option<MonsterId> {
    use sts_core::card::TargetRequirement;
    use sts_core::content::cards::get_card_definition;

    // Only direct enemy-target plays. Havoc/Mayhem force-plays use
    // random_living_target inside the core queue and must not receive a stale
    // target (illegal when the forced top card is non-targeted).
    let requires_enemy = combat
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .and_then(|card| get_card_definition(card.content_id))
        .is_some_and(|definition| definition.target == TargetRequirement::Enemy);
    if !requires_enemy {
        return None;
    }
    let mut living = combat.monsters.iter().filter(|monster| monster.alive);
    let first = living.next()?;
    if living.next().is_some() {
        return None;
    }
    Some(first.id)
}

fn combat_action_from_command(command: &str, combat: &CombatState) -> Option<CombatAction> {
    use sts_core::card::TargetRequirement;
    use sts_core::content::cards::get_card_definition;

    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("END") => Some(CombatAction::EndTurn),
        [cmd, hand_index] if cmd.eq_ignore_ascii_case("PLAY") => {
            let card_id = hand_card_id_from_bridge_slot(combat, hand_index)?;
            // CommunicationMod often omits the target index when only one living
            // enemy remains; the real client auto-targets that enemy.
            let target = sole_living_enemy_target_if_required(combat, card_id);
            Some(CombatAction::PlayCard { card_id, target })
        }
        [cmd, hand_index, target_index] if cmd.eq_ignore_ascii_case("PLAY") => {
            let card_id = hand_card_id_from_bridge_slot(combat, hand_index)?;
            let mut target = Some(monster_id_from_bridge_slot(combat, target_index)?);
            if let Some(definition) = combat
                .piles
                .hand
                .iter()
                .find(|card| card.id == card_id)
                .and_then(|card| get_card_definition(card.content_id))
            {
                if definition.target != TargetRequirement::Enemy {
                    target = None;
                }
            }
            Some(CombatAction::PlayCard { card_id, target })
        }
        _ => None,
    }
}

fn monster_id_from_bridge_slot(combat: &CombatState, target_index: &str) -> Option<MonsterId> {
    let index = target_index.parse::<usize>().ok()?;
    let monster = combat.monsters.get(index)?;
    monster.alive.then_some(monster.id)
}

fn hand_card_id_from_bridge_slot(combat: &CombatState, hand_index: &str) -> Option<CardId> {
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    Some(combat.piles.hand.get(index)?.id)
}

fn compare_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
) {
    let diffs = subset_diffs(expected, actual);
    if diffs.is_empty() {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: label.to_owned(),
        });
    } else {
        report.unexpected_diffs.push(UnexpectedDiff {
            action_step: action.step,
            command: action.command.clone(),
            label: label.to_owned(),
            diffs,
        });
    }
}

fn subset_diffs(expected: Value, actual: Value) -> Vec<String> {
    canonical_value_diff(&expected, &actual)
}

fn combat_label_for_action(action: CombatAction, run: &RunState) -> String {
    let CombatAction::PlayCard { card_id, .. } = action else {
        return "end turn".to_owned();
    };
    let Some(combat) = &run.combat else {
        return "combat".to_owned();
    };
    let key = combat
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| content_key(card.content_id))
        .unwrap_or("unknown");
    key.to_owned()
}

fn reward_gold_offer(game: &Value) -> i32 {
    reward_gold_at_reward_type_from_game(game, "GOLD")
}

fn reward_gold_at_reward_type_from_game(game: &Value, reward_type: &str) -> i32 {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)
        .and_then(|rewards| {
            rewards
                .iter()
                .find(|reward| {
                    reward
                        .get("reward_type")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| kind.eq_ignore_ascii_case(reward_type))
                })
                .and_then(|reward| reward.get("gold"))
        })
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32
}

fn card_upgrade_count(card: &Value) -> Option<u8> {
    card.get("upgrades")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
}

fn content_id_from_card_value(card: &Value) -> Option<ContentId> {
    let id = card.get("id").and_then(Value::as_str)?;
    let upgrades = card_upgrade_count(card)?;
    let base = content_id_from_key(id)?;
    if upgrades == 0 {
        return Some(base);
    }
    // Synthetic Prismatic cards retain one content identity and carry their
    // upgrade count on the instance.
    if sts_core::content::cards::is_synthetic_any_color_content_id(base) {
        return Some(base);
    }
    // CommunicationMod keeps the base `id` while exposing the authoritative
    // instance upgrade count. If a caller supplies an already-upgraded id, a
    // single upgrade is still that same content identity. Repeated upgrades
    // then follow the content's own upgrade lifecycle (Searing Blow is the
    // modeled self-upgrade case) rather than reading the display name.
    if upgrades == 1 && card_content_id_is_upgraded(base) {
        return Some(base);
    }
    (0..upgrades).try_fold(base, |content_id, _| upgrade_content_id(content_id))
}

fn card_content_id_is_upgraded(content_id: ContentId) -> bool {
    sts_core::content::cards::ALL_CARDS
        .iter()
        .any(|definition| definition.upgrade == Some(content_id))
}

fn observed_card_projection_key(card: &Value) -> Option<String> {
    let Some(content_id) = content_id_from_card_value(card) else {
        let identity = observed_display_card_identity(card)?;
        if let Some(pool_key) =
            sts_core::run::reward::any_color_reward_card_key_from_identity(&identity)
        {
            let upgrades = card_upgrade_count(card).unwrap_or(0);
            return Some(any_color_card_projection_key(pool_key, upgrades));
        }
        return Some(identity);
    };
    let upgrades = card_upgrade_count(card)?;
    if sts_core::content::cards::get_card_definition(content_id).is_none() {
        if let Some(pool_key) = sts_core::run::reward::any_color_reward_card_key(content_id) {
            return Some(any_color_card_projection_key(pool_key, upgrades));
        }
    }
    let mut key = modeled_card_projection_key(content_id);
    if upgrades > 1 && upgrade_content_id(content_id) == Some(content_id) {
        return Some(format!("{}+{}", key.trim_end_matches('+'), upgrades));
    }
    if upgrades > 0 && !key.ends_with('+') {
        key.push('+');
    }
    Some(key)
}

fn modeled_card_projection_key(content_id: ContentId) -> String {
    let key = deck_content_key(content_id);
    if card_content_id_is_upgraded(content_id) && !key.ends_with('+') {
        format!("{key}+")
    } else {
        key.to_owned()
    }
}

fn observed_display_card_identity(card: &Value) -> Option<String> {
    let id = card
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())?;
    let upgrades = card_upgrade_count(card)?;
    if upgrades == 0 {
        return Some(id.to_owned());
    }
    if let Some(name) = card
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
    {
        return Some(name.to_owned());
    }
    Some(format!("{id} [upgrades={upgrades}]"))
}

fn upgrade_content_id(base: ContentId) -> Option<ContentId> {
    sts_core::content::cards::upgrade_content_id(base)
}

fn normalized_observed_card_key(key: &str) -> String {
    let mut normalized = String::new();
    for ch in key.chars() {
        if ch == '+' {
            normalized.push_str("plus");
        } else if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        }
    }
    normalized
}

fn content_id_from_key(key: &str) -> Option<ContentId> {
    let normalized = normalized_observed_card_key(key);
    if let Some(definition) = sts_core::content::cards::ALL_CARDS
        .iter()
        .find(|definition| {
            normalized_observed_card_key(definition.key) == normalized
                || normalized_observed_card_key(definition.name) == normalized
        })
    {
        return Some(definition.id);
    }

    use sts_core::content::cards::{
        ANGER_ID, APPARITION_ID, ARMAMENTS_ID, BARRICADE_ID, BASH_ID, BASH_PLUS_ID,
        BATTLE_TRANCE_ID, BERSERK_ID, BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID,
        BLUDGEON_ID, BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID, BURN_ID, CARNAGE_ID,
        CHARGE_BATTERY_ANY_COLOR_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, CLUMSY_ID, COMBUST_ID,
        CORRUPTION_ID, CORRUPTION_PLUS_ID, DARK_EMBRACE_ID, DARK_SHACKLES_ID, DAZED_ID, DECAY_ID,
        DEEP_BREATH_ID, DEFEND_R_ID, DEFEND_R_PLUS_ID, DEMON_FORM_ID, DISARM_ID, DISCOVERY_ID,
        DOUBLE_TAP_ID, DOUBLE_TAP_PLUS_ID, DOUBT_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID,
        DUAL_WIELD_ID, ENTRENCH_ID, EQUILIBRIUM_ANY_COLOR_ID, EVOLVE_ID, EXHUME_ID, FEED_ID,
        FEEL_NO_PAIN_ID, FIEND_FIRE_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID,
        GHOSTLY_ARMOR_ID, HAVOC_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID,
        IMMOLATE_PLUS_ID, INFERNAL_BLADE_ID, INFLAME_ID, INJURY_ID, INTIMIDATE_ID, IRON_WAVE_ID,
        JACK_OF_ALL_TRADES_ID, JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, METALLICIZE_PLUS_ID,
        NECRONOMICURSE_ID, NORMALITY_ID, OFFERING_ID, PAIN_ID, PARASITE_ID, PERFECTED_STRIKE_ID,
        POMMEL_STRIKE_ID, POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID, RAMPAGE_ID, REAPER_ID,
        REAPER_PLUS_ID, RECKLESS_CHARGE_ID, REGRET_ID, RUPTURE_ID, RUPTURE_PLUS_ID,
        SEARING_BLOW_ID, SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID, SEVER_SOUL_ID, SHAME_ID,
        SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, SPOT_WEAKNESS_ID,
        STRIKE_R_ID, SWIFT_STRIKE_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID, TRIP_ID, TRUE_GRIT_ID,
        TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WILD_STRIKE_ID,
        WOUND_ID, WRITHE_ID,
    };
    match key {
        "Strike_R" | "Strike" => Some(STRIKE_R_ID),
        "Defend_R" | "Defend" => Some(DEFEND_R_ID),
        "Defend_R+" | "Defend+" => Some(DEFEND_R_PLUS_ID),
        "Bash" => Some(BASH_ID),
        "Bash+" | "bash+" => Some(BASH_PLUS_ID),
        "Bludgeon" | "bludgeon" => Some(BLUDGEON_ID),
        "Burn" | "burn" => Some(BURN_ID),
        "Burning Pact" | "burning pact" | "Burning Pact+" | "burning pact+" => {
            Some(BURNING_PACT_ID)
        }
        "Combust" | "combust" | "Combust+" | "combust+" => Some(COMBUST_ID),
        "Corruption" | "corruption" => Some(CORRUPTION_ID),
        "Corruption+" | "corruption+" => Some(CORRUPTION_PLUS_ID),
        "Dark Embrace" | "dark embrace" | "Dark Embrace+" | "dark embrace+" => {
            Some(DARK_EMBRACE_ID)
        }
        "Dazed" | "dazed" => Some(DAZED_ID),
        "Apparition" | "apparition" | "Ghostly" | "ghostly" => Some(APPARITION_ID),
        "Wound" | "wound" => Some(WOUND_ID),
        "Slimed" | "slimed" => Some(SLIMED_ID),
        "Thunderclap" | "thunderclap" => Some(THUNDERCLAP_ID),
        "Anger" | "anger" => Some(ANGER_ID),
        "Warcry" | "warcry" => Some(WARCRY_ID),
        "Warcry+" | "warcry+" => Some(WARCRY_PLUS_ID),
        "Metallicize" | "metallicize" => Some(METALLICIZE_ID),
        "Metallicize+" | "metallicize+" => Some(METALLICIZE_PLUS_ID),
        "Twin Strike" | "twin strike" => Some(TWIN_STRIKE_ID),
        "Battle Trance" | "battle trance" => Some(BATTLE_TRANCE_ID),
        "Shrug It Off" | "shrug it off" => Some(SHRUG_IT_OFF_ID),
        "Shrug It Off+" | "shrug it off+" => Some(SHRUG_IT_OFF_PLUS_ID),
        "Body Slam" | "body slam" => Some(BODY_SLAM_ID),
        "Clash" | "clash" => Some(CLASH_ID),
        "Cleave" | "cleave" => Some(CLEAVE_ID),
        "Deep Breath" | "deep breath" => Some(DEEP_BREATH_ID),
        "Dramatic Entrance" | "dramatic entrance" => Some(DRAMATIC_ENTRANCE_ID),
        "Dark Shackles" | "dark shackles" => Some(DARK_SHACKLES_ID),
        "Discovery" | "discovery" => Some(DISCOVERY_ID),
        "Swift Strike" | "swift strike" => Some(SWIFT_STRIKE_ID),
        "Jack Of All Trades" | "jack of all trades" => Some(JACK_OF_ALL_TRADES_ID),
        "Entrench" | "entrench" => Some(ENTRENCH_ID),
        "Fire Breathing" | "fire breathing" => Some(FIRE_BREATHING_ID),
        "Flex" | "flex" => Some(FLEX_ID),
        "Spot Weakness" | "spot weakness" => Some(SPOT_WEAKNESS_ID),
        "Flame Barrier" | "flame barrier" => Some(FLAME_BARRIER_ID),
        "Heavy Blade" | "heavy blade" => Some(HEAVY_BLADE_ID),
        "Intimidate" | "intimidate" => Some(INTIMIDATE_ID),
        "Iron Wave" | "iron wave" => Some(IRON_WAVE_ID),
        "Perfected Strike" | "perfected strike" => Some(PERFECTED_STRIKE_ID),
        "Sword Boomerang" | "sword boomerang" => Some(SWORD_BOOMERANG_ID),
        "True Grit" | "true grit" => Some(TRUE_GRIT_ID),
        "Headbutt" | "headbutt" => Some(HEADBUTT_ID),
        "Clothesline" | "clothesline" => Some(CLOTHESLINE_ID),
        "Shockwave" | "shockwave" => Some(SHOCKWAVE_ID),
        "Rampage" | "rampage" => Some(RAMPAGE_ID),
        "Rage" | "rage" => Some(RAGE_ID),
        "Whirlwind" | "whirlwind" => Some(WHIRLWIND_ID),
        "Pommel Strike" | "pommel strike" => Some(POMMEL_STRIKE_ID),
        "Pummel" | "pummel" => Some(PUMMEL_ID),
        "Searing Blow" | "searing blow" => Some(SEARING_BLOW_ID),
        "Sever Soul" | "sever soul" => Some(SEVER_SOUL_ID),
        "Sentinel" | "sentinel" => Some(SENTINEL_ID),
        "Uppercut" | "uppercut" => Some(UPPERCUT_ID),
        "Disarm" | "disarm" => Some(DISARM_ID),
        "Dual Wield" | "dual wield" => Some(DUAL_WIELD_ID),
        "Immolate" | "immolate" => Some(IMMOLATE_ID),
        "Immolate+" | "immolate+" => Some(IMMOLATE_PLUS_ID),
        "Berserk" | "berserk" => Some(BERSERK_ID),
        "Limit Break" | "limit break" => Some(LIMIT_BREAK_ID),
        "Armaments" | "armaments" => Some(ARMAMENTS_ID),
        "Regret" | "regret" => Some(REGRET_ID),
        "Doubt" | "doubt" => Some(DOUBT_ID),
        "Clumsy" | "clumsy" => Some(CLUMSY_ID),
        "Decay" | "decay" => Some(DECAY_ID),
        "Injury" | "injury" => Some(INJURY_ID),
        "Normality" | "normality" => Some(NORMALITY_ID),
        "Pain" | "pain" => Some(PAIN_ID),
        "Parasite" | "parasite" => Some(PARASITE_ID),
        "Necronomicurse" | "necronomicurse" => Some(NECRONOMICURSE_ID),
        "Shame" | "shame" => Some(SHAME_ID),
        "Writhe" | "writhe" => Some(WRITHE_ID),
        "Offering" | "offering" => Some(OFFERING_ID),
        "Demon Form" | "demon form" => Some(DEMON_FORM_ID),
        "Double Tap" | "double tap" => Some(DOUBLE_TAP_ID),
        "Double Tap+" | "double tap+" => Some(DOUBLE_TAP_PLUS_ID),
        "Barricade" | "barricade" => Some(BARRICADE_ID),
        "Bloodletting" | "bloodletting" => Some(BLOODLETTING_ID),
        "Blood for Blood" | "blood for blood" => Some(BLOOD_FOR_BLOOD_ID),
        "Blood for Blood+" | "blood for blood+" => Some(BLOOD_FOR_BLOOD_PLUS_ID),
        "Reaper" | "reaper" => Some(REAPER_ID),
        "Reaper+" | "reaper+" => Some(REAPER_PLUS_ID),
        "Rupture" | "rupture" => Some(RUPTURE_ID),
        "Rupture+" | "rupture+" => Some(RUPTURE_PLUS_ID),
        "Hemokinesis" | "hemokinesis" => Some(HEMOKINESIS_ID),
        "Dropkick" | "dropkick" => Some(DROPKICK_ID),
        "Wild Strike" | "wild strike" => Some(WILD_STRIKE_ID),
        "Power Through" | "power through" => Some(POWER_THROUGH_ID),
        "Infernal Blade" | "infernal blade" => Some(INFERNAL_BLADE_ID),
        "Ghostly Armor" | "ghostly armor" => Some(GHOSTLY_ARMOR_ID),
        "Reckless Charge" | "reckless charge" => Some(RECKLESS_CHARGE_ID),
        "Feel No Pain" | "feel no pain" => Some(FEEL_NO_PAIN_ID),
        "Seeing Red" | "seeing red" => Some(SEEING_RED_ID),
        "Inflame" | "inflame" => Some(INFLAME_ID),
        "Havoc" | "havoc" => Some(HAVOC_ID),
        "Second Wind" | "second wind" => Some(SECOND_WIND_ID),
        "Carnage" | "carnage" => Some(CARNAGE_ID),
        "Evolve" | "evolve" => Some(EVOLVE_ID),
        "Feed" | "feed" => Some(FEED_ID),
        "Fiend Fire" | "fiend fire" => Some(FIEND_FIRE_ID),
        "Juggernaut" | "juggernaut" => Some(JUGGERNAUT_ID),
        "Brutality" | "brutality" => Some(BRUTALITY_ID),
        "Exhume" | "exhume" => Some(EXHUME_ID),
        "Trip" | "trip" => Some(TRIP_ID),
        "Conserve Battery" | "conserve battery" => Some(CHARGE_BATTERY_ANY_COLOR_ID),
        "Undo" | "undo" => Some(EQUILIBRIUM_ANY_COLOR_ID),
        "ClearTheMind" | "clear the mind" => {
            sts_core::run::reward::any_color_reward_card_key_from_identity("TRANQUILITY")
                .map(sts_core::content::shop_pool::shop_card_content_id)
        }
        "PathToVictory" | "path to victory" => {
            sts_core::run::reward::any_color_reward_card_key_from_identity("PRESSURE_POINTS")
                .map(sts_core::content::shop_pool::shop_card_content_id)
        }
        _ => sts_core::run::reward::any_color_reward_card_key_from_identity(key)
            .map(sts_core::content::shop_pool::shop_card_content_id),
    }
}

fn content_key(content_id: ContentId) -> &'static str {
    if let Some(definition) = sts_core::content::cards::get_card_definition(content_id) {
        return definition.name;
    }
    if let Some(key) = sts_core::run::reward::any_color_reward_card_key(content_id) {
        // Stable static pool names (e.g. BLASPHEMY) — display mapping happens in
        // simulated_card_projection_key; combat support checks only need non-unknown.
        return key;
    }

    use sts_core::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BARRICADE_ID, BASH_ID, BASH_PLUS_ID, BATTLE_TRANCE_ID, BERSERK_ID,
        BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID, BLUDGEON_ID, BODY_SLAM_ID,
        BURNING_PACT_ID, BURN_ID, CHRYSALIS_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, CLUMSY_ID,
        COMBUST_ID, CORRUPTION_ID, CORRUPTION_PLUS_ID, CURSE_OF_THE_BELL_ID, DARK_EMBRACE_ID,
        DARK_SHACKLES_ID, DAZED_ID, DECAY_ID, DEEP_BREATH_ID, DEFEND_R_ID, DEFEND_R_PLUS_ID,
        DEMON_FORM_ID, DISARM_ID, DISCOVERY_ID, DOUBLE_TAP_ID, DOUBLE_TAP_PLUS_ID, DOUBT_ID,
        DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID, ENTRENCH_ID, FEED_ID, FEEL_NO_PAIN_ID,
        FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID, FLEX_PLUS_ID, HAND_OF_GREED_ID, HAVOC_ID,
        HAVOC_PLUS_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, IMMOLATE_PLUS_ID,
        IMPERVIOUS_ID, INFLAME_ID, INFLAME_PLUS_ID, INJURY_ID, INTIMIDATE_ID,
        JACK_OF_ALL_TRADES_ID, LIMIT_BREAK_ID, MAGNETISM_ID, MAYHEM_ID, METALLICIZE_ID,
        METALLICIZE_PLUS_ID, NECRONOMICURSE_ID, NORMALITY_ID, OFFERING_ID, OFFERING_PLUS_ID,
        PAIN_ID, PARASITE_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID,
        RAGE_ID, RAMPAGE_ID, REAPER_ID, REAPER_PLUS_ID, REGRET_ID, RUPTURE_ID, RUPTURE_PLUS_ID,
        SEARING_BLOW_ID, SECRET_WEAPON_ID, SENTINEL_ID, SEVER_SOUL_ID, SHAME_ID, SHOCKWAVE_ID,
        SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, SPOT_WEAKNESS_ID, STRIKE_R_ID,
        STRIKE_R_PLUS_ID, SWIFT_STRIKE_ID, SWIFT_STRIKE_PLUS_ID, SWORD_BOOMERANG_ID,
        THUNDERCLAP_ID, TRANSMUTATION_ID, TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID,
        WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WILD_STRIKE_ID, WOUND_ID, WRITHE_ID,
    };
    match content_id {
        id if id == STRIKE_R_ID || id == STRIKE_R_PLUS_ID => "Strike_R",
        id if id == DEFEND_R_ID || id == DEFEND_R_PLUS_ID => "Defend_R",
        id if id == BASH_ID || id == BASH_PLUS_ID => "Bash",
        id if id == BLUDGEON_ID => "Bludgeon",
        id if id == BURN_ID => "Burn",
        id if id == BURNING_PACT_ID => "Burning Pact",
        id if id == CURSE_OF_THE_BELL_ID => "Curse of the Bell",
        id if id == DARK_EMBRACE_ID => "Dark Embrace",
        id if id == DAZED_ID => "Dazed",
        id if id == WOUND_ID => "Wound",
        id if id == SLIMED_ID => "Slimed",
        id if id == THUNDERCLAP_ID => "Thunderclap",
        id if id == ANGER_ID => "Anger",
        id if id == WARCRY_ID => "Warcry",
        id if id == WARCRY_PLUS_ID => "Warcry+",
        id if id == METALLICIZE_ID => "Metallicize",
        id if id == METALLICIZE_PLUS_ID => "Metallicize+",
        id if id == TWIN_STRIKE_ID => "Twin Strike",
        id if id == BATTLE_TRANCE_ID => "Battle Trance",
        id if id == SHRUG_IT_OFF_ID => "Shrug It Off",
        id if id == SHRUG_IT_OFF_PLUS_ID => "Shrug It Off+",
        id if id == BODY_SLAM_ID => "Body Slam",
        id if id == CLASH_ID => "Clash",
        id if id == CLEAVE_ID => "Cleave",
        id if id == WILD_STRIKE_ID => "Wild Strike",
        id if id == HAVOC_ID => "Havoc",
        id if id == HAVOC_PLUS_ID => "Havoc+",
        id if id == INFLAME_ID => "Inflame",
        id if id == INFLAME_PLUS_ID => "Inflame+",
        id if id == COMBUST_ID => "Combust",
        id if id == CORRUPTION_ID => "Corruption",
        id if id == CORRUPTION_PLUS_ID => "Corruption+",
        id if id == OFFERING_ID => "Offering",
        id if id == OFFERING_PLUS_ID => "Offering+",
        id if id == DOUBLE_TAP_ID => "Double Tap",
        id if id == DOUBLE_TAP_PLUS_ID => "Double Tap+",
        id if id == DEEP_BREATH_ID => "Deep Breath",
        id if id == DRAMATIC_ENTRANCE_ID => "Dramatic Entrance",
        id if id == DARK_SHACKLES_ID => "Dark Shackles",
        id if id == DISCOVERY_ID => "Discovery",
        id if id == SWIFT_STRIKE_ID => "Swift Strike",
        id if id == SWIFT_STRIKE_PLUS_ID => "Swift Strike+",
        id if id == JACK_OF_ALL_TRADES_ID => "Jack Of All Trades",
        id if id == ENTRENCH_ID => "Entrench",
        id if id == FIRE_BREATHING_ID => "Fire Breathing",
        id if id == FLEX_ID => "Flex",
        id if id == FLEX_PLUS_ID => "Flex+",
        id if id == SPOT_WEAKNESS_ID => "Spot Weakness",
        id if id == FLAME_BARRIER_ID => "Flame Barrier",
        id if id == HEAVY_BLADE_ID => "Heavy Blade",
        id if id == INTIMIDATE_ID => "Intimidate",
        id if id == PERFECTED_STRIKE_ID => "Perfected Strike",
        id if id == SWORD_BOOMERANG_ID => "Sword Boomerang",
        id if id == TRUE_GRIT_ID => "True Grit",
        id if id == HEADBUTT_ID => "Headbutt",
        id if id == IMMOLATE_ID || id == IMMOLATE_PLUS_ID => "Immolate",
        id if id == BERSERK_ID => "Berserk",
        id if id == LIMIT_BREAK_ID => "Limit Break",
        id if id == IMPERVIOUS_ID => "Impervious",
        id if id == FEED_ID => "Feed",
        id if id == ARMAMENTS_ID => "Armaments",
        id if id == CLOTHESLINE_ID => "Clothesline",
        id if id == SHOCKWAVE_ID => "Shockwave",
        id if id == RAMPAGE_ID => "Rampage",
        id if id == RAGE_ID => "Rage",
        id if id == WHIRLWIND_ID => "Whirlwind",
        id if id == POMMEL_STRIKE_ID => "Pommel Strike",
        id if id == POMMEL_STRIKE_PLUS_ID => "Pommel Strike+",
        id if id == SEVER_SOUL_ID => "Sever Soul",
        id if id == SENTINEL_ID => "Sentinel",
        id if id == UPPERCUT_ID => "Uppercut",
        id if id == DISARM_ID => "Disarm",
        id if id == DUAL_WIELD_ID => "Dual Wield",
        id if id == SEARING_BLOW_ID => "Searing Blow",
        id if id == REGRET_ID => "Regret",
        id if id == DOUBT_ID => "Doubt",
        id if id == CLUMSY_ID => "Clumsy",
        id if id == DECAY_ID => "Decay",
        id if id == INJURY_ID => "Injury",
        id if id == NORMALITY_ID => "Normality",
        id if id == PAIN_ID => "Pain",
        id if id == PARASITE_ID => "Parasite",
        id if id == NECRONOMICURSE_ID => "Necronomicurse",
        id if id == SHAME_ID => "Shame",
        id if id == WRITHE_ID => "Writhe",
        id if id == DEMON_FORM_ID => "Demon Form",
        id if id == BARRICADE_ID => "Barricade",
        id if id == BLOODLETTING_ID => "Bloodletting",
        id if id == BLOOD_FOR_BLOOD_ID => "Blood for Blood",
        id if id == BLOOD_FOR_BLOOD_PLUS_ID => "Blood for Blood+",
        id if id == REAPER_ID => "Reaper",
        id if id == REAPER_PLUS_ID => "Reaper+",
        id if id == RUPTURE_ID => "Rupture",
        id if id == RUPTURE_PLUS_ID => "Rupture+",
        id if id == HEMOKINESIS_ID => "Hemokinesis",
        id if id == DROPKICK_ID => "Dropkick",
        id if id == TRIP_ID => "Trip",
        id if id == FEEL_NO_PAIN_ID => "Feel No Pain",
        id if id == MAYHEM_ID => "Mayhem",
        id if id == SECRET_WEAPON_ID => "Secret Weapon",
        id if id == TRANSMUTATION_ID => "Transmutation",
        id if id == MAGNETISM_ID => "Magnetism",
        id if id == CHRYSALIS_ID => "Chrysalis",
        id if id == HAND_OF_GREED_ID => "Hand Of Greed",
        other if shop_pool_trace_name(other).is_some() => {
            shop_pool_trace_name(other).unwrap_or("unknown")
        }
        _ => "unknown",
    }
}

fn deck_content_key(content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        DEFEND_R_ID, DEFEND_R_PLUS_ID, FLEX_PLUS_ID, HAND_OF_GREED_ID, HAND_OF_GREED_PLUS_ID,
        HAVOC_PLUS_ID, INFLAME_PLUS_ID, OFFERING_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID, WARCRY_PLUS_ID,
    };
    match content_id {
        id if id == STRIKE_R_ID || id == STRIKE_R_PLUS_ID => "Strike_R",
        id if id == DEFEND_R_ID || id == DEFEND_R_PLUS_ID => "Defend_R",
        id if id == WARCRY_PLUS_ID => "Warcry",
        id if id == FLEX_PLUS_ID => "Flex",
        id if id == HAVOC_PLUS_ID => "Havoc",
        id if id == INFLAME_PLUS_ID => "Inflame",
        id if id == OFFERING_ID => "Offering",
        id if id == HAND_OF_GREED_ID || id == HAND_OF_GREED_PLUS_ID => "HandOfGreed",
        other => {
            let key = content_key(other);
            if key.ends_with('+') {
                key.trim_end_matches('+')
            } else {
                key
            }
        }
    }
}

fn reward_card_display_key(run: &RunState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::WARCRY_PLUS_ID;
    if content_id == WARCRY_PLUS_ID {
        return "Warcry+";
    }
    if let Some(upgraded) = egg_preview_upgrade(run, content_id) {
        return content_key(upgraded);
    }
    content_key(content_id)
}

fn reward_card_choice_display_key(run: &RunState, card: &CardInstance) -> String {
    if card.searing_blow_upgrades > 0 {
        return format!("Searing Blow+{}", card.searing_blow_upgrades);
    }
    if sts_core::content::cards::is_synthetic_any_color_content_id(card.content_id) {
        if let Some(key) = sts_core::run::reward::any_color_reward_card_key(card.content_id) {
            // CommunicationMod's choice_list uses American `judgment`; the
            // cardID / card_reward_ids field is `Judgement`.
            let mut label = if key == "JUDGEMENT" {
                "Judgment".to_owned()
            } else {
                key.replace('_', " ")
            };
            // Synthetic Prismatic cards can be upgraded by the matching Egg even
            // though they retain one content identity.
            if (card.upgrades > 0 || egg_preview_upgrade(run, card.content_id).is_some())
                && !label.ends_with('+')
            {
                label.push('+');
            }
            return label;
        }
    }
    // Ironclad / defined cards may also carry instance upgrades (Ritual Dagger).
    let mut key = reward_card_display_key(run, card.content_id).to_owned();
    if card.upgrades > 0 && !key.ends_with('+') {
        key.push('+');
    }
    key
}

fn egg_preview_upgrade(run: &RunState, content_id: ContentId) -> Option<ContentId> {
    let upgraded = upgrade_content_id(content_id).or_else(|| {
        sts_core::content::cards::is_synthetic_any_color_content_id(content_id)
            .then_some(content_id)
    })?;
    let (card_type, _) = card_type_and_rarity(content_id)?;
    let has_matching_egg = match card_type {
        CardType::Attack => run_has_relic_key(run, RelicKey::MoltenEgg),
        CardType::Skill => run_has_relic_key(run, RelicKey::ToxicEgg),
        CardType::Power => run_has_relic_key(run, RelicKey::FrozenEgg),
        CardType::Status => false,
    };
    has_matching_egg.then_some(upgraded)
}

fn choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd, index] if cmd.eq_ignore_ascii_case("CHOOSE") => index.parse().ok(),
        _ => None,
    }
}

fn action_source_command_execution_seq(action: &TraceAction) -> Option<u64> {
    action
        .command_meta
        .as_ref()?
        .get("source_command_execution_seq")?
        .as_u64()
}

fn action_source_command_settlement_seq(action: &TraceAction) -> Option<u64> {
    action
        .command_meta
        .as_ref()?
        .get("source_command_settlement_seq")?
        .as_u64()
}

fn action_command_id(action: &TraceAction) -> Option<&str> {
    action
        .command_meta
        .as_ref()?
        .get("command_id")?
        .as_str()
        .filter(|id| !id.is_empty())
}

fn validate_schema7_completion(
    action: &TraceAction,
    message: &Value,
    kind: &str,
    state_command: bool,
    last_execution: Option<u64>,
    last_settlement: Option<u64>,
) -> Result<(), SimRealError> {
    let command_id =
        action_command_id(action).ok_or_else(|| SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "schema-7 action {} requires command_meta.command_id",
                action.command
            ),
        })?;
    let source_execution = action_source_command_execution_seq(action).ok_or_else(|| {
        SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 action requires command_meta.source_command_execution_seq".to_owned(),
        }
    })?;
    let source_settlement = action_source_command_settlement_seq(action).ok_or_else(|| {
        SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 action requires command_meta.source_command_settlement_seq"
                .to_owned(),
        }
    })?;
    if last_execution.is_some_and(|previous| source_execution != previous) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "source_command_execution_seq {source_execution} does not match preceding completion sequence {}",
                last_execution.expect("checked")
            ),
        });
    }
    if last_settlement.is_some_and(|previous| source_settlement != previous) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "source_command_settlement_seq {source_settlement} does not match preceding completion sequence {}",
                last_settlement.expect("checked")
            ),
        });
    }
    let response_id = message.get("command_response_id").and_then(Value::as_str);
    let response_kind = message.get("command_response_kind").and_then(Value::as_str);
    let execution_seq = message
        .get("command_execution_seq")
        .and_then(Value::as_u64)
        .expect("schema-7 sequence validated");
    let settlement_seq = message
        .get("command_settlement_seq")
        .and_then(Value::as_u64)
        .expect("schema-7 settlement validated");
    if state_command {
        if kind != "poll" || response_kind != Some("poll") || response_id != Some(command_id) {
            return Err(SimRealError::InvalidBoundaryContract {
                step: action.step,
                reason: format!(
                    "schema-7 STATE completion identity mismatch for {}",
                    action.command
                ),
            });
        }
        if execution_seq != source_execution || settlement_seq != source_settlement {
            return Err(SimRealError::InvalidBoundaryContract {
                step: action.step,
                reason: format!(
                    "schema-7 STATE sequences must remain source execution {source_execution} and settlement {source_settlement}"
                ),
            });
        }
        return Ok(());
    }
    if !matches!(kind, "interaction_ready" | "quiescent" | "terminal")
        || response_kind != Some("settled")
        || response_id != Some(command_id)
    {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "schema-7 gameplay completion identity mismatch for {}",
                action.command
            ),
        });
    }
    let expected_execution =
        source_execution
            .checked_add(1)
            .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                step: action.step,
                reason: "schema-7 source command execution sequence overflow".to_owned(),
            })?;
    let expected_settlement =
        source_settlement
            .checked_add(1)
            .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                step: action.step,
                reason: "schema-7 source command settlement sequence overflow".to_owned(),
            })?;
    if execution_seq != expected_execution || settlement_seq != expected_settlement {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "schema-7 gameplay sequences must be source+1 (execution {source_execution} -> {expected_execution}, settlement {source_settlement} -> {expected_settlement})"
            ),
        });
    }
    Ok(())
}

fn validate_schema7_rejection(
    action: &TraceAction,
    message: &Value,
    last_execution: Option<u64>,
    last_settlement: Option<u64>,
) -> Result<(), SimRealError> {
    let command_id =
        action_command_id(action).ok_or_else(|| SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 command error requires command_meta.command_id".to_owned(),
        })?;
    let source_execution = action_source_command_execution_seq(action).ok_or_else(|| {
        SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 command error requires command_meta.source_command_execution_seq"
                .to_owned(),
        }
    })?;
    let source_settlement = action_source_command_settlement_seq(action).ok_or_else(|| {
        SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 command error requires command_meta.source_command_settlement_seq"
                .to_owned(),
        }
    })?;
    if last_execution.is_some_and(|previous| source_execution != previous) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "source_command_execution_seq {source_execution} does not match preceding completion sequence {}",
                last_execution.expect("checked")
            ),
        });
    }
    if last_settlement.is_some_and(|previous| source_settlement != previous) {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "source_command_settlement_seq {source_settlement} does not match preceding completion sequence {}",
                last_settlement.expect("checked")
            ),
        });
    }
    if message.get("boundary_schema").and_then(Value::as_u64) != Some(7)
        || message.get("command_response_id").and_then(Value::as_str) != Some(command_id)
        || message.get("command_response_kind").and_then(Value::as_str) != Some("rejected")
        || message.get("transaction_pending").and_then(Value::as_bool) != Some(false)
    {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 command error identity or transaction status mismatch".to_owned(),
        });
    }
    let execution_seq = message
        .get("command_execution_seq")
        .and_then(Value::as_u64)
        .ok_or_else(|| SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 command error requires command_execution_seq".to_owned(),
        })?;
    let settlement_seq = message
        .get("command_settlement_seq")
        .and_then(Value::as_u64)
        .ok_or_else(|| SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: "schema-7 command error requires command_settlement_seq".to_owned(),
        })?;
    let observation_command =
        command_head_eq(&action.command, "STATE") || command_head_eq(&action.command, "PROFILE");
    let expected_execution = if observation_command {
        source_execution
    } else {
        source_execution
            .checked_add(1)
            .ok_or_else(|| SimRealError::InvalidBoundaryContract {
                step: action.step,
                reason: "schema-7 source command execution sequence overflow".to_owned(),
            })?
    };
    if execution_seq != expected_execution || settlement_seq != source_settlement {
        return Err(SimRealError::InvalidBoundaryContract {
            step: action.step,
            reason: format!(
                "schema-7 rejection sequences must use expected execution {expected_execution} and unchanged settlement {source_settlement}"
            ),
        });
    }
    Ok(())
}

fn command_head_eq(command: &str, expected: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|head| head.eq_ignore_ascii_case(expected))
}

struct ParsedPotionUse {
    slot: usize,
    target: Option<MonsterId>,
}

fn seed_start_potion_command_target(
    run: &RunState,
    potion_use: &ParsedPotionUse,
) -> Option<MonsterId> {
    run.potion_at_slot(potion_use.slot)
        .is_some_and(Potion::requires_target)
        .then_some(potion_use.target)
        .flatten()
}

fn parse_potion_use(command: &str) -> Option<ParsedPotionUse> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [head, second, slot]
            if head.eq_ignore_ascii_case("POTION") && second.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: None,
            })
        }
        [head, second, slot, target]
            if head.eq_ignore_ascii_case("POTION") && second.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: Some(MonsterId::new(target.parse::<u64>().ok()? + 1)),
            })
        }
        [head, slot, target]
            if head.eq_ignore_ascii_case("potion") && !slot.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: Some(MonsterId::new(target.parse::<u64>().ok()? + 1)),
            })
        }
        [head, slot]
            if head.eq_ignore_ascii_case("potion") && !slot.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: None,
            })
        }
        _ => None,
    }
}

fn deck_keys_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .map(|card| {
            observed_card_projection_key(card)
                .expect("trace deck card schema was validated before projection")
        })
        .collect()
}

fn deck_content_keys(deck: &[CardInstance]) -> Vec<String> {
    deck.iter().map(simulated_card_projection_key).collect()
}

fn screen_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("screen_type"))
        .and_then(Value::as_str)
}

fn intent_key(monster: &MonsterState) -> String {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, BANDIT_BEAR_ID, BANDIT_LEADER_ID, BRONZE_ORB_ID, BYRD_ID, CHAMP_ID,
        CHOSEN_ID, EXPLODER_ID, GREMLIN_WIZARD_ID, GUARDIAN_ID, HEXAGHOST_ID, LAGAVULIN_ID,
        RED_LOUSE_ID, SLIME_BOSS_ID, SNECKO_ID, SPIKER_ID, SPIKE_SLIME_ID,
    };

    if monster.content_id == sts_core::content::monsters::TIME_EATER_ID {
        return match monster.intent {
            MonsterIntent::AttackAndBlock { .. } => "DEFEND_DEBUFF".to_owned(),
            MonsterIntent::Attack { .. } => "ATTACK_DEBUFF".to_owned(),
            MonsterIntent::StrengthSelf { amount: 0 } => "BUFF".to_owned(),
            MonsterIntent::AttackMultiple { .. } => "ATTACK".to_owned(),
            _ => "UNKNOWN".to_owned(),
        };
    }

    match monster.intent {
        // The target publishes event-combat setup while the initial AI action
        // is still queued; CommunicationMod labels that transient intent DEBUG.
        MonsterIntent::PendingAiRoll => "DEBUG".to_owned(),
        MonsterIntent::DarklingCount | MonsterIntent::AwakenedOneHalfDead => "UNKNOWN".to_owned(),
        MonsterIntent::Attack { .. }
        | MonsterIntent::AttackAddSlimedToDiscard { .. }
        | MonsterIntent::AttackAddWoundsToDiscard { .. }
        | MonsterIntent::AttackAddVoidToDraw { .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { .. }
        | MonsterIntent::AttackApplyPlayerFrail { .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
        | MonsterIntent::AttackApplyPlayerFrailAndVulnerable { .. }
        | MonsterIntent::AttackApplyPlayerWeak { .. }
        | MonsterIntent::AttackApplyPlayerVulnerable { .. }
        | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. }
        | MonsterIntent::AttackMultiple { .. }
        | MonsterIntent::AttackMultipleAddDazedToDiscard { .. }
        | MonsterIntent::AttackMultipleApplyPlayerWeak { .. }
        | MonsterIntent::AttackMultipleUpgradeBurns { .. }
        | MonsterIntent::AttackStealGold { .. } => {
            if monster.content_id == GUARDIAN_ID
                && matches!(
                    monster.intent,
                    MonsterIntent::AttackMultiple { hits: 2, .. }
                )
            {
                "ATTACK_BUFF".to_owned()
            } else if matches!(
                monster.intent,
                MonsterIntent::AttackApplyPlayerWeak { .. }
                    | MonsterIntent::AttackApplyPlayerVulnerable { .. }
                    | MonsterIntent::AttackApplyPlayerFrail { .. }
                    | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
                    | MonsterIntent::AttackApplyPlayerFrailAndVulnerable { .. }
                    | MonsterIntent::AttackAddSlimedToDiscard { .. }
                    | MonsterIntent::AttackAddWoundsToDiscard { .. }
                    | MonsterIntent::AttackAddVoidToDraw { .. }
                    | MonsterIntent::AddBurnToDiscardAndDraw { .. }
                    | MonsterIntent::AttackMultipleAddDazedToDiscard { .. }
                    | MonsterIntent::AttackMultipleApplyPlayerWeak { .. }
                    | MonsterIntent::AttackMultipleUpgradeBurns { .. }
                    | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. }
            ) {
                "ATTACK_DEBUFF".to_owned()
            } else {
                "ATTACK".to_owned()
            }
        }
        MonsterIntent::Block { .. } if monster.content_id == GREMLIN_WIZARD_ID => {
            "UNKNOWN".to_owned()
        }
        MonsterIntent::Block { .. } => "DEFEND".to_owned(),
        MonsterIntent::StrengthAndBlock { .. } if monster.content_id == RED_LOUSE_ID => {
            "BUFF".to_owned()
        }
        MonsterIntent::StrengthAndBlock { .. } if monster.content_id == SPIKER_ID => {
            "BUFF".to_owned()
        }
        MonsterIntent::StrengthAndBlock { .. } => "DEFEND_BUFF".to_owned(),
        MonsterIntent::StrengthSelf { .. } if monster.content_id == GREMLIN_WIZARD_ID => {
            "UNKNOWN".to_owned()
        }
        MonsterIntent::StrengthSelf { amount: 0 } if monster.content_id == BYRD_ID => {
            "UNKNOWN".to_owned()
        }
        MonsterIntent::Ritual { .. }
        | MonsterIntent::HealAllMonsters { .. }
        | MonsterIntent::StrengthSelf { .. }
        | MonsterIntent::StrengthAllMonsters { .. }
        | MonsterIntent::GuardianCloseUp { .. } => "BUFF".to_owned(),
        MonsterIntent::EncourageGremlins { .. } => "DEFEND_BUFF".to_owned(),
        MonsterIntent::AttackAndBlock { .. } => "ATTACK_DEFEND".to_owned(),
        MonsterIntent::AttackHealSelf { .. } => "ATTACK_BUFF".to_owned(),
        MonsterIntent::AddBurnToDiscard { damage, .. } if damage > 0 => "ATTACK_DEBUFF".to_owned(),
        MonsterIntent::ApplyPlayerFrailAndWeak { .. }
            if matches!(monster.content_id, ACID_SLIME_ID | SPIKE_SLIME_ID) =>
        {
            "DEBUFF".to_owned()
        }
        MonsterIntent::SiphonPlayer { .. }
            if matches!(
                monster.content_id,
                LAGAVULIN_ID | BRONZE_ORB_ID | BANDIT_BEAR_ID
            ) =>
        {
            "STRONG_DEBUFF".to_owned()
        }
        MonsterIntent::ApplyPlayerHex { .. } if monster.content_id == CHOSEN_ID => {
            "STRONG_DEBUFF".to_owned()
        }
        MonsterIntent::ApplyPlayerConfusion if monster.content_id == SNECKO_ID => {
            "STRONG_DEBUFF".to_owned()
        }
        MonsterIntent::ApplyPlayerWeak { .. }
        | MonsterIntent::ApplyPlayerHex { .. }
        | MonsterIntent::ApplyPlayerWeakStrengthSelf { .. }
        | MonsterIntent::ApplyPlayerConfusion
        | MonsterIntent::AddDazedToDiscard { .. }
        | MonsterIntent::AddDazedToDraw { .. }
        | MonsterIntent::AddBurnToDiscard { .. }
        | MonsterIntent::SiphonPlayer { .. } => "DEBUFF".to_owned(),
        MonsterIntent::ApplyPlayerFrailWeakVulnerable { .. } if monster.content_id == CHAMP_ID => {
            "DEBUFF".to_owned()
        }
        MonsterIntent::ApplyPlayerFrailAndWeak { .. }
        | MonsterIntent::ApplyPlayerFrailWeakVulnerable { .. }
        | MonsterIntent::ApplyPlayerConstricted { .. }
        | MonsterIntent::ApplyPlayerEntangled { .. } => "STRONG_DEBUFF".to_owned(),
        MonsterIntent::AddSlimedToDiscard { .. } if monster.content_id == SLIME_BOSS_ID => {
            "STRONG_DEBUFF".to_owned()
        }
        MonsterIntent::AddSlimedToDiscard { .. } => "DEBUFF".to_owned(),
        MonsterIntent::Sleep => "SLEEP".to_owned(),
        MonsterIntent::Stun
            if matches!(
                monster.content_id,
                EXPLODER_ID | SLIME_BOSS_ID | HEXAGHOST_ID | BANDIT_LEADER_ID
            ) =>
        {
            "UNKNOWN".to_owned()
        }
        MonsterIntent::Stun => "STUN".to_owned(),
        MonsterIntent::Escape => "ESCAPE".to_owned(),
        MonsterIntent::DefensiveCharge { .. }
        | MonsterIntent::SummonGremlins { .. }
        | MonsterIntent::SummonCollectorTorchHeads { .. } => "UNKNOWN".to_owned(),
    }
}

fn int(value: &Value, key: &str) -> i32 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

fn insert<T: Serialize>(map: &mut serde_json::Map<String, Value>, key: &str, value: T) {
    map.insert(
        key.to_owned(),
        serde_json::to_value(value).expect("json value"),
    );
}

#[cfg(test)]
mod tests;
