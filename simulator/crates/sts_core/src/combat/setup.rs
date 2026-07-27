use crate::{
    card::CardInstance,
    combat::CardPiles,
    content::cards::{get_card_definition, is_basic_starter_card},
    relic::{Relic, SNECKO_EYE_DRAW},
    rng::{JavaRng, StsRng},
    ContentId, SimError, SimResult,
};

const OPENING_HAND_SIZE: usize = 5;

pub fn card_has_innate(content_id: ContentId) -> SimResult<bool> {
    get_card_definition(content_id)
        .map(|definition| definition.keywords.innate)
        .ok_or(SimError::UnknownContent(content_id))
}

pub fn card_starts_in_opening_hand(card: &CardInstance) -> SimResult<bool> {
    let innate = card_has_innate(card.content_id)?;
    Ok(card.bottled || innate)
}

#[must_use]
pub fn order_deck_for_combat_shuffle(deck: &[CardInstance]) -> Vec<CardInstance> {
    deck.to_vec()
}

pub fn initialize_combat_piles_with_relics(
    deck: &[CardInstance],
    shuffle_rng: &mut StsRng,
    card_random_rng: &mut StsRng,
    relics: &[Relic],
) -> SimResult<CardPiles> {
    let shuffled = order_deck_for_combat_shuffle(deck);
    let mut prepared = Vec::with_capacity(shuffled.len());
    for card in shuffled {
        let definition = get_card_definition(card.content_id)
            .ok_or(SimError::UnknownContent(card.content_id))?;
        prepared.push((
            card,
            card.bottled || definition.keywords.innate,
            definition.keywords.unplayable,
        ));
    }

    JavaRng::new(shuffle_rng.random_long()).collections_shuffle(&mut prepared);

    let mut draw_pile = Vec::with_capacity(prepared.len());
    let mut opening_cards = Vec::new();
    for (card, starts_in_opening_hand, unplayable) in prepared {
        if starts_in_opening_hand {
            opening_cards.push((card, unplayable));
        } else {
            draw_pile.push((card, unplayable));
        }
    }
    draw_pile.extend(opening_cards);

    let mut hand = Vec::new();

    let draw_count = opening_hand_size(relics);
    let split_at = draw_pile.len().saturating_sub(draw_count);
    let mut opening_draw = draw_pile.split_off(split_at);
    opening_draw.reverse();

    for (mut card, unplayable) in opening_draw {
        if relics.contains(&Relic::SneckoEye)
            && !unplayable
            && get_card_definition(card.content_id).is_some_and(|definition| definition.cost >= 0)
        {
            card.temp_cost = Some(card_random_rng.random_int(3) as u8);
        }
        hand.push(card);
    }

    Ok(CardPiles {
        hand,
        draw_pile: draw_pile.into_iter().map(|(card, _)| card).collect(),
        discard_pile: Vec::new(),
        exhaust_pile: Vec::new(),
        limbo: Vec::new(),
    })
}

fn opening_hand_size(relics: &[Relic]) -> usize {
    OPENING_HAND_SIZE
        + if relics.contains(&Relic::SneckoEye) {
            SNECKO_EYE_DRAW
        } else {
            0
        }
}

pub fn starter_only_deck(deck: &[CardInstance]) -> SimResult<bool> {
    for card in deck {
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;
    }
    Ok(deck
        .iter()
        .all(|card| is_basic_starter_card(card.content_id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardId, ContentId};

    #[test]
    fn unknown_deck_content_fails_before_combat_setup_rng_advances() {
        let unknown_id = ContentId::new(999_999);
        let deck = vec![CardInstance::new(CardId::new(1), unknown_id)];
        let mut shuffle_rng = StsRng::new(11);
        let mut card_random_rng = StsRng::new(12);
        let shuffle_before = shuffle_rng.clone();
        let card_random_before = card_random_rng.clone();

        assert_eq!(
            initialize_combat_piles_with_relics(
                &deck,
                &mut shuffle_rng,
                &mut card_random_rng,
                &[Relic::SneckoEye],
            ),
            Err(SimError::UnknownContent(unknown_id))
        );
        assert_eq!(shuffle_rng, shuffle_before);
        assert_eq!(card_random_rng, card_random_before);
        assert_eq!(
            card_has_innate(unknown_id),
            Err(SimError::UnknownContent(unknown_id))
        );
        let mut bottled_unknown = deck[0];
        bottled_unknown.bottled = true;
        assert_eq!(
            card_starts_in_opening_hand(&bottled_unknown),
            Err(SimError::UnknownContent(unknown_id))
        );
        assert_eq!(
            starter_only_deck(&deck),
            Err(SimError::UnknownContent(unknown_id))
        );
    }

    #[test]
    fn snecko_eye_leaves_x_cost_opening_cards_unchanged_without_rng_draw() {
        let whirlwind = CardInstance::new(CardId::new(1), crate::content::cards::WHIRLWIND_ID);
        let strike = CardInstance::new(CardId::new(2), crate::content::cards::STRIKE_R_ID);
        let mut shuffle_rng = StsRng::new(11);
        let mut card_random_rng = StsRng::new(12);

        let piles = initialize_combat_piles_with_relics(
            &[whirlwind, strike],
            &mut shuffle_rng,
            &mut card_random_rng,
            &[Relic::SneckoEye],
        )
        .expect("combat piles initialize");

        let x_cost = piles
            .hand
            .iter()
            .find(|card| card.content_id == crate::content::cards::WHIRLWIND_ID)
            .expect("Whirlwind remains in the opening hand");
        let ordinary = piles
            .hand
            .iter()
            .find(|card| card.content_id == crate::content::cards::STRIKE_R_ID)
            .expect("Strike remains in the opening hand");
        assert_eq!(x_cost.temp_cost, None);
        assert!(ordinary.temp_cost.is_some());
        assert_eq!(card_random_rng.counter(), 1);
    }
}
