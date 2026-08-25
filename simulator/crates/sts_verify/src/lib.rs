#![forbid(unsafe_code)]
#![doc = "Trace formats, canonical diffs, and fixture loaders for simulator verification."]

pub mod diff;
pub mod minimize;
pub mod outcome;
pub mod seed;
pub mod sim_real;
pub mod slaythedata;
pub mod trace;

pub use diff::{canonical_diff, canonical_value_diff};
pub use minimize::{
    minimize_communication_mod_trace, serialize_communication_mod_trace, MinimizeError,
    MinimizeFailureKind, MinimizeReport,
};
pub use outcome::{
    assess_verification, assess_verification_with_options, AssessmentOptions, VerificationFailure,
    VerificationIntegrity, VerificationOutcome,
};
pub use seed::{
    sts_seed_long_to_string, sts_seed_string_to_long, try_sts_seed_string_to_long,
    STS_SEED_ALPHABET,
};
pub use sim_real::{
    replay_communication_mod_trace, replay_communication_mod_trace_reader,
    verify_communication_mod_trace, verify_communication_mod_trace_diagnostic_reader,
    verify_communication_mod_trace_reader, verify_seed_start_communication_mod_trace,
    ActionDisposition, ActionDispositionKind, ReplayCheckpoint, ReplayCheckpointState,
    ReplayResult, SeedStartBoundary, SeedStartReport, SimRealError, SimRealReport, StartRunCommand,
    UnexpectedDiff, UnsupportedTransition, VerificationReadMode, VerifiedTransition,
    REPLAY_ARTIFACT_SCHEMA,
};
pub use slaythedata::{
    import_slaythedata_jsonl_line, import_slaythedata_run_json, import_slaythedata_run_value,
    slaythedata_replay_plan, slaythedata_replay_preflight, SlayTheDataBossRelicChoice,
    SlayTheDataBridgeCommandHint, SlayTheDataBridgeDescriptor, SlayTheDataCampfireChoice,
    SlayTheDataCardName, SlayTheDataCardReward, SlayTheDataCheckpoint, SlayTheDataCheckpointKind,
    SlayTheDataDiagnostic, SlayTheDataDiagnosticSeverity, SlayTheDataEventChoice,
    SlayTheDataFinalObserved, SlayTheDataFloorDecision, SlayTheDataImportError,
    SlayTheDataNamedFloorItem, SlayTheDataPotionFloorDecision, SlayTheDataPreflightReport,
    SlayTheDataPreflightStatus, SlayTheDataPreflightStep, SlayTheDataReplayOrdering,
    SlayTheDataReplayPlan, SlayTheDataReplayPolicy, SlayTheDataReplayStep,
    SlayTheDataReplayStepKind, SlayTheDataRoute, SlayTheDataRunConfig, SlayTheDataRunImport,
    SlayTheDataRunStart, SlayTheDataShopPurchase, SlayTheDataSource, SlayTheDataSourceKind,
    SLAYTHEDATA_IMPORT_SCHEMA_VERSION, SLAYTHEDATA_NORMAL_MAX_FLOOR_REACHED,
};
pub use trace::{
    import_communication_mod_trace, parse_trace_jsonl, parse_trace_jsonl_line,
    CommunicationModTrace, ManualFixture, TraceAction, TraceError, TraceExternalRng, TraceLine,
    TraceMetadata, TraceProfile, TraceRunConfig, TraceState,
};

use std::path::{Path, PathBuf};

/// Repository root (`slay-the-spire/`), relative to this crate manifest.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Simulator workspace root (`slay-the-spire/simulator/`).
pub fn simulator_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Path under `simulator/verification/corpus/`.
pub fn corpus_path(relative: impl AsRef<Path>) -> PathBuf {
    simulator_root().join("verification/corpus").join(relative)
}

/// Trace-id prefixes retained as evidence but excluded from the parity gate.
///
/// The manifest sits beside the corpus root so a corpus directory carries its
/// own quarantine list. A missing manifest quarantines nothing.
pub fn load_quarantine_manifest(corpus_root: &Path) -> Vec<String> {
    let manifest = corpus_root
        .parent()
        .map(|parent| parent.join("quarantine.txt"))
        .unwrap_or_else(|| PathBuf::from("quarantine.txt"));
    let Ok(contents) = std::fs::read_to_string(&manifest) else {
        return Vec::new();
    };
    parse_quarantine_manifest(&contents)
}

/// Parse one `quarantine.txt` body: blank lines and `#` comments are ignored.
pub fn parse_quarantine_manifest(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// True when `trace` (a filename or FIDL prefix) matches a quarantined prefix.
pub fn trace_is_quarantined(trace: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| trace.starts_with(prefix.as_str()))
}

/// Load file contents when present; returns `None` if the path does not exist.
pub fn load_corpus_file(relative: impl AsRef<Path>) -> Option<String> {
    let path = corpus_path(relative);
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quarantine_manifest_ignores_comments_and_blank_lines() {
        let prefixes = parse_quarantine_manifest(
            "# leading comment\n\nFIDL01807\n  FIDL01727  # inline reason\n\n# trailing\n",
        );
        assert_eq!(prefixes, vec!["FIDL01807", "FIDL01727"]);
    }

    #[test]
    fn working_tree_leftover_prefixes_match_filenames() {
        let prefixes = vec!["FIDL00026".to_owned(), "FIDL00034".to_owned()];
        assert!(trace_is_quarantined(
            "FIDL00026-p26-2026-08-21T16-34-12-353Z-6883.jsonl",
            &prefixes
        ));
        assert!(!trace_is_quarantined(
            "FIDL02367-p2367-2026-08-21T11-32-10-140Z-1973297.jsonl",
            &prefixes
        ));
    }

    #[test]
    fn active_quarantine_manifest_lists_fifty_working_tree_leftovers() {
        let prefixes = load_quarantine_manifest(&corpus_path("permanent_traces"));
        assert_eq!(prefixes.len(), 50);
        assert!(prefixes.contains(&"FIDL00026".to_owned()));
        assert!(prefixes.contains(&"FIDL00034".to_owned()));
        assert!(!prefixes.iter().any(|prefix| prefix.starts_with("FIDL02")));
    }
}
