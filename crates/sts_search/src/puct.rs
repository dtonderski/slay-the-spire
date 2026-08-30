//! Naive deterministic privileged PUCT over public combat choices.
//!
//! Search clones authoritative `RunState` values. The leaf evaluator receives
//! only a detached fair observation and the public choice list.

use serde::{Deserialize, Serialize};
use sts_core::{
    apply_run_decision_action, fair_combat_observation, player_choices, resolve_player_choice,
    CombatPhase, DecisionRevision, FairCombatObservation, PlayerChoice, PlayerChoiceRequest,
    Potion, RunAction, RunDecisionAction, RunPhase, RunState,
};

pub const FAIR_LEAF_BATCH_SCHEMA: &str = "fair_leaf_batch_v1";
pub const PRIVILEGED_PUCT_TEACHER_NAME: &str = "privileged_puct";
pub const PRIVILEGED_PUCT_TEACHER_VERSION: &str = "synchronous_batch1_v1";

const SEARCH_REVISION: DecisionRevision = DecisionRevision::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub transition_budget: usize,
    pub reward: CombatProxyConfig,
}

impl PuctConfig {
    pub fn validate(&self) -> Result<(), PuctError> {
        if !self.c_puct.is_finite() || self.c_puct < 0.0 {
            return Err(PuctError::InvalidConfig(
                "c_puct must be finite and nonnegative".to_owned(),
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
    pub unique_evaluations: usize,
    pub budget_exhausted: bool,
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
    if combat_status(root).is_some() {
        return Err(PuctError::TerminalRoot);
    }
    root.validate()
        .map_err(|error| PuctError::InvalidConfig(error.to_string()))?;

    let mut nodes = Vec::new();
    let mut unique_evaluations = 0usize;
    let root_eval = expand_ongoing(root, evaluator, &mut unique_evaluations)?;
    let root_priors = root_eval
        .edges
        .iter()
        .map(|edge| edge.prior)
        .collect::<Vec<_>>();
    let root_choices = root_eval
        .edges
        .iter()
        .map(|edge| edge.choice)
        .collect::<Vec<_>>();
    let root_value = root_eval
        .terminal_value
        .expect("ongoing root stores its evaluator value in terminal_value until children exist");
    let mut root_node = root_eval;
    root_node.terminal_value = None;
    let mut unexpanded = root_node.edges.len();
    nodes.push(root_node);

    let mut transitions = 0usize;
    let mut completed_simulations = 0usize;
    let mut budget_exhausted = false;

    loop {
        if unexpanded == 0 {
            break;
        }
        if transitions >= config.transition_budget {
            budget_exhausted = true;
            break;
        }
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut node_idx = 0usize;
        loop {
            if let Some(value) = nodes[node_idx].terminal_value {
                backup(&mut nodes, &path, value);
                completed_simulations += 1;
                break;
            }
            if nodes[node_idx].edges.is_empty() {
                return Err(PuctError::EmptyChoices);
            }
            let edge_idx = select_puct_index(&nodes[node_idx], config.c_puct)?;
            if nodes[node_idx].edges[edge_idx].child.is_some() {
                path.push((node_idx, edge_idx));
                node_idx = nodes[node_idx].edges[edge_idx]
                    .child
                    .expect("expanded child");
                continue;
            }
            if transitions >= config.transition_budget {
                budget_exhausted = true;
                break;
            }
            let action = nodes[node_idx].edges[edge_idx].action;
            let parent_state = nodes[node_idx].state.clone();
            let child_state = apply_run_decision_action(&parent_state, action)
                .map_err(|error| PuctError::Transition(error.to_string()))?;
            transitions += 1;
            unexpanded = unexpanded.saturating_sub(1);
            path.push((node_idx, edge_idx));
            let child_idx = nodes.len();
            if let Some(status) = transition_status(&parent_state, &action, &child_state) {
                let value = proxy_value(root, &child_state, status, &config.reward)?;
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
            let mut child = expand_ongoing(&child_state, evaluator, &mut unique_evaluations)?;
            let value = child
                .terminal_value
                .expect("ongoing expand stores evaluator value");
            unexpanded += child.edges.len();
            child.terminal_value = None;
            nodes.push(child);
            nodes[node_idx].edges[edge_idx].child = Some(child_idx);
            backup(&mut nodes, &path, value);
            completed_simulations += 1;
            break;
        }
        if budget_exhausted {
            break;
        }
    }

    let root_node = &nodes[0];
    let visits = root_node
        .edges
        .iter()
        .map(|edge| edge.visit_count)
        .collect::<Vec<_>>();
    let selected_index = argmax_visits(&visits);
    let selected_edge = &root_node.edges[selected_index];
    let selected_q = if selected_edge.visit_count == 0 {
        root_value
    } else {
        selected_edge.value_sum / selected_edge.visit_count as f64
    };
    if !selected_q.is_finite() {
        return Err(PuctError::MalformedEvaluation(
            "backed-up PUCT value is not finite".to_owned(),
        ));
    }
    Ok(PuctSearchResult {
        selected_index,
        selected_choice: selected_edge.choice,
        selected_action: selected_edge.action,
        visits,
        priors: root_priors,
        value: selected_q,
        transitions,
        completed_simulations,
        unique_evaluations,
        budget_exhausted,
        choices: root_choices,
    })
}

fn expand_ongoing<E: FairLeafEvaluator>(
    state: &RunState,
    evaluator: &mut E,
    unique_evaluations: &mut usize,
) -> Result<Node, PuctError> {
    let (choices, actions) = public_choice_actions(state)?;
    let observation = fair_combat_observation(state)
        .map_err(|error| PuctError::Observation(error.to_string()))?;
    let evaluation = evaluator
        .evaluate(&observation, &choices)
        .map_err(PuctError::Evaluator)?;
    *unique_evaluations += 1;
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
        actions.push(action);
    }
    if actions.len() != set.choices.len() {
        return Err(PuctError::ChoiceProjection(
            "public choice resolution is not bijective".to_owned(),
        ));
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
    let parent_visits = node.visit_count as f64;
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
        let score =
            q + c_puct * edge.prior * parent_visits.sqrt() / (1.0 + edge.visit_count as f64);
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

fn backup(nodes: &mut [Node], path: &[(usize, usize)], value: f64) {
    for &(node_idx, edge_idx) in path {
        let edge = &mut nodes[node_idx].edges[edge_idx];
        edge.visit_count += 1;
        edge.value_sum += value;
        nodes[node_idx].visit_count += 1;
    }
}

fn combat_status(state: &RunState) -> Option<&'static str> {
    let combat = state.combat.as_ref()?;
    if combat.phase == CombatPhase::Lost || combat.player.hp <= 0 {
        Some("lost")
    } else if combat.phase == CombatPhase::Won {
        Some("won")
    } else {
        None
    }
}

fn transition_status(
    before: &RunState,
    action: &RunDecisionAction,
    after: &RunState,
) -> Option<&'static str> {
    if combat_status(before).is_some() {
        return None;
    }
    if let Some(status) = combat_status(after) {
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

fn proxy_value(
    root: &RunState,
    state: &RunState,
    status: &str,
    config: &CombatProxyConfig,
) -> Result<f64, PuctError> {
    let (terminal_hp, terminal_max_hp) = player_hp_and_max(state);
    let (_, root_max_hp) = player_hp_and_max(root);
    config.value(
        status,
        terminal_hp,
        terminal_max_hp,
        terminal_max_hp - root_max_hp,
        state.gold - root.gold,
        remaining_potions(state),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_core::fair_combat_observation;

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

    fn config(budget: usize) -> PuctConfig {
        PuctConfig {
            c_puct: 1.5,
            transition_budget: budget,
            reward: CombatProxyConfig::default(),
        }
    }

    #[test]
    fn search_is_deterministic_and_reports_accounting() {
        let root = RunState::combat_fixture();
        let search_config = config(8);
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
        assert_eq!(first.unique_evaluations, first_eval.calls);
        assert!(first.unique_evaluations >= 1);
        assert!(first.budget_exhausted);
        assert_eq!(first.visits.iter().sum::<u64>(), 8);
        assert_eq!(first.choices.len(), first.visits.len());
        assert_eq!(first.selected_choice, first.choices[first.selected_index]);
    }

    #[test]
    fn transition_budget_does_not_overshoot() {
        let root = RunState::combat_fixture();
        for budget in [0usize, 1, 3, 7] {
            let mut evaluator = UniformEvaluator {
                value: 0.0,
                calls: 0,
            };
            let result =
                puct_search(&root, &config(budget), &mut evaluator).expect("budgeted search");
            assert!(result.transitions <= budget, "budget {budget}");
            assert!(result.unique_evaluations <= budget.saturating_add(1));
            if budget == 0 {
                assert_eq!(result.transitions, 0);
                assert_eq!(result.completed_simulations, 0);
                assert_eq!(result.unique_evaluations, 1);
                assert!(result.visits.iter().all(|visits| *visits == 0));
                assert_eq!(result.selected_index, 0);
            }
        }
    }

    #[test]
    fn priors_influence_root_selection() {
        let root = RunState::combat_fixture();
        let target = player_choices(&root, SEARCH_REVISION)
            .expect("choices")
            .choices
            .iter()
            .position(|choice| *choice == PlayerChoice::EndTurn)
            .expect("fixture has EndTurn");
        let mut evaluator = BiasedEvaluator { value: 0.0 };
        let result = puct_search(&root, &config(24), &mut evaluator).expect("biased search");
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
        let result = puct_search(&root, &config(1), &mut evaluator).expect("one expansion");
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
            let error = puct_search(&root, &config(1), &mut evaluator).expect_err("malformed");
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
        puct_search(&root, &config(2), &mut evaluator).expect("search");
        assert!(evaluator.calls >= 1);
    }

    #[test]
    fn combat_proxy_matches_disjoint_status_bands() {
        let config = CombatProxyConfig::default();
        config.validate().expect("default config");
        let lost = config.value("lost", 0, 80, 0, 0, 0).expect("lost");
        let escaped = config.value("escaped", 40, 80, 0, 0, 0).expect("escaped");
        let won = config.value("won", 80, 80, 0, 0, 0).expect("won");
        assert_eq!(lost, -1.0);
        assert!(escaped > lost);
        assert!(won > escaped);
        assert!(won <= 1.0);
        let overlapping = CombatProxyConfig {
            win_base: 0.3,
            escape_base: 0.25,
            resource_clip: 0.2,
            ..CombatProxyConfig::default()
        };
        assert!(overlapping.validate().is_err());
    }
}
