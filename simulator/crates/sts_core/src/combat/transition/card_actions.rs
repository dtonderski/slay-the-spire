use super::{
    apply_copied_card_play_triggers, apply_enrage_on_card_type, apply_hand_card_play_triggers,
    apply_mummified_hand_on_power_play, apply_on_card_play_powers, apply_rage_on_card_type,
    card_content_definition, find_hand_card, find_hand_card_mut,
};
use crate::{
    action::InternalAction,
    combat::{cost::effective_card_cost_with_corruption, CombatState},
    content::cards::get_card_definition,
    ids::CardId,
    SimError, SimResult,
};

pub(super) fn consume_duplication_potion(
    state: &mut CombatState,
) -> SimResult<Vec<InternalAction>> {
    if state.duplication_potion_stacks > 0 {
        state.duplication_potion_stacks -= 1;
        if state.duplication_potion_stacks == 0 {
            state.duplication_potion_pending = false;
        }
    } else if state.duplication_potion_pending {
        state.duplication_potion_pending = false;
    }
    Ok(Vec::new())
}

pub(super) fn consume_double_tap(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.double_tap_pending = state.double_tap_pending.saturating_sub(1);
    Ok(Vec::new())
}

pub(super) fn consume_necronomicon(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.relic_counters.necronomicon_used_this_turn = true;
    Ok(Vec::new())
}

pub(super) fn consume_vigor(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    state.player.powers.vigor = 0;
    Ok(Vec::new())
}

pub(super) fn play_card(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let card = find_hand_card(state, card_id)?;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;
    apply_enrage_on_card_type(state, definition.card_type)?;
    apply_rage_on_card_type(state, definition.card_type)?;
    let mut follow_ups = crate::relic::apply_on_card_play_relics(state, definition.card_type)?;
    apply_mummified_hand_on_power_play(state, card_id, definition.card_type);
    follow_ups.extend(apply_on_card_play_powers(state, definition.card_type)?);
    follow_ups.extend(apply_hand_card_play_triggers(state, card_id));
    Ok(follow_ups)
}

pub(super) fn play_card_copy(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let definition = card_content_definition(state, card_id)?;
    apply_enrage_on_card_type(state, definition.card_type)?;
    apply_rage_on_card_type(state, definition.card_type)?;
    let mut follow_ups = crate::relic::apply_on_card_play_relics(state, definition.card_type)?;
    follow_ups.extend(apply_on_card_play_powers(state, definition.card_type)?);
    follow_ups.extend(apply_copied_card_play_triggers(state));
    Ok(follow_ups)
}

pub(super) fn spend_energy(state: &mut CombatState, amount: i32) -> SimResult<Vec<InternalAction>> {
    state.player.energy -= amount;
    Ok(Vec::new())
}

pub(super) fn spend_card_energy(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let card = state
        .piles
        .hand
        .iter()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;
    let cost = effective_card_cost_with_corruption(card, state.player.powers.corruption > 0)?;
    state.player.energy = state
        .player
        .energy
        .checked_sub(cost)
        .ok_or(SimError::InvalidState("combat energy spend overflows i32"))?;
    Ok(Vec::new())
}

pub(super) fn set_hand_card_cost_for_turn(
    state: &mut CombatState,
    card_id: CardId,
    cost: u8,
) -> SimResult<Vec<InternalAction>> {
    let card = find_hand_card_mut(state, card_id)?;
    card.temp_cost = Some(cost);
    card.temp_cost_turn_only = true;
    Ok(Vec::new())
}

pub(super) fn set_hand_card_cost_for_combat(
    state: &mut CombatState,
    card_id: CardId,
    cost: u8,
) -> SimResult<Vec<InternalAction>> {
    let card = find_hand_card_mut(state, card_id)?;
    card.temp_cost = Some(cost);
    card.temp_cost_turn_only = false;
    Ok(Vec::new())
}
