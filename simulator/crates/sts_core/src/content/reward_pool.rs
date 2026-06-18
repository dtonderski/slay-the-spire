use crate::{
    card::CardRarity,
    content::cards::{
        ANGER_ID, BATTLE_TRANCE_ID, CLEAVE_ID, HAVOC_ID, POMMEL_STRIKE_ID, SEARING_BLOW_ID,
        SHRUG_IT_OFF_ID, TWIN_STRIKE_ID, WARCRY_ID,
    },
    ContentId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardCardEntry {
    pub content_id: ContentId,
    pub rarity: CardRarity,
}

/// Ironclad combat-reward pool with game rarities. Weights are placeholder until verified.
pub const IRONCLAD_REWARD_ENTRIES: &[RewardCardEntry] = &[
    RewardCardEntry {
        content_id: ANGER_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: CLEAVE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: SHRUG_IT_OFF_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: TWIN_STRIKE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: POMMEL_STRIKE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: BATTLE_TRANCE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: HAVOC_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: WARCRY_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: SEARING_BLOW_ID,
        rarity: CardRarity::Common,
    },
];

#[must_use]
pub fn ironclad_reward_content_ids() -> Vec<ContentId> {
    IRONCLAD_REWARD_ENTRIES
        .iter()
        .map(|entry| entry.content_id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ironclad_reward_pool_has_nine_unique_cards() {
        let ids = ironclad_reward_content_ids();
        assert_eq!(ids.len(), 9);
        assert_eq!(ids.len(), {
            let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
            unique.len()
        });
    }

    #[test]
    fn havoc_is_the_only_rare_ironclad_reward_card() {
        let rare: Vec<_> = IRONCLAD_REWARD_ENTRIES
            .iter()
            .filter(|entry| entry.rarity == CardRarity::Rare)
            .map(|entry| entry.content_id)
            .collect();

        assert_eq!(rare, vec![HAVOC_ID]);
    }
}
