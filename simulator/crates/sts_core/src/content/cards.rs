use crate::{
    card::{
        CardDefinition, CardKeywords, CardRarity, CardType, CardValues, TargetRequirement,
        CARD_KEYWORDS_NONE,
    },
    ContentId,
};

pub const STRIKE_R_ID: ContentId = ContentId::new(1);
pub const DEFEND_R_ID: ContentId = ContentId::new(2);
pub const BASH_ID: ContentId = ContentId::new(3);
pub const WOUND_ID: ContentId = ContentId::new(4);
pub const DAZED_ID: ContentId = ContentId::new(5);
pub const BURN_ID: ContentId = ContentId::new(6);
pub const SLIMED_ID: ContentId = ContentId::new(7);
pub const REGRET_ID: ContentId = ContentId::new(62);
pub const DOUBT_ID: ContentId = ContentId::new(63);
pub const CURSE_OF_THE_BELL_ID: ContentId = ContentId::new(64);
pub const ASCENDERS_BANE_ID: ContentId = ContentId::new(61);
pub const ETHEREAL_STRIKE_ID: ContentId = ContentId::new(8);
pub const RETAIN_DEFEND_ID: ContentId = ContentId::new(9);
pub const ANGER_ID: ContentId = ContentId::new(10);
pub const CLEAVE_ID: ContentId = ContentId::new(11);
pub const TWIN_STRIKE_ID: ContentId = ContentId::new(12);
pub const ANGER_PLUS_ID: ContentId = ContentId::new(13);
pub const CLEAVE_PLUS_ID: ContentId = ContentId::new(14);
pub const TWIN_STRIKE_PLUS_ID: ContentId = ContentId::new(15);
pub const SHRUG_IT_OFF_ID: ContentId = ContentId::new(16);
pub const TRUE_GRIT_ID: ContentId = ContentId::new(17);
pub const BURNING_PACT_ID: ContentId = ContentId::new(18);
pub const FEEL_NO_PAIN_ID: ContentId = ContentId::new(19);
pub const DARK_EMBRACE_ID: ContentId = ContentId::new(20);
pub const POMMEL_STRIKE_ID: ContentId = ContentId::new(21);
pub const BATTLE_TRANCE_ID: ContentId = ContentId::new(22);
pub const SEEING_RED_ID: ContentId = ContentId::new(23);
pub const POMMEL_STRIKE_PLUS_ID: ContentId = ContentId::new(24);
pub const BATTLE_TRANCE_PLUS_ID: ContentId = ContentId::new(25);
pub const SEEING_RED_PLUS_ID: ContentId = ContentId::new(26);
pub const INFLAME_ID: ContentId = ContentId::new(27);
pub const FLEX_ID: ContentId = ContentId::new(28);
pub const SPOT_WEAKNESS_ID: ContentId = ContentId::new(29);
pub const INFLAME_PLUS_ID: ContentId = ContentId::new(30);
pub const FLEX_PLUS_ID: ContentId = ContentId::new(31);
pub const SPOT_WEAKNESS_PLUS_ID: ContentId = ContentId::new(32);
pub const WHIRLWIND_ID: ContentId = ContentId::new(33);
pub const WHIRLWIND_PLUS_ID: ContentId = ContentId::new(34);
pub const STRIKE_R_PLUS_ID: ContentId = ContentId::new(35);
pub const HAVOC_ID: ContentId = ContentId::new(36);
pub const HAVOC_PLUS_ID: ContentId = ContentId::new(37);
pub const WARCRY_ID: ContentId = ContentId::new(38);
pub const WARCRY_PLUS_ID: ContentId = ContentId::new(39);
pub const DUAL_WIELD_ID: ContentId = ContentId::new(40);
pub const DUAL_WIELD_PLUS_ID: ContentId = ContentId::new(41);
pub const SEARING_BLOW_ID: ContentId = ContentId::new(42);
pub const SEARING_BLOW_PLUS_ID: ContentId = ContentId::new(43);
pub const DRAMATIC_ENTRANCE_ID: ContentId = ContentId::new(44);
pub const SWIFT_STRIKE_ID: ContentId = ContentId::new(45);
pub const BANDAGE_UP_ID: ContentId = ContentId::new(1_802_661_242_803_912);
pub const BLIND_ID: ContentId = ContentId::new(63_289_741);
pub const FINESSE_ID: ContentId = ContentId::new(64_289_358_915);
pub const FLASH_OF_STEEL_ID: ContentId = ContentId::new(18_371_492_448_625_970_986);
pub const GOOD_INSTINCTS_ID: ContentId = ContentId::new(8_602_552_533_669_984_653);

pub const IRON_WAVE_ID: ContentId = ContentId::new(100);
pub const BODY_SLAM_ID: ContentId = ContentId::new(101);
pub const CLASH_ID: ContentId = ContentId::new(102);
pub const THUNDERCLAP_ID: ContentId = ContentId::new(103);
pub const CLOTHESLINE_ID: ContentId = ContentId::new(104);
pub const ARMAMENTS_ID: ContentId = ContentId::new(105);
pub const HEADBUTT_ID: ContentId = ContentId::new(106);
pub const WILD_STRIKE_ID: ContentId = ContentId::new(107);
pub const HEAVY_BLADE_ID: ContentId = ContentId::new(108);
pub const PERFECTED_STRIKE_ID: ContentId = ContentId::new(109);
pub const SWORD_BOOMERANG_ID: ContentId = ContentId::new(110);
pub const POWER_THROUGH_ID: ContentId = ContentId::new(111);
pub const INFERNAL_BLADE_ID: ContentId = ContentId::new(112);
pub const RECKLESS_CHARGE_ID: ContentId = ContentId::new(113);
pub const HEMOKINESIS_ID: ContentId = ContentId::new(114);
pub const INTIMIDATE_ID: ContentId = ContentId::new(115);
pub const BLOOD_FOR_BLOOD_ID: ContentId = ContentId::new(116);
pub const FLAME_BARRIER_ID: ContentId = ContentId::new(117);
pub const PUMMEL_ID: ContentId = ContentId::new(118);
pub const METALLICIZE_ID: ContentId = ContentId::new(119);
pub const SHOCKWAVE_ID: ContentId = ContentId::new(120);
pub const RAMPAGE_ID: ContentId = ContentId::new(121);
pub const SEVER_SOUL_ID: ContentId = ContentId::new(122);
pub const COMBUST_ID: ContentId = ContentId::new(123);
pub const DISARM_ID: ContentId = ContentId::new(124);
pub const RAGE_ID: ContentId = ContentId::new(125);
pub const ENTRENCH_ID: ContentId = ContentId::new(126);
pub const SENTINEL_ID: ContentId = ContentId::new(127);
pub const SECOND_WIND_ID: ContentId = ContentId::new(128);
pub const RUPTURE_ID: ContentId = ContentId::new(129);
pub const BLOODLETTING_ID: ContentId = ContentId::new(130);
pub const CARNAGE_ID: ContentId = ContentId::new(131);
pub const DROPKICK_ID: ContentId = ContentId::new(132);
pub const FIRE_BREATHING_ID: ContentId = ContentId::new(133);
pub const GHOSTLY_ARMOR_ID: ContentId = ContentId::new(134);
pub const UPPERCUT_ID: ContentId = ContentId::new(135);
pub const EVOLVE_ID: ContentId = ContentId::new(136);
pub const DOUBLE_TAP_ID: ContentId = ContentId::new(137);
pub const DEMON_FORM_ID: ContentId = ContentId::new(138);
pub const BLUDGEON_ID: ContentId = ContentId::new(139);
pub const FEED_ID: ContentId = ContentId::new(140);
pub const LIMIT_BREAK_ID: ContentId = ContentId::new(141);
pub const CORRUPTION_ID: ContentId = ContentId::new(142);
pub const BARRICADE_ID: ContentId = ContentId::new(143);
pub const FIEND_FIRE_ID: ContentId = ContentId::new(144);
pub const BERSERK_ID: ContentId = ContentId::new(145);
pub const IMPERVIOUS_ID: ContentId = ContentId::new(146);
pub const JUGGERNAUT_ID: ContentId = ContentId::new(147);
pub const BRUTALITY_ID: ContentId = ContentId::new(148);
pub const REAPER_ID: ContentId = ContentId::new(149);
pub const EXHUME_ID: ContentId = ContentId::new(150);
pub const OFFERING_ID: ContentId = ContentId::new(151);
pub const IMMOLATE_ID: ContentId = ContentId::new(152);

pub const STRIKE_R: CardDefinition = CardDefinition {
    id: STRIKE_R_ID,
    key: "Strike_R",
    name: "Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const STRIKE_R_PLUS: CardDefinition = CardDefinition {
    id: STRIKE_R_PLUS_ID,
    key: "Strike_R+",
    name: "Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DEFEND_R: CardDefinition = CardDefinition {
    id: DEFEND_R_ID,
    key: "Defend_R",
    name: "Defend",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BASH: CardDefinition = CardDefinition {
    id: BASH_ID,
    key: "Bash",
    name: "Bash",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: Some(2),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WOUND: CardDefinition = CardDefinition {
    id: WOUND_ID,
    key: "Wound",
    name: "Wound",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        unplayable: true,
        ethereal: false,
        exhaust: false,
        retain: false,
    },
};

pub const ASCENDERS_BANE: CardDefinition = CardDefinition {
    id: ASCENDERS_BANE_ID,
    key: "Ascenders Bane",
    name: "Ascender's Bane",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        unplayable: true,
        ethereal: true,
        exhaust: false,
        retain: false,
    },
};

pub const DAZED: CardDefinition = CardDefinition {
    id: DAZED_ID,
    key: "Dazed",
    name: "Dazed",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        unplayable: true,
        ethereal: true,
        exhaust: false,
        retain: false,
    },
};

/// Burn status deals this much HP loss per copy in hand at end of turn.
pub const BURN_END_TURN_DAMAGE: i32 = 2;

/// Combust loses this much player HP per stack at end of turn.
pub const COMBUST_HP_LOSS: i32 = 1;
/// Combust deals this much damage to all living enemies per stack at end of turn.
pub const COMBUST_DAMAGE: i32 = 5;

pub const BURN: CardDefinition = CardDefinition {
    id: BURN_ID,
    key: "Burn",
    name: "Burn",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(BURN_END_TURN_DAMAGE),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        unplayable: true,
        ethereal: false,
        exhaust: false,
        retain: false,
    },
};

pub const REGRET: CardDefinition = CardDefinition {
    id: REGRET_ID,
    key: "Regret",
    name: "Regret",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: true,
    },
};

pub const DOUBT: CardDefinition = CardDefinition {
    id: DOUBT_ID,
    key: "Doubt",
    name: "Doubt",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: true,
    },
};

pub const CURSE_OF_THE_BELL: CardDefinition = CardDefinition {
    id: CURSE_OF_THE_BELL_ID,
    key: "CurseOfTheBell",
    name: "Curse of the Bell",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: true,
    },
};

pub const SLIMED: CardDefinition = CardDefinition {
    id: SLIMED_ID,
    key: "Slimed",
    name: "Slimed",
    cost: 1,
    card_type: CardType::Status,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(0),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const ETHEREAL_STRIKE: CardDefinition = CardDefinition {
    id: ETHEREAL_STRIKE_ID,
    key: "Ethereal_Strike",
    name: "Ethereal Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: true,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const RETAIN_DEFEND: CardDefinition = CardDefinition {
    id: RETAIN_DEFEND_ID,
    key: "Retain_Defend",
    name: "Retain Defend",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: false,
        retain: true,
        unplayable: false,
    },
};

pub const ANGER: CardDefinition = CardDefinition {
    id: ANGER_ID,
    key: "Anger",
    name: "Anger",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CLEAVE: CardDefinition = CardDefinition {
    id: CLEAVE_ID,
    key: "Cleave",
    name: "Cleave",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TWIN_STRIKE: CardDefinition = CardDefinition {
    id: TWIN_STRIKE_ID,
    key: "Twin Strike",
    name: "Twin Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const ANGER_PLUS: CardDefinition = CardDefinition {
    id: ANGER_PLUS_ID,
    key: "Anger+",
    name: "Anger+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CLEAVE_PLUS: CardDefinition = CardDefinition {
    id: CLEAVE_PLUS_ID,
    key: "Cleave+",
    name: "Cleave+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TWIN_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: TWIN_STRIKE_PLUS_ID,
    key: "Twin Strike+",
    name: "Twin Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SHRUG_IT_OFF: CardDefinition = CardDefinition {
    id: SHRUG_IT_OFF_ID,
    key: "Shrug It Off",
    name: "Shrug It Off",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(8),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TRUE_GRIT: CardDefinition = CardDefinition {
    id: TRUE_GRIT_ID,
    key: "True Grit",
    name: "True Grit",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(7),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BURNING_PACT: CardDefinition = CardDefinition {
    id: BURNING_PACT_ID,
    key: "Burning Pact",
    name: "Burning Pact",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FEEL_NO_PAIN: CardDefinition = CardDefinition {
    id: FEEL_NO_PAIN_ID,
    key: "Feel No Pain",
    name: "Feel No Pain",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DARK_EMBRACE: CardDefinition = CardDefinition {
    id: DARK_EMBRACE_ID,
    key: "Dark Embrace",
    name: "Dark Embrace",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const COMBUST: CardDefinition = CardDefinition {
    id: COMBUST_ID,
    key: "COMBUST",
    name: "Combust",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(COMBUST_DAMAGE),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DEMON_FORM: CardDefinition = CardDefinition {
    id: DEMON_FORM_ID,
    key: "Demon Form",
    name: "Demon Form",
    cost: 3,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const EVOLVE: CardDefinition = CardDefinition {
    id: EVOLVE_ID,
    key: "EVOLVE",
    name: "Evolve",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CORRUPTION: CardDefinition = CardDefinition {
    id: CORRUPTION_ID,
    key: "CORRUPTION",
    name: "Corruption",
    cost: 3,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BARRICADE: CardDefinition = CardDefinition {
    id: BARRICADE_ID,
    key: "BARRICADE",
    name: "Barricade",
    cost: 3,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BERSERK: CardDefinition = CardDefinition {
    id: BERSERK_ID,
    key: "BERSERK",
    name: "Berserk",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: Some(2),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const RUPTURE: CardDefinition = CardDefinition {
    id: RUPTURE_ID,
    key: "RUPTURE",
    name: "Rupture",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const JUGGERNAUT: CardDefinition = CardDefinition {
    id: JUGGERNAUT_ID,
    key: "JUGGERNAUT",
    name: "Juggernaut",
    cost: 2,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BRUTALITY: CardDefinition = CardDefinition {
    id: BRUTALITY_ID,
    key: "BRUTALITY",
    name: "Brutality",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DOUBLE_TAP: CardDefinition = CardDefinition {
    id: DOUBLE_TAP_ID,
    key: "DOUBLE_TAP",
    name: "Double Tap",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FIRE_BREATHING: CardDefinition = CardDefinition {
    id: FIRE_BREATHING_ID,
    key: "FIRE_BREATHING",
    name: "Fire Breathing",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const LIMIT_BREAK: CardDefinition = CardDefinition {
    id: LIMIT_BREAK_ID,
    key: "Limit Break",
    name: "Limit Break",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        unplayable: false,
        ethereal: false,
        exhaust: true,
        retain: false,
    },
};

pub const OFFERING: CardDefinition = CardDefinition {
    id: OFFERING_ID,
    key: "Offering",
    name: "Offering",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        unplayable: false,
        ethereal: false,
        exhaust: true,
        retain: false,
    },
};

pub const ARMAMENTS: CardDefinition = CardDefinition {
    id: ARMAMENTS_ID,
    key: "Armaments",
    name: "Armaments",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const HEADBUTT: CardDefinition = CardDefinition {
    id: HEADBUTT_ID,
    key: "Headbutt",
    name: "Headbutt",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const POMMEL_STRIKE: CardDefinition = CardDefinition {
    id: POMMEL_STRIKE_ID,
    key: "Pommel Strike",
    name: "Pommel Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BATTLE_TRANCE: CardDefinition = CardDefinition {
    id: BATTLE_TRANCE_ID,
    key: "Battle Trance",
    name: "Battle Trance",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SEEING_RED: CardDefinition = CardDefinition {
    id: SEEING_RED_ID,
    key: "Seeing Red",
    name: "Seeing Red",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const POMMEL_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: POMMEL_STRIKE_PLUS_ID,
    key: "Pommel Strike+",
    name: "Pommel Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(12),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BATTLE_TRANCE_PLUS: CardDefinition = CardDefinition {
    id: BATTLE_TRANCE_PLUS_ID,
    key: "Battle Trance+",
    name: "Battle Trance+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SEEING_RED_PLUS: CardDefinition = CardDefinition {
    id: SEEING_RED_PLUS_ID,
    key: "Seeing Red+",
    name: "Seeing Red+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const INFLAME: CardDefinition = CardDefinition {
    id: INFLAME_ID,
    key: "Inflame",
    name: "Inflame",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FLEX: CardDefinition = CardDefinition {
    id: FLEX_ID,
    key: "Flex",
    name: "Flex",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SPOT_WEAKNESS: CardDefinition = CardDefinition {
    id: SPOT_WEAKNESS_ID,
    key: "Spot Weakness",
    name: "Spot Weakness",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const INFLAME_PLUS: CardDefinition = CardDefinition {
    id: INFLAME_PLUS_ID,
    key: "Inflame+",
    name: "Inflame+",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FLEX_PLUS: CardDefinition = CardDefinition {
    id: FLEX_PLUS_ID,
    key: "Flex+",
    name: "Flex+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SPOT_WEAKNESS_PLUS: CardDefinition = CardDefinition {
    id: SPOT_WEAKNESS_PLUS_ID,
    key: "Spot Weakness+",
    name: "Spot Weakness+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WHIRLWIND: CardDefinition = CardDefinition {
    id: WHIRLWIND_ID,
    key: "Whirlwind",
    name: "Whirlwind",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WHIRLWIND_PLUS: CardDefinition = CardDefinition {
    id: WHIRLWIND_PLUS_ID,
    key: "Whirlwind+",
    name: "Whirlwind+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const HAVOC: CardDefinition = CardDefinition {
    id: HAVOC_ID,
    key: "Havoc",
    name: "Havoc",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const HAVOC_PLUS: CardDefinition = CardDefinition {
    id: HAVOC_PLUS_ID,
    key: "Havoc+",
    name: "Havoc+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WARCRY: CardDefinition = CardDefinition {
    id: WARCRY_ID,
    key: "Warcry",
    name: "Warcry",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const WARCRY_PLUS: CardDefinition = CardDefinition {
    id: WARCRY_PLUS_ID,
    key: "Warcry+",
    name: "Warcry+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const DUAL_WIELD: CardDefinition = CardDefinition {
    id: DUAL_WIELD_ID,
    key: "Dual Wield",
    name: "Dual Wield",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const DUAL_WIELD_PLUS: CardDefinition = CardDefinition {
    id: DUAL_WIELD_PLUS_ID,
    key: "Dual Wield+",
    name: "Dual Wield+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const SEARING_BLOW: CardDefinition = CardDefinition {
    id: SEARING_BLOW_ID,
    key: "Searing Blow",
    name: "Searing Blow",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(12),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SEARING_BLOW_PLUS: CardDefinition = CardDefinition {
    id: SEARING_BLOW_PLUS_ID,
    key: "Searing Blow+",
    name: "Searing Blow+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(20),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DRAMATIC_ENTRANCE: CardDefinition = CardDefinition {
    id: DRAMATIC_ENTRANCE_ID,
    key: "Dramatic Entrance",
    name: "Dramatic Entrance",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: true,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const BANDAGE_UP: CardDefinition = CardDefinition {
    id: BANDAGE_UP_ID,
    key: "BANDAGE_UP",
    name: "Bandage Up",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const BLIND: CardDefinition = CardDefinition {
    id: BLIND_ID,
    key: "BLIND",
    name: "Blind",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SWIFT_STRIKE: CardDefinition = CardDefinition {
    id: SWIFT_STRIKE_ID,
    key: "Swift Strike",
    name: "Swift Strike",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FLASH_OF_STEEL: CardDefinition = CardDefinition {
    id: FLASH_OF_STEEL_ID,
    key: "Flash of Steel",
    name: "Flash of Steel",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(3),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const GOOD_INSTINCTS: CardDefinition = CardDefinition {
    id: GOOD_INSTINCTS_ID,
    key: "GOOD_INSTINCTS",
    name: "Good Instincts",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(6),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FINESSE: CardDefinition = CardDefinition {
    id: FINESSE_ID,
    key: "FINESSE",
    name: "Finesse",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(2),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const IRON_WAVE: CardDefinition = CardDefinition {
    id: IRON_WAVE_ID,
    key: "IRON_WAVE",
    name: "Iron Wave",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(5),
        block: Some(5),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BODY_SLAM: CardDefinition = CardDefinition {
    id: BODY_SLAM_ID,
    key: "BODY_SLAM",
    name: "Body Slam",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CLASH: CardDefinition = CardDefinition {
    id: CLASH_ID,
    key: "CLASH",
    name: "Clash",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(14),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WILD_STRIKE: CardDefinition = CardDefinition {
    id: WILD_STRIKE_ID,
    key: "WILD_STRIKE",
    name: "Wild Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(12),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const HEAVY_BLADE: CardDefinition = CardDefinition {
    id: HEAVY_BLADE_ID,
    key: "HEAVY_BLADE",
    name: "Heavy Blade",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(14),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const PERFECTED_STRIKE: CardDefinition = CardDefinition {
    id: PERFECTED_STRIKE_ID,
    key: "PERFECTED_STRIKE",
    name: "Perfected Strike",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const RAMPAGE: CardDefinition = CardDefinition {
    id: RAMPAGE_ID,
    key: "RAMPAGE",
    name: "Rampage",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const POWER_THROUGH: CardDefinition = CardDefinition {
    id: POWER_THROUGH_ID,
    key: "POWER_THROUGH",
    name: "Power Through",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(15),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const INFERNAL_BLADE: CardDefinition = CardDefinition {
    id: INFERNAL_BLADE_ID,
    key: "INFERNAL_BLADE",
    name: "Infernal Blade",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const ENTRENCH: CardDefinition = CardDefinition {
    id: ENTRENCH_ID,
    key: "ENTRENCH",
    name: "Entrench",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const GHOSTLY_ARMOR: CardDefinition = CardDefinition {
    id: GHOSTLY_ARMOR_ID,
    key: "GHOSTLY_ARMOR",
    name: "Ghostly Armor",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(10),
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: true,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const FLAME_BARRIER: CardDefinition = CardDefinition {
    id: FLAME_BARRIER_ID,
    key: "FLAME_BARRIER",
    name: "Flame Barrier",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(12),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const RECKLESS_CHARGE: CardDefinition = CardDefinition {
    id: RECKLESS_CHARGE_ID,
    key: "RECKLESS_CHARGE",
    name: "Reckless Charge",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const PUMMEL: CardDefinition = CardDefinition {
    id: PUMMEL_ID,
    key: "PUMMEL",
    name: "Pummel",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(2),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const CLOTHESLINE: CardDefinition = CardDefinition {
    id: CLOTHESLINE_ID,
    key: "CLOTHESLINE",
    name: "Clothesline",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(12),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const INTIMIDATE: CardDefinition = CardDefinition {
    id: INTIMIDATE_ID,
    key: "INTIMIDATE",
    name: "Intimidate",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const SHOCKWAVE: CardDefinition = CardDefinition {
    id: SHOCKWAVE_ID,
    key: "SHOCKWAVE",
    name: "Shockwave",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: Some(3),
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const DISARM: CardDefinition = CardDefinition {
    id: DISARM_ID,
    key: "DISARM",
    name: "Disarm",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const RAGE: CardDefinition = CardDefinition {
    id: RAGE_ID,
    key: "RAGE",
    name: "Rage",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SEVER_SOUL: CardDefinition = CardDefinition {
    id: SEVER_SOUL_ID,
    key: "Sever Soul",
    name: "Sever Soul",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(16),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SECOND_WIND: CardDefinition = CardDefinition {
    id: SECOND_WIND_ID,
    key: "SECOND_WIND",
    name: "Second Wind",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const SENTINEL: CardDefinition = CardDefinition {
    id: SENTINEL_ID,
    key: "Sentinel",
    name: "Sentinel",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BLOODLETTING: CardDefinition = CardDefinition {
    id: BLOODLETTING_ID,
    key: "Bloodletting",
    name: "Bloodletting",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CARNAGE: CardDefinition = CardDefinition {
    id: CARNAGE_ID,
    key: "CARNAGE",
    name: "Carnage",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(20),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: true,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const DROPKICK: CardDefinition = CardDefinition {
    id: DROPKICK_ID,
    key: "DROPKICK",
    name: "Dropkick",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SWORD_BOOMERANG: CardDefinition = CardDefinition {
    id: SWORD_BOOMERANG_ID,
    key: "Sword Boomerang",
    name: "Sword Boomerang",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(3),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const HEMOKINESIS: CardDefinition = CardDefinition {
    id: HEMOKINESIS_ID,
    key: "Hemokinesis",
    name: "Hemokinesis",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(15),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BLOOD_FOR_BLOOD: CardDefinition = CardDefinition {
    id: BLOOD_FOR_BLOOD_ID,
    key: "BLOOD_FOR_BLOOD",
    name: "Blood for Blood",
    cost: 4,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(18),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const IMMOLATE: CardDefinition = CardDefinition {
    id: IMMOLATE_ID,
    key: "Immolate",
    name: "Immolate",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(21),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const BLUDGEON: CardDefinition = CardDefinition {
    id: BLUDGEON_ID,
    key: "BLUDGEON",
    name: "Bludgeon",
    cost: 3,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(32),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FEED: CardDefinition = CardDefinition {
    id: FEED_ID,
    key: "FEED",
    name: "Feed",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(10),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const IMPERVIOUS: CardDefinition = CardDefinition {
    id: IMPERVIOUS_ID,
    key: "IMPERVIOUS",
    name: "Impervious",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(30),
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const FIEND_FIRE: CardDefinition = CardDefinition {
    id: FIEND_FIRE_ID,
    key: "FIEND_FIRE",
    name: "Fiend Fire",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const REAPER: CardDefinition = CardDefinition {
    id: REAPER_ID,
    key: "REAPER",
    name: "Reaper",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(4),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const EXHUME: CardDefinition = CardDefinition {
    id: EXHUME_ID,
    key: "EXHUME",
    name: "Exhume",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const METALLICIZE: CardDefinition = CardDefinition {
    id: METALLICIZE_ID,
    key: "Metallicize",
    name: "Metallicize",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(3),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const THUNDERCLAP: CardDefinition = CardDefinition {
    id: THUNDERCLAP_ID,
    key: "Thunderclap",
    name: "Thunderclap",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(4),
        block: None,
        vulnerable: Some(1),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const UPPERCUT: CardDefinition = CardDefinition {
    id: UPPERCUT_ID,
    key: "Uppercut",
    name: "Uppercut",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(13),
        block: None,
        vulnerable: Some(1),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const IRONCLAD_STARTER_CARDS: [CardDefinition; 3] = [STRIKE_R, DEFEND_R, BASH];
pub const STATUS_CARDS: [CardDefinition; 5] = [WOUND, DAZED, BURN, SLIMED, ASCENDERS_BANE];
pub const MECHANIC_TEST_CARDS: [CardDefinition; 2] = [ETHEREAL_STRIKE, RETAIN_DEFEND];
pub const MILESTONE5_ATTACK_CARDS: [CardDefinition; 10] = [
    ANGER,
    CLEAVE,
    TWIN_STRIKE,
    ANGER_PLUS,
    CLEAVE_PLUS,
    TWIN_STRIKE_PLUS,
    POMMEL_STRIKE,
    POMMEL_STRIKE_PLUS,
    WHIRLWIND,
    WHIRLWIND_PLUS,
];
pub const MILESTONE5_SKILL_CARDS: [CardDefinition; 11] = [
    SHRUG_IT_OFF,
    TRUE_GRIT,
    BURNING_PACT,
    BATTLE_TRANCE,
    SEEING_RED,
    BATTLE_TRANCE_PLUS,
    SEEING_RED_PLUS,
    FLEX,
    SPOT_WEAKNESS,
    FLEX_PLUS,
    SPOT_WEAKNESS_PLUS,
];
pub const MILESTONE5_COMPLEX_CARDS: [CardDefinition; 8] = [
    HAVOC,
    HAVOC_PLUS,
    WARCRY,
    WARCRY_PLUS,
    DUAL_WIELD,
    DUAL_WIELD_PLUS,
    SEARING_BLOW,
    SEARING_BLOW_PLUS,
];
pub const MILESTONE5_POWER_CARDS: [CardDefinition; 4] =
    [FEEL_NO_PAIN, DARK_EMBRACE, INFLAME, INFLAME_PLUS];
pub const ALL_CARDS: [CardDefinition; 107] = [
    STRIKE_R,
    STRIKE_R_PLUS,
    DEFEND_R,
    BASH,
    WOUND,
    DAZED,
    BURN,
    SLIMED,
    REGRET,
    DOUBT,
    CURSE_OF_THE_BELL,
    ASCENDERS_BANE,
    ETHEREAL_STRIKE,
    RETAIN_DEFEND,
    ANGER,
    CLEAVE,
    TWIN_STRIKE,
    ANGER_PLUS,
    CLEAVE_PLUS,
    TWIN_STRIKE_PLUS,
    SHRUG_IT_OFF,
    TRUE_GRIT,
    BURNING_PACT,
    FEEL_NO_PAIN,
    DARK_EMBRACE,
    COMBUST,
    DEMON_FORM,
    EVOLVE,
    CORRUPTION,
    BARRICADE,
    BERSERK,
    RUPTURE,
    JUGGERNAUT,
    BRUTALITY,
    DOUBLE_TAP,
    FIRE_BREATHING,
    LIMIT_BREAK,
    OFFERING,
    ARMAMENTS,
    HEADBUTT,
    POMMEL_STRIKE,
    BATTLE_TRANCE,
    SEEING_RED,
    POMMEL_STRIKE_PLUS,
    BATTLE_TRANCE_PLUS,
    SEEING_RED_PLUS,
    INFLAME,
    FLEX,
    SPOT_WEAKNESS,
    INFLAME_PLUS,
    FLEX_PLUS,
    SPOT_WEAKNESS_PLUS,
    WHIRLWIND,
    WHIRLWIND_PLUS,
    HAVOC,
    HAVOC_PLUS,
    WARCRY,
    WARCRY_PLUS,
    DUAL_WIELD,
    DUAL_WIELD_PLUS,
    SEARING_BLOW,
    SEARING_BLOW_PLUS,
    DRAMATIC_ENTRANCE,
    BANDAGE_UP,
    BLIND,
    SWIFT_STRIKE,
    FLASH_OF_STEEL,
    GOOD_INSTINCTS,
    FINESSE,
    IRON_WAVE,
    BODY_SLAM,
    CLASH,
    WILD_STRIKE,
    HEAVY_BLADE,
    PERFECTED_STRIKE,
    RAMPAGE,
    POWER_THROUGH,
    INFERNAL_BLADE,
    ENTRENCH,
    GHOSTLY_ARMOR,
    FLAME_BARRIER,
    RECKLESS_CHARGE,
    PUMMEL,
    CLOTHESLINE,
    INTIMIDATE,
    SHOCKWAVE,
    DISARM,
    RAGE,
    SEVER_SOUL,
    SECOND_WIND,
    SENTINEL,
    BLOODLETTING,
    CARNAGE,
    DROPKICK,
    SWORD_BOOMERANG,
    HEMOKINESIS,
    BLOOD_FOR_BLOOD,
    IMMOLATE,
    BLUDGEON,
    FEED,
    IMPERVIOUS,
    FIEND_FIRE,
    REAPER,
    EXHUME,
    METALLICIZE,
    THUNDERCLAP,
    UPPERCUT,
];

#[must_use]
pub fn get_card_definition(id: ContentId) -> Option<&'static CardDefinition> {
    ALL_CARDS.iter().find(|definition| definition.id == id)
}

#[must_use]
pub fn is_curse_content_id(id: ContentId) -> bool {
    matches!(id, id if id == REGRET_ID || id == DOUBT_ID || id == CURSE_OF_THE_BELL_ID || id == ASCENDERS_BANE_ID)
}

#[must_use]
pub fn is_basic_starter_card(id: ContentId) -> bool {
    matches!(id, id if id == STRIKE_R_ID || id == DEFEND_R_ID || id == BASH_ID)
}

#[must_use]
pub fn is_pandoras_box_removed_starter(id: ContentId) -> bool {
    matches!(id, id if id == STRIKE_R_ID || id == STRIKE_R_PLUS_ID || id == DEFEND_R_ID)
}

#[must_use]
pub fn card_type_and_rarity(id: ContentId) -> Option<(CardType, CardRarity)> {
    match id {
        id if id == STRIKE_R_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == DEFEND_R_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == BASH_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == BANDAGE_UP_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BLIND_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == IRON_WAVE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == BODY_SLAM_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == CLASH_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == ARMAMENTS_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == HEADBUTT_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == WILD_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == HEAVY_BLADE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == PERFECTED_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == RAMPAGE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == POWER_THROUGH_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == INFERNAL_BLADE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == ENTRENCH_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == GHOSTLY_ARMOR_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FLAME_BARRIER_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == RECKLESS_CHARGE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == PUMMEL_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == BLOODLETTING_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == CARNAGE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == DROPKICK_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == HEMOKINESIS_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == BLOOD_FOR_BLOOD_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == CLOTHESLINE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == ANGER_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == CLEAVE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == TWIN_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == SHRUG_IT_OFF_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == TRUE_GRIT_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == POMMEL_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == BATTLE_TRANCE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SEEING_RED_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BURNING_PACT_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FEEL_NO_PAIN_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == DARK_EMBRACE_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == COMBUST_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == DEMON_FORM_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == EVOLVE_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == CORRUPTION_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BARRICADE_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BERSERK_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == RUPTURE_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == JUGGERNAUT_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BRUTALITY_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == DOUBLE_TAP_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == FIRE_BREATHING_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == LIMIT_BREAK_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == OFFERING_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == INFLAME_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == FLEX_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == SPOT_WEAKNESS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == WHIRLWIND_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == HAVOC_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == WARCRY_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == DUAL_WIELD_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SEARING_BLOW_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == SECOND_WIND_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SENTINEL_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == INTIMIDATE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SHOCKWAVE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == DISARM_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == RAGE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BLUDGEON_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == FEED_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == IMPERVIOUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == FIEND_FIRE_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == REAPER_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == EXHUME_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == DRAMATIC_ENTRANCE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == FLASH_OF_STEEL_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == GOOD_INSTINCTS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FINESSE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        _ => None,
    }
}

/// Maps a base card content id to its upgraded (+) version, if one exists.
#[must_use]
pub fn upgrade_content_id(id: ContentId) -> Option<ContentId> {
    match id {
        STRIKE_R_ID => Some(STRIKE_R_PLUS_ID),
        ANGER_ID => Some(ANGER_PLUS_ID),
        CLEAVE_ID => Some(CLEAVE_PLUS_ID),
        TWIN_STRIKE_ID => Some(TWIN_STRIKE_PLUS_ID),
        POMMEL_STRIKE_ID => Some(POMMEL_STRIKE_PLUS_ID),
        BATTLE_TRANCE_ID => Some(BATTLE_TRANCE_PLUS_ID),
        SEEING_RED_ID => Some(SEEING_RED_PLUS_ID),
        INFLAME_ID => Some(INFLAME_PLUS_ID),
        FLEX_ID => Some(FLEX_PLUS_ID),
        SPOT_WEAKNESS_ID => Some(SPOT_WEAKNESS_PLUS_ID),
        WHIRLWIND_ID => Some(WHIRLWIND_PLUS_ID),
        HAVOC_ID => Some(HAVOC_PLUS_ID),
        WARCRY_ID => Some(WARCRY_PLUS_ID),
        DUAL_WIELD_ID => Some(DUAL_WIELD_PLUS_ID),
        SEARING_BLOW_ID => Some(SEARING_BLOW_PLUS_ID),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strike_r_has_expected_starter_values() {
        assert_eq!(STRIKE_R.cost, 1);
        assert_eq!(STRIKE_R.target, TargetRequirement::Enemy);
        assert_eq!(STRIKE_R.card_type, CardType::Attack);
        assert_eq!(STRIKE_R.values.damage, Some(6));
    }

    #[test]
    fn defend_r_has_expected_starter_values() {
        assert_eq!(DEFEND_R.cost, 1);
        assert_eq!(DEFEND_R.target, TargetRequirement::None);
        assert_eq!(DEFEND_R.card_type, CardType::Skill);
        assert_eq!(DEFEND_R.values.block, Some(5));
    }

    #[test]
    fn bash_has_expected_starter_values() {
        assert_eq!(BASH.cost, 2);
        assert_eq!(BASH.target, TargetRequirement::Enemy);
        assert_eq!(BASH.card_type, CardType::Attack);
        assert_eq!(BASH.values.damage, Some(8));
        assert_eq!(BASH.values.vulnerable, Some(2));
    }

    #[test]
    fn evolve_has_expected_base_power_values() {
        assert_eq!(EVOLVE.id, EVOLVE_ID);
        assert_eq!(EVOLVE.cost, 1);
        assert_eq!(EVOLVE.target, TargetRequirement::None);
        assert_eq!(EVOLVE.card_type, CardType::Power);
        assert_eq!(
            card_type_and_rarity(EVOLVE_ID),
            Some((CardType::Power, CardRarity::Uncommon))
        );
    }

    #[test]
    fn wound_is_unplayable_non_ethereal_status_with_no_target() {
        assert_eq!(WOUND.id, WOUND_ID);
        assert_eq!(WOUND.card_type, CardType::Status);
        assert_eq!(WOUND.target, TargetRequirement::None);
        assert!(WOUND.keywords.unplayable);
        assert!(!WOUND.keywords.ethereal);
        assert!(!WOUND.keywords.retain);
        assert!(!WOUND.keywords.exhaust);
    }

    #[test]
    fn regret_is_unplayable_non_ethereal_status_curse_with_no_target() {
        assert_eq!(REGRET.id, REGRET_ID);
        assert_eq!(REGRET.card_type, CardType::Status);
        assert_eq!(REGRET.target, TargetRequirement::None);
        assert!(REGRET.keywords.unplayable);
        assert!(!REGRET.keywords.ethereal);
        assert!(!REGRET.keywords.retain);
        assert!(!REGRET.keywords.exhaust);
        assert!(is_curse_content_id(REGRET_ID));
    }

    #[test]
    fn exhume_is_rare_cost_one_skill_with_exhaust_and_no_target() {
        assert_eq!(EXHUME.id, EXHUME_ID);
        assert_eq!(EXHUME.cost, 1);
        assert_eq!(EXHUME.card_type, CardType::Skill);
        assert_eq!(EXHUME.target, TargetRequirement::None);
        assert!(EXHUME.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(EXHUME_ID),
            Some((CardType::Skill, CardRarity::Rare))
        );
    }

    #[test]
    fn modeled_curse_ids_are_classified_explicitly() {
        assert!(is_curse_content_id(REGRET_ID));
        assert!(is_curse_content_id(DOUBT_ID));
        assert!(is_curse_content_id(CURSE_OF_THE_BELL_ID));
        assert!(is_curse_content_id(ASCENDERS_BANE_ID));
        assert!(!is_curse_content_id(WOUND_ID));
    }

    #[test]
    fn anger_has_expected_values() {
        assert_eq!(ANGER.cost, 0);
        assert_eq!(ANGER.target, TargetRequirement::Enemy);
        assert_eq!(ANGER.card_type, CardType::Attack);
        assert_eq!(ANGER.values.damage, Some(6));
    }

    #[test]
    fn cleave_has_expected_values() {
        assert_eq!(CLEAVE.cost, 1);
        assert_eq!(CLEAVE.target, TargetRequirement::AllEnemies);
        assert_eq!(CLEAVE.card_type, CardType::Attack);
        assert_eq!(CLEAVE.values.damage, Some(8));
    }

    #[test]
    fn dramatic_entrance_has_expected_values() {
        assert_eq!(DRAMATIC_ENTRANCE.cost, 0);
        assert_eq!(DRAMATIC_ENTRANCE.target, TargetRequirement::AllEnemies);
        assert_eq!(DRAMATIC_ENTRANCE.card_type, CardType::Attack);
        assert_eq!(DRAMATIC_ENTRANCE.values.damage, Some(8));
        assert!(DRAMATIC_ENTRANCE.keywords.exhaust);
    }

    #[test]
    fn bandage_up_has_expected_values_keywords_and_rarity() {
        assert_eq!(BANDAGE_UP.id, BANDAGE_UP_ID);
        assert_eq!(BANDAGE_UP.cost, 0);
        assert_eq!(BANDAGE_UP.target, TargetRequirement::None);
        assert_eq!(BANDAGE_UP.card_type, CardType::Skill);
        assert!(BANDAGE_UP.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(BANDAGE_UP_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn blind_has_expected_values_and_rarity() {
        assert_eq!(BLIND.id, BLIND_ID);
        assert_eq!(BLIND.cost, 0);
        assert_eq!(BLIND.target, TargetRequirement::AllEnemies);
        assert_eq!(BLIND.card_type, CardType::Skill);
        assert_eq!(BLIND.keywords, CARD_KEYWORDS_NONE);
        assert_eq!(
            card_type_and_rarity(BLIND_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn good_instincts_has_expected_values_and_rarity() {
        assert_eq!(GOOD_INSTINCTS.id, GOOD_INSTINCTS_ID);
        assert_eq!(GOOD_INSTINCTS.cost, 0);
        assert_eq!(GOOD_INSTINCTS.target, TargetRequirement::None);
        assert_eq!(GOOD_INSTINCTS.card_type, CardType::Skill);
        assert_eq!(GOOD_INSTINCTS.values.block, Some(6));
        assert_eq!(GOOD_INSTINCTS.keywords, CARD_KEYWORDS_NONE);
        assert_eq!(
            card_type_and_rarity(GOOD_INSTINCTS_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn flash_of_steel_has_expected_values_and_rarity() {
        assert_eq!(FLASH_OF_STEEL.id, FLASH_OF_STEEL_ID);
        assert_eq!(FLASH_OF_STEEL.cost, 0);
        assert_eq!(FLASH_OF_STEEL.target, TargetRequirement::Enemy);
        assert_eq!(FLASH_OF_STEEL.card_type, CardType::Attack);
        assert_eq!(FLASH_OF_STEEL.values.damage, Some(3));
        assert_eq!(FLASH_OF_STEEL.keywords, CARD_KEYWORDS_NONE);
        assert_eq!(
            card_type_and_rarity(FLASH_OF_STEEL_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn finesse_has_expected_values_and_rarity() {
        assert_eq!(FINESSE.id, FINESSE_ID);
        assert_eq!(FINESSE.cost, 0);
        assert_eq!(FINESSE.target, TargetRequirement::None);
        assert_eq!(FINESSE.card_type, CardType::Skill);
        assert_eq!(FINESSE.values.block, Some(2));
        assert_eq!(FINESSE.keywords, CARD_KEYWORDS_NONE);
        assert_eq!(
            card_type_and_rarity(FINESSE_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn twin_strike_has_expected_values() {
        assert_eq!(TWIN_STRIKE.cost, 1);
        assert_eq!(TWIN_STRIKE.target, TargetRequirement::Enemy);
        assert_eq!(TWIN_STRIKE.card_type, CardType::Attack);
        assert_eq!(TWIN_STRIKE.values.damage, Some(5));
    }

    #[test]
    fn iron_wave_has_expected_values() {
        assert_eq!(IRON_WAVE.cost, 1);
        assert_eq!(IRON_WAVE.target, TargetRequirement::Enemy);
        assert_eq!(IRON_WAVE.card_type, CardType::Attack);
        assert_eq!(IRON_WAVE.values.damage, Some(5));
        assert_eq!(IRON_WAVE.values.block, Some(5));
        assert_eq!(
            card_type_and_rarity(IRON_WAVE_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn body_slam_has_expected_values() {
        assert_eq!(BODY_SLAM.cost, 1);
        assert_eq!(BODY_SLAM.target, TargetRequirement::Enemy);
        assert_eq!(BODY_SLAM.card_type, CardType::Attack);
        assert_eq!(BODY_SLAM.values.damage, None);
        assert_eq!(BODY_SLAM.values.block, None);
        assert_eq!(
            card_type_and_rarity(BODY_SLAM_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn armaments_has_expected_values() {
        assert_eq!(ARMAMENTS.cost, 1);
        assert_eq!(ARMAMENTS.target, TargetRequirement::None);
        assert_eq!(ARMAMENTS.card_type, CardType::Skill);
        assert_eq!(ARMAMENTS.values.block, Some(5));
        assert_eq!(
            card_type_and_rarity(ARMAMENTS_ID),
            Some((CardType::Skill, CardRarity::Common))
        );
    }

    #[test]
    fn headbutt_has_expected_values() {
        assert_eq!(HEADBUTT.cost, 1);
        assert_eq!(HEADBUTT.target, TargetRequirement::Enemy);
        assert_eq!(HEADBUTT.card_type, CardType::Attack);
        assert_eq!(HEADBUTT.values.damage, Some(9));
        assert_eq!(
            card_type_and_rarity(HEADBUTT_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn clash_has_expected_values() {
        assert_eq!(CLASH.cost, 0);
        assert_eq!(CLASH.target, TargetRequirement::Enemy);
        assert_eq!(CLASH.card_type, CardType::Attack);
        assert_eq!(CLASH.values.damage, Some(14));
        assert_eq!(
            card_type_and_rarity(CLASH_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn wild_strike_has_expected_values() {
        assert_eq!(WILD_STRIKE.cost, 1);
        assert_eq!(WILD_STRIKE.target, TargetRequirement::Enemy);
        assert_eq!(WILD_STRIKE.card_type, CardType::Attack);
        assert_eq!(WILD_STRIKE.values.damage, Some(12));
        assert_eq!(
            card_type_and_rarity(WILD_STRIKE_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn heavy_blade_has_expected_values() {
        assert_eq!(HEAVY_BLADE.cost, 2);
        assert_eq!(HEAVY_BLADE.target, TargetRequirement::Enemy);
        assert_eq!(HEAVY_BLADE.card_type, CardType::Attack);
        assert_eq!(HEAVY_BLADE.values.damage, Some(14));
        assert_eq!(
            card_type_and_rarity(HEAVY_BLADE_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn perfected_strike_has_expected_values() {
        assert_eq!(PERFECTED_STRIKE.cost, 2);
        assert_eq!(PERFECTED_STRIKE.target, TargetRequirement::Enemy);
        assert_eq!(PERFECTED_STRIKE.card_type, CardType::Attack);
        assert_eq!(PERFECTED_STRIKE.values.damage, Some(6));
        assert_eq!(
            card_type_and_rarity(PERFECTED_STRIKE_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn rampage_has_expected_values_and_rarity() {
        assert_eq!(RAMPAGE.cost, 1);
        assert_eq!(RAMPAGE.target, TargetRequirement::Enemy);
        assert_eq!(RAMPAGE.card_type, CardType::Attack);
        assert_eq!(RAMPAGE.values.damage, Some(8));
        assert_eq!(
            card_type_and_rarity(RAMPAGE_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn power_through_has_expected_values() {
        assert_eq!(POWER_THROUGH.cost, 1);
        assert_eq!(POWER_THROUGH.target, TargetRequirement::None);
        assert_eq!(POWER_THROUGH.card_type, CardType::Skill);
        assert_eq!(POWER_THROUGH.values.block, Some(15));
        assert_eq!(
            card_type_and_rarity(POWER_THROUGH_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn entrench_has_expected_values_and_rarity() {
        assert_eq!(ENTRENCH.cost, 2);
        assert_eq!(ENTRENCH.target, TargetRequirement::None);
        assert_eq!(ENTRENCH.card_type, CardType::Skill);
        assert_eq!(ENTRENCH.values.block, None);
        assert_eq!(
            card_type_and_rarity(ENTRENCH_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn ghostly_armor_has_expected_values() {
        assert_eq!(GHOSTLY_ARMOR.cost, 1);
        assert_eq!(GHOSTLY_ARMOR.target, TargetRequirement::None);
        assert_eq!(GHOSTLY_ARMOR.card_type, CardType::Skill);
        assert_eq!(GHOSTLY_ARMOR.values.block, Some(10));
        assert!(GHOSTLY_ARMOR.keywords.ethereal);
        assert!(!GHOSTLY_ARMOR.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(GHOSTLY_ARMOR_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn reckless_charge_has_expected_values() {
        assert_eq!(RECKLESS_CHARGE.cost, 0);
        assert_eq!(RECKLESS_CHARGE.target, TargetRequirement::Enemy);
        assert_eq!(RECKLESS_CHARGE.card_type, CardType::Attack);
        assert_eq!(RECKLESS_CHARGE.values.damage, Some(7));
        assert_eq!(
            card_type_and_rarity(RECKLESS_CHARGE_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn pummel_has_expected_values() {
        assert_eq!(PUMMEL.cost, 1);
        assert_eq!(PUMMEL.target, TargetRequirement::Enemy);
        assert_eq!(PUMMEL.card_type, CardType::Attack);
        assert_eq!(PUMMEL.values.damage, Some(2));
        assert!(PUMMEL.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(PUMMEL_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn bludgeon_has_expected_values() {
        assert_eq!(BLUDGEON.cost, 3);
        assert_eq!(BLUDGEON.target, TargetRequirement::Enemy);
        assert_eq!(BLUDGEON.card_type, CardType::Attack);
        assert_eq!(BLUDGEON.values.damage, Some(32));
        assert_eq!(
            card_type_and_rarity(BLUDGEON_ID),
            Some((CardType::Attack, CardRarity::Rare))
        );
    }

    #[test]
    fn feed_has_expected_values_keywords_and_rarity() {
        assert_eq!(FEED.id, FEED_ID);
        assert_eq!(FEED.cost, 1);
        assert_eq!(FEED.target, TargetRequirement::Enemy);
        assert_eq!(FEED.card_type, CardType::Attack);
        assert_eq!(FEED.values.damage, Some(10));
        assert!(FEED.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(FEED_ID),
            Some((CardType::Attack, CardRarity::Rare))
        );
    }

    #[test]
    fn impervious_has_expected_values() {
        assert_eq!(IMPERVIOUS.cost, 2);
        assert_eq!(IMPERVIOUS.target, TargetRequirement::None);
        assert_eq!(IMPERVIOUS.card_type, CardType::Skill);
        assert_eq!(IMPERVIOUS.values.block, Some(30));
        assert!(IMPERVIOUS.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(IMPERVIOUS_ID),
            Some((CardType::Skill, CardRarity::Rare))
        );
    }

    #[test]
    fn reaper_has_expected_values() {
        assert_eq!(REAPER.id, REAPER_ID);
        assert_eq!(REAPER.cost, 2);
        assert_eq!(REAPER.target, TargetRequirement::AllEnemies);
        assert_eq!(REAPER.card_type, CardType::Attack);
        assert_eq!(REAPER.values.damage, Some(4));
        assert!(REAPER.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(REAPER_ID),
            Some((CardType::Attack, CardRarity::Rare))
        );
    }

    #[test]
    fn fiend_fire_has_expected_values_keywords_and_rarity() {
        assert_eq!(FIEND_FIRE.id, FIEND_FIRE_ID);
        assert_eq!(FIEND_FIRE.cost, 2);
        assert_eq!(FIEND_FIRE.target, TargetRequirement::Enemy);
        assert_eq!(FIEND_FIRE.card_type, CardType::Attack);
        assert_eq!(FIEND_FIRE.values.damage, Some(7));
        assert!(FIEND_FIRE.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(FIEND_FIRE_ID),
            Some((CardType::Attack, CardRarity::Rare))
        );
    }

    #[test]
    fn clothesline_has_expected_values() {
        assert_eq!(CLOTHESLINE.cost, 2);
        assert_eq!(CLOTHESLINE.target, TargetRequirement::Enemy);
        assert_eq!(CLOTHESLINE.card_type, CardType::Attack);
        assert_eq!(CLOTHESLINE.values.damage, Some(12));
        assert_eq!(
            card_type_and_rarity(CLOTHESLINE_ID),
            Some((CardType::Attack, CardRarity::Common))
        );
    }

    #[test]
    fn intimidate_has_expected_values() {
        assert_eq!(INTIMIDATE.cost, 0);
        assert_eq!(INTIMIDATE.target, TargetRequirement::None);
        assert_eq!(INTIMIDATE.card_type, CardType::Skill);
        assert!(INTIMIDATE.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(INTIMIDATE_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn shockwave_has_expected_values() {
        assert_eq!(SHOCKWAVE.id, SHOCKWAVE_ID);
        assert_eq!(SHOCKWAVE.cost, 2);
        assert_eq!(SHOCKWAVE.target, TargetRequirement::None);
        assert_eq!(SHOCKWAVE.card_type, CardType::Skill);
        assert_eq!(SHOCKWAVE.values.vulnerable, Some(3));
        assert_eq!(
            SHOCKWAVE.keywords,
            CardKeywords {
                innate: false,
                ethereal: false,
                exhaust: true,
                retain: false,
                unplayable: false,
            }
        );
        assert_eq!(
            card_type_and_rarity(SHOCKWAVE_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn disarm_has_expected_values() {
        assert_eq!(DISARM.id, DISARM_ID);
        assert_eq!(DISARM.cost, 1);
        assert_eq!(DISARM.target, TargetRequirement::Enemy);
        assert_eq!(DISARM.card_type, CardType::Skill);
        assert_eq!(
            DISARM.keywords,
            CardKeywords {
                innate: false,
                ethereal: false,
                exhaust: true,
                retain: false,
                unplayable: false,
            }
        );
        assert_eq!(
            card_type_and_rarity(DISARM_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn rage_has_expected_values_and_rarity() {
        assert_eq!(RAGE.id, RAGE_ID);
        assert_eq!(RAGE.cost, 0);
        assert_eq!(RAGE.target, TargetRequirement::None);
        assert_eq!(RAGE.card_type, CardType::Skill);
        assert_eq!(RAGE.values.block, None);
        assert_eq!(
            card_type_and_rarity(RAGE_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn strike_r_plus_deals_three_more_damage() {
        assert_eq!(STRIKE_R_PLUS.values.damage, Some(9));
    }

    #[test]
    fn anger_plus_deals_one_more_damage() {
        assert_eq!(ANGER_PLUS.values.damage, Some(7));
    }

    #[test]
    fn cleave_plus_deals_one_more_damage() {
        assert_eq!(CLEAVE_PLUS.values.damage, Some(9));
    }

    #[test]
    fn twin_strike_plus_deals_one_more_damage() {
        assert_eq!(TWIN_STRIKE_PLUS.values.damage, Some(6));
    }

    #[test]
    fn shrug_it_off_has_expected_values() {
        assert_eq!(SHRUG_IT_OFF.cost, 1);
        assert_eq!(SHRUG_IT_OFF.target, TargetRequirement::None);
        assert_eq!(SHRUG_IT_OFF.card_type, CardType::Skill);
        assert_eq!(SHRUG_IT_OFF.values.block, Some(8));
    }

    #[test]
    fn true_grit_has_expected_values() {
        assert_eq!(TRUE_GRIT.cost, 1);
        assert_eq!(TRUE_GRIT.target, TargetRequirement::None);
        assert_eq!(TRUE_GRIT.card_type, CardType::Skill);
        assert_eq!(TRUE_GRIT.values.block, Some(7));
    }

    #[test]
    fn burning_pact_has_expected_values() {
        assert_eq!(BURNING_PACT.cost, 1);
        assert_eq!(BURNING_PACT.target, TargetRequirement::None);
        assert_eq!(BURNING_PACT.card_type, CardType::Skill);
    }

    #[test]
    fn sentinel_has_expected_values_and_rarity() {
        assert_eq!(SENTINEL.cost, 1);
        assert_eq!(SENTINEL.target, TargetRequirement::None);
        assert_eq!(SENTINEL.card_type, CardType::Skill);
        assert_eq!(SENTINEL.values.block, Some(5));
        assert_eq!(
            card_type_and_rarity(SENTINEL_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn second_wind_has_expected_values_keywords_and_rarity() {
        assert_eq!(SECOND_WIND.id, SECOND_WIND_ID);
        assert_eq!(SECOND_WIND.cost, 1);
        assert_eq!(SECOND_WIND.target, TargetRequirement::None);
        assert_eq!(SECOND_WIND.card_type, CardType::Skill);
        assert_eq!(SECOND_WIND.values.block, Some(5));
        assert_eq!(
            SECOND_WIND.keywords,
            CardKeywords {
                innate: false,
                unplayable: false,
                ethereal: false,
                exhaust: false,
                retain: false,
            }
        );
        assert_eq!(
            card_type_and_rarity(SECOND_WIND_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn feel_no_pain_has_expected_values() {
        assert_eq!(FEEL_NO_PAIN.cost, 1);
        assert_eq!(FEEL_NO_PAIN.target, TargetRequirement::None);
        assert_eq!(FEEL_NO_PAIN.card_type, CardType::Power);
    }

    #[test]
    fn dark_embrace_has_expected_values() {
        assert_eq!(DARK_EMBRACE.cost, 1);
        assert_eq!(DARK_EMBRACE.target, TargetRequirement::None);
        assert_eq!(DARK_EMBRACE.card_type, CardType::Power);
    }

    #[test]
    fn demon_form_has_expected_values_and_rarity() {
        assert_eq!(DEMON_FORM.cost, 3);
        assert_eq!(DEMON_FORM.target, TargetRequirement::None);
        assert_eq!(DEMON_FORM.card_type, CardType::Power);
        assert_eq!(
            card_type_and_rarity(DEMON_FORM_ID),
            Some((CardType::Power, CardRarity::Rare))
        );
    }

    #[test]
    fn corruption_has_expected_values_and_rarity() {
        assert_eq!(CORRUPTION.id, CORRUPTION_ID);
        assert_eq!(CORRUPTION.cost, 3);
        assert_eq!(CORRUPTION.target, TargetRequirement::None);
        assert_eq!(CORRUPTION.card_type, CardType::Power);
        assert_eq!(
            card_type_and_rarity(CORRUPTION_ID),
            Some((CardType::Power, CardRarity::Rare))
        );
    }

    #[test]
    fn barricade_has_expected_values_and_rarity() {
        assert_eq!(BARRICADE.id, BARRICADE_ID);
        assert_eq!(BARRICADE.cost, 3);
        assert_eq!(BARRICADE.target, TargetRequirement::None);
        assert_eq!(BARRICADE.card_type, CardType::Power);
        assert_eq!(
            card_type_and_rarity(BARRICADE_ID),
            Some((CardType::Power, CardRarity::Rare))
        );
    }

    #[test]
    fn berserk_has_expected_values_and_rarity() {
        assert_eq!(BERSERK.id, BERSERK_ID);
        assert_eq!(BERSERK.cost, 0);
        assert_eq!(BERSERK.target, TargetRequirement::None);
        assert_eq!(BERSERK.card_type, CardType::Power);
        assert_eq!(BERSERK.values.vulnerable, Some(2));
        assert_eq!(
            card_type_and_rarity(BERSERK_ID),
            Some((CardType::Power, CardRarity::Rare))
        );
    }

    #[test]
    fn combust_has_expected_values_and_rarity() {
        assert_eq!(COMBUST.id, COMBUST_ID);
        assert_eq!(COMBUST.cost, 1);
        assert_eq!(COMBUST.target, TargetRequirement::None);
        assert_eq!(COMBUST.card_type, CardType::Power);
        assert_eq!(COMBUST.values.damage, Some(COMBUST_DAMAGE));
        assert_eq!(
            card_type_and_rarity(COMBUST_ID),
            Some((CardType::Power, CardRarity::Uncommon))
        );
    }

    #[test]
    fn rupture_has_expected_values_and_rarity() {
        assert_eq!(RUPTURE.id, RUPTURE_ID);
        assert_eq!(RUPTURE.cost, 1);
        assert_eq!(RUPTURE.target, TargetRequirement::None);
        assert_eq!(RUPTURE.card_type, CardType::Power);
        assert_eq!(
            card_type_and_rarity(RUPTURE_ID),
            Some((CardType::Power, CardRarity::Uncommon))
        );
    }

    #[test]
    fn juggernaut_has_expected_values_and_rarity() {
        assert_eq!(JUGGERNAUT.id, JUGGERNAUT_ID);
        assert_eq!(JUGGERNAUT.cost, 2);
        assert_eq!(JUGGERNAUT.target, TargetRequirement::None);
        assert_eq!(JUGGERNAUT.card_type, CardType::Power);
        assert_eq!(JUGGERNAUT.values.damage, Some(5));
        assert_eq!(
            card_type_and_rarity(JUGGERNAUT_ID),
            Some((CardType::Power, CardRarity::Rare))
        );
    }

    #[test]
    fn brutality_has_expected_values_and_rarity() {
        assert_eq!(BRUTALITY.id, BRUTALITY_ID);
        assert_eq!(BRUTALITY.cost, 0);
        assert_eq!(BRUTALITY.target, TargetRequirement::None);
        assert_eq!(BRUTALITY.card_type, CardType::Power);
        assert_eq!(
            card_type_and_rarity(BRUTALITY_ID),
            Some((CardType::Power, CardRarity::Rare))
        );
    }

    #[test]
    fn fire_breathing_has_expected_values_and_rarity() {
        assert_eq!(FIRE_BREATHING.id, FIRE_BREATHING_ID);
        assert_eq!(FIRE_BREATHING.cost, 1);
        assert_eq!(FIRE_BREATHING.target, TargetRequirement::None);
        assert_eq!(FIRE_BREATHING.card_type, CardType::Power);
        assert_eq!(FIRE_BREATHING.values.damage, Some(6));
        assert_eq!(
            card_type_and_rarity(FIRE_BREATHING_ID),
            Some((CardType::Power, CardRarity::Uncommon))
        );
    }

    #[test]
    fn infernal_blade_has_expected_values_and_rarity() {
        assert_eq!(INFERNAL_BLADE.id, INFERNAL_BLADE_ID);
        assert_eq!(INFERNAL_BLADE.cost, 1);
        assert_eq!(INFERNAL_BLADE.target, TargetRequirement::None);
        assert_eq!(INFERNAL_BLADE.card_type, CardType::Skill);
        assert_eq!(
            card_type_and_rarity(INFERNAL_BLADE_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn limit_break_has_expected_values_keywords_and_rarity() {
        assert_eq!(LIMIT_BREAK.id, LIMIT_BREAK_ID);
        assert_eq!(LIMIT_BREAK.cost, 1);
        assert_eq!(LIMIT_BREAK.target, TargetRequirement::None);
        assert_eq!(LIMIT_BREAK.card_type, CardType::Skill);
        assert_eq!(
            LIMIT_BREAK.keywords,
            CardKeywords {
                innate: false,
                unplayable: false,
                ethereal: false,
                exhaust: true,
                retain: false,
            }
        );
        assert_eq!(
            card_type_and_rarity(LIMIT_BREAK_ID),
            Some((CardType::Skill, CardRarity::Rare))
        );
    }

    #[test]
    fn offering_has_expected_values_keywords_and_rarity() {
        assert_eq!(OFFERING.id, OFFERING_ID);
        assert_eq!(OFFERING.cost, 0);
        assert_eq!(OFFERING.target, TargetRequirement::None);
        assert_eq!(OFFERING.card_type, CardType::Skill);
        assert_eq!(
            OFFERING.keywords,
            CardKeywords {
                innate: false,
                unplayable: false,
                ethereal: false,
                exhaust: true,
                retain: false,
            }
        );
        assert_eq!(
            card_type_and_rarity(OFFERING_ID),
            Some((CardType::Skill, CardRarity::Rare))
        );
    }

    #[test]
    fn pommel_strike_has_expected_values() {
        assert_eq!(POMMEL_STRIKE.cost, 1);
        assert_eq!(POMMEL_STRIKE.target, TargetRequirement::Enemy);
        assert_eq!(POMMEL_STRIKE.card_type, CardType::Attack);
        assert_eq!(POMMEL_STRIKE.values.damage, Some(9));
    }

    #[test]
    fn battle_trance_has_expected_values() {
        assert_eq!(BATTLE_TRANCE.cost, 0);
        assert_eq!(BATTLE_TRANCE.target, TargetRequirement::None);
        assert_eq!(BATTLE_TRANCE.card_type, CardType::Skill);
    }

    #[test]
    fn seeing_red_has_expected_values() {
        assert_eq!(SEEING_RED.cost, 1);
        assert_eq!(SEEING_RED.target, TargetRequirement::None);
        assert_eq!(SEEING_RED.card_type, CardType::Skill);
    }

    #[test]
    fn bloodletting_has_expected_values_and_rarity() {
        assert_eq!(BLOODLETTING.cost, 0);
        assert_eq!(BLOODLETTING.target, TargetRequirement::None);
        assert_eq!(BLOODLETTING.card_type, CardType::Skill);
        assert_eq!(
            card_type_and_rarity(BLOODLETTING_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn carnage_has_expected_values_and_keywords() {
        assert_eq!(CARNAGE.cost, 2);
        assert_eq!(CARNAGE.target, TargetRequirement::Enemy);
        assert_eq!(CARNAGE.card_type, CardType::Attack);
        assert_eq!(CARNAGE.values.damage, Some(20));
        assert_eq!(
            CARNAGE.keywords,
            CardKeywords {
                innate: false,
                ethereal: true,
                exhaust: false,
                retain: false,
                unplayable: false,
            }
        );
        assert_eq!(
            card_type_and_rarity(CARNAGE_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn dropkick_has_expected_values_and_rarity() {
        assert_eq!(DROPKICK.cost, 1);
        assert_eq!(DROPKICK.target, TargetRequirement::Enemy);
        assert_eq!(DROPKICK.card_type, CardType::Attack);
        assert_eq!(DROPKICK.values.damage, Some(5));
        assert_eq!(
            card_type_and_rarity(DROPKICK_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn hemokinesis_has_expected_values_and_rarity() {
        assert_eq!(HEMOKINESIS.cost, 1);
        assert_eq!(HEMOKINESIS.target, TargetRequirement::Enemy);
        assert_eq!(HEMOKINESIS.card_type, CardType::Attack);
        assert_eq!(HEMOKINESIS.values.damage, Some(15));
        assert_eq!(
            card_type_and_rarity(HEMOKINESIS_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn blood_for_blood_has_expected_values_and_rarity() {
        assert_eq!(BLOOD_FOR_BLOOD.id, BLOOD_FOR_BLOOD_ID);
        assert_eq!(BLOOD_FOR_BLOOD.cost, 4);
        assert_eq!(BLOOD_FOR_BLOOD.target, TargetRequirement::Enemy);
        assert_eq!(BLOOD_FOR_BLOOD.card_type, CardType::Attack);
        assert_eq!(BLOOD_FOR_BLOOD.values.damage, Some(18));
        assert_eq!(
            card_type_and_rarity(BLOOD_FOR_BLOOD_ID),
            Some((CardType::Attack, CardRarity::Uncommon))
        );
    }

    #[test]
    fn flame_barrier_has_expected_values_and_rarity() {
        assert_eq!(FLAME_BARRIER.id, FLAME_BARRIER_ID);
        assert_eq!(FLAME_BARRIER.cost, 2);
        assert_eq!(FLAME_BARRIER.target, TargetRequirement::None);
        assert_eq!(FLAME_BARRIER.card_type, CardType::Skill);
        assert_eq!(FLAME_BARRIER.values.block, Some(12));
        assert!(!FLAME_BARRIER.keywords.exhaust);
        assert_eq!(
            card_type_and_rarity(FLAME_BARRIER_ID),
            Some((CardType::Skill, CardRarity::Uncommon))
        );
    }

    #[test]
    fn pommel_strike_plus_deals_three_more_damage() {
        assert_eq!(POMMEL_STRIKE_PLUS.values.damage, Some(12));
    }

    #[test]
    fn battle_trance_plus_is_zero_cost_skill() {
        assert_eq!(BATTLE_TRANCE_PLUS.cost, 0);
        assert_eq!(BATTLE_TRANCE_PLUS.card_type, CardType::Skill);
    }

    #[test]
    fn seeing_red_plus_costs_zero() {
        assert_eq!(SEEING_RED_PLUS.cost, 0);
    }

    #[test]
    fn inflame_has_expected_values() {
        assert_eq!(INFLAME.cost, 1);
        assert_eq!(INFLAME.target, TargetRequirement::None);
        assert_eq!(INFLAME.card_type, CardType::Power);
    }

    #[test]
    fn flex_has_expected_values() {
        assert_eq!(FLEX.cost, 0);
        assert_eq!(FLEX.target, TargetRequirement::None);
        assert_eq!(FLEX.card_type, CardType::Skill);
    }

    #[test]
    fn spot_weakness_has_expected_values() {
        assert_eq!(SPOT_WEAKNESS.cost, 1);
        assert_eq!(SPOT_WEAKNESS.target, TargetRequirement::None);
        assert_eq!(SPOT_WEAKNESS.card_type, CardType::Skill);
    }

    #[test]
    fn inflame_plus_grants_one_more_strength_than_base() {
        assert_eq!(INFLAME_PLUS.card_type, CardType::Power);
    }

    #[test]
    fn flex_plus_is_zero_cost_skill() {
        assert_eq!(FLEX_PLUS.cost, 0);
        assert_eq!(FLEX_PLUS.card_type, CardType::Skill);
    }

    #[test]
    fn spot_weakness_plus_is_one_cost_skill() {
        assert_eq!(SPOT_WEAKNESS_PLUS.cost, 1);
        assert_eq!(SPOT_WEAKNESS_PLUS.card_type, CardType::Skill);
    }

    #[test]
    fn whirlwind_has_expected_values() {
        assert_eq!(WHIRLWIND.cost, 0);
        assert_eq!(WHIRLWIND.target, TargetRequirement::AllEnemies);
        assert_eq!(WHIRLWIND.card_type, CardType::Attack);
        assert_eq!(WHIRLWIND.values.damage, Some(5));
    }

    #[test]
    fn whirlwind_plus_deals_three_more_damage_per_hit() {
        assert_eq!(WHIRLWIND_PLUS.values.damage, Some(8));
    }

    #[test]
    fn havoc_has_expected_values() {
        assert_eq!(HAVOC.cost, 1);
        assert_eq!(HAVOC.card_type, CardType::Skill);
        assert_eq!(HAVOC_PLUS.cost, 0);
    }

    #[test]
    fn warcry_exhausts_and_warcry_plus_draws_two() {
        assert!(WARCRY.keywords.exhaust);
        assert_eq!(WARCRY_PLUS.cost, 0);
    }

    #[test]
    fn dual_wield_exhausts_and_plus_costs_zero() {
        assert!(DUAL_WIELD.keywords.exhaust);
        assert_eq!(DUAL_WIELD_PLUS.cost, 0);
    }

    #[test]
    fn searing_blow_plus_deals_eight_more_damage() {
        assert_eq!(SEARING_BLOW.values.damage, Some(12));
        assert_eq!(SEARING_BLOW_PLUS.values.damage, Some(20));
    }

    #[test]
    fn sever_soul_has_expected_values() {
        assert_eq!(SEVER_SOUL.cost, 2);
        assert_eq!(SEVER_SOUL.target, TargetRequirement::Enemy);
        assert_eq!(SEVER_SOUL.card_type, CardType::Attack);
        assert_eq!(SEVER_SOUL.values.damage, Some(16));
    }
}
