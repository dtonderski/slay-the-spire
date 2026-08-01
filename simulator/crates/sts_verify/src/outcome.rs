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
    pub applicable_actions: usize,
    pub disposed_actions: usize,
    pub duplicate_dispositions: usize,
    pub unresolved_transient_assertions: usize,
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
    IgnoredTailActions {
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
    IncompleteActionAccounting {
        applicable_actions: usize,
        disposed_actions: usize,
    },
    DuplicateActionDispositions {
        count: usize,
    },
    UnresolvedTransientAssertions {
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
    if report.ignored_tail_actions != 0 {
        failures.push(VerificationFailure::IgnoredTailActions {
            count: report.ignored_tail_actions,
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
            if integrity.unresolved_transient_assertions != 0 {
                failures.push(VerificationFailure::UnresolvedTransientAssertions {
                    count: integrity.unresolved_transient_assertions,
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
    use crate::{SeedStartReport, StartRunCommand, UnexpectedDiff, UnsupportedTransition};

    fn report() -> SimRealReport {
        SimRealReport {
            total_actions: 1,
            ignored_tail_actions: 0,
            action_dispositions: Vec::new(),
            action_integrity: None,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: Vec::new(),
            seed_start: Some(SeedStartReport {
                start_command: StartRunCommand {
                    action_step: 0,
                    character: "IRONCLAD".to_owned(),
                    ascension: 0,
                    external_seed: "TEST".to_owned(),
                    numeric_seed: 1_218_623,
                    verification_starting_hp: None,
                },
                failed: false,
                first_boundary: no_boundary(),
                sim_run_state: None,
            }),
        }
    }

    fn no_boundary() -> SeedStartBoundary {
        SeedStartBoundary {
            path: "$.actions[verified]".to_owned(),
            category: "none".to_owned(),
            reason: "all applicable actions verified".to_owned(),
        }
    }

    fn clean_integrity() -> VerificationIntegrity {
        VerificationIntegrity {
            applicable_actions: 1,
            disposed_actions: 1,
            duplicate_dispositions: 0,
            unresolved_transient_assertions: 0,
            terminal_state_observed: true,
            rejected_actions: 0,
        }
    }

    #[test]
    fn clean_through_eof_passes_without_terminal() {
        let report = report();
        let integrity = VerificationIntegrity {
            terminal_state_observed: false,
            ..clean_integrity()
        };
        assert_eq!(
            assess_verification(Ok(&report), Some(&integrity)),
            VerificationOutcome::CompletePass
        );
    }

    #[test]
    fn require_terminal_rejects_non_terminal() {
        let report = report();
        let integrity = VerificationIntegrity {
            terminal_state_observed: false,
            ..clean_integrity()
        };
        let outcome = assess_verification_with_options(
            Ok(&report),
            Some(&integrity),
            AssessmentOptions {
                require_terminal: true,
            },
        );
        let VerificationOutcome::Failed { failures } = outcome else {
            panic!("expected failure: {outcome:?}");
        };
        assert!(failures.contains(&VerificationFailure::CompleteTraceNotTerminal));
    }

    #[test]
    fn complete_pass_requires_clean_report_and_integrity() {
        let report = report();
        assert_eq!(
            assess_verification(Ok(&report), Some(&clean_integrity())),
            VerificationOutcome::CompletePass
        );
    }

    #[test]
    fn parse_and_start_errors_are_invalid_input() {
        let error = SimRealError::MissingStartCommand;
        assert_eq!(
            assess_verification(Err(&error), None),
            VerificationOutcome::InvalidInput {
                reason: "trace does not contain START command".to_owned(),
            }
        );
    }

    #[test]
    fn in_report_invalid_input_boundary_is_decisive() {
        let mut report = report();
        let boundary = SeedStartBoundary {
            path: "$.actions[step=12].sent_at".to_owned(),
            category: "invalid_input".to_owned(),
            reason: "command timing is missing".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = boundary.clone();
        report.unexpected_diffs.push(UnexpectedDiff {
            action_step: 11,
            command: "CHOOSE 0".to_owned(),
            label: "partial evidence remains in the report".to_owned(),
            diffs: vec!["gold: 10 != 20".to_owned()],
        });

        assert_eq!(
            assess_verification(Ok(&report), Some(&clean_integrity())),
            VerificationOutcome::InvalidInput {
                reason: format!("{}: {}", boundary.path, boundary.reason),
            }
        );
    }

    #[test]
    fn inconsistent_invalid_input_boundary_is_a_verifier_failure() {
        let mut report = report();
        let boundary = SeedStartBoundary {
            path: "$.actions[step=12].sent_at".to_owned(),
            category: "invalid_input".to_owned(),
            reason: "command timing is missing".to_owned(),
        };
        report
            .seed_start
            .as_mut()
            .expect("seed-start report")
            .first_boundary = boundary.clone();

        let outcome = assess_verification(Ok(&report), Some(&clean_integrity()));
        let VerificationOutcome::Failed { failures } = outcome else {
            panic!("inconsistent boundary was not a verifier failure: {outcome:?}");
        };
        assert!(
            failures.contains(&VerificationFailure::InconsistentBoundaryStatus {
                failed: false,
                boundary,
            })
        );
    }

    #[test]
    fn every_integrity_gap_prevents_a_pass() {
        let report = report();
        let cases = [
            (None, VerificationFailure::MissingActionIntegrity),
            (
                Some(VerificationIntegrity {
                    applicable_actions: 2,
                    disposed_actions: 1,
                    ..clean_integrity()
                }),
                VerificationFailure::IncompleteActionAccounting {
                    applicable_actions: 2,
                    disposed_actions: 1,
                },
            ),
            (
                Some(VerificationIntegrity {
                    duplicate_dispositions: 1,
                    ..clean_integrity()
                }),
                VerificationFailure::DuplicateActionDispositions { count: 1 },
            ),
            (
                Some(VerificationIntegrity {
                    unresolved_transient_assertions: 1,
                    ..clean_integrity()
                }),
                VerificationFailure::UnresolvedTransientAssertions { count: 1 },
            ),
        ];

        for (integrity, expected_failure) in cases {
            let outcome = assess_verification(Ok(&report), integrity.as_ref());
            let VerificationOutcome::Failed { failures } = outcome else {
                panic!("integrity gap unexpectedly passed: {outcome:?}");
            };
            assert!(failures.contains(&expected_failure), "{failures:?}");
        }
    }

    #[test]
    fn require_terminal_rejects_rejected_actions() {
        let report = report();
        let integrity = VerificationIntegrity {
            applicable_actions: 0,
            disposed_actions: 0,
            rejected_actions: 1,
            ..clean_integrity()
        };
        let outcome = assess_verification_with_options(
            Ok(&report),
            Some(&integrity),
            AssessmentOptions {
                require_terminal: true,
            },
        );
        let VerificationOutcome::Failed { failures } = outcome else {
            panic!("expected failure: {outcome:?}");
        };
        assert!(
            failures.contains(&VerificationFailure::CompleteTraceHasRejectedActions { count: 1 })
        );
    }

    #[test]
    fn every_report_gap_prevents_a_pass() {
        let mut cases = Vec::new();

        let mut with_diff = report();
        with_diff.unexpected_diffs.push(UnexpectedDiff {
            action_step: 1,
            command: "CHOOSE 0".to_owned(),
            label: "mismatch".to_owned(),
            diffs: vec!["gold: 10 != 20".to_owned()],
        });
        cases.push((with_diff, VerificationFailure::UnexpectedDiffs { count: 1 }));

        let mut with_unsupported = report();
        with_unsupported.unsupported.push(UnsupportedTransition {
            action_step: 1,
            command: "CHOOSE 0".to_owned(),
            reason: "unsupported".to_owned(),
        });
        cases.push((
            with_unsupported,
            VerificationFailure::UnsupportedTransitions { count: 1 },
        ));

        let mut with_tail = report();
        with_tail.ignored_tail_actions = 1;
        cases.push((
            with_tail,
            VerificationFailure::IgnoredTailActions { count: 1 },
        ));

        let mut with_boundary = report();
        let boundary = SeedStartBoundary {
            path: "$.actions[1]".to_owned(),
            category: "unexpected".to_owned(),
            reason: "stopped".to_owned(),
        };
        let seed_start = with_boundary
            .seed_start
            .as_mut()
            .expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = boundary.clone();
        cases.push((
            with_boundary,
            VerificationFailure::UnexpectedBoundary { boundary },
        ));

        let mut without_seed_start = report();
        without_seed_start.seed_start = None;
        cases.push((
            without_seed_start,
            VerificationFailure::MissingSeedStartReport,
        ));

        for (report, expected_failure) in cases {
            let outcome = assess_verification(Ok(&report), Some(&clean_integrity()));
            let VerificationOutcome::Failed { failures } = outcome else {
                panic!("report gap unexpectedly passed: {outcome:?}");
            };
            assert!(failures.contains(&expected_failure), "{failures:?}");
        }
    }
}
