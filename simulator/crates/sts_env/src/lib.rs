#![forbid(unsafe_code)]
#![doc = "Fair, state-owning environment for the Slay the Spire simulator."]

mod action;
mod combat_observation;
mod environment;
#[cfg(test)]
mod fair_json_allowlist;
mod run_observation;

pub use action::{DecisionRevision, FairError, PublicChoice, PublicChoiceRequest};
pub use combat_observation::{
    FairCard, FairCardDynamicValues, FairCombatContext, FairCombatObservation, FairCombatPhase,
    FairCounter, FairHandCard, FairIntentCategory, FairMonster, FairMonsterIntent,
    FairObservationError, FairOrb, FairOrbSlot, FairPile, FairPlayer, FairPotionSlot, FairPower,
    FairRelic, FairSelection, FairSelectionKind, FairSelectionOption,
    FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION,
};
pub use environment::{FairDecision, FairEnvironment, FAIR_ENV_SCHEMA_VERSION};
pub use run_observation::{
    FairCardSlot, FairEventChoice, FairEventObservation, FairGridObservation, FairMapNode,
    FairMapObservation, FairMatchAndKeepCard, FairQueuedCardReward, FairRestObservation,
    FairRestOption, FairRewardObservation, FairRunContext, FairRunObservation, FairRunPhase,
    FairRunPotionSlot, FairRunRelic, FairRunScreen, FairShopCard, FairShopObservation,
    FairShopPotion, FairShopRelic, FairTreasureObservation, FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
};

pub(crate) use run_observation::fair_run_observation;

/// Parses either a decimal seed or the game's public seed alphabet.
pub fn parse_seed(seed: &str) -> Result<u64, FairError> {
    if seed.trim().is_empty() {
        return Err(FairError::InvalidSeed);
    }
    if let Ok(value) = seed.parse::<u64>() {
        return Ok(value);
    }
    sts_core::adapter_internals::try_sts_seed_string_to_long(seed)
        .map(|value| value as u64)
        .map_err(|_| FairError::InvalidSeed)
}
