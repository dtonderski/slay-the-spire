use super::{
    add_card_to_pile, add_generated_card_to_draw_pile_random_spot, add_generated_card_to_pile,
    add_stat_equivalent_copy_to_pile, apply_unceasing_top_after_hand_emptied,
    draw_random_attacks_from_draw_pile, find_hand_card, hand_contains_attack, move_card,
    move_forethought_card_to_draw_bottom, player_draw_cards,
    player_draw_cards_with_deferred_evolve, player_draw_cards_without_evolve,
    random_colorless_card, random_hand_card_id_except, remove_card_from_pile,
};
use crate::{
    action::{CardPile, InternalAction},
    card::CardType,
    combat::CombatState,
    content::cards::get_card_definition,
    ids::{CardId, ContentId},
    CardInstance, SimError, SimResult,
};

pub(super) fn exhaust_all_non_attack_cards(
    state: &mut CombatState,
    excluded_card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    // ExhaustAllNonAttackAction snapshots the hand once. Soulbound
    // replacements created by triggerOnExhaust are not in that snapshot; a
    // Necronomicon / Double Tap copy snapshots them on the second use().
    let targets: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| card.id != excluded_card_id)
        .filter(|card| {
            get_card_definition(card.content_id)
                .is_some_and(|definition| definition.card_type != CardType::Attack)
        })
        .map(|card| card.id)
        .collect();
    let mut follow_ups = Vec::new();
    for card_id in targets {
        follow_ups.extend(move_card_between_piles(
            state,
            card_id,
            CardPile::Hand,
            CardPile::ExhaustPile,
        )?);
    }
    Ok(follow_ups)
}

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
    let hand_emptied = from == CardPile::Hand && state.piles.hand.is_empty();
    if to == CardPile::ExhaustPile {
        if hand_exhaust_is_attack {
            follow_ups.push(InternalAction::HandCardExhausted { card_id });
        } else {
            follow_ups.push(InternalAction::CardExhausted { card_id });
        }
    }
    // Unceasing Top is an addToBot-style follow-up after the exhaust callback
    // queue. Dead Branch/Feel No Pain/Dark Embrace must get the first chance to
    // refill the hand, otherwise the immediate draw steals the source card's
    // next RNG/card-order slot when the last hand card exhausts.
    if hand_emptied && state.relics.contains(&crate::relic::Relic::UnceasingTop) {
        follow_ups.push(InternalAction::UnceasingTopDraw);
    }
    Ok(follow_ups)
}

pub(super) fn discard_to_hand(
    state: &mut CombatState,
    card_id: CardId,
) -> SimResult<Vec<InternalAction>> {
    // DiscardToHandAction: if the card is still in discard and hand.size() < 10,
    // addToHand then remove from discard. The played source is already out of
    // the target hand (useCard removes it before ChangeStance), so occupancy
    // must ignore card_in_use.
    let Some(index) = state
        .piles
        .discard_pile
        .iter()
        .position(|card| card.id == card_id)
    else {
        return Ok(Vec::new());
    };
    let occupancy = state
        .piles
        .hand
        .iter()
        .filter(|card| Some(card.id) != state.card_in_use)
        .count();
    if occupancy >= crate::combat::draw::MAX_HAND_SIZE {
        return Ok(Vec::new());
    }
    let card = state.piles.discard_pile.remove(index);
    state.piles.hand.push(card);
    Ok(Vec::new())
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
    // Keep the source on limbo so ID allocation cannot collide with it.
    let source = state.piles.hand.remove(source_index);
    state.piles.limbo.push(source);
    let result = (0..count).try_for_each(|_| {
        add_generated_card_to_pile(
            state,
            content_id,
            CardPile::Hand,
            temp_cost,
            temp_cost_turn_only,
        )
    });
    let source = state.piles.limbo.pop().ok_or(SimError::InvalidState(
        "limbo source card missing after generate",
    ))?;
    if source.id != source_card_id {
        return Err(SimError::InvalidState("limbo source card id mismatch"));
    }
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

pub(super) fn add_random_colorless_cards_to_hand_while_source_in_limbo(
    state: &mut CombatState,
    source_card_id: CardId,
    count: usize,
    temp_cost: Option<u8>,
    upgrade: bool,
) -> SimResult<Vec<InternalAction>> {
    let source_index = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == source_card_id)
        .ok_or(SimError::UnknownCard(source_card_id))?;
    // Park in limbo (not a bare local) so instance-ID allocation cannot reuse
    // the source id while it is off the hand.
    let source = state.piles.hand.remove(source_index);
    state.piles.limbo.push(source);
    let result = (0..count).try_for_each(|_| {
        state.reserve_card_instance_ids(1)?;
        let content_id = random_colorless_card(state, upgrade)?;
        add_generated_card_to_pile(
            state,
            content_id,
            CardPile::Hand,
            temp_cost,
            temp_cost.is_some(),
        )
    });
    let source = state.piles.limbo.pop().ok_or(SimError::InvalidState(
        "Transmutation source missing from limbo",
    ))?;
    if source.id != source_card_id {
        return Err(SimError::InvalidState(
            "Transmutation limbo source id mismatch",
        ));
    }
    state
        .piles
        .hand
        .insert(source_index.min(state.piles.hand.len()), source);
    result?;
    Ok(Vec::new())
}

pub(super) fn draw_cards(state: &mut CombatState, count: usize) -> SimResult<Vec<InternalAction>> {
    player_draw_cards_with_deferred_evolve(state, count)
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
    // EvolvePower.onCardDraw addToBot's DrawCardAction, so status-triggered
    // draws must wait until after this DrawCardAction and the later
    // UseCardAction MoveCard (discard/exhaust). Inline evolve here reshuffled
    // without the played card still in limbo, desyncing discard/draw order
    // after Reckless Charge → Dazed → Shrug It Off + Evolve (permanent
    // random-fidelity-0667712a2814e2cf step 646/648).
    let follow_ups = if trigger_evolve {
        player_draw_cards_with_deferred_evolve(state, count)?
    } else {
        player_draw_cards_without_evolve(state, count)?;
        Vec::new()
    };
    if let Some(played_card) = played_card {
        // Keep the source in hand after the limbo draw so later card effects
        // (Battle Trance No Draw, Corruption exhaust via MoveCard rewrite,
        // Havoc force-exhaust placement) settle after those effects. Returning
        // the card here mirrors limbo occupying a non-hand slot only for the
        // draw itself, then UseCardAction finishing afterward.
        state.piles.hand.push(played_card);
    }
    Ok(follow_ups)
}

pub(super) fn shuffle_discard_into_draw(state: &mut CombatState) -> SimResult<Vec<InternalAction>> {
    crate::combat::draw::shuffle_discard_into_draw_with_combat_rng(state)
}

pub(super) fn deep_breath_shuffle_discard_into_draw(
    state: &mut CombatState,
) -> SimResult<Vec<InternalAction>> {
    crate::combat::draw::deep_breath_shuffle_discard_into_draw_with_combat_rng(state)
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

pub(super) fn draw_then_scrape_discard(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    let before: Vec<CardId> = state.piles.hand.iter().map(|card| card.id).collect();
    let mut follow_ups = draw_cards(state, count)?;
    let newly_drawn: Vec<CardId> = state
        .piles
        .hand
        .iter()
        .filter(|card| !before.contains(&card.id))
        .map(|card| card.id)
        .collect();
    for card_id in newly_drawn {
        let Some(card) = state.piles.hand.iter().find(|card| card.id == card_id) else {
            continue;
        };
        // Unplayable curses/statuses use costForTurn -2 in the target, but our
        // definitions store cost 0 + unplayable. ScrapeFollowUp discards those.
        let unplayable = get_card_definition(card.content_id)
            .is_some_and(|definition| definition.keywords.unplayable);
        let cost = crate::combat::cost::effective_card_cost(card).unwrap_or(0);
        if card.free_to_play_once || (cost == 0 && !unplayable) {
            continue;
        }
        follow_ups.extend(move_card_between_piles(
            state,
            card_id,
            CardPile::Hand,
            CardPile::DiscardPile,
        )?);
    }
    Ok(follow_ups)
}

pub(super) fn draw_random_attacks(
    state: &mut CombatState,
    count: usize,
) -> SimResult<Vec<InternalAction>> {
    draw_random_attacks_from_draw_pile(state, count);
    Ok(Vec::new())
}
