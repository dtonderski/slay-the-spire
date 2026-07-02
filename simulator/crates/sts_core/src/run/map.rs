use crate::{
    card::CardInstance,
    combat::{initialize_combat_piles_with_relics, CombatState, MonsterState},
    content::cards::WOUND_ID,
    content::monsters::{
        content_id_from_game_monster_id, donu_deca_boss_monsters_for_ascension,
        get_monster_definition, living_monster_missing_hp, monster_state_for_ascension,
        prepare_monster_intent_for_ascension, record_target_move,
        target_acid_slime_entry_intent_from_roll, target_beyond_encounter_spawn_for_key,
        target_book_of_stabbing_next_intent_from_roll_with_stab_count,
        target_bronze_orb_next_intent_from_roll, target_byrd_next_intent_from_roll,
        target_centurion_next_intent_from_roll, target_chosen_next_intent_from_roll,
        target_city_normal_encounter_spawn_at_combat_index,
        target_elite_encounter_spawn_at_combat_index, target_exploder_next_intent_from_roll,
        target_fungi_beast_next_intent_from_roll, target_gremlin_leader_next_intent_from_roll,
        target_healer_next_intent_from_roll, target_jaw_worm_next_intent_from_roll,
        target_large_acid_slime_next_intent_from_roll, target_louse_entry_intent_from_roll,
        target_normal_encounter_spawn_at_combat_index, target_orb_walker_next_intent_from_roll,
        target_reptomancer_next_intent_from_roll, target_repulsor_next_intent_from_roll,
        target_sentry_next_intent, target_shelled_parasite_next_intent_from_roll,
        target_slaver_blue_next_intent_from_roll, target_slaver_red_next_intent_from_roll,
        target_small_acid_slime_entry_intent_from_bool, target_snake_plant_next_intent_from_roll,
        target_snecko_next_intent_from_roll, target_spike_slime_entry_intent_from_roll,
        TargetEncounterSpawn, ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE, ACID_SLIME_S_A7_HP_RANGE,
        BOOK_OF_STABBING_ID, BRONZE_ORB_ID, BYRD_ID, CENTURION_ID, CHOSEN_ID, DAGGER_ID,
        DARKLING_ID, DECA_ID, EXPLODER_ID, FUNGI_BEAST_ID, GREEN_LOUSE_BITE_DAMAGE, GREEN_LOUSE_ID,
        GREEN_LOUSE_WEAK, GREMLIN_LEADER_ID, HEALER_ID, JAW_WORM_ID, LOUSE_CURL_STRENGTH,
        ORB_WALKER_ID, RED_LOUSE_BITE_DAMAGE, RED_LOUSE_ID, REPTOMANCER_ID, REPULSOR_ID, SENTRY_ID,
        SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLAVER_RED_ID, SNAKE_PLANT_ID, SNECKO_ID,
        SPIKE_SLIME_ID, TASKMASTER_ID,
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
    let monster_hp_rng = StsRng::new(run.event_rng_seed as i64 + i64::from(run.current_floor));
    let mut card_random_rng = Some(run.card_random_rng());
    // This local field is the target game's combat aiRng. Target monsterRng is the
    // run-level encounter-list stream.
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
    base.monster_rng = Some(monster_rng.clone());
    base.card_random_rng = card_random_rng;
    run.phase = RunPhase::Combat;
    let mut combat = run.init_combat_consuming_relics(base.clone());
    combat.monster_rng = Some(monster_rng);
    add_mark_of_pain_wounds_to_draw_pile(run, &mut combat);
    run.combat = Some(combat);
}

fn record_initial_monster_moves(combat: &mut CombatState) {
    for monster in &mut combat.monsters {
        if monster.alive {
            record_target_move(monster);
        }
        monster.initial_intent_locked = false;
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
        if monster.initial_intent_locked {
            continue;
        }
        let roll = rng.random_int(99);
        if monster.content_id == ACID_SLIME_ID && monster.hp <= ACID_SLIME_S_A7_HP_RANGE.max {
            let attack = combat.ascension < 17 && rng.random_bool();
            monster.intent =
                target_small_acid_slime_entry_intent_from_bool(attack, combat.ascension);
            if matches!(monster.intent, crate::MonsterIntent::Attack { .. }) {
                monster.moves_executed = 1;
            }
        } else if monster.content_id == ACID_SLIME_ID && monster.hp <= ACID_SLIME_M_A7_HP_RANGE.max
        {
            monster.intent = target_acid_slime_entry_intent_from_roll(monster.hp, roll);
            if matches!(monster.intent, crate::MonsterIntent::Attack { .. }) {
                monster.moves_executed = 1;
            }
        } else if monster.content_id == SPIKE_SLIME_ID {
            monster.intent = target_spike_slime_entry_intent_from_roll(monster.hp, roll);
        } else if monster.content_id == ACID_SLIME_ID && monster.hp > ACID_SLIME_M_A7_HP_RANGE.max {
            monster.intent = target_large_acid_slime_next_intent_from_roll(
                &monster.move_history,
                roll,
                rng,
                combat.ascension,
            );
        } else if monster.content_id == JAW_WORM_ID {
            monster.intent =
                target_jaw_worm_next_intent_from_roll(&monster.move_history, roll, rng);
        } else if monster.content_id == RED_LOUSE_ID {
            monster.intent = target_louse_entry_intent_from_roll(
                roll,
                monster.rolled_attack_damage,
                RED_LOUSE_BITE_DAMAGE,
                crate::MonsterIntent::StrengthAndBlock {
                    strength: LOUSE_CURL_STRENGTH,
                    block: 0,
                },
            );
        } else if monster.content_id == GREEN_LOUSE_ID {
            monster.intent = target_louse_entry_intent_from_roll(
                roll,
                monster.rolled_attack_damage,
                GREEN_LOUSE_BITE_DAMAGE,
                crate::MonsterIntent::ApplyPlayerWeak {
                    amount: GREEN_LOUSE_WEAK,
                },
            );
        } else if monster.content_id == CHOSEN_ID {
            monster.intent =
                target_chosen_next_intent_from_roll(&monster.move_history, roll, combat.ascension);
        } else if monster.content_id == BYRD_ID && monster.moves_executed == 0 {
            monster.intent = target_byrd_next_intent_from_roll(
                &monster.move_history,
                roll,
                rng,
                combat.ascension,
            );
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
        } else if monster.content_id == SLAVER_BLUE_ID {
            monster.intent = target_slaver_blue_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == SLAVER_RED_ID {
            monster.intent = target_slaver_red_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == SENTRY_ID {
            monster.intent =
                target_sentry_next_intent(&monster.move_history, index, combat.ascension);
        } else if monster.content_id == SNECKO_ID {
            monster.intent =
                target_snecko_next_intent_from_roll(&monster.move_history, roll, combat.ascension);
        } else if monster.content_id == BOOK_OF_STABBING_ID {
            let mut stab_count = monster.powers.book_stab_count.max(1);
            monster.intent = target_book_of_stabbing_next_intent_from_roll_with_stab_count(
                &monster.move_history,
                &mut stab_count,
                roll,
                combat.ascension,
            );
            monster.powers.book_stab_count = stab_count;
        } else if monster.content_id == TASKMASTER_ID {
            monster.intent = crate::MonsterIntent::AttackAddWoundsToDiscard {
                damage: 7,
                count: 1,
            };
        } else if monster.content_id == BRONZE_ORB_ID {
            monster.intent = target_bronze_orb_next_intent_from_roll(&monster.move_history, roll);
        } else if monster.content_id == REPTOMANCER_ID {
            let can_spawn = living_monster_count.saturating_sub(1) <= 3;
            monster.intent = target_reptomancer_next_intent_from_roll(
                &monster.move_history,
                roll,
                can_spawn,
                Some(rng),
                combat.ascension,
            );
        } else if monster.content_id == ORB_WALKER_ID {
            monster.intent = target_orb_walker_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == REPULSOR_ID {
            monster.intent = target_repulsor_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == EXPLODER_ID {
            monster.intent =
                target_exploder_next_intent_from_roll(monster.moves_executed, combat.ascension);
        } else if monster.content_id == GREMLIN_LEADER_ID {
            monster.intent = target_gremlin_leader_next_intent_from_roll(
                &monster.move_history,
                roll,
                Some(rng),
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
        assign_initial_reptomancer_dagger_slots(&mut combat.monsters);
    }
    combat
}

fn elite_combat_state_for_run(run: &mut RunState) -> CombatState {
    let combat_index = run.elite_combat_count as usize;
    let floor = u32::try_from(run.current_floor.max(1)).unwrap_or(1);
    let neow_lament = run.neow_lament_combats_remaining > 0;
    let act = match run.current_act {
        2 => TargetMapAct::City,
        3 => TargetMapAct::Beyond,
        _ => TargetMapAct::Exordium,
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
        assign_initial_reptomancer_dagger_slots(&mut combat.monsters);
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

fn assign_initial_reptomancer_dagger_slots(monsters: &mut [MonsterState]) {
    let Some(reptomancer_index) = monsters
        .iter()
        .position(|monster| monster.content_id == REPTOMANCER_ID)
    else {
        return;
    };
    let mut left_slot_assigned = false;
    let mut right_slot_assigned = false;
    for (index, monster) in monsters.iter_mut().enumerate() {
        if monster.content_id != DAGGER_ID {
            continue;
        }
        monster.powers.minion = 1;
        if index < reptomancer_index && !left_slot_assigned {
            monster.gremlin_leader_slot = Some(1);
            left_slot_assigned = true;
        } else if index > reptomancer_index && !right_slot_assigned {
            monster.gremlin_leader_slot = Some(0);
            right_slot_assigned = true;
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
        return match run.act1_boss {
            crate::run::Act1Boss::Hexaghost => CombatState::hexaghost_fixture(),
            crate::run::Act1Boss::SlimeBoss => CombatState::slime_boss_fixture(),
            crate::run::Act1Boss::Guardian => CombatState::guardian_fixture(),
        };
    }
    if run.current_act == 2 {
        let mut combat = CombatState::initial_fixture();
        let boss_key =
            crate::content::encounters::target_city_act_two_boss(run.monster_rng_seed as i64);
        let definition = get_monster_definition(content_id_from_game_monster_id(&boss_key))
            .unwrap_or_else(|| {
                get_monster_definition(crate::content::monsters::BRONZE_AUTOMATON_ID)
                    .expect("Bronze Automaton definition is registered")
            });
        combat.monsters = vec![monster_state_for_ascension(
            definition,
            crate::MonsterId::new(1),
            run.ascension,
        )];
        return combat;
    }
    if run.current_act == 3 {
        let mut combat = CombatState::initial_fixture();
        let boss_key =
            crate::content::encounters::target_beyond_act_three_boss(run.monster_rng_seed as i64);
        combat.monsters = if boss_key == "Donu and Deca" {
            donu_deca_boss_monsters_for_ascension(run.ascension)
        } else {
            let content_id = content_id_from_game_monster_id(&boss_key);
            let definition = get_monster_definition(content_id)
                .unwrap_or_else(|| get_monster_definition(DECA_ID).expect("Deca is registered"));
            vec![monster_state_for_ascension(
                definition,
                crate::MonsterId::new(1),
                run.ascension,
            )]
        };
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
    monster.max_hp = spawn.max_hp;
    monster.block = spawn.block;
    monster.alive = spawn.current_hp > 0;
    monster.powers = spawn_monster_powers(spawn);
    monster.rolled_attack_damage = spawn.rolled_attack_damage;
    if spawn.intent == "AttackAddSlimedToDiscard"
        && !(monster.content_id == ACID_SLIME_ID && monster.max_hp <= ACID_SLIME_M_A7_HP_RANGE.max)
    {
        if let Some(damage) = spawn.rolled_attack_damage {
            monster.intent = crate::MonsterIntent::AttackAddSlimedToDiscard {
                damage,
                count: if spawn.name.ends_with("(L)") { 2 } else { 1 },
            };
            monster.initial_intent_locked = true;
        }
    } else if spawn.intent == "ApplyPlayerFrailAndWeak" {
        monster.intent = crate::MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: observed_spike_slime_frail_amount(spawn, ascension),
            weak: 0,
        };
        monster.initial_intent_locked = true;
    } else if spawn.intent == "AddDazedToDiscard" {
        monster.intent = crate::MonsterIntent::AddDazedToDiscard { count: 2 };
        monster.initial_intent_locked = true;
    } else if spawn.intent == "AddDazedToDraw" {
        monster.intent = crate::MonsterIntent::AddDazedToDraw { count: 2 };
        monster.initial_intent_locked = true;
    } else if spawn.intent == "AddBurnToDiscardAndDraw" {
        monster.intent = crate::MonsterIntent::AddBurnToDiscardAndDraw {
            count: 1,
            damage: spawn.rolled_attack_damage.unwrap_or(10),
        };
        monster.rolled_attack_damage = None;
        monster.initial_intent_locked = true;
    } else if spawn.intent == "StrengthAndBlock" {
        let (strength, block) = if spawn.name == "Spiker" {
            (0, 0)
        } else {
            (3, 6)
        };
        monster.intent = crate::MonsterIntent::StrengthAndBlock { strength, block };
        monster.initial_intent_locked = true;
    } else if spawn.intent == "Attack" {
        if let Some(damage) = spawn.rolled_attack_damage {
            monster.intent = crate::MonsterIntent::Attack { damage };
            monster.initial_intent_locked = true;
        }
        if spawn.name == "Sentry" {
            monster.moves_executed = 1;
        }
    }
    monster
}

fn observed_spike_slime_frail_amount(spawn: &TargetEncounterSpawn, ascension: u8) -> i32 {
    let large = spawn.name.ends_with("(L)")
        || spawn.max_hp > crate::content::monsters::SPIKE_SLIME_M_A7_HP_RANGE.max;
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

fn spawn_monster_powers(spawn: &TargetEncounterSpawn) -> MonsterPowers {
    let mut powers = MonsterPowers::default();
    for power in &spawn.powers {
        match power.id {
            "Curl Up" => powers.curl_up = power.amount,
            "Strength" => powers.strength = power.amount,
            "Ritual" => powers.ritual = power.amount,
            "Metallicize" => powers.metallicize = power.amount,
            "Artifact" => powers.artifact = power.amount,
            "Flight" => powers.flight = power.amount,
            "Plated Armor" => powers.plated_armor = power.amount,
            "Thorns" => powers.spikes = power.amount,
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
            enter_normal_combat(run);
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
    use crate::{
        content::monsters::{
            target_book_of_stabbing_next_intent_from_roll, target_bronze_orb_next_intent_from_roll,
            target_exploder_next_intent_from_roll, target_orb_walker_next_intent_from_roll,
            target_repulsor_next_intent_from_roll, target_sentry_next_intent,
            target_slaver_red_next_intent_from_roll, target_snecko_next_intent_from_roll,
            BOOK_OF_STABBING_ID, BRONZE_ORB_ID, CULTIST_ID, DECA_ID, DONU_ID, EXPLODER_ID,
            ORB_WALKER_ID, REPULSOR_ID, SENTRY_ID, SLAVER_RED_ID, SNECKO_ID, SPIKE_SLIME_ID,
            TASKMASTER_ID,
        },
        ContentId, MonsterIntent,
    };

    #[test]
    fn source_locked_initial_intent_does_not_consume_ai_rng() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = CULTIST_ID;
        combat.monsters[0].intent = MonsterIntent::Attack { damage: 6 };
        combat.monsters[0].initial_intent_locked = true;

        let mut rng = StsRng::new(123);
        apply_initial_monster_ai_rolls(&mut combat, &mut rng);

        assert_eq!(rng.counter(), 0);
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::Attack { damage: 6 }
        );
    }

    #[test]
    fn unlocked_initial_intent_consumes_one_ai_roll() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = CULTIST_ID;

        let mut rng = StsRng::new(123);
        apply_initial_monster_ai_rolls(&mut combat, &mut rng);

        assert_eq!(rng.counter(), 1);
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::Ritual { amount: 3 }
        );
    }

    #[test]
    fn combat_entry_uses_source_helpers_for_close_monster_batch() {
        assert_initial_intent_from_roll(SLAVER_RED_ID, |history, roll, ascension| {
            target_slaver_red_next_intent_from_roll(history, roll, ascension)
        });
        assert_initial_intent_from_roll(SNECKO_ID, |history, roll, ascension| {
            target_snecko_next_intent_from_roll(history, roll, ascension)
        });
        assert_initial_intent_from_roll(BOOK_OF_STABBING_ID, |history, roll, ascension| {
            target_book_of_stabbing_next_intent_from_roll(history, roll, ascension)
        });
        assert_initial_intent_from_roll(BRONZE_ORB_ID, |history, roll, _ascension| {
            target_bronze_orb_next_intent_from_roll(history, roll)
        });
        assert_initial_intent_from_roll(ORB_WALKER_ID, |history, roll, ascension| {
            target_orb_walker_next_intent_from_roll(history, roll, ascension)
        });
        assert_initial_intent_from_roll(REPULSOR_ID, |history, roll, ascension| {
            target_repulsor_next_intent_from_roll(history, roll, ascension)
        });
        assert_initial_intent_from_roll(EXPLODER_ID, |_history, _roll, ascension| {
            target_exploder_next_intent_from_roll(0, ascension)
        });
        assert_initial_intent_from_roll(TASKMASTER_ID, |_history, _roll, _ascension| {
            MonsterIntent::AttackAddWoundsToDiscard {
                damage: 7,
                count: 1,
            }
        });
        assert_initial_intent_from_roll(DECA_ID, |_history, _roll, _ascension| {
            MonsterIntent::AttackMultiple {
                damage: 10,
                hits: 2,
            }
        });
        assert_initial_intent_from_roll(DONU_ID, |_history, _roll, _ascension| {
            MonsterIntent::StrengthAllMonsters { amount: 3 }
        });
    }

    #[test]
    fn sentry_entry_ignores_roll_value_and_uses_group_index_for_first_move() {
        let mut combat = CombatState::initial_fixture();
        combat.ascension = 3;
        combat.monsters = vec![
            MonsterState {
                content_id: SENTRY_ID,
                id: crate::ids::MonsterId::new(1),
                ..combat.monsters[0].clone()
            },
            MonsterState {
                content_id: SENTRY_ID,
                id: crate::ids::MonsterId::new(2),
                ..combat.monsters[0].clone()
            },
        ];
        let mut rng = StsRng::new(123);

        apply_initial_monster_ai_rolls(&mut combat, &mut rng);

        assert_eq!(rng.counter(), 2);
        assert_eq!(
            combat.monsters[0].intent,
            target_sentry_next_intent(&[], 0, 3)
        );
        assert_eq!(
            combat.monsters[1].intent,
            target_sentry_next_intent(&[], 1, 3)
        );
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::AddDazedToDiscard { count: 2 }
        );
        assert_eq!(
            combat.monsters[1].intent,
            MonsterIntent::Attack { damage: 10 }
        );
    }

    #[test]
    fn red_slaver_entry_consumes_roll_but_opens_with_stab() {
        let mut combat = CombatState::initial_fixture();
        combat.ascension = 2;
        combat.monsters[0].content_id = SLAVER_RED_ID;
        let mut rng = StsRng::new(0);

        apply_initial_monster_ai_rolls(&mut combat, &mut rng);

        assert_eq!(rng.counter(), 1);
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::Attack { damage: 14 }
        );
    }

    #[test]
    fn small_spike_slime_entry_consumes_roll_but_ignores_value() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = SPIKE_SLIME_ID;
        combat.monsters[0].hp = 10;
        combat.monsters[0].max_hp = 10;
        let mut rng = StsRng::new(123);

        apply_initial_monster_ai_rolls(&mut combat, &mut rng);

        assert_eq!(rng.counter(), 1);
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::Attack { damage: 5 }
        );
    }

    #[test]
    fn small_acid_slime_entry_consumes_boolean_only_below_a17() {
        let mut a16_combat = CombatState::initial_fixture();
        a16_combat.ascension = 16;
        a16_combat.monsters[0].content_id = crate::content::monsters::ACID_SLIME_ID;
        a16_combat.monsters[0].hp = 12;
        a16_combat.monsters[0].max_hp = 12;
        let mut a16_rng = StsRng::new(123);

        apply_initial_monster_ai_rolls(&mut a16_combat, &mut a16_rng);

        assert_eq!(a16_rng.counter(), 2);

        let mut a17_combat = a16_combat.clone();
        a17_combat.ascension = 17;
        a17_combat.monsters[0].intent = MonsterIntent::Attack { damage: 3 };
        a17_combat.monsters[0].moves_executed = 0;
        let mut a17_rng = StsRng::new(123);

        apply_initial_monster_ai_rolls(&mut a17_combat, &mut a17_rng);

        assert_eq!(a17_rng.counter(), 1);
        assert_eq!(
            a17_combat.monsters[0].intent,
            MonsterIntent::ApplyPlayerWeak { amount: 1 }
        );
    }

    fn assert_initial_intent_from_roll(
        content_id: ContentId,
        expected: fn(&[u8], i32, u8) -> MonsterIntent,
    ) {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = content_id;
        combat.ascension = 0;

        let rng = StsRng::new(123);
        let mut expected_rng = rng.clone();
        let roll = expected_rng.random_int(99);
        let mut actual_rng = rng;

        apply_initial_monster_ai_rolls(&mut combat, &mut actual_rng);

        assert_eq!(actual_rng.counter(), 1);
        assert_eq!(
            combat.monsters[0].intent,
            expected(&[], roll, combat.ascension)
        );
    }
}
