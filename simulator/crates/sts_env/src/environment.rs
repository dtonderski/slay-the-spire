use serde::{Deserialize, Serialize};
use sts_core::adapter_internals::{Relic, RunState};

use crate::{
    action::{projected_choices, DecisionRevision, FairError, PublicChoice, PublicChoiceRequest},
    fair_run_observation, FairRunObservation,
};

pub const FAIR_ENV_SCHEMA_VERSION: u32 = 1;

/// One atomic fair decision. Observation and choices describe the same revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairDecision {
    pub schema_version: u32,
    pub revision: DecisionRevision,
    pub observation: FairRunObservation,
    pub choices: Vec<PublicChoice>,
}

/// State-owning fair policy environment.
///
/// The authoritative simulator state is intentionally private. Policy callers
/// can inspect only fair observations and decision-local choices.
#[derive(Debug, Clone)]
pub struct FairEnvironment {
    state: RunState,
    revision: DecisionRevision,
}

impl FairEnvironment {
    pub fn new_ironclad(seed: u64, ascension: u8) -> Result<Self, FairError> {
        let state = RunState::try_seeded_ironclad(seed, ascension)
            .map_err(|_| FairError::DecisionUnavailable)?;
        state
            .validate()
            .map_err(|_| FairError::DecisionUnavailable)?;
        Ok(Self {
            state,
            revision: DecisionRevision::new(0),
        })
    }

    #[must_use]
    pub const fn revision(&self) -> DecisionRevision {
        self.revision
    }

    pub fn observation(&self) -> Result<FairRunObservation, FairError> {
        fair_run_observation(&self.state).map_err(|_| FairError::DecisionUnavailable)
    }

    pub fn legal_choices(&self) -> Result<Vec<PublicChoice>, FairError> {
        projected_choices(&self.state)
            .map(|choices| choices.into_iter().map(|(choice, _)| choice).collect())
    }

    pub fn decision(&self) -> Result<FairDecision, FairError> {
        Self::decision_for(&self.state, self.revision)
    }

    fn decision_for(
        state: &RunState,
        revision: DecisionRevision,
    ) -> Result<FairDecision, FairError> {
        // The core Prismatic Shard pool contains cards whose public costs are
        // not yet represented by this schema. Fail before such a state can be
        // committed rather than fabricating metadata or stranding the policy.
        if state.relics.contains(&Relic::PrismaticShard) {
            return Err(FairError::DecisionUnavailable);
        }
        Ok(FairDecision {
            schema_version: FAIR_ENV_SCHEMA_VERSION,
            revision,
            observation: fair_run_observation(state).map_err(|_| FairError::DecisionUnavailable)?,
            choices: projected_choices(state)?
                .into_iter()
                .map(|(choice, _)| choice)
                .collect(),
        })
    }

    pub fn step(&mut self, request: PublicChoiceRequest) -> Result<FairDecision, FairError> {
        if request.revision != self.revision {
            return Err(FairError::StaleDecision);
        }
        let next_revision = self
            .revision
            .checked_next()
            .ok_or(FairError::RevisionExhausted)?;
        let action = projected_choices(&self.state)?
            .into_iter()
            .find_map(|(choice, action)| (choice == request.choice).then_some(action))
            .ok_or(FairError::InvalidChoice)?;
        let next = sts_core::adapter_internals::apply_run_decision_action(&self.state, action)
            .map_err(|_| FairError::InvalidChoice)?;
        let decision = Self::decision_for(&next, next_revision)?;
        self.state = next;
        self.revision = next_revision;
        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_and_invalid_choices_are_atomic() {
        let mut env = FairEnvironment::new_ironclad(1, 0).expect("environment");
        let before = env.decision().expect("decision");
        let stale = PublicChoiceRequest {
            revision: DecisionRevision::new(99),
            choice: PublicChoice::Proceed,
        };
        assert_eq!(env.step(stale), Err(FairError::StaleDecision));
        assert_eq!(env.decision().expect("unchanged"), before);

        let invalid = PublicChoiceRequest {
            revision: before.revision,
            choice: PublicChoice::EndTurn,
        };
        assert_eq!(env.step(invalid), Err(FairError::InvalidChoice));
        assert_eq!(env.decision().expect("unchanged"), before);
    }

    #[test]
    fn accepted_choice_increments_revision_once() {
        let mut env = FairEnvironment::new_ironclad(1, 0).expect("environment");
        let first = env.decision().expect("decision");
        let choice = *first.choices.first().expect("initial choice");
        let second = env
            .step(PublicChoiceRequest {
                revision: first.revision,
                choice,
            })
            .expect("step");
        assert_eq!(second.revision.get(), first.revision.get() + 1);
        assert_eq!(env.revision(), second.revision);
    }

    #[test]
    fn clone_preserves_decision_and_revision() {
        let env = FairEnvironment::new_ironclad(42, 5).expect("environment");
        let cloned = env.clone();
        assert_eq!(cloned.revision(), env.revision());
        assert_eq!(cloned.decision(), env.decision());
    }

    #[test]
    fn unsupported_prismatic_mechanics_are_rejected_before_commit() {
        let mut env = FairEnvironment::new_ironclad(1, 0).expect("environment");
        let before = env.decision().expect("initial decision");
        env.state.relics.push(Relic::PrismaticShard);
        assert_eq!(env.decision(), Err(FairError::DecisionUnavailable));
        env.state.relics.pop();
        assert_eq!(env.decision().expect("restored supported state"), before);
    }
}
