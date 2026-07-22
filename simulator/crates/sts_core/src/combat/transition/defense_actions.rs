use super::{
    apply_player_card_block_gain, apply_player_vulnerable_debuff,
    apply_sadistic_nature_after_monster_debuff, checked_add_combat_value, living_monster_mut_opt,
};
use crate::{
    action::InternalAction,
    combat::CombatState,
    ids::MonsterId,
    power::{apply_monster_weak, reduce_monster_strength},
    SimError, SimResult,
};

pub(super) fn heal_player(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    crate::relic::heal_combat_player_with_relics(state, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_player_block(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    apply_player_card_block_gain(state, amount)
}

pub(super) fn gain_monster_block(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if let Some(monster) = living_monster_mut_opt(state, target) {
        checked_add_combat_value(&mut monster.block, amount)?;
    }
    Ok(Vec::new())
}

pub(super) fn prevent_block_gain(
    state: &mut CombatState,
    turns: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.no_block_turns = state.player.no_block_turns.max(turns);
    Ok(Vec::new())
}

pub(super) fn gain_temporary_thorns(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.temp_thorns, amount)?;
    Ok(Vec::new())
}

pub(super) fn double_player_block(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.player.block = state
        .player
        .block
        .checked_mul(2)
        .ok_or(SimError::InvalidState(
            "combat integer multiplication overflows i32",
        ))?;
    Ok(Vec::new())
}

pub(super) fn apply_monster_vulnerable(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    apply_player_vulnerable_debuff(state, target, amount)?;
    Ok(Vec::new())
}

pub(super) fn apply_player_vulnerable(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    crate::power::apply_player_vulnerable(&mut state.player.powers, amount)?;
    Ok(Vec::new())
}

pub(super) fn apply_weak(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let mut applied = false;
    if let Some(monster) = living_monster_mut_opt(state, target) {
        applied = apply_monster_weak(&mut monster.powers, amount)?;
    }
    apply_sadistic_nature_after_monster_debuff(state, target, applied)?;
    Ok(Vec::new())
}

pub(super) fn reduce_strength(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let mut applied = false;
    if let Some(monster) = living_monster_mut_opt(state, target) {
        applied = reduce_monster_strength(&mut monster.powers, amount)?;
    }
    apply_sadistic_nature_after_monster_debuff(state, target, applied)?;
    Ok(Vec::new())
}

pub(super) fn reduce_strength_this_turn(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    let mut applied = false;
    if let Some(monster) = living_monster_mut_opt(state, target) {
        applied = reduce_monster_strength(&mut monster.powers, amount)?;
        if applied {
            checked_add_combat_value(&mut monster.temp_strength_down, amount)?;
        }
    }
    apply_sadistic_nature_after_monster_debuff(state, target, applied)?;
    Ok(Vec::new())
}
