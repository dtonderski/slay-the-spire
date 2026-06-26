use crate::card::{CardRarity, CardType};
use crate::content::cards::{
    ANGER_ID, ARMAMENTS_ID, BANDAGE_UP_ID, BARRICADE_ID, BATTLE_TRANCE_ID, BERSERK_ID, BLIND_ID,
    BLOODLETTING_ID, BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID,
    CARNAGE_ID, CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, COMBUST_ID, CORRUPTION_ID, DARK_EMBRACE_ID,
    DARK_SHACKLES_ID, DEEP_BREATH_ID, DEMON_FORM_ID, DISARM_ID, DISCOVERY_ID, DOUBLE_TAP_ID,
    DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID, ENLIGHTENMENT_ID, ENTRENCH_ID, EVOLVE_ID,
    EXHUME_ID, FEED_ID, FEEL_NO_PAIN_ID, FIEND_FIRE_ID, FIRE_BREATHING_ID, FLAME_BARRIER_ID,
    FLASH_OF_STEEL_ID, FLEX_ID, FORETHOUGHT_ID, GHOSTLY_ARMOR_ID, GOOD_INSTINCTS_ID, HAVOC_ID,
    HEADBUTT_ID, HEAVY_BLADE_ID, HEMOKINESIS_ID, IMMOLATE_ID, IMPATIENCE_ID, IMPERVIOUS_ID,
    INFERNAL_BLADE_ID, INFLAME_ID, INTIMIDATE_ID, IRON_WAVE_ID, JACK_OF_ALL_TRADES_ID,
    JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, OFFERING_ID, PANACEA_ID, PERFECTED_STRIKE_ID,
    POMMEL_STRIKE_ID, POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID, RAMPAGE_ID, REAPER_ID,
    RECKLESS_CHARGE_ID, RUPTURE_ID, SEARING_BLOW_ID, SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID,
    SEVER_SOUL_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID, SWIFT_STRIKE_ID,
    SWORD_BOOMERANG_ID, THUNDERCLAP_ID, TRIP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID,
    WARCRY_ID, WHIRLWIND_ID, WILD_STRIKE_ID,
};
use crate::content::reward_pool::ironclad_reward_card_rarity;
use crate::rng::StsRng;
use crate::ContentId;

const IRONCLAD_ATTACK_COMMON: &[&str] = &[
    "ANGER",
    "BODY_SLAM",
    "CLASH",
    "CLEAVE",
    "CLOTHESLINE",
    "HEADBUTT",
    "HEAVY_BLADE",
    "IRON_WAVE",
    "PERFECTED_STRIKE",
    "POMMEL_STRIKE",
    "SWORD_BOOMERANG",
    "THUNDERCLAP",
    "TWIN_STRIKE",
    "WILD_STRIKE",
];
const IRONCLAD_ATTACK_UNCOMMON: &[&str] = &[
    "BLOOD_FOR_BLOOD",
    "CARNAGE",
    "DROPKICK",
    "HEMOKINESIS",
    "PUMMEL",
    "RAMPAGE",
    "RECKLESS_CHARGE",
    "SEARING_BLOW",
    "SEVER_SOUL",
    "UPPERCUT",
    "WHIRLWIND",
];
const IRONCLAD_ATTACK_RARE: &[&str] = &["BLUDGEON", "FEED", "FIEND_FIRE", "IMMOLATE", "REAPER"];
const IRONCLAD_SKILL_COMMON: &[&str] = &[
    "ARMAMENTS",
    "FLEX",
    "HAVOC",
    "SHRUG_IT_OFF",
    "TRUE_GRIT",
    "WARCRY",
];
const IRONCLAD_SKILL_UNCOMMON: &[&str] = &[
    "BATTLE_TRANCE",
    "BLOODLETTING",
    "BURNING_PACT",
    "DISARM",
    "DUAL_WIELD",
    "ENTRENCH",
    "FLAME_BARRIER",
    "GHOSTLY_ARMOR",
    "INFERNAL_BLADE",
    "INTIMIDATE",
    "POWER_THROUGH",
    "RAGE",
    "SECOND_WIND",
    "SEEING_RED",
    "SENTINEL",
    "SHOCKWAVE",
    "SPOT_WEAKNESS",
];
const IRONCLAD_SKILL_RARE: &[&str] = &[
    "DOUBLE_TAP",
    "EXHUME",
    "IMPERVIOUS",
    "LIMIT_BREAK",
    "OFFERING",
];
const IRONCLAD_POWER_UNCOMMON: &[&str] = &[
    "COMBUST",
    "DARK_EMBRACE",
    "EVOLVE",
    "FEEL_NO_PAIN",
    "FIRE_BREATHING",
    "INFLAME",
    "METALLICIZE",
    "RUPTURE",
];
const IRONCLAD_POWER_RARE: &[&str] = &[
    "BARRICADE",
    "BERSERK",
    "BRUTALITY",
    "CORRUPTION",
    "DEMON_FORM",
    "JUGGERNAUT",
];
const COLORLESS_UNCOMMON: &[&str] = &[
    "BANDAGE_UP",
    "BLIND",
    "DARK_SHACKLES",
    "DEEP_BREATH",
    "DISCOVERY",
    "DRAMATIC_ENTRANCE",
    "ENLIGHTENMENT",
    "FINESSE",
    "FLASH_OF_STEEL",
    "FORETHOUGHT",
    "GOOD_INSTINCTS",
    "IMPATIENCE",
    "JACK_OF_ALL_TRADES",
    "MADNESS",
    "MIND_BLAST",
    "PANACEA",
    "PANIC_BUTTON",
    "PURITY",
    "SWIFT_STRIKE",
    "TRIP",
];
const COLORLESS_RARE: &[&str] = &[
    "APOTHEOSIS",
    "CHRYSALIS",
    "HAND_OF_GREED",
    "MAGNETISM",
    "MASTER_OF_STRATEGY",
    "MAYHEM",
    "METAMORPHOSIS",
    "PANACHE",
    "SADISTIC_NATURE",
    "SECRET_TECHNIQUE",
    "SECRET_WEAPON",
    "THE_BOMB",
    "THINKING_AHEAD",
    "TRANSMUTATION",
    "VIOLENCE",
];

fn ironclad_pool(card_type: CardType, rarity: CardRarity) -> &'static [&'static str] {
    match (card_type, rarity) {
        (CardType::Attack, CardRarity::Common) => IRONCLAD_ATTACK_COMMON,
        (CardType::Attack, CardRarity::Uncommon) => IRONCLAD_ATTACK_UNCOMMON,
        (CardType::Attack, CardRarity::Rare) => IRONCLAD_ATTACK_RARE,
        (CardType::Skill, CardRarity::Common) => IRONCLAD_SKILL_COMMON,
        (CardType::Skill, CardRarity::Uncommon) => IRONCLAD_SKILL_UNCOMMON,
        (CardType::Skill, CardRarity::Rare) => IRONCLAD_SKILL_RARE,
        (CardType::Power, CardRarity::Uncommon) => IRONCLAD_POWER_UNCOMMON,
        (CardType::Power, CardRarity::Rare) => IRONCLAD_POWER_RARE,
        _ => &[],
    }
}

/// Target `rollCardRarityShop` from `sts_lightspeed` `Shop.cpp`.
#[must_use]
pub fn roll_card_rarity_shop(rng: &mut StsRng, card_rarity_factor: i32) -> CardRarity {
    let roll = rng.random_int(99) + card_rarity_factor;
    if roll < 9 {
        CardRarity::Rare
    } else if roll >= 46 {
        CardRarity::Common
    } else {
        CardRarity::Uncommon
    }
}

#[must_use]
pub fn shop_card_content_id(name: &str) -> ContentId {
    match name {
        "ANGER" => ANGER_ID,
        "BODY_SLAM" => BODY_SLAM_ID,
        "CLASH" => CLASH_ID,
        "CLEAVE" => CLEAVE_ID,
        "CLOTHESLINE" => CLOTHESLINE_ID,
        "HEADBUTT" => HEADBUTT_ID,
        "HEAVY_BLADE" => HEAVY_BLADE_ID,
        "IRON_WAVE" => IRON_WAVE_ID,
        "PERFECTED_STRIKE" => PERFECTED_STRIKE_ID,
        "POMMEL_STRIKE" => POMMEL_STRIKE_ID,
        "SWORD_BOOMERANG" => SWORD_BOOMERANG_ID,
        "THUNDERCLAP" => THUNDERCLAP_ID,
        "TWIN_STRIKE" => TWIN_STRIKE_ID,
        "WILD_STRIKE" => WILD_STRIKE_ID,
        "BLOOD_FOR_BLOOD" => BLOOD_FOR_BLOOD_ID,
        "CARNAGE" => CARNAGE_ID,
        "DROPKICK" => DROPKICK_ID,
        "HEMOKINESIS" => HEMOKINESIS_ID,
        "PUMMEL" => PUMMEL_ID,
        "RAMPAGE" => RAMPAGE_ID,
        "RECKLESS_CHARGE" => RECKLESS_CHARGE_ID,
        "SEARING_BLOW" => SEARING_BLOW_ID,
        "SEVER_SOUL" => SEVER_SOUL_ID,
        "UPPERCUT" => UPPERCUT_ID,
        "WHIRLWIND" => WHIRLWIND_ID,
        "BLUDGEON" => BLUDGEON_ID,
        "FEED" => FEED_ID,
        "FIEND_FIRE" => FIEND_FIRE_ID,
        "IMMOLATE" => IMMOLATE_ID,
        "REAPER" => REAPER_ID,
        "ARMAMENTS" => ARMAMENTS_ID,
        "FLEX" => FLEX_ID,
        "HAVOC" => HAVOC_ID,
        "SHRUG_IT_OFF" => SHRUG_IT_OFF_ID,
        "TRUE_GRIT" => TRUE_GRIT_ID,
        "WARCRY" => WARCRY_ID,
        "BATTLE_TRANCE" => BATTLE_TRANCE_ID,
        "BLOODLETTING" => BLOODLETTING_ID,
        "BURNING_PACT" => BURNING_PACT_ID,
        "DISARM" => DISARM_ID,
        "DUAL_WIELD" => DUAL_WIELD_ID,
        "ENTRENCH" => ENTRENCH_ID,
        "FLAME_BARRIER" => FLAME_BARRIER_ID,
        "GHOSTLY_ARMOR" => GHOSTLY_ARMOR_ID,
        "INFERNAL_BLADE" => INFERNAL_BLADE_ID,
        "INTIMIDATE" => INTIMIDATE_ID,
        "POWER_THROUGH" => POWER_THROUGH_ID,
        "RAGE" => RAGE_ID,
        "SECOND_WIND" => SECOND_WIND_ID,
        "SEEING_RED" => SEEING_RED_ID,
        "SENTINEL" => SENTINEL_ID,
        "SHOCKWAVE" => SHOCKWAVE_ID,
        "SPOT_WEAKNESS" => SPOT_WEAKNESS_ID,
        "DOUBLE_TAP" => DOUBLE_TAP_ID,
        "EXHUME" => EXHUME_ID,
        "IMPERVIOUS" => IMPERVIOUS_ID,
        "LIMIT_BREAK" => LIMIT_BREAK_ID,
        "OFFERING" => OFFERING_ID,
        "COMBUST" => COMBUST_ID,
        "DARK_EMBRACE" => DARK_EMBRACE_ID,
        "EVOLVE" => EVOLVE_ID,
        "FEEL_NO_PAIN" => FEEL_NO_PAIN_ID,
        "FIRE_BREATHING" => FIRE_BREATHING_ID,
        "INFLAME" => INFLAME_ID,
        "METALLICIZE" => METALLICIZE_ID,
        "RUPTURE" => RUPTURE_ID,
        "BARRICADE" => BARRICADE_ID,
        "BERSERK" => BERSERK_ID,
        "BRUTALITY" => BRUTALITY_ID,
        "CORRUPTION" => CORRUPTION_ID,
        "DEMON_FORM" => DEMON_FORM_ID,
        "JUGGERNAUT" => JUGGERNAUT_ID,
        "DRAMATIC_ENTRANCE" => DRAMATIC_ENTRANCE_ID,
        "BANDAGE_UP" => BANDAGE_UP_ID,
        "BLIND" => BLIND_ID,
        "DARK_SHACKLES" => DARK_SHACKLES_ID,
        "DEEP_BREATH" => DEEP_BREATH_ID,
        "SWIFT_STRIKE" => SWIFT_STRIKE_ID,
        "DISCOVERY" => DISCOVERY_ID,
        "ENLIGHTENMENT" => ENLIGHTENMENT_ID,
        "FLASH_OF_STEEL" => FLASH_OF_STEEL_ID,
        "FORETHOUGHT" => FORETHOUGHT_ID,
        "GOOD_INSTINCTS" => GOOD_INSTINCTS_ID,
        "PANACEA" => PANACEA_ID,
        "TRIP" => TRIP_ID,
        "IMPATIENCE" => IMPATIENCE_ID,
        "JACK_OF_ALL_TRADES" => JACK_OF_ALL_TRADES_ID,
        other => ContentId::new(600 + stable_pool_name_id(other)),
    }
}

fn stable_pool_name_id(name: &str) -> u64 {
    name.bytes().fold(0u64, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u64::from(byte))
    })
}

#[must_use]
pub fn random_class_card_of_type_and_rarity(
    rng: &mut StsRng,
    card_type: CardType,
    rarity: CardRarity,
) -> ContentId {
    let pool = ironclad_pool(card_type, rarity);
    assert!(!pool.is_empty(), "empty shop type/rarity pool");
    let idx = rng.random_int((pool.len() - 1) as i32) as usize;
    shop_card_content_id(pool[idx])
}

#[must_use]
pub fn random_class_card_of_type_and_rarity_with_fallback(
    rng: &mut StsRng,
    card_type: CardType,
    rarity: CardRarity,
) -> ContentId {
    let mut effective_rarity = rarity;
    if card_type == CardType::Power && rarity == CardRarity::Common {
        effective_rarity = CardRarity::Uncommon;
    }
    random_class_card_of_type_and_rarity(rng, card_type, effective_rarity)
}

#[must_use]
pub fn random_colorless_from_pool(rng: &mut StsRng, rarity: CardRarity) -> ContentId {
    let pool = match rarity {
        CardRarity::Uncommon => COLORLESS_UNCOMMON,
        CardRarity::Rare => COLORLESS_RARE,
        _ => COLORLESS_UNCOMMON,
    };
    let idx = rng.random_int((pool.len() - 1) as i32) as usize;
    shop_card_content_id(pool[idx])
}

#[must_use]
pub fn shop_card_is_colorless(content_id: ContentId) -> bool {
    COLORLESS_UNCOMMON
        .iter()
        .chain(COLORLESS_RARE.iter())
        .any(|name| shop_card_content_id(name) == content_id)
}

#[must_use]
pub fn shop_card_type(content_id: ContentId) -> Option<CardType> {
    for card_type in [CardType::Attack, CardType::Skill, CardType::Power] {
        for rarity in [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare] {
            if ironclad_pool(card_type, rarity)
                .iter()
                .any(|name| shop_card_content_id(name) == content_id)
            {
                return Some(card_type);
            }
        }
    }
    if shop_card_is_colorless(content_id) {
        crate::content::cards::get_card_definition(content_id)
            .map(|definition| definition.card_type)
    } else {
        None
    }
}

/// Target shop pricing uses each card's library rarity, not the rolled shop slot rarity.
#[must_use]
pub fn shop_card_price_rarity(content_id: ContentId) -> CardRarity {
    if let Some(rarity) = ironclad_reward_card_rarity(content_id) {
        return rarity;
    }
    for name in COLORLESS_UNCOMMON {
        if shop_card_content_id(name) == content_id {
            return CardRarity::Uncommon;
        }
    }
    for name in COLORLESS_RARE {
        if shop_card_content_id(name) == content_id {
            return CardRarity::Rare;
        }
    }
    CardRarity::Common
}

#[must_use]
pub fn assign_random_class_card_excluding(
    rng: &mut StsRng,
    card_type: CardType,
    exclude: ContentId,
    card_rarity_factor: i32,
) -> (ContentId, CardRarity) {
    loop {
        let rarity = roll_card_rarity_shop(rng, card_rarity_factor);
        let id = random_class_card_of_type_and_rarity(rng, card_type, rarity);
        if id != exclude {
            return (id, rarity);
        }
    }
}

/// Target Ironclad `CombatTypeCardPool::powers` from `sts_lightspeed` `CardPools.h`.
#[must_use]
pub fn ironclad_combat_power_discovery_pool() -> &'static [ContentId] {
    use crate::content::cards::{
        BARRICADE_ID, BERSERK_ID, BRUTALITY_ID, COMBUST_ID, CORRUPTION_ID, DARK_EMBRACE_ID,
        DEMON_FORM_ID, EVOLVE_ID, FEEL_NO_PAIN_ID, FIRE_BREATHING_ID, INFLAME_ID, JUGGERNAUT_ID,
        METALLICIZE_ID, RUPTURE_ID,
    };
    const POOL: &[ContentId] = &[
        EVOLVE_ID,
        FIRE_BREATHING_ID,
        RUPTURE_ID,
        FEEL_NO_PAIN_ID,
        DARK_EMBRACE_ID,
        COMBUST_ID,
        METALLICIZE_ID,
        INFLAME_ID,
        DEMON_FORM_ID,
        CORRUPTION_ID,
        BARRICADE_ID,
        BERSERK_ID,
        JUGGERNAUT_ID,
        BRUTALITY_ID,
    ];
    POOL
}

#[must_use]
pub fn ironclad_combat_attack_discovery_pool() -> Vec<ContentId> {
    IRONCLAD_ATTACK_COMMON
        .iter()
        .chain(IRONCLAD_ATTACK_UNCOMMON)
        .chain(IRONCLAD_ATTACK_RARE)
        .map(|name| shop_card_content_id(name))
        .collect()
}

#[must_use]
pub fn ironclad_combat_skill_discovery_pool() -> Vec<ContentId> {
    IRONCLAD_SKILL_COMMON
        .iter()
        .chain(IRONCLAD_SKILL_UNCOMMON)
        .chain(IRONCLAD_SKILL_RARE)
        .map(|name| shop_card_content_id(name))
        .collect()
}

#[must_use]
pub fn colorless_discovery_pool() -> Vec<ContentId> {
    COLORLESS_UNCOMMON
        .iter()
        .chain(COLORLESS_RARE)
        .map(|name| shop_card_content_id(name))
        .collect()
}

/// Target `sts::generateDiscoveryCards` / `DiscoveryAction.generateCardChoices`.
#[must_use]
pub fn discovery_card_choices(
    rng: &mut StsRng,
    card_type: CardType,
    count: usize,
) -> Vec<ContentId> {
    let owned_pool;
    let pool: &[ContentId] = match card_type {
        CardType::Attack => {
            owned_pool = ironclad_combat_attack_discovery_pool();
            &owned_pool
        }
        CardType::Skill => {
            owned_pool = ironclad_combat_skill_discovery_pool();
            &owned_pool
        }
        CardType::Power => ironclad_combat_power_discovery_pool(),
        CardType::Status => &[],
    };
    assert!(!pool.is_empty(), "discovery pool must not be empty");
    let mut choices = Vec::with_capacity(count);
    while choices.len() < count {
        let idx = rng.random_int((pool.len() - 1) as i32) as usize;
        let content_id = pool[idx];
        if !choices.contains(&content_id) {
            choices.push(content_id);
        }
    }
    choices
}

#[must_use]
pub fn colorless_discovery_card_choices(rng: &mut StsRng, count: usize) -> Vec<ContentId> {
    let pool = colorless_discovery_pool();
    let mut choices = Vec::with_capacity(count);
    while choices.len() < count {
        let idx = rng.random_int((pool.len() - 1) as i32) as usize;
        let content_id = pool[idx];
        if !choices.contains(&content_id) {
            choices.push(content_id);
        }
    }
    choices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        BANDAGE_UP_ID, BLIND_ID, DARK_SHACKLES_ID, DEEP_BREATH_ID, DISCOVERY_ID, ENLIGHTENMENT_ID,
        FLASH_OF_STEEL_ID, FORETHOUGHT_ID, GOOD_INSTINCTS_ID, IMPATIENCE_ID, JACK_OF_ALL_TRADES_ID,
        PANACEA_ID, SWIFT_STRIKE_ID, TRIP_ID,
    };

    #[test]
    fn bandage_up_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("BANDAGE_UP"), BANDAGE_UP_ID);
        assert!(shop_card_is_colorless(BANDAGE_UP_ID));
        assert_eq!(shop_card_type(BANDAGE_UP_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(BANDAGE_UP_ID), CardRarity::Uncommon);
    }

    #[test]
    fn blind_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("BLIND"), BLIND_ID);
        assert!(shop_card_is_colorless(BLIND_ID));
        assert_eq!(shop_card_type(BLIND_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(BLIND_ID), CardRarity::Uncommon);
    }

    #[test]
    fn good_instincts_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("GOOD_INSTINCTS"), GOOD_INSTINCTS_ID);
        assert!(shop_card_is_colorless(GOOD_INSTINCTS_ID));
        assert_eq!(shop_card_type(GOOD_INSTINCTS_ID), Some(CardType::Skill));
        assert_eq!(
            shop_card_price_rarity(GOOD_INSTINCTS_ID),
            CardRarity::Uncommon
        );
    }

    #[test]
    fn enlightenment_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("ENLIGHTENMENT"), ENLIGHTENMENT_ID);
        assert!(shop_card_is_colorless(ENLIGHTENMENT_ID));
        assert_eq!(shop_card_type(ENLIGHTENMENT_ID), Some(CardType::Skill));
        assert_eq!(
            shop_card_price_rarity(ENLIGHTENMENT_ID),
            CardRarity::Uncommon
        );
    }

    #[test]
    fn flash_of_steel_pool_key_maps_to_concrete_colorless_attack() {
        assert_eq!(shop_card_content_id("FLASH_OF_STEEL"), FLASH_OF_STEEL_ID);
        assert!(shop_card_is_colorless(FLASH_OF_STEEL_ID));
        assert_eq!(shop_card_type(FLASH_OF_STEEL_ID), Some(CardType::Attack));
        assert_eq!(
            shop_card_price_rarity(FLASH_OF_STEEL_ID),
            CardRarity::Uncommon
        );
    }

    #[test]
    fn panacea_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("PANACEA"), PANACEA_ID);
        assert!(shop_card_is_colorless(PANACEA_ID));
        assert_eq!(shop_card_type(PANACEA_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(PANACEA_ID), CardRarity::Uncommon);
    }

    #[test]
    fn trip_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("TRIP"), TRIP_ID);
        assert!(shop_card_is_colorless(TRIP_ID));
        assert_eq!(shop_card_type(TRIP_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(TRIP_ID), CardRarity::Uncommon);
    }

    #[test]
    fn swift_strike_pool_key_maps_to_concrete_colorless_attack() {
        assert_eq!(shop_card_content_id("SWIFT_STRIKE"), SWIFT_STRIKE_ID);
        assert!(shop_card_is_colorless(SWIFT_STRIKE_ID));
        assert_eq!(shop_card_type(SWIFT_STRIKE_ID), Some(CardType::Attack));
        assert_eq!(
            shop_card_price_rarity(SWIFT_STRIKE_ID),
            CardRarity::Uncommon
        );
        assert!(colorless_discovery_pool().contains(&SWIFT_STRIKE_ID));
    }

    #[test]
    fn dark_shackles_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("DARK_SHACKLES"), DARK_SHACKLES_ID);
        assert!(shop_card_is_colorless(DARK_SHACKLES_ID));
        assert_eq!(shop_card_type(DARK_SHACKLES_ID), Some(CardType::Skill));
        assert_eq!(
            shop_card_price_rarity(DARK_SHACKLES_ID),
            CardRarity::Uncommon
        );
    }

    #[test]
    fn deep_breath_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("DEEP_BREATH"), DEEP_BREATH_ID);
        assert!(shop_card_is_colorless(DEEP_BREATH_ID));
        assert_eq!(shop_card_type(DEEP_BREATH_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(DEEP_BREATH_ID), CardRarity::Uncommon);
    }

    #[test]
    fn impatience_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("IMPATIENCE"), IMPATIENCE_ID);
        assert!(shop_card_is_colorless(IMPATIENCE_ID));
        assert_eq!(shop_card_type(IMPATIENCE_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(IMPATIENCE_ID), CardRarity::Uncommon);
    }

    #[test]
    fn discovery_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("DISCOVERY"), DISCOVERY_ID);
        assert!(shop_card_is_colorless(DISCOVERY_ID));
        assert_eq!(shop_card_type(DISCOVERY_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(DISCOVERY_ID), CardRarity::Uncommon);
    }

    #[test]
    fn forethought_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(shop_card_content_id("FORETHOUGHT"), FORETHOUGHT_ID);
        assert!(shop_card_is_colorless(FORETHOUGHT_ID));
        assert_eq!(shop_card_type(FORETHOUGHT_ID), Some(CardType::Skill));
        assert_eq!(shop_card_price_rarity(FORETHOUGHT_ID), CardRarity::Uncommon);
    }

    #[test]
    fn jack_of_all_trades_pool_key_maps_to_concrete_colorless_skill() {
        assert_eq!(
            shop_card_content_id("JACK_OF_ALL_TRADES"),
            JACK_OF_ALL_TRADES_ID
        );
        assert!(shop_card_is_colorless(JACK_OF_ALL_TRADES_ID));
        assert_eq!(shop_card_type(JACK_OF_ALL_TRADES_ID), Some(CardType::Skill));
        assert_eq!(
            shop_card_price_rarity(JACK_OF_ALL_TRADES_ID),
            CardRarity::Uncommon
        );
    }
}
