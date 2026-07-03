use crate::{
    card::{
        CardDefinition, CardInstance, CardKeywords, CardRarity, CardType, CardValues,
        TargetRequirement, CARD_KEYWORDS_NONE,
    },
    ContentId,
};

pub const STRIKE_R_ID: ContentId = ContentId::new(1);
pub const DEFEND_R_ID: ContentId = ContentId::new(2);
pub const DEFEND_R_PLUS_ID: ContentId = ContentId::new(2_000_001);
pub const BASH_ID: ContentId = ContentId::new(3);
pub const BASH_PLUS_ID: ContentId = ContentId::new(3_000_001);
pub const WOUND_ID: ContentId = ContentId::new(4);
pub const DAZED_ID: ContentId = ContentId::new(5);
pub const BURN_ID: ContentId = ContentId::new(6);
pub const SLIMED_ID: ContentId = ContentId::new(7);
pub const REGRET_ID: ContentId = ContentId::new(62);
pub const DOUBT_ID: ContentId = ContentId::new(63);
pub const CURSE_OF_THE_BELL_ID: ContentId = ContentId::new(64);
pub const CLUMSY_ID: ContentId = ContentId::new(65);
pub const DECAY_ID: ContentId = ContentId::new(66);
pub const INJURY_ID: ContentId = ContentId::new(67);
pub const NORMALITY_ID: ContentId = ContentId::new(68);
pub const PAIN_ID: ContentId = ContentId::new(69);
pub const PARASITE_ID: ContentId = ContentId::new(70);
pub const SHAME_ID: ContentId = ContentId::new(71);
pub const WRITHE_ID: ContentId = ContentId::new(72);
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
pub const SHRUG_IT_OFF_PLUS_ID: ContentId = ContentId::new(10_016);
pub const TRUE_GRIT_ID: ContentId = ContentId::new(17);
pub const TRUE_GRIT_PLUS_ID: ContentId = ContentId::new(10_017);
pub const BURNING_PACT_ID: ContentId = ContentId::new(18);
pub const BURNING_PACT_PLUS_ID: ContentId = ContentId::new(18_000_001);
pub const FEEL_NO_PAIN_ID: ContentId = ContentId::new(19);
pub const FEEL_NO_PAIN_PLUS_ID: ContentId = ContentId::new(19_000_001);
pub const DARK_EMBRACE_ID: ContentId = ContentId::new(20);
pub const DARK_EMBRACE_PLUS_ID: ContentId = ContentId::new(20_000_001);
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
pub const SWIFT_STRIKE_PLUS_ID: ContentId = ContentId::new(46);
pub const BITE_ID: ContentId = ContentId::new(47);
pub const RITUAL_DAGGER_ID: ContentId = ContentId::new(48);
pub const APPARITION_ID: ContentId = ContentId::new(49);
pub const APPARITION_PLUS_ID: ContentId = ContentId::new(49_000_001);
pub const JAX_ID: ContentId = ContentId::new(50);
pub const JAX_PLUS_ID: ContentId = ContentId::new(50_000_001);
pub const BANDAGE_UP_ID: ContentId = ContentId::new(1_802_661_242_803_912);
pub const BANDAGE_UP_PLUS_ID: ContentId = ContentId::new(1_802_661_242_803_913);
pub const APOTHEOSIS_ID: ContentId = ContentId::new(1_789_056_897_720_887);
pub const APOTHEOSIS_PLUS_ID: ContentId = ContentId::new(1_789_056_897_720_888);
pub const BLIND_ID: ContentId = ContentId::new(63_289_741);
pub const BLIND_PLUS_ID: ContentId = ContentId::new(63_289_742);
pub const DARK_SHACKLES_ID: ContentId = ContentId::new(18_388_408_013_683_944_583);
pub const DARK_SHACKLES_PLUS_ID: ContentId = ContentId::new(18_388_408_013_683_944_584);
pub const DEEP_BREATH_ID: ContentId = ContentId::new(57_620_194_214_716_779);
pub const DEEP_BREATH_PLUS_ID: ContentId = ContentId::new(57_620_194_214_716_780);
pub const DISCOVERY_ID: ContentId = ContentId::new(60_080_667_924_456);
pub const DISCOVERY_PLUS_ID: ContentId = ContentId::new(60_080_667_924_457);
pub const ENLIGHTENMENT_ID: ContentId = ContentId::new(1_054_645_513_201_118_220);
pub const ENLIGHTENMENT_PLUS_ID: ContentId = ContentId::new(1_054_645_513_201_118_221);
pub const FINESSE_ID: ContentId = ContentId::new(64_289_358_915);
pub const FINESSE_PLUS_ID: ContentId = ContentId::new(64_289_358_916);
pub const FLASH_OF_STEEL_ID: ContentId = ContentId::new(18_371_492_448_625_970_986);
pub const FLASH_OF_STEEL_PLUS_ID: ContentId = ContentId::new(18_371_492_448_625_970_987);
pub const FORETHOUGHT_ID: ContentId = ContentId::new(59_534_622_361_962_517);
pub const FORETHOUGHT_PLUS_ID: ContentId = ContentId::new(59_534_622_361_962_518);
pub const GOOD_INSTINCTS_ID: ContentId = ContentId::new(8_602_552_533_669_984_653);
pub const GOOD_INSTINCTS_PLUS_ID: ContentId = ContentId::new(8_602_552_533_669_984_654);
pub const HAND_OF_GREED_ID: ContentId = ContentId::new(3_088_851_373_662_850_713);
pub const HAND_OF_GREED_PLUS_ID: ContentId = ContentId::new(3_088_851_373_662_850_714);
pub const CHRYSALIS_ID: ContentId = ContentId::new(59_200_009_685_460);
pub const CHRYSALIS_PLUS_ID: ContentId = ContentId::new(59_200_009_685_461);
pub const MAGNETISM_ID: ContentId = ContentId::new(67_526_241_934_097);
pub const MAGNETISM_PLUS_ID: ContentId = ContentId::new(67_526_241_934_098);
pub const MIND_BLAST_ID: ContentId = ContentId::new(2_100_321_069_307_395);
pub const MIND_BLAST_PLUS_ID: ContentId = ContentId::new(2_100_321_069_307_396);
pub const PANACEA_ID: ContentId = ContentId::new(72_935_227_539);
pub const PANACEA_PLUS_ID: ContentId = ContentId::new(72_935_227_540);
pub const PANACHE_ID: ContentId = ContentId::new(72_935_227_636);
pub const PANACHE_PLUS_ID: ContentId = ContentId::new(72_935_227_637);
pub const PANIC_BUTTON_ID: ContentId = ContentId::new(2_088_080_471_569_008_754);
pub const PANIC_BUTTON_PLUS_ID: ContentId = ContentId::new(2_088_080_471_569_008_755);
pub const PURITY_ID: ContentId = ContentId::new(2_371_347_673);
pub const PURITY_PLUS_ID: ContentId = ContentId::new(2_371_347_674);
pub const SADISTIC_NATURE_ID: ContentId = ContentId::new(16_049_541_496_988_266_320);
pub const SADISTIC_NATURE_PLUS_ID: ContentId = ContentId::new(16_049_541_496_988_266_321);
pub const TRIP_ID: ContentId = ContentId::new(2_584_189);
pub const TRIP_PLUS_ID: ContentId = ContentId::new(2_584_190);
pub const IMPATIENCE_ID: ContentId = ContentId::new(1_998_026_198_879_085);
pub const IMPATIENCE_PLUS_ID: ContentId = ContentId::new(1_998_026_198_879_086);
pub const JACK_OF_ALL_TRADES_ID: ContentId = ContentId::new(13_737_426_385_707_302_253);
pub const JACK_OF_ALL_TRADES_PLUS_ID: ContentId = ContentId::new(13_737_426_385_707_302_254);
pub const MADNESS_ID: ContentId = ContentId::new(70_263_870_943);
pub const MADNESS_PLUS_ID: ContentId = ContentId::new(70_263_870_944);
pub const MASTER_OF_STRATEGY_ID: ContentId = ContentId::new(9_350_765_816_531_572_950);
pub const MASTER_OF_STRATEGY_PLUS_ID: ContentId = ContentId::new(9_350_765_816_531_572_951);
pub const MAYHEM_ID: ContentId = ContentId::new(2_267_196_899);
pub const SECRET_TECHNIQUE_ID: ContentId = ContentId::new(2_746_448_811_048_118_713);
pub const SECRET_TECHNIQUE_PLUS_ID: ContentId = ContentId::new(2_746_448_811_048_118_714);
pub const SECRET_WEAPON_ID: ContentId = ContentId::new(11_846_108_130_828_291_299);
pub const SECRET_WEAPON_PLUS_ID: ContentId = ContentId::new(11_846_108_130_828_291_300);
pub const VIOLENCE_ID: ContentId = ContentId::new(2_433_206_606_067);
pub const VIOLENCE_PLUS_ID: ContentId = ContentId::new(2_433_206_606_068);
pub const THE_BOMB_ID: ContentId = ContentId::new(2_377_025_041_448);
pub const THE_BOMB_PLUS_ID: ContentId = ContentId::new(2_377_025_041_449);
pub const THINKING_AHEAD_ID: ContentId = ContentId::new(6_777_582_279_578_789_034);
pub const THINKING_AHEAD_PLUS_ID: ContentId = ContentId::new(6_777_582_279_578_789_035);
pub const TRANSMUTATION_ID: ContentId = ContentId::new(12_962_347_838_129_665_929);
pub const TRANSMUTATION_PLUS_ID: ContentId = ContentId::new(12_962_347_838_129_665_930);
pub const METAMORPHOSIS_ID: ContentId = ContentId::new(7_133_622_309_229_402_345);
pub const METAMORPHOSIS_PLUS_ID: ContentId = ContentId::new(7_133_622_309_229_402_346);

pub const IRON_WAVE_ID: ContentId = ContentId::new(100);
pub const IRON_WAVE_PLUS_ID: ContentId = ContentId::new(10_100);
pub const BODY_SLAM_ID: ContentId = ContentId::new(101);
pub const BODY_SLAM_PLUS_ID: ContentId = ContentId::new(10_101);
pub const CLASH_ID: ContentId = ContentId::new(102);
pub const CLASH_PLUS_ID: ContentId = ContentId::new(10_102);
pub const THUNDERCLAP_ID: ContentId = ContentId::new(103);
pub const THUNDERCLAP_PLUS_ID: ContentId = ContentId::new(10_103);
pub const CLOTHESLINE_ID: ContentId = ContentId::new(104);
pub const CLOTHESLINE_PLUS_ID: ContentId = ContentId::new(10_104);
pub const ARMAMENTS_ID: ContentId = ContentId::new(105);
pub const ARMAMENTS_PLUS_ID: ContentId = ContentId::new(10_105);
pub const HEADBUTT_ID: ContentId = ContentId::new(106);
pub const HEADBUTT_PLUS_ID: ContentId = ContentId::new(10_106);
pub const WILD_STRIKE_ID: ContentId = ContentId::new(107);
pub const WILD_STRIKE_PLUS_ID: ContentId = ContentId::new(10_107);
pub const HEAVY_BLADE_ID: ContentId = ContentId::new(108);
pub const HEAVY_BLADE_PLUS_ID: ContentId = ContentId::new(10_108);
pub const PERFECTED_STRIKE_ID: ContentId = ContentId::new(109);
pub const PERFECTED_STRIKE_PLUS_ID: ContentId = ContentId::new(10_109);
pub const SWORD_BOOMERANG_ID: ContentId = ContentId::new(110);
pub const SWORD_BOOMERANG_PLUS_ID: ContentId = ContentId::new(10_110);
pub const POWER_THROUGH_ID: ContentId = ContentId::new(111);
pub const POWER_THROUGH_PLUS_ID: ContentId = ContentId::new(10_111);
pub const INFERNAL_BLADE_ID: ContentId = ContentId::new(112);
pub const INFERNAL_BLADE_PLUS_ID: ContentId = ContentId::new(10_112);
pub const RECKLESS_CHARGE_ID: ContentId = ContentId::new(113);
pub const RECKLESS_CHARGE_PLUS_ID: ContentId = ContentId::new(10_113);
pub const HEMOKINESIS_ID: ContentId = ContentId::new(114);
pub const HEMOKINESIS_PLUS_ID: ContentId = ContentId::new(10_114);
pub const INTIMIDATE_ID: ContentId = ContentId::new(115);
pub const INTIMIDATE_PLUS_ID: ContentId = ContentId::new(10_115);
pub const BLOOD_FOR_BLOOD_ID: ContentId = ContentId::new(116);
pub const BLOOD_FOR_BLOOD_PLUS_ID: ContentId = ContentId::new(10_116);
pub const FLAME_BARRIER_ID: ContentId = ContentId::new(117);
pub const FLAME_BARRIER_PLUS_ID: ContentId = ContentId::new(10_117);
pub const PUMMEL_ID: ContentId = ContentId::new(118);
pub const PUMMEL_PLUS_ID: ContentId = ContentId::new(10_118);
pub const METALLICIZE_ID: ContentId = ContentId::new(119);
pub const METALLICIZE_PLUS_ID: ContentId = ContentId::new(10_119);
pub const SHOCKWAVE_ID: ContentId = ContentId::new(120);
pub const SHOCKWAVE_PLUS_ID: ContentId = ContentId::new(10_120);
pub const RAMPAGE_ID: ContentId = ContentId::new(121);
pub const RAMPAGE_PLUS_ID: ContentId = ContentId::new(10_121);
pub const SEVER_SOUL_ID: ContentId = ContentId::new(122);
pub const SEVER_SOUL_PLUS_ID: ContentId = ContentId::new(10_122);
pub const COMBUST_ID: ContentId = ContentId::new(123);
pub const COMBUST_PLUS_ID: ContentId = ContentId::new(123_000_001);
pub const DISARM_ID: ContentId = ContentId::new(124);
pub const DISARM_PLUS_ID: ContentId = ContentId::new(10_124);
pub const RAGE_ID: ContentId = ContentId::new(125);
pub const RAGE_PLUS_ID: ContentId = ContentId::new(10_125);
pub const ENTRENCH_ID: ContentId = ContentId::new(126);
pub const ENTRENCH_PLUS_ID: ContentId = ContentId::new(10_126);
pub const SENTINEL_ID: ContentId = ContentId::new(127);
pub const SENTINEL_PLUS_ID: ContentId = ContentId::new(10_127);
pub const SECOND_WIND_ID: ContentId = ContentId::new(128);
pub const SECOND_WIND_PLUS_ID: ContentId = ContentId::new(10_128);
pub const RUPTURE_ID: ContentId = ContentId::new(129);
pub const RUPTURE_PLUS_ID: ContentId = ContentId::new(129_000_001);
pub const BLOODLETTING_ID: ContentId = ContentId::new(130);
pub const BLOODLETTING_PLUS_ID: ContentId = ContentId::new(10_130);
pub const CARNAGE_ID: ContentId = ContentId::new(131);
pub const CARNAGE_PLUS_ID: ContentId = ContentId::new(10_131);
pub const DROPKICK_ID: ContentId = ContentId::new(132);
pub const DROPKICK_PLUS_ID: ContentId = ContentId::new(10_132);
pub const FIRE_BREATHING_ID: ContentId = ContentId::new(133);
pub const FIRE_BREATHING_PLUS_ID: ContentId = ContentId::new(133_000_001);
pub const GHOSTLY_ARMOR_ID: ContentId = ContentId::new(134);
pub const GHOSTLY_ARMOR_PLUS_ID: ContentId = ContentId::new(10_134);
pub const UPPERCUT_ID: ContentId = ContentId::new(135);
pub const UPPERCUT_PLUS_ID: ContentId = ContentId::new(10_135);
pub const EVOLVE_ID: ContentId = ContentId::new(136);
pub const EVOLVE_PLUS_ID: ContentId = ContentId::new(136_000_001);
pub const DOUBLE_TAP_ID: ContentId = ContentId::new(137);
pub const DOUBLE_TAP_PLUS_ID: ContentId = ContentId::new(137_000_001);
pub const DEMON_FORM_ID: ContentId = ContentId::new(138);
pub const DEMON_FORM_PLUS_ID: ContentId = ContentId::new(138_000_001);
pub const BLUDGEON_ID: ContentId = ContentId::new(139);
pub const BLUDGEON_PLUS_ID: ContentId = ContentId::new(10_139);
pub const FEED_ID: ContentId = ContentId::new(140);
pub const FEED_PLUS_ID: ContentId = ContentId::new(10_140);
pub const LIMIT_BREAK_ID: ContentId = ContentId::new(141);
pub const LIMIT_BREAK_PLUS_ID: ContentId = ContentId::new(10_141);
pub const CORRUPTION_ID: ContentId = ContentId::new(142);
pub const CORRUPTION_PLUS_ID: ContentId = ContentId::new(142_000_001);
pub const BARRICADE_ID: ContentId = ContentId::new(143);
pub const BARRICADE_PLUS_ID: ContentId = ContentId::new(143_000_001);
pub const FIEND_FIRE_ID: ContentId = ContentId::new(144);
pub const FIEND_FIRE_PLUS_ID: ContentId = ContentId::new(144_000_001);
pub const BERSERK_ID: ContentId = ContentId::new(145);
pub const BERSERK_PLUS_ID: ContentId = ContentId::new(145_000_001);
pub const IMPERVIOUS_ID: ContentId = ContentId::new(146);
pub const IMPERVIOUS_PLUS_ID: ContentId = ContentId::new(10_146);
pub const JUGGERNAUT_ID: ContentId = ContentId::new(147);
pub const JUGGERNAUT_PLUS_ID: ContentId = ContentId::new(147_000_001);
pub const BRUTALITY_ID: ContentId = ContentId::new(148);
pub const BRUTALITY_PLUS_ID: ContentId = ContentId::new(148_000_001);
pub const REAPER_ID: ContentId = ContentId::new(149);
pub const REAPER_PLUS_ID: ContentId = ContentId::new(10_149);
pub const EXHUME_ID: ContentId = ContentId::new(150);
pub const EXHUME_PLUS_ID: ContentId = ContentId::new(150_000_001);
pub const OFFERING_ID: ContentId = ContentId::new(151);
pub const OFFERING_PLUS_ID: ContentId = ContentId::new(10_151);
pub const IMMOLATE_ID: ContentId = ContentId::new(152);
pub const IMMOLATE_PLUS_ID: ContentId = ContentId::new(152_000_001);

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

pub const DEFEND_R_PLUS: CardDefinition = CardDefinition {
    id: DEFEND_R_PLUS_ID,
    key: "Defend_R+",
    name: "Defend+",
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

pub const BASH_PLUS: CardDefinition = CardDefinition {
    id: BASH_PLUS_ID,
    key: "Bash+",
    name: "Bash+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(10),
        block: None,
        vulnerable: Some(3),
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
pub const COMBUST_PLUS_DAMAGE: i32 = 7;
pub const THE_BOMB_DAMAGE: i32 = 40;
pub const THE_BOMB_TURNS: i32 = 3;

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

pub const CLUMSY: CardDefinition = inert_curse(CLUMSY_ID, "Clumsy", "Clumsy", true, false);
pub const DECAY: CardDefinition = inert_curse(DECAY_ID, "Decay", "Decay", false, false);
pub const INJURY: CardDefinition = inert_curse(INJURY_ID, "Injury", "Injury", false, false);
pub const NORMALITY: CardDefinition =
    inert_curse(NORMALITY_ID, "Normality", "Normality", false, false);
pub const PAIN: CardDefinition = inert_curse(PAIN_ID, "Pain", "Pain", false, false);
pub const PARASITE: CardDefinition = inert_curse(PARASITE_ID, "Parasite", "Parasite", false, false);
pub const SHAME: CardDefinition = inert_curse(SHAME_ID, "Shame", "Shame", false, false);
pub const WRITHE: CardDefinition = inert_curse(WRITHE_ID, "Writhe", "Writhe", false, true);

const fn inert_curse(
    id: ContentId,
    key: &'static str,
    name: &'static str,
    ethereal: bool,
    innate: bool,
) -> CardDefinition {
    CardDefinition {
        id,
        key,
        name,
        cost: 0,
        card_type: CardType::Status,
        target: TargetRequirement::None,
        values: CardValues {
            damage: None,
            block: None,
            vulnerable: None,
        },
        keywords: CardKeywords {
            innate,
            ethereal,
            exhaust: false,
            retain: false,
            unplayable: true,
        },
    }
}

pub const SLIMED: CardDefinition = CardDefinition {
    id: SLIMED_ID,
    key: "Slimed",
    name: "Slimed",
    cost: 1,
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
        damage: Some(8),
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
        damage: Some(11),
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
        damage: Some(7),
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

pub const SHRUG_IT_OFF_PLUS: CardDefinition = CardDefinition {
    id: SHRUG_IT_OFF_PLUS_ID,
    key: "Shrug It Off+",
    name: "Shrug It Off+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(11),
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

pub const TRUE_GRIT_PLUS: CardDefinition = CardDefinition {
    id: TRUE_GRIT_PLUS_ID,
    key: "True Grit+",
    name: "True Grit+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(9),
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

pub const BURNING_PACT_PLUS: CardDefinition = CardDefinition {
    id: BURNING_PACT_PLUS_ID,
    key: "Burning Pact+",
    name: "Burning Pact+",
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

pub const FEEL_NO_PAIN_PLUS: CardDefinition = CardDefinition {
    id: FEEL_NO_PAIN_PLUS_ID,
    key: "Feel No Pain+",
    name: "Feel No Pain+",
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
    cost: 2,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DARK_EMBRACE_PLUS: CardDefinition = CardDefinition {
    id: DARK_EMBRACE_PLUS_ID,
    key: "Dark Embrace+",
    name: "Dark Embrace+",
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

pub const COMBUST_PLUS: CardDefinition = CardDefinition {
    id: COMBUST_PLUS_ID,
    key: "COMBUST+",
    name: "Combust+",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(COMBUST_PLUS_DAMAGE),
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

pub const DEMON_FORM_PLUS: CardDefinition = CardDefinition {
    id: DEMON_FORM_PLUS_ID,
    key: "Demon Form+",
    name: "Demon Form+",
    cost: 3,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(3),
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

pub const EVOLVE_PLUS: CardDefinition = CardDefinition {
    id: EVOLVE_PLUS_ID,
    key: "EVOLVE+",
    name: "Evolve+",
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

pub const CORRUPTION_PLUS: CardDefinition = CardDefinition {
    id: CORRUPTION_PLUS_ID,
    key: "CORRUPTION+",
    name: "Corruption+",
    cost: 2,
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

pub const BARRICADE_PLUS: CardDefinition = CardDefinition {
    id: BARRICADE_PLUS_ID,
    key: "BARRICADE+",
    name: "Barricade+",
    cost: 2,
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

pub const BERSERK_PLUS: CardDefinition = CardDefinition {
    id: BERSERK_PLUS_ID,
    key: "BERSERK+",
    name: "Berserk+",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: Some(1),
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

pub const RUPTURE_PLUS: CardDefinition = CardDefinition {
    id: RUPTURE_PLUS_ID,
    key: "RUPTURE+",
    name: "Rupture+",
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

pub const JUGGERNAUT_PLUS: CardDefinition = CardDefinition {
    id: JUGGERNAUT_PLUS_ID,
    key: "JUGGERNAUT+",
    name: "Juggernaut+",
    cost: 2,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(7),
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

pub const MAYHEM: CardDefinition = CardDefinition {
    id: MAYHEM_ID,
    key: "MAYHEM",
    name: "Mayhem",
    cost: 2,
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

pub const DOUBLE_TAP_PLUS: CardDefinition = CardDefinition {
    id: DOUBLE_TAP_PLUS_ID,
    key: "DOUBLE_TAP+",
    name: "Double Tap+",
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

pub const FIRE_BREATHING_PLUS: CardDefinition = CardDefinition {
    id: FIRE_BREATHING_PLUS_ID,
    key: "FIRE_BREATHING+",
    name: "Fire Breathing+",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(10),
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

pub const LIMIT_BREAK_PLUS: CardDefinition = CardDefinition {
    id: LIMIT_BREAK_PLUS_ID,
    key: "Limit Break+",
    name: "Limit Break+",
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

pub const OFFERING_PLUS: CardDefinition = CardDefinition {
    id: OFFERING_PLUS_ID,
    key: "Offering+",
    name: "Offering+",
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

pub const ARMAMENTS_PLUS: CardDefinition = CardDefinition {
    id: ARMAMENTS_PLUS_ID,
    key: "Armaments+",
    name: "Armaments+",
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
    keywords: CardKeywords {
        exhaust: true,
        ..CARD_KEYWORDS_NONE
    },
};

pub const POMMEL_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: POMMEL_STRIKE_PLUS_ID,
    key: "Pommel Strike+",
    name: "Pommel Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(10),
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
    keywords: CardKeywords {
        exhaust: true,
        ..CARD_KEYWORDS_NONE
    },
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
    target: TargetRequirement::Enemy,
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
    target: TargetRequirement::Enemy,
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
    cost: -1,
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
    cost: -1,
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
    keywords: CARD_KEYWORDS_NONE,
};

pub const DUAL_WIELD_PLUS: CardDefinition = CardDefinition {
    id: DUAL_WIELD_PLUS_ID,
    key: "Dual Wield+",
    name: "Dual Wield+",
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
        damage: Some(16),
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

pub const HEADBUTT_PLUS: CardDefinition = CardDefinition {
    id: HEADBUTT_PLUS_ID,
    key: "Headbutt+",
    name: "Headbutt+",
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

pub const BRUTALITY_PLUS: CardDefinition = CardDefinition {
    id: BRUTALITY_PLUS_ID,
    key: "BRUTALITY+",
    name: "Brutality+",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: true,
        ..CARD_KEYWORDS_NONE
    },
};

pub const BANDAGE_UP_PLUS: CardDefinition = CardDefinition {
    id: BANDAGE_UP_PLUS_ID,
    key: "BANDAGE_UP+",
    name: "Bandage Up+",
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

pub const APOTHEOSIS: CardDefinition = CardDefinition {
    id: APOTHEOSIS_ID,
    key: "APOTHEOSIS",
    name: "Apotheosis",
    cost: 2,
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
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BLIND_PLUS: CardDefinition = CardDefinition {
    id: BLIND_PLUS_ID,
    key: "BLIND+",
    name: "Blind+",
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

pub const ENLIGHTENMENT: CardDefinition = CardDefinition {
    id: ENLIGHTENMENT_ID,
    key: "ENLIGHTENMENT",
    name: "Enlightenment",
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

pub const ENLIGHTENMENT_PLUS: CardDefinition = CardDefinition {
    id: ENLIGHTENMENT_PLUS_ID,
    key: "ENLIGHTENMENT+",
    name: "Enlightenment+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: ENLIGHTENMENT.values,
    keywords: ENLIGHTENMENT.keywords,
};

pub const SWIFT_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: SWIFT_STRIKE_PLUS_ID,
    key: "Swift Strike+",
    name: "Swift Strike+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(10),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BITE: CardDefinition = CardDefinition {
    id: BITE_ID,
    key: "Bite",
    name: "Bite",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const RITUAL_DAGGER: CardDefinition = CardDefinition {
    id: RITUAL_DAGGER_ID,
    key: "RitualDagger",
    name: "Ritual Dagger",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(15),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        exhaust: true,
        ethereal: false,
        innate: false,
        retain: false,
        unplayable: false,
    },
};

pub const APPARITION: CardDefinition = CardDefinition {
    id: APPARITION_ID,
    key: "Apparition",
    name: "Apparition",
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
        ethereal: true,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const APPARITION_PLUS: CardDefinition = CardDefinition {
    id: APPARITION_PLUS_ID,
    key: "Apparition+",
    name: "Apparition+",
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

pub const JAX: CardDefinition = CardDefinition {
    id: JAX_ID,
    key: "J.A.X.",
    name: "J.A.X.",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(3),
        block: None,
        vulnerable: Some(2),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const JAX_PLUS: CardDefinition = CardDefinition {
    id: JAX_PLUS_ID,
    key: "J.A.X.+",
    name: "J.A.X.+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(3),
        block: None,
        vulnerable: Some(3),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DEEP_BREATH: CardDefinition = CardDefinition {
    id: DEEP_BREATH_ID,
    key: "DEEP_BREATH",
    name: "Deep Breath",
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

pub const DEEP_BREATH_PLUS: CardDefinition = CardDefinition {
    id: DEEP_BREATH_PLUS_ID,
    key: "DEEP_BREATH+",
    name: "Deep Breath+",
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

pub const DISCOVERY: CardDefinition = CardDefinition {
    id: DISCOVERY_ID,
    key: "DISCOVERY",
    name: "Discovery",
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

pub const FLASH_OF_STEEL_PLUS: CardDefinition = CardDefinition {
    id: FLASH_OF_STEEL_PLUS_ID,
    key: "Flash of Steel+",
    name: "Flash of Steel+",
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

pub const MIND_BLAST: CardDefinition = CardDefinition {
    id: MIND_BLAST_ID,
    key: "Mind Blast",
    name: "Mind Blast",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: true,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const DISCOVERY_PLUS: CardDefinition = CardDefinition {
    id: DISCOVERY_PLUS_ID,
    key: "DISCOVERY+",
    name: "Discovery+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: DISCOVERY.values,
    keywords: CARD_KEYWORDS_NONE,
};

pub const APOTHEOSIS_PLUS: CardDefinition = CardDefinition {
    id: APOTHEOSIS_PLUS_ID,
    key: "APOTHEOSIS+",
    name: "Apotheosis+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: APOTHEOSIS.values,
    keywords: APOTHEOSIS.keywords,
};

pub const MIND_BLAST_PLUS: CardDefinition = CardDefinition {
    id: MIND_BLAST_PLUS_ID,
    key: "Mind Blast+",
    name: "Mind Blast+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        innate: true,
        ethereal: false,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const DARK_SHACKLES: CardDefinition = CardDefinition {
    id: DARK_SHACKLES_ID,
    key: "DARK_SHACKLES",
    name: "Dark Shackles",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::Enemy,
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

pub const DARK_SHACKLES_PLUS: CardDefinition = CardDefinition {
    id: DARK_SHACKLES_PLUS_ID,
    key: "DARK_SHACKLES+",
    name: "Dark Shackles+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::Enemy,
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

pub const FORETHOUGHT: CardDefinition = CardDefinition {
    id: FORETHOUGHT_ID,
    key: "FORETHOUGHT",
    name: "Forethought",
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

pub const FORETHOUGHT_PLUS: CardDefinition = CardDefinition {
    id: FORETHOUGHT_PLUS_ID,
    key: "FORETHOUGHT+",
    name: "Forethought+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: FORETHOUGHT.values,
    keywords: FORETHOUGHT.keywords,
};

pub const GOOD_INSTINCTS_PLUS: CardDefinition = CardDefinition {
    id: GOOD_INSTINCTS_PLUS_ID,
    key: "GOOD_INSTINCTS+",
    name: "Good Instincts+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(9),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const HAND_OF_GREED: CardDefinition = CardDefinition {
    id: HAND_OF_GREED_ID,
    key: "HAND_OF_GREED",
    name: "Hand Of Greed",
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

pub const HAND_OF_GREED_PLUS: CardDefinition = CardDefinition {
    id: HAND_OF_GREED_PLUS_ID,
    key: "HAND_OF_GREED+",
    name: "Hand Of Greed+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(25),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FINESSE_PLUS: CardDefinition = CardDefinition {
    id: FINESSE_PLUS_ID,
    key: "FINESSE+",
    name: "Finesse+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(4),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const PANACEA: CardDefinition = CardDefinition {
    id: PANACEA_ID,
    key: "PANACEA",
    name: "Panacea",
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

pub const PANACEA_PLUS: CardDefinition = CardDefinition {
    id: PANACEA_PLUS_ID,
    key: "PANACEA+",
    name: "Panacea+",
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

pub const PANACHE: CardDefinition = CardDefinition {
    id: PANACHE_ID,
    key: "PANACHE",
    name: "Panache",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(10),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const PURITY: CardDefinition = CardDefinition {
    id: PURITY_ID,
    key: "PURITY",
    name: "Purity",
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

pub const MADNESS: CardDefinition = CardDefinition {
    id: MADNESS_ID,
    key: "MADNESS",
    name: "Madness",
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

pub const PURITY_PLUS: CardDefinition = CardDefinition {
    id: PURITY_PLUS_ID,
    key: "PURITY+",
    name: "Purity+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: PURITY.values,
    keywords: PURITY.keywords,
};

pub const MADNESS_PLUS: CardDefinition = CardDefinition {
    id: MADNESS_PLUS_ID,
    key: "MADNESS+",
    name: "Madness+",
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

pub const MASTER_OF_STRATEGY: CardDefinition = CardDefinition {
    id: MASTER_OF_STRATEGY_ID,
    key: "MASTER_OF_STRATEGY",
    name: "Master of Strategy",
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

pub const SECRET_TECHNIQUE: CardDefinition = CardDefinition {
    id: SECRET_TECHNIQUE_ID,
    key: "SECRET_TECHNIQUE",
    name: "Secret Technique",
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

pub const MASTER_OF_STRATEGY_PLUS: CardDefinition = CardDefinition {
    id: MASTER_OF_STRATEGY_PLUS_ID,
    key: "MASTER_OF_STRATEGY+",
    name: "Master of Strategy+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: MASTER_OF_STRATEGY.values,
    keywords: MASTER_OF_STRATEGY.keywords,
};

pub const SECRET_TECHNIQUE_PLUS: CardDefinition = CardDefinition {
    id: SECRET_TECHNIQUE_PLUS_ID,
    key: "SECRET_TECHNIQUE+",
    name: "Secret Technique+",
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

pub const PANACHE_PLUS: CardDefinition = CardDefinition {
    id: PANACHE_PLUS_ID,
    key: "PANACHE+",
    name: "Panache+",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(14),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SECRET_WEAPON: CardDefinition = CardDefinition {
    id: SECRET_WEAPON_ID,
    key: "SECRET_WEAPON",
    name: "Secret Weapon",
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

pub const SECRET_WEAPON_PLUS: CardDefinition = CardDefinition {
    id: SECRET_WEAPON_PLUS_ID,
    key: "SECRET_WEAPON+",
    name: "Secret Weapon+",
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

pub const VIOLENCE: CardDefinition = CardDefinition {
    id: VIOLENCE_ID,
    key: "VIOLENCE",
    name: "Violence",
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

pub const VIOLENCE_PLUS: CardDefinition = CardDefinition {
    id: VIOLENCE_PLUS_ID,
    key: "VIOLENCE+",
    name: "Violence+",
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

pub const THE_BOMB: CardDefinition = CardDefinition {
    id: THE_BOMB_ID,
    key: "THE_BOMB",
    name: "The Bomb",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(THE_BOMB_DAMAGE),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const THINKING_AHEAD: CardDefinition = CardDefinition {
    id: THINKING_AHEAD_ID,
    key: "THINKING_AHEAD",
    name: "Thinking Ahead",
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

pub const TRANSMUTATION: CardDefinition = CardDefinition {
    id: TRANSMUTATION_ID,
    key: "TRANSMUTATION",
    name: "Transmutation",
    cost: -1,
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

pub const TRANSMUTATION_PLUS: CardDefinition = CardDefinition {
    id: TRANSMUTATION_PLUS_ID,
    key: "TRANSMUTATION+",
    name: "Transmutation+",
    cost: -1,
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

pub const METAMORPHOSIS: CardDefinition = CardDefinition {
    id: METAMORPHOSIS_ID,
    key: "METAMORPHOSIS",
    name: "Metamorphosis",
    cost: 2,
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

pub const JACK_OF_ALL_TRADES: CardDefinition = CardDefinition {
    id: JACK_OF_ALL_TRADES_ID,
    key: "JACK_OF_ALL_TRADES",
    name: "Jack Of All Trades",
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

pub const IMPATIENCE: CardDefinition = CardDefinition {
    id: IMPATIENCE_ID,
    key: "IMPATIENCE",
    name: "Impatience",
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

pub const CHRYSALIS: CardDefinition = CardDefinition {
    id: CHRYSALIS_ID,
    key: "CHRYSALIS",
    name: "Chrysalis",
    cost: 2,
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

pub const SADISTIC_NATURE: CardDefinition = CardDefinition {
    id: SADISTIC_NATURE_ID,
    key: "SADISTIC_NATURE",
    name: "Sadistic Nature",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const PANIC_BUTTON: CardDefinition = CardDefinition {
    id: PANIC_BUTTON_ID,
    key: "PANIC_BUTTON",
    name: "Panic Button",
    cost: 0,
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

pub const JACK_OF_ALL_TRADES_PLUS: CardDefinition = CardDefinition {
    id: JACK_OF_ALL_TRADES_PLUS_ID,
    key: "JACK_OF_ALL_TRADES+",
    name: "Jack Of All Trades+",
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

pub const MAGNETISM: CardDefinition = CardDefinition {
    id: MAGNETISM_ID,
    key: "MAGNETISM",
    name: "Magnetism",
    cost: 2,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const THE_BOMB_PLUS: CardDefinition = CardDefinition {
    id: THE_BOMB_PLUS_ID,
    key: "THE_BOMB+",
    name: "The Bomb+",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(50),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const THINKING_AHEAD_PLUS: CardDefinition = CardDefinition {
    id: THINKING_AHEAD_PLUS_ID,
    key: "THINKING_AHEAD+",
    name: "Thinking Ahead+",
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

pub const METAMORPHOSIS_PLUS: CardDefinition = CardDefinition {
    id: METAMORPHOSIS_PLUS_ID,
    key: "METAMORPHOSIS+",
    name: "Metamorphosis+",
    cost: 2,
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

pub const IMPATIENCE_PLUS: CardDefinition = CardDefinition {
    id: IMPATIENCE_PLUS_ID,
    key: "IMPATIENCE+",
    name: "Impatience+",
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

pub const CHRYSALIS_PLUS: CardDefinition = CardDefinition {
    id: CHRYSALIS_PLUS_ID,
    key: "CHRYSALIS+",
    name: "Chrysalis+",
    cost: 2,
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

pub const SADISTIC_NATURE_PLUS: CardDefinition = CardDefinition {
    id: SADISTIC_NATURE_PLUS_ID,
    key: "SADISTIC_NATURE+",
    name: "Sadistic Nature+",
    cost: 0,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const PANIC_BUTTON_PLUS: CardDefinition = CardDefinition {
    id: PANIC_BUTTON_PLUS_ID,
    key: "PANIC_BUTTON+",
    name: "Panic Button+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(40),
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

pub const MAGNETISM_PLUS: CardDefinition = CardDefinition {
    id: MAGNETISM_PLUS_ID,
    key: "MAGNETISM+",
    name: "Magnetism+",
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

pub const TRIP: CardDefinition = CardDefinition {
    id: TRIP_ID,
    key: "TRIP",
    name: "Trip",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: Some(2),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TRIP_PLUS: CardDefinition = CardDefinition {
    id: TRIP_PLUS_ID,
    key: "TRIP+",
    name: "Trip+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: Some(2),
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

pub const IRON_WAVE_PLUS: CardDefinition = CardDefinition {
    id: IRON_WAVE_PLUS_ID,
    key: "IRON_WAVE+",
    name: "Iron Wave+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: Some(7),
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

pub const BODY_SLAM_PLUS: CardDefinition = CardDefinition {
    id: BODY_SLAM_PLUS_ID,
    key: "BODY_SLAM+",
    name: "Body Slam+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: BODY_SLAM.values,
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

pub const CLASH_PLUS: CardDefinition = CardDefinition {
    id: CLASH_PLUS_ID,
    key: "CLASH+",
    name: "Clash+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(18),
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

pub const WILD_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: WILD_STRIKE_PLUS_ID,
    key: "WILD_STRIKE+",
    name: "Wild Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(17),
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

pub const HEAVY_BLADE_PLUS: CardDefinition = CardDefinition {
    id: HEAVY_BLADE_PLUS_ID,
    key: "HEAVY_BLADE+",
    name: "Heavy Blade+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: HEAVY_BLADE.values,
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

pub const PERFECTED_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: PERFECTED_STRIKE_PLUS_ID,
    key: "PERFECTED_STRIKE+",
    name: "Perfected Strike+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: PERFECTED_STRIKE.values,
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

pub const RAMPAGE_PLUS: CardDefinition = CardDefinition {
    id: RAMPAGE_PLUS_ID,
    key: "RAMPAGE+",
    name: "Rampage+",
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

pub const POWER_THROUGH_PLUS: CardDefinition = CardDefinition {
    id: POWER_THROUGH_PLUS_ID,
    key: "POWER_THROUGH+",
    name: "Power Through+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(20),
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
    keywords: CardKeywords {
        exhaust: true,
        ..CARD_KEYWORDS_NONE
    },
};

pub const INFERNAL_BLADE_PLUS: CardDefinition = CardDefinition {
    id: INFERNAL_BLADE_PLUS_ID,
    key: "INFERNAL_BLADE+",
    name: "Infernal Blade+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: INFERNAL_BLADE.values,
    keywords: INFERNAL_BLADE.keywords,
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

pub const ENTRENCH_PLUS: CardDefinition = CardDefinition {
    id: ENTRENCH_PLUS_ID,
    key: "ENTRENCH+",
    name: "Entrench+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: ENTRENCH.values,
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

pub const GHOSTLY_ARMOR_PLUS: CardDefinition = CardDefinition {
    id: GHOSTLY_ARMOR_PLUS_ID,
    key: "GHOSTLY_ARMOR+",
    name: "Ghostly Armor+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(13),
        vulnerable: None,
    },
    keywords: GHOSTLY_ARMOR.keywords,
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

pub const FLAME_BARRIER_PLUS: CardDefinition = CardDefinition {
    id: FLAME_BARRIER_PLUS_ID,
    key: "FLAME_BARRIER+",
    name: "Flame Barrier+",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(16),
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

pub const RECKLESS_CHARGE_PLUS: CardDefinition = CardDefinition {
    id: RECKLESS_CHARGE_PLUS_ID,
    key: "RECKLESS_CHARGE+",
    name: "Reckless Charge+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(10),
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

pub const PUMMEL_PLUS: CardDefinition = CardDefinition {
    id: PUMMEL_PLUS_ID,
    key: "PUMMEL+",
    name: "Pummel+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: PUMMEL.values,
    keywords: PUMMEL.keywords,
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

pub const CLOTHESLINE_PLUS: CardDefinition = CardDefinition {
    id: CLOTHESLINE_PLUS_ID,
    key: "CLOTHESLINE+",
    name: "Clothesline+",
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

pub const INTIMIDATE_PLUS: CardDefinition = CardDefinition {
    id: INTIMIDATE_PLUS_ID,
    key: "INTIMIDATE+",
    name: "Intimidate+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: INTIMIDATE.values,
    keywords: INTIMIDATE.keywords,
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

pub const SHOCKWAVE_PLUS: CardDefinition = CardDefinition {
    id: SHOCKWAVE_PLUS_ID,
    key: "SHOCKWAVE+",
    name: "Shockwave+",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: Some(5),
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

pub const DISARM_PLUS: CardDefinition = CardDefinition {
    id: DISARM_PLUS_ID,
    key: "DISARM+",
    name: "Disarm+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::Enemy,
    values: DISARM.values,
    keywords: DISARM.keywords,
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

pub const RAGE_PLUS: CardDefinition = CardDefinition {
    id: RAGE_PLUS_ID,
    key: "RAGE+",
    name: "Rage+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: RAGE.values,
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

pub const SEVER_SOUL_PLUS: CardDefinition = CardDefinition {
    id: SEVER_SOUL_PLUS_ID,
    key: "Sever Soul+",
    name: "Sever Soul+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(22),
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

pub const SECOND_WIND_PLUS: CardDefinition = CardDefinition {
    id: SECOND_WIND_PLUS_ID,
    key: "SECOND_WIND+",
    name: "Second Wind+",
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

pub const SENTINEL_PLUS: CardDefinition = CardDefinition {
    id: SENTINEL_PLUS_ID,
    key: "Sentinel+",
    name: "Sentinel+",
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

pub const BLOODLETTING_PLUS: CardDefinition = CardDefinition {
    id: BLOODLETTING_PLUS_ID,
    key: "Bloodletting+",
    name: "Bloodletting+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: BLOODLETTING.values,
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

pub const CARNAGE_PLUS: CardDefinition = CardDefinition {
    id: CARNAGE_PLUS_ID,
    key: "CARNAGE+",
    name: "Carnage+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(28),
        block: None,
        vulnerable: None,
    },
    keywords: CARNAGE.keywords,
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

pub const DROPKICK_PLUS: CardDefinition = CardDefinition {
    id: DROPKICK_PLUS_ID,
    key: "DROPKICK+",
    name: "Dropkick+",
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

pub const SWORD_BOOMERANG: CardDefinition = CardDefinition {
    id: SWORD_BOOMERANG_ID,
    key: "Sword Boomerang",
    name: "Sword Boomerang",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(3),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SWORD_BOOMERANG_PLUS: CardDefinition = CardDefinition {
    id: SWORD_BOOMERANG_PLUS_ID,
    key: "Sword Boomerang+",
    name: "Sword Boomerang+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
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

pub const HEMOKINESIS_PLUS: CardDefinition = CardDefinition {
    id: HEMOKINESIS_PLUS_ID,
    key: "Hemokinesis+",
    name: "Hemokinesis+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(20),
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

pub const BLOOD_FOR_BLOOD_PLUS: CardDefinition = CardDefinition {
    id: BLOOD_FOR_BLOOD_PLUS_ID,
    key: "BLOOD_FOR_BLOOD+",
    name: "Blood for Blood+",
    cost: 3,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(22),
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

pub const IMMOLATE_PLUS: CardDefinition = CardDefinition {
    id: IMMOLATE_PLUS_ID,
    key: "Immolate+",
    name: "Immolate+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(28),
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

pub const BLUDGEON_PLUS: CardDefinition = CardDefinition {
    id: BLUDGEON_PLUS_ID,
    key: "BLUDGEON+",
    name: "Bludgeon+",
    cost: 3,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(42),
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

pub const FEED_PLUS: CardDefinition = CardDefinition {
    id: FEED_PLUS_ID,
    key: "FEED+",
    name: "Feed+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(12),
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

pub const IMPERVIOUS_PLUS: CardDefinition = CardDefinition {
    id: IMPERVIOUS_PLUS_ID,
    key: "IMPERVIOUS+",
    name: "Impervious+",
    cost: 2,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(40),
        vulnerable: None,
    },
    keywords: IMPERVIOUS.keywords,
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

pub const FIEND_FIRE_PLUS: CardDefinition = CardDefinition {
    id: FIEND_FIRE_PLUS_ID,
    key: "FIEND_FIRE+",
    name: "Fiend Fire+",
    cost: 2,
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

pub const REAPER_PLUS: CardDefinition = CardDefinition {
    id: REAPER_PLUS_ID,
    key: "REAPER+",
    name: "Reaper+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(5),
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

pub const EXHUME_PLUS: CardDefinition = CardDefinition {
    id: EXHUME_PLUS_ID,
    key: "EXHUME+",
    name: "Exhume+",
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

pub const METALLICIZE_PLUS: CardDefinition = CardDefinition {
    id: METALLICIZE_PLUS_ID,
    key: "Metallicize+",
    name: "Metallicize+",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(4),
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
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(4),
        block: None,
        vulnerable: Some(1),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const THUNDERCLAP_PLUS: CardDefinition = CardDefinition {
    id: THUNDERCLAP_PLUS_ID,
    key: "THUNDERCLAP+",
    name: "Thunderclap+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(7),
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

pub const UPPERCUT_PLUS: CardDefinition = CardDefinition {
    id: UPPERCUT_PLUS_ID,
    key: "Uppercut+",
    name: "Uppercut+",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(13),
        block: None,
        vulnerable: Some(2),
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
pub const MILESTONE5_SKILL_CARDS: [CardDefinition; 13] = [
    SHRUG_IT_OFF,
    SHRUG_IT_OFF_PLUS,
    TRUE_GRIT,
    BURNING_PACT,
    BURNING_PACT_PLUS,
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
pub const ALL_CARDS: [CardDefinition; 242] = [
    STRIKE_R,
    STRIKE_R_PLUS,
    DEFEND_R,
    DEFEND_R_PLUS,
    BASH,
    BASH_PLUS,
    WOUND,
    DAZED,
    BURN,
    SLIMED,
    REGRET,
    DOUBT,
    CURSE_OF_THE_BELL,
    CLUMSY,
    DECAY,
    INJURY,
    NORMALITY,
    PAIN,
    PARASITE,
    SHAME,
    WRITHE,
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
    SHRUG_IT_OFF_PLUS,
    TRUE_GRIT,
    TRUE_GRIT_PLUS,
    BURNING_PACT,
    BURNING_PACT_PLUS,
    FEEL_NO_PAIN,
    FEEL_NO_PAIN_PLUS,
    DARK_EMBRACE,
    DARK_EMBRACE_PLUS,
    COMBUST,
    COMBUST_PLUS,
    DEMON_FORM,
    DEMON_FORM_PLUS,
    EVOLVE,
    EVOLVE_PLUS,
    CORRUPTION,
    CORRUPTION_PLUS,
    BARRICADE,
    BARRICADE_PLUS,
    BERSERK,
    BERSERK_PLUS,
    RUPTURE,
    RUPTURE_PLUS,
    JUGGERNAUT,
    JUGGERNAUT_PLUS,
    BRUTALITY,
    BRUTALITY_PLUS,
    MAYHEM,
    DOUBLE_TAP,
    DOUBLE_TAP_PLUS,
    FIRE_BREATHING,
    FIRE_BREATHING_PLUS,
    LIMIT_BREAK,
    LIMIT_BREAK_PLUS,
    OFFERING,
    OFFERING_PLUS,
    ARMAMENTS,
    ARMAMENTS_PLUS,
    HEADBUTT,
    HEADBUTT_PLUS,
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
    BANDAGE_UP_PLUS,
    APOTHEOSIS,
    APOTHEOSIS_PLUS,
    BLIND,
    BLIND_PLUS,
    ENLIGHTENMENT,
    ENLIGHTENMENT_PLUS,
    SWIFT_STRIKE,
    SWIFT_STRIKE_PLUS,
    BITE,
    RITUAL_DAGGER,
    APPARITION,
    APPARITION_PLUS,
    JAX,
    JAX_PLUS,
    DEEP_BREATH,
    DEEP_BREATH_PLUS,
    DISCOVERY,
    DISCOVERY_PLUS,
    FLASH_OF_STEEL,
    FLASH_OF_STEEL_PLUS,
    MIND_BLAST,
    MIND_BLAST_PLUS,
    DARK_SHACKLES,
    DARK_SHACKLES_PLUS,
    FORETHOUGHT,
    FORETHOUGHT_PLUS,
    GOOD_INSTINCTS,
    GOOD_INSTINCTS_PLUS,
    HAND_OF_GREED,
    HAND_OF_GREED_PLUS,
    FINESSE,
    FINESSE_PLUS,
    MAGNETISM,
    MAGNETISM_PLUS,
    PANACEA,
    PANACEA_PLUS,
    PANACHE,
    PANACHE_PLUS,
    PANIC_BUTTON,
    PANIC_BUTTON_PLUS,
    PURITY,
    PURITY_PLUS,
    SADISTIC_NATURE,
    SADISTIC_NATURE_PLUS,
    TRIP,
    TRIP_PLUS,
    IMPATIENCE,
    IMPATIENCE_PLUS,
    CHRYSALIS,
    CHRYSALIS_PLUS,
    JACK_OF_ALL_TRADES,
    JACK_OF_ALL_TRADES_PLUS,
    MADNESS,
    MADNESS_PLUS,
    MASTER_OF_STRATEGY,
    MASTER_OF_STRATEGY_PLUS,
    SECRET_TECHNIQUE,
    SECRET_TECHNIQUE_PLUS,
    SECRET_WEAPON,
    SECRET_WEAPON_PLUS,
    VIOLENCE,
    VIOLENCE_PLUS,
    THE_BOMB,
    THE_BOMB_PLUS,
    THINKING_AHEAD,
    THINKING_AHEAD_PLUS,
    TRANSMUTATION,
    TRANSMUTATION_PLUS,
    METAMORPHOSIS,
    METAMORPHOSIS_PLUS,
    IRON_WAVE,
    IRON_WAVE_PLUS,
    BODY_SLAM,
    BODY_SLAM_PLUS,
    CLASH,
    CLASH_PLUS,
    WILD_STRIKE,
    WILD_STRIKE_PLUS,
    HEAVY_BLADE,
    HEAVY_BLADE_PLUS,
    PERFECTED_STRIKE,
    PERFECTED_STRIKE_PLUS,
    RAMPAGE,
    RAMPAGE_PLUS,
    POWER_THROUGH,
    POWER_THROUGH_PLUS,
    INFERNAL_BLADE,
    INFERNAL_BLADE_PLUS,
    ENTRENCH,
    ENTRENCH_PLUS,
    GHOSTLY_ARMOR,
    GHOSTLY_ARMOR_PLUS,
    FLAME_BARRIER,
    FLAME_BARRIER_PLUS,
    RECKLESS_CHARGE,
    RECKLESS_CHARGE_PLUS,
    PUMMEL,
    PUMMEL_PLUS,
    CLOTHESLINE,
    CLOTHESLINE_PLUS,
    INTIMIDATE,
    INTIMIDATE_PLUS,
    SHOCKWAVE,
    SHOCKWAVE_PLUS,
    DISARM,
    DISARM_PLUS,
    RAGE,
    RAGE_PLUS,
    SEVER_SOUL,
    SEVER_SOUL_PLUS,
    SECOND_WIND,
    SECOND_WIND_PLUS,
    SENTINEL,
    SENTINEL_PLUS,
    BLOODLETTING,
    BLOODLETTING_PLUS,
    CARNAGE,
    CARNAGE_PLUS,
    DROPKICK,
    DROPKICK_PLUS,
    SWORD_BOOMERANG,
    SWORD_BOOMERANG_PLUS,
    HEMOKINESIS,
    HEMOKINESIS_PLUS,
    BLOOD_FOR_BLOOD,
    BLOOD_FOR_BLOOD_PLUS,
    IMMOLATE,
    IMMOLATE_PLUS,
    BLUDGEON,
    BLUDGEON_PLUS,
    FEED,
    FEED_PLUS,
    IMPERVIOUS,
    IMPERVIOUS_PLUS,
    FIEND_FIRE,
    FIEND_FIRE_PLUS,
    REAPER,
    REAPER_PLUS,
    EXHUME,
    EXHUME_PLUS,
    METALLICIZE,
    METALLICIZE_PLUS,
    THUNDERCLAP,
    THUNDERCLAP_PLUS,
    UPPERCUT,
    UPPERCUT_PLUS,
];

#[must_use]
pub fn get_card_definition(id: ContentId) -> Option<&'static CardDefinition> {
    ALL_CARDS.iter().find(|definition| definition.id == id)
}

#[must_use]
pub fn is_curse_content_id(id: ContentId) -> bool {
    matches!(
        id,
        id if id == REGRET_ID
            || id == DOUBT_ID
            || id == CURSE_OF_THE_BELL_ID
            || id == ASCENDERS_BANE_ID
            || id == CLUMSY_ID
            || id == DECAY_ID
            || id == INJURY_ID
            || id == NORMALITY_ID
            || id == PAIN_ID
            || id == PARASITE_ID
            || id == SHAME_ID
            || id == WRITHE_ID
    )
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
        id if id == DEFEND_R_PLUS_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == BASH_ID || id == BASH_PLUS_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == BANDAGE_UP_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BANDAGE_UP_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BLIND_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == BLIND_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == DEEP_BREATH_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == DEEP_BREATH_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == DISCOVERY_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == DISCOVERY_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == ENLIGHTENMENT_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == ENLIGHTENMENT_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == IRON_WAVE_ID || id == IRON_WAVE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == BODY_SLAM_ID || id == BODY_SLAM_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == CLASH_ID || id == CLASH_PLUS_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == ARMAMENTS_ID || id == ARMAMENTS_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Common))
        }
        id if id == HEADBUTT_ID || id == HEADBUTT_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == WILD_STRIKE_ID || id == WILD_STRIKE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == HEAVY_BLADE_ID || id == HEAVY_BLADE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == PERFECTED_STRIKE_ID || id == PERFECTED_STRIKE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == RAMPAGE_ID || id == RAMPAGE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == POWER_THROUGH_ID || id == POWER_THROUGH_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == INFERNAL_BLADE_ID || id == INFERNAL_BLADE_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == ENTRENCH_ID || id == ENTRENCH_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == GHOSTLY_ARMOR_ID || id == GHOSTLY_ARMOR_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == FLAME_BARRIER_ID || id == FLAME_BARRIER_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == RECKLESS_CHARGE_ID || id == RECKLESS_CHARGE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == PUMMEL_ID || id == PUMMEL_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == BLOODLETTING_ID || id == BLOODLETTING_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == CARNAGE_ID || id == CARNAGE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == DROPKICK_ID || id == DROPKICK_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == HEMOKINESIS_ID || id == HEMOKINESIS_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == BLOOD_FOR_BLOOD_ID || id == BLOOD_FOR_BLOOD_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == CLOTHESLINE_ID || id == CLOTHESLINE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == ANGER_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == CLEAVE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == TWIN_STRIKE_ID => Some((CardType::Attack, CardRarity::Common)),
        id if id == SWORD_BOOMERANG_ID || id == SWORD_BOOMERANG_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == SHRUG_IT_OFF_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == SHRUG_IT_OFF_PLUS_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == TRUE_GRIT_ID || id == TRUE_GRIT_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Common))
        }
        id if id == POMMEL_STRIKE_ID || id == POMMEL_STRIKE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == BATTLE_TRANCE_ID || id == BATTLE_TRANCE_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == SEEING_RED_ID || id == SEEING_RED_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == BURNING_PACT_ID || id == BURNING_PACT_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == FEEL_NO_PAIN_ID || id == FEEL_NO_PAIN_PLUS_ID => {
            Some((CardType::Power, CardRarity::Uncommon))
        }
        id if id == DARK_EMBRACE_ID || id == DARK_EMBRACE_PLUS_ID => {
            Some((CardType::Power, CardRarity::Uncommon))
        }
        id if id == COMBUST_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == COMBUST_PLUS_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == DEMON_FORM_ID || id == DEMON_FORM_PLUS_ID => {
            Some((CardType::Power, CardRarity::Rare))
        }
        id if id == EVOLVE_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == EVOLVE_PLUS_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == CORRUPTION_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == CORRUPTION_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BARRICADE_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BARRICADE_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BERSERK_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BERSERK_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == RUPTURE_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == RUPTURE_PLUS_ID => Some((CardType::Power, CardRarity::Uncommon)),
        id if id == JUGGERNAUT_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == JUGGERNAUT_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BRUTALITY_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == BRUTALITY_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == MAYHEM_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == DOUBLE_TAP_ID || id == DOUBLE_TAP_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Rare))
        }
        id if id == FIRE_BREATHING_ID || id == FIRE_BREATHING_PLUS_ID => {
            Some((CardType::Power, CardRarity::Uncommon))
        }
        id if id == LIMIT_BREAK_ID || id == LIMIT_BREAK_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Rare))
        }
        id if id == OFFERING_ID || id == OFFERING_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Rare))
        }
        id if id == INFLAME_ID || id == INFLAME_PLUS_ID => {
            Some((CardType::Power, CardRarity::Uncommon))
        }
        id if id == METALLICIZE_ID || id == METALLICIZE_PLUS_ID => {
            Some((CardType::Power, CardRarity::Uncommon))
        }
        id if id == FLEX_ID || id == FLEX_PLUS_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == SPOT_WEAKNESS_ID || id == SPOT_WEAKNESS_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == THUNDERCLAP_ID || id == THUNDERCLAP_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == WHIRLWIND_ID || id == WHIRLWIND_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == HAVOC_ID || id == HAVOC_PLUS_ID => Some((CardType::Skill, CardRarity::Common)),
        id if id == WARCRY_ID || id == WARCRY_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Common))
        }
        id if id == DUAL_WIELD_ID || id == DUAL_WIELD_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == SEARING_BLOW_ID || id == SEARING_BLOW_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == SECOND_WIND_ID || id == SECOND_WIND_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == SENTINEL_ID || id == SENTINEL_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == INTIMIDATE_ID || id == INTIMIDATE_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == SHOCKWAVE_ID || id == SHOCKWAVE_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == THUNDERCLAP_ID || id == THUNDERCLAP_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Common))
        }
        id if id == DISARM_ID || id == DISARM_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Uncommon))
        }
        id if id == RAGE_ID || id == RAGE_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SEVER_SOUL_ID || id == SEVER_SOUL_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == IMMOLATE_ID || id == IMMOLATE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Rare))
        }
        id if id == BLUDGEON_ID || id == BLUDGEON_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Rare))
        }
        id if id == FEED_ID || id == FEED_PLUS_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == UPPERCUT_ID || id == UPPERCUT_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Uncommon))
        }
        id if id == IMPERVIOUS_ID || id == IMPERVIOUS_PLUS_ID => {
            Some((CardType::Skill, CardRarity::Rare))
        }
        id if id == FIEND_FIRE_ID || id == FIEND_FIRE_PLUS_ID => {
            Some((CardType::Attack, CardRarity::Rare))
        }
        id if id == REAPER_ID || id == REAPER_PLUS_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == EXHUME_ID || id == EXHUME_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == DRAMATIC_ENTRANCE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == APOTHEOSIS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == APOTHEOSIS_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == SWIFT_STRIKE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == SWIFT_STRIKE_PLUS_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == BITE_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == RITUAL_DAGGER_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == APPARITION_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == APPARITION_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == JAX_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == JAX_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == FLASH_OF_STEEL_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == FLASH_OF_STEEL_PLUS_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == MIND_BLAST_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == MIND_BLAST_PLUS_ID => Some((CardType::Attack, CardRarity::Uncommon)),
        id if id == DARK_SHACKLES_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == DARK_SHACKLES_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FORETHOUGHT_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FORETHOUGHT_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == GOOD_INSTINCTS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == GOOD_INSTINCTS_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == HAND_OF_GREED_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == HAND_OF_GREED_PLUS_ID => Some((CardType::Attack, CardRarity::Rare)),
        id if id == FINESSE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == FINESSE_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == MAGNETISM_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == MAGNETISM_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == PANACEA_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == PANACEA_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == PANACHE_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == PANACHE_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == PANIC_BUTTON_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == PANIC_BUTTON_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == PURITY_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == PURITY_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == SADISTIC_NATURE_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == SADISTIC_NATURE_PLUS_ID => Some((CardType::Power, CardRarity::Rare)),
        id if id == TRIP_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == TRIP_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == IMPATIENCE_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == IMPATIENCE_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == CHRYSALIS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == CHRYSALIS_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == JACK_OF_ALL_TRADES_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == JACK_OF_ALL_TRADES_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == MADNESS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == MADNESS_PLUS_ID => Some((CardType::Skill, CardRarity::Uncommon)),
        id if id == MASTER_OF_STRATEGY_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == MASTER_OF_STRATEGY_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == SECRET_TECHNIQUE_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == SECRET_TECHNIQUE_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == SECRET_WEAPON_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == SECRET_WEAPON_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == VIOLENCE_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == VIOLENCE_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == THE_BOMB_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == THE_BOMB_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == THINKING_AHEAD_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == THINKING_AHEAD_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == TRANSMUTATION_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == TRANSMUTATION_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == METAMORPHOSIS_ID => Some((CardType::Skill, CardRarity::Rare)),
        id if id == METAMORPHOSIS_PLUS_ID => Some((CardType::Skill, CardRarity::Rare)),
        _ => None,
    }
}

/// Maps a base card content id to its upgraded (+) version, if one exists.
#[must_use]
pub fn upgrade_content_id(id: ContentId) -> Option<ContentId> {
    match id {
        APPARITION_ID => Some(APPARITION_PLUS_ID),
        ARMAMENTS_ID => Some(ARMAMENTS_PLUS_ID),
        HEADBUTT_ID => Some(HEADBUTT_PLUS_ID),
        BLOOD_FOR_BLOOD_ID => Some(BLOOD_FOR_BLOOD_PLUS_ID),
        FLAME_BARRIER_ID => Some(FLAME_BARRIER_PLUS_ID),
        SECOND_WIND_ID => Some(SECOND_WIND_PLUS_ID),
        INFERNAL_BLADE_ID => Some(INFERNAL_BLADE_PLUS_ID),
        IRON_WAVE_ID => Some(IRON_WAVE_PLUS_ID),
        BODY_SLAM_ID => Some(BODY_SLAM_PLUS_ID),
        CLASH_ID => Some(CLASH_PLUS_ID),
        THUNDERCLAP_ID => Some(THUNDERCLAP_PLUS_ID),
        CLOTHESLINE_ID => Some(CLOTHESLINE_PLUS_ID),
        WILD_STRIKE_ID => Some(WILD_STRIKE_PLUS_ID),
        HEAVY_BLADE_ID => Some(HEAVY_BLADE_PLUS_ID),
        PERFECTED_STRIKE_ID => Some(PERFECTED_STRIKE_PLUS_ID),
        POWER_THROUGH_ID => Some(POWER_THROUGH_PLUS_ID),
        RECKLESS_CHARGE_ID => Some(RECKLESS_CHARGE_PLUS_ID),
        HEMOKINESIS_ID => Some(HEMOKINESIS_PLUS_ID),
        INTIMIDATE_ID => Some(INTIMIDATE_PLUS_ID),
        PUMMEL_ID => Some(PUMMEL_PLUS_ID),
        DISARM_ID => Some(DISARM_PLUS_ID),
        RAGE_ID => Some(RAGE_PLUS_ID),
        ENTRENCH_ID => Some(ENTRENCH_PLUS_ID),
        SENTINEL_ID => Some(SENTINEL_PLUS_ID),
        BLOODLETTING_ID => Some(BLOODLETTING_PLUS_ID),
        CARNAGE_ID => Some(CARNAGE_PLUS_ID),
        DROPKICK_ID => Some(DROPKICK_PLUS_ID),
        FIRE_BREATHING_ID => Some(FIRE_BREATHING_PLUS_ID),
        GHOSTLY_ARMOR_ID => Some(GHOSTLY_ARMOR_PLUS_ID),
        SEVER_SOUL_ID => Some(SEVER_SOUL_PLUS_ID),
        FEEL_NO_PAIN_ID => Some(FEEL_NO_PAIN_PLUS_ID),
        DARK_EMBRACE_ID => Some(DARK_EMBRACE_PLUS_ID),
        IMPERVIOUS_ID => Some(IMPERVIOUS_PLUS_ID),
        SHOCKWAVE_ID => Some(SHOCKWAVE_PLUS_ID),
        RAMPAGE_ID => Some(RAMPAGE_PLUS_ID),
        LIMIT_BREAK_ID => Some(LIMIT_BREAK_PLUS_ID),
        BLUDGEON_ID => Some(BLUDGEON_PLUS_ID),
        FEED_ID => Some(FEED_PLUS_ID),
        EXHUME_ID => Some(EXHUME_PLUS_ID),
        OFFERING_ID => Some(OFFERING_PLUS_ID),
        REAPER_ID => Some(REAPER_PLUS_ID),
        FIEND_FIRE_ID => Some(FIEND_FIRE_PLUS_ID),
        DEFEND_R_ID => Some(DEFEND_R_PLUS_ID),
        BASH_ID => Some(BASH_PLUS_ID),
        IMMOLATE_ID => Some(IMMOLATE_PLUS_ID),
        STRIKE_R_ID => Some(STRIKE_R_PLUS_ID),
        ANGER_ID => Some(ANGER_PLUS_ID),
        CLEAVE_ID => Some(CLEAVE_PLUS_ID),
        TWIN_STRIKE_ID => Some(TWIN_STRIKE_PLUS_ID),
        SHRUG_IT_OFF_ID => Some(SHRUG_IT_OFF_PLUS_ID),
        POMMEL_STRIKE_ID => Some(POMMEL_STRIKE_PLUS_ID),
        SWORD_BOOMERANG_ID => Some(SWORD_BOOMERANG_PLUS_ID),
        UPPERCUT_ID => Some(UPPERCUT_PLUS_ID),
        BATTLE_TRANCE_ID => Some(BATTLE_TRANCE_PLUS_ID),
        SEEING_RED_ID => Some(SEEING_RED_PLUS_ID),
        BURNING_PACT_ID => Some(BURNING_PACT_PLUS_ID),
        INFLAME_ID => Some(INFLAME_PLUS_ID),
        FLEX_ID => Some(FLEX_PLUS_ID),
        SPOT_WEAKNESS_ID => Some(SPOT_WEAKNESS_PLUS_ID),
        WHIRLWIND_ID => Some(WHIRLWIND_PLUS_ID),
        HAVOC_ID => Some(HAVOC_PLUS_ID),
        WARCRY_ID => Some(WARCRY_PLUS_ID),
        DUAL_WIELD_ID => Some(DUAL_WIELD_PLUS_ID),
        SEARING_BLOW_ID => Some(SEARING_BLOW_PLUS_ID),
        SEARING_BLOW_PLUS_ID => Some(SEARING_BLOW_PLUS_ID),
        COMBUST_ID => Some(COMBUST_PLUS_ID),
        RUPTURE_ID => Some(RUPTURE_PLUS_ID),
        EVOLVE_ID => Some(EVOLVE_PLUS_ID),
        DOUBLE_TAP_ID => Some(DOUBLE_TAP_PLUS_ID),
        DEMON_FORM_ID => Some(DEMON_FORM_PLUS_ID),
        CORRUPTION_ID => Some(CORRUPTION_PLUS_ID),
        BARRICADE_ID => Some(BARRICADE_PLUS_ID),
        BERSERK_ID => Some(BERSERK_PLUS_ID),
        JUGGERNAUT_ID => Some(JUGGERNAUT_PLUS_ID),
        BRUTALITY_ID => Some(BRUTALITY_PLUS_ID),
        SWIFT_STRIKE_ID => Some(SWIFT_STRIKE_PLUS_ID),
        BANDAGE_UP_ID => Some(BANDAGE_UP_PLUS_ID),
        BLIND_ID => Some(BLIND_PLUS_ID),
        DARK_SHACKLES_ID => Some(DARK_SHACKLES_PLUS_ID),
        DEEP_BREATH_ID => Some(DEEP_BREATH_PLUS_ID),
        FINESSE_ID => Some(FINESSE_PLUS_ID),
        FLASH_OF_STEEL_ID => Some(FLASH_OF_STEEL_PLUS_ID),
        GOOD_INSTINCTS_ID => Some(GOOD_INSTINCTS_PLUS_ID),
        MIND_BLAST_ID => Some(MIND_BLAST_PLUS_ID),
        PANACEA_ID => Some(PANACEA_PLUS_ID),
        APOTHEOSIS_ID => Some(APOTHEOSIS_PLUS_ID),
        JAX_ID => Some(JAX_PLUS_ID),
        DISCOVERY_ID => Some(DISCOVERY_PLUS_ID),
        ENLIGHTENMENT_ID => Some(ENLIGHTENMENT_PLUS_ID),
        FORETHOUGHT_ID => Some(FORETHOUGHT_PLUS_ID),
        HAND_OF_GREED_ID => Some(HAND_OF_GREED_PLUS_ID),
        CHRYSALIS_ID => Some(CHRYSALIS_PLUS_ID),
        MAGNETISM_ID => Some(MAGNETISM_PLUS_ID),
        PANACHE_ID => Some(PANACHE_PLUS_ID),
        PANIC_BUTTON_ID => Some(PANIC_BUTTON_PLUS_ID),
        PURITY_ID => Some(PURITY_PLUS_ID),
        SADISTIC_NATURE_ID => Some(SADISTIC_NATURE_PLUS_ID),
        TRIP_ID => Some(TRIP_PLUS_ID),
        IMPATIENCE_ID => Some(IMPATIENCE_PLUS_ID),
        JACK_OF_ALL_TRADES_ID => Some(JACK_OF_ALL_TRADES_PLUS_ID),
        MADNESS_ID => Some(MADNESS_PLUS_ID),
        MASTER_OF_STRATEGY_ID => Some(MASTER_OF_STRATEGY_PLUS_ID),
        SECRET_TECHNIQUE_ID => Some(SECRET_TECHNIQUE_PLUS_ID),
        SECRET_WEAPON_ID => Some(SECRET_WEAPON_PLUS_ID),
        VIOLENCE_ID => Some(VIOLENCE_PLUS_ID),
        THE_BOMB_ID => Some(THE_BOMB_PLUS_ID),
        THINKING_AHEAD_ID => Some(THINKING_AHEAD_PLUS_ID),
        TRANSMUTATION_ID => Some(TRANSMUTATION_PLUS_ID),
        METAMORPHOSIS_ID => Some(METAMORPHOSIS_PLUS_ID),
        METALLICIZE_ID => Some(METALLICIZE_PLUS_ID),
        _ => None,
    }
}

#[must_use]
pub fn searing_blow_damage_for_upgrades(upgrades: u8) -> i32 {
    let upgrades = i32::from(upgrades);
    SEARING_BLOW.values.damage.unwrap_or(12) + (upgrades * (upgrades + 7)) / 2
}

#[must_use]
pub fn searing_blow_card_damage(card: &CardInstance) -> Option<i32> {
    match card.content_id {
        SEARING_BLOW_ID => Some(searing_blow_damage_for_upgrades(card.searing_blow_upgrades)),
        SEARING_BLOW_PLUS_ID => Some(searing_blow_damage_for_upgrades(
            card.searing_blow_upgrades.max(1),
        )),
        _ => None,
    }
}

#[must_use]
pub fn upgrade_card_instance(card: CardInstance) -> Option<CardInstance> {
    let upgraded_content_id = upgrade_content_id(card.content_id)?;
    let mut upgraded = card;
    upgraded.content_id = upgraded_content_id;
    if matches!(card.content_id, SEARING_BLOW_ID | SEARING_BLOW_PLUS_ID) {
        upgraded.searing_blow_upgrades =
            card.searing_blow_upgrades
                .max(if card.content_id == SEARING_BLOW_PLUS_ID {
                    1
                } else {
                    0
                })
                + 1;
    }
    Some(upgraded)
}
