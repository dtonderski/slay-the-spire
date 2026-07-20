#![forbid(unsafe_code)]
#![doc = "Trace formats, canonical diffs, and fixture loaders for simulator verification."]

pub mod diff;
pub mod importer;
pub mod m22;
pub mod minimize;
pub mod normalize;
pub mod outcome;
pub mod seed;
pub mod sim_real;
pub mod slaythedata;
pub mod trace;

pub use diff::canonical_diff;
pub use importer::{observations_from_trace, ImportedTraceStep};
pub use m22::{
    verify_m22_encounter_spawn_prefix, M22EncounterEntry, M22EncounterMismatch, M22EncounterReport,
};
pub use minimize::{
    minimize_communication_mod_trace, serialize_communication_mod_trace, MinimizeError,
    MinimizeFailureKind, MinimizeReport,
};
pub use normalize::{
    normalize_combat_state, normalize_communication_mod_message, CanonicalCombatObservation,
    CanonicalMonsterObservation, CanonicalRunObservation,
};
pub use outcome::{
    assess_verification, ExpectedBoundary, RetainedPrefixEndpoint, VerificationCorpusEntry,
    VerificationCorpusManifest, VerificationExpectation, VerificationFailure,
    VerificationIntegrity, VerificationOutcome, VERIFICATION_CORPUS_MANIFEST_SCHEMA,
};
pub use seed::{
    sts_seed_long_to_string, sts_seed_string_to_long, try_sts_seed_string_to_long,
    STS_SEED_ALPHABET,
};
pub use sim_real::{
    verify_communication_mod_trace, verify_seed_start_communication_mod_trace, ActionDisposition,
    ActionDispositionKind, RngBoundary, SeedStartBoundary, SeedStartReport, SimRealError,
    SimRealReport, StartRunCommand, UnexpectedDiff, UnsupportedTransition, VerifiedTransition,
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
    import_communication_mod_trace, parse_trace_jsonl, CommunicationModTrace, ManualFixture,
    TraceAction, TraceError, TraceLine, TraceMetadata, TraceState,
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

/// Load file contents when present; returns `None` if the path does not exist.
pub fn load_corpus_file(relative: impl AsRef<Path>) -> Option<String> {
    let path = corpus_path(relative);
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}
