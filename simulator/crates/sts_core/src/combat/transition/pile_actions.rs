use super::{
    add_card_to_pile, add_generated_card_to_draw_pile_random_spot, add_generated_card_to_pile,
    add_stat_equivalent_copy_to_pile, apply_unceasing_top_after_hand_emptied,
    draw_random_attacks_from_draw_pile, find_hand_card, hand_contains_attack, move_card,
    move_forethought_card_to_draw_bottom, player_deep_breath_shuffle_discard_into_draw,
    player_draw_cards, player_draw_cards_with_deferred_evolve, player_draw_cards_without_evolve,
    player_shuffle_discard_into_draw, random_colorless_card, random_hand_card_id_except,
    remove_card_from_pile,
};
use crate::{
    action::{CardPile, InternalAction},
    card::CardType,
    combat::CombatState,
    content::cards::get_card_definition,
    ids::{CardId, ContentId},
    CardInstance, SimError, SimResult,
};

pub(super) fn move_card_between_piles(
    state: &mut CombatState,
    card_id: CardId,
    from: CardPile,
    to: CardPile,
) -> SimResult<Vec<InternalAction>> {
    let hand_exhaust_is_attack = if from == CardPile::Hand && to == CardPile::ExhaustPile {
        find_hand_card(state, card_id)
            .ok()
            .and_then(|card| get_card_definition(card.content_id))
            .is_some_and(|definition| definition.card_type == CardType::Attack)
    } else {
        false
    };
    move_card(state, card_id, from, to)?;
    let mut follow_ups = Vec::new();
    if from == CardPile::Hand && state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    if to == CardPile::ExhaustPile {
        if hand_exhaust_is_attack {
            follow_ups.push(InternalAction::HandCardExhausted { card_id });
        } else {
            follow_ups.push(InternalAction::CardExhausted { card_id });
        }
    }
    Ok(follow_ups)
}

pub(super) fn return_exhaust_card_to_hand(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let card = remove_card_from_pile(state, card_id, CardPile::ExhaustPile)?;
    state.piles.hand.push(card);
    Ok(Vec::new())
}

pub(super) fn forethought_auto_move(
    state: &mut CombatState,
    source_card_id: CardId,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    move_forethought_card_to_draw_bottom(state, source_card_id, card_id)?;
    Ok(Vec::new())
}

pub(super) fn exhaust_random_hand_card_except(
    state: &mut CombatState,
    excluded_card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    let Some(card_id) = random_hand_card_id_except(state, excluded_card_id) else {
        return Ok(Vec::new());
    };
    move_card(state, card_id, CardPile::Hand, CardPile::ExhaustPile)?;
    if state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    Ok(vec![InternalAction::CardExhausted { card_id }])
}

pub(super) fn remove_card(
    state: &mut CombatState,
    card_id: CardId,
    from: CardPile,
) -> SimResult<Vec<InternalAction>> {
    remove_card_from_pile(state, card_id, from)?;
    if from == CardPile::Hand && state.piles.hand.is_empty() {
        apply_unceasing_top_after_hand_emptied(state)?;
    }
    Ok(Vec::new())
}

pub(super) fn add_card(
    state: &mut CombatState,
    content_id: ContentId,
    to: CardPile,
) -> SimResult<Vec<InternalAction>> {
    add_card_to_pile(state, content_id, to)?;
    Ok(Vec::new())
}

pub(super) fn add_generated_card(
    state: &mut CombatState,
    content_id: ContentId,
    to: CardPile,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<Vec<InternalAction>> {
    add_generated_card_to_pile(state, content_id, to, temp_cost, temp_cost_turn_only)?;
    Ok(Vec::new())
}

pub(super) fn add_generated_cards_to_hand_while_source_in_limbo(
    state: &mut CombatState,
    content_id: ContentId,
    source_card_id: CardId,
    count: usize,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<Vec<InternalAction>> {
    let source_index = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == source_card_id)
        .ok_or(SimError::UnknownCard(source_card_id))?;
    let source = state.piles.hand.remove(source_index);
    let result = (0..count).try_for_each(|_| {
        add_generated_card_to_pile(
            state,
            content_id,
            CardPile::Hand,
            temp_cost,
            temp_cost_turn_only,
        )
    });
    state
        .piles
        .hand
        .insert(source_index.min(state.piles.hand.len()), source);
    result?;
    Ok(Vec::new())
}

pub(super) fn add_generated_hand_card_before_pending_draw(
    state: &mut CombatState,
    content_id: ContentId,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<Vec<InternalAction>> {
    add_generated_card_to_pile(
        state,
        content_id,
        CardPile::Hand,
        temp_cost,
        temp_cost_turn_only,
    )?;
    Ok(Vec::new())
}

pub(super) fn add_stat_equivalent_copy(
    state: &mut CombatState,
    card: CardInstance,
    to: CardPile,
) -> SimResult<Vec<InternalAction>> {
    add_stat_equivalent_copy_to_pile(state, card, to)?;
    Ok(Vec::new())
}

pub(super) fn add_card_instance_to_hand_or_discard(
    state: &mut CombatState,
    card: CardInstance,
) -> SimResult<Vec<InternalAction>> {
    if state.piles.hand.len() < crate::combat::draw::MAX_HAND_SIZE {
        state.piles.hand.push(card);
    } else {
        state.piles.discard_pile.push(card);
    }
    Ok(Vec::new())
}

pub(super) fn add_generated_card_to_random_draw_spot(
    state: &mut CombatState,
    content_id: ContentId,
    temp_cost: Option<u8>,
    temp_cost_turn_only: bool,
) -> SimResult<Vec<InternalAction>> {
    add_generated_card_to_draw_pile_random_spot(state, content_id, temp_cost, temp_cost_turn_only)?;
    Ok(Vec::new())
}

pub(super) fn add_random_colorless_card_to_hand(
    state: &mut CombatState,
    temp_cost: Option<u8>,
    upgrade: bool,
) -> SimResult<Vec<InternalAction>> {
    state.reserve_card_instance_ids(1)?;
    let content_id = random_colorless_card(state, upgrade)?;
    add_generated_card_to_pile(
        state,
        content_id,
        CardPile::Hand,
        temp_cost,
        temp_cost.is_some(),
    )?;
    Ok(Vec::new())
}

pub(super) fn draw_cards(state: &mut CombatState, count: usize) -> SimResult<Vec<InternalAction>> {
    let deferred_evolve_draws = player_draw_cards_with_deferred_evolve(state, count)?;
    Ok(deferred_evolve_draws
        .into_iter()
        .map(|count| InternalAction::DrawCards { count })
        .collect())
}

pub(super) fn draw_cards_without_evolve(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    player_draw_cards_without_evolve(state, count)?;
    Ok(Vec::new())
}

pub(super) fn draw_cards_while_played_card_is_in_limbo(
    state: &mut CombatState,
    card_id: CardId,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    draw_cards_while_played_card_is_in_limbo_with_mode(state, card_id, count, true)
}

pub(super) fn draw_cards_while_played_card_is_in_limbo_without_evolve(
    state: &mut CombatState,
    card_id: CardId,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    draw_cards_while_played_card_is_in_limbo_with_mode(state, card_id, count, false)
}

fn draw_cards_while_played_card_is_in_limbo_with_mode(
    state: &mut CombatState,
    card_id: CardId,
    count: usize,
    trigger_evolve: bool,
) -> SimResult<Vec<InternalAction>> {
    let played_card = match state.piles.hand.iter().position(|card| card.id == card_id) {
        Some(hand_index) => Some(state.piles.hand.remove(hand_index)),
        None if state
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == card_id) =>
        {
            None
        }
        None => {
            return Err(SimError::IllegalAction(
                "played card is not in hand or discard",
            ));
        }
    };
    if trigger_evolve {
        player_draw_cards(state, count)?;
    } else {
        player_draw_cards_without_evolve(state, count)?;
    }
    if let Some(played_card) = played_card {
        // Keep the source in hand after the limbo draw so later card effects
        // (Battle Trance No Draw, Corruption exhaust via MoveCard rewrite,
        // Havoc force-exhaust placement) settle after those effects. Returning
        // the card here mirrors limbo occupying a non-hand slot only for the
        // draw itself, then UseCardAction finishing afterward.
        state.piles.hand.push(played_card);
    }
    Ok(Vec::new())
}

pub(super) fn shuffle_discard_into_draw(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    player_shuffle_discard_into_draw(state)?;
    Ok(Vec::new())
}

pub(super) fn deep_breath_shuffle_discard_into_draw(
    state: &mut CombatState,
) -> SimResult<Vec<InternalAction>> {
    player_deep_breath_shuffle_discard_into_draw(state)?;
    Ok(Vec::new())
}

pub(super) fn draw_cards_if_no_attacks_in_hand(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    if !hand_contains_attack(state) {
        player_draw_cards(state, count)?;
    }
    Ok(Vec::new())
}

pub(super) fn draw_random_attacks(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    draw_random_attacks_from_draw_pile(state, count);
    Ok(Vec::new())
}
