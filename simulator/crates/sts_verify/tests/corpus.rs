use sts_verify::{
    canonical_diff, corpus_path, load_corpus_file, observations_from_trace,
    verify_communication_mod_trace, verify_seed_start_communication_mod_trace, ManualFixture,
    VerificationMode,
};

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
        "Twin Strike reward",
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
    assert!(seed_start.expected_failure);
    assert_eq!(seed_start.start_command.action_step, 2);
    assert_eq!(seed_start.start_command.character, "IRONCLAD");
    assert_eq!(seed_start.start_command.ascension, 0);
    assert_eq!(seed_start.start_command.external_seed, "VERIFY01");
    assert_eq!(seed_start.first_boundary.path, "$.actions[step=19].command");
    assert_eq!(
        seed_start.first_boundary.category,
        "unsupported_post_reward_map"
    );
    assert!(seed_start.first_boundary.reason.contains("Milestone 18"));

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
}
