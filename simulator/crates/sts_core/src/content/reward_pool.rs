use crate::{
    card::CardRarity,
    content::cards::{
        is_curse_content_id, ANGER_ID, ARMAMENTS_ID, BARRICADE_ID, BATTLE_TRANCE_ID, BERSERK_ID,
        BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BODY_SLAM_ID, BRUTALITY_ID,
        BURNING_PACT_ID, CARNAGE_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, CLUMSY_ID, COMBUST_ID,
        CORRUPTION_ID, DARK_EMBRACE_ID, DECAY_ID, DEMON_FORM_ID, DISARM_ID, DOUBLE_TAP_ID,
        DOUBT_ID, DROPKICK_ID, DUAL_WIELD_ID, ENTRENCH_ID, EVOLVE_ID, EXHUME_ID, FEED_ID,
        FEEL_NO_PAIN_ID, FIEND_FIRE_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID,
        GHOSTLY_ARMOR_ID, HAVOC_ID, HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID,
        IMPERVIOUS_ID, INFERNAL_BLADE_ID, INFLAME_ID, INJURY_ID, INTIMIDATE_ID, IRON_WAVE_ID,
        JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, NORMALITY_ID, OFFERING_ID, PAIN_ID,
        PARASITE_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID, POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID,
        RAMPAGE_ID, REAPER_ID, RECKLESS_CHARGE_ID, REGRET_ID, RUPTURE_ID, SEARING_BLOW_ID,
        SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID, SEVER_SOUL_ID, SHAME_ID, SHOCKWAVE_ID,
        SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID, SWORD_BOOMERANG_ID, THUNDERCLAP_ID, TRUE_GRIT_ID,
        TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WHIRLWIND_ID, WILD_STRIKE_ID, WRITHE_ID,
    },
    rng::StsRng,
    ContentId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewardCardEntry {
    pub content_id: ContentId,
    pub rarity: CardRarity,
}

/// Target normal curse pool/order used by `AbstractDungeon.returnRandomCurse`.
///
/// `CardLibrary.getCurse()` iterates a Java `HashMap`, not source declaration order. With the
/// target curse insertions the map has resized to 32 buckets by the time normal curses are sampled,
/// then special curses (`Ascender's Bane`, `Curse of the Bell`, `Necronomicurse`, and `Pride`) are
/// filtered out. Several entries are currently inert/unplayable placeholders in combat; this pool
/// only claims identity/RNG parity.
pub const NORMAL_CURSE_POOL: &[ContentId] = &[
    REGRET_ID,
    INJURY_ID,
    SHAME_ID,
    PARASITE_ID,
    NORMALITY_ID,
    DOUBT_ID,
    WRITHE_ID,
    PAIN_ID,
    DECAY_ID,
    CLUMSY_ID,
];

pub fn random_normal_curse(rng: &mut StsRng) -> ContentId {
    let pick = rng.random_int((NORMAL_CURSE_POOL.len() - 1) as i32) as usize;
    NORMAL_CURSE_POOL[pick]
}

/// Target Ironclad combat-reward pool.
///
/// The target reward pool order follows `CardLibrary.cards.entrySet()` rather than class-file
/// declaration order. Treat this as trace/source-backed ordering, not an alphabetized list.
/// Some entries are not yet implemented as playable `CardDefinition`s; they still belong here
/// because reward RNG indexes into the full target pool before the player can choose a card.
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
        content_id: IRON_WAVE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: BODY_SLAM_ID,
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
        content_id: CLASH_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: THUNDERCLAP_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: POMMEL_STRIKE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: TWIN_STRIKE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: CLOTHESLINE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: ARMAMENTS_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: HAVOC_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: HEADBUTT_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: WILD_STRIKE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: HEAVY_BLADE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: PERFECTED_STRIKE_ID,
        rarity: CardRarity::Common,
    },
    RewardCardEntry {
        content_id: SWORD_BOOMERANG_ID,
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
        content_id: POWER_THROUGH_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: DUAL_WIELD_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: INFERNAL_BLADE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: RECKLESS_CHARGE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: HEMOKINESIS_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: INTIMIDATE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: BLOOD_FOR_BLOOD_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: FLAME_BARRIER_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: PUMMEL_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: BURNING_PACT_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: METALLICIZE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: SHOCKWAVE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: RAMPAGE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: SEVER_SOUL_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: WHIRLWIND_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: COMBUST_ID,
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
        content_id: DISARM_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: FEEL_NO_PAIN_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: RAGE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: ENTRENCH_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: SENTINEL_ID,
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
    RewardCardEntry {
        content_id: SECOND_WIND_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: RUPTURE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: BLOODLETTING_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: CARNAGE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: DROPKICK_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: FIRE_BREATHING_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: GHOSTLY_ARMOR_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: UPPERCUT_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: EVOLVE_ID,
        rarity: CardRarity::Uncommon,
    },
    RewardCardEntry {
        content_id: IMMOLATE_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: OFFERING_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: EXHUME_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: REAPER_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: BRUTALITY_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: JUGGERNAUT_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: IMPERVIOUS_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: BERSERK_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: FIEND_FIRE_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: BARRICADE_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: CORRUPTION_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: LIMIT_BREAK_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: FEED_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: BLUDGEON_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: DEMON_FORM_ID,
        rarity: CardRarity::Rare,
    },
    RewardCardEntry {
        content_id: DOUBLE_TAP_ID,
        rarity: CardRarity::Rare,
    },
];

const IRONCLAD_TRANSFORM_POOL: &[ContentId] = &[
    ANGER_ID,
    CLEAVE_ID,
    WARCRY_ID,
    FLEX_ID,
    IRON_WAVE_ID,
    BODY_SLAM_ID,
    TRUE_GRIT_ID,
    SHRUG_IT_OFF_ID,
    CLASH_ID,
    THUNDERCLAP_ID,
    POMMEL_STRIKE_ID,
    TWIN_STRIKE_ID,
    CLOTHESLINE_ID,
    ARMAMENTS_ID,
    HAVOC_ID,
    HEADBUTT_ID,
    WILD_STRIKE_ID,
    HEAVY_BLADE_ID,
    PERFECTED_STRIKE_ID,
    SWORD_BOOMERANG_ID,
    EVOLVE_ID,
    UPPERCUT_ID,
    GHOSTLY_ARMOR_ID,
    FIRE_BREATHING_ID,
    DROPKICK_ID,
    CARNAGE_ID,
    BLOODLETTING_ID,
    RUPTURE_ID,
    SECOND_WIND_ID,
    SEARING_BLOW_ID,
    BATTLE_TRANCE_ID,
    SENTINEL_ID,
    ENTRENCH_ID,
    RAGE_ID,
    FEEL_NO_PAIN_ID,
    DISARM_ID,
    SEEING_RED_ID,
    DARK_EMBRACE_ID,
    COMBUST_ID,
    WHIRLWIND_ID,
    SEVER_SOUL_ID,
    RAMPAGE_ID,
    SHOCKWAVE_ID,
    METALLICIZE_ID,
    BURNING_PACT_ID,
    PUMMEL_ID,
    FLAME_BARRIER_ID,
    BLOOD_FOR_BLOOD_ID,
    INTIMIDATE_ID,
    HEMOKINESIS_ID,
    RECKLESS_CHARGE_ID,
    INFERNAL_BLADE_ID,
    DUAL_WIELD_ID,
    POWER_THROUGH_ID,
    INFLAME_ID,
    SPOT_WEAKNESS_ID,
    DOUBLE_TAP_ID,
    DEMON_FORM_ID,
    BLUDGEON_ID,
    FEED_ID,
    LIMIT_BREAK_ID,
    CORRUPTION_ID,
    BARRICADE_ID,
    FIEND_FIRE_ID,
    BERSERK_ID,
    IMPERVIOUS_ID,
    JUGGERNAUT_ID,
    BRUTALITY_ID,
    REAPER_ID,
    EXHUME_ID,
    OFFERING_ID,
    IMMOLATE_ID,
];

#[must_use]
pub fn ironclad_reward_content_ids() -> Vec<ContentId> {
    IRONCLAD_REWARD_ENTRIES
        .iter()
        .map(|entry| entry.content_id)
        .collect()
}

pub fn ironclad_transform_card_content_id(source: ContentId, rng: &mut StsRng) -> ContentId {
    let pool = ironclad_transform_card_pool(source);
    let pick = rng.random_int((pool.len() - 1) as i32) as usize;
    pool[pick]
}

/// Event/special colorless cards (Bite, Apparition, …) are not shop colorless
/// entries but still use `srcColorlessCardPool` under `AbstractDungeon.transformCard`.
fn is_colorless_for_transform(source: ContentId) -> bool {
    use crate::content::cards::{
        APPARITION_ID, APPARITION_PLUS_ID, BITE_ID, BITE_PLUS_ID, JAX_ID, JAX_PLUS_ID,
        RITUAL_DAGGER_ID,
    };
    crate::content::shop_pool::shop_card_is_colorless(source)
        || matches!(
            source,
            BITE_ID
                | BITE_PLUS_ID
                | RITUAL_DAGGER_ID
                | APPARITION_ID
                | APPARITION_PLUS_ID
                | JAX_ID
                | JAX_PLUS_ID
        )
}

#[must_use]
pub fn ironclad_transform_card_pool(source: ContentId) -> Vec<ContentId> {
    // AbstractDungeon.transformCard branches on card color. Curse transforms call
    // CardLibrary.getCurse(source, rng), which excludes the source and special curses.
    // Colorless cards use the colorless card library pool; non-curse Ironclad
    // cards use the character transform pool below.
    //
    // STS compares `cardID` strings, so upgraded copies share the base identity
    // for exclusion. Normalize before filtering (FIDL00263 Thunderclap+).
    let source = crate::content::cards::base_content_id(source);
    if is_colorless_for_transform(source) {
        return crate::content::shop_pool::colorless_transform_pool()
            .into_iter()
            .filter(|content_id| *content_id != source)
            .collect();
    }
    let pool = if is_curse_content_id(source) {
        NORMAL_CURSE_POOL
    } else {
        IRONCLAD_TRANSFORM_POOL
    };
    pool.iter()
        .copied()
        .filter(|content_id| *content_id != source)
        .collect()
}

#[must_use]
pub fn ironclad_reward_card_rarity(content_id: ContentId) -> Option<CardRarity> {
    IRONCLAD_REWARD_ENTRIES
        .iter()
        .find(|entry| entry.content_id == content_id)
        .map(|entry| entry.rarity)
}

#[must_use]
pub fn ironclad_truly_random_card_pool() -> Vec<ContentId> {
    // AbstractDungeon.initializeCardPools builds each rarity pool with addToTop, then
    // returnTrulyRandomCard concatenates the resulting source pools common/uncommon/rare.
    // Keep this separate from combat reward order, which has its own rarity-roll flow.
    [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare]
        .into_iter()
        .flat_map(|rarity| {
            IRONCLAD_REWARD_ENTRIES
                .iter()
                .filter(move |entry| entry.rarity == rarity)
                .rev()
                .map(|entry| entry.content_id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{CHRYSALIS_ID, CURSE_OF_THE_BELL_ID, MASTER_OF_STRATEGY_ID};

    #[test]
    fn upgraded_source_excludes_base_id_from_transform_pool() {
        use crate::content::cards::THUNDERCLAP_PLUS_ID;
        let from_base = ironclad_transform_card_pool(THUNDERCLAP_ID);
        let from_plus = ironclad_transform_card_pool(THUNDERCLAP_PLUS_ID);
        assert_eq!(from_base, from_plus);
        assert!(!from_plus.contains(&THUNDERCLAP_ID));
        assert_eq!(from_plus.len(), IRONCLAD_TRANSFORM_POOL.len() - 1);
    }

    #[test]
    fn curse_transform_pool_uses_normal_curses_and_excludes_source() {
        let regret_pool = ironclad_transform_card_pool(REGRET_ID);
        assert_eq!(regret_pool.len(), NORMAL_CURSE_POOL.len() - 1);
        assert!(!regret_pool.contains(&REGRET_ID));
        assert!(regret_pool.contains(&CLUMSY_ID));
        assert!(!regret_pool.contains(&HEAVY_BLADE_ID));

        let bell_pool = ironclad_transform_card_pool(CURSE_OF_THE_BELL_ID);
        assert_eq!(bell_pool, NORMAL_CURSE_POOL);
    }

    #[test]
    fn colorless_transform_pool_stays_colorless() {
        let pool = ironclad_transform_card_pool(MASTER_OF_STRATEGY_ID);
        assert!(pool.contains(&CHRYSALIS_ID));
        assert!(!pool.contains(&ANGER_ID));
        assert!(!pool.contains(&MASTER_OF_STRATEGY_ID));
    }

    #[test]
    fn bite_transform_uses_colorless_pool_not_ironclad() {
        // Drug Dealer / event transforms: Bite is colorless SPECIAL, not red.
        use crate::content::cards::{BITE_ID, SWIFT_STRIKE_ID};
        let pool = ironclad_transform_card_pool(BITE_ID);
        assert!(
            pool.contains(&SWIFT_STRIKE_ID),
            "Bite must transform via colorless pool"
        );
        assert!(
            !pool.contains(&ANGER_ID),
            "Bite must not use the Ironclad transform pool"
        );
        assert!(!pool.contains(&BITE_ID));
    }
}
