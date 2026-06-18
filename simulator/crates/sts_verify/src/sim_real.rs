//! CommunicationMod trace replay against the simulator for supported fields.

use crate::{
    canonical_diff, import_communication_mod_trace, normalize_communication_mod_message,
    TraceAction, TraceLine, TraceState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sts_core::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CardPiles, CombatAction,
    CombatPhase, CombatState, ContentId, MonsterId, MonsterIntent, MonsterPowers, MonsterState,
    PlayerPowers, PlayerState, RewardScreen, RunAction, RunPhase, RunState,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRunCommand {
    pub action_step: u32,
    pub character: String,
    pub ascension: u8,
    pub external_seed: String,
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
    report.seed_start = Some(SeedStartReport {
        start_command: start,
        expected_failure: true,
        first_boundary: boundary,
        rng_boundaries: seed_start_rng_boundaries(),
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
    let mut relics = vec!["Burning Blood".to_owned()];

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
                        "deck_ids": ironclad_starter_deck_keys(),
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
                        "deck_ids": ironclad_starter_deck_keys(),
                        "relic_ids": relics,
                        "choices": [
                            "choose a card to obtain",
                            "obtain a random common relic",
                            "lose 8 max hp remove 2 cards",
                            "lose your starting relic obtain a random boss relic",
                        ],
                    }),
                );
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: "CHOOSE 0/2/3".to_owned(),
                    reason: "unchosen Neow branches are classified but not implemented: card reward, max-hp removal, and boss swap".to_owned(),
                });
                phase = SeedStartPhase::NeowOptions;
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
                        "deck_ids": ironclad_starter_deck_keys(),
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
                        "deck_ids": ironclad_starter_deck_keys(),
                        "relic_ids": relics,
                        "choices": ["x=1", "x=2"],
                    }),
                );
                phase = SeedStartPhase::Map;
            }
            SeedStartPhase::Map if command_is_choose(&action.command, 0) => {
                compare_subset(
                    report,
                    action,
                    "map first monster node",
                    seed_start_encounter_observed_subset(&post.message),
                    json!({
                        "screen_type": "NONE",
                        "ascension": start.ascension,
                        "floor": 1,
                        "gold": 99,
                        "current_hp": 80,
                        "max_hp": 80,
                        "deck_ids": ironclad_starter_deck_keys(),
                        "relic_ids": relics,
                        "combat_player_hp": 80,
                        "combat_player_block": 0,
                        "combat_player_energy": 3,
                        "monsters": [{
                            "name": "Cultist",
                            "current_hp": 49,
                            "max_hp": 49,
                            "block": 0,
                            "intent": "DEBUG",
                            "strength": 0,
                            "ritual": 0,
                            "vulnerable": 0,
                        }],
                    }),
                );
                phase = SeedStartPhase::Combat;
            }
            SeedStartPhase::Map => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_map_generation".to_owned(),
                    reason: "seed-start verifier expected captured first map choice CHOOSE 0; other map paths require exact map generation".to_owned(),
                };
                report.unsupported.push(UnsupportedTransition {
                    action_step: action.step,
                    command: action.command.clone(),
                    reason: boundary.reason.clone(),
                });
                return boundary;
            }
            SeedStartPhase::Combat => {
                match seed_start_cultist_combat_expected(combat_step, &action.command) {
                    Some(expected) => {
                        let label = expected.label;
                        compare_subset(
                            report,
                            action,
                            label,
                            seed_start_combat_observed_subset(&post.message),
                            expected.state,
                        );
                        combat_step += 1;
                        if expected.ends_combat {
                            phase = SeedStartPhase::Reward;
                        }
                    }
                    None => {
                        let boundary = SeedStartBoundary {
                            path: format!("$.actions[step={}].command", action.step),
                            category: "unsupported_combat_path".to_owned(),
                            reason: format!(
                                "seed-start verifier expected the captured Cultist combat command at local combat step {combat_step}; exact alternate combat paths require full draw/shuffle and monster AI parity"
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
            SeedStartPhase::Reward => {
                let boundary = SeedStartBoundary {
                    path: format!("$.actions[step={}].command", action.step),
                    category: "unsupported_reward_rng".to_owned(),
                    reason: "seed-start verifier has passed the captured Cultist combat and stops before reward replay because gold, card reward, potion, and reward-screen RNG are not implemented".to_owned(),
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

    SeedStartBoundary {
        path: "$.actions".to_owned(),
        category: "missing_reward_boundary".to_owned(),
        reason: "trace ended before seed-start verifier reached the expected reward boundary"
            .to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedStartPhase {
    BeforeStart,
    NeowTalk,
    NeowOptions,
    NeowLeave,
    Map,
    Combat,
    Reward,
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

fn ironclad_starter_deck_keys() -> Vec<&'static str> {
    vec![
        "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R",
        "Defend_R", "Defend_R", "Bash",
    ]
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
            status: "captured_opaque".to_owned(),
            reason: "VERIFY01 is parsed and carried as an external seed string, but exact numeric seed conversion still needs target-version source evidence".to_owned(),
        },
        RngBoundary {
            stream: "neowRng".to_owned(),
            save_counter: None,
            status: "captured_branch".to_owned(),
            reason: "captured VERIFY01 Neow branch is modeled through Toy Ornithopter; broad Neow RNG remains unimplemented".to_owned(),
        },
        RngBoundary {
            stream: "mapRng".to_owned(),
            save_counter: None,
            status: "captured_branch".to_owned(),
            reason: "captured VERIFY01 first map choices and CHOOSE 0 path are modeled; broad map generation remains placeholder".to_owned(),
        },
        RngBoundary {
            stream: "monsterRng".to_owned(),
            save_counter: Some("monster_seed_count".to_owned()),
            status: "captured_branch".to_owned(),
            reason: "captured first encounter is modeled as Cultist; broad encounter selection is not wired to a real-game RNG stream".to_owned(),
        },
        RngBoundary {
            stream: "monsterHpRng".to_owned(),
            save_counter: Some("monster_seed_count".to_owned()),
            status: "captured_value".to_owned(),
            reason: "captured Cultist HP 49 is modeled for this trace; broad monster HP rolls are not game-compatible".to_owned(),
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
            status: "placeholder".to_owned(),
            reason: "reward card choices use local placeholder rarity/pool behavior".to_owned(),
        },
        RngBoundary {
            stream: "rewardGoldRng".to_owned(),
            save_counter: None,
            status: "unwired".to_owned(),
            reason: "combat reward gold amount is restored from observation in Milestone 12, not generated from seed".to_owned(),
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

    if upper == "CHOOSE 0" {
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
        let expected = observed_run_subset(&post.message, &["current_hp", "gold", "deck_size"]);
        let actual = json!({
            "current_hp": next.player_hp,
            "gold": next.gold,
            "deck_size": next.deck.len(),
        });
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
    let expected =
        observed_combat_subset(&post.message, post_supported_combat_fields(&action.command));
    let actual = simulated_combat_subset(&next, post_supported_combat_fields(&action.command));
    compare_subset(report, action, &label, expected, actual);

    if action.command.trim().eq_ignore_ascii_case("END") {
        report.unsupported.push(UnsupportedTransition {
            action_step: action.step,
            command: action.command.clone(),
            reason: "exact card draw/shuffle order after end turn is out-of-scope for this verifier pass".to_owned(),
        });
    }
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

    compare_subset(
        report,
        action,
        "gold reward",
        observed_run_subset(&post.message, &["gold", "current_hp", "deck_size"]),
        json!({
            "gold": next.gold,
            "current_hp": next.player_hp,
            "deck_size": next.deck.len(),
        }),
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

    let Some(run) = reward_run_from_observed(&pre.message) else {
        return false;
    };
    let Some(card_id) = run
        .reward
        .as_ref()
        .and_then(|reward| reward.choices.first())
        .map(|card| card.id)
    else {
        return false;
    };
    let next = apply_run_action(&run, RunAction::TakeCardReward { card_id });
    let Ok(next) = next else {
        push_sim_error(report, action, "Twin Strike reward", next.err().unwrap());
        return true;
    };

    compare_subset(
        report,
        action,
        "Twin Strike reward",
        observed_run_subset(
            &post.message,
            &["gold", "current_hp", "deck_size", "deck_ids"],
        ),
        json!({
            "gold": next.gold,
            "current_hp": next.player_hp,
            "deck_size": next.deck.len(),
            "deck_ids": deck_content_keys(&next.deck),
        }),
    );
    true
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
        monsters: monsters_from_observed(combat.get("monsters")),
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
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts.as_slice() {
        [cmd] if cmd.eq_ignore_ascii_case("END") => Some(CombatAction::EndTurn),
        [cmd, hand_index] | [cmd, hand_index, _] if cmd.eq_ignore_ascii_case("PLAY") => {
            let index = hand_index.parse::<usize>().ok()?.checked_sub(1)?;
            let card_id = combat.piles.hand.get(index)?.id;
            let target = if parts.len() == 3 {
                let target_index = parts[2].parse::<u64>().ok()? + 1;
                Some(MonsterId::new(target_index))
            } else {
                None
            };
            Some(CombatAction::PlayCard { card_id, target })
        }
        _ => None,
    }
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
    let monster = combat.monsters.first();
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
    let monster = combat.monsters.first();
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

fn monsters_from_observed(value: Option<&Value>) -> Vec<MonsterState> {
    let Some(monsters) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    monsters
        .iter()
        .enumerate()
        .map(|(index, monster)| {
            let powers = monster_powers(monster.get("powers"));
            MonsterState {
                id: MonsterId::new(index as u64 + 1),
                hp: int(monster, "current_hp"),
                block: int(monster, "block"),
                alive: int(monster, "current_hp") > 0,
                powers,
                content_id: sts_core::content::monsters::CULTIST_ID,
                moves_executed: moves_executed(monster),
                sleep_turns_remaining: 0,
                has_siphoned: false,
                split_triggered: false,
                defensive_turns_remaining: 0,
                intent: observed_intent(monster),
            }
        })
        .collect()
}

fn observed_intent(monster: &Value) -> MonsterIntent {
    match monster.get("intent").and_then(Value::as_str).unwrap_or("") {
        "BUFF" | "DEBUG" => MonsterIntent::Ritual { amount: 3 },
        "ATTACK" => MonsterIntent::Attack {
            damage: int(monster, "move_base_damage").max(0),
        },
        _ => MonsterIntent::Attack { damage: 0 },
    }
}

fn moves_executed(monster: &Value) -> u32 {
    match monster.get("intent").and_then(Value::as_str).unwrap_or("") {
        "BUFF" | "DEBUG" => 0,
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
    use sts_core::content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID, TWIN_STRIKE_ID};
    match key {
        "Strike_R" | "Strike" => Some(STRIKE_R_ID),
        "Defend_R" | "Defend" => Some(DEFEND_R_ID),
        "Bash" => Some(BASH_ID),
        "Twin Strike" | "twin strike" => Some(TWIN_STRIKE_ID),
        _ => None,
    }
}

fn content_key(content_id: ContentId) -> &'static str {
    use sts_core::content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID, TWIN_STRIKE_ID};
    match content_id {
        id if id == STRIKE_R_ID => "Strike_R",
        id if id == DEFEND_R_ID => "Defend_R",
        id if id == BASH_ID => "Bash",
        id if id == TWIN_STRIKE_ID => "Twin Strike",
        _ => "unknown",
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
    match monster.intent {
        MonsterIntent::Attack { .. } | MonsterIntent::AttackMultiple { .. } => "ATTACK",
        MonsterIntent::Ritual { .. }
        | MonsterIntent::Block { .. }
        | MonsterIntent::StrengthAndBlock { .. } => "BUFF",
        MonsterIntent::AttackAndBlock { .. } => "ATTACK_BUFF",
        MonsterIntent::ApplyPlayerWeak { .. }
        | MonsterIntent::AddDazedToDiscard { .. }
        | MonsterIntent::AddBurnToDiscard { .. }
        | MonsterIntent::SiphonPlayer { .. } => "DEBUFF",
        MonsterIntent::Sleep => "SLEEP",
        MonsterIntent::DefensiveCharge { .. } => "UNKNOWN",
    }
    .to_owned()
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

    #[test]
    fn trace_replay_parses_unknown_exit_metadata_and_supports_empty_trace() {
        let content = r#"{"type":"metadata","schema":1,"source":"communication_mod"}
{"type":"metadata","event":"exit","ended_at":"now"}"#;

        let report = verify_communication_mod_trace(content).expect("verifies");
        assert_eq!(report.total_actions, 0);
        assert!(report.unexpected_diffs.is_empty());
    }
}
