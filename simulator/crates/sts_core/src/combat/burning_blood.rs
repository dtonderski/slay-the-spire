use crate::{
    combat::{CombatPhase, CombatState},
    content::character::BURNING_BLOOD_HEAL_AMOUNT,
    relic::{heal_combat_player_with_relics, Relic, BLACK_BLOOD_HEAL, MEAT_ON_THE_BONE_HEAL},
    SimResult,
};

pub fn apply_burning_blood(state: &mut CombatState) -> SimResult<()> {
    if state.phase != CombatPhase::Won {
        return Ok(());
    }

    let mut next = state.clone();
    if next.player.authority.relics.contains(&Relic::BlackBlood) {
        heal_combat_player_with_relics(&mut next, BLACK_BLOOD_HEAL)?;
    } else if next.player.authority.relics.contains(&Relic::BurningBlood) {
        heal_combat_player_with_relics(&mut next, BURNING_BLOOD_HEAL_AMOUNT)?;
    }

    if next.player.authority.relics.contains(&Relic::MeatOnTheBone)
        && next.player.hp <= next.player.max_hp / 2
    {
        heal_combat_player_with_relics(&mut next, MEAT_ON_THE_BONE_HEAL)?;
    }
    *state = next;
    Ok(())
}
