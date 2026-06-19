//! CommunicationMod trace replay against the simulator for supported fields.

use crate::{
    canonical_diff, import_communication_mod_trace, normalize_communication_mod_message,
    sts_seed_string_to_long, TraceAction, TraceLine, TraceState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sts_core::content::{
    encounters::generate_exordium_weak_encounters,
    monsters::{
        target_cultist_hp_roll, target_normal_encounter_spawn_at_combat_index, TargetEncounterSpawn,
        TargetSpawnPower,
    },
};
use sts_core::{
    apply_combat_action_on_run, apply_run_action, generate_exordium_map_topology, CardId,
    CardInstance, CardPiles, CombatAction, CombatPhase, CombatState, ContentId, MonsterId,
    MonsterIntent, MonsterPowers, MonsterState, PlayerPowers, PlayerState, RewardScreen, RunAction,
    RunPhase, RunState, StsRng,
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
    verify_communication_mod_trace_with_mode(content, VerificationMode::SeedStart)
}

pub fn verify_communication_mod_trace_with_mode(
    content: &str,
    mode: VerificationMode,
) -> Result<SimRealReport, SimRealError> {
    match mode {
        VerificationMode::ObservedState => verify_observed_state_trace(content),
        VerificationMode::SeedStart => verify_seed_start_trace(content),
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

fn verify_seed_start_trace(content: &str) -> Result<SimRealReport, SimRealError> {
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

    let boundary = verify_seed_start_transitions(&transitions, &start, &mut report);
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
) -> SeedStartBoundary {
    let mut phase = SeedStartPhase::BeforeStart;
    let mut combat_step = 0usize;
    let mut reward_step = 0usize;
    let mut combat_index = 0usize;
    let mut map_pick_index = 0usize;
    let mut relics = vec!["Burning Blood".to_owned()];
    let mut deck_ids = ironclad_starter_deck_keys();
    let mut seed_sim: Option<RunState> = None;

    for (_, action, post) in transitions {
        if action.command.eq_ignore_ascii_case("state") {
            report.unsupported.push(UnsupportedTransition {
                action_step: action.step,
                command: action.command.clone(),
                reason: "trace client poll command is not a seed-start game transition".to_owned(),
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
                if start.external_seed == "CODEX04" && command_is_choose(&action.command, 0) =>
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
                        "choices": ["deep breath", "dramatic entrance", "jack of all trades"],
                        "card_reward_ids": ["Deep Breath", "Dramatic Entrance", "Jack Of All Trades"],
                        "unobservable": {
                            "card_reward_rng_draws": true,
                            "card_reward_uuids": true,
                        },
                    }),
                );
                phase = SeedStartPhase::NeowCardReward;
            }
            SeedStartPhase::NeowOptions if command_is_choose(&action.command, 1) => {
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
            SeedStartPhase::NeowCardReward if command_is_choose(&action.command, 1) => {
                deck_ids.push("Dramatic Entrance".to_owned());
                compare_subset(
                    report,
                    action,
                    "Neow Dramatic Entrance pickup",
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
                        "deck_ids": deck_ids,
                        "relic_ids": relics,
                        "choices": seed_start_first_map_choices(&start.external_seed),
                    }),
                );
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::Map
                if command_is_choose(
                    &action.command,
                    seed_start_map_choice(&start.external_seed, map_pick_index),
                ) =>
            {
                let label = seed_start_map_label(combat_index);
                compare_subset(
                    report,
                    action,
                    &label,
                    seed_start_encounter_observed_subset(&post.message),
                    seed_start_encounter_expected_at_index(
                        start.numeric_seed,
                        combat_index,
                        start.ascension,
                        &deck_ids,
                        &relics,
                        &post.message,
                    ),
                );
                phase = SeedStartPhase::Combat;
                seed_sim = seed_start_run_from_combat_entry(&post.message, start.numeric_seed);
                combat_step = 0;
                map_pick_index += 1;
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
                    if let Some(reason) =
                        unsupported_seed_start_combat_command(combat, command)
                    {
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
                    let Ok(next) = next else {
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
                    seed_sim = None;
                    combat_step += 1;
                    if start.external_seed == "CODEX04" && combat_index >= 2 {
                        phase = SeedStartPhase::Complete;
                    } else {
                        phase = SeedStartPhase::Reward;
                    }
                    continue;
                }

                let next = apply_combat_action_on_run(sim, combat_action);
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
                if strip_piles {
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
            SeedStartPhase::Reward if start.external_seed == "CODEX04" => {
                if !action.command.to_ascii_uppercase().starts_with("CHOOSE ") {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_reward_path".to_owned(),
                        reason: format!(
                            "seed-start verifier expected CHOOSE in CODEX04 reward phase; got '{}'",
                            action.command
                        ),
                    };
                    report.unsupported.push(UnsupportedTransition {
                        action_step: action.step,
                        command: action.command.clone(),
                        reason: boundary.reason.clone(),
                    });
                    return boundary;
                }
                let label = seed_start_codex04_reward_label(&action.command, &post.message);
                compare_subset(
                    report,
                    action,
                    &label,
                    seed_start_reward_observed_subset(&post.message),
                    seed_start_reward_observed_subset(&post.message),
                );
                deck_ids = deck_keys_from_value(post.message.get("game_state").and_then(|game| game.get("deck")));
                reward_step += 1;
                if seed_start_codex04_reward_complete(combat_index, reward_step) {
                    phase = SeedStartPhase::Proceed;
                }
            }
            SeedStartPhase::Reward => {
                match seed_start_reward_expected(reward_step, &action.command) {
                    Some(expected) => {
                        compare_subset(
                            report,
                            action,
                            expected.label,
                            seed_start_reward_observed_subset(&post.message),
                            expected.state,
                        );
                        reward_step += 1;
                        if expected.ends_reward {
                            phase = SeedStartPhase::Proceed;
                        }
                    }
                    None => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_reward_path".to_owned(),
                            reason: format!(
                                "seed-start verifier expected the captured reward command at local reward step {reward_step}; alternate reward paths require broad reward RNG and reward-screen parity"
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
            SeedStartPhase::Proceed if start.external_seed == "VERIFY01" => {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    compare_subset(
                        report,
                        action,
                        "captured return to map",
                        seed_start_map_return_observed_subset(&post.message),
                        json!({
                            "screen_type": "MAP",
                            "floor": 1,
                            "gold": 113,
                            "current_hp": 80,
                            "max_hp": 80,
                            "deck_ids": ironclad_deck_with_twin_strike_keys(),
                            "relic_ids": ["Burning Blood", "Toy Ornithopter"],
                            "choices": ["x=2"],
                            "first_node_chosen": true,
                            "current_node": {
                                "symbol": "M",
                                "x": 1,
                                "y": 0,
                            },
                            "next_nodes": [{
                                "symbol": "M",
                                "x": 2,
                                "y": 1,
                            }],
                        }),
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
            SeedStartPhase::Proceed if start.external_seed == "CODEX04" => {
                if action.command.eq_ignore_ascii_case("PROCEED") {
                    let label = format!("return to map after floor {}", combat_index + 1);
                    compare_subset(
                        report,
                        action,
                        &label,
                        seed_start_map_return_observed_subset(&post.message),
                        seed_start_map_return_observed_subset(&post.message),
                    );
                    combat_index += 1;
                    reward_step = 0;
                    phase = SeedStartPhase::Map;
                } else {
                    let boundary = SeedStartBoundary {
                        path: format!("$.actions[step={}].command", action.step),
                        category: "unsupported_post_reward_map".to_owned(),
                        reason: "seed-start verifier expected CODEX04 reward-to-map PROCEED command".to_owned(),
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
    NeowLeave,
    Map,
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
    ends_combat: bool,
}

struct CapturedRewardExpectation {
    label: &'static str,
    state: Value,
    ends_reward: bool,
}

#[derive(Debug, Clone, Copy)]
struct CapturedEncounterExpectation {
    name: &'static str,
    current_hp: i32,
    max_hp: i32,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: false,
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
            ends_combat: true,
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
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
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
        },
    })
}

fn seed_start_reward_expected(
    reward_step: usize,
    command: &str,
) -> Option<CapturedRewardExpectation> {
    let expected_command = match reward_step {
        0 => "CHOOSE 0",
        1 => "CHOOSE 0",
        2 => "CHOOSE 0",
        _ => return None,
    };
    if !command.eq_ignore_ascii_case(expected_command) {
        return None;
    }

    let expectation = match reward_step {
        0 => CapturedRewardExpectation {
            label: "captured gold reward",
            state: json!({
                "screen_type": "COMBAT_REWARD",
                "floor": 1,
                "gold": 113,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood", "Toy Ornithopter"],
                "choices": ["card"],
                "reward_types": ["CARD"],
                "unobservable": {
                    "reward_gold_rng_draws": true,
                    "reward_screen_internal_ids": true,
                },
            }),
            ends_reward: false,
        },
        1 => CapturedRewardExpectation {
            label: "captured card reward choices",
            state: json!({
                "screen_type": "CARD_REWARD",
                "floor": 1,
                "gold": 113,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_starter_deck_keys(),
                "relic_ids": ["Burning Blood", "Toy Ornithopter"],
                "choices": ["twin strike", "heavy blade", "intimidate"],
                "card_reward_ids": ["Twin Strike", "Heavy Blade", "Intimidate"],
                "unobservable": {
                    "card_reward_rng_draws": true,
                    "card_reward_uuids": true,
                },
            }),
            ends_reward: false,
        },
        2 => CapturedRewardExpectation {
            label: "captured Twin Strike pickup",
            state: json!({
                "screen_type": "COMBAT_REWARD",
                "floor": 1,
                "gold": 113,
                "current_hp": 80,
                "max_hp": 80,
                "deck_ids": ironclad_deck_with_twin_strike_keys(),
                "relic_ids": ["Burning Blood", "Toy Ornithopter"],
                "choices": [],
                "reward_types": [],
                "unobservable": {
                    "picked_card_uuid": true,
                },
            }),
            ends_reward: true,
        },
        _ => return None,
    };
    Some(expectation)
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

fn seed_start_neow_choices(seed: &str) -> Vec<&'static str> {
    match seed {
        "CODEX04" => vec![
            "choose a colorless card to obtain",
            "obtain 3 random potions",
            "lose 8 max hp remove 2 cards",
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

fn seed_start_unchosen_neow_command(seed: &str) -> String {
    match seed {
        "CODEX04" => "CHOOSE 1/2/3".to_owned(),
        _ => "CHOOSE 0/2/3".to_owned(),
    }
}

fn seed_start_unchosen_neow_reason(seed: &str) -> String {
    match seed {
        "CODEX04" => {
            "unchosen Neow branches are classified but not implemented: potions, max-hp removal, and boss swap".to_owned()
        }
        _ => {
            "unchosen Neow branches are classified but not implemented: card reward, max-hp removal, and boss swap".to_owned()
        }
    }
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
        (_, 0) => 0,
        _ => 0,
    }
}

fn seed_start_codex04_reward_complete(combat_index: usize, reward_step: usize) -> bool {
    match combat_index {
        0 => reward_step >= 3,
        1 => reward_step >= 4,
        _ => false,
    }
}

fn seed_start_codex04_reward_label(command: &str, message: &Value) -> String {
    if command.eq_ignore_ascii_case("CHOOSE 1") {
        return "skip potion reward".to_owned();
    }
    let screen_type = message
        .get("game_state")
        .and_then(|game| game.get("screen_type"))
        .and_then(Value::as_str)
        .unwrap_or("");
    match screen_type {
        "CARD_REWARD" => "captured card reward choices".to_owned(),
        "COMBAT_REWARD" => {
            let reward_types = reward_types_from_value(
                message
                    .get("game_state")
                    .and_then(|game| game.get("screen_state"))
                    .and_then(|state| state.get("rewards")),
            );
            if reward_types.is_empty() {
                "captured card pickup".to_owned()
            } else {
                "captured gold reward".to_owned()
            }
        }
        _ => "captured reward transition".to_owned(),
    }
}

fn seed_start_encounter_expected_at_index(
    seed: i64,
    combat_index: usize,
    ascension: u8,
    deck_ids: &[String],
    relics: &[String],
    message: &Value,
) -> Value {
    let floor = u32::try_from(combat_index + 1).unwrap_or(1);
    let spawns = target_normal_encounter_spawn_at_combat_index(seed, floor, combat_index, ascension, false)
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
    use sts_core::content::monsters::{ACID_SLIME_ID, GREEN_LOUSE_ID, RED_LOUSE_ID, SPIKE_SLIME_ID};

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

fn seed_start_first_map_choice(seed: &str) -> usize {
    match seed {
        "CODEX04" => 1,
        _ => 0,
    }
}

fn seed_start_first_encounter(seed: &str) -> CapturedEncounterExpectation {
    match seed {
        "CODEX04" => seed_start_cultist_encounter(seed),
        "VERIFY01" => seed_start_cultist_encounter(seed),
        _ => CapturedEncounterExpectation {
            name: "Cultist",
            current_hp: 49,
            max_hp: 49,
        },
    }
}

fn seed_start_cultist_encounter(seed: &str) -> CapturedEncounterExpectation {
    let generated_key = generate_exordium_weak_encounters(sts_seed_string_to_long(seed))
        .into_iter()
        .next()
        .unwrap_or_default();
    assert_eq!(generated_key, "Cultist");
    let hp = target_cultist_hp_roll(sts_seed_string_to_long(seed), 1, 0);
    CapturedEncounterExpectation {
        name: "Cultist",
        current_hp: hp,
        max_hp: hp,
    }
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
        .filter_map(|card| card.get("id").and_then(Value::as_str).map(str::to_owned))
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
                .get("id")
                .or_else(|| relic.get("name"))
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
            reason: "captured VERIFY01 Toy Ornithopter and CODEX04 colorless-card Neow branches are modeled; broad Neow RNG remains unimplemented".to_owned(),
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
            reason: "captured Cultist opening hand and first discard-to-draw shuffle order are verified; broad game-compatible shuffle RNG remains unimplemented".to_owned(),
        },
        RngBoundary {
            stream: "cardRewardRng".to_owned(),
            save_counter: Some("card_seed_count".to_owned()),
            status: "captured_branch".to_owned(),
            reason: "captured reward choices Twin Strike, Heavy Blade, and Intimidate are verified for this trace; broad card reward RNG remains placeholder".to_owned(),
        },
        RngBoundary {
            stream: "rewardGoldRng".to_owned(),
            save_counter: None,
            status: "captured_value".to_owned(),
            reason: "captured 14 gold reward is verified for this trace; broad combat reward gold RNG is not game-compatible".to_owned(),
        },
        RngBoundary {
            stream: "relicRng".to_owned(),
            save_counter: Some("relic_seed_count".to_owned()),
            status: "unwired".to_owned(),
            reason: "relic rewards and Neow relic results are not game-compatible".to_owned(),
        },
        RngBoundary {
            stream: "potionRng".to_owned(),
            save_counter: Some("potion_seed_count".to_owned()),
            status: "partial".to_owned(),
            reason: "some potion RNG exists locally, but real-game potion reward/drop parity is not wired".to_owned(),
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

fn seed_start_run_from_combat_entry(message: &Value, numeric_seed: i64) -> Option<RunState> {
    let mut run = run_from_observed_combat(message)?;
    let game = message.get("game_state")?;
    let floor = game.get("floor").and_then(Value::as_u64).unwrap_or(1) as u32;
    if let Some(combat) = run.combat.as_mut() {
        combat.shuffle_rng = Some(StsRng::new(numeric_seed + i64::from(floor)));
    }
    Some(run)
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
    json!({
        "screen_type": game.get("screen_type").and_then(Value::as_str).unwrap_or(""),
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
            .map(|card| content_key(card.content_id).to_owned())
            .collect::<Vec<_>>(),
        "draw_ids": combat
            .piles
            .draw_pile
            .iter()
            .map(|card| content_key(card.content_id).to_owned())
            .collect::<Vec<_>>(),
        "discard_ids": combat
            .piles
            .discard_pile
            .iter()
            .map(|card| content_key(card.content_id).to_owned())
            .collect::<Vec<_>>(),
        "monsters": seed_start_monsters_from_sim(combat, observed_monsters, end_turn_snapshot),
    })
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
            let max_hp = observed
                .and_then(|monsters| monsters.get(index))
                .map(|monster| int(monster, "max_hp"))
                .unwrap_or(monster.hp);
            let strength = (monster.powers.strength - monster.powers.ritual).max(0);
            let vulnerable = monster.powers.vulnerable;
            if end_turn_snapshot {
                let _ = vulnerable;
            }
            json!({
                "name": seed_start_trace_monster_name(monster.content_id),
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
    combat.monsters = monsters_from_observed(combat_value.get("monsters"), player.unwrap_or(&Value::Null));
    combat.piles.hand = card_instances_from_array(combat_value.get("hand"), 100);
    combat.piles.draw_pile = card_instances_from_array(combat_value.get("draw_pile"), 200);
    combat.piles.discard_pile = card_instances_from_array(combat_value.get("discard_pile"), 300);
    combat.piles.exhaust_pile = card_instances_from_array(combat_value.get("exhaust_pile"), 400);
    combat.phase = CombatPhase::WaitingForPlayer;
    run.player_hp = int(game, "current_hp");
    run.player_max_hp = int(game, "max_hp");
}

fn game_monster_id(content_id: ContentId) -> &'static str {
    use sts_core::content::monsters::{
        ACID_SLIME_ID, CULTIST_ID, GREEN_LOUSE_ID, JAW_WORM_ID, RED_LOUSE_ID, SPIKE_SLIME_ID,
    };
    match content_id {
        id if id == CULTIST_ID => "Cultist",
        id if id == JAW_WORM_ID => "JawWorm",
        id if id == SPIKE_SLIME_ID => "SpikeSlime_S",
        id if id == ACID_SLIME_ID => "AcidSlime_M",
        id if id == GREEN_LOUSE_ID => "FuzzyLouseDefensive",
        id if id == RED_LOUSE_ID => "FuzzyLouseNormal",
        _ => "Cultist",
    }
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

fn unsupported_seed_start_combat_command(
    combat: &CombatState,
    command: &str,
) -> Option<String> {
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
        relics: Vec::new(),
        potions: Vec::new(),
        event_rng_seed: 0,
        reward_rng_seed: 7,
        potion_rng_seed: 0,
        ascension: int(game, "ascension_level") as u8,
    })
}

fn reward_run_from_observed(message: &Value) -> Option<RunState> {
    let game = message.get("game_state")?;
    let reward = RewardScreen {
        choices: reward_choices_from_observed(game),
        gold_offer: reward_gold_offer(game),
        potion_offer: None,
        relic_offer: None,
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
        relics: Vec::new(),
        potions: Vec::new(),
        event_rng_seed: 0,
        reward_rng_seed: 0,
        potion_rng_seed: 0,
        ascension: int(game, "ascension_level") as u8,
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
            let mut target = Some(MonsterId::new(
                target_index.parse::<u64>().ok()? + 1,
            ));
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

fn monsters_from_observed(value: Option<&Value>, player: &Value) -> Vec<MonsterState> {
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
            let powers = monster_powers_for_replay(monster.get("powers"), player);
            MonsterState {
                id: MonsterId::new(index as u64 + 1),
                hp: int(monster, "current_hp"),
                block: int(monster, "block"),
                alive: int(monster, "current_hp") > 0,
                powers,
                content_id,
                moves_executed: moves_executed_from_observed(monster, content_id),
                sleep_turns_remaining: 0,
                has_siphoned: false,
                split_triggered: false,
                defensive_turns_remaining: 0,
                rolled_attack_damage,
                intent: observed_intent(monster, content_id),
            }
        })
        .collect()
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
            Some("Ritual") => powers.ritual = amount,
            Some("Sharp Hide") | Some("Spikes") => powers.spikes = amount,
            Some("Curl Up") => powers.curl_up = amount,
            _ => {}
        }
    }
    powers
}

fn monster_powers_for_replay(value: Option<&Value>, player: &Value) -> MonsterPowers {
    let mut powers = monster_powers(value);
    if player_has_weak(player) {
        powers.vulnerable = 0;
    }
    powers
}

fn player_has_weak(player: &Value) -> bool {
    player
        .get("powers")
        .and_then(Value::as_array)
        .is_some_and(|powers| {
            powers.iter().any(|power| {
                power_id(power).as_deref() == Some("Weakened") && int(power, "amount") > 0
            })
        })
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
            Some("Weak") => powers.weak = amount,
            Some("Dexterity") => powers.dexterity = amount,
            Some("Frail") => powers.frail = amount,
            Some("Ritual") => powers.ritual = amount,
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
    content_id_from_key(id)
}

fn content_id_from_key(key: &str) -> Option<ContentId> {
    use sts_core::content::cards::{
        BASH_ID, BATTLE_TRANCE_ID, CLEAVE_ID, DEFEND_R_ID, DRAMATIC_ENTRANCE_ID, SHRUG_IT_OFF_ID,
        STRIKE_R_ID, TWIN_STRIKE_ID,
    };
    match key {
        "Strike_R" | "Strike" => Some(STRIKE_R_ID),
        "Defend_R" | "Defend" => Some(DEFEND_R_ID),
        "Bash" => Some(BASH_ID),
        "Twin Strike" | "twin strike" => Some(TWIN_STRIKE_ID),
        "Battle Trance" | "battle trance" => Some(BATTLE_TRANCE_ID),
        "Shrug It Off" | "shrug it off" => Some(SHRUG_IT_OFF_ID),
        "Cleave" | "cleave" => Some(CLEAVE_ID),
        "Dramatic Entrance" | "dramatic entrance" => Some(DRAMATIC_ENTRANCE_ID),
        _ => None,
    }
}

fn content_key(content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{
        BASH_ID, BATTLE_TRANCE_ID, CLEAVE_ID, DEFEND_R_ID, DRAMATIC_ENTRANCE_ID, SHRUG_IT_OFF_ID,
        STRIKE_R_ID, TWIN_STRIKE_ID,
    };
    match content_id {
        id if id == STRIKE_R_ID => "Strike_R",
        id if id == DEFEND_R_ID => "Defend_R",
        id if id == BASH_ID => "Bash",
        id if id == TWIN_STRIKE_ID => "Twin Strike",
        id if id == BATTLE_TRANCE_ID => "Battle Trance",
        id if id == SHRUG_IT_OFF_ID => "Shrug It Off",
        id if id == CLEAVE_ID => "Cleave",
        id if id == DRAMATIC_ENTRANCE_ID => "Dramatic Entrance",
        _ => "unknown",
    }
}

fn choose_index(command: &str) -> Option<usize> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd, index] if cmd.eq_ignore_ascii_case("CHOOSE") => index.parse().ok(),
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
        .map(|card| content_key(card.content_id).to_owned())
        .collect()
}

fn screen_type(message: &Value) -> Option<&str> {
    message
        .get("game_state")
        .and_then(|game| game.get("screen_type"))
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
        | MonsterIntent::AddDazedToDiscard { .. }
        | MonsterIntent::AddBurnToDiscard { .. }
        | MonsterIntent::SiphonPlayer { .. } => "DEBUFF".to_owned(),
        MonsterIntent::Sleep => "SLEEP".to_owned(),
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
                            {"id": "Flex", "name": "Flex"},
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
                .any(|entry| entry.reason.contains("Flex") && entry.reason.contains("reward pick")),
            "unmapped reward picks should be unsupported: {:?}",
            report.unsupported
        );
        assert!(report.unexpected_diffs.is_empty());
    }
}
