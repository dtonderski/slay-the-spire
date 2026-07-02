use crate::{
    card::CardRarity,
    content::cards::{
        ANGER_ID, ARMAMENTS_ID, BARRICADE_ID, BATTLE_TRANCE_ID, BERSERK_ID, BLOODLETTING_ID,
        BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID, CARNAGE_ID,
        CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, CLUMSY_ID, COMBUST_ID, CORRUPTION_ID, DARK_EMBRACE_ID,
        DECAY_ID, DEMON_FORM_ID, DISARM_ID, DOUBLE_TAP_ID, DOUBT_ID, DROPKICK_ID, DUAL_WIELD_ID,
        ENTRENCH_ID, EVOLVE_ID, EXHUME_ID, FEED_ID, FEEL_NO_PAIN_ID, FIEND_FIRE_ID,
        FIRE_BREATHING_ID, FLAME_BARRIER_ID, FLEX_ID, GHOSTLY_ARMOR_ID, HAVOC_ID, HEADBUTT_ID,
        HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, IMPERVIOUS_ID, INFERNAL_BLADE_ID, INFLAME_ID,
        INJURY_ID, INTIMIDATE_ID, IRON_WAVE_ID, JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID,
        NORMALITY_ID, OFFERING_ID, PAIN_ID, PARASITE_ID, PERFECTED_STRIKE_ID, POMMEL_STRIKE_ID,
        POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID, RAMPAGE_ID, REAPER_ID, RECKLESS_CHARGE_ID, REGRET_ID,
        RUPTURE_ID, SEARING_BLOW_ID, SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID, SEVER_SOUL_ID,
        SHAME_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID, SWORD_BOOMERANG_ID,
        THUNDERCLAP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WHIRLWIND_ID,
        WILD_STRIKE_ID, WRITHE_ID,
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
        content_id: IMMOLATE_ID,
        rarity: CardRarity::Rare,
    },
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

#[must_use]
pub fn ironclad_transform_card_pool(source: ContentId) -> Vec<ContentId> {
    IRONCLAD_TRANSFORM_POOL
        .iter()
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
    [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare]
        .into_iter()
        .flat_map(|rarity| {
            IRONCLAD_REWARD_ENTRIES
                .iter()
                .filter(move |entry| entry.rarity == rarity)
                .map(|entry| entry.content_id)
        })
        .collect()
}
