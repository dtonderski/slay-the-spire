//! Version 1 public content ids. Order matches the current Python catalogs.
//! Do not reorder without bumping CONTENT_VOCABULARY_VERSION.
use std::collections::HashMap;
use std::sync::OnceLock;

use pyo3::prelude::*;

pub const CONTENT_VOCABULARY_VERSION: u32 = 1;

pub static CARD_KEYS: &[&str] = &[
    "AFTER_IMAGE",
    "APOTHEOSIS",
    "APOTHEOSIS+",
    "Anger",
    "Anger+",
    "Apparition",
    "Apparition+",
    "Armaments",
    "Armaments+",
    "Ascenders Bane",
    "BACKFLIP",
    "BANDAGE_UP",
    "BANDAGE_UP+",
    "BANE",
    "BARRICADE",
    "BARRICADE+",
    "BEAM_CELL",
    "BERSERK",
    "BERSERK+",
    "BIASED_COGNITION",
    "BLASPHEMY",
    "BLASPHEMY+",
    "BLIND",
    "BLIND+",
    "BLOOD_FOR_BLOOD",
    "BLOOD_FOR_BLOOD+",
    "BLUDGEON",
    "BLUDGEON+",
    "BODY_SLAM",
    "BODY_SLAM+",
    "BOOT_SEQUENCE",
    "BOWLING_BASH",
    "BRUTALITY",
    "BRUTALITY+",
    "Bash",
    "Bash+",
    "Battle Trance",
    "Battle Trance+",
    "Bite",
    "Bite+",
    "Bloodletting",
    "Bloodletting+",
    "Burn",
    "Burning Pact",
    "Burning Pact+",
    "CALTROPS",
    "CAPACITOR",
    "CARNAGE",
    "CARNAGE+",
    "CHARGE_BATTERY",
    "CHRYSALIS",
    "CHRYSALIS+",
    "CLASH",
    "CLASH+",
    "CLOAK_AND_DAGGER",
    "CLOTHESLINE",
    "CLOTHESLINE+",
    "COMBUST",
    "COMBUST+",
    "COMPILE_DRIVER",
    "CONCLUDE",
    "COOLHEADED",
    "CORRUPTION",
    "CORRUPTION+",
    "CREATIVE_AI",
    "CRESCENDO",
    "CRIPPLING_CLOUD",
    "CRUSH_JOINTS",
    "Cleave",
    "Cleave+",
    "Clumsy",
    "CurseOfTheBell",
    "DAGGER_SPRAY",
    "DARKNESS",
    "DARK_SHACKLES",
    "DARK_SHACKLES+",
    "DEADLY_POISON",
    "DEEP_BREATH",
    "DEEP_BREATH+",
    "DISARM",
    "DISARM+",
    "DISCOVERY",
    "DISCOVERY+",
    "DOPPELGANGER",
    "DOUBLE_TAP",
    "DOUBLE_TAP+",
    "DROPKICK",
    "DROPKICK+",
    "Dark Embrace",
    "Dark Embrace+",
    "Dazed",
    "Decay",
    "Defend_R",
    "Defend_R+",
    "Demon Form",
    "Demon Form+",
    "Doubt",
    "Dramatic Entrance",
    "Dramatic Entrance+",
    "Dual Wield",
    "Dual Wield+",
    "EMPTY_BODY",
    "EMPTY_MIND",
    "ENLIGHTENMENT",
    "ENLIGHTENMENT+",
    "ENTRENCH",
    "ENTRENCH+",
    "EQUILIBRIUM",
    "EVALUATE",
    "EVISCERATE",
    "EVOLVE",
    "EVOLVE+",
    "EXHUME",
    "EXHUME+",
    "Ethereal_Strike",
    "FASTING",
    "FEAR_NO_EVIL",
    "FEED",
    "FEED+",
    "FIEND_FIRE",
    "FIEND_FIRE+",
    "FINESSE",
    "FINESSE+",
    "FIRE_BREATHING",
    "FIRE_BREATHING+",
    "FLAME_BARRIER",
    "FLAME_BARRIER+",
    "FLURRY_OF_BLOWS",
    "FLYING_SLEEVES",
    "FOLLOW_UP",
    "FORETHOUGHT",
    "FORETHOUGHT+",
    "Feel No Pain",
    "Feel No Pain+",
    "Flash of Steel",
    "Flash of Steel+",
    "Flex",
    "Flex+",
    "GHOSTLY_ARMOR",
    "GHOSTLY_ARMOR+",
    "GOOD_INSTINCTS",
    "GOOD_INSTINCTS+",
    "GO_FOR_THE_EYES",
    "HALT",
    "HAND_OF_GREED",
    "HAND_OF_GREED+",
    "HEAVY_BLADE",
    "HEAVY_BLADE+",
    "HOLOGRAM",
    "Havoc",
    "Havoc+",
    "Headbutt",
    "Headbutt+",
    "Hemokinesis",
    "Hemokinesis+",
    "IMPATIENCE",
    "IMPATIENCE+",
    "IMPERVIOUS",
    "IMPERVIOUS+",
    "INFERNAL_BLADE",
    "INFERNAL_BLADE+",
    "INSIGHT",
    "INTIMIDATE",
    "INTIMIDATE+",
    "IRON_WAVE",
    "IRON_WAVE+",
    "Immolate",
    "Immolate+",
    "Inflame",
    "Inflame+",
    "Injury",
    "J.A.X.",
    "J.A.X.+",
    "JACK_OF_ALL_TRADES",
    "JACK_OF_ALL_TRADES+",
    "JUDGEMENT",
    "JUGGERNAUT",
    "JUGGERNAUT+",
    "JUST_LUCKY",
    "LEAP",
    "LEG_SWEEP",
    "LESSON_LEARNED",
    "LIKE_WATER",
    "Limit Break",
    "Limit Break+",
    "MADNESS",
    "MADNESS+",
    "MAGNETISM",
    "MAGNETISM+",
    "MALAISE",
    "MASTER_OF_STRATEGY",
    "MASTER_OF_STRATEGY+",
    "MAYHEM",
    "MAYHEM+",
    "METAMORPHOSIS",
    "METAMORPHOSIS+",
    "Metallicize",
    "Metallicize+",
    "Mind Blast",
    "Mind Blast+",
    "NIRVANA",
    "Necronomicurse",
    "Normality",
    "Offering",
    "Offering+",
    "PANACEA",
    "PANACEA+",
    "PANACHE",
    "PANACHE+",
    "PANIC_BUTTON",
    "PANIC_BUTTON+",
    "PERFECTED_STRIKE",
    "PERFECTED_STRIKE+",
    "POISONED_STAB",
    "POWER_THROUGH",
    "POWER_THROUGH+",
    "PRAY",
    "PREPARED",
    "PRESSURE_POINTS",
    "PROSTRATE",
    "PROTECT",
    "PUMMEL",
    "PUMMEL+",
    "PURITY",
    "PURITY+",
    "Pain",
    "Parasite",
    "Pommel Strike",
    "Pommel Strike+",
    "QUICK_SLASH",
    "RAGE",
    "RAGE+",
    "RAMPAGE",
    "RAMPAGE+",
    "REAPER",
    "REAPER+",
    "RECKLESS_CHARGE",
    "RECKLESS_CHARGE+",
    "RECURSION",
    "RECYCLE",
    "RIP_AND_TEAR",
    "RUPTURE",
    "RUPTURE+",
    "Regret",
    "Retain_Defend",
    "RitualDagger",
    "SADISTIC_NATURE",
    "SADISTIC_NATURE+",
    "SANDS_OF_TIME",
    "SANDS_OF_TIME+",
    "SCRAPE",
    "SECOND_WIND",
    "SECOND_WIND+",
    "SECRET_TECHNIQUE",
    "SECRET_TECHNIQUE+",
    "SECRET_WEAPON",
    "SECRET_WEAPON+",
    "SHIV",
    "SHOCKWAVE",
    "SHOCKWAVE+",
    "SKEWER",
    "SKIM",
    "SLICE",
    "SNEAKY_STRIKE",
    "STATIC_DISCHARGE",
    "STEAM_BARRIER",
    "STORM",
    "STORM_OF_STEEL",
    "STREAMLINE",
    "SWEEPING_BEAM",
    "Searing Blow",
    "Searing Blow+",
    "Seeing Red",
    "Seeing Red+",
    "Sentinel",
    "Sentinel+",
    "Sever Soul",
    "Sever Soul+",
    "Shame",
    "Shrug It Off",
    "Shrug It Off+",
    "Slimed",
    "Spot Weakness",
    "Spot Weakness+",
    "Strike_R",
    "Strike_R+",
    "Swift Strike",
    "Swift Strike+",
    "Sword Boomerang",
    "Sword Boomerang+",
    "THE_BOMB",
    "THE_BOMB+",
    "THINKING_AHEAD",
    "THINKING_AHEAD+",
    "THIRD_EYE",
    "THUNDERCLAP+",
    "TRANQUILITY",
    "TRANSMUTATION",
    "TRANSMUTATION+",
    "TRIP",
    "TRIP+",
    "Thunderclap",
    "True Grit",
    "True Grit+",
    "Twin Strike",
    "Twin Strike+",
    "UNLOAD",
    "Uppercut",
    "Uppercut+",
    "VIOLENCE",
    "VIOLENCE+",
    "Void",
    "WALLOP",
    "WILD_STRIKE",
    "WILD_STRIKE+",
    "WINDMILL_STRIKE",
    "Warcry",
    "Warcry+",
    "Whirlwind",
    "Whirlwind+",
    "Wound",
    "Writhe",
];

pub static MONSTER_KEYS: &[&str] = &[
    "Acid Slime (S)",
    "Awakened One",
    "Bear",
    "Blue Slaver",
    "Book of Stabbing",
    "Bronze Automaton",
    "Bronze Orb",
    "Byrd",
    "Centurion",
    "Chosen",
    "Corrupt Heart",
    "Cultist",
    "Dagger",
    "Darkling",
    "Deca",
    "Donu",
    "Exploder",
    "Fixed Simple Monster",
    "Fungi Beast",
    "Giant Head",
    "Green Louse",
    "Gremlin Fat",
    "Gremlin Leader",
    "Gremlin Nob",
    "Gremlin Thief",
    "Gremlin Tsundere",
    "Gremlin Warrior",
    "Gremlin Wizard",
    "Guardian",
    "Hexaghost",
    "Jaw Worm",
    "Lagavulin",
    "Looter",
    "Mugger",
    "Mystic",
    "Nemesis",
    "Orb Walker",
    "Pointy",
    "Red Louse",
    "Red Slaver",
    "Reptomancer",
    "Repulsor",
    "Romeo",
    "Sentry",
    "Shelled Parasite",
    "Slime Boss",
    "Snake Plant",
    "Snecko",
    "Spheric Guardian",
    "Spike Slime",
    "Spiker",
    "Spire Growth",
    "Spire Shield",
    "Spire Spear",
    "Taskmaster",
    "The Champ",
    "The Collector",
    "The Maw",
    "Time Eater",
    "Torch Head",
    "Transient",
    "Writhing Mass",
];

pub static RELIC_KEYS: &[&str] = &[
    "Akabeko",
    "Anchor",
    "Ancient Tea Set",
    "Art of War",
    "Astrolabe",
    "Bag of Marbles",
    "Bag of Preparation",
    "Bird-Faced Urn",
    "Black Blood",
    "Black Star",
    "Blood Vial",
    "Bloody Idol",
    "Blue Candle",
    "Bottled Flame",
    "Bottled Lightning",
    "Bottled Tornado",
    "Brimstone",
    "Bronze Scales",
    "Burning Blood",
    "Busted Crown",
    "Calipers",
    "Calling Bell",
    "Captain's Wheel",
    "Cauldron",
    "Centennial Puzzle",
    "Ceramic Fish",
    "Champion Belt",
    "Charon's Ashes",
    "Chemical X",
    "Circlet",
    "Clockwork Souvenir",
    "Coffee Dripper",
    "Cracked Core",
    "Cultist Headpiece",
    "Cursed Key",
    "Darkstone Periapt",
    "Dead Branch",
    "Dolly's Mirror",
    "Dream Catcher",
    "Du-Vu Doll",
    "Ectoplasm",
    "Empty Cage",
    "Enchiridion",
    "Eternal Feather",
    "FaceOfCleric",
    "Fossilized Helix",
    "Frozen Core",
    "Frozen Egg",
    "Frozen Eye",
    "Fusion Hammer",
    "Gambling Chip",
    "Ginger",
    "Girya",
    "Golden Idol",
    "Gremlin Horn",
    "GremlinMask",
    "Hand Drill",
    "Happy Flower",
    "Holy Water",
    "Horn Cleat",
    "Ice Cream",
    "Incense Burner",
    "Ink Bottle",
    "Juzu Bracelet",
    "Kunai",
    "Lantern",
    "Lee's Waffle",
    "Letter Opener",
    "Lizard Tail",
    "Magic Flower",
    "Mango",
    "Mark of Pain",
    "Mark of the Bloom",
    "Matryoshka",
    "Maw Bank",
    "Meal Ticket",
    "Meat on the Bone",
    "Medical Kit",
    "Membership Card",
    "Mercury Hourglass",
    "Molten Egg",
    "Mummified Hand",
    "Mutagenic Strength",
    "N'loth's Gift",
    "Necronomicon",
    "Neow's Lament",
    "Nilry's Codex",
    "NlothsMask",
    "Nunchaku",
    "Odd Mushroom",
    "Oddly Smooth Stone",
    "Old Coin",
    "Omamori",
    "Orange Pellets",
    "Orichalcum",
    "Ornamental Fan",
    "Orrery",
    "Pandora's Box",
    "Pantograph",
    "Paper Phrog",
    "Peace Pipe",
    "Pear",
    "Pen Nib",
    "Philosopher's Stone",
    "Pocketwatch",
    "Potion Belt",
    "Prayer Wheel",
    "Preserved Insect",
    "Prismatic Shard",
    "Pure Water",
    "Question Card",
    "Red Circlet",
    "Red Mask",
    "Red Skull",
    "Regal Pillow",
    "Ring of the Serpent",
    "Ring of the Snake",
    "Runic Cube",
    "Runic Dome",
    "Runic Pyramid",
    "Sacred Bark",
    "Self-Forming Clay",
    "Shovel",
    "Shuriken",
    "Singing Bowl",
    "Slaver's Collar",
    "Sling of Courage",
    "Smiling Mask",
    "Snecko Eye",
    "Sozu",
    "Spirit Poop",
    "Ssserpent Head",
    "Stone Calendar",
    "Strange Spoon",
    "Strawberry",
    "Strike Dummy",
    "Sundial",
    "The Abacus",
    "The Boot",
    "The Courier",
    "Thread and Needle",
    "Tiny Chest",
    "Tiny House",
    "Toolbox",
    "Torii",
    "Toxic Egg",
    "Toy Ornithopter",
    "Tungsten Rod",
    "Turnip",
    "Unceasing Top",
    "Vajra",
    "Velvet Choker",
    "War Paint",
    "Warped Tongs",
    "Whetstone",
    "White Beast Statue",
    "Wing Boots",
];

pub static POTION_KEYS: &[&str] = &[
    "ancient",
    "attack",
    "blessing_of_the_forge",
    "block",
    "blood",
    "colorless",
    "cultist",
    "dexterity",
    "distilled_chaos",
    "duplication",
    "elixir",
    "energy",
    "entropic_brew",
    "essence_of_steel",
    "explosive",
    "fairy_in_a_bottle",
    "fear",
    "fire",
    "flex",
    "fruit_juice",
    "gamblers_brew",
    "heart_of_iron",
    "liquid_bronze",
    "liquid_memories",
    "power",
    "regen",
    "skill",
    "smoke_bomb",
    "snecko_oil",
    "speed",
    "strength",
    "swift",
    "weak",
];

pub static POWER_KEYS: &[&str] = &[
    "after_image",
    "anger",
    "artifact",
    "barricade",
    "berserk",
    "bomb_1_damage",
    "bomb_2_damage",
    "bomb_3_damage",
    "brutality",
    "buffer",
    "combust",
    "combust_damage",
    "confusion",
    "constricted",
    "corruption",
    "creative_ai",
    "curl_up",
    "dark_embrace",
    "demon_form",
    "dexterity",
    "double_tap",
    "duplication",
    "entangled",
    "evolve",
    "explosive",
    "fasting",
    "feel_no_pain",
    "fire_breathing",
    "flight",
    "focus",
    "frail",
    "hex",
    "intangible",
    "juggernaut",
    "like_water",
    "lock_on",
    "lose_dexterity",
    "lose_strength",
    "magnetism",
    "malleable",
    "mantra",
    "mark",
    "mayhem",
    "metallicize",
    "mode_shift",
    "nirvana",
    "no_block",
    "no_draw",
    "painful_stabs",
    "panache",
    "panache_cards_played",
    "plated_armor",
    "poison",
    "rage",
    "regen",
    "restore_strength",
    "ritual",
    "rupture",
    "sadistic_nature",
    "slow",
    "spikes",
    "spore_cloud",
    "static_discharge",
    "storm",
    "strength",
    "strength_up",
    "temporary_thorns",
    "thorns",
    "vigor",
    "vulnerable",
    "weak",
    "wrath",
];

pub static COUNTER_KEYS: &[&str] = &[
    "active",
    "armed",
    "attack_played",
    "attacks",
    "attacks_last_turn",
    "attacks_played_this_turn",
    "attacks_this_combat",
    "attacks_this_turn",
    "available",
    "cards",
    "cards_discarded_this_turn",
    "cards_last_turn",
    "cards_played_this_turn",
    "cards_this_turn",
    "charges",
    "charges_remaining",
    "chests_remaining",
    "combats_remaining",
    "lifts",
    "next_turn_block",
    "player_turns_started",
    "power_played",
    "rooms",
    "shuffles",
    "skill_played",
    "skills_this_turn",
    "triggers",
    "turns",
    "used_this_turn",
];

pub static SLIME_SIZES: &[&str] = &["Small", "Medium", "Large"];

pub static INTENT_CATEGORIES: &[&str] = &[
    "unknown",
    "attack",
    "attack_buff",
    "attack_debuff",
    "attack_defend",
    "buff",
    "debuff",
    "strong_debuff",
    "defend",
    "defend_buff",
    "escape",
    "sleep",
    "stun",
];

pub static SELECTION_KINDS: &[&str] = &[
    "potion_attack_reward",
    "potion_skill_reward",
    "potion_power_reward",
    "potion_colorless_reward",
    "toolbox_reward",
    "discovery_reward",
    "warcry_put_on_draw",
    "armaments_upgrade",
    "forethought_put_on_draw",
    "forethought_put_any_on_draw",
    "thinking_ahead_put_on_draw",
    "prepared_discard",
    "dual_wield_copy",
    "secret_technique_skill_to_hand",
    "secret_weapon_attack_to_hand",
    "scry",
    "liquid_memories_return_to_hand",
    "headbutt_put_on_draw",
    "hologram_return_to_hand",
    "exhaust",
    "gambling_chip",
    "exhume_return_to_hand",
    "purity_exhaust_up_to_three",
    "burning_pact_draw_two",
    "burning_pact_draw_three",
    "true_grit_exhaust_one",
    "recycle_exhaust_one",
];

pub fn card_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            CARD_KEYS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn monster_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            MONSTER_KEYS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn relic_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            RELIC_KEYS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn power_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            POWER_KEYS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn counter_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            COUNTER_KEYS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn slime_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            SLIME_SIZES
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn intent_category_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            INTENT_CATEGORIES
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn selection_kind_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            SELECTION_KINDS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64))
                .collect()
        })
        .get(key)
        .copied()
}

/// Empty potion is 0, matching the Python encoder's `None` row. Present potions are `index + 1`.
pub fn potion_id(key: &str) -> Option<i64> {
    static INDEX: OnceLock<HashMap<&'static str, i64>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            POTION_KEYS
                .iter()
                .enumerate()
                .map(|(index, value)| (*value, index as i64 + 1))
                .collect()
        })
        .get(key)
        .copied()
}

pub fn intent_id(kind: &str) -> Option<i64> {
    match kind {
        "hidden" => Some(0),
        "none" => Some(1),
        other => intent_category_id(other).map(|index| index + 2),
    }
}

pub fn slime_catalog_id(size: Option<&str>) -> Option<i64> {
    match size {
        None => Some(0),
        Some(value) => slime_id(value).map(|index| index + 1),
    }
}

pub fn selection_catalog_id(kind: Option<&str>) -> Option<i64> {
    match kind {
        None => Some(0),
        Some(value) => selection_kind_id(value).map(|index| index + 1),
    }
}

#[pyfunction]
pub fn content_vocabulary_version() -> u32 {
    CONTENT_VOCABULARY_VERSION
}
