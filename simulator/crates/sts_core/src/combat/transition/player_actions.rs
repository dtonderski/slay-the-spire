use super::{
    add_rampage_damage_bonus, checked_add_combat_value, checked_combat_sum,
    set_random_hand_card_cost_for_combat, upgrade_combat_cards, upgrade_hand_card,
    upgrade_hand_cards_except,
};
use crate::{
    action::{HpLossSource, InternalAction},
    combat::{state::BombTimer, CombatState},
    ids::CardId,
    SimResult,
};

pub(super) fn gain_energy(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.energy, amount)?;
    Ok(Vec::new())
}

pub(super) fn lose_hp(
    state: &mut CombatState,
    amount: i32,
    source: HpLossSource,
) -> SimResult<Vec<InternalAction>> {
    let hp_loss = crate::combat::hp_loss::lose_player_hp(state, amount);
    if matches!(source, HpLossSource::Card(_)) {
        crate::combat::hp_loss::apply_player_card_hp_loss_hooks(state, hp_loss)?;
    } else {
        crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
    }
    Ok(Vec::new())
}

pub(super) fn set_cannot_draw(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.player.cannot_draw = true;
    Ok(Vec::new())
}

pub(super) fn gain_rage(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.temp_rage_block, amount)?;
    Ok(Vec::new())
}

pub(super) fn set_random_hand_card_cost(
    state: &mut CombatState,
    amount: u8,
    excluded_card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    set_random_hand_card_cost_for_combat(state, amount, excluded_card_id)?;
    Ok(Vec::new())
}

pub(super) fn upgrade_hand_cards_other_than(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    upgrade_hand_cards_except(state, card_id)?;
    Ok(Vec::new())
}

pub(super) fn upgrade_one_hand_card(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    upgrade_hand_card(state, card_id)?;
    Ok(Vec::new())
}

pub(super) fn increase_rampage_damage(
    state: &mut CombatState,
    card_id: CardId,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    add_rampage_damage_bonus(state, card_id, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_feel_no_pain(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.feel_no_pain, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_dark_embrace(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.dark_embrace, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_barricade(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.powers.barricade = state.player.powers.barricade.max(amount);
    Ok(Vec::new())
}

pub(super) fn gain_evolve(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.evolve, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_berserk(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.berserk, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_rupture(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.rupture, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_juggernaut(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.juggernaut, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_brutality(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.brutality, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_mayhem(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.mayhem, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_panache(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.panache, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_combust(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    let combust = checked_combat_sum(state.player.powers.combust, 1)?;
    let combust_damage = checked_combat_sum(state.player.powers.combust_damage, amount)?;
    state.player.powers.combust = combust;
    state.player.powers.combust_damage = combust_damage;
    Ok(Vec::new())
}

pub(super) fn gain_double_tap(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.double_tap_pending, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_fire_breathing(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.fire_breathing, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_corruption(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    state.player.powers.corruption = state.player.powers.corruption.max(amount);
    Ok(Vec::new())
}

pub(super) fn gain_sadistic_nature(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.sadistic_nature, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_magnetism(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.magnetism, amount)?;
    Ok(Vec::new())
}

pub(super) fn arm_the_bomb(
    state: &mut CombatState,
    turns: i32,
    damage: i32,
) -> SimResult<Vec<InternalAction>> {
    state.bomb_timers.push(BombTimer {
        turns_remaining: turns,
        damage,
    });
    Ok(Vec::new())
}

pub(super) fn gain_metallicize(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.metallicize, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_strength(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.strength, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_dexterity(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.dexterity, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_temp_strength(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    // Flex applies Strength and a debuff that removes it at end of turn.
    // Artifact blocks that debuff when it is created, consuming one Artifact
    // and leaving the gained Strength permanent.
    if state.player.powers.artifact > 0 {
        let strength = checked_combat_sum(state.player.powers.strength, amount)?;
        state.player.powers.artifact -= 1;
        state.player.powers.strength = strength;
    } else {
        checked_add_combat_value(&mut state.player.temp_strength, amount)?;
    }
    Ok(Vec::new())
}

pub(super) fn gain_intangible(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.intangible, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_ritual(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.ritual, amount)?;
    Ok(Vec::new())
}

pub(super) fn gain_artifact(
    state: &mut CombatState,
    amount: i32,
) -> SimResult<Vec<InternalAction>> {
    checked_add_combat_value(&mut state.player.powers.artifact, amount)?;
    Ok(Vec::new())
}

pub(super) fn upgrade_all_combat_cards(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    upgrade_combat_cards(state)?;
    Ok(Vec::new())
}
