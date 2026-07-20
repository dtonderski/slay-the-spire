use crate::{card::CardInstance, combat::CardPiles, ids::CardId, ContentId};

pub fn add_cards_to_discard(piles: &mut CardPiles, content_id: ContentId, count: i32) {
    for _ in 0..count {
        let next_id = CardId::new(piles.max_card_instance_id() + 1);
        piles
            .discard_pile
            .push(CardInstance::new(next_id, content_id));
    }
}

pub fn add_cards_to_draw_random_spot(
    piles: &mut CardPiles,
    content_id: ContentId,
    count: i32,
    rng: &mut crate::rng::StsRng,
) {
    for _ in 0..count {
        let next_id = CardId::new(piles.max_card_instance_id() + 1);
        let card = CardInstance::new(next_id, content_id);
        if piles.draw_pile.is_empty() {
            piles.draw_pile.push(card);
        } else {
            let index = rng.random_int((piles.draw_pile.len() - 1) as i32) as usize;
            piles.draw_pile.insert(index, card);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::BURN_ID, rng::StsRng};

    #[test]
    fn random_draw_insertion_requires_and_consumes_rng() {
        let mut piles = CardPiles {
            hand: Vec::new(),
            draw_pile: vec![
                CardInstance::new(CardId::new(1), BURN_ID),
                CardInstance::new(CardId::new(2), BURN_ID),
            ],
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        };
        let mut rng = StsRng::new(17);

        add_cards_to_draw_random_spot(&mut piles, BURN_ID, 2, &mut rng);

        assert_eq!(rng.counter(), 2);
        assert_eq!(piles.draw_pile.len(), 4);
        let mut ids = piles
            .draw_pile
            .iter()
            .map(|card| card.id.get())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4]);
    }
}
