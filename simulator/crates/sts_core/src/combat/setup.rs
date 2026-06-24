use crate::{
    card::CardInstance,
    combat::CardPiles,
    content::cards::{get_card_definition, BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
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

    let draw_count = OPENING_HAND_SIZE.saturating_sub(innate.len());
    let mut hand = innate;
    hand.extend(pool.drain(..draw_count.min(pool.len())));

    CardPiles {
        hand,
        draw_pile: pool,
        discard_pile: Vec::new(),
        exhaust_pile: Vec::new(),
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
}
