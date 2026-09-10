//! Explicit public-history record of known draw-pile positions.
//!
//! This tracker is advanced at accepted rule transitions from the *kind* of
//! public knowledge those transitions create. It is never reconstructed from
//! hidden draw order, CommunicationMod traces, or post-hoc observation repair.
//! Internal `CardId` values are private bookkeeping and must not leak through
//! fair observations.
//!
//! Position 0 is the next draw (current `Vec` last). Unknown-index mutations
//! invalidate positional knowledge rather than using the true RNG index.

use super::state::CardPiles;
use crate::card::CardInstance;
use crate::ids::CardId;
use serde::{Deserialize, Serialize};

/// Publicly known draw-pile positions, stored by private instance identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DrawPilePublicKnowledge {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entries: Vec<(u16, CardId)>,
}

impl DrawPilePublicKnowledge {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Known positions in ascending next-draw order. Never inspects the true pile.
    pub fn iter_sorted(&self) -> impl Iterator<Item = (u16, CardId)> + '_ {
        self.entries.iter().copied()
    }

    pub fn insert_top(&mut self, card_id: CardId) {
        self.forget_card(card_id);
        if !self.shift_positions(1) {
            return;
        }
        self.entries.insert(0, (0, card_id));
    }

    pub fn insert_bottom(&mut self, card_id: CardId, pile_len_after: usize) {
        self.forget_card(card_id);
        let Some(position) = position_from_len(pile_len_after.saturating_sub(1)) else {
            self.clear();
            return;
        };
        self.entries.push((position, card_id));
        self.sort();
    }

    pub fn insert_known_prefix(&mut self, card_ids: &[CardId]) {
        for card_id in card_ids {
            self.forget_card(*card_id);
        }
        let Some(shift) = u16::try_from(card_ids.len()).ok() else {
            self.clear();
            return;
        };
        if !self.shift_positions(shift) {
            return;
        }
        let prefix = card_ids.iter().enumerate().map(|(index, card_id)| {
            (
                u16::try_from(index).expect("prefix length already checked"),
                *card_id,
            )
        });
        self.entries.splice(0..0, prefix);
    }

    pub fn remove_top(&mut self) {
        self.entries.retain(|(position, _)| *position != 0);
        for (position, _) in &mut self.entries {
            *position = position.saturating_sub(1);
        }
    }

    pub fn insert_unknown_index(&mut self) {
        self.clear();
    }

    pub fn remove_unknown_index(&mut self) {
        // Unknown-position removals must not branch on private CardId or the
        // true RNG index. Conservative invalidation is the safe default.
        self.clear();
    }

    pub fn shuffle_or_replace_unknown(&mut self) {
        self.clear();
    }

    /// Record a publicly revealed top prefix. Does not write Frozen Eye overlays.
    pub fn reveal_prefix(&mut self, card_ids: &[CardId]) {
        let Some(prefix_len) = u16::try_from(card_ids.len()).ok() else {
            self.clear();
            return;
        };
        let prefix_ids = card_ids;
        self.entries
            .retain(|(position, card_id)| *position >= prefix_len && !prefix_ids.contains(card_id));
        let prefix = card_ids.iter().enumerate().map(|(index, card_id)| {
            (
                u16::try_from(index).expect("prefix length already checked"),
                *card_id,
            )
        });
        self.entries.splice(0..0, prefix);
        self.sort();
    }

    fn forget_card(&mut self, card_id: CardId) {
        self.entries.retain(|(_, known_id)| *known_id != card_id);
    }

    fn shift_positions(&mut self, delta: u16) -> bool {
        if delta == 0 {
            return true;
        }
        let mut shifted = Vec::with_capacity(self.entries.len());
        for &(position, card_id) in &self.entries {
            let Some(next) = position.checked_add(delta) else {
                self.clear();
                return false;
            };
            shifted.push((next, card_id));
        }
        self.entries = shifted;
        true
    }

    fn sort(&mut self) {
        self.entries.sort_by_key(|(position, _)| *position);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

fn position_from_len(index: usize) -> Option<u16> {
    u16::try_from(index).ok()
}

impl CardPiles {
    pub(crate) fn push_draw_top(&mut self, card: CardInstance) {
        self.draw_pile_knowledge.insert_top(card.id);
        self.draw_pile.push(card);
    }

    /// Gameplay top restore that is not a public insert (hidden hold-outs).
    /// Remaining positional knowledge is cleared rather than shifted from a
    /// private pre-pop.
    pub(crate) fn restore_hidden_draw_top(&mut self, card: CardInstance) {
        self.draw_pile.push(card);
        self.draw_pile_knowledge.shuffle_or_replace_unknown();
    }

    pub(crate) fn pop_draw_top(&mut self) -> Option<CardInstance> {
        let card = self.draw_pile.pop()?;
        self.draw_pile_knowledge.remove_top();
        Some(card)
    }

    pub(crate) fn insert_draw_bottom(&mut self, card: CardInstance) {
        let card_id = card.id;
        self.draw_pile.insert(0, card);
        self.draw_pile_knowledge
            .insert_bottom(card_id, self.draw_pile.len());
    }

    pub(crate) fn insert_draw_unknown_index(&mut self, index: usize, card: CardInstance) {
        self.draw_pile.insert(index, card);
        self.draw_pile_knowledge.insert_unknown_index();
    }

    pub(crate) fn remove_draw_unknown_index(&mut self, index: usize) -> CardInstance {
        let card = self.draw_pile.remove(index);
        self.draw_pile_knowledge.remove_unknown_index();
        card
    }

    pub(crate) fn invalidate_draw_order(&mut self) {
        self.draw_pile_knowledge.shuffle_or_replace_unknown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID};
    use crate::CardInstance;

    fn id(value: u64) -> CardId {
        CardId::new(value)
    }

    #[test]
    fn double_headbutt_records_newest_card_at_position_zero() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(1));
        knowledge.insert_top(id(2));
        assert_eq!(
            knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(2)), (1, id(1))]
        );
    }

    #[test]
    fn remove_top_shifts_remaining_known_positions() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(1));
        knowledge.insert_top(id(2));
        knowledge.remove_top();
        assert_eq!(
            knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(1))]
        );
        knowledge.remove_top();
        assert!(knowledge.is_empty());
    }

    #[test]
    fn insert_bottom_does_not_shift_existing_known_tops() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(1));
        knowledge.insert_bottom(id(2), 4);
        assert_eq!(
            knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(1)), (3, id(2))]
        );
    }

    #[test]
    fn unknown_index_insert_clears_all_positions() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(1));
        knowledge.insert_unknown_index();
        assert!(knowledge.is_empty());
    }

    #[test]
    fn unknown_index_remove_always_clears_positional_knowledge() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(1));
        knowledge.insert_top(id(2));
        knowledge.remove_unknown_index();
        assert!(knowledge.is_empty());
        knowledge.insert_top(id(1));
        knowledge.remove_unknown_index();
        assert!(knowledge.is_empty());
    }

    #[test]
    fn hidden_restore_does_not_record_the_restored_card() {
        let mut piles = CardPiles {
            hand: Vec::new(),
            draw_pile: vec![CardInstance::new(id(1), STRIKE_R_ID)],
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            limbo: Vec::new(),
            draw_pile_knowledge: DrawPilePublicKnowledge::default(),
        };
        piles.push_draw_top(CardInstance::new(id(2), BASH_ID));
        piles.restore_hidden_draw_top(CardInstance::new(id(3), DEFEND_R_ID));
        assert_eq!(piles.draw_pile.last().map(|card| card.id), Some(id(3)));
        assert!(piles.draw_pile_knowledge.is_empty());
    }

    #[test]
    fn shuffle_clears_positional_knowledge() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(1));
        knowledge.shuffle_or_replace_unknown();
        assert!(knowledge.is_empty());
    }

    #[test]
    fn reveal_prefix_overwrites_the_public_top() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_bottom(id(9), 5);
        knowledge.reveal_prefix(&[id(1), id(2)]);
        assert_eq!(
            knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(1)), (1, id(2)), (4, id(9))]
        );
    }

    #[test]
    fn insert_known_prefix_shifts_existing_entries() {
        let mut knowledge = DrawPilePublicKnowledge::default();
        knowledge.insert_top(id(9));
        knowledge.insert_known_prefix(&[id(1), id(2)]);
        assert_eq!(
            knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(1)), (1, id(2)), (2, id(9))]
        );
    }

    #[test]
    fn card_piles_helpers_follow_top_and_bottom_geometry() {
        let mut piles = CardPiles {
            hand: Vec::new(),
            draw_pile: vec![CardInstance::new(id(1), STRIKE_R_ID)],
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            limbo: Vec::new(),
            draw_pile_knowledge: DrawPilePublicKnowledge::default(),
        };
        piles.push_draw_top(CardInstance::new(id(2), BASH_ID));
        piles.push_draw_top(CardInstance::new(id(3), DEFEND_R_ID));
        assert_eq!(piles.draw_pile.last().map(|card| card.id), Some(id(3)));
        assert_eq!(
            piles.draw_pile_knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(3)), (1, id(2))]
        );

        let drawn = piles.pop_draw_top().expect("top card");
        assert_eq!(drawn.id, id(3));
        assert_eq!(
            piles.draw_pile_knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(2))]
        );

        piles.insert_draw_bottom(CardInstance::new(id(4), STRIKE_R_ID));
        assert_eq!(piles.draw_pile[0].id, id(4));
        assert_eq!(
            piles.draw_pile_knowledge.iter_sorted().collect::<Vec<_>>(),
            vec![(0, id(2)), (2, id(4))]
        );
    }
}
