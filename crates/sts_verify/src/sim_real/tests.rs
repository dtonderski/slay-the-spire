use super::*;
use serde_json::json;
use std::collections::VecDeque;

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
        "command_execution_seq": 1,
        "effects_size": 0,
        "top_level_effects_size": 0,
        "queued_top_level_effects_size": 0,
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
fn observed_upgrade_count_drives_repeated_self_upgrade_identity() {
    let repeated = json!({
        "id": "Searing Blow",
        "name": "misleading display name",
        "upgrades": 2
    });
    assert_eq!(
        content_id_from_card_value(&repeated),
        Some(sts_core::content::cards::SEARING_BLOW_PLUS_ID)
    );
    assert_eq!(
        observed_card_projection_key(&repeated).as_deref(),
        Some("Searing Blow+2")
    );

    let one = json!({
        "id": "Searing Blow",
        "name": "also misleading",
        "upgrades": 1
    });
    assert_eq!(
        observed_card_projection_key(&one).as_deref(),
        Some("Searing Blow+")
    );
}

#[test]
fn simulated_upgrade_count_drives_repeated_self_upgrade_projection() {
    let mut repeated = CardInstance::new(
        sts_core::CardId::new(1),
        sts_core::content::cards::SEARING_BLOW_PLUS_ID,
    );
    repeated.searing_blow_upgrades = 2;
    assert_eq!(simulated_card_projection_key(&repeated), "Searing Blow+2");
}

#[test]
fn compact_any_color_deck_ids_project_to_spaced_display_names() {
    let empty_body = json!({
        "id": "EmptyBody",
        "name": "Empty Body",
        "upgrades": 0
    });
    assert_eq!(
        observed_card_projection_key(&empty_body).as_deref(),
        Some("Empty Body")
    );
    let content_id = content_id_from_card_value(&empty_body).expect("EmptyBody maps");
    let simulated = CardInstance::new(sts_core::CardId::new(1), content_id);
    assert_eq!(simulated_card_projection_key(&simulated), "Empty Body");
}

#[test]
fn combat_player_debuff_projection_uses_communication_mod_power_ids() {
    let observed = seed_start_combat_observed_subset(&json!({
        "game_state": {
            "screen_type": "NONE",
            "combat_state": {
                "player": {
                    "current_hp": 90,
                    "block": 3,
                    "energy": 2,
                    "powers": [
                        {"id": "Frail", "amount": 1},
                        {"id": "Weakened", "amount": 2},
                        {"id": "Vulnerable", "amount": 3},
                        {"id": "Artifact", "amount": 4}
                    ]
                },
                "hand": [],
                "draw_pile": [],
                "discard_pile": [],
                "monsters": []
            }
        }
    }));
    assert_eq!(observed["combat_player_frail"], 1);
    assert_eq!(observed["combat_player_weak"], 2);
    assert_eq!(observed["combat_player_vulnerable"], 3);
    assert_eq!(observed["combat_player_artifact"], 4);

    let mut run = RunState::combat_fixture();
    let combat = run.combat.as_mut().expect("combat");
    combat.player.powers.frail = 1;
    combat.player.powers.weak = 2;
    combat.player.powers.vulnerable = 3;
    combat.player.powers.artifact = 4;
    let simulated = seed_start_simulated_combat_subset(&run);
    for field in [
        "combat_player_frail",
        "combat_player_weak",
        "combat_player_vulnerable",
        "combat_player_artifact",
    ] {
        assert_eq!(simulated[field], observed[field], "{field}");
    }
}

#[test]
fn dead_monster_power_projection_uses_the_documented_visibility_boundary() {
    let observed_game = json!({
        "monsters": [
            {
                "name": "living",
                "current_hp": 10,
                "max_hp": 20,
                "block": 0,
                "intent": "ATTACK",
                "move_id": 1,
                "powers": [
                    {"id": "Strength", "amount": 3},
                    {"id": "Ritual", "amount": 5},
                    {"id": "Vulnerable", "amount": 2}
                ]
            },
            {
                "name": "dead",
                "current_hp": 20,
                "max_hp": 20,
                "is_gone": true,
                "block": 0,
                "intent": "ATTACK",
                "move_id": 1,
                "powers": [
                    {"id": "Strength", "amount": 7},
                    {"id": "Ritual", "amount": 4},
                    {"id": "Vulnerable", "amount": 1}
                ]
            }
        ]
    });
    let observed = seed_start_monsters_from_value(observed_game.get("monsters"), true);
    assert_eq!(observed[0]["strength"], json!(3));
    assert_eq!(observed[0]["ritual"], json!(5));
    assert_eq!(observed[0]["vulnerable"], json!(2));
    assert!(observed[1].get("strength").is_none());
    assert!(observed[1].get("ritual").is_none());
    assert!(observed[1].get("vulnerable").is_none());

    let mut combat = CombatState::initial_fixture();
    combat.monsters[0].alive = false;
    combat.monsters[0].hp = 0;
    combat.monsters[0].powers.strength = 7;
    combat.monsters[0].powers.ritual = 4;
    combat.monsters[0].powers.vulnerable = 1;
    let simulated = seed_start_monsters_from_sim(&combat, true);
    assert!(simulated[0].get("strength").is_none());
    assert!(simulated[0].get("ritual").is_none());
    assert!(simulated[0].get("vulnerable").is_none());
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
    assert!(error.to_string().contains("boundary schema"));
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
    assert!(error.to_string().contains("boundary_schema"));
}

#[test]
fn state_boundary_schema_must_match_metadata() {
    let content = trace(vec![
        metadata(Some(2), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":boundary_message("quiescent")}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema-1 state under schema-2 metadata must fail");
    assert!(matches!(
        error,
        SimRealError::InvalidBoundaryContract { step: 1, .. }
    ));
    assert!(error.to_string().contains("must match metadata"));
}

#[test]
fn quiescent_schemas_require_end_turn_queued() {
    for schema in [2, 3, 4, 5, 6, 7] {
        let mut message = boundary_message("quiescent");
        message["boundary_schema"] = json!(schema);
        let action = if schema == 7 {
            json!({
                "type":"action","step":1,"command":"START IRONCLAD 0 1",
                "command_meta":{
                    "command_id":"start-1",
                    "source_command_execution_seq":0,
                    "source_command_settlement_seq":0
                }
            })
        } else {
            json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"})
        };
        let content = trace(vec![
            metadata(Some(schema), true),
            action,
            json!({"type":"state","step":1,"message":message}),
        ]);
        let error = verify_communication_mod_trace(&content)
            .expect_err("quiescent schema without end_turn_queued must fail");
        assert!(error
            .to_string()
            .contains("requires boolean end_turn_queued"));
    }
}

#[test]
fn schema_five_requires_command_execution_sequence() {
    let mut message = boundary_message("quiescent");
    message["boundary_schema"] = json!(5);
    message["end_turn_queued"] = json!(false);
    message
        .as_object_mut()
        .expect("boundary object")
        .remove("command_execution_seq");
    let content = trace(vec![
        metadata(Some(5), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":message}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema 5 without command_execution_seq must fail");
    assert!(error.to_string().contains("command_execution_seq"));
}

#[test]
fn schema_six_requires_empty_effect_queues() {
    let mut message = boundary_message("quiescent");
    message["boundary_schema"] = json!(6);
    message["end_turn_queued"] = json!(false);
    message["effects_size"] = json!(1);
    let content = trace(vec![
        metadata(Some(6), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":message}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema 6 with an active effect must fail");
    assert!(error.to_string().contains("effects_size=0"));
}

#[test]
fn schema_five_rejects_nonadvancing_gameplay_fence() {
    let mut first = boundary_message("quiescent");
    first["boundary_schema"] = json!(5);
    first["end_turn_queued"] = json!(false);
    first["command_execution_seq"] = json!(7);
    let mut stale = first.clone();
    stale["command_execution_seq"] = json!(7);
    let content = trace(vec![
        metadata(Some(5), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1","command_meta":{"source_command_execution_seq":6}}),
        json!({"type":"state","step":1,"message":first}),
        json!({"type":"action","step":2,"command":"CHOOSE 0","command_meta":{"source_command_execution_seq":7}}),
        json!({"type":"state","step":2,"message":stale}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema 5 gameplay sequence must advance");
    assert!(error.to_string().contains("did not advance"));
}

#[test]
fn schema_six_rejects_stale_completion_after_rejected_command_fence() {
    let mut first = boundary_message("quiescent");
    first["boundary_schema"] = json!(6);
    first["end_turn_queued"] = json!(false);
    first["command_execution_seq"] = json!(8);
    let mut stale = first.clone();
    stale["command_execution_seq"] = json!(9);
    let content = trace(vec![
        metadata(Some(6), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1","command_meta":{"source_command_execution_seq":7}}),
        json!({"type":"state","step":1,"message":first}),
        json!({"type":"action","step":2,"command":"CHOOSE 99","command_meta":{"source_command_execution_seq":8}}),
        json!({"type":"error","step":2,"message":{"error":"rejected","command_execution_seq":9}}),
        json!({"type":"action","step":3,"command":"CHOOSE 0","command_meta":{"source_command_execution_seq":9}}),
        json!({"type":"state","step":3,"message":stale}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("completion equal to its action source fence must fail");
    assert!(error.to_string().contains("did not advance beyond"));
}

fn schema7_quiescent(command_id: &str, execution: u64, settlement: u64) -> Value {
    let mut message = boundary_message("quiescent");
    message["boundary_schema"] = json!(7);
    message["end_turn_queued"] = json!(false);
    message["command_execution_seq"] = json!(execution);
    message["command_settlement_seq"] = json!(settlement);
    message["command_response_id"] = json!(command_id);
    message["command_response_kind"] = json!("settled");
    message["transaction_pending"] = json!(false);
    message
}

#[test]
fn schema_seven_requires_exact_identity_and_sequence_deltas() {
    let first = schema7_quiescent("start-1", 1, 1);
    let second = schema7_quiescent("choose-1", 2, 2);
    let content = trace(vec![
        metadata(Some(7), true),
        json!({
            "type":"action","step":1,"command":"START IRONCLAD 0 1",
            "command_meta":{
                "command_id":"start-1",
                "source_command_execution_seq":0,
                "source_command_settlement_seq":0
            }
        }),
        json!({"type":"state","step":1,"message":first}),
        json!({
            "type":"action","step":2,"command":"CHOOSE 0",
            "command_meta":{
                "command_id":"choose-1",
                "source_command_execution_seq":1,
                "source_command_settlement_seq":1
            }
        }),
        json!({"type":"state","step":2,"message":second}),
    ]);
    verify_communication_mod_trace(&content).expect("schema 7 exact deltas verify");
}

#[test]
fn schema_seven_state_poll_preserves_sequences_and_echoes_identity() {
    let first = schema7_quiescent("start-1", 1, 1);
    let mut poll = schema7_quiescent("state-1", 1, 1);
    poll["boundary_kind"] = json!("poll");
    poll["command_response_kind"] = json!("poll");
    let content = trace(vec![
        metadata(Some(7), true),
        json!({
            "type":"action","step":1,"command":"START IRONCLAD 0 1",
            "command_meta":{
                "command_id":"start-1",
                "source_command_execution_seq":0,
                "source_command_settlement_seq":0
            }
        }),
        json!({"type":"state","step":1,"message":first}),
        json!({
            "type":"action","step":2,"command":"STATE",
            "command_meta":{
                "command_id":"state-1",
                "source_command_execution_seq":1,
                "source_command_settlement_seq":1
            }
        }),
        json!({"type":"state","step":2,"message":poll}),
    ]);
    verify_communication_mod_trace(&content).expect("schema 7 STATE poll verifies");
}

#[test]
fn schema_seven_rejects_duplicate_command_identity() {
    let first = schema7_quiescent("command-1", 1, 1);
    let second = schema7_quiescent("command-1", 2, 2);
    let content = trace(vec![
        metadata(Some(7), true),
        json!({
            "type":"action","step":1,"command":"START IRONCLAD 0 1",
            "command_meta":{
                "command_id":"command-1",
                "source_command_execution_seq":0,
                "source_command_settlement_seq":0
            }
        }),
        json!({"type":"state","step":1,"message":first}),
        json!({
            "type":"action","step":2,"command":"CHOOSE 0",
            "command_meta":{
                "command_id":"command-1",
                "source_command_execution_seq":1,
                "source_command_settlement_seq":1
            }
        }),
        json!({"type":"state","step":2,"message":second}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema 7 duplicate command identity must fail");
    assert!(error.to_string().contains("duplicate schema-7 command_id"));
}

#[test]
fn schema_seven_rejects_wrong_completion_identity() {
    let message = schema7_quiescent("other", 1, 1);
    let content = trace(vec![
        metadata(Some(7), true),
        json!({
            "type":"action","step":1,"command":"START IRONCLAD 0 1",
            "command_meta":{
                "command_id":"start-1",
                "source_command_execution_seq":0,
                "source_command_settlement_seq":0
            }
        }),
        json!({"type":"state","step":1,"message":message}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema 7 mismatched command id must fail");
    assert!(error.to_string().contains("identity mismatch"));
}

#[test]
fn schema_seven_rejection_advances_execution_only() {
    let first = schema7_quiescent("start-1", 1, 1);
    let content = trace(vec![
        metadata(Some(7), true),
        json!({
            "type":"action","step":1,"command":"START IRONCLAD 0 1",
            "command_meta":{
                "command_id":"start-1",
                "source_command_execution_seq":0,
                "source_command_settlement_seq":0
            }
        }),
        json!({"type":"state","step":1,"message":first}),
        json!({
            "type":"action","step":2,"command":"CHOOSE 99",
            "command_meta":{
                "command_id":"bad-1",
                "source_command_execution_seq":1,
                "source_command_settlement_seq":1
            }
        }),
        json!({
            "type":"error","step":2,
            "message":{
                "error":"rejected",
                "boundary_schema":7,
                "command_response_id":"bad-1",
                "command_response_kind":"rejected",
                "command_execution_seq":2,
                "command_settlement_seq":1,
                "transaction_pending":false
            }
        }),
    ]);
    verify_communication_mod_trace(&content).expect("schema 7 rejection sequences verify");
}

#[test]
fn schema_seven_observation_rejection_leaves_both_sequences_unchanged() {
    let first = schema7_quiescent("start-1", 1, 1);
    let content = trace(vec![
        metadata(Some(7), true),
        json!({
            "type":"action","step":1,"command":"START IRONCLAD 0 1",
            "command_meta":{
                "command_id":"start-1",
                "source_command_execution_seq":0,
                "source_command_settlement_seq":0
            }
        }),
        json!({"type":"state","step":1,"message":first}),
        json!({
            "type":"action","step":2,"command":"STATE",
            "command_meta":{
                "command_id":"state-1",
                "source_command_execution_seq":1,
                "source_command_settlement_seq":1
            }
        }),
        json!({
            "type":"error","step":2,
            "message":{
                "error":"rejected",
                "boundary_schema":7,
                "command_response_id":"state-1",
                "command_response_kind":"rejected",
                "command_execution_seq":1,
                "command_settlement_seq":1,
                "transaction_pending":false
            }
        }),
    ]);
    verify_communication_mod_trace(&content)
        .expect("schema 7 observation rejection sequences verify");
}

#[test]
fn schema_five_gameplay_action_requires_source_fence() {
    let mut message = boundary_message("quiescent");
    message["boundary_schema"] = json!(5);
    message["end_turn_queued"] = json!(false);
    message["command_execution_seq"] = json!(1);
    let content = trace(vec![
        metadata(Some(5), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
        json!({"type":"state","step":1,"message":message}),
    ]);
    let error = verify_communication_mod_trace(&content)
        .expect_err("schema-5 gameplay without an action source fence must fail");
    assert!(error
        .to_string()
        .contains("command_meta.source_command_execution_seq"));
}

#[test]
fn quiescent_ready_state_rejects_queued_end_turn() {
    for schema in [2, 3, 4, 5, 6] {
        let mut message = boundary_message("quiescent");
        message["boundary_schema"] = json!(schema);
        message["end_turn_queued"] = json!(true);
        let content = trace(vec![
            metadata(Some(schema), true),
            json!({"type":"action","step":1,"command":"START IRONCLAD 0 1"}),
            json!({"type":"state","step":1,"message":message}),
        ]);
        let error = verify_communication_mod_trace(&content)
            .expect_err("ready quiescent state with queued end turn must fail");
        assert!(error.to_string().contains("cannot have an end turn queued"));
    }
}

#[test]
fn interaction_ready_state_allows_queued_end_turn() {
    let mut message = boundary_message("interaction_ready");
    message["boundary_schema"] = json!(6);
    message["end_turn_queued"] = json!(true);
    message["current_action"] = json!("CodexAction");
    message["current_action_instance"] = json!(5);
    message["current_action_update_count"] = json!(1);
    message["actions_queued"] = json!(1);
    let content = trace(vec![
        metadata(Some(6), true),
        json!({"type":"action","step":1,"command":"START IRONCLAD 0 1","command_meta":{"source_command_execution_seq":0}}),
        json!({"type":"state","step":1,"message":message}),
    ]);
    verify_communication_mod_trace(&content)
        .expect("a Codex decision may pause a queued end turn at an interaction boundary");
}

#[test]
fn death_terminal_state_allows_queued_end_turn_and_residual_combat_work() {
    let mut message = boundary_message("terminal");
    message["boundary_schema"] = json!(6);
    message["in_game"] = json!(true);
    message["game_state"] = json!({"screen_type":"GAME_OVER"});
    message["end_turn_queued"] = json!(true);
    message["current_action"] = json!("DamageAction");
    message["current_action_instance"] = json!(5);
    message["current_action_update_count"] = json!(7);
    message["actions_queued"] = json!(1);
    message["card_queue_size"] = json!(3);
    let mut state = TraceState {
        step: 1,
        received_at: None,
        message,
    };

    assert_eq!(
        validate_boundary_state(&state, 6)
            .expect("the target publishes GAME_OVER before its combat queues drain"),
        "terminal"
    );

    state.message["game_state"]["screen_type"] = json!("NONE");
    let error = validate_boundary_state(&state, 6)
        .expect_err("a non-death terminal label cannot bypass end-turn quiescence");
    assert!(error.to_string().contains("cannot have an end turn queued"));
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
fn forethought_multi_select_hides_selected_cards_from_hand_projection() {
    let mut run = RunState::combat_fixture();
    let (hand_len, selected_key) = {
        let combat = run.combat.as_ref().expect("combat fixture");
        (
            combat.piles.hand.len(),
            simulated_card_projection_key(&combat.piles.hand[1]),
        )
    };
    run.combat.as_mut().expect("combat fixture").decision = Some(CombatDecisionState::HandSelect {
        state: sts_core::combat::HandSelectState {
            purpose: HandSelectPurpose::ForethoughtPutAnyOnDraw,
            source_card_id: CardId::new(999_999),
            selected_hand_index: None,
            selected_hand_indices: vec![1],
            dual_wield_restore_on_confirm: Vec::new(),
            dual_wield_force_exhaust: false,
        },
        pending_actions: VecDeque::new(),
    });

    let projected = seed_start_simulated_combat_subset(&run);
    let hand = projected["hand_ids"].as_array().expect("hand projection");
    assert_eq!(hand.len(), hand_len - 1);
    assert!(!hand.iter().any(|card| card == &json!(selected_key)));
}

#[test]
fn armaments_select_keeps_ritual_dagger_in_hand_projection() {
    // Ritual Dagger upgrades in place (no + content id). CommunicationMod still
    // lists it in combat_state.hand while ArmamentsAction is open (FIDL01774).
    let mut run = RunState::combat_fixture();
    {
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.piles.hand.push(CardInstance::new(
            CardId::new(10_001),
            sts_core::content::cards::RITUAL_DAGGER_ID,
        ));
        combat.piles.hand.push(CardInstance::new(
            CardId::new(10_002),
            sts_core::content::cards::DAZED_ID,
        ));
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: sts_core::combat::HandSelectState {
                purpose: HandSelectPurpose::ArmamentsUpgrade,
                source_card_id: CardId::new(999_999),
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: VecDeque::new(),
        });
    }

    let projected = seed_start_simulated_combat_subset(&run);
    let hand = projected["hand_ids"].as_array().expect("hand projection");
    assert!(
        hand.iter().any(|card| card == &json!("Ritual Dagger")),
        "Ritual Dagger must remain visible: {hand:?}"
    );
    assert!(
        !hand.iter().any(|card| card == &json!("Dazed")),
        "statuses are not Armaments-upgradeable: {hand:?}"
    );
}

#[test]
fn spire_heart_sleep_uses_complete_phase_for_terminal_game_over_projection() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.current_act = 3;
    run.current_floor = 51;
    run.current_room_override = Some(RoomKind::Victory);
    run.phase = RunPhase::Event;
    let mut heart = sts_core::event_screen(Event::SpireHeart);
    heart.stage = 3;
    heart.choices = vec![sts_core::EventChoice {
        label: "Sleep".to_owned(),
    }];
    run.event = Some(heart);

    let completed = apply_run_decision_action(
        &run,
        RunDecisionAction::Event(sts_core::EventAction::Choose { choice_index: 0 }),
    )
    .expect("Spire Heart Sleep enters the terminal core phase");

    assert_eq!(completed.phase, RunPhase::Complete);
    assert_eq!(completed.current_room_kind(), Some(RoomKind::Victory));
    assert!(legal_run_decision_actions(&completed)
        .expect("terminal legal actions")
        .is_empty());
    completed
        .validate()
        .expect("terminal Heart state validates");
    assert_eq!(
        seed_start_complete_simulated_subset(&completed),
        json!({
            "screen_type": "GAME_OVER",
            "floor": 51,
            "gold": completed.gold,
            "current_hp": completed.player_hp,
            "max_hp": completed.player_max_hp,
        })
    );

    let mut pre_heart = completed.clone();
    pre_heart.phase = RunPhase::Victory;
    pre_heart.current_floor = 50;
    pre_heart.current_room_override = Some(RoomKind::Boss);
    pre_heart.event = None;
    assert_eq!(
        seed_start_victory_simulated_subset(&pre_heart)["screen_type"],
        json!("COMPLETE")
    );
}

#[test]
fn forgotten_altar_offer_projection_uses_relic_specific_commmod_label() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.relics.push(Relic::GoldenIdol);
    run.event = Some(sts_core::run::event::event_screen_for_run(
        &run,
        Event::ForgottenAltar,
    ));

    assert_eq!(
        run.event
            .as_ref()
            .expect("Forgotten Altar event")
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Offer", "Sacrifice", "Desecrate"]
    );
    assert_eq!(
        seed_start_event_simulated_subset(&run)["choices"],
        json!(["offer: golden idol", "sacrifice", "desecrate"])
    );
    assert_eq!(
        seed_start_visible_event_choice_label_for_event(Event::ForgottenAltar, 0, "Offer"),
        Some("offer: golden idol".to_owned())
    );

    let mut without_idol = RunState::seeded_ironclad(1, 0);
    without_idol.phase = RunPhase::Event;
    without_idol.event = Some(sts_core::run::event::event_screen_for_run(
        &without_idol,
        Event::ForgottenAltar,
    ));
    assert_eq!(
        seed_start_event_simulated_subset(&without_idol)["choices"],
        json!(["sacrifice", "desecrate"])
    );
}

#[test]
fn match_and_keep_revealed_cards_use_source_card_ids() {
    let ordinary = [
        (sts_core::content::cards::BANDAGE_UP_ID, "bandage up"),
        (sts_core::content::cards::SEVER_SOUL_ID, "sever soul"),
        (sts_core::content::cards::SPOT_WEAKNESS_ID, "spot weakness"),
        (
            sts_core::content::cards::PERFECTED_STRIKE_ID,
            "perfected strike",
        ),
        (
            sts_core::content::cards::DRAMATIC_ENTRANCE_ID,
            "dramatic entrance",
        ),
        (sts_core::content::cards::MIND_BLAST_ID, "mind blast"),
        (sts_core::content::cards::PANIC_BUTTON_ID, "panicbutton"),
    ];
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.match_and_keep = Some(sts_core::run::event::MatchAndKeepState {
        cards: ordinary
            .iter()
            .map(|(content_id, _)| sts_core::run::event::MatchAndKeepCard {
                content_id: *content_id,
                revealed: true,
                matched: false,
            })
            .collect(),
        attempts_remaining: 5,
        first_flipped_index: None,
        second_flipped_index: None,
        matched_cards: Vec::new(),
        game_done: false,
    });
    run.event = Some(EventScreen {
        event: Event::MatchAndKeep,
        choices: vec![sts_core::EventChoice {
            label: "choice text is not board authority".to_owned(),
        }],
        stage: 2,
        event_data: 0,
    });

    assert_eq!(
        seed_start_event_simulated_subset(&run)["choices"],
        json!(ordinary
            .iter()
            .map(|(_, expected)| *expected)
            .collect::<Vec<_>>())
    );
    assert_eq!(
        seed_start_communication_mod_card_id(sts_core::content::cards::PANIC_BUTTON_ID),
        "panicbutton"
    );
}

#[test]
fn match_and_keep_presentation_preserves_hidden_slots_and_board_authority() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.match_and_keep = Some(sts_core::run::event::MatchAndKeepState {
        cards: vec![
            sts_core::run::event::MatchAndKeepCard {
                content_id: sts_core::content::cards::PANIC_BUTTON_ID,
                revealed: true,
                matched: false,
            },
            sts_core::run::event::MatchAndKeepCard {
                content_id: sts_core::content::cards::BANDAGE_UP_ID,
                revealed: false,
                matched: false,
            },
            sts_core::run::event::MatchAndKeepCard {
                content_id: sts_core::content::cards::SEVER_SOUL_ID,
                revealed: true,
                matched: true,
            },
            sts_core::run::event::MatchAndKeepCard {
                content_id: sts_core::content::cards::SPOT_WEAKNESS_ID,
                revealed: true,
                matched: false,
            },
        ],
        attempts_remaining: 5,
        first_flipped_index: Some(3),
        second_flipped_index: None,
        matched_cards: vec![sts_core::content::cards::SEVER_SOUL_ID],
        game_done: false,
    });
    run.event = Some(EventScreen {
        event: Event::MatchAndKeep,
        choices: vec![sts_core::EventChoice {
            label: "observed choice text must not select cards".to_owned(),
        }],
        stage: 2,
        event_data: 0,
    });

    assert_eq!(
        seed_start_event_simulated_subset(&run)["choices"],
        json!(["panicbutton", "card1"])
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
