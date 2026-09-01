//! Naive deterministic privileged PUCT over public combat choices.
//!
//! Search clones authoritative `RunState` values. The leaf evaluator receives
//! only a detached fair observation and the public choice list. Both the
//! simulation and transition budgets must be positive; every search stops at
//! the first exhausted bound. `c_puct` must be finite and positive.
//!
//! Revisiting an already expanded terminal is standard MCTS backup: the stored
//! terminal value is applied again and visit counts increment without consuming
//! a transition. Highest-visit action selection can therefore overweight short
//! terminal lines relative to longer unfinished branches.
//!
//! Public `puct_search` does not memoize leaves. Exact-state memoization is
//! opt-in via `PuctLeafCache::ExactState` and is used only at deterministic
//! privileged teacher/network call sites. The cache key is the complete
//! serialized `RunState` byte vector, including hidden RNG and queues, not a
//! digest and not a fair observation. Equal keys are exact, so the entry may
//! store legal-choice projection plus leaf output: `player_choices` is
//! deterministic and RNG-free. Tree nodes are not merged, visit accounting
//! stays per-edge, and observationally equal hidden states stay distinct.
//! Memoization requires a deterministic pure evaluator for that exact state.
//! The map lives in one search call, is bounded by unique expanded states
//! under the simulation budget (~10 KB of JSON per unique state), and is
//! never shared across searches. `fair_leaf_batch_v1` is intentionally
//! batch-size 1 and not an extensible request protocol; request/response
//! correlation ids are deferred.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use sts_core::content::cards::CONCLUDE_ANY_COLOR_ID;
use sts_core::content::monsters::TIME_EATER_ID;
use sts_core::{
    apply_run_decision_action, fair_combat_observation, player_choices, potion_key,
    resolve_player_choice, CombatAction, CombatPhase, DecisionRevision, FairCombatObservation,
    PlayerChoice, PlayerChoiceRequest, Potion, RunAction, RunDecisionAction, RunPhase, RunState,
};

pub const FAIR_LEAF_BATCH_SCHEMA: &str = "fair_leaf_batch_v1";
pub const PRIVILEGED_PUCT_TEACHER_NAME: &str = "privileged_puct";
pub const PRIVILEGED_PUCT_TEACHER_VERSION: &str = "synchronous_batch1_v3";

const SEARCH_REVISION: DecisionRevision = DecisionRevision::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatProxyConfig {
    pub name: String,
    pub version: u32,
    pub win_base: f64,
    pub escape_base: f64,
    pub loss_value: f64,
    pub hp_fraction_weight: f64,
    pub max_hp_change_per_ten_weight: f64,
    pub gold_change_per_hundred_weight: f64,
    pub potion_weight: f64,
    pub resource_clip: f64,
}

impl Default for CombatProxyConfig {
    fn default() -> Self {
        Self {
            name: "combat_proxy_v1".to_owned(),
            version: 1,
            win_base: 0.75,
            escape_base: 0.25,
            loss_value: -1.0,
            hp_fraction_weight: 0.20,
            max_hp_change_per_ten_weight: 0.01,
            gold_change_per_hundred_weight: 0.01,
            potion_weight: 0.01,
            resource_clip: 0.20,
        }
    }
}

impl CombatProxyConfig {
    pub fn validate(&self) -> Result<(), PuctError> {
        if self.name != "combat_proxy_v1" || self.version != 1 {
            return Err(PuctError::InvalidConfig(
                "unsupported combat reward contract".to_owned(),
            ));
        }
        let numeric = [
            self.win_base,
            self.escape_base,
            self.loss_value,
            self.hp_fraction_weight,
            self.max_hp_change_per_ten_weight,
            self.gold_change_per_hundred_weight,
            self.potion_weight,
            self.resource_clip,
        ];
        if numeric.iter().any(|value| !value.is_finite()) {
            return Err(PuctError::InvalidConfig(
                "reward coefficients must be finite".to_owned(),
            ));
        }
        if !(0.0 < self.resource_clip && self.resource_clip < 0.25) {
            return Err(PuctError::InvalidConfig(
                "resource clip must preserve disjoint status bands".to_owned(),
            ));
        }
        if self.win_base - self.resource_clip <= self.escape_base + self.resource_clip {
            return Err(PuctError::InvalidConfig(
                "win and escape reward bands overlap".to_owned(),
            ));
        }
        if self.loss_value < -1.0 || self.loss_value >= self.escape_base - self.resource_clip {
            return Err(PuctError::InvalidConfig(
                "loss reward must remain below escape".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn value(
        &self,
        status: &str,
        terminal_hp: i32,
        terminal_max_hp: i32,
        max_hp_change: i32,
        gold_change: i32,
        remaining_potions: usize,
    ) -> Result<f64, PuctError> {
        if status == "lost" {
            return Ok(self.loss_value);
        }
        if status != "won" && status != "escaped" {
            return Err(PuctError::InvalidConfig(format!(
                "unknown combat outcome: {status}"
            )));
        }
        if terminal_max_hp <= 0 {
            return Err(PuctError::InvalidConfig(
                "terminal max HP must be positive".to_owned(),
            ));
        }
        let hp_fraction = f64::from(terminal_hp) / f64::from(terminal_max_hp);
        let resource = hp_fraction * self.hp_fraction_weight
            + (f64::from(max_hp_change) / 10.0) * self.max_hp_change_per_ten_weight
            + (f64::from(gold_change) / 100.0) * self.gold_change_per_hundred_weight
            + remaining_potions as f64 * self.potion_weight;
        let resource = resource.clamp(-self.resource_clip, self.resource_clip);
        let base = if status == "won" {
            self.win_base
        } else {
            self.escape_base
        };
        Ok((base + resource).clamp(-1.0, 1.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PuctConfig {
    pub c_puct: f64,
    pub simulation_budget: usize,
    pub transition_budget: usize,
    pub reward: CombatProxyConfig,
    pub episode_root_max_hp: i32,
    pub episode_root_gold: i32,
}

impl PuctConfig {
    pub fn validate(&self) -> Result<(), PuctError> {
        if !self.c_puct.is_finite() || self.c_puct <= 0.0 {
            return Err(PuctError::InvalidConfig(
                "c_puct must be finite and positive".to_owned(),
            ));
        }
        if self.simulation_budget == 0 {
            return Err(PuctError::InvalidConfig(
                "simulation_budget must be positive".to_owned(),
            ));
        }
        if self.transition_budget == 0 {
            return Err(PuctError::InvalidConfig(
                "transition_budget must be positive".to_owned(),
            ));
        }
        if self.episode_root_max_hp <= 0 {
            return Err(PuctError::InvalidConfig(
                "episode root max HP must be positive".to_owned(),
            ));
        }
        if self.episode_root_gold < 0 {
            return Err(PuctError::InvalidConfig(
                "episode root gold must be nonnegative".to_owned(),
            ));
        }
        self.reward.validate()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FairLeafEvaluation {
    pub priors: Vec<f64>,
    pub value: f64,
}

pub trait FairLeafEvaluator {
    fn evaluate(
        &mut self,
        observation: &FairCombatObservation,
        choices: &[PlayerChoice],
    ) -> Result<FairLeafEvaluation, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PuctStopReason {
    SimulationBudget,
    TransitionBudget,
}

impl PuctStopReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SimulationBudget => "simulation_budget",
            Self::TransitionBudget => "transition_budget",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PuctSearchResult {
    pub selected_index: usize,
    pub selected_choice: PlayerChoice,
    pub selected_action: RunDecisionAction,
    pub visits: Vec<u64>,
    pub priors: Vec<f64>,
    pub value: f64,
    pub transitions: usize,
    pub completed_simulations: usize,
    /// Evaluator invocations for this search. Diagnostic only: teacher labels
    /// (visits, selected choice/action, priors, backed-up value) do not include
    /// this count. Exact-state memoization may lower it without changing labels.
    pub leaf_evaluations: usize,
    pub stop_reason: PuctStopReason,
    pub choices: Vec<PlayerChoice>,
}

/// Leaf memoization keyed by complete authoritative `RunState` identity.
///
/// The library default is [`PuctLeafCache::Off`]. Privileged teacher/network
/// search opts into [`PuctLeafCache::ExactState`] explicitly after the
/// evaluator is known to be deterministic and pure for an exact state.
///
/// Exact-state caching reuses legal-choice projection and evaluator output for
/// a later expand of the same complete byte identity. It does not merge tree
/// nodes, change visit accounting, or treat observationally equal hidden
/// states as identical. Entries are dropped when the search returns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PuctLeafCache {
    #[default]
    Off,
    ExactState,
}

/// Nanosecond timings and identity counters for one profiled `puct_search`.
///
/// Hit/miss and identity duplicate counters are recorded only when profiling
/// is enabled. Unprofiled searches do not accumulate them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PuctSearchProfile {
    pub total_ns: u64,
    pub clone_ns: u64,
    pub clone_count: u64,
    pub apply_ns: u64,
    pub apply_count: u64,
    pub observation_ns: u64,
    pub observation_count: u64,
    pub evaluate_ns: u64,
    pub evaluate_count: u64,
    pub choice_projection_ns: u64,
    pub choice_projection_count: u64,
    pub select_ns: u64,
    pub backup_ns: u64,
    pub identity_ns: u64,
    pub identity_count: u64,
    pub exact_state_cache_hits: u64,
    pub exact_state_cache_misses: u64,
    pub unique_exact_states: u64,
    pub duplicate_exact_states: u64,
    pub unique_observation_choice_keys: u64,
    pub duplicate_observation_choice_keys: u64,
    pub serialized_state_bytes: u64,
    pub tree_nodes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PuctCloneConfig {
    pub search: PuctConfig,
    pub max_decisions: usize,
    pub max_player_turns: usize,
}

impl PuctCloneConfig {
    pub fn validate(&self) -> Result<(), PuctError> {
        self.search.validate()?;
        if self.max_decisions == 0 {
            return Err(PuctError::InvalidConfig(
                "max_decisions must be positive".to_owned(),
            ));
        }
        if self.max_player_turns == 0 {
            return Err(PuctError::InvalidConfig(
                "max_player_turns must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PuctCloneStep {
    pub observation: FairCombatObservation,
    pub choices: Vec<PlayerChoice>,
    pub selected_index: usize,
    pub visits: Vec<u64>,
    pub value: f64,
    pub transitions: usize,
    pub completed_simulations: usize,
    pub leaf_evaluations: usize,
    pub stop_reason: PuctStopReason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PuctCloneOutcome {
    pub status: &'static str,
    pub terminal_hp: i32,
    pub terminal_max_hp: i32,
    pub hp_change: i32,
    pub max_hp_change: i32,
    pub gold_change: i32,
    pub potion_slots: Vec<Option<&'static str>>,
    pub terminal: bool,
    pub truncated: bool,
    pub accepted_decisions: usize,
    pub player_turns: usize,
    pub truncation_trigger: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PuctCloneEpisode {
    pub steps: Vec<PuctCloneStep>,
    pub outcome: PuctCloneOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PuctError {
    NotInCombat,
    TerminalRoot,
    EmptyChoices,
    InvalidConfig(String),
    ChoiceProjection(String),
    Observation(String),
    Transition(String),
    Evaluator(String),
    MalformedEvaluation(String),
}

impl std::fmt::Display for PuctError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInCombat => write!(f, "PUCT requires an active combat root"),
            Self::TerminalRoot => write!(f, "PUCT requires an ongoing combat decision"),
            Self::EmptyChoices => write!(f, "PUCT reached an empty public decision"),
            Self::InvalidConfig(reason)
            | Self::ChoiceProjection(reason)
            | Self::Observation(reason)
            | Self::Transition(reason)
            | Self::Evaluator(reason)
            | Self::MalformedEvaluation(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for PuctError {}

struct Edge {
    choice: PlayerChoice,
    action: RunDecisionAction,
    prior: f64,
    visit_count: u64,
    value_sum: f64,
    child: Option<usize>,
}

struct Node {
    state: RunState,
    edges: Vec<Edge>,
    visit_count: u64,
    terminal_value: Option<f64>,
}

struct ExactStateLeaf {
    choices: Vec<PlayerChoice>,
    actions: Vec<RunDecisionAction>,
    priors: Vec<f64>,
    value: f64,
}

struct ExpandScratch<'a, E> {
    evaluator: &'a mut E,
    leaf_evaluations: &'a mut usize,
    cache: &'a mut HashMap<Vec<u8>, ExactStateLeaf>,
    cache_enabled: bool,
    profile: &'a mut PuctSearchProfile,
    collect_profile: bool,
    seen_states: &'a mut HashSet<Vec<u8>>,
    seen_observations: &'a mut HashSet<Vec<u8>>,
}

/// Canonical complete `RunState` bytes, including hidden RNG and queues.
///
/// This is the only permitted cache identity for privileged PUCT. Fair
/// observation equality is not a substitute.
pub fn complete_run_state_identity(state: &RunState) -> Result<Vec<u8>, PuctError> {
    serde_json::to_vec(state).map_err(|error| {
        PuctError::InvalidConfig(format!(
            "complete state identity serialization failed: {error}"
        ))
    })
}

pub fn puct_search<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctConfig,
    evaluator: &mut E,
) -> Result<PuctSearchResult, PuctError> {
    search_impl(root, config, evaluator, PuctLeafCache::Off, false).map(|(result, _profile)| result)
}

pub fn puct_search_with_leaf_cache<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctConfig,
    evaluator: &mut E,
    leaf_cache: PuctLeafCache,
) -> Result<PuctSearchResult, PuctError> {
    search_impl(root, config, evaluator, leaf_cache, false).map(|(result, _profile)| result)
}

pub fn puct_search_profiled<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctConfig,
    evaluator: &mut E,
    leaf_cache: PuctLeafCache,
) -> Result<(PuctSearchResult, PuctSearchProfile), PuctError> {
    search_impl(root, config, evaluator, leaf_cache, true)
}

fn search_impl<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctConfig,
    evaluator: &mut E,
    leaf_cache: PuctLeafCache,
    collect_profile: bool,
) -> Result<(PuctSearchResult, PuctSearchProfile), PuctError> {
    config.validate()?;
    if root.phase != RunPhase::Combat || root.combat.is_none() {
        return Err(PuctError::NotInCombat);
    }
    if classify_combat_state(root).is_some() {
        return Err(PuctError::TerminalRoot);
    }
    root.validate()
        .map_err(|error| PuctError::InvalidConfig(error.to_string()))?;

    let started = Instant::now();
    let mut profile = PuctSearchProfile::default();
    let cache_enabled = matches!(leaf_cache, PuctLeafCache::ExactState);
    let mut cache = HashMap::<Vec<u8>, ExactStateLeaf>::new();
    let mut seen_states = HashSet::<Vec<u8>>::new();
    let mut seen_observations = HashSet::<Vec<u8>>::new();
    let mut leaf_evaluations = 0usize;
    let mut scratch = ExpandScratch {
        evaluator,
        leaf_evaluations: &mut leaf_evaluations,
        cache: &mut cache,
        cache_enabled,
        profile: &mut profile,
        collect_profile,
        seen_states: &mut seen_states,
        seen_observations: &mut seen_observations,
    };

    let root_state = if collect_profile {
        let clone_started = Instant::now();
        let cloned = root.clone();
        scratch.profile.clone_ns += elapsed_ns(clone_started);
        scratch.profile.clone_count += 1;
        cloned
    } else {
        root.clone()
    };
    let mut root_node = expand_ongoing(root_state, &mut scratch)?;
    let root_priors = root_node
        .edges
        .iter()
        .map(|edge| edge.prior)
        .collect::<Vec<_>>();
    let root_choices = root_node
        .edges
        .iter()
        .map(|edge| edge.choice)
        .collect::<Vec<_>>();
    root_node.terminal_value = None;

    let mut nodes = Vec::new();
    nodes.push(root_node);

    let mut transitions = 0usize;
    let mut completed_simulations = 0usize;

    while completed_simulations < config.simulation_budget && transitions < config.transition_budget
    {
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut node_idx = 0usize;
        loop {
            if let Some(value) = nodes[node_idx].terminal_value {
                // Standard terminal revisit: backup the stored value again.
                if collect_profile {
                    let backup_started = Instant::now();
                    backup(&mut nodes, &path, value);
                    scratch.profile.backup_ns += elapsed_ns(backup_started);
                } else {
                    backup(&mut nodes, &path, value);
                }
                completed_simulations += 1;
                break;
            }
            if nodes[node_idx].edges.is_empty() {
                return Err(PuctError::EmptyChoices);
            }
            let edge_idx = if collect_profile {
                let select_started = Instant::now();
                let index = select_puct_index(&nodes[node_idx], config.c_puct)?;
                scratch.profile.select_ns += elapsed_ns(select_started);
                index
            } else {
                select_puct_index(&nodes[node_idx], config.c_puct)?
            };
            if let Some(child_idx) = nodes[node_idx].edges[edge_idx].child {
                path.push((node_idx, edge_idx));
                node_idx = child_idx;
                continue;
            }
            let action = nodes[node_idx].edges[edge_idx].action;
            let child_state = if collect_profile {
                let apply_started = Instant::now();
                let next = apply_run_decision_action(&nodes[node_idx].state, action)
                    .map_err(|error| PuctError::Transition(error.to_string()))?;
                scratch.profile.apply_ns += elapsed_ns(apply_started);
                scratch.profile.apply_count += 1;
                next
            } else {
                apply_run_decision_action(&nodes[node_idx].state, action)
                    .map_err(|error| PuctError::Transition(error.to_string()))?
            };
            transitions += 1;
            path.push((node_idx, edge_idx));
            let child_idx = nodes.len();
            if let Some(status) =
                classify_combat_episode_transition(&nodes[node_idx].state, &action, &child_state)
            {
                let value = proxy_value(&child_state, status, config)?;
                nodes.push(Node {
                    state: child_state,
                    edges: Vec::new(),
                    visit_count: 0,
                    terminal_value: Some(value),
                });
                nodes[node_idx].edges[edge_idx].child = Some(child_idx);
                if collect_profile {
                    let backup_started = Instant::now();
                    backup(&mut nodes, &path, value);
                    scratch.profile.backup_ns += elapsed_ns(backup_started);
                } else {
                    backup(&mut nodes, &path, value);
                }
                completed_simulations += 1;
                break;
            }
            let mut child = expand_ongoing(child_state, &mut scratch)?;
            let value = child
                .terminal_value
                .expect("ongoing expand stores evaluator value");
            child.terminal_value = None;
            nodes.push(child);
            nodes[node_idx].edges[edge_idx].child = Some(child_idx);
            if collect_profile {
                let backup_started = Instant::now();
                backup(&mut nodes, &path, value);
                scratch.profile.backup_ns += elapsed_ns(backup_started);
            } else {
                backup(&mut nodes, &path, value);
            }
            completed_simulations += 1;
            break;
        }
    }

    let stop_reason = if completed_simulations >= config.simulation_budget {
        PuctStopReason::SimulationBudget
    } else {
        PuctStopReason::TransitionBudget
    };
    let root_node = &nodes[0];
    let visits = root_node
        .edges
        .iter()
        .map(|edge| edge.visit_count)
        .collect::<Vec<_>>();
    let selected_index = argmax_visits(&visits);
    let selected_edge = &root_node.edges[selected_index];
    let selected_choice = selected_edge.choice;
    let selected_action = selected_edge.action;
    let value = root_backed_up_mean(root_node)?;
    let tree_nodes = nodes.len() as u64;
    scratch.profile.tree_nodes = tree_nodes;
    scratch.profile.unique_exact_states = scratch.seen_states.len() as u64;
    scratch.profile.unique_observation_choice_keys = scratch.seen_observations.len() as u64;
    if collect_profile {
        scratch.profile.total_ns = elapsed_ns(started);
    }
    let ExpandScratch {
        evaluator: _,
        leaf_evaluations: _,
        cache: _,
        cache_enabled: _,
        profile: _,
        collect_profile: _,
        seen_states: _,
        seen_observations: _,
    } = scratch;
    Ok((
        PuctSearchResult {
            selected_index,
            selected_choice,
            selected_action,
            visits,
            priors: root_priors,
            value,
            transitions,
            completed_simulations,
            leaf_evaluations,
            stop_reason,
            choices: root_choices,
        },
        profile,
    ))
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn expand_ongoing<E: FairLeafEvaluator>(
    state: RunState,
    scratch: &mut ExpandScratch<'_, E>,
) -> Result<Node, PuctError> {
    let identity = if scratch.cache_enabled || scratch.collect_profile {
        let identity_started = Instant::now();
        let bytes = complete_run_state_identity(&state)?;
        if scratch.collect_profile {
            scratch.profile.identity_ns += elapsed_ns(identity_started);
            scratch.profile.identity_count += 1;
            scratch.profile.serialized_state_bytes += bytes.len() as u64;
            if !scratch.seen_states.insert(bytes.clone()) {
                scratch.profile.duplicate_exact_states += 1;
            }
        }
        Some(bytes)
    } else {
        None
    };
    if scratch.cache_enabled {
        let key = identity.as_ref().ok_or_else(|| {
            PuctError::InvalidConfig(
                "exact-state cache requires complete state identity".to_owned(),
            )
        })?;
        if let Some(cached) = scratch.cache.get(key) {
            if scratch.collect_profile {
                scratch.profile.exact_state_cache_hits += 1;
            }
            return Ok(node_from_cached_leaf(state, cached));
        }
        if scratch.collect_profile {
            scratch.profile.exact_state_cache_misses += 1;
        }
    }

    let (choices, actions) = if scratch.collect_profile {
        let choice_started = Instant::now();
        let projected = public_choice_actions(&state)?;
        scratch.profile.choice_projection_ns += elapsed_ns(choice_started);
        scratch.profile.choice_projection_count += 1;
        projected
    } else {
        public_choice_actions(&state)?
    };
    let observation = if scratch.collect_profile {
        let observation_started = Instant::now();
        let observed = fair_combat_observation(&state)
            .map_err(|error| PuctError::Observation(error.to_string()))?;
        scratch.profile.observation_ns += elapsed_ns(observation_started);
        scratch.profile.observation_count += 1;
        observed
    } else {
        fair_combat_observation(&state)
            .map_err(|error| PuctError::Observation(error.to_string()))?
    };
    if scratch.collect_profile {
        let observation_key = serde_json::to_vec(&(&observation, &choices)).map_err(|error| {
            PuctError::Observation(format!(
                "observation identity serialization failed: {error}"
            ))
        })?;
        if !scratch.seen_observations.insert(observation_key) {
            scratch.profile.duplicate_observation_choice_keys += 1;
        }
    }
    let evaluation = if scratch.collect_profile {
        let evaluate_started = Instant::now();
        let evaluated = scratch
            .evaluator
            .evaluate(&observation, &choices)
            .map_err(PuctError::Evaluator)?;
        scratch.profile.evaluate_ns += elapsed_ns(evaluate_started);
        scratch.profile.evaluate_count += 1;
        evaluated
    } else {
        scratch
            .evaluator
            .evaluate(&observation, &choices)
            .map_err(PuctError::Evaluator)?
    };
    *scratch.leaf_evaluations += 1;
    let (priors, value) = validate_evaluation(choices.len(), &evaluation)?;
    if let Some(key) = identity {
        if scratch.cache_enabled {
            scratch.cache.insert(
                key,
                ExactStateLeaf {
                    choices: choices.clone(),
                    actions: actions.clone(),
                    priors: priors.clone(),
                    value,
                },
            );
        }
    }
    Ok(node_from_parts(state, choices, actions, priors, value))
}

fn node_from_cached_leaf(state: RunState, cached: &ExactStateLeaf) -> Node {
    node_from_parts(
        state,
        cached.choices.clone(),
        cached.actions.clone(),
        cached.priors.clone(),
        cached.value,
    )
}

fn node_from_parts(
    state: RunState,
    choices: Vec<PlayerChoice>,
    actions: Vec<RunDecisionAction>,
    priors: Vec<f64>,
    value: f64,
) -> Node {
    let edges = choices
        .into_iter()
        .zip(actions)
        .zip(priors)
        .map(|((choice, action), prior)| Edge {
            choice,
            action,
            prior,
            visit_count: 0,
            value_sum: 0.0,
            child: None,
        })
        .collect();
    Node {
        state,
        edges,
        visit_count: 0,
        terminal_value: Some(value),
    }
}

fn public_choice_actions(
    state: &RunState,
) -> Result<(Vec<PlayerChoice>, Vec<RunDecisionAction>), PuctError> {
    let set = player_choices(state, SEARCH_REVISION)
        .map_err(|error| PuctError::ChoiceProjection(error.to_string()))?;
    if set.choices.is_empty() {
        return Err(PuctError::EmptyChoices);
    }
    let mut actions = Vec::with_capacity(set.choices.len());
    for choice in &set.choices {
        let action = resolve_player_choice(
            state,
            SEARCH_REVISION,
            PlayerChoiceRequest {
                decision_revision: SEARCH_REVISION,
                choice: *choice,
            },
        )
        .map_err(|error| PuctError::ChoiceProjection(error.to_string()))?;
        if actions.contains(&action) {
            return Err(PuctError::ChoiceProjection(
                "public choice resolution maps multiple rows to one authoritative action"
                    .to_owned(),
            ));
        }
        actions.push(action);
    }
    Ok((set.choices, actions))
}

fn validate_evaluation(
    choice_count: usize,
    evaluation: &FairLeafEvaluation,
) -> Result<(Vec<f64>, f64), PuctError> {
    if evaluation.priors.len() != choice_count {
        return Err(PuctError::MalformedEvaluation(format!(
            "evaluator priors length {} does not match {} public choices",
            evaluation.priors.len(),
            choice_count
        )));
    }
    if evaluation
        .priors
        .iter()
        .any(|prior| !prior.is_finite() || *prior < 0.0)
    {
        return Err(PuctError::MalformedEvaluation(
            "evaluator priors must be finite and nonnegative".to_owned(),
        ));
    }
    let mass: f64 = evaluation.priors.iter().sum();
    if !mass.is_finite() || mass <= 0.0 {
        return Err(PuctError::MalformedEvaluation(
            "evaluator priors must have positive finite mass".to_owned(),
        ));
    }
    if !evaluation.value.is_finite() || !(-1.0..=1.0).contains(&evaluation.value) {
        return Err(PuctError::MalformedEvaluation(
            "evaluator value must be finite and in [-1, 1]".to_owned(),
        ));
    }
    let priors = evaluation
        .priors
        .iter()
        .map(|prior| *prior / mass)
        .collect();
    Ok((priors, evaluation.value))
}

fn select_puct_index(node: &Node, c_puct: f64) -> Result<usize, PuctError> {
    let parent_visit_term = (node.visit_count as f64 + 1.0).sqrt();
    let mut best_index = 0usize;
    let mut best_score = f64::NEG_INFINITY;
    for (index, edge) in node.edges.iter().enumerate() {
        let q = if edge.visit_count == 0 {
            0.0
        } else {
            edge.value_sum / edge.visit_count as f64
        };
        if !q.is_finite() {
            return Err(PuctError::MalformedEvaluation(
                "edge Q is not finite".to_owned(),
            ));
        }
        let score = q + c_puct * edge.prior * parent_visit_term / (1.0 + edge.visit_count as f64);
        if !score.is_finite() {
            return Err(PuctError::MalformedEvaluation(
                "PUCT score is not finite".to_owned(),
            ));
        }
        if score > best_score {
            best_score = score;
            best_index = index;
        }
    }
    Ok(best_index)
}

fn argmax_visits(visits: &[u64]) -> usize {
    let mut best_index = 0usize;
    let mut best_visits = 0u64;
    for (index, visits) in visits.iter().copied().enumerate() {
        if visits > best_visits {
            best_visits = visits;
            best_index = index;
        }
    }
    best_index
}

fn root_backed_up_mean(root: &Node) -> Result<f64, PuctError> {
    if root.visit_count == 0 {
        return Err(PuctError::InvalidConfig(
            "PUCT completed no simulations".to_owned(),
        ));
    }
    let value_sum: f64 = root.edges.iter().map(|edge| edge.value_sum).sum();
    let value = value_sum / root.visit_count as f64;
    if !value.is_finite() {
        return Err(PuctError::MalformedEvaluation(
            "backed-up PUCT value is not finite".to_owned(),
        ));
    }
    Ok(value)
}

fn backup(nodes: &mut [Node], path: &[(usize, usize)], value: f64) {
    for &(node_idx, edge_idx) in path {
        let edge = &mut nodes[node_idx].edges[edge_idx];
        edge.visit_count += 1;
        edge.value_sum += value;
        nodes[node_idx].visit_count += 1;
    }
}

pub fn classify_combat_state(state: &RunState) -> Option<&'static str> {
    let combat = state.combat.as_ref()?;
    if combat.phase == CombatPhase::Lost || combat.player.hp <= 0 {
        Some("lost")
    } else if combat.phase == CombatPhase::Won {
        Some("won")
    } else {
        None
    }
}

pub fn classify_combat_episode_transition(
    before: &RunState,
    action: &RunDecisionAction,
    after: &RunState,
) -> Option<&'static str> {
    // Proceed is run-screen cleanup after an already terminal combat, not a
    // second combat outcome. In particular, lost -> Proceed must not become a win.
    if classify_combat_state(before).is_some() {
        return None;
    }
    if let Some(status) = classify_combat_state(after) {
        return Some(status);
    }
    if after.phase == RunPhase::Combat {
        return None;
    }
    let escaped = matches!(
        action,
        RunDecisionAction::Run(RunAction::UsePotion { slot, .. })
            if before.potion_at_slot(*slot) == Some(Potion::SmokeBomb)
    );
    if escaped {
        return Some("escaped");
    }
    // Victory clears the combat and opens the next run screen, so it is
    // recognized by exclusion. Guard the one case exclusion would get maximally
    // wrong and label a dead player's exit as the best possible outcome.
    if player_hp_and_max(after).0 <= 0 {
        return Some("lost");
    }
    Some("won")
}

pub fn puct_clone_episode<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctCloneConfig,
    evaluator: &mut E,
) -> Result<PuctCloneEpisode, PuctError> {
    puct_clone_episode_with_leaf_cache(root, config, evaluator, PuctLeafCache::ExactState)
}

/// Privileged teacher episode search with an explicit leaf-cache policy.
pub fn puct_clone_episode_with_leaf_cache<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctCloneConfig,
    evaluator: &mut E,
    leaf_cache: PuctLeafCache,
) -> Result<PuctCloneEpisode, PuctError> {
    config.validate()?;
    if root.phase != RunPhase::Combat || root.combat.is_none() {
        return Err(PuctError::NotInCombat);
    }
    root.validate()
        .map_err(|error| PuctError::InvalidConfig(error.to_string()))?;
    let (root_hp, root_max_hp) = player_hp_and_max(root);
    let root_gold = root.gold;
    let mut state = root.clone();
    let mut steps = Vec::new();
    let mut accepted_decisions = 0usize;
    let mut player_turns = 1usize;
    let mut terminal_status = classify_combat_state(root);
    let mut truncation_trigger = None;

    while terminal_status.is_none() {
        let search = puct_search_with_leaf_cache(&state, &config.search, evaluator, leaf_cache)?;
        let visit_sum = search.visits.iter().copied().sum::<u64>();
        if visit_sum != search.completed_simulations as u64 {
            return Err(PuctError::MalformedEvaluation(
                "PUCT visit mass must equal completed simulations".to_owned(),
            ));
        }
        if search.transitions > config.search.transition_budget
            || search.completed_simulations > config.search.simulation_budget
        {
            return Err(PuctError::MalformedEvaluation(
                "PUCT episode overshot its search budgets".to_owned(),
            ));
        }
        let observation = fair_combat_observation(&state)
            .map_err(|error| PuctError::Observation(error.to_string()))?;
        steps.push(PuctCloneStep {
            observation,
            choices: search.choices.clone(),
            selected_index: search.selected_index,
            visits: search.visits.clone(),
            value: search.value,
            transitions: search.transitions,
            completed_simulations: search.completed_simulations,
            leaf_evaluations: search.leaf_evaluations,
            stop_reason: search.stop_reason,
        });
        let action = search.selected_action;
        let next = apply_run_decision_action(&state, action)
            .map_err(|error| PuctError::Transition(error.to_string()))?;
        accepted_decisions += 1;
        player_turns = player_turns.saturating_add(player_turn_advances(&state, &action, &next));
        if let Some(status) = classify_combat_episode_transition(&state, &action, &next) {
            terminal_status = Some(status);
            state = next;
            break;
        }
        if accepted_decisions >= config.max_decisions {
            truncation_trigger = Some("accepted_decisions");
            state = next;
            break;
        }
        if player_turns > config.max_player_turns {
            truncation_trigger = Some("player_turns");
            state = next;
            break;
        }
        state = next;
    }

    let status = terminal_status.unwrap_or("truncated");
    let (terminal_hp, terminal_max_hp) = player_hp_and_max(&state);
    let terminal = terminal_status.is_some();
    Ok(PuctCloneEpisode {
        steps,
        outcome: PuctCloneOutcome {
            status,
            terminal_hp,
            terminal_max_hp,
            hp_change: terminal_hp - root_hp,
            max_hp_change: terminal_max_hp - root_max_hp,
            gold_change: state.gold - root_gold,
            potion_slots: (0..state.potion_capacity())
                .map(|slot| state.potion_at_slot(slot).map(potion_key))
                .collect(),
            terminal,
            truncated: !terminal,
            accepted_decisions,
            player_turns,
            truncation_trigger,
        },
    })
}

#[must_use]
pub fn player_turn_advances(
    before: &RunState,
    action: &RunDecisionAction,
    after: &RunState,
) -> usize {
    let (Some(before_combat), Some(after_combat)) = (before.combat.as_ref(), after.combat.as_ref())
    else {
        return 0;
    };
    if after_combat.phase != CombatPhase::WaitingForPlayer {
        return 0;
    }

    // This counter is authoritative when maintained, and preserves deltas
    // greater than one. It is not the sole signal because some relic sets do
    // not maintain it.
    let counter_delta = after_combat
        .relic_counters
        .player_turns_started
        .saturating_sub(before_combat.relic_counters.player_turns_started)
        as usize;
    if counter_delta > 0 {
        return counter_delta;
    }
    if matches!(action, RunDecisionAction::Combat(CombatAction::EndTurn)) {
        return 1;
    }

    // Conclude's PressEndTurnButtonAction is a modeled forced turn source just
    // like explicit END, but its accepted public action remains PlayCard.
    let forced_turn_card = match action {
        RunDecisionAction::Combat(CombatAction::PlayCard { card_id, .. }) => before_combat
            .piles
            .hand
            .iter()
            .find(|card| card.id == *card_id)
            .is_some_and(|card| card.content_id == CONCLUDE_ANY_COLOR_ID),
        _ => false,
    };
    if forced_turn_card {
        return 1;
    }

    // Time Warp can execute end_player_turn from the twelfth card transition
    // or from a later selection transition. Both are authoritative state
    // evidence that the forced turn completed.
    let time_warp_wrapped = before_combat.monsters.iter().any(|before_monster| {
        before_monster.alive
            && before_monster.content_id == TIME_EATER_ID
            && before_monster.powers.time_warp == 11
            && after_combat.monsters.iter().any(|after_monster| {
                after_monster.content_id == before_monster.content_id
                    && after_monster.powers.time_warp == 0
            })
    });
    let forced_end_settled = (before_combat.time_warp_end_turn
        || before_combat.defer_time_warp_end_turn)
        && !after_combat.time_warp_end_turn
        && !after_combat.defer_time_warp_end_turn;
    usize::from(time_warp_wrapped || forced_end_settled)
}

fn player_hp_and_max(state: &RunState) -> (i32, i32) {
    state
        .combat
        .as_ref()
        .map(|combat| (combat.player.hp, combat.player.max_hp))
        .unwrap_or((state.player_hp, state.player_max_hp))
}

fn remaining_potions(state: &RunState) -> usize {
    (0..state.potion_capacity())
        .filter(|&slot| state.potion_at_slot(slot).is_some())
        .count()
}

fn proxy_value(state: &RunState, status: &str, config: &PuctConfig) -> Result<f64, PuctError> {
    let (terminal_hp, terminal_max_hp) = player_hp_and_max(state);
    config.reward.value(
        status,
        terminal_hp,
        terminal_max_hp,
        terminal_max_hp - config.episode_root_max_hp,
        state.gold - config.episode_root_gold,
        remaining_potions(state),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{
        expand_ongoing, public_choice_actions, validate_evaluation, ExactStateLeaf, ExpandScratch,
        Node,
    };
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};
    use sts_core::{
        combat::{HandSelectPurpose, HandSelectState},
        fair_combat_observation, restore_run_snapshot_json, CombatAction, CombatDecisionState,
        RngTraceStream,
    };

    struct UniformEvaluator {
        value: f64,
        calls: usize,
    }

    impl FairLeafEvaluator for UniformEvaluator {
        fn evaluate(
            &mut self,
            observation: &FairCombatObservation,
            choices: &[PlayerChoice],
        ) -> Result<FairLeafEvaluation, String> {
            self.calls += 1;
            let json = serde_json::to_string(observation).expect("observation serializes");
            for forbidden in [
                "card_id",
                "monster_id",
                "content_id",
                "rng",
                "move_history",
                "queued_decisions",
                "pending_actions",
            ] {
                if json.contains(forbidden) {
                    return Err(format!("hidden field {forbidden} reached evaluator"));
                }
            }
            Ok(FairLeafEvaluation {
                priors: vec![1.0; choices.len()],
                value: self.value,
            })
        }
    }

    fn fnv1a(bytes: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    struct ObservationSensitiveEvaluator {
        calls: usize,
    }

    impl ObservationSensitiveEvaluator {
        fn leaf(
            observation: &FairCombatObservation,
            choices: &[PlayerChoice],
        ) -> FairLeafEvaluation {
            let observation_bytes =
                serde_json::to_vec(observation).expect("observation serializes");
            let observation_hash = fnv1a(&observation_bytes);
            let value = ((observation_hash % 1999) as f64 / 999.5) - 1.0;
            let priors = choices
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    let choice_bytes = serde_json::to_vec(choice).expect("choice serializes");
                    let mixed = fnv1a(&choice_bytes) ^ observation_hash ^ index as u64;
                    ((mixed % 89) + 1) as f64
                })
                .collect();
            FairLeafEvaluation { priors, value }
        }
    }

    impl FairLeafEvaluator for ObservationSensitiveEvaluator {
        fn evaluate(
            &mut self,
            observation: &FairCombatObservation,
            choices: &[PlayerChoice],
        ) -> Result<FairLeafEvaluation, String> {
            self.calls += 1;
            let json = serde_json::to_string(observation).expect("observation serializes");
            for forbidden in [
                "card_id",
                "monster_id",
                "content_id",
                "rng",
                "move_history",
                "queued_decisions",
                "pending_actions",
            ] {
                if json.contains(forbidden) {
                    return Err(format!("hidden field {forbidden} reached evaluator"));
                }
            }
            Ok(Self::leaf(observation, choices))
        }
    }

    struct ImpureCallEvaluator {
        calls: usize,
    }

    impl FairLeafEvaluator for ImpureCallEvaluator {
        fn evaluate(
            &mut self,
            _observation: &FairCombatObservation,
            choices: &[PlayerChoice],
        ) -> Result<FairLeafEvaluation, String> {
            self.calls += 1;
            let value = ((self.calls % 21) as f64 / 10.0) - 1.0;
            Ok(FairLeafEvaluation {
                priors: vec![1.0; choices.len()],
                value,
            })
        }
    }

    struct OneHotEvaluator {
        value: f64,
    }

    impl FairLeafEvaluator for OneHotEvaluator {
        fn evaluate(
            &mut self,
            _observation: &FairCombatObservation,
            choices: &[PlayerChoice],
        ) -> Result<FairLeafEvaluation, String> {
            if choices.is_empty() {
                return Err("one-hot evaluator received no choices".to_owned());
            }
            let mut priors = vec![0.0; choices.len()];
            priors[choices.len() - 1] = 1.0;
            Ok(FairLeafEvaluation {
                priors,
                value: self.value,
            })
        }
    }

    struct BiasedEvaluator {
        value: f64,
    }

    impl FairLeafEvaluator for BiasedEvaluator {
        fn evaluate(
            &mut self,
            _observation: &FairCombatObservation,
            choices: &[PlayerChoice],
        ) -> Result<FairLeafEvaluation, String> {
            if choices.is_empty() {
                return Err("biased evaluator received no choices".to_owned());
            }
            let target = choices
                .iter()
                .position(|choice| *choice == PlayerChoice::EndTurn)
                .unwrap_or(choices.len() - 1);
            let mut priors = vec![0.01; choices.len()];
            priors[target] = 0.99;
            Ok(FairLeafEvaluation {
                priors,
                value: self.value,
            })
        }
    }

    struct BrokenEvaluator {
        priors: Vec<f64>,
        value: f64,
    }

    impl FairLeafEvaluator for BrokenEvaluator {
        fn evaluate(
            &mut self,
            _observation: &FairCombatObservation,
            _choices: &[PlayerChoice],
        ) -> Result<FairLeafEvaluation, String> {
            Ok(FairLeafEvaluation {
                priors: self.priors.clone(),
                value: self.value,
            })
        }
    }

    fn config(root: &RunState, budget: usize) -> PuctConfig {
        config_with(root, 1.5, budget, budget)
    }

    fn config_with(
        root: &RunState,
        c_puct: f64,
        simulation_budget: usize,
        transition_budget: usize,
    ) -> PuctConfig {
        let (_, max_hp) = player_hp_and_max(root);
        PuctConfig {
            c_puct,
            simulation_budget,
            transition_budget,
            reward: CombatProxyConfig::default(),
            episode_root_max_hp: max_hp,
            episode_root_gold: root.gold,
        }
    }

    #[test]
    fn search_is_deterministic_and_reports_accounting() {
        let root = RunState::combat_fixture();
        let search_config = config(&root, 8);
        let mut first_eval = UniformEvaluator {
            value: 0.1,
            calls: 0,
        };
        let first = puct_search(&root, &search_config, &mut first_eval).expect("first search");
        let mut second_eval = UniformEvaluator {
            value: 0.1,
            calls: 0,
        };
        let second = puct_search(&root, &search_config, &mut second_eval).expect("second search");
        assert_eq!(first, second);
        assert_eq!(first.transitions, 8);
        assert!(first.transitions <= search_config.transition_budget);
        assert_eq!(first.completed_simulations, 8);
        assert_eq!(first.leaf_evaluations, first_eval.calls);
        assert!(first.leaf_evaluations >= 1);
        assert_eq!(first.stop_reason, PuctStopReason::SimulationBudget);
        assert_eq!(first.visits.iter().sum::<u64>(), 8);
        assert_eq!(first.choices.len(), first.visits.len());
        assert_eq!(first.selected_choice, first.choices[first.selected_index]);
        assert!((first.value - 0.1).abs() < 1e-12);
    }

    #[test]
    fn dual_budgets_are_positive_and_do_not_overshoot() {
        let root = RunState::combat_fixture();
        for (sim, trans) in [(0usize, 1usize), (1, 0)] {
            let mut evaluator = UniformEvaluator {
                value: 0.0,
                calls: 0,
            };
            let error = puct_search(&root, &config_with(&root, 1.5, sim, trans), &mut evaluator)
                .expect_err("zero budget");
            assert!(matches!(error, PuctError::InvalidConfig(_)), "{error}");
        }
        for budget in [1usize, 3, 7] {
            let mut evaluator = UniformEvaluator {
                value: 0.0,
                calls: 0,
            };
            let result = puct_search(&root, &config(&root, budget), &mut evaluator)
                .expect("budgeted search");
            assert!(result.transitions <= budget, "budget {budget}");
            assert!(result.completed_simulations <= budget, "budget {budget}");
            assert!(result.leaf_evaluations <= budget.saturating_add(1));
        }
    }

    #[test]
    fn zero_puct_constant_is_rejected() {
        let root = RunState::combat_fixture();
        let mut evaluator = UniformEvaluator {
            value: 0.0,
            calls: 0,
        };
        let error = puct_search(&root, &config_with(&root, 0.0, 16, 100), &mut evaluator)
            .expect_err("zero c_puct");
        assert!(matches!(error, PuctError::InvalidConfig(_)), "{error}");
        assert_eq!(evaluator.calls, 0);
    }

    #[test]
    fn sparse_one_hot_priors_cannot_hang() {
        let root = RunState::combat_fixture();
        let mut evaluator = OneHotEvaluator { value: 0.5 };
        let result = puct_search(&root, &config_with(&root, 1.5, 16, 100), &mut evaluator)
            .expect("one-hot search");
        assert_eq!(result.completed_simulations, 16);
        assert!(result.transitions <= 16);
        assert_eq!(result.stop_reason, PuctStopReason::SimulationBudget);
        let last = result.visits.len() - 1;
        assert_eq!(
            result.visits[last],
            *result.visits.iter().max().expect("visits")
        );
    }

    #[test]
    fn search_stops_at_the_earlier_budget_without_overshoot() {
        let root = RunState::combat_fixture();
        let mut trans_eval = UniformEvaluator {
            value: 0.0,
            calls: 0,
        };
        let trans_limited =
            puct_search(&root, &config_with(&root, 1.5, 8, 3), &mut trans_eval).expect("trans cap");
        assert_eq!(trans_limited.transitions, 3);
        assert_eq!(trans_limited.completed_simulations, 3);
        assert_eq!(trans_limited.stop_reason, PuctStopReason::TransitionBudget);
        let mut sim_eval = UniformEvaluator {
            value: 0.0,
            calls: 0,
        };
        let sim_limited =
            puct_search(&root, &config_with(&root, 1.5, 3, 8), &mut sim_eval).expect("sim cap");
        assert_eq!(sim_limited.completed_simulations, 3);
        assert!(sim_limited.transitions <= 3);
        assert_eq!(sim_limited.stop_reason, PuctStopReason::SimulationBudget);
    }

    #[test]
    fn priors_guide_the_first_descent() {
        let root = RunState::combat_fixture();
        let target = player_choices(&root, SEARCH_REVISION)
            .expect("choices")
            .choices
            .iter()
            .position(|choice| *choice == PlayerChoice::EndTurn)
            .expect("fixture has EndTurn");
        let mut evaluator = BiasedEvaluator { value: 0.0 };
        let first = puct_search(&root, &config(&root, 1), &mut evaluator).expect("first descent");
        assert_eq!(first.selected_index, target);
        assert_eq!(first.visits[target], 1);
        assert_eq!(first.visits.iter().sum::<u64>(), 1);
        let mut evaluator = BiasedEvaluator { value: 0.0 };
        let result = puct_search(&root, &config(&root, 24), &mut evaluator).expect("biased search");
        assert_eq!(result.selected_index, target);
        assert_eq!(result.selected_choice, PlayerChoice::EndTurn);
        assert_eq!(
            result.visits[target],
            *result.visits.iter().max().expect("visits")
        );
        assert!(result.visits[target] > result.visits[0]);
    }

    #[test]
    fn backup_keeps_the_same_player_sign() {
        let root = RunState::combat_fixture();
        let mut evaluator = UniformEvaluator {
            value: 0.25,
            calls: 0,
        };
        let result = puct_search(&root, &config(&root, 1), &mut evaluator).expect("one expansion");
        assert_eq!(result.transitions, 1);
        assert_eq!(result.completed_simulations, 1);
        assert!((result.value - 0.25).abs() < 1e-12);
        assert!(result.value > 0.0);
    }

    #[test]
    fn malformed_evaluator_output_fails_closed() {
        let root = RunState::combat_fixture();
        let choice_count = player_choices(&root, SEARCH_REVISION)
            .expect("choices")
            .choices
            .len();
        let cases = [
            BrokenEvaluator {
                priors: vec![1.0; choice_count.saturating_sub(1)],
                value: 0.0,
            },
            BrokenEvaluator {
                priors: vec![1.0; choice_count],
                value: 2.0,
            },
            BrokenEvaluator {
                priors: vec![-1.0; choice_count],
                value: 0.0,
            },
            BrokenEvaluator {
                priors: vec![f64::NAN; choice_count],
                value: 0.0,
            },
        ];
        for mut evaluator in cases {
            let error =
                puct_search(&root, &config(&root, 1), &mut evaluator).expect_err("malformed");
            assert!(
                matches!(error, PuctError::MalformedEvaluation(_)),
                "{error}"
            );
        }
    }

    #[test]
    fn evaluator_payload_is_fair_observation_and_public_choices() {
        let root = RunState::combat_fixture();
        let observation = fair_combat_observation(&root).expect("fair observation");
        let json = serde_json::to_string(&observation).expect("serializes");
        assert!(!json.contains("card_id"));
        assert!(!json.contains("monster_id"));
        let mut evaluator = UniformEvaluator {
            value: 0.0,
            calls: 0,
        };
        puct_search(&root, &config(&root, 2), &mut evaluator).expect("search");
        assert!(evaluator.calls >= 1);
    }

    #[test]
    fn combat_proxy_matches_python_combat_proxy_v1() {
        let config = CombatProxyConfig::default();
        config.validate().expect("default config");
        let lost = config.value("lost", 0, 80, 0, 0, 0).expect("lost");
        let escaped = config.value("escaped", 40, 80, 0, 0, 0).expect("escaped");
        let won = config.value("won", 80, 80, 0, 0, 0).expect("won");
        let won_resources = config
            .value("won", 40, 80, 10, 100, 2)
            .expect("won resources");
        assert_eq!(lost, -1.0);
        assert!((escaped - 0.35).abs() < 1e-12);
        assert!((won - 0.95).abs() < 1e-12);
        assert!((won_resources - 0.89).abs() < 1e-12);
        assert!(escaped > lost);
        assert!(won > escaped);
        let current_root = config.value("won", 40, 80, 0, 0, 0).expect("current root");
        let episode_root = config
            .value("won", 40, 80, 80 - 70, 99 - 50, 0)
            .expect("episode baseline delta");
        assert!((current_root - 0.85).abs() < 1e-12);
        assert!(episode_root > current_root);
        let overlapping = CombatProxyConfig {
            win_base: 0.3,
            escape_base: 0.25,
            resource_clip: 0.2,
            ..CombatProxyConfig::default()
        };
        assert!(overlapping.validate().is_err());
        let extra = serde_json::from_str::<CombatProxyConfig>(
            r#"{"name":"combat_proxy_v1","version":1,"win_base":0.75,"escape_base":0.25,"loss_value":-1.0,"hp_fraction_weight":0.2,"max_hp_change_per_ten_weight":0.01,"gold_change_per_hundred_weight":0.01,"potion_weight":0.01,"resource_clip":0.2,"extra":1}"#,
        );
        assert!(extra.is_err());
    }

    #[test]
    fn terminal_cleanup_does_not_create_a_second_combat_outcome() {
        let mut lost = RunState::combat_fixture();
        let combat = lost.combat.as_mut().expect("combat fixture");
        combat.phase = CombatPhase::Lost;
        combat.player.hp = 0;
        lost.player_hp = 0;
        let after = lost.clone();

        assert_eq!(classify_combat_state(&lost), Some("lost"));
        assert_eq!(
            classify_combat_episode_transition(
                &lost,
                &RunDecisionAction::Run(RunAction::Proceed),
                &after,
            ),
            None
        );
    }

    #[test]
    fn a_dead_player_leaving_combat_is_not_classified_as_a_win() {
        let before = RunState::combat_fixture();
        let mut after = before.clone();
        after.combat = None;
        after.phase = RunPhase::Reward;
        after.player_hp = 0;

        assert_eq!(
            classify_combat_episode_transition(
                &before,
                &RunDecisionAction::Combat(CombatAction::EndTurn),
                &after,
            ),
            Some("lost")
        );
    }

    #[test]
    fn smoke_bomb_exit_is_classified_as_escaped() {
        let mut before = RunState::combat_fixture();
        before.potions = vec![Potion::SmokeBomb];
        let mut after = before.clone();
        after.combat = None;
        after.phase = RunPhase::Idle;
        after.potions.clear();

        assert_eq!(
            classify_combat_episode_transition(
                &before,
                &RunDecisionAction::Run(RunAction::UsePotion {
                    slot: 0,
                    target: None,
                }),
                &after,
            ),
            Some("escaped")
        );
    }

    fn clone_config(root: &RunState, max_decisions: usize) -> PuctCloneConfig {
        PuctCloneConfig {
            search: config(root, 4),
            max_decisions,
            max_player_turns: 100,
        }
    }

    #[test]
    fn clone_episode_is_deterministic_and_does_not_mutate_the_root() {
        let root = RunState::combat_fixture();
        let before = serde_json::to_vec(&root).expect("serialize root");
        let mut first_eval = UniformEvaluator {
            value: 0.1,
            calls: 0,
        };
        let first =
            puct_clone_episode(&root, &clone_config(&root, 1), &mut first_eval).expect("first");
        let after = serde_json::to_vec(&root).expect("serialize root after");
        assert_eq!(before, after);
        let mut second_eval = UniformEvaluator {
            value: 0.1,
            calls: 0,
        };
        let second =
            puct_clone_episode(&root, &clone_config(&root, 1), &mut second_eval).expect("second");
        assert_eq!(first, second);
        assert_eq!(first.steps.len(), 1);
        assert_eq!(first.outcome.status, "truncated");
        assert_eq!(first.outcome.truncation_trigger, Some("accepted_decisions"));
        assert_eq!(first.outcome.accepted_decisions, 1);
        let step = &first.steps[0];
        assert_eq!(
            step.visits.iter().sum::<u64>(),
            step.completed_simulations as u64
        );
        assert!(step.transitions <= 4);
        assert!(step.completed_simulations <= 4);
        let selected = step.selected_index;
        assert_eq!(
            step.visits[selected],
            *step.visits.iter().max().expect("visits")
        );
        assert!(step
            .visits
            .iter()
            .take(selected)
            .all(|visits| *visits < step.visits[selected]));
        assert!((step.value - 0.1).abs() < 1e-12);
    }

    #[test]
    fn clone_episode_classifies_an_initial_terminal_without_search() {
        let mut lost = RunState::combat_fixture();
        let combat = lost.combat.as_mut().expect("combat fixture");
        combat.phase = CombatPhase::Lost;
        combat.player.hp = 0;
        lost.player_hp = 0;
        let mut evaluator = UniformEvaluator {
            value: 0.0,
            calls: 0,
        };
        let episode =
            puct_clone_episode(&lost, &clone_config(&lost, 8), &mut evaluator).expect("lost root");
        assert!(episode.steps.is_empty());
        assert_eq!(episode.outcome.status, "lost");
        assert_eq!(episode.outcome.accepted_decisions, 0);
        assert_eq!(evaluator.calls, 0);
    }

    #[test]
    fn clone_episode_evaluator_payload_stays_fair() {
        let root = RunState::combat_fixture();
        let mut evaluator = UniformEvaluator {
            value: 0.0,
            calls: 0,
        };
        puct_clone_episode(&root, &clone_config(&root, 1), &mut evaluator).expect("search");
        assert!(evaluator.calls >= 1);
    }

    fn assert_same_search_decision(left: &PuctSearchResult, right: &PuctSearchResult) {
        assert_eq!(left.selected_index, right.selected_index);
        assert_eq!(left.selected_choice, right.selected_choice);
        assert_eq!(left.selected_action, right.selected_action);
        assert_eq!(left.visits, right.visits);
        assert_eq!(left.priors, right.priors);
        assert_eq!(left.value, right.value);
        assert_eq!(left.transitions, right.transitions);
        assert_eq!(left.completed_simulations, right.completed_simulations);
        assert_eq!(left.stop_reason, right.stop_reason);
        assert_eq!(left.choices, right.choices);
    }

    fn assert_root_bytes_unchanged(root: &RunState, before: &[u8]) {
        assert_eq!(
            serde_json::to_vec(root).expect("root bytes after search"),
            before
        );
    }

    #[test]
    fn observation_sensitive_evaluator_varies_with_observation_not_choice_count() {
        let root = RunState::combat_fixture();
        let mut shifted = root.clone();
        shifted.combat.as_mut().expect("combat").player.hp =
            shifted.combat.as_ref().expect("combat").player.hp - 1;
        let observation = fair_combat_observation(&root).expect("root observation");
        let shifted_observation = fair_combat_observation(&shifted).expect("shifted observation");
        assert_ne!(observation, shifted_observation);
        let choices = player_choices(&root, SEARCH_REVISION)
            .expect("root choices")
            .choices;
        let shifted_choices = player_choices(&shifted, SEARCH_REVISION)
            .expect("shifted choices")
            .choices;
        assert_eq!(choices.len(), shifted_choices.len());
        let first = ObservationSensitiveEvaluator::leaf(&observation, &choices);
        let repeat = ObservationSensitiveEvaluator::leaf(&observation, &choices);
        let shifted_leaf =
            ObservationSensitiveEvaluator::leaf(&shifted_observation, &shifted_choices);
        assert_eq!(first, repeat);
        assert_ne!(first.value, shifted_leaf.value);
        assert_ne!(first.priors, shifted_leaf.priors);
        let uniform = FairLeafEvaluation {
            priors: vec![1.0; choices.len()],
            value: 0.1,
        };
        assert_ne!(first.priors, uniform.priors);
        let mut evaluator = ObservationSensitiveEvaluator { calls: 0 };
        let once = evaluator
            .evaluate(&observation, &choices)
            .expect("first call");
        let twice = evaluator
            .evaluate(&observation, &choices)
            .expect("second call");
        assert_eq!(evaluator.calls, 2);
        assert_eq!(once, twice);
        assert_eq!(once, first);
    }

    #[test]
    fn complete_state_identity_guards_hidden_rng_gameplay_and_skipped_diagnostics() {
        let original = RunState::combat_fixture();
        let original_identity = complete_run_state_identity(&original).expect("original identity");
        assert_eq!(
            original_identity,
            serde_json::to_vec(&original).expect("raw serialized bytes")
        );

        let mut hidden_twin = original.clone();
        hidden_twin.misc_rng_counter = hidden_twin.misc_rng_counter.saturating_add(1);
        let original_observation =
            fair_combat_observation(&original).expect("original observation");
        let twin_observation = fair_combat_observation(&hidden_twin).expect("twin observation");
        assert_eq!(original_observation, twin_observation);
        assert_ne!(
            original_identity,
            complete_run_state_identity(&hidden_twin).expect("twin identity")
        );

        let mut shuffle_twin = original.clone();
        let next_shuffle_counter = shuffle_twin
            .combat
            .as_ref()
            .expect("combat")
            .rng
            .shuffle_rng
            .counter()
            .saturating_add(1);
        shuffle_twin
            .combat
            .as_mut()
            .expect("combat")
            .rng
            .shuffle_rng
            .set_counter(next_shuffle_counter);

        let mut hp = original.clone();
        hp.combat.as_mut().expect("combat").player.hp -= 1;
        let mut gold = original.clone();
        gold.gold += 1;
        let mut energy = original.clone();
        energy.combat.as_mut().expect("combat").player.energy -= 1;
        let mut monster_hp = original.clone();
        monster_hp.combat.as_mut().expect("combat").monsters[0].hp -= 1;
        let mut queued = original.clone();
        {
            let combat = queued.combat.as_mut().expect("combat");
            let source_card_id = combat.piles.hand[0].id;
            combat
                .queued_decisions
                .push_back(CombatDecisionState::HandSelect {
                    state: HandSelectState {
                        purpose: HandSelectPurpose::ForethoughtPutAnyOnDraw,
                        source_card_id,
                        selected_hand_index: None,
                        selected_hand_indices: Vec::new(),
                        dual_wield_restore_on_confirm: Vec::new(),
                        dual_wield_force_exhaust: false,
                    },
                    pending_actions: Default::default(),
                });
        }

        for (label, mutated) in [
            ("shuffle_rng", shuffle_twin),
            ("player_hp", hp),
            ("gold", gold),
            ("energy", energy),
            ("monster_hp", monster_hp),
            ("queued_decisions", queued),
        ] {
            assert_ne!(
                original_identity,
                complete_run_state_identity(&mutated).expect(label),
                "{label} must change complete-state identity"
            );
        }

        let mut diagnostic = original.clone();
        {
            let combat = diagnostic.combat.as_mut().expect("combat");
            combat.rng.shuffle_rng = combat
                .rng
                .shuffle_rng
                .clone()
                .for_stream(RngTraceStream::Monster);
        }
        assert_eq!(
            original_identity,
            complete_run_state_identity(&diagnostic).expect("diagnostic identity")
        );
        assert_eq!(original, diagnostic);

        let before_projection = complete_run_state_identity(&original).expect("before projection");
        let first_choices = player_choices(&original, SEARCH_REVISION).expect("choices");
        let second_choices = player_choices(&original, SEARCH_REVISION).expect("choices again");
        assert_eq!(first_choices, second_choices);
        assert_eq!(
            before_projection,
            complete_run_state_identity(&original).expect("after projection")
        );
    }

    fn node_leaf_parts_eq(left: &Node, right: &Node) {
        assert_eq!(left.edges.len(), right.edges.len());
        for (left_edge, right_edge) in left.edges.iter().zip(&right.edges) {
            assert_eq!(left_edge.choice, right_edge.choice);
            assert_eq!(left_edge.action, right_edge.action);
            assert_eq!(left_edge.prior, right_edge.prior);
        }
        assert_eq!(left.terminal_value, right.terminal_value);
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_once<E: FairLeafEvaluator>(
        state: RunState,
        evaluator: &mut E,
        cache: &mut HashMap<Vec<u8>, ExactStateLeaf>,
        cache_enabled: bool,
        profile: &mut PuctSearchProfile,
        leaf_evaluations: &mut usize,
        seen_states: &mut HashSet<Vec<u8>>,
        seen_observations: &mut HashSet<Vec<u8>>,
    ) -> Node {
        let mut scratch = ExpandScratch {
            evaluator,
            leaf_evaluations,
            cache,
            cache_enabled,
            profile,
            collect_profile: true,
            seen_states,
            seen_observations,
        };
        expand_ongoing(state, &mut scratch).expect("expand")
    }

    #[test]
    fn exact_state_hit_reuses_projection_and_observation_sensitive_leaf() {
        let state = RunState::combat_fixture();
        let (fresh_choices, fresh_actions) =
            public_choice_actions(&state).expect("fresh projection");
        let mut evaluator = ObservationSensitiveEvaluator { calls: 0 };
        let mut cache = HashMap::new();
        let mut profile = PuctSearchProfile::default();
        let mut leaf_evaluations = 0usize;
        let mut seen_states = HashSet::new();
        let mut seen_observations = HashSet::new();
        let first = expand_once(
            state.clone(),
            &mut evaluator,
            &mut cache,
            true,
            &mut profile,
            &mut leaf_evaluations,
            &mut seen_states,
            &mut seen_observations,
        );
        assert_eq!(evaluator.calls, 1);
        assert_eq!(profile.exact_state_cache_misses, 1);
        assert_eq!(profile.choice_projection_count, 1);
        let second = expand_once(
            state.clone(),
            &mut evaluator,
            &mut cache,
            true,
            &mut profile,
            &mut leaf_evaluations,
            &mut seen_states,
            &mut seen_observations,
        );
        assert_eq!(evaluator.calls, 1);
        assert_eq!(leaf_evaluations, 1);
        assert_eq!(profile.exact_state_cache_hits, 1);
        assert_eq!(profile.choice_projection_count, 1);
        node_leaf_parts_eq(&first, &second);
        let expected = ObservationSensitiveEvaluator::leaf(
            &fair_combat_observation(&state).expect("observation"),
            &fresh_choices,
        );
        let (expected_priors, expected_value) =
            validate_evaluation(fresh_choices.len(), &expected).expect("normalize");
        assert_eq!(first.terminal_value, Some(expected_value));
        assert_eq!(
            first
                .edges
                .iter()
                .map(|edge| edge.choice)
                .collect::<Vec<_>>(),
            fresh_choices
        );
        assert_eq!(
            first
                .edges
                .iter()
                .map(|edge| edge.action)
                .collect::<Vec<_>>(),
            fresh_actions
        );
        assert_eq!(
            first
                .edges
                .iter()
                .map(|edge| edge.prior)
                .collect::<Vec<_>>(),
            expected_priors
        );
    }

    #[test]
    fn exact_state_hit_is_incorrect_when_the_evaluator_is_impure() {
        let state = RunState::combat_fixture();
        let mut cached_eval = ImpureCallEvaluator { calls: 0 };
        let mut cache = HashMap::new();
        let mut profile = PuctSearchProfile::default();
        let mut leaf_evaluations = 0usize;
        let mut seen_states = HashSet::new();
        let mut seen_observations = HashSet::new();
        let cached_first = expand_once(
            state.clone(),
            &mut cached_eval,
            &mut cache,
            true,
            &mut profile,
            &mut leaf_evaluations,
            &mut seen_states,
            &mut seen_observations,
        );
        let cached_second = expand_once(
            state.clone(),
            &mut cached_eval,
            &mut cache,
            true,
            &mut profile,
            &mut leaf_evaluations,
            &mut seen_states,
            &mut seen_observations,
        );
        assert_eq!(cached_eval.calls, 1);
        node_leaf_parts_eq(&cached_first, &cached_second);

        let mut fresh_eval = ImpureCallEvaluator { calls: 0 };
        let mut unused_cache = HashMap::new();
        let mut fresh_profile = PuctSearchProfile::default();
        let mut fresh_evals = 0usize;
        let mut fresh_states = HashSet::new();
        let mut fresh_obs = HashSet::new();
        let fresh_first = expand_once(
            state.clone(),
            &mut fresh_eval,
            &mut unused_cache,
            false,
            &mut fresh_profile,
            &mut fresh_evals,
            &mut fresh_states,
            &mut fresh_obs,
        );
        let fresh_second = expand_once(
            state,
            &mut fresh_eval,
            &mut unused_cache,
            false,
            &mut fresh_profile,
            &mut fresh_evals,
            &mut fresh_states,
            &mut fresh_obs,
        );
        assert_eq!(fresh_eval.calls, 2);
        assert_ne!(fresh_first.terminal_value, fresh_second.terminal_value);
        assert_eq!(cached_second.terminal_value, fresh_first.terminal_value);
        assert_ne!(cached_second.terminal_value, fresh_second.terminal_value);
    }

    #[test]
    fn library_default_is_cache_off_and_observation_sensitive_cache_is_equivalent() {
        assert_eq!(PuctLeafCache::default(), PuctLeafCache::Off);
        let root = RunState::combat_fixture();
        let search_config = config(&root, 64);
        let before = serde_json::to_vec(&root).expect("root bytes");
        let mut off_eval = ObservationSensitiveEvaluator { calls: 0 };
        let off =
            puct_search_with_leaf_cache(&root, &search_config, &mut off_eval, PuctLeafCache::Off)
                .expect("cache off");
        let mut default_eval = ObservationSensitiveEvaluator { calls: 0 };
        let default_search =
            puct_search(&root, &search_config, &mut default_eval).expect("library default");
        let mut cached_eval = ObservationSensitiveEvaluator { calls: 0 };
        let cached = puct_search_with_leaf_cache(
            &root,
            &search_config,
            &mut cached_eval,
            PuctLeafCache::ExactState,
        )
        .expect("exact-state cache");
        let mut profiled_off_eval = ObservationSensitiveEvaluator { calls: 0 };
        let (profiled_off, off_profile) = puct_search_profiled(
            &root,
            &search_config,
            &mut profiled_off_eval,
            PuctLeafCache::Off,
        )
        .expect("profiled off");
        let mut profiled_on_eval = ObservationSensitiveEvaluator { calls: 0 };
        let (profiled_on, on_profile) = puct_search_profiled(
            &root,
            &search_config,
            &mut profiled_on_eval,
            PuctLeafCache::ExactState,
        )
        .expect("profiled exact-state");
        assert_root_bytes_unchanged(&root, &before);
        assert_eq!(off, default_search);
        assert_eq!(off, profiled_off);
        assert_same_search_decision(&off, &cached);
        assert_same_search_decision(&cached, &profiled_on);
        assert_eq!(off.leaf_evaluations, off_eval.calls);
        assert_eq!(default_search.leaf_evaluations, default_eval.calls);
        assert_eq!(cached.leaf_evaluations, cached_eval.calls);
        assert_eq!(cached.leaf_evaluations, profiled_on.leaf_evaluations);
        assert!(cached.leaf_evaluations <= off.leaf_evaluations);
        assert_eq!(off_profile.exact_state_cache_hits, 0);
        assert_eq!(off_profile.exact_state_cache_misses, 0);
        assert_eq!(on_profile.evaluate_count, cached.leaf_evaluations as u64);
        assert_eq!(
            on_profile.unique_exact_states + on_profile.duplicate_exact_states,
            on_profile.identity_count
        );
        assert_eq!(
            on_profile.exact_state_cache_hits + on_profile.exact_state_cache_misses,
            on_profile.identity_count
        );
        // The combat fixture tree has no exact-state transpositions under this
        // budget, so cache on/off evaluation counts match. Hit reuse is covered
        // by expand-level tests. Remaining simulations are terminal revisits.
        assert_eq!(on_profile.exact_state_cache_hits, 0);
        assert_eq!(cached.leaf_evaluations, off.leaf_evaluations);
        assert_eq!(PRIVILEGED_PUCT_TEACHER_VERSION, "synchronous_batch1_v3");
    }

    #[test]
    fn teacher_clone_opts_into_exact_state_without_changing_labels() {
        let root = RunState::combat_fixture();
        let before = serde_json::to_vec(&root).expect("root bytes");
        let clone_cfg = clone_config(&root, 1);
        let mut off_eval = ObservationSensitiveEvaluator { calls: 0 };
        let off = puct_clone_episode_with_leaf_cache(
            &root,
            &clone_cfg,
            &mut off_eval,
            PuctLeafCache::Off,
        )
        .expect("clone off");
        let mut on_eval = ObservationSensitiveEvaluator { calls: 0 };
        let on = puct_clone_episode(&root, &clone_cfg, &mut on_eval).expect("teacher clone");
        assert_root_bytes_unchanged(&root, &before);
        assert_eq!(off.outcome, on.outcome);
        assert_eq!(off.steps.len(), on.steps.len());
        for (left, right) in off.steps.iter().zip(&on.steps) {
            assert_eq!(left.selected_index, right.selected_index);
            assert_eq!(left.choices, right.choices);
            assert_eq!(left.visits, right.visits);
            assert_eq!(left.value, right.value);
            assert_eq!(left.transitions, right.transitions);
            assert_eq!(left.completed_simulations, right.completed_simulations);
            assert_eq!(left.stop_reason, right.stop_reason);
        }
    }

    fn runtime_source_epoch() -> String {
        if let Ok(epoch) = std::env::var("STS_PUCT_PERF_SOURCE_EPOCH") {
            if !epoch.is_empty() {
                return epoch;
            }
        }
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo)
            .output()
            .expect("git rev-parse HEAD");
        assert!(output.status.success(), "git rev-parse HEAD failed");
        String::from_utf8(output.stdout)
            .expect("git sha utf8")
            .trim()
            .to_owned()
    }

    fn required_root_manifest() -> PathBuf {
        let path = PathBuf::from(
            std::env::var("STS_PUCT_PERF_ROOTS")
                .expect("STS_PUCT_PERF_ROOTS must point at a root-manifest.json"),
        );
        assert!(
            path.is_file(),
            "STS_PUCT_PERF_ROOTS is not a file: {}",
            path.display()
        );
        path
    }

    fn load_development_roots(limit: usize) -> Vec<(String, RunState, Vec<u8>)> {
        let manifest_path = required_root_manifest();
        let raw = std::fs::read_to_string(&manifest_path).expect("read root manifest");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse root manifest");
        let roots = parsed
            .get("roots")
            .and_then(serde_json::Value::as_array)
            .expect("roots array");
        let parent = manifest_path.parent().expect("manifest parent");
        let mut loaded = Vec::new();
        for root in roots {
            if root.get("split").and_then(serde_json::Value::as_str) != Some("development") {
                continue;
            }
            let root_id = root
                .get("root_id")
                .and_then(serde_json::Value::as_str)
                .expect("root_id")
                .to_owned();
            let relative = root
                .get("relative_path")
                .and_then(serde_json::Value::as_str)
                .expect("relative_path");
            let snapshot = std::fs::read_to_string(parent.join(relative)).expect("read snapshot");
            let first = restore_run_snapshot_json(&snapshot)
                .expect("restore snapshot")
                .state;
            let second = restore_run_snapshot_json(&snapshot)
                .expect("second restore")
                .state;
            let first_bytes = serde_json::to_vec(&first).expect("serialize restored");
            let second_bytes = serde_json::to_vec(&second).expect("serialize second restore");
            assert_eq!(
                first_bytes, second_bytes,
                "independent restores diverged {root_id}"
            );
            if first.phase != RunPhase::Combat
                || first.combat.is_none()
                || classify_combat_state(&first).is_some()
            {
                continue;
            }
            loaded.push((root_id, first, first_bytes));
            if loaded.len() == limit {
                break;
            }
        }
        assert_eq!(loaded.len(), limit, "expected {limit} development roots");
        loaded
    }

    #[test]
    #[ignore = "set STS_PUCT_PERF_ROOTS to a root-manifest.json; set STS_PUCT_PERF_REPORT_DIR to write artifacts"]
    fn representative_roots_observation_sensitive_cache_equivalence() {
        let roots = load_development_roots(20);
        let mut rows = Vec::new();
        let mut total_profile = PuctSearchProfile::default();
        let mut total_hits = 0u64;
        let mut total_off_evals = 0usize;
        let mut total_on_evals = 0usize;
        for (root_id, root, before_bytes) in &roots {
            let search_config = config(root, 64);
            let mut baseline_eval = ObservationSensitiveEvaluator { calls: 0 };
            let baseline = puct_search_with_leaf_cache(
                root,
                &search_config,
                &mut baseline_eval,
                PuctLeafCache::Off,
            )
            .expect("baseline root");
            assert_eq!(
                serde_json::to_vec(root).expect("serialize after search"),
                *before_bytes,
                "search mutated {root_id}"
            );
            let mut cached_eval = ObservationSensitiveEvaluator { calls: 0 };
            let cached = puct_search_with_leaf_cache(
                root,
                &search_config,
                &mut cached_eval,
                PuctLeafCache::ExactState,
            )
            .expect("cached root");
            assert_same_search_decision(&baseline, &cached);
            assert_eq!(
                serde_json::to_vec(root).expect("serialize after cache"),
                *before_bytes,
                "cached search mutated {root_id}"
            );
            let mut profiled_eval = ObservationSensitiveEvaluator { calls: 0 };
            let (profiled, profile) = puct_search_profiled(
                root,
                &search_config,
                &mut profiled_eval,
                PuctLeafCache::ExactState,
            )
            .expect("profiled root");
            assert_same_search_decision(&baseline, &profiled);
            total_hits += profile.exact_state_cache_hits;
            total_off_evals += baseline.leaf_evaluations;
            total_on_evals += cached.leaf_evaluations;
            total_profile.total_ns += profile.total_ns;
            total_profile.clone_ns += profile.clone_ns;
            total_profile.clone_count += profile.clone_count;
            total_profile.apply_ns += profile.apply_ns;
            total_profile.apply_count += profile.apply_count;
            total_profile.observation_ns += profile.observation_ns;
            total_profile.observation_count += profile.observation_count;
            total_profile.evaluate_ns += profile.evaluate_ns;
            total_profile.evaluate_count += profile.evaluate_count;
            total_profile.choice_projection_ns += profile.choice_projection_ns;
            total_profile.choice_projection_count += profile.choice_projection_count;
            total_profile.select_ns += profile.select_ns;
            total_profile.backup_ns += profile.backup_ns;
            total_profile.identity_ns += profile.identity_ns;
            total_profile.identity_count += profile.identity_count;
            total_profile.exact_state_cache_hits += profile.exact_state_cache_hits;
            total_profile.exact_state_cache_misses += profile.exact_state_cache_misses;
            total_profile.duplicate_exact_states += profile.duplicate_exact_states;
            total_profile.duplicate_observation_choice_keys +=
                profile.duplicate_observation_choice_keys;
            total_profile.unique_exact_states += profile.unique_exact_states;
            total_profile.unique_observation_choice_keys += profile.unique_observation_choice_keys;
            total_profile.serialized_state_bytes += profile.serialized_state_bytes;
            total_profile.tree_nodes += profile.tree_nodes;
            rows.push(serde_json::json!({
                "root_id": root_id,
                "selected_index": baseline.selected_index,
                "visits": baseline.visits,
                "value": baseline.value,
                "transitions": baseline.transitions,
                "completed_simulations": baseline.completed_simulations,
                "leaf_evaluations": baseline.leaf_evaluations,
                "cached_leaf_evaluations": cached.leaf_evaluations,
                "stop_reason": baseline.stop_reason.as_str(),
                "exact_state_cache_hits": profile.exact_state_cache_hits,
                "profile": profile,
            }));
        }
        assert!(
            total_hits > 0,
            "representative roots produced no exact-state hits"
        );
        if let Some(dir) = std::env::var_os("STS_PUCT_PERF_REPORT_DIR").map(PathBuf::from) {
            let report = serde_json::json!({
                "source_epoch": runtime_source_epoch(),
                "teacher_version": PRIVILEGED_PUCT_TEACHER_VERSION,
                "budgets": {"simulation": 64, "transition": 64, "c_puct": 1.5},
                "evaluator": "observation_sensitive_rust",
                "root_count": rows.len(),
                "cache_policy": "library_off_teacher_exact_state",
                "total_off_leaf_evaluations": total_off_evals,
                "total_on_leaf_evaluations": total_on_evals,
                "exact_state_cache_hits": total_hits,
                "aggregates": total_profile,
                "exact_state_duplicate_rate":
                    (total_profile.duplicate_exact_states as f64)
                        / (total_profile.identity_count.max(1) as f64),
                "observation_duplicate_rate":
                    (total_profile.duplicate_observation_choice_keys as f64)
                        / (total_profile.identity_count.max(1) as f64),
                "roots": rows,
            });
            std::fs::create_dir_all(&dir).expect("create profiling dir");
            std::fs::write(
                dir.join("profiling-report.json"),
                serde_json::to_vec_pretty(&report).expect("serialize profiling report"),
            )
            .expect("write profiling report");
        }
    }
}
