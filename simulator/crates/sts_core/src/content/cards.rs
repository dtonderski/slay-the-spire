use crate::{
    card::{
        CardDefinition, CardInstance, CardKeywords, CardRarity, CardType, CardValues,
        TargetRequirement, CARD_KEYWORDS_NONE,
    },
    ContentId, SimError, SimResult,
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
pub const VOID_ID: ContentId = ContentId::new(73);
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
/// Stable synthetic identities used when Prismatic Shard rewards cards that
/// are outside the modeled Ironclad registry.
pub const CHARGE_BATTERY_ANY_COLOR_ID: ContentId = ContentId::new(15_754_310_908_692_596_154);
pub const EMPTY_BODY_ANY_COLOR_ID: ContentId = ContentId::new(1_892_284_736_181_196);
pub const JUST_LUCKY_ANY_COLOR_ID: ContentId = ContentId::new(2_031_388_667_909_133);
pub const GO_FOR_THE_EYES_ANY_COLOR_ID: ContentId = ContentId::new(2_618_527_352_455_044_789);
pub const EQUILIBRIUM_ANY_COLOR_ID: ContentId = ContentId::new(58_770_534_959_378_700);
pub const SNEAKY_STRIKE_ANY_COLOR_ID: ContentId = ContentId::new(12_075_979_460_702_295_972);
pub const PROSTRATE_ANY_COLOR_ID: ContentId = ContentId::new(70_559_886_447_078);
pub const CLOAK_AND_DAGGER_ANY_COLOR_ID: ContentId = ContentId::new(12_608_504_500_537_169_241);
pub const SHIV_ANY_COLOR_ID: ContentId = ContentId::new(2_544_794);
pub const BACKFLIP_ANY_COLOR_ID: ContentId = ContentId::new(1_875_509_849_132);
pub const LESSON_LEARNED_ANY_COLOR_ID: ContentId = ContentId::new(12_246_701_764_208_556_052);
pub const RECYCLE_ANY_COLOR_ID: ContentId = ContentId::new(74_815_307_979);
pub const BIASED_COGNITION_ANY_COLOR_ID: ContentId = ContentId::new(13_922_568_352_391_244_891);
pub const PRESSURE_POINTS_ANY_COLOR_ID: ContentId = ContentId::new(6_032_211_985_609_368_181);
pub const EMPTY_MIND_ANY_COLOR_ID: ContentId = ContentId::new(1_892_284_736_503_420);
pub const TRANQUILITY_ANY_COLOR_ID: ContentId = ContentId::new(71_074_483_415_927_220);
pub const SKIM_ANY_COLOR_ID: ContentId = ContentId::new(2_547_668);
pub const DOPPELGANGER_ANY_COLOR_ID: ContentId = ContentId::new(1_794_712_432_598_607_498);
/// Curse granted by [crate::relic::Relic::Necronomicon] on equip. Unpurgeable.
pub const NECRONOMICURSE_ID: ContentId = ContentId::new(74);
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
pub const DRAMATIC_ENTRANCE_PLUS_ID: ContentId = ContentId::new(10_044);
pub const SWIFT_STRIKE_ID: ContentId = ContentId::new(45);
pub const SWIFT_STRIKE_PLUS_ID: ContentId = ContentId::new(46);
pub const BITE_ID: ContentId = ContentId::new(47);
pub const BITE_PLUS_ID: ContentId = ContentId::new(47_000_001);
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
pub const MAYHEM_PLUS_ID: ContentId = ContentId::new(2_267_196_900);
pub const SECRET_TECHNIQUE_ID: ContentId = ContentId::new(2_746_448_811_048_118_713);
pub const SECRET_TECHNIQUE_PLUS_ID: ContentId = ContentId::new(2_746_448_811_048_118_714);
pub const SECRET_WEAPON_ID: ContentId = ContentId::new(11_846_108_130_828_291_299);
pub const SECRET_WEAPON_PLUS_ID: ContentId = ContentId::new(11_846_108_130_828_291_300);
/// Prismatic/Watcher Blasphemy — id matches `shop_card_content_id("BLASPHEMY")`.
pub const BLASPHEMY_ID: ContentId = ContentId::new(58_441_907_198_357);
pub const BLASPHEMY_PLUS_ID: ContentId = ContentId::new(58_441_907_198_358);
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(STRIKE_R_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(DEFEND_R_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(BASH_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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

/// Unplayable soulbound curse from Necronomicon.onEquip.
pub const NECRONOMICURSE: CardDefinition = CardDefinition {
    id: NECRONOMICURSE_ID,
    key: "Necronomicurse",
    name: "Necronomicurse",
    cost: 0,
    card_type: CardType::Status,
    rarity: None,
    upgrade: None,
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
        rarity: None,
        upgrade: None,
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
    rarity: None,
    upgrade: None,
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

pub const VOID: CardDefinition = CardDefinition {
    id: VOID_ID,
    key: "Void",
    name: "Void",
    cost: 0,
    card_type: CardType::Status,
    rarity: None,
    upgrade: None,
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

pub const ETHEREAL_STRIKE: CardDefinition = CardDefinition {
    id: ETHEREAL_STRIKE_ID,
    key: "Ethereal_Strike",
    name: "Ethereal Strike",
    cost: 1,
    card_type: CardType::Attack,
    rarity: None,
    upgrade: None,
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
    rarity: None,
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(ANGER_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(CLEAVE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(TWIN_STRIKE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(SHRUG_IT_OFF_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(TRUE_GRIT_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BURNING_PACT_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(FEEL_NO_PAIN_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DARK_EMBRACE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(COMBUST_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(DEMON_FORM_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
    target: TargetRequirement::None,
    values: DEMON_FORM.values,
    keywords: CARD_KEYWORDS_NONE,
};

pub const EVOLVE: CardDefinition = CardDefinition {
    id: EVOLVE_ID,
    key: "EVOLVE",
    name: "Evolve",
    cost: 1,
    card_type: CardType::Power,
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(EVOLVE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(CORRUPTION_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(BARRICADE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(BERSERK_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(RUPTURE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(JUGGERNAUT_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(BRUTALITY_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(MAYHEM_PLUS_ID),
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const MAYHEM_PLUS: CardDefinition = CardDefinition {
    id: MAYHEM_PLUS_ID,
    key: "MAYHEM+",
    name: "Mayhem+",
    cost: 1,
    card_type: CardType::Power,
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(DOUBLE_TAP_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(FIRE_BREATHING_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(LIMIT_BREAK_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(OFFERING_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(ARMAMENTS_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(HEADBUTT_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(POMMEL_STRIKE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BATTLE_TRANCE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SEEING_RED_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(INFLAME_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(FLEX_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SPOT_WEAKNESS_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(WHIRLWIND_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(HAVOC_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(WARCRY_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DUAL_WIELD_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SEARING_BLOW_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SEARING_BLOW_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DRAMATIC_ENTRANCE_PLUS_ID),
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

pub const DRAMATIC_ENTRANCE_PLUS: CardDefinition = CardDefinition {
    id: DRAMATIC_ENTRANCE_PLUS_ID,
    key: "Dramatic Entrance+",
    name: "Dramatic Entrance+",
    cost: 0,
    card_type: CardType::Attack,
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(12),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BANDAGE_UP_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(APOTHEOSIS_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BLIND_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(ENLIGHTENMENT_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SWIFT_STRIKE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BITE_PLUS_ID),
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BITE_PLUS: CardDefinition = CardDefinition {
    id: BITE_PLUS_ID,
    key: "Bite+",
    name: "Bite+",
    cost: 1,
    card_type: CardType::Attack,
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(8),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(APPARITION_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(JAX_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DEEP_BREATH_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DISCOVERY_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(FLASH_OF_STEEL_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(MIND_BLAST_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DARK_SHACKLES_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(FORETHOUGHT_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(GOOD_INSTINCTS_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(HAND_OF_GREED_PLUS_ID),
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(20),
        block: None,
        vulnerable: Some(20),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FINESSE: CardDefinition = CardDefinition {
    id: FINESSE_ID,
    key: "FINESSE",
    name: "Finesse",
    cost: 0,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(FINESSE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(25),
        block: None,
        vulnerable: Some(25),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FINESSE_PLUS: CardDefinition = CardDefinition {
    id: FINESSE_PLUS_ID,
    key: "FINESSE+",
    name: "Finesse+",
    cost: 0,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(PANACEA_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(PANACHE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(PURITY_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(MADNESS_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(MASTER_OF_STRATEGY_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(SECRET_TECHNIQUE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(SECRET_WEAPON_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

/// Enter Divinity (3× attack damage) and die at end of turn. Exhaust.
pub const BLASPHEMY: CardDefinition = CardDefinition {
    id: BLASPHEMY_ID,
    key: "BLASPHEMY",
    name: "Blasphemy",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Rare),
    upgrade: Some(BLASPHEMY_PLUS_ID),
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

pub const BLASPHEMY_PLUS: CardDefinition = CardDefinition {
    id: BLASPHEMY_PLUS_ID,
    key: "BLASPHEMY+",
    name: "Blasphemy+",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Rare),
    upgrade: None,
    target: TargetRequirement::None,
    values: BLASPHEMY.values,
    keywords: CardKeywords {
        innate: false,
        ethereal: false,
        exhaust: true,
        retain: true,
        unplayable: false,
    },
};

pub const VIOLENCE: CardDefinition = CardDefinition {
    id: VIOLENCE_ID,
    key: "VIOLENCE",
    name: "Violence",
    cost: 0,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Rare),
    upgrade: Some(VIOLENCE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(THE_BOMB_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(THINKING_AHEAD_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(TRANSMUTATION_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(METAMORPHOSIS_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(JACK_OF_ALL_TRADES_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(IMPATIENCE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(CHRYSALIS_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(SADISTIC_NATURE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(PANIC_BUTTON_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(MAGNETISM_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(TRIP_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(IRON_WAVE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(BODY_SLAM_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(CLASH_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(WILD_STRIKE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(HEAVY_BLADE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(PERFECTED_STRIKE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(RAMPAGE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(POWER_THROUGH_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(INFERNAL_BLADE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(ENTRENCH_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(GHOSTLY_ARMOR_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(FLAME_BARRIER_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(RECKLESS_CHARGE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(PUMMEL_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(CLOTHESLINE_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(INTIMIDATE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SHOCKWAVE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DISARM_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(RAGE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SEVER_SOUL_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SECOND_WIND_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(SENTINEL_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BLOODLETTING_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(CARNAGE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(DROPKICK_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(SWORD_BOOMERANG_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(HEMOKINESIS_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(BLOOD_FOR_BLOOD_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(IMMOLATE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(BLUDGEON_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(FEED_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(IMPERVIOUS_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(FIEND_FIRE_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(REAPER_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Rare),
    upgrade: Some(EXHUME_PLUS_ID),
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
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(METALLICIZE_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
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
    rarity: Some(CardRarity::Common),
    upgrade: Some(THUNDERCLAP_PLUS_ID),
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
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: Some(UPPERCUT_PLUS_ID),
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
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(13),
        block: None,
        vulnerable: Some(2),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const IRONCLAD_STARTER_CARDS: [CardDefinition; 3] = [STRIKE_R, DEFEND_R, BASH];
pub const STATUS_CARDS: [CardDefinition; 6] = [WOUND, DAZED, BURN, SLIMED, VOID, ASCENDERS_BANE];
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
pub static ALL_CARDS: [CardDefinition; 249] = [
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
    VOID,
    REGRET,
    DOUBT,
    CURSE_OF_THE_BELL,
    NECRONOMICURSE,
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
    MAYHEM_PLUS,
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
    DRAMATIC_ENTRANCE_PLUS,
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
    BITE_PLUS,
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
    BLASPHEMY,
    BLASPHEMY_PLUS,
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
pub fn is_synthetic_any_color_content_id(id: ContentId) -> bool {
    matches!(
        id,
        CHARGE_BATTERY_ANY_COLOR_ID
            | EMPTY_BODY_ANY_COLOR_ID
            | JUST_LUCKY_ANY_COLOR_ID
            | GO_FOR_THE_EYES_ANY_COLOR_ID
            | EQUILIBRIUM_ANY_COLOR_ID
            | SNEAKY_STRIKE_ANY_COLOR_ID
            | PROSTRATE_ANY_COLOR_ID
            | CLOAK_AND_DAGGER_ANY_COLOR_ID
            | SHIV_ANY_COLOR_ID
            | BACKFLIP_ANY_COLOR_ID
            | LESSON_LEARNED_ANY_COLOR_ID
            | RECYCLE_ANY_COLOR_ID
            | BIASED_COGNITION_ANY_COLOR_ID
            | PRESSURE_POINTS_ANY_COLOR_ID
            | EMPTY_MIND_ANY_COLOR_ID
            | TRANQUILITY_ANY_COLOR_ID
            | SKIM_ANY_COLOR_ID
            | DOPPELGANGER_ANY_COLOR_ID
    ) || (get_card_definition(id).is_none()
        && crate::run::reward::any_color_reward_card_key(id).is_some())
}

pub static CHARGE_BATTERY_ANY_COLOR: CardDefinition = CardDefinition {
    id: CHARGE_BATTERY_ANY_COLOR_ID,
    key: "CHARGE_BATTERY",
    name: "Charge Battery",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(7),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static SKIM_ANY_COLOR: CardDefinition = CardDefinition {
    id: SKIM_ANY_COLOR_ID,
    key: "SKIM",
    name: "Skim",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
        unplayable: false,
    },
};

pub static DOPPELGANGER_ANY_COLOR: CardDefinition = CardDefinition {
    id: DOPPELGANGER_ANY_COLOR_ID,
    key: "DOPPELGANGER",
    name: "Doppelganger",
    cost: -1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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

pub static TRANQUILITY_ANY_COLOR: CardDefinition = CardDefinition {
    id: TRANQUILITY_ANY_COLOR_ID,
    key: "TRANQUILITY",
    name: "Tranquility",
    cost: 0,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
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
        retain: true,
        unplayable: false,
    },
};

pub static EMPTY_MIND_ANY_COLOR: CardDefinition = CardDefinition {
    id: EMPTY_MIND_ANY_COLOR_ID,
    key: "EMPTY_MIND",
    name: "Empty Mind",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static PRESSURE_POINTS_ANY_COLOR: CardDefinition = CardDefinition {
    id: PRESSURE_POINTS_ANY_COLOR_ID,
    key: "PRESSURE_POINTS",
    name: "Pressure Points",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static BIASED_COGNITION_ANY_COLOR: CardDefinition = CardDefinition {
    id: BIASED_COGNITION_ANY_COLOR_ID,
    key: "BIASED_COGNITION",
    name: "Biased Cognition",
    cost: 1,
    card_type: CardType::Power,
    rarity: Some(CardRarity::Rare),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static RECYCLE_ANY_COLOR: CardDefinition = CardDefinition {
    id: RECYCLE_ANY_COLOR_ID,
    key: "RECYCLE",
    name: "Recycle",
    cost: 0,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static LESSON_LEARNED_ANY_COLOR: CardDefinition = CardDefinition {
    id: LESSON_LEARNED_ANY_COLOR_ID,
    key: "LESSON_LEARNED",
    name: "Lesson Learned",
    cost: 2,
    card_type: CardType::Attack,
    rarity: Some(CardRarity::Rare),
    upgrade: None,
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

pub static BACKFLIP_ANY_COLOR: CardDefinition = CardDefinition {
    id: BACKFLIP_ANY_COLOR_ID,
    key: "BACKFLIP",
    name: "Backflip",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static SHIV_ANY_COLOR: CardDefinition = CardDefinition {
    id: SHIV_ANY_COLOR_ID,
    key: "SHIV",
    name: "Shiv",
    cost: 0,
    card_type: CardType::Attack,
    rarity: None,
    upgrade: None,
    target: TargetRequirement::Enemy,
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

pub static CLOAK_AND_DAGGER_ANY_COLOR: CardDefinition = CardDefinition {
    id: CLOAK_AND_DAGGER_ANY_COLOR_ID,
    key: "CLOAK_AND_DAGGER",
    name: "Cloak And Dagger",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(6),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static PROSTRATE_ANY_COLOR: CardDefinition = CardDefinition {
    id: PROSTRATE_ANY_COLOR_ID,
    key: "PROSTRATE",
    name: "Prostrate",
    cost: 0,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(4),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static SNEAKY_STRIKE_ANY_COLOR: CardDefinition = CardDefinition {
    id: SNEAKY_STRIKE_ANY_COLOR_ID,
    key: "SNEAKY_STRIKE",
    name: "Sneaky Strike",
    cost: 2,
    card_type: CardType::Attack,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(16),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static EQUILIBRIUM_ANY_COLOR: CardDefinition = CardDefinition {
    id: EQUILIBRIUM_ANY_COLOR_ID,
    key: "EQUILIBRIUM",
    name: "Equilibrium",
    cost: 2,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Uncommon),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(13),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static GO_FOR_THE_EYES_ANY_COLOR: CardDefinition = CardDefinition {
    id: GO_FOR_THE_EYES_ANY_COLOR_ID,
    key: "GO_FOR_THE_EYES",
    name: "Go For The Eyes",
    cost: 0,
    card_type: CardType::Attack,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(3),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static JUST_LUCKY_ANY_COLOR: CardDefinition = CardDefinition {
    id: JUST_LUCKY_ANY_COLOR_ID,
    key: "JUST_LUCKY",
    name: "Just Lucky",
    cost: 0,
    card_type: CardType::Attack,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(3),
        block: Some(2),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub static EMPTY_BODY_ANY_COLOR: CardDefinition = CardDefinition {
    id: EMPTY_BODY_ANY_COLOR_ID,
    key: "EMPTY_BODY",
    name: "Empty Body",
    cost: 1,
    card_type: CardType::Skill,
    rarity: Some(CardRarity::Common),
    upgrade: None,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(7),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

#[must_use]
pub fn get_card_definition(id: ContentId) -> Option<&'static CardDefinition> {
    ALL_CARDS
        .iter()
        .find(|definition| definition.id == id)
        .or_else(|| (id == CHARGE_BATTERY_ANY_COLOR_ID).then_some(&CHARGE_BATTERY_ANY_COLOR))
        .or_else(|| (id == EQUILIBRIUM_ANY_COLOR_ID).then_some(&EQUILIBRIUM_ANY_COLOR))
        .or_else(|| (id == SNEAKY_STRIKE_ANY_COLOR_ID).then_some(&SNEAKY_STRIKE_ANY_COLOR))
        .or_else(|| (id == PROSTRATE_ANY_COLOR_ID).then_some(&PROSTRATE_ANY_COLOR))
        .or_else(|| (id == CLOAK_AND_DAGGER_ANY_COLOR_ID).then_some(&CLOAK_AND_DAGGER_ANY_COLOR))
        .or_else(|| (id == SHIV_ANY_COLOR_ID).then_some(&SHIV_ANY_COLOR))
        .or_else(|| (id == BACKFLIP_ANY_COLOR_ID).then_some(&BACKFLIP_ANY_COLOR))
        .or_else(|| (id == LESSON_LEARNED_ANY_COLOR_ID).then_some(&LESSON_LEARNED_ANY_COLOR))
        .or_else(|| (id == RECYCLE_ANY_COLOR_ID).then_some(&RECYCLE_ANY_COLOR))
        .or_else(|| (id == BIASED_COGNITION_ANY_COLOR_ID).then_some(&BIASED_COGNITION_ANY_COLOR))
        .or_else(|| (id == PRESSURE_POINTS_ANY_COLOR_ID).then_some(&PRESSURE_POINTS_ANY_COLOR))
        .or_else(|| (id == EMPTY_MIND_ANY_COLOR_ID).then_some(&EMPTY_MIND_ANY_COLOR))
        .or_else(|| (id == TRANQUILITY_ANY_COLOR_ID).then_some(&TRANQUILITY_ANY_COLOR))
        .or_else(|| (id == SKIM_ANY_COLOR_ID).then_some(&SKIM_ANY_COLOR))
        .or_else(|| (id == DOPPELGANGER_ANY_COLOR_ID).then_some(&DOPPELGANGER_ANY_COLOR))
        .or_else(|| (id == GO_FOR_THE_EYES_ANY_COLOR_ID).then_some(&GO_FOR_THE_EYES_ANY_COLOR))
        .or_else(|| (id == JUST_LUCKY_ANY_COLOR_ID).then_some(&JUST_LUCKY_ANY_COLOR))
        .or_else(|| (id == EMPTY_BODY_ANY_COLOR_ID).then_some(&EMPTY_BODY_ANY_COLOR))
}

/// Returns the vanilla `AbstractCard.cardID` spelling for a modeled card.
///
/// Match-and-Keep sends this source identifier for revealed cards. Ordinary
/// cards use the source-spelled card name in their durable definition metadata;
/// Panic Button is the source-specific exception (`PanicButton`, not its
/// display name `Panic Button`). Upgraded content IDs resolve to their base
/// identity because vanilla upgrades retain the same `cardID`.
#[must_use]
pub fn communication_mod_card_id(id: ContentId) -> Option<&'static str> {
    let base_id = base_content_id(id);
    let definition = get_card_definition(base_id)?;
    Some(match base_id {
        PANIC_BUTTON_ID => "PanicButton",
        _ => definition.name,
    })
}

#[must_use]
pub fn is_curse_content_id(id: ContentId) -> bool {
    matches!(
        id,
        id if id == REGRET_ID
            || id == DOUBT_ID
            || id == CURSE_OF_THE_BELL_ID
            || id == NECRONOMICURSE_ID
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

/// Whether the target's `CardGroup.getPurgeableCards` includes this card.
/// That helper excludes only Necronomicurse, Curse of the Bell, and
/// Ascender's Bane. Event/shop remove screens apply a later bottled-card
/// filter; Empty Cage does not.
#[must_use]
pub fn is_purgeable_card_content(content_id: ContentId) -> bool {
    !matches!(
        content_id,
        ASCENDERS_BANE_ID | CURSE_OF_THE_BELL_ID | NECRONOMICURSE_ID
    )
}

/// Event/shop remove eligibility: `getPurgeableCards` plus the later bottled
/// filter those screens apply.
#[must_use]
pub fn is_purgeable_card(card: &CardInstance) -> bool {
    !card.bottled && is_purgeable_card_content(card.content_id)
}

#[must_use]
pub fn is_basic_starter_card(id: ContentId) -> bool {
    matches!(
        id,
        id if id == STRIKE_R_ID
            || id == STRIKE_R_PLUS_ID
            || id == DEFEND_R_ID
            || id == DEFEND_R_PLUS_ID
            || id == BASH_ID
            || id == BASH_PLUS_ID
    )
}

#[must_use]
pub fn is_pandoras_box_removed_starter(id: ContentId) -> bool {
    // Target Pandora's Box removes every Strike/Defend including upgrades
    // (cardID Strike_R / Defend_R with upgradeLevel). DEFEND_R+ was missing
    // (FIDL00240/244 boss relic grid size/order).
    matches!(
        id,
        id if id == STRIKE_R_ID
            || id == STRIKE_R_PLUS_ID
            || id == DEFEND_R_ID
            || id == DEFEND_R_PLUS_ID
    )
}

#[must_use]
pub fn card_type_and_rarity(id: ContentId) -> Option<(CardType, CardRarity)> {
    let definition = get_card_definition(id)?;
    Some((definition.card_type, definition.rarity?))
}

/// Returns whether a card belongs to the requested `AbstractCard.CardRarity`
/// bucket used by Bronze Orb's Stasis action.
///
/// `CardDefinition::rarity` deliberately models reward rarity, which does not
/// represent status cards or colorless event cards with `SPECIAL` rarity.
#[must_use]
pub(crate) fn card_matches_stasis_rarity(id: ContentId, rarity: CardRarity) -> bool {
    match id {
        WOUND_ID | DAZED_ID | BURN_ID | SLIMED_ID | VOID_ID => rarity == CardRarity::Common,
        BITE_ID | BITE_PLUS_ID | RITUAL_DAGGER_ID | APPARITION_ID | APPARITION_PLUS_ID | JAX_ID
        | JAX_PLUS_ID => false,
        _ => card_type_and_rarity(id).is_some_and(|(_, card_rarity)| card_rarity == rarity),
    }
}

/// Maps a base card content id to its upgraded (+) version, if one exists.
#[must_use]
pub fn upgrade_content_id(id: ContentId) -> Option<ContentId> {
    get_card_definition(id)?.upgrade
}

/// Maps an upgraded card content id back to its base form.
///
/// STS `transformCard` keys exclusion off `cardID` (shared by base and +).
/// Our upgraded cards use distinct `ContentId`s, so transforms must normalize
/// before excluding the source from the pool (FIDL00263 Astrolabe Thunderclap+).
#[must_use]
pub fn base_content_id(id: ContentId) -> ContentId {
    for definition in &ALL_CARDS {
        if definition.upgrade == Some(id) {
            return definition.id;
        }
    }
    id
}

pub(crate) fn required_upgrade_content_id(id: ContentId) -> SimResult<ContentId> {
    get_card_definition(id)
        .ok_or(SimError::UnknownContent(id))?
        .upgrade
        .ok_or(SimError::UnsupportedMechanic(id))
}

pub fn searing_blow_damage_for_upgrades(upgrades: u8) -> SimResult<i32> {
    let upgrades = i32::from(upgrades);
    let base_damage = SEARING_BLOW.values.damage.ok_or(SimError::InvalidState(
        "Searing Blow definition is missing damage",
    ))?;
    let triangular_bonus = upgrades
        .checked_mul(
            upgrades
                .checked_add(7)
                .ok_or(SimError::InvalidState("Searing Blow damage overflows i32"))?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(SimError::InvalidState("Searing Blow damage overflows i32"))?;
    base_damage
        .checked_add(triangular_bonus)
        .ok_or(SimError::InvalidState("Searing Blow damage overflows i32"))
}

pub fn searing_blow_card_damage(card: &CardInstance) -> SimResult<Option<i32>> {
    validate_searing_blow_metadata(card)?;
    match card.content_id {
        SEARING_BLOW_ID | SEARING_BLOW_PLUS_ID => Ok(Some(searing_blow_damage_for_upgrades(
            card.searing_blow_upgrades,
        )?)),
        _ => Ok(None),
    }
}

pub(crate) fn validate_searing_blow_metadata(card: &CardInstance) -> SimResult<()> {
    match card.content_id {
        SEARING_BLOW_ID if card.searing_blow_upgrades != 0 => Err(SimError::InvalidState(
            "base Searing Blow carries upgrade-count metadata",
        )),
        SEARING_BLOW_PLUS_ID if card.searing_blow_upgrades == 0 => Err(SimError::InvalidState(
            "Searing Blow+ is missing its upgrade count",
        )),
        SEARING_BLOW_ID | SEARING_BLOW_PLUS_ID => Ok(()),
        _ if card.searing_blow_upgrades != 0 => Err(SimError::InvalidState(
            "non-Searing-Blow card carries Searing Blow upgrade metadata",
        )),
        _ => Ok(()),
    }
}

pub fn ritual_dagger_card_damage(card: &CardInstance) -> SimResult<Option<i32>> {
    if card.content_id == RITUAL_DAGGER_ID {
        let base_damage = RITUAL_DAGGER.values.damage.ok_or(SimError::InvalidState(
            "Ritual Dagger definition is missing damage",
        ))?;
        Ok(Some(
            base_damage
                .checked_add(card.ritual_dagger_damage_bonus)
                .ok_or(SimError::InvalidState("Ritual Dagger damage overflows i32"))?,
        ))
    } else {
        Ok(None)
    }
}

#[must_use]
pub fn ritual_dagger_card_growth(card: &CardInstance) -> Option<i32> {
    if card.content_id == RITUAL_DAGGER_ID {
        Some(if card.upgrades > 0 { 5 } else { 3 })
    } else {
        None
    }
}

pub fn upgrade_card_instance(card: CardInstance) -> SimResult<Option<CardInstance>> {
    validate_searing_blow_metadata(&card)?;
    if card.content_id == RITUAL_DAGGER_ID && card.upgrades == 0 {
        let mut upgraded = card;
        upgraded.upgrades = 1;
        return Ok(Some(upgraded));
    }

    let Some(upgraded_content_id) = upgrade_content_id(card.content_id) else {
        // Prismatic / any-color pool cards use synthetic content ids without a
        // separate upgraded ContentId. Track the upgrade on the instance so
        // reward projection can emit the CommMod `name+` form (e.g. flying knee+).
        if (get_card_definition(card.content_id).is_none()
            || is_synthetic_any_color_content_id(card.content_id))
            && card.upgrades == 0
        {
            let mut upgraded = card;
            upgraded.upgrades = 1;
            return Ok(Some(upgraded));
        }
        return Ok(None);
    };
    let mut upgraded = card;
    upgraded.content_id = upgraded_content_id;
    if matches!(card.content_id, SEARING_BLOW_ID | SEARING_BLOW_PLUS_ID) {
        upgraded.searing_blow_upgrades =
            card.searing_blow_upgrades
                .checked_add(1)
                .ok_or(SimError::InvalidState(
                    "Searing Blow upgrade count overflows u8",
                ))?;
    }
    adjust_temp_cost_for_upgrade(card, &mut upgraded);
    Ok(Some(upgraded))
}

pub(crate) fn card_instance_after_upgrades(
    mut card: CardInstance,
    upgrades: u8,
) -> SimResult<CardInstance> {
    validate_searing_blow_metadata(&card)?;
    for _ in 0..upgrades {
        card = upgrade_card_instance(card)?.ok_or(SimError::InvalidState(
            "card upgrade count exceeds its content upgrade path",
        ))?;
    }
    Ok(card)
}

fn adjust_temp_cost_for_upgrade(card: CardInstance, upgraded: &mut CardInstance) {
    let Some(cost_for_turn) = card.temp_cost else {
        return;
    };
    let (Some(base), Some(upgraded_base)) = (
        get_card_definition(card.content_id),
        get_card_definition(upgraded.content_id),
    ) else {
        return;
    };
    if base.cost == upgraded_base.cost || base.cost < 0 || upgraded_base.cost < 0 {
        return;
    }

    // ConfusionPower.setCostForTurn also writes AbstractCard.cost when the roll
    // differs from the current cost. upgradeBaseCost then uses
    // costForTurn - cost, not costForTurn - printedBase. After a Confusion 2 on
    // Havoc, both fields are 2, so Havoc+ stays 0 (FIDL01816).
    let adjusted = if cost_for_turn == 0 {
        0
    } else {
        i16::from(upgraded_base.cost).max(0) as u8
    };
    upgraded.temp_cost = Some(adjusted);
}

#[must_use]
pub fn card_instance_is_upgradeable(card: &CardInstance) -> bool {
    (card.content_id == RITUAL_DAGGER_ID && card.upgrades == 0)
        || upgrade_content_id(card.content_id).is_some()
        || (is_synthetic_any_color_content_id(card.content_id) && card.upgrades == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CardId;

    #[test]
    fn upgrade_content_id_covers_true_grit() {
        assert_eq!(upgrade_content_id(TRUE_GRIT_ID), Some(TRUE_GRIT_PLUS_ID));
    }

    #[test]
    fn communication_mod_card_id_preserves_source_spelling() {
        for (id, expected) in [
            (BANDAGE_UP_ID, "Bandage Up"),
            (SEVER_SOUL_ID, "Sever Soul"),
            (SPOT_WEAKNESS_ID, "Spot Weakness"),
            (PERFECTED_STRIKE_ID, "Perfected Strike"),
            (DRAMATIC_ENTRANCE_ID, "Dramatic Entrance"),
            (MIND_BLAST_ID, "Mind Blast"),
        ] {
            assert_eq!(communication_mod_card_id(id), Some(expected));
        }
        assert_eq!(
            communication_mod_card_id(PANIC_BUTTON_ID),
            Some("PanicButton")
        );
        assert_eq!(
            communication_mod_card_id(PANIC_BUTTON_PLUS_ID),
            Some("PanicButton")
        );
    }

    #[test]
    fn pandoras_box_removes_upgraded_strikes_and_defends() {
        assert!(is_pandoras_box_removed_starter(STRIKE_R_ID));
        assert!(is_pandoras_box_removed_starter(STRIKE_R_PLUS_ID));
        assert!(is_pandoras_box_removed_starter(DEFEND_R_ID));
        assert!(is_pandoras_box_removed_starter(DEFEND_R_PLUS_ID));
        assert!(!is_pandoras_box_removed_starter(BASH_ID));
        assert!(!is_pandoras_box_removed_starter(BASH_PLUS_ID));
    }

    #[test]
    fn upgrade_card_instance_tracks_synthetic_pool_card_upgrades() {
        // Prismatic shard any-color cards use stable synthetic content ids
        // without a CardDefinition / upgrade ContentId pair.
        let synthetic = crate::content::shop_pool::shop_card_content_id("FLYING_KNEE");
        assert!(get_card_definition(synthetic).is_none());
        let base = CardInstance::new(CardId::new(1), synthetic);
        let upgraded = upgrade_card_instance(base)
            .expect("upgrade is fallible only for invalid searing-blow metadata")
            .expect("synthetic pool cards are upgradeable");
        assert_eq!(upgraded.content_id, synthetic);
        assert_eq!(upgraded.upgrades, 1);
        assert!(card_instance_is_upgradeable(&base));
        assert!(!card_instance_is_upgradeable(&upgraded));
    }

    #[test]
    fn required_upgrade_content_distinguishes_unknown_and_unsupported_content() {
        let unknown = ContentId::new(999_999);
        assert_eq!(
            required_upgrade_content_id(unknown),
            Err(SimError::UnknownContent(unknown))
        );
        assert_eq!(
            required_upgrade_content_id(STRIKE_R_PLUS_ID),
            Err(SimError::UnsupportedMechanic(STRIKE_R_PLUS_ID))
        );
        assert_eq!(
            required_upgrade_content_id(STRIKE_R_ID),
            Ok(STRIKE_R_PLUS_ID)
        );
    }

    #[test]
    fn upgraded_starter_cards_remain_basic_for_relic_spawn_checks() {
        for id in [
            STRIKE_R_ID,
            STRIKE_R_PLUS_ID,
            DEFEND_R_ID,
            DEFEND_R_PLUS_ID,
            BASH_ID,
            BASH_PLUS_ID,
        ] {
            assert!(is_basic_starter_card(id), "{id:?} should remain basic");
        }
    }

    #[test]
    fn canonical_card_metadata_is_complete_and_self_consistent() {
        let mut common = 0;
        let mut uncommon = 0;
        let mut rare = 0;
        let mut unrated = 0;
        let mut upgrades = 0;

        for definition in &ALL_CARDS {
            match definition.rarity {
                Some(CardRarity::Common) => common += 1,
                Some(CardRarity::Uncommon) => uncommon += 1,
                Some(CardRarity::Rare) => rare += 1,
                None => unrated += 1,
            }
            assert_eq!(
                card_type_and_rarity(definition.id),
                definition
                    .rarity
                    .map(|rarity| (definition.card_type, rarity))
            );

            let Some(upgraded_id) = definition.upgrade else {
                continue;
            };
            upgrades += 1;
            let upgraded = get_card_definition(upgraded_id)
                .expect("every canonical upgrade must have a definition");
            assert_eq!(upgraded.rarity, definition.rarity);
            assert_eq!(upgraded.card_type, definition.card_type);
            assert!(
                upgraded.upgrade.is_none()
                    || (upgraded.id == SEARING_BLOW_PLUS_ID
                        && upgraded.upgrade == Some(SEARING_BLOW_PLUS_ID))
            );
            assert_eq!(upgrade_content_id(definition.id), Some(upgraded_id));
        }

        assert_eq!((common, uncommon, rare, unrated), (46, 114, 69, 20));
        assert_eq!(upgrades, 115);
    }

    #[test]
    fn armaments_keeps_confused_havoc_plus_at_zero() {
        let mut havoc = CardInstance::new(CardId::new(1), HAVOC_ID);
        havoc.temp_cost = Some(2);
        let upgraded = upgrade_card_instance(havoc)
            .expect("Havoc upgrades")
            .expect("Havoc is upgradeable");
        assert_eq!(upgraded.content_id, HAVOC_PLUS_ID);
        assert_eq!(upgraded.temp_cost, Some(0));
    }

    #[test]
    fn upgraded_cards_inherit_rarity_from_their_base_definition() {
        for (base, upgraded) in [
            (STRIKE_R_ID, STRIKE_R_PLUS_ID),
            (ANGER_ID, ANGER_PLUS_ID),
            (CLEAVE_ID, CLEAVE_PLUS_ID),
            (TWIN_STRIKE_ID, TWIN_STRIKE_PLUS_ID),
        ] {
            let base = get_card_definition(base).expect("base definition");
            let upgraded = get_card_definition(upgraded).expect("upgraded definition");
            assert_eq!(upgraded.rarity, base.rarity);
            assert_eq!(
                card_type_and_rarity(upgraded.id).map(|(_, rarity)| rarity),
                base.rarity
            );
        }
    }

    #[test]
    fn searing_blow_max_upgrade_is_playable_but_cannot_upgrade_again() {
        let mut card = CardInstance::new(CardId::new(1), SEARING_BLOW_PLUS_ID);
        card.searing_blow_upgrades = u8::MAX;

        assert!(card_instance_is_upgradeable(&card));
        assert_eq!(
            searing_blow_card_damage(&card),
            Ok(Some(searing_blow_damage_for_upgrades(u8::MAX).unwrap()))
        );
        assert_eq!(
            upgrade_card_instance(card),
            Err(SimError::InvalidState(
                "Searing Blow upgrade count overflows u8"
            ))
        );
    }
}
