use sts_verify::{
    import_slaythedata_jsonl_line, import_slaythedata_run_json, SlayTheDataDiagnosticSeverity,
    SlayTheDataSourceKind,
};

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
