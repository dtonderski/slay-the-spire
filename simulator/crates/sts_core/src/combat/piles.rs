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
    rng: Option<&mut crate::rng::StsRng>,
) {
    let mut rng = rng;
    for _ in 0..count {
        let next_id = CardId::new(piles.max_card_instance_id() + 1);
        let card = CardInstance::new(next_id, content_id);
        if piles.draw_pile.is_empty() {
            piles.draw_pile.push(card);
        } else if let Some(rng) = rng.as_deref_mut() {
            let index = rng.random_int((piles.draw_pile.len() - 1) as i32) as usize;
            piles.draw_pile.insert(index, card);
        } else {
            piles.draw_pile.push(card);
        }
    }
}
