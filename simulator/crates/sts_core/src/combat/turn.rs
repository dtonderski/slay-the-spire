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
        apply_bronze_automaton_orb_spawn, apply_gremlin_leader_encourage,
        apply_gremlin_leader_rally_representative, apply_gremlin_leader_rally_target,
        apply_heal_all_monsters, apply_large_acid_slime_split, apply_monster_intent_with_card_rng,
        apply_strength_all_monsters, clear_lagavulin_metallicize_if_awake,
        heal_monster_to_definition_cap, living_monster_missing_hp,
        prepare_monster_intent_for_ascension, record_target_move,
        target_bronze_orb_next_intent_from_roll, target_centurion_next_intent_from_roll,
        target_chosen_next_intent_from_roll, target_fungi_beast_next_intent_from_roll,
        target_gremlin_leader_next_intent_from_roll, target_healer_next_intent_from_roll,
        target_jaw_worm_next_intent_from_roll, target_large_acid_slime_next_intent_from_roll,
        target_louse_next_intent_from_roll, target_medium_acid_slime_next_intent_from_roll,
        target_shelled_parasite_next_intent_from_roll, target_snake_plant_next_intent_from_roll,
        ACID_SLIME_ID, ACID_SLIME_M_A7_HP_RANGE, ACID_SLIME_S_A7_HP_RANGE, BRONZE_AUTOMATON_ID,
        BRONZE_ORB_ID, CENTURION_ID, CHOSEN_ID, DARKLING_ID, FUNGI_BEAST_ID,
        GREEN_LOUSE_BITE_DAMAGE, GREEN_LOUSE_ID, GREEN_LOUSE_WEAK, GREMLIN_LEADER_ID, HEALER_ID,
        HEXAGHOST_ID, JAW_WORM_ID, LOUSE_CURL_STRENGTH, RED_LOUSE_BITE_DAMAGE, RED_LOUSE_ID,
        SHELLED_PARASITE_ID, SNAKE_PLANT_ID, SPHERIC_GUARDIAN_ID,
    },
    ids::MonsterId,
    rng::JavaRng,
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
    clear_living_monster_block(&mut next);
    next.phase = CombatPhase::MonsterTurn;
    run_monster_turn(&mut next);

    if next.player.hp <= 0 {
        next.phase = CombatPhase::Lost;
        return next;
    }

    start_player_turn_with_no_rng_discard_limit(&mut next, no_rng_discard_len_before_end_turn);
    next
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
        state.phase = CombatPhase::Won;
        return;
    }
    prepare_next_intents(state);
    state.phase = CombatPhase::WaitingForPlayer;
}

fn apply_start_of_turn_brutality(state: &mut CombatState) {
    for _ in 0..state.player.powers.brutality.max(0) {
        let mitigated = crate::relic::mitigate_hp_loss(&state.relics, 1);
        let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
        state.player.hp -= hp_loss;
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
    let mut pending_damage = Vec::new();
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
                continue;
            }
            crate::MonsterIntent::StrengthAllMonsters { amount } => {
                apply_strength_all_monsters(&mut state.monsters, amount);
                state.monsters[index].moves_executed += 1;
                continue;
            }
            crate::MonsterIntent::EncourageGremlins { strength, block } => {
                let leader_id = state.monsters[index].id;
                apply_gremlin_leader_encourage(&mut state.monsters, leader_id, strength, block);
                state.monsters[index].moves_executed += 1;
                continue;
            }
            crate::MonsterIntent::SummonGremlins { count } => {
                let summoner_id = state.monsters[index].id;
                if state.monsters[index].content_id == BRONZE_AUTOMATON_ID {
                    apply_bronze_automaton_orb_spawn(&mut state.monsters, summoner_id);
                } else if state.monsters[index].content_id == ACID_SLIME_ID {
                    apply_large_acid_slime_split(&mut state.monsters, summoner_id);
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
                if let Some(monster) = state
                    .monsters
                    .iter_mut()
                    .find(|monster| monster.id == summoner_id)
                {
                    monster.moves_executed += 1;
                }
                continue;
            }
            _ => {}
        }
        let player_snapshot = state.player.clone();
        let intent = state.monsters[index].intent;
        let hits = match intent {
            crate::MonsterIntent::AttackMultiple { hits, .. } => hits,
            _ => 1,
        };
        let damage = apply_monster_intent_with_card_rng(
            &mut state.monsters[index],
            &mut state.player,
            &mut state.piles,
            ascension,
            &player_snapshot,
            &relics,
            state.card_random_rng.as_mut(),
        );
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
            pending_damage.push((
                damage,
                hits,
                state.monsters[index].powers.painful_stabs,
                heal_self,
                burn_to_discard_and_draw,
            ));
        }
    }

    for (damage, hits, painful_stabs, heal_self, burn_to_discard_and_draw) in pending_damage {
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
        apply_attack_heal_self_after_player_damage(state, heal_self, total_hp_damage);
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

fn deal_damage_to_player(state: &mut CombatState, amount: i32) -> i32 {
    let incoming = if state.player.powers.intangible > 0 && amount > 1 {
        1
    } else {
        amount
    };
    let blocked = state.player.block.min(incoming);
    state.player.block -= blocked;
    let mitigated =
        crate::relic::mitigate_unblocked_attack_damage(&state.relics, incoming - blocked);
    let hp_damage = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp -= hp_damage;
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

fn prepare_next_intents(state: &mut CombatState) {
    let living_monster_count = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .count();
    let alive_gremlin_count = gremlin_leader_alive_minion_count(&state.monsters);
    let missing_hp = living_monster_missing_hp(&state.monsters, state.ascension);
    for (monster_index, monster) in state.monsters.iter_mut().enumerate() {
        if is_half_dead_darkling(monster) {
            let _ = state.monster_rng.as_mut().map(|rng| rng.random_int(99));
            monster.intent = crate::MonsterIntent::Attack { damage: 0 };
            continue;
        }

        if monster.alive {
            let roll = state.monster_rng.as_mut().map(|rng| rng.random_int(99));
            monster.intent = if monster.content_id == HEXAGHOST_ID && monster.moves_executed == 1 {
                crate::MonsterIntent::AttackMultiple {
                    damage: (state.player.hp / 12) + 1,
                    hits: 6,
                }
            } else if monster.content_id == JAW_WORM_ID {
                if let Some(roll) = roll {
                    target_jaw_worm_next_intent_from_roll(&monster.move_history, roll)
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
    use crate::{
        apply_combat_action,
        combat::MonsterIntent,
        content::cards::{
            BASH_ID, DEFEND_R_ID, MAYHEM_ID, RETAIN_DEFEND_ID, STRIKE_R_ID, WOUND_ID,
        },
        content::monsters::{
            monster_state, ACID_SLIME_A0, BOOK_OF_STABBING_A0, CULTIST_A0, FIXED_SIMPLE_MONSTER,
            GREMLIN_LEADER_A0, GREMLIN_LEADER_ID, GREMLIN_WARRIOR_ID, HEXAGHOST_A0, MUGGER_A0,
            SHELLED_PARASITE_A0,
        },
        ids::CardId,
        CardInstance, CombatAction,
    };

    #[test]
    fn metallicize_grants_block_before_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.metallicize = 4;
        state.player.hp = 30;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 28);
    }

    #[test]
    fn hexaghost_divider_damage_uses_current_player_hp_after_activate() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 58;
        state.monsters = vec![monster_state(&HEXAGHOST_A0, MonsterId::new(1))];
        state.monsters[0].moves_executed = 1;

        prepare_next_intents(&mut state);

        assert_eq!(
            state.monsters[0].intent,
            MonsterIntent::AttackMultiple { damage: 5, hits: 6 }
        );
    }

    #[test]
    fn damaged_medium_acid_slime_keeps_medium_move_table() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&ACID_SLIME_A0, MonsterId::new(1))];
        state.monsters[0].hp = 12;
        state.monsters[0].move_history = vec![1, 2, 4];
        state.monsters[0].intent = MonsterIntent::ApplyPlayerWeak { amount: 1 };
        state.monster_rng = (0..1000).find_map(|seed| {
            let mut rng = crate::rng::StsRng::new(seed);
            (rng.random_int(99) < 30).then_some(crate::rng::StsRng::new(seed))
        });

        prepare_next_intents(&mut state);

        assert_eq!(
            state.monsters[0].intent,
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: crate::content::monsters::ACID_SLIME_ATTACK_DAMAGE,
                count: 1
            }
        );
    }

    #[test]
    fn plated_armor_blocks_then_loses_stack_on_unblocked_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.plated_armor = 4;
        state.player.hp = 20;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 18);
        assert_eq!(next.player.powers.plated_armor, 3);
    }

    #[test]
    fn plated_armor_does_not_decrement_when_attack_is_fully_blocked() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.plated_armor = 4;
        state.player.block = 10;
        state.player.hp = 20;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.powers.plated_armor, 4);
    }

    #[test]
    fn fire_breathing_triggers_on_next_turn_hand_drawn_statuses() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.fire_breathing = 6;
        state.piles.hand.clear();
        state.piles.discard_pile.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(20), STRIKE_R_ID),
            CardInstance::new(CardId::new(21), STRIKE_R_ID),
            CardInstance::new(CardId::new(22), STRIKE_R_ID),
            CardInstance::new(CardId::new(23), WOUND_ID),
            CardInstance::new(CardId::new(24), WOUND_ID),
        ];
        state.monsters[0].hp = 40;

        let next = end_player_turn(&state);

        assert_eq!(next.monsters[0].hp, 28);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            2
        );
    }

    #[test]
    fn monster_multi_hit_damage_consumes_block_per_hit() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 50;
        state.player.block = 8;
        state.player.powers.vulnerable = 2;
        state.monsters[0].intent = MonsterIntent::AttackMultiple { damage: 6, hits: 2 };
        state.monsters[0].powers.weak = 1;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 46);
    }

    #[test]
    fn shelled_parasite_life_suck_heals_by_actual_player_hp_damage() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1))];
        state.monsters[0].hp = 60;
        state.monsters[0].intent = MonsterIntent::AttackHealSelf { damage: 10 };
        state.player.hp = 40;
        state.player.block = 0;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 30);
        assert_eq!(next.monsters[0].hp, 70);
    }

    #[test]
    fn shelled_parasite_life_suck_does_not_heal_fully_blocked_hit() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1))];
        state.monsters[0].hp = 60;
        state.monsters[0].intent = MonsterIntent::AttackHealSelf { damage: 10 };
        state.player.hp = 40;
        state.player.block = 10;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 40);
        assert_eq!(next.monsters[0].hp, 60);
    }

    #[test]
    fn shelled_parasite_life_suck_healing_caps_at_definition_hp() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1))];
        state.monsters[0].hp = 65;
        state.monsters[0].intent = MonsterIntent::AttackHealSelf { damage: 10 };
        state.player.hp = 40;
        state.player.block = 0;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 30);
        assert_eq!(next.monsters[0].hp, 70);
    }

    #[test]
    fn book_of_stabbing_painful_stabs_adds_wound_after_unblocked_hit() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&BOOK_OF_STABBING_A0, MonsterId::new(1))];
        state.monsters[0].intent = MonsterIntent::Attack { damage: 6 };
        state.player.hp = 40;
        state.player.block = 0;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 34);
        assert_eq!(
            next.piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == WOUND_ID)
                .count(),
            1
        );
    }

    #[test]
    fn book_of_stabbing_painful_stabs_skips_fully_blocked_hit() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&BOOK_OF_STABBING_A0, MonsterId::new(1))];
        state.monsters[0].intent = MonsterIntent::Attack { damage: 6 };
        state.player.hp = 40;
        state.player.block = 6;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 40);
        assert!(!next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.content_id == WOUND_ID));
    }

    #[test]
    fn gremlin_leader_rally_summons_minions_without_immediate_minion_turns() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&GREMLIN_LEADER_A0, MonsterId::new(10))];
        state.monsters[0].intent = MonsterIntent::SummonGremlins { count: 2 };
        state.player.hp = 40;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 40);
        assert_eq!(next.monsters.len(), 3);
        assert_eq!(
            next.monsters
                .iter()
                .filter(|monster| monster.content_id == GREMLIN_WARRIOR_ID)
                .count(),
            2
        );
        let leader = next
            .monsters
            .iter()
            .find(|monster| monster.content_id == GREMLIN_LEADER_ID)
            .expect("leader remains present");
        assert_eq!(leader.moves_executed, 1);
    }

    #[test]
    fn mugger_escape_intent_ends_single_monster_combat_without_damage() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&MUGGER_A0, MonsterId::new(1))];
        state.monsters[0].intent = MonsterIntent::Escape;
        state.monsters[0].stolen_gold = 15;
        state.player.hp = 40;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.phase, CombatPhase::Won);
        assert_eq!(next.player.hp, 40);
        assert!(!next.monsters[0].alive);
        assert!(next.monsters[0].escaped);
        assert_eq!(next.monsters[0].stolen_gold, 15);
    }

    #[test]
    fn end_turn_is_legal() {
        let state = CombatState::initial_fixture();

        assert!(crate::legal_combat_actions(&state).contains(&CombatAction::EndTurn));
    }

    #[test]
    fn end_turn_moves_remaining_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let starting_hand_ids: Vec<_> = state.piles.hand.iter().map(|card| card.id).collect();

        let next = end_player_turn(&state);

        for card_id in starting_hand_ids {
            assert!(next
                .piles
                .discard_pile
                .iter()
                .any(|card| card.id == card_id));
        }
    }

    #[test]
    fn monster_attack_reduces_block_before_hp() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 4;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 18);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn torii_reduces_small_unblocked_monster_attack_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 1;
        state.relics = vec![crate::Relic::Torii];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
    }

    #[test]
    fn tungsten_rod_reduces_unblocked_monster_attack_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 4;
        state.relics = vec![crate::Relic::TungstenRod];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
    }

    #[test]
    fn buffer_prevents_next_monster_attack_hp_loss_after_block() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 4;
        state.player.powers.buffer = 1;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.powers.buffer, 0);
    }

    #[test]
    fn intangible_caps_monster_attack_damage_until_monster_turn_cleanup() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.powers.intangible = 1;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
        assert_eq!(next.player.powers.intangible, 0);
    }

    #[test]
    fn self_forming_clay_block_can_absorb_later_monster_attack() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 0;
        state.relics = vec![crate::Relic::SelfFormingClay];
        state.monsters[0].intent = MonsterIntent::Attack { damage: 2 };
        let mut second_monster = state.monsters[0].clone();
        second_monster.id = crate::MonsterId::new(2);
        second_monster.intent = MonsterIntent::Attack { damage: 2 };
        state.monsters.push(second_monster);

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 18);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn end_turn_enters_next_player_turn_with_refilled_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;

        let next = end_player_turn(&state);

        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
        assert_eq!(next.player.energy, crate::combat::state::BASE_PLAYER_ENERGY);
    }

    #[test]
    fn cannot_draw_clears_at_start_of_next_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.cannot_draw = true;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert!(!next.player.cannot_draw);
    }

    #[test]
    fn temp_strength_clears_at_start_of_next_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.temp_strength = 2;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.temp_strength, 0);
    }

    #[test]
    fn temp_thorns_clears_without_removing_persistent_thorns() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.thorns = 3;
        state.player.temp_thorns = 4;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.powers.thorns, 3);
        assert_eq!(next.player.temp_thorns, 0);
    }

    #[test]
    fn next_intent_placeholder_is_fixed_attack() {
        let state = CombatState::initial_fixture();

        let next = end_player_turn(&state);

        assert_eq!(
            next.monsters[0].intent,
            MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage,
            }
        );
    }

    #[test]
    fn block_clears_after_simplified_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 10;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn calipers_loses_fifteen_block_instead_of_all_block_after_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 30;
        state.relics = vec![crate::Relic::Calipers];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 9);
    }

    #[test]
    fn calipers_floors_retained_block_at_zero() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 16;
        state.relics = vec![crate::Relic::Calipers];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn barricade_preserves_block_after_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 30;
        state.player.powers.barricade = 1;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 24);
    }

    #[test]
    fn barricade_takes_precedence_over_calipers_block_loss() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 30;
        state.player.powers.barricade = 1;
        state.relics = vec![crate::Relic::Calipers];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 24);
    }

    #[test]
    fn next_hand_is_drawn_deterministically_without_shuffle() {
        let state = CombatState::initial_fixture();

        let next = end_player_turn(&state);

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn retained_cards_do_not_reduce_next_turn_draw_amount() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(20), RETAIN_DEFEND_ID)];
        state.piles.draw_pile = (30..36)
            .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();

        let next = end_player_turn(&state);

        assert_eq!(next.piles.hand.len(), 6);
        assert_eq!(next.piles.hand[0].content_id, RETAIN_DEFEND_ID);
        assert_eq!(
            next.piles
                .hand
                .iter()
                .filter(|card| card.content_id == STRIKE_R_ID)
                .count(),
            5
        );
        assert_eq!(next.piles.draw_pile.len(), 1);
    }

    #[test]
    fn snecko_eye_draws_seven_each_turn_and_randomizes_drawn_costs() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::SneckoEye];
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand.clear();
        state.piles.draw_pile = (10..20)
            .map(|id| crate::CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();

        start_player_turn(&mut state);

        assert_eq!(state.piles.hand.len(), 7);
        assert!(state.piles.hand.iter().all(|card| card.temp_cost.is_some()));
        assert_eq!(
            state.card_random_rng.as_ref().expect("card rng").counter(),
            7
        );
    }

    #[test]
    fn mayhem_plays_no_target_top_draw_card_after_normal_turn_draw() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), DEFEND_R_ID),
            CardInstance::new(CardId::new(11), STRIKE_R_ID),
            CardInstance::new(CardId::new(12), STRIKE_R_ID),
            CardInstance::new(CardId::new(13), STRIKE_R_ID),
            CardInstance::new(CardId::new(14), STRIKE_R_ID),
            CardInstance::new(CardId::new(15), STRIKE_R_ID),
        ];
        let mut expected_rng = crate::rng::StsRng::new(123);
        expected_rng.random_int(0);

        start_player_turn(&mut state);

        assert_eq!(state.player.block, 5);
        assert_eq!(state.piles.exhaust_pile.len(), 1);
        assert_eq!(state.piles.exhaust_pile[0].content_id, DEFEND_R_ID);
        assert_eq!(state.piles.hand.len(), 5);
        assert!(state
            .piles
            .hand
            .iter()
            .all(|card| card.content_id == STRIKE_R_ID));
        assert_eq!(
            state.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
    }

    #[test]
    fn mayhem_plays_enemy_target_top_draw_card_with_random_target() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.mayhem = 1;
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(10), STRIKE_R_ID),
            CardInstance::new(CardId::new(11), DEFEND_R_ID),
            CardInstance::new(CardId::new(12), DEFEND_R_ID),
            CardInstance::new(CardId::new(13), DEFEND_R_ID),
            CardInstance::new(CardId::new(14), DEFEND_R_ID),
            CardInstance::new(CardId::new(15), DEFEND_R_ID),
        ];
        let starting_hp = state.monsters[0].hp;
        let mut expected_rng = crate::rng::StsRng::new(123);
        expected_rng.random_int(0);

        start_player_turn(&mut state);

        assert_eq!(state.monsters[0].hp, starting_hp - 6);
        assert_eq!(state.piles.exhaust_pile.len(), 1);
        assert_eq!(state.piles.exhaust_pile[0].content_id, STRIKE_R_ID);
        assert_eq!(state.piles.hand.len(), 5);
        assert!(state
            .piles
            .hand
            .iter()
            .all(|card| card.content_id == DEFEND_R_ID));
        assert_eq!(
            state.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
    }

    #[test]
    fn played_mayhem_adds_start_turn_power_stack_and_removes_card() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(10), MAYHEM_ID));
        state.player.energy = 2;

        let next = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: CardId::new(10),
                target: None,
            },
        )
        .expect("Mayhem applies");

        assert_eq!(next.player.energy, 0);
        assert_eq!(next.player.powers.mayhem, 1);
        assert!(next.piles.hand.is_empty());
    }

    #[test]
    fn magnetism_adds_deterministic_colorless_card_before_normal_turn_draw() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.magnetism = 1;
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        let mut expected_rng = crate::rng::StsRng::new(123);
        let expected_pool = crate::combat::card_effects::magnetism_modeled_colorless_pool();
        let expected =
            expected_pool[expected_rng.random_int((expected_pool.len() - 1) as i32) as usize];

        start_player_turn(&mut state);

        assert_eq!(state.piles.hand.len(), 1);
        assert_eq!(state.piles.hand[0].content_id, expected);
        assert!(state.piles.hand[0].combat_only);
        assert_eq!(
            state.card_random_rng.as_ref().expect("card rng").counter(),
            expected_rng.counter()
        );
    }

    #[test]
    fn magnetism_stacks_add_one_colorless_card_each() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.magnetism = 2;
        state.piles.hand.clear();
        state.piles.draw_pile.clear();

        start_player_turn(&mut state);

        assert_eq!(state.piles.hand.len(), 2);
        let modeled_pool = crate::combat::card_effects::magnetism_modeled_colorless_pool();
        assert!(state
            .piles
            .hand
            .iter()
            .all(|card| modeled_pool.contains(&card.content_id)));
        assert!(state.piles.hand.iter().all(|card| card.combat_only));
    }

    #[test]
    fn magnetism_skips_generation_when_all_monsters_are_dead() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.magnetism = 1;
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.monsters[0].alive = false;
        let starting_card_rng_counter = state.card_random_rng.as_ref().expect("card rng").counter();

        start_player_turn(&mut state);

        assert!(state.piles.hand.is_empty());
        assert_eq!(
            state.card_random_rng.as_ref().expect("card rng").counter(),
            starting_card_rng_counter
        );
    }

    #[test]
    fn combat_can_reach_lost_state() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 6;
        state.player.block = 0;

        let next = end_player_turn(&state);

        assert_eq!(next.phase, CombatPhase::Lost);
    }

    #[test]
    fn monster_ritual_does_not_tick_on_the_turn_it_is_applied() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(&CULTIST_A0, MonsterId::new(1))];
        state.piles.hand.clear();
        state.piles.draw_pile.clear();

        let mut after_incantation = end_player_turn(&state);
        assert_eq!(
            after_incantation.monsters[0].powers.ritual,
            CULTIST_A0.ritual_amount
        );
        assert_eq!(after_incantation.monsters[0].powers.strength, 0);
        assert_eq!(
            after_incantation.monsters[0].intent,
            MonsterIntent::Attack { damage: 6 }
        );

        after_incantation.player.block = 5;
        let after_attack = end_player_turn(&after_incantation);

        assert_eq!(after_attack.player.hp, state.player.hp - 1);
        assert_eq!(
            after_attack.monsters[0].powers.strength,
            CULTIST_A0.ritual_amount
        );
    }

    #[test]
    fn vulnerable_decrements_at_end_of_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 100;
        state = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");
        assert_eq!(state.monsters[0].powers.vulnerable, 2);

        state = end_player_turn(&state);
        assert_eq!(state.monsters[0].powers.vulnerable, 1);

        state = end_player_turn(&state);
        assert_eq!(state.monsters[0].powers.vulnerable, 0);
    }

    #[test]
    fn monster_malleable_resets_at_end_of_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.malleable = 5;
        state.monsters[0].powers.malleable_base = 3;

        let next = end_player_turn(&state);

        assert_eq!(next.monsters[0].powers.malleable, 3);
    }

    #[test]
    fn living_monster_block_clears_at_player_end_turn() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].block = 12;
        state.monsters[0].intent = MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.monsters[0].block, 0);
    }

    #[test]
    fn spheric_guardian_barricade_preserves_block_at_player_end_turn() {
        let mut state = CombatState::initial_fixture();
        state.monsters = vec![monster_state(
            &crate::content::monsters::SPHERIC_GUARDIAN_A0,
            MonsterId::new(1),
        )];
        state.monsters[0].intent = MonsterIntent::Block { block: 0 };
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.monsters[0].block, 40);
    }

    fn bash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BASH_ID),
            target: Some(state.monsters[0].id),
        }
    }

    fn hand_card_id(state: &CombatState, content_id: crate::ContentId) -> CardId {
        state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == content_id)
            .expect("card is in hand")
            .id
    }
}
