use crate::{
    combat::{CombatPhase, CombatState},
    content::character::BURNING_BLOOD_HEAL_AMOUNT,
};

pub fn apply_burning_blood(state: &mut CombatState) {
    if state.phase != CombatPhase::Won {
        return;
    }

    state.player.hp = (state.player.hp + BURNING_BLOOD_HEAL_AMOUNT).min(state.player.max_hp);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::character::IRONCLAD_A0_BASE_HP;

    #[test]
    fn combat_victory_heals_six() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 70;
        state.player.max_hp = IRONCLAD_A0_BASE_HP;
        state.phase = CombatPhase::Won;

        apply_burning_blood(&mut state);

        assert_eq!(state.player.hp, 76);
    }

    #[test]
    fn combat_victory_heal_is_capped_by_max_hp() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 78;
        state.player.max_hp = IRONCLAD_A0_BASE_HP;
        state.phase = CombatPhase::Won;

        apply_burning_blood(&mut state);

        assert_eq!(state.player.hp, IRONCLAD_A0_BASE_HP);
    }

    #[test]
    fn combat_loss_does_not_heal() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 10;
        state.player.max_hp = IRONCLAD_A0_BASE_HP;
        state.phase = CombatPhase::Lost;

        apply_burning_blood(&mut state);

        assert_eq!(state.player.hp, 10);
    }
}
