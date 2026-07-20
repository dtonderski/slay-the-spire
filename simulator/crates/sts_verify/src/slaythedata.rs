use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, error::Error, fmt};
use sts_core::{
    apply_run_decision_action,
    content::{cards::get_card_definition, monsters::get_monster_definition},
    generate_neow_options, legal_event_actions, legal_map_actions_on_run, EventAction,
    GeneratedNeowOption, MapAction, NeowDrawback, NeowRewardType, RoomKind, RunAction,
    RunDecisionAction, RunPhase, RunState,
};

use crate::try_sts_seed_string_to_long;

fn apply_event_action(run: &RunState, action: EventAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Event(action))
}

fn apply_map_action_on_run(run: &RunState, action: MapAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Map(action))
}

fn apply_run_action(run: &RunState, action: RunAction) -> sts_core::SimResult<RunState> {
    apply_run_decision_action(run, RunDecisionAction::Run(action))
}

pub const SLAYTHEDATA_IMPORT_SCHEMA_VERSION: u32 = 1;
pub const SLAYTHEDATA_NORMAL_MAX_FLOOR_REACHED: i32 = 57;

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
    pub route_fully_checked: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_command: Option<SlayTheDataBridgeCommandHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataBridgeCommandHint {
    pub descriptor: SlayTheDataBridgeDescriptor,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum SlayTheDataBridgeDescriptor {
    ChooseVisibleOption { option_slot: usize },
    SkipVisibleReward,
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
    CombatEncounter {
        enemies: Option<String>,
    },
    CardReward {
        picked: Option<SlayTheDataCardName>,
        skipped: bool,
    },
    EventChoice {
        event_name: Option<String>,
        player_choice: Option<String>,
        cards_obtained: Vec<SlayTheDataCardName>,
        cards_removed: Vec<SlayTheDataCardName>,
        #[serde(default)]
        cards_transformed: Vec<SlayTheDataCardName>,
        cards_upgraded: Vec<SlayTheDataCardName>,
        relics_obtained: Vec<String>,
        relics_lost: Vec<String>,
    },
    ShopPurchase {
        item: String,
        base_item: String,
    },
    ShopPurge {
        card: SlayTheDataCardName,
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
    pub is_beta: Option<bool>,
    pub is_daily: Option<bool>,
    pub is_endless: Option<bool>,
    pub is_prod: Option<bool>,
    pub is_trial: Option<bool>,
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
    pub combats: Vec<SlayTheDataCombatEncounter>,
    pub card_rewards: Vec<SlayTheDataCardReward>,
    pub relics_obtained: Vec<SlayTheDataNamedFloorItem>,
    pub events: Vec<SlayTheDataEventChoice>,
    pub shop_purchases: Vec<SlayTheDataShopPurchase>,
    pub shop_purges: Vec<SlayTheDataCardName>,
    pub campfires: Vec<SlayTheDataCampfireChoice>,
    pub potions: SlayTheDataPotionFloorDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlayTheDataCombatEncounter {
    pub ordinal: usize,
    pub enemies: Option<String>,
    pub damage: Option<i32>,
    pub turns: Option<i32>,
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
    #[serde(default)]
    pub cards_transformed: Vec<SlayTheDataCardName>,
    pub cards_upgraded: Vec<SlayTheDataCardName>,
    pub relics_obtained: Vec<String>,
    pub relics_lost: Vec<String>,
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
    import_shop_purges(event, &mut floors);
    import_campfires(event, &mut floors);
    import_potions(event, &mut floors);
    import_combat_encounters(event, &mut floors);
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
            is_beta: parse_bool(event.get("is_beta")),
            is_daily: parse_bool(event.get("is_daily")),
            is_endless: parse_bool(event.get("is_endless")),
            is_prod: parse_bool(event.get("is_prod")),
            is_trial: parse_bool(event.get("is_trial")),
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
            path_per_floor: string_list_preserving_empty(event.get("path_per_floor")),
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

fn is_knowing_skull_sequence_step(event_name: Option<&str>, player_choice: Option<&str>) -> bool {
    event_name.is_some_and(|name| normalize_slaythedata_label(name) == "knowingskull")
        && player_choice.is_some_and(|choice| {
            matches!(
                normalize_slaythedata_label(choice).as_str(),
                "potion" | "gold" | "card" | "leave"
            )
        })
}

fn knowing_skull_sequence_choices(event: &SlayTheDataEventChoice) -> Vec<Option<String>> {
    let Some(player_choice) = event.player_choice.as_deref() else {
        return Vec::new();
    };
    if event
        .event_name
        .as_deref()
        .is_none_or(|name| normalize_slaythedata_label(name) != "knowingskull")
    {
        return Vec::new();
    }

    let mut choices = player_choice
        .split_whitespace()
        .filter_map(|token| match token.to_ascii_uppercase().as_str() {
            "POTION" | "GOLD" | "CARD" => Some(Some(token.to_ascii_uppercase())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if choices.is_empty() {
        return Vec::new();
    }
    choices.push(Some("LEAVE".to_owned()));
    choices
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
        // SlayTheData records a selectable Neow card reward in card_choices at
        // floor zero. Resolve it between choosing the bonus and leaving Neow;
        // appending it with ordinary floor rewards would place it after leave.
        if let Some(floor_zero) = imported
            .floor_decisions
            .iter()
            .find(|floor| floor.floor == 0)
        {
            for reward in &floor_zero.card_rewards {
                steps.push(SlayTheDataReplayStep {
                    floor: 0,
                    ordinal: steps.len(),
                    kind: SlayTheDataReplayStepKind::CardReward {
                        picked: reward.picked.clone(),
                        skipped: reward.skipped,
                    },
                });
            }
        }
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
        for combat in &floor.combats {
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::CombatEncounter {
                    enemies: combat.enemies.clone(),
                },
            });
        }
        for event in &floor.events {
            let player_choices = {
                let sequence = knowing_skull_sequence_choices(event);
                if sequence.is_empty() {
                    vec![event.player_choice.clone()]
                } else {
                    sequence
                }
            };
            for player_choice in player_choices {
                steps.push(SlayTheDataReplayStep {
                    floor: floor.floor,
                    ordinal: steps.len(),
                    kind: SlayTheDataReplayStepKind::EventChoice {
                        event_name: event.event_name.clone(),
                        player_choice,
                        cards_obtained: event.cards_obtained.clone(),
                        cards_removed: event.cards_removed.clone(),
                        cards_transformed: event.cards_transformed.clone(),
                        cards_upgraded: event.cards_upgraded.clone(),
                        relics_obtained: event.relics_obtained.clone(),
                        relics_lost: event.relics_lost.clone(),
                    },
                });
            }
        }
        // Orrery's five card choices are recorded on the shop floor even though
        // buying Orrery is what creates them. Preserve that causal boundary:
        // purchases through Orrery happen before its choices, while later shop
        // purchases remain after the overlay has closed.
        let orrery_purchase_index = floor
            .route
            .as_deref()
            .is_some_and(|route| normalize_route_symbol(route).as_deref() == Some("$"))
            .then(|| {
                floor
                    .shop_purchases
                    .iter()
                    .position(|purchase| normalize_slaythedata_label(&purchase.item) == "orrery")
            })
            .flatten();

        // The dataset stores purchases and purges in separate arrays, so it
        // cannot preserve their interleaved order within one shop. Resolve
        // required purges first: optional purchases can otherwise spend the
        // gold that makes the recorded purge legal.
        if orrery_purchase_index.is_some() {
            for card in &floor.shop_purges {
                steps.push(SlayTheDataReplayStep {
                    floor: floor.floor,
                    ordinal: steps.len(),
                    kind: SlayTheDataReplayStepKind::ShopPurge { card: card.clone() },
                });
            }
        }
        if let Some(index) = orrery_purchase_index {
            for purchase in &floor.shop_purchases[..=index] {
                steps.push(SlayTheDataReplayStep {
                    floor: floor.floor,
                    ordinal: steps.len(),
                    kind: SlayTheDataReplayStepKind::ShopPurchase {
                        item: purchase.item.clone(),
                        base_item: purchase.base_item.clone(),
                    },
                });
            }
        }

        // Event combats (for example Dead Adventurer) record their normal
        // post-combat card choice on the same floor. The event choices must
        // happen first so replay reaches that combat before consuming its
        // reward. This ordering is also correct for event-provided card grids.
        for reward in &floor.card_rewards {
            // Floor-zero Neow card picks were inserted before NeowLeave above.
            if floor.floor == 0 && imported.config.neow_bonus.is_some() {
                continue;
            }
            steps.push(SlayTheDataReplayStep {
                floor: floor.floor,
                ordinal: steps.len(),
                kind: SlayTheDataReplayStepKind::CardReward {
                    picked: reward.picked.clone(),
                    skipped: reward.skipped,
                },
            });
        }
        if orrery_purchase_index.is_none() {
            for card in &floor.shop_purges {
                steps.push(SlayTheDataReplayStep {
                    floor: floor.floor,
                    ordinal: steps.len(),
                    kind: SlayTheDataReplayStepKind::ShopPurge { card: card.clone() },
                });
            }
        }
        let remaining_purchases = orrery_purchase_index
            .map_or(floor.shop_purchases.as_slice(), |index| {
                &floor.shop_purchases[index.saturating_add(1)..]
            });
        for purchase in remaining_purchases {
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
    let seed_result = plan
        .run_start
        .as_ref()
        .map(|start| slaythedata_seed_to_long(&start.seed_played));
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
        let seed = seed_result.as_ref()?.as_ref().ok()?;
        Some(RunState::seeded_ironclad(*seed as u64, ascension))
    });
    let numeric_seed = seed_result
        .as_ref()
        .and_then(|result| result.as_ref().ok().copied());

    if let Some(Err(message)) = seed_result.as_ref() {
        diagnostics.push(SlayTheDataDiagnostic {
            severity: SlayTheDataDiagnosticSeverity::Error,
            code: "invalid_seed_played".to_owned(),
            path: "$.run_start.seed_played".to_owned(),
            message: message.clone(),
        });
    }

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
    let mut route_proof_start: Option<RunState> = None;
    for step in &plan.steps {
        let (status, code, message, bridge_command) = match &step.kind {
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
                                    Some(choose_visible_hint(0)),
                                )
                            }
                            Err(error) => (
                                SlayTheDataPreflightStatus::Blocked,
                                "neow_talk_apply_failed".to_owned(),
                                format!("Neow talk was legal but failed to apply: {error}"),
                                None,
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
                            None,
                        )
                    }
                } else {
                    (
                        SlayTheDataPreflightStatus::Blocked,
                        "missing_run_state".to_owned(),
                        "cannot check Neow talk without an initialized simulator run".to_owned(),
                        None,
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
                                        Some(choose_visible_hint(option.slot)),
                                    )
                                }
                                Err(error) => (
                                    SlayTheDataPreflightStatus::Blocked,
                                    "neow_bonus_apply_failed".to_owned(),
                                    format!(
                                        "matched Neow option slot {} but failed to apply it: {error}",
                                        option.slot
                                    ),
                                    None,
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
                                None,
                            )
                        }
                    }
                    Err(message) => (
                        SlayTheDataPreflightStatus::Blocked,
                        "neow_option_not_available".to_owned(),
                        message,
                        None,
                    ),
                },
                None => (
                    SlayTheDataPreflightStatus::Blocked,
                    "missing_run_state".to_owned(),
                    "cannot check Neow bonus without an initialized simulator run".to_owned(),
                    None,
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
                                    Some(choose_visible_hint(0)),
                                )
                            }
                            Err(error) => (
                                SlayTheDataPreflightStatus::Blocked,
                                "neow_leave_apply_failed".to_owned(),
                                format!("Neow leave was legal but failed to apply: {error}"),
                                None,
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
                            None,
                        )
                    }
                } else {
                    (
                        SlayTheDataPreflightStatus::Blocked,
                        "missing_run_state".to_owned(),
                        "cannot check Neow leave without an initialized simulator run".to_owned(),
                        None,
                    )
                }
            }
            SlayTheDataReplayStepKind::MapRoom { symbol } => match run.as_ref() {
                Some(current) if current.phase == RunPhase::Idle => {
                    if route_proof_start.is_none() {
                        route_proof_start = Some(current.clone());
                    }
                    let actions = legal_map_actions_on_run(current);
                    let action_symbols: Vec<_> = actions
                        .iter()
                        .filter_map(|action| map_action_room_kind(current, *action))
                        .map(room_kind_symbol)
                        .collect();
                    let matches: Vec<_> = actions
                        .iter()
                        .copied()
                        .filter(|action| map_action_matches_symbol(current, *action, symbol))
                        .collect();
                    let constrained_match = constrain_map_action_by_slaythedata_evidence(
                        current,
                        &actions,
                        &matches,
                        plan,
                        step.floor,
                    );
                    if let Some(ConstrainedMapAction {
                        action,
                        action_slot,
                        evidence,
                    }) = constrained_match.first()
                    {
                        match apply_map_action_on_run(current, *action) {
                            Ok(next) => {
                                run = Some(next);
                                (
                                    SlayTheDataPreflightStatus::Checked,
                                    "legal_map_room".to_owned(),
                                    format!(
                                        "route symbol {symbol:?} matched legal map action {:?} using {evidence}",
                                        action
                                    ),
                                    Some(choose_visible_hint(*action_slot)),
                                )
                            }
                            Err(error) => (
                                SlayTheDataPreflightStatus::Blocked,
                                "map_action_apply_failed".to_owned(),
                                format!(
                                    "route symbol {symbol:?} matched {:?} but failed to apply: {error}",
                                    action
                                ),
                                None,
                            ),
                        }
                    } else {
                        match matches.as_slice() {
                        [action, ..] => {
                            let action_slot = actions
                                .iter()
                                .position(|candidate| *candidate == *action)
                                .unwrap_or(0);
                            match apply_map_action_on_run(current, *action) {
                                Ok(next) => {
                                    run = Some(next);
                                    (
                                        SlayTheDataPreflightStatus::Checked,
                                        "legal_map_room".to_owned(),
                                        format!(
                                            "route symbol {symbol:?} matched legal map action {:?}; ambiguity accepted by first legal candidate",
                                            action
                                        ),
                                        Some(choose_visible_hint(action_slot)),
                                    )
                                }
                                Err(error) => (
                                    SlayTheDataPreflightStatus::Blocked,
                                    "map_action_apply_failed".to_owned(),
                                    format!(
                                        "route symbol {symbol:?} matched {:?} but failed to apply: {error}",
                                        action
                                    ),
                                    None,
                                ),
                            }
                        }
                        [] => (
                            SlayTheDataPreflightStatus::Blocked,
                            "map_symbol_unmatched".to_owned(),
                            format!(
                                "route symbol {symbol:?} matched no legal map actions from phase {:?} ({} legal map action(s), candidate symbols {:?})",
                                current.phase,
                                actions.len(),
                                action_symbols
                            ),
                            None,
                        ),
                        }
                    }
                }
                Some(current) => (
                    SlayTheDataPreflightStatus::Guided,
                    "pending_room_resolution".to_owned(),
                    format!(
                        "route symbol {symbol:?} cannot be checked until phase {:?} resolves back to the map",
                        current.phase
                    ),
                    None,
                ),
                None => (
                    SlayTheDataPreflightStatus::Blocked,
                    "missing_run_state".to_owned(),
                    "cannot check map route without an initialized simulator run".to_owned(),
                    None,
                ),
            },
            SlayTheDataReplayStepKind::CardReward { picked, skipped } => {
                match run.as_ref() {
                    Some(current) if current.phase == RunPhase::Reward => {
                        match slaythedata_card_reward_action(current, picked, *skipped) {
                            Ok((action, hint)) => match apply_run_action(current, action) {
                                Ok(next) => {
                                    run = Some(next);
                                    (
                                        SlayTheDataPreflightStatus::Checked,
                                        "legal_card_reward".to_owned(),
                                        format!(
                                            "card reward choice picked={:?} skipped={skipped} matched core reward choices",
                                            picked.as_ref().map(|card| card.raw.as_str())
                                        ),
                                        Some(hint),
                                    )
                                }
                                Err(error) => (
                                    SlayTheDataPreflightStatus::Blocked,
                                    "card_reward_apply_failed".to_owned(),
                                    format!(
                                        "card reward choice picked={:?} skipped={skipped} matched but failed to apply: {error}",
                                        picked.as_ref().map(|card| card.raw.as_str())
                                    ),
                                    None,
                                ),
                            },
                            Err(message) => (
                                SlayTheDataPreflightStatus::Guided,
                                "guided_card_reward".to_owned(),
                                message,
                                None,
                            ),
                        }
                    }
                    Some(current) => (
                        SlayTheDataPreflightStatus::Guided,
                        "pending_card_reward".to_owned(),
                        format!(
                            "card reward choice picked={:?} skipped={skipped} is pending because simulator phase is {:?}",
                            picked.as_ref().map(|card| card.raw.as_str()),
                            current.phase
                        ),
                        None,
                    ),
                    None => (
                        SlayTheDataPreflightStatus::Blocked,
                        "missing_run_state".to_owned(),
                        "cannot check card reward without an initialized simulator run".to_owned(),
                        None,
                    ),
                }
            }
            SlayTheDataReplayStepKind::CombatEncounter { enemies } => (
                SlayTheDataPreflightStatus::Guided,
                "combat_encounter_evidence".to_owned(),
                format!(
                    "recorded combat encounter {:?} is used as map-branch evidence; combat actions are delegated to the combat agent",
                    enemies
                ),
                None,
            ),
            SlayTheDataReplayStepKind::EventChoice {
                event_name,
                player_choice,
                cards_obtained,
                cards_removed,
                cards_transformed,
                cards_upgraded,
                relics_obtained,
                relics_lost,
            } => (
                SlayTheDataPreflightStatus::Guided,
                if is_knowing_skull_sequence_step(
                    event_name.as_deref(),
                    player_choice.as_deref(),
                ) {
                    "guided_event_sequence".to_owned()
                } else {
                    "guided_event_choice".to_owned()
                },
                format!(
                    "event {:?} choice {:?} obtained {:?} removed {:?} transformed {:?} upgraded {:?} relics obtained {:?} lost {:?} is high-level guidance until event choice label/grid mapping is connected",
                    event_name,
                    player_choice,
                    card_names_for_message(cards_obtained),
                    card_names_for_message(cards_removed),
                    card_names_for_message(cards_transformed),
                    card_names_for_message(cards_upgraded),
                    relics_obtained,
                    relics_lost
                ),
                None,
            ),
            SlayTheDataReplayStepKind::ShopPurchase { item, .. } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_shop_purchase".to_owned(),
                format!(
                    "shop purchase {item:?} is high-level guidance until shop slot mapping is connected"
                ),
                None,
            ),
            SlayTheDataReplayStepKind::ShopPurge { card } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_shop_purge".to_owned(),
                format!(
                    "shop purge target {:?} is high-level guidance until shop removal grid mapping is connected",
                    card.raw
                ),
                None,
            ),
            SlayTheDataReplayStepKind::Campfire { key, target_card } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_campfire".to_owned(),
                format!(
                    "campfire key {:?} target {:?} is high-level guidance until rest/grid mapping is connected",
                    key,
                    target_card.as_ref().map(|card| card.raw.as_str())
                ),
                None,
            ),
            SlayTheDataReplayStepKind::BossRelic { act, picked } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_boss_relic".to_owned(),
                format!(
                    "act {act} boss relic {:?} is high-level guidance until boss reward screen mapping is connected",
                    picked
                ),
                None,
            ),
            SlayTheDataReplayStepKind::PotionBudget { uses_allowed } => (
                SlayTheDataPreflightStatus::Guided,
                "guided_potion_budget".to_owned(),
                format!(
                    "combat agent may spend up to {uses_allowed} potion use(s) on this floor; SlayTheData lacks timing, target, and potion identity"
                ),
                None,
            ),
        };
        steps.push(SlayTheDataPreflightStep {
            floor: step.floor,
            ordinal: step.ordinal,
            status,
            code,
            message,
            bridge_command,
        });
        if status == SlayTheDataPreflightStatus::Blocked {
            run = None;
        }
    }

    let route_fully_checked = route_fully_checked(route_proof_start.as_ref(), plan);
    if !route_fully_checked {
        diagnostics.push(SlayTheDataDiagnostic {
            severity: SlayTheDataDiagnosticSeverity::Error,
            code: "route_not_fully_proven".to_owned(),
            path: "$.steps".to_owned(),
            message: "SlayTheData route contains at least one map step that was not uniquely checked against simulator map, monster, or event evidence".to_owned(),
        });
    }

    SlayTheDataPreflightReport {
        schema: SLAYTHEDATA_IMPORT_SCHEMA_VERSION,
        source: plan.source.clone(),
        run_start: plan.run_start.clone(),
        numeric_seed,
        start_phase: run.map(|run| phase_name(run.phase)),
        route_fully_checked,
        steps,
        diagnostics,
    }
}

fn card_names_for_message(cards: &[SlayTheDataCardName]) -> Vec<&str> {
    cards.iter().map(|card| card.base.as_str()).collect()
}

fn route_fully_checked(start: Option<&RunState>, plan: &SlayTheDataReplayPlan) -> bool {
    let route_steps: Vec<(u32, String)> = plan
        .steps
        .iter()
        .filter_map(|step| match &step.kind {
            SlayTheDataReplayStepKind::MapRoom { symbol } => {
                normalize_route_symbol(symbol).map(|symbol| (step.floor, symbol))
            }
            _ => None,
        })
        .collect();
    if route_steps.is_empty() {
        return true;
    }
    let Some(start) = start else {
        return false;
    };
    prove_route_suffix(start.clone(), plan, &route_steps)
}

fn prove_route_suffix(
    run: RunState,
    plan: &SlayTheDataReplayPlan,
    route: &[(u32, String)],
) -> bool {
    let Some(((floor, symbol), rest)) = route.split_first() else {
        return true;
    };
    let idle = run_completed_for_route_lookahead(run);
    let actions = legal_map_actions_on_run(&idle);
    let matches: Vec<_> = actions
        .iter()
        .copied()
        .filter(|action| map_action_matches_symbol(&idle, *action, symbol))
        .collect();
    let chosen = match matches.as_slice() {
        [] => return false,
        [action] => *action,
        _ => {
            let constrained = constrain_map_action_by_slaythedata_evidence(
                &idle, &actions, &matches, plan, *floor,
            );
            constrained
                .first()
                .map(|candidate| candidate.action)
                .unwrap_or(matches[0])
        }
    };
    let Ok(next) = apply_map_action_on_run(&idle, chosen) else {
        return false;
    };
    prove_route_suffix(next, plan, rest)
}

#[derive(Debug, Clone)]
struct ConstrainedMapAction {
    action: MapAction,
    action_slot: usize,
    evidence: String,
}

fn constrain_map_action_by_slaythedata_evidence(
    current: &RunState,
    actions: &[MapAction],
    matches: &[MapAction],
    plan: &SlayTheDataReplayPlan,
    floor: u32,
) -> Vec<ConstrainedMapAction> {
    if matches.len() <= 1 {
        return Vec::new();
    }
    let route_constrained =
        constrain_map_actions_by_future_route_symbols(current, actions, matches, plan, floor);
    if route_constrained.len() == 1 {
        return route_constrained;
    }
    let mut constrained = Vec::new();
    for action in matches {
        let Ok(next) = apply_map_action_on_run(current, *action) else {
            continue;
        };
        let evidence = slaythedata_map_candidate_evidence(&next, plan, floor);
        if let Some(evidence) = evidence {
            let action_slot = actions
                .iter()
                .position(|candidate| candidate == action)
                .unwrap_or(0);
            constrained.push(ConstrainedMapAction {
                action: *action,
                action_slot,
                evidence,
            });
        }
    }
    constrained
}

fn constrain_map_actions_by_future_route_symbols(
    current: &RunState,
    actions: &[MapAction],
    matches: &[MapAction],
    plan: &SlayTheDataReplayPlan,
    floor: u32,
) -> Vec<ConstrainedMapAction> {
    let future_steps: Vec<(u32, String)> = plan
        .steps
        .iter()
        .filter(|step| step.floor > floor)
        .filter_map(|step| match &step.kind {
            SlayTheDataReplayStepKind::MapRoom { symbol } => {
                normalize_route_symbol(symbol).map(|symbol| (step.floor, symbol))
            }
            _ => None,
        })
        .collect();
    if future_steps.is_empty() {
        return Vec::new();
    }

    let mut constrained = Vec::new();
    for action in matches {
        let Ok(next) = apply_map_action_on_run(current, *action) else {
            continue;
        };
        if run_state_can_match_route_suffix(next, plan, &future_steps) {
            let action_slot = actions
                .iter()
                .position(|candidate| candidate == action)
                .unwrap_or(0);
            constrained.push(ConstrainedMapAction {
                action: *action,
                action_slot,
                evidence: "future SlayTheData route symbols".to_owned(),
            });
        }
    }
    constrained
}

fn run_state_can_match_route_suffix(
    run: RunState,
    plan: &SlayTheDataReplayPlan,
    future_steps: &[(u32, String)],
) -> bool {
    let Some(((floor, symbol), rest)) = future_steps.split_first() else {
        return true;
    };
    let idle = run_completed_for_route_lookahead(run);
    legal_map_actions_on_run(&idle).into_iter().any(|action| {
        let Some(kind) = map_action_room_kind(&idle, action) else {
            return false;
        };
        if normalize_route_symbol(room_kind_symbol(kind)).as_ref() != Some(symbol) {
            return false;
        }
        let Ok(next) = apply_map_action_on_run(&idle, action) else {
            return false;
        };
        if floor_has_candidate_evidence(plan, *floor)
            && slaythedata_map_candidate_evidence(&next, plan, *floor).is_none()
        {
            return false;
        }
        run_state_can_match_route_suffix(next, plan, rest)
    })
}

fn run_completed_for_route_lookahead(mut run: RunState) -> RunState {
    run.phase = RunPhase::Idle;
    run.combat = None;
    run.reward = None;
    run.event = None;
    run.shop = None;
    run.shop_merchant_open = false;
    run.card_grid = None;
    run.treasure_room = None;
    run.rest_room_complete = false;
    run
}

fn floor_has_candidate_evidence(plan: &SlayTheDataReplayPlan, floor: u32) -> bool {
    plan.steps.iter().any(|step| {
        step.floor == floor
            && matches!(
                step.kind,
                SlayTheDataReplayStepKind::EventChoice { .. }
                    | SlayTheDataReplayStepKind::CombatEncounter { .. }
                    | SlayTheDataReplayStepKind::CardReward { .. }
                    | SlayTheDataReplayStepKind::ShopPurchase { .. }
                    | SlayTheDataReplayStepKind::ShopPurge { .. }
                    | SlayTheDataReplayStepKind::Campfire { .. }
                    | SlayTheDataReplayStepKind::PotionBudget { .. }
            )
    })
}

fn slaythedata_map_candidate_evidence(
    next: &RunState,
    plan: &SlayTheDataReplayPlan,
    floor: u32,
) -> Option<String> {
    let floor_steps: Vec<&SlayTheDataReplayStep> = plan
        .steps
        .iter()
        .filter(|candidate| candidate.floor == floor)
        .collect();
    if floor_steps.is_empty() {
        return None;
    }

    match next.phase {
        RunPhase::Event => {
            let event = next.event.as_ref()?;
            for step in &floor_steps {
                if let SlayTheDataReplayStepKind::EventChoice {
                    event_name: Some(event_name),
                    ..
                } = &step.kind
                {
                    if event_name_matches(event.event, event_name) {
                        return Some(format!("recorded event {event_name:?}"));
                    }
                }
            }
            None
        }
        RunPhase::Reward => {
            let reward = next.reward.as_ref()?;
            if floor_steps.iter().any(|step| {
                matches!(
                    step.kind,
                    SlayTheDataReplayStepKind::CardReward { .. }
                        | SlayTheDataReplayStepKind::PotionBudget { .. }
                )
            }) {
                return Some("recorded reward/card-reward evidence".to_owned());
            }
            if reward.relic_offer.is_some()
                && floor_steps
                    .iter()
                    .any(|step| matches!(step.kind, SlayTheDataReplayStepKind::BossRelic { .. }))
            {
                return Some("recorded relic reward evidence".to_owned());
            }
            None
        }
        RunPhase::Shop => {
            if floor_steps.iter().any(|step| {
                matches!(
                    step.kind,
                    SlayTheDataReplayStepKind::ShopPurchase { .. }
                        | SlayTheDataReplayStepKind::ShopPurge { .. }
                )
            }) {
                return Some("recorded shop purchase evidence".to_owned());
            }
            None
        }
        RunPhase::Rest => {
            if floor_steps
                .iter()
                .any(|step| matches!(step.kind, SlayTheDataReplayStepKind::Campfire { .. }))
            {
                return Some("recorded campfire evidence".to_owned());
            }
            None
        }
        RunPhase::Combat => {
            let combat = next.combat.as_ref()?;
            for step in &floor_steps {
                if let SlayTheDataReplayStepKind::CombatEncounter {
                    enemies: Some(enemies),
                } = &step.kind
                {
                    if combat_encounter_matches(combat, enemies) {
                        return Some(format!("recorded combat encounter {enemies:?}"));
                    }
                }
            }
            if floor_steps.iter().any(|step| {
                matches!(
                    step.kind,
                    SlayTheDataReplayStepKind::CardReward { .. }
                        | SlayTheDataReplayStepKind::PotionBudget { .. }
                )
            }) {
                return Some("recorded combat reward/potion evidence".to_owned());
            }
            None
        }
        RunPhase::Treasure | RunPhase::Idle | RunPhase::Complete => None,
    }
}

fn event_name_matches(event: sts_core::Event, slaythedata_name: &str) -> bool {
    normalized_event_name(event).iter().any(|candidate| {
        normalize_slaythedata_label(candidate) == normalize_slaythedata_label(slaythedata_name)
    })
}

fn combat_encounter_matches(combat: &sts_core::CombatState, slaythedata_enemies: &str) -> bool {
    let target = normalize_slaythedata_label(slaythedata_enemies);
    combat_encounter_labels(combat)
        .iter()
        .any(|label| normalize_slaythedata_label(label) == target)
}

fn combat_encounter_labels(combat: &sts_core::CombatState) -> Vec<String> {
    let names: Vec<String> = combat
        .monsters
        .iter()
        .filter_map(|monster| {
            get_monster_definition(monster.content_id).map(|definition| definition.name.to_owned())
        })
        .collect();
    let mut labels = Vec::new();
    if names.is_empty() {
        return labels;
    }
    labels.push(names.join(" and "));
    labels.push(names.join(", "));

    if names.len() == 1 {
        labels.push(names[0].clone());
    }
    if names.len() == 2 && names.iter().all(|name| name.contains("Louse")) {
        labels.push("2 Louse".to_owned());
    }
    if names.len() == 3 && names.iter().all(|name| name == "Sentry") {
        labels.push("3 Sentries".to_owned());
    }
    if names.iter().all(|name| name.contains("Slime")) {
        labels.push("Small Slimes".to_owned());
    }
    if names.len() == 2
        && names.iter().any(|name| name == "Centurion")
        && names
            .iter()
            .any(|name| name == "Mystic" || name == "Healer")
    {
        labels.push("Centurion and Healer".to_owned());
    }
    if names.len() == 2
        && names.iter().any(|name| name == "Shelled Parasite")
        && names.iter().any(|name| name == "Fungi Beast")
    {
        labels.push("Shelled Parasite and Fungi".to_owned());
    }
    if names.len() == 2
        && names.iter().any(|name| name == "Chosen")
        && names.iter().any(|name| name == "Byrd")
    {
        labels.push("Chosen and Byrds".to_owned());
    }
    if names.len() == 3 && names.iter().all(|name| name == "Darkling") {
        labels.push("3 Darklings".to_owned());
    }
    labels
}

fn normalized_event_name(event: sts_core::Event) -> Vec<String> {
    let debug = format!("{event:?}");
    let spaced = debug
        .chars()
        .enumerate()
        .fold(String::new(), |mut out, (index, ch)| {
            if index > 0 && ch.is_ascii_uppercase() {
                out.push(' ');
            }
            out.push(ch);
            out
        });
    let known: &[&str] = match event {
        sts_core::Event::BonfireElementals => &["Bonfire Elementals"],
        sts_core::Event::Designer => &["Designer"],
        sts_core::Event::Duplicator => &["Duplicator"],
        sts_core::Event::AccursedBlacksmith => &["Accursed Blacksmith", "Ominous Forge"],
        sts_core::Event::FountainOfCleansing => &["Fountain of Cleansing", "The Divine Fountain"],
        sts_core::Event::GoldenShrine => &["Golden Shrine"],
        sts_core::Event::BigFish => &["Big Fish"],
        sts_core::Event::TheCleric => &["The Cleric"],
        sts_core::Event::DeadAdventurer => &["Dead Adventurer"],
        sts_core::Event::GoldenIdol => &["Golden Idol"],
        sts_core::Event::WorldOfGoop => &["World of Goop"],
        sts_core::Event::LivingWall => &["Living Wall"],
        sts_core::Event::ScrapOoze => &["Scrap Ooze"],
        sts_core::Event::FaceTrader => &["Face Trader", "FaceTrader"],
        sts_core::Event::Nloth => &["N'loth"],
        sts_core::Event::NoteForYourself => &["A Note For Yourself", "NoteForYourself"],
        sts_core::Event::SecretPortal => &["Secret Portal", "SecretPortal"],
        sts_core::Event::TheJoust => &["The Joust"],
        sts_core::Event::TheWomanInBlue => &["The Woman in Blue"],
        sts_core::Event::Transmorgrifier => &["Transmogrifier", "Transmorgrifier"],
        sts_core::Event::Purifier => &["Purifier"],
        sts_core::Event::UpgradeShrine => &["Upgrade Shrine"],
        sts_core::Event::MatchAndKeep => &["Match and Keep!", "Match and Keep"],
        sts_core::Event::Addict => &["Addict"],
        sts_core::Event::BackToBasics => &["Back to Basics"],
        sts_core::Event::Beggar => &["Beggar"],
        sts_core::Event::Colosseum => &["Colosseum"],
        sts_core::Event::CursedTome => &["Cursed Tome"],
        sts_core::Event::DrugDealer => &["Drug Dealer"],
        sts_core::Event::ForgottenAltar => &["Forgotten Altar"],
        sts_core::Event::Ghosts => &["Council of Ghosts", "Ghosts"],
        sts_core::Event::KnowingSkull => &["Knowing Skull"],
        sts_core::Event::MaskedBandits => &["Masked Bandits"],
        sts_core::Event::Nest => &["The Nest", "Nest"],
        sts_core::Event::TheLibrary => &["The Library"],
        sts_core::Event::TheMausoleum => &["The Mausoleum"],
        sts_core::Event::Vampires => &["Vampires", "Vampires(?)"],
        sts_core::Event::Lab => &["Lab"],
        sts_core::Event::Falling => &["Falling"],
        sts_core::Event::MindBloom => &["Mind Bloom", "MindBloom"],
        sts_core::Event::MysteriousSphere => &["Mysterious Sphere"],
        sts_core::Event::SensoryStone => &["Sensory Stone", "SensoryStone"],
        sts_core::Event::TombOfLordRedMask => &["Tomb of Lord Red Mask"],
        sts_core::Event::WindingHalls => &["Winding Halls"],
        sts_core::Event::TheSsssserpent => &["The Sssserpent"],
        sts_core::Event::HypnotizingColoredMushrooms => &["Hypnotizing Colored Mushrooms"],
        sts_core::Event::WheelOfChange => &["Wheel of Change"],
        sts_core::Event::WingStatue => &["Wing Statue", "Golden Wing"],
        sts_core::Event::ShiningLight => &["Shining Light"],
        sts_core::Event::MoaiHead => &["The Moai Head"],
        _ => &[],
    };
    let mut names = vec![debug, spaced];
    names.extend(known.iter().map(|name| (*name).to_owned()));
    names
}

fn normalize_slaythedata_label(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn slaythedata_seed_to_long(seed: &str) -> Result<i64, String> {
    let trimmed = seed.trim();
    if trimmed.is_empty() {
        return Err("seed_played is empty".to_owned());
    }
    if trimmed
        .strip_prefix('-')
        .unwrap_or(trimmed)
        .chars()
        .all(|ch| ch.is_ascii_digit())
    {
        return trimmed
            .parse::<i64>()
            .map_err(|error| format!("invalid numeric seed_played {trimmed:?}: {error}"));
    }
    try_sts_seed_string_to_long(trimmed).map_err(|error| error.to_string())
}

fn choose_visible_hint(option_slot: usize) -> SlayTheDataBridgeCommandHint {
    SlayTheDataBridgeCommandHint {
        descriptor: SlayTheDataBridgeDescriptor::ChooseVisibleOption { option_slot },
        command: format!("CHOOSE {option_slot}"),
    }
}

fn skip_visible_reward_hint() -> SlayTheDataBridgeCommandHint {
    SlayTheDataBridgeCommandHint {
        descriptor: SlayTheDataBridgeDescriptor::SkipVisibleReward,
        command: "SKIP".to_owned(),
    }
}

fn slaythedata_card_reward_action(
    run: &RunState,
    picked: &Option<SlayTheDataCardName>,
    skipped: bool,
) -> Result<(RunAction, SlayTheDataBridgeCommandHint), String> {
    let Some(reward) = run.reward.as_ref() else {
        return Err("card reward cannot be checked because reward screen is missing".to_owned());
    };
    if !reward.card_reward_active {
        return Err(
            "card reward cannot be checked because the card reward screen is not open".to_owned(),
        );
    }
    if skipped {
        return Ok((RunAction::SkipReward, skip_visible_reward_hint()));
    }
    let Some(picked) = picked.as_ref() else {
        return Err("card reward has no picked card and was not marked skipped".to_owned());
    };
    let matches: Vec<_> = reward
        .choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| {
            get_card_definition(choice.content_id).is_some_and(|definition| {
                definition.name.eq_ignore_ascii_case(&picked.raw)
                    || definition.name.eq_ignore_ascii_case(&picked.base)
            })
        })
        .collect();
    match matches.as_slice() {
        [(slot, choice)] => Ok((
            RunAction::TakeCardReward { card_id: choice.id },
            choose_visible_hint(*slot),
        )),
        [] => Err(format!(
            "card reward picked {:?} is not among the current core reward choices",
            picked.raw
        )),
        _ => Err(format!(
            "card reward picked {:?} matched multiple current core reward choices",
            picked.raw
        )),
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
        "THREE_RARE_CARDS" => Some(NeowRewardType::ThreeRareCards),
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

fn map_action_matches_symbol(run: &RunState, action: MapAction, symbol: &str) -> bool {
    map_action_room_kind(run, action).is_some_and(|kind| {
        Some(room_kind_symbol(kind)) == normalize_route_symbol(symbol).as_deref()
    })
}

fn map_action_room_kind(run: &RunState, action: MapAction) -> Option<RoomKind> {
    let MapAction::ChooseNode { node_id } = action;
    run.map
        .as_ref()
        .and_then(|map| map.map.node(node_id))
        .map(|node| node.room_kind)
}

fn room_kind_symbol(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Combat => "M",
        RoomKind::Elite => "E",
        RoomKind::Event => "?",
        RoomKind::Rest => "R",
        RoomKind::Shop => "$",
        RoomKind::Treasure => "T",
        RoomKind::Boss => "B",
        RoomKind::Victory => "V",
    }
}

fn normalize_route_symbol(symbol: &str) -> Option<String> {
    let normalized = symbol.trim().trim_matches('"').trim().to_ascii_uppercase();
    (!normalized.is_empty()).then_some(normalized)
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
                cards_transformed: card_name_list(choice.get("cards_transformed")),
                cards_upgraded: card_name_list(choice.get("cards_upgraded")),
                relics_obtained: string_list(choice.get("relics_obtained")),
                relics_lost: string_list(choice.get("relics_lost")),
            });
    }
}

fn import_combat_encounters(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    for (ordinal, combat) in array(event.get("damage_taken")).iter().enumerate() {
        let Some(floor) = parse_positive_floor(combat.get("floor")) else {
            continue;
        };
        floor_entry(floors, floor)
            .combats
            .push(SlayTheDataCombatEncounter {
                ordinal,
                enemies: optional_string(combat.get("enemies")),
                damage: parse_i32(combat.get("damage")),
                turns: parse_i32(combat.get("turns")),
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

fn import_shop_purges(
    event: &serde_json::Map<String, Value>,
    floors: &mut BTreeMap<u32, SlayTheDataFloorDecision>,
) {
    let purge_floors = array(event.get("items_purged_floors"));
    for (ordinal, card) in array(event.get("items_purged")).iter().enumerate() {
        let Some(floor) = purge_floors
            .get(ordinal)
            .and_then(|value| parse_positive_floor(Some(value)))
        else {
            continue;
        };
        if let Some(card) = card_name(Some(card)) {
            floor_entry(floors, floor).shop_purges.push(card);
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
    let taken = string_list(event.get("path_taken"));
    if !taken.is_empty() {
        let mut floor = 1_u32;
        for route in taken {
            let route = normalize_route_atom(&route);
            floor_entry(floors, floor).route = Some(route.clone());
            floor += 1;
            if route == "B" {
                floor += 1;
            }
        }
        return;
    }
    let per_floor = string_list_preserving_empty(event.get("path_per_floor"));
    if !per_floor.is_empty() {
        for (index, route) in per_floor.into_iter().enumerate() {
            if !route.trim().is_empty() {
                floor_entry(floors, index as u32 + 1).route = Some(normalize_route_atom(&route));
            }
        }
    }
}

fn normalize_route_atom(route: &str) -> String {
    match route.trim().to_ascii_uppercase().as_str() {
        "BOSS" => "B".to_owned(),
        other => other.to_owned(),
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

    let unsupported_modes = [
        ("is_beta", imported.config.is_beta == Some(true)),
        ("is_daily", imported.config.is_daily == Some(true)),
        ("is_endless", imported.config.is_endless == Some(true)),
        ("is_prod", imported.config.is_prod == Some(true)),
        ("is_trial", imported.config.is_trial == Some(true)),
    ];
    for (field, unsupported) in unsupported_modes {
        if unsupported {
            diagnostics.push(SlayTheDataDiagnostic {
                severity: SlayTheDataDiagnosticSeverity::Error,
                code: "unsupported_run_mode".to_owned(),
                path: format!("$.{field}"),
                message: format!("{field} marks this as a non-standard production run"),
            });
        }
    }

    if imported
        .final_observed
        .floor_reached
        .is_some_and(|floor| floor > SLAYTHEDATA_NORMAL_MAX_FLOOR_REACHED)
    {
        diagnostics.push(SlayTheDataDiagnostic {
            severity: SlayTheDataDiagnosticSeverity::Error,
            code: "unsupported_floor_reached".to_owned(),
            path: "$.floor_reached".to_owned(),
            message: format!(
                "normal non-endless Slay the Spire runs should not exceed floor_reached {}",
                SLAYTHEDATA_NORMAL_MAX_FLOOR_REACHED
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
            event.cards_obtained.len()
                + event.cards_removed.len()
                + event.cards_transformed.len()
                + event.cards_upgraded.len()
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
            combats: Vec::new(),
            card_rewards: Vec::new(),
            relics_obtained: Vec::new(),
            events: Vec::new(),
            shop_purchases: Vec::new(),
            shop_purges: Vec::new(),
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

fn string_list_preserving_empty(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| match value {
            Value::String(text) => text.trim().to_owned(),
            Value::Null => String::new(),
            other => other.to_string(),
        })
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
        Value::Number(number) => number.as_i64().or_else(|| {
            let float = number.as_f64()?;
            (float.fract() == 0.0).then_some(float as i64)
        }),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn parse_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn route_import_preserves_act_transition_floor_slots() {
        let value = json!({"path_per_floor": ["M", "BOSS", "", "?"]});
        let mut floors = BTreeMap::new();
        import_route(value.as_object().unwrap(), &mut floors);

        assert_eq!(
            floors.get(&1).and_then(|floor| floor.route.as_deref()),
            Some("M")
        );
        assert_eq!(
            floors.get(&2).and_then(|floor| floor.route.as_deref()),
            Some("B")
        );
        assert!(!floors.contains_key(&3));
        assert_eq!(
            floors.get(&4).and_then(|floor| floor.route.as_deref()),
            Some("?")
        );
    }

    #[test]
    fn compressed_path_taken_inserts_floor_after_each_boss() {
        let value = json!({"path_taken": ["M", "BOSS", "?", "BOSS", "R"]});
        let mut floors = BTreeMap::new();
        import_route(value.as_object().unwrap(), &mut floors);

        assert_eq!(
            floors.get(&4).and_then(|floor| floor.route.as_deref()),
            Some("?")
        );
        assert_eq!(
            floors.get(&5).and_then(|floor| floor.route.as_deref()),
            Some("B")
        );
        assert_eq!(
            floors.get(&7).and_then(|floor| floor.route.as_deref()),
            Some("R")
        );
    }

    #[test]
    fn traversed_path_wins_when_per_floor_route_disagrees() {
        let value = json!({
            "path_taken": ["M", "BOSS", "?"],
            "path_per_floor": ["M", "BOSS", null, "$"]
        });
        let mut floors = BTreeMap::new();
        import_route(value.as_object().unwrap(), &mut floors);

        assert_eq!(
            floors.get(&4).and_then(|floor| floor.route.as_deref()),
            Some("?")
        );
    }

    fn empty_plan_with_step(floor: u32, kind: SlayTheDataReplayStepKind) -> SlayTheDataReplayPlan {
        SlayTheDataReplayPlan {
            schema: SLAYTHEDATA_IMPORT_SCHEMA_VERSION,
            source: SlayTheDataSource {
                kind: SlayTheDataSourceKind::RawRun,
                run_id: None,
                play_id: None,
                source_file: None,
                source_run_ordinal: None,
            },
            run_start: None,
            ordering: SlayTheDataReplayOrdering::FloorGrouped,
            steps: vec![SlayTheDataReplayStep {
                floor,
                ordinal: 0,
                kind,
            }],
            checkpoints: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn same_floor_event_precedes_its_post_combat_card_reward() {
        let imported = SlayTheDataRunImport {
            schema: 1,
            source: SlayTheDataSource {
                kind: SlayTheDataSourceKind::RawRun,
                run_id: Some(1),
                play_id: None,
                source_file: None,
                source_run_ordinal: None,
            },
            config: SlayTheDataRunConfig {
                character: None,
                ascension: None,
                build_version: None,
                seed_played: None,
                seed_source_timestamp: None,
                special_seed: None,
                neow_bonus: None,
                neow_cost: None,
                is_beta: None,
                is_daily: None,
                is_endless: None,
                is_prod: None,
                is_trial: None,
            },
            replay_policy: SlayTheDataReplayPolicy {
                mode: "guided".to_owned(),
                exact_combat_actions: false,
                on_illegal_high_level_choice: "stop".to_owned(),
                on_legal_divergence: "stop".to_owned(),
                potion_budget_mode: "floor".to_owned(),
            },
            route: SlayTheDataRoute {
                path_taken: Vec::new(),
                path_per_floor: Vec::new(),
            },
            floor_decisions: vec![SlayTheDataFloorDecision {
                floor: 8,
                route: None,
                combats: Vec::new(),
                card_rewards: vec![SlayTheDataCardReward {
                    ordinal: 2,
                    picked: Some(SlayTheDataCardName {
                        raw: "Iron Wave".to_owned(),
                        base: "Iron Wave".to_owned(),
                        upgraded: false,
                    }),
                    not_picked: Vec::new(),
                    skipped: false,
                }],
                relics_obtained: Vec::new(),
                events: vec![SlayTheDataEventChoice {
                    ordinal: 1,
                    event_name: Some("Dead Adventurer".to_owned()),
                    player_choice: Some("Searched '2' times".to_owned()),
                    damage_taken: None,
                    damage_healed: None,
                    max_hp_gain: None,
                    max_hp_loss: None,
                    gold_gain: None,
                    gold_loss: None,
                    cards_obtained: Vec::new(),
                    cards_removed: Vec::new(),
                    cards_transformed: Vec::new(),
                    cards_upgraded: Vec::new(),
                    relics_obtained: Vec::new(),
                    relics_lost: Vec::new(),
                }],
                shop_purchases: Vec::new(),
                shop_purges: Vec::new(),
                campfires: Vec::new(),
                potions: SlayTheDataPotionFloorDecision::default(),
            }],
            boss_relic_choices: Vec::new(),
            final_observed: SlayTheDataFinalObserved {
                floor_reached: Some(8),
                victory: false,
                master_deck: Vec::new(),
                relics: Vec::new(),
                gold: None,
            },
            diagnostics: Vec::new(),
        };

        let plan = slaythedata_replay_plan(&imported);
        assert!(matches!(
            plan.steps[0].kind,
            SlayTheDataReplayStepKind::EventChoice { .. }
        ));
        assert!(matches!(
            plan.steps[1].kind,
            SlayTheDataReplayStepKind::CardReward { .. }
        ));
    }

    #[test]
    fn shop_orrery_purchase_precedes_its_same_floor_card_rewards() {
        let imported = import_slaythedata_run_value(&json!({
            "path_per_floor": ["$"],
            "card_choices": [
                {"floor": 1, "picked": "Barricade", "not_picked": ["Cleave", "Entrench"]},
                {"floor": 1, "picked": "Headbutt", "not_picked": ["Armaments", "Clothesline"]}
            ],
            "items_purchased": ["Orrery", "Membership Card"],
            "item_purchase_floors": [1, 1]
        }))
        .expect("SlayTheData row imports");

        let plan = slaythedata_replay_plan(&imported);
        assert!(matches!(
            plan.steps[0].kind,
            SlayTheDataReplayStepKind::MapRoom { .. }
        ));
        assert!(matches!(
            &plan.steps[1].kind,
            SlayTheDataReplayStepKind::ShopPurchase { item, .. } if item == "Orrery"
        ));
        assert!(matches!(
            plan.steps[2].kind,
            SlayTheDataReplayStepKind::CardReward { .. }
        ));
        assert!(matches!(
            plan.steps[3].kind,
            SlayTheDataReplayStepKind::CardReward { .. }
        ));
        assert!(matches!(
            &plan.steps[4].kind,
            SlayTheDataReplayStepKind::ShopPurchase { item, .. } if item == "Membership Card"
        ));
    }

    #[test]
    fn event_name_matching_accepts_slaythedata_display_names() {
        assert!(event_name_matches(
            sts_core::Event::WheelOfChange,
            "Wheel of Change"
        ));
        assert!(event_name_matches(
            sts_core::Event::HypnotizingColoredMushrooms,
            "Hypnotizing Colored Mushrooms"
        ));
        assert!(event_name_matches(
            sts_core::Event::TheSsssserpent,
            "The Ssssserpent"
        ));
        assert!(event_name_matches(
            sts_core::Event::WingStatue,
            "Golden Wing"
        ));
        assert!(event_name_matches(
            sts_core::Event::FountainOfCleansing,
            "The Divine Fountain"
        ));
        assert!(event_name_matches(
            sts_core::Event::MoaiHead,
            "The Moai Head"
        ));
        assert!(event_name_matches(
            sts_core::Event::AccursedBlacksmith,
            "Ominous Forge"
        ));
        assert!(event_name_matches(
            sts_core::Event::Ghosts,
            "Council of Ghosts"
        ));
        assert!(event_name_matches(sts_core::Event::Nest, "The Nest"));
        assert!(event_name_matches(
            sts_core::Event::NoteForYourself,
            "A Note For Yourself"
        ));
        assert!(event_name_matches(
            sts_core::Event::Transmorgrifier,
            "Transmogrifier"
        ));
        assert!(event_name_matches(sts_core::Event::Vampires, "Vampires(?)"));
    }

    #[test]
    fn knowing_skull_record_expands_into_individual_live_choices_and_leave() {
        let event = SlayTheDataEventChoice {
            ordinal: 0,
            event_name: Some("Knowing Skull".to_owned()),
            player_choice: Some("POTION GOLD CARD ".to_owned()),
            damage_taken: None,
            damage_healed: None,
            max_hp_gain: None,
            max_hp_loss: None,
            gold_gain: None,
            gold_loss: None,
            cards_obtained: Vec::new(),
            cards_removed: Vec::new(),
            cards_transformed: Vec::new(),
            cards_upgraded: Vec::new(),
            relics_obtained: Vec::new(),
            relics_lost: Vec::new(),
        };

        assert_eq!(
            knowing_skull_sequence_choices(&event),
            vec![
                Some("POTION".to_owned()),
                Some("GOLD".to_owned()),
                Some("CARD".to_owned()),
                Some("LEAVE".to_owned()),
            ]
        );

        for choice in knowing_skull_sequence_choices(&event) {
            let plan = empty_plan_with_step(
                12,
                SlayTheDataReplayStepKind::EventChoice {
                    event_name: event.event_name.clone(),
                    player_choice: choice,
                    cards_obtained: Vec::new(),
                    cards_removed: Vec::new(),
                    cards_transformed: Vec::new(),
                    cards_upgraded: Vec::new(),
                    relics_obtained: Vec::new(),
                    relics_lost: Vec::new(),
                },
            );
            let report = slaythedata_replay_preflight(&plan);
            assert_eq!(report.steps[0].code, "guided_event_sequence");
        }
    }

    #[test]
    fn event_name_matching_covers_all_recorded_event_names() {
        let recorded_names = [
            (sts_core::Event::Neow, "Neow"),
            (sts_core::Event::AccursedBlacksmith, "Ominous Forge"),
            (sts_core::Event::BonfireElementals, "Bonfire Elementals"),
            (sts_core::Event::Designer, "Designer"),
            (sts_core::Event::Duplicator, "Duplicator"),
            (sts_core::Event::FountainOfCleansing, "The Divine Fountain"),
            (sts_core::Event::GoldenShrine, "Golden Shrine"),
            (sts_core::Event::BigFish, "Big Fish"),
            (sts_core::Event::TheCleric, "The Cleric"),
            (sts_core::Event::DeadAdventurer, "Dead Adventurer"),
            (sts_core::Event::GoldenIdol, "Golden Idol"),
            (sts_core::Event::WingStatue, "Wing Statue"),
            (sts_core::Event::WorldOfGoop, "World of Goop"),
            (sts_core::Event::TheSsssserpent, "The Sssserpent"),
            (sts_core::Event::LivingWall, "Living Wall"),
            (
                sts_core::Event::HypnotizingColoredMushrooms,
                "Hypnotizing Colored Mushrooms",
            ),
            (sts_core::Event::ScrapOoze, "Scrap Ooze"),
            (sts_core::Event::ShiningLight, "Shining Light"),
            (sts_core::Event::FaceTrader, "Face Trader"),
            (sts_core::Event::Nloth, "N'loth"),
            (sts_core::Event::NoteForYourself, "A Note For Yourself"),
            (sts_core::Event::SecretPortal, "Secret Portal"),
            (sts_core::Event::TheJoust, "The Joust"),
            (sts_core::Event::WeMeetAgain, "We Meet Again!"),
            (sts_core::Event::TheWomanInBlue, "The Woman in Blue"),
            (sts_core::Event::Transmorgrifier, "Transmogrifier"),
            (sts_core::Event::Purifier, "Purifier"),
            (sts_core::Event::UpgradeShrine, "Upgrade Shrine"),
            (sts_core::Event::WheelOfChange, "Wheel of Change"),
            (sts_core::Event::MatchAndKeep, "Match and Keep!"),
            (sts_core::Event::Addict, "Addict"),
            (sts_core::Event::BackToBasics, "Back to Basics"),
            (sts_core::Event::Beggar, "Beggar"),
            (sts_core::Event::Colosseum, "Colosseum"),
            (sts_core::Event::CursedTome, "Cursed Tome"),
            (sts_core::Event::DrugDealer, "Drug Dealer"),
            (sts_core::Event::ForgottenAltar, "Forgotten Altar"),
            (sts_core::Event::Ghosts, "Council of Ghosts"),
            (sts_core::Event::KnowingSkull, "Knowing Skull"),
            (sts_core::Event::MaskedBandits, "Masked Bandits"),
            (sts_core::Event::Nest, "The Nest"),
            (sts_core::Event::TheLibrary, "The Library"),
            (sts_core::Event::TheMausoleum, "The Mausoleum"),
            (sts_core::Event::Vampires, "Vampires(?)"),
            (sts_core::Event::Lab, "Lab"),
            (sts_core::Event::Falling, "Falling"),
            (sts_core::Event::MindBloom, "Mind Bloom"),
            (sts_core::Event::MoaiHead, "The Moai Head"),
            (sts_core::Event::MysteriousSphere, "Mysterious Sphere"),
            (sts_core::Event::SensoryStone, "Sensory Stone"),
            (sts_core::Event::TombOfLordRedMask, "Tomb of Lord Red Mask"),
            (sts_core::Event::WindingHalls, "Winding Halls"),
        ];

        for (event, recorded_name) in recorded_names {
            assert!(
                event_name_matches(event, recorded_name),
                "missing SlayTheData name mapping for {event:?} / {recorded_name}"
            );
        }
    }

    #[test]
    fn map_candidate_evidence_uses_recorded_event_identity() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(sts_core::EventScreen {
            event: sts_core::Event::WheelOfChange,
            choices: Vec::new(),
            stage: 0,
            event_data: 0,
        });
        let plan = empty_plan_with_step(
            3,
            SlayTheDataReplayStepKind::EventChoice {
                event_name: Some("Wheel of Change".to_owned()),
                player_choice: Some("Play".to_owned()),
                cards_obtained: Vec::new(),
                cards_removed: Vec::new(),
                cards_transformed: Vec::new(),
                cards_upgraded: Vec::new(),
                relics_obtained: Vec::new(),
                relics_lost: Vec::new(),
            },
        );

        let evidence = slaythedata_map_candidate_evidence(&run, &plan, 3);

        assert_eq!(
            evidence,
            Some("recorded event \"Wheel of Change\"".to_owned())
        );
    }

    #[test]
    fn preflight_event_choice_message_includes_grid_effect_cards() {
        let plan = empty_plan_with_step(
            3,
            SlayTheDataReplayStepKind::EventChoice {
                event_name: Some("The Cleric".to_owned()),
                player_choice: Some("Card Removal".to_owned()),
                cards_obtained: Vec::new(),
                cards_removed: vec![SlayTheDataCardName {
                    raw: "Strike".to_owned(),
                    base: "Strike".to_owned(),
                    upgraded: false,
                }],
                cards_transformed: vec![SlayTheDataCardName {
                    raw: "Defend_R".to_owned(),
                    base: "Defend_R".to_owned(),
                    upgraded: false,
                }],
                cards_upgraded: Vec::new(),
                relics_obtained: Vec::new(),
                relics_lost: Vec::new(),
            },
        );

        let report = slaythedata_replay_preflight(&plan);

        assert!(report.steps[0].message.contains("removed [\"Strike\"]"));
        assert!(report.steps[0]
            .message
            .contains("transformed [\"Defend_R\"]"));
    }

    #[test]
    fn map_candidate_evidence_rejects_wrong_event_identity() {
        let mut run = RunState::seeded_ironclad(1, 0);
        run.phase = RunPhase::Event;
        run.event = Some(sts_core::EventScreen {
            event: sts_core::Event::GoldenShrine,
            choices: Vec::new(),
            stage: 0,
            event_data: 0,
        });
        let plan = empty_plan_with_step(
            3,
            SlayTheDataReplayStepKind::EventChoice {
                event_name: Some("Wheel of Change".to_owned()),
                player_choice: Some("Play".to_owned()),
                cards_obtained: Vec::new(),
                cards_removed: Vec::new(),
                cards_transformed: Vec::new(),
                cards_upgraded: Vec::new(),
                relics_obtained: Vec::new(),
                relics_lost: Vec::new(),
            },
        );

        let evidence = slaythedata_map_candidate_evidence(&run, &plan, 3);

        assert_eq!(evidence, None);
    }
}
