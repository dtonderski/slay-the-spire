use crate::{
    combat::{
        initialize_combat_piles_with_relics, CombatRngState, CombatState, MonsterState,
        PlayerState, SlimeSize,
    },
    content::monsters::{
        advance_reptomancer_monster_hp_rng_for_entry, content_id_from_game_monster_id,
        donu_deca_boss_monsters_for_ascension, get_monster_definition, living_monster_missing_hp,
        monster_state_for_ascension, prepare_monster_intent_for_ascension, record_target_move,
        requires_rolled_attack_damage, target_acid_slime_entry_intent_from_roll,
        target_beyond_encounter_spawn_for_key_with_misc_rng,
        target_book_of_stabbing_next_intent_from_roll_with_stab_count,
        target_bronze_orb_next_intent_from_roll, target_byrd_next_intent_from_roll,
        target_centurion_next_intent_from_roll, target_champ_next_intent_from_roll,
        target_chosen_next_intent_from_roll, target_city_normal_encounter_spawn_at_combat_index,
        target_elite_encounter_spawn_at_combat_index, target_exploder_next_intent_from_roll,
        target_fungi_beast_next_intent_from_roll, target_giant_head_next_intent_from_roll,
        target_gremlin_leader_next_intent_from_roll, target_healer_next_intent_from_roll,
        target_jaw_worm_next_intent_from_roll, target_large_acid_slime_next_intent_from_roll,
        target_louse_entry_intent_from_roll, target_monster_hp_range_for_content_id,
        target_normal_encounter_spawn_at_combat_index, target_orb_walker_next_intent_from_roll,
        target_reptomancer_next_intent_from_roll, target_repulsor_next_intent_from_roll,
        target_sentry_next_intent, target_shelled_parasite_next_intent_from_roll,
        target_slaver_blue_next_intent_from_roll, target_slaver_red_next_intent_from_roll,
        target_small_acid_slime_entry_intent_from_bool, target_snake_plant_next_intent_from_roll,
        target_snecko_next_intent_from_roll, target_spike_slime_entry_intent_from_roll,
        target_spiker_next_intent_from_roll, target_spire_growth_next_intent_from_roll,
        target_writhing_mass_next_intent_from_roll, TargetEncounterSpawn, TargetSpawnIntent,
        ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE, ACID_SLIME_S_A7_HP_RANGE, AWAKENED_ONE_ID,
        BOOK_OF_STABBING_ID, BRONZE_ORB_ID, BYRD_ID, CENTURION_ID, CHAMP_ID, CHOSEN_ID, CULTIST_A0,
        CULTIST_ID, DAGGER_ID, DARKLING_ID, EXPLODER_ID, FUNGI_BEAST_ID, GIANT_HEAD_ID,
        GREEN_LOUSE_ID, GREEN_LOUSE_WEAK, GREMLIN_LEADER_ID, GUARDIAN_ID, HEALER_ID, HEXAGHOST_ID,
        JAW_WORM_ID, LOUSE_CURL_STRENGTH, ORB_WALKER_ID, RED_LOUSE_ID, REPTOMANCER_ID, REPULSOR_ID,
        SENTRY_ID, SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLAVER_RED_ID, SLIME_BOSS_ID,
        SNAKE_PLANT_ID, SNECKO_ID, SPIKER_ID, SPIKE_SLIME_ID, SPIRE_GROWTH_ID, TASKMASTER_ID,
        WRITHING_MASS_ID,
    },
    map::{
        apply_map_action, legal_map_actions, reachable_nodes, validate_map_action,
        wing_boots_reachable_nodes, MapAction, RoomKind, TargetMapAct,
    },
    rng::{seed_for_floor, StsRng},
    MonsterPowers, Relic, RunPhase, RunState, SimError, SimResult,
};

use super::event::enter_event_screen;
use super::reward::setup_treasure_room;
use super::shop::enter_shop_room;
use super::state::{
    EventRoomChance, RunRngStream, DEFAULT_EVENT_ROOM_MONSTER_CHANCE,
    DEFAULT_EVENT_ROOM_SHOP_CHANCE, DEFAULT_EVENT_ROOM_TREASURE_CHANCE,
};

fn current_room_kind(run: &RunState) -> Option<RoomKind> {
    run.map.as_ref().and_then(|map_state| {
        map_state
            .map
            .node(map_state.current_node)
            .map(|node| node.room_kind)
    })
}

pub fn legal_map_actions_on_run(run: &RunState) -> SimResult<Vec<MapAction>> {
    run.validate()?;
    if run.phase != RunPhase::Idle {
        return Ok(Vec::new());
    }

    let Some(map_state) = run.map.as_ref() else {
        return Ok(Vec::new());
    };

    let mut actions = legal_map_actions(map_state)?;
    if run.relics.contains(&Relic::WingBoots) && run.wing_boots_charges > 0 {
        for node_id in wing_boots_reachable_nodes(map_state)? {
            let action = MapAction::ChooseNode { node_id };
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
    }
    actions.sort_unstable_by_key(|action| match action {
        MapAction::ChooseNode { node_id } => *node_id,
    });
    Ok(actions)
}

pub fn validate_map_action_on_run(run: &RunState, action: MapAction) -> SimResult<()> {
    run.validate()?;

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
        && wing_boots_action_is_legal(map_state, action)?
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
        && !reachable_nodes(map_state)?.contains(&chosen_node_id(action));
    let next_map = if uses_wing_boots {
        apply_wing_boots_map_action(map_state, action)?
    } else {
        apply_map_action(map_state, action)?
    };
    let next_floor = i32::try_from(next_map.floor)
        .map_err(|_| SimError::InvalidState("map floor exceeds supported run range"))?;

    let mut next = run.clone();
    next.map = Some(next_map);
    if let Some(map) = next.map.as_ref() {
        next.current_floor = next_floor;
        next.current_act = i32::from(map.act);
    }
    next.reinit_room_rngs_for_floor();
    next.current_room_override = None;
    if uses_wing_boots {
        next.wing_boots_charges =
            next.wing_boots_charges
                .checked_sub(1)
                .ok_or(SimError::InvalidState(
                    "Wing Boots traversal has no remaining charge",
                ))?;
    }
    next.apply_floor_entry_relics()?;

    if current_room_kind(&next) == Some(RoomKind::Rest) {
        next.apply_rest_site_entry_relics()?;
        next.phase = RunPhase::Rest;
        next.rest_room_complete = false;
    } else if current_room_kind(&next) == Some(RoomKind::Combat) {
        enter_normal_combat(&mut next)?;
    } else if current_room_kind(&next) == Some(RoomKind::Elite) {
        enter_elite_combat(&mut next)?;
    } else if current_room_kind(&next) == Some(RoomKind::Boss) {
        enter_boss_combat(&mut next)?;
    } else if current_room_kind(&next) == Some(RoomKind::Shop) {
        enter_shop_room(&mut next)?;
    } else if current_room_kind(&next) == Some(RoomKind::Treasure) {
        setup_treasure_room(&mut next);
        next.phase = RunPhase::Treasure;
    } else if current_room_kind(&next) == Some(RoomKind::Event) {
        apply_event_room_outcome(&mut next, last_room_was_shop)?;
    }

    Ok(next)
}

fn enter_normal_combat(run: &mut RunState) -> SimResult<()> {
    let next_combat_count = run
        .normal_combat_count
        .checked_add(1)
        .ok_or(SimError::InvalidState("normal combat count overflows u32"))?;
    let monsters = normal_combat_monsters_for_run(run)?;
    enter_combat_with_monsters(run, monsters)?;
    run.normal_combat_count = next_combat_count;
    Ok(())
}

fn enter_elite_combat(run: &mut RunState) -> SimResult<()> {
    let next_combat_count = run
        .elite_combat_count
        .checked_add(1)
        .ok_or(SimError::InvalidState("elite combat count overflows u32"))?;
    let monsters = elite_combat_monsters_for_run(run)?;
    enter_combat_with_monsters(run, monsters)?;
    run.elite_combat_count = next_combat_count;
    Ok(())
}

fn enter_boss_combat(run: &mut RunState) -> SimResult<()> {
    let monsters = boss_combat_monsters_for_run(run)?;
    enter_combat_with_monsters(run, monsters)?;
    Ok(())
}

pub(crate) fn enter_secret_portal_boss_combat(run: &mut RunState) -> SimResult<()> {
    enter_boss_combat(run)
}

fn enter_combat_with_monsters(run: &mut RunState, monsters: Vec<MonsterState>) -> SimResult<()> {
    run.reset_card_random_rng_for_combat();
    let mut shuffle_rng = StsRng::new(seed_for_floor(run.event_rng_seed as i64, run.current_floor));
    let monster_hp_rng = StsRng::new(seed_for_floor(run.event_rng_seed as i64, run.current_floor));
    let mut card_random_rng = run.card_random_rng();
    // This local field is the target game's combat aiRng. Target monsterRng is the
    // run-level encounter-list stream.
    let mut monster_rng = StsRng::new(seed_for_floor(
        run.monster_rng_seed as i64,
        run.current_floor,
    ));
    let piles = initialize_combat_piles_with_relics(
        &run.deck,
        &mut shuffle_rng,
        &mut card_random_rng,
        &run.relics,
    )?;
    let mut combat = CombatState::new_run_entry(
        PlayerState::new_run_entry(run.player_hp, run.player_max_hp, run.energy_per_turn)?,
        monsters,
        piles,
        run.relics.clone(),
        run.ascension,
        CombatRngState {
            shuffle_rng,
            monster_rng: monster_rng.clone(),
            monster_hp_rng,
            card_random_rng,
        },
    )?;
    advance_monster_hp_rng_for_combat_entry(
        &mut combat.monsters,
        &mut combat.rng.monster_hp_rng,
        run.ascension,
    );
    apply_initial_monster_ai_rolls(&mut combat, &mut monster_rng)?;
    record_initial_monster_moves(&mut combat);
    combat.rng.monster_rng = monster_rng.clone();
    run.phase = RunPhase::Combat;
    let mut initialized = run.init_combat_consuming_relics(combat)?;
    initialized.rng.monster_rng = monster_rng;
    add_mark_of_pain_wounds_to_draw_pile(run, &mut initialized)?;
    initialized.validate()?;
    run.combat = Some(initialized);
    Ok(())
}

fn advance_monster_hp_rng_for_combat_entry(
    monsters: &mut [MonsterState],
    monster_hp_rng: &mut StsRng,
    ascension: u8,
) {
    if monsters
        .iter()
        .any(|monster| monster.content_id == REPTOMANCER_ID)
    {
        advance_reptomancer_monster_hp_rng_for_entry(monster_hp_rng, ascension);
        return;
    }

    let awakened_one_encounter = monsters
        .iter()
        .any(|monster| monster.content_id == AWAKENED_ONE_ID);
    for monster in monsters {
        if let Some(range) = target_monster_hp_range_for_content_id(monster.content_id, ascension) {
            let hp = range.roll(monster_hp_rng);
            if monster.content_id == CULTIST_ID && awakened_one_encounter {
                monster.hp = hp;
                monster.max_hp = hp;
            }
        }
    }
}

fn record_initial_monster_moves(combat: &mut CombatState) {
    for monster in &mut combat.monsters {
        if monster.alive {
            record_target_move(monster);
        }
        monster.initial_intent_locked = false;
    }
}

/// Mark of Pain `atBattleStart` queues `MakeTempCardInDrawPileAction(Wound, 2, true, true)`
/// after the opening five-card draw and in relic-list order relative to other
/// `atBattleStart` hooks. Bag of Preparation's extra draw is also `atBattleStart`,
/// so when Mark of Pain appears earlier on the relic bar the Wounds are inserted
/// before those two cards are drawn. Target `CardGroup.addToRandomSpot` draws from
/// `cardRandomRng`, which has already been advanced by Confusion/Snecko cost rolls
/// (and any other pre-draw consumers) on that same stream. Must not re-seed from a
/// fresh run-level counter of 0.
pub(crate) fn add_mark_of_pain_wounds_to_draw_pile(
    run: &mut RunState,
    combat: &mut CombatState,
) -> SimResult<()> {
    if !run.relics.contains(&Relic::MarkOfPain) {
        return Ok(());
    }
    // Toolbox parks DrawCardAction behind ChooseOneColorless. Mark of Pain's
    // atBattleStart MakeTempCard sits after that draw, so skip until the
    // opening queue drains (FIDL01611). Other deferred opening draws (event
    // fight-two) still insert here, matching pre-Toolbox combat entry.
    if combat.pending_opening_hand_draw > 0 && combat.relics.contains(&Relic::Toolbox) {
        return Ok(());
    }
    crate::relic::insert_mark_of_pain_wounds(combat)?;
    run.store_rng_counter(RunRngStream::CardRandom, &combat.rng.card_random_rng);
    Ok(())
}

pub fn apply_initial_monster_ai_rolls(combat: &mut CombatState, rng: &mut StsRng) -> SimResult<()> {
    if combat.monsters.iter().any(|monster| {
        monster.alive
            && requires_rolled_attack_damage(monster.content_id)
            && monster.rolled_attack_damage.is_none()
    }) {
        return Err(SimError::InvalidState(
            "monster requires rolled attack damage",
        ));
    }
    let living_monster_count = combat
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .count();
    let alive_gremlin_count = gremlin_leader_alive_minion_count(&combat.monsters);
    let missing_hp = living_monster_missing_hp(&combat.monsters);
    for (index, monster) in combat.monsters.iter_mut().enumerate() {
        if !monster.alive {
            continue;
        }
        if monster.initial_intent_locked {
            if monster.content_id == JAW_WORM_ID {
                let _ = rng.random_int(99);
            }
            continue;
        }
        let roll = rng.random_int(99);
        if monster.content_id == ACID_SLIME_ID && monster.max_hp <= ACID_SLIME_S_A7_HP_RANGE.max {
            let attack = combat.ascension < 17 && rng.random_bool();
            monster.intent =
                target_small_acid_slime_entry_intent_from_bool(attack, combat.ascension);
            if matches!(monster.intent, crate::MonsterIntent::Attack { .. }) {
                monster.moves_executed = 1;
            }
        } else if monster.content_id == ACID_SLIME_ID
            && monster.max_hp <= ACID_SLIME_M_A7_HP_RANGE.max
        {
            monster.intent = target_acid_slime_entry_intent_from_roll(monster.max_hp, roll);
            if matches!(monster.intent, crate::MonsterIntent::Attack { .. }) {
                monster.moves_executed = 1;
            }
        } else if monster.content_id == SPIKE_SLIME_ID {
            monster.intent = target_spike_slime_entry_intent_from_roll(monster.max_hp, roll);
        } else if monster.content_id == ACID_SLIME_ID
            && monster.max_hp > ACID_SLIME_M_A7_HP_RANGE.max
        {
            monster.intent = target_large_acid_slime_next_intent_from_roll(
                &monster.move_history,
                roll,
                rng,
                combat.ascension,
            );
        } else if monster.content_id == AWAKENED_ONE_ID {
            monster.intent = crate::content::monsters::target_awakened_one_next_intent_from_roll(
                &monster.move_history,
                roll,
                monster.mode_shift,
                combat.ascension,
            );
        } else if monster.content_id == crate::content::monsters::TIME_EATER_ID {
            monster.intent = crate::content::monsters::target_time_eater_next_intent_from_roll(
                &monster.move_history,
                roll,
                monster.hp,
                monster.max_hp,
                combat.ascension,
                rng,
            );
        } else if monster.content_id == JAW_WORM_ID {
            monster.intent =
                target_jaw_worm_next_intent_from_roll(&monster.move_history, roll, rng);
        } else if monster.content_id == RED_LOUSE_ID {
            let attack_damage = monster.rolled_attack_damage.ok_or(SimError::InvalidState(
                "monster requires rolled attack damage",
            ))?;
            monster.intent = target_louse_entry_intent_from_roll(
                roll,
                attack_damage,
                crate::MonsterIntent::StrengthAndBlock {
                    strength: LOUSE_CURL_STRENGTH,
                    block: 0,
                },
            );
        } else if monster.content_id == GREEN_LOUSE_ID {
            let attack_damage = monster.rolled_attack_damage.ok_or(SimError::InvalidState(
                "monster requires rolled attack damage",
            ))?;
            monster.intent = target_louse_entry_intent_from_roll(
                roll,
                attack_damage,
                crate::MonsterIntent::ApplyPlayerWeak {
                    amount: GREEN_LOUSE_WEAK,
                },
            );
        } else if monster.content_id == CHOSEN_ID {
            monster.intent =
                target_chosen_next_intent_from_roll(&monster.move_history, roll, combat.ascension);
        } else if monster.content_id == CHAMP_ID {
            monster.intent = target_champ_next_intent_from_roll(
                &monster.move_history,
                roll,
                monster.hp,
                monster.max_hp,
                combat.ascension,
            );
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
                rng,
                combat.ascension,
            );
        } else if monster.content_id == ORB_WALKER_ID {
            monster.intent = target_orb_walker_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == SPIKER_ID {
            monster.intent = target_spiker_next_intent_from_roll(
                &monster.move_history,
                monster.powers.spiker_thorns_buffs,
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
                rng,
                alive_gremlin_count,
                combat.ascension,
            );
        } else if monster.content_id == SNAKE_PLANT_ID {
            monster.intent = target_snake_plant_next_intent_from_roll(
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == SPIRE_GROWTH_ID {
            monster.intent = target_spire_growth_next_intent_from_roll(
                monster.moves_executed,
                &monster.move_history,
                roll,
                false,
                combat.ascension,
            );
        } else if monster.content_id == GIANT_HEAD_ID {
            monster.intent = target_giant_head_next_intent_from_roll(
                monster.moves_executed,
                &monster.move_history,
                roll,
                combat.ascension,
            );
        } else if monster.content_id == WRITHING_MASS_ID {
            monster.intent = target_writhing_mass_next_intent_from_roll(
                true,
                &monster.move_history,
                false,
                roll,
                rng,
                combat.ascension,
            );
        } else if monster.content_id == crate::content::monsters::NEMESIS_ID {
            monster.intent = crate::content::monsters::target_nemesis_next_intent_from_roll(
                monster.moves_executed,
                &monster.move_history,
                roll,
                rng,
                combat.ascension,
            );
        } else if monster.content_id == DARKLING_ID {
            let attack_damage = monster.rolled_attack_damage.ok_or(SimError::InvalidState(
                "monster requires rolled attack damage",
            ))?;
            monster.intent =
                crate::content::monsters::target_darkling_next_intent_from_roll_with_rng(
                    &monster.move_history,
                    roll,
                    index,
                    attack_damage,
                    combat.ascension,
                    rng,
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
            monster.intent = prepare_monster_intent_for_ascension(monster, combat.ascension)?;
        }
    }
    Ok(())
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

fn encounter_floor(run: &RunState) -> SimResult<u32> {
    let floor = u32::try_from(run.current_floor)
        .map_err(|_| SimError::InvalidState("combat encounter requires a positive floor"))?;
    if floor == 0 {
        return Err(SimError::InvalidState(
            "combat encounter requires a positive floor",
        ));
    }
    Ok(floor)
}

fn normal_combat_monsters_for_run(run: &mut RunState) -> SimResult<Vec<MonsterState>> {
    let combat_index = normal_combat_index_for_run(run);
    let floor = encounter_floor(run)?;
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
                target_beyond_encounter_spawn_for_run(run, floor, &encounter_key, neow_lament)
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
    } else if run.current_act == 1 {
        target_normal_encounter_spawn_at_combat_index(
            run.event_rng_seed as i64,
            floor,
            combat_index,
            run.ascension,
            neow_lament,
        )
    } else {
        return Err(SimError::InvalidState(
            "normal combat requires a supported act",
        ));
    }
    .ok_or(SimError::InvalidState(
        "normal encounter spawn generation is unavailable",
    ))?;

    if spawns.is_empty() {
        return Err(SimError::InvalidState(
            "normal encounter generated no monsters",
        ));
    }
    let mut monsters = spawns
        .iter()
        .enumerate()
        .map(|(index, spawn)| target_spawn_monster_state(spawn, index, run.ascension))
        .collect::<SimResult<Vec<_>>>()?;
    assign_initial_gremlin_leader_slots(&mut monsters);
    assign_initial_reptomancer_dagger_slots(&mut monsters);
    Ok(monsters)
}

fn elite_combat_monsters_for_run(run: &mut RunState) -> SimResult<Vec<MonsterState>> {
    let combat_index = run.elite_combat_count as usize;
    let floor = encounter_floor(run)?;
    let neow_lament = run.neow_lament_combats_remaining > 0;
    let act = match run.current_act {
        1 => TargetMapAct::Exordium,
        2 => TargetMapAct::City,
        3 => TargetMapAct::Beyond,
        _ => {
            return Err(SimError::InvalidState(
                "elite combat requires a supported act",
            ));
        }
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
    .ok_or(SimError::InvalidState(
        "elite encounter spawn generation is unavailable",
    ))?;

    if spawns.is_empty() {
        return Err(SimError::InvalidState(
            "elite encounter generated no monsters",
        ));
    }
    let mut monsters = spawns
        .iter()
        .enumerate()
        .map(|(index, spawn)| target_spawn_monster_state(spawn, index, run.ascension))
        .collect::<SimResult<Vec<_>>>()?;
    assign_initial_gremlin_leader_slots(&mut monsters);
    assign_initial_reptomancer_dagger_slots(&mut monsters);
    Ok(monsters)
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
        &mut misc_rng,
    );
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    spawns
}

fn target_beyond_encounter_spawn_for_run(
    run: &mut RunState,
    floor: u32,
    encounter_key: &str,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let spawns = target_beyond_encounter_spawn_for_key_with_misc_rng(
        run.event_rng_seed as i64,
        floor,
        encounter_key,
        run.ascension,
        neow_lament,
        &mut misc_rng,
    );
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    spawns
}

fn boss_combat_monsters_for_run(run: &RunState) -> SimResult<Vec<MonsterState>> {
    if run.current_act == 3 && run.act3_boss == crate::run::Act3Boss::AwakenedOne {
        return Ok(vec![
            monster_state_for_ascension(&CULTIST_A0, crate::MonsterId::new(1), run.ascension),
            monster_state_for_ascension(&CULTIST_A0, crate::MonsterId::new(2), run.ascension),
            monster_state_for_ascension(
                get_monster_definition(AWAKENED_ONE_ID).expect("Awakened One definition"),
                crate::MonsterId::new(3),
                run.ascension,
            ),
        ]);
    }
    let content_id = match run.current_act {
        1 => match run.act1_boss {
            crate::run::Act1Boss::Hexaghost => HEXAGHOST_ID,
            crate::run::Act1Boss::SlimeBoss => SLIME_BOSS_ID,
            crate::run::Act1Boss::Guardian => GUARDIAN_ID,
        },
        2 => {
            let boss_key = crate::content::encounters::try_target_city_act_two_boss(
                run.monster_rng_seed as i64,
            )?;
            content_id_from_game_monster_id(&boss_key).ok_or(SimError::InvalidState(
                "Act 2 boss has unknown monster content",
            ))?
        }
        3 => match run.act3_boss {
            crate::run::Act3Boss::AwakenedOne => unreachable!("handled above"),
            crate::run::Act3Boss::TimeEater => crate::content::monsters::TIME_EATER_ID,
            crate::run::Act3Boss::DonuAndDeca => {
                return Ok(donu_deca_boss_monsters_for_ascension(run.ascension));
            }
        },
        _ => {
            return Err(SimError::InvalidState(
                "boss combat requires a supported act",
            ));
        }
    };
    let definition =
        get_monster_definition(content_id).ok_or(SimError::UnknownContent(content_id))?;
    Ok(vec![monster_state_for_ascension(
        definition,
        crate::MonsterId::new(1),
        run.ascension,
    )])
}

fn normal_combat_index_for_run(run: &RunState) -> usize {
    run.normal_combat_count as usize
}

fn target_spawn_monster_state(
    spawn: &TargetEncounterSpawn,
    index: usize,
    ascension: u8,
) -> SimResult<MonsterState> {
    let content_id = content_id_from_game_monster_id(spawn.name).ok_or(SimError::InvalidState(
        "encounter spawn has unknown monster content",
    ))?;
    let definition =
        get_monster_definition(content_id).ok_or(SimError::UnknownContent(content_id))?;
    let mut monster = monster_state_for_ascension(
        definition,
        crate::MonsterId::new(index as u64 + 1),
        ascension,
    );

    monster.hp = spawn.current_hp;
    monster.max_hp = spawn.max_hp;
    monster.slime_size = target_spawn_slime_size(spawn.name).or(monster.slime_size);
    monster.block = spawn.block;
    monster.alive = spawn.current_hp > 0;
    monster.powers = spawn_monster_powers(spawn, content_id)?;
    monster.rolled_attack_damage = spawn.rolled_attack_damage;
    monster.intent = match spawn.intent {
        TargetSpawnIntent::PendingAiRoll => crate::MonsterIntent::PendingAiRoll,
        TargetSpawnIntent::Attack { damage } => crate::MonsterIntent::Attack { damage },
        TargetSpawnIntent::AttackAndBlock { damage, block } => {
            crate::MonsterIntent::AttackAndBlock { damage, block }
        }
        TargetSpawnIntent::StrengthAndBlock { strength, block } => {
            crate::MonsterIntent::StrengthAndBlock { strength, block }
        }
        TargetSpawnIntent::ApplyPlayerFrailAndWeak { frail, weak } => {
            crate::MonsterIntent::ApplyPlayerFrailAndWeak { frail, weak }
        }
        TargetSpawnIntent::AttackAddSlimedToDiscard { damage, count } => {
            crate::MonsterIntent::AttackAddSlimedToDiscard { damage, count }
        }
        TargetSpawnIntent::AddDazedToDraw { count } => {
            crate::MonsterIntent::AddDazedToDraw { count }
        }
    };
    if matches!(spawn.intent, TargetSpawnIntent::Attack { .. }) && spawn.name == "Sentry" {
        monster.moves_executed = 1;
    }
    if spawn.name == "Jaw Worm"
        && spawn.block == 6
        && spawn
            .powers
            .iter()
            .any(|power| power.id == "Strength" && power.amount > 0)
    {
        monster.initial_intent_locked = true;
    }
    Ok(monster)
}

fn target_spawn_slime_size(name: &str) -> Option<SlimeSize> {
    match name {
        "SpikeSlime_S" | "Spike Slime (S)" | "AcidSlime_S" | "Acid Slime (S)" => {
            Some(SlimeSize::Small)
        }
        "SpikeSlime_M" | "Spike Slime (M)" | "AcidSlime_M" | "Acid Slime (M)" => {
            Some(SlimeSize::Medium)
        }
        "SpikeSlime_L" | "Spike Slime (L)" | "AcidSlime_L" | "Acid Slime (L)" => {
            Some(SlimeSize::Large)
        }
        _ => None,
    }
}

fn spawn_monster_powers(
    spawn: &TargetEncounterSpawn,
    content_id: crate::ContentId,
) -> SimResult<MonsterPowers> {
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
            "Explosive" => powers.explosive = power.amount,
            "Malleable" => {
                powers.malleable = power.amount;
                powers.malleable_base = power.amount;
            }
            "Spore Cloud" => powers.spore_cloud = power.amount,
            "Minion" => powers.minion = power.amount,
            "Angry" => powers.anger = power.amount,
            "Generic Strength Up Power" => powers.strength_up = power.amount,
            // Thievery is represented by AttackStealGold intent rather than a
            // persistent core monster power.
            "Thievery" => {}
            _ => return Err(SimError::UnsupportedMechanic(content_id)),
        }
    }
    Ok(powers)
}

fn wing_boots_action_is_legal(
    map_state: &crate::MapRunState,
    action: MapAction,
) -> SimResult<bool> {
    Ok(wing_boots_reachable_nodes(map_state)?.contains(&chosen_node_id(action)))
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
    let floor = map_state
        .floor
        .checked_add(1)
        .ok_or(SimError::InvalidState("map floor overflow"))?;
    Ok(crate::MapRunState {
        act: target.act,
        floor,
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

fn apply_event_room_outcome(run: &mut RunState, last_room_was_shop: bool) -> SimResult<()> {
    let mut next = run.clone();
    apply_event_room_outcome_inner(&mut next, last_room_was_shop)?;
    *run = next;
    Ok(())
}

fn apply_event_room_outcome_inner(run: &mut RunState, last_room_was_shop: bool) -> SimResult<()> {
    let mut rng = StsRng::with_counter(run.event_rng_seed as i64, run.event_rng_counter);
    // Target EventHelper.roll: float roll = rng.random(); index = (int)(roll * 100.0f).
    let roll_index = (rng.random_float() * 100.0) as u32;
    run.event_rng_counter = rng.counter();

    let raw_outcome = if apply_tiny_chest(run)? {
        EventRoomOutcome::Treasure
    } else {
        target_event_room_outcome(
            roll_index,
            run.event_room_monster_chance.weight(),
            run.event_room_shop_chance.weight(),
            run.event_room_treasure_chance.weight(),
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
            enter_normal_combat(run)?;
        }
        EventRoomOutcome::Shop => {
            run.current_room_override = Some(RoomKind::Shop);
            enter_shop_room(run)?;
        }
        EventRoomOutcome::Treasure => {
            run.current_room_override = Some(RoomKind::Treasure);
            setup_treasure_room(run);
            run.phase = RunPhase::Treasure;
        }
        EventRoomOutcome::Event => {
            run.current_room_override = Some(RoomKind::Event);
            enter_event_screen(run)?;
        }
    }
    Ok(())
}

fn target_event_room_outcome(
    roll_index: u32,
    monster_weight: u32,
    shop_weight: u32,
    treasure_weight: u32,
    last_room_was_shop: bool,
) -> EventRoomOutcome {
    let roll_index = u64::from(roll_index);
    let monster_size = u64::from(monster_weight);
    let shop_size = monster_size
        + if last_room_was_shop {
            0
        } else {
            u64::from(shop_weight)
        };
    let treasure_size = shop_size + u64::from(treasure_weight);

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

/// Target `EventHelper.roll` post-roll probability ramps use Java `float` add/reset.
fn update_event_room_chances(
    run: &mut RunState,
    raw_outcome: EventRoomOutcome,
    resolved_outcome: EventRoomOutcome,
) {
    run.event_room_monster_chance = if raw_outcome == EventRoomOutcome::Monster {
        EventRoomChance::new(DEFAULT_EVENT_ROOM_MONSTER_CHANCE)
    } else {
        run.event_room_monster_chance
            .saturating_add(DEFAULT_EVENT_ROOM_MONSTER_CHANCE)
    };

    run.event_room_shop_chance = if resolved_outcome == EventRoomOutcome::Shop {
        EventRoomChance::new(DEFAULT_EVENT_ROOM_SHOP_CHANCE)
    } else {
        run.event_room_shop_chance
            .saturating_add(DEFAULT_EVENT_ROOM_SHOP_CHANCE)
    };

    run.event_room_treasure_chance = if resolved_outcome == EventRoomOutcome::Treasure {
        EventRoomChance::new(DEFAULT_EVENT_ROOM_TREASURE_CHANCE)
    } else {
        run.event_room_treasure_chance
            .saturating_add(DEFAULT_EVENT_ROOM_TREASURE_CHANCE)
    };
}

fn apply_tiny_chest(run: &mut RunState) -> SimResult<bool> {
    if !run.relics.contains(&Relic::TinyChest) {
        return Ok(false);
    }

    if run.tiny_chest_counter >= crate::relic::TINY_CHEST_THRESHOLD {
        return Err(SimError::InvalidState(
            "Tiny Chest counter is outside its stable range",
        ));
    }
    let next_counter = run
        .tiny_chest_counter
        .checked_add(1)
        .ok_or(SimError::InvalidState("Tiny Chest counter overflows u32"))?;
    if next_counter >= crate::relic::TINY_CHEST_THRESHOLD {
        run.tiny_chest_counter = 0;
        Ok(true)
    } else {
        run.tiny_chest_counter = next_counter;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        card::CardInstance,
        content::cards::WOUND_ID,
        content::monsters::{
            target_book_of_stabbing_next_intent_from_roll, target_bronze_orb_next_intent_from_roll,
            target_exploder_next_intent_from_roll, target_giant_head_next_intent_from_roll,
            target_orb_walker_next_intent_from_roll, target_repulsor_next_intent_from_roll,
            target_sentry_next_intent, target_slaver_red_next_intent_from_roll,
            target_snecko_next_intent_from_roll, target_spiker_next_intent_from_roll,
            TargetSpawnPower, BOOK_OF_STABBING_ID, BRONZE_ORB_ID, CULTIST_ID, DECA_ID, DONU_ID,
            EXPLODER_ID, GIANT_HEAD_ID, ORB_WALKER_ID, REPULSOR_ID, SENTRY_ID, SLAVER_RED_ID,
            SNECKO_ID, SPIKER_ID, SPIKE_SLIME_ID, TASKMASTER_ID,
        },
        ids::CardId,
        relic::MARK_OF_PAIN_WOUNDS,
        ContentId, MonsterIntent,
    };

    #[test]
    fn mark_of_pain_wounds_consume_card_random_rng_after_snecko_opening_costs() {
        // Target order: opening hand draw (Snecko Confusion rolls cardRandomRng per
        // playable card) then Mark of Pain addToRandomSpot insertions on the same stream.
        let mut run = RunState::map_fixture();
        run.current_floor = 1;
        run.current_act = 1;
        run.energy_per_turn = 4;
        run.relics = vec![Relic::SneckoEye, Relic::MarkOfPain];
        run.deck = crate::content::deck::ironclad_starter_deck_for_ascension(0);
        // Enough non-innate cards that the draw pile is non-empty after a 7-card hand.
        for (index, content_id) in [
            crate::content::cards::CLEAVE_ID,
            crate::content::cards::POMMEL_STRIKE_ID,
            crate::content::cards::THUNDERCLAP_ID,
            crate::content::cards::INFLAME_ID,
            crate::content::cards::UPPERCUT_ID,
        ]
        .into_iter()
        .enumerate()
        {
            run.deck.push(CardInstance::new(
                CardId::new(100 + index as u64),
                content_id,
            ));
        }
        run.normal_encounter_list = vec!["Cultist".to_owned()];
        run.normal_combat_count = 0;

        enter_normal_combat(&mut run).expect("combat starts");
        let combat = run.combat.as_ref().expect("combat present");

        let wound_count = combat
            .piles
            .draw_pile
            .iter()
            .filter(|card| card.content_id == WOUND_ID)
            .count();
        assert_eq!(wound_count, MARK_OF_PAIN_WOUNDS);
        assert!(combat
            .piles
            .draw_pile
            .iter()
            .filter(|card| card.content_id == WOUND_ID)
            .all(|card| card.combat_only));
        assert!(combat
            .piles
            .hand
            .iter()
            .all(|card| card.content_id != WOUND_ID));
        // Hand size is Snecko-expanded (5 + 2).
        assert_eq!(combat.piles.hand.len(), 7);

        // 7 opening costs (Snecko hand size) + 2 random-spot inserts.
        assert_eq!(combat.rng.card_random_rng.counter(), 9);
        assert_eq!(run.card_random_rng_counter, 9);

        // Wounds must not match counter-0 insertion into the post-draw pile
        // (the previous bug ignored Snecko's seven cardRandomRng advances).
        let base_draw: Vec<ContentId> = combat
            .piles
            .draw_pile
            .iter()
            .filter(|card| card.content_id != WOUND_ID)
            .map(|card| card.content_id)
            .collect();
        let seed = run.rng_stream_state(RunRngStream::CardRandom).seed as i64;
        let mut buggy_rng = StsRng::with_counter(seed, 0);
        let mut buggy_draw = base_draw.clone();
        for _ in 0..MARK_OF_PAIN_WOUNDS {
            let bound = (buggy_draw.len() - 1) as i32;
            let index = buggy_rng.random_int(bound) as usize;
            buggy_draw.insert(index, WOUND_ID);
        }
        let actual_ids: Vec<ContentId> = combat
            .piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect();
        assert_ne!(
            actual_ids, buggy_draw,
            "wound positions should advance past Snecko cost rolls, not reuse counter 0"
        );

        let mut expected_rng = StsRng::with_counter(seed, 0);
        for _ in 0..7 {
            let _ = expected_rng.random_int(3);
        }
        let mut expected_draw = base_draw;
        for _ in 0..MARK_OF_PAIN_WOUNDS {
            let bound = (expected_draw.len() - 1) as i32;
            let index = expected_rng.random_int(bound) as usize;
            expected_draw.insert(index, WOUND_ID);
        }
        assert_eq!(actual_ids, expected_draw);
    }

    #[test]
    fn mark_of_pain_inserts_before_bag_of_preparation_draw() {
        // Mark of Pain atBattleStart is earlier on the relic bar than Bag of
        // Preparation, so addToRandomSpot uses the post-five remaining pile.
        // Inserting after the bag's two draws is FIDL01469 (Bite vs Wound).
        let mut run = RunState::map_fixture();
        run.current_floor = 1;
        run.current_act = 1;
        run.relics = vec![Relic::MarkOfPain, Relic::BagOfPreparation];
        run.deck = crate::content::deck::ironclad_starter_deck_for_ascension(0);
        for (index, content_id) in [
            crate::content::cards::CLEAVE_ID,
            crate::content::cards::POMMEL_STRIKE_ID,
            crate::content::cards::THUNDERCLAP_ID,
            crate::content::cards::INFLAME_ID,
            crate::content::cards::UPPERCUT_ID,
            crate::content::cards::TWIN_STRIKE_ID,
            crate::content::cards::HEAVY_BLADE_ID,
        ]
        .into_iter()
        .enumerate()
        {
            run.deck.push(CardInstance::new(
                CardId::new(100 + index as u64),
                content_id,
            ));
        }
        run.normal_encounter_list = vec!["Cultist".to_owned()];
        run.normal_combat_count = 0;

        enter_normal_combat(&mut run).expect("combat starts");
        let combat = run.combat.as_ref().expect("combat present");
        assert_eq!(combat.piles.hand.len(), 7);
        assert_eq!(
            combat
                .piles
                .hand
                .iter()
                .chain(combat.piles.draw_pile.iter())
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            MARK_OF_PAIN_WOUNDS
        );
        assert_eq!(combat.rng.card_random_rng.counter(), 2);

        let non_wound_draw: Vec<ContentId> = combat
            .piles
            .draw_pile
            .iter()
            .filter(|card| card.content_id != WOUND_ID)
            .map(|card| card.content_id)
            .collect();
        let seed = run.rng_stream_state(RunRngStream::CardRandom).seed as i64;
        let mut after_bag_rng = StsRng::with_counter(seed, 0);
        let mut after_bag_draw = non_wound_draw;
        for _ in 0..MARK_OF_PAIN_WOUNDS {
            let bound = (after_bag_draw.len() - 1) as i32;
            let index = after_bag_rng.random_int(bound) as usize;
            after_bag_draw.insert(index, WOUND_ID);
        }
        let actual_ids: Vec<ContentId> = combat
            .piles
            .draw_pile
            .iter()
            .map(|card| card.content_id)
            .collect();
        assert_ne!(
            actual_ids, after_bag_draw,
            "Mark of Pain must not insert into the post-Bag-of-Preparation pile"
        );
    }

    #[test]
    fn map_counter_overflow_fails_before_mutation() {
        let mut normal = RunState::map_fixture();
        normal.normal_combat_count = u32::MAX;
        let normal_before = normal.clone();
        assert_eq!(
            enter_normal_combat(&mut normal),
            Err(SimError::InvalidState("normal combat count overflows u32"))
        );
        assert_eq!(normal, normal_before);

        let mut elite = RunState::map_fixture();
        elite.elite_combat_count = u32::MAX;
        let elite_before = elite.clone();
        assert_eq!(
            enter_elite_combat(&mut elite),
            Err(SimError::InvalidState("elite combat count overflows u32"))
        );
        assert_eq!(elite, elite_before);

        let mut tiny_chest = RunState::map_fixture();
        tiny_chest.relics.push(Relic::TinyChest);
        tiny_chest.tiny_chest_counter = crate::relic::TINY_CHEST_THRESHOLD;
        let tiny_chest_before = tiny_chest.clone();
        assert_eq!(
            apply_tiny_chest(&mut tiny_chest),
            Err(SimError::InvalidState(
                "Tiny Chest counter is outside its stable range"
            ))
        );
        assert_eq!(tiny_chest, tiny_chest_before);

        // Target EventHelper accumulates Java floats without a hard u32 ceiling.
        // Large weights still select Monster when the roll index is inside the
        // monster band (first slots of the 100-entry table).
        assert_eq!(
            target_event_room_outcome(0, 50, 20, 10, false),
            EventRoomOutcome::Monster
        );
        assert_eq!(
            target_event_room_outcome(55, 50, 20, 10, false),
            EventRoomOutcome::Shop
        );
        assert_eq!(
            target_event_room_outcome(75, 50, 20, 10, false),
            EventRoomOutcome::Treasure
        );
        assert_eq!(
            target_event_room_outcome(90, 50, 20, 10, false),
            EventRoomOutcome::Event
        );
    }

    #[test]
    fn event_room_chance_float_accumulation_matches_java_weight_truncation() {
        // Reproduce the FIDL00199 path: monster, event, monster, event, then a
        // roll of 44. Integer percent storage would treat shop/treasure weights
        // as 15/10 (treasure through index 44); Java float ramps yield 14/9
        // (event from index 43).
        let mut shop = EventRoomChance::new(DEFAULT_EVENT_ROOM_SHOP_CHANCE);
        let mut treasure = EventRoomChance::new(DEFAULT_EVENT_ROOM_TREASURE_CHANCE);
        for _ in 0..4 {
            shop = shop.saturating_add(DEFAULT_EVENT_ROOM_SHOP_CHANCE);
            treasure = treasure.saturating_add(DEFAULT_EVENT_ROOM_TREASURE_CHANCE);
        }
        // After the second monster reset, one further event ramp remains on monster.
        let monster = EventRoomChance::new(DEFAULT_EVENT_ROOM_MONSTER_CHANCE)
            .saturating_add(DEFAULT_EVENT_ROOM_MONSTER_CHANCE);

        assert_eq!(monster.weight(), 20);
        assert_eq!(shop.weight(), 14);
        assert_eq!(treasure.weight(), 9);
        assert_eq!(
            target_event_room_outcome(
                44,
                monster.weight(),
                shop.weight(),
                treasure.weight(),
                false
            ),
            EventRoomOutcome::Event
        );
        // Integer percent ramps would have selected Treasure at the same roll.
        assert_eq!(
            target_event_room_outcome(44, 20, 15, 10, false),
            EventRoomOutcome::Treasure
        );
    }

    #[test]
    fn standalone_map_transition_rejects_floor_overflow() {
        let mut map = crate::map::milestone8_fixture();
        map.floor = u32::MAX;

        assert_eq!(
            apply_map_action(
                &map,
                MapAction::ChooseNode {
                    node_id: crate::MapNodeId::new(1),
                },
            ),
            Err(SimError::InvalidState("map floor overflow"))
        );
    }

    #[test]
    fn run_validation_rejects_unrepresentable_map_floor() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map fixture").floor = (i32::MAX as u32) + 1;

        assert_eq!(
            run.validate(),
            Err(SimError::InvalidState(
                "map floor exceeds supported run range"
            ))
        );
    }

    #[test]
    fn run_map_transition_rejects_next_unrepresentable_floor() {
        let mut run = RunState::map_fixture();
        run.map.as_mut().expect("map fixture").floor = i32::MAX as u32;

        assert_eq!(
            apply_map_action_on_run(
                &run,
                MapAction::ChooseNode {
                    node_id: crate::MapNodeId::new(1),
                },
            ),
            Err(SimError::InvalidState(
                "map floor exceeds supported run range"
            ))
        );
    }

    #[test]
    fn target_spawn_slime_size_preserves_target_class_independently_of_hp() {
        assert_eq!(
            target_spawn_slime_size("Spike Slime (S)"),
            Some(SlimeSize::Small)
        );
        assert_eq!(
            target_spawn_slime_size("AcidSlime_M"),
            Some(SlimeSize::Medium)
        );
        assert_eq!(
            target_spawn_slime_size("SpikeSlime_L"),
            Some(SlimeSize::Large)
        );
        assert_eq!(target_spawn_slime_size("Cultist"), None);
    }

    #[test]
    fn encounter_entry_rejects_missing_and_unknown_content() {
        assert!(crate::content::monsters::target_encounter_spawn_for_key(
            1,
            1,
            "not-an-encounter",
            0,
            false,
        )
        .is_none());

        let spawn = TargetEncounterSpawn {
            name: "not-a-monster",
            current_hp: 10,
            max_hp: 10,
            block: 0,
            intent: TargetSpawnIntent::PendingAiRoll,
            powers: Vec::new(),
            rolled_attack_damage: None,
        };
        assert_eq!(
            target_spawn_monster_state(&spawn, 0, 0),
            Err(SimError::InvalidState(
                "encounter spawn has unknown monster content"
            ))
        );

        let mut run = RunState::map_fixture();
        run.current_floor = 1;
        run.current_act = 2;
        run.normal_encounter_list = vec!["not-an-encounter".to_owned()];
        run.normal_combat_count = 0;
        assert_eq!(
            normal_combat_monsters_for_run(&mut run),
            Err(SimError::InvalidState(
                "normal encounter spawn generation is unavailable"
            ))
        );

        run.current_act = 4;
        assert_eq!(
            boss_combat_monsters_for_run(&run),
            Err(SimError::InvalidState(
                "boss combat requires a supported act"
            ))
        );
    }

    #[test]
    fn encounter_entry_preserves_explosive_and_rejects_unknown_spawn_powers() {
        let mut spawn = TargetEncounterSpawn {
            name: "Exploder",
            current_hp: 30,
            max_hp: 30,
            block: 0,
            intent: TargetSpawnIntent::Attack { damage: 9 },
            powers: vec![TargetSpawnPower {
                id: "Explosive",
                amount: 3,
            }],
            rolled_attack_damage: Some(9),
        };

        let monster = target_spawn_monster_state(&spawn, 0, 0).expect("supported power converts");
        assert_eq!(monster.powers.explosive, 3);

        spawn.powers = vec![TargetSpawnPower {
            id: "Unsupported Power",
            amount: 1,
        }];
        assert_eq!(
            target_spawn_monster_state(&spawn, 0, 0),
            Err(SimError::UnsupportedMechanic(EXPLODER_ID))
        );
    }

    #[test]
    fn encounter_entry_rejects_nonpositive_floor_instead_of_using_floor_one() {
        for current_floor in [-1, 0] {
            let mut normal_run = RunState::map_fixture();
            normal_run.current_floor = current_floor;
            assert_eq!(
                normal_combat_monsters_for_run(&mut normal_run),
                Err(SimError::InvalidState(
                    "combat encounter requires a positive floor"
                ))
            );

            let mut elite_run = RunState::map_fixture();
            elite_run.current_floor = current_floor;
            assert_eq!(
                elite_combat_monsters_for_run(&mut elite_run),
                Err(SimError::InvalidState(
                    "combat encounter requires a positive floor"
                ))
            );
        }
    }

    #[test]
    fn act_one_boss_entry_uses_the_run_ascension_instead_of_a0_fixtures() {
        let mut run = RunState::map_fixture();
        run.current_act = 1;
        run.ascension = 9;
        run.act1_boss = crate::run::Act1Boss::Hexaghost;
        run.current_room_override = Some(RoomKind::Boss);

        enter_boss_combat(&mut run).expect("A9 Hexaghost combat entry succeeds");
        let combat = run.combat.as_ref().expect("combat is published");

        assert_eq!(combat.monsters.len(), 1);
        assert_eq!(combat.monsters[0].content_id, HEXAGHOST_ID);
        assert_eq!(combat.monsters[0].max_hp, 287);
        assert_eq!(combat.monsters[0].hp, 287);
        combat.validate().expect("published combat is valid");
    }

    #[test]
    fn typed_spawn_intent_carries_complete_payload_without_damage_fallback() {
        let spawn = TargetEncounterSpawn {
            name: "Jaw Worm",
            current_hp: 40,
            max_hp: 40,
            block: 0,
            intent: TargetSpawnIntent::AttackAndBlock {
                damage: 123,
                block: 17,
            },
            powers: Vec::new(),
            rolled_attack_damage: None,
        };

        let monster =
            target_spawn_monster_state(&spawn, 0, 0).expect("typed spawn intent converts");
        assert_eq!(
            monster.intent,
            MonsterIntent::AttackAndBlock {
                damage: 123,
                block: 17,
            }
        );
        assert_eq!(monster.rolled_attack_damage, None);
    }

    #[test]
    fn variable_damage_spawns_resolve_pending_ai_before_validation() {
        let spawns = [
            TargetEncounterSpawn {
                name: "FuzzyLouseNormal",
                current_hp: 12,
                max_hp: 12,
                block: 0,
                intent: TargetSpawnIntent::PendingAiRoll,
                powers: Vec::new(),
                rolled_attack_damage: Some(7),
            },
            TargetEncounterSpawn {
                name: "Darkling",
                current_hp: 50,
                max_hp: 50,
                block: 0,
                intent: TargetSpawnIntent::PendingAiRoll,
                powers: Vec::new(),
                rolled_attack_damage: Some(10),
            },
        ];
        let mut combat = CombatState::initial_fixture();
        combat.monsters = spawns
            .iter()
            .enumerate()
            .map(|(index, spawn)| {
                target_spawn_monster_state(spawn, index, 0).expect("known target spawn")
            })
            .collect();
        assert!(combat
            .monsters
            .iter()
            .all(|monster| monster.intent == MonsterIntent::PendingAiRoll));

        let mut rng = StsRng::new(123);
        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("complete rolled profiles");

        assert_eq!(rng.counter(), 2);
        assert!(combat
            .monsters
            .iter()
            .all(|monster| monster.intent != MonsterIntent::PendingAiRoll));
        combat.validate().expect("initial AI is fully resolved");
    }

    #[test]
    fn missing_variable_damage_fails_before_initial_ai_rng_is_consumed() {
        let spawn = TargetEncounterSpawn {
            name: "FuzzyLouseNormal",
            current_hp: 12,
            max_hp: 12,
            block: 0,
            intent: TargetSpawnIntent::PendingAiRoll,
            powers: Vec::new(),
            rolled_attack_damage: None,
        };
        let mut combat = CombatState::initial_fixture();
        combat.monsters =
            vec![target_spawn_monster_state(&spawn, 0, 0).expect("known target spawn")];
        let original = combat.clone();
        let mut rng = StsRng::new(123);

        assert_eq!(
            apply_initial_monster_ai_rolls(&mut combat, &mut rng),
            Err(SimError::InvalidState(
                "monster requires rolled attack damage"
            ))
        );
        assert_eq!(rng.counter(), 0);
        assert_eq!(combat, original);
    }

    #[test]
    fn source_locked_initial_intent_does_not_consume_ai_rng() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = CULTIST_ID;
        combat.monsters[0].intent = MonsterIntent::Attack { damage: 6 };
        combat.monsters[0].initial_intent_locked = true;

        let mut rng = StsRng::new(123);
        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("supported monster intent");

        assert_eq!(rng.counter(), 0);
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::Attack { damage: 6 }
        );
    }

    #[test]
    fn locked_jaw_worm_horde_intents_advance_ai_rng() {
        let mut combat = CombatState::initial_fixture();
        let opening = MonsterIntent::AttackAndBlock {
            damage: 7,
            block: 5,
        };
        combat.monsters[0].content_id = JAW_WORM_ID;
        combat.monsters[0].intent = opening;
        combat.monsters[0].initial_intent_locked = true;

        let mut rng = StsRng::new(123);
        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("supported monster intent");

        assert_eq!(rng.counter(), 1);
        assert_eq!(combat.monsters[0].intent, opening);
    }

    #[test]
    fn unlocked_initial_intent_consumes_one_ai_roll() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = CULTIST_ID;

        let mut rng = StsRng::new(123);
        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("supported monster intent");

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
        assert_initial_intent_from_roll(GIANT_HEAD_ID, |history, roll, ascension| {
            target_giant_head_next_intent_from_roll(0, history, roll, ascension)
        });
        assert_initial_intent_from_roll(TASKMASTER_ID, |_history, _roll, _ascension| {
            MonsterIntent::AttackAddWoundsToDiscard {
                damage: 7,
                count: 1,
            }
        });
        assert_initial_intent_from_roll(DECA_ID, |_history, _roll, _ascension| {
            MonsterIntent::AttackMultipleAddDazedToDiscard {
                damage: 10,
                hits: 2,
                count: 2,
            }
        });
        assert_initial_intent_from_roll(DONU_ID, |_history, _roll, _ascension| {
            MonsterIntent::StrengthAllMonsters { amount: 3 }
        });
    }

    #[test]
    fn nemesis_entry_uses_consumed_ai_roll_instead_of_representative_intent() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = crate::content::monsters::NEMESIS_ID;
        combat.ascension = 3;

        let rng = StsRng::with_counter(22_079_335_079, 1);
        let mut expected_rng = rng.clone();
        let roll = expected_rng.random_int(99);
        assert_eq!(roll, 25);
        let expected = crate::content::monsters::target_nemesis_next_intent_from_roll(
            0,
            &[],
            roll,
            &mut expected_rng,
            combat.ascension,
        );
        let mut actual_rng = rng;

        apply_initial_monster_ai_rolls(&mut combat, &mut actual_rng)
            .expect("Nemesis opening intent is source-backed");

        assert_eq!(actual_rng.counter(), expected_rng.counter());
        assert_eq!(combat.monsters[0].intent, expected);
        assert!(matches!(expected, MonsterIntent::AttackMultiple { .. }));
    }

    #[test]
    fn hjtgfct_map_path_third_pick_enters_event_room() {
        let mut run = RunState::seeded_ironclad(32_291_153_573, 0);
        run.phase = RunPhase::Idle;
        run.event = None;

        let first = legal_map_actions_on_run(&run).expect("valid first map state");
        assert_eq!(first.len(), 3);
        run = apply_map_action_on_run(&run, first[1]).expect("x=2 first node");
        assert_eq!(run.current_room_kind(), Some(RoomKind::Combat));

        run.phase = RunPhase::Idle;
        run.combat = None;
        let second = legal_map_actions_on_run(&run).expect("valid second map state");
        assert_eq!(second.len(), 1);
        run = apply_map_action_on_run(&run, second[0]).expect("x=3 second node");
        assert_eq!(run.current_room_kind(), Some(RoomKind::Combat));

        run.phase = RunPhase::Idle;
        run.combat = None;
        let third = legal_map_actions_on_run(&run).expect("valid third map state");
        assert_eq!(third.len(), 1);
        run = apply_map_action_on_run(&run, third[0]).expect("x=2 third node");
        assert_eq!(run.current_room_kind(), Some(RoomKind::Event));
        assert_eq!(run.phase, RunPhase::Event);
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

        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("supported monster intent");

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

        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("supported monster intent");

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

        apply_initial_monster_ai_rolls(&mut combat, &mut rng).expect("supported monster intent");

        assert_eq!(rng.counter(), 1);
        assert_eq!(
            combat.monsters[0].intent,
            MonsterIntent::Attack { damage: 5 }
        );
    }

    #[test]
    fn spiker_entry_uses_the_common_ai_roll() {
        let mut combat = CombatState::initial_fixture();
        combat.monsters[0].content_id = SPIKER_ID;
        combat.monsters[0].powers.spiker_thorns_buffs = 0;
        let mut expected_rng = StsRng::new(34_961_238_615_630_i64.wrapping_add(37));
        let roll = expected_rng.random_int(99);
        let expected = target_spiker_next_intent_from_roll(&[], 0, roll, combat.ascension);
        let mut actual_rng = StsRng::new(34_961_238_615_630_i64.wrapping_add(37));

        apply_initial_monster_ai_rolls(&mut combat, &mut actual_rng)
            .expect("supported monster intent");

        assert_eq!(actual_rng.counter(), expected_rng.counter());
        assert_eq!(combat.monsters[0].intent, expected);
    }

    #[test]
    fn small_acid_slime_entry_consumes_boolean_only_below_a17() {
        let mut a16_combat = CombatState::initial_fixture();
        a16_combat.ascension = 16;
        a16_combat.monsters[0].content_id = crate::content::monsters::ACID_SLIME_ID;
        a16_combat.monsters[0].hp = 12;
        a16_combat.monsters[0].max_hp = 12;
        let mut a16_rng = StsRng::new(123);

        apply_initial_monster_ai_rolls(&mut a16_combat, &mut a16_rng)
            .expect("supported monster intent");

        assert_eq!(a16_rng.counter(), 2);

        let mut a17_combat = a16_combat.clone();
        a17_combat.ascension = 17;
        a17_combat.monsters[0].intent = MonsterIntent::Attack { damage: 3 };
        a17_combat.monsters[0].moves_executed = 0;
        let mut a17_rng = StsRng::new(123);

        apply_initial_monster_ai_rolls(&mut a17_combat, &mut a17_rng)
            .expect("supported monster intent");

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

        apply_initial_monster_ai_rolls(&mut combat, &mut actual_rng)
            .expect("supported monster intent");

        assert_eq!(actual_rng.counter(), 1);
        assert_eq!(
            combat.monsters[0].intent,
            expected(&[], roll, combat.ascension)
        );
    }
}
