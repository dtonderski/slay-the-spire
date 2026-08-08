use std::{
    fs,
    path::{Path, PathBuf},
};

use sts_verify::{
    assess_verification, import_communication_mod_trace, verify_communication_mod_trace,
    VerificationOutcome,
};

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verification/corpus")
}

fn communication_traces(relative: &str) -> Vec<PathBuf> {
    let root = corpus_root().join(relative);
    if !root.exists() {
        return Vec::new();
    }
    let mut paths = fs::read_dir(root)
        .expect("corpus directory is readable")
        .map(|entry| entry.expect("corpus entry is readable").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn assert_explicit_v1(path: &Path, content: &str) {
    let trace = import_communication_mod_trace(content)
        .unwrap_or_else(|error| panic!("{} imports: {error}", path.display()));
    let metadata = trace
        .metadata
        .as_ref()
        .unwrap_or_else(|| panic!("{} has metadata", path.display()));
    assert_eq!(metadata.schema, 1, "{} record schema", path.display());
    assert_eq!(
        metadata.boundary_schema,
        Some(1),
        "{} metadata boundary schema",
        path.display()
    );
    assert_eq!(
        metadata.source,
        "communication_mod",
        "{} metadata source",
        path.display()
    );
    assert!(
        metadata
            .run_config
            .as_ref()
            .and_then(|config| config.profile.as_ref())
            .is_some(),
        "{} has typed profile input",
        path.display()
    );
    for state in trace.lines.iter().filter_map(|line| match line {
        sts_verify::TraceLine::State(state) => Some(state),
        _ => None,
    }) {
        assert_eq!(
            state
                .message
                .get("boundary_schema")
                .and_then(serde_json::Value::as_u64),
            Some(1),
            "{} step {} state boundary schema",
            path.display(),
            state.step
        );
    }
}

#[test]
fn permanent_schema_v1_passes_are_genuine_complete_passes() {
    for path in communication_traces("permanent_traces") {
        let content = fs::read_to_string(&path).expect("permanent trace is readable");
        assert_explicit_v1(&path, &content);
        let report = verify_communication_mod_trace(&content)
            .unwrap_or_else(|error| panic!("{} verifies: {error}", path.display()));
        assert_eq!(
            assess_verification(Ok(&report), report.action_integrity.as_ref()),
            VerificationOutcome::CompletePass,
            "{} is not a genuine complete pass",
            path.display()
        );
    }
}

#[test]
fn schema_v1_failure_witnesses_are_honest_and_fully_accounted() {
    let paths = communication_traces("open_failures");
    assert!(
        !paths.is_empty(),
        "open_failures must contain strict schema-v1 evidence"
    );
    for path in paths {
        let content = fs::read_to_string(&path).expect("failure witness is readable");
        assert_explicit_v1(&path, &content);
        let report = verify_communication_mod_trace(&content)
            .unwrap_or_else(|error| panic!("{} verifies structurally: {error}", path.display()));
        let integrity = report
            .action_integrity
            .as_ref()
            .expect("strict report has action integrity");
        assert_eq!(report.action_dispositions.len(), report.total_actions);
        assert_eq!(
            integrity.applicable_actions + integrity.rejected_actions,
            report.total_actions
        );
        assert_eq!(integrity.disposed_actions, integrity.applicable_actions);
        assert_eq!(integrity.duplicate_dispositions, 0);
        assert!(
            !matches!(
                assess_verification(Ok(&report), Some(integrity)),
                VerificationOutcome::CompletePass
            ),
            "{} must remain honest failure evidence",
            path.display()
        );
    }
}
