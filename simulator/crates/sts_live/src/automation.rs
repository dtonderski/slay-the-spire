use crate::model::{
    ActionId, AutomationConfig, AutomationPlanSnapshot, AutomationPlannedAction, AutomationPolicy,
    BlockedState, LegalAction, LegalActionKind, LivePhase, LiveState,
};
use serde_json::Value;
use std::cmp::Ordering;
use sts_core::{
    apply_combat_action_on_run, apply_run_action,
    content::{
        cards::{get_card_definition, HAVOC_ID, HAVOC_PLUS_ID},
        monsters::get_monster_definition,
    },
    legal_combat_actions, validate_potion_action, CardId, CombatAction, CombatPhase, CombatState,
    ContentId, MonsterId, MonsterIntent, RunAction, RunPhase, RunState,
};

#[derive(Clone)]
enum PlannerAction {
    Combat(CombatAction),
    Potion(RunAction),
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
}

pub(super) fn plan_action(
    config: &AutomationConfig,
    state: &LiveState,
) -> Result<(AutomationPlannedAction, AutomationPlanSnapshot), BlockedState> {
    match config.policy {
        AutomationPolicy::FakePlayFirstCard => plan_single_card_play(state),
        AutomationPolicy::GreedySearch | AutomationPolicy::BeamSearch => {
            plan_search_action(config, state)
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
    };
    Ok((planned, snapshot))
}

fn plan_search_action(
    config: &AutomationConfig,
    state: &LiveState,
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

    let recommendation = match config.policy {
        AutomationPolicy::GreedySearch => greedy_search(&run, config),
        AutomationPolicy::BeamSearch => beam_search(&run, config),
        AutomationPolicy::FakePlayFirstCard => unreachable!("handled by caller"),
    };
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
    let mut future_run = apply_planner_action(&run, first).ok();
    for action in recommendation.principal_variation.iter().skip(1) {
        let Some(run_before_action) = future_run.as_ref() else {
            planned_actions.push(planned_future_action(state.sequence, &run, action));
            continue;
        };
        planned_actions.push(planned_future_action(
            state.sequence,
            run_before_action,
            action,
        ));
        future_run = apply_planner_action(run_before_action, action).ok();
    }

    let snapshot = AutomationPlanSnapshot {
        actions: planned_actions,
        played_actions: 0,
        predicted_final_hp: Some(recommendation.final_hp),
        predicted_monster_hp: Some(recommendation.monster_hp),
        value: Some(recommendation.value),
        nodes: recommendation.nodes,
        terminal_reason: recommendation.terminal_reason,
    };
    Ok((planned, snapshot))
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

fn greedy_search(state: &RunState, config: &AutomationConfig) -> SearchRecommendation {
    let mut current = state.clone();
    let mut principal_variation = Vec::new();
    let mut nodes = 1usize;
    let mut terminal = terminal_reason(&current);

    while terminal.is_none() && principal_variation.len() < config.depth {
        let actions = planner_actions(&current, config);
        if actions.is_empty() {
            break;
        }
        let mut best_action = None;
        let mut best_score = f64::NEG_INFINITY;
        for action in actions {
            let Ok(next) = apply_planner_action(&current, &action) else {
                continue;
            };
            nodes += 1;
            let score =
                run_score(&next, terminal_reason(&next).as_deref()) - action_penalty(&action);
            if best_action.is_none() || score > best_score {
                best_score = score;
                best_action = Some(action);
            }
        }
        let Some(action) = best_action else {
            break;
        };
        let Ok(next) = apply_planner_action(&current, &action) else {
            break;
        };
        principal_variation.push(action);
        current = next;
        terminal = terminal_reason(&current);
    }

    let value = run_score(&current, terminal.as_deref());
    let (final_hp, monster_hp) = combat_hp(&current);
    SearchRecommendation {
        principal_variation,
        value,
        nodes,
        terminal_reason: terminal,
        final_hp,
        monster_hp,
    }
}

fn beam_search(state: &RunState, config: &AutomationConfig) -> SearchRecommendation {
    let initial_terminal_reason = terminal_reason(state);
    let mut best = SearchNode {
        state: state.clone(),
        first_action: None,
        principal_variation: Vec::new(),
        actions: 0,
        score: run_score(state, initial_terminal_reason.as_deref()),
        terminal_reason: initial_terminal_reason,
    };
    let mut frontier = vec![best.clone()];
    let mut nodes = 1usize;
    let width = config.width.max(1);

    for _ in 0..config.depth {
        let mut next_frontier = Vec::new();
        for node in std::mem::take(&mut frontier) {
            if node.terminal_reason.is_some() {
                if node_better(&node, &best) {
                    best = node.clone();
                }
                next_frontier.push(node);
                continue;
            }
            let actions = planner_actions(&node.state, config);
            if actions.is_empty() {
                if node_better(&node, &best) {
                    best = node.clone();
                }
                next_frontier.push(node);
                continue;
            }
            for action in actions {
                let Ok(next_state) = apply_planner_action(&node.state, &action) else {
                    continue;
                };
                nodes += 1;
                let child_terminal_reason = terminal_reason(&next_state);
                let score = run_score(&next_state, child_terminal_reason.as_deref())
                    - action_penalty(&action)
                    - node.actions as f64 * 0.05;
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
                next_frontier.push(child);
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        next_frontier.sort_by(node_order);
        next_frontier.truncate(width);
        frontier = next_frontier;
    }

    for node in frontier {
        if node_better(&node, &best) {
            best = node;
        }
    }

    let (final_hp, monster_hp) = combat_hp(&best.state);
    SearchRecommendation {
        principal_variation: best.principal_variation,
        value: best.score,
        nodes,
        terminal_reason: best.terminal_reason,
        final_hp,
        monster_hp,
    }
}

fn planner_actions(state: &RunState, config: &AutomationConfig) -> Vec<PlannerAction> {
    let Some(combat) = state.combat.as_ref() else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    if state.phase == RunPhase::Combat && combat.phase == CombatPhase::WaitingForPlayer {
        actions.extend(
            legal_combat_actions(combat)
                .into_iter()
                .map(PlannerAction::Combat),
        );
    }
    if !config.allowed_potion_slots.is_empty() {
        actions.extend(
            legal_potion_actions(state)
                .into_iter()
                .filter(|action| match action {
                    RunAction::UsePotion { slot, .. } => config.allowed_potion_slots.contains(slot),
                    _ => false,
                })
                .map(PlannerAction::Potion),
        );
    }
    actions
}

fn legal_potion_actions(state: &RunState) -> Vec<RunAction> {
    state
        .occupied_potion_slots()
        .into_iter()
        .flat_map(|(slot, potion)| {
            if potion.requires_target() {
                return state
                    .combat
                    .as_ref()
                    .into_iter()
                    .flat_map(|combat| combat.monsters.iter())
                    .filter(|monster| monster.alive)
                    .map(move |monster| RunAction::UsePotion {
                        slot,
                        target: Some(monster.id),
                    })
                    .collect::<Vec<_>>();
            }
            vec![RunAction::UsePotion { slot, target: None }]
        })
        .filter(|action| validate_potion_action(state, *action).is_ok())
        .collect()
}

fn apply_planner_action(state: &RunState, action: &PlannerAction) -> sts_core::SimResult<RunState> {
    match action {
        PlannerAction::Combat(action) => apply_combat_action_on_run(state, *action),
        PlannerAction::Potion(action) => apply_run_action(state, *action),
    }
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
        return Some("won".to_owned());
    }
    None
}

fn run_score(state: &RunState, terminal_reason: Option<&str>) -> f64 {
    let Some(combat) = state.combat.as_ref() else {
        return match terminal_reason {
            Some("won") => 1_000_000.0,
            Some("lost") => -1_000_000.0,
            _ => 0.0,
        };
    };
    let player_hp = f64::from(combat.player.hp) + f64::from(state.gold) / 25.0;
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
    let state_score =
        player_hp * 25.0 - unblocked * 45.0 + useful_block * 7.5 + player_energy * 0.5
            - monster_hp * 4.0
            - monster_block * 0.75
            - alive_count * 60.0;
    match terminal_reason {
        Some("won") => 1_000_000.0 + state_score,
        Some("lost") => -1_000_000.0 + state_score,
        _ => state_score,
    }
}

fn intent_damage(intent: MonsterIntent) -> i32 {
    match intent {
        MonsterIntent::Attack { damage }
        | MonsterIntent::AttackAndBlock { damage, .. }
        | MonsterIntent::AttackApplyPlayerWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { damage, .. }
        | MonsterIntent::AttackApplyPlayerFrail { damage, .. }
        | MonsterIntent::AttackHealSelf { damage }
        | MonsterIntent::AttackAddWoundsToDiscard { damage, .. }
        | MonsterIntent::AttackAddSlimedToDiscard { damage, .. }
        | MonsterIntent::AttackStealGold { damage, .. } => damage,
        MonsterIntent::AttackMultiple { damage, hits } => damage * hits,
        MonsterIntent::AddBurnToDiscard { damage, .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { damage, .. } => damage,
        _ => 0,
    }
}

fn action_penalty(action: &PlannerAction) -> f64 {
    match action {
        PlannerAction::Potion(_) => 5_000.0,
        PlannerAction::Combat(CombatAction::EndTurn) => 0.1,
        PlannerAction::Combat(CombatAction::PlayCard { .. }) => 0.0,
    }
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
    if candidate.terminal_reason.as_deref() == Some("won")
        && best.terminal_reason.as_deref() != Some("won")
    {
        return true;
    }
    if candidate.terminal_reason.as_deref() != Some("lost")
        && best.terminal_reason.as_deref() == Some("lost")
    {
        return true;
    }
    candidate.score > best.score
}

fn node_order(left: &SearchNode, right: &SearchNode) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.actions.cmp(&right.actions))
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
    let live_slot = state
        .raw
        .pointer("/summary/combat/monsters")
        .and_then(Value::as_array)
        .and_then(|monsters| monsters.get(position))
        .and_then(|monster| monster.get("index"))
        .and_then(Value::as_u64)
        .and_then(|slot| usize::try_from(slot).ok());
    live_slot.or(Some(position))
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
        "use_potion" => {
            let slot = parts.next()?.strip_prefix("slot=")?.parse::<usize>().ok()?;
            let target = parts
                .next()
                .and_then(|part| part.strip_prefix("target="))
                .and_then(|target| target.parse::<u64>().ok())
                .map(MonsterId::new);
            Some(PlannerAction::Potion(RunAction::UsePotion { slot, target }))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LivePhase, LiveState};
    use serde_json::json;
    use sts_core::{
        content::cards::{HAVOC_PLUS_ID, STRIKE_R_ID},
        CardInstance,
    };

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
}
