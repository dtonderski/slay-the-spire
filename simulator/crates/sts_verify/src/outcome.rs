//! Typed verification outcomes and the evidence required to claim them.
//!
//! Default success is **clean through EOF**: every verifiable transition in the
//! file matches, with first-boundary `category=none`. Incomplete / non-terminal
//! traces pass. Optional strict terminal mode is a flag only.

use crate::{SeedStartBoundary, SimRealError, SimRealReport};
use serde::{Deserialize, Serialize};

/// Options for [`assess_verification`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssessmentOptions {
    /// When true, require `terminal_state_observed` (full game-over run).
    pub require_terminal: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationIntegrity {
    /// True only when the complete JSONL input was parsed and validated.
    pub eof_validated: bool,
    pub applicable_actions: usize,
    pub disposed_actions: usize,
    pub duplicate_dispositions: usize,
    pub terminal_state_observed: bool,
    pub rejected_actions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum VerificationOutcome {
    /// Clean through EOF (and terminal when [`AssessmentOptions::require_terminal`]).
    CompletePass,
    InvalidInput {
        reason: String,
    },
    Failed {
        failures: Vec<VerificationFailure>,
    },
}

impl VerificationOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::CompletePass)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum VerificationFailure {
    UnexpectedDiffs {
        count: usize,
    },
    UnsupportedTransitions {
        count: usize,
    },
    MissingSeedStartReport,
    InconsistentBoundaryStatus {
        failed: bool,
        boundary: SeedStartBoundary,
    },
    UnexpectedBoundary {
        boundary: SeedStartBoundary,
    },
    /// Only emitted when [`AssessmentOptions::require_terminal`] is set.
    CompleteTraceNotTerminal,
    CompleteTraceHasRejectedActions {
        count: usize,
    },
    MissingActionIntegrity,
    TailNotValidated,
    IncompleteActionAccounting {
        applicable_actions: usize,
        disposed_actions: usize,
    },
    DuplicateActionDispositions {
        count: usize,
    },
}

/// Assess a seed-start / parity report under clean-through-EOF rules.
pub fn assess_verification(
    result: Result<&SimRealReport, &SimRealError>,
    integrity: Option<&VerificationIntegrity>,
) -> VerificationOutcome {
    assess_verification_with_options(result, integrity, AssessmentOptions::default())
}

/// Assess with optional strict terminal requirement.
pub fn assess_verification_with_options(
    result: Result<&SimRealReport, &SimRealError>,
    integrity: Option<&VerificationIntegrity>,
    options: AssessmentOptions,
) -> VerificationOutcome {
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            return VerificationOutcome::InvalidInput {
                reason: error.to_string(),
            };
        }
    };

    let mut failures = Vec::new();
    if !report.unexpected_diffs.is_empty() {
        failures.push(VerificationFailure::UnexpectedDiffs {
            count: report.unexpected_diffs.len(),
        });
    }
    let (actual_boundary, boundary_status_consistent) = if let Some(seed_start) = &report.seed_start
    {
        let boundary_failed = seed_start.first_boundary.category != "none";
        let boundary_status_consistent = seed_start.failed == boundary_failed;
        if !boundary_status_consistent {
            failures.push(VerificationFailure::InconsistentBoundaryStatus {
                failed: seed_start.failed,
                boundary: seed_start.first_boundary.clone(),
            });
        }
        (
            Some(seed_start.first_boundary.clone()),
            boundary_status_consistent,
        )
    } else {
        failures.push(VerificationFailure::MissingSeedStartReport);
        (None, false)
    };

    if boundary_status_consistent {
        if let Some(boundary) = actual_boundary
            .as_ref()
            .filter(|boundary| boundary.category == "invalid_input")
        {
            return VerificationOutcome::InvalidInput {
                reason: format!("{}: {}", boundary.path, boundary.reason),
            };
        }
    }

    if !report.unsupported.is_empty() {
        failures.push(VerificationFailure::UnsupportedTransitions {
            count: report.unsupported.len(),
        });
    }

    if let Some(actual) = actual_boundary
        .as_ref()
        .filter(|boundary| boundary.category != "none")
    {
        failures.push(VerificationFailure::UnexpectedBoundary {
            boundary: actual.clone(),
        });
    }

    match integrity {
        Some(integrity) => {
            if !integrity.eof_validated {
                failures.push(VerificationFailure::TailNotValidated);
            }
            if integrity.disposed_actions != integrity.applicable_actions {
                failures.push(VerificationFailure::IncompleteActionAccounting {
                    applicable_actions: integrity.applicable_actions,
                    disposed_actions: integrity.disposed_actions,
                });
            }
            if integrity.duplicate_dispositions != 0 {
                failures.push(VerificationFailure::DuplicateActionDispositions {
                    count: integrity.duplicate_dispositions,
                });
            }
            if options.require_terminal && !integrity.terminal_state_observed {
                failures.push(VerificationFailure::CompleteTraceNotTerminal);
            }
            // Rejected-action accounting is only required for full terminal runs.
            // Clean truncated prefixes may still record out-of-scope rejections.
            if options.require_terminal && integrity.rejected_actions != 0 {
                failures.push(VerificationFailure::CompleteTraceHasRejectedActions {
                    count: integrity.rejected_actions,
                });
            }
        }
        None => failures.push(VerificationFailure::MissingActionIntegrity),
    }

    if !failures.is_empty() {
        return VerificationOutcome::Failed { failures };
    }

    VerificationOutcome::CompletePass
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SeedStartReport, StartRunCommand};

    fn report(category: &str) -> SimRealReport {
        SimRealReport {
            total_actions: 1,
            action_dispositions: Vec::new(),
            action_integrity: None,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: Vec::new(),
            seed_start: Some(SeedStartReport {
                start_command: StartRunCommand {
                    action_step: 1,
                    character: "IRONCLAD".to_owned(),
                    ascension: 0,
                    external_seed: "1".to_owned(),
                    numeric_seed: 1,
                    verification_starting_hp: None,
                },
                failed: category != "none",
                first_boundary: SeedStartBoundary {
                    path: "$.actions[verified]".to_owned(),
                    category: category.to_owned(),
                    reason: "test".to_owned(),
                },
                sim_run_state: None,
            }),
        }
    }

    #[test]
    fn clean_direct_report_passes() {
        let report = report("none");
        let integrity = VerificationIntegrity {
            eof_validated: true,
            applicable_actions: 1,
            disposed_actions: 1,
            duplicate_dispositions: 0,
            terminal_state_observed: false,
            rejected_actions: 0,
        };
        assert_eq!(
            assess_verification(Ok(&report), Some(&integrity)),
            VerificationOutcome::CompletePass
        );
    }

    #[test]
    fn non_none_boundary_fails() {
        let report = report("unexpected_sim_real_diff");
        assert!(matches!(
            assess_verification(Ok(&report), Some(&VerificationIntegrity::default())),
            VerificationOutcome::Failed { .. }
        ));
    }

    #[test]
    fn terminal_requirement_is_explicit() {
        let report = report("none");
        let integrity = VerificationIntegrity {
            eof_validated: true,
            applicable_actions: 1,
            disposed_actions: 1,
            ..VerificationIntegrity::default()
        };
        assert!(matches!(
            assess_verification_with_options(
                Ok(&report),
                Some(&integrity),
                AssessmentOptions {
                    require_terminal: true
                }
            ),
            VerificationOutcome::Failed { .. }
        ));
    }
}
