use super::*;
use std::collections::VecDeque;
use sts_core::content::cards::{
    BASH_PLUS_ID, BATTLE_TRANCE_ID, BURN_ID, COMBUST_ID, CORRUPTION_PLUS_ID, DEFEND_R_ID,
    DEMON_FORM_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID, ENTRENCH_ID, POMMEL_STRIKE_PLUS_ID,
    STRIKE_R_PLUS_ID, TWIN_STRIKE_ID,
};
use sts_core::content::monsters::{monster_state, SLAVER_BLUE_A0};
use sts_core::relic::IRONCLAD_BOSS_RELIC_POOL;

fn serialize_trace_test_lines(mut lines: Vec<Value>) -> String {
    let act_boss = lines
        .iter()
        .find_map(|line| {
            let command = line.get("command")?.as_str()?;
            let step = u32::try_from(line.get("step")?.as_u64()?).ok()?;
            parse_start_command(&TraceAction {
                step,
                command: command.to_owned(),
                sent_at: None,
                playtime_seconds: None,
            })?
            .ok()
        })
        .map(|start| {
            target_exordium_act_one_boss_with_unlocks(
                start.numeric_seed,
                BossUnlockState::default(),
            )
            .to_owned()
        });
    for line in &mut lines {
        let Some(game) = line
            .get_mut("message")
            .and_then(|message| message.get_mut("game_state"))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        game.entry("potions").or_insert_with(|| json!([]));
        if let Some(deck) = game.get_mut("deck").and_then(Value::as_array_mut) {
            for card in deck {
                card.as_object_mut()
                    .expect("test deck card is an object")
                    .entry("upgrades")
                    .or_insert_with(|| json!(0));
            }
        }
        if let Some(combat) = game.get_mut("combat_state").and_then(Value::as_object_mut) {
            for pile in ["hand", "draw_pile", "discard_pile"] {
                if let Some(cards) = combat.get_mut(pile).and_then(Value::as_array_mut) {
                    for card in cards {
                        card.as_object_mut()
                            .expect("test combat card is an object")
                            .entry("upgrades")
                            .or_insert_with(|| json!(0));
                    }
                }
            }
        }
        if let Some(act_boss) = &act_boss {
            game.entry("act_boss").or_insert_with(|| json!(act_boss));
        }
        let screen_type = game
            .get("screen_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let event_options = game
            .get("choice_list")
            .and_then(Value::as_array)
            .map(|choices| {
                choices
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|text| json!({ "text": text }))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if matches!(
            screen_type.as_str(),
            "BOSS_REWARD" | "CARD_REWARD" | "GRID" | "MAP"
        ) {
            game.entry("choice_list").or_insert_with(|| json!([]));
        }
        if matches!(
            screen_type.as_str(),
            "BOSS_REWARD" | "CARD_REWARD" | "COMBAT_REWARD" | "EVENT" | "GRID" | "MAP"
        ) {
            let screen = game
                .entry("screen_state")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .expect("test screen state is an object");
            match screen_type.as_str() {
                "CARD_REWARD" => {
                    let cards = screen
                        .entry("cards")
                        .or_insert_with(|| json!([]))
                        .as_array_mut()
                        .expect("test card choices are an array");
                    for card in cards {
                        card.as_object_mut()
                            .expect("test card choice is an object")
                            .entry("upgrades")
                            .or_insert_with(|| json!(0));
                    }
                }
                "BOSS_REWARD" => {
                    screen.entry("relics").or_insert_with(|| json!([]));
                }
                "COMBAT_REWARD" => {
                    let rewards = screen
                        .entry("rewards")
                        .or_insert_with(|| json!([]))
                        .as_array_mut()
                        .expect("test rewards are an array");
                    for reward in rewards {
                        let reward = reward.as_object_mut().expect("test reward is an object");
                        match reward.get("reward_type").and_then(Value::as_str) {
                            Some("GOLD" | "STOLEN_GOLD") => {
                                reward.entry("gold").or_insert_with(|| json!(0));
                            }
                            Some("POTION") => {
                                reward
                                    .entry("potion")
                                    .or_insert_with(|| json!({ "id": "Unknown Potion" }));
                            }
                            Some("RELIC") => {
                                reward
                                    .entry("relic")
                                    .or_insert_with(|| json!({ "id": "Unknown Relic" }));
                            }
                            _ => {}
                        }
                    }
                }
                "EVENT" => {
                    screen
                        .entry("event_id")
                        .or_insert_with(|| json!("Neow Event"));
                    screen
                        .entry("options")
                        .or_insert_with(|| json!(event_options));
                }
                "MAP" => {
                    screen.entry("next_nodes").or_insert_with(|| json!([]));
                    screen
                        .entry("first_node_chosen")
                        .or_insert_with(|| json!(false));
                    screen
                        .entry("current_node")
                        .or_insert_with(|| json!({ "x": 0, "y": -1 }));
                }
                "GRID" => {
                    screen.entry("cards").or_insert_with(|| json!([]));
                    screen.entry("selected_cards").or_insert_with(|| json!([]));
                    for field in [
                        "confirm_up",
                        "for_purge",
                        "for_transform",
                        "for_upgrade",
                        "any_number",
                    ] {
                        screen.entry(field).or_insert_with(|| json!(false));
                    }
                    screen.entry("num_cards").or_insert_with(|| json!(1));
                }
                _ => unreachable!(),
            }
        }
    }
    lines
        .into_iter()
        .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn pre_run_profile_sets_note_card_without_observation_input() {
    let profile = TraceProfile {
        note_card: "Twin Strike".to_owned(),
        note_upgrades: 1,
    };
    let mut run = RunState::seeded_ironclad(1, 0);
    replay::seed_start_apply_note_profile_for_test(&mut run, Some(&profile));
    assert_eq!(run.note_card_content_id, TWIN_STRIKE_ID);
    assert_eq!(run.note_card_upgrades, 1);
}

fn observed_deck_cards(cards: &[CardInstance]) -> Vec<Value> {
    cards
        .iter()
        .map(|card| {
            let upgrades = card
                .upgrades
                .max(card.searing_blow_upgrades)
                .max(u8::from(card_content_id_is_upgraded(card.content_id)));
            json!({
                "id": deck_content_key(card.content_id),
                "upgrades": upgrades,
            })
        })
        .collect()
}

fn forge_grid_observation(message: &mut Value) {
    let game = message
        .get_mut("game_state")
        .and_then(Value::as_object_mut)
        .expect("forged observation has game state");
    game.insert("screen_type".to_owned(), json!("GRID"));
    game.insert("choice_list".to_owned(), json!(["Strike"]));
    game.insert(
        "screen_state".to_owned(),
        json!({
            "cards": [{ "id": "Strike_R" }],
            "selected_cards": [],
            "confirm_up": false,
            "for_purge": false,
            "for_transform": false,
            "for_upgrade": false,
            "any_number": false,
            "num_cards": 1,
        }),
    );
}

fn forge_boss_reward_observation(message: &mut Value) {
    let game = message
        .get_mut("game_state")
        .and_then(Value::as_object_mut)
        .expect("forged observation has game state");
    game.insert("screen_type".to_owned(), json!("BOSS_REWARD"));
    game.insert("choice_list".to_owned(), json!(["Black Blood"]));
    game.insert(
        "screen_state".to_owned(),
        json!({ "relics": [{ "name": "Black Blood" }] }),
    );
}

fn forge_rest_observation(message: &mut Value) {
    let game = message
        .get_mut("game_state")
        .and_then(Value::as_object_mut)
        .expect("forged observation has game state");
    game.insert("screen_type".to_owned(), json!("REST"));
    game.insert("choice_list".to_owned(), json!(["rest"]));
    game.insert(
        "screen_state".to_owned(),
        json!({ "has_rested": false, "rest_options": ["rest"] }),
    );
}

#[test]
fn unknown_boss_relic_identity_remains_visible() {
    let game = json!({
        "screen_type": "BOSS_REWARD",
        "screen_state": {
            "relics": [{ "name": "Future Relic" }],
        },
    });

    assert_eq!(
        observed_boss_relic_choice_ids(&game),
        vec!["Future Relic".to_owned()]
    );
}

#[test]
fn smoke_bomb_transient_projection_preserves_the_core_destination() {
    let mut source = RunState::map_fixture();
    source.player_hp = 10;
    source.player_max_hp = 80;
    source.potions = vec![Potion::SmokeBomb];
    source.phase = RunPhase::Combat;
    let mut combat = source
        .init_combat(CombatState::initial_fixture())
        .expect("combat initializes");
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
        crate::load_corpus_file("permanent_traces/trace-2026-07-07T18-33-54-807Z.jsonl")
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
fn smoke_bomb_hidden_gold_rng_keeps_later_combat_reward_aligned() {
    let Some(content) = crate::load_corpus_file(
        "fidelity_regressions/random-fidelity-fidl00001-escaped-gold-rng.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("escaped Looter gold RNG regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report.action_dispositions.iter().any(|entry| {
        entry.action_step == 843
            && entry.command == "PLAY 2 0"
            && entry.disposition == ActionDispositionKind::Verified
    }));
}

#[test]
fn smoke_bomb_queued_end_resolves_the_source_combat_before_escape() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-665dff26cbb6e5f4.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Smoke Bomb queued END regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    for step in [86, 87] {
        assert_eq!(
            report
                .action_dispositions
                .iter()
                .find(|entry| entry.action_step == step)
                .map(|entry| entry.disposition),
            Some(ActionDispositionKind::Verified),
            "Smoke Bomb queued END step {step}"
        );
    }
}

#[test]
fn smoke_bomb_queued_end_applies_victory_heal_before_late_enemy_turn() {
    let mut source = RunState::combat_fixture_with_relics(vec![Relic::BurningBlood]);
    source.potions = vec![Potion::SmokeBomb];
    source.empty_potion_slots = vec![1, 2];
    source.player_hp = 80;
    source.player_max_hp = 80;
    let combat = source.combat.as_mut().expect("combat fixture");
    combat.player.hp = 80;
    combat.player.max_hp = 80;
    combat.player.block = 0;

    let destination = apply_run_action(
        &source,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .expect("Smoke Bomb reaches its core destination");
    assert_eq!(destination.player_hp, 80);

    let queued_destination =
        super::replay::seed_start_smoke_bomb_queued_end_destination(&source, &destination)
            .expect("queued END resolves after the escape heal");
    assert_eq!(
        queued_destination.player_hp, 74,
        "Burning Blood is capped at full HP before the queued six-damage enemy turn"
    );
}

#[test]
fn smoke_bomb_event_queued_end_does_not_start_a_late_enemy_turn() {
    let mut source = RunState::combat_fixture_with_relics(vec![Relic::BurningBlood]);
    source.current_room_override = Some(RoomKind::Event);
    source.potions = vec![Potion::SmokeBomb];
    source.empty_potion_slots = vec![1, 2];

    let destination = apply_run_action(
        &source,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .expect("Smoke Bomb reaches its event-combat destination");

    let queued_destination =
        super::replay::seed_start_smoke_bomb_queued_end_destination(&source, &destination)
            .expect("event queued END reaches the empty reward frame");
    assert_eq!(queued_destination.player_hp, destination.player_hp);
    assert_eq!(queued_destination.phase, RunPhase::Idle);
}

#[test]
fn smoke_bomb_trace_can_end_at_a_verified_escape_transient() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-0e918c922e0616f8.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Smoke Bomb transient-end trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    let disposition = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 134)
        .expect("Smoke Bomb action disposition");
    assert_eq!(disposition.disposition, ActionDispositionKind::Verified);
    assert!(disposition.deferred_assertion_reconciled);
}

#[test]
fn reactive_thorns_preserve_a_queued_dead_monster_roll() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-9ef59d65e4f6728e.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Jaw Worm reactive-thorns trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn the_joust_trace_uses_target_bet_labels() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-bc1722606a474ee9.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("The Joust trace verifies through the bet screen");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn the_joust_trace_uses_target_watch_label_after_bet() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-a46dfdb55ac478a3.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("The Joust trace verifies through the watch screen");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn snecko_oil_keeps_the_following_juggernaut_target_in_sync() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-2589152bd2f3b1b7.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Snecko Oil/Juggernaut trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn smoke_bomb_trace_reconciles_queued_combat_command_at_stable_reward() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-7a15d436727123b4.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("queued Smoke Bomb command regression trace verifies");

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
    for (step, command) in [(66, "POTION USE 0 0"), (67, "END")] {
        assert!(report.action_dispositions.iter().any(|entry| {
            entry.action_step == step
                && entry.command == command
                && entry.disposition == ActionDispositionKind::Verified
        }));
    }
}

#[test]
fn smoke_bomb_trace_reconciles_a_queued_command_at_the_captured_endpoint() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-b51801b5fbe7f86b.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("captured queued Smoke Bomb command regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary,
        SeedStartBoundary {
            path: "$.actions[verified]".to_owned(),
            category: "none".to_owned(),
            reason: "seed-start verifier checked every verifiable transition in the trace"
                .to_owned(),
        }
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    assert!(report.action_dispositions.iter().any(|entry| {
        entry.action_step == 93
            && entry.command == "PLAY 2 0"
            && entry.disposition == ActionDispositionKind::Verified
    }));
}

#[test]
fn smith_trace_models_in_flight_effect_before_proceed() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-619700a8aef6dadc.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Smith effect timing regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary,
        SeedStartBoundary {
            path: "$.actions[verified]".to_owned(),
            category: "none".to_owned(),
            reason: "seed-start verifier checked every verifiable transition in the trace"
                .to_owned(),
        }
    );
    let integrity = report.action_integrity.as_ref().expect("action integrity");
    assert_eq!(integrity.unresolved_transient_assertions, 0);
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 106 && transition.label == "rest smith effect queued"
    }));
}

#[test]
fn smith_mid_effect_and_stale_release_reconcile_vampires_after_multiple_smiths() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-597134b9957bd497.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("multi-smith + Vampires regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report
        .verified
        .iter()
        .any(|transition| { transition.label == "rest smith effect queued" }));
}

#[test]
fn smith_mid_effect_deck_omission_does_not_fail_as_rest_smith_identity() {
    // Witness quarantined: SuperFastMode smith→shop purge (invalid collector
    // residual). Keep the regression on the quarantined path.
    let Some(content) = crate::load_corpus_file(
        "quarantined_traces/superfast_smith_shop_purge/random-fidelity-814046a9628c9f89.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("smith mid-effect omission trace parses");

    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 110 && transition.label == "rest smith effect queued"
    }));
    let failed_as_smith_identity = report
        .unsupported
        .iter()
        .any(|entry| entry.action_step == 113 && entry.reason.contains("rest smith effect"))
        || report
            .unexpected_diffs
            .iter()
            .any(|entry| entry.action_step == 113 && entry.label == "rest smith effect");
    assert!(
        !failed_as_smith_identity,
        "step 113 must not fail as rest smith identity: {report:#?}"
    );
}

#[test]
fn purifier_direct_leave_reaches_its_terminal_leave_screen() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-f2676070173c3be6.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Purifier direct-leave regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn golden_idol_direct_leave_reaches_its_terminal_leave_screen() {
    for name in [
        "random-fidelity-90f176908abf7404.jsonl",
        "random-fidelity-8e680d9593a06359.jsonl",
        "random-fidelity-a66d580b08db3587.jsonl",
    ] {
        let path = format!("permanent_traces/{name}");
        let Some(content) = crate::load_corpus_file(&path) else {
            continue;
        };
        let report = verify_seed_start_communication_mod_trace(&content)
            .unwrap_or_else(|error| panic!("{name} verifies: {error}"));

        assert!(
            report.unexpected_diffs.is_empty(),
            "{name} diffs: {:?}",
            report.unexpected_diffs
        );
        assert!(
            report.unsupported.is_empty(),
            "{name} unsupported: {:?}",
            report.unsupported
        );
        assert_eq!(
            report
                .seed_start
                .as_ref()
                .expect("seed-start report")
                .first_boundary
                .category,
            "none",
            "{name} has a replay boundary"
        );
        assert_eq!(
            report
                .action_integrity
                .as_ref()
                .expect("action integrity")
                .unresolved_transient_assertions,
            0,
            "{name} has an unresolved transient assertion"
        );
    }
}

#[test]
fn shovel_dig_enters_and_resolves_its_relic_reward() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-58d7201043d79df6.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Shovel Dig regression trace replays");

    assert!(
        report.unexpected_diffs.is_empty(),
        "Dig trace diffs: {:?}",
        report.unexpected_diffs
    );
    assert!(
        report.unsupported.is_empty(),
        "Dig trace unsupported: {:?}",
        report.unsupported
    );
    assert!(
        !report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .failed
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn rest_relic_reward_proceed_completes_the_rest_room_before_map_entry() {
    let content =
        crate::load_corpus_file("permanent_traces/random-fidelity-b788a4e142c8fc26.jsonl")
            .expect("full rest relic-reward trace");
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("rest relic-reward trace replays");

    assert!(
        report.unexpected_diffs.is_empty(),
        "{:#?}",
        report.unexpected_diffs
    );
    assert!(report
        .unsupported
        .iter()
        .all(|entry| entry.action_step != 451));
    assert!(report
        .verified
        .iter()
        .any(|entry| { entry.action_step == 451 && entry.label == "rest reward proceed to map" }));
    let seed_start = report.seed_start.expect("seed-start report");
    // Green permanent traces must be clean through EOF (no expected-boundary grades).
    assert_eq!(seed_start.first_boundary.category, "none");
    assert!(!seed_start.failed);
}

#[test]
fn distilled_chaos_hand_select_is_a_stable_verified_endpoint() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-29265014fff604b3.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Distilled Chaos regression trace replays");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert!(
        !report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .failed
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 106
            && transition.label == "combat potion use"
            && transition.command == "POTION USE 0"
    }));
}

#[test]
fn combat_selection_endpoint_reconciles_final_decision_frame() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-b9c0db157d03167f.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("unreconciled combat selection regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary,
        SeedStartBoundary {
            path: "$.actions[verified]".to_owned(),
            category: "none".to_owned(),
            reason: "seed-start verifier checked every verifiable transition in the trace"
                .to_owned(),
        }
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn queued_potion_reward_endpoint_reconciles_final_decision_frame() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-733fa5c1c94c6af0.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("queued potion reward regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.path, "$.actions[verified]");
    assert_eq!(boundary.category, "none");
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn delayed_double_tap_copy_is_canceled_by_end_turn_command() {
    let Some(content) = crate::load_corpus_file(
        "permanent_traces/trace-2026-07-07-session-16-codex10-complete.jsonl",
    )
    .or_else(|| {
        crate::load_corpus_file("open_failures/trace-2026-07-07-session-16-codex10-complete.jsonl")
    }) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("delayed copied attack regression trace verifies");

    // Double Tap cancel must accept END while a copy is still settling (step 481).
    assert!(
        report
            .verified
            .iter()
            .any(|entry| entry.action_step == 481 && entry.command == "END"),
        "END at step 481 must verify after canceling the not-yet-started Double Tap copy"
    );
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert!(
        boundary.category == "none"
            || boundary
                .path
                .contains("step=")
                .then(|| {
                    boundary
                        .path
                        .trim_start_matches("$.actions[step=")
                        .trim_end_matches("].command")
                        .parse::<u32>()
                        .ok()
                })
                .flatten()
                .is_some_and(|step| step > 481),
        "Double Tap cancel must not fail at or before step 481; got {boundary:?}"
    );
}

#[test]
fn fidl00425_replays_brutality_order_and_terminal_reward_lag_exactly() {
    let Some(content) = crate::load_corpus_file(
        "open_failures/FIDL00425-p425-2026-07-29T17-27-28-563Z-110764.jsonl",
    ) else {
        return;
    };
    let report =
        verify_seed_start_communication_mod_trace(&content).expect("FIDL00425 trace report");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "{report:#?}"
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 122
            && transition.command == "PLAY 1 0"
            && transition.label == "Anger"
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1020
            && transition.command == "END"
            && transition.label == "end turn (source terminal reward lag frame)"
    }));
}

#[test]
fn fidl00401_replays_exhaust_order_and_charons_ashes_source_lag_exactly() {
    let Some(content) = crate::load_corpus_file(
        "open_failures/FIDL00401-p401-2026-07-29T14-01-46-791Z-72752.jsonl",
    ) else {
        return;
    };
    let report =
        verify_seed_start_communication_mod_trace(&content).expect("FIDL00401 trace report");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "{report:#?}"
    );
    assert_eq!(report.verified.len(), 1411, "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1012
            && transition.command == "CONFIRM"
            && transition.label == "Burning Pact deferred selection transient"
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1015
            && transition.command == "END"
            && transition.label == "end turn (source pile settlement frame)"
    }));
}

#[test]
fn delayed_double_tap_copy_is_canceled_by_next_semantic_command() {
    let attack_id = CardId::new(91_003);
    let skill_id = CardId::new(91_004);
    let mut combat = CombatState::initial_fixture();
    combat.double_tap_pending = 2;
    combat.piles.hand = vec![
        CardInstance::new(attack_id, STRIKE_R_ID),
        CardInstance::new(skill_id, DEFEND_R_ID),
    ];

    let expectation = seed_start_copied_attack_expectation(
        &combat,
        CombatAction::PlayCard {
            card_id: attack_id,
            target: Some(MonsterId::new(1)),
        },
    );
    assert_eq!(
        expectation,
        Some(CopiedAttackExpectation {
            remaining_double_tap: 1,
        })
    );

    let cancelled = {
        combat.double_tap_pending = 0;
        sts_core::combat::apply_combat_action(
            &combat,
            CombatAction::PlayCard {
                card_id: attack_id,
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("the original attack remains authoritative")
    };
    assert_eq!(cancelled.double_tap_pending, 0);
}

#[test]
fn combat_card_reward_source_frame_reconciles_without_boundary() {
    let trace = "random-fidelity-acaabd41a504598f.jsonl";
    let Some(content) = crate::load_corpus_file(format!("permanent_traces/{trace}")) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .unwrap_or_else(|error| panic!("{trace} verifies: {error}"));

    assert!(report.unexpected_diffs.is_empty(), "{trace}");
    assert!(report.unsupported.is_empty(), "{trace}: {report:#?}");
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.path, "$.actions[verified]");
    assert_eq!(boundary.category, "none");
}

#[test]
fn stable_combat_trace_reconciles_compound_transient_evidence() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-c2aa19ad6556e10e.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("compound deferred combat regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.category, "none");
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn random_fidelity_havoc_exhume_skipped_return_with_dark_embrace() {
    // Havoc→Exhume CHOOSE: Exhume exhausts + DE draws, chosen exhaust card stays.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-6a06a48f3b8f0727.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Havoc Exhume skipped return permanent trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "{report:#?}"
    );
    assert_eq!(
        report
            .verified
            .iter()
            .find(|entry| entry.action_step == 561)
            .map(|entry| entry.label.as_str()),
        Some("Exhume skipped return retrieval frame"),
        "{report:#?}"
    );
}

#[test]
fn exhume_selection_post_click_transient_reconciles_without_boundary() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-8375d0aa0e56c94b.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Exhume post-click transient regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.category, "none");
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn warcry_skipped_put_on_deck_retrieval_frame_replays_source_transition() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-354debe1cdef9bc6.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry skipped retrieval regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 231)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn warcry_skipped_retrieval_with_dead_branch_rolls_into_hand() {
    // Warcry CONFIRM under skipped PutOnDeck retrieval while Dead Branch is
    // held: selected Dazed stays in limbo (not on draw), Warcry exhausts, and
    // Dead Branch still adds Blood for Blood to hand (step 1500).
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-584b41fbb0fd6dfa.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry+Dead Branch skipped retrieval permanent trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 1500)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "Warcry hand-select CONFIRM must verify as skipped retrieval with Dead Branch: {report:#?}"
    );
}

#[test]
fn warcry_skipped_retrieval_with_dark_embrace_draws_real_top_not_selected() {
    // Warcry CONFIRM under skipped PutOnDeck retrieval while Dark Embrace is
    // active: selected Defend+ stays in selection-screen limbo, DE draws the
    // pre-select top (Dual Wield) into hand. Permanent trace ends at step 649.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-aacf87be6c7234a6.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry+Dark Embrace skipped retrieval permanent trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "step 649 CONFIRM must match Dual Wield drawn via DE: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 649)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "Warcry hand-select CONFIRM must verify as skipped retrieval: {report:#?}"
    );
}

#[test]
fn random_fidelity_warcry_skipped_retrieval_preserves_delayed_discard_order() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-36e6dccfb5901688.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry delayed-discard permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "the selected status must settle before the following Strike: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 313)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the follow-up Strike must observe the delayed discard in source order: {report:#?}"
    );
}

#[test]
fn warcry_source_settlement_frame_verifies_without_a_follow_up_poll() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-ee35f57424b997d7.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry source settlement regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 301)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_warcry_singleton_auto_put_on_deck_allows_end() {
    // After Ink Bottle draws Warcry into an empty hand, PLAY Warcry draws one
    // card and PutOnDeckAction auto-places it (hand.size()==amount==1) without
    // a HAND_SELECT / CHOOSE / CONFIRM. The following END must be legal.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-58c2f0f27ef22764.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry singleton auto put-on-deck permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "END after singleton-hand Warcry must not see an active HandSelect: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 469)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "step 469 END must verify after auto put-on-deck Warcry: {report:#?}"
    );
}

#[test]
fn put_on_deck_source_card_survives_the_following_end_turn_refill() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-a364e2a698e879dc.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("put-on-deck end-turn regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 102)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_warcry_limbo_and_evolve_end_turn_draw_order() {
    // Warcry skipped-retrieval limbo must not re-enter discard on empty-hand
    // ENDs that reshuffle, nor on a subsequent single-card END after that
    // miss; Evolve then draws statuses after the base hand refill (not
    // interleaved). Permanent trace ends mid-run.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-ae18829cad583a71.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry limbo + Evolve end-turn draw permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "step 472 END draw/hand order must match through the full trace: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 472)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the Evolve-adjacent end-turn draw must verify: {report:#?}"
    );
}

#[test]
fn warcry_put_on_deck_lethal_end_does_not_residual_dropkick_next_combat() {
    // Warcry put-on-deck skipped retrieval selects Dropkick; Combust then wins
    // on the same END before hand discard. Must not inject a cross-combat
    // residual Dropkick into the next combat's discard
    // (end turn: discard_ids[N]: null != "Dropkick").
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-f3c0d2bea83d9313.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Warcry Dropkick lethal-END residual permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(
        report
            .unsupported
            .iter()
            .all(|entry| entry.action_step > 431),
        "step 431 END must not fail with residual Dropkick: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 431)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "next-combat first END after Warcry limbo lethal must verify: {report:#?}"
    );
}

#[test]
fn random_fidelity_master_of_strategy_full_hand_draw_enables_play_ten() {
    // Runic Pyramid + Snecko floor keeps a 10-card hand. Master of Strategy
    // must limbo-draw into the freed slot (Pommel Strike) so PLAY 10 0 parses.
    // Permanent tip: random-fidelity-809d00fe56ad6122 step 561.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-809d00fe56ad6122.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Master of Strategy full-hand draw permanent tip replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(
        report
            .unsupported
            .iter()
            .all(|entry| entry.action_step > 561),
        "step 561 PLAY 10 0 must parse after Master of Strategy draw: {report:#?}"
    );
    for step in [560, 561] {
        assert_eq!(
            report
                .action_dispositions
                .iter()
                .find(|entry| entry.action_step == step)
                .map(|entry| entry.disposition),
            Some(ActionDispositionKind::Verified),
            "step {step} must verify after full-hand Master of Strategy draw: {report:#?}"
        );
    }
}

#[test]
fn back_to_basics_elegance_grid_choose_removes_and_returns_to_leave() {
    // Ancient Writing / Back to Basics Elegance: CHOOSE on the remove grid
    // auto-confirms (no CONFIRM command) and lands on EVENT Leave with the
    // selected card removed. Permanent tip
    // random-fidelity-f3c0d2bea83d9313 ends at step 582 CHOOSE 23 (Metallicize).
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-f3c0d2bea83d9313.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Back to Basics Elegance permanent tip replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(
        report.unsupported.is_empty(),
        "Elegance CHOOSE 23 must complete the tip: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 582)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "step 582 grid remove → Leave must verify: {report:#?}"
    );
}

#[test]
fn random_fidelity_mark_of_pain_wounds_after_snecko_combat_entry() {
    // Mark of Pain inserts 2 Wounds via cardRandomRng after the opening hand
    // draw; Snecko Confusion cost rolls must advance that stream first.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-56f5d5f2bad30be7.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Mark of Pain + Snecko combat-entry permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "map first monster node draw_ids Wound order must match: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 348)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "post-boss Mark of Pain combat entry must verify: {report:#?}"
    );
}

#[test]
fn forethought_skipped_put_on_deck_retrieval_frame_replays_source_transition() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-ded412a8f5a83ec0.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Forethought skipped retrieval regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 46)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn forethought_skipped_retrieval_continues_through_combat_potion_use() {
    let Some(content) = crate::load_corpus_file(
        "fidelity_regressions/random-fidelity-5b364b2faf1f9e9d-forethought-potion-continuation.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("captured Forethought and Regen Potion trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 100
            && transition.command == "POTION USE 0"
            && transition.label == "combat potion use"
    }));
}

#[test]
fn gambling_chip_source_settlement_frame_verifies_without_a_follow_up_poll() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-7e93e8670a459612.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Gambling Chip source settlement regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 122)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn exhaust_select_hides_played_source_card_in_action_frame() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-ca1c5042a4810f20.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("True Grit action-frame regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.category, "none");
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn smoke_bomb_trace_replays_a_queued_command_that_mutates_transient_combat() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-617b5319ca2c85b4.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("mutated transient Smoke Bomb regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    assert!(report.action_dispositions.iter().any(|entry| {
        entry.action_step == 230
            && entry.command == "PLAY 2"
            && entry.disposition == ActionDispositionKind::Verified
    }));
}

#[test]
fn smoke_bomb_trace_reconciles_a_queued_end_at_the_captured_endpoint() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-5e634e6cfbe0ca83.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("captured Smoke Bomb END regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    assert!(report.action_dispositions.iter().any(|entry| {
        entry.action_step == 198
            && entry.command == "END"
            && entry.disposition == ActionDispositionKind::Verified
    }));
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
fn trace_timestamp_millis_handles_calendar_boundaries() {
    let before_midnight =
        trace_timestamp_millis("2026-07-07T23:59:59.950Z").expect("valid timestamp");
    let after_midnight =
        trace_timestamp_millis("2026-07-08T00:00:00.050Z").expect("valid timestamp");
    assert_eq!(after_midnight - before_midnight, 100);
    assert!(trace_timestamp_millis("2025-02-29T00:00:00Z").is_none());
    assert!(trace_timestamp_millis("2026-07-07T21:30:42.Z").is_none());
}

#[test]
fn subset_diffs_reports_known_card_against_unknown() {
    let diffs = subset_diffs(json!(["Offering+"]), json!(["unknown"]));

    assert_eq!(diffs, vec!["[0]: \"Offering+\" != \"unknown\""]);
}

#[test]
fn copied_attack_visibility_separates_command_semantics_from_observation() {
    let attack_id = CardId::new(91_001);
    let skill_id = CardId::new(91_002);
    let mut combat = CombatState::initial_fixture();
    combat.double_tap_pending = 2;
    combat.piles.hand = vec![
        CardInstance::new(attack_id, STRIKE_R_ID),
        CardInstance::new(skill_id, DEFEND_R_ID),
    ];

    let expectation = seed_start_copied_attack_expectation(
        &combat,
        CombatAction::PlayCard {
            card_id: attack_id,
            target: Some(MonsterId::new(1)),
        },
    );
    assert_eq!(
        expectation,
        Some(CopiedAttackExpectation {
            remaining_double_tap: 1,
        })
    );
    assert!(seed_start_copied_attack_expectation(
        &combat,
        CombatAction::PlayCard {
            card_id: skill_id,
            target: None,
        },
    )
    .is_none());

    let queued_frame = json!({
        "game_state": {
            "screen_type": "NONE",
            "action_phase": "WAITING_ON_USER",
            "combat_state": {
                "player": {
                    "powers": [{"id": "Double Tap", "amount": 1}],
                },
            },
        },
    });
    assert_eq!(
        seed_start_classify_copied_attack_frame(false, expectation, &queued_frame),
        CopiedAttackFrame::Deferred
    );
    assert_eq!(
        seed_start_classify_copied_attack_frame(true, expectation, &queued_frame),
        CopiedAttackFrame::Stable,
        "a fully matching projection cannot be forced into deferral"
    );

    for malformed in [
        json!({"game_state": {"screen_type": "NONE", "action_phase": "WAITING_ON_USER"}}),
        json!({"game_state": {
            "screen_type": "NONE",
            "action_phase": "WAITING_ON_USER",
            "combat_state": {"player": {"powers": [{"id": "Double Tap"}]}},
        }}),
        json!({"game_state": {
            "screen_type": "NONE",
            "action_phase": "EXECUTING_ACTIONS",
            "combat_state": {"player": {"powers": [{"id": "Double Tap", "amount": 1}]}},
        }}),
        json!({"game_state": {
            "screen_type": "NONE",
            "action_phase": "WAITING_ON_USER",
            "current_action": {},
            "combat_state": {"player": {"powers": [{"id": "Double Tap", "amount": 1}]}},
        }}),
    ] {
        assert_eq!(
            seed_start_classify_copied_attack_frame(false, expectation, &malformed),
            CopiedAttackFrame::Diverged
        );
    }

    combat.double_tap_pending = 1;
    let final_copy = seed_start_copied_attack_expectation(
        &combat,
        CombatAction::PlayCard {
            card_id: attack_id,
            target: Some(MonsterId::new(1)),
        },
    );
    let consumed_power_frame = json!({
        "game_state": {
            "screen_type": "NONE",
            "action_phase": "WAITING_ON_USER",
            "combat_state": {"player": {"powers": []}},
        },
    });
    assert_eq!(
        seed_start_classify_copied_attack_frame(false, final_copy, &consumed_power_frame,),
        CopiedAttackFrame::Deferred,
        "an authoritative empty power array represents zero remaining copies"
    );
}

#[test]
fn combat_projection_schema_keeps_authoritative_run_identity() {
    let message = json!({
        "game_state": {
            "screen_type": "NONE",
            "ascension_level": 7,
            "floor": 12,
            "gold": 123,
            "current_hp": 40,
            "max_hp": 80,
            "deck": [
                {"id": "Strike_R", "name": "Strike", "type": "ATTACK", "rarity": "BASIC", "upgrades": 0},
                {"id": "Bash", "name": "Bash", "type": "ATTACK", "rarity": "BASIC", "upgrades": 0},
            ],
            "relics": [{"id": "Burning Blood", "name": "Burning Blood"}],
            "potions": [],
            "combat_state": {
                "player": {
                    "current_hp": 40,
                    "block": 0,
                    "energy": 3,
                    "powers": [],
                },
                "hand": [],
                "draw_pile": [],
                "discard_pile": [],
                "monsters": [],
            },
        },
    });
    let observed = seed_start_combat_observed_subset(&message);
    assert_eq!(observed["ascension"], json!(7));
    assert_eq!(observed["deck_ids"], json!(["Strike_R", "Bash"]));
    assert_eq!(observed["relic_ids"], json!(["Burning Blood"]));

    for missing_key in ["ascension", "deck_ids", "relic_ids"] {
        let mut missing = observed.clone();
        missing
            .as_object_mut()
            .expect("combat projection is an object")
            .remove(missing_key);
        assert!(
            !seed_start_combat_subsets_match(missing, observed.clone()),
            "missing observed {missing_key} must not delete simulator authority"
        );
    }
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

    let normalized = seed_start_normalize_combat_compare(value);
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
    let normalized = seed_start_normalize_combat_compare(json!({
        "combat_player_hp": 0,
        "monsters": [{
            "current_hp": 47,
            "strength": 3,
            "ritual": 2,
            "vulnerable": 1,
            "intent": "ATTACK",
            "move_id": 6,
        }]
    }));

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
    use sts_core::content::monsters::{
        BYRD_ID, EXPLODER_ID, GREMLIN_WIZARD_ID, GUARDIAN_ID, SPIKER_ID,
    };

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

    monster.content_id = EXPLODER_ID;
    monster.intent = MonsterIntent::Stun;
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
    let mut run = RunState::seeded_ironclad(1, 0);
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
fn ordinary_combat_victory_compares_generated_reward_contents() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-e85eaa294d635e48.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Sozu reward regression trace replays");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert!(
        !report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .failed
    );

    let tampered = content
        .lines()
        .map(|line| {
            let mut value: Value = serde_json::from_str(line).expect("trace line parses");
            if value.get("type").and_then(Value::as_str) == Some("state")
                && value.get("step").and_then(Value::as_u64) == Some(19)
            {
                value
                    .pointer_mut("/message/game_state/screen_state/rewards")
                    .and_then(Value::as_array_mut)
                    .expect("combat reward list")
                    .retain(|reward| {
                        reward.get("reward_type").and_then(Value::as_str) != Some("POTION")
                    });
                value
                    .pointer_mut("/message/game_state/choice_list")
                    .and_then(Value::as_array_mut)
                    .expect("combat reward choices")
                    .retain(|choice| choice.as_str() != Some("potion"));
            }
            serde_json::to_string(&value).expect("trace line serializes")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let report = verify_seed_start_communication_mod_trace(&tampered)
        .expect("tampered reward trace replays to a fidelity failure");

    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.path, "$.actions[step=19].command");
    assert_eq!(boundary.category, "unexpected_sim_real_diff");
    assert!(
        boundary.reason.contains("reward_types["),
        "unexpected killing-blow boundary: {boundary:?}"
    );
}

#[test]
fn dual_wield_hand_select_projects_only_attack_and_power_candidates() {
    let mut run = RunState::seeded_ironclad(1, 0);
    let mut combat = CombatState::initial_fixture();
    combat.piles.hand = vec![
        CardInstance::new(CardId::new(1), POMMEL_STRIKE_PLUS_ID),
        CardInstance::new(CardId::new(3), DEFEND_R_ID),
        CardInstance::new(CardId::new(4), COMBUST_ID),
    ];
    combat.decision = Some(CombatDecisionState::HandSelect {
        state: sts_core::combat::HandSelectState {
            purpose: HandSelectPurpose::DualWieldCopy,
            source_card_id: CardId::new(2),
            selected_hand_index: None,
            selected_hand_indices: Vec::new(),
        },
        pending_actions: Default::default(),
    });
    run.combat = Some(combat);
    let projected = seed_start_simulated_combat_subset(&run, false);

    assert_eq!(projected["hand_ids"], json!(["Pommel Strike+", "Combust"]));
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
fn trace_transitions_associate_external_rng_with_the_producing_action() {
    let input = sts_core::ExternalRngInput {
        kind: sts_core::ExternalRngKind::CardGroupGetRandomCardByType,
        state: sts_core::MathUtilsRngState {
            state0: 0xfedc_ba98_7654_3210,
            state1: 0x0123_4567_89ab_cdef,
        },
        range_inclusive: 16,
    };
    let lines = vec![
        TraceLine::State(TraceState {
            step: 10,
            received_at: None,
            message: json!({"game_state": {"screen_type": "SHOP"}}),
        }),
        TraceLine::Action(TraceAction {
            step: 11,
            command: "CHOOSE 0".to_owned(),
            sent_at: None,
            playtime_seconds: None,
        }),
        TraceLine::ExternalRng(crate::TraceExternalRng {
            step: 11,
            draws: vec![input],
        }),
        TraceLine::State(TraceState {
            step: 11,
            received_at: None,
            message: json!({"game_state": {"screen_type": "SHOP"}}),
        }),
    ];

    let transitions = trace_transitions(&lines).expect("trace transitions");
    assert_eq!(
        transitions.external_rng_by_action_step.get(&11),
        Some(&vec![input])
    );
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
fn trace_transitions_settle_cursed_key_chest_open_before_relic_choose() {
    // FIDL00415: relic CHOOSE arrives while curse is still pending (no STATE poll).
    let state = |step, screen_type: &str, deck: &[&str], relics: serde_json::Value| {
        TraceLine::State(TraceState {
            step,
            received_at: None,
            message: json!({
                "ready_for_command": true,
                "game_state": {
                    "deck": deck.iter().map(|id| json!({"id": id})).collect::<Vec<_>>(),
                    "relics": relics,
                    "room_type": "TreasureRoom",
                    "screen_type": screen_type,
                    "choice_list": if screen_type == "COMBAT_REWARD" {
                        json!(["relic"])
                    } else {
                        json!(["open"])
                    },
                }
            }),
        })
    };
    let cursed = json!([{"id": "Cursed Key", "counter": -1}]);
    let with_bag = json!([
        {"id": "Cursed Key", "counter": -1},
        {"id": "Bag of Marbles", "counter": -1}
    ]);
    let lines = vec![
        state(1, "CHEST", &["Strike_R"], cursed.clone()),
        TraceLine::Action(TraceAction {
            step: 2,
            command: "CHOOSE 0".to_owned(),
            sent_at: None,
            playtime_seconds: Some(10),
        }),
        state(2, "COMBAT_REWARD", &["Strike_R"], cursed),
        TraceLine::Action(TraceAction {
            step: 3,
            command: "CHOOSE 0".to_owned(),
            sent_at: None,
            playtime_seconds: Some(11),
        }),
        state(3, "COMBAT_REWARD", &["Strike_R", "Decay"], with_bag),
    ];

    let transitions = trace_transitions(&lines).expect("trace transitions");
    assert_eq!(transitions.transitions.len(), 2);
    assert_eq!(transitions.transitions[0].1.step, 2);
    assert_eq!(transitions.transitions[0].2.step, 2);
    assert_eq!(transitions.transitions[1].1.step, 3);
    assert_eq!(transitions.transitions[1].2.step, 3);
    assert_eq!(transitions.ignored_tail_actions, 0);
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(sts_core::relic::ORRERY_CARD_REWARDS),
    };

    assert_eq!(
        sim_reward_combat_choices(&RunState::map_fixture(), &reward),
        vec!["card".to_owned(); sts_core::relic::ORRERY_CARD_REWARDS as usize]
    );
}

#[test]
fn reward_projection_keeps_pending_card_items_after_closing_card_screen() {
    let reward = RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: vec![CardInstance::new(
            CardId::new(1),
            sts_core::content::cards::BASH_ID,
        )],
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(2),
    };

    assert_eq!(
        sim_reward_combat_choices(&RunState::map_fixture(), &reward),
        ["card", "card"].map(str::to_owned)
    );
}

#[test]
fn reward_command_opens_next_compacted_queued_card_reward() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Reward;
    let first = CardInstance::new(CardId::new(10_001), sts_core::content::cards::BASH_ID);
    let second = CardInstance::new(CardId::new(10_002), sts_core::content::cards::STRIKE_R_ID);
    run.reward = Some(RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: vec![vec![first], vec![second]],
        gold_offer: 10,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(2),
    });

    // The first queued reward was already opened and consumed from the
    // compact queue, but the outer reward screen still has two card entries.
    let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 1")
        .expect("outer card ordinal selects the corresponding queued reward");

    assert_eq!(label, "card reward");
    let reward = run.reward.as_ref().expect("reward remains open");
    assert!(reward.card_reward_is_active());
    assert_eq!(reward.choices, vec![first]);
    assert_eq!(reward.queued_card_rewards.len(), 2);

    let closed = apply_run_action(&run, RunAction::CloseCardReward)
        .expect("closed card reward retains its visible choices");
    run = closed;
    seed_start_apply_reward_choose(&mut run, "CHOOSE 2")
        .expect("queued open replaces stale choices after compaction");
    let reward = run.reward.as_ref().expect("compacted reward remains open");
    assert_eq!(reward.choices, vec![second]);
    assert_eq!(reward.queued_card_rewards.len(), 2);

    let selected = apply_run_action(&run, RunAction::TakeCardReward { card_id: second.id })
        .expect("selected queued reward is consumed");
    let reward = selected
        .reward
        .as_ref()
        .expect("remaining reward stays open");
    assert_eq!(reward.queued_card_rewards, vec![vec![first]]);
    assert_eq!(reward.remaining_card_reward_count(), 1);
}

#[test]
fn reward_projection_places_stolen_gold_before_combat_gold() {
    let reward = RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 19,
        stolen_gold_offer: 30,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(1),
    };

    assert_eq!(
        sim_reward_combat_choices(&RunState::map_fixture(), &reward),
        ["stolen_gold", "gold", "card"].map(str::to_owned)
    );
}

#[test]
fn reward_projection_keeps_black_star_offers_in_screen_order() {
    let reward = RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 27,
        stolen_gold_offer: 0,
        potion_offer: Some(sts_core::Potion::Fire),
        potion_offers: Vec::new(),
        relic_offer: Some(Relic::ToyOrnithopter),
        pending_relic_offer: Some(Relic::Vajra),
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(1),
    };

    assert_eq!(
        sim_reward_combat_choices(&RunState::map_fixture(), &reward),
        ["gold", "relic", "relic", "potion", "card"].map(str::to_owned)
    );
}

#[test]
fn reward_projection_keeps_matryoshka_chest_offers_in_screen_order() {
    let mut run = RunState::map_fixture();
    run.current_room_override = Some(RoomKind::Treasure);
    run.treasure_room = Some(sts_core::run::TreasureRoomState {
        chest_size: sts_core::run::reward::ChestSize::Large,
        relic_tier: sts_core::RelicTier::Uncommon,
        have_gold: true,
        relic_before_gold: true,
    });
    let reward = RewardScreen {
        continuation: sts_core::RewardContinuation::Map,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 24,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: Some(Relic::BottledLightning),
        pending_relic_offer: Some(Relic::OddlySmoothStone),
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    };

    assert_eq!(
        sim_reward_combat_choices(&run, &reward),
        ["relic", "gold", "relic"].map(str::to_owned)
    );
}

#[test]
fn indexed_map_chest_relic_pick_preserves_remaining_reward_order() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Reward;
    run.current_room_override = Some(RoomKind::Treasure);
    run.treasure_room = Some(sts_core::run::TreasureRoomState {
        chest_size: sts_core::run::reward::ChestSize::Large,
        relic_tier: sts_core::RelicTier::Uncommon,
        have_gold: true,
        relic_before_gold: true,
    });
    run.reward = Some(RewardScreen {
        continuation: sts_core::RewardContinuation::Map,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 72,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        // Non-bottled relics so indexed picks stay on the combat-reward screen.
        relic_offer: Some(Relic::Orichalcum),
        pending_relic_offer: Some(Relic::Sundial),
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });

    let picked_last = apply_run_action(&run, RunAction::TakeRelicRewardAt { index: 1 })
        .expect("last chest relic can be picked");
    assert!(
        picked_last
            .treasure_room
            .as_ref()
            .expect("treasure room remains open")
            .relic_before_gold
    );
    assert_eq!(
        sim_reward_combat_choices(
            &picked_last,
            picked_last.reward.as_ref().expect("remaining chest reward"),
        ),
        ["relic", "gold"].map(str::to_owned)
    );
    assert!(picked_last.relics.contains(&Relic::Sundial));
    assert!(!picked_last.relics.contains(&Relic::Orichalcum));

    let picked_first = apply_run_action(&run, RunAction::TakeRelicRewardAt { index: 0 })
        .expect("first chest relic can be picked");
    assert!(
        !picked_first
            .treasure_room
            .as_ref()
            .expect("treasure room remains open")
            .relic_before_gold
    );
    assert_eq!(
        sim_reward_combat_choices(
            &picked_first,
            picked_first
                .reward
                .as_ref()
                .expect("remaining chest reward"),
        ),
        ["gold", "relic"].map(str::to_owned)
    );
    assert!(picked_first.relics.contains(&Relic::Orichalcum));
    assert!(!picked_first.relics.contains(&Relic::Sundial));
}

#[test]
fn reward_projection_keeps_single_chest_gold_before_relic() {
    let mut run = RunState::map_fixture();
    run.current_room_override = Some(RoomKind::Treasure);
    let reward = RewardScreen {
        continuation: sts_core::RewardContinuation::Map,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 72,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: Some(Relic::Orichalcum),
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    };

    assert_eq!(
        sim_reward_combat_choices(&run, &reward),
        ["gold", "relic"].map(str::to_owned)
    );
}

#[test]
fn reward_projection_keeps_full_belt_potion_offer_visible() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Reward;
    run.potions = vec![Potion::BlessingOfTheForge, Potion::Dexterity, Potion::Power];
    run.reward = Some(RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 10,
        stolen_gold_offer: 0,
        potion_offer: Some(Potion::Fire),
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(1),
    });

    assert_eq!(
        sim_reward_combat_choices(&run, run.reward.as_ref().expect("reward screen")),
        ["gold", "potion", "card"].map(str::to_owned)
    );
    let subset = seed_start_reward_simulated_subset(&run);
    assert_eq!(subset["choices"], json!(["gold", "potion", "card"]));
    assert_eq!(subset["reward_types"], json!(["GOLD", "POTION", "CARD"]));
}

#[test]
fn random_fidelity_e45_potion_drop_sequence_replays_without_boundary() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-e45fe3dd64059dc2.jsonl")
    else {
        return;
    };
    let report =
        verify_seed_start_communication_mod_trace(&content).expect("potion drop trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    let seed_start = report.seed_start.expect("seed-start report");
    assert!(!seed_start.failed, "unexpected boundary: {seed_start:#?}");
    assert_eq!(seed_start.first_boundary.category, "none");
    assert!(report.action_dispositions.iter().any(|entry| {
        entry.action_step == 120 && entry.disposition == ActionDispositionKind::Verified
    }));
    for action_step in [33, 34, 35] {
        assert!(
            report.action_dispositions.iter().any(|entry| {
                entry.action_step == action_step
                    && entry.disposition == ActionDispositionKind::Verified
            }),
            "Smoke Bomb escape/reward settlement step {action_step} must remain verified"
        );
    }
}

#[test]
fn preexisting_full_belt_potion_reward_remains_visible() {
    let Some(content) = crate::load_corpus_file(
        "permanent_traces/trace-2026-06-25T00-44-15-558Z.retained.step548.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("full-belt potion reward trace replays");

    assert!(
        report
            .unexpected_diffs
            .iter()
            .all(|diff| diff.action_step != 158),
        "step 158 must retain the target's visible potion reward: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 158)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn fusion_hammer_removes_smith_from_seed_start_rest_projection() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Rest;
    run.event = None;
    run.current_room_override = Some(RoomKind::Rest);
    run.relics.push(Relic::FusionHammer);

    assert_eq!(
        seed_start_rest_screen_actions(&run).expect("valid rest decisions"),
        vec![RestAction::Heal]
    );
    assert_eq!(
        seed_start_rest_simulated_subset(&run)["choices"],
        json!(["rest"])
    );
}

#[test]
fn seed_start_rest_projection_uses_dynamic_relic_action_order() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Rest;
    run.event = None;
    run.current_room_override = Some(RoomKind::Rest);
    run.relics.extend([
        Relic::CoffeeDripper,
        Relic::PeacePipe,
        Relic::Girya,
        Relic::Shovel,
    ]);

    assert_eq!(
        seed_start_rest_screen_actions(&run).expect("valid rest decisions"),
        vec![
            RestAction::OpenSmith,
            RestAction::OpenRemove,
            RestAction::Lift,
            RestAction::Dig,
        ]
    );
    assert_eq!(
        seed_start_rest_simulated_subset(&run)["choices"],
        json!(["smith", "toke", "lift", "dig"])
    );

    // Shovel before Girya (as in random-fidelity-b788a4e142c8fc26) yields dig then lift.
    run.relics
        .retain(|relic| !matches!(relic, Relic::PeacePipe | Relic::Girya | Relic::Shovel));
    run.relics
        .extend([Relic::Shovel, Relic::Girya, Relic::PeacePipe]);
    assert_eq!(
        seed_start_rest_screen_actions(&run).expect("valid rest decisions"),
        vec![
            RestAction::OpenSmith,
            RestAction::Dig,
            RestAction::Lift,
            RestAction::OpenRemove,
        ]
    );
    assert_eq!(
        seed_start_rest_simulated_subset(&run)["choices"],
        json!(["smith", "dig", "lift", "toke"])
    );
}

#[test]
fn seed_start_rest_projection_exposes_invalid_core_legal_state() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Rest;
    run.event = None;
    run.current_room_override = Some(RoomKind::Rest);
    run.ascension = 21;

    assert!(seed_start_rest_simulated_subset(&run)["simulator_error"]
        .as_str()
        .is_some_and(|message| message.contains("run ascension exceeds 20")));
}

#[test]
fn seed_start_potion_command_drops_stray_target_for_targetless_potion() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.potions = vec![Potion::Dexterity];

    let command = ParsedPotionUse {
        slot: 0,
        target: Some(MonsterId::new(1)),
    };

    assert_eq!(seed_start_potion_command_target(&run, &command), None);
}

#[test]
fn seed_start_potion_command_keeps_target_for_targeted_potion() {
    let mut run = RunState::seeded_ironclad(1, 0);
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
fn observed_potion_projection_preserves_unknown_identity() {
    let potions = json!([
        { "id": "Potion Slot", "name": "Potion Slot" },
        { "id": "FuturePotion", "name": "Future Potion" },
        { "id": "Dexterity Potion", "name": "Dexterity Potion" }
    ]);

    assert_eq!(
        potion_keys_from_value(Some(&potions)),
        vec!["Future Potion", "Dexterity Potion"]
    );
}

#[test]
fn full_belt_potion_reward_command_binds_to_prior_gold_offer() {
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });
    seed_start_apply_reward_choose(&mut run, "CHOOSE 1")
        .expect("the target command bridge claims the preceding gold reward");
    assert_eq!(run.gold, 219);
    let reward = run.reward.as_ref().expect("reward screen remains");
    assert_eq!(reward.gold_offer, 0);
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });
    let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 0")
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
fn reward_command_uses_simulator_owned_order() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Reward;
    run.gold = 99;
    run.potions.clear();
    run.reward = Some(RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 120,
        stolen_gold_offer: 0,
        potion_offer: Some(Potion::Dexterity),
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });
    let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 0")
        .expect("simulator-owned first reward is taken");

    assert_eq!(label, "gold reward");
    assert_eq!(run.gold, 219);
    assert!(run.potions.is_empty());
    assert_eq!(
        run.reward.as_ref().and_then(|reward| reward.potion_offer),
        Some(Potion::Dexterity)
    );
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::active(1),
    });
    run.current_floor = 8;
    let subset = seed_start_reward_simulated_subset(&run);
    assert_eq!(
        subset["choices"].as_array().unwrap().last(),
        Some(&json!("bowl"))
    );

    let max_hp = run.player_max_hp;
    let hp = run.player_hp;
    let label =
        seed_start_apply_reward_choose(&mut run, "CHOOSE 1").expect("Singing Bowl choice applies");
    assert_eq!(label, "singing bowl card reward");
    assert_eq!(run.player_max_hp, max_hp + 2);
    // Singing Bowl heals when raising max HP (AbstractPlayer.increaseMaxHp).
    assert_eq!(run.player_hp, hp + 2);
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::pending(1),
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

    let simulated = seed_start_reward_simulated_subset(&run);
    let observed = seed_start_reward_observed_subset(&observed_message);
    assert_eq!(simulated["choices"], json!(["gold", "card"]));
    assert_eq!(observed["choices"], json!(["card", "gold"]));
    assert_eq!(simulated["gold_offer"], 17);
    assert_eq!(observed["gold_offer"], 999);
}

#[test]
fn relic_mismatch_is_a_projection_diff_and_cannot_block_the_core_transition() {
    let mut run = RunState::map_fixture();
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });
    let observed_message = json!({
        "game_state": {
            "screen_type": "COMBAT_REWARD",
            "screen_state": {
                "rewards": [{
                    "reward_type": "RELIC",
                    "relic": {"id": "Pantograph", "name": "Pantograph"}
                }]
            }
        }
    });

    let observed = seed_start_reward_observed_subset(&observed_message);
    let simulated = seed_start_reward_simulated_subset(&run);
    assert_eq!(observed["relic_offer_ids"], json!(["Pantograph"]));
    assert_eq!(simulated["relic_offer_ids"], json!(["The Boot"]));
    assert!(subset_diffs(observed, simulated)
        .iter()
        .any(|diff| diff.starts_with("relic_offer_ids")));

    let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 0")
        .expect("observed mismatch cannot block simulator-owned transition");
    assert_eq!(label, "relic reward");
    assert!(run.relics.contains(&Relic::TheBoot));
}

#[test]
fn observed_reward_projection_preserves_unknown_relic_identity() {
    let message = json!({
        "game_state": {
            "screen_type": "COMBAT_REWARD",
            "screen_state": {
                "rewards": [{
                    "reward_type": "RELIC",
                    "relic": {"id": "FutureRelic", "name": "Future Relic"}
                }]
            }
        }
    });

    assert_eq!(
        seed_start_reward_observed_subset(&message)["relic_offer_ids"],
        json!(["Future Relic"])
    );
}

#[test]
fn match_and_keep_choice_omission_is_not_normalized() {
    let path =
        crate::corpus_path("fidelity_regressions/session-19-match-and-keep-flip-identity.jsonl");
    let content = std::fs::read_to_string(path).expect("Match and Keep trace");
    let imported = import_communication_mod_trace(&content).expect("trace imports");
    let transitions = trace_transitions(&imported.lines).expect("trace transitions");
    let (action_step, post_state) = transitions
        .transitions
        .iter()
        .find_map(|(pre, action, post)| {
            let choices = post
                .message
                .pointer("/game_state/choice_list")
                .and_then(Value::as_array)?;
            (first_choice(&pre.message) == Some("play")
                && command_is_choose(&action.command, 0)
                && choices.len() == 12
                && choices.iter().all(|choice| {
                    choice
                        .as_str()
                        .is_some_and(|label| label.starts_with("card"))
                }))
            .then_some((action.step, post.clone()))
        })
        .expect("fixture opens the twelve-card Match and Keep board");

    let original = verify_communication_mod_trace(&content).expect("original trace verifies");
    assert!(original.unexpected_diffs.is_empty());

    let metadata = imported.metadata.expect("trace metadata");
    let mut lines = imported
        .lines
        .into_iter()
        .filter(|line| !matches!(line, TraceLine::Metadata(_)))
        .collect::<Vec<_>>();
    let forged_post = lines
        .iter_mut()
        .find_map(|line| match line {
            TraceLine::State(state) if *state == post_state => Some(state),
            _ => None,
        })
        .expect("Match and Keep post-state remains in trace");
    forged_post
        .message
        .pointer_mut("/game_state/choice_list")
        .and_then(Value::as_array_mut)
        .expect("Match and Keep choice list")
        .remove(3);

    let forged_trace = crate::serialize_communication_mod_trace(&metadata, &lines);
    let forged = verify_communication_mod_trace(&forged_trace).expect("forged trace parses");
    let diff = forged
        .unexpected_diffs
        .iter()
        .find(|entry| entry.action_step == action_step && entry.label == "event choice")
        .expect("omitted Match and Keep slot must differ");
    assert!(
        diff.diffs.iter().any(|line| line.starts_with("choices[")),
        "{diff:#?}"
    );
    let disposition = forged
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == action_step)
        .expect("forged event action disposition");
    assert_eq!(
        disposition.disposition,
        ActionDispositionKind::UnexpectedDiff
    );
    assert_eq!(
        forged
            .seed_start
            .as_ref()
            .and_then(|report| report.sim_run_state.as_ref()),
        original
            .seed_start
            .as_ref()
            .and_then(|report| report.sim_run_state.as_ref()),
        "the forged observation must not steer simulator state"
    );
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
            "deck": [{"id": "Strike_R", "upgrades": 0}],
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
fn explicit_trace_replay_exposes_deterministic_snapshots_and_checkpoints() {
    let Some(content) = crate::load_corpus_file("permanent_traces/trace-session-8.jsonl") else {
        return;
    };

    let verified = verify_communication_mod_trace(&content).expect("trace verifies");
    let replay = replay_communication_mod_trace(&content, None).expect("trace replays");
    assert_eq!(
        replay
            .final_snapshot
            .as_ref()
            .map(|snapshot| &snapshot.state),
        verified
            .seed_start
            .as_ref()
            .and_then(|seed_start| seed_start.sim_run_state.as_ref())
    );
    assert!(!replay.checkpoints.is_empty());
    assert!(replay
        .checkpoints
        .iter()
        .any(|checkpoint| checkpoint.state_hash.is_some()));

    let selected_step = replay
        .checkpoints
        .iter()
        .find(|checkpoint| checkpoint.state_hash.is_some())
        .expect("replay has a state checkpoint")
        .action_step;
    let selected = replay_communication_mod_trace(&content, Some(selected_step))
        .expect("selected replay succeeds")
        .selected_checkpoint
        .expect("selected step has a state");
    assert_eq!(selected.action_step, selected_step);
    let serialized = serde_json::to_string(&selected.snapshot).expect("snapshot serializes");
    let restored = sts_core::restore_run_snapshot_json(&serialized).expect("snapshot restores");
    assert_eq!(restored, selected.snapshot);

    let repeated = replay_communication_mod_trace(&content, None).expect("replay repeats");
    assert_eq!(replay.checkpoints, repeated.checkpoints);
    assert_eq!(replay.final_snapshot, repeated.final_snapshot);
}

#[test]
fn unknown_trace_record_is_invalid_input_instead_of_disappearing() {
    let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"exit","ended_at":"now"}"#;

    let error = verify_communication_mod_trace(content).expect_err("unknown record rejected");
    assert!(matches!(error, SimRealError::Trace(_)));
    assert!(matches!(
        crate::assess_verification(Err(&error), None),
        crate::VerificationOutcome::InvalidInput { reason }
            if reason.contains("unknown variant `exit`")
    ));
}

#[test]
fn malformed_choose_is_rejected_instead_of_selecting_choice_zero() {
    let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"state","step":0,"message":{}}
{"type":"action","step":1,"command":"START IRONCLAD 0 VERIFY01"}
{"type":"state","step":1,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":0,"gold":99,"current_hp":80,"max_hp":80,"deck":[{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Bash"}],"relics":[{"name":"Burning Blood"}],"potions":[],"choice_list":["talk"],"screen_state":{"event_id":"Neow Event","options":[{"text":"talk"}]}}}}
{"type":"action","step":2,"command":"CHOOSE nope"}
{"type":"state","step":2,"message":{"game_state":{"screen_type":"EVENT","ascension_level":0,"floor":0,"gold":99,"current_hp":80,"max_hp":80,"deck":[{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Strike_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Defend_R"},{"id":"Bash"}],"relics":[{"name":"Burning Blood"}],"potions":[],"choice_list":["talk"],"screen_state":{"event_id":"Neow Event","options":[{"text":"talk"}]}}}}"#
        .replace(
            r#"{"id":"Strike_R"}"#,
            r#"{"id":"Strike_R","upgrades":0}"#,
        )
        .replace(
            r#"{"id":"Defend_R"}"#,
            r#"{"id":"Defend_R","upgrades":0}"#,
        )
        .replace(r#"{"id":"Bash"}"#, r#"{"id":"Bash","upgrades":0}"#);

    let error = verify_communication_mod_trace(&content).expect_err("malformed trace rejected");
    assert!(matches!(
        error,
        SimRealError::MalformedChooseCommand {
            step: 2,
            ref command,
        } if command == "CHOOSE nope"
    ));
    assert!(matches!(
        crate::assess_verification(Err(&error), None),
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
        RunState::seeded_ironclad(start.numeric_seed as u64, start.ascension).act1_boss;
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
        .find(|diff| diff.action_step == start.action_step && diff.label == "seed-start bootstrap")
        .and_then(|diff| diff.diffs.iter().find(|line| line.starts_with("act_boss:")))
        .expect("forged observed boss must differ from seed-derived boss");
    assert!(boss_diff.contains(forged_boss), "{boss_diff}");
    assert!(boss_diff.contains(expected_boss_name), "{boss_diff}");
}

#[test]
fn missing_observed_boss_identity_is_reported_at_bootstrap() {
    let path = crate::corpus_path("permanent_traces/trace-2026-07-03T20-12-12-408Z.jsonl");
    let content = std::fs::read_to_string(path).expect("retained trace");
    let imported = import_communication_mod_trace(&content).expect("trace imports");
    let start_step = imported
        .lines
        .iter()
        .find_map(|line| match line {
            TraceLine::Action(action) => parse_start_command(action)
                .and_then(Result::ok)
                .map(|_| action.step),
            _ => None,
        })
        .expect("trace start command");

    let mut removed_states = 0;
    let missing = content
        .lines()
        .map(|line| {
            let mut value: Value = serde_json::from_str(line).expect("trace line JSON");
            if value.get("type").and_then(Value::as_str) == Some("state")
                && value
                    .get_mut("message")
                    .and_then(|message| message.get_mut("game_state"))
                    .and_then(Value::as_object_mut)
                    .and_then(|game| game.remove("act_boss"))
                    .is_some()
            {
                removed_states += 1;
            }
            serde_json::to_string(&value).expect("mutated trace line serializes")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(removed_states > 0, "fixture must expose boss identity");

    let report = verify_communication_mod_trace(&missing).expect("mutated trace parses");
    assert!(report.unexpected_diffs.iter().any(|diff| {
        diff.action_step == start_step
            && diff.label == "seed-start bootstrap"
            && diff.diffs.iter().any(|line| line.starts_with("act_boss:"))
    }));
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
        .find(|diff| diff.action_step == chest_action_step && diff.label == "open treasure chest")
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
    forge_boss_reward_observation(&mut mutated_state.message);

    let metadata = imported.metadata.expect("trace metadata");
    let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
    let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
    let screen_diff = report
        .unexpected_diffs
        .iter()
        .find(|diff| diff.action_step == chest_action_step && diff.label == "open treasure chest")
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
fn stable_combat_entry_compares_opening_piles() {
    let path = crate::corpus_path("permanent_traces/trace-session-8.jsonl");
    let content = std::fs::read_to_string(path).expect("complete trace");
    let imported = import_communication_mod_trace(&content).expect("trace imports");
    let transitions = trace_transitions(&imported.lines).expect("trace transitions");
    let (entry_step, entry_post) = transitions
        .transitions
        .iter()
        .find_map(|(pre, action, post)| {
            (screen_type(&pre.message) == Some("MAP")
                && command_head_eq(&action.command, "CHOOSE")
                && post.message.pointer("/game_state/combat_state").is_some()
                && post
                    .message
                    .pointer("/game_state/action_phase")
                    .and_then(Value::as_str)
                    == Some("WAITING_ON_USER"))
            .then_some((action.step, post.clone()))
        })
        .expect("fixture has a stable map-to-combat entry");

    let mut mutated_lines = imported
        .lines
        .into_iter()
        .filter(|line| !matches!(line, TraceLine::Metadata(_)))
        .collect::<Vec<_>>();
    let mutated_state = mutated_lines
        .iter_mut()
        .find_map(|line| match line {
            TraceLine::State(state) if *state == entry_post => Some(state),
            _ => None,
        })
        .expect("combat entry post-state remains in imported trace");
    let card_id = mutated_state
        .message
        .pointer_mut("/game_state/combat_state/hand/0/id")
        .expect("stable combat entry exposes its opening hand");
    *card_id = if card_id.as_str() == Some("Bash") {
        json!("Strike_R")
    } else {
        json!("Bash")
    };

    let metadata = imported.metadata.expect("trace metadata");
    let mutated = crate::serialize_communication_mod_trace(&metadata, &mutated_lines);
    let report = verify_communication_mod_trace(&mutated).expect("mutated trace parses");
    let pile_diff = report
        .unexpected_diffs
        .iter()
        .find(|diff| {
            diff.action_step == entry_step
                && diff
                    .diffs
                    .iter()
                    .any(|line| line.starts_with("hand_ids[0]:"))
        })
        .expect("forged opening hand must differ from simulator piles");
    assert!(pile_diff.label.contains("monster node"), "{pile_diff:#?}");
}

#[test]
fn executing_toolbox_entry_reconciles_at_a_stable_combat_frame() {
    let path = crate::corpus_path("permanent_traces/trace-session-17.jsonl");
    let content = std::fs::read_to_string(path).expect("Toolbox trace");
    let report = verify_communication_mod_trace(&content).expect("Toolbox trace verifies");
    let entry = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 5356 && entry.command == "CHOOSE 0")
        .expect("Toolbox combat-entry disposition");

    assert_eq!(entry.disposition, ActionDispositionKind::Verified);
    assert!(entry.deferred_assertion_reconciled);
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
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
    forge_rest_observation(&mut mutated_state.message);

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
    let event_screen = mutated_state
        .message
        .pointer_mut("/game_state/screen_state")
        .and_then(Value::as_object_mut)
        .expect("shop screen state");
    event_screen.insert("event_id".to_owned(), json!("Golden Shrine"));
    event_screen.insert("options".to_owned(), json!([]));

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
fn end_turn_reconciles_a_captured_card_reward_frame_before_resolution() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-599f7cd81ae66c46.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("captured end-turn card reward frame verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn combat_hand_order_source_settlement_accepts_captured_endpoint() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-c3199c4dd4ffd0ff.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("combat hand-order settlement trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn source_hand_order_settlement_binds_the_following_play_by_observed_slot() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-7e93e8670a459612.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("source hand-order follow-up trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 57
            && transition.command == "PLAY 3"
            && transition.label == "Defend"
    }));
}

#[test]
fn random_fidelity_68c54240_defend_slot_uses_captured_hand_order() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-68c54240e41ce245.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("68c54240 fidelity regression trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 139
            && transition.command == "PLAY 3 0"
            && transition.label == "Strike"
    }));
}

#[test]
fn end_turn_reconciles_a_short_source_appended_discard_frame() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-c07d0eb5fa699d29.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("short end-turn discard settlement trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn smith_source_projection_preserves_bottled_flame_for_the_next_combat() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-911672c13a04d363.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Smith and bottled opening trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn event_grid_source_settlement_accepts_the_captured_event_frame() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-4251e4da40015ef4.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("event grid settlement trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn distilled_chaos_accepts_a_source_settlement_frame_with_delayed_monster_hp() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-8c45b99d2c2a473d.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Distilled Chaos settlement trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn combat_entry_reconciles_when_source_card_reward_settles_the_initial_hand() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-af6462493d32f815.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("combat entry card reward settlement trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn shop_entry_source_inventory_refresh_requires_one_card_and_seventy_five_gold() {
    let observed = json!({
        "screen_type": "SHOP_ROOM",
        "floor": 11,
        "gold": 204,
        "current_hp": 100,
        "max_hp": 100,
        "deck_ids": ["Strike_R", "Defend_R", "Bash"],
        "relic_ids": ["Burning Blood"],
        "choices": ["shop"],
    });
    let simulated = json!({
        "screen_type": "SHOP_ROOM",
        "floor": 11,
        "gold": 279,
        "current_hp": 100,
        "max_hp": 100,
        "deck_ids": ["Strike_R", "Defend_R", "Defend_R", "Bash"],
        "relic_ids": ["Burning Blood"],
        "choices": ["shop"],
    });
    assert!(replay::seed_start_shop_source_inventory_refresh_frame(
        &observed, &simulated
    ));

    let mut wrong_gold = observed.clone();
    wrong_gold["gold"] = json!(203);
    assert!(!replay::seed_start_shop_source_inventory_refresh_frame(
        &wrong_gold,
        &simulated
    ));
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
    forge_grid_observation(&mut mutated_state.message);

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
fn transient_boss_act_proceed_reconciles_only_at_the_stable_map() {
    let path =
        crate::corpus_path("permanent_traces/live-regression-2026-07-02T23-24-13-178Z.jsonl");
    let content = std::fs::read_to_string(path).expect("complete two-act trace");
    let imported = import_communication_mod_trace(&content).expect("trace imports");
    let transitions = trace_transitions(&imported.lines).expect("trace transitions");
    let (proceed_step, stable_map) = transitions
        .transitions
        .iter()
        .find_map(|(pre, action, post)| {
            (screen_type(&pre.message) == Some("CHEST")
                && trace_room_type(&pre.message) == Some("TreasureRoomBoss")
                && action.command.eq_ignore_ascii_case("PROCEED")
                && screen_type(&post.message) == Some("MAP")
                && post
                    .message
                    .pointer("/game_state/act")
                    .and_then(Value::as_u64)
                    == Some(3))
            .then_some((action.step, post.clone()))
        })
        .expect("fixture proceeds from the act-two boss chest to act three");

    let metadata = imported.metadata.expect("trace metadata");
    let mut lines = imported
        .lines
        .into_iter()
        .filter(|line| !matches!(line, TraceLine::Metadata(_)))
        .collect::<Vec<_>>();
    let action_index = lines
        .iter()
        .position(|line| {
            matches!(line, TraceLine::Action(action) if action.step == proceed_step && action.command.eq_ignore_ascii_case("PROCEED"))
        })
        .expect("boss proceed action remains in trace");
    let stable_index = lines
        .iter()
        .enumerate()
        .skip(action_index + 1)
        .find_map(|(index, line)| {
            matches!(line, TraceLine::State(state) if *state == stable_map).then_some(index)
        })
        .expect("stable act-three map state remains in trace");
    let mut transient = stable_map.clone();
    transient.step = proceed_step;
    *transient
        .message
        .pointer_mut("/game_state/screen_type")
        .expect("map screen type") = json!("NONE");
    *transient
        .message
        .pointer_mut("/game_state/screen_name")
        .expect("map screen name") = json!("NONE");
    *transient
        .message
        .pointer_mut("/game_state/room_type")
        .expect("map room type") = json!("TreasureRoomBoss");
    assert!(seed_start_is_candidate_boss_act_transient_frame(
        &transient.message
    ));

    let mut unresolved_lines = lines[..stable_index].to_vec();
    unresolved_lines.push(TraceLine::State(transient.clone()));
    let unresolved_transitions =
        trace_transitions(&unresolved_lines).expect("transient trace transitions");
    assert!(unresolved_transitions
        .transitions
        .iter()
        .any(|(_, action, post)| action.step == proceed_step
            && action.command.eq_ignore_ascii_case("PROCEED")
            && seed_start_is_candidate_boss_act_transient_frame(&post.message)));
    let unresolved_trace = crate::serialize_communication_mod_trace(&metadata, &unresolved_lines);
    let unresolved =
        verify_communication_mod_trace(&unresolved_trace).expect("transient-only trace parses");
    let proceed_disposition = unresolved
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == proceed_step && entry.command == "PROCEED");
    let proceed_diffs = unresolved
        .unexpected_diffs
        .iter()
        .filter(|entry| entry.action_step == proceed_step)
        .collect::<Vec<_>>();
    let proceed_unsupported = unresolved
        .unsupported
        .iter()
        .filter(|entry| entry.action_step == proceed_step)
        .collect::<Vec<_>>();
    assert_eq!(
        unresolved
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        1,
        "a transient boss frame cannot complete the proceed assertion: boundary={:?}, disposition={proceed_disposition:?}, diffs={proceed_diffs:?}, unsupported={proceed_unsupported:?}",
        unresolved
            .seed_start
            .as_ref()
            .map(|report| &report.first_boundary)
    );
    assert!(unresolved.verified.iter().all(|entry| {
        entry.action_step != proceed_step
            || entry.label != "boss reward proceed to settled next-act map"
    }));

    const POLL_STEP: u32 = 900_522;
    lines.insert(stable_index, TraceLine::State(transient));
    lines.insert(
        stable_index + 1,
        TraceLine::Action(TraceAction {
            step: POLL_STEP,
            command: "STATE".to_owned(),
            sent_at: None,
            playtime_seconds: None,
        }),
    );
    let settled_trace = crate::serialize_communication_mod_trace(&metadata, &lines);
    let settled = verify_communication_mod_trace(&settled_trace).expect("settled trace parses");
    assert!(
        settled.unexpected_diffs.is_empty(),
        "{:#?}",
        settled.unexpected_diffs
    );
    assert!(settled.unsupported.is_empty(), "{:#?}", settled.unsupported);
    assert_eq!(
        settled
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    let proceed = settled
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == proceed_step && entry.command == "PROCEED")
        .expect("boss proceed disposition");
    assert_eq!(proceed.disposition, ActionDispositionKind::Verified);
    assert!(proceed.deferred_assertion_reconciled);
    assert!(settled.verified.iter().any(|entry| {
        entry.action_step == POLL_STEP && entry.label == "stable next-act map observation poll"
    }));

    let mut forged_lines = lines;
    let TraceLine::State(forged_transient) = &mut forged_lines[stable_index] else {
        panic!("inserted transient state remains at its expected position");
    };
    *forged_transient
        .message
        .pointer_mut("/game_state/screen_name")
        .expect("transient screen name") = json!("FORGED_TRANSIENT");
    let forged_trace = crate::serialize_communication_mod_trace(&metadata, &forged_lines);
    let forged = verify_communication_mod_trace(&forged_trace).expect("forged trace parses");
    let forged_diff = forged
        .unexpected_diffs
        .iter()
        .find(|entry| entry.action_step == proceed_step)
        .expect("forged transient field must differ");
    assert!(
        forged_diff
            .diffs
            .iter()
            .any(|diff| diff.starts_with("screen_name:")),
        "{forged_diff:#?}"
    );
    let forged_proceed = forged
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == proceed_step && entry.command == "PROCEED")
        .expect("forged boss proceed disposition");
    assert_eq!(
        forged_proceed.disposition,
        ActionDispositionKind::UnexpectedDiff
    );
    assert!(!forged_proceed.deferred_assertion_reconciled);
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
        .find(|diff| diff.action_step == open_action_step && diff.label == "open boss relic chest")
        .and_then(|diff| {
            diff.diffs
                .iter()
                .find(|line| line.contains("boss_relic_ids"))
        })
        .expect("forged observed boss relic must differ from generated choices");
    assert!(relic_diff.contains(forged_name), "{relic_diff}");
}

#[test]
fn executing_combat_selection_verifies_without_a_deferred_marker() {
    let path =
        crate::corpus_path("fidelity_regressions/session-38-floor21-hex-dazed-insertion.jsonl");
    let content = std::fs::read_to_string(path).expect("session-38 trace");
    let imported = import_communication_mod_trace(&content).expect("trace imports");
    let original = verify_communication_mod_trace(&content).expect("original trace verifies");
    assert!(original.unexpected_diffs.is_empty(), "{original:#?}");
    for step in [1592, 1593, 1594, 1595] {
        let disposition = original
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == step)
            .unwrap_or_else(|| panic!("step {step} disposition"));
        assert_eq!(disposition.disposition, ActionDispositionKind::Verified);
        assert!(!disposition.deferred_assertion_reconciled);
    }

    let metadata = imported.metadata.expect("trace metadata");
    let mut lines = imported
        .lines
        .into_iter()
        .filter(|line| !matches!(line, TraceLine::Metadata(_)))
        .collect::<Vec<_>>();
    let transient = lines
        .iter_mut()
        .find_map(|line| match line {
            TraceLine::State(state)
                if state.step == 1592
                    && seed_start_is_transient_combat_post_state(&state.message) =>
            {
                Some(state)
            }
            _ => None,
        })
        .expect("Armaments executing hand-select frame");
    forge_grid_observation(&mut transient.message);

    let forged_trace = crate::serialize_communication_mod_trace(&metadata, &lines);
    let forged = verify_communication_mod_trace(&forged_trace).expect("forged trace parses");
    let diff = forged
        .unexpected_diffs
        .iter()
        .find(|entry| entry.action_step == 1592)
        .expect("forged transient screen must differ");
    assert!(
        diff.diffs
            .iter()
            .any(|line| line.starts_with("screen_type:")),
        "{diff:#?}"
    );
    let disposition = forged
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 1592)
        .expect("forged Armaments disposition");
    assert_eq!(
        disposition.disposition,
        ActionDispositionKind::UnexpectedDiff
    );
    assert!(!disposition.deferred_assertion_reconciled);
    assert_eq!(
        forged
            .seed_start
            .as_ref()
            .and_then(|report| report.sim_run_state.as_ref()),
        original
            .seed_start
            .as_ref()
            .and_then(|report| report.sim_run_state.as_ref()),
        "transient observation must not steer simulator state"
    );
}

#[test]
fn armaments_confirm_accepts_source_hand_settlement_frame() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-ab69c7e7be342616.jsonl");
    let content = std::fs::read_to_string(path).expect("Armaments trace");
    let report = verify_communication_mod_trace(&content).expect("Armaments trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 37)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .map(|integrity| integrity.unresolved_transient_assertions),
        Some(0)
    );
}

#[test]
fn discovery_choose_accepts_source_hand_settlement_frame() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-dfe4b19eed53a0f1.jsonl");
    let content = std::fs::read_to_string(path).expect("Discovery trace");
    let report = verify_communication_mod_trace(&content).expect("Discovery trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 43)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn calling_bell_indexed_relic_choice_reconciles_generated_offers() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-df31745cc7f48c21.jsonl");
    let content = std::fs::read_to_string(path).expect("Calling Bell trace");
    let report = verify_communication_mod_trace(&content).expect("Calling Bell trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 10)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn neow_proceed_discards_unclaimed_tiny_house_overlay_rewards() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-fde0756ddd0eeb19.jsonl");
    let content = std::fs::read_to_string(path).expect("Tiny House trace");
    let report = verify_communication_mod_trace(&content).expect("Tiny House trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 10)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn tiny_house_parent_reward_skip_discards_unclaimed_offers() {
    let Some(content) = crate::load_corpus_file(
        "fidelity_regressions/random-fidelity-200c4c2257bb6033-tiny-house-parent-skip.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("captured Tiny House parent reward SKIP trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 9
            && transition.command == "SKIP"
            && transition.label == "skip Tiny House reward overlay to event"
    }));
}

#[test]
fn evolve_does_not_draw_for_shame_curse() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-a13717f0e4c2dd25.jsonl");
    let content = std::fs::read_to_string(path).expect("Evolve curse trace");
    let report = verify_communication_mod_trace(&content).expect("Evolve curse trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 182)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn ssssserpent_settles_doubt_before_continue_screen() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-1429bd0063c30c94.jsonl");
    let content = std::fs::read_to_string(path).expect("Ssssserpent trace");
    let report = verify_communication_mod_trace(&content).expect("Ssssserpent trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 117)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn neow_astrolabe_accepts_pre_transform_source_frame() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-baa0bd2e49a54236.jsonl");
    let content = std::fs::read_to_string(path).expect("Astrolabe trace");
    let report = verify_communication_mod_trace(&content).expect("Astrolabe trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 8)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn duplicator_uses_source_pray_choice_label() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-83e76cac8d987cd3.jsonl");
    let content = std::fs::read_to_string(path).expect("Duplicator trace");
    let report = verify_communication_mod_trace(&content).expect("Duplicator trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 696)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn elixir_exhaust_cards_reconcile_source_discard_frame() {
    let path = crate::corpus_path("permanent_traces/random-fidelity-39e501b30ef74e72.jsonl");
    let content = std::fs::read_to_string(path).expect("Elixir trace");
    let report = verify_communication_mod_trace(&content).expect("Elixir trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 188)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
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
    assert_eq!(parsed.verification_starting_hp, None);
}

#[test]
fn start_verify_command_accepts_bounded_starting_hp() {
    let parsed = parse_start_command(&TraceAction {
        step: 1,
        command: "START_VERIFY IRONCLAD 0 TEST 10000".to_owned(),
        sent_at: None,
        playtime_seconds: None,
    })
    .expect("start verify command")
    .expect("valid start verify command");

    assert_eq!(parsed.external_seed, "TEST");
    assert_eq!(parsed.verification_starting_hp, Some(10_000));
    assert_eq!(parsed.starting_hp(), 10_000);

    for hp in ["0", "1000001", "nope"] {
        let parsed = parse_start_command(&TraceAction {
            step: 1,
            command: format!("START_VERIFY IRONCLAD 0 TEST {hp}"),
            sent_at: None,
            playtime_seconds: None,
        });
        assert!(matches!(
            parsed,
            Some(Err(SimRealError::MalformedStartCommand(_)))
        ));
    }
}

#[test]
fn boss_relic_deck_overlay_requires_stable_reconciliation() {
    let path = crate::corpus_path("fidelity_regressions/session-1-boss-relic-deck-overlay.jsonl");
    let content = std::fs::read_to_string(path).expect("boss relic overlay trace");
    let report = verify_communication_mod_trace(&content).expect("overlay trace verifies");
    let relic_pick = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 350 && entry.command == "CHOOSE 0")
        .expect("boss relic pick disposition");

    assert_eq!(relic_pick.disposition, ActionDispositionKind::Verified);
    assert!(relic_pick.deferred_assertion_reconciled);
    assert!(report.verified.iter().any(|entry| {
        entry.action_step == 350 && entry.label == "boss relic reward reconciled after deck overlay"
    }));
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );

    let imported = import_communication_mod_trace(&content).expect("overlay trace imports");
    let metadata = imported.metadata.expect("overlay trace metadata");
    let mut lines = imported
        .lines
        .into_iter()
        .filter(|line| !matches!(line, TraceLine::Metadata(_)))
        .collect::<Vec<_>>();
    let mut divergent_lines = lines.clone();
    let settled_chest = divergent_lines
        .iter_mut()
        .find_map(|line| match line {
            TraceLine::State(state) if state.step == 352 => Some(state),
            _ => None,
        })
        .expect("settled boss chest state");
    settled_chest
        .message
        .pointer_mut("/game_state/relics")
        .and_then(Value::as_array_mut)
        .expect("settled relic list")
        .pop()
        .expect("picked boss relic is visible after closing the overlay");
    let divergent_trace = crate::serialize_communication_mod_trace(&metadata, &divergent_lines);
    let divergent =
        verify_communication_mod_trace(&divergent_trace).expect("divergent overlay trace parses");
    let divergent_pick = divergent
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 350 && entry.command == "CHOOSE 0")
        .expect("divergent boss relic pick disposition");
    assert_eq!(
        divergent_pick.disposition,
        ActionDispositionKind::Unsupported
    );

    let overlay_poll = lines
        .iter()
        .rposition(|line| matches!(line, TraceLine::State(state) if state.step == 351))
        .expect("overlay poll state");
    lines.truncate(overlay_poll + 1);
    let unresolved_trace = crate::serialize_communication_mod_trace(&metadata, &lines);
    let unresolved =
        verify_communication_mod_trace(&unresolved_trace).expect("truncated overlay trace parses");
    assert_eq!(
        unresolved
            .action_integrity
            .as_ref()
            .expect("truncated action integrity")
            .unresolved_transient_assertions,
        1
    );
    let unresolved_pick = unresolved
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 350 && entry.command == "CHOOSE 0")
        .expect("unresolved boss relic pick disposition");
    assert_eq!(
        unresolved_pick.disposition,
        ActionDispositionKind::PendingTransient
    );
    assert!(!unresolved_pick.deferred_assertion_reconciled);
}

#[test]
fn tiny_house_overlay_uses_authoritative_pre_pick_state() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-43601d9614827319.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Tiny House overlay regression trace verifies");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_eq!(boundary.category, "none");
    let pick = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 418 && entry.command == "CHOOSE 2")
        .expect("Tiny House pick disposition");
    assert_eq!(pick.disposition, ActionDispositionKind::Verified);
    assert!(pick.deferred_assertion_reconciled);
    assert!(report.verified.iter().any(|entry| {
        entry.action_step == 418
            && entry.label == "boss relic reward reconciled at captured Tiny House deck overlay"
    }));
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn ftue_dismissal_routes_through_typed_overlay_pre_dispatch() {
    let pre = TraceState {
        step: 10,
        received_at: None,
        message: json!({
            "game_state": {
                "screen_name": "FTUE"
            }
        }),
    };
    let action = TraceAction {
        step: 11,
        command: "CLICK LEFT 1080 700 250".to_owned(),
        sent_at: None,
        playtime_seconds: None,
    };
    let post = TraceState {
        step: 12,
        received_at: None,
        message: json!({
            "game_state": {
                "screen_type": "COMBAT_REWARD"
            }
        }),
    };
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Reward;
    run.reward = Some(RewardScreen {
        continuation: sts_core::RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });
    let mut phase = SeedStartPhase::NeowTalk;
    let mut pending_overlay = None;
    let mut reconciled = Vec::new();
    let mut report = SimRealReport {
        total_actions: 1,
        ignored_tail_actions: 0,
        action_dispositions: Vec::new(),
        action_integrity: None,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let result = replay::seed_start_handle_overlay_command(
        &pre,
        &action,
        &post,
        &mut phase,
        Some(&run),
        &mut pending_overlay,
        &mut reconciled,
        &mut report,
    );

    assert!(matches!(result, replay::SeedStartPreDispatch::Handled));
    assert_eq!(phase, SeedStartPhase::Reward);
    assert!(report
        .unexpected_diffs
        .iter()
        .all(|diff| diff.label == "dismiss FTUE overlay"));
}

#[test]
fn dramatic_entrance_maps_from_observed_card_json() {
    let card = json!({"id": "Dramatic Entrance", "name": "Dramatic Entrance", "upgrades": 0});
    assert_eq!(
        content_id_from_card_value(&card),
        Some(DRAMATIC_ENTRANCE_ID)
    );
}

#[test]
fn colorless_reward_cards_map_from_observed_card_json() {
    use sts_core::content::cards::{DARK_SHACKLES_ID, DISCOVERY_ID, SECRET_WEAPON_ID};

    for (id, expected, key) in [
        (
            "Dramatic Entrance",
            DRAMATIC_ENTRANCE_ID,
            "Dramatic Entrance",
        ),
        ("Dark Shackles", DARK_SHACKLES_ID, "Dark Shackles"),
        ("Discovery", DISCOVERY_ID, "Discovery"),
        ("Secret Weapon", SECRET_WEAPON_ID, "Secret Weapon"),
    ] {
        let card = json!({"id": id, "name": id, "upgrades": 0});

        assert_eq!(content_id_from_card_value(&card), Some(expected));
        assert_eq!(content_key(expected), key);
    }
}

#[test]
fn dropkick_maps_from_observed_card_json() {
    let card = json!({"id": "Dropkick", "name": "Dropkick", "upgrades": 0});

    assert_eq!(content_id_from_card_value(&card), Some(DROPKICK_ID));
    assert_eq!(content_key(DROPKICK_ID), "Dropkick");
}

#[test]
fn burn_maps_from_observed_card_json() {
    let card = json!({"id": "Burn", "name": "Burn", "upgrades": 0});

    assert_eq!(content_id_from_card_value(&card), Some(BURN_ID));
    assert_eq!(content_key(BURN_ID), "Burn");
}

#[test]
fn long_trace_observed_cards_map_from_card_json() {
    use sts_core::content::cards::{
        BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BURNING_PACT_ID, COMBUST_ID, DARK_EMBRACE_ID, DAZED_ID,
        DOUBLE_TAP_ID, FEEL_NO_PAIN_ID, RAGE_ID, REAPER_ID, RUPTURE_ID, WOUND_ID,
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
        let card = json!({"id": id, "name": id, "upgrades": 0});

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
fn observed_combat_card_projection_preserves_unknown_identity() {
    let cards = json!([
        {"id": "Strike_R", "name": "Strike", "upgrades": 0},
        {"id": "future-card", "name": "Future Card", "upgrades": 0}
    ]);

    assert_eq!(
        combat_card_ids(Some(&cards)),
        vec!["Strike_R".to_owned(), "future-card".to_owned()]
    );
}

#[test]
fn observed_unmodeled_upgrade_retains_visible_upgrade_evidence() {
    let named = json!({"id": "Burn", "name": "Burn+", "upgrades": 1});
    assert_eq!(content_id_from_card_value(&named), None);
    assert_eq!(
        observed_card_projection_key(&named).as_deref(),
        Some("Burn+")
    );

    let unnamed = json!({"id": "future-card", "upgrades": 2});
    assert_eq!(
        observed_card_projection_key(&unnamed).as_deref(),
        Some("future-card [upgrades=2]")
    );
}

#[test]
fn observed_card_mapping_does_not_default_missing_upgrades() {
    let malformed = json!({"id": "Strike_R", "name": "Strike"});

    assert_eq!(content_id_from_card_value(&malformed), None);
    assert_eq!(observed_card_projection_key(&malformed), None);
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
fn champ_debuff_observed_intent_imports_weak_and_vulnerable() {
    use sts_core::content::monsters::CHAMP_ID;

    let monster = json!({
        "id": "Champ",
        "intent": "DEBUFF",
        "move_id": 6,
        "move_base_damage": -1
    });

    assert_eq!(
        observed_intent(&monster, CHAMP_ID, 0),
        MonsterIntent::ApplyPlayerFrailWeakVulnerable {
            frail: 0,
            weak: 2,
            vulnerable: 2,
        }
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
fn mind_bloom_rich_permanent_trace_preserves_normality_settlement_frame() {
    let content = std::fs::read_to_string(crate::corpus_path(
        "permanent_traces/random-fidelity-350adbf8276a3c06.jsonl",
    ))
    .expect("Mind Bloom permanent trace");
    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report
        .verified
        .iter()
        .any(|transition| transition.action_step == 1384));
}

#[test]
fn random_fidelity_addict_obtain_may_end_on_the_source_transient_deck_frame() {
    let content = std::fs::read_to_string(crate::corpus_path(
        "permanent_traces/random-fidelity-3e3e39b3e8607252.jsonl",
    ))
    .expect("Addict permanent trace");
    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "the captured Addict transient is source-valid: {report:#?}"
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action-integrity report")
            .unresolved_transient_assertions,
        0,
        "the pending Shame obtain is represented by the typed event state: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 327)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn grid_trace_choice_label_does_not_preview_upgrade_existing_cards() {
    use sts_core::content::cards::{
        RITUAL_DAGGER_ID, SEARING_BLOW_PLUS_ID, TRUE_GRIT_ID, TRUE_GRIT_PLUS_ID,
    };

    let mut run = RunState::map_fixture();
    run.gain_relic_key(RelicKey::ToxicEgg)
        .expect("Toxic Egg pickup succeeds");

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
    let mut searing_blow = CardInstance::new(CardId::new(4), SEARING_BLOW_PLUS_ID);
    searing_blow.searing_blow_upgrades = 1;
    assert_eq!(
        grid_trace_choice_label(&run, &searing_blow),
        "searing blow+1"
    );
}

#[test]
fn seed_start_event_grid_requires_explicit_confirm_after_final_selection() {
    let mut run = RunState::seeded_ironclad(1, 0);
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

    let after_second =
        seed_start_apply_grid_command(&after_first, "CHOOSE 1").expect("second source is selected");
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
        seed_start_event_simulated_subset_with_delayed_deck_append(&completed, Some(2));
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
    let mut run = RunState::seeded_ironclad(1, 0);
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
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: sts_core::CardRewardFlow::None,
    });

    assert!(seed_start_reward_sequence_complete(&run));
    assert_eq!(
        seed_start_phase_after_reward_completion(&run),
        SeedStartPhase::Reward
    );
}

#[test]
fn observed_map_return_cannot_verify_itself() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Idle;
    run.gold = 99;
    let forged_post = json!({
        "game_state": {
            "screen_type": "MAP",
            "gold": 1099
        }
    });
    let action = TraceAction {
        step: 7,
        command: "CHOOSE 0".to_owned(),
        sent_at: None,
        playtime_seconds: None,
    };
    let mut report = SimRealReport {
        total_actions: 1,
        ignored_tail_actions: 0,
        action_dispositions: Vec::new(),
        action_integrity: None,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    seed_start_compare_map_return(
        &mut report,
        &action,
        &forged_post,
        seed_start_simulated_map_return(&run).expect("core map projection"),
    );

    assert!(report.verified.is_empty());
    let diff = report
        .unexpected_diffs
        .iter()
        .find(|diff| diff.label == "map return")
        .expect("forged map return must differ from simulator projection");
    assert!(
        diff.diffs.iter().any(|line| line == "gold: 1099 != 99"),
        "{diff:#?}"
    );
}

#[test]
fn map_phase_dispatches_noncombat_potion_use_without_treating_it_as_a_node_choice() {
    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Idle;
    run.potions = vec![Potion::FruitJuice];
    run.empty_potion_slots = vec![1, 2];
    let expected = apply_run_action(
        &run,
        RunAction::UsePotion {
            slot: 0,
            target: None,
        },
    )
    .expect("Fruit Juice is usable from the map");
    let projection = seed_start_simulated_map_return(&expected).expect("map projection");
    let relics = projection["relic_ids"]
        .as_array()
        .expect("projected relic ids")
        .iter()
        .map(|id| json!({ "id": id }))
        .collect::<Vec<_>>();
    let potions = expected
        .potions
        .iter()
        .map(|potion| json!({ "id": potion_trace_name(*potion) }))
        .collect::<Vec<_>>();
    let pre = TraceState {
        step: 1,
        received_at: None,
        message: json!({ "game_state": { "screen_type": "MAP" } }),
    };
    let action = TraceAction {
        step: 2,
        command: "POTION USE 0".to_owned(),
        sent_at: None,
        playtime_seconds: None,
    };
    let post = TraceState {
        step: 2,
        received_at: None,
        message: json!({
            "game_state": {
                "screen_type": projection["screen_type"],
                "floor": projection["floor"],
                "gold": projection["gold"],
                "current_hp": projection["current_hp"],
                "max_hp": projection["max_hp"],
                "deck": observed_deck_cards(&expected.deck),
                "relics": relics,
                "potions": potions,
                "choice_list": projection["choices"],
                "screen_state": {
                    "first_node_chosen": projection["first_node_chosen"],
                    "current_node": projection["current_node"],
                    "next_nodes": projection["next_nodes"],
                },
            }
        }),
    };
    let start = StartRunCommand {
        action_step: 0,
        character: "IRONCLAD".to_owned(),
        ascension: 0,
        external_seed: "TEST".to_owned(),
        numeric_seed: 0,
        verification_starting_hp: None,
    };
    let mut pending_curse = None;
    let mut pending_curse_rng = false;
    let mut map_path = Vec::new();
    let mut event_index = 0;
    let mut combat_index = 0;
    let mut seed_sim = Some(run);
    let mut pending_combat = None;
    let mut phase = SeedStartPhase::Map;
    let mut smoke_bomb_ui = None;
    let mut report = SimRealReport {
        total_actions: 1,
        ignored_tail_actions: 0,
        action_dispositions: Vec::new(),
        action_integrity: None,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let disposition = replay::seed_start_handle_map_phase(
        &pre,
        &action,
        &post,
        &start,
        BossUnlockState::default(),
        &mut pending_curse,
        &mut pending_curse_rng,
        &mut map_path,
        &mut event_index,
        &mut combat_index,
        &mut seed_sim,
        &mut smoke_bomb_ui,
        &mut pending_combat,
        &mut phase,
        &mut report,
    );

    assert!(matches!(disposition, replay::SeedStartPreDispatch::Handled));
    assert_eq!(seed_sim, Some(expected));
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 2
            && transition.command == "POTION USE 0"
            && transition.label == "map potion use"
    }));
}

#[test]
fn seed_start_map_return_rejects_missing_core_map_authority() {
    let mut run = RunState::map_fixture();
    run.map = None;

    assert_eq!(
        seed_start_simulated_map_return(&run),
        Err("core run state has no authoritative map".to_owned())
    );
}

#[test]
fn seed_start_map_return_rejects_missing_current_node() {
    let mut run = RunState::map_fixture();
    run.map.as_mut().expect("map fixture").current_node = sts_core::MapNodeId::new(999);

    assert_eq!(
        seed_start_simulated_map_return(&run),
        Err("core map current node is missing".to_owned())
    );
}

#[test]
fn seed_start_map_return_uses_core_wing_boots_destinations() {
    let seed = sts_seed_string_to_long("3WUU08ZMEVMV2");
    let mut run = seed_start_seeded_idle_run(seed, 0, &ironclad_starter_deck_keys());
    run.relics.push(Relic::WingBoots);
    run.wing_boots_charges = 3;
    let first_node = legal_map_decisions(&run)
        .expect("valid map decisions")
        .into_iter()
        .find(|action| match action {
            sts_core::MapAction::ChooseNode { node_id } => seed_start_map_node_xy(*node_id).0 == 2,
        })
        .expect("seed offers x=2 as a first-row node");
    run = apply_map_action_on_run(&run, first_node).expect("first map node is legal");
    run.phase = RunPhase::Reward;

    let projected = seed_start_simulated_map_return(&run).expect("core Wing Boots projection");

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
fn designer_punch_choice_uses_communication_mod_label() {
    assert_eq!(
        seed_start_visible_event_choice_label_for_event(Event::Designer, 1, "Get punched (12 HP)"),
        Some("punch".to_owned())
    );
}

#[test]
fn upgrade_shrine_leave_projects_the_source_event_settlement_frame() {
    let mut source = RunState::seeded_ironclad(1, 0);
    source.phase = RunPhase::Event;
    source.event = Some(EventScreen {
        event: Event::UpgradeShrine,
        choices: vec![
            EventChoice {
                label: "Pray".to_owned(),
            },
            EventChoice {
                label: "Leave".to_owned(),
            },
        ],
        stage: 0,
        event_data: 0,
    });
    let settled = apply_event_action(&source, EventAction::Choose { choice_index: 1 })
        .expect("Upgrade Shrine Leave returns to the map phase");
    let transient = replay::seed_start_upgrade_shrine_leave_transient(&source, &settled)
        .expect("Upgrade Shrine has a source settlement frame");

    assert_eq!(settled.phase, RunPhase::Idle);
    assert!(settled.event.is_none());
    assert_eq!(transient.phase, RunPhase::Event);
    assert_eq!(
        seed_start_event_simulated_subset(&transient)["choices"],
        json!(["leave"])
    );
    assert_eq!(
        seed_start_event_simulated_subset(&transient)["screen_type"],
        json!("EVENT")
    );
}

#[test]
fn random_fidelity_upgrade_shrine_leave_transient_endpoint_replays() {
    for trace_name in [
        "permanent_traces/random-fidelity-4071b226e326d68f.jsonl",
        "permanent_traces/random-fidelity-f90b4d20ff89e9ff.jsonl",
    ] {
        let Some(content) = crate::load_corpus_file(trace_name) else {
            continue;
        };
        let report = verify_seed_start_communication_mod_trace(&content)
            .expect("Upgrade Shrine transient endpoint trace verifies");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.unsupported.is_empty(), "{report:#?}");
        assert_eq!(
            report
                .seed_start
                .as_ref()
                .expect("seed-start report")
                .first_boundary
                .category,
            "none",
            "{trace_name}: {report:#?}"
        );
        assert_eq!(
            report
                .action_integrity
                .as_ref()
                .expect("action integrity")
                .unresolved_transient_assertions,
            0,
            "{trace_name}: {report:#?}"
        );
        assert!(
            report.verified.iter().any(|entry| {
                entry.label == "Upgrade Shrine leave reconciled at captured transient endpoint"
            }),
            "{trace_name}: {report:#?}"
        );
    }
}

#[test]
fn seed_start_mushrooms_event_uses_communication_mod_identity_and_labels() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.current_floor = 8;
    run.phase = RunPhase::Event;
    run.event = Some(event_screen(Event::HypnotizingColoredMushrooms));

    let subset = seed_start_event_simulated_subset(&run);

    assert_eq!(subset["event_id"], "mushrooms");
    assert_eq!(subset["choices"], json!(["stomp", "eat"]));
}

#[test]
fn seed_start_colosseum_uses_communication_mod_outcome_choice_ids() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.current_act = 2;
    run.current_floor = 30;
    run.phase = RunPhase::Event;
    run.event = Some(EventScreen {
        event: Event::Colosseum,
        choices: vec![
            EventChoice {
                label: "Flee".to_owned(),
            },
            EventChoice {
                label: "Fight Nobs".to_owned(),
            },
        ],
        stage: 2,
        event_data: 0,
    });

    let subset = seed_start_event_simulated_subset(&run);

    assert_eq!(subset["event_id"], "colosseum");
    assert_eq!(subset["choices"], json!(["cowardice", "victory"]));
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
    let mut run = RunState::seeded_ironclad(1, 0);
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
fn match_and_keep_removal_stale_pre_binds_by_visible_index() {
    // Pre still lists card4 after the sim flipped it. Index 7 on the shorter
    // live board is card9, not the stale pre label warcry.
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.event = Some(EventScreen {
        event: Event::MatchAndKeep,
        stage: 2,
        event_data: 0,
        choices: [
            "card0", "card1", "card3", "card5", "card6", "bash", "warcry", "card9", "card10",
        ]
        .into_iter()
        .map(|label| EventChoice {
            label: label.to_owned(),
        })
        .collect(),
    });
    let pre_message = json!({
        "game_state": {
            "screen_type": "EVENT",
            "choice_list": [
                "card0", "card1", "card3", "card4", "card5", "card6", "bash", "warcry", "card9",
                "card10"
            ],
            "screen_state": { "event_id": "Match and Keep!" }
        }
    });

    assert_eq!(
        seed_start_event_choice_index_for_communication_mod(&run, 7, &pre_message),
        Some(7)
    );
    assert_eq!(run.event.as_ref().unwrap().choices[7].label, "card9");
}

#[test]
fn match_and_keep_resolution_stale_pre_binds_by_card_label() {
    // Pre is mid-pair (card9 still cardN) while sim already resolved names.
    // Collector CHOOSE 4 targets card6 on the shorter pre list.
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.event = Some(EventScreen {
        event: Event::MatchAndKeep,
        stage: 2,
        event_data: 0,
        choices: [
            "card0",
            "card1",
            "card3",
            "deep breath",
            "card5",
            "card6",
            "bash",
            "warcry",
            "inflame",
            "card10",
        ]
        .into_iter()
        .map(|label| EventChoice {
            label: label.to_owned(),
        })
        .collect(),
    });
    let pre_message = json!({
        "game_state": {
            "screen_type": "EVENT",
            "choice_list": [
                "card0", "card1", "card3", "card5", "card6", "bash", "warcry", "card9", "card10"
            ],
            "screen_state": { "event_id": "Match and Keep!" }
        }
    });

    let mapped =
        seed_start_event_choice_index_for_communication_mod(&run, 4, &pre_message).expect("maps");
    assert_eq!(run.event.as_ref().unwrap().choices[mapped].label, "card6");
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
        Some(BRONZE_AUTOMATON_ID)
    );
    assert_eq!(
        content_id_from_game_monster_id("BronzeOrb"),
        Some(BRONZE_ORB_ID)
    );
    assert_eq!(
        content_id_from_game_monster_id("Orb Walker"),
        Some(ORB_WALKER_ID)
    );
    assert_eq!(content_id_from_game_monster_id("not-a-monster"), None);
}

#[test]
fn neow_generated_identity_display_names_are_mapped() {
    use sts_core::content::cards::{
        ARMAMENTS_ID, CHRYSALIS_ID, DECAY_ID, DOUBT_ID, FEED_ID, HAND_OF_GREED_ID, IMPERVIOUS_ID,
        LIMIT_BREAK_ID, MAGNETISM_ID, MAYHEM_ID, PARASITE_ID, SECRET_WEAPON_ID, TRANSMUTATION_ID,
        WRITHE_ID,
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
fn visible_card_projection_preserves_modeled_and_instance_upgrades() {
    use sts_core::content::cards::{BURN_ID, SEARING_BLOW_PLUS_ID, STRIKE_R_PLUS_ID};

    let strike_plus = CardInstance::new(CardId::new(1), STRIKE_R_PLUS_ID);
    assert_eq!(simulated_card_projection_key(&strike_plus), "Strike_R+");
    assert_eq!(
        observed_card_projection_key(&json!({"id": "Strike_R", "name": "Strike+", "upgrades": 1}))
            .as_deref(),
        Some("Strike_R+")
    );

    let mut burn_plus = CardInstance::new(CardId::new(2), BURN_ID);
    burn_plus.upgrades = 1;
    assert_eq!(simulated_card_projection_key(&burn_plus), "Burn+");

    let mut searing_blow_plus_two = CardInstance::new(CardId::new(3), SEARING_BLOW_PLUS_ID);
    searing_blow_plus_two.searing_blow_upgrades = 2;
    assert_eq!(
        simulated_card_projection_key(&searing_blow_plus_two),
        "Searing Blow+2"
    );
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

    assert_eq!(
        relic_key_from_trace_name("N'loth's Hungry Face"),
        Some(RelicKey::NlothsMask)
    );
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
    run.combat = Some(
        run.init_combat(CombatState::initial_fixture())
            .expect("combat initializes"),
    );

    let subset = seed_start_simulated_combat_subset(&run, false);

    assert_eq!(subset["current_hp"], json!(36));
    assert_eq!(subset["combat_player_hp"], json!(36));
}

#[test]
fn combat_subset_uses_run_floor() {
    let mut run = RunState::map_fixture();
    run.current_floor = 17;
    run.combat = Some(
        run.init_combat(CombatState::initial_fixture())
            .expect("combat initializes"),
    );

    let subset = seed_start_simulated_combat_subset(&run, false);

    assert_eq!(subset["floor"], json!(17));
}

#[test]
fn combat_subset_uses_simulated_monster_identity_and_max_hp() {
    let mut run = RunState::map_fixture();
    let mut combat = run
        .init_combat(CombatState::initial_fixture())
        .expect("combat initializes");
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

    combat.decision = Some(CombatDecisionState::ToolboxCardReward {
        choices: vec![CardInstance::new(source_card_id, STRIKE_R_ID)],
    });
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
    combat.decision = None;

    combat.decision = Some(CombatDecisionState::ExhaustSelect {
        state: ExhaustSelectState {
            purpose: ExhaustSelectPurpose::BurningPactDraw2,
            source_card_id: Some(source_card_id),
            source_card: None,
            selected_hand_indices: Vec::new(),
            pending_actions: VecDeque::new(),
            interrupted_by_cultist_potion: false,
        },
    });
    assert_eq!(
        seed_start_simulated_combat_screen_type(&combat),
        "HAND_SELECT"
    );
    combat.exhaust_select_mut().unwrap().purpose = ExhaustSelectPurpose::ExhumeReturnToHand;
    assert_eq!(seed_start_simulated_combat_screen_type(&combat), "GRID");
    combat.decision = None;

    combat.decision = Some(CombatDecisionState::DrawSelect {
        state: DrawSelectState {
            purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
            source_card_id,
            selectable_card_ids: Vec::new(),
            selected_draw_index: None,
        },
    });
    assert_eq!(seed_start_simulated_combat_screen_type(&combat), "GRID");
    combat.decision = None;

    combat.decision = Some(CombatDecisionState::DiscardSelect {
        state: DiscardSelectState {
            purpose: DiscardSelectPurpose::HeadbuttPutOnDraw,
            source_card_id: Some(source_card_id),
            source_card: None,
            selected_discard_indices: Vec::new(),
            max_choices: 1,
            selected_discard_index: None,
        },
    });
    assert_eq!(seed_start_simulated_combat_screen_type(&combat), "GRID");

    let mut run = RunState::map_fixture();
    run.phase = RunPhase::Combat;
    run.combat = Some(combat.clone());
    assert_eq!(
        seed_start_active_combat_decision(&run).expect("one decision"),
        Some(SeedStartCombatDecision::DiscardSelect)
    );

    let (draw_choice, draw_label) =
        seed_start_bind_combat_decision_command(SeedStartCombatDecision::DrawSelect, "CHOOSE 2")
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

    run.combat
        .as_mut()
        .expect("combat")
        .queued_decisions
        .push_back(CombatDecisionState::DrawSelect {
            state: DrawSelectState {
                purpose: DrawSelectPurpose::SecretTechniqueSkillToHand,
                source_card_id,
                selectable_card_ids: Vec::new(),
                selected_draw_index: None,
            },
        });
    assert_eq!(
        seed_start_active_combat_decision(&run).expect("active decision remains unique"),
        Some(SeedStartCombatDecision::DiscardSelect)
    );
}

#[test]
fn encounter_observation_projects_visible_toolbox_cards() {
    use sts_core::content::cards::{BASH_ID, DEFEND_R_ID};

    let message = json!({
        "game_state": {
            "screen_type": "CARD_REWARD",
            "screen_state": {
                "cards": [
                    {"id": "Strike_R", "upgrades": 0},
                    {"id": "Defend_R", "upgrades": 0},
                    {"id": "Bash", "upgrades": 0}
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
    let mut combat = run
        .init_combat(CombatState::initial_fixture())
        .expect("combat initializes");
    combat.decision = Some(CombatDecisionState::DiscoveryCardReward {
        choices: vec![
            CardInstance::new(CardId::new(100), SHRUG_IT_OFF_ID),
            CardInstance::new(CardId::new(101), PUMMEL_ID),
            CardInstance::new(CardId::new(102), SEARING_BLOW_ID),
        ],
        source_card: None,
    });
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
    run.gain_relic_key(RelicKey::ToxicEgg)
        .expect("Toxic Egg pickup succeeds");

    assert_eq!(shop_card_display_key(&run, WARCRY_ID), "Warcry+");
    assert_eq!(shop_card_display_key(&run, PANACEA_ID), "Panacea+");
}

#[test]
fn shop_room_fruit_juice_use_replays_from_seed_start() {
    let Some(content) = crate::load_corpus_file(
        "fidelity_regressions/random-fidelity-d7a5a5c4225dba29-shop-room-fruit-juice.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("captured shop-room Fruit Juice trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 106
            && transition.command == "POTION USE 0"
            && transition.label == "shop room potion use"
    }));
}

#[test]
fn shop_choose_binding_uses_core_merchant_state_and_rejects_room_index_drift() {
    let mut run = RunState::seeded_ironclad(1_218_623, 0);
    run.event = None;
    run.gold = 999;
    sts_core::enter_shop_room(&mut run).expect("shop entry succeeds");
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
    let run =
        seed_start_apply_neow_relic_reward(numeric_seed, &ironclad_starter_deck_keys(), &option);
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
    let content = serialize_trace_test_lines(lines.clone());

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
    assert!(
        relic_ids_for_simulated_subset(carried).contains(&seed_start_newest_trace_relic_name(&run))
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
fn seed_start_neow_curse_rare_relic_delays_visible_curse_until_after_leave() {
    let numeric_seed = -1_396_911_955_486_209_732;
    let seed_string = "51KQHCFJ38T5Z";
    let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
        .expect("session-197 slot 2 rare relic option");
    assert_eq!(option.drawback, NeowDrawback::Curse);
    assert_eq!(option.reward, NeowRewardType::OneRareRelic);
    let run =
        seed_start_apply_neow_relic_reward(numeric_seed, &ironclad_starter_deck_keys(), &option);
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow rare relic"
    }));
}

#[test]
fn seed_start_neow_curse_rare_relic_accepts_settled_curse_on_leave_ready() {
    // FIDL00214 / random-fidelity-36b4907ea3fed69f shows Shame already on the
    // master deck in the leave-ready frame after curse+rare relic (settled),
    // while session-197 captures the lagged pre-curse frame (deferred).
    let numeric_seed = 34_961_238_618_114_i64;
    let seed_string = "FIDL00214";
    let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
        .expect("FIDL00214 slot 2 rare relic option");
    assert_eq!(option.drawback, NeowDrawback::Curse);
    assert_eq!(option.reward, NeowRewardType::OneRareRelic);
    let run =
        seed_start_apply_neow_relic_reward(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let settled_deck: Vec<_> = deck_content_keys(&run.deck)
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    assert_eq!(
        settled_deck
            .last()
            .and_then(|card| card.get("id"))
            .and_then(|id| id.as_str()),
        Some("Shame")
    );
    let post_relics = vec![
        json!({ "name": "Burning Blood" }),
        json!({ "name": seed_start_newest_trace_relic_name(&run) }),
    ];
    let starting_deck: Vec<_> = ironclad_starter_deck_keys()
        .into_iter()
        .map(|id| json!({ "id": id }))
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
        json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
        json!({"type": "state", "step": 3, "message": {"game_state": {
            "screen_type": "EVENT",
            "ascension_level": 0,
            "floor": 0,
            "gold": 99,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck": settled_deck,
            "relics": post_relics,
            "choice_list": ["leave"]
        }}}),
    ];
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow rare relic"
    }));
    let carried = report
        .seed_start
        .as_ref()
        .and_then(|seed_start| seed_start.sim_run_state.as_ref())
        .expect("seed-start carries simulator state after settled curse");
    assert_eq!(
        deck_content_keys(&carried.deck).last().map(String::as_str),
        Some("Shame"),
        "settled leave-ready frame must keep Shame on the carried deck"
    );
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

    let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");

    assert!(seed_start_neow_option_is_supported_boss_swap(option));
    assert!(!run.relics.contains(&Relic::BurningBlood));
    assert_eq!(run.relics.len(), 1);

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
    run.gain_relic(Relic::BlackBlood)
        .expect("Black Blood pickup succeeds");

    let relic_ids = relic_ids_for_simulated_subset(&run);

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
    apply_neow_lament_reward(&mut run);

    let relic_ids = relic_ids_for_simulated_subset(&run);

    assert!(relic_ids.contains(&"Neow's Lament".to_owned()));
}

#[test]
fn inline_relic_projection_uses_typed_start_then_core_state() {
    assert_eq!(
        seed_start_relic_ids_for_inline_projection(None),
        vec!["Burning Blood".to_owned()]
    );

    let mut run = RunState::map_fixture();
    run.relics = vec![Relic::BurningBlood, Relic::NeowsLament, Relic::Lantern];

    assert_eq!(
        seed_start_relic_ids_for_inline_projection(Some(&run)),
        vec![
            "Burning Blood".to_owned(),
            "Neow's Lament".to_owned(),
            "Lantern".to_owned(),
        ]
    );
}

#[test]
fn simulated_relic_projection_retains_spent_owned_neows_lament() {
    let mut run = RunState::map_fixture();
    run.relics.push(Relic::NeowsLament);
    run.neow_lament_combats_remaining = 0;

    let relic_ids = relic_ids_for_simulated_subset(&run);

    assert!(relic_ids.contains(&"Neow's Lament".to_owned()));
}

#[test]
fn combat_projection_uses_owned_spent_neows_lament() {
    let mut run = RunState::map_fixture();
    run.relics = vec![Relic::BurningBlood, Relic::NeowsLament];
    run.phase = RunPhase::Combat;
    run.neow_lament_combats_remaining = 0;
    run.combat = Some(
        run.init_combat(CombatState::initial_fixture())
            .expect("combat initializes"),
    );

    let subset = seed_start_simulated_combat_subset(&run, false);

    assert_eq!(
        subset["relic_ids"],
        json!(["Burning Blood", "Neow's Lament"])
    );
}

#[test]
fn simulated_relic_projection_uses_core_relic_order_before_new_pickups() {
    let mut run = RunState::map_fixture();
    run.relics = vec![Relic::BurningBlood, Relic::NeowsLament, Relic::Lantern];
    run.neow_lament_combats_remaining = 0;

    let relic_ids = relic_ids_for_simulated_subset(&run);

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
fn treasure_projection_uses_owned_spent_neows_lament() {
    let mut run = RunState::map_fixture();
    run.relics.push(Relic::NeowsLament);
    run.phase = RunPhase::Treasure;
    run.current_floor = 9;
    run.neow_lament_combats_remaining = 0;

    let subset = seed_start_treasure_simulated_subset(&run);
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
    run = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
        .expect("leave Neow after boss swap");
    let map_action = legal_map_decisions(&run)
        .expect("valid map decisions")
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
    let run =
        seed_start_apply_neow_boss_swap(-3_280_889_720_909_526_167, &ironclad_starter_deck_keys());

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

    assert_eq!(seed_start_grid_simulated_subset(&run)["choices"], json!([]));
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
    let simulated = seed_start_grid_simulated_subset(&run);

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
    run.gain_deck_card(sts_core::content::cards::PERFECTED_STRIKE_ID)
        .expect("Perfected Strike gain succeeds");

    let projected = seed_start_event_simulated_subset_with_delayed_deck_append(&run, Some(1));
    let deck = projected["deck_ids"].as_array().expect("projected deck");

    assert_eq!(deck.len(), visible_before);
    assert!(!deck.iter().any(|card| card == "Perfected Strike"));
}

#[test]
fn event_projection_defers_simulator_owned_pending_obtain_cards() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.phase = RunPhase::Event;
    run.event = Some(event_screen(Event::GoldenShrine));
    let deck_before = deck_content_keys(&run.deck);
    let run = sts_core::apply_event_action(&run, sts_core::EventAction::Choose { choice_index: 1 })
        .expect("Golden Shrine queues its deferred Regret");

    let transient_projection = seed_start_event_simulated_subset(&run);
    let settled_projection = deck_content_keys_after_pending_obtain_cards_settle(&run);

    assert_eq!(transient_projection["deck_ids"], json!(deck_before));
    let mut expected_deck = deck_before;
    expected_deck.push("Regret".to_owned());
    assert_eq!(settled_projection, expected_deck);

    let mut protected = RunState::seeded_ironclad(1, 0);
    protected
        .gain_relic_key(RelicKey::Omamori)
        .expect("Omamori pickup succeeds");
    protected.phase = RunPhase::Event;
    protected.event = Some(event_screen(Event::AccursedBlacksmith));
    let protected_deck = deck_content_keys(&protected.deck);
    let protected = sts_core::apply_event_action(
        &protected,
        sts_core::EventAction::Choose { choice_index: 1 },
    )
    .expect("Accursed Blacksmith queues its deferred Pain");
    assert_eq!(
        deck_content_keys_after_pending_obtain_cards_settle(&protected),
        protected_deck,
        "settled projection must apply core card-obtain prevention"
    );
    assert_eq!(
        protected.omamori_charges_used, 1,
        "Omamori is consumed when the pending obtain is queued; projection remains read-only"
    );

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

    let settled = ["Strike".to_owned(), "Regret".to_owned()];
    let transient = vec![vec!["Strike".to_owned()]];
    assert_eq!(
        classify_deferred_deck_reconciliation(&settled, &transient, &settled),
        PendingDeckObservation::Settled
    );
    assert_eq!(
        classify_deferred_deck_reconciliation(&transient[0], &transient, &settled),
        PendingDeckObservation::Deferred
    );
    assert!(matches!(
        classify_deferred_deck_reconciliation(
            &["Strike".to_owned(), "Regret".to_owned(), "Pain".to_owned()],
            &transient,
            &settled,
        ),
        PendingDeckObservation::Diverged(diffs) if !diffs.is_empty()
    ));

    let alternate_settled = ["Strike".to_owned(), "Pain".to_owned(), "Regret".to_owned()];
    assert_eq!(
        classify_deferred_deck_reconciliation_with_alternative(
            &alternate_settled,
            &transient,
            &settled,
            Some(&alternate_settled),
        ),
        PendingDeckObservation::Settled
    );
}

#[test]
fn seed_start_vampires_projection_delays_bites_until_leave() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.current_act = 2;
    run.current_floor = 31;
    run.phase = RunPhase::Event;
    run.event = Some(event_screen(Event::Vampires));
    let accepted = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
        .expect("Vampires accept applies");

    let projected = seed_start_event_simulated_subset_with_delayed_deck_append(
        &accepted,
        Some(VAMPIRES_BITE_COUNT),
    );
    let deck = projected["deck_ids"].as_array().expect("projected deck");

    assert!(!deck
        .iter()
        .any(|card| { card.as_str().is_some_and(|name| name.starts_with("Strike")) }));
    assert!(!deck.iter().any(|card| card == "Bite"));
}

#[test]
fn seed_start_vampires_projection_reconciles_settled_and_transient_frames() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.current_act = 2;
    run.current_floor = 31;
    run.phase = RunPhase::Event;
    run.event = Some(event_screen(Event::Vampires));
    let accepted = apply_event_action(&run, EventAction::Choose { choice_index: 0 })
        .expect("Vampires accept applies");
    let settled = seed_start_event_simulated_subset(&accepted);
    let transient = seed_start_event_simulated_subset_with_delayed_deck_append(
        &accepted,
        Some(VAMPIRES_BITE_COUNT),
    );

    assert_eq!(
        seed_start_event_simulated_subset_for_observation(
            &accepted,
            &settled,
            Some(VAMPIRES_BITE_COUNT),
        ),
        settled
    );
    assert_eq!(
        seed_start_event_simulated_subset_for_observation(
            &accepted,
            &transient,
            Some(VAMPIRES_BITE_COUNT),
        ),
        transient
    );
}

#[test]
fn seed_start_mind_bloom_healthy_projection_delays_doubt_and_darkstone_hp() {
    let mut run = RunState::seeded_ironclad(1, 0);
    run.current_floor = 41;
    run.phase = RunPhase::Event;
    run.relics.push(Relic::DarkstonePeriapt);
    run.event = Some(event_screen(Event::MindBloom));

    let accepted = apply_event_action(&run, EventAction::Choose { choice_index: 2 })
        .expect("Mind Bloom Healthy choice applies");
    assert_eq!((accepted.player_hp, accepted.player_max_hp), (86, 86));

    let settled = seed_start_event_simulated_subset(&accepted);
    let transient = seed_start_event_simulated_subset_with_delayed_deck_append_and_hp_gain(
        &accepted,
        1,
        Some(sts_core::relic::DARKSTONE_PERIAPT_MAX_HP),
    );
    assert_eq!(transient["current_hp"], 80);
    assert_eq!(transient["max_hp"], 80);
    assert!(!transient["deck_ids"]
        .as_array()
        .expect("projected deck")
        .iter()
        .any(|card| card == "Doubt"));
    assert_eq!(
        seed_start_event_simulated_subset_for_observation_with_delayed_hp_gain(
            &accepted,
            &transient,
            Some(1),
            Some(sts_core::relic::DARKSTONE_PERIAPT_MAX_HP),
        ),
        transient
    );
    assert_ne!(settled, transient);
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
        let content = serialize_trace_test_lines(lines);
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
        assert!(report
            .verified
            .iter()
            .any(|transition| { transition.action_step == 4 && transition.label == "Neow leave" }));

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

    run.gain_relic(Relic::Astrolabe)
        .expect("Astrolabe pickup succeeds");

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
    let visible_relic_rewards = |run: &RunState| {
        let reward = run.reward.as_ref().expect("Calling Bell reward screen");
        reward
            .relic_offer
            .iter()
            .chain(reward.pending_relic_offer.iter())
            .chain(reward.queued_relic_offers.iter())
            .map(|relic| {
                json!({
                    "reward_type": "RELIC",
                    "relic": {"name": relic_key_trace_name(relic.key())}
                })
            })
            .collect::<Vec<_>>()
    };
    let initial_relic_rewards = visible_relic_rewards(&after_confirm);
    let post_common_relic_rewards = visible_relic_rewards(&after_common);
    let post_uncommon_relic_rewards = visible_relic_rewards(&after_uncommon);
    let bell_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let common_relics: Vec<_> = relic_ids_for_simulated_subset(&after_common)
        .into_iter()
        .filter(|name| name != "Unknown Relic")
        .map(|name| json!({ "name": name }))
        .collect();
    let uncommon_relics: Vec<_> = relic_ids_for_simulated_subset(&after_uncommon)
        .into_iter()
        .filter(|name| name != "Unknown Relic")
        .map(|name| json!({ "name": name }))
        .collect();
    let rare_relics: Vec<_> = relic_ids_for_simulated_subset(&after_rare)
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
                "rewards": initial_relic_rewards
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
                "rewards": post_common_relic_rewards
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
                "rewards": post_uncommon_relic_rewards
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
    let content = serialize_trace_test_lines(lines);

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
    assert!(after_third.card_grid.is_none());
    let transformed_deck: Vec<_> = deck_content_keys(&after_third.deck)
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let grid_choices: Vec<_> = seed_start_grid_simulated_subset(&astrolabe_run)["choices"]
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow boss swap Astrolabe grid"
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 6 && transition.label == "Neow boss swap Astrolabe transformed"
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
    let grid_choices: Vec<_> = seed_start_grid_simulated_subset(&pandora_run)["choices"]
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow boss swap Pandora's Box grid"
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 4 && transition.label == "Neow boss swap Pandora's Box confirm"
    }));
}

#[test]
fn seed_start_boss_swap_pandoras_box_grid_matches_live_trace_pool_order() {
    let run =
        seed_start_apply_neow_boss_swap(7_003_943_589_014_798_927, &ironclad_starter_deck_keys());
    let grid = run.card_grid.as_ref().expect("Pandora's Box grid");
    let subset = seed_start_grid_simulated_subset(&run);
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
    let after_second_select =
        select_grid_card(&after_first_select, 1).expect("second selection resolves Empty Cage");
    let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&empty_cage_run)["choices"]
        .as_array()
        .expect("first grid choices")
        .clone();
    let second_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&after_first_select)
        ["choices"]
        .as_array()
        .expect("second grid choices")
        .clone();
    let two_removed_deck: Vec<_> = deck_content_keys(&after_second_select.deck)
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();

    assert_eq!(empty_cage_run.deck.len(), 10);
    assert_eq!(after_first_select.deck.len(), 10);
    assert_eq!(after_second_select.deck.len(), 8);

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
            "choice_list": second_grid_choices
        }}}),
        json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
        json!({"type": "state", "step": 5, "message": {"game_state": {
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow boss swap Empty Cage grid"
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 5 && transition.label == "Neow boss swap Empty Cage confirm"
    }));
}

#[test]
fn seed_start_simple_neow_reward_uses_core_helper() {
    let option =
        seed_start_selected_neow_option(40_560_393_133, "CHOOSE 1").expect("M290008 slot 1 option");

    assert_eq!(option.reward, NeowRewardType::HundredGold);
    assert_eq!(
        seed_start_apply_neow_simple_option(option),
        Some((199, 80, 80))
    );
}

#[test]
fn seed_start_simple_neow_drawback_and_reward_use_core_helpers() {
    let option =
        seed_start_selected_neow_option(40_560_393_133, "CHOOSE 2").expect("M290008 slot 2 option");

    assert_eq!(option.drawback, NeowDrawback::NoGold);
    assert_eq!(option.reward, NeowRewardType::TwentyPercentHpBonus);
    assert_eq!(
        seed_start_apply_neow_simple_option(option),
        Some((0, 96, 96))
    );
}

#[test]
fn seed_start_simple_neow_helper_rejects_identity_branches() {
    let option =
        seed_start_selected_neow_option(40_560_393_133, "CHOOSE 0").expect("M290008 slot 0 option");

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
    let option =
        seed_start_selected_neow_option(40_560_393_126, "CHOOSE 2").expect("M290001 slot 2 option");

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
    let content = serialize_trace_test_lines(lines);

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
        seed_start_grid_simulated_subset(&run),
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
    assert!(!seed_start_neow_grid_auto_confirms_after_choose(&run));
    assert_eq!(
        seed_start_grid_simulated_subset(&run),
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
            "Strike_R+",
            "Strike_R",
            "Strike_R",
            "Strike_R",
            "Strike_R",
            "Defend_R",
            "Defend_R",
            "Defend_R",
            "Defend_R",
            "Bash",
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
    assert!(!seed_start_neow_grid_auto_confirms_after_choose(&run));
    assert_eq!(run.deck.len(), 10);
    assert_eq!(
        seed_start_grid_simulated_subset(&run)["choices"]
            .as_array()
            .expect("choices")
            .len(),
        10
    );

    run = select_grid_card(&run, 1).expect("select second strike");
    assert!(seed_start_neow_grid_auto_confirms_after_choose(&run));
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
    let initial_run =
        seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let after_first_select = select_grid_card(&initial_run, 0).expect("select first");
    let after_second_select = select_grid_card(&after_first_select, 1).expect("select second");
    let after_second_confirm = confirm_grid(&after_second_select).expect("remove second");
    let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run)["choices"]
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
    let content = serialize_trace_test_lines(lines);

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
    let initial_run =
        seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let after_select = select_grid_card(&initial_run, 0).expect("select first");
    let after_confirm = confirm_grid(&after_select).expect("confirm upgrade");
    let grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run)["choices"]
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
    let content = serialize_trace_test_lines(lines);

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
fn seed_start_neow_curse_transform_two_rejects_forged_map_after_second_pick() {
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
    let initial_run =
        seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let after_first_select = select_grid_card(&initial_run, 0).expect("select first");
    let after_second_select =
        select_grid_card(&after_first_select, 1).expect("select second and transform");
    let after_confirm = confirm_grid(&after_second_select).expect("confirm transform two");
    let curse_key = seed_start_neow_curse_deck_key(numeric_seed, 0).expect("generated curse key");
    let mut grid_deck_ids = deck_content_keys(&initial_run.deck);
    grid_deck_ids.push(curse_key.clone());
    let grid_deck: Vec<_> = grid_deck_ids
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
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
    let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run)["choices"]
        .as_array()
        .expect("first grid choices")
        .clone();
    let second_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&after_first_select)
        ["choices"]
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow transform two grid"
    }));
    assert_eq!(initial_run.card_rng_counter, 0);
    let diff = report
        .unexpected_diffs
        .iter()
        .find(|diff| diff.action_step == 5 && diff.label == "Neow grid confirm")
        .expect("forged map destination must differ");
    assert!(
        diff.diffs
            .iter()
            .any(|line| line.starts_with("screen_type:")),
        "{diff:#?}"
    );
    let simulated = report
        .seed_start
        .as_ref()
        .and_then(|seed_start| seed_start.sim_run_state.as_ref())
        .expect("simulated run state remains available after the observed mismatch");
    assert_eq!(simulated.phase, RunPhase::Event);
    assert!(simulated.card_grid.is_none());
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
    let option =
        seed_start_selected_neow_option(numeric_seed, "CHOOSE 0").expect("CODEX04 slot 0 option");

    assert_eq!(option.reward, NeowRewardType::RandomColorless);

    let mut run =
        seed_start_apply_neow_reward_drawback(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
    assert_eq!(reward_ids[1], deck_content_key(DRAMATIC_ENTRANCE_ID));

    assert_eq!(run.event_rng_counter, 0);
    run.card_rng_counter =
        seed_start_neow_card_reward_card_rng_counter(numeric_seed, &option, Some(&run))
            .expect("colorless reward consumes cardRng");
    let mut deck_ids = deck_content_keys(&run.deck);
    deck_ids.push(reward_ids[1].clone());
    run.deck = deck_instances_from_keys(&deck_ids);
    let mut combat = CombatState::initial_fixture();
    combat.phase = CombatPhase::Won;
    for monster in &mut combat.monsters {
        monster.hp = 0;
        monster.alive = false;
    }
    run.phase = RunPhase::Combat;
    run.current_room_override = Some(RoomKind::Combat);
    run.event = None;
    run.combat = Some(combat);

    enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");

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
    let run =
        seed_start_apply_neow_reward_drawback(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let neow_deck: Vec<_> = deck_content_keys(&run.deck)
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
    let reward_names = seed_start_neow_card_reward_choice_names(numeric_seed, &option, Some(&run));
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
    let content = serialize_trace_test_lines(lines);

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
fn seed_start_neow_curse_three_rare_cards_settles_curse_before_pick() {
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
    let run =
        seed_start_apply_neow_reward_drawback(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let neow_deck: Vec<_> = deck_content_keys(&run.deck)
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
    let reward_names = seed_start_neow_card_reward_choice_names(numeric_seed, &option, Some(&run));
    let reward_cards: Vec<_> = reward_ids
        .iter()
        .map(|id| json!({ "id": id, "name": id }))
        .collect();
    let curse_key =
        seed_start_neow_curse_deck_key(numeric_seed, 0).expect("curse generated after leave");

    let mut reward_deck = neow_deck.clone();
    reward_deck.push(json!({ "id": curse_key }));
    let picked_card = reward_ids[0].clone();
    let mut leave_deck = reward_deck.clone();
    leave_deck.push(json!({ "id": picked_card }));
    let map_deck = leave_deck.clone();

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
            "deck": reward_deck,
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(
        |transition| transition.action_step == 4 && transition.label == "Neow colorless pickup"
    ));
    assert!(report
        .verified
        .iter()
        .any(|transition| transition.action_step == 5 && transition.label == "Neow leave"));
}

#[test]
fn seed_start_neow_immediate_random_rare_leave_defers_when_visible_deck_lags() {
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
    let run =
        seed_start_apply_neow_reward_drawback(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let mut settled_deck = starting_deck.clone();
    settled_deck.push(json!({ "id": deck_content_key(BARRICADE_ID) }));

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
        json!({"type": "action", "step": 5, "command": "state"}),
        json!({"type": "state", "step": 5, "message": {"game_state": {
            "screen_type": "MAP",
            "ascension_level": 0,
            "floor": 0,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck": settled_deck,
            "relics": starting_relics,
            "choice_list": seed_start_first_map_choices(&external_seed)
        }}}),
    ];
    let mut divergent_lines = lines.clone();
    divergent_lines
        .iter_mut()
        .find_map(|line| {
            (line.get("step").and_then(Value::as_u64) == Some(5))
                .then(|| line.pointer_mut("/message/game_state/deck"))
                .flatten()
        })
        .and_then(Value::as_array_mut)
        .expect("Neow leave deck")
        .pop()
        .expect("settled deck has a rare reward");
    divergent_lines
        .iter_mut()
        .find_map(|line| {
            (line.get("step").and_then(Value::as_u64) == Some(5))
                .then(|| line.pointer_mut("/message/game_state/deck"))
                .flatten()
        })
        .and_then(Value::as_array_mut)
        .expect("Neow leave deck")
        .pop()
        .expect("starter deck has a final card");
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow random rare card reward"
    }));
    let reward = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 3)
        .expect("Neow random rare disposition");
    assert!(reward.deferred_assertion_reconciled);
    assert!(report
        .verified
        .iter()
        .any(|transition| transition.action_step == 4 && transition.label == "Neow leave"));
    let leave = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 4 && entry.command == "CHOOSE 0")
        .expect("Neow leave disposition");
    assert_eq!(leave.disposition, ActionDispositionKind::Verified);
    assert!(leave.deferred_assertion_reconciled);
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        0
    );
    let carried = report
        .seed_start
        .as_ref()
        .and_then(|seed_start| seed_start.sim_run_state.as_ref())
        .expect("seed-start carries simulator state after Neow leave");
    assert!(deck_content_keys(&carried.deck)
        .iter()
        .any(|key| key == deck_content_key(BARRICADE_ID)));

    let divergent_content = serialize_trace_test_lines(divergent_lines);
    let divergent = verify_seed_start_communication_mod_trace(&divergent_content)
        .expect("divergent Neow leave trace parses");
    assert!(divergent
        .unexpected_diffs
        .iter()
        .any(|diff| { diff.action_step == 4 && diff.label == "Neow leave" }));
    let divergent_leave = divergent
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 4 && entry.command == "CHOOSE 0")
        .expect("divergent Neow leave disposition");
    assert_eq!(
        divergent_leave.disposition,
        ActionDispositionKind::UnexpectedDiff
    );
    let divergent_integrity = divergent
        .action_integrity
        .expect("divergent action integrity");
    assert_eq!(
        divergent_integrity.applicable_actions,
        divergent_integrity.disposed_actions
    );
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

    let generated = generate_neow_colorless_reward(numeric_seed, option.reward)
        .expect("matched generated Neow colorless reward option");
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
    let generated = generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorless)
        .expect("RandomColorless is a colorless Neow reward");
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
fn seed_start_neow_curse_rare_colorless_reconciles_chained_transient_decks() {
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

    let run =
        seed_start_apply_neow_reward_drawback(numeric_seed, &ironclad_starter_deck_keys(), &option);
    let generated = generate_neow_colorless_reward(numeric_seed, option.reward)
        .expect("matched generated Neow colorless reward option");
    let choices = seed_start_neow_card_reward_content_ids(numeric_seed, &option, Some(&run));
    let curse = seed_start_neow_curse_deck_key(numeric_seed, generated.card_rng_counter)
        .expect("Neow curse");

    assert_eq!(run.card_rng_counter, 0);
    assert_eq!(run.deck.len(), ironclad_starter_deck_keys().len());
    assert_eq!(choices, generated.cards);
    assert!(content_id_from_key(&curse).is_some_and(sts_core::content::cards::is_curse_content_id));

    let external_seed = test_seed_string_from_long(numeric_seed);
    let starting_deck_keys = ironclad_starter_deck_keys();
    let starting_deck = starting_deck_keys
        .iter()
        .map(|id| json!({ "id": id }))
        .collect::<Vec<_>>();
    let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
    let reward_names = seed_start_neow_card_reward_choice_names(numeric_seed, &option, Some(&run));
    let reward_cards = reward_ids
        .iter()
        .map(|id| json!({ "id": id, "name": id }))
        .collect::<Vec<_>>();
    let picked_card = reward_ids[0].clone();
    let mut transient_pick_deck = starting_deck_keys.clone();
    transient_pick_deck.push(picked_card);
    let mut settled_deck = transient_pick_deck.clone();
    settled_deck.push(curse);
    let trace_deck = |keys: &[String]| {
        keys.iter()
            .map(|id| json!({ "id": id }))
            .collect::<Vec<_>>()
    };
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
            "deck": trace_deck(&starting_deck_keys),
            "relics": [{"name": "Burning Blood"}],
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
            "deck": trace_deck(&starting_deck_keys),
            "relics": [{"name": "Burning Blood"}],
            "choice_list": reward_names,
            "screen_state": {"cards": reward_cards}
        }}}),
        json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
        json!({"type": "state", "step": 4, "message": {"game_state": {
            "screen_type": "EVENT",
            "ascension_level": 0,
            "floor": 0,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck": trace_deck(&transient_pick_deck),
            "relics": [{"name": "Burning Blood"}],
            "choice_list": ["leave"]
        }}}),
        json!({"type": "action", "step": 5, "command": "STATE"}),
        json!({"type": "state", "step": 5, "message": {"game_state": {
            "screen_type": "EVENT",
            "ascension_level": 0,
            "floor": 0,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck": trace_deck(&settled_deck),
            "relics": [{"name": "Burning Blood"}],
            "choice_list": ["leave"]
        }}}),
        json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
        json!({"type": "state", "step": 6, "message": {"game_state": {
            "screen_type": "MAP",
            "ascension_level": 0,
            "floor": 0,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck": trace_deck(&settled_deck),
            "relics": [{"name": "Burning Blood"}],
            "choice_list": seed_start_first_map_choices(&external_seed)
        }}}),
    ];
    let report = verify_seed_start_communication_mod_trace(&serialize_trace_test_lines(lines))
        .expect("settled curse trace verifies");
    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    let pick = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 4)
        .expect("pick disposition");
    assert_eq!(pick.disposition, ActionDispositionKind::Verified);
    assert!(pick.deferred_assertion_reconciled);
    let reward_open = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 3)
        .expect("reward-open disposition");
    assert_eq!(reward_open.disposition, ActionDispositionKind::Verified);
    assert!(reward_open.deferred_assertion_reconciled);
    let state = report
        .seed_start
        .as_ref()
        .and_then(|seed_start| seed_start.sim_run_state.as_ref())
        .expect("simulator state");
    assert_eq!(deck_content_keys(&state.deck), settled_deck);
    assert_eq!(state.card_rng_counter, generated.card_rng_counter + 1);
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
    let option =
        seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 3").expect("boss swap option");

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
    open_neow_reward_grid(&mut run, NeowRewardType::RemoveCard)
        .expect("RemoveCard opens a Neow grid");

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
    let swapped_run = seed_start_apply_neow_boss_swap(numeric_seed, &ironclad_starter_deck_keys());
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report
        .verified
        .iter()
        .any(|transition| { transition.action_step == 3 && transition.label == "Neow boss swap" }));
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
    let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");
    assert_eq!(option.reward, NeowRewardType::BossRelic);

    let starting_deck: Vec<_> = ironclad_starter_deck_keys()
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let tiny_house_deck = observed_deck_cards(&tiny_house_run.deck);
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
        .map(|card| reward_card_display_key(&card_reward_run, card.content_id).to_ascii_lowercase())
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow boss swap Tiny House reward"
    }));
    assert!(report
        .verified
        .iter()
        .any(|transition| { transition.action_step == 4 && transition.label == "card reward" }));
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
    let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");
    assert_eq!(option.reward, NeowRewardType::BossRelic);

    let starting_deck: Vec<_> = ironclad_starter_deck_keys()
        .into_iter()
        .map(|id| json!({ "id": id }))
        .collect();
    let tiny_house_deck = observed_deck_cards(&tiny_house_run.deck);
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
        .map(|card| reward_card_display_key(&card_reward_run, card.content_id).to_ascii_lowercase())
        .collect();
    let selected_card_key = reward_card_display_key(&card_reward_run, reward.choices[1].content_id);
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

    let truncated_content = serialize_trace_test_lines(lines.clone());
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
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow boss swap Tiny House reward"
    }));
    assert!(report
        .verified
        .iter()
        .any(|transition| { transition.action_step == 4 && transition.label == "card reward" }));
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
    let potion_names = seed_start_neow_potion_names(22_079_335_079);
    let potion_rewards: Vec<_> = potion_names
        .iter()
        .map(|name| {
            json!({
                "reward_type": "POTION",
                "potion": { "name": name },
            })
        })
        .collect();
    let mut lines = vec![
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
            "screen_type": "COMBAT_REWARD",
            "ascension_level": 0,
            "floor": 0,
            "gold": 99,
            "current_hp": 80,
            "max_hp": 80,
            "deck": deck,
            "relics": relics,
            "potions": [],
            "choice_list": ["potion", "potion", "potion"],
            "screen_state": { "rewards": potion_rewards }
        }}}),
    ];
    let opening_lines = lines.clone();
    for pick in 1..=3 {
        let owned = potion_names
            .iter()
            .take(pick)
            .map(|name| json!({ "name": name }))
            .collect::<Vec<_>>();
        let remaining = potion_names
            .iter()
            .skip(pick)
            .map(|name| {
                json!({
                    "reward_type": "POTION",
                    "potion": { "name": name },
                })
            })
            .collect::<Vec<_>>();
        let choices = vec!["potion"; 3 - pick];
        let step = 3 + pick as u32;
        lines.push(json!({"type": "action", "step": step, "command": "CHOOSE 0"}));
        lines.push(
            json!({"type": "state", "step": step, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "potions": owned,
                "choice_list": choices,
                "screen_state": { "rewards": remaining }
            }}}),
        );
    }
    let content = serialize_trace_test_lines(lines);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 3 && transition.label == "Neow three potion reward"
    }));
    for pick in 1..=3 {
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 + pick
                && transition.label == format!("Neow potion reward pick {pick}")
        }));
    }
    assert_eq!(
        report
            .seed_start
            .expect("seed-start")
            .first_boundary
            .category,
        "none"
    );

    let mut divergent_lines = opening_lines;
    divergent_lines.pop();
    divergent_lines.push(
        json!({"type": "state", "step": 3, "message": {"game_state": {
            "screen_type": "EVENT",
            "ascension_level": 0,
            "floor": 0,
            "gold": 99,
            "current_hp": 80,
            "max_hp": 80,
            "deck": deck,
            "relics": relics,
            "potions": [],
            "choice_list": ["leave"]
        }}}),
    );
    let divergent =
        verify_seed_start_communication_mod_trace(&serialize_trace_test_lines(divergent_lines))
            .expect("observation-independent divergent report");
    assert!(divergent
        .unexpected_diffs
        .iter()
        .any(|diff| { diff.action_step == 3 && diff.label == "Neow three potion reward" }));
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
    let mut card_random_rng = StsRng::new(0);
    let simulated =
        initialize_combat_piles_with_relics(&deck, &mut shuffle_rng, &mut card_random_rng, &[])
            .expect("trace deck contains known card content");

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

#[test]
fn random_fidelity_gremlin_leader_rally_preserves_monster_group_order() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-e5f5126b26961e8a.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Gremlin Leader rally ordering trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 610)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_havoc_burning_pact_defers_selected_card_settlement() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-6bb06bc1b46cc683.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Havoc top-draw Burning Pact trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 52)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action-integrity report")
            .unresolved_transient_assertions,
        0
    );
}

#[test]
fn random_fidelity_thunderclap_discard_ordering_after_burning_pact() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-47d58a2e70711da8.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Thunderclap discard-ordering trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 205)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_burning_pact_small_hand_transient_is_source_derived() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-1798668c9838293e.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("small-hand Burning Pact trace replays");

    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 123)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the deferred selection frame must be accepted from the typed core state: {report:#?}"
    );
    assert_ne!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .path,
        "$.actions[step=123].command",
        "the old Burning Pact boundary must be gone: {report:#?}"
    );
}

#[test]
fn random_fidelity_burning_pact_draw_order_corpus_replays_past_boundary() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-92e742c9f2c8470c.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("broad-corpus Burning Pact trace replays");

    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 175)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the draw/hand ordering boundary must be gone: {report:#?}"
    );
}

#[test]
fn random_fidelity_burning_pact_hidden_selection_preserves_end_turn_draw_order() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-9667b7fd8ff939a8.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("hidden-selection Burning Pact trace replays");

    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 181)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the END after the hidden Burning Pact selection must verify: {report:#?}"
    );
    assert_ne!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .path,
        "$.actions[step=181].command",
        "the hidden selected card must not be reinserted before the end-turn draw: {report:#?}"
    );
}

#[test]
fn random_fidelity_burning_pact_normal_hand_selection_can_settle_deferred() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-bb2cf06ce5dff840.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("normal-hand Burning Pact settlement trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 749)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
    assert_eq!(
        report
            .verified
            .iter()
            .find(|entry| entry.action_step == 749)
            .map(|entry| entry.label.as_str()),
        Some("Burning Pact deferred selection transient")
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 751)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_gambling_chip_lag_frame_advances_without_unceasing_top_draw() {
    // GC CONFIRM lag removes selected cards from hand only; advancing the fully
    // settled sim would Unceasing-Top draw Berserk/Shrug before PLAY Sentinel.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-d3b52f426b3aff94.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Gambling Chip lag permanent trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "Sentinel after GC lag must verify: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 804)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 805)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_runic_cube_lethal_end_turn_keeps_draw_pile() {
    // Runic Cube must not draw on lethal Lagavulin hit (bot Draw cancelled).
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-a7f662aa8ed22115.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Runic Cube lethal permanent trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "step 2896 lethal END must leave Bash+ in draw: {report:#?}"
    );
}

#[test]
fn random_fidelity_burning_pact_dark_embrace_draws_after_discard() {
    // DE×2 after Burning Pact exhaust: BP draws first, source discards, then DE
    // reshuffles (can pull BP back into hand). Permanent minimized prefix ends
    // at step 459 CONFIRM.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-9bf0204173fb2a7f.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Burning Pact + Dark Embrace permanent trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "step 459 CONFIRM must match DE-after-discard hand: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 459)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "Burning Pact exhaust-select CONFIRM must verify: {report:#?}"
    );
}

#[test]
fn random_fidelity_havoc_headbutt_returns_source_to_draw() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-f99c08d43d7c329e.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Havoc/Headbutt permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 703)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "Havoc must settle before the forced top-card Headbutt resolves: {report:#?}"
    );
}

#[test]
fn random_fidelity_headbutt_discard_select_accepts_put_on_draw_source_lag() {
    // CHOOSE on DiscardPileToTopOfDeck can publish a stable frame where Headbutt
    // has settled but the chosen discard card is still in discard. Sim applies
    // put-on-draw atomically; reconcile the lag without hydrating.
    for (name, step) in [
        (
            "permanent_traces/random-fidelity-1f14d6b99fbf0dc4.jsonl",
            375,
        ),
        (
            "permanent_traces/random-fidelity-8bbdfdd20e40ab31.jsonl",
            315,
        ),
        (
            "permanent_traces/random-fidelity-c706d6d8d55b13fe.jsonl",
            807,
        ),
    ] {
        let Some(content) = crate::load_corpus_file(name) else {
            continue;
        };
        let report = verify_seed_start_communication_mod_trace(&content)
            .unwrap_or_else(|error| panic!("{name} should replay: {error}"));
        assert!(
            report.unexpected_diffs.is_empty(),
            "{name} unexpected diffs: {report:#?}"
        );
        assert!(
            !report
                .unsupported
                .iter()
                .any(|entry| entry.action_step == step),
            "{name} step {step} must not be unsupported: {report:#?}"
        );
        assert_eq!(
            report
                .action_dispositions
                .iter()
                .find(|entry| entry.action_step == step)
                .map(|entry| entry.disposition),
            Some(ActionDispositionKind::Verified),
            "{name} Headbutt discard-select CHOOSE must verify past put-on-draw lag: {report:#?}"
        );
    }
}

#[test]
fn random_fidelity_headbutt_put_on_draw_permanent_omit_before_end_draw() {
    // de6148c1: Havoc→Headbutt CHOOSE 1 accepts put-on-draw lag, but real never
    // moves Havoc out of discard. Reverse the settled put-on-draw before END so
    // the next hand is not poisoned (Ghostly Armor block/hand mismatch at 379).
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-de6148c1d6dafaef.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("de6148c1 permanent omit Headbutt put-on-draw replays");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {report:#?}"
    );
    assert!(
        !report
            .unsupported
            .iter()
            .any(|entry| entry.action_step == 379),
        "step 379 must not be unsupported: {report:#?}"
    );
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "de6148c1 must clear to category=none: {report:#?}"
    );
}

#[test]
fn random_fidelity_havoc_empty_draw_shuffles_without_source() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-9c74b1b3157af014.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("empty-draw Havoc permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none",
        "the empty-draw Havoc boundary must be gone: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 5149)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_burning_pact_exhausts_before_crossing_deck_boundary() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-b9c0db157d03167f.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Burning Pact discard-ordering trace replays");

    let boundary = &report
        .seed_start
        .as_ref()
        .expect("seed-start report")
        .first_boundary;
    assert_ne!(
        boundary.path, "$.actions[step=335].command",
        "the old discard-ordering boundary must be gone: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 335)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "step 335 must verify after Burning Pact exhausts its selection"
    );
}

#[test]
fn random_fidelity_burning_pact_keeps_visible_normal_selection_in_exhaust() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-44f7dfd426e439c6.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("normal-hand Burning Pact trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 225)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_burning_pact_end_turn_draw_ordering_after_deferred_selection() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-2e1bd52d86404d3b.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("deferred Burning Pact end-turn trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    let seed_start = report.seed_start.as_ref().expect("seed-start report");
    assert!(!seed_start.failed, "{report:#?}");
    assert_eq!(seed_start.first_boundary.category, "none", "{report:#?}");
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 301)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the deferred selected card must not re-enter the end-turn shuffle: {report:#?}"
    );
}

#[test]
fn random_fidelity_burning_pact_empty_hand_end_holds_deferred_selected_card() {
    // Deferred ExhaustAction retrieval parks Thunderclap outside every pile.
    // Spending the rest of the hand then END must not inject that card into
    // the empty-hand discard→draw shuffle (hand_ids Burning Pact/Strike order
    // and no Thunderclap at hand[3]).
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-c60c2349aa8da68d.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("empty-hand deferred Burning Pact permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(
        report
            .unsupported
            .iter()
            .all(|entry| entry.action_step > 237),
        "step 237 END must not fail with deferred Thunderclap in hand: {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 237)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "empty-hand END after deferred Burning Pact must verify: {report:#?}"
    );
}

#[test]
fn random_fidelity_champ_taunt_preserves_frail_for_following_defend() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-520f091a7b46a976.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Champ Taunt permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 574)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified)
    );
}

#[test]
fn random_fidelity_collector_rolls_fireball_after_opening_summon() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-2273d66230c5560b.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Collector intent permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 906)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the Collector must select Fireball after its opening summon: {report:#?}"
    );
}

#[test]
fn random_fidelity_lethal_monster_hit_cancels_following_thorns() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-86059d43fea814c1.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("lethal end-turn Thorns permanent trace replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 1087)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "the lethal Centurion hit must prevent Mystic's queued Thorns damage"
    );
}

#[test]
fn random_fidelity_legacy_trace_uses_profile_note_card_for_event_grid() {
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-f6b5080af3ce19fd.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("legacy Note For Yourself trace verifies");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
}

#[test]
fn random_fidelity_iron_wave_juggernaut_kills_before_malleable_reward() {
    // Iron Wave GainBlock queues Juggernaut (addToBot) before Iron Wave damage
    // queues Malleable (addToBot). Juggernaut's thorns must land before Malleable
    // block or Snake Plant survives at 4 HP instead of opening COMBAT_REWARD.
    // Permanent tip: random-fidelity-1ac7db2c9f4a3da9 step 670.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-1ac7db2c9f4a3da9.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Iron Wave Juggernaut-before-Malleable permanent tip replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 670)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "step 670 Iron Wave must end combat (Juggernaut before Malleable): {report:#?}"
    );
}

#[test]
fn random_fidelity_reckless_charge_evolve_shrug_draw_order() {
    // Reckless Charge puts Dazed on empty draw; Shrug It Off draws it under Evolve.
    // EvolvePower.onCardDraw must addToBot after UseCardAction discards Shrug so
    // the status-triggered reshuffle includes the played card. Inline limbo
    // evolve left Shrug alone in discard → wrong piles at step 648 PLAY.
    // Permanent tip: random-fidelity-0667712a2814e2cf steps 645–648.
    let Some(content) =
        crate::load_corpus_file("permanent_traces/random-fidelity-0667712a2814e2cf.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Reckless Charge / Evolve / Shrug permanent tip replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert_eq!(
        report
            .seed_start
            .as_ref()
            .expect("seed-start report")
            .first_boundary
            .category,
        "none"
    );
    assert!(
        report.verified.iter().any(|transition| {
            transition.action_step == 645 && transition.label == "Reckless Charge"
        }),
        "step 645 Reckless Charge must hard-verify: {report:#?}"
    );
    assert!(
        report.verified.iter().any(|transition| {
            transition.action_step == 646 && transition.label.contains("Shrug It Off")
        }),
        "step 646 Shrug It Off+ must hard-verify (not pile settlement only): {report:#?}"
    );
    assert_eq!(
        report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == 648)
            .map(|entry| entry.disposition),
        Some(ActionDispositionKind::Verified),
        "step 648 PLAY after Evolve reshuffle must verify: {report:#?}"
    );
}
