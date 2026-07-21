use std::{fmt::Debug, fs};
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
    assess_verification, canonical_diff, corpus_path, load_corpus_file, observations_from_trace,
    verify_communication_mod_trace, verify_seed_start_communication_mod_trace,
    ActionDispositionKind, ManualFixture, VerificationCorpusManifest,
    VERIFICATION_CORPUS_MANIFEST_SCHEMA,
};

#[derive(Debug, serde::Deserialize)]
struct Act1CorpusManifest {
    status: String,
    required_passing_traces: Act1CorpusRequirements,
    entries: Vec<Act1CorpusEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct Act1CorpusRequirements {
    min: usize,
}

#[derive(Debug, serde::Deserialize)]
struct Act1CorpusEntry {
    path: String,
    external_seed: String,
    numeric_seed: i64,
    failed: bool,
    first_boundary_category: String,
}

#[derive(Debug, serde::Deserialize)]
struct LiveRegressionManifest {
    entries: Vec<LiveRegressionEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct LiveRegressionEntry {
    path: String,
    external_seed: String,
    expected_verified: bool,
    rust_seed_start_unexpected_diffs: usize,
}

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
    assert!(report.unexpected_diffs.is_empty());

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(
        !seed_start.failed,
        "seed-start boundary: {:?}",
        seed_start.first_boundary
    );
    assert_eq!(seed_start.start_command.action_step, 2);
    assert_eq!(seed_start.start_command.character, "IRONCLAD");
    assert_eq!(seed_start.start_command.ascension, 0);
    assert_eq!(seed_start.start_command.external_seed, "VERIFY01");
    assert_eq!(seed_start.start_command.numeric_seed, 1_957_307_888_551);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");
    assert!(seed_start
        .first_boundary
        .reason
        .contains("verifiable transitions"));

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
        "Bash",
        "Strike_R",
        "end turn",
        "Strike_R",
        "Strike_R",
        "Defend_R",
        "end turn",
        "Bash",
        "Strike_R",
        "captured Cultist lethal Strike",
        "gold reward",
        "card reward",
        "card reward pick 0",
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
            .all(|entry| !entry.reason.contains("unchosen Neow branches")),
        "unchosen Neow branches are counterfactual caveats, not unsupported transitions: {:?}",
        report.unsupported
    );
    assert!(
        report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("Toy Ornithopter")
                && entry.reason.contains("no potion-use transition")),
        "Toy Ornithopter trace-only scope should be classified"
    );
}

#[test]
fn codex04_controller_trace_verifies_supported_seed_start_scope() {
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
            entry
                .reason
                .contains("draw/shuffle order after end turn is out-of-scope")
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
        !report
            .unsupported
            .iter()
            .any(|entry| entry.reason.contains("FuzzyLouseDefensive")),
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
    assert!(report.unexpected_diffs.is_empty());

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(
        !seed_start.failed,
        "seed-start boundary: {:?}",
        seed_start.first_boundary
    );
    assert_eq!(seed_start.start_command.external_seed, "CODEX04");
    assert_eq!(seed_start.start_command.numeric_seed, 22_079_335_079);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");
    assert!(seed_start
        .first_boundary
        .reason
        .contains("verifiable transitions"));

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
        "Dramatic Entrance",
        "map floor 2 monster node",
        "map floor 3 monster node",
        "gold reward",
        "card reward",
        "card reward pick 0",
        "return to map after floor 1",
        "gold reward",
        "potion reward",
        "card reward",
        "card reward pick 1",
        "return to map after floor 2",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified seed-start label {expected}; labels: {labels:?}"
        );
    }
    assert!(
        labels.iter().filter(|label| **label == "end turn").count() >= 5,
        "floor 1-3 combats should verify multiple end turns; labels: {labels:?}"
    );
    assert!(
        report
            .unsupported
            .iter()
            .all(|entry| !entry.reason.contains("unchosen Neow branches")),
        "unchosen CODEX04 Neow branches are counterfactual caveats, not unsupported transitions: {:?}",
        report.unsupported
    );
}

#[test]
fn codex03_seed_start_replays_neow_lament_three_combat_prefix() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-06-18T16-45-23-530Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(report.unexpected_diffs.is_empty());

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(
        !seed_start.failed,
        "seed-start boundary: {:?}",
        seed_start.first_boundary
    );
    assert_eq!(seed_start.start_command.external_seed, "CODEX03");
    assert_eq!(seed_start.start_command.numeric_seed, 22_079_335_078);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");
    assert!(seed_start
        .first_boundary
        .reason
        .contains("verifiable transitions"));
}

#[test]
fn test_seed_start_m28_shop_entry_parity() {
    let Some(content) = load_corpus_file("permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");

    let prefix_diffs: Vec<_> = report
        .unexpected_diffs
        .iter()
        .filter(|diff| diff.action_step <= 168)
        .collect();
    assert!(
        prefix_diffs.is_empty(),
        "unexpected diffs through shop entry (step <= 168): {prefix_diffs:?}"
    );

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    assert!(
        labels.contains(&"enter shop merchant"),
        "missing verified shop entry; labels: {labels:?}"
    );
}

#[test]
fn test_seed_start_m29_m290001_sentries_prefix_zero_diffs() {
    let Some(content) =
        load_corpus_file("communication_mod/trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:?}",
        report.unexpected_diffs
    );
    assert_eq!(
        report.verified.len(),
        225,
        "the cleaned M29 Sentries trace should verify every retained action"
    );

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(!seed_start.failed);
    assert_eq!(seed_start.start_command.external_seed, "M290001");
    assert_eq!(seed_start.start_command.numeric_seed, 40_560_393_126);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    assert!(labels.contains(&"map elite node 1"));
    assert!(labels.contains(&"relic reward"));
    assert!(labels.contains(&"trace client poll"));
}

#[test]
fn test_seed_start_m30_m290008_hexaghost_early_act1_slice() {
    let Some(content) =
        load_corpus_file("communication_mod/trace-2026-06-23T07-42-06-085Z.best-run.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:?}",
        report.unexpected_diffs
    );

    let seed_start = report.seed_start.expect("seed-start details");
    assert_eq!(seed_start.start_command.external_seed, "M290008");
    assert_eq!(seed_start.start_command.numeric_seed, 40_560_393_133);
    assert!(
        report.unsupported.iter().all(|entry| !entry
            .reason
            .contains("Sword Boomerang multi-enemy random target parity")),
        "Sword Boomerang should not remain an unsupported seed-start frontier: {:?}",
        report.unsupported
    );

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "Neow transform confirm",
        "captured Scrap Ooze success",
        "card reward",
        "card reward pick 0",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified label {expected}; labels: {labels:?}"
        );
    }
}

#[test]
fn test_m32c_20260625_retained_trace_records_32b_shop_reward_deck_evidence() {
    let Some(content) =
        load_corpus_file("permanent_traces/trace-2026-06-25T00-44-15-558Z.retained.step548.jsonl")
    else {
        return;
    };

    let trace = sts_verify::import_communication_mod_trace(&content).expect("prefix imports");
    assert_eq!(raw_trace_count(&content, "action"), 541);
    assert_eq!(raw_trace_count(&content, "state"), 526);
    assert_eq!(raw_trace_count(&content, "error"), 16);
    assert!(trace.lines.iter().all(|line| match line {
        sts_verify::TraceLine::State(state) => state.step <= 541,
        sts_verify::TraceLine::Action(action) => action.step <= 541,
        sts_verify::TraceLine::Error(error) => error.step <= 541,
        sts_verify::TraceLine::Metadata(_) => true,
        sts_verify::TraceLine::CommandAccept(accepted) => accepted.step <= 541,
        sts_verify::TraceLine::Response(response) => response.sequence <= 541,
        sts_verify::TraceLine::SlayTheData(_) => false,
        sts_verify::TraceLine::Automation(_) => false,
        sts_verify::TraceLine::CommandObservedTimeout(timeout) => timeout.step <= 541,
    }));

    let last = state_message_at_step(&content, 541).expect("step 541 state");
    assert_eq!(game_i64(&last, "floor"), Some(37));
    assert_eq!(game_str(&last, "screen_type"), Some("SHOP_SCREEN"));

    assert_screen_cards_include(&content, 21, &["Discovery", "Secret Technique"]);
    assert_screen_cards_include(&content, 43, &["Sword Boomerang"]);
    assert_deck_includes(&content, 44, "Sword Boomerang");
    assert_deck_includes(&content, 46, "Sword Boomerang");
    assert_screen_cards_include(&content, 227, &["Forethought", "Chrysalis"]);
    assert_screen_cards_include(&content, 541, &["Panache+"]);
    assert_deck_includes(&content, 541, "Sword Boomerang");

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "retained trace should replay without unexpected diffs: {:?}",
        report.unexpected_diffs
    );
}

#[test]
fn test_m33_manual01_selected_neow_random_rare_card_prefix() {
    let Some(content) = load_corpus_file(
        "communication_mod/trace-2026-06-25T00-44-15-558Z.neow-rare-prefix.step4.jsonl",
    ) else {
        return;
    };

    let trace = sts_verify::import_communication_mod_trace(&content).expect("prefix imports");
    assert_eq!(raw_trace_count(&content, "metadata"), 1);
    assert_eq!(raw_trace_count(&content, "action"), 4);
    assert_eq!(raw_trace_count(&content, "state"), 5);
    assert!(trace.lines.iter().all(|line| match line {
        sts_verify::TraceLine::State(state) => state.step <= 4,
        sts_verify::TraceLine::Action(action) => action.step <= 4,
        sts_verify::TraceLine::Error(error) => error.step <= 4,
        sts_verify::TraceLine::Metadata(_) => true,
        sts_verify::TraceLine::CommandAccept(accepted) => accepted.step <= 4,
        sts_verify::TraceLine::Response(response) => response.sequence <= 4,
        sts_verify::TraceLine::SlayTheData(_) => false,
        sts_verify::TraceLine::Automation(_) => false,
        sts_verify::TraceLine::CommandObservedTimeout(timeout) => timeout.step <= 4,
    }));

    let options = state_message_at_step(&content, 2).expect("step 2 state");
    let choices = game_choice_labels(&options);
    assert!(
        choices.contains(&"obtain a random rare card"),
        "step 2 missing random rare Neow choice; choices: {choices:?}"
    );

    let leave = state_message_at_step(&content, 3).expect("step 3 state");
    assert_eq!(game_str(&leave, "screen_type"), Some("EVENT"));
    assert_eq!(game_choice_labels(&leave), vec!["leave"]);
    assert!(!deck_names(&leave).contains(&"Immolate"));

    let map = state_message_at_step(&content, 4).expect("step 4 state");
    assert_eq!(game_str(&map, "screen_type"), Some("MAP"));
    assert_deck_includes(&content, 4, "Immolate");
    assert_eq!(deck_names(&map).len(), 11);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:?}",
        report.unexpected_diffs
    );
    assert!(
        report.unsupported.is_empty(),
        "passing retained TEST trace should not report unsupported transitions: {:?}",
        report.unsupported
    );

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(!seed_start.failed);
    assert_eq!(seed_start.start_command.external_seed, "MANUAL01");
    assert_eq!(seed_start.start_command.numeric_seed, 1_435_099_163_226);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "seed-start bootstrap",
        "Neow talk",
        "Neow random rare card reward",
        "Neow leave",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified seed-start label {expected}; labels: {labels:?}"
        );
    }
}

#[test]
fn test_m33_m290005_selected_neow_remove_card_grid_prefix() {
    let Some(content) = load_corpus_file(
        "communication_mod/trace-2026-06-23T07-42-06-085Z.m290005-neow-remove-card.jsonl",
    ) else {
        return;
    };

    let trace = sts_verify::import_communication_mod_trace(&content).expect("prefix imports");
    assert_eq!(raw_trace_count(&content, "metadata"), 1);
    assert_eq!(raw_trace_count(&content, "action"), 7);
    assert_eq!(raw_trace_count(&content, "state"), 8);
    assert!(trace.lines.iter().all(|line| match line {
        sts_verify::TraceLine::State(state) => state.step <= 7,
        sts_verify::TraceLine::Action(action) => action.step <= 7,
        sts_verify::TraceLine::Error(error) => error.step <= 7,
        sts_verify::TraceLine::Metadata(_) => true,
        sts_verify::TraceLine::CommandAccept(accepted) => accepted.step <= 7,
        sts_verify::TraceLine::Response(response) => response.sequence <= 7,
        sts_verify::TraceLine::SlayTheData(_) => false,
        sts_verify::TraceLine::Automation(_) => false,
        sts_verify::TraceLine::CommandObservedTimeout(timeout) => timeout.step <= 7,
    }));

    let options = state_message_at_step(&content, 3).expect("step 3 state");
    let choices = game_choice_labels(&options);
    assert!(
        choices.contains(&"remove a card from your deck"),
        "step 3 missing remove-card Neow choice; choices: {choices:?}"
    );

    let grid = state_message_at_step(&content, 4).expect("step 4 state");
    assert_eq!(game_str(&grid, "screen_type"), Some("GRID"));
    assert_screen_cards_include(&content, 4, &["Strike", "Defend", "Bash"]);
    assert_eq!(deck_name_count(&grid, "Strike"), 5);

    let selected = state_message_at_step(&content, 5).expect("step 5 state");
    assert_eq!(game_str(&selected, "screen_type"), Some("GRID"));

    let leave = state_message_at_step(&content, 6).expect("step 6 state");
    assert_eq!(game_str(&leave, "screen_type"), Some("EVENT"));
    assert_eq!(game_choice_labels(&leave), vec!["leave"]);
    assert_eq!(deck_names(&leave).len(), 9);
    assert_eq!(deck_name_count(&leave, "Strike"), 4);

    let map = state_message_at_step(&content, 7).expect("step 7 state");
    assert_eq!(game_str(&map, "screen_type"), Some("MAP"));
    assert_eq!(deck_names(&map).len(), 9);
    assert_eq!(deck_name_count(&map, "Strike"), 4);

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:?}",
        report.unexpected_diffs
    );

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(!seed_start.failed);
    assert_eq!(seed_start.start_command.external_seed, "M290005");
    assert_eq!(seed_start.start_command.numeric_seed, 40_560_393_130);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "seed-start bootstrap",
        "Neow talk",
        "Neow remove card grid",
        "Neow grid select",
        "Neow grid confirm",
        "Neow leave",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified seed-start label {expected}; labels: {labels:?}"
        );
    }
}

#[test]
fn test_seed_start_boss_relic_retained_trace() {
    let Some(content) = load_corpus_file("permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:?}",
        report.unexpected_diffs
    );

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(!seed_start.failed);
    assert_eq!(seed_start.start_command.external_seed, "TEST");
    assert_eq!(seed_start.start_command.numeric_seed, 1_218_623);
    assert_eq!(seed_start.first_boundary.path, "$.actions[verified]");
    assert_eq!(seed_start.first_boundary.category, "none");

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "map event node 1",
        "map event node 2",
        "event choice",
        "enter shop merchant",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified label {expected}; labels: {labels:?}"
        );
    }
}

#[test]
fn m35_act1_manifest_entries_pass_seed_start() {
    let Some(manifest_content) = load_corpus_file("act1_a0_ironclad.json") else {
        return;
    };
    let manifest: Act1CorpusManifest =
        serde_json::from_str(&manifest_content).expect("Act 1 corpus manifest parses");

    assert!(
        manifest.status == "partial_data_blocked" || manifest.status == "satisfied",
        "unexpected Act 1 corpus status: {}",
        manifest.status
    );
    if manifest.status == "satisfied" {
        assert!(
            manifest.entries.len() >= manifest.required_passing_traces.min,
            "satisfied Act 1 corpus has only {} entries; expected at least {}",
            manifest.entries.len(),
            manifest.required_passing_traces.min
        );
    }

    for entry in manifest.entries {
        let content = load_corpus_file(&entry.path)
            .unwrap_or_else(|| panic!("manifest trace is readable: {}", entry.path));
        let report = verify_seed_start_communication_mod_trace(&content)
            .unwrap_or_else(|err| panic!("seed-start report for {}: {err}", entry.path));
        assert!(
            report.unexpected_diffs.is_empty(),
            "{} unexpected diffs: {:?}",
            entry.path,
            report.unexpected_diffs
        );

        let seed_start = report
            .seed_start
            .unwrap_or_else(|| panic!("seed-start details for {}", entry.path));
        assert_eq!(
            seed_start.failed, entry.failed,
            "{} failed mismatch",
            entry.path
        );
        assert_eq!(
            seed_start.start_command.external_seed, entry.external_seed,
            "{} external seed mismatch",
            entry.path
        );
        assert_eq!(
            seed_start.start_command.numeric_seed, entry.numeric_seed,
            "{} numeric seed mismatch",
            entry.path
        );
        assert_eq!(
            seed_start.first_boundary.category, entry.first_boundary_category,
            "{} first boundary mismatch",
            entry.path
        );
    }
}

#[test]
fn live_regression_manifest_entries_pass_seed_start() {
    let Some(manifest_content) = load_corpus_file("live_regressions.json") else {
        return;
    };
    let manifest: LiveRegressionManifest =
        serde_json::from_str(&manifest_content).expect("live regression manifest parses");

    for entry in manifest.entries {
        let content = load_corpus_file(&entry.path)
            .unwrap_or_else(|| panic!("live regression trace is readable: {}", entry.path));
        let report = verify_seed_start_communication_mod_trace(&content)
            .unwrap_or_else(|err| panic!("seed-start report for {}: {err}", entry.path));
        assert_eq!(
            report.unexpected_diffs.len(),
            entry.rust_seed_start_unexpected_diffs,
            "{} unexpected diff count changed: {:?}",
            entry.path,
            report.unexpected_diffs
        );
        let seed_start = report
            .seed_start
            .unwrap_or_else(|| panic!("seed-start details for {}", entry.path));
        assert_eq!(
            seed_start.start_command.external_seed, entry.external_seed,
            "{} external seed mismatch",
            entry.path
        );
        if entry.expected_verified {
            assert!(
                !report.verified.is_empty(),
                "{} had no verified seed-start transitions",
                entry.path
            );
        }
    }
}

#[test]
fn codex10_neow_transform_two_trace_verifies_through_first_map_node() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-07-06T16-59-52-285Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report.unexpected_diffs.is_empty(),
        "unexpected diffs: {:?}",
        report.unexpected_diffs
    );
    assert!(
        report.unsupported.is_empty(),
        "unsupported transitions: {:?}",
        report.unsupported
    );

    let seed_start = report.seed_start.expect("seed-start details");
    assert!(
        !seed_start.failed,
        "boundary: {:?}",
        seed_start.first_boundary
    );
    assert_eq!(seed_start.start_command.external_seed, "CODEX10");
    assert_eq!(seed_start.start_command.numeric_seed, 22_079_335_110);
    assert_eq!(seed_start.first_boundary.category, "none");

    let labels: Vec<_> = report
        .verified
        .iter()
        .map(|step| step.label.as_str())
        .collect();
    for expected in [
        "seed-start bootstrap",
        "Neow talk",
        "Neow transform two grid",
        "Neow grid select",
        "Neow grid confirm",
        "Neow leave",
        "map first monster node",
    ] {
        assert!(
            labels.contains(&expected),
            "missing verified label {expected}; labels: {labels:?}"
        );
    }
}

#[test]
fn library_grid_uses_target_card_group_bottom_order() {
    let Some(content) = load_corpus_file("communication_mod/trace-2026-07-07T18-33-54-807Z.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("Library grid regression trace replays");

    assert!(
        report.unexpected_diffs.is_empty(),
        "{:#?}",
        report.unexpected_diffs
    );
    assert!(report.unsupported.is_empty(), "{:#?}", report.unsupported);
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 541
            && transition.command.eq_ignore_ascii_case("CHOOSE 0")
            && transition.label == "event choice"
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 542 && transition.command.eq_ignore_ascii_case("CHOOSE 11")
    }));
}

#[test]
fn seed_start_random_rare_neow_reward_carries_into_first_combat() {
    let Some(content) =
        load_corpus_file("permanent_traces/live-regression-2026-07-02T23-24-13-178Z.jsonl")
    else {
        return;
    };

    assert_deck_includes(&content, 4, "Double Tap");
    assert_deck_includes(&content, 5, "Double Tap");

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report
            .verified
            .iter()
            .any(|transition| transition.action_step == 5
                && transition.label == "map first monster node"),
        "first map combat entry should verify; report: {report:#?}"
    );
    assert!(
        report
            .unexpected_diffs
            .iter()
            .all(|diff| diff.action_step > 5),
        "random rare Neow reward should not cause an early deck diff: {:?}",
        report.unexpected_diffs
    );
}

#[test]
fn seed_start_wing_statue_filters_locked_choice_list() {
    let Some(content) =
        load_corpus_file("permanent_traces/live-regression-2026-07-02T23-24-13-178Z.jsonl")
    else {
        return;
    };

    let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start report");
    assert!(
        report
            .verified
            .iter()
            .any(|transition| transition.action_step == 34
                && transition.label == "map event node 1"),
        "Wing Statue event entry should verify through the hidden locked option; report: {report:#?}"
    );
    assert!(
        report
            .unexpected_diffs
            .iter()
            .all(|diff| diff.action_step > 34),
        "hidden locked event choice should not cause an event choice-list diff: {:?}",
        report.unexpected_diffs
    );
}

#[test]
fn permanent_trace_entries_pass_seed_start() {
    let dir = corpus_path("permanent_traces");
    if !dir.exists() {
        return;
    }
    let manifest_content =
        load_corpus_file("permanent_traces.json").expect("permanent trace manifest is readable");
    let manifest: VerificationCorpusManifest =
        serde_json::from_str(&manifest_content).expect("permanent trace manifest parses");
    assert_eq!(manifest.schema, VERIFICATION_CORPUS_MANIFEST_SCHEMA);

    let mut actual_traces = fs::read_dir(&dir)
        .expect("permanent trace directory is readable")
        .map(|entry| entry.expect("permanent trace entry is readable").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("jsonl"))
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .expect("permanent trace filename is UTF-8")
                .to_owned()
        })
        .collect::<Vec<_>>();
    actual_traces.sort();
    let mut declared_traces = manifest
        .entries
        .iter()
        .map(|entry| entry.trace.clone())
        .collect::<Vec<_>>();
    declared_traces.sort();
    assert_eq!(
        declared_traces, actual_traces,
        "permanent trace manifest must exactly match the corpus directory"
    );

    for entry in manifest.entries {
        let path = dir.join(&entry.trace);
        let display_path = path.display().to_string();
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("permanent trace is readable: {display_path}: {err}"));
        let report = verify_seed_start_communication_mod_trace(&content)
            .unwrap_or_else(|err| panic!("seed-start report for {display_path}: {err}"));
        assert!(
            report.unexpected_diffs.is_empty(),
            "{display_path} unexpected diffs: {:?}",
            report.unexpected_diffs
        );
        assert_eq!(
            report.action_dispositions.len(),
            report.total_actions,
            "{display_path} must assign one ledger entry to every trace action"
        );
        let integrity = report
            .action_integrity
            .as_ref()
            .unwrap_or_else(|| panic!("{display_path} action-integrity evidence"));
        assert_eq!(
            integrity.applicable_actions + integrity.rejected_actions,
            report.total_actions,
            "{display_path} action-integrity scope must classify rejected commands explicitly"
        );
        assert_eq!(
            integrity.disposed_actions, integrity.applicable_actions,
            "{display_path} has unclassified trace actions: {:?}",
            report
                .action_dispositions
                .iter()
                .filter(|entry| entry.disposition == sts_verify::ActionDispositionKind::Unclassified)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            integrity.duplicate_dispositions, 0,
            "{display_path} has duplicate or orphan verifier dispositions"
        );
        let expected_unreconciled_boundary = matches!(
            &entry.expectation,
            sts_verify::VerificationExpectation::ExpectedBoundary { boundary }
                if boundary.category == "unreconciled_copied_attack_frame"
        );
        if expected_unreconciled_boundary {
            assert_eq!(
                integrity.unresolved_transient_assertions, 1,
                "{display_path} expected boundary must retain exactly its one causal unresolved transient assertion"
            );
        } else {
            assert_eq!(
                integrity.unresolved_transient_assertions, 0,
                "{display_path} has unresolved transient assertions"
            );
        }
        let outcome = assess_verification(Ok(&report), &entry.expectation, Some(integrity));
        assert!(
            outcome.is_success(),
            "{display_path} typed outcome: {outcome:#?}; ignored actions: {:?}",
            report
                .action_dispositions
                .iter()
                .filter(|entry| entry.disposition == sts_verify::ActionDispositionKind::IgnoredTail)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn fidelity_regression_trace_entries_pass_seed_start() {
    let dir = corpus_path("fidelity_regressions");
    if !dir.exists() {
        return;
    }

    let mut entries = fs::read_dir(&dir)
        .expect("fidelity regression trace directory is readable")
        .map(|entry| {
            entry
                .expect("fidelity regression trace entry is readable")
                .path()
        })
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    entries.sort();

    assert!(
        !entries.is_empty(),
        "fidelity regression directory should contain at least one .jsonl trace"
    );

    for path in entries {
        let display_path = path.display().to_string();
        let content = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("fidelity regression trace is readable: {display_path}: {err}")
        });
        let report = verify_seed_start_communication_mod_trace(&content)
            .unwrap_or_else(|err| panic!("seed-start report for {display_path}: {err}"));
        assert!(
            report.unexpected_diffs.is_empty(),
            "{display_path} unexpected diffs: {:?}",
            report.unexpected_diffs
        );
        let seed_start = report
            .seed_start
            .unwrap_or_else(|| panic!("seed-start details for {display_path}"));
        assert!(
            !seed_start.failed,
            "{display_path} boundary: {:?}",
            seed_start.first_boundary
        );
    }
}

#[test]
fn session13_golden_shrine_curse_is_deferred_until_the_stable_deck_frame() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-13-transmogrifier-curse-transform.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-13 Golden Shrine regression replays");

    assert!(report.unexpected_diffs.is_empty());
    let disposition = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 4467 && entry.command == "CHOOSE 2")
        .expect("Golden Shrine desecrate action has a disposition");
    assert_eq!(
        disposition.disposition,
        sts_verify::ActionDispositionKind::Verified
    );
    assert!(disposition.deferred_assertion_reconciled);
    let grid_disposition = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 4482 && entry.command == "CONFIRM")
        .expect("Transmogrifier grid confirmation has a disposition");
    assert_eq!(
        grid_disposition.disposition,
        sts_verify::ActionDispositionKind::Verified
    );
    assert!(
        grid_disposition.deferred_assertion_reconciled,
        "the transient event deck must reconcile before grid confirmation verifies"
    );
    assert_eq!(
        report
            .action_integrity
            .expect("verification integrity")
            .unresolved_transient_assertions,
        0
    );

    let mut truncated_lines = Vec::new();
    for line in content.lines() {
        truncated_lines.push(line);
        let value: serde_json::Value = serde_json::from_str(line).expect("trace line parses");
        if value.get("type").and_then(serde_json::Value::as_str) == Some("state")
            && value.get("step").and_then(serde_json::Value::as_u64) == Some(4468)
        {
            break;
        }
    }
    let truncated = truncated_lines.join("\n");
    let truncated_report = verify_seed_start_communication_mod_trace(&truncated)
        .expect("truncated session-13 trace replays");
    assert!(truncated_report.unexpected_diffs.is_empty());
    assert_eq!(
        truncated_report
            .action_integrity
            .expect("truncated verification integrity")
            .unresolved_transient_assertions,
        1,
        "a retained transient frame must not count as complete verification"
    );
}

#[test]
fn session31_stale_post_regression_verifies_the_settled_combat_action() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-31-floor1-stale-combat-post-state.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-31 stale post-state regression replays");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 458 && transition.command.eq_ignore_ascii_case("PLAY 2 1")
    }));
}

#[test]
fn session31_mushrooms_confirmation_regression_verifies_one_semantic_event_choice() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-31-floor8-mushrooms-confirmation.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-31 Mushrooms confirmation regression replays");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 678 && transition.command.eq_ignore_ascii_case("CHOOSE 0")
    }));
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 686 && transition.command.eq_ignore_ascii_case("END")
    }));
    assert!(!report
        .verified
        .iter()
        .any(|transition| transition.action_step == 679));
}

#[test]
fn session31_cursed_key_chest_regression_waits_for_the_queued_curse() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-31-floor22-cursed-key-chest-delay.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-31 Cursed Key chest regression replays");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 969
            && transition.command.eq_ignore_ascii_case("CHOOSE 0")
            && transition.label == "open treasure chest"
    }));
    assert!(!report
        .verified
        .iter()
        .any(|transition| transition.action_step == 970));
}

#[test]
fn session31_floor25_cursed_key_card_rng_regression_matches_live_reward() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-31-floor25-card-reward-rng.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-31 floor-25 card reward regression replays");

    assert!(report.unexpected_diffs.is_empty());
    assert!(report.unsupported.is_empty());
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1031 && transition.command.eq_ignore_ascii_case("CHOOSE 0")
    }));
}

#[test]
fn session32_tiny_house_upgrade_instance_regression_matches_first_combat() {
    let Some(content) = load_corpus_file(
        "fidelity_regressions/session-32-floor1-tiny-house-upgrade-instance.jsonl",
    ) else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-32 Tiny House upgrade-instance regression replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty());
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1062 && transition.command.eq_ignore_ascii_case("PLAY 1 0")
    }));
}

#[test]
fn session35_second_distilled_chaos_reshuffles_before_third_card() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-35-floor1-second-distilled-chaos.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-35 second Distilled Chaos regression replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty());
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1109 && transition.command.eq_ignore_ascii_case("POTION USE 2")
    }));
}

#[test]
fn session38_hex_dazed_reconciles_stable_selection_and_names_unresolved_endpoint() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-38-floor21-hex-dazed-insertion.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-38 Hex and Armaments regression replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty());
    for step in [1592, 1593, 1594, 1595] {
        let disposition = report
            .action_dispositions
            .iter()
            .find(|entry| entry.action_step == step)
            .unwrap_or_else(|| panic!("step {step} disposition"));
        assert_eq!(disposition.disposition, ActionDispositionKind::Verified);
        assert!(
            disposition.deferred_assertion_reconciled,
            "step {step} must reconcile only at the stable CONFIRM frame"
        );
    }
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 1596 && transition.command.eq_ignore_ascii_case("CONFIRM")
    }));
    let endpoint = report
        .action_dispositions
        .iter()
        .find(|entry| entry.action_step == 1986)
        .expect("terminal Armaments action disposition");
    assert_eq!(
        endpoint.disposition,
        ActionDispositionKind::PendingTransient
    );
    assert!(!endpoint.deferred_assertion_reconciled);
    assert_eq!(
        report
            .action_integrity
            .as_ref()
            .expect("action integrity")
            .unresolved_transient_assertions,
        1
    );
}

#[test]
fn session38_no_action_campfire_allows_immediate_proceed() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-38-floor23-no-campfire-actions.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-38 no-action campfire regression replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 2050 && transition.command.eq_ignore_ascii_case("PROCEED")
    }));
}

#[test]
fn session38_floor25_excludes_colosseum_before_selecting_masked_bandits() {
    let Some(content) =
        load_corpus_file("fidelity_regressions/session-38-floor25-colosseum-eligibility.jsonl")
    else {
        return;
    };
    let report = verify_seed_start_communication_mod_trace(&content)
        .expect("session-38 Colosseum eligibility regression replays");

    assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
    assert!(report.unsupported.is_empty(), "{report:#?}");
    assert!(report.verified.iter().any(|transition| {
        transition.action_step == 2061 && transition.command.eq_ignore_ascii_case("CHOOSE 1")
    }));
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
        RoomKind::Victory => "V",
    }
}

fn state_message_at_step(content: &str, step: u32) -> Option<serde_json::Value> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|value| {
            value.get("type").and_then(serde_json::Value::as_str) == Some("state")
                && value.get("step").and_then(serde_json::Value::as_u64) == Some(u64::from(step))
        })
        .and_then(|value| value.get("message").cloned())
}

fn raw_trace_count(content: &str, type_name: &str) -> usize {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value.get("type").and_then(serde_json::Value::as_str) == Some(type_name))
        .count()
}

fn game_str<'a>(message: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    message
        .get("game_state")
        .and_then(|game| game.get(key))
        .and_then(serde_json::Value::as_str)
}

fn game_i64(message: &serde_json::Value, key: &str) -> Option<i64> {
    message
        .get("game_state")
        .and_then(|game| game.get(key))
        .and_then(serde_json::Value::as_i64)
}

fn game_choice_labels(message: &serde_json::Value) -> Vec<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|choice| {
            choice
                .get("label")
                .and_then(serde_json::Value::as_str)
                .or_else(|| choice.as_str())
        })
        .collect()
}

fn deck_names(message: &serde_json::Value) -> Vec<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("deck"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|card| card.get("name").and_then(serde_json::Value::as_str))
        .collect()
}

fn deck_name_count(message: &serde_json::Value, expected_name: &str) -> usize {
    deck_names(message)
        .into_iter()
        .filter(|name| *name == expected_name)
        .count()
}

fn assert_screen_cards_include(content: &str, step: u32, expected_names: &[&str]) {
    let message = state_message_at_step(content, step).expect("state exists");
    let cards: Vec<_> = message
        .get("game_state")
        .and_then(|game| game.get("screen_state"))
        .and_then(|screen| screen.get("cards"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|card| card.get("name").and_then(serde_json::Value::as_str))
        .collect();

    for expected in expected_names {
        assert!(
            cards.contains(expected),
            "step {step} missing screen card {expected}; cards: {cards:?}"
        );
    }
}

fn assert_deck_includes(content: &str, step: u32, expected_name: &str) {
    let message = state_message_at_step(content, step).expect("state exists");
    let deck: Vec<_> = message
        .get("game_state")
        .and_then(|game| game.get("deck"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|card| card.get("name").and_then(serde_json::Value::as_str))
        .collect();

    assert!(
        deck.contains(&expected_name),
        "step {step} missing deck card {expected_name}; deck: {deck:?}"
    );
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
