use super::{
    apply_player_card_block_gain, apply_player_vulnerable_debuff, checked_add_combat_value,
    juggernaut_follow_up_for_positive_block_gain, living_monster_mut_opt,
    sadistic_nature_follow_up_after_monster_debuff,
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

pub(super) fn gain_precomputed_player_card_block(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    if state.player.no_block_turns > 0 {
        return Ok(Vec::new());
    }
    checked_add_combat_value(&mut state.player.block, amount)?;
    Ok(juggernaut_follow_up_for_positive_block_gain(state, amount))
}

pub(super) fn gain_player_block_direct(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    // Relic/power callbacks using the direct path (Rage, Abacus, Fan) bypass
    // No Block; ordinary card block uses gain_player_block above.
    checked_add_combat_value(&mut state.player.block, amount)?;
    Ok(juggernaut_follow_up_for_positive_block_gain(state, amount))
}

pub(super) fn gain_player_block_from_exhaust(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.block, amount)?;
    Ok(juggernaut_follow_up_for_positive_block_gain(state, amount))
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
    // No Block is a debuff. Artifact prevents Panic Button's application and
    // is consumed instead of leaving a hidden no-block duration behind
    // (FIDL01632).
    if state.player.powers.artifact > 0 {
        state.player.powers.artifact -= 1;
        return Ok(Vec::new());
    }
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
    // Entrench doubles block; the added half is a block gain for Juggernaut.
    let before = state.player.block;
    state.player.block = before.checked_mul(2).ok_or(SimError::InvalidState(
        "combat integer multiplication overflows i32",
    ))?;
    let gained = state.player.block - before;
    Ok(juggernaut_follow_up_for_positive_block_gain(state, gained))
}

pub(super) fn apply_monster_vulnerable(
    state: &mut CombatState,
    target: MonsterId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    apply_player_vulnerable_debuff(state, target, amount, false)
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
    Ok(
        sadistic_nature_follow_up_after_monster_debuff(state, target, applied)
            .into_iter()
            .collect(),
    )
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
    Ok(
        sadistic_nature_follow_up_after_monster_debuff(state, target, applied)
            .into_iter()
            .collect(),
    )
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
    Ok(
        sadistic_nature_follow_up_after_monster_debuff(state, target, applied)
            .into_iter()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feel_no_pain_exhaust_block_ignores_no_block_power() {
        let mut state = CombatState::initial_fixture();
        state.player.no_block_turns = 2;

        gain_player_block_from_exhaust(&mut state, 3).expect("exhaust block gain succeeds");

        assert_eq!(state.player.block, 3);
        assert_eq!(state.player.no_block_turns, 2);
    }

    #[test]
    fn artifact_prevents_panic_button_no_block() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.artifact = 1;

        prevent_block_gain(&mut state, 2).expect("artifact prevents no-block application");

        assert_eq!(state.player.powers.artifact, 0);
        assert_eq!(state.player.no_block_turns, 0);
    }
}
