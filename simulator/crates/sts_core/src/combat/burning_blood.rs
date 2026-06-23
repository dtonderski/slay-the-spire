use crate::{
    combat::{CombatPhase, CombatState},
    content::character::BURNING_BLOOD_HEAL_AMOUNT,
    relic::{Relic, BLACK_BLOOD_HEAL, MEAT_ON_THE_BONE_HEAL},
};

pub fn apply_burning_blood(state: &mut CombatState) {
    if state.phase != CombatPhase::Won {
        return;
    }

    let burning_blood_heal = if state.relics.contains(&Relic::BlackBlood) {
        BLACK_BLOOD_HEAL
    } else {
        BURNING_BLOOD_HEAL_AMOUNT
    };
    state.player.hp = (state.player.hp + burning_blood_heal).min(state.player.max_hp);

    if state.relics.contains(&Relic::MeatOnTheBone) && state.player.hp * 2 <= state.player.max_hp {
        state.player.hp = (state.player.hp + MEAT_ON_THE_BONE_HEAL).min(state.player.max_hp);
    }
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

    #[test]
    fn black_blood_replaces_burning_blood_victory_heal() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::BlackBlood];
        state.player.hp = 60;
        state.player.max_hp = IRONCLAD_A0_BASE_HP;
        state.phase = CombatPhase::Won;

        apply_burning_blood(&mut state);

        assert_eq!(state.player.hp, 60 + BLACK_BLOOD_HEAL);
    }

    #[test]
    fn meat_on_the_bone_heals_after_victory_when_at_or_below_half_hp() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::MeatOnTheBone];
        state.player.hp = 30;
        state.player.max_hp = IRONCLAD_A0_BASE_HP;
        state.phase = CombatPhase::Won;

        apply_burning_blood(&mut state);

        assert_eq!(
            state.player.hp,
            30 + BURNING_BLOOD_HEAL_AMOUNT + MEAT_ON_THE_BONE_HEAL
        );
    }

    #[test]
    fn meat_on_the_bone_skips_when_victory_heal_lifts_above_half_hp() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![Relic::MeatOnTheBone];
        state.player.hp = 38;
        state.player.max_hp = IRONCLAD_A0_BASE_HP;
        state.phase = CombatPhase::Won;

        apply_burning_blood(&mut state);

        assert_eq!(state.player.hp, 38 + BURNING_BLOOD_HEAL_AMOUNT);
    }
}
