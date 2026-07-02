use crate::{
    combat::turn_powers::{
        apply_end_of_monster_turn_powers, apply_end_of_monster_turn_powers_without_ritual,
        apply_end_of_player_turn_powers,
    },
    combat::{
        draw::{
            apply_confusion_cost_randomization, apply_fire_breathing_on_draw,
            draw_cards_with_sts_rng, draw_cards_without_shuffle, evolve_extra_draw_count,
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
        apply_large_spike_slime_split, apply_monster_intent_with_card_rng, apply_slime_boss_split,
        apply_strength_all_monsters, clear_lagavulin_metallicize_if_awake,
        heal_monster_to_definition_cap, living_monster_missing_hp,
        prepare_monster_intent_for_ascension, record_target_move,
        target_bronze_orb_next_intent_from_roll, target_byrd_flight_amount,
        target_byrd_next_intent_from_roll, target_centurion_next_intent_from_roll,
        target_chosen_next_intent_from_roll, target_collector_next_intent_from_roll,
        target_fungi_beast_next_intent_from_roll, target_gremlin_leader_next_intent_from_roll,
        target_gremlin_nob_next_intent_from_roll, target_healer_next_intent_from_roll,
        target_jaw_worm_next_intent_from_roll, target_large_acid_slime_next_intent_from_roll,
        target_looter_next_intent_from_roll, target_louse_next_intent_from_roll,
        target_medium_acid_slime_next_intent_from_roll,
        target_medium_or_large_spike_slime_next_intent_from_roll,
        target_shelled_parasite_next_intent_from_roll, target_slaver_blue_next_intent_from_roll,
        target_slaver_red_next_intent_from_roll, target_small_acid_slime_followup_intent,
        target_snake_plant_next_intent_from_roll, target_snecko_next_intent_from_roll,
        target_spiker_next_intent_from_roll, ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE,
        ACID_SLIME_S_A7_HP_RANGE, BRONZE_AUTOMATON_ID, BRONZE_ORB_ID, BYRD_ID, CENTURION_ID,
        CHOSEN_ID, DARKLING_ID, FUNGI_BEAST_ID, GREEN_LOUSE_BITE_DAMAGE, GREEN_LOUSE_ID,
        GREEN_LOUSE_WEAK, GREMLIN_LEADER_ID, GREMLIN_NOB_ID, GREMLIN_TSUNDERE_ID, HEALER_ID,
        HEXAGHOST_ID, JAW_WORM_ID, LOOTER_ID, LOUSE_CURL_STRENGTH, RED_LOUSE_BITE_DAMAGE,
        RED_LOUSE_ID, SHELLED_PARASITE_ID, SLAVER_BLUE_ID, SLAVER_RED_ID, SLIME_BOSS_ID,
        SNAKE_PLANT_ID, SNECKO_ID, SPHERIC_GUARDIAN_ID, SPIKER_ID, SPIKE_SLIME_ID,
        SPIKE_SLIME_S_A7_HP_RANGE, THE_COLLECTOR_ID, TORCH_HEAD_ID, TRANSIENT_ID,
    },
    ids::MonsterId,
    rng::{JavaRng, StsRng},
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
    let no_rng_discard_len_before_end_turn = if state.shuffle_rng.is_none() {
        Some(state.piles.discard_pile.len())
    } else {
        None
    };

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
    apply_pending_player_spikes_damage(&mut next);
    if next.player.hp <= 0 {
        next.phase = CombatPhase::Lost;
        return next;
    }
    clear_living_monster_block(&mut next);
    next.phase = CombatPhase::MonsterTurn;
    run_monster_turn(&mut next);

    if next.player.hp <= 0 {
        next.player.hp = 0;
        next.phase = CombatPhase::Lost;
        return next;
    }
    if finish_combat_if_over(&mut next, started_with_living_monster) {
        return next;
    }

    start_player_turn_with_no_rng_discard_limit(&mut next, no_rng_discard_len_before_end_turn);
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
    start_player_turn_with_no_rng_discard_limit(state, None);
}

fn start_player_turn_with_no_rng_discard_limit(
    state: &mut CombatState,
    no_rng_discard_len_before_end_turn: Option<usize>,
) {
    crate::relic::reset_turn_relic_counters(state);
    reset_turn_only_temp_costs(state);
    if !crate::relic::preserves_energy_between_turns(&state.relics) {
        state.player.energy = state.player.max_energy;
    }
    state.player.cannot_draw = false;
    state.player.temp_strength = 0;
    state.player.temp_thorns = 0;
    state.player.temp_rage_block = 0;
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
    draw_next_hand_without_shuffle(state, no_rng_discard_len_before_end_turn);
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
        state.piles.hand.push(crate::CardInstance {
            combat_only: true,
            ..crate::CardInstance::new(next_id, content_id)
        });
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
        if !state.monsters[index].alive {
            continue;
        }
        clear_lagavulin_metallicize_if_awake(&mut state.monsters[index]);
        match state.monsters[index].intent {
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
                apply_gremlin_leader_encourage(&mut state.monsters, leader_id, strength, block);
                state.monsters[index].moves_executed += 1;
                prepare_next_intent_for_actor(state, actor_id);
                continue;
            }
            crate::MonsterIntent::SummonGremlins { count } => {
                let summoner_id = state.monsters[index].id;
                if state.monsters[index].content_id == BRONZE_AUTOMATON_ID {
                    apply_bronze_automaton_orb_spawn(&mut state.monsters, summoner_id);
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
                    apply_slime_boss_split(&mut state.monsters, summoner_id, ascension);
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
                if state.monsters[index].content_id == GREMLIN_TSUNDERE_ID =>
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
        prepare_next_intent_for_actor(state, actor_id);
        apply_transient_fading_after_turn(&mut state.monsters, actor_id);
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

fn draw_next_hand_without_shuffle(
    state: &mut CombatState,
    no_rng_discard_len_before_end_turn: Option<usize>,
) {
    if let Some(mut rng) = state.shuffle_rng.take() {
        draw_next_hand_with_sts_rng(state, &mut rng);
        state.shuffle_rng = Some(rng);
    } else {
        draw_next_hand_without_rng(state, no_rng_discard_len_before_end_turn);
    }
}

fn draw_next_hand_with_sts_rng(state: &mut CombatState, rng: &mut crate::rng::StsRng) {
    for _ in 0..target_hand_size(state) {
        if state.piles.draw_pile.is_empty() && !state.piles.discard_pile.is_empty() {
            state.piles.draw_pile.append(&mut state.piles.discard_pile);
            let shuffle_seed = rng.random_long();
            JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
            crate::relic::apply_shuffle_relics(state);
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

fn draw_next_hand_without_rng(
    state: &mut CombatState,
    no_rng_discard_len_before_end_turn: Option<usize>,
) {
    let mut no_rng_discard_remaining = no_rng_discard_len_before_end_turn;
    for _ in 0..target_hand_size(state) {
        if state.piles.draw_pile.is_empty() && !state.piles.discard_pile.is_empty() {
            if let Some(limit) = no_rng_discard_remaining {
                if limit == 0 {
                    break;
                }
                let available = limit.min(state.piles.discard_pile.len());
                state.piles.draw_pile = state.piles.discard_pile.drain(..available).collect();
                no_rng_discard_remaining = Some(limit - available);
                crate::relic::apply_shuffle_relics(state);
            } else {
                break;
            }
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

fn target_hand_size(state: &CombatState) -> usize {
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
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
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
            let roll = state.monster_rng.as_mut().map(|rng| rng.random_int(99));
            monster.intent = if monster.content_id == HEXAGHOST_ID && monster.moves_executed == 1 {
                crate::MonsterIntent::AttackMultiple {
                    damage: (state.player.hp / 12) + 1,
                    hits: 6,
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
            } else if monster.content_id == LOOTER_ID {
                if let Some(roll) = roll {
                    target_looter_next_intent_from_roll(
                        &monster.move_history,
                        roll,
                        state.ascension,
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
                        target_byrd_next_intent_from_roll(
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
                && monster.hp > ACID_SLIME_M_A7_HP_RANGE.max
            {
                if let Some(roll) = roll {
                    if let Some(rng) = state.monster_rng.as_mut() {
                        target_large_acid_slime_next_intent_from_roll(
                            monster.intent,
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
            } else if monster.content_id == GREMLIN_TSUNDERE_ID {
                let mut source_branch = monster.clone();
                source_branch.moves_executed = if living_monster_count > 1 { 0 } else { 1 };
                prepare_monster_intent_for_ascension(&source_branch, state.ascension)
            } else if monster.content_id == BRONZE_ORB_ID {
                if let Some(roll) = roll {
                    target_bronze_orb_next_intent_from_roll(&monster.move_history, roll)
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
    monster.hp > ACID_SLIME_S_A7_HP_RANGE.max
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

fn gremlin_leader_alive_minion_count(monsters: &[crate::MonsterState]) -> usize {
    monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && crate::content::monsters::is_gremlin_leader_minion_content_id(monster.content_id)
        })
        .count()
}
