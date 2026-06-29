use crate::combat::{CombatState, MonsterState, PlayerState};
use crate::content::cards::{COMBUST_DAMAGE, COMBUST_HP_LOSS};
use crate::content::monsters::{
    check_slime_boss_split, guardian_on_hp_damage, wake_lagavulin_on_damage,
};
use crate::power::attack_damage_with_vulnerable;
use crate::relic::{heal_player_in_combat_with_relics, Relic};
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
        heal_player_in_combat_with_relics(
            &mut state.player.hp,
            state.player.max_hp,
            state.player.powers.regen,
            &state.relics,
        );
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
    let mitigated = crate::relic::mitigate_hp_loss(&state.relics, amount);
    let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp -= hp_loss;
    hp_loss
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::CombatPhase, content::character::IRONCLAD_A0_BASE_HP, CombatState, Relic};

    #[test]
    fn ritual_grants_strength_at_end_of_player_turn() {
        let mut player = CombatState::initial_fixture().player;
        player.powers.ritual = 2;

        apply_player_end_of_turn_powers(&mut player);

        assert_eq!(player.powers.strength, 2);
    }

    #[test]
    fn metallicize_grants_block_at_end_of_player_turn() {
        let mut player = CombatState::initial_fixture().player;
        player.powers.metallicize = 3;

        apply_player_end_of_turn_powers(&mut player);

        assert_eq!(player.block, 3);
    }

    #[test]
    fn plated_armor_grants_block_at_end_of_player_turn() {
        let mut player = CombatState::initial_fixture().player;
        player.powers.plated_armor = 4;

        apply_player_end_of_turn_powers(&mut player);

        assert_eq!(player.block, 4);
        assert_eq!(player.powers.plated_armor, 4);
    }

    #[test]
    fn player_powers_with_ritual_round_trip_through_json() {
        let mut player = CombatState::initial_fixture().player;
        player.powers.ritual = 2;
        let json = serde_json::to_string(&player.powers).expect("powers serialize");
        let restored: crate::PlayerPowers =
            serde_json::from_str(&json).expect("powers deserialize");

        assert_eq!(restored, player.powers);
    }

    #[test]
    fn regen_heals_and_decrements_at_end_of_player_turn() {
        let mut player = CombatState::initial_fixture().player;
        player.hp = 70;
        player.powers.regen = 5;

        apply_player_end_of_turn_powers(&mut player);

        assert_eq!(player.hp, 75);
        assert_eq!(player.powers.regen, 4);
    }

    #[test]
    fn regen_heal_caps_at_max_hp() {
        let mut player = CombatState::initial_fixture().player;
        player.hp = 78;
        player.powers.regen = 5;

        apply_player_end_of_turn_powers(&mut player);

        assert_eq!(player.hp, player.max_hp);
        assert_eq!(player.powers.regen, 4);
    }

    #[test]
    fn combust_hp_loss_triggers_rupture_strength() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 50;
        state.player.powers.combust = 1;
        state.player.powers.combust_damage = 7;
        state.player.powers.rupture = 2;

        apply_end_of_player_turn_powers(&mut state);

        assert_eq!(state.player.hp, 49);
        assert_eq!(state.player.powers.strength, 2);
    }

    #[test]
    fn magic_flower_increases_regen_combat_healing() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::MagicFlower];
        state.player.hp = 60;
        state.player.powers.regen = 5;

        apply_end_of_player_turn_powers(&mut state);

        assert_eq!(state.player.hp, 68);
        assert_eq!(state.player.powers.regen, 4);
    }

    #[test]
    fn monster_ritual_grants_strength_after_monster_turn() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.ritual = 1;

        apply_end_of_monster_turn_powers(&mut monster);

        assert_eq!(monster.powers.strength, 1);
    }

    #[test]
    fn monster_metallicize_grants_block_after_monster_turn() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.metallicize = 8;

        apply_end_of_monster_turn_powers(&mut monster);

        assert_eq!(monster.block, 8);
    }

    #[test]
    fn monster_strength_increases_attack_damage() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.strength = 2;

        assert_eq!(monster_attack_damage(&monster, 6), 8);
    }

    #[test]
    fn monster_anger_does_not_directly_increase_attack_damage() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.anger = 2;

        assert_eq!(monster_attack_damage(&monster, 6), 6);
    }

    #[test]
    fn monster_weak_reduces_attack_damage_with_floor() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.weak = 1;

        assert_eq!(monster_attack_damage(&monster, 6), 4);
        assert_eq!(monster_attack_damage(&monster, 7), 5);
    }

    #[test]
    fn frail_decrements_at_end_of_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.frail = 2;
        state.player.hp = IRONCLAD_A0_BASE_HP;
        state.player.block = 0;

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.frail, 1);
        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
    }

    #[test]
    fn entangled_expires_at_end_of_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.entangled = 1;
        state.player.hp = IRONCLAD_A0_BASE_HP;
        state.player.block = 0;

        let next = crate::combat::end_player_turn(&state);

        assert_eq!(next.player.powers.entangled, 0);
        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
    }
}
