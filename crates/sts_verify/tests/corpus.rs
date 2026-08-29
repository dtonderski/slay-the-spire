use std::{
    fs,
    path::{Path, PathBuf},
};

use sts_verify::{
    assess_verification, import_communication_mod_trace, verify_communication_mod_trace,
    VerificationOutcome,
};

const EXTERNAL_CORPUS_ENV: &str = "STS_PERMANENT_CORPUS_DIR";

fn external_corpus_traces() -> Vec<PathBuf> {
    let root = std::env::var_os(EXTERNAL_CORPUS_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| sts_verify::corpus_path("permanent_traces"));
    assert!(root.is_dir(), "{} is not a directory", root.display());
    let mut paths = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{} is not readable: {error}", root.display()))
        .map(|entry| entry.expect("corpus entry is readable").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "{} contains no JSONL traces",
        root.display()
    );
    paths
}

fn assert_current_boundary_schema(path: &Path, content: &str) {
    let trace = import_communication_mod_trace(content)
        .unwrap_or_else(|error| panic!("{} imports: {error}", path.display()));
    let metadata = trace
        .metadata
        .as_ref()
        .unwrap_or_else(|| panic!("{} has metadata", path.display()));
    assert_eq!(metadata.schema, 1, "{} record schema", path.display());
    assert_eq!(
        metadata.boundary_schema,
        Some(6),
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
            Some(6),
            "{} step {} state boundary schema",
            path.display(),
            state.step
        );
    }
}

#[test]
#[ignore = "requires STS_PERMANENT_CORPUS_DIR"]
fn external_permanent_traces_are_structurally_replayable() {
    for path in external_corpus_traces() {
        let content = fs::read_to_string(&path).expect("external trace is readable");
        assert_current_boundary_schema(&path, &content);
        let report = verify_communication_mod_trace(&content).unwrap_or_else(|error| {
            panic!("{} must not crash the verifier: {error}", path.display())
        });
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
    }
}

#[test]
#[ignore = "requires STS_PERMANENT_CORPUS_DIR and an explicit all-green run"]
fn external_permanent_traces_are_complete_passes() {
    for path in external_corpus_traces() {
        let content = fs::read_to_string(&path).expect("external trace is readable");
        assert_current_boundary_schema(&path, &content);
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
