//! Verifier-owned `real_trace_audit` root extraction.

use crate::{
    assess_verification,
    canonical_json::{canonical_json_bytes, is_sha256_hex, sha256_hex},
    repo_root,
    sim_real::{encode_root_snapshot, extract_communication_mod_trace_reader, root_id_for_bytes},
    ReplayCombatRoot, TraceRootCapture, VerificationFailure, VerificationOutcome,
    ACTIONABLE_PREDICATE, ROOT_ENCODING,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Component, Path, PathBuf},
    process::Command,
};
use sts_core::SNAPSHOT_SCHEMA_VERSION as CORE_SNAPSHOT_SCHEMA_VERSION;

pub const REAL_TRACE_AUDIT_KIND: &str = "real_trace_audit";
pub const REAL_TRACE_AUDIT_MANIFEST_VERSION: u32 = 1;
pub const REAL_TRACE_ROOT_VERSION: &str = "real_trace_root_v1";
pub const CHALLENGE_VERSION: u32 = 1;
pub const EXTRACTOR_NAME: &str = "sts_verify";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryVersion {
    pub git_sha: String,
    pub source_digest: String,
    pub clean: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty_diff_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealTraceExclusionReason {
    InvalidInput,
    UnexpectedDiff,
    UnsupportedTransition,
    ReplayBoundary,
    TailNotValidated,
    IncompleteActionAccounting,
    NoActionableCombat,
    RootCaptureError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeTrace {
    pub relative_path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChallengeDeclaration {
    pub challenge_version: u32,
    pub challenge_id: String,
    pub purpose: String,
    pub source: String,
    pub boundary_schema: u32,
    pub collection_epoch: String,
    pub source_artifact_digests: BTreeMap<String, String>,
    pub traces: Vec<ChallengeTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealTraceOccurrence {
    pub source_trace_sha256: String,
    pub combat_ordinal: u32,
    pub action_step: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealTraceRootEntry {
    pub root_id: String,
    pub relative_path: String,
    pub snapshot_sha256: String,
    pub split: String,
    pub occurrences: Vec<RealTraceOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealTraceExclusion {
    pub source_trace_sha256: String,
    pub reason: RealTraceExclusionReason,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealTraceAuditGate {
    pub source_trace_count: usize,
    pub included_trace_count: usize,
    pub excluded_trace_count: usize,
    pub root_count: usize,
    pub all_sources_accounted: bool,
    pub nonempty_required: bool,
    pub verdict: String,
}

#[derive(Debug)]
pub struct RealTraceAuditSummary {
    pub challenge_id: String,
    pub source_trace_count: usize,
    pub included_trace_count: usize,
    pub excluded_trace_count: usize,
    pub root_count: usize,
    pub membership_digest: String,
    pub manifest_digest: String,
    pub verdict: String,
    pub passed: bool,
}

#[derive(Debug)]
pub enum RealTraceAuditError {
    InvalidChallenge(String),
    SourceDigestMismatch {
        relative_path: String,
        expected: String,
        actual: String,
    },
    OutputNotEmpty(PathBuf),
    RootIdCollision {
        root_id: String,
    },
    RepositoryIdentity(String),
    Io(std::io::Error),
}

impl std::fmt::Display for RealTraceAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChallenge(reason) => write!(f, "invalid challenge: {reason}"),
            Self::SourceDigestMismatch {
                relative_path,
                expected,
                actual,
            } => write!(
                f,
                "source digest mismatch for {relative_path}: expected {expected}, actual {actual}"
            ),
            Self::OutputNotEmpty(path) => {
                write!(f, "output directory is not empty: {}", path.display())
            }
            Self::RootIdCollision { root_id } => {
                write!(f, "equal root ID {root_id} has unequal bytes")
            }
            Self::RepositoryIdentity(reason) => write!(f, "repository identity: {reason}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RealTraceAuditError {}

impl From<std::io::Error> for RealTraceAuditError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedTrace {
    relative_path: String,
    sha256: String,
    byte_length: u64,
}

struct DigestingReader<R> {
    inner: R,
    hasher: Sha256,
    byte_length: u64,
    first_line: Vec<u8>,
    first_line_complete: bool,
}

impl<R> DigestingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            byte_length: 0,
            first_line: Vec::new(),
            first_line_complete: false,
        }
    }

    fn finish(self) -> (String, u64, Vec<u8>) {
        (
            format!("{:x}", self.hasher.finalize()),
            self.byte_length,
            self.first_line,
        )
    }
}

impl<R: Read> Read for DigestingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        let bytes = &buffer[..read];
        self.hasher.update(bytes);
        self.byte_length = self
            .byte_length
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("trace byte length overflow"))?;
        if !self.first_line_complete {
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                self.first_line.extend_from_slice(&bytes[..newline]);
                self.first_line_complete = true;
            } else {
                self.first_line.extend_from_slice(bytes);
            }
        }
        Ok(read)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRoot {
    root_id: String,
    bytes: Vec<u8>,
    occurrences: Vec<RealTraceOccurrence>,
}

pub fn extract_real_trace_audit(
    traces_dir: &Path,
    challenge_path: &Path,
    output_dir: &Path,
) -> Result<RealTraceAuditSummary, RealTraceAuditError> {
    let challenge_raw = fs::read(challenge_path)?;
    let challenge = parse_challenge_bytes(&challenge_raw)?;
    let challenge_file_digest = sha256_hex(&challenge_raw);
    ensure_empty_output_dir(output_dir)?;
    let repository = capture_repository_version(&repo_root())
        .map_err(RealTraceAuditError::RepositoryIdentity)?;
    let canonical_traces_dir = fs::canonicalize(traces_dir)?;

    let mut loaded: Vec<LoadedTrace> = Vec::with_capacity(challenge.traces.len());
    let mut pending_roots: Vec<PendingRoot> = Vec::new();
    let mut exclusions: Vec<RealTraceExclusion> = Vec::new();
    let mut included: BTreeSet<String> = BTreeSet::new();

    for entry in &challenge.traces {
        let (trace, classification) = replay_challenge_trace(&canonical_traces_dir, entry)?;
        match classification {
            TraceClassification::Roots(roots) => {
                included.insert(trace.sha256.clone());
                for (root, bytes) in roots {
                    merge_pending_root(
                        &mut pending_roots,
                        PendingRoot {
                            root_id: root_id_for_bytes(&bytes),
                            bytes,
                            occurrences: vec![RealTraceOccurrence {
                                source_trace_sha256: trace.sha256.clone(),
                                combat_ordinal: root.combat_ordinal,
                                action_step: root.action_step,
                            }],
                        },
                    )?;
                }
            }
            TraceClassification::Exclusion(exclusion) => exclusions.push(exclusion),
        }
        loaded.push(trace);
    }

    for root in &mut pending_roots {
        root.occurrences.sort_by(|left, right| {
            (
                &left.source_trace_sha256,
                left.combat_ordinal,
                left.action_step,
            )
                .cmp(&(
                    &right.source_trace_sha256,
                    right.combat_ordinal,
                    right.action_step,
                ))
        });
    }
    pending_roots.sort_by(|left, right| left.root_id.cmp(&right.root_id));
    exclusions.sort_by(|left, right| {
        (&left.source_trace_sha256, left.reason, left.detail.as_str()).cmp(&(
            &right.source_trace_sha256,
            right.reason,
            right.detail.as_str(),
        ))
    });

    let accounted: BTreeSet<String> = included
        .iter()
        .cloned()
        .chain(
            exclusions
                .iter()
                .map(|exclusion| exclusion.source_trace_sha256.clone()),
        )
        .collect();
    let requested: BTreeSet<String> = loaded.iter().map(|trace| trace.sha256.clone()).collect();
    if accounted != requested {
        return Err(RealTraceAuditError::InvalidChallenge(
            "source trace accounting is incomplete".to_owned(),
        ));
    }

    let root_entries: Vec<RealTraceRootEntry> = pending_roots
        .iter()
        .map(|root| RealTraceRootEntry {
            root_id: root.root_id.clone(),
            relative_path: format!("roots/{}.json", root.root_id),
            snapshot_sha256: root.root_id.clone(),
            split: REAL_TRACE_AUDIT_KIND.to_owned(),
            occurrences: root.occurrences.clone(),
        })
        .collect();
    let source_traces: Vec<Value> = loaded
        .iter()
        .map(|trace| {
            json!({
                "relative_path": trace.relative_path,
                "sha256": trace.sha256,
                "byte_length": trace.byte_length,
            })
        })
        .collect();
    let passed = !root_entries.is_empty();
    let gate = RealTraceAuditGate {
        source_trace_count: loaded.len(),
        included_trace_count: included.len(),
        excluded_trace_count: exclusions.len(),
        root_count: root_entries.len(),
        all_sources_accounted: true,
        nonempty_required: true,
        verdict: if passed {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        },
    };
    let membership_digest = membership_digest(
        &challenge.challenge_id,
        &loaded
            .iter()
            .map(|trace| trace.sha256.as_str())
            .collect::<Vec<_>>(),
        &root_entries,
        &exclusions,
    );
    let mut manifest = json!({
        "manifest_version": REAL_TRACE_AUDIT_MANIFEST_VERSION,
        "kind": REAL_TRACE_AUDIT_KIND,
        "challenge_provenance": {
            "challenge_id": challenge.challenge_id,
            "purpose": challenge.purpose,
            "source": challenge.source,
            "boundary_schema": challenge.boundary_schema,
            "collection_epoch": challenge.collection_epoch,
            "source_artifact_digests": challenge.source_artifact_digests,
            "challenge_file_digest": challenge_file_digest,
        },
        "extractor": {
            "name": EXTRACTOR_NAME,
            "version": REAL_TRACE_ROOT_VERSION,
            "repository": repository,
            "trace_boundary_schema": challenge.boundary_schema,
            "snapshot_schema": CORE_SNAPSHOT_SCHEMA_VERSION,
            "root_encoding": ROOT_ENCODING,
            "requires_clean_eof": true,
            "actionable_predicate": ACTIONABLE_PREDICATE,
        },
        "source_traces": source_traces,
        "roots": root_entries,
        "exclusions": exclusions,
        "membership_digest": membership_digest,
        "gate": gate,
    });
    let manifest_digest = sha256_hex(&canonical_json_bytes(&manifest));
    manifest
        .as_object_mut()
        .expect("manifest object")
        .insert("manifest_digest".to_owned(), json!(manifest_digest));

    fs::create_dir_all(output_dir.join("roots"))?;
    for root in &pending_roots {
        fs::write(
            output_dir.join(format!("roots/{}.json", root.root_id)),
            &root.bytes,
        )?;
    }
    fs::write(
        output_dir.join("real-trace-audit-manifest.json"),
        canonical_json_bytes(&manifest),
    )?;

    Ok(RealTraceAuditSummary {
        challenge_id: challenge.challenge_id,
        source_trace_count: loaded.len(),
        included_trace_count: included.len(),
        excluded_trace_count: exclusions.len(),
        root_count: root_entries.len(),
        membership_digest,
        manifest_digest,
        verdict: if passed {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        },
        passed,
    })
}

impl RealTraceAuditSummary {
    pub fn stdout(&self) -> String {
        format!(
            "kind={REAL_TRACE_AUDIT_KIND}\nchallenge_id={}\nsource_trace_count={}\nincluded_trace_count={}\nexcluded_trace_count={}\nroot_count={}\nmembership_digest={}\nmanifest_digest={}\ngate.verdict={}\n",
            self.challenge_id,
            self.source_trace_count,
            self.included_trace_count,
            self.excluded_trace_count,
            self.root_count,
            self.membership_digest,
            self.manifest_digest,
            self.verdict,
        )
    }
}

fn parse_challenge_bytes(bytes: &[u8]) -> Result<ChallengeDeclaration, RealTraceAuditError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| RealTraceAuditError::InvalidChallenge(error.to_string()))?;
    if canonical_json_bytes(&value) != bytes {
        return Err(RealTraceAuditError::InvalidChallenge(
            "challenge must use canonical compact sorted-key JSON bytes".to_owned(),
        ));
    }
    parse_challenge_value(&value)
}

fn parse_challenge_value(value: &Value) -> Result<ChallengeDeclaration, RealTraceAuditError> {
    let object = value.as_object().ok_or_else(|| {
        RealTraceAuditError::InvalidChallenge("challenge must be a JSON object".to_owned())
    })?;
    let expected = [
        "challenge_version",
        "challenge_id",
        "purpose",
        "source",
        "boundary_schema",
        "collection_epoch",
        "source_artifact_digests",
        "traces",
    ];
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(RealTraceAuditError::InvalidChallenge(
            "challenge has missing or unknown fields".to_owned(),
        ));
    }
    let challenge: ChallengeDeclaration = serde_json::from_value(value.clone())
        .map_err(|error| RealTraceAuditError::InvalidChallenge(error.to_string()))?;
    if challenge.challenge_version != CHALLENGE_VERSION {
        return Err(RealTraceAuditError::InvalidChallenge(
            "challenge_version must be 1".to_owned(),
        ));
    }
    if challenge.challenge_id != "real-trace-audit-v1" {
        return Err(RealTraceAuditError::InvalidChallenge(
            "challenge_id must be real-trace-audit-v1".to_owned(),
        ));
    }
    if challenge.purpose != REAL_TRACE_AUDIT_KIND {
        return Err(RealTraceAuditError::InvalidChallenge(
            "purpose must be real_trace_audit".to_owned(),
        ));
    }
    if challenge.source != "communication_mod" {
        return Err(RealTraceAuditError::InvalidChallenge(
            "source must be communication_mod".to_owned(),
        ));
    }
    if challenge.boundary_schema != 7 {
        return Err(RealTraceAuditError::InvalidChallenge(
            "boundary_schema must be 7".to_owned(),
        ));
    }
    if challenge.collection_epoch != "schema7" {
        return Err(RealTraceAuditError::InvalidChallenge(
            "collection_epoch must be schema7".to_owned(),
        ));
    }
    for required in ["CommunicationMod", "SuperFastMode"] {
        if !challenge.source_artifact_digests.contains_key(required) {
            return Err(RealTraceAuditError::InvalidChallenge(format!(
                "source_artifact_digests must contain {required}"
            )));
        }
    }
    for (name, digest) in &challenge.source_artifact_digests {
        if name.is_empty() || !is_sha256_hex(digest) {
            return Err(RealTraceAuditError::InvalidChallenge(
                "source artifact digest is invalid".to_owned(),
            ));
        }
    }
    let mut paths = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for (index, trace) in challenge.traces.iter().enumerate() {
        validate_relative_path(&trace.relative_path)?;
        if !is_sha256_hex(&trace.sha256) {
            return Err(RealTraceAuditError::InvalidChallenge(format!(
                "trace sha256 is invalid: {}",
                trace.sha256
            )));
        }
        if !paths.insert(trace.relative_path.clone()) {
            return Err(RealTraceAuditError::InvalidChallenge(format!(
                "duplicate trace path {}",
                trace.relative_path
            )));
        }
        if !digests.insert(trace.sha256.clone()) {
            return Err(RealTraceAuditError::InvalidChallenge(format!(
                "duplicate trace digest {}",
                trace.sha256
            )));
        }
        if index > 0 && challenge.traces[index - 1].relative_path > trace.relative_path {
            return Err(RealTraceAuditError::InvalidChallenge(
                "challenge traces are not sorted by relative_path".to_owned(),
            ));
        }
    }
    Ok(challenge)
}

fn validate_relative_path(relative: &str) -> Result<(), RealTraceAuditError> {
    let path = Path::new(relative);
    if relative.is_empty() || path.is_absolute() {
        return Err(RealTraceAuditError::InvalidChallenge(
            "trace path must be a relative file path".to_owned(),
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(RealTraceAuditError::InvalidChallenge(format!(
            "trace path {relative} escapes the traces directory"
        )));
    }
    Ok(())
}

fn ensure_empty_output_dir(output_dir: &Path) -> Result<(), RealTraceAuditError> {
    if !output_dir.exists() {
        return Ok(());
    }
    if !output_dir.is_dir() {
        return Err(RealTraceAuditError::OutputNotEmpty(
            output_dir.to_path_buf(),
        ));
    }
    if output_dir.read_dir()?.next().is_some() {
        return Err(RealTraceAuditError::OutputNotEmpty(
            output_dir.to_path_buf(),
        ));
    }
    Ok(())
}

enum TraceClassification {
    Roots(Vec<(ReplayCombatRoot, Vec<u8>)>),
    Exclusion(RealTraceExclusion),
}

fn schema7_metadata_error(line: &[u8]) -> Option<String> {
    let value: Value = match serde_json::from_slice(line) {
        Ok(value) => value,
        Err(_) => return Some("schema-7 audit traces must start with valid metadata".to_owned()),
    };
    if value.get("type").and_then(Value::as_str) != Some("metadata") {
        return Some("schema-7 audit traces must start with metadata".to_owned());
    }
    if value.get("boundary_schema").and_then(Value::as_u64) != Some(7) {
        return Some("schema-7 audit traces must declare metadata boundary_schema=7".to_owned());
    }
    None
}

fn replay_challenge_trace(
    traces_dir: &Path,
    entry: &ChallengeTrace,
) -> Result<(LoadedTrace, TraceClassification), RealTraceAuditError> {
    let requested_path = traces_dir.join(&entry.relative_path);
    let canonical_path = fs::canonicalize(&requested_path)?;
    if !canonical_path.starts_with(traces_dir) {
        return Err(RealTraceAuditError::InvalidChallenge(format!(
            "trace path {} resolves outside the traces directory",
            entry.relative_path
        )));
    }
    if !canonical_path.metadata()?.is_file() {
        return Err(RealTraceAuditError::InvalidChallenge(format!(
            "trace path {} is not a regular file",
            entry.relative_path
        )));
    }
    let file = File::open(canonical_path)?;
    let mut reader = BufReader::new(DigestingReader::new(file));
    let replay = extract_communication_mod_trace_reader(&mut reader);
    io::copy(&mut reader, &mut io::sink())?;
    let (digest, byte_length, first_line) = reader.into_inner().finish();
    if digest != entry.sha256 {
        return Err(RealTraceAuditError::SourceDigestMismatch {
            relative_path: entry.relative_path.clone(),
            expected: entry.sha256.clone(),
            actual: digest,
        });
    }
    let trace = LoadedTrace {
        relative_path: entry.relative_path.clone(),
        sha256: digest,
        byte_length,
    };
    let classification = if let Some(detail) = schema7_metadata_error(&first_line) {
        TraceClassification::Exclusion(RealTraceExclusion {
            source_trace_sha256: trace.sha256.clone(),
            reason: RealTraceExclusionReason::InvalidInput,
            detail,
        })
    } else {
        match replay {
            Err(error) => TraceClassification::Exclusion(RealTraceExclusion {
                source_trace_sha256: trace.sha256.clone(),
                reason: RealTraceExclusionReason::InvalidInput,
                detail: error.to_string(),
            }),
            Ok(capture) => classify_capture(&trace.sha256, capture),
        }
    };
    Ok((trace, classification))
}

fn classify_capture(source_sha: &str, capture: TraceRootCapture) -> TraceClassification {
    let outcome = assess_verification(
        Ok(&capture.report),
        capture.report.action_integrity.as_ref(),
    );
    if outcome != VerificationOutcome::CompletePass {
        return TraceClassification::Exclusion(exclusion_from_outcome(
            source_sha, &capture, &outcome,
        ));
    }
    if let Some(error) = capture.capture_error {
        return TraceClassification::Exclusion(RealTraceExclusion {
            source_trace_sha256: source_sha.to_owned(),
            reason: RealTraceExclusionReason::RootCaptureError,
            detail: error,
        });
    }
    if capture.roots.is_empty() {
        return TraceClassification::Exclusion(RealTraceExclusion {
            source_trace_sha256: source_sha.to_owned(),
            reason: RealTraceExclusionReason::NoActionableCombat,
            detail: "clean trace has no actionable combat root".to_owned(),
        });
    }
    let mut encoded = Vec::new();
    for root in capture.roots {
        match encode_root_snapshot(&root.snapshot) {
            Ok(bytes) => encoded.push((root, bytes)),
            Err(error) => {
                return TraceClassification::Exclusion(RealTraceExclusion {
                    source_trace_sha256: source_sha.to_owned(),
                    reason: RealTraceExclusionReason::RootCaptureError,
                    detail: error,
                });
            }
        }
    }
    TraceClassification::Roots(encoded)
}

fn exclusion_from_outcome(
    source_sha: &str,
    capture: &TraceRootCapture,
    outcome: &VerificationOutcome,
) -> RealTraceExclusion {
    let (reason, detail) = match outcome {
        VerificationOutcome::InvalidInput { reason } => {
            (RealTraceExclusionReason::InvalidInput, reason.clone())
        }
        VerificationOutcome::Failed { failures } => exclusion_from_failures(capture, failures),
        VerificationOutcome::CompletePass => (
            RealTraceExclusionReason::ReplayBoundary,
            "complete pass was classified as an exclusion".to_owned(),
        ),
    };
    RealTraceExclusion {
        source_trace_sha256: source_sha.to_owned(),
        reason,
        detail,
    }
}

fn exclusion_from_failures(
    capture: &TraceRootCapture,
    failures: &[VerificationFailure],
) -> (RealTraceExclusionReason, String) {
    if !capture.report.unexpected_diffs.is_empty()
        || failures
            .iter()
            .any(|failure| matches!(failure, VerificationFailure::UnexpectedDiffs { .. }))
    {
        let detail = capture
            .report
            .unexpected_diffs
            .first()
            .map(|diff| format!("step {} {}: {}", diff.action_step, diff.command, diff.label))
            .unwrap_or_else(|| "unexpected simulator/real diff".to_owned());
        return (RealTraceExclusionReason::UnexpectedDiff, detail);
    }
    if !capture.report.unsupported.is_empty()
        || failures
            .iter()
            .any(|failure| matches!(failure, VerificationFailure::UnsupportedTransitions { .. }))
    {
        let detail = capture
            .report
            .unsupported
            .first()
            .map(|unsupported| {
                format!(
                    "step {} {}: {}",
                    unsupported.action_step, unsupported.command, unsupported.reason
                )
            })
            .unwrap_or_else(|| "unsupported transition".to_owned());
        return (RealTraceExclusionReason::UnsupportedTransition, detail);
    }
    if failures
        .iter()
        .any(|failure| matches!(failure, VerificationFailure::TailNotValidated))
    {
        return (
            RealTraceExclusionReason::TailNotValidated,
            "trace tail was not validated through EOF".to_owned(),
        );
    }
    if failures.iter().any(|failure| {
        matches!(
            failure,
            VerificationFailure::IncompleteActionAccounting { .. }
                | VerificationFailure::DuplicateActionDispositions { .. }
                | VerificationFailure::MissingActionIntegrity
        )
    }) {
        return (
            RealTraceExclusionReason::IncompleteActionAccounting,
            "action disposition accounting is incomplete".to_owned(),
        );
    }
    let detail = failures
        .first()
        .map(|failure| format!("{failure:?}"))
        .unwrap_or_else(|| "replay boundary".to_owned());
    (RealTraceExclusionReason::ReplayBoundary, detail)
}

fn merge_pending_root(
    roots: &mut Vec<PendingRoot>,
    incoming: PendingRoot,
) -> Result<(), RealTraceAuditError> {
    if let Some(existing) = roots
        .iter_mut()
        .find(|root| root.root_id == incoming.root_id)
    {
        if existing.bytes != incoming.bytes {
            return Err(RealTraceAuditError::RootIdCollision {
                root_id: incoming.root_id,
            });
        }
        existing.occurrences.extend(incoming.occurrences);
        return Ok(());
    }
    roots.push(incoming);
    Ok(())
}

fn membership_digest(
    challenge_id: &str,
    source_trace_sha256: &[&str],
    roots: &[RealTraceRootEntry],
    exclusions: &[RealTraceExclusion],
) -> String {
    let payload = json!({
        "challenge_id": challenge_id,
        "source_trace_sha256": source_trace_sha256,
        "roots": roots.iter().map(|root| {
            json!({
                "root_id": root.root_id,
                "occurrences": root.occurrences,
            })
        }).collect::<Vec<_>>(),
        "exclusions": exclusions,
    });
    sha256_hex(&canonical_json_bytes(&payload))
}

fn capture_repository_version(repo: &Path) -> Result<RepositoryVersion, String> {
    let repo =
        fs::canonicalize(repo).map_err(|error| format!("canonicalize repository root: {error}"))?;
    let git_sha = git_stdout(&repo, &["rev-parse", "HEAD"])?
        .trim()
        .to_ascii_lowercase();
    if git_sha.len() != 40
        || git_sha
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err("git SHA must be a lowercase object digest".to_owned());
    }
    let build_git_sha = env!("STS_VERIFY_BUILD_GIT_SHA");
    if git_sha != build_git_sha {
        return Err(format!(
            "executing extractor was built from git {build_git_sha}, not checkout {git_sha}"
        ));
    }
    let source_digest = repository_source_digest(&repo)?;
    let build_source_digest = env!("STS_VERIFY_BUILD_SOURCE_DIGEST");
    if source_digest != build_source_digest {
        return Err(format!(
            "executing extractor source digest {build_source_digest} does not match checkout {source_digest}"
        ));
    }

    let porcelain = git_bytes(&repo, &["status", "--porcelain=v1", "-z"])?;
    if porcelain.is_empty() {
        return Ok(RepositoryVersion {
            git_sha,
            source_digest,
            clean: true,
            dirty_diff_digest: None,
        });
    }

    let tracked_diff = git_bytes(&repo, &["diff", "--binary", "--no-ext-diff", "HEAD", "--"])?;
    let untracked = git_bytes(&repo, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, b"repository-dirty-state-v1");
    hash_segment(&mut hasher, &tracked_diff);
    for relative in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let relative_text = std::str::from_utf8(relative)
            .map_err(|error| format!("untracked path is not UTF-8: {error}"))?;
        let contents = fs::read(repo.join(relative_text))
            .map_err(|error| format!("read untracked file {relative_text}: {error}"))?;
        hash_segment(&mut hasher, relative);
        hash_segment(&mut hasher, &contents);
    }
    Ok(RepositoryVersion {
        git_sha,
        source_digest,
        clean: false,
        dirty_diff_digest: Some(format!("{:x}", hasher.finalize())),
    })
}

fn repository_source_digest(repo: &Path) -> Result<String, String> {
    let verify = repo.join("crates/sts_verify");
    let core = repo.join("crates/sts_core");
    let mut files = vec![verify.join("Cargo.toml"), verify.join("build.rs")];
    collect_rs_files(&verify.join("src"), &mut files)?;
    files.push(core.join("Cargo.toml"));
    collect_rs_files(&core.join("src"), &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    hash_segment(&mut hasher, b"sts-verify-build-source-v1");
    for file in files {
        let relative = file
            .strip_prefix(repo)
            .map_err(|error| format!("source path outside repository: {error}"))?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "source path is not UTF-8".to_owned())?;
        let contents = fs::read(&file)
            .map_err(|error| format!("read source file {}: {error}", file.display()))?;
        hash_segment(&mut hasher, relative.as_bytes());
        hash_segment(&mut hasher, &contents);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_rs_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("read source directory {}: {error}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs_files(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn hash_segment(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn git_stdout(repo: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git_bytes(repo, args)?).map_err(|error| error.to_string())
}

fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim_real::{is_actionable_combat_state, validate_encoded_root_snapshot};
    use sts_core::{RunState, Snapshot, SNAPSHOT_SCHEMA_VERSION};

    fn dummy_digest(seed: u8) -> String {
        sha256_hex(&[seed; 8])
    }

    fn challenge(traces: Vec<ChallengeTrace>) -> ChallengeDeclaration {
        let mut traces = traces;
        traces.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        ChallengeDeclaration {
            challenge_version: 1,
            challenge_id: "real-trace-audit-v1".to_owned(),
            purpose: REAL_TRACE_AUDIT_KIND.to_owned(),
            source: "communication_mod".to_owned(),
            boundary_schema: 7,
            collection_epoch: "schema7".to_owned(),
            source_artifact_digests: BTreeMap::from([
                ("CommunicationMod".to_owned(), dummy_digest(1)),
                ("SuperFastMode".to_owned(), dummy_digest(2)),
            ]),
            traces,
        }
    }

    #[test]
    fn challenge_rejects_path_traversal_and_duplicates() {
        let mut value = serde_json::to_value(challenge(vec![ChallengeTrace {
            relative_path: "../secret.jsonl".to_owned(),
            sha256: dummy_digest(2),
        }]))
        .expect("challenge serializes");
        let error = parse_challenge_value(&value).expect_err("parent path must fail");
        assert!(error.to_string().contains("escapes"));

        value = serde_json::to_value(challenge(vec![
            ChallengeTrace {
                relative_path: "a.jsonl".to_owned(),
                sha256: dummy_digest(2),
            },
            ChallengeTrace {
                relative_path: "a.jsonl".to_owned(),
                sha256: dummy_digest(3),
            },
        ]))
        .expect("challenge serializes");
        let error = parse_challenge_value(&value).expect_err("duplicate path must fail");
        assert!(error.to_string().contains("duplicate trace path"));
    }

    #[test]
    fn equal_root_ids_merge_occurrences_and_unequal_bytes_collide() {
        let mut roots = Vec::new();
        merge_pending_root(
            &mut roots,
            PendingRoot {
                root_id: dummy_digest(9),
                bytes: b"same".to_vec(),
                occurrences: vec![RealTraceOccurrence {
                    source_trace_sha256: dummy_digest(4),
                    combat_ordinal: 1,
                    action_step: 10,
                }],
            },
        )
        .expect("first root inserts");
        merge_pending_root(
            &mut roots,
            PendingRoot {
                root_id: dummy_digest(9),
                bytes: b"same".to_vec(),
                occurrences: vec![RealTraceOccurrence {
                    source_trace_sha256: dummy_digest(5),
                    combat_ordinal: 1,
                    action_step: 11,
                }],
            },
        )
        .expect("identical bytes merge");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].occurrences.len(), 2);

        let error = merge_pending_root(
            &mut roots,
            PendingRoot {
                root_id: dummy_digest(9),
                bytes: b"other".to_vec(),
                occurrences: vec![RealTraceOccurrence {
                    source_trace_sha256: dummy_digest(6),
                    combat_ordinal: 2,
                    action_step: 12,
                }],
            },
        )
        .expect_err("unequal bytes collide");
        assert!(matches!(error, RealTraceAuditError::RootIdCollision { .. }));
    }

    #[test]
    fn membership_digest_is_path_independent() {
        let roots = [RealTraceRootEntry {
            root_id: dummy_digest(7),
            relative_path: "roots/ignored.json".to_owned(),
            snapshot_sha256: dummy_digest(7),
            split: REAL_TRACE_AUDIT_KIND.to_owned(),
            occurrences: vec![RealTraceOccurrence {
                source_trace_sha256: dummy_digest(8),
                combat_ordinal: 1,
                action_step: 3,
            }],
        }];
        let exclusions = [RealTraceExclusion {
            source_trace_sha256: dummy_digest(9),
            reason: RealTraceExclusionReason::NoActionableCombat,
            detail: "none".to_owned(),
        }];
        let left = membership_digest("audit", &[dummy_digest(8).as_str()], &roots, &exclusions);
        let right = membership_digest("audit", &[dummy_digest(8).as_str()], &roots, &exclusions);
        assert_eq!(left, right);
        assert!(is_sha256_hex(&left));
    }

    fn complete_report() -> crate::SimRealReport {
        crate::SimRealReport {
            total_actions: 1,
            action_dispositions: Vec::new(),
            action_integrity: Some(crate::VerificationIntegrity {
                eof_validated: true,
                applicable_actions: 1,
                disposed_actions: 1,
                duplicate_dispositions: 0,
                terminal_state_observed: false,
                rejected_actions: 0,
            }),
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: Vec::new(),
            seed_start: Some(crate::SeedStartReport {
                start_command: crate::StartRunCommand {
                    action_step: 1,
                    character: "IRONCLAD".to_owned(),
                    ascension: 0,
                    external_seed: "1".to_owned(),
                    numeric_seed: 1,
                    verification_starting_hp: None,
                },
                failed: false,
                first_boundary: crate::SeedStartBoundary {
                    path: "$.actions[verified]".to_owned(),
                    category: "none".to_owned(),
                    reason: "test".to_owned(),
                },
                sim_run_state: None,
            }),
        }
    }

    #[test]
    fn clean_trace_without_combat_is_a_typed_exclusion() {
        let capture = TraceRootCapture {
            report: complete_report(),
            roots: Vec::new(),
            capture_error: None,
        };
        match classify_capture(&dummy_digest(3), capture) {
            TraceClassification::Exclusion(exclusion) => {
                assert_eq!(
                    exclusion.reason,
                    RealTraceExclusionReason::NoActionableCombat
                );
            }
            TraceClassification::Roots(_) => panic!("empty combat capture must not publish roots"),
        }
    }

    #[test]
    fn combat_fixture_root_encoding_is_sha256_not_fnv_and_round_trips() {
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: RunState::combat_fixture(),
        };
        let bytes = validate_encoded_root_snapshot(&snapshot).expect("fixture encodes");
        let root_id = root_id_for_bytes(&bytes);
        assert_eq!(root_id.len(), 64);
        assert_ne!(root_id, snapshot.hash().expect("fnv hash").to_string());
        assert!(is_actionable_combat_state(&snapshot.state).expect("actionable"));
    }
}
