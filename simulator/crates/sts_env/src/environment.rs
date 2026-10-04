use serde::{Deserialize, Serialize};
use sts_core::adapter_internals::RunState;

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
    /// Build a new synthetic A0 combat, installing inputs before combat-start rules.
    pub fn from_synthetic_spec(spec: crate::SyntheticCombatSpec) -> Result<Self, String> {
        let env = Self {
            state: crate::synthetic::build(spec)?,
            revision: DecisionRevision::new(0),
        };
        env.decision().map_err(|e| e.to_string())?;
        Ok(env)
    }

    pub fn new_ironclad(seed: u64, ascension: u8) -> Result<Self, FairError> {
        Self::new_ironclad_with_final_act(seed, ascension, false)
    }

    /// Ordinary initial run with an explicit pre-run Heart-unlocked profile.
    /// Uses natural starting HP/deck; grants no keys and offers no state repair.
    /// The profile is installed once before the first policy decision.
    pub fn new_ironclad_with_final_act(
        seed: u64,
        ascension: u8,
        final_act: bool,
    ) -> Result<Self, FairError> {
        let mut state = RunState::try_seeded_ironclad(seed, ascension)
            .map_err(|_| FairError::DecisionUnavailable)?;
        if final_act {
            state
                .set_final_act_available(Some(true))
                .map_err(|_| FairError::DecisionUnavailable)?;
        }
        Ok(Self {
            state,
            revision: DecisionRevision::new(0),
        })
    }

    /// Explicit synthetic experiment constructor; never a replay/trace repair path.
    pub fn new_synthetic_ironclad(
        seed: u64,
        ascension: u8,
        hp: i32,
        final_act: bool,
    ) -> Result<Self, FairError> {
        if hp <= 0 {
            return Err(FairError::InvalidChoice);
        }
        let mut env = Self::new_ironclad(seed, ascension)?;
        env.state.hp = hp;
        env.state.max_hp = hp;
        if final_act {
            env.state
                .set_final_act_available(Some(true))
                .map_err(|_| FairError::DecisionUnavailable)?;
        }
        Ok(env)
    }

    /// Create an independent synthetic combat scenario. The source is unchanged.
    /// This changes HP only, not already-resolved combat-start effects or RNG.
    pub fn synthetic_combat_root(&self, hp: i32) -> Result<Self, FairError> {
        if hp <= 0 || self.state.phase != sts_core::adapter_internals::RunPhase::Combat {
            return Err(FairError::InvalidChoice);
        }
        let mut root = self.clone();
        root.state.hp = hp;
        root.state.max_hp = hp;
        root.revision = self
            .revision
            .checked_next()
            .ok_or(FairError::RevisionExhausted)?;
        root.decision()?;
        Ok(root)
    }

    #[must_use]
    pub const fn revision(&self) -> DecisionRevision {
        self.revision
    }

    pub fn observation(&self) -> Result<FairRunObservation, FairError> {
        fair_run_observation(&self.state).map_err(|_| FairError::DecisionUnavailable)
    }

    /// Public context HP. This is the same field as `FairRunContext.player_hp`
    /// and does not project the screen, enumerate choices, or draw RNG.
    #[must_use]
    pub fn public_player_hp(&self) -> i32 {
        self.state.hp
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
        // Validate the actual public content below, not relic ownership.
        // Prismatic acquisition/equip is modeled by the core; unmodeled
        // cross-color cards still fail projection rather than gaining invented
        // costs or mechanics. This is not blanket cross-color support.
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

    /// Apply one index into the current public legal list.
    ///
    /// Uses the same `projected_choices` pairing as [`Self::step`], then the same
    /// successor projection. It does not invent a second legality or observation
    /// contract, and it does not scan the pre-state twice.
    pub fn step_at_public_index(
        &mut self,
        revision: DecisionRevision,
        index: usize,
    ) -> Result<FairDecision, FairError> {
        if revision != self.revision {
            return Err(FairError::StaleDecision);
        }
        let next_revision = self
            .revision
            .checked_next()
            .ok_or(FairError::RevisionExhausted)?;
        let action = projected_choices(&self.state)?
            .into_iter()
            .nth(index)
            .map(|(_, action)| action)
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
    use sts_core::adapter_internals::Relic;

    #[test]
    fn ordinary_final_act_constructor_preserves_natural_inputs_and_profile_rng() {
        for seed in [1, 7, 42] {
            let legacy = FairEnvironment::new_ironclad(seed, 0).expect("ordinary run");
            let explicit =
                FairEnvironment::new_ironclad_with_final_act(seed, 0, false).expect("ordinary run");
            assert_eq!(legacy.state, explicit.state);
            assert_eq!(
                legacy.decision().expect("decision"),
                explicit.decision().expect("decision")
            );

            let env =
                FairEnvironment::new_ironclad_with_final_act(seed, 0, true).expect("Heart profile");
            let mut expected = RunState::seeded_ironclad(seed, 0);
            expected
                .set_final_act_available(Some(true))
                .expect("initial profile rule");
            assert_eq!(env.state, expected);
            assert_eq!(env.revision(), DecisionRevision::new(0));
            assert_eq!(env.public_player_hp(), 80);
            let observation = env.observation().expect("public observation");
            assert_eq!(observation.context.player_max_hp, 80);
            assert_eq!(
                observation.context.deck,
                legacy
                    .observation()
                    .expect("ordinary observation")
                    .context
                    .deck
            );
            assert!(observation.context.final_act_available);
            assert!(!observation.context.keys.ruby);
            assert!(!observation.context.keys.emerald);
            assert!(!observation.context.keys.sapphire);
            assert!(env.state.emerald_key_node.is_some());
            let before = env.state.clone();
            assert_eq!(
                env.clone().decision().expect("clone"),
                env.decision().expect("decision")
            );
            assert_eq!(env.observation().expect("repeat"), observation);
            assert_eq!(
                env.state, before,
                "reads and clones do not reselect burning elite"
            );
        }
    }

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
    fn public_index_step_matches_choice_step_and_rejects_without_advancing() {
        let mut by_choice = FairEnvironment::new_ironclad(1, 0).expect("environment");
        let mut by_index = by_choice.clone();
        for _ in 0..30 {
            let decision = by_choice.decision().expect("decision");
            if decision.choices.is_empty() {
                break;
            }
            let index = decision.choices.len() / 2;
            let choice = decision.choices[index];
            let left = by_choice
                .step(PublicChoiceRequest {
                    revision: decision.revision,
                    choice,
                })
                .expect("choice step");
            let right = by_index
                .step_at_public_index(decision.revision, index)
                .expect("index step");
            assert_eq!(left, right);
        }
        let mut stale = FairEnvironment::new_ironclad(3, 0).expect("environment");
        let before = stale.decision().expect("decision");
        assert_eq!(
            stale.step_at_public_index(DecisionRevision::new(99), 0),
            Err(FairError::StaleDecision)
        );
        assert_eq!(stale.decision().expect("unchanged"), before);
        assert_eq!(
            stale.step_at_public_index(before.revision, 10_000),
            Err(FairError::InvalidChoice)
        );
        assert_eq!(stale.decision().expect("unchanged"), before);
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
    fn public_player_hp_matches_observation_context_without_rng() {
        let env = FairEnvironment::new_ironclad(7, 0).expect("environment");
        let before = env.clone();
        assert_eq!(
            env.public_player_hp(),
            env.observation().expect("observation").context.player_hp
        );
        assert_eq!(env.decision(), before.decision());
    }

    #[test]
    fn clone_preserves_decision_and_revision() {
        let env = FairEnvironment::new_ironclad(42, 5).expect("environment");
        let cloned = env.clone();
        assert_eq!(cloned.revision(), env.revision());
        assert_eq!(cloned.decision(), env.decision());
    }

    #[test]
    fn courier_missing_environmental_input_rejects_atomically_without_rerolling() {
        use sts_core::adapter_internals::{apply_run_decision_action, enter_shop_screen, SimError};

        let mut state = RunState::map_fixture();
        state.gold = 500;
        state.relics.push(Relic::TheCourier);
        enter_shop_screen(&mut state).expect("shop");
        let mut env = FairEnvironment {
            state,
            revision: DecisionRevision::new(0),
        };
        let before = env.clone();
        let decision = env.decision().expect("shop decision");
        let choice = PublicChoice::BuyShopCard { shop_slot: 0 };
        let (index, (_, action)) = projected_choices(&env.state)
            .expect("projected choices")
            .into_iter()
            .enumerate()
            .find(|(_, (c, _))| *c == choice)
            .expect("advertised colored-card purchase");
        assert_eq!(
            apply_run_decision_action(&env.state, action),
            Err(SimError::MissingExternalRng(
                "courier_colored_card_selection"
            )),
        );
        assert_eq!(
            env.step(PublicChoiceRequest {
                revision: decision.revision,
                choice
            }),
            Err(FairError::InvalidChoice),
        );
        assert_eq!(env.state, before.state);
        assert_eq!(env.revision, before.revision);
        assert_eq!(
            env.step_at_public_index(decision.revision, index),
            Err(FairError::InvalidChoice),
        );
        assert_eq!(env.state, before.state);
        assert_eq!(env.decision().expect("unchanged decision"), decision);
    }

    #[test]
    fn prismatic_shop_purchase_publishes_successor_for_both_step_apis() {
        use sts_core::adapter_internals::{enter_shop_screen, ShopRelicSlot};

        let mut state = RunState::map_fixture();
        state.gold = 500;
        enter_shop_screen(&mut state).expect("shop");
        state.shop.as_mut().expect("shop").relics = vec![ShopRelicSlot {
            relic_key: Relic::PrismaticShard,
            price: 150,
            sold: false,
        }];
        let mut env = FairEnvironment {
            state,
            revision: DecisionRevision::new(0),
        };
        let before = env.decision().expect("shop decision");
        let choice = PublicChoice::BuyShopRelic { shop_slot: 0 };
        let index = before
            .choices
            .iter()
            .position(|c| *c == choice)
            .expect("legal purchase");
        let mut by_index = env.clone();
        let next = env
            .step(PublicChoiceRequest {
                revision: before.revision,
                choice,
            })
            .expect("Prismatic acquisition is modeled");
        assert_eq!(next.revision, DecisionRevision::new(1));
        assert_eq!(env.state.gold, 350);
        assert!(env.state.relics.contains(&Relic::PrismaticShard));
        assert!(env.state.shop.as_ref().expect("shop").relics[0].sold);
        assert!(!next.choices.contains(&choice));
        assert!(!next.choices.is_empty());
        assert_eq!(
            next,
            by_index
                .step_at_public_index(before.revision, index)
                .expect("index step")
        );
        assert_eq!(env.state, by_index.state);
        env.state.validate().expect("valid purchased state");
        let committed = env.clone();
        assert_eq!(
            env.step(PublicChoiceRequest {
                revision: before.revision,
                choice
            }),
            Err(FairError::StaleDecision)
        );
        assert_eq!(env.state, committed.state);
        assert_eq!(env.decision().expect("stable successor"), next);
    }

    #[test]
    fn prismatic_checks_actual_public_content_not_relic_ownership() {
        use sts_core::adapter_internals::{
            content::{cards::COOLHEADED_ANY_COLOR_ID, shop_pool::shop_card_content_id},
            CardId, CardInstance,
        };

        let mut env = FairEnvironment::new_ironclad(1, 0).expect("environment");
        env.state.relics.push(Relic::PrismaticShard);
        env.state
            .deck
            .push(CardInstance::new(CardId::new(100), COOLHEADED_ANY_COLOR_ID));
        let before = env.state.clone();
        let decision = env.decision().expect("modeled cross-color card");
        assert_eq!(env.decision().expect("repeat"), decision);
        assert_eq!(env.state, before, "export draws no RNG");
        // This change is not a claim that all cross-color mechanics are modeled.
        // Retain the content-level guard instead of fabricating a card cost.
        env.state.deck.push(CardInstance::new(
            CardId::new(101),
            shop_card_content_id("FLYING_KNEE"),
        ));
        assert_eq!(env.decision(), Err(FairError::DecisionUnavailable));
    }
}
