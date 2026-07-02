use crate::{
    combat::{transition::apply_on_exhaust_effects, CombatState},
    content::cards::{get_card_definition, BURN_END_TURN_DAMAGE, BURN_ID, DOUBT_ID, REGRET_ID},
    ids::CardId,
};

pub fn resolve_end_of_turn_hand(state: &mut CombatState) {
    let hand_size_for_regret = state.piles.hand.len() as i32;
    apply_burn_damage_in_hand(state);
    apply_regret_damage_in_hand(state, hand_size_for_regret);
    exhaust_unplayed_ethereal_cards(state);
}

pub(crate) fn resolve_end_of_turn_doubt(state: &mut CombatState) {
    apply_doubt_weak_in_hand(state);
}

pub(crate) fn discard_end_of_turn_hand(state: &mut CombatState) {
    let stable_discard_order_visible =
        state.piles.draw_pile.len() >= crate::combat::turn::target_hand_size(state);
    discard_non_retain_hand(state, stable_discard_order_visible);
}

fn apply_burn_damage_in_hand(state: &mut CombatState) {
    let mut remaining = Vec::with_capacity(state.piles.hand.len());
    let mut burns = Vec::new();

    for card in state.piles.hand.drain(..) {
        if card.content_id == BURN_ID {
            burns.push(card);
        } else {
            remaining.push(card);
        }
    }
    state.piles.hand = remaining;

    let burn_damage = burns
        .iter()
        .map(|card| {
            if card.upgrades > 0 {
                BURN_END_TURN_DAMAGE * 2
            } else {
                BURN_END_TURN_DAMAGE
            }
        })
        .sum();

    let hp_loss = crate::combat::hp_loss::lose_player_blockable_hp(state, burn_damage);
    crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
    state.piles.discard_pile.extend(burns);
}

fn apply_regret_damage_in_hand(state: &mut CombatState, hand_size: i32) {
    let mut remaining = Vec::with_capacity(state.piles.hand.len());
    let mut regrets = Vec::new();

    for card in state.piles.hand.drain(..) {
        if card.content_id == REGRET_ID {
            regrets.push(card);
        } else {
            remaining.push(card);
        }
    }
    state.piles.hand = remaining;

    for _ in &regrets {
        let hp_loss = crate::combat::hp_loss::lose_player_hp(state, hand_size);
        crate::combat::hp_loss::apply_player_hp_loss_hooks(state, hp_loss);
    }
    state.piles.discard_pile.extend(regrets);
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

fn discard_non_retain_hand(state: &mut CombatState, stable_discard_order_visible: bool) {
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

    if stable_discard_order_visible {
        discarded.reverse();
    }
    state.piles.hand = retained;
    state.piles.discard_pile.extend(discarded);
}
