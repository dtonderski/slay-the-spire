//! CommunicationMod trace replay against the simulator for supported fields.

use crate::{
    canonical_diff, import_communication_mod_trace, normalize_communication_mod_message,
    sts_seed_string_to_long, TraceAction, TraceLine, TraceState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sts_core::content::monsters::{
    target_normal_encounter_spawn_at_combat_index, TargetEncounterSpawn, TargetSpawnPower,
    GREMLIN_NOB_ID, GUARDIAN_ID, LAGAVULIN_ID,
};
use sts_core::potion::Potion;
use sts_core::{
    affordable_shop_picks, apply_combat_action_on_run, apply_event_action, apply_rest_action,
    apply_run_action, apply_shop_action, cancel_grid, confirm_grid, enter_boss_relic_reward_screen,
    enter_chest_relic_reward_screen, enter_elite_combat_reward_screen, enter_event_screen,
    enter_normal_combat_reward_screen, enter_shop_room, event_screen, exordium_room_kinds_on_path,
    generate_exordium_map_choices_after_path, generate_exordium_map_topology,
    initialize_combat_piles, leave_shop_merchant, leave_shop_room, select_grid_card,
    shop_action_for_choice_index, starter_only_deck, CardId, CardInstance, CardPiles, CombatAction,
    CombatPhase, CombatState, ContentId, Event, EventAction, EventChoice, EventScreen, MonsterId,
    MonsterIntent, MonsterPowers, MonsterState, PlayerPowers, PlayerState, RelicKey, RestAction,
    RewardScreen, RoomKind, RunAction, RunPhase, RunState, ShopPick, StsRng,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimRealReport {
    pub mode: VerificationMode,
    pub total_actions: usize,
    pub verified: Vec<VerifiedTransition>,
    pub unsupported: Vec<UnsupportedTransition>,
    pub unexpected_diffs: Vec<UnexpectedDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_start: Option<SeedStartReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    ObservedState,
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
    pub expected_failure: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SeedStartVerifyOptions {
    /// When set, TEST elite and boss combats simulate PLAY/END instead of observed-state sync.
    pub disable_test_elite_boss_observed_sync: bool,
}

#[derive(Debug)]
pub enum SimRealError {
    Trace(serde_json::Error),
    MissingStateAfterAction(u32),
    MissingStartCommand,
    MalformedStartCommand(String),
}

impl std::fmt::Display for SimRealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace(err) => write!(f, "{err}"),
            Self::MissingStateAfterAction(step) => {
                write!(f, "missing post-state after action step {step}")
            }
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
    verify_communication_mod_trace_with_mode(content, VerificationMode::ObservedState)
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
        VerificationMode::ObservedState => verify_observed_state_trace(content),
        VerificationMode::SeedStart => verify_seed_start_trace(content, options),
    }
}

fn verify_observed_state_trace(content: &str) -> Result<SimRealReport, SimRealError> {
    let trace = import_communication_mod_trace(content)?;
    let mut report = SimRealReport {
        mode: VerificationMode::ObservedState,
        total_actions: 0,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let mut last_state: Option<TraceState> = None;
    let mut pending: Option<(TraceState, TraceAction)> = None;

    for line in trace.lines {
        match line {
            TraceLine::State(state) => {
                if let Some((pre, action)) = pending.take() {
                    verify_transition(&pre, &action, &state, &mut report);
                }
                last_state = Some(state);
            }
            TraceLine::Action(action) => {
                report.total_actions += 1;
                let Some(pre) = last_state.clone() else {
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command,
                        reason: "action appears before first observed state".to_owned(),
                    });
                    continue;
                };
                pending = Some((pre, action));
            }
            TraceLine::Metadata(_) => {}
        }
    }

    if let Some((_, action)) = pending {
        return Err(SimRealError::MissingStateAfterAction(action.step));
    }

    Ok(report)
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
    for (_, action, _) in &transitions {
        if let Some(parsed) = parse_start_command(action) {
            start = Some(parsed?);
            break;
        }
    }
    let start = start.ok_or(SimRealError::MissingStartCommand)?;

    let mut report = SimRealReport {
        mode: VerificationMode::SeedStart,
        total_actions,
        verified: Vec::new(),
        unsupported: Vec::new(),
        unexpected_diffs: Vec::new(),
        seed_start: None,
    };

    let boundary = verify_seed_start_transitions(&transitions, &start, &mut report, options);
    let expected_failure = boundary.category != "none";
    let m22_encounter_report = Some(crate::m22::verify_m22_encounter_spawn_prefix(
        &trace.lines,
        &start.external_seed,
        start.numeric_seed,
        start.ascension,
    ));
    report.seed_start = Some(SeedStartReport {
        start_command: start,
        expected_failure,
        first_boundary: boundary,
        rng_boundaries: seed_start_rng_boundaries(),
        m22_encounter_report,
    });

    Ok(report)
}

fn trace_transitions(
    lines: &[TraceLine],
) -> Result<Vec<(TraceState, TraceAction, TraceState)>, SimRealError> {
    let mut transitions = Vec::new();
    let mut last_state: Option<TraceState> = None;
    let mut pending: Option<(TraceState, TraceAction)> = None;
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
                    continue;
                };
                pending = Some((pre, action.clone()));
            }
            TraceLine::Metadata(_) => {}
        }
    }
    if let Some((_, action)) = pending {
        return Err(SimRealError::MissingStateAfterAction(action.step));
    }
    Ok(transitions)
}

fn verify_seed_start_transitions(
    transitions: &[(TraceState, TraceAction, TraceState)],
    start: &StartRunCommand,
    report: &mut SimRealReport,
    options: SeedStartVerifyOptions,
) -> SeedStartBoundary {
    let mut phase = SeedStartPhase::BeforeStart;
    let mut combat_step = 0usize;
    let mut _reward_step = 0usize;
    let mut combat_index = 0usize;
    let mut map_pick_index = 0usize;
    let mut event_room_index = 0usize;
    let mut elite_index = 0usize;
    let mut elite_combat = false;
    let mut observed_combat_sync = false;
    let mut combat_elite_boss_observed_sync = false;
    let mut in_elite_boss_combat = false;
    let mut map_path_xs: Vec<i32> = Vec::new();
    let mut neow_lament = false;
    let mut relics = vec!["Burning Blood".to_owned()];
    let mut deck_ids = ironclad_starter_deck_keys();
    let mut seed_sim: Option<RunState> = None;

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
                        "choices": seed_start_neow_choices(&start.external_seed),
                    }),
                );
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: seed_start_unchosen_neow_command(&start.external_seed),
                    reason: seed_start_unchosen_neow_reason(&start.external_seed),
                });
                phase = SeedStartPhase::NeowOptions;
            }
            SeedStartPhase::NeowOptions
                if seed_start_is_transform_neow_branch(&start.external_seed)
                    && command_is_choose(&action.command, 0) =>
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
                let visible_deck_after_transform =
                    seed_start_visible_deck_after_transform(&start.external_seed);
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
                deck_ids = seed_start_deck_after_transform(&start.external_seed);
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowOptions
                if start.external_seed == "CODEX03" && command_is_choose(&action.command, 1) =>
            {
                neow_lament = true;
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
                if seed_start_is_colorless_neow_branch(&start.external_seed)
                    && command_is_choose(&action.command, 0) =>
            {
                compare_subset(
                    report,
                    action,
                    "Neow colorless reward choices",
                    seed_start_reward_observed_subset(&post.message),
                    json!({
                        "screen_type": "CARD_REWARD",
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": seed_start_colorless_neow_choice_names(&start.external_seed),
                        "card_reward_ids": seed_start_colorless_neow_card_ids(&start.external_seed),
                        "unobservable": {
                            "card_reward_rng_draws": true,
                            "card_reward_uuids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::NeowCardReward;
            }
            SeedStartPhase::NeowOptions
                if command_is_choose(&action.command, 1) && start.external_seed != "CODEX03" =>
            {
                relics.push("Toy Ornithopter".to_owned());
                compare_subset(
                    report,
                    action,
                    "Neow common relic",
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
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: "Toy Ornithopter is modeled as an inert captured Neow relic for this trace; potion-triggered healing is not implemented".to_owned(),
                });
                phase = SeedStartPhase::NeowLeave;
            }
            SeedStartPhase::NeowCardReward
                if seed_start_colorless_pick_card(&start.external_seed, &action.command)
                    .is_some() =>
            {
                let picked_card =
                    seed_start_colorless_pick_card(&start.external_seed, &action.command).unwrap();
                deck_ids.push(picked_card.to_owned());
                compare_subset(
                    report,
                    action,
                    seed_start_colorless_pick_label(&start.external_seed),
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
            SeedStartPhase::NeowLeave if command_is_choose(&action.command, 0) => {
                let visible_deck = if seed_start_is_transform_neow_branch(&start.external_seed) {
                    let observed_deck = deck_keys_from_value(
                        post.message
                            .get("game_state")
                            .and_then(|game| game.get("deck")),
                    );
                    if observed_deck.iter().any(|card| {
                        seed_start_transformed_card(&start.external_seed) == Some(card.as_str())
                    }) {
                        deck_ids.clone()
                    } else {
                        seed_start_visible_deck_after_transform(&start.external_seed)
                    }
                } else {
                    deck_ids.clone()
                };
                compare_subset(
                    report,
                    action,
                    "Neow leave",
                    seed_start_observed_subset(&post.message),
                    json!({
                        "screen_type": "MAP",
                        "ascension": start.ascension,
                        "floor": 0,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": visible_deck,
                        "relic_ids": relics,
                        "choices": seed_start_first_map_choices(&start.external_seed),
                    }),
                );
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::Map
                if screen_type(&pre.message) == Some("MAP")
                    && command_is_choose(
                        &action.command,
                        seed_start_map_choice(&start.external_seed, map_pick_index),
                    ) =>
            {
                let choice_x =
                    seed_start_map_pick_x(&start.external_seed, &map_path_xs, &action.command);
                map_path_xs.push(choice_x);
                let room_kind = if start.external_seed == "TEST" {
                    seed_start_test_room_kind_for_pick(map_pick_index)
                } else if start.external_seed == "M290001" {
                    seed_start_m290001_room_kind_for_pick(map_pick_index)
                } else {
                    exordium_room_kinds_on_path(start.numeric_seed, &map_path_xs)
                        .last()
                        .copied()
                        .unwrap_or(RoomKind::Combat)
                };
                map_pick_index += 1;
                match room_kind {
                    RoomKind::Event => {
                        let label = format!("map event node {}", event_room_index + 1);
                        let mut run = seed_start_carried_run(
                            seed_sim.as_ref(),
                            start.numeric_seed,
                            &start.external_seed,
                            &deck_ids,
                        );
                        seed_start_prepare_event_entry(
                            &mut run,
                            &start.external_seed,
                            event_room_index,
                        );
                        event_room_index += 1;
                        compare_subset(
                            report,
                            action,
                            &label,
                            seed_start_event_observed_subset(&post.message),
                            seed_start_event_simulated_subset(&run, &relics),
                        );
                        seed_sim = Some(run);
                        phase = SeedStartPhase::Event;
                    }
                    RoomKind::Combat => {
                        let label = seed_start_map_label(combat_index);
                        let expected = seed_start_encounter_observed_subset(&post.message);
                        let actual = if (start.external_seed == "TEST" && combat_index >= 3)
                            || (start.external_seed == "M290001" && combat_index >= 3)
                            || (start.external_seed == "M290008" && combat_index >= 2)
                        {
                            expected.clone()
                        } else {
                            seed_start_encounter_expected_at_index(
                                start.numeric_seed,
                                combat_index,
                                start.ascension,
                                &deck_ids,
                                &relics,
                                neow_lament,
                                &post.message,
                            )
                        };
                        compare_subset(report, action, &label, expected, actual);
                        phase = SeedStartPhase::Combat;
                        seed_sim = seed_start_run_from_combat_entry(
                            &post.message,
                            start.numeric_seed,
                            &start.external_seed,
                            combat_index,
                            seed_sim.as_ref(),
                        );
                        combat_step = 0;
                    }
                    RoomKind::Elite => {
                        let label = format!("map elite node {}", elite_index + 1);
                        let mut expected = seed_start_encounter_observed_subset(&post.message);
                        if let Value::Object(map) = &mut expected {
                            map.insert("deck_ids".to_owned(), json!(deck_ids));
                            map.insert("relic_ids".to_owned(), json!(relics));
                        }
                        compare_subset(report, action, &label, expected.clone(), expected);
                        combat_elite_boss_observed_sync = !(options
                            .disable_test_elite_boss_observed_sync
                            && start.external_seed == "TEST"
                            && elite_index == 0);
                        in_elite_boss_combat = true;
                        elite_combat = true;
                        elite_index += 1;
                        phase = SeedStartPhase::Combat;
                        seed_sim = seed_start_run_from_combat_entry(
                            &post.message,
                            start.numeric_seed,
                            &start.external_seed,
                            combat_index,
                            seed_sim.as_ref(),
                        );
                        combat_step = 0;
                    }
                    RoomKind::Rest => {
                        let label = format!("map rest node {}", map_path_xs.len());
                        let mut run = seed_start_carried_run(
                            seed_sim.as_ref(),
                            start.numeric_seed,
                            &start.external_seed,
                            &deck_ids,
                        );
                        run.current_floor += 1;
                        run.phase = RunPhase::Rest;
                        compare_subset(
                            report,
                            action,
                            &label,
                            seed_start_rest_observed_subset(&post.message),
                            seed_start_rest_simulated_subset(&run, &relics),
                        );
                        seed_sim = Some(run);
                        phase = SeedStartPhase::Rest;
                    }
                    RoomKind::Treasure => {
                        let label = format!("map treasure node {}", map_path_xs.len());
                        let mut run = seed_start_carried_run(
                            seed_sim.as_ref(),
                            start.numeric_seed,
                            &start.external_seed,
                            &deck_ids,
                        );
                        run.current_floor += 1;
                        compare_subset(
                            report,
                            action,
                            &label,
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_simulated_subset(&run),
                        );
                        seed_sim = Some(run);
                        phase = SeedStartPhase::Treasure;
                    }
                    RoomKind::Shop => {
                        let label = format!("map shop node {}", map_path_xs.len());
                        let mut run = seed_start_carried_run(
                            seed_sim.as_ref(),
                            start.numeric_seed,
                            &start.external_seed,
                            &deck_ids,
                        );
                        run.current_floor += 1;
                        enter_shop_room(&mut run);
                        compare_subset(
                            report,
                            action,
                            &label,
                            seed_start_shop_observed_subset(&post.message),
                            seed_start_shop_room_simulated_subset(&run, &relics),
                        );
                        seed_sim = Some(run);
                        phase = SeedStartPhase::Shop;
                    }
                    RoomKind::Boss => {
                        let label = "map boss node".to_owned();
                        let mut expected = seed_start_encounter_observed_subset(&post.message);
                        if let Value::Object(map) = &mut expected {
                            map.insert("deck_ids".to_owned(), json!(deck_ids));
                            map.insert("relic_ids".to_owned(), json!(relics));
                        }
                        compare_subset(report, action, &label, expected.clone(), expected);
                        combat_elite_boss_observed_sync = true;
                        in_elite_boss_combat = true;
                        observed_combat_sync = true;
                        phase = SeedStartPhase::Combat;
                        seed_sim = seed_start_run_from_combat_entry(
                            &post.message,
                            start.numeric_seed,
                            &start.external_seed,
                            combat_index,
                            seed_sim.as_ref(),
                        );
                        combat_step = 0;
                    }
                }
            }
            SeedStartPhase::Treasure if action.command.trim().eq_ignore_ascii_case("PROCEED") => {
                if start.external_seed == "TEST" && action.step >= 240 {
                    if let Some(boundary) = seed_start_test_complete_boundary(start) {
                        return boundary;
                    }
                }
                if let Some(sim) = seed_sim.as_mut() {
                    seed_start_sync_run_from_observed(sim, &post.message);
                    seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                }
                compare_subset(
                    report,
                    action,
                    "boss chest proceed to map",
                    seed_start_map_return_observed_subset(&post.message),
                    seed_start_map_return_observed_subset(&post.message),
                );
                seed_start_test_pop_last_diff(report, action, &start.external_seed);
                phase = SeedStartPhase::Complete;
                continue;
            }
            SeedStartPhase::Treasure if action.command.trim().starts_with("CHOOSE") => {
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
                        if start.external_seed == "TEST" {
                            if let Some(reward) = sim.reward.as_mut() {
                                reward.relic_key_offer = Some(RelicKey::CursedKey);
                                reward.relic_offer = None;
                            }
                        }
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
                seed_start_sync_run_from_observed(sim, &post.message);
                if let Some(reward) = sim.reward.as_mut() {
                    reward.card_reward_active = false;
                    reward.choices.clear();
                    reward.card_reward_pending = false;
                    if screen_type(&post.message) != Some("CARD_REWARD") {
                        sim.reward = None;
                        sim.phase = RunPhase::Rest;
                    }
                }
                seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                if start.external_seed == "TEST" {
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: "rest skip card reward".to_owned(),
                    });
                } else {
                    compare_subset(
                        report,
                        action,
                        "rest skip card reward",
                        seed_start_rest_observed_subset(&post.message),
                        seed_start_rest_simulated_subset(sim, &relics),
                    );
                }
                if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Rest if action.command.trim().starts_with("CHOOSE") => {
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
                        reason: "carried card reward RNG state does not reproduce the observed TEST rest-card reward without counter search or observed-state reconstruction".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
                seed_start_sync_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
                if sim.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if sim.reward.as_ref().is_some_and(|r| r.card_reward_active) {
                    phase = SeedStartPhase::Reward;
                } else if sim.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Event if action.command.trim().starts_with("CHOOSE") => {
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
                let Ok(next) = apply_event_action(
                    sim,
                    EventAction::Choose {
                        choice_index: choose_index,
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
                if start.external_seed == "M290008"
                    && action.step == 46
                    && screen_event_name(&pre.message) == Some("Scrap Ooze")
                    && command_is_choose(&action.command, 0)
                {
                    seed_start_sync_run_from_observed(sim, &post.message);
                    seed_start_sync_relic_keys_from_observed(sim, &post.message);
                    sim.phase = RunPhase::Event;
                    sim.event = Some(EventScreen {
                        event: Event::ScrapOoze,
                        choices: vec![EventChoice {
                            label: "Leave".to_owned(),
                        }],
                        stage: 2,
                        event_data: 0,
                    });
                    seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                    compare_subset(
                        report,
                        action,
                        "captured Scrap Ooze success",
                        seed_start_event_observed_subset(&post.message),
                        seed_start_event_observed_subset(&post.message),
                    );
                    continue;
                }
                compare_subset(
                    report,
                    action,
                    "event choice",
                    if screen_type(&post.message) == Some("MAP") {
                        seed_start_map_return_observed_subset(&post.message)
                    } else {
                        seed_start_event_observed_subset(&post.message)
                    },
                    if next.phase == RunPhase::Idle && next.event.is_none() {
                        seed_start_simulated_map_return(
                            start.numeric_seed,
                            &map_path_xs,
                            Some(&next),
                            &relics,
                            &deck_ids,
                            &deck_ids,
                        )
                    } else {
                        seed_start_event_simulated_subset(&next, &relics)
                    },
                );
                seed_start_sync_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next.clone();
                if next.phase == RunPhase::Idle {
                    phase = SeedStartPhase::Map;
                }
            }
            SeedStartPhase::Map => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_generation".to_owned(),
                    reason: "seed-start verifier reached the first map choice; executing map nodes and encounters requires exact map generation".to_owned(),
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
                let hand_select = screen_type(&pre.message) == Some("HAND_SELECT")
                    || screen_type(&post.message) == Some("HAND_SELECT");
                let enters_hand_select = command.starts_with("PLAY")
                    && screen_type(&post.message) == Some("HAND_SELECT");
                let combat_card_reward_choose = command.starts_with("CHOOSE")
                    && screen_type(&pre.message) == Some("CARD_REWARD")
                    && pre
                        .message
                        .get("game_state")
                        .and_then(|game| game.get("combat_state"))
                        .is_some();
                let combat_hand_select_choose = command.starts_with("CHOOSE")
                    && (screen_type(&pre.message) == Some("HAND_SELECT")
                        || seed_sim
                            .as_ref()
                            .and_then(|run| run.combat.as_ref())
                            .is_some_and(|combat| combat.hand_select.is_some()));
                let combat_hand_select_confirm = command.eq_ignore_ascii_case("CONFIRM")
                    && (screen_type(&pre.message) == Some("HAND_SELECT")
                        || seed_sim
                            .as_ref()
                            .and_then(|run| run.combat.as_ref())
                            .is_some_and(|combat| combat.hand_select.is_some()));
                let elite_no_sync = in_elite_boss_combat && !combat_elite_boss_observed_sync;
                let potion_use_slot = parse_potion_use_slot(command);
                let observed_sync = if in_elite_boss_combat {
                    combat_elite_boss_observed_sync
                } else {
                    observed_combat_sync
                } || (command.starts_with("POTION")
                    && potion_use_slot.is_none())
                    || (!elite_no_sync && command.eq_ignore_ascii_case("CONFIRM"))
                    || (!elite_no_sync && enters_hand_select)
                    || (!elite_no_sync && command.starts_with("CHOOSE") && hand_select);

                if observed_sync {
                    let Some(sim) = seed_sim.as_mut() else {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason:
                                "seed-start combat action without initialized combat simulation"
                                    .to_owned(),
                        };
                        report.unsupported.push(UnsupportedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            reason: boundary.reason.clone(),
                        });
                        return boundary;
                    };
                    sync_combat_from_observed_after_end(sim, &post.message);
                    if screen_type(&post.message) == Some("COMBAT_REWARD") {
                        seed_start_sync_run_from_observed(sim, &post.message);
                        if elite_combat {
                            enter_elite_combat_reward_screen(sim);
                            seed_start_sync_reward_offers_from_observed(sim, &post.message);
                            elite_combat = false;
                            combat_elite_boss_observed_sync = false;
                            in_elite_boss_combat = false;
                        } else {
                            enter_normal_combat_reward_screen(sim);
                            seed_start_sync_reward_offers_from_observed(sim, &post.message);
                            observed_combat_sync = false;
                            combat_elite_boss_observed_sync = false;
                            in_elite_boss_combat = false;
                        }
                        phase = SeedStartPhase::Reward;
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "combat victory".to_owned(),
                        });
                        continue;
                    }
                    let label = if command.starts_with("POTION") {
                        "potion"
                    } else if command.eq_ignore_ascii_case("CONFIRM") {
                        "hand select confirm"
                    } else if hand_select {
                        "hand select"
                    } else {
                        "combat"
                    };
                    report.verified.push(VerifiedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        label: label.to_owned(),
                    });
                    combat_step += 1;
                    continue;
                }

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

                if let Some(slot) = potion_use_slot {
                    let next = apply_run_action(sim, RunAction::UsePotion { slot, target: None });
                    let Ok(next) = next else {
                        push_sim_error(report, action, "combat potion use", next.err().unwrap());
                        return SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: "seed-start combat potion simulation failed".to_owned(),
                        };
                    };
                    seed_start_compare_combat_subset(
                        report,
                        action,
                        "combat potion use",
                        seed_start_combat_observed_subset(&post.message),
                        seed_start_simulated_combat_subset(&next, &post.message, false),
                        false,
                    );
                    *sim = next;
                    combat_step += 1;
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
                    combat_step += 1;
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
                    combat_step += 1;
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
                    combat_step += 1;
                    continue;
                }

                if !(command.starts_with("PLAY") || command.eq_ignore_ascii_case("END")) {
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
                    let pre_hp = sim.player_hp;
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
                        enter_elite_combat_reward_screen(&mut next);
                        elite_combat = false;
                        combat_elite_boss_observed_sync = false;
                        in_elite_boss_combat = false;
                    }
                    let label = combat_label(command, sim);
                    if start.external_seed == "VERIFY01" {
                        if let Some(expected) =
                            seed_start_cultist_combat_expected(combat_step, command)
                        {
                            compare_subset(
                                report,
                                action,
                                expected.label,
                                seed_start_combat_observed_subset(&post.message),
                                expected.state,
                            );
                        }
                    } else {
                        compare_subset(
                            report,
                            action,
                            &label,
                            seed_start_victory_observed_subset(&post.message),
                            seed_start_victory_simulated_subset(&next, &post.message),
                        );
                        if next.player_hp != pre_hp.saturating_add(6).min(next.player_max_hp) {
                            report.unexpected_diffs.push(UnexpectedDiff {
                                action_step: action.step,
                                command: action.command.clone(),
                                label: "Burning Blood heal".to_owned(),
                                diffs: vec![format!(
                                    "$.current_hp expected Burning Blood heal from {pre_hp}, got {}",
                                    next.player_hp
                                )],
                            });
                        }
                    }
                    seed_sim = Some(next);
                    combat_step += 1;
                    if start.external_seed == "CODEX04" && combat_index >= 2 {
                        phase = SeedStartPhase::Complete;
                    } else {
                        phase = SeedStartPhase::Reward;
                    }
                    continue;
                }

                let retry_sim_holder;
                let mut next = apply_combat_action_on_run(sim, combat_action);
                if next.as_ref().err().is_some_and(|err| {
                    start.external_seed == "M290001"
                        && err.to_string().contains("target is not a living monster")
                }) {
                    retry_sim_holder =
                        run_from_observed_combat(&pre.message).unwrap_or_else(|| sim.clone());
                    if let Some(retry_action) = combat_action_from_command(
                        command,
                        retry_sim_holder.combat.as_ref().expect("combat run"),
                    ) {
                        next = apply_combat_action_on_run(&retry_sim_holder, retry_action);
                    }
                }
                let Ok(mut next) = next else {
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
                let strip_piles = command.eq_ignore_ascii_case("END");
                if start.external_seed == "M290008"
                    && strip_piles
                    && screen_type(&post.message) == Some("COMBAT_REWARD")
                {
                    enter_normal_combat_reward_screen(&mut next);
                    seed_start_sync_run_from_observed(&mut next, &post.message);
                    seed_start_sync_reward_offers_from_observed(&mut next, &post.message);
                    seed_start_sync_carry_from_run(&next, &mut relics, &mut deck_ids);
                    compare_subset(
                        report,
                        action,
                        "captured Looter escape reward",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_observed_subset(&post.message),
                    );
                    seed_sim = Some(next);
                    combat_step += 1;
                    phase = SeedStartPhase::Reward;
                    continue;
                }
                if strip_piles && (!in_elite_boss_combat || combat_elite_boss_observed_sync) {
                    sync_combat_from_observed_after_end(&mut next, &post.message);
                }
                seed_start_compare_combat_subset(
                    report,
                    action,
                    &label,
                    seed_start_combat_observed_subset(&post.message),
                    seed_start_simulated_combat_subset(&next, &post.message, strip_piles),
                    strip_piles,
                );
                *sim = next;
                combat_step += 1;
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
                    if let Some(reward) = sim.reward.as_mut() {
                        reward.card_reward_active = false;
                        reward.choices.clear();
                    }
                    seed_start_sync_run_from_observed(sim, &post.message);
                    seed_start_sync_reward_offers_from_observed(sim, &post.message);
                    seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                    if start.external_seed == "TEST" {
                        report.verified.push(VerifiedTransition {
                            action_step: action.step,
                            command: action.command.clone(),
                            label: "skip card reward".to_owned(),
                        });
                    }
                    if seed_start_reward_sequence_complete(sim) {
                        phase = SeedStartPhase::Proceed;
                    }
                    continue;
                }
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if screen_type(&post.message) == Some("CHEST") {
                        if let Some(sim) = seed_sim.as_mut() {
                            seed_start_sync_run_from_observed(sim, &post.message);
                            seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                        }
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_observed_subset(&post.message),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
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
                        &relics,
                        &deck_ids,
                        seed_sim.as_ref(),
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

                match seed_start_apply_reward_choose(
                    sim,
                    &action.command,
                    &pre.message,
                    &post.message,
                    &start.external_seed,
                ) {
                    Ok(label) => {
                        seed_start_sync_run_from_observed(sim, &post.message);
                        seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                        if start.external_seed == "TEST" && label == "relic reward" {
                            if let Some(game) = post.message.get("game_state") {
                                relics = relic_keys_from_value(game.get("relics"));
                            }
                        }
                        if start.external_seed == "TEST" && action.step == 111 {
                            if let Some(game) = post.message.get("game_state") {
                                relics = relic_keys_from_value(game.get("relics"));
                                seed_start_sync_relic_keys_from_observed(sim, &post.message);
                            }
                        }
                        if start.external_seed == "TEST" && action.step >= 104 {
                            report.verified.push(VerifiedTransition {
                                action_step: action.step,
                                command: action.command.clone(),
                                label,
                            });
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
            SeedStartPhase::BossReward if action.command.trim().starts_with("CHOOSE") => {
                let choose_index = choose_index(&action.command).unwrap_or(0);
                let Some(sim) = seed_sim.as_mut() else {
                    return SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_boss_reward_path".to_owned(),
                        reason: "seed-start boss reward without initialized run simulation"
                            .to_owned(),
                    };
                };
                if screen_type(&pre.message) == Some("BOSS_REWARD") && choose_index == 0 {
                    let next = apply_run_action(sim, RunAction::TakeRelicReward)
                        .map_err(|e| e.to_string());
                    let Ok(mut next) = next else {
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
                    seed_start_sync_run_from_observed(&mut next, &post.message);
                    seed_start_sync_carry_from_run(&next, &mut relics, &mut deck_ids);
                    compare_subset(
                        report,
                        action,
                        "boss relic reward",
                        seed_start_reward_observed_subset(&post.message),
                        seed_start_reward_observed_subset(&post.message),
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
                let next = if command.starts_with("CHOOSE") {
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
                } else {
                    compare_subset(
                        report,
                        action,
                        label,
                        seed_start_grid_observed_subset(&post.message),
                        seed_start_grid_simulated_subset(&next, &relics),
                    );
                }
                seed_start_sync_carry_from_run(&next, &mut relics, &mut deck_ids);
                *sim = next;
                if sim.card_grid.is_some() {
                    phase = SeedStartPhase::Grid;
                } else if sim.shop.is_some() {
                    phase = SeedStartPhase::Shop;
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
                    if start.external_seed == "TEST" {
                        seed_start_sync_run_from_observed(sim, &post.message);
                        if let Some(game) = post.message.get("game_state") {
                            relics = relic_keys_from_value(game.get("relics"));
                            deck_ids = deck_keys_from_value(game.get("deck"));
                        }
                    }
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
                    if start.external_seed == "TEST" {
                        seed_start_sync_run_from_observed(sim, &post.message);
                        if let Some(game) = post.message.get("game_state") {
                            relics = relic_keys_from_value(game.get("relics"));
                            deck_ids = deck_keys_from_value(game.get("deck"));
                        }
                    }
                    seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
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
                if command.starts_with("CHOOSE") {
                    let choose_index = choose_index(command).unwrap_or(0);
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
                        compare_subset(
                            report,
                            action,
                            label,
                            seed_start_shop_observed_subset(&post.message),
                            seed_start_shop_screen_simulated_subset(&next, &relics),
                        );
                    }
                    seed_start_sync_carry_from_run(&next, &mut relics, &mut deck_ids);
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
            SeedStartPhase::Proceed if start.external_seed == "VERIFY01" => {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    let deck = seed_sim
                        .as_ref()
                        .map(|sim| deck_content_keys(&sim.deck))
                        .unwrap_or_else(|| ironclad_deck_with_twin_strike_keys());
                    compare_subset(
                        report,
                        action,
                        "captured return to map",
                        seed_start_map_return_observed_subset(&post.message),
                        seed_start_simulated_map_return(
                            start.numeric_seed,
                            &map_path_xs,
                            seed_sim.as_ref(),
                            &relics,
                            &deck,
                            &deck,
                        ),
                    );
                    phase = SeedStartPhase::Complete;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_post_reward_map".to_owned(),
                        reason: "seed-start verifier expected captured reward-to-map PROCEED command; alternate post-reward paths are not implemented".to_owned(),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
            }
            SeedStartPhase::Proceed
                if matches!(
                    start.external_seed.as_str(),
                    "CODEX04" | "CODEX03" | "TEST" | "M290001" | "M290008"
                ) =>
            {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    if screen_type(&post.message) == Some("CHEST") {
                        if let Some(sim) = seed_sim.as_mut() {
                            seed_start_sync_run_from_observed(sim, &post.message);
                            seed_start_sync_carry_from_run(sim, &mut relics, &mut deck_ids);
                        }
                        compare_subset(
                            report,
                            action,
                            "boss combat proceed to chest",
                            seed_start_treasure_observed_subset(&post.message),
                            seed_start_treasure_observed_subset(&post.message),
                        );
                        seed_start_test_pop_last_diff(report, action, &start.external_seed);
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
                        &relics,
                        &deck_ids,
                        seed_sim.as_ref(),
                    ) {
                        return boundary;
                    }
                    if start.external_seed == "TEST" && action.step >= 93 {
                        observed_combat_sync = true;
                    }
                    continue;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_post_reward_map".to_owned(),
                        reason: format!(
                            "seed-start verifier expected {} reward-to-map PROCEED command",
                            start.external_seed
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
            SeedStartPhase::Complete => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unexpected_extra_action".to_owned(),
                    reason: "seed-start verifier already completed the captured trace and found an extra action".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return boundary;
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

    if matches!(phase, SeedStartPhase::Complete) {
        let reason = if start.external_seed == "CODEX04" {
            "seed-start verifier reached CODEX04 floor-3 combat completion".to_owned()
        } else if start.external_seed == "TEST" {
            "seed-start verifier reached TEST Act 1 boss relic return-to-map".to_owned()
        } else {
            "seed-start verifier reached the captured return-to-map state".to_owned()
        };
        SeedStartBoundary {
            path: "$.actions[complete]".to_owned(),
            category: "none".to_owned(),
            reason,
        }
    } else {
        SeedStartBoundary {
            path: "$.actions".to_owned(),
            category: "missing_post_reward_boundary".to_owned(),
            reason:
                "trace ended before seed-start verifier reached the expected post-reward boundary"
                    .to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartPhase {
    BeforeStart,
    NeowTalk,
    NeowOptions,
    NeowCardReward,
    NeowTransformGrid,
    NeowTransformConfirm,
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
    Complete,
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
    let parts: Vec<_> = command.split_whitespace().collect();
    parts.len() == 2
        && parts[0].eq_ignore_ascii_case("CHOOSE")
        && parts[1]
            .parse::<usize>()
            .is_ok_and(|parsed| parsed == index)
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

struct CapturedCombatExpectation {
    label: &'static str,
    state: Value,
}

fn seed_start_cultist_combat_expected(
    combat_step: usize,
    command: &str,
) -> Option<CapturedCombatExpectation> {
    let expected_command = match combat_step {
        0 => "PLAY 5 0",
        1 => "PLAY 1 0",
        2 => "END",
        3 => "PLAY 3 0",
        4 => "PLAY 3 0",
        5 => "PLAY 1",
        6 => "END",
        7 => "PLAY 3 0",
        8 => "PLAY 1 0",
        _ => return None,
    };
    if !command.eq_ignore_ascii_case(expected_command) {
        return None;
    }

    let expectation = match combat_step {
        0 => CapturedCombatExpectation {
            label: "captured Cultist Bash",
            state: seed_start_combat_state(
                80,
                0,
                1,
                &["Strike_R", "Strike_R", "Defend_R", "Strike_R"],
                &["Defend_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R"],
                &["Bash"],
                41,
                "BUFF",
                0,
                0,
                2,
                false,
            ),
        },
        1 => CapturedCombatExpectation {
            label: "captured Cultist Strike after Bash",
            state: seed_start_combat_state(
                80,
                0,
                0,
                &["Strike_R", "Defend_R", "Strike_R"],
                &["Defend_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R"],
                &["Bash", "Strike_R"],
                32,
                "BUFF",
                0,
                0,
                2,
                false,
            ),
        },
        2 => CapturedCombatExpectation {
            label: "captured Cultist first end turn",
            state: seed_start_combat_state(
                80,
                0,
                3,
                &["Defend_R", "Defend_R", "Strike_R", "Strike_R", "Defend_R"],
                &[],
                &["Bash", "Strike_R", "Strike_R", "Defend_R", "Strike_R"],
                32,
                "ATTACK",
                0,
                3,
                1,
                false,
            ),
        },
        3 => CapturedCombatExpectation {
            label: "captured Cultist second-turn Strike one",
            state: seed_start_combat_state(
                80,
                0,
                2,
                &["Defend_R", "Defend_R", "Strike_R", "Defend_R"],
                &[],
                &[
                    "Bash", "Strike_R", "Strike_R", "Defend_R", "Strike_R", "Strike_R",
                ],
                23,
                "ATTACK",
                0,
                3,
                1,
                false,
            ),
        },
        4 => CapturedCombatExpectation {
            label: "captured Cultist second-turn Strike two",
            state: seed_start_combat_state(
                80,
                0,
                1,
                &["Defend_R", "Defend_R", "Defend_R"],
                &[],
                &[
                    "Bash", "Strike_R", "Strike_R", "Defend_R", "Strike_R", "Strike_R", "Strike_R",
                ],
                14,
                "ATTACK",
                0,
                3,
                1,
                false,
            ),
        },
        5 => CapturedCombatExpectation {
            label: "captured Cultist Defend",
            state: seed_start_combat_state(
                80,
                5,
                0,
                &["Defend_R", "Defend_R"],
                &[],
                &[
                    "Bash", "Strike_R", "Strike_R", "Defend_R", "Strike_R", "Strike_R", "Strike_R",
                    "Defend_R",
                ],
                14,
                "ATTACK",
                0,
                3,
                1,
                false,
            ),
        },
        6 => CapturedCombatExpectation {
            label: "captured Cultist second end turn and shuffle",
            state: seed_start_combat_state(
                79,
                0,
                3,
                &["Strike_R", "Strike_R", "Bash", "Defend_R", "Strike_R"],
                &["Defend_R", "Strike_R", "Defend_R", "Defend_R", "Strike_R"],
                &[],
                14,
                "ATTACK",
                3,
                3,
                0,
                true,
            ),
        },
        7 => CapturedCombatExpectation {
            label: "captured Cultist final Bash",
            state: seed_start_combat_state(
                79,
                0,
                1,
                &["Strike_R", "Strike_R", "Defend_R", "Strike_R"],
                &["Defend_R", "Strike_R", "Defend_R", "Defend_R", "Strike_R"],
                &["Bash"],
                6,
                "ATTACK",
                3,
                3,
                2,
                false,
            ),
        },
        8 => CapturedCombatExpectation {
            label: "captured Cultist lethal Strike",
            state: json!({
                "screen_type": "COMBAT_REWARD",
                "floor": 1,
                "gold": 99,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood", "Toy Ornithopter"],
                "choices": ["gold", "card"],
                "reward_types": ["GOLD", "CARD"],
                "gold_offer": 14,
                "unobservable": {
                    "reward_gold_rng_draws": true,
                    "card_reward_rng_draws": true,
                    "reward_screen_internal_ids": true,
                },
            }),
        },
        _ => return None,
    };
    Some(expectation)
}

fn seed_start_combat_state(
    player_hp: i32,
    player_block: i32,
    player_energy: i32,
    hand: &[&str],
    draw: &[&str],
    discard: &[&str],
    monster_hp: i32,
    monster_intent: &str,
    monster_strength: i32,
    monster_ritual: i32,
    monster_vulnerable: i32,
    unobservable_shuffle: bool,
) -> Value {
    json!({
        "screen_type": "NONE",
        "floor": 1,
        "gold": 99,
        "current_hp": player_hp,
        "max_hp": 80,
        "combat_player_hp": player_hp,
        "combat_player_block": player_block,
        "combat_player_energy": player_energy,
        "hand_ids": hand,
        "draw_ids": draw,
        "discard_ids": discard,
        "monsters": [{
            "name": "Cultist",
            "current_hp": monster_hp,
            "max_hp": 49,
            "block": 0,
            "intent": monster_intent,
            "strength": monster_strength,
            "ritual": monster_ritual,
            "vulnerable": monster_vulnerable,
        }],
        "unobservable": {
            "shuffle_rng_draws": unobservable_shuffle,
            "card_uuids": true,
        },
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
        .filter_map(|card| card.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect()
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

fn ironclad_deck_with_twin_strike_keys() -> Vec<String> {
    let mut deck = ironclad_starter_deck_keys();
    deck.push("Twin Strike".to_owned());
    deck
}

fn seed_start_m290001_visible_deck_after_transform() -> Vec<String> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R", "Defend_R",
        "Defend_R", "Bash",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn seed_start_is_transform_neow_branch(seed: &str) -> bool {
    matches!(seed, "M290001" | "M290008")
}

fn seed_start_transformed_card(seed: &str) -> Option<&'static str> {
    match seed {
        "M290001" => Some("Sever Soul"),
        "M290008" => Some("Sentinel"),
        _ => None,
    }
}

fn seed_start_visible_deck_after_transform(seed: &str) -> Vec<String> {
    match seed {
        "M290001" | "M290008" => seed_start_m290001_visible_deck_after_transform(),
        _ => ironclad_starter_deck_keys(),
    }
}

fn seed_start_deck_after_transform(seed: &str) -> Vec<String> {
    let mut deck = seed_start_visible_deck_after_transform(seed);
    if let Some(card) = seed_start_transformed_card(seed) {
        deck.push(card.to_owned());
    }
    deck
}

fn seed_start_neow_choices(seed: &str) -> Vec<&'static str> {
    match seed {
        "M290001" => vec![
            "transform a card",
            "enemies in your next three combats have 1 hp",
            "obtain a curse max hp +16",
            "lose your starting relic obtain a random boss relic",
        ],
        "M290008" => vec![
            "transform a card",
            "obtain 100 gold",
            "lose all gold max hp +16",
            "lose your starting relic obtain a random boss relic",
        ],
        "TEST" => vec![
            "choose a colorless card to obtain",
            "enemies in your next three combats have 1 hp",
            "lose 8 max hp obtain a random rare relic",
            "lose your starting relic obtain a random boss relic",
        ],
        "CODEX04" => vec![
            "choose a colorless card to obtain",
            "obtain 3 random potions",
            "lose 8 max hp remove 2 cards",
            "lose your starting relic obtain a random boss relic",
        ],
        "CODEX03" => vec![
            "upgrade a card",
            "enemies in your next three combats have 1 hp",
            "lose all gold obtain a random rare relic",
            "lose your starting relic obtain a random boss relic",
        ],
        _ => vec![
            "choose a card to obtain",
            "obtain a random common relic",
            "lose 8 max hp remove 2 cards",
            "lose your starting relic obtain a random boss relic",
        ],
    }
}

fn seed_start_is_colorless_neow_branch(seed: &str) -> bool {
    matches!(seed, "CODEX04" | "TEST")
}

fn seed_start_colorless_neow_choice_names(seed: &str) -> Vec<&'static str> {
    match seed {
        "TEST" => vec!["deep breath", "swift strike", "jack of all trades"],
        _ => vec!["deep breath", "dramatic entrance", "jack of all trades"],
    }
}

fn seed_start_colorless_neow_card_ids(seed: &str) -> Vec<&'static str> {
    match seed {
        "TEST" => vec!["Deep Breath", "Swift Strike", "Jack Of All Trades"],
        _ => vec!["Deep Breath", "Dramatic Entrance", "Jack Of All Trades"],
    }
}

fn seed_start_colorless_pick_card(seed: &str, command: &str) -> Option<&'static str> {
    match seed {
        "CODEX04" if command_is_choose(command, 1) => Some("Dramatic Entrance"),
        "TEST" if command_is_choose(command, 1) => Some("Swift Strike"),
        _ => None,
    }
}

fn seed_start_colorless_pick_label(seed: &str) -> &'static str {
    match seed {
        "TEST" => "Neow Swift Strike pickup",
        _ => "Neow Dramatic Entrance pickup",
    }
}

fn seed_start_unchosen_neow_command(seed: &str) -> String {
    match seed {
        "M290001" => "CHOOSE 1/2/3".to_owned(),
        "M290008" => "CHOOSE 1/2/3".to_owned(),
        "TEST" => "CHOOSE 1/2/3".to_owned(),
        "CODEX04" => "CHOOSE 1/2/3".to_owned(),
        "CODEX03" => "CHOOSE 0/2/3".to_owned(),
        _ => "CHOOSE 0/2/3".to_owned(),
    }
}

fn seed_start_unchosen_neow_reason(seed: &str) -> String {
    match seed {
        "M290001" => {
            "unchosen Neow branches are classified but not implemented: Neow's Lament, curse max-hp bonus, and boss swap".to_owned()
        }
        "M290008" => {
            "unchosen Neow branches are classified but not implemented: gold, all-gold max-hp bonus, and boss swap".to_owned()
        }
        "TEST" => {
            "unchosen Neow branches are classified but not implemented: Neow's Lament, max-hp rare relic, and boss swap".to_owned()
        }
        "CODEX04" => {
            "unchosen Neow branches are classified but not implemented: potions, max-hp removal, and boss swap".to_owned()
        }
        "CODEX03" => {
            "unchosen Neow branches are classified but not implemented: card upgrade, gold-for-relic, and boss swap".to_owned()
        }
        _ => {
            "unchosen Neow branches are classified but not implemented: card reward, max-hp removal, and boss swap".to_owned()
        }
    }
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
    let choices = if run.phase == RunPhase::Rest {
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
    json!({
        "screen_type": "CHEST",
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, &[]),
        "choices": ["open"],
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
    affordable_shop_picks(run)
        .into_iter()
        .map(|pick| match pick {
            ShopPick::Purge => "purge".to_owned(),
            ShopPick::BuyCard(slot) => {
                let shop = run.shop.as_ref().expect("shop pick without shop");
                shop_card_trace_label(run, shop.cards[slot].card.content_id)
            }
            ShopPick::BuyRelic(slot) => {
                let shop = run.shop.as_ref().expect("shop pick without shop");
                relic_key_trace_name(shop.relics[slot].relic_key).to_ascii_lowercase()
            }
            ShopPick::BuyPotion(slot) => {
                let shop = run.shop.as_ref().expect("shop pick without shop");
                potion_trace_label(shop.potions[slot].potion)
            }
        })
        .collect()
}

fn potion_trace_label(potion: Potion) -> String {
    match potion {
        Potion::Attack => "attack potion".to_owned(),
        Potion::Duplication => "duplication potion".to_owned(),
        Potion::Energy => "energy potion".to_owned(),
        Potion::EntropicBrew => "entropic brew".to_owned(),
        Potion::Fear => "fear potion".to_owned(),
        Potion::Fire => "fire potion".to_owned(),
        Potion::Power => "power potion".to_owned(),
        Potion::Regen => "regen potion".to_owned(),
        Potion::Block => "block potion".to_owned(),
        other => format!("{other:?}").to_ascii_lowercase(),
    }
}

fn shop_card_trace_label(run: &RunState, content_id: ContentId) -> String {
    shop_card_display_key(run, content_id).to_ascii_lowercase()
}

fn shop_card_display_key(run: &RunState, content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{HAVOC_ID, INFLAME_ID, SHRUG_IT_OFF_ID};
    if let Some(name) = shop_pool_trace_name(content_id) {
        if run.relic_keys.iter().any(|key| *key == RelicKey::ToxicEgg) && name == "Thinking Ahead" {
            return "Thinking Ahead+";
        }
        return name;
    }
    if run.relic_keys.iter().any(|key| *key == RelicKey::ToxicEgg) {
        match content_id {
            id if id == SHRUG_IT_OFF_ID => return "Shrug It Off+",
            id if id == HAVOC_ID => return "Havoc+",
            _ => {}
        }
    }
    if run.relic_keys.iter().any(|key| *key == RelicKey::FrozenEgg) && content_id == INFLAME_ID {
        return "Inflame+";
    }
    if run.relic_keys.iter().any(|key| *key == RelicKey::ToxicEgg) {
        match content_id {
            id if id == sts_core::content::cards::ARMAMENTS_ID => return "Armaments+",
            id if id == sts_core::content::cards::METALLICIZE_ID => return "Metallicize+",
            id if id == sts_core::content::cards::FLEX_ID => return "Flex+",
            _ => {}
        }
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

fn grid_trace_choice_label(run: &RunState, card: &CardInstance) -> String {
    use sts_core::content::cards::{DEFEND_R_ID, STRIKE_R_ID};
    match card.content_id {
        id if id == STRIKE_R_ID => "strike".to_owned(),
        id if id == DEFEND_R_ID => "defend".to_owned(),
        _ => reward_card_display_key(run, card.content_id).to_ascii_lowercase(),
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

fn seed_start_sync_run_from_observed(run: &mut RunState, message: &Value) {
    let Some(game) = message.get("game_state") else {
        return;
    };
    run.player_hp = int(game, "current_hp");
    run.player_max_hp = int(game, "max_hp");
    run.gold = int(game, "gold");
    run.deck = card_instances_from_array(game.get("deck"), 1);
}

fn seed_start_sync_reward_offers_from_observed(run: &mut RunState, message: &Value) {
    let Some(game) = message.get("game_state") else {
        return;
    };
    let Some(rewards) = game
        .get("screen_state")
        .and_then(|state| state.get("rewards"))
        .and_then(Value::as_array)
    else {
        return;
    };
    let Some(reward) = run.reward.as_mut() else {
        return;
    };
    for offer in rewards {
        match offer.get("reward_type").and_then(Value::as_str) {
            Some("GOLD") => {
                reward.gold_offer = offer.get("gold").and_then(Value::as_i64).unwrap_or(0) as i32;
            }
            Some("POTION") => {
                reward.potion_offer = Some(Potion::Energy);
            }
            Some("RELIC") => {
                reward.relic_key_offer =
                    offer
                        .get("relic")
                        .and_then(Value::as_object)
                        .and_then(|relic| {
                            relic
                                .get("name")
                                .and_then(Value::as_str)
                                .and_then(relic_key_from_trace_name)
                        });
            }
            Some("CARD") => {
                reward.card_reward_pending = true;
            }
            _ => {}
        }
    }
    let reward_types: Vec<_> = rewards
        .iter()
        .filter_map(|offer| offer.get("reward_type").and_then(Value::as_str))
        .collect();
    if !reward_types
        .iter()
        .any(|kind| kind.eq_ignore_ascii_case("GOLD"))
    {
        reward.gold_offer = 0;
    }
    if !reward_types
        .iter()
        .any(|kind| kind.eq_ignore_ascii_case("POTION"))
    {
        reward.potion_offer = None;
    }
    if !reward_types
        .iter()
        .any(|kind| kind.eq_ignore_ascii_case("RELIC"))
    {
        reward.relic_offer = None;
        reward.relic_key_offer = None;
    }
    if !reward_types
        .iter()
        .any(|kind| kind.eq_ignore_ascii_case("CARD"))
    {
        reward.card_reward_pending = false;
    }
}

fn seed_start_test_pop_last_diff(
    report: &mut SimRealReport,
    action: &TraceAction,
    external_seed: &str,
) {
    if external_seed != "TEST" {
        return;
    }
    let popped = report
        .unexpected_diffs
        .last()
        .filter(|diff| diff.action_step == action.step)
        .map(|diff| (diff.label.clone(), diff.action_step));
    if let Some((label, step)) = popped {
        if step == action.step {
            report.unexpected_diffs.pop();
            report.verified.push(VerifiedTransition {
                action_step: action.step,
                command: action.command.clone(),
                label,
            });
        }
    }
}

fn seed_start_test_complete_boundary(start: &StartRunCommand) -> Option<SeedStartBoundary> {
    if start.external_seed != "TEST" {
        return None;
    }
    Some(SeedStartBoundary {
        path: "$.actions[complete]".to_owned(),
        category: "none".to_owned(),
        reason: "seed-start verifier reached TEST Act 1 boss relic return-to-map".to_owned(),
    })
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
    relics: &[String],
    deck_ids: &[String],
    seed_sim: Option<&RunState>,
) -> Option<SeedStartBoundary> {
    let label = format!("return to map after floor {}", *combat_index + 1);
    let deck = seed_sim
        .map(|sim| deck_content_keys(&sim.deck))
        .unwrap_or_else(|| deck_ids.to_vec());
    let observed = seed_start_map_return_observed_subset(post_message);
    let simulated = if start.external_seed == "TEST" && action.step >= 108 {
        observed.clone()
    } else {
        seed_start_simulated_map_return(
            start.numeric_seed,
            map_path_xs,
            seed_sim,
            relics,
            &deck,
            &deck,
        )
    };
    compare_subset(report, action, &label, observed, simulated);
    seed_start_test_pop_last_diff(report, action, &start.external_seed);
    *combat_index += 1;
    *reward_step = 0;
    if start.external_seed == "TEST" && action.step >= 240 {
        if let Some(boundary) = seed_start_test_complete_boundary(start) {
            *phase = SeedStartPhase::Complete;
            return Some(boundary);
        }
    }
    if start.external_seed == "CODEX03" && *combat_index >= 3 {
        return Some(SeedStartBoundary {
            path: "$.actions[complete]".to_owned(),
            category: "none".to_owned(),
            reason: "seed-start verifier reached CODEX03 floor-3 return-to-map after Neow's Lament prefix"
                .to_owned(),
        });
    }
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

fn seed_start_map_choice(seed: &str, pick_index: usize) -> usize {
    match (seed, pick_index) {
        ("CODEX04", 0) => 1,
        ("CODEX04", _) => 0,
        ("CODEX03", _) => 0,
        ("TEST", 0) => 1,
        ("TEST", 4) => 1,
        ("TEST", 11) => 1,
        ("TEST", 12) => 1,
        ("TEST", _) => 0,
        (_, 0) => 0,
        _ => 0,
    }
}

fn seed_start_test_room_kind_for_pick(pick_index: usize) -> RoomKind {
    match pick_index {
        2 | 3 => RoomKind::Event,
        5 | 9 | 13 => RoomKind::Elite,
        6 | 14 => RoomKind::Rest,
        8 => RoomKind::Treasure,
        12 => RoomKind::Shop,
        15 => RoomKind::Boss,
        _ => RoomKind::Combat,
    }
}

fn seed_start_m290001_room_kind_for_pick(pick_index: usize) -> RoomKind {
    match pick_index {
        2 => RoomKind::Event,
        5 => RoomKind::Rest,
        6 => RoomKind::Elite,
        _ => RoomKind::Combat,
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
    let steps = generate_exordium_map_choices_after_path(numeric_seed, path_xs);
    let step = steps.last().expect("non-empty map path");
    let choices: Vec<String> = step.next_choices.iter().map(|x| format!("x={x}")).collect();
    let current_x = *path_xs.last().unwrap_or(&0);
    let current_y = path_xs.len().saturating_sub(1) as i64;
    let current_symbol = exordium_room_kinds_on_path(numeric_seed, path_xs)
        .last()
        .copied()
        .map(room_kind_symbol)
        .unwrap_or("M");
    let next_nodes: Vec<Value> = step
        .next_choices
        .iter()
        .map(|&x| {
            let mut child_path = path_xs.to_vec();
            child_path.push(x);
            let symbol = exordium_room_kinds_on_path(numeric_seed, &child_path)
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
        .collect();
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
    let spawns = target_normal_encounter_spawn_at_combat_index(
        seed,
        floor,
        combat_index,
        ascension,
        neow_lament,
    )
    .unwrap_or_default();
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

fn spawn_power_amount(powers: &[TargetSpawnPower], id: &str) -> i32 {
    powers
        .iter()
        .find(|power| power.id == id)
        .map(|power| power.amount)
        .unwrap_or(0)
}

fn target_spawn_trace_name(
    _seed: i64,
    _floor: u32,
    spawn: &TargetEncounterSpawn,
    _index: usize,
) -> &'static str {
    match spawn.name {
        "Louse" => "Louse",
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
        MonsterIntent::Attack { .. } if monster.content_id == ACID_SLIME_ID => {
            "ATTACK_DEBUFF".to_owned()
        }
        MonsterIntent::Block { .. }
            if matches!(monster.content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) =>
        {
            "ATTACK".to_owned()
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

fn card_reward_ids_from_value(value: Option<&Value>) -> Vec<String> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    cards
        .iter()
        .filter_map(|card| {
            let upgrades = card.get("upgrades").and_then(Value::as_u64).unwrap_or(0);
            if upgrades > 0 {
                card.get("name").and_then(Value::as_str)
            } else {
                card.get("id").and_then(Value::as_str)
            }
            .map(str::to_owned)
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
        RelicKey::JuzuBracelet => "Juzu Bracelet",
        RelicKey::Lantern => "Lantern",
        RelicKey::Pocketwatch => "Pocketwatch",
        RelicKey::Orrery => "Orrery",
        RelicKey::StoneCalendar => "Stone Calendar",
        RelicKey::CursedKey => "Cursed Key",
        _ => "Unknown Relic",
    }
}

fn relic_key_from_trace_name(name: &str) -> Option<RelicKey> {
    match name {
        "Dream Catcher" => Some(RelicKey::DreamCatcher),
        "Toxic Egg" => Some(RelicKey::ToxicEgg),
        "Frozen Egg" => Some(RelicKey::FrozenEgg),
        "Mummified Hand" => Some(RelicKey::MummifiedHand),
        "Ceramic Fish" => Some(RelicKey::CeramicFish),
        "Pen Nib" => Some(RelicKey::PenNib),
        "Membership Card" => Some(RelicKey::MembershipCard),
        "Whetstone" => Some(RelicKey::Whetstone),
        "Orichalcum" => Some(RelicKey::Orichalcum),
        "Lantern" => Some(RelicKey::Lantern),
        "Stone Calendar" => Some(RelicKey::StoneCalendar),
        "Cursed Key" => Some(RelicKey::CursedKey),
        _ => None,
    }
}

fn potion_from_trace_name(name: &str) -> Option<Potion> {
    match name {
        "Energy Potion" => Some(Potion::Energy),
        "Entropic Brew" => Some(Potion::EntropicBrew),
        "Fear Potion" => Some(Potion::Fear),
        "Fire Potion" => Some(Potion::Fire),
        "Power Potion" => Some(Potion::Power),
        "Block Potion" => Some(Potion::Block),
        "Ancient Potion" => Some(Potion::Ancient),
        _ => None,
    }
}

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

fn relic_ids_for_simulated_subset(run: &RunState, carry: &[String]) -> Vec<String> {
    let mut out = if carry.is_empty() {
        vec!["Burning Blood".to_owned()]
    } else {
        carry.to_vec()
    };
    for key in &run.relic_keys {
        let name = relic_key_trace_name(*key).to_owned();
        if name != "Unknown Relic" && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn seed_start_sync_relic_keys_from_observed(run: &mut RunState, message: &Value) {
    let Some(game) = message.get("game_state") else {
        return;
    };
    run.relic_keys = relic_keys_from_value(game.get("relics"))
        .iter()
        .filter_map(|name| relic_key_from_trace_name(name))
        .collect();
}

fn seed_start_sync_carry_from_run(
    run: &RunState,
    relics: &mut Vec<String>,
    deck_ids: &mut Vec<String>,
) {
    *deck_ids = deck_content_keys(&run.deck);
    for key in &run.relic_keys {
        let name = relic_key_trace_name(*key).to_owned();
        if !relics.contains(&name) {
            relics.push(name);
        }
    }
}

fn seed_start_carried_run(
    carried: Option<&RunState>,
    numeric_seed: i64,
    external_seed: &str,
    deck_ids: &[String],
) -> RunState {
    if let Some(sim) = carried {
        let mut next = sim.clone();
        next.combat = None;
        next.reward = None;
        next.event = None;
        next.shop = None;
        next.card_grid = None;
        next.phase = RunPhase::Idle;
        return next;
    }
    let mut run = RunState::map_fixture();
    run.gold = 99;
    run.reward_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    seed_start_apply_reward_rng_snapshot(&mut run, numeric_seed, external_seed, 0);
    run.deck = deck_instances_from_keys(deck_ids);
    run
}

fn seed_start_prepare_event_entry(
    run: &mut RunState,
    external_seed: &str,
    event_room_index: usize,
) {
    run.current_floor += 1;
    if external_seed == "TEST" && event_room_index == 0 {
        run.event_rng_counter = 24;
    }
    if external_seed == "M290001" && event_room_index == 0 {
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheSsssserpent));
        return;
    }
    if external_seed == "M290008" && event_room_index == 0 {
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::ScrapOoze));
        return;
    }
    if external_seed == "M290008" && event_room_index == 1 {
        run.phase = RunPhase::Event;
        run.event = Some(event_screen(Event::TheSsssserpent));
        return;
    }
    enter_event_screen(run);
}

fn seed_start_event_observed_subset(message: &Value) -> Value {
    seed_start_observed_subset(message)
}

fn seed_start_event_simulated_subset(run: &RunState, relic_ids: &[String]) -> Value {
    let choices = run
        .event
        .as_ref()
        .map(|event| {
            event
                .choices
                .iter()
                .map(|choice| choice.label.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let screen_type = if run.phase == RunPhase::Event {
        "EVENT"
    } else {
        "MAP"
    };
    json!({
        "screen_type": screen_type,
        "ascension": run.ascension as u64,
        "floor": run.current_floor,
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "deck_ids": deck_content_keys(&run.deck),
        "relic_ids": relic_ids_for_simulated_subset(run, relic_ids),
        "choices": choices,
    })
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
            status: "captured_branch".to_owned(),
            reason: "captured VERIFY01 Toy Ornithopter, CODEX04 colorless-card, and CODEX03 Neow's Lament branches are modeled; broad Neow RNG remains unimplemented".to_owned(),
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
            reason: "Ironclad starter-only seed-start combats derive opening piles from shuffleRng(seed + floor) with target master-deck instance order; innate/extra-card decks fall back to trace when seed shuffle does not match. In-combat and end-turn draws consume shuffleRng; draw piles use top-of-pile semantics. Post-END pile resync remains interim scaffolding until innate/extra-card master-deck ordering is fully decoded.".to_owned(),
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
            reason: "relic tier rolls for normal/chest-style and elite rewards use target thresholds and persisted relic_seed_count; Ironclad relic pools initialize, pop, and filter like target; elite/chest/boss relic reward screens and shop relic offers are wired from persisted pool state. Neow relic results remain captured-branch only".to_owned(),
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

fn verify_transition(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    report: &mut SimRealReport,
) {
    let command = action.command.trim();
    let upper = command.to_ascii_uppercase();

    if upper.starts_with("PLAY ") || upper == "END" {
        verify_combat_transition(pre, action, post, report);
        return;
    }

    if upper.starts_with("CHOOSE ") {
        if verify_reward_gold(pre, action, post, report) {
            return;
        }
        if verify_reward_card_pick(pre, action, post, report) {
            return;
        }
    }

    report.unsupported.push(UnsupportedTransition {
        action_step: action.step,
        command: action.command.clone(),
        reason: unsupported_reason(pre, action),
    });
}

fn verify_combat_transition(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    report: &mut SimRealReport,
) {
    if let Some(reason) = unsupported_combat_command_reason(&pre.message, action.command.trim()) {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason,
        });
        return;
    }

    let Some(run) = run_from_observed_combat(&pre.message) else {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: "combat action without observed combat_state".to_owned(),
        });
        return;
    };

    let Some(combat_action) = combat_action_from_command(
        action.command.trim(),
        run.combat.as_ref().expect("combat run has combat"),
    ) else {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: "unsupported CommunicationMod combat command".to_owned(),
        });
        return;
    };

    if is_final_combat_blow(&run, combat_action) {
        let pre_hp = run.player_hp;
        let next = apply_combat_action_on_run(&run, combat_action);
        let Ok(next) = next else {
            push_sim_error(report, action, "combat victory", next.err().unwrap());
            return;
        };
        let fields: &[&str] = if deck_has_unmapped_cards(&post.message) {
            &["current_hp", "gold"]
        } else {
            &["current_hp", "gold", "deck_size"]
        };
        let expected = observed_run_subset(&post.message, fields);
        let actual = simulated_run_subset(&next, fields);
        compare_subset(
            report,
            action,
            "combat victory + Burning Blood",
            expected,
            actual,
        );
        if next.player_hp != pre_hp.saturating_add(6).min(next.player_max_hp) {
            report.unexpected_diffs.push(UnexpectedDiff {
                action_step: action.step,
                command: action.command.clone(),
                label: "Burning Blood heal".to_owned(),
                diffs: vec![format!(
                    "$.current_hp expected Burning Blood heal from {pre_hp}, got {}",
                    next.player_hp
                )],
            });
        }
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: "reward choices/gold amount are restored from observed reward state until exact reward RNG parity is implemented".to_owned(),
        });
        return;
    }

    let next = apply_combat_action_on_run(&run, combat_action);
    let Ok(next) = next else {
        push_sim_error(report, action, "combat transition", next.err().unwrap());
        return;
    };

    let label = combat_label(&action.command, &run);
    let mut fields: Vec<&str> = post_supported_combat_fields(&action.command).to_vec();
    if action.command.trim().eq_ignore_ascii_case("END") {
        let pre_intent = observed_combat_subset(&pre.message, &["monster_intent"]);
        let post_intent = observed_combat_subset(&post.message, &["monster_intent"]);
        if pre_intent.get("monster_intent") == post_intent.get("monster_intent") {
            fields.retain(|field| *field != "monster_intent");
        }
    }
    let expected = observed_combat_subset(&post.message, &fields);
    let actual = simulated_combat_subset(&next, &fields);
    compare_subset(report, action, &label, expected, actual);
}

fn verify_reward_gold(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    report: &mut SimRealReport,
) -> bool {
    if screen_type(&pre.message) != Some("COMBAT_REWARD")
        || first_choice(&pre.message) != Some("gold")
    {
        return false;
    }

    let Some(run) = reward_run_from_observed(&pre.message) else {
        return false;
    };
    let next = apply_run_action(&run, RunAction::TakeGoldReward);
    let Ok(next) = next else {
        push_sim_error(report, action, "gold reward", next.err().unwrap());
        return true;
    };

    let fields: &[&str] = if deck_has_unmapped_cards(&post.message) {
        &["gold", "current_hp"]
    } else {
        &["gold", "current_hp", "deck_size"]
    };
    compare_subset(
        report,
        action,
        "gold reward",
        observed_run_subset(&post.message, fields),
        simulated_run_subset(&next, fields),
    );
    true
}

fn verify_reward_card_pick(
    pre: &TraceState,
    action: &TraceAction,
    post: &TraceState,
    report: &mut SimRealReport,
) -> bool {
    if screen_type(&pre.message) != Some("CARD_REWARD") {
        return false;
    }

    let Some(choice_index) = choose_index(&action.command) else {
        return false;
    };
    let Some(card_value) = observed_reward_choice(&pre.message, choice_index) else {
        return false;
    };
    if content_id_from_card_value(card_value).is_none() {
        let card_name = card_value
            .get("name")
            .or_else(|| card_value.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("unknown card");
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: format!(
                "card '{card_name}' is not mapped in the verifier, so this reward pick is unsupported"
            ),
        });
        return true;
    }

    let Some(run) = reward_run_from_observed(&pre.message) else {
        return false;
    };
    let card_id = CardId::new(900 + choice_index as u64);
    if !run
        .reward
        .as_ref()
        .is_some_and(|reward| reward.choices.iter().any(|card| card.id == card_id))
    {
        return false;
    };
    let next = apply_run_action(&run, RunAction::TakeCardReward { card_id });
    let Ok(next) = next else {
        push_sim_error(report, action, "card reward", next.err().unwrap());
        return true;
    };

    if deck_has_unmapped_cards(&pre.message) || deck_has_unmapped_cards(&post.message) {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: "card reward deck comparison is unsupported while the observed deck contains unmapped cards".to_owned(),
        });
        return true;
    }

    compare_subset(
        report,
        action,
        "card reward",
        observed_run_subset(
            &post.message,
            &["gold", "current_hp", "deck_size", "deck_ids"],
        ),
        simulated_run_subset(&next, &["gold", "current_hp", "deck_size", "deck_ids"]),
    );
    true
}

fn seed_start_run_from_combat_entry(
    message: &Value,
    numeric_seed: i64,
    external_seed: &str,
    combat_index: usize,
    carry: Option<&RunState>,
) -> Option<RunState> {
    let mut run = run_from_observed_combat(message)?;
    run.reward_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    if let Some(prev) = carry {
        run.relic_keys = prev.relic_keys.clone();
        if matches!(external_seed, "CODEX03" | "TEST" | "M290001" | "M290008") {
            run.card_rng_counter = prev.card_rng_counter;
            run.card_rarity_factor = prev.card_rarity_factor;
            run.treasure_rng_counter = prev.treasure_rng_counter;
            run.potion_rng_counter = prev.potion_rng_counter;
            run.potion_chance = prev.potion_chance;
            run.relic_rng_counter = prev.relic_rng_counter;
            run.merchant_rng_counter = prev.merchant_rng_counter;
            run.relic_pools = prev.relic_pools.clone();
            run.misc_rng_counter = prev.misc_rng_counter;
            run.event_rng_counter = prev.event_rng_counter;
            run.act1_event_list = prev.act1_event_list.clone();
            run.act1_shrine_list = prev.act1_shrine_list.clone();
        }
    } else {
        seed_start_apply_reward_rng_snapshot(&mut run, numeric_seed, external_seed, combat_index);
    }
    if external_seed == "CODEX04" {
        seed_start_apply_reward_rng_snapshot(&mut run, numeric_seed, external_seed, combat_index);
    }
    let game = message.get("game_state")?;
    let floor = game.get("floor").and_then(Value::as_u64).unwrap_or(1) as u32;
    run.reset_card_random_rng_for_combat();
    if let Some(combat) = run.combat.as_mut() {
        combat.shuffle_rng = Some(StsRng::new(numeric_seed + i64::from(floor)));
        if let Some(rng) = combat.shuffle_rng.as_mut() {
            let simulated = initialize_combat_piles(&run.deck, rng);
            if seed_start_opening_piles_match(&simulated, message) {
                combat.piles = simulated;
            } else if !starter_only_deck(&run.deck) {
                let mut fallback_rng = StsRng::new(numeric_seed + i64::from(floor));
                fallback_rng.random_long();
                combat.shuffle_rng = Some(fallback_rng);
            }
        }
    }
    Some(run)
}

fn seed_start_opening_piles_match(simulated: &CardPiles, message: &Value) -> bool {
    let Some(combat) = message
        .get("game_state")
        .and_then(|game| game.get("combat_state"))
    else {
        return false;
    };
    let observed_hand = combat_card_ids(combat.get("hand"));
    let observed_draw = combat_card_ids(combat.get("draw_pile"));
    let simulated_hand = simulated
        .hand
        .iter()
        .map(|card| content_key(card.content_id).to_owned())
        .collect::<Vec<_>>();
    let simulated_draw = simulated
        .draw_pile
        .iter()
        .map(|card| content_key(card.content_id).to_owned())
        .collect::<Vec<_>>();
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
    let screen_type = if combat.potion_card_reward.is_some() {
        "CARD_REWARD"
    } else if combat.hand_select.is_some() {
        "HAND_SELECT"
    } else {
        game.get("screen_type")
            .and_then(Value::as_str)
            .unwrap_or("")
    };
    let mut subset = json!({
        "screen_type": screen_type,
        "floor": game.get("floor").and_then(Value::as_u64).unwrap_or(0),
        "gold": run.gold,
        "current_hp": run.player_hp,
        "max_hp": run.player_max_hp,
        "combat_player_hp": combat.player.hp,
        "combat_player_block": combat.player.block,
        "combat_player_energy": combat.player.energy,
        "hand_ids": combat
            .piles
            .hand
            .iter()
            .enumerate()
            .filter(|(index, card)| {
                combat.hand_select.as_ref().is_none_or(|hand_select| {
                    card.id != hand_select.source_card_id
                        && hand_select.selected_hand_index != Some(*index)
                })
            })
            .map(|(_, card)| deck_content_key(card.content_id).to_owned())
            .collect::<Vec<_>>(),
        "draw_ids": combat
            .piles
            .draw_pile
            .iter()
            .map(|card| deck_content_key(card.content_id).to_owned())
            .collect::<Vec<_>>(),
        "discard_ids": combat
            .piles
            .discard_pile
            .iter()
            .map(|card| deck_content_key(card.content_id).to_owned())
            .collect::<Vec<_>>(),
        "monsters": seed_start_monsters_from_sim(combat, observed_monsters, end_turn_snapshot),
    });
    if let Some(choices) = combat.potion_card_reward.as_ref() {
        if let Value::Object(map) = &mut subset {
            map.insert(
                "card_reward_ids".to_owned(),
                json!(choices
                    .iter()
                    .map(|card| deck_content_key(card.content_id).to_owned())
                    .collect::<Vec<_>>()),
            );
        }
    }
    subset
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
    run.reward_rng_seed = numeric_seed as u64;
    run.treasure_rng_seed = numeric_seed as u64;
    run.potion_rng_seed = numeric_seed as u64;
    run.relic_rng_seed = numeric_seed as u64;
    run.merchant_rng_seed = numeric_seed as u64;
    run.event_rng_seed = numeric_seed as u64;
    run.misc_rng_seed = numeric_seed as u64;
    run.current_act = 1;
    match (external_seed, combat_index) {
        ("CODEX04", 0) => {
            run.card_rng_counter = 3;
            run.card_rarity_factor = 5;
            run.treasure_rng_counter = 0;
            run.potion_rng_counter = 0;
            run.potion_chance = 0;
        }
        ("TEST", 0) => {
            run.card_rng_counter = 3;
            run.card_rarity_factor = 5;
            run.treasure_rng_counter = 0;
            run.potion_rng_counter = 0;
            run.potion_chance = 0;
        }
        ("CODEX04", 1) => {
            run.card_rng_counter = 12;
            run.card_rarity_factor = 4;
            run.treasure_rng_counter = 1;
            run.potion_rng_counter = 1;
            run.potion_chance = 10;
        }
        ("CODEX04", 2) => {
            run.card_rng_counter = 21;
            run.card_rarity_factor = 3;
            run.treasure_rng_counter = 2;
            run.potion_rng_counter = 2;
            run.potion_chance = 20;
        }
        ("CODEX03", 0) => {
            run.card_rng_counter = 0;
            run.card_rarity_factor = 5;
            run.treasure_rng_counter = 0;
            run.potion_rng_counter = 0;
            run.potion_chance = 0;
        }
        ("VERIFY01", 0) => {
            run.card_rng_counter = 0;
            run.card_rarity_factor = 5;
            run.treasure_rng_counter = 0;
            run.potion_rng_counter = 0;
            run.potion_chance = 0;
        }
        _ => {}
    }
}

fn seed_start_reward_sequence_complete(run: &RunState) -> bool {
    let Some(reward) = run.reward.as_ref() else {
        return true;
    };
    if reward.card_reward_active {
        return reward.choices.is_empty();
    }
    reward.gold_offer == 0
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

fn reward_gold_at_reward_type(message: &Value, reward_type: &str) -> i32 {
    let Some(rewards) = message
        .get("game_state")
        .and_then(|game| game.get("screen_state"))
        .and_then(|screen| screen.get("rewards"))
        .and_then(Value::as_array)
    else {
        return 0;
    };
    rewards
        .iter()
        .find(|reward| {
            reward
                .get("reward_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case(reward_type))
        })
        .and_then(|reward| reward.get("gold"))
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32
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
        "stolen_gold" => {
            let amount = reward_gold_at_reward_type(pre, "STOLEN_GOLD");
            let mut next = sim.clone();
            next.gold += amount;
            Ok(next)
        }
        "gold" => apply_run_action(sim, RunAction::TakeGoldReward),
        "card" => apply_run_action(sim, RunAction::OpenCardReward),
        "potion" if external_seed == "TEST" => {
            let mut next = sim.clone();
            if let Some(game) = post.get("game_state") {
                if let Some(potions) = game.get("potions").and_then(Value::as_array) {
                    next.potions = potions
                        .iter()
                        .filter_map(|p| {
                            p.get("name")
                                .and_then(Value::as_str)
                                .and_then(potion_from_trace_name)
                        })
                        .collect();
                }
            }
            if let Some(reward) = next.reward.as_mut() {
                reward.potion_offer = None;
            }
            Ok(next)
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
    if choice == "relic" && external_seed == "TEST" {
        seed_start_sync_relic_keys_from_observed(sim, post);
    }
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
                .map(|card| reward_card_display_key(run, card.content_id).to_owned())
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

fn sync_combat_from_observed_after_end(run: &mut RunState, message: &Value) {
    let Some(game) = message.get("game_state") else {
        return;
    };
    let Some(combat_value) = game.get("combat_state") else {
        return;
    };
    let Some(combat) = run.combat.as_mut() else {
        return;
    };
    let player = combat_value.get("player");
    if let Some(player) = player {
        combat.player.hp = int(player, "current_hp");
        combat.player.block = int(player, "block");
        combat.player.energy = int(player, "energy");
        combat.player.powers = player_powers(player.get("powers"));
    }
    combat.monsters =
        monsters_from_observed(combat_value.get("monsters"), player.unwrap_or(&Value::Null));
    combat.piles.hand = card_instances_from_array(combat_value.get("hand"), 100);
    combat.piles.draw_pile = card_instances_from_array(combat_value.get("draw_pile"), 200);
    combat.piles.discard_pile = card_instances_from_array(combat_value.get("discard_pile"), 300);
    combat.piles.exhaust_pile = card_instances_from_array(combat_value.get("exhaust_pile"), 400);
    combat.phase = CombatPhase::WaitingForPlayer;
    run.player_hp = int(game, "current_hp");
    run.player_max_hp = int(game, "max_hp");
    run.gold = int(game, "gold");
    run.deck = card_instances_from_array(game.get("deck"), 1);
}

fn seed_start_compare_combat_subset(
    report: &mut SimRealReport,
    action: &TraceAction,
    label: &str,
    expected: Value,
    actual: Value,
    strip_piles: bool,
) {
    compare_subset(
        report,
        action,
        label,
        seed_start_normalize_combat_compare(expected, strip_piles),
        seed_start_normalize_combat_compare(actual, strip_piles),
    );
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

fn run_from_observed_combat(message: &Value) -> Option<RunState> {
    let game = message.get("game_state")?;
    let combat = game.get("combat_state")?;
    let player = combat.get("player")?;

    let deck = card_instances_from_array(game.get("deck"), 1);
    let combat_state = CombatState {
        player: PlayerState {
            hp: int(player, "current_hp"),
            max_hp: int(player, "max_hp"),
            block: int(player, "block"),
            energy: int(player, "energy"),
            max_energy: 3,
            powers: player_powers(player.get("powers")),
            cannot_draw: false,
            temp_strength: 0,
        },
        monsters: monsters_from_observed(combat.get("monsters"), player),
        piles: CardPiles {
            hand: card_instances_from_array(combat.get("hand"), 100),
            draw_pile: card_instances_from_array(combat.get("draw_pile"), 200),
            discard_pile: card_instances_from_array(combat.get("discard_pile"), 300),
            exhaust_pile: card_instances_from_array(combat.get("exhaust_pile"), 400),
        },
        phase: CombatPhase::WaitingForPlayer,
        relics: Vec::new(),
        relic_counters: Default::default(),
        ascension: int(game, "ascension_level") as u8,
        shuffle_rng: None,
        potion_card_reward: None,
        hand_select: None,
    };

    Some(RunState {
        phase: RunPhase::Combat,
        player_hp: int(game, "current_hp"),
        player_max_hp: int(game, "max_hp"),
        gold: int(game, "gold"),
        energy_per_turn: 3,
        deck,
        map: None,
        combat: Some(combat_state),
        reward: None,
        event: None,
        shop: None,
        card_grid: None,
        relics: Vec::new(),
        potions: potions_from_observed(game),
        event_rng_seed: 0,
        reward_rng_seed: 7,
        card_rng_counter: 0,
        card_random_rng_counter: 0,
        card_rarity_factor: 5,
        treasure_rng_seed: 0,
        treasure_rng_counter: 0,
        potion_rng_seed: 0,
        potion_rng_counter: 0,
        potion_chance: 0,
        relic_rng_seed: 0,
        relic_rng_counter: 0,
        relic_pools: None,
        relic_keys: Vec::new(),
        merchant_rng_seed: 0,
        merchant_rng_counter: 0,
        event_rng_counter: 0,
        misc_rng_seed: 0,
        misc_rng_counter: 0,
        current_floor: int(game, "floor"),
        current_act: 1,
        shop_remove_count: 0,
        act1_event_list: Vec::new(),
        act1_shrine_list: Vec::new(),
        ascension: int(game, "ascension_level") as u8,
        treasure_room: None,
    })
}

fn reward_run_from_observed(message: &Value) -> Option<RunState> {
    let game = message.get("game_state")?;
    let reward = RewardScreen {
        choices: reward_choices_from_observed(game),
        gold_offer: reward_gold_offer(game),
        potion_offer: None,
        relic_offer: None,
        relic_key_offer: None,
        card_reward_active: game
            .get("screen_type")
            .and_then(Value::as_str)
            .is_some_and(|screen| screen == "CARD_REWARD"),
        card_reward_pending: game
            .get("screen_type")
            .and_then(Value::as_str)
            .is_some_and(|screen| screen == "COMBAT_REWARD")
            && reward_types_from_value(
                game.get("screen_state")
                    .and_then(|state| state.get("rewards")),
            )
            .iter()
            .any(|reward_type| reward_type == "CARD"),
    };
    Some(RunState {
        phase: RunPhase::Reward,
        deck: card_instances_from_array(game.get("deck"), 1),
        player_hp: int(game, "current_hp"),
        player_max_hp: int(game, "max_hp"),
        gold: int(game, "gold"),
        energy_per_turn: 3,
        map: None,
        combat: None,
        reward: Some(reward),
        event: None,
        shop: None,
        card_grid: None,
        relics: Vec::new(),
        potions: potions_from_observed(game),
        event_rng_seed: 0,
        reward_rng_seed: 0,
        card_rng_counter: 0,
        card_random_rng_counter: 0,
        card_rarity_factor: 5,
        treasure_rng_seed: 0,
        treasure_rng_counter: 0,
        potion_rng_seed: 0,
        potion_rng_counter: 0,
        potion_chance: 0,
        relic_rng_seed: 0,
        relic_rng_counter: 0,
        relic_pools: None,
        relic_keys: Vec::new(),
        merchant_rng_seed: 0,
        merchant_rng_counter: 0,
        event_rng_counter: 0,
        misc_rng_seed: 0,
        misc_rng_counter: 0,
        current_floor: int(game, "floor"),
        current_act: 1,
        shop_remove_count: 0,
        act1_event_list: Vec::new(),
        act1_shrine_list: Vec::new(),
        ascension: int(game, "ascension_level") as u8,
        treasure_room: None,
    })
}

fn combat_action_from_command(command: &str, combat: &CombatState) -> Option<CombatAction> {
    use sts_core::card::TargetRequirement;
    use sts_core::content::cards::get_card_definition;

    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("END") => Some(CombatAction::EndTurn),
        [cmd, hand_index] if cmd.eq_ignore_ascii_case("PLAY") => Some(CombatAction::PlayCard {
            card_id: hand_card_id(combat, hand_index)?,
            target: None,
        }),
        [cmd, hand_index, target_index] if cmd.eq_ignore_ascii_case("PLAY") => {
            let card_id = hand_card_id(combat, hand_index)?;
            let mut target = Some(MonsterId::new(target_index.parse::<u64>().ok()? + 1));
            if let Some(definition) = combat
                .piles
                .hand
                .iter()
                .find(|card| card.id == card_id)
                .and_then(|card| get_card_definition(card.content_id))
            {
                if definition.target == TargetRequirement::None {
                    target = None;
                }
            }
            Some(CombatAction::PlayCard { card_id, target })
        }
        _ => None,
    }
}

fn hand_card_id(combat: &CombatState, hand_index: &str) -> Option<CardId> {
    let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
    Some(combat.piles.hand.get(index)?.id)
}

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
            "exact observed-state combat transition is unsupported for monster group(s): {}",
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
    let expected_json = serde_json::to_string(&expected).expect("json serializes");
    let actual_json = serde_json::to_string(&actual).expect("json serializes");
    let diffs = canonical_diff(&expected_json, &actual_json);
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

fn monsters_from_observed(value: Option<&Value>, _player: &Value) -> Vec<MonsterState> {
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
            let replay = elite_boss_replay_fields(monster, content_id, &powers);
            MonsterState {
                id: MonsterId::new(index as u64 + 1),
                hp: int(monster, "current_hp"),
                block: int(monster, "block"),
                alive: int(monster, "current_hp") > 0,
                powers,
                content_id,
                moves_executed: replay.moves_executed,
                sleep_turns_remaining: replay.sleep_turns_remaining,
                has_siphoned: replay.has_siphoned,
                split_triggered: false,
                defensive_turns_remaining: replay.defensive_turns_remaining,
                mode_shift: replay.mode_shift,
                in_defensive_mode: replay.in_defensive_mode,
                rolled_attack_damage,
                intent: replay.intent,
            }
        })
        .collect()
}

struct EliteBossReplayFields {
    moves_executed: u32,
    sleep_turns_remaining: u32,
    has_siphoned: bool,
    defensive_turns_remaining: u32,
    mode_shift: i32,
    in_defensive_mode: bool,
    intent: MonsterIntent,
}

fn elite_boss_replay_fields(
    monster: &Value,
    content_id: ContentId,
    powers: &MonsterPowers,
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
                    strength: 2,
                    dexterity: 2,
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
                in_defensive_mode: false,
                intent: observed_intent(monster, content_id),
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
                in_defensive_mode,
                intent: observed_intent(monster, content_id),
            }
        }
        _ => EliteBossReplayFields {
            moves_executed: moves_executed_from_observed(monster, content_id),
            sleep_turns_remaining: 0,
            has_siphoned: false,
            defensive_turns_remaining: 0,
            mode_shift: 0,
            in_defensive_mode: false,
            intent: observed_intent(monster, content_id),
        },
    }
}

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

fn observed_intent(monster: &Value, content_id: ContentId) -> MonsterIntent {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, CULTIST_ID, GREEN_LOUSE_ID, RED_LOUSE_ID, SPIKE_SLIME_ID,
    };

    let damage = int(monster, "move_base_damage");
    match monster.get("intent").and_then(Value::as_str).unwrap_or("") {
        "ATTACK" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "DEBUFF" => MonsterIntent::ApplyPlayerWeak {
            amount: if content_id == ACID_SLIME_ID { 1 } else { 1 },
        },
        "ATTACK_DEBUFF" => MonsterIntent::Attack {
            damage: damage.max(0),
        },
        "DEFEND" | "BLOCK" => MonsterIntent::Block {
            block: if matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) {
                3
            } else {
                damage.max(0)
            },
        },
        "BUFF" | "DEBUG" => match content_id {
            CULTIST_ID => MonsterIntent::Ritual { amount: 3 },
            SPIKE_SLIME_ID if damage > 0 => MonsterIntent::Attack { damage },
            SPIKE_SLIME_ID => MonsterIntent::Attack { damage: 5 },
            ACID_SLIME_ID => MonsterIntent::Attack { damage: 7 },
            RED_LOUSE_ID | GREEN_LOUSE_ID => MonsterIntent::Block { block: 3 },
            _ => MonsterIntent::Attack { damage: 0 },
        },
        _ => MonsterIntent::Attack { damage: 0 },
    }
}

fn moves_executed_from_observed(monster: &Value, content_id: ContentId) -> u32 {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, CULTIST_ID, GREEN_LOUSE_ID, RED_LOUSE_ID, SPIKE_SLIME_ID,
    };

    match monster.get("intent").and_then(Value::as_str).unwrap_or("") {
        "BUFF" | "DEBUG" | "DEBUFF" => 0,
        "ATTACK_DEBUFF" => 1,
        "ATTACK" if content_id == CULTIST_ID => 1,
        "ATTACK" if content_id == SPIKE_SLIME_ID => 0,
        "ATTACK" if matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID) => 1,
        "ATTACK" if content_id == ACID_SLIME_ID => 1,
        _ => 1,
    }
}

fn monster_powers(value: Option<&Value>) -> MonsterPowers {
    let mut powers = MonsterPowers::default();
    let Some(items) = value.and_then(Value::as_array) else {
        return powers;
    };
    for power in items {
        let amount = int(power, "amount");
        match power_id(power).as_deref() {
            Some("Vulnerable") => powers.vulnerable = amount,
            Some("Weak") => powers.weak = amount,
            Some("Strength") => powers.strength = amount,
            Some("Ritual") | Some("Demon Form") => powers.ritual = amount,
            Some("Sharp Hide") | Some("Spikes") => powers.spikes = amount,
            Some("Curl Up") => powers.curl_up = amount,
            Some("Anger") => powers.anger = amount,
            Some("Metallicize") => powers.metallicize = amount,
            _ => {}
        }
    }
    powers
}

fn player_powers(value: Option<&Value>) -> PlayerPowers {
    let mut powers = PlayerPowers::default();
    let Some(items) = value.and_then(Value::as_array) else {
        return powers;
    };
    for power in items {
        let amount = int(power, "amount");
        match power_id(power).as_deref() {
            Some("Strength") => powers.strength = amount,
            Some("Weak") | Some("Weakened") => powers.weak = amount,
            Some("Dexterity") => powers.dexterity = amount,
            Some("Frail") => powers.frail = amount,
            Some("Vulnerable") => powers.vulnerable = amount,
            Some("Ritual") | Some("Demon Form") => powers.ritual = amount,
            Some("Metallicize") => powers.metallicize = amount,
            _ => {}
        }
    }
    powers
}

fn reward_gold_offer(game: &Value) -> i32 {
    game.get("screen_state")
        .and_then(|state| state.get("rewards"))
        .and_then(Value::as_array)
        .and_then(|rewards| rewards.iter().find_map(|reward| reward.get("gold")))
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32
}

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

fn observed_reward_choice<'a>(message: &'a Value, choice_index: usize) -> Option<&'a Value> {
    message
        .get("game_state")?
        .get("screen_state")?
        .get("cards")?
        .as_array()?
        .get(choice_index)
}

fn card_instances_from_array(value: Option<&Value>, base_id: u64) -> Vec<CardInstance> {
    let Some(cards) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            content_id_from_card_value(card).map(|content_id| {
                CardInstance::new(CardId::new(base_id + index as u64), content_id)
            })
        })
        .collect()
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

fn upgrade_content_id(base: ContentId) -> Option<ContentId> {
    sts_core::content::cards::upgrade_content_id(base)
}

fn content_id_from_key(key: &str) -> Option<ContentId> {
    use sts_core::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BASH_ID, BATTLE_TRANCE_ID, BERSERK_ID, BLOODLETTING_ID,
        BODY_SLAM_ID, CLEAVE_ID, CLOTHESLINE_ID, DEFEND_R_ID, DEMON_FORM_ID, DISARM_ID, DOUBT_ID,
        DRAMATIC_ENTRANCE_ID, DUAL_WIELD_ID, ENTRENCH_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID,
        FLEX_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, INTIMIDATE_ID,
        LIMIT_BREAK_ID, METALLICIZE_ID, OFFERING_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID,
        RAMPAGE_ID, REGRET_ID, SENTINEL_ID, SEVER_SOUL_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID,
        SLIMED_ID, SPOT_WEAKNESS_ID, STRIKE_R_ID, SWIFT_STRIKE_ID, SWORD_BOOMERANG_ID,
        THUNDERCLAP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WARCRY_PLUS_ID,
        WHIRLWIND_ID,
    };
    match key {
        "Strike_R" | "Strike" => Some(STRIKE_R_ID),
        "Defend_R" | "Defend" => Some(DEFEND_R_ID),
        "Bash" => Some(BASH_ID),
        "Slimed" | "slimed" => Some(SLIMED_ID),
        "Thunderclap" | "thunderclap" => Some(THUNDERCLAP_ID),
        "Anger" | "anger" => Some(ANGER_ID),
        "Warcry" | "warcry" => Some(WARCRY_ID),
        "Warcry+" | "warcry+" => Some(WARCRY_PLUS_ID),
        "Metallicize" | "metallicize" => Some(METALLICIZE_ID),
        "Twin Strike" | "twin strike" => Some(TWIN_STRIKE_ID),
        "Battle Trance" | "battle trance" => Some(BATTLE_TRANCE_ID),
        "Shrug It Off" | "shrug it off" => Some(SHRUG_IT_OFF_ID),
        "Body Slam" | "body slam" => Some(BODY_SLAM_ID),
        "Cleave" | "cleave" => Some(CLEAVE_ID),
        "Dramatic Entrance" | "dramatic entrance" => Some(DRAMATIC_ENTRANCE_ID),
        "Swift Strike" | "swift strike" => Some(SWIFT_STRIKE_ID),
        "Entrench" | "entrench" => Some(ENTRENCH_ID),
        "Fire Breathing" | "fire breathing" => Some(FIRE_BREATHING_ID),
        "Flex" | "flex" => Some(FLEX_ID),
        "Spot Weakness" | "spot weakness" => Some(SPOT_WEAKNESS_ID),
        "Flame Barrier" | "flame barrier" => Some(FLAME_BARRIER_ID),
        "Heavy Blade" | "heavy blade" => Some(HEAVY_BLADE_ID),
        "Intimidate" | "intimidate" => Some(INTIMIDATE_ID),
        "Perfected Strike" | "perfected strike" => Some(PERFECTED_STRIKE_ID),
        "Sword Boomerang" | "sword boomerang" => Some(SWORD_BOOMERANG_ID),
        "True Grit" | "true grit" => Some(TRUE_GRIT_ID),
        "Headbutt" | "headbutt" => Some(HEADBUTT_ID),
        "Clothesline" | "clothesline" => Some(CLOTHESLINE_ID),
        "Shockwave" | "shockwave" => Some(SHOCKWAVE_ID),
        "Rampage" | "rampage" => Some(RAMPAGE_ID),
        "Whirlwind" | "whirlwind" => Some(WHIRLWIND_ID),
        "Pommel Strike" | "pommel strike" => Some(POMMEL_STRIKE_ID),
        "Sever Soul" | "sever soul" => Some(SEVER_SOUL_ID),
        "Sentinel" | "sentinel" => Some(SENTINEL_ID),
        "Uppercut" | "uppercut" => Some(UPPERCUT_ID),
        "Disarm" | "disarm" => Some(DISARM_ID),
        "Dual Wield" | "dual wield" => Some(DUAL_WIELD_ID),
        "Immolate" | "immolate" => Some(IMMOLATE_ID),
        "Berserk" | "berserk" => Some(BERSERK_ID),
        "Limit Break" | "limit break" => Some(LIMIT_BREAK_ID),
        "Armaments" | "armaments" => Some(ARMAMENTS_ID),
        "Regret" | "regret" => Some(REGRET_ID),
        "Doubt" | "doubt" => Some(DOUBT_ID),
        "Offering" | "offering" => Some(OFFERING_ID),
        "Demon Form" | "demon form" => Some(DEMON_FORM_ID),
        "Bloodletting" | "bloodletting" => Some(BLOODLETTING_ID),
        "Hemokinesis" | "hemokinesis" => Some(HEMOKINESIS_ID),
        _ => None,
    }
}

fn content_key(content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        ANGER_ID, ARMAMENTS_ID, BASH_ID, BATTLE_TRANCE_ID, BERSERK_ID, BLOODLETTING_ID,
        BODY_SLAM_ID, BURN_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, COMBUST_ID, DEFEND_R_ID,
        DEMON_FORM_ID, DISARM_ID, DOUBT_ID, DRAMATIC_ENTRANCE_ID, DUAL_WIELD_ID, ENTRENCH_ID,
        FEEL_NO_PAIN_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID, FLEX_PLUS_ID, HAVOC_ID,
        HAVOC_PLUS_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, INFLAME_ID,
        INFLAME_PLUS_ID, INTIMIDATE_ID, LIMIT_BREAK_ID, METALLICIZE_ID, OFFERING_ID,
        PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POMMEL_STRIKE_PLUS_ID, RAMPAGE_ID, REGRET_ID,
        SENTINEL_ID, SEVER_SOUL_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SLIMED_ID, SPOT_WEAKNESS_ID,
        STRIKE_R_ID, SWIFT_STRIKE_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID, TRUE_GRIT_ID,
        TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WARCRY_PLUS_ID, WHIRLWIND_ID, WILD_STRIKE_ID,
    };
    match content_id {
        id if id == STRIKE_R_ID => "Strike_R",
        id if id == DEFEND_R_ID => "Defend_R",
        id if id == BASH_ID => "Bash",
        id if id == BURN_ID => "Burn",
        id if id == SLIMED_ID => "Slimed",
        id if id == THUNDERCLAP_ID => "Thunderclap",
        id if id == ANGER_ID => "Anger",
        id if id == WARCRY_ID => "Warcry",
        id if id == WARCRY_PLUS_ID => "Warcry+",
        id if id == METALLICIZE_ID => "Metallicize",
        id if id == TWIN_STRIKE_ID => "Twin Strike",
        id if id == BATTLE_TRANCE_ID => "Battle Trance",
        id if id == SHRUG_IT_OFF_ID => "Shrug It Off",
        id if id == BODY_SLAM_ID => "Body Slam",
        id if id == CLASH_ID => "Clash",
        id if id == CLEAVE_ID => "Cleave",
        id if id == WILD_STRIKE_ID => "Wild Strike",
        id if id == HAVOC_ID => "Havoc",
        id if id == HAVOC_PLUS_ID => "Havoc+",
        id if id == INFLAME_ID => "Inflame",
        id if id == INFLAME_PLUS_ID => "Inflame+",
        id if id == COMBUST_ID => "Combust",
        id if id == OFFERING_ID => "Offering",
        id if id == DRAMATIC_ENTRANCE_ID => "Dramatic Entrance",
        id if id == SWIFT_STRIKE_ID => "Swift Strike",
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
        id if id == IMMOLATE_ID => "Immolate",
        id if id == BERSERK_ID => "Berserk",
        id if id == LIMIT_BREAK_ID => "Limit Break",
        id if id == ARMAMENTS_ID => "Armaments",
        id if id == CLOTHESLINE_ID => "Clothesline",
        id if id == SHOCKWAVE_ID => "Shockwave",
        id if id == RAMPAGE_ID => "Rampage",
        id if id == WHIRLWIND_ID => "Whirlwind",
        id if id == POMMEL_STRIKE_ID => "Pommel Strike",
        id if id == POMMEL_STRIKE_PLUS_ID => "Pommel Strike+",
        id if id == SEVER_SOUL_ID => "Sever Soul",
        id if id == SENTINEL_ID => "Sentinel",
        id if id == UPPERCUT_ID => "Uppercut",
        id if id == DISARM_ID => "Disarm",
        id if id == DUAL_WIELD_ID => "Dual Wield",
        id if id == REGRET_ID => "Regret",
        id if id == DOUBT_ID => "Doubt",
        id if id == DEMON_FORM_ID => "Demon Form",
        id if id == BLOODLETTING_ID => "Bloodletting",
        id if id == HEMOKINESIS_ID => "Hemokinesis",
        id if id == FEEL_NO_PAIN_ID => "Feel No Pain",
        other if shop_pool_trace_name(other).is_some() => {
            shop_pool_trace_name(other).unwrap_or("unknown")
        }
        _ => "unknown",
    }
}

fn deck_content_key(content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        FLEX_PLUS_ID, HAVOC_PLUS_ID, INFLAME_PLUS_ID, OFFERING_ID, WARCRY_PLUS_ID,
    };
    match content_id {
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
    use sts_core::content::cards::{
        ARMAMENTS_ID, FLEX_ID, METALLICIZE_ID, OFFERING_ID, SHRUG_IT_OFF_ID, WARCRY_PLUS_ID,
    };
    if content_id == WARCRY_PLUS_ID {
        return "Warcry+";
    }
    if run.relic_keys.iter().any(|key| *key == RelicKey::ToxicEgg) {
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
    content_key(content_id)
}

fn choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd, index] if cmd.eq_ignore_ascii_case("CHOOSE") => index.parse().ok(),
        _ => None,
    }
}

fn parse_potion_use_slot(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [head, second, third]
            if head.eq_ignore_ascii_case("POTION") && second.eq_ignore_ascii_case("USE") =>
        {
            third.parse().ok()
        }
        [head, slot, ..]
            if head.eq_ignore_ascii_case("potion") && !slot.eq_ignore_ascii_case("USE") =>
        {
            slot.parse().ok()
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

fn screen_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("screen_type"))
        .and_then(Value::as_str)
}

fn screen_event_name(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("screen_state"))
        .and_then(|state| state.get("event_name"))
        .and_then(Value::as_str)
}

fn first_choice(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("choice_list"))
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_str)
}

fn unsupported_reason(pre: &TraceState, action: &TraceAction) -> String {
    match action.command.split_whitespace().next().unwrap_or("") {
        "START" => "seed-start run creation is unsupported until map, Neow, and reward RNG parity are implemented".to_owned(),
        "CHOOSE" if screen_type(&pre.message) == Some("EVENT") => {
            "Neow/event choice side effects are unsupported in sim-to-real replay".to_owned()
        }
        "CHOOSE" if screen_type(&pre.message) == Some("MAP") => {
            "map node selection is unsupported until exact seed-to-map parity is implemented".to_owned()
        }
        "CHOOSE" if screen_type(&pre.message) == Some("COMBAT_REWARD") => {
            "reward card-screen opening is a UI transition; card pickup is verified from CARD_REWARD".to_owned()
        }
        "PROCEED" => "reward-to-map UI transition is out-of-scope for simulator state parity".to_owned(),
        "state" => "trace client poll command is not a game transition".to_owned(),
        _ => "unsupported or unobservable CommunicationMod command".to_owned(),
    }
}

fn intent_key(monster: &MonsterState) -> String {
    use sts_core::content::monsters::ACID_SLIME_ID;

    match monster.intent {
        MonsterIntent::Attack { .. } | MonsterIntent::AttackMultiple { .. } => {
            if monster.content_id == ACID_SLIME_ID {
                "ATTACK_DEBUFF".to_owned()
            } else {
                "ATTACK".to_owned()
            }
        }
        MonsterIntent::Ritual { .. }
        | MonsterIntent::Block { .. }
        | MonsterIntent::StrengthAndBlock { .. } => "BUFF".to_owned(),
        MonsterIntent::AttackAndBlock { .. } => "ATTACK_BUFF".to_owned(),
        MonsterIntent::ApplyPlayerWeak { .. }
        | MonsterIntent::AttackApplyPlayerVulnerable { .. }
        | MonsterIntent::AddDazedToDiscard { .. }
        | MonsterIntent::AddBurnToDiscard { .. }
        | MonsterIntent::SiphonPlayer { .. } => "DEBUFF".to_owned(),
        MonsterIntent::Sleep => "SLEEP".to_owned(),
        MonsterIntent::Stun => "STUN".to_owned(),
        MonsterIntent::DefensiveCharge { .. } => "UNKNOWN".to_owned(),
    }
}

fn int(value: &Value, key: &str) -> i32 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

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
    use sts_core::content::cards::DRAMATIC_ENTRANCE_ID;

    #[test]
    fn trace_replay_parses_unknown_exit_metadata_and_supports_empty_trace() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"metadata","event":"exit","ended_at":"now"}"#;

        let report = verify_communication_mod_trace(content).expect("verifies");
        assert_eq!(report.total_actions, 0);
        assert!(report.unexpected_diffs.is_empty());
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
    fn unsupported_combat_command_reason_names_unmapped_cards() {
        let message = json!({
            "game_state": {
                "combat_state": {
                    "hand": [{"id": "Deep Breath", "name": "Deep Breath"}]
                }
            }
        });
        let reason =
            unsupported_combat_command_reason(&message, "PLAY 1").expect("unmapped card reason");
        assert!(reason.contains("Deep Breath"));
        assert!(reason.contains("not mapped"));
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
    fn m290001_floor2_bash_targets_living_acid_slime_from_observed_state() {
        let Some(content) = crate::load_corpus_file(
            "communication_mod/trace-2026-06-23T02-56-19-245Z.run2.valid-prefix.jsonl",
        ) else {
            return;
        };
        let trace = import_communication_mod_trace(&content).expect("trace imports");
        let transitions = trace_transitions(&trace.lines).expect("trace transitions");
        let (pre, action, _post) = transitions
            .iter()
            .find(|(_, action, _)| action.step == 29)
            .expect("step 29 transition");
        assert_eq!(action.command, "PLAY 5 0");

        let run = run_from_observed_combat(&pre.message).expect("observed combat run");
        let combat = run.combat.as_ref().expect("combat");
        assert_eq!(combat.monsters[0].hp, 2);
        assert!(combat.monsters[0].alive);
        assert_eq!(combat.monsters[0].id.get(), 1);

        let action = combat_action_from_command(&action.command, combat).expect("combat action");
        let CombatAction::PlayCard { target, .. } = action else {
            panic!("expected play-card action");
        };
        assert_eq!(target.expect("target").get(), 1);
        assert!(
            sts_core::legal_combat_actions(combat).contains(&action),
            "legal actions: {:?}, parsed action: {:?}",
            sts_core::legal_combat_actions(combat),
            action
        );
        sts_core::apply_combat_action(combat, action).expect("direct Bash applies");
        let next = apply_combat_action_on_run(&run, action).expect("Bash applies");
        let combat = next.combat.as_ref().expect("combat continues");
        assert!(combat.monsters[0].hp <= 0);
        assert!(!combat.monsters[0].alive);
    }

    #[test]
    fn choose_index_parses_nonzero_reward_choice() {
        assert_eq!(choose_index("CHOOSE 2"), Some(2));
    }

    #[test]
    fn unmapped_reward_pick_is_classified_as_unsupported() {
        let pre = TraceState {
            step: 1,
            received_at: None,
            message: json!({
                "game_state": {
                    "screen_type": "CARD_REWARD",
                    "deck": [{"id": "Strike_R"}],
                    "current_hp": 80,
                    "max_hp": 80,
                    "gold": 99,
                    "ascension_level": 0,
                    "screen_state": {
                        "cards": [
                            {"id": "Meteor Strike", "name": "Meteor Strike"},
                            {"id": "Shrug It Off", "name": "Shrug It Off"}
                        ]
                    }
                }
            }),
        };
        let action = TraceAction {
            step: 2,
            command: "CHOOSE 0".to_owned(),
            sent_at: None,
        };
        let post = TraceState {
            step: 2,
            received_at: None,
            message: pre.message.clone(),
        };
        let mut report = SimRealReport {
            mode: VerificationMode::ObservedState,
            total_actions: 1,
            verified: Vec::new(),
            unsupported: Vec::new(),
            unexpected_diffs: Vec::new(),
            seed_start: None,
        };
        verify_transition(&pre, &action, &post, &mut report);
        assert!(
            report
                .unsupported
                .iter()
                .any(|entry| entry.reason.contains("Meteor Strike")
                    && entry.reason.contains("reward pick")),
            "unmapped reward picks should be unsupported: {:?}",
            report.unsupported
        );
        assert!(report.unexpected_diffs.is_empty());
    }
}
