use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, error::Error, fmt};
use sts_core::{
    apply_event_action, generate_neow_options, legal_event_actions, EventAction,
    GeneratedNeowOption, NeowDrawback, NeowRewardType, RunPhase, RunState,
};

use crate::sts_seed_string_to_long;

pub const SLAYTHEDATA_IMPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataRunImport {
    pub schema: u32,
    pub source: SlayTheDataSource,
    pub config: SlayTheDataRunConfig,
    pub replay_policy: SlayTheDataReplayPolicy,
    pub route: SlayTheDataRoute,
    pub floor_decisions: Vec<SlayTheDataFloorDecision>,
    pub boss_relic_choices: Vec<SlayTheDataBossRelicChoice>,
    pub final_observed: SlayTheDataFinalObserved,
    pub diagnostics: Vec<SlayTheDataDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataReplayPlan {
    pub schema: u32,
    pub source: SlayTheDataSource,
    pub run_start: Option<SlayTheDataRunStart>,
    pub ordering: SlayTheDataReplayOrdering,
    pub steps: Vec<SlayTheDataReplayStep>,
    pub checkpoints: Vec<SlayTheDataCheckpoint>,
    pub diagnostics: Vec<SlayTheDataDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataPreflightReport {
    pub schema: u32,
    pub source: SlayTheDataSource,
    pub run_start: Option<SlayTheDataRunStart>,
    pub numeric_seed: Option<i64>,
    pub start_phase: Option<String>,
    pub steps: Vec<SlayTheDataPreflightStep>,
    pub diagnostics: Vec<SlayTheDataDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataPreflightStep {
    pub floor: u32,
    pub ordinal: usize,
    pub status: SlayTheDataPreflightStatus,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataPreflightStatus {
    Checked,
    Guided,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataRunStart {
    pub character: String,
    pub ascension: i32,
    pub seed_played: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataReplayOrdering {
    FloorGrouped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataReplayStep {
    pub floor: u32,
    pub ordinal: usize,
    pub kind: SlayTheDataReplayStepKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlayTheDataReplayStepKind {
    NeowTalk,
    NeowBonus {
        bonus: Option<String>,
        cost: Option<String>,
    },
    NeowLeave,
    MapRoom {
        symbol: String,
    },
    CardReward {
        picked: Option<SlayTheDataCardName>,
        skipped: bool,
    },
    EventChoice {
        event_name: Option<String>,
        player_choice: Option<String>,
    },
    ShopPurchase {
        item: String,
        base_item: String,
    },
    Campfire {
        key: Option<String>,
        target_card: Option<SlayTheDataCardName>,
    },
    BossRelic {
        act: u32,
        picked: Option<String>,
    },
    PotionBudget {
        uses_allowed: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataCheckpoint {
    pub floor: Option<u32>,
    pub kind: SlayTheDataCheckpointKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlayTheDataCheckpointKind {
    RunStart,
    FloorStart {
        route: Option<String>,
    },
    FinalObserved {
        floor_reached: Option<i32>,
        victory: bool,
        deck_count: usize,
        relic_count: usize,
        gold: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataSource {
    pub kind: SlayTheDataSourceKind,
    pub run_id: Option<i64>,
    pub play_id: Option<String>,
    pub source_file: Option<String>,
    pub source_run_ordinal: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataSourceKind {
    ChunkExport,
    RawRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataRunConfig {
    pub character: Option<String>,
    pub ascension: Option<i32>,
    pub build_version: Option<String>,
    pub seed_played: Option<String>,
    pub seed_source_timestamp: Option<String>,
    pub special_seed: Option<String>,
    pub neow_bonus: Option<String>,
    pub neow_cost: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataReplayPolicy {
    pub mode: String,
    pub exact_combat_actions: bool,
    pub on_illegal_high_level_choice: String,
    pub on_legal_divergence: String,
    pub potion_budget_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataRoute {
    pub path_taken: Vec<String>,
    pub path_per_floor: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataFloorDecision {
    pub floor: u32,
    pub route: Option<String>,
    pub card_rewards: Vec<SlayTheDataCardReward>,
    pub relics_obtained: Vec<SlayTheDataNamedFloorItem>,
    pub events: Vec<SlayTheDataEventChoice>,
    pub shop_purchases: Vec<SlayTheDataShopPurchase>,
    pub campfires: Vec<SlayTheDataCampfireChoice>,
    pub potions: SlayTheDataPotionFloorDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataCardReward {
    pub ordinal: usize,
    pub picked: Option<SlayTheDataCardName>,
    pub not_picked: Vec<SlayTheDataCardName>,
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataCardName {
    pub raw: String,
    pub base: String,
    pub upgraded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataNamedFloorItem {
    pub ordinal: usize,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataEventChoice {
    pub ordinal: usize,
    pub event_name: Option<String>,
    pub player_choice: Option<String>,
    pub damage_taken: Option<i32>,
    pub damage_healed: Option<i32>,
    pub max_hp_gain: Option<i32>,
    pub max_hp_loss: Option<i32>,
    pub gold_gain: Option<i32>,
    pub gold_loss: Option<i32>,
    pub cards_obtained: Vec<SlayTheDataCardName>,
    pub cards_removed: Vec<SlayTheDataCardName>,
    pub cards_upgraded: Vec<SlayTheDataCardName>,
    pub relics_obtained: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataShopPurchase {
    pub ordinal: usize,
    pub item: String,
    pub base_item: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataCampfireChoice {
    pub ordinal: usize,
    pub key: Option<String>,
    pub data: Option<SlayTheDataCardName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SlayTheDataPotionFloorDecision {
    pub uses_allowed: u32,
    pub usage_ordinals: Vec<usize>,
    pub obtained: Vec<SlayTheDataNamedFloorItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataBossRelicChoice {
    pub act: u32,
    pub ordinal: usize,
    pub picked: Option<String>,
    pub not_picked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataFinalObserved {
    pub floor_reached: Option<i32>,
    pub victory: bool,
    pub master_deck: Vec<SlayTheDataCardName>,
    pub relics: Vec<String>,
    pub gold: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataDiagnostic {
    pub severity: SlayTheDataDiagnosticSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlayTheDataDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug)]
pub enum SlayTheDataImportError {
    Json(serde_json::Error),
    NotObject,
    JsonlLineOutOfRange {
        line_index: usize,
        line_count: usize,
    },
}

impl fmt::Display for SlayTheDataImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid SlayTheData JSON: {error}"),
            Self::NotObject => write!(f, "SlayTheData row must be a JSON object"),
            Self::JsonlLineOutOfRange {
                line_index,
                line_count,
            } => write!(
                f,
                "line_index {line_index} is outside {line_count} SlayTheData JSONL rows"
            ),
        }
    }
}

impl Error for SlayTheDataImportError {}

impl From<serde_json::Error> for SlayTheDataImportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn import_slaythedata_run_json(
    content: &str,
) -> Result<SlayTheDataRunImport, SlayTheDataImportError> {
    let value: Value = serde_json::from_str(content)?;
    import_slaythedata_run_value(&value)
}

pub fn import_slaythedata_jsonl_line(
    content: &str,
    line_index: usize,
) -> Result<SlayTheDataRunImport, SlayTheDataImportError> {
    let rows: Vec<&str> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let Some(row) = rows.get(line_index) else {
        return Err(SlayTheDataImportError::JsonlLineOutOfRange {
            line_index,
            line_count: rows.len(),
        });
    };
    import_slaythedata_run_json(row)
}

pub fn import_slaythedata_run_value(
    value: &Value,
) -> Result<SlayTheDataRunImport, SlayTheDataImportError> {
    let Some(root) = value.as_object() else {
        return Err(SlayTheDataImportError::NotObject);
    };
    let event = root
        .get("event")
        .and_then(Value::as_object)
        .map_or(root, |event| event);
    let source_kind = if root.contains_key("event") {
        SlayTheDataSourceKind::ChunkExport
    } else {
        SlayTheDataSourceKind::RawRun
    };

    let mut floors: BTreeMap<u32, SlayTheDataFloorDecision> = BTreeMap::new();
    import_card_rewards(event, &mut floors);
    import_relics_obtained(event, &mut floors);
    import_event_choices(event, &mut floors);
    import_shop_purchases(event, &mut floors);
    import_campfires(event, &mut floors);
    import_potions(event, &mut floors);
    import_route(event, &mut floors);

    let mut imported = SlayTheDataRunImport {
        schema: SLAYTHEDATA_IMPORT_SCHEMA_VERSION,
        source: SlayTheDataSource {
            kind: source_kind,
            run_id: parse_i64(root.get("run_id")),
            play_id: optional_string(event.get("play_id")),
            source_file: optional_string(root.get("source_file")),
            source_run_ordinal: parse_i64(root.get("source_run_ordinal")),
        },
        config: SlayTheDataRunConfig {
            character: optional_string(event.get("character_chosen")),
            ascension: parse_i32(event.get("ascension_level")),
            build_version: optional_string(event.get("build_version")),
            seed_played: optional_string(event.get("seed_played")),
            seed_source_timestamp: optional_string(event.get("seed_source_timestamp")),
            special_seed: optional_string(event.get("special_seed")),
            neow_bonus: optional_string(event.get("neow_bonus")),
            neow_cost: optional_string(event.get("neow_cost")),
        },
        replay_policy: SlayTheDataReplayPolicy {
            mode: "guided_slaythedata".to_owned(),
            exact_combat_actions: false,
            on_illegal_high_level_choice: "discard_run".to_owned(),
            on_legal_divergence: "continue_and_tag".to_owned(),
            potion_budget_mode: "floor".to_owned(),
        },
        route: SlayTheDataRoute {
            path_taken: string_list(event.get("path_taken")),
            path_per_floor: string_list(event.get("path_per_floor")),
        },
        floor_decisions: floors.into_values().collect(),
        boss_relic_choices: boss_relic_choices(event),
        final_observed: SlayTheDataFinalObserved {
            floor_reached: parse_i32(event.get("floor_reached")),
            victory: event
                .get("victory")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            master_deck: card_name_list(event.get("master_deck")),
            relics: string_list(event.get("relics")),
            gold: parse_i32(event.get("gold")),
        },
        diagnostics: Vec::new(),
    };
    imported.diagnostics = diagnostics_for(&imported);
    Ok(imported)
}

pub fn slaythedata_replay_plan(imported: &SlayTheDataRunImport) -> SlayTheDataReplayPlan {
    let mut diagnostics = imported.diagnostics.clone();
    let run_start = match (
        imported.config.character.clone(),
        imported.config.ascension,
        imported.config.seed_played.clone(),
    ) {
        (Some(character), Some(ascension), Some(seed_played)) => Some(SlayTheDataRunStart {
            character,
            ascension,
            seed_played,
        }),
        _ => {
            diagnostics.push(SlayTheDataDiagnostic {
                severity: SlayTheDataDiagnosticSeverity::Error,
                code: "missing_run_start_identity".to_owned(),
                path: "$.config".to_owned(),
                message: "character, ascension, and seed_played are required to start replay from SlayTheData".to_owned(),
            });
            None
        }
    };

    let mut steps = Vec::new();
    if imported.config.neow_bonus.is_some() || imported.config.neow_cost.is_some() {
        steps.push(SlayTheDataReplayStep {
            floor: 0,
            ordinal: steps.len(),
            kind: SlayTheDataReplayStepKind::NeowTalk,
        });
        steps.push(SlayTheDataReplayStep {
            floor: 0,
            ordinal: steps.len(),
            kind: SlayTheDataReplayStepKind::NeowBonus {
                bonus: imported.config.neow_bonus.clone(),
                cost: imported.config.neow_cost.clone(),
            },
        });
        steps.push(SlayTheDataReplayStep {
            floor: 0,
            ordinal: steps.len(),
            kind: SlayTheDataReplayStepKind::NeowLeave,
        });
    }

    for floor in &imported.floor_decisions {
        if let Some(route) = &floor.route {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::MapRoom {
                    symbol: route.clone(),
                },
            });
        }
        for reward in &floor.card_rewards {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::CardReward {
                    picked: reward.picked.clone(),
                    skipped: reward.skipped,
                },
            });
        }
        for event in &floor.events {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::EventChoice {
                    event_name: event.event_name.clone(),
                    player_choice: event.player_choice.clone(),
                },
            });
        }
        for purchase in &floor.shop_purchases {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::ShopPurchase {
                    item: purchase.item.clone(),
                    base_item: purchase.base_item.clone(),
                },
            });
        }
        for campfire in &floor.campfires {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::Campfire {
                    key: campfire.key.clone(),
                    target_card: campfire.data.clone(),
                },
            });
        }
        if floor.potions.uses_allowed > 0 {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::PotionBudget {
                    uses_allowed: floor.potions.uses_allowed,
                },
            });
        }
    }
    for choice in &imported.boss_relic_choices {
        steps.push(SlayTheDataReplayStep {
            floor: choice.act * 17,
            ordinal: steps.len(),
            kind: SlayTheDataReplayStepKind::BossRelic {
                act: choice.act,
                picked: choice.picked.clone(),
            },
        });
    }

    let mut checkpoints = Vec::new();
    if run_start.is_some() {
        checkpoints.push(SlayTheDataCheckpoint {
            floor: Some(0),
            kind: SlayTheDataCheckpointKind::RunStart,
        });
    }
    checkpoints.extend(
        imported
            .floor_decisions
            .iter()
            .map(|floor| SlayTheDataCheckpoint {
                floor: Some(floor.floor),
                kind: SlayTheDataCheckpointKind::FloorStart {
                    route: floor.route.clone(),
                },
            }),
    );
    checkpoints.push(SlayTheDataCheckpoint {
        floor: imported
            .final_observed
            .floor_reached
            .and_then(|floor| u32::try_from(floor).ok()),
        kind: SlayTheDataCheckpointKind::FinalObserved {
            floor_reached: imported.final_observed.floor_reached,
            victory: imported.final_observed.victory,
            deck_count: imported.final_observed.master_deck.len(),
            relic_count: imported.final_observed.relics.len(),
            gold: imported.final_observed.gold,
        },
    });

    SlayTheDataReplayPlan {
        schema: SLAYTHEDATA_IMPORT_SCHEMA_VERSION,
        source: imported.source.clone(),
        run_start,
        ordering: SlayTheDataReplayOrdering::FloorGrouped,
        steps,
        checkpoints,
        diagnostics,
    }
}

pub fn slaythedata_replay_preflight(plan: &SlayTheDataReplayPlan) -> SlayTheDataPreflightReport {
    let mut diagnostics = plan.diagnostics.clone();
    let mut run = plan.run_start.as_ref().and_then(|start| {
        if !start.character.eq_ignore_ascii_case("IRONCLAD") {
            return None;
        }
        let Ok(ascension) = u8::try_from(start.ascension) else {
            return None;
        };
        if ascension > 20 {
            return None;
        }
        Some(RunState::placeholder_seeded_ironclad(
            sts_seed_string_to_long(&start.seed_played) as u64,
            ascension,
        ))
    });
    let numeric_seed = plan
        .run_start
        .as_ref()
        .map(|start| sts_seed_string_to_long(&start.seed_played));

    if plan.run_start.is_some() && run.is_none() {
        diagnostics.push(SlayTheDataDiagnostic {
            severity: SlayTheDataDiagnosticSeverity::Error,
            code: "cannot_initialize_run_state".to_owned(),
            path: "$.run_start".to_owned(),
            message:
                "preflight can currently initialize only Ironclad runs with ascension in 0..=20"
                    .to_owned(),
        });
    }

    let mut steps = Vec::with_capacity(plan.steps.len());
    for step in &plan.steps {
        let (status, code, message) = match &step.kind {
            SlayTheDataReplayStepKind::NeowTalk => {
                if let Some(current) = run.as_ref() {
                    let actions = legal_event_actions(current);
                    if actions.contains(&EventAction::Choose { choice_index: 0 }) {
                        let next = apply_event_action(
                            current,
                            EventAction::Choose { choice_index: 0 },
                        );
                        match next {
                            Ok(next) => {
                                run = Some(next);
                                (
                                    SlayTheDataPreflightStatus::Checked,
                                    "legal_neow_talk".to_owned(),
                                    "Neow talk is legal from the initialized simulator state"
                                        .to_owned(),
                                )
                            }
                            Err(error) => (
                                SlayTheDataPreflightStatus::Blocked,
                                "neow_talk_apply_failed".to_owned(),
                                format!("Neow talk was legal but failed to apply: {error}"),
                            ),
                        }
                    } else {
                        (
                            SlayTheDataPreflightStatus::Blocked,
                            "illegal_neow_talk".to_owned(),
                            format!(
                                "Neow talk is not legal from phase {:?}; legal event actions: {:?}",
                                current.phase, actions
                            ),
                        )
                    }
                } else {
                    (
                        SlayTheDataPreflightStatus::Blocked,
                        "missing_run_state".to_owned(),
                        "cannot check Neow talk without an initialized simulator run".to_owned(),
                    )
                }
            }
            SlayTheDataReplayStepKind::NeowBonus { bonus, cost } => match run.as_ref() {
                Some(current) => match slaythedata_neow_option(current, bonus, cost) {
                    Ok(option) => {
                        let action = EventAction::Choose {
                            choice_index: option.slot,
                        };
                        if legal_event_actions(current).contains(&action) {
                            match apply_event_action(current, action) {
                                Ok(next) => {
                                    run = Some(next);
                                    (
                                        SlayTheDataPreflightStatus::Checked,
                                        "legal_neow_bonus".to_owned(),
                                        format!(
                                            "matched SlayTheData Neow bonus {:?} cost {:?} to generated option slot {}",
                                            bonus, cost, option.slot
                                        ),
                                    )
                                }
                                Err(error) => (
                                    SlayTheDataPreflightStatus::Blocked,
                                    "neow_bonus_apply_failed".to_owned(),
                                    format!(
                                        "matched Neow option slot {} but failed to apply it: {error}",
                                        option.slot
                                    ),
                                ),
                            }
                        } else {
                            (
                                SlayTheDataPreflightStatus::Blocked,
                                "illegal_neow_bonus_slot".to_owned(),
                                format!(
                                    "matched Neow option slot {} but it is not legal from phase {:?}",
                                    option.slot, current.phase
                                ),
                            )
                        }
                    }
                    Err(message) => (
                        SlayTheDataPreflightStatus::Blocked,
                        "neow_option_not_available".to_owned(),
                        message,
                    ),
                },
                None => (
                    SlayTheDataPreflightStatus::Blocked,
                    "missing_run_state".to_owned(),
                    "cannot check Neow bonus without an initialized simulator run".to_owned(),
                ),
            },
            SlayTheDataReplayStepKind::NeowLeave => {
                if let Some(current) = run.as_ref() {
                    let action = EventAction::Choose { choice_index: 0 };
                    if current.phase == RunPhase::Event && legal_event_actions(current).contains(&action) {
                        match apply_event_action(current, action) {
                            Ok(next) => {
                                run = Some(next);
                                (
                                    SlayTheDataPreflightStatus::Checked,
                                    "legal_neow_leave".to_owned(),
                                    "Neow leave is legal after the selected immediate Neow option"
                                        .to_owned(),
                                )
                            }
                            Err(error) => (
                                SlayTheDataPreflightStatus::Blocked,
                                "neow_leave_apply_failed".to_owned(),
                                format!("Neow leave was legal but failed to apply: {error}"),
                            ),
                        }
                    } else {
                        (
                            SlayTheDataPreflightStatus::Guided,
                            "pending_neow_followup".to_owned(),
                            format!(
                                "Neow leave is pending because the selected option moved the simulator to phase {:?}",
                                current.phase
                            ),
                        )
                    }
                } else {
                    (
                        SlayTheDataPreflightStatus::Blocked,
                        "missing_run_state".to_owned(),
                        "cannot check Neow leave without an initialized simulator run".to_owned(),
                    )
                }
            }
            SlayTheDataReplayStepKind::MapRoom { symbol } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_map_room".to_owned(),
                format!(
                    "route symbol {symbol:?} is available as high-level guidance; exact map-node replay is not connected yet"
                ),
            ),
            SlayTheDataReplayStepKind::CardReward { picked, skipped } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_card_reward".to_owned(),
                format!(
                    "card reward choice picked={:?} skipped={skipped}; concrete reward screen mapping is pending",
                    picked.as_ref().map(|card| card.raw.as_str())
                ),
            ),
            SlayTheDataReplayStepKind::EventChoice {
                event_name,
                player_choice,
            } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_event_choice".to_owned(),
                format!(
                    "event {:?} choice {:?} is high-level guidance until event choice label mapping is connected",
                    event_name, player_choice
                ),
            ),
            SlayTheDataReplayStepKind::ShopPurchase { item, .. } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_shop_purchase".to_owned(),
                format!(
                    "shop purchase {item:?} is high-level guidance until shop slot mapping is connected"
                ),
            ),
            SlayTheDataReplayStepKind::Campfire { key, target_card } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_campfire".to_owned(),
                format!(
                    "campfire key {:?} target {:?} is high-level guidance until rest/grid mapping is connected",
                    key,
                    target_card.as_ref().map(|card| card.raw.as_str())
                ),
            ),
            SlayTheDataReplayStepKind::BossRelic { act, picked } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_boss_relic".to_owned(),
                format!(
                    "act {act} boss relic {:?} is high-level guidance until boss reward screen mapping is connected",
                    picked
                ),
            ),
            SlayTheDataReplayStepKind::PotionBudget { uses_allowed } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_potion_budget".to_owned(),
                format!(
                    "combat agent may spend up to {uses_allowed} potion use(s) on this floor; SlayTheData lacks timing, target, and potion identity"
                ),
            ),
        };
        steps.push(SlayTheDataPreflightStep {
            floor: step.floor,
            ordinal: step.ordinal,
            status,
            code,
            message,
        });
    }

    SlayTheDataPreflightReport {
        schema: SLAYTHEDATA_IMPORT_SCHEMA_VERSION,
        source: plan.source.clone(),
        run_start: plan.run_start.clone(),
        numeric_seed,
        start_phase: run.map(|run| phase_name(run.phase)),
        steps,
        diagnostics,
    }
}

fn phase_name(phase: RunPhase) -> String {
    format!("{phase:?}")
}

fn slaythedata_neow_option(
    run: &RunState,
    bonus: &Option<String>,
    cost: &Option<String>,
) -> Result<GeneratedNeowOption, String> {
    let Some(reward) = bonus.as_deref().and_then(slaythedata_neow_reward_type) else {
        return Err(format!(
            "unknown or missing SlayTheData Neow bonus {bonus:?}"
        ));
    };
    let Some(drawback) = slaythedata_neow_drawback(cost.as_deref().unwrap_or("NONE")) else {
        return Err(format!("unknown SlayTheData Neow cost {cost:?}"));
    };
    let options = generate_neow_options(run.event_rng_seed as i64, run.player_max_hp);
    options
        .into_iter()
        .find(|option| option.reward == reward && option.drawback == drawback)
        .ok_or_else(|| {
            format!(
                "SlayTheData Neow bonus {:?} cost {:?} is not among generated options",
                bonus, cost
            )
        })
}

fn slaythedata_neow_reward_type(value: &str) -> Option<NeowRewardType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "THREE_CARDS" => Some(NeowRewardType::ThreeCards),
        "ONE_RANDOM_RARE_CARD" | "RANDOM_RARE_CARD" => Some(NeowRewardType::OneRandomRareCard),
        "RANDOM_COLORLESS" => Some(NeowRewardType::RandomColorless),
        "RANDOM_COLORLESS_2" => Some(NeowRewardType::RandomColorlessTwo),
        "REMOVE_CARD" => Some(NeowRewardType::RemoveCard),
        "REMOVE_TWO" => Some(NeowRewardType::RemoveTwo),
        "UPGRADE_CARD" => Some(NeowRewardType::UpgradeCard),
        "TRANSFORM_CARD" => Some(NeowRewardType::TransformCard),
        "TRANSFORM_TWO_CARDS" => Some(NeowRewardType::TransformTwoCards),
        "THREE_SMALL_POTIONS" => Some(NeowRewardType::ThreeSmallPotions),
        "RANDOM_COMMON_RELIC" => Some(NeowRewardType::RandomCommonRelic),
        "ONE_RARE_RELIC" => Some(NeowRewardType::OneRareRelic),
        "TEN_PERCENT_HP_BONUS" => Some(NeowRewardType::TenPercentHpBonus),
        "TWENTY_PERCENT_HP_BONUS" => Some(NeowRewardType::TwentyPercentHpBonus),
        "THREE_ENEMY_KILL" => Some(NeowRewardType::ThreeEnemyKill),
        "HUNDRED_GOLD" => Some(NeowRewardType::HundredGold),
        "TWO_FIFTY_GOLD" => Some(NeowRewardType::TwoFiftyGold),
        "BOSS_RELIC" => Some(NeowRewardType::BossRelic),
        _ => None,
    }
}

fn slaythedata_neow_drawback(value: &str) -> Option<NeowDrawback> {
    match value.trim().to_ascii_uppercase().as_str() {
        "NONE" => Some(NeowDrawback::None),
        "TEN_PERCENT_HP_LOSS" => Some(NeowDrawback::TenPercentHpLoss),
        "NO_GOLD" => Some(NeowDrawback::NoGold),
        "CURSE" => Some(NeowDrawback::Curse),
        "PERCENT_DAMAGE" => Some(NeowDrawback::PercentDamage),
        _ => None,
    }
}

fn import_card_rewards(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (ordinal, choice) in array(event.get("card_choices")).iter().enumerate() {
        let Some(floor) = parse_non_negative_floor(choice.get("floor")) else {
            continue;
        };
        floor_entry(floors, floor)
            .card_rewards
            .push(SlayTheDataCardReward {
                ordinal,
                picked: card_name(choice.get("picked")),
                not_picked: card_name_list(choice.get("not_picked")),
                skipped: optional_string(choice.get("picked")).as_deref() == Some("SKIP"),
            });
    }
}

fn import_relics_obtained(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (ordinal, relic) in array(event.get("relics_obtained")).iter().enumerate() {
        let Some(floor) = parse_positive_floor(relic.get("floor")) else {
            continue;
        };
        if let Some(key) = optional_string(relic.get("key")) {
            floor_entry(floors, floor)
                .relics_obtained
                .push(SlayTheDataNamedFloorItem { ordinal, key });
        }
    }
}

fn import_event_choices(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (ordinal, choice) in array(event.get("event_choices")).iter().enumerate() {
        let Some(floor) = parse_positive_floor(choice.get("floor")) else {
            continue;
        };
        floor_entry(floors, floor)
            .events
            .push(SlayTheDataEventChoice {
                ordinal,
                event_name: optional_string(choice.get("event_name")),
                player_choice: optional_string(choice.get("player_choice")),
                damage_taken: parse_i32(choice.get("damage_taken")),
                damage_healed: parse_i32(choice.get("damage_healed")),
                max_hp_gain: parse_i32(choice.get("max_hp_gain")),
                max_hp_loss: parse_i32(choice.get("max_hp_loss")),
                gold_gain: parse_i32(choice.get("gold_gain")),
                gold_loss: parse_i32(choice.get("gold_loss")),
                cards_obtained: card_name_list(choice.get("cards_obtained")),
                cards_removed: card_name_list(choice.get("cards_removed")),
                cards_upgraded: card_name_list(choice.get("cards_upgraded")),
                relics_obtained: string_list(choice.get("relics_obtained")),
            });
    }
}

fn import_shop_purchases(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    let purchase_floors = array(event.get("item_purchase_floors"));
    for (ordinal, item) in array(event.get("items_purchased")).iter().enumerate() {
        let Some(floor) = purchase_floors
            .get(ordinal)
            .and_then(|value| parse_positive_floor(Some(value)))
        else {
            continue;
        };
        if let Some(item) = optional_string(Some(item)) {
            floor_entry(floors, floor)
                .shop_purchases
                .push(SlayTheDataShopPurchase {
                    base_item: clean_card_text(&item).base,
                    item,
                    ordinal,
                });
        }
    }
}

fn import_campfires(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (ordinal, choice) in array(event.get("campfire_choices")).iter().enumerate() {
        let Some(floor) = parse_positive_floor(choice.get("floor")) else {
            continue;
        };
        floor_entry(floors, floor)
            .campfires
            .push(SlayTheDataCampfireChoice {
                ordinal,
                key: optional_string(choice.get("key")),
                data: card_name(choice.get("data")),
            });
    }
}

fn import_potions(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (ordinal, value) in array(event.get("potions_floor_usage")).iter().enumerate() {
        if let Some(floor) = parse_positive_floor(Some(value)) {
            let potions = &mut floor_entry(floors, floor).potions;
            potions.uses_allowed += 1;
            potions.usage_ordinals.push(ordinal);
        }
    }

    for (ordinal, potion) in array(event.get("potions_obtained")).iter().enumerate() {
        let Some(floor) = parse_positive_floor(potion.get("floor")) else {
            continue;
        };
        if let Some(key) = optional_string(potion.get("key")) {
            floor_entry(floors, floor)
                .potions
                .obtained
                .push(SlayTheDataNamedFloorItem { ordinal, key });
        }
    }
}

fn import_route(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (index, route) in string_list(event.get("path_per_floor"))
        .into_iter()
        .enumerate()
    {
        floor_entry(floors, index as u32 + 1).route = Some(route);
    }
}

fn boss_relic_choices(event: &serde_json::Map<String, Value>) -> Vec<SlayTheDataBossRelicChoice> {
    array(event.get("boss_relics"))
        .iter()
        .enumerate()
        .map(|(ordinal, choice)| SlayTheDataBossRelicChoice {
            act: ordinal as u32 + 1,
            ordinal,
            picked: optional_string(choice.get("picked")),
            not_picked: string_list(choice.get("not_picked")),
        })
        .collect()
}

fn diagnostics_for(imported: &SlayTheDataRunImport) -> Vec<SlayTheDataDiagnostic> {
    let mut diagnostics = vec![SlayTheDataDiagnostic {
        severity: SlayTheDataDiagnosticSeverity::Info,
        code: "exact_combat_actions_unavailable".to_owned(),
        path: "$".to_owned(),
        message: "SlayTheData does not record exact combat actions; combat must be delegated to the simulator combat agent or strict traces".to_owned(),
    }];

    if imported
        .config
        .character
        .as_deref()
        .is_some_and(|character| !character.eq_ignore_ascii_case("IRONCLAD"))
    {
        diagnostics.push(SlayTheDataDiagnostic {
            severity: SlayTheDataDiagnosticSeverity::Error,
            code: "unsupported_character".to_owned(),
            path: "$.character_chosen".to_owned(),
            message: format!(
                "only Ironclad imports are currently supported, got {:?}",
                imported.config.character
            ),
        });
    }

    if imported
        .config
        .ascension
        .is_some_and(|ascension| !(0..=20).contains(&ascension))
    {
        diagnostics.push(SlayTheDataDiagnostic {
            severity: SlayTheDataDiagnosticSeverity::Error,
            code: "unsupported_ascension".to_owned(),
            path: "$.ascension_level".to_owned(),
            message: format!(
                "ascension must be in 0..=20, got {:?}",
                imported.config.ascension
            ),
        });
    }

    for floor in &imported.floor_decisions {
        if floor.potions.uses_allowed > 0 {
            diagnostics.push(SlayTheDataDiagnostic {
                severity: SlayTheDataDiagnosticSeverity::Warning,
                code: "floor_only_potion_budget".to_owned(),
                path: format!("$.floor_decisions[floor={}].potions", floor.floor),
                message: "SlayTheData records potion usage by floor only; identity, target, and timing must be chosen by the combat agent".to_owned(),
            });
        }
        let grid_targets = floor_grid_target_count(floor);
        if grid_targets > 1 {
            diagnostics.push(SlayTheDataDiagnostic {
                severity: SlayTheDataDiagnosticSeverity::Warning,
                code: "ambiguous_repeated_grid_floor".to_owned(),
                path: format!("$.floor_decisions[floor={}]", floor.floor),
                message: format!(
                    "floor {} has {grid_targets} card-grid targets; run history may not order repeated grids precisely enough",
                    floor.floor
                ),
            });
        }
    }

    diagnostics
}

fn floor_grid_target_count(floor: &SlayTheDataFloorDecision) -> usize {
    let campfire_targets = floor
        .campfires
        .iter()
        .filter(|choice| choice.data.is_some())
        .count();
    let event_targets: usize = floor
        .events
        .iter()
        .map(|event| {
            event.cards_obtained.len() + event.cards_removed.len() + event.cards_upgraded.len()
        })
        .sum();
    campfire_targets + event_targets
}

fn floor_entry(
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
    floor: u32,
) -> &mut SlayTheDataFloorDecision {
    floors
        .entry(floor)
        .or_insert_with(|| SlayTheDataFloorDecision {
            floor,
            route: None,
            card_rewards: Vec::new(),
            relics_obtained: Vec::new(),
            events: Vec::new(),
            shop_purchases: Vec::new(),
            campfires: Vec::new(),
            potions: SlayTheDataPotionFloorDecision::default(),
        })
}

fn card_name(value: Option<&Value>) -> Option<SlayTheDataCardName> {
    optional_string(value).map(|text| clean_card_text(&text))
}

fn card_name_list(value: Option<&Value>) -> Vec<SlayTheDataCardName> {
    string_list(value)
        .into_iter()
        .map(|text| clean_card_text(&text))
        .collect()
}

fn clean_card_text(text: &str) -> SlayTheDataCardName {
    let upgraded = text.ends_with('+');
    let base = if upgraded {
        text.trim_end_matches('+').to_owned()
    } else {
        text.to_owned()
    };
    SlayTheDataCardName {
        raw: text.to_owned(),
        base,
        upgraded,
    }
}

fn array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| optional_string(Some(value)))
        .collect()
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn parse_i32(value: Option<&Value>) -> Option<i32> {
    parse_i64(value).and_then(|value| i32::try_from(value).ok())
}

fn parse_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn parse_positive_floor(value: Option<&Value>) -> Option<u32> {
    let parsed = parse_i64(value)?;
    (parsed > 0).then_some(parsed as u32)
}

fn parse_non_negative_floor(value: Option<&Value>) -> Option<u32> {
    let parsed = parse_i64(value)?;
    (parsed >= 0).then_some(parsed as u32)
}
