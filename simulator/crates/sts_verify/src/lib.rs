#![forbid(unsafe_code)]
#![doc = "Trace formats, canonical diffs, and fixture loaders for simulator verification."]

pub mod diff;
pub mod importer;
pub mod m22;
pub mod normalize;
pub mod seed;
pub mod sim_real;
pub mod trace;

pub use diff::canonical_diff;
pub use importer::{observations_from_trace, ImportedTraceStep};
pub use m22::{
    verify_m22_encounter_spawn_prefix, M22EncounterEntry, M22EncounterMismatch, M22EncounterReport,
};
pub use normalize::{
    normalize_combat_state, normalize_communication_mod_message, CanonicalCombatObservation,
    CanonicalMonsterObservation, CanonicalRunObservation,
};
pub use seed::{sts_seed_string_to_long, STS_SEED_ALPHABET};
pub use sim_real::{
    verify_communication_mod_trace, verify_communication_mod_trace_with_mode,
    verify_seed_start_communication_mod_trace, RngBoundary, SeedStartBoundary, SeedStartReport,
    SimRealError, SimRealReport, StartRunCommand, UnexpectedDiff, UnsupportedTransition,
    VerificationMode, VerifiedTransition,
};
pub use trace::{
    import_communication_mod_trace, parse_trace_jsonl, CommunicationModTrace, ManualFixture,
    TraceAction, TraceLine, TraceMetadata, TraceState,
};

use std::path::{Path, PathBuf};

/// Repository root (`slay-the-spire/`), relative to this crate manifest.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Path under `verification/corpus/`.
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
