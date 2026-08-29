use crate::model::{
    ActionId, AutomationConfig, AutomationPlanSnapshot, AutomationPlannedAction, AutomationPolicy,
    BlockedState, LegalAction, LegalActionKind, LivePhase, LiveState,
};
use serde_json::Value;
use std::time::Instant;
use sts_core::{
    content::cards::{HAVOC_ID, HAVOC_PLUS_ID},
    CombatAction, CombatState, MonsterId, RunAction, RunPhase, RunState, SimError,
};
use sts_search::{
    apply_planner_action, planner_action_display_label, planner_action_from_label,
    planner_action_label, search_with_warm_start, PlannerAction,
};

pub(super) fn plan_action_with_warm_start(
    config: &AutomationConfig,
    state: &LiveState,
    warm_steps: &[AutomationPlannedAction],
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    match config.policy {
        AutomationPolicy::FakePlayFirstCard => plan_single_card_play(state),
        AutomationPolicy::GreedySearch | AutomationPolicy::BeamSearch => {
            plan_search_action(config, state, warm_steps)
        }
    }
}

pub(super) fn bind_plan_step_to_live_action(
    state: &LiveState,
    step: &AutomationPlannedAction,
) -> Option<AutomationPlannedAction> {
    let run = observed_run_state(state).ok()?;
    let action = planner_action_from_label(&step.planner_action)?;
    let expected_command = expected_command(state, &run, &action)?;
    let live = match_live_action(state, &expected_command).ok()?;
    Some(planned_live_action(
        state.sequence,
        live,
        Some(&expected_command),
        step.planner_action.clone(),
    ))
}

fn plan_single_card_play(
    state: &LiveState,
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    if state.phase != LivePhase::Combat {
        return Err(blocked(
            "automation_not_combat",
            "automation can only plan combat actions",
        ));
    }

    let candidates = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::PlayCard)
        .collect::<Vec<_>>();

    let action = match candidates.as_slice() {
        [action] => *action,
        [] => {
            return Err(blocked(
                "automation_no_matching_action",
                "fake planner found no enabled card play",
            ))
        }
        _ => {
            return Err(blocked(
                "automation_ambiguous_action",
                "fake planner found more than one enabled card play",
            ))
        }
    };
    let planned = planned_live_action(
        state.sequence,
        action,
        action.command.get("command").and_then(Value::as_str),
        "fake_play_first_card".to_owned(),
    );
    let snapshot = AutomationPlanSnapshot {
        actions: vec![planned.clone()],
        played_actions: 0,
        predicted_final_hp: None,
        predicted_monster_hp: None,
        value: None,
        nodes: 1,
        terminal_reason: None,
        search_elapsed_ms: 0,
        budget_exhausted: false,
        timed_out: false,
        duplicate_checks: 0,
        duplicates: 0,
        cache_hits: 0,
    };
    Ok((planned, snapshot))
}

fn plan_search_action(
    config: &AutomationConfig,
    state: &LiveState,
    warm_steps: &[AutomationPlannedAction],
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    if state.phase != LivePhase::Combat {
        return Err(blocked(
            "automation_not_combat",
            "automation can only plan combat actions",
        ));
    }
    let run = observed_run_state(state)?;
    if run.phase != RunPhase::Combat || run.combat.is_none() {
        return Err(blocked(
            "automation_not_combat",
            "latest observed simulator state is not in combat",
        ));
    }

    if let Some((live, planner_action)) = live_selection_confirm(&run, state) {
        let planned = planned_live_action(
            state.sequence,
            live,
            Some("CONFIRM"),
            planner_action.to_owned(),
        );
        return Ok((
            planned.clone(),
            AutomationPlanSnapshot {
                actions: vec![planned],
                played_actions: 0,
                predicted_final_hp: None,
                predicted_monster_hp: None,
                value: None,
                nodes: 1,
                terminal_reason: None,
                search_elapsed_ms: 0,
                budget_exhausted: false,
                timed_out: false,
                duplicate_checks: 0,
                duplicates: 0,
                cache_hits: 0,
            },
        ));
    }

    let search_started = Instant::now();
    let search_config = config.search_config();
    let warm_actions = warm_steps
        .iter()
        .map(|step| planner_action_from_label(&step.planner_action))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    let recommendation = search_with_warm_start(&run, &search_config, &warm_actions)
        .map_err(planner_simulator_blocked)?;
    if recommendation.principal_variation.is_empty() {
        return Err(blocked(
            "automation_no_plan",
            "combat planner found no legal current combat action",
        ));
    }

    let first = &recommendation.principal_variation[0];
    let expected_command = expected_command(state, &run, first).ok_or_else(|| {
        blocked(
            "automation_unsupported_action",
            "planner selected an action that cannot be mapped to a live command",
        )
    })?;
    let live = match_live_action(state, &expected_command)?;
    let planned = planned_live_action(
        state.sequence,
        live,
        Some(&expected_command),
        planner_action_label(first),
    );

    let mut planned_actions = vec![planned.clone()];
    let mut future_run = apply_planner_action(&run, first).map_err(planner_simulator_blocked)?;
    for action in recommendation.principal_variation.iter().skip(1) {
        planned_actions.push(planned_future_action(state.sequence, &future_run, action));
        future_run =
            apply_planner_action(&future_run, action).map_err(planner_simulator_blocked)?;
    }

    let snapshot = AutomationPlanSnapshot {
        actions: planned_actions,
        played_actions: 0,
        predicted_final_hp: Some(recommendation.final_hp),
        predicted_monster_hp: Some(recommendation.monster_hp),
        value: Some(recommendation.value),
        nodes: recommendation.nodes,
        terminal_reason: recommendation.terminal_reason,
        search_elapsed_ms: u64::try_from(search_started.elapsed().as_millis()).unwrap_or(u64::MAX),
        budget_exhausted: recommendation.budget_exhausted,
        timed_out: recommendation.timed_out,
        duplicate_checks: recommendation.duplicate_checks,
        duplicates: recommendation.duplicates,
        cache_hits: recommendation.cache_hits,
    };
    Ok((planned, snapshot))
}

fn live_selection_confirm<'a>(
    run: &RunState,
    state: &'a LiveState,
) -> Option<(&'a LegalAction, &'static str)> {
    let combat = run.combat.as_ref()?;
    let planner_action = if combat.hand_select().is_some() {
        "confirm_hand_select"
    } else if combat.draw_select().is_some() {
        "confirm_draw_select"
    } else if combat.discard_select().is_some() {
        "confirm_discard_select"
    } else if combat.exhaust_select().is_some() {
        "confirm_exhaust_select"
    } else {
        return None;
    };
    let mut confirms = state.legal_actions.iter().filter(|action| {
        action.enabled
            && action.kind == LegalActionKind::Confirm
            && action
                .command
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.eq_ignore_ascii_case("CONFIRM"))
    });
    let confirm = confirms.next()?;
    confirms
        .next()
        .is_none()
        .then_some((confirm, planner_action))
}

fn observed_run_state(state: &LiveState) -> Result<RunState, BlockedState> {
    if let Some(value) = state.raw.get("sim_run_state") {
        if let Ok(run) = serde_json::from_value(value.clone()) {
            return Ok(run);
        }
    }
    Err(blocked(
        "automation_missing_sim_state",
        "automation requires simulator-tracked run state; hydrating from live observations is forbidden",
    ))
}

fn expected_command(state: &LiveState, run: &RunState, action: &PlannerAction) -> Option<String> {
    match action {
        PlannerAction::Combat(CombatAction::EndTurn) => Some("END".to_owned()),
        PlannerAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let combat = run.combat.as_ref()?;
            let hand_position = combat
                .piles
                .hand
                .iter()
                .position(|card| card.id == *card_id)?;
            let hand_slot = live_hand_slot(state, hand_position).unwrap_or(hand_position);
            if combat.piles.hand[hand_position].content_id == HAVOC_ID
                || combat.piles.hand[hand_position].content_id == HAVOC_PLUS_ID
            {
                return Some(format!("PLAY {hand_slot}"));
            }
            match target {
                Some(target) => {
                    let target_slot = live_monster_slot(state, combat, *target)?;
                    Some(format!("PLAY {hand_slot} {target_slot}"))
                }
                None => Some(format!("PLAY {hand_slot}")),
            }
        }
        PlannerAction::Potion(RunAction::UsePotion { slot, target }) => match target {
            Some(target) => {
                let combat = run.combat.as_ref()?;
                let target_slot = live_monster_slot(state, combat, *target)?;
                Some(format!("POTION USE {slot} {target_slot}"))
            }
            None => Some(format!("POTION USE {slot}")),
        },
        PlannerAction::Potion(_) => None,
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(RunAction::ChooseHandSelect { index })
        | PlannerAction::Run(RunAction::ChooseDrawSelect { index })
        | PlannerAction::Run(RunAction::ChooseDiscardSelect { index })
        | PlannerAction::Run(RunAction::ChooseExhaustSelect { index })
        | PlannerAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            Some(format!("CHOOSE {index}"))
        }
        PlannerAction::Run(RunAction::ConfirmHandSelect)
        | PlannerAction::Run(RunAction::ConfirmDrawSelect)
        | PlannerAction::Run(RunAction::ConfirmDiscardSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(_) => None,
    }
}

fn expected_sim_command(run: &RunState, action: &PlannerAction) -> Option<String> {
    match action {
        PlannerAction::Combat(CombatAction::EndTurn) => Some("END".to_owned()),
        PlannerAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let combat = run.combat.as_ref()?;
            let hand_slot = combat
                .piles
                .hand
                .iter()
                .position(|card| card.id == *card_id)?;
            match target {
                Some(target) => {
                    let target_slot = monster_position(combat, *target)?;
                    Some(format!("PLAY {hand_slot} {target_slot}"))
                }
                None => Some(format!("PLAY {hand_slot}")),
            }
        }
        PlannerAction::Potion(RunAction::UsePotion { slot, target }) => match target {
            Some(target) => {
                let combat = run.combat.as_ref()?;
                let target_slot = monster_position(combat, *target)?;
                Some(format!("POTION USE {slot} {target_slot}"))
            }
            None => Some(format!("POTION USE {slot}")),
        },
        PlannerAction::Potion(_) => None,
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(RunAction::ChooseHandSelect { index })
        | PlannerAction::Run(RunAction::ChooseDrawSelect { index })
        | PlannerAction::Run(RunAction::ChooseDiscardSelect { index })
        | PlannerAction::Run(RunAction::ChooseExhaustSelect { index })
        | PlannerAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            Some(format!("CHOOSE {index}"))
        }
        PlannerAction::Run(RunAction::ConfirmHandSelect)
        | PlannerAction::Run(RunAction::ConfirmDrawSelect)
        | PlannerAction::Run(RunAction::ConfirmDiscardSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(_) => None,
    }
}

fn live_hand_slot(state: &LiveState, hand_position: usize) -> Option<usize> {
    state
        .raw
        .pointer("/summary/combat/hand")
        .and_then(Value::as_array)
        .and_then(|hand| hand.get(hand_position))
        .and_then(|card| card.get("index"))
        .and_then(Value::as_u64)
        .and_then(|slot| usize::try_from(slot).ok())
}

fn live_monster_slot(state: &LiveState, combat: &CombatState, target: MonsterId) -> Option<usize> {
    let position = monster_position(combat, target)?;
    state
        .raw
        .pointer("/summary/combat/monsters")
        .and_then(Value::as_array)
        .and_then(|monsters| monsters.get(position))
        .and_then(|monster| monster.get("index"))
        .and_then(Value::as_u64)
        .and_then(|slot| usize::try_from(slot).ok())
}

fn monster_position(combat: &CombatState, target: MonsterId) -> Option<usize> {
    combat
        .monsters
        .iter()
        .position(|monster| monster.id == target)
}

fn match_live_action<'a>(
    state: &'a LiveState,
    expected_command: &str,
) -> Result<&'a LegalAction, BlockedState> {
    let candidates = state
        .legal_actions
        .iter()
        .filter(|action| {
            action.enabled
                && action
                    .command
                    .get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|command| command.eq_ignore_ascii_case(expected_command))
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [action] => Ok(*action),
        [] => Err(blocked(
            "automation_no_matching_action",
            &format!("planner command {expected_command:?} does not match a live legal action"),
        )),
        _ => Err(blocked(
            "automation_ambiguous_action",
            &format!("planner command {expected_command:?} matched multiple live legal actions"),
        )),
    }
}

fn planned_live_action(
    source_sequence: u64,
    action: &LegalAction,
    command: Option<&str>,
    planner_action: String,
) -> AutomationPlannedAction {
    AutomationPlannedAction {
        action_id: action.id.clone(),
        kind: action.kind.clone(),
        label: action.label.clone(),
        source_sequence,
        command: command.map(str::to_owned),
        planner_action,
    }
}

fn planned_future_action(
    source_sequence: u64,
    run: &RunState,
    action: &PlannerAction,
) -> AutomationPlannedAction {
    AutomationPlannedAction {
        action_id: ActionId("future".to_owned()),
        kind: planner_action_kind(action),
        label: planner_action_display_label(run, action),
        source_sequence,
        command: expected_sim_command(run, action),
        planner_action: planner_action_label(action),
    }
}

fn planner_action_kind(action: &PlannerAction) -> LegalActionKind {
    match action {
        PlannerAction::Combat(CombatAction::PlayCard { .. }) => LegalActionKind::PlayCard,
        PlannerAction::Combat(CombatAction::EndTurn) => LegalActionKind::EndTurn,
        PlannerAction::Potion(_) => LegalActionKind::UsePotion,
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => LegalActionKind::Confirm,
        PlannerAction::Run(_) => LegalActionKind::Confirm,
    }
}

pub(super) fn blocked(reason_code: &str, message: &str) -> BlockedState {
    BlockedState {
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    }
}

fn planner_simulator_blocked(error: SimError) -> BlockedState {
    blocked(
        "automation_simulator_error",
        &format!("combat planner rejected simulator state: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LivePhase, LiveState};
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use sts_core::{
        apply_run_decision_action,
        combat::{HandSelectPurpose, HandSelectState},
        content::cards::{
            CHRYSALIS_ID, DARK_SHACKLES_ID, HAVOC_PLUS_ID, SADISTIC_NATURE_ID, STRIKE_R_ID,
            WARCRY_ID,
        },
        potion::Potion,
        CardId, CardInstance, CombatDecisionState, RunDecisionAction,
    };
    use sts_search::planner_actions;
    use sts_verify::{
        import_communication_mod_trace, serialize_communication_mod_trace,
        verify_seed_start_communication_mod_trace, TraceLine, TraceMetadata, TraceState,
    };

    fn planner_run_actions(run: &RunState) -> Vec<RunAction> {
        planner_actions(run, &AutomationConfig::default().search_config())
            .expect("valid combat decisions")
            .into_iter()
            .filter_map(|action| match action {
                PlannerAction::Potion(action) | PlannerAction::Run(action) => Some(action),
                PlannerAction::Combat(_) => None,
            })
            .collect()
    }

    #[test]
    fn invalid_simulator_state_blocks_search_instead_of_looking_actionless() {
        let mut run = RunState::combat_fixture();
        let duplicate = run.combat.as_ref().expect("combat").piles.hand[0];
        run.combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile
            .push(duplicate);
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({"sim_run_state": run}),
        };

        let error = plan_search_action(&AutomationConfig::default(), &state, &[])
            .expect_err("invalid simulator state must block automation");

        assert_eq!(error.reason_code, "automation_simulator_error");
        assert!(error.message.contains("invalid state"));
    }

    #[test]
    fn malformed_persisted_label_discards_the_whole_warm_suffix() {
        let run = RunState::combat_fixture();
        let state = LiveState {
            sequence: 2,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("end".to_owned()),
                kind: LegalActionKind::EndTurn,
                label: "End turn".to_owned(),
                enabled: true,
                command: json!({"command": "END"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };
        let config = AutomationConfig {
            depth: 2,
            width: 10,
            search_transition_budget: 10_000,
            search_time_budget_ms: 0,
            ..AutomationConfig::default()
        };
        let mut valid = planned_future_action(
            state.sequence,
            &run,
            &PlannerAction::Combat(CombatAction::EndTurn),
        );
        valid.action_id = ActionId("persisted-end".to_owned());
        let mut malformed = valid.clone();
        malformed.action_id = ActionId("persisted-malformed".to_owned());
        malformed.planner_action = "not a planner action".to_owned();

        let (_, snapshot) = plan_search_action(&config, &state, &[valid, malformed])
            .expect("malformed warm suffix is discarded before search");

        assert_eq!(snapshot.cache_hits, 0);
    }

    #[test]
    fn explosive_potion_planner_actions_include_live_monster_targets() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Explosive];
        run.empty_potion_slots = vec![0, 2];
        let living_targets = run
            .combat
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| monster.id)
            .collect::<Vec<_>>();

        let actions = planner_run_actions(&run);

        assert_eq!(actions.len(), living_targets.len());
        for target in living_targets {
            assert!(actions.contains(&RunAction::UsePotion {
                slot: 1,
                target: Some(target),
            }));
        }
        assert!(!actions.contains(&RunAction::UsePotion {
            slot: 1,
            target: None,
        }));
    }

    #[test]
    fn toolbox_choices_are_planned_as_combat_card_rewards() {
        let mut run = RunState::combat_fixture();
        run.combat.as_mut().expect("combat").decision =
            Some(CombatDecisionState::ToolboxCardReward {
                choices: vec![
                    CardInstance::new(CardId::new(101), CHRYSALIS_ID),
                    CardInstance::new(CardId::new(102), SADISTIC_NATURE_ID),
                    CardInstance::new(CardId::new(103), DARK_SHACKLES_ID),
                ],
            });
        let state = LiveState {
            sequence: 7692,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-1".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "sadistic nature".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 1"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };
        let run: RunState = serde_json::from_value(state.raw["sim_run_state"].clone()).unwrap();

        let actions = planner_actions(&run, &AutomationConfig::default().search_config())
            .expect("valid combat decisions");

        assert_eq!(actions.len(), 3);
        assert!(matches!(
            actions[1],
            PlannerAction::Run(RunAction::ChooseCombatCardReward { index: 1 })
        ));
        assert_eq!(
            expected_command(&state, &run, &actions[1]),
            Some("CHOOSE 1".to_owned())
        );
        assert_eq!(
            planner_action_display_label(&run, &actions[1]),
            "Choose combat card Sadistic Nature"
        );
    }

    #[test]
    fn liquid_memories_grid_is_planned_as_a_combat_discard_selection() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::LiquidMemories];
        run.empty_potion_slots.clear();
        let recalled = CardInstance::new(CardId::new(91), STRIKE_R_ID);
        run.combat
            .as_mut()
            .expect("combat")
            .piles
            .discard_pile
            .push(recalled);
        let run = apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::UsePotion {
                slot: 0,
                target: None,
            }),
        )
        .expect("Liquid Memories opens its discard selection");
        let state = LiveState {
            sequence: 12,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "strike+".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };
        let run: RunState = serde_json::from_value(state.raw["sim_run_state"].clone()).unwrap();

        let actions = planner_actions(&run, &AutomationConfig::default().search_config())
            .expect("valid combat decisions");

        assert!(matches!(
            actions.as_slice(),
            [PlannerAction::Run(RunAction::ChooseDiscardSelect {
                index: 0
            })]
        ));
        assert_eq!(
            expected_command(&state, &run, &actions[0]),
            Some("CHOOSE 0".to_owned())
        );
    }

    #[test]
    fn warcry_hand_select_is_planned_as_choose_then_confirm() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        let source = CardInstance::new(CardId::new(90), WARCRY_ID);
        let target = CardInstance::new(CardId::new(91), STRIKE_R_ID);
        combat.piles.hand = vec![source, target];
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id: source.id,
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: Default::default(),
        });

        let state = LiveState {
            sequence: 13,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "strike".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };

        let actions = planner_actions(&run, &AutomationConfig::default().search_config())
            .expect("valid combat decisions");
        assert!(matches!(
            actions.as_slice(),
            [PlannerAction::Run(RunAction::ChooseHandSelect { index: 0 })]
        ));
        assert_eq!(
            expected_command(&state, &run, &actions[0]),
            Some("CHOOSE 0".to_owned())
        );

        let selected = apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::ChooseHandSelect { index: 0 }),
        )
        .expect("Warcry choice applies");
        let follow_up = planner_actions(&selected, &AutomationConfig::default().search_config())
            .expect("valid follow-up decisions");
        assert!(matches!(
            follow_up.as_slice(),
            [PlannerAction::Run(RunAction::ConfirmHandSelect)]
        ));
    }

    #[test]
    fn havoc_live_command_uses_havoc_card_slot_without_top_card_target() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let target = combat
            .monsters
            .iter()
            .find(|monster| monster.alive)
            .expect("living monster")
            .id;
        combat.piles.hand = vec![CardInstance::new(CardId::new(42), HAVOC_PLUS_ID)];
        combat.piles.draw_pile = vec![CardInstance::new(CardId::new(43), STRIKE_R_ID)];

        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": {
                    "combat": {
                        "hand": [
                            { "index": 2 }
                        ],
                        "monsters": [
                            { "index": 0 }
                        ]
                    }
                }
            }),
        };
        let action = PlannerAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(42),
            target: Some(target),
        });

        assert_eq!(
            expected_command(&state, &run, &action),
            Some("PLAY 2".to_owned())
        );
    }

    #[test]
    fn explicit_targets_require_authoritative_live_monster_slots() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Fire];
        let combat = run.combat.as_ref().expect("combat fixture");
        let card_id = combat.piles.hand[0].id;
        let target = combat
            .monsters
            .iter()
            .find(|monster| monster.alive)
            .expect("living monster")
            .id;
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": {
                    "combat": {
                        "hand": [{ "index": 0 }],
                        "monsters": [{}]
                    }
                }
            }),
        };

        assert_eq!(
            expected_command(
                &state,
                &run,
                &PlannerAction::Combat(CombatAction::PlayCard {
                    card_id,
                    target: Some(target),
                }),
            ),
            None
        );
        assert_eq!(
            expected_command(
                &state,
                &run,
                &PlannerAction::Potion(RunAction::UsePotion {
                    slot: 0,
                    target: Some(target),
                }),
            ),
            None
        );
    }

    #[test]
    fn planner_confirms_gambling_chip_select_without_discards() {
        let mut run = RunState::combat_fixture();
        sts_core::combat::open_gambling_chip_select(run.combat.as_mut().expect("combat"))
            .expect("Gambling Chip selection opens");
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({
                    "transport": "communication_mod",
                    "command": "CONFIRM",
                }),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "screen_type": "HAND_SELECT",
                },
                "sim_run_state": run,
            }),
        };

        let (planned, snapshot) =
            plan_action_with_warm_start(&AutomationConfig::default(), &state, &[])
                .expect("planner chooses confirm");

        assert_eq!(planned.kind, LegalActionKind::Confirm);
        assert_eq!(planned.command.as_deref(), Some("CONFIRM"));
        assert_eq!(planned.planner_action, "confirm_exhaust_select");
        assert_eq!(snapshot.actions[0].planner_action, "confirm_exhaust_select");
    }

    #[test]
    #[ignore = "expensive corpus-wide combat benchmark; run explicitly with --ignored"]
    fn collected_trace_benchmark_reports_train_and_validation_reward() {
        let cases = collect_trace_combat_cases();
        assert!(
            cases.len() >= 12,
            "expected enough trace combat roots, found {}",
            cases.len()
        );

        let config = AutomationConfig::default();
        let train_cases = cases
            .iter()
            .enumerate()
            .filter_map(|(index, case)| (index % 4 != 0).then_some(case))
            .collect::<Vec<_>>();
        let validation_cases = cases
            .iter()
            .enumerate()
            .filter_map(|(index, case)| (index % 4 == 0).then_some(case))
            .collect::<Vec<_>>();
        let train_sample = evenly_sample_cases(&train_cases, 32);
        let validation_sample = evenly_sample_cases(&validation_cases, 16);

        let train = evaluate_trace_cases("train", &train_sample, &config);
        let validation = evaluate_trace_cases("validation", &validation_sample, &config);

        assert!(
            train.compared >= 6,
            "training split produced too few comparable roots: {train:?}"
        );
        assert!(
            validation.compared >= 3,
            "validation split produced too few comparable roots: {validation:?}"
        );
        assert!(
            train.machine_reward_avg + 2.0 >= train.human_reward_avg,
            "training machine reward regressed too far behind human: {train:?}"
        );
        assert!(
            validation.machine_reward_avg >= validation.human_reward_avg,
            "validation machine reward did not match or beat human: {validation:?}"
        );
    }

    #[derive(Debug)]
    struct TraceCombatCase {
        path: PathBuf,
        start_line_index: usize,
        start_hp: i32,
        human_terminal: ObservedTerminal,
    }

    #[derive(Debug)]
    struct ObservedTerminal {
        hp: i32,
        max_hp: i32,
        gold: i32,
        potions: usize,
    }

    #[derive(Debug)]
    struct TraceBenchmarkReport {
        compared: usize,
        skipped: usize,
        human_reward_avg: f64,
        machine_reward_avg: f64,
        human_hp_loss_avg: f64,
        machine_hp_loss_avg: f64,
        worst_hp_losses: Vec<TraceCaseResult>,
    }

    #[derive(Debug, Clone)]
    struct TraceCaseResult {
        path: PathBuf,
        start_line_index: usize,
        root_potions: usize,
        human_hp_loss: i32,
        machine_hp_loss: i32,
        human_reward: f64,
        machine_reward: f64,
        terminal_reason: Option<String>,
        first_actions: Vec<String>,
    }

    fn collect_trace_combat_cases() -> Vec<TraceCombatCase> {
        let mut paths = std::env::var_os("STS_PERMANENT_CORPUS_DIR")
            .map(PathBuf::from)
            .and_then(|root| std::fs::read_dir(root).ok())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();

        let mut cases = Vec::new();
        for path in paths {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(trace) = import_communication_mod_trace(&content) else {
                continue;
            };

            let mut previous_state: Option<&TraceState> = None;
            for (index, line) in trace.lines.iter().enumerate() {
                let TraceLine::State(state) = line else {
                    continue;
                };
                if observed_room_phase(state) == Some("COMBAT")
                    && previous_state
                        .and_then(observed_room_phase)
                        .is_some_and(|phase| phase != "COMBAT")
                {
                    if let Some(terminal) = trace.lines[index + 1..]
                        .iter()
                        .filter_map(|line| match line {
                            TraceLine::State(state) => Some(state),
                            _ => None,
                        })
                        .find(|state| observed_room_phase(state) != Some("COMBAT"))
                        .and_then(observed_terminal)
                    {
                        if let Some(start_hp) = observed_i32(state, "current_hp") {
                            cases.push(TraceCombatCase {
                                path: path.clone(),
                                start_line_index: index,
                                start_hp,
                                human_terminal: terminal,
                            });
                        }
                    }
                }
                previous_state = Some(state);
            }
        }
        cases
    }

    fn evaluate_trace_cases(
        label: &'static str,
        cases: &[&TraceCombatCase],
        config: &AutomationConfig,
    ) -> TraceBenchmarkReport {
        let mut compared = 0usize;
        let mut skipped = 0usize;
        let mut human_reward = 0.0;
        let mut machine_reward = 0.0;
        let mut human_hp_loss = 0.0;
        let mut machine_hp_loss = 0.0;
        let mut case_results = Vec::new();

        for case in cases {
            let Some(root) = verify_trace_prefix_root(&case.path, case.start_line_index) else {
                skipped += 1;
                continue;
            };
            if root.phase != RunPhase::Combat || root.combat.is_none() {
                skipped += 1;
                continue;
            }

            let Ok(recommendation) = search_with_warm_start(&root, &config.search_config(), &[])
            else {
                skipped += 1;
                continue;
            };
            let Some(reason) = recommendation.terminal_reason.as_deref() else {
                skipped += 1;
                continue;
            };
            if !matches!(reason, "won" | "escaped") {
                skipped += 1;
                continue;
            }

            let human_case_reward = observed_terminal_value(&case.human_terminal);
            let machine_case_reward = recommendation.value - 1_000_000.0;
            let human_case_hp_loss = case.start_hp - case.human_terminal.hp;
            let machine_case_hp_loss = case.start_hp - recommendation.final_hp;
            compared += 1;
            human_reward += human_case_reward;
            machine_reward += machine_case_reward;
            human_hp_loss += f64::from(human_case_hp_loss);
            machine_hp_loss += f64::from(machine_case_hp_loss);
            case_results.push(TraceCaseResult {
                path: case.path.clone(),
                start_line_index: case.start_line_index,
                root_potions: root.potions.len(),
                human_hp_loss: human_case_hp_loss,
                machine_hp_loss: machine_case_hp_loss,
                human_reward: human_case_reward,
                machine_reward: machine_case_reward,
                terminal_reason: recommendation.terminal_reason.clone(),
                first_actions: planned_action_labels(&root, &recommendation.principal_variation, 8),
            });
        }

        case_results.sort_by(|left, right| {
            let left_delta = left.machine_hp_loss - left.human_hp_loss;
            let right_delta = right.machine_hp_loss - right.human_hp_loss;
            right_delta
                .cmp(&left_delta)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.start_line_index.cmp(&right.start_line_index))
        });
        let worst_hp_losses = case_results.into_iter().take(5).collect::<Vec<_>>();

        let report = TraceBenchmarkReport {
            compared,
            skipped,
            human_reward_avg: average(human_reward, compared),
            machine_reward_avg: average(machine_reward, compared),
            human_hp_loss_avg: average(human_hp_loss, compared),
            machine_hp_loss_avg: average(machine_hp_loss, compared),
            worst_hp_losses,
        };
        println!(
            "trace_benchmark {label} compared={} skipped={} human_reward_avg={:.2} machine_reward_avg={:.2} human_hp_loss_avg={:.2} machine_hp_loss_avg={:.2}",
            report.compared,
            report.skipped,
            report.human_reward_avg,
            report.machine_reward_avg,
            report.human_hp_loss_avg,
            report.machine_hp_loss_avg
        );
        for case in &report.worst_hp_losses {
            println!(
                "trace_benchmark_worst_hp {label} file={} line={} root_potions={} human_hp_loss={} machine_hp_loss={} human_reward={:.2} machine_reward={:.2} terminal={:?} actions={}",
                case.path.file_name().and_then(|name| name.to_str()).unwrap_or("<unknown>"),
                case.start_line_index,
                case.root_potions,
                case.human_hp_loss,
                case.machine_hp_loss,
                case.human_reward,
                case.machine_reward,
                case.terminal_reason,
                case.first_actions.join(" | ")
            );
        }
        report
    }

    fn planned_action_labels(
        root: &RunState,
        actions: &[PlannerAction],
        limit: usize,
    ) -> Vec<String> {
        let mut labels = Vec::new();
        let mut state = root.clone();
        for action in actions.iter().take(limit) {
            labels.push(planner_action_display_label(&state, action));
            let Ok(next) = apply_planner_action(&state, action) else {
                break;
            };
            state = next;
        }
        labels
    }

    fn evenly_sample_cases<'a>(
        cases: &[&'a TraceCombatCase],
        max_cases: usize,
    ) -> Vec<&'a TraceCombatCase> {
        if cases.len() <= max_cases {
            return cases.to_vec();
        }
        (0..max_cases)
            .map(|index| {
                let case_index = index * cases.len() / max_cases;
                cases[case_index]
            })
            .collect()
    }

    fn verify_trace_prefix_root(path: &Path, line_index: usize) -> Option<RunState> {
        let content = std::fs::read_to_string(path).ok()?;
        let trace = import_communication_mod_trace(&content).ok()?;
        let metadata = trace.metadata.unwrap_or(TraceMetadata {
            schema: 1,
            source: "communication_mod".to_owned(),
            boundary_schema: None,
            client: None,
            mode: None,
            started_at: None,
            ended_at: None,
            event: None,
            boss_unlocks: None,
            run_config: None,
        });
        let prefix = serialize_communication_mod_trace(&metadata, &trace.lines[..=line_index]);
        let report = verify_seed_start_communication_mod_trace(&prefix).ok()?;
        if !report.unexpected_diffs.is_empty() || !report.unsupported.is_empty() {
            return None;
        }
        report.seed_start?.sim_run_state
    }

    fn observed_room_phase(state: &TraceState) -> Option<&str> {
        state
            .message
            .pointer("/game_state/room_phase")
            .and_then(serde_json::Value::as_str)
    }

    fn observed_terminal(state: &TraceState) -> Option<ObservedTerminal> {
        Some(ObservedTerminal {
            hp: observed_i32(state, "current_hp")?,
            max_hp: observed_i32(state, "max_hp")?,
            gold: observed_i32(state, "gold")?,
            potions: observed_potion_count(state),
        })
    }

    fn observed_terminal_value(terminal: &ObservedTerminal) -> f64 {
        f64::from(terminal.hp)
            + f64::from(terminal.max_hp) * 3.0
            + f64::from(terminal.gold) / 10.0
            + terminal.potions as f64 * 8.0
    }

    fn observed_i32(state: &TraceState, key: &str) -> Option<i32> {
        state
            .message
            .get("game_state")?
            .get(key)?
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
    }

    fn observed_potion_count(state: &TraceState) -> usize {
        state
            .message
            .pointer("/game_state/potions")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|potion| {
                potion
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id != "Potion Slot")
            })
            .count()
    }

    fn average(total: f64, count: usize) -> f64 {
        if count == 0 {
            0.0
        } else {
            total / count as f64
        }
    }
}
