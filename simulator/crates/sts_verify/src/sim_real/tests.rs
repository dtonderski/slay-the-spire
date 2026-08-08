use super::*;
use serde_json::json;

fn metadata(boundary_schema: Option<u32>, with_profile: bool) -> Value {
    let mut value = json!({
        "type": "metadata",
        "schema": 1,
        "source": "communication_mod"
    });
    if let Some(schema) = boundary_schema {
        value["boundary_schema"] = json!(schema);
    }
    if with_profile {
        value["run_config"] = json!({
            "profile": {"note_card": "Strike", "note_upgrades": 0}
        });
    }
    value
}

fn boundary_message(kind: &str) -> Value {
    json!({
        "boundary_schema": 1,
        "boundary_kind": kind,
        "game_update_seq": 1,
        "dungeon_update_seq": 1,
        "actions_queued": 0,
        "card_queue_size": 0,
        "pre_turn_actions_size": 0,
        "current_action": null,
        "ready_for_command": true,
        "in_game": false
    })
}

fn trace(records: Vec<Value>) -> String {
    records
        .into_iter()
        .map(|record| serde_json::to_string(&record).expect("record serializes"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn schema_v0_metadata_is_rejected_before_replay() {
    let content = trace(vec![
        metadata(None, false),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":{"ready_for_command":true}}),
    ]);
    let error = verify_communication_mod_trace(&content).expect_err("v0 must be unsupported");
    assert!(matches!(
        error,
        SimRealError::UnsupportedSchema {
            boundary_schema: None
        }
    ));
    assert!(
        error.to_string().contains("only schema 1 is supported")
            || error.to_string().contains("boundary_schema=1")
    );
}

#[test]
fn explicit_schema_zero_is_rejected_before_replay() {
    let content = trace(vec![metadata(Some(0), true)]);
    assert!(matches!(
        verify_communication_mod_trace(&content),
        Err(SimRealError::UnsupportedSchema {
            boundary_schema: Some(0)
        })
    ));
}

#[test]
fn schema_v0_is_rejected_before_a_malformed_body_is_parsed() {
    let content = format!("{}\n{{malformed body", metadata(Some(0), true));
    assert!(matches!(
        verify_communication_mod_trace(&content),
        Err(SimRealError::UnsupportedSchema {
            boundary_schema: Some(0)
        })
    ));
}

#[test]
fn metadata_v1_cannot_fall_back_to_state_without_boundary_schema() {
    let content = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":{"ready_for_command":true}}),
    ]);
    let error = verify_communication_mod_trace(&content).expect_err("state contract must fail");
    assert!(matches!(
        error,
        SimRealError::InvalidBoundaryContract { step: 1, .. }
    ));
    assert!(error.to_string().contains("boundary_schema=1"));
}

#[test]
fn profile_must_be_explicit_typed_metadata() {
    let content = trace(vec![
        metadata(Some(1), false),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    let error = verify_communication_mod_trace(&content).expect_err("profile is mandatory");
    assert!(matches!(error, SimRealError::InvalidProfileInput(_)));
    assert!(error.to_string().contains("metadata.run_config.profile"));
}

#[test]
fn metadata_must_be_the_unique_leading_record() {
    let content = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        metadata(Some(1), true),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    let error = verify_communication_mod_trace(&content).expect_err("late metadata must fail");
    assert!(error.to_string().contains("leading metadata"));
}

#[test]
fn actions_must_be_contiguous_and_pair_immediately() {
    let content = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
        json!({"type":"action","step":3,"command":"STATE"}),
    ]);
    let error = verify_communication_mod_trace(&content).expect_err("step gap must fail");
    assert!(error.to_string().contains("not contiguous"));

    let starts_late = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":2,"command":"START IRONCLAD 0 1"}),
    ]);
    assert!(verify_communication_mod_trace(&starts_late)
        .expect_err("first step must be one")
        .to_string()
        .contains("first action step"));
}

#[test]
fn state_poll_and_gameplay_boundary_kinds_are_authoritative() {
    let gameplay_on_poll = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("poll")}),
    ]);
    assert!(verify_communication_mod_trace(&gameplay_on_poll)
        .expect_err("gameplay cannot complete on poll")
        .to_string()
        .contains("expected interaction_ready, quiescent, or terminal"));

    let state_on_quiescent = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
        json!({"type":"action","step":2,"command":"STATE"}),
        json!({"type":"state","step":2,"message":boundary_message("quiescent")}),
    ]);
    assert!(verify_communication_mod_trace(&state_on_quiescent)
        .expect_err("STATE requires poll")
        .to_string()
        .contains("expected poll"));
}

#[test]
fn external_rng_must_pair_with_the_pending_same_step_action() {
    let orphan = trace(vec![
        metadata(Some(1), true),
        json!({"type":"external_rng","step":1,"draws":[]}),
    ]);
    assert!(matches!(
        verify_communication_mod_trace(&orphan),
        Err(SimRealError::OrphanExternalRng { step: 1 })
    ));

    let wrong_step = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"external_rng","step":2,"draws":[]}),
    ]);
    assert!(matches!(
        verify_communication_mod_trace(&wrong_step),
        Err(SimRealError::OrphanExternalRng { step: 2 })
    ));

    let paired = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"external_rng","step":1,"draws":[]}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    assert!(verify_communication_mod_trace(&paired).is_ok());
}

#[test]
fn explicit_combat_target_never_falls_back_from_a_stale_slot() {
    let run = RunState::combat_fixture();
    let combat = run.combat.as_ref().expect("combat fixture");
    assert!(
        combat
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .count()
            == 1
    );
    assert_eq!(monster_id_from_bridge_slot(combat, "99"), None);
}

#[test]
fn matching_error_is_the_only_non_state_action_completion() {
    let content = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
        json!({"type":"action","step":2,"command":"CHOOSE 99"}),
        json!({"type":"error","step":2,"message":{"error":"rejected"}}),
    ]);
    let report = verify_communication_mod_trace(&content).expect("matching error is valid");
    assert_eq!(report.action_integrity.unwrap().rejected_actions, 1);
    assert_eq!(
        report.action_dispositions[1].disposition,
        ActionDispositionKind::TargetRejected
    );
}

#[test]
fn strict_mode_validates_malformed_tail_after_first_semantic_difference() {
    let prefix = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    let diagnostic = verify_communication_mod_trace_diagnostic_reader(std::io::Cursor::new(
        format!("{prefix}{{malformed tail").into_bytes(),
    ))
    .expect("diagnostic mode stops at the first difference");
    assert!(!diagnostic.action_integrity.as_ref().unwrap().eof_validated);
    assert!(matches!(
        crate::assess_verification(
            Ok(&diagnostic),
            diagnostic.action_integrity.as_ref()
        ),
        crate::VerificationOutcome::Failed { failures }
            if failures.contains(&crate::VerificationFailure::TailNotValidated)
    ));

    assert!(matches!(
        verify_communication_mod_trace(&format!("{prefix}{{malformed tail")),
        Err(SimRealError::Trace(_))
    ));
}

#[test]
fn semantic_boundary_is_preserved_while_valid_tail_is_fully_accounted() {
    let mut poll = boundary_message("poll");
    poll["in_game"] = json!(true);
    let content = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
        json!({"type":"action","step":2,"command":"STATE"}),
        json!({"type":"state","step":2,"message":poll}),
        json!({"type":"action","step":3,"command":"CHOOSE 99"}),
        json!({"type":"error","step":3,"message":{"error":"rejected"}}),
    ]);
    let report = verify_communication_mod_trace(&content).expect("valid tail is accepted");
    let boundary = &report.seed_start.as_ref().unwrap().first_boundary;
    assert_eq!(boundary.category, "unexpected_sim_real_diff");
    assert_eq!(boundary.path, "$.actions[step=1].command");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .map(|entry| entry.disposition)
            .collect::<Vec<_>>(),
        vec![
            ActionDispositionKind::UnexpectedDiff,
            ActionDispositionKind::BeyondBoundary,
            ActionDispositionKind::TargetRejected,
        ]
    );
    let integrity = report.action_integrity.unwrap();
    assert!(integrity.eof_validated);
    assert_eq!(integrity.applicable_actions, 2);
    assert_eq!(integrity.disposed_actions, 2);
    assert_eq!(integrity.rejected_actions, 1);
    assert!(!integrity.terminal_state_observed);
}

#[test]
fn every_malformed_tail_class_fails_after_a_semantic_boundary() {
    let prefix = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    let malformed_tails = [
        json!({"type":"action","step":3,"command":"STATE"}).to_string(),
        json!({"type":"external_rng","step":2,"draws":[]}).to_string(),
        metadata(Some(1), true).to_string(),
        format!(
            "{}\n{}",
            json!({"type":"action","step":2,"command":"STATE"}),
            json!({"type":"state","step":3,"message":boundary_message("poll")})
        ),
        format!(
            "{}\n{}",
            json!({"type":"action","step":2,"command":"CHOOSE 99"}),
            json!({"type":"error","step":3,"message":{"error":"wrong step"}})
        ),
        json!({"type":"action","step":2,"command":"STATE"}).to_string(),
    ];
    for tail in malformed_tails {
        assert!(
            verify_communication_mod_trace(&format!("{prefix}{tail}\n")).is_err(),
            "malformed tail unexpectedly passed: {tail}"
        );
    }
}

#[test]
fn terminality_comes_from_the_final_valid_tail_state() {
    let content = |terminal: bool| {
        let mut poll = boundary_message("poll");
        poll["in_game"] = json!(!terminal);
        trace(vec![
            metadata(Some(1), true),
            json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
            json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
            json!({"type":"action","step":2,"command":"STATE"}),
            json!({"type":"state","step":2,"message":poll}),
        ])
    };
    assert!(
        !verify_communication_mod_trace(&content(false))
            .unwrap()
            .action_integrity
            .unwrap()
            .terminal_state_observed
    );
    assert!(
        verify_communication_mod_trace(&content(true))
            .unwrap()
            .action_integrity
            .unwrap()
            .terminal_state_observed
    );
}

#[test]
fn observation_metadata_never_selects_or_mutates_simulator_state() {
    let content = |game_update_seq| {
        let mut boundary = boundary_message("quiescent");
        boundary["game_update_seq"] = json!(game_update_seq);
        trace(vec![
            metadata(Some(1), true),
            json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
            json!({"type":"state","step":1,"message":boundary}),
        ])
    };
    let left = verify_communication_mod_trace(&content(1)).expect("left trace verifies");
    let right = verify_communication_mod_trace(&content(999)).expect("right trace verifies");
    assert_eq!(
        left.seed_start.unwrap().sim_run_state,
        right.seed_start.unwrap().sim_run_state
    );
}

#[test]
fn valid_v1_enters_direct_replay_without_candidate_fallback() {
    let content = trace(vec![
        metadata(Some(1), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    let report = verify_communication_mod_trace(&content).expect("strict trace parses");
    let seed_start = report.seed_start.expect("direct replay report");
    assert_eq!(
        seed_start.first_boundary.category,
        "unexpected_sim_real_diff"
    );
    assert!(seed_start
        .first_boundary
        .reason
        .contains("direct START transition"));
    assert_eq!(report.action_dispositions.len(), 1);
}

#[test]
fn strict_replay_binds_leave_to_the_simulator_shop_action() {
    let content = crate::load_corpus_file(
        "open_failures/FIDL01274-p1274-2026-08-07T13-08-53-551Z-2116632.jsonl",
    )
    .expect("strict shop witness is present");
    let report = verify_communication_mod_trace(&content).expect("strict trace parses");

    assert!(report
        .verified
        .iter()
        .any(|transition| transition.action_step == 93
            && transition.command == "LEAVE"
            && transition.label == "direct Shop transition"));
    assert!(!report
        .unsupported
        .iter()
        .any(|transition| transition.action_step == 93));
    assert!(report
        .verified
        .iter()
        .any(|transition| transition.action_step == 94));
}

#[test]
fn fidl01271_neow_remove_two_final_choose_and_leave_verify_strictly() {
    let mut metadata = metadata(Some(1), true);
    metadata["run_config"]["profile"]["note_card"] = json!("Normality");
    let deck = |ids: &[&str]| {
        ids.iter()
            .map(|id| json!({"id": id, "upgrades": 0}))
            .collect::<Vec<_>>()
    };
    let starting_deck = deck(&[
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
        "Defend_R", "Defend_R", "Bash",
    ]);
    let settled_deck = deck(&[
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R", "Bash",
    ]);
    let relics = vec![json!({"id": "Burning Blood"})];
    let state = |step, game_state| {
        let mut message = boundary_message("quiescent");
        message["game_state"] = game_state;
        json!({"type": "state", "step": step, "message": message})
    };
    let event_state = |hp, choices, cards| {
        json!({
            "screen_type": "EVENT",
            "ascension_level": 0,
            "floor": 0,
            "gold": 99,
            "current_hp": hp,
            "max_hp": 10000,
            "deck": cards,
            "relics": relics.clone(),
            "potions": [],
            "choice_list": choices,
            "screen_state": {
                "event_id": "Neow Event",
                "options": []
            }
        })
    };
    let grid_state = |hp, selected_cards| {
        json!({
            "screen_type": "GRID",
            "ascension_level": 0,
            "floor": 0,
            "gold": 99,
            "current_hp": hp,
            "max_hp": 10000,
            "deck": starting_deck.clone(),
            "relics": relics.clone(),
            "potions": [],
            "choice_list": [
                "strike", "strike", "strike", "strike", "strike",
                "defend", "defend", "defend", "defend", "bash"
            ],
            "screen_state": {
                "cards": starting_deck.clone(),
                "selected_cards": selected_cards,
                "confirm_up": false,
                "for_purge": false,
                "for_transform": false,
                "for_upgrade": false,
                "any_number": false,
                "num_cards": 2
            }
        })
    };
    let map_state = json!({
        "screen_type": "MAP",
        "ascension_level": 0,
        "floor": 0,
        "gold": 99,
        "current_hp": 7000,
        "max_hp": 10000,
        "deck": settled_deck,
        "relics": relics,
        "potions": [],
        "choice_list": ["x=0", "x=3", "x=5"],
        "screen_state": {
            "first_node_chosen": false,
            "current_node": {"x": 0, "y": -1},
            "next_nodes": [
                {"symbol": "M", "x": 0, "y": 0},
                {"symbol": "M", "x": 3, "y": 0},
                {"symbol": "M", "x": 5, "y": 0}
            ]
        }
    });
    let content = trace(vec![
        metadata,
        json!({
            "type": "action",
            "step": 1,
            "command": "START_VERIFY IRONCLAD 0 FIDL01271 10000"
        }),
        state(1, event_state(10000, vec!["talk"], starting_deck.clone())),
        json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
        state(
            2,
            event_state(
                10000,
                vec![
                    "choose a colorless card to obtain",
                    "obtain 3 random potions",
                    "take 3000 damage remove 2 cards",
                    "lose your starting relic obtain a random boss relic",
                ],
                starting_deck.clone(),
            ),
        ),
        json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
        state(3, grid_state(7000, Vec::<Value>::new())),
        json!({"type": "action", "step": 4, "command": "CHOOSE 8"}),
        state(
            4,
            grid_state(7000, vec![json!({"id": "Defend_R", "upgrades": 0})]),
        ),
        json!({"type": "action", "step": 5, "command": "CHOOSE 6"}),
        state(5, event_state(7000, vec!["leave"], settled_deck.clone())),
        json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
        state(6, map_state),
    ]);
    let report = verify_communication_mod_trace(&content).expect("strict trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    for step in [5, 6] {
        assert_eq!(
            report
                .action_dispositions
                .iter()
                .find(|entry| entry.action_step == step)
                .map(|entry| entry.disposition),
            Some(ActionDispositionKind::Verified),
            "step {step} should verify: {report:#?}"
        );
    }
}

#[test]
fn neow_card_reward_pick_closes_to_the_leave_event_boundary() {
    let run = RunState::seeded_ironclad(34_961_238_661_095, 0);
    let talked = apply_run_decision_action(
        &run,
        RunDecisionAction::Event(sts_core::EventAction::Choose { choice_index: 0 }),
    )
    .expect("Neow talk opens reward options");
    let option_index =
        sts_core::generate_neow_options(talked.event_rng_seed as i64, talked.player_max_hp)
            .iter()
            .position(|option| {
                matches!(
                    option.reward,
                    sts_core::NeowRewardType::ThreeCards
                        | sts_core::NeowRewardType::OneRandomRareCard
                        | sts_core::NeowRewardType::RandomColorless
                        | sts_core::NeowRewardType::RandomColorlessTwo
                        | sts_core::NeowRewardType::ThreeRareCards
                )
            })
            .expect("fixture offers a Neow card reward");
    let reward = apply_run_decision_action(
        &talked,
        RunDecisionAction::Event(sts_core::EventAction::Choose {
            choice_index: option_index,
        }),
    )
    .expect("Neow card reward opens");
    assert_eq!(
        reward
            .reward
            .as_ref()
            .expect("card reward screen")
            .continuation,
        RewardContinuation::Neow
    );

    let card_index = 0;
    let card_id = reward.reward.as_ref().expect("card reward screen").choices[card_index].id;
    let bound = seed_start_bind_reward_choose_action(&reward, card_index)
        .expect("CHOOSE binds to the visible card");
    assert_eq!(bound, RunAction::TakeCardReward { card_id });
    let settled = apply_run_decision_action(&reward, RunDecisionAction::Run(bound))
        .expect("card pick settles its Neow reward overlay");
    assert_eq!(settled.phase, RunPhase::Event);
    assert!(settled.reward.is_none());
    assert_eq!(
        settled.event.as_ref().expect("Neow leave screen").choices[0].label,
        "Leave"
    );
}

#[test]
fn reward_choose_routes_a_bottle_overlay_to_the_active_grid() {
    let mut run = RunState::map_fixture();
    run.current_room_override = Some(RoomKind::Treasure);
    run.phase = RunPhase::Treasure;
    sts_core::enter_chest_relic_reward_screen(&mut run).expect("chest reward opens");
    run.reward
        .as_mut()
        .expect("chest reward screen")
        .relic_offer = Some(Relic::BottledFlame);

    let opened = sts_core::apply_run_action(&run, RunAction::TakeRelicReward)
        .expect("bottled relic pickup opens its card grid");
    assert!(opened.card_grid.is_some());
    assert_eq!(opened.phase, RunPhase::Reward);

    let (action, label) = replay::direct_decision(&opened, "CHOOSE 0")
        .expect("Reward CHOOSE binds to the active bottle grid");
    assert_eq!(label, "direct Reward transition");
    assert_eq!(action, RunDecisionAction::GridSelect { index: 0 });

    let settled = apply_run_decision_action(&opened, action)
        .expect("bottle grid selection settles through the authoritative boundary");
    assert!(settled.card_grid.is_none());
    assert!(settled.deck.iter().any(|card| card.bottled));
}
