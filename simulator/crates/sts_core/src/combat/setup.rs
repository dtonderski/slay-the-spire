use crate::{
    card::CardInstance,
    combat::CardPiles,
    content::cards::{get_card_definition, BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    relic::{Relic, SNECKO_EYE_DRAW},
    rng::StsRng,
    ContentId,
};

const OPENING_HAND_SIZE: usize = 5;

/// Target Ironclad starter master-deck order before the first combat shuffle.
///
/// Starter deck instances are assigned sequential IDs in strike/defend/bash order;
/// the game shuffles this fixed instance order rather than CommunicationMod deck export
/// order or naive strike/defend grouping.
const IRONCLAD_STARTER_SHUFFLE_CARD_IDS: [u64; 10] = [4, 9, 6, 5, 10, 3, 1, 2, 8, 7];

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
    let mut starter_slots = [None; 10];
    let mut extras = Vec::new();

    for card in deck {
        let id = card.id.get();
        if (1..=10).contains(&id) {
            starter_slots[(id - 1) as usize] = Some(card.clone());
        } else {
            extras.push(card.clone());
        }
    }

    let mut ordered = Vec::with_capacity(deck.len());
    for slot_id in IRONCLAD_STARTER_SHUFFLE_CARD_IDS {
        if let Some(card) = starter_slots[(slot_id - 1) as usize].take() {
            ordered.push(card);
        }
    }

    for card in starter_slots.into_iter().flatten() {
        ordered.push(card);
    }

    extras.sort_by_key(|card| card.id.get());
    ordered.extend(extras);
    ordered
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
    let ordered = order_deck_for_combat_shuffle(deck);
    let mut innate = Vec::new();
    let mut pool = Vec::new();

    for card in ordered {
        if card_starts_in_opening_hand(&card) {
            innate.push(card);
        } else {
            pool.push(card);
        }
    }

    shuffle_rng.collections_shuffle(&mut pool);

    let draw_count = opening_hand_size(relics).saturating_sub(innate.len());
    let mut hand = innate;
    for mut card in pool.drain(..draw_count.min(pool.len())) {
        if relics.contains(&Relic::SneckoEye)
            && get_card_definition(card.content_id).is_some_and(|definition| definition.cost > 0)
        {
            if let Some(rng) = card_random_rng.as_mut() {
                card.temp_cost = Some(rng.random_int(3) as u8);
            }
        }
        hand.push(card);
    }

    CardPiles {
        hand,
        draw_pile: pool,
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
    deck.iter().all(|card| {
        matches!(
            card.content_id,
            id if id == STRIKE_R_ID || id == DEFEND_R_ID || id == BASH_ID
        )
    })
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
    fn order_deck_for_combat_shuffle_uses_target_instance_order() {
        let deck = ironclad_starter_deck();
        let ordered = order_deck_for_combat_shuffle(&deck);
        assert_eq!(
            ordered.iter().map(|card| card.id.get()).collect::<Vec<_>>(),
            IRONCLAD_STARTER_SHUFFLE_CARD_IDS.to_vec()
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
