use sts_verify::{
    canonical_diff, corpus_path, load_corpus_file, observations_from_trace, ManualFixture,
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
