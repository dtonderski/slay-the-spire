//! Fair-belief foundations for combat-only generated rollouts.
//!
//! This module is crate-private until a fair facade can issue observation-bound
//! start tokens and resolve public choices. A belief starts from public observations
//! and public history. Its weighted hypotheses are private and independently generated.
//! Only the materializer creates an authoritative state, and only from a belief-owned
//! particle. That state is a fresh combat-horizon rollout which must project back to
//! the public observation exactly.
//!
//! The current prior is an independently sampled exchangeable-future approximation.
//! It is not the game's master-seed run initialization, and it is not the post-entry
//! combat RNG state after opening shuffle, HP, and AI rolls.

use crate::{
    card::CardInstance,
    combat::{
        fair_combat_observation, CardPiles, CombatPhase, CombatRngState, CombatState, FairCard,
        FairCombatObservation, FairCombatPhase, FairIntentCategory, FairMonsterIntent,
        MonsterIntent, PlayerState,
    },
    content::{
        cards::ALL_CARDS,
        monsters::{
            monster_state, prepare_monster_intent_for_ascension, target_move_byte, CULTIST_A0,
            FIXED_SIMPLE_MONSTER,
        },
    },
    ids::{CardId, MonsterId},
    relic::{Relic, RelicCounters},
    rng::{RngTraceStream, StsRng},
    run::{PlayerChoice, RunPhase, RunState},
    CombatAction, SimError,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    num::NonZeroU64,
};

pub const FAIR_BELIEF_SCHEMA_VERSION: u32 = 1;
pub const FAIR_BELIEF_PRIOR_VERSION: &str = "a0_act1_simple_combat_exchangeable_v1";
const MAX_FAIR_BELIEF_PARTICLES: usize = 4_096;

/// Public events retained because a current observation alone cannot recover them.
///
/// This first slice records the durable public vocabulary but only consumes monster move events
/// during materialization. Future mechanics must extend the vocabulary before accepting roots
/// that depend on additional history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicCardZone {
    Hand,
    Draw,
    Discard,
    Exhaust,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublicCombatEvent {
    CardDrawn {
        card: FairCard,
    },
    CardPlayed {
        hand_slot: u16,
    },
    CardMoved {
        card: FairCard,
        from: PublicCardZone,
        to: PublicCardZone,
    },
    PileShuffled,
    MonsterMoveExecuted {
        monster_slot: u16,
        category: FairIntentCategory,
    },
    TurnStarted {
        turn: u32,
    },
}

/// One accepted public action and the public result observed after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicCombatStep {
    pub action: PlayerChoice,
    pub events: Vec<PublicCombatEvent>,
    pub observation: FairCombatObservation,
}

/// Opaque proof that an observation was emitted at the first player decision.
///
/// The token is observation-bound and not cloneable. A production issuer does not exist
/// yet; tests bind a token to a specific observation. The constructor consumes the token.
pub(crate) struct PublicCombatStart {
    bound_observation: FairCombatObservation,
}

impl PublicCombatStart {
    #[cfg(test)]
    fn bind(observation: FairCombatObservation) -> Self {
        Self {
            bound_observation: observation,
        }
    }
}

/// Hidden-free knowledge from the fair channel.
///
/// This foundation currently accepts only an opaque first-decision capability. Mid-combat
/// construction stays unavailable until the runtime emits and validates a complete typed public
/// prefix. There is intentionally no snapshot-only constructor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PublicCombatKnowledge {
    pub schema_version: u32,
    initial_observation: FairCombatObservation,
    history: Vec<PublicCombatStep>,
}

impl PublicCombatKnowledge {
    pub fn at_first_player_decision(
        observation: FairCombatObservation,
        start: PublicCombatStart,
    ) -> Result<Self, FairBeliefError> {
        if start.bound_observation != observation {
            return Err(FairBeliefError::InvalidPublicObservation(
                "first-decision token is not bound to this observation",
            ));
        }
        require_current_observation_schema(&observation)?;
        if observation.phase != FairCombatPhase::WaitingForPlayer {
            return Err(FairBeliefError::UnsupportedRoot(
                "fair belief starts only at a waiting-for-player combat boundary",
            ));
        }
        Ok(Self {
            schema_version: FAIR_BELIEF_SCHEMA_VERSION,
            initial_observation: observation,
            history: Vec::new(),
        })
    }

    /// Explicit refusal for roots without a facade-issued first-decision capability or a future
    /// validated public-history checkpoint. No privileged snapshot is accepted as an argument.
    pub fn refuse_unproven_root(
        _observation: FairCombatObservation,
    ) -> Result<Self, FairBeliefError> {
        Err(FairBeliefError::MissingPublicHistory(
            "combat root lacks fair first-decision or complete-history provenance",
        ))
    }

    #[must_use]
    pub fn initial_observation(&self) -> &FairCombatObservation {
        &self.initial_observation
    }

    #[must_use]
    pub fn history(&self) -> &[PublicCombatStep] {
        &self.history
    }

    #[must_use]
    pub fn latest_observation(&self) -> &FairCombatObservation {
        self.history
            .last()
            .map_or(&self.initial_observation, |step| &step.observation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FairBeliefPrior {
    pub schema_version: u32,
    pub prior_version: String,
}

impl FairBeliefPrior {
    #[must_use]
    pub fn a0_act1_simple_combat() -> Self {
        Self {
            schema_version: FAIR_BELIEF_SCHEMA_VERSION,
            prior_version: FAIR_BELIEF_PRIOR_VERSION.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GeneratedRngState {
    seed0: u64,
    seed1: u64,
    counter: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GeneratedCombatRngStates {
    shuffle: GeneratedRngState,
    monster: GeneratedRngState,
    monster_hp: GeneratedRngState,
    card_random: GeneratedRngState,
}

/// Fresh run RNG seeds. These independently sampled values fill unreachable envelope
/// fields so the combat-only shell cannot carry true RNG. They are not reconstructed
/// from a master seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GeneratedRunRngSeeds {
    event: u64,
    reward: u64,
    treasure: u64,
    potion: u64,
    relic: u64,
    shuffle: u64,
    merchant: u64,
    misc: u64,
    monster: u64,
}

/// One latent assignment owned only by [`FairBelief`]. Pile vectors contain indices into the
/// corresponding public canonical multiset, in authoritative storage order (bottom to top).
#[derive(Debug, Clone, PartialEq, Eq)]
struct HiddenHypothesis {
    schema_version: u32,
    prior_version: String,
    draw_storage_order: Vec<usize>,
    discard_storage_order: Vec<usize>,
    exhaust_storage_order: Vec<usize>,
    combat_rng: GeneratedCombatRngStates,
    run_rng: GeneratedRunRngSeeds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WeightedHiddenHypothesis {
    weight: NonZeroU64,
    hypothesis: HiddenHypothesis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BeliefNamedDraw {
    name: String,
    draw_count: u64,
}

/// Named deterministic sampler state. Keys are `stream|call_site`; values are counters.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BeliefRng {
    seed: u64,
    counters: BTreeMap<String, u64>,
}

impl Serialize for BeliefRng {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let named_draws = self
            .counters
            .iter()
            .map(|(name, draw_count)| BeliefNamedDraw {
                name: name.clone(),
                draw_count: *draw_count,
            })
            .collect::<Vec<_>>();
        let mut state = serializer.serialize_struct("BeliefRng", 2)?;
        state.serialize_field("seed", &self.seed)?;
        state.serialize_field("named_draws", &named_draws)?;
        state.end()
    }
}

impl BeliefRng {
    #[must_use]
    fn new(seed: u64) -> Self {
        Self {
            seed,
            counters: BTreeMap::new(),
        }
    }

    fn draw_u64(&mut self, stream: &str, call_site: &str) -> u64 {
        let key = format!("{stream}|{call_site}");
        let counter = self.counters.entry(key).or_default();
        let mixed = splitmix64(
            self.seed
                ^ stable_name_hash(stream)
                ^ stable_name_hash(call_site).rotate_left(17)
                ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        *counter = counter.wrapping_add(1);
        mixed
    }

    fn uniform_index(&mut self, stream: &str, call_site: &str, bound: usize) -> usize {
        assert!(bound > 0, "uniform belief draw requires a positive bound");
        let bound = bound as u64;
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.draw_u64(stream, call_site);
            if value >= threshold {
                return (value % bound) as usize;
            }
        }
    }
}

/// Public knowledge, declared prior, and weighted latent hypotheses. No authoritative state is
/// retained in the belief. Particles are private: callers materialize by index, never by supplying
/// a hypothesis.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct FairBelief {
    pub schema_version: u32,
    pub knowledge: PublicCombatKnowledge,
    pub prior: FairBeliefPrior,
    belief_rng: BeliefRng,
    #[serde(skip)]
    particles: Vec<WeightedHiddenHypothesis>,
}

impl FairBelief {
    pub fn initialize(
        knowledge: PublicCombatKnowledge,
        prior: FairBeliefPrior,
        particle_count: usize,
        belief_seed: u64,
    ) -> Result<Self, FairBeliefError> {
        if particle_count == 0 || particle_count > MAX_FAIR_BELIEF_PARTICLES {
            return Err(FairBeliefError::InvalidParticleCount);
        }
        validate_supported_public_state(&knowledge, &prior)?;
        let mut belief_rng = BeliefRng::new(belief_seed);
        let mut particles = Vec::new();
        particles
            .try_reserve_exact(particle_count)
            .map_err(|_| FairBeliefError::InvalidParticleCount)?;
        for particle_index in 0..particle_count {
            let hypothesis =
                sample_hypothesis(&knowledge, &prior, particle_index, &mut belief_rng)?;
            // Initialization is a checked constructor: every owned hypothesis must be capable
            // of producing a valid fresh state with the exact public projection.
            let _ = materialize_owned_hypothesis(&knowledge, &hypothesis)?;
            particles.push(WeightedHiddenHypothesis {
                weight: NonZeroU64::MIN,
                hypothesis,
            });
        }
        Ok(Self {
            schema_version: FAIR_BELIEF_SCHEMA_VERSION,
            knowledge,
            prior,
            belief_rng,
            particles,
        })
    }

    #[must_use]
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    pub fn materialize(
        &self,
        particle_index: usize,
    ) -> Result<GeneratedCombatRollout, FairBeliefError> {
        materialize_combat_rollout(self, particle_index)
    }
}

impl fmt::Debug for FairBelief {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FairBelief")
            .field("schema_version", &self.schema_version)
            .field("prior_version", &self.prior.prior_version)
            .field("particle_count", &self.particles.len())
            .finish_non_exhaustive()
    }
}

/// Fresh generated authority for combat rollouts only. It is not a posterior sample of the
/// surrounding run and must not be advanced into reward/map screens.
#[derive(Clone, PartialEq, Eq)]
pub struct GeneratedCombatRollout {
    run: RunState,
}

impl GeneratedCombatRollout {
    #[must_use]
    pub fn observation(&self) -> FairCombatObservation {
        fair_combat_observation(&self.run)
            .expect("materialized rollout was projection-checked at construction")
    }

    #[must_use]
    pub fn is_active_combat(&self) -> bool {
        self.run.phase == RunPhase::Combat
            && self
                .run
                .combat
                .as_ref()
                .is_some_and(|combat| !matches!(combat.phase, CombatPhase::Won | CombatPhase::Lost))
    }

    /// Advance this generated combat-only rollout by one public combat action.
    ///
    /// Already-terminated combats are refused. An action that ends combat returns the terminal
    /// combat state; a later step then fails. This authority does not continue into reward or
    /// map screens, so it must not use the run-level combat wrapper that enters those screens.
    pub fn apply_combat_action(&self, action: CombatAction) -> Result<Self, FairBeliefError> {
        if !self.is_active_combat() {
            return Err(FairBeliefError::CombatTerminated);
        }
        let combat = self.run.combat.as_ref().ok_or_else(|| {
            FairBeliefError::GeneratedStateInvalid("combat state is missing".to_owned())
        })?;
        let next_combat = crate::apply_combat_action(combat, action)
            .map_err(|error| FairBeliefError::GeneratedStateInvalid(error.to_string()))?;
        let mut next = self.run.clone();
        next.player_hp = next_combat.player.hp;
        next.player_max_hp = next_combat.player.max_hp;
        next.card_random_rng_counter = next_combat.rng.card_random_rng.counter();
        next.combat = Some(next_combat);
        Ok(Self { run: next })
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn authoritative_state(&self) -> &RunState {
        &self.run
    }
}

impl fmt::Debug for GeneratedCombatRollout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeneratedCombatRollout")
            .field("active_combat", &self.is_active_combat())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FairBeliefError {
    InvalidSchema,
    InvalidParticleCount,
    InvalidPublicObservation(&'static str),
    MissingPublicHistory(&'static str),
    UnsupportedRoot(&'static str),
    UnsupportedPrior(&'static str),
    UnsupportedMechanic(&'static str),
    InvalidHypothesis(&'static str),
    GeneratedStateInvalid(String),
    ProjectionMismatch,
    CombatTerminated,
    UnknownParticle,
}

impl fmt::Display for FairBeliefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema => f.write_str("unsupported fair-belief schema"),
            Self::InvalidParticleCount => {
                f.write_str("particle count must be positive and within the declared bound")
            }
            Self::InvalidPublicObservation(message)
            | Self::MissingPublicHistory(message)
            | Self::UnsupportedRoot(message)
            | Self::UnsupportedPrior(message)
            | Self::UnsupportedMechanic(message)
            | Self::InvalidHypothesis(message) => f.write_str(message),
            Self::GeneratedStateInvalid(message) => {
                write!(f, "generated combat state is invalid: {message}")
            }
            Self::ProjectionMismatch => {
                f.write_str("generated state does not project to the public observation")
            }
            Self::CombatTerminated => {
                f.write_str("generated combat rollout has already terminated")
            }
            Self::UnknownParticle => f.write_str("particle index is not owned by this belief"),
        }
    }
}

impl Error for FairBeliefError {}

pub fn materialize_combat_rollout(
    belief: &FairBelief,
    particle_index: usize,
) -> Result<GeneratedCombatRollout, FairBeliefError> {
    let hypothesis = &belief
        .particles
        .get(particle_index)
        .ok_or(FairBeliefError::UnknownParticle)?
        .hypothesis;
    materialize_owned_hypothesis(&belief.knowledge, hypothesis)
}

fn materialize_owned_hypothesis(
    knowledge: &PublicCombatKnowledge,
    hypothesis: &HiddenHypothesis,
) -> Result<GeneratedCombatRollout, FairBeliefError> {
    validate_supported_public_state(knowledge, &FairBeliefPrior::a0_act1_simple_combat())?;
    if hypothesis.schema_version != FAIR_BELIEF_SCHEMA_VERSION
        || hypothesis.prior_version != FAIR_BELIEF_PRIOR_VERSION
    {
        return Err(FairBeliefError::UnsupportedPrior(
            "hypothesis prior does not match the supported materializer",
        ));
    }

    let observation = knowledge.latest_observation();
    validate_permutation(
        &hypothesis.draw_storage_order,
        observation.draw_pile.cards.len(),
    )?;
    validate_permutation(
        &hypothesis.discard_storage_order,
        observation.discard_pile.cards.len(),
    )?;
    validate_permutation(
        &hypothesis.exhaust_storage_order,
        observation.exhaust_pile.cards.len(),
    )?;
    validate_generated_rng_hypothesis(hypothesis)?;

    let mut next_card_id = 1_u64;
    let hand = observation
        .hand
        .iter()
        .enumerate()
        .map(|(expected_slot, item)| {
            if item.slot != expected_slot {
                return Err(FairBeliefError::InvalidPublicObservation(
                    "hand slots are not contiguous public indices",
                ));
            }
            materialize_card(&item.card, &mut next_card_id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let draw_source = materialize_public_cards(&observation.draw_pile.cards, &mut next_card_id)?;
    let discard_source =
        materialize_public_cards(&observation.discard_pile.cards, &mut next_card_id)?;
    let exhaust_source =
        materialize_public_cards(&observation.exhaust_pile.cards, &mut next_card_id)?;

    let draw_pile = apply_storage_order(&draw_source, &hypothesis.draw_storage_order);
    let discard_pile = apply_storage_order(&discard_source, &hypothesis.discard_storage_order);
    let exhaust_pile = apply_storage_order(&exhaust_source, &hypothesis.exhaust_storage_order);

    let mut deck = hand.clone();
    deck.extend(draw_source.iter().cloned());
    deck.extend(discard_source.iter().cloned());
    deck.extend(exhaust_source.iter().cloned());
    deck.sort_by_key(|card| card.id.get());

    let mut player = PlayerState::new_run_entry(
        observation.player.hp,
        observation.player.max_hp,
        observation.player.max_energy,
    )
    .map_err(sim_error)?;
    player.block = observation.player.block;
    player.energy = observation.player.energy;

    let monsters = materialize_monsters(knowledge)?;
    let relics = materialize_relics(observation)?;
    let rng = CombatRngState {
        shuffle_rng: generated_sts_rng(hypothesis.combat_rng.shuffle, RngTraceStream::Shuffle),
        monster_rng: generated_sts_rng(hypothesis.combat_rng.monster, RngTraceStream::Monster),
        monster_hp_rng: generated_sts_rng(
            hypothesis.combat_rng.monster_hp,
            RngTraceStream::MonsterHp,
        ),
        card_random_rng: generated_sts_rng(
            hypothesis.combat_rng.card_random,
            RngTraceStream::CardRandom,
        ),
    };
    let mut combat = CombatState::from_entry_parts(
        player,
        monsters,
        CardPiles {
            hand,
            draw_pile,
            discard_pile,
            exhaust_pile,
            limbo: Vec::new(),
        },
        relics.clone(),
        observation.context.ascension,
        rng,
    );
    apply_public_counters(&mut combat, observation)?;
    combat.validate().map_err(sim_error)?;

    let mut run = RunState::fresh_combat_rollout_shell(observation.context.ascension);
    run.phase = RunPhase::Combat;
    run.deck = deck;
    run.player_hp = observation.player.hp;
    run.player_max_hp = observation.player.max_hp;
    run.gold = observation.context.gold;
    run.energy_per_turn = observation.player.max_energy;
    run.current_act = observation.context.act;
    run.current_floor = observation.context.floor;
    run.relics = relics;
    run.event_rng_seed = hypothesis.run_rng.event;
    run.reward_rng_seed = hypothesis.run_rng.reward;
    run.treasure_rng_seed = hypothesis.run_rng.treasure;
    run.potion_rng_seed = hypothesis.run_rng.potion;
    run.relic_rng_seed = hypothesis.run_rng.relic;
    run.shuffle_rng_seed = hypothesis.run_rng.shuffle;
    run.merchant_rng_seed = hypothesis.run_rng.merchant;
    run.misc_rng_seed = hypothesis.run_rng.misc;
    run.monster_rng_seed = hypothesis.run_rng.monster;
    run.combat = Some(combat);
    run.validate().map_err(sim_error)?;

    let projected = fair_combat_observation(&run)
        .map_err(|error| FairBeliefError::GeneratedStateInvalid(error.to_string()))?;
    if projected != *observation {
        return Err(FairBeliefError::ProjectionMismatch);
    }
    Ok(GeneratedCombatRollout { run })
}

fn sample_hypothesis(
    knowledge: &PublicCombatKnowledge,
    prior: &FairBeliefPrior,
    particle_index: usize,
    rng: &mut BeliefRng,
) -> Result<HiddenHypothesis, FairBeliefError> {
    if *prior != FairBeliefPrior::a0_act1_simple_combat() {
        return Err(FairBeliefError::UnsupportedPrior(
            "only the A0 Act 1 simple-combat prior is implemented",
        ));
    }
    let observation = knowledge.latest_observation();
    let draw_storage_order = if has_frozen_eye(observation) {
        known_top_to_storage_indices(
            &observation.draw_pile.cards,
            &observation.draw_pile.known_order,
        )?
    } else {
        shuffled_indices(
            observation.draw_pile.cards.len(),
            "belief.draw_pile_order",
            particle_index,
            rng,
        )
    };
    let discard_storage_order = shuffled_indices(
        observation.discard_pile.cards.len(),
        "belief.discard_pile_order",
        particle_index,
        rng,
    );
    let exhaust_storage_order = shuffled_indices(
        observation.exhaust_pile.cards.len(),
        "belief.exhaust_pile_order",
        particle_index,
        rng,
    );
    let run_seed = |rng: &mut BeliefRng, stream: &str| {
        rng.draw_u64(stream, &format!("particle.{particle_index}.initialize"))
    };
    let run_rng = GeneratedRunRngSeeds {
        event: run_seed(rng, "belief.run_rng.event"),
        reward: run_seed(rng, "belief.run_rng.reward"),
        treasure: run_seed(rng, "belief.run_rng.treasure"),
        potion: run_seed(rng, "belief.run_rng.potion"),
        relic: run_seed(rng, "belief.run_rng.relic"),
        shuffle: run_seed(rng, "belief.run_rng.shuffle"),
        merchant: run_seed(rng, "belief.run_rng.merchant"),
        misc: run_seed(rng, "belief.run_rng.misc"),
        monster: run_seed(rng, "belief.run_rng.monster"),
    };
    // Exchangeable-future approximation: combat streams are independently sampled
    // unrealized generators at counter zero. They are not the post-entry states of
    // `enter_combat_with_monsters`, and they are not derived from a master seed.
    let combat_rng = sample_independent_combat_rng(rng, particle_index);
    Ok(HiddenHypothesis {
        schema_version: FAIR_BELIEF_SCHEMA_VERSION,
        prior_version: prior.prior_version.clone(),
        draw_storage_order,
        discard_storage_order,
        exhaust_storage_order,
        combat_rng,
        run_rng,
    })
}

fn validate_supported_public_state(
    knowledge: &PublicCombatKnowledge,
    prior: &FairBeliefPrior,
) -> Result<(), FairBeliefError> {
    if knowledge.schema_version != FAIR_BELIEF_SCHEMA_VERSION
        || *prior != FairBeliefPrior::a0_act1_simple_combat()
    {
        return Err(FairBeliefError::InvalidSchema);
    }
    if !knowledge.history().is_empty() {
        return Err(FairBeliefError::MissingPublicHistory(
            "mid-combat public-history validation is not integrated yet",
        ));
    }
    let observation = knowledge.latest_observation();
    require_current_observation_schema(observation)?;
    if observation.context.floor < 0 {
        return Err(FairBeliefError::InvalidPublicObservation(
            "combat floor cannot be negative",
        ));
    }
    if observation.context.ascension != 0 || observation.context.act != 1 {
        return Err(FairBeliefError::UnsupportedRoot(
            "first fair materializer slice supports only A0 Act 1",
        ));
    }
    if observation.phase != FairCombatPhase::WaitingForPlayer {
        return Err(FairBeliefError::UnsupportedRoot(
            "first fair materializer slice supports only waiting-for-player roots",
        ));
    }
    if observation.selection.is_some() {
        return Err(FairBeliefError::UnsupportedMechanic(
            "active combat selections require queue/history reconstruction",
        ));
    }
    if !observation.player.powers.is_empty() || !observation.orb_slots.is_empty() {
        return Err(FairBeliefError::UnsupportedMechanic(
            "player powers and orbs are outside the simple-combat prior",
        ));
    }
    validate_pile(&observation.draw_pile, "draw")?;
    validate_pile(&observation.discard_pile, "discard")?;
    validate_pile(&observation.exhaust_pile, "exhaust")?;
    if !observation.discard_pile.known_order.is_empty()
        || !observation.exhaust_pile.known_order.is_empty()
    {
        return Err(FairBeliefError::InvalidPublicObservation(
            "discard/exhaust storage order is not public",
        ));
    }
    let frozen_eye = has_frozen_eye(observation);
    if frozen_eye {
        if observation.draw_pile.known_order.len() != observation.draw_pile.count {
            return Err(FairBeliefError::MissingPublicHistory(
                "Frozen Eye requires a complete public draw order",
            ));
        }
    } else if !observation.draw_pile.known_order.is_empty() {
        return Err(FairBeliefError::InvalidPublicObservation(
            "draw order is present without a supported visibility rule",
        ));
    }
    if observation
        .monsters
        .iter()
        .any(|monster| matches!(monster.intent, FairMonsterIntent::Hidden))
    {
        return Err(FairBeliefError::UnsupportedPrior(
            "hidden monster intent requires a source-backed move-table posterior",
        ));
    }
    if observation
        .potion_slots
        .iter()
        .any(|slot| slot.content_key.is_some())
    {
        return Err(FairBeliefError::UnsupportedMechanic(
            "combat potion paths are not authorized in the first materializer slice",
        ));
    }
    for item in &observation.hand {
        validate_supported_card(&item.card)?;
    }
    for pile in [
        &observation.draw_pile,
        &observation.discard_pile,
        &observation.exhaust_pile,
    ] {
        for card in &pile.cards {
            validate_supported_card(card)?;
        }
    }
    for relic in &observation.relics {
        if !relic.state.is_empty()
            || !matches!(relic.content_key.as_str(), "Burning Blood" | "Frozen Eye")
        {
            return Err(FairBeliefError::UnsupportedMechanic(
                "only stateless Burning Blood and Frozen Eye are supported",
            ));
        }
    }
    validate_monster_surface(knowledge)?;
    Ok(())
}

fn validate_monster_surface(knowledge: &PublicCombatKnowledge) -> Result<(), FairBeliefError> {
    let observation = knowledge.latest_observation();
    if observation.monsters.is_empty() {
        return Err(FairBeliefError::InvalidPublicObservation(
            "combat root has no monsters",
        ));
    }
    for (expected_slot, monster) in observation.monsters.iter().enumerate() {
        if monster.slot != expected_slot
            || !monster.powers.is_empty()
            || monster.stolen_gold != 0
            || monster.stasis_card.is_some()
            || monster.slime_size.is_some()
            || monster.escaped
            || monster.minion
            || monster.in_defensive_mode
            || monster.targetable != monster.alive
        {
            return Err(FairBeliefError::UnsupportedMechanic(
                "monster surface is outside the simple Cultist/fixed-monster prior",
            ));
        }
        if !matches!(
            monster.content_key.as_str(),
            "Cultist" | "Fixed Simple Monster"
        ) {
            return Err(FairBeliefError::UnsupportedPrior(
                "monster has no implemented private-state reconstruction rule",
            ));
        }
    }
    Ok(())
}

fn materialize_monsters(
    knowledge: &PublicCombatKnowledge,
) -> Result<Vec<crate::MonsterState>, FairBeliefError> {
    let observation = knowledge.latest_observation();
    observation
        .monsters
        .iter()
        .enumerate()
        .map(|(slot, public)| {
            let definition = monster_definition(&public.content_key)?;
            let mut monster = monster_state(definition, MonsterId::new(slot as u64 + 1));
            monster.hp = public.hp;
            monster.max_hp = public.max_hp;
            monster.block = public.block;
            monster.alive = public.alive;
            monster.escaped = public.escaped;
            let executed = public_monster_moves(knowledge, slot, &public.content_key)?;
            monster.moves_executed = executed.len() as u32;
            monster.move_history = if public.content_key == "Fixed Simple Monster" {
                // The deterministic test monster has no target move-byte table and does not read
                // history. Keep its synthetic history empty rather than inventing a source byte.
                Vec::new()
            } else {
                executed
                    .iter()
                    .map(|intent| {
                        target_move_byte(definition.content_id, *intent).ok_or(
                            FairBeliefError::UnsupportedPrior(
                                "public monster move has no source move-byte rule",
                            ),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            monster.intent =
                prepare_monster_intent_for_ascension(&monster, observation.context.ascension)
                    .map_err(sim_error)?;
            Ok(monster)
        })
        .collect()
}

fn public_monster_moves(
    knowledge: &PublicCombatKnowledge,
    slot: usize,
    content_key: &str,
) -> Result<Vec<MonsterIntent>, FairBeliefError> {
    let ascension = knowledge.latest_observation().context.ascension;
    let mut intents = Vec::new();
    for event in knowledge.history().iter().flat_map(|step| &step.events) {
        if let PublicCombatEvent::MonsterMoveExecuted {
            monster_slot,
            category,
        } = event
        {
            if usize::from(*monster_slot) != slot {
                continue;
            }
            let intent = match (content_key, category, intents.is_empty()) {
                ("Cultist", FairIntentCategory::Buff, true)
                | ("Cultist", FairIntentCategory::Attack, false)
                | ("Fixed Simple Monster", FairIntentCategory::Attack, _) => {
                    let mut reconstructed =
                        monster_state(monster_definition(content_key)?, MonsterId::new(1));
                    reconstructed.moves_executed = intents.len() as u32;
                    prepare_monster_intent_for_ascension(&reconstructed, ascension)
                        .map_err(sim_error)?
                }
                _ => {
                    return Err(FairBeliefError::UnsupportedPrior(
                        "public move history is incompatible with the simple monster rule",
                    ));
                }
            };
            intents.push(intent);
        }
    }
    Ok(intents)
}

fn monster_definition(
    content_key: &str,
) -> Result<&'static crate::content::monsters::MonsterDefinition, FairBeliefError> {
    match content_key {
        "Cultist" => Ok(&CULTIST_A0),
        "Fixed Simple Monster" => Ok(&FIXED_SIMPLE_MONSTER),
        _ => Err(FairBeliefError::UnsupportedPrior(
            "monster has no implemented reconstruction rule",
        )),
    }
}

fn materialize_relics(observation: &FairCombatObservation) -> Result<Vec<Relic>, FairBeliefError> {
    observation
        .relics
        .iter()
        .enumerate()
        .map(|(expected_slot, relic)| {
            if relic.slot != expected_slot {
                return Err(FairBeliefError::InvalidPublicObservation(
                    "relic slots are not contiguous public indices",
                ));
            }
            Relic::from_trace_name(&relic.content_key).ok_or(FairBeliefError::UnsupportedMechanic(
                "unknown public relic identity",
            ))
        })
        .collect()
}

fn apply_public_counters(
    combat: &mut CombatState,
    observation: &FairCombatObservation,
) -> Result<(), FairBeliefError> {
    let mut seen = BTreeMap::new();
    for counter in &observation.public_counters {
        if seen.insert(counter.key.as_str(), counter.value).is_some() || counter.value < 0 {
            return Err(FairBeliefError::InvalidPublicObservation(
                "public combat counters are duplicated or negative",
            ));
        }
    }
    let value = |key: &'static str| -> Result<u32, FairBeliefError> {
        let raw = *seen
            .get(key)
            .ok_or(FairBeliefError::InvalidPublicObservation(
                "required public combat counter is missing",
            ))?;
        u32::try_from(raw).map_err(|_| {
            FairBeliefError::InvalidPublicObservation("public combat counter exceeds u32")
        })
    };
    combat.relic_counters = RelicCounters::default();
    combat.relic_counters.cards_played_this_turn = value("cards_played_this_turn")?;
    combat.relic_counters.attacks_played_this_turn = value("attacks_played_this_turn")?;
    combat.total_discarded_this_turn = i32::try_from(value("cards_discarded_this_turn")?)
        .map_err(|_| FairBeliefError::InvalidPublicObservation("discard counter exceeds i32"))?;
    if seen.len() != 3 {
        return Err(FairBeliefError::InvalidPublicObservation(
            "unsupported public counter is present",
        ));
    }
    Ok(())
}

fn validate_supported_card(card: &FairCard) -> Result<(), FairBeliefError> {
    let definition = ALL_CARDS
        .iter()
        .find(|definition| definition.key == card.content_key)
        .ok_or(FairBeliefError::UnsupportedMechanic(
            "public card key is not in the authoritative card registry",
        ))?;
    if !matches!(definition.key, "Strike_R" | "Defend_R" | "Bash") {
        return Err(FairBeliefError::UnsupportedMechanic(
            "first materializer slice supports only the A0 starter card definitions",
        ));
    }
    if card.cost != i32::from(definition.cost)
        || card.cost_is_modified
        || card.cost_resets_next_turn
        || card.upgrade_level != 0
        || card.bottled
        || card.temporary
        || card.dynamic != Default::default()
    {
        return Err(FairBeliefError::UnsupportedMechanic(
            "combat-local card metadata lacks a reconstruction rule",
        ));
    }
    Ok(())
}

fn materialize_card(
    card: &FairCard,
    next_card_id: &mut u64,
) -> Result<CardInstance, FairBeliefError> {
    validate_supported_card(card)?;
    let definition = ALL_CARDS
        .iter()
        .find(|definition| definition.key == card.content_key)
        .expect("validated card key");
    let id = CardId::new(*next_card_id);
    *next_card_id =
        next_card_id
            .checked_add(1)
            .ok_or(FairBeliefError::InvalidPublicObservation(
                "card identity allocation overflowed",
            ))?;
    Ok(CardInstance::new(id, definition.id))
}

fn materialize_public_cards(
    cards: &[FairCard],
    next_card_id: &mut u64,
) -> Result<Vec<CardInstance>, FairBeliefError> {
    cards
        .iter()
        .map(|card| materialize_card(card, next_card_id))
        .collect()
}

fn validate_pile(pile: &crate::FairPile, label: &'static str) -> Result<(), FairBeliefError> {
    if pile.count != pile.cards.len() {
        return Err(FairBeliefError::InvalidPublicObservation(match label {
            "draw" => "draw pile count does not match its public multiset",
            "discard" => "discard pile count does not match its public multiset",
            _ => "exhaust pile count does not match its public multiset",
        }));
    }
    if pile.cards.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(FairBeliefError::InvalidPublicObservation(
            "public pile multiset is not canonically ordered",
        ));
    }
    Ok(())
}

fn require_current_observation_schema(
    observation: &FairCombatObservation,
) -> Result<(), FairBeliefError> {
    if observation.schema_version != crate::FAIR_COMBAT_OBSERVATION_SCHEMA_VERSION {
        return Err(FairBeliefError::InvalidSchema);
    }
    Ok(())
}

fn known_top_to_storage_indices(
    canonical: &[FairCard],
    known_top: &[FairCard],
) -> Result<Vec<usize>, FairBeliefError> {
    if canonical.len() != known_top.len() {
        return Err(FairBeliefError::MissingPublicHistory(
            "known draw order does not cover the public multiset",
        ));
    }
    let mut used = vec![false; canonical.len()];
    let mut top_indices = Vec::with_capacity(canonical.len());
    for card in known_top {
        let index = canonical
            .iter()
            .enumerate()
            .find_map(|(index, candidate)| (!used[index] && candidate == card).then_some(index))
            .ok_or(FairBeliefError::InvalidPublicObservation(
                "known draw order is not a permutation of the public multiset",
            ))?;
        used[index] = true;
        top_indices.push(index);
    }
    top_indices.reverse();
    Ok(top_indices)
}

fn shuffled_indices(
    len: usize,
    stream: &str,
    particle_index: usize,
    rng: &mut BeliefRng,
) -> Vec<usize> {
    let mut indices = (0..len).collect::<Vec<_>>();
    for index in (1..len).rev() {
        let call_site = format!("particle.{particle_index}.fisher_yates.{index}");
        let choice = rng.uniform_index(stream, &call_site, index + 1);
        indices.swap(index, choice);
    }
    indices
}

fn validate_permutation(order: &[usize], len: usize) -> Result<(), FairBeliefError> {
    if order.len() != len {
        return Err(FairBeliefError::InvalidHypothesis(
            "pile order length does not match public pile size",
        ));
    }
    let mut seen = vec![false; len];
    for index in order {
        if *index >= len || seen[*index] {
            return Err(FairBeliefError::InvalidHypothesis(
                "pile order is not a permutation",
            ));
        }
        seen[*index] = true;
    }
    Ok(())
}

fn apply_storage_order(cards: &[CardInstance], order: &[usize]) -> Vec<CardInstance> {
    order.iter().map(|index| cards[*index]).collect()
}

fn sample_independent_combat_rng(
    rng: &mut BeliefRng,
    particle_index: usize,
) -> GeneratedCombatRngStates {
    loop {
        let sampled = GeneratedCombatRngStates {
            shuffle: sample_rng_state(rng, "belief.combat_rng.shuffle", particle_index),
            monster: sample_rng_state(rng, "belief.combat_rng.monster", particle_index),
            monster_hp: sample_rng_state(rng, "belief.combat_rng.monster_hp", particle_index),
            card_random: sample_rng_state(rng, "belief.combat_rng.card_random", particle_index),
        };
        let states = [
            sampled.shuffle,
            sampled.monster,
            sampled.monster_hp,
            sampled.card_random,
        ];
        if states.iter().collect::<BTreeSet<_>>().len() == 4 {
            return sampled;
        }
    }
}

fn sample_rng_state(rng: &mut BeliefRng, stream: &str, particle_index: usize) -> GeneratedRngState {
    let call = |field: &str| format!("particle.{particle_index}.{field}");
    loop {
        let seed0 = rng.draw_u64(stream, &call("seed0"));
        let seed1 = rng.draw_u64(stream, &call("seed1"));
        if seed0 != 0 || seed1 != 0 {
            return GeneratedRngState {
                seed0,
                seed1,
                counter: 0,
            };
        }
    }
}

fn validate_generated_rng_hypothesis(hypothesis: &HiddenHypothesis) -> Result<(), FairBeliefError> {
    let states = [
        hypothesis.combat_rng.shuffle,
        hypothesis.combat_rng.monster,
        hypothesis.combat_rng.monster_hp,
        hypothesis.combat_rng.card_random,
    ];
    if states
        .iter()
        .any(|state| state.counter != 0 || (state.seed0 == 0 && state.seed1 == 0))
    {
        return Err(FairBeliefError::InvalidHypothesis(
            "generated combat RNG must have a nonzero raw state at counter zero",
        ));
    }
    if states.iter().collect::<BTreeSet<_>>().len() != 4 {
        return Err(FairBeliefError::InvalidHypothesis(
            "generated combat RNG streams must be independently distinct",
        ));
    }
    Ok(())
}

fn generated_sts_rng(state: GeneratedRngState, stream: RngTraceStream) -> StsRng {
    StsRng::from_raw_state(state.seed0, state.seed1, state.counter).for_stream(stream)
}

fn has_frozen_eye(observation: &FairCombatObservation) -> bool {
    observation
        .relics
        .iter()
        .any(|relic| relic.content_key == "Frozen Eye")
}

fn sim_error(error: SimError) -> FairBeliefError {
    FairBeliefError::GeneratedStateInvalid(error.to_string())
}

fn stable_name_hash(name: &str) -> u64 {
    name.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::FairSelection,
        content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
        run::RunState,
    };
    use std::collections::BTreeSet;

    fn knowledge_from(observation: FairCombatObservation) -> PublicCombatKnowledge {
        let start = PublicCombatStart::bind(observation.clone());
        PublicCombatKnowledge::at_first_player_decision(observation, start).expect("public root")
    }

    fn public_card(observation: &FairCombatObservation) -> FairCard {
        observation.hand[0].card.clone()
    }

    fn knowledge_with_public_vocabulary() -> PublicCombatKnowledge {
        let mut knowledge = ordinary_knowledge();
        let observation = knowledge.latest_observation().clone();
        let card = public_card(&observation);
        let step = |action: PlayerChoice, events: Vec<PublicCombatEvent>| PublicCombatStep {
            action,
            events,
            observation: observation.clone(),
        };
        knowledge.history = vec![
            step(
                PlayerChoice::PlayHandSlot {
                    hand_slot: 0,
                    target_slot: Some(0),
                },
                vec![
                    PublicCombatEvent::CardDrawn { card: card.clone() },
                    PublicCombatEvent::CardPlayed { hand_slot: 0 },
                    PublicCombatEvent::CardMoved {
                        card: card.clone(),
                        from: PublicCardZone::Hand,
                        to: PublicCardZone::Discard,
                    },
                    PublicCombatEvent::PileShuffled,
                    PublicCombatEvent::MonsterMoveExecuted {
                        monster_slot: 0,
                        category: FairIntentCategory::Attack,
                    },
                    PublicCombatEvent::TurnStarted { turn: 2 },
                ],
            ),
            step(PlayerChoice::EndTurn, Vec::new()),
            step(
                PlayerChoice::UsePotionSlot {
                    potion_slot: 0,
                    target_slot: None,
                },
                Vec::new(),
            ),
            step(
                PlayerChoice::DiscardPotionSlot { potion_slot: 1 },
                Vec::new(),
            ),
            step(
                PlayerChoice::ToggleVisibleCard { option_slot: 0 },
                Vec::new(),
            ),
            step(
                PlayerChoice::ChooseVisibleOption { option_slot: 1 },
                Vec::new(),
            ),
            step(PlayerChoice::ConfirmSelection, Vec::new()),
            step(PlayerChoice::SkipSelection, Vec::new()),
            step(PlayerChoice::Proceed, Vec::new()),
        ];
        knowledge
    }

    fn observation_with_relics(relics: Vec<Relic>) -> FairCombatObservation {
        let run = RunState::combat_fixture_with_relics(relics);
        fair_combat_observation(&run).expect("fixture projects")
    }

    fn ordinary_knowledge() -> PublicCombatKnowledge {
        knowledge_from(observation_with_relics(Vec::new()))
    }

    fn diverse_pile_run(relics: Vec<Relic>) -> RunState {
        let mut run = RunState::combat_fixture_with_relics(relics);
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.draw_pile = vec![
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
            CardInstance::new(CardId::new(6), STRIKE_R_ID),
            CardInstance::new(CardId::new(7), DEFEND_R_ID),
            CardInstance::new(CardId::new(8), DEFEND_R_ID),
            CardInstance::new(CardId::new(9), BASH_ID),
        ];
        combat.piles.discard_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
        ];
        combat.piles.exhaust_pile = vec![
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
            CardInstance::new(CardId::new(13), DEFEND_R_ID),
        ];
        run.validate().expect("diverse pile fixture validates");
        run
    }

    fn diverse_knowledge(relics: Vec<Relic>) -> PublicCombatKnowledge {
        let observation =
            fair_combat_observation(&diverse_pile_run(relics)).expect("diverse fixture projects");
        assert!(observation.draw_pile.cards.len() >= 5);
        knowledge_from(observation)
    }

    fn id_content(run: &RunState) -> Vec<(u64, u64)> {
        let combat = run.combat.as_ref().expect("combat");
        let mut rows = combat
            .piles
            .hand
            .iter()
            .chain(&combat.piles.draw_pile)
            .chain(&combat.piles.discard_pile)
            .chain(&combat.piles.exhaust_pile)
            .map(|card| (card.id.get(), card.content_id.get()))
            .collect::<Vec<_>>();
        rows.sort_unstable();
        rows
    }

    fn content_keys(cards: impl IntoIterator<Item = CardInstance>) -> Vec<&'static str> {
        cards
            .into_iter()
            .map(|card| {
                ALL_CARDS
                    .iter()
                    .find(|definition| definition.id == card.content_id)
                    .expect("generated card is in the registry")
                    .key
            })
            .collect()
    }

    fn hidden_signature(run: &RunState) -> (Vec<u64>, [(u64, u64); 4]) {
        let combat = run.combat.as_ref().expect("generated combat");
        let order = combat
            .piles
            .draw_pile
            .iter()
            .map(|card| card.id.get())
            .collect();
        let states = [
            combat.rng.shuffle_rng.state(),
            combat.rng.monster_rng.state(),
            combat.rng.monster_hp_rng.state(),
            combat.rng.card_random_rng.state(),
        ];
        (order, states)
    }

    #[test]
    fn belief_generation_is_fresh_deterministic_and_projection_exact() {
        let knowledge = ordinary_knowledge();
        let first = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            16,
            71,
        )
        .expect("belief initializes");
        let second = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            16,
            71,
        )
        .expect("belief initializes deterministically");
        assert_eq!(first, second);

        let serialized = (0..first.particle_count())
            .map(|index| {
                let generated =
                    materialize_combat_rollout(&first, index).expect("particle materializes");
                assert_eq!(generated.observation(), *knowledge.latest_observation());
                serde_json::to_string(generated.authoritative_state()).expect("run serializes")
            })
            .collect::<Vec<_>>();
        let serialized_again = (0..second.particle_count())
            .map(|index| {
                let generated =
                    materialize_combat_rollout(&second, index).expect("particle materializes");
                serde_json::to_string(generated.authoritative_state()).expect("run serializes")
            })
            .collect::<Vec<_>>();
        assert_eq!(serialized, serialized_again);
    }

    #[test]
    fn particles_vary_hidden_order_and_rng_but_not_public_state_or_ids() {
        let knowledge = diverse_knowledge(Vec::new());
        let belief = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            32,
            83,
        )
        .expect("belief initializes");
        let other_seed = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            32,
            84,
        )
        .expect("second belief seed initializes");
        assert_ne!(belief.particles, other_seed.particles);
        let generated = (0..belief.particle_count())
            .map(|index| belief.materialize(index).expect("particle materializes"))
            .collect::<Vec<_>>();
        let signatures = generated
            .iter()
            .map(|rollout| hidden_signature(rollout.authoritative_state()))
            .collect::<BTreeSet<_>>();
        assert!(signatures.len() > 1);
        let draw_orders = belief
            .particles
            .iter()
            .map(|particle| particle.hypothesis.draw_storage_order.clone())
            .collect::<BTreeSet<_>>();
        let discard_orders = belief
            .particles
            .iter()
            .map(|particle| particle.hypothesis.discard_storage_order.clone())
            .collect::<BTreeSet<_>>();
        let exhaust_orders = belief
            .particles
            .iter()
            .map(|particle| particle.hypothesis.exhaust_storage_order.clone())
            .collect::<BTreeSet<_>>();
        assert!(
            draw_orders.len() > 8,
            "five-card draw pile must produce permutation diversity, got {}",
            draw_orders.len()
        );
        assert!(
            discard_orders.len() > 1,
            "two-card discard pile must vary storage order"
        );
        assert!(
            exhaust_orders.len() > 1,
            "two-card exhaust pile must vary storage order"
        );
        let expected_ids = id_content(generated[0].authoritative_state());
        assert!(generated
            .iter()
            .all(|rollout| id_content(rollout.authoritative_state()) == expected_ids));
        assert!(generated
            .iter()
            .all(|rollout| rollout.observation() == *knowledge.latest_observation()));
    }

    #[test]
    fn frozen_eye_fixes_draw_but_not_other_hidden_pile_orders() {
        let knowledge = diverse_knowledge(vec![Relic::FrozenEye]);
        assert!(knowledge.latest_observation().draw_pile.cards.len() >= 5);
        assert_eq!(
            knowledge.latest_observation().draw_pile.known_order.len(),
            knowledge.latest_observation().draw_pile.cards.len()
        );
        let belief = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            48,
            97,
        )
        .expect("belief initializes");
        let draw_orders = belief
            .particles
            .iter()
            .map(|particle| particle.hypothesis.draw_storage_order.clone())
            .collect::<BTreeSet<_>>();
        let discard_orders = belief
            .particles
            .iter()
            .map(|particle| particle.hypothesis.discard_storage_order.clone())
            .collect::<BTreeSet<_>>();
        let exhaust_orders = belief
            .particles
            .iter()
            .map(|particle| particle.hypothesis.exhaust_storage_order.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(draw_orders.len(), 1);
        assert!(discard_orders.len() > 1);
        assert!(exhaust_orders.len() > 1);
        let generated = (0..belief.particle_count())
            .map(|index| {
                belief
                    .materialize(index)
                    .expect("Frozen Eye particle materializes")
            })
            .collect::<Vec<_>>();
        let expected_ids = id_content(generated[0].authoritative_state());
        let known = &knowledge.latest_observation().draw_pile.known_order;
        for rollout in &generated {
            assert_eq!(rollout.observation(), *knowledge.latest_observation());
            assert_eq!(id_content(rollout.authoritative_state()), expected_ids);
            let storage = &rollout
                .authoritative_state()
                .combat
                .as_ref()
                .expect("generated combat")
                .piles
                .draw_pile;
            let top_to_bottom = content_keys(storage.iter().rev().copied());
            let known_keys = known
                .iter()
                .map(|card| card.content_key.as_str())
                .collect::<Vec<_>>();
            assert_eq!(top_to_bottom, known_keys);
        }
    }

    #[test]
    fn combat_rng_is_an_independent_exchangeable_future_approximation() {
        let knowledge = ordinary_knowledge();
        let belief = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            1,
            101,
        )
        .expect("belief initializes");
        let hypothesis = &belief.particles[0].hypothesis;
        let states = [
            hypothesis.combat_rng.shuffle,
            hypothesis.combat_rng.monster,
            hypothesis.combat_rng.monster_hp,
            hypothesis.combat_rng.card_random,
        ];
        assert_eq!(states.iter().collect::<BTreeSet<_>>().len(), 4);
        assert!(states.iter().all(|state| state.counter == 0));
        assert_ne!(
            hypothesis.combat_rng.shuffle, hypothesis.combat_rng.monster_hp,
            "exchangeable combat streams are not the shared event+floor entry pair"
        );
        let run_seeds = [
            hypothesis.run_rng.event,
            hypothesis.run_rng.reward,
            hypothesis.run_rng.treasure,
            hypothesis.run_rng.potion,
            hypothesis.run_rng.relic,
            hypothesis.run_rng.shuffle,
            hypothesis.run_rng.merchant,
            hypothesis.run_rng.misc,
            hypothesis.run_rng.monster,
        ];
        assert_eq!(run_seeds.into_iter().collect::<BTreeSet<_>>().len(), 9);
        let generated = belief.materialize(0).expect("particle materializes");
        let state = generated.authoritative_state();
        assert_eq!(state.event_rng_seed, hypothesis.run_rng.event);
        assert_eq!(state.reward_rng_seed, hypothesis.run_rng.reward);
        assert_eq!(state.potion_rng_seed, hypothesis.run_rng.potion);
        assert_eq!(state.misc_rng_seed, hypothesis.run_rng.misc);
        let combat = state.combat.as_ref().expect("generated combat");
        assert_eq!(
            combat.rng.shuffle_rng.state(),
            (
                hypothesis.combat_rng.shuffle.seed0,
                hypothesis.combat_rng.shuffle.seed1
            )
        );
        assert_eq!(
            combat.rng.card_random_rng.state(),
            (
                hypothesis.combat_rng.card_random.seed0,
                hypothesis.combat_rng.card_random.seed1
            )
        );
    }

    #[test]
    fn missing_history_hidden_intent_selection_and_bad_public_card_fail_closed() {
        let observation = observation_with_relics(Vec::new());
        assert!(matches!(
            PublicCombatKnowledge::refuse_unproven_root(observation.clone()),
            Err(FairBeliefError::MissingPublicHistory(_))
        ));

        let mut hidden_intent = observation.clone();
        hidden_intent.monsters[0].intent = FairMonsterIntent::Hidden;
        let hidden_knowledge = knowledge_from(hidden_intent);
        assert!(matches!(
            FairBelief::initialize(
                hidden_knowledge,
                FairBeliefPrior::a0_act1_simple_combat(),
                1,
                1
            ),
            Err(FairBeliefError::UnsupportedPrior(_))
        ));

        let mut selection = observation.clone();
        selection.selection = Some(FairSelection {
            kind: crate::FairSelectionKind::ArmamentsUpgrade,
            options: Vec::new(),
            selected_slots: Vec::new(),
        });
        let selection_knowledge = knowledge_from(selection);
        assert!(matches!(
            FairBelief::initialize(
                selection_knowledge,
                FairBeliefPrior::a0_act1_simple_combat(),
                1,
                2
            ),
            Err(FairBeliefError::UnsupportedMechanic(_))
        ));

        let mut bad_cost = observation;
        bad_cost.hand[0].card.cost += 1;
        let bad_knowledge = knowledge_from(bad_cost);
        assert!(matches!(
            FairBelief::initialize(
                bad_knowledge,
                FairBeliefPrior::a0_act1_simple_combat(),
                1,
                3
            ),
            Err(FairBeliefError::UnsupportedMechanic(_))
        ));
    }

    #[test]
    fn malformed_hypothesis_is_rejected_instead_of_repaired() {
        let knowledge = ordinary_knowledge();
        let belief = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            1,
            109,
        )
        .expect("belief initializes");
        let original = &belief.particles[0].hypothesis;

        let mut bad_order = original.clone();
        bad_order.draw_storage_order.clear();
        assert!(matches!(
            materialize_owned_hypothesis(&knowledge, &bad_order),
            Err(FairBeliefError::InvalidHypothesis(_))
        ));

        let mut advanced_rng = original.clone();
        advanced_rng.combat_rng.shuffle.counter = 1;
        assert!(matches!(
            materialize_owned_hypothesis(&knowledge, &advanced_rng),
            Err(FairBeliefError::InvalidHypothesis(_))
        ));

        let mut duplicated = original.clone();
        duplicated.combat_rng.monster_hp = duplicated.combat_rng.shuffle;
        assert!(matches!(
            materialize_owned_hypothesis(&knowledge, &duplicated),
            Err(FairBeliefError::InvalidHypothesis(_))
        ));
        assert!(matches!(
            materialize_combat_rollout(&belief, 1),
            Err(FairBeliefError::UnknownParticle)
        ));
    }

    #[test]
    fn card_random_authority_is_continuous_across_combat_horizon_steps() {
        let knowledge = ordinary_knowledge();
        let belief = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            1,
            117,
        )
        .expect("belief initializes");
        let generated = belief.materialize(0).expect("particle materializes");
        let before_rng = generated
            .authoritative_state()
            .combat
            .as_ref()
            .expect("generated combat")
            .rng
            .card_random_rng
            .clone();
        let after = generated
            .apply_combat_action(crate::CombatAction::EndTurn)
            .expect("generated rollout advances");
        assert!(after.is_active_combat());
        let after_rng = &after
            .authoritative_state()
            .combat
            .as_ref()
            .expect("combat remains active")
            .rng
            .card_random_rng;
        assert_eq!(after_rng.state(), before_rng.state());
        assert_eq!(after_rng.counter(), before_rng.counter());
    }

    #[test]
    fn particle_weights_are_nonzero_and_hypotheses_are_not_public_json() {
        let knowledge = ordinary_knowledge();
        let belief =
            FairBelief::initialize(knowledge, FairBeliefPrior::a0_act1_simple_combat(), 2, 119)
                .expect("belief initializes");
        assert!(belief
            .particles
            .iter()
            .all(|particle| particle.weight.get() > 0));
        let json = serde_json::to_value(&belief).expect("belief serializes");
        crate::combat::fair_json_allowlist::check_schema(
            &json,
            &crate::combat::fair_json_allowlist::FAIR_BELIEF_SCHEMA,
            "$",
        )
        .expect("initialized belief matches the path-sensitive allowlist");
    }

    #[test]
    fn opening_cultist_root_materializes_without_true_state_input() {
        let mut run = RunState::combat_fixture_with_relics(vec![Relic::BurningBlood]);
        run.combat = Some(CombatState::cultist_fixture());
        run.combat.as_mut().expect("combat").relics = vec![Relic::BurningBlood];
        run.validate().expect("Cultist oracle fixture validates");
        let observation = fair_combat_observation(&run).expect("Cultist projects");
        let knowledge = knowledge_from(observation);
        let belief = FairBelief::initialize(
            knowledge.clone(),
            FairBeliefPrior::a0_act1_simple_combat(),
            4,
            121,
        )
        .expect("opening Cultist prior is supported");
        for index in 0..belief.particle_count() {
            let generated = belief
                .materialize(index)
                .expect("Cultist particle materializes");
            assert_eq!(generated.observation(), *knowledge.latest_observation());
        }
    }

    #[test]
    fn mid_combat_observation_is_refused_until_history_capability_is_integrated() {
        let run = RunState::combat_fixture();
        let next = crate::apply_combat_action_on_run(&run, crate::CombatAction::EndTurn)
            .expect("public action advances the oracle fixture");
        let observation = fair_combat_observation(&next).expect("next observation");
        assert!(matches!(
            PublicCombatKnowledge::refuse_unproven_root(observation),
            Err(FairBeliefError::MissingPublicHistory(_))
        ));
    }

    #[test]
    fn generated_belief_serialization_uses_a_path_sensitive_allowlist() {
        let knowledge = diverse_knowledge(vec![Relic::FrozenEye]);
        let mut belief =
            FairBelief::initialize(knowledge, FairBeliefPrior::a0_act1_simple_combat(), 1, 113)
                .expect("belief initializes");
        let empty_history = serde_json::to_value(&belief).expect("belief serializes");
        crate::combat::fair_json_allowlist::check_schema(
            &empty_history,
            &crate::combat::fair_json_allowlist::FAIR_BELIEF_SCHEMA,
            "$",
        )
        .expect("initialized belief matches the path-sensitive allowlist");

        belief.knowledge = knowledge_with_public_vocabulary();
        let with_history = serde_json::to_value(&belief).expect("belief with history serializes");
        crate::combat::fair_json_allowlist::check_schema(
            &with_history,
            &crate::combat::fair_json_allowlist::FAIR_BELIEF_SCHEMA,
            "$",
        )
        .expect("public event and choice variants match the path-sensitive allowlist");

        let mut leaked = with_history;
        leaked["knowledge"]["seed"] = serde_json::json!(1);
        let error = crate::combat::fair_json_allowlist::check_schema(
            &leaked,
            &crate::combat::fair_json_allowlist::FAIR_BELIEF_SCHEMA,
            "$",
        )
        .expect_err("generic key seed is not allowlisted on knowledge");
        assert!(
            error.contains("knowledge.seed"),
            "path-sensitive allowlist should reject $.knowledge.seed, got {error}"
        );
        leaked = serde_json::to_value(&belief).expect("belief serializes");
        leaked["knowledge"]["state"] = serde_json::json!({"value": 1});
        let error = crate::combat::fair_json_allowlist::check_schema(
            &leaked,
            &crate::combat::fair_json_allowlist::FAIR_BELIEF_SCHEMA,
            "$",
        )
        .expect_err("generic key state is not allowlisted on knowledge");
        assert!(
            error.contains("knowledge.state"),
            "path-sensitive allowlist should reject $.knowledge.state, got {error}"
        );
    }

    #[test]
    fn rollout_stepping_refuses_a_terminated_combat() {
        let mut observation = observation_with_relics(Vec::new());
        observation.monsters[0].hp = 1;
        let knowledge = knowledge_from(observation);
        let belief =
            FairBelief::initialize(knowledge, FairBeliefPrior::a0_act1_simple_combat(), 1, 131)
                .expect("belief initializes");
        let generated = belief.materialize(0).expect("particle materializes");
        assert!(generated.is_active_combat());
        let combat = generated
            .authoritative_state()
            .combat
            .as_ref()
            .expect("combat");
        let strike = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == STRIKE_R_ID)
            .expect("strike in hand");
        let target = combat.monsters[0].id;
        let after = generated
            .apply_combat_action(CombatAction::PlayCard {
                card_id: strike.id,
                target: Some(target),
            })
            .expect("lethal strike applies");
        assert!(!after.is_active_combat());
        assert!(matches!(
            after.apply_combat_action(CombatAction::EndTurn),
            Err(FairBeliefError::CombatTerminated)
        ));
    }

    #[test]
    fn debug_output_does_not_expose_hypotheses_or_run_state() {
        let knowledge = ordinary_knowledge();
        let belief =
            FairBelief::initialize(knowledge, FairBeliefPrior::a0_act1_simple_combat(), 2, 139)
                .expect("belief initializes");
        let generated = belief.materialize(0).expect("particle materializes");
        let belief_debug = format!("{belief:?}");
        let rollout_debug = format!("{generated:?}");
        for leaked in [
            "draw_storage_order",
            "HiddenHypothesis",
            "seed0",
            "shuffle_rng",
            "particles",
            "RunState",
        ] {
            assert!(
                !belief_debug.contains(leaked),
                "FairBelief debug leaked {leaked}: {belief_debug}"
            );
            assert!(
                !rollout_debug.contains(leaked),
                "GeneratedCombatRollout debug leaked {leaked}: {rollout_debug}"
            );
        }
        assert!(belief_debug.contains("particle_count"));
        assert!(rollout_debug.contains("active_combat"));
    }

    #[test]
    fn initialize_rejects_unbounded_particle_counts() {
        let knowledge = ordinary_knowledge();
        assert!(matches!(
            FairBelief::initialize(
                knowledge.clone(),
                FairBeliefPrior::a0_act1_simple_combat(),
                0,
                1
            ),
            Err(FairBeliefError::InvalidParticleCount)
        ));
        assert!(matches!(
            FairBelief::initialize(
                knowledge,
                FairBeliefPrior::a0_act1_simple_combat(),
                usize::MAX,
                1
            ),
            Err(FairBeliefError::InvalidParticleCount)
        ));
    }

    #[test]
    fn first_decision_token_is_bound_to_one_observation() {
        let first = observation_with_relics(Vec::new());
        let mut second = first.clone();
        second.player.block += 1;
        let start = PublicCombatStart::bind(first.clone());
        assert!(matches!(
            PublicCombatKnowledge::at_first_player_decision(second, start),
            Err(FairBeliefError::InvalidPublicObservation(_))
        ));
        let rebound = PublicCombatStart::bind(first.clone());
        PublicCombatKnowledge::at_first_player_decision(first, rebound)
            .expect("matching token is accepted");
    }
}
