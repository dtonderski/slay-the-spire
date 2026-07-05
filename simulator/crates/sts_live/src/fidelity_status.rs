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
    if is_restorable_observed_state_diff(diff) {
        return FidelityStatus {
            kind: FidelityKind::Ok,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some(
                "observed-state replay matched supported transitions; monster intent/block parity is restored from observed state"
                    .to_owned(),
            ),
        };
    }
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

fn is_restorable_observed_state_diff(diff: &sts_verify::UnexpectedDiff) -> bool {
    !diff.diffs.is_empty()
        && diff.diffs.iter().all(|entry| {
            entry.starts_with("monster_intent:") || entry.starts_with("monster_block:")
        })
}

#[cfg(test)]
mod tests {
    use super::{is_restorable_observed_state_diff, unexpected_diff_status};
    use crate::model::FidelityKind;
    use sts_verify::{SimRealReport, UnexpectedDiff, VerificationMode};

    #[test]
    fn monster_intent_only_diff_is_restorable_for_live_observed_state_fidelity() {
        let diff = UnexpectedDiff {
            action_step: 81,
            command: "END".to_owned(),
            label: "end turn".to_owned(),
            diffs: vec!["monster_intent: \"ATTACK_DEBUFF\" != \"ATTACK\"".to_owned()],
        };

        assert!(is_restorable_observed_state_diff(&diff));
        let status = unexpected_diff_status(&SimRealReport {
            mode: VerificationMode::ObservedState,
            total_actions: 1,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: vec![diff],
            observed_state_restorations: Vec::new(),
            seed_start: None,
        });

        assert_eq!(status.kind, FidelityKind::Ok);
        assert!(status.compact_diff.is_empty());
    }

    #[test]
    fn monster_block_only_diff_is_restorable_for_live_observed_state_fidelity() {
        let diff = UnexpectedDiff {
            action_step: 151,
            command: "END".to_owned(),
            label: "end turn".to_owned(),
            diffs: vec!["monster_block: 8 != 16".to_owned()],
        };

        assert!(is_restorable_observed_state_diff(&diff));
        let status = unexpected_diff_status(&SimRealReport {
            mode: VerificationMode::ObservedState,
            total_actions: 1,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: vec![diff],
            observed_state_restorations: Vec::new(),
            seed_start: None,
        });

        assert_eq!(status.kind, FidelityKind::Ok);
        assert!(status.compact_diff.is_empty());
    }
}
