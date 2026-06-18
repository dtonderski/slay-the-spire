use crate::{
    card::CardInstance,
    content::ascension::AscensionConfig,
    content::cards::{ASCENDERS_BANE_ID, BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    ids::CardId,
};

const STRIKE_COUNT: usize = 5;
const DEFEND_COUNT: usize = 4;
const BASH_COUNT: usize = 1;

/// Canonical Ironclad starter deck composition for ascension 0:
/// 5 `Strike_R`, 4 `Defend_R`, 1 `Bash`.
///
/// Card instances receive stable sequential IDs in that order.
/// Ascension 10+ adds one [ASCENDERS_BANE_ID].
#[must_use]
pub fn ironclad_starter_deck() -> Vec<CardInstance> {
    ironclad_starter_deck_for_ascension(0)
}

#[must_use]
pub fn ironclad_starter_deck_for_ascension(ascension: u8) -> Vec<CardInstance> {
    let mut deck = Vec::with_capacity(STRIKE_COUNT + DEFEND_COUNT + BASH_COUNT + 1);
    let mut next_id = 1_u64;

    for _ in 0..STRIKE_COUNT {
        deck.push(CardInstance::new(CardId::new(next_id), STRIKE_R_ID));
        next_id += 1;
    }
    for _ in 0..DEFEND_COUNT {
        deck.push(CardInstance::new(CardId::new(next_id), DEFEND_R_ID));
        next_id += 1;
    }
    for _ in 0..BASH_COUNT {
        deck.push(CardInstance::new(CardId::new(next_id), BASH_ID));
        next_id += 1;
    }

    if AscensionConfig::new(ascension).ascenders_bane_in_deck() {
        deck.push(CardInstance::new(CardId::new(next_id), ASCENDERS_BANE_ID));
    }

    deck
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentId;

    fn count_content_id(deck: &[CardInstance], content_id: ContentId) -> usize {
        deck.iter()
            .filter(|card| card.content_id == content_id)
            .count()
    }

    #[test]
    fn ironclad_starter_deck_has_expected_composition() {
        let deck = ironclad_starter_deck();

        assert_eq!(deck.len(), 10);
        assert_eq!(count_content_id(&deck, STRIKE_R_ID), 5);
        assert_eq!(count_content_id(&deck, DEFEND_R_ID), 4);
        assert_eq!(count_content_id(&deck, BASH_ID), 1);
    }

    #[test]
    fn ironclad_starter_deck_has_stable_card_instance_ids() {
        let first = ironclad_starter_deck();
        let second = ironclad_starter_deck();

        assert_eq!(first, second);
        assert_eq!(
            first.iter().map(|card| card.id).collect::<Vec<_>>(),
            (1_u64..=10).map(CardId::new).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a10_starter_deck_includes_ascenders_bane() {
        let deck = ironclad_starter_deck_for_ascension(10);

        assert_eq!(deck.len(), 11);
        assert_eq!(count_content_id(&deck, ASCENDERS_BANE_ID), 1);
    }

    #[test]
    fn ironclad_a0_base_hp_is_eighty() {
        assert_eq!(super::super::character::IRONCLAD_A0_BASE_HP, 80);
    }
}
