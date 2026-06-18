#![forbid(unsafe_code)]
#![doc = "Trace formats, canonical diffs, and fixture loaders for simulator verification."]

pub mod diff;
pub mod trace;

pub use diff::canonical_diff;
pub use trace::{parse_trace_jsonl, ManualFixture, TraceLine, TraceMetadata, TraceState};

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
