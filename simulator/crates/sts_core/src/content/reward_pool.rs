use crate::{
    card::CardRarity,
    content::cards::{
        ANGER_ID, BATTLE_TRANCE_ID, BURNING_PACT_ID, CLEAVE_ID, DARK_EMBRACE_ID, DUAL_WIELD_ID,
        FEEL_NO_PAIN_ID, FLEX_ID, HAVOC_ID, INFLAME_ID, POMMEL_STRIKE_ID, SEARING_BLOW_ID,
        SEEING_RED_ID, SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, WARCRY_ID,
        WHIRLWIND_ID,
    },
    ContentId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardCardEntry {
    pub content_id: ContentId,
    pub rarity: CardRarity,
}

/// Source-order subset of the Ironclad combat-reward pool for cards modeled by this simulator.
///
/// The full target pool has 72 Ironclad cards. This subset preserves target rarity and relative
/// order for the cards that currently have simulator definitions; reward generation must report
/// pool completeness separately from RNG control-flow parity.
pub const IRONCLAD_REWARD_ENTRIES: &[RewardCardEntry] = &[
    RewardCardEntry {
        content_id: ANGER_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: CLEAVE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: WARCRY_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: FLEX_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: TRUE_GRIT_ID,
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
        content_id: HAVOC_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: SPOT_WEAKNESS_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: INFLAME_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: DUAL_WIELD_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: BURNING_PACT_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: WHIRLWIND_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: DARK_EMBRACE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: SEEING_RED_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: FEEL_NO_PAIN_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: BATTLE_TRANCE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: SEARING_BLOW_ID,
        rarity: CardRarity::Uncommon,
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
    fn ironclad_reward_pool_has_unique_modeled_cards() {
        let ids = ironclad_reward_content_ids();
        assert_eq!(ids.len(), IRONCLAD_REWARD_ENTRIES.len());
        assert_eq!(ids.len(), {
            let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
            unique.len()
        });
    }

    #[test]
    fn modeled_pool_preserves_target_rarities_for_known_cards() {
        let rarity = |id| {
            IRONCLAD_REWARD_ENTRIES
                .iter()
                .find(|entry| entry.content_id == id)
                .map(|entry| entry.rarity)
                .expect("modeled card is in reward pool")
        };

        assert_eq!(rarity(ANGER_ID), CardRarity::Common);
        assert_eq!(rarity(HAVOC_ID), CardRarity::Common);
        assert_eq!(rarity(WARCRY_ID), CardRarity::Common);
        assert_eq!(rarity(BATTLE_TRANCE_ID), CardRarity::Uncommon);
        assert_eq!(rarity(SEARING_BLOW_ID), CardRarity::Uncommon);
    }
}
