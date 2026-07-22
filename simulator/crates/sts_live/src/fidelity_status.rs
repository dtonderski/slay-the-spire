use crate::model::{FidelityKind, FidelityStatus, LiveResult};
use sts_core::RunState;
use sts_verify::verify_communication_mod_trace;

pub(super) fn verify_seed_start_trace(
    jsonl: &str,
) -> LiveResult<(FidelityStatus, Option<RunState>)> {
    match verify_communication_mod_trace(jsonl) {
        Ok(report) => {
            let sim_run_state = report
                .seed_start
                .as_ref()
                .and_then(|seed_start| seed_start.sim_run_state.clone());
            Ok((seed_start_status(&report), sim_run_state))
        }
        Err(err) => Ok((
            FidelityStatus {
                kind: FidelityKind::Unknown,
                first_divergent_step: None,
                compact_diff: vec![err.to_string()],
                message: Some("seed-start replay could not be completed".to_owned()),
            },
            None,
        )),
    }
}

fn seed_start_status(report: &sts_verify::SimRealReport) -> FidelityStatus {
    let Some(seed_start) = &report.seed_start else {
        return FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: Vec::new(),
            message: Some("seed-start replay returned no seed-start report".to_owned()),
        };
    };
    if !report.unexpected_diffs.is_empty() {
        return unexpected_diff_status(report);
    }
    if seed_start.failed {
        if seed_start
            .first_boundary
            .reason
            .contains("trace ended before seed-start verifier reached")
        {
            return FidelityStatus {
                kind: FidelityKind::Unknown,
                first_divergent_step: None,
                compact_diff: Vec::new(),
                message: Some(
                    "waiting for live trace to reach the next seed-start verifier boundary"
                        .to_owned(),
                ),
            };
        }
        return FidelityStatus {
            kind: FidelityKind::Unknown,
            first_divergent_step: None,
            compact_diff: vec![seed_start.first_boundary.reason.clone()],
            message: Some(format!(
                "seed-start replay reached boundary {}: {}",
                seed_start.first_boundary.category, seed_start.first_boundary.reason
            )),
        };
    }
    FidelityStatus {
        kind: FidelityKind::Ok,
        first_divergent_step: None,
        compact_diff: Vec::new(),
        message: Some("seed-start replay matched supported run history".to_owned()),
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

#[cfg(test)]
mod tests {
    use super::{seed_start_status, unexpected_diff_status};
    use crate::model::FidelityKind;
    use sts_verify::{
        SeedStartBoundary, SeedStartReport, SimRealReport, StartRunCommand, UnexpectedDiff,
    };

    #[test]
    fn monster_intent_diff_is_strict_live_fidelity_loss() {
        let diff = UnexpectedDiff {
            action_step: 81,
            command: "END".to_owned(),
            label: "end turn".to_owned(),
            diffs: vec!["monster_intent: \"ATTACK_DEBUFF\" != \"ATTACK\"".to_owned()],
        };

        let status = unexpected_diff_status(&SimRealReport {
            total_actions: 1,
            ignored_tail_actions: 0,
            action_dispositions: Vec::new(),
            action_integrity: None,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: vec![diff],
            seed_start: None,
        });

        assert_eq!(status.kind, FidelityKind::Lost);
        assert_eq!(status.first_divergent_step, Some(81));
        assert!(!status.compact_diff.is_empty());
    }

    #[test]
    fn seed_start_unexpected_diff_takes_priority_over_later_boundary() {
        let status = seed_start_status(&SimRealReport {
            total_actions: 2,
            ignored_tail_actions: 0,
            action_dispositions: Vec::new(),
            action_integrity: None,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: vec![UnexpectedDiff {
                action_step: 15,
                command: "CHOOSE 0".to_owned(),
                label: "card reward".to_owned(),
                diffs: vec!["choices[0]: \"battle trance\" != \"body slam\"".to_owned()],
            }],
            seed_start: Some(SeedStartReport {
                start_command: StartRunCommand {
                    action_step: 1,
                    character: "IRONCLAD".to_owned(),
                    ascension: 0,
                    external_seed: "CODEX04".to_owned(),
                    numeric_seed: 22_079_335_079,
                    verification_starting_hp: None,
                },
                failed: true,
                first_boundary: SeedStartBoundary {
                    path: "$.actions[step=51].command".to_owned(),
                    category: "unexpected_seed_start_command".to_owned(),
                    reason: "seed-start bootstrap harness did not expect command 'PLAY 2' in phase Event"
                        .to_owned(),
                },
                sim_run_state: None,
            }),
        });

        assert_eq!(status.kind, FidelityKind::Lost);
        assert_eq!(status.first_divergent_step, Some(15));
        assert!(status
            .message
            .unwrap()
            .contains("unexpected simulator diff"));
    }
}
