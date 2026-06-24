use crate::{
    map::{
        apply_map_action, legal_map_actions, reachable_nodes, validate_map_action,
        wing_boots_reachable_nodes, MapAction, RoomKind,
    },
    rng::StsRng,
    Relic, RunPhase, RunState, SimError, SimResult,
};

use super::event::enter_event_screen;
use super::reward::setup_treasure_room;
use super::shop::enter_shop_room;
use super::state::{
    DEFAULT_EVENT_ROOM_MONSTER_CHANCE, DEFAULT_EVENT_ROOM_SHOP_CHANCE,
    DEFAULT_EVENT_ROOM_TREASURE_CHANCE,
};

fn current_room_kind(run: &RunState) -> Option<RoomKind> {
    run.map.as_ref().and_then(|map_state| {
        map_state
            .map
            .node(map_state.current_node)
            .map(|node| node.room_kind)
    })
}

pub fn legal_map_actions_on_run(run: &RunState) -> Vec<MapAction> {
    if run.phase != RunPhase::Idle {
        return Vec::new();
    }

    let Some(map_state) = run.map.as_ref() else {
        return Vec::new();
    };

    let mut actions = legal_map_actions(map_state);
    if run.relics.contains(&Relic::WingBoots) && run.wing_boots_charges > 0 {
        for node_id in wing_boots_reachable_nodes(map_state) {
            let action = MapAction::ChooseNode { node_id };
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
    }
    actions
}

pub fn validate_map_action_on_run(run: &RunState, action: MapAction) -> SimResult<()> {
    if run.phase != RunPhase::Idle {
        return Err(SimError::IllegalAction("map actions require idle phase"));
    }

    let map_state = run
        .map
        .as_ref()
        .ok_or(SimError::InvalidState("map state is missing"))?;

    if validate_map_action(map_state, action).is_ok() {
        return Ok(());
    }

    if run.relics.contains(&Relic::WingBoots)
        && run.wing_boots_charges > 0
        && wing_boots_action_is_legal(map_state, action)
    {
        Ok(())
    } else {
        Err(SimError::IllegalAction("map node is not reachable"))
    }
}

pub fn apply_map_action_on_run(run: &RunState, action: MapAction) -> SimResult<RunState> {
    validate_map_action_on_run(run, action)?;

    let map_state = run.map.as_ref().expect("validated map state");
    let last_room_was_shop = run.current_room_kind() == Some(RoomKind::Shop);
    let uses_wing_boots = run.relics.contains(&Relic::WingBoots)
        && run.wing_boots_charges > 0
        && !reachable_nodes(map_state).contains(&chosen_node_id(action));
    let next_map = if uses_wing_boots {
        apply_wing_boots_map_action(map_state, action)?
    } else {
        apply_map_action(map_state, action)?
    };

    let mut next = run.clone();
    next.map = Some(next_map);
    next.current_room_override = None;
    if uses_wing_boots {
        next.wing_boots_charges = next.wing_boots_charges.saturating_sub(1);
    }
    next.apply_floor_entry_relics();

    if current_room_kind(&next) == Some(RoomKind::Rest) {
        next.apply_rest_site_entry_relics();
        next.phase = RunPhase::Rest;
    } else if current_room_kind(&next) == Some(RoomKind::Shop) {
        enter_shop_room(&mut next);
    } else if current_room_kind(&next) == Some(RoomKind::Treasure) {
        setup_treasure_room(&mut next);
    } else if current_room_kind(&next) == Some(RoomKind::Event) {
        apply_event_room_outcome(&mut next, last_room_was_shop);
    }

    Ok(next)
}

fn wing_boots_action_is_legal(map_state: &crate::MapRunState, action: MapAction) -> bool {
    wing_boots_reachable_nodes(map_state).contains(&chosen_node_id(action))
}

fn chosen_node_id(action: MapAction) -> crate::MapNodeId {
    match action {
        MapAction::ChooseNode { node_id } => node_id,
    }
}

fn apply_wing_boots_map_action(
    map_state: &crate::MapRunState,
    action: MapAction,
) -> SimResult<crate::MapRunState> {
    let node_id = chosen_node_id(action);
    let target = map_state
        .map
        .node(node_id)
        .ok_or(SimError::UnknownMapNode(node_id))?;
    Ok(crate::MapRunState {
        act: target.act,
        floor: map_state.floor + 1,
        current_node: node_id,
        map: map_state.map.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventRoomOutcome {
    Monster,
    Shop,
    Treasure,
    Event,
}

fn apply_event_room_outcome(run: &mut RunState, last_room_was_shop: bool) {
    let mut rng = StsRng::with_counter(run.event_rng_seed as i64, run.event_rng_counter);
    let roll_index = (rng.random_float() * 100.0) as u32;
    run.event_rng_counter = rng.counter();

    let raw_outcome = if apply_tiny_chest(run) {
        EventRoomOutcome::Treasure
    } else {
        target_event_room_outcome(
            roll_index,
            run.event_room_monster_chance,
            run.event_room_shop_chance,
            run.event_room_treasure_chance,
            last_room_was_shop,
        )
    };

    let mut outcome = raw_outcome;
    if outcome == EventRoomOutcome::Monster && run.relics.contains(&Relic::JuzuBracelet) {
        outcome = EventRoomOutcome::Event;
    }
    update_event_room_chances(run, raw_outcome, outcome);

    match outcome {
        EventRoomOutcome::Monster => {
            run.current_room_override = Some(RoomKind::Combat);
        }
        EventRoomOutcome::Shop => {
            run.current_room_override = Some(RoomKind::Shop);
            enter_shop_room(run);
        }
        EventRoomOutcome::Treasure => {
            run.current_room_override = Some(RoomKind::Treasure);
            setup_treasure_room(run);
        }
        EventRoomOutcome::Event => {
            run.current_room_override = Some(RoomKind::Event);
            enter_event_screen(run);
        }
    }
}

fn target_event_room_outcome(
    roll_index: u32,
    monster_chance: u32,
    shop_chance: u32,
    treasure_chance: u32,
    last_room_was_shop: bool,
) -> EventRoomOutcome {
    let monster_size = monster_chance;
    let shop_size = monster_size + if last_room_was_shop { 0 } else { shop_chance };
    let treasure_size = shop_size + treasure_chance;

    if roll_index < monster_size {
        EventRoomOutcome::Monster
    } else if roll_index < shop_size {
        EventRoomOutcome::Shop
    } else if roll_index < treasure_size {
        EventRoomOutcome::Treasure
    } else {
        EventRoomOutcome::Event
    }
}

fn update_event_room_chances(
    run: &mut RunState,
    raw_outcome: EventRoomOutcome,
    resolved_outcome: EventRoomOutcome,
) {
    if raw_outcome == EventRoomOutcome::Monster {
        run.event_room_monster_chance = DEFAULT_EVENT_ROOM_MONSTER_CHANCE;
    } else {
        run.event_room_monster_chance += DEFAULT_EVENT_ROOM_MONSTER_CHANCE;
    }

    if resolved_outcome == EventRoomOutcome::Shop {
        run.event_room_shop_chance = DEFAULT_EVENT_ROOM_SHOP_CHANCE;
    } else {
        run.event_room_shop_chance += DEFAULT_EVENT_ROOM_SHOP_CHANCE;
    }

    if resolved_outcome == EventRoomOutcome::Treasure {
        run.event_room_treasure_chance = DEFAULT_EVENT_ROOM_TREASURE_CHANCE;
    } else {
        run.event_room_treasure_chance += DEFAULT_EVENT_ROOM_TREASURE_CHANCE;
    }
}

fn apply_tiny_chest(run: &mut RunState) -> bool {
    if !run.relics.contains(&Relic::TinyChest) {
        return false;
    }

    run.tiny_chest_counter += 1;
    if run.tiny_chest_counter >= 4 {
        run.tiny_chest_counter = 0;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::shop::open_shop_merchant;
    use crate::{
        ids::MapNodeId,
        map::{FixedMap, MapNode, RoomKind},
    };

    fn event_chain_run() -> RunState {
        let mut run = RunState::map_fixture();
        run.event_room_monster_chance = 0;
        run.event_room_shop_chance = 0;
        run.event_room_treasure_chance = 0;
        run.map = Some(crate::map::MapRunState {
            act: 1,
            floor: 0,
            current_node: MapNodeId::new(0),
            map: FixedMap {
                nodes: vec![
                    MapNode {
                        id: MapNodeId::new(0),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(1)],
                    },
                    MapNode {
                        id: MapNodeId::new(1),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(2)],
                    },
                    MapNode {
                        id: MapNodeId::new(2),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(3)],
                    },
                    MapNode {
                        id: MapNodeId::new(3),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: vec![MapNodeId::new(4)],
                    },
                    MapNode {
                        id: MapNodeId::new(4),
                        act: 1,
                        room_kind: RoomKind::Event,
                        children: Vec::new(),
                    },
                ],
            },
        });
        run
    }

    fn wing_boots_branch_run() -> RunState {
        let mut run = RunState::map_fixture();
        run.map = Some(crate::map::MapRunState {
            act: 1,
            floor: 1,
            current_node: MapNodeId::new(1),
            map: FixedMap {
                nodes: vec![
                    MapNode {
                        id: MapNodeId::new(0),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(1), MapNodeId::new(2)],
                    },
                    MapNode {
                        id: MapNodeId::new(1),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(3)],
                    },
                    MapNode {
                        id: MapNodeId::new(2),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: vec![MapNodeId::new(4)],
                    },
                    MapNode {
                        id: MapNodeId::new(3),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: Vec::new(),
                    },
                    MapNode {
                        id: MapNodeId::new(4),
                        act: 1,
                        room_kind: RoomKind::Combat,
                        children: Vec::new(),
                    },
                ],
            },
        });
        run
    }

    #[test]
    fn map_actions_require_idle_phase() {
        let run = RunState::combat_fixture();

        let err = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect_err("combat blocks map");

        assert_eq!(
            err,
            SimError::IllegalAction("map actions require idle phase")
        );
    }

    #[test]
    fn entering_rest_node_transitions_to_rest_phase() {
        let run = RunState::map_fixture();

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("choose rest node");

        assert_eq!(next.phase, RunPhase::Rest);
        assert_eq!(
            next.map.as_ref().expect("map").current_node,
            MapNodeId::new(2)
        );
        assert_eq!(
            next.map
                .as_ref()
                .and_then(|map| map.map.node(map.current_node))
                .map(|node| node.room_kind),
            Some(RoomKind::Rest)
        );
    }

    #[test]
    fn entering_rest_node_arms_ancient_tea_set() {
        let mut run = RunState::map_fixture();
        run.relics.push(crate::Relic::AncientTeaSet);

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("choose rest node");

        assert!(next.ancient_tea_set_armed);
    }

    #[test]
    fn entering_combat_node_stays_idle() {
        let run = RunState::map_fixture();

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("choose combat node");

        assert_eq!(next.phase, RunPhase::Idle);
    }

    #[test]
    fn wing_boots_allows_same_floor_branch_jump_and_consumes_charge() {
        let mut run = wing_boots_branch_run();
        run.gain_relic(Relic::WingBoots);

        let jump = MapAction::ChooseNode {
            node_id: MapNodeId::new(4),
        };
        assert!(legal_map_actions_on_run(&run).contains(&jump));

        let next = apply_map_action_on_run(&run, jump).expect("wing boots jump applies");

        assert_eq!(next.wing_boots_charges, 2);
        assert_eq!(
            next.map.as_ref().expect("map").current_node,
            MapNodeId::new(4)
        );
    }

    #[test]
    fn wing_boots_does_not_consume_charge_for_normal_child() {
        let mut run = wing_boots_branch_run();
        run.gain_relic(Relic::WingBoots);

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(3),
            },
        )
        .expect("normal child applies");

        assert_eq!(next.wing_boots_charges, 3);
    }

    #[test]
    fn maw_bank_grants_gold_when_entering_map_nodes_until_broken() {
        let mut run = RunState::map_fixture();
        run.relics.push(crate::Relic::MawBank);
        let gold_before = run.gold;

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("choose combat node");

        assert_eq!(next.gold, gold_before + crate::relic::MAW_BANK_GOLD);

        let mut broken = run;
        broken.maw_bank_broken = true;
        let after_broken = apply_map_action_on_run(
            &broken,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("choose combat node");

        assert_eq!(after_broken.gold, gold_before);
    }

    #[test]
    fn entering_shop_node_transitions_to_shop_phase() {
        let mut run = RunState::map_fixture();
        for node_id in [MapNodeId::new(1), MapNodeId::new(3), MapNodeId::new(4)] {
            run = apply_map_action_on_run(&run, MapAction::ChooseNode { node_id })
                .expect("reach shop");
        }

        assert_eq!(run.phase, RunPhase::Shop);
        assert_eq!(
            run.map
                .as_ref()
                .and_then(|map| map.map.node(map.current_node))
                .map(|node| node.room_kind),
            Some(RoomKind::Shop)
        );
        assert!(run.shop.is_none());
        open_shop_merchant(&mut run);
        assert!(run.shop.is_some());
    }

    #[test]
    fn tiny_chest_turns_fourth_event_room_into_treasure() {
        let mut run = event_chain_run();
        run.relics.push(Relic::TinyChest);

        for node_id in [MapNodeId::new(1), MapNodeId::new(2), MapNodeId::new(3)] {
            run = apply_map_action_on_run(&run, MapAction::ChooseNode { node_id })
                .expect("enter event");
            assert_eq!(run.phase, RunPhase::Event);
            assert!(run.event.is_some());
            run.phase = RunPhase::Idle;
            run.event = None;
        }

        assert_eq!(run.tiny_chest_counter, 3);
        let run = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(4),
            },
        )
        .expect("enter fourth event");

        assert_eq!(run.phase, RunPhase::Idle);
        assert!(run.event.is_none());
        assert!(run.treasure_room.is_some());
        assert_eq!(run.tiny_chest_counter, 0);
        assert_eq!(run.current_room_kind(), Some(RoomKind::Treasure));
        assert_eq!(run.event_rng_counter, 10);
    }

    #[test]
    fn event_room_without_tiny_chest_still_opens_event() {
        let run = event_chain_run();

        let run = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("enter event");

        assert_eq!(run.phase, RunPhase::Event);
        assert!(run.event.is_some());
        assert!(run.treasure_room.is_none());
        assert_eq!(run.tiny_chest_counter, 0);
        assert_eq!(run.current_room_kind(), Some(RoomKind::Event));
        assert_eq!(run.event_rng_counter, 3);
    }

    #[test]
    fn event_room_outcome_uses_escalating_chances() {
        assert_eq!(
            target_event_room_outcome(9, 10, 3, 2, false),
            EventRoomOutcome::Monster
        );
        assert_eq!(
            target_event_room_outcome(11, 10, 3, 2, false),
            EventRoomOutcome::Shop
        );
        assert_eq!(
            target_event_room_outcome(11, 10, 3, 2, true),
            EventRoomOutcome::Treasure
        );
        assert_eq!(
            target_event_room_outcome(14, 10, 3, 2, false),
            EventRoomOutcome::Treasure
        );
        assert_eq!(
            target_event_room_outcome(15, 10, 3, 2, false),
            EventRoomOutcome::Event
        );
    }

    #[test]
    fn juzu_bracelet_converts_event_room_monster_outcome_to_event() {
        let mut run = event_chain_run();
        run.relics.push(Relic::JuzuBracelet);
        run.event_room_monster_chance = 100;

        let run = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("enter event");

        assert_eq!(run.phase, RunPhase::Event);
        assert!(run.event.is_some());
        assert_eq!(run.current_room_kind(), Some(RoomKind::Event));
        assert_eq!(
            run.event_room_monster_chance,
            DEFAULT_EVENT_ROOM_MONSTER_CHANCE
        );
        assert_eq!(run.event_room_shop_chance, DEFAULT_EVENT_ROOM_SHOP_CHANCE);
        assert_eq!(
            run.event_room_treasure_chance,
            DEFAULT_EVENT_ROOM_TREASURE_CHANCE
        );
    }

    #[test]
    fn event_room_monster_outcome_records_resolved_combat_room() {
        let mut run = event_chain_run();
        run.event_room_monster_chance = 100;

        let run = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("enter event");

        assert_eq!(run.phase, RunPhase::Idle);
        assert!(run.event.is_none());
        assert_eq!(run.current_room_kind(), Some(RoomKind::Combat));
        assert_eq!(
            run.event_room_monster_chance,
            DEFAULT_EVENT_ROOM_MONSTER_CHANCE
        );
    }
}
