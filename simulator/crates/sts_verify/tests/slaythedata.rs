use sts_verify::{
    import_slaythedata_jsonl_line, import_slaythedata_run_json, slaythedata_replay_plan,
    slaythedata_replay_preflight, SlayTheDataBridgeDescriptor, SlayTheDataDiagnosticSeverity,
    SlayTheDataPreflightStatus, SlayTheDataReplayOrdering, SlayTheDataReplayStepKind,
    SlayTheDataSourceKind,
};

fn generated_neow_three_cards_fixture() -> (String, &'static str, &'static str) {
    for index in 0..500 {
        let seed = format!("CARD{index:03}");
        if let Some((picked, cost)) = generated_neow_three_cards_first_pick(&seed) {
            return (seed, picked, cost);
        }
    }
    panic!("test fixture search did not find a seed with Three Cards Neow option");
}

fn generated_neow_three_cards_first_pick(seed: &str) -> Option<(&'static str, &'static str)> {
    let run = sts_core::RunState::placeholder_seeded_ironclad(
        sts_verify::sts_seed_string_to_long(seed) as u64,
        0,
    );
    let run = sts_core::apply_event_action(&run, sts_core::EventAction::Choose { choice_index: 0 })
        .expect("Neow talk applies");
    let option = sts_core::generate_neow_options(run.event_rng_seed as i64, run.player_max_hp)
        .into_iter()
        .find(|option| option.reward == sts_core::NeowRewardType::ThreeCards)?;
    let cost = match option.drawback {
        sts_core::NeowDrawback::None => "NONE",
        sts_core::NeowDrawback::Curse => "CURSE",
        sts_core::NeowDrawback::NoGold => "NO_GOLD",
        sts_core::NeowDrawback::TenPercentHpLoss => "TEN_PERCENT_HP_LOSS",
        sts_core::NeowDrawback::PercentDamage => "PERCENT_DAMAGE",
    };
    let run = sts_core::apply_event_action(
        &run,
        sts_core::EventAction::Choose {
            choice_index: option.slot,
        },
    )
    .expect("Neow option applies");
    let first = run.reward.as_ref().expect("reward screen").choices[0].content_id;
    let name = sts_core::content::cards::get_card_definition(first)
        .expect("generated card has a definition")
        .name;
    Some((name, cost))
}

#[test]
fn imports_chunk_export_row_into_typed_run_contract() {
    let content = r#"{
        "run_id": 123,
        "source_file": "IRONCLAD/runs.json",
        "source_run_ordinal": 7,
        "event": {
            "play_id": "play-1",
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "ABC123",
            "build_version": "2022-12-18",
            "neow_bonus": "TEN_PERCENT_HP_BONUS",
            "neow_cost": "NONE",
            "path_taken": ["M", "?"],
            "path_per_floor": ["M", "?"],
            "card_choices": [
                {"floor": 1, "picked": "Inflame+", "not_picked": ["Clash", "Flex+"]}
            ],
            "relics_obtained": [{"floor": 1, "key": "Oddly Smooth Stone"}],
            "event_choices": [
                {
                    "floor": 2,
                    "event_name": "World of Goop",
                    "player_choice": "Gather Gold",
                    "gold_gain": 75,
                    "cards_removed": ["Strike"]
                }
            ],
            "items_purchased": ["Shrug It Off"],
            "item_purchase_floors": [3],
            "campfire_choices": [{"floor": 4, "key": "SMITH", "data": "Bash+"}],
            "potions_floor_usage": [1, 3, 3],
            "potions_obtained": [{"floor": 2, "key": "Fire Potion"}],
            "boss_relics": [{"picked": "Black Blood", "not_picked": ["Snecko Eye"]}],
            "master_deck": ["Bash+", "Inflame+"],
            "relics": ["Burning Blood"],
            "gold": 99,
            "floor_reached": 4,
            "victory": false
        }
    }"#;

    let imported = import_slaythedata_run_json(content).expect("imports");

    assert_eq!(imported.source.kind, SlayTheDataSourceKind::ChunkExport);
    assert_eq!(imported.source.run_id, Some(123));
    assert_eq!(imported.config.character.as_deref(), Some("IRONCLAD"));
    assert_eq!(imported.config.ascension, Some(0));
    assert_eq!(imported.config.seed_played.as_deref(), Some("ABC123"));
    assert!(!imported.replay_policy.exact_combat_actions);
    assert_eq!(imported.route.path_per_floor, ["M", "?"]);

    let floor_1 = imported
        .floor_decisions
        .iter()
        .find(|floor| floor.floor == 1)
        .expect("floor 1");
    assert_eq!(floor_1.route.as_deref(), Some("M"));
    assert_eq!(
        floor_1.card_rewards[0].picked.as_ref().unwrap().base,
        "Inflame"
    );
    assert!(floor_1.card_rewards[0].picked.as_ref().unwrap().upgraded);
    assert_eq!(floor_1.card_rewards[0].not_picked[1].base, "Flex");
    assert_eq!(floor_1.relics_obtained[0].key, "Oddly Smooth Stone");
    assert_eq!(floor_1.potions.uses_allowed, 1);

    let floor_2 = imported
        .floor_decisions
        .iter()
        .find(|floor| floor.floor == 2)
        .expect("floor 2");
    assert_eq!(
        floor_2.events[0].event_name.as_deref(),
        Some("World of Goop")
    );
    assert_eq!(floor_2.events[0].cards_removed[0].base, "Strike");
    assert_eq!(floor_2.potions.obtained[0].key, "Fire Potion");

    let floor_3 = imported
        .floor_decisions
        .iter()
        .find(|floor| floor.floor == 3)
        .expect("floor 3");
    assert_eq!(floor_3.shop_purchases[0].base_item, "Shrug It Off");
    assert_eq!(floor_3.potions.uses_allowed, 2);

    assert_eq!(imported.boss_relic_choices[0].act, 1);
    assert_eq!(imported.final_observed.master_deck[0].base, "Bash");
    assert!(imported
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "exact_combat_actions_unavailable"));
    assert!(imported
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "floor_only_potion_budget"));
}

#[test]
fn imports_raw_run_and_reports_reconstruction_blockers() {
    let content = r#"{
        "character_chosen": "THE_SILENT",
        "ascension_level": 21,
        "event_choices": [
            {"floor": 3, "player_choice": "Remove", "cards_removed": ["Strike"] }
        ],
        "campfire_choices": [
            {"floor": 3, "key": "SMITH", "data": "Bash+"}
        ]
    }"#;

    let imported = import_slaythedata_run_json(content).expect("imports");
    let diagnostics: Vec<_> = imported
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.severity, diagnostic.code.as_str()))
        .collect();

    assert_eq!(imported.source.kind, SlayTheDataSourceKind::RawRun);
    assert!(diagnostics.contains(&(
        SlayTheDataDiagnosticSeverity::Error,
        "unsupported_character"
    )));
    assert!(diagnostics.contains(&(
        SlayTheDataDiagnosticSeverity::Error,
        "unsupported_ascension"
    )));
    assert!(diagnostics.contains(&(
        SlayTheDataDiagnosticSeverity::Warning,
        "ambiguous_repeated_grid_floor"
    )));
}

#[test]
fn imports_selected_jsonl_line() {
    let rows = r#"{"character_chosen":"IRONCLAD","seed_played":"FIRST"}
{"character_chosen":"IRONCLAD","seed_played":"SECOND"}"#;

    let imported = import_slaythedata_jsonl_line(rows, 1).expect("imports second row");

    assert_eq!(imported.config.seed_played.as_deref(), Some("SECOND"));
}

#[test]
fn derives_floor_grouped_replay_plan_from_import() {
    let imported = import_slaythedata_run_json(
        r#"{
            "event": {
                "character_chosen": "IRONCLAD",
                "ascension_level": 0,
                "seed_played": "PLAN01",
                "neow_bonus": "TEN_PERCENT_HP_BONUS",
                "neow_cost": "NONE",
                "path_per_floor": ["M", "?"],
                "card_choices": [{"floor": 1, "picked": "Inflame"}],
                "event_choices": [{"floor": 2, "event_name": "Golden Shrine", "player_choice": "Pray"}],
                "items_purchased": ["Shrug It Off"],
                "item_purchase_floors": [3],
                "campfire_choices": [{"floor": 4, "key": "SMITH", "data": "Bash+"}],
                "potions_floor_usage": [1],
                "boss_relics": [{"picked": "Black Blood"}],
                "master_deck": ["Bash", "Inflame"],
                "relics": ["Burning Blood"],
                "floor_reached": 4,
                "gold": 99
            }
        }"#,
    )
    .expect("imports");

    let plan = slaythedata_replay_plan(&imported);

    assert_eq!(plan.ordering, SlayTheDataReplayOrdering::FloorGrouped);
    assert_eq!(plan.run_start.as_ref().unwrap().seed_played, "PLAN01");
    assert!(matches!(
        plan.steps[0].kind,
        SlayTheDataReplayStepKind::NeowTalk
    ));
    assert!(matches!(
        plan.steps[1].kind,
        SlayTheDataReplayStepKind::NeowBonus { .. }
    ));
    assert!(matches!(
        plan.steps[3].kind,
        SlayTheDataReplayStepKind::MapRoom { .. }
    ));
    assert!(plan
        .steps
        .iter()
        .any(|step| matches!(step.kind, SlayTheDataReplayStepKind::CardReward { .. })));
    assert!(plan.steps.iter().any(|step| matches!(
        step.kind,
        SlayTheDataReplayStepKind::PotionBudget { uses_allowed: 1 }
    )));
    assert!(plan.steps.iter().any(|step| matches!(
        step.kind,
        SlayTheDataReplayStepKind::BossRelic { act: 1, .. }
    )));
    assert!(plan
        .checkpoints
        .iter()
        .any(|checkpoint| checkpoint.floor == Some(4)));
}

#[test]
fn replay_plan_reports_missing_start_identity() {
    let imported = import_slaythedata_run_json(r#"{"character_chosen":"IRONCLAD"}"#)
        .expect("imports partial row");

    let plan = slaythedata_replay_plan(&imported);

    assert!(plan.run_start.is_none());
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing_run_start_identity"));
}

#[test]
fn preflight_checks_neow_talk_against_simulator_state() {
    let imported = import_slaythedata_run_json(
        r#"{
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "PLAN01",
            "neow_bonus": "TEN_PERCENT_HP_BONUS",
            "neow_cost": "NONE"
        }"#,
    )
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    assert_eq!(
        report.numeric_seed,
        Some(sts_verify::sts_seed_string_to_long("PLAN01"))
    );
    assert_eq!(report.steps[0].status, SlayTheDataPreflightStatus::Checked);
    assert_eq!(report.steps[0].code, "legal_neow_talk");
    assert_eq!(
        report.steps[0]
            .bridge_command
            .as_ref()
            .map(|hint| &hint.descriptor),
        Some(&SlayTheDataBridgeDescriptor::ChooseVisibleOption { option_slot: 0 })
    );
    assert_eq!(
        report.steps[0]
            .bridge_command
            .as_ref()
            .map(|hint| hint.command.as_str()),
        Some("CHOOSE 0")
    );
    assert_eq!(report.steps[1].status, SlayTheDataPreflightStatus::Checked);
    assert_eq!(report.steps[1].code, "legal_neow_bonus");
    assert!(report.steps[1].bridge_command.is_some());
    assert_eq!(report.steps[2].status, SlayTheDataPreflightStatus::Checked);
    assert_eq!(report.steps[2].code, "legal_neow_leave");
    assert_eq!(
        report.steps[2]
            .bridge_command
            .as_ref()
            .map(|hint| hint.command.as_str()),
        Some("CHOOSE 0")
    );
}

#[test]
fn preflight_accepts_slaythedata_signed_numeric_seed() {
    let imported = import_slaythedata_run_json(
        r#"{
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "-5230933468808623542",
            "neow_bonus": "TEN_PERCENT_HP_BONUS",
            "neow_cost": "NONE"
        }"#,
    )
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    assert_eq!(report.numeric_seed, Some(-5_230_933_468_808_623_542));
    assert!(!report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_seed_played"));
}

#[test]
fn preflight_checks_open_card_reward_against_core_choices() {
    let (seed, picked, cost) = generated_neow_three_cards_fixture();
    let imported = import_slaythedata_run_json(&format!(
        r#"{{
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "{seed}",
            "neow_bonus": "THREE_CARDS",
            "neow_cost": "{cost}",
            "card_choices": [{{"floor": 0, "picked": "{picked}"}}]
        }}"#,
    ))
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    let reward_step = report
        .steps
        .iter()
        .find(|step| step.code == "legal_card_reward")
        .expect("checked card reward");
    assert_eq!(reward_step.status, SlayTheDataPreflightStatus::Checked);
    assert_eq!(
        reward_step
            .bridge_command
            .as_ref()
            .map(|hint| &hint.descriptor),
        Some(&SlayTheDataBridgeDescriptor::ChooseVisibleOption { option_slot: 0 })
    );
    assert_eq!(
        reward_step
            .bridge_command
            .as_ref()
            .map(|hint| hint.command.as_str()),
        Some("CHOOSE 0")
    );
}

#[test]
fn preflight_checks_card_reward_skip_against_core_choices() {
    let (seed, _, cost) = generated_neow_three_cards_fixture();
    let imported = import_slaythedata_run_json(&format!(
        r#"{{
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "{seed}",
            "neow_bonus": "THREE_CARDS",
            "neow_cost": "{cost}",
            "card_choices": [{{"floor": 0, "picked": "SKIP"}}]
        }}"#
    ))
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    let reward_step = report
        .steps
        .iter()
        .find(|step| step.code == "legal_card_reward")
        .expect("checked card reward skip");
    assert_eq!(reward_step.status, SlayTheDataPreflightStatus::Checked);
    assert_eq!(
        reward_step
            .bridge_command
            .as_ref()
            .map(|hint| &hint.descriptor),
        Some(&SlayTheDataBridgeDescriptor::SkipVisibleReward)
    );
    assert_eq!(
        reward_step
            .bridge_command
            .as_ref()
            .map(|hint| hint.command.as_str()),
        Some("SKIP")
    );
}

#[test]
fn preflight_does_not_emit_hints_after_blocked_authoritative_step() {
    let imported = import_slaythedata_run_json(
        r#"{
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "CARD000",
            "neow_bonus": "THREE_CARDS",
            "neow_cost": "NONE",
            "card_choices": [{"floor": 0, "picked": "SKIP"}]
        }"#,
    )
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    assert!(report
        .steps
        .iter()
        .any(|step| step.code == "neow_option_not_available"
            && step.status == SlayTheDataPreflightStatus::Blocked));
    assert!(report
        .steps
        .iter()
        .filter(|step| step.ordinal > 1)
        .all(|step| step.bridge_command.is_none()));
}

#[test]
fn preflight_blocks_steps_when_run_state_cannot_be_initialized() {
    let imported = import_slaythedata_run_json(
        r#"{
            "character_chosen": "THE_SILENT",
            "ascension_level": 0,
            "seed_played": "PLAN01",
            "neow_bonus": "TEN_PERCENT_HP_BONUS",
            "neow_cost": "NONE"
        }"#,
    )
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "cannot_initialize_run_state"));
    assert_eq!(report.steps[0].status, SlayTheDataPreflightStatus::Blocked);
    assert_eq!(report.steps[0].code, "missing_run_state");
}

#[test]
fn preflight_reports_ambiguous_map_symbol_without_map_coordinates() {
    let imported = import_slaythedata_run_json(
        r#"{
            "character_chosen": "IRONCLAD",
            "ascension_level": 0,
            "seed_played": "PLAN01",
            "neow_bonus": "TEN_PERCENT_HP_BONUS",
            "neow_cost": "NONE",
            "path_per_floor": ["M"]
        }"#,
    )
    .expect("imports");
    let plan = slaythedata_replay_plan(&imported);

    let report = slaythedata_replay_preflight(&plan);

    let route_step = report
        .steps
        .iter()
        .find(|step| step.code == "ambiguous_map_symbol")
        .expect("ambiguous route step");
    assert_eq!(route_step.status, SlayTheDataPreflightStatus::Guided);
    assert!(route_step.bridge_command.is_none());
}
