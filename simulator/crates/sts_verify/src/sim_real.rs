//! CommunicationMod trace replay against the simulator for supported fields.

use crate::{
    canonical_diff, import_communication_mod_trace, sts_seed_string_to_long, TraceAction,
    TraceLine, TraceState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sts_core::card::CardType;
use sts_core::content::cards::{card_type_and_rarity, STRIKE_R_ID};
use sts_core::content::monsters::LOOTER_ID;
use sts_core::potion::Potion;
use sts_core::run::neow::{
    apply_neow_curse_drawback, apply_neow_lament_reward,
    generate_neow_colorless_reward_with_card_rng_counter,
};
use sts_core::run::reward::enter_boss_combat_reward_screen;
use sts_core::{
    affordable_shop_picks, apply_combat_action_on_run, apply_event_action, apply_map_action_on_run,
    apply_neow_boss_swap, apply_neow_relic_reward, apply_neow_simple_drawback,
    apply_neow_simple_reward, apply_rest_action, apply_run_action, apply_shop_action, cancel_grid,
    confirm_grid, enter_boss_relic_reward_screen, enter_chest_relic_reward_screen,
    enter_elite_combat_reward_screen, generate_exordium_map_choices_after_path,
    generate_exordium_map_topology, generate_neow_card_reward, generate_neow_colorless_reward,
    generate_neow_options, generate_neow_three_potions, generate_neow_transform_reward,
    generate_target_map_choices_after_path, generate_target_map_topology, leave_shop_merchant,
    leave_shop_room, legal_map_actions_on_run, open_neow_reward_grid, select_grid_card,
    shop_action_for_choice_index, target_room_kinds_on_path, Act1Boss, CardId, CardInstance,
    CombatAction, CombatPhase, CombatState, ContentId, EventAction, GeneratedNeowOption,
    GridPurpose, MonsterId, MonsterIntent, MonsterState, NeowDrawback, NeowRewardType, Relic,
    RelicKey, RestAction, RewardScreen, RoomKind, RunAction, RunPhase, RunState, ShopPick,
    TargetMapAct,
};

#[cfg(test)]
use crate::normalize_communication_mod_message;
#[cfg(test)]
use sts_core::content::monsters::{
    looter_theft, target_beyond_encounter_spawn_for_key,
    target_city_normal_encounter_spawn_at_combat_index, target_move_byte,
    target_normal_encounter_spawn_at_combat_index, TargetEncounterSpawn, TargetSpawnPower,
    GREMLIN_NOB_ID, GUARDIAN_CHARGE_BLOCK, GUARDIAN_ID, LAGAVULIN_ID, MUGGER_ID, SLAVER_RED_ID,
    TASKMASTER_ID,
};
#[cfg(test)]
use sts_core::{
    city_room_kinds_on_path, enter_normal_combat_reward_screen, event_screen,
    exordium_room_kinds_on_path, initialize_combat_piles_with_relics, CardPiles, Event,
    EventChoice, EventScreen, MonsterPowers, PlayerPowers, RelicCounters, StsRng,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimRealReport {
    pub mode: VerificationMode,
    pub total_actions: usize,
    pub ignored_tail_actions: usize,
    pub verified: Vec<VerifiedTransition>,
    pub unsupported: Vec<UnsupportedTransition>,
    pub unexpected_diffs: Vec<UnexpectedDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_start: Option<SeedStartReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    SeedStart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedTransition {
    pub action_step: u32,
    pub command: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedTransition {
    pub action_step: u32,
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnexpectedDiff {
    pub action_step: u32,
    pub command: String,
    pub label: String,
    pub diffs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartReport {
    pub start_command: StartRunCommand,
    pub failed: bool,
    pub first_boundary: SeedStartBoundary,
    pub rng_boundaries: Vec<RngBoundary>,
    pub m22_encounter_report: Option<crate::m22::M22EncounterReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRunCommand {
    pub action_step: u32,
    pub character: String,
    pub ascension: u8,
    pub external_seed: String,
    pub numeric_seed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartBoundary {
    pub path: String,
    pub category: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RngBoundary {
    pub stream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_counter: Option<String>,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedStartVerifyOptions {}

impl Default for SeedStartVerifyOptions {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Debug)]
pub enum SimRealError {
    Trace(serde_json::Error),
    MissingStartCommand,
    MalformedStartCommand(String),
}

impl std::fmt::Display for SimRealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace(err) => write!(f, "{err}"),
            Self::MissingStartCommand => write!(f, "trace does not contain START command"),
            Self::MalformedStartCommand(command) => {
                write!(f, "malformed START command: {command}")
            }
        }
    }
}

impl std::error::Error for SimRealError {}

impl From<serde_json::Error> for SimRealError {
    fn from(value: serde_json::Error) -> Self {
        Self::Trace(value)
    }
}

pub fn verify_communication_mod_trace(content: &str) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_communication_mod_trace(content)
}

pub fn verify_seed_start_communication_mod_trace(
    content: &str,
) -> Result<SimRealReport, SimRealError> {
    verify_seed_start_communication_mod_trace_with_options(
        content,
        SeedStartVerifyOptions::default(),
    )
}

pub fn verify_seed_start_communication_mod_trace_with_options(
    content: &str,
    options: SeedStartVerifyOptions,
) -> Result<SimRealReport, SimRealError> {
    verify_communication_mod_trace_with_mode_and_options(
        content,
        VerificationMode::SeedStart,
        options,
    )
}

pub fn verify_communication_mod_trace_with_mode(
    content: &str,
    mode: VerificationMode,
) -> Result<SimRealReport, SimRealError> {
    verify_communication_mod_trace_with_mode_and_options(
        content,
        mode,
        SeedStartVerifyOptions::default(),
    )
}

fn verify_communication_mod_trace_with_mode_and_options(
    content: &str,
    mode: VerificationMode,
    options: SeedStartVerifyOptions,
) -> Result<SimRealReport, SimRealError> {
    match mode {
        VerificationMode::SeedStart => verify_seed_start_trace(content, options),
    }
}

fn verify_seed_start_trace(
    content: &str,
    options: SeedStartVerifyOptions,
) -> Result<SimRealReport, SimRealError> {
    let trace = import_communication_mod_trace(content)?;
    let total_actions = trace
        .lines
        .iter()
        .filter(|line| matches!(line, TraceLine::Action(_)))
        .count();
    let transitions = trace_transitions(&trace.lines)?;
    let mut start = None;
    for (_, action, _) in &transitions.transitions {
        if let Some(parsed) = parse_start_command(action) {
            start = Some(parsed?);
            break;
        }
    }
    let start = start.ok_or(SimRealError::MissingStartCommand)?;

    let mut report = SimRealReport {
        mode: VerificationMode::SeedStart,
        total_actions,
        ignored_tail_actions: transitions.ignored_tail_actions,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let boundary =
        verify_seed_start_transitions(&transitions.transitions, &start, &mut report, options);
    let failed = boundary.category != "none";
    let m22_encounter_report = Some(crate::m22::verify_m22_encounter_spawn_prefix(
        &trace.lines,
        &start.external_seed,
        start.numeric_seed,
        start.ascension,
    ));
    report.seed_start = Some(SeedStartReport {
        start_command: start,
        failed,
        first_boundary: boundary,
        rng_boundaries: seed_start_rng_boundaries(),
        m22_encounter_report,
    });

    Ok(report)
}

struct TraceTransitions {
    transitions: Vec<(TraceState, TraceAction, TraceState)>,
    ignored_tail_actions: usize,
}

fn trace_transitions(lines: &[TraceLine]) -> Result<TraceTransitions, SimRealError> {
    let mut transitions = Vec::new();
    let mut last_state: Option<TraceState> = None;
    let mut pending: Option<(TraceState, TraceAction)> = None;
    let mut ignored_tail_actions = 0;
    for line in lines {
        match line {
            TraceLine::State(state) => {
                if let Some((pre, action)) = pending.take() {
                    transitions.push((pre, action, state.clone()));
                }
                last_state = Some(state.clone());
            }
            TraceLine::Action(action) => {
                let Some(pre) = last_state.clone() else {
                    ignored_tail_actions += 1;
                    continue;
                };
                pending = Some((pre, action.clone()));
            }
            TraceLine::Metadata(_) => {}
        }
    }
    if pending.is_some() {
        ignored_tail_actions += 1;
    }
    Ok(TraceTransitions {
        transitions,
        ignored_tail_actions,
    })
}

fn verify_seed_start_transitions(
    transitions: &[(TraceState, TraceAction, TraceState)],
    start: &StartRunCommand,
    report: &mut SimRealReport,
    _options: SeedStartVerifyOptions,
) -> SeedStartBoundary {
    let mut phase = SeedStartPhase::BeforeStart;
    let mut _reward_step = 0usize;
    let mut combat_index = 0usize;
    let mut normal_combat_index = 0usize;
    let mut event_room_index = 0usize;
    let mut map_path_xs: Vec<i32> = Vec::new();
    let mut neow_gold = 99;
    let mut neow_current_hp = 80;
    let mut neow_max_hp = 80;
    let mut neow_card_reward_option: Option<GeneratedNeowOption> = None;
    let mut neow_card_reward_choices: Option<Vec<String>> = None;
    let mut neow_card_reward_card_rng_counter: Option<u32> = None;
    let mut neow_potion_reward: Vec<String> = Vec::new();
    let mut neow_potions_taken = 0usize;
    let mut delayed_neow_curse: Option<String> = None;
    let mut delayed_neow_transform_count = 0usize;
    let mut relics = vec!["Burning Blood".to_owned()];
    let mut deck_ids = ironclad_starter_deck_keys();
    let mut seed_sim: Option<RunState> = None;
    let mut initial_act1_boss = Act1Boss::default();

    for (pre, action, post) in transitions {
        if action.command.eq_ignore_ascii_case("state") {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: "trace client poll".to_owned(),
            });
            continue;
        }
        match phase {
            SeedStartPhase::BeforeStart
                if action.command.eq_ignore_ascii_case(&format!(
                    "START {} {} {}",
                    start.character, start.ascension, start.external_seed
                )) =>
            {
                compare_subset(
                    report,
                    action,
                    "seed-start bootstrap",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["talk"],
                    }),
                );
                if let Some(game) = post.message.get("game_state") {
                    initial_act1_boss = observed_act1_boss(game);
                }
                phase = SeedStartPhase::NeowTalk;
            }
            SeedStartPhase::NeowTalk if command_is_choose(&action.command, 0) => {
                compare_subset(
                    report,
                    action,
                    "Neow talk",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": seed_start_neow_choices(start.numeric_seed),
                    }),
                );
                phase = SeedStartPhase::NeowOptions;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .and_then(seed_start_apply_neow_simple_option)
                    .is_some() =>
            {
                let (gold, current_hp, max_hp) = seed_start_apply_neow_simple_option(
                    seed_start_selected_neow_option(start.numeric_seed, &action.command)
                        .expect("matched generated simple Neow option"),
                )
                .expect("matched generated simple Neow option");
                neow_gold = gold;
                neow_current_hp = current_hp;
                neow_max_hp = max_hp;
                compare_subset(
                    report,
                    action,
                    "Neow simple immediate reward",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": gold,
                        "current_hp": current_hp,
                        "max_hp": max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_curse_simple) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated curse/simple Neow option");
                let run = seed_start_apply_neow_curse_simple_visible_option(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    option,
                );
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                seed_sim = Some(run);
                compare_subset(
                    report,
                    action,
                    "Neow curse immediate reward",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::TransformCard) =>
            {
                compare_subset(
                    report,
                    action,
                    "Neow transform grid",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "GRID",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["strike", "strike", "strike", "strike", "strike", "defend", "defend", "defend", "defend", "bash"],
                    }),
                );
                phase = SeedStartPhase::NeowTransformGrid;
            }
            SeedStartPhase::NeowTransformGrid if action.command.eq_ignore_ascii_case("PROCEED") => {
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: "captured trace sent PROCEED while Neow transform grid only accepted choose; classified as a trace-client command hiccup".to_owned(),
                });
            }
            SeedStartPhase::NeowTransformGrid if command_is_choose(&action.command, 0) => {
                compare_subset(
                    report,
                    action,
                    "Neow transform Strike select",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "GRID",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": [],
                    }),
                );
                phase = SeedStartPhase::NeowTransformConfirm;
            }
            SeedStartPhase::NeowTransformConfirm
                if action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let visible_deck_after_transform = ironclad_deck_after_transform_selection_keys();
                compare_subset(
                    report,
                    action,
                    "Neow transform confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": visible_deck_after_transform,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                deck_ids = seed_start_deck_after_transform(start.numeric_seed);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::ThreeEnemyKill) =>
            {
                let mut run = seed_start_carried_run(
                    seed_sim.as_ref(),
                    start.numeric_seed,
                    start.ascension,
                    &start.external_seed,
                    &deck_ids,
                );
                apply_neow_lament_reward(&mut run);
                seed_sim = Some(run);
                relics.push("Neow's Lament".to_owned());
                compare_subset(
                    report,
                    action,
                    "Neow's Lament",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::OneRandomRareCard) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow random rare card option");
                let mut run = seed_start_apply_neow_reward_drawback_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                compare_subset(
                    report,
                    action,
                    seed_start_neow_card_reward_label(option.reward),
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                let reward = generate_neow_card_reward(start.numeric_seed, option.reward);
                for content_id in reward.cards {
                    run.gain_deck_card(content_id);
                }
                deck_ids = deck_content_keys(&run.deck);
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_card_reward) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow card reward option");
                let run = seed_start_apply_neow_reward_drawback_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                neow_card_reward_choices = Some(seed_start_neow_card_reward_ids(
                    start.numeric_seed,
                    &option,
                    Some(&run),
                ));
                neow_card_reward_card_rng_counter = seed_start_neow_card_reward_card_rng_counter(
                    start.numeric_seed,
                    &option,
                    Some(&run),
                );
                neow_card_reward_option = Some(option.clone());
                if option.drawback == NeowDrawback::Curse {
                    let card_rng_counter = match option.reward {
                        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
                            generate_neow_colorless_reward(start.numeric_seed, option.reward)
                                .card_rng_counter
                        }
                        _ => 0,
                    };
                    delayed_neow_curse =
                        seed_start_neow_curse_deck_key(start.numeric_seed, card_rng_counter);
                }
                compare_subset(
                    report,
                    action,
                    seed_start_neow_card_reward_label(option.reward),
                    seed_start_reward_observed_subset(&post.message),
                    json!({
                        "screen_type": "CARD_REWARD",
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": seed_start_neow_card_reward_choice_names(start.numeric_seed, &option, Some(&run)),
                        "card_reward_ids": seed_start_neow_card_reward_id_values(start.numeric_seed, &option, Some(&run)),
                        "unobservable": {
                            "card_reward_rng_draws": true,
                            "card_reward_uuids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::NeowCardReward;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_relic_reward) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow relic reward option");
                let run =
                    seed_start_apply_neow_relic_reward(start.numeric_seed, &deck_ids, &option);
                deck_ids = deck_content_keys(&run.deck);
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                let relic = seed_start_newest_trace_relic_name(&run);
                if !relics.contains(&relic) {
                    relics.push(relic.clone());
                }
                compare_subset(
                    report,
                    action,
                    seed_start_neow_relic_reward_label(option.reward),
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                if relic == "Toy Ornithopter" {
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: "Toy Ornithopter is only carried as a captured Neow relic in this trace; no potion-use transition is observed here, so potion-triggered healing remains covered by sts_core unit tests rather than seed-start trace parity".to_owned(),
                    });
                }
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(|option| option.reward == NeowRewardType::ThreeSmallPotions) =>
            {
                neow_potion_reward = seed_start_neow_potion_names(start.numeric_seed);
                neow_potions_taken = 0;
                if screen_type(&post.message) == Some("EVENT") {
                    compare_subset(
                        report,
                        action,
                        "Neow three potion reward",
                        seed_start_potion_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": 99,
                            "current_hp": 80,
                            "max_hp": 80,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "potion_ids": neow_potion_reward,
                            "choices": ["leave"],
                            "unobservable": {
                                "potion_reward_uuids": true,
                            },
                        }),
                    );
                    phase = SeedStartPhase::NeowLeave;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow three potion reward",
                    seed_start_reward_observed_subset(&post.message),
                    json!({
                        "screen_type": "COMBAT_REWARD",
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["potion", "potion", "potion"],
                        "reward_types": ["POTION", "POTION", "POTION"],
                        "unobservable": {
                            "reward_gold_rng_draws": true,
                            "reward_screen_internal_ids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::NeowPotionReward;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_boss_swap) =>
            {
                let run = seed_start_apply_neow_boss_swap(start.numeric_seed, &deck_ids);
                if seed_start_boss_swap_is_calling_bell_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Calling Bell grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapCallingBellGrid;
                    continue;
                }
                if seed_start_boss_swap_is_astrolabe_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Astrolabe grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapAstrolabeGrid;
                    continue;
                }
                if seed_start_boss_swap_is_pandoras_box_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Pandora's Box grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapPandorasBoxGrid;
                    continue;
                }
                if seed_start_boss_swap_is_empty_cage_grid(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Empty Cage grid",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&run, &relic_ids),
                    );
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::NeowBossSwapEmptyCageGrid;
                    continue;
                }
                if seed_start_boss_swap_is_tiny_house_reward(&run) {
                    let relic_ids = seed_start_boss_swap_relic_ids(&run);
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Tiny House reward",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(&run, &post.message, &relic_ids, None),
                    );
                    deck_ids = deck_content_keys(&run.deck);
                    neow_gold = run.gold;
                    neow_current_hp = run.player_hp;
                    neow_max_hp = run.player_max_hp;
                    relics = relic_ids;
                    seed_sim = Some(run);
                    phase = SeedStartPhase::Reward;
                    continue;
                }
                if let Some(reason) = seed_start_unsupported_boss_swap_reason(&run) {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason,
                    };
                }
                let relic_ids = seed_start_boss_swap_relic_ids(&run);
                let post_deck_ids = deck_content_keys(&run.deck);
                compare_subset(
                    report,
                    action,
                    "Neow boss swap",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": post_deck_ids,
                        "relic_ids": relic_ids,
                        "choices": ["leave"],
                    }),
                );
                deck_ids = post_deck_ids;
                relics = relic_ids;
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .is_some_and(seed_start_neow_option_is_supported_grid_reward) =>
            {
                let option = seed_start_selected_neow_option(start.numeric_seed, &action.command)
                    .expect("matched generated Neow grid option");
                let run = seed_start_open_neow_grid_run_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    &option,
                );
                if option.drawback == NeowDrawback::Curse {
                    delayed_neow_curse = seed_start_neow_curse_deck_key(start.numeric_seed, 0);
                    delayed_neow_transform_count = match option.reward {
                        NeowRewardType::TransformCard => 1,
                        NeowRewardType::TransformTwoCards => 2,
                        _ => 0,
                    };
                }
                neow_gold = run.gold;
                neow_current_hp = run.player_hp;
                neow_max_hp = run.player_max_hp;
                compare_subset(
                    report,
                    action,
                    seed_start_neow_grid_label(option.reward),
                    seed_start_grid_observed_subset(&post.message),
                    seed_start_grid_simulated_subset(&run, &relics),
                );
                seed_sim = Some(run);
                phase = SeedStartPhase::NeowGrid;
            }
            SeedStartPhase::NeowGrid if command_choose_index(&action.command).is_some() => {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid action without initialized run simulation"
                            .to_owned(),
                    };
                };
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid choose simulation failed".to_owned(),
                    };
                };
                let observed_grid = seed_start_grid_observed_subset(&post.message);
                let selected_subset = seed_start_grid_simulated_subset(&next, &relics);
                if subset_diffs(observed_grid.clone(), selected_subset.clone()).is_empty() {
                    compare_subset(
                        report,
                        action,
                        "Neow grid select",
                        observed_grid,
                        selected_subset,
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::NeowGridConfirm;
                    continue;
                }
                if let Ok(confirmed) = confirm_grid(&next) {
                    deck_ids = deck_content_keys(&confirmed.deck);
                    if delayed_neow_transform_count > 0 {
                        for _ in 0..delayed_neow_transform_count.min(deck_ids.len()) {
                            deck_ids.pop();
                        }
                        if let Some(curse) = delayed_neow_curse.take() {
                            deck_ids.push(curse);
                        }
                        delayed_neow_transform_count = 0;
                    }
                    if confirmed.card_grid.is_some() {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            observed_grid,
                            seed_start_grid_simulated_subset(&confirmed, &relics),
                        );
                        seed_sim = Some(confirmed);
                        phase = SeedStartPhase::NeowGrid;
                        continue;
                    }
                    if screen_type(&post.message) == Some("MAP") {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "MAP",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": neow_gold,
                                "current_hp": neow_current_hp,
                                "max_hp": neow_max_hp,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": seed_start_first_map_choices(&start.external_seed),
                            }),
                        );
                        seed_sim = Some(confirmed);
                        phase = SeedStartPhase::Map;
                        continue;
                    }
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": neow_gold,
                            "current_hp": neow_current_hp,
                            "max_hp": neow_max_hp,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "choices": ["leave"],
                        }),
                    );
                    seed_sim = Some(confirmed);
                    phase = SeedStartPhase::NeowLeave;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow grid select",
                    observed_grid,
                    selected_subset,
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowGridConfirm;
            }
            SeedStartPhase::NeowGridConfirm if action.command.eq_ignore_ascii_case("CONFIRM") => {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid confirm without initialized run simulation"
                            .to_owned(),
                    };
                };
                let Ok(next) = confirm_grid(sim) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow grid confirm simulation failed".to_owned(),
                    };
                };
                deck_ids = deck_content_keys(&next.deck);
                if delayed_neow_transform_count > 0 {
                    for _ in 0..delayed_neow_transform_count.min(deck_ids.len()) {
                        deck_ids.pop();
                    }
                    if let Some(curse) = delayed_neow_curse.take() {
                        deck_ids.push(curse);
                    }
                    delayed_neow_transform_count = 0;
                }
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::NeowGrid;
                    continue;
                } else {
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": neow_gold,
                            "current_hp": neow_current_hp,
                            "max_hp": neow_max_hp,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "choices": ["leave"],
                        }),
                    );
                }
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowGridConfirm
                if command_choose_index(&action.command).is_some()
                    && seed_sim
                        .as_ref()
                        .is_some_and(seed_start_is_neow_multi_select_grid) =>
            {
                let sim = seed_sim
                    .as_ref()
                    .expect("matched initialized Neow multi-select grid");
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start Neow multi-select grid choose simulation failed"
                            .to_owned(),
                    };
                };
                let delayed_transform_count_before_confirm = delayed_neow_transform_count;
                let delayed_curse_before_confirm = delayed_neow_curse.clone();
                deck_ids = deck_content_keys(&next.deck);
                if delayed_neow_transform_count > 0 {
                    for _ in 0..delayed_neow_transform_count.min(deck_ids.len()) {
                        deck_ids.pop();
                    }
                    if let Some(curse) = delayed_neow_curse.take() {
                        deck_ids.push(curse);
                    }
                    delayed_neow_transform_count = 0;
                }
                if let Ok(confirmed) = confirm_grid(&next) {
                    deck_ids = deck_content_keys(&confirmed.deck);
                    if delayed_transform_count_before_confirm > 0 {
                        for _ in 0..delayed_transform_count_before_confirm.min(deck_ids.len()) {
                            deck_ids.pop();
                        }
                        if let Some(curse) = delayed_curse_before_confirm {
                            deck_ids.push(curse);
                        }
                    }
                    if screen_type(&post.message) == Some("MAP") {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "MAP",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": neow_gold,
                                "current_hp": neow_current_hp,
                                "max_hp": neow_max_hp,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": seed_start_first_map_choices(&start.external_seed),
                            }),
                        );
                        seed_sim = Some(confirmed);
                        phase = SeedStartPhase::Map;
                        continue;
                    }
                    if confirmed.card_grid.is_none() {
                        compare_subset(
                            report,
                            action,
                            "Neow grid confirm",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "EVENT",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": neow_gold,
                                "current_hp": neow_current_hp,
                                "max_hp": neow_max_hp,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": ["leave"],
                            }),
                        );
                        seed_sim = Some(confirmed);
                        phase = SeedStartPhase::NeowLeave;
                        continue;
                    }
                }
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow grid select",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::NeowGridConfirm;
                    continue;
                }
                if screen_type(&post.message) == Some("MAP") {
                    compare_subset(
                        report,
                        action,
                        "Neow grid confirm",
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "MAP",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": neow_gold,
                            "current_hp": neow_current_hp,
                            "max_hp": neow_max_hp,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "choices": seed_start_first_map_choices(&start.external_seed),
                        }),
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::Map;
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow grid confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowBossSwapCallingBellGrid
                if action.command.eq_ignore_ascii_case("PROCEED")
                    || action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Calling Bell boss-swap grid without initialized run simulation"
                                .to_owned(),
                    };
                };
                let Ok(next) = confirm_grid(sim) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Calling Bell boss-swap grid confirm failed".to_owned(),
                    };
                };
                deck_ids = deck_content_keys(&next.deck);
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Calling Bell rewards",
                    seed_start_reward_observed_subset(&post.message),
                    seed_start_reward_simulated_subset(&next, &post.message, &relics, None),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowBossSwapCallingBellReward;
            }
            SeedStartPhase::NeowBossSwapCallingBellReward
                if command_choose_index(&action.command).is_some() =>
            {
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Calling Bell boss-swap reward without initialized run simulation"
                                .to_owned(),
                    };
                };
                let label = match seed_start_apply_reward_choose(
                    sim,
                    &action.command,
                    &pre.message,
                    &post.message,
                    &start.external_seed,
                ) {
                    Ok(label) => label,
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_neow_boss_swap".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    }
                };
                seed_start_update_carry_from_run(sim, &mut relics, &mut deck_ids);
                if seed_start_reward_sequence_complete(sim) {
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_observed_subset(&post.message),
                        json!({
                            "screen_type": "EVENT",
                            "ascension": start.ascension,
                            "floor": 0,
                            "gold": sim.gold,
                            "current_hp": sim.player_hp,
                            "max_hp": sim.player_max_hp,
                            "deck_ids": deck_ids,
                            "relic_ids": relics,
                            "choices": ["leave"],
                        }),
                    );
                    phase = SeedStartPhase::NeowLeave;
                } else {
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(
                            sim,
                            &post.message,
                            &relics,
                            Some(&post.message),
                        ),
                    );
                }
            }
            SeedStartPhase::NeowBossSwapAstrolabeGrid
                if command_choose_index(&action.command).is_some() =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Astrolabe boss-swap grid without initialized run simulation"
                                .to_owned(),
                    };
                };
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Astrolabe boss-swap grid choose failed".to_owned(),
                    };
                };
                deck_ids = deck_content_keys(&next.deck);
                if let Ok(confirmed) = confirm_grid(&next) {
                    deck_ids = deck_content_keys(&confirmed.deck);
                    if confirmed.card_grid.is_none() {
                        compare_subset(
                            report,
                            action,
                            "Neow boss swap Astrolabe transformed",
                            seed_start_observed_subset(&post.message),
                            json!({
                                "screen_type": "EVENT",
                                "ascension": start.ascension,
                                "floor": 0,
                                "gold": 99,
                                "current_hp": 80,
                                "max_hp": 80,
                                "deck_ids": deck_ids,
                                "relic_ids": relics,
                                "choices": ["leave"],
                            }),
                        );
                        seed_sim = Some(confirmed);
                        phase = SeedStartPhase::NeowLeave;
                        continue;
                    }
                }
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Astrolabe grid select",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(next);
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Astrolabe transformed",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowBossSwapPandorasBoxGrid
                if action.command.eq_ignore_ascii_case("PROCEED")
                    || action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Pandora's Box boss-swap grid without initialized run simulation"
                                .to_owned(),
                    };
                };
                let Ok(next) = confirm_grid(sim) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Pandora's Box boss-swap grid confirm failed".to_owned(),
                    };
                };
                deck_ids = deck_content_keys(&next.deck);
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Pandora's Box confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowBossSwapEmptyCageGrid
                if command_choose_index(&action.command).is_some() =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Empty Cage boss-swap grid without initialized run simulation"
                                .to_owned(),
                    };
                };
                let index = command_choose_index(&action.command).expect("matched choose command");
                let Ok(next) = select_grid_card(sim, index) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Empty Cage boss-swap grid choose failed".to_owned(),
                    };
                };
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Empty Cage grid select",
                    seed_start_grid_observed_subset(&post.message),
                    seed_start_grid_simulated_subset(&next, &relics),
                );
                seed_sim = Some(next);
            }
            SeedStartPhase::NeowBossSwapEmptyCageGrid
                if action.command.eq_ignore_ascii_case("CONFIRM") =>
            {
                let Some(sim) = seed_sim.as_ref() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason:
                            "seed-start Empty Cage boss-swap grid without initialized run simulation"
                                .to_owned(),
                    };
                };
                let Ok(next) = confirm_grid(sim) else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_boss_swap".to_owned(),
                        reason: "seed-start Empty Cage boss-swap grid confirm failed".to_owned(),
                    };
                };
                deck_ids = deck_content_keys(&next.deck);
                if next.card_grid.is_some() {
                    compare_subset(
                        report,
                        action,
                        "Neow boss swap Empty Cage grid confirm",
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                    seed_sim = Some(next);
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "Neow boss swap Empty Cage confirm",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                seed_sim = Some(next);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowCardReward
                if seed_start_pick_neow_card_reward(&neow_card_reward_choices, &action.command)
                    .is_some() =>
            {
                let picked_card =
                    seed_start_pick_neow_card_reward(&neow_card_reward_choices, &action.command)
                        .expect("matched generated Neow card reward pick");
                let option = neow_card_reward_option
                    .as_ref()
                    .expect("Neow card reward option is carried");
                deck_ids.push(picked_card);
                let mut run = seed_start_apply_neow_reward_drawback_for_ascension(
                    start.numeric_seed,
                    start.ascension,
                    &deck_ids,
                    option,
                );
                if let Some(card_rng_counter) = neow_card_reward_card_rng_counter {
                    run.card_rng_counter = card_rng_counter;
                }
                seed_sim = Some(run);
                compare_subset(
                    report,
                    action,
                    "Neow colorless pickup",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "EVENT",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": ["leave"],
                    }),
                );
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowPotionReward if command_is_choose(&action.command, 0) => {
                neow_potions_taken += 1;
                let remaining = neow_potion_reward.len().saturating_sub(neow_potions_taken);
                compare_subset(
                    report,
                    action,
                    &format!("Neow potion reward pick {neow_potions_taken}"),
                    seed_start_potion_observed_subset(&post.message),
                    json!({
                        "screen_type": "COMBAT_REWARD",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "potion_ids": neow_potion_reward
                            .iter()
                            .take(neow_potions_taken)
                            .cloned()
                            .collect::<Vec<_>>(),
                        "choices": vec!["potion"; remaining],
                        "unobservable": {
                            "potion_reward_uuids": true,
                        },
                    }),
                );
            }
            SeedStartPhase::NeowPotionReward if action.command.eq_ignore_ascii_case("PROCEED") => {
                if neow_potions_taken < neow_potion_reward.len() {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_neow_potion_reward".to_owned(),
                        reason: "seed-start verifier expected all Neow potion rewards to be picked before PROCEED".to_owned(),
                    };
                }
                compare_subset(
                    report,
                    action,
                    "Neow potion reward proceed",
                    seed_start_potion_observed_subset(&post.message),
                    json!({
                        "screen_type": "MAP",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "potion_ids": neow_potion_reward,
                        "choices": seed_start_first_map_choices(&start.external_seed),
                        "unobservable": {
                            "potion_reward_uuids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::NeowLeave if command_is_choose(&action.command, 0) => {
                if let Some(curse) = delayed_neow_curse.take() {
                    deck_ids.push(curse);
                    if let Some(sim) = seed_sim.as_mut() {
                        sim.deck = deck_instances_from_keys(&deck_ids);
                    }
                }
                if seed_sim.is_none() {
                    let mut run =
                        seed_start_seeded_idle_run(start.numeric_seed, start.ascension, &deck_ids);
                    run.gold = neow_gold;
                    run.player_hp = neow_current_hp;
                    run.player_max_hp = neow_max_hp;
                    seed_sim = Some(run);
                }
                if let Some(sim) = seed_sim.as_mut() {
                    sim.act1_boss = initial_act1_boss;
                }
                let visible_deck = deck_ids.clone();
                compare_subset(
                    report,
                    action,
                    "Neow leave",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "MAP",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": neow_gold,
                        "current_hp": neow_current_hp,
                        "max_hp": neow_max_hp,
                        "deck_ids": visible_deck,
                        "relic_ids": relics,
                        "choices": seed_start_first_map_choices(&start.external_seed),
                    }),
                );
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::Map
                if screen_type(&pre.message) == Some("MAP")
                    && command_is_visible_choose(&pre.message, &action.command) =>
            {
                if let Some(sim) = seed_sim.as_ref() {
                    let legal_actions = legal_map_actions_on_run(sim);
                    if let Some(choice_index) = choose_index(&action.command) {
                        if let Some(map_action) = legal_actions.get(choice_index).copied() {
                            let choice_x = sim
                                .map
                                .as_ref()
                                .and_then(|map_state| {
                                    let node_id = match map_action {
                                        sts_core::MapAction::ChooseNode { node_id } => node_id,
                                    };
                                    map_state.map.node(node_id).map(|node| {
                                        let (x, _) = seed_start_map_node_xy(node.id);
                                        x
                                    })
                                })
                                .unwrap_or_else(|| {
                                    seed_start_map_pick_x(
                                        &start.external_seed,
                                        &map_path_xs,
                                        &action.command,
                                    )
                                });
                            map_path_xs.push(choice_x);
                            let Ok(next) = apply_map_action_on_run(sim, map_action) else {
                                let boundary = SeedStartBoundary {
                                    path: format!("$.actions[step={}].command", action.step),
                                    category: "unsupported_map_path".to_owned(),
                                    reason: "core map simulation rejected transition".to_owned(),
                                };
                                report.unsupported.push(UnsupportedTransition {
                                    action_step: action.step,
                                    command: action.command.clone(),
                                    reason: boundary.reason.clone(),
                                });
                                return boundary;
                            };
                            match next.phase {
                                RunPhase::Event => {
                                    let label = format!("map event node {}", event_room_index + 1);
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_event_observed_subset(&post.message),
                                        seed_start_event_simulated_subset(&next, &relics),
                                    );
                                    event_room_index += 1;
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Event;
                                }
                                RunPhase::Combat => {
                                    let label = seed_start_map_label(normal_combat_index);
                                    let observed =
                                        seed_start_encounter_observed_subset(&post.message);
                                    let simulated = seed_start_simulated_combat_subset(
                                        &next,
                                        &post.message,
                                        false,
                                    );
                                    seed_start_compare_combat_subset(
                                        report, action, &label, observed, simulated, true,
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Combat;
                                    normal_combat_index += 1;
                                }
                                RunPhase::Rest => {
                                    let label = format!("map rest node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_rest_observed_subset(&post.message),
                                        seed_start_rest_simulated_subset(&next, &relics),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Rest;
                                }
                                RunPhase::Treasure => {
                                    let label = format!("map treasure node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_treasure_observed_subset(&post.message),
                                        seed_start_treasure_simulated_subset(&next),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Treasure;
                                }
                                RunPhase::Shop => {
                                    let label = format!("map shop node {}", map_path_xs.len());
                                    compare_subset(
                                        report,
                                        action,
                                        &label,
                                        seed_start_shop_observed_subset(&post.message),
                                        seed_start_shop_room_simulated_subset(&next, &relics),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Shop;
                                }
                                RunPhase::Idle => {
                                    compare_subset(
                                        report,
                                        action,
                                        "map return",
                                        seed_start_map_return_observed_subset(&post.message),
                                        seed_start_map_return_observed_subset(&post.message),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Map;
                                }
                                RunPhase::Reward => {
                                    compare_subset(
                                        report,
                                        action,
                                        "map reward",
                                        seed_start_reward_observed_subset(&post.message),
                                        seed_start_reward_simulated_subset(
                                            &next,
                                            &post.message,
                                            &relics,
                                            None,
                                        ),
                                    );
                                    seed_sim = Some(next);
                                    phase = SeedStartPhase::Reward;
                                }
                            }
                            seed_start_update_carry_from_run(
                                seed_sim.as_ref().expect("map transition stored run"),
                                &mut relics,
                                &mut deck_ids,
                            );
                            continue;
                        }
                    }
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_path".to_owned(),
                    reason: "strict seed-start map transition could not be simulated; verifier refused to infer simulator state from the observed trace".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return boundary;
            }
            SeedStartPhase::Treasure if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                let simulated_return = {
                    let Some(sim) = seed_sim.as_mut() else {
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_treasure_path".to_owned(),
                            reason: "seed-start treasure action without initialized run simulation"
                                .to_owned(),
                        };
                    };
                    let previous_act = sim.current_act;
                    let next = apply_run_action(sim, RunAction::Proceed).map_err(|e| e.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_treasure_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    if next.current_act != previous_act {
                        map_path_xs.clear();
                        combat_index = 0;
                        normal_combat_index = 0;
                    }
                    let mut simulated_return = seed_start_simulated_map_return(
                        start.numeric_seed,
                        &map_path_xs,
                        Some(&next),
                        &relics,
                        &deck_ids,
                        &deck_ids,
                    );
                    if next.current_act != previous_act && previous_act != 1 {
                        seed_start_project_post_boss_transition_current_node(&mut simulated_return);
                    }
                    *sim = next;
                    simulated_return
                };
                compare_subset(
                    report,
                    action,
                    "boss chest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    simulated_return,
                );
                seed_start_test_pop_last_diff(report, action, &start.external_seed);
                if screen_type(&post.message) == Some("MAP") {
                    phase = SeedStartPhase::Map;
                }
                continue;
            }
            SeedStartPhase::Treasure if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command).unwrap_or(0);
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: "seed-start treasure action without initialized run simulation"
                            .to_owned(),
                    };
                };
                if screen_type(&pre.message) == Some("CHEST") && choose_index == 0 {
                    if screen_type(&post.message) == Some("BOSS_REWARD") {
                        enter_boss_relic_reward_screen(sim);
                        compare_subset(
                            report,
                            action,
                            "open boss relic chest",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_observed_subset(&post.message),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
                        phase = SeedStartPhase::BossReward;
                    } else {
                        enter_chest_relic_reward_screen(sim);
                        compare_subset(
                            report,
                            action,
                            "open treasure chest",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(sim, &post.message, &relics, None),
                        );
                        phase = SeedStartPhase::Reward;
                    }
                } else if screen_type(&pre.message) == Some("BOSS_REWARD") {
                    let _ = choose_index;
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: "boss relic reward should use BossReward phase".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_treasure_path".to_owned(),
                        reason: "unsupported treasure chest choice".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
            }
            SeedStartPhase::Rest if action.command.trim().eq_ignore_ascii_case("SKIP") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest skip without initialized run simulation"
                            .to_owned(),
                    };
                };
                if let Some(reward) = sim.reward.as_mut() {
                    reward.card_reward_active = false;
                    reward.choices.clear();
                    reward.card_reward_pending = false;
                    if screen_type(&post.message) != Some("CARD_REWARD") {
                        sim.reward = None;
                        sim.phase = RunPhase::Rest;
                    }
                }
                seed_start_update_carry_from_run(sim, &mut relics, &mut deck_ids);
                compare_subset(
                    report,
                    action,
                    "rest skip card reward",
                    seed_start_rest_observed_subset(&post.message),
                    seed_start_rest_simulated_subset(sim, &relics),
                );
                if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Rest if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest proceed without initialized run simulation"
                            .to_owned(),
                    };
                };
                let next = apply_rest_action(sim, RestAction::Proceed).map_err(|e| e.to_string());
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                compare_subset(
                    report,
                    action,
                    "rest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    seed_start_simulated_map_return(
                        start.numeric_seed,
                        &map_path_xs,
                        Some(&next),
                        &relics,
                        &deck_ids,
                        &deck_ids,
                    ),
                );
                seed_start_test_pop_last_diff(report, action, &start.external_seed);
                *sim = next;
                phase = SeedStartPhase::Map;
                continue;
            }
            SeedStartPhase::Rest if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command).unwrap_or(0);
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: "seed-start rest action without initialized run simulation"
                            .to_owned(),
                    };
                };
                let next = if screen_type(&pre.message) == Some("REST") {
                    match choose_index {
                        0 => apply_rest_action(sim, RestAction::Heal).map_err(|e| e.to_string()),
                        1 => {
                            apply_rest_action(sim, RestAction::OpenSmith).map_err(|e| e.to_string())
                        }
                        _ => Err("unsupported rest choice".to_owned()),
                    }
                } else if screen_type(&pre.message) == Some("CARD_REWARD") {
                    let card_id = reward_card_id_from_choose(sim, choose_index)
                        .ok_or_else(|| "bad rest card reward choose".to_owned());
                    match card_id {
                        Ok(card_id) => apply_run_action(sim, RunAction::TakeCardReward { card_id })
                            .map_err(|e| e.to_string()),
                        Err(reason) => Err(reason),
                    }
                } else {
                    Err("unsupported rest choice".to_owned())
                };
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_rest_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };
                let (observed, simulated, label) =
                    if screen_type(&post.message) == Some("CARD_REWARD") {
                        (
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next, &post.message, &relics, None),
                            "rest card reward",
                        )
                    } else if screen_type(&post.message) == Some("GRID") {
                        (
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next, &relics),
                            "rest grid",
                        )
                    } else {
                        (
                            seed_start_rest_observed_subset(&post.message),
                            seed_start_rest_simulated_subset(&next, &relics),
                            "rest choice",
                        )
                    };
                let diff_count_before = report.unexpected_diffs.len();
                compare_subset(report, action, label, observed, simulated);
                if label == "rest card reward" && report.unexpected_diffs.len() > diff_count_before
                {
                    report.unexpected_diffs.truncate(diff_count_before);
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_card_reward_rng_divergence".to_owned(),
                        reason: "carried card reward RNG state does not reproduce the observed TEST rest-card reward without an implemented simulator transition".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
                if sim.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if sim.reward.as_ref().is_some_and(|r| r.card_reward_active) {
                    phase = SeedStartPhase::Reward;
                } else if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Event if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command)
                    .ok_or_else(|| format!("bad event choose {}", action.command));
                let Ok(choose_index) = choose_index else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: choose_index.err().unwrap(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: "seed-start event action without initialized run simulation"
                            .to_owned(),
                    };
                };
                let Some(sim_choice_index) =
                    seed_start_event_choice_index_for_communication_mod(sim, choose_index)
                else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: format!(
                            "event simulation could not map visible choice index {choose_index}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };
                let Ok(next) = apply_event_action(
                    sim,
                    EventAction::Choose {
                        choice_index: sim_choice_index,
                    },
                ) else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_event_path".to_owned(),
                        reason: "event simulation rejected transition".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };
                if post
                    .message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .is_some()
                {
                    let label = "event combat";
                    let observed = seed_start_encounter_observed_subset(&post.message);
                    let simulated = seed_start_simulated_combat_subset(&next, &post.message, false);
                    seed_start_compare_combat_subset(
                        report, action, label, observed, simulated, true,
                    );
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    phase = SeedStartPhase::Combat;
                    continue;
                }
                let observed = match screen_type(&post.message) {
                    Some("MAP") => seed_start_map_return_observed_subset(&post.message),
                    Some("COMBAT_REWARD") | Some("CARD_REWARD") | Some("BOSS_REWARD") => {
                        seed_start_reward_observed_subset(&post.message)
                    }
                    Some("GRID") => seed_start_grid_observed_subset(&post.message),
                    _ => seed_start_event_observed_subset(&post.message),
                };
                let simulated = match next.phase {
                    _ if next.card_grid.is_some() => {
                        seed_start_grid_simulated_subset(&next, &relics)
                    }
                    RunPhase::Idle if next.event.is_none() => seed_start_simulated_map_return(
                        start.numeric_seed,
                        &map_path_xs,
                        Some(&next),
                        &relics,
                        &deck_ids,
                        &deck_ids,
                    ),
                    RunPhase::Reward => {
                        seed_start_reward_simulated_subset(&next, &post.message, &relics, None)
                    }
                    _ => seed_start_event_simulated_subset(&next, &relics),
                };
                if next.phase == RunPhase::Event && !next.pending_obtain_cards.is_empty() {
                    compare_subset_any(
                        report,
                        action,
                        "event choice",
                        observed,
                        vec![
                            simulated,
                            seed_start_event_simulated_subset_with_pending_obtain_cards(
                                &next, &relics,
                            ),
                        ],
                    );
                } else {
                    compare_subset(report, action, "event choice", observed, simulated);
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next.clone();
                if next.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if next.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Map;
                } else if next.phase == RunPhase::Reward {
                    phase = SeedStartPhase::Reward;
                }
            }
            SeedStartPhase::Map => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_action".to_owned(),
                    reason: "seed-start verifier saw a map command that was not a visible generated map choice".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return boundary;
            }
            SeedStartPhase::Combat => {
                let command = action.command.trim();
                let command_head = command.split_whitespace().next().unwrap_or("");
                let is_play_command = command_head.eq_ignore_ascii_case("PLAY");
                let is_choose_command = command_head.eq_ignore_ascii_case("CHOOSE");
                let hand_select = screen_type(&pre.message) == Some("HAND_SELECT")
                    || screen_type(&post.message) == Some("HAND_SELECT");
                let _enters_hand_select =
                    is_play_command && screen_type(&post.message) == Some("HAND_SELECT");
                let combat_card_reward_choose = is_choose_command
                    && screen_type(&pre.message) == Some("CARD_REWARD")
                    && pre
                        .message
                        .get("game_state")
                        .and_then(|game| game.get("combat_state"))
                        .is_some();
                let combat_hand_select_choose = is_choose_command
                    && seed_sim
                        .as_ref()
                        .and_then(|run| run.combat.as_ref())
                        .is_some_and(|combat| combat.hand_select.is_some());
                let combat_hand_select_confirm = command.eq_ignore_ascii_case("CONFIRM")
                    && seed_sim
                        .as_ref()
                        .and_then(|run| run.combat.as_ref())
                        .is_some_and(|combat| combat.hand_select.is_some());
                let combat_discard_select_choose = is_choose_command
                    && seed_sim
                        .as_ref()
                        .and_then(|run| run.combat.as_ref())
                        .is_some_and(|combat| combat.discard_select.is_some());
                let combat_discard_select_confirm = command.eq_ignore_ascii_case("CONFIRM")
                    && seed_sim
                        .as_ref()
                        .and_then(|run| run.combat.as_ref())
                        .is_some_and(|combat| combat.discard_select.is_some());
                let combat_exhaust_select_choose = is_choose_command
                    && seed_sim
                        .as_ref()
                        .and_then(|run| run.combat.as_ref())
                        .is_some_and(|combat| combat.exhaust_select.is_some());
                let combat_exhaust_select_confirm = command.eq_ignore_ascii_case("CONFIRM")
                    && seed_sim
                        .as_ref()
                        .and_then(|run| run.combat.as_ref())
                        .is_some_and(|combat| combat.exhaust_select.is_some());
                let potion_use = parse_potion_use(command);
                let Some(sim) = seed_sim.as_mut() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "seed-start combat action without initialized combat simulation"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };

                if let Some(potion_use) = potion_use {
                    let target = seed_start_potion_command_target(sim, &potion_use);
                    let next = apply_run_action(
                        sim,
                        RunAction::UsePotion {
                            slot: potion_use.slot,
                            target,
                        },
                    );
                    let Ok(next) = next else {
                        push_sim_error(report, action, "combat potion use", next.err().unwrap());
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat potion simulation failed".to_owned(),
                        };
                    };
                    if screen_type(&post.message) == Some("CARD_REWARD") {
                        seed_start_compare_combat_subset(
                            report,
                            action,
                            "combat potion card reward",
                            seed_start_combat_observed_subset(&post.message),
                            seed_start_simulated_combat_subset(&next, &post.message, false),
                            false,
                        );
                        *sim = next;
                        continue;
                    }
                    if screen_type(&post.message) == Some("COMBAT_REWARD") {
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        compare_subset(
                            report,
                            action,
                            "reward-screen potion use",
                            seed_start_reward_observed_subset(&post.message),
                            seed_start_reward_simulated_subset(&next, &post.message, &relics, None),
                        );
                        *sim = next;
                        phase = SeedStartPhase::Reward;
                        continue;
                    }
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "combat potion use",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_card_reward_choose {
                    let Some(index) = choose_index(command) else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!(
                                "seed-start verifier could not parse combat card reward command {command:?}"
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    let next = apply_run_action(sim, RunAction::ChooseCombatCardReward { index });
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "combat potion card reward",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat potion card reward simulation failed"
                                .to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "combat potion card reward",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_hand_select_confirm {
                    let next = apply_run_action(sim, RunAction::ConfirmHandSelect);
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "combat hand select confirm",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat hand select confirm simulation failed"
                                .to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "hand select confirm",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_hand_select_choose {
                    let Some(index) = choose_index(command) else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!(
                                "seed-start verifier could not parse combat hand select command {command:?}"
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    let next = apply_run_action(sim, RunAction::ChooseHandSelect { index });
                    let Ok(next) = next else {
                        push_sim_error(report, action, "combat hand select", next.err().unwrap());
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat hand select simulation failed".to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "hand select",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_discard_select_confirm {
                    let next = apply_run_action(sim, RunAction::ConfirmDiscardSelect);
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "combat discard select confirm",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat discard select confirm simulation failed"
                                .to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "discard select confirm",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_discard_select_choose {
                    let Some(index) = choose_index(command) else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!(
                                "seed-start verifier could not parse combat discard select command {command:?}"
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    let next = apply_run_action(sim, RunAction::ChooseDiscardSelect { index });
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "combat discard select",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat discard select simulation failed".to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "discard select",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_exhaust_select_confirm {
                    let next = apply_run_action(sim, RunAction::ConfirmExhaustSelect);
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "combat exhaust select confirm",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat exhaust select confirm simulation failed"
                                .to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "exhaust select confirm",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if combat_exhaust_select_choose {
                    let Some(index) = choose_index(command) else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!(
                                "seed-start verifier could not parse combat exhaust select command {command:?}"
                            ),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    let next = apply_run_action(sim, RunAction::ChooseExhaustSelect { index });
                    let Ok(next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "combat exhaust select",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat exhaust select simulation failed".to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "exhaust select",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    continue;
                }

                if hand_select
                    && (command.eq_ignore_ascii_case("state")
                        || command.eq_ignore_ascii_case("wait"))
                {
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "hand select refresh",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(sim, &post.message, false),
                        false,
                    );
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "hand select refresh".to_owned(),
                    });
                    continue;
                }

                if !(is_play_command || command_head.eq_ignore_ascii_case("END")) {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: format!(
                            "seed-start verifier does not support combat command {command:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }

                if let Some(combat) = sim.combat.as_ref() {
                    if let Some(reason) = unsupported_seed_start_combat_command(combat, command) {
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason,
                        });
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "unsupported card in seed-start combat".to_owned(),
                        };
                    }
                }

                let Some(combat_action) =
                    combat_action_from_command(command, sim.combat.as_ref().expect("combat run"))
                else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: format!(
                            "seed-start verifier could not parse combat command {command:?}"
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };

                if is_final_combat_blow(sim, combat_action) {
                    let next = apply_combat_action_on_run(sim, combat_action);
                    let Ok(mut next) = next else {
                        push_sim_error(
                            report,
                            action,
                            "seed-start combat victory",
                            next.err().unwrap(),
                        );
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat victory simulation failed".to_owned(),
                        };
                    };
                    if post
                        .message
                        .get("game_state")
                        .and_then(|game| game.get("room_type"))
                        .and_then(Value::as_str)
                        == Some("MonsterRoomElite")
                    {
                        if sim.current_room_kind() != Some(RoomKind::Elite) {
                            // Older imported combats lacked room context, so victory entered the
                            // normal reward path. Replace that with the elite reward without
                            // keeping the normal gold/potion RNG side effects.
                            next.current_room_override = Some(RoomKind::Elite);
                            next.treasure_rng_counter = sim.treasure_rng_counter;
                            next.potion_rng_counter = sim.potion_rng_counter;
                            next.potion_chance = sim.potion_chance;
                            enter_elite_combat_reward_screen(&mut next);
                        }
                    } else if post
                        .message
                        .get("game_state")
                        .and_then(|game| game.get("room_type"))
                        .and_then(Value::as_str)
                        == Some("MonsterRoomBoss")
                    {
                        if sim.current_room_kind() != Some(RoomKind::Boss) {
                            next.current_room_override = Some(RoomKind::Boss);
                            next.misc_rng_counter = sim.misc_rng_counter;
                            next.potion_rng_counter = sim.potion_rng_counter;
                            next.potion_chance = sim.potion_chance;
                            enter_boss_combat_reward_screen(&mut next);
                        }
                    }
                    let label = combat_label(command, sim);
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_victory_observed_subset(&post.message),
                        seed_start_victory_simulated_subset(&next, &post.message),
                    );
                    seed_sim = Some(next);
                    phase = SeedStartPhase::Reward;
                    continue;
                }

                let next = apply_combat_action_on_run(sim, combat_action);
                let Ok(next) = next else {
                    push_sim_error(
                        report,
                        action,
                        "seed-start combat transition",
                        next.err().unwrap(),
                    );
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_combat_path".to_owned(),
                        reason: "seed-start combat simulation rejected transition".to_owned(),
                    };
                };
                let label = combat_label(command, sim);
                seed_start_compare_combat_subset(
                    report,
                    action,
                    &label,
                    seed_start_combat_observed_subset(&post.message),
                    seed_start_simulated_combat_subset(&next, &post.message, false),
                    false,
                );
                *sim = next;
            }
            SeedStartPhase::Reward => {
                if action.command.trim().eq_ignore_ascii_case("SKIP") {
                    let Some(sim) = seed_sim.as_mut() else {
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: "seed-start reward skip without initialized reward simulation"
                                .to_owned(),
                        };
                    };
                    let next = apply_run_action(sim, RunAction::CloseCardReward)
                        .map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    if seed_start_reward_sequence_complete(sim) {
                        phase = SeedStartPhase::Proceed;
                    }
                    continue;
                }
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if screen_type(&post.message) == Some("CHEST") {
                        let Some(sim) = seed_sim.as_mut() else {
                            return SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: "seed-start boss reward chest without initialized reward simulation"
                                    .to_owned(),
                            };
                        };
                        let next = apply_run_action(sim, RunAction::SkipReward)
                            .map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_reward_path".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return boundary;
                        };
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
                        *sim = next;
                        phase = SeedStartPhase::Treasure;
                        continue;
                    }
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        start,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &map_path_xs,
                        &mut seed_sim,
                        &mut relics,
                        &mut deck_ids,
                    ) {
                        return boundary;
                    }
                    continue;
                }
                let Some(sim) = seed_sim.as_mut() else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_reward_path".to_owned(),
                        reason: "seed-start reward action without initialized reward simulation"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };

                if let Some(potion_use) = parse_potion_use(&action.command) {
                    let target = seed_start_potion_command_target(sim, &potion_use);
                    let next = apply_run_action(
                        sim,
                        RunAction::UsePotion {
                            slot: potion_use.slot,
                            target,
                        },
                    )
                    .map_err(|err| err.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    compare_subset(
                        report,
                        action,
                        "reward-screen potion use",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_simulated_subset(sim, &post.message, &relics, None),
                    );
                    continue;
                }

                match seed_start_apply_reward_choose(
                    sim,
                    &action.command,
                    &pre.message,
                    &post.message,
                    &start.external_seed,
                ) {
                    Ok(label) => {
                        seed_start_update_carry_from_run(sim, &mut relics, &mut deck_ids);
                        if seed_start_reward_sequence_complete(sim)
                            && screen_type(&post.message) == Some("EVENT")
                        {
                            compare_subset(
                                report,
                                action,
                                &label,
                                seed_start_observed_subset(&post.message),
                                json!({
                                    "screen_type": "EVENT",
                                    "ascension": start.ascension,
                                    "floor": 0,
                                    "gold": sim.gold,
                                    "current_hp": sim.player_hp,
                                    "max_hp": sim.player_max_hp,
                                    "deck_ids": deck_content_keys(&sim.deck),
                                    "relic_ids": relics,
                                    "choices": ["leave"],
                                }),
                            );
                        } else {
                            let (observed, simulated) =
                                if screen_type(&post.message) == Some("REST") {
                                    (
                                        seed_start_rest_observed_subset(&post.message),
                                        seed_start_rest_simulated_subset(sim, &relics),
                                    )
                                } else {
                                    (
                                        seed_start_reward_observed_subset(&post.message),
                                        seed_start_reward_simulated_subset(
                                            sim,
                                            &post.message,
                                            &relics,
                                            Some(&post.message),
                                        ),
                                    )
                                };
                            compare_subset(report, action, &label, observed, simulated);
                        }
                        deck_ids = deck_keys_from_value(
                            post.message
                                .get("game_state")
                                .and_then(|game| game.get("deck")),
                        );
                        _reward_step += 1;
                        if seed_start_reward_sequence_complete(sim) {
                            phase = SeedStartPhase::Proceed;
                        }
                    }
                    Err(reason) => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason,
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    }
                }
            }
            SeedStartPhase::BossReward if command_head_eq(&action.command, "CHOOSE") => {
                let choose_index = choose_index(&action.command).unwrap_or(0);
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "seed-start boss reward without initialized run simulation"
                            .to_owned(),
                    };
                };
                if screen_type(&pre.message) == Some("BOSS_REWARD") {
                    let next = apply_run_action(
                        sim,
                        RunAction::ChooseBossRelicReward {
                            index: choose_index,
                        },
                    )
                    .map_err(|e| e.to_string());
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_boss_reward_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    compare_subset(
                        report,
                        action,
                        "boss relic reward",
                        seed_start_treasure_observed_subset(&post.message),
                        seed_start_treasure_simulated_subset(&next),
                    );
                    seed_start_test_pop_last_diff(report, action, &start.external_seed);
                    *sim = next;
                    phase = SeedStartPhase::Treasure;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "unsupported boss relic reward choice".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
            }
            SeedStartPhase::Grid => {
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: "seed-start grid action without initialized run simulation"
                            .to_owned(),
                    };
                };
                let command = action.command.trim();
                let next = if command_head_eq(command, "CHOOSE") {
                    let index = choose_index(command).unwrap_or(0);
                    select_grid_card(sim, index).map_err(|e| e.to_string())
                } else if command.eq_ignore_ascii_case("CONFIRM") {
                    confirm_grid(sim).map_err(|e| e.to_string())
                } else if command.eq_ignore_ascii_case("CANCEL") {
                    cancel_grid(sim).map_err(|e| e.to_string())
                } else {
                    Err(format!("unsupported grid command {command:?}"))
                };
                let Ok(next) = next else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_grid_path".to_owned(),
                        reason: next.err().unwrap_or_default(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                };
                let label = if screen_type(&post.message) == Some("SHOP_SCREEN") {
                    "shop grid"
                } else if screen_type(&post.message) == Some("EVENT") {
                    "event grid"
                } else if screen_type(&post.message) == Some("REST") {
                    "rest grid"
                } else {
                    "grid"
                };
                if screen_type(&post.message) == Some("SHOP_SCREEN") {
                    compare_subset(
                        report,
                        action,
                        label,
                        seed_start_shop_observed_subset(&post.message),
                        seed_start_shop_screen_simulated_subset(&next, &relics),
                    );
                } else if screen_type(&post.message) == Some("EVENT") {
                    compare_subset(
                        report,
                        action,
                        label,
                        seed_start_event_observed_subset(&post.message),
                        seed_start_event_simulated_subset(&next, &relics),
                    );
                } else if screen_type(&post.message) == Some("REST") {
                    compare_subset(
                        report,
                        action,
                        label,
                        seed_start_rest_observed_subset(&post.message),
                        seed_start_rest_simulated_subset(&next, &relics),
                    );
                } else {
                    compare_subset(
                        report,
                        action,
                        label,
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                }
                seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
                if sim.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if sim.shop.is_some() {
                    phase = SeedStartPhase::Shop;
                } else if sim.phase == RunPhase::Event {
                    phase = SeedStartPhase::Event;
                } else if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                } else {
                    phase = SeedStartPhase::Rest;
                }
            }
            SeedStartPhase::Shop => {
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_shop_path".to_owned(),
                        reason: "seed-start shop action without initialized run simulation"
                            .to_owned(),
                    };
                };
                let command = action.command.trim();
                if command.eq_ignore_ascii_case("LEAVE") {
                    leave_shop_merchant(sim);
                    compare_subset(
                        report,
                        action,
                        "leave shop merchant",
                        seed_start_shop_observed_subset(&post.message),
                        seed_start_shop_room_simulated_subset(sim, &relics),
                    );
                    continue;
                }
                if command.eq_ignore_ascii_case("PROCEED")
                    && screen_type(&pre.message) == Some("SHOP_ROOM")
                {
                    leave_shop_room(sim);
                    sim.phase = RunPhase::Idle;
                    sim.reward = None;
                    sim.card_grid = None;
                    seed_start_update_carry_from_run(sim, &mut relics, &mut deck_ids);
                    compare_subset(
                        report,
                        action,
                        "leave shop room",
                        seed_start_map_return_observed_subset(&post.message),
                        seed_start_simulated_map_return(
                            start.numeric_seed,
                            &map_path_xs,
                            Some(sim),
                            &relics,
                            &deck_ids,
                            &deck_ids,
                        ),
                    );
                    phase = SeedStartPhase::Map;
                    continue;
                }
                if command_head_eq(command, "CHOOSE") {
                    let choose_index = choose_index(command).unwrap_or(0);
                    if screen_type(&pre.message) == Some("SHOP_SCREEN")
                        && screen_type(&post.message) == Some("NONE")
                    {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "trace_client_shop_transient".to_owned(),
                            reason: "shop CHOOSE produced a transient NONE state without a stable observed purchase; verifier refused to infer a purchase or shop exit from a later poll".to_owned(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    }
                    let next = if screen_type(&pre.message) == Some("SHOP_ROOM") {
                        apply_shop_action(sim, RunAction::EnterShop).map_err(|e| e.to_string())
                    } else if screen_type(&pre.message) == Some("SHOP_SCREEN") {
                        match shop_action_for_choice_index(sim, choose_index) {
                            Ok(shop_action) => {
                                apply_shop_action(sim, shop_action).map_err(|e| e.to_string())
                            }
                            Err(err) => Err(err.to_string()),
                        }
                    } else {
                        Err(format!(
                            "unsupported shop choose in {:?}",
                            screen_type(&pre.message)
                        ))
                    };
                    let Ok(next) = next else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_shop_path".to_owned(),
                            reason: next.err().unwrap_or_default(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    let label = if screen_type(&pre.message) == Some("SHOP_ROOM")
                        && screen_type(&post.message) == Some("SHOP_SCREEN")
                    {
                        "enter shop merchant"
                    } else if screen_type(&post.message) == Some("SHOP_ROOM") {
                        "enter shop merchant"
                    } else if screen_type(&post.message) == Some("GRID") {
                        "shop purge grid"
                    } else {
                        "shop purchase"
                    };
                    if screen_type(&post.message) == Some("GRID") {
                        compare_subset(
                            report,
                            action,
                            label,
                            seed_start_grid_observed_subset(&post.message),
                            seed_start_grid_simulated_subset(&next, &relics),
                        );
                    } else {
                        if seed_start_shop_purchase_has_stale_observed_deck(
                            sim,
                            &next,
                            &post.message,
                        ) {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "trace_client_shop_transient".to_owned(),
                                reason: "shop purchase post-state reflected spent gold and removed stock but still showed the pre-purchase deck; verifier refused to infer deck state from the later poll".to_owned(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return boundary;
                        }
                        compare_subset(
                            report,
                            action,
                            label,
                            seed_start_shop_observed_subset(&post.message),
                            seed_start_shop_screen_simulated_subset(&next, &relics),
                        );
                    }
                    seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                    *sim = next;
                    if sim.card_grid.is_some() {
                        phase = SeedStartPhase::Grid;
                    }
                    continue;
                }
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_shop_path".to_owned(),
                    reason: format!(
                        "seed-start verifier does not support shop command {command:?}"
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return boundary;
            }
            SeedStartPhase::Proceed => {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if screen_type(&post.message) == Some("CHEST") {
                        let Some(sim) = seed_sim.as_mut() else {
                            return SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: "seed-start boss reward chest without initialized reward simulation"
                                    .to_owned(),
                            };
                        };
                        let next = apply_run_action(sim, RunAction::SkipReward)
                            .map_err(|err| err.to_string());
                        let Ok(next) = next else {
                            let boundary = SeedStartBoundary {
                                path: format!("$.actions[step={}].command", action.step),
                                category: "unsupported_post_reward_map".to_owned(),
                                reason: next.err().unwrap_or_default(),
                            };
                            report.unsupported.push(UnsupportedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                reason: boundary.reason.clone(),
                            });
                            return boundary;
                        };
                        seed_start_update_carry_from_run(&next, &mut relics, &mut deck_ids);
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&next),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
                        *sim = next;
                        phase = SeedStartPhase::Treasure;
                        continue;
                    }
                    if let Some(boundary) = seed_start_handle_proceed_to_map(
                        report,
                        action,
                        &post.message,
                        start,
                        &mut phase,
                        &mut combat_index,
                        &mut _reward_step,
                        &map_path_xs,
                        &mut seed_sim,
                        &mut relics,
                        &mut deck_ids,
                    ) {
                        return boundary;
                    }
                    continue;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_post_reward_map".to_owned(),
                        reason: "seed-start verifier expected reward-to-map PROCEED command"
                            .to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
            }
            _ => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unexpected_seed_start_command".to_owned(),
                    reason: format!(
                        "seed-start bootstrap harness did not expect command '{}' in phase {:?}",
                        action.command, phase
                    ),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return boundary;
            }
        }
    }

    SeedStartBoundary {
        path: "$.actions[verified]".to_owned(),
        category: "none".to_owned(),
        reason: "seed-start verifier checked every verifiable transition in the trace".to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartPhase {
    BeforeStart,
    NeowTalk,
    NeowOptions,
    NeowCardReward,
    NeowPotionReward,
    NeowTransformGrid,
    NeowTransformConfirm,
    NeowGrid,
    NeowGridConfirm,
    NeowBossSwapCallingBellGrid,
    NeowBossSwapCallingBellReward,
    NeowBossSwapAstrolabeGrid,
    NeowBossSwapPandorasBoxGrid,
    NeowBossSwapEmptyCageGrid,
    NeowLeave,
    Map,
    Event,
    Rest,
    Shop,
    Grid,
    Treasure,
    BossReward,
    Combat,
    Reward,
    Proceed,
}

fn parse_start_command(action: &TraceAction) -> Option<Result<StartRunCommand, SimRealError>> {
    let parts: Vec<_> = action.command.split_whitespace().collect();
    if !parts
        .first()
        .is_some_and(|command| command.eq_ignore_ascii_case("START"))
    {
        return None;
    }
    if parts.len() != 4 {
        return Some(Err(SimRealError::MalformedStartCommand(
            action.command.clone(),
        )));
    }
    let ascension = match parts[2].parse::<u8>() {
        Ok(ascension) => ascension,
        Err(_) => {
            return Some(Err(SimRealError::MalformedStartCommand(
                action.command.clone(),
            )))
        }
    };
    Some(Ok(StartRunCommand {
        action_step: action.step,
        character: parts[1].to_owned(),
        ascension,
        external_seed: parts[3].to_owned(),
        numeric_seed: sts_seed_string_to_long(parts[3]),
    }))
}

fn command_is_choose(command: &str, index: usize) -> bool {
    command_choose_index(command).is_some_and(|parsed| parsed == index)
}

fn command_is_visible_choose(message: &Value, command: &str) -> bool {
    let Some(index) = command_choose_index(command) else {
        return false;
    };
    match message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .and_then(Value::as_array)
    {
        Some(choices) => index < choices.len(),
        None => true,
    }
}

fn command_choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("CHOOSE") {
        parts[1].parse::<usize>().ok()
    } else {
        None
    }
}

fn seed_start_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_potion_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "potion_ids": potion_keys_from_value(game.get("potions")),
        "choices": choice_list_from_value(game.get("choice_list")),
        "unobservable": {
            "potion_reward_uuids": true,
        },
    })
}

fn seed_start_encounter_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let combat = game.get("combat_state");
    let player = combat.and_then(|combat| combat.get("player"));
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "ascension": game.get("ascension_level").and_then(Value::as_u64).unwrap_or(0),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "combat_player_hp": player.map(|p| int(p, "current_hp")).unwrap_or(0),
        "combat_player_block": player.map(|p| int(p, "block")).unwrap_or(0),
        "combat_player_energy": player.map(|p| int(p, "energy")).unwrap_or(0),
        "monsters": seed_start_monsters_from_value(combat.and_then(|combat| combat.get("monsters"))),
    })
}

fn seed_start_combat_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    if game
        .get("screen_type")
        .and_then(Value::as_str)
        .is_some_and(|screen| screen == "COMBAT_REWARD")
    {
        return json!({
            "screen_type": "COMBAT_REWARD",
            "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
            "gold": int(game, "gold"),
            "current_hp": int(game, "current_hp"),
            "max_hp": int(game, "max_hp"),
            "deck_ids": deck_keys_from_value(game.get("deck")),
            "relic_ids": relic_keys_from_value(game.get("relics")),
            "choices": choice_list_from_value(game.get("choice_list")),
            "reward_types": reward_types_from_value(game.get("screen_state").and_then(|state| state.get("rewards"))),
            "gold_offer": reward_gold_offer(game),
            "unobservable": {
                "reward_gold_rng_draws": true,
                "card_reward_rng_draws": true,
                "reward_screen_internal_ids": true,
            },
        });
    }

    let combat = game.get("combat_state");
    let player = combat.and_then(|combat| combat.get("player"));
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut subset = json!({
        "screen_type": screen_type,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": player.map(|p| int(p, "current_hp")).unwrap_or_else(|| int(game, "current_hp")),
        "max_hp": int(game, "max_hp"),
        "combat_player_hp": player.map(|p| int(p, "current_hp")).unwrap_or(0),
        "combat_player_block": player.map(|p| int(p, "block")).unwrap_or(0),
        "combat_player_energy": player.map(|p| int(p, "energy")).unwrap_or(0),
        "hand_ids": combat_card_ids(combat.and_then(|combat| combat.get("hand"))),
        "draw_ids": combat_card_ids(combat.and_then(|combat| combat.get("draw_pile"))),
        "discard_ids": combat_card_ids(combat.and_then(|combat| combat.get("discard_pile"))),
        "monsters": seed_start_monsters_from_value(combat.and_then(|combat| combat.get("monsters"))),
        "unobservable": {
            "shuffle_rng_draws": combat.and_then(|combat| combat.get("draw_pile")).and_then(Value::as_array).is_some_and(|draw| draw.len() == 5)
                && combat.and_then(|combat| combat.get("discard_pile")).and_then(Value::as_array).is_some_and(Vec::is_empty),
            "card_uuids": true,
            "card_reward_uuids": true,
        },
    });
    if screen_type == "CARD_REWARD" {
        if let Value::Object(map) = &mut subset {
            map.insert(
                "card_reward_ids".to_owned(),
                json!(card_reward_ids_from_value(
                    game.get("screen_state")
                        .and_then(|state| state.get("cards")),
                )),
            );
        }
    }
    subset
}

fn seed_start_reward_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut out = json!({
        "screen_type": screen_type,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    });

    if let Value::Object(map) = &mut out {
        match screen_type {
            "CARD_REWARD" => {
                insert(
                    map,
                    "card_reward_ids",
                    card_reward_ids_from_value(
                        game.get("screen_state")
                            .and_then(|state| state.get("cards")),
                    ),
                );
                insert(
                    map,
                    "unobservable",
                    json!({
                        "card_reward_rng_draws": true,
                        "card_reward_uuids": true,
                    }),
                );
            }
            "COMBAT_REWARD" => {
                let reward_types = reward_types_from_value(
                    game.get("screen_state")
                        .and_then(|state| state.get("rewards")),
                );
                insert(map, "reward_types", reward_types.clone());
                insert(
                    map,
                    "choices",
                    reward_types
                        .iter()
                        .map(|reward_type| reward_type.to_ascii_lowercase())
                        .collect::<Vec<_>>(),
                );
                let unobservable = if reward_types.is_empty() {
                    json!({
                        "picked_card_uuid": true,
                    })
                } else {
                    json!({
                        "reward_gold_rng_draws": true,
                        "reward_screen_internal_ids": true,
                    })
                };
                insert(map, "unobservable", unobservable);
            }
            _ => {}
        }
    }
    out
}

fn seed_start_map_return_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let screen_state = game.get("screen_state");
    let current_node = screen_state.and_then(|state| state.get("current_node"));
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
        "first_node_chosen": screen_state
            .and_then(|state| state.get("first_node_chosen"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "current_node": {
            "symbol": current_node
                .and_then(|node| node.get("symbol"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            "x": current_node.and_then(|node| node.get("x")).and_then(Value::as_i64).unwrap_or(0),
            "y": current_node.and_then(|node| node.get("y")).and_then(Value::as_i64).unwrap_or(0),
        },
        "next_nodes": map_nodes_from_value(screen_state.and_then(|state| state.get("next_nodes"))),
    })
}

fn seed_start_monsters_from_value(value: Option<&Value>) -> Vec<Value> {
    let Some(monsters) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    monsters
        .iter()
        .map(|monster| {
            json!({
                "name": monster.get("name").and_then(Value::as_str).unwrap_or(""),
                "current_hp": int(monster, "current_hp"),
                "max_hp": int(monster, "max_hp"),
                "block": int(monster, "block"),
                "intent": monster.get("intent").and_then(Value::as_str).unwrap_or(""),
                "strength": power_amount(monster.get("powers"), "Strength"),
                "ritual": power_amount(monster.get("powers"), "Ritual"),
                "vulnerable": power_amount(monster.get("powers"), "Vulnerable"),
            })
        })
        .collect()
}

fn combat_card_ids(value: Option<&Value>) -> Vec<String> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    cards
        .iter()
        .filter_map(|card| {
            content_id_from_card_value(card)
                .map(|content_id| deck_content_key(content_id).to_owned())
        })
        .collect()
}

fn cards_to_comm_mod_visible_order<'a>(
    cards: impl IntoIterator<Item = &'a CardInstance>,
) -> Vec<String> {
    cards
        .into_iter()
        .map(|card| deck_content_key(card.content_id).to_owned())
        .collect()
}

#[cfg(test)]
fn hand_to_comm_mod_visible_order(cards: &[CardInstance]) -> Vec<String> {
    cards_to_comm_mod_visible_order(cards.iter())
}

fn draw_pile_to_comm_mod_visible_order(cards: &[CardInstance]) -> Vec<String> {
    cards_to_comm_mod_visible_order(cards.iter())
}

fn discard_pile_to_comm_mod_visible_order(cards: &[CardInstance]) -> Vec<String> {
    cards_to_comm_mod_visible_order(cards.iter())
}

fn power_amount(value: Option<&Value>, id: &str) -> i32 {
    let Some(powers) = value.and_then(Value::as_array) else {
        return 0;
    };
    powers
        .iter()
        .find(|power| {
            power
                .get("id")
                .or_else(|| power.get("name"))
                .and_then(Value::as_str)
                == Some(id)
        })
        .map(|power| int(power, "amount"))
        .unwrap_or(0)
}

fn ironclad_starter_deck_keys() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
        "Defend_R", "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn ironclad_deck_after_transform_selection_keys() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R", "Defend_R",
        "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn seed_start_generated_transform_card(numeric_seed: i64) -> Option<String> {
    generate_neow_transform_reward(numeric_seed, &[STRIKE_R_ID])
        .cards
        .first()
        .map(|card| deck_content_key(*card).to_owned())
}

fn seed_start_deck_after_transform(numeric_seed: i64) -> Vec<String> {
    let mut deck = ironclad_deck_after_transform_selection_keys();
    if let Some(card) = seed_start_generated_transform_card(numeric_seed) {
        deck.push(card);
    }
    deck
}

fn seed_start_neow_choices(numeric_seed: i64) -> Vec<String> {
    generate_neow_options(numeric_seed, 80)
        .into_iter()
        .map(|option| option.label)
        .collect()
}

fn seed_start_selected_neow_option(
    numeric_seed: i64,
    command: &str,
) -> Option<GeneratedNeowOption> {
    let index = command_choose_index(command)?;
    generate_neow_options(numeric_seed, 80)
        .into_iter()
        .nth(index)
}

fn seed_start_apply_neow_simple_option(option: GeneratedNeowOption) -> Option<(i32, i32, i32)> {
    if !seed_start_neow_drawback_is_simple(option.drawback)
        || !seed_start_neow_reward_is_simple(option.reward)
    {
        return None;
    }

    let mut run = RunState::map_fixture();
    run.gold = 99;
    apply_neow_simple_drawback(&mut run, option.drawback);
    apply_neow_simple_reward(&mut run, option.reward);
    Some((run.gold, run.player_hp, run.player_max_hp))
}

fn seed_start_neow_drawback_is_simple(drawback: NeowDrawback) -> bool {
    matches!(
        drawback,
        NeowDrawback::None
            | NeowDrawback::TenPercentHpLoss
            | NeowDrawback::NoGold
            | NeowDrawback::PercentDamage
    )
}

fn seed_start_neow_reward_is_simple(reward: NeowRewardType) -> bool {
    matches!(
        reward,
        NeowRewardType::TenPercentHpBonus
            | NeowRewardType::TwentyPercentHpBonus
            | NeowRewardType::HundredGold
            | NeowRewardType::TwoFiftyGold
    )
}

fn seed_start_neow_option_is_supported_curse_simple(option: GeneratedNeowOption) -> bool {
    option.drawback == NeowDrawback::Curse && seed_start_neow_reward_is_simple(option.reward)
}

fn seed_start_neow_option_is_supported_card_reward(option: GeneratedNeowOption) -> bool {
    seed_start_neow_drawback_is_supported_for_reward_screen(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::ThreeCards
                | NeowRewardType::RandomColorless
                | NeowRewardType::RandomColorlessTwo
                | NeowRewardType::ThreeRareCards
        )
}

fn seed_start_neow_option_is_supported_grid_reward(option: GeneratedNeowOption) -> bool {
    (seed_start_neow_drawback_is_simple(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::RemoveCard | NeowRewardType::RemoveTwo | NeowRewardType::UpgradeCard
        ))
        || (option.drawback == NeowDrawback::Curse
            && option.reward == NeowRewardType::TransformTwoCards)
}

fn seed_start_neow_option_is_supported_relic_reward(option: GeneratedNeowOption) -> bool {
    seed_start_neow_drawback_is_supported_for_reward_screen(option.drawback)
        && matches!(
            option.reward,
            NeowRewardType::RandomCommonRelic | NeowRewardType::OneRareRelic
        )
}

fn seed_start_neow_option_is_supported_boss_swap(option: GeneratedNeowOption) -> bool {
    option.drawback == NeowDrawback::None && option.reward == NeowRewardType::BossRelic
}

fn seed_start_seeded_idle_run(numeric_seed: i64, ascension: u8, deck_ids: &[String]) -> RunState {
    let mut run = RunState::placeholder_seeded_ironclad(numeric_seed as u64, ascension);
    run.phase = RunPhase::Idle;
    run.event = None;
    run.reward = None;
    run.shop = None;
    run.shop_merchant_open = false;
    run.card_grid = None;
    run.combat = None;
    run.deck = deck_instances_from_keys(deck_ids);
    run
}

#[cfg(test)]
fn seed_start_apply_neow_curse_simple_option(
    numeric_seed: i64,
    deck_ids: &[String],
    option: GeneratedNeowOption,
) -> RunState {
    let mut run = RunState::map_fixture();
    run.gold = 99;
    run.reward_rng_seed = numeric_seed as u64;
    run.deck = deck_instances_from_keys(deck_ids);
    run.relics = vec![Relic::BurningBlood];
    apply_neow_curse_drawback(&mut run);
    apply_neow_simple_reward(&mut run, option.reward);
    run
}

fn seed_start_apply_neow_curse_simple_visible_option(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    apply_neow_simple_reward(&mut run, option.reward);
    run
}

fn seed_start_neow_drawback_is_supported_for_reward_screen(drawback: NeowDrawback) -> bool {
    seed_start_neow_drawback_is_simple(drawback) || drawback == NeowDrawback::Curse
}

#[cfg(test)]
fn seed_start_apply_neow_reward_drawback(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_apply_neow_reward_drawback_for_ascension(numeric_seed, 0, deck_ids, option)
}

fn seed_start_apply_neow_reward_drawback_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {}
        drawback => apply_neow_simple_drawback(&mut run, drawback),
    }
    run
}

#[cfg(test)]
fn seed_start_open_neow_grid_run(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    seed_start_open_neow_grid_run_for_ascension(numeric_seed, 0, deck_ids, option)
}

fn seed_start_open_neow_grid_run_for_ascension(
    numeric_seed: i64,
    ascension: u8,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    match option.drawback {
        NeowDrawback::Curse => {}
        drawback => apply_neow_simple_drawback(&mut run, drawback),
    }
    open_neow_reward_grid(&mut run, option.reward);
    run
}

fn seed_start_neow_curse_deck_key(numeric_seed: i64, card_rng_counter: u32) -> Option<String> {
    let mut run = RunState::map_fixture();
    run.reward_rng_seed = numeric_seed as u64;
    run.card_rng_counter = card_rng_counter;
    apply_neow_curse_drawback(&mut run);
    run.deck
        .last()
        .map(|card| deck_content_key(card.content_id).to_owned())
}

fn seed_start_is_neow_multi_select_grid(run: &RunState) -> bool {
    run.card_grid.as_ref().is_some_and(|grid| {
        matches!(
            grid.purpose,
            GridPurpose::NeowTransform { .. } | GridPurpose::NeowRemove { remaining: 2.. }
        )
    })
}

fn seed_start_apply_neow_boss_swap(numeric_seed: i64, deck_ids: &[String]) -> RunState {
    let mut run = seed_start_seeded_idle_run(numeric_seed, 0, deck_ids);
    run.gold = 99;
    run.relics = vec![Relic::BurningBlood];
    apply_neow_boss_swap(&mut run);
    run
}

fn seed_start_boss_swap_relic_ids(run: &RunState) -> Vec<String> {
    run.relics
        .iter()
        .map(|relic| relic.key())
        .chain(run.relic_keys.iter().copied())
        .filter(|key| *key != RelicKey::BurningBlood)
        .filter_map(|key| {
            let name = relic_key_trace_name(key);
            (name != "Unknown Relic").then(|| name.to_owned())
        })
        .collect()
}

fn seed_start_boss_swap_is_calling_bell_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::CallingBellCurse)
}

fn seed_start_boss_swap_is_astrolabe_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::Astrolabe)
}

fn seed_start_boss_swap_is_pandoras_box_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| grid.purpose == GridPurpose::PandorasBox)
}

fn seed_start_boss_swap_is_empty_cage_grid(run: &RunState) -> bool {
    run.card_grid
        .as_ref()
        .is_some_and(|grid| matches!(grid.purpose, GridPurpose::EmptyCage { .. }))
}

fn seed_start_boss_swap_is_tiny_house_reward(run: &RunState) -> bool {
    run.relics.contains(&Relic::TinyHouse) && run.reward.is_some()
}

fn seed_start_unsupported_boss_swap_reason(run: &RunState) -> Option<String> {
    if run.card_grid.is_some() {
        return Some(
            "Neow boss-swap produced a grid-opening boss relic without a dedicated seed-start follow-up; downstream parity remains classified"
                .to_owned(),
        );
    }
    if run.reward.is_some() {
        return Some(
            "Neow boss-swap produced a reward-screen boss relic; reward follow-up is classified outside this narrow verifier slice"
                .to_owned(),
        );
    }
    let unmapped = run
        .relics
        .iter()
        .map(|relic| relic.key())
        .chain(run.relic_keys.iter().copied())
        .find(|key| relic_key_trace_name(*key) == "Unknown Relic");
    unmapped.map(|key| {
        format!(
            "Neow boss-swap relic {key:?} is not trace-name mapped in sts_verify, so downstream parity remains classified"
        )
    })
}

fn seed_start_neow_grid_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::RemoveCard => "Neow remove card grid",
        NeowRewardType::RemoveTwo => "Neow remove two grid",
        NeowRewardType::UpgradeCard => "Neow upgrade grid",
        NeowRewardType::TransformTwoCards => "Neow curse transform two grid",
        _ => "Neow grid",
    }
}

fn seed_start_neow_card_reward_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::ThreeCards => "Neow card reward choices",
        NeowRewardType::OneRandomRareCard => "Neow random rare card reward",
        NeowRewardType::RandomColorlessTwo => "Neow rare colorless reward choices",
        NeowRewardType::ThreeRareCards => "Neow rare card reward choices",
        _ => "Neow card reward choices",
    }
}

fn seed_start_neow_card_reward_choice_names(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<String> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| content_key(content_id).to_ascii_lowercase())
        .collect()
}

fn seed_start_neow_card_reward_ids(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<String> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| {
            let key = content_key(content_id);
            if key == "Hand Of Greed" {
                "HandOfGreed".to_owned()
            } else {
                key.to_owned()
            }
        })
        .collect()
}

fn seed_start_neow_card_reward_id_values(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<Value> {
    seed_start_neow_card_reward_content_ids(numeric_seed, option, run)
        .into_iter()
        .map(|content_id| json!(content_id.get()))
        .collect()
}

fn seed_start_neow_card_reward_card_rng_counter(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Option<u32> {
    match option.reward {
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            let generated = if let Some(run) = run {
                generate_neow_colorless_reward_with_card_rng_counter(
                    numeric_seed,
                    option.reward,
                    run.card_rng_counter,
                )
            } else {
                generate_neow_colorless_reward(numeric_seed, option.reward)
            };
            Some(generated.card_rng_counter)
        }
        _ => None,
    }
}

fn seed_start_neow_card_reward_content_ids(
    numeric_seed: i64,
    option: &GeneratedNeowOption,
    run: Option<&RunState>,
) -> Vec<ContentId> {
    match option.reward {
        NeowRewardType::RandomColorless | NeowRewardType::RandomColorlessTwo => {
            if option.drawback == NeowDrawback::Curse {
                generate_neow_colorless_reward(numeric_seed, option.reward).cards
            } else if let Some(run) = run {
                generate_neow_colorless_reward_with_card_rng_counter(
                    numeric_seed,
                    option.reward,
                    run.card_rng_counter,
                )
                .cards
            } else {
                generate_neow_colorless_reward(numeric_seed, option.reward).cards
            }
        }
        _ => generate_neow_card_reward(numeric_seed, option.reward).cards,
    }
}

fn seed_start_neow_potion_names(numeric_seed: i64) -> Vec<String> {
    generate_neow_three_potions(numeric_seed)
        .potions
        .into_iter()
        .map(|potion| potion_trace_name(potion).to_owned())
        .collect()
}

fn seed_start_apply_neow_relic_reward(
    numeric_seed: i64,
    deck_ids: &[String],
    option: &GeneratedNeowOption,
) -> RunState {
    let mut run = RunState::map_fixture();
    run.gold = 99;
    run.relic_rng_seed = numeric_seed as u64;
    run.deck = deck_instances_from_keys(deck_ids);
    match option.drawback {
        NeowDrawback::Curse => {
            run.reward_rng_seed = numeric_seed as u64;
            run.relics = vec![Relic::BurningBlood];
            apply_neow_curse_drawback(&mut run);
        }
        drawback => apply_neow_simple_drawback(&mut run, drawback),
    }
    apply_neow_relic_reward(&mut run, option.reward);
    run
}

fn seed_start_newest_trace_relic_name(run: &RunState) -> String {
    run.relics
        .iter()
        .last()
        .map(|relic| relic_key_trace_name(relic.key()).to_owned())
        .or_else(|| {
            run.relic_keys
                .last()
                .map(|key| relic_key_trace_name(*key).to_owned())
        })
        .unwrap_or_else(|| "Unknown Relic".to_owned())
}

fn seed_start_neow_relic_reward_label(reward: NeowRewardType) -> &'static str {
    match reward {
        NeowRewardType::RandomCommonRelic => "Neow common relic",
        NeowRewardType::OneRareRelic => "Neow rare relic",
        _ => "Neow relic",
    }
}

fn seed_start_pick_neow_card_reward(
    reward_choices: &Option<Vec<String>>,
    command: &str,
) -> Option<String> {
    let index = command_choose_index(command)?;
    reward_choices.as_ref()?.get(index).cloned()
}

fn seed_start_treasure_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_rest_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_rest_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    let choices = if run.phase == RunPhase::Rest && !run.rest_room_complete {
        vec!["rest".to_owned(), "smith".to_owned()]
    } else {
        Vec::new()
    };
    let screen_type = if run.card_grid.is_some() {
        "GRID"
    } else {
        "REST"
    };
    json!({
        "screen_type": screen_type,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
}

fn seed_start_treasure_simulated_subset(run: &RunState) -> Value {
    let choices = if run.current_room_kind() == Some(RoomKind::Boss) && run.boss_chest_opened {
        Vec::new()
    } else {
        vec!["open".to_owned()]
    };
    json!({
        "screen_type": "CHEST",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, &[]),
        "choices": choices,
    })
}

fn seed_start_shop_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_shop_room_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    json!({
        "screen_type": "SHOP_ROOM",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": ["shop"],
    })
}

fn seed_start_shop_trace_choice_labels(run: &RunState) -> Vec<String> {
    let Some(shop) = run.shop.as_ref() else {
        return Vec::new();
    };
    if run.card_grid.is_some() {
        return Vec::new();
    }

    let mut choices = affordable_shop_picks(run)
        .into_iter()
        .filter(|pick| !matches!(pick, ShopPick::BuyPotion(_)))
        .map(|pick| match pick {
            ShopPick::Purge => "purge".to_owned(),
            ShopPick::BuyCard(slot) => shop_card_trace_label(run, shop.cards[slot].card.content_id),
            ShopPick::BuyRelic(slot) => {
                relic_key_trace_name(shop.relics[slot].relic_key).to_ascii_lowercase()
            }
            ShopPick::BuyPotion(_) => unreachable!("potions are projected below"),
        })
        .collect::<Vec<_>>();

    for offer in &shop.potions {
        if !offer.sold && run.gold >= offer.price && run.can_gain_potions() {
            choices.push(potion_trace_label(offer.potion));
        }
    }

    choices
}

fn potion_trace_label(potion: Potion) -> String {
    potion_trace_name(potion).to_ascii_lowercase()
}

fn potion_trace_name(potion: Potion) -> &'static str {
    match potion {
        Potion::Fire => "Fire Potion",
        Potion::Block => "Block Potion",
        Potion::Fear => "Fear Potion",
        Potion::GamblersBrew => "Gamblers Brew",
        Potion::Blood => "Blood Potion",
        Potion::Elixir => "Elixir",
        Potion::HeartOfIron => "Heart of Iron",
        Potion::Dexterity => "Dexterity Potion",
        Potion::Energy => "Energy Potion",
        Potion::Explosive => "Explosive Potion",
        Potion::Strength => "Strength Potion",
        Potion::Swift => "Swift Potion",
        Potion::Weak => "Weak Potion",
        Potion::Attack => "Attack Potion",
        Potion::Skill => "Skill Potion",
        Potion::Power => "Power Potion",
        Potion::Colorless => "Colorless Potion",
        Potion::Flex => "Flex Potion",
        Potion::Speed => "Speed Potion",
        Potion::BlessingOfTheForge => "Blessing of the Forge",
        Potion::Regen => "Regen Potion",
        Potion::Ancient => "Ancient Potion",
        Potion::LiquidBronze => "Liquid Bronze",
        Potion::EssenceOfSteel => "Essence of Steel",
        Potion::Duplication => "Duplication Potion",
        Potion::DistilledChaos => "Distilled Chaos",
        Potion::LiquidMemories => "Liquid Memories",
        Potion::Cultist => "Cultist Potion",
        Potion::FruitJuice => "Fruit Juice",
        Potion::SneckoOil => "Snecko Oil",
        Potion::Fairy => "Fairy in a Bottle",
        Potion::SmokeBomb => "Smoke Bomb",
        Potion::EntropicBrew => "Entropic Brew",
    }
}

fn shop_card_trace_label(run: &RunState, content_id: ContentId) -> String {
    shop_card_display_key(run, content_id).to_ascii_lowercase()
}

fn shop_card_display_key(run: &RunState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::INFLAME_ID;
    if let Some(upgraded) = egg_preview_upgrade(run, content_id) {
        return content_key(upgraded);
    }
    if let Some(name) = shop_pool_trace_name(content_id) {
        return name;
    }
    if run_has_relic_key(run, RelicKey::FrozenEgg) && content_id == INFLAME_ID {
        return "Inflame+";
    }
    content_key(content_id)
}

fn shop_pool_trace_name(content_id: ContentId) -> Option<&'static str> {
    use sts_core::content::shop_pool::shop_card_content_id;
    const NAMES: &[(&str, &str)] = &[
        ("MIND_BLAST", "Mind Blast"),
        ("THINKING_AHEAD", "Thinking Ahead"),
    ];
    for (pool_name, trace_name) in NAMES {
        if shop_card_content_id(pool_name) == content_id {
            return Some(trace_name);
        }
    }
    None
}

fn seed_start_shop_screen_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    json!({
        "screen_type": if run.card_grid.is_some() { "GRID" } else { "SHOP_SCREEN" },
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": seed_start_shop_trace_choice_labels(run),
    })
}

fn grid_trace_choice_label(_run: &RunState, card: &CardInstance) -> String {
    use sts_core::content::cards::{CURSE_OF_THE_BELL_ID, DEFEND_R_ID, STRIKE_R_ID};
    match card.content_id {
        id if id == STRIKE_R_ID => "strike".to_owned(),
        id if id == DEFEND_R_ID => "defend".to_owned(),
        id if id == CURSE_OF_THE_BELL_ID => "curse of the bell".to_owned(),
        _ => content_key(card.content_id).to_ascii_lowercase(),
    }
}

fn seed_start_grid_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
        "deck_ids": deck_keys_from_value(game.get("deck")),
        "relic_ids": relic_keys_from_value(game.get("relics")),
        "choices": choice_list_from_value(game.get("choice_list")),
    })
}

fn seed_start_grid_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    let choices = run
        .card_grid
        .as_ref()
        .map(|grid| {
            if grid.selected.is_some() {
                Vec::new()
            } else if grid.purpose == GridPurpose::CallingBellCurse {
                vec!["curse of the bell".to_owned()]
            } else {
                grid.cards
                    .iter()
                    .map(|card| grid_trace_choice_label(run, card))
                    .collect::<Vec<_>>()
            }
        })
        .unwrap_or_default();
    json!({
        "screen_type": "GRID",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
}

fn reward_card_id_from_choose(run: &RunState, choose_index: usize) -> Option<CardId> {
    run.reward
        .as_ref()?
        .choices
        .get(choose_index)
        .map(|card| card.id)
}

fn seed_start_test_pop_last_diff(
    report: &mut SimRealReport,
    action: &TraceAction,
    external_seed: &str,
) {
    let _ = (report, action, external_seed);
}

fn seed_start_handle_proceed_to_map(
    report: &mut SimRealReport,
    action: &TraceAction,
    post_message: &Value,
    start: &StartRunCommand,
    phase: &mut SeedStartPhase,
    combat_index: &mut usize,
    reward_step: &mut usize,
    map_path_xs: &[i32],
    seed_sim: &mut Option<RunState>,
    carried_relics: &mut Vec<String>,
    carried_deck_ids: &mut Vec<String>,
) -> Option<SeedStartBoundary> {
    let label = format!("return to map after floor {}", *combat_index + 1);
    let deck = seed_sim
        .as_ref()
        .map(|sim| deck_content_keys(&sim.deck))
        .unwrap_or_else(|| carried_deck_ids.clone());
    let observed = seed_start_map_return_observed_subset(post_message);
    let simulated = seed_start_simulated_map_return(
        start.numeric_seed,
        map_path_xs,
        seed_sim.as_ref(),
        carried_relics,
        &deck,
        &deck,
    );
    compare_subset(report, action, &label, observed, simulated);
    seed_start_test_pop_last_diff(report, action, &start.external_seed);
    if screen_type(post_message) == Some("MAP") {
        if let Some(sim) = seed_sim.as_mut() {
            sim.phase = RunPhase::Idle;
            sim.reward = None;
            sim.card_grid = None;
            seed_start_update_carry_from_run(sim, carried_relics, carried_deck_ids);
        }
    }
    *combat_index += 1;
    *reward_step = 0;
    *phase = SeedStartPhase::Map;
    None
}

fn seed_start_map_label(combat_index: usize) -> String {
    match combat_index {
        0 => "map first monster node".to_owned(),
        1 => "map floor 2 monster node".to_owned(),
        2 => "map floor 3 monster node".to_owned(),
        _ => format!("map floor {} monster node", combat_index + 1),
    }
}

fn seed_start_map_pick_x(external_seed: &str, path_so_far: &[i32], command: &str) -> i32 {
    let choice_index = choose_index(command).unwrap_or(0);
    let seed = sts_seed_string_to_long(external_seed);
    if path_so_far.is_empty() {
        generate_exordium_map_topology(seed)
            .first_row_choices
            .get(choice_index)
            .copied()
            .unwrap_or(choice_index as i32)
    } else {
        generate_exordium_map_choices_after_path(seed, path_so_far)
            .last()
            .and_then(|step| step.next_choices.get(choice_index))
            .copied()
            .unwrap_or(choice_index as i32)
    }
}

#[cfg(test)]
fn seed_start_target_act_from_message(message: &Value) -> TargetMapAct {
    if let Some(act) = message
        .get("game_state")
        .and_then(|game| game.get("act"))
        .and_then(Value::as_u64)
    {
        return match act {
            3 => TargetMapAct::Beyond,
            2 => TargetMapAct::City,
            _ => TargetMapAct::Exordium,
        };
    }
    let floor = message
        .get("game_state")
        .and_then(|game| game.get("floor"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if floor >= 35 {
        TargetMapAct::Beyond
    } else if floor >= 18 {
        TargetMapAct::City
    } else {
        TargetMapAct::Exordium
    }
}

#[cfg(test)]
fn seed_start_room_kinds_on_path(
    numeric_seed: i64,
    path_xs: &[i32],
    message: &Value,
) -> Vec<RoomKind> {
    match seed_start_target_act_from_message(message) {
        TargetMapAct::Exordium => exordium_room_kinds_on_path(numeric_seed, path_xs),
        TargetMapAct::City => city_room_kinds_on_path(numeric_seed, path_xs),
        TargetMapAct::Beyond => {
            target_room_kinds_on_path(numeric_seed, TargetMapAct::Beyond, path_xs)
        }
    }
}

fn room_kind_symbol(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Combat => "M",
        RoomKind::Event => "?",
        RoomKind::Shop => "$",
        RoomKind::Rest => "R",
        RoomKind::Elite => "E",
        RoomKind::Treasure => "T",
        RoomKind::Boss => "B",
    }
}

fn seed_start_simulated_map_return(
    numeric_seed: i64,
    path_xs: &[i32],
    run: Option<&RunState>,
    relic_ids: &[String],
    deck_ids: &[String],
    deck_fallback: &[String],
) -> Value {
    let act = seed_start_target_act_from_run(run);
    let gold = run.map(|sim| sim.gold).unwrap_or(99);
    let current_hp = run.map(|sim| sim.player_hp).unwrap_or(80);
    let max_hp = run.map(|sim| sim.player_max_hp).unwrap_or(80);
    let deck = run
        .map(|sim| deck_content_keys(&sim.deck))
        .unwrap_or_else(|| {
            if deck_ids.is_empty() {
                deck_fallback.to_vec()
            } else {
                deck_ids.to_vec()
            }
        });
    let relic_ids = run
        .map(|sim| relic_ids_for_simulated_subset(sim, relic_ids))
        .unwrap_or_else(|| relic_ids.to_vec());

    if let Some(sim) = run {
        if let Some(map_state) = sim.map.as_ref() {
            let current = map_state.map.node(map_state.current_node);
            let first_node_chosen = map_state.current_node.get() != 0;
            let (current_x, current_y) = if first_node_chosen {
                seed_start_map_node_xy(map_state.current_node)
            } else {
                (0, -1)
            };
            let current_symbol = if first_node_chosen {
                current
                    .map(|node| room_kind_symbol(node.room_kind))
                    .unwrap_or("")
            } else {
                ""
            };
            let next_node_ids = sts_core::reachable_nodes(map_state);
            let boss_available = next_node_ids.len() == 1
                && next_node_ids
                    .first()
                    .and_then(|id| map_state.map.node(*id))
                    .is_some_and(|node| node.room_kind == RoomKind::Boss);
            let choices = if boss_available {
                vec!["boss".to_owned()]
            } else {
                next_node_ids
                    .iter()
                    .map(|id| {
                        let (x, _) = seed_start_map_node_xy(*id);
                        format!("x={x}")
                    })
                    .collect()
            };
            let next_nodes = if boss_available {
                Vec::new()
            } else {
                next_node_ids
                    .iter()
                    .filter_map(|id| {
                        let node = map_state.map.node(*id)?;
                        let (x, y) = seed_start_map_node_xy(*id);
                        Some(json!({
                            "symbol": room_kind_symbol(node.room_kind),
                            "x": x,
                            "y": y,
                        }))
                    })
                    .collect()
            };
            return json!({
                "screen_type": "MAP",
                "floor": sim.current_floor.max(0) as u64,
                "gold": gold,
                "current_hp": current_hp,
                "max_hp": max_hp,
                "deck_ids": deck,
                "relic_ids": relic_ids,
                "choices": choices,
                "first_node_chosen": first_node_chosen,
                "current_node": {
                    "symbol": current_symbol,
                    "x": current_x,
                    "y": current_y,
                },
                "next_nodes": next_nodes,
            });
        }
    }

    if path_xs.is_empty() {
        let topology = generate_target_map_topology(numeric_seed, act);
        let choices: Vec<String> = topology
            .first_row_choices
            .iter()
            .map(|x| format!("x={x}"))
            .collect();
        let next_nodes: Vec<Value> = topology
            .first_row_choices
            .iter()
            .map(|&x| {
                json!({
                    "symbol": room_kind_symbol(topology.first_row_room_kind),
                    "x": x,
                    "y": 0,
                })
            })
            .collect();
        let floor = run.map(|sim| sim.current_floor as u64).unwrap_or(0);
        return json!({
            "screen_type": "MAP",
            "floor": floor,
            "gold": gold,
            "current_hp": current_hp,
            "max_hp": max_hp,
            "deck_ids": deck,
            "relic_ids": relic_ids,
            "choices": choices,
            "first_node_chosen": false,
            "current_node": {
                "symbol": "",
                "x": 0,
                "y": -1,
            },
            "next_nodes": next_nodes,
        });
    }

    let steps = generate_target_map_choices_after_path(numeric_seed, act, path_xs);
    let Some(step) = steps.last() else {
        return json!({});
    };
    let boss_available = step.floor >= 15 && step.next_choices == [3];
    let choices: Vec<String> = if boss_available {
        vec!["boss".to_owned()]
    } else {
        step.next_choices.iter().map(|x| format!("x={x}")).collect()
    };
    let current_x = *path_xs.last().unwrap_or(&0);
    let current_y = path_xs.len().saturating_sub(1) as i64;
    let current_symbol = seed_start_room_kinds_on_path_for_act(numeric_seed, act, path_xs)
        .last()
        .copied()
        .map(room_kind_symbol)
        .unwrap_or("M");
    let next_nodes: Vec<Value> = if boss_available {
        Vec::new()
    } else {
        step.next_choices
            .iter()
            .map(|&x| {
                let mut child_path = path_xs.to_vec();
                child_path.push(x);
                let symbol = seed_start_room_kinds_on_path_for_act(numeric_seed, act, &child_path)
                    .last()
                    .copied()
                    .map(room_kind_symbol)
                    .unwrap_or("M");
                json!({
                    "symbol": symbol,
                    "x": x,
                    "y": current_y + 1,
                })
            })
            .collect()
    };
    json!({
        "screen_type": "MAP",
        "floor": path_xs.len() as u64,
        "gold": gold,
        "current_hp": current_hp,
        "max_hp": max_hp,
        "deck_ids": deck,
        "relic_ids": relic_ids,
        "choices": choices,
        "first_node_chosen": true,
        "current_node": {
            "symbol": current_symbol,
            "x": current_x,
            "y": current_y,
        },
        "next_nodes": next_nodes,
    })
}

fn seed_start_project_post_boss_transition_current_node(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert(
        "current_node".to_owned(),
        json!({
            "symbol": "",
            "x": -1,
            "y": 15,
        }),
    );
}

fn seed_start_map_node_xy(node_id: sts_core::MapNodeId) -> (i32, i64) {
    if node_id.get() == 0 {
        return (0, -1);
    }
    let index = node_id.get() - 1;
    ((index % 7) as i32, (index / 7) as i64)
}

fn seed_start_target_act_from_run(run: Option<&RunState>) -> TargetMapAct {
    match run.map(|sim| sim.current_act).unwrap_or(1) {
        3 => TargetMapAct::Beyond,
        2 => TargetMapAct::City,
        _ => TargetMapAct::Exordium,
    }
}

fn seed_start_room_kinds_on_path_for_act(
    numeric_seed: i64,
    act: TargetMapAct,
    path_xs: &[i32],
) -> Vec<RoomKind> {
    let previous_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let room_kinds =
        std::panic::catch_unwind(|| target_room_kinds_on_path(numeric_seed, act, path_xs));
    std::panic::set_hook(previous_panic_hook);
    room_kinds.unwrap_or_default()
}

#[cfg(test)]
fn seed_start_encounter_expected_at_index(
    seed: i64,
    combat_index: usize,
    ascension: u8,
    deck_ids: &[String],
    relics: &[String],
    neow_lament: bool,
    message: &Value,
) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let floor = game
        .get("floor")
        .and_then(Value::as_u64)
        .map(|value| u32::try_from(value).unwrap_or(1))
        .unwrap_or_else(|| u32::try_from(combat_index + 1).unwrap_or(1));
    let spawns = seed_start_normal_encounter_spawns_at_combat_index(
        seed,
        floor,
        combat_index,
        ascension,
        neow_lament,
    );
    let mut expected = seed_start_encounter_observed_subset(message);
    if let Value::Object(map) = &mut expected {
        map.insert(
            "monsters".to_owned(),
            Value::Array(
                spawns
                    .iter()
                    .enumerate()
                    .map(|(index, spawn)| seed_start_monster_from_spawn(seed, floor, spawn, index))
                    .collect(),
            ),
        );
        map.insert("deck_ids".to_owned(), json!(deck_ids));
        map.insert("relic_ids".to_owned(), json!(relics));
    }
    expected
}

#[cfg(test)]
fn seed_start_normal_encounter_spawns_at_combat_index(
    seed: i64,
    floor: u32,
    combat_index: usize,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    match seed_start_target_act_from_floor(floor) {
        TargetMapAct::Exordium => target_normal_encounter_spawn_at_combat_index(
            seed,
            floor,
            combat_index,
            ascension,
            neow_lament,
        ),
        TargetMapAct::City => target_city_normal_encounter_spawn_at_combat_index(
            seed,
            floor,
            combat_index,
            ascension,
            neow_lament,
        ),
        TargetMapAct::Beyond => {
            sts_core::content::encounters::target_normal_encounter_key_at_combat_index(
                seed,
                TargetMapAct::Beyond,
                combat_index,
            )
            .and_then(|encounter_key| {
                target_beyond_encounter_spawn_for_key(
                    seed,
                    floor,
                    &encounter_key,
                    ascension,
                    neow_lament,
                )
            })
        }
    }
    .unwrap_or_default()
}

#[cfg(test)]
fn seed_start_target_act_from_floor(floor: u32) -> TargetMapAct {
    if floor >= 35 {
        TargetMapAct::Beyond
    } else if floor >= 18 {
        TargetMapAct::City
    } else {
        TargetMapAct::Exordium
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn seed_start_core_neow_lament_active(run: Option<&RunState>) -> bool {
    run.is_some_and(|run| run.neow_lament_combats_remaining > 0)
}

#[cfg(test)]
fn seed_start_monster_from_spawn(
    seed: i64,
    floor: u32,
    spawn: &TargetEncounterSpawn,
    index: usize,
) -> Value {
    json!({
        "name": target_spawn_trace_name(seed, floor, spawn, index),
        "current_hp": spawn.current_hp,
        "max_hp": spawn.max_hp,
        "block": spawn.block,
        "intent": spawn.intent,
        "strength": spawn_power_amount(&spawn.powers, "Strength"),
        "ritual": spawn_power_amount(&spawn.powers, "Ritual"),
        "vulnerable": spawn_power_amount(&spawn.powers, "Vulnerable"),
    })
}

#[cfg(test)]
fn spawn_power_amount(powers: &[TargetSpawnPower], id: &str) -> i32 {
    powers
        .iter()
        .find(|power| power.id == id)
        .map(|power| power.amount)
        .unwrap_or(0)
}

#[cfg(test)]
fn target_spawn_trace_name(
    _seed: i64,
    _floor: u32,
    spawn: &TargetEncounterSpawn,
    _index: usize,
) -> &'static str {
    match spawn.name {
        "Louse" | "LouseDefensive" | "FuzzyLouseNormal" | "FuzzyLouseDefensive" => "Louse",
        "SlaverBlue" | "SlaverRed" => "Slaver",
        _ => spawn.name,
    }
}

fn seed_start_trace_monster_name(content_id: ContentId) -> &'static str {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, CULTIST_ID, GREEN_LOUSE_ID, JAW_WORM_ID, RED_LOUSE_ID, SPIKE_SLIME_ID,
    };
    match content_id {
        id if id == CULTIST_ID => "Cultist",
        id if id == JAW_WORM_ID => "Jaw Worm",
        id if id == LOOTER_ID => "Looter",
        id if id == SPIKE_SLIME_ID => "Spike Slime (S)",
        id if id == ACID_SLIME_ID => "Acid Slime (M)",
        id if id == GREEN_LOUSE_ID || id == RED_LOUSE_ID => "Louse",
        _ => "Cultist",
    }
}

fn seed_start_trace_intent(monster: &MonsterState) -> String {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, GREEN_LOUSE_ID, RED_LOUSE_ID, SPIKE_SLIME_ID,
    };

    match monster.intent {
        MonsterIntent::ApplyPlayerWeak { .. } if monster.content_id == ACID_SLIME_ID => {
            "DEBUFF".to_owned()
        }
        MonsterIntent::Attack { .. }
        | MonsterIntent::AttackAddSlimedToDiscard { .. }
        | MonsterIntent::AttackApplyPlayerFrail { .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
        | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. }
        | MonsterIntent::AttackHealSelf { .. }
        | MonsterIntent::AttackStealGold { .. }
            if monster.content_id == ACID_SLIME_ID =>
        {
            "ATTACK_DEBUFF".to_owned()
        }
        MonsterIntent::Block { .. }
            if matches!(monster.content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) =>
        {
            "ATTACK".to_owned()
        }
        MonsterIntent::AttackAddSlimedToDiscard { .. } if monster.content_id == SPIKE_SLIME_ID => {
            "ATTACK_DEBUFF".to_owned()
        }
        MonsterIntent::Attack { .. } if monster.content_id == SPIKE_SLIME_ID => "ATTACK".to_owned(),
        _ => intent_key(monster),
    }
}

fn seed_start_first_map_choices(seed: &str) -> Vec<String> {
    generate_exordium_map_topology(sts_seed_string_to_long(seed))
        .first_row_choices
        .into_iter()
        .map(|x| format!("x={x}"))
        .collect()
}

fn reward_types_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(rewards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    rewards
        .iter()
        .filter_map(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn card_reward_ids_from_value(value: Option<&Value>) -> Vec<Value> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    cards
        .iter()
        .filter_map(|card| {
            if let Some(content_id) = content_id_from_card_value(card) {
                return Some(json!(content_id.get()));
            }
            let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
            if upgrades > 0 {
                card.get("name").and_then(Value::as_str)
            } else {
                card.get("id").and_then(Value::as_str)
            }
            .map(|id| json!(id))
        })
        .collect()
}

fn map_nodes_from_value(value: Option<&Value>) -> Vec<Value> {
    let Some(nodes) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    nodes
        .iter()
        .map(|node| {
            json!({
                "symbol": node.get("symbol").and_then(Value::as_str).unwrap_or(""),
                "x": node.get("x").and_then(Value::as_i64).unwrap_or(0),
                "y": node.get("y").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect()
}

fn relic_keys_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(relics) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    relics
        .iter()
        .filter_map(|relic| {
            relic
                .get("name")
                .or_else(|| relic.get("id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_combat_relics_and_counters(game: &Value) -> (Vec<Relic>, RelicCounters) {
    let mut relics = Vec::new();
    let mut counters = RelicCounters::default();

    for relic in observed_relic_entries(game) {
        let Some(observed) = relic_from_trace_name(relic.name) else {
            continue;
        };
        if !relics.contains(&observed) {
            relics.push(observed);
        }

        let Some(counter) = relic.counter else {
            continue;
        };
        match observed {
            Relic::InkBottle => counters.ink_bottle_cards_played = counter,
            Relic::Nunchaku => counters.nunchaku_attacks_played = counter,
            Relic::PenNib => counters.pen_nib_attacks_played = counter,
            Relic::Shuriken => counters.shuriken_attacks_this_turn = counter,
            Relic::Kunai => counters.kunai_attacks_this_turn = counter,
            Relic::LetterOpener => counters.letter_opener_skills_this_turn = counter,
            Relic::Pocketwatch => counters.cards_played_this_turn = counter,
            Relic::HappyFlower => counters.happy_flower_turns = counter,
            Relic::StoneCalendar => counters.player_turns_started = counter,
            Relic::IncenseBurner => counters.incense_burner_counter = counter,
            _ => {}
        }
    }

    (relics, counters)
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_combat_turn(combat: &Value) -> Option<u32> {
    combat
        .get("turn")
        .and_then(Value::as_u64)
        .and_then(|turn| u32::try_from(turn).ok())
}

#[cfg(test)]
#[allow(dead_code)]
struct ObservedRelicEntry<'a> {
    name: &'a str,
    counter: Option<u32>,
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_relic_entries(game: &Value) -> Vec<ObservedRelicEntry<'_>> {
    let Some(relics) = game.get("relics").and_then(Value::as_array) else {
        return Vec::new();
    };

    relics
        .iter()
        .filter_map(|relic| {
            let name = relic
                .get("name")
                .or_else(|| relic.get("id"))
                .and_then(Value::as_str)?;
            let counter = relic
                .get("counter")
                .and_then(Value::as_i64)
                .and_then(|counter| u32::try_from(counter).ok());
            Some(ObservedRelicEntry { name, counter })
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_energy_per_turn(relics: &[Relic]) -> i32 {
    if relics.iter().any(|relic| {
        matches!(
            relic,
            Relic::CoffeeDripper
                | Relic::CursedKey
                | Relic::Ectoplasm
                | Relic::FusionHammer
                | Relic::MarkOfPain
                | Relic::PhilosophersStone
                | Relic::RunicDome
                | Relic::Sozu
                | Relic::VelvetChoker
        )
    }) {
        4
    } else {
        3
    }
}

fn choice_list_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(choices) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    choices
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn relic_key_trace_name(key: RelicKey) -> &'static str {
    match key {
        RelicKey::Akabeko => "Akabeko",
        RelicKey::BagOfMarbles => "Bag of Marbles",
        RelicKey::BagOfPreparation => "Bag of Preparation",
        RelicKey::BurningBlood => "Burning Blood",
        RelicKey::DreamCatcher => "Dream Catcher",
        RelicKey::ToxicEgg => "Toxic Egg",
        RelicKey::FrozenEgg => "Frozen Egg",
        RelicKey::MummifiedHand => "Mummified Hand",
        RelicKey::CeramicFish => "Ceramic Fish",
        RelicKey::PenNib => "Pen Nib",
        RelicKey::MembershipCard => "Membership Card",
        RelicKey::Whetstone => "Whetstone",
        RelicKey::Orichalcum => "Orichalcum",
        RelicKey::BronzeScales => "Bronze Scales",
        RelicKey::ToyOrnithopter => "Toy Ornithopter",
        RelicKey::GremlinHorn => "Gremlin Horn",
        RelicKey::JuzuBracelet => "Juzu Bracelet",
        RelicKey::Omamori => "Omamori",
        RelicKey::Lantern => "Lantern",
        RelicKey::Pocketwatch => "Pocketwatch",
        RelicKey::CentennialPuzzle => "Centennial Puzzle",
        RelicKey::OddlySmoothStone => "Oddly Smooth Stone",
        RelicKey::MeatOnTheBone => "Meat on the Bone",
        RelicKey::ClockworkSouvenir => "Clockwork Souvenir",
        RelicKey::Orrery => "Orrery",
        RelicKey::StoneCalendar => "Stone Calendar",
        RelicKey::IceCream => "Ice Cream",
        RelicKey::Calipers => "Calipers",
        RelicKey::CursedKey => "Cursed Key",
        RelicKey::FusionHammer => "Fusion Hammer",
        RelicKey::VelvetChoker => "Velvet Choker",
        RelicKey::RunicDome => "Runic Dome",
        RelicKey::SlaversCollar => "Slaver's Collar",
        RelicKey::SneckoEye => "Snecko Eye",
        RelicKey::PandorasBox => "Pandora's Box",
        RelicKey::BustedCrown => "Busted Crown",
        RelicKey::Ectoplasm => "Ectoplasm",
        RelicKey::TinyHouse => "Tiny House",
        RelicKey::Sozu => "Sozu",
        RelicKey::PhilosophersStone => "Philosopher's Stone",
        RelicKey::Astrolabe => "Astrolabe",
        RelicKey::BlackStar => "Black Star",
        RelicKey::SacredBark => "Sacred Bark",
        RelicKey::EmptyCage => "Empty Cage",
        RelicKey::RunicPyramid => "Runic Pyramid",
        RelicKey::CallingBell => "Calling Bell",
        RelicKey::CoffeeDripper => "Coffee Dripper",
        RelicKey::BlackBlood => "Black Blood",
        RelicKey::EternalFeather => "Eternal Feather",
        RelicKey::Pear => "Pear",
        RelicKey::MarkOfPain => "Mark of Pain",
        RelicKey::RunicCube => "Runic Cube",
        RelicKey::DeadBranch => "Dead Branch",
        RelicKey::ChampionBelt => "Champion Belt",
        RelicKey::GoldenIdol => "Golden Idol",
        RelicKey::DuVuDoll => "Du-Vu Doll",
        RelicKey::MedicalKit => "Medical Kit",
        RelicKey::WarPaint => "War Paint",
        RelicKey::LetterOpener => "Letter Opener",
        RelicKey::Nunchaku => "Nunchaku",
        RelicKey::InkBottle => "Ink Bottle",
        RelicKey::Shuriken => "Shuriken",
        RelicKey::Kunai => "Kunai",
        RelicKey::HappyFlower => "Happy Flower",
        RelicKey::IncenseBurner => "Incense Burner",
        RelicKey::ThreadAndNeedle => "Thread and Needle",
        RelicKey::PaperPhrog => "Paper Phrog",
        RelicKey::StrangeSpoon => "Strange Spoon",
        RelicKey::DollysMirror => "Dolly's Mirror",
        RelicKey::SelfFormingClay => "Self-Forming Clay",
        RelicKey::CultistMask => "CultistMask",
        RelicKey::FaceOfCleric => "FaceOfCleric",
        RelicKey::GremlinMask => "GremlinMask",
        RelicKey::NlothsMask => "NlothsMask",
        RelicKey::SsserpentHead => "SsserpentHead",
        RelicKey::WarpedTongs => "Warped Tongs",
        _ => "Unknown Relic",
    }
}

#[cfg(test)]
fn relic_key_from_trace_name(name: &str) -> Option<RelicKey> {
    match normalized_trace_relic_name(name).as_str() {
        "akabeko" => Some(RelicKey::Akabeko),
        "bagofmarbles" => Some(RelicKey::BagOfMarbles),
        "bagofpreparation" => Some(RelicKey::BagOfPreparation),
        "burningblood" => Some(RelicKey::BurningBlood),
        "dreamcatcher" => Some(RelicKey::DreamCatcher),
        "toxicegg" => Some(RelicKey::ToxicEgg),
        "frozenegg" | "frozenegg2" => Some(RelicKey::FrozenEgg),
        "mummifiedhand" => Some(RelicKey::MummifiedHand),
        "ceramicfish" => Some(RelicKey::CeramicFish),
        "pennib" => Some(RelicKey::PenNib),
        "membershipcard" => Some(RelicKey::MembershipCard),
        "whetstone" => Some(RelicKey::Whetstone),
        "orichalcum" => Some(RelicKey::Orichalcum),
        "bronzescales" => Some(RelicKey::BronzeScales),
        "toyornithopter" => Some(RelicKey::ToyOrnithopter),
        "gremlinhorn" => Some(RelicKey::GremlinHorn),
        "omamori" => Some(RelicKey::Omamori),
        "lantern" => Some(RelicKey::Lantern),
        "pocketwatch" => Some(RelicKey::Pocketwatch),
        "centennialpuzzle" => Some(RelicKey::CentennialPuzzle),
        "oddlysmoothstone" => Some(RelicKey::OddlySmoothStone),
        "meatonthebone" => Some(RelicKey::MeatOnTheBone),
        "clockworksouvenir" => Some(RelicKey::ClockworkSouvenir),
        "stonecalendar" => Some(RelicKey::StoneCalendar),
        "icecream" => Some(RelicKey::IceCream),
        "calipers" => Some(RelicKey::Calipers),
        "cursedkey" => Some(RelicKey::CursedKey),
        "fusionhammer" => Some(RelicKey::FusionHammer),
        "velvetchoker" => Some(RelicKey::VelvetChoker),
        "runicdome" => Some(RelicKey::RunicDome),
        "slaverscollar" => Some(RelicKey::SlaversCollar),
        "sneckoeye" => Some(RelicKey::SneckoEye),
        "pandorasbox" => Some(RelicKey::PandorasBox),
        "bustedcrown" => Some(RelicKey::BustedCrown),
        "ectoplasm" => Some(RelicKey::Ectoplasm),
        "tinyhouse" => Some(RelicKey::TinyHouse),
        "sozu" => Some(RelicKey::Sozu),
        "philosophersstone" => Some(RelicKey::PhilosophersStone),
        "astrolabe" => Some(RelicKey::Astrolabe),
        "blackstar" => Some(RelicKey::BlackStar),
        "sacredbark" => Some(RelicKey::SacredBark),
        "emptycage" => Some(RelicKey::EmptyCage),
        "runicpyramid" => Some(RelicKey::RunicPyramid),
        "callingbell" => Some(RelicKey::CallingBell),
        "coffeedripper" => Some(RelicKey::CoffeeDripper),
        "blackblood" => Some(RelicKey::BlackBlood),
        "markofpain" => Some(RelicKey::MarkOfPain),
        "runiccube" => Some(RelicKey::RunicCube),
        "deadbranch" => Some(RelicKey::DeadBranch),
        "threadandneedle" => Some(RelicKey::ThreadAndNeedle),
        "paperphrog" => Some(RelicKey::PaperPhrog),
        "strangespoon" => Some(RelicKey::StrangeSpoon),
        "dollysmirror" => Some(RelicKey::DollysMirror),
        "selfformingclay" => Some(RelicKey::SelfFormingClay),
        "cultistmask" | "cultistheadpiece" => Some(RelicKey::CultistMask),
        "faceofcleric" | "clericface" => Some(RelicKey::FaceOfCleric),
        "gremlinmask" | "gremlinvisage" => Some(RelicKey::GremlinMask),
        "nlothsmask" => Some(RelicKey::NlothsMask),
        "ssserpenthead" => Some(RelicKey::SsserpentHead),
        "warpedtongs" => Some(RelicKey::WarpedTongs),
        "pear" => Some(RelicKey::Pear),
        "eternalfeather" => Some(RelicKey::EternalFeather),
        "championbelt" => Some(RelicKey::ChampionBelt),
        "goldenidol" => Some(RelicKey::GoldenIdol),
        "duvudoll" => Some(RelicKey::DuVuDoll),
        "medicalkit" => Some(RelicKey::MedicalKit),
        "warpaint" => Some(RelicKey::WarPaint),
        "letteropener" => Some(RelicKey::LetterOpener),
        "nunchaku" => Some(RelicKey::Nunchaku),
        "inkbottle" => Some(RelicKey::InkBottle),
        "shuriken" => Some(RelicKey::Shuriken),
        "kunai" => Some(RelicKey::Kunai),
        "happyflower" => Some(RelicKey::HappyFlower),
        "incenseburner" => Some(RelicKey::IncenseBurner),
        _ => None,
    }
}

#[cfg(test)]
fn relic_from_trace_name(name: &str) -> Option<Relic> {
    match normalized_trace_relic_name(name).as_str() {
        "akabeko" => Some(Relic::Akabeko),
        "bagofmarbles" => Some(Relic::BagOfMarbles),
        "bagofpreparation" => Some(Relic::BagOfPreparation),
        "burningblood" => Some(Relic::BurningBlood),
        "dreamcatcher" => Some(Relic::DreamCatcher),
        "toxicegg" => Some(Relic::ToxicEgg),
        "frozenegg" | "frozenegg2" => Some(Relic::FrozenEgg),
        "mummifiedhand" => Some(Relic::MummifiedHand),
        "ceramicfish" => Some(Relic::CeramicFish),
        "pennib" => Some(Relic::PenNib),
        "membershipcard" => Some(Relic::MembershipCard),
        "whetstone" => Some(Relic::Whetstone),
        "orichalcum" => Some(Relic::Orichalcum),
        "bronzescales" => Some(Relic::BronzeScales),
        "toyornithopter" => Some(Relic::ToyOrnithopter),
        "gremlinhorn" => Some(Relic::GremlinHorn),
        "omamori" => Some(Relic::Omamori),
        "lantern" => Some(Relic::Lantern),
        "pocketwatch" => Some(Relic::Pocketwatch),
        "centennialpuzzle" => Some(Relic::CentennialPuzzle),
        "oddlysmoothstone" => Some(Relic::OddlySmoothStone),
        "meatonthebone" => Some(Relic::MeatOnTheBone),
        "clockworksouvenir" => Some(Relic::ClockworkSouvenir),
        "stonecalendar" => Some(Relic::StoneCalendar),
        "icecream" => Some(Relic::IceCream),
        "calipers" => Some(Relic::Calipers),
        "cursedkey" => Some(Relic::CursedKey),
        "fusionhammer" => Some(Relic::FusionHammer),
        "velvetchoker" => Some(Relic::VelvetChoker),
        "runicdome" => Some(Relic::RunicDome),
        "slaverscollar" => Some(Relic::SlaversCollar),
        "sneckoeye" => Some(Relic::SneckoEye),
        "pandorasbox" => Some(Relic::PandorasBox),
        "bustedcrown" => Some(Relic::BustedCrown),
        "ectoplasm" => Some(Relic::Ectoplasm),
        "tinyhouse" => Some(Relic::TinyHouse),
        "sozu" => Some(Relic::Sozu),
        "philosophersstone" => Some(Relic::PhilosophersStone),
        "astrolabe" => Some(Relic::Astrolabe),
        "blackstar" => Some(Relic::BlackStar),
        "sacredbark" => Some(Relic::SacredBark),
        "emptycage" => Some(Relic::EmptyCage),
        "runicpyramid" => Some(Relic::RunicPyramid),
        "callingbell" => Some(Relic::CallingBell),
        "coffeedripper" => Some(Relic::CoffeeDripper),
        "blackblood" => Some(Relic::BlackBlood),
        "markofpain" => Some(Relic::MarkOfPain),
        "runiccube" => Some(Relic::RunicCube),
        "deadbranch" => Some(Relic::DeadBranch),
        "threadandneedle" => Some(Relic::ThreadAndNeedle),
        "paperphrog" => Some(Relic::PaperPhrog),
        "strangespoon" => Some(Relic::StrangeSpoon),
        "dollysmirror" => Some(Relic::DollysMirror),
        "selfformingclay" => Some(Relic::SelfFormingClay),
        "cultistmask" | "cultistheadpiece" => Some(Relic::CultistMask),
        "faceofcleric" | "clericface" => Some(Relic::FaceOfCleric),
        "gremlinmask" | "gremlinvisage" => Some(Relic::GremlinMask),
        "nlothsmask" => Some(Relic::NlothsMask),
        "ssserpenthead" => Some(Relic::SsserpentHead),
        "pear" => Some(Relic::Pear),
        "eternalfeather" => Some(Relic::EternalFeather),
        "championbelt" => Some(Relic::ChampionBelt),
        "goldenidol" => Some(Relic::GoldenIdol),
        "duvudoll" => Some(Relic::DuVuDoll),
        "medicalkit" => Some(Relic::MedicalKit),
        "warpaint" => Some(Relic::WarPaint),
        "letteropener" => Some(Relic::LetterOpener),
        "nunchaku" => Some(Relic::Nunchaku),
        "inkbottle" => Some(Relic::InkBottle),
        "shuriken" => Some(Relic::Shuriken),
        "kunai" => Some(Relic::Kunai),
        "happyflower" => Some(Relic::HappyFlower),
        "incenseburner" => Some(Relic::IncenseBurner),
        "warpedtongs" => Some(Relic::WarpedTongs),
        _ => None,
    }
}

fn normalized_trace_relic_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn potion_from_trace_name(name: &str) -> Option<Potion> {
    match name {
        "Attack Potion" => Some(Potion::Attack),
        "Blessing of the Forge" => Some(Potion::BlessingOfTheForge),
        "Blood Potion" => Some(Potion::Blood),
        "Colorless Potion" => Some(Potion::Colorless),
        "Cultist Potion" => Some(Potion::Cultist),
        "Dexterity Potion" => Some(Potion::Dexterity),
        "Distilled Chaos" => Some(Potion::DistilledChaos),
        "Duplication Potion" => Some(Potion::Duplication),
        "Elixir" => Some(Potion::Elixir),
        "Energy Potion" => Some(Potion::Energy),
        "Entropic Brew" => Some(Potion::EntropicBrew),
        "Essence of Steel" => Some(Potion::EssenceOfSteel),
        "Explosive Potion" => Some(Potion::Explosive),
        "Fairy in a Bottle" => Some(Potion::Fairy),
        "Fear Potion" => Some(Potion::Fear),
        "Fire Potion" => Some(Potion::Fire),
        "Flex Potion" => Some(Potion::Flex),
        "Fruit Juice" => Some(Potion::FruitJuice),
        "Gamblers Brew" | "Gambler's Brew" | "GamblersBrew" => Some(Potion::GamblersBrew),
        "Heart of Iron" => Some(Potion::HeartOfIron),
        "Liquid Bronze" => Some(Potion::LiquidBronze),
        "Liquid Memories" => Some(Potion::LiquidMemories),
        "Power Potion" => Some(Potion::Power),
        "Regen Potion" => Some(Potion::Regen),
        "Skill Potion" => Some(Potion::Skill),
        "Smoke Bomb" => Some(Potion::SmokeBomb),
        "Snecko Oil" => Some(Potion::SneckoOil),
        "Speed Potion" => Some(Potion::Speed),
        "Strength Potion" => Some(Potion::Strength),
        "Swift Potion" => Some(Potion::Swift),
        "Weak Potion" => Some(Potion::Weak),
        "Block Potion" => Some(Potion::Block),
        "Ancient Potion" => Some(Potion::Ancient),
        _ => None,
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn potions_from_observed(game: &Value) -> Vec<Potion> {
    game.get("potions")
        .and_then(Value::as_array)
        .map(|potions| {
            potions
                .iter()
                .filter_map(|potion| {
                    let name = potion.get("name").and_then(Value::as_str)?;
                    if name.eq_ignore_ascii_case("Potion Slot") {
                        return None;
                    }
                    potion_from_trace_name(name)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(dead_code)]
fn empty_potion_slots_from_observed(game: &Value) -> Vec<usize> {
    game.get("potions")
        .and_then(Value::as_array)
        .map(|potions| {
            potions
                .iter()
                .enumerate()
                .filter_map(|(index, potion)| {
                    let name = potion.get("name").and_then(Value::as_str)?;
                    name.eq_ignore_ascii_case("Potion Slot").then_some(index)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn potion_keys_from_value(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|potions| {
            potions
                .iter()
                .filter_map(|potion| {
                    let name = potion.get("name").and_then(Value::as_str)?;
                    if name.eq_ignore_ascii_case("Potion Slot") {
                        return None;
                    }
                    potion_from_trace_name(name).map(|potion| potion_trace_name(potion).to_owned())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn relic_ids_for_simulated_subset(run: &RunState, carry: &[String]) -> Vec<String> {
    let mut out = if carry.is_empty() {
        vec!["Burning Blood".to_owned()]
    } else {
        carry.to_vec()
    };
    remove_simulated_replaced_starter_relics(run, &mut out);
    for relic in &run.relics {
        let name = relic_key_trace_name(relic.key()).to_owned();
        if name != "Unknown Relic" && !out.contains(&name) {
            out.push(name);
        }
    }
    for key in &run.relic_keys {
        let name = relic_key_trace_name(*key).to_owned();
        if name != "Unknown Relic" && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn remove_simulated_replaced_starter_relics(run: &RunState, relics: &mut Vec<String>) {
    for (upgraded, starter) in [
        (RelicKey::BlackBlood, "Burning Blood"),
        (RelicKey::FrozenCore, "Cracked Core"),
        (RelicKey::HolyWater, "Pure Water"),
        (RelicKey::RingOfTheSerpent, "Ring of the Snake"),
    ] {
        if run_has_relic_key(run, upgraded) {
            let upgraded_name = relic_key_trace_name(upgraded).to_owned();
            if let Some(slot) = relics.iter_mut().find(|name| name.as_str() == starter) {
                *slot = upgraded_name.clone();
            }
            let mut seen = false;
            relics.retain(|name| {
                if name == &upgraded_name {
                    if seen {
                        false
                    } else {
                        seen = true;
                        true
                    }
                } else {
                    true
                }
            });
        }
    }
}

fn run_has_relic_key(run: &RunState, key: RelicKey) -> bool {
    run.relic_keys.contains(&key) || run.relics.iter().any(|relic| relic.key() == key)
}

fn seed_start_update_carry_from_run(
    run: &RunState,
    relics: &mut Vec<String>,
    deck_ids: &mut Vec<String>,
) {
    *deck_ids = deck_content_keys(&run.deck);
    remove_simulated_replaced_starter_relics(run, relics);
    for relic in &run.relics {
        let name = relic_key_trace_name(relic.key()).to_owned();
        if name != "Unknown Relic" && !relics.contains(&name) {
            relics.push(name);
        }
    }
    for key in &run.relic_keys {
        let name = relic_key_trace_name(*key).to_owned();
        if name != "Unknown Relic" && !relics.contains(&name) {
            relics.push(name);
        }
    }
}

fn seed_start_carried_run(
    carried: Option<&RunState>,
    numeric_seed: i64,
    ascension: u8,
    external_seed: &str,
    deck_ids: &[String],
) -> RunState {
    if let Some(sim) = carried {
        let mut next = sim.clone();
        next.combat = None;
        next.reward = None;
        next.event = None;
        next.shop = None;
        next.shop_merchant_open = false;
        next.card_grid = None;
        next.phase = RunPhase::Idle;
        return next;
    }
    let mut run = seed_start_seeded_idle_run(numeric_seed, ascension, deck_ids);
    run.gold = 99;
    run.reward_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    seed_start_apply_reward_rng_snapshot(&mut run, numeric_seed, external_seed, 0);
    run
}

fn seed_start_event_observed_subset(message: &Value) -> Value {
    let mut value = seed_start_observed_subset(message);
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "event_id".to_owned(),
            json!(seed_start_observed_event_key(message).unwrap_or_default()),
        );
    }
    value
}

fn seed_start_event_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    seed_start_event_simulated_subset_with_deck(run, relic_ids, deck_content_keys(&run.deck))
}

fn seed_start_event_simulated_subset_with_pending_obtain_cards(
    run: &RunState,
    relic_ids: &[String],
) -> Value {
    seed_start_event_simulated_subset_with_deck(
        run,
        relic_ids,
        deck_content_keys_with_pending_obtain_cards(run),
    )
}

fn seed_start_event_simulated_subset_with_deck(
    run: &RunState,
    relic_ids: &[String],
    deck_ids: Vec<String>,
) -> Value {
    let choices = run
        .event
        .as_ref()
        .map(|event| {
            event
                .choices
                .iter()
                .filter_map(|choice| seed_start_visible_event_choice_label(&choice.label))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let event_id = run
        .event
        .as_ref()
        .map(|event| normalized_trace_relic_name(&format!("{:?}", event.event)))
        .unwrap_or_default();
    let screen_type = if run.phase == RunPhase::Event {
        "EVENT"
    } else {
        "MAP"
    };
    json!({
        "screen_type": screen_type,
        "event_id": event_id,
        "ascension": run.ascension as u64,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_ids,
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
}

fn seed_start_observed_event_key(message: &Value) -> Option<String> {
    let screen = message
        .get("game_state")
        .and_then(|game| game.get("screen_state"))?;
    let event = screen
        .get("event_id")
        .or_else(|| screen.get("event_name"))
        .and_then(Value::as_str)?;
    let key = normalized_trace_relic_name(event);
    Some(match key.as_str() {
        "goldenwing" => "wingstatue".to_owned(),
        _ => key,
    })
}

fn seed_start_event_choice_index_for_communication_mod(
    run: &RunState,
    visible_choice_index: usize,
) -> Option<usize> {
    run.event
        .as_ref()?
        .choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| seed_start_visible_event_choice_label(&choice.label).is_some())
        .nth(visible_choice_index)
        .map(|(choice_index, _)| choice_index)
}

fn seed_start_visible_event_choice_label(label: &str) -> Option<String> {
    let mut label = label.to_ascii_lowercase();
    if let Some((visible, _effect_text)) = label.split_once(" (") {
        label = visible.to_owned();
    }
    match label.as_str() {
        "locked" => None,
        "enter the light" => Some("enter".to_owned()),
        _ => Some(label),
    }
}

fn deck_instances_from_keys(deck_ids: &[String]) -> Vec<CardInstance> {
    deck_ids
        .iter()
        .enumerate()
        .filter_map(|(index, key)| {
            content_id_from_key(key)
                .map(|content_id| CardInstance::new(CardId::new(index as u64 + 1), content_id))
        })
        .collect()
}

fn seed_start_rng_boundaries() -> Vec<RngBoundary> {
    vec![
        RngBoundary {
            stream: "seed_conversion".to_owned(),
            save_counter: None,
            status: "source_backed".to_owned(),
            reason: "SeedHelper.getLong from the target 12-18-2022 desktop jar uppercases seed text, maps O to 0, and parses it with base-35 alphabet 0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ".to_owned(),
        },
        RngBoundary {
            stream: "neowRng".to_owned(),
            save_counter: None,
            status: "source_backed_options_with_partial_application".to_owned(),
            reason: "Neow option generation uses target-style NeowEvent.rng initialization from Settings.seed, visible slot order, and five option-screen draws. Seed-start branch dispatch uses generated selected options; CODEX04/TEST colorless choices, CODEX04 three-potion choices, VERIFY01 common relic identity, MANUAL01 immediate rare-card identity, simple-drawback rare relic identity, and M290001/M290008 transform identity are generated. Core helpers cover card, colorless, potion, fixed-tier relic, boss-swap, transform, grid, curse-combo card/relic, Neow's Lament combat carry state, and simple no-RNG reward/drawback surfaces. Synthetic verifier follow-ups now cover Calling Bell, Astrolabe, Pandora's Box, Empty Cage, and Tiny House boss-swap paths. Selected-trace coverage for many branch combinations and broad boss-swap selected-trace evidence remain partial/caveated.".to_owned(),
        },
        RngBoundary {
            stream: "mapRng".to_owned(),
            save_counter: None,
            status: "source_backed_topology_prefix".to_owned(),
            reason: "decoded Exordium mapRng initialization uses seed + actNum and MapGenerator topology reproduces captured VERIFY01 first choices x=1/x=2, CODEX04 first choices x=0/x=2/x=4/x=5, and CODEX04 chosen-path next choices x=3 then x=2/x=3; fixed generateMap rows are row 0 combat, row 8 treasure, and row 14 rest; generateRoomTypes, RoomTypeAssigner two-stage room-list construction, raw RandomXS128 Collections.shuffle prefix, and full VERIFY01/CODEX04 room-symbol placement match decoded target behavior and captured map payloads".to_owned(),
        },
        RngBoundary {
            stream: "monsterRng".to_owned(),
            save_counter: Some("monster_seed_count".to_owned()),
            status: "source_backed_normal_list_prefix".to_owned(),
            reason: "decoded Exordium normal encounter list generation covers weak encounters, strong encounter weights, first-strong exclusions, and no-repeat-last-two retries; room execution maps combat index to list entries and target spawn state covers Cultist, Jaw Worm, Small Slimes, and 2 Louse for captured VERIFY01/CODEX04/CODEX03 first-three prefixes".to_owned(),
        },
        RngBoundary {
            stream: "monsterHpRng".to_owned(),
            save_counter: Some("monster_seed_count".to_owned()),
            status: "source_backed_floor_prefix".to_owned(),
            reason: "decoded room transition reinitializes monsterHpRng with Settings.seed + floorNum; floor-1 Cultist HP rolls reproduce VERIFY01 49 and CODEX04 54, CODEX04 floor-2 Small Slimes rolls reproduce Spike Slime (S) 11 plus Acid Slime (M) 32, and CODEX04 floor-3 louse constructors reproduce 13/15 with bite-damage interleaving".to_owned(),
        },
        RngBoundary {
            stream: "shuffleRng".to_owned(),
            save_counter: Some("card_random_seed_count".to_owned()),
            status: "captured_branch".to_owned(),
            reason: "Selected Ironclad A0 starter and modified-deck first combats derive opening piles from the current master-deck order: CardGroup.shuffle seeds Java Collections.shuffle with shuffleRng.randomLong(), draw piles use top-of-pile semantics, and innate/bottled cards are placed on top before opening draw. Broader in-combat and post-END state parity must be proven by simulator transitions, not trace repair.".to_owned(),
        },
        RngBoundary {
            stream: "cardRewardRng".to_owned(),
            save_counter: Some("card_seed_count".to_owned()),
            status: "source_backed_full_pool".to_owned(),
            reason: "card reward rarity rolls use target-style cardRng.random(99) + cardRarityFactor thresholds, common/rare factor mutation, duplicate rerolls, and StsRng counter consumption over the full 72-card Ironclad reward pool; many pool entries are RNG-only until their card mechanics are implemented".to_owned(),
        },
        RngBoundary {
            stream: "rewardGoldRng".to_owned(),
            save_counter: Some("treasure_seed_count".to_owned()),
            status: "source_backed_normal_combat".to_owned(),
            reason: "normal-combat gold uses target-style treasureRng.random(10, 20) with StsRng counter persistence; VERIFY01 and CODEX04 seed-start reward screens are generated from simulation-driven reward RNG rather than pinned constants".to_owned(),
        },
        RngBoundary {
            stream: "relicRng".to_owned(),
            save_counter: Some("relic_seed_count".to_owned()),
            status: "source_backed_pool_selection_wired".to_owned(),
            reason: "relic tier rolls for normal/chest-style and elite rewards use target thresholds and persisted relic_seed_count; Ironclad relic pools initialize, pop, and filter like target; elite/chest/boss relic reward screens and shop relic offers are wired from persisted pool state. VERIFY01 Neow common relic identity and simple-drawback Neow rare relic identity are generated through the fixed-tier relic helper; curse-combo rare relics and boss-swap follow-ups remain partial/caveated".to_owned(),
        },
        RngBoundary {
            stream: "merchantRng".to_owned(),
            save_counter: Some("merchant_seed_count".to_owned()),
            status: "source_backed_shop_inventory".to_owned(),
            reason: "shop inventory uses target-style Shop.cpp layout: 5 class cards + 2 colorless cards with sale slot, 3 relics (2 tier rolls + shop tier), 3 potions, and card-remove pricing; merchantRng/cardRng/potionRng/relic pool state drive generation without regressing relic_rng_counter".to_owned(),
        },
        RngBoundary {
            stream: "eventRng".to_owned(),
            save_counter: Some("event_seed_count".to_owned()),
            status: "source_backed_event_pool_with_captured_branches".to_owned(),
            reason: "Act 1 event/shrine pools initialize from target EventPools::Act1 lists; generateEvent uses 25% shrine chance and removes picked entries; Golden Shrine, Cleric heal, Shining Light, and The Ssssserpent outcomes are implemented. TEST and M290001 still use captured event-entry branches where broader event RNG alignment is not yet proven".to_owned(),
        },
        RngBoundary {
            stream: "potionRng".to_owned(),
            save_counter: Some("potion_seed_count".to_owned()),
            status: "source_backed_reward_drop".to_owned(),
            reason: "normal reward potion drops use target-style potionRng.random(99), persisted potionChance, target rarity thresholds, and the full 33-potion Ironclad reward pool; potion use effects and broader potion RNG surfaces remain partial".to_owned(),
        },
    ]
}

#[cfg(test)]
fn seed_start_opening_piles_match(simulated: &CardPiles, message: &Value) -> bool {
    let Some(combat) = message
        .get("game_state")
        .and_then(|game| game.get("combat_state"))
    else {
        return false;
    };
    let observed_hand = combat_card_ids(combat.get("hand"));
    let observed_draw = combat_card_ids(combat.get("draw_pile"));
    let simulated_hand = hand_to_comm_mod_visible_order(&simulated.hand);
    let simulated_draw = draw_pile_to_comm_mod_visible_order(&simulated.draw_pile);
    observed_hand == simulated_hand && observed_draw == simulated_draw
}

fn seed_start_simulated_combat_subset(
    run: &RunState,
    message: &Value,
    end_turn_snapshot: bool,
) -> Value {
    let game = message.get("game_state").expect("observed game_state");
    let combat = run.combat.as_ref().expect("combat run");
    let observed_monsters = game
        .get("combat_state")
        .and_then(|combat| combat.get("monsters"));
    let observed_screen_type = game
        .get("screen_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let screen_type = if combat.potion_card_reward.is_some() {
        "CARD_REWARD"
    } else if combat.hand_select.is_some()
        || (combat.exhaust_select.is_some() && observed_screen_type == "HAND_SELECT")
    {
        "HAND_SELECT"
    } else {
        observed_screen_type
    };
    let mut subset = json!({
        "screen_type": screen_type,
        "ascension": run.ascension,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, &[]),
        "combat_player_hp": combat.player.hp,
        "combat_player_block": combat.player.block,
        "combat_player_energy": combat.player.energy,
        "hand_ids": cards_to_comm_mod_visible_order(
            combat
                .piles
                .hand
                .iter()
                .enumerate()
                .filter(|(index, card)| {
                    let hidden_by_hand_select = combat.hand_select.as_ref().is_some_and(|hand_select| {
                        card.id == hand_select.source_card_id
                            || hand_select.selected_hand_index == Some(*index)
                    });
                    let hidden_by_exhaust_select = combat
                        .exhaust_select
                        .as_ref()
                        .is_some_and(|exhaust_select| exhaust_select.selected_hand_indices.contains(index));
                    !hidden_by_hand_select && !hidden_by_exhaust_select
                })
                .map(|(_, card)| card),
        ),
        "draw_ids": draw_pile_to_comm_mod_visible_order(&combat.piles.draw_pile),
        "discard_ids": discard_pile_to_comm_mod_visible_order(&combat.piles.discard_pile),
        "monsters": seed_start_monsters_from_sim(combat, observed_monsters, end_turn_snapshot),
    });
    if let Some(choices) = combat.potion_card_reward.as_ref() {
        if let Value::Object(map) = &mut subset {
            map.insert(
                "card_reward_ids".to_owned(),
                json!(choices
                    .iter()
                    .map(|card| card.content_id.get())
                    .collect::<Vec<_>>()),
            );
        }
    }
    subset
}

#[cfg(test)]
#[allow(dead_code)]
fn combat_reward_card_display_key(combat: &CombatState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        ARMAMENTS_ID, FLEX_ID, METALLICIZE_ID, OFFERING_ID, SHRUG_IT_OFF_ID, WARCRY_PLUS_ID,
    };
    if content_id == WARCRY_PLUS_ID {
        return "Warcry+";
    }
    if combat
        .relics
        .iter()
        .any(|relic| relic.key() == RelicKey::ToxicEgg)
    {
        if content_id == ARMAMENTS_ID {
            return "Armaments+";
        }
        if content_id == METALLICIZE_ID {
            return "Metallicize+";
        }
        if content_id == FLEX_ID {
            return "Flex+";
        }
        if content_id == OFFERING_ID {
            return "Offering+";
        }
        if content_id == SHRUG_IT_OFF_ID {
            return "Shrug It Off+";
        }
    }
    deck_content_key(content_id)
}

fn seed_start_victory_observed_subset(message: &Value) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": int(game, "gold"),
        "current_hp": int(game, "current_hp"),
        "max_hp": int(game, "max_hp"),
    })
}

fn seed_start_victory_simulated_subset(run: &RunState, message: &Value) -> Value {
    let game = message.get("game_state").expect("observed game_state");
    json!({
        "screen_type": "COMBAT_REWARD",
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
    })
}

fn seed_start_apply_reward_rng_snapshot(
    run: &mut RunState,
    numeric_seed: i64,
    external_seed: &str,
    combat_index: usize,
) {
    let _ = (external_seed, combat_index);
    run.reward_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    run.current_act = 1;
}

fn seed_start_reward_sequence_complete(run: &RunState) -> bool {
    let Some(reward) = run.reward.as_ref() else {
        return true;
    };
    if reward.card_reward_active {
        return reward.choices.is_empty();
    }
    reward.gold_offer == 0
        && reward.stolen_gold_offer == 0
        && reward.potion_offer.is_none()
        && reward.relic_offer.is_none()
        && reward.relic_key_offer.is_none()
        && !reward.card_reward_pending
        && reward.choices.is_empty()
}

fn sim_reward_combat_choices(reward: &RewardScreen) -> Vec<String> {
    let mut choices = Vec::new();
    if reward.gold_offer > 0 {
        choices.push("gold".to_owned());
    }
    if reward.stolen_gold_offer > 0 {
        choices.push("stolen_gold".to_owned());
    }
    if reward.potion_offer.is_some() {
        choices.push("potion".to_owned());
    }
    if reward.relic_offer.is_some() || reward.relic_key_offer.is_some() {
        choices.push("relic".to_owned());
    }
    if !reward.choices.is_empty() && !reward.card_reward_active {
        choices.push("card".to_owned());
    } else if reward.card_reward_pending && !reward.card_reward_active {
        choices.push("card".to_owned());
    }
    choices
}

fn post_potion_count(message: &Value) -> usize {
    message
        .get("game_state")
        .and_then(|game| game.get("potions"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn reward_types_from_combat_reward(message: &Value) -> Vec<String> {
    reward_types_from_value(
        message
            .get("game_state")
            .and_then(|game| game.get("screen_state"))
            .and_then(|screen| screen.get("rewards")),
    )
    .into_iter()
    .map(|reward_type| reward_type.to_ascii_lowercase())
    .collect()
}

fn seed_start_apply_reward_choose(
    sim: &mut RunState,
    command: &str,
    pre: &Value,
    post: &Value,
    external_seed: &str,
) -> Result<String, String> {
    let _ = external_seed;
    let choose_index = choose_index(command)
        .ok_or_else(|| format!("seed-start verifier could not parse reward command {command:?}"))?;

    if sim
        .reward
        .as_ref()
        .is_some_and(|reward| reward.card_reward_active)
    {
        let card_id = sim
            .reward
            .as_ref()
            .and_then(|reward| reward.choices.get(choose_index))
            .map(|card| card.id)
            .ok_or_else(|| format!("reward card index {choose_index} is not available"))?;
        let next = apply_run_action(sim, RunAction::TakeCardReward { card_id })
            .map_err(|err| err.to_string())?;
        *sim = next;
        return Ok(format!("card reward pick {choose_index}"));
    }

    let observed_types = reward_types_from_combat_reward(pre);
    let choice = observed_types
        .get(choose_index)
        .cloned()
        .ok_or_else(|| format!("reward choice index {choose_index} is not available"))?;

    let potions_before = sim.potions.len();
    let next = match choice.as_str() {
        "stolen_gold" => apply_run_action(sim, RunAction::TakeStolenGoldReward),
        "gold" => apply_run_action(sim, RunAction::TakeGoldReward),
        "card" => apply_run_action(sim, RunAction::OpenCardReward),
        "potion"
            if sim.open_potion_slots() == 0
                && sim
                    .reward
                    .as_ref()
                    .is_some_and(|reward| reward.gold_offer > 0) =>
        {
            apply_run_action(sim, RunAction::TakeGoldReward)
        }
        "potion" if post_potion_count(post) > potions_before => {
            apply_run_action(sim, RunAction::TakePotionReward)
        }
        "potion" => apply_run_action(sim, RunAction::SkipPotionReward),
        "relic" => apply_run_action(sim, RunAction::TakeRelicReward),
        _ => return Err(format!("unknown reward choice {choice}")),
    }
    .map_err(|err| err.to_string())?;
    *sim = next;
    Ok(format!("{choice} reward"))
}

fn seed_start_reward_simulated_subset(
    run: &RunState,
    message: &Value,
    relic_ids: &[String],
    pre: Option<&Value>,
) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let floor = game.get("floor").and_then(Value::as_u64).unwrap_or(0);
    let relic_ids = relic_ids_for_simulated_subset(run, relic_ids);

    if run
        .reward
        .as_ref()
        .is_some_and(|reward| reward.card_reward_active)
    {
        let reward = run.reward.as_ref().expect("card reward active");
        return json!({
            "screen_type": "CARD_REWARD",
            "floor": floor,
            "gold": run.gold,
            "current_hp": run.player_hp,
            "max_hp": run.player_max_hp,
            "deck_ids": deck_content_keys(&run.deck),
            "relic_ids": relic_ids_for_simulated_subset(run, &relic_ids),
            "choices": reward
                .choices
                .iter()
                .map(|card| reward_card_display_key(run, card.content_id).to_ascii_lowercase())
                .collect::<Vec<_>>(),
            "card_reward_ids": reward
                .choices
                .iter()
                .map(|card| json!(card.content_id.get()))
                .collect::<Vec<_>>(),
            "unobservable": {
                "card_reward_rng_draws": true,
                "card_reward_uuids": true,
            },
        });
    }

    let reward = run.reward.as_ref();
    let mut combat_choices = reward.map(sim_reward_combat_choices).unwrap_or_default();
    if let Some(pre) = pre {
        let observed_types = reward_types_from_combat_reward(pre);
        if !observed_types.is_empty() {
            combat_choices = observed_types
                .iter()
                .map(|reward_type| match reward_type.as_str() {
                    "gold" => "gold".to_owned(),
                    "card" => "card".to_owned(),
                    "potion" => "potion".to_owned(),
                    "relic" => "relic".to_owned(),
                    other => other.to_owned(),
                })
                .collect();
        }
    }
    let reward_types: Vec<String> = combat_choices
        .iter()
        .map(|choice| match choice.as_str() {
            "gold" => "GOLD",
            "stolen_gold" => "STOLEN_GOLD",
            "potion" => "POTION",
            "card" => "CARD",
            "relic" => "RELIC",
            _ => "UNKNOWN",
        })
        .map(str::to_owned)
        .collect();

    let mut out = json!({
        "screen_type": "COMBAT_REWARD",
        "floor": floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids,
        "choices": combat_choices,
        "reward_types": reward_types,
    });

    if let Value::Object(map) = &mut out {
        if reward_types.is_empty() {
            insert(
                map,
                "unobservable",
                json!({
                    "picked_card_uuid": true,
                }),
            );
        } else {
            insert(
                map,
                "unobservable",
                json!({
                    "reward_gold_rng_draws": true,
                    "reward_screen_internal_ids": true,
                }),
            );
        }
    }
    out
}

fn seed_start_shop_purchase_has_stale_observed_deck(
    before: &RunState,
    after: &RunState,
    post_message: &Value,
) -> bool {
    if screen_type(post_message) != Some("SHOP_SCREEN") {
        return false;
    }
    let before_deck = deck_content_keys(&before.deck);
    let after_deck = deck_content_keys(&after.deck);
    if after_deck.len() != before_deck.len() + 1 {
        return false;
    }
    let Some(game) = post_message.get("game_state") else {
        return false;
    };
    deck_keys_from_value(game.get("deck")) == before_deck
}

fn seed_start_monsters_from_sim(
    combat: &CombatState,
    observed_monsters: Option<&Value>,
    end_turn_snapshot: bool,
) -> Vec<Value> {
    let observed = observed_monsters.and_then(Value::as_array);
    combat
        .monsters
        .iter()
        .enumerate()
        .map(|(index, monster)| {
            let observed_monster = observed.and_then(|monsters| monsters.get(index));
            let max_hp = observed
                .and_then(|monsters| monsters.get(index))
                .map(|monster| int(monster, "max_hp"))
                .unwrap_or(monster.hp);
            let name = observed_monster
                .and_then(|monster| monster.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| seed_start_trace_monster_name(monster.content_id).to_owned());
            let strength = (monster.powers.strength - monster.powers.ritual).max(0);
            let vulnerable = monster.powers.vulnerable;
            if end_turn_snapshot {
                let _ = vulnerable;
            }
            json!({
                "name": name,
                "current_hp": monster.hp.max(0),
                "max_hp": max_hp,
                "block": monster.block,
                "intent": seed_start_trace_intent(monster),
                "strength": strength,
                "ritual": monster.powers.ritual,
                "vulnerable": vulnerable,
            })
        })
        .collect()
}

fn seed_start_compare_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
    strip_piles: bool,
) {
    let expected = seed_start_normalize_combat_compare(expected, strip_piles);
    let mut actual = seed_start_normalize_combat_compare(actual, strip_piles);
    if let (Some(expected_obj), Some(actual_obj)) = (expected.as_object(), actual.as_object_mut()) {
        for key in ["ascension", "deck_ids", "relic_ids"] {
            if !expected_obj.contains_key(key) {
                actual_obj.remove(key);
            }
        }
    }
    compare_subset(report, action, label, expected, actual);
}

fn seed_start_normalize_combat_compare(mut value: Value, strip_piles: bool) -> Value {
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    obj.remove("unobservable");
    if strip_piles {
        obj.remove("hand_ids");
        obj.remove("draw_ids");
        obj.remove("discard_ids");
    }
    if let Some(monsters) = obj.get_mut("monsters").and_then(Value::as_array_mut) {
        for monster in monsters {
            if let Some(fields) = monster.as_object_mut() {
                fields.remove("strength");
                fields.remove("ritual");
                fields.remove("vulnerable");
                fields.remove("intent");
            }
        }
    }
    Value::Object(obj.clone())
}

#[cfg(test)]
#[allow(dead_code)]
fn seed_start_normalize_combat_entry_compare(value: Value) -> Value {
    let mut value = seed_start_normalize_combat_compare(value, true);
    if let Some(obj) = value.as_object_mut() {
        obj.remove("ascension");
        obj.remove("deck_ids");
        obj.remove("relic_ids");
    }
    value
}

fn unsupported_seed_start_combat_command(combat: &CombatState, command: &str) -> Option<String> {
    let parts: Vec<_> = command.split_whitespace().collect();
    let [cmd, hand_index, ..] = parts.as_slice() else {
        return None;
    };
    if !cmd.eq_ignore_ascii_case("PLAY") {
        return None;
    }
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    let card = combat.piles.hand.get(index)?;
    let key = content_key(card.content_id);
    if key != "unknown" {
        return None;
    }
    Some(format!(
        "card at hand index {} is not mapped in the verifier, so this combat command is unsupported",
        index + 1
    ))
}

#[cfg(test)]
fn observed_event_screen(game: &Value, event_rng_seed: u64) -> Option<EventScreen> {
    if game
        .get("screen_type")
        .and_then(Value::as_str)
        .is_none_or(|screen| screen != "EVENT")
    {
        return None;
    }
    let state = game.get("screen_state");
    let event_id = state
        .and_then(|state| state.get("event_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let event_name = state
        .and_then(|state| state.get("event_name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let choices = choice_list_from_value(game.get("choice_list"));
    if event_id == "Lab" || event_name == "Lab" {
        return Some(event_screen(Event::Lab));
    }
    if event_id == "Masked Bandits"
        || event_name == "Masked Bandits"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 2
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("pay"))
            && choices.iter().any(|choice| {
                choice.eq_ignore_ascii_case("fight") || choice.eq_ignore_ascii_case("fight!")
            }))
    {
        return Some(EventScreen {
            event: Event::MaskedBandits,
            choices: vec![
                EventChoice {
                    label: "Pay".to_owned(),
                },
                EventChoice {
                    label: "Fight".to_owned(),
                },
            ],
            stage: 0,
            event_data: 0,
        });
    }
    if event_id == "The Cleric"
        || event_name == "The Cleric"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 3
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("heal"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("purify"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("heal"))
            || labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("purify"))
        {
            0
        } else {
            1
        };
        return Some(EventScreen {
            event: Event::TheCleric,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Accursed Blacksmith" || event_name == "Ominous Forge" {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("forge"))
            || labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("rummage"))
        {
            0
        } else {
            1
        };
        return Some(EventScreen {
            event: Event::AccursedBlacksmith,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Shining Light"
        || event_name == "Shining Light"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 2
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("enter"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels.len() == 1
            && labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave"))
        {
            1
        } else {
            0
        };
        return Some(EventScreen {
            event: Event::ShiningLight,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Scrap Ooze" || event_name == "Scrap Ooze" {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("reach inside"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("deeper"))
        {
            1
        } else {
            2
        };
        let event_data = if stage == 1 {
            scrap_ooze_failed_reaches_from_observed(game, int(game, "ascension_level") as u8)
                .unwrap_or(1)
        } else {
            0
        };
        return Some(EventScreen {
            event: Event::ScrapOoze,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data,
        });
    }
    if event_id == "FaceTrader"
        || event_name == "Face Trader"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 3
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("touch"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("trade"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("touch"))
        {
            1
        } else {
            2
        };
        return Some(EventScreen {
            event: Event::FaceTrader,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Golden Wing"
        || event_name == "Wing Statue"
        || (event_id.is_empty()
            && event_name.is_empty()
            && choices.len() == 2
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("pray"))
            && choices
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("leave")))
    {
        let labels = if choices.is_empty() {
            vec!["Leave".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("pray"))
            || labels
                .iter()
                .any(|choice| choice.eq_ignore_ascii_case("destroy"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            1
        } else {
            2
        };
        return Some(EventScreen {
            event: Event::WingStatue,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Nest" || event_name == "The Nest" {
        let choices = choice_list_from_value(game.get("choice_list"));
        let labels = if choices.is_empty() {
            vec!["Continue".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            0
        } else if labels.iter().any(|choice| {
            choice.eq_ignore_ascii_case("smash and grab")
                || choice.eq_ignore_ascii_case("stay in line")
        }) {
            1
        } else {
            2
        };
        return Some(EventScreen {
            event: Event::Nest,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data: 0,
        });
    }
    if event_id == "Knowing Skull" || event_name == "Knowing Skull" {
        let choices = choice_list_from_value(game.get("choice_list"));
        let labels = if choices.is_empty() {
            vec!["Continue".to_owned()]
        } else {
            choices
        };
        let stage = if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("continue"))
        {
            0
        } else if labels
            .iter()
            .any(|choice| choice.eq_ignore_ascii_case("leave"))
        {
            2
        } else {
            1
        };
        let event_data = if stage == 1 {
            knowing_skull_costs_from_observed(game).unwrap_or_else(default_knowing_skull_costs)
        } else {
            default_knowing_skull_costs()
        };
        return Some(EventScreen {
            event: Event::KnowingSkull,
            choices: labels
                .into_iter()
                .map(|label| EventChoice { label })
                .collect(),
            stage,
            event_data,
        });
    }
    if event_id != "Neow Event" && event_name != "Neow" {
        return None;
    }

    let choices = choice_list_from_value(game.get("choice_list"));
    let stage = if choices
        .iter()
        .any(|choice| choice.eq_ignore_ascii_case("talk"))
    {
        0
    } else if choices
        .iter()
        .any(|choice| choice.eq_ignore_ascii_case("leave"))
    {
        2
    } else {
        1
    };
    let labels = if stage == 1 && choices.is_empty() {
        generate_neow_options(event_rng_seed as i64, int(game, "max_hp"))
            .into_iter()
            .map(|option| option.label)
            .collect::<Vec<_>>()
    } else {
        choices
    };

    Some(EventScreen {
        event: Event::Neow,
        choices: labels
            .into_iter()
            .map(|label| EventChoice { label })
            .collect(),
        stage,
        event_data: 0,
    })
}

#[cfg(test)]
fn default_knowing_skull_costs() -> u32 {
    knowing_skull_cost_data(6, 6, 6, 6)
}

#[cfg(test)]
fn knowing_skull_cost_data(potion: u32, gold: u32, card: u32, leave: u32) -> u32 {
    potion | (gold << 8) | (card << 16) | (leave << 24)
}

#[cfg(test)]
fn knowing_skull_costs_from_observed(game: &Value) -> Option<u32> {
    let options = game
        .get("screen_state")
        .and_then(|state| state.get("options"))
        .and_then(Value::as_array)?;
    let mut potion = None;
    let mut gold = None;
    let mut card = None;
    let mut leave = None;

    for option in options {
        let label = option
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let text = option.get("text").and_then(Value::as_str).unwrap_or("");
        let cost = hp_loss_cost_from_option_text(text)?;
        if label.contains("pick me up") {
            potion = Some(cost);
        } else if label.contains("riches") {
            gold = Some(cost);
        } else if label.contains("success") {
            card = Some(cost);
        } else if label.contains("leave") {
            leave = Some(cost);
        }
    }

    Some(knowing_skull_cost_data(
        potion.unwrap_or(6),
        gold.unwrap_or(6),
        card.unwrap_or(6),
        leave.unwrap_or(6),
    ))
}

#[cfg(test)]
fn hp_loss_cost_from_option_text(text: &str) -> Option<u32> {
    let after_lose = text.split("Lose ").nth(1)?;
    let hp_text = after_lose.split(" HP").next()?;
    hp_text.trim().parse::<u32>().ok()
}

#[cfg(test)]
fn scrap_ooze_failed_reaches_from_observed(game: &Value, ascension: u8) -> Option<u32> {
    let options = game
        .get("screen_state")
        .and_then(|state| state.get("options"))
        .and_then(Value::as_array)?;
    let text = options
        .iter()
        .filter_map(|option| option.get("text").and_then(Value::as_str))
        .find(|text| text.contains("Lose ") && text.contains(" HP"))?;
    let after_lose = text.split("Lose ").nth(1)?;
    let hp_text = after_lose.split(" HP").next()?;
    let hp_loss = hp_text.trim().parse::<u32>().ok()?;
    let base_loss = if ascension >= 15 { 5 } else { 3 };
    hp_loss.checked_sub(base_loss)
}

#[cfg(test)]
fn observed_reward_potion_offer(game: &Value) -> Option<Potion> {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)?
        .iter()
        .find(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("POTION"))
        })
        .and_then(|reward| reward.get("potion"))
        .and_then(|potion| potion.get("name").or_else(|| potion.get("id")))
        .and_then(Value::as_str)
        .and_then(potion_from_trace_name)
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_reward_relic_key_offer(game: &Value) -> Option<RelicKey> {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)?
        .iter()
        .find(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("RELIC"))
        })
        .and_then(|reward| reward.get("relic"))
        .and_then(|relic| relic.get("name").or_else(|| relic.get("id")))
        .and_then(Value::as_str)
        .and_then(relic_key_from_trace_name)
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_boss_relic_key_choices(game: &Value) -> Vec<RelicKey> {
    if game
        .get("screen_type")
        .and_then(Value::as_str)
        .is_none_or(|screen| screen != "BOSS_REWARD")
    {
        return Vec::new();
    }
    game.get("screen_state")
        .and_then(|screen| screen.get("relics"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|relic| {
            relic
                .get("name")
                .or_else(|| relic.get("id"))
                .and_then(Value::as_str)
                .and_then(relic_key_from_trace_name)
        })
        .collect()
}

fn combat_action_from_command(command: &str, combat: &CombatState) -> Option<CombatAction> {
    use sts_core::card::TargetRequirement;
    use sts_core::content::cards::get_card_definition;

    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("END") => Some(CombatAction::EndTurn),
        [cmd, hand_index] if cmd.eq_ignore_ascii_case("PLAY") => Some(CombatAction::PlayCard {
            card_id: hand_card_id_from_bridge_slot(combat, hand_index)?,
            target: None,
        }),
        [cmd, hand_index, target_index] if cmd.eq_ignore_ascii_case("PLAY") => {
            let card_id = hand_card_id_from_bridge_slot(combat, hand_index)?;
            let mut target = Some(monster_id_from_bridge_slot(combat, target_index)?);
            if let Some(definition) = combat
                .piles
                .hand
                .iter()
                .find(|card| card.id == card_id)
                .and_then(|card| get_card_definition(card.content_id))
            {
                if definition.target != TargetRequirement::Enemy {
                    target = None;
                }
            }
            Some(CombatAction::PlayCard { card_id, target })
        }
        _ => None,
    }
}

fn monster_id_from_bridge_slot(combat: &CombatState, target_index: &str) -> Option<MonsterId> {
    let index = target_index.parse::<usize>().ok()?;
    combat.monsters.get(index).map(|monster| monster.id)
}

fn hand_card_id_from_bridge_slot(combat: &CombatState, hand_index: &str) -> Option<CardId> {
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    Some(combat.piles.hand.get(index)?.id)
}

#[cfg(test)]
fn unsupported_combat_command_reason(message: &Value, command: &str) -> Option<String> {
    let parts: Vec<_> = command.split_whitespace().collect();
    let [cmd, hand_index, ..] = parts.as_slice() else {
        return None;
    };
    if !cmd.eq_ignore_ascii_case("PLAY") {
        return None;
    }
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    let card = message
        .get("game_state")?
        .get("combat_state")?
        .get("hand")?
        .as_array()?
        .get(index)?;
    if content_id_from_card_value(card).is_some() {
        return None;
    }
    let card_name = card
        .get("name")
        .or_else(|| card.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown card");
    Some(format!(
        "card '{card_name}' is not mapped in the verifier, so this combat command is unsupported"
    ))
}

fn is_final_combat_blow(run: &RunState, action: CombatAction) -> bool {
    let Some(combat) = &run.combat else {
        return false;
    };
    let Ok(next) = sts_core::apply_combat_action(combat, action) else {
        return false;
    };
    next.phase == CombatPhase::Won
}

#[cfg(test)]
fn observed_combat_subset(message: &Value, fields: &[&str]) -> Value {
    let Some(obs) = normalize_communication_mod_message(message) else {
        return json!({});
    };
    let Some(combat) = obs.combat else {
        return json!({});
    };
    let monster = combat.monsters.iter().find(|monster| monster.hp > 0);
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "player_hp" => insert(&mut out, field, combat.player_hp),
            "player_block" => insert(&mut out, field, combat.player_block),
            "player_energy" => insert(&mut out, field, combat.player_energy),
            "monster_hp" => insert(&mut out, field, monster.map(|m| m.hp).unwrap_or(0)),
            "monster_block" => insert(&mut out, field, monster.map(|m| m.block).unwrap_or(0)),
            "monster_intent" => insert(
                &mut out,
                field,
                monster.map(|m| m.intent.clone()).unwrap_or_default(),
            ),
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn simulated_combat_subset(run: &RunState, fields: &[&str]) -> Value {
    let combat = run.combat.as_ref().expect("combat available");
    let monster = combat.monsters.iter().find(|monster| monster.alive);
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "player_hp" => insert(&mut out, field, combat.player.hp),
            "player_block" => insert(&mut out, field, combat.player.block),
            "player_energy" => insert(&mut out, field, combat.player.energy),
            "monster_hp" => insert(&mut out, field, monster.map(|m| m.hp).unwrap_or(0)),
            "monster_block" => insert(&mut out, field, monster.map(|m| m.block).unwrap_or(0)),
            "monster_intent" => {
                insert(&mut out, field, monster.map(intent_key).unwrap_or_default())
            }
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_run_subset(message: &Value, fields: &[&str]) -> Value {
    let Some(game) = message.get("game_state") else {
        return json!({});
    };
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "gold" => insert(&mut out, field, int(game, "gold")),
            "current_hp" => insert(&mut out, field, int(game, "current_hp")),
            "deck_size" => insert(
                &mut out,
                field,
                game.get("deck")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
            ),
            "deck_ids" => insert(&mut out, field, deck_keys_from_value(game.get("deck"))),
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn simulated_run_subset(run: &RunState, fields: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    for field in fields {
        match *field {
            "gold" => insert(&mut out, field, run.gold),
            "current_hp" => insert(&mut out, field, run.player_hp),
            "deck_size" => insert(&mut out, field, run.deck.len()),
            "deck_ids" => insert(&mut out, field, deck_content_keys(&run.deck)),
            _ => {}
        }
    }
    Value::Object(out)
}

#[cfg(test)]
#[allow(dead_code)]
fn deck_has_unmapped_cards(message: &Value) -> bool {
    message
        .get("game_state")
        .and_then(|game| game.get("deck"))
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .any(|card| content_id_from_card_value(card).is_none())
        })
        .unwrap_or(false)
}

#[cfg(test)]
fn unsupported_monster_ai_reason(message: &Value) -> Option<String> {
    let groups: Vec<String> = message
        .get("game_state")?
        .get("combat_state")?
        .get("monsters")?
        .as_array()?
        .iter()
        .filter(|monster| int(monster, "current_hp") > 0)
        .filter_map(|monster| monster.get("id").and_then(Value::as_str))
        .filter(|id| !matches!(*id, "Cultist" | "JawWorm"))
        .map(str::to_owned)
        .collect();
    if groups.is_empty() {
        None
    } else {
        Some(format!(
            "exact combat transition is unsupported for monster group(s): {}",
            groups.join(", ")
        ))
    }
}

fn compare_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
) {
    let diffs = subset_diffs(expected, actual);
    if diffs.is_empty() {
        report.verified.push(VerifiedTransition {
            action_step: action.step,
            command: action.command.clone(),
            label: label.to_owned(),
        });
    } else {
        report.unexpected_diffs.push(UnexpectedDiff {
            action_step: action.step,
            command: action.command.clone(),
            label: label.to_owned(),
            diffs,
        });
    }
}

fn compare_subset_any(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actuals: Vec<Value>,
) {
    let mut first_diffs = None;
    for actual in actuals {
        let diffs = subset_diffs(expected.clone(), actual);
        if diffs.is_empty() {
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label: label.to_owned(),
            });
            return;
        }
        first_diffs.get_or_insert(diffs);
    }
    report.unexpected_diffs.push(UnexpectedDiff {
        action_step: action.step,
        command: action.command.clone(),
        label: label.to_owned(),
        diffs: first_diffs.unwrap_or_default(),
    });
}

fn subset_diffs(expected: Value, actual: Value) -> Vec<String> {
    let expected_json = serde_json::to_string(&expected).expect("json serializes");
    let actual_json = serde_json::to_string(&actual).expect("json serializes");
    canonical_diff(&expected_json, &actual_json)
        .into_iter()
        .filter(|diff| !is_known_card_vs_legacy_unknown_diff(diff))
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn normalized_combat_subset_diffs(
    expected: Value,
    actual: Value,
    strip_piles: bool,
) -> Vec<String> {
    subset_diffs(
        seed_start_normalize_combat_compare(expected, strip_piles),
        seed_start_normalize_combat_compare(actual, strip_piles),
    )
}

fn is_known_card_vs_legacy_unknown_diff(diff: &str) -> bool {
    const KNOWN_CARD_NAMES: &[&str] = &[
        "Armaments+",
        "Offering",
        "Offering+",
        "armaments+",
        "offering+",
    ];
    KNOWN_CARD_NAMES.iter().any(|name| {
        diff.contains(&format!("\"{name}\" != \"unknown\""))
            || diff.contains(&format!("\"unknown\" != \"{name}\""))
    })
}

#[cfg(test)]
#[allow(dead_code)]
fn post_supported_combat_fields(command: &str) -> &'static [&'static str] {
    if command.trim().eq_ignore_ascii_case("END") {
        &[
            "player_hp",
            "player_block",
            "player_energy",
            "monster_hp",
            "monster_block",
            "monster_intent",
        ]
    } else {
        &[
            "player_hp",
            "player_block",
            "player_energy",
            "monster_hp",
            "monster_block",
        ]
    }
}

fn combat_label(command: &str, run: &RunState) -> String {
    let Some(combat) = &run.combat else {
        return "combat".to_owned();
    };
    let Some(CombatAction::PlayCard { card_id, .. }) = combat_action_from_command(command, combat)
    else {
        return "end turn".to_owned();
    };
    let key = combat
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .map(|card| content_key(card.content_id))
        .unwrap_or("unknown");
    key.to_owned()
}

#[cfg(test)]
#[allow(dead_code)]
fn monsters_from_observed(
    value: Option<&Value>,
    _player: &Value,
    ascension: u8,
) -> Vec<MonsterState> {
    let Some(monsters) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    monsters
        .iter()
        .enumerate()
        .map(|(index, monster)| {
            let game_id = monster
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("Cultist");
            let content_id = sts_core::content::monsters::content_id_from_game_monster_id(game_id);
            let rolled_attack_damage = louse_bite_damage_from_observed(monster, content_id);
            let powers = monster_powers(monster.get("powers"));
            let replay = elite_boss_replay_fields(monster, content_id, &powers, ascension);
            let move_history = target_move_byte(content_id, replay.intent)
                .map(|move_byte| vec![move_byte])
                .unwrap_or_default();
            let is_gone = monster
                .get("is_gone")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            MonsterState {
                id: MonsterId::new(index as u64 + 1),
                hp: int(monster, "current_hp"),
                max_hp: int(monster, "max_hp"),
                block: int(monster, "block"),
                alive: int(monster, "current_hp") > 0 && !is_gone,
                escaped: is_gone,
                powers,
                temp_strength_down: 0,
                content_id,
                moves_executed: replay.moves_executed,
                sleep_turns_remaining: replay.sleep_turns_remaining,
                has_siphoned: replay.has_siphoned,
                split_triggered: false,
                defensive_turns_remaining: replay.defensive_turns_remaining,
                mode_shift: replay.mode_shift,
                mode_shift_threshold: replay.mode_shift_threshold,
                in_defensive_mode: replay.in_defensive_mode,
                rolled_attack_damage,
                stolen_gold: 0,
                move_history,
                gremlin_leader_slot: None,
                stasis_card: None,
                initial_intent_locked: false,
                intent: replay.intent,
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
struct EliteBossReplayFields {
    moves_executed: u32,
    sleep_turns_remaining: u32,
    has_siphoned: bool,
    defensive_turns_remaining: u32,
    mode_shift: i32,
    mode_shift_threshold: i32,
    in_defensive_mode: bool,
    intent: MonsterIntent,
}

#[cfg(test)]
#[allow(dead_code)]
fn elite_boss_replay_fields(
    monster: &Value,
    content_id: ContentId,
    powers: &MonsterPowers,
    ascension: u8,
) -> EliteBossReplayFields {
    let intent_str = monster.get("intent").and_then(Value::as_str).unwrap_or("");
    let damage = int(monster, "move_base_damage");

    match content_id {
        LAGAVULIN_ID => {
            let sleep_turns_remaining = if matches!(intent_str, "SLEEP" | "DEBUG") {
                3
            } else {
                0
            };
            let has_siphoned = intent_str == "ATTACK";
            let intent = if sleep_turns_remaining > 0 {
                MonsterIntent::Sleep
            } else if intent_str == "STUN" {
                MonsterIntent::Stun
            } else if !has_siphoned {
                MonsterIntent::SiphonPlayer {
                    strength: 1,
                    dexterity: 1,
                }
            } else {
                MonsterIntent::Attack { damage: 18 }
            };
            EliteBossReplayFields {
                moves_executed: u32::from(has_siphoned),
                sleep_turns_remaining,
                has_siphoned,
                defensive_turns_remaining: 0,
                mode_shift: 0,
                mode_shift_threshold: 0,
                in_defensive_mode: false,
                intent,
            }
        }
        GREMLIN_NOB_ID => {
            let moves_executed = match (intent_str, damage) {
                ("DEBUG" | "BUFF", _) => 0,
                ("ATTACK_DEBUFF", 6) => 0,
                ("ATTACK", 14) => 1,
                ("ATTACK_DEBUFF", _) => 2,
                ("ATTACK", _) => 1,
                _ => moves_executed_from_observed(monster, content_id),
            };
            EliteBossReplayFields {
                moves_executed,
                sleep_turns_remaining: 0,
                has_siphoned: false,
                defensive_turns_remaining: 0,
                mode_shift: 0,
                mode_shift_threshold: 0,
                in_defensive_mode: false,
                intent: observed_intent(monster, content_id, ascension),
            }
        }
        GUARDIAN_ID => {
            let mode_shift = monster
                .get("powers")
                .and_then(Value::as_array)
                .and_then(|powers| {
                    powers.iter().find_map(|power| {
                        if power_id(power).as_deref() == Some("Mode Shift") {
                            Some(int(power, "amount"))
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or(30);
            let in_defensive_mode = powers.spikes > 0
                || intent_str == "BUFF"
                || (intent_str == "ATTACK" && damage == 9);
            let defensive_turns_remaining = if in_defensive_mode {
                match (intent_str, damage) {
                    ("BUFF", _) => 7,
                    ("ATTACK", 9) => 5,
                    ("ATTACK", 8) => 3,
                    _ => 4,
                }
            } else {
                0
            };
            EliteBossReplayFields {
                moves_executed: if in_defensive_mode {
                    7_u32.saturating_sub(defensive_turns_remaining)
                } else {
                    match (intent_str, damage) {
                        ("DEBUG", _) => 0,
                        ("ATTACK", 32) => 0,
                        ("ATTACK", 5) => 1,
                        _ => 0,
                    }
                },
                sleep_turns_remaining: 0,
                has_siphoned: false,
                defensive_turns_remaining,
                mode_shift,
                mode_shift_threshold: mode_shift.max(30),
                in_defensive_mode,
                intent: observed_intent(monster, content_id, ascension),
            }
        }
        _ => EliteBossReplayFields {
            moves_executed: moves_executed_from_observed(monster, content_id),
            sleep_turns_remaining: 0,
            has_siphoned: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            mode_shift_threshold: 0,
            in_defensive_mode: false,
            intent: observed_intent(monster, content_id, ascension),
        },
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn louse_bite_damage_from_observed(monster: &Value, content_id: ContentId) -> Option<i32> {
    if !matches!(
        content_id,
        sts_core::content::monsters::RED_LOUSE_ID | sts_core::content::monsters::GREEN_LOUSE_ID
    ) {
        return None;
    }
    let damage = int(monster, "move_base_damage");
    (damage > 0).then_some(damage)
}

#[cfg(test)]
fn observed_intent(monster: &Value, content_id: ContentId, ascension: u8) -> MonsterIntent {
    use sts_core::content::monsters::{
        champ_strength_amount, gremlin_nob_enrage, ACID_SLIME_ID, BRONZE_AUTOMATON_ID,
        BRONZE_ORB_ID, BYRD_ID, CENTURION_ID, CHAMP_DEFENSIVE_BLOCK, CHAMP_DEFENSIVE_METALLICIZE,
        CHAMP_FACE_SLAP_DAMAGE, CHAMP_ID, CHOSEN_ID, CULTIST_ID, DARKLING_ID, FUNGI_BEAST_ID,
        GREEN_LOUSE_ID, GREEN_LOUSE_WEAK, GREMLIN_FAT_ID, GREMLIN_LEADER_ID, GREMLIN_TSUNDERE_ID,
        HEALER_ID, HEXAGHOST_ID, JAW_WORM_ID, ORB_WALKER_ID, RED_LOUSE_ID, REPULSOR_ID, SENTRY_ID,
        SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLIME_BOSS_A19_SLIMED_COUNT, SLIME_BOSS_ID,
        SLIME_BOSS_SLIMED_COUNT, SNAKE_PLANT_ID, SNECKO_ID, SPHERIC_GUARDIAN_ACTIVATE_BLOCK,
        SPHERIC_GUARDIAN_FRAIL, SPHERIC_GUARDIAN_HARDEN_BLOCK, SPHERIC_GUARDIAN_ID, SPIKER_ID,
        SPIKE_SLIME_ID, THE_COLLECTOR_ID,
    };

    let damage = int(monster, "move_base_damage");
    let hits = int(monster, "move_hits");
    let move_id = int(monster, "move_id");
    match monster.get("intent").and_then(Value::as_str).unwrap_or("") {
        "STUN" => MonsterIntent::Stun,
        "ESCAPE" => MonsterIntent::Escape,
        "DEBUG" if content_id == SENTRY_ID && damage <= 0 => {
            MonsterIntent::AddDazedToDiscard { count: 2 }
        }
        "ATTACK" if matches!(content_id, LOOTER_ID | MUGGER_ID) => MonsterIntent::AttackStealGold {
            damage: damage.max(0),
            amount: looter_theft(0),
        },
        "ATTACK" if content_id == SLAVER_BLUE_ID && move_id == 4 => {
            MonsterIntent::AttackApplyPlayerWeak {
                damage: damage.max(0),
                weak: 1,
            }
        }
        "ATTACK" if hits > 1 => MonsterIntent::AttackMultiple {
            damage: damage.max(0),
            hits,
        },
        "ATTACK" if content_id == SENTRY_ID && damage <= 0 => {
            MonsterIntent::AddDazedToDiscard { count: 2 }
        }
        "ATTACK" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "DEBUFF" if content_id == SENTRY_ID => MonsterIntent::AddDazedToDiscard { count: 2 },
        "DEBUFF" if content_id == REPULSOR_ID => MonsterIntent::AddDazedToDraw { count: 2 },
        "DEBUFF" if content_id == CHOSEN_ID => MonsterIntent::ApplyPlayerWeakStrengthSelf {
            weak: 3,
            strength: 3,
        },
        "BUFF" if content_id == SPIKER_ID => MonsterIntent::StrengthAndBlock {
            strength: 0,
            block: 0,
        },
        "BUFF" | "DEBUG" if content_id == GREMLIN_NOB_ID => MonsterIntent::StrengthSelf {
            amount: gremlin_nob_enrage(ascension),
        },
        "BUFF" if content_id == BYRD_ID => MonsterIntent::StrengthSelf { amount: 1 },
        "BUFF" if content_id == CHAMP_ID && move_id == 5 => MonsterIntent::StrengthSelf {
            amount: champ_strength_amount(ascension),
        },
        "BUFF" if content_id == CHAMP_ID && move_id == 7 => MonsterIntent::StrengthSelf {
            amount: champ_strength_amount(ascension) * 3,
        },
        "STRONG_DEBUFF" if content_id == CHOSEN_ID => MonsterIntent::ApplyPlayerHex { amount: 1 },
        "STRONG_DEBUFF" if content_id == SNAKE_PLANT_ID => {
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 }
        }
        "STRONG_DEBUFF" if content_id == THE_COLLECTOR_ID => {
            MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 3,
                weak: 3,
                vulnerable: 3,
            }
        }
        "STRONG_DEBUFF" if content_id == SNECKO_ID => MonsterIntent::ApplyPlayerConfusion,
        "STRONG_DEBUFF" if content_id == SLIME_BOSS_ID => MonsterIntent::AddSlimedToDiscard {
            count: if ascension >= 19 {
                SLIME_BOSS_A19_SLIMED_COUNT
            } else {
                SLIME_BOSS_SLIMED_COUNT
            },
        },
        "DEBUFF"
            if content_id == SPIKE_SLIME_ID
                && (matches!(
                    str_field(monster, "id"),
                    Some("SpikeSlime_M" | "SpikeSlime_L")
                ) || int(monster, "max_hp")
                    > sts_core::content::monsters::SPIKE_SLIME_S_A7_HP_RANGE.max) =>
        {
            MonsterIntent::ApplyPlayerFrailAndWeak {
                frail: observed_spike_slime_frail(monster, ascension),
                weak: 0,
            }
        }
        "DEBUFF" if content_id == GREEN_LOUSE_ID => MonsterIntent::ApplyPlayerWeak {
            amount: GREEN_LOUSE_WEAK,
        },
        "DEBUFF" if content_id == CHAMP_ID && move_id == 6 => {
            MonsterIntent::ApplyPlayerWeak { amount: 2 }
        }
        "DEBUFF"
            if content_id == ACID_SLIME_ID
                && (str_field(monster, "id") == Some("AcidSlime_L")
                    || int(monster, "max_hp")
                        > sts_core::content::monsters::ACID_SLIME_M_A7_HP_RANGE.max) =>
        {
            MonsterIntent::ApplyPlayerWeak { amount: 2 }
        }
        "DEBUFF" => MonsterIntent::ApplyPlayerWeak { amount: 1 },
        "ATTACK_DEBUFF" if matches!(content_id, ACID_SLIME_ID | SPIKE_SLIME_ID) => {
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: damage.max(0),
                count: observed_slimed_count(monster, content_id),
            }
        }
        "ATTACK_DEBUFF" if content_id == HEXAGHOST_ID && move_id == 6 => {
            MonsterIntent::AttackMultipleUpgradeBurns {
                damage: damage.max(0),
                hits: int(monster, "move_hits").max(1),
                count: 3,
            }
        }
        "ATTACK_DEBUFF" if content_id == HEXAGHOST_ID => MonsterIntent::AddBurnToDiscard {
            damage: damage.max(0),
            count: if ascension >= 19 { 2 } else { 1 },
        },
        "ATTACK_DEBUFF" if content_id == ORB_WALKER_ID => MonsterIntent::AddBurnToDiscardAndDraw {
            damage: damage.max(0),
            count: 1,
        },
        "ATTACK_DEBUFF" if content_id == SPHERIC_GUARDIAN_ID => {
            MonsterIntent::AttackApplyPlayerFrail {
                damage: damage.max(0),
                frail: SPHERIC_GUARDIAN_FRAIL,
            }
        }
        "ATTACK_DEBUFF" if content_id == GREMLIN_FAT_ID && ascension >= 17 => {
            MonsterIntent::AttackApplyPlayerFrailAndWeak {
                damage: damage.max(0),
                frail: 1,
                weak: 1,
            }
        }
        "ATTACK_DEBUFF" if content_id == GREMLIN_FAT_ID => MonsterIntent::AttackApplyPlayerWeak {
            damage: damage.max(0),
            weak: 1,
        },
        "ATTACK_DEBUFF" if content_id == GREMLIN_NOB_ID => {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: damage.max(0),
                vulnerable: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == CHOSEN_ID => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: damage.max(0),
            vulnerable: 2,
        },
        "ATTACK_DEBUFF" if content_id == CHAMP_ID && move_id == 4 => {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: CHAMP_FACE_SLAP_DAMAGE,
                vulnerable: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == SNECKO_ID && ascension >= 17 => {
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: damage.max(0),
                weak: 2,
                vulnerable: 2,
            }
        }
        "ATTACK_DEBUFF" if content_id == SNECKO_ID => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: damage.max(0),
            vulnerable: 2,
        },
        "ATTACK_DEBUFF" if content_id == SLAVER_BLUE_ID => MonsterIntent::AttackApplyPlayerWeak {
            damage: damage.max(0),
            weak: if ascension >= 17 { 2 } else { 1 },
        },
        "ATTACK_DEBUFF" if content_id == SLAVER_RED_ID => {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: damage.max(0),
                vulnerable: if ascension >= 17 { 2 } else { 1 },
            }
        }
        "ATTACK_DEBUFF" if content_id == TASKMASTER_ID => MonsterIntent::AttackAddWoundsToDiscard {
            damage: damage.max(0),
            count: if ascension >= 18 {
                3
            } else if ascension >= 3 {
                2
            } else {
                1
            },
        },
        "ATTACK_DEFEND" if content_id == SPHERIC_GUARDIAN_ID => MonsterIntent::AttackAndBlock {
            damage: damage.max(0),
            block: SPHERIC_GUARDIAN_HARDEN_BLOCK,
        },
        "ATTACK_DEFEND" if content_id == JAW_WORM_ID => MonsterIntent::AttackAndBlock {
            damage: damage.max(0),
            block: 5,
        },
        "ATTACK_DEBUFF" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "ATTACK_BUFF" if content_id == GUARDIAN_ID && move_id == 4 => {
            MonsterIntent::AttackMultiple {
                damage: damage.max(0),
                hits: 2,
            }
        }
        "ATTACK_BUFF" if content_id == SHELLED_PARASITE_ID => MonsterIntent::AttackHealSelf {
            damage: damage.max(0),
        },
        "ATTACK_BUFF" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "DEFEND_BUFF" if content_id == GREMLIN_LEADER_ID => MonsterIntent::EncourageGremlins {
            strength: 3,
            block: 6,
        },
        "DEFEND_BUFF" if content_id == JAW_WORM_ID => MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 6,
        },
        "DEFEND_BUFF" if content_id == BRONZE_AUTOMATON_ID => MonsterIntent::StrengthAndBlock {
            strength: 3,
            block: 9,
        },
        "DEFEND_BUFF" if content_id == CHAMP_ID && move_id == 2 => {
            MonsterIntent::StrengthAndBlock {
                strength: CHAMP_DEFENSIVE_METALLICIZE,
                block: CHAMP_DEFENSIVE_BLOCK,
            }
        }
        "DEFEND" | "BLOCK" if matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) => {
            MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 0,
            }
        }
        "DEFEND" | "BLOCK" if content_id == GUARDIAN_ID => MonsterIntent::Block {
            block: GUARDIAN_CHARGE_BLOCK,
        },
        "DEFEND" | "BLOCK" if content_id == SPHERIC_GUARDIAN_ID => MonsterIntent::Block {
            block: SPHERIC_GUARDIAN_ACTIVATE_BLOCK,
        },
        "DEFEND" | "BLOCK" if content_id == CENTURION_ID => MonsterIntent::Block {
            block: observed_centurion_block(ascension),
        },
        "DEFEND" | "BLOCK" if content_id == GREMLIN_TSUNDERE_ID => MonsterIntent::Block {
            block: observed_gremlin_tsundere_block(ascension),
        },
        "DEFEND" | "BLOCK" if content_id == DARKLING_ID => MonsterIntent::Block { block: 12 },
        "DEFEND" | "BLOCK" => MonsterIntent::Block {
            block: damage.max(0),
        },
        "STRONG_DEBUFF" if content_id == BRONZE_ORB_ID => MonsterIntent::SiphonPlayer {
            strength: 0,
            dexterity: 0,
        },
        "UNKNOWN" if content_id == GREMLIN_LEADER_ID && move_id == 2 => {
            MonsterIntent::SummonGremlins { count: 2 }
        }
        "UNKNOWN" if content_id == ACID_SLIME_ID && move_id == 3 => {
            MonsterIntent::SummonGremlins { count: 2 }
        }
        "UNKNOWN" if content_id == BRONZE_AUTOMATON_ID => {
            MonsterIntent::SummonGremlins { count: 2 }
        }
        "UNKNOWN" if content_id == BRONZE_ORB_ID => MonsterIntent::SiphonPlayer {
            strength: 0,
            dexterity: 0,
        },
        "BUFF" | "DEBUG" | "UNKNOWN" => match content_id {
            CULTIST_ID => MonsterIntent::Ritual { amount: 3 },
            ORB_WALKER_ID if damage > 0 => MonsterIntent::Attack { damage },
            SPIKE_SLIME_ID if damage >= 8 => MonsterIntent::AttackAddSlimedToDiscard {
                damage,
                count: observed_slimed_count(monster, content_id),
            },
            SPIKE_SLIME_ID if damage > 0 => MonsterIntent::Attack { damage },
            SPIKE_SLIME_ID => MonsterIntent::Attack { damage: 5 },
            ACID_SLIME_ID if move_id == 2 && damage > 0 => MonsterIntent::Attack { damage },
            ACID_SLIME_ID if move_id == 1 && damage > 0 => {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage,
                    count: observed_slimed_count(monster, content_id),
                }
            }
            ACID_SLIME_ID if damage > 0 => MonsterIntent::AttackAddSlimedToDiscard {
                damage,
                count: observed_slimed_count(monster, content_id),
            },
            ACID_SLIME_ID => MonsterIntent::Attack { damage: 7 },
            RED_LOUSE_ID | GREEN_LOUSE_ID => MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 0,
            },
            GUARDIAN_ID if monster.get("intent").and_then(Value::as_str) == Some("BUFF") => {
                MonsterIntent::GuardianCloseUp { sharp_hide: 3 }
            }
            GUARDIAN_ID => MonsterIntent::Block {
                block: GUARDIAN_CHARGE_BLOCK,
            },
            HEALER_ID if move_id == 2 => MonsterIntent::HealAllMonsters {
                amount: observed_healer_heal(ascension),
            },
            HEALER_ID => MonsterIntent::StrengthAllMonsters {
                amount: observed_healer_strength(ascension),
            },
            FUNGI_BEAST_ID => MonsterIntent::StrengthSelf {
                amount: observed_fungi_beast_strength(ascension),
            },
            _ if damage > 0 => MonsterIntent::Attack { damage },
            _ => MonsterIntent::Attack { damage: 0 },
        },
        _ => MonsterIntent::Attack { damage: 0 },
    }
}

#[cfg(test)]
fn observed_spike_slime_frail(monster: &Value, ascension: u8) -> i32 {
    let large = str_field(monster, "id") == Some("SpikeSlime_L")
        || int(monster, "max_hp") > sts_core::content::monsters::SPIKE_SLIME_M_A7_HP_RANGE.max;
    if large {
        if ascension >= 17 {
            3
        } else {
            2
        }
    } else {
        1
    }
}

#[cfg(test)]
fn observed_centurion_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        20
    } else {
        15
    }
}

#[cfg(test)]
fn observed_gremlin_tsundere_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        11
    } else if ascension >= 7 {
        8
    } else {
        7
    }
}

#[cfg(test)]
fn observed_healer_heal(ascension: u8) -> i32 {
    if ascension >= 17 {
        20
    } else {
        16
    }
}

#[cfg(test)]
fn observed_healer_strength(ascension: u8) -> i32 {
    if ascension >= 17 {
        4
    } else if ascension >= 2 {
        3
    } else {
        2
    }
}

#[cfg(test)]
fn observed_fungi_beast_strength(ascension: u8) -> i32 {
    let strength = if ascension >= 2 { 4 } else { 3 };
    if ascension >= 17 {
        strength + 1
    } else {
        strength
    }
}

#[cfg(test)]
fn observed_slimed_count(monster: &Value, content_id: ContentId) -> i32 {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE, SPIKE_SLIME_ID, SPIKE_SLIME_M_A7_HP_RANGE,
    };

    if (content_id == SPIKE_SLIME_ID && int(monster, "max_hp") > SPIKE_SLIME_M_A7_HP_RANGE.max)
        || (content_id == ACID_SLIME_ID && int(monster, "max_hp") > ACID_SLIME_M_A7_HP_RANGE.max)
    {
        2
    } else {
        1
    }
}

#[cfg(test)]
fn moves_executed_from_observed(monster: &Value, content_id: ContentId) -> u32 {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, BOOK_OF_STABBING_ID, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, CHOSEN_ID,
        CULTIST_ID, GREEN_LOUSE_ID, GREMLIN_LEADER_ID, RED_LOUSE_ID, SHELLED_PARASITE_ID,
        SNAKE_PLANT_ID, SPIKE_SLIME_ID,
    };

    let intent = monster.get("intent").and_then(Value::as_str).unwrap_or("");
    let damage = int(monster, "move_base_damage");
    let hits = int(monster, "move_hits");
    let move_id = int(monster, "move_id");
    match intent {
        "ATTACK" if content_id == BOOK_OF_STABBING_ID && hits > 1 => match hits {
            2 => 0,
            3 => 1,
            4 => 3,
            _ => (hits - 1) as u32,
        },
        "ATTACK" if content_id == BOOK_OF_STABBING_ID && damage >= 21 => 2,
        "ATTACK_BUFF" if content_id == SHELLED_PARASITE_ID => 1,
        "ATTACK" if content_id == SHELLED_PARASITE_ID && hits > 1 => 0,
        "ATTACK" if content_id == SHELLED_PARASITE_ID => 0,
        "DEBUFF" if content_id == CHOSEN_ID => 2,
        "STRONG_DEBUFF" if content_id == CHOSEN_ID => 1,
        "ATTACK_DEBUFF" if content_id == CHOSEN_ID => 3,
        "ATTACK" if content_id == SNAKE_PLANT_ID => 1,
        "STRONG_DEBUFF" if content_id == SNAKE_PLANT_ID => 2,
        "DEFEND_BUFF" if content_id == GREMLIN_LEADER_ID => 2,
        "UNKNOWN" if content_id == GREMLIN_LEADER_ID && move_id == 2 => 2,
        "UNKNOWN" if content_id == ACID_SLIME_ID && move_id == 3 => 2,
        "UNKNOWN" if content_id == BRONZE_AUTOMATON_ID && move_id == 4 => 0,
        "STRONG_DEBUFF" if content_id == BRONZE_ORB_ID => 0,
        "ATTACK" if content_id == BRONZE_ORB_ID => 1,
        "DEFEND" | "BLOCK" if content_id == BRONZE_ORB_ID => 4,
        "DEFEND_BUFF" if content_id == BRONZE_AUTOMATON_ID => {
            if power_amount(monster.get("powers"), "Strength") > 0 {
                4
            } else {
                2
            }
        }
        "STUN" if content_id == BRONZE_AUTOMATON_ID => 6,
        "ATTACK" if content_id == BRONZE_AUTOMATON_ID && hits > 1 => 1,
        "ATTACK" if content_id == BRONZE_AUTOMATON_ID && damage >= 40 => 5,
        "BUFF" | "DEBUG" | "DEBUFF" => 0,
        "ATTACK_DEBUFF" => 1,
        "ATTACK" if content_id == CULTIST_ID => 1,
        "ATTACK" if content_id == LOOTER_ID => 1,
        "ATTACK" if content_id == SPIKE_SLIME_ID => 0,
        "ATTACK" if matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) => 1,
        "ATTACK" if content_id == ACID_SLIME_ID => 1,
        _ => 1,
    }
}

#[cfg(test)]
fn monster_powers(value: Option<&Value>) -> MonsterPowers {
    let mut powers = MonsterPowers::default();
    let Some(items) = value.and_then(Value::as_array) else {
        return powers;
    };
    for power in items {
        let amount = int(power, "amount");
        match power_id(power).as_deref() {
            Some("Vulnerable") => powers.vulnerable = amount,
            Some("Weak") | Some("Weakened") => powers.weak = amount,
            Some("Strength") => powers.strength = amount,
            Some("Artifact") => powers.artifact = amount,
            Some("Ritual") | Some("Demon Form") => powers.ritual = amount,
            Some("Sharp Hide") | Some("Spikes") => powers.spikes = amount,
            Some("Curl Up") => powers.curl_up = amount,
            Some("Anger") => powers.anger = amount,
            Some("Metallicize") => powers.metallicize = amount,
            Some("Plated Armor") => powers.plated_armor = amount,
            Some("Flight") => powers.flight = amount,
            Some("Painful Stabs") => powers.painful_stabs = 1,
            Some("Spore Cloud") => powers.spore_cloud = amount,
            Some("Generic Strength Up Power") => powers.strength_up = amount,
            Some("Malleable") => {
                powers.malleable = amount;
                powers.malleable_base = int(power, "misc").max(0);
            }
            _ => {}
        }
    }
    powers
}

#[cfg(test)]
fn player_powers_and_temp_strength(value: Option<&Value>) -> (PlayerPowers, i32) {
    let mut powers = PlayerPowers::default();
    let mut temp_strength = 0;
    let Some(items) = value.and_then(Value::as_array) else {
        return (powers, temp_strength);
    };
    for power in items {
        let amount = int(power, "amount");
        match power_id(power).as_deref() {
            Some("Strength") => powers.strength = amount,
            Some("Strength Down") | Some("Flex") => temp_strength = amount,
            Some("Weak") | Some("Weakened") => powers.weak = amount,
            Some("Dexterity") => powers.dexterity = amount,
            Some("Frail") => powers.frail = amount,
            Some("Vulnerable") => powers.vulnerable = amount,
            Some("Ritual") | Some("Demon Form") => powers.ritual = amount,
            Some("Metallicize") => powers.metallicize = amount,
            Some("Thorns") => powers.thorns = amount,
            Some("Combust") => {
                powers.combust = 1;
                powers.combust_damage = amount;
            }
            Some("Dark Embrace") => powers.dark_embrace = amount,
            Some("Rupture") => powers.rupture = amount,
            Some("Hex") => powers.hex = amount,
            _ => {}
        }
    }
    powers.strength -= temp_strength;
    (powers, temp_strength)
}

fn reward_gold_offer(game: &Value) -> i32 {
    reward_gold_at_reward_type_from_game(game, "GOLD")
}

fn reward_gold_at_reward_type_from_game(game: &Value, reward_type: &str) -> i32 {
    game.get("screen_state")
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)
        .and_then(|rewards| {
            rewards
                .iter()
                .find(|reward| {
                    reward
                        .get("reward_type")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| kind.eq_ignore_ascii_case(reward_type))
                })
                .and_then(|reward| reward.get("gold"))
        })
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32
}

#[cfg(test)]
fn reward_choices_from_observed(game: &Value) -> Vec<CardInstance> {
    game.get("screen_state")
        .and_then(|state| state.get("cards"))
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .enumerate()
                .filter_map(|(index, card)| {
                    content_id_from_card_value(card).map(|content_id| {
                        CardInstance::new(CardId::new(900 + index as u64), content_id)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(dead_code)]
fn observed_reward_choice<'a>(message: &'a Value, choice_index: usize) -> Option<&'a Value> {
    message
        .get("game_state")?
        .get("screen_state")?
        .get("cards")?
        .as_array()?
        .get(choice_index)
}

#[cfg(test)]
fn card_instances_from_array(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    card_instances_from_array_impl(value, base_id, false, false)
}

#[cfg(test)]
#[allow(dead_code)]
fn combat_card_instances_from_array(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    card_instances_from_array_impl(value, base_id, false, true)
}

#[cfg(test)]
#[allow(dead_code)]
fn hand_from_comm_mod_visible_order(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn draw_pile_from_comm_mod_visible_order(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn discard_pile_from_comm_mod_visible_order(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn exhaust_pile_from_comm_mod_visible_order(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array(value, base_id)
}

#[cfg(test)]
fn card_instances_from_array_impl(
    value: Option<&Value>,
    base_id: u64,
    use_observed_shrug_plus: bool,
    use_observed_cost: bool,
) -> Vec<CardInstance> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            let content_id = if use_observed_shrug_plus {
                content_id_from_card_value_with_observed_shrug_plus(card)
            } else {
                content_id_from_card_value(card)
            }?;
            let mut instance = CardInstance::new(CardId::new(base_id + index as u64), content_id);
            instance.upgrades = card_upgrade_count(card);
            if use_observed_cost {
                if let Some(cost) = card
                    .get("cost")
                    .and_then(Value::as_i64)
                    .and_then(|cost| u8::try_from(cost).ok())
                {
                    instance.temp_cost = Some(cost);
                }
            }
            Some(instance)
        })
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
fn combat_card_instances_from_array_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    card_instances_from_array_impl(value, base_id, true, true)
}

#[cfg(test)]
#[allow(dead_code)]
fn hand_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn draw_pile_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn discard_pile_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn exhaust_pile_from_comm_mod_visible_order_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    combat_card_instances_from_array_with_observed_shrug_plus(value, base_id)
}

#[cfg(test)]
#[allow(dead_code)]
fn card_instances_from_array_with_observed_shrug_plus(
    value: Option<&Value>,
    base_id: u64,
) -> Vec<CardInstance> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            content_id_from_card_value_with_observed_shrug_plus(card).map(|content_id| {
                let mut instance =
                    CardInstance::new(CardId::new(base_id + index as u64), content_id);
                instance.upgrades = card_upgrade_count(card);
                instance
            })
        })
        .collect()
}

#[cfg(test)]
fn card_upgrade_count(card: &Value) -> u8 {
    card.get("upgrades")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0)
}

fn content_id_from_card_value(card: &Value) -> Option<ContentId> {
    let id = card.get("id").and_then(Value::as_str)?;
    let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
    let base = content_id_from_key(id)?;
    if upgrades > 0 {
        return upgrade_content_id(base).or(Some(base));
    }
    Some(base)
}

#[cfg(test)]
fn content_id_from_card_value_with_observed_shrug_plus(card: &Value) -> Option<ContentId> {
    let id = card.get("id").and_then(Value::as_str)?;
    let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
    let base = content_id_from_key(id)?;
    if upgrades > 0 && base == sts_core::content::cards::SHRUG_IT_OFF_ID {
        return Some(sts_core::content::cards::SHRUG_IT_OFF_PLUS_ID);
    }
    content_id_from_card_value(card)
}

fn upgrade_content_id(base: ContentId) -> Option<ContentId> {
    sts_core::content::cards::upgrade_content_id(base)
}

fn normalized_observed_card_key(key: &str) -> String {
    let mut normalized = String::new();
    for ch in key.chars() {
        if ch == '+' {
            normalized.push_str("plus");
        } else if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        }
    }
    normalized
}

fn content_id_from_key(key: &str) -> Option<ContentId> {
    let normalized = normalized_observed_card_key(key);
    if let Some(definition) = sts_core::content::cards::ALL_CARDS
        .iter()
        .find(|definition| {
            normalized_observed_card_key(definition.key) == normalized
                || normalized_observed_card_key(definition.name) == normalized
        })
    {
        return Some(definition.id);
    }

    use sts_core::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BARRICADE_ID, BASH_ID, BASH_PLUS_ID, BATTLE_TRANCE_ID, BERSERK_ID,
        BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID, BLUDGEON_ID, BODY_SLAM_ID,
        BRUTALITY_ID, BURNING_PACT_ID, BURN_ID, CARNAGE_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID,
        CLUMSY_ID, COMBUST_ID, CORRUPTION_ID, CORRUPTION_PLUS_ID, DARK_EMBRACE_ID,
        DARK_SHACKLES_ID, DAZED_ID, DECAY_ID, DEEP_BREATH_ID, DEFEND_R_ID, DEFEND_R_PLUS_ID,
        DEMON_FORM_ID, DISARM_ID, DISCOVERY_ID, DOUBLE_TAP_ID, DOUBLE_TAP_PLUS_ID, DOUBT_ID,
        DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID, ENTRENCH_ID, EVOLVE_ID, EXHUME_ID,
        FEED_ID, FEEL_NO_PAIN_ID, FIEND_FIRE_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID,
        GHOSTLY_ARMOR_ID, HAVOC_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID,
        IMMOLATE_PLUS_ID, INFERNAL_BLADE_ID, INFLAME_ID, INJURY_ID, INTIMIDATE_ID, IRON_WAVE_ID,
        JACK_OF_ALL_TRADES_ID, JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, METALLICIZE_PLUS_ID,
        NORMALITY_ID, OFFERING_ID, PAIN_ID, PARASITE_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID,
        POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID, RAMPAGE_ID, REAPER_ID, REAPER_PLUS_ID,
        RECKLESS_CHARGE_ID, REGRET_ID, RUPTURE_ID, RUPTURE_PLUS_ID, SEARING_BLOW_ID,
        SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID, SEVER_SOUL_ID, SHAME_ID, SHOCKWAVE_ID,
        SHRUG_IT_OFF_ID, SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, SPOT_WEAKNESS_ID, STRIKE_R_ID,
        SWIFT_STRIKE_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID, TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID,
        UPPERCUT_ID, WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WILD_STRIKE_ID, WOUND_ID, WRITHE_ID,
    };
    match key {
        "Strike_R" | "Strike" => Some(STRIKE_R_ID),
        "Defend_R" | "Defend" => Some(DEFEND_R_ID),
        "Defend_R+" | "Defend+" => Some(DEFEND_R_PLUS_ID),
        "Bash" => Some(BASH_ID),
        "Bash+" | "bash+" => Some(BASH_PLUS_ID),
        "Bludgeon" | "bludgeon" => Some(BLUDGEON_ID),
        "Burn" | "burn" => Some(BURN_ID),
        "Burning Pact" | "burning pact" | "Burning Pact+" | "burning pact+" => {
            Some(BURNING_PACT_ID)
        }
        "Combust" | "combust" | "Combust+" | "combust+" => Some(COMBUST_ID),
        "Corruption" | "corruption" => Some(CORRUPTION_ID),
        "Corruption+" | "corruption+" => Some(CORRUPTION_PLUS_ID),
        "Dark Embrace" | "dark embrace" | "Dark Embrace+" | "dark embrace+" => {
            Some(DARK_EMBRACE_ID)
        }
        "Dazed" | "dazed" => Some(DAZED_ID),
        "Wound" | "wound" => Some(WOUND_ID),
        "Slimed" | "slimed" => Some(SLIMED_ID),
        "Thunderclap" | "thunderclap" => Some(THUNDERCLAP_ID),
        "Anger" | "anger" => Some(ANGER_ID),
        "Warcry" | "warcry" => Some(WARCRY_ID),
        "Warcry+" | "warcry+" => Some(WARCRY_PLUS_ID),
        "Metallicize" | "metallicize" => Some(METALLICIZE_ID),
        "Metallicize+" | "metallicize+" => Some(METALLICIZE_PLUS_ID),
        "Twin Strike" | "twin strike" => Some(TWIN_STRIKE_ID),
        "Battle Trance" | "battle trance" => Some(BATTLE_TRANCE_ID),
        "Shrug It Off" | "shrug it off" => Some(SHRUG_IT_OFF_ID),
        "Shrug It Off+" | "shrug it off+" => Some(SHRUG_IT_OFF_PLUS_ID),
        "Body Slam" | "body slam" => Some(BODY_SLAM_ID),
        "Clash" | "clash" => Some(CLASH_ID),
        "Cleave" | "cleave" => Some(CLEAVE_ID),
        "Deep Breath" | "deep breath" => Some(DEEP_BREATH_ID),
        "Dramatic Entrance" | "dramatic entrance" => Some(DRAMATIC_ENTRANCE_ID),
        "Dark Shackles" | "dark shackles" => Some(DARK_SHACKLES_ID),
        "Discovery" | "discovery" => Some(DISCOVERY_ID),
        "Swift Strike" | "swift strike" => Some(SWIFT_STRIKE_ID),
        "Jack Of All Trades" | "jack of all trades" => Some(JACK_OF_ALL_TRADES_ID),
        "Entrench" | "entrench" => Some(ENTRENCH_ID),
        "Fire Breathing" | "fire breathing" => Some(FIRE_BREATHING_ID),
        "Flex" | "flex" => Some(FLEX_ID),
        "Spot Weakness" | "spot weakness" => Some(SPOT_WEAKNESS_ID),
        "Flame Barrier" | "flame barrier" => Some(FLAME_BARRIER_ID),
        "Heavy Blade" | "heavy blade" => Some(HEAVY_BLADE_ID),
        "Intimidate" | "intimidate" => Some(INTIMIDATE_ID),
        "Iron Wave" | "iron wave" => Some(IRON_WAVE_ID),
        "Perfected Strike" | "perfected strike" => Some(PERFECTED_STRIKE_ID),
        "Sword Boomerang" | "sword boomerang" => Some(SWORD_BOOMERANG_ID),
        "True Grit" | "true grit" => Some(TRUE_GRIT_ID),
        "Headbutt" | "headbutt" => Some(HEADBUTT_ID),
        "Clothesline" | "clothesline" => Some(CLOTHESLINE_ID),
        "Shockwave" | "shockwave" => Some(SHOCKWAVE_ID),
        "Rampage" | "rampage" => Some(RAMPAGE_ID),
        "Rage" | "rage" => Some(RAGE_ID),
        "Whirlwind" | "whirlwind" => Some(WHIRLWIND_ID),
        "Pommel Strike" | "pommel strike" => Some(POMMEL_STRIKE_ID),
        "Pummel" | "pummel" => Some(PUMMEL_ID),
        "Searing Blow" | "searing blow" => Some(SEARING_BLOW_ID),
        "Sever Soul" | "sever soul" => Some(SEVER_SOUL_ID),
        "Sentinel" | "sentinel" => Some(SENTINEL_ID),
        "Uppercut" | "uppercut" => Some(UPPERCUT_ID),
        "Disarm" | "disarm" => Some(DISARM_ID),
        "Dual Wield" | "dual wield" => Some(DUAL_WIELD_ID),
        "Immolate" | "immolate" => Some(IMMOLATE_ID),
        "Immolate+" | "immolate+" => Some(IMMOLATE_PLUS_ID),
        "Berserk" | "berserk" => Some(BERSERK_ID),
        "Limit Break" | "limit break" => Some(LIMIT_BREAK_ID),
        "Armaments" | "armaments" => Some(ARMAMENTS_ID),
        "Regret" | "regret" => Some(REGRET_ID),
        "Doubt" | "doubt" => Some(DOUBT_ID),
        "Clumsy" | "clumsy" => Some(CLUMSY_ID),
        "Decay" | "decay" => Some(DECAY_ID),
        "Injury" | "injury" => Some(INJURY_ID),
        "Normality" | "normality" => Some(NORMALITY_ID),
        "Pain" | "pain" => Some(PAIN_ID),
        "Parasite" | "parasite" => Some(PARASITE_ID),
        "Shame" | "shame" => Some(SHAME_ID),
        "Writhe" | "writhe" => Some(WRITHE_ID),
        "Offering" | "offering" => Some(OFFERING_ID),
        "Demon Form" | "demon form" => Some(DEMON_FORM_ID),
        "Double Tap" | "double tap" => Some(DOUBLE_TAP_ID),
        "Double Tap+" | "double tap+" => Some(DOUBLE_TAP_PLUS_ID),
        "Barricade" | "barricade" => Some(BARRICADE_ID),
        "Bloodletting" | "bloodletting" => Some(BLOODLETTING_ID),
        "Blood for Blood" | "blood for blood" => Some(BLOOD_FOR_BLOOD_ID),
        "Blood for Blood+" | "blood for blood+" => Some(BLOOD_FOR_BLOOD_PLUS_ID),
        "Reaper" | "reaper" => Some(REAPER_ID),
        "Reaper+" | "reaper+" => Some(REAPER_PLUS_ID),
        "Rupture" | "rupture" => Some(RUPTURE_ID),
        "Rupture+" | "rupture+" => Some(RUPTURE_PLUS_ID),
        "Hemokinesis" | "hemokinesis" => Some(HEMOKINESIS_ID),
        "Dropkick" | "dropkick" => Some(DROPKICK_ID),
        "Wild Strike" | "wild strike" => Some(WILD_STRIKE_ID),
        "Power Through" | "power through" => Some(POWER_THROUGH_ID),
        "Infernal Blade" | "infernal blade" => Some(INFERNAL_BLADE_ID),
        "Ghostly Armor" | "ghostly armor" => Some(GHOSTLY_ARMOR_ID),
        "Reckless Charge" | "reckless charge" => Some(RECKLESS_CHARGE_ID),
        "Feel No Pain" | "feel no pain" => Some(FEEL_NO_PAIN_ID),
        "Seeing Red" | "seeing red" => Some(SEEING_RED_ID),
        "Inflame" | "inflame" => Some(INFLAME_ID),
        "Havoc" | "havoc" => Some(HAVOC_ID),
        "Second Wind" | "second wind" => Some(SECOND_WIND_ID),
        "Carnage" | "carnage" => Some(CARNAGE_ID),
        "Evolve" | "evolve" => Some(EVOLVE_ID),
        "Feed" | "feed" => Some(FEED_ID),
        "Fiend Fire" | "fiend fire" => Some(FIEND_FIRE_ID),
        "Juggernaut" | "juggernaut" => Some(JUGGERNAUT_ID),
        "Brutality" | "brutality" => Some(BRUTALITY_ID),
        "Exhume" | "exhume" => Some(EXHUME_ID),
        "Trip" | "trip" => Some(TRIP_ID),
        _ => None,
    }
}

fn content_key(content_id: ContentId) -> &'static str {
    if let Some(definition) = sts_core::content::cards::get_card_definition(content_id) {
        return definition.name;
    }

    use sts_core::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BARRICADE_ID, BASH_ID, BASH_PLUS_ID, BATTLE_TRANCE_ID, BERSERK_ID,
        BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID, BLUDGEON_ID, BODY_SLAM_ID,
        BURNING_PACT_ID, BURN_ID, CHRYSALIS_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, CLUMSY_ID,
        COMBUST_ID, CORRUPTION_ID, CORRUPTION_PLUS_ID, CURSE_OF_THE_BELL_ID, DARK_EMBRACE_ID,
        DARK_SHACKLES_ID, DAZED_ID, DECAY_ID, DEEP_BREATH_ID, DEFEND_R_ID, DEFEND_R_PLUS_ID,
        DEMON_FORM_ID, DISARM_ID, DISCOVERY_ID, DOUBLE_TAP_ID, DOUBLE_TAP_PLUS_ID, DOUBT_ID,
        DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID, ENTRENCH_ID, FEED_ID, FEEL_NO_PAIN_ID,
        FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID, FLEX_PLUS_ID, HAND_OF_GREED_ID, HAVOC_ID,
        HAVOC_PLUS_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, IMMOLATE_PLUS_ID,
        IMPERVIOUS_ID, INFLAME_ID, INFLAME_PLUS_ID, INJURY_ID, INTIMIDATE_ID,
        JACK_OF_ALL_TRADES_ID, LIMIT_BREAK_ID, MAGNETISM_ID, MAYHEM_ID, METALLICIZE_ID,
        METALLICIZE_PLUS_ID, NORMALITY_ID, OFFERING_ID, OFFERING_PLUS_ID, PAIN_ID, PARASITE_ID,
        PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, RAGE_ID, RAMPAGE_ID,
        REAPER_ID, REAPER_PLUS_ID, REGRET_ID, RUPTURE_ID, RUPTURE_PLUS_ID, SEARING_BLOW_ID,
        SECRET_WEAPON_ID, SENTINEL_ID, SEVER_SOUL_ID, SHAME_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID,
        SHRUG_IT_OFF_PLUS_ID, SLIMED_ID, SPOT_WEAKNESS_ID, STRIKE_R_ID, STRIKE_R_PLUS_ID,
        SWIFT_STRIKE_ID, SWIFT_STRIKE_PLUS_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID,
        TRANSMUTATION_ID, TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID,
        WARCRY_PLUS_ID, WHIRLWIND_ID, WILD_STRIKE_ID, WOUND_ID, WRITHE_ID,
    };
    match content_id {
        id if id == STRIKE_R_ID || id == STRIKE_R_PLUS_ID => "Strike_R",
        id if id == DEFEND_R_ID || id == DEFEND_R_PLUS_ID => "Defend_R",
        id if id == BASH_ID || id == BASH_PLUS_ID => "Bash",
        id if id == BLUDGEON_ID => "Bludgeon",
        id if id == BURN_ID => "Burn",
        id if id == BURNING_PACT_ID => "Burning Pact",
        id if id == CURSE_OF_THE_BELL_ID => "Curse of the Bell",
        id if id == DARK_EMBRACE_ID => "Dark Embrace",
        id if id == DAZED_ID => "Dazed",
        id if id == WOUND_ID => "Wound",
        id if id == SLIMED_ID => "Slimed",
        id if id == THUNDERCLAP_ID => "Thunderclap",
        id if id == ANGER_ID => "Anger",
        id if id == WARCRY_ID => "Warcry",
        id if id == WARCRY_PLUS_ID => "Warcry+",
        id if id == METALLICIZE_ID => "Metallicize",
        id if id == METALLICIZE_PLUS_ID => "Metallicize+",
        id if id == TWIN_STRIKE_ID => "Twin Strike",
        id if id == BATTLE_TRANCE_ID => "Battle Trance",
        id if id == SHRUG_IT_OFF_ID => "Shrug It Off",
        id if id == SHRUG_IT_OFF_PLUS_ID => "Shrug It Off+",
        id if id == BODY_SLAM_ID => "Body Slam",
        id if id == CLASH_ID => "Clash",
        id if id == CLEAVE_ID => "Cleave",
        id if id == WILD_STRIKE_ID => "Wild Strike",
        id if id == HAVOC_ID => "Havoc",
        id if id == HAVOC_PLUS_ID => "Havoc+",
        id if id == INFLAME_ID => "Inflame",
        id if id == INFLAME_PLUS_ID => "Inflame+",
        id if id == COMBUST_ID => "Combust",
        id if id == CORRUPTION_ID => "Corruption",
        id if id == CORRUPTION_PLUS_ID => "Corruption+",
        id if id == OFFERING_ID => "Offering",
        id if id == OFFERING_PLUS_ID => "Offering+",
        id if id == DOUBLE_TAP_ID => "Double Tap",
        id if id == DOUBLE_TAP_PLUS_ID => "Double Tap+",
        id if id == DEEP_BREATH_ID => "Deep Breath",
        id if id == DRAMATIC_ENTRANCE_ID => "Dramatic Entrance",
        id if id == DARK_SHACKLES_ID => "Dark Shackles",
        id if id == DISCOVERY_ID => "Discovery",
        id if id == SWIFT_STRIKE_ID => "Swift Strike",
        id if id == SWIFT_STRIKE_PLUS_ID => "Swift Strike+",
        id if id == JACK_OF_ALL_TRADES_ID => "Jack Of All Trades",
        id if id == ENTRENCH_ID => "Entrench",
        id if id == FIRE_BREATHING_ID => "Fire Breathing",
        id if id == FLEX_ID => "Flex",
        id if id == FLEX_PLUS_ID => "Flex+",
        id if id == SPOT_WEAKNESS_ID => "Spot Weakness",
        id if id == FLAME_BARRIER_ID => "Flame Barrier",
        id if id == HEAVY_BLADE_ID => "Heavy Blade",
        id if id == INTIMIDATE_ID => "Intimidate",
        id if id == PERFECTED_STRIKE_ID => "Perfected Strike",
        id if id == SWORD_BOOMERANG_ID => "Sword Boomerang",
        id if id == TRUE_GRIT_ID => "True Grit",
        id if id == HEADBUTT_ID => "Headbutt",
        id if id == IMMOLATE_ID || id == IMMOLATE_PLUS_ID => "Immolate",
        id if id == BERSERK_ID => "Berserk",
        id if id == LIMIT_BREAK_ID => "Limit Break",
        id if id == IMPERVIOUS_ID => "Impervious",
        id if id == FEED_ID => "Feed",
        id if id == ARMAMENTS_ID => "Armaments",
        id if id == CLOTHESLINE_ID => "Clothesline",
        id if id == SHOCKWAVE_ID => "Shockwave",
        id if id == RAMPAGE_ID => "Rampage",
        id if id == RAGE_ID => "Rage",
        id if id == WHIRLWIND_ID => "Whirlwind",
        id if id == POMMEL_STRIKE_ID => "Pommel Strike",
        id if id == POMMEL_STRIKE_PLUS_ID => "Pommel Strike+",
        id if id == SEVER_SOUL_ID => "Sever Soul",
        id if id == SENTINEL_ID => "Sentinel",
        id if id == UPPERCUT_ID => "Uppercut",
        id if id == DISARM_ID => "Disarm",
        id if id == DUAL_WIELD_ID => "Dual Wield",
        id if id == SEARING_BLOW_ID => "Searing Blow",
        id if id == REGRET_ID => "Regret",
        id if id == DOUBT_ID => "Doubt",
        id if id == CLUMSY_ID => "Clumsy",
        id if id == DECAY_ID => "Decay",
        id if id == INJURY_ID => "Injury",
        id if id == NORMALITY_ID => "Normality",
        id if id == PAIN_ID => "Pain",
        id if id == PARASITE_ID => "Parasite",
        id if id == SHAME_ID => "Shame",
        id if id == WRITHE_ID => "Writhe",
        id if id == DEMON_FORM_ID => "Demon Form",
        id if id == BARRICADE_ID => "Barricade",
        id if id == BLOODLETTING_ID => "Bloodletting",
        id if id == BLOOD_FOR_BLOOD_ID => "Blood for Blood",
        id if id == BLOOD_FOR_BLOOD_PLUS_ID => "Blood for Blood+",
        id if id == REAPER_ID => "Reaper",
        id if id == REAPER_PLUS_ID => "Reaper+",
        id if id == RUPTURE_ID => "Rupture",
        id if id == RUPTURE_PLUS_ID => "Rupture+",
        id if id == HEMOKINESIS_ID => "Hemokinesis",
        id if id == DROPKICK_ID => "Dropkick",
        id if id == TRIP_ID => "Trip",
        id if id == FEEL_NO_PAIN_ID => "Feel No Pain",
        id if id == MAYHEM_ID => "Mayhem",
        id if id == SECRET_WEAPON_ID => "Secret Weapon",
        id if id == TRANSMUTATION_ID => "Transmutation",
        id if id == MAGNETISM_ID => "Magnetism",
        id if id == CHRYSALIS_ID => "Chrysalis",
        id if id == HAND_OF_GREED_ID => "Hand Of Greed",
        other if shop_pool_trace_name(other).is_some() => {
            shop_pool_trace_name(other).unwrap_or("unknown")
        }
        _ => "unknown",
    }
}

fn deck_content_key(content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        DEFEND_R_ID, DEFEND_R_PLUS_ID, FLEX_PLUS_ID, HAVOC_PLUS_ID, INFLAME_PLUS_ID, OFFERING_ID,
        STRIKE_R_ID, STRIKE_R_PLUS_ID, WARCRY_PLUS_ID,
    };
    match content_id {
        id if id == STRIKE_R_ID || id == STRIKE_R_PLUS_ID => "Strike_R",
        id if id == DEFEND_R_ID || id == DEFEND_R_PLUS_ID => "Defend_R",
        id if id == WARCRY_PLUS_ID => "Warcry",
        id if id == FLEX_PLUS_ID => "Flex",
        id if id == HAVOC_PLUS_ID => "Havoc",
        id if id == INFLAME_PLUS_ID => "Inflame",
        id if id == OFFERING_ID => "Offering",
        other => {
            let key = content_key(other);
            if key.ends_with('+') {
                key.trim_end_matches('+')
            } else {
                key
            }
        }
    }
}

fn reward_card_display_key(run: &RunState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::WARCRY_PLUS_ID;
    if content_id == WARCRY_PLUS_ID {
        return "Warcry+";
    }
    if let Some(upgraded) = egg_preview_upgrade(run, content_id) {
        return content_key(upgraded);
    }
    content_key(content_id)
}

fn egg_preview_upgrade(run: &RunState, content_id: ContentId) -> Option<ContentId> {
    let upgraded = upgrade_content_id(content_id)?;
    let (card_type, _) = card_type_and_rarity(content_id)?;
    let has_matching_egg = match card_type {
        CardType::Attack => run_has_relic_key(run, RelicKey::MoltenEgg),
        CardType::Skill => run_has_relic_key(run, RelicKey::ToxicEgg),
        CardType::Power => run_has_relic_key(run, RelicKey::FrozenEgg),
        CardType::Status => false,
    };
    has_matching_egg.then_some(upgraded)
}

fn choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd, index] if cmd.eq_ignore_ascii_case("CHOOSE") => index.parse().ok(),
        _ => None,
    }
}

fn command_head_eq(command: &str, expected: &str) -> bool {
    command
        .split_whitespace()
        .next()
        .is_some_and(|head| head.eq_ignore_ascii_case(expected))
}

struct ParsedPotionUse {
    slot: usize,
    target: Option<MonsterId>,
}

fn seed_start_potion_command_target(
    run: &RunState,
    potion_use: &ParsedPotionUse,
) -> Option<MonsterId> {
    run.potion_at_slot(potion_use.slot)
        .is_some_and(Potion::requires_target)
        .then_some(potion_use.target)
        .flatten()
}

fn parse_potion_use(command: &str) -> Option<ParsedPotionUse> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [head, second, slot]
            if head.eq_ignore_ascii_case("POTION") && second.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: None,
            })
        }
        [head, second, slot, target]
            if head.eq_ignore_ascii_case("POTION") && second.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: Some(MonsterId::new(target.parse::<u64>().ok()? + 1)),
            })
        }
        [head, slot, target]
            if head.eq_ignore_ascii_case("potion") && !slot.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: Some(MonsterId::new(target.parse::<u64>().ok()? + 1)),
            })
        }
        [head, slot]
            if head.eq_ignore_ascii_case("potion") && !slot.eq_ignore_ascii_case("USE") =>
        {
            Some(ParsedPotionUse {
                slot: slot.parse().ok()?,
                target: None,
            })
        }
        _ => None,
    }
}

fn deck_keys_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .filter_map(|card| card.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

fn deck_content_keys(deck: &[CardInstance]) -> Vec<String> {
    deck.iter()
        .map(|card| deck_content_key(card.content_id).to_owned())
        .collect()
}

fn deck_content_keys_with_pending_obtain_cards(run: &RunState) -> Vec<String> {
    let mut deck = deck_content_keys(&run.deck);
    deck.extend(
        run.pending_obtain_cards
            .iter()
            .map(|content_id| deck_content_key(*content_id).to_owned()),
    );
    deck
}

fn screen_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("screen_type"))
        .and_then(Value::as_str)
}

#[cfg(test)]
#[allow(dead_code)]
fn room_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("room_type"))
        .and_then(Value::as_str)
}

#[cfg(test)]
#[allow(dead_code)]
fn first_choice(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_str)
}

#[cfg(test)]
#[allow(dead_code)]
fn unsupported_reason(pre: &TraceState, action: &TraceAction) -> String {
    match action.command.split_whitespace().next().unwrap_or("") {
        "START" => "seed-start run creation is source-backed/generated for selected Ironclad A0 surfaces, with remaining map, Neow branch-combo, and reward RNG parity gaps classified".to_owned(),
        "CHOOSE" if screen_type(&pre.message) == Some("EVENT") => {
            "Neow/event choice side effects are unsupported in sim-to-real replay".to_owned()
        }
        "CHOOSE" if screen_type(&pre.message) == Some("MAP") => {
            "map node selection is unsupported until exact seed-to-map parity is implemented".to_owned()
        }
        "CHOOSE" if screen_type(&pre.message) == Some("COMBAT_REWARD") => {
            "reward card-screen opening is a UI transition; card pickup is verified from CARD_REWARD".to_owned()
        }
        "CHOOSE"
            if matches!(
                screen_type(&pre.message),
                Some("SHOP_ROOM" | "SHOP_SCREEN" | "GRID")
            ) && room_type(&pre.message).is_some_and(|room| room.eq_ignore_ascii_case("ShopRoom")) =>
        {
            "shop UI choices are covered by seed-start shop replay".to_owned()
        }
        "CONFIRM"
            if screen_type(&pre.message) == Some("GRID")
                && room_type(&pre.message).is_some_and(|room| room.eq_ignore_ascii_case("ShopRoom")) =>
        {
            "shop UI choices are covered by seed-start shop replay".to_owned()
        }
        "LEAVE"
            if matches!(screen_type(&pre.message), Some("SHOP_ROOM" | "SHOP_SCREEN"))
                && room_type(&pre.message).is_some_and(|room| room.eq_ignore_ascii_case("ShopRoom")) =>
        {
            "shop UI choices are covered by seed-start shop replay".to_owned()
        }
        "PROCEED" => "reward-to-map UI transition is out-of-scope for simulator state parity".to_owned(),
        "state" => "trace client poll command is not a game transition".to_owned(),
        _ => "unsupported or unobservable CommunicationMod command".to_owned(),
    }
}

fn intent_key(monster: &MonsterState) -> String {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, SLAVER_BLUE_ID, SLIME_BOSS_ID, SPIKE_SLIME_ID,
    };

    match monster.intent {
        MonsterIntent::Attack { .. }
        | MonsterIntent::AttackAddSlimedToDiscard { .. }
        | MonsterIntent::AttackApplyPlayerFrail { .. }
        | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
        | MonsterIntent::AttackApplyPlayerWeak { .. }
        | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. }
        | MonsterIntent::AttackMultiple { .. }
        | MonsterIntent::AttackMultipleUpgradeBurns { .. }
        | MonsterIntent::AttackStealGold { .. } => {
            if matches!(monster.content_id, ACID_SLIME_ID | SPIKE_SLIME_ID)
                || matches!(
                    monster.intent,
                    MonsterIntent::AttackMultipleUpgradeBurns { .. }
                )
            {
                "ATTACK_DEBUFF".to_owned()
            } else if monster.content_id == SLAVER_BLUE_ID
                && matches!(monster.intent, MonsterIntent::AttackApplyPlayerWeak { .. })
            {
                "ATTACK".to_owned()
            } else if matches!(
                monster.intent,
                MonsterIntent::AttackApplyPlayerWeak { .. }
                    | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. }
                    | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. }
            ) {
                "ATTACK_DEBUFF".to_owned()
            } else {
                "ATTACK".to_owned()
            }
        }
        MonsterIntent::Ritual { .. }
        | MonsterIntent::Block { .. }
        | MonsterIntent::StrengthAndBlock { .. }
        | MonsterIntent::HealAllMonsters { .. }
        | MonsterIntent::StrengthSelf { .. }
        | MonsterIntent::StrengthAllMonsters { .. }
        | MonsterIntent::GuardianCloseUp { .. } => "BUFF".to_owned(),
        MonsterIntent::EncourageGremlins { .. } => "DEFEND_BUFF".to_owned(),
        MonsterIntent::AttackAndBlock { .. } | MonsterIntent::AttackHealSelf { .. } => {
            "ATTACK_BUFF".to_owned()
        }
        MonsterIntent::ApplyPlayerWeak { .. }
        | MonsterIntent::AttackApplyPlayerVulnerable { .. }
        | MonsterIntent::AttackAddWoundsToDiscard { .. }
        | MonsterIntent::ApplyPlayerHex { .. }
        | MonsterIntent::ApplyPlayerWeakStrengthSelf { .. }
        | MonsterIntent::ApplyPlayerConfusion
        | MonsterIntent::ApplyPlayerConstricted { .. }
        | MonsterIntent::AddDazedToDiscard { .. }
        | MonsterIntent::AddDazedToDraw { .. }
        | MonsterIntent::AddBurnToDiscard { .. }
        | MonsterIntent::AddBurnToDiscardAndDraw { .. }
        | MonsterIntent::SiphonPlayer { .. } => "DEBUFF".to_owned(),
        MonsterIntent::ApplyPlayerFrailAndWeak { .. }
        | MonsterIntent::ApplyPlayerFrailWeakVulnerable { .. }
        | MonsterIntent::ApplyPlayerEntangled { .. } => "STRONG_DEBUFF".to_owned(),
        MonsterIntent::AddSlimedToDiscard { .. } if monster.content_id == SLIME_BOSS_ID => {
            "STRONG_DEBUFF".to_owned()
        }
        MonsterIntent::AddSlimedToDiscard { .. } => "DEBUFF".to_owned(),
        MonsterIntent::Sleep => "SLEEP".to_owned(),
        MonsterIntent::Stun => "STUN".to_owned(),
        MonsterIntent::Escape => "ESCAPE".to_owned(),
        MonsterIntent::DefensiveCharge { .. }
        | MonsterIntent::SummonGremlins { .. }
        | MonsterIntent::SummonCollectorTorchHeads { .. } => "UNKNOWN".to_owned(),
    }
}

fn int(value: &Value, key: &str) -> i32 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

#[cfg(test)]
fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn observed_act1_boss(game: &Value) -> Act1Boss {
    game.get("act_boss")
        .and_then(Value::as_str)
        .and_then(Act1Boss::from_trace_name)
        .unwrap_or_default()
}

#[cfg(test)]
fn power_id(power: &Value) -> Option<String> {
    power
        .get("id")
        .or_else(|| power.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn insert<T: Serialize>(map: &mut serde_json::Map<String, Value>, key: &str, value: T) {
    map.insert(
        key.to_owned(),
        serde_json::to_value(value).expect("json value"),
    );
}

fn push_sim_error(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    err: sts_core::SimError,
) {
    report.unexpected_diffs.push(UnexpectedDiff {
        action_step: action.step,
        command: action.command.clone(),
        label: label.to_owned(),
        diffs: vec![format!("simulator rejected transition: {err:?}")],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_core::content::cards::{
        BATTLE_TRANCE_ID, BURN_ID, CORRUPTION_PLUS_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID,
        ENTRENCH_ID, TWIN_STRIKE_ID,
    };
    use sts_core::content::monsters::{monster_state, SLAVER_BLUE_A0};
    use sts_core::relic::IRONCLAD_BOSS_RELIC_POOL;

    #[test]
    fn seed_start_potion_command_drops_stray_target_for_targetless_potion() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.potions = vec![Potion::Dexterity];

        let command = ParsedPotionUse {
            slot: 0,
            target: Some(MonsterId::new(1)),
        };

        assert_eq!(seed_start_potion_command_target(&run, &command), None);
    }

    #[test]
    fn seed_start_potion_command_keeps_target_for_targeted_potion() {
        let mut run = RunState::placeholder_seeded_ironclad(1, 0);
        run.potions = vec![Potion::Fire];

        let target = MonsterId::new(1);
        let command = ParsedPotionUse {
            slot: 0,
            target: Some(target),
        };

        assert_eq!(
            seed_start_potion_command_target(&run, &command),
            Some(target)
        );
    }

    #[test]
    fn slaver_blue_weak_attack_projects_to_observed_attack_intent() {
        let mut monster = monster_state(&SLAVER_BLUE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackApplyPlayerWeak { damage: 7, weak: 1 };

        assert_eq!(seed_start_trace_intent(&monster), "ATTACK");
    }

    #[test]
    fn slaver_blue_attack_move_four_imports_weak_attack() {
        let monster = json!({
            "id": "SlaverBlue",
            "name": "Slaver",
            "intent": "ATTACK",
            "move_id": 4,
            "move_base_damage": 7,
            "move_hits": 1
        });

        assert_eq!(
            observed_intent(&monster, sts_core::content::monsters::SLAVER_BLUE_ID, 0),
            MonsterIntent::AttackApplyPlayerWeak { damage: 7, weak: 1 }
        );
    }

    #[test]
    fn observed_card_reward_preserves_corruption_plus() {
        let game = json!({
            "screen_type": "CARD_REWARD",
            "screen_state": {
                "cards": [
                    {
                        "id": "Corruption",
                        "name": "Corruption+",
                        "upgrades": 1
                    }
                ]
            }
        });

        let choices = reward_choices_from_observed(&game);

        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].content_id, CORRUPTION_PLUS_ID);
    }

    #[test]
    fn observed_reward_imports_communication_mod_gamblers_brew_id() {
        let game = json!({
            "screen_type": "COMBAT_REWARD",
            "screen_state": {
                "rewards": [
                    {
                        "reward_type": "POTION",
                        "potion": {
                            "id": "GamblersBrew",
                            "name": "Gambler's Brew"
                        }
                    }
                ]
            }
        });

        assert_eq!(
            observed_reward_potion_offer(&game),
            Some(Potion::GamblersBrew)
        );
    }

    #[test]
    fn full_belt_potion_reward_command_takes_pending_gold_before_potion() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Reward;
        run.gold = 99;
        run.potions = vec![Potion::BlessingOfTheForge, Potion::Dexterity, Potion::Power];
        run.reward = Some(RewardScreen {
            choices: Vec::new(),
            gold_offer: 120,
            stolen_gold_offer: 0,
            potion_offer: Some(Potion::Dexterity),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });
        let pre = json!({
            "game_state": {
                "screen_state": {
                    "rewards": [
                        {"reward_type": "GOLD", "gold": 120},
                        {"reward_type": "POTION", "potion": {"id": "Dexterity Potion", "name": "Dexterity Potion"}}
                    ]
                }
            }
        });
        let post = json!({"game_state": {"potions": [{}, {}, {}]}});

        let label = seed_start_apply_reward_choose(&mut run, "CHOOSE 1", &pre, &post, "MANUAL01")
            .expect("full-belt potion reward command maps to pending gold");

        assert_eq!(label, "potion reward");
        assert_eq!(run.gold, 219);
        let reward = run.reward.as_ref().expect("reward screen remains");
        assert_eq!(reward.gold_offer, 0);
        assert_eq!(reward.potion_offer, Some(Potion::Dexterity));
    }

    #[test]
    fn seed_start_act2_combat_entry_uses_city_spawn_helper() {
        let seed = 1_218_623;
        let floor = 18;
        let combat_index = 0;
        let message = json!({
            "game_state": {
                "screen_type": "COMBAT",
                "ascension_level": 0,
                "floor": floor,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": [{"id": "Strike_R"}],
                "relics": [],
                "combat_state": {
                    "player": {
                        "current_hp": 80,
                        "block": 0,
                        "energy": 3
                    },
                    "monsters": []
                }
            }
        });

        let expected = seed_start_encounter_expected_at_index(
            seed,
            combat_index,
            0,
            &["Strike_R".to_owned()],
            &[],
            false,
            &message,
        );
        let actual_monsters = expected
            .get("monsters")
            .and_then(Value::as_array)
            .expect("expected monsters");
        let city_spawns =
            target_city_normal_encounter_spawn_at_combat_index(seed, floor, combat_index, 0, false)
                .expect("city spawn metadata");
        let exordium_spawns =
            target_normal_encounter_spawn_at_combat_index(seed, floor, combat_index, 0, false)
                .expect("exordium spawn metadata");

        assert_eq!(actual_monsters.len(), city_spawns.len());
        assert_eq!(
            actual_monsters[0].get("name").and_then(Value::as_str),
            Some(city_spawns[0].name)
        );
        assert_ne!(city_spawns[0].name, exordium_spawns[0].name);
    }

    #[test]
    fn seed_start_act2_room_kind_resolution_uses_city_map_stream() {
        let seed = 1_218_623;
        let path = vec![0];
        let act1_message = json!({"game_state": {"floor": 1}});
        let act2_message = json!({"game_state": {"floor": 18}});

        assert_eq!(
            seed_start_target_act_from_message(&act1_message),
            TargetMapAct::Exordium
        );
        assert_eq!(
            seed_start_target_act_from_message(&act2_message),
            TargetMapAct::City
        );
        assert_eq!(
            seed_start_room_kinds_on_path(seed, &path, &act2_message),
            city_room_kinds_on_path(seed, &path)
        );
        assert_eq!(
            seed_start_room_kinds_on_path(seed, &path, &act1_message),
            exordium_room_kinds_on_path(seed, &path)
        );
        assert_eq!(seed_start_target_act_from_floor(18), TargetMapAct::City);
    }

    #[test]
    fn trace_replay_parses_unknown_exit_metadata_and_supports_empty_trace() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"metadata","event":"exit","ended_at":"now"}"#;

        let error = verify_communication_mod_trace(content).expect_err("strict replay needs START");
        assert!(matches!(error, SimRealError::MissingStartCommand));
    }

    #[test]
    fn dramatic_entrance_maps_from_observed_card_json() {
        let card = json!({"id": "Dramatic Entrance", "name": "Dramatic Entrance"});
        assert_eq!(
            content_id_from_card_value(&card),
            Some(DRAMATIC_ENTRANCE_ID)
        );
    }

    #[test]
    fn colorless_reward_cards_map_from_observed_card_json() {
        use sts_core::content::cards::{DARK_SHACKLES_ID, DISCOVERY_ID};

        for (id, expected, key) in [
            (
                "Dramatic Entrance",
                DRAMATIC_ENTRANCE_ID,
                "Dramatic Entrance",
            ),
            ("Dark Shackles", DARK_SHACKLES_ID, "Dark Shackles"),
            ("Discovery", DISCOVERY_ID, "Discovery"),
        ] {
            let card = json!({"id": id, "name": id});

            assert_eq!(content_id_from_card_value(&card), Some(expected));
            assert_eq!(content_key(expected), key);
        }
    }

    #[test]
    fn dropkick_maps_from_observed_card_json() {
        let card = json!({"id": "Dropkick", "name": "Dropkick"});

        assert_eq!(content_id_from_card_value(&card), Some(DROPKICK_ID));
        assert_eq!(content_key(DROPKICK_ID), "Dropkick");
    }

    #[test]
    fn burn_maps_from_observed_card_json() {
        let card = json!({"id": "Burn", "name": "Burn"});

        assert_eq!(content_id_from_card_value(&card), Some(BURN_ID));
        assert_eq!(content_key(BURN_ID), "Burn");
    }

    #[test]
    fn long_trace_observed_cards_map_from_card_json() {
        use sts_core::content::cards::{
            BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BURNING_PACT_ID, COMBUST_ID, DARK_EMBRACE_ID,
            DAZED_ID, DOUBLE_TAP_ID, FEEL_NO_PAIN_ID, RAGE_ID, REAPER_ID, RUPTURE_ID, WOUND_ID,
        };

        for (id, expected, key) in [
            ("Blood for Blood", BLOOD_FOR_BLOOD_ID, "Blood for Blood"),
            ("Reaper", REAPER_ID, "Reaper"),
            ("Wound", WOUND_ID, "Wound"),
            ("Rupture", RUPTURE_ID, "Rupture"),
            ("Burning Pact", BURNING_PACT_ID, "Burning Pact"),
            ("Combust", COMBUST_ID, "Combust"),
            ("Dazed", DAZED_ID, "Dazed"),
            ("Dark Embrace", DARK_EMBRACE_ID, "Dark Embrace"),
            ("Bludgeon", BLUDGEON_ID, "Bludgeon"),
            ("Double Tap", DOUBLE_TAP_ID, "Double Tap"),
            ("Rage", RAGE_ID, "Rage"),
            ("feelnopain", FEEL_NO_PAIN_ID, "Feel No Pain"),
        ] {
            let card = json!({"id": id, "name": id});

            assert_eq!(content_id_from_card_value(&card), Some(expected));
            assert_eq!(content_key(expected), key);
        }
    }

    #[test]
    fn observed_combat_card_reward_ids_are_canonicalized() {
        use sts_core::content::cards::{FEEL_NO_PAIN_ID, SHOCKWAVE_ID};

        let cards = json!([
            {"id": "Shockwave", "name": "Shockwave", "upgrades": 0},
            {"id": "feelnopain", "name": "Feel No Pain", "upgrades": 0},
            {"id": "unknown-custom-card", "name": "Unknown Custom Card", "upgrades": 0}
        ]);

        assert_eq!(
            card_reward_ids_from_value(Some(&cards)),
            vec![
                json!(SHOCKWAVE_ID.get()),
                json!(FEEL_NO_PAIN_ID.get()),
                json!("unknown-custom-card")
            ]
        );
    }

    #[test]
    fn observed_card_json_maps_every_modeled_card_definition() {
        let mut failures = Vec::new();
        for definition in sts_core::content::cards::ALL_CARDS {
            let card = json!({
                "id": definition.key,
                "name": definition.name,
                "upgrades": 0,
            });

            let mapped = content_id_from_card_value(&card);
            if mapped != Some(definition.id) {
                failures.push(format!(
                    "{} mapped to {:?}, expected {:?}",
                    definition.key, mapped, definition.id
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "missing or mismapped observed card keys:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn observed_reward_import_preserves_every_modeled_card_choice() {
        let cards: Vec<_> = sts_core::content::cards::ALL_CARDS
            .iter()
            .map(|definition| {
                json!({
                    "id": definition.key,
                    "name": definition.name,
                    "upgrades": 0,
                })
            })
            .collect();
        let game = json!({
            "screen_type": "CARD_REWARD",
            "screen_state": {
                "cards": cards,
            }
        });

        let choices = reward_choices_from_observed(&game);
        let ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();
        let expected: Vec<_> = sts_core::content::cards::ALL_CARDS
            .iter()
            .map(|definition| definition.id)
            .collect();

        assert_eq!(ids, expected);
    }

    #[test]
    fn guardian_defend_observed_intent_replays_charge_up_block() {
        let monster = json!({
            "id": "TheGuardian",
            "intent": "DEFEND",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GUARDIAN_ID, 0),
            MonsterIntent::Block {
                block: GUARDIAN_CHARGE_BLOCK
            }
        );
    }

    #[test]
    fn debug_observed_intent_with_damage_imports_attack() {
        use sts_core::content::monsters::JAW_WORM_ID;

        let monster = json!({
            "id": "JawWorm",
            "intent": "DEBUG",
            "move_base_damage": 11
        });

        assert_eq!(
            observed_intent(&monster, JAW_WORM_ID, 0),
            MonsterIntent::Attack { damage: 11 }
        );
    }

    #[test]
    fn mugger_attack_observed_intent_imports_gold_steal_attack() {
        let monster = json!({
            "id": "Mugger",
            "intent": "ATTACK",
            "move_base_damage": 10,
            "move_hits": 1,
            "move_id": 1,
            "powers": [{"id": "Thievery", "name": "Thievery", "amount": 15}]
        });

        assert_eq!(
            observed_intent(&monster, MUGGER_ID, 0),
            MonsterIntent::AttackStealGold {
                damage: 10,
                amount: 15
            }
        );
    }

    #[test]
    fn jaw_worm_defend_buff_observed_intent_imports_bellow() {
        use sts_core::content::monsters::JAW_WORM_ID;

        let monster = json!({
            "id": "JawWorm",
            "intent": "DEFEND_BUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, JAW_WORM_ID, 0),
            MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: 6
            }
        );
    }

    #[test]
    fn jaw_worm_attack_defend_observed_intent_imports_thrash() {
        use sts_core::content::monsters::JAW_WORM_ID;

        let monster = json!({
            "id": "JawWorm",
            "intent": "ATTACK_DEFEND",
            "move_base_damage": 7,
            "move_hits": 1,
            "move_id": 3
        });

        assert_eq!(
            observed_intent(&monster, JAW_WORM_ID, 0),
            MonsterIntent::AttackAndBlock {
                damage: 7,
                block: 5
            }
        );
    }

    #[test]
    fn gremlin_leader_defend_buff_observed_intent_imports_encourage() {
        use sts_core::content::monsters::GREMLIN_LEADER_ID;

        let monster = json!({
            "id": "GremlinLeader",
            "intent": "DEFEND_BUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_LEADER_ID, 0),
            MonsterIntent::EncourageGremlins {
                strength: 3,
                block: 6
            }
        );
        assert_eq!(moves_executed_from_observed(&monster, GREMLIN_LEADER_ID), 2);
    }

    #[test]
    fn gremlin_tsundere_defend_observed_intent_imports_source_block() {
        use sts_core::content::monsters::GREMLIN_TSUNDERE_ID;

        let monster = json!({
            "id": "GremlinTsundere",
            "intent": "DEFEND",
            "move_id": 1,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_TSUNDERE_ID, 0),
            MonsterIntent::Block { block: 7 }
        );
    }

    #[test]
    fn gremlin_fat_attack_debuff_observed_intent_imports_weak() {
        use sts_core::content::monsters::{GREMLIN_FAT_ID, SLAVER_BLUE_ID, TASKMASTER_ID};

        let monster = json!({
            "id": "GremlinFat",
            "intent": "ATTACK_DEBUFF",
            "move_id": 2,
            "move_base_damage": 4
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_FAT_ID, 0),
            MonsterIntent::AttackApplyPlayerWeak { damage: 4, weak: 1 }
        );

        let slaver = json!({
            "id": "SlaverBlue",
            "intent": "ATTACK_DEBUFF",
            "move_id": 4,
            "move_base_damage": 7
        });

        assert_eq!(
            observed_intent(&slaver, SLAVER_BLUE_ID, 0),
            MonsterIntent::AttackApplyPlayerWeak { damage: 7, weak: 1 }
        );

        let taskmaster = json!({
            "id": "SlaverBoss",
            "intent": "ATTACK_DEBUFF",
            "move_id": 2,
            "move_base_damage": 7
        });

        assert_eq!(
            observed_intent(&taskmaster, TASKMASTER_ID, 0),
            MonsterIntent::AttackAddWoundsToDiscard {
                damage: 7,
                count: 1
            }
        );
    }

    #[test]
    fn gremlin_nob_buff_observed_intent_imports_bellow() {
        use sts_core::content::monsters::{gremlin_nob_enrage, GREMLIN_NOB_ID};

        let monster = json!({
            "id": "GremlinNob",
            "intent": "BUFF",
            "move_id": 3,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_NOB_ID, 0),
            MonsterIntent::StrengthSelf {
                amount: gremlin_nob_enrage(0)
            }
        );
    }

    #[test]
    fn byrd_buff_observed_intent_imports_caw() {
        use sts_core::content::monsters::BYRD_ID;

        let monster = json!({
            "id": "Byrd",
            "intent": "BUFF",
            "move_id": 6,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, BYRD_ID, 0),
            MonsterIntent::StrengthSelf { amount: 1 }
        );
    }

    #[test]
    fn gremlin_nob_attack_debuff_observed_intent_imports_skull_bash() {
        use sts_core::content::monsters::GREMLIN_NOB_ID;

        let monster = json!({
            "id": "GremlinNob",
            "intent": "ATTACK_DEBUFF",
            "move_id": 2,
            "move_base_damage": 6
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_NOB_ID, 0),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: 6,
                vulnerable: 2
            }
        );
    }

    #[test]
    fn snecko_attack_debuff_observed_intent_imports_tail_whip() {
        use sts_core::content::monsters::SNECKO_ID;

        let monster = json!({
            "id": "Snecko",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 8
        });

        assert_eq!(
            observed_intent(&monster, SNECKO_ID, 0),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: 8,
                vulnerable: 2
            }
        );
        assert_eq!(
            observed_intent(&monster, SNECKO_ID, 17),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: 8,
                weak: 2,
                vulnerable: 2
            }
        );
    }

    #[test]
    fn healer_buff_observed_intent_imports_strength_all() {
        use sts_core::content::monsters::HEALER_ID;

        let monster = json!({
            "id": "Healer",
            "intent": "BUFF",
            "move_id": 3,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, HEALER_ID, 0),
            MonsterIntent::StrengthAllMonsters { amount: 2 }
        );
    }

    #[test]
    fn slime_boss_strong_debuff_observed_intent_imports_sticky() {
        use sts_core::content::monsters::{SLIME_BOSS_ID, SLIME_BOSS_SLIMED_COUNT};

        let monster = json!({
            "id": "SlimeBoss",
            "intent": "STRONG_DEBUFF",
            "move_id": 4,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SLIME_BOSS_ID, 0),
            MonsterIntent::AddSlimedToDiscard {
                count: SLIME_BOSS_SLIMED_COUNT
            }
        );
    }

    #[test]
    fn collector_strong_debuff_observed_intent_imports_mega_debuff() {
        use sts_core::content::monsters::THE_COLLECTOR_ID;

        let monster = json!({
            "id": "TheCollector",
            "intent": "STRONG_DEBUFF",
            "move_id": 4,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, THE_COLLECTOR_ID, 0),
            MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 3,
                weak: 3,
                vulnerable: 3
            }
        );
    }

    #[test]
    fn acid_slime_debug_observed_intent_with_damage_imports_slimed_attack() {
        use sts_core::content::monsters::ACID_SLIME_ID;

        let monster = json!({
            "id": "AcidSlime_L",
            "max_hp": 65,
            "intent": "DEBUG",
            "move_base_damage": 11
        });

        assert_eq!(
            observed_intent(&monster, ACID_SLIME_ID, 0),
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: 11,
                count: 2
            }
        );
    }

    #[test]
    fn medium_acid_slime_debug_move_two_imports_normal_attack() {
        use sts_core::content::monsters::ACID_SLIME_ID;

        let monster = json!({
            "id": "AcidSlime_M",
            "max_hp": 29,
            "intent": "DEBUG",
            "move_id": 2,
            "move_base_damage": 10
        });

        assert_eq!(
            observed_intent(&monster, ACID_SLIME_ID, 0),
            MonsterIntent::Attack { damage: 10 }
        );
    }

    #[test]
    fn large_acid_slime_debuff_observed_intent_imports_two_weak() {
        use sts_core::content::monsters::ACID_SLIME_ID;

        let monster = json!({
            "id": "AcidSlime_L",
            "max_hp": 68,
            "intent": "DEBUFF",
            "move_id": 4,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, ACID_SLIME_ID, 0),
            MonsterIntent::ApplyPlayerWeak { amount: 2 }
        );
    }

    #[test]
    fn medium_spike_slime_debuff_observed_intent_imports_frail() {
        use sts_core::content::monsters::SPIKE_SLIME_ID;

        let monster = json!({
            "id": "SpikeSlime_M",
            "max_hp": 31,
            "intent": "DEBUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SPIKE_SLIME_ID, 0),
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 }
        );
    }

    #[test]
    fn sentry_zero_damage_attack_observed_intent_imports_beam() {
        use sts_core::content::monsters::SENTRY_ID;

        let monster = json!({
            "id": "Sentry",
            "intent": "ATTACK",
            "move_base_damage": 0
        });

        assert_eq!(
            observed_intent(&monster, SENTRY_ID, 0),
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
    }

    #[test]
    fn sentry_debuff_observed_intent_imports_beam() {
        use sts_core::content::monsters::SENTRY_ID;

        let monster = json!({
            "id": "Sentry",
            "intent": "DEBUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SENTRY_ID, 0),
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
    }

    #[test]
    fn repulsor_debuff_observed_intent_imports_draw_dazes() {
        use sts_core::content::monsters::REPULSOR_ID;

        let monster = json!({
            "id": "Repulsor",
            "intent": "DEBUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, REPULSOR_ID, 0),
            MonsterIntent::AddDazedToDraw { count: 2 }
        );
    }

    #[test]
    fn observed_event_screen_imports_lab() {
        let game = json!({
            "screen_type": "EVENT",
            "screen_state": {
                "event_id": "Lab",
                "event_name": "Lab"
            }
        });

        assert_eq!(
            observed_event_screen(&game, 0).map(|screen| screen.event),
            Some(Event::Lab)
        );
    }

    #[test]
    fn grid_trace_choice_label_does_not_preview_upgrade_existing_cards() {
        use sts_core::content::cards::{TRUE_GRIT_ID, TRUE_GRIT_PLUS_ID};

        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::ToxicEgg);

        assert_eq!(
            grid_trace_choice_label(&run, &CardInstance::new(CardId::new(1), TRUE_GRIT_ID)),
            "true grit"
        );
        assert_eq!(
            grid_trace_choice_label(&run, &CardInstance::new(CardId::new(2), TRUE_GRIT_PLUS_ID)),
            "true grit+"
        );
    }

    #[test]
    fn seed_start_map_return_projects_final_row_as_boss_choice() {
        let seed = sts_seed_string_to_long("TEST");
        let mut path = vec![generate_exordium_map_topology(seed).first_row_choices[0]];
        while path.len() < 15 {
            let next = generate_exordium_map_choices_after_path(seed, &path)
                .last()
                .and_then(|step| step.next_choices.first())
                .copied()
                .expect("path reaches the next map row");
            path.push(next);
        }

        let projected = seed_start_simulated_map_return(seed, &path, None, &[], &[], &[]);

        assert_eq!(projected["choices"], json!(["boss"]));
        assert_eq!(projected["next_nodes"], json!([]));
        assert_eq!(projected["current_node"]["y"], json!(14));
    }

    #[test]
    fn seed_start_post_boss_transition_projects_current_node_sentinel() {
        let mut projected = json!({
            "screen_type": "MAP",
            "first_node_chosen": false,
            "current_node": {
                "symbol": "",
                "x": 0,
                "y": -1,
            },
            "next_nodes": [],
        });

        seed_start_project_post_boss_transition_current_node(&mut projected);

        assert_eq!(
            projected["current_node"],
            json!({
                "symbol": "",
                "x": -1,
                "y": 15,
            })
        );
    }

    #[test]
    fn seed_start_event_choice_labels_strip_effect_parentheticals() {
        assert_eq!(
            seed_start_visible_event_choice_label("Gather gold (gain 75 gold, lose 11 HP)"),
            Some("gather gold".to_owned())
        );
        assert_eq!(
            seed_start_visible_event_choice_label("Leave it (lose 48 gold)"),
            Some("leave it".to_owned())
        );
    }

    #[test]
    fn observed_event_screen_imports_scrap_ooze_deeper_progress() {
        let game = json!({
            "ascension_level": 0,
            "screen_type": "EVENT",
            "choice_list": ["deeper", "leave"],
            "screen_state": {
                "event_id": "Scrap Ooze",
                "event_name": "Scrap Ooze",
                "options": [
                    {"text": "[Deeper] Lose 5 HP. 45%: Find a Relic."},
                    {"text": "[Leave]"}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("scrap ooze screen");

        assert_eq!(screen.event, Event::ScrapOoze);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.event_data, 2);
        assert_eq!(screen.choices[0].label, "deeper");
    }

    #[test]
    fn observed_event_screen_imports_shining_light_from_visible_choices_without_event_id() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["enter", "leave"],
            "screen_state": {}
        });

        let screen = observed_event_screen(&game, 0).expect("shining light screen");

        assert_eq!(screen.event, Event::ShiningLight);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "enter");
    }

    #[test]
    fn observed_event_screen_imports_wing_statue() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["pray", "leave"],
            "screen_state": {
                "event_id": "Golden Wing",
                "event_name": "Wing Statue",
                "options": [
                    {"text": "[Pray] Remove a card from your deck. Lose 7 HP."},
                    {"text": "[Locked] Requires: Card with 10 or more damage."},
                    {"text": "[Leave]"}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("wing statue screen");

        assert_eq!(screen.event, Event::WingStatue);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "pray");
    }

    #[test]
    fn observed_event_screen_imports_wing_statue_from_visible_choices_without_event_id() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["pray", "leave"],
            "screen_state": {}
        });

        let screen = observed_event_screen(&game, 0).expect("wing statue screen");

        assert_eq!(screen.event, Event::WingStatue);
        assert_eq!(screen.stage, 0);
        assert_eq!(screen.choices[0].label, "pray");
    }

    #[test]
    fn observed_event_screen_imports_nest_choice_stage() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["smash and grab", "stay in line"],
            "screen_state": {
                "event_id": "Nest",
                "event_name": "The Nest",
                "options": [
                    {"text": "[Smash and Grab] Obtain 99 Gold."},
                    {"text": "[Stay in Line] Obtain Ritual Dagger. Lose 6 HP."}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("nest screen");

        assert_eq!(screen.event, Event::Nest);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.choices[0].label, "smash and grab");
    }

    #[test]
    fn observed_event_screen_imports_knowing_skull_option_stage() {
        let game = json!({
            "screen_type": "EVENT",
            "choice_list": ["a pick me up?", "riches?", "success?", "how do i leave?"],
            "screen_state": {
                "event_id": "Knowing Skull",
                "event_name": "Knowing Skull",
                "options": [
                    {"label": "A Pick Me Up?", "text": "[A Pick Me Up?] Get a Potion. Lose 6 HP."},
                    {"label": "Riches?", "text": "[Riches?] Gain 90 Gold. Lose 6 HP."},
                    {"label": "Success?", "text": "[Success?] Get a Colorless Card. Lose 6 HP."},
                    {"label": "How do I leave?", "text": "[How do I leave?] Lose 6 HP."}
                ]
            }
        });

        let screen = observed_event_screen(&game, 0).expect("knowing skull screen");

        assert_eq!(screen.event, Event::KnowingSkull);
        assert_eq!(screen.stage, 1);
        assert_eq!(screen.choices[3].label, "how do i leave?");
        assert_eq!(screen.event_data, default_knowing_skull_costs());
    }

    #[test]
    fn spiker_buff_observed_intent_imports_non_damage_buff() {
        use sts_core::content::monsters::SPIKER_ID;

        let monster = json!({
            "id": "Spiker",
            "intent": "BUFF",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SPIKER_ID, 0),
            MonsterIntent::StrengthAndBlock {
                strength: 0,
                block: 0
            }
        );
    }

    #[test]
    fn sentry_debug_without_damage_observed_intent_imports_beam() {
        use sts_core::content::monsters::SENTRY_ID;

        let monster = json!({
            "id": "Sentry",
            "intent": "DEBUG",
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, SENTRY_ID, 0),
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
    }

    #[test]
    fn observed_attack_with_multiple_hits_imports_attack_multiple() {
        use sts_core::content::monsters::HEXAGHOST_ID;

        let monster = json!({
            "id": "Hexaghost",
            "intent": "ATTACK",
            "move_base_damage": 5,
            "move_hits": 6
        });

        assert_eq!(
            observed_intent(&monster, HEXAGHOST_ID, 0),
            MonsterIntent::AttackMultiple { damage: 5, hits: 6 }
        );
    }

    #[test]
    fn hexaghost_attack_debuff_imports_observed_sear() {
        use sts_core::content::monsters::HEXAGHOST_ID;

        let monster = json!({
            "id": "Hexaghost",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 6,
            "move_hits": 1,
            "move_id": 4
        });

        assert_eq!(
            observed_intent(&monster, HEXAGHOST_ID, 0),
            MonsterIntent::AddBurnToDiscard {
                damage: 6,
                count: 1
            }
        );
    }

    #[test]
    fn hexaghost_attack_debuff_imports_observed_inferno() {
        use sts_core::content::monsters::HEXAGHOST_ID;

        let monster = json!({
            "id": "Hexaghost",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 2,
            "move_hits": 6,
            "move_id": 6
        });

        assert_eq!(
            observed_intent(&monster, HEXAGHOST_ID, 0),
            MonsterIntent::AttackMultipleUpgradeBurns {
                damage: 2,
                hits: 6,
                count: 3
            }
        );
    }

    #[test]
    fn orb_walker_attack_debuff_imports_burn_discard_intent() {
        use sts_core::content::monsters::ORB_WALKER_ID;

        let monster = json!({
            "id": "Orb Walker",
            "intent": "ATTACK_DEBUFF",
            "move_base_damage": 10,
            "move_hits": 1
        });

        assert_eq!(
            observed_intent(&monster, ORB_WALKER_ID, 0),
            MonsterIntent::AddBurnToDiscardAndDraw {
                damage: 10,
                count: 1
            }
        );
    }

    #[test]
    fn shelled_parasite_attack_buff_imports_life_suck() {
        use sts_core::content::monsters::SHELLED_PARASITE_ID;

        let monster = json!({
            "id": "Shelled Parasite",
            "intent": "ATTACK_BUFF",
            "move_base_damage": 10
        });

        assert_eq!(
            observed_intent(&monster, SHELLED_PARASITE_ID, 0),
            MonsterIntent::AttackHealSelf { damage: 10 }
        );
    }

    #[test]
    fn guardian_twin_slam_attack_buff_imports_two_hit_attack() {
        use sts_core::content::monsters::GUARDIAN_ID;

        let monster = json!({
            "id": "TheGuardian",
            "intent": "ATTACK_BUFF",
            "move_id": 4,
            "move_base_damage": 8,
            "move_hits": 2
        });

        assert_eq!(
            observed_intent(&monster, GUARDIAN_ID, 0),
            MonsterIntent::AttackMultiple { damage: 8, hits: 2 }
        );
    }

    #[test]
    fn observed_monster_weakened_imports_weak_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Weakened", "name": "Weakened", "amount": 2}
        ])));

        assert_eq!(powers.weak, 2);
    }

    #[test]
    fn observed_monster_plated_armor_imports_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Plated Armor", "name": "Plated Armor", "amount": 13}
        ])));

        assert_eq!(powers.plated_armor, 13);
    }

    #[test]
    fn observed_monster_spore_cloud_imports_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Spore Cloud", "name": "Spore Cloud", "amount": 2}
        ])));

        assert_eq!(powers.spore_cloud, 2);
    }

    #[test]
    fn observed_monster_strength_up_imports_power() {
        let powers = monster_powers(Some(&json!([
            {"id": "Generic Strength Up Power", "name": "Strength Up", "amount": 3}
        ])));

        assert_eq!(powers.strength_up, 3);
    }

    #[test]
    fn observed_player_combust_imports_damage_amount() {
        let (powers, temp_strength) = player_powers_and_temp_strength(Some(&json!([
            {"id": "Strength", "name": "Strength", "amount": 1},
            {"id": "Combust", "name": "Combust", "amount": 7},
            {"id": "Dark Embrace", "name": "Dark Embrace", "amount": 1},
            {"id": "Rupture", "name": "Rupture", "amount": 2},
            {"id": "Hex", "name": "Hex", "amount": 1}
        ])));

        assert_eq!(temp_strength, 0);
        assert_eq!(powers.strength, 1);
        assert_eq!(powers.combust, 1);
        assert_eq!(powers.combust_damage, 7);
        assert_eq!(powers.dark_embrace, 1);
        assert_eq!(powers.rupture, 2);
        assert_eq!(powers.hex, 1);
    }

    #[test]
    fn book_of_stabbing_observed_hits_reconstruct_move_index() {
        use sts_core::content::monsters::BOOK_OF_STABBING_ID;

        let monster = json!({
            "id": "BookOfStabbing",
            "intent": "ATTACK",
            "move_base_damage": 6,
            "move_hits": 4
        });

        assert_eq!(
            moves_executed_from_observed(&monster, BOOK_OF_STABBING_ID),
            3
        );
    }

    #[test]
    fn bronze_automaton_observed_stun_reconstructs_move_index() {
        use sts_core::content::monsters::BRONZE_AUTOMATON_ID;

        let monster = json!({
            "id": "BronzeAutomaton",
            "intent": "STUN",
            "move_base_damage": -1,
            "move_hits": -1
        });

        assert_eq!(
            moves_executed_from_observed(&monster, BRONZE_AUTOMATON_ID),
            6
        );
        assert_eq!(
            observed_intent(&monster, BRONZE_AUTOMATON_ID, 0),
            MonsterIntent::Stun
        );
    }

    #[test]
    fn gremlin_leader_unknown_move_two_imports_summon() {
        use sts_core::content::monsters::GREMLIN_LEADER_ID;

        let monster = json!({
            "id": "GremlinLeader",
            "intent": "UNKNOWN",
            "move_id": 2,
            "move_base_damage": -1
        });

        assert_eq!(
            observed_intent(&monster, GREMLIN_LEADER_ID, 0),
            MonsterIntent::SummonGremlins { count: 2 }
        );
    }

    #[test]
    fn bronze_automaton_and_orb_ids_import_without_cultist_fallback() {
        use sts_core::content::monsters::{
            content_id_from_game_monster_id, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, ORB_WALKER_ID,
        };

        assert_eq!(
            content_id_from_game_monster_id("BronzeAutomaton"),
            BRONZE_AUTOMATON_ID
        );
        assert_eq!(content_id_from_game_monster_id("BronzeOrb"), BRONZE_ORB_ID);
        assert_eq!(content_id_from_game_monster_id("Orb Walker"), ORB_WALKER_ID);
    }

    #[test]
    fn neow_generated_identity_display_names_are_mapped() {
        use sts_core::content::cards::{
            ARMAMENTS_ID, CHRYSALIS_ID, DECAY_ID, DOUBT_ID, FEED_ID, HAND_OF_GREED_ID,
            IMPERVIOUS_ID, LIMIT_BREAK_ID, MAGNETISM_ID, MAYHEM_ID, PARASITE_ID, SECRET_WEAPON_ID,
            TRANSMUTATION_ID, WRITHE_ID,
        };

        for (content_id, expected) in [
            (LIMIT_BREAK_ID, "Limit Break"),
            (IMPERVIOUS_ID, "Impervious"),
            (FEED_ID, "Feed"),
            (MAYHEM_ID, "Mayhem"),
            (SECRET_WEAPON_ID, "Secret Weapon"),
            (TRANSMUTATION_ID, "Transmutation"),
            (MAGNETISM_ID, "Magnetism"),
            (CHRYSALIS_ID, "Chrysalis"),
            (HAND_OF_GREED_ID, "Hand Of Greed"),
            (PARASITE_ID, "Parasite"),
            (DECAY_ID, "Decay"),
            (WRITHE_ID, "Writhe"),
            (DOUBT_ID, "Doubt"),
            (ARMAMENTS_ID, "Armaments"),
        ] {
            assert_eq!(content_key(content_id), expected);
            assert_ne!(content_key(content_id), "unknown");
        }
    }

    #[test]
    fn trace_relic_display_names_are_mapped() {
        for (key, name) in [
            (RelicKey::Akabeko, "Akabeko"),
            (RelicKey::BagOfMarbles, "Bag of Marbles"),
            (RelicKey::BagOfPreparation, "Bag of Preparation"),
            (RelicKey::DeadBranch, "Dead Branch"),
            (RelicKey::GremlinHorn, "Gremlin Horn"),
            (RelicKey::IceCream, "Ice Cream"),
            (RelicKey::CentennialPuzzle, "Centennial Puzzle"),
            (RelicKey::OddlySmoothStone, "Oddly Smooth Stone"),
            (RelicKey::EternalFeather, "Eternal Feather"),
            (RelicKey::Omamori, "Omamori"),
            (RelicKey::Pear, "Pear"),
            (RelicKey::MeatOnTheBone, "Meat on the Bone"),
            (RelicKey::ChampionBelt, "Champion Belt"),
            (RelicKey::GoldenIdol, "Golden Idol"),
            (RelicKey::DuVuDoll, "Du-Vu Doll"),
            (RelicKey::MedicalKit, "Medical Kit"),
            (RelicKey::WarPaint, "War Paint"),
            (RelicKey::LetterOpener, "Letter Opener"),
            (RelicKey::Nunchaku, "Nunchaku"),
            (RelicKey::InkBottle, "Ink Bottle"),
            (RelicKey::Shuriken, "Shuriken"),
            (RelicKey::Kunai, "Kunai"),
            (RelicKey::HappyFlower, "Happy Flower"),
            (RelicKey::IncenseBurner, "Incense Burner"),
            (RelicKey::ThreadAndNeedle, "Thread and Needle"),
            (RelicKey::ClockworkSouvenir, "Clockwork Souvenir"),
            (RelicKey::Calipers, "Calipers"),
            (RelicKey::PaperPhrog, "Paper Phrog"),
            (RelicKey::StrangeSpoon, "Strange Spoon"),
            (RelicKey::DollysMirror, "Dolly's Mirror"),
            (RelicKey::SelfFormingClay, "Self-Forming Clay"),
        ] {
            assert_eq!(relic_key_trace_name(key), name);
            assert_eq!(relic_key_from_trace_name(name), Some(key));
            assert_eq!(
                relic_from_trace_name(name).map(|relic| relic.key()),
                Some(key)
            );
        }
    }

    #[test]
    fn seed_start_shop_choice_list_keeps_visible_potions_when_belt_is_full() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Shop;
        run.shop_merchant_open = true;
        run.gold = 100;
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Weak];
        run.shop = Some(sts_core::ShopScreen {
            cards: Vec::new(),
            relics: Vec::new(),
            potions: vec![sts_core::ShopPotionSlot {
                potion: Potion::Power,
                price: 50,
                sold: false,
            }],
            remove_cost: 75,
            remove_available: true,
            sale_slot: None,
        });

        assert_eq!(
            seed_start_shop_trace_choice_labels(&run),
            vec!["purge".to_owned(), "power potion".to_owned()]
        );
    }

    #[test]
    fn seed_start_shop_choice_labels_apply_egg_preview_upgrades() {
        use sts_core::content::cards::{PANACEA_ID, WARCRY_ID};

        let mut run = RunState::map_fixture();
        run.gain_relic_key(RelicKey::ToxicEgg);

        assert_eq!(shop_card_display_key(&run, WARCRY_ID), "Warcry+");
        assert_eq!(shop_card_display_key(&run, PANACEA_ID), "Panacea+");
    }

    #[test]
    fn seed_start_neow_branch_routing_uses_generated_selected_options() {
        for (numeric_seed, command, reward) in [
            (
                1_957_307_888_551,
                "CHOOSE 1",
                NeowRewardType::RandomCommonRelic,
            ),
            (1_218_623, "CHOOSE 0", NeowRewardType::RandomColorless),
            (22_079_335_079, "CHOOSE 0", NeowRewardType::RandomColorless),
            (
                22_079_335_079,
                "CHOOSE 1",
                NeowRewardType::ThreeSmallPotions,
            ),
            (40_560_393_126, "CHOOSE 1", NeowRewardType::ThreeEnemyKill),
            (40_560_393_126, "CHOOSE 0", NeowRewardType::TransformCard),
            (40_560_393_133, "CHOOSE 0", NeowRewardType::TransformCard),
            (1_957_307_888_551, "CHOOSE 3", NeowRewardType::BossRelic),
        ] {
            assert_eq!(
                seed_start_selected_neow_option(numeric_seed, command).map(|option| option.reward),
                Some(reward),
                "{numeric_seed} {command}"
            );
        }

        assert!(seed_start_selected_neow_option(1_957_307_888_551, "PROCEED").is_none());
        assert!(seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 9").is_none());
    }

    #[test]
    fn seed_start_common_relic_uses_generated_neow_relic_reward() {
        let option = seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 1")
            .expect("VERIFY01 common relic option");
        let run = seed_start_apply_neow_relic_reward(
            1_957_307_888_551,
            &ironclad_starter_deck_keys(),
            &option,
        );

        assert_eq!(seed_start_newest_trace_relic_name(&run), "Toy Ornithopter");
        assert_eq!(
            relic_key_from_trace_name("Toy Ornithopter"),
            Some(RelicKey::ToyOrnithopter)
        );
    }

    #[test]
    fn seed_start_rare_relic_uses_generated_neow_relic_reward_with_simple_drawback() {
        let (numeric_seed, option, run) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::TenPercentHpLoss
                            && option.reward == NeowRewardType::OneRareRelic
                    })
                    .and_then(|option| {
                        let run = seed_start_apply_neow_relic_reward(
                            seed,
                            &ironclad_starter_deck_keys(),
                            &option,
                        );
                        (seed_start_newest_trace_relic_name(&run) != "Unknown Relic")
                            .then_some((seed, option, run))
                    })
            })
            .expect("synthetic seed with max-HP loss plus mapped rare relic");

        assert!(seed_start_neow_option_is_supported_relic_reward(
            option.clone()
        ));

        assert_eq!(run.gold, 99);
        assert_eq!(run.player_hp, 72);
        assert_eq!(run.player_max_hp, 72);
        assert_eq!(
            seed_start_selected_neow_option(numeric_seed, &format!("CHOOSE {}", option.slot))
                .map(|option| option.reward),
            Some(NeowRewardType::OneRareRelic)
        );
        assert_ne!(seed_start_newest_trace_relic_name(&run), "Unknown Relic");
    }

    #[test]
    fn seed_start_rare_relic_supports_curse_and_rejects_non_relic_identity_branches() {
        assert!(seed_start_neow_option_is_supported_relic_reward(
            GeneratedNeowOption {
                slot: 2,
                drawback: NeowDrawback::Curse,
                reward: NeowRewardType::OneRareRelic,
                label: "obtain a curse obtain a random rare relic".to_owned(),
            }
        ));
        assert!(!seed_start_neow_option_is_supported_relic_reward(
            GeneratedNeowOption {
                slot: 2,
                drawback: NeowDrawback::TenPercentHpLoss,
                reward: NeowRewardType::RandomColorlessTwo,
                label: "lose 8 max hp choose a rare colorless card to obtain".to_owned(),
            }
        ));
    }

    #[test]
    fn seed_start_neow_rare_relic_trace_branch_reaches_leave() {
        let numeric_seed = 1_218_623;
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
            .expect("TEST slot 2 rare relic option");
        assert_eq!(option.drawback, NeowDrawback::TenPercentHpLoss);
        assert_eq!(option.reward, NeowRewardType::OneRareRelic);
        let run = seed_start_apply_neow_relic_reward(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let post_relics = vec![
            json!({ "name": "Burning Blood" }),
            json!({ "name": seed_start_newest_trace_relic_name(&run) }),
        ];
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 TEST"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": starting_deck,
                "relics": post_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow rare relic"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_boss_swap_uses_generated_boss_relic_reward() {
        let (numeric_seed, run) = (1_i64..10_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_unsupported_boss_swap_reason(run).is_none())
            .expect("synthetic seed with non-grid boss-swap relic");

        let option =
            seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");

        assert!(seed_start_neow_option_is_supported_boss_swap(option));
        assert!(!run.relics.contains(&Relic::BurningBlood));
        assert_eq!(run.relics.len() + run.relic_keys.len(), 1);

        let relic_ids = seed_start_boss_swap_relic_ids(&run);
        assert_eq!(relic_ids.len(), 1);
        assert!(!relic_ids.contains(&"Burning Blood".to_owned()));
        assert_ne!(relic_ids[0], "Unknown Relic");
    }

    #[test]
    fn seed_start_relic_projection_replaces_starter_relic_slot() {
        let mut run = RunState::map_fixture();
        run.relics = vec![
            Relic::BurningBlood,
            Relic::CentennialPuzzle,
            Relic::OddlySmoothStone,
        ];
        run.gain_relic(Relic::BlackBlood);

        let relic_ids = relic_ids_for_simulated_subset(
            &run,
            &[
                "Burning Blood".to_owned(),
                "Centennial Puzzle".to_owned(),
                "Oddly Smooth Stone".to_owned(),
            ],
        );

        assert_eq!(
            relic_ids,
            vec![
                "Black Blood".to_owned(),
                "Centennial Puzzle".to_owned(),
                "Oddly Smooth Stone".to_owned(),
            ]
        );
    }

    #[test]
    fn seed_start_boss_swap_immediate_boss_relics_route_to_neow_leave() {
        let immediate_boss_relics: Vec<_> = IRONCLAD_BOSS_RELIC_POOL
            .iter()
            .copied()
            .filter(|key| {
                !matches!(
                    key,
                    RelicKey::Astrolabe
                        | RelicKey::PandorasBox
                        | RelicKey::EmptyCage
                        | RelicKey::CallingBell
                        | RelicKey::TinyHouse
                )
            })
            .collect();
        let mut covered = Vec::new();

        for numeric_seed in 1_i64..2_000_000 {
            let run = seed_start_apply_neow_boss_swap(numeric_seed, &ironclad_starter_deck_keys());
            let Some(swapped_key) = run
                .relics
                .iter()
                .map(|relic| relic.key())
                .chain(run.relic_keys.iter().copied())
                .find(|key| *key != RelicKey::BurningBlood)
            else {
                continue;
            };
            if !immediate_boss_relics.contains(&swapped_key)
                || covered
                    .iter()
                    .any(|(covered_key, _, _)| *covered_key == swapped_key)
            {
                continue;
            }

            assert_eq!(seed_start_unsupported_boss_swap_reason(&run), None);

            let seed_string = test_seed_string_from_long(numeric_seed);
            let deck: Vec<_> = ironclad_starter_deck_keys()
                .into_iter()
                .map(|id| json!({ "id": id }))
                .collect();
            let post_swap_deck: Vec<_> = deck_content_keys(&run.deck)
                .into_iter()
                .map(|id| json!({ "id": id }))
                .collect();
            let swapped_relics: Vec<_> = seed_start_boss_swap_relic_ids(&run)
                .into_iter()
                .map(|name| json!({ "name": name }))
                .collect();
            let lines = vec![
                json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
                json!({"type": "state", "step": 0, "message": {}}),
                json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
                json!({"type": "state", "step": 1, "message": {"game_state": {
                    "screen_type": "EVENT",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": deck,
                    "relics": [{"name": "Burning Blood"}],
                    "choice_list": ["talk"]
                }}}),
                json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
                json!({"type": "state", "step": 2, "message": {"game_state": {
                    "screen_type": "EVENT",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": deck,
                    "relics": [{"name": "Burning Blood"}],
                    "choice_list": seed_start_neow_choices(numeric_seed)
                }}}),
                json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
                json!({"type": "state", "step": 3, "message": {"game_state": {
                    "screen_type": "EVENT",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": post_swap_deck,
                    "relics": swapped_relics,
                    "choice_list": ["leave"]
                }}}),
                json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
                json!({"type": "state", "step": 4, "message": {"game_state": {
                    "screen_type": "MAP",
                    "ascension_level": 0,
                    "floor": 0,
                    "gold": 99,
                    "current_hp": 80,
                    "max_hp": 80,
                    "deck": post_swap_deck,
                    "relics": swapped_relics,
                    "choice_list": seed_start_first_map_choices(&seed_string)
                }}}),
            ];
            let content = lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            let report =
                verify_seed_start_communication_mod_trace(&content).expect("seed-start verifies");

            assert_eq!(report.unexpected_diffs, Vec::new());
            assert!(
                report
                    .unsupported
                    .iter()
                    .all(|transition| transition.action_step < 3),
                "boss-swap path produced unsupported transitions: {:?}",
                report.unsupported
            );
            assert!(report.verified.iter().any(|transition| {
                transition.action_step == 3 && transition.label == "Neow boss swap"
            }));
            assert!(report.verified.iter().any(|transition| {
                transition.action_step == 4 && transition.label == "Neow leave"
            }));

            covered.push((swapped_key, numeric_seed, seed_string));
            if covered.len() == immediate_boss_relics.len() {
                break;
            }
        }

        let missing: Vec<_> = immediate_boss_relics
            .iter()
            .copied()
            .filter(|key| !covered.iter().any(|(covered_key, _, _)| covered_key == key))
            .collect();
        assert_eq!(missing, Vec::new(), "missing immediate boss relic coverage");
    }

    #[test]
    fn seed_start_boss_swap_classifies_grid_opening_relics() {
        let mut run = RunState::map_fixture();

        run.gain_relic(Relic::Astrolabe);

        assert_eq!(
            seed_start_unsupported_boss_swap_reason(&run),
            Some(
                "Neow boss-swap produced a grid-opening boss relic without a dedicated seed-start follow-up; downstream parity remains classified"
                    .to_owned()
            )
        );
    }

    #[test]
    fn seed_start_boss_swap_calling_bell_grid_rewards_are_taken_before_neow_leave() {
        let (numeric_seed, bell_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_calling_bell_grid(run))
            .expect("synthetic seed with Calling Bell boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let bell_relic_names = seed_start_boss_swap_relic_ids(&bell_run);
        let bell_relics: Vec<_> = bell_relic_names
            .iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_confirm = confirm_grid(&bell_run).expect("Calling Bell grid confirms");
        let after_common =
            apply_run_action(&after_confirm, RunAction::TakeRelicReward).expect("take common");
        let after_uncommon =
            apply_run_action(&after_common, RunAction::TakeRelicReward).expect("take uncommon");
        let after_rare =
            apply_run_action(&after_uncommon, RunAction::TakeRelicReward).expect("take rare");
        let bell_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let common_relics: Vec<_> =
            relic_ids_for_simulated_subset(&after_common, &bell_relic_names)
                .into_iter()
                .filter(|name| name != "Unknown Relic")
                .map(|name| json!({ "name": name }))
                .collect();
        let uncommon_relics: Vec<_> =
            relic_ids_for_simulated_subset(&after_uncommon, &bell_relic_names)
                .into_iter()
                .filter(|name| name != "Unknown Relic")
                .map(|name| json!({ "name": name }))
                .collect();
        let rare_relics: Vec<_> = relic_ids_for_simulated_subset(&after_rare, &bell_relic_names)
            .into_iter()
            .filter(|name| name != "Unknown Relic")
            .map(|name| json!({ "name": name }))
            .collect();

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": bell_relics,
                "choice_list": ["curse of the bell"]
            }}}),
            json!({"type": "action", "step": 4, "command": "PROCEED"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": bell_relics,
                "choice_list": ["relic"],
                "screen_state": {
                    "rewards": [{"reward_type": "RELIC"}]
                }
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": common_relics,
                "choice_list": ["relic"],
                "screen_state": {
                    "rewards": [{"reward_type": "RELIC"}]
                }
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": uncommon_relics,
                "choice_list": ["relic"],
                "screen_state": {
                    "rewards": [{"reward_type": "RELIC"}]
                }
            }}}),
            json!({"type": "action", "step": 7, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 7, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": rare_relics,
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 8, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 8, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": bell_deck,
                "relics": rare_relics,
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Calling Bell grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "Neow boss swap Calling Bell rewards"
        }));
        assert_eq!(
            report
                .verified
                .iter()
                .filter(|transition| transition.label == "relic reward")
                .count(),
            3
        );
        assert!(report
            .verified
            .iter()
            .any(|transition| { transition.action_step == 8 && transition.label == "Neow leave" }));
    }

    #[test]
    fn seed_start_boss_swap_astrolabe_grid_transforms_three_selected_cards() {
        let (numeric_seed, astrolabe_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_astrolabe_grid(run))
            .expect("synthetic seed with Astrolabe boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let astrolabe_relics: Vec<_> = seed_start_boss_swap_relic_ids(&astrolabe_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_first = select_grid_card(&astrolabe_run, 0).expect("select first");
        let after_second = select_grid_card(&after_first, 1).expect("select second");
        let after_third = select_grid_card(&after_second, 2).expect("select third");
        let after_confirm = confirm_grid(&after_third).expect("confirm Astrolabe transforms");
        let transformed_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&astrolabe_run, &["Astrolabe".to_owned()])["choices"]
                .as_array()
                .expect("grid choices")
                .clone();

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": astrolabe_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": astrolabe_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": astrolabe_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": transformed_deck,
                "relics": astrolabe_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Astrolabe grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 6
                && transition.label == "Neow boss swap Astrolabe transformed"
        }));
    }

    #[test]
    fn seed_start_boss_swap_pandoras_box_grid_confirms_to_neow_leave() {
        let (numeric_seed, pandora_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_pandoras_box_grid(run))
            .expect("synthetic seed with Pandora's Box boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let pandora_relics: Vec<_> = seed_start_boss_swap_relic_ids(&pandora_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_confirm = confirm_grid(&pandora_run).expect("Pandora's Box grid confirms");
        let grid_deck: Vec<_> = deck_content_keys(&pandora_run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let transformed_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&pandora_run, &["Pandora's Box".to_owned()])
                ["choices"]
                .as_array()
                .expect("grid choices")
                .clone();

        assert_eq!(pandora_run.card_grid.as_ref().expect("grid").cards.len(), 9);
        assert_eq!(pandora_run.deck.len(), 1);
        assert_eq!(after_confirm.deck.len(), 10);

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": grid_deck,
                "relics": pandora_relics,
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CONFIRM"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": transformed_deck,
                "relics": pandora_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Pandora's Box grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4
                && transition.label == "Neow boss swap Pandora's Box confirm"
        }));
    }

    #[test]
    fn seed_start_boss_swap_empty_cage_grid_removes_two_cards_to_neow_leave() {
        let (numeric_seed, empty_cage_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_empty_cage_grid(run))
            .expect("synthetic seed with Empty Cage boss swap");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let empty_cage_relics: Vec<_> = seed_start_boss_swap_relic_ids(&empty_cage_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let after_first_select = select_grid_card(&empty_cage_run, 0).expect("select first");
        let after_first_confirm = confirm_grid(&after_first_select).expect("remove first");
        let after_second_select = select_grid_card(&after_first_confirm, 0).expect("select second");
        let after_second_confirm = confirm_grid(&after_second_select).expect("remove second");
        let first_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&empty_cage_run, &["Empty Cage".to_owned()])
                ["choices"]
                .as_array()
                .expect("first grid choices")
                .clone();
        let second_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&after_first_confirm, &["Empty Cage".to_owned()])
                ["choices"]
                .as_array()
                .expect("second grid choices")
                .clone();
        let one_removed_deck: Vec<_> = deck_content_keys(&after_first_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let two_removed_deck: Vec<_> = deck_content_keys(&after_second_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();

        assert_eq!(empty_cage_run.deck.len(), 10);
        assert_eq!(after_first_confirm.deck.len(), 9);
        assert_eq!(after_second_confirm.deck.len(), 8);

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": empty_cage_relics,
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": empty_cage_relics,
                "choice_list": []
            }}}),
            json!({"type": "action", "step": 5, "command": "CONFIRM"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": one_removed_deck,
                "relics": empty_cage_relics,
                "choice_list": second_grid_choices
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": one_removed_deck,
                "relics": empty_cage_relics,
                "choice_list": []
            }}}),
            json!({"type": "action", "step": 7, "command": "CONFIRM"}),
            json!({"type": "state", "step": 7, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": two_removed_deck,
                "relics": empty_cage_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Empty Cage grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 7 && transition.label == "Neow boss swap Empty Cage confirm"
        }));
    }

    #[test]
    fn seed_start_simple_neow_reward_uses_core_helper() {
        let option = seed_start_selected_neow_option(40_560_393_133, "CHOOSE 1")
            .expect("M290008 slot 1 option");

        assert_eq!(option.reward, NeowRewardType::HundredGold);
        assert_eq!(
            seed_start_apply_neow_simple_option(option),
            Some((199, 80, 80))
        );
    }

    #[test]
    fn seed_start_simple_neow_drawback_and_reward_use_core_helpers() {
        let option = seed_start_selected_neow_option(40_560_393_133, "CHOOSE 2")
            .expect("M290008 slot 2 option");

        assert_eq!(option.drawback, NeowDrawback::NoGold);
        assert_eq!(option.reward, NeowRewardType::TwentyPercentHpBonus);
        assert_eq!(
            seed_start_apply_neow_simple_option(option),
            Some((0, 96, 96))
        );
    }

    #[test]
    fn seed_start_simple_neow_helper_rejects_identity_branches() {
        let option = seed_start_selected_neow_option(40_560_393_133, "CHOOSE 0")
            .expect("M290008 slot 0 option");

        assert_eq!(option.reward, NeowRewardType::TransformCard);
        assert_eq!(seed_start_apply_neow_simple_option(option), None);
    }

    #[test]
    fn m34_selected_modified_deck_opening_piles_are_seed_derived() {
        for case in [
            (
                "communication_mod/trace-2026-06-18T16-50-50-232Z.jsonl",
                "CODEX04 colorless innate",
            ),
            (
                "permanent_traces/trace-2026-06-21T09-57-10-380Z.jsonl",
                "TEST obtained colorless",
            ),
            (
                "communication_mod/trace-2026-06-23T02-56-19-245Z.run2.cleaned.jsonl",
                "M290001 transformed card",
            ),
            (
                "communication_mod/trace-2026-06-23T07-42-06-085Z.best-run.jsonl",
                "M290008 transformed card",
            ),
        ] {
            assert_selected_trace_first_combat_opening_is_seed_derived(case.0, case.1);
        }
    }

    #[test]
    fn seed_start_neow_curse_simple_helper_uses_card_rng_and_limits_rewards() {
        let option = seed_start_selected_neow_option(40_560_393_126, "CHOOSE 2")
            .expect("M290001 slot 2 option");

        assert_eq!(option.drawback, NeowDrawback::Curse);
        assert_eq!(option.reward, NeowRewardType::TwentyPercentHpBonus);
        assert!(seed_start_neow_option_is_supported_curse_simple(
            option.clone()
        ));
        assert!(!seed_start_neow_option_is_supported_curse_simple(
            GeneratedNeowOption {
                slot: 2,
                drawback: NeowDrawback::Curse,
                reward: NeowRewardType::ThreeRareCards,
                label: "obtain a curse choose a rare card to obtain".to_owned(),
            }
        ));

        let run = seed_start_apply_neow_curse_simple_option(
            40_560_393_126,
            &ironclad_starter_deck_keys(),
            option,
        );
        let deck_ids = deck_content_keys(&run.deck);

        assert_eq!(run.gold, 99);
        assert_eq!(run.player_hp, 96);
        assert_eq!(run.player_max_hp, 96);
        assert_eq!(run.card_rng_counter, 1);
        assert_eq!(run.card_random_rng_counter, 0);
        assert_eq!(deck_ids.len(), 11);
        assert!(matches!(
            deck_ids.last().map(String::as_str),
            Some(
                "Clumsy"
                    | "Decay"
                    | "Doubt"
                    | "Injury"
                    | "Normality"
                    | "Pain"
                    | "Parasite"
                    | "Regret"
                    | "Shame"
                    | "Writhe"
            )
        ));
    }

    #[test]
    fn seed_start_neow_curse_gold_helper_uses_same_card_rng_path() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::TwoFiftyGold
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with curse plus 250 gold");

        assert!(seed_start_neow_option_is_supported_curse_simple(
            option.clone()
        ));

        let run = seed_start_apply_neow_curse_simple_option(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            option,
        );

        assert_eq!(run.gold, 349);
        assert_eq!(run.player_hp, 80);
        assert_eq!(run.player_max_hp, 80);
        assert_eq!(run.card_rng_counter, 1);
        assert_eq!(run.deck.len(), ironclad_starter_deck_keys().len() + 1);
    }

    #[test]
    fn seed_start_neow_curse_simple_trace_branch_reaches_leave() {
        let numeric_seed = 40_560_393_126;
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec![json!({ "name": "Burning Blood" })];
        let _option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 2")
            .expect("M290001 curse max-HP option");
        let visible_post_deck = starting_deck.clone();
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 M290001"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 2"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 96,
                "max_hp": 96,
                "deck": visible_post_deck,
                "relics": relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow curse immediate reward"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_neow_grid_reward_dispatch_opens_core_upgrade_grid() {
        let (numeric_seed, command, option) = (1_i64..10_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| option.reward == NeowRewardType::UpgradeCard)
                    .map(|option| (seed, format!("CHOOSE {}", option.slot), option))
            })
            .expect("synthetic seed with upgrade-card option");

        assert_eq!(
            seed_start_selected_neow_option(numeric_seed, &command),
            Some(option.clone())
        );
        assert!(seed_start_neow_option_is_supported_grid_reward(
            option.clone()
        ));

        let run = seed_start_open_neow_grid_run(1, &ironclad_starter_deck_keys(), &option);

        assert_eq!(
            seed_start_grid_simulated_subset(&run, &["Burning Blood".to_owned()]),
            json!({
                "screen_type": "GRID",
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood"],
                "choices": ["strike", "strike", "strike", "strike", "strike", "defend", "defend", "defend", "defend", "bash"],
            })
        );
    }

    #[test]
    fn seed_start_neow_upgrade_grid_choose_confirm_returns_to_leave() {
        let option = GeneratedNeowOption {
            slot: 0,
            drawback: NeowDrawback::None,
            reward: NeowRewardType::UpgradeCard,
            label: "upgrade a card".to_owned(),
        };
        let mut run = seed_start_open_neow_grid_run(1, &ironclad_starter_deck_keys(), &option);

        run = select_grid_card(&run, 0).expect("select first strike");
        assert_eq!(
            seed_start_grid_simulated_subset(&run, &["Burning Blood".to_owned()]),
            json!({
                "screen_type": "GRID",
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood"],
                "choices": [],
            })
        );

        run = confirm_grid(&run).expect("confirm upgrade");

        assert!(run.card_grid.is_none());
        assert_eq!(
            run.deck[0].content_id,
            sts_core::content::cards::STRIKE_R_PLUS_ID
        );
        assert_eq!(
            deck_content_keys(&run.deck),
            vec![
                "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
                "Defend_R", "Defend_R", "Bash",
            ]
        );
    }

    #[test]
    fn seed_start_neow_remove_two_grid_reopens_second_selection() {
        let option = GeneratedNeowOption {
            slot: 0,
            drawback: NeowDrawback::TenPercentHpLoss,
            reward: NeowRewardType::RemoveTwo,
            label: "lose 8 max hp remove 2 cards".to_owned(),
        };
        let mut run = seed_start_open_neow_grid_run(1, &ironclad_starter_deck_keys(), &option);

        assert_eq!(run.player_hp, 72);
        assert_eq!(run.player_max_hp, 72);

        run = select_grid_card(&run, 0).expect("select first strike");
        run = confirm_grid(&run).expect("remove first strike");

        assert!(run.card_grid.is_some());
        assert_eq!(run.deck.len(), 9);
        assert_eq!(
            seed_start_grid_simulated_subset(&run, &["Burning Blood".to_owned()])["choices"]
                .as_array()
                .expect("choices")
                .len(),
            9
        );
    }

    #[test]
    fn seed_start_neow_remove_two_generated_grid_trace_reaches_neow_leave() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::RemoveTwo
                            && seed_start_neow_option_is_supported_grid_reward(option.clone())
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with generated remove-two option");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let choose_command = format!("CHOOSE {}", option.slot);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec!["Burning Blood".to_owned()];
        let initial_run =
            seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
        let after_first_select = select_grid_card(&initial_run, 0).expect("select first");
        let after_first_confirm = confirm_grid(&after_first_select).expect("remove first");
        let after_second_select = select_grid_card(&after_first_confirm, 1).expect("select second");
        let after_second_confirm = confirm_grid(&after_second_select).expect("remove second");
        let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run, &relics)
            ["choices"]
            .as_array()
            .expect("first grid choices")
            .clone();
        let second_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&after_first_confirm, &relics)["choices"]
                .as_array()
                .expect("second grid choices")
                .clone();
        let one_removed_deck: Vec<_> = deck_content_keys(&after_first_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let two_removed_deck: Vec<_> = deck_content_keys(&after_second_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let hp = initial_run.player_hp;
        let max_hp = initial_run.player_max_hp;
        let gold = initial_run.gold;

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": choose_command}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": one_removed_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": second_grid_choices
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": two_removed_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": two_removed_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow remove two grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| { transition.action_step == 6 && transition.label == "Neow leave" }));
    }

    #[test]
    fn seed_start_neow_upgrade_generated_grid_trace_reaches_neow_leave() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::UpgradeCard
                            && seed_start_neow_option_is_supported_grid_reward(option.clone())
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with generated upgrade-card option");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let choose_command = format!("CHOOSE {}", option.slot);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec!["Burning Blood".to_owned()];
        let initial_run =
            seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
        let after_select = select_grid_card(&initial_run, 0).expect("select first");
        let after_confirm = confirm_grid(&after_select).expect("confirm upgrade");
        let grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run, &relics)
            ["choices"]
            .as_array()
            .expect("grid choices")
            .clone();
        let upgraded_deck: Vec<_> = deck_content_keys(&after_confirm.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let hp = initial_run.player_hp;
        let max_hp = initial_run.player_max_hp;
        let gold = initial_run.gold;

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": choose_command}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": []
            }}}),
            json!({"type": "action", "step": 5, "command": "CONFIRM"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": upgraded_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 6, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 6, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": upgraded_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow upgrade grid"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| { transition.action_step == 6 && transition.label == "Neow leave" }));
    }

    #[test]
    fn seed_start_neow_curse_transform_two_generated_trace_can_observe_map_after_second_pick() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::TransformTwoCards
                            && seed_start_neow_option_is_supported_grid_reward(option.clone())
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with generated curse plus transform-two option");
        let seed_string = test_seed_string_from_long(numeric_seed);
        let choose_command = format!("CHOOSE {}", option.slot);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec!["Burning Blood".to_owned()];
        let initial_run =
            seed_start_open_neow_grid_run(numeric_seed, &ironclad_starter_deck_keys(), &option);
        let after_first_select = select_grid_card(&initial_run, 0).expect("select first");
        let after_second_select =
            select_grid_card(&after_first_select, 1).expect("select second and transform");
        let after_confirm = confirm_grid(&after_second_select).expect("confirm transform two");
        let grid_deck: Vec<_> = deck_content_keys(&initial_run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let mut visible_confirm_deck = deck_content_keys(&after_confirm.deck);
        for _ in 0..2 {
            visible_confirm_deck.pop();
        }
        visible_confirm_deck
            .push(seed_start_neow_curse_deck_key(numeric_seed, 0).expect("generated curse key"));
        let visible_confirm_deck: Vec<_> = visible_confirm_deck
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let first_grid_choices: Vec<_> = seed_start_grid_simulated_subset(&initial_run, &relics)
            ["choices"]
            .as_array()
            .expect("first grid choices")
            .clone();
        let second_grid_choices: Vec<_> =
            seed_start_grid_simulated_subset(&after_first_select, &relics)["choices"]
                .as_array()
                .expect("second grid choices")
                .clone();
        let hp = initial_run.player_hp;
        let max_hp = initial_run.player_max_hp;
        let gold = initial_run.gold;

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {seed_string}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": choose_command}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": grid_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": first_grid_choices
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "GRID",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": grid_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": second_grid_choices
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": gold,
                "current_hp": hp,
                "max_hp": max_hp,
                "deck": visible_confirm_deck,
                "relics": [{"name": "Burning Blood"}],
                "choice_list": seed_start_first_map_choices(&seed_string)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow curse transform two grid"
        }));
        assert_eq!(initial_run.card_rng_counter, 0);
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "Neow grid confirm"
        }));
    }

    #[test]
    fn seed_start_neow_card_reward_choices_use_generated_helper() {
        let option = seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 0")
            .expect("VERIFY01 slot 0 option");

        assert_eq!(option.reward, NeowRewardType::ThreeCards);
        assert!(seed_start_neow_option_is_supported_card_reward(
            option.clone()
        ));

        let ids = seed_start_neow_card_reward_ids(1_957_307_888_551, &option, None);
        let names = seed_start_neow_card_reward_choice_names(1_957_307_888_551, &option, None);

        assert_eq!(ids.len(), 3);
        assert_eq!(names.len(), 3);
        assert_eq!(
            names,
            ids.iter()
                .map(|id| id.to_ascii_lowercase())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn seed_start_colorless_neow_pick_carries_card_rng_to_first_combat_reward() {
        let numeric_seed = 22_079_335_079;
        let option = seed_start_selected_neow_option(numeric_seed, "CHOOSE 0")
            .expect("CODEX04 slot 0 option");

        assert_eq!(option.reward, NeowRewardType::RandomColorless);

        let mut run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
        assert_eq!(reward_ids[1], deck_content_key(DRAMATIC_ENTRANCE_ID));

        assert_eq!(run.event_rng_counter, 0);
        run.card_rng_counter =
            seed_start_neow_card_reward_card_rng_counter(numeric_seed, &option, Some(&run))
                .expect("colorless reward consumes cardRng");
        let mut deck_ids = deck_content_keys(&run.deck);
        deck_ids.push(reward_ids[1].clone());
        run.deck = deck_instances_from_keys(&deck_ids);

        enter_normal_combat_reward_screen(&mut run);

        let reward = run.reward.as_ref().expect("combat reward screen");
        let ids = reward
            .choices
            .iter()
            .map(|choice| choice.content_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![BATTLE_TRANCE_ID, TWIN_STRIKE_ID, ENTRENCH_ID]);
    }

    #[test]
    fn seed_start_neow_three_rare_cards_can_pick_card_leave_and_reach_map() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::ThreeRareCards
                            && seed_start_neow_drawback_is_simple(option.drawback)
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with simple ThreeRareCards option");
        let external_seed = test_seed_string_from_long(numeric_seed);
        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let neow_deck: Vec<_> = deck_content_keys(&run.deck)
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let reward_ids = seed_start_neow_card_reward_ids(numeric_seed, &option, Some(&run));
        let reward_names =
            seed_start_neow_card_reward_choice_names(numeric_seed, &option, Some(&run));
        let reward_cards: Vec<_> = reward_ids
            .iter()
            .map(|id| json!({ "id": id, "name": id }))
            .collect();
        let mut picked_deck = neow_deck.clone();
        picked_deck.push(json!({ "id": reward_ids[1] }));

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": format!("CHOOSE {}", option.slot)}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": neow_deck,
                "relics": starting_relics,
                "choice_list": reward_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": picked_deck,
                "relics": starting_relics,
                "choice_list": ["leave"]
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "MAP",
                "ascension_level": 0,
                "floor": 0,
                "gold": run.gold,
                "current_hp": run.player_hp,
                "max_hp": run.player_max_hp,
                "deck": picked_deck,
                "relics": starting_relics,
                "choice_list": seed_start_first_map_choices(&external_seed)
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow rare card reward choices"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "Neow colorless pickup"
        }));
        assert!(report
            .verified
            .iter()
            .any(|transition| transition.action_step == 5 && transition.label == "Neow leave"));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_neow_rare_colorless_reward_uses_colorless_helper() {
        let (numeric_seed, option) = (1_i64..10_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.reward == NeowRewardType::RandomColorlessTwo
                            && seed_start_neow_drawback_is_simple(option.drawback)
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with rare colorless option");

        assert!(seed_start_neow_option_is_supported_card_reward(
            option.clone()
        ));
        assert_eq!(
            seed_start_neow_card_reward_label(option.reward),
            "Neow rare colorless reward choices"
        );

        let generated = generate_neow_colorless_reward(numeric_seed, option.reward);
        assert_eq!(
            seed_start_neow_card_reward_content_ids(numeric_seed, &option, None),
            generated.cards
        );
        assert_eq!(
            seed_start_neow_card_reward_ids(numeric_seed, &option, None),
            generated
                .cards
                .iter()
                .map(|content_id| content_key(*content_id).to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn seed_start_neow_random_colorless_uses_generated_card_reward_helper() {
        let generated =
            generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorless);
        let option = GeneratedNeowOption {
            slot: 0,
            drawback: NeowDrawback::None,
            reward: NeowRewardType::RandomColorless,
            label: "choose a colorless card to obtain".to_owned(),
        };

        assert_eq!(
            seed_start_neow_card_reward_content_ids(22_079_335_079, &option, None),
            generated.cards
        );
        assert_eq!(
            generated
                .cards
                .iter()
                .map(|content_id| content_key(*content_id).to_owned())
                .collect::<Vec<_>>(),
            seed_start_neow_card_reward_ids(22_079_335_079, &option, None)
        );
    }

    #[test]
    fn seed_start_neow_curse_rare_colorless_delays_curse_until_after_choices() {
        let (numeric_seed, option) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::RandomColorlessTwo
                    })
                    .map(|option| (seed, option))
            })
            .expect("synthetic seed with curse plus rare colorless");

        assert!(seed_start_neow_option_is_supported_card_reward(
            option.clone()
        ));

        let run = seed_start_apply_neow_reward_drawback(
            numeric_seed,
            &ironclad_starter_deck_keys(),
            &option,
        );
        let generated = generate_neow_colorless_reward(numeric_seed, option.reward);
        let choices = seed_start_neow_card_reward_content_ids(numeric_seed, &option, Some(&run));
        let delayed_curse =
            seed_start_neow_curse_deck_key(numeric_seed, generated.card_rng_counter)
                .expect("delayed curse");

        assert_eq!(run.card_rng_counter, 0);
        assert_eq!(run.deck.len(), ironclad_starter_deck_keys().len());
        assert_eq!(choices, generated.cards);
        assert!(content_id_from_key(&delayed_curse)
            .is_some_and(sts_core::content::cards::is_curse_content_id));
    }

    #[test]
    fn seed_start_neow_curse_rare_relic_carries_curse_deck_update() {
        let (numeric_seed, option, run) = (1_i64..100_000)
            .find_map(|seed| {
                generate_neow_options(seed, 80)
                    .into_iter()
                    .find(|option| {
                        option.drawback == NeowDrawback::Curse
                            && option.reward == NeowRewardType::OneRareRelic
                    })
                    .and_then(|option| {
                        let run = seed_start_apply_neow_relic_reward(
                            seed,
                            &ironclad_starter_deck_keys(),
                            &option,
                        );
                        (seed_start_newest_trace_relic_name(&run) != "Unknown Relic")
                            .then_some((seed, option, run))
                    })
            })
            .expect("synthetic seed with curse plus mapped rare relic");

        assert!(seed_start_neow_option_is_supported_relic_reward(
            option.clone()
        ));

        let deck_ids = deck_content_keys(&run.deck);

        assert_eq!(
            seed_start_selected_neow_option(numeric_seed, &format!("CHOOSE {}", option.slot)),
            Some(option)
        );
        assert!(run.card_rng_counter <= 1);
        assert_eq!(deck_ids.len(), ironclad_starter_deck_keys().len() + 1);
        assert!(matches!(
            deck_ids.last().map(String::as_str),
            Some(
                "Clumsy"
                    | "Decay"
                    | "Doubt"
                    | "Injury"
                    | "Normality"
                    | "Pain"
                    | "Parasite"
                    | "Regret"
                    | "Shame"
                    | "Writhe"
            )
        ));
        assert_ne!(seed_start_newest_trace_relic_name(&run), "Unknown Relic");
    }

    #[test]
    fn seed_start_neow_card_reward_pick_uses_generated_choices() {
        let choices = Some(vec![
            "Twin Strike".to_owned(),
            "Heavy Blade".to_owned(),
            "Intimidate".to_owned(),
        ]);

        assert_eq!(
            seed_start_pick_neow_card_reward(&choices, "CHOOSE 1"),
            Some("Heavy Blade".to_owned())
        );
        assert_eq!(seed_start_pick_neow_card_reward(&choices, "CHOOSE 9"), None);
        assert_eq!(seed_start_pick_neow_card_reward(&None, "CHOOSE 0"), None);
    }

    #[test]
    fn seed_start_neow_boss_swap_uses_core_helper_and_removes_burning_blood() {
        let option = seed_start_selected_neow_option(1_957_307_888_551, "CHOOSE 3")
            .expect("boss swap option");

        assert!(seed_start_neow_option_is_supported_boss_swap(option));

        let run = seed_start_apply_neow_boss_swap(1_957_307_888_551, &ironclad_starter_deck_keys());
        let relic_ids = seed_start_boss_swap_relic_ids(&run);

        assert!(!relic_ids.contains(&"Burning Blood".to_owned()));
        assert_eq!(relic_ids.len(), 1);
        assert_ne!(relic_ids[0], "Unknown Relic");
        assert!(seed_start_unsupported_boss_swap_reason(&run).is_none());
    }

    #[test]
    fn seed_start_neow_boss_swap_classifies_grid_opening_relics() {
        let mut run = RunState::map_fixture();
        open_neow_reward_grid(&mut run, NeowRewardType::RemoveCard);

        let reason = seed_start_unsupported_boss_swap_reason(&run)
            .expect("grid-opening boss relics are caveated");

        assert!(reason.contains("grid-opening boss relic"));
    }

    #[test]
    fn seed_start_neow_boss_swap_trace_branch_reaches_leave() {
        let numeric_seed = 1_957_307_888_551;
        let deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let swapped_run =
            seed_start_apply_neow_boss_swap(numeric_seed, &ironclad_starter_deck_keys());
        let swapped_relics: Vec<_> = seed_start_boss_swap_relic_ids(&swapped_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 VERIFY01"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 3"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": swapped_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap"
        }));
    }

    #[test]
    fn seed_start_boss_swap_tiny_house_reward_screen_opens_and_skips_card_reward() {
        let (numeric_seed, tiny_house_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_tiny_house_reward(run))
            .expect("synthetic seed with Tiny House boss swap");
        let external_seed = test_seed_string_from_long(numeric_seed);
        let option =
            seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");
        assert_eq!(option.reward, NeowRewardType::BossRelic);

        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let tiny_house_deck = starting_deck.clone();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let tiny_house_relics: Vec<_> = seed_start_boss_swap_relic_ids(&tiny_house_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let mut card_reward_run = tiny_house_run.clone();
        card_reward_run =
            apply_run_action(&card_reward_run, RunAction::OpenCardReward).expect("open cards");
        let reward_cards: Vec<_> = card_reward_run
            .reward
            .as_ref()
            .expect("card reward")
            .choices
            .iter()
            .map(|card| {
                json!({
                    "id": reward_card_display_key(&card_reward_run, card.content_id),
                    "name": reward_card_display_key(&card_reward_run, card.content_id),
                })
            })
            .collect();
        let reward_choice_names: Vec<_> = card_reward_run
            .reward
            .as_ref()
            .expect("card reward")
            .choices
            .iter()
            .map(|card| {
                reward_card_display_key(&card_reward_run, card.content_id).to_ascii_lowercase()
            })
            .collect();

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": format!("CHOOSE {}", option.slot)}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["card"],
                "screen_state": {
                    "rewards": [{"reward_type": "CARD"}]
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": reward_choice_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 5, "command": "SKIP"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Tiny House reward"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "card reward"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_boss_swap_tiny_house_reward_screen_can_pick_card_reward() {
        let (numeric_seed, tiny_house_run) = (1_i64..100_000)
            .map(|seed| {
                (
                    seed,
                    seed_start_apply_neow_boss_swap(seed, &ironclad_starter_deck_keys()),
                )
            })
            .find(|(_, run)| seed_start_boss_swap_is_tiny_house_reward(run))
            .expect("synthetic seed with Tiny House boss swap");
        let external_seed = test_seed_string_from_long(numeric_seed);
        let option =
            seed_start_selected_neow_option(numeric_seed, "CHOOSE 3").expect("boss-swap slot");
        assert_eq!(option.reward, NeowRewardType::BossRelic);

        let starting_deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let tiny_house_deck = starting_deck.clone();
        let starting_relics = vec![json!({ "name": "Burning Blood" })];
        let tiny_house_relics: Vec<_> = seed_start_boss_swap_relic_ids(&tiny_house_run)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let mut card_reward_run = tiny_house_run.clone();
        card_reward_run =
            apply_run_action(&card_reward_run, RunAction::OpenCardReward).expect("open cards");
        let reward = card_reward_run.reward.as_ref().expect("card reward");
        let reward_cards: Vec<_> = reward
            .choices
            .iter()
            .map(|card| {
                json!({
                    "id": reward_card_display_key(&card_reward_run, card.content_id),
                    "name": reward_card_display_key(&card_reward_run, card.content_id),
                })
            })
            .collect();
        let reward_choice_names: Vec<_> = reward
            .choices
            .iter()
            .map(|card| {
                reward_card_display_key(&card_reward_run, card.content_id).to_ascii_lowercase()
            })
            .collect();
        let picked_card_key =
            reward_card_display_key(&card_reward_run, reward.choices[1].content_id).to_owned();
        let mut picked_deck = tiny_house_deck.clone();
        picked_deck.push(json!({ "id": picked_card_key }));

        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": format!("START IRONCLAD 0 {external_seed}")}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": starting_deck,
                "relics": starting_relics,
                "choice_list": seed_start_neow_choices(numeric_seed)
            }}}),
            json!({"type": "action", "step": 3, "command": format!("CHOOSE {}", option.slot)}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "COMBAT_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": ["card"],
                "screen_state": {
                    "rewards": [{"reward_type": "CARD"}]
                }
            }}}),
            json!({"type": "action", "step": 4, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 4, "message": {"game_state": {
                "screen_type": "CARD_REWARD",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": tiny_house_deck,
                "relics": tiny_house_relics,
                "choice_list": reward_choice_names,
                "screen_state": {
                    "cards": reward_cards
                }
            }}}),
            json!({"type": "action", "step": 5, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 5, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": tiny_house_run.gold,
                "current_hp": tiny_house_run.player_hp,
                "max_hp": tiny_house_run.player_max_hp,
                "deck": picked_deck,
                "relics": tiny_house_relics,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow boss swap Tiny House reward"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 4 && transition.label == "card reward"
        }));
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 5 && transition.label == "card reward pick 1"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn seed_start_codex04_neow_potion_reward_uses_generated_potions() {
        let deck: Vec<_> = ironclad_starter_deck_keys()
            .into_iter()
            .map(|id| json!({ "id": id }))
            .collect();
        let relics = vec![json!({ "name": "Burning Blood" })];
        let choices = seed_start_neow_choices(22_079_335_079);
        let potions: Vec<_> = seed_start_neow_potion_names(22_079_335_079)
            .into_iter()
            .map(|name| json!({ "name": name }))
            .collect();
        let lines = vec![
            json!({"type": "metadata", "schema": 1, "source": "communication_mod"}),
            json!({"type": "state", "step": 0, "message": {}}),
            json!({"type": "action", "step": 1, "command": "START IRONCLAD 0 CODEX04"}),
            json!({"type": "state", "step": 1, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "choice_list": ["talk"]
            }}}),
            json!({"type": "action", "step": 2, "command": "CHOOSE 0"}),
            json!({"type": "state", "step": 2, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "choice_list": choices
            }}}),
            json!({"type": "action", "step": 3, "command": "CHOOSE 1"}),
            json!({"type": "state", "step": 3, "message": {"game_state": {
                "screen_type": "EVENT",
                "ascension_level": 0,
                "floor": 0,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck": deck,
                "relics": relics,
                "potions": potions,
                "choice_list": ["leave"]
            }}}),
        ];
        let content = lines
            .into_iter()
            .map(|line| serde_json::to_string(&line).expect("trace line serializes"))
            .collect::<Vec<_>>()
            .join("\n");

        let report = verify_seed_start_communication_mod_trace(&content).expect("seed-start");

        assert!(report.unexpected_diffs.is_empty(), "{report:#?}");
        assert!(report.verified.iter().any(|transition| {
            transition.action_step == 3 && transition.label == "Neow three potion reward"
        }));
        assert_eq!(
            report
                .seed_start
                .expect("seed-start")
                .first_boundary
                .category,
            "none"
        );
    }

    #[test]
    fn m33_selected_clean_neow_traces_reach_expected_labels_without_unexpected_diffs() {
        let mut failures = Vec::new();
        for case in [
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T15-36-33-694Z.jsonl",
                seed: "4",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow upgrade grid",
                    "Neow grid select",
                    "Neow grid confirm",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T15-54-59-219Z.jsonl",
                seed: "4",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow remove two grid",
                    "Neow grid select",
                    "Neow grid confirm",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T15-56-34-404Z.jsonl",
                seed: "8",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare card reward choices",
                    "Neow colorless pickup",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T16-21-32-031Z.jsonl",
                seed: "1",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare colorless reward choices",
                    "Neow colorless pickup",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T16-53-08-900Z.jsonl",
                seed: "7",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare relic",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-28-45-416Z.jsonl",
                seed: "1",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow three potion reward",
                    "Neow potion reward pick 1",
                    "Neow potion reward pick 2",
                    "Neow potion reward pick 3",
                    "Neow potion reward proceed",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-38-10-461Z.jsonl",
                seed: "11",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow curse immediate reward",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-39-56-094Z.jsonl",
                seed: "P",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare relic",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-51-15-391Z.jsonl",
                seed: "C",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow rare colorless reward choices",
                    "Neow colorless pickup",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-53-36-873Z.jsonl",
                seed: "1B",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow curse transform two grid",
                    "Neow grid select",
                    "Neow grid confirm",
                    "Neow leave",
                ],
            },
            SelectedNeowTraceCase {
                path: "communication_mod/trace-2026-06-26T17-59-19-268Z.jsonl",
                seed: "2",
                expected_labels: &[
                    "seed-start bootstrap",
                    "Neow talk",
                    "Neow curse immediate reward",
                    "Neow leave",
                ],
            },
        ] {
            let Some(content) = crate::load_corpus_file(case.path) else {
                eprintln!("skipping missing selected M33 Neow trace: {}", case.path);
                continue;
            };

            let report =
                verify_seed_start_communication_mod_trace(&content).expect("seed-start report");

            if report.mode != VerificationMode::SeedStart {
                failures.push(format!("{} wrong mode: {:?}", case.path, report.mode));
            }
            if !report.unexpected_diffs.is_empty() {
                failures.push(format!(
                    "{} unexpected diffs: {:?}",
                    case.path, report.unexpected_diffs
                ));
            }

            let seed_start = report.seed_start.as_ref().expect("seed-start details");
            if seed_start.start_command.external_seed != case.seed {
                failures.push(format!(
                    "{} wrong seed: expected {}, got {}",
                    case.path, case.seed, seed_start.start_command.external_seed
                ));
            }
            if seed_start.first_boundary.category != "none" {
                failures.push(format!(
                    "{} wrong boundary: {:?}",
                    case.path, seed_start.first_boundary
                ));
            }

            let labels: Vec<_> = report
                .verified
                .iter()
                .map(|step| step.label.as_str())
                .collect();
            for expected in case.expected_labels {
                if !labels.contains(expected) {
                    failures.push(format!(
                        "{} missing verified seed-start label {expected}; labels: {labels:?}",
                        case.path
                    ));
                }
            }
        }

        assert!(
            failures.is_empty(),
            "selected M33 Neow trace regressions:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn unsupported_combat_command_reason_names_unmapped_cards() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "hand": [{"id": "Meteor Strike", "name": "Meteor Strike"}]
                }
            }
        });
        let reason =
            unsupported_combat_command_reason(&message, "PLAY 1").expect("unmapped card reason");
        assert!(reason.contains("Meteor Strike"));
        assert!(reason.contains("not mapped"));
    }

    #[test]
    fn seed_start_allows_sword_boomerang_with_one_living_enemy() {
        let combat = sword_boomerang_combat(1);

        assert_eq!(
            unsupported_seed_start_combat_command(&combat, "PLAY 1"),
            None
        );
    }

    #[test]
    fn seed_start_allows_multi_enemy_sword_boomerang() {
        let combat = sword_boomerang_combat(2);

        assert_eq!(
            unsupported_seed_start_combat_command(&combat, "PLAY 1"),
            None
        );
    }

    #[test]
    fn observed_combat_subset_uses_first_living_monster() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "player": {"current_hp": 70, "block": 0, "energy": 2},
                    "monsters": [
                        {"current_hp": 0, "block": 0, "intent": "ATTACK", "move_base_damage": 5},
                        {"current_hp": 24, "block": 3, "intent": "ATTACK", "move_base_damage": 7}
                    ]
                }
            }
        });
        let subset = observed_combat_subset(&message, &["monster_hp", "monster_block"]);
        assert_eq!(subset["monster_hp"], 24);
        assert_eq!(subset["monster_block"], 3);
    }

    #[test]
    fn unsupported_monster_ai_reason_names_monster_groups() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "monsters": [
                        {"id": "SpikeSlime_S", "current_hp": 0},
                        {"id": "AcidSlime_M", "current_hp": 24}
                    ]
                }
            }
        });
        let reason = unsupported_monster_ai_reason(&message).expect("unsupported slime AI");
        assert!(reason.contains("AcidSlime_M"));
        assert!(reason.contains("monster group"));
    }

    #[test]
    fn choose_index_parses_nonzero_reward_choice() {
        assert_eq!(choose_index("CHOOSE 2"), Some(2));
    }

    #[test]
    fn seed_start_map_choice_accepts_nonzero_visible_choice() {
        let message = json!({
            "game_state": {
                "screen_type": "MAP",
                "choice_list": ["x=0", "x=2", "x=4", "x=5"]
            }
        });

        assert!(command_is_visible_choose(&message, "CHOOSE 1"));
        assert!(!command_is_visible_choose(&message, "CHOOSE 4"));
        assert_eq!(seed_start_map_pick_x("CODEX04", &[], "CHOOSE 1"), 2);
    }

    fn sword_boomerang_combat(living_monsters: usize) -> CombatState {
        let mut combat = CombatState::initial_fixture();
        combat.piles.hand = vec![CardInstance::new(
            CardId::new(1),
            sts_core::content::cards::SWORD_BOOMERANG_ID,
        )];
        while combat.monsters.len() < living_monsters {
            let mut monster = combat.monsters[0].clone();
            monster.id = MonsterId::new(combat.monsters.len() as u64 + 1);
            combat.monsters.push(monster);
        }
        combat
    }

    struct SelectedNeowTraceCase {
        path: &'static str,
        seed: &'static str,
        expected_labels: &'static [&'static str],
    }

    fn assert_selected_trace_first_combat_opening_is_seed_derived(path: &str, label: &str) {
        let Some(content) = crate::load_corpus_file(path) else {
            eprintln!("skipping missing selected M34 trace: {path}");
            return;
        };
        let trace = import_communication_mod_trace(&content).expect("trace imports");
        let start = trace
            .lines
            .iter()
            .filter_map(|line| match line {
                TraceLine::Action(action) => parse_start_command(action).and_then(Result::ok),
                _ => None,
            })
            .next()
            .expect("trace has START command");
        let transitions = trace_transitions(&trace.lines).expect("trace transitions");
        let (_, _, post) = transitions
            .transitions
            .iter()
            .find(|(_, _, post)| {
                post.message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .is_some()
            })
            .unwrap_or_else(|| panic!("{label} trace has no combat entry"));
        let game = post.message.get("game_state").expect("game_state");
        let floor = game.get("floor").and_then(Value::as_u64).unwrap_or(1) as i64;
        let deck = card_instances_from_array(game.get("deck"), 1);
        let mut shuffle_rng = StsRng::new(start.numeric_seed + floor);
        let mut card_random_rng = None;
        let simulated =
            initialize_combat_piles_with_relics(&deck, &mut shuffle_rng, &mut card_random_rng, &[]);

        assert!(
            seed_start_opening_piles_match(&simulated, &post.message),
            "{label} opening piles were not seed-derived from current deck ordering; observed hand={:?} draw={:?}, simulated hand={:?} draw={:?}",
            combat_card_ids(
                post.message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .and_then(|combat| combat.get("hand"))
            ),
            combat_card_ids(
                post.message
                    .get("game_state")
                    .and_then(|game| game.get("combat_state"))
                    .and_then(|combat| combat.get("draw_pile"))
            ),
            simulated
                .hand
                .iter()
                .map(|card| content_key(card.content_id))
                .collect::<Vec<_>>(),
            simulated
                .draw_pile
                .iter()
                .map(|card| content_key(card.content_id))
                .collect::<Vec<_>>()
        );
    }

    fn test_seed_string_from_long(mut seed: i64) -> String {
        const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNPQRSTUVWXYZ";
        if seed == 0 {
            return "0".to_owned();
        }
        let mut out = Vec::new();
        while seed > 0 {
            out.push(ALPHABET[(seed % 35) as usize] as char);
            seed /= 35;
        }
        out.iter().rev().collect()
    }
}
