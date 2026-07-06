use crate::{
    combat::{transition::apply_on_exhaust_effects, CombatState},
    content::cards::{
        get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, DECAY_ID, DOUBT_ID, REGRET_ID, SHAME_ID,
    },
    ids::CardId,
};

pub fn resolve_end_of_turn_hand(state: &mut CombatState) {
    let hand_size_for_regret = state.piles.hand.len() as i32;
    apply_end_of_turn_for_playing_cards_in_hand_order(state, hand_size_for_regret);
    exhaust_unplayed_ethereal_cards(state);
}

pub(crate) fn resolve_end_of_turn_doubt(state: &mut CombatState) {
    apply_doubt_weak_in_hand(state);
    apply_shame_frail_in_hand(state);
}

pub(crate) fn discard_end_of_turn_hand(state: &mut CombatState) {
    discard_non_retain_hand(state);
}

fn apply_end_of_turn_for_playing_cards_in_hand_order(state: &mut CombatState, hand_size: i32) {
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
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
                state.piles.discard_pile.push(card);
            }
            DECAY_ID => {
                let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, 2);
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
                state.piles.discard_pile.push(card);
            }
            REGRET_ID => {
                let hp_loss = crate::combat::hp_loss::lose_player_hp(state, hand_size);
                crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
                state.piles.discard_pile.push(card);
            }
            _ => remaining.push(card),
        }
    }
    state.piles.hand = remaining;
}

fn apply_doubt_weak_in_hand(state: &mut CombatState) {
    let mut doubt_copies = 0;
    let mut remaining = Vec::with_capacity(state.piles.hand.len());
    let mut doubts = Vec::new();

    for card in state.piles.hand.drain(..) {
        if card.content_id == DOUBT_ID {
            doubt_copies += 1;
            doubts.push(card);
        } else {
            remaining.push(card);
        }
    }
    state.piles.hand = remaining;

    if doubt_copies > 0 {
        crate::relic::apply_player_weak_with_relics(
            &mut state.player.powers,
            &state.relics,
            doubt_copies,
        );
        if state.relics.contains(&crate::Relic::RunicPyramid) {
            state.piles.hand.extend(doubts);
        } else {
            state.piles.discard_pile.extend(doubts);
        }
    }
}

fn apply_shame_frail_in_hand(state: &mut CombatState) {
    let mut shame_copies = 0;
    let mut remaining = Vec::with_capacity(state.piles.hand.len());
    let mut shames = Vec::new();

    for card in state.piles.hand.drain(..) {
        if card.content_id == SHAME_ID {
            shame_copies += 1;
            shames.push(card);
        } else {
            remaining.push(card);
        }
    }
    state.piles.hand = remaining;

    if shame_copies > 0 {
        crate::relic::apply_player_frail_with_relics(
            &mut state.player.powers,
            &state.relics,
            shame_copies,
        );
        state.piles.discard_pile.extend(shames);
    }
}

fn exhaust_unplayed_ethereal_cards(state: &mut CombatState) {
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
            apply_on_exhaust_effects(state, card_id);
        }
    }
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
    fn end_turn_pseudo_play_cards_discard_in_hand_order() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand = vec![
            CardInstance::new(CardId::new(1), DEFEND_R_ID),
            CardInstance::new(CardId::new(2), REGRET_ID),
            CardInstance::new(CardId::new(3), BURN_ID),
        ];
        state.piles.discard_pile.clear();

        resolve_end_of_turn_hand(&mut state);

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
            vec![REGRET_ID, BURN_ID]
        );
    }
}
