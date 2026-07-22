use crate::{
    combat::{transition::apply_on_exhaust_effects, CombatState},
    content::cards::{
        get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, DECAY_ID, DOUBT_ID, REGRET_ID, SHAME_ID,
    },
    ids::CardId,
    SimResult,
};

pub fn resolve_end_of_turn_hand(state: &mut CombatState) -> SimResult<()> {
    let mut next = state.clone();
    resolve_end_of_turn_hand_inner(&mut next)?;
    *state = next;
    Ok(())
}

fn resolve_end_of_turn_hand_inner(state: &mut CombatState) -> SimResult<()> {
    let hand_size_for_regret = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(state, hand_size_for_regret)?;
    exhaust_unplayed_ethereal_cards(state)
}

pub(crate) fn discard_end_of_turn_hand(state: &mut CombatState) {
    discard_non_retain_hand(state);
}

fn apply_end_of_turn_for_playing_cards_in_hand_order(
    state: &mut CombatState,
    hand_size: i32,
) -> SimResult<()> {
    let mut remaining = Vec::with_capacity(state.piles.hand.len());
    let hand = std::mem::take(&mut state.piles.hand);

    for card in hand {
        match card.content_id {
            BURN_ID => {
                let burn_damage = if card.upgrades > 0 {
                    BURN_END_TURN_DAMAGE * 2
                } else {
                    BURN_END_TURN_DAMAGE
                };
                let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, burn_damage);
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
                state.piles.discard_pile.push(card);
            }
            DECAY_ID => {
                let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, 2);
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
                state.piles.discard_pile.push(card);
            }
            REGRET_ID => {
                let hp_loss = crate::combat::hp_loss::lose_player_hp(state, hand_size);
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss)?;
                state.piles.discard_pile.push(card);
            }
            DOUBT_ID => {
                if state.relics.contains(&crate::Relic::RunicPyramid) {
                    crate::relic::apply_player_weak_with_relics(
                        &mut state.player.powers,
                        &state.relics,
                        1,
                    );
                    remaining.push(card);
                } else {
                    crate::relic::apply_player_weak_with_relics(
                        &mut state.player.powers,
                        &state.relics,
                        1,
                    );
                    state.piles.discard_pile.push(card);
                }
            }
            SHAME_ID => {
                crate::relic::apply_player_frail_with_relics(
                    &mut state.player.powers,
                    &state.relics,
                    1,
                );
                state.piles.discard_pile.push(card);
            }
            _ => remaining.push(card),
        }
    }
    state.piles.hand = remaining;
    Ok(())
}

fn exhaust_unplayed_ethereal_cards(state: &mut CombatState) -> SimResult<()> {
    let ethereal_ids: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.keywords.ethereal)
        })
        .map(|card| card.id)
        .collect();

    for card_id in ethereal_ids {
        if let Some(index) = state.piles.hand.iter().position(|card| card.id == card_id) {
            let card = state.piles.hand.remove(index);
            state.piles.exhaust_pile.push(card);
            apply_on_exhaust_effects(state, card_id)?;
        }
    }
    Ok(())
}

fn discard_non_retain_hand(state: &mut CombatState) {
    if state.relics.contains(&crate::Relic::RunicPyramid) {
        return;
    }

    let mut retained = Vec::new();
    let mut discarded = Vec::new();

    for card in state.piles.hand.drain(..) {
        if get_card_definition(card.content_id).is_some_and(|definition| definition.keywords.retain)
        {
            retained.push(card);
        } else {
            discarded.push(card);
        }
    }

    discarded.reverse();
    state.piles.hand = retained;
    state.piles.discard_pile.extend(discarded);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::DEFEND_R_ID, ids::CardId, CardInstance};

    #[test]
    fn end_turn_trigger_cards_discard_in_hand_order() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(2), REGRET_ID),
            CardInstance::new(CardId::new(3), BURN_ID),
            CardInstance::new(CardId::new(4), DOUBT_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state).expect("end-turn hand resolves");

        assert_eq!(
            state
                .piles
                .hand
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![DEFEND_R_ID]
        );
        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![REGRET_ID, BURN_ID, DOUBT_ID]
        );
        assert_eq!(state.player.powers.weak, 1);
    }

    #[test]
    fn end_turn_doubt_and_shame_preserve_hand_order_in_discard() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), SHAME_ID),
            CardInstance::new(CardId::new(2), DOUBT_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state).expect("end-turn hand resolves");

        assert_eq!(
            state
                .piles
                .discard_pile
                .iter()
                .map(|card| card.content_id)
                .collect::<Vec<_>>(),
            vec![SHAME_ID, DOUBT_ID]
        );
        assert_eq!(state.player.powers.weak, 1);
        assert_eq!(state.player.powers.frail, 1);
    }
}
