use crate::{
    combat::{CombatPhase, CombatState},
    content::character::BURNING_BLOOD_HEAL_AMOUNT,
    relic::{heal_combat_player_with_relics, Relic, BLACK_BLOOD_HEAL, MEAT_ON_THE_BONE_HEAL},
};

pub fn apply_burning_blood(state: &mut CombatState) {
    if state.phase != CombatPhase::Won {
        return;
    }

    if state.relics.contains(&Relic::BlackBlood) {
        heal_combat_player_with_relics(state, BLACK_BLOOD_HEAL);
    } else if state.relics.contains(&Relic::BurningBlood) {
        heal_combat_player_with_relics(state, BURNING_BLOOD_HEAL_AMOUNT);
    }

    if state.relics.contains(&Relic::MeatOnTheBone) && state.player.hp * 2 <= state.player.max_hp {
        heal_combat_player_with_relics(state, MEAT_ON_THE_BONE_HEAL);
    }
}
