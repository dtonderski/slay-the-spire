use crate::{
    card::CardInstance,
    combat::CardPiles,
    content::cards::{get_card_definition, is_basic_starter_card},
    relic::{Relic, SNECKO_EYE_DRAW},
    rng::{JavaRng, StsRng},
    ContentId,
};

const OPENING_HAND_SIZE: usize = 5;

#[must_use]
pub fn card_has_innate(content_id: ContentId) -> bool {
    get_card_definition(content_id)
        .map(|definition| definition.keywords.innate)
        .unwrap_or(false)
}

#[must_use]
pub fn card_starts_in_opening_hand(card: &CardInstance) -> bool {
    card.bottled || card_has_innate(card.content_id)
}

#[must_use]
pub fn order_deck_for_combat_shuffle(deck: &[CardInstance]) -> Vec<CardInstance> {
    deck.to_vec()
}

#[must_use]
pub fn initialize_combat_piles(deck: &[CardInstance], shuffle_rng: &mut StsRng) -> CardPiles {
    let mut card_random_rng = None;
    initialize_combat_piles_with_relics(deck, shuffle_rng, &mut card_random_rng, &[])
}

#[must_use]
pub fn initialize_combat_piles_with_relics(
    deck: &[CardInstance],
    shuffle_rng: &mut StsRng,
    card_random_rng: &mut Option<StsRng>,
    relics: &[Relic],
) -> CardPiles {
    let mut shuffled = order_deck_for_combat_shuffle(deck);
    JavaRng::new(shuffle_rng.random_long()).collections_shuffle(&mut shuffled);

    let mut draw_pile: Vec<_> = shuffled
        .iter()
        .filter(|card| !card_starts_in_opening_hand(card))
        .cloned()
        .collect();
    draw_pile.extend(shuffled.into_iter().filter(card_starts_in_opening_hand));

    let mut hand = Vec::new();

    let draw_count = opening_hand_size(relics);
    let split_at = draw_pile.len().saturating_sub(draw_count);
    let mut opening_draw = draw_pile.split_off(split_at);
    opening_draw.reverse();

    for mut card in opening_draw {
        if relics.contains(&Relic::SneckoEye)
            && get_card_definition(card.content_id)
                .is_some_and(|definition| !definition.keywords.unplayable)
        {
            if let Some(rng) = card_random_rng.as_mut() {
                card.temp_cost = Some(rng.random_int(3) as u8);
            }
        }
        hand.push(card);
    }

    CardPiles {
        hand,
        draw_pile,
        discard_pile: Vec::new(),
        exhaust_pile: Vec::new(),
    }
}

fn opening_hand_size(relics: &[Relic]) -> usize {
    OPENING_HAND_SIZE
        + if relics.contains(&Relic::SneckoEye) {
            SNECKO_EYE_DRAW
        } else {
            0
        }
}

#[must_use]
pub fn starter_only_deck(deck: &[CardInstance]) -> bool {
    deck.iter()
        .all(|card| is_basic_starter_card(card.content_id))
}
