use crate::{
    combat::{CombatState, PlayerState},
    content::cards::{BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID},
    SimResult,
};

pub(crate) fn cap_player_damage_with_intangible(player: &PlayerState, amount: i32) -> i32 {
    let amount = amount.max(0);
    if player.powers.intangible > 0 && amount > 1 {
        1
    } else {
        amount
    }
}

pub(crate) fn lose_player_hp(state: &mut CombatState, amount: i32) -> i32 {
    let incoming = cap_player_damage_with_intangible(&state.player, amount);
    let mitigated = crate::relic::mitigate_hp_loss(&state.relics, incoming);
    let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp = (state.player.hp - hp_loss).max(0);
    hp_loss
}

pub(crate) fn lose_player_blockable_hp(state: &mut CombatState, amount: i32) -> i32 {
    let incoming = cap_player_damage_with_intangible(&state.player, amount);
    let blocked = state.player.block.min(incoming);
    state.player.block -= blocked;
    let mitigated = crate::relic::mitigate_hp_loss(&state.relics, incoming - blocked);
    let hp_loss = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp = (state.player.hp - hp_loss).max(0);
    hp_loss
}

pub(crate) fn apply_player_hp_loss_hooks(state: &mut CombatState, hp_loss: i32) -> SimResult<()> {
    if hp_loss <= 0 {
        return Ok(());
    }

    let mut next = state.clone();
    apply_player_hp_loss_hooks_in_place(&mut next, hp_loss)?;
    *state = next;
    Ok(())
}

pub(crate) fn apply_player_card_hp_loss_hooks(
    state: &mut CombatState,
    hp_loss: i32,
) -> SimResult<()> {
    if hp_loss <= 0 {
        return Ok(());
    }

    let mut next = state.clone();
    apply_player_hp_loss_hooks_in_place(&mut next, hp_loss)?;
    next.player.powers.strength = next
        .player
        .powers
        .strength
        .checked_add(next.player.powers.rupture)
        .ok_or(crate::SimError::InvalidState(
            "Rupture Strength gain overflows i32",
        ))?;
    *state = next;
    Ok(())
}

fn apply_player_hp_loss_hooks_in_place(state: &mut CombatState, hp_loss: i32) -> SimResult<()> {
    reduce_blood_for_blood_costs(state)?;
    crate::relic::apply_player_hp_loss_relics(state, hp_loss)
}

fn reduce_blood_for_blood_costs(state: &mut CombatState) -> SimResult<()> {
    for pile in [
        &mut state.piles.hand,
        &mut state.piles.draw_pile,
        &mut state.piles.discard_pile,
        &mut state.piles.exhaust_pile,
    ] {
        for card in pile {
            if card.content_id == BLOOD_FOR_BLOOD_ID || card.content_id == BLOOD_FOR_BLOOD_PLUS_ID {
                card.blood_for_blood_cost_reduction = card
                    .blood_for_blood_cost_reduction
                    .checked_add(1)
                    .ok_or(crate::SimError::InvalidState(
                        "Blood for Blood cost reduction overflows i32",
                    ))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardId, CardInstance, Relic, SimError};

    #[test]
    fn blood_for_blood_overflow_rolls_back_all_pile_reductions() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![CardInstance::new(CardId::new(100), BLOOD_FOR_BLOOD_ID)];
        let mut overflowing = CardInstance::new(CardId::new(101), BLOOD_FOR_BLOOD_PLUS_ID);
        overflowing.blood_for_blood_cost_reduction = i32::MAX;
        state.piles.draw_pile = vec![overflowing];
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        let before = state.clone();

        assert_eq!(
            apply_player_hp_loss_hooks(&mut state, 1),
            Err(SimError::InvalidState(
                "Blood for Blood cost reduction overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn self_forming_clay_overflow_rolls_back_earlier_hp_loss_triggers() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::SelfFormingClay);
        state.relic_counters.self_forming_clay_next_turn_block = i32::MAX;
        state.piles.hand = vec![CardInstance::new(CardId::new(100), BLOOD_FOR_BLOOD_ID)];
        let before = state.clone();

        assert_eq!(
            apply_player_hp_loss_hooks(&mut state, 1),
            Err(SimError::InvalidState(
                "Self-Forming Clay block accumulation overflows i32"
            ))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn rupture_overflow_rolls_back_relic_draws_and_card_reductions() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.strength = i32::MAX;
        state.player.powers.rupture = 1;
        state.relics.push(Relic::CentennialPuzzle);
        state.piles.hand = vec![CardInstance::new(CardId::new(100), BLOOD_FOR_BLOOD_ID)];
        state.piles.draw_pile = vec![CardInstance::new(
            CardId::new(101),
            crate::content::cards::DEFEND_R_ID,
        )];
        let before = state.clone();

        assert_eq!(
            apply_player_card_hp_loss_hooks(&mut state, 1),
            Err(SimError::InvalidState(
                "Rupture Strength gain overflows i32"
            ))
        );
        assert_eq!(state, before);
    }
}
