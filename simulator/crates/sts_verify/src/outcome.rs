//! Typed verification outcomes and the evidence required to claim them.

use crate::{ActionDispositionKind, SeedStartBoundary, SimRealError, SimRealReport};
use serde::{Deserialize, Serialize};

pub const VERIFICATION_CORPUS_MANIFEST_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCorpusManifest {
    pub schema: u32,
    pub entries: Vec<VerificationCorpusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCorpusEntry {
    pub trace: String,
    pub expectation: VerificationExpectation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerificationExpectation {
    Complete,
    RetainedPrefix { endpoint: RetainedPrefixEndpoint },
    ExpectedBoundary { boundary: ExpectedBoundary },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedPrefixEndpoint {
    pub action_step: u32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedBoundary {
    pub path: String,
    pub category: String,
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
    CompletePass,
    RetainedPrefixPass { endpoint: RetainedPrefixEndpoint },
    ExpectedBoundary { boundary: SeedStartBoundary },
    InvalidInput { reason: String },
    Failed { failures: Vec<VerificationFailure> },
}

impl VerificationOutcome {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Self::CompletePass | Self::RetainedPrefixPass { .. } | Self::ExpectedBoundary { .. }
        )
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
    ExpectedBoundaryNotReached {
        expected: ExpectedBoundary,
        actual: SeedStartBoundary,
    },
    CompleteTraceNotTerminal,
    CompleteTraceHasRejectedActions {
        count: usize,
    },
    RetainedPrefixEndpointMismatch {
        expected: RetainedPrefixEndpoint,
        actual_action_step: Option<u32>,
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

pub fn assess_verification(
    result: Result<&SimRealReport, &SimRealError>,
    expectation: &VerificationExpectation,
    integrity: Option<&VerificationIntegrity>,
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

    let unsupported_is_exact_boundary_cause = matches!(
        (
            expectation,
            actual_boundary.as_ref(),
            report.unsupported.as_slice(),
        ),
        (
            VerificationExpectation::ExpectedBoundary { boundary: expected },
            Some(actual),
            [unsupported],
        ) if actual.path == expected.path
            && actual.category == expected.category
            && actual.path == format!("$.actions[step={}].command", unsupported.action_step)
            && actual.reason == unsupported.reason
    );
    if !report.unsupported.is_empty() && !unsupported_is_exact_boundary_cause {
        failures.push(VerificationFailure::UnsupportedTransitions {
            count: report.unsupported.len(),
        });
    }

    match (expectation, actual_boundary.as_ref()) {
        (VerificationExpectation::ExpectedBoundary { boundary: expected }, Some(actual))
            if actual.path != expected.path || actual.category != expected.category =>
        {
            failures.push(VerificationFailure::ExpectedBoundaryNotReached {
                expected: expected.clone(),
                actual: actual.clone(),
            });
        }
        (VerificationExpectation::ExpectedBoundary { .. }, Some(_)) => {}
        (VerificationExpectation::ExpectedBoundary { .. }, None) => {}
        (_, Some(actual)) if actual.category != "none" => {
            failures.push(VerificationFailure::UnexpectedBoundary {
                boundary: actual.clone(),
            });
        }
        _ => {}
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
            let unresolved_is_expected_boundary_cause = matches!(
                (expectation, actual_boundary.as_ref()),
                (
                    VerificationExpectation::ExpectedBoundary { boundary: expected },
                    Some(actual),
                ) if actual.path == expected.path
                    && actual.category == expected.category
                    && (expected.category == "unreconciled_copied_attack_frame"
                        && integrity.unresolved_transient_assertions == 1
                        || expected.category == "unreconciled_smoke_bomb_frame"
                            && integrity.unresolved_transient_assertions > 0
                        || expected.category == "unsupported_smoke_bomb_queued_combat"
                            && integrity.unresolved_transient_assertions > 0)
            );
            if integrity.unresolved_transient_assertions != 0
                && !unresolved_is_expected_boundary_cause
            {
                failures.push(VerificationFailure::UnresolvedTransientAssertions {
                    count: integrity.unresolved_transient_assertions,
                });
            }
            if matches!(expectation, VerificationExpectation::Complete)
                && !integrity.terminal_state_observed
            {
                failures.push(VerificationFailure::CompleteTraceNotTerminal);
            }
            if matches!(expectation, VerificationExpectation::Complete)
                && integrity.rejected_actions != 0
            {
                failures.push(VerificationFailure::CompleteTraceHasRejectedActions {
                    count: integrity.rejected_actions,
                });
            }
        }
        None => failures.push(VerificationFailure::MissingActionIntegrity),
    }

    if let VerificationExpectation::RetainedPrefix { endpoint } = expectation {
        let actual_action_step = report
            .action_dispositions
            .iter()
            .rev()
            .find(|entry| entry.disposition == ActionDispositionKind::Verified)
            .map(|entry| entry.action_step);
        if actual_action_step != Some(endpoint.action_step) {
            failures.push(VerificationFailure::RetainedPrefixEndpointMismatch {
                expected: endpoint.clone(),
                actual_action_step,
            });
        }
    }

    if !failures.is_empty() {
        return VerificationOutcome::Failed { failures };
    }

    match expectation {
        VerificationExpectation::Complete => VerificationOutcome::CompletePass,
        VerificationExpectation::RetainedPrefix { endpoint } => {
            VerificationOutcome::RetainedPrefixPass {
                endpoint: endpoint.clone(),
            }
        }
        VerificationExpectation::ExpectedBoundary { .. } => VerificationOutcome::ExpectedBoundary {
            boundary: actual_boundary.expect("successful assessment has seed-start boundary"),
        },
    }
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

    fn complete_integrity() -> VerificationIntegrity {
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
    fn complete_pass_requires_clean_report_and_complete_integrity() {
        let report = report();
        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::Complete,
                Some(&complete_integrity()),
            ),
            VerificationOutcome::CompletePass
        );
    }

    #[test]
    fn retained_prefix_pass_preserves_declared_endpoint() {
        let report = report();
        let endpoint = RetainedPrefixEndpoint {
            action_step: 548,
            label: "floor 37 shop return to map".to_owned(),
        };
        let mut report = report;
        report.action_dispositions.push(crate::ActionDisposition {
            action_ordinal: 0,
            action_step: endpoint.action_step,
            command: "CHOOSE 0".to_owned(),
            disposition: ActionDispositionKind::Verified,
            detail: Some(endpoint.label.clone()),
            deferred_assertion_reconciled: false,
        });
        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::RetainedPrefix {
                    endpoint: endpoint.clone(),
                },
                Some(&complete_integrity()),
            ),
            VerificationOutcome::RetainedPrefixPass { endpoint }
        );
    }

    #[test]
    fn exact_expected_boundary_is_a_distinct_success() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[12]".to_owned(),
            category: "unsupported_mechanic".to_owned(),
            reason: "mechanic is outside retained coverage".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        let expected = ExpectedBoundary {
            path: actual.path.clone(),
            category: actual.category.clone(),
        };

        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::ExpectedBoundary { boundary: expected },
                Some(&complete_integrity()),
            ),
            VerificationOutcome::ExpectedBoundary { boundary: actual }
        );
    }

    #[test]
    fn exact_expected_boundary_allows_its_single_causal_unsupported_transition() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[step=12].command".to_owned(),
            category: "unsupported_mechanic".to_owned(),
            reason: "mechanic is outside retained coverage".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        report.unsupported.push(UnsupportedTransition {
            action_step: 12,
            command: "CHOOSE 1".to_owned(),
            reason: actual.reason.clone(),
        });
        let expected = ExpectedBoundary {
            path: actual.path.clone(),
            category: actual.category.clone(),
        };

        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::ExpectedBoundary { boundary: expected },
                Some(&complete_integrity()),
            ),
            VerificationOutcome::ExpectedBoundary { boundary: actual }
        );
    }

    #[test]
    fn copied_attack_boundary_preserves_its_unresolved_transient_evidence() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[step=481].command".to_owned(),
            category: "unreconciled_copied_attack_frame".to_owned(),
            reason: "queued copied attack did not reach a captured stable frame".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        report.unsupported.push(UnsupportedTransition {
            action_step: 481,
            command: "END".to_owned(),
            reason: actual.reason.clone(),
        });
        let expected = ExpectedBoundary {
            path: actual.path.clone(),
            category: actual.category.clone(),
        };
        let integrity = VerificationIntegrity {
            unresolved_transient_assertions: 1,
            ..complete_integrity()
        };

        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::ExpectedBoundary { boundary: expected },
                Some(&integrity),
            ),
            VerificationOutcome::ExpectedBoundary { boundary: actual }
        );
    }

    #[test]
    fn smoke_bomb_boundary_preserves_all_unresolved_queued_commands() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[step=93].command".to_owned(),
            category: "unreconciled_smoke_bomb_frame".to_owned(),
            reason: "Smoke Bomb escape did not reach a captured stable reward frame".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        let expected = ExpectedBoundary {
            path: actual.path.clone(),
            category: actual.category.clone(),
        };
        let integrity = VerificationIntegrity {
            unresolved_transient_assertions: 2,
            ..complete_integrity()
        };

        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::ExpectedBoundary { boundary: expected },
                Some(&integrity),
            ),
            VerificationOutcome::ExpectedBoundary { boundary: actual }
        );
    }

    #[test]
    fn queued_smoke_bomb_combat_boundary_preserves_unresolved_escape_evidence() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[step=230].command".to_owned(),
            category: "unsupported_smoke_bomb_queued_combat".to_owned(),
            reason: "a queued command mutated transient combat after the authoritative Smoke Bomb escape".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        report.unsupported.push(UnsupportedTransition {
            action_step: 230,
            command: "PLAY 2".to_owned(),
            reason: actual.reason.clone(),
        });
        let expected = ExpectedBoundary {
            path: actual.path.clone(),
            category: actual.category.clone(),
        };
        let integrity = VerificationIntegrity {
            unresolved_transient_assertions: 2,
            ..complete_integrity()
        };

        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::ExpectedBoundary { boundary: expected },
                Some(&integrity),
            ),
            VerificationOutcome::ExpectedBoundary { boundary: actual }
        );
    }

    #[test]
    fn expected_boundary_rejects_additional_unsupported_transitions() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[step=12].command".to_owned(),
            category: "unsupported_mechanic".to_owned(),
            reason: "mechanic is outside retained coverage".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        report.unsupported.extend([
            UnsupportedTransition {
                action_step: 11,
                command: "CHOOSE 0".to_owned(),
                reason: "earlier unsupported transition".to_owned(),
            },
            UnsupportedTransition {
                action_step: 12,
                command: "CHOOSE 1".to_owned(),
                reason: actual.reason.clone(),
            },
        ]);
        let expected = ExpectedBoundary {
            path: actual.path.clone(),
            category: actual.category.clone(),
        };

        let outcome = assess_verification(
            Ok(&report),
            &VerificationExpectation::ExpectedBoundary { boundary: expected },
            Some(&complete_integrity()),
        );
        assert_eq!(
            outcome,
            VerificationOutcome::Failed {
                failures: vec![VerificationFailure::UnsupportedTransitions { count: 2 }]
            }
        );
    }

    #[test]
    fn parse_and_start_errors_are_invalid_input() {
        let error = SimRealError::MissingStartCommand;
        assert_eq!(
            assess_verification(Err(&error), &VerificationExpectation::Complete, None),
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
            assess_verification(
                Ok(&report),
                &VerificationExpectation::Complete,
                Some(&complete_integrity()),
            ),
            VerificationOutcome::InvalidInput {
                reason: format!("{}: {}", boundary.path, boundary.reason),
            }
        );
    }

    #[test]
    fn invalid_input_boundary_cannot_be_an_expected_boundary() {
        let mut report = report();
        let boundary = SeedStartBoundary {
            path: "$.actions[step=12].sent_at".to_owned(),
            category: "invalid_input".to_owned(),
            reason: "command timing is malformed".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = boundary.clone();
        let expectation = VerificationExpectation::ExpectedBoundary {
            boundary: ExpectedBoundary {
                path: boundary.path.clone(),
                category: boundary.category.clone(),
            },
        };

        assert_eq!(
            assess_verification(Ok(&report), &expectation, Some(&complete_integrity()),),
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

        let outcome = assess_verification(
            Ok(&report),
            &VerificationExpectation::Complete,
            Some(&complete_integrity()),
        );
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
                    ..complete_integrity()
                }),
                VerificationFailure::IncompleteActionAccounting {
                    applicable_actions: 2,
                    disposed_actions: 1,
                },
            ),
            (
                Some(VerificationIntegrity {
                    duplicate_dispositions: 1,
                    ..complete_integrity()
                }),
                VerificationFailure::DuplicateActionDispositions { count: 1 },
            ),
            (
                Some(VerificationIntegrity {
                    unresolved_transient_assertions: 1,
                    ..complete_integrity()
                }),
                VerificationFailure::UnresolvedTransientAssertions { count: 1 },
            ),
            (
                Some(VerificationIntegrity {
                    terminal_state_observed: false,
                    ..complete_integrity()
                }),
                VerificationFailure::CompleteTraceNotTerminal,
            ),
            (
                Some(VerificationIntegrity {
                    applicable_actions: 0,
                    disposed_actions: 0,
                    rejected_actions: 1,
                    ..complete_integrity()
                }),
                VerificationFailure::CompleteTraceHasRejectedActions { count: 1 },
            ),
        ];

        for (integrity, expected_failure) in cases {
            let outcome = assess_verification(
                Ok(&report),
                &VerificationExpectation::Complete,
                integrity.as_ref(),
            );
            let VerificationOutcome::Failed { failures } = outcome else {
                panic!("integrity gap unexpectedly passed: {outcome:?}");
            };
            assert!(failures.contains(&expected_failure), "{failures:?}");
        }
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
            let outcome = assess_verification(
                Ok(&report),
                &VerificationExpectation::Complete,
                Some(&complete_integrity()),
            );
            let VerificationOutcome::Failed { failures } = outcome else {
                panic!("report gap unexpectedly passed: {outcome:?}");
            };
            assert!(failures.contains(&expected_failure), "{failures:?}");
        }
    }

    #[test]
    fn expected_boundary_must_match_both_category_and_path() {
        let mut report = report();
        let actual = SeedStartBoundary {
            path: "$.actions[12]".to_owned(),
            category: "unsupported_mechanic".to_owned(),
            reason: "stopped".to_owned(),
        };
        let seed_start = report.seed_start.as_mut().expect("seed-start report");
        seed_start.failed = true;
        seed_start.first_boundary = actual.clone();
        let expected = ExpectedBoundary {
            path: "$.actions[13]".to_owned(),
            category: actual.category.clone(),
        };

        assert_eq!(
            assess_verification(
                Ok(&report),
                &VerificationExpectation::ExpectedBoundary {
                    boundary: expected.clone(),
                },
                Some(&complete_integrity()),
            ),
            VerificationOutcome::Failed {
                failures: vec![VerificationFailure::ExpectedBoundaryNotReached {
                    expected,
                    actual,
                }],
            }
        );
    }

    #[test]
    fn retained_prefix_endpoint_must_be_the_last_verified_action() {
        let mut report = report();
        report.action_dispositions.push(crate::ActionDisposition {
            action_ordinal: 0,
            action_step: 12,
            command: "CHOOSE 0".to_owned(),
            disposition: ActionDispositionKind::Verified,
            detail: Some("floor 1 map".to_owned()),
            deferred_assertion_reconciled: false,
        });
        let endpoint = RetainedPrefixEndpoint {
            action_step: 13,
            label: "wrong endpoint".to_owned(),
        };

        let outcome = assess_verification(
            Ok(&report),
            &VerificationExpectation::RetainedPrefix {
                endpoint: endpoint.clone(),
            },
            Some(&complete_integrity()),
        );
        let VerificationOutcome::Failed { failures } = outcome else {
            panic!("wrong retained endpoint unexpectedly passed: {outcome:?}");
        };
        assert!(
            failures.contains(&VerificationFailure::RetainedPrefixEndpointMismatch {
                expected: endpoint,
                actual_action_step: Some(12),
            })
        );
    }
}
