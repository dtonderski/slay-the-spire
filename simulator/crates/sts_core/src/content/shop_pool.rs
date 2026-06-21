use crate::card::{CardRarity, CardType};
use crate::content::cards::{
    ANGER_ID, ARMAMENTS_ID, BARRICADE_ID, BATTLE_TRANCE_ID, BERSERK_ID, BLOODLETTING_ID,
    BLOOD_FOR_BLOOD_ID, BLUDGEON_ID, BODY_SLAM_ID, BRUTALITY_ID, BURNING_PACT_ID, CARNAGE_ID,
    CLASH_ID, CLEAVE_ID, CLOTHESLINE_ID, COMBUST_ID, CORRUPTION_ID, DARK_EMBRACE_ID, DEMON_FORM_ID,
    DISARM_ID, DOUBLE_TAP_ID, DRAMATIC_ENTRANCE_ID, DROPKICK_ID, DUAL_WIELD_ID, ENTRENCH_ID,
    EVOLVE_ID, EXHUME_ID, FEED_ID, FEEL_NO_PAIN_ID, FIEND_FIRE_ID, FIRE_BREATHING_ID,
    FLAME_BARRIER_ID, FLEX_ID, GHOSTLY_ARMOR_ID, HAVOC_ID, HEADBUTT_ID, HEAVY_BLADE_ID,
    HEMOKINESIS_ID, IMMOLATE_ID, IMPERVIOUS_ID, INFERNAL_BLADE_ID, INFLAME_ID, INTIMIDATE_ID,
    IRON_WAVE_ID, JUGGERNAUT_ID, LIMIT_BREAK_ID, METALLICIZE_ID, OFFERING_ID, PERFECTED_STRIKE_ID,
    POMMEL_STRIKE_ID, POWER_THROUGH_ID, PUMMEL_ID, RAGE_ID, RAMPAGE_ID, REAPER_ID,
    RECKLESS_CHARGE_ID, RUPTURE_ID, SEARING_BLOW_ID, SECOND_WIND_ID, SEEING_RED_ID, SENTINEL_ID,
    SEVER_SOUL_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID, SWORD_BOOMERANG_ID,
    THUNDERCLAP_ID, TRUE_GRIT_ID, TWIN_STRIKE_ID, UPPERCUT_ID, WARCRY_ID, WHIRLWIND_ID,
    WILD_STRIKE_ID,
};
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
