use crate::{card::CardInstance, combat::CardPiles, ids::CardId, ContentId};

pub fn add_cards_to_discard(piles: &mut CardPiles, content_id: ContentId, count: i32) {
    for _ in 0..count {
        let next_id = CardId::new(piles.max_card_instance_id() + 1);
        piles
            .discard_pile
            .push(CardInstance::new(next_id, content_id));
    }
}
