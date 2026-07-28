use crate::{
    card::CardInstance,
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
    let pending_hand: &mut [CardInstance] = &mut [];
    apply_player_card_hp_loss_hooks_with_pending_hand(state, hp_loss, pending_hand)
}

pub(crate) fn apply_player_card_hp_loss_hooks_with_pending_hand(
    state: &mut CombatState,
    hp_loss: i32,
    pending_hand: &mut [CardInstance],
) -> SimResult<()> {
    if hp_loss <= 0 {
        return Ok(());
    }

    let mut next = state.clone();
    let mut next_pending_hand = pending_hand.to_vec();
    apply_player_hp_loss_hooks_in_place(&mut next, hp_loss)?;
    reduce_blood_for_blood_costs_in_cards(&mut next_pending_hand)?;
    next.player.powers.strength = next
        .player
        .powers
        .strength
        .checked_add(next.player.powers.rupture)
        .ok_or(crate::SimError::InvalidState(
            "Rupture Strength gain overflows i32",
        ))?;
    *state = next;
    pending_hand.copy_from_slice(&next_pending_hand);
    Ok(())
}

fn apply_player_hp_loss_hooks_in_place(state: &mut CombatState, hp_loss: i32) -> SimResult<()> {
    state.player.damage_events_this_combat = state
        .player
        .damage_events_this_combat
        .checked_add(1)
        .ok_or(crate::SimError::InvalidState(
            "player damage event counter overflows i32",
        ))?;
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
        reduce_blood_for_blood_costs_in_cards(pile)?;
    }
    Ok(())
}

fn reduce_blood_for_blood_costs_in_cards(cards: &mut [CardInstance]) -> SimResult<()> {
    for card in cards {
        if card.content_id == BLOOD_FOR_BLOOD_ID || card.content_id == BLOOD_FOR_BLOOD_PLUS_ID {
            card.blood_for_blood_cost_reduction =
                card.blood_for_blood_cost_reduction.checked_add(1).ok_or(
                    crate::SimError::InvalidState("Blood for Blood cost reduction overflows i32"),
                )?;
            // BloodForBlood.tookDamage calls AbstractCard.updateCost(-1).
            // updateCost preserves the difference between the printed cost
            // and costForTurn, so a card with a temporary current-turn
            // cost must lower that value along with its combat reduction.
            if let Some(temp_cost) = card.temp_cost.as_mut() {
                *temp_cost = temp_cost.saturating_sub(1);
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
    fn blood_for_blood_damage_reduces_a_temporary_current_turn_cost() {
        let mut state = CombatState::initial_fixture();
        let mut card = CardInstance::new(CardId::new(100), BLOOD_FOR_BLOOD_ID);
        card.temp_cost = Some(3);
        state.piles.hand = vec![card];

        apply_player_hp_loss_hooks(&mut state, 1).expect("Blood for Blood reduction succeeds");
        apply_player_hp_loss_hooks(&mut state, 1).expect("Blood for Blood reduction succeeds");

        assert_eq!(state.piles.hand[0].blood_for_blood_cost_reduction, 2);
        assert_eq!(state.piles.hand[0].temp_cost, Some(1));
        assert_eq!(
            crate::combat::cost::effective_card_cost(&state.piles.hand[0]),
            Ok(1)
        );
    }

    #[test]
    fn runic_cube_does_not_draw_on_lethal_hp_loss() {
        // RunicCube.onLoseHp queues DrawCardAction; lethal damage cancels the bot.
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::RunicCube);
        state.player.hp = 3;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(CardId::new(1), crate::content::cards::STRIKE_R_ID),
            CardInstance::new(CardId::new(2), crate::content::cards::BASH_ID),
        ];
        state.piles.discard_pile.clear();

        // Simulate post-damage hooks after HP has already been reduced to 0.
        state.player.hp = 0;
        apply_player_hp_loss_hooks(&mut state, 18).expect("lethal Runic Cube hooks succeed");

        assert!(
            state.piles.hand.is_empty(),
            "lethal Runic Cube must not draw: hand={:?}",
            state.piles.hand
        );
        assert_eq!(state.piles.draw_pile.len(), 2);
    }

    #[test]
    fn runic_cube_draws_on_nonlethal_hp_loss() {
        let mut state = CombatState::initial_fixture();
        state.relics.push(Relic::RunicCube);
        state.player.hp = 10;
        state.piles.hand.clear();
        state.piles.draw_pile = vec![CardInstance::new(
            CardId::new(1),
            crate::content::cards::STRIKE_R_ID,
        )];

        state.player.hp = 7;
        apply_player_hp_loss_hooks(&mut state, 3).expect("nonlethal Runic Cube draws");

        assert_eq!(state.piles.hand.len(), 1);
        assert!(state.piles.draw_pile.is_empty());
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
