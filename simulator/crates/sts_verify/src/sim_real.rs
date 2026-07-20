//! CommunicationMod trace replay against the simulator for supported fields.

#[cfg(test)]
use crate::sts_seed_string_to_long;
use crate::{
    canonical_diff, import_communication_mod_trace, try_sts_seed_string_to_long, TraceAction,
    TraceLine, TraceState, VerificationIntegrity,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sts_core::card::CardType;
use sts_core::combat::{ExhaustSelectPurpose, HandSelectPurpose};
use sts_core::content::cards::{card_type_and_rarity, STRIKE_R_ID};
use sts_core::content::encounters::{
    target_beyond_act_three_boss_with_unlocks, target_exordium_act_one_boss_with_unlocks,
    BossUnlockState,
};
use sts_core::content::monsters::target_move_byte_for_monster;
use sts_core::potion::Potion;
use sts_core::run::event::{neow_screen_for_stage, VAMPIRES_BITE_COUNT};
use sts_core::run::neow::{
    apply_neow_curse_drawback, apply_neow_lament_reward,
    generate_neow_colorless_reward_with_card_rng_counter,
};
use sts_core::{
    affordable_shop_picks, apply_combat_action_on_run, apply_event_action, apply_map_action_on_run,
    apply_neow_boss_swap, apply_neow_relic_reward, apply_neow_simple_drawback,
    apply_neow_simple_reward, apply_rest_action, apply_run_action, cancel_grid, confirm_grid,
    consume_neow_three_potions_hidden_card_reward, generate_exordium_map_choices_after_path,
    generate_exordium_map_topology, generate_neow_card_reward, generate_neow_colorless_reward,
    generate_neow_options, generate_neow_three_potions, generate_neow_transform_reward,
    generate_target_map_choices_after_path, generate_target_map_topology, legal_map_actions_on_run,
    legal_rest_actions, open_neow_reward_grid, select_grid_card, shop_action_for_choice_index,
    target_room_kinds_on_path, Act1Boss, Act3Boss, CardGridScreen, CardId, CardInstance,
    CombatAction, CombatPhase, CombatState, ContentId, Event, EventAction, GeneratedNeowOption,
    GridPurpose, MonsterId, MonsterIntent, MonsterState, NeowDrawback, NeowRewardType, Relic,
    RelicKey, RestAction, RewardScreen, RoomKind, RunAction, RunPhase, RunState, ShopPick,
    TargetMapAct,
};

#[cfg(test)]
use crate::normalize_communication_mod_message;
#[cfg(test)]
use sts_core::content::monsters::{
    looter_theft, target_beyond_encounter_spawn_for_key,
    target_city_normal_encounter_spawn_at_combat_index, target_move_byte,
    target_normal_encounter_spawn_at_combat_index, TargetEncounterSpawn, TargetSpawnPower,
    GREMLIN_NOB_ID, GUARDIAN_CHARGE_BLOCK, GUARDIAN_ID, LAGAVULIN_ID, LOOTER_ID, MUGGER_ID,
    SLAVER_RED_ID, TASKMASTER_ID,
};
#[cfg(test)]
use sts_core::{
    city_room_kinds_on_path, enter_normal_combat_reward_screen, event_screen,
    exordium_room_kinds_on_path, initialize_combat_piles_with_relics, CardPiles, EventChoice,
    EventScreen, MonsterPowers, PlayerPowers, RelicCounters, StsRng,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimRealReport {
    pub mode: VerificationMode,
    pub total_actions: usize,
    pub ignored_tail_actions: usize,
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deferred_assertion_reconciled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionDispositionKind {
    Verified,
    Unsupported,
    UnexpectedDiff,
    IgnoredTail,
    ObservationPoll,
    FoldedTargetConfirmation,
    TargetRejected,
    Boundary,
    BeyondBoundary,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    SeedStart,
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
    pub rng_boundaries: Vec<RngBoundary>,
    pub m22_encounter_report: Option<crate::m22::M22EncounterReport>,
    #[serde(skip)]
    pub sim_run_state: Option<RunState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRunCommand {
    pub action_step: u32,
    pub character: String,
    pub ascension: u8,
    pub external_seed: String,
    pub numeric_seed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartBoundary {
    pub path: String,
    pub category: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RngBoundary {
    pub stream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_counter: Option<String>,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartVerifyOptions {}

#[derive(Debug)]
pub enum SimRealError {
    Trace(serde_json::Error),
    MissingStartCommand,
    MalformedStartCommand(String),
    MalformedChooseCommand { step: u32, command: String },
}

impl std::fmt::Display for SimRealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace(err) => write!(f, "{err}"),
            Self::MissingStartCommand => write!(f, "trace does not contain START command"),
            Self::MalformedStartCommand(command) => {
                write!(f, "malformed START command: {command}")
            }
            Self::MalformedChooseCommand { step, command } => write!(
                f,
                "malformed CHOOSE command at step {step}: {command}; expected exactly `CHOOSE <non-negative index>`"
            ),
        }
    }
}

impl std::error::Error for SimRealError {}

impl From<serde_json::Error> for SimRealError {
    fn from(value: serde_json::Error) -> Self {
        Self::Trace(value)
    }
}

pub fn verify_communication_mod_trace(content: &str) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_communication_mod_trace(content)
}

pub fn verify_seed_start_communication_mod_trace(
    content: &str,
) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_communication_mod_trace_with_options(
        content,
        SeedStartVerifyOptions::default(),
    )
}

pub fn verify_seed_start_communication_mod_trace_with_options(
    content: &str,
    options: SeedStartVerifyOptions,
) -> Result<SimRealReport, SimRealError> {
    verify_communication_mod_trace_with_mode_and_options(
        content,
        VerificationMode::SeedStart,
        options,
    )
}

pub fn verify_communication_mod_trace_with_mode(
    content: &str,
    mode: VerificationMode,
) -> Result<SimRealReport, SimRealError> {
    verify_communication_mod_trace_with_mode_and_options(
        content,
        mode,
        SeedStartVerifyOptions::default(),
    )
}

fn verify_communication_mod_trace_with_mode_and_options(
    content: &str,
    mode: VerificationMode,
    options: SeedStartVerifyOptions,
) -> Result<SimRealReport, SimRealError> {
    match mode {
        VerificationMode::SeedStart => verify_seed_start_trace(content, options),
    }
}

fn verify_seed_start_trace(
    content: &str,
    options: SeedStartVerifyOptions,
) -> Result<SimRealReport, SimRealError> {
    let trace = import_communication_mod_trace(content)?;
    let boss_unlocks = trace
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.boss_unlocks)
        .unwrap_or_default();
    let total_actions = trace
        .lines
        .iter()
        .filter(|line| matches!(line, TraceLine::Action(_)))
        .count();
    let transitions = trace_transitions(&trace.lines)?;
    let mut start = None;
    for (_, action, _) in &transitions.transitions {
        if let Some(parsed) = parse_start_command(action) {
            start = Some(parsed?);
            break;
        }
    }
    let start = start.ok_or(SimRealError::MissingStartCommand)?;

    let mut report = SimRealReport {
        mode: VerificationMode::SeedStart,
        total_actions,
        ignored_tail_actions: transitions.ignored_tail_actions,
        action_dispositions: Vec::new(),
        action_integrity: None,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let verification = verify_seed_start_transitions(
        &transitions.transitions,
        &start,
        &mut report,
        options,
        boss_unlocks,
    );
    let failed = verification.boundary.category != "none";
    let m22_encounter_report = Some(crate::m22::verify_m22_encounter_spawn_prefix(
        &trace.lines,
        &start.external_seed,
        start.numeric_seed,
        start.ascension,
    ));
    report.seed_start = Some(SeedStartReport {
        start_command: start,
        failed,
        first_boundary: verification.boundary,
        rng_boundaries: seed_start_rng_boundaries(),
        m22_encounter_report,
        sim_run_state: verification.final_run_state,
    });
    let (action_dispositions, action_integrity) = build_action_accounting(
        &trace.lines,
        &transitions,
        &report,
        &verification.reconciled_deferred_action_steps,
        verification.unresolved_transient_assertions,
    );
    report.action_dispositions = action_dispositions;
    report.action_integrity = Some(action_integrity);

    Ok(report)
}

struct TraceTransitions {
    transitions: Vec<(TraceState, TraceAction, TraceState)>,
    transition_action_ordinals: Vec<usize>,
    folded_action_dispositions: Vec<(usize, ActionDispositionKind)>,
    rejected_action_dispositions: Vec<(usize, String)>,
    ignored_action_ordinals: Vec<usize>,
    reconciled_deferred_action_ordinals: Vec<usize>,
    unresolved_transient_assertions: usize,
    ignored_tail_actions: usize,
}

struct PendingTraceAction {
    pre: TraceState,
    action: TraceAction,
    action_ordinal: usize,
    deferred_assertion: bool,
}

fn trace_transitions(lines: &[TraceLine]) -> Result<TraceTransitions, SimRealError> {
    let mut transitions = Vec::new();
    let mut transition_action_ordinals = Vec::new();
    let mut folded_action_dispositions = Vec::new();
    let mut rejected_action_dispositions = Vec::new();
    let mut ignored_action_ordinals = Vec::new();
    let mut reconciled_deferred_action_ordinals = Vec::new();
    let mut unresolved_transient_assertions = 0;
    let mut last_state: Option<TraceState> = None;
    let mut pending: Option<PendingTraceAction> = None;
    let mut next_action_ordinal = 0;
    for line in lines {
        match line {
            TraceLine::State(state) => {
                if let Some(mut pending_action) = pending.take() {
                    if is_delayed_map_choice(&pending_action.pre, &pending_action.action)
                        && screen_type(&state.message) == Some("MAP")
                        || is_unsettled_trace_action_state(
                            &pending_action.pre,
                            &pending_action.action,
                            state,
                        )
                        || is_mushrooms_fight_confirmation_state(
                            &pending_action.pre,
                            &pending_action.action,
                            state,
                        )
                        || is_cursed_key_chest_curse_pending_state(
                            &pending_action.pre,
                            &pending_action.action,
                            state,
                        )
                    {
                        pending_action.deferred_assertion = true;
                        pending = Some(pending_action);
                    } else {
                        if pending_action.deferred_assertion {
                            reconciled_deferred_action_ordinals.push(pending_action.action_ordinal);
                        }
                        transition_action_ordinals.push(pending_action.action_ordinal);
                        transitions.push((
                            pending_action.pre,
                            pending_action.action,
                            state.clone(),
                        ));
                    }
                }
                last_state = Some(state.clone());
            }
            TraceLine::Action(action) => {
                if command_head_eq(&action.command, "CHOOSE")
                    && command_choose_index(&action.command).is_none()
                {
                    return Err(SimRealError::MalformedChooseCommand {
                        step: action.step,
                        command: action.command.clone(),
                    });
                }
                let action_ordinal = next_action_ordinal;
                next_action_ordinal += 1;
                if let Some(pending_action) = pending.take() {
                    let pending_is_unsettled = last_state.as_ref().is_some_and(|state| {
                        is_unsettled_trace_action_state(
                            &pending_action.pre,
                            &pending_action.action,
                            state,
                        )
                    });
                    let pending_is_mushrooms_confirmation =
                        last_state.as_ref().is_some_and(|state| {
                            is_mushrooms_fight_confirmation_state(
                                &pending_action.pre,
                                &pending_action.action,
                                state,
                            )
                        });
                    let pending_is_cursed_key_chest = last_state.as_ref().is_some_and(|state| {
                        is_cursed_key_chest_curse_pending_state(
                            &pending_action.pre,
                            &pending_action.action,
                            state,
                        )
                    });
                    if (is_delayed_map_choice(&pending_action.pre, &pending_action.action)
                        || pending_is_unsettled
                        || pending_is_cursed_key_chest)
                        && is_trace_observation_poll(action)
                        || pending_is_mushrooms_confirmation
                            && command_choose_index(&action.command) == Some(0)
                    {
                        let disposition = if pending_is_mushrooms_confirmation
                            && command_choose_index(&action.command) == Some(0)
                        {
                            ActionDispositionKind::FoldedTargetConfirmation
                        } else {
                            ActionDispositionKind::ObservationPoll
                        };
                        folded_action_dispositions.push((action_ordinal, disposition));
                        pending = Some(pending_action);
                        continue;
                    }
                    if pending_action.deferred_assertion {
                        unresolved_transient_assertions += 1;
                    }
                    ignored_action_ordinals.push(pending_action.action_ordinal);
                }
                let pre = if let Some(pre) = last_state.clone() {
                    pre
                } else if parse_start_command(action).is_some() {
                    TraceState {
                        step: action.step,
                        received_at: None,
                        message: Value::Null,
                    }
                } else {
                    ignored_action_ordinals.push(action_ordinal);
                    continue;
                };
                pending = Some(PendingTraceAction {
                    pre,
                    action: action.clone(),
                    action_ordinal,
                    deferred_assertion: false,
                });
            }
            TraceLine::Error(error) => {
                if let Some(pending_action) = pending.take() {
                    if pending_action.action.step == error.step {
                        rejected_action_dispositions.push((
                            pending_action.action_ordinal,
                            serde_json::to_string(&error.message)
                                .unwrap_or_else(|_| "target rejected command".to_owned()),
                        ));
                    } else {
                        pending = Some(pending_action);
                    }
                }
            }
            TraceLine::Metadata(_) => {}
        }
    }
    if let Some(pending_action) = pending {
        if pending_action.deferred_assertion {
            unresolved_transient_assertions += 1;
        }
        ignored_action_ordinals.push(pending_action.action_ordinal);
    }
    let ignored_tail_actions = ignored_action_ordinals.len();
    Ok(TraceTransitions {
        transitions,
        transition_action_ordinals,
        folded_action_dispositions,
        rejected_action_dispositions,
        ignored_action_ordinals,
        reconciled_deferred_action_ordinals,
        unresolved_transient_assertions,
        ignored_tail_actions,
    })
}

fn build_action_accounting(
    lines: &[TraceLine],
    transitions: &TraceTransitions,
    report: &SimRealReport,
    semantic_reconciled_action_steps: &[u32],
    semantic_unresolved_transient_assertions: usize,
) -> (Vec<ActionDisposition>, VerificationIntegrity) {
    let actions = lines
        .iter()
        .filter_map(|line| match line {
            TraceLine::Action(action) => Some(action),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut dispositions: Vec<Option<ActionDisposition>> = vec![None; actions.len()];
    let mut reconciled = transitions
        .reconciled_deferred_action_ordinals
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    for (transition_index, ordinal) in transitions
        .transition_action_ordinals
        .iter()
        .copied()
        .enumerate()
    {
        if semantic_reconciled_action_steps
            .contains(&transitions.transitions[transition_index].1.step)
        {
            reconciled.insert(ordinal);
        }
    }
    let mut duplicate_dispositions = 0;

    for (ordinal, disposition) in &transitions.folded_action_dispositions {
        let detail = match disposition {
            ActionDispositionKind::ObservationPoll => {
                "observation poll folded into the pending semantic action"
            }
            ActionDispositionKind::FoldedTargetConfirmation => {
                "target-only confirmation folded into the pending semantic action"
            }
            _ => "folded trace action",
        };
        duplicate_dispositions += assign_action_disposition(
            &actions,
            &reconciled,
            &mut dispositions,
            *ordinal,
            *disposition,
            Some(detail.to_owned()),
        );
    }
    for ordinal in &transitions.ignored_action_ordinals {
        duplicate_dispositions += assign_action_disposition(
            &actions,
            &reconciled,
            &mut dispositions,
            *ordinal,
            ActionDispositionKind::IgnoredTail,
            Some("trace action has no settled post-state".to_owned()),
        );
    }
    for (ordinal, reason) in &transitions.rejected_action_dispositions {
        duplicate_dispositions += assign_action_disposition(
            &actions,
            &reconciled,
            &mut dispositions,
            *ordinal,
            ActionDispositionKind::TargetRejected,
            Some(reason.clone()),
        );
    }

    let mut used_verified = vec![false; report.verified.len()];
    let mut used_unsupported = vec![false; report.unsupported.len()];
    let mut used_diffs = vec![false; report.unexpected_diffs.len()];
    let boundary = report
        .seed_start
        .as_ref()
        .map(|seed_start| &seed_start.first_boundary);
    let boundary_step = boundary
        .filter(|boundary| boundary.category != "none")
        .and_then(|boundary| action_step_from_boundary_path(&boundary.path));
    let mut boundary_reached = false;

    for (transition_index, ordinal) in transitions
        .transition_action_ordinals
        .iter()
        .copied()
        .enumerate()
    {
        let action = &transitions.transitions[transition_index].1;
        let verified_matches = report
            .verified
            .iter()
            .enumerate()
            .filter(|(index, entry)| {
                !used_verified[*index]
                    && entry.action_step == action.step
                    && entry.command == action.command
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let unsupported_matches = report
            .unsupported
            .iter()
            .enumerate()
            .filter(|(index, entry)| {
                !used_unsupported[*index]
                    && entry.action_step == action.step
                    && entry.command == action.command
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let diff_matches = report
            .unexpected_diffs
            .iter()
            .enumerate()
            .filter(|(index, entry)| {
                !used_diffs[*index]
                    && entry.action_step == action.step
                    && entry.command == action.command
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let (disposition, detail) = if let Some(index) = diff_matches.first().copied() {
            used_diffs[index] = true;
            (
                ActionDispositionKind::UnexpectedDiff,
                Some(report.unexpected_diffs[index].label.clone()),
            )
        } else if let Some(index) = unsupported_matches.first().copied() {
            used_unsupported[index] = true;
            (
                ActionDispositionKind::Unsupported,
                Some(report.unsupported[index].reason.clone()),
            )
        } else if let Some(index) = verified_matches.first().copied() {
            used_verified[index] = true;
            (
                ActionDispositionKind::Verified,
                Some(report.verified[index].label.clone()),
            )
        } else if boundary_step == Some(action.step) && !boundary_reached {
            let boundary = boundary.expect("boundary step came from boundary");
            (
                ActionDispositionKind::Boundary,
                Some(format!("{}: {}", boundary.category, boundary.reason)),
            )
        } else if boundary_reached {
            (
                ActionDispositionKind::BeyondBoundary,
                Some("action follows the verifier boundary".to_owned()),
            )
        } else {
            (ActionDispositionKind::Unclassified, None)
        };

        duplicate_dispositions += assign_action_disposition(
            &actions,
            &reconciled,
            &mut dispositions,
            ordinal,
            disposition,
            detail,
        );
        if boundary_step == Some(action.step) && !boundary_reached {
            boundary_reached = true;
        }
    }

    let unmatched_report_dispositions = used_verified.iter().filter(|used| !**used).count()
        + used_unsupported.iter().filter(|used| !**used).count()
        + used_diffs.iter().filter(|used| !**used).count();
    duplicate_dispositions += unmatched_report_dispositions;

    for (ordinal, action) in actions.iter().enumerate() {
        if dispositions[ordinal].is_none() {
            dispositions[ordinal] = Some(ActionDisposition {
                action_ordinal: ordinal,
                action_step: action.step,
                command: action.command.clone(),
                disposition: ActionDispositionKind::Unclassified,
                detail: None,
                deferred_assertion_reconciled: reconciled.contains(&ordinal),
            });
        }
    }

    let dispositions = dispositions
        .into_iter()
        .map(|disposition| disposition.expect("every trace action receives a disposition"))
        .collect::<Vec<_>>();
    let rejected_actions = dispositions
        .iter()
        .filter(|entry| entry.disposition == ActionDispositionKind::TargetRejected)
        .count();
    let disposed_actions = dispositions
        .iter()
        .filter(|entry| {
            !matches!(
                entry.disposition,
                ActionDispositionKind::Unclassified | ActionDispositionKind::TargetRejected
            )
        })
        .count();
    let integrity = VerificationIntegrity {
        applicable_actions: actions.len() - rejected_actions,
        disposed_actions,
        duplicate_dispositions,
        unresolved_transient_assertions: transitions.unresolved_transient_assertions
            + semantic_unresolved_transient_assertions,
        terminal_state_observed: trace_terminal_state_observed(lines),
        rejected_actions,
    };
    (dispositions, integrity)
}

fn trace_terminal_state_observed(lines: &[TraceLine]) -> bool {
    let Some(message) = lines.iter().rev().find_map(|line| match line {
        TraceLine::State(state) => Some(&state.message),
        _ => None,
    }) else {
        return false;
    };
    message.get("in_game").and_then(Value::as_bool) == Some(false)
        || screen_type(message) == Some("GAME_OVER")
}

fn assign_action_disposition(
    actions: &[&TraceAction],
    reconciled: &std::collections::HashSet<usize>,
    dispositions: &mut [Option<ActionDisposition>],
    ordinal: usize,
    disposition: ActionDispositionKind,
    detail: Option<String>,
) -> usize {
    let Some(action) = actions.get(ordinal) else {
        return 1;
    };
    let entry = ActionDisposition {
        action_ordinal: ordinal,
        action_step: action.step,
        command: action.command.clone(),
        disposition,
        detail,
        deferred_assertion_reconciled: reconciled.contains(&ordinal),
    };
    usize::from(dispositions[ordinal].replace(entry).is_some())
}

fn action_step_from_boundary_path(path: &str) -> Option<u32> {
    let (_, suffix) = path.split_once("step=")?;
    let digits = suffix
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn is_delayed_map_choice(pre: &TraceState, action: &TraceAction) -> bool {
    screen_type(&pre.message) == Some("MAP") && command_choose_index(&action.command).is_some()
}

fn is_trace_observation_poll(action: &TraceAction) -> bool {
    action.command.eq_ignore_ascii_case("STATE") || action.command.eq_ignore_ascii_case("WAIT")
}

fn is_unsettled_trace_action_state(
    pre: &TraceState,
    action: &TraceAction,
    candidate: &TraceState,
) -> bool {
    if is_trace_observation_poll(action) {
        return false;
    }
    action.playtime_seconds.is_some()
        && same_trace_message_ignoring_playtime(&pre.message, &candidate.message)
        || trace_ready_for_command(&pre.message).is_some()
            && trace_ready_for_command(&candidate.message) == Some(false)
}

fn is_mushrooms_fight_confirmation_state(
    pre: &TraceState,
    action: &TraceAction,
    candidate: &TraceState,
) -> bool {
    command_choose_index(&action.command) == Some(0)
        && trace_event_id(&pre.message).is_some_and(|id| id.eq_ignore_ascii_case("Mushrooms"))
        && choice_list_from_value(pre.message.pointer("/game_state/choice_list"))
            .first()
            .is_some_and(|choice| choice.eq_ignore_ascii_case("stomp"))
        && trace_event_id(&candidate.message).is_some_and(|id| id.eq_ignore_ascii_case("Mushrooms"))
        && choice_list_from_value(candidate.message.pointer("/game_state/choice_list")).as_slice()
            == ["fight"]
}

fn is_cursed_key_chest_curse_pending_state(
    pre: &TraceState,
    action: &TraceAction,
    candidate: &TraceState,
) -> bool {
    command_choose_index(&action.command) == Some(0)
        && screen_type(&pre.message) == Some("CHEST")
        && trace_room_type(&pre.message) != Some("TreasureRoomBoss")
        && trace_relic_counter(&pre.message, &["Cursed Key", "CursedKey"]).is_some()
        && trace_relic_counter(&pre.message, &["Omamori"]).is_none_or(|counter| counter <= 0)
        && screen_type(&candidate.message) == Some("COMBAT_REWARD")
        && trace_deck_len(&pre.message)
            .zip(trace_deck_len(&candidate.message))
            .is_some_and(|(pre_len, candidate_len)| candidate_len <= pre_len)
}

fn trace_room_type(message: &Value) -> Option<&str> {
    message
        .pointer("/game_state/room_type")
        .and_then(Value::as_str)
}

fn trace_deck_len(message: &Value) -> Option<usize> {
    message
        .pointer("/game_state/deck")
        .and_then(Value::as_array)
        .map(Vec::len)
}

fn trace_relic_counter(message: &Value, aliases: &[&str]) -> Option<i64> {
    message
        .pointer("/game_state/relics")
        .and_then(Value::as_array)?
        .iter()
        .find(|relic| {
            relic
                .get("id")
                .or_else(|| relic.get("name"))
                .and_then(Value::as_str)
                .is_some_and(|relic| {
                    aliases
                        .iter()
                        .any(|alias| relic.eq_ignore_ascii_case(alias))
                })
        })
        .and_then(|relic| relic.get("counter"))
        .and_then(Value::as_i64)
}

fn trace_event_id(message: &Value) -> Option<&str> {
    message
        .pointer("/game_state/screen_state/event_id")
        .and_then(Value::as_str)
}

fn trace_ready_for_command(message: &Value) -> Option<bool> {
    message.get("ready_for_command").and_then(Value::as_bool)
}

fn same_trace_message_ignoring_playtime(source: &Value, candidate: &Value) -> bool {
    let (Some(source), Some(candidate)) = (source.as_object(), candidate.as_object()) else {
        return source == candidate;
    };
    source.len() == candidate.len()
        && source.iter().all(|(key, value)| {
            if key == "game_state" {
                candidate.get(key).is_some_and(|candidate| {
                    same_trace_game_state_ignoring_playtime(value, candidate)
                })
            } else {
                candidate.get(key) == Some(value)
            }
        })
}

fn same_trace_game_state_ignoring_playtime(source: &Value, candidate: &Value) -> bool {
    let (Some(source), Some(candidate)) = (source.as_object(), candidate.as_object()) else {
        return source == candidate;
    };
    let source_fields = source
        .keys()
        .filter(|key| key.as_str() != "playtime_seconds")
        .count();
    let candidate_fields = candidate
        .keys()
        .filter(|key| key.as_str() != "playtime_seconds")
        .count();
    source_fields == candidate_fields
        && source
            .iter()
            .all(|(key, value)| key == "playtime_seconds" || candidate.get(key) == Some(value))
}

fn recorded_action_playtime_seconds(pre: &TraceState, action: &TraceAction) -> Option<u32> {
    action.playtime_seconds.or_else(|| {
        pre.message
            .pointer("/game_state/playtime_seconds")
            .and_then(Value::as_f64)
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .map(|seconds| seconds.min(f64::from(u32::MAX)).floor() as u32)
    })
}

struct SeedStartVerification {
    boundary: SeedStartBoundary,
    final_run_state: Option<RunState>,
    reconciled_deferred_action_steps: Vec<u32>,
    unresolved_transient_assertions: usize,
}

struct PendingDeckAssertion {
    action: TraceAction,
    label: String,
    expected_deck: Vec<String>,
}

enum SmokeBombUiState {
    Escaping {
        source: Box<RunState>,
        action: TraceAction,
        transient_matches: bool,
    },
    Reward {
        pending_proceeds: Vec<TraceAction>,
    },
}

impl SmokeBombUiState {
    fn unresolved_assertions(&self) -> usize {
        match self {
            Self::Escaping { .. } => 1,
            Self::Reward { pending_proceeds } => pending_proceeds.len(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PendingDeckObservation {
    Settled,
    Deferred,
    Diverged(Vec<String>),
}

fn verify_seed_start_transitions(
    transitions: &[(TraceState, TraceAction, TraceState)],
    start: &StartRunCommand,
    report: &mut SimRealReport,
    _options: SeedStartVerifyOptions,
    boss_unlocks: BossUnlockState,
) -> SeedStartVerification {
    let mut phase = SeedStartPhase::BeforeStart;
    let mut _reward_step = 0usize;
    let mut combat_index = 0usize;
    let mut normal_combat_index = 0usize;
    let mut event_room_index = 0usize;
    let mut map_path_xs: Vec<i32> = Vec::new();
    let mut neow_gold = 99;
    let mut neow_current_hp = 80;
    let mut neow_max_hp = 80;
    let mut neow_card_reward_option: Option<GeneratedNeowOption> = None;
    let mut neow_card_reward_choices: Option<Vec<String>> = None;
    let mut neow_card_reward_card_rng_counter: Option<u32> = None;
    let mut neow_leave_visible_deck_ids: Option<Vec<String>> = None;
    let mut neow_potion_reward: Vec<String> = Vec::new();
    let mut neow_potion_rng_counter: Option<u32> = None;
    let mut neow_potions_taken = 0usize;
    let mut delayed_neow_curse: Option<String> = None;
    let mut delayed_neow_curse_before_last_deck_card = false;
    let mut pending_neow_room_entry_curse: Option<String> = None;
    let mut pending_neow_room_entry_curse_advances_card_rng = false;
    let mut delayed_neow_transform_count = 0usize;
    let mut relics = vec!["Burning Blood".to_owned()];
    let mut deck_ids = ironclad_starter_deck_keys();
    let mut seed_sim: Option<RunState> = None;
    let mut smoke_bomb_ui: Option<SmokeBombUiState> = None;
    let mut pending_deck_assertion: Option<PendingDeckAssertion> = None;
    let mut reconciled_deferred_action_steps = Vec::new();

    macro_rules! finish_boundary {
        ($boundary:expr) => {
            seed_start_finish_boundary(
                &seed_sim,
                $boundary,
                start.numeric_seed,
                boss_unlocks,
                reconciled_deferred_action_steps,
                usize::from(pending_deck_assertion.is_some())
                    + smoke_bomb_ui
                        .as_ref()
                        .map_or(0, SmokeBombUiState::unresolved_assertions),
            )
        };
    }

    for (pre, action, post) in transitions {
        if let Some(pending) = pending_deck_assertion.take() {
            if is_trace_observation_poll(action) {
                pending_deck_assertion = Some(pending);
            } else {
                let observed_deck = seed_start_observed_deck(&post.message);
                if observed_deck.starts_with(&pending.expected_deck) {
                    report.verified.push(VerifiedTransition {
                        action_step: pending.action.step,
                        command: pending.action.command,
                        label: pending.label,
                    });
                    reconciled_deferred_action_steps.push(pending.action.step);
                } else {
                    report.unexpected_diffs.push(UnexpectedDiff {
                        action_step: pending.action.step,
                        command: pending.action.command,
                        label: pending.label,
                        diffs: subset_diffs(json!(observed_deck), json!(pending.expected_deck)),
                    });
                }
            }
        }
        if let Some(sim) = seed_sim.as_mut() {
            seed_start_apply_boss_unlocks(sim, start.numeric_seed, boss_unlocks);
        }
        // Target event eligibility can depend on CardCrawlGame.playtime (Secret
        // Portal). This non-seeded clock is recorded as an explicit transition
        // input; deterministic gameplay state is never hydrated from observations.
        if let (Some(sim), Some(playtime_seconds)) = (
            seed_sim.as_mut(),
            recorded_action_playtime_seconds(pre, action),
        ) {
            sim.playtime_seconds = playtime_seconds;
        }
        if action.command.eq_ignore_ascii_case("state")
            || smoke_bomb_ui.is_some() && action.command.eq_ignore_ascii_case("wait")
        {
            if let Some(SmokeBombUiState::Escaping {
                source,
                action: escape_action,
                transient_matches,
            }) = smoke_bomb_ui.as_mut()
            {
                if screen_type(&post.message) == Some("NONE")
                    && post.message.pointer("/game_state/combat_state").is_some()
                {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb escape keeps its core destination");
                    *transient_matches &= seed_start_compare_deferred_combat_subset(
                        report,
                        escape_action,
                        "Smoke Bomb transient combat frame",
                        seed_start_smoke_bomb_transient_observed_subset(&post.message),
                        seed_start_smoke_bomb_transient_simulated_subset(source, destination),
                    );
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "Smoke Bomb transient observation poll".to_owned(),
                    });
                    continue;
                }
                if screen_type(&post.message) == Some("COMBAT_REWARD") {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb escape keeps its core destination");
                    let stable_matches = seed_start_compare_deferred_subset(
                        report,
                        escape_action,
                        "Smoke Bomb escape settled to empty reward",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(destination, &relics),
                    );
                    if *transient_matches && stable_matches {
                        report.verified.push(VerifiedTransition {
                            action_step: escape_action.step,
                            command: escape_action.command.clone(),
                            label: "Smoke Bomb escape reconciled at empty reward".to_owned(),
                        });
                        reconciled_deferred_action_steps.push(escape_action.step);
                    }
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "Smoke Bomb stable reward observation poll".to_owned(),
                    });
                    phase = SeedStartPhase::Reward;
                    smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                        pending_proceeds: Vec::new(),
                    });
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_smoke_bomb_ui_transition".to_owned(),
                    reason: format!(
                        "Smoke Bomb escape poll reached unsupported screen {:?}",
                        screen_type(&post.message)
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            if let Some(SmokeBombUiState::Reward { pending_proceeds }) = smoke_bomb_ui.as_ref() {
                if screen_type(&post.message) == Some("COMBAT_REWARD") {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb reward keeps its core destination");
                    compare_subset(
                        report,
                        action,
                        "Smoke Bomb empty reward observation poll",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(destination, &relics),
                    );
                    continue;
                }
                if !pending_proceeds.is_empty() && screen_type(&post.message) == Some("MAP") {
                    let pending_proceeds = pending_proceeds.clone();
                    let diff_count = report.unexpected_diffs.len();
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        start,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &mut map_path_xs,
                        &mut seed_sim,
                        &mut relics,
                        &mut deck_ids,
                    ) {
                        return finish_boundary!(boundary);
                    }
                    if report.unexpected_diffs.len() == diff_count {
                        for pending in pending_proceeds {
                            report.verified.push(VerifiedTransition {
                                action_step: pending.step,
                                command: pending.command,
                                label: "Smoke Bomb reward proceed reconciled at map".to_owned(),
                            });
                            reconciled_deferred_action_steps.push(pending.step);
                        }
                        smoke_bomb_ui = None;
                    }
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_smoke_bomb_ui_transition".to_owned(),
                    reason: format!(
                        "Smoke Bomb reward poll reached unsupported screen {:?}",
                        screen_type(&post.message)
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "trace client poll".to_owned(),
            });
            continue;
        }
        if action
            .command
            .split_whitespace()
            .next()
            .is_some_and(|head| head.eq_ignore_ascii_case("CLICK"))
            && pre
                .message
                .get("game_state")
                .and_then(|game| game.get("screen_name"))
                .and_then(Value::as_str)
                .is_some_and(|screen| screen.eq_ignore_ascii_case("FTUE"))
        {
            let Some(sim) = seed_sim.as_ref() else {
                return finish_boundary!(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_ftue_dismiss".to_owned(),
                    reason: "FTUE dismissal occurred without initialized deterministic replay"
                        .to_owned(),
                });
            };
            if sim.phase != RunPhase::Reward {
                return finish_boundary!(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_ftue_dismiss".to_owned(),
                    reason: format!(
                        "FTUE dismissal expected deterministic reward state, found {:?}",
                        sim.phase
                    ),
                });
            }
            compare_subset(
                report,
                action,
                "dismiss FTUE overlay",
                seed_start_reward_observed_subset(&post.message),
                seed_start_reward_simulated_subset(sim, &relics),
            );
            seed_start_test_pop_last_diff(report, action, &start.external_seed);
            phase = SeedStartPhase::Reward;
            continue;
        }
        if action
            .command
            .split_whitespace()
            .next()
            .is_some_and(|head| head.eq_ignore_ascii_case("KEY"))
            && action
                .command
                .split_whitespace()
                .nth(1)
                .is_some_and(|key| key.eq_ignore_ascii_case("CANCEL"))
            && pre
                .message
                .get("game_state")
                .and_then(|game| game.get("screen_name"))
                .and_then(Value::as_str)
                .is_some_and(|screen| screen.eq_ignore_ascii_case("MASTER_DECK_VIEW"))
            && phase == SeedStartPhase::Treasure
        {
            let Some(sim) = seed_sim.as_ref() else {
                return finish_boundary!(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_boss_reward_overlay".to_owned(),
                    reason:
                        "boss relic deck overlay closed without initialized deterministic replay"
                            .to_owned(),
                });
            };
            compare_subset(
                report,
                action,
                "close boss relic deck overlay",
                seed_start_treasure_observed_subset(&post.message),
                seed_start_treasure_simulated_subset(sim, &relics),
            );
            seed_start_test_pop_last_diff(report, action, &start.external_seed);
            continue;
        }
        match phase {
            SeedStartPhase::BeforeStart
                if action.command.eq_ignore_ascii_case(&format!(
                    "START {} {} {}",
                    start.character, start.ascension, start.external_seed
                )) =>
            {
                let mut observed = seed_start_observed_subset(&post.message);
                let mut simulated = json!({
                    "screen_type": "EVENT",
                    "ascension": start.ascension,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck_ids": deck_ids,
                    "relic_ids": relics,
                    "choices": ["talk"],
                });
                if let Some(observed_boss) = post
                    .message
                    .get("game_state")
                    .and_then(|game| game.get("act_boss"))
                    .and_then(Value::as_str)
                {
                    observed
                        .as_object_mut()
                        .expect("observed bootstrap subset is an object")
                        .insert("act_boss".to_owned(), json!(observed_boss));
                    simulated
                        .as_object_mut()
                        .expect("simulated bootstrap subset is an object")
                        .insert(
                            "act_boss".to_owned(),
                            json!(target_exordium_act_one_boss_with_unlocks(
                                start.numeric_seed,
                                boss_unlocks,
                            )),
                        );
                }
                compare_subset(report, action, "seed-start bootstrap", observed, simulated);
                phase = SeedStartPhase::NeowTalk;
            }
            SeedStartPhase::NeowTalk if command_is_choose(&action.command, 0) => {
                compare_subset(
                    report,
                    action,
                    "Neow talk",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": seed_start_neow_choices(start.numeric_seed),
                    }),
                );
                phase = SeedStartPhase::NeowOptions;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .and_then(seed_start_apply_neow_simple_option)
                    .is_some() =>
            {
                let (gold, current_hp, max_hp) = seed_start_apply_neow_simple_option(
                    seed_start_selected_neow_option(start.numeric_seed, &action.command)
                        .expect("matched generated simple Neow option"),
                )
                .expect("matched generated simple Neow option");
                neow_gold = gold;
                neow_current_hp = current_hp;
                neow_max_hp = max_hp;
                compare_subset(
                    report,
                    action,
                    "Neow simple immediate reward",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": gold,
                        "current_hp": current_hp,
                        "max_hp": max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_curse_simple) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated curse/simple Neow option");
                let mut run = seed_start_apply_neow_curse_simple_visible_option(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    option,
                );
                let mut curse_run = run.clone();
                let curse = apply_neow_curse_drawback(&mut curse_run);
                pending_neow_room_entry_curse = Some(deck_content_key(curse.curse).to_owned());
                pending_neow_room_entry_curse_advances_card_rng = false;
                run.card_rng_counter = curse.card_rng_counter;
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                seed_sim = Some(run);
                compare_subset(
                    report,
                    action,
                    "Neow curse immediate reward",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::TransformCard) =>
            {
                compare_subset(
                    report,
                    action,
                    "Neow transform grid",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "GRID",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["strike", "strike", "strike", "strike", "strike", "defend", "defend", "defend", "defend", "bash"],
                    }),
                );
                phase = SeedStartPhase::NeowTransformGrid;
            }
            SeedStartPhase::NeowTransformGrid if action.command.eq_ignore_ascii_case("PROCEED") => {
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: "captured trace sent PROCEED while Neow transform grid only accepted choose; classified as a trace-client command hiccup".to_owned(),
                });
            }
            SeedStartPhase::NeowTransformGrid if command_is_choose(&action.command, 0) => {
                compare_subset(
                    report,
                    action,
                    "Neow transform Strike select",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "GRID",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": [],
                    }),
                );
                phase = SeedStartPhase::NeowTransformConfirm;
            }
            SeedStartPhase::NeowTransformConfirm
                if action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let visible_deck_after_transform = ironclad_deck_after_transform_selection_keys();
                compare_subset(
                    report,
                    action,
                    "Neow transform confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": visible_deck_after_transform,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                deck_ids = seed_start_deck_after_transform(start.numeric_seed);
                neow_leave_visible_deck_ids = Some(visible_deck_after_transform);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::ThreeEnemyKill) =>
            {
                let mut run = seed_start_carried_run(
                    seed_sim.as_ref(),
                    start.numeric_seed,
                    start.ascension,
                    &start.external_seed,
                    &deck_ids,
                );
                apply_neow_lament_reward(&mut run);
                seed_sim = Some(run);
                relics.push("Neow's Lament".to_owned());
                compare_subset(
                    report,
                    action,
                    "Neow's Lament",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::OneRandomRareCard) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow random rare card option");
                let mut run = seed_start_apply_neow_reward_drawback_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                compare_subset(
                    report,
                    action,
                    seed_start_neow_card_reward_label(option.reward),
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                let reward = generate_neow_card_reward(start.numeric_seed, option.reward);
                neow_leave_visible_deck_ids = Some(deck_ids.clone());
                for content_id in reward.cards {
                    run.gain_deck_card(content_id);
                }
                deck_ids = deck_content_keys(&run.deck);
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_card_reward) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow card reward option");
                let run = seed_start_apply_neow_reward_drawback_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                neow_card_reward_choices = Some(seed_start_neow_card_reward_ids(
                    start.numeric_seed,
                    &option,
                    Some(&run),
                ));
                neow_card_reward_card_rng_counter = seed_start_neow_card_reward_card_rng_counter(
                    start.numeric_seed,
                    &option,
                    Some(&run),
                );
                neow_card_reward_option = Some(option.clone());
                if option.drawback == NeowDrawback::Curse {
                    let card_rng_counter = match option.reward {
                        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
                            generate_neow_colorless_reward(start.numeric_seed, option.reward)
                                .card_rng_counter
                        }
                        _ => 0,
                    };
                    delayed_neow_curse =
                        seed_start_neow_curse_deck_key(start.numeric_seed, card_rng_counter);
                }
                compare_subset(
                    report,
                    action,
                    seed_start_neow_card_reward_label(option.reward),
                    seed_start_reward_observed_subset(&post.message),
                    json!({
                        "screen_type": "CARD_REWARD",
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": seed_start_neow_card_reward_choice_names(start.numeric_seed, &option, Some(&run)),
                        "card_reward_ids": seed_start_neow_card_reward_id_values(start.numeric_seed, &option, Some(&run)),
                        "unobservable": {
                            "card_reward_rng_draws": true,
                            "card_reward_uuids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::NeowCardReward;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_relic_reward) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow relic reward option");
                let mut run = seed_start_apply_neow_relic_reward_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                let mut visible_deck_ids = deck_content_keys(&run.deck);
                if option.drawback == NeowDrawback::Curse {
                    if let Some(curse) = visible_deck_ids.pop() {
                        pending_neow_room_entry_curse = Some(curse);
                        pending_neow_room_entry_curse_advances_card_rng = false;
                        run.deck = deck_instances_from_keys(&visible_deck_ids);
                    }
                }
                deck_ids = visible_deck_ids;
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                let relic = seed_start_newest_trace_relic_name(&run);
                if !relics.contains(&relic) {
                    relics.push(relic.clone());
                }
                compare_subset(
                    report,
                    action,
                    seed_start_neow_relic_reward_label(option.reward),
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                if relic == "Toy Ornithopter" {
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: "Toy Ornithopter is only carried as a captured Neow relic in this trace; no potion-use transition is observed here, so potion-triggered healing remains covered by sts_core unit tests rather than seed-start trace parity".to_owned(),
                    });
                }
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::ThreeSmallPotions) =>
            {
                let reward = generate_neow_three_potions(start.numeric_seed);
                neow_potion_rng_counter = Some(reward.potion_rng_counter);
                neow_potion_reward = reward
                    .potions
                    .into_iter()
                    .map(|potion| potion_trace_name(potion).to_owned())
                    .collect();
                neow_potions_taken = 0;
                let mut run = seed_start_carried_run(
                    seed_sim.as_ref(),
                    start.numeric_seed,
                    start.ascension,
                    &start.external_seed,
                    &deck_ids,
                );
                run.gold = neow_gold;
                run.player_hp = neow_current_hp;
                run.player_max_hp = neow_max_hp;
                run.potions = neow_potion_reward
                    .iter()
                    .filter_map(|name| potion_from_trace_name(name))
                    .collect();
                if let Some(counter) = neow_potion_rng_counter {
                    run.potion_rng_counter = counter;
                }
                consume_neow_three_potions_hidden_card_reward(&mut run);
                seed_sim = Some(run);
                if screen_type(&post.message) == Some("EVENT") {
                    compare_subset(
                        report,
                        action,
                        "Neow three potion reward",
                        seed_start_potion_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": 99,
                            "current_hp": 80,
                            "max_hp": 80,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "potion_ids": neow_potion_reward,
                            "choices": ["leave"],
                            "unobservable": {
                                "potion_reward_uuids": true,
                            },
                        }),
                    );
                    phase = SeedStartPhase::NeowLeave;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow three potion reward",
                    seed_start_reward_observed_subset(&post.message),
                    json!({
                        "screen_type": "COMBAT_REWARD",
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["potion", "potion", "potion"],
                        "reward_types": ["POTION", "POTION", "POTION"],
                        "unobservable": {
                            "reward_gold_rng_draws": true,
                            "reward_screen_internal_ids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::NeowPotionReward;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_boss_swap) =>
            {
                let run = seed_start_apply_neow_boss_swap(start.numeric_seed, &deck_ids);
                if seed_start_boss_swap_is_calling_bell_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Calling Bell grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapCallingBellGrid;
                    continue;
                }
                if seed_start_boss_swap_is_astrolabe_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Astrolabe grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapAstrolabeGrid;
                    continue;
                }
                if seed_start_boss_swap_is_pandoras_box_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Pandora's Box grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapPandorasBoxGrid;
                    continue;
                }
                if seed_start_boss_swap_is_empty_cage_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Empty Cage grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapEmptyCageGrid;
                    continue;
                }
                if seed_start_boss_swap_is_tiny_house_reward(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Tiny House reward",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(&run, &relic_ids),
                    );
                    deck_ids = deck_content_keys(&run.deck);
                    neow_gold = run.gold;
                    neow_current_hp = run.player_hp;
                    neow_max_hp = run.player_max_hp;
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::Reward;
                    continue;
                }
                if let Some(reason) = seed_start_unsupported_boss_swap_reason(&run) {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason,
                    });
                }
                let relic_ids = seed_start_boss_swap_relic_ids(&run);
                let post_deck_ids = deck_content_keys(&run.deck);
                compare_subset(
                    report,
                    action,
                    "Neow boss swap",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": post_deck_ids,
                        "relic_ids": relic_ids,
                        "choices": ["leave"],
                    }),
                );
                deck_ids = post_deck_ids;
                relics = relic_ids;
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_grid_reward) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow grid option");
                let mut run = seed_start_open_neow_grid_run_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                if option.drawback == NeowDrawback::Curse {
                    let mut curse_run = run.clone();
                    let curse = apply_neow_curse_drawback(&mut curse_run);
                    delayed_neow_curse = Some(deck_content_key(curse.curse).to_owned());
                    run.card_rng_counter = curse.card_rng_counter;
                    delayed_neow_transform_count = match option.reward {
                        NeowRewardType::TransformCard => 1,
                        NeowRewardType::TransformTwoCards => 2,
                        _ => 0,
                    };
                }
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                compare_subset(
                    report,
                    action,
                    seed_start_neow_grid_label(option.reward),
                    seed_start_grid_observed_subset(&post.message),
                    seed_start_grid_simulated_subset(&run, &relics),
                );
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowGrid;
            }
            SeedStartPhase::NeowGrid if command_choose_index(&action.command).is_some() => {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid choose simulation failed".to_owned(),
                    });
                };
                let observed_grid = seed_start_grid_observed_subset(&post.message);
                let selected_subset = if delayed_neow_transform_count > 0 {
                    let mut visible_deck_ids = deck_content_keys(&next.deck);
                    if let Some(curse) = delayed_neow_curse.as_deref() {
                        visible_deck_ids.push(curse.to_owned());
                    }
                    seed_start_grid_simulated_subset_with_deck(&next, &relics, visible_deck_ids)
                } else {
                    seed_start_grid_simulated_subset(&next, &relics)
                };
                if subset_diffs(observed_grid.clone(), selected_subset.clone()).is_empty() {
                    compare_subset(
                        report,
                        action,
                        "Neow grid select",
                        observed_grid,
                        selected_subset,
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::NeowGridConfirm;
                    continue;
                }
                if let Ok(confirmed) = confirm_grid(&next) {
                    deck_ids = deck_content_keys(&confirmed.deck);
                    let confirmed_deck_ids = deck_ids.clone();
                    let transform_count_before_confirm =
                        seed_start_neow_grid_transform_count(&next)
                            .unwrap_or(delayed_neow_transform_count);
                    let mut visible_deck_ids = deck_ids.clone();
                    if transform_count_before_confirm > 0 {
                        visible_deck_ids = seed_start_visible_deck_after_neow_transform_selection(
                            &deck_ids,
                            transform_count_before_confirm,
                            delayed_neow_curse.as_deref(),
                        );
                    }
                    if delayed_neow_transform_count > 0 {
                        deck_ids = visible_deck_ids.clone();
                        let transformed_start = confirmed_deck_ids
                            .len()
                            .saturating_sub(delayed_neow_transform_count);
                        deck_ids.extend(confirmed_deck_ids[transformed_start..].iter().cloned());
                        if let Some(curse) = delayed_neow_curse.take() {
                            if !deck_ids.contains(&curse) {
                                deck_ids.push(curse);
                            }
                        }
                        delayed_neow_transform_count = 0;
                    }
                    let mut carried_confirmed = confirmed.clone();
                    if transform_count_before_confirm > 0 {
                        carried_confirmed.deck = deck_instances_from_keys(&deck_ids);
                    }
                    if confirmed.card_grid.is_some() {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            observed_grid,
                            seed_start_grid_simulated_subset(&confirmed, &relics),
                        );
                        seed_sim = Some(carried_confirmed);
                        phase = SeedStartPhase::NeowGrid;
                        continue;
                    }
                    if screen_type(&post.message) == Some("MAP") {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "MAP",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": neow_gold,
                                "current_hp": neow_current_hp,
                                "max_hp": neow_max_hp,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": seed_start_first_map_choices(&start.external_seed),
                            }),
                        );
                        seed_sim = Some(carried_confirmed);
                        phase = SeedStartPhase::Map;
                        continue;
                    }
                    if transform_count_before_confirm > 0 {
                        neow_leave_visible_deck_ids = Some(visible_deck_ids.clone());
                    }
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": neow_gold,
                            "current_hp": neow_current_hp,
                            "max_hp": neow_max_hp,
                            "deck_ids": visible_deck_ids,
                            "relic_ids": relics,
                            "choices": ["leave"],
                        }),
                    );
                    seed_sim = Some(carried_confirmed);
                    phase = SeedStartPhase::NeowLeave;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow grid select",
                    observed_grid,
                    selected_subset,
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowGridConfirm;
            }
            SeedStartPhase::NeowGridConfirm if action.command.eq_ignore_ascii_case("CONFIRM") => {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid confirm without initialized run simulation"
                            .to_owned(),
                    });
                };
                let Ok(next) = confirm_grid(sim) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid confirm simulation failed".to_owned(),
                    });
                };
                let had_delayed_transform = delayed_neow_transform_count > 0;
                deck_ids = deck_content_keys(&next.deck);
                if delayed_neow_transform_count > 0 {
                    for _ in 0..delayed_neow_transform_count.min(deck_ids.len()) {
                        deck_ids.pop();
                    }
                    if let Some(curse) = delayed_neow_curse.take() {
                        deck_ids.push(curse);
                    }
                    delayed_neow_transform_count = 0;
                }
                let mut carried_next = next.clone();
                if had_delayed_transform {
                    carried_next.deck = deck_instances_from_keys(&deck_ids);
                }
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(carried_next);
                    phase = SeedStartPhase::NeowGrid;
                    continue;
                } else {
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": neow_gold,
                            "current_hp": neow_current_hp,
                            "max_hp": neow_max_hp,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "choices": ["leave"],
                        }),
                    );
                }
                seed_sim = Some(carried_next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowGridConfirm
                if command_choose_index(&action.command).is_some()
                    && seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_neow_multi_select_grid) =>
            {
                let sim = seed_sim
                    .as_ref()
                    .expect("matched initialized Neow multi-select grid");
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow multi-select grid choose simulation failed"
                            .to_owned(),
                    });
                };
                let delayed_transform_count_before_confirm = delayed_neow_transform_count;
                let delayed_curse_before_confirm = delayed_neow_curse.clone();
                deck_ids = deck_content_keys(&next.deck);
                if delayed_neow_transform_count > 0 {
                    for _ in 0..delayed_neow_transform_count.min(deck_ids.len()) {
                        deck_ids.pop();
                    }
                    if let Some(curse) = delayed_neow_curse.take() {
                        deck_ids.push(curse);
                    }
                    delayed_neow_transform_count = 0;
                }
                if let Ok(confirmed) = confirm_grid(&next) {
                    deck_ids = deck_content_keys(&confirmed.deck);
                    let confirmed_deck_ids = deck_ids.clone();
                    let transform_count_before_confirm =
                        seed_start_neow_grid_transform_count(&next)
                            .unwrap_or(delayed_transform_count_before_confirm);
                    let mut visible_deck_ids = deck_ids.clone();
                    if transform_count_before_confirm > 0 {
                        visible_deck_ids = seed_start_visible_deck_after_neow_transform_selection(
                            &deck_ids,
                            transform_count_before_confirm,
                            delayed_curse_before_confirm.as_deref(),
                        );
                    }
                    if delayed_transform_count_before_confirm > 0 {
                        deck_ids = visible_deck_ids.clone();
                        let transformed_start = confirmed_deck_ids
                            .len()
                            .saturating_sub(delayed_transform_count_before_confirm);
                        deck_ids.extend(confirmed_deck_ids[transformed_start..].iter().cloned());
                        if let Some(curse) = delayed_curse_before_confirm {
                            if !deck_ids.contains(&curse) {
                                deck_ids.push(curse);
                            }
                        }
                    }
                    let mut carried_confirmed = confirmed.clone();
                    if transform_count_before_confirm > 0 {
                        carried_confirmed.deck = deck_instances_from_keys(&deck_ids);
                    }
                    if screen_type(&post.message) == Some("MAP") {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "MAP",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": neow_gold,
                                "current_hp": neow_current_hp,
                                "max_hp": neow_max_hp,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": seed_start_first_map_choices(&start.external_seed),
                            }),
                        );
                        seed_sim = Some(carried_confirmed);
                        phase = SeedStartPhase::Map;
                        continue;
                    }
                    if confirmed.card_grid.is_none() {
                        if transform_count_before_confirm > 0 {
                            neow_leave_visible_deck_ids = Some(visible_deck_ids.clone());
                        }
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "EVENT",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": neow_gold,
                                "current_hp": neow_current_hp,
                                "max_hp": neow_max_hp,
                                "deck_ids": visible_deck_ids,
                                "relic_ids": relics,
                                "choices": ["leave"],
                            }),
                        );
                        seed_sim = Some(carried_confirmed);
                        phase = SeedStartPhase::NeowLeave;
                        continue;
                    }
                }
                if next.card_grid.is_some() {
                    let simulated = if delayed_neow_transform_count > 0 {
                        let mut visible_deck_ids = deck_content_keys(&next.deck);
                        if let Some(curse) = delayed_neow_curse.as_deref() {
                            visible_deck_ids.push(curse.to_owned());
                        }
                        seed_start_grid_simulated_subset_with_deck(&next, &relics, visible_deck_ids)
                    } else {
                        seed_start_grid_simulated_subset(&next, &relics)
                    };
                    compare_subset(
                        report,
                        action,
                        "Neow grid select",
                        seed_start_grid_observed_subset(&post.message),
                        simulated,
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::NeowGridConfirm;
                    continue;
                }
                if screen_type(&post.message) == Some("MAP") {
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "MAP",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": neow_gold,
                            "current_hp": neow_current_hp,
                            "max_hp": neow_max_hp,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "choices": seed_start_first_map_choices(&start.external_seed),
                        }),
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::Map;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow grid confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowBossSwapCallingBellGrid
                if action.command.eq_ignore_ascii_case("PROCEED")
                    || action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Calling Bell boss-swap grid without initialized run simulation"
                                .to_owned(),
                    });
                };
                let Ok(next) = confirm_grid(sim) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Calling Bell boss-swap grid confirm failed".to_owned(),
                    });
                };
                deck_ids = deck_content_keys(&next.deck);
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Calling Bell rewards",
                    seed_start_reward_observed_subset(&post.message),
                    seed_start_reward_simulated_subset(&next, &relics),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowBossSwapCallingBellReward;
            }
            SeedStartPhase::NeowBossSwapCallingBellReward
                if command_choose_index(&action.command).is_some() =>
            {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Calling Bell boss-swap reward without initialized run simulation"
                                .to_owned(),
                    });
                };
                let label = match seed_start_apply_reward_choose(sim, &action.command, &pre.message)
                {
                    Ok(label) => label,
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_neow_boss_swap".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                };
                seed_start_update_carry_from_run(sim, &mut relics, &mut deck_ids);
                if seed_start_reward_sequence_complete(sim) {
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(sim, &relics),
                    );
                    phase = SeedStartPhase::Reward;
                } else {
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(sim, &relics),
                    );
                }
            }
            SeedStartPhase::NeowBossSwapAstrolabeGrid
                if command_choose_index(&action.command).is_some() =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Astrolabe boss-swap grid without initialized run simulation"
                                .to_owned(),
                    });
                };
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Astrolabe boss-swap grid choose failed".to_owned(),
                    });
                };
                deck_ids = deck_content_keys(&next.deck);
                if let Ok(confirmed) = confirm_grid(&next) {
                    deck_ids = deck_content_keys(&confirmed.deck);
                    if confirmed.card_grid.is_none() {
                        compare_subset(
                            report,
                            action,
                            "Neow boss swap Astrolabe transformed",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "EVENT",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": 99,
                                "current_hp": 80,
                                "max_hp": 80,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": ["leave"],
                            }),
                        );
                        seed_sim = Some(confirmed);
                        phase = SeedStartPhase::NeowLeave;
                        continue;
                    }
                }
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Astrolabe grid select",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(next);
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Astrolabe transformed",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowBossSwapPandorasBoxGrid
                if action.command.eq_ignore_ascii_case("PROCEED")
                    || action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Pandora's Box boss-swap grid without initialized run simulation"
                                .to_owned(),
                    });
                };
                let Ok(next) = confirm_grid(sim) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Pandora's Box boss-swap grid confirm failed".to_owned(),
                    });
                };
                deck_ids = deck_content_keys(&next.deck);
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Pandora's Box confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowBossSwapEmptyCageGrid
                if command_choose_index(&action.command).is_some() =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Empty Cage boss-swap grid without initialized run simulation"
                                .to_owned(),
                    });
                };
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Empty Cage boss-swap grid choose failed".to_owned(),
                    });
                };
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Empty Cage grid select",
                    seed_start_grid_observed_subset(&post.message),
                    seed_start_grid_simulated_subset(&next, &relics),
                );
                seed_sim = Some(next);
            }
            SeedStartPhase::NeowBossSwapEmptyCageGrid
                if action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Empty Cage boss-swap grid without initialized run simulation"
                                .to_owned(),
                    });
                };
                let Ok(next) = confirm_grid(sim) else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Empty Cage boss-swap grid confirm failed".to_owned(),
                    });
                };
                deck_ids = deck_content_keys(&next.deck);
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Empty Cage grid confirm",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(next);
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Empty Cage confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowCardReward
                if seed_start_pick_neow_card_reward(&neow_card_reward_choices, &action.command)
                    .is_some() =>
            {
                let picked_card =
                    seed_start_pick_neow_card_reward(&neow_card_reward_choices, &action.command)
                        .expect("matched generated Neow card reward pick");
                let option = neow_card_reward_option
                    .as_ref()
                    .expect("Neow card reward option is carried");
                let pre_pick_deck_ids = deck_ids.clone();
                deck_ids.push(picked_card.clone());
                let mut run = seed_start_apply_neow_reward_drawback_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    option,
                );
                if let Some(card_rng_counter) = neow_card_reward_card_rng_counter {
                    run.card_rng_counter = card_rng_counter;
                }
                if delayed_neow_curse.is_some() {
                    let observed_deck_ids = post
                        .message
                        .get("game_state")
                        .and_then(|game| game.get("deck"))
                        .map(|deck| deck_keys_from_value(Some(deck)))
                        .unwrap_or_default();
                    let curse_now_deck_ids = delayed_neow_curse.as_ref().map(|curse| {
                        let mut ids = pre_pick_deck_ids.clone();
                        ids.push(curse.clone());
                        ids.push(picked_card.clone());
                        ids
                    });
                    if curse_now_deck_ids
                        .as_ref()
                        .is_some_and(|ids| *ids == observed_deck_ids)
                    {
                        deck_ids = curse_now_deck_ids.expect("checked Some");
                        delayed_neow_curse = None;
                        run.deck = deck_instances_from_keys(&deck_ids);
                        run.card_rng_counter = run.card_rng_counter.saturating_add(1);
                    } else {
                        delayed_neow_curse_before_last_deck_card = false;
                    }
                }
                seed_sim = Some(run);
                compare_subset(
                    report,
                    action,
                    "Neow colorless pickup",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowPotionReward if command_is_choose(&action.command, 0) => {
                neow_potions_taken += 1;
                let remaining = neow_potion_reward.len().saturating_sub(neow_potions_taken);
                compare_subset(
                    report,
                    action,
                    &format!("Neow potion reward pick {neow_potions_taken}"),
                    seed_start_potion_observed_subset(&post.message),
                    json!({
                        "screen_type": "COMBAT_REWARD",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "potion_ids": neow_potion_reward
                            .iter()
                            .take(neow_potions_taken)
                            .cloned()
                            .collect::<Vec<_>>(),
                        "choices": vec!["potion"; remaining],
                        "unobservable": {
                            "potion_reward_uuids": true,
                        },
                    }),
                );
            }
            SeedStartPhase::NeowPotionReward if action.command.eq_ignore_ascii_case("PROCEED") => {
                if neow_potions_taken < neow_potion_reward.len() {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_potion_reward".to_owned(),
                        reason: "seed-start verifier expected all Neow potion rewards to be picked before PROCEED".to_owned(),
                    });
                }
                compare_subset(
                    report,
                    action,
                    "Neow potion reward proceed",
                    seed_start_potion_observed_subset(&post.message),
                    json!({
                        "screen_type": "MAP",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "potion_ids": neow_potion_reward,
                        "choices": seed_start_first_map_choices(&start.external_seed),
                        "unobservable": {
                            "potion_reward_uuids": true,
                        },
                    }),
                );
                if seed_sim.is_none() {
                    let mut run = seed_start_carried_run(
                        None,
                        start.numeric_seed,
                        start.ascension,
                        &start.external_seed,
                        &deck_ids,
                    );
                    run.gold = neow_gold;
                    run.player_hp = neow_current_hp;
                    run.player_max_hp = neow_max_hp;
                    run.potions = neow_potion_reward
                        .iter()
                        .filter_map(|name| potion_from_trace_name(name))
                        .collect();
                    if let Some(counter) = neow_potion_rng_counter {
                        run.potion_rng_counter = counter;
                    }
                    consume_neow_three_potions_hidden_card_reward(&mut run);
                    seed_sim = Some(run);
                }
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::NeowLeave if command_is_choose(&action.command, 0) => {
                if let Some(curse) = delayed_neow_curse.take() {
                    pending_neow_room_entry_curse = Some(curse);
                    pending_neow_room_entry_curse_advances_card_rng = true;
                }
                let initialized_seed_sim = seed_sim.is_none();
                if seed_sim.is_none() {
                    let mut run =
                        seed_start_seeded_idle_run(start.numeric_seed, start.ascension, &deck_ids);
                    run.gold = neow_gold;
                    run.player_hp = neow_current_hp;
                    run.player_max_hp = neow_max_hp;
                    seed_sim = Some(run);
                }
                if let Some(sim) = seed_sim.as_mut() {
                    sim.phase = RunPhase::Idle;
                    sim.event = None;
                    sim.reward = None;
                    sim.card_grid = None;
                    if initialized_seed_sim {
                        sim.deck = deck_instances_from_keys(&deck_ids);
                    }
                }
                let lagged_visible_deck = neow_leave_visible_deck_ids.take();
                let observed_deck_ids = post
                    .message
                    .get("game_state")
                    .and_then(|game| game.get("deck"))
                    .map(|deck| deck_keys_from_value(Some(deck)))
                    .unwrap_or_default();
                let visible_deck = match lagged_visible_deck {
                    Some(lagged) if observed_deck_ids == lagged => lagged,
                    _ => deck_ids.clone(),
                };
                let visible_deck = if let Some(curse) = pending_neow_room_entry_curse.as_ref() {
                    let mut curse_visible_deck = visible_deck.clone();
                    let delayed_tail = delayed_neow_curse_before_last_deck_card
                        .then(|| curse_visible_deck.pop())
                        .flatten();
                    curse_visible_deck.push(curse.clone());
                    if let Some(card) = delayed_tail {
                        curse_visible_deck.push(card);
                    }
                    if observed_deck_ids == curse_visible_deck {
                        curse_visible_deck
                    } else {
                        visible_deck
                    }
                } else {
                    visible_deck
                };
                compare_subset(
                    report,
                    action,
                    "Neow leave",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "MAP",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": visible_deck,
                        "relic_ids": relics,
                        "choices": seed_start_first_map_choices(&start.external_seed),
                    }),
                );
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::Map
                if screen_type(&pre.message) == Some("MAP")
                    && command_choose_index(&action.command).is_some() =>
            {
                if let Some(sim) = seed_sim.as_ref() {
                    let mut transition_base = sim.clone();
                    seed_start_apply_boss_unlocks(
                        &mut transition_base,
                        start.numeric_seed,
                        boss_unlocks,
                    );
                    if let Some(curse) = pending_neow_room_entry_curse.take() {
                        let mut next_deck_ids = deck_content_keys(&transition_base.deck);
                        let delayed_tail = delayed_neow_curse_before_last_deck_card
                            .then(|| next_deck_ids.pop())
                            .flatten();
                        next_deck_ids.push(curse);
                        if let Some(card) = delayed_tail {
                            next_deck_ids.push(card);
                        }
                        if pending_neow_room_entry_curse_advances_card_rng {
                            transition_base.card_rng_counter =
                                transition_base.card_rng_counter.saturating_add(1);
                        }
                        pending_neow_room_entry_curse_advances_card_rng = false;
                        delayed_neow_curse_before_last_deck_card = false;
                        transition_base.deck = deck_instances_from_keys(&next_deck_ids);
                        deck_ids = next_deck_ids;
                    }
                    let legal_actions = legal_map_actions_on_run(&transition_base);
                    if let Some(choice_index) = choose_index(&action.command) {
                        if let Some(map_action) = legal_actions.get(choice_index).copied() {
                            let choice_x = transition_base
                                .map
                                .as_ref()
                                .and_then(|map_state| {
                                    let node_id = match map_action {
                                        sts_core::MapAction::ChooseNode { node_id } => node_id,
                                    };
                                    map_state.map.node(node_id).map(|node| {
                                        let (x, _) = seed_start_map_node_xy(node.id);
                                        x
                                    })
                                })
                                .unwrap_or_else(|| {
                                    seed_start_map_pick_x(
                                        &start.external_seed,
                                        &map_path_xs,
                                        &action.command,
                                    )
                                });
                            map_path_xs.push(choice_x);
                            let Ok(next) = apply_map_action_on_run(&transition_base, map_action)
                            else {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "unsupported_map_path".to_owned(),
                                    reason: "core map simulation rejected transition".to_owned(),
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            };
                            match next.phase {
                                RunPhase::Event => {
                                    let label = format!("map event node {}", event_room_index + 1);
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_event_observed_subset(&post.message),
                                        seed_start_event_simulated_subset(&next, &relics),
                                    );
                                    event_room_index += 1;
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Event;
                                }
                                RunPhase::Combat => {
                                    let label = seed_start_map_label(normal_combat_index);
                                    let observed =
                                        seed_start_encounter_observed_subset(&post.message);
                                    let simulated = seed_start_simulated_map_combat_subset(
                                        &next,
                                        &relics,
                                        normal_combat_index,
                                    );
                                    seed_start_compare_combat_subset(
                                        report, action, &label, observed, simulated, true,
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Combat;
                                    normal_combat_index += 1;
                                }
                                RunPhase::Rest => {
                                    let label = format!("map rest node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_rest_observed_subset(&post.message),
                                        seed_start_rest_simulated_subset(&next, &relics),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Rest;
                                }
                                RunPhase::Treasure => {
                                    let label = format!("map treasure node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_treasure_observed_subset(&post.message),
                                        seed_start_treasure_simulated_subset(&next, &relics),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Treasure;
                                }
                                RunPhase::Shop => {
                                    let label = format!("map shop node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_shop_observed_subset(&post.message),
                                        seed_start_shop_room_simulated_subset(&next, &relics),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Shop;
                                }
                                RunPhase::Idle => {
                                    compare_subset(
                                        report,
                                        action,
                                        "map return",
                                        seed_start_map_return_observed_subset(&post.message),
                                        seed_start_map_return_observed_subset(&post.message),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Map;
                                }
                                RunPhase::Reward => {
                                    compare_subset(
                                        report,
                                        action,
                                        "map reward",
                                        seed_start_reward_observed_subset(&post.message),
                                        seed_start_reward_simulated_subset(&next, &relics),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Reward;
                                }
                                RunPhase::Complete => {
                                    let boundary = SeedStartBoundary {
                                        path: format!("$.actions[step={}].command", action.step),
                                        category: "unsupported_map_path".to_owned(),
                                        reason: "map choice unexpectedly completed the run"
                                            .to_owned(),
                                    };
                                    report.unsupported.push(UnsupportedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        reason: boundary.reason.clone(),
                                    });
                                    return finish_boundary!(boundary);
                                }
                            }
                            seed_start_update_carry_from_run(
                                seed_sim.as_ref().expect("map transition stored run"),
                                &mut relics,
                                &mut deck_ids,
                            );
                            continue;
                        }
                    }
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_path".to_owned(),
                    reason: "strict seed-start map transition could not be simulated; verifier refused to infer simulator state from the observed trace".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            SeedStartPhase::Treasure if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                let simulated_return = {
                    let Some(sim) = seed_sim.as_mut() else {
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_treasure_path".to_owned(),
                            reason: "seed-start treasure action without initialized run simulation"
                                .to_owned(),
                        });
                    };
                    let previous_act = sim.current_act;
                    let next = apply_run_action(sim, RunAction::Proceed).map_err(|e| e.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_treasure_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    if next.current_act != previous_act {
                        map_path_xs.clear();
                        combat_index = 0;
                        normal_combat_index = 0;
                    }
                    let mut simulated_return = seed_start_simulated_map_return(
                        start.numeric_seed,
                        &map_path_xs,
                        Some(&next),
                        &relics,
                        &deck_ids,
                        &deck_ids,
                    );
                    if next.current_act != previous_act && previous_act != 1 {
                        seed_start_project_post_boss_transition_current_node(&mut simulated_return);
                    }
                    let act_changed = next.current_act != previous_act;
                    if next.phase != RunPhase::Idle || !act_changed {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_treasure_destination".to_owned(),
                            reason: format!(
                                "treasure proceed produced phase {:?} and act transition {} -> {}",
                                next.phase, previous_act, next.current_act
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    *sim = next;
                    simulated_return
                };
                compare_subset(
                    report,
                    action,
                    "boss chest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    simulated_return,
                );
                seed_start_test_pop_last_diff(report, action, &start.external_seed);
                phase = SeedStartPhase::Map;
                continue;
            }
            SeedStartPhase::Treasure if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .expect("malformed CHOOSE rejected before phase dispatch");
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: "seed-start treasure action without initialized run simulation"
                            .to_owned(),
                    });
                };
                if choose_index != 0 {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: format!("treasure chest choice {choose_index} is not available"),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                let next =
                    apply_run_action(sim, RunAction::OpenChest).map_err(|error| error.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let reward = next.reward.as_ref();
                let boss_reward = next.phase == RunPhase::Reward
                    && next.boss_chest_opened
                    && reward.is_some_and(|reward| !reward.boss_relic_choices.is_empty());
                let ordinary_reward = next.phase == RunPhase::Reward
                    && !next.boss_chest_opened
                    && reward.is_some_and(|reward| reward.boss_relic_choices.is_empty());
                if boss_reward {
                    compare_subset(
                        report,
                        action,
                        "open boss relic chest",
                        seed_start_boss_reward_observed_subset(&post.message),
                        seed_start_boss_reward_simulated_subset(&next, &relics),
                    );
                    seed_start_test_pop_last_diff(report, action, &start.external_seed);
                    phase = SeedStartPhase::BossReward;
                } else if ordinary_reward {
                    compare_subset(
                        report,
                        action,
                        "open treasure chest",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(&next, &relics),
                    );
                    phase = SeedStartPhase::Reward;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_treasure_destination".to_owned(),
                        reason: format!(
                            "open chest produced inconsistent simulator destination: phase={:?}, boss_chest_opened={}, reward={}, boss_choices={}",
                            next.phase,
                            next.boss_chest_opened,
                            reward.is_some(),
                            reward.map_or(0, |reward| reward.boss_relic_choices.len()),
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
            }
            SeedStartPhase::Rest if action.command.trim().eq_ignore_ascii_case("SKIP") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest skip without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = apply_run_action(sim, RunAction::CloseCardReward)
                    .map_err(|error| error.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if next.phase != RunPhase::Rest || next.reward.is_some() {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_rest_reward_continuation".to_owned(),
                        reason: format!(
                            "rest reward skip produced phase {:?} with reward_present={}",
                            next.phase,
                            next.reward.is_some()
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                compare_subset(
                    report,
                    action,
                    "rest skip card reward",
                    seed_start_rest_observed_subset(&post.message),
                    seed_start_rest_simulated_subset(&next, &relics),
                );
                *sim = next;
            }
            SeedStartPhase::Rest if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest proceed without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = apply_rest_action(sim, RestAction::Proceed).map_err(|e| e.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                compare_subset(
                    report,
                    action,
                    "rest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    seed_start_simulated_map_return(
                        start.numeric_seed,
                        &map_path_xs,
                        Some(&next),
                        &relics,
                        &deck_ids,
                        &deck_ids,
                    ),
                );
                seed_start_test_pop_last_diff(report, action, &start.external_seed);
                *sim = next;
                phase = SeedStartPhase::Map;
                continue;
            }
            SeedStartPhase::Rest if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .expect("malformed CHOOSE rejected before phase dispatch");
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = if screen_type(&pre.message) == Some("REST") {
                    seed_start_rest_screen_actions(sim)
                        .get(choose_index)
                        .copied()
                        .ok_or_else(|| "unsupported rest choice".to_owned())
                        .and_then(|action| {
                            apply_rest_action(sim, action).map_err(|e| e.to_string())
                        })
                } else if screen_type(&pre.message) == Some("CARD_REWARD") {
                    let card_id = reward_card_id_from_choose(sim, choose_index)
                        .ok_or_else(|| "bad rest card reward choose".to_owned());
                    match card_id {
                        Ok(card_id) => apply_run_action(sim, RunAction::TakeCardReward { card_id })
                            .map_err(|e| e.to_string()),
                        Err(reason) => Err(reason),
                    }
                } else {
                    Err("unsupported rest choice".to_owned())
                };
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let (observed, simulated, label) = if next.card_grid.is_some() {
                    (
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                        "rest grid",
                    )
                } else {
                    match next.phase {
                        RunPhase::Reward
                            if next
                                .reward
                                .as_ref()
                                .is_some_and(|reward| reward.card_reward_active) =>
                        {
                            (
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next, &relics),
                                "rest card reward",
                            )
                        }
                        RunPhase::Rest if next.reward.is_none() => (
                            seed_start_rest_observed_subset(&post.message),
                            seed_start_rest_simulated_subset(&next, &relics),
                            "rest choice",
                        ),
                        phase => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_rest_destination".to_owned(),
                                reason: format!(
                                    "rest choice produced unsupported simulator phase {phase:?}"
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    }
                };
                compare_subset(report, action, label, observed, simulated);
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
                if sim.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if sim.reward.as_ref().is_some_and(|r| r.card_reward_active) {
                    phase = SeedStartPhase::Reward;
                } else if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Event if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .ok_or_else(|| format!("bad event choose {}", action.command));
                let Ok(choose_index) = choose_index else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: choose_index.err().unwrap(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: "seed-start event action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let Some(sim_choice_index) = seed_start_event_choice_index_for_communication_mod(
                    sim,
                    choose_index,
                    &pre.message,
                ) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: format!(
                            "event simulation could not map visible choice index {choose_index}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let delayed_event_deck_append_count = sim.event.as_ref().and_then(|screen| {
                    (screen.event == Event::Vampires
                        && screen.stage == 0
                        && sim_choice_index < screen.choices.len().saturating_sub(1))
                    .then_some(VAMPIRES_BITE_COUNT)
                });
                let spire_heart_stage = sim
                    .event
                    .as_ref()
                    .filter(|screen| screen.event == Event::SpireHeart)
                    .map(|screen| screen.stage);
                let Ok(next) = apply_event_action(
                    sim,
                    EventAction::Choose {
                        choice_index: sim_choice_index,
                    },
                ) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: "event simulation rejected transition".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if let Some(spire_heart_stage) = spire_heart_stage {
                    if spire_heart_stage == 3 {
                        compare_subset(
                            report,
                            action,
                            "Spire Heart completion",
                            seed_start_game_over_observed_subset(&post.message),
                            seed_start_game_over_simulated_subset(&next),
                        );
                    } else {
                        compare_subset(
                            report,
                            action,
                            "Spire Heart choice",
                            seed_start_event_observed_subset(&post.message),
                            seed_start_event_simulated_subset(&next, &relics),
                        );
                    }
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    phase = if next.phase == RunPhase::Complete {
                        SeedStartPhase::Complete
                    } else {
                        SeedStartPhase::Event
                    };
                    *sim = next;
                    continue;
                }
                if next.phase == RunPhase::Combat {
                    if next.combat.is_none() {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_event_destination".to_owned(),
                            reason: "event choice entered combat phase without combat state"
                                .to_owned(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    let label = "event combat";
                    let observed = seed_start_encounter_observed_subset(&post.message);
                    let simulated = seed_start_simulated_combat_subset(&next, false);
                    seed_start_compare_combat_subset(
                        report, action, label, observed, simulated, true,
                    );
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    phase = SeedStartPhase::Combat;
                    continue;
                }
                let (mut observed, mut simulated) = if next.card_grid.is_some() {
                    (
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    )
                } else {
                    match next.phase {
                        RunPhase::Idle if next.event.is_none() => (
                            seed_start_map_return_observed_subset(&post.message),
                            seed_start_simulated_map_return(
                                start.numeric_seed,
                                &map_path_xs,
                                Some(&next),
                                &relics,
                                &deck_ids,
                                &deck_ids,
                            ),
                        ),
                        RunPhase::Reward if next.reward.is_some() => (
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next, &relics),
                        ),
                        RunPhase::Event if next.event.is_some() => (
                            seed_start_event_observed_subset(&post.message),
                            seed_start_event_simulated_subset_with_delayed_deck_append(
                                &next,
                                &relics,
                                delayed_event_deck_append_count,
                            ),
                        ),
                        _ => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_event_destination".to_owned(),
                                reason: format!(
                                    "event choice produced unsupported simulator phase {:?}",
                                    next.phase
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    }
                };
                normalize_match_and_keep_transient_choices(&next, &mut observed, &simulated);
                if next.phase == RunPhase::Event && !next.pending_obtain_cards.is_empty() {
                    let observed_deck = observed
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let simulated_deck = simulated
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let non_deck_diffs = subset_diffs(observed, simulated);
                    if !non_deck_diffs.is_empty() {
                        report.unexpected_diffs.push(UnexpectedDiff {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "event choice".to_owned(),
                            diffs: non_deck_diffs,
                        });
                    } else {
                        let expected_deck =
                            deck_content_keys_after_pending_obtain_cards_settle(&next);
                        match classify_deferred_deck_observation(
                            &observed_deck,
                            &simulated_deck,
                            &expected_deck,
                        ) {
                            PendingDeckObservation::Settled => {
                                report.verified.push(VerifiedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: "event choice".to_owned(),
                                });
                            }
                            PendingDeckObservation::Deferred => {
                                pending_deck_assertion = Some(PendingDeckAssertion {
                                    action: action.clone(),
                                    label: "event choice".to_owned(),
                                    expected_deck,
                                });
                            }
                            PendingDeckObservation::Diverged(diffs) => {
                                report.unexpected_diffs.push(UnexpectedDiff {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    label: "event choice".to_owned(),
                                    diffs,
                                });
                            }
                        }
                    }
                } else {
                    compare_subset(report, action, "event choice", observed, simulated);
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next.clone();
                if next.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if next.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Map;
                } else if next.phase == RunPhase::Reward {
                    phase = SeedStartPhase::Reward;
                }
            }
            SeedStartPhase::Map => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_action".to_owned(),
                    reason: "seed-start verifier saw a map command that was not a visible generated map choice".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            SeedStartPhase::Combat => {
                let command = action.command.trim();
                let command_head = command.split_whitespace().next().unwrap_or("");
                let is_play_command = command_head.eq_ignore_ascii_case("PLAY");
                let combat_decision = match seed_sim
                    .as_ref()
                    .map(seed_start_active_combat_decision)
                    .transpose()
                {
                    Ok(decision) => decision.flatten(),
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_combat_decision_state".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                };
                let potion_use = parse_potion_use(command);
                let Some(sim) = seed_sim.as_mut() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "seed-start combat action without initialized combat simulation"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };

                if let Some(decision) = combat_decision {
                    if command.eq_ignore_ascii_case("WAIT") {
                        seed_start_compare_combat_subset(
                            report,
                            action,
                            "combat decision refresh",
                            seed_start_combat_observed_subset(&post.message),
                            seed_start_simulated_combat_subset(sim, false),
                            false,
                        );
                        continue;
                    }
                    let (decision_action, label) =
                        match seed_start_bind_combat_decision_command(decision, command) {
                            Ok(bound) => bound,
                            Err(reason) => {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "unsupported_combat_decision_command".to_owned(),
                                    reason,
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            }
                        };
                    let next = apply_run_action(sim, decision_action);
                    let Ok(next) = next else {
                        push_sim_error(report, action, label, next.err().unwrap());
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!("seed-start {label} simulation failed"),
                        });
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        label,
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if let Some(potion_use) = potion_use {
                    let is_smoke_bomb =
                        sim.potion_at_slot(potion_use.slot) == Some(Potion::SmokeBomb);
                    let target = seed_start_potion_command_target(sim, &potion_use);
                    let next = apply_run_action(
                        sim,
                        RunAction::UsePotion {
                            slot: potion_use.slot,
                            target,
                        },
                    );
                    let Ok(next) = next else {
                        push_sim_error(report, action, "combat potion use", next.err().unwrap());
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat potion simulation failed".to_owned(),
                        });
                    };
                    if is_smoke_bomb {
                        if next.phase != RunPhase::Idle
                            || next.combat.is_some()
                            || next.reward.is_some()
                        {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_smoke_bomb_core_destination".to_owned(),
                                reason: format!(
                                    "Smoke Bomb core transition produced phase {:?}, combat={}, reward={}",
                                    next.phase,
                                    next.combat.is_some(),
                                    next.reward.is_some(),
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                        if screen_type(&post.message) == Some("NONE")
                            && post.message.pointer("/game_state/combat_state").is_some()
                        {
                            let source = sim.clone();
                            let transient_matches = seed_start_compare_deferred_combat_subset(
                                report,
                                action,
                                "Smoke Bomb escape queued",
                                seed_start_smoke_bomb_transient_observed_subset(&post.message),
                                seed_start_smoke_bomb_transient_simulated_subset(&source, &next),
                            );
                            seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                            *sim = next;
                            smoke_bomb_ui = Some(SmokeBombUiState::Escaping {
                                source: Box::new(source),
                                action: action.clone(),
                                transient_matches,
                            });
                            continue;
                        }
                        if screen_type(&post.message) == Some("COMBAT_REWARD") {
                            seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                            compare_subset(
                                report,
                                action,
                                "Smoke Bomb escape settled to empty reward",
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next, &relics),
                            );
                            *sim = next;
                            phase = SeedStartPhase::Reward;
                            smoke_bomb_ui = Some(SmokeBombUiState::Reward {
                                pending_proceeds: Vec::new(),
                            });
                            continue;
                        }
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_smoke_bomb_ui_transition".to_owned(),
                            reason: format!(
                                "Smoke Bomb command reached unsupported screen {:?}",
                                screen_type(&post.message)
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    if seed_start_run_has_combat_card_reward(&next) {
                        seed_start_compare_combat_subset(
                            report,
                            action,
                            "combat potion card reward",
                            seed_start_combat_observed_subset(&post.message),
                            seed_start_simulated_combat_subset(&next, false),
                            false,
                        );
                        *sim = next;
                        continue;
                    }
                    if next.phase == RunPhase::Reward && next.reward.is_some() {
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        compare_subset(
                            report,
                            action,
                            "reward-screen potion use",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next, &relics),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Reward;
                        continue;
                    }
                    if next.phase != RunPhase::Combat || next.combat.is_none() {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_combat_potion_destination".to_owned(),
                            reason: format!(
                                "combat potion produced phase {:?}, combat={}, reward={}",
                                next.phase,
                                next.combat.is_some(),
                                next.reward.is_some(),
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "combat potion use",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if command.eq_ignore_ascii_case("PROCEED")
                    && sim
                        .combat
                        .as_ref()
                        .is_some_and(|combat| combat.phase == CombatPhase::Lost)
                {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "death screen proceed".to_owned(),
                    });
                    continue;
                }

                if !(is_play_command || command_head.eq_ignore_ascii_case("END")) {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: format!(
                            "seed-start verifier does not support combat command {command:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }

                let Some(combat) = sim.combat.as_ref() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_simulator_state".to_owned(),
                        reason:
                            "seed-start verifier entered its combat phase without core combat state"
                                .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if let Some(reason) = unsupported_seed_start_combat_command(combat, command) {
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason,
                    });
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "unsupported card in seed-start combat".to_owned(),
                    });
                }

                let Some(combat_action) = combat_action_from_command(command, combat) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: format!(
                            "seed-start verifier could not parse combat command {command:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };

                if is_final_combat_blow(sim, combat_action) {
                    let next = apply_combat_action_on_run(sim, combat_action);
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "seed-start combat victory",
                            next.err().unwrap(),
                        );
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat victory simulation failed".to_owned(),
                        });
                    };
                    let label = combat_label(command, sim);
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_victory_observed_subset(&post.message),
                        seed_start_victory_simulated_subset(&next),
                    );
                    let final_boss_complete = seed_start_is_final_boss_victory(&next);
                    seed_sim = Some(next);
                    phase = if final_boss_complete {
                        SeedStartPhase::Proceed
                    } else {
                        SeedStartPhase::Reward
                    };
                    continue;
                }

                let next = apply_combat_action_on_run(sim, combat_action);
                let Ok(next) = next else {
                    push_sim_error(
                        report,
                        action,
                        "seed-start combat transition",
                        next.err().unwrap(),
                    );
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "seed-start combat simulation rejected transition".to_owned(),
                    });
                };
                let label = combat_label(command, sim);
                let observed = seed_start_combat_observed_subset(&post.message);
                let simulated = seed_start_simulated_combat_subset(&next, false);
                if seed_start_is_transient_combat_post_state(&post.message) {
                    seed_start_compare_transient_combat_subset(
                        report, action, &label, observed, simulated,
                    );
                } else {
                    seed_start_compare_combat_subset(
                        report, action, &label, observed, simulated, false,
                    );
                }
                *sim = next;
            }
            SeedStartPhase::Reward => {
                if matches!(smoke_bomb_ui, Some(SmokeBombUiState::Reward { .. }))
                    && action.command.eq_ignore_ascii_case("PROCEED")
                {
                    let destination = seed_sim
                        .as_ref()
                        .expect("Smoke Bomb reward keeps its core destination");
                    if screen_type(&post.message) == Some("COMBAT_REWARD") {
                        if seed_start_compare_deferred_subset(
                            report,
                            action,
                            "Smoke Bomb reward proceed awaiting map",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(destination, &relics),
                        ) {
                            let Some(SmokeBombUiState::Reward { pending_proceeds }) =
                                smoke_bomb_ui.as_mut()
                            else {
                                unreachable!("Smoke Bomb reward state checked above");
                            };
                            pending_proceeds.push(action.clone());
                        }
                        continue;
                    }
                    if screen_type(&post.message) == Some("MAP") {
                        let pending_proceeds = match smoke_bomb_ui.as_ref() {
                            Some(SmokeBombUiState::Reward { pending_proceeds }) => {
                                pending_proceeds.clone()
                            }
                            _ => unreachable!("Smoke Bomb reward state checked above"),
                        };
                        let diff_count = report.unexpected_diffs.len();
                        if let Some(boundary) = seed_start_handle_proceed_to_map(
                            report,
                            action,
                            &post.message,
                            start,
                            &mut phase,
                            &mut combat_index,
                            &mut _reward_step,
                            &mut map_path_xs,
                            &mut seed_sim,
                            &mut relics,
                            &mut deck_ids,
                        ) {
                            return finish_boundary!(boundary);
                        }
                        if report.unexpected_diffs.len() == diff_count {
                            for pending in pending_proceeds {
                                report.verified.push(VerifiedTransition {
                                    action_step: pending.step,
                                    command: pending.command,
                                    label: "Smoke Bomb reward proceed reconciled at map".to_owned(),
                                });
                                reconciled_deferred_action_steps.push(pending.step);
                            }
                            smoke_bomb_ui = None;
                        }
                        continue;
                    }
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "invalid_smoke_bomb_ui_transition".to_owned(),
                        reason: format!(
                            "Smoke Bomb reward proceed reached unsupported screen {:?}",
                            screen_type(&post.message)
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                if action.command.trim().eq_ignore_ascii_case("SKIP") {
                    let Some(sim) = seed_sim.as_mut() else {
                        return finish_boundary!(SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: "seed-start reward skip without initialized reward simulation"
                                .to_owned(),
                        });
                    };
                    let next = apply_run_action(sim, RunAction::CloseCardReward)
                        .map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    let (label, observed, simulated) = if next.card_grid.is_some() {
                        (
                            "skip card reward to grid",
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next, &relics),
                        )
                    } else {
                        match next.phase {
                            RunPhase::Reward if next.reward.is_some() => (
                                "skip combat card reward",
                                seed_start_reward_observed_subset(&post.message),
                                seed_start_reward_simulated_subset(&next, &relics),
                            ),
                            RunPhase::Rest if next.reward.is_none() => (
                                "skip rest card reward",
                                seed_start_rest_observed_subset(&post.message),
                                seed_start_rest_simulated_subset(&next, &relics),
                            ),
                            RunPhase::Event if next.event.is_some() => (
                                "skip event card reward",
                                seed_start_event_observed_subset(&post.message),
                                seed_start_event_simulated_subset(&next, &relics),
                            ),
                            RunPhase::Shop if next.shop.is_some() => (
                                "skip shop card reward",
                                seed_start_shop_observed_subset(&post.message),
                                seed_start_shop_screen_simulated_subset(&next, &relics),
                            ),
                            phase => {
                                let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_reward_destination".to_owned(),
                                reason: format!(
                                    "card reward skip produced unsupported simulator phase {phase:?}"
                                ),
                            };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return finish_boundary!(boundary);
                            }
                        }
                    };
                    compare_subset(report, action, label, observed, simulated);
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    if seed_start_reward_sequence_complete(sim) {
                        phase = seed_start_phase_after_reward_completion(sim);
                    }
                    continue;
                }
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_final_boss_victory)
                    {
                        let sim = seed_sim
                            .as_mut()
                            .expect("final boss simulation checked above");
                        let next = seed_start_apply_final_boss_proceed(sim);
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        compare_subset(
                            report,
                            action,
                            "final boss proceed to Spire Heart",
                            seed_start_spire_heart_observed_subset(&post.message),
                            seed_start_spire_heart_simulated_subset(&next),
                        );
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        *sim = next;
                        phase = SeedStartPhase::Event;
                        continue;
                    }
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_boss_chest_proceed)
                    {
                        let Some(sim) = seed_sim.as_mut() else {
                            return finish_boundary!(SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: "seed-start boss reward chest without initialized reward simulation"
                                    .to_owned(),
                            });
                        };
                        let next = apply_run_action(sim, RunAction::SkipReward)
                            .map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        if next.phase != RunPhase::Treasure {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: format!(
                                    "boss combat proceed ended in simulator phase {:?}",
                                    next.phase
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next, &relics),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
                        *sim = next;
                        phase = SeedStartPhase::Treasure;
                        continue;
                    }
                    if seed_sim
                        .as_ref()
                        .is_some_and(|sim| sim.phase == RunPhase::Reward && sim.event.is_some())
                    {
                        let sim = seed_sim.as_mut().expect("reward simulation checked above");
                        let reward_action = if seed_start_reward_sequence_complete(sim) {
                            RunAction::Proceed
                        } else {
                            RunAction::SkipReward
                        };
                        let next =
                            apply_run_action(sim, reward_action).map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        let (label, expected, next_phase) = if next.phase == RunPhase::Idle {
                            (
                                "empty Neow reward proceed to map",
                                json!({
                                    "screen_type": "MAP",
                                    "ascension": start.ascension,
                                    "floor": 0,
                                    "gold": next.gold,
                                    "current_hp": next.player_hp,
                                    "max_hp": next.player_max_hp,
                                    "deck_ids": deck_content_keys(&next.deck),
                                    "relic_ids": relics,
                                    "choices": seed_start_first_map_choices(&start.external_seed),
                                }),
                                SeedStartPhase::Map,
                            )
                        } else {
                            (
                                "empty reward proceed to event",
                                json!({
                                    "screen_type": "EVENT",
                                    "ascension": start.ascension,
                                    "floor": next.current_floor,
                                    "gold": next.gold,
                                    "current_hp": next.player_hp,
                                    "max_hp": next.player_max_hp,
                                    "deck_ids": deck_content_keys(&next.deck),
                                    "relic_ids": relics,
                                    "choices": ["leave"],
                                }),
                                SeedStartPhase::Event,
                            )
                        };
                        compare_subset(
                            report,
                            action,
                            label,
                            seed_start_observed_subset(&post.message),
                            expected,
                        );
                        *sim = next;
                        phase = next_phase;
                        continue;
                    }
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        start,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &mut map_path_xs,
                        &mut seed_sim,
                        &mut relics,
                        &mut deck_ids,
                    ) {
                        return finish_boundary!(boundary);
                    }
                    continue;
                }
                let Some(sim) = seed_sim.as_mut() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_reward_path".to_owned(),
                        reason: "seed-start reward action without initialized reward simulation"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                if let Some(potion_use) = parse_potion_use(&action.command) {
                    let target = seed_start_potion_command_target(sim, &potion_use);
                    let next = apply_run_action(
                        sim,
                        RunAction::UsePotion {
                            slot: potion_use.slot,
                            target,
                        },
                    )
                    .map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    compare_subset(
                        report,
                        action,
                        "reward-screen potion use",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(sim, &relics),
                    );
                    continue;
                }

                let deck_before_reward_choice = deck_content_keys(&sim.deck);
                match seed_start_apply_reward_choose(sim, &action.command, &pre.message) {
                    Ok(label) => {
                        seed_start_update_carry_from_run(sim, &mut relics, &mut deck_ids);
                        let (mut observed, mut simulated) = if sim.card_grid.is_some() {
                            (
                                seed_start_grid_observed_subset(&post.message),
                                seed_start_grid_simulated_subset(sim, &relics),
                            )
                        } else {
                            match sim.phase {
                                RunPhase::Reward if sim.reward.is_some() => (
                                    seed_start_reward_observed_subset(&post.message),
                                    seed_start_reward_simulated_subset(sim, &relics),
                                ),
                                RunPhase::Rest if sim.reward.is_none() => (
                                    seed_start_rest_observed_subset(&post.message),
                                    seed_start_rest_simulated_subset(sim, &relics),
                                ),
                                RunPhase::Event
                                    if sim.event.as_ref().is_some_and(|event| {
                                        event.event == Event::Neow && event.stage == 2
                                    }) =>
                                {
                                    // Neow's completed continuation is authoritative core
                                    // state, but the command-facing frame remains the empty
                                    // reward screen until PROCEED leaves the room.
                                    (
                                        seed_start_reward_observed_subset(&post.message),
                                        seed_start_reward_simulated_subset(sim, &relics),
                                    )
                                }
                                RunPhase::Event if sim.event.is_some() => (
                                    seed_start_event_observed_subset(&post.message),
                                    seed_start_event_simulated_subset(sim, &relics),
                                ),
                                RunPhase::Shop if sim.shop.is_some() => (
                                    seed_start_shop_observed_subset(&post.message),
                                    seed_start_shop_screen_simulated_subset(sim, &relics),
                                ),
                                RunPhase::Treasure if sim.reward.is_none() => (
                                    seed_start_reward_observed_subset(&post.message),
                                    seed_start_reward_simulated_subset(sim, &relics),
                                ),
                                phase => {
                                    let boundary = SeedStartBoundary {
                                        path: format!(
                                            "$.actions[step={}].command",
                                            action.step
                                        ),
                                        category: "invalid_reward_destination".to_owned(),
                                        reason: format!(
                                            "reward choice produced unsupported simulator phase {phase:?}"
                                        ),
                                    };
                                    report.unsupported.push(UnsupportedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        reason: boundary.reason.clone(),
                                    });
                                    return finish_boundary!(boundary);
                                }
                            }
                        };
                        if label.starts_with("card reward pick ") && sim.phase == RunPhase::Reward {
                            let observed_deck = observed
                                .as_object_mut()
                                .and_then(|object| object.remove("deck_ids"))
                                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                                .unwrap_or_default();
                            let simulated_deck = simulated
                                .as_object_mut()
                                .and_then(|object| object.remove("deck_ids"))
                                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                                .unwrap_or_default();
                            let mut diffs = subset_diffs(observed, simulated);
                            let deck_observation = classify_deferred_deck_observation(
                                &observed_deck,
                                &deck_before_reward_choice,
                                &simulated_deck,
                            );
                            match deck_observation {
                                PendingDeckObservation::Settled if diffs.is_empty() => {
                                    report.verified.push(VerifiedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.clone(),
                                    });
                                }
                                PendingDeckObservation::Deferred if diffs.is_empty() => {
                                    pending_deck_assertion = Some(PendingDeckAssertion {
                                        action: action.clone(),
                                        label: label.clone(),
                                        expected_deck: simulated_deck,
                                    });
                                }
                                PendingDeckObservation::Diverged(deck_diffs) => {
                                    diffs.extend(deck_diffs);
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.clone(),
                                        diffs,
                                    });
                                }
                                PendingDeckObservation::Settled
                                | PendingDeckObservation::Deferred => {
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.clone(),
                                        diffs,
                                    });
                                }
                            }
                        } else {
                            compare_subset(report, action, &label, observed, simulated);
                        }
                        deck_ids = deck_content_keys(&sim.deck);
                        _reward_step += 1;
                        if sim.card_grid.is_some() {
                            phase = SeedStartPhase::Grid;
                        } else if seed_start_reward_sequence_complete(sim) {
                            phase = seed_start_phase_after_reward_completion(sim);
                        }
                    }
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                }
            }
            SeedStartPhase::BossReward if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .expect("malformed CHOOSE rejected before phase dispatch");
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "seed-start boss reward without initialized run simulation"
                            .to_owned(),
                    });
                };
                if screen_type(&pre.message) == Some("BOSS_REWARD") {
                    let next = apply_run_action(
                        sim,
                        RunAction::ChooseBossRelicReward {
                            index: choose_index,
                        },
                    )
                    .map_err(|e| e.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_boss_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    let visible_relics_before_pick = relics.clone();
                    let opened_master_deck_overlay =
                        seed_start_is_boss_relic_master_deck_overlay(&post.message);
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    if next.card_grid.is_some() {
                        compare_subset(
                            report,
                            action,
                            "boss relic reward grid",
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next, &relics),
                        );
                    } else if opened_master_deck_overlay {
                        compare_subset(
                            report,
                            action,
                            "boss relic reward deck overlay",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_boss_relic_deck_overlay_simulated_subset(
                                &next,
                                &visible_relics_before_pick,
                            ),
                        );
                    } else {
                        compare_subset(
                            report,
                            action,
                            "boss relic reward",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next, &relics),
                        );
                    }
                    seed_start_test_pop_last_diff(report, action, &start.external_seed);
                    *sim = next;
                    phase = if sim.card_grid.is_some() {
                        SeedStartPhase::Grid
                    } else {
                        SeedStartPhase::Treasure
                    };
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "unsupported boss relic reward choice".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
            }
            SeedStartPhase::BossReward if action.command.trim().eq_ignore_ascii_case("SKIP") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "seed-start boss reward without initialized run simulation"
                            .to_owned(),
                    });
                };
                let next = apply_run_action(sim, RunAction::SkipReward).map_err(|e| e.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                compare_subset(
                    report,
                    action,
                    "boss relic reward skip",
                    seed_start_treasure_observed_subset(&post.message),
                    seed_start_treasure_simulated_subset(&next, &relics),
                );
                seed_start_test_pop_last_diff(report, action, &start.external_seed);
                *sim = next;
                phase = SeedStartPhase::Treasure;
            }
            SeedStartPhase::Grid => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start grid action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let command = action.command.trim();
                let delayed_event_deck_append_count = (command_head_eq(command, "CHOOSE")
                    || command.eq_ignore_ascii_case("CONFIRM"))
                .then(|| {
                    sim.card_grid.as_ref().and_then(|grid| match grid.purpose {
                        GridPurpose::EventTransform { count }
                        | GridPurpose::EventTransformReturnToEvent { count, .. } => {
                            Some(usize::from(count))
                        }
                        GridPurpose::EventObtainCard
                        | GridPurpose::EventObtainCardReturnToEvent { .. } => Some(1),
                        _ => None,
                    })
                })
                .flatten();
                let next = seed_start_apply_grid_command(sim, command);
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                };
                let destination = match seed_start_grid_destination(&next) {
                    Ok(destination) => destination,
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_grid_destination".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                };
                let (label, mut observed, mut simulated, next_phase) = match destination {
                    SeedStartGridDestination::Grid => (
                        "grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                        SeedStartPhase::Grid,
                    ),
                    SeedStartGridDestination::Shop => (
                        "shop grid",
                        seed_start_shop_observed_subset(&post.message),
                        seed_start_shop_screen_simulated_subset(&next, &relics),
                        SeedStartPhase::Shop,
                    ),
                    SeedStartGridDestination::Event => (
                        "event grid",
                        seed_start_event_observed_subset(&post.message),
                        seed_start_event_simulated_subset_with_delayed_deck_append(
                            &next,
                            &relics,
                            delayed_event_deck_append_count,
                        ),
                        SeedStartPhase::Event,
                    ),
                    SeedStartGridDestination::Rest => (
                        "rest grid",
                        seed_start_rest_observed_subset(&post.message),
                        seed_start_rest_simulated_subset(&next, &relics),
                        SeedStartPhase::Rest,
                    ),
                    SeedStartGridDestination::Reward => (
                        "grid",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(&next, &relics),
                        if seed_start_reward_sequence_complete(&next) {
                            seed_start_phase_after_reward_completion(&next)
                        } else {
                            SeedStartPhase::Reward
                        },
                    ),
                    SeedStartGridDestination::Treasure => (
                        "boss relic grid confirm",
                        seed_start_treasure_observed_subset(&post.message),
                        seed_start_treasure_simulated_subset(&next, &relics),
                        SeedStartPhase::Treasure,
                    ),
                    SeedStartGridDestination::Proceed => (
                        "grid proceed",
                        seed_start_observed_subset(&post.message),
                        seed_start_proceed_simulated_subset(&next, &relics),
                        SeedStartPhase::Proceed,
                    ),
                };
                if destination == SeedStartGridDestination::Event
                    && delayed_event_deck_append_count.is_some()
                    && next.card_grid.is_none()
                {
                    let observed_deck = observed
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let simulated_deck = simulated
                        .as_object_mut()
                        .and_then(|object| object.remove("deck_ids"))
                        .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                        .unwrap_or_default();
                    let mut diffs = subset_diffs(observed, simulated);
                    let expected_deck = deck_content_keys(&next.deck);
                    match classify_deferred_deck_observation(
                        &observed_deck,
                        &simulated_deck,
                        &expected_deck,
                    ) {
                        PendingDeckObservation::Settled if diffs.is_empty() => {
                            report.verified.push(VerifiedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: label.to_owned(),
                            });
                        }
                        PendingDeckObservation::Deferred if diffs.is_empty() => {
                            pending_deck_assertion = Some(PendingDeckAssertion {
                                action: action.clone(),
                                label: label.to_owned(),
                                expected_deck,
                            });
                        }
                        PendingDeckObservation::Diverged(deck_diffs) => {
                            diffs.extend(deck_diffs);
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: label.to_owned(),
                                diffs,
                            });
                        }
                        PendingDeckObservation::Settled | PendingDeckObservation::Deferred => {
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: label.to_owned(),
                                diffs,
                            });
                        }
                    }
                } else {
                    compare_subset(report, action, label, observed, simulated);
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
                phase = next_phase;
            }
            SeedStartPhase::Shop => {
                let Some(sim) = seed_sim.as_mut() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_shop_path".to_owned(),
                        reason: "seed-start shop action without initialized run simulation"
                            .to_owned(),
                    });
                };
                let command = action.command.trim();
                if command.eq_ignore_ascii_case("LEAVE") {
                    let next = match apply_run_action(sim, RunAction::LeaveShop) {
                        Ok(next) => next,
                        Err(err) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_shop_path".to_owned(),
                                reason: format!("core rejected shop merchant leave: {err}"),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if seed_start_shop_destination(&next) != Ok(SeedStartShopDestination::Room) {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_shop_destination".to_owned(),
                            reason: seed_start_shop_destination(&next).err().unwrap_or_else(|| {
                                "shop merchant leave did not reach the shop room".to_owned()
                            }),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    compare_subset(
                        report,
                        action,
                        "leave shop merchant",
                        seed_start_shop_observed_subset(&post.message),
                        seed_start_shop_room_simulated_subset(&next, &relics),
                    );
                    *sim = next;
                    continue;
                }
                if command.eq_ignore_ascii_case("PROCEED") {
                    let next = match apply_run_action(sim, RunAction::Proceed) {
                        Ok(next) => next,
                        Err(err) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_shop_path".to_owned(),
                                reason: format!("core rejected shop room proceed: {err}"),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if seed_start_shop_destination(&next) != Ok(SeedStartShopDestination::Map) {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "invalid_shop_destination".to_owned(),
                            reason: seed_start_shop_destination(&next).err().unwrap_or_else(|| {
                                "shop room proceed did not reach the map".to_owned()
                            }),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    compare_subset(
                        report,
                        action,
                        "leave shop room",
                        seed_start_map_return_observed_subset(&post.message),
                        seed_start_simulated_map_return(
                            start.numeric_seed,
                            &map_path_xs,
                            Some(&next),
                            &relics,
                            &deck_ids,
                            &deck_ids,
                        ),
                    );
                    *sim = next;
                    phase = SeedStartPhase::Map;
                    continue;
                }
                if command_head_eq(command, "CHOOSE") {
                    let choose_index = choose_index(command)
                        .expect("malformed CHOOSE rejected before phase dispatch");
                    let (shop_action, label) = match seed_start_bind_shop_choose(sim, choose_index)
                    {
                        Ok(bound) => bound,
                        Err(reason) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_shop_path".to_owned(),
                                reason,
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    let next = apply_run_action(sim, shop_action).map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_shop_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    };
                    let destination = match seed_start_shop_destination(&next) {
                        Ok(destination) => destination,
                        Err(reason) => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_shop_destination".to_owned(),
                                reason,
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    };
                    if screen_type(&post.message) == Some("NONE") {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "trace_client_shop_transient".to_owned(),
                            reason: format!(
                                "shop {shop_action:?} reached a transient NONE frame before its core-owned {destination:?} destination became observable"
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return finish_boundary!(boundary);
                    }
                    match destination {
                        SeedStartShopDestination::Screen => {
                            let mut observed = seed_start_shop_observed_subset(&post.message);
                            let mut simulated =
                                seed_start_shop_screen_simulated_subset(&next, &relics);
                            let observed_deck = observed
                                .as_object_mut()
                                .and_then(|fields| fields.remove("deck_ids"))
                                .and_then(|deck| serde_json::from_value::<Vec<String>>(deck).ok())
                                .unwrap_or_default();
                            if let Some(fields) = simulated.as_object_mut() {
                                fields.remove("deck_ids");
                            }
                            let mut diffs = subset_diffs(observed, simulated);
                            let expected_deck = deck_content_keys(&next.deck);
                            match classify_deferred_deck_observation(
                                &observed_deck,
                                &deck_content_keys(&sim.deck),
                                &expected_deck,
                            ) {
                                PendingDeckObservation::Settled if diffs.is_empty() => {
                                    report.verified.push(VerifiedTransition {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.to_owned(),
                                    });
                                }
                                PendingDeckObservation::Deferred if diffs.is_empty() => {
                                    pending_deck_assertion = Some(PendingDeckAssertion {
                                        action: action.clone(),
                                        label: label.to_owned(),
                                        expected_deck,
                                    });
                                }
                                PendingDeckObservation::Diverged(deck_diffs) => {
                                    diffs.extend(deck_diffs);
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.to_owned(),
                                        diffs,
                                    });
                                }
                                PendingDeckObservation::Settled
                                | PendingDeckObservation::Deferred => {
                                    report.unexpected_diffs.push(UnexpectedDiff {
                                        action_step: action.step,
                                        command: action.command.clone(),
                                        label: label.to_owned(),
                                        diffs,
                                    });
                                }
                            }
                        }
                        SeedStartShopDestination::Grid => compare_subset(
                            report,
                            action,
                            label,
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next, &relics),
                        ),
                        SeedStartShopDestination::Reward => compare_subset(
                            report,
                            action,
                            label,
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next, &relics),
                        ),
                        destination => {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "invalid_shop_destination".to_owned(),
                                reason: format!(
                                    "shop CHOOSE {choose_index} produced unsupported destination {destination:?}"
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                    }
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    phase = match destination {
                        SeedStartShopDestination::Grid => SeedStartPhase::Grid,
                        SeedStartShopDestination::Reward => SeedStartPhase::Reward,
                        SeedStartShopDestination::Screen => SeedStartPhase::Shop,
                        _ => unreachable!("shop CHOOSE destination checked above"),
                    };
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_shop_path".to_owned(),
                    reason: format!(
                        "seed-start verifier does not support shop command {command:?}"
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
            SeedStartPhase::Proceed => {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_final_boss_victory)
                    {
                        let sim = seed_sim
                            .as_mut()
                            .expect("final boss simulation checked above");
                        let next = seed_start_apply_final_boss_proceed(sim);
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        compare_subset(
                            report,
                            action,
                            "final boss proceed to Spire Heart",
                            seed_start_spire_heart_observed_subset(&post.message),
                            seed_start_spire_heart_simulated_subset(&next),
                        );
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        *sim = next;
                        phase = SeedStartPhase::Event;
                        continue;
                    }
                    if seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_boss_chest_proceed)
                    {
                        let Some(sim) = seed_sim.as_mut() else {
                            return finish_boundary!(SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: "seed-start boss reward chest without initialized reward simulation"
                                    .to_owned(),
                            });
                        };
                        let next = apply_run_action(sim, RunAction::SkipReward)
                            .map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        };
                        if next.phase != RunPhase::Treasure {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: format!(
                                    "boss combat proceed ended in simulator phase {:?}",
                                    next.phase
                                ),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return finish_boundary!(boundary);
                        }
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next, &relics),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
                        *sim = next;
                        phase = SeedStartPhase::Treasure;
                        continue;
                    }
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        start,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &mut map_path_xs,
                        &mut seed_sim,
                        &mut relics,
                        &mut deck_ids,
                    ) {
                        return finish_boundary!(boundary);
                    }
                    continue;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_post_reward_map".to_owned(),
                        reason: "seed-start verifier expected reward-to-map PROCEED command"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
            }
            SeedStartPhase::Complete if action.command.eq_ignore_ascii_case("PROCEED") => {
                let Some(sim) = seed_sim.as_ref() else {
                    return finish_boundary!(SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_complete_path".to_owned(),
                        reason: "terminal proceed without initialized run simulation".to_owned(),
                    });
                };
                if sim.phase != RunPhase::Complete
                    || !sim
                        .event
                        .as_ref()
                        .is_some_and(|event| event.event == Event::SpireHeart && event.stage == 4)
                {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_complete_path".to_owned(),
                        reason: "terminal proceed requires completed Spire Heart state".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return finish_boundary!(boundary);
                }
                compare_subset(
                    report,
                    action,
                    "leave completed run",
                    json!({
                        "in_game": post.message.get("in_game").and_then(Value::as_bool),
                    }),
                    json!({ "in_game": false }),
                );
                continue;
            }
            _ => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unexpected_seed_start_command".to_owned(),
                    reason: format!(
                        "seed-start bootstrap harness did not expect command '{}' in phase {:?}",
                        action.command, phase
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return finish_boundary!(boundary);
            }
        }
    }

    finish_boundary!(SeedStartBoundary {
        path: "$.actions[verified]".to_owned(),
        category: "none".to_owned(),
        reason: "seed-start verifier checked every verifiable transition in the trace".to_owned(),
    })
}

fn seed_start_finish_boundary(
    seed_sim: &Option<RunState>,
    boundary: SeedStartBoundary,
    numeric_seed: i64,
    boss_unlocks: BossUnlockState,
    reconciled_deferred_action_steps: Vec<u32>,
    unresolved_transient_assertions: usize,
) -> SeedStartVerification {
    let mut final_run_state = seed_sim.clone();
    if let Some(run) = final_run_state.as_mut() {
        seed_start_apply_boss_unlocks(run, numeric_seed, boss_unlocks);
    }
    SeedStartVerification {
        boundary,
        final_run_state,
        reconciled_deferred_action_steps,
        unresolved_transient_assertions,
    }
}

fn seed_start_apply_boss_unlocks(
    run: &mut RunState,
    numeric_seed: i64,
    boss_unlocks: BossUnlockState,
) {
    run.act1_boss = Act1Boss::from_trace_name(&target_exordium_act_one_boss_with_unlocks(
        numeric_seed,
        boss_unlocks,
    ))
    .unwrap_or_default();
    run.act3_boss = Act3Boss::from_game_key(&target_beyond_act_three_boss_with_unlocks(
        numeric_seed,
        boss_unlocks,
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartPhase {
    BeforeStart,
    NeowTalk,
    NeowOptions,
    NeowCardReward,
    NeowPotionReward,
    NeowTransformGrid,
    NeowTransformConfirm,
    NeowGrid,
    NeowGridConfirm,
    NeowBossSwapCallingBellGrid,
    NeowBossSwapCallingBellReward,
    NeowBossSwapAstrolabeGrid,
    NeowBossSwapPandorasBoxGrid,
    NeowBossSwapEmptyCageGrid,
    NeowLeave,
    Map,
    Event,
    Rest,
    Shop,
    Grid,
    Treasure,
    BossReward,
    Combat,
    Reward,
    Proceed,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartGridDestination {
    Grid,
    Shop,
    Event,
    Rest,
    Reward,
    Treasure,
    Proceed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartShopDestination {
    Room,
    Screen,
    Grid,
    Reward,
    Map,
}

fn seed_start_shop_destination(run: &RunState) -> Result<SeedStartShopDestination, String> {
    if run.card_grid.is_some() {
        return if run.phase == RunPhase::Shop && run.shop.is_some() {
            Ok(SeedStartShopDestination::Grid)
        } else {
            Err(format!(
                "shop grid has inconsistent simulator state: phase={:?}, shop={}",
                run.phase,
                run.shop.is_some()
            ))
        };
    }
    match run.phase {
        RunPhase::Shop if run.shop.is_some() && run.shop_merchant_open => {
            Ok(SeedStartShopDestination::Screen)
        }
        RunPhase::Shop if run.shop.is_some() => Ok(SeedStartShopDestination::Room),
        RunPhase::Reward if run.reward.is_some() => Ok(SeedStartShopDestination::Reward),
        RunPhase::Idle if run.shop.is_none() && run.reward.is_none() => {
            Ok(SeedStartShopDestination::Map)
        }
        phase => Err(format!(
            "shop command produced inconsistent simulator destination: phase={phase:?}, shop={}, merchant_open={}, grid={}, reward={}",
            run.shop.is_some(),
            run.shop_merchant_open,
            run.card_grid.is_some(),
            run.reward.is_some(),
        )),
    }
}

fn seed_start_bind_shop_choose(
    run: &RunState,
    choose_index: usize,
) -> Result<(RunAction, &'static str), String> {
    if run.phase != RunPhase::Shop || run.shop.is_none() || run.card_grid.is_some() {
        return Err(format!(
            "shop CHOOSE requires an available simulator-owned shop decision: phase={:?}, shop={}, grid={}",
            run.phase,
            run.shop.is_some(),
            run.card_grid.is_some(),
        ));
    }
    if !run.shop_merchant_open {
        if choose_index != 0 {
            return Err(format!(
                "shop room exposes only merchant choice zero, received {choose_index}"
            ));
        }
        return Ok((RunAction::EnterShop, "enter shop merchant"));
    }
    let action = shop_action_for_choice_index(run, choose_index).map_err(|err| err.to_string())?;
    let label = if action == RunAction::OpenShopRemove {
        "shop purge grid"
    } else {
        "shop purchase"
    };
    Ok((action, label))
}

fn seed_start_grid_destination(run: &RunState) -> Result<SeedStartGridDestination, String> {
    if run.card_grid.is_some() {
        return Ok(SeedStartGridDestination::Grid);
    }

    match run.phase {
        RunPhase::Shop if run.shop.is_some() => Ok(SeedStartGridDestination::Shop),
        RunPhase::Event if run.event.is_some() => Ok(SeedStartGridDestination::Event),
        RunPhase::Rest => Ok(SeedStartGridDestination::Rest),
        RunPhase::Reward if run.reward.is_some() => Ok(SeedStartGridDestination::Reward),
        RunPhase::Treasure => Ok(SeedStartGridDestination::Treasure),
        RunPhase::Idle
            if run.shop.is_none()
                && run.event.is_none()
                && run.reward.is_none()
                && run.combat.is_none() =>
        {
            Ok(SeedStartGridDestination::Proceed)
        }
        phase => Err(format!(
            "grid command produced inconsistent simulator destination: phase={phase:?}, grid={}, shop={}, event={}, reward={}, combat={}",
            run.card_grid.is_some(),
            run.shop.is_some(),
            run.event.is_some(),
            run.reward.is_some(),
            run.combat.is_some(),
        )),
    }
}

fn parse_start_command(action: &TraceAction) -> Option<Result<StartRunCommand, SimRealError>> {
    let parts: Vec<_> = action.command.split_whitespace().collect();
    if !parts
        .first()
        .is_some_and(|command| command.eq_ignore_ascii_case("START"))
    {
        return None;
    }
    if parts.len() != 4 {
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
    Some(Ok(StartRunCommand {
        action_step: action.step,
        character: parts[1].to_owned(),
        ascension,
        external_seed: parts[3].to_owned(),
        numeric_seed,
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

fn command_is_choose(command: &str, index: usize) -> bool {
    command_choose_index(command).is_some_and(|parsed| parsed == index)
}

fn command_choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("CHOOSE") {
        parts[1].parse::<usize>().ok()
    } else {
        None
    }
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

fn seed_start_proceed_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    json!({
        "screen_type": "NONE",
        "ascension": run.ascension as u64,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": Vec::<String>::new(),
    })
}

fn seed_start_potion_observed_subset(message: &Value) -> Value {
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
        "potion_ids": potion_keys_from_value(game.get("potions")),
        "choices": choice_list_from_value(game.get("choice_list")),
        "unobservable": {
            "potion_reward_uuids": true,
        },
    })
}

fn seed_start_encounter_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let combat = game.get("combat_state");
    let player = combat.and_then(|combat| combat.get("player"));
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let monster_intents_visible = observed_monster_intents_visible(game);
    let mut subset = json!({
        "screen_type": screen_type,
        "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "potion_ids": potion_keys_from_value(game.get("potions")),
        "combat_player_hp": player.map(|p| int(p, "current_hp")).unwrap_or(0),
        "combat_player_block": player.map(|p| int(p, "block")).unwrap_or(0),
        "combat_player_energy": player.map(|p| int(p, "energy")).unwrap_or(0),
        "monster_intents_visible": monster_intents_visible,
        "monsters": seed_start_monsters_from_value(
            combat.and_then(|combat| combat.get("monsters")),
            monster_intents_visible,
        ),
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
    let monster_intents_visible = observed_monster_intents_visible(game);
    let mut subset = json!({
        "screen_type": screen_type,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": current_hp,
        "max_hp": int(game, "max_hp"),
        "potion_ids": potion_keys_from_value(game.get("potions")),
        "combat_player_hp": if screen_type == "CARD_REWARD" { current_hp } else { player.map(|p| int(p, "current_hp")).unwrap_or(0) },
        "combat_player_block": player.map(|p| int(p, "block")).unwrap_or(0),
        "combat_player_energy": player.map(|p| int(p, "energy")).unwrap_or(0),
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

fn seed_start_smoke_bomb_transient_observed_subset(message: &Value) -> Value {
    let mut subset = seed_start_combat_observed_subset(message);
    seed_start_defer_smoke_bomb_hp(&mut subset);
    subset
}

fn seed_start_smoke_bomb_transient_simulated_subset(
    source: &RunState,
    destination: &RunState,
) -> Value {
    let mut projection = source.clone();
    projection.potions = destination.potions.clone();
    let mut subset = seed_start_simulated_combat_subset(&projection, false);
    seed_start_defer_smoke_bomb_hp(&mut subset);
    subset
}

fn seed_start_defer_smoke_bomb_hp(subset: &mut Value) {
    if let Value::Object(fields) = subset {
        fields.remove("current_hp");
        fields.remove("combat_player_hp");
    }
}

fn seed_start_reward_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut out = json!({
        "screen_type": screen_type,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
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
            let mut projected = json!({
                "name": monster.get("name").and_then(Value::as_str).unwrap_or(""),
                "current_hp": int(monster, "current_hp"),
                "max_hp": int(monster, "max_hp"),
                "block": int(monster, "block"),
                "intent": monster.get("intent").and_then(Value::as_str).unwrap_or(""),
                "move_id": int(monster, "move_id"),
                "strength": power_amount(monster.get("powers"), "Strength"),
                "ritual": power_amount(monster.get("powers"), "Ritual"),
                "vulnerable": power_amount(monster.get("powers"), "Vulnerable"),
            });
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
    cards
        .iter()
        .filter_map(|card| {
            content_id_from_card_value(card)
                .map(|content_id| deck_content_key(content_id).to_owned())
        })
        .collect()
}

fn cards_to_comm_mod_visible_order<'a>(
    cards: impl IntoIterator<Item = &'a CardInstance>,
) -> Vec<String> {
    cards
        .into_iter()
        .map(|card| deck_content_key(card.content_id).to_owned())
        .collect()
}

#[cfg(test)]
fn hand_to_comm_mod_visible_order(cards: &[CardInstance]) -> Vec<String> {
    cards_to_comm_mod_visible_order(cards.iter())
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

fn ironclad_starter_deck_keys() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
        "Defend_R", "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn ironclad_deck_after_transform_selection_keys() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R", "Defend_R",
        "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn seed_start_generated_transform_card(numeric_seed: i64) -> Option<String> {
    generate_neow_transform_reward(numeric_seed, &[STRIKE_R_ID])
        .cards
        .first()
        .map(|card| deck_content_key(*card).to_owned())
}

fn seed_start_deck_after_transform(numeric_seed: i64) -> Vec<String> {
    let mut deck = ironclad_deck_after_transform_selection_keys();
    if let Some(card) = seed_start_generated_transform_card(numeric_seed) {
        deck.push(card);
    }
    deck
}

fn seed_start_neow_choices(numeric_seed: i64) -> Vec<String> {
    generate_neow_options(numeric_seed, 80)
        .into_iter()
        .map(|option| option.label)
        .collect()
}

fn seed_start_selected_neow_option(
    numeric_seed: i64,
    command: &str,
) -> Option<GeneratedNeowOption> {
    let index = command_choose_index(command)?;
    generate_neow_options(numeric_seed, 80)
        .into_iter()
        .nth(index)
}

fn seed_start_apply_neow_simple_option(option: GeneratedNeowOption) -> Option<(i32, i32, i32)> {
    if !seed_start_neow_drawback_is_simple(option.drawback)
        || !seed_start_neow_reward_is_simple(option.reward)
    {
        return None;
    }

    let mut run = RunState::map_fixture();
    run.gold = 99;
    apply_neow_simple_drawback(&mut run, option.drawback);
    apply_neow_simple_reward(&mut run, option.reward);
    Some((run.gold, run.player_hp, run.player_max_hp))
}

fn seed_start_neow_drawback_is_simple(drawback: NeowDrawback) -> bool {
    matches!(
        drawback,
        NeowDrawback::None
            | NeowDrawback::TenPercentHpLoss
            | NeowDrawback::NoGold
            | NeowDrawback::PercentDamage
    )
}

fn seed_start_neow_reward_is_simple(reward: NeowRewardType) -> bool {
    matches!(
        reward,
        NeowRewardType::TenPercentHpBonus
            | NeowRewardType::TwentyPercentHpBonus
            | NeowRewardType::HundredGold
            | NeowRewardType::TwoFiftyGold
    )
}

fn seed_start_neow_option_is_supported_curse_simple(option: GeneratedNeowOption) -> bool {
    option.drawback == NeowDrawback::Curse && seed_start_neow_reward_is_simple(option.reward)
}

fn seed_start_neow_option_is_supported_card_reward(option: GeneratedNeowOption) -> bool {
    seed_start_neow_drawback_is_supported_for_reward_screen(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::ThreeCards
                | NeowRewardType::RandomColorless
                | NeowRewardType::RandomColorlessTwo
                | NeowRewardType::ThreeRareCards
        )
}

fn seed_start_neow_option_is_supported_grid_reward(option: GeneratedNeowOption) -> bool {
    (seed_start_neow_drawback_is_simple(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::RemoveCard
                | NeowRewardType::RemoveTwo
                | NeowRewardType::UpgradeCard
                | NeowRewardType::TransformCard
                | NeowRewardType::TransformTwoCards
        ))
        || (option.drawback == NeowDrawback::Curse
            && option.reward == NeowRewardType::TransformTwoCards)
}

fn seed_start_neow_option_is_supported_relic_reward(option: GeneratedNeowOption) -> bool {
    seed_start_neow_drawback_is_supported_for_reward_screen(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::RandomCommonRelic | NeowRewardType::OneRareRelic
        )
}

fn seed_start_neow_option_is_supported_boss_swap(option: GeneratedNeowOption) -> bool {
    option.drawback == NeowDrawback::None && option.reward == NeowRewardType::BossRelic
}

fn seed_start_seeded_idle_run(numeric_seed: i64, ascension: u8, deck_ids: &[String]) -> RunState {
    let mut run = RunState::placeholder_seeded_ironclad(numeric_seed as u64, ascension);
    run.phase = RunPhase::Idle;
    run.event = None;
    run.reward = None;
    run.shop = None;
    run.shop_merchant_open = false;
    run.card_grid = None;
    run.combat = None;
    run.deck = deck_instances_from_keys(deck_ids);
    run
}

#[cfg(test)]
fn seed_start_apply_neow_curse_simple_option(
    numeric_seed: i64,
    deck_ids: &[String],
    option: GeneratedNeowOption,
) -> RunState {
    let mut run = RunState::map_fixture();
    run.gold = 99;
    run.reward_rng_seed = numeric_seed as u64;
    run.deck = deck_instances_from_keys(deck_ids);
    run.relics = vec![Relic::BurningBlood];
    apply_neow_curse_drawback(&mut run);
    apply_neow_simple_reward(&mut run, option.reward);
    run
}

fn seed_start_apply_neow_curse_simple_visible_option(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    apply_neow_simple_reward(&mut run, option.reward);
    run
}

fn seed_start_neow_drawback_is_supported_for_reward_screen(drawback: NeowDrawback) -> bool {
    seed_start_neow_drawback_is_simple(drawback) || drawback == NeowDrawback::Curse
}

#[cfg(test)]
fn seed_start_apply_neow_reward_drawback(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_apply_neow_reward_drawback_for_ascension(numeric_seed, 0, deck_ids, option)
}

fn seed_start_apply_neow_reward_drawback_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {}
        drawback => apply_neow_simple_drawback(&mut run, drawback),
    }
    run
}

#[cfg(test)]
fn seed_start_open_neow_grid_run(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_open_neow_grid_run_for_ascension(numeric_seed, 0, deck_ids, option)
}

fn seed_start_open_neow_grid_run_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {}
        drawback => apply_neow_simple_drawback(&mut run, drawback),
    }
    open_neow_reward_grid(&mut run, option.reward);
    run
}

fn seed_start_neow_curse_deck_key(numeric_seed: i64, card_rng_counter: u32) -> Option<String> {
    let mut run = RunState::map_fixture();
    run.reward_rng_seed = numeric_seed as u64;
    run.card_rng_counter = card_rng_counter;
    apply_neow_curse_drawback(&mut run);
    run.deck
        .last()
        .map(|card| deck_content_key(card.content_id).to_owned())
}

fn seed_start_is_neow_multi_select_grid(run: &RunState) -> bool {
    run.card_grid.as_ref().is_some_and(|grid| {
        matches!(
            grid.purpose,
            GridPurpose::NeowTransform { .. } | GridPurpose::NeowRemove { remaining: 2.. }
        )
    })
}

fn seed_start_neow_grid_transform_count(run: &RunState) -> Option<usize> {
    run.card_grid.as_ref().and_then(|grid| match grid.purpose {
        GridPurpose::NeowTransform { count } => Some(usize::from(count)),
        _ => None,
    })
}

fn seed_start_visible_deck_after_neow_transform_selection(
    deck_ids: &[String],
    transform_count: usize,
    delayed_curse: Option<&str>,
) -> Vec<String> {
    let mut visible = deck_ids.to_vec();
    for _ in 0..transform_count.min(visible.len()) {
        visible.pop();
    }
    if let Some(curse) = delayed_curse {
        visible.push(curse.to_owned());
    }
    visible
}

fn seed_start_apply_neow_boss_swap(numeric_seed: i64, deck_ids: &[String]) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, 0, deck_ids);
    run.gold = 99;
    run.relics = vec![Relic::BurningBlood];
    seed_start_prepare_neow_relic_equip(&mut run);
    apply_neow_boss_swap(&mut run);
    run.event = Some(neow_screen_for_stage(&run, 2));
    run
}

fn seed_start_prepare_neow_relic_equip(run: &mut RunState) {
    // Captured session-352 shows Neow-spawned Whetstone using the second
    // miscRng draw for its onEquip shuffle. Session-32 proves that boss-swap
    // Tiny House uses the same offset before choosing the upgraded starter
    // instance. The exact UI/update-site draw before relic equip is not
    // exposed by CommunicationMod, so keep this scoped to seed-start Neow
    // relic replay instead of changing ordinary relic pickup.
    if run.misc_rng_counter == 0 {
        run.misc_rng_counter = 1;
    }
}

fn seed_start_boss_swap_relic_ids(run: &RunState) -> Vec<String> {
    run.relics
        .iter()
        .map(|relic| relic.key())
        .chain(run.relic_keys.iter().copied())
        .filter(|key| *key != RelicKey::BurningBlood)
        .filter_map(|key| {
            let name = relic_key_trace_name(key);
            (name != "Unknown Relic").then(|| name.to_owned())
        })
        .collect()
}

fn seed_start_boss_swap_is_calling_bell_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::CallingBellCurse)
}

fn seed_start_boss_swap_is_astrolabe_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::Astrolabe)
}

fn seed_start_boss_swap_is_pandoras_box_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::PandorasBox)
}

fn seed_start_boss_swap_is_empty_cage_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| matches!(grid.purpose, GridPurpose::EmptyCage { .. }))
}

fn seed_start_boss_swap_is_tiny_house_reward(run: &RunState) -> bool {
    run.relics.contains(&Relic::TinyHouse) && run.reward.is_some()
}

fn seed_start_unsupported_boss_swap_reason(run: &RunState) -> Option<String> {
    if run.card_grid.is_some() {
        return Some(
            "Neow boss-swap produced a grid-opening boss relic without a dedicated seed-start follow-up; downstream parity remains classified"
                .to_owned(),
        );
    }
    if run.reward.is_some() {
        return Some(
            "Neow boss-swap produced a reward-screen boss relic; reward follow-up is classified outside this narrow verifier slice"
                .to_owned(),
        );
    }
    let unmapped = run
        .relics
        .iter()
        .map(|relic| relic.key())
        .chain(run.relic_keys.iter().copied())
        .find(|key| relic_key_trace_name(*key) == "Unknown Relic");
    unmapped.map(|key| {
        format!(
            "Neow boss-swap relic {key:?} is not trace-name mapped in sts_verify, so downstream parity remains classified"
        )
    })
}

fn seed_start_neow_grid_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::RemoveCard => "Neow remove card grid",
        NeowRewardType::RemoveTwo => "Neow remove two grid",
        NeowRewardType::UpgradeCard => "Neow upgrade grid",
        NeowRewardType::TransformTwoCards => "Neow transform two grid",
        _ => "Neow grid",
    }
}

fn seed_start_neow_card_reward_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::ThreeCards => "Neow card reward choices",
        NeowRewardType::OneRandomRareCard => "Neow random rare card reward",
        NeowRewardType::RandomColorlessTwo => "Neow rare colorless reward choices",
        NeowRewardType::ThreeRareCards => "Neow rare card reward choices",
        _ => "Neow card reward choices",
    }
}

fn seed_start_neow_card_reward_choice_names(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<String> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| content_key(content_id).to_ascii_lowercase())
        .collect()
}

fn seed_start_neow_card_reward_ids(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<String> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| {
            let key = content_key(content_id);
            if key == "Hand Of Greed" {
                "HandOfGreed".to_owned()
            } else {
                key.to_owned()
            }
        })
        .collect()
}

fn seed_start_neow_card_reward_id_values(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<Value> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| json!(content_id.get()))
        .collect()
}

fn seed_start_neow_card_reward_card_rng_counter(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Option<u32> {
    match option.reward {
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            let generated = if let Some(run) = run {
                generate_neow_colorless_reward_with_card_rng_counter(
                    numeric_seed,
                    option.reward,
                    run.card_rng_counter,
                )
            } else {
                generate_neow_colorless_reward(numeric_seed, option.reward)
            };
            Some(generated.card_rng_counter)
        }
        _ => None,
    }
}

fn seed_start_neow_card_reward_content_ids(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<ContentId> {
    match option.reward {
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            if option.drawback == NeowDrawback::Curse {
                generate_neow_colorless_reward(numeric_seed, option.reward).cards
            } else if let Some(run) = run {
                generate_neow_colorless_reward_with_card_rng_counter(
                    numeric_seed,
                    option.reward,
                    run.card_rng_counter,
                )
                .cards
            } else {
                generate_neow_colorless_reward(numeric_seed, option.reward).cards
            }
        }
        _ => generate_neow_card_reward(numeric_seed, option.reward).cards,
    }
}

#[cfg(test)]
fn seed_start_neow_potion_names(numeric_seed: i64) -> Vec<String> {
    generate_neow_three_potions(numeric_seed)
        .potions
        .into_iter()
        .map(|potion| potion_trace_name(potion).to_owned())
        .collect()
}

#[cfg(test)]
fn seed_start_apply_neow_relic_reward(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_apply_neow_relic_reward_for_ascension(numeric_seed, 0, deck_ids, option)
}

fn seed_start_apply_neow_relic_reward_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {
            run.reward_rng_seed = numeric_seed as u64;
            apply_neow_curse_drawback(&mut run);
        }
        drawback => apply_neow_simple_drawback(&mut run, drawback),
    }
    seed_start_prepare_neow_relic_equip(&mut run);
    apply_neow_relic_reward(&mut run, option.reward);
    run
}

fn seed_start_newest_trace_relic_name(run: &RunState) -> String {
    run.relics
        .iter()
        .last()
        .map(|relic| relic_key_trace_name(relic.key()).to_owned())
        .or_else(|| {
            run.relic_keys
                .last()
                .map(|key| relic_key_trace_name(*key).to_owned())
        })
        .unwrap_or_else(|| "Unknown Relic".to_owned())
}

fn seed_start_neow_relic_reward_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::RandomCommonRelic => "Neow common relic",
        NeowRewardType::OneRareRelic => "Neow rare relic",
        _ => "Neow relic",
    }
}

fn seed_start_pick_neow_card_reward(
    reward_choices: &Option<Vec<String>>,
    command: &str,
) -> Option<String> {
    let index = command_choose_index(command)?;
    reward_choices.as_ref()?.get(index).cloned()
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
    let boss_relic_ids = observed_boss_relic_key_choices(game)
        .into_iter()
        .map(|key| relic_key_trace_name(key).to_owned())
        .collect::<Vec<_>>();
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

fn seed_start_rest_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    let choices = if run.phase == RunPhase::Rest && !run.rest_room_complete {
        seed_start_rest_screen_actions(run)
            .into_iter()
            .filter_map(|action| match action {
                RestAction::Heal => Some("rest".to_owned()),
                RestAction::OpenSmith => Some("smith".to_owned()),
                RestAction::OpenRemove => Some("toke".to_owned()),
                RestAction::Lift => Some("lift".to_owned()),
                RestAction::Dig => Some("dig".to_owned()),
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
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
}

fn seed_start_rest_screen_actions(run: &RunState) -> Vec<RestAction> {
    legal_rest_actions(run)
        .into_iter()
        .filter(|action| {
            matches!(
                action,
                RestAction::Heal
                    | RestAction::OpenSmith
                    | RestAction::OpenRemove
                    | RestAction::Lift
                    | RestAction::Dig
            )
        })
        .collect()
}

fn seed_start_treasure_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
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
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
}

fn seed_start_boss_reward_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
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
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": boss_relic_ids.iter().map(|key| key.to_ascii_lowercase()).collect::<Vec<_>>(),
        "boss_relic_ids": boss_relic_ids,
    })
}

fn seed_start_is_boss_relic_master_deck_overlay(message: &Value) -> bool {
    screen_type(message) == Some("NONE")
        && message
            .get("game_state")
            .and_then(|game| game.get("screen_name"))
            .and_then(Value::as_str)
            .is_some_and(|screen| screen.eq_ignore_ascii_case("MASTER_DECK_VIEW"))
        && message
            .get("game_state")
            .and_then(|game| game.get("room_type"))
            .and_then(Value::as_str)
            .is_some_and(|room| room.eq_ignore_ascii_case("TreasureRoomBoss"))
}

fn seed_start_boss_relic_deck_overlay_simulated_subset(
    run: &RunState,
    visible_relic_ids_before_pick: &[String],
) -> Value {
    json!({
        "screen_type": "NONE",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": visible_relic_ids_before_pick,
        "choices": Vec::<String>::new(),
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

fn seed_start_shop_room_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    json!({
        "screen_type": "SHOP_ROOM",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
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
            ShopPick::BuyCard(slot) => shop_card_trace_label(run, shop.cards[slot].card.content_id),
            ShopPick::BuyRelic(slot) => {
                relic_key_trace_name(shop.relics[slot].relic_key).to_ascii_lowercase()
            }
            ShopPick::BuyPotion(_) => unreachable!("potions are projected below"),
        })
        .collect::<Vec<_>>();

    for offer in &shop.potions {
        if !offer.sold && run.gold >= offer.price && run.can_gain_potions() {
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

fn shop_card_trace_label(run: &RunState, content_id: ContentId) -> String {
    shop_card_display_key(run, content_id).to_ascii_lowercase()
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

fn seed_start_shop_screen_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    json!({
        "screen_type": if run.card_grid.is_some() { "GRID" } else { "SHOP_SCREEN" },
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": seed_start_shop_trace_choice_labels(run),
    })
}

fn grid_trace_choice_label(_run: &RunState, card: &CardInstance) -> String {
    use sts_core::content::cards::{
        CURSE_OF_THE_BELL_ID, DEFEND_R_ID, RITUAL_DAGGER_ID, STRIKE_R_ID,
    };
    if card.content_id == RITUAL_DAGGER_ID && card.upgrades > 0 {
        return "ritual dagger+".to_owned();
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
                        .map(|name| name.to_ascii_lowercase())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn seed_start_grid_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
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
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
}

fn grid_selection_ready_for_confirm(grid: &CardGridScreen) -> bool {
    if grid.selected.is_some() {
        return true;
    }
    let required = match grid.purpose {
        GridPurpose::Astrolabe => Some(3),
        GridPurpose::NeowRemove { remaining } if remaining > 1 => Some(usize::from(remaining)),
        GridPurpose::NeowTransform { count }
        | GridPurpose::EventTransform { count }
        | GridPurpose::EventTransformReturnToEvent { count, .. } => Some(usize::from(count)),
        _ => None,
    };
    required.is_some_and(|required| grid.selected_indices.len() >= required)
}

fn seed_start_grid_simulated_subset_with_deck(
    run: &RunState,
    relic_ids: &[String],
    visible_deck_ids: Vec<String>,
) -> Value {
    let mut subset = seed_start_grid_simulated_subset(run, relic_ids);
    if let Some(object) = subset.as_object_mut() {
        object.insert("deck_ids".to_owned(), json!(visible_deck_ids));
    }
    subset
}

fn reward_card_id_from_choose(run: &RunState, choose_index: usize) -> Option<CardId> {
    run.reward
        .as_ref()?
        .choices
        .get(choose_index)
        .map(|card| card.id)
}

fn seed_start_test_pop_last_diff(
    report: &mut SimRealReport,
    action: &TraceAction,
    external_seed: &str,
) {
    let _ = (report, action, external_seed);
}

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_proceed_to_map(
    report: &mut SimRealReport,
    action: &TraceAction,
    post_message: &Value,
    start: &StartRunCommand,
    phase: &mut SeedStartPhase,
    combat_index: &mut usize,
    reward_step: &mut usize,
    map_path_xs: &mut Vec<i32>,
    seed_sim: &mut Option<RunState>,
    carried_relics: &mut Vec<String>,
    carried_deck_ids: &mut Vec<String>,
) -> Option<SeedStartBoundary> {
    let Some(sim) = seed_sim.as_ref() else {
        return Some(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_post_reward_map".to_owned(),
            reason: "proceed-to-map command without initialized deterministic replay".to_owned(),
        });
    };
    let transient_boss_act_transition = screen_type(post_message) == Some("NONE")
        && post_message
            .get("game_state")
            .and_then(|game| game.get("room_type"))
            .and_then(Value::as_str)
            == Some("TreasureRoomBoss");
    let ftue_open = post_message
        .get("game_state")
        .and_then(|game| game.get("screen_name"))
        .and_then(Value::as_str)
        .is_some_and(|screen| screen.eq_ignore_ascii_case("FTUE"));
    if transient_boss_act_transition && ftue_open && sim.phase == RunPhase::Reward {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "boss reward proceed intercepted by FTUE overlay".to_owned(),
        });
        *phase = SeedStartPhase::Reward;
        return None;
    }

    let next: Result<RunState, String> = match sim.phase {
        RunPhase::Reward if sim.event.is_none() => {
            apply_run_action(sim, RunAction::SkipReward).map_err(|err| err.to_string())
        }
        RunPhase::Treasure => {
            apply_run_action(sim, RunAction::Proceed).map_err(|err| err.to_string())
        }
        RunPhase::Event
            if sim
                .event
                .as_ref()
                .is_some_and(|event| event.event == Event::Neow && event.stage == 2) =>
        {
            apply_event_action(sim, EventAction::Choose { choice_index: 0 })
                .map_err(|err| err.to_string())
        }
        RunPhase::Idle => Ok(sim.clone()),
        phase => Err(format!("simulator phase {phase:?} cannot proceed to map")),
    };
    let next = match next {
        Ok(next) => next,
        Err(reason) => {
            return Some(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_post_reward_map".to_owned(),
                reason,
            });
        }
    };
    if next.phase != RunPhase::Idle {
        return Some(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_post_reward_map".to_owned(),
            reason: format!(
                "proceed-to-map transition ended in simulator phase {:?}",
                next.phase
            ),
        });
    }
    *seed_sim = Some(next);

    if transient_boss_act_transition {
        if seed_sim.as_ref().is_none_or(|sim| sim.current_act <= 1) {
            let replay_state = seed_sim.as_ref().map(|sim| {
                format!(
                    "phase={:?} act={} floor={} boss_chest_opened={} reward_complete={}",
                    sim.phase,
                    sim.current_act,
                    sim.current_floor,
                    sim.boss_chest_opened,
                    seed_start_reward_sequence_complete(sim)
                )
            });
            return Some(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "unsupported_boss_act_transition".to_owned(),
                reason: format!(
                    "observed the transient post-boss NONE frame, but deterministic replay did not reach the next act ({})",
                    replay_state.unwrap_or_else(|| "run unavailable".to_owned())
                ),
            });
        }
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: "boss reward proceed awaiting settled next-act map".to_owned(),
        });
        if let Some(sim) = seed_sim.as_mut() {
            seed_start_update_carry_from_run(sim, carried_relics, carried_deck_ids);
        }
        map_path_xs.clear();
        *combat_index = 0;
        *reward_step = 0;
        *phase = SeedStartPhase::Map;
        return None;
    }
    let label = format!("return to map after floor {}", *combat_index + 1);
    let deck = seed_sim
        .as_ref()
        .map(|sim| deck_content_keys(&sim.deck))
        .unwrap_or_else(|| carried_deck_ids.clone());
    let observed = seed_start_map_return_observed_subset(post_message);
    let simulated = seed_start_simulated_map_return(
        start.numeric_seed,
        map_path_xs,
        seed_sim.as_ref(),
        carried_relics,
        &deck,
        &deck,
    );
    compare_subset(report, action, &label, observed, simulated);
    seed_start_test_pop_last_diff(report, action, &start.external_seed);
    if let Some(sim) = seed_sim.as_mut() {
        seed_start_update_carry_from_run(sim, carried_relics, carried_deck_ids);
    }
    *combat_index += 1;
    *reward_step = 0;
    *phase = SeedStartPhase::Map;
    None
}

fn seed_start_map_label(combat_index: usize) -> String {
    match combat_index {
        0 => "map first monster node".to_owned(),
        1 => "map floor 2 monster node".to_owned(),
        2 => "map floor 3 monster node".to_owned(),
        _ => format!("map floor {} monster node", combat_index + 1),
    }
}

fn seed_start_map_pick_x(external_seed: &str, path_so_far: &[i32], command: &str) -> i32 {
    let choice_index = choose_index(command).expect("map pick requires a valid CHOOSE command");
    let seed = seed_text_to_long(external_seed).expect("start command seed already parsed");
    if path_so_far.is_empty() {
        generate_exordium_map_topology(seed)
            .first_row_choices
            .get(choice_index)
            .copied()
            .unwrap_or(choice_index as i32)
    } else {
        generate_exordium_map_choices_after_path(seed, path_so_far)
            .last()
            .and_then(|step| step.next_choices.get(choice_index))
            .copied()
            .unwrap_or(choice_index as i32)
    }
}

#[cfg(test)]
fn seed_start_target_act_from_message(message: &Value) -> TargetMapAct {
    if let Some(act) = message
        .get("game_state")
        .and_then(|game| game.get("act"))
        .and_then(Value::as_u64)
    {
        return match act {
            3 => TargetMapAct::Beyond,
            2 => TargetMapAct::City,
            _ => TargetMapAct::Exordium,
        };
    }
    let floor = message
        .get("game_state")
        .and_then(|game| game.get("floor"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if floor >= 35 {
        TargetMapAct::Beyond
    } else if floor >= 18 {
        TargetMapAct::City
    } else {
        TargetMapAct::Exordium
    }
}

#[cfg(test)]
fn seed_start_room_kinds_on_path(
    numeric_seed: i64,
    path_xs: &[i32],
    message: &Value,
) -> Vec<RoomKind> {
    match seed_start_target_act_from_message(message) {
        TargetMapAct::Exordium => exordium_room_kinds_on_path(numeric_seed, path_xs),
        TargetMapAct::City => city_room_kinds_on_path(numeric_seed, path_xs),
        TargetMapAct::Beyond => {
            target_room_kinds_on_path(numeric_seed, TargetMapAct::Beyond, path_xs)
        }
    }
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

fn seed_start_simulated_map_return(
    numeric_seed: i64,
    path_xs: &[i32],
    run: Option<&RunState>,
    relic_ids: &[String],
    deck_ids: &[String],
    deck_fallback: &[String],
) -> Value {
    let act = seed_start_target_act_from_run(run);
    let gold = run.map(|sim| sim.gold).unwrap_or(99);
    let current_hp = run.map(|sim| sim.player_hp).unwrap_or(80);
    let max_hp = run.map(|sim| sim.player_max_hp).unwrap_or(80);
    let deck = run
        .map(|sim| deck_content_keys(&sim.deck))
        .unwrap_or_else(|| {
            if deck_ids.is_empty() {
                deck_fallback.to_vec()
            } else {
                deck_ids.to_vec()
            }
        });
    let relic_ids = run
        .map(|sim| relic_ids_for_simulated_subset(sim, relic_ids))
        .unwrap_or_else(|| relic_ids.to_vec());

    if let Some(sim) = run {
        if let Some(map_state) = sim.map.as_ref() {
            let current = map_state.map.node(map_state.current_node);
            let first_node_chosen = map_state.current_node.get() != 0;
            let (current_x, current_y) = if first_node_chosen {
                seed_start_map_node_xy(map_state.current_node)
            } else {
                (0, -1)
            };
            let current_symbol = if first_node_chosen {
                current
                    .map(|node| room_kind_symbol(node.room_kind))
                    .unwrap_or("")
            } else {
                ""
            };
            let mut map_action_run = sim.clone();
            map_action_run.phase = RunPhase::Idle;
            let legal_node_ids: Vec<_> = legal_map_actions_on_run(&map_action_run)
                .into_iter()
                .map(|action| match action {
                    sts_core::MapAction::ChooseNode { node_id } => node_id,
                })
                .collect();
            let next_node_ids = if legal_node_ids.is_empty() {
                sts_core::reachable_nodes(map_state)
            } else {
                legal_node_ids
            };
            if next_node_ids.is_empty() && !path_xs.is_empty() {
                // Some reward-to-map verifier states carry a stale map cursor after the room has
                // completed. The deterministic path projection below still derives the visible
                // map choices from the seed and accepted path, without hydrating from observation.
            } else {
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
                        .filter_map(|id| {
                            let node = map_state.map.node(*id)?;
                            let (x, y) = seed_start_map_node_xy(*id);
                            Some(json!({
                                "symbol": room_kind_symbol(node.room_kind),
                                "x": x,
                                "y": y,
                            }))
                        })
                        .collect()
                };
                return json!({
                    "screen_type": "MAP",
                    "floor": sim.current_floor.max(0) as u64,
                    "gold": gold,
                    "current_hp": current_hp,
                    "max_hp": max_hp,
                    "deck_ids": deck,
                    "relic_ids": relic_ids,
                    "choices": choices,
                    "first_node_chosen": first_node_chosen,
                    "current_node": {
                        "symbol": current_symbol,
                        "x": current_x,
                        "y": current_y,
                    },
                    "next_nodes": next_nodes,
                });
            }
        }
    }

    if path_xs.is_empty() {
        let topology = generate_target_map_topology(numeric_seed, act);
        let choices: Vec<String> = topology
            .first_row_choices
            .iter()
            .map(|x| format!("x={x}"))
            .collect();
        let next_nodes: Vec<Value> = topology
            .first_row_choices
            .iter()
            .map(|&x| {
                json!({
                    "symbol": room_kind_symbol(topology.first_row_room_kind),
                    "x": x,
                    "y": 0,
                })
            })
            .collect();
        let floor = run.map(|sim| sim.current_floor as u64).unwrap_or(0);
        return json!({
            "screen_type": "MAP",
            "floor": floor,
            "gold": gold,
            "current_hp": current_hp,
            "max_hp": max_hp,
            "deck_ids": deck,
            "relic_ids": relic_ids,
            "choices": choices,
            "first_node_chosen": false,
            "current_node": {
                "symbol": "",
                "x": 0,
                "y": -1,
            },
            "next_nodes": next_nodes,
        });
    }

    let steps = generate_target_map_choices_after_path(numeric_seed, act, path_xs);
    let Some(step) = steps.last() else {
        return json!({});
    };
    let boss_available = step.floor >= 15 && step.next_choices == [3];
    let choices: Vec<String> = if boss_available {
        vec!["boss".to_owned()]
    } else {
        step.next_choices.iter().map(|x| format!("x={x}")).collect()
    };
    let current_x = *path_xs.last().unwrap_or(&0);
    let current_y = path_xs.len().saturating_sub(1) as i64;
    let current_symbol = seed_start_room_kinds_on_path_for_act(numeric_seed, act, path_xs)
        .last()
        .copied()
        .map(room_kind_symbol)
        .unwrap_or("M");
    let next_nodes: Vec<Value> = if boss_available {
        Vec::new()
    } else {
        step.next_choices
            .iter()
            .map(|&x| {
                let mut child_path = path_xs.to_vec();
                child_path.push(x);
                let symbol = seed_start_room_kinds_on_path_for_act(numeric_seed, act, &child_path)
                    .last()
                    .copied()
                    .map(room_kind_symbol)
                    .unwrap_or("M");
                json!({
                    "symbol": symbol,
                    "x": x,
                    "y": current_y + 1,
                })
            })
            .collect()
    };
    json!({
        "screen_type": "MAP",
        "floor": path_xs.len() as u64,
        "gold": gold,
        "current_hp": current_hp,
        "max_hp": max_hp,
        "deck_ids": deck,
        "relic_ids": relic_ids,
        "choices": choices,
        "first_node_chosen": true,
        "current_node": {
            "symbol": current_symbol,
            "x": current_x,
            "y": current_y,
        },
        "next_nodes": next_nodes,
    })
}

fn seed_start_project_post_boss_transition_current_node(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert(
        "current_node".to_owned(),
        json!({
            "symbol": "",
            "x": -1,
            "y": 15,
        }),
    );
}

fn seed_start_map_node_xy(node_id: sts_core::MapNodeId) -> (i32, i64) {
    if node_id.get() == 0 {
        return (0, -1);
    }
    let index = node_id.get() - 1;
    ((index % 7) as i32, (index / 7) as i64)
}

fn seed_start_target_act_from_run(run: Option<&RunState>) -> TargetMapAct {
    match run.map(|sim| sim.current_act).unwrap_or(1) {
        3 => TargetMapAct::Beyond,
        2 => TargetMapAct::City,
        _ => TargetMapAct::Exordium,
    }
}

fn seed_start_room_kinds_on_path_for_act(
    numeric_seed: i64,
    act: TargetMapAct,
    path_xs: &[i32],
) -> Vec<RoomKind> {
    let previous_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let room_kinds =
        std::panic::catch_unwind(|| target_room_kinds_on_path(numeric_seed, act, path_xs));
    std::panic::set_hook(previous_panic_hook);
    room_kinds.unwrap_or_default()
}

#[cfg(test)]
fn seed_start_encounter_expected_at_index(
    seed: i64,
    combat_index: usize,
    ascension: u8,
    deck_ids: &[String],
    relics: &[String],
    neow_lament: bool,
    message: &Value,
) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let floor = game
        .get("floor")
        .and_then(Value::as_u64)
        .map(|value| u32::try_from(value).unwrap_or(1))
        .unwrap_or_else(|| u32::try_from(combat_index + 1).unwrap_or(1));
    let spawns = seed_start_normal_encounter_spawns_at_combat_index(
        seed,
        floor,
        combat_index,
        ascension,
        neow_lament,
    );
    let mut expected = seed_start_encounter_observed_subset(message);
    if let Value::Object(map) = &mut expected {
        map.insert(
            "monsters".to_owned(),
            Value::Array(
                spawns
                    .iter()
                    .enumerate()
                    .map(|(index, spawn)| seed_start_monster_from_spawn(seed, floor, spawn, index))
                    .collect(),
            ),
        );
        map.insert("deck_ids".to_owned(), json!(deck_ids));
        map.insert("relic_ids".to_owned(), json!(relics));
    }
    expected
}

#[cfg(test)]
fn seed_start_normal_encounter_spawns_at_combat_index(
    seed: i64,
    floor: u32,
    combat_index: usize,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    match seed_start_target_act_from_floor(floor) {
        TargetMapAct::Exordium => target_normal_encounter_spawn_at_combat_index(
            seed,
            floor,
            combat_index,
            ascension,
            neow_lament,
        ),
        TargetMapAct::City => target_city_normal_encounter_spawn_at_combat_index(
            seed,
            floor,
            combat_index,
            ascension,
            neow_lament,
        ),
        TargetMapAct::Beyond => {
            sts_core::content::encounters::target_normal_encounter_key_at_combat_index(
                seed,
                TargetMapAct::Beyond,
                combat_index,
            )
            .and_then(|encounter_key| {
                target_beyond_encounter_spawn_for_key(
                    seed,
                    floor,
                    &encounter_key,
                    ascension,
                    neow_lament,
                )
            })
        }
    }
    .unwrap_or_default()
}

#[cfg(test)]
fn seed_start_target_act_from_floor(floor: u32) -> TargetMapAct {
    if floor >= 35 {
        TargetMapAct::Beyond
    } else if floor >= 18 {
        TargetMapAct::City
    } else {
        TargetMapAct::Exordium
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn seed_start_core_neow_lament_active(run: Option<&RunState>) -> bool {
    run.is_some_and(|run| run.neow_lament_combats_remaining > 0)
}

#[cfg(test)]
fn seed_start_monster_from_spawn(
    seed: i64,
    floor: u32,
    spawn: &TargetEncounterSpawn,
    index: usize,
) -> Value {
    json!({
        "name": target_spawn_trace_name(seed, floor, spawn, index),
        "current_hp": spawn.current_hp,
        "max_hp": spawn.max_hp,
        "block": spawn.block,
        "intent": spawn.intent,
        "strength": spawn_power_amount(&spawn.powers, "Strength"),
        "ritual": spawn_power_amount(&spawn.powers, "Ritual"),
        "vulnerable": spawn_power_amount(&spawn.powers, "Vulnerable"),
    })
}

#[cfg(test)]
fn spawn_power_amount(powers: &[TargetSpawnPower], id: &str) -> i32 {
    powers
        .iter()
        .find(|power| power.id == id)
        .map(|power| power.amount)
        .unwrap_or(0)
}

#[cfg(test)]
fn target_spawn_trace_name(
    _seed: i64,
    _floor: u32,
    spawn: &TargetEncounterSpawn,
    _index: usize,
) -> &'static str {
    match spawn.name {
        "Louse" | "LouseDefensive" | "FuzzyLouseNormal" | "FuzzyLouseDefensive" => "Louse",
        "SlaverBlue" | "SlaverRed" => "Slaver",
        _ => spawn.name,
    }
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

fn seed_start_first_map_choices(seed: &str) -> Vec<String> {
    generate_exordium_map_topology(
        seed_text_to_long(seed).expect("start command seed already parsed"),
    )
    .first_row_choices
    .into_iter()
    .map(|x| format!("x={x}"))
    .collect()
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
        .filter_map(|card| {
            if let Some(content_id) = content_id_from_card_value(card) {
                return Some(json!(content_id.get()));
            }
            let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
            let identity = if upgrades > 0 {
                card.get("name").and_then(Value::as_str)
            } else {
                card.get("id").and_then(Value::as_str)
            }?;
            let identity = sts_core::run::reward::any_color_reward_card_key_from_identity(identity)
                .map(normalize_card_identity)
                .unwrap_or_else(|| identity.to_owned());
            Some(json!(identity))
        })
        .collect()
}

fn normalize_card_identity(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn simulated_card_reward_identity(content_id: ContentId) -> Value {
    if sts_core::content::cards::get_card_definition(content_id).is_some() {
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
                "symbol": node.get("symbol").and_then(Value::as_str).unwrap_or(""),
                "x": node.get("x").and_then(Value::as_i64).unwrap_or(0),
                "y": node.get("y").and_then(Value::as_i64).unwrap_or(0),
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
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_combat_relics_and_counters(game: &Value) -> (Vec<Relic>, RelicCounters) {
    let mut relics = Vec::new();
    let mut counters = RelicCounters::default();

    for relic in observed_relic_entries(game) {
        let Some(observed) = relic_from_trace_name(relic.name) else {
            continue;
        };
        if !relics.contains(&observed) {
            relics.push(observed);
        }

        let Some(counter) = relic.counter else {
            continue;
        };
        match observed {
            Relic::InkBottle => counters.ink_bottle_cards_played = counter,
            Relic::Nunchaku => counters.nunchaku_attacks_played = counter,
            Relic::PenNib => counters.pen_nib_attacks_played = counter,
            Relic::Shuriken => counters.shuriken_attacks_this_turn = counter,
            Relic::Kunai => counters.kunai_attacks_this_turn = counter,
            Relic::LetterOpener => counters.letter_opener_skills_this_turn = counter,
            Relic::Pocketwatch => counters.cards_played_this_turn = counter,
            Relic::HappyFlower => counters.happy_flower_turns = counter,
            Relic::StoneCalendar => counters.player_turns_started = counter,
            Relic::IncenseBurner => counters.incense_burner_counter = counter,
            _ => {}
        }
    }

    (relics, counters)
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_combat_turn(combat: &Value) -> Option<u32> {
    combat
        .get("turn")
        .and_then(Value::as_u64)
        .and_then(|turn| u32::try_from(turn).ok())
}

#[cfg(test)]
#[allow(dead_code)]
struct ObservedRelicEntry<'a> {
    name: &'a str,
    counter: Option<u32>,
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_relic_entries(game: &Value) -> Vec<ObservedRelicEntry<'_>> {
    let Some(relics) = game.get("relics").and_then(Value::as_array) else {
        return Vec::new();
    };

    relics
        .iter()
        .filter_map(|relic| {
            let name = relic
                .get("name")
                .or_else(|| relic.get("id"))
                .and_then(Value::as_str)?;
            let counter = relic
                .get("counter")
                .and_then(Value::as_i64)
                .and_then(|counter| u32::try_from(counter).ok());
            Some(ObservedRelicEntry { name, counter })
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_energy_per_turn(relics: &[Relic]) -> i32 {
    if relics.iter().any(|relic| {
        matches!(
            relic,
            Relic::CoffeeDripper
                | Relic::CursedKey
                | Relic::Ectoplasm
                | Relic::FusionHammer
                | Relic::MarkOfPain
                | Relic::PhilosophersStone
                | Relic::RunicDome
                | Relic::Sozu
                | Relic::VelvetChoker
        )
    }) {
        4
    } else {
        3
    }
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
    match key {
        RelicKey::Akabeko => "Akabeko",
        RelicKey::CrackedCore => "Cracked Core",
        RelicKey::RingOfTheSnake => "Ring of the Snake",
        RelicKey::PureWater => "Pure Water",
        RelicKey::Vajra => "Vajra",
        RelicKey::BottledTornado => "Bottled Tornado",
        RelicKey::Sundial => "Sundial",
        RelicKey::TheCourier => "The Courier",
        RelicKey::OrnamentalFan => "Ornamental Fan",
        RelicKey::HornCleat => "Horn Cleat",
        RelicKey::BottledFlame => "Bottled Flame",
        RelicKey::DarkstonePeriapt => "Darkstone Periapt",
        RelicKey::MercuryHourglass => "Mercury Hourglass",
        RelicKey::OldCoin => "Old Coin",
        RelicKey::Shovel => "Shovel",
        RelicKey::Turnip => "Turnip",
        RelicKey::FrozenCore => "Frozen Core",
        RelicKey::RingOfTheSerpent => "Ring of the Serpent",
        RelicKey::HolyWater => "Holy Water",
        RelicKey::HandDrill => "Hand Drill",
        RelicKey::LeesWaffle => "Lee's Waffle",
        RelicKey::FrozenEye => "Frozen Eye",
        RelicKey::TheAbacus => "The Abacus",
        RelicKey::Necronomicon => "Necronomicon",
        RelicKey::Enchiridion => "Enchiridion",
        RelicKey::NilrysCodex => "Nilry's Codex",
        RelicKey::MutagenicStrength => "Mutagenic Strength",
        RelicKey::BloodyIdol => "Bloody Idol",
        RelicKey::MarkOfBloom => "Mark of the Bloom",
        RelicKey::SpiritPoop => "Spirit Poop",
        RelicKey::OddMushroom => "Odd Mushroom",
        RelicKey::NlothsGift => "N'loth's Gift",
        RelicKey::Circlet => "Circlet",
        RelicKey::RedCirclet => "Red Circlet",
        RelicKey::Anchor => "Anchor",
        RelicKey::TheBoot => "The Boot",
        RelicKey::TinyChest => "Tiny Chest",
        RelicKey::BagOfMarbles => "Bag of Marbles",
        RelicKey::BagOfPreparation => "Bag of Preparation",
        RelicKey::BurningBlood => "Burning Blood",
        RelicKey::BloodVial => "Blood Vial",
        RelicKey::RedSkull => "Red Skull",
        RelicKey::DreamCatcher => "Dream Catcher",
        RelicKey::Torii => "Torii",
        RelicKey::MoltenEgg => "Molten Egg",
        RelicKey::ToxicEgg => "Toxic Egg",
        RelicKey::FrozenEgg => "Frozen Egg",
        RelicKey::MummifiedHand => "Mummified Hand",
        RelicKey::CharonsAshes => "Charon's Ashes",
        RelicKey::CeramicFish => "Ceramic Fish",
        RelicKey::GamblingChip => "Gambling Chip",
        RelicKey::PenNib => "Pen Nib",
        RelicKey::MembershipCard => "Membership Card",
        RelicKey::Pantograph => "Pantograph",
        RelicKey::StrikeDummy => "Strike Dummy",
        RelicKey::WhiteBeastStatue => "White Beast Statue",
        RelicKey::SmilingMask => "Smiling Mask",
        RelicKey::Whetstone => "Whetstone",
        RelicKey::Orichalcum => "Orichalcum",
        RelicKey::BronzeScales => "Bronze Scales",
        RelicKey::Ginger => "Ginger",
        RelicKey::Strawberry => "Strawberry",
        RelicKey::TungstenRod => "Tungsten Rod",
        RelicKey::MagicFlower => "Magic Flower",
        RelicKey::ToyOrnithopter => "Toy Ornithopter",
        RelicKey::BirdFacedUrn => "Bird-Faced Urn",
        RelicKey::UnceasingTop => "Unceasing Top",
        RelicKey::Toolbox => "Toolbox",
        RelicKey::PotionBelt => "Potion Belt",
        RelicKey::RegalPillow => "Regal Pillow",
        RelicKey::Mango => "Mango",
        RelicKey::GremlinHorn => "Gremlin Horn",
        RelicKey::JuzuBracelet => "Juzu Bracelet",
        RelicKey::MawBank => "Maw Bank",
        RelicKey::Omamori => "Omamori",
        RelicKey::Lantern => "Lantern",
        RelicKey::AncientTeaSet => "Ancient Tea Set",
        RelicKey::Pocketwatch => "Pocketwatch",
        RelicKey::CentennialPuzzle => "Centennial Puzzle",
        RelicKey::OddlySmoothStone => "Oddly Smooth Stone",
        RelicKey::MeatOnTheBone => "Meat on the Bone",
        RelicKey::ClockworkSouvenir => "Clockwork Souvenir",
        RelicKey::Orrery => "Orrery",
        RelicKey::StoneCalendar => "Stone Calendar",
        RelicKey::IceCream => "Ice Cream",
        RelicKey::ChemicalX => "Chemical X",
        RelicKey::Calipers => "Calipers",
        RelicKey::QuestionCard => "Question Card",
        RelicKey::SingingBowl => "Singing Bowl",
        RelicKey::CursedKey => "Cursed Key",
        RelicKey::FusionHammer => "Fusion Hammer",
        RelicKey::VelvetChoker => "Velvet Choker",
        RelicKey::RunicDome => "Runic Dome",
        RelicKey::SlaversCollar => "Slaver's Collar",
        RelicKey::SneckoEye => "Snecko Eye",
        RelicKey::PandorasBox => "Pandora's Box",
        RelicKey::BustedCrown => "Busted Crown",
        RelicKey::Ectoplasm => "Ectoplasm",
        RelicKey::TinyHouse => "Tiny House",
        RelicKey::Cauldron => "Cauldron",
        RelicKey::Sozu => "Sozu",
        RelicKey::PhilosophersStone => "Philosopher's Stone",
        RelicKey::Astrolabe => "Astrolabe",
        RelicKey::BlackStar => "Black Star",
        RelicKey::SacredBark => "Sacred Bark",
        RelicKey::EmptyCage => "Empty Cage",
        RelicKey::RunicPyramid => "Runic Pyramid",
        RelicKey::CallingBell => "Calling Bell",
        RelicKey::CoffeeDripper => "Coffee Dripper",
        RelicKey::BlackBlood => "Black Blood",
        RelicKey::Brimstone => "Brimstone",
        RelicKey::RedMask => "Red Mask",
        RelicKey::EternalFeather => "Eternal Feather",
        RelicKey::Pear => "Pear",
        RelicKey::MarkOfPain => "Mark of Pain",
        RelicKey::RunicCube => "Runic Cube",
        RelicKey::DeadBranch => "Dead Branch",
        RelicKey::MealTicket => "Meal Ticket",
        RelicKey::PrismaticShard => "Prismatic Shard",
        RelicKey::ChampionBelt => "Champion Belt",
        RelicKey::GoldenIdol => "Golden Idol",
        RelicKey::DuVuDoll => "Du-Vu Doll",
        RelicKey::MedicalKit => "Medical Kit",
        RelicKey::WarPaint => "War Paint",
        RelicKey::LetterOpener => "Letter Opener",
        RelicKey::PreservedInsect => "Preserved Insect",
        RelicKey::SlingOfCourage => "Sling of Courage",
        RelicKey::ArtOfWar => "Art of War",
        RelicKey::PrayerWheel => "Prayer Wheel",
        RelicKey::CaptainsWheel => "Captain's Wheel",
        RelicKey::LizardTail => "Lizard Tail",
        RelicKey::Nunchaku => "Nunchaku",
        RelicKey::InkBottle => "Ink Bottle",
        RelicKey::Shuriken => "Shuriken",
        RelicKey::Kunai => "Kunai",
        RelicKey::HappyFlower => "Happy Flower",
        RelicKey::IncenseBurner => "Incense Burner",
        RelicKey::ThreadAndNeedle => "Thread and Needle",
        RelicKey::FossilizedHelix => "Fossilized Helix",
        RelicKey::PeacePipe => "Peace Pipe",
        RelicKey::PaperPhrog => "Paper Phrog",
        RelicKey::StrangeSpoon => "Strange Spoon",
        RelicKey::DollysMirror => "Dolly's Mirror",
        RelicKey::SelfFormingClay => "Self-Forming Clay",
        RelicKey::OrangePellets => "Orange Pellets",
        RelicKey::Matryoshka => "Matryoshka",
        RelicKey::BlueCandle => "Blue Candle",
        RelicKey::BottledLightning => "Bottled Lightning",
        RelicKey::WingBoots => "Wing Boots",
        RelicKey::CultistMask => "Cultist Headpiece",
        RelicKey::FaceOfCleric => "FaceOfCleric",
        RelicKey::GremlinMask => "GremlinMask",
        RelicKey::Girya => "Girya",
        RelicKey::NlothsMask => "NlothsMask",
        RelicKey::SsserpentHead => "Ssserpent Head",
        RelicKey::WarpedTongs => "Warped Tongs",
    }
}

fn relic_key_from_trace_name(name: &str) -> Option<RelicKey> {
    match normalized_trace_relic_name(name).as_str() {
        "akabeko" => Some(RelicKey::Akabeko),
        "crackedcore" => Some(RelicKey::CrackedCore),
        "ringofthesnake" => Some(RelicKey::RingOfTheSnake),
        "purewater" => Some(RelicKey::PureWater),
        "vajra" => Some(RelicKey::Vajra),
        "bottledtornado" => Some(RelicKey::BottledTornado),
        "sundial" => Some(RelicKey::Sundial),
        "thecourier" => Some(RelicKey::TheCourier),
        "ornamentalfan" => Some(RelicKey::OrnamentalFan),
        "horncleat" => Some(RelicKey::HornCleat),
        "bottledflame" => Some(RelicKey::BottledFlame),
        "darkstoneperiapt" => Some(RelicKey::DarkstonePeriapt),
        "mercuryhourglass" => Some(RelicKey::MercuryHourglass),
        "oldcoin" => Some(RelicKey::OldCoin),
        "shovel" => Some(RelicKey::Shovel),
        "turnip" => Some(RelicKey::Turnip),
        "frozencore" => Some(RelicKey::FrozenCore),
        "ringoftheserpent" => Some(RelicKey::RingOfTheSerpent),
        "holywater" => Some(RelicKey::HolyWater),
        "handdrill" => Some(RelicKey::HandDrill),
        "leeswaffle" => Some(RelicKey::LeesWaffle),
        "frozeneye" => Some(RelicKey::FrozenEye),
        "theabacus" => Some(RelicKey::TheAbacus),
        "necronomicon" => Some(RelicKey::Necronomicon),
        "enchiridion" => Some(RelicKey::Enchiridion),
        "nilryscodex" => Some(RelicKey::NilrysCodex),
        "mutagenicstrength" => Some(RelicKey::MutagenicStrength),
        "bloodyidol" => Some(RelicKey::BloodyIdol),
        "markofthebloom" | "markofbloom" => Some(RelicKey::MarkOfBloom),
        "spiritpoop" => Some(RelicKey::SpiritPoop),
        "oddmushroom" => Some(RelicKey::OddMushroom),
        "nlothsgift" => Some(RelicKey::NlothsGift),
        "circlet" => Some(RelicKey::Circlet),
        "redcirclet" => Some(RelicKey::RedCirclet),
        "anchor" => Some(RelicKey::Anchor),
        "theboot" => Some(RelicKey::TheBoot),
        "tinychest" => Some(RelicKey::TinyChest),
        "bagofmarbles" => Some(RelicKey::BagOfMarbles),
        "bagofpreparation" => Some(RelicKey::BagOfPreparation),
        "burningblood" => Some(RelicKey::BurningBlood),
        "bloodvial" => Some(RelicKey::BloodVial),
        "redskull" => Some(RelicKey::RedSkull),
        "dreamcatcher" => Some(RelicKey::DreamCatcher),
        "torii" => Some(RelicKey::Torii),
        "moltenegg" | "moltenegg2" => Some(RelicKey::MoltenEgg),
        "toxicegg" => Some(RelicKey::ToxicEgg),
        "frozenegg" | "frozenegg2" => Some(RelicKey::FrozenEgg),
        "mummifiedhand" => Some(RelicKey::MummifiedHand),
        "charonsashes" => Some(RelicKey::CharonsAshes),
        "ceramicfish" => Some(RelicKey::CeramicFish),
        "gamblingchip" => Some(RelicKey::GamblingChip),
        "pennib" => Some(RelicKey::PenNib),
        "membershipcard" => Some(RelicKey::MembershipCard),
        "pantograph" => Some(RelicKey::Pantograph),
        "strikedummy" => Some(RelicKey::StrikeDummy),
        "whitebeaststatue" => Some(RelicKey::WhiteBeastStatue),
        "smilingmask" => Some(RelicKey::SmilingMask),
        "whetstone" => Some(RelicKey::Whetstone),
        "orichalcum" => Some(RelicKey::Orichalcum),
        "bronzescales" => Some(RelicKey::BronzeScales),
        "ginger" => Some(RelicKey::Ginger),
        "strawberry" => Some(RelicKey::Strawberry),
        "tungstenrod" => Some(RelicKey::TungstenRod),
        "magicflower" => Some(RelicKey::MagicFlower),
        "toyornithopter" => Some(RelicKey::ToyOrnithopter),
        "birdfacedurn" => Some(RelicKey::BirdFacedUrn),
        "unceasingtop" => Some(RelicKey::UnceasingTop),
        "toolbox" => Some(RelicKey::Toolbox),
        "potionbelt" => Some(RelicKey::PotionBelt),
        "regalpillow" => Some(RelicKey::RegalPillow),
        "mango" => Some(RelicKey::Mango),
        "gremlinhorn" => Some(RelicKey::GremlinHorn),
        "juzubracelet" => Some(RelicKey::JuzuBracelet),
        "mawbank" => Some(RelicKey::MawBank),
        "omamori" => Some(RelicKey::Omamori),
        "lantern" => Some(RelicKey::Lantern),
        "ancientteaset" => Some(RelicKey::AncientTeaSet),
        "pocketwatch" => Some(RelicKey::Pocketwatch),
        "centennialpuzzle" => Some(RelicKey::CentennialPuzzle),
        "oddlysmoothstone" => Some(RelicKey::OddlySmoothStone),
        "meatonthebone" => Some(RelicKey::MeatOnTheBone),
        "clockworksouvenir" => Some(RelicKey::ClockworkSouvenir),
        "orrery" => Some(RelicKey::Orrery),
        "stonecalendar" => Some(RelicKey::StoneCalendar),
        "icecream" => Some(RelicKey::IceCream),
        "chemicalx" => Some(RelicKey::ChemicalX),
        "calipers" => Some(RelicKey::Calipers),
        "questioncard" => Some(RelicKey::QuestionCard),
        "singingbowl" => Some(RelicKey::SingingBowl),
        "cursedkey" => Some(RelicKey::CursedKey),
        "fusionhammer" => Some(RelicKey::FusionHammer),
        "velvetchoker" => Some(RelicKey::VelvetChoker),
        "runicdome" => Some(RelicKey::RunicDome),
        "slaverscollar" => Some(RelicKey::SlaversCollar),
        "sneckoeye" => Some(RelicKey::SneckoEye),
        "pandorasbox" => Some(RelicKey::PandorasBox),
        "bustedcrown" => Some(RelicKey::BustedCrown),
        "ectoplasm" => Some(RelicKey::Ectoplasm),
        "tinyhouse" => Some(RelicKey::TinyHouse),
        "sozu" => Some(RelicKey::Sozu),
        "philosophersstone" => Some(RelicKey::PhilosophersStone),
        "astrolabe" => Some(RelicKey::Astrolabe),
        "blackstar" => Some(RelicKey::BlackStar),
        "sacredbark" => Some(RelicKey::SacredBark),
        "emptycage" => Some(RelicKey::EmptyCage),
        "runicpyramid" => Some(RelicKey::RunicPyramid),
        "callingbell" => Some(RelicKey::CallingBell),
        "cauldron" => Some(RelicKey::Cauldron),
        "coffeedripper" => Some(RelicKey::CoffeeDripper),
        "blackblood" => Some(RelicKey::BlackBlood),
        "brimstone" => Some(RelicKey::Brimstone),
        "redmask" => Some(RelicKey::RedMask),
        "markofpain" => Some(RelicKey::MarkOfPain),
        "runiccube" => Some(RelicKey::RunicCube),
        "deadbranch" => Some(RelicKey::DeadBranch),
        "mealticket" => Some(RelicKey::MealTicket),
        "prismaticshard" => Some(RelicKey::PrismaticShard),
        "threadandneedle" => Some(RelicKey::ThreadAndNeedle),
        "paperphrog" => Some(RelicKey::PaperPhrog),
        "strangespoon" => Some(RelicKey::StrangeSpoon),
        "dollysmirror" => Some(RelicKey::DollysMirror),
        "selfformingclay" => Some(RelicKey::SelfFormingClay),
        "orangepellets" => Some(RelicKey::OrangePellets),
        "matryoshka" => Some(RelicKey::Matryoshka),
        "bluecandle" => Some(RelicKey::BlueCandle),
        "bottledlightning" => Some(RelicKey::BottledLightning),
        "wingboots" => Some(RelicKey::WingBoots),
        "cultistmask" | "cultistheadpiece" => Some(RelicKey::CultistMask),
        "faceofcleric" | "clericface" => Some(RelicKey::FaceOfCleric),
        "gremlinmask" | "gremlinvisage" => Some(RelicKey::GremlinMask),
        "girya" => Some(RelicKey::Girya),
        "nlothsmask" => Some(RelicKey::NlothsMask),
        "ssserpenthead" => Some(RelicKey::SsserpentHead),
        "warpedtongs" => Some(RelicKey::WarpedTongs),
        "pear" => Some(RelicKey::Pear),
        "eternalfeather" => Some(RelicKey::EternalFeather),
        "championbelt" => Some(RelicKey::ChampionBelt),
        "goldenidol" => Some(RelicKey::GoldenIdol),
        "duvudoll" => Some(RelicKey::DuVuDoll),
        "medicalkit" => Some(RelicKey::MedicalKit),
        "warpaint" => Some(RelicKey::WarPaint),
        "letteropener" => Some(RelicKey::LetterOpener),
        "preservedinsect" => Some(RelicKey::PreservedInsect),
        "slingofcourage" => Some(RelicKey::SlingOfCourage),
        "artofwar" => Some(RelicKey::ArtOfWar),
        "prayerwheel" => Some(RelicKey::PrayerWheel),
        "captainswheel" => Some(RelicKey::CaptainsWheel),
        "lizardtail" => Some(RelicKey::LizardTail),
        "nunchaku" => Some(RelicKey::Nunchaku),
        "inkbottle" => Some(RelicKey::InkBottle),
        "shuriken" => Some(RelicKey::Shuriken),
        "kunai" => Some(RelicKey::Kunai),
        "happyflower" => Some(RelicKey::HappyFlower),
        "incenseburner" => Some(RelicKey::IncenseBurner),
        "fossilizedhelix" => Some(RelicKey::FossilizedHelix),
        "peacepipe" => Some(RelicKey::PeacePipe),
        _ => None,
    }
}

#[cfg(test)]
fn relic_from_trace_name(name: &str) -> Option<Relic> {
    match normalized_trace_relic_name(name).as_str() {
        "akabeko" => Some(Relic::Akabeko),
        "crackedcore" => Some(Relic::CrackedCore),
        "ringofthesnake" => Some(Relic::RingOfTheSnake),
        "purewater" => Some(Relic::PureWater),
        "vajra" => Some(Relic::Vajra),
        "bottledtornado" => Some(Relic::BottledTornado),
        "sundial" => Some(Relic::Sundial),
        "thecourier" => Some(Relic::TheCourier),
        "ornamentalfan" => Some(Relic::OrnamentalFan),
        "horncleat" => Some(Relic::HornCleat),
        "bottledflame" => Some(Relic::BottledFlame),
        "darkstoneperiapt" => Some(Relic::DarkstonePeriapt),
        "mercuryhourglass" => Some(Relic::MercuryHourglass),
        "oldcoin" => Some(Relic::OldCoin),
        "shovel" => Some(Relic::Shovel),
        "turnip" => Some(Relic::Turnip),
        "frozencore" => Some(Relic::FrozenCore),
        "ringoftheserpent" => Some(Relic::RingOfTheSerpent),
        "holywater" => Some(Relic::HolyWater),
        "handdrill" => Some(Relic::HandDrill),
        "leeswaffle" => Some(Relic::LeesWaffle),
        "frozeneye" => Some(Relic::FrozenEye),
        "theabacus" => Some(Relic::TheAbacus),
        "necronomicon" => Some(Relic::Necronomicon),
        "enchiridion" => Some(Relic::Enchiridion),
        "nilryscodex" => Some(Relic::NilrysCodex),
        "mutagenicstrength" => Some(Relic::MutagenicStrength),
        "bloodyidol" => Some(Relic::BloodyIdol),
        "circlet" => Some(Relic::Circlet),
        "redcirclet" => Some(Relic::RedCirclet),
        "anchor" => Some(Relic::Anchor),
        "theboot" => Some(Relic::TheBoot),
        "tinychest" => Some(Relic::TinyChest),
        "bagofmarbles" => Some(Relic::BagOfMarbles),
        "bagofpreparation" => Some(Relic::BagOfPreparation),
        "burningblood" => Some(Relic::BurningBlood),
        "bloodvial" => Some(Relic::BloodVial),
        "redskull" => Some(Relic::RedSkull),
        "dreamcatcher" => Some(Relic::DreamCatcher),
        "torii" => Some(Relic::Torii),
        "moltenegg" | "moltenegg2" => Some(Relic::MoltenEgg),
        "toxicegg" => Some(Relic::ToxicEgg),
        "frozenegg" | "frozenegg2" => Some(Relic::FrozenEgg),
        "mummifiedhand" => Some(Relic::MummifiedHand),
        "charonsashes" => Some(Relic::CharonsAshes),
        "ceramicfish" => Some(Relic::CeramicFish),
        "gamblingchip" => Some(Relic::GamblingChip),
        "pennib" => Some(Relic::PenNib),
        "membershipcard" => Some(Relic::MembershipCard),
        "pantograph" => Some(Relic::Pantograph),
        "whetstone" => Some(Relic::Whetstone),
        "orichalcum" => Some(Relic::Orichalcum),
        "bronzescales" => Some(Relic::BronzeScales),
        "ginger" => Some(Relic::Ginger),
        "strawberry" => Some(Relic::Strawberry),
        "tungstenrod" => Some(Relic::TungstenRod),
        "magicflower" => Some(Relic::MagicFlower),
        "toyornithopter" => Some(Relic::ToyOrnithopter),
        "birdfacedurn" => Some(Relic::BirdFacedUrn),
        "unceasingtop" => Some(Relic::UnceasingTop),
        "toolbox" => Some(Relic::Toolbox),
        "potionbelt" => Some(Relic::PotionBelt),
        "mango" => Some(Relic::Mango),
        "gremlinhorn" => Some(Relic::GremlinHorn),
        "mawbank" => Some(Relic::MawBank),
        "omamori" => Some(Relic::Omamori),
        "lantern" => Some(Relic::Lantern),
        "ancientteaset" => Some(Relic::AncientTeaSet),
        "pocketwatch" => Some(Relic::Pocketwatch),
        "centennialpuzzle" => Some(Relic::CentennialPuzzle),
        "oddlysmoothstone" => Some(Relic::OddlySmoothStone),
        "meatonthebone" => Some(Relic::MeatOnTheBone),
        "clockworksouvenir" => Some(Relic::ClockworkSouvenir),
        "stonecalendar" => Some(Relic::StoneCalendar),
        "icecream" => Some(Relic::IceCream),
        "chemicalx" => Some(Relic::ChemicalX),
        "calipers" => Some(Relic::Calipers),
        "questioncard" => Some(Relic::QuestionCard),
        "singingbowl" => Some(Relic::SingingBowl),
        "cursedkey" => Some(Relic::CursedKey),
        "fusionhammer" => Some(Relic::FusionHammer),
        "velvetchoker" => Some(Relic::VelvetChoker),
        "runicdome" => Some(Relic::RunicDome),
        "slaverscollar" => Some(Relic::SlaversCollar),
        "sneckoeye" => Some(Relic::SneckoEye),
        "pandorasbox" => Some(Relic::PandorasBox),
        "bustedcrown" => Some(Relic::BustedCrown),
        "ectoplasm" => Some(Relic::Ectoplasm),
        "tinyhouse" => Some(Relic::TinyHouse),
        "sozu" => Some(Relic::Sozu),
        "philosophersstone" => Some(Relic::PhilosophersStone),
        "astrolabe" => Some(Relic::Astrolabe),
        "blackstar" => Some(Relic::BlackStar),
        "sacredbark" => Some(Relic::SacredBark),
        "emptycage" => Some(Relic::EmptyCage),
        "runicpyramid" => Some(Relic::RunicPyramid),
        "callingbell" => Some(Relic::CallingBell),
        "coffeedripper" => Some(Relic::CoffeeDripper),
        "blackblood" => Some(Relic::BlackBlood),
        "brimstone" => Some(Relic::Brimstone),
        "markofpain" => Some(Relic::MarkOfPain),
        "runiccube" => Some(Relic::RunicCube),
        "deadbranch" => Some(Relic::DeadBranch),
        "mealticket" => Some(Relic::MealTicket),
        "prismaticshard" => Some(Relic::PrismaticShard),
        "threadandneedle" => Some(Relic::ThreadAndNeedle),
        "paperphrog" => Some(Relic::PaperPhrog),
        "strangespoon" => Some(Relic::StrangeSpoon),
        "dollysmirror" => Some(Relic::DollysMirror),
        "selfformingclay" => Some(Relic::SelfFormingClay),
        "bluecandle" => Some(Relic::BlueCandle),
        "bottledlightning" => Some(Relic::BottledLightning),
        "wingboots" => Some(Relic::WingBoots),
        "cultistmask" | "cultistheadpiece" => Some(Relic::CultistMask),
        "faceofcleric" | "clericface" => Some(Relic::FaceOfCleric),
        "gremlinmask" | "gremlinvisage" => Some(Relic::GremlinMask),
        "girya" => Some(Relic::Girya),
        "nlothsmask" => Some(Relic::NlothsMask),
        "ssserpenthead" => Some(Relic::SsserpentHead),
        "pear" => Some(Relic::Pear),
        "eternalfeather" => Some(Relic::EternalFeather),
        "championbelt" => Some(Relic::ChampionBelt),
        "goldenidol" => Some(Relic::GoldenIdol),
        "duvudoll" => Some(Relic::DuVuDoll),
        "medicalkit" => Some(Relic::MedicalKit),
        "warpaint" => Some(Relic::WarPaint),
        "letteropener" => Some(Relic::LetterOpener),
        "preservedinsect" => Some(Relic::PreservedInsect),
        "slingofcourage" => Some(Relic::SlingOfCourage),
        "artofwar" => Some(Relic::ArtOfWar),
        "prayerwheel" => Some(Relic::PrayerWheel),
        "captainswheel" => Some(Relic::CaptainsWheel),
        "lizardtail" => Some(Relic::LizardTail),
        "nunchaku" => Some(Relic::Nunchaku),
        "inkbottle" => Some(Relic::InkBottle),
        "shuriken" => Some(Relic::Shuriken),
        "kunai" => Some(Relic::Kunai),
        "happyflower" => Some(Relic::HappyFlower),
        "incenseburner" => Some(Relic::IncenseBurner),
        "fossilizedhelix" => Some(Relic::FossilizedHelix),
        "peacepipe" => Some(Relic::PeacePipe),
        "warpedtongs" => Some(Relic::WarpedTongs),
        _ => None,
    }
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

#[cfg(test)]
#[allow(dead_code)]
fn potions_from_observed(game: &Value) -> Vec<Potion> {
    game.get("potions")
        .and_then(Value::as_array)
        .map(|potions| {
            potions
                .iter()
                .filter_map(|potion| {
                    let name = potion.get("name").and_then(Value::as_str)?;
                    if name.eq_ignore_ascii_case("Potion Slot") {
                        return None;
                    }
                    potion_from_trace_name(name)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(dead_code)]
fn empty_potion_slots_from_observed(game: &Value) -> Vec<usize> {
    game.get("potions")
        .and_then(Value::as_array)
        .map(|potions| {
            potions
                .iter()
                .enumerate()
                .filter_map(|(index, potion)| {
                    let name = potion.get("name").and_then(Value::as_str)?;
                    name.eq_ignore_ascii_case("Potion Slot").then_some(index)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn potion_keys_from_value(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|potions| {
            potions
                .iter()
                .filter_map(|potion| {
                    let name = potion.get("name").and_then(Value::as_str)?;
                    if name.eq_ignore_ascii_case("Potion Slot") {
                        return None;
                    }
                    potion_from_trace_name(name).map(|potion| potion_trace_name(potion).to_owned())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn relic_ids_for_simulated_subset(run: &RunState, carry: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    // Carry the observed order forward. This preserves relics that are represented
    // only by verifier state (such as Neow's Lament) relative to later pickups.
    for name in carry {
        if name != "Unknown Relic" && !out.contains(name) {
            out.push(name.clone());
        }
    }
    for relic in &run.relics {
        let name = relic_key_trace_name(relic.key()).to_owned();
        if name != "Unknown Relic" && !out.contains(&name) {
            out.push(name);
        }
    }
    for key in &run.relic_keys {
        let name = relic_key_trace_name(*key).to_owned();
        if name != "Unknown Relic" && !out.contains(&name) {
            out.push(name);
        }
    }
    remove_simulated_replaced_starter_relics(run, &mut out);
    if run.neow_lament_combats_remaining > 0 && !out.iter().any(|name| name == "Neow's Lament") {
        out.push("Neow's Lament".to_owned());
    }
    out
}

fn remove_simulated_replaced_starter_relics(run: &RunState, relics: &mut Vec<String>) {
    for (upgraded, starter) in [
        (RelicKey::BlackBlood, "Burning Blood"),
        (RelicKey::FrozenCore, "Cracked Core"),
        (RelicKey::HolyWater, "Pure Water"),
        (RelicKey::RingOfTheSerpent, "Ring of the Snake"),
    ] {
        if run_has_relic_key(run, upgraded) {
            let upgraded_name = relic_key_trace_name(upgraded).to_owned();
            if let Some(slot) = relics.iter_mut().find(|name| name.as_str() == starter) {
                *slot = upgraded_name.clone();
            }
            let mut seen = false;
            relics.retain(|name| {
                if name == &upgraded_name {
                    if seen {
                        false
                    } else {
                        seen = true;
                        true
                    }
                } else {
                    true
                }
            });
        }
    }
}

fn run_has_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relic_keys.contains(&key) || run.relics.iter().any(|relic| relic.key() == key)
}

fn seed_start_update_carry_from_run(
    run: &RunState,
    relics: &mut Vec<String>,
    deck_ids: &mut Vec<String>,
) {
    *deck_ids = deck_content_keys(&run.deck);
    remove_simulated_replaced_starter_relics(run, relics);
    for relic in &run.relics {
        let name = relic_key_trace_name(relic.key()).to_owned();
        if name != "Unknown Relic" && !relics.contains(&name) {
            relics.push(name);
        }
    }
    for key in &run.relic_keys {
        let name = relic_key_trace_name(*key).to_owned();
        if name != "Unknown Relic" && !relics.contains(&name) {
            relics.push(name);
        }
    }
}

fn seed_start_carried_run(
    carried: Option<&RunState>,
    numeric_seed: i64,
    ascension: u8,
    external_seed: &str,
    deck_ids: &[String],
) -> RunState {
    if let Some(sim) = carried {
        let mut next = sim.clone();
        next.combat = None;
        next.reward = None;
        next.event = None;
        next.shop = None;
        next.shop_merchant_open = false;
        next.card_grid = None;
        next.phase = RunPhase::Idle;
        return next;
    }
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    run.reward_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    seed_start_apply_reward_rng_snapshot(&mut run, numeric_seed, external_seed, 0);
    run
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

fn seed_start_event_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    seed_start_event_simulated_subset_with_deck(run, relic_ids, deck_content_keys(&run.deck))
}

fn seed_start_event_simulated_subset_with_delayed_deck_append(
    run: &RunState,
    relic_ids: &[String],
    delayed_event_deck_append_count: Option<usize>,
) -> Value {
    if run.card_grid.is_some() {
        return seed_start_event_simulated_subset(run, relic_ids);
    }

    let Some(count) = delayed_event_deck_append_count else {
        return seed_start_event_simulated_subset(run, relic_ids);
    };
    let mut visible_deck = deck_content_keys(&run.deck);
    // Live event grids publish selected cards and transform results on the
    // next state poll. Core state is already complete, so project only the
    // action frame without those newly appended cards.
    visible_deck.truncate(visible_deck.len().saturating_sub(count));
    seed_start_event_simulated_subset_with_deck(run, relic_ids, visible_deck)
}

fn normalize_match_and_keep_transient_choices(
    run: &RunState,
    observed: &mut Value,
    simulated: &Value,
) {
    if !run
        .event
        .as_ref()
        .is_some_and(|screen| screen.event == Event::MatchAndKeep)
    {
        return;
    }
    let Some(observed_choices) = observed.get("choices").and_then(Value::as_array) else {
        return;
    };
    let Some(simulated_choices) = simulated.get("choices").and_then(Value::as_array) else {
        return;
    };
    let card_label = |value: &Value| {
        value.as_str().is_some_and(|label| {
            label
                .strip_prefix("card")
                .is_some_and(|number| number.parse::<usize>().is_ok())
        })
    };
    if observed_choices.iter().all(card_label)
        && simulated_choices.iter().all(card_label)
        && observed_choices
            .iter()
            .all(|choice| simulated_choices.contains(choice))
        && observed_choices.len() < simulated_choices.len()
    {
        if let Some(object) = observed.as_object_mut() {
            object.insert(
                "choices".to_owned(),
                Value::Array(simulated_choices.clone()),
            );
        }
        return;
    }

    // MatchAndKeep's grid can publish a stale duplicate card identity for one
    // frame while flipped cards turn back over. The available slots are still
    // authoritative: normalize only when every hidden `cardN` slot is in the
    // same position and the set of revealed identities is unchanged. Final
    // matching/deck effects remain independently verified.
    let same_hidden_slots = observed_choices.len() == simulated_choices.len()
        && observed_choices
            .iter()
            .zip(simulated_choices)
            .all(|(observed, simulated)| {
                if card_label(observed) || card_label(simulated) {
                    observed == simulated
                } else {
                    true
                }
            });
    let revealed_identity_set = |choices: &[Value]| {
        let mut identities = choices
            .iter()
            .filter(|choice| !card_label(choice))
            .filter_map(Value::as_str)
            .map(normalized_trace_relic_name)
            .collect::<Vec<_>>();
        identities.sort_unstable();
        identities.dedup();
        identities
    };
    if same_hidden_slots
        && revealed_identity_set(observed_choices) == revealed_identity_set(simulated_choices)
    {
        if let Some(object) = observed.as_object_mut() {
            object.insert(
                "choices".to_owned(),
                Value::Array(simulated_choices.clone()),
            );
        }
        return;
    }

    let Some(state) = run.match_and_keep.as_ref() else {
        return;
    };
    let Some(first_index) = state.first_flipped_index else {
        return;
    };
    let Some(second_index) = state.second_flipped_index else {
        return;
    };
    let mut observed_reveals = observed_choices
        .iter()
        .filter(|choice| !card_label(choice))
        .filter_map(Value::as_str)
        .map(normalized_trace_relic_name)
        .collect::<Vec<_>>();
    let simulated_reveals = simulated_choices
        .iter()
        .filter(|choice| !card_label(choice))
        .filter_map(Value::as_str)
        .map(normalized_trace_relic_name)
        .collect::<Vec<_>>();
    for reveal in simulated_reveals {
        if let Some(position) = observed_reveals
            .iter()
            .position(|candidate| *candidate == reveal)
        {
            observed_reveals.remove(position);
        } else {
            return;
        }
    }
    let mut expected_reveals = [first_index, second_index]
        .into_iter()
        .filter_map(|index| state.cards.get(index))
        .filter_map(|card| sts_core::content::cards::get_card_definition(card.content_id))
        .map(|definition| normalized_trace_relic_name(definition.name))
        .collect::<Vec<_>>();
    observed_reveals.sort();
    expected_reveals.sort();
    if observed_choices.len() == simulated_choices.len().saturating_add(2)
        && observed_reveals == expected_reveals
    {
        if let Some(object) = observed.as_object_mut() {
            object.insert(
                "choices".to_owned(),
                Value::Array(simulated_choices.clone()),
            );
        }
    }
}

fn seed_start_event_simulated_subset_with_deck(
    run: &RunState,
    relic_ids: &[String],
    deck_ids: Vec<String>,
) -> Value {
    let choices = run
        .event
        .as_ref()
        .map(|event| {
            event
                .choices
                .iter()
                .filter_map(|choice| seed_start_visible_event_choice_label(&choice.label))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let event_id = run
        .event
        .as_ref()
        .map(|event| match event.event {
            sts_core::Event::TheSsssserpent => "liarsgame".to_owned(),
            sts_core::Event::HypnotizingColoredMushrooms => "mushrooms".to_owned(),
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
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
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

fn seed_start_event_choice_index_for_communication_mod(
    run: &RunState,
    visible_choice_index: usize,
    pre_message: &Value,
) -> Option<usize> {
    if let Some(observed_choice_label) = pre_message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .map(|choices| choice_list_from_value(Some(choices)))
        .and_then(|choices| choices.get(visible_choice_index).cloned())
        .and_then(|choice| seed_start_visible_event_choice_label(&choice))
    {
        let matches = run
            .event
            .as_ref()?
            .choices
            .iter()
            .enumerate()
            .filter_map(|(choice_index, choice)| {
                (seed_start_visible_event_choice_label(&choice.label).as_deref()
                    == Some(observed_choice_label.as_str()))
                .then_some(choice_index)
            })
            .collect::<Vec<_>>();
        if let [choice_index] = matches.as_slice() {
            return Some(*choice_index);
        }
    }

    run.event
        .as_ref()?
        .choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| seed_start_visible_event_choice_label(&choice.label).is_some())
        .nth(visible_choice_index)
        .map(|(choice_index, _)| choice_index)
}

fn seed_start_visible_event_choice_label(label: &str) -> Option<String> {
    let mut label = label.to_ascii_lowercase();
    if let Some((visible, _effect_text)) = label.split_once(" (") {
        label = visible.to_owned();
    }
    label = label.trim_end_matches(['!', '?', '.', ':', ';']).to_owned();
    match label.as_str() {
        "locked" => None,
        "enter the light" => Some("enter".to_owned()),
        _ => Some(label),
    }
}

fn deck_instances_from_keys(deck_ids: &[String]) -> Vec<CardInstance> {
    deck_ids
        .iter()
        .enumerate()
        .filter_map(|(index, key)| {
            content_id_from_key(key)
                .map(|content_id| CardInstance::new(CardId::new(index as u64 + 1), content_id))
        })
        .collect()
}

fn seed_start_rng_boundaries() -> Vec<RngBoundary> {
    vec![
        RngBoundary {
            stream: "seed_conversion".to_owned(),
            save_counter: None,
            status: "source_backed".to_owned(),
            reason: "SeedHelper.getLong from the target 12-18-2022 desktop jar uppercases seed text, maps O to 0, and parses it with base-35 alphabet 0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ".to_owned(),
        },
        RngBoundary {
            stream: "neowRng".to_owned(),
            save_counter: None,
            status: "source_backed_options_with_partial_application".to_owned(),
            reason: "Neow option generation uses target-style NeowEvent.rng initialization from Settings.seed, visible slot order, and five option-screen draws. Seed-start branch dispatch uses generated selected options; CODEX04/TEST colorless choices, CODEX04 three-potion choices, VERIFY01 common relic identity, MANUAL01 immediate rare-card identity, simple-drawback rare relic identity, and M290001/M290008 transform identity are generated. Core helpers cover card, colorless, potion, fixed-tier relic, boss-swap, transform, grid, curse-combo card/relic, Neow's Lament combat carry state, and simple no-RNG reward/drawback surfaces. Synthetic verifier follow-ups now cover Calling Bell, Astrolabe, Pandora's Box, Empty Cage, and Tiny House boss-swap paths. Selected-trace coverage for many branch combinations and broad boss-swap selected-trace evidence remain partial/caveated.".to_owned(),
        },
        RngBoundary {
            stream: "mapRng".to_owned(),
            save_counter: None,
            status: "source_backed_topology_prefix".to_owned(),
            reason: "decoded Exordium mapRng initialization uses seed + actNum and MapGenerator topology reproduces captured VERIFY01 first choices x=1/x=2, CODEX04 first choices x=0/x=2/x=4/x=5, and CODEX04 chosen-path next choices x=3 then x=2/x=3; fixed generateMap rows are row 0 combat, row 8 treasure, and row 14 rest; generateRoomTypes, RoomTypeAssigner two-stage room-list construction, raw RandomXS128 Collections.shuffle prefix, and full VERIFY01/CODEX04 room-symbol placement match decoded target behavior and captured map payloads".to_owned(),
        },
        RngBoundary {
            stream: "monsterRng".to_owned(),
            save_counter: Some("monster_seed_count".to_owned()),
            status: "source_backed_normal_list_prefix".to_owned(),
            reason: "decoded Exordium normal encounter list generation covers weak encounters, strong encounter weights, first-strong exclusions, and no-repeat-last-two retries; room execution maps combat index to list entries and target spawn state covers Cultist, Jaw Worm, Small Slimes, and 2 Louse for captured VERIFY01/CODEX04/CODEX03 first-three prefixes".to_owned(),
        },
        RngBoundary {
            stream: "monsterHpRng".to_owned(),
            save_counter: Some("monster_seed_count".to_owned()),
            status: "source_backed_floor_prefix".to_owned(),
            reason: "decoded room transition reinitializes monsterHpRng with Settings.seed + floorNum; floor-1 Cultist HP rolls reproduce VERIFY01 49 and CODEX04 54, CODEX04 floor-2 Small Slimes rolls reproduce Spike Slime (S) 11 plus Acid Slime (M) 32, and CODEX04 floor-3 louse constructors reproduce 13/15 with bite-damage interleaving".to_owned(),
        },
        RngBoundary {
            stream: "shuffleRng".to_owned(),
            save_counter: Some("card_random_seed_count".to_owned()),
            status: "captured_branch".to_owned(),
            reason: "Selected Ironclad A0 starter and modified-deck first combats derive opening piles from the current master-deck order: CardGroup.shuffle seeds Java Collections.shuffle with shuffleRng.randomLong(), draw piles use top-of-pile semantics, and innate/bottled cards are placed on top before opening draw. Broader in-combat and post-END state parity must be proven by simulator transitions, not trace repair.".to_owned(),
        },
        RngBoundary {
            stream: "cardRewardRng".to_owned(),
            save_counter: Some("card_seed_count".to_owned()),
            status: "source_backed_full_pool".to_owned(),
            reason: "card reward rarity rolls use target-style cardRng.random(99) + cardRarityFactor thresholds, common/rare factor mutation, duplicate rerolls, and StsRng counter consumption over the full 72-card Ironclad reward pool; many pool entries are RNG-only until their card mechanics are implemented".to_owned(),
        },
        RngBoundary {
            stream: "rewardGoldRng".to_owned(),
            save_counter: Some("treasure_seed_count".to_owned()),
            status: "source_backed_normal_combat".to_owned(),
            reason: "normal-combat gold uses target-style treasureRng.random(10, 20) with StsRng counter persistence; VERIFY01 and CODEX04 seed-start reward screens are generated from simulation-driven reward RNG rather than pinned constants".to_owned(),
        },
        RngBoundary {
            stream: "relicRng".to_owned(),
            save_counter: Some("relic_seed_count".to_owned()),
            status: "source_backed_pool_selection_wired".to_owned(),
            reason: "relic tier rolls for normal/chest-style and elite rewards use target thresholds and persisted relic_seed_count; Ironclad relic pools initialize, pop, and filter like target; elite/chest/boss relic reward screens and shop relic offers are wired from persisted pool state. VERIFY01 Neow common relic identity and simple-drawback Neow rare relic identity are generated through the fixed-tier relic helper; curse-combo rare relics and boss-swap follow-ups remain partial/caveated".to_owned(),
        },
        RngBoundary {
            stream: "merchantRng".to_owned(),
            save_counter: Some("merchant_seed_count".to_owned()),
            status: "source_backed_shop_inventory".to_owned(),
            reason: "shop inventory uses target-style Shop.cpp layout: 5 class cards + 2 colorless cards with sale slot, 3 relics (2 tier rolls + shop tier), 3 potions, and card-remove pricing; merchantRng/cardRng/potionRng/relic pool state drive generation without regressing relic_rng_counter".to_owned(),
        },
        RngBoundary {
            stream: "eventRng".to_owned(),
            save_counter: Some("event_seed_count".to_owned()),
            status: "source_backed_event_pool_with_captured_branches".to_owned(),
            reason: "Act 1 event/shrine pools initialize from target EventPools::Act1 lists; generateEvent uses 25% shrine chance and removes picked entries; Golden Shrine, Cleric heal, Shining Light, and The Ssssserpent outcomes are implemented. TEST and M290001 still use captured event-entry branches where broader event RNG alignment is not yet proven".to_owned(),
        },
        RngBoundary {
            stream: "potionRng".to_owned(),
            save_counter: Some("potion_seed_count".to_owned()),
            status: "source_backed_reward_drop".to_owned(),
            reason: "normal reward potion drops use target-style potionRng.random(99), persisted potionChance, target rarity thresholds, and the full 33-potion Ironclad reward pool; potion use effects and broader potion RNG surfaces remain partial".to_owned(),
        },
    ]
}

#[cfg(test)]
fn seed_start_opening_piles_match(simulated: &CardPiles, message: &Value) -> bool {
    let Some(combat) = message
        .get("game_state")
        .and_then(|game| game.get("combat_state"))
    else {
        return false;
    };
    let observed_hand = combat_card_ids(combat.get("hand"));
    let observed_draw = combat_card_ids(combat.get("draw_pile"));
    let simulated_hand = hand_to_comm_mod_visible_order(&simulated.hand);
    let simulated_draw = draw_pile_to_comm_mod_visible_order(&simulated.draw_pile);
    observed_hand == simulated_hand && observed_draw == simulated_draw
}

fn seed_start_simulated_combat_subset(run: &RunState, end_turn_snapshot: bool) -> Value {
    seed_start_simulated_combat_subset_with_options(run, end_turn_snapshot, &[])
}

fn seed_start_run_has_combat_card_reward(run: &RunState) -> bool {
    run.combat.as_ref().is_some_and(|combat| {
        combat.potion_card_reward.is_some()
            || combat.discovery_card_reward.is_some()
            || combat.toolbox_card_reward.is_some()
    })
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
    let mut decisions = Vec::new();
    if seed_start_run_has_combat_card_reward(run) {
        decisions.push(SeedStartCombatDecision::CardReward);
    }
    if combat.hand_select.is_some() {
        decisions.push(SeedStartCombatDecision::HandSelect);
    }
    if combat.draw_select.is_some() {
        decisions.push(SeedStartCombatDecision::DrawSelect);
    }
    if combat.discard_select.is_some() {
        decisions.push(SeedStartCombatDecision::DiscardSelect);
    }
    if combat.exhaust_select.is_some() {
        decisions.push(SeedStartCombatDecision::ExhaustSelect);
    }
    match decisions.as_slice() {
        [] => Ok(None),
        [decision] => Ok(Some(*decision)),
        _ => Err(format!(
            "combat exposes multiple authoritative decisions: {decisions:?}"
        )),
    }
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

fn seed_start_simulated_map_combat_subset(
    run: &RunState,
    relics: &[String],
    _normal_combat_index: usize,
) -> Value {
    seed_start_simulated_combat_subset_with_options(run, false, relics)
}

fn seed_start_simulated_combat_subset_with_options(
    run: &RunState,
    end_turn_snapshot: bool,
    relics: &[String],
) -> Value {
    let Some(combat) = run.combat.as_ref() else {
        return json!({
            "screen_type": "NO_COMBAT",
            "floor": run.current_floor,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "potion_ids": run.potions.iter().map(|potion| potion_trace_name(*potion)).collect::<Vec<_>>(),
            "combat_player_hp": run.player_hp,
            "combat_player_block": 0,
            "combat_player_energy": 0,
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
    let monster_intents_visible = !run_has_relic_key(run, RelicKey::RunicDome);
    let mut subset = json!({
        "screen_type": screen_type,
        "ascension": run.ascension,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": combat.player.hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relics),
        "potion_ids": run.potions.iter().map(|potion| potion_trace_name(*potion)).collect::<Vec<_>>(),
        "combat_player_hp": combat.player.hp,
        "combat_player_block": combat.player.block,
        "combat_player_energy": combat.player.energy,
        "hand_ids": cards_to_comm_mod_visible_order(
            combat
                .piles
                .hand
                .iter()
                .enumerate()
                .filter(|(index, card)| {
                    let hidden_by_hand_select = combat.hand_select.as_ref().is_some_and(|hand_select| {
                        card.id == hand_select.source_card_id
                            || hand_select.selected_hand_index == Some(*index)
                            || (hand_select.purpose == HandSelectPurpose::ArmamentsUpgrade
                                && upgrade_content_id(card.content_id).is_none())
                            || (hand_select.purpose == HandSelectPurpose::DualWieldCopy
                                && card_type_and_rarity(card.content_id).is_none_or(|(card_type, _)| {
                                    !matches!(card_type, CardType::Attack | CardType::Power)
                                }))
                    });
                    let hidden_by_exhaust_select = combat
                        .exhaust_select
                        .as_ref()
                        .is_some_and(|exhaust_select| exhaust_select.selected_hand_indices.contains(index));
                    !hidden_by_hand_select && !hidden_by_exhaust_select
                })
                .map(|(_, card)| card),
        ),
        "draw_ids": draw_pile_to_comm_mod_visible_order(&combat.piles.draw_pile),
        "discard_ids": discard_pile_to_comm_mod_visible_order(&combat.piles.discard_pile),
        "monster_intents_visible": monster_intents_visible,
        "monsters": seed_start_monsters_from_sim(
            combat,
            end_turn_snapshot,
            monster_intents_visible,
        ),
    });
    if let Some(choices) = combat
        .potion_card_reward
        .as_ref()
        .or(combat.discovery_card_reward.as_ref())
        .or(combat.toolbox_card_reward.as_ref())
    {
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
    } else if combat.potion_card_reward.is_some()
        || combat.discovery_card_reward.is_some()
        || combat.toolbox_card_reward.is_some()
    {
        "CARD_REWARD"
    } else if combat.hand_select.is_some()
        || combat
            .exhaust_select
            .as_ref()
            .is_some_and(|select| select.purpose != ExhaustSelectPurpose::ExhumeReturnToHand)
    {
        "HAND_SELECT"
    } else if combat.draw_select.is_some()
        || combat.discard_select.is_some()
        || combat
            .exhaust_select
            .as_ref()
            .is_some_and(|select| select.purpose == ExhaustSelectPurpose::ExhumeReturnToHand)
    {
        "GRID"
    } else {
        "NONE"
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn combat_reward_card_display_key(combat: &CombatState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        ARMAMENTS_ID, FLEX_ID, METALLICIZE_ID, OFFERING_ID, SHRUG_IT_OFF_ID, WARCRY_PLUS_ID,
    };
    if content_id == WARCRY_PLUS_ID {
        return "Warcry+";
    }
    if combat
        .relics
        .iter()
        .any(|relic| relic.key() == RelicKey::ToxicEgg)
    {
        if content_id == ARMAMENTS_ID {
            return "Armaments+";
        }
        if content_id == METALLICIZE_ID {
            return "Metallicize+";
        }
        if content_id == FLEX_ID {
            return "Flex+";
        }
        if content_id == OFFERING_ID {
            return "Offering+";
        }
        if content_id == SHRUG_IT_OFF_ID {
            return "Shrug It Off+";
        }
    }
    deck_content_key(content_id)
}

fn seed_start_victory_observed_subset(message: &Value) -> Value {
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

fn seed_start_is_final_boss_victory(run: &RunState) -> bool {
    run.current_act == 3 && run.current_room_kind() == Some(RoomKind::Boss)
}

fn seed_start_is_boss_chest_proceed(run: &RunState) -> bool {
    run.phase == RunPhase::Reward
        && run.current_room_kind() == Some(RoomKind::Boss)
        && !seed_start_is_final_boss_victory(run)
}

fn seed_start_apply_final_boss_proceed(run: &RunState) -> Result<RunState, String> {
    let previous_floor = run.current_floor;
    let next = apply_run_action(run, RunAction::Proceed).map_err(|err| err.to_string())?;
    if next.phase != RunPhase::Event
        || next.current_room_kind() != Some(RoomKind::Victory)
        || !next
            .event
            .as_ref()
            .is_some_and(|event| event.event == Event::SpireHeart && event.stage == 0)
        || next.current_floor != previous_floor + 1
    {
        return Err(format!(
            "final boss proceed produced phase {:?}, room {:?}, event {:?}, floor {} from {}",
            next.phase,
            next.current_room_kind(),
            next.event.as_ref().map(|event| (event.event, event.stage)),
            next.current_floor,
            previous_floor
        ));
    }
    Ok(next)
}

fn seed_start_spire_heart_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "event_id": game.pointer("/screen_state/event_id").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
    })
}

fn seed_start_spire_heart_simulated_subset(run: &RunState) -> Value {
    let event_id = run
        .event
        .as_ref()
        .filter(|event| event.event == Event::SpireHeart)
        .map(|_| "Spire Heart")
        .unwrap_or("");
    json!({
        "screen_type": "EVENT",
        "event_id": event_id,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
    })
}

fn seed_start_game_over_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "victory": game.pointer("/screen_state/victory").and_then(Value::as_bool),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
    })
}

fn seed_start_game_over_simulated_subset(run: &RunState) -> Value {
    let victory = run.phase == RunPhase::Complete
        && run
            .event
            .as_ref()
            .is_some_and(|event| event.event == Event::SpireHeart && event.stage == 4);
    json!({
        "screen_type": if victory { "GAME_OVER" } else { "" },
        "victory": victory,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
    })
}

fn seed_start_victory_simulated_subset(run: &RunState) -> Value {
    let screen_type = if seed_start_is_final_boss_victory(run) {
        "COMPLETE"
    } else {
        "COMBAT_REWARD"
    };
    json!({
        "screen_type": screen_type,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
    })
}

fn seed_start_apply_reward_rng_snapshot(
    run: &mut RunState,
    numeric_seed: i64,
    external_seed: &str,
    combat_index: usize,
) {
    let _ = (external_seed, combat_index);
    run.reward_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    run.current_act = 1;
}

fn seed_start_reward_sequence_complete(run: &RunState) -> bool {
    let Some(reward) = run.reward.as_ref() else {
        return true;
    };
    if reward.card_reward_active {
        return reward.choices.is_empty();
    }
    reward.gold_offer == 0
        && reward.stolen_gold_offer == 0
        && reward.potion_offer.is_none()
        && reward.potion_offers.is_empty()
        && reward.relic_offer.is_none()
        && reward.relic_key_offer.is_none()
        && reward.pending_relic_offer.is_none()
        && reward.pending_relic_key_offer.is_none()
        && !reward.card_reward_pending
        && reward.choices.is_empty()
}

fn seed_start_phase_after_reward_completion(run: &RunState) -> SeedStartPhase {
    if run.phase == RunPhase::Shop {
        SeedStartPhase::Shop
    } else if run.phase == RunPhase::Rest {
        SeedStartPhase::Rest
    } else if run.current_room_kind() == Some(RoomKind::Boss) && run.boss_chest_opened {
        SeedStartPhase::Treasure
    } else if run.phase == RunPhase::Reward && run.event.is_some() {
        // Event-owned reward screens still need their explicit PROCEED action
        // before the core can return to the event continuation.
        SeedStartPhase::Reward
    } else {
        SeedStartPhase::Proceed
    }
}

fn sim_reward_combat_choices(reward: &RewardScreen) -> Vec<String> {
    let mut choices = Vec::new();
    let has_relic = reward.relic_offer.is_some() || reward.relic_key_offer.is_some();
    let has_pending_relic =
        reward.pending_relic_offer.is_some() || reward.pending_relic_key_offer.is_some();
    if has_relic && has_pending_relic && reward.gold_offer > 0 {
        choices.push("relic".to_owned());
        choices.push("gold".to_owned());
        choices.push("relic".to_owned());
        return choices;
    }
    if reward.gold_offer > 0 {
        choices.push("gold".to_owned());
    }
    if reward.stolen_gold_offer > 0 {
        choices.push("stolen_gold".to_owned());
    }
    if has_relic {
        choices.push("relic".to_owned());
        if has_pending_relic {
            choices.push("relic".to_owned());
        }
        choices.extend(std::iter::repeat_n(
            "relic".to_owned(),
            reward.queued_relic_key_offers.len(),
        ));
    }
    if !reward.potion_offers.is_empty() {
        choices.extend(std::iter::repeat_n(
            "potion".to_owned(),
            reward.potion_offers.len(),
        ));
    } else if reward.potion_offer.is_some() {
        choices.push("potion".to_owned());
    }
    if !reward.choices.is_empty() && !reward.card_reward_active {
        choices.push("card".to_owned());
    } else if reward.card_reward_pending && !reward.card_reward_active {
        choices.extend(std::iter::repeat_n(
            "card".to_owned(),
            reward.pending_card_reward_count() as usize,
        ));
    }
    choices
}

fn seed_start_apply_grid_command(sim: &RunState, command: &str) -> Result<RunState, String> {
    if command_head_eq(command, "CHOOSE") {
        let index = choose_index(command)
            .ok_or_else(|| format!("malformed grid CHOOSE command {command:?}"))?;
        select_grid_card(sim, index).map_err(|err| err.to_string())
    } else if command.eq_ignore_ascii_case("CONFIRM") {
        confirm_grid(sim).map_err(|err| err.to_string())
    } else if command.eq_ignore_ascii_case("CANCEL") {
        cancel_grid(sim).map_err(|err| err.to_string())
    } else {
        Err(format!("unsupported grid command {command:?}"))
    }
}

fn reward_types_from_combat_reward(message: &Value) -> Vec<String> {
    reward_types_from_value(
        message
            .get("game_state")
            .and_then(|game| game.get("screen_state"))
            .and_then(|screen| screen.get("rewards")),
    )
    .into_iter()
    .map(|reward_type| reward_type.to_ascii_lowercase())
    .collect()
}

fn seed_start_apply_reward_choose(
    sim: &mut RunState,
    command: &str,
    pre: &Value,
) -> Result<String, String> {
    let choose_index = choose_index(command)
        .ok_or_else(|| format!("seed-start verifier could not parse reward command {command:?}"))?;

    if sim.card_grid.is_some() {
        let next = select_grid_card(sim, choose_index).map_err(|err| err.to_string())?;
        *sim = next;
        return Ok(format!("reward grid select {choose_index}"));
    }

    if sim
        .reward
        .as_ref()
        .is_some_and(|reward| reward.card_reward_active)
    {
        let live_card_choices = choice_list_from_value(pre.pointer("/game_state/choice_list"));
        if live_card_choices
            .get(choose_index)
            .is_some_and(|choice| choice.eq_ignore_ascii_case("bowl"))
        {
            let next = apply_run_action(sim, RunAction::TakeSingingBowlReward)
                .map_err(|err| err.to_string())?;
            *sim = next;
            return Ok("singing bowl card reward".to_owned());
        }
        let card_id = sim
            .reward
            .as_ref()
            .and_then(|reward| reward.choices.get(choose_index))
            .map(|card| card.id)
            .ok_or_else(|| format!("reward card index {choose_index} is not available"))?;
        let next = apply_run_action(sim, RunAction::TakeCardReward { card_id })
            .map_err(|err| err.to_string())?;
        *sim = next;
        return Ok(format!("card reward pick {choose_index}"));
    }

    let observed_types = reward_types_from_combat_reward(pre);
    let choice = observed_types
        .get(choose_index)
        .cloned()
        .ok_or_else(|| format!("reward choice index {choose_index} is not available"))?;

    let potion_index = observed_types[..choose_index]
        .iter()
        .filter(|reward_type| reward_type.as_str() == "potion")
        .count();
    let next = match choice.as_str() {
        "stolen_gold" => apply_run_action(sim, RunAction::TakeStolenGoldReward),
        "gold" => apply_run_action(sim, RunAction::TakeGoldReward),
        "card" => apply_run_action(sim, RunAction::OpenCardReward),
        "potion" => apply_run_action(
            sim,
            RunAction::TakePotionReward {
                index: potion_index,
            },
        ),
        "relic" => {
            if let Some(observed) = pre
                .get("game_state")
                .and_then(observed_reward_relic_key_offer)
            {
                verify_primary_relic_offer_matches_observed(sim, observed)?;
            }
            apply_run_action(sim, RunAction::TakeRelicReward)
        }
        _ => return Err(format!("unknown reward choice {choice}")),
    }
    .map_err(|err| err.to_string())?;
    *sim = next;
    Ok(format!("{choice} reward"))
}

fn verify_primary_relic_offer_matches_observed(
    run: &RunState,
    observed: RelicKey,
) -> Result<(), String> {
    let predicted = run
        .reward
        .as_ref()
        .and_then(|reward| {
            reward
                .relic_offer
                .map(|relic| relic.key())
                .or(reward.relic_key_offer)
        })
        .ok_or_else(|| "no relic reward offered".to_owned())?;
    if predicted == observed {
        return Ok(());
    }
    Err(format!(
        "relic reward mismatch: observed {observed:?}, simulator predicted {predicted:?}; relic_rng_counter={} treasure_rng_counter={} treasure_room={:?}",
        run.relic_rng_counter,
        run.treasure_rng_counter,
        run.treasure_room,
    ))
}

fn seed_start_reward_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    if run.card_grid.is_some() {
        return seed_start_grid_simulated_subset(run, relic_ids);
    }
    let floor = run.current_floor;
    let relic_ids = relic_ids_for_simulated_subset(run, relic_ids);

    if run
        .reward
        .as_ref()
        .is_some_and(|reward| reward.card_reward_active)
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
            "relic_ids": relic_ids_for_simulated_subset(run, &relic_ids),
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
    let combat_choices = reward.map(sim_reward_combat_choices).unwrap_or_default();
    let reward_types: Vec<String> = combat_choices
        .iter()
        .map(|choice| match choice.as_str() {
            "gold" => "GOLD",
            "stolen_gold" => "STOLEN_GOLD",
            "potion" => "POTION",
            "card" => "CARD",
            "relic" => "RELIC",
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

fn seed_start_monsters_from_sim(
    combat: &CombatState,
    end_turn_snapshot: bool,
    intents_visible: bool,
) -> Vec<Value> {
    combat
        .monsters
        .iter()
        .map(|monster| {
            let name = seed_start_trace_monster_name(monster);
            let strength = monster.powers.strength;
            let vulnerable = monster.powers.vulnerable;
            if end_turn_snapshot {
                let _ = vulnerable;
            }
            let mut projected = json!({
                "name": name,
                "current_hp": monster.hp.max(0),
                "max_hp": monster.max_hp,
                "block": monster.block,
                "intent": seed_start_trace_intent(monster),
                "move_id": target_move_byte_for_monster(monster)
                    .map(i32::from)
                    .unwrap_or(-1),
                "strength": strength,
                "ritual": monster.powers.ritual,
                "vulnerable": vulnerable,
            });
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

fn seed_start_compare_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
    strip_piles: bool,
) {
    let mut expected = seed_start_normalize_combat_compare(expected, strip_piles);
    let mut actual = seed_start_normalize_combat_compare(actual, strip_piles);
    apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);
    if let (Some(expected_obj), Some(actual_obj)) = (expected.as_object(), actual.as_object_mut()) {
        for key in ["ascension", "deck_ids", "relic_ids"] {
            if !expected_obj.contains_key(key) {
                actual_obj.remove(key);
            }
        }
    }
    compare_subset(report, action, label, expected, actual);
}

fn seed_start_compare_deferred_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
) -> bool {
    let mut expected = seed_start_normalize_combat_compare(expected, false);
    let mut actual = seed_start_normalize_combat_compare(actual, false);
    apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);
    if let (Some(expected_obj), Some(actual_obj)) = (expected.as_object(), actual.as_object_mut()) {
        for key in ["ascension", "deck_ids", "relic_ids"] {
            if !expected_obj.contains_key(key) {
                actual_obj.remove(key);
            }
        }
    }
    seed_start_compare_deferred_subset(report, action, label, expected, actual)
}

fn apply_observed_debug_intent_visibility_contract(expected: &mut Value, actual: &mut Value) {
    if let (Some(expected_monsters), Some(actual_monsters)) = (
        expected.get_mut("monsters").and_then(Value::as_array_mut),
        actual.get_mut("monsters").and_then(Value::as_array_mut),
    ) {
        for (expected_monster, actual_monster) in expected_monsters.iter_mut().zip(actual_monsters)
        {
            if expected_monster.get("move_id").is_none() {
                if let Some(fields) = actual_monster.as_object_mut() {
                    fields.remove("move_id");
                }
            }
            let observed_is_unsettled = expected_monster
                .get("intent")
                .and_then(Value::as_str)
                .is_some_and(|intent| intent == "DEBUG");
            if observed_is_unsettled {
                if let Some(fields) = expected_monster.as_object_mut() {
                    fields.remove("intent");
                }
                if let Some(fields) = actual_monster.as_object_mut() {
                    fields.remove("intent");
                }
            }
        }
    }
}

fn seed_start_is_transient_combat_post_state(message: &Value) -> bool {
    let Some(game) = message.get("game_state") else {
        return false;
    };
    let screen_type = game.get("screen_type").and_then(Value::as_str);
    let action_phase = game.get("action_phase").and_then(Value::as_str);
    matches!(screen_type, Some("GRID" | "HAND_SELECT"))
        && action_phase == Some("EXECUTING_ACTIONS")
        && game.get("current_action").is_some()
}

fn seed_start_compare_transient_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    mut expected: Value,
    mut actual: Value,
) {
    for value in [&mut expected, &mut actual] {
        if let Some(object) = value.as_object_mut() {
            for key in [
                "current_hp",
                "combat_player_hp",
                "combat_player_block",
                "combat_player_energy",
                "monsters",
            ] {
                object.remove(key);
            }
        }
    }
    seed_start_compare_combat_subset(report, action, label, expected, actual, false);
}

fn seed_start_normalize_combat_compare(mut value: Value, strip_piles: bool) -> Value {
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    let player_is_dead = obj
        .get("combat_player_hp")
        .and_then(Value::as_i64)
        .is_some_and(|hp| hp <= 0);
    obj.remove("unobservable");
    if strip_piles {
        obj.remove("hand_ids");
        obj.remove("draw_ids");
        obj.remove("discard_ids");
    }
    if let Some(monsters) = obj.get_mut("monsters").and_then(Value::as_array_mut) {
        for monster in monsters {
            if let Some(fields) = monster.as_object_mut() {
                let monster_is_dead = fields
                    .get("current_hp")
                    .and_then(Value::as_i64)
                    .is_some_and(|hp| hp <= 0);
                // CommunicationMod exposes dead-monster powers inconsistently across
                // lethal and settling frames. They no longer affect simulation, so
                // compare powers only while the monster is alive.
                if monster_is_dead {
                    fields.remove("strength");
                    fields.remove("ritual");
                    fields.remove("vulnerable");
                }
                // A lethal player-damage frame can retain an in-flight or already
                // prepared monster intent, but no future player decision can observe
                // or act on it. Keep terminal HP and every other stable field strict.
                if player_is_dead || monster_is_dead {
                    fields.remove("intent");
                    fields.remove("move_id");
                }
            }
        }
    }
    Value::Object(obj.clone())
}

#[cfg(test)]
#[allow(dead_code)]
fn seed_start_normalize_combat_entry_compare(value: Value) -> Value {
    let mut value = seed_start_normalize_combat_compare(value, true);
    if let Some(obj) = value.as_object_mut() {
        obj.remove("ascension");
        obj.remove("deck_ids");
        obj.remove("relic_ids");
    }
    value
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

#[cfg(test)]
fn observed_event_screen(game: &Value, event_rng_seed: u64) -> Option<EventScreen> {
    if game
        .get("screen_type")
        .and_then(Value::as_str)
        .is_none_or(|screen| screen != "EVENT")
    {
        return None;
    }
    let state = game.get("screen_state");
    let event_id = state
        .and_then(|state| state.get("event_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let event_name = state
        .and_then(|state| state.get("event_name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let choices = choice_list_from_value(game.get("choice_list"));
    if event_id == "Lab" || event_name == "Lab" {
        return Some(event_screen(Event::Lab));
    }
    if event_id == "Masked Bandits"
        || event_name == "Masked Bandits"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 2
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("pay"))
            && choices.iter().any(|choice| {
                choice.eq_ignore_ascii_case("fight") || choice.eq_ignore_ascii_case("fight!")
            }))
    {
        return Some(EventScreen {
            event: Event::MaskedBandits,
            choices: vec![
                EventChoice {
                    label: "Pay".to_owned(),
                },
                EventChoice {
                    label: "Fight".to_owned(),
                },
            ],
            stage: 0,
            event_data: 0,
        });
    }
    if event_id == "The Cleric"
        || event_name == "The Cleric"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 3
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("heal"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("purify"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("heal"))
            || labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("purify"))
        {
            0
        } else {
            1
        };
        return Some(EventScreen {
            event: Event::TheCleric,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Accursed Blacksmith" || event_name == "Ominous Forge" {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("forge"))
            || labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("rummage"))
        {
            0
        } else {
            1
        };
        return Some(EventScreen {
            event: Event::AccursedBlacksmith,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Shining Light"
        || event_name == "Shining Light"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 2
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("enter"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels.len() == 1
            && labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave"))
        {
            1
        } else {
            0
        };
        return Some(EventScreen {
            event: Event::ShiningLight,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Scrap Ooze" || event_name == "Scrap Ooze" {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("reach inside"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("deeper"))
        {
            1
        } else {
            2
        };
        let event_data = if stage == 1 {
            scrap_ooze_failed_reaches_from_observed(game, int(game, "ascension_level") as u8)
                .unwrap_or(1)
        } else {
            0
        };
        return Some(EventScreen {
            event: Event::ScrapOoze,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data,
        });
    }
    if event_id == "FaceTrader"
        || event_name == "Face Trader"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 3
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("touch"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("trade"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("touch"))
        {
            1
        } else {
            2
        };
        return Some(EventScreen {
            event: Event::FaceTrader,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Golden Wing"
        || event_name == "Wing Statue"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 2
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("pray"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("pray"))
            || labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("destroy"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            1
        } else {
            2
        };
        return Some(EventScreen {
            event: Event::WingStatue,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Transmorgrifier" || event_name == "Transmogrifier" {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("pray"))
        {
            0
        } else {
            1
        };
        return Some(EventScreen {
            event: Event::Transmorgrifier,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Nest" || event_name == "The Nest" {
        let choices = choice_list_from_value(game.get("choice_list"));
        let labels = if choices.is_empty() {
            vec!["Continue".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            0
        } else if labels.iter().any(|choice| {
            choice.eq_ignore_ascii_case("smash and grab")
                || choice.eq_ignore_ascii_case("stay in line")
        }) {
            1
        } else {
            2
        };
        return Some(EventScreen {
            event: Event::Nest,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Knowing Skull" || event_name == "Knowing Skull" {
        let choices = choice_list_from_value(game.get("choice_list"));
        let labels = if choices.is_empty() {
            vec!["Continue".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("leave"))
        {
            2
        } else {
            1
        };
        let event_data = if stage == 1 {
            knowing_skull_costs_from_observed(game).unwrap_or_else(default_knowing_skull_costs)
        } else {
            default_knowing_skull_costs()
        };
        return Some(EventScreen {
            event: Event::KnowingSkull,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data,
        });
    }
    if event_id != "Neow Event" && event_name != "Neow" {
        return None;
    }

    let choices = choice_list_from_value(game.get("choice_list"));
    let stage = if choices
        .iter()
        .any(|choice| choice.eq_ignore_ascii_case("talk"))
    {
        0
    } else if choices
        .iter()
        .any(|choice| choice.eq_ignore_ascii_case("leave"))
    {
        2
    } else {
        1
    };
    let labels = if stage == 1 && choices.is_empty() {
        generate_neow_options(event_rng_seed as i64, int(game, "max_hp"))
            .into_iter()
            .map(|option| option.label)
            .collect::<Vec<_>>()
    } else {
        choices
    };

    Some(EventScreen {
        event: Event::Neow,
        choices: labels
            .into_iter()
            .map(|label| EventChoice { label })
            .collect(),
        stage,
        event_data: 0,
    })
}

#[cfg(test)]
fn default_knowing_skull_costs() -> u32 {
    knowing_skull_cost_data(6, 6, 6, 6)
}

#[cfg(test)]
fn knowing_skull_cost_data(potion: u32, gold: u32, card: u32, leave: u32) -> u32 {
    potion | (gold << 8) | (card << 16) | (leave << 24)
}

#[cfg(test)]
fn knowing_skull_costs_from_observed(game: &Value) -> Option<u32> {
    let options = game
        .get("screen_state")
        .and_then(|state| state.get("options"))
        .and_then(Value::as_array)?;
    let mut potion = None;
    let mut gold = None;
    let mut card = None;
    let mut leave = None;

    for option in options {
        let label = option
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = option.get("text").and_then(Value::as_str).unwrap_or("");
        let cost = hp_loss_cost_from_option_text(text)?;
        if label.contains("pick me up") {
            potion = Some(cost);
        } else if label.contains("riches") {
            gold = Some(cost);
        } else if label.contains("success") {
            card = Some(cost);
        } else if label.contains("leave") {
            leave = Some(cost);
        }
    }

    Some(knowing_skull_cost_data(
        potion.unwrap_or(6),
        gold.unwrap_or(6),
        card.unwrap_or(6),
        leave.unwrap_or(6),
    ))
}

#[cfg(test)]
fn hp_loss_cost_from_option_text(text: &str) -> Option<u32> {
    let after_lose = text.split("Lose ").nth(1)?;
    let hp_text = after_lose.split(" HP").next()?;
    hp_text.trim().parse::<u32>().ok()
}

#[cfg(test)]
fn scrap_ooze_failed_reaches_from_observed(game: &Value, ascension: u8) -> Option<u32> {
    let options = game
        .get("screen_state")
        .and_then(|state| state.get("options"))
        .and_then(Value::as_array)?;
    let text = options
        .iter()
        .filter_map(|option| option.get("text").and_then(Value::as_str))
        .find(|text| text.contains("Lose ") && text.contains(" HP"))?;
    let after_lose = text.split("Lose ").nth(1)?;
    let hp_text = after_lose.split(" HP").next()?;
    let hp_loss = hp_text.trim().parse::<u32>().ok()?;
    let base_loss = if ascension >= 15 { 5 } else { 3 };
    hp_loss.checked_sub(base_loss)
}

#[cfg(test)]
fn observed_reward_potion_offer(game: &Value) -> Option<Potion> {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)?
        .iter()
        .find(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("POTION"))
        })
        .and_then(|reward| reward.get("potion"))
        .and_then(|potion| potion.get("name").or_else(|| potion.get("id")))
        .and_then(Value::as_str)
        .and_then(potion_from_trace_name)
}

fn observed_reward_relic_key_offer(game: &Value) -> Option<RelicKey> {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)?
        .iter()
        .find(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("RELIC"))
        })
        .and_then(|reward| reward.get("relic"))
        .and_then(|relic| relic.get("name").or_else(|| relic.get("id")))
        .and_then(Value::as_str)
        .and_then(relic_key_from_trace_name)
}

fn observed_boss_relic_key_choices(game: &Value) -> Vec<RelicKey> {
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
                .and_then(relic_key_from_trace_name)
        })
        .collect()
}

fn combat_action_from_command(command: &str, combat: &CombatState) -> Option<CombatAction> {
    use sts_core::card::TargetRequirement;
    use sts_core::content::cards::get_card_definition;

    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("END") => Some(CombatAction::EndTurn),
        [cmd, hand_index] if cmd.eq_ignore_ascii_case("PLAY") => Some(CombatAction::PlayCard {
            card_id: hand_card_id_from_bridge_slot(combat, hand_index)?,
            target: None,
        }),
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
    combat.monsters.get(index).map(|monster| monster.id)
}

fn hand_card_id_from_bridge_slot(combat: &CombatState, hand_index: &str) -> Option<CardId> {
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    Some(combat.piles.hand.get(index)?.id)
}

#[cfg(test)]
fn unsupported_combat_command_reason(message: &Value, command: &str) -> Option<String> {
    let parts: Vec<_> = command.split_whitespace().collect();
    let [cmd, hand_index, ..] = parts.as_slice() else {
        return None;
    };
    if !cmd.eq_ignore_ascii_case("PLAY") {
        return None;
    }
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    let card = message
        .get("game_state")?
        .get("combat_state")?
        .get("hand")?
        .as_array()?
        .get(index)?;
    if content_id_from_card_value(card).is_some() {
        return None;
    }
    let card_name = card
        .get("name")
        .or_else(|| card.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown card");
    Some(format!(
        "card '{card_name}' is not mapped in the verifier, so this combat command is unsupported"
    ))
}

fn is_final_combat_blow(run: &RunState, action: CombatAction) -> bool {
    let Some(combat) = &run.combat else {
        return false;
    };
    let Ok(next) = sts_core::apply_combat_action(combat, action) else {
        return false;
    };
    next.phase == CombatPhase::Won
}

#[cfg(test)]
fn observed_combat_subset(message: &Value, fields: &[&str]) -> Value {
    let Some(obs) = normalize_communication_mod_message(message) else {
        return json!({});
    };
    let Some(combat) = obs.combat else {
        return json!({});
    };
    let monster = combat.monsters.iter().find(|monster| monster.hp > 0);
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "player_hp" => insert(&mut out, field, combat.player_hp),
            "player_block" => insert(&mut out, field, combat.player_block),
            "player_energy" => insert(&mut out, field, combat.player_energy),
            "monster_hp" => insert(&mut out, field, monster.map(|m| m.hp).unwrap_or(0)),
            "monster_block" => insert(&mut out, field, monster.map(|m| m.block).unwrap_or(0)),
            "monster_intent" => insert(
                &mut out,
                field,
                monster.map(|m| m.intent.clone()).unwrap_or_default(),
            ),
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn simulated_combat_subset(run: &RunState, fields: &[&str]) -> Value {
    let combat = run.combat.as_ref().expect("combat available");
    let monster = combat.monsters.iter().find(|monster| monster.alive);
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "player_hp" => insert(&mut out, field, combat.player.hp),
            "player_block" => insert(&mut out, field, combat.player.block),
            "player_energy" => insert(&mut out, field, combat.player.energy),
            "monster_hp" => insert(&mut out, field, monster.map(|m| m.hp).unwrap_or(0)),
            "monster_block" => insert(&mut out, field, monster.map(|m| m.block).unwrap_or(0)),
            "monster_intent" => {
                insert(&mut out, field, monster.map(intent_key).unwrap_or_default())
            }
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_run_subset(message: &Value, fields: &[&str]) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "gold" => insert(&mut out, field, int(game, "gold")),
            "current_hp" => insert(&mut out, field, int(game, "current_hp")),
            "deck_size" => insert(
                &mut out,
                field,
                game.get("deck")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
            ),
            "deck_ids" => insert(&mut out, field, deck_keys_from_value(game.get("deck"))),
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn simulated_run_subset(run: &RunState, fields: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "gold" => insert(&mut out, field, run.gold),
            "current_hp" => insert(&mut out, field, run.player_hp),
            "deck_size" => insert(&mut out, field, run.deck.len()),
            "deck_ids" => insert(&mut out, field, deck_content_keys(&run.deck)),
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn deck_has_unmapped_cards(message: &Value) -> bool {
    message
        .get("game_state")
        .and_then(|game| game.get("deck"))
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .any(|card| content_id_from_card_value(card).is_none())
        })
        .unwrap_or(false)
}

#[cfg(test)]
fn unsupported_monster_ai_reason(message: &Value) -> Option<String> {
    let groups: Vec<String> = message
        .get("game_state")?
        .get("combat_state")?
        .get("monsters")?
        .as_array()?
        .iter()
        .filter(|monster| int(monster, "current_hp") > 0)
        .filter_map(|monster| monster.get("id").and_then(Value::as_str))
        .filter(|id| !matches!(*id, "Cultist" | "JawWorm"))
        .map(str::to_owned)
        .collect();
    if groups.is_empty() {
        None
    } else {
        Some(format!(
            "exact combat transition is unsupported for monster group(s): {}",
            groups.join(", ")
        ))
    }
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

fn seed_start_compare_deferred_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
) -> bool {
    let diffs = subset_diffs(expected, actual);
    if diffs.is_empty() {
        true
    } else {
        if let Some(existing) = report.unexpected_diffs.iter_mut().find(|existing| {
            existing.action_step == action.step && existing.command == action.command
        }) {
            existing
                .diffs
                .extend(diffs.into_iter().map(|diff| format!("{label}: {diff}")));
        } else {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: label.to_owned(),
                diffs,
            });
        }
        false
    }
}

fn subset_diffs(expected: Value, actual: Value) -> Vec<String> {
    let expected_json = serde_json::to_string(&expected).expect("json serializes");
    let actual_json = serde_json::to_string(&actual).expect("json serializes");
    canonical_diff(&expected_json, &actual_json)
}

#[cfg(test)]
#[allow(dead_code)]
fn normalized_combat_subset_diffs(
    expected: Value,
    actual: Value,
    strip_piles: bool,
) -> Vec<String> {
    subset_diffs(
        seed_start_normalize_combat_compare(expected, strip_piles),
        seed_start_normalize_combat_compare(actual, strip_piles),
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn post_supported_combat_fields(command: &str) -> &'static [&'static str] {
    if command.trim().eq_ignore_ascii_case("END") {
        &[
            "player_hp",
            "player_block",
            "player_energy",
            "monster_hp",
            "monster_block",
            "monster_intent",
        ]
    } else {
        &[
            "player_hp",
            "player_block",
            "player_energy",
            "monster_hp",
            "monster_block",
        ]
    }
}

fn combat_label(command: &str, run: &RunState) -> String {
    let Some(combat) = &run.combat else {
        return "combat".to_owned();
    };
    let Some(CombatAction::PlayCard { card_id, .. }) = combat_action_from_command(command, combat)
    else {
        return "end turn".to_owned();
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

#[cfg(test)]
#[allow(dead_code)]
fn monsters_from_observed(
    value: Option<&Value>,
    _player: &Value,
    ascension: u8,
) -> Vec<MonsterState> {
    let Some(monsters) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    monsters
        .iter()
        .enumerate()
        .map(|(index, monster)| {
            let game_id = monster
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("Cultist");
            let content_id = sts_core::content::monsters::content_id_from_game_monster_id(game_id);
            let rolled_attack_damage = louse_bite_damage_from_observed(monster, content_id);
            let powers = monster_powers(monster.get("powers"));
            let replay = elite_boss_replay_fields(monster, content_id, &powers, ascension);
            let move_history = target_move_byte(content_id, replay.intent)
                .map(|move_byte| vec![move_byte])
                .unwrap_or_default();
            let is_gone = monster
                .get("is_gone")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            MonsterState {
                id: MonsterId::new(index as u64 + 1),
                hp: int(monster, "current_hp"),
                max_hp: int(monster, "max_hp"),
                block: int(monster, "block"),
                alive: int(monster, "current_hp") > 0 && !is_gone,
                escaped: is_gone,
                powers,
                temp_strength_down: 0,
                content_id,
                slime_size: None,
                moves_executed: replay.moves_executed,
                sleep_turns_remaining: replay.sleep_turns_remaining,
                has_siphoned: replay.has_siphoned,
                split_triggered: false,
                defensive_turns_remaining: replay.defensive_turns_remaining,
                mode_shift: replay.mode_shift,
                mode_shift_threshold: replay.mode_shift_threshold,
                in_defensive_mode: replay.in_defensive_mode,
                rolled_attack_damage,
                stolen_gold: 0,
                move_history,
                gremlin_leader_slot: None,
                stasis_card: None,
                initial_intent_locked: false,
                intent: replay.intent,
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
struct EliteBossReplayFields {
    moves_executed: u32,
    sleep_turns_remaining: u32,
    has_siphoned: bool,
    defensive_turns_remaining: u32,
    mode_shift: i32,
    mode_shift_threshold: i32,
    in_defensive_mode: bool,
    intent: MonsterIntent,
}

#[cfg(test)]
#[allow(dead_code)]
fn elite_boss_replay_fields(
    monster: &Value,
    content_id: ContentId,
    powers: &MonsterPowers,
    ascension: u8,
) -> EliteBossReplayFields {
    let intent_str = monster.get("intent").and_then(Value::as_str).unwrap_or("");
    let damage = int(monster, "move_base_damage");

    match content_id {
        LAGAVULIN_ID => {
            let sleep_turns_remaining = if matches!(intent_str, "SLEEP" | "DEBUG") {
                3
            } else {
                0
            };
            let has_siphoned = intent_str == "ATTACK";
            let intent = if sleep_turns_remaining > 0 {
                MonsterIntent::Sleep
            } else if intent_str == "STUN" {
                MonsterIntent::Stun
            } else if !has_siphoned {
                MonsterIntent::SiphonPlayer {
                    strength: 1,
                    dexterity: 1,
                }
            } else {
                MonsterIntent::Attack { damage: 18 }
            };
            EliteBossReplayFields {
                moves_executed: u32::from(has_siphoned),
                sleep_turns_remaining,
                has_siphoned,
                defensive_turns_remaining: 0,
                mode_shift: 0,
                mode_shift_threshold: 0,
                in_defensive_mode: false,
                intent,
            }
        }
        GREMLIN_NOB_ID => {
            let moves_executed = match (intent_str, damage) {
                ("DEBUG" | "BUFF", _) => 0,
                ("ATTACK_DEBUFF", 6) => 0,
                ("ATTACK", 14) => 1,
                ("ATTACK_DEBUFF", _) => 2,
                ("ATTACK", _) => 1,
                _ => moves_executed_from_observed(monster, content_id),
            };
            EliteBossReplayFields {
                moves_executed,
                sleep_turns_remaining: 0,
                has_siphoned: false,
                defensive_turns_remaining: 0,
                mode_shift: 0,
                mode_shift_threshold: 0,
                in_defensive_mode: false,
                intent: observed_intent(monster, content_id, ascension),
            }
        }
        GUARDIAN_ID => {
            let mode_shift = monster
                .get("powers")
                .and_then(Value::as_array)
                .and_then(|powers| {
                    powers.iter().find_map(|power| {
                        if power_id(power).as_deref() == Some("Mode Shift") {
                            Some(int(power, "amount"))
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or(30);
            let in_defensive_mode = powers.spikes > 0
                || intent_str == "BUFF"
                || (intent_str == "ATTACK" && damage == 9);
            let defensive_turns_remaining = if in_defensive_mode {
                match (intent_str, damage) {
                    ("BUFF", _) => 7,
                    ("ATTACK", 9) => 5,
                    ("ATTACK", 8) => 3,
                    _ => 4,
                }
            } else {
                0
            };
            EliteBossReplayFields {
                moves_executed: if in_defensive_mode {
                    7_u32.saturating_sub(defensive_turns_remaining)
                } else {
                    match (intent_str, damage) {
                        ("DEBUG", _) => 0,
                        ("ATTACK", 32) => 0,
                        ("ATTACK", 5) => 1,
                        _ => 0,
                    }
                },
                sleep_turns_remaining: 0,
                has_siphoned: false,
                defensive_turns_remaining,
                mode_shift,
                mode_shift_threshold: mode_shift.max(30),
                in_defensive_mode,
                intent: observed_intent(monster, content_id, ascension),
            }
        }
        _ => EliteBossReplayFields {
            moves_executed: moves_executed_from_observed(monster, content_id),
            sleep_turns_remaining: 0,
            has_siphoned: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            intent: observed_intent(monster, content_id, ascension),
        },
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn louse_bite_damage_from_observed(monster: &Value, content_id: ContentId) -> Option<i32> {
    if !matches!(
        content_id,
        sts_core::content::monsters::RED_LOUSE_ID | sts_core::content::monsters::GREEN_LOUSE_ID
    ) {
        return None;
    }
    let damage = int(monster, "move_base_damage");
    (damage > 0).then_some(damage)
}

#[cfg(test)]
fn observed_intent(monster: &Value, content_id: ContentId, ascension: u8) -> MonsterIntent {
    use sts_core::content::monsters::{
        champ_strength_amount, gremlin_nob_enrage, ACID_SLIME_ID, BRONZE_AUTOMATON_ID,
        BRONZE_ORB_ID, BYRD_ID, CENTURION_ID, CHAMP_DEFENSIVE_BLOCK, CHAMP_DEFENSIVE_METALLICIZE,
        CHAMP_FACE_SLAP_DAMAGE, CHAMP_ID, CHOSEN_ID, CULTIST_ID, DARKLING_ID, DECA_ID,
        FUNGI_BEAST_ID, GREEN_LOUSE_ID, GREEN_LOUSE_WEAK, GREMLIN_FAT_ID, GREMLIN_LEADER_ID,
        GREMLIN_TSUNDERE_ID, HEALER_ID, HEXAGHOST_ID, JAW_WORM_ID, ORB_WALKER_ID, RED_LOUSE_ID,
        REPULSOR_ID, SENTRY_ID, SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLIME_BOSS_A19_SLIMED_COUNT,
        SLIME_BOSS_ID, SLIME_BOSS_SLIMED_COUNT, SNAKE_PLANT_ID, SNECKO_ID,
        SPHERIC_GUARDIAN_ACTIVATE_BLOCK, SPHERIC_GUARDIAN_FRAIL, SPHERIC_GUARDIAN_HARDEN_BLOCK,
        SPHERIC_GUARDIAN_ID, SPIKER_ID, SPIKE_SLIME_ID, THE_COLLECTOR_ID,
    };

    let damage = int(monster, "move_base_damage");
    let hits = int(monster, "move_hits");
    let move_id = int(monster, "move_id");
    match monster.get("intent").and_then(Value::as_str).unwrap_or("") {
        "STUN" => MonsterIntent::Stun,
        "ESCAPE" => MonsterIntent::Escape,
        "DEBUG" if content_id == SENTRY_ID && damage <= 0 => {
            MonsterIntent::AddDazedToDiscard { count: 2 }
        }
        "ATTACK" if matches!(content_id, LOOTER_ID | MUGGER_ID) => MonsterIntent::AttackStealGold {
            damage: damage.max(0),
            amount: looter_theft(0),
        },
        "ATTACK" if content_id == SLAVER_BLUE_ID && move_id == 4 => {
            MonsterIntent::AttackApplyPlayerWeak {
                damage: damage.max(0),
                weak: 1,
            }
        }
        "ATTACK" if hits > 1 => MonsterIntent::AttackMultiple {
            damage: damage.max(0),
            hits,
        },
        "ATTACK" if content_id == SENTRY_ID && damage <= 0 => {
            MonsterIntent::AddDazedToDiscard { count: 2 }
        }
        "ATTACK" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "DEBUFF" if content_id == SENTRY_ID => MonsterIntent::AddDazedToDiscard { count: 2 },
        "DEBUFF" if content_id == REPULSOR_ID => MonsterIntent::AddDazedToDraw { count: 2 },
        "DEBUFF" if content_id == CHOSEN_ID => MonsterIntent::ApplyPlayerWeakStrengthSelf {
            weak: 3,
            strength: 3,
        },
        "BUFF" if content_id == SPIKER_ID => MonsterIntent::StrengthAndBlock {
            strength: 0,
            block: 0,
        },
        "BUFF" | "DEBUG" if content_id == GREMLIN_NOB_ID => MonsterIntent::StrengthSelf {
            amount: gremlin_nob_enrage(ascension),
        },
        "BUFF" if content_id == BYRD_ID => MonsterIntent::StrengthSelf { amount: 1 },
        "BUFF" if content_id == CHAMP_ID && move_id == 5 => MonsterIntent::StrengthSelf {
            amount: champ_strength_amount(ascension),
        },
        "BUFF" if content_id == CHAMP_ID && move_id == 7 => MonsterIntent::StrengthSelf {
            amount: champ_strength_amount(ascension) * 3,
        },
        "STRONG_DEBUFF" if content_id == CHOSEN_ID => MonsterIntent::ApplyPlayerHex { amount: 1 },
        "STRONG_DEBUFF" if content_id == SNAKE_PLANT_ID => {
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 }
        }
        "STRONG_DEBUFF" if content_id == THE_COLLECTOR_ID => {
            MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 3,
                weak: 3,
                vulnerable: 3,
            }
        }
        "STRONG_DEBUFF" if content_id == SNECKO_ID => MonsterIntent::ApplyPlayerConfusion,
        "STRONG_DEBUFF" if content_id == SLIME_BOSS_ID => MonsterIntent::AddSlimedToDiscard {
            count: if ascension >= 19 {
                SLIME_BOSS_A19_SLIMED_COUNT
            } else {
                SLIME_BOSS_SLIMED_COUNT
            },
        },
        "DEBUFF"
            if content_id == SPIKE_SLIME_ID
                && (matches!(
                    str_field(monster, "id"),
                    Some("SpikeSlime_M" | "SpikeSlime_L")
                ) || int(monster, "max_hp")
                    > sts_core::content::monsters::SPIKE_SLIME_S_A7_HP_RANGE.max) =>
        {
            MonsterIntent::ApplyPlayerFrailAndWeak {
                frail: observed_spike_slime_frail(monster, ascension),
                weak: 0,
            }
        }
        "DEBUFF" if content_id == GREEN_LOUSE_ID => MonsterIntent::ApplyPlayerWeak {
            amount: GREEN_LOUSE_WEAK,
        },
        "DEBUFF" if content_id == CHAMP_ID && move_id == 6 => {
            MonsterIntent::ApplyPlayerWeak { amount: 2 }
        }
        "DEBUFF"
            if content_id == ACID_SLIME_ID
                && (str_field(monster, "id") == Some("AcidSlime_L")
                    || int(monster, "max_hp")
                        > sts_core::content::monsters::ACID_SLIME_M_A7_HP_RANGE.max) =>
        {
            MonsterIntent::ApplyPlayerWeak { amount: 2 }
        }
        "DEBUFF" => MonsterIntent::ApplyPlayerWeak { amount: 1 },
        "ATTACK_DEBUFF" if matches!(content_id, ACID_SLIME_ID | SPIKE_SLIME_ID) => {
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: damage.max(0),
                count: observed_slimed_count(monster, content_id),
            }
        }
        "ATTACK_DEBUFF" if content_id == HEXAGHOST_ID && move_id == 6 => {
            MonsterIntent::AttackMultipleUpgradeBurns {
                damage: damage.max(0),
                hits: int(monster, "move_hits").max(1),
                count: 3,
            }
        }
        "ATTACK_DEBUFF" if content_id == HEXAGHOST_ID => MonsterIntent::AddBurnToDiscard {
            damage: damage.max(0),
            count: if ascension >= 19 { 2 } else { 1 },
        },
        "ATTACK_DEBUFF" if content_id == ORB_WALKER_ID => MonsterIntent::AddBurnToDiscardAndDraw {
            damage: damage.max(0),
            count: 1,
        },
        "ATTACK_DEBUFF" if content_id == DECA_ID => {
            MonsterIntent::AttackMultipleAddDazedToDiscard {
                damage: damage.max(0),
                hits: hits.max(1),
                count: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == SPHERIC_GUARDIAN_ID => {
            MonsterIntent::AttackApplyPlayerFrail {
                damage: damage.max(0),
                frail: SPHERIC_GUARDIAN_FRAIL,
            }
        }
        "ATTACK_DEBUFF" if content_id == GREMLIN_FAT_ID && ascension >= 17 => {
            MonsterIntent::AttackApplyPlayerFrailAndWeak {
                damage: damage.max(0),
                frail: 1,
                weak: 1,
            }
        }
        "ATTACK_DEBUFF" if content_id == GREMLIN_FAT_ID => MonsterIntent::AttackApplyPlayerWeak {
            damage: damage.max(0),
            weak: 1,
        },
        "ATTACK_DEBUFF" if content_id == GREMLIN_NOB_ID => {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: damage.max(0),
                vulnerable: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == CHOSEN_ID => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: damage.max(0),
            vulnerable: 2,
        },
        "ATTACK_DEBUFF" if content_id == CHAMP_ID && move_id == 4 => {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: CHAMP_FACE_SLAP_DAMAGE,
                vulnerable: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == SNECKO_ID && ascension >= 17 => {
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: damage.max(0),
                weak: 2,
                vulnerable: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == SNECKO_ID => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: damage.max(0),
            vulnerable: 2,
        },
        "ATTACK_DEBUFF" if content_id == SLAVER_BLUE_ID => MonsterIntent::AttackApplyPlayerWeak {
            damage: damage.max(0),
            weak: if ascension >= 17 { 2 } else { 1 },
        },
        "ATTACK_DEBUFF" if content_id == SLAVER_RED_ID => {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: damage.max(0),
                vulnerable: if ascension >= 17 { 2 } else { 1 },
            }
        }
        "ATTACK_DEBUFF" if content_id == TASKMASTER_ID => MonsterIntent::AttackAddWoundsToDiscard {
            damage: damage.max(0),
            count: if ascension >= 18 {
                3
            } else if ascension >= 3 {
                2
            } else {
                1
            },
        },
        "ATTACK_DEFEND" if content_id == SPHERIC_GUARDIAN_ID => MonsterIntent::AttackAndBlock {
            damage: damage.max(0),
            block: SPHERIC_GUARDIAN_HARDEN_BLOCK,
        },
        "ATTACK_DEFEND" if content_id == JAW_WORM_ID => MonsterIntent::AttackAndBlock {
            damage: damage.max(0),
            block: 5,
        },
        "ATTACK_DEBUFF" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "ATTACK_BUFF" if content_id == GUARDIAN_ID && move_id == 4 => {
            MonsterIntent::AttackMultiple {
                damage: damage.max(0),
                hits: 2,
            }
        }
        "ATTACK_BUFF" if content_id == SHELLED_PARASITE_ID => MonsterIntent::AttackHealSelf {
            damage: damage.max(0),
        },
        "ATTACK_BUFF" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "DEFEND_BUFF" if content_id == GREMLIN_LEADER_ID => MonsterIntent::EncourageGremlins {
            strength: 3,
            block: 6,
        },
        "DEFEND_BUFF" if content_id == JAW_WORM_ID => MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        },
        "DEFEND_BUFF" if content_id == BRONZE_AUTOMATON_ID => MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 9,
        },
        "DEFEND_BUFF" if content_id == CHAMP_ID && move_id == 2 => {
            MonsterIntent::StrengthAndBlock {
                strength: CHAMP_DEFENSIVE_METALLICIZE,
                block: CHAMP_DEFENSIVE_BLOCK,
            }
        }
        "DEFEND" | "BLOCK" if matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) => {
            MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 0,
            }
        }
        "DEFEND" | "BLOCK" if content_id == GUARDIAN_ID => MonsterIntent::Block {
            block: GUARDIAN_CHARGE_BLOCK,
        },
        "DEFEND" | "BLOCK" if content_id == SPHERIC_GUARDIAN_ID => MonsterIntent::Block {
            block: SPHERIC_GUARDIAN_ACTIVATE_BLOCK,
        },
        "DEFEND" | "BLOCK" if content_id == CENTURION_ID => MonsterIntent::Block {
            block: observed_centurion_block(ascension),
        },
        "DEFEND" | "BLOCK" if content_id == GREMLIN_TSUNDERE_ID => MonsterIntent::Block {
            block: observed_gremlin_tsundere_block(ascension),
        },
        "DEFEND" | "BLOCK" if content_id == DARKLING_ID => MonsterIntent::Block { block: 12 },
        "DEFEND" | "BLOCK" => MonsterIntent::Block {
            block: damage.max(0),
        },
        "STRONG_DEBUFF" if content_id == BRONZE_ORB_ID => MonsterIntent::SiphonPlayer {
            strength: 0,
            dexterity: 0,
        },
        "UNKNOWN" if content_id == GREMLIN_LEADER_ID && move_id == 2 => {
            MonsterIntent::SummonGremlins { count: 2 }
        }
        "UNKNOWN" if content_id == ACID_SLIME_ID && move_id == 3 => {
            MonsterIntent::SummonGremlins { count: 2 }
        }
        "UNKNOWN" if content_id == BRONZE_AUTOMATON_ID => {
            MonsterIntent::SummonGremlins { count: 2 }
        }
        "UNKNOWN" if content_id == BRONZE_ORB_ID => MonsterIntent::SiphonPlayer {
            strength: 0,
            dexterity: 0,
        },
        "BUFF" | "DEBUG" | "UNKNOWN" => match content_id {
            CULTIST_ID => MonsterIntent::Ritual { amount: 3 },
            ORB_WALKER_ID if damage > 0 => MonsterIntent::Attack { damage },
            SPIKE_SLIME_ID if damage >= 8 => MonsterIntent::AttackAddSlimedToDiscard {
                damage,
                count: observed_slimed_count(monster, content_id),
            },
            SPIKE_SLIME_ID if damage > 0 => MonsterIntent::Attack { damage },
            SPIKE_SLIME_ID => MonsterIntent::Attack { damage: 5 },
            ACID_SLIME_ID if move_id == 2 && damage > 0 => MonsterIntent::Attack { damage },
            ACID_SLIME_ID if move_id == 1 && damage > 0 => {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage,
                    count: observed_slimed_count(monster, content_id),
                }
            }
            ACID_SLIME_ID if damage > 0 => MonsterIntent::AttackAddSlimedToDiscard {
                damage,
                count: observed_slimed_count(monster, content_id),
            },
            ACID_SLIME_ID => MonsterIntent::Attack { damage: 7 },
            RED_LOUSE_ID | GREEN_LOUSE_ID => MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 0,
            },
            GUARDIAN_ID if monster.get("intent").and_then(Value::as_str) == Some("BUFF") => {
                MonsterIntent::GuardianCloseUp { sharp_hide: 3 }
            }
            GUARDIAN_ID => MonsterIntent::Block {
                block: GUARDIAN_CHARGE_BLOCK,
            },
            HEALER_ID if move_id == 2 => MonsterIntent::HealAllMonsters {
                amount: observed_healer_heal(ascension),
            },
            HEALER_ID => MonsterIntent::StrengthAllMonsters {
                amount: observed_healer_strength(ascension),
            },
            FUNGI_BEAST_ID => MonsterIntent::StrengthSelf {
                amount: observed_fungi_beast_strength(ascension),
            },
            _ if damage > 0 => MonsterIntent::Attack { damage },
            _ => MonsterIntent::Attack { damage: 0 },
        },
        _ => MonsterIntent::Attack { damage: 0 },
    }
}

#[cfg(test)]
fn observed_spike_slime_frail(monster: &Value, ascension: u8) -> i32 {
    let large = str_field(monster, "id") == Some("SpikeSlime_L")
        || int(monster, "max_hp") > sts_core::content::monsters::SPIKE_SLIME_M_A7_HP_RANGE.max;
    if large {
        if ascension >= 17 {
            3
        } else {
            2
        }
    } else {
        1
    }
}

#[cfg(test)]
fn observed_centurion_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        20
    } else {
        15
    }
}

#[cfg(test)]
fn observed_gremlin_tsundere_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        11
    } else if ascension >= 7 {
        8
    } else {
        7
    }
}

#[cfg(test)]
fn observed_healer_heal(ascension: u8) -> i32 {
    if ascension >= 17 {
        20
    } else {
        16
    }
}

#[cfg(test)]
fn observed_healer_strength(ascension: u8) -> i32 {
    if ascension >= 17 {
        4
    } else if ascension >= 2 {
        3
    } else {
        2
    }
}

#[cfg(test)]
fn observed_fungi_beast_strength(ascension: u8) -> i32 {
    let strength = if ascension >= 2 { 4 } else { 3 };
    if ascension >= 17 {
        strength + 1
    } else {
        strength
    }
}

#[cfg(test)]
fn observed_slimed_count(monster: &Value, content_id: ContentId) -> i32 {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE, SPIKE_SLIME_ID, SPIKE_SLIME_M_A7_HP_RANGE,
    };

    if (content_id == SPIKE_SLIME_ID && int(monster, "max_hp") > SPIKE_SLIME_M_A7_HP_RANGE.max)
        || (content_id == ACID_SLIME_ID && int(monster, "max_hp") > ACID_SLIME_M_A7_HP_RANGE.max)
    {
        2
    } else {
        1
    }
}

#[cfg(test)]
fn moves_executed_from_observed(monster: &Value, content_id: ContentId) -> u32 {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, BOOK_OF_STABBING_ID, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, CHOSEN_ID,
        CULTIST_ID, GREEN_LOUSE_ID, GREMLIN_LEADER_ID, RED_LOUSE_ID, SHELLED_PARASITE_ID,
        SNAKE_PLANT_ID, SPIKE_SLIME_ID,
    };

    let intent = monster.get("intent").and_then(Value::as_str).unwrap_or("");
    let damage = int(monster, "move_base_damage");
    let hits = int(monster, "move_hits");
    let move_id = int(monster, "move_id");
    match intent {
        "ATTACK" if content_id == BOOK_OF_STABBING_ID && hits > 1 => match hits {
            2 => 0,
            3 => 1,
            4 => 3,
            _ => (hits - 1) as u32,
        },
        "ATTACK" if content_id == BOOK_OF_STABBING_ID && damage >= 21 => 2,
        "ATTACK_BUFF" if content_id == SHELLED_PARASITE_ID => 1,
        "ATTACK" if content_id == SHELLED_PARASITE_ID && hits > 1 => 0,
        "ATTACK" if content_id == SHELLED_PARASITE_ID => 0,
        "DEBUFF" if content_id == CHOSEN_ID => 2,
        "STRONG_DEBUFF" if content_id == CHOSEN_ID => 1,
        "ATTACK_DEBUFF" if content_id == CHOSEN_ID => 3,
        "ATTACK" if content_id == SNAKE_PLANT_ID => 1,
        "STRONG_DEBUFF" if content_id == SNAKE_PLANT_ID => 2,
        "DEFEND_BUFF" if content_id == GREMLIN_LEADER_ID => 2,
        "UNKNOWN" if content_id == GREMLIN_LEADER_ID && move_id == 2 => 2,
        "UNKNOWN" if content_id == ACID_SLIME_ID && move_id == 3 => 2,
        "UNKNOWN" if content_id == BRONZE_AUTOMATON_ID && move_id == 4 => 0,
        "STRONG_DEBUFF" if content_id == BRONZE_ORB_ID => 0,
        "ATTACK" if content_id == BRONZE_ORB_ID => 1,
        "DEFEND" | "BLOCK" if content_id == BRONZE_ORB_ID => 4,
        "DEFEND_BUFF" if content_id == BRONZE_AUTOMATON_ID => {
            if power_amount(monster.get("powers"), "Strength") > 0 {
                4
            } else {
                2
            }
        }
        "STUN" if content_id == BRONZE_AUTOMATON_ID => 6,
        "ATTACK" if content_id == BRONZE_AUTOMATON_ID && hits > 1 => 1,
        "ATTACK" if content_id == BRONZE_AUTOMATON_ID && damage >= 40 => 5,
        "BUFF" | "DEBUG" | "DEBUFF" => 0,
        "ATTACK_DEBUFF" => 1,
        "ATTACK" if content_id == CULTIST_ID => 1,
        "ATTACK" if content_id == LOOTER_ID => 1,
        "ATTACK" if content_id == SPIKE_SLIME_ID => 0,
        "ATTACK" if matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) => 1,
        "ATTACK" if content_id == ACID_SLIME_ID => 1,
        _ => 1,
    }
}

#[cfg(test)]
fn monster_powers(value: Option<&Value>) -> MonsterPowers {
    let mut powers = MonsterPowers::default();
    let Some(items) = value.and_then(Value::as_array) else {
        return powers;
    };
    for power in items {
        let amount = int(power, "amount");
        match power_id(power).as_deref() {
            Some("Vulnerable") => powers.vulnerable = amount,
            Some("Weak") | Some("Weakened") => powers.weak = amount,
            Some("Strength") => powers.strength = amount,
            Some("Artifact") => powers.artifact = amount,
            Some("Ritual") | Some("Demon Form") => powers.ritual = amount,
            Some("Sharp Hide") | Some("Spikes") => powers.spikes = amount,
            Some("Curl Up") => powers.curl_up = amount,
            Some("Anger") => powers.anger = amount,
            Some("Metallicize") => powers.metallicize = amount,
            Some("Plated Armor") => powers.plated_armor = amount,
            Some("Flight") => powers.flight = amount,
            Some("Painful Stabs") => powers.painful_stabs = 1,
            Some("Spore Cloud") => powers.spore_cloud = amount,
            Some("Generic Strength Up Power") => powers.strength_up = amount,
            Some("Malleable") => {
                powers.malleable = amount;
                powers.malleable_base = int(power, "misc").max(0);
            }
            _ => {}
        }
    }
    powers
}

#[cfg(test)]
fn player_powers_and_temp_strength(value: Option<&Value>) -> (PlayerPowers, i32) {
    let mut powers = PlayerPowers::default();
    let mut temp_strength = 0;
    let Some(items) = value.and_then(Value::as_array) else {
        return (powers, temp_strength);
    };
    for power in items {
        let amount = int(power, "amount");
        match power_id(power).as_deref() {
            Some("Strength") => powers.strength = amount,
            Some("Strength Down") | Some("Flex") => temp_strength = amount,
            Some("Weak") | Some("Weakened") => powers.weak = amount,
            Some("Dexterity") => powers.dexterity = amount,
            Some("Frail") => powers.frail = amount,
            Some("Vulnerable") => powers.vulnerable = amount,
            Some("Ritual") | Some("Demon Form") => powers.ritual = amount,
            Some("Metallicize") => powers.metallicize = amount,
            Some("Thorns") => powers.thorns = amount,
            Some("Combust") => {
                powers.combust = 1;
                powers.combust_damage = amount;
            }
            Some("Dark Embrace") => powers.dark_embrace = amount,
            Some("Rupture") => powers.rupture = amount,
            Some("Hex") => powers.hex = amount,
            _ => {}
        }
    }
    powers.strength -= temp_strength;
    (powers, temp_strength)
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

#[cfg(test)]
fn reward_choices_from_observed(game: &Value) -> Vec<CardInstance> {
    game.get("screen_state")
        .and_then(|state| state.get("cards"))
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .enumerate()
                .filter_map(|(index, card)| {
                    content_id_from_card_value(card).map(|content_id| {
                        CardInstance::new(CardId::new(900 + index as u64), content_id)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_reward_choice(message: &Value, choice_index: usize) -> Option<&Value> {
    message
        .get("game_state")?
        .get("screen_state")?
        .get("cards")?
        .as_array()?
        .get(choice_index)
}

#[cfg(test)]
fn card_instances_from_array(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    card_instances_from_array_impl(value, base_id, false, false)
}

#[cfg(test)]
#[allow(dead_code)]
fn combat_card_instances_from_array(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    card_instances_from_array_impl(value, base_id, false, true)
}

#[cfg(test)]
#[allow(dead_code)]
fn hand_from_comm_mod_visible_order(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn draw_pile_from_comm_mod_visible_order(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn discard_pile_from_comm_mod_visible_order(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn exhaust_pile_from_comm_mod_visible_order(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
fn card_instances_from_array_impl(
    value: Option<&Value>,
    base_id: u64,
    use_observed_shrug_plus: bool,
    use_observed_cost: bool,
) -> Vec<CardInstance> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            let content_id = if use_observed_shrug_plus {
                content_id_from_card_value_with_observed_shrug_plus(card)
            } else {
                content_id_from_card_value(card)
            }?;
            let mut instance = CardInstance::new(CardId::new(base_id + index as u64), content_id);
            instance.upgrades = card_upgrade_count(card);
            if use_observed_cost {
                if let Some(cost) = card
                    .get("cost")
                    .and_then(Value::as_i64)
                    .and_then(|cost| u8::try_from(cost).ok())
                {
                    instance.temp_cost = Some(cost);
                }
            }
            Some(instance)
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn combat_card_instances_from_array_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    card_instances_from_array_impl(value, base_id, true, true)
}

#[cfg(test)]
#[allow(dead_code)]
fn hand_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn draw_pile_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn discard_pile_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn exhaust_pile_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn card_instances_from_array_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            content_id_from_card_value_with_observed_shrug_plus(card).map(|content_id| {
                let mut instance =
                    CardInstance::new(CardId::new(base_id + index as u64), content_id);
                instance.upgrades = card_upgrade_count(card);
                instance
            })
        })
        .collect()
}

#[cfg(test)]
fn card_upgrade_count(card: &Value) -> u8 {
    card.get("upgrades")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0)
}

fn content_id_from_card_value(card: &Value) -> Option<ContentId> {
    let id = card.get("id").and_then(Value::as_str)?;
    let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
    let base = content_id_from_key(id)?;
    if upgrades > 0 {
        return upgrade_content_id(base).or(Some(base));
    }
    Some(base)
}

#[cfg(test)]
fn content_id_from_card_value_with_observed_shrug_plus(card: &Value) -> Option<ContentId> {
    let id = card.get("id").and_then(Value::as_str)?;
    let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
    let base = content_id_from_key(id)?;
    if upgrades > 0 && base == sts_core::content::cards::SHRUG_IT_OFF_ID {
        return Some(sts_core::content::cards::SHRUG_IT_OFF_PLUS_ID);
    }
    content_id_from_card_value(card)
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
        BLUDGEON_ID, BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID, BURN_ID, CARNAGE_ID, CLASH_ID,
        CLEAVE_ID, CLOTHESLINE_ID, CLUMSY_ID, COMBUST_ID, CORRUPTION_ID, CORRUPTION_PLUS_ID,
        DARK_EMBRACE_ID, DARK_SHACKLES_ID, DAZED_ID, DECAY_ID, DEEP_BREATH_ID, DEFEND_R_ID,
        DEFEND_R_PLUS_ID, DEMON_FORM_ID, DISARM_ID, DISCOVERY_ID, DOUBLE_TAP_ID,
        DOUBLE_TAP_PLUS_ID, DOUBT_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID,
        ENTRENCH_ID, EVOLVE_ID, EXHUME_ID, FEED_ID, FEEL_NO_PAIN_ID, FIEND_FIRE_ID,
        FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID, GHOSTLY_ARMOR_ID, HAVOC_ID, HEADBUTT_ID,
        HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, IMMOLATE_PLUS_ID, INFERNAL_BLADE_ID,
        INFLAME_ID, INJURY_ID, INTIMIDATE_ID, IRON_WAVE_ID, JACK_OF_ALL_TRADES_ID, JUGGERNAUT_ID,
        LIMIT_BREAK_ID, METALLICIZE_ID, METALLICIZE_PLUS_ID, NORMALITY_ID, OFFERING_ID, PAIN_ID,
        PARASITE_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID,
        RAMPAGE_ID, REAPER_ID, REAPER_PLUS_ID, RECKLESS_CHARGE_ID, REGRET_ID, RUPTURE_ID,
        RUPTURE_PLUS_ID, SEARING_BLOW_ID, SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID,
        SEVER_SOUL_ID, SHAME_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID, SLIMED_ID,
        SPOT_WEAKNESS_ID, STRIKE_R_ID, SWIFT_STRIKE_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID,
        TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WARCRY_PLUS_ID,
        WHIRLWIND_ID, WILD_STRIKE_ID, WOUND_ID, WRITHE_ID,
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
        _ => None,
    }
}

fn content_key(content_id: ContentId) -> &'static str {
    if let Some(definition) = sts_core::content::cards::get_card_definition(content_id) {
        return definition.name;
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
        METALLICIZE_PLUS_ID, NORMALITY_ID, OFFERING_ID, OFFERING_PLUS_ID, PAIN_ID, PARASITE_ID,
        PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, RAGE_ID, RAMPAGE_ID,
        REAPER_ID, REAPER_PLUS_ID, REGRET_ID, RUPTURE_ID, RUPTURE_PLUS_ID, SEARING_BLOW_ID,
        SECRET_WEAPON_ID, SENTINEL_ID, SEVER_SOUL_ID, SHAME_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID,
        SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, SPOT_WEAKNESS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID,
        SWIFT_STRIKE_ID, SWIFT_STRIKE_PLUS_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID,
        TRANSMUTATION_ID, TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID,
        WARCRY_PLUS_ID, WHIRLWIND_ID, WILD_STRIKE_ID, WOUND_ID, WRITHE_ID,
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
    if sts_core::content::cards::get_card_definition(card.content_id).is_none() {
        if let Some(key) = sts_core::run::reward::any_color_reward_card_key(card.content_id) {
            return key.replace('_', " ");
        }
    }
    reward_card_display_key(run, card.content_id).to_owned()
}

fn egg_preview_upgrade(run: &RunState, content_id: ContentId) -> Option<ContentId> {
    let upgraded = upgrade_content_id(content_id)?;
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
        .filter_map(|card| {
            content_id_from_card_value(card)
                .map(|content_id| deck_content_key(content_id).to_owned())
                .or_else(|| card.get("id").and_then(Value::as_str).map(str::to_owned))
        })
        .collect()
}

fn deck_content_keys(deck: &[CardInstance]) -> Vec<String> {
    deck.iter()
        .map(|card| deck_content_key(card.content_id).to_owned())
        .collect()
}

fn deck_content_keys_after_pending_obtain_cards_settle(run: &RunState) -> Vec<String> {
    let mut settled = run.clone();
    settled.flush_pending_obtain_cards();
    deck_content_keys(&settled.deck)
}

fn classify_deferred_deck_observation(
    observed: &[String],
    transient: &[String],
    settled: &[String],
) -> PendingDeckObservation {
    if observed == settled {
        PendingDeckObservation::Settled
    } else if observed == transient {
        PendingDeckObservation::Deferred
    } else {
        PendingDeckObservation::Diverged(subset_diffs(json!(observed), json!(settled)))
    }
}

fn seed_start_observed_deck(message: &Value) -> Vec<String> {
    message
        .get("game_state")
        .map(|game| deck_keys_from_value(game.get("deck")))
        .unwrap_or_default()
}

fn screen_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("screen_type"))
        .and_then(Value::as_str)
}

#[cfg(test)]
#[allow(dead_code)]
fn room_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("room_type"))
        .and_then(Value::as_str)
}

#[cfg(test)]
#[allow(dead_code)]
fn first_choice(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_str)
}

#[cfg(test)]
#[allow(dead_code)]
fn unsupported_reason(pre: &TraceState, action: &TraceAction) -> String {
    match action.command.split_whitespace().next().unwrap_or("") {
        "START" => "seed-start run creation is source-backed/generated for selected Ironclad A0 surfaces, with remaining map, Neow branch-combo, and reward RNG parity gaps classified".to_owned(),
        "CHOOSE" if screen_type(&pre.message) == Some("EVENT") => {
            "Neow/event choice side effects are unsupported in sim-to-real replay".to_owned()
        }
        "CHOOSE" if screen_type(&pre.message) == Some("MAP") => {
            "map node selection is unsupported until exact seed-to-map parity is implemented".to_owned()
        }
        "CHOOSE" if screen_type(&pre.message) == Some("COMBAT_REWARD") => {
            "reward card-screen opening is a UI transition; card pickup is verified from CARD_REWARD".to_owned()
        }
        "CHOOSE"
            if matches!(
                screen_type(&pre.message),
                Some("SHOP_ROOM" | "SHOP_SCREEN" | "GRID")
            ) && room_type(&pre.message).is_some_and(|room| room.eq_ignore_ascii_case("ShopRoom")) =>
        {
            "shop UI choices are covered by seed-start shop replay".to_owned()
        }
        "CONFIRM"
            if screen_type(&pre.message) == Some("GRID")
                && room_type(&pre.message).is_some_and(|room| room.eq_ignore_ascii_case("ShopRoom")) =>
        {
            "shop UI choices are covered by seed-start shop replay".to_owned()
        }
        "LEAVE"
            if matches!(screen_type(&pre.message), Some("SHOP_ROOM" | "SHOP_SCREEN"))
                && room_type(&pre.message).is_some_and(|room| room.eq_ignore_ascii_case("ShopRoom")) =>
        {
            "shop UI choices are covered by seed-start shop replay".to_owned()
        }
        "PROCEED" => "reward-to-map UI transition is out-of-scope for simulator state parity".to_owned(),
        "state" => "trace client poll command is not a game transition".to_owned(),
        _ => "unsupported or unobservable CommunicationMod command".to_owned(),
    }
}

fn intent_key(monster: &MonsterState) -> String {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, BANDIT_BEAR_ID, BANDIT_LEADER_ID, BRONZE_ORB_ID, BYRD_ID, CHOSEN_ID,
        GREMLIN_WIZARD_ID, GUARDIAN_ID, HEXAGHOST_ID, LAGAVULIN_ID, RED_LOUSE_ID, SLIME_BOSS_ID,
        SNECKO_ID, SPIKER_ID, SPIKE_SLIME_ID,
    };

    match monster.intent {
        MonsterIntent::Attack { .. }
        | MonsterIntent::AttackAddSlimedToDiscard { .. }
        | MonsterIntent::AttackAddWoundsToDiscard { .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { .. }
        | MonsterIntent::AttackApplyPlayerFrail { .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
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
                    | MonsterIntent::AttackAddSlimedToDiscard { .. }
                    | MonsterIntent::AttackAddWoundsToDiscard { .. }
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
                SLIME_BOSS_ID | HEXAGHOST_ID | BANDIT_LEADER_ID
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

#[cfg(test)]
fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
fn power_id(power: &Value) -> Option<String> {
    power
        .get("id")
        .or_else(|| power.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn insert<T: Serialize>(map: &mut serde_json::Map<String, Value>, key: &str, value: T) {
    map.insert(
        key.to_owned(),
        serde_json::to_value(value).expect("json value"),
    );
}

fn push_sim_error(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    err: sts_core::SimError,
) {
    report.unexpected_diffs.push(UnexpectedDiff {
        action_step: action.step,
        command: action.command.clone(),
        label: label.to_owned(),
        diffs: vec![format!("simulator rejected transition: {err:?}")],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_core::content::cards::{
        BASH_PLUS_ID, BATTLE_TRANCE_ID, BURN_ID, COMBUST_ID, CORRUPTION_PLUS_ID, DEFEND_R_ID,
        DEMON_FORM_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID, ENTRENCH_ID, POMMEL_STRIKE_PLUS_ID,
        STRIKE_R_PLUS_ID, TWIN_STRIKE_ID,
    };
    use sts_core::content::monsters::{monster_state, SLAVER_BLUE_A0};
    use sts_core::relic::IRONCLAD_BOSS_RELIC_POOL;

    #[test]
    fn smoke_bomb_transient_projection_preserves_the_core_destination() {
        let mut source = RunState::map_fixture();
        source.player_hp = 10;
        source.player_max_hp = 80;
        source.potions = vec![Potion::SmokeBomb];
        source.phase = RunPhase::Combat;
        let mut combat = source.init_combat(CombatState::initial_fixture());
        combat.player.hp = 10;
        combat.player.max_hp = 80;
        source.combat = Some(combat);

        let destination = apply_run_action(
            &source,
            RunAction::UsePotion {
                slot: 0,
                target: None,
            },
        )
        .expect("Smoke Bomb reaches the core escape destination");
        assert_eq!(destination.phase, RunPhase::Idle);
        assert!(destination.combat.is_none());
        assert!(destination.reward.is_none());
        assert!(destination.potions.is_empty());

        let projection = seed_start_smoke_bomb_transient_simulated_subset(&source, &destination);
        assert_eq!(projection["screen_type"], json!("NONE"));
        assert_eq!(projection["potion_ids"], json!([]));
        assert!(projection.get("current_hp").is_none());
        assert!(projection.get("combat_player_hp").is_none());
        assert_eq!(source.phase, RunPhase::Combat);
        assert!(source.combat.is_some());
        assert_eq!(destination.phase, RunPhase::Idle);
        assert!(destination.combat.is_none());
    }

    #[test]
    fn smoke_bomb_trace_reconciles_escape_and_reward_proceeds_at_stable_frames() {
        let Some(content) =
            crate::load_corpus_file("communication_mod/trace-2026-07-07T18-33-54-807Z.jsonl")
        else {
            return;
        };
        let report = verify_seed_start_communication_mod_trace(&content)
            .expect("Smoke Bomb regression trace verifies");
        assert!(
            report.unexpected_diffs.is_empty(),
            "{:#?}",
            report.unexpected_diffs
        );
        assert!(report.unsupported.is_empty(), "{:#?}", report.unsupported);
        assert_eq!(
            report
                .action_integrity
                .as_ref()
                .expect("action integrity")
                .unresolved_transient_assertions,
            0
        );
        for (step, command) in [(808, "POTION USE 1 0"), (811, "PROCEED"), (812, "PROCEED")] {
            let disposition = report
                .action_dispositions
                .iter()
                .find(|entry| entry.action_step == step && entry.command == command)
                .unwrap_or_else(|| panic!("disposition for step {step} {command}"));
            assert_eq!(disposition.disposition, ActionDispositionKind::Verified);
            assert!(
                disposition.deferred_assertion_reconciled,
                "step {step} must be reconciled only after its stable frame"
            );
        }
    }

    #[test]
    fn recorded_action_input_drives_time_gated_run_state_without_gameplay_hydration() {
        let pre = TraceState {
            step: 10,
            received_at: Some("wall clock is deliberately ignored".to_owned()),
            message: json!({"game_state": {"playtime_seconds": 812.75}}),
        };
        let mut action = TraceAction {
            step: 10,
            command: "CHOOSE 0".to_owned(),
            sent_at: None,
            playtime_seconds: None,
        };
        assert_eq!(recorded_action_playtime_seconds(&pre, &action), Some(812));
        action.playtime_seconds = Some(799);
        assert_eq!(
            recorded_action_playtime_seconds(&pre, &action),
            Some(799),
            "the explicit action input wins over its source state's copy"
        );
    }

    #[test]
    fn subset_diffs_reports_known_card_against_unknown() {
        let diffs = subset_diffs(json!(["Offering+"]), json!(["unknown"]));

        assert_eq!(diffs, vec!["[0]: \"Offering+\" != \"unknown\""]);
    }

    #[test]
    fn combat_normalization_preserves_living_monster_powers_only() {
        let value = json!({
            "monsters": [
                {
                    "current_hp": 10,
                    "strength": 3,
                    "ritual": 5,
                    "vulnerable": 2,
                    "intent": "ATTACK_BUFF",
                    "move_id": 2,
                },
                {
                    "current_hp": 0,
                    "strength": 7,
                    "ritual": 4,
                    "vulnerable": 1,
                    "intent": "DEAD",
                    "move_id": 3,
                },
            ]
        });

        let normalized = seed_start_normalize_combat_compare(value, false);
        assert_eq!(normalized["monsters"][0]["strength"], json!(3));
        assert_eq!(normalized["monsters"][0]["ritual"], json!(5));
        assert_eq!(normalized["monsters"][0]["vulnerable"], json!(2));
        assert_eq!(normalized["monsters"][0]["intent"], json!("ATTACK_BUFF"));
        assert_eq!(normalized["monsters"][0]["move_id"], json!(2));
        assert!(normalized["monsters"][1].get("strength").is_none());
        assert!(normalized["monsters"][1].get("ritual").is_none());
        assert!(normalized["monsters"][1].get("vulnerable").is_none());
        assert!(normalized["monsters"][1].get("intent").is_none());
        assert!(normalized["monsters"][1].get("move_id").is_none());
    }

    #[test]
    fn terminal_player_death_hides_only_monster_intent_fields() {
        let normalized = seed_start_normalize_combat_compare(
            json!({
                "combat_player_hp": 0,
                "monsters": [{
                    "current_hp": 47,
                    "strength": 3,
                    "ritual": 2,
                    "vulnerable": 1,
                    "intent": "ATTACK",
                    "move_id": 6,
                }]
            }),
            false,
        );

        assert_eq!(normalized["monsters"][0]["strength"], json!(3));
        assert_eq!(normalized["monsters"][0]["ritual"], json!(2));
        assert_eq!(normalized["monsters"][0]["vulnerable"], json!(1));
        assert!(normalized["monsters"][0].get("intent").is_none());
        assert!(normalized["monsters"][0].get("move_id").is_none());
    }

    #[test]
    fn debug_intent_visibility_contract_still_compares_move_id() {
        let mut expected = json!({
            "monsters": [{"current_hp": 20, "intent": "DEBUG", "move_id": 2}]
        });
        let mut actual = json!({
            "monsters": [{"current_hp": 20, "intent": "ATTACK", "move_id": 3}]
        });

        apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);

        assert!(expected["monsters"][0].get("intent").is_none());
        assert!(actual["monsters"][0].get("intent").is_none());
        assert_eq!(expected["monsters"][0]["move_id"], json!(2));
        assert_eq!(actual["monsters"][0]["move_id"], json!(3));
        assert_eq!(
            subset_diffs(expected, actual),
            vec!["monsters[0].move_id: 2 != 3"]
        );
    }

    #[test]
    fn observed_monster_projection_does_not_invent_missing_move_id() {
        let monsters = seed_start_monsters_from_value(
            Some(&json!([{
                "name": "Acid Slime (S)",
                "current_hp": 12,
                "max_hp": 12,
                "block": 0,
                "intent": "ATTACK",
                "powers": [],
            }])),
            true,
        );

        assert!(monsters[0].get("move_id").is_none());

        let mut expected = json!({"monsters": monsters});
        let mut actual = json!({
            "monsters": [{
                "name": "Acid Slime (S)",
                "current_hp": 12,
                "max_hp": 12,
                "block": 0,
                "intent": "ATTACK",
                "move_id": 1,
                "strength": 0,
                "ritual": 0,
                "vulnerable": 0,
            }]
        });
        apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);
        assert!(subset_diffs(expected, actual).is_empty());
    }

    #[test]
    fn runic_dome_hides_intent_in_both_monster_projections() {
        let observed = seed_start_monsters_from_value(
            Some(&json!([{
                "name": "Cultist",
                "current_hp": 50,
                "max_hp": 50,
                "block": 0,
                "intent": "BUFF",
                "move_id": 3,
                "powers": [],
            }])),
            false,
        );
        let simulated = seed_start_monsters_from_sim(&CombatState::initial_fixture(), false, false);

        for monster in [&observed[0], &simulated[0]] {
            assert!(monster.get("intent").is_none());
            assert!(monster.get("move_id").is_none());
        }
    }

    #[test]
    fn intent_projection_preserves_distinct_communication_mod_categories() {
        use sts_core::content::monsters::{BYRD_ID, GREMLIN_WIZARD_ID, GUARDIAN_ID, SPIKER_ID};

        let mut monster = CombatState::initial_fixture().monsters.remove(0);
        let cases = [
            (MonsterIntent::Block { block: 8 }, "DEFEND"),
            (
                MonsterIntent::StrengthAndBlock {
                    strength: 2,
                    block: 8,
                },
                "DEFEND_BUFF",
            ),
            (
                MonsterIntent::AttackAndBlock {
                    damage: 7,
                    block: 5,
                },
                "ATTACK_DEFEND",
            ),
            (
                MonsterIntent::ApplyPlayerConstricted { amount: 10 },
                "STRONG_DEBUFF",
            ),
        ];
        for (intent, expected) in cases {
            monster.intent = intent;
            assert_eq!(intent_key(&monster), expected);
        }

        monster.content_id = GUARDIAN_ID;
        monster.intent = MonsterIntent::AttackMultiple {
            damage: 10,
            hits: 2,
        };
        assert_eq!(intent_key(&monster), "ATTACK_BUFF");

        monster.content_id = GREMLIN_WIZARD_ID;
        monster.intent = MonsterIntent::Block { block: 0 };
        assert_eq!(intent_key(&monster), "UNKNOWN");

        monster.content_id = BYRD_ID;
        monster.intent = MonsterIntent::StrengthSelf { amount: 0 };
        assert_eq!(intent_key(&monster), "UNKNOWN");

        monster.content_id = SPIKER_ID;
        monster.intent = MonsterIntent::StrengthAndBlock {
            strength: 2,
            block: 0,
        };
        assert_eq!(intent_key(&monster), "BUFF");
    }

    #[test]
    fn simulated_monster_projection_reports_strength_separately_from_ritual() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].powers.strength = 3;
        combat.monsters[0].powers.ritual = 3;

        let projected = seed_start_monsters_from_sim(&combat, false, true);

        assert_eq!(projected[0]["strength"], json!(3));
        assert_eq!(projected[0]["ritual"], json!(3));
    }

    #[test]
    fn victory_projection_uses_only_simulator_state() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_floor = 16;
        run.current_act = 1;
        run.current_room_override = Some(RoomKind::Boss);
        run.gold = 123;
        run.player_hp = 45;
        run.player_max_hp = 80;

        assert_eq!(
            seed_start_victory_simulated_subset(&run),
            json!({
                "screen_type": "COMBAT_REWARD",
                "floor": 16,
                "gold": 123,
                "current_hp": 45,
                "max_hp": 80,
            })
        );

        run.current_act = 3;
        run.current_floor = 51;
        assert_eq!(
            seed_start_victory_simulated_subset(&run)["screen_type"],
            json!("COMPLETE")
        );
    }

    #[test]
    fn dual_wield_hand_select_projects_only_attack_and_power_candidates() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        let mut combat = CombatState::initial_fixture();
        combat.piles.hand = vec![
            CardInstance::new(CardId::new(1), POMMEL_STRIKE_PLUS_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), COMBUST_ID),
        ];
        combat.hand_select = Some(sts_core::combat::HandSelectState {
            purpose: HandSelectPurpose::DualWieldCopy,
            source_card_id: CardId::new(2),
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
        });
        run.combat = Some(combat);
        let projected = seed_start_simulated_combat_subset(&run, false);

        assert_eq!(projected["hand_ids"], json!(["Pommel Strike", "Combust"]));
        assert_eq!(projected["screen_type"], json!("HAND_SELECT"));
    }

    #[test]
    fn trace_transitions_preserve_delayed_map_choice_across_state_polls() {
        let lines = vec![
            TraceLine::State(TraceState {
                step: 813,
                received_at: None,
                message: json!({"game_state": {"screen_type": "MAP"}}),
            }),
            TraceLine::Action(TraceAction {
                step: 814,
                command: "CHOOSE 0".to_owned(),
                sent_at: None,
                playtime_seconds: None,
            }),
            TraceLine::Action(TraceAction {
                step: 815,
                command: "STATE".to_owned(),
                sent_at: None,
                playtime_seconds: None,
            }),
            TraceLine::State(TraceState {
                step: 815,
                received_at: None,
                message: json!({"game_state": {"screen_type": "MAP"}}),
            }),
            TraceLine::Action(TraceAction {
                step: 816,
                command: "STATE".to_owned(),
                sent_at: None,
                playtime_seconds: None,
            }),
            TraceLine::State(TraceState {
                step: 816,
                received_at: None,
                message: json!({
                    "game_state": {
                        "screen_type": "NONE",
                        "room_phase": "COMBAT"
                    }
                }),
            }),
        ];

        let transitions = trace_transitions(&lines).expect("trace transitions");

        assert_eq!(transitions.transitions.len(), 1);
        assert_eq!(transitions.transitions[0].1.command, "CHOOSE 0");
        assert_eq!(transitions.transitions[0].2.step, 816);
        assert_eq!(transitions.ignored_tail_actions, 0);
        assert_eq!(
            transitions.folded_action_dispositions,
            vec![
                (1, ActionDispositionKind::ObservationPoll),
                (2, ActionDispositionKind::ObservationPoll),
            ]
        );
        assert_eq!(transitions.reconciled_deferred_action_ordinals, vec![0]);
        assert_eq!(transitions.unresolved_transient_assertions, 0);
    }

    #[test]
    fn trace_transitions_wait_past_timer_only_and_busy_combat_states() {
        let state = |step, playtime_seconds, ready_for_command, energy| {
            TraceLine::State(TraceState {
                step,
                received_at: None,
                message: json!({
                    "ready_for_command": ready_for_command,
                    "game_state": {
                        "playtime_seconds": playtime_seconds,
                        "screen_type": "NONE",
                        "combat_state": {"player": {"energy": energy}}
                    }
                }),
            })
        };
        let poll = |step| {
            TraceLine::Action(TraceAction {
                step,
                command: "STATE".to_owned(),
                sent_at: None,
                playtime_seconds: None,
            })
        };
        let lines = vec![
            state(1, 10.0, true, 3),
            TraceLine::Action(TraceAction {
                step: 2,
                command: "PLAY 2 1".to_owned(),
                sent_at: None,
                playtime_seconds: Some(10),
            }),
            state(2, 10.1, true, 3),
            poll(3),
            state(3, 10.2, false, 2),
            poll(4),
            state(4, 10.3, true, 2),
        ];

        let transitions = trace_transitions(&lines).expect("trace transitions");

        assert_eq!(transitions.transitions.len(), 1);
        assert_eq!(transitions.transitions[0].1.command, "PLAY 2 1");
        assert_eq!(transitions.transitions[0].2.step, 4);
        assert_eq!(transitions.ignored_tail_actions, 0);
        assert_eq!(
            transitions.folded_action_dispositions,
            vec![
                (1, ActionDispositionKind::ObservationPoll),
                (2, ActionDispositionKind::ObservationPoll),
            ]
        );
        assert_eq!(transitions.reconciled_deferred_action_ordinals, vec![0]);
        assert_eq!(transitions.unresolved_transient_assertions, 0);
    }

    #[test]
    fn trace_transitions_fold_mushrooms_fight_confirmation_into_event_choice() {
        let state = |step, event_id: Option<&str>, choices: &[&str], screen_type| {
            TraceLine::State(TraceState {
                step,
                received_at: None,
                message: json!({
                    "ready_for_command": true,
                    "game_state": {
                        "screen_type": screen_type,
                        "choice_list": choices,
                        "screen_state": {"event_id": event_id}
                    }
                }),
            })
        };
        let choose = |step| {
            TraceLine::Action(TraceAction {
                step,
                command: "CHOOSE 0".to_owned(),
                sent_at: None,
                playtime_seconds: Some(10),
            })
        };
        let lines = vec![
            state(1, Some("Mushrooms"), &["stomp", "eat"], "EVENT"),
            choose(2),
            state(2, Some("Mushrooms"), &["fight"], "EVENT"),
            choose(3),
            state(3, None, &[], "NONE"),
        ];

        let transitions = trace_transitions(&lines).expect("trace transitions");
        assert_eq!(transitions.transitions.len(), 1);
        assert_eq!(transitions.transitions[0].1.step, 2);
        assert_eq!(
            screen_type(&transitions.transitions[0].2.message),
            Some("NONE")
        );
        assert_eq!(transitions.ignored_tail_actions, 0);
        assert_eq!(
            transitions.folded_action_dispositions,
            vec![(1, ActionDispositionKind::FoldedTargetConfirmation)]
        );
        assert_eq!(transitions.reconciled_deferred_action_ordinals, vec![0]);
        assert_eq!(transitions.unresolved_transient_assertions, 0);
    }

    #[test]
    fn trace_transitions_wait_for_cursed_key_chest_curse_effect() {
        let state = |step, screen_type: &str, deck: &[&str]| {
            TraceLine::State(TraceState {
                step,
                received_at: None,
                message: json!({
                    "ready_for_command": true,
                    "game_state": {
                        "deck": deck.iter().map(|id| json!({"id": id})).collect::<Vec<_>>(),
                        "relics": [{"id": "Cursed Key", "counter": -1}],
                        "room_type": "TreasureRoom",
                        "screen_type": screen_type
                    }
                }),
            })
        };
        let lines = vec![
            state(1, "CHEST", &["Strike_R"]),
            TraceLine::Action(TraceAction {
                step: 2,
                command: "CHOOSE 0".to_owned(),
                sent_at: None,
                playtime_seconds: Some(10),
            }),
            state(2, "COMBAT_REWARD", &["Strike_R"]),
            TraceLine::Action(TraceAction {
                step: 3,
                command: "STATE".to_owned(),
                sent_at: None,
                playtime_seconds: None,
            }),
            state(3, "COMBAT_REWARD", &["Strike_R", "Writhe"]),
        ];

        let transitions = trace_transitions(&lines).expect("trace transitions");
        assert_eq!(transitions.transitions.len(), 1);
        assert_eq!(transitions.transitions[0].1.step, 2);
        assert_eq!(transitions.transitions[0].2.step, 3);
        assert_eq!(transitions.ignored_tail_actions, 0);
        assert_eq!(
            transitions.folded_action_dispositions,
            vec![(1, ActionDispositionKind::ObservationPoll)]
        );
        assert_eq!(transitions.reconciled_deferred_action_ordinals, vec![0]);
        assert_eq!(transitions.unresolved_transient_assertions, 0);
    }

    #[test]
    fn trace_transitions_report_unresolved_transient_at_end_of_trace() {
        let state = |step, playtime_seconds| {
            TraceLine::State(TraceState {
                step,
                received_at: None,
                message: json!({
                    "ready_for_command": true,
                    "game_state": {
                        "playtime_seconds": playtime_seconds,
                        "screen_type": "NONE",
                        "combat_state": {"player": {"energy": 3}}
                    }
                }),
            })
        };
        let lines = vec![
            state(1, 10.0),
            TraceLine::Action(TraceAction {
                step: 2,
                command: "PLAY 2 1".to_owned(),
                sent_at: None,
                playtime_seconds: Some(10),
            }),
            state(2, 10.1),
        ];

        let transitions = trace_transitions(&lines).expect("trace transitions");
        assert!(transitions.transitions.is_empty());
        assert_eq!(transitions.ignored_action_ordinals, vec![0]);
        assert_eq!(transitions.ignored_tail_actions, 1);
        assert_eq!(transitions.unresolved_transient_assertions, 1);
    }

    #[test]
    fn trace_transitions_classify_target_rejection_without_ignored_tail() {
        let lines = vec![
            TraceLine::State(TraceState {
                step: 6,
                received_at: None,
                message: json!({"ready_for_command": true}),
            }),
            TraceLine::Action(TraceAction {
                step: 7,
                command: "POTION USE 1".to_owned(),
                sent_at: None,
                playtime_seconds: None,
            }),
            TraceLine::Error(crate::TraceError {
                step: 7,
                message: json!({"error": "Potion cannot be used"}),
            }),
        ];

        let transitions = trace_transitions(&lines).expect("trace transitions");
        assert!(transitions.transitions.is_empty());
        assert_eq!(transitions.rejected_action_dispositions.len(), 1);
        assert_eq!(transitions.rejected_action_dispositions[0].0, 0);
        assert!(transitions.rejected_action_dispositions[0]
            .1
            .contains("Potion cannot be used"));
        assert_eq!(transitions.ignored_tail_actions, 0);
        assert_eq!(transitions.unresolved_transient_assertions, 0);
    }

    #[test]
    fn reward_projection_lists_every_pending_orrery_card_reward() {
        let reward = RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: true,
            pending_card_reward_count: sts_core::relic::ORRERY_CARD_REWARDS,
        };

        assert_eq!(
            sim_reward_combat_choices(&reward),
            vec!["card".to_owned(); sts_core::relic::ORRERY_CARD_REWARDS as usize]
        );
    }

    #[test]
    fn fusion_hammer_removes_smith_from_seed_start_rest_projection() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Rest;
        run.relics.push(Relic::FusionHammer);

        assert_eq!(seed_start_rest_screen_actions(&run), vec![RestAction::Heal]);
        assert_eq!(
            seed_start_rest_simulated_subset(&run, &[])["choices"],
            json!(["rest"])
        );
    }

    #[test]
    fn seed_start_rest_projection_uses_dynamic_relic_action_order() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Rest;
        run.relics.extend([
            Relic::CoffeeDripper,
            Relic::PeacePipe,
            Relic::Girya,
            Relic::Shovel,
        ]);

        assert_eq!(
            seed_start_rest_screen_actions(&run),
            vec![
                RestAction::OpenSmith,
                RestAction::OpenRemove,
                RestAction::Lift,
                RestAction::Dig,
            ]
        );
        assert_eq!(
            seed_start_rest_simulated_subset(&run, &[])["choices"],
            json!(["smith", "toke", "lift", "dig"])
        );
    }

    #[test]
    fn seed_start_potion_command_drops_stray_target_for_targetless_potion() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.potions = vec![Potion::Dexterity];

        let command = ParsedPotionUse {
            slot: 0,
            target: Some(MonsterId::new(1)),
        };

        assert_eq!(seed_start_potion_command_target(&run, &command), None);
    }

    #[test]
    fn seed_start_potion_command_keeps_target_for_targeted_potion() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.potions = vec![Potion::Fire];

        let target = MonsterId::new(1);
        let command = ParsedPotionUse {
            slot: 0,
            target: Some(target),
        };

        assert_eq!(
            seed_start_potion_command_target(&run, &command),
            Some(target)
        );
    }

    #[test]
    fn slaver_blue_weak_attack_projects_to_observed_attack_debuff_intent() {
        let mut monster = monster_state(&SLAVER_BLUE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackApplyPlayerWeak { damage: 7, weak: 1 };

        assert_eq!(seed_start_trace_intent(&monster), "ATTACK_DEBUFF");
    }

    #[test]
    fn slaver_blue_attack_move_four_imports_weak_attack() {
        let monster = json!({
            "id": "SlaverBlue",
            "name": "Slaver",
            "intent": "ATTACK",
            "move_id": 4,
            "move_base_damage": 7,
            "move_hits": 1
        });

        assert_eq!(
            observed_intent(&monster, sts_core::content::monsters::SLAVER_BLUE_ID, 0),
            MonsterIntent::AttackApplyPlayerWeak { damage: 7, weak: 1 }
        );
    }

    #[test]
    fn observed_card_reward_preserves_corruption_plus() {
        let game = json!({
            "screen_type": "CARD_REWARD",
            "screen_state": {
                "cards": [
                    {
                        "id": "Corruption",
                        "name": "Corruption+",
                        "upgrades": 1
                    }
                ]
            }
        });

        let choices = reward_choices_from_observed(&game);

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].content_id, CORRUPTION_PLUS_ID);
    }

    #[test]
    fn observed_reward_imports_communication_mod_gamblers_brew_id() {
        let game = json!({
            "screen_type": "COMBAT_REWARD",
            "screen_state": {
                "rewards": [
                    {
                        "reward_type": "POTION",
                        "potion": {
                            "id": "GamblersBrew",
                            "name": "Gambler's Brew"
                        }
                    }
                ]
            }
        });

        assert_eq!(
            observed_reward_potion_offer(&game),
            Some(Potion::GamblersBrew)
        );
    }

    #[test]
    fn full_belt_potion_reward_command_fails_without_consuming_another_reward() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Reward;
        run.gold = 99;
        run.potions = vec![Potion::BlessingOfTheForge, Potion::Dexterity, Potion::Power];
        run.reward = Some(RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 120,
            stolen_gold_offer: 0,
            potion_offer: Some(Potion::Dexterity),
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });
        let pre = json!({
            "game_state": {
                "screen_state": {
                    "rewards": [
                        {"reward_type": "GOLD", "gold": 120},
                        {"reward_type": "POTION", "potion": {"id": "Dexterity Potion", "name": "Dexterity Potion"}}
                    ]
                }
            }
        });
        let error = seed_start_apply_reward_choose(&mut run, "CHOOSE 1", &pre)
            .expect_err("full-belt potion reward must fail closed");

        assert_eq!(error, "illegal action: potion belt is full");
        assert_eq!(run.gold, 99);
        let reward = run.reward.as_ref().expect("reward screen remains");
        assert_eq!(reward.gold_offer, 120);
        assert_eq!(reward.potion_offer, Some(Potion::Dexterity));
    }

    #[test]
    fn potion_reward_command_takes_simulated_offer() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Reward;
        run.potions.clear();
        run.reward = Some(RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: Some(Potion::Dexterity),
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });
        let pre = json!({
            "game_state": {
                "screen_state": {
                    "rewards": [
                        {"reward_type": "POTION", "potion": {"id": "Dexterity Potion", "name": "Dexterity Potion"}}
                    ]
                }
            }
        });

        let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 0", &pre)
            .expect("available simulator potion reward is taken");

        assert_eq!(label, "potion reward");
        assert_eq!(run.potions, vec![Potion::Dexterity]);
        assert!(run
            .reward
            .as_ref()
            .expect("reward screen remains")
            .potion_offer
            .is_none());
    }

    #[test]
    fn singing_bowl_is_exposed_and_applied_on_active_card_rewards() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Reward;
        run.relics.push(Relic::SingingBowl);
        let card = run.deck[0];
        run.reward = Some(RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: vec![card],
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: true,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });
        run.current_floor = 8;
        let subset = seed_start_reward_simulated_subset(&run, &[]);
        assert_eq!(
            subset["choices"].as_array().unwrap().last(),
            Some(&json!("bowl"))
        );

        let pre = json!({"game_state": {"choice_list": ["strike", "bowl"]}});
        let max_hp = run.player_max_hp;
        let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 1", &pre)
            .expect("Singing Bowl choice applies");
        assert_eq!(label, "singing bowl card reward");
        assert!(run.player_max_hp > max_hp);
    }

    #[test]
    fn reward_projection_uses_simulated_order_and_visible_gold_amounts() {
        let mut run = RunState::map_fixture();
        run.current_floor = 8;
        run.phase = RunPhase::Reward;
        run.reward = Some(RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 17,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: true,
            pending_card_reward_count: 1,
        });
        let observed_message = json!({
            "game_state": {
                "screen_type": "COMBAT_REWARD",
                "floor": 8,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": [],
                "relics": [],
                "choice_list": ["card", "gold"],
                "screen_state": {
                    "rewards": [
                        {"reward_type": "CARD"},
                        {"reward_type": "GOLD", "gold": 999}
                    ]
                }
            }
        });

        let simulated = seed_start_reward_simulated_subset(&run, &[]);
        let observed = seed_start_reward_observed_subset(&observed_message);
        assert_eq!(simulated["choices"], json!(["gold", "card"]));
        assert_eq!(observed["choices"], json!(["card", "gold"]));
        assert_eq!(simulated["gold_offer"], 17);
        assert_eq!(observed["gold_offer"], 999);
    }

    #[test]
    fn relic_mismatch_is_reported_without_mutating_simulator_state() {
        let mut run = RunState::map_fixture();
        run.ensure_ironclad_relic_pools();
        let pools = run.relic_pools.as_mut().unwrap();
        pools.common.retain(|key| *key != RelicKey::TheBoot);
        assert!(pools.uncommon.contains(&RelicKey::Pantograph));
        run.phase = RunPhase::Reward;
        run.reward = Some(RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: Some(Relic::TheBoot),
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });

        let before = run.clone();
        let error = verify_primary_relic_offer_matches_observed(&run, RelicKey::Pantograph)
            .expect_err("relic mismatch must stop deterministic replay");
        assert!(error.contains("observed Pantograph"));
        assert_eq!(run, before);
        let pools = run.relic_pools.as_ref().unwrap();
        assert_ne!(pools.common.first(), Some(&RelicKey::TheBoot));
        assert!(pools.uncommon.contains(&RelicKey::Pantograph));
    }

    #[test]
    fn match_and_keep_transient_choices_ignore_only_stale_omissions() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::MatchAndKeep));
        let simulated = json!({"choices": ["card0", "card1", "card2", "card3", "card4"]});
        let mut transient = json!({"choices": ["card0", "card1", "card2", "card4"]});
        normalize_match_and_keep_transient_choices(&run, &mut transient, &simulated);
        assert_eq!(transient, simulated);

        let mut wrong = json!({"choices": ["card0", "card1", "card5"]});
        normalize_match_and_keep_transient_choices(&run, &mut wrong, &simulated);
        assert_ne!(wrong, simulated);

        let duplicate_simulated = json!({"choices": ["bash", "card1", "brutality", "bash"]});
        let mut stale_duplicate = json!({"choices": ["bash", "card1", "brutality", "brutality"]});
        normalize_match_and_keep_transient_choices(
            &run,
            &mut stale_duplicate,
            &duplicate_simulated,
        );
        assert_eq!(stale_duplicate, duplicate_simulated);

        let mut different_reveal = json!({"choices": ["bash", "card1", "brutality", "cleave"]});
        normalize_match_and_keep_transient_choices(
            &run,
            &mut different_reveal,
            &duplicate_simulated,
        );
        assert_ne!(different_reveal, duplicate_simulated);
    }

    #[test]
    fn seed_start_act2_combat_entry_uses_city_spawn_helper() {
        let seed = 1_218_623;
        let floor = 18;
        let combat_index = 0;
        let message = json!({
            "game_state": {
                "screen_type": "COMBAT",
                "ascension_level": 0,
                "floor": floor,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": [{"id": "Strike_R"}],
                "relics": [],
                "combat_state": {
                    "player": {
                        "current_hp": 80,
                        "block": 0,
                        "energy": 3
                    },
                    "monsters": []
                }
            }
        });

        let expected = seed_start_encounter_expected_at_index(
            seed,
            combat_index,
            0,
            &["Strike_R".to_owned()],
            &[],
            false,
            &message,
        );
        let actual_monsters = expected
            .get("monsters")
            .and_then(Value::as_array)
            .expect("expected monsters");
        let city_spawns =
            target_city_normal_encounter_spawn_at_combat_index(seed, floor, combat_index, 0, false)
                .expect("city spawn metadata");
        let exordium_spawns =
            target_normal_encounter_spawn_at_combat_index(seed, floor, combat_index, 0, false)
                .expect("exordium spawn metadata");

        assert_eq!(actual_monsters.len(), city_spawns.len());
        assert_eq!(
            actual_monsters[0].get("name").and_then(Value::as_str),
            Some(city_spawns[0].name)
        );
        assert_ne!(city_spawns[0].name, exordium_spawns[0].name);
    }

    #[test]
    fn seed_start_act2_room_kind_resolution_uses_city_map_stream() {
        let seed = 1_218_623;
        let path = vec![0];
        let act1_message = json!({"game_state": {"floor": 1}});
        let act2_message = json!({"game_state": {"floor": 18}});

        assert_eq!(
            seed_start_target_act_from_message(&act1_message),
            TargetMapAct::Exordium
        );
        assert_eq!(
            seed_start_target_act_from_message(&act2_message),
            TargetMapAct::City
        );
        assert_eq!(
            seed_start_room_kinds_on_path(seed, &path, &act2_message),
            city_room_kinds_on_path(seed, &path)
        );
        assert_eq!(
            seed_start_room_kinds_on_path(seed, &path, &act1_message),
            exordium_room_kinds_on_path(seed, &path)
        );
        assert_eq!(seed_start_target_act_from_floor(18), TargetMapAct::City);
    }

    #[test]
    fn trace_replay_parses_unknown_exit_metadata_and_supports_empty_trace() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"metadata","event":"exit","ended_at":"now"}"#;

        let error = verify_communication_mod_trace(content).expect_err("strict replay needs START");
        assert!(matches!(error, SimRealError::MissingStartCommand));
    }

    #[test]
    fn malformed_choose_is_rejected_instead_of_selecting_choice_zero() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"state","step":0,"message":{}}
{"type":"action","step":1,"command":"START IRONCLAD 0 VERIFY01"}
{"type":"state","step":1,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":0,"gold":99,"current_hp":80,"max_hp":80,"deck":[{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Bash"}],"relics":[{"name":"Burning Blood"}],"choice_list":["talk"]}}}
{"type":"action","step":2,"command":"CHOOSE nope"}
{"type":"state","step":2,"message":{"game_state":{"screen_type":"EVENT","choice_list":["talk"]}}}"#;

        let error = verify_communication_mod_trace(content).expect_err("malformed trace rejected");
        assert!(matches!(
            error,
            SimRealError::MalformedChooseCommand {
                step: 2,
                ref command,
            } if command == "CHOOSE nope"
        ));
        assert!(matches!(
            crate::assess_verification(
                Err(&error),
                &crate::VerificationExpectation::Complete,
                None,
            ),
            crate::VerificationOutcome::InvalidInput { reason }
                if reason.contains("expected exactly `CHOOSE <non-negative index>`")
        ));
    }

    #[test]
    fn observed_boss_identity_is_compared_without_steering_simulation() {
        let path = crate::corpus_path("permanent_traces/trace-2026-07-03T20-12-12-408Z.jsonl");
        let content = std::fs::read_to_string(path).expect("retained trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let start = imported
            .lines
            .iter()
            .find_map(|line| match line {
                TraceLine::Action(action) => parse_start_command(action).and_then(Result::ok),
                _ => None,
            })
            .expect("trace start command");
        let expected_boss =
            RunState::placeholder_seeded_ironclad(start.numeric_seed as u64, start.ascension)
                .act1_boss;
        let forged_boss = match expected_boss {
            sts_core::Act1Boss::Guardian => "Hexaghost",
            sts_core::Act1Boss::Hexaghost | sts_core::Act1Boss::SlimeBoss => "The Guardian",
        };
        let expected_boss_name = match expected_boss {
            sts_core::Act1Boss::Guardian => "The Guardian",
            sts_core::Act1Boss::Hexaghost => "Hexaghost",
            sts_core::Act1Boss::SlimeBoss => "Slime Boss",
        };

        let mut mutated_states = 0;
        let mutated = content
            .lines()
            .map(|line| {
                let mut value: Value = serde_json::from_str(line).expect("trace line JSON");
                if value.get("type").and_then(Value::as_str) == Some("state") {
                    if let Some(act_boss) = value
                        .get_mut("message")
                        .and_then(|message| message.get_mut("game_state"))
                        .and_then(|game| game.get_mut("act_boss"))
                    {
                        *act_boss = json!(forged_boss);
                        mutated_states += 1;
                    }
                }
                serde_json::to_string(&value).expect("mutated trace line serializes")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(mutated_states > 0, "fixture must expose boss identity");

        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let boss_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| {
                diff.action_step == start.action_step && diff.label == "seed-start bootstrap"
            })
            .and_then(|diff| diff.diffs.iter().find(|line| line.starts_with("act_boss:")))
            .expect("forged observed boss must differ from seed-derived boss");
        assert!(boss_diff.contains(forged_boss), "{boss_diff}");
        assert!(boss_diff.contains(expected_boss_name), "{boss_diff}");
    }

    #[test]
    fn observed_chest_gold_is_compared_without_steering_simulation() {
        let path = crate::corpus_path("permanent_traces/trace-session-8.jsonl");
        let content = std::fs::read_to_string(path).expect("complete trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (chest_action_step, chest_post) = transitions
            .transitions
            .iter()
            .find_map(|(pre, action, post)| {
                (screen_type(&pre.message) == Some("CHEST")
                    && command_is_choose(&action.command, 0)
                    && screen_type(&post.message) == Some("COMBAT_REWARD")
                    && post
                        .message
                        .get("game_state")
                        .map(reward_gold_offer)
                        .unwrap_or(0)
                        > 0)
                .then_some((action.step, post.clone()))
            })
            .expect("fixture has a treasure chest with gold");
        let original_gold = chest_post
            .message
            .get("game_state")
            .map(reward_gold_offer)
            .expect("chest gold");
        let forged_gold = original_gold + 1_000;

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == chest_post => Some(state),
                _ => None,
            })
            .expect("chest post-state remains in imported trace");
        let rewards = mutated_state
            .message
            .pointer_mut("/game_state/screen_state/rewards")
            .and_then(Value::as_array_mut)
            .expect("chest rewards");
        let gold = rewards
            .iter_mut()
            .find(|reward| reward.get("reward_type").and_then(Value::as_str) == Some("GOLD"))
            .and_then(|reward| reward.get_mut("gold"))
            .expect("gold reward amount");
        *gold = json!(forged_gold);

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let gold_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| {
                diff.action_step == chest_action_step && diff.label == "open treasure chest"
            })
            .and_then(|diff| {
                diff.diffs
                    .iter()
                    .find(|line| line.starts_with("gold_offer:"))
            })
            .expect("forged observed chest gold must differ from simulated gold");
        assert!(
            gold_diff.contains(&original_gold.to_string()),
            "{gold_diff}"
        );
        assert!(gold_diff.contains(&forged_gold.to_string()), "{gold_diff}");
    }

    #[test]
    fn observed_chest_screen_cannot_choose_boss_reward_transition() {
        let path = crate::corpus_path("permanent_traces/trace-session-8.jsonl");
        let content = std::fs::read_to_string(path).expect("complete trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (chest_action_step, reward_post) = transitions
            .transitions
            .iter()
            .find_map(|(pre, action, post)| {
                (screen_type(&pre.message) == Some("CHEST")
                    && command_is_choose(&action.command, 0)
                    && screen_type(&post.message) == Some("COMBAT_REWARD")
                    && trace_room_type(&pre.message) != Some("TreasureRoomBoss"))
                .then_some((action.step, post.clone()))
            })
            .expect("fixture opens an ordinary treasure chest");

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == reward_post => Some(state),
                _ => None,
            })
            .expect("treasure reward post-state remains in imported trace");
        *mutated_state
            .message
            .pointer_mut("/game_state/screen_type")
            .expect("treasure reward screen type") = json!("BOSS_REWARD");

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let screen_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| {
                diff.action_step == chest_action_step && diff.label == "open treasure chest"
            })
            .and_then(|diff| {
                diff.diffs
                    .iter()
                    .find(|line| line.starts_with("screen_type:"))
            })
            .expect("forged boss screen must differ from the core-owned treasure reward");
        assert!(screen_diff.contains("BOSS_REWARD"), "{screen_diff}");
        assert!(screen_diff.contains("COMBAT_REWARD"), "{screen_diff}");
        assert!(report.unexpected_diffs.iter().all(|diff| {
            diff.action_step != chest_action_step || diff.label != "open boss relic chest"
        }));
    }

    #[test]
    fn observed_combat_potion_offer_is_compared_at_open() {
        let path = crate::corpus_path("permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl");
        let content = std::fs::read_to_string(path).expect("retained trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (potion_action_step, reward_post) = transitions
            .transitions
            .iter()
            .find_map(|(_, action, post)| {
                (action.command.starts_with("POTION USE ")
                    && screen_type(&post.message) == Some("CARD_REWARD")
                    && post.message.pointer("/game_state/combat_state").is_some())
                .then_some((action.step, post.clone()))
            })
            .expect("fixture opens a combat potion card reward");

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == reward_post => Some(state),
                _ => None,
            })
            .expect("combat potion reward post-state remains in imported trace");
        *mutated_state
            .message
            .pointer_mut("/game_state/screen_state/cards/0")
            .expect("first combat potion card offer") =
            json!({"id": "Strike_R", "name": "Strike", "upgrades": 0});

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let offer_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| {
                diff.action_step == potion_action_step && diff.label == "combat potion card reward"
            })
            .and_then(|diff| {
                diff.diffs
                    .iter()
                    .find(|line| line.starts_with("card_reward_ids"))
            })
            .unwrap_or_else(|| {
                panic!(
                    "forged combat potion offer must differ when the offer opens: {:#?}",
                    report.unexpected_diffs
                )
            });
        assert!(
            offer_diff.contains(&STRIKE_R_ID.get().to_string()),
            "{offer_diff}"
        );
        assert!(
            offer_diff.contains(&DEMON_FORM_ID.get().to_string()),
            "{offer_diff}"
        );
    }

    #[test]
    fn rest_card_reward_projection_and_diffs_are_core_owned() {
        let path = crate::corpus_path("permanent_traces/trace-session-8.jsonl");
        let content = std::fs::read_to_string(path).expect("complete trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (rest_action_step, reward_post) = transitions
            .transitions
            .iter()
            .find_map(|(pre, action, post)| {
                (screen_type(&pre.message) == Some("REST")
                    && command_head_eq(&action.command, "CHOOSE")
                    && screen_type(&post.message) == Some("CARD_REWARD"))
                .then_some((action.step, post.clone()))
            })
            .expect("fixture opens a rest-site card reward");

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == reward_post => Some(state),
                _ => None,
            })
            .expect("rest card reward post-state remains in imported trace");
        *mutated_state
            .message
            .pointer_mut("/game_state/screen_type")
            .expect("reward screen type") = json!("REST");
        *mutated_state
            .message
            .pointer_mut("/game_state/screen_state/cards/0")
            .expect("first rest card reward") =
            json!({"id": "Strike_R", "name": "Strike", "upgrades": 0});

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let reward_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| diff.action_step == rest_action_step && diff.label == "rest card reward")
            .expect("forged rest reward must remain an unexpected diff");
        assert!(
            reward_diff
                .diffs
                .iter()
                .any(|diff| diff.starts_with("screen_type:") || diff.contains("card_reward_ids")),
            "{reward_diff:#?}"
        );
    }

    #[test]
    fn observed_grid_destination_cannot_steer_projection() {
        let path = crate::corpus_path("permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl");
        let content = std::fs::read_to_string(path).expect("retained trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (grid_action_step, shop_post) = transitions
            .transitions
            .iter()
            .find_map(|(pre, action, post)| {
                (screen_type(&pre.message) == Some("GRID")
                    && action.command.eq_ignore_ascii_case("CONFIRM")
                    && screen_type(&post.message) == Some("SHOP_SCREEN"))
                .then_some((action.step, post.clone()))
            })
            .expect("fixture confirms a shop purge grid");

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == shop_post => Some(state),
                _ => None,
            })
            .expect("shop grid post-state remains in imported trace");
        *mutated_state
            .message
            .pointer_mut("/game_state/screen_type")
            .expect("shop screen type") = json!("EVENT");

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let destination_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| diff.action_step == grid_action_step && diff.label == "shop grid")
            .expect("forged destination must be compared against the core-owned shop projection");
        assert!(
            destination_diff
                .diffs
                .iter()
                .any(|diff| diff.starts_with("screen_type:")),
            "{destination_diff:#?}"
        );
        assert!(report
            .unexpected_diffs
            .iter()
            .all(|diff| diff.action_step != grid_action_step || diff.label != "event grid"));
    }

    #[test]
    fn observed_shop_post_screen_cannot_choose_purchase_destination() {
        let path = crate::corpus_path("permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl");
        let content = std::fs::read_to_string(path).expect("retained trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (purchase_step, shop_post) = transitions
            .transitions
            .iter()
            .find_map(|(pre, action, post)| {
                (screen_type(&pre.message) == Some("SHOP_SCREEN")
                    && command_head_eq(&action.command, "CHOOSE")
                    && screen_type(&post.message) == Some("SHOP_SCREEN")
                    && trace_deck_len(&pre.message)
                        .zip(trace_deck_len(&post.message))
                        .is_some_and(|(before, after)| after > before))
                .then_some((action.step, post.clone()))
            })
            .expect("fixture buys a card while remaining in the merchant screen");

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == shop_post => Some(state),
                _ => None,
            })
            .expect("shop purchase post-state remains in imported trace");
        *mutated_state
            .message
            .pointer_mut("/game_state/screen_type")
            .expect("shop screen type") = json!("GRID");

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let destination_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| diff.action_step == purchase_step && diff.label == "shop purchase")
            .expect("forged grid must differ from the core-owned merchant destination");
        assert!(
            destination_diff
                .diffs
                .iter()
                .any(|diff| diff.starts_with("screen_type:")),
            "{destination_diff:#?}"
        );
        assert!(report
            .unexpected_diffs
            .iter()
            .all(|diff| { diff.action_step != purchase_step || diff.label != "shop purge grid" }));
    }

    #[test]
    fn observed_boss_relics_are_compared_without_steering_simulation() {
        let path = crate::corpus_path("permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl");
        let content = std::fs::read_to_string(path).expect("retained trace");
        let imported = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&imported.lines).expect("trace transitions");
        let (open_action_step, boss_reward_post) = transitions
            .transitions
            .iter()
            .find_map(|(pre, action, post)| {
                (screen_type(&pre.message) == Some("CHEST")
                    && command_is_choose(&action.command, 0)
                    && screen_type(&post.message) == Some("BOSS_REWARD"))
                .then_some((action.step, post.clone()))
            })
            .expect("fixture opens a boss relic chest");
        let original_choices = boss_reward_post
            .message
            .get("game_state")
            .map(observed_boss_relic_key_choices)
            .expect("observed boss relic choices");
        let forged_key = if original_choices.contains(&RelicKey::CoffeeDripper) {
            RelicKey::Ectoplasm
        } else {
            RelicKey::CoffeeDripper
        };
        let forged_name = relic_key_trace_name(forged_key);

        let mut mutated_lines = imported
            .lines
            .into_iter()
            .filter(|line| !matches!(line, TraceLine::Metadata(_)))
            .collect::<Vec<_>>();
        let mutated_state = mutated_lines
            .iter_mut()
            .find_map(|line| match line {
                TraceLine::State(state) if *state == boss_reward_post => Some(state),
                _ => None,
            })
            .expect("boss reward post-state remains in imported trace");
        let first_relic = mutated_state
            .message
            .pointer_mut("/game_state/screen_state/relics/0")
            .expect("first boss relic");
        *first_relic = json!({"id": forged_name, "name": forged_name});

        let metadata = imported.metadata.expect("trace metadata");
        let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
        let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
        let relic_diff = report
            .unexpected_diffs
            .iter()
            .find(|diff| {
                diff.action_step == open_action_step && diff.label == "open boss relic chest"
            })
            .and_then(|diff| {
                diff.diffs
                    .iter()
                    .find(|line| line.contains("boss_relic_ids"))
            })
            .expect("forged observed boss relic must differ from generated choices");
        assert!(relic_diff.contains(forged_name), "{relic_diff}");
    }

    #[test]
    fn executing_combat_grid_is_treated_as_a_transient_post_state() {
        let transient = json!({
            "game_state": {
                "screen_type": "GRID",
                "action_phase": "EXECUTING_ACTIONS",
                "current_action": "DiscardPileToTopOfDeckAction"
            }
        });
        let stable = json!({
            "game_state": {
                "screen_type": "GRID",
                "action_phase": "WAITING_ON_USER",
                "current_action": null
            }
        });

        assert!(seed_start_is_transient_combat_post_state(&transient));
        assert!(!seed_start_is_transient_combat_post_state(&stable));
    }

    #[test]
    fn start_command_accepts_signed_numeric_seed() {
        let parsed = parse_start_command(&TraceAction {
            step: 1,
            command: "START IRONCLAD 0 -5230933468808623542".to_owned(),
            sent_at: None,
            playtime_seconds: None,
        })
        .expect("start command")
        .expect("valid start command");

        assert_eq!(parsed.numeric_seed, -5_230_933_468_808_623_542);
        assert_eq!(parsed.external_seed, "-5230933468808623542");
    }

    #[test]
    fn dramatic_entrance_maps_from_observed_card_json() {
        let card = json!({"id": "Dramatic Entrance", "name": "Dramatic Entrance"});
        assert_eq!(
            content_id_from_card_value(&card),
            Some(DRAMATIC_ENTRANCE_ID)
        );
    }

    #[test]
    fn colorless_reward_cards_map_from_observed_card_json() {
        use sts_core::content::cards::{DARK_SHACKLES_ID, DISCOVERY_ID};

        for (id, expected, key) in [
            (
                "Dramatic Entrance",
                DRAMATIC_ENTRANCE_ID,
                "Dramatic Entrance",
            ),
            ("Dark Shackles", DARK_SHACKLES_ID, "Dark Shackles"),
            ("Discovery", DISCOVERY_ID, "Discovery"),
        ] {
            let card = json!({"id": id, "name": id});

            assert_eq!(content_id_from_card_value(&card), Some(expected));
            assert_eq!(content_key(expected), key);
        }
    }

    #[test]
    fn dropkick_maps_from_observed_card_json() {
        let card = json!({"id": "Dropkick", "name": "Dropkick"});

        assert_eq!(content_id_from_card_value(&card), Some(DROPKICK_ID));
        assert_eq!(content_key(DROPKICK_ID), "Dropkick");
    }

    #[test]
    fn burn_maps_from_observed_card_json() {
        let card = json!({"id": "Burn", "name": "Burn"});

        assert_eq!(content_id_from_card_value(&card), Some(BURN_ID));
        assert_eq!(content_key(BURN_ID), "Burn");
    }

    #[test]
    fn long_trace_observed_cards_map_from_card_json() {
        use sts_core::content::cards::{
            BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BURNING_PACT_ID, COMBUST_ID, DARK_EMBRACE_ID,
            DAZED_ID, DOUBLE_TAP_ID, FEEL_NO_PAIN_ID, RAGE_ID, REAPER_ID, RUPTURE_ID, WOUND_ID,
        };

        for (id, expected, key) in [
            ("Blood for Blood", BLOOD_FOR_BLOOD_ID, "Blood for Blood"),
            ("Reaper", REAPER_ID, "Reaper"),
            ("Wound", WOUND_ID, "Wound"),
            ("Rupture", RUPTURE_ID, "Rupture"),
            ("Burning Pact", BURNING_PACT_ID, "Burning Pact"),
            ("Combust", COMBUST_ID, "Combust"),
            ("Dazed", DAZED_ID, "Dazed"),
            ("Dark Embrace", DARK_EMBRACE_ID, "Dark Embrace"),
            ("Bludgeon", BLUDGEON_ID, "Bludgeon"),
            ("Double Tap", DOUBLE_TAP_ID, "Double Tap"),
            ("Rage", RAGE_ID, "Rage"),
            ("feelnopain", FEEL_NO_PAIN_ID, "Feel No Pain"),
        ] {
            let card = json!({"id": id, "name": id});

            assert_eq!(content_id_from_card_value(&card), Some(expected));
            assert_eq!(content_key(expected), key);
        }
    }

    #[test]
    fn observed_combat_card_reward_ids_are_canonicalized() {
        use sts_core::content::cards::{FEEL_NO_PAIN_ID, SHOCKWAVE_ID};

        let cards = json!([
            {"id": "Shockwave", "name": "Shockwave", "upgrades": 0},
            {"id": "feelnopain", "name": "Feel No Pain", "upgrades": 0},
            {"id": "unknown-custom-card", "name": "Unknown Custom Card", "upgrades": 0}
        ]);

        assert_eq!(
            card_reward_ids_from_value(Some(&cards)),
            vec![
                json!(SHOCKWAVE_ID.get()),
                json!(FEEL_NO_PAIN_ID.get()),
                json!("unknown-custom-card")
            ]
        );
    }

    #[test]
    fn observed_card_json_maps_every_modeled_card_definition() {
        let mut failures = Vec::new();
        for definition in sts_core::content::cards::ALL_CARDS {
            let card = json!({
                "id": definition.key,
                "name": definition.name,
                "upgrades": 0,
            });

            let mapped = content_id_from_card_value(&card);
            if mapped != Some(definition.id) {
                failures.push(format!(
                    "{} mapped to {:?}, expected {:?}",
                    definition.key, mapped, definition.id
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "missing or mismapped observed card keys:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn observed_reward_import_preserves_every_modeled_card_choice() {
        let cards: Vec<_> = sts_core::content::cards::ALL_CARDS
            .iter()
            .map(|definition| {
                json!({
                    "id": definition.key,
                    "name": definition.name,
                    "upgrades": 0,
                })
            })
            .collect();
        let game = json!({
            "screen_type": "CARD_REWARD",
            "screen_state": {
                "cards": cards,
            }
        });

        let choices = reward_choices_from_observed(&game);
        let ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();
        let expected: Vec<_> = sts_core::content::cards::ALL_CARDS
            .iter()
            .map(|definition| definition.id)
            .collect();

        assert_eq!(ids, expected);
    }

    #[test]
    fn guardian_defend_observed_intent_replays_charge_up_block() {
        let monster = json!({
            "id": "TheGuardian",
            "intent": "DEFEND",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GUARDIAN_ID, 0),
            MonsterIntent::Block {
                block: GUARDIAN_CHARGE_BLOCK
            }
        );
    }

    #[test]
    fn debug_observed_intent_with_damage_imports_attack() {
        use sts_core::content::monsters::JAW_WORM_ID;

        let monster = json!({
            "id": "JawWorm",
            "intent": "DEBUG",
            "move_base_damage": 11
        });

        assert_eq!(
            observed_intent(&monster, JAW_WORM_ID, 0),
            MonsterIntent::Attack { damage: 11 }
        );
    }

    #[test]
    fn mugger_attack_observed_intent_imports_gold_steal_attack() {
        let monster = json!({
            "id": "Mugger",
            "intent": "ATTACK",
            "move_base_damage": 10,
            "move_hits": 1,
            "move_id": 1,
            "powers": [{"id": "Thievery", "name": "Thievery", "amount": 15}]
        });

        assert_eq!(
            observed_intent(&monster, MUGGER_ID, 0),
            MonsterIntent::AttackStealGold {
                damage: 10,
                amount: 15
            }
        );
    }

    #[test]
    fn jaw_worm_defend_buff_observed_intent_imports_bellow() {
        use sts_core::content::monsters::JAW_WORM_ID;

        let monster = json!({
            "id": "JawWorm",
            "intent": "DEFEND_BUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, JAW_WORM_ID, 0),
            MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 6
            }
        );
    }

    #[test]
    fn jaw_worm_attack_defend_observed_intent_imports_thrash() {
        use sts_core::content::monsters::JAW_WORM_ID;

        let monster = json!({
            "id": "JawWorm",
            "intent": "ATTACK_DEFEND",
            "move_base_damage": 7,
            "move_hits": 1,
            "move_id": 3
        });

        assert_eq!(
            observed_intent(&monster, JAW_WORM_ID, 0),
            MonsterIntent::AttackAndBlock {
                damage: 7,
                block: 5
            }
        );
    }

    #[test]
    fn gremlin_leader_defend_buff_observed_intent_imports_encourage() {
        use sts_core::content::monsters::GREMLIN_LEADER_ID;

        let monster = json!({
            "id": "GremlinLeader",
            "intent": "DEFEND_BUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_LEADER_ID, 0),
            MonsterIntent::EncourageGremlins {
                strength: 3,
                block: 6
            }
        );
        assert_eq!(moves_executed_from_observed(&monster, GREMLIN_LEADER_ID), 2);
    }

    #[test]
    fn gremlin_tsundere_defend_observed_intent_imports_source_block() {
        use sts_core::content::monsters::GREMLIN_TSUNDERE_ID;

        let monster = json!({
            "id": "GremlinTsundere",
            "intent": "DEFEND",
            "move_id": 1,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_TSUNDERE_ID, 0),
            MonsterIntent::Block { block: 7 }
        );
    }

    #[test]
    fn gremlin_fat_attack_debuff_observed_intent_imports_weak() {
        use sts_core::content::monsters::{GREMLIN_FAT_ID, SLAVER_BLUE_ID, TASKMASTER_ID};

        let monster = json!({
            "id": "GremlinFat",
            "intent": "ATTACK_DEBUFF",
            "move_id": 2,
            "move_base_damage": 4
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_FAT_ID, 0),
            MonsterIntent::AttackApplyPlayerWeak { damage: 4, weak: 1 }
        );

        let slaver = json!({
            "id": "SlaverBlue",
            "intent": "ATTACK_DEBUFF",
            "move_id": 4,
            "move_base_damage": 7
        });

        assert_eq!(
            observed_intent(&slaver, SLAVER_BLUE_ID, 0),
            MonsterIntent::AttackApplyPlayerWeak { damage: 7, weak: 1 }
        );

        let taskmaster = json!({
            "id": "SlaverBoss",
            "intent": "ATTACK_DEBUFF",
            "move_id": 2,
            "move_base_damage": 7
        });

        assert_eq!(
            observed_intent(&taskmaster, TASKMASTER_ID, 0),
            MonsterIntent::AttackAddWoundsToDiscard {
                damage: 7,
                count: 1
            }
        );
    }

    #[test]
    fn gremlin_nob_buff_observed_intent_imports_bellow() {
        use sts_core::content::monsters::{gremlin_nob_enrage, GREMLIN_NOB_ID};

        let monster = json!({
            "id": "GremlinNob",
            "intent": "BUFF",
            "move_id": 3,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_NOB_ID, 0),
            MonsterIntent::StrengthSelf {
                amount: gremlin_nob_enrage(0)
            }
        );
    }

    #[test]
    fn byrd_buff_observed_intent_imports_caw() {
        use sts_core::content::monsters::BYRD_ID;

        let monster = json!({
            "id": "Byrd",
            "intent": "BUFF",
            "move_id": 6,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, BYRD_ID, 0),
            MonsterIntent::StrengthSelf { amount: 1 }
        );
    }

    #[test]
    fn gremlin_nob_attack_debuff_observed_intent_imports_skull_bash() {
        use sts_core::content::monsters::GREMLIN_NOB_ID;

        let monster = json!({
            "id": "GremlinNob",
            "intent": "ATTACK_DEBUFF",
            "move_id": 2,
            "move_base_damage": 6
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_NOB_ID, 0),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: 6,
                vulnerable: 2
            }
        );
    }

    #[test]
    fn snecko_attack_debuff_observed_intent_imports_tail_whip() {
        use sts_core::content::monsters::SNECKO_ID;

        let monster = json!({
            "id": "Snecko",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 8
        });

        assert_eq!(
            observed_intent(&monster, SNECKO_ID, 0),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: 8,
                vulnerable: 2
            }
        );
        assert_eq!(
            observed_intent(&monster, SNECKO_ID, 17),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: 8,
                weak: 2,
                vulnerable: 2
            }
        );
    }

    #[test]
    fn healer_buff_observed_intent_imports_strength_all() {
        use sts_core::content::monsters::HEALER_ID;

        let monster = json!({
            "id": "Healer",
            "intent": "BUFF",
            "move_id": 3,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, HEALER_ID, 0),
            MonsterIntent::StrengthAllMonsters { amount: 2 }
        );
    }

    #[test]
    fn slime_boss_strong_debuff_observed_intent_imports_sticky() {
        use sts_core::content::monsters::{SLIME_BOSS_ID, SLIME_BOSS_SLIMED_COUNT};

        let monster = json!({
            "id": "SlimeBoss",
            "intent": "STRONG_DEBUFF",
            "move_id": 4,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SLIME_BOSS_ID, 0),
            MonsterIntent::AddSlimedToDiscard {
                count: SLIME_BOSS_SLIMED_COUNT
            }
        );
    }

    #[test]
    fn collector_strong_debuff_observed_intent_imports_mega_debuff() {
        use sts_core::content::monsters::THE_COLLECTOR_ID;

        let monster = json!({
            "id": "TheCollector",
            "intent": "STRONG_DEBUFF",
            "move_id": 4,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, THE_COLLECTOR_ID, 0),
            MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 3,
                weak: 3,
                vulnerable: 3
            }
        );
    }

    #[test]
    fn acid_slime_debug_observed_intent_with_damage_imports_slimed_attack() {
        use sts_core::content::monsters::ACID_SLIME_ID;

        let monster = json!({
            "id": "AcidSlime_L",
            "max_hp": 65,
            "intent": "DEBUG",
            "move_base_damage": 11
        });

        assert_eq!(
            observed_intent(&monster, ACID_SLIME_ID, 0),
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: 11,
                count: 2
            }
        );
    }

    #[test]
    fn medium_acid_slime_debug_move_two_imports_normal_attack() {
        use sts_core::content::monsters::ACID_SLIME_ID;

        let monster = json!({
            "id": "AcidSlime_M",
            "max_hp": 29,
            "intent": "DEBUG",
            "move_id": 2,
            "move_base_damage": 10
        });

        assert_eq!(
            observed_intent(&monster, ACID_SLIME_ID, 0),
            MonsterIntent::Attack { damage: 10 }
        );
    }

    #[test]
    fn large_acid_slime_debuff_observed_intent_imports_two_weak() {
        use sts_core::content::monsters::ACID_SLIME_ID;

        let monster = json!({
            "id": "AcidSlime_L",
            "max_hp": 68,
            "intent": "DEBUFF",
            "move_id": 4,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, ACID_SLIME_ID, 0),
            MonsterIntent::ApplyPlayerWeak { amount: 2 }
        );
    }

    #[test]
    fn medium_spike_slime_debuff_observed_intent_imports_frail() {
        use sts_core::content::monsters::SPIKE_SLIME_ID;

        let monster = json!({
            "id": "SpikeSlime_M",
            "max_hp": 31,
            "intent": "DEBUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SPIKE_SLIME_ID, 0),
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 }
        );
    }

    #[test]
    fn sentry_zero_damage_attack_observed_intent_imports_beam() {
        use sts_core::content::monsters::SENTRY_ID;

        let monster = json!({
            "id": "Sentry",
            "intent": "ATTACK",
            "move_base_damage": 0
        });

        assert_eq!(
            observed_intent(&monster, SENTRY_ID, 0),
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
    }

    #[test]
    fn sentry_debuff_observed_intent_imports_beam() {
        use sts_core::content::monsters::SENTRY_ID;

        let monster = json!({
            "id": "Sentry",
            "intent": "DEBUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SENTRY_ID, 0),
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
    }

    #[test]
    fn repulsor_debuff_observed_intent_imports_draw_dazes() {
        use sts_core::content::monsters::REPULSOR_ID;

        let monster = json!({
            "id": "Repulsor",
            "intent": "DEBUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, REPULSOR_ID, 0),
            MonsterIntent::AddDazedToDraw { count: 2 }
        );
    }

    #[test]
    fn observed_event_screen_imports_lab() {
        let game = json!({
            "screen_type": "EVENT",
            "screen_state": {
                "event_id": "Lab",
                "event_name": "Lab"
            }
        });

        assert_eq!(
            observed_event_screen(&game, 0).map(|screen| screen.event),
            Some(Event::Lab)
        );
    }

    #[test]
    fn grid_trace_choice_label_does_not_preview_upgrade_existing_cards() {
        use sts_core::content::cards::{RITUAL_DAGGER_ID, TRUE_GRIT_ID, TRUE_GRIT_PLUS_ID};

        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::ToxicEgg);

        assert_eq!(
            grid_trace_choice_label(&run, &CardInstance::new(CardId::new(1), TRUE_GRIT_ID)),
            "true grit"
        );
        assert_eq!(
            grid_trace_choice_label(&run, &CardInstance::new(CardId::new(2), TRUE_GRIT_PLUS_ID)),
            "true grit+"
        );
        let mut ritual_dagger = CardInstance::new(CardId::new(3), RITUAL_DAGGER_ID);
        ritual_dagger.upgrades = 1;
        assert_eq!(
            grid_trace_choice_label(&run, &ritual_dagger),
            "ritual dagger+"
        );
    }

    #[test]
    fn seed_start_event_grid_requires_explicit_confirm_after_final_selection() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(EventScreen {
            event: Event::DrugDealer,
            choices: vec![
                EventChoice {
                    label: "Test J.A.X.".to_owned(),
                },
                EventChoice {
                    label: "Become test subject".to_owned(),
                },
                EventChoice {
                    label: "Ingest mutagens".to_owned(),
                },
            ],
            stage: 0,
            event_data: 0,
        });
        let original_deck_len = run.deck.len();
        let opened = apply_event_action(&run, EventAction::Choose { choice_index: 1 })
            .expect("Drug Dealer transform grid opens");
        let after_first =
            seed_start_apply_grid_command(&opened, "CHOOSE 0").expect("first source is selected");
        assert!(after_first.card_grid.is_some());

        let after_second = seed_start_apply_grid_command(&after_first, "CHOOSE 1")
            .expect("second source is selected");
        assert!(after_second.card_grid.is_some());

        let completed = seed_start_apply_grid_command(&after_second, "CONFIRM")
            .expect("explicit confirmation resolves the grid");
        assert!(completed.card_grid.is_none());
        assert_eq!(completed.deck.len(), original_deck_len);
        assert_eq!(completed.phase, RunPhase::Event);
        assert_eq!(
            completed
                .event
                .as_ref()
                .expect("Drug Dealer leave screen")
                .choices[0]
                .label,
            "Leave"
        );

        let action_frame =
            seed_start_event_simulated_subset_with_delayed_deck_append(&completed, &[], Some(2));
        assert_eq!(
            action_frame["deck_ids"]
                .as_array()
                .expect("projected deck")
                .len(),
            original_deck_len - 2
        );
        assert_eq!(action_frame["choices"], json!(["leave"]));
    }

    #[test]
    fn seed_start_completed_event_reward_waits_for_proceed() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Reward;
        run.event = Some(EventScreen {
            event: Event::CursedTome,
            choices: vec![EventChoice {
                label: "Leave".to_owned(),
            }],
            stage: 5,
            event_data: 0,
        });
        run.reward = Some(RewardScreen {
            continuation: sts_core::RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });

        assert!(seed_start_reward_sequence_complete(&run));
        assert_eq!(
            seed_start_phase_after_reward_completion(&run),
            SeedStartPhase::Reward
        );
    }

    #[test]
    fn seed_start_map_return_projects_final_row_as_boss_choice() {
        let seed = sts_seed_string_to_long("TEST");
        let mut path = vec![generate_exordium_map_topology(seed).first_row_choices[0]];
        while path.len() < 15 {
            let next = generate_exordium_map_choices_after_path(seed, &path)
                .last()
                .and_then(|step| step.next_choices.first())
                .copied()
                .expect("path reaches the next map row");
            path.push(next);
        }

        let projected = seed_start_simulated_map_return(seed, &path, None, &[], &[], &[]);

        assert_eq!(projected["choices"], json!(["boss"]));
        assert_eq!(projected["next_nodes"], json!([]));
        assert_eq!(projected["current_node"]["y"], json!(14));
    }

    #[test]
    fn seed_start_map_return_falls_back_when_carried_map_has_no_reachable_nodes() {
        let seed = sts_seed_string_to_long("3WUU08ZMEVMV2");
        let mut run = seed_start_seeded_idle_run(seed, 0, &ironclad_starter_deck_keys());
        run.relics.push(Relic::WingBoots);
        run.wing_boots_charges = 3;
        let first_node = legal_map_actions_on_run(&run)
            .into_iter()
            .find(|action| match action {
                sts_core::MapAction::ChooseNode { node_id } => {
                    seed_start_map_node_xy(*node_id).0 == 2
                }
            })
            .expect("seed offers x=2 as a first-row node");
        run = apply_map_action_on_run(&run, first_node).expect("first map node is legal");
        run.phase = RunPhase::Reward;

        let projected = seed_start_simulated_map_return(seed, &[2], Some(&run), &[], &[], &[]);

        assert_eq!(
            projected["choices"],
            json!(["x=2", "x=3", "x=4", "x=5", "x=6"])
        );
        assert_eq!(projected["current_node"]["x"], json!(2));
        assert_eq!(projected["current_node"]["y"], json!(0));
    }

    #[test]
    fn seed_start_post_boss_transition_projects_current_node_sentinel() {
        let mut projected = json!({
            "screen_type": "MAP",
            "first_node_chosen": false,
            "current_node": {
                "symbol": "",
                "x": 0,
                "y": -1,
            },
            "next_nodes": [],
        });

        seed_start_project_post_boss_transition_current_node(&mut projected);

        assert_eq!(
            projected["current_node"],
            json!({
                "symbol": "",
                "x": -1,
                "y": 15,
            })
        );
    }

    #[test]
    fn seed_start_event_choice_labels_strip_effect_parentheticals() {
        assert_eq!(
            seed_start_visible_event_choice_label("Gather gold (gain 75 gold, lose 11 HP)"),
            Some("gather gold".to_owned())
        );
        assert_eq!(
            seed_start_visible_event_choice_label("Leave it (lose 48 gold)"),
            Some("leave it".to_owned())
        );
        assert_eq!(
            seed_start_visible_event_choice_label("Prize?"),
            seed_start_visible_event_choice_label("Prize!")
        );
    }

    #[test]
    fn seed_start_mushrooms_event_uses_communication_mod_identity_and_labels() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_floor = 8;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::HypnotizingColoredMushrooms));

        let subset = seed_start_event_simulated_subset(&run, &[]);

        assert_eq!(subset["event_id"], "mushrooms");
        assert_eq!(subset["choices"], json!(["stomp", "eat"]));
    }

    #[test]
    fn observed_event_screen_imports_scrap_ooze_deeper_progress() {
        let game = json!({
            "ascension_level": 0,
            "screen_type": "EVENT",
            "choice_list": ["deeper", "leave"],
            "screen_state": {
                "event_id": "Scrap Ooze",
                "event_name": "Scrap Ooze",
                "options": [
                    {"text": "[Deeper] Lose 5 HP. 45%: Find a Relic."},
                    {"text": "[Leave]"}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("scrap ooze screen");

        assert_eq!(screen.event, Event::ScrapOoze);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.event_data, 2);
        assert_eq!(screen.choices[0].label, "deeper");
    }

    #[test]
    fn observed_event_screen_imports_shining_light_from_visible_choices_without_event_id() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["enter", "leave"],
            "screen_state": {}
        });

        let screen = observed_event_screen(&game, 0).expect("shining light screen");

        assert_eq!(screen.event, Event::ShiningLight);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "enter");
    }

    #[test]
    fn seed_start_event_choice_index_matches_observed_shining_light_leave_label() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ShiningLight));
        let pre_message = json!({
            "game_state": {
                "screen_type": "MAP",
                "choice_list": ["enter", "leave"],
                "screen_state": {}
            }
        });

        assert_eq!(
            seed_start_event_choice_index_for_communication_mod(&run, 1, &pre_message),
            Some(1)
        );
    }

    #[test]
    fn observed_event_screen_imports_wing_statue() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["pray", "leave"],
            "screen_state": {
                "event_id": "Golden Wing",
                "event_name": "Wing Statue",
                "options": [
                    {"text": "[Pray] Remove a card from your deck. Lose 7 HP."},
                    {"text": "[Locked] Requires: Card with 10 or more damage."},
                    {"text": "[Leave]"}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("wing statue screen");

        assert_eq!(screen.event, Event::WingStatue);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "pray");
    }

    #[test]
    fn observed_event_screen_imports_wing_statue_from_visible_choices_without_event_id() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["pray", "leave"],
            "screen_state": {}
        });

        let screen = observed_event_screen(&game, 0).expect("wing statue screen");

        assert_eq!(screen.event, Event::WingStatue);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "pray");
    }

    #[test]
    fn observed_event_screen_imports_transmogrifier_from_event_id() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["pray", "leave"],
            "screen_state": {
                "event_id": "Transmorgrifier",
                "event_name": "Transmogrifier"
            }
        });

        let screen = observed_event_screen(&game, 0).expect("transmogrifier screen");

        assert_eq!(screen.event, Event::Transmorgrifier);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "pray");
    }

    #[test]
    fn observed_event_screen_imports_nest_choice_stage() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["smash and grab", "stay in line"],
            "screen_state": {
                "event_id": "Nest",
                "event_name": "The Nest",
                "options": [
                    {"text": "[Smash and Grab] Obtain 99 Gold."},
                    {"text": "[Stay in Line] Obtain Ritual Dagger. Lose 6 HP."}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("nest screen");

        assert_eq!(screen.event, Event::Nest);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.choices[0].label, "smash and grab");
    }

    #[test]
    fn observed_event_screen_imports_knowing_skull_option_stage() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["a pick me up?", "riches?", "success?", "how do i leave?"],
            "screen_state": {
                "event_id": "Knowing Skull",
                "event_name": "Knowing Skull",
                "options": [
                    {"label": "A Pick Me Up?", "text": "[A Pick Me Up?] Get a Potion. Lose 6 HP."},
                    {"label": "Riches?", "text": "[Riches?] Gain 90 Gold. Lose 6 HP."},
                    {"label": "Success?", "text": "[Success?] Get a Colorless Card. Lose 6 HP."},
                    {"label": "How do I leave?", "text": "[How do I leave?] Lose 6 HP."}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("knowing skull screen");

        assert_eq!(screen.event, Event::KnowingSkull);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.choices[3].label, "how do i leave?");
        assert_eq!(screen.event_data, default_knowing_skull_costs());
    }

    #[test]
    fn spiker_buff_observed_intent_imports_non_damage_buff() {
        use sts_core::content::monsters::SPIKER_ID;

        let monster = json!({
            "id": "Spiker",
            "intent": "BUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SPIKER_ID, 0),
            MonsterIntent::StrengthAndBlock {
                strength: 0,
                block: 0
            }
        );
    }

    #[test]
    fn sentry_debug_without_damage_observed_intent_imports_beam() {
        use sts_core::content::monsters::SENTRY_ID;

        let monster = json!({
            "id": "Sentry",
            "intent": "DEBUG",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SENTRY_ID, 0),
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
    }

    #[test]
    fn observed_attack_with_multiple_hits_imports_attack_multiple() {
        use sts_core::content::monsters::HEXAGHOST_ID;

        let monster = json!({
            "id": "Hexaghost",
            "intent": "ATTACK",
            "move_base_damage": 5,
            "move_hits": 6
        });

        assert_eq!(
            observed_intent(&monster, HEXAGHOST_ID, 0),
            MonsterIntent::AttackMultiple { damage: 5, hits: 6 }
        );
    }

    #[test]
    fn hexaghost_attack_debuff_imports_observed_sear() {
        use sts_core::content::monsters::HEXAGHOST_ID;

        let monster = json!({
            "id": "Hexaghost",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 6,
            "move_hits": 1,
            "move_id": 4
        });

        assert_eq!(
            observed_intent(&monster, HEXAGHOST_ID, 0),
            MonsterIntent::AddBurnToDiscard {
                damage: 6,
                count: 1
            }
        );
    }

    #[test]
    fn hexaghost_attack_debuff_imports_observed_inferno() {
        use sts_core::content::monsters::HEXAGHOST_ID;

        let monster = json!({
            "id": "Hexaghost",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 2,
            "move_hits": 6,
            "move_id": 6
        });

        assert_eq!(
            observed_intent(&monster, HEXAGHOST_ID, 0),
            MonsterIntent::AttackMultipleUpgradeBurns {
                damage: 2,
                hits: 6,
                count: 3
            }
        );
    }

    #[test]
    fn orb_walker_attack_debuff_imports_burn_discard_intent() {
        use sts_core::content::monsters::ORB_WALKER_ID;

        let monster = json!({
            "id": "Orb Walker",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 10,
            "move_hits": 1
        });

        assert_eq!(
            observed_intent(&monster, ORB_WALKER_ID, 0),
            MonsterIntent::AddBurnToDiscardAndDraw {
                damage: 10,
                count: 1
            }
        );
    }

    #[test]
    fn shelled_parasite_attack_buff_imports_life_suck() {
        use sts_core::content::monsters::SHELLED_PARASITE_ID;

        let monster = json!({
            "id": "Shelled Parasite",
            "intent": "ATTACK_BUFF",
            "move_base_damage": 10
        });

        assert_eq!(
            observed_intent(&monster, SHELLED_PARASITE_ID, 0),
            MonsterIntent::AttackHealSelf { damage: 10 }
        );
    }

    #[test]
    fn guardian_twin_slam_attack_buff_imports_two_hit_attack() {
        use sts_core::content::monsters::GUARDIAN_ID;

        let monster = json!({
            "id": "TheGuardian",
            "intent": "ATTACK_BUFF",
            "move_id": 4,
            "move_base_damage": 8,
            "move_hits": 2
        });

        assert_eq!(
            observed_intent(&monster, GUARDIAN_ID, 0),
            MonsterIntent::AttackMultiple { damage: 8, hits: 2 }
        );
    }

    #[test]
    fn observed_monster_weakened_imports_weak_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Weakened", "name": "Weakened", "amount": 2}
        ])));

        assert_eq!(powers.weak, 2);
    }

    #[test]
    fn observed_monster_plated_armor_imports_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Plated Armor", "name": "Plated Armor", "amount": 13}
        ])));

        assert_eq!(powers.plated_armor, 13);
    }

    #[test]
    fn observed_monster_spore_cloud_imports_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Spore Cloud", "name": "Spore Cloud", "amount": 2}
        ])));

        assert_eq!(powers.spore_cloud, 2);
    }

    #[test]
    fn observed_monster_strength_up_imports_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Generic Strength Up Power", "name": "Strength Up", "amount": 3}
        ])));

        assert_eq!(powers.strength_up, 3);
    }

    #[test]
    fn observed_player_combust_imports_damage_amount() {
        let (powers, temp_strength) = player_powers_and_temp_strength(Some(&json!([
            {"id": "Strength", "name": "Strength", "amount": 1},
            {"id": "Combust", "name": "Combust", "amount": 7},
            {"id": "Dark Embrace", "name": "Dark Embrace", "amount": 1},
            {"id": "Rupture", "name": "Rupture", "amount": 2},
            {"id": "Hex", "name": "Hex", "amount": 1}
        ])));

        assert_eq!(temp_strength, 0);
        assert_eq!(powers.strength, 1);
        assert_eq!(powers.combust, 1);
        assert_eq!(powers.combust_damage, 7);
        assert_eq!(powers.dark_embrace, 1);
        assert_eq!(powers.rupture, 2);
        assert_eq!(powers.hex, 1);
    }

    #[test]
    fn book_of_stabbing_observed_hits_reconstruct_move_index() {
        use sts_core::content::monsters::BOOK_OF_STABBING_ID;

        let monster = json!({
            "id": "BookOfStabbing",
            "intent": "ATTACK",
            "move_base_damage": 6,
            "move_hits": 4
        });

        assert_eq!(
            moves_executed_from_observed(&monster, BOOK_OF_STABBING_ID),
            3
        );
    }

    #[test]
    fn bronze_automaton_observed_stun_reconstructs_move_index() {
        use sts_core::content::monsters::BRONZE_AUTOMATON_ID;

        let monster = json!({
            "id": "BronzeAutomaton",
            "intent": "STUN",
            "move_base_damage": -1,
            "move_hits": -1
        });

        assert_eq!(
            moves_executed_from_observed(&monster, BRONZE_AUTOMATON_ID),
            6
        );
        assert_eq!(
            observed_intent(&monster, BRONZE_AUTOMATON_ID, 0),
            MonsterIntent::Stun
        );
    }

    #[test]
    fn gremlin_leader_unknown_move_two_imports_summon() {
        use sts_core::content::monsters::GREMLIN_LEADER_ID;

        let monster = json!({
            "id": "GremlinLeader",
            "intent": "UNKNOWN",
            "move_id": 2,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_LEADER_ID, 0),
            MonsterIntent::SummonGremlins { count: 2 }
        );
    }

    #[test]
    fn bronze_automaton_and_orb_ids_import_without_cultist_fallback() {
        use sts_core::content::monsters::{
            content_id_from_game_monster_id, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, ORB_WALKER_ID,
        };

        assert_eq!(
            content_id_from_game_monster_id("BronzeAutomaton"),
            BRONZE_AUTOMATON_ID
        );
        assert_eq!(content_id_from_game_monster_id("BronzeOrb"), BRONZE_ORB_ID);
        assert_eq!(content_id_from_game_monster_id("Orb Walker"), ORB_WALKER_ID);
    }

    #[test]
    fn neow_generated_identity_display_names_are_mapped() {
        use sts_core::content::cards::{
            ARMAMENTS_ID, CHRYSALIS_ID, DECAY_ID, DOUBT_ID, FEED_ID, HAND_OF_GREED_ID,
            IMPERVIOUS_ID, LIMIT_BREAK_ID, MAGNETISM_ID, MAYHEM_ID, PARASITE_ID, SECRET_WEAPON_ID,
            TRANSMUTATION_ID, WRITHE_ID,
        };

        for (content_id, expected) in [
            (LIMIT_BREAK_ID, "Limit Break"),
            (IMPERVIOUS_ID, "Impervious"),
            (FEED_ID, "Feed"),
            (MAYHEM_ID, "Mayhem"),
            (SECRET_WEAPON_ID, "Secret Weapon"),
            (TRANSMUTATION_ID, "Transmutation"),
            (MAGNETISM_ID, "Magnetism"),
            (CHRYSALIS_ID, "Chrysalis"),
            (HAND_OF_GREED_ID, "Hand Of Greed"),
            (PARASITE_ID, "Parasite"),
            (DECAY_ID, "Decay"),
            (WRITHE_ID, "Writhe"),
            (DOUBT_ID, "Doubt"),
            (ARMAMENTS_ID, "Armaments"),
        ] {
            assert_eq!(content_key(content_id), expected);
            assert_ne!(content_key(content_id), "unknown");
        }
    }

    #[test]
    fn deck_projection_uses_communication_mod_card_ids() {
        use sts_core::content::cards::{
            HAND_OF_GREED_ID, HAND_OF_GREED_PLUS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID,
        };

        for (content_id, expected) in [
            (STRIKE_R_ID, "Strike_R"),
            (STRIKE_R_PLUS_ID, "Strike_R"),
            (HAND_OF_GREED_ID, "HandOfGreed"),
            (HAND_OF_GREED_PLUS_ID, "HandOfGreed"),
        ] {
            assert_eq!(deck_content_key(content_id), expected);
        }
    }

    #[test]
    fn trace_relic_display_names_are_mapped() {
        for (key, name) in [
            (RelicKey::Akabeko, "Akabeko"),
            (RelicKey::Anchor, "Anchor"),
            (RelicKey::TheBoot, "The Boot"),
            (RelicKey::BagOfMarbles, "Bag of Marbles"),
            (RelicKey::BagOfPreparation, "Bag of Preparation"),
            (RelicKey::BloodVial, "Blood Vial"),
            (RelicKey::CharonsAshes, "Charon's Ashes"),
            (RelicKey::DeadBranch, "Dead Branch"),
            (RelicKey::GamblingChip, "Gambling Chip"),
            (RelicKey::Torii, "Torii"),
            (RelicKey::GremlinHorn, "Gremlin Horn"),
            (RelicKey::MawBank, "Maw Bank"),
            (RelicKey::IceCream, "Ice Cream"),
            (RelicKey::CentennialPuzzle, "Centennial Puzzle"),
            (RelicKey::OddlySmoothStone, "Oddly Smooth Stone"),
            (RelicKey::EternalFeather, "Eternal Feather"),
            (RelicKey::Omamori, "Omamori"),
            (RelicKey::AncientTeaSet, "Ancient Tea Set"),
            (RelicKey::Pear, "Pear"),
            (RelicKey::MeatOnTheBone, "Meat on the Bone"),
            (RelicKey::Ginger, "Ginger"),
            (RelicKey::Strawberry, "Strawberry"),
            (RelicKey::TungstenRod, "Tungsten Rod"),
            (RelicKey::MagicFlower, "Magic Flower"),
            (RelicKey::BirdFacedUrn, "Bird-Faced Urn"),
            (RelicKey::UnceasingTop, "Unceasing Top"),
            (RelicKey::Toolbox, "Toolbox"),
            (RelicKey::PotionBelt, "Potion Belt"),
            (RelicKey::Pantograph, "Pantograph"),
            (RelicKey::ChampionBelt, "Champion Belt"),
            (RelicKey::GoldenIdol, "Golden Idol"),
            (RelicKey::DuVuDoll, "Du-Vu Doll"),
            (RelicKey::MedicalKit, "Medical Kit"),
            (RelicKey::WarPaint, "War Paint"),
            (RelicKey::LetterOpener, "Letter Opener"),
            (RelicKey::CaptainsWheel, "Captain's Wheel"),
            (RelicKey::LizardTail, "Lizard Tail"),
            (RelicKey::SlingOfCourage, "Sling of Courage"),
            (RelicKey::CultistMask, "Cultist Headpiece"),
            (RelicKey::Brimstone, "Brimstone"),
            (RelicKey::Nunchaku, "Nunchaku"),
            (RelicKey::InkBottle, "Ink Bottle"),
            (RelicKey::Shuriken, "Shuriken"),
            (RelicKey::Kunai, "Kunai"),
            (RelicKey::HappyFlower, "Happy Flower"),
            (RelicKey::IncenseBurner, "Incense Burner"),
            (RelicKey::ThreadAndNeedle, "Thread and Needle"),
            (RelicKey::FossilizedHelix, "Fossilized Helix"),
            (RelicKey::PeacePipe, "Peace Pipe"),
            (RelicKey::ClockworkSouvenir, "Clockwork Souvenir"),
            (RelicKey::ChemicalX, "Chemical X"),
            (RelicKey::Calipers, "Calipers"),
            (RelicKey::QuestionCard, "Question Card"),
            (RelicKey::MoltenEgg, "Molten Egg"),
            (RelicKey::PaperPhrog, "Paper Phrog"),
            (RelicKey::StrangeSpoon, "Strange Spoon"),
            (RelicKey::DollysMirror, "Dolly's Mirror"),
            (RelicKey::SelfFormingClay, "Self-Forming Clay"),
            (RelicKey::BlueCandle, "Blue Candle"),
            (RelicKey::BottledLightning, "Bottled Lightning"),
            (RelicKey::Girya, "Girya"),
            (RelicKey::SsserpentHead, "Ssserpent Head"),
        ] {
            assert_eq!(relic_key_trace_name(key), name);
            assert_eq!(relic_key_from_trace_name(name), Some(key));
            assert_eq!(
                relic_from_trace_name(name).map(|relic| relic.key()),
                Some(key)
            );
        }
    }

    #[test]
    fn every_ironclad_relic_pool_entry_has_a_round_trip_trace_name() {
        use sts_core::relic::{
            IRONCLAD_BOSS_RELIC_POOL, IRONCLAD_COMMON_RELIC_POOL, IRONCLAD_RARE_RELIC_POOL,
            IRONCLAD_SHOP_RELIC_POOL, IRONCLAD_UNCOMMON_RELIC_POOL,
        };

        for key in IRONCLAD_COMMON_RELIC_POOL
            .into_iter()
            .chain(IRONCLAD_UNCOMMON_RELIC_POOL)
            .chain(IRONCLAD_RARE_RELIC_POOL)
            .chain(IRONCLAD_SHOP_RELIC_POOL)
            .chain(IRONCLAD_BOSS_RELIC_POOL)
        {
            let name = relic_key_trace_name(key);
            assert_ne!(name, "Unknown Relic", "{key:?}");
            assert_eq!(relic_key_from_trace_name(name), Some(key), "{name}");
        }
    }

    #[test]
    fn combat_subset_reports_start_of_combat_relic_healed_hp() {
        let mut run = RunState::map_fixture();
        run.player_hp = 34;
        run.player_max_hp = 86;
        run.relics = vec![Relic::BurningBlood, Relic::BloodVial];
        run.combat = Some(run.init_combat(CombatState::initial_fixture()));

        let subset = seed_start_simulated_combat_subset(&run, false);

        assert_eq!(subset["current_hp"], json!(36));
        assert_eq!(subset["combat_player_hp"], json!(36));
    }

    #[test]
    fn combat_subset_uses_run_floor() {
        let mut run = RunState::map_fixture();
        run.current_floor = 17;
        run.combat = Some(run.init_combat(CombatState::initial_fixture()));

        let subset = seed_start_simulated_combat_subset(&run, false);

        assert_eq!(subset["floor"], json!(17));
    }

    #[test]
    fn combat_subset_uses_simulated_monster_identity_and_max_hp() {
        let mut run = RunState::map_fixture();
        let mut combat = run.init_combat(CombatState::initial_fixture());
        combat.monsters[0].hp = 31;
        combat.monsters[0].max_hp = 47;
        run.combat = Some(combat);
        let subset = seed_start_simulated_combat_subset(&run, false);

        assert_eq!(subset["monsters"][0]["current_hp"], json!(31));
        assert_eq!(subset["monsters"][0]["max_hp"], json!(47));
        assert_eq!(subset["monsters"][0]["name"], json!("Fixed Simple Monster"));
    }

    #[test]
    fn simulated_monster_identity_uses_target_display_names_and_slime_sizes() {
        use sts_core::combat::SlimeSize;
        use sts_core::content::monsters::{
            ACID_SLIME_ID, BRONZE_ORB_ID, GREEN_LOUSE_ID, GREMLIN_FAT_ID, GREMLIN_THIEF_ID,
            GREMLIN_TSUNDERE_ID, GREMLIN_WARRIOR_ID, GUARDIAN_ID, RED_LOUSE_ID, SLAVER_BLUE_ID,
            SLAVER_RED_ID, SPIKE_SLIME_ID,
        };

        let mut monster = CombatState::initial_fixture().monsters.remove(0);
        for (content_id, max_hp, expected) in [
            (GREEN_LOUSE_ID, 12, "Louse"),
            (RED_LOUSE_ID, 11, "Louse"),
            (SLAVER_BLUE_ID, 50, "Slaver"),
            (SLAVER_RED_ID, 50, "Slaver"),
            (GREMLIN_WARRIOR_ID, 22, "Mad Gremlin"),
            (GREMLIN_THIEF_ID, 12, "Sneaky Gremlin"),
            (GREMLIN_FAT_ID, 16, "Fat Gremlin"),
            (GREMLIN_TSUNDERE_ID, 14, "Shield Gremlin"),
            (BRONZE_ORB_ID, 54, "Orb"),
            (GUARDIAN_ID, 240, "The Guardian"),
            (SPIKE_SLIME_ID, 13, "Spike Slime (S)"),
            (SPIKE_SLIME_ID, 30, "Spike Slime (M)"),
            (SPIKE_SLIME_ID, 68, "Spike Slime (L)"),
            (ACID_SLIME_ID, 10, "Acid Slime (S)"),
            (ACID_SLIME_ID, 29, "Acid Slime (M)"),
            (ACID_SLIME_ID, 67, "Acid Slime (L)"),
        ] {
            monster.content_id = content_id;
            monster.max_hp = max_hp;
            assert_eq!(seed_start_trace_monster_name(&monster), expected);
        }

        monster.content_id = SPIKE_SLIME_ID;
        monster.max_hp = 9;
        monster.slime_size = Some(SlimeSize::Medium);
        assert_eq!(seed_start_trace_monster_name(&monster), "Spike Slime (M)");

        monster.content_id = ACID_SLIME_ID;
        monster.max_hp = 9;
        monster.slime_size = Some(SlimeSize::Medium);
        assert_eq!(seed_start_trace_monster_name(&monster), "Acid Slime (M)");
    }

    #[test]
    fn simulated_combat_screen_type_comes_from_typed_decision_state() {
        use sts_core::combat::{
            DiscardSelectPurpose, DiscardSelectState, DrawSelectPurpose, DrawSelectState,
            ExhaustSelectState,
        };

        let source_card_id = CardId::new(100);
        let mut combat = CombatState::initial_fixture();
        assert_eq!(seed_start_simulated_combat_screen_type(&combat), "NONE");

        combat.phase = CombatPhase::Lost;
        assert_eq!(
            seed_start_simulated_combat_screen_type(&combat),
            "GAME_OVER"
        );
        combat.phase = CombatPhase::WaitingForPlayer;

        combat.toolbox_card_reward = Some(vec![CardInstance::new(source_card_id, STRIKE_R_ID)]);
        assert_eq!(
            seed_start_simulated_combat_screen_type(&combat),
            "CARD_REWARD"
        );
        let mut toolbox_run = RunState::map_fixture();
        toolbox_run.combat = Some(combat.clone());
        let toolbox_subset = seed_start_simulated_combat_subset(&toolbox_run, false);
        assert_eq!(toolbox_subset["screen_type"], json!("CARD_REWARD"));
        assert_eq!(
            toolbox_subset["card_reward_ids"],
            json!([STRIKE_R_ID.get()])
        );
        combat.toolbox_card_reward = None;

        combat.exhaust_select = Some(ExhaustSelectState {
            purpose: ExhaustSelectPurpose::BurningPactDraw2,
            source_card_id: Some(source_card_id),
            source_card: None,
            selected_hand_indices: Vec::new(),
        });
        assert_eq!(
            seed_start_simulated_combat_screen_type(&combat),
            "HAND_SELECT"
        );
        combat.exhaust_select.as_mut().unwrap().purpose = ExhaustSelectPurpose::ExhumeReturnToHand;
        assert_eq!(seed_start_simulated_combat_screen_type(&combat), "GRID");
        combat.exhaust_select = None;

        combat.draw_select = Some(DrawSelectState {
            purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
            source_card_id,
            selected_draw_index: None,
        });
        assert_eq!(seed_start_simulated_combat_screen_type(&combat), "GRID");
        combat.draw_select = None;

        combat.discard_select = Some(DiscardSelectState {
            purpose: DiscardSelectPurpose::HeadbuttPutOnDraw,
            source_card_id: Some(source_card_id),
            source_card: None,
            selected_discard_indices: Vec::new(),
            max_choices: 1,
            selected_discard_index: None,
        });
        assert_eq!(seed_start_simulated_combat_screen_type(&combat), "GRID");

        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.combat = Some(combat.clone());
        assert_eq!(
            seed_start_active_combat_decision(&run).expect("one decision"),
            Some(SeedStartCombatDecision::DiscardSelect)
        );

        let (draw_choice, draw_label) = seed_start_bind_combat_decision_command(
            SeedStartCombatDecision::DrawSelect,
            "CHOOSE 2",
        )
        .expect("draw selection binds");
        assert!(matches!(
            draw_choice,
            RunAction::ChooseDrawSelect { index: 2 }
        ));
        assert_eq!(draw_label, "draw select");
        assert!(matches!(
            seed_start_bind_combat_decision_command(SeedStartCombatDecision::DrawSelect, "CONFIRM"),
            Ok((RunAction::ConfirmDrawSelect, "draw select confirm"))
        ));

        run.combat.as_mut().expect("combat").draw_select = Some(DrawSelectState {
            purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
            source_card_id,
            selected_draw_index: None,
        });
        let error = seed_start_active_combat_decision(&run)
            .expect_err("multiple decisions must fail closed");
        assert!(error.contains("DrawSelect"), "{error}");
        assert!(error.contains("DiscardSelect"), "{error}");
    }

    #[test]
    fn encounter_observation_projects_visible_toolbox_cards() {
        use sts_core::content::cards::{BASH_ID, DEFEND_R_ID};

        let message = json!({
            "game_state": {
                "screen_type": "CARD_REWARD",
                "screen_state": {
                    "cards": [
                        {"id": "Strike_R"},
                        {"id": "Defend_R"},
                        {"id": "Bash"}
                    ]
                }
            }
        });

        let observed = seed_start_encounter_observed_subset(&message);

        assert_eq!(
            observed["card_reward_ids"],
            json!([STRIKE_R_ID.get(), DEFEND_R_ID.get(), BASH_ID.get()])
        );
    }

    #[test]
    fn combat_subset_reports_discovery_card_reward_choices() {
        use sts_core::{
            card::CardInstance,
            content::cards::{PUMMEL_ID, SEARING_BLOW_ID, SHRUG_IT_OFF_ID},
            ids::CardId,
        };

        let mut run = RunState::map_fixture();
        let mut combat = run.init_combat(CombatState::initial_fixture());
        combat.discovery_card_reward = Some(vec![
            CardInstance::new(CardId::new(100), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(101), PUMMEL_ID),
            CardInstance::new(CardId::new(102), SEARING_BLOW_ID),
        ]);
        run.combat = Some(combat);

        let subset = seed_start_simulated_combat_subset(&run, false);

        assert_eq!(subset["screen_type"], json!("CARD_REWARD"));
        assert_eq!(
            subset["card_reward_ids"],
            json!([
                SHRUG_IT_OFF_ID.get(),
                PUMMEL_ID.get(),
                SEARING_BLOW_ID.get()
            ])
        );
    }

    #[test]
    fn combat_subset_reports_simulated_escape_instead_of_panicking() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Idle;
        run.combat = None;
        run.player_hp = 70;
        let message = json!({
            "game_state": {
                "floor": 44,
                "screen_type": "NONE",
                "gold": run.gold,
                "current_hp": 70,
                "max_hp": run.player_max_hp,
                "potions": [],
                "combat_state": {
                    "player": {"current_hp": 70, "block": 0, "energy": 3},
                    "hand": [],
                    "draw_pile": [],
                    "discard_pile": [],
                    "monsters": [{
                        "id": "GiantHead",
                        "name": "Giant Head",
                        "current_hp": 300,
                        "max_hp": 500,
                        "block": 0,
                        "intent": "ATTACK",
                        "powers": [],
                    }],
                },
            }
        });

        let observed = seed_start_combat_observed_subset(&message);
        let simulated = seed_start_simulated_combat_subset(&run, false);

        assert_eq!(simulated["screen_type"], json!("NO_COMBAT"));
        assert!(!subset_diffs(observed, simulated).is_empty());
    }

    #[test]
    fn seed_start_shop_choice_list_keeps_visible_potions_when_belt_is_full() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Shop;
        run.shop_merchant_open = true;
        run.gold = 100;
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Weak];
        run.shop = Some(sts_core::ShopScreen {
            cards: Vec::new(),
            relics: Vec::new(),
            potions: vec![sts_core::ShopPotionSlot {
                potion: Potion::Power,
                price: 50,
                sold: false,
            }],
            remove_cost: 75,
            remove_available: true,
            sale_slot: None,
        });

        assert_eq!(
            seed_start_shop_trace_choice_labels(&run),
            vec!["purge".to_owned(), "power potion".to_owned()]
        );
    }

    #[test]
    fn seed_start_shop_choice_labels_apply_egg_preview_upgrades() {
        use sts_core::content::cards::{PANACEA_ID, WARCRY_ID};

        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::ToxicEgg);

        assert_eq!(shop_card_display_key(&run, WARCRY_ID), "Warcry+");
        assert_eq!(shop_card_display_key(&run, PANACEA_ID), "Panacea+");
    }

    #[test]
    fn shop_choose_binding_uses_core_merchant_state_and_rejects_room_index_drift() {
        let mut run = RunState::placeholder_seeded_ironclad(1_218_623, 0);
        run.gold = 999;
        sts_core::enter_shop_room(&mut run);
        assert_eq!(
            seed_start_shop_destination(&run),
            Ok(SeedStartShopDestination::Room)
        );
        assert_eq!(
            seed_start_bind_shop_choose(&run, 0),
            Ok((RunAction::EnterShop, "enter shop merchant"))
        );
        assert!(seed_start_bind_shop_choose(&run, 1)
            .expect_err("shop room choice drift must fail closed")
            .contains("choice zero"));

        let open = apply_run_action(&run, RunAction::EnterShop).expect("merchant opens");
        assert_eq!(
            seed_start_shop_destination(&open),
            Ok(SeedStartShopDestination::Screen)
        );
        let (purchase, label) = seed_start_bind_shop_choose(&open, 0)
            .expect("open merchant binds its simulator-owned first choice");
        assert_eq!(purchase, RunAction::OpenShopRemove);
        assert_eq!(label, "shop purge grid");

        let room = apply_run_action(&open, RunAction::LeaveShop).expect("merchant closes");
        let map = apply_run_action(&room, RunAction::Proceed).expect("shop room proceeds");
        assert_eq!(
            seed_start_shop_destination(&map),
            Ok(SeedStartShopDestination::Map)
        );
    }

    #[test]
    fn seed_start_neow_branch_routing_uses_generated_selected_options() {
        for (numeric_seed, command, reward) in [
            (
                1_957_307_888_551,
                "CHOOSE 1",
                NeowRewardType::RandomCommonRelic,
            ),
            (1_218_623, "CHOOSE 0", NeowRewardType::RandomColorless),
            (22_079_335_079, "CHOOSE 0", NeowRewardType::RandomColorless),
            (
                22_079_335_079,
                "CHOOSE 1",
                NeowRewardType::ThreeSmallPotions,
            ),
            (40_560_393_126, "CHOOSE 1", NeowRewardType::ThreeEnemyKill),
            (40_560_393_126, "CHOOSE 0", NeowRewardType::TransformCard),
            (40_560_393_133, "CHOOSE 0", NeowRewardType::TransformCard),
            (1_957_307_888_551, "CHOOSE 3", NeowRewardType::BossRelic),
        ] {
            assert_eq!(
                seed_start_selected_neow_option(numeric_seed, command).map(|option| option.reward),
                Some(reward),
                "{numeric_seed} {command}"
            );
        }

        assert!(seed_start_selected_neow_option(1_957_307_888_551, "PROCEED").is_none());
        assert!(seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 9").is_none());
    }

    #[test]
    fn seed_start_common_relic_uses_generated_neow_relic_reward() {
        let option = seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 1")
            .expect("VERIFY01 common relic option");
        let run = seed_start_apply_neow_relic_reward(
            1_957_307_888_551,
            &ironclad_starter_deck_keys(),
            &option,
        );

        assert_eq!(seed_start_newest_trace_relic_name(&run), "Toy Ornithopter");
        assert_eq!(
            relic_key_from_trace_name("Toy Ornithopter"),
            Some(RelicKey::ToyOrnithopter)
        );
    }

    #[test]
    fn seed_start_whetstone_neow_reward_carries_upgraded_deck() {
        let option = seed_start_selected_neow_option(-6_210_429_870_108_378_492, "CHOOSE 1")
            .expect("session-352 common relic option");
        let run = seed_start_apply_neow_relic_reward(
            -6_210_429_870_108_378_492,
            &ironclad_starter_deck_keys(),
            &option,
        );

        assert_eq!(seed_start_newest_trace_relic_name(&run), "Whetstone");
        let upgraded_positions = run
            .deck
            .iter()
            .enumerate()
            .filter_map(|(index, card)| {
                matches!(card.content_id, STRIKE_R_PLUS_ID | BASH_PLUS_ID).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            upgraded_positions,
            vec![2, 9],
            "session-352 trace upgrades original deck Strike index 2 and Bash index 9"
        );

        let session_459_option =
            seed_start_selected_neow_option(-4_817_115_419_492_724_334, "CHOOSE 1")
                .expect("session-459 common relic option");
        let session_459_run = seed_start_apply_neow_relic_reward(
            -4_817_115_419_492_724_334,
            &ironclad_starter_deck_keys(),
            &session_459_option,
        );
        assert_eq!(
            seed_start_newest_trace_relic_name(&session_459_run),
            "Whetstone"
        );
        let session_459_upgraded_positions = session_459_run
            .deck
            .iter()
            .enumerate()
            .filter_map(|(index, card)| {
                matches!(card.content_id, STRIKE_R_PLUS_ID | BASH_PLUS_ID).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            session_459_upgraded_positions,
            vec![0, 4],
            "session-459 trace upgrades original deck Strike indices 0 and 4"
        );
    }

    #[test]
    fn seed_start_rare_relic_uses_generated_neow_relic_reward_with_simple_drawback() {
        let (numeric_seed, option, run) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::TenPercentHpLoss
                            && option.reward == NeowRewardType::OneRareRelic
                    })
                    .and_then(|option| {
                        let run = seed_start_apply_neow_relic_reward(
                            seed,
                            &ironclad_starter_deck_keys(),
                            &option,
                        );
                        (seed_start_newest_trace_relic_name(&run) != "Unknown Relic")
                            .then_some((seed, option, run))
                    })
            })
            .expect("synthetic seed with max-HP loss plus mapped rare relic");

        assert!(seed_start_neow_option_is_supported_relic_reward(
            option.clone()
        ));

        assert_eq!(run.gold, 99);
        assert_eq!(run.player_hp, 72);
        assert_eq!(run.player_max_hp, 72);
        assert!(run.relics.contains(&Relic::BurningBlood));
        assert_eq!(
            seed_start_selected_neow_option(numeric_seed, &format!("CHOOSE {}", option.slot))
                .map(|option| option.reward),
            Some(NeowRewardType::OneRareRelic)
        );
        assert_ne!(seed_start_newest_trace_relic_name(&run), "Unknown Relic");
    }

    #[test]
    fn seed_start_rare_relic_supports_curse_and_rejects_non_relic_identity_branches() {
        assert!(seed_start_neow_option_is_supported_relic_reward(
            GeneratedNeowOption {
                slot: 2,
                drawback: NeowDrawback::Curse,
                reward: NeowRewardType::OneRareRelic,
                label: "obtain a curse obtain a random rare relic".to_owned(),
            }
        ));
        assert!(!seed_start_neow_option_is_supported_relic_reward(
            GeneratedNeowOption {
                slot: 2,
                drawback: NeowDrawback::TenPercentHpLoss,
                reward: NeowRewardType::RandomColorlessTwo,
                label: "lose 8 max hp choose a rare colorless card to obtain".to_owned(),
            }
        ));
    }

    #[test]
    fn seed_start_neow_rare_relic_trace_branch_reaches_leave() {
        let numeric_seed = 1_218_623;
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
            .expect("TEST slot 2 rare relic option");
        assert_eq!(option.drawback, NeowDrawback::TenPercentHpLoss);
        assert_eq!(option.reward, NeowRewardType::OneRareRelic);
        let run = seed_start_apply_neow_relic_reward(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let post_relics = vec![
            json!({ "name": "Burning Blood" }),
            json!({ "name": seed_start_newest_trace_relic_name(&run) }),
        ];
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 TEST"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": starting_deck,
                "relics": post_relics,
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": starting_deck,
                "relics": post_relics,
                "choice_list": seed_start_first_map_choices("TEST")
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow rare relic"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| transition.action_step == 4 && transition.label == "Neow leave"));
        let carried = report
            .seed_start
            .as_ref()
            .and_then(|seed_start| seed_start.sim_run_state.as_ref())
            .expect("seed-start carries simulator state after Neow leave");
        assert!(carried.event.is_none(), "Neow must be cleared after leave");
        assert!(relic_ids_for_simulated_subset(carried, &[])
            .contains(&seed_start_newest_trace_relic_name(&run)));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_neow_curse_rare_relic_delays_visible_curse_until_after_leave() {
        let numeric_seed = -1_396_911_955_486_209_732;
        let seed_string = "51KQHCFJ38T5Z";
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
            .expect("session-197 slot 2 rare relic option");
        assert_eq!(option.drawback, NeowDrawback::Curse);
        assert_eq!(option.reward, NeowRewardType::OneRareRelic);
        let run = seed_start_apply_neow_relic_reward(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let post_relics = vec![
            json!({ "name": "Burning Blood" }),
            json!({ "name": seed_start_newest_trace_relic_name(&run) }),
        ];
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": starting_deck,
                "relics": post_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow rare relic"
        }));
    }

    #[test]
    fn seed_start_boss_swap_uses_generated_boss_relic_reward() {
        let (numeric_seed, run) = (1_i64..10_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_unsupported_boss_swap_reason(run).is_none())
            .expect("synthetic seed with non-grid boss-swap relic");

        let option =
            seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");

        assert!(seed_start_neow_option_is_supported_boss_swap(option));
        assert!(!run.relics.contains(&Relic::BurningBlood));
        assert_eq!(run.relics.len() + run.relic_keys.len(), 1);

        let relic_ids = seed_start_boss_swap_relic_ids(&run);
        assert_eq!(relic_ids.len(), 1);
        assert!(!relic_ids.contains(&"Burning Blood".to_owned()));
        assert_ne!(relic_ids[0], "Unknown Relic");
    }

    #[test]
    fn seed_start_relic_projection_replaces_starter_relic_slot() {
        let mut run = RunState::map_fixture();
        run.relics = vec![
            Relic::BurningBlood,
            Relic::CentennialPuzzle,
            Relic::OddlySmoothStone,
        ];
        run.gain_relic(Relic::BlackBlood);

        let relic_ids = relic_ids_for_simulated_subset(
            &run,
            &[
                "Burning Blood".to_owned(),
                "Centennial Puzzle".to_owned(),
                "Oddly Smooth Stone".to_owned(),
            ],
        );

        assert_eq!(
            relic_ids,
            vec![
                "Black Blood".to_owned(),
                "Centennial Puzzle".to_owned(),
                "Oddly Smooth Stone".to_owned(),
            ]
        );
    }

    #[test]
    fn simulated_relic_projection_includes_active_neows_lament() {
        let mut run = RunState::map_fixture();
        run.neow_lament_combats_remaining = 3;

        let relic_ids = relic_ids_for_simulated_subset(&run, &[]);

        assert!(relic_ids.contains(&"Neow's Lament".to_owned()));
    }

    #[test]
    fn simulated_relic_projection_preserves_carried_visible_neows_lament() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.neow_lament_combats_remaining = 0;

        let relic_ids = relic_ids_for_simulated_subset(&run, &["Neow's Lament".to_owned()]);

        assert!(relic_ids.contains(&"Neow's Lament".to_owned()));
    }

    #[test]
    fn simulated_relic_projection_preserves_carried_relic_order_before_new_pickups() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::BurningBlood, Relic::Lantern];
        run.neow_lament_combats_remaining = 0;

        let relic_ids = relic_ids_for_simulated_subset(
            &run,
            &["Burning Blood".to_owned(), "Neow's Lament".to_owned()],
        );

        assert_eq!(
            relic_ids,
            vec![
                "Burning Blood".to_owned(),
                "Neow's Lament".to_owned(),
                "Lantern".to_owned(),
            ]
        );
    }

    #[test]
    fn treasure_projection_preserves_carried_visible_neows_lament() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_floor = 9;
        run.neow_lament_combats_remaining = 0;

        let subset = seed_start_treasure_simulated_subset(&run, &["Neow's Lament".to_owned()]);
        let relic_ids = subset["relic_ids"]
            .as_array()
            .expect("relic_ids is an array")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();

        assert!(relic_ids.contains(&"Neow's Lament"));
    }

    #[test]
    fn runic_dome_boss_swap_jaw_worm_sequence_keeps_hidden_attack_damage() {
        let mut run =
            seed_start_apply_neow_boss_swap(903_575_075_592_564_628, &ironclad_starter_deck_keys());
        assert!(run_has_relic_key(&run, RelicKey::RunicDome));
        let map_action = legal_map_actions_on_run(&run)
            .into_iter()
            .next()
            .expect("first map action");
        run = apply_map_action_on_run(&run, map_action).expect("enter first combat");
        run = serde_json::from_value(serde_json::to_value(run).expect("serialize run"))
            .expect("deserialize run");

        for command in [
            "PLAY 2", "PLAY 1 0", "PLAY 2 0", "END", "PLAY 2 0", "PLAY 2 0", "PLAY 2 0",
        ] {
            let action = combat_action_from_command(command, run.combat.as_ref().unwrap())
                .expect("combat command");
            run = apply_combat_action_on_run(&run, action).expect("combat action applies");
        }

        assert_eq!(run.player_hp, 74);
        assert_eq!(run.phase, RunPhase::Reward);
    }

    #[test]
    fn seed_start_boss_swap_loses_burning_blood_before_black_blood_eligibility() {
        let run = seed_start_apply_neow_boss_swap(
            -3_280_889_720_909_526_167,
            &ironclad_starter_deck_keys(),
        );

        assert!(run_has_relic_key(&run, RelicKey::CallingBell));
        assert!(!run_has_relic_key(&run, RelicKey::BlackBlood));
        assert!(!run.relics.contains(&Relic::BurningBlood));
    }

    #[test]
    fn seed_start_grid_observed_subset_uses_screen_cards_when_choice_list_is_empty() {
        let message = json!({
            "game_state": {
                "screen_type": "GRID",
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": [],
                "relics": [{"name": "Calling Bell"}],
                "choice_list": [],
                "screen_state": {
                    "cards": [{"name": "Curse of the Bell", "id": "CurseOfTheBell"}]
                }
            }
        });

        assert_eq!(
            seed_start_grid_observed_subset(&message)["choices"],
            json!(["curse of the bell"])
        );
    }

    #[test]
    fn seed_start_grid_observed_subset_hides_cards_when_grid_confirm_is_up() {
        let message = json!({
            "game_state": {
                "screen_type": "GRID",
                "floor": 3,
                "gold": 73,
                "current_hp": 79,
                "max_hp": 80,
                "deck": [],
                "relics": [],
                "choice_list": [],
                "screen_state": {
                    "confirm_up": true,
                    "for_upgrade": true,
                    "cards": [{"name": "Strike", "id": "Strike_R"}]
                }
            }
        });

        assert_eq!(
            seed_start_grid_observed_subset(&message)["choices"],
            json!([])
        );
    }

    #[test]
    fn seed_start_grid_simulated_subset_hides_completed_transform_selection() {
        let mut run = RunState::map_fixture();
        run.card_grid = Some(CardGridScreen {
            cards: vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)],
            purpose: GridPurpose::EventTransformReturnToEvent {
                event: Event::Transmorgrifier,
                count: 1,
            },
            selected: None,
            selected_indices: vec![0],
        });

        assert_eq!(
            seed_start_grid_simulated_subset(&run, &[])["choices"],
            json!([])
        );
    }

    #[test]
    fn observed_event_obtain_grid_does_not_replace_simulated_choices() {
        let mut run = RunState::map_fixture();
        let simulated_cards = vec![
            CardInstance::new(CardId::new(10_001), STRIKE_R_ID),
            CardInstance::new(CardId::new(10_002), DEFEND_R_ID),
        ];
        run.card_grid = Some(CardGridScreen {
            cards: simulated_cards.clone(),
            purpose: GridPurpose::EventObtainCardReturnToEvent {
                event: Event::TheLibrary,
            },
            selected: None,
            selected_indices: Vec::new(),
        });
        let forged_observation = json!({
            "game_state": {
                "screen_type": "GRID",
                "choice_list": ["bash", "anger"],
                "screen_state": {"cards": []}
            }
        });

        let observed = seed_start_grid_observed_subset(&forged_observation);
        let simulated = seed_start_grid_simulated_subset(&run, &[]);

        assert_eq!(observed["choices"], json!(["bash", "anger"]));
        assert_eq!(simulated["choices"], json!(["strike", "defend"]));
        assert!(!subset_diffs(
            json!({"choices": observed["choices"].clone()}),
            json!({"choices": simulated["choices"].clone()}),
        )
        .is_empty());
        assert_eq!(
            run.card_grid.as_ref().expect("simulated grid").cards,
            simulated_cards
        );
    }

    #[test]
    fn seed_start_event_grid_projection_delays_confirmed_transform_output() {
        let mut run = RunState::map_fixture();
        let visible_before = run.deck.len();
        run.gain_deck_card(sts_core::content::cards::PERFECTED_STRIKE_ID);

        let projected =
            seed_start_event_simulated_subset_with_delayed_deck_append(&run, &[], Some(1));
        let deck = projected["deck_ids"].as_array().expect("projected deck");

        assert_eq!(deck.len(), visible_before);
        assert!(!deck.iter().any(|card| card == "Perfected Strike"));
    }

    #[test]
    fn event_projection_defers_simulator_owned_pending_obtain_cards() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheSsssserpent));
        let deck_before = deck_content_keys(&run.deck);
        run.queue_pending_obtain_card(sts_core::content::cards::REGRET_ID);

        let transient_projection = seed_start_event_simulated_subset(&run, &[]);
        let settled_projection = deck_content_keys_after_pending_obtain_cards_settle(&run);

        assert_eq!(transient_projection["deck_ids"], json!(deck_before));
        let mut expected_deck = deck_before;
        expected_deck.push("Regret".to_owned());
        assert_eq!(settled_projection, expected_deck);

        let mut protected = RunState::placeholder_seeded_ironclad(1, 0);
        let protected_deck = deck_content_keys(&protected.deck);
        protected.gain_relic_key(RelicKey::Omamori);
        protected.queue_pending_obtain_card(sts_core::content::cards::PAIN_ID);
        assert_eq!(
            deck_content_keys_after_pending_obtain_cards_settle(&protected),
            protected_deck,
            "settled projection must apply core card-obtain prevention"
        );
        assert_eq!(protected.omamori_charges_used, 0, "projection is read-only");

        assert_eq!(
            classify_deferred_deck_observation(
                &["Strike".to_owned()],
                &["Strike".to_owned()],
                &["Strike".to_owned(), "Regret".to_owned()],
            ),
            PendingDeckObservation::Deferred
        );
        assert_eq!(
            classify_deferred_deck_observation(
                &["Strike".to_owned(), "Regret".to_owned()],
                &["Strike".to_owned()],
                &["Strike".to_owned(), "Regret".to_owned()],
            ),
            PendingDeckObservation::Settled
        );
        assert!(matches!(
            classify_deferred_deck_observation(
                &["Strike".to_owned(), "Pain".to_owned()],
                &["Strike".to_owned()],
                &["Strike".to_owned(), "Regret".to_owned()],
            ),
            PendingDeckObservation::Diverged(diffs) if !diffs.is_empty()
        ));
    }

    #[test]
    fn seed_start_vampires_projection_delays_bites_until_leave() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.current_act = 2;
        run.current_floor = 31;
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::Vampires));
        let accepted = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
            .expect("Vampires accept applies");

        let projected = seed_start_event_simulated_subset_with_delayed_deck_append(
            &accepted,
            &[],
            Some(VAMPIRES_BITE_COUNT),
        );
        let deck = projected["deck_ids"].as_array().expect("projected deck");

        assert!(!deck
            .iter()
            .any(|card| { card.as_str().is_some_and(|name| name.starts_with("Strike")) }));
        assert!(!deck.iter().any(|card| card == "Bite"));
    }

    #[test]
    fn seed_start_grid_observed_subset_keeps_confirm_only_grid_cards() {
        let message = json!({
            "game_state": {
                "screen_type": "GRID",
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": [],
                "relics": [{"name": "Pandora's Box"}],
                "choice_list": [],
                "screen_state": {
                    "confirm_up": true,
                    "selected_cards": [],
                    "cards": [{"name": "Body Slam", "id": "Body Slam"}]
                }
            }
        });

        assert_eq!(
            seed_start_grid_observed_subset(&message)["choices"],
            json!(["body slam"])
        );
    }

    #[test]
    fn seed_start_boss_swap_immediate_boss_relics_route_to_neow_leave() {
        let immediate_boss_relics: Vec<_> = IRONCLAD_BOSS_RELIC_POOL
            .iter()
            .copied()
            .filter(|key| {
                !matches!(
                    key,
                    RelicKey::Astrolabe
                        | RelicKey::PandorasBox
                        | RelicKey::EmptyCage
                        | RelicKey::CallingBell
                        | RelicKey::BlackBlood
                        | RelicKey::TinyHouse
                )
            })
            .collect();
        let mut covered = Vec::new();

        for numeric_seed in 1_i64..2_000_000 {
            let run = seed_start_apply_neow_boss_swap(numeric_seed, &ironclad_starter_deck_keys());
            let Some(swapped_key) = run
                .relics
                .iter()
                .map(|relic| relic.key())
                .chain(run.relic_keys.iter().copied())
                .find(|key| *key != RelicKey::BurningBlood)
            else {
                continue;
            };
            if !immediate_boss_relics.contains(&swapped_key)
                || covered
                    .iter()
                    .any(|(covered_key, _, _)| *covered_key == swapped_key)
            {
                continue;
            }

            assert_eq!(seed_start_unsupported_boss_swap_reason(&run), None);

            let seed_string = test_seed_string_from_long(numeric_seed);
            let deck: Vec<_> = ironclad_starter_deck_keys()
                .into_iter()
                .map(|id| json!({ "id": id }))
                .collect();
            let post_swap_deck: Vec<_> = deck_content_keys(&run.deck)
                .into_iter()
                .map(|id| json!({ "id": id }))
                .collect();
            let swapped_relics: Vec<_> = seed_start_boss_swap_relic_ids(&run)
                .into_iter()
                .map(|name| json!({ "name": name }))
                .collect();
            let lines = vec![
                json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
                json!({"type": "state", "step": 0, "message": {}}),
                json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
                json!({"type": "state", "step": 1, "message": {"game_state": {
                    "screen_type": "EVENT",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": deck,
                    "relics": [{"name": "Burning Blood"}],
                    "choice_list": ["talk"]
                }}}),
                json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
                json!({"type": "state", "step": 2, "message": {"game_state": {
                    "screen_type": "EVENT",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": deck,
                    "relics": [{"name": "Burning Blood"}],
                    "choice_list": seed_start_neow_choices(numeric_seed)
                }}}),
                json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
                json!({"type": "state", "step": 3, "message": {"game_state": {
                    "screen_type": "EVENT",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": post_swap_deck,
                    "relics": swapped_relics,
                    "choice_list": ["leave"]
                }}}),
                json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
                json!({"type": "state", "step": 4, "message": {"game_state": {
                    "screen_type": "MAP",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": post_swap_deck,
                    "relics": swapped_relics,
                    "choice_list": seed_start_first_map_choices(&seed_string)
                }}}),
            ];
            let content = lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            let report =
                verify_seed_start_communication_mod_trace(&content).expect("seed-start verifies");

            assert_eq!(report.unexpected_diffs, Vec::new());
            assert!(
                report
                    .unsupported
                    .iter()
                    .all(|transition| transition.action_step < 3),
                "boss-swap path produced unsupported transitions: {:?}",
                report.unsupported
            );
            assert!(report.verified.iter().any(|transition| {
                transition.action_step == 3 && transition.label == "Neow boss swap"
            }));
            assert!(report.verified.iter().any(|transition| {
                transition.action_step == 4 && transition.label == "Neow leave"
            }));

            covered.push((swapped_key, numeric_seed, seed_string));
            if covered.len() == immediate_boss_relics.len() {
                break;
            }
        }

        let missing: Vec<_> = immediate_boss_relics
            .iter()
            .copied()
            .filter(|key| !covered.iter().any(|(covered_key, _, _)| covered_key == key))
            .collect();
        assert_eq!(missing, Vec::new(), "missing immediate boss relic coverage");
    }

    #[test]
    fn seed_start_boss_swap_classifies_grid_opening_relics() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::Astrolabe);

        assert_eq!(
            seed_start_unsupported_boss_swap_reason(&run),
            Some(
                "Neow boss-swap produced a grid-opening boss relic without a dedicated seed-start follow-up; downstream parity remains classified"
                    .to_owned()
            )
        );
    }

    #[test]
    fn seed_start_boss_swap_calling_bell_grid_rewards_are_taken_before_neow_leave() {
        let (numeric_seed, bell_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_calling_bell_grid(run))
            .expect("synthetic seed with Calling Bell boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let bell_relic_names = seed_start_boss_swap_relic_ids(&bell_run);
        let bell_relics: Vec<_> = bell_relic_names
            .iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_confirm = confirm_grid(&bell_run).expect("Calling Bell grid confirms");
        assert!(
            after_confirm.card_rng_counter >= 9,
            "Calling Bell must consume the hidden NeowRoom card reward before replacing it with relics"
        );
        let after_common =
            apply_run_action(&after_confirm, RunAction::TakeRelicReward).expect("take common");
        let after_uncommon =
            apply_run_action(&after_common, RunAction::TakeRelicReward).expect("take uncommon");
        let after_rare =
            apply_run_action(&after_uncommon, RunAction::TakeRelicReward).expect("take rare");
        let bell_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let common_relics: Vec<_> =
            relic_ids_for_simulated_subset(&after_common, &bell_relic_names)
                .into_iter()
                .filter(|name| name != "Unknown Relic")
                .map(|name| json!({ "name": name }))
                .collect();
        let uncommon_relics: Vec<_> =
            relic_ids_for_simulated_subset(&after_uncommon, &bell_relic_names)
                .into_iter()
                .filter(|name| name != "Unknown Relic")
                .map(|name| json!({ "name": name }))
                .collect();
        let rare_relics: Vec<_> = relic_ids_for_simulated_subset(&after_rare, &bell_relic_names)
            .into_iter()
            .filter(|name| name != "Unknown Relic")
            .map(|name| json!({ "name": name }))
            .collect();

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": bell_relics,
                "choice_list": ["curse of the bell"]
            }}}),
            json!({"type": "action", "step": 4, "command": "PROCEED"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": bell_relics,
                "choice_list": ["relic", "relic", "relic"],
                "screen_state": {
                    "rewards": [{"reward_type": "RELIC"}, {"reward_type": "RELIC"}, {"reward_type": "RELIC"}]
                }
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": common_relics,
                "choice_list": ["relic", "relic"],
                "screen_state": {
                    "rewards": [{"reward_type": "RELIC"}, {"reward_type": "RELIC"}]
                }
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": uncommon_relics,
                "choice_list": ["relic"],
                "screen_state": {
                    "rewards": [{"reward_type": "RELIC"}]
                }
            }}}),
            json!({"type": "action", "step": 7, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 7, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": rare_relics,
                "screen_state": {"rewards": []}
            }}}),
            json!({"type": "action", "step": 8, "command": "PROCEED"}),
            json!({"type": "state", "step": 8, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": rare_relics,
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Calling Bell grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "Neow boss swap Calling Bell rewards"
        }));
        assert_eq!(
            report
                .verified
                .iter()
                .filter(|transition| transition.label == "relic reward")
                .count(),
            3
        );
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 8 && transition.label == "empty Neow reward proceed to map"
        }));
    }

    #[test]
    fn seed_start_boss_swap_astrolabe_grid_transforms_three_selected_cards() {
        let (numeric_seed, astrolabe_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_astrolabe_grid(run))
            .expect("synthetic seed with Astrolabe boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let astrolabe_relics: Vec<_> = seed_start_boss_swap_relic_ids(&astrolabe_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_first = select_grid_card(&astrolabe_run, 0).expect("select first");
        let after_second = select_grid_card(&after_first, 1).expect("select second");
        let after_third = select_grid_card(&after_second, 2).expect("select third");
        let after_confirm = confirm_grid(&after_third).expect("confirm Astrolabe transforms");
        let transformed_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&astrolabe_run, &["Astrolabe".to_owned()])["choices"]
                .as_array()
                .expect("grid choices")
                .clone();

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": astrolabe_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": astrolabe_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": astrolabe_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": transformed_deck,
                "relics": astrolabe_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Astrolabe grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 6
                && transition.label == "Neow boss swap Astrolabe transformed"
        }));
    }

    #[test]
    fn seed_start_boss_swap_pandoras_box_grid_confirms_to_neow_leave() {
        let (numeric_seed, pandora_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_pandoras_box_grid(run))
            .expect("synthetic seed with Pandora's Box boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let pandora_relics: Vec<_> = seed_start_boss_swap_relic_ids(&pandora_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_confirm = confirm_grid(&pandora_run).expect("Pandora's Box grid confirms");
        let grid_deck: Vec<_> = deck_content_keys(&pandora_run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let transformed_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&pandora_run, &["Pandora's Box".to_owned()])
                ["choices"]
                .as_array()
                .expect("grid choices")
                .clone();

        assert_eq!(pandora_run.card_grid.as_ref().expect("grid").cards.len(), 9);
        assert_eq!(pandora_run.deck.len(), 1);
        assert_eq!(after_confirm.deck.len(), 10);

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": grid_deck,
                "relics": pandora_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CONFIRM"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": transformed_deck,
                "relics": pandora_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Pandora's Box grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4
                && transition.label == "Neow boss swap Pandora's Box confirm"
        }));
    }

    #[test]
    fn seed_start_boss_swap_pandoras_box_grid_matches_live_trace_pool_order() {
        let run = seed_start_apply_neow_boss_swap(
            7_003_943_589_014_798_927,
            &ironclad_starter_deck_keys(),
        );
        let grid = run.card_grid.as_ref().expect("Pandora's Box grid");
        let subset = seed_start_grid_simulated_subset(&run, &["Pandora's Box".to_owned()]);
        let choices = subset
            .get("choices")
            .and_then(Value::as_array)
            .expect("Pandora choices");

        assert_eq!(grid.purpose, GridPurpose::PandorasBox);
        assert_eq!(
            choices,
            &[
                json!("berserk"),
                json!("iron wave"),
                json!("evolve"),
                json!("feed"),
                json!("wild strike"),
                json!("rage"),
                json!("bludgeon"),
                json!("perfected strike"),
                json!("anger"),
            ]
        );
    }

    #[test]
    fn seed_start_boss_swap_empty_cage_grid_removes_two_cards_to_neow_leave() {
        let (numeric_seed, empty_cage_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_empty_cage_grid(run))
            .expect("synthetic seed with Empty Cage boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let empty_cage_relics: Vec<_> = seed_start_boss_swap_relic_ids(&empty_cage_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_first_select = select_grid_card(&empty_cage_run, 0).expect("select first");
        let after_first_confirm = confirm_grid(&after_first_select).expect("remove first");
        let after_second_select = select_grid_card(&after_first_confirm, 0).expect("select second");
        let after_second_confirm = confirm_grid(&after_second_select).expect("remove second");
        let first_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&empty_cage_run, &["Empty Cage".to_owned()])
                ["choices"]
                .as_array()
                .expect("first grid choices")
                .clone();
        let second_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&after_first_confirm, &["Empty Cage".to_owned()])
                ["choices"]
                .as_array()
                .expect("second grid choices")
                .clone();
        let one_removed_deck: Vec<_> = deck_content_keys(&after_first_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let two_removed_deck: Vec<_> = deck_content_keys(&after_second_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();

        assert_eq!(empty_cage_run.deck.len(), 10);
        assert_eq!(after_first_confirm.deck.len(), 9);
        assert_eq!(after_second_confirm.deck.len(), 8);

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": empty_cage_relics,
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": empty_cage_relics,
                "choice_list": []
            }}}),
            json!({"type": "action", "step": 5, "command": "CONFIRM"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": one_removed_deck,
                "relics": empty_cage_relics,
                "choice_list": second_grid_choices
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": one_removed_deck,
                "relics": empty_cage_relics,
                "choice_list": []
            }}}),
            json!({"type": "action", "step": 7, "command": "CONFIRM"}),
            json!({"type": "state", "step": 7, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": two_removed_deck,
                "relics": empty_cage_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Empty Cage grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 7 && transition.label == "Neow boss swap Empty Cage confirm"
        }));
    }

    #[test]
    fn seed_start_simple_neow_reward_uses_core_helper() {
        let option = seed_start_selected_neow_option(40_560_393_133, "CHOOSE 1")
            .expect("M290008 slot 1 option");

        assert_eq!(option.reward, NeowRewardType::HundredGold);
        assert_eq!(
            seed_start_apply_neow_simple_option(option),
            Some((199, 80, 80))
        );
    }

    #[test]
    fn seed_start_simple_neow_drawback_and_reward_use_core_helpers() {
        let option = seed_start_selected_neow_option(40_560_393_133, "CHOOSE 2")
            .expect("M290008 slot 2 option");

        assert_eq!(option.drawback, NeowDrawback::NoGold);
        assert_eq!(option.reward, NeowRewardType::TwentyPercentHpBonus);
        assert_eq!(
            seed_start_apply_neow_simple_option(option),
            Some((0, 96, 96))
        );
    }

    #[test]
    fn seed_start_simple_neow_helper_rejects_identity_branches() {
        let option = seed_start_selected_neow_option(40_560_393_133, "CHOOSE 0")
            .expect("M290008 slot 0 option");

        assert_eq!(option.reward, NeowRewardType::TransformCard);
        assert_eq!(seed_start_apply_neow_simple_option(option), None);
    }

    #[test]
    fn seed_start_transform_card_exact_live_seed_generates_searing_blow() {
        assert_eq!(
            seed_start_generated_transform_card(8_418_289_729_765_700_364).as_deref(),
            Some("Searing Blow")
        );
    }

    #[test]
    fn m34_selected_modified_deck_opening_piles_are_seed_derived() {
        for case in [
            (
                "communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl",
                "CODEX04 colorless innate",
            ),
            (
                "permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl",
                "TEST obtained colorless",
            ),
            (
                "communication_mod/trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl",
                "M290001 transformed card",
            ),
            (
                "communication_mod/trace-2026-06-23T07-42-06-085Z.best-run.jsonl",
                "M290008 transformed card",
            ),
        ] {
            assert_selected_trace_first_combat_opening_is_seed_derived(case.0, case.1);
        }
    }

    #[test]
    fn seed_start_neow_curse_simple_helper_uses_card_rng_and_limits_rewards() {
        let option = seed_start_selected_neow_option(40_560_393_126, "CHOOSE 2")
            .expect("M290001 slot 2 option");

        assert_eq!(option.drawback, NeowDrawback::Curse);
        assert_eq!(option.reward, NeowRewardType::TwentyPercentHpBonus);
        assert!(seed_start_neow_option_is_supported_curse_simple(
            option.clone()
        ));
        assert!(!seed_start_neow_option_is_supported_curse_simple(
            GeneratedNeowOption {
                slot: 2,
                drawback: NeowDrawback::Curse,
                reward: NeowRewardType::ThreeRareCards,
                label: "obtain a curse choose a rare card to obtain".to_owned(),
            }
        ));

        let run = seed_start_apply_neow_curse_simple_option(
            40_560_393_126,
            &ironclad_starter_deck_keys(),
            option,
        );
        let deck_ids = deck_content_keys(&run.deck);

        assert_eq!(run.gold, 99);
        assert_eq!(run.player_hp, 96);
        assert_eq!(run.player_max_hp, 96);
        assert_eq!(run.card_rng_counter, 1);
        assert_eq!(run.card_random_rng_counter, 0);
        assert_eq!(deck_ids.len(), 11);
        assert!(matches!(
            deck_ids.last().map(String::as_str),
            Some(
                "Clumsy"
                    | "Decay"
                    | "Doubt"
                    | "Injury"
                    | "Normality"
                    | "Pain"
                    | "Parasite"
                    | "Regret"
                    | "Shame"
                    | "Writhe"
            )
        ));
    }

    #[test]
    fn seed_start_neow_curse_gold_helper_uses_same_card_rng_path() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::TwoFiftyGold
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with curse plus 250 gold");

        assert!(seed_start_neow_option_is_supported_curse_simple(
            option.clone()
        ));

        let run = seed_start_apply_neow_curse_simple_option(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            option,
        );

        assert_eq!(run.gold, 349);
        assert_eq!(run.player_hp, 80);
        assert_eq!(run.player_max_hp, 80);
        assert_eq!(run.card_rng_counter, 1);
        assert_eq!(run.deck.len(), ironclad_starter_deck_keys().len() + 1);

        let traced_seed = 8_749_615_620_867_394_322;
        let traced_option = seed_start_selected_neow_option(traced_seed, "CHOOSE 2")
            .expect("session-123 curse plus 250 gold option");
        assert_eq!(traced_option.drawback, NeowDrawback::Curse);
        assert_eq!(traced_option.reward, NeowRewardType::TwoFiftyGold);
        let traced_run = seed_start_apply_neow_curse_simple_option(
            traced_seed,
            &ironclad_starter_deck_keys(),
            traced_option,
        );
        assert_eq!(traced_run.gold, 349);
        assert_eq!(
            deck_content_keys(&traced_run.deck)
                .last()
                .map(String::as_str),
            Some("Normality")
        );
    }

    #[test]
    fn seed_start_neow_curse_simple_trace_branch_reaches_leave() {
        let numeric_seed = 40_560_393_126;
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec![json!({ "name": "Burning Blood" })];
        let _option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
            .expect("M290001 curse max-HP option");
        let visible_post_deck = starting_deck.clone();
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 M290001"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 96,
                "max_hp": 96,
                "deck": visible_post_deck,
                "relics": relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow curse immediate reward"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_neow_grid_reward_dispatch_opens_core_upgrade_grid() {
        let (numeric_seed, command, option) = (1_i64..10_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| option.reward == NeowRewardType::UpgradeCard)
                    .map(|option| (seed, format!("CHOOSE {}", option.slot), option))
            })
            .expect("synthetic seed with upgrade-card option");

        assert_eq!(
            seed_start_selected_neow_option(numeric_seed, &command),
            Some(option.clone())
        );
        assert!(seed_start_neow_option_is_supported_grid_reward(
            option.clone()
        ));

        let run = seed_start_open_neow_grid_run(1, &ironclad_starter_deck_keys(), &option);

        assert_eq!(
            seed_start_grid_simulated_subset(&run, &["Burning Blood".to_owned()]),
            json!({
                "screen_type": "GRID",
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood"],
                "choices": ["strike", "strike", "strike", "strike", "strike", "defend", "defend", "defend", "defend", "bash"],
            })
        );
    }

    #[test]
    fn seed_start_neow_upgrade_grid_choose_confirm_returns_to_leave() {
        let option = GeneratedNeowOption {
            slot: 0,
            drawback: NeowDrawback::None,
            reward: NeowRewardType::UpgradeCard,
            label: "upgrade a card".to_owned(),
        };
        let mut run = seed_start_open_neow_grid_run(1, &ironclad_starter_deck_keys(), &option);

        run = select_grid_card(&run, 0).expect("select first strike");
        assert_eq!(
            seed_start_grid_simulated_subset(&run, &["Burning Blood".to_owned()]),
            json!({
                "screen_type": "GRID",
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood"],
                "choices": [],
            })
        );

        run = confirm_grid(&run).expect("confirm upgrade");

        assert!(run.card_grid.is_none());
        assert_eq!(
            run.deck[0].content_id,
            sts_core::content::cards::STRIKE_R_PLUS_ID
        );
        assert_eq!(
            deck_content_keys(&run.deck),
            vec![
                "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
                "Defend_R", "Defend_R", "Bash",
            ]
        );
    }

    #[test]
    fn seed_start_neow_remove_two_grid_keeps_full_grid_until_second_selection() {
        let option = GeneratedNeowOption {
            slot: 0,
            drawback: NeowDrawback::TenPercentHpLoss,
            reward: NeowRewardType::RemoveTwo,
            label: "lose 8 max hp remove 2 cards".to_owned(),
        };
        let mut run = seed_start_open_neow_grid_run(1, &ironclad_starter_deck_keys(), &option);

        assert_eq!(run.player_hp, 72);
        assert_eq!(run.player_max_hp, 72);

        run = select_grid_card(&run, 0).expect("select first strike");

        assert!(run.card_grid.is_some());
        assert_eq!(run.deck.len(), 10);
        assert_eq!(
            seed_start_grid_simulated_subset(&run, &["Burning Blood".to_owned()])["choices"]
                .as_array()
                .expect("choices")
                .len(),
            10
        );

        run = select_grid_card(&run, 1).expect("select second strike");
        run = confirm_grid(&run).expect("remove selected strikes");

        assert!(run.card_grid.is_none());
        assert_eq!(run.deck.len(), 8);
    }

    #[test]
    fn seed_start_neow_remove_two_generated_grid_trace_reaches_neow_leave() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::RemoveTwo
                            && seed_start_neow_option_is_supported_grid_reward(option.clone())
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with generated remove-two option");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let choose_command = format!("CHOOSE {}", option.slot);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec!["Burning Blood".to_owned()];
        let initial_run =
            seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
        let after_first_select = select_grid_card(&initial_run, 0).expect("select first");
        let after_second_select = select_grid_card(&after_first_select, 1).expect("select second");
        let after_second_confirm = confirm_grid(&after_second_select).expect("remove second");
        let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run, &relics)
            ["choices"]
            .as_array()
            .expect("first grid choices")
            .clone();
        let two_removed_deck: Vec<_> = deck_content_keys(&after_second_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let hp = initial_run.player_hp;
        let max_hp = initial_run.player_max_hp;
        let gold = initial_run.gold;

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": choose_command}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": two_removed_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": two_removed_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow remove two grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| { transition.action_step == 6 && transition.label == "Neow leave" }));
    }

    #[test]
    fn seed_start_neow_upgrade_generated_grid_trace_reaches_neow_leave() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::UpgradeCard
                            && seed_start_neow_option_is_supported_grid_reward(option.clone())
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with generated upgrade-card option");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let choose_command = format!("CHOOSE {}", option.slot);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec!["Burning Blood".to_owned()];
        let initial_run =
            seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
        let after_select = select_grid_card(&initial_run, 0).expect("select first");
        let after_confirm = confirm_grid(&after_select).expect("confirm upgrade");
        let grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run, &relics)
            ["choices"]
            .as_array()
            .expect("grid choices")
            .clone();
        let upgraded_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let hp = initial_run.player_hp;
        let max_hp = initial_run.player_max_hp;
        let gold = initial_run.gold;

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": choose_command}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": []
            }}}),
            json!({"type": "action", "step": 5, "command": "CONFIRM"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": upgraded_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": upgraded_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow upgrade grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| { transition.action_step == 6 && transition.label == "Neow leave" }));
    }

    #[test]
    fn seed_start_neow_curse_transform_two_generated_trace_can_observe_map_after_second_pick() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::TransformTwoCards
                            && seed_start_neow_option_is_supported_grid_reward(option.clone())
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with generated curse plus transform-two option");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let choose_command = format!("CHOOSE {}", option.slot);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec!["Burning Blood".to_owned()];
        let initial_run =
            seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
        let after_first_select = select_grid_card(&initial_run, 0).expect("select first");
        let after_second_select =
            select_grid_card(&after_first_select, 1).expect("select second and transform");
        let after_confirm = confirm_grid(&after_second_select).expect("confirm transform two");
        let grid_deck: Vec<_> = deck_content_keys(&initial_run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let curse_key =
            seed_start_neow_curse_deck_key(numeric_seed, 0).expect("generated curse key");
        let mut first_select_deck = deck_content_keys(&initial_run.deck);
        first_select_deck.push(curse_key.clone());
        let first_select_deck: Vec<_> = first_select_deck
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let transformed_deck = deck_content_keys(&after_confirm.deck);
        let mut visible_confirm_deck = transformed_deck.clone();
        for _ in 0..2 {
            visible_confirm_deck.pop();
        }
        visible_confirm_deck.push(curse_key);
        let mut final_map_deck = visible_confirm_deck.clone();
        let transformed_start = transformed_deck.len().saturating_sub(2);
        final_map_deck.extend(transformed_deck[transformed_start..].iter().cloned());
        let final_map_deck: Vec<_> = final_map_deck
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run, &relics)
            ["choices"]
            .as_array()
            .expect("first grid choices")
            .clone();
        let second_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&after_first_select, &relics)["choices"]
                .as_array()
                .expect("second grid choices")
                .clone();
        let hp = initial_run.player_hp;
        let max_hp = initial_run.player_max_hp;
        let gold = initial_run.gold;

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": choose_command}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": grid_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": first_select_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": second_grid_choices
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": final_map_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow transform two grid"
        }));
        assert_eq!(initial_run.card_rng_counter, 0);
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
    }

    #[test]
    fn seed_start_neow_card_reward_choices_use_generated_helper() {
        let option = seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 0")
            .expect("VERIFY01 slot 0 option");

        assert_eq!(option.reward, NeowRewardType::ThreeCards);
        assert!(seed_start_neow_option_is_supported_card_reward(
            option.clone()
        ));

        let ids = seed_start_neow_card_reward_ids(1_957_307_888_551, &option, None);
        let names = seed_start_neow_card_reward_choice_names(1_957_307_888_551, &option, None);

        assert_eq!(ids.len(), 3);
        assert_eq!(names.len(), 3);
        assert_eq!(
            names,
            ids.iter()
                .map(|id| id.to_ascii_lowercase())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn seed_start_colorless_neow_pick_carries_card_rng_to_first_combat_reward() {
        let numeric_seed = 22_079_335_079;
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 0")
            .expect("CODEX04 slot 0 option");

        assert_eq!(option.reward, NeowRewardType::RandomColorless);

        let mut run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
        assert_eq!(reward_ids[1], deck_content_key(DRAMATIC_ENTRANCE_ID));

        assert_eq!(run.event_rng_counter, 0);
        run.card_rng_counter =
            seed_start_neow_card_reward_card_rng_counter(numeric_seed, &option, Some(&run))
                .expect("colorless reward consumes cardRng");
        let mut deck_ids = deck_content_keys(&run.deck);
        deck_ids.push(reward_ids[1].clone());
        run.deck = deck_instances_from_keys(&deck_ids);

        enter_normal_combat_reward_screen(&mut run);

        let reward = run.reward.as_ref().expect("combat reward screen");
        let ids = reward
            .choices
            .iter()
            .map(|choice| choice.content_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![BATTLE_TRANCE_ID, TWIN_STRIKE_ID, ENTRENCH_ID]);
    }

    #[test]
    fn seed_start_neow_three_rare_cards_can_pick_card_leave_and_reach_map() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::ThreeRareCards
                            && seed_start_neow_drawback_is_simple(option.drawback)
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with simple ThreeRareCards option");
        let external_seed = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let neow_deck: Vec<_> = deck_content_keys(&run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
        let reward_names =
            seed_start_neow_card_reward_choice_names(numeric_seed, &option, Some(&run));
        let reward_cards: Vec<_> = reward_ids
            .iter()
            .map(|id| json!({ "id": id, "name": id }))
            .collect();
        let mut picked_deck = neow_deck.clone();
        picked_deck.push(json!({ "id": reward_ids[1] }));

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": format!("CHOOSE {}", option.slot)}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": neow_deck,
                "relics": starting_relics,
                "choice_list": reward_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": picked_deck,
                "relics": starting_relics,
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": picked_deck,
                "relics": starting_relics,
                "choice_list": seed_start_first_map_choices(&external_seed)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow rare card reward choices"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "Neow colorless pickup"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| transition.action_step == 5 && transition.label == "Neow leave"));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_neow_curse_three_rare_cards_delays_curse_until_leave() {
        let numeric_seed = 3_768_852_066_369_722_076;
        let external_seed = "1418KCQFMRQCW";
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
            .expect("session-113 slot 2 option");
        assert_eq!(option.drawback, NeowDrawback::Curse);
        assert_eq!(option.reward, NeowRewardType::ThreeRareCards);

        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let neow_deck: Vec<_> = deck_content_keys(&run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
        let reward_names =
            seed_start_neow_card_reward_choice_names(numeric_seed, &option, Some(&run));
        let reward_cards: Vec<_> = reward_ids
            .iter()
            .map(|id| json!({ "id": id, "name": id }))
            .collect();
        let curse_key =
            seed_start_neow_curse_deck_key(numeric_seed, 0).expect("curse generated after leave");

        let picked_card = reward_ids[0].clone();
        let mut leave_deck = neow_deck.clone();
        leave_deck.push(json!({ "id": picked_card }));
        let mut map_deck = leave_deck.clone();
        map_deck.push(json!({ "id": curse_key }));

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": neow_deck,
                "relics": starting_relics,
                "choice_list": reward_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": leave_deck,
                "relics": starting_relics,
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": map_deck,
                "relics": starting_relics,
                "choice_list": seed_start_first_map_choices(external_seed)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report
            .verified
            .iter()
            .any(|transition| transition.action_step == 4
                && transition.label == "Neow colorless pickup"));
        assert!(report
            .verified
            .iter()
            .any(|transition| transition.action_step == 5 && transition.label == "Neow leave"));
    }

    #[test]
    fn seed_start_neow_immediate_random_rare_leave_keeps_sim_card_when_visible_deck_lags() {
        use sts_core::content::cards::BARRICADE_ID;

        let numeric_seed = 1_094_946_230_504_461_238;
        let external_seed = test_seed_string_from_long(numeric_seed);
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 0")
            .expect("session-58 slot 0 option");
        assert_eq!(option.reward, NeowRewardType::OneRandomRareCard);

        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_first_map_choices(&external_seed)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow random rare card reward"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| transition.action_step == 4 && transition.label == "Neow leave"));
        let carried = report
            .seed_start
            .as_ref()
            .and_then(|seed_start| seed_start.sim_run_state.as_ref())
            .expect("seed-start carries simulator state after Neow leave");
        assert!(deck_content_keys(&carried.deck)
            .iter()
            .any(|key| key == deck_content_key(BARRICADE_ID)));
    }

    #[test]
    fn seed_start_neow_rare_colorless_reward_uses_colorless_helper() {
        let (numeric_seed, option) = (1_i64..10_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::RandomColorlessTwo
                            && seed_start_neow_drawback_is_simple(option.drawback)
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with rare colorless option");

        assert!(seed_start_neow_option_is_supported_card_reward(
            option.clone()
        ));
        assert_eq!(
            seed_start_neow_card_reward_label(option.reward),
            "Neow rare colorless reward choices"
        );

        let generated = generate_neow_colorless_reward(numeric_seed, option.reward);
        assert_eq!(
            seed_start_neow_card_reward_content_ids(numeric_seed, &option, None),
            generated.cards
        );
        assert_eq!(
            seed_start_neow_card_reward_ids(numeric_seed, &option, None),
            generated
                .cards
                .iter()
                .map(|content_id| content_key(*content_id).to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn seed_start_neow_random_colorless_uses_generated_card_reward_helper() {
        let generated =
            generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorless);
        let option = GeneratedNeowOption {
            slot: 0,
            drawback: NeowDrawback::None,
            reward: NeowRewardType::RandomColorless,
            label: "choose a colorless card to obtain".to_owned(),
        };

        assert_eq!(
            seed_start_neow_card_reward_content_ids(22_079_335_079, &option, None),
            generated.cards
        );
        assert_eq!(
            generated
                .cards
                .iter()
                .map(|content_id| content_key(*content_id).to_owned())
                .collect::<Vec<_>>(),
            seed_start_neow_card_reward_ids(22_079_335_079, &option, None)
        );
    }

    #[test]
    fn seed_start_neow_curse_rare_colorless_delays_curse_until_after_choices() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::RandomColorlessTwo
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with curse plus rare colorless");

        assert!(seed_start_neow_option_is_supported_card_reward(
            option.clone()
        ));

        let run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let generated = generate_neow_colorless_reward(numeric_seed, option.reward);
        let choices = seed_start_neow_card_reward_content_ids(numeric_seed, &option, Some(&run));
        let delayed_curse =
            seed_start_neow_curse_deck_key(numeric_seed, generated.card_rng_counter)
                .expect("delayed curse");

        assert_eq!(run.card_rng_counter, 0);
        assert_eq!(run.deck.len(), ironclad_starter_deck_keys().len());
        assert_eq!(choices, generated.cards);
        assert!(content_id_from_key(&delayed_curse)
            .is_some_and(sts_core::content::cards::is_curse_content_id));
    }

    #[test]
    fn seed_start_neow_curse_rare_relic_carries_curse_deck_update() {
        let (numeric_seed, option, run) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::OneRareRelic
                    })
                    .and_then(|option| {
                        let run = seed_start_apply_neow_relic_reward(
                            seed,
                            &ironclad_starter_deck_keys(),
                            &option,
                        );
                        (seed_start_newest_trace_relic_name(&run) != "Unknown Relic")
                            .then_some((seed, option, run))
                    })
            })
            .expect("synthetic seed with curse plus mapped rare relic");

        assert!(seed_start_neow_option_is_supported_relic_reward(
            option.clone()
        ));

        let deck_ids = deck_content_keys(&run.deck);

        assert_eq!(
            seed_start_selected_neow_option(numeric_seed, &format!("CHOOSE {}", option.slot)),
            Some(option)
        );
        assert!(run.card_rng_counter <= 1);
        assert_eq!(deck_ids.len(), ironclad_starter_deck_keys().len() + 1);
        assert!(matches!(
            deck_ids.last().map(String::as_str),
            Some(
                "Clumsy"
                    | "Decay"
                    | "Doubt"
                    | "Injury"
                    | "Normality"
                    | "Pain"
                    | "Parasite"
                    | "Regret"
                    | "Shame"
                    | "Writhe"
            )
        ));
        assert_ne!(seed_start_newest_trace_relic_name(&run), "Unknown Relic");
    }

    #[test]
    fn seed_start_neow_card_reward_pick_uses_generated_choices() {
        let choices = Some(vec![
            "Twin Strike".to_owned(),
            "Heavy Blade".to_owned(),
            "Intimidate".to_owned(),
        ]);

        assert_eq!(
            seed_start_pick_neow_card_reward(&choices, "CHOOSE 1"),
            Some("Heavy Blade".to_owned())
        );
        assert_eq!(seed_start_pick_neow_card_reward(&choices, "CHOOSE 9"), None);
        assert_eq!(seed_start_pick_neow_card_reward(&None, "CHOOSE 0"), None);
    }

    #[test]
    fn seed_start_neow_boss_swap_uses_core_helper_and_removes_burning_blood() {
        let option = seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 3")
            .expect("boss swap option");

        assert!(seed_start_neow_option_is_supported_boss_swap(option));

        let run = seed_start_apply_neow_boss_swap(1_957_307_888_551, &ironclad_starter_deck_keys());
        let relic_ids = seed_start_boss_swap_relic_ids(&run);

        assert!(!relic_ids.contains(&"Burning Blood".to_owned()));
        assert_eq!(relic_ids.len(), 1);
        assert_ne!(relic_ids[0], "Unknown Relic");
        assert!(seed_start_unsupported_boss_swap_reason(&run).is_none());
    }

    #[test]
    fn seed_start_neow_boss_swap_classifies_grid_opening_relics() {
        let mut run = RunState::map_fixture();
        open_neow_reward_grid(&mut run, NeowRewardType::RemoveCard);

        let reason = seed_start_unsupported_boss_swap_reason(&run)
            .expect("grid-opening boss relics are caveated");

        assert!(reason.contains("grid-opening boss relic"));
    }

    #[test]
    fn seed_start_neow_boss_swap_trace_branch_reaches_leave() {
        let numeric_seed = 1_957_307_888_551;
        let deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let swapped_run =
            seed_start_apply_neow_boss_swap(numeric_seed, &ironclad_starter_deck_keys());
        let swapped_relics: Vec<_> = seed_start_boss_swap_relic_ids(&swapped_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 VERIFY01"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": swapped_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap"
        }));
    }

    #[test]
    fn seed_start_boss_swap_tiny_house_reward_screen_opens_and_skips_card_reward() {
        let (numeric_seed, tiny_house_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_tiny_house_reward(run))
            .expect("synthetic seed with Tiny House boss swap");
        let external_seed = test_seed_string_from_long(numeric_seed);
        let option =
            seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");
        assert_eq!(option.reward, NeowRewardType::BossRelic);

        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let tiny_house_deck = starting_deck.clone();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let tiny_house_relics: Vec<_> = seed_start_boss_swap_relic_ids(&tiny_house_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let mut card_reward_run = tiny_house_run.clone();
        card_reward_run =
            apply_run_action(&card_reward_run, RunAction::OpenCardReward).expect("open cards");
        let reward_cards: Vec<_> = card_reward_run
            .reward
            .as_ref()
            .expect("card reward")
            .choices
            .iter()
            .map(|card| {
                json!({
                    "id": reward_card_display_key(&card_reward_run, card.content_id),
                    "name": reward_card_display_key(&card_reward_run, card.content_id),
                })
            })
            .collect();
        let reward_choice_names: Vec<_> = card_reward_run
            .reward
            .as_ref()
            .expect("card reward")
            .choices
            .iter()
            .map(|card| {
                reward_card_display_key(&card_reward_run, card.content_id).to_ascii_lowercase()
            })
            .collect();

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": format!("CHOOSE {}", option.slot)}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["gold", "potion", "card"],
                "screen_state": {
                    "rewards": [
                        {"reward_type": "GOLD", "gold": 50},
                        {"reward_type": "POTION"},
                        {"reward_type": "CARD"}
                    ]
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": reward_choice_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 5, "command": "SKIP"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["gold", "potion", "card"],
                "screen_state": {
                    "rewards": [
                        {"reward_type": "GOLD", "gold": 50},
                        {"reward_type": "POTION"},
                        {"reward_type": "CARD"}
                    ]
                }
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Tiny House reward"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "card reward"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_boss_swap_tiny_house_reward_screen_can_pick_card_reward() {
        let (numeric_seed, tiny_house_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_tiny_house_reward(run))
            .expect("synthetic seed with Tiny House boss swap");
        let external_seed = test_seed_string_from_long(numeric_seed);
        let option =
            seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");
        assert_eq!(option.reward, NeowRewardType::BossRelic);

        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let tiny_house_deck = starting_deck.clone();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let tiny_house_relics: Vec<_> = seed_start_boss_swap_relic_ids(&tiny_house_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let mut card_reward_run = tiny_house_run.clone();
        card_reward_run =
            apply_run_action(&card_reward_run, RunAction::OpenCardReward).expect("open cards");
        let reward = card_reward_run.reward.as_ref().expect("card reward");
        let reward_cards: Vec<_> = reward
            .choices
            .iter()
            .map(|card| {
                json!({
                    "id": reward_card_display_key(&card_reward_run, card.content_id),
                    "name": reward_card_display_key(&card_reward_run, card.content_id),
                })
            })
            .collect();
        let reward_choice_names: Vec<_> = reward
            .choices
            .iter()
            .map(|card| {
                reward_card_display_key(&card_reward_run, card.content_id).to_ascii_lowercase()
            })
            .collect();
        let selected_card_key =
            reward_card_display_key(&card_reward_run, reward.choices[1].content_id);
        let mut settled_deck = tiny_house_deck.clone();
        settled_deck.push(json!({ "id": selected_card_key }));
        let mut lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": format!("CHOOSE {}", option.slot)}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["gold", "potion", "card"],
                "screen_state": {
                    "rewards": [
                        {"reward_type": "GOLD", "gold": 50},
                        {"reward_type": "POTION"},
                        {"reward_type": "CARD"}
                    ]
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": reward_choice_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["gold", "potion"],
                "screen_state": {
                    "rewards": [
                        {"reward_type": "GOLD", "gold": 50},
                        {"reward_type": "POTION"}
                    ]
                }
            }}}),
        ];

        let truncated_content = lines
            .iter()
            .map(|line| serde_json::to_string(line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");
        let truncated_report = verify_seed_start_communication_mod_trace(&truncated_content)
            .expect("truncated seed-start");
        assert!(truncated_report.unexpected_diffs.is_empty());
        assert!(!truncated_report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "card reward pick 1"
        }));
        assert_eq!(
            truncated_report
                .action_integrity
                .as_ref()
                .expect("truncated action integrity")
                .unresolved_transient_assertions,
            1
        );

        lines.extend([
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold + 50,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": settled_deck,
                "relics": tiny_house_relics,
                "choice_list": ["potion"],
                "screen_state": {
                    "rewards": [
                        {"reward_type": "POTION"}
                    ]
                }
            }}}),
        ]);
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Tiny House reward"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "card reward"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "card reward pick 1"
        }));
        let card_pick_disposition = report
            .action_dispositions
            .iter()
            .find(|disposition| disposition.action_step == 5)
            .expect("card pick disposition");
        assert!(card_pick_disposition.deferred_assertion_reconciled);
        assert_eq!(
            report
                .action_integrity
                .as_ref()
                .expect("action integrity")
                .unresolved_transient_assertions,
            0
        );
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_codex04_neow_potion_reward_uses_generated_potions() {
        let deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec![json!({ "name": "Burning Blood" })];
        let choices = seed_start_neow_choices(22_079_335_079);
        let potions: Vec<_> = seed_start_neow_potion_names(22_079_335_079)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 CODEX04"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "choice_list": choices
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "potions": potions,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow three potion reward"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn m33_selected_clean_neow_traces_reach_expected_labels_without_unexpected_diffs() {
        let mut failures = Vec::new();
        for case in [
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T15-36-33-694Z.jsonl",
                seed: "4",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow upgrade grid",
                    "Neow grid select",
                    "Neow grid confirm",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T15-54-59-219Z.jsonl",
                seed: "4",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow remove two grid",
                    "Neow grid select",
                    "Neow grid confirm",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T15-56-34-404Z.jsonl",
                seed: "8",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare card reward choices",
                    "Neow colorless pickup",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T16-21-32-031Z.jsonl",
                seed: "1",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare colorless reward choices",
                    "Neow colorless pickup",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T16-53-08-900Z.jsonl",
                seed: "7",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare relic",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-28-45-416Z.jsonl",
                seed: "1",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow three potion reward",
                    "Neow potion reward pick 1",
                    "Neow potion reward pick 2",
                    "Neow potion reward pick 3",
                    "Neow potion reward proceed",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-38-10-461Z.jsonl",
                seed: "11",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow curse immediate reward",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-39-56-094Z.jsonl",
                seed: "P",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare relic",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-51-15-391Z.jsonl",
                seed: "C",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare colorless reward choices",
                    "Neow colorless pickup",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-53-36-873Z.jsonl",
                seed: "1B",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow transform two grid",
                    "Neow grid select",
                    "Neow grid confirm",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-59-19-268Z.jsonl",
                seed: "2",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow curse immediate reward",
                    "Neow leave",
                ],
            },
        ] {
            let Some(content) = crate::load_corpus_file(case.path) else {
                eprintln!("skipping missing selected M33 Neow trace: {}", case.path);
                continue;
            };

            let report =
                verify_seed_start_communication_mod_trace(&content).expect("seed-start report");

            if report.mode != VerificationMode::SeedStart {
                failures.push(format!("{} wrong mode: {:?}", case.path, report.mode));
            }
            if !report.unexpected_diffs.is_empty() {
                failures.push(format!(
                    "{} unexpected diffs: {:?}",
                    case.path, report.unexpected_diffs
                ));
            }

            let seed_start = report.seed_start.as_ref().expect("seed-start details");
            if seed_start.start_command.external_seed != case.seed {
                failures.push(format!(
                    "{} wrong seed: expected {}, got {}",
                    case.path, case.seed, seed_start.start_command.external_seed
                ));
            }
            if seed_start.first_boundary.category != "none" {
                failures.push(format!(
                    "{} wrong boundary: {:?}",
                    case.path, seed_start.first_boundary
                ));
            }

            let labels: Vec<_> = report
                .verified
                .iter()
                .map(|step| step.label.as_str())
                .collect();
            for expected in case.expected_labels {
                if !labels.contains(expected) {
                    failures.push(format!(
                        "{} missing verified seed-start label {expected}; labels: {labels:?}",
                        case.path
                    ));
                }
            }
        }

        assert!(
            failures.is_empty(),
            "selected M33 Neow trace regressions:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn unsupported_combat_command_reason_names_unmapped_cards() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "hand": [{"id": "Meteor Strike", "name": "Meteor Strike"}]
                }
            }
        });
        let reason =
            unsupported_combat_command_reason(&message, "PLAY 1").expect("unmapped card reason");
        assert!(reason.contains("Meteor Strike"));
        assert!(reason.contains("not mapped"));
    }

    #[test]
    fn seed_start_allows_sword_boomerang_with_one_living_enemy() {
        let combat = sword_boomerang_combat(1);

        assert_eq!(
            unsupported_seed_start_combat_command(&combat, "PLAY 1"),
            None
        );
    }

    #[test]
    fn seed_start_allows_multi_enemy_sword_boomerang() {
        let combat = sword_boomerang_combat(2);

        assert_eq!(
            unsupported_seed_start_combat_command(&combat, "PLAY 1"),
            None
        );
    }

    #[test]
    fn observed_combat_subset_uses_first_living_monster() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "player": {"current_hp": 70, "block": 0, "energy": 2},
                    "monsters": [
                        {"current_hp": 0, "block": 0, "intent": "ATTACK", "move_base_damage": 5},
                        {"current_hp": 24, "block": 3, "intent": "ATTACK", "move_base_damage": 7}
                    ]
                }
            }
        });
        let subset = observed_combat_subset(&message, &["monster_hp", "monster_block"]);
        assert_eq!(subset["monster_hp"], 24);
        assert_eq!(subset["monster_block"], 3);
    }

    #[test]
    fn unsupported_monster_ai_reason_names_monster_groups() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "monsters": [
                        {"id": "SpikeSlime_S", "current_hp": 0},
                        {"id": "AcidSlime_M", "current_hp": 24}
                    ]
                }
            }
        });
        let reason = unsupported_monster_ai_reason(&message).expect("unsupported slime AI");
        assert!(reason.contains("AcidSlime_M"));
        assert!(reason.contains("monster group"));
    }

    #[test]
    fn choose_index_parses_nonzero_reward_choice() {
        assert_eq!(choose_index("CHOOSE 2"), Some(2));
    }

    #[test]
    fn seed_start_map_choice_resolves_nonzero_choice_index() {
        assert_eq!(command_choose_index("CHOOSE 1"), Some(1));
        assert_eq!(seed_start_map_pick_x("CODEX04", &[], "CHOOSE 1"), 2);
    }

    fn sword_boomerang_combat(living_monsters: usize) -> CombatState {
        let mut combat = CombatState::initial_fixture();
        combat.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            sts_core::content::cards::SWORD_BOOMERANG_ID,
        )];
        while combat.monsters.len() < living_monsters {
            let mut monster = combat.monsters[0].clone();
            monster.id = MonsterId::new(combat.monsters.len() as u64 + 1);
            combat.monsters.push(monster);
        }
        combat
    }

    struct SelectedNeowTraceCase {
        path: &'static str,
        seed: &'static str,
        expected_labels: &'static [&'static str],
    }

    fn assert_selected_trace_first_combat_opening_is_seed_derived(path: &str, label: &str) {
        let Some(content) = crate::load_corpus_file(path) else {
            eprintln!("skipping missing selected M34 trace: {path}");
            return;
        };
        let trace = import_communication_mod_trace(&content).expect("trace imports");
        let start = trace
            .lines
            .iter()
            .filter_map(|line| match line {
                TraceLine::Action(action) => parse_start_command(action).and_then(Result::ok),
                _ => None,
            })
            .next()
            .expect("trace has START command");
        let transitions = trace_transitions(&trace.lines).expect("trace transitions");
        let (_, _, post) = transitions
            .transitions
            .iter()
            .find(|(_, _, post)| {
                post.message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .is_some()
            })
            .unwrap_or_else(|| panic!("{label} trace has no combat entry"));
        let game = post.message.get("game_state").expect("game_state");
        let floor = game.get("floor").and_then(Value::as_u64).unwrap_or(1) as i64;
        let deck = card_instances_from_array(game.get("deck"), 1);
        let mut shuffle_rng = StsRng::new(start.numeric_seed + floor);
        let mut card_random_rng = None;
        let simulated =
            initialize_combat_piles_with_relics(&deck, &mut shuffle_rng, &mut card_random_rng, &[]);

        assert!(
            seed_start_opening_piles_match(&simulated, &post.message),
            "{label} opening piles were not seed-derived from current deck ordering; observed hand={:?} draw={:?}, simulated hand={:?} draw={:?}",
            combat_card_ids(
                post.message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .and_then(|combat| combat.get("hand"))
            ),
            combat_card_ids(
                post.message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .and_then(|combat| combat.get("draw_pile"))
            ),
            simulated
                .hand
                .iter()
                .map(|card| content_key(card.content_id))
                .collect::<Vec<_>>(),
            simulated
                .draw_pile
                .iter()
                .map(|card| content_key(card.content_id))
                .collect::<Vec<_>>()
        );
    }

    fn test_seed_string_from_long(mut seed: i64) -> String {
        const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
        if seed == 0 {
            return "0".to_owned();
        }
        let mut out = Vec::new();
        while seed > 0 {
            out.push(ALPHABET[(seed % 35) as usize] as char);
            seed /= 35;
        }
        out.iter().rev().collect()
    }
}
