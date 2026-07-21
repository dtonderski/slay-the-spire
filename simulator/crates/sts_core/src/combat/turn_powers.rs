use crate::combat::{CombatState, MonsterState, PlayerState};
use crate::content::cards::COMBUST_HP_LOSS;
use crate::content::monsters::{
    check_slime_boss_split, guardian_on_hp_damage, wake_lagavulin_on_damage,
};
use crate::relic::{heal_combat_player_with_relics, heal_player_in_combat_with_relics, Relic};
use crate::{combat::damage::deal_unmodified_damage_to_monster, MonsterId, SimError, SimResult};

pub fn apply_end_of_player_turn_powers(state: &mut CombatState) -> SimResult<()> {
    apply_player_end_of_turn_powers_for_combat_state(state)?;
    apply_end_of_turn_constricted(state);
    if state.player.hp <= 0 {
        return Ok(());
    }
    apply_end_of_turn_combust(state);
    if state.player.hp <= 0 {
        return Ok(());
    }
    apply_end_of_turn_bomb_timers(state);
    Ok(())
}

fn apply_player_end_of_turn_powers_for_combat_state(state: &mut CombatState) -> SimResult<()> {
    if state.player.powers.ritual > 0 {
        state.player.powers.strength = state
            .player
            .powers
            .strength
            .checked_add(state.player.powers.ritual)
            .ok_or(SimError::InvalidState(
                "combat integer addition overflows i32",
            ))?;
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
    Ok(())
}

fn apply_end_of_turn_constricted(state: &mut CombatState) {
    if state.player.powers.constricted <= 0 {
        return;
    }
    let hp_loss = lose_player_hp(state, state.player.powers.constricted);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
    crate::combat::turn::revive_player_if_available(state);
}

pub fn apply_player_end_of_turn_powers(player: &mut PlayerState) {
    apply_player_end_of_turn_powers_with_relics(player, &[]);
}

pub fn apply_player_end_of_turn_powers_with_relics(player: &mut PlayerState, relics: &[Relic]) {
    if player.powers.ritual > 0 {
        player.powers.strength += player.powers.ritual;
    }
    if player.powers.metallicize > 0 && player.no_block_turns == 0 {
        player.block += player.powers.metallicize;
    }
    if player.powers.plated_armor > 0 && player.no_block_turns == 0 {
        player.block += player.powers.plated_armor;
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
    if player.powers.constricted > 0 {
        player.hp = (player.hp - player.powers.constricted).max(0);
    }
}

fn apply_end_of_turn_combust(state: &mut CombatState) {
    let combust_stacks = state.player.powers.combust.max(0);
    if combust_stacks > 0 {
        // Stacked Combust is one LoseHPAction whose hpLoss field is increased by
        // one per stack. Card-loss hooks such as Rupture therefore fire once,
        // not once for every point of HP lost.
        let hp_loss = lose_player_hp(state, combust_stacks * COMBUST_HP_LOSS);
        crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss);
        crate::combat::turn::revive_player_if_available(state);
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
    deal_unmodified_damage_to_living_monsters(state, state.player.powers.combust_damage);
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

pub fn monster_attack_damage(monster: &MonsterState, base: i32) -> SimResult<i32> {
    let with_strength = base
        .checked_add(monster.powers.strength)
        .ok_or(SimError::InvalidState(
            "monster attack damage arithmetic overflow",
        ))?
        .max(0);
    if monster.powers.weak > 0 {
        Ok(i32::try_from(i64::from(with_strength) * 3 / 4)
            .map_err(|_| SimError::InvalidState("monster attack damage arithmetic overflow"))?)
    } else {
        Ok(with_strength)
    }
}

/// Monster attack damage after monster Weak and player Vulnerable.
pub fn monster_damage_to_player(
    player: &PlayerState,
    monster: &MonsterState,
    base: i32,
) -> SimResult<i32> {
    let damage = base
        .checked_add(monster.powers.strength)
        .ok_or(SimError::InvalidState(
            "monster attack damage arithmetic overflow",
        ))?
        .max(0);
    let mut numerator = i128::from(damage);
    let mut denominator = 1_i128;
    if monster.powers.weak > 0 {
        numerator *= 3;
        denominator *= 4;
    }
    if player.powers.vulnerable > 0 {
        numerator *= 3;
        denominator *= 2;
    }
    i32::try_from(numerator / denominator)
        .map_err(|_| SimError::InvalidState("monster attack damage arithmetic overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;

    #[test]
    fn monster_weak_and_player_vulnerable_truncate_once() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.strength = 3;
        state.monsters[0].powers.weak = 1;
        state.player.powers.vulnerable = 1;

        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], 18),
            Ok(23)
        );
    }

    #[test]
    fn monster_attack_damage_rejects_unrepresentable_values() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].powers.strength = i32::MAX;

        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], 1),
            Err(SimError::InvalidState(
                "monster attack damage arithmetic overflow"
            ))
        );

        state.monsters[0].powers.strength = 0;
        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], i32::MAX),
            Ok(i32::MAX)
        );

        state.player.powers.vulnerable = 1;
        assert_eq!(
            monster_damage_to_player(&state.player, &state.monsters[0], i32::MAX),
            Err(SimError::InvalidState(
                "monster attack damage arithmetic overflow"
            ))
        );
    }

    #[test]
    fn stacked_combust_triggers_rupture_once_for_the_combined_hp_loss() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.powers.combust = 2;
        state.player.powers.combust_damage = 10;
        state.player.powers.rupture = 2;
        let monster_hp = state.monsters[0].hp;

        apply_end_of_player_turn_powers(&mut state).expect("end-turn powers resolve");

        assert_eq!(state.player.hp, 18);
        assert_eq!(state.player.powers.strength, 2);
        assert_eq!(state.monsters[0].hp, monster_hp - 10);
    }
}
