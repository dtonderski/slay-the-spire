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
    target_beyond_act_three_boss_kind_with_unlocks, target_exordium_act_one_boss_kind_with_unlocks,
    target_exordium_act_one_boss_with_unlocks, BossUnlockState,
};
use sts_core::content::monsters::target_move_byte_for_monster;
use sts_core::potion::Potion;
use sts_core::run::event::{neow_screen_for_stage, VAMPIRES_BITE_COUNT};
use sts_core::run::neow::{
    apply_neow_curse_drawback, apply_neow_lament_reward,
    generate_neow_colorless_reward_with_card_rng_counter,
};
use sts_core::{
    affordable_shop_picks, apply_neow_boss_swap, apply_neow_relic_reward,
    apply_neow_simple_drawback, apply_neow_simple_reward, apply_run_decision_action,
    generate_exordium_map_topology, generate_neow_card_reward, generate_neow_colorless_reward,
    generate_neow_options, generate_neow_transform_reward, legal_run_decision_actions,
    open_neow_reward_grid, shop_action_for_choice_index, CardGridScreen, CardId, CardInstance,
    CombatAction, CombatDecisionState, CombatPhase, CombatState, ContentId, Event, EventAction,
    GeneratedNeowOption, GridPurpose, MapAction, MonsterId, MonsterIntent, MonsterState,
    NeowDrawback, NeowRewardType, Relic, RelicKey, RestAction, RewardScreen, RoomKind, RunAction,
    RunDecisionAction, RunPhase, RunState, ShopPick,
};

#[cfg(test)]
use sts_core::content::monsters::{
    looter_theft, target_beyond_encounter_spawn_for_key,
    target_city_normal_encounter_spawn_at_combat_index,
    target_normal_encounter_spawn_at_combat_index, TargetEncounterSpawn, TargetSpawnPower,
    GREMLIN_NOB_ID, GUARDIAN_CHARGE_BLOCK, GUARDIAN_ID, LOOTER_ID, MUGGER_ID, SLAVER_RED_ID,
    TASKMASTER_ID,
};
#[cfg(test)]
use sts_core::{
    city_room_kinds_on_path, enter_normal_combat_reward_screen, event_screen,
    exordium_room_kinds_on_path, generate_neow_three_potions, initialize_combat_piles_with_relics,
    CardPiles, EventChoice, EventScreen, MonsterPowers, PlayerPowers, StsRng,
};
#[cfg(test)]
use sts_core::{target_room_kinds_on_path, TargetMapAct};

mod neow;
mod replay;
use neow::*;
use replay::verify_seed_start_transitions;

fn apply_combat_action_on_run(
    run: &RunState,
    action: CombatAction,
) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Combat(action))
}

fn apply_event_action(run: &RunState, action: EventAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Event(action))
}

fn apply_map_action_on_run(run: &RunState, action: MapAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Map(action))
}

fn apply_rest_action(run: &RunState, action: RestAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Rest(action))
}

fn apply_run_action(run: &RunState, action: RunAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Run(action))
}

fn select_grid_card(run: &RunState, index: usize) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::GridSelect { index })
}

fn confirm_grid(run: &RunState) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::GridConfirm)
}

fn cancel_grid(run: &RunState) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::GridCancel)
}

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
    PendingTransient,
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
    fn starting_hp(&self) -> i32 {
        self.verification_starting_hp.unwrap_or(80)
    }

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
    verify_seed_start_trace(content)
}

fn verify_seed_start_trace(content: &str) -> Result<SimRealReport, SimRealError> {
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
        total_actions,
        ignored_tail_actions: transitions.ignored_tail_actions,
        action_dispositions: Vec::new(),
        action_integrity: None,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let verification =
        verify_seed_start_transitions(&transitions.transitions, &start, &mut report, boss_unlocks);
    let failed = verification.boundary.category != "none";
    report.seed_start = Some(SeedStartReport {
        start_command: start,
        failed,
        first_boundary: verification.boundary,
        sim_run_state: verification.final_run_state,
    });
    let (action_dispositions, action_integrity) = build_action_accounting(
        &trace.lines,
        &transitions,
        &report,
        &verification.reconciled_deferred_action_steps,
        &verification.unresolved_deferred_action_steps,
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
            TraceLine::Metadata(_)
            | TraceLine::CommandAccept(_)
            | TraceLine::Response(_)
            | TraceLine::SlayTheData(_)
            | TraceLine::Automation(_)
            | TraceLine::CommandObservedTimeout(_) => {}
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
    semantic_pending_action_steps: &[u32],
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
    let mut pending = std::collections::HashSet::new();
    for (transition_index, ordinal) in transitions
        .transition_action_ordinals
        .iter()
        .copied()
        .enumerate()
    {
        if semantic_pending_action_steps.contains(&transitions.transitions[transition_index].1.step)
        {
            pending.insert(ordinal);
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
        } else if pending.contains(&ordinal) {
            (
                ActionDispositionKind::PendingTransient,
                Some("deferred assertion has no stable reconciliation frame".to_owned()),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingNeowCurseOrder {
    BeforePickedCard,
    AfterPickedCard,
}

fn pending_neow_curse_order(
    pre: &TraceState,
    action: &TraceAction,
) -> Result<PendingNeowCurseOrder, &'static str> {
    // NeowReward.update can queue the pending curse on the first target update
    // after the card reward opens. Recorded commands delayed by at least 100 ms
    // have crossed an update opportunity even on a heavily loaded 10 FPS target.
    // Traces without timestamps represent synchronous dispatch from their source
    // state and retain the picked-card-before-curse ordering.
    const TARGET_UPDATE_OPPORTUNITY_MILLIS: i64 = 100;

    match (pre.received_at.as_deref(), action.sent_at.as_deref()) {
        (Some(received), Some(sent)) => {
            let received = trace_timestamp_millis(received)
                .ok_or("invalid source-state received_at timestamp")?;
            let sent = trace_timestamp_millis(sent).ok_or("invalid action sent_at timestamp")?;
            let delay = sent
                .checked_sub(received)
                .filter(|delay| *delay >= 0)
                .ok_or("action sent_at precedes source-state received_at")?;
            Ok(if delay >= TARGET_UPDATE_OPPORTUNITY_MILLIS {
                PendingNeowCurseOrder::BeforePickedCard
            } else {
                PendingNeowCurseOrder::AfterPickedCard
            })
        }
        _ => Ok(PendingNeowCurseOrder::AfterPickedCard),
    }
}

fn trace_timestamp_millis(timestamp: &str) -> Option<i64> {
    let timestamp = timestamp.strip_suffix('Z')?;
    let (date, time) = timestamp.split_once('T')?;
    let mut date = date.split('-');
    let year = date.next()?.parse::<i64>().ok()?;
    let month = date.next()?.parse::<i64>().ok()?;
    let day = date.next()?.parse::<i64>().ok()?;
    if date.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    let days_in_month = match month {
        2 if year % 400 == 0 || year % 4 == 0 && year % 100 != 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }

    let mut time = time.split(':');
    let hour = time.next()?.parse::<i64>().ok()?;
    let minute = time.next()?.parse::<i64>().ok()?;
    let second_and_fraction = time.next()?;
    if time.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    let (second, fraction) = match second_and_fraction.split_once('.') {
        Some((_, "")) => return None,
        Some(parts) => parts,
        None => (second_and_fraction, ""),
    };
    let second = second.parse::<i64>().ok()?;
    if second > 59 || !fraction.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let mut millis = 0i64;
    for (index, digit) in fraction.bytes().take(3).enumerate() {
        millis += i64::from(digit - b'0') * 10i64.pow(2 - index as u32);
    }

    // Howard Hinnant's civil-date conversion, with 1970-01-01 as day zero.
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days_since_epoch = era * 146_097 + day_of_era - 719_468;
    days_since_epoch
        .checked_mul(86_400_000)?
        .checked_add(hour * 3_600_000 + minute * 60_000 + second * 1_000 + millis)
}

struct SeedStartVerification {
    boundary: SeedStartBoundary,
    final_run_state: Option<RunState>,
    reconciled_deferred_action_steps: Vec<u32>,
    unresolved_deferred_action_steps: Vec<u32>,
    unresolved_transient_assertions: usize,
}

struct PendingDeckAssertion {
    action: TraceAction,
    label: String,
    related_actions: Vec<(TraceAction, String)>,
    transient_decks: Vec<Vec<String>>,
    expected_deck: Vec<String>,
}

struct PendingMapAssertion {
    action: TraceAction,
    label: String,
    simulated_map: Value,
    transient_matches: bool,
}

struct PendingBossRelicOverlayAssertion {
    action: TraceAction,
    simulated_overlay: Value,
    transient_matches: bool,
}

struct PendingCombatTransition {
    action: TraceAction,
    label: String,
    transient_matches: bool,
}

#[derive(Default)]
struct PendingCombatAssertion {
    transitions: Vec<PendingCombatTransition>,
    requires_stable_frame_before_next_command: bool,
    failed_reconciliation: Option<SeedStartBoundary>,
}

enum SmokeBombUiState {
    Escaping {
        source: Box<RunState>,
        action: TraceAction,
        pending_commands: Vec<TraceAction>,
        transient_matches: bool,
    },
    Reward {
        pending_proceeds: Vec<TraceAction>,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum PendingDeckObservation {
    Settled,
    Deferred,
    Diverged(Vec<String>),
}

fn seed_start_finish_boundary(
    seed_sim: &Option<RunState>,
    boundary: SeedStartBoundary,
    numeric_seed: i64,
    boss_unlocks: BossUnlockState,
    reconciled_deferred_action_steps: Vec<u32>,
    unresolved_deferred_action_steps: Vec<u32>,
) -> SeedStartVerification {
    let mut final_run_state = seed_sim.clone();
    if let Some(run) = final_run_state.as_mut() {
        seed_start_apply_boss_unlocks(run, numeric_seed, boss_unlocks);
    }
    SeedStartVerification {
        boundary,
        final_run_state,
        reconciled_deferred_action_steps,
        unresolved_transient_assertions: unresolved_deferred_action_steps.len(),
        unresolved_deferred_action_steps,
    }
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

fn seed_start_apply_boss_unlocks(
    run: &mut RunState,
    numeric_seed: i64,
    boss_unlocks: BossUnlockState,
) {
    run.act1_boss = target_exordium_act_one_boss_kind_with_unlocks(numeric_seed, boss_unlocks)
        .expect("static Exordium encounter pools are valid");
    run.act3_boss = target_beyond_act_three_boss_kind_with_unlocks(numeric_seed, boss_unlocks)
        .expect("static Beyond encounter pools are valid");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartPhase {
    BeforeStart,
    BootstrapSettling,
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

fn seed_start_bootstrap_observed_subset(message: &Value) -> Value {
    let mut observed = seed_start_observed_subset(message);
    let act_boss = message
        .get("game_state")
        .and_then(|game| game.get("act_boss"))
        .cloned()
        .unwrap_or(Value::Null);
    observed
        .as_object_mut()
        .expect("seed-start observed subset is an object")
        .insert("act_boss".to_owned(), act_boss);
    observed
}

fn seed_start_bootstrap_simulated_subset(
    start: &StartRunCommand,
    boss_unlocks: BossUnlockState,
    deck_ids: &[String],
) -> Value {
    json!({
        "screen_type": "EVENT",
        "ascension": start.ascension,
        "floor": 0,
        "gold": 99,
        "current_hp": start.starting_hp(),
        "max_hp": start.starting_hp(),
        "deck_ids": deck_ids,
        "relic_ids": ["Burning Blood"],
        "choices": ["talk"],
        "act_boss": target_exordium_act_one_boss_with_unlocks(
            start.numeric_seed,
            boss_unlocks,
        ),
    })
}

fn seed_start_proceed_simulated_subset(run: &RunState) -> Value {
    json!({
        "screen_type": "NONE",
        "ascension": run.ascension as u64,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run),
        "choices": Vec::<String>::new(),
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
        "hand_ids": combat_card_ids(combat.and_then(|combat| combat.get("hand"))),
        "draw_ids": combat_card_ids(combat.and_then(|combat| combat.get("draw_pile"))),
        "discard_ids": combat_card_ids(combat.and_then(|combat| combat.get("discard_pile"))),
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
    let monster_intents_visible = observed_monster_intents_visible(game);
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

fn seed_start_neow_potion_reward_observed_subset(message: &Value) -> Value {
    let mut subset = seed_start_reward_observed_subset(message);
    seed_start_insert_observed_potion_ids(&mut subset, message);
    if let Some(fields) = subset.as_object_mut() {
        let offers = message
            .pointer("/game_state/screen_state/rewards")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|reward| reward.get("reward_type").and_then(Value::as_str) == Some("POTION"))
            .filter_map(|reward| reward.get("potion").and_then(potion_key_from_value))
            .collect::<Vec<_>>();
        fields.insert("potion_offer_ids".to_owned(), json!(offers));
    }
    subset
}

fn seed_start_neow_potion_reward_simulated_subset(run: &RunState) -> Value {
    let mut subset = seed_start_reward_simulated_subset(run);
    seed_start_insert_simulated_potion_ids(&mut subset, run);
    if let Some(fields) = subset.as_object_mut() {
        let offers = run
            .reward
            .as_ref()
            .map(|reward| {
                if reward.potion_offers.is_empty() {
                    reward.potion_offer.into_iter().collect::<Vec<_>>()
                } else {
                    reward.potion_offers.clone()
                }
            })
            .unwrap_or_default()
            .into_iter()
            .map(|potion| potion_trace_name(potion).to_owned())
            .collect::<Vec<_>>();
        fields.insert("potion_offer_ids".to_owned(), json!(offers));
    }
    subset
}

fn seed_start_insert_observed_potion_ids(subset: &mut Value, message: &Value) {
    if let Some(fields) = subset.as_object_mut() {
        fields.insert(
            "potion_ids".to_owned(),
            json!(potion_keys_from_value(
                message.pointer("/game_state/potions")
            )),
        );
    }
}

fn seed_start_insert_simulated_potion_ids(subset: &mut Value, run: &RunState) {
    if let Some(fields) = subset.as_object_mut() {
        fields.insert(
            "potion_ids".to_owned(),
            json!(run
                .potions
                .iter()
                .map(|potion| potion_trace_name(*potion).to_owned())
                .collect::<Vec<_>>()),
        );
    }
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

fn seed_start_is_candidate_boss_act_transient_frame(message: &Value) -> bool {
    screen_type(message) == Some("NONE")
}

fn seed_start_boss_act_transient_observed_subset(message: &Value) -> Value {
    json!({
        "screen_name": message.pointer("/game_state/screen_name").and_then(Value::as_str).unwrap_or(""),
        "room_type": message.pointer("/game_state/room_type").and_then(Value::as_str).unwrap_or(""),
    })
}

fn seed_start_boss_act_transient_simulated_subset() -> Value {
    json!({
        "screen_name": "NONE",
        "room_type": "TreasureRoomBoss",
    })
}

#[allow(clippy::too_many_arguments)]
fn seed_start_compare_pending_map_assertion(
    report: &mut SimRealReport,
    pending: &PendingMapAssertion,
    message: &Value,
) -> bool {
    seed_start_compare_deferred_subset(
        report,
        &pending.action,
        &pending.label,
        seed_start_map_return_observed_subset(message),
        pending.simulated_map.clone(),
    )
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

fn simulated_card_projection_key(card: &CardInstance) -> String {
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

fn seed_start_boss_relic_deck_overlay_simulated_subset(pre_pick_run: &RunState) -> Value {
    json!({
        "screen_type": "NONE",
        "floor": pre_pick_run.current_floor,
        "gold": pre_pick_run.gold,
        "current_hp": pre_pick_run.player_hp,
        "max_hp": pre_pick_run.player_max_hp,
        "deck_ids": deck_content_keys(&pre_pick_run.deck),
        "relic_ids": relic_ids_for_simulated_subset(pre_pick_run),
        "choices": Vec::<String>::new(),
    })
}

fn seed_start_reconcile_boss_relic_overlay(
    report: &mut SimRealReport,
    pending: PendingBossRelicOverlayAssertion,
    stable_matches: bool,
    stable_action_step: u32,
    reconciled_deferred_action_steps: &mut Vec<u32>,
) {
    if !pending.transient_matches {
        return;
    }
    if stable_matches {
        report.verified.push(VerifiedTransition {
            action_step: pending.action.step,
            command: pending.action.command,
            label: "boss relic reward reconciled after deck overlay".to_owned(),
        });
        reconciled_deferred_action_steps.push(pending.action.step);
    } else {
        report.unsupported.push(UnsupportedTransition {
            action_step: pending.action.step,
            command: pending.action.command,
            reason: format!(
                "boss relic deck overlay did not reconcile at stable action step {stable_action_step}"
            ),
        });
    }
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
    visible_deck_ids: Vec<String>,
) -> Value {
    let mut subset = seed_start_grid_simulated_subset(run);
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

#[allow(clippy::too_many_arguments)]
fn seed_start_handle_proceed_to_map(
    report: &mut SimRealReport,
    action: &TraceAction,
    post_message: &Value,
    phase: &mut SeedStartPhase,
    combat_index: &mut usize,
    reward_step: &mut usize,
    map_path_xs: &mut Vec<i32>,
    seed_sim: &mut Option<RunState>,
    pending_map_assertion: &mut Option<PendingMapAssertion>,
) -> Option<SeedStartBoundary> {
    let Some(sim) = seed_sim.as_ref() else {
        return Some(SeedStartBoundary {
            path: format!("$.actions[step={}].command", action.step),
            category: "unsupported_post_reward_map".to_owned(),
            reason: "proceed-to-map command without initialized deterministic replay".to_owned(),
        });
    };
    let boss_room_none = screen_type(post_message) == Some("NONE")
        && post_message
            .get("game_state")
            .and_then(|game| game.get("room_type"))
            .and_then(Value::as_str)
            == Some("TreasureRoomBoss");
    let transient_boss_act_transition =
        seed_start_is_candidate_boss_act_transient_frame(post_message);
    let ftue_open = post_message
        .get("game_state")
        .and_then(|game| game.get("screen_name"))
        .and_then(Value::as_str)
        .is_some_and(|screen| screen.eq_ignore_ascii_case("FTUE"));
    if boss_room_none && ftue_open && sim.phase == RunPhase::Reward {
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
        let label = "boss reward proceed to settled next-act map";
        let transient_matches = seed_start_compare_deferred_subset(
            report,
            action,
            label,
            seed_start_boss_act_transient_observed_subset(post_message),
            seed_start_boss_act_transient_simulated_subset(),
        );
        map_path_xs.clear();
        let simulated_map = match seed_start_simulated_map_return(
            seed_sim
                .as_ref()
                .expect("proceed-to-map transition retained core run state"),
        ) {
            Ok(projection) => projection,
            Err(reason) => {
                return Some(SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "invalid_boss_act_map_projection".to_owned(),
                    reason,
                });
            }
        };
        *pending_map_assertion = Some(PendingMapAssertion {
            action: action.clone(),
            label: label.to_owned(),
            simulated_map,
            transient_matches,
        });
        *combat_index = 0;
        *reward_step = 0;
        *phase = SeedStartPhase::Proceed;
        return None;
    }
    let label = format!("return to map after floor {}", *combat_index + 1);
    let observed = seed_start_map_return_observed_subset(post_message);
    let simulated = match seed_start_simulated_map_return(
        seed_sim
            .as_ref()
            .expect("proceed-to-map transition retained core run state"),
    ) {
        Ok(projection) => projection,
        Err(reason) => {
            return Some(SeedStartBoundary {
                path: format!("$.actions[step={}].command", action.step),
                category: "invalid_map_projection".to_owned(),
                reason,
            });
        }
    };
    compare_subset(report, action, &label, observed, simulated);
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

fn seed_start_compare_map_return(
    report: &mut SimRealReport,
    action: &TraceAction,
    post_message: &Value,
    simulated: Value,
) {
    compare_subset(
        report,
        action,
        "map return",
        seed_start_map_return_observed_subset(post_message),
        simulated,
    );
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
        "intent": spawn.intent.trace_label(),
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
        .map(|card| {
            if let Some(content_id) = content_id_from_card_value(card) {
                return json!(content_id.get());
            }
            let identity = observed_display_card_identity(card)
                .expect("trace card reward schema was validated before projection");
            let identity =
                sts_core::run::reward::any_color_reward_card_key_from_identity(&identity)
                    .map(normalize_card_identity)
                    .unwrap_or(identity);
            json!(identity)
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
                .map(str::to_owned)
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

#[cfg(test)]
fn relic_from_trace_name(name: &str) -> Option<Relic> {
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

fn seed_start_relic_ids_for_inline_projection(run: Option<&RunState>) -> Vec<String> {
    run.map(relic_ids_for_simulated_subset)
        .unwrap_or_else(|| vec![relic_key_trace_name(RelicKey::BurningBlood).to_owned()])
}

fn run_has_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relics.iter().any(|relic| relic.key() == key)
}

fn seed_start_carried_run_with_hp(
    carried: Option<&RunState>,
    numeric_seed: i64,
    ascension: u8,
    external_seed: &str,
    deck_ids: &[String],
    starting_hp: i32,
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
    let mut run =
        seed_start_seeded_idle_run_with_hp(numeric_seed, ascension, deck_ids, starting_hp);
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

fn seed_start_event_simulated_subset(run: &RunState) -> Value {
    seed_start_event_simulated_subset_with_deck(run, deck_content_keys(&run.deck))
}

fn seed_start_event_simulated_subset_with_delayed_deck_append(
    run: &RunState,
    delayed_event_deck_append_count: Option<usize>,
) -> Value {
    if run.card_grid.is_some() {
        return seed_start_event_simulated_subset(run);
    }

    let Some(count) = delayed_event_deck_append_count else {
        return seed_start_event_simulated_subset(run);
    };
    let mut visible_deck = deck_content_keys(&run.deck);
    // Live event grids publish selected cards and transform results on the
    // next state poll. Core state is already complete, so project only the
    // action frame without those newly appended cards.
    visible_deck.truncate(visible_deck.len().saturating_sub(count));
    seed_start_event_simulated_subset_with_deck(run, visible_deck)
}

fn seed_start_event_simulated_subset_with_deck(run: &RunState, deck_ids: Vec<String>) -> Value {
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
    seed_start_simulated_combat_subset_with_options(run, end_turn_snapshot)
}

fn seed_start_run_has_combat_card_reward(run: &RunState) -> bool {
    run.combat
        .as_ref()
        .is_some_and(|combat| combat.combat_card_reward_choices().is_some())
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
        | CombatDecisionState::DiscoveryCardReward { .. } => SeedStartCombatDecision::CardReward,
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

fn seed_start_simulated_map_combat_subset(run: &RunState, _normal_combat_index: usize) -> Value {
    seed_start_simulated_combat_subset_with_options(run, false)
}

fn seed_start_simulated_combat_subset_with_options(
    run: &RunState,
    end_turn_snapshot: bool,
) -> Value {
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
        "relic_ids": relic_ids_for_simulated_subset(run),
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
                    let hidden_by_hand_select = combat.hand_select().is_some_and(|hand_select| {
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
                        .exhaust_select()
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
    if reward.card_reward_is_active() {
        return reward.choices.is_empty();
    }
    reward.gold_offer == 0
        && reward.stolen_gold_offer == 0
        && reward.potion_offer.is_none()
        && reward.potion_offers.is_empty()
        && reward.relic_offer.is_none()
        && reward.pending_relic_offer.is_none()
        && reward.queued_relic_offers.is_empty()
        && !reward.card_reward_is_pending()
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
    let has_relic = reward.relic_offer.is_some();
    let has_pending_relic = reward.pending_relic_offer.is_some();
    if has_relic && has_pending_relic && reward.gold_offer > 0 {
        choices.push("relic".to_owned());
        choices.push("gold".to_owned());
        choices.push("relic".to_owned());
        return choices;
    }
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
    if !reward.potion_offers.is_empty() {
        choices.extend(std::iter::repeat_n(
            "potion".to_owned(),
            reward.potion_offers.len(),
        ));
    } else if reward.potion_offer.is_some() {
        choices.push("potion".to_owned());
    }
    if !reward.choices.is_empty() && !reward.card_reward_is_active() {
        choices.push("card".to_owned());
    } else if reward.card_reward_is_pending() {
        choices.extend(std::iter::repeat_n(
            "card".to_owned(),
            reward.remaining_card_reward_count() as usize,
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

fn seed_start_apply_reward_choose(sim: &mut RunState, command: &str) -> Result<String, String> {
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
        .is_some_and(RewardScreen::card_reward_is_active)
    {
        let card_choice_count = sim
            .reward
            .as_ref()
            .expect("card reward is active")
            .choices
            .len();
        if choose_index == card_choice_count && sim.relics.contains(&Relic::SingingBowl) {
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

    let simulated_choices = sim
        .reward
        .as_ref()
        .map(sim_reward_combat_choices)
        .ok_or_else(|| "reward screen is missing".to_owned())?;
    let choice = simulated_choices
        .get(choose_index)
        .cloned()
        .ok_or_else(|| {
            format!(
                "reward choice index {choose_index} is not available; simulated choices: {simulated_choices:?}"
            )
        })?;

    let potion_index = simulated_choices[..choose_index]
        .iter()
        .filter(|choice| choice.as_str() == "potion")
        .count();
    // The target command bridge resolves the two-item [gold, potion] frame by
    // marking the preceding gold reward when the full-belt potion is clicked.
    // The potion claim itself still fails in vanilla and remains on screen.
    let choice = if choice == "potion"
        && sim.open_potion_slots() == 0
        && choose_index == 1
        && simulated_choices == ["gold", "potion"]
    {
        "gold"
    } else {
        choice.as_str()
    };
    let next = match choice {
        "stolen_gold" => apply_run_action(sim, RunAction::TakeStolenGoldReward),
        "gold" => apply_run_action(sim, RunAction::TakeGoldReward),
        "card" => apply_run_action(sim, RunAction::OpenCardReward),
        "potion" => apply_run_action(
            sim,
            RunAction::TakePotionReward {
                index: potion_index,
            },
        ),
        "relic" => apply_run_action(sim, RunAction::TakeRelicReward),
        _ => return Err(format!("unknown reward choice {choice}")),
    }
    .map_err(|err| err.to_string())?;
    *sim = next;
    Ok(format!("{choice} reward"))
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
    let combat_choices = reward.map(sim_reward_combat_choices).unwrap_or_default();
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
) {
    let mut expected = seed_start_normalize_combat_compare(expected);
    let mut actual = seed_start_normalize_combat_compare(actual);
    apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);
    compare_subset(report, action, label, expected, actual);
}

fn seed_start_compare_deferred_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
) -> bool {
    let mut expected = seed_start_normalize_combat_compare(expected);
    let mut actual = seed_start_normalize_combat_compare(actual);
    apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);
    seed_start_compare_deferred_subset(report, action, label, expected, actual)
}

fn seed_start_combat_subsets_match(mut expected: Value, mut actual: Value) -> bool {
    expected = seed_start_normalize_combat_compare(expected);
    actual = seed_start_normalize_combat_compare(actual);
    apply_observed_debug_intent_visibility_contract(&mut expected, &mut actual);
    subset_diffs(expected, actual).is_empty()
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

fn seed_start_is_stable_combat_decision_frame(message: &Value) -> bool {
    let Some(game) = message.get("game_state") else {
        return false;
    };
    let screen_type = game.get("screen_type").and_then(Value::as_str);
    matches!(screen_type, Some("GRID" | "HAND_SELECT"))
        && game.get("action_phase").and_then(Value::as_str) == Some("EXECUTING_ACTIONS")
        && game.get("current_action").is_some()
        && message.get("ready_for_command").and_then(Value::as_bool) == Some(true)
        && message
            .get("available_commands")
            .and_then(Value::as_array)
            .is_some_and(|commands| {
                commands
                    .iter()
                    .any(|command| matches!(command.as_str(), Some("choose" | "confirm")))
            })
}

fn seed_start_is_transient_combat_entry_post_state(message: &Value) -> bool {
    let Some(game) = message.get("game_state") else {
        return false;
    };
    game.get("combat_state").is_some()
        && game.get("action_phase").and_then(Value::as_str) == Some("EXECUTING_ACTIONS")
        && game.get("current_action").is_some()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CopiedAttackExpectation {
    remaining_double_tap: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CopiedAttackFrame {
    Stable,
    Deferred,
    Diverged,
}

fn seed_start_copied_attack_expectation(
    pre_combat: &CombatState,
    action: CombatAction,
) -> Option<CopiedAttackExpectation> {
    let CombatAction::PlayCard { card_id, .. } = action else {
        return None;
    };
    if pre_combat.double_tap_pending <= 0 {
        return None;
    }
    let card = pre_combat
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)?;
    let definition = sts_core::content::cards::get_card_definition(card.content_id)?;
    if definition.card_type != CardType::Attack {
        return None;
    }

    Some(CopiedAttackExpectation {
        remaining_double_tap: i64::from(pre_combat.double_tap_pending.saturating_sub(1)),
    })
}

fn seed_start_classify_copied_attack_frame(
    stable_projection_matches: bool,
    expectation: Option<CopiedAttackExpectation>,
    post_message: &Value,
) -> CopiedAttackFrame {
    if stable_projection_matches {
        return CopiedAttackFrame::Stable;
    }
    if expectation.is_some_and(|expectation| {
        seed_start_observed_double_tap_matches(post_message, expectation.remaining_double_tap)
    }) {
        CopiedAttackFrame::Deferred
    } else {
        CopiedAttackFrame::Diverged
    }
}

fn seed_start_observed_double_tap_matches(post_message: &Value, expected: i64) -> bool {
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    if game.get("screen_type").and_then(Value::as_str) != Some("NONE")
        || game.get("action_phase").and_then(Value::as_str) != Some("WAITING_ON_USER")
        || game.get("current_action").is_some()
    {
        return false;
    }
    let Some(powers) = game
        .pointer("/combat_state/player/powers")
        .and_then(Value::as_array)
    else {
        return false;
    };
    let observed = powers
        .iter()
        .find(|power| power.get("id").and_then(Value::as_str) == Some("Double Tap"));
    let observed = match observed {
        Some(power) => match power.get("amount").and_then(Value::as_i64) {
            Some(amount) => amount,
            None => return false,
        },
        None => 0,
    };

    observed == expected
}

fn seed_start_compare_transient_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    mut expected: Value,
    mut actual: Value,
) -> bool {
    let defer_queued_card_reward_ui = expected.get("screen_type").and_then(Value::as_str)
        == Some("HAND_SELECT")
        && actual.get("screen_type").and_then(Value::as_str) == Some("CARD_REWARD");
    for value in [&mut expected, &mut actual] {
        if let Some(object) = value.as_object_mut() {
            for key in [
                "current_hp",
                "combat_player_hp",
                "combat_player_block",
                "combat_player_energy",
                "hand_ids",
                "draw_ids",
                "discard_ids",
                "monsters",
            ] {
                object.remove(key);
            }
            if defer_queued_card_reward_ui {
                object.remove("screen_type");
                object.remove("card_reward_ids");
            }
        }
    }
    seed_start_compare_deferred_combat_subset(report, action, label, expected, actual)
}

fn seed_start_compare_or_defer_combat_entry(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    post_message: &Value,
    observed: Value,
    simulated: Value,
    pending_combat_assertion: &mut Option<PendingCombatAssertion>,
) {
    if seed_start_is_transient_combat_entry_post_state(post_message) {
        let transient_matches =
            seed_start_compare_transient_combat_subset(report, action, label, observed, simulated);
        pending_combat_assertion
            .get_or_insert_default()
            .transitions
            .push(PendingCombatTransition {
                action: action.clone(),
                label: label.to_owned(),
                transient_matches,
            });
        return;
    }
    seed_start_compare_combat_subset(report, action, label, observed, simulated);
}

#[allow(clippy::too_many_arguments)]
fn seed_start_compare_or_defer_combat_transition(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    post_message: &Value,
    observed: Value,
    simulated: Value,
    pending_combat_assertion: &mut Option<PendingCombatAssertion>,
    reconciled_deferred_action_steps: &mut Vec<u32>,
) {
    let candidate_transient = seed_start_is_transient_combat_post_state(post_message)
        || pending_combat_assertion.is_some()
            && seed_start_is_transient_combat_entry_post_state(post_message);
    if candidate_transient && !seed_start_combat_subsets_match(observed.clone(), simulated.clone())
    {
        let transient_matches =
            seed_start_compare_transient_combat_subset(report, action, label, observed, simulated);
        pending_combat_assertion
            .get_or_insert_default()
            .transitions
            .push(PendingCombatTransition {
                action: action.clone(),
                label: label.to_owned(),
                transient_matches,
            });
        return;
    }

    let diff_count = report.unexpected_diffs.len();
    seed_start_compare_combat_subset(report, action, label, observed, simulated);
    let stable_matches = report.unexpected_diffs.len() == diff_count;
    let Some(mut pending) = pending_combat_assertion.take() else {
        return;
    };
    if stable_matches {
        for transition in pending.transitions {
            if !transition.transient_matches {
                continue;
            }
            report.verified.push(VerifiedTransition {
                action_step: transition.action.step,
                command: transition.action.command,
                label: transition.label,
            });
            reconciled_deferred_action_steps.push(transition.action.step);
        }
        return;
    }

    let stable_diffs = report.unexpected_diffs.split_off(diff_count);
    let reason = stable_diffs
        .iter()
        .flat_map(|diff| {
            diff.diffs
                .iter()
                .map(move |detail| format!("{}: {detail}", diff.label))
        })
        .collect::<Vec<_>>()
        .join("; ");
    pending.failed_reconciliation = Some(SeedStartBoundary {
        path: format!("$.actions[step={}].command", action.step),
        category: "unreconciled_combat_frame".to_owned(),
        reason,
    });
    *pending_combat_assertion = Some(pending);
}

fn seed_start_normalize_combat_compare(mut value: Value) -> Value {
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    let player_is_dead = obj
        .get("combat_player_hp")
        .and_then(Value::as_i64)
        .is_some_and(|hp| hp <= 0);
    obj.remove("unobservable");
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

#[cfg(test)]
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
fn card_instances_from_array(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            let content_id = content_id_from_card_value(card)?;
            let mut instance = CardInstance::new(CardId::new(base_id + index as u64), content_id);
            instance.upgrades = card_upgrade_count(card)?;
            Some(instance)
        })
        .collect()
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
    match upgrades {
        0 => Some(base),
        1 if card_content_id_is_upgraded(base) => Some(base),
        1 => upgrade_content_id(base),
        _ => None,
    }
}

fn card_content_id_is_upgraded(content_id: ContentId) -> bool {
    sts_core::content::cards::ALL_CARDS
        .iter()
        .any(|definition| definition.upgrade == Some(content_id))
}

fn observed_card_projection_key(card: &Value) -> Option<String> {
    content_id_from_card_value(card)
        .map(modeled_card_projection_key)
        .or_else(|| observed_display_card_identity(card))
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
        .map(|card| {
            observed_card_projection_key(card)
                .expect("trace deck card schema was validated before projection")
        })
        .collect()
}

fn deck_content_keys(deck: &[CardInstance]) -> Vec<String> {
    deck.iter().map(simulated_card_projection_key).collect()
}

fn seed_start_deck_with_pending_neow_curse(deck: &[String], curse: &str) -> Vec<String> {
    let mut settled = deck.to_vec();
    settled.push(curse.to_owned());
    settled
}

fn deck_content_keys_after_pending_obtain_cards_settle(run: &RunState) -> Vec<String> {
    let mut settled = run.clone();
    settled
        .flush_pending_obtain_cards()
        .expect("canonical seed-start deck has card ID allocation headroom");
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

fn classify_deferred_deck_reconciliation(
    observed: &[String],
    transient_decks: &[Vec<String>],
    settled: &[String],
) -> PendingDeckObservation {
    if observed == settled {
        PendingDeckObservation::Settled
    } else if transient_decks.iter().any(|deck| deck == observed) {
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
fn first_choice(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_str)
}

fn intent_key(monster: &MonsterState) -> String {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, BANDIT_BEAR_ID, BANDIT_LEADER_ID, BRONZE_ORB_ID, BYRD_ID, CHOSEN_ID,
        GREMLIN_WIZARD_ID, GUARDIAN_ID, HEXAGHOST_ID, LAGAVULIN_ID, RED_LOUSE_ID, SLIME_BOSS_ID,
        SNECKO_ID, SPIKER_ID, SPIKE_SLIME_ID,
    };

    match monster.intent {
        MonsterIntent::PendingAiRoll => "PENDING_AI_ROLL".to_owned(),
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

fn push_sim_unsupported(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    err: sts_core::SimError,
) -> String {
    let reason = format!("{label}: simulator rejected transition: {err:?}");
    report.unsupported.push(UnsupportedTransition {
        action_step: action.step,
        command: action.command.clone(),
        reason: reason.clone(),
    });
    reason
}

#[cfg(test)]
mod tests;
