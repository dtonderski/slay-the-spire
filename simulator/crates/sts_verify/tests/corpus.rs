use std::fmt::Debug;
use sts_core::content::encounters::generate_exordium_normal_encounters;
use sts_core::content::monsters::{
    target_cultist_hp_roll, target_jaw_worm_hp_roll, target_small_slimes_hp_rolls,
    target_two_louse_hp_rolls, TargetMonsterHp,
};
use sts_core::{
    generate_exordium_map_choices_after_path, generate_exordium_map_topology,
    ExordiumMapChoiceStep, RoomKind,
};
use sts_verify::{
    canonical_diff, corpus_path, load_corpus_file, observations_from_trace,
    verify_communication_mod_trace, verify_seed_start_communication_mod_trace, ManualFixture,
    VerificationMode,
};

#[derive(Debug, PartialEq, Eq)]
struct CapturedEncounterPrefix {
    action_step: u32,
    floor: i64,
    monsters: Vec<(String, i64, i64)>,
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedMapPrefix {
    action_step: u32,
    floor: i64,
    choices: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CapturedMapNode {
    x: i64,
    y: i64,
    symbol: String,
    children: Vec<(i64, i64)>,
}

#[test]
fn manual_milestone1_corpus_loads_if_present() {
    let path = corpus_path("manual/milestone1.jsonl");
    if !path.exists() {
        return;
    }

    let content = load_corpus_file("manual/milestone1.jsonl").expect("corpus file readable");
    let fixture: ManualFixture =
        serde_json::from_str(content.trim()).expect("manual fixture parses");

    assert_eq!(fixture.name, "milestone1_manual_win");
    assert_eq!(fixture.rng_draws, 0);
    assert_eq!(fixture.actions.len(), 2);
}

#[test]
fn codex04_trace_records_first_three_map_and_encounter_targets() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl")
    else {
        return;
    };

    let (maps, encounters) = captured_map_and_encounter_prefixes(&content);

    assert_sequence_eq(
        "CODEX04 first map-choice screens",
        maps,
        vec![
            CapturedMapPrefix {
                action_step: 5,
                floor: 0,
                choices: vec![
                    "x=0".to_owned(),
                    "x=2".to_owned(),
                    "x=4".to_owned(),
                    "x=5".to_owned(),
                ],
            },
            CapturedMapPrefix {
                action_step: 19,
                floor: 1,
                choices: vec!["x=3".to_owned()],
            },
            CapturedMapPrefix {
                action_step: 34,
                floor: 2,
                choices: vec!["x=2".to_owned(), "x=3".to_owned()],
            },
        ],
    );
    assert_sequence_eq(
        "CODEX04 generated map choices after path",
        generate_exordium_map_choices_after_path(22_079_335_079, &[2, 3]),
        vec![
            ExordiumMapChoiceStep {
                floor: 1,
                x: 2,
                next_choices: vec![3],
            },
            ExordiumMapChoiceStep {
                floor: 2,
                x: 3,
                next_choices: vec![2, 3],
            },
        ],
    );
    assert_sequence_eq(
        "CODEX04 first three encounters",
        encounters,
        vec![
            CapturedEncounterPrefix {
                action_step: 6,
                floor: 1,
                monsters: vec![("Cultist".to_owned(), 54, 54)],
            },
            CapturedEncounterPrefix {
                action_step: 20,
                floor: 2,
                monsters: vec![
                    ("Spike Slime (S)".to_owned(), 11, 11),
                    ("Acid Slime (M)".to_owned(), 32, 32),
                ],
            },
            CapturedEncounterPrefix {
                action_step: 35,
                floor: 3,
                monsters: vec![("Louse".to_owned(), 13, 13), ("Louse".to_owned(), 15, 15)],
            },
        ],
    );
    assert_sequence_eq(
        "CODEX04 floor-2 Small Slimes HP rolls",
        target_small_slimes_hp_rolls(22_079_335_079, 2, 0).expect("decoded reached variant"),
        vec![
            TargetMonsterHp {
                name: "Spike Slime (S)",
                hp: 11,
            },
            TargetMonsterHp {
                name: "Acid Slime (M)",
                hp: 32,
            },
        ],
    );
    assert_sequence_eq(
        "CODEX04 floor-3 louse HP rolls",
        target_two_louse_hp_rolls(22_079_335_079, 3, 0),
        vec![
            TargetMonsterHp {
                name: "Louse",
                hp: 13,
            },
            TargetMonsterHp {
                name: "Louse",
                hp: 15,
            },
        ],
    );
}

#[test]
fn verify01_trace_records_first_map_choice_targets() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T06-04-49-264Z.jsonl")
    else {
        return;
    };

    let (maps, _encounters) = captured_map_and_encounter_prefixes(&content);

    assert_sequence_eq(
        "VERIFY01 first map-choice screens",
        maps.into_iter().take(2).collect(),
        vec![
            CapturedMapPrefix {
                action_step: 5,
                floor: 0,
                choices: vec!["x=1".to_owned(), "x=2".to_owned()],
            },
            CapturedMapPrefix {
                action_step: 19,
                floor: 1,
                choices: vec!["x=2".to_owned()],
            },
        ],
    );
    assert_sequence_eq(
        "VERIFY01 generated map choices after path",
        generate_exordium_map_choices_after_path(1_957_307_888_551, &[1]),
        vec![ExordiumMapChoiceStep {
            floor: 1,
            x: 1,
            next_choices: vec![2],
        }],
    );
}

#[test]
fn codex03_lament_trace_records_first_three_map_and_encounter_targets() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl")
    else {
        return;
    };

    let (maps, encounters) = captured_map_and_encounter_prefixes(&content);

    assert_sequence_eq(
        "CODEX03 first map-choice screens",
        maps,
        vec![
            CapturedMapPrefix {
                action_step: 4,
                floor: 0,
                choices: vec!["x=1".to_owned(), "x=2".to_owned(), "x=5".to_owned()],
            },
            CapturedMapPrefix {
                action_step: 10,
                floor: 1,
                choices: vec!["x=0".to_owned(), "x=1".to_owned()],
            },
            CapturedMapPrefix {
                action_step: 16,
                floor: 2,
                choices: vec!["x=1".to_owned()],
            },
        ],
    );
    assert_sequence_eq(
        "CODEX03 generated map choices after path",
        generate_exordium_map_choices_after_path(22_079_335_078, &[1, 0, 1])
            .into_iter()
            .take(2)
            .collect(),
        vec![
            ExordiumMapChoiceStep {
                floor: 1,
                x: 1,
                next_choices: vec![0, 1],
            },
            ExordiumMapChoiceStep {
                floor: 2,
                x: 0,
                next_choices: vec![1],
            },
        ],
    );
    assert_sequence_eq(
        "CODEX03 first three encounters under Neow's Lament",
        encounters.into_iter().take(3).collect(),
        vec![
            CapturedEncounterPrefix {
                action_step: 5,
                floor: 1,
                monsters: vec![("Jaw Worm".to_owned(), 1, 43)],
            },
            CapturedEncounterPrefix {
                action_step: 11,
                floor: 2,
                monsters: vec![("Cultist".to_owned(), 1, 54)],
            },
            CapturedEncounterPrefix {
                action_step: 17,
                floor: 3,
                monsters: vec![("Louse".to_owned(), 1, 12), ("Louse".to_owned(), 1, 16)],
            },
        ],
    );
    assert_sequence_eq(
        "CODEX03 generated normal encounter prefix",
        generate_exordium_normal_encounters(22_079_335_078)
            .into_iter()
            .take(3)
            .collect(),
        vec![
            "Jaw Worm".to_owned(),
            "Cultist".to_owned(),
            "2 Louse".to_owned(),
        ],
    );
    assert_eq!(target_jaw_worm_hp_roll(22_079_335_078, 1, 0), 43);
    assert_eq!(target_cultist_hp_roll(22_079_335_078, 2, 0), 54);
    assert_sequence_eq(
        "CODEX03 floor-3 louse max HP rolls",
        target_two_louse_hp_rolls(22_079_335_078, 3, 0),
        vec![
            TargetMonsterHp {
                name: "Louse",
                hp: 12,
            },
            TargetMonsterHp {
                name: "Louse",
                hp: 16,
            },
        ],
    );
}

#[test]
fn codex03_full_captured_map_matches_target_topology() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl")
    else {
        return;
    };

    assert_captured_map_matches_target_topology(&content, 22_079_335_078);
}

#[test]
fn codex04_full_captured_map_matches_target_topology() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl")
    else {
        return;
    };

    assert_captured_map_matches_target_topology(&content, 22_079_335_079);
}

#[test]
fn verify01_full_captured_map_matches_target_topology() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T06-04-49-264Z.jsonl")
    else {
        return;
    };

    assert_captured_map_matches_target_topology(&content, 1_957_307_888_551);
}

fn assert_captured_map_matches_target_topology(content: &str, seed: i64) {
    let captured = captured_first_full_map(content);
    let generated = generate_exordium_map_topology(seed)
        .assigned_rooms
        .iter()
        .map(|room| CapturedMapNode {
            x: i64::from(room.x),
            y: room.row as i64,
            symbol: room_symbol(room.room_kind).to_owned(),
            children: room
                .children
                .iter()
                .map(|child| (i64::from(child.x), child.row as i64))
                .collect(),
        })
        .collect::<Vec<_>>();

    assert_sequence_eq("captured Exordium full map", generated, captured);
}

#[test]
fn cultist_manual_fixture_matches_imported_trace_step_if_present() {
    let (Some(trace_content), Some(manual_content)) = (
        load_corpus_file("communication_mod/trace-2026-06-18T00-53-06-235Z.jsonl"),
        load_corpus_file("manual/cultist_bash.json"),
    ) else {
        return;
    };

    let manual: serde_json::Value = serde_json::from_str(&manual_content).expect("manual json");
    let expected = manual
        .get("observation")
        .expect("observation field")
        .to_string();
    let step = manual
        .get("step")
        .and_then(|value| value.as_u64())
        .expect("step") as u32;

    let imported = observations_from_trace(&trace_content).expect("import trace");
    let actual = imported
        .iter()
        .find(|entry| entry.step == step)
        .expect("trace step")
        .observation
        .clone();
    let actual_json = serde_json::to_string(&actual).expect("serialize observation");

    let diffs = canonical_diff(&expected, &actual_json);
    assert!(diffs.is_empty(), "diffs: {diffs:?}");
}

#[test]
fn communication_mod_trace_imports_actions_if_present() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T00-53-06-235Z.jsonl")
    else {
        return;
    };

    let trace = sts_verify::import_communication_mod_trace(&content).expect("import");
    let actions = trace
        .lines
        .iter()
        .filter(|line| matches!(line, sts_verify::TraceLine::Action(_)))
        .count();
    assert!(actions >= 1);
}

#[test]
fn captured_communication_mod_trace_verifies_supported_sim_real_scope() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T06-04-49-264Z.jsonl")
    else {
        return;
    };

    let report = verify_communication_mod_trace(&content).expect("verify trace");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:#?}",
        report.unexpected_diffs
    );

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();

    for expected in [
        "Bash",
        "Strike_R",
        "Defend_R",
        "end turn",
        "combat victory + Burning Blood",
        "gold reward",
        "card reward",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified label {expected}; labels: {labels:?}"
        );
    }

    assert!(
        report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("seed-start run creation")),
        "seed-start parity gap should be explicit"
    );
    assert!(
        report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("reward RNG parity")),
        "reward RNG parity gap should be explicit"
    );
}

#[test]
fn captured_trace_seed_start_mode_reports_expected_rng_boundary() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T06-04-49-264Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert_eq!(report.mode, VerificationMode::SeedStart);
    assert!(report.unexpected_diffs.is_empty());

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(!seed_start.expected_failure);
    assert_eq!(seed_start.start_command.action_step, 2);
    assert_eq!(seed_start.start_command.character, "IRONCLAD");
    assert_eq!(seed_start.start_command.ascension, 0);
    assert_eq!(seed_start.start_command.external_seed, "VERIFY01");
    assert_eq!(seed_start.start_command.numeric_seed, 1_957_307_888_551);
    assert_eq!(seed_start.first_boundary.path, "$.actions[complete]");
    assert_eq!(seed_start.first_boundary.category, "none");
    assert!(seed_start.first_boundary.reason.contains("return-to-map"));

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "seed-start bootstrap",
        "Neow talk",
        "Neow common relic",
        "Neow leave",
        "map first monster node",
        "captured Cultist Bash",
        "captured Cultist Strike after Bash",
        "captured Cultist first end turn",
        "captured Cultist second-turn Strike one",
        "captured Cultist second-turn Strike two",
        "captured Cultist Defend",
        "captured Cultist second end turn and shuffle",
        "captured Cultist final Bash",
        "captured Cultist lethal Strike",
        "captured gold reward",
        "captured card reward choices",
        "captured Twin Strike pickup",
        "captured return to map",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified seed-start label {expected}; labels: {labels:?}"
        );
    }
    assert!(
        report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("unchosen Neow branches")),
        "unchosen Neow branches should be classified"
    );
    assert!(
        report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("Toy Ornithopter")),
        "Toy Ornithopter inert scope should be classified"
    );

    for stream in [
        "seed_conversion",
        "neowRng",
        "mapRng",
        "monsterRng",
        "monsterHpRng",
        "shuffleRng",
        "cardRewardRng",
        "rewardGoldRng",
        "relicRng",
        "potionRng",
    ] {
        assert!(
            seed_start
                .rng_boundaries
                .iter()
                .any(|boundary| boundary.stream == stream),
            "missing RNG boundary for {stream}"
        );
    }
    let seed_conversion = seed_start
        .rng_boundaries
        .iter()
        .find(|boundary| boundary.stream == "seed_conversion")
        .expect("seed conversion boundary");
    assert_eq!(seed_conversion.status, "source_backed");
    assert!(
        seed_start
            .rng_boundaries
            .iter()
            .all(|boundary| boundary.status != "captured_opaque"),
        "no RNG boundary should remain captured_opaque"
    );

    let m22 = seed_start
        .m22_encounter_report
        .as_ref()
        .expect("m22 encounter report");
    assert!(m22.mismatches.is_empty(), "m22 mismatches: {:?}", m22.mismatches);
    assert_eq!(m22.verified_entries.len(), 3);
    assert_eq!(
        m22
            .verified_entries
            .iter()
            .map(|entry| entry.encounter_key.as_str())
            .collect::<Vec<_>>(),
        vec!["Cultist", "Jaw Worm", "2 Louse"]
    );
}

#[test]
fn codex04_controller_trace_verifies_supported_observed_state_scope() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl")
    else {
        return;
    };

    let report = verify_communication_mod_trace(&content).expect("verify trace");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:#?}",
        report.unexpected_diffs
    );

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "Dramatic Entrance",
        "Bash",
        "Strike_R",
        "end turn",
        "combat victory + Burning Blood",
        "gold reward",
        "card reward",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified label {expected}; labels: {labels:?}"
        );
    }

    assert!(
        report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("seed-start run creation")),
        "seed-start parity gap should be explicit"
    );
    assert!(
        !report.unsupported.iter().any(|entry| {
            entry.reason.contains("draw/shuffle order after end turn is out-of-scope")
        }),
        "END transitions should no longer be unsupported for draw/shuffle scope"
    );
    assert!(
        !report.unsupported.iter().any(|entry| {
            entry.reason.contains("AcidSlime_M") || entry.reason.contains("SpikeSlime_S")
        }),
        "slime combat should be verified, not unsupported"
    );
    assert!(
        !report.unsupported.iter().any(|entry| entry.reason.contains("FuzzyLouseDefensive")),
        "louse combat should be verified, not unsupported"
    );
    assert!(
        labels.iter().filter(|label| **label == "end turn").count() >= 5,
        "floor 1-3 combats should verify multiple end turns; labels: {labels:?}"
    );
}

#[test]
fn codex04_seed_start_enters_first_captured_encounter_after_colorless_neow_pick() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert_eq!(report.mode, VerificationMode::SeedStart);
    assert!(report.unexpected_diffs.is_empty());

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(seed_start.expected_failure);
    assert_eq!(seed_start.start_command.external_seed, "CODEX04");
    assert_eq!(seed_start.start_command.numeric_seed, 22_079_335_079);
    assert_eq!(seed_start.first_boundary.path, "$.actions[step=7].command");
    assert_eq!(
        seed_start.first_boundary.category,
        "unsupported_combat_path"
    );

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "seed-start bootstrap",
        "Neow talk",
        "Neow colorless reward choices",
        "Neow Dramatic Entrance pickup",
        "Neow leave",
        "map first monster node",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified seed-start label {expected}; labels: {labels:?}"
        );
    }
    assert!(
        report.unsupported.iter().any(|entry| entry
            .reason
            .contains("potions, max-hp removal, and boss swap")),
        "unchosen CODEX04 Neow branches should be named"
    );

    let m22 = seed_start
        .m22_encounter_report
        .as_ref()
        .expect("m22 encounter report");
    assert!(m22.mismatches.is_empty(), "m22 mismatches: {:?}", m22.mismatches);
    assert_eq!(m22.verified_entries.len(), 3);
    assert_eq!(
        m22
            .verified_entries
            .iter()
            .map(|entry| entry.encounter_key.as_str())
            .collect::<Vec<_>>(),
        vec!["Cultist", "Small Slimes", "2 Louse"]
    );
}

fn captured_first_full_map(content: &str) -> Vec<CapturedMapNode> {
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).expect("trace line parses");
        let Some(nodes) = value
            .get("message")
            .and_then(|message| message.get("game_state"))
            .and_then(|game| game.get("map"))
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };

        return nodes
            .iter()
            .map(|node| CapturedMapNode {
                x: node
                    .get("x")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(-1),
                y: node
                    .get("y")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(-1),
                symbol: node
                    .get("symbol")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                children: node
                    .get("children")
                    .and_then(serde_json::Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .map(|child| {
                        (
                            child
                                .get("x")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or(-1),
                            child
                                .get("y")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or(-1),
                        )
                    })
                    .collect(),
            })
            .collect();
    }

    Vec::new()
}

fn room_symbol(room_kind: RoomKind) -> &'static str {
    match room_kind {
        RoomKind::Combat => "M",
        RoomKind::Elite => "E",
        RoomKind::Event => "?",
        RoomKind::Rest => "R",
        RoomKind::Shop => "$",
        RoomKind::Treasure => "T",
        RoomKind::Boss => "B",
    }
}

fn assert_sequence_eq<T>(label: &str, actual: Vec<T>, expected: Vec<T>)
where
    T: Debug + PartialEq,
{
    if let Some(message) = first_sequence_mismatch(label, &actual, &expected) {
        panic!("{message}");
    }
}

fn first_sequence_mismatch<T>(label: &str, actual: &[T], expected: &[T]) -> Option<String>
where
    T: Debug + PartialEq,
{
    if actual.len() != expected.len() {
        return Some(format!(
            "{label} length mismatch: actual {}, expected {}",
            actual.len(),
            expected.len()
        ));
    }
    actual
        .iter()
        .zip(expected.iter())
        .enumerate()
        .find_map(|(index, (actual_item, expected_item))| {
            (actual_item != expected_item).then(|| {
                format!(
                    "{label} mismatch at index {index}: actual {actual_item:?}, expected {expected_item:?}"
                )
            })
        })
}

fn captured_map_and_encounter_prefixes(
    content: &str,
) -> (Vec<CapturedMapPrefix>, Vec<CapturedEncounterPrefix>) {
    let mut pending_action_step = None;
    let mut maps = Vec::new();
    let mut encounters = Vec::new();
    let mut encounter_floors = Vec::new();

    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).expect("trace line parses");
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("action") => {
                pending_action_step = value.get("step").and_then(serde_json::Value::as_u64);
            }
            Some("state") => {
                let Some(action_step) = pending_action_step.take() else {
                    continue;
                };
                let Some(game) = value
                    .get("message")
                    .and_then(|message| message.get("game_state"))
                else {
                    continue;
                };
                let screen_type = game
                    .get("screen_type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let floor = game
                    .get("floor")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(-1);

                if screen_type == "MAP" && maps.len() < 3 {
                    let empty_choices = Vec::new();
                    maps.push(CapturedMapPrefix {
                        action_step: action_step as u32,
                        floor,
                        choices: game
                            .get("choice_list")
                            .and_then(serde_json::Value::as_array)
                            .unwrap_or(&empty_choices)
                            .iter()
                            .filter_map(|choice| {
                                choice
                                    .get("label")
                                    .and_then(serde_json::Value::as_str)
                                    .or_else(|| choice.as_str())
                                    .map(str::to_owned)
                            })
                            .collect(),
                    });
                }

                let Some(monsters) = game
                    .get("combat_state")
                    .and_then(|combat| combat.get("monsters"))
                    .and_then(serde_json::Value::as_array)
                else {
                    continue;
                };
                if !monsters.is_empty()
                    && encounters.len() < 3
                    && !encounter_floors.contains(&floor)
                {
                    encounter_floors.push(floor);
                    encounters.push(CapturedEncounterPrefix {
                        action_step: action_step as u32,
                        floor,
                        monsters: monsters
                            .iter()
                            .map(|monster| {
                                (
                                    monster
                                        .get("name")
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or("")
                                        .to_owned(),
                                    monster
                                        .get("current_hp")
                                        .and_then(serde_json::Value::as_i64)
                                        .unwrap_or(-1),
                                    monster
                                        .get("max_hp")
                                        .and_then(serde_json::Value::as_i64)
                                        .unwrap_or(-1),
                                )
                            })
                            .collect(),
                    });
                }
            }
            _ => {}
        }
    }

    (maps, encounters)
}
