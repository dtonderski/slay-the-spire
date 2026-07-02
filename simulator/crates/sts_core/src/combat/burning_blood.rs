use crate::{
    combat::{CombatPhase, CombatState},
    content::character::BURNING_BLOOD_HEAL_AMOUNT,
    relic::{heal_player_in_combat_with_relics, Relic, BLACK_BLOOD_HEAL, MEAT_ON_THE_BONE_HEAL},
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
    heal_player_in_combat_with_relics(
        &mut state.player.hp,
        state.player.max_hp,
        burning_blood_heal,
        &state.relics,
    );

    if state.relics.contains(&Relic::MeatOnTheBone) && state.player.hp * 2 <= state.player.max_hp {
        heal_player_in_combat_with_relics(
            &mut state.player.hp,
            state.player.max_hp,
            MEAT_ON_THE_BONE_HEAL,
            &state.relics,
        );
    }
}
