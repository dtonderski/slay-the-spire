//! Deterministic planning over authoritative simulator `RunState` values.

mod puct;

pub use puct::{
    classify_combat_episode_transition, classify_combat_state, puct_search, CombatProxyConfig,
    FairLeafEvaluation, FairLeafEvaluator, PuctConfig, PuctError, PuctSearchResult, PuctStopReason,
    FAIR_LEAF_BATCH_SCHEMA, PRIVILEGED_PUCT_TEACHER_NAME, PRIVILEGED_PUCT_TEACHER_VERSION,
};

use serde::Serialize;
use std::cmp::Ordering;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use sts_core::{
    apply_run_decision_action,
    card::CardType,
    combat::{ExhaustSelectPurpose, HandSelectPurpose},
    content::{cards::get_card_definition, monsters::get_monster_definition},
    legal_combat_actions, legal_run_decision_actions,
    potion::PotionRarity,
    CardId, CombatAction, CombatPhase, CombatState, ContentId, MonsterId, MonsterIntent, Potion,
    RunAction, RunDecisionAction, RunPhase, RunState, SimError, SimResult,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPolicy {
    Greedy,
    Beam,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchConfig {
    pub policy: SearchPolicy,
    pub depth: usize,
    pub width: usize,
    pub allowed_potion_slots: Vec<usize>,
    pub transition_budget: usize,
    pub time_budget_ms: u64,
    pub deduplicate_states: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            policy: SearchPolicy::Beam,
            depth: 100,
            width: 300,
            allowed_potion_slots: (0..5).collect(),
            transition_budget: 100_000,
            time_budget_ms: 30_000,
            deduplicate_states: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannerAction {
    Combat(CombatAction),
    Potion(RunAction),
    Run(RunAction),
}

#[derive(Clone)]
struct SearchNode {
    state: RunState,
    first_action: Option<PlannerAction>,
    principal_variation: Vec<PlannerAction>,
    actions: usize,
    score: f64,
    terminal_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchRecommendation {
    pub principal_variation: Vec<PlannerAction>,
    pub value: f64,
    pub nodes: usize,
    pub terminal_reason: Option<String>,
    pub final_hp: i32,
    pub monster_hp: i32,
    pub budget_exhausted: bool,
    pub timed_out: bool,
    pub expanded: usize,
    pub generated: usize,
    pub terminal_nodes: usize,
    pub pruned: usize,
    pub max_frontier: usize,
    pub duplicate_checks: usize,
    pub duplicates: usize,
    pub cache_hits: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkSearchResult {
    pub outcome: String,
    pub terminal_reason: Option<String>,
    pub final_hp: i32,
    pub final_max_hp: i32,
    pub final_gold: i32,
    pub remaining_potions: usize,
    pub remaining_potion_value: usize,
    pub remaining_monster_hp: i32,
    pub transitions: usize,
    pub budget_exhausted: bool,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    pub expanded: usize,
    pub generated: usize,
    pub terminal_nodes: usize,
    pub pruned: usize,
    pub max_frontier: usize,
    pub duplicate_checks: usize,
    pub duplicates: usize,
    pub cache_hits: usize,
    pub actions: usize,
    pub action_labels: Vec<String>,
    pub replay_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum CombatOutcome {
    Lost,
    Ongoing,
    Escaped,
    Won,
}

/// Detached result from replanning at one public decision.
#[derive(Debug, Clone)]
pub struct BeamTeacherDecision {
    pub action: RunDecisionAction,
    pub nodes: usize,
    pub value: f64,
    pub budget_exhausted: bool,
}

/// Replan with the production beam search at one public decision.
pub fn beam_teacher_decision(
    state: &RunState,
    depth: usize,
    width: usize,
    transition_budget: usize,
    deduplicate_states: bool,
) -> SimResult<BeamTeacherDecision> {
    let config = SearchConfig {
        policy: SearchPolicy::Beam,
        depth,
        width,
        allowed_potion_slots: (0..state.potion_capacity()).collect(),
        transition_budget,
        time_budget_ms: 0,
        deduplicate_states,
    };
    let recommendation = beam_search_with_warm_start(state, &config, &[])?;
    let first = recommendation
        .principal_variation
        .first()
        .ok_or(SimError::IllegalAction("beam teacher produced no action"))?;
    Ok(BeamTeacherDecision {
        action: first.as_run_decision_action(),
        nodes: recommendation.nodes,
        value: recommendation.value,
        budget_exhausted: recommendation.budget_exhausted,
    })
}

impl PlannerAction {
    #[must_use]
    pub fn as_run_decision_action(&self) -> RunDecisionAction {
        match self {
            Self::Combat(action) => RunDecisionAction::Combat(*action),
            Self::Potion(action) | Self::Run(action) => RunDecisionAction::Run(*action),
        }
    }
}

fn greedy_search(state: &RunState, config: &SearchConfig) -> SimResult<SearchRecommendation> {
    let mut current = state.clone();
    let mut principal_variation = Vec::new();
    let mut nodes = 1usize;
    let mut terminal = terminal_reason(&current);

    while terminal.is_none() && principal_variation.len() < config.depth {
        let actions = planner_actions(&current, config)?;
        if actions.is_empty() {
            break;
        }
        let mut best_action = None;
        let mut best_score = f64::NEG_INFINITY;
        for action in actions {
            let next = apply_planner_action(&current, &action)?;
            nodes += 1;
            let next_terminal_reason = terminal_reason(&next);
            let score = child_score(
                &next,
                next_terminal_reason.as_deref(),
                &current,
                &action,
                principal_variation.len(),
            );
            if best_action.is_none() || score > best_score {
                best_score = score;
                best_action = Some(action);
            }
        }
        let Some(action) = best_action else {
            break;
        };
        let next = apply_planner_action(&current, &action)?;
        principal_variation.push(action);
        current = next;
        terminal = terminal_reason(&current);
    }

    let value = search_score(&current, terminal.as_deref());
    let (final_hp, monster_hp) = combat_hp(&current);
    let terminal_nodes = usize::from(terminal.is_some());
    Ok(SearchRecommendation {
        principal_variation,
        value,
        nodes,
        terminal_reason: terminal,
        final_hp,
        monster_hp,
        budget_exhausted: false,
        timed_out: false,
        expanded: nodes.saturating_sub(1),
        generated: nodes.saturating_sub(1),
        terminal_nodes,
        pruned: 0,
        max_frontier: 1,
        duplicate_checks: 0,
        duplicates: 0,
        cache_hits: 0,
    })
}

fn beam_search_with_warm_start(
    state: &RunState,
    config: &SearchConfig,
    warm_actions: &[PlannerAction],
) -> SimResult<SearchRecommendation> {
    beam_search_with_node_limit(
        state,
        config,
        config.transition_budget.saturating_add(1),
        warm_actions,
        (config.time_budget_ms > 0)
            .then(|| Instant::now() + Duration::from_millis(config.time_budget_ms)),
    )
}

fn beam_search_with_node_limit(
    state: &RunState,
    config: &SearchConfig,
    node_limit: usize,
    warm_actions: &[PlannerAction],
    deadline: Option<Instant>,
) -> SimResult<SearchRecommendation> {
    let (warm_node, cache_hits) = validated_warm_start(state, warm_actions)?;
    let primary = action_depth_beam_search_with_node_limit(
        state,
        config,
        node_limit,
        warm_node.as_ref(),
        cache_hits,
        deadline,
    )?;
    if primary.terminal_reason.is_some() || primary.nodes >= node_limit || primary.timed_out {
        return Ok(primary);
    }

    let remaining_node_limit = node_limit.saturating_sub(primary.nodes).saturating_add(1);
    let fallback = complete_turn_beam_search_with_node_limit(
        state,
        config,
        remaining_node_limit,
        warm_node.as_ref(),
        0,
        deadline,
    )?;
    Ok(combine_fallback_effort(primary, fallback))
}

fn combine_fallback_effort(
    primary: SearchRecommendation,
    fallback: SearchRecommendation,
) -> SearchRecommendation {
    let use_fallback = matches!(fallback.terminal_reason.as_deref(), Some("won" | "escaped"));
    let (mut selected, other) = if use_fallback {
        (fallback, primary)
    } else {
        (primary, fallback)
    };
    selected.nodes = selected.nodes.saturating_add(other.nodes).saturating_sub(1);
    selected.budget_exhausted |= other.budget_exhausted;
    selected.timed_out |= other.timed_out;
    selected.expanded += other.expanded;
    selected.generated += other.generated;
    selected.terminal_nodes += other.terminal_nodes;
    selected.pruned += other.pruned;
    selected.max_frontier = selected.max_frontier.max(other.max_frontier);
    selected.duplicate_checks += other.duplicate_checks;
    selected.duplicates += other.duplicates;
    selected.cache_hits += other.cache_hits;
    selected
}

fn action_depth_beam_search_with_node_limit(
    state: &RunState,
    config: &SearchConfig,
    node_limit: usize,
    warm_node: Option<&SearchNode>,
    cache_hits: usize,
    deadline: Option<Instant>,
) -> SimResult<SearchRecommendation> {
    let initial_terminal_reason = terminal_reason(state);
    let mut best = SearchNode {
        state: state.clone(),
        first_action: None,
        principal_variation: Vec::new(),
        actions: 0,
        score: search_score(state, initial_terminal_reason.as_deref()),
        terminal_reason: initial_terminal_reason,
    };
    let mut frontier = vec![best.clone()];
    if let Some(warm_node) = warm_node {
        if node_better(warm_node, &best) {
            best = warm_node.clone();
        }
    }
    let mut nodes = 1usize;
    let width = config.width.max(1);
    let mut expanded = 0usize;
    let mut generated = 0usize;
    let mut terminal_nodes = 0usize;
    let mut pruned = 0usize;
    let mut max_frontier = frontier.len();
    let mut duplicate_checks = 0usize;
    let mut duplicates = 0usize;
    let mut budget_exhausted = false;
    let mut timed_out = false;

    'depth: for _ in 0..config.depth {
        let mut next_frontier = Vec::new();
        for node in std::mem::take(&mut frontier) {
            if node.terminal_reason.is_some() {
                if node_better(&node, &best) {
                    best = node.clone();
                }
                continue;
            }
            expanded += 1;
            let actions = planner_actions(&node.state, config)?;
            if actions.is_empty() {
                if node_better(&node, &best) {
                    best = node.clone();
                }
                next_frontier.push(node);
                continue;
            }
            for action in actions {
                if nodes >= node_limit
                    || deadline.is_some_and(|deadline| Instant::now() >= deadline)
                {
                    budget_exhausted = true;
                    timed_out = nodes < node_limit;
                    break 'depth;
                }
                let next_state = apply_planner_action(&node.state, &action)?;
                nodes += 1;
                generated += 1;
                let child_terminal_reason = terminal_reason(&next_state);
                let score = child_score(
                    &next_state,
                    child_terminal_reason.as_deref(),
                    &node.state,
                    &action,
                    node.actions,
                );
                let mut principal_variation = node.principal_variation.clone();
                principal_variation.push(action.clone());
                let child = SearchNode {
                    state: next_state,
                    first_action: node.first_action.clone().or(Some(action)),
                    principal_variation,
                    actions: node.actions + 1,
                    score,
                    terminal_reason: child_terminal_reason,
                };
                if node_better(&child, &best) {
                    best = child.clone();
                }
                if child.terminal_reason.is_some() {
                    terminal_nodes += 1;
                    continue;
                }
                next_frontier.push(child);
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        let before = next_frontier.len();
        next_frontier = prune_frontier(next_frontier, width);
        pruned += before.saturating_sub(next_frontier.len());
        if config.deduplicate_states {
            let (deduplicated, checks, removed) = deduplicate_search_nodes(next_frontier);
            next_frontier = deduplicated;
            duplicate_checks += checks;
            duplicates += removed;
        }
        max_frontier = max_frontier.max(next_frontier.len());
        frontier = next_frontier;
    }

    for node in frontier {
        if node_better(&node, &best) {
            best = node;
        }
    }

    let (final_hp, monster_hp) = combat_hp(&best.state);
    Ok(SearchRecommendation {
        principal_variation: best.principal_variation,
        value: best.score,
        nodes,
        terminal_reason: best.terminal_reason,
        final_hp,
        monster_hp,
        budget_exhausted,
        timed_out,
        expanded,
        generated,
        terminal_nodes,
        pruned,
        max_frontier,
        duplicate_checks,
        duplicates,
        cache_hits,
    })
}

fn complete_turn_beam_search_with_node_limit(
    state: &RunState,
    config: &SearchConfig,
    node_limit: usize,
    warm_node: Option<&SearchNode>,
    cache_hits: usize,
    deadline: Option<Instant>,
) -> SimResult<SearchRecommendation> {
    let initial_terminal_reason = terminal_reason(state);
    let mut best = SearchNode {
        state: state.clone(),
        first_action: None,
        principal_variation: Vec::new(),
        actions: 0,
        score: search_score(state, initial_terminal_reason.as_deref()),
        terminal_reason: initial_terminal_reason,
    };
    let mut frontier = vec![best.clone()];
    if let Some(warm_node) = warm_node {
        if node_better(warm_node, &best) {
            best = warm_node.clone();
        }
    }
    let mut nodes = 1usize;
    let width = config.width.max(1);
    let mut expanded = 0usize;
    let mut generated = 0usize;
    let mut terminal_nodes = 0usize;
    let mut pruned = 0usize;
    let mut max_frontier = frontier.len();
    let mut duplicate_checks = 0usize;
    let mut duplicates = 0usize;

    let mut budget_exhausted = false;
    let mut timed_out = false;
    for _ in 0..config.depth {
        frontier = expand_complete_turns(
            std::mem::take(&mut frontier),
            config,
            width,
            node_limit,
            &mut nodes,
            &mut expanded,
            &mut generated,
            &mut terminal_nodes,
            &mut pruned,
            &mut max_frontier,
            &mut budget_exhausted,
            &mut timed_out,
            &mut duplicate_checks,
            &mut duplicates,
            deadline,
            &mut best,
        )?;
        if frontier.is_empty() || budget_exhausted {
            break;
        }
        if frontier.iter().all(|node| node.terminal_reason.is_some()) {
            break;
        }
    }

    for node in frontier {
        if node_better(&node, &best) {
            best = node;
        }
    }

    let (final_hp, monster_hp) = combat_hp(&best.state);
    Ok(SearchRecommendation {
        principal_variation: best.principal_variation,
        value: best.score,
        nodes,
        terminal_reason: best.terminal_reason,
        final_hp,
        monster_hp,
        budget_exhausted,
        timed_out,
        expanded,
        generated,
        terminal_nodes,
        pruned,
        max_frontier,
        duplicate_checks,
        duplicates,
        cache_hits,
    })
}

const MAX_ACTIONS_PER_TURN: usize = 24;

#[allow(clippy::too_many_arguments)]
fn expand_complete_turns(
    turn_roots: Vec<SearchNode>,
    config: &SearchConfig,
    width: usize,
    node_limit: usize,
    nodes: &mut usize,
    expanded: &mut usize,
    generated: &mut usize,
    terminal_nodes: &mut usize,
    pruned: &mut usize,
    max_frontier: &mut usize,
    budget_exhausted: &mut bool,
    timed_out: &mut bool,
    duplicate_checks: &mut usize,
    duplicates: &mut usize,
    deadline: Option<Instant>,
    best: &mut SearchNode,
) -> SimResult<Vec<SearchNode>> {
    let mut active = turn_roots;
    let mut completed = Vec::new();

    for _ in 0..MAX_ACTIONS_PER_TURN {
        let mut next_active = Vec::new();
        for node in std::mem::take(&mut active) {
            if node.terminal_reason.is_some() {
                if node_better(&node, best) {
                    *best = node.clone();
                }
                completed.push(node);
                continue;
            }
            *expanded += 1;
            let actions = planner_actions(&node.state, config)?;
            if actions.is_empty() {
                if node_better(&node, best) {
                    *best = node.clone();
                }
                completed.push(node);
                continue;
            }
            for action in actions {
                if *nodes >= node_limit
                    || deadline.is_some_and(|deadline| Instant::now() >= deadline)
                {
                    *budget_exhausted = true;
                    *timed_out = *nodes < node_limit;
                    break;
                }
                let next_state = apply_planner_action(&node.state, &action)?;
                *nodes += 1;
                *generated += 1;
                let child_terminal_reason = terminal_reason(&next_state);
                let score = child_score(
                    &next_state,
                    child_terminal_reason.as_deref(),
                    &node.state,
                    &action,
                    node.actions,
                );
                let mut principal_variation = node.principal_variation.clone();
                principal_variation.push(action.clone());
                let turn_ended = matches!(action, PlannerAction::Combat(CombatAction::EndTurn));
                let child = SearchNode {
                    state: next_state,
                    first_action: node.first_action.clone().or(Some(action)),
                    principal_variation,
                    actions: node.actions + 1,
                    score,
                    terminal_reason: child_terminal_reason,
                };
                if node_better(&child, best) {
                    *best = child.clone();
                }
                if child.terminal_reason.is_some() {
                    *terminal_nodes += 1;
                    completed.push(child);
                } else if turn_ended {
                    completed.push(child);
                } else {
                    next_active.push(child);
                }
            }
            if *budget_exhausted {
                break;
            }
        }
        if *budget_exhausted || next_active.is_empty() {
            break;
        }
        let before = next_active.len();
        active = prune_frontier(next_active, width);
        *pruned += before.saturating_sub(active.len());
        if config.deduplicate_states {
            let (deduplicated, checks, removed) = deduplicate_search_nodes(active);
            active = deduplicated;
            *duplicate_checks += checks;
            *duplicates += removed;
        }
        *max_frontier = (*max_frontier).max(active.len());
    }

    for node in active {
        if node_better(&node, best) {
            *best = node;
        }
    }
    let before = completed.len();
    let mut completed = prune_frontier(completed, width);
    *pruned += before.saturating_sub(completed.len());
    if config.deduplicate_states {
        let (deduplicated, checks, removed) = deduplicate_search_nodes(completed);
        completed = deduplicated;
        *duplicate_checks += checks;
        *duplicates += removed;
    }
    *max_frontier = (*max_frontier).max(completed.len());
    Ok(completed)
}

fn deduplicate_search_nodes(nodes: Vec<SearchNode>) -> (Vec<SearchNode>, usize, usize) {
    let checks = nodes.len();
    let mut indices = HashMap::<Vec<u8>, usize>::with_capacity(nodes.len());
    let mut unique = Vec::<SearchNode>::with_capacity(nodes.len());
    for node in nodes {
        let key = serde_json::to_vec(&node.state)
            .expect("authoritative RunState must remain serializable for search caching");
        if let Some(index) = indices.get(&key).copied() {
            if node_better(&node, &unique[index]) {
                unique[index] = node;
            }
        } else {
            indices.insert(key, unique.len());
            unique.push(node);
        }
    }
    let duplicates = checks.saturating_sub(unique.len());
    (unique, checks, duplicates)
}

fn validated_warm_start(
    state: &RunState,
    actions: &[PlannerAction],
) -> SimResult<(Option<SearchNode>, usize)> {
    let mut current = state.clone();
    let mut principal_variation = Vec::new();
    let mut score = search_score(&current, terminal_reason(&current).as_deref());
    let mut terminal = terminal_reason(&current);

    for action in actions {
        if terminal.is_some() {
            break;
        }
        let parent = current.clone();
        let next = match apply_planner_action(&parent, action) {
            Ok(next) => next,
            Err(SimError::IllegalAction(_)) => break,
            Err(error) => return Err(error),
        };
        terminal = terminal_reason(&next);
        score = child_score(
            &next,
            terminal.as_deref(),
            &parent,
            action,
            principal_variation.len(),
        );
        principal_variation.push(action.clone());
        current = next;
    }

    let cache_hits = principal_variation.len();
    if principal_variation.is_empty() {
        return Ok((None, 0));
    }
    Ok((
        Some(SearchNode {
            state: current,
            first_action: principal_variation.first().cloned(),
            actions: principal_variation.len(),
            principal_variation,
            score,
            terminal_reason: terminal,
        }),
        cache_hits,
    ))
}

pub fn benchmark_beam_search(
    state: &RunState,
    config: &SearchConfig,
    transition_budget: usize,
) -> BenchmarkSearchResult {
    let started = Instant::now();
    let recommendation = beam_search_with_node_limit(
        state,
        config,
        transition_budget.saturating_add(1),
        &[],
        None,
    );
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let recommendation = match recommendation {
        Ok(recommendation) => recommendation,
        Err(error) => {
            let (final_hp, final_max_hp) = state
                .combat
                .as_ref()
                .map(|combat| (combat.player.hp, combat.player.max_hp))
                .unwrap_or((state.player_hp, state.player_max_hp));
            let (_, remaining_monster_hp) = combat_hp(state);
            return BenchmarkSearchResult {
                outcome: "illegal".to_owned(),
                terminal_reason: terminal_reason(state),
                final_hp,
                final_max_hp,
                final_gold: state.gold,
                remaining_potions: state.potions.len(),
                remaining_potion_value: potion_inventory_value(state),
                remaining_monster_hp,
                transitions: 0,
                budget_exhausted: false,
                timed_out: false,
                elapsed_ms,
                expanded: 0,
                generated: 0,
                terminal_nodes: 0,
                pruned: 0,
                max_frontier: 0,
                duplicate_checks: 0,
                duplicates: 0,
                cache_hits: 0,
                actions: 0,
                action_labels: Vec::new(),
                replay_error: Some(error.to_string()),
            };
        }
    };
    let mut replay = state.clone();
    let mut replay_error = None;
    let mut action_labels = Vec::new();
    for action in &recommendation.principal_variation {
        action_labels.push(planner_action_display_label(&replay, action));
        match apply_planner_action(&replay, action) {
            Ok(next) => replay = next,
            Err(error) => {
                replay_error = Some(error.to_string());
                break;
            }
        }
    }
    let terminal = terminal_reason(&replay);
    let outcome = match terminal.as_deref() {
        Some("won") => "won",
        Some("escaped") => "escaped",
        Some("lost") => "lost",
        _ if replay_error.is_some() => "illegal",
        _ => "nonterminal",
    }
    .to_owned();
    let (final_hp, final_max_hp) = replay
        .combat
        .as_ref()
        .map(|combat| (combat.player.hp, combat.player.max_hp))
        .unwrap_or((replay.player_hp, replay.player_max_hp));
    let (_, remaining_monster_hp) = combat_hp(&replay);

    BenchmarkSearchResult {
        outcome,
        terminal_reason: terminal,
        final_hp,
        final_max_hp,
        final_gold: replay.gold,
        remaining_potions: replay.potions.len(),
        remaining_potion_value: potion_inventory_value(&replay),
        remaining_monster_hp,
        transitions: recommendation.nodes.saturating_sub(1),
        budget_exhausted: recommendation.budget_exhausted,
        timed_out: recommendation.timed_out,
        elapsed_ms,
        expanded: recommendation.expanded,
        generated: recommendation.generated,
        terminal_nodes: recommendation.terminal_nodes,
        pruned: recommendation.pruned,
        max_frontier: recommendation.max_frontier,
        duplicate_checks: recommendation.duplicate_checks,
        duplicates: recommendation.duplicates,
        cache_hits: recommendation.cache_hits,
        actions: recommendation.principal_variation.len(),
        action_labels,
        replay_error,
    }
}

pub fn planner_actions(state: &RunState, config: &SearchConfig) -> SimResult<Vec<PlannerAction>> {
    if state.phase != RunPhase::Combat {
        return Ok(Vec::new());
    }

    let legal = legal_run_decision_actions(state)?;
    let suppress_hand_retargets =
        planner_suppresses_completed_exact_count_hand_retargets(state, &legal);
    let suppress_exhaust_retargets =
        planner_suppresses_completed_exact_count_exhaust_retargets(state, &legal);

    Ok(legal
        .into_iter()
        .filter_map(|action| match action {
            RunDecisionAction::Run(RunAction::ChooseHandSelect { .. })
                if suppress_hand_retargets =>
            {
                None
            }
            RunDecisionAction::Run(RunAction::ChooseExhaustSelect { .. })
                if suppress_exhaust_retargets =>
            {
                None
            }
            RunDecisionAction::Combat(action) => Some(PlannerAction::Combat(action)),
            RunDecisionAction::Run(action @ RunAction::UsePotion { slot, .. })
                if planner_allows_potion(state, config, slot) =>
            {
                Some(PlannerAction::Potion(action))
            }
            RunDecisionAction::Run(
                action @ (RunAction::ChooseCombatCardReward { .. }
                | RunAction::ChooseHandSelect { .. }
                | RunAction::ConfirmHandSelect
                | RunAction::ChooseDrawSelect { .. }
                | RunAction::ConfirmDrawSelect
                | RunAction::ChooseDiscardSelect { .. }
                | RunAction::ConfirmDiscardSelect
                | RunAction::ChooseExhaustSelect { .. }
                | RunAction::ConfirmExhaustSelect),
            ) => Some(PlannerAction::Run(action)),
            _ => None,
        })
        .collect())
}

fn planner_suppresses_completed_exact_count_hand_retargets(
    state: &RunState,
    legal: &[RunDecisionAction],
) -> bool {
    let Some(hand_select) = state.combat.as_ref().and_then(CombatState::hand_select) else {
        return false;
    };
    if !legal.contains(&RunDecisionAction::Run(RunAction::ConfirmHandSelect)) {
        return false;
    }
    // Optional/multi Forethought+ stays retargetable even though empty confirm is legal.
    // Prepared is exact-count: Confirm is only legal once the required number is selected.
    !matches!(
        hand_select.purpose,
        HandSelectPurpose::ForethoughtPutAnyOnDraw
    )
}

fn planner_suppresses_completed_exact_count_exhaust_retargets(
    state: &RunState,
    legal: &[RunDecisionAction],
) -> bool {
    let Some(exhaust_select) = state.combat.as_ref().and_then(CombatState::exhaust_select) else {
        return false;
    };
    if !legal.contains(&RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)) {
        return false;
    }
    matches!(
        exhaust_select.purpose,
        ExhaustSelectPurpose::ExhumeReturnToHand
            | ExhaustSelectPurpose::BurningPactDraw2
            | ExhaustSelectPurpose::BurningPactDraw3
            | ExhaustSelectPurpose::TrueGritExhaustOne
            | ExhaustSelectPurpose::RecycleExhaustOne
    )
}

fn planner_allows_potion(state: &RunState, config: &SearchConfig, slot: usize) -> bool {
    if !config.allowed_potion_slots.contains(&slot) {
        return false;
    }
    let Some(potion) = state.potion_at_slot(slot) else {
        return false;
    };

    // Escaping skips rewards and breaks SlayTheData's post-combat guidance, so
    // keep Smoke Bomb available for manual use but never offer it to search.
    if potion == Potion::SmokeBomb {
        return false;
    }
    // Discovery potion timing can desynchronize cardRandomRng in SuperFastMode.
    // Keep these potions available for manual use until their hidden update
    // contract is modeled deterministically.
    !matches!(
        potion,
        Potion::Attack | Potion::Skill | Potion::Power | Potion::Colorless
    )
}

pub fn apply_planner_action(
    state: &RunState,
    action: &PlannerAction,
) -> sts_core::SimResult<RunState> {
    let action = match action {
        PlannerAction::Combat(action) => RunDecisionAction::Combat(*action),
        PlannerAction::Potion(action) | PlannerAction::Run(action) => {
            RunDecisionAction::Run(*action)
        }
    };
    apply_run_decision_action(state, action)
}

fn terminal_reason(state: &RunState) -> Option<String> {
    if let Some(combat) = state.combat.as_ref() {
        if combat.phase == CombatPhase::Lost || combat.player.hp <= 0 {
            return Some("lost".to_owned());
        }
        if combat.phase == CombatPhase::Won {
            return Some("won".to_owned());
        }
    }
    if state.phase != RunPhase::Combat {
        if state.phase == RunPhase::Idle && state.combat.is_none() {
            return Some("escaped".to_owned());
        }
        return Some("won".to_owned());
    }
    None
}

fn search_score(state: &RunState, terminal_reason: Option<&str>) -> f64 {
    match terminal_reason {
        Some("won") => 1_000_000.0 + terminal_state_value(state),
        Some("escaped") => 1_000_000.0 + terminal_state_value(state) - 12.0,
        Some("lost") => -1_000_000.0 + terminal_state_value(state),
        _ => nonterminal_search_heuristic(state),
    }
}

fn terminal_state_value(state: &RunState) -> f64 {
    let (hp, max_hp) = state
        .combat
        .as_ref()
        .map(|combat| (combat.player.hp, combat.player.max_hp))
        .unwrap_or((state.player_hp, state.player_max_hp));
    f64::from(hp)
        + f64::from(max_hp) * 3.0
        + f64::from(state.gold) / 10.0
        + state.potions.len() as f64 * 8.0
}

fn nonterminal_search_heuristic(state: &RunState) -> f64 {
    let Some(combat) = state.combat.as_ref() else {
        return terminal_state_value(state);
    };
    let player_block = f64::from(combat.player.block);
    let player_energy = f64::from(combat.player.energy);
    let alive_monsters = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .collect::<Vec<_>>();
    let incoming = alive_monsters
        .iter()
        .map(|monster| f64::from(intent_damage(monster.intent)))
        .sum::<f64>();
    let unblocked = (incoming - player_block).max(0.0);
    let useful_block = player_block.min(incoming);
    let monster_hp = alive_monsters
        .iter()
        .map(|monster| f64::from(monster.hp))
        .sum::<f64>();
    let monster_block = alive_monsters
        .iter()
        .map(|monster| f64::from(monster.block))
        .sum::<f64>();
    let alive_count = alive_monsters.len() as f64;
    terminal_state_value(state) - unblocked * 5.0 + useful_block * 0.75 + player_energy * 0.25
        - monster_hp * 1.75
        - monster_block * 0.25
        - alive_count * 12.0
        + hand_attack_damage_potential(combat) * 1.5
}

fn intent_damage(intent: MonsterIntent) -> i32 {
    match intent {
        MonsterIntent::Attack { damage }
        | MonsterIntent::AttackAndBlock { damage, .. }
        | MonsterIntent::AttackApplyPlayerWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrailAndVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrail { damage, .. }
        | MonsterIntent::AttackHealSelf { damage }
        | MonsterIntent::AttackAddWoundsToDiscard { damage, .. }
        | MonsterIntent::AttackAddSlimedToDiscard { damage, .. }
        | MonsterIntent::AttackAddVoidToDraw { damage, .. }
        | MonsterIntent::AttackStealGold { damage, .. } => damage,
        MonsterIntent::AttackMultiple { damage, hits } => damage * hits,
        MonsterIntent::AddBurnToDiscard { damage, .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { damage, .. } => damage,
        _ => 0,
    }
}

fn hand_attack_damage_potential(combat: &CombatState) -> f64 {
    combat
        .piles
        .hand
        .iter()
        .filter_map(|card| get_card_definition(card.content_id))
        .filter(|definition| definition.card_type == CardType::Attack)
        .map(|definition| f64::from(definition.values.damage.unwrap_or(0).max(0)))
        .sum()
}

fn action_penalty(state: &RunState, action: &PlannerAction) -> f64 {
    match action {
        PlannerAction::Potion(_) => 0.0,
        PlannerAction::Run(_) => 0.0,
        PlannerAction::Combat(CombatAction::EndTurn) if has_playable_card_action(state) => 12.0,
        PlannerAction::Combat(CombatAction::EndTurn) => 0.1,
        PlannerAction::Combat(CombatAction::PlayCard { .. }) => 0.0,
    }
}

fn child_score(
    state: &RunState,
    terminal_reason: Option<&str>,
    parent: &RunState,
    action: &PlannerAction,
    parent_actions: usize,
) -> f64 {
    let score = search_score(state, terminal_reason);
    if terminal_reason.is_some() {
        score
    } else {
        score - action_penalty(parent, action) - parent_actions as f64 * 0.05
    }
}

fn has_playable_card_action(state: &RunState) -> bool {
    let Some(combat) = state.combat.as_ref() else {
        return false;
    };
    if state.phase != RunPhase::Combat || combat.phase != CombatPhase::WaitingForPlayer {
        return false;
    }
    legal_combat_actions(combat)
        .expect("planner run state was validated before scoring")
        .into_iter()
        .any(|action| matches!(action, CombatAction::PlayCard { .. }))
}

fn combat_hp(state: &RunState) -> (i32, i32) {
    let Some(combat) = state.combat.as_ref() else {
        return (state.player_hp, 0);
    };
    let monster_hp = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.hp)
        .sum();
    (combat.player.hp, monster_hp)
}

fn node_better(candidate: &SearchNode, best: &SearchNode) -> bool {
    if candidate.first_action.is_some() && best.first_action.is_none() {
        return true;
    }
    if candidate.first_action.is_none() && best.first_action.is_some() {
        return false;
    }
    let candidate_outcome = combat_outcome(candidate.terminal_reason.as_deref());
    let best_outcome = combat_outcome(best.terminal_reason.as_deref());
    if candidate_outcome != best_outcome {
        if best_outcome == CombatOutcome::Ongoing && candidate_outcome != CombatOutcome::Lost {
            return true;
        }
        if candidate_outcome == CombatOutcome::Ongoing && best_outcome != CombatOutcome::Lost {
            return false;
        }
        return candidate_outcome > best_outcome;
    }
    if candidate_outcome == CombatOutcome::Won {
        let (candidate_hp, candidate_max_hp) = player_hp_and_max(&candidate.state);
        let (best_hp, best_max_hp) = player_hp_and_max(&best.state);
        let candidate_scaled_hp = i64::from(candidate_hp) * i64::from(best_max_hp.max(1));
        let best_scaled_hp = i64::from(best_hp) * i64::from(candidate_max_hp.max(1));
        if candidate_scaled_hp != best_scaled_hp {
            return candidate_scaled_hp > best_scaled_hp;
        }
        let candidate_potions = potion_inventory_value(&candidate.state);
        let best_potions = potion_inventory_value(&best.state);
        if candidate_potions != best_potions {
            return candidate_potions > best_potions;
        }
        if candidate.state.gold != best.state.gold {
            return candidate.state.gold > best.state.gold;
        }
        if candidate.actions != best.actions {
            return candidate.actions < best.actions;
        }
    }
    candidate.score > best.score
        || (candidate.score == best.score && candidate.actions < best.actions)
}

fn player_hp_and_max(state: &RunState) -> (i32, i32) {
    state
        .combat
        .as_ref()
        .map(|combat| (combat.player.hp, combat.player.max_hp))
        .unwrap_or((state.player_hp, state.player_max_hp))
}

fn potion_inventory_value(state: &RunState) -> usize {
    state
        .potions
        .iter()
        .map(|potion| match potion.rarity() {
            PotionRarity::Common => 1,
            PotionRarity::Uncommon => 2,
            PotionRarity::Rare => 3,
        })
        .sum()
}

fn node_order(left: &SearchNode, right: &SearchNode) -> Ordering {
    let left_outcome = combat_outcome(left.terminal_reason.as_deref());
    let right_outcome = combat_outcome(right.terminal_reason.as_deref());
    if left_outcome != right_outcome {
        return right_outcome.cmp(&left_outcome);
    }
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.actions.cmp(&right.actions))
}

fn prune_frontier(mut nodes: Vec<SearchNode>, width: usize) -> Vec<SearchNode> {
    nodes.sort_by(node_order);
    if nodes.len() <= width {
        return nodes;
    }

    let mut selected = Vec::new();
    let mut selected_indices = HashSet::new();
    let mut first_action_keys = HashSet::new();

    for (index, node) in nodes.iter().enumerate() {
        let Some(first_action) = node.first_action.as_ref() else {
            continue;
        };
        if first_action_keys.insert(planner_action_label(first_action)) {
            selected.push(node.clone());
            selected_indices.insert(index);
            if selected.len() == width {
                return selected;
            }
        }
    }

    for (index, node) in nodes.into_iter().enumerate() {
        if selected_indices.contains(&index) {
            continue;
        }
        selected.push(node);
        if selected.len() == width {
            break;
        }
    }
    selected
}

fn combat_outcome(terminal_reason: Option<&str>) -> CombatOutcome {
    match terminal_reason {
        Some("won") => CombatOutcome::Won,
        Some("escaped") => CombatOutcome::Escaped,
        Some("lost") => CombatOutcome::Lost,
        _ => CombatOutcome::Ongoing,
    }
}

pub fn planner_action_label(action: &PlannerAction) -> String {
    match action {
        PlannerAction::Combat(CombatAction::PlayCard { card_id, target }) => match target {
            Some(target) => format!("play_card card={} target={}", card_id.get(), target.get()),
            None => format!("play_card card={}", card_id.get()),
        },
        PlannerAction::Combat(CombatAction::EndTurn) => "end_turn".to_owned(),
        PlannerAction::Potion(RunAction::UsePotion { slot, target }) => match target {
            Some(target) => format!("use_potion slot={slot} target={}", target.get()),
            None => format!("use_potion slot={slot}"),
        },
        PlannerAction::Potion(_) => "potion_action".to_owned(),
        PlannerAction::Run(RunAction::ChooseHandSelect { index }) => {
            format!("choose_hand_select index={index}")
        }
        PlannerAction::Run(RunAction::ConfirmHandSelect) => "confirm_hand_select".to_owned(),
        PlannerAction::Run(RunAction::ChooseDrawSelect { index }) => {
            format!("choose_draw_select index={index}")
        }
        PlannerAction::Run(RunAction::ConfirmDrawSelect) => "confirm_draw_select".to_owned(),
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => "confirm_exhaust_select".to_owned(),
        PlannerAction::Run(RunAction::ChooseDiscardSelect { index }) => {
            format!("choose_discard_select index={index}")
        }
        PlannerAction::Run(RunAction::ConfirmDiscardSelect) => "confirm_discard_select".to_owned(),
        PlannerAction::Run(RunAction::ChooseExhaustSelect { index }) => {
            format!("choose_exhaust_select index={index}")
        }
        PlannerAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            format!("choose_combat_card_reward index={index}")
        }
        PlannerAction::Run(_) => "run_action".to_owned(),
    }
}

pub fn planner_action_from_label(label: &str) -> Option<PlannerAction> {
    let mut parts = label.split_whitespace();
    match parts.next()? {
        "end_turn" => Some(PlannerAction::Combat(CombatAction::EndTurn)),
        "play_card" => {
            let card = parts
                .next()?
                .strip_prefix("card=")?
                .parse::<u64>()
                .ok()
                .map(CardId::new)?;
            let target = parts
                .next()
                .and_then(|part| part.strip_prefix("target="))
                .and_then(|target| target.parse::<u64>().ok())
                .map(MonsterId::new);
            Some(PlannerAction::Combat(CombatAction::PlayCard {
                card_id: card,
                target,
            }))
        }
        "choose_discard_select" => parts
            .next()?
            .strip_prefix("index=")?
            .parse::<usize>()
            .ok()
            .map(|index| PlannerAction::Run(RunAction::ChooseDiscardSelect { index })),
        "choose_hand_select" => parts
            .next()?
            .strip_prefix("index=")?
            .parse::<usize>()
            .ok()
            .map(|index| PlannerAction::Run(RunAction::ChooseHandSelect { index })),
        "choose_draw_select" => parts
            .next()?
            .strip_prefix("index=")?
            .parse::<usize>()
            .ok()
            .map(|index| PlannerAction::Run(RunAction::ChooseDrawSelect { index })),
        "choose_exhaust_select" => parts
            .next()?
            .strip_prefix("index=")?
            .parse::<usize>()
            .ok()
            .map(|index| PlannerAction::Run(RunAction::ChooseExhaustSelect { index })),
        "choose_combat_card_reward" => parts
            .next()?
            .strip_prefix("index=")?
            .parse::<usize>()
            .ok()
            .map(|index| PlannerAction::Run(RunAction::ChooseCombatCardReward { index })),
        "use_potion" => {
            let slot = parts.next()?.strip_prefix("slot=")?.parse::<usize>().ok()?;
            let target = parts
                .next()
                .and_then(|part| part.strip_prefix("target="))
                .and_then(|target| target.parse::<u64>().ok())
                .map(MonsterId::new);
            Some(PlannerAction::Potion(RunAction::UsePotion { slot, target }))
        }
        "confirm_exhaust_select" => Some(PlannerAction::Run(RunAction::ConfirmExhaustSelect)),
        "confirm_hand_select" => Some(PlannerAction::Run(RunAction::ConfirmHandSelect)),
        "confirm_draw_select" => Some(PlannerAction::Run(RunAction::ConfirmDrawSelect)),
        "confirm_discard_select" => Some(PlannerAction::Run(RunAction::ConfirmDiscardSelect)),
        _ => None,
    }
}

pub fn planner_action_display_label(run: &RunState, action: &PlannerAction) -> String {
    match action {
        PlannerAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let card_label = run
                .combat
                .as_ref()
                .and_then(|combat| combat.piles.hand.iter().find(|card| card.id == *card_id))
                .map(|card| card_display_name(card.content_id))
                .unwrap_or_else(|| format!("card {}", card_id.get()));
            match target {
                Some(target) => {
                    let target_label = planner_target_display_label(run, *target);
                    format!("Play {card_label} -> {target_label}")
                }
                None => format!("Play {card_label}"),
            }
        }
        PlannerAction::Combat(CombatAction::EndTurn) => "End turn".to_owned(),
        PlannerAction::Potion(RunAction::UsePotion { slot, target }) => match target {
            Some(target) => {
                let target_label = planner_target_display_label(run, *target);
                format!("Use potion slot {slot} -> {target_label}")
            }
            None => format!("Use potion slot {slot}"),
        },
        PlannerAction::Potion(_) => "Use potion".to_owned(),
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => "Confirm selection".to_owned(),
        PlannerAction::Run(RunAction::ChooseHandSelect { index }) => {
            format!("Choose hand card {index}")
        }
        PlannerAction::Run(RunAction::ConfirmHandSelect) => "Confirm hand selection".to_owned(),
        PlannerAction::Run(RunAction::ChooseDrawSelect { index }) => {
            format!("Choose draw card {index}")
        }
        PlannerAction::Run(RunAction::ConfirmDrawSelect) => "Confirm draw selection".to_owned(),
        PlannerAction::Run(RunAction::ChooseDiscardSelect { index }) => {
            format!("Choose discard card {index}")
        }
        PlannerAction::Run(RunAction::ConfirmDiscardSelect) => {
            "Confirm discard selection".to_owned()
        }
        PlannerAction::Run(RunAction::ChooseExhaustSelect { index }) => {
            format!("Choose exhaust card {index}")
        }
        PlannerAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            let card = run
                .combat
                .as_ref()
                .and_then(CombatState::combat_card_reward_choices)
                .and_then(|choices| choices.get(*index))
                .map(|card| card_display_name(card.content_id))
                .unwrap_or_else(|| format!("card reward {index}"));
            format!("Choose combat card {card}")
        }
        PlannerAction::Run(_) => "Run action".to_owned(),
    }
}

fn monster_position(combat: &CombatState, target: MonsterId) -> Option<usize> {
    combat
        .monsters
        .iter()
        .position(|monster| monster.id == target)
}

fn card_display_name(content_id: ContentId) -> String {
    get_card_definition(content_id)
        .map(|definition| definition.name.to_owned())
        .unwrap_or_else(|| format!("card content {}", content_id.get()))
}

fn planner_target_display_label(run: &RunState, target: MonsterId) -> String {
    run.combat
        .as_ref()
        .and_then(|combat| combat.monsters.iter().find(|monster| monster.id == target))
        .and_then(|monster| {
            get_monster_definition(monster.content_id).map(|definition| definition.name.to_owned())
        })
        .or_else(|| {
            run.combat
                .as_ref()
                .and_then(|combat| monster_position(combat, target))
                .map(|index| format!("target {index}"))
        })
        .unwrap_or_else(|| format!("target {}", target.get()))
}

pub fn search_with_warm_start(
    state: &RunState,
    config: &SearchConfig,
    warm_actions: &[PlannerAction],
) -> SimResult<SearchRecommendation> {
    match config.policy {
        SearchPolicy::Greedy => greedy_search(state, config),
        SearchPolicy::Beam => beam_search_with_warm_start(state, config, warm_actions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_core::{
        combat::{ExhaustSelectPurpose, ExhaustSelectState, HandSelectPurpose, HandSelectState},
        content::cards::{
            BASH_ID, BURNING_PACT_ID, DEFEND_R_ID, PURITY_ID, STRIKE_R_ID, THINKING_AHEAD_ID,
            TRUE_GRIT_PLUS_ID,
        },
        CardId, CardInstance, CombatDecisionState,
    };

    fn fixture_config() -> SearchConfig {
        SearchConfig {
            policy: SearchPolicy::Beam,
            depth: 2,
            width: 10,
            allowed_potion_slots: (0..5).collect(),
            transition_budget: 10_000,
            time_budget_ms: 0,
            deduplicate_states: false,
        }
    }

    fn labels(result: &SearchRecommendation) -> Vec<String> {
        result
            .principal_variation
            .iter()
            .map(planner_action_label)
            .collect()
    }

    #[test]
    fn incumbent_beam_result_is_frozen_and_repeatable() {
        let root = RunState::combat_fixture();
        let config = fixture_config();
        let first = beam_search_with_warm_start(&root, &config, &[]).expect("search succeeds");
        let second = beam_search_with_warm_start(&root, &config, &[]).expect("search repeats");

        assert_eq!(first, second);
        assert_eq!(
            labels(&first),
            vec!["end_turn".to_owned(), "play_card card=2".to_owned()]
        );
        assert_eq!(
            first,
            SearchRecommendation {
                principal_variation: first.principal_variation.clone(),
                value: 271.09999999999997,
                nodes: 146,
                terminal_reason: None,
                final_hp: 74,
                monster_hp: 40,
                budget_exhausted: false,
                timed_out: false,
                expanded: 49,
                generated: 145,
                terminal_nodes: 0,
                pruned: 78,
                max_frontier: 10,
                duplicate_checks: 0,
                duplicates: 0,
                cache_hits: 0,
            }
        );

        let mut final_state = root.clone();
        for action in &first.principal_variation {
            final_state = apply_planner_action(&final_state, action).expect("PV action is legal");
        }
        let combat = final_state.combat.as_ref().expect("combat remains active");
        assert_eq!((combat.player.hp, combat.player.max_hp), (74, 80));
        assert_eq!(final_state.gold, 99);
        assert!(final_state.potions.is_empty());
        assert_eq!(combat_hp(&final_state), (74, 40));
        assert_eq!(final_state.phase, RunPhase::Combat);
    }

    #[test]
    fn finite_transition_budget_accounting_is_frozen() {
        let root = RunState::combat_fixture();
        let mut config = fixture_config();
        config.transition_budget = 7;
        let result = beam_search_with_warm_start(&root, &config, &[]).expect("search succeeds");

        assert_eq!(
            labels(&result),
            vec![
                "play_card card=2".to_owned(),
                "play_card card=3 target=1".to_owned(),
            ]
        );
        assert_eq!(result.value, 269.59999999999997);
        assert_eq!((result.nodes, result.expanded, result.generated), (8, 3, 7));
        assert_eq!(
            (result.terminal_nodes, result.pruned, result.max_frontier),
            (0, 0, 4)
        );
        assert!(result.budget_exhausted);
        assert!(!result.timed_out);
        assert_eq!((result.final_hp, result.monster_hp), (80, 32));
    }

    #[test]
    fn valid_warm_suffix_keeps_incumbent_result_and_counts_cache_hit() {
        let root = RunState::combat_fixture();
        let config = fixture_config();
        let warm = [PlannerAction::Combat(CombatAction::EndTurn)];
        let result = beam_search_with_warm_start(&root, &config, &warm).expect("search succeeds");

        assert_eq!(
            labels(&result),
            vec!["end_turn".to_owned(), "play_card card=2".to_owned()]
        );
        assert_eq!(result.value, 271.09999999999997);
        assert_eq!(
            (result.nodes, result.expanded, result.generated),
            (146, 49, 145)
        );
        assert_eq!(result.cache_hits, 1);
        assert!(
            result.expanded > 0,
            "warm suffix must not replace fresh search"
        );
    }

    #[test]
    fn constrained_budget_warm_suffix_changes_recommendation_and_still_searches() {
        let root = RunState::combat_fixture();
        let mut config = fixture_config();
        config.transition_budget = 7;
        let warm = [
            PlannerAction::Combat(CombatAction::EndTurn),
            PlannerAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(2),
                target: None,
            }),
        ];

        let cold = beam_search_with_warm_start(&root, &config, &[]).expect("cold search succeeds");
        let warmed =
            beam_search_with_warm_start(&root, &config, &warm).expect("warm search succeeds");

        assert_eq!(
            labels(&cold),
            vec![
                "play_card card=2".to_owned(),
                "play_card card=3 target=1".to_owned(),
            ]
        );
        assert_eq!(cold.value, 269.59999999999997);
        assert_eq!(
            labels(&warmed),
            vec!["end_turn".to_owned(), "play_card card=2".to_owned()]
        );
        assert_eq!(warmed.value, 271.09999999999997);
        assert_eq!((warmed.nodes, warmed.expanded, warmed.generated), (8, 3, 7));
        assert_eq!((warmed.final_hp, warmed.monster_hp), (74, 40));
        assert!(warmed.budget_exhausted);
        assert!(!warmed.timed_out);
        assert_eq!(warmed.cache_hits, 2);
        assert!(
            warmed.expanded > 0,
            "warm suffix must seed the incumbent without replacing fresh expansion"
        );
    }

    #[test]
    fn invalid_warm_suffix_stops_at_the_legal_incumbent_and_stale_prefix_is_ignored() {
        let root = RunState::combat_fixture();
        let config = fixture_config();
        let valid = PlannerAction::Combat(CombatAction::EndTurn);
        let stale = PlannerAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: Some(MonsterId::new(1)),
        });

        let valid_result =
            beam_search_with_warm_start(&root, &config, std::slice::from_ref(&valid))
                .expect("valid warm search succeeds");
        let suffix_result = beam_search_with_warm_start(&root, &config, &[valid, stale.clone()])
            .expect("invalid suffix stops at its legal prefix");
        assert_eq!(suffix_result, valid_result);
        assert_eq!(suffix_result.cache_hits, 1);

        let cold_result =
            beam_search_with_warm_start(&root, &config, &[]).expect("cold search succeeds");
        let stale_result = beam_search_with_warm_start(&root, &config, &[stale])
            .expect("fully stale warm start is ignored");
        assert_eq!(stale_result, cold_result);
        assert_eq!(stale_result.cache_hits, 0);
    }

    fn node(
        state: RunState,
        first_action: PlannerAction,
        actions: usize,
        score: f64,
        terminal_reason: Option<&str>,
    ) -> SearchNode {
        SearchNode {
            state,
            first_action: Some(first_action.clone()),
            principal_variation: vec![first_action],
            actions,
            score,
            terminal_reason: terminal_reason.map(str::to_owned),
        }
    }

    fn won_node(
        hp: i32,
        max_hp: i32,
        potions: Vec<Potion>,
        gold: i32,
        actions: usize,
    ) -> SearchNode {
        let mut state = RunState::combat_fixture();
        let combat = state.combat.as_mut().expect("combat");
        combat.phase = CombatPhase::Won;
        combat.player.hp = hp;
        combat.player.max_hp = max_hp;
        state.player_hp = hp;
        state.player_max_hp = max_hp;
        state.potions = potions;
        state.gold = gold;
        node(
            state,
            PlannerAction::Combat(CombatAction::EndTurn),
            actions,
            1_000_000.0,
            Some("won"),
        )
    }

    #[test]
    fn benchmark_result_wire_fields_and_deterministic_values_are_frozen() {
        let root = RunState::combat_fixture();
        let config = fixture_config();
        let expected = serde_json::json!({
            "outcome": "nonterminal",
            "terminal_reason": null,
            "final_hp": 80,
            "final_max_hp": 80,
            "final_gold": 99,
            "remaining_potions": 0,
            "remaining_potion_value": 0,
            "remaining_monster_hp": 32,
            "transitions": 7,
            "budget_exhausted": true,
            "timed_out": false,
            "expanded": 3,
            "generated": 7,
            "terminal_nodes": 0,
            "pruned": 0,
            "max_frontier": 4,
            "duplicate_checks": 0,
            "duplicates": 0,
            "cache_hits": 0,
            "actions": 2,
            "action_labels": ["Play Defend", "Play Bash -> Fixed Simple Monster"],
            "replay_error": null,
        });

        for _ in 0..2 {
            let mut actual = serde_json::to_value(benchmark_beam_search(&root, &config, 7))
                .expect("benchmark serializes");
            assert!(
                actual
                    .as_object_mut()
                    .expect("benchmark is an object")
                    .remove("elapsed_ms")
                    .is_some(),
                "elapsed_ms remains present but is excluded from deterministic equality"
            );
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn comparator_and_frontier_tie_break_chain_is_frozen() {
        assert_eq!(
            [
                CombatOutcome::Won,
                CombatOutcome::Lost,
                CombatOutcome::Escaped,
                CombatOutcome::Ongoing,
            ]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
            vec![
                CombatOutcome::Lost,
                CombatOutcome::Ongoing,
                CombatOutcome::Escaped,
                CombatOutcome::Won,
            ]
        );

        let root = RunState::combat_fixture();
        let lost = node(
            root.clone(),
            PlannerAction::Combat(CombatAction::EndTurn),
            1,
            10_000.0,
            Some("lost"),
        );
        let ongoing = node(
            root.clone(),
            PlannerAction::Combat(CombatAction::EndTurn),
            2,
            -10_000.0,
            None,
        );
        assert!(node_better(&ongoing, &lost));

        let won = won_node(1, 80, Vec::new(), 0, 9);
        assert!(node_better(&won, &ongoing));
        let escaped = node(
            root.clone(),
            PlannerAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(3),
                target: None,
            }),
            1,
            -20_000.0,
            Some("escaped"),
        );
        assert_eq!(node_order(&won, &escaped), Ordering::Less);
        assert_eq!(node_order(&escaped, &ongoing), Ordering::Less);
        assert_eq!(node_order(&ongoing, &lost), Ordering::Less);
        let outcomes = prune_frontier(
            vec![lost.clone(), ongoing.clone(), escaped.clone(), won.clone()],
            3,
        );
        assert_eq!(
            outcomes
                .iter()
                .map(|node| node.terminal_reason.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("won"), Some("escaped"), None]
        );

        let better_fraction = won_node(40, 80, Vec::new(), 0, 4);
        let lower_fraction_with_rare = won_node(39, 80, vec![Potion::Fairy], 999, 1);
        assert!(node_better(&better_fraction, &lower_fraction_with_rare));
        let uncommon = won_node(40, 80, vec![Potion::LiquidMemories], 0, 4);
        let common = won_node(40, 80, vec![Potion::Block], 999, 1);
        assert!(node_better(&uncommon, &common));
        let more_gold = won_node(40, 80, vec![Potion::Block], 10, 4);
        let less_gold = won_node(40, 80, vec![Potion::Block], 9, 1);
        assert!(node_better(&more_gold, &less_gold));
        let fewer_actions = won_node(40, 80, vec![Potion::Block], 10, 2);
        assert!(node_better(&fewer_actions, &more_gold));

        let action_a = PlannerAction::Combat(CombatAction::EndTurn);
        let action_b = PlannerAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(2),
            target: None,
        });
        let tied_longer = node(root.clone(), action_a.clone(), 2, 30.0, None);
        let tied_shorter = node(root.clone(), action_a.clone(), 1, 30.0, None);
        assert!(node_better(&tied_shorter, &tied_longer));
        assert_eq!(node_order(&tied_shorter, &tied_longer), Ordering::Less);

        let a_second = node(root.clone(), action_a.clone(), 1, 20.0, None);
        let b_lower = node(root.clone(), action_b.clone(), 1, 10.0, None);
        let selected = prune_frontier(vec![a_second, b_lower, tied_shorter], 2);
        assert_eq!(
            selected
                .iter()
                .map(|node| planner_action_label(node.first_action.as_ref().unwrap()))
                .collect::<Vec<_>>(),
            vec!["end_turn", "play_card card=2"]
        );

        let mut a_tied_first = node(root.clone(), action_a.clone(), 1, 30.0, None);
        a_tied_first.state.gold = 1;
        let mut a_tied_second = node(root.clone(), action_a, 1, 30.0, None);
        a_tied_second.state.gold = 2;
        let b_tied = node(root, action_b, 1, 30.0, None);
        let stable_diverse_ties = prune_frontier(vec![a_tied_first, a_tied_second, b_tied], 2);
        assert_eq!(
            stable_diverse_ties
                .iter()
                .map(|node| planner_action_label(node.first_action.as_ref().unwrap()))
                .collect::<Vec<_>>(),
            vec!["end_turn", "play_card card=2"]
        );
        assert_eq!(stable_diverse_ties[0].state.gold, 1);
    }

    #[test]
    fn terminal_value_uses_hp_max_hp_gold_and_remaining_potions() {
        let mut run = RunState::combat_fixture();
        run.player_hp = 40;
        run.player_max_hp = 82;
        run.gold = 115;
        run.potions = vec![Potion::Fire, Potion::SmokeBomb];
        let player = &mut run.combat.as_mut().expect("combat").player;
        player.hp = 37;
        player.max_hp = 82;

        assert_eq!(terminal_state_value(&run), 37.0 + 82.0 * 3.0 + 11.5 + 16.0);
    }

    #[test]
    fn ending_turn_is_penalized_when_cards_are_playable() {
        let run = RunState::combat_fixture();

        let end_turn_penalty = action_penalty(&run, &PlannerAction::Combat(CombatAction::EndTurn));
        let play_penalty = action_penalty(
            &run,
            &PlannerAction::Combat(CombatAction::PlayCard {
                card_id: run
                    .combat
                    .as_ref()
                    .expect("combat")
                    .piles
                    .hand
                    .first()
                    .expect("hand card")
                    .id,
                target: None,
            }),
        );

        assert_eq!(end_turn_penalty, 12.0);
        assert_eq!(play_penalty, 0.0);
    }

    #[test]
    fn expired_deadline_is_a_structured_timeout() {
        let root = RunState::combat_fixture();
        let result = beam_search_with_node_limit(
            &root,
            &fixture_config(),
            10_000,
            &[],
            Some(Instant::now() - Duration::from_millis(1)),
        )
        .expect("search succeeds");
        assert!(result.budget_exhausted);
        assert!(result.timed_out);
        assert_eq!(result.nodes, 1);
    }

    #[test]
    fn state_cache_keeps_better_path_for_identical_state() {
        let state = RunState::combat_fixture();
        let action = PlannerAction::Combat(CombatAction::EndTurn);
        let worse = node(state.clone(), action.clone(), 2, 1.0, None);
        let better = node(state.clone(), action, 1, 2.0, None);
        let (deduplicated, checks, duplicates) = deduplicate_search_nodes(vec![worse, better]);
        assert_eq!((checks, duplicates, deduplicated.len()), (2, 1, 1));
        assert_eq!(deduplicated[0].state, state);
        assert_eq!((deduplicated[0].score, deduplicated[0].actions), (2.0, 1));
    }

    #[test]
    fn planner_filters_unsupported_potions() {
        for potion in [
            Potion::SmokeBomb,
            Potion::Attack,
            Potion::Skill,
            Potion::Power,
            Potion::Colorless,
        ] {
            let mut run = RunState::combat_fixture();
            run.potions = vec![potion, Potion::Energy];
            run.empty_potion_slots = vec![2];
            let actions = planner_actions(&run, &fixture_config()).expect("valid actions");
            assert!(!actions.iter().any(|action| {
                matches!(
                    action,
                    PlannerAction::Potion(RunAction::UsePotion { slot: 0, .. })
                )
            }));
            assert!(actions.iter().any(|action| {
                matches!(
                    action,
                    PlannerAction::Potion(RunAction::UsePotion {
                        slot: 1,
                        target: None
                    })
                )
            }));
        }
    }

    fn play_card(run: &RunState, card_id: u64) -> RunState {
        apply_planner_action(
            run,
            &PlannerAction::Combat(CombatAction::PlayCard {
                card_id: CardId::new(card_id),
                target: None,
            }),
        )
        .expect("card plays")
    }

    fn combat_with_hand(cards: Vec<CardInstance>) -> RunState {
        let mut run = RunState::combat_fixture();
        {
            let combat = run.combat.as_mut().expect("combat fixture");
            combat.player.energy = 3;
            combat.piles.hand = cards;
            combat.piles.draw_pile.clear();
            combat.piles.discard_pile.clear();
            combat.piles.exhaust_pile.clear();
        }
        run
    }

    fn teacher_config_actions(run: &RunState) -> Vec<PlannerAction> {
        planner_actions(run, &fixture_config()).expect("planner actions")
    }

    fn has_choose_exhaust(actions: &[PlannerAction]) -> bool {
        actions.iter().any(|action| {
            matches!(
                action,
                PlannerAction::Run(RunAction::ChooseExhaustSelect { .. })
            )
        })
    }

    fn has_confirm_exhaust(actions: &[PlannerAction]) -> bool {
        actions
            .iter()
            .any(|action| matches!(action, PlannerAction::Run(RunAction::ConfirmExhaustSelect)))
    }

    fn has_choose_hand(actions: &[PlannerAction]) -> bool {
        actions.iter().any(|action| {
            matches!(
                action,
                PlannerAction::Run(RunAction::ChooseHandSelect { .. })
            )
        })
    }

    fn has_confirm_hand(actions: &[PlannerAction]) -> bool {
        actions
            .iter()
            .any(|action| matches!(action, PlannerAction::Run(RunAction::ConfirmHandSelect)))
    }

    #[test]
    fn empty_exact_one_exhaust_teacher_chooses_instead_of_confirming() {
        for source in [TRUE_GRIT_PLUS_ID, BURNING_PACT_ID] {
            let opened = play_card(
                &combat_with_hand(vec![
                    CardInstance::new(CardId::new(1), source),
                    CardInstance::new(CardId::new(2), STRIKE_R_ID),
                    CardInstance::new(CardId::new(3), DEFEND_R_ID),
                    CardInstance::new(CardId::new(4), BASH_ID),
                ]),
                1,
            );
            let actions = teacher_config_actions(&opened);
            assert!(
                has_choose_exhaust(&actions),
                "{source:?} empty select must offer choose"
            );
            assert!(
                !has_confirm_exhaust(&actions),
                "{source:?} empty confirm must not enter search"
            );

            let teacher = beam_teacher_decision(&opened, 2, 8, 2_000, false)
                .expect("empty exact-one exhaust teacher succeeds");
            assert!(
                matches!(
                    teacher.action,
                    RunDecisionAction::Run(RunAction::ChooseExhaustSelect { .. })
                ),
                "teacher first action for {source:?} was {:?}",
                teacher.action
            );
        }
    }

    #[test]
    fn completed_exact_one_exhaust_search_drops_retargets_but_player_legal_keeps_them() {
        let opened = play_card(
            &combat_with_hand(vec![
                CardInstance::new(CardId::new(1), TRUE_GRIT_PLUS_ID),
                CardInstance::new(CardId::new(2), STRIKE_R_ID),
                CardInstance::new(CardId::new(3), DEFEND_R_ID),
                CardInstance::new(CardId::new(4), BASH_ID),
            ]),
            1,
        );
        let selected = apply_planner_action(
            &opened,
            &PlannerAction::Run(RunAction::ChooseExhaustSelect { index: 0 }),
        )
        .expect("True Grit choice applies");

        let legal = legal_run_decision_actions(&selected).expect("player legal actions");
        assert!(legal.contains(&RunDecisionAction::Run(RunAction::ConfirmExhaustSelect)));
        assert!(legal.iter().any(|action| {
            matches!(
                action,
                RunDecisionAction::Run(RunAction::ChooseExhaustSelect { .. })
            )
        }));

        let planner = teacher_config_actions(&selected);
        assert!(has_confirm_exhaust(&planner));
        assert!(!has_choose_exhaust(&planner));
    }

    #[test]
    fn thinking_ahead_teacher_confirms_after_the_first_choice() {
        let mut run = combat_with_hand(vec![
            CardInstance::new(CardId::new(1), THINKING_AHEAD_ID),
            CardInstance::new(CardId::new(2), STRIKE_R_ID),
            CardInstance::new(CardId::new(3), DEFEND_R_ID),
            CardInstance::new(CardId::new(4), BASH_ID),
            CardInstance::new(CardId::new(5), STRIKE_R_ID),
        ]);
        {
            let combat = run.combat.as_mut().expect("combat fixture");
            combat.piles.draw_pile = vec![
                CardInstance::new(CardId::new(6), STRIKE_R_ID),
                CardInstance::new(CardId::new(7), DEFEND_R_ID),
            ];
        }
        let opened = play_card(&run, 1);
        let first = beam_teacher_decision(&opened, 2, 8, 2_000, false)
            .expect("empty Thinking Ahead teacher succeeds");
        assert!(matches!(
            first.action,
            RunDecisionAction::Run(RunAction::ChooseHandSelect { .. })
        ));

        let mut state =
            apply_run_decision_action(&opened, first.action).expect("first choose applies");
        let mut choose_indices = Vec::new();
        if let RunDecisionAction::Run(RunAction::ChooseHandSelect { index }) = first.action {
            choose_indices.push(index);
        }
        for _ in 0..3 {
            let teacher = beam_teacher_decision(&state, 2, 8, 2_000, false)
                .expect("Thinking Ahead replan succeeds");
            match teacher.action {
                RunDecisionAction::Run(RunAction::ConfirmHandSelect) => {
                    let planner = teacher_config_actions(&state);
                    assert!(has_confirm_hand(&planner));
                    assert!(
                        !has_choose_hand(&planner),
                        "completed exact-one hand select must not retarget in search"
                    );
                    return;
                }
                RunDecisionAction::Run(RunAction::ChooseHandSelect { index }) => {
                    choose_indices.push(index);
                    state = apply_run_decision_action(&state, teacher.action)
                        .expect("retarget applies");
                }
                other => panic!("unexpected Thinking Ahead teacher action {other:?}"),
            }
        }
        panic!("Thinking Ahead teacher retargeted without confirming: {choose_indices:?}");
    }

    #[test]
    fn optional_exhaust_and_forethought_any_keep_choose_when_confirm_is_legal() {
        let mut gambling = RunState::combat_fixture();
        gambling.potions = vec![Potion::Energy];
        gambling.empty_potion_slots = vec![1, 2];
        sts_core::combat::open_gambling_chip_select(
            gambling.combat.as_mut().expect("combat fixture"),
        )
        .expect("Gambling Chip opens");
        let gambling_actions = teacher_config_actions(&gambling);
        assert!(has_confirm_exhaust(&gambling_actions));
        assert!(has_choose_exhaust(&gambling_actions));
        assert!(gambling_actions.iter().any(|action| {
            matches!(
                action,
                PlannerAction::Potion(RunAction::UsePotion {
                    slot: 0,
                    target: None
                })
            )
        }));

        let mut forethought = RunState::combat_fixture();
        {
            let combat = forethought.combat.as_mut().expect("combat fixture");
            let source_card_id = combat.piles.hand[0].id;
            combat.decision = Some(CombatDecisionState::HandSelect {
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
        let forethought_actions = teacher_config_actions(&forethought);
        assert!(has_confirm_hand(&forethought_actions));
        assert!(has_choose_hand(&forethought_actions));

        let mut elixir = RunState::combat_fixture();
        elixir.combat.as_mut().expect("combat fixture").decision =
            Some(CombatDecisionState::ExhaustSelect {
                state: ExhaustSelectState {
                    purpose: ExhaustSelectPurpose::Exhaust,
                    source_card_id: None,
                    source_card: None,
                    source_card_force_exhaust: false,
                    selected_hand_indices: Vec::new(),
                    interrupted_by_cultist_potion: false,
                    pending_actions: Default::default(),
                },
            });
        let elixir_actions = teacher_config_actions(&elixir);
        assert!(has_confirm_exhaust(&elixir_actions));
        assert!(has_choose_exhaust(&elixir_actions));

        let mut purity = RunState::combat_fixture();
        {
            let combat = purity.combat.as_mut().expect("combat fixture");
            let source_card_id = CardId::new(100);
            combat.piles.hand = vec![
                CardInstance::new(source_card_id, PURITY_ID),
                CardInstance::new(CardId::new(101), STRIKE_R_ID),
                CardInstance::new(CardId::new(102), STRIKE_R_ID),
                CardInstance::new(CardId::new(103), STRIKE_R_ID),
                CardInstance::new(CardId::new(104), STRIKE_R_ID),
            ];
            combat.decision = Some(CombatDecisionState::ExhaustSelect {
                state: ExhaustSelectState {
                    purpose: ExhaustSelectPurpose::PurityExhaustUpTo3,
                    source_card_id: Some(source_card_id),
                    source_card: None,
                    source_card_force_exhaust: false,
                    selected_hand_indices: vec![1, 2, 3],
                    interrupted_by_cultist_potion: false,
                    pending_actions: Default::default(),
                },
            });
        }
        let purity_actions = teacher_config_actions(&purity);
        assert!(has_confirm_exhaust(&purity_actions));
        assert_eq!(
            purity_actions
                .iter()
                .filter_map(|action| match action {
                    PlannerAction::Run(RunAction::ChooseExhaustSelect { index }) => Some(*index),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            Vec::<usize>::new(),
            "Purity at its cap drops additions after selected cards leave the visible grid"
        );
    }
}
