use crate::model::{FidelityKind, FidelityStatus, LiveResult};
use sts_verify::{verify_communication_mod_trace_with_mode, VerificationMode};

pub(super) fn verify_seed_start_trace(jsonl: &str) -> LiveResult<FidelityStatus> {
    match verify_communication_mod_trace_with_mode(jsonl, VerificationMode::SeedStart) {
        Ok(report) => {
            let Some(seed_start) = &report.seed_start else {
                return Ok(FidelityStatus {
                    kind: FidelityKind::Unknown,
                    first_divergent_step: None,
                    compact_diff: Vec::new(),
                    message: Some("seed-start replay returned no seed-start report".to_owned()),
                });
            };
            if seed_start.expected_failure {
                if seed_start
                    .first_boundary
                    .reason
                    .contains("trace ended before seed-start verifier reached")
                {
                    return Ok(FidelityStatus {
                        kind: FidelityKind::Unknown,
                        first_divergent_step: None,
                        compact_diff: Vec::new(),
                        message: Some(
                            "waiting for live trace to reach the next seed-start verifier boundary"
                                .to_owned(),
                        ),
                    });
                }
                return Ok(FidelityStatus {
                    kind: FidelityKind::Unknown,
                    first_divergent_step: None,
                    compact_diff: vec![seed_start.first_boundary.reason.clone()],
                    message: Some(format!(
                        "seed-start replay reached boundary {}: {}",
                        seed_start.first_boundary.category, seed_start.first_boundary.reason
                    )),
                });
            }
            if !report.unexpected_diffs.is_empty() {
                return Ok(unexpected_diff_status(&report));
            }
            if !report.observed_state_restorations.is_empty() {
                return Ok(observed_state_restoration_status(&report));
            }
            Ok(FidelityStatus {
                kind: FidelityKind::Ok,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: Some("seed-start replay matched supported run history".to_owned()),
            })
        }
        Err(err) => Ok(FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: vec![err.to_string()],
            message: Some("seed-start replay could not be completed".to_owned()),
        }),
    }
}

pub(super) fn unexpected_diff_status(report: &sts_verify::SimRealReport) -> FidelityStatus {
    let diff = &report.unexpected_diffs[0];
    FidelityStatus {
        kind: FidelityKind::Lost,
        first_divergent_step: Some(diff.action_step as u64),
        compact_diff: diff.diffs.clone(),
        message: Some(format!(
            "unexpected simulator diff after {} at step {}",
            diff.command, diff.action_step
        )),
    }
}

pub(super) fn observed_state_restoration_status(
    report: &sts_verify::SimRealReport,
) -> FidelityStatus {
    let restoration = &report.observed_state_restorations[0];
    FidelityStatus {
        kind: FidelityKind::Lost,
        first_divergent_step: Some(restoration.action_step as u64),
        compact_diff: vec![restoration.reason.clone()],
        message: Some(format!(
            "simulator replay restored from observed state after {} at step {}",
            restoration.command, restoration.action_step
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{observed_state_restoration_status, unexpected_diff_status};
    use crate::model::FidelityKind;
    use sts_verify::{ObservedStateRestoration, SimRealReport, UnexpectedDiff, VerificationMode};

    #[test]
    fn monster_intent_diff_is_strict_live_fidelity_loss() {
        let diff = UnexpectedDiff {
            action_step: 81,
            command: "END".to_owned(),
            label: "end turn".to_owned(),
            diffs: vec!["monster_intent: \"ATTACK_DEBUFF\" != \"ATTACK\"".to_owned()],
        };

        let status = unexpected_diff_status(&SimRealReport {
            mode: VerificationMode::ObservedState,
            total_actions: 1,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: vec![diff],
            observed_state_restorations: Vec::new(),
            seed_start: None,
        });

        assert_eq!(status.kind, FidelityKind::Lost);
        assert_eq!(status.first_divergent_step, Some(81));
        assert!(!status.compact_diff.is_empty());
    }

    #[test]
    fn observed_state_restoration_is_strict_live_fidelity_loss() {
        let status = observed_state_restoration_status(&SimRealReport {
            mode: VerificationMode::ObservedState,
            total_actions: 1,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: Vec::new(),
            observed_state_restorations: vec![ObservedStateRestoration {
                action_step: 151,
                command: "END".to_owned(),
                reason: "post-END non-pile combat state restored from observed state".to_owned(),
            }],
            seed_start: None,
        });

        assert_eq!(status.kind, FidelityKind::Lost);
        assert_eq!(status.first_divergent_step, Some(151));
        assert!(status
            .message
            .unwrap()
            .contains("restored from observed state"));
    }
}
