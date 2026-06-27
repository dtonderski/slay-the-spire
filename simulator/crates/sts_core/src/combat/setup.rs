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

    let mut hand: Vec<_> = shuffled
        .iter()
        .filter(|card| card_starts_in_opening_hand(card))
        .cloned()
        .collect();
    let mut draw_pile: Vec<_> = shuffled
        .into_iter()
        .filter(|card| !card_starts_in_opening_hand(card))
        .collect();

    let draw_count = opening_hand_size(relics).saturating_sub(hand.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::deck::ironclad_starter_deck;

    fn content_keys(cards: &[CardInstance]) -> Vec<&'static str> {
        cards
            .iter()
            .map(|card| {
                get_card_definition(card.content_id)
                    .map(|definition| definition.key)
                    .unwrap_or("unknown")
            })
            .collect()
    }

    #[test]
    fn ironclad_starter_shuffle_order_matches_verify01_seed() {
        let deck = ironclad_starter_deck();
        let mut rng = StsRng::new(1_957_307_888_551 + 1);
        let piles = initialize_combat_piles(&deck, &mut rng);

        assert_eq!(
            content_keys(&piles.hand),
            vec!["Strike_R", "Strike_R", "Defend_R", "Strike_R", "Bash"]
        );
        assert_eq!(
            content_keys(&piles.draw_pile),
            vec!["Defend_R", "Strike_R", "Strike_R", "Defend_R", "Defend_R"]
        );
    }

    #[test]
    fn order_deck_for_combat_shuffle_uses_master_deck_order() {
        let deck = ironclad_starter_deck();
        let ordered = order_deck_for_combat_shuffle(&deck);
        assert_eq!(
            ordered.iter().map(|card| card.id.get()).collect::<Vec<_>>(),
            deck.iter().map(|card| card.id.get()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn dramatic_entrance_is_innate() {
        use crate::content::cards::DRAMATIC_ENTRANCE_ID;
        assert!(card_has_innate(DRAMATIC_ENTRANCE_ID));
    }

    #[test]
    fn bottled_card_starts_in_opening_hand() {
        use crate::content::cards::ANGER_ID;

        let mut deck = ironclad_starter_deck();
        deck.push(CardInstance::new(crate::ids::CardId::new(100), ANGER_ID));
        deck.last_mut().expect("anger").bottled = true;

        let mut rng = StsRng::new(1_957_307_888_551 + 1);
        let piles = initialize_combat_piles(&deck, &mut rng);

        assert!(piles
            .hand
            .iter()
            .any(|card| card.content_id == ANGER_ID && card.bottled));
        assert!(!piles
            .draw_pile
            .iter()
            .any(|card| card.content_id == ANGER_ID && card.bottled));
    }

    #[test]
    fn snecko_eye_draws_seven_and_randomizes_playable_opening_hand_costs() {
        let deck = ironclad_starter_deck();
        let mut shuffle_rng = StsRng::new(1_957_307_888_551 + 1);
        let mut card_random_rng = Some(StsRng::new(1_957_307_888_551 + 1));

        let piles = initialize_combat_piles_with_relics(
            &deck,
            &mut shuffle_rng,
            &mut card_random_rng,
            &[crate::Relic::SneckoEye],
        );

        assert_eq!(piles.hand.len(), 7);
        assert_eq!(piles.draw_pile.len(), 3);
        assert!(piles.hand.iter().any(|card| card.temp_cost.is_some()));
        assert_eq!(
            card_random_rng.as_ref().expect("card rng").counter() as usize,
            piles
                .hand
                .iter()
                .filter(|card| card.temp_cost.is_some())
                .count()
        );
    }
}
