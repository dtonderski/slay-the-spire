use crate::model::{
    ActionId, AutomationConfig, AutomationPlanSnapshot, AutomationPlannedAction, AutomationPolicy,
    BlockedState, LegalAction, LegalActionKind, LivePhase, LiveState,
};
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use sts_core::{
    apply_run_decision_action,
    card::CardType,
    content::{
        cards::{get_card_definition, HAVOC_ID, HAVOC_PLUS_ID},
        monsters::get_monster_definition,
    },
    legal_combat_actions, legal_run_decision_actions,
    potion::PotionRarity,
    CardId, CombatAction, CombatPhase, CombatState, ContentId, MonsterId, MonsterIntent, Potion,
    RunAction, RunDecisionAction, RunPhase, RunState, SimError, SimResult,
};

#[derive(Clone)]
enum PlannerAction {
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

struct SearchRecommendation {
    principal_variation: Vec<PlannerAction>,
    value: f64,
    nodes: usize,
    terminal_reason: Option<String>,
    final_hp: i32,
    monster_hp: i32,
    budget_exhausted: bool,
    timed_out: bool,
    expanded: usize,
    generated: usize,
    terminal_nodes: usize,
    pruned: usize,
    max_frontier: usize,
    duplicate_checks: usize,
    duplicates: usize,
    cache_hits: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BenchmarkSearchResult {
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

pub(super) fn plan_action_with_warm_start(
    config: &AutomationConfig,
    state: &LiveState,
    warm_steps: &[AutomationPlannedAction],
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    match config.policy {
        AutomationPolicy::FakePlayFirstCard => plan_single_card_play(state),
        AutomationPolicy::GreedySearch | AutomationPolicy::BeamSearch => {
            plan_search_action(config, state, warm_steps)
        }
    }
}

pub(super) fn bind_plan_step_to_live_action(
    state: &LiveState,
    step: &AutomationPlannedAction,
) -> Option<AutomationPlannedAction> {
    let run = observed_run_state(state).ok()?;
    let action = planner_action_from_label(&step.planner_action)?;
    let expected_command = expected_command(state, &run, &action)?;
    let live = match_live_action(state, &expected_command).ok()?;
    Some(planned_live_action(
        state.sequence,
        live,
        Some(&expected_command),
        step.planner_action.clone(),
    ))
}

fn plan_single_card_play(
    state: &LiveState,
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    if state.phase != LivePhase::Combat {
        return Err(blocked(
            "automation_not_combat",
            "automation can only plan combat actions",
        ));
    }

    let candidates = state
        .legal_actions
        .iter()
        .filter(|action| action.enabled && action.kind == LegalActionKind::PlayCard)
        .collect::<Vec<_>>();

    let action = match candidates.as_slice() {
        [action] => *action,
        [] => {
            return Err(blocked(
                "automation_no_matching_action",
                "fake planner found no enabled card play",
            ))
        }
        _ => {
            return Err(blocked(
                "automation_ambiguous_action",
                "fake planner found more than one enabled card play",
            ))
        }
    };
    let planned = planned_live_action(
        state.sequence,
        action,
        action.command.get("command").and_then(Value::as_str),
        "fake_play_first_card".to_owned(),
    );
    let snapshot = AutomationPlanSnapshot {
        actions: vec![planned.clone()],
        played_actions: 0,
        predicted_final_hp: None,
        predicted_monster_hp: None,
        value: None,
        nodes: 1,
        terminal_reason: None,
        search_elapsed_ms: 0,
        budget_exhausted: false,
        timed_out: false,
        duplicate_checks: 0,
        duplicates: 0,
        cache_hits: 0,
    };
    Ok((planned, snapshot))
}

fn plan_search_action(
    config: &AutomationConfig,
    state: &LiveState,
    warm_steps: &[AutomationPlannedAction],
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    if state.phase != LivePhase::Combat {
        return Err(blocked(
            "automation_not_combat",
            "automation can only plan combat actions",
        ));
    }
    let run = observed_run_state(state)?;
    if run.phase != RunPhase::Combat || run.combat.is_none() {
        return Err(blocked(
            "automation_not_combat",
            "latest observed simulator state is not in combat",
        ));
    }

    if let Some((live, planner_action)) = live_selection_confirm(&run, state) {
        let planned = planned_live_action(
            state.sequence,
            live,
            Some("CONFIRM"),
            planner_action.to_owned(),
        );
        return Ok((
            planned.clone(),
            AutomationPlanSnapshot {
                actions: vec![planned],
                played_actions: 0,
                predicted_final_hp: None,
                predicted_monster_hp: None,
                value: None,
                nodes: 1,
                terminal_reason: None,
                search_elapsed_ms: 0,
                budget_exhausted: false,
                timed_out: false,
                duplicate_checks: 0,
                duplicates: 0,
                cache_hits: 0,
            },
        ));
    }

    let search_started = Instant::now();
    let recommendation = match config.policy {
        AutomationPolicy::GreedySearch => greedy_search(&run, config),
        AutomationPolicy::BeamSearch => beam_search_with_warm_start(&run, config, warm_steps),
        AutomationPolicy::FakePlayFirstCard => unreachable!("handled by caller"),
    }
    .map_err(planner_simulator_blocked)?;
    if recommendation.principal_variation.is_empty() {
        return Err(blocked(
            "automation_no_plan",
            "combat planner found no legal current combat action",
        ));
    }

    let first = &recommendation.principal_variation[0];
    let expected_command = expected_command(state, &run, first).ok_or_else(|| {
        blocked(
            "automation_unsupported_action",
            "planner selected an action that cannot be mapped to a live command",
        )
    })?;
    let live = match_live_action(state, &expected_command)?;
    let planned = planned_live_action(
        state.sequence,
        live,
        Some(&expected_command),
        planner_action_label(first),
    );

    let mut planned_actions = vec![planned.clone()];
    let mut future_run = apply_planner_action(&run, first).map_err(planner_simulator_blocked)?;
    for action in recommendation.principal_variation.iter().skip(1) {
        planned_actions.push(planned_future_action(state.sequence, &future_run, action));
        future_run =
            apply_planner_action(&future_run, action).map_err(planner_simulator_blocked)?;
    }

    let snapshot = AutomationPlanSnapshot {
        actions: planned_actions,
        played_actions: 0,
        predicted_final_hp: Some(recommendation.final_hp),
        predicted_monster_hp: Some(recommendation.monster_hp),
        value: Some(recommendation.value),
        nodes: recommendation.nodes,
        terminal_reason: recommendation.terminal_reason,
        search_elapsed_ms: u64::try_from(search_started.elapsed().as_millis()).unwrap_or(u64::MAX),
        budget_exhausted: recommendation.budget_exhausted,
        timed_out: recommendation.timed_out,
        duplicate_checks: recommendation.duplicate_checks,
        duplicates: recommendation.duplicates,
        cache_hits: recommendation.cache_hits,
    };
    Ok((planned, snapshot))
}

fn live_selection_confirm<'a>(
    run: &RunState,
    state: &'a LiveState,
) -> Option<(&'a LegalAction, &'static str)> {
    let combat = run.combat.as_ref()?;
    let planner_action = if combat.hand_select().is_some() {
        "confirm_hand_select"
    } else if combat.draw_select().is_some() {
        "confirm_draw_select"
    } else if combat.discard_select().is_some() {
        "confirm_discard_select"
    } else if combat.exhaust_select().is_some() {
        "confirm_exhaust_select"
    } else {
        return None;
    };
    let mut confirms = state.legal_actions.iter().filter(|action| {
        action.enabled
            && action.kind == LegalActionKind::Confirm
            && action
                .command
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.eq_ignore_ascii_case("CONFIRM"))
    });
    let confirm = confirms.next()?;
    confirms
        .next()
        .is_none()
        .then_some((confirm, planner_action))
}

fn observed_run_state(state: &LiveState) -> Result<RunState, BlockedState> {
    if let Some(value) = state.raw.get("sim_run_state") {
        if let Ok(run) = serde_json::from_value(value.clone()) {
            return Ok(run);
        }
    }
    Err(blocked(
        "automation_missing_sim_state",
        "automation requires simulator-tracked run state; hydrating from live observations is forbidden",
    ))
}

fn greedy_search(state: &RunState, config: &AutomationConfig) -> SimResult<SearchRecommendation> {
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

#[cfg(test)]
fn beam_search(state: &RunState, config: &AutomationConfig) -> SimResult<SearchRecommendation> {
    beam_search_with_node_limit(state, config, usize::MAX, &[], None)
}

fn beam_search_with_warm_start(
    state: &RunState,
    config: &AutomationConfig,
    warm_steps: &[AutomationPlannedAction],
) -> SimResult<SearchRecommendation> {
    let warm_actions = warm_steps
        .iter()
        .map(|step| planner_action_from_label(&step.planner_action))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    beam_search_with_node_limit(
        state,
        config,
        config.search_transition_budget.saturating_add(1),
        &warm_actions,
        (config.search_time_budget_ms > 0)
            .then(|| Instant::now() + Duration::from_millis(config.search_time_budget_ms)),
    )
}

fn beam_search_with_node_limit(
    state: &RunState,
    config: &AutomationConfig,
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
    config: &AutomationConfig,
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
        if config.deduplicate_search_states {
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
    config: &AutomationConfig,
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
    config: &AutomationConfig,
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
        if config.deduplicate_search_states {
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
    if config.deduplicate_search_states {
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

pub(crate) fn benchmark_beam_search(
    state: &RunState,
    config: &AutomationConfig,
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

fn planner_actions(state: &RunState, config: &AutomationConfig) -> SimResult<Vec<PlannerAction>> {
    if state.phase != RunPhase::Combat {
        return Ok(Vec::new());
    }

    Ok(legal_run_decision_actions(state)?
        .into_iter()
        .filter_map(|action| match action {
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

fn planner_allows_potion(state: &RunState, config: &AutomationConfig, slot: usize) -> bool {
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

fn apply_planner_action(state: &RunState, action: &PlannerAction) -> sts_core::SimResult<RunState> {
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

fn expected_command(state: &LiveState, run: &RunState, action: &PlannerAction) -> Option<String> {
    match action {
        PlannerAction::Combat(CombatAction::EndTurn) => Some("END".to_owned()),
        PlannerAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let combat = run.combat.as_ref()?;
            let hand_position = combat
                .piles
                .hand
                .iter()
                .position(|card| card.id == *card_id)?;
            let hand_slot = live_hand_slot(state, hand_position).unwrap_or(hand_position);
            if combat.piles.hand[hand_position].content_id == HAVOC_ID
                || combat.piles.hand[hand_position].content_id == HAVOC_PLUS_ID
            {
                return Some(format!("PLAY {hand_slot}"));
            }
            match target {
                Some(target) => {
                    let target_slot = live_monster_slot(state, combat, *target)?;
                    Some(format!("PLAY {hand_slot} {target_slot}"))
                }
                None => Some(format!("PLAY {hand_slot}")),
            }
        }
        PlannerAction::Potion(RunAction::UsePotion { slot, target }) => match target {
            Some(target) => {
                let combat = run.combat.as_ref()?;
                let target_slot = live_monster_slot(state, combat, *target)?;
                Some(format!("POTION USE {slot} {target_slot}"))
            }
            None => Some(format!("POTION USE {slot}")),
        },
        PlannerAction::Potion(_) => None,
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(RunAction::ChooseHandSelect { index })
        | PlannerAction::Run(RunAction::ChooseDrawSelect { index })
        | PlannerAction::Run(RunAction::ChooseDiscardSelect { index })
        | PlannerAction::Run(RunAction::ChooseExhaustSelect { index })
        | PlannerAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            Some(format!("CHOOSE {index}"))
        }
        PlannerAction::Run(RunAction::ConfirmHandSelect)
        | PlannerAction::Run(RunAction::ConfirmDrawSelect)
        | PlannerAction::Run(RunAction::ConfirmDiscardSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(_) => None,
    }
}

fn expected_sim_command(run: &RunState, action: &PlannerAction) -> Option<String> {
    match action {
        PlannerAction::Combat(CombatAction::EndTurn) => Some("END".to_owned()),
        PlannerAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let combat = run.combat.as_ref()?;
            let hand_slot = combat
                .piles
                .hand
                .iter()
                .position(|card| card.id == *card_id)?;
            match target {
                Some(target) => {
                    let target_slot = monster_position(combat, *target)?;
                    Some(format!("PLAY {hand_slot} {target_slot}"))
                }
                None => Some(format!("PLAY {hand_slot}")),
            }
        }
        PlannerAction::Potion(RunAction::UsePotion { slot, target }) => match target {
            Some(target) => {
                let combat = run.combat.as_ref()?;
                let target_slot = monster_position(combat, *target)?;
                Some(format!("POTION USE {slot} {target_slot}"))
            }
            None => Some(format!("POTION USE {slot}")),
        },
        PlannerAction::Potion(_) => None,
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(RunAction::ChooseHandSelect { index })
        | PlannerAction::Run(RunAction::ChooseDrawSelect { index })
        | PlannerAction::Run(RunAction::ChooseDiscardSelect { index })
        | PlannerAction::Run(RunAction::ChooseExhaustSelect { index })
        | PlannerAction::Run(RunAction::ChooseCombatCardReward { index }) => {
            Some(format!("CHOOSE {index}"))
        }
        PlannerAction::Run(RunAction::ConfirmHandSelect)
        | PlannerAction::Run(RunAction::ConfirmDrawSelect)
        | PlannerAction::Run(RunAction::ConfirmDiscardSelect) => Some("CONFIRM".to_owned()),
        PlannerAction::Run(_) => None,
    }
}

fn live_hand_slot(state: &LiveState, hand_position: usize) -> Option<usize> {
    state
        .raw
        .pointer("/summary/combat/hand")
        .and_then(Value::as_array)
        .and_then(|hand| hand.get(hand_position))
        .and_then(|card| card.get("index"))
        .and_then(Value::as_u64)
        .and_then(|slot| usize::try_from(slot).ok())
}

fn live_monster_slot(state: &LiveState, combat: &CombatState, target: MonsterId) -> Option<usize> {
    let position = monster_position(combat, target)?;
    state
        .raw
        .pointer("/summary/combat/monsters")
        .and_then(Value::as_array)
        .and_then(|monsters| monsters.get(position))
        .and_then(|monster| monster.get("index"))
        .and_then(Value::as_u64)
        .and_then(|slot| usize::try_from(slot).ok())
}

fn monster_position(combat: &CombatState, target: MonsterId) -> Option<usize> {
    combat
        .monsters
        .iter()
        .position(|monster| monster.id == target)
}

fn match_live_action<'a>(
    state: &'a LiveState,
    expected_command: &str,
) -> Result<&'a LegalAction, BlockedState> {
    let candidates = state
        .legal_actions
        .iter()
        .filter(|action| {
            action.enabled
                && action
                    .command
                    .get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|command| command.eq_ignore_ascii_case(expected_command))
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [action] => Ok(*action),
        [] => Err(blocked(
            "automation_no_matching_action",
            &format!("planner command {expected_command:?} does not match a live legal action"),
        )),
        _ => Err(blocked(
            "automation_ambiguous_action",
            &format!("planner command {expected_command:?} matched multiple live legal actions"),
        )),
    }
}

fn planned_live_action(
    source_sequence: u64,
    action: &LegalAction,
    command: Option<&str>,
    planner_action: String,
) -> AutomationPlannedAction {
    AutomationPlannedAction {
        action_id: action.id.clone(),
        kind: action.kind.clone(),
        label: action.label.clone(),
        source_sequence,
        command: command.map(str::to_owned),
        planner_action,
    }
}

fn planned_future_action(
    source_sequence: u64,
    run: &RunState,
    action: &PlannerAction,
) -> AutomationPlannedAction {
    AutomationPlannedAction {
        action_id: ActionId("future".to_owned()),
        kind: planner_action_kind(action),
        label: planner_action_display_label(run, action),
        source_sequence,
        command: expected_sim_command(run, action),
        planner_action: planner_action_label(action),
    }
}

fn planner_action_kind(action: &PlannerAction) -> LegalActionKind {
    match action {
        PlannerAction::Combat(CombatAction::PlayCard { .. }) => LegalActionKind::PlayCard,
        PlannerAction::Combat(CombatAction::EndTurn) => LegalActionKind::EndTurn,
        PlannerAction::Potion(_) => LegalActionKind::UsePotion,
        PlannerAction::Run(RunAction::ConfirmExhaustSelect) => LegalActionKind::Confirm,
        PlannerAction::Run(_) => LegalActionKind::Confirm,
    }
}

fn planner_action_label(action: &PlannerAction) -> String {
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

fn planner_action_from_label(label: &str) -> Option<PlannerAction> {
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

fn planner_action_display_label(run: &RunState, action: &PlannerAction) -> String {
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

pub(super) fn blocked(reason_code: &str, message: &str) -> BlockedState {
    BlockedState {
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    }
}

fn planner_simulator_blocked(error: SimError) -> BlockedState {
    blocked(
        "automation_simulator_error",
        &format!("combat planner rejected simulator state: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LivePhase, LiveState};
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use sts_core::{
        combat::{HandSelectPurpose, HandSelectState},
        content::cards::{
            CHRYSALIS_ID, DARK_SHACKLES_ID, HAVOC_PLUS_ID, SADISTIC_NATURE_ID, STRIKE_R_ID,
            WARCRY_ID,
        },
        potion::Potion,
        CardInstance, CombatDecisionState,
    };
    use sts_verify::{
        import_communication_mod_trace, serialize_communication_mod_trace,
        verify_seed_start_communication_mod_trace, TraceLine, TraceMetadata, TraceState,
    };

    fn planner_run_actions(run: &RunState) -> Vec<RunAction> {
        planner_actions(run, &AutomationConfig::default())
            .expect("valid combat decisions")
            .into_iter()
            .filter_map(|action| match action {
                PlannerAction::Potion(action) | PlannerAction::Run(action) => Some(action),
                PlannerAction::Combat(_) => None,
            })
            .collect()
    }

    #[test]
    fn invalid_simulator_state_blocks_search_instead_of_looking_actionless() {
        let mut run = RunState::combat_fixture();
        let duplicate = run.combat.as_ref().expect("combat").piles.hand[0];
        run.combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile
            .push(duplicate);
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({"sim_run_state": run}),
        };

        let error = plan_search_action(&AutomationConfig::default(), &state, &[])
            .expect_err("invalid simulator state must block automation");

        assert_eq!(error.reason_code, "automation_simulator_error");
        assert!(error.message.contains("invalid state"));
    }

    #[test]
    fn warm_suffix_seeds_but_does_not_replace_fresh_search() {
        let run = RunState::combat_fixture();
        let config = AutomationConfig {
            depth: 2,
            width: 10,
            ..AutomationConfig::default()
        };

        let result = beam_search_with_node_limit(
            &run,
            &config,
            100,
            &[PlannerAction::Combat(CombatAction::EndTurn)],
            None,
        )
        .expect("valid combat search");

        assert_eq!(result.cache_hits, 1);
        assert!(
            result.expanded > 0,
            "a fresh frontier must still be searched"
        );
        assert!(result.nodes > 1, "fresh search must consume its own budget");
    }

    #[test]
    fn expired_live_search_deadline_returns_a_structured_timeout() {
        let run = RunState::combat_fixture();
        let config = AutomationConfig {
            depth: 10,
            width: 30,
            ..AutomationConfig::default()
        };

        let result = beam_search_with_node_limit(
            &run,
            &config,
            10_000,
            &[],
            Some(Instant::now() - Duration::from_millis(1)),
        )
        .expect("valid combat search");

        assert!(result.budget_exhausted);
        assert!(result.timed_out);
        assert_eq!(result.nodes, 1);
    }

    #[test]
    fn state_cache_keeps_the_better_path_without_changing_state() {
        let state = RunState::combat_fixture();
        let worse = SearchNode {
            state: state.clone(),
            first_action: Some(PlannerAction::Combat(CombatAction::EndTurn)),
            principal_variation: vec![PlannerAction::Combat(CombatAction::EndTurn)],
            actions: 2,
            score: 1.0,
            terminal_reason: None,
        };
        let better = SearchNode {
            state: state.clone(),
            first_action: Some(PlannerAction::Combat(CombatAction::EndTurn)),
            principal_variation: vec![PlannerAction::Combat(CombatAction::EndTurn)],
            actions: 1,
            score: 2.0,
            terminal_reason: None,
        };

        let (deduplicated, checks, duplicates) = deduplicate_search_nodes(vec![worse, better]);

        assert_eq!(checks, 2);
        assert_eq!(duplicates, 1);
        assert_eq!(deduplicated.len(), 1);
        assert_eq!(deduplicated[0].state, state);
        assert_eq!(deduplicated[0].score, 2.0);
        assert_eq!(deduplicated[0].actions, 1);
    }

    #[test]
    fn winning_nodes_compare_hp_fraction_before_potion_value() {
        fn won_node(hp: i32, potions: Vec<Potion>, actions: usize) -> SearchNode {
            let mut state = RunState::combat_fixture();
            let combat = state.combat.as_mut().expect("combat");
            combat.phase = CombatPhase::Won;
            combat.player.hp = hp;
            combat.player.max_hp = 80;
            state.player_hp = hp;
            state.player_max_hp = 80;
            state.potions = potions;
            SearchNode {
                state,
                first_action: Some(PlannerAction::Combat(CombatAction::EndTurn)),
                principal_variation: vec![PlannerAction::Combat(CombatAction::EndTurn)],
                actions,
                score: 1_000_000.0,
                terminal_reason: Some("won".to_owned()),
            }
        }

        let higher_hp = won_node(40, Vec::new(), 3);
        let lower_hp_with_rare_potion = won_node(39, vec![Potion::Fairy], 2);
        assert!(node_better(&higher_hp, &lower_hp_with_rare_potion));

        let uncommon_potion = won_node(40, vec![Potion::LiquidMemories], 3);
        let common_potion = won_node(40, vec![Potion::Block], 2);
        assert!(node_better(&uncommon_potion, &common_potion));
    }

    #[test]
    fn explosive_potion_planner_actions_include_live_monster_targets() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Explosive];
        run.empty_potion_slots = vec![0, 2];
        let living_targets = run
            .combat
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .filter(|monster| monster.alive)
            .map(|monster| monster.id)
            .collect::<Vec<_>>();

        let actions = planner_run_actions(&run);

        assert_eq!(actions.len(), living_targets.len());
        for target in living_targets {
            assert!(actions.contains(&RunAction::UsePotion {
                slot: 1,
                target: Some(target),
            }));
        }
        assert!(!actions.contains(&RunAction::UsePotion {
            slot: 1,
            target: None,
        }));
    }

    #[test]
    fn toolbox_choices_are_planned_as_combat_card_rewards() {
        let mut run = RunState::combat_fixture();
        run.combat.as_mut().expect("combat").decision =
            Some(CombatDecisionState::ToolboxCardReward {
                choices: vec![
                    CardInstance::new(CardId::new(101), CHRYSALIS_ID),
                    CardInstance::new(CardId::new(102), SADISTIC_NATURE_ID),
                    CardInstance::new(CardId::new(103), DARK_SHACKLES_ID),
                ],
            });
        let state = LiveState {
            sequence: 7692,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-1".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "sadistic nature".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 1"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };
        let run: RunState = serde_json::from_value(state.raw["sim_run_state"].clone()).unwrap();

        let actions =
            planner_actions(&run, &AutomationConfig::default()).expect("valid combat decisions");

        assert_eq!(actions.len(), 3);
        assert!(matches!(
            actions[1],
            PlannerAction::Run(RunAction::ChooseCombatCardReward { index: 1 })
        ));
        assert_eq!(
            expected_command(&state, &run, &actions[1]),
            Some("CHOOSE 1".to_owned())
        );
        assert_eq!(
            planner_action_display_label(&run, &actions[1]),
            "Choose combat card Sadistic Nature"
        );
    }

    #[test]
    fn liquid_memories_grid_is_planned_as_a_combat_discard_selection() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::LiquidMemories];
        run.empty_potion_slots.clear();
        let recalled = CardInstance::new(CardId::new(91), STRIKE_R_ID);
        run.combat
            .as_mut()
            .expect("combat")
            .piles
            .discard_pile
            .push(recalled);
        let run = apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::UsePotion {
                slot: 0,
                target: None,
            }),
        )
        .expect("Liquid Memories opens its discard selection");
        let state = LiveState {
            sequence: 12,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "strike+".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };
        let run: RunState = serde_json::from_value(state.raw["sim_run_state"].clone()).unwrap();

        let actions =
            planner_actions(&run, &AutomationConfig::default()).expect("valid combat decisions");

        assert!(matches!(
            actions.as_slice(),
            [PlannerAction::Run(RunAction::ChooseDiscardSelect {
                index: 0
            })]
        ));
        assert_eq!(
            expected_command(&state, &run, &actions[0]),
            Some("CHOOSE 0".to_owned())
        );
    }

    #[test]
    fn warcry_hand_select_is_planned_as_choose_then_confirm() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        let source = CardInstance::new(CardId::new(90), WARCRY_ID);
        let target = CardInstance::new(CardId::new(91), STRIKE_R_ID);
        combat.piles.hand = vec![source, target];
        combat.decision = Some(CombatDecisionState::HandSelect {
            state: HandSelectState {
                purpose: HandSelectPurpose::WarcryPutOnDraw,
                source_card_id: source.id,
                selected_hand_index: None,
                selected_hand_indices: Vec::new(),
                dual_wield_restore_on_confirm: Vec::new(),
                dual_wield_force_exhaust: false,
            },
            pending_actions: Default::default(),
        });

        let state = LiveState {
            sequence: 13,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("choose-0".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "strike".to_owned(),
                enabled: true,
                command: json!({"command": "CHOOSE 0"}),
                disabled_reason: None,
            }],
            raw: json!({"sim_run_state": run}),
        };

        let actions =
            planner_actions(&run, &AutomationConfig::default()).expect("valid combat decisions");
        assert!(matches!(
            actions.as_slice(),
            [PlannerAction::Run(RunAction::ChooseHandSelect { index: 0 })]
        ));
        assert_eq!(
            expected_command(&state, &run, &actions[0]),
            Some("CHOOSE 0".to_owned())
        );

        let selected = apply_run_decision_action(
            &run,
            RunDecisionAction::Run(RunAction::ChooseHandSelect { index: 0 }),
        )
        .expect("Warcry choice applies");
        let follow_up = planner_actions(&selected, &AutomationConfig::default())
            .expect("valid follow-up decisions");
        assert!(matches!(
            follow_up.as_slice(),
            [PlannerAction::Run(RunAction::ConfirmHandSelect)]
        ));
    }

    #[test]
    fn havoc_live_command_uses_havoc_card_slot_without_top_card_target() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        let target = combat
            .monsters
            .iter()
            .find(|monster| monster.alive)
            .expect("living monster")
            .id;
        combat.piles.hand = vec![CardInstance::new(CardId::new(42), HAVOC_PLUS_ID)];
        combat.piles.draw_pile = vec![CardInstance::new(CardId::new(43), STRIKE_R_ID)];

        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": {
                    "combat": {
                        "hand": [
                            { "index": 2 }
                        ],
                        "monsters": [
                            { "index": 0 }
                        ]
                    }
                }
            }),
        };
        let action = PlannerAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(42),
            target: Some(target),
        });

        assert_eq!(
            expected_command(&state, &run, &action),
            Some("PLAY 2".to_owned())
        );
    }

    #[test]
    fn explicit_targets_require_authoritative_live_monster_slots() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Fire];
        let combat = run.combat.as_ref().expect("combat fixture");
        let card_id = combat.piles.hand[0].id;
        let target = combat
            .monsters
            .iter()
            .find(|monster| monster.alive)
            .expect("living monster")
            .id;
        let state = LiveState {
            sequence: 1,
            phase: LivePhase::Combat,
            legal_actions: Vec::new(),
            raw: json!({
                "summary": {
                    "combat": {
                        "hand": [{ "index": 0 }],
                        "monsters": [{}]
                    }
                }
            }),
        };

        assert_eq!(
            expected_command(
                &state,
                &run,
                &PlannerAction::Combat(CombatAction::PlayCard {
                    card_id,
                    target: Some(target),
                }),
            ),
            None
        );
        assert_eq!(
            expected_command(
                &state,
                &run,
                &PlannerAction::Potion(RunAction::UsePotion {
                    slot: 0,
                    target: Some(target),
                }),
            ),
            None
        );
    }

    #[test]
    fn combat_search_never_offers_smoke_bomb() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::SmokeBomb, Potion::Energy];
        run.empty_potion_slots = vec![2];

        let actions = planner_run_actions(&run);

        assert!(!actions
            .iter()
            .any(|action| matches!(action, RunAction::UsePotion { slot: 0, .. })));
        assert!(actions.contains(&RunAction::UsePotion {
            slot: 1,
            target: None,
        }));
    }

    #[test]
    fn combat_search_never_offers_discovery_potions() {
        for potion in [
            Potion::Attack,
            Potion::Skill,
            Potion::Power,
            Potion::Colorless,
        ] {
            let mut run = RunState::combat_fixture();
            run.potions = vec![potion, Potion::Energy];
            run.empty_potion_slots = vec![2];
            let actions = planner_run_actions(&run);
            assert!(!actions
                .iter()
                .any(|action| matches!(action, RunAction::UsePotion { slot: 0, .. })));
            assert!(actions.contains(&RunAction::UsePotion {
                slot: 1,
                target: None,
            }));
        }
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

        assert!(end_turn_penalty >= 12.0);
        assert_eq!(play_penalty, 0.0);
    }

    #[test]
    fn planner_confirms_gambling_chip_select_without_discards() {
        let mut run = RunState::combat_fixture();
        sts_core::combat::open_gambling_chip_select(run.combat.as_mut().expect("combat"))
            .expect("Gambling Chip selection opens");
        let state = LiveState {
            sequence: 7,
            phase: LivePhase::Combat,
            legal_actions: vec![LegalAction {
                id: ActionId("confirm".to_owned()),
                kind: LegalActionKind::Confirm,
                label: "Confirm".to_owned(),
                enabled: true,
                command: json!({
                    "transport": "communication_mod",
                    "command": "CONFIRM",
                }),
                disabled_reason: None,
            }],
            raw: json!({
                "summary": {
                    "screen_type": "HAND_SELECT",
                },
                "sim_run_state": run,
            }),
        };

        let (planned, snapshot) =
            plan_action_with_warm_start(&AutomationConfig::default(), &state, &[])
                .expect("planner chooses confirm");

        assert_eq!(planned.kind, LegalActionKind::Confirm);
        assert_eq!(planned.command.as_deref(), Some("CONFIRM"));
        assert_eq!(planned.planner_action, "confirm_exhaust_select");
        assert_eq!(snapshot.actions[0].planner_action, "confirm_exhaust_select");
    }

    #[test]
    #[ignore = "expensive corpus-wide combat benchmark; run explicitly with --ignored"]
    fn collected_trace_benchmark_reports_train_and_validation_reward() {
        let cases = collect_trace_combat_cases();
        assert!(
            cases.len() >= 12,
            "expected enough trace combat roots, found {}",
            cases.len()
        );

        let config = AutomationConfig::default();
        let train_cases = cases
            .iter()
            .enumerate()
            .filter_map(|(index, case)| (index % 4 != 0).then_some(case))
            .collect::<Vec<_>>();
        let validation_cases = cases
            .iter()
            .enumerate()
            .filter_map(|(index, case)| (index % 4 == 0).then_some(case))
            .collect::<Vec<_>>();
        let train_sample = evenly_sample_cases(&train_cases, 32);
        let validation_sample = evenly_sample_cases(&validation_cases, 16);

        let train = evaluate_trace_cases("train", &train_sample, &config);
        let validation = evaluate_trace_cases("validation", &validation_sample, &config);

        assert!(
            train.compared >= 6,
            "training split produced too few comparable roots: {train:?}"
        );
        assert!(
            validation.compared >= 3,
            "validation split produced too few comparable roots: {validation:?}"
        );
        assert!(
            train.machine_reward_avg + 2.0 >= train.human_reward_avg,
            "training machine reward regressed too far behind human: {train:?}"
        );
        assert!(
            validation.machine_reward_avg >= validation.human_reward_avg,
            "validation machine reward did not match or beat human: {validation:?}"
        );
    }

    #[derive(Debug)]
    struct TraceCombatCase {
        path: PathBuf,
        start_line_index: usize,
        start_hp: i32,
        human_terminal: ObservedTerminal,
    }

    #[derive(Debug)]
    struct ObservedTerminal {
        hp: i32,
        max_hp: i32,
        gold: i32,
        potions: usize,
    }

    #[derive(Debug)]
    struct TraceBenchmarkReport {
        compared: usize,
        skipped: usize,
        human_reward_avg: f64,
        machine_reward_avg: f64,
        human_hp_loss_avg: f64,
        machine_hp_loss_avg: f64,
        worst_hp_losses: Vec<TraceCaseResult>,
    }

    #[derive(Debug, Clone)]
    struct TraceCaseResult {
        path: PathBuf,
        start_line_index: usize,
        root_potions: usize,
        human_hp_loss: i32,
        machine_hp_loss: i32,
        human_reward: f64,
        machine_reward: f64,
        terminal_reason: Option<String>,
        first_actions: Vec<String>,
    }

    fn collect_trace_combat_cases() -> Vec<TraceCombatCase> {
        let mut paths = std::env::var_os("STS_PERMANENT_CORPUS_DIR")
            .map(PathBuf::from)
            .and_then(|root| std::fs::read_dir(root).ok())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();

        let mut cases = Vec::new();
        for path in paths {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(trace) = import_communication_mod_trace(&content) else {
                continue;
            };

            let mut previous_state: Option<&TraceState> = None;
            for (index, line) in trace.lines.iter().enumerate() {
                let TraceLine::State(state) = line else {
                    continue;
                };
                if observed_room_phase(state) == Some("COMBAT")
                    && previous_state
                        .and_then(observed_room_phase)
                        .is_some_and(|phase| phase != "COMBAT")
                {
                    if let Some(terminal) = trace.lines[index + 1..]
                        .iter()
                        .filter_map(|line| match line {
                            TraceLine::State(state) => Some(state),
                            _ => None,
                        })
                        .find(|state| observed_room_phase(state) != Some("COMBAT"))
                        .and_then(observed_terminal)
                    {
                        if let Some(start_hp) = observed_i32(state, "current_hp") {
                            cases.push(TraceCombatCase {
                                path: path.clone(),
                                start_line_index: index,
                                start_hp,
                                human_terminal: terminal,
                            });
                        }
                    }
                }
                previous_state = Some(state);
            }
        }
        cases
    }

    fn evaluate_trace_cases(
        label: &'static str,
        cases: &[&TraceCombatCase],
        config: &AutomationConfig,
    ) -> TraceBenchmarkReport {
        let mut compared = 0usize;
        let mut skipped = 0usize;
        let mut human_reward = 0.0;
        let mut machine_reward = 0.0;
        let mut human_hp_loss = 0.0;
        let mut machine_hp_loss = 0.0;
        let mut case_results = Vec::new();

        for case in cases {
            let Some(root) = verify_trace_prefix_root(&case.path, case.start_line_index) else {
                skipped += 1;
                continue;
            };
            if root.phase != RunPhase::Combat || root.combat.is_none() {
                skipped += 1;
                continue;
            }

            let Ok(recommendation) = beam_search(&root, config) else {
                skipped += 1;
                continue;
            };
            let Some(reason) = recommendation.terminal_reason.as_deref() else {
                skipped += 1;
                continue;
            };
            if !matches!(reason, "won" | "escaped") {
                skipped += 1;
                continue;
            }

            let human_case_reward = observed_terminal_value(&case.human_terminal);
            let machine_case_reward = recommendation.value - 1_000_000.0;
            let human_case_hp_loss = case.start_hp - case.human_terminal.hp;
            let machine_case_hp_loss = case.start_hp - recommendation.final_hp;
            compared += 1;
            human_reward += human_case_reward;
            machine_reward += machine_case_reward;
            human_hp_loss += f64::from(human_case_hp_loss);
            machine_hp_loss += f64::from(machine_case_hp_loss);
            case_results.push(TraceCaseResult {
                path: case.path.clone(),
                start_line_index: case.start_line_index,
                root_potions: root.potions.len(),
                human_hp_loss: human_case_hp_loss,
                machine_hp_loss: machine_case_hp_loss,
                human_reward: human_case_reward,
                machine_reward: machine_case_reward,
                terminal_reason: recommendation.terminal_reason.clone(),
                first_actions: planned_action_labels(&root, &recommendation.principal_variation, 8),
            });
        }

        case_results.sort_by(|left, right| {
            let left_delta = left.machine_hp_loss - left.human_hp_loss;
            let right_delta = right.machine_hp_loss - right.human_hp_loss;
            right_delta
                .cmp(&left_delta)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.start_line_index.cmp(&right.start_line_index))
        });
        let worst_hp_losses = case_results.into_iter().take(5).collect::<Vec<_>>();

        let report = TraceBenchmarkReport {
            compared,
            skipped,
            human_reward_avg: average(human_reward, compared),
            machine_reward_avg: average(machine_reward, compared),
            human_hp_loss_avg: average(human_hp_loss, compared),
            machine_hp_loss_avg: average(machine_hp_loss, compared),
            worst_hp_losses,
        };
        println!(
            "trace_benchmark {label} compared={} skipped={} human_reward_avg={:.2} machine_reward_avg={:.2} human_hp_loss_avg={:.2} machine_hp_loss_avg={:.2}",
            report.compared,
            report.skipped,
            report.human_reward_avg,
            report.machine_reward_avg,
            report.human_hp_loss_avg,
            report.machine_hp_loss_avg
        );
        for case in &report.worst_hp_losses {
            println!(
                "trace_benchmark_worst_hp {label} file={} line={} root_potions={} human_hp_loss={} machine_hp_loss={} human_reward={:.2} machine_reward={:.2} terminal={:?} actions={}",
                case.path.file_name().and_then(|name| name.to_str()).unwrap_or("<unknown>"),
                case.start_line_index,
                case.root_potions,
                case.human_hp_loss,
                case.machine_hp_loss,
                case.human_reward,
                case.machine_reward,
                case.terminal_reason,
                case.first_actions.join(" | ")
            );
        }
        report
    }

    fn planned_action_labels(
        root: &RunState,
        actions: &[PlannerAction],
        limit: usize,
    ) -> Vec<String> {
        let mut labels = Vec::new();
        let mut state = root.clone();
        for action in actions.iter().take(limit) {
            labels.push(planner_action_display_label(&state, action));
            let Ok(next) = apply_planner_action(&state, action) else {
                break;
            };
            state = next;
        }
        labels
    }

    fn evenly_sample_cases<'a>(
        cases: &[&'a TraceCombatCase],
        max_cases: usize,
    ) -> Vec<&'a TraceCombatCase> {
        if cases.len() <= max_cases {
            return cases.to_vec();
        }
        (0..max_cases)
            .map(|index| {
                let case_index = index * cases.len() / max_cases;
                cases[case_index]
            })
            .collect()
    }

    fn verify_trace_prefix_root(path: &Path, line_index: usize) -> Option<RunState> {
        let content = std::fs::read_to_string(path).ok()?;
        let trace = import_communication_mod_trace(&content).ok()?;
        let metadata = trace.metadata.unwrap_or(TraceMetadata {
            schema: 1,
            source: "communication_mod".to_owned(),
            boundary_schema: None,
            client: None,
            mode: None,
            started_at: None,
            ended_at: None,
            event: None,
            boss_unlocks: None,
            run_config: None,
        });
        let prefix = serialize_communication_mod_trace(&metadata, &trace.lines[..=line_index]);
        let report = verify_seed_start_communication_mod_trace(&prefix).ok()?;
        if !report.unexpected_diffs.is_empty() || !report.unsupported.is_empty() {
            return None;
        }
        report.seed_start?.sim_run_state
    }

    fn observed_room_phase(state: &TraceState) -> Option<&str> {
        state
            .message
            .pointer("/game_state/room_phase")
            .and_then(serde_json::Value::as_str)
    }

    fn observed_terminal(state: &TraceState) -> Option<ObservedTerminal> {
        Some(ObservedTerminal {
            hp: observed_i32(state, "current_hp")?,
            max_hp: observed_i32(state, "max_hp")?,
            gold: observed_i32(state, "gold")?,
            potions: observed_potion_count(state),
        })
    }

    fn observed_terminal_value(terminal: &ObservedTerminal) -> f64 {
        f64::from(terminal.hp)
            + f64::from(terminal.max_hp) * 3.0
            + f64::from(terminal.gold) / 10.0
            + terminal.potions as f64 * 8.0
    }

    fn observed_i32(state: &TraceState, key: &str) -> Option<i32> {
        state
            .message
            .get("game_state")?
            .get(key)?
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
    }

    fn observed_potion_count(state: &TraceState) -> usize {
        state
            .message
            .pointer("/game_state/potions")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|potion| {
                potion
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| id != "Potion Slot")
            })
            .count()
    }

    fn average(total: f64, count: usize) -> f64 {
        if count == 0 {
            0.0
        } else {
            total / count as f64
        }
    }
}
