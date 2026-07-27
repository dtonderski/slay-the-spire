//! Visibility-safe observations for non-combat run decisions.
//!
//! Combat has a detailed, stable observation contract in
//! `combat::fair_observation`. This module supplies the run-level envelope and
//! the public screens around it. It deliberately serializes only public
//! content; authoritative IDs, RNG state, and pre-rolled future outcomes never
//! cross this boundary.

use crate::{
    combat::{fair_combat_observation, FairObservationError},
    content::cards::get_card_definition,
    map::RoomKind,
    run::state::RunState,
    CardInstance, FairCard, Potion, RunPhase,
};
use serde_json::{json, Value};

pub const FAIR_RUN_OBSERVATION_SCHEMA_VERSION: u32 = 1;

/// Returns one tagged observation for every supported run decision screen.
///
/// The payload is intentionally JSON-shaped at this boundary so the Python
/// facade can add screen-specific typed wrappers without duplicating Rust
/// legality or visibility logic.
pub fn fair_run_observation(run: &RunState) -> Result<Value, FairObservationError> {
    let context = public_context(run)?;
    let (kind, screen) = if run.card_grid.is_some() {
        ("grid", grid_screen(run)?)
    } else {
        match run.phase {
            RunPhase::Combat => (
                "combat",
                serde_json::to_value(fair_combat_observation(run)?)
                    .map_err(|_| FairObservationError::InvalidAuthoritativeState)?,
            ),
            RunPhase::Reward => ("reward", reward_screen(run)?),
            RunPhase::Treasure => ("treasure", treasure_screen(run)?),
            RunPhase::Rest => ("rest", rest_screen(run)?),
            RunPhase::Event => ("event", event_screen(run)?),
            RunPhase::Shop => ("shop", shop_screen(run)?),
            RunPhase::Idle => match run.map.as_ref() {
                Some(_) => ("map", map_screen(run)?),
                None => ("idle", json!({})),
            },
            RunPhase::Complete => ("complete", json!({})),
        }
    };

    Ok(json!({
        "schema_version": FAIR_RUN_OBSERVATION_SCHEMA_VERSION,
        "phase": run_phase_name(run.phase),
        "kind": kind,
        "context": context,
        "screen": screen,
    }))
}

fn public_context(run: &RunState) -> Result<Value, FairObservationError> {
    let deck = run
        .deck
        .iter()
        .map(|card| project_card(card, false))
        .collect::<Result<Vec<_>, _>>()?;
    let relics = run
        .relics
        .iter()
        .enumerate()
        .map(|(slot, relic)| json!({ "slot": slot, "content_key": relic.trace_name() }))
        .collect::<Vec<_>>();
    let potions = (0..run.potion_capacity())
        .map(|slot| {
            json!({
                "slot": slot,
                "content_key": run.potion_at_slot(slot).map(potion_key),
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "ascension": run.ascension,
        "act": run.current_act,
        "floor": run.current_floor,
        "gold": run.gold,
        "player_hp": run.player_hp,
        "player_max_hp": run.player_max_hp,
        "deck": deck,
        "relics": relics,
        "potion_slots": potions,
    }))
}

fn map_screen(run: &RunState) -> Result<Value, FairObservationError> {
    let map = run
        .map
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    let node_index = map
        .map
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let current = *node_index
        .get(&map.current_node)
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    let reachable = map
        .map
        .children_of(map.current_node)
        .map_err(|_| FairObservationError::InvalidAuthoritativeState)?
        .iter()
        .filter_map(|id| node_index.get(id).copied())
        .collect::<Vec<_>>();
    let nodes = map
        .map
        .nodes
        .iter()
        .enumerate()
        .map(|(slot, node)| {
            json!({
                "slot": slot,
                "act": node.act,
                "room_kind": room_kind_name(node.room_kind),
                "children": node.children.iter().filter_map(|id| node_index.get(id).copied()).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(
        json!({ "act": map.act, "floor": map.floor, "current_node": current, "reachable_nodes": reachable, "nodes": nodes }),
    )
}

fn event_screen(run: &RunState) -> Result<Value, FairObservationError> {
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
                    if card.revealed || card.matched {
                        let definition = get_card_definition(card.content_id)
                            .ok_or(FairObservationError::UnknownPublicContent)?;
                        Ok(json!({
                            "content_key": definition.key,
                            "revealed": card.revealed,
                            "matched": card.matched,
                        }))
                    } else {
                        Ok(json!({ "content_key": Value::Null, "revealed": false, "matched": false }))
                    }
                })
                .collect::<Result<Vec<_>, FairObservationError>>()
        })
        .transpose()?;
    Ok(json!({
        "event": format!("{:?}", event.event),
        "choices": event.choices.iter().enumerate().map(|(slot, choice)| json!({ "slot": slot, "label": choice.label })).collect::<Vec<_>>(),
        "match_and_keep": match_and_keep,
    }))
}

fn reward_screen(run: &RunState) -> Result<Value, FairObservationError> {
    let reward = run
        .reward
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    let cards = reward
        .choices
        .iter()
        .enumerate()
        .map(|(slot, card)| project_card_slot(slot, card))
        .collect::<Result<Vec<_>, _>>()?;
    let queued = reward
        .queued_card_rewards
        .iter()
        .enumerate()
        .map(|(slot, cards)| json!({ "slot": slot, "choice_count": cards.len() }))
        .collect::<Vec<_>>();
    Ok(json!({
        "cards": cards,
        "queued_card_rewards": queued,
        "gold_offer": reward.gold_offer,
        "stolen_gold_offer": reward.stolen_gold_offer,
        "potion_offer": reward.potion_offer.map(potion_key),
        "potion_offers": reward.potion_offers.iter().map(|potion| potion_key(*potion)).collect::<Vec<_>>(),
        "relic_offer": reward.relic_offer.map(|relic| relic.trace_name()),
        "boss_relic_choices": reward.boss_relic_choices.iter().map(|relic| format!("{:?}", relic)).collect::<Vec<_>>(),
        "card_reward_flow": format!("{:?}", reward.card_reward_flow),
    }))
}

fn treasure_screen(run: &RunState) -> Result<Value, FairObservationError> {
    let treasure = run
        .treasure_room
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    Ok(json!({
        "chest_size": format!("{:?}", treasure.chest_size),
        "opened": run.boss_chest_opened,
    }))
}

fn rest_screen(run: &RunState) -> Result<Value, FairObservationError> {
    Ok(json!({
        "complete": run.rest_room_complete,
        "options": crate::run::legal_rest_actions(run).map_err(|_| FairObservationError::InvalidAuthoritativeState)?.into_iter().map(|action| format!("{:?}", action)).collect::<Vec<_>>(),
    }))
}

fn shop_screen(run: &RunState) -> Result<Value, FairObservationError> {
    let open = run.shop_merchant_open;
    let shop = if open {
        Some(
            run.shop
                .as_ref()
                .ok_or(FairObservationError::InvalidAuthoritativeState)?,
        )
    } else {
        None
    };
    let cards = match shop {
        Some(shop) => shop
            .cards
            .iter()
            .enumerate()
            .map(|(slot, offer)| {
                Ok(json!({
                    "slot": slot,
                    "content_key": project_card(&offer.card, false)?.content_key,
                    "price": offer.price,
                    "sold": offer.sold,
                }))
            })
            .collect::<Result<Vec<_>, FairObservationError>>()?,
        None => Vec::new(),
    };
    Ok(json!({
        "merchant_open": open,
        "remove_cost": shop.map(|shop| shop.remove_cost),
        "cards": cards,
        "relics": shop.map(|shop| shop.relics.iter().enumerate().map(|(slot, offer)| json!({ "slot": slot, "content_key": offer.relic_key.trace_name(), "price": offer.price, "sold": offer.sold })).collect::<Vec<_>>()).unwrap_or_default(),
        "potions": shop.map(|shop| shop.potions.iter().enumerate().map(|(slot, offer)| json!({ "slot": slot, "content_key": potion_key(offer.potion), "price": offer.price, "sold": offer.sold })).collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

fn grid_screen(run: &RunState) -> Result<Value, FairObservationError> {
    let grid = run
        .card_grid
        .as_ref()
        .ok_or(FairObservationError::InvalidAuthoritativeState)?;
    Ok(json!({
        "purpose": format!("{:?}", grid.purpose),
        "cards": grid.cards.iter().enumerate().map(|(slot, card)| project_card_slot(slot, card)).collect::<Result<Vec<_>, _>>()?,
        "selected": grid.selected,
        "selected_indices": grid.selected_indices,
    }))
}

fn project_card_slot(slot: usize, card: &CardInstance) -> Result<Value, FairObservationError> {
    Ok(json!({ "slot": slot, "card": project_card(card, false)? }))
}

fn project_card(
    card: &CardInstance,
    corruption_active: bool,
) -> Result<FairCard, FairObservationError> {
    crate::combat::fair_observation::project_card(card, corruption_active)
}

fn potion_key(potion: Potion) -> &'static str {
    crate::combat::fair_observation::potion_key(potion)
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

fn run_phase_name(phase: RunPhase) -> &'static str {
    match phase {
        RunPhase::Combat => "combat",
        RunPhase::Reward => "reward",
        RunPhase::Treasure => "treasure",
        RunPhase::Rest => "rest",
        RunPhase::Event => "event",
        RunPhase::Shop => "shop",
        RunPhase::Idle => "idle",
        RunPhase::Complete => "complete",
    }
}
