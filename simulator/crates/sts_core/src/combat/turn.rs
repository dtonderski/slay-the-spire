use crate::{
    combat::turn_powers::{
        apply_end_of_monster_turn_powers, apply_end_of_monster_turn_powers_without_ritual,
        apply_end_of_player_turn_powers,
    },
    combat::{
        draw::{
            apply_confusion_cost_randomization, apply_fire_breathing_on_draw,
            draw_cards_with_sts_rng, draw_cards_without_shuffle, evolve_extra_draw_count,
            shuffle_discard_into_draw, shuffle_discard_into_draw_sts,
        },
        hand::{discard_end_of_turn_hand, resolve_end_of_turn_doubt, resolve_end_of_turn_hand},
        piles::{add_cards_to_discard, add_cards_to_draw_random_spot},
    },
    combat::{CombatPhase, CombatState},
    content::cards::{BURN_ID, WOUND_ID},
    content::monsters::{
        apply_bronze_automaton_orb_spawn, apply_collector_spawn_torch_heads,
        apply_gremlin_leader_encourage, apply_gremlin_leader_rally_representative,
        apply_gremlin_leader_rally_target, apply_heal_all_monsters, apply_large_acid_slime_split,
        apply_large_spike_slime_split, apply_monster_intent_with_card_rng,
        apply_reptomancer_dagger_spawn, apply_slime_boss_split, apply_strength_all_monsters,
        clear_lagavulin_metallicize_if_awake, heal_monster_to_definition_cap,
        living_monster_missing_hp, prepare_monster_intent_for_ascension, record_target_move,
        target_book_of_stabbing_next_intent_from_roll_with_stab_count,
        target_bronze_automaton_next_intent, target_bronze_orb_next_intent_from_roll,
        target_byrd_flight_amount, target_byrd_go_airborne_intent,
        target_byrd_next_intent_from_roll, target_centurion_next_intent_from_roll,
        target_chosen_next_intent_from_roll, target_collector_next_intent_from_roll,
        target_exploder_next_intent_from_roll, target_fungi_beast_next_intent_from_roll,
        target_giant_head_next_intent_from_roll, target_gremlin_leader_next_intent_from_roll,
        target_gremlin_nob_next_intent_from_roll,
        target_gremlin_wizard_direct_next_intent_after_turn, target_grounded_byrd_next_intent,
        target_healer_next_intent_from_roll, target_jaw_worm_next_intent_from_roll,
        target_lagavulin_direct_wake_attack_intent, target_large_acid_slime_next_intent_from_roll,
        target_looter_direct_next_intent_after_turn, target_louse_next_intent_from_roll,
        target_maw_next_intent_from_roll, target_medium_acid_slime_next_intent_from_roll,
        target_medium_or_large_spike_slime_next_intent_from_roll,
        target_mugger_direct_next_intent_after_turn, target_nemesis_next_intent_from_roll,
        target_orb_walker_next_intent_from_roll, target_reptomancer_next_intent_from_roll,
        target_repulsor_next_intent_from_roll, target_sentry_next_intent,
        target_shelled_parasite_next_intent_from_roll, target_slaver_blue_next_intent_from_roll,
        target_slaver_red_next_intent_from_roll, target_small_acid_slime_followup_intent,
        target_snake_plant_next_intent_from_roll, target_snecko_next_intent_from_roll,
        target_spheric_guardian_next_intent_from_roll, target_spiker_next_intent_from_roll,
        target_spire_growth_next_intent_from_roll, ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE,
        ACID_SLIME_S_A7_HP_RANGE, BOOK_OF_STABBING_ID, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, BYRD_ID,
        CENTURION_ID, CHOSEN_ID, DARKLING_ID, DECA_ID, EXPLODER_ID, FUNGI_BEAST_ID, GIANT_HEAD_ID,
        GREEN_LOUSE_BITE_DAMAGE, GREEN_LOUSE_ID, GREEN_LOUSE_WEAK, GREMLIN_LEADER_ID,
        GREMLIN_NOB_ID, GREMLIN_TSUNDERE_ID, GREMLIN_WIZARD_ID, HEALER_ID, HEXAGHOST_ID,
        JAW_WORM_ID, LAGAVULIN_ID, LOOTER_ID, LOUSE_CURL_STRENGTH, MAW_ID, MUGGER_ID, NEMESIS_ID,
        ORB_WALKER_ID, RED_LOUSE_BITE_DAMAGE, RED_LOUSE_ID, REPTOMANCER_ID, REPULSOR_ID, SENTRY_ID,
        SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLAVER_RED_ID, SLIME_BOSS_ID, SNAKE_PLANT_ID,
        SNECKO_ID, SPHERIC_GUARDIAN_ID, SPIKER_ID, SPIKE_SLIME_ID, SPIKE_SLIME_S_A7_HP_RANGE,
        SPIRE_GROWTH_ID, THE_COLLECTOR_ID, TORCH_HEAD_ID, TRANSIENT_ID,
    },
    ids::MonsterId,
    rng::{SimulatorRng, StsRng},
    TargetRequirement,
};

const HAND_SIZE: usize = 5;

/// Simplified milestone timing:
///
/// 1. Ending the player turn discards the remaining hand.
/// 2. The monster turn consumes current player block before HP.
/// 3. Player block clears after the monster turn, before the next hand is drawn.
/// 4. Monster vulnerable decrements by 1 during monster-turn cleanup.
/// 5. The next player turn refills energy and draws from the draw pile without shuffle.
pub fn end_player_turn(state: &CombatState) -> CombatState {
    let mut next = state.clone();
    let started_with_living_monster = state.monsters.iter().any(|monster| monster.alive);

    apply_end_of_player_turn_powers(&mut next);
    resolve_end_of_turn_hand(&mut next);
    if finish_combat_if_over(&mut next, started_with_living_monster) {
        return next;
    }
    resolve_end_of_turn_doubt(&mut next);
    crate::relic::apply_end_of_player_turn_relics(&mut next);
    if finish_combat_if_over(&mut next, started_with_living_monster) {
        return next;
    }
    discard_end_of_turn_hand(&mut next);
    next.discard_reshuffle_limit = Some(next.piles.discard_pile.len());
    apply_pending_player_spikes_damage(&mut next);
    if next.player.hp <= 0 {
        next.phase = CombatPhase::Lost;
        next.discard_reshuffle_limit = None;
        return next;
    }
    clear_living_monster_block(&mut next);
    next.phase = CombatPhase::MonsterTurn;
    run_monster_turn(&mut next);

    if next.player.hp <= 0 {
        next.player.hp = 0;
        next.phase = CombatPhase::Lost;
        next.discard_reshuffle_limit = None;
        return next;
    }
    if finish_combat_if_over(&mut next, started_with_living_monster) {
        next.discard_reshuffle_limit = None;
        return next;
    }

    start_player_turn(&mut next);
    next.discard_reshuffle_limit = None;
    next
}

fn apply_pending_player_spikes_damage(state: &mut CombatState) {
    let damage = std::mem::take(&mut state.pending_player_spikes_damage);
    if damage <= 0 {
        return;
    }
    let hp_loss =
        crate::combat::damage::reflect_spikes_to_player(&mut state.player, &state.relics, damage);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
}

fn clear_living_monster_block(state: &mut CombatState) {
    for monster in &mut state.monsters {
        if monster.alive && monster.content_id != SPHERIC_GUARDIAN_ID {
            monster.block = 0;
        }
    }
}

pub fn start_player_turn(state: &mut CombatState) {
    crate::relic::reset_turn_relic_counters(state);
    reset_turn_only_temp_costs(state);
    if !crate::relic::preserves_energy_between_turns(&state.relics) {
        state.player.energy = state.player.max_energy;
    }
    state.player.cannot_draw = false;
    state.player.temp_strength = 0;
    state.player.temp_thorns = 0;
    state.player.temp_rage_block = 0;
    state.double_tap_pending = 0;
    if state.player.no_block_turns > 0 {
        state.player.no_block_turns -= 1;
    }
    if state.player.temp_dexterity > 0 {
        state.player.powers.dexterity -= state.player.temp_dexterity;
        state.player.temp_dexterity = 0;
    }
    state.player.energy += state.player.powers.berserk;
    crate::relic::apply_start_of_player_turn_relics(state);
    apply_start_of_turn_brutality(state);
    if state.player.hp <= 0 {
        state.phase = CombatPhase::Lost;
        return;
    }
    apply_start_of_turn_magnetism(state);
    draw_next_hand_without_shuffle(state);
    crate::relic::apply_start_of_player_turn_post_draw_relics(state);
    apply_start_of_turn_mayhem(state);
    if state.player.hp <= 0 {
        state.phase = CombatPhase::Lost;
        return;
    }
    if state.monsters.iter().all(|monster| !monster.alive) {
        let was_already_won = state.phase == CombatPhase::Won;
        state.phase = CombatPhase::Won;
        if !was_already_won {
            crate::combat::apply_burning_blood(state);
        }
        return;
    }
    state.phase = CombatPhase::WaitingForPlayer;
}

fn apply_start_of_turn_brutality(state: &mut CombatState) {
    for _ in 0..state.player.powers.brutality.max(0) {
        let hp_loss = crate::combat::hp_loss::lose_player_hp(state, 1);
        crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss);
        if state.player.hp <= 0 {
            return;
        }
        crate::combat::transition::player_draw_cards(state, 1);
    }
}

fn apply_start_of_turn_magnetism(state: &mut CombatState) {
    if state.monsters.iter().all(|monster| !monster.alive) {
        return;
    }

    for _ in 0..state.player.powers.magnetism.max(0) {
        let content_id = crate::combat::card_effects::magnetism_generated_colorless_card(state);
        let next_id = crate::CardId::new(state.piles.max_card_instance_id() + 1);
        let generated = crate::CardInstance {
            combat_only: true,
            ..crate::CardInstance::new(next_id, content_id)
        };
        if state.piles.hand.len() >= 10 {
            state.piles.discard_pile.push(generated);
        } else {
            state.piles.hand.push(generated);
        }
    }
}

fn apply_start_of_turn_mayhem(state: &mut CombatState) {
    for _ in 0..state.player.powers.mayhem.max(0) {
        let random_target = mayhem_random_living_target(state);
        let Some(definition) = state
            .piles
            .draw_pile
            .last()
            .and_then(|card| crate::content::cards::get_card_definition(card.content_id))
        else {
            return;
        };
        if definition.keywords.unplayable {
            continue;
        }
        let target = if definition.target == TargetRequirement::Enemy {
            random_target
        } else {
            None
        };
        if crate::combat::transition::apply_play_top_draw_card_to_state(state, target).is_err() {
            return;
        }
        if state.player.hp <= 0 || state.monsters.iter().all(|monster| !monster.alive) {
            return;
        }
    }
}

fn mayhem_random_living_target(state: &mut CombatState) -> Option<MonsterId> {
    let living = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    if living.is_empty() {
        return None;
    }
    let index = if let Some(rng) = state.card_random_rng.as_mut() {
        rng.random_int((living.len() - 1) as i32) as usize
    } else {
        0
    };
    living.get(index).copied()
}

fn finish_combat_if_over(state: &mut CombatState, started_with_living_monster: bool) -> bool {
    if state.player.hp <= 0 {
        state.phase = CombatPhase::Lost;
        return true;
    }

    if started_with_living_monster && state.monsters.iter().all(|monster| !monster.alive) {
        state.phase = CombatPhase::Won;
        crate::combat::apply_burning_blood(state);
        return true;
    }

    false
}

fn reset_turn_only_temp_costs(state: &mut CombatState) {
    for pile in [
        &mut state.piles.hand,
        &mut state.piles.draw_pile,
        &mut state.piles.discard_pile,
        &mut state.piles.exhaust_pile,
    ] {
        for card in pile {
            if card.temp_cost_turn_only {
                card.temp_cost = None;
                card.temp_cost_turn_only = false;
            }
        }
    }
}

fn run_monster_turn(state: &mut CombatState) {
    let ascension = state.ascension;
    let relics = state.relics.clone();
    let mut skip_ritual_tick = Vec::new();
    let turn_order = state
        .monsters
        .iter()
        .map(|monster| monster.id)
        .collect::<Vec<_>>();
    for actor_id in turn_order {
        let Some(index) = state
            .monsters
            .iter()
            .position(|monster| monster.id == actor_id)
        else {
            continue;
        };
        if !state.monsters[index].alive && !is_half_dead_darkling(&state.monsters[index]) {
            continue;
        }
        clear_lagavulin_metallicize_if_awake(&mut state.monsters[index]);
        match state.monsters[index].intent {
            crate::MonsterIntent::Attack { damage }
                if is_half_dead_darkling(&state.monsters[index]) && damage == 0 =>
            {
                state.monsters[index].moves_executed += 1;
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::Stun if is_half_dead_darkling(&state.monsters[index]) => {
                state.monsters[index].alive = true;
                state.monsters[index].escaped = false;
                state.monsters[index].hp = state.monsters[index].max_hp / 2;
                state.monsters[index].moves_executed += 1;
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::HealAllMonsters { amount } => {
                apply_heal_all_monsters(&mut state.monsters, ascension, amount);
                state.monsters[index].moves_executed += 1;
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::StrengthAllMonsters { amount } => {
                apply_strength_all_monsters(&mut state.monsters, amount);
                state.monsters[index].moves_executed += 1;
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::StrengthAndBlock { strength, block }
                if state.monsters[index].content_id == THE_COLLECTOR_ID =>
            {
                apply_strength_all_monsters(&mut state.monsters, strength);
                if let Some(monster) = state
                    .monsters
                    .iter_mut()
                    .find(|monster| monster.id == actor_id)
                {
                    monster.block += block;
                    monster.moves_executed += 1;
                }
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::EncourageGremlins { strength, block } => {
                let leader_id = state.monsters[index].id;
                if state.monsters[index].content_id == GREMLIN_LEADER_ID {
                    let _ = state.monster_rng.as_mut().map(|rng| rng.random_int(2));
                }
                apply_gremlin_leader_encourage(&mut state.monsters, leader_id, strength, block);
                state.monsters[index].moves_executed += 1;
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::Attack { damage }
                if state.monsters[index].content_id == BYRD_ID && damage == 3 =>
            {
                let player_snapshot = state.player.clone();
                let damage = apply_monster_intent_with_card_rng(
                    &mut state.monsters[index],
                    &mut state.player,
                    &mut state.piles,
                    ascension,
                    &player_snapshot,
                    &state.relics,
                    state.card_random_rng.as_mut(),
                );
                let painful_stabs = state.monsters[index].powers.painful_stabs;
                apply_monster_pending_effects(state, damage, 1, painful_stabs, None, 0, 0);
                record_target_move(&mut state.monsters[index]);
                state.monsters[index].intent = target_byrd_go_airborne_intent();
                record_target_move(&mut state.monsters[index]);
                continue;
            }
            crate::MonsterIntent::SummonGremlins { count } => {
                let summoner_id = state.monsters[index].id;
                if state.monsters[index].content_id == BRONZE_AUTOMATON_ID {
                    apply_bronze_automaton_orb_spawn(
                        &mut state.monsters,
                        summoner_id,
                        state.monster_rng.as_mut(),
                        state.monster_hp_rng.as_mut(),
                        ascension,
                    );
                } else if state.monsters[index].content_id == THE_COLLECTOR_ID {
                    apply_collector_spawn_torch_heads(
                        &mut state.monsters,
                        count,
                        state.monster_rng.as_mut(),
                        state.monster_hp_rng.as_mut(),
                        ascension,
                    );
                } else if state.monsters[index].content_id == ACID_SLIME_ID {
                    apply_large_acid_slime_split(
                        &mut state.monsters,
                        summoner_id,
                        state.monster_rng.as_mut(),
                        ascension,
                    );
                } else if state.monsters[index].content_id == SPIKE_SLIME_ID {
                    apply_large_spike_slime_split(
                        &mut state.monsters,
                        summoner_id,
                        state.monster_rng.as_mut(),
                        ascension,
                    );
                } else if state.monsters[index].content_id == SLIME_BOSS_ID {
                    apply_slime_boss_split(
                        &mut state.monsters,
                        summoner_id,
                        state.monster_rng.as_mut(),
                        ascension,
                    );
                } else if state.monsters[index].content_id == REPTOMANCER_ID {
                    apply_reptomancer_dagger_spawn(
                        &mut state.monsters,
                        summoner_id,
                        count,
                        state.monster_rng.as_mut(),
                        state.monster_hp_rng.as_mut(),
                    );
                } else if let (Some(ai_rng), Some(hp_rng)) =
                    (state.monster_rng.as_mut(), state.monster_hp_rng.as_mut())
                {
                    apply_gremlin_leader_rally_target(
                        &mut state.monsters,
                        count,
                        ai_rng,
                        hp_rng,
                        ascension,
                    );
                } else {
                    apply_gremlin_leader_rally_representative(&mut state.monsters, count);
                }
                let mut summoner_alive = false;
                if let Some(monster) = state
                    .monsters
                    .iter_mut()
                    .find(|monster| monster.id == summoner_id)
                {
                    monster.moves_executed += 1;
                    summoner_alive = monster.alive;
                }
                if summoner_alive {
                    prepare_next_intent_for_actor(state, actor_id);
                }
                continue;
            }
            crate::MonsterIntent::SummonCollectorTorchHeads { count } => {
                let summoner_id = state.monsters[index].id;
                apply_collector_spawn_torch_heads(
                    &mut state.monsters,
                    count,
                    state.monster_rng.as_mut(),
                    state.monster_hp_rng.as_mut(),
                    ascension,
                );
                if let Some(monster) = state
                    .monsters
                    .iter_mut()
                    .find(|monster| monster.id == summoner_id)
                {
                    monster.moves_executed += 1;
                }
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::Block { block }
                if state.monsters[index].content_id == DECA_ID =>
            {
                apply_deca_square(&mut state.monsters, block, ascension);
                if let Some(monster) = state
                    .monsters
                    .iter_mut()
                    .find(|monster| monster.id == actor_id)
                {
                    monster.moves_executed += 1;
                }
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::Block { block }
                if matches!(
                    state.monsters[index].content_id,
                    CENTURION_ID | GREMLIN_TSUNDERE_ID
                ) =>
            {
                apply_shield_gremlin_random_block(
                    &mut state.monsters,
                    actor_id,
                    block,
                    state.monster_rng.as_mut(),
                );
                if let Some(monster) = state
                    .monsters
                    .iter_mut()
                    .find(|monster| monster.id == actor_id)
                {
                    monster.moves_executed += 1;
                }
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            _ => {}
        }
        let player_snapshot = state.player.clone();
        let intent = state.monsters[index].intent;
        let damage = apply_monster_intent_with_card_rng(
            &mut state.monsters[index],
            &mut state.player,
            &mut state.piles,
            ascension,
            &player_snapshot,
            &relics,
            state.card_random_rng.as_mut(),
        );
        let hits = match state.monsters[index].intent {
            crate::MonsterIntent::AttackMultiple { hits, .. }
            | crate::MonsterIntent::AttackMultipleUpgradeBurns { hits, .. } => hits,
            _ => 1,
        };
        if matches!(intent, crate::MonsterIntent::Ritual { .. }) {
            skip_ritual_tick.push(actor_id);
        }
        let heal_self = matches!(
            state.monsters[index].intent,
            crate::MonsterIntent::AttackHealSelf { .. }
        )
        .then_some(state.monsters[index].id);
        let burn_to_discard_and_draw = match intent {
            crate::MonsterIntent::AddBurnToDiscardAndDraw { count, .. } => count,
            _ => 0,
        };
        if damage > 0 || burn_to_discard_and_draw > 0 {
            let heal_self_thorns = if heal_self.is_some() {
                (state.player.powers.thorns + state.player.temp_thorns) * hits.max(1)
            } else {
                0
            };
            apply_monster_pending_effects(
                state,
                damage,
                hits,
                state.monsters[index].powers.painful_stabs,
                heal_self,
                heal_self_thorns,
                burn_to_discard_and_draw,
            );
        }
        if state.monsters[index].alive
            && state.monsters[index].content_id == NEMESIS_ID
            && state.monsters[index].powers.intangible == 0
        {
            state.monsters[index].powers.intangible = 1;
        }
        if state.monsters[index].alive {
            if state.monsters[index].content_id == LAGAVULIN_ID
                && matches!(intent, crate::MonsterIntent::Sleep)
                && state.monsters[index].sleep_turns_remaining == 0
            {
                state.monsters[index].intent =
                    target_lagavulin_direct_wake_attack_intent(ascension);
                record_target_move(&mut state.monsters[index]);
                continue;
            }
            prepare_next_intent_for_actor(state, actor_id);
            apply_transient_fading_after_turn(&mut state.monsters, actor_id);
        }
        if state.player.hp <= 0 {
            return;
        }
    }

    for monster in &mut state.monsters {
        if monster.alive {
            if monster.powers.vulnerable > 0 {
                monster.powers.vulnerable -= 1;
            }
            if monster.powers.weak > 0 {
                monster.powers.weak -= 1;
            }
            if monster.powers.malleable_base > 0 {
                monster.powers.malleable = monster.powers.malleable_base;
            }
            if skip_ritual_tick.contains(&monster.id) {
                apply_end_of_monster_turn_powers_without_ritual(monster);
            } else {
                apply_end_of_monster_turn_powers(monster);
            }
            if monster.content_id == BYRD_ID && monster.powers.flight > 0 {
                monster.powers.flight = target_byrd_flight_amount(state.ascension);
            }
            if monster.temp_strength_down > 0 {
                monster.powers.strength += monster.temp_strength_down;
                monster.temp_strength_down = 0;
            }
        }
    }

    if state.player.powers.vulnerable > 0 && state.player.vulnerable_just_applied {
        state.player.vulnerable_just_applied = false;
    } else if state.player.powers.vulnerable > 0 {
        state.player.powers.vulnerable -= 1;
    } else {
        state.player.vulnerable_just_applied = false;
    }
    if state.player.powers.intangible > 0 {
        state.player.powers.intangible -= 1;
    }

    apply_turn_transition_block_loss(state);
}

fn apply_monster_pending_effects(
    state: &mut CombatState,
    damage: i32,
    hits: i32,
    painful_stabs: i32,
    heal_self: Option<MonsterId>,
    heal_self_thorns: i32,
    burn_to_discard_and_draw: i32,
) {
    let mut total_hp_damage = 0;
    let hit_count = hits.max(1);
    if damage > 0 && hit_count > 1 {
        let hit_damage = damage / hit_count;
        for _ in 0..hit_count {
            let hp_damage = deal_damage_to_player(state, hit_damage);
            apply_painful_stabs_after_player_damage(state, painful_stabs, hp_damage);
            total_hp_damage += hp_damage;
        }
    } else if damage > 0 {
        let hp_damage = deal_damage_to_player(state, damage);
        apply_painful_stabs_after_player_damage(state, painful_stabs, hp_damage);
        total_hp_damage += hp_damage;
    }
    if state.player.hp <= 0 {
        return;
    }
    apply_attack_heal_self_after_player_damage(state, heal_self, total_hp_damage);
    apply_attack_heal_self_thorns_after_heal(state, heal_self, heal_self_thorns);
    if burn_to_discard_and_draw > 0 {
        add_cards_to_draw_random_spot(
            &mut state.piles,
            BURN_ID,
            burn_to_discard_and_draw,
            state.card_random_rng.as_mut(),
        );
        add_cards_to_discard(&mut state.piles, BURN_ID, burn_to_discard_and_draw);
    }
}

fn apply_turn_transition_block_loss(state: &mut CombatState) {
    if state.player.powers.barricade > 0 {
        return;
    }

    if state.relics.contains(&crate::Relic::Calipers) {
        state.player.block = (state.player.block - crate::relic::CALIPERS_BLOCK_LOSS).max(0);
    } else {
        state.player.block = 0;
    }
}

fn apply_transient_fading_after_turn(monsters: &mut [crate::MonsterState], actor_id: MonsterId) {
    let Some(monster) = monsters
        .iter_mut()
        .find(|monster| monster.id == actor_id && monster.content_id == TRANSIENT_ID)
    else {
        return;
    };
    if monster.moves_executed < 5 {
        return;
    }
    monster.alive = false;
    monster.escaped = true;
    monster.block = 0;
    monster.intent = crate::MonsterIntent::Attack { damage: 0 };
}

fn deal_damage_to_player(state: &mut CombatState, amount: i32) -> i32 {
    let incoming = crate::combat::hp_loss::cap_player_damage_with_intangible(&state.player, amount);
    let blocked = state.player.block.min(incoming);
    state.player.block -= blocked;
    let mitigated =
        crate::relic::mitigate_unblocked_attack_damage(&state.relics, incoming - blocked);
    let hp_damage = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp = (state.player.hp - hp_damage).max(0);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_damage);
    if hp_damage > 0 && state.player.powers.plated_armor > 0 {
        state.player.powers.plated_armor -= 1;
    }
    hp_damage
}

fn apply_painful_stabs_after_player_damage(
    state: &mut CombatState,
    painful_stabs: i32,
    hp_damage: i32,
) {
    if painful_stabs <= 0 || hp_damage <= 0 {
        return;
    }

    for _ in 0..painful_stabs {
        let next_id = crate::CardId::new(state.piles.max_card_instance_id() + 1);
        state
            .piles
            .discard_pile
            .push(crate::CardInstance::new(next_id, WOUND_ID));
    }
}

fn apply_attack_heal_self_after_player_damage(
    state: &mut CombatState,
    monster_id: Option<MonsterId>,
    hp_damage: i32,
) {
    if hp_damage <= 0 {
        return;
    }
    let Some(monster_id) = monster_id else {
        return;
    };
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id && monster.alive)
    {
        heal_monster_to_definition_cap(monster, state.ascension, hp_damage);
    }
}

fn apply_attack_heal_self_thorns_after_heal(
    state: &mut CombatState,
    monster_id: Option<MonsterId>,
    thorns_damage: i32,
) {
    if thorns_damage <= 0 {
        return;
    }
    let Some(monster_id) = monster_id else {
        return;
    };
    if let Some(monster) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id && monster.alive)
    {
        crate::combat::damage::deal_unmodified_damage_to_monster(monster, thorns_damage);
    }
}

fn draw_next_hand_without_shuffle(state: &mut CombatState) {
    if let Some(mut rng) = state.shuffle_rng.take() {
        draw_next_hand_with_sts_rng(state, &mut rng);
        state.shuffle_rng = Some(rng);
    } else {
        draw_next_hand_without_rng(state);
    }
}

fn draw_next_hand_with_sts_rng(state: &mut CombatState, rng: &mut crate::rng::StsRng) {
    for _ in 0..target_hand_size(state) {
        if state.piles.draw_pile.is_empty() && !state.piles.discard_pile.is_empty() {
            shuffle_discard_into_draw_sts(state, rng);
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = state.piles.draw_pile.pop() {
            let content_id = card.content_id;
            let extra_draws = evolve_extra_draw_count(state, content_id);
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id);
            draw_cards_with_sts_rng(state, extra_draws, rng);
        }
    }
}

fn draw_next_hand_without_rng(state: &mut CombatState) {
    for _ in 0..target_hand_size(state) {
        if state.piles.draw_pile.is_empty() && !state.piles.discard_pile.is_empty() {
            let mut rng = SimulatorRng::new(0);
            shuffle_discard_into_draw(state, &mut rng);
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = state.piles.draw_pile.pop() {
            let content_id = card.content_id;
            let extra_draws = evolve_extra_draw_count(state, content_id);
            apply_confusion_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
            apply_fire_breathing_on_draw(state, content_id);
            draw_cards_without_shuffle(state, extra_draws);
        }
    }
}

pub(crate) fn target_hand_size(state: &CombatState) -> usize {
    HAND_SIZE
        + if state.relics.contains(&crate::Relic::SneckoEye) {
            crate::relic::SNECKO_EYE_DRAW
        } else {
            0
        }
}

fn prepare_next_intent_for_actor(state: &mut CombatState, actor_id: MonsterId) {
    prepare_next_intents_for_ids(state, Some(&[actor_id]));
}

fn prepare_next_intents_for_ids(state: &mut CombatState, only_ids: Option<&[MonsterId]>) {
    let living_monster_count = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .count();
    let alive_gremlin_count = gremlin_leader_alive_minion_count(&state.monsters);
    let collector_minion_dead = state
        .monsters
        .iter()
        .any(|monster| monster.powers.minion != 0 && !monster.alive);
    let missing_hp = living_monster_missing_hp(&state.monsters, state.ascension);
    for (monster_index, monster) in state.monsters.iter_mut().enumerate() {
        if only_ids.is_some_and(|ids| !ids.contains(&monster.id)) {
            continue;
        }
        if is_half_dead_darkling(monster) {
            let _ = state.monster_rng.as_mut().map(|rng| rng.random_int(99));
            monster.intent = crate::MonsterIntent::Stun;
            record_target_move(monster);
            continue;
        }

        if monster.alive || only_ids.is_some() {
            if monster.initial_intent_locked {
                monster.initial_intent_locked = false;
                record_target_move(monster);
                continue;
            }
            if monster.split_triggered
                && matches!(monster.intent, crate::MonsterIntent::SummonGremlins { .. })
                && matches!(
                    monster.content_id,
                    ACID_SLIME_ID | SPIKE_SLIME_ID | SLIME_BOSS_ID
                )
            {
                continue;
            }
            if monster.content_id == ACID_SLIME_ID
                && monster.hp <= ACID_SLIME_S_A7_HP_RANGE.max
                && !acid_slime_uses_medium_move_table(monster)
            {
                monster.intent =
                    target_small_acid_slime_followup_intent(monster.intent, state.ascension);
                record_target_move(monster);
                continue;
            }
            if monster.content_id == SPIKE_SLIME_ID
                && monster.hp <= SPIKE_SLIME_S_A7_HP_RANGE.max
                && !spike_slime_uses_medium_or_large_move_table(monster)
            {
                let _ = state.monster_rng.as_mut().map(|rng| rng.random_int(99));
                monster.intent = crate::MonsterIntent::Attack {
                    damage: if state.ascension >= 2 { 6 } else { 5 },
                };
                record_target_move(monster);
                continue;
            }
            if monster.content_id == TORCH_HEAD_ID {
                monster.intent = crate::MonsterIntent::Attack {
                    damage: crate::content::monsters::TORCH_HEAD_ATTACK_DAMAGE,
                };
                record_target_move(monster);
                continue;
            }
            if monster.content_id == TRANSIENT_ID {
                monster.intent = crate::MonsterIntent::Attack {
                    damage: crate::content::monsters::transient_attack_damage(
                        monster.moves_executed,
                        state.ascension,
                    ),
                };
                record_target_move(monster);
                continue;
            }
            if monster.content_id == LOOTER_ID {
                monster.intent = target_looter_direct_next_intent_after_turn(
                    &monster.move_history,
                    monster.moves_executed,
                    state.monster_rng.as_mut(),
                    state.ascension,
                );
                record_target_move(monster);
                continue;
            }
            if monster.content_id == MUGGER_ID {
                monster.intent = target_mugger_direct_next_intent_after_turn(
                    &monster.move_history,
                    monster.moves_executed,
                    state.monster_rng.as_mut(),
                    state.ascension,
                );
                record_target_move(monster);
                continue;
            }
            if monster.content_id == GREMLIN_TSUNDERE_ID {
                let mut source_branch = monster.clone();
                source_branch.moves_executed = if living_monster_count > 1 { 0 } else { 1 };
                monster.intent =
                    prepare_monster_intent_for_ascension(&source_branch, state.ascension);
                record_target_move(monster);
                continue;
            }
            if monster.content_id == GREMLIN_WIZARD_ID {
                monster.intent = target_gremlin_wizard_direct_next_intent_after_turn(
                    monster.moves_executed,
                    state.ascension,
                );
                record_target_move(monster);
                continue;
            }
            if monster.content_id == SLIME_BOSS_ID {
                monster.intent = prepare_monster_intent_for_ascension(monster, state.ascension);
                record_target_move(monster);
                continue;
            }
            let roll = state.monster_rng.as_mut().map(|rng| rng.random_int(99));
            monster.intent = if monster.content_id == HEXAGHOST_ID && monster.moves_executed == 1 {
                crate::MonsterIntent::AttackMultiple {
                    damage: (state.player.hp / 12) + 1,
                    hits: 6,
                }
            } else if monster.content_id == BRONZE_AUTOMATON_ID {
                if roll.is_some() {
                    target_bronze_automaton_next_intent(
                        monster.moves_executed,
                        &monster.move_history,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == EXPLODER_ID {
                if roll.is_some() {
                    target_exploder_next_intent_from_roll(monster.moves_executed, state.ascension)
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SPHERIC_GUARDIAN_ID {
                if roll.is_some() {
                    target_spheric_guardian_next_intent_from_roll(
                        monster.moves_executed,
                        &monster.move_history,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == MAW_ID {
                if let Some(roll) = roll {
                    target_maw_next_intent_from_roll(
                        monster.moves_executed,
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SPIRE_GROWTH_ID {
                if let Some(roll) = roll {
                    target_spire_growth_next_intent_from_roll(
                        monster.moves_executed,
                        &monster.move_history,
                        roll,
                        state.player.powers.constricted > 0,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == GIANT_HEAD_ID {
                if let Some(roll) = roll {
                    target_giant_head_next_intent_from_roll(
                        monster.moves_executed,
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == NEMESIS_ID {
                if let Some(roll) = roll {
                    target_nemesis_next_intent_from_roll(
                        monster.moves_executed,
                        &monster.move_history,
                        roll,
                        state.monster_rng.as_mut(),
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == JAW_WORM_ID {
                if let (Some(roll), Some(rng)) = (roll, state.monster_rng.as_mut()) {
                    target_jaw_worm_next_intent_from_roll(&monster.move_history, roll, rng)
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == RED_LOUSE_ID {
                if let Some(roll) = roll {
                    target_louse_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        monster.rolled_attack_damage,
                        RED_LOUSE_BITE_DAMAGE,
                        crate::MonsterIntent::StrengthAndBlock {
                            strength: LOUSE_CURL_STRENGTH,
                            block: 0,
                        },
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == GREEN_LOUSE_ID {
                if let Some(roll) = roll {
                    target_louse_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        monster.rolled_attack_damage,
                        GREEN_LOUSE_BITE_DAMAGE,
                        crate::MonsterIntent::ApplyPlayerWeak {
                            amount: GREEN_LOUSE_WEAK,
                        },
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == GREMLIN_NOB_ID {
                if let Some(roll) = roll {
                    target_gremlin_nob_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == CHOSEN_ID {
                if let Some(roll) = roll {
                    target_chosen_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == BYRD_ID {
                if let Some(roll) = roll {
                    if let Some(rng) = state.monster_rng.as_mut() {
                        if monster.powers.flight <= 0 {
                            target_grounded_byrd_next_intent()
                        } else {
                            target_byrd_next_intent_from_roll(
                                &monster.move_history,
                                roll,
                                rng,
                                state.ascension,
                            )
                        }
                    } else {
                        prepare_monster_intent_for_ascension(monster, state.ascension)
                    }
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == ACID_SLIME_ID
                && acid_slime_uses_large_move_table(monster)
            {
                if let Some(roll) = roll {
                    if let Some(rng) = state.monster_rng.as_mut() {
                        target_large_acid_slime_next_intent_from_roll(
                            &monster.move_history,
                            roll,
                            rng,
                            state.ascension,
                        )
                    } else {
                        prepare_monster_intent_for_ascension(monster, state.ascension)
                    }
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == ACID_SLIME_ID
                && acid_slime_uses_medium_move_table(monster)
            {
                if let Some(roll) = roll {
                    if let Some(rng) = state.monster_rng.as_mut() {
                        target_medium_acid_slime_next_intent_from_roll(
                            &monster.move_history,
                            roll,
                            rng,
                            state.ascension,
                        )
                    } else {
                        prepare_monster_intent_for_ascension(monster, state.ascension)
                    }
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SPIKE_SLIME_ID
                && spike_slime_uses_medium_or_large_move_table(monster)
            {
                if let Some(roll) = roll {
                    target_medium_or_large_spike_slime_next_intent_from_roll(
                        monster.hp,
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SENTRY_ID {
                if roll.is_some() {
                    target_sentry_next_intent(&monster.move_history, monster_index, state.ascension)
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SHELLED_PARASITE_ID {
                if let Some(roll) = roll {
                    if let Some(rng) = state.monster_rng.as_mut() {
                        target_shelled_parasite_next_intent_from_roll(
                            &monster.move_history,
                            roll,
                            rng,
                            state.ascension,
                        )
                    } else {
                        prepare_monster_intent_for_ascension(monster, state.ascension)
                    }
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SNAKE_PLANT_ID {
                if let Some(roll) = roll {
                    target_snake_plant_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SNECKO_ID {
                if let Some(roll) = roll {
                    target_snecko_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == BOOK_OF_STABBING_ID {
                if let Some(roll) = roll {
                    let mut stab_count = monster.powers.book_stab_count.max(1);
                    let intent = target_book_of_stabbing_next_intent_from_roll_with_stab_count(
                        &monster.move_history,
                        &mut stab_count,
                        roll,
                        state.ascension,
                    );
                    monster.powers.book_stab_count = stab_count;
                    intent
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == CENTURION_ID {
                if let Some(roll) = roll {
                    target_centurion_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        living_monster_count,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == HEALER_ID {
                if let Some(roll) = roll {
                    target_healer_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        missing_hp,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == FUNGI_BEAST_ID {
                if let Some(roll) = roll {
                    target_fungi_beast_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SLAVER_BLUE_ID {
                if let Some(roll) = roll {
                    target_slaver_blue_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SLAVER_RED_ID {
                if let Some(roll) = roll {
                    target_slaver_red_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == GREMLIN_LEADER_ID {
                if let Some(roll) = roll {
                    target_gremlin_leader_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.monster_rng.as_mut().map(|rng| &mut *rng),
                        alive_gremlin_count,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == THE_COLLECTOR_ID {
                if let Some(roll) = roll {
                    target_collector_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        collector_minion_dead,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == BRONZE_ORB_ID {
                if let Some(roll) = roll {
                    target_bronze_orb_next_intent_from_roll(&monster.move_history, roll)
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == ORB_WALKER_ID {
                if let Some(roll) = roll {
                    target_orb_walker_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == REPTOMANCER_ID {
                if let Some(roll) = roll {
                    target_reptomancer_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        living_monster_count.saturating_sub(1) <= 3,
                        state.monster_rng.as_mut().map(|rng| &mut *rng),
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == REPULSOR_ID {
                if let Some(roll) = roll {
                    target_repulsor_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == DARKLING_ID {
                if let Some(roll) = roll {
                    if let Some(rng) = state.monster_rng.as_mut() {
                        crate::content::monsters::target_darkling_next_intent_from_roll_with_rng(
                            &monster.move_history,
                            roll,
                            monster_index,
                            monster.rolled_attack_damage,
                            state.ascension,
                            rng,
                        )
                    } else {
                        crate::content::monsters::target_darkling_next_intent_from_roll(
                            &monster.move_history,
                            roll,
                            monster_index,
                            monster.rolled_attack_damage,
                            state.ascension,
                        )
                    }
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else if monster.content_id == SPIKER_ID {
                if let Some(roll) = roll {
                    target_spiker_next_intent_from_roll(
                        &monster.move_history,
                        monster.powers.spiker_thorns_buffs,
                        roll,
                        state.ascension,
                    )
                } else {
                    prepare_monster_intent_for_ascension(monster, state.ascension)
                }
            } else {
                prepare_monster_intent_for_ascension(monster, state.ascension)
            };
            record_target_move(monster);
        }
    }
}

fn is_half_dead_darkling(monster: &crate::MonsterState) -> bool {
    monster.content_id == DARKLING_ID && !monster.alive && monster.escaped
}

fn acid_slime_uses_medium_move_table(monster: &crate::MonsterState) -> bool {
    acid_slime_uses_large_move_table(monster)
        || monster.hp > ACID_SLIME_S_A7_HP_RANGE.max
        || monster.move_history.contains(&2)
        || matches!(
            monster.intent,
            crate::MonsterIntent::AttackAddSlimedToDiscard { .. }
        )
        || matches!(
            monster.intent,
            crate::MonsterIntent::Attack { damage }
                if damage >= crate::content::monsters::ACID_SLIME_M_NORMAL_TACKLE_DAMAGE
        )
}

fn acid_slime_uses_large_move_table(monster: &crate::MonsterState) -> bool {
    monster.max_hp > ACID_SLIME_M_A7_HP_RANGE.max
        || matches!(
            monster.rolled_attack_damage,
            Some(damage) if damage >= crate::content::monsters::ACID_SLIME_L_NORMAL_TACKLE_DAMAGE
        )
}

fn spike_slime_uses_medium_or_large_move_table(monster: &crate::MonsterState) -> bool {
    monster.hp > SPIKE_SLIME_S_A7_HP_RANGE.max
        || matches!(
            monster.intent,
            crate::MonsterIntent::AttackAddSlimedToDiscard { .. }
                | crate::MonsterIntent::ApplyPlayerFrailAndWeak { .. }
        )
}

fn apply_shield_gremlin_random_block(
    monsters: &mut [crate::MonsterState],
    source_id: MonsterId,
    block: i32,
    rng: Option<&mut StsRng>,
) {
    let candidates = monsters
        .iter()
        .enumerate()
        .filter_map(|(index, monster)| {
            (monster.id != source_id
                && monster.alive
                && !matches!(monster.intent, crate::MonsterIntent::Escape))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let target_index = if candidates.is_empty() {
        monsters.iter().position(|monster| monster.id == source_id)
    } else if let Some(rng) = rng {
        Some(candidates[rng.random_int(candidates.len() as i32 - 1) as usize])
    } else {
        candidates.first().copied()
    };
    if let Some(target_index) = target_index {
        monsters[target_index].block += block;
    }
}

fn apply_deca_square(monsters: &mut [crate::MonsterState], block: i32, ascension: u8) {
    for monster in monsters.iter_mut().filter(|monster| monster.alive) {
        monster.block += block;
        if ascension >= 19 {
            monster.powers.plated_armor += 3;
        }
    }
}

fn gremlin_leader_alive_minion_count(monsters: &[crate::MonsterState]) -> usize {
    monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && crate::content::monsters::is_gremlin_leader_minion_content_id(monster.content_id)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CardId;
    use crate::content::cards::STRIKE_R_ID;
    use crate::content::monsters::{
        donu_deca_boss_monsters_for_ascension, monster_state_for_ascension,
        target_giant_head_next_intent_from_roll,
        target_gremlin_wizard_direct_next_intent_after_turn,
        target_looter_direct_next_intent_after_turn, target_nemesis_next_intent_from_roll,
        target_spheric_guardian_next_intent_from_roll, target_spire_growth_next_intent_from_roll,
        transient_attack_damage, BOOK_OF_STABBING_A0, BRONZE_AUTOMATON_A0, BYRD_A0, CENTURION_A0,
        DAGGER_A0, DAGGER_ID, DARKLING_A0, EXPLODER_A0, GIANT_HEAD_A0, GIANT_HEAD_ID,
        GREMLIN_NOB_A0, GREMLIN_TSUNDERE_A0, GREMLIN_WIZARD_A0, HEALER_A0, LAGAVULIN_A0, LOOTER_A0,
        LOOTER_ID, MAW_A0, MAW_ID, MUGGER_A0, MUGGER_ID, NEMESIS_A0, NEMESIS_ID, SENTRY_A0,
        SPHERIC_GUARDIAN_A0, SPHERIC_GUARDIAN_ID, SPIRE_GROWTH_A0, SPIRE_GROWTH_ID, TRANSIENT_A0,
    };

    #[test]
    fn magnetism_generated_card_overflows_full_hand_to_discard() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.magnetism = 1;
        state.piles.hand = (1..=10)
            .map(|id| crate::CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();
        state.piles.discard_pile.clear();
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            MonsterId::new(1),
            state.ascension,
        )];

        apply_start_of_turn_magnetism(&mut state);

        assert_eq!(state.piles.hand.len(), 10);
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert!(state.piles.discard_pile[0].combat_only);
    }

    #[test]
    fn transient_direct_set_move_does_not_consume_ai_rng_after_turn() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 4;
        state.monsters = vec![monster_state_for_ascension(
            &TRANSIENT_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1];
        state.monster_rng = Some(StsRng::new(123));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack {
                damage: transient_attack_damage(2, 4)
            }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            0
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 1, 1]);
    }

    #[test]
    fn start_player_turn_clears_unused_double_tap() {
        let mut state = CombatState::initial_fixture();
        state.double_tap_pending = 1;

        start_player_turn(&mut state);

        assert_eq!(state.double_tap_pending, 0);
    }

    #[test]
    fn deca_square_blocks_all_living_monsters_and_adds_a19_plated_armor() {
        let deca_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 19;
        state.monsters = donu_deca_boss_monsters_for_ascension(state.ascension);
        state.monsters[0].moves_executed = 1;
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 16 };
        state.monster_rng = Some(StsRng::new(123));

        run_monster_turn(&mut state);

        let deca = state
            .monsters
            .iter()
            .find(|monster| monster.id == deca_id)
            .expect("Deca remains present");
        let donu = state
            .monsters
            .iter()
            .find(|monster| monster.id == MonsterId::new(2))
            .expect("Donu remains present");
        assert_eq!(deca.block, 19);
        assert_eq!(donu.block, 19);
        assert_eq!(deca.powers.plated_armor, 3);
        assert_eq!(donu.powers.plated_armor, 3);
        assert_eq!(deca.moves_executed, 2);
        assert_eq!(
            deca.intent,
            crate::MonsterIntent::AttackMultiple {
                damage: 12,
                hits: 2
            }
        );
    }

    #[test]
    fn bronze_automaton_turn_prep_uses_source_post_beam_a19_boost() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 19;
        state.monsters = vec![monster_state_for_ascension(
            &BRONZE_AUTOMATON_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 6;
        state.monsters[0].move_history = vec![4, 1, 5, 1, 5, 2];
        state.monster_rng = Some(StsRng::new(4444));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthAndBlock {
                strength: 4,
                block: 12,
            }
        );
        assert_eq!(state.monsters[0].move_history.last().copied(), Some(5));
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn book_of_stabbing_turn_prep_uses_stored_stab_count() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &BOOK_OF_STABBING_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].move_history = vec![2];
        state.monsters[0].powers.book_stab_count = 4;
        state.monster_rng = Some(StsRng::new(9));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackMultiple { damage: 7, hits: 5 }
        );
        assert_eq!(state.monsters[0].powers.book_stab_count, 5);
        assert_eq!(state.monsters[0].move_history.last().copied(), Some(1));
    }

    #[test]
    fn looter_direct_set_move_consumes_speech_bool_without_roll_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&LOOTER_A0, actor_id, 0)];
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![1];
        state.monster_rng = Some(StsRng::new(123));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackStealGold {
                damage: 10,
                amount: 15
            }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 1]);
    }

    #[test]
    fn looter_second_mug_uses_source_half_chance_without_roll_move() {
        let mut expected_rng = StsRng::new(456);
        let expected =
            target_looter_direct_next_intent_after_turn(&[1, 1], 2, Some(&mut expected_rng), 0);
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&LOOTER_A0, actor_id, 0)];
        state.monsters[0].content_id = LOOTER_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1];
        state.monster_rng = Some(StsRng::new(456));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(state.monsters[0].intent, expected);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            expected_rng.counter()
        );
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            crate::content::monsters::target_move_byte(LOOTER_ID, expected)
        );
    }

    #[test]
    fn mugger_direct_set_move_consumes_attack_voice_roll_without_roll_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&MUGGER_A0, actor_id, 0)];
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![1];
        state.monster_rng = Some(StsRng::new(789));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackStealGold {
                damage: 10,
                amount: 15
            }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 1]);
    }

    #[test]
    fn mugger_second_mug_consumes_voice_talk_and_half_chance_without_roll_move() {
        let mut expected_rng = StsRng::new(987);
        let expected = crate::content::monsters::target_mugger_direct_next_intent_after_turn(
            &[1, 1],
            2,
            Some(&mut expected_rng),
            17,
        );
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &MUGGER_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1];
        state.monster_rng = Some(StsRng::new(987));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(state.monsters[0].intent, expected);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            expected_rng.counter()
        );
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            crate::content::monsters::target_move_byte(MUGGER_ID, expected)
        );
    }

    #[test]
    fn gremlin_wizard_direct_cycle_does_not_consume_ai_rng_after_turn() {
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(1, 0),
            crate::MonsterIntent::Block { block: 0 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(2, 0),
            crate::MonsterIntent::Attack { damage: 25 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(3, 0),
            crate::MonsterIntent::Block { block: 0 }
        );
        assert_eq!(
            target_gremlin_wizard_direct_next_intent_after_turn(3, 17),
            crate::MonsterIntent::Attack { damage: 30 }
        );

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&GREMLIN_WIZARD_A0, actor_id, 0)];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 2];
        state.monster_rng = Some(StsRng::new(246));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 25 }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            0
        );
        assert_eq!(state.monsters[0].move_history, vec![2, 2, 1]);
    }

    #[test]
    fn gremlin_tsundere_protect_uses_ai_rng_for_target_but_direct_sets_next_move() {
        let actor_id = MonsterId::new(1);
        let target_id = MonsterId::new(2);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![
            monster_state_for_ascension(&GREMLIN_TSUNDERE_A0, actor_id, 0),
            monster_state_for_ascension(&LOOTER_A0, target_id, 0),
        ];
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 7 };
        state.monsters[0].move_history = vec![1];
        state.monster_rng = Some(StsRng::new(246));

        run_monster_turn(&mut state);

        assert_eq!(state.monsters[1].block, 7);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Block { block: 7 }
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 1]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn centurion_protect_uses_ai_rng_for_ally_block_before_roll_move() {
        let actor_id = MonsterId::new(1);
        let target_id = MonsterId::new(2);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![
            monster_state_for_ascension(&CENTURION_A0, actor_id, state.ascension),
            monster_state_for_ascension(&HEALER_A0, target_id, state.ascension),
        ];
        state.monsters[0].intent = crate::MonsterIntent::Block { block: 20 };
        state.monsters[0].move_history = vec![1, 1];
        state.monster_rng = Some(StsRng::new(2468));

        run_monster_turn(&mut state);

        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(state.monsters[1].block, 20);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(state.monsters[0].move_history.last().copied(), Some(2));
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            3
        );
    }

    #[test]
    fn sentry_turn_prep_ignores_roll_value_and_alternates_from_last_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 3;
        state.monsters = vec![monster_state_for_ascension(
            &SENTRY_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![4];
        state.monster_rng = Some(StsRng::new(123));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AddDazedToDiscard { count: 2 }
        );
        assert_eq!(state.monsters[0].move_history, vec![4, 3]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn grounded_byrd_turn_prep_uses_headbutt_without_replacement_draw() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &BYRD_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].powers.flight = 0;
        state.monsters[0].move_history = vec![4];
        state.monster_rng = Some(StsRng::new(123));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(state.monsters[0].intent, target_grounded_byrd_next_intent());
        assert_eq!(state.monsters[0].move_history, vec![4, 5]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn byrd_headbutt_direct_sets_go_airborne_without_ai_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &BYRD_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].powers.flight = 0;
        state.monsters[0].intent = target_grounded_byrd_next_intent();
        state.monsters[0].move_history = vec![4];
        state.monster_rng = Some(StsRng::new(456));
        let player_hp = state.player.hp;

        run_monster_turn(&mut state);

        assert_eq!(state.player.hp, player_hp - 3);
        assert_eq!(state.monsters[0].intent, target_byrd_go_airborne_intent());
        assert_eq!(
            crate::content::monsters::target_move_byte(BYRD_ID, state.monsters[0].intent),
            Some(2)
        );
        assert_eq!(state.monsters[0].move_history, vec![4, 5, 2]);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            0
        );
    }

    #[test]
    fn half_dead_darkling_count_sets_reincarnate_after_one_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &DARKLING_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].alive = false;
        state.monsters[0].escaped = true;
        state.monsters[0].hp = 0;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 0 };
        state.monsters[0].move_history = vec![4];
        state.monster_rng = Some(StsRng::new(111));

        run_monster_turn(&mut state);

        assert!(!state.monsters[0].alive);
        assert!(state.monsters[0].escaped);
        assert_eq!(state.monsters[0].intent, crate::MonsterIntent::Stun);
        assert_eq!(state.monsters[0].move_history, vec![4, 5]);
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn half_dead_darkling_reincarnates_then_rolls_next_move() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &DARKLING_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].alive = false;
        state.monsters[0].escaped = true;
        state.monsters[0].hp = 0;
        state.monsters[0].max_hp = 58;
        state.monsters[0].rolled_attack_damage = Some(11);
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.monsters[0].move_history = vec![4, 5];
        state.monster_rng = Some(StsRng::new(222));
        let mut expected_rng = StsRng::new(222);
        let roll = expected_rng.random_int(99);
        let expected_intent =
            crate::content::monsters::target_darkling_next_intent_from_roll_with_rng(
                &[4, 5],
                roll,
                0,
                Some(11),
                state.ascension,
                &mut expected_rng,
            );
        let expected_move =
            crate::content::monsters::target_move_byte(DARKLING_ID, expected_intent);

        run_monster_turn(&mut state);

        assert!(state.monsters[0].alive);
        assert!(!state.monsters[0].escaped);
        assert_eq!(state.monsters[0].hp, 29);
        assert_eq!(state.monsters[0].intent, expected_intent);
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            expected_move
        );
        assert_eq!(state.monsters[0].moves_executed, 1);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            expected_rng.counter()
        );
    }

    #[test]
    fn lagavulin_natural_wake_direct_sets_attack_without_extra_ai_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 3;
        state.monsters = vec![monster_state_for_ascension(
            &LAGAVULIN_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].sleep_turns_remaining = 1;
        state.monsters[0].intent = crate::MonsterIntent::Sleep;
        state.monsters[0].move_history = vec![5, 5];
        state.monster_rng = Some(StsRng::new(123));

        run_monster_turn(&mut state);

        assert_eq!(state.monsters[0].sleep_turns_remaining, 0);
        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 20 }
        );
        assert_eq!(state.monsters[0].move_history, vec![5, 5, 3]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            0
        );
    }

    #[test]
    fn lagavulin_damage_wake_stun_consumes_roll_move_before_attack() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 3;
        state.monsters = vec![monster_state_for_ascension(
            &LAGAVULIN_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].sleep_turns_remaining = 0;
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.monsters[0].move_history = vec![5, 4];
        state.monster_rng = Some(StsRng::new(456));

        run_monster_turn(&mut state);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 20 }
        );
        assert_eq!(state.monsters[0].move_history, vec![5, 4, 3]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn gremlin_nob_turn_prep_uses_a18_history_guard_after_roll_action() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &GREMLIN_NOB_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![3, 2];
        state.monster_rng = Some(StsRng::new(123));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 16 }
        );
        assert_eq!(state.monsters[0].move_history, vec![3, 2, 1]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
    }

    #[test]
    fn spheric_guardian_uses_source_roll_table_and_move_bytes() {
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(0, &[], 17),
            crate::MonsterIntent::Block { block: 35 }
        );
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(1, &[2], 0),
            crate::MonsterIntent::AttackApplyPlayerFrail {
                damage: 10,
                frail: 5
            }
        );
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(2, &[2, 4], 2),
            crate::MonsterIntent::AttackMultiple {
                damage: 11,
                hits: 2
            }
        );
        assert_eq!(
            target_spheric_guardian_next_intent_from_roll(3, &[2, 4, 1], 2),
            crate::MonsterIntent::AttackAndBlock {
                damage: 11,
                block: 15
            }
        );

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 2;
        state.monsters = vec![monster_state_for_ascension(
            &SPHERIC_GUARDIAN_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = SPHERIC_GUARDIAN_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 4];
        state.monster_rng = Some(StsRng::new(246));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::AttackMultiple {
                damage: 11,
                hits: 2
            }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
        assert_eq!(state.monsters[0].move_history, vec![2, 4, 1]);
    }

    #[test]
    fn maw_uses_source_turn_count_roll_table_and_move_bytes() {
        assert_eq!(
            target_maw_next_intent_from_roll(0, &[], 99, 17),
            crate::MonsterIntent::ApplyPlayerFrailAndWeak { frail: 5, weak: 5 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(1, &[2], 49, 0),
            crate::MonsterIntent::Attack { damage: 5 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(2, &[2, 5], 0, 0),
            crate::MonsterIntent::StrengthSelf { amount: 3 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(3, &[2, 5, 4], 99, 2),
            crate::MonsterIntent::Attack { damage: 30 }
        );
        assert_eq!(
            target_maw_next_intent_from_roll(4, &[2, 5, 4, 3], 0, 17),
            crate::MonsterIntent::AttackMultiple { damage: 5, hits: 3 }
        );

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &MAW_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = MAW_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 5];
        state.monster_rng = Some(StsRng::new(135));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::StrengthSelf { amount: 5 }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
        assert_eq!(state.monsters[0].move_history, vec![2, 5, 4]);
    }

    #[test]
    fn spire_growth_uses_source_constrict_roll_table_and_hp() {
        assert_eq!(
            target_spire_growth_next_intent_from_roll(0, &[], 99, false, 17),
            crate::MonsterIntent::ApplyPlayerConstricted { amount: 12 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(0, &[], 49, false, 0),
            crate::MonsterIntent::Attack { damage: 16 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(1, &[1], 99, false, 0),
            crate::MonsterIntent::ApplyPlayerConstricted { amount: 10 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(2, &[1, 2], 99, true, 2),
            crate::MonsterIntent::Attack { damage: 25 }
        );
        assert_eq!(
            target_spire_growth_next_intent_from_roll(4, &[1, 2, 3, 3], 99, true, 2),
            crate::MonsterIntent::Attack { damage: 18 }
        );

        let mut source_monster =
            monster_state_for_ascension(&SPIRE_GROWTH_A0, MonsterId::new(1), 17);
        assert_eq!((source_monster.hp, source_monster.max_hp), (190, 190));
        source_monster.intent = crate::MonsterIntent::ApplyPlayerConstricted { amount: 12 };
        let fixture = CombatState::initial_fixture();
        let mut player = fixture.player;
        let before = player.clone();
        let mut piles = fixture.piles;
        let damage = crate::content::monsters::apply_monster_intent(
            &mut source_monster,
            &mut player,
            &mut piles,
            17,
            &before,
            &[],
        );
        assert_eq!(damage, 0);
        assert_eq!(player.powers.constricted, 12);

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 17;
        state.monsters = vec![monster_state_for_ascension(
            &SPIRE_GROWTH_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = SPIRE_GROWTH_ID;
        state.monsters[0].moves_executed = 1;
        state.monsters[0].move_history = vec![1];
        state.player.powers.constricted = 0;
        state.monster_rng = Some(StsRng::new(2468));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::ApplyPlayerConstricted { amount: 12 }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 2]);
    }

    #[test]
    fn giant_head_uses_source_countdown_roll_table_hp_and_slow_setup() {
        assert_eq!(
            target_giant_head_next_intent_from_roll(0, &[], 49, 0),
            crate::MonsterIntent::ApplyPlayerWeak { amount: 1 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(0, &[], 50, 0),
            crate::MonsterIntent::Attack { damage: 13 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(2, &[1, 1], 0, 0),
            crate::MonsterIntent::Attack { damage: 13 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(4, &[1, 3, 1, 3], 0, 0),
            crate::MonsterIntent::Attack { damage: 30 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(3, &[1, 3, 1], 0, 18),
            crate::MonsterIntent::Attack { damage: 40 }
        );
        assert_eq!(
            target_giant_head_next_intent_from_roll(10, &[2, 2, 2], 0, 18),
            crate::MonsterIntent::Attack { damage: 70 }
        );

        let source_monster = monster_state_for_ascension(&GIANT_HEAD_A0, MonsterId::new(1), 18);
        assert_eq!((source_monster.hp, source_monster.max_hp), (520, 520));
        assert_eq!(source_monster.powers.slow, 1);

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &GIANT_HEAD_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = GIANT_HEAD_ID;
        state.monsters[0].moves_executed = 3;
        state.monsters[0].move_history = vec![1, 3, 1];
        state.monster_rng = Some(StsRng::new(97531));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(
            state.monsters[0].intent,
            crate::MonsterIntent::Attack { damage: 40 }
        );
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            1
        );
        assert_eq!(state.monsters[0].move_history, vec![1, 3, 1, 2]);
    }

    #[test]
    fn nemesis_uses_source_replacement_booleans_burns_hp_and_intangible() {
        assert_eq!(
            target_nemesis_next_intent_from_roll(0, &[], 49, None, 3),
            crate::MonsterIntent::AttackMultiple { damage: 7, hits: 3 }
        );
        assert_eq!(
            target_nemesis_next_intent_from_roll(0, &[], 50, None, 18),
            crate::MonsterIntent::AddBurnToDiscard {
                count: 5,
                damage: 0
            }
        );
        assert_eq!(
            target_nemesis_next_intent_from_roll(1, &[2], 29, None, 0),
            crate::MonsterIntent::Attack { damage: 45 }
        );

        let mut expected_rng = StsRng::new(4242);
        let expected =
            target_nemesis_next_intent_from_roll(2, &[2, 3], 20, Some(&mut expected_rng), 18);
        assert_eq!(expected_rng.counter(), 1);
        assert!(matches!(
            expected,
            crate::MonsterIntent::AttackMultiple { .. }
                | crate::MonsterIntent::AddBurnToDiscard { .. }
        ));

        let source_monster = monster_state_for_ascension(&NEMESIS_A0, MonsterId::new(1), 18);
        assert_eq!((source_monster.hp, source_monster.max_hp), (200, 200));

        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.ascension = 18;
        state.monsters = vec![monster_state_for_ascension(
            &NEMESIS_A0,
            actor_id,
            state.ascension,
        )];
        state.monsters[0].content_id = NEMESIS_ID;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![2, 3];
        state.monster_rng = Some(StsRng::new(4242));

        prepare_next_intent_for_actor(&mut state, actor_id);

        assert_eq!(state.monsters[0].intent, expected);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            expected_rng.counter() + 1
        );
        assert_eq!(
            state.monsters[0].move_history.last().copied(),
            crate::content::monsters::target_move_byte(NEMESIS_ID, expected)
        );

        state.monsters[0].intent = crate::MonsterIntent::AddBurnToDiscard {
            count: 5,
            damage: 0,
        };
        state.monsters[0].moves_executed = 0;
        state.monsters[0].move_history.clear();
        run_monster_turn(&mut state);

        assert_eq!(state.monsters[0].powers.intangible, 1);
        let hp_before = state.monsters[0].hp;
        let hp_damage =
            crate::combat::damage::deal_unmodified_damage_to_monster(&mut state.monsters[0], 99);
        assert_eq!(hp_damage, 1);
        assert_eq!(state.monsters[0].hp, hp_before - 1);
    }

    #[test]
    fn dagger_explode_attacks_then_loses_all_hp_without_next_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&DAGGER_A0, actor_id, 0)];
        state.monsters[0].content_id = DAGGER_ID;
        state.monsters[0].hp = 20;
        state.monsters[0].max_hp = 20;
        state.monsters[0].intent = crate::MonsterIntent::Attack { damage: 25 };
        state.monsters[0].move_history = vec![1, 2];
        state.monster_rng = Some(StsRng::new(11));
        let player_hp = state.player.hp;

        run_monster_turn(&mut state);

        assert_eq!(state.player.hp, player_hp - 25);
        assert_eq!(state.monsters[0].hp, 0);
        assert!(!state.monsters[0].alive);
        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(state.monsters[0].move_history, vec![1, 2]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            0
        );
    }

    #[test]
    fn exploder_unknown_move_deals_explosive_damage_and_dies_without_next_roll() {
        let actor_id = MonsterId::new(1);
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state_for_ascension(&EXPLODER_A0, actor_id, 0)];
        state.monsters[0].intent = crate::MonsterIntent::Stun;
        state.monsters[0].moves_executed = 2;
        state.monsters[0].move_history = vec![1, 1, 2];
        state.monster_rng = Some(StsRng::new(12));
        let player_hp = state.player.hp;

        run_monster_turn(&mut state);

        assert_eq!(state.player.hp, player_hp - 3);
        assert_eq!(state.monsters[0].hp, 0);
        assert!(!state.monsters[0].alive);
        assert_eq!(state.monsters[0].block, 0);
        assert_eq!(state.monsters[0].powers.explosive, 0);
        assert_eq!(state.monsters[0].move_history, vec![1, 1, 2]);
        assert_eq!(
            state
                .monster_rng
                .as_ref()
                .expect("test installs monster rng")
                .counter(),
            0
        );
    }
}
