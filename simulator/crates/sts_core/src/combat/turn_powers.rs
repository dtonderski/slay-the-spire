use crate::combat::{CombatState, MonsterState, PlayerState};
use crate::content::cards::{COMBUST_DAMAGE, COMBUST_HP_LOSS};
use crate::content::monsters::{
    check_slime_boss_split, guardian_on_hp_damage, wake_lagavulin_on_damage,
};
use crate::power::attack_damage_with_vulnerable;
use crate::relic::{heal_combat_player_with_relics, heal_player_in_combat_with_relics, Relic};
use crate::{combat::damage::deal_unmodified_damage_to_monster, MonsterId};

pub fn apply_end_of_player_turn_powers(state: &mut CombatState) {
    apply_player_end_of_turn_powers_for_combat_state(state);
    apply_end_of_turn_combust(state);
    apply_end_of_turn_bomb_timers(state);
}

fn apply_player_end_of_turn_powers_for_combat_state(state: &mut CombatState) {
    if state.player.powers.ritual > 0 {
        state.player.powers.strength += state.player.powers.ritual;
    }
    if state.player.powers.metallicize > 0 {
        crate::combat::transition::apply_player_direct_block_gain(
            state,
            state.player.powers.metallicize,
        );
    }
    if state.player.powers.plated_armor > 0 {
        crate::combat::transition::apply_player_direct_block_gain(
            state,
            state.player.powers.plated_armor,
        );
    }
    if state.player.powers.regen > 0 {
        heal_combat_player_with_relics(state, state.player.powers.regen);
        state.player.powers.regen -= 1;
    }
    if state.player.powers.weak > 0 {
        state.player.powers.weak -= 1;
    }
    if state.player.powers.frail > 0 {
        state.player.powers.frail -= 1;
    }
    if state.player.powers.entangled > 0 {
        state.player.powers.entangled = 0;
    }
}

pub fn apply_player_end_of_turn_powers(player: &mut PlayerState) {
    apply_player_end_of_turn_powers_with_relics(player, &[]);
}

pub fn apply_player_end_of_turn_powers_with_relics(player: &mut PlayerState, relics: &[Relic]) {
    if player.powers.ritual > 0 {
        player.powers.strength += player.powers.ritual;
    }
    if player.powers.metallicize > 0 {
        if player.no_block_turns == 0 {
            player.block += player.powers.metallicize;
        }
    }
    if player.powers.plated_armor > 0 {
        if player.no_block_turns == 0 {
            player.block += player.powers.plated_armor;
        }
    }
    if player.powers.regen > 0 {
        heal_player_in_combat_with_relics(
            &mut player.hp,
            player.max_hp,
            player.powers.regen,
            relics,
        );
        player.powers.regen -= 1;
    }
    if player.powers.weak > 0 {
        player.powers.weak -= 1;
    }
    if player.powers.frail > 0 {
        player.powers.frail -= 1;
    }
    if player.powers.entangled > 0 {
        player.powers.entangled = 0;
    }
}

fn apply_end_of_turn_combust(state: &mut CombatState) {
    for _ in 0..state.player.powers.combust.max(0) {
        let hp_loss = lose_player_hp(state, COMBUST_HP_LOSS);
        crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss);
        if state.player.hp <= 0 {
            return;
        }
    }
    deal_combust_damage_to_living_monsters(state);
}

fn lose_player_hp(state: &mut CombatState, amount: i32) -> i32 {
    crate::combat::hp_loss::lose_player_hp(state, amount)
}

fn deal_combust_damage_to_living_monsters(state: &mut CombatState) {
    let legacy_base_damage = state.player.powers.combust * COMBUST_DAMAGE;
    let damage = state.player.powers.combust_damage.max(legacy_base_damage);
    deal_unmodified_damage_to_living_monsters(state, damage);
}

fn apply_end_of_turn_bomb_timers(state: &mut CombatState) {
    if state.bomb_timers.is_empty() {
        return;
    }

    let timers = std::mem::take(&mut state.bomb_timers);
    for mut timer in timers {
        timer.turns_remaining -= 1;
        if timer.turns_remaining <= 0 {
            deal_unmodified_damage_to_living_monsters(state, timer.damage);
            if state.player.hp <= 0 || state.monsters.iter().all(|monster| !monster.alive) {
                return;
            }
        } else {
            state.bomb_timers.push(timer);
        }
    }
}

fn deal_unmodified_damage_to_living_monsters(state: &mut CombatState, amount: i32) {
    let targets = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster.id)
        .collect::<Vec<MonsterId>>();

    for target in targets {
        let killed = {
            let monster = state
                .monsters
                .iter_mut()
                .find(|monster| monster.id == target && monster.alive)
                .expect("target was collected from living monsters");
            let hp_damage = deal_unmodified_damage_to_monster(monster, amount);
            wake_lagavulin_on_damage(monster, hp_damage);
            guardian_on_hp_damage(monster, hp_damage);
            !monster.alive
        };
        check_slime_boss_split(state, target);
        if killed {
            crate::combat::transition::apply_monster_death_hooks(state, target);
        }
    }
}

pub fn apply_end_of_monster_turn_powers(monster: &mut MonsterState) {
    apply_end_of_monster_turn_powers_with_ritual(monster, true);
}

pub fn apply_end_of_monster_turn_powers_without_ritual(monster: &mut MonsterState) {
    apply_end_of_monster_turn_powers_with_ritual(monster, false);
}

fn apply_end_of_monster_turn_powers_with_ritual(monster: &mut MonsterState, apply_ritual: bool) {
    if apply_ritual && monster.powers.ritual > 0 {
        monster.powers.strength += monster.powers.ritual;
    }
    if monster.powers.metallicize > 0 {
        monster.block += monster.powers.metallicize;
    }
    if monster.powers.plated_armor > 0 {
        monster.block += monster.powers.plated_armor;
    }
}

pub fn monster_attack_damage(monster: &MonsterState, base: i32) -> i32 {
    let with_strength = (base + monster.powers.strength).max(0);
    if monster.powers.weak > 0 {
        with_strength * 3 / 4
    } else {
        with_strength
    }
}

/// Monster attack damage after player vulnerable (1.5x floored per hit).
#[must_use]
pub fn monster_damage_to_player(player: &PlayerState, monster: &MonsterState, base: i32) -> i32 {
    let raw = monster_attack_damage(monster, base);
    attack_damage_with_vulnerable(raw, player.powers.vulnerable)
}
