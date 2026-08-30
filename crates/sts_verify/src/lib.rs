#![forbid(unsafe_code)]
#![doc = "Trace formats, canonical diffs, and fixture loaders for simulator verification."]

pub mod canonical_json;
pub mod diff;
pub mod minimize;
pub mod outcome;
pub mod real_trace_audit;
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
pub use real_trace_audit::{extract_real_trace_audit, RealTraceAuditError, RealTraceAuditSummary};
pub use sim_real::{
    extract_communication_mod_trace_reader, replay_communication_mod_trace,
    replay_communication_mod_trace_reader, verify_communication_mod_trace,
    verify_communication_mod_trace_diagnostic_reader, verify_communication_mod_trace_reader,
    verify_seed_start_communication_mod_trace, ActionDisposition, ActionDispositionKind,
    ReplayCheckpoint, ReplayCheckpointState, ReplayCombatRoot, ReplayResult, SeedStartBoundary,
    SeedStartReport, SimRealError, SimRealReport, StartRunCommand, TraceRootCapture,
    UnexpectedDiff, UnsupportedTransition, VerificationReadMode, VerifiedTransition,
    ACTIONABLE_PREDICATE, REPLAY_ARTIFACT_SCHEMA, ROOT_ENCODING,
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Path under the repository-owned `verification/corpus/` directory.
pub fn corpus_path(relative: impl AsRef<Path>) -> PathBuf {
    repo_root().join("verification/corpus").join(relative)
}

/// Load file contents when present; returns `None` if the path does not exist.
pub fn load_corpus_file(relative: impl AsRef<Path>) -> Option<String> {
    let path = corpus_path(relative);
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}
