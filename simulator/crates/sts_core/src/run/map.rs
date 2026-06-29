use crate::{
    card::CardInstance,
    combat::{initialize_combat_piles_with_relics, CombatState, MonsterState},
    content::cards::WOUND_ID,
    content::monsters::{
        content_id_from_game_monster_id, get_monster_definition, living_monster_missing_hp,
        monster_state_for_ascension, prepare_monster_intent_for_ascension, record_target_move,
        target_beyond_encounter_spawn_for_key, target_centurion_next_intent_from_roll,
        target_chosen_next_intent_from_roll, target_city_normal_encounter_spawn_at_combat_index,
        target_elite_encounter_spawn_at_combat_index, target_fungi_beast_next_intent_from_roll,
        target_gremlin_leader_next_intent_from_roll, target_healer_next_intent_from_roll,
        target_jaw_worm_next_intent_from_roll, target_large_acid_slime_next_intent_from_roll,
        target_normal_encounter_spawn_at_combat_index,
        target_shelled_parasite_next_intent_from_roll, target_snake_plant_next_intent_from_roll,
        TargetEncounterSpawn, ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE, BRONZE_AUTOMATON_A0,
        CENTURION_ID, CHOSEN_ID, DARKLING_ID, FUNGI_BEAST_ID, GREMLIN_LEADER_ID, HEALER_ID,
        JAW_WORM_ID, SHELLED_PARASITE_ID, SNAKE_PLANT_ID,
    },
    ids::CardId,
    map::{
        apply_map_action, legal_map_actions, reachable_nodes, validate_map_action,
        wing_boots_reachable_nodes, MapAction, RoomKind, TargetMapAct,
    },
    relic::MARK_OF_PAIN_WOUNDS,
    rng::StsRng,
    MonsterPowers, Relic, RunPhase, RunState, SimError, SimResult,
};

use super::event::enter_event_screen;
use super::reward::setup_treasure_room;
use super::shop::enter_shop_room;
use super::state::{
    RunRngStream, DEFAULT_EVENT_ROOM_MONSTER_CHANCE, DEFAULT_EVENT_ROOM_SHOP_CHANCE,
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
    if let Some(map) = next.map.as_ref() {
        next.current_floor = i32::try_from(map.floor).unwrap_or(i32::MAX);
        next.current_act = i32::from(map.act);
    }
    next.reinit_room_rngs_for_floor();
    next.current_room_override = None;
    if uses_wing_boots {
        next.wing_boots_charges = next.wing_boots_charges.saturating_sub(1);
    }
    next.apply_floor_entry_relics();

    if current_room_kind(&next) == Some(RoomKind::Rest) {
        next.apply_rest_site_entry_relics();
        next.phase = RunPhase::Rest;
        next.rest_room_complete = false;
    } else if current_room_kind(&next) == Some(RoomKind::Combat) {
        enter_normal_combat(&mut next);
    } else if current_room_kind(&next) == Some(RoomKind::Elite) {
        enter_elite_combat(&mut next);
    } else if current_room_kind(&next) == Some(RoomKind::Boss) {
        enter_boss_combat(&mut next);
    } else if current_room_kind(&next) == Some(RoomKind::Shop) {
        enter_shop_room(&mut next);
    } else if current_room_kind(&next) == Some(RoomKind::Treasure) {
        setup_treasure_room(&mut next);
        next.phase = RunPhase::Treasure;
    } else if current_room_kind(&next) == Some(RoomKind::Event) {
        apply_event_room_outcome(&mut next, last_room_was_shop);
    }

    Ok(next)
}

fn enter_normal_combat(run: &mut RunState) {
    let mut base = normal_combat_state_for_run(run);
    enter_combat_with_base(run, &mut base);
    run.normal_combat_count = run.normal_combat_count.saturating_add(1);
}

fn enter_elite_combat(run: &mut RunState) {
    let mut base = elite_combat_state_for_run(run);
    enter_combat_with_base(run, &mut base);
    run.elite_combat_count = run.elite_combat_count.saturating_add(1);
}

fn enter_boss_combat(run: &mut RunState) {
    let mut base = boss_combat_state_for_run(run);
    enter_combat_with_base(run, &mut base);
}

fn enter_combat_with_base(run: &mut RunState, base: &mut CombatState) {
    let mut shuffle_rng = StsRng::new(run.event_rng_seed as i64 + i64::from(run.current_floor));
    let monster_hp_rng = StsRng::with_counter(
        run.event_rng_seed as i64 + i64::from(run.current_floor),
        base.monsters.len() as u32,
    );
    let mut card_random_rng = run
        .relics
        .contains(&Relic::SneckoEye)
        .then(|| run.card_random_rng());
    let mut monster_rng = StsRng::new(run.monster_rng_seed as i64 + i64::from(run.current_floor));
    base.piles = initialize_combat_piles_with_relics(
        &run.deck,
        &mut shuffle_rng,
        &mut card_random_rng,
        &run.relics,
    );
    base.shuffle_rng = Some(shuffle_rng);
    base.monster_hp_rng = Some(monster_hp_rng);
    apply_initial_monster_ai_rolls(base, &mut monster_rng);
    record_initial_monster_moves(base);
    base.monster_rng = Some(monster_rng);
    base.card_random_rng = card_random_rng;
    run.phase = RunPhase::Combat;
    let mut combat = run.init_combat_consuming_relics(base.clone());
    add_mark_of_pain_wounds_to_draw_pile(run, &mut combat);
    run.combat = Some(combat);
}

fn record_initial_monster_moves(combat: &mut CombatState) {
    for monster in &mut combat.monsters {
        if monster.alive {
            record_target_move(monster);
        }
    }
}

fn add_mark_of_pain_wounds_to_draw_pile(run: &mut RunState, combat: &mut CombatState) {
    if !run.relics.contains(&Relic::MarkOfPain) {
        return;
    }
    let mut rng = run.card_random_rng();
    for _ in 0..MARK_OF_PAIN_WOUNDS {
        let next_id = CardId::new(combat.piles.max_card_instance_id() + 1);
        let wound = CardInstance::new(next_id, WOUND_ID);
        if combat.piles.draw_pile.is_empty() {
            combat.piles.draw_pile.push(wound);
        } else {
            let index = rng.random_int((combat.piles.draw_pile.len() - 1) as i32) as usize;
            combat.piles.draw_pile.insert(index, wound);
        }
    }
    combat.card_random_rng = Some(rng.clone());
    run.store_rng_counter(RunRngStream::CardRandom, &rng);
}

fn apply_initial_monster_ai_rolls(combat: &mut CombatState, rng: &mut StsRng) {
    let living_monster_count = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .count();
    let alive_gremlin_count = gremlin_leader_alive_minion_count(&combat.monsters);
    let missing_hp = living_monster_missing_hp(&combat.monsters, combat.ascension);
    for (index, monster) in combat.monsters.iter_mut().enumerate() {
        if !monster.alive {
            continue;
        }
        let roll = rng.random_int(99);
        if monster.content_id == ACID_SLIME_ID && monster.hp > ACID_SLIME_M_A7_HP_RANGE.max {
            monster.intent = target_large_acid_slime_next_intent_from_roll(
                crate::MonsterIntent::Stun,
                roll,
                rng,
                combat.ascension,
            );
        } else if monster.content_id == JAW_WORM_ID {
            monster.intent = target_jaw_worm_next_intent_from_roll(&monster.move_history, roll);
        } else if monster.content_id == CHOSEN_ID {
            monster.intent =
                target_chosen_next_intent_from_roll(&monster.move_history, roll, combat.ascension);
        } else if monster.content_id == CENTURION_ID {
            monster.intent = target_centurion_next_intent_from_roll(
                &monster.move_history,
                roll,
                living_monster_count,
                combat.ascension,
            );
        } else if monster.content_id == HEALER_ID {
            monster.intent = target_healer_next_intent_from_roll(
                &monster.move_history,
                roll,
                missing_hp,
                combat.ascension,
            );
        } else if monster.content_id == FUNGI_BEAST_ID {
            monster.intent = target_fungi_beast_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == GREMLIN_LEADER_ID {
            monster.intent = target_gremlin_leader_next_intent_from_roll(
                &monster.move_history,
                roll,
                alive_gremlin_count,
                combat.ascension,
            );
        } else if monster.content_id == SNAKE_PLANT_ID {
            monster.intent = target_snake_plant_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == DARKLING_ID {
            monster.intent = crate::content::monsters::target_darkling_next_intent_from_roll(
                &monster.move_history,
                roll,
                index,
                monster.rolled_attack_damage,
                combat.ascension,
            );
        } else if monster.content_id == SHELLED_PARASITE_ID
            && monster.moves_executed == 0
            && combat.ascension < 17
        {
            monster.intent = if rng.random_bool() {
                crate::MonsterIntent::AttackMultiple {
                    damage: if combat.ascension >= 2 { 7 } else { 6 },
                    hits: 2,
                }
            } else {
                crate::MonsterIntent::AttackHealSelf {
                    damage: if combat.ascension >= 2 { 12 } else { 10 },
                }
            };
        } else if monster.content_id == SHELLED_PARASITE_ID {
            monster.intent = target_shelled_parasite_next_intent_from_roll(
                &monster.move_history,
                roll,
                rng,
                combat.ascension,
            );
        } else {
            monster.intent = prepare_monster_intent_for_ascension(monster, combat.ascension);
        }
    }
}

fn gremlin_leader_alive_minion_count(monsters: &[MonsterState]) -> usize {
    monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && crate::content::monsters::is_gremlin_leader_minion_content_id(monster.content_id)
        })
        .count()
}

fn normal_combat_state_for_run(run: &mut RunState) -> CombatState {
    let combat_index = normal_combat_index_for_run(run);
    let floor = u32::try_from(run.current_floor.max(1)).unwrap_or(1);
    let neow_lament = run.neow_lament_combats_remaining > 0;
    let spawns = if run.current_act == 3 {
        run.normal_encounter_list
            .get(combat_index)
            .cloned()
            .or_else(|| {
                crate::content::encounters::target_normal_encounter_key_at_combat_index(
                    run.event_rng_seed as i64,
                    TargetMapAct::Beyond,
                    combat_index,
                )
            })
            .and_then(|encounter_key| {
                target_beyond_encounter_spawn_for_key(
                    run.event_rng_seed as i64,
                    floor,
                    &encounter_key,
                    run.ascension,
                    neow_lament,
                )
            })
    } else if run.current_act == 2 {
        if let Some(encounter_key) = run.normal_encounter_list.get(combat_index).cloned() {
            target_city_encounter_spawn_for_run(run, floor, &encounter_key, neow_lament)
        } else {
            target_city_normal_encounter_spawn_at_combat_index(
                run.event_rng_seed as i64,
                floor,
                combat_index,
                run.ascension,
                neow_lament,
            )
        }
    } else {
        target_normal_encounter_spawn_at_combat_index(
            run.event_rng_seed as i64,
            floor,
            combat_index,
            run.ascension,
            neow_lament,
        )
    }
    .unwrap_or_default();

    let mut combat = CombatState::initial_fixture();
    if !spawns.is_empty() {
        combat.monsters = spawns
            .iter()
            .enumerate()
            .map(|(index, spawn)| target_spawn_monster_state(spawn, index, run.ascension))
            .collect();
        assign_initial_gremlin_leader_slots(&mut combat.monsters);
    }
    combat
}

fn elite_combat_state_for_run(run: &mut RunState) -> CombatState {
    let combat_index = run.elite_combat_count as usize;
    let floor = u32::try_from(run.current_floor.max(1)).unwrap_or(1);
    let neow_lament = run.neow_lament_combats_remaining > 0;
    let act = if run.current_act == 2 {
        TargetMapAct::City
    } else {
        TargetMapAct::Exordium
    };
    let spawns = if run.current_act == 2 {
        if let Some(encounter_key) = run.elite_encounter_list.get(combat_index).cloned() {
            target_city_encounter_spawn_for_run(run, floor, &encounter_key, neow_lament)
        } else {
            target_elite_encounter_spawn_at_combat_index(
                run.event_rng_seed as i64,
                act,
                floor,
                combat_index,
                run.ascension,
                neow_lament,
            )
        }
    } else {
        target_elite_encounter_spawn_at_combat_index(
            run.event_rng_seed as i64,
            act,
            floor,
            combat_index,
            run.ascension,
            neow_lament,
        )
    }
    .unwrap_or_default();

    let mut combat = CombatState::initial_fixture();
    if !spawns.is_empty() {
        combat.monsters = spawns
            .iter()
            .enumerate()
            .map(|(index, spawn)| target_spawn_monster_state(spawn, index, run.ascension))
            .collect();
        assign_initial_gremlin_leader_slots(&mut combat.monsters);
    }
    combat
}

fn assign_initial_gremlin_leader_slots(monsters: &mut [MonsterState]) {
    if !monsters
        .iter()
        .any(|monster| monster.content_id == GREMLIN_LEADER_ID)
    {
        return;
    }

    let mut next_slot = 0_u8;
    for monster in monsters.iter_mut() {
        if monster.content_id == GREMLIN_LEADER_ID {
            break;
        }
        if crate::content::monsters::is_gremlin_leader_minion_content_id(monster.content_id) {
            monster.gremlin_leader_slot = Some(next_slot);
            next_slot = next_slot.saturating_add(1);
        }
    }
}

fn target_city_encounter_spawn_for_run(
    run: &mut RunState,
    floor: u32,
    encounter_key: &str,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let spawns = crate::content::monsters::target_city_encounter_spawn_for_key_with_misc_rng(
        run.event_rng_seed as i64,
        floor,
        encounter_key,
        run.ascension,
        neow_lament,
        Some(&mut misc_rng),
    );
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    spawns
}

fn boss_combat_state_for_run(run: &RunState) -> CombatState {
    if run.current_act == 1 {
        return CombatState::hexaghost_fixture();
    }
    if run.current_act == 2 {
        let mut combat = CombatState::initial_fixture();
        combat.monsters = vec![monster_state_for_ascension(
            &BRONZE_AUTOMATON_A0,
            crate::MonsterId::new(1),
            run.ascension,
        )];
        return combat;
    }
    CombatState::hexaghost_fixture()
}

fn normal_combat_index_for_run(run: &RunState) -> usize {
    run.normal_combat_count as usize
}

fn target_spawn_monster_state(
    spawn: &TargetEncounterSpawn,
    index: usize,
    ascension: u8,
) -> MonsterState {
    let content_id = content_id_from_game_monster_id(spawn.name);
    let mut monster = get_monster_definition(content_id)
        .map(|definition| {
            monster_state_for_ascension(
                definition,
                crate::MonsterId::new(index as u64 + 1),
                ascension,
            )
        })
        .unwrap_or_else(|| {
            let mut fallback = CombatState::cultist_fixture()
                .monsters
                .into_iter()
                .next()
                .expect("cultist fixture has a monster");
            fallback.id = crate::MonsterId::new(index as u64 + 1);
            fallback
        });

    monster.hp = spawn.current_hp;
    monster.block = spawn.block;
    monster.alive = spawn.current_hp > 0;
    monster.powers = spawn_monster_powers(spawn);
    monster.rolled_attack_damage = spawn.rolled_attack_damage;
    if spawn.intent == "AttackAddSlimedToDiscard" {
        if let Some(damage) = spawn.rolled_attack_damage {
            monster.intent = crate::MonsterIntent::AttackAddSlimedToDiscard {
                damage,
                count: if spawn.name.ends_with("(L)") { 2 } else { 1 },
            };
        }
    } else if spawn.intent == "Attack" {
        if let Some(damage) = spawn.rolled_attack_damage {
            monster.intent = crate::MonsterIntent::Attack { damage };
        }
        if spawn.name == "Sentry" {
            monster.moves_executed = 1;
        }
    }
    monster
}

fn spawn_monster_powers(spawn: &TargetEncounterSpawn) -> MonsterPowers {
    let mut powers = MonsterPowers::default();
    for power in &spawn.powers {
        match power.id {
            "Curl Up" => powers.curl_up = power.amount,
            "Strength" => powers.strength = power.amount,
            "Ritual" => powers.ritual = power.amount,
            "Metallicize" => powers.metallicize = power.amount,
            "Plated Armor" => powers.plated_armor = power.amount,
            "Painful Stabs" => powers.painful_stabs = power.amount,
            "Malleable" => {
                powers.malleable = power.amount;
                powers.malleable_base = power.amount;
            }
            "Spore Cloud" => powers.spore_cloud = power.amount,
            "Minion" => powers.minion = power.amount,
            "Angry" => powers.anger = power.amount,
            "Generic Strength Up Power" => powers.strength_up = power.amount,
            _ => {}
        }
    }
    powers
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
            run.phase = RunPhase::Treasure;
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
    fn entering_rest_node_triggers_eternal_feather_heal() {
        let mut run = RunState::map_fixture();
        run.player_hp = 70;
        run.relics.push(crate::Relic::EternalFeather);

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("choose rest node");

        assert_eq!(next.phase, RunPhase::Rest);
        assert_eq!(next.player_hp, 76);
    }

    #[test]
    fn entering_combat_node_enters_combat() {
        let run = RunState::map_fixture();

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(1),
            },
        )
        .expect("choose combat node");

        assert_eq!(next.phase, RunPhase::Combat);
        assert!(next.combat.is_some());
    }

    #[test]
    fn entering_boss_node_enters_hexaghost_combat() {
        let mut run = RunState::map_fixture();
        let map = run.map.as_mut().expect("map fixture");
        map.current_node = MapNodeId::new(5);
        map.floor = 5;

        let next = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(6),
            },
        )
        .expect("choose boss node");

        assert_eq!(next.phase, RunPhase::Combat);
        assert_eq!(next.current_room_kind(), Some(RoomKind::Boss));
        let combat = next.combat.expect("boss combat");
        assert_eq!(combat.monsters.len(), 1);
        assert_eq!(
            combat.monsters[0].content_id,
            crate::content::monsters::HEXAGHOST_ID
        );
    }

    #[test]
    fn elite_spawn_preserves_book_of_stabbing_painful_stabs() {
        let spawn = crate::content::monsters::target_city_encounter_spawn_for_key(
            1,
            23,
            "Book of Stabbing",
            0,
            false,
        )
        .expect("Book of Stabbing spawn");

        let powers = spawn_monster_powers(&spawn[0]);

        assert_eq!(powers.painful_stabs, 1);
    }

    #[test]
    fn normal_spawn_preserves_snake_plant_malleable() {
        let spawn = crate::content::monsters::target_city_encounter_spawn_for_key(
            1,
            24,
            "Snake Plant",
            0,
            false,
        )
        .expect("Snake Plant spawn");

        let powers = spawn_monster_powers(&spawn[0]);

        assert_eq!(powers.malleable, 3);
        assert_eq!(powers.malleable_base, 3);
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
            if run.phase == RunPhase::Combat {
                run.phase = RunPhase::Idle;
                run.combat = None;
            }
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

        assert_eq!(run.phase, RunPhase::Treasure);
        assert!(run.event.is_none());
        assert!(run.treasure_room.is_some());
        assert_eq!(run.tiny_chest_counter, 0);
        assert_eq!(run.current_room_kind(), Some(RoomKind::Treasure));
        assert_eq!(run.event_rng_counter, 4);
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
        assert_eq!(run.event_rng_counter, 1);
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
