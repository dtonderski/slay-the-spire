use crate::combat::{CombatState, MonsterState, PlayerState};

pub fn apply_end_of_player_turn_powers(state: &mut CombatState) {
    apply_player_end_of_turn_powers(&mut state.player);
}

pub fn apply_player_end_of_turn_powers(player: &mut PlayerState) {
    if player.powers.ritual > 0 {
        player.powers.strength += player.powers.ritual;
    }
    if player.powers.metallicize > 0 {
        player.block += player.powers.metallicize;
    }
    if player.powers.weak > 0 {
        player.powers.weak -= 1;
    }
    if player.powers.frail > 0 {
        player.powers.frail -= 1;
    }
}

pub fn apply_end_of_monster_turn_powers(monster: &mut MonsterState) {
    if monster.powers.ritual > 0 {
        monster.powers.strength += monster.powers.ritual;
    }
}

pub fn monster_attack_damage(monster: &MonsterState, base: i32) -> i32 {
    base + monster.powers.strength
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::CombatPhase, content::character::IRONCLAD_A0_BASE_HP, CombatState};

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
    fn player_powers_with_ritual_round_trip_through_json() {
        let mut player = CombatState::initial_fixture().player;
        player.powers.ritual = 2;
        let json = serde_json::to_string(&player.powers).expect("powers serialize");
        let restored: crate::PlayerPowers =
            serde_json::from_str(&json).expect("powers deserialize");

        assert_eq!(restored, player.powers);
    }

    #[test]
    fn monster_ritual_grants_strength_after_monster_turn() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.ritual = 1;

        apply_end_of_monster_turn_powers(&mut monster);

        assert_eq!(monster.powers.strength, 1);
    }

    #[test]
    fn monster_strength_increases_attack_damage() {
        let mut monster = CombatState::initial_fixture().monsters[0].clone();
        monster.powers.strength = 2;

        assert_eq!(monster_attack_damage(&monster, 6), 8);
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
}
