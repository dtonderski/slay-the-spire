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
//! `fair_leaf_batch_v1` is intentionally batch-size 1 and not an extensible
//! request protocol; request/response correlation ids are deferred.

use serde::{Deserialize, Serialize};
use sts_core::{
    apply_run_decision_action, fair_combat_observation, player_choices, resolve_player_choice,
    CombatPhase, DecisionRevision, FairCombatObservation, PlayerChoice, PlayerChoiceRequest,
    Potion, RunAction, RunDecisionAction, RunPhase, RunState,
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
    pub leaf_evaluations: usize,
    pub stop_reason: PuctStopReason,
    pub choices: Vec<PlayerChoice>,
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

pub fn puct_search<E: FairLeafEvaluator>(
    root: &RunState,
    config: &PuctConfig,
    evaluator: &mut E,
) -> Result<PuctSearchResult, PuctError> {
    config.validate()?;
    if root.phase != RunPhase::Combat || root.combat.is_none() {
        return Err(PuctError::NotInCombat);
    }
    if classify_combat_state(root).is_some() {
        return Err(PuctError::TerminalRoot);
    }
    root.validate()
        .map_err(|error| PuctError::InvalidConfig(error.to_string()))?;

    let mut nodes = Vec::new();
    let mut leaf_evaluations = 0usize;
    let mut root_node = expand_ongoing(root, evaluator, &mut leaf_evaluations)?;
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
                backup(&mut nodes, &path, value);
                completed_simulations += 1;
                break;
            }
            if nodes[node_idx].edges.is_empty() {
                return Err(PuctError::EmptyChoices);
            }
            let edge_idx = select_puct_index(&nodes[node_idx], config.c_puct)?;
            if let Some(child_idx) = nodes[node_idx].edges[edge_idx].child {
                path.push((node_idx, edge_idx));
                node_idx = child_idx;
                continue;
            }
            let action = nodes[node_idx].edges[edge_idx].action;
            let parent_state = nodes[node_idx].state.clone();
            let child_state = apply_run_decision_action(&parent_state, action)
                .map_err(|error| PuctError::Transition(error.to_string()))?;
            transitions += 1;
            path.push((node_idx, edge_idx));
            let child_idx = nodes.len();
            if let Some(status) =
                classify_combat_episode_transition(&parent_state, &action, &child_state)
            {
                let value = proxy_value(&child_state, status, config)?;
                nodes.push(Node {
                    state: child_state,
                    edges: Vec::new(),
                    visit_count: 0,
                    terminal_value: Some(value),
                });
                nodes[node_idx].edges[edge_idx].child = Some(child_idx);
                backup(&mut nodes, &path, value);
                completed_simulations += 1;
                break;
            }
            let mut child = expand_ongoing(&child_state, evaluator, &mut leaf_evaluations)?;
            let value = child
                .terminal_value
                .expect("ongoing expand stores evaluator value");
            child.terminal_value = None;
            nodes.push(child);
            nodes[node_idx].edges[edge_idx].child = Some(child_idx);
            backup(&mut nodes, &path, value);
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
    let value = root_backed_up_mean(root_node)?;
    Ok(PuctSearchResult {
        selected_index,
        selected_choice: selected_edge.choice,
        selected_action: selected_edge.action,
        visits,
        priors: root_priors,
        value,
        transitions,
        completed_simulations,
        leaf_evaluations,
        stop_reason,
        choices: root_choices,
    })
}

fn expand_ongoing<E: FairLeafEvaluator>(
    state: &RunState,
    evaluator: &mut E,
    leaf_evaluations: &mut usize,
) -> Result<Node, PuctError> {
    let (choices, actions) = public_choice_actions(state)?;
    let observation = fair_combat_observation(state)
        .map_err(|error| PuctError::Observation(error.to_string()))?;
    let evaluation = evaluator
        .evaluate(&observation, &choices)
        .map_err(PuctError::Evaluator)?;
    *leaf_evaluations += 1;
    let (priors, value) = validate_evaluation(choices.len(), &evaluation)?;
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
    Ok(Node {
        state: state.clone(),
        edges,
        visit_count: 0,
        terminal_value: Some(value),
    })
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
    use sts_core::{fair_combat_observation, CombatAction};

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
}
