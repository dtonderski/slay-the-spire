//! Visibility-safe, strongly typed observations for run decisions.

use crate::combat_observation::{
    fair_combat_observation, potion_key, project_card, FairCard, FairCombatObservation,
    FairObservationError,
};
use serde::{Deserialize, Serialize};
use sts_core::adapter_internals::{
    content::cards::get_card_definition,
    map::RoomKind,
    run::{reward::ChestSize, CardRewardFlow, GridPurpose, RunState},
    CardInstance, RestAction, RunPhase,
};

pub const FAIR_RUN_OBSERVATION_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRunObservation {
    pub schema_version: u32,
    pub phase: FairRunPhase,
    pub context: FairRunContext,
    pub screen: FairRunScreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairRunPhase {
    Combat,
    Reward,
    Treasure,
    Rest,
    Event,
    Shop,
    Idle,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRunContext {
    pub ascension: u8,
    pub act: i32,
    pub floor: i32,
    pub gold: i32,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub deck: Vec<FairCard>,
    pub relics: Vec<FairRunRelic>,
    pub potion_slots: Vec<FairRunPotionSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRunRelic {
    pub slot: usize,
    pub content_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRunPotionSlot {
    pub slot: usize,
    pub content_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum FairRunScreen {
    Combat(FairCombatObservation),
    Map(FairMapObservation),
    Event(FairEventObservation),
    Reward(FairRewardObservation),
    Treasure(FairTreasureObservation),
    Rest(FairRestObservation),
    Shop(FairShopObservation),
    Grid(FairGridObservation),
    Idle,
    Complete,
}

impl FairRunScreen {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Combat(_) => "combat",
            Self::Map(_) => "map",
            Self::Event(_) => "event",
            Self::Reward(_) => "reward",
            Self::Treasure(_) => "treasure",
            Self::Rest(_) => "rest",
            Self::Shop(_) => "shop",
            Self::Grid(_) => "grid",
            Self::Idle => "idle",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairMapObservation {
    pub act: u8,
    pub floor: u32,
    pub current_node: usize,
    pub reachable_nodes: Vec<usize>,
    pub nodes: Vec<FairMapNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairMapNode {
    pub slot: usize,
    pub act: u8,
    pub room_kind: String,
    pub children: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairEventObservation {
    pub event: String,
    pub choices: Vec<FairEventChoice>,
    pub match_and_keep: Option<Vec<FairMatchAndKeepCard>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairEventChoice {
    pub slot: usize,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairMatchAndKeepCard {
    pub content_key: Option<String>,
    pub revealed: bool,
    pub matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRewardObservation {
    pub cards: Vec<FairCardSlot>,
    pub queued_card_rewards: Vec<FairQueuedCardReward>,
    pub gold_offer: i32,
    pub stolen_gold_offer: i32,
    pub potion_offer: Option<String>,
    pub potion_offers: Vec<String>,
    pub relic_offer: Option<String>,
    pub boss_relic_choices: Vec<String>,
    pub card_reward_flow: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairCardSlot {
    pub slot: usize,
    pub card: FairCard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairQueuedCardReward {
    pub slot: usize,
    pub choice_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairTreasureObservation {
    pub chest_size: String,
    pub opened: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairRestObservation {
    pub complete: bool,
    pub options: Vec<FairRestOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FairRestOption {
    Heal,
    OpenSmith,
    OpenRemove,
    Smith { card_slot: usize },
    RemoveCard { card_slot: usize },
    Lift,
    Dig,
    Recall,
    Proceed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairShopObservation {
    pub merchant_open: bool,
    pub remove_cost: Option<i32>,
    pub cards: Vec<FairShopCard>,
    pub relics: Vec<FairShopRelic>,
    pub potions: Vec<FairShopPotion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairShopCard {
    pub slot: usize,
    pub content_key: String,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairShopRelic {
    pub slot: usize,
    pub content_key: String,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairShopPotion {
    pub slot: usize,
    pub content_key: String,
    pub price: i32,
    pub sold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FairGridObservation {
    pub purpose: String,
    pub cards: Vec<FairCardSlot>,
    pub selected: Option<usize>,
    pub selected_indices: Vec<usize>,
}

pub fn fair_run_observation(run: &RunState) -> Result<FairRunObservation, FairObservationError> {
    let screen = if run.card_grid.is_some() {
        FairRunScreen::Grid(grid_screen(run)?)
    } else {
        match run.phase {
            RunPhase::Combat => FairRunScreen::Combat(fair_combat_observation(run)?),
            RunPhase::Reward => FairRunScreen::Reward(reward_screen(run)?),
            RunPhase::Treasure => FairRunScreen::Treasure(treasure_screen(run)?),
            RunPhase::Rest => FairRunScreen::Rest(rest_screen(run)?),
            RunPhase::Event => FairRunScreen::Event(event_screen(run)?),
            RunPhase::Shop => FairRunScreen::Shop(shop_screen(run)?),
            RunPhase::Idle if run.map.is_some() => FairRunScreen::Map(map_screen(run)?),
            RunPhase::Idle => FairRunScreen::Idle,
            RunPhase::Victory | RunPhase::Complete => FairRunScreen::Complete,
        }
    };
    Ok(FairRunObservation {
        schema_version: FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
        phase: run.phase.into(),
        context: public_context(run)?,
        screen,
    })
}

fn public_context(run: &RunState) -> Result<FairRunContext, FairObservationError> {
    Ok(FairRunContext {
        ascension: run.ascension,
        act: run.current_act,
        floor: run.current_floor,
        gold: run.gold,
        player_hp: run.hp,
        player_max_hp: run.max_hp,
        deck: run
            .deck
            .iter()
            .map(|card| project_card(card, false))
            .collect::<Result<_, _>>()?,
        relics: run
            .relics
            .iter()
            .enumerate()
            .map(|(slot, relic)| FairRunRelic {
                slot,
                content_key: relic.trace_name().to_owned(),
            })
            .collect(),
        potion_slots: (0..run.potion_capacity())
            .map(|slot| FairRunPotionSlot {
                slot,
                content_key: run.potion_at_slot(slot).map(potion_key).map(str::to_owned),
            })
            .collect(),
    })
}

fn map_screen(run: &RunState) -> Result<FairMapObservation, FairObservationError> {
    let map = run
        .map
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    let index = map
        .map
        .nodes
        .iter()
        .enumerate()
        .map(|(slot, node)| (node.id, slot))
        .collect::<std::collections::BTreeMap<_, _>>();
    Ok(FairMapObservation {
        act: map.act,
        floor: map.floor,
        current_node: *index
            .get(&map.current_node)
            .ok_or(FairObservationError::InvalidAuthoritativeState)?,
        reachable_nodes: map
            .map
            .children_of(map.current_node)
            .map_err(|_| FairObservationError::InvalidAuthoritativeState)?
            .iter()
            .filter_map(|id| index.get(id).copied())
            .collect(),
        nodes: map
            .map
            .nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| FairMapNode {
                slot,
                act: node.act,
                room_kind: room_kind_name(node.room_kind).to_owned(),
                children: node
                    .children
                    .iter()
                    .filter_map(|id| index.get(id).copied())
                    .collect(),
            })
            .collect(),
    })
}

fn event_screen(run: &RunState) -> Result<FairEventObservation, FairObservationError> {
    let event = run
        .event
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    let match_and_keep = run
        .match_and_keep
        .as_ref()
        .map(|state| {
            state
                .cards
                .iter()
                .map(|card| {
                    let content_key = if card.revealed || card.matched {
                        Some(
                            get_card_definition(card.content_id)
                                .ok_or(FairObservationError::UnknownPublicContent)?
                                .key
                                .to_owned(),
                        )
                    } else {
                        None
                    };
                    Ok(FairMatchAndKeepCard {
                        content_key,
                        revealed: card.revealed,
                        matched: card.matched,
                    })
                })
                .collect::<Result<Vec<_>, FairObservationError>>()
        })
        .transpose()?;
    Ok(FairEventObservation {
        event: stable_serialized_key(event.event)?,
        choices: event
            .choices
            .iter()
            .enumerate()
            .map(|(slot, choice)| FairEventChoice {
                slot,
                label: choice.label.clone(),
            })
            .collect(),
        match_and_keep,
    })
}

fn reward_screen(run: &RunState) -> Result<FairRewardObservation, FairObservationError> {
    let reward = run
        .reward
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    Ok(FairRewardObservation {
        cards: reward
            .choices
            .iter()
            .enumerate()
            .map(|(slot, card)| project_card_slot(slot, card))
            .collect::<Result<_, _>>()?,
        queued_card_rewards: reward
            .queued_card_rewards
            .iter()
            .enumerate()
            .map(|(slot, cards)| FairQueuedCardReward {
                slot,
                choice_count: cards.len(),
            })
            .collect(),
        gold_offer: reward.gold_offer,
        stolen_gold_offer: reward.stolen_gold_offer,
        potion_offer: reward.potion_offer.map(potion_key).map(str::to_owned),
        potion_offers: reward
            .potion_offers
            .iter()
            .copied()
            .map(potion_key)
            .map(str::to_owned)
            .collect(),
        relic_offer: reward
            .relic_offer
            .map(|relic| relic.trace_name().to_owned()),
        boss_relic_choices: reward
            .boss_relic_choices
            .iter()
            .map(|relic| relic.trace_name().to_owned())
            .collect(),
        card_reward_flow: match reward.card_reward_flow {
            CardRewardFlow::None => "none",
            CardRewardFlow::Pending { .. } => "pending",
            CardRewardFlow::Active { .. } => "active",
        }
        .to_owned(),
    })
}

fn treasure_screen(run: &RunState) -> Result<FairTreasureObservation, FairObservationError> {
    let room = run
        .treasure_room
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    Ok(FairTreasureObservation {
        chest_size: match room.chest_size {
            ChestSize::Small => "small",
            ChestSize::Medium => "medium",
            ChestSize::Large => "large",
        }
        .to_owned(),
        opened: run.boss_chest_opened,
    })
}

fn rest_screen(run: &RunState) -> Result<FairRestObservation, FairObservationError> {
    let options = sts_core::adapter_internals::run::legal_rest_actions(run)
        .map_err(|_| FairObservationError::InvalidAuthoritativeState)?
        .into_iter()
        .map(|action| match action {
            RestAction::Heal => Ok(FairRestOption::Heal),
            RestAction::OpenSmith => Ok(FairRestOption::OpenSmith),
            RestAction::OpenRemove => Ok(FairRestOption::OpenRemove),
            RestAction::Smith { card_id } => run
                .deck
                .iter()
                .position(|card| card.id == card_id)
                .map(|card_slot| FairRestOption::Smith { card_slot })
                .ok_or(FairObservationError::InvalidAuthoritativeState),
            RestAction::RemoveCard { card_id } => run
                .deck
                .iter()
                .position(|card| card.id == card_id)
                .map(|card_slot| FairRestOption::RemoveCard { card_slot })
                .ok_or(FairObservationError::InvalidAuthoritativeState),
            RestAction::Lift => Ok(FairRestOption::Lift),
            RestAction::Dig => Ok(FairRestOption::Dig),
            RestAction::Recall => Ok(FairRestOption::Recall),
            RestAction::Proceed => Ok(FairRestOption::Proceed),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FairRestObservation {
        complete: run.rest_room_complete,
        options,
    })
}

fn shop_screen(run: &RunState) -> Result<FairShopObservation, FairObservationError> {
    let shop = run.shop_merchant_open.then_some(
        run.shop
            .as_ref()
            .ok_or(FairObservationError::InvalidAuthoritativeState)?,
    );
    Ok(FairShopObservation {
        merchant_open: run.shop_merchant_open,
        remove_cost: shop.map(|shop| shop.remove_cost),
        cards: match shop {
            Some(shop) => shop
                .cards
                .iter()
                .enumerate()
                .map(|(slot, offer)| {
                    Ok(FairShopCard {
                        slot,
                        content_key: project_card(&offer.card, false)?.content_key,
                        price: offer.price,
                        sold: offer.sold,
                    })
                })
                .collect::<Result<_, FairObservationError>>()?,
            None => Vec::new(),
        },
        relics: shop
            .map(|shop| {
                shop.relics
                    .iter()
                    .enumerate()
                    .map(|(slot, offer)| FairShopRelic {
                        slot,
                        content_key: offer.relic_key.trace_name().to_owned(),
                        price: offer.price,
                        sold: offer.sold,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        potions: shop
            .map(|shop| {
                shop.potions
                    .iter()
                    .enumerate()
                    .map(|(slot, offer)| FairShopPotion {
                        slot,
                        content_key: potion_key(offer.potion).to_owned(),
                        price: offer.price,
                        sold: offer.sold,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn grid_screen(run: &RunState) -> Result<FairGridObservation, FairObservationError> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    Ok(FairGridObservation {
        purpose: grid_purpose_key(grid.purpose).to_owned(),
        cards: grid
            .cards
            .iter()
            .enumerate()
            .map(|(slot, card)| project_card_slot(slot, card))
            .collect::<Result<_, _>>()?,
        selected: grid.selected,
        selected_indices: grid.selected_indices.clone(),
    })
}

fn project_card_slot(
    slot: usize,
    card: &CardInstance,
) -> Result<FairCardSlot, FairObservationError> {
    Ok(FairCardSlot {
        slot,
        card: project_card(card, false)?,
    })
}
fn stable_serialized_key<T: Serialize>(value: T) -> Result<String, FairObservationError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(FairObservationError::InvalidAuthoritativeState)
}

fn grid_purpose_key(purpose: GridPurpose) -> &'static str {
    match purpose {
        GridPurpose::RestSmith => "rest_smith",
        GridPurpose::RestRemove => "rest_remove",
        GridPurpose::ShopRemove => "shop_remove",
        GridPurpose::EventRemove | GridPurpose::EventRemoveReturnToEvent { .. } => "event_remove",
        GridPurpose::EventObtainCard | GridPurpose::EventObtainCardReturnToEvent { .. } => {
            "event_obtain_card"
        }
        GridPurpose::EventUpgrade | GridPurpose::EventUpgradeReturnToEvent { .. } => {
            "event_upgrade"
        }
        GridPurpose::EmptyCage { .. } => "empty_cage",
        GridPurpose::NeowRemove { .. } => "neow_remove",
        GridPurpose::NeowUpgrade => "neow_upgrade",
        GridPurpose::Bottle { .. } => "bottle",
        GridPurpose::DollysMirror => "dollys_mirror",
        GridPurpose::CallingBellCurse => "calling_bell_curse",
        GridPurpose::PandorasBox => "pandoras_box",
        GridPurpose::Astrolabe => "astrolabe",
        GridPurpose::NeowTransform { .. } => "neow_transform",
        GridPurpose::EventTransform { .. } | GridPurpose::EventTransformReturnToEvent { .. } => {
            "event_transform"
        }
        GridPurpose::BonfireElementals => "bonfire_elementals",
        GridPurpose::DesignerRemoveAndUpgrade => "designer_remove_and_upgrade",
    }
}

fn room_kind_name(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Combat => "combat",
        RoomKind::Elite => "elite",
        RoomKind::Event => "event",
        RoomKind::Rest => "rest",
        RoomKind::Shop => "shop",
        RoomKind::Treasure => "treasure",
        RoomKind::Boss => "boss",
        RoomKind::Victory => "victory",
    }
}
impl From<RunPhase> for FairRunPhase {
    fn from(value: RunPhase) -> Self {
        match value {
            RunPhase::Combat => Self::Combat,
            RunPhase::Reward => Self::Reward,
            RunPhase::Treasure => Self::Treasure,
            RunPhase::Rest => Self::Rest,
            RunPhase::Event => Self::Event,
            RunPhase::Shop => Self::Shop,
            RunPhase::Idle => Self::Idle,
            RunPhase::Victory | RunPhase::Complete => Self::Complete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fair_json_allowlist::{check_schema, FAIR_RUN_OBSERVATION_SCHEMA};

    #[test]
    fn fair_run_schema_rejects_missing_and_unknown_fields() {
        let observation = fair_run_observation(&RunState::map_fixture()).expect("map projects");
        let mut value = serde_json::to_value(observation).expect("serializes");
        check_schema(&value, &FAIR_RUN_OBSERVATION_SCHEMA, "run").expect("schema accepted");

        let object = value.as_object_mut().expect("run object");
        object.insert("seed".to_owned(), serde_json::json!(123));
        assert!(check_schema(&value, &FAIR_RUN_OBSERVATION_SCHEMA, "run").is_err());

        let mut missing = serde_json::to_value(
            fair_run_observation(&RunState::map_fixture()).expect("map projects"),
        )
        .expect("serializes");
        missing
            .as_object_mut()
            .expect("run object")
            .remove("context");
        assert!(check_schema(&missing, &FAIR_RUN_OBSERVATION_SCHEMA, "run").is_err());
    }

    #[test]
    fn rest_observation_never_serializes_card_ids() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(RoomKind::Rest);
        let first_id = run.deck[0].id;
        let observation = fair_run_observation(&run).expect("rest projects");
        let json = serde_json::to_string(&observation).expect("serializes");
        assert!(!json.contains("card_id"));
        assert!(!json.contains(&format!("card:{}", first_id.get())));
        assert!(json.contains("card_slot"));
    }
}
