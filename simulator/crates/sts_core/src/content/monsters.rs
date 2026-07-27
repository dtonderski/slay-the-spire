use crate::{
    card::{CardInstance, CardRarity},
    combat::piles::{
        add_cards_to_discard, add_cards_to_draw_random_spot,
        upgrade_burns_and_add_upgraded_to_discard,
    },
    combat::{CardPiles, MonsterIntent, MonsterState, SlimeSize},
    content::ascension::AscensionConfig,
    content::cards::{
        card_matches_stasis_rarity, get_card_definition, BURN_ID, DAZED_ID, SLIMED_ID, VOID_ID,
    },
    ids::{ContentId, MonsterId},
    power::MonsterPowers,
    rng::{seed_for_floor, StsRng},
    SimError, SimResult,
};

pub const FIXED_SIMPLE_MONSTER_ID: ContentId = ContentId::new(100);
pub const CULTIST_ID: ContentId = ContentId::new(101);
pub const JAW_WORM_ID: ContentId = ContentId::new(102);
pub const GREMLIN_NOB_ID: ContentId = ContentId::new(103);
pub const RED_LOUSE_ID: ContentId = ContentId::new(104);
pub const GREEN_LOUSE_ID: ContentId = ContentId::new(105);
pub const SPIKE_SLIME_ID: ContentId = ContentId::new(106);
pub const ACID_SLIME_ID: ContentId = ContentId::new(107);
pub const LAGAVULIN_ID: ContentId = ContentId::new(108);
pub const SENTRY_ID: ContentId = ContentId::new(109);
pub const HEXAGHOST_ID: ContentId = ContentId::new(110);
pub const SLIME_BOSS_ID: ContentId = ContentId::new(111);
pub const GUARDIAN_ID: ContentId = ContentId::new(112);
pub const LOOTER_ID: ContentId = ContentId::new(113);
pub const SPHERIC_GUARDIAN_ID: ContentId = ContentId::new(114);
pub const MUGGER_ID: ContentId = ContentId::new(115);
pub const CHOSEN_ID: ContentId = ContentId::new(116);
pub const SNAKE_PLANT_ID: ContentId = ContentId::new(117);
pub const SNECKO_ID: ContentId = ContentId::new(118);
pub const CENTURION_ID: ContentId = ContentId::new(119);
pub const HEALER_ID: ContentId = ContentId::new(120);
pub const BYRD_ID: ContentId = ContentId::new(121);
pub const SHELLED_PARASITE_ID: ContentId = ContentId::new(122);
pub const BOOK_OF_STABBING_ID: ContentId = ContentId::new(123);
pub const TASKMASTER_ID: ContentId = ContentId::new(124);
pub const GREMLIN_LEADER_ID: ContentId = ContentId::new(125);
pub const FUNGI_BEAST_ID: ContentId = ContentId::new(126);
pub const SLAVER_BLUE_ID: ContentId = ContentId::new(127);
pub const SLAVER_RED_ID: ContentId = ContentId::new(128);
pub const GREMLIN_WARRIOR_ID: ContentId = ContentId::new(129);
pub const GREMLIN_THIEF_ID: ContentId = ContentId::new(130);
pub const GREMLIN_FAT_ID: ContentId = ContentId::new(131);
pub const GREMLIN_TSUNDERE_ID: ContentId = ContentId::new(132);
pub const GREMLIN_WIZARD_ID: ContentId = ContentId::new(133);
pub const BRONZE_AUTOMATON_ID: ContentId = ContentId::new(134);
pub const BRONZE_ORB_ID: ContentId = ContentId::new(135);
pub const ORB_WALKER_ID: ContentId = ContentId::new(136);
pub const DARKLING_ID: ContentId = ContentId::new(137);
pub const THE_COLLECTOR_ID: ContentId = ContentId::new(138);
pub const TORCH_HEAD_ID: ContentId = ContentId::new(139);
pub const EXPLODER_ID: ContentId = ContentId::new(140);
pub const SPIKER_ID: ContentId = ContentId::new(141);
pub const REPULSOR_ID: ContentId = ContentId::new(142);
pub const TRANSIENT_ID: ContentId = ContentId::new(143);
pub const BANDIT_BEAR_ID: ContentId = ContentId::new(144);
pub const BANDIT_POINTY_ID: ContentId = ContentId::new(145);
pub const BANDIT_LEADER_ID: ContentId = ContentId::new(146);
pub const CHAMP_ID: ContentId = ContentId::new(147);
pub const AWAKENED_ONE_ID: ContentId = ContentId::new(148);
pub const DAGGER_ID: ContentId = ContentId::new(149);
pub const DECA_ID: ContentId = ContentId::new(150);
pub const DONU_ID: ContentId = ContentId::new(151);
pub const GIANT_HEAD_ID: ContentId = ContentId::new(152);
pub const NEMESIS_ID: ContentId = ContentId::new(153);
pub const REPTOMANCER_ID: ContentId = ContentId::new(154);
pub const SPIRE_GROWTH_ID: ContentId = ContentId::new(155);
pub const MAW_ID: ContentId = ContentId::new(156);
pub const TIME_EATER_ID: ContentId = ContentId::new(157);
pub const WRITHING_MASS_ID: ContentId = ContentId::new(158);
pub const CORRUPT_HEART_ID: ContentId = ContentId::new(159);
pub const SPIRE_SHIELD_ID: ContentId = ContentId::new(160);
pub const SPIRE_SPEAR_ID: ContentId = ContentId::new(161);

pub(crate) const RED_LOUSE_BITE_DAMAGE: i32 = 6;
pub(crate) const LOUSE_CURL_STRENGTH: i32 = 3;

pub const GREEN_LOUSE_WEAK: i32 = 2;
const GREEN_LOUSE_SPIKES: i32 = 3;

const SPIKE_SLIME_LICK_WEAK: i32 = 1;
const SPIKE_SLIME_L_FRAIL: i32 = 2;
const SPIKE_SLIME_L_A17_FRAIL: i32 = 3;
const SPIKE_SLIME_S_SPIT_DAMAGE: i32 = 5;
const SPIKE_SLIME_M_SPIT_DAMAGE: i32 = 8;
pub(crate) const SPIKE_SLIME_L_SPIT_DAMAGE: i32 = 16;

pub(crate) const ACID_SLIME_S_TACKLE_DAMAGE: i32 = 3;
pub(crate) const ACID_SLIME_ATTACK_DAMAGE: i32 = 7;
pub(crate) const ACID_SLIME_M_NORMAL_TACKLE_DAMAGE: i32 = 10;
const ACID_SLIME_L_WOUND_TACKLE_DAMAGE: i32 = 11;
const ACID_SLIME_L_A2_WOUND_TACKLE_DAMAGE: i32 = 12;
pub(crate) const ACID_SLIME_L_NORMAL_TACKLE_DAMAGE: i32 = 16;
const ACID_SLIME_L_A2_NORMAL_TACKLE_DAMAGE: i32 = 18;
const ACID_SLIME_WEAK: i32 = 1;

const LAGAVULIN_SLEEP_TURNS: u32 = 3;
const LAGAVULIN_SIPHON_STRENGTH: i32 = 1;
const LAGAVULIN_SIPHON_DEXTERITY: i32 = 1;
const LAGAVULIN_ATTACK_DAMAGE: i32 = 18;
const LAGAVULIN_A3_ATTACK_DAMAGE: i32 = 20;

const SENTRY_BEAM_DAZED: i32 = 2;
const SENTRY_ATTACK_DAMAGE: i32 = 9;
const SENTRY_A3_ATTACK_DAMAGE: i32 = 10;
const SENTRY_ARTIFACT: i32 = 1;

const SPHERIC_GUARDIAN_DAMAGE: i32 = 10;
const SPHERIC_GUARDIAN_A2_DAMAGE: i32 = 11;
const SPHERIC_GUARDIAN_STARTING_BLOCK: i32 = 40;
const SPHERIC_GUARDIAN_ARTIFACT: i32 = 3;
const BRONZE_AUTOMATON_ARTIFACT: i32 = 3;
pub const SPHERIC_GUARDIAN_ACTIVATE_BLOCK: i32 = 25;
const SPHERIC_GUARDIAN_A17_ACTIVATE_BLOCK: i32 = 35;
pub const SPHERIC_GUARDIAN_HARDEN_BLOCK: i32 = 15;
pub const SPHERIC_GUARDIAN_FRAIL: i32 = 5;
const SPHERIC_GUARDIAN_SLAM_HITS: i32 = 2;

const HEXAGHOST_DIVIDER_DAMAGE: i32 = 6;
const HEXAGHOST_DIVIDER_HITS: i32 = 2;
const HEXAGHOST_TACKLE_DAMAGE: i32 = 5;
const HEXAGHOST_TACKLE_HITS: i32 = 2;
const HEXAGHOST_SEAR_BURNS: i32 = 1;
const HEXAGHOST_STRENGTHEN_BLOCK: i32 = 12;
const HEXAGHOST_STRENGTHEN_STRENGTH: i32 = 2;
const HEXAGHOST_INFERNO_BURNS: i32 = 3;
const HEXAGHOST_INFERNO_DAMAGE: i32 = 2;

const SLIME_BOSS_SLAM_DAMAGE: i32 = 35;
pub const SLIME_BOSS_SLIMED_COUNT: i32 = 3;
pub const SLIME_BOSS_A19_SLIMED_COUNT: i32 = 5;
const SLIME_BOSS_SPLIT_HP_THRESHOLD: i32 = 70;

const GUARDIAN_MODE_SHIFT_START: i32 = 30;
const GUARDIAN_MODE_SHIFT_INCREASE: i32 = 10;
const GUARDIAN_DEFENSIVE_SEQUENCE_TURNS: u32 = 3;
/// Block granted when Mode Shift enters defensive mode (source `DEFENSIVE_BLOCK`).
pub const GUARDIAN_DEFENSIVE_BLOCK: i32 = 20;
const GUARDIAN_DEFENSIVE_SPIKES: i32 = 3;
pub const GUARDIAN_CHARGE_BLOCK: i32 = 9;
const GUARDIAN_FIERCE_BASH_DAMAGE: i32 = 32;
const GUARDIAN_A4_FIERCE_BASH_DAMAGE: i32 = 36;
const GUARDIAN_DEFENSIVE_ATTACK_DAMAGE: i32 = 9;
const GUARDIAN_A4_DEFENSIVE_ATTACK_DAMAGE: i32 = 10;
const GUARDIAN_DEFENSIVE_COMBO_DAMAGE: i32 = 8;
const GUARDIAN_WHIRLWIND_DAMAGE: i32 = 5;
const GUARDIAN_WHIRLWIND_HITS: i32 = 4;
const GUARDIAN_VENT_DEBUFF: i32 = 2;

const GREMLIN_NOB_RUSH_DAMAGE: i32 = 14;
const GREMLIN_NOB_A3_RUSH_DAMAGE: i32 = 16;
const GREMLIN_NOB_SKULL_BASH_DAMAGE: i32 = 6;
const GREMLIN_NOB_A3_SKULL_BASH_DAMAGE: i32 = 8;
pub const GREMLIN_NOB_A0_ENRAGE: i32 = 2;
pub const GREMLIN_NOB_A17_ENRAGE: i32 = 3;

const JAW_WORM_CHOMP_DAMAGE: i32 = 11;
const JAW_WORM_THRASH_DAMAGE: i32 = 7;
const JAW_WORM_THRASH_BLOCK: i32 = 5;
const JAW_WORM_BELLOW_STRENGTH: i32 = 3;
const JAW_WORM_BELLOW_BLOCK: i32 = 6;
const JAW_WORM_HORDE_STARTING_BLOCK: i32 = 6;
const JAW_WORM_HORDE_STARTING_STRENGTH: i32 = 3;

pub const LOOTER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(44, 48);
pub const LOOTER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(46, 50);
const LOOTER_SWIPE_DAMAGE: i32 = 10;
const LOOTER_A2_SWIPE_DAMAGE: i32 = 11;
const LOOTER_LUNGE_DAMAGE: i32 = 12;
const LOOTER_A2_LUNGE_DAMAGE: i32 = 14;
const LOOTER_SMOKE_BOMB_BLOCK: i32 = 6;
const LOOTER_THEFT: i32 = 15;
const LOOTER_A17_THEFT: i32 = 20;
const MUGGER_SWIPE_DAMAGE: i32 = 10;
const MUGGER_A2_SWIPE_DAMAGE: i32 = 11;
const MUGGER_BIG_SWIPE_DAMAGE: i32 = 16;
const MUGGER_A2_BIG_SWIPE_DAMAGE: i32 = 18;
const MUGGER_THEFT: i32 = 15;
const MUGGER_A17_THEFT: i32 = 20;
const MUGGER_SMOKE_BOMB_BLOCK: i32 = 11;
const MUGGER_A17_SMOKE_BOMB_BLOCK: i32 = 17;
const CHOSEN_POKE_DAMAGE: i32 = 5;
const CHOSEN_A2_POKE_DAMAGE: i32 = 6;
const CHOSEN_POKE_HITS: i32 = 2;
const CHOSEN_ZAP_DAMAGE: i32 = 18;
const CHOSEN_A2_ZAP_DAMAGE: i32 = 21;
const CHOSEN_DEBILITATE_DAMAGE: i32 = 10;
const CHOSEN_A2_DEBILITATE_DAMAGE: i32 = 12;
const CHOSEN_DEBILITATE_VULNERABLE: i32 = 2;
const CHOSEN_DRAIN_STRENGTH: i32 = 3;
const CHOSEN_DRAIN_WEAK: i32 = 3;
const CHOSEN_HEX: i32 = 1;
const SNAKE_PLANT_CHOMPY_DAMAGE: i32 = 7;
const SNAKE_PLANT_A2_CHOMPY_DAMAGE: i32 = 8;
const SNAKE_PLANT_CHOMPY_HITS: i32 = 3;
const SNAKE_PLANT_SPORES_DEBUFF: i32 = 2;
const SNAKE_PLANT_MALLEABLE: i32 = 3;
const SNECKO_BITE_DAMAGE: i32 = 15;
const SNECKO_TAIL_DAMAGE: i32 = 8;
const SNECKO_A2_BITE_DAMAGE: i32 = 18;
const SNECKO_A2_TAIL_DAMAGE: i32 = 10;
const SNECKO_VULNERABLE: i32 = 2;
const SNECKO_A17_WEAK: i32 = 2;
const CENTURION_SLASH_DAMAGE: i32 = 12;
const CENTURION_FURY_DAMAGE: i32 = 6;
const CENTURION_A2_SLASH_DAMAGE: i32 = 14;
const CENTURION_A2_FURY_DAMAGE: i32 = 7;
const CENTURION_FURY_HITS: i32 = 3;
const CENTURION_BLOCK: i32 = 15;
const CENTURION_A17_BLOCK: i32 = 20;
const HEALER_ATTACK_DAMAGE: i32 = 8;
const HEALER_A2_ATTACK_DAMAGE: i32 = 9;
const HEALER_FRAIL: i32 = 2;
const HEALER_HEAL: i32 = 16;
const HEALER_A17_HEAL: i32 = 20;
const HEALER_STRENGTH: i32 = 2;
const HEALER_A2_STRENGTH: i32 = 3;
const HEALER_A17_STRENGTH: i32 = 4;
const BYRD_PECK_DAMAGE: i32 = 1;
const BYRD_PECK_HITS: i32 = 5;
const BYRD_A2_PECK_HITS: i32 = 6;
const BYRD_SWOOP_DAMAGE: i32 = 12;
const BYRD_A2_SWOOP_DAMAGE: i32 = 14;
const BYRD_HEADBUTT_DAMAGE: i32 = 3;
const BYRD_CAW_STRENGTH: i32 = 1;
const BYRD_FLIGHT: i32 = 3;
const BYRD_A17_FLIGHT: i32 = 4;
const SHELLED_PARASITE_PLATED_ARMOR: i32 = 14;
const SHELLED_PARASITE_FELL_DAMAGE: i32 = 18;
const SHELLED_PARASITE_A2_FELL_DAMAGE: i32 = 21;
const SHELLED_PARASITE_FELL_FRAIL: i32 = 2;
const SHELLED_PARASITE_DOUBLE_STRIKE_DAMAGE: i32 = 6;
const SHELLED_PARASITE_A2_DOUBLE_STRIKE_DAMAGE: i32 = 7;
const SHELLED_PARASITE_DOUBLE_STRIKE_HITS: i32 = 2;
const SHELLED_PARASITE_SUCK_DAMAGE: i32 = 10;
const SHELLED_PARASITE_A2_SUCK_DAMAGE: i32 = 12;
const BOOK_OF_STABBING_STAB_DAMAGE: i32 = 6;
const BOOK_OF_STABBING_BIG_STAB_DAMAGE: i32 = 21;
const BOOK_OF_STABBING_A3_STAB_DAMAGE: i32 = 7;
const BOOK_OF_STABBING_A3_BIG_STAB_DAMAGE: i32 = 24;
const BOOK_OF_STABBING_PAINFUL_STABS: i32 = 1;
const TASKMASTER_SCOURING_WHIP_DAMAGE: i32 = 7;
const TASKMASTER_WOUNDS: i32 = 1;
const TASKMASTER_A3_WOUNDS: i32 = 2;
const TASKMASTER_A18_WOUNDS: i32 = 3;
const TASKMASTER_A18_STRENGTH: i32 = 1;
const GREMLIN_LEADER_STAB_DAMAGE: i32 = 6;
const GREMLIN_LEADER_STAB_HITS: i32 = 3;
const GREMLIN_LEADER_STRENGTH: i32 = 3;
const GREMLIN_LEADER_A3_STRENGTH: i32 = 4;
const GREMLIN_LEADER_A18_STRENGTH: i32 = 5;
const GREMLIN_LEADER_BLOCK: i32 = 6;
const GREMLIN_LEADER_A18_BLOCK: i32 = 10;
const FUNGI_BEAST_BITE_DAMAGE: i32 = 6;
const FUNGI_BEAST_GROW_STRENGTH: i32 = 3;
const FUNGI_BEAST_A2_GROW_STRENGTH: i32 = 4;
const FUNGI_BEAST_A17_GROW_BONUS: i32 = 1;
const FUNGI_BEAST_SPORE_CLOUD: i32 = 2;
const SLAVER_BLUE_STAB_DAMAGE: i32 = 12;
const SLAVER_BLUE_A2_STAB_DAMAGE: i32 = 13;
const SLAVER_BLUE_RAKE_DAMAGE: i32 = 7;
const SLAVER_BLUE_A2_RAKE_DAMAGE: i32 = 8;
const SLAVER_BLUE_WEAK: i32 = 1;
const SLAVER_BLUE_A17_WEAK: i32 = 2;
const SLAVER_RED_STAB_DAMAGE: i32 = 13;
const SLAVER_RED_A2_STAB_DAMAGE: i32 = 14;
const SLAVER_RED_SCRAPE_DAMAGE: i32 = 8;
const SLAVER_RED_A2_SCRAPE_DAMAGE: i32 = 9;
const SLAVER_RED_VULNERABLE: i32 = 1;
const SLAVER_RED_A17_VULNERABLE: i32 = 2;
const SLAVER_RED_ENTANGLED: i32 = 1;
const GREMLIN_WARRIOR_SCRATCH_DAMAGE: i32 = 4;
const GREMLIN_WARRIOR_A2_SCRATCH_DAMAGE: i32 = 5;
const GREMLIN_WARRIOR_ANGER: i32 = 1;
const GREMLIN_WARRIOR_A17_ANGER: i32 = 2;
const GREMLIN_THIEF_DAMAGE: i32 = 9;
const GREMLIN_THIEF_A2_DAMAGE: i32 = 10;
const GREMLIN_FAT_DAMAGE: i32 = 4;
const GREMLIN_FAT_A2_DAMAGE: i32 = 5;
const GREMLIN_FAT_WEAK: i32 = 1;
const GREMLIN_TSUNDERE_BLOCK: i32 = 7;
const GREMLIN_TSUNDERE_A7_BLOCK: i32 = 8;
const GREMLIN_TSUNDERE_A17_BLOCK: i32 = 11;
const GREMLIN_TSUNDERE_BASH_DAMAGE: i32 = 6;
const GREMLIN_TSUNDERE_A2_BASH_DAMAGE: i32 = 8;
const GREMLIN_WIZARD_MAGIC_DAMAGE: i32 = 25;
const GREMLIN_WIZARD_A2_MAGIC_DAMAGE: i32 = 30;
const BRONZE_AUTOMATON_FLAIL_DAMAGE: i32 = 7;
const BRONZE_AUTOMATON_A4_FLAIL_DAMAGE: i32 = 8;
const BRONZE_AUTOMATON_FLAIL_HITS: i32 = 2;
const BRONZE_AUTOMATON_HYPER_BEAM_DAMAGE: i32 = 45;
const BRONZE_AUTOMATON_A4_HYPER_BEAM_DAMAGE: i32 = 50;
const BRONZE_AUTOMATON_BOOST_BLOCK: i32 = 9;
const BRONZE_AUTOMATON_A9_BOOST_BLOCK: i32 = 12;
const BRONZE_AUTOMATON_BOOST_STRENGTH: i32 = 3;
const BRONZE_AUTOMATON_A4_BOOST_STRENGTH: i32 = 4;
const BRONZE_ORB_BEAM_DAMAGE: i32 = 8;
const BRONZE_ORB_SUPPORT_BEAM_BLOCK: i32 = 12;
const THE_COLLECTOR_BLOCK: i32 = 15;
const THE_COLLECTOR_STRENGTH: i32 = 3;
pub(crate) const TORCH_HEAD_ATTACK_DAMAGE: i32 = 7;
const ORB_WALKER_LASER_DAMAGE: i32 = 10;
const ORB_WALKER_A2_LASER_DAMAGE: i32 = 11;
const ORB_WALKER_CLAW_DAMAGE: i32 = 15;
const ORB_WALKER_A2_CLAW_DAMAGE: i32 = 16;
const ORB_WALKER_STRENGTH_UP: i32 = 3;
const DARKLING_CHOMP_DAMAGE: i32 = 8;
const DARKLING_BLOCK: i32 = 12;
const BANDIT_BEAR_MAUL_DAMAGE: i32 = 18;
const BANDIT_BEAR_A2_MAUL_DAMAGE: i32 = 20;
const BANDIT_BEAR_LUNGE_DAMAGE: i32 = 9;
const BANDIT_BEAR_A2_LUNGE_DAMAGE: i32 = 10;
const BANDIT_BEAR_LUNGE_BLOCK: i32 = 9;
const BANDIT_POINTY_DAMAGE: i32 = 5;
const BANDIT_POINTY_HITS: i32 = 2;
const BANDIT_LEADER_SLASH_DAMAGE: i32 = 15;
const BANDIT_LEADER_A2_SLASH_DAMAGE: i32 = 17;
const BANDIT_LEADER_AGONIZE_DAMAGE: i32 = 10;
const BANDIT_LEADER_A2_AGONIZE_DAMAGE: i32 = 12;
const BANDIT_LEADER_WEAK: i32 = 2;
const CHAMP_HEAVY_SLASH_DAMAGE: i32 = 16;
const CHAMP_A4_HEAVY_SLASH_DAMAGE: i32 = 18;
pub const CHAMP_FACE_SLAP_DAMAGE: i32 = 12;
pub const CHAMP_FACE_SLAP_FRAIL: i32 = 2;
const CHAMP_A4_FACE_SLAP_DAMAGE: i32 = 14;
const CHAMP_EXECUTE_DAMAGE: i32 = 10;
const CHAMP_EXECUTE_HITS: i32 = 2;
pub const CHAMP_DEFENSIVE_BLOCK: i32 = 15;
pub const CHAMP_DEFENSIVE_METALLICIZE: i32 = 5;
const COLLECTOR_FIREBALL_DAMAGE: i32 = 18;
const COLLECTOR_A4_FIREBALL_DAMAGE: i32 = 21;
const COLLECTOR_BUFF_BLOCK: i32 = 35;
const TORCH_HEAD_TACKLE_DAMAGE: i32 = 7;
const AWAKENED_ONE_SLASH_DAMAGE: i32 = 20;
const AWAKENED_ONE_SOUL_STRIKE_DAMAGE: i32 = 6;
const AWAKENED_ONE_SOUL_STRIKE_HITS: i32 = 4;
const AWAKENED_ONE_DARK_ECHO_DAMAGE: i32 = 40;
const AWAKENED_ONE_TACKLE_DAMAGE: i32 = 10;
const AWAKENED_ONE_TACKLE_HITS: i32 = 3;
const DAGGER_WOUND_DAMAGE: i32 = 9;
const DAGGER_EXPLODE_DAMAGE: i32 = 25;
const DECA_BEAM_DAMAGE: i32 = 10;
const DECA_A4_BEAM_DAMAGE: i32 = 12;
const DECA_BEAM_HITS: i32 = 2;
const DECA_PROTECTION_BLOCK: i32 = 16;
const DONU_BEAM_DAMAGE: i32 = 10;
const DONU_A4_BEAM_DAMAGE: i32 = 12;
const DONU_BEAM_HITS: i32 = 2;
const EXPLODER_ATTACK_DAMAGE: i32 = 9;
const EXPLODER_A2_ATTACK_DAMAGE: i32 = 11;
const EXPLODER_EXPLOSIVE: i32 = 3;
const EXPLODER_EXPLOSION_DAMAGE: i32 = 30;
const GIANT_HEAD_HP: i32 = 500;
const GIANT_HEAD_A8_HP: i32 = 520;
const GIANT_HEAD_DEATH_DAMAGE: i32 = 30;
const GIANT_HEAD_A3_DEATH_DAMAGE: i32 = 40;
const GIANT_HEAD_DAMAGE_INCREMENT: i32 = 5;
const GIANT_HEAD_COUNT_DAMAGE: i32 = 13;
const GIANT_HEAD_GLARE_WEAK: i32 = 1;
const NEMESIS_TRI_ATTACK_DAMAGE: i32 = 6;
const NEMESIS_A3_TRI_ATTACK_DAMAGE: i32 = 7;
const NEMESIS_TRI_ATTACK_HITS: i32 = 3;
const NEMESIS_SCYTHE_DAMAGE: i32 = 45;
const NEMESIS_HP: i32 = 185;
const NEMESIS_A8_HP: i32 = 200;
const NEMESIS_BURNS: i32 = 3;
const NEMESIS_A18_BURNS: i32 = 5;
const REPTOMANCER_SNAKE_STRIKE_DAMAGE: i32 = 13;
const REPTOMANCER_A3_SNAKE_STRIKE_DAMAGE: i32 = 16;
const REPTOMANCER_SNAKE_STRIKE_HITS: i32 = 2;
const REPTOMANCER_BIG_BITE_DAMAGE: i32 = 30;
const REPTOMANCER_A3_BIG_BITE_DAMAGE: i32 = 34;
const REPULSOR_ATTACK_DAMAGE: i32 = 11;
const REPULSOR_A2_ATTACK_DAMAGE: i32 = 13;
const REPULSOR_DAZES: i32 = 2;
const SPIKER_ATTACK_DAMAGE: i32 = 7;
const SPIKER_A2_ATTACK_DAMAGE: i32 = 9;
const SPIKER_THORNS: i32 = 3;
const SPIKER_A2_THORNS: i32 = 4;
const SPIKER_A17_THORNS_BONUS: i32 = 3;
const SPIKER_THORNS_BUFF: i32 = 2;
const SPIRE_GROWTH_QUICK_TACKLE_DAMAGE: i32 = 16;
const SPIRE_GROWTH_A2_QUICK_TACKLE_DAMAGE: i32 = 18;
const SPIRE_GROWTH_SMASH_DAMAGE: i32 = 22;
const SPIRE_GROWTH_A2_SMASH_DAMAGE: i32 = 25;
const SPIRE_GROWTH_HP: i32 = 170;
const SPIRE_GROWTH_A7_HP: i32 = 190;
const SPIRE_GROWTH_CONSTRICT: i32 = 10;
const SPIRE_GROWTH_A17_CONSTRICT: i32 = 12;
const MAW_SLAM_DAMAGE: i32 = 25;
const MAW_A2_SLAM_DAMAGE: i32 = 30;
const MAW_NOM_DAMAGE: i32 = 5;
const MAW_ROAR_DEBUFF: i32 = 3;
const MAW_A17_ROAR_DEBUFF: i32 = 5;
const MAW_STRENGTH: i32 = 3;
const MAW_A17_STRENGTH: i32 = 5;
const TIME_EATER_REVERBERATE_DAMAGE: i32 = 7;
const TIME_EATER_A4_REVERBERATE_DAMAGE: i32 = 8;
const TIME_EATER_HEAD_SLAM_DAMAGE: i32 = 26;
const TIME_EATER_A4_HEAD_SLAM_DAMAGE: i32 = 32;
const TIME_EATER_RIPPLE_BLOCK: i32 = 20;
const TRANSIENT_HP: i32 = 999;
const TRANSIENT_ATTACK_DAMAGE: i32 = 30;
const TRANSIENT_A4_ATTACK_DAMAGE: i32 = 40;
const TRANSIENT_ATTACK_DAMAGE_STEP: i32 = 10;
const WRITHING_MASS_BIG_HIT_DAMAGE: i32 = 32;
const WRITHING_MASS_A2_BIG_HIT_DAMAGE: i32 = 38;
const WRITHING_MASS_MULTI_HIT_DAMAGE: i32 = 7;
const WRITHING_MASS_A2_MULTI_HIT_DAMAGE: i32 = 9;
const WRITHING_MASS_MULTI_HIT_HITS: i32 = 3;
const WRITHING_MASS_ATTACK_BLOCK_DAMAGE: i32 = 15;
const WRITHING_MASS_A2_ATTACK_BLOCK_DAMAGE: i32 = 16;
const WRITHING_MASS_ATTACK_DEBUFF_DAMAGE: i32 = 10;
const WRITHING_MASS_A2_ATTACK_DEBUFF_DAMAGE: i32 = 12;
const WRITHING_MASS_MALLEABLE: i32 = 3;
const CORRUPT_HEART_ECHO_ATTACK_DAMAGE: i32 = 40;
const SPIRE_SHIELD_BASH_DAMAGE: i32 = 12;
const SPIRE_SPEAR_BURN_STRIKE_DAMAGE: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterDefinition {
    pub content_id: ContentId,
    pub name: &'static str,
    pub hp: i32,
    pub attack_damage: i32,
    pub ritual_amount: i32,
    /// Anger stacks granted to this monster when the player plays a skill (Gremlin Nob).
    pub enrage_weak_on_skill: i32,
    /// Spikes applied at combat start (thorns on attack).
    pub starting_spikes: i32,
    /// Turns spent asleep before acting (Lagavulin).
    pub starting_sleep_turns: u32,
    /// Turns spent in defensive mode before attacking (Guardian).
    pub starting_defensive_turns: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterHpRange {
    pub min: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetMonsterHp {
    pub name: &'static str,
    pub hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSpawnPower {
    pub id: &'static str,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetEncounterSpawn {
    pub name: &'static str,
    pub current_hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub intent: TargetSpawnIntent,
    pub powers: Vec<TargetSpawnPower>,
    pub rolled_attack_damage: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSpawnIntent {
    PendingAiRoll,
    Attack { damage: i32 },
    AttackAndBlock { damage: i32, block: i32 },
    StrengthAndBlock { strength: i32, block: i32 },
    ApplyPlayerFrailAndWeak { frail: i32, weak: i32 },
    AttackAddSlimedToDiscard { damage: i32, count: i32 },
    AddDazedToDraw { count: i32 },
}

impl TargetSpawnIntent {
    #[must_use]
    pub const fn trace_label(self) -> &'static str {
        match self {
            Self::PendingAiRoll => "DEBUG",
            Self::Attack { .. } => "Attack",
            Self::AttackAndBlock { .. } => "AttackAndBlock",
            Self::StrengthAndBlock { .. } => "StrengthAndBlock",
            Self::ApplyPlayerFrailAndWeak { .. } => "ApplyPlayerFrailAndWeak",
            Self::AttackAddSlimedToDiscard { .. } => "AttackAddSlimedToDiscard",
            Self::AddDazedToDraw { .. } => "AddDazedToDraw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetEncounterMember {
    pub monster_name: &'static str,
    pub x: Option<&'static str>,
    pub y: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetEncounterGroup {
    pub encounter_key: String,
    pub display_name: &'static str,
    pub members: Vec<TargetEncounterMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetMonsterConstant {
    pub name: &'static str,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCityMonsterProfile {
    pub monster_name: &'static str,
    pub hp_range: MonsterHpRange,
    pub constants: Vec<TargetMonsterConstant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmallSlimesVariant {
    SpikeSmallAcidMedium,
    AcidSmallSpikeMedium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LargeSlimeVariant {
    Acid,
    Spike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LouseKind {
    Normal,
    Defensive,
}

impl MonsterHpRange {
    #[must_use]
    pub const fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }

    #[must_use]
    pub const fn contains(self, hp: i32) -> bool {
        self.min <= hp && hp <= self.max
    }

    pub fn roll(self, rng: &mut StsRng) -> i32 {
        rng.random_int_range(self.min, self.max)
    }
}

pub const CULTIST_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(48, 54);
pub const CULTIST_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(50, 56);
pub const JAW_WORM_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(40, 44);
pub const JAW_WORM_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(42, 46);
pub const SPIKE_SLIME_S_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(10, 14);
pub const SPIKE_SLIME_S_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(11, 15);
pub const ACID_SLIME_S_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(8, 12);
pub const ACID_SLIME_S_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(9, 13);
pub const SPIKE_SLIME_M_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(28, 32);
pub const SPIKE_SLIME_M_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(29, 34);
pub const ACID_SLIME_M_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(28, 32);
pub const ACID_SLIME_M_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(29, 34);
pub const SPIKE_SLIME_L_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(64, 70);
pub const SPIKE_SLIME_L_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(67, 73);
pub const ACID_SLIME_L_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(65, 69);
pub const ACID_SLIME_L_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(68, 72);
pub const LOUSE_NORMAL_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(10, 15);
pub const LOUSE_NORMAL_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(11, 16);
pub const LOUSE_DEFENSIVE_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(11, 17);
pub const LOUSE_DEFENSIVE_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(12, 18);
pub const LOUSE_A0_BITE_DAMAGE_RANGE: MonsterHpRange = MonsterHpRange::new(5, 7);
pub const LOUSE_A2_BITE_DAMAGE_RANGE: MonsterHpRange = MonsterHpRange::new(6, 8);
pub const LOUSE_A0_CURL_UP_RANGE: MonsterHpRange = MonsterHpRange::new(3, 7);
pub const LOUSE_A7_CURL_UP_RANGE: MonsterHpRange = MonsterHpRange::new(4, 8);
pub const LAGAVULIN_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(109, 111);
pub const LAGAVULIN_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(112, 115);
pub const SENTRY_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(38, 42);
pub const SENTRY_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(39, 45);
pub const GREMLIN_NOB_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(82, 86);
pub const GREMLIN_NOB_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(85, 90);

pub const BYRD_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(25, 31);
pub const BYRD_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(26, 33);
pub const CHOSEN_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(95, 99);
pub const CHOSEN_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(98, 103);
pub const BANDIT_POINTY_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(30, 30);
pub const BANDIT_POINTY_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(34, 34);
pub const BANDIT_LEADER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(35, 39);
pub const BANDIT_LEADER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(37, 41);
pub const BANDIT_BEAR_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(38, 42);
pub const BANDIT_BEAR_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(40, 44);
pub const SHELLED_PARASITE_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(68, 72);
pub const SHELLED_PARASITE_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(70, 75);
pub const SPHERIC_GUARDIAN_HP_RANGE: MonsterHpRange = MonsterHpRange::new(20, 20);
pub const MUGGER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(48, 52);
pub const MUGGER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(50, 54);
pub const SNAKE_PLANT_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(75, 79);
pub const SNAKE_PLANT_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(78, 82);
pub const SNECKO_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(114, 120);
pub const SNECKO_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(120, 125);
pub const CENTURION_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(76, 80);
pub const CENTURION_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(78, 83);
pub const HEALER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(48, 56);
pub const HEALER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(50, 58);
pub const BOOK_OF_STABBING_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(160, 164);
pub const BOOK_OF_STABBING_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(168, 172);
pub const GREMLIN_LEADER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(140, 148);
pub const GREMLIN_LEADER_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(145, 155);
pub const TASKMASTER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(54, 60);
pub const TASKMASTER_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(57, 64);
pub const BRONZE_ORB_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(52, 58);
pub const BRONZE_ORB_A9_HP_RANGE: MonsterHpRange = MonsterHpRange::new(54, 60);
pub const FUNGI_BEAST_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(22, 28);
pub const FUNGI_BEAST_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(24, 28);
pub const SLAVER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(46, 50);
pub const SLAVER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(48, 52);
pub const GREMLIN_WARRIOR_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(20, 24);
pub const GREMLIN_WARRIOR_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(21, 25);
pub const GREMLIN_THIEF_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(10, 14);
pub const GREMLIN_THIEF_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(11, 15);
pub const GREMLIN_FAT_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(13, 17);
pub const GREMLIN_FAT_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(14, 18);
pub const GREMLIN_TSUNDERE_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(12, 15);
pub const GREMLIN_TSUNDERE_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(13, 17);
pub const GREMLIN_WIZARD_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(21, 25);
pub const GREMLIN_WIZARD_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(22, 26);
pub const DARKLING_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(48, 56);
pub const DARKLING_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(50, 59);
pub const DARKLING_A0_NIP_DAMAGE_RANGE: MonsterHpRange = MonsterHpRange::new(7, 11);
pub const DARKLING_A2_NIP_DAMAGE_RANGE: MonsterHpRange = MonsterHpRange::new(9, 13);
pub const DAGGER_HP_RANGE: MonsterHpRange = MonsterHpRange::new(20, 25);
pub const REPTOMANCER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(180, 190);
pub const REPTOMANCER_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(190, 200);
pub const TORCH_HEAD_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(38, 40);
pub const TORCH_HEAD_A9_HP_RANGE: MonsterHpRange = MonsterHpRange::new(40, 45);
pub const ORB_WALKER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(90, 96);
pub const ORB_WALKER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(92, 98);
pub const SPIKER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(42, 56);
pub const SPIKER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(44, 60);
pub const REPULSOR_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(29, 35);
pub const REPULSOR_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(31, 38);

pub const FIXED_SIMPLE_MONSTER: MonsterDefinition = MonsterDefinition {
    content_id: FIXED_SIMPLE_MONSTER_ID,
    name: "Fixed Simple Monster",
    hp: 40,
    attack_damage: 6,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Cultist at ascension 0: 50 HP, Ritual 3 on first turn, then 6-damage attacks.
pub const CULTIST_A0: MonsterDefinition = MonsterDefinition {
    content_id: CULTIST_ID,
    name: "Cultist",
    hp: 50,
    attack_damage: 6,
    ritual_amount: 3,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Jaw Worm at ascension 0, simplified: 42 HP (within 40–44), three-move cycle.
pub const JAW_WORM_A0: MonsterDefinition = MonsterDefinition {
    content_id: JAW_WORM_ID,
    name: "Jaw Worm",
    hp: 42,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Gremlin Nob at ascension 0, simplified: 82 HP, enrages on skill play, 6/14/10 attack cycle.
pub const GREMLIN_NOB_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_NOB_ID,
    name: "Gremlin Nob",
    hp: 82,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: GREMLIN_NOB_A0_ENRAGE,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Red Louse at ascension 0, simplified: 11 HP (within 11–12), Curl/Bite two-move cycle.
pub const RED_LOUSE_A0: MonsterDefinition = MonsterDefinition {
    content_id: RED_LOUSE_ID,
    name: "Red Louse",
    hp: 11,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Green Louse at ascension 0: 12 HP, Spikes 3, Curl/Bite cycle.
pub const GREEN_LOUSE_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREEN_LOUSE_ID,
    name: "Green Louse",
    hp: 12,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: GREEN_LOUSE_SPIKES,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Looter at ascension 0: 44-48 HP, opens with Mug for 10 damage and 15 gold theft.
pub const LOOTER_A0: MonsterDefinition = MonsterDefinition {
    content_id: LOOTER_ID,
    name: "Looter",
    hp: 46,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Mugger at ascension 0: 48-52 HP, opens with Mug for 10 damage and 15 gold theft.
pub const MUGGER_A0: MonsterDefinition = MonsterDefinition {
    content_id: MUGGER_ID,
    name: "Mugger",
    hp: 50,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Chosen at ascension 0: 95-99 HP, opens with double Poke then Hex.
pub const CHOSEN_A0: MonsterDefinition = MonsterDefinition {
    content_id: CHOSEN_ID,
    name: "Chosen",
    hp: 97,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Snake Plant at ascension 0: 75-79 HP, starts with Malleable 3.
pub const SNAKE_PLANT_A0: MonsterDefinition = MonsterDefinition {
    content_id: SNAKE_PLANT_ID,
    name: "Snake Plant",
    hp: 77,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Snecko at ascension 0: 114-120 HP, opens with Confusion.
pub const SNECKO_A0: MonsterDefinition = MonsterDefinition {
    content_id: SNECKO_ID,
    name: "Snecko",
    hp: 117,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Centurion at ascension 0: 76-80 HP, Slash/Protect/Fury move table.
pub const CENTURION_A0: MonsterDefinition = MonsterDefinition {
    content_id: CENTURION_ID,
    name: "Centurion",
    hp: 78,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Mystic at ascension 0: 48-56 HP, heals and buffs all living monsters.
pub const HEALER_A0: MonsterDefinition = MonsterDefinition {
    content_id: HEALER_ID,
    name: "Mystic",
    hp: 52,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Byrd at ascension 0: 25-31 HP, starts with Flight 3.
pub const BYRD_A0: MonsterDefinition = MonsterDefinition {
    content_id: BYRD_ID,
    name: "Byrd",
    hp: 28,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Shelled Parasite at ascension 0: 68-72 HP, starts with Plated Armor 14 and block 14.
pub const SHELLED_PARASITE_A0: MonsterDefinition = MonsterDefinition {
    content_id: SHELLED_PARASITE_ID,
    name: "Shelled Parasite",
    hp: 70,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 elite Book of Stabbing at ascension 0: 160-164 HP, starts with Painful Stabs.
pub const BOOK_OF_STABBING_A0: MonsterDefinition = MonsterDefinition {
    content_id: BOOK_OF_STABBING_ID,
    name: "Book of Stabbing",
    hp: 162,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 elite Taskmaster at ascension 0: 54-60 HP, repeatedly uses Scouring Whip.
pub const TASKMASTER_A0: MonsterDefinition = MonsterDefinition {
    content_id: TASKMASTER_ID,
    name: "Taskmaster",
    hp: 57,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 elite Gremlin Leader at ascension 0: 140-148 HP, Stab/Rally/Encourage AI.
pub const GREMLIN_LEADER_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_LEADER_ID,
    name: "Gremlin Leader",
    hp: 144,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1/2 Fungi Beast at ascension 0: 22-28 HP, starts with Spore Cloud 2.
pub const FUNGI_BEAST_A0: MonsterDefinition = MonsterDefinition {
    content_id: FUNGI_BEAST_ID,
    name: "Fungi Beast",
    hp: 25,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1/2 Blue Slaver at ascension 0: 46-50 HP, Stab/Rake representative sequence.
pub const SLAVER_BLUE_A0: MonsterDefinition = MonsterDefinition {
    content_id: SLAVER_BLUE_ID,
    name: "Blue Slaver",
    hp: 48,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1/2 Red Slaver at ascension 0: 46-50 HP, opens with Stab before Scrape/Entangle.
pub const SLAVER_RED_A0: MonsterDefinition = MonsterDefinition {
    content_id: SLAVER_RED_ID,
    name: "Red Slaver",
    hp: 48,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Gremlin Leader minion: Angry Gremlin, 20-24 HP, Scratch and Angry pre-battle power.
pub const GREMLIN_WARRIOR_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_WARRIOR_ID,
    name: "Gremlin Warrior",
    hp: 22,
    attack_damage: GREMLIN_WARRIOR_SCRATCH_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Gremlin Leader minion: Sneaky Gremlin, 10-14 HP, Puncture attack.
pub const GREMLIN_THIEF_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_THIEF_ID,
    name: "Gremlin Thief",
    hp: 12,
    attack_damage: GREMLIN_THIEF_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Gremlin Leader minion: Fat Gremlin, 13-17 HP, attack+Weak surface.
pub const GREMLIN_FAT_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_FAT_ID,
    name: "Gremlin Fat",
    hp: 15,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Gremlin Leader minion: Shield Gremlin, 12-15 HP, protect surface.
pub const GREMLIN_TSUNDERE_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_TSUNDERE_ID,
    name: "Gremlin Tsundere",
    hp: 13,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Gremlin Leader minion: Wizard Gremlin, 21-25 HP, charge then magic attack surface.
pub const GREMLIN_WIZARD_A0: MonsterDefinition = MonsterDefinition {
    content_id: GREMLIN_WIZARD_ID,
    name: "Gremlin Wizard",
    hp: 23,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 boss Bronze Automaton at ascension 0.
pub const BRONZE_AUTOMATON_A0: MonsterDefinition = MonsterDefinition {
    content_id: BRONZE_AUTOMATON_ID,
    name: "Bronze Automaton",
    hp: 300,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Bronze Automaton minion at ascension 0.
pub const BRONZE_ORB_A0: MonsterDefinition = MonsterDefinition {
    content_id: BRONZE_ORB_ID,
    name: "Bronze Orb",
    hp: 52,
    attack_damage: BRONZE_ORB_BEAM_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 boss The Collector at ascension 0.
pub const THE_COLLECTOR_A0: MonsterDefinition = MonsterDefinition {
    content_id: THE_COLLECTOR_ID,
    name: "The Collector",
    hp: 282,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// The Collector minion at ascension 0.
pub const TORCH_HEAD_A0: MonsterDefinition = MonsterDefinition {
    content_id: TORCH_HEAD_ID,
    name: "Torch Head",
    hp: 39,
    attack_damage: TORCH_HEAD_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 3 Orb Walker at ascension 0.
pub const ORB_WALKER_A0: MonsterDefinition = MonsterDefinition {
    content_id: ORB_WALKER_ID,
    name: "Orb Walker",
    hp: 96,
    attack_damage: ORB_WALKER_LASER_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 3 Darkling at ascension 0.
pub const DARKLING_A0: MonsterDefinition = MonsterDefinition {
    content_id: DARKLING_ID,
    name: "Darkling",
    hp: 56,
    attack_damage: DARKLING_CHOMP_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const BANDIT_BEAR_A0: MonsterDefinition = MonsterDefinition {
    content_id: BANDIT_BEAR_ID,
    name: "Bear",
    hp: 40,
    attack_damage: BANDIT_BEAR_MAUL_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const BANDIT_POINTY_A0: MonsterDefinition = MonsterDefinition {
    content_id: BANDIT_POINTY_ID,
    name: "Pointy",
    hp: 30,
    attack_damage: BANDIT_POINTY_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const BANDIT_LEADER_A0: MonsterDefinition = MonsterDefinition {
    content_id: BANDIT_LEADER_ID,
    name: "Romeo",
    hp: 37,
    attack_damage: BANDIT_LEADER_SLASH_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const CHAMP_A0: MonsterDefinition = MonsterDefinition {
    content_id: CHAMP_ID,
    name: "The Champ",
    hp: 420,
    attack_damage: CHAMP_HEAVY_SLASH_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const AWAKENED_ONE_A0: MonsterDefinition = MonsterDefinition {
    content_id: AWAKENED_ONE_ID,
    name: "Awakened One",
    hp: 300,
    attack_damage: AWAKENED_ONE_SLASH_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const DAGGER_A0: MonsterDefinition = MonsterDefinition {
    content_id: DAGGER_ID,
    name: "Dagger",
    hp: 23,
    attack_damage: DAGGER_WOUND_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const DECA_A0: MonsterDefinition = MonsterDefinition {
    content_id: DECA_ID,
    name: "Deca",
    hp: 250,
    attack_damage: DECA_BEAM_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const DONU_A0: MonsterDefinition = MonsterDefinition {
    content_id: DONU_ID,
    name: "Donu",
    hp: 250,
    attack_damage: DONU_BEAM_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 3 Exploder at ascension 0.
pub const EXPLODER_A0: MonsterDefinition = MonsterDefinition {
    content_id: EXPLODER_ID,
    name: "Exploder",
    hp: 30,
    attack_damage: EXPLODER_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const GIANT_HEAD_A0: MonsterDefinition = MonsterDefinition {
    content_id: GIANT_HEAD_ID,
    name: "Giant Head",
    hp: 500,
    attack_damage: GIANT_HEAD_COUNT_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const NEMESIS_A0: MonsterDefinition = MonsterDefinition {
    content_id: NEMESIS_ID,
    name: "Nemesis",
    hp: 185,
    attack_damage: NEMESIS_TRI_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const REPTOMANCER_A0: MonsterDefinition = MonsterDefinition {
    content_id: REPTOMANCER_ID,
    name: "Reptomancer",
    hp: 185,
    attack_damage: REPTOMANCER_SNAKE_STRIKE_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 3 Repulsor at ascension 0.
pub const REPULSOR_A0: MonsterDefinition = MonsterDefinition {
    content_id: REPULSOR_ID,
    name: "Repulsor",
    hp: 32,
    attack_damage: REPULSOR_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 3 Spiker at ascension 0.
pub const SPIKER_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPIKER_ID,
    name: "Spiker",
    hp: 49,
    attack_damage: SPIKER_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: SPIKER_THORNS,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const SPIRE_GROWTH_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPIRE_GROWTH_ID,
    name: "Spire Growth",
    hp: 170,
    attack_damage: SPIRE_GROWTH_QUICK_TACKLE_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const MAW_A0: MonsterDefinition = MonsterDefinition {
    content_id: MAW_ID,
    name: "The Maw",
    hp: 300,
    attack_damage: MAW_SLAM_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const TIME_EATER_A0: MonsterDefinition = MonsterDefinition {
    content_id: TIME_EATER_ID,
    name: "Time Eater",
    hp: 456,
    attack_damage: TIME_EATER_HEAD_SLAM_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const TRANSIENT_A0: MonsterDefinition = MonsterDefinition {
    content_id: TRANSIENT_ID,
    name: "Transient",
    hp: TRANSIENT_HP,
    attack_damage: TRANSIENT_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const WRITHING_MASS_A0: MonsterDefinition = MonsterDefinition {
    content_id: WRITHING_MASS_ID,
    name: "Writhing Mass",
    hp: 160,
    attack_damage: WRITHING_MASS_BIG_HIT_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const CORRUPT_HEART_A0: MonsterDefinition = MonsterDefinition {
    content_id: CORRUPT_HEART_ID,
    name: "Corrupt Heart",
    hp: 750,
    attack_damage: CORRUPT_HEART_ECHO_ATTACK_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const SPIRE_SHIELD_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPIRE_SHIELD_ID,
    name: "Spire Shield",
    hp: 110,
    attack_damage: SPIRE_SHIELD_BASH_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

pub const SPIRE_SPEAR_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPIRE_SPEAR_ID,
    name: "Spire Spear",
    hp: 160,
    attack_damage: SPIRE_SPEAR_BURN_STRIKE_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Lagavulin fixture at ascension 0: 109-111 HP, sleeps 3 turns, then attacks twice and siphons.
pub const LAGAVULIN_A0: MonsterDefinition = MonsterDefinition {
    content_id: LAGAVULIN_ID,
    name: "Lagavulin",
    hp: 110,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: LAGAVULIN_SLEEP_TURNS,
    starting_defensive_turns: 0,
};

/// Dead Adventurer / MonsterHelper `"Lagavulin Event"`: `new Lagavulin(false)`.
///
/// Awake on entry — no sleep armor. Target `usePreBattleAction` when `asleep`
/// is false sets Siphon Soul (`STRONG_DEBUFF`, move byte 1) as the opening
/// intent. Subsequent cycle is Attack → Attack → Siphon (see
/// `lagavulin_intent` phase alignment via opening `moves_executed`).
pub const LAGAVULIN_EVENT_A0: MonsterDefinition = MonsterDefinition {
    content_id: LAGAVULIN_ID,
    name: "Lagavulin",
    hp: 110,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Sentry at ascension 0: 38-42 HP, Beam / Attack alternating by position.
pub const SENTRY_A0: MonsterDefinition = MonsterDefinition {
    content_id: SENTRY_ID,
    name: "Sentry",
    hp: 40,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Hexaghost at ascension 0: 250 HP, Divider / Tackle / Inferno cycle.
pub const HEXAGHOST_A0: MonsterDefinition = MonsterDefinition {
    content_id: HEXAGHOST_ID,
    name: "Hexaghost",
    hp: 250,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Slime Boss at ascension 0: 140 HP, slams for 35, splits into acid slimes at 50% HP.
pub const SLIME_BOSS_A0: MonsterDefinition = MonsterDefinition {
    content_id: SLIME_BOSS_ID,
    name: "Slime Boss",
    hp: 140,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Guardian at ascension 0: 240 HP, Mode Shift defensive transitions.
pub const GUARDIAN_A0: MonsterDefinition = MonsterDefinition {
    content_id: GUARDIAN_ID,
    name: "Guardian",
    hp: 240,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Spike Slime at ascension 0: 14 HP, Lick (weak) / Spit (attack) cycle.
pub const SPIKE_SLIME_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPIKE_SLIME_ID,
    name: "Spike Slime",
    hp: 14,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 1 Acid Slime (small) at ascension 0: 12 HP, attack then apply weak cycle.
pub const ACID_SLIME_A0: MonsterDefinition = MonsterDefinition {
    content_id: ACID_SLIME_ID,
    name: "Acid Slime (S)",
    hp: 12,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

/// Act 2 Spheric Guardian at ascension 0: 20 HP, starts with Artifact 3 and 40 block,
/// then opens with Harden for 25 block.
pub const SPHERIC_GUARDIAN_A0: MonsterDefinition = MonsterDefinition {
    content_id: SPHERIC_GUARDIAN_ID,
    name: "Spheric Guardian",
    hp: 20,
    attack_damage: SPHERIC_GUARDIAN_DAMAGE,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: 0,
    starting_defensive_turns: 0,
};

#[must_use]
pub fn target_louse_curl_up_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        LOUSE_A7_CURL_UP_RANGE
    } else {
        LOUSE_A0_CURL_UP_RANGE
    }
}

#[must_use]
pub fn target_cultist_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        CULTIST_A7_HP_RANGE
    } else {
        CULTIST_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_jaw_worm_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        JAW_WORM_A7_HP_RANGE
    } else {
        JAW_WORM_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_spike_slime_s_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        SPIKE_SLIME_S_A7_HP_RANGE
    } else {
        SPIKE_SLIME_S_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_acid_slime_s_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        ACID_SLIME_S_A7_HP_RANGE
    } else {
        ACID_SLIME_S_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_spike_slime_m_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        SPIKE_SLIME_M_A7_HP_RANGE
    } else {
        SPIKE_SLIME_M_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_acid_slime_m_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        ACID_SLIME_M_A7_HP_RANGE
    } else {
        ACID_SLIME_M_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_spike_slime_l_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        SPIKE_SLIME_L_A7_HP_RANGE
    } else {
        SPIKE_SLIME_L_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_acid_slime_l_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        ACID_SLIME_L_A7_HP_RANGE
    } else {
        ACID_SLIME_L_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_louse_normal_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        LOUSE_NORMAL_A7_HP_RANGE
    } else {
        LOUSE_NORMAL_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_louse_defensive_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        LOUSE_DEFENSIVE_A7_HP_RANGE
    } else {
        LOUSE_DEFENSIVE_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_louse_bite_damage_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 2 {
        LOUSE_A2_BITE_DAMAGE_RANGE
    } else {
        LOUSE_A0_BITE_DAMAGE_RANGE
    }
}

#[must_use]
pub fn target_sentry_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 8 {
        SENTRY_A8_HP_RANGE
    } else {
        SENTRY_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_lagavulin_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 8 {
        LAGAVULIN_A8_HP_RANGE
    } else {
        LAGAVULIN_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_gremlin_nob_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 8 {
        GREMLIN_NOB_A8_HP_RANGE
    } else {
        GREMLIN_NOB_A0_HP_RANGE
    }
}

#[must_use]
pub fn target_sentry_attack_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        SENTRY_A3_ATTACK_DAMAGE
    } else {
        SENTRY_ATTACK_DAMAGE
    }
}

#[must_use]
pub fn target_looter_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        LOOTER_A7_HP_RANGE
    } else {
        LOOTER_A0_HP_RANGE
    }
}

#[must_use]
pub fn looter_swipe_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        LOOTER_A2_SWIPE_DAMAGE
    } else {
        LOOTER_SWIPE_DAMAGE
    }
}

#[must_use]
pub fn looter_theft(ascension: u8) -> i32 {
    if ascension >= 17 {
        LOOTER_A17_THEFT
    } else {
        LOOTER_THEFT
    }
}

#[must_use]
pub fn target_cultist_hp_roll(seed: i64, floor_num: u32, ascension: u8) -> i32 {
    let mut rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_cultist_hp_range(ascension).roll(&mut rng)
}

#[must_use]
pub fn target_jaw_worm_hp_roll(seed: i64, floor_num: u32, ascension: u8) -> i32 {
    let mut rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_jaw_worm_hp_range(ascension).roll(&mut rng)
}

#[must_use]
pub fn target_looter_hp_roll(seed: i64, floor_num: u32, ascension: u8) -> i32 {
    let mut rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_looter_hp_range(ascension).roll(&mut rng)
}

#[must_use]
pub fn target_small_slimes_variant(seed: i64, floor_num: u32) -> SmallSlimesVariant {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    if misc_rng.random_bool() {
        SmallSlimesVariant::SpikeSmallAcidMedium
    } else {
        SmallSlimesVariant::AcidSmallSpikeMedium
    }
}

#[must_use]
pub fn target_large_slime_variant(seed: i64, floor_num: u32) -> LargeSlimeVariant {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    if misc_rng.random_bool() {
        LargeSlimeVariant::Acid
    } else {
        LargeSlimeVariant::Spike
    }
}

#[must_use]
pub fn target_small_slimes_hp_rolls(
    seed: i64,
    floor_num: u32,
    ascension: u8,
) -> Option<Vec<TargetMonsterHp>> {
    match target_small_slimes_variant(seed, floor_num) {
        SmallSlimesVariant::SpikeSmallAcidMedium => {
            let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
            Some(vec![
                TargetMonsterHp {
                    name: "Spike Slime (S)",
                    hp: target_spike_slime_s_hp_range(ascension).roll(&mut hp_rng),
                },
                TargetMonsterHp {
                    name: "Acid Slime (M)",
                    hp: target_acid_slime_m_hp_range(ascension).roll(&mut hp_rng),
                },
            ])
        }
        SmallSlimesVariant::AcidSmallSpikeMedium => {
            let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
            Some(vec![
                TargetMonsterHp {
                    name: "Acid Slime (S)",
                    hp: target_acid_slime_s_hp_range(ascension).roll(&mut hp_rng),
                },
                TargetMonsterHp {
                    name: "Spike Slime (M)",
                    hp: target_spike_slime_m_hp_range(ascension).roll(&mut hp_rng),
                },
            ])
        }
    }
}

#[must_use]
pub fn target_large_slime_hp_roll(seed: i64, floor_num: u32, ascension: u8) -> TargetMonsterHp {
    let variant = target_large_slime_variant(seed, floor_num);
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    match variant {
        LargeSlimeVariant::Acid => TargetMonsterHp {
            name: "Acid Slime (L)",
            hp: target_acid_slime_l_hp_range(ascension).roll(&mut hp_rng),
        },
        LargeSlimeVariant::Spike => TargetMonsterHp {
            name: "Spike Slime (L)",
            hp: target_spike_slime_l_hp_range(ascension).roll(&mut hp_rng),
        },
    }
}

#[must_use]
pub fn target_two_louse_kinds(seed: i64, floor_num: u32) -> [LouseKind; 2] {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    [
        target_louse_kind(&mut misc_rng),
        target_louse_kind(&mut misc_rng),
    ]
}

#[must_use]
pub fn target_two_louse_hp_rolls(seed: i64, floor_num: u32, ascension: u8) -> Vec<TargetMonsterHp> {
    target_two_louse_spawn_states(seed, floor_num, ascension, false)
        .into_iter()
        .map(|spawn| TargetMonsterHp {
            name: spawn.name,
            hp: spawn.max_hp,
        })
        .collect()
}

#[must_use]
pub fn target_two_louse_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    target_louse_spawn_states(seed, floor_num, ascension, neow_lament, 2)
}

#[must_use]
pub fn target_three_louse_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    target_louse_spawn_states(seed, floor_num, ascension, neow_lament, 3)
}

fn target_louse_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
    count: usize,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));

    let mut spawns = (0..count)
        .map(|_| {
            let kind = target_louse_kind(&mut misc_rng);
            let hp_range = match kind {
                LouseKind::Normal => target_louse_normal_hp_range(ascension),
                LouseKind::Defensive => target_louse_defensive_hp_range(ascension),
            };
            let max_hp = hp_range.roll(&mut hp_rng);
            let bite_damage = target_louse_bite_damage_range(ascension).roll(&mut hp_rng);
            let name = match kind {
                LouseKind::Normal => "LouseNormal",
                LouseKind::Defensive => "LouseDefensive",
            };
            let mut spawn = target_combat_entry_spawn(name, max_hp, neow_lament, Vec::new());
            spawn.rolled_attack_damage = Some(bite_damage);
            spawn
        })
        .collect::<Vec<_>>();

    for spawn in &mut spawns {
        spawn.powers = vec![TargetSpawnPower {
            id: "Curl Up",
            amount: target_louse_curl_up_range(ascension).roll(&mut hp_rng),
        }];
    }

    spawns
}

#[must_use]
pub fn target_normal_encounter_spawn_at_combat_index(
    seed: i64,
    floor_num: u32,
    combat_index: usize,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    use crate::content::encounters::normal_encounter_key_at_combat_index;

    let encounter_key = normal_encounter_key_at_combat_index(seed, combat_index)?;
    target_encounter_spawn_for_key(seed, floor_num, &encounter_key, ascension, neow_lament)
}

#[must_use]
pub fn target_city_normal_encounter_group_at_combat_index(
    seed: i64,
    combat_index: usize,
) -> Option<TargetEncounterGroup> {
    use crate::content::encounters::city_normal_encounter_key_at_combat_index;

    let encounter_key = city_normal_encounter_key_at_combat_index(seed, combat_index)?;
    target_city_encounter_group_for_key(&encounter_key)
}

#[must_use]
pub fn target_city_encounter_group_for_key(encounter_key: &str) -> Option<TargetEncounterGroup> {
    let member = |monster_name, x, y| TargetEncounterMember { monster_name, x, y };
    let group = |display_name, members| TargetEncounterGroup {
        encounter_key: encounter_key.to_owned(),
        display_name,
        members,
    };

    match encounter_key {
        "2 Thieves" => Some(group(
            "2 Thieves",
            vec![
                member("Looter", Some("-200.0"), Some("15.0")),
                member("Mugger", Some("80.0"), Some("0.0")),
            ],
        )),
        "3 Byrds" => Some(group(
            "3 Byrds",
            vec![
                member("Byrd", Some("-360.0"), Some("random(25.0, 70.0)")),
                member("Byrd", Some("-80.0"), Some("random(25.0, 70.0)")),
                member("Byrd", Some("200.0"), Some("random(25.0, 70.0)")),
            ],
        )),
        "Chosen" => Some(group("Chosen", vec![member("Chosen", None, None)])),
        "Shell Parasite" => Some(group(
            "Shell Parasite",
            vec![member("ShelledParasite", None, None)],
        )),
        "Spheric Guardian" => Some(group(
            "Spheric Guardian",
            vec![member("SphericGuardian", None, None)],
        )),
        "Cultist and Chosen" => Some(group(
            "Cultist and Chosen",
            vec![
                member("Cultist", Some("-230.0"), Some("15.0")),
                member("Chosen", Some("100.0"), Some("25.0")),
            ],
        )),
        "3 Cultists" => Some(group(
            "3 Cultists",
            vec![
                member("Cultist", Some("-465.0"), Some("-20.0")),
                member("Cultist", Some("-130.0"), Some("15.0")),
                member("Cultist", Some("200.0"), Some("-5.0")),
            ],
        )),
        "Chosen and Byrds" => Some(group(
            "Chosen and Byrds",
            vec![
                member("Byrd", Some("-170.0"), Some("random(25.0, 70.0)")),
                member("Chosen", Some("80.0"), Some("0.0")),
            ],
        )),
        "Sentry and Sphere" => Some(group(
            "Sentry and Sphere",
            vec![
                member("Sentry", Some("-305.0"), Some("30.0")),
                member("SphericGuardian", None, None),
            ],
        )),
        "Snake Plant" => Some(group(
            "Snake Plant",
            vec![member("SnakePlant", Some("-30.0"), Some("-30.0"))],
        )),
        "Snecko" => Some(group("Snecko", vec![member("Snecko", None, None)])),
        "Centurion and Healer" => Some(group(
            "Centurion and Healer",
            vec![
                member("Centurion", Some("-200.0"), Some("15.0")),
                member("Healer", Some("120.0"), Some("0.0")),
            ],
        )),
        "Shelled Parasite and Fungi" => Some(group(
            "Shelled Parasite and Fungi",
            vec![
                member("ShelledParasite", Some("-260.0"), Some("15.0")),
                member("FungiBeast", Some("120.0"), Some("0.0")),
            ],
        )),
        "Book of Stabbing" => Some(group(
            "Book of Stabbing",
            vec![member("BookOfStabbing", None, None)],
        )),
        "Gremlin Leader" => Some(group(
            "Gremlin Leader",
            vec![
                member(
                    "random gremlin",
                    Some("GremlinLeader.POSX[0]"),
                    Some("GremlinLeader.POSY[0]"),
                ),
                member(
                    "random gremlin",
                    Some("GremlinLeader.POSX[1]"),
                    Some("GremlinLeader.POSY[1]"),
                ),
                member("GremlinLeader", None, None),
            ],
        )),
        "Slavers" => Some(group(
            "Taskmaster",
            vec![
                member("SlaverBlue", Some("-385.0"), Some("-15.0")),
                member("Taskmaster", Some("-133.0"), Some("0.0")),
                member("SlaverRed", Some("125.0"), Some("-30.0")),
            ],
        )),
        _ => None,
    }
}

#[must_use]
pub fn executable_city_member_definition(monster_name: &str) -> Option<&'static MonsterDefinition> {
    match monster_name {
        "Cultist" => Some(&CULTIST_A0),
        "Looter" => Some(&LOOTER_A0),
        "Sentry" => Some(&SENTRY_A0),
        "SphericGuardian" | "Spheric Guardian" => Some(&SPHERIC_GUARDIAN_A0),
        "Mugger" => Some(&MUGGER_A0),
        "Chosen" => Some(&CHOSEN_A0),
        "SnakePlant" | "Snake Plant" => Some(&SNAKE_PLANT_A0),
        "Snecko" => Some(&SNECKO_A0),
        "Centurion" => Some(&CENTURION_A0),
        "Healer" => Some(&HEALER_A0),
        "Byrd" => Some(&BYRD_A0),
        "ShelledParasite" | "Shell Parasite" | "Shelled Parasite" => Some(&SHELLED_PARASITE_A0),
        "BookOfStabbing" | "Book of Stabbing" => Some(&BOOK_OF_STABBING_A0),
        "Taskmaster" | "SlaverBoss" => Some(&TASKMASTER_A0),
        "GremlinLeader" | "Gremlin Leader" => Some(&GREMLIN_LEADER_A0),
        "FungiBeast" | "Fungi Beast" => Some(&FUNGI_BEAST_A0),
        "SlaverBlue" | "Blue Slaver" => Some(&SLAVER_BLUE_A0),
        "SlaverRed" | "Red Slaver" => Some(&SLAVER_RED_A0),
        "random gremlin" | "GremlinWarrior" | "Gremlin Warrior" => Some(&GREMLIN_WARRIOR_A0),
        "GremlinThief" | "Gremlin Thief" => Some(&GREMLIN_THIEF_A0),
        "GremlinFat" | "Gremlin Fat" => Some(&GREMLIN_FAT_A0),
        "GremlinTsundere" | "Gremlin Tsundere" => Some(&GREMLIN_TSUNDERE_A0),
        "GremlinWizard" | "Gremlin Wizard" => Some(&GREMLIN_WIZARD_A0),
        _ => None,
    }
}

#[must_use]
pub fn executable_city_encounter_monsters_for_key(
    encounter_key: &str,
) -> Option<Vec<MonsterState>> {
    let group = target_city_encounter_group_for_key(encounter_key)?;
    group
        .members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let definition = executable_city_member_definition(member.monster_name)?;
            Some(monster_state(definition, MonsterId::new(index as u64 + 1)))
        })
        .collect()
}

#[must_use]
pub fn target_city_normal_encounter_spawn_at_combat_index(
    seed: i64,
    floor_num: u32,
    combat_index: usize,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    use crate::content::encounters::city_normal_encounter_key_at_combat_index;

    let encounter_key = city_normal_encounter_key_at_combat_index(seed, combat_index)?;
    target_city_encounter_spawn_for_key(seed, floor_num, &encounter_key, ascension, neow_lament)
}

#[must_use]
pub fn target_elite_encounter_spawn_at_combat_index(
    seed: i64,
    act: crate::map::TargetMapAct,
    floor_num: u32,
    combat_index: usize,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    use crate::content::encounters::{
        city_elite_encounter_key_at_combat_index, exordium_elite_encounter_key_at_combat_index,
    };

    let encounter_key = match act {
        crate::map::TargetMapAct::Exordium => {
            exordium_elite_encounter_key_at_combat_index(seed, combat_index)?
        }
        crate::map::TargetMapAct::City => {
            city_elite_encounter_key_at_combat_index(seed, combat_index)?
        }
        crate::map::TargetMapAct::Beyond => {
            crate::content::encounters::generate_beyond_elite_encounters(seed)
                .into_iter()
                .nth(combat_index)?
        }
    };
    match act {
        crate::map::TargetMapAct::Exordium => {
            target_encounter_spawn_for_key(seed, floor_num, &encounter_key, ascension, neow_lament)
        }
        crate::map::TargetMapAct::City => target_city_encounter_spawn_for_key(
            seed,
            floor_num,
            &encounter_key,
            ascension,
            neow_lament,
        ),
        crate::map::TargetMapAct::Beyond => target_beyond_encounter_spawn_for_key(
            seed,
            floor_num,
            &encounter_key,
            ascension,
            neow_lament,
        ),
    }
}

#[must_use]
pub fn target_beyond_encounter_spawn_for_key(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_beyond_encounter_spawn_for_key_with_misc_rng(
        seed,
        floor_num,
        encounter_key,
        ascension,
        neow_lament,
        &mut misc_rng,
    )
}

#[must_use]
pub fn target_beyond_encounter_spawn_for_key_with_misc_rng(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
    misc_rng: &mut StsRng,
) -> Option<Vec<TargetEncounterSpawn>> {
    match encounter_key {
        "3 Darklings" => target_darkling_encounter_spawn(seed, floor_num, ascension, neow_lament),
        "3 Shapes" => Some(target_three_shapes_encounter_spawn(
            seed,
            floor_num,
            ascension,
            neow_lament,
            misc_rng,
        )),
        "4 Shapes" => Some(target_four_shapes_encounter_spawn(
            seed,
            floor_num,
            ascension,
            neow_lament,
            misc_rng,
        )),
        "Jaw Worm Horde" => Some(target_jaw_worm_horde_spawn(
            seed,
            floor_num,
            ascension,
            neow_lament,
        )),
        "Orb Walker" => {
            let mut spawn = target_combat_entry_spawn(
                "Orb Walker",
                target_orb_walker_hp(seed, floor_num, ascension),
                neow_lament,
                vec![TargetSpawnPower {
                    id: "Generic Strength Up Power",
                    amount: ORB_WALKER_STRENGTH_UP,
                }],
            );
            let damage = orb_walker_laser_damage(ascension);
            spawn.intent = TargetSpawnIntent::Attack { damage };
            spawn.rolled_attack_damage = Some(damage);
            Some(vec![spawn])
        }
        "Transient" => {
            let mut spawn =
                target_combat_entry_spawn("Transient", TRANSIENT_HP, neow_lament, vec![]);
            let damage = transient_attack_base_damage(ascension);
            spawn.intent = TargetSpawnIntent::Attack { damage };
            spawn.rolled_attack_damage = Some(damage);
            Some(vec![spawn])
        }
        "Sphere and 2 Shapes" => Some(target_sphere_and_two_shapes_encounter_spawn(
            seed,
            floor_num,
            ascension,
            neow_lament,
            misc_rng,
        )),
        "Spire Growth" => Some(vec![target_definition_spawn(&SPIRE_GROWTH_A0, neow_lament)]),
        "Maw" => Some(vec![target_definition_spawn(&MAW_A0, neow_lament)]),
        "Writhing Mass" => {
            let mut spawn = target_definition_spawn(&WRITHING_MASS_A0, neow_lament);
            spawn.powers.push(TargetSpawnPower {
                id: "Malleable",
                amount: WRITHING_MASS_MALLEABLE,
            });
            Some(vec![spawn])
        }
        "Giant Head" => Some(vec![target_definition_spawn(&GIANT_HEAD_A0, neow_lament)]),
        "Nemesis" => Some(vec![target_definition_spawn(&NEMESIS_A0, neow_lament)]),
        "Reptomancer" => Some(target_reptomancer_encounter_spawn(
            seed,
            floor_num,
            ascension,
            neow_lament,
        )),
        _ => None,
    }
}

fn target_reptomancer_encounter_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let left_dagger_hp = DAGGER_HP_RANGE.roll(&mut hp_rng);
    let _reptomancer_constructor_hp = REPTOMANCER_A0_HP_RANGE.roll(&mut hp_rng);
    let reptomancer_hp = reptomancer_hp_range(ascension).roll(&mut hp_rng);
    let right_dagger_hp = DAGGER_HP_RANGE.roll(&mut hp_rng);
    vec![
        target_dagger_spawn(left_dagger_hp, neow_lament),
        target_combat_entry_spawn("Reptomancer", reptomancer_hp, neow_lament, vec![]),
        target_dagger_spawn(right_dagger_hp, neow_lament),
    ]
}

fn target_dagger_spawn(max_hp: i32, neow_lament: bool) -> TargetEncounterSpawn {
    target_combat_entry_spawn(
        "Dagger",
        max_hp,
        neow_lament,
        vec![TargetSpawnPower {
            id: "Minion",
            amount: 1,
        }],
    )
}

fn target_jaw_worm_horde_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let floor_seed = seed_for_floor(seed, floor_num);
    let mut hp_rng = StsRng::new(floor_seed);
    let mut ai_rng = StsRng::new(floor_seed);
    let hp_range = if ascension >= 7 {
        JAW_WORM_A7_HP_RANGE
    } else {
        JAW_WORM_A0_HP_RANGE
    };

    let mut spawns = Vec::with_capacity(3);
    for _ in 0..3 {
        let max_hp = hp_range.roll(&mut hp_rng);
        let mut spawn = target_combat_entry_spawn(
            "Jaw Worm",
            max_hp,
            neow_lament,
            vec![TargetSpawnPower {
                id: "Strength",
                amount: JAW_WORM_HORDE_STARTING_STRENGTH,
            }],
        );
        spawn.block = JAW_WORM_HORDE_STARTING_BLOCK;
        let roll = ai_rng.random_int(99);
        if roll < 25 {
            spawn.intent = TargetSpawnIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE,
            };
            spawn.rolled_attack_damage = Some(JAW_WORM_CHOMP_DAMAGE);
        } else if roll < 55 {
            spawn.intent = TargetSpawnIntent::AttackAndBlock {
                damage: JAW_WORM_THRASH_DAMAGE,
                block: JAW_WORM_THRASH_BLOCK,
            };
            spawn.rolled_attack_damage = Some(JAW_WORM_THRASH_DAMAGE);
        } else {
            spawn.intent = TargetSpawnIntent::StrengthAndBlock {
                strength: JAW_WORM_BELLOW_STRENGTH,
                block: JAW_WORM_BELLOW_BLOCK,
            };
        }
        spawns.push(spawn);
    }
    spawns
}

fn target_orb_walker_hp(seed: i64, floor_num: u32, ascension: u8) -> i32 {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let hp_range = if ascension >= 7 {
        ORB_WALKER_A7_HP_RANGE
    } else {
        ORB_WALKER_A0_HP_RANGE
    };
    let _constructor_hp_roll = hp_rng.random_int(0);
    hp_range.roll(&mut hp_rng)
}

fn target_definition_spawn(
    definition: &MonsterDefinition,
    neow_lament: bool,
) -> TargetEncounterSpawn {
    target_combat_entry_spawn(definition.name, definition.hp, neow_lament, Vec::new())
}

fn target_darkling_encounter_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let hp_range = if ascension >= 7 {
        DARKLING_A7_HP_RANGE
    } else {
        DARKLING_A0_HP_RANGE
    };
    let nip_range = if ascension >= 2 {
        DARKLING_A2_NIP_DAMAGE_RANGE
    } else {
        DARKLING_A0_NIP_DAMAGE_RANGE
    };

    let mut spawns = Vec::with_capacity(3);
    for _ in 0..3 {
        let max_hp = hp_range.roll(&mut hp_rng);
        let nip_damage = nip_range.roll(&mut hp_rng);
        let mut spawn = target_combat_entry_spawn("Darkling", max_hp, neow_lament, Vec::new());
        spawn.rolled_attack_damage = Some(nip_damage);
        spawns.push(spawn);
    }
    Some(spawns)
}

fn target_three_shapes_encounter_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
    misc_rng: &mut StsRng,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_shapes_from_pool(3, misc_rng, &mut hp_rng, ascension, neow_lament)
}

fn target_four_shapes_encounter_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
    misc_rng: &mut StsRng,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_shapes_from_pool(4, misc_rng, &mut hp_rng, ascension, neow_lament)
}

fn target_sphere_and_two_shapes_encounter_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
    misc_rng: &mut StsRng,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut spawns = (0..2)
        .map(|_| {
            let name = target_random_ancient_shape_name(misc_rng);
            target_ancient_shape_spawn(name, &mut hp_rng, ascension, neow_lament)
        })
        .collect::<Vec<_>>();
    let mut spheric_guardian = target_definition_spawn(&SPHERIC_GUARDIAN_A0, neow_lament);
    spheric_guardian.block = SPHERIC_GUARDIAN_STARTING_BLOCK;
    spheric_guardian.powers.push(TargetSpawnPower {
        id: "Artifact",
        amount: SPHERIC_GUARDIAN_ARTIFACT,
    });
    spawns.push(spheric_guardian);
    spawns
}

fn target_shapes_from_pool(
    count: usize,
    misc_rng: &mut StsRng,
    hp_rng: &mut StsRng,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut pool = vec![
        "Repulsor", "Repulsor", "Exploder", "Exploder", "Spiker", "Spiker",
    ];
    (0..count)
        .map(|_| {
            let index = misc_rng.random_int((pool.len() - 1) as i32) as usize;
            let name = pool.remove(index);
            target_ancient_shape_spawn(name, hp_rng, ascension, neow_lament)
        })
        .collect()
}

fn target_random_ancient_shape_name(misc_rng: &mut StsRng) -> &'static str {
    match misc_rng.random_int(2) {
        0 => "Spiker",
        1 => "Repulsor",
        _ => "Exploder",
    }
}

fn target_ancient_shape_spawn(
    name: &str,
    hp_rng: &mut StsRng,
    ascension: u8,
    neow_lament: bool,
) -> TargetEncounterSpawn {
    let spiker_hp_range = if ascension >= 7 {
        SPIKER_A7_HP_RANGE
    } else {
        SPIKER_A0_HP_RANGE
    };
    let repulsor_hp_range = if ascension >= 7 {
        REPULSOR_A7_HP_RANGE
    } else {
        REPULSOR_A0_HP_RANGE
    };
    let exploder_damage = if ascension >= 2 {
        EXPLODER_A2_ATTACK_DAMAGE
    } else {
        EXPLODER_ATTACK_DAMAGE
    };
    let spiker_thorns = if ascension >= 2 {
        SPIKER_A2_THORNS
    } else {
        SPIKER_THORNS
    };

    match name {
        "Spiker" => target_combat_entry_spawn(
            "Spiker",
            spiker_hp_range.roll(hp_rng),
            neow_lament,
            vec![TargetSpawnPower {
                id: "Thorns",
                amount: spiker_thorns,
            }],
        ),
        "Repulsor" => target_combat_entry_spawn(
            "Repulsor",
            repulsor_hp_range.roll(hp_rng),
            neow_lament,
            Vec::new(),
        ),
        "Exploder" => {
            let _constructor_hp_roll = hp_rng.random_int(0);
            let mut spawn = target_combat_entry_spawn(
                "Exploder",
                EXPLODER_A0.hp,
                neow_lament,
                vec![TargetSpawnPower {
                    id: "Explosive",
                    amount: EXPLODER_EXPLOSIVE,
                }],
            );
            spawn.intent = TargetSpawnIntent::Attack {
                damage: exploder_damage,
            };
            spawn.rolled_attack_damage = Some(exploder_damage);
            spawn
        }
        _ => unreachable!("ancient shape pool contains only Spiker, Repulsor, and Exploder"),
    }
}

#[must_use]
pub fn target_city_encounter_spawn_for_key(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    target_city_encounter_spawn_for_key_with_misc_rng(
        seed,
        floor_num,
        encounter_key,
        ascension,
        neow_lament,
        &mut misc_rng,
    )
}

#[must_use]
pub fn target_city_encounter_spawn_for_key_with_misc_rng(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
    misc_rng: &mut StsRng,
) -> Option<Vec<TargetEncounterSpawn>> {
    let group = target_city_encounter_group_for_key(encounter_key)?;
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    group
        .members
        .iter()
        .map(|member| {
            target_city_member_spawn(
                member.monster_name,
                &mut hp_rng,
                misc_rng,
                ascension,
                neow_lament,
            )
        })
        .collect()
}

fn target_city_member_spawn(
    monster_name: &str,
    hp_rng: &mut StsRng,
    misc_rng: &mut StsRng,
    ascension: u8,
    neow_lament: bool,
) -> Option<TargetEncounterSpawn> {
    let monster_name = if monster_name == "random gremlin" {
        target_random_gremlin_name(misc_rng)
    } else {
        monster_name
    };
    let (name, hp_range) = match monster_name {
        "Cultist" => ("Cultist", target_cultist_hp_range(ascension)),
        "Looter" => ("Looter", target_looter_hp_range(ascension)),
        "Sentry" => ("Sentry", target_sentry_hp_range(ascension)),
        _ => {
            let profile = target_city_monster_profile(monster_name, ascension)?;
            (profile.monster_name, profile.hp_range)
        }
    };
    if name == "Taskmaster" {
        let _ = hp_range.roll(hp_rng);
    }
    let max_hp = hp_range.roll(hp_rng);
    let mut spawn = target_combat_entry_spawn(name, max_hp, neow_lament, Vec::new());

    match name {
        "Looter" => spawn.powers.push(TargetSpawnPower {
            id: "Thievery",
            amount: looter_theft(ascension),
        }),
        "Sentry" => spawn.powers.push(TargetSpawnPower {
            id: "Artifact",
            amount: SENTRY_ARTIFACT,
        }),
        "SphericGuardian" => {
            spawn.block = SPHERIC_GUARDIAN_STARTING_BLOCK;
            spawn.powers.push(TargetSpawnPower {
                id: "Artifact",
                amount: SPHERIC_GUARDIAN_ARTIFACT,
            });
        }
        "Byrd" => spawn.powers.push(TargetSpawnPower {
            id: "Flight",
            amount: BYRD_FLIGHT,
        }),
        "SnakePlant" => spawn.powers.push(TargetSpawnPower {
            id: "Malleable",
            amount: SNAKE_PLANT_MALLEABLE,
        }),
        "ShelledParasite" => {
            spawn.block = SHELLED_PARASITE_PLATED_ARMOR;
            spawn.powers.push(TargetSpawnPower {
                id: "Plated Armor",
                amount: SHELLED_PARASITE_PLATED_ARMOR,
            });
        }
        "BookOfStabbing" => spawn.powers.push(TargetSpawnPower {
            id: "Painful Stabs",
            amount: 1,
        }),
        "FungiBeast" => spawn.powers.push(TargetSpawnPower {
            id: "Spore Cloud",
            amount: FUNGI_BEAST_SPORE_CLOUD,
        }),
        "GremlinWarrior" => {
            spawn.powers.push(TargetSpawnPower {
                id: "Minion",
                amount: 1,
            });
            spawn.powers.push(TargetSpawnPower {
                id: "Angry",
                amount: GREMLIN_WARRIOR_ANGER,
            });
        }
        "GremlinThief" | "GremlinFat" | "GremlinTsundere" | "GremlinWizard" => {
            spawn.powers.push(TargetSpawnPower {
                id: "Minion",
                amount: 1,
            });
        }
        _ => {}
    }

    Some(spawn)
}

fn target_random_gremlin_name(misc_rng: &mut StsRng) -> &'static str {
    const WEIGHTED_GREMLINS: [&str; 8] = [
        "GremlinWarrior",
        "GremlinWarrior",
        "GremlinThief",
        "GremlinThief",
        "GremlinFat",
        "GremlinFat",
        "GremlinTsundere",
        "GremlinWizard",
    ];
    WEIGHTED_GREMLINS[misc_rng.random_int(WEIGHTED_GREMLINS.len() as i32 - 1) as usize]
}

#[must_use]
pub fn target_city_monster_hp_range(monster_name: &str, ascension: u8) -> Option<MonsterHpRange> {
    let range = match monster_name {
        "Byrd" => {
            if ascension >= 7 {
                BYRD_A7_HP_RANGE
            } else {
                BYRD_A0_HP_RANGE
            }
        }
        "Chosen" => {
            if ascension >= 7 {
                CHOSEN_A7_HP_RANGE
            } else {
                CHOSEN_A0_HP_RANGE
            }
        }
        "ShelledParasite" | "Shell Parasite" | "Shelled Parasite" => {
            if ascension >= 7 {
                SHELLED_PARASITE_A7_HP_RANGE
            } else {
                SHELLED_PARASITE_A0_HP_RANGE
            }
        }
        "SphericGuardian" | "Spheric Guardian" => SPHERIC_GUARDIAN_HP_RANGE,
        "Mugger" => {
            if ascension >= 7 {
                MUGGER_A7_HP_RANGE
            } else {
                MUGGER_A0_HP_RANGE
            }
        }
        "SnakePlant" | "Snake Plant" => {
            if ascension >= 7 {
                SNAKE_PLANT_A7_HP_RANGE
            } else {
                SNAKE_PLANT_A0_HP_RANGE
            }
        }
        "Snecko" => {
            if ascension >= 7 {
                SNECKO_A7_HP_RANGE
            } else {
                SNECKO_A0_HP_RANGE
            }
        }
        "Centurion" => {
            if ascension >= 7 {
                CENTURION_A7_HP_RANGE
            } else {
                CENTURION_A0_HP_RANGE
            }
        }
        "Healer" => {
            if ascension >= 7 {
                HEALER_A7_HP_RANGE
            } else {
                HEALER_A0_HP_RANGE
            }
        }
        "BookOfStabbing" | "Book of Stabbing" => {
            if ascension >= 8 {
                BOOK_OF_STABBING_A8_HP_RANGE
            } else {
                BOOK_OF_STABBING_A0_HP_RANGE
            }
        }
        "GremlinLeader" | "Gremlin Leader" => {
            if ascension >= 8 {
                GREMLIN_LEADER_A8_HP_RANGE
            } else {
                GREMLIN_LEADER_A0_HP_RANGE
            }
        }
        "Taskmaster" => {
            if ascension >= 8 {
                TASKMASTER_A8_HP_RANGE
            } else {
                TASKMASTER_A0_HP_RANGE
            }
        }
        "FungiBeast" | "Fungi Beast" => {
            if ascension >= 7 {
                FUNGI_BEAST_A7_HP_RANGE
            } else {
                FUNGI_BEAST_A0_HP_RANGE
            }
        }
        "SlaverBlue" | "Blue Slaver" | "SlaverRed" | "Red Slaver" => {
            if ascension >= 7 {
                SLAVER_A7_HP_RANGE
            } else {
                SLAVER_A0_HP_RANGE
            }
        }
        "random gremlin" | "GremlinWarrior" | "Gremlin Warrior" => {
            if ascension >= 7 {
                GREMLIN_WARRIOR_A7_HP_RANGE
            } else {
                GREMLIN_WARRIOR_A0_HP_RANGE
            }
        }
        "GremlinThief" | "Gremlin Thief" => {
            if ascension >= 7 {
                GREMLIN_THIEF_A7_HP_RANGE
            } else {
                GREMLIN_THIEF_A0_HP_RANGE
            }
        }
        "GremlinFat" | "Gremlin Fat" => {
            if ascension >= 7 {
                GREMLIN_FAT_A7_HP_RANGE
            } else {
                GREMLIN_FAT_A0_HP_RANGE
            }
        }
        "GremlinTsundere" | "Gremlin Tsundere" => {
            if ascension >= 7 {
                GREMLIN_TSUNDERE_A7_HP_RANGE
            } else {
                GREMLIN_TSUNDERE_A0_HP_RANGE
            }
        }
        "GremlinWizard" | "Gremlin Wizard" => {
            if ascension >= 7 {
                GREMLIN_WIZARD_A7_HP_RANGE
            } else {
                GREMLIN_WIZARD_A0_HP_RANGE
            }
        }
        _ => return None,
    };
    Some(range)
}

#[must_use]
pub fn target_monster_hp_range_for_content_id(
    content_id: ContentId,
    ascension: u8,
) -> Option<MonsterHpRange> {
    let range = match content_id {
        CULTIST_ID => target_cultist_hp_range(ascension),
        JAW_WORM_ID => target_jaw_worm_hp_range(ascension),
        SPIKE_SLIME_ID => {
            if ascension >= 7 {
                SPIKE_SLIME_L_A7_HP_RANGE
            } else {
                SPIKE_SLIME_L_A0_HP_RANGE
            }
        }
        ACID_SLIME_ID => {
            if ascension >= 7 {
                ACID_SLIME_L_A7_HP_RANGE
            } else {
                ACID_SLIME_L_A0_HP_RANGE
            }
        }
        RED_LOUSE_ID | GREEN_LOUSE_ID => {
            if ascension >= 7 {
                LOUSE_NORMAL_A7_HP_RANGE
            } else {
                LOUSE_NORMAL_A0_HP_RANGE
            }
        }
        SENTRY_ID => target_sentry_hp_range(ascension),
        LAGAVULIN_ID => target_lagavulin_hp_range(ascension),
        GREMLIN_NOB_ID => target_gremlin_nob_hp_range(ascension),
        LOOTER_ID => target_looter_hp_range(ascension),
        BYRD_ID => {
            if ascension >= 7 {
                BYRD_A7_HP_RANGE
            } else {
                BYRD_A0_HP_RANGE
            }
        }
        CHOSEN_ID => {
            if ascension >= 7 {
                CHOSEN_A7_HP_RANGE
            } else {
                CHOSEN_A0_HP_RANGE
            }
        }
        BANDIT_POINTY_ID => {
            if ascension >= 7 {
                BANDIT_POINTY_A7_HP_RANGE
            } else {
                BANDIT_POINTY_A0_HP_RANGE
            }
        }
        BANDIT_LEADER_ID => {
            if ascension >= 7 {
                BANDIT_LEADER_A7_HP_RANGE
            } else {
                BANDIT_LEADER_A0_HP_RANGE
            }
        }
        BANDIT_BEAR_ID => {
            if ascension >= 7 {
                BANDIT_BEAR_A7_HP_RANGE
            } else {
                BANDIT_BEAR_A0_HP_RANGE
            }
        }
        SHELLED_PARASITE_ID => {
            if ascension >= 7 {
                SHELLED_PARASITE_A7_HP_RANGE
            } else {
                SHELLED_PARASITE_A0_HP_RANGE
            }
        }
        SPHERIC_GUARDIAN_ID => SPHERIC_GUARDIAN_HP_RANGE,
        MUGGER_ID => {
            if ascension >= 7 {
                MUGGER_A7_HP_RANGE
            } else {
                MUGGER_A0_HP_RANGE
            }
        }
        SNAKE_PLANT_ID => {
            if ascension >= 7 {
                SNAKE_PLANT_A7_HP_RANGE
            } else {
                SNAKE_PLANT_A0_HP_RANGE
            }
        }
        SNECKO_ID => {
            if ascension >= 7 {
                SNECKO_A7_HP_RANGE
            } else {
                SNECKO_A0_HP_RANGE
            }
        }
        CENTURION_ID => {
            if ascension >= 7 {
                CENTURION_A7_HP_RANGE
            } else {
                CENTURION_A0_HP_RANGE
            }
        }
        HEALER_ID => {
            if ascension >= 7 {
                HEALER_A7_HP_RANGE
            } else {
                HEALER_A0_HP_RANGE
            }
        }
        BOOK_OF_STABBING_ID => {
            if ascension >= 8 {
                BOOK_OF_STABBING_A8_HP_RANGE
            } else {
                BOOK_OF_STABBING_A0_HP_RANGE
            }
        }
        GREMLIN_LEADER_ID => {
            if ascension >= 8 {
                GREMLIN_LEADER_A8_HP_RANGE
            } else {
                GREMLIN_LEADER_A0_HP_RANGE
            }
        }
        TASKMASTER_ID => {
            if ascension >= 8 {
                TASKMASTER_A8_HP_RANGE
            } else {
                TASKMASTER_A0_HP_RANGE
            }
        }
        FUNGI_BEAST_ID => {
            if ascension >= 7 {
                FUNGI_BEAST_A7_HP_RANGE
            } else {
                FUNGI_BEAST_A0_HP_RANGE
            }
        }
        SLAVER_BLUE_ID | SLAVER_RED_ID => {
            if ascension >= 7 {
                SLAVER_A7_HP_RANGE
            } else {
                SLAVER_A0_HP_RANGE
            }
        }
        GREMLIN_WARRIOR_ID => {
            if ascension >= 7 {
                GREMLIN_WARRIOR_A7_HP_RANGE
            } else {
                GREMLIN_WARRIOR_A0_HP_RANGE
            }
        }
        GREMLIN_THIEF_ID => {
            if ascension >= 7 {
                GREMLIN_THIEF_A7_HP_RANGE
            } else {
                GREMLIN_THIEF_A0_HP_RANGE
            }
        }
        GREMLIN_FAT_ID => {
            if ascension >= 7 {
                GREMLIN_FAT_A7_HP_RANGE
            } else {
                GREMLIN_FAT_A0_HP_RANGE
            }
        }
        GREMLIN_TSUNDERE_ID => {
            if ascension >= 7 {
                GREMLIN_TSUNDERE_A7_HP_RANGE
            } else {
                GREMLIN_TSUNDERE_A0_HP_RANGE
            }
        }
        GREMLIN_WIZARD_ID => {
            if ascension >= 7 {
                GREMLIN_WIZARD_A7_HP_RANGE
            } else {
                GREMLIN_WIZARD_A0_HP_RANGE
            }
        }
        DARKLING_ID => {
            if ascension >= 7 {
                DARKLING_A7_HP_RANGE
            } else {
                DARKLING_A0_HP_RANGE
            }
        }
        DAGGER_ID => DAGGER_HP_RANGE,
        REPTOMANCER_ID => {
            if ascension >= 8 {
                REPTOMANCER_A8_HP_RANGE
            } else {
                REPTOMANCER_A0_HP_RANGE
            }
        }
        TORCH_HEAD_ID => {
            if ascension >= 9 {
                TORCH_HEAD_A9_HP_RANGE
            } else {
                TORCH_HEAD_A0_HP_RANGE
            }
        }
        ORB_WALKER_ID => {
            if ascension >= 7 {
                ORB_WALKER_A7_HP_RANGE
            } else {
                ORB_WALKER_A0_HP_RANGE
            }
        }
        SPIKER_ID => {
            if ascension >= 7 {
                SPIKER_A7_HP_RANGE
            } else {
                SPIKER_A0_HP_RANGE
            }
        }
        REPULSOR_ID => {
            if ascension >= 7 {
                REPULSOR_A7_HP_RANGE
            } else {
                REPULSOR_A0_HP_RANGE
            }
        }
        BRONZE_ORB_ID => {
            if ascension >= 9 {
                BRONZE_ORB_A9_HP_RANGE
            } else {
                BRONZE_ORB_A0_HP_RANGE
            }
        }
        BRONZE_AUTOMATON_ID => {
            if ascension >= 9 {
                MonsterHpRange::new(320, 320)
            } else {
                MonsterHpRange::new(300, 300)
            }
        }
        THE_COLLECTOR_ID => {
            if ascension >= 9 {
                MonsterHpRange::new(300, 300)
            } else {
                MonsterHpRange::new(282, 282)
            }
        }
        _ => return None,
    };
    Some(range)
}

#[must_use]
pub fn target_city_monster_profile(
    monster_name: &str,
    ascension: u8,
) -> Option<TargetCityMonsterProfile> {
    let hp_range = target_city_monster_hp_range(monster_name, ascension)?;
    let constant = |name, value| TargetMonsterConstant { name, value };
    let constants = match monster_name {
        "Byrd" => vec![
            constant("peck_damage", 1),
            constant("peck_hits", if ascension >= 2 { 6 } else { 5 }),
            constant("swoop_damage", if ascension >= 2 { 14 } else { 12 }),
            constant("headbutt_damage", 3),
            constant("caw_strength", 1),
            constant("flight_amount", if ascension >= 17 { 4 } else { 3 }),
        ],
        "Chosen" => vec![
            constant("zap_damage", if ascension >= 2 { 21 } else { 18 }),
            constant("debilitate_damage", if ascension >= 2 { 12 } else { 10 }),
            constant("poke_damage", if ascension >= 2 { 6 } else { 5 }),
            constant("debilitate_vulnerable", 2),
            constant("drain_strength", 3),
            constant("drain_weak", 3),
            constant("hex_amount", 1),
        ],
        "ShelledParasite" | "Shell Parasite" | "Shelled Parasite" => vec![
            constant("plated_armor", 14),
            constant("starting_block", 14),
            constant("fell_damage", if ascension >= 2 { 21 } else { 18 }),
            constant("double_strike_damage", if ascension >= 2 { 7 } else { 6 }),
            constant("double_strike_hits", 2),
            constant("suck_damage", if ascension >= 2 { 12 } else { 10 }),
            constant("fell_frail", 2),
        ],
        "SphericGuardian" | "Spheric Guardian" => vec![
            constant("damage", if ascension >= 2 { 11 } else { 10 }),
            constant("slam_hits", 2),
            constant("harden_block", 15),
            constant("frail", 5),
            constant("activate_block", if ascension >= 17 { 35 } else { 25 }),
            constant("artifact", 3),
            constant("starting_block", 40),
        ],
        "Mugger" => vec![
            constant("swipe_damage", if ascension >= 2 { 11 } else { 10 }),
            constant("big_swipe_damage", if ascension >= 2 { 18 } else { 16 }),
            constant("theft", if ascension >= 17 { 20 } else { 15 }),
            constant("escape_block", 11),
        ],
        "SnakePlant" | "Snake Plant" => vec![
            constant("chompy_damage", if ascension >= 2 { 8 } else { 7 }),
            constant("chompy_hits", 3),
        ],
        "Snecko" => vec![
            constant("bite_damage", if ascension >= 2 { 18 } else { 15 }),
            constant("tail_damage", if ascension >= 2 { 10 } else { 8 }),
            constant("vulnerable", 2),
        ],
        "Centurion" => vec![
            constant("slash_damage", if ascension >= 2 { 14 } else { 12 }),
            constant("fury_damage", if ascension >= 2 { 7 } else { 6 }),
            constant("fury_hits", 3),
            constant("block", if ascension >= 17 { 20 } else { 15 }),
        ],
        "Healer" => vec![
            constant("magic_damage", if ascension >= 2 { 9 } else { 8 }),
            constant("heal", if ascension >= 17 { 20 } else { 16 }),
            constant(
                "strength",
                if ascension >= 17 {
                    4
                } else if ascension >= 2 {
                    3
                } else {
                    2
                },
            ),
        ],
        "BookOfStabbing" | "Book of Stabbing" => vec![
            constant("stab_damage", if ascension >= 3 { 7 } else { 6 }),
            constant("big_stab_damage", if ascension >= 3 { 24 } else { 21 }),
            constant("painful_stabs", 1),
        ],
        "GremlinLeader" | "Gremlin Leader" => vec![
            constant("stab_damage", 6),
            constant("stab_hits", 3),
            constant(
                "strength",
                if ascension >= 18 {
                    5
                } else if ascension >= 3 {
                    4
                } else {
                    3
                },
            ),
            constant("block", if ascension >= 18 { 10 } else { 6 }),
        ],
        "Taskmaster" => vec![
            constant("whip_damage", 4),
            constant("scouring_whip_damage", 7),
            constant(
                "wounds",
                if ascension >= 18 {
                    3
                } else if ascension >= 3 {
                    2
                } else {
                    1
                },
            ),
        ],
        "FungiBeast" | "Fungi Beast" => vec![
            constant("bite_damage", 6),
            constant(
                "grow_strength",
                if ascension >= 17 {
                    5
                } else if ascension >= 2 {
                    4
                } else {
                    3
                },
            ),
            constant("spore_cloud", 2),
        ],
        "SlaverBlue" | "Blue Slaver" => vec![
            constant("stab_damage", if ascension >= 2 { 13 } else { 12 }),
            constant("rake_damage", if ascension >= 2 { 8 } else { 7 }),
            constant("weak", if ascension >= 17 { 2 } else { 1 }),
        ],
        "SlaverRed" | "Red Slaver" => vec![
            constant("stab_damage", if ascension >= 2 { 14 } else { 13 }),
            constant("scrape_damage", if ascension >= 2 { 9 } else { 8 }),
            constant("vulnerable", if ascension >= 17 { 2 } else { 1 }),
            constant("entangled", 1),
        ],
        "random gremlin" | "GremlinWarrior" | "Gremlin Warrior" => vec![
            constant("scratch_damage", if ascension >= 2 { 5 } else { 4 }),
            constant("anger", if ascension >= 17 { 2 } else { 1 }),
            constant("minion", 1),
        ],
        "GremlinThief" | "Gremlin Thief" => vec![
            constant("puncture_damage", if ascension >= 2 { 10 } else { 9 }),
            constant("minion", 1),
        ],
        "GremlinFat" | "Gremlin Fat" => vec![
            constant("blunt_damage", if ascension >= 2 { 5 } else { 4 }),
            constant("weak", 1),
            constant("frail", if ascension >= 17 { 1 } else { 0 }),
            constant("minion", 1),
        ],
        "GremlinTsundere" | "Gremlin Tsundere" => vec![
            constant(
                "block",
                if ascension >= 17 {
                    11
                } else if ascension >= 2 {
                    8
                } else {
                    7
                },
            ),
            constant("bash_damage", if ascension >= 2 { 8 } else { 6 }),
            constant("minion", 1),
        ],
        "GremlinWizard" | "Gremlin Wizard" => vec![
            constant("magic_damage", if ascension >= 2 { 30 } else { 25 }),
            constant("charge_limit", 3),
            constant("minion", 1),
        ],
        _ => return None,
    };
    Some(TargetCityMonsterProfile {
        monster_name: match monster_name {
            "Byrd" => "Byrd",
            "Chosen" => "Chosen",
            "Shell Parasite" | "Shelled Parasite" => "ShelledParasite",
            "ShelledParasite" => "ShelledParasite",
            "Spheric Guardian" => "SphericGuardian",
            "SphericGuardian" => "SphericGuardian",
            "Mugger" => "Mugger",
            "Snake Plant" => "SnakePlant",
            "SnakePlant" => "SnakePlant",
            "Snecko" => "Snecko",
            "Centurion" => "Centurion",
            "Healer" => "Healer",
            "Book of Stabbing" => "BookOfStabbing",
            "BookOfStabbing" => "BookOfStabbing",
            "Gremlin Leader" => "GremlinLeader",
            "GremlinLeader" => "GremlinLeader",
            "Taskmaster" => "Taskmaster",
            "Fungi Beast" => "FungiBeast",
            "FungiBeast" => "FungiBeast",
            "Blue Slaver" => "SlaverBlue",
            "SlaverBlue" => "SlaverBlue",
            "Red Slaver" => "SlaverRed",
            "SlaverRed" => "SlaverRed",
            "random gremlin" | "Gremlin Warrior" => "GremlinWarrior",
            "GremlinWarrior" => "GremlinWarrior",
            "Gremlin Thief" => "GremlinThief",
            "GremlinThief" => "GremlinThief",
            "Gremlin Fat" => "GremlinFat",
            "GremlinFat" => "GremlinFat",
            "Gremlin Tsundere" => "GremlinTsundere",
            "GremlinTsundere" => "GremlinTsundere",
            "Gremlin Wizard" => "GremlinWizard",
            "GremlinWizard" => "GremlinWizard",
            _ => return None,
        },
        hp_range,
        constants,
    })
}

#[must_use]
pub fn target_encounter_spawn_for_key(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    Some(match encounter_key {
        "Cultist" => {
            let max_hp = target_cultist_hp_roll(seed, floor_num, ascension);
            vec![target_combat_entry_spawn(
                "Cultist",
                max_hp,
                neow_lament,
                Vec::new(),
            )]
        }
        "Jaw Worm" => {
            let max_hp = target_jaw_worm_hp_roll(seed, floor_num, ascension);
            vec![target_combat_entry_spawn(
                "Jaw Worm",
                max_hp,
                neow_lament,
                Vec::new(),
            )]
        }
        "Small Slimes" => {
            target_small_slimes_spawn_states(seed, floor_num, ascension, neow_lament)?
        }
        "Large Slime" => {
            let roll = target_large_slime_hp_roll(seed, floor_num, ascension);
            let mut spawn = target_combat_entry_spawn(roll.name, roll.hp, neow_lament, Vec::new());
            spawn.rolled_attack_damage = match roll.name {
                "Acid Slime (L)" => Some(if ascension >= 2 {
                    ACID_SLIME_L_A2_WOUND_TACKLE_DAMAGE
                } else {
                    ACID_SLIME_L_WOUND_TACKLE_DAMAGE
                }),
                "Spike Slime (L)" => Some(if ascension >= 2 { 18 } else { 16 }),
                _ => None,
            };
            vec![spawn]
        }
        "2 Louse" => target_two_louse_spawn_states(seed, floor_num, ascension, neow_lament),
        "3 Louse" => target_three_louse_spawn_states(seed, floor_num, ascension, neow_lament),
        "2 Fungi Beasts" => {
            target_two_fungi_beasts_spawn_states(seed, floor_num, ascension, neow_lament)
        }
        "Exordium Thugs" => {
            target_exordium_thugs_spawn_states(seed, floor_num, ascension, neow_lament)
        }
        "Exordium Wildlife" => {
            target_exordium_wildlife_spawn_states(seed, floor_num, ascension, neow_lament)
        }
        "Blue Slaver" => {
            let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
            let max_hp = target_slaver_hp_range(ascension).roll(&mut hp_rng);
            vec![target_combat_entry_spawn(
                "SlaverBlue",
                max_hp,
                neow_lament,
                Vec::new(),
            )]
        }
        "Red Slaver" => {
            let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
            let max_hp = target_slaver_hp_range(ascension).roll(&mut hp_rng);
            vec![target_combat_entry_spawn(
                "SlaverRed",
                max_hp,
                neow_lament,
                Vec::new(),
            )]
        }
        "Gremlin Gang" => target_gremlin_gang_spawn_states(seed, floor_num, ascension, neow_lament),
        "Lots of Slimes" => {
            target_lots_of_slimes_spawn_states(seed, floor_num, ascension, neow_lament)
        }
        "Looter" => {
            let max_hp = target_looter_hp_roll(seed, floor_num, ascension);
            vec![target_combat_entry_spawn(
                "Looter",
                max_hp,
                neow_lament,
                vec![TargetSpawnPower {
                    id: "Thievery",
                    amount: looter_theft(ascension),
                }],
            )]
        }
        "GremlinNob" => {
            let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
            let max_hp = target_gremlin_nob_hp_range(ascension).roll(&mut hp_rng);
            vec![target_combat_entry_spawn(
                "GremlinNob",
                max_hp,
                neow_lament,
                Vec::new(),
            )]
        }
        "Lagavulin" => {
            let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
            let max_hp = target_lagavulin_hp_range(ascension).roll(&mut hp_rng);
            let mut spawn = target_combat_entry_spawn("Lagavulin", max_hp, neow_lament, Vec::new());
            spawn.block = 8;
            vec![spawn]
        }
        "3 Sentries" => target_three_sentries_spawn_states(seed, floor_num, ascension, neow_lament),
        _ => return None,
    })
}

fn target_two_fungi_beasts_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    (0..2)
        .filter_map(|_| {
            target_city_member_spawn(
                "FungiBeast",
                &mut hp_rng,
                &mut misc_rng,
                ascension,
                neow_lament,
            )
        })
        .collect()
}

fn target_exordium_thugs_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let seed = seed_for_floor(seed, floor_num);
    let mut misc_rng = StsRng::new(seed);
    let mut hp_rng = StsRng::new(seed);

    let louse_is_normal = misc_rng.random_bool();
    let weak_index = misc_rng.random_int_range(0, 2);
    let louse_hp_range = if louse_is_normal {
        target_louse_normal_hp_range(ascension)
    } else {
        target_louse_defensive_hp_range(ascension)
    };
    let louse_hp = louse_hp_range.roll(&mut hp_rng);
    let louse_bite_damage = target_louse_bite_damage_range(ascension).roll(&mut hp_rng);
    let spike_hp = target_spike_slime_m_hp_range(ascension).roll(&mut hp_rng);
    let acid_hp = target_acid_slime_m_hp_range(ascension).roll(&mut hp_rng);

    let slaver_is_red = misc_rng.random_bool();
    let strong_index = misc_rng.random_int_range(0, 2);
    let cultist_hp = target_cultist_hp_range(ascension).roll(&mut hp_rng);
    let slaver_hp = target_slaver_hp_range(ascension).roll(&mut hp_rng);
    let looter_hp = target_looter_hp_range(ascension).roll(&mut hp_rng);

    vec![
        match weak_index {
            0 => {
                let name = if louse_is_normal {
                    "LouseNormal"
                } else {
                    "LouseDefensive"
                };
                let louse_curl_up = target_louse_curl_up_range(ascension).roll(&mut hp_rng);
                let mut spawn = target_combat_entry_spawn(
                    name,
                    louse_hp,
                    neow_lament,
                    vec![TargetSpawnPower {
                        id: "Curl Up",
                        amount: louse_curl_up,
                    }],
                );
                spawn.rolled_attack_damage = Some(louse_bite_damage);
                spawn
            }
            1 => {
                let mut spawn =
                    target_combat_entry_spawn("Spike Slime (M)", spike_hp, neow_lament, Vec::new());
                spawn.intent = TargetSpawnIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 };
                spawn
            }
            _ => {
                let mut spawn =
                    target_combat_entry_spawn("Acid Slime (M)", acid_hp, neow_lament, Vec::new());
                let damage = if ascension >= 2 {
                    8
                } else {
                    ACID_SLIME_ATTACK_DAMAGE
                };
                spawn.intent = TargetSpawnIntent::AttackAddSlimedToDiscard { damage, count: 1 };
                spawn.rolled_attack_damage = Some(damage);
                spawn
            }
        },
        match strong_index {
            0 => target_combat_entry_spawn("Cultist", cultist_hp, neow_lament, Vec::new()),
            1 => {
                let name = if slaver_is_red {
                    "SlaverRed"
                } else {
                    "SlaverBlue"
                };
                let mut spawn = target_combat_entry_spawn(name, slaver_hp, neow_lament, Vec::new());
                if slaver_is_red {
                    let damage = if ascension >= 2 {
                        SLAVER_RED_A2_STAB_DAMAGE
                    } else {
                        SLAVER_RED_STAB_DAMAGE
                    };
                    spawn.intent = TargetSpawnIntent::Attack { damage };
                    spawn.rolled_attack_damage = Some(damage);
                }
                spawn
            }
            _ => target_combat_entry_spawn(
                "Looter",
                looter_hp,
                neow_lament,
                vec![TargetSpawnPower {
                    id: "Thievery",
                    amount: looter_theft(ascension),
                }],
            ),
        },
    ]
}

fn target_exordium_wildlife_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let seed = seed_for_floor(seed, floor_num);
    let mut misc_rng = StsRng::new(seed);
    let mut hp_rng = StsRng::new(seed);

    let fungi_hp = if ascension >= 7 {
        FUNGI_BEAST_A7_HP_RANGE
    } else {
        FUNGI_BEAST_A0_HP_RANGE
    }
    .roll(&mut hp_rng);
    let jaw_worm_hp = target_jaw_worm_hp_range(ascension).roll(&mut hp_rng);
    let strong_index = misc_rng.random_int_range(0, 1);

    let louse_is_normal = misc_rng.random_bool();
    let louse_hp_range = if louse_is_normal {
        target_louse_normal_hp_range(ascension)
    } else {
        target_louse_defensive_hp_range(ascension)
    };
    let louse_hp = louse_hp_range.roll(&mut hp_rng);
    let louse_bite_damage = target_louse_bite_damage_range(ascension).roll(&mut hp_rng);
    let spike_hp = target_spike_slime_m_hp_range(ascension).roll(&mut hp_rng);
    let acid_hp = target_acid_slime_m_hp_range(ascension).roll(&mut hp_rng);
    let weak_index = misc_rng.random_int_range(0, 2);

    vec![
        match strong_index {
            0 => target_combat_entry_spawn(
                "FungiBeast",
                fungi_hp,
                neow_lament,
                vec![TargetSpawnPower {
                    id: "Spore Cloud",
                    amount: FUNGI_BEAST_SPORE_CLOUD,
                }],
            ),
            _ => target_combat_entry_spawn("Jaw Worm", jaw_worm_hp, neow_lament, Vec::new()),
        },
        match weak_index {
            0 => {
                let name = if louse_is_normal {
                    "LouseNormal"
                } else {
                    "LouseDefensive"
                };
                let louse_curl_up = target_louse_curl_up_range(ascension).roll(&mut hp_rng);
                let mut spawn = target_combat_entry_spawn(
                    name,
                    louse_hp,
                    neow_lament,
                    vec![TargetSpawnPower {
                        id: "Curl Up",
                        amount: louse_curl_up,
                    }],
                );
                spawn.rolled_attack_damage = Some(louse_bite_damage);
                spawn
            }
            1 => {
                let mut spawn =
                    target_combat_entry_spawn("Spike Slime (M)", spike_hp, neow_lament, Vec::new());
                spawn.intent = TargetSpawnIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 };
                spawn
            }
            _ => {
                let mut spawn =
                    target_combat_entry_spawn("Acid Slime (M)", acid_hp, neow_lament, Vec::new());
                let damage = if ascension >= 2 {
                    8
                } else {
                    ACID_SLIME_ATTACK_DAMAGE
                };
                spawn.intent = TargetSpawnIntent::AttackAddSlimedToDiscard { damage, count: 1 };
                spawn.rolled_attack_damage = Some(damage);
                spawn
            }
        },
    ]
}

fn target_slaver_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 7 {
        SLAVER_A7_HP_RANGE
    } else {
        SLAVER_A0_HP_RANGE
    }
}

fn target_gremlin_gang_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut pool = vec![
        "GremlinWarrior",
        "GremlinWarrior",
        "GremlinThief",
        "GremlinThief",
        "GremlinFat",
        "GremlinFat",
        "GremlinTsundere",
        "GremlinWizard",
    ];

    (0..4)
        .filter_map(|_| {
            let index = misc_rng.random_int(pool.len() as i32 - 1) as usize;
            let name = pool.remove(index);
            target_gremlin_gang_member_spawn(name, &mut hp_rng, ascension, neow_lament)
        })
        .collect()
}

fn target_gremlin_gang_member_spawn(
    name: &'static str,
    hp_rng: &mut StsRng,
    ascension: u8,
    neow_lament: bool,
) -> Option<TargetEncounterSpawn> {
    let hp_range = target_city_monster_hp_range(name, ascension)?;
    let max_hp = hp_range.roll(hp_rng);
    let mut spawn = target_combat_entry_spawn(name, max_hp, neow_lament, Vec::new());
    if name == "GremlinWarrior" {
        spawn.powers.push(TargetSpawnPower {
            id: "Angry",
            amount: gremlin_warrior_anger(ascension),
        });
    }
    Some(spawn)
}

fn target_lots_of_slimes_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut misc_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    let mut pool = vec![
        "Spike Slime (S)",
        "Spike Slime (S)",
        "Spike Slime (S)",
        "Acid Slime (S)",
        "Acid Slime (S)",
    ];

    (0..5)
        .map(|_| {
            let index = misc_rng.random_int(pool.len() as i32 - 1) as usize;
            let name = pool.remove(index);
            let max_hp = match name {
                "Spike Slime (S)" => target_spike_slime_s_hp_range(ascension).roll(&mut hp_rng),
                "Acid Slime (S)" => target_acid_slime_s_hp_range(ascension).roll(&mut hp_rng),
                _ => unreachable!("Lots of Slimes pool only contains small slimes"),
            };
            target_combat_entry_spawn(name, max_hp, neow_lament, Vec::new())
        })
        .collect()
}

fn target_three_sentries_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed_for_floor(seed, floor_num));
    (0..3)
        .map(|index| {
            let max_hp = target_sentry_hp_range(ascension).roll(&mut hp_rng);
            let mut spawn = target_combat_entry_spawn(
                "Sentry",
                max_hp,
                neow_lament,
                vec![TargetSpawnPower {
                    id: "Artifact",
                    amount: SENTRY_ARTIFACT,
                }],
            );
            if index % 2 == 1 {
                let damage = target_sentry_attack_damage(ascension);
                spawn.intent = TargetSpawnIntent::Attack { damage };
                spawn.rolled_attack_damage = Some(damage);
            }
            spawn
        })
        .collect()
}

fn target_small_slimes_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let rolls = target_small_slimes_hp_rolls(seed, floor_num, ascension)?;
    Some(
        rolls
            .into_iter()
            .map(|roll| target_combat_entry_spawn(roll.name, roll.hp, neow_lament, Vec::new()))
            .collect(),
    )
}

fn target_combat_entry_spawn(
    name: &'static str,
    max_hp: i32,
    neow_lament: bool,
    powers: Vec<TargetSpawnPower>,
) -> TargetEncounterSpawn {
    TargetEncounterSpawn {
        name,
        current_hp: if neow_lament { 1 } else { max_hp },
        max_hp,
        block: 0,
        intent: TargetSpawnIntent::PendingAiRoll,
        powers,
        rolled_attack_damage: None,
    }
}

fn target_louse_kind(rng: &mut StsRng) -> LouseKind {
    if rng.random_bool() {
        LouseKind::Normal
    } else {
        LouseKind::Defensive
    }
}

#[must_use]
pub fn content_id_from_game_monster_id(game_id: &str) -> Option<ContentId> {
    Some(match game_id {
        "Cultist" => CULTIST_ID,
        "JawWorm" | "Jaw Worm" => JAW_WORM_ID,
        "GremlinNob" => GREMLIN_NOB_ID,
        "Lagavulin" => LAGAVULIN_ID,
        "Sentry" => SENTRY_ID,
        "Hexaghost" => HEXAGHOST_ID,
        "SlimeBoss" => SLIME_BOSS_ID,
        "TheGuardian" => GUARDIAN_ID,
        "Looter" => LOOTER_ID,
        "SphericGuardian" | "Spheric Guardian" => SPHERIC_GUARDIAN_ID,
        "Mugger" => MUGGER_ID,
        "Chosen" => CHOSEN_ID,
        "SnakePlant" | "Snake Plant" => SNAKE_PLANT_ID,
        "Snecko" => SNECKO_ID,
        "Centurion" => CENTURION_ID,
        "Healer" => HEALER_ID,
        "Byrd" => BYRD_ID,
        "ShelledParasite" | "Shell Parasite" | "Shelled Parasite" => SHELLED_PARASITE_ID,
        "BookOfStabbing" | "Book of Stabbing" => BOOK_OF_STABBING_ID,
        "SlaverBoss" | "Taskmaster" => TASKMASTER_ID,
        "GremlinLeader" | "Gremlin Leader" => GREMLIN_LEADER_ID,
        "FungiBeast" | "Fungi Beast" => FUNGI_BEAST_ID,
        "SlaverBlue" | "Blue Slaver" => SLAVER_BLUE_ID,
        "SlaverRed" | "Red Slaver" => SLAVER_RED_ID,
        "GremlinWarrior" | "Gremlin Warrior" => GREMLIN_WARRIOR_ID,
        "GremlinThief" | "Gremlin Thief" => GREMLIN_THIEF_ID,
        "GremlinFat" | "Gremlin Fat" => GREMLIN_FAT_ID,
        "GremlinTsundere" | "Gremlin Tsundere" => GREMLIN_TSUNDERE_ID,
        "GremlinWizard" | "Gremlin Wizard" => GREMLIN_WIZARD_ID,
        "Automaton" | "BronzeAutomaton" | "Bronze Automaton" => BRONZE_AUTOMATON_ID,
        "BronzeOrb" | "Bronze Orb" | "Orb" => BRONZE_ORB_ID,
        "TheCollector" | "The Collector" | "Collector" => THE_COLLECTOR_ID,
        "TorchHead" | "Torch Head" => TORCH_HEAD_ID,
        "Orb Walker" | "OrbWalker" => ORB_WALKER_ID,
        "Darkling" => DARKLING_ID,
        "BanditBear" | "Bear" => BANDIT_BEAR_ID,
        "BanditPointy" | "BanditChild" | "Pointy" => BANDIT_POINTY_ID,
        "BanditLeader" | "Romeo" => BANDIT_LEADER_ID,
        "Champ" | "TheChamp" | "The Champ" => CHAMP_ID,
        "AwakenedOne" | "Awakened One" => AWAKENED_ONE_ID,
        "Dagger" => DAGGER_ID,
        "Deca" => DECA_ID,
        "Donu" => DONU_ID,
        "Exploder" => EXPLODER_ID,
        "GiantHead" | "Giant Head" => GIANT_HEAD_ID,
        "Nemesis" => NEMESIS_ID,
        "Reptomancer" => REPTOMANCER_ID,
        "Repulsor" => REPULSOR_ID,
        "Spiker" => SPIKER_ID,
        "SpireGrowth" | "Spire Growth" | "Serpent" => SPIRE_GROWTH_ID,
        "Maw" | "TheMaw" | "The Maw" => MAW_ID,
        "TimeEater" | "Time Eater" => TIME_EATER_ID,
        "Transient" => TRANSIENT_ID,
        "WrithingMass" | "Writhing Mass" => WRITHING_MASS_ID,
        "CorruptHeart" | "Corrupt Heart" => CORRUPT_HEART_ID,
        "SpireShield" | "Spire Shield" => SPIRE_SHIELD_ID,
        "SpireSpear" | "Spire Spear" => SPIRE_SPEAR_ID,
        "SpikeSlime_S" | "SpikeSlime_M" | "SpikeSlime_L" | "Spike Slime (S)"
        | "Spike Slime (M)" | "Spike Slime (L)" => SPIKE_SLIME_ID,
        "AcidSlime_S" | "AcidSlime_M" | "AcidSlime_L" | "Acid Slime (S)" | "Acid Slime (M)"
        | "Acid Slime (L)" => ACID_SLIME_ID,
        "FuzzyLouseDefensive" | "LouseDefensive" => GREEN_LOUSE_ID,
        "FuzzyLouseNormal" | "LouseNormal" => RED_LOUSE_ID,
        _ => return None,
    })
}

#[must_use]
pub fn get_monster_definition(content_id: ContentId) -> Option<&'static MonsterDefinition> {
    match content_id {
        FIXED_SIMPLE_MONSTER_ID => Some(&FIXED_SIMPLE_MONSTER),
        CULTIST_ID => Some(&CULTIST_A0),
        JAW_WORM_ID => Some(&JAW_WORM_A0),
        GREMLIN_NOB_ID => Some(&GREMLIN_NOB_A0),
        RED_LOUSE_ID => Some(&RED_LOUSE_A0),
        GREEN_LOUSE_ID => Some(&GREEN_LOUSE_A0),
        SPIKE_SLIME_ID => Some(&SPIKE_SLIME_A0),
        ACID_SLIME_ID => Some(&ACID_SLIME_A0),
        LAGAVULIN_ID => Some(&LAGAVULIN_A0),
        SENTRY_ID => Some(&SENTRY_A0),
        HEXAGHOST_ID => Some(&HEXAGHOST_A0),
        SLIME_BOSS_ID => Some(&SLIME_BOSS_A0),
        GUARDIAN_ID => Some(&GUARDIAN_A0),
        LOOTER_ID => Some(&LOOTER_A0),
        SPHERIC_GUARDIAN_ID => Some(&SPHERIC_GUARDIAN_A0),
        MUGGER_ID => Some(&MUGGER_A0),
        CHOSEN_ID => Some(&CHOSEN_A0),
        SNAKE_PLANT_ID => Some(&SNAKE_PLANT_A0),
        SNECKO_ID => Some(&SNECKO_A0),
        CENTURION_ID => Some(&CENTURION_A0),
        HEALER_ID => Some(&HEALER_A0),
        BYRD_ID => Some(&BYRD_A0),
        SHELLED_PARASITE_ID => Some(&SHELLED_PARASITE_A0),
        BOOK_OF_STABBING_ID => Some(&BOOK_OF_STABBING_A0),
        TASKMASTER_ID => Some(&TASKMASTER_A0),
        GREMLIN_LEADER_ID => Some(&GREMLIN_LEADER_A0),
        FUNGI_BEAST_ID => Some(&FUNGI_BEAST_A0),
        SLAVER_BLUE_ID => Some(&SLAVER_BLUE_A0),
        SLAVER_RED_ID => Some(&SLAVER_RED_A0),
        GREMLIN_WARRIOR_ID => Some(&GREMLIN_WARRIOR_A0),
        GREMLIN_THIEF_ID => Some(&GREMLIN_THIEF_A0),
        GREMLIN_FAT_ID => Some(&GREMLIN_FAT_A0),
        GREMLIN_TSUNDERE_ID => Some(&GREMLIN_TSUNDERE_A0),
        GREMLIN_WIZARD_ID => Some(&GREMLIN_WIZARD_A0),
        BRONZE_AUTOMATON_ID => Some(&BRONZE_AUTOMATON_A0),
        BRONZE_ORB_ID => Some(&BRONZE_ORB_A0),
        THE_COLLECTOR_ID => Some(&THE_COLLECTOR_A0),
        TORCH_HEAD_ID => Some(&TORCH_HEAD_A0),
        ORB_WALKER_ID => Some(&ORB_WALKER_A0),
        DARKLING_ID => Some(&DARKLING_A0),
        BANDIT_BEAR_ID => Some(&BANDIT_BEAR_A0),
        BANDIT_POINTY_ID => Some(&BANDIT_POINTY_A0),
        BANDIT_LEADER_ID => Some(&BANDIT_LEADER_A0),
        CHAMP_ID => Some(&CHAMP_A0),
        AWAKENED_ONE_ID => Some(&AWAKENED_ONE_A0),
        DAGGER_ID => Some(&DAGGER_A0),
        DECA_ID => Some(&DECA_A0),
        DONU_ID => Some(&DONU_A0),
        EXPLODER_ID => Some(&EXPLODER_A0),
        GIANT_HEAD_ID => Some(&GIANT_HEAD_A0),
        NEMESIS_ID => Some(&NEMESIS_A0),
        REPTOMANCER_ID => Some(&REPTOMANCER_A0),
        REPULSOR_ID => Some(&REPULSOR_A0),
        SPIKER_ID => Some(&SPIKER_A0),
        SPIRE_GROWTH_ID => Some(&SPIRE_GROWTH_A0),
        MAW_ID => Some(&MAW_A0),
        TIME_EATER_ID => Some(&TIME_EATER_A0),
        TRANSIENT_ID => Some(&TRANSIENT_A0),
        WRITHING_MASS_ID => Some(&WRITHING_MASS_A0),
        CORRUPT_HEART_ID => Some(&CORRUPT_HEART_A0),
        SPIRE_SHIELD_ID => Some(&SPIRE_SHIELD_A0),
        SPIRE_SPEAR_ID => Some(&SPIRE_SPEAR_A0),
        _ => None,
    }
}

#[must_use]
pub fn is_gremlin_leader_minion_content_id(content_id: ContentId) -> bool {
    matches!(
        content_id,
        GREMLIN_WARRIOR_ID
            | GREMLIN_THIEF_ID
            | GREMLIN_FAT_ID
            | GREMLIN_TSUNDERE_ID
            | GREMLIN_WIZARD_ID
            | BRONZE_ORB_ID
    )
}

#[must_use]
pub fn monster_state(definition: &MonsterDefinition, id: MonsterId) -> MonsterState {
    monster_state_for_ascension(definition, id, 0)
}

#[must_use]
pub fn monster_state_for_ascension(
    definition: &MonsterDefinition,
    id: MonsterId,
    ascension: u8,
) -> MonsterState {
    let config = AscensionConfig::new(ascension);
    let max_hp = if definition.content_id == SPHERIC_GUARDIAN_ID {
        definition.hp
    } else if definition.content_id == SPIRE_GROWTH_ID {
        spire_growth_max_hp(ascension)
    } else if definition.content_id == GIANT_HEAD_ID {
        giant_head_max_hp(ascension)
    } else if definition.content_id == NEMESIS_ID {
        nemesis_max_hp(ascension)
    } else {
        config.scaled_enemy_hp(definition.hp)
    };
    MonsterState {
        id,
        hp: max_hp,
        max_hp,
        block: if definition.content_id == SPHERIC_GUARDIAN_ID {
            SPHERIC_GUARDIAN_STARTING_BLOCK
        } else if definition.content_id == SHELLED_PARASITE_ID {
            SHELLED_PARASITE_PLATED_ARMOR
        } else {
            0
        },
        alive: true,
        escaped: false,
        powers: MonsterPowers {
            spikes: if definition.content_id == SPIKER_ID {
                spiker_starting_thorns(ascension)
            } else {
                definition.starting_spikes
            },
            artifact: match definition.content_id {
                SENTRY_ID => SENTRY_ARTIFACT,
                SPHERIC_GUARDIAN_ID => SPHERIC_GUARDIAN_ARTIFACT,
                BRONZE_AUTOMATON_ID => BRONZE_AUTOMATON_ARTIFACT,
                _ => 0,
            },
            flight: if definition.content_id == BYRD_ID {
                byrd_flight(ascension)
            } else {
                0
            },
            plated_armor: if definition.content_id == SHELLED_PARASITE_ID {
                SHELLED_PARASITE_PLATED_ARMOR
            } else {
                0
            },
            painful_stabs: if definition.content_id == BOOK_OF_STABBING_ID {
                BOOK_OF_STABBING_PAINFUL_STABS
            } else {
                0
            },
            book_stab_count: if definition.content_id == BOOK_OF_STABBING_ID {
                1
            } else {
                0
            },
            explosive: if definition.content_id == EXPLODER_ID {
                EXPLODER_EXPLOSIVE
            } else {
                0
            },
            malleable: match definition.content_id {
                SNAKE_PLANT_ID => SNAKE_PLANT_MALLEABLE,
                WRITHING_MASS_ID => WRITHING_MASS_MALLEABLE,
                _ => 0,
            },
            malleable_base: match definition.content_id {
                SNAKE_PLANT_ID => SNAKE_PLANT_MALLEABLE,
                WRITHING_MASS_ID => WRITHING_MASS_MALLEABLE,
                _ => 0,
            },
            spore_cloud: if definition.content_id == FUNGI_BEAST_ID {
                FUNGI_BEAST_SPORE_CLOUD
            } else {
                0
            },
            slow: if definition.content_id == GIANT_HEAD_ID {
                1
            } else {
                0
            },
            minion: if is_gremlin_leader_minion_content_id(definition.content_id) {
                1
            } else {
                0
            },
            anger: if definition.content_id == GREMLIN_WARRIOR_ID {
                gremlin_warrior_anger(ascension)
            } else {
                0
            },
            ..MonsterPowers::default()
        },
        temp_strength_down: 0,
        content_id: definition.content_id,
        slime_size: match definition.content_id {
            SPIKE_SLIME_ID | ACID_SLIME_ID => Some(SlimeSize::Small),
            _ => None,
        },
        // Awake Lagavulin Event opens on Siphon. The sleep-elite cycle is
        // Attack→Attack→Siphon keyed on moves_executed % 3 after natural/damage
        // wake (moves_executed starts at 0). Starting at 2 places the opening
        // intent on Siphon and preserves Attack→Attack→Siphon afterwards.
        moves_executed: lagavulin_opening_moves_executed(definition),
        sleep_turns_remaining: definition.starting_sleep_turns,
        has_siphoned: false,
        split_triggered: false,
        defensive_turns_remaining: definition.starting_defensive_turns,
        mode_shift: if definition.content_id == GUARDIAN_ID {
            GUARDIAN_MODE_SHIFT_START
        } else {
            0
        },
        mode_shift_threshold: if definition.content_id == GUARDIAN_ID {
            GUARDIAN_MODE_SHIFT_START
        } else {
            0
        },
        in_defensive_mode: false,
        rolled_attack_damage: None,
        stolen_gold: 0,
        move_history: Vec::new(),
        gremlin_leader_slot: None,
        stasis_card: None,
        initial_intent_locked: false,
        burns_upgraded: false,
        intent: prepare_monster_intent_for_monster(
            definition,
            lagavulin_opening_moves_executed(definition),
            ascension,
            definition.starting_sleep_turns,
            false,
            definition.starting_defensive_turns,
            false,
            if definition.content_id == GUARDIAN_ID {
                GUARDIAN_MODE_SHIFT_START
            } else {
                0
            },
            None,
        ),
    }
}

#[must_use]
fn lagavulin_opening_moves_executed(definition: &MonsterDefinition) -> u32 {
    if definition.content_id == LAGAVULIN_ID && definition.starting_sleep_turns == 0 {
        2
    } else {
        0
    }
}

#[must_use]
pub fn boss_monsters_for_ascension(
    definition: &MonsterDefinition,
    ascension: u8,
) -> Vec<MonsterState> {
    let mut monsters = vec![monster_state_for_ascension(
        definition,
        MonsterId::new(1),
        ascension,
    )];
    if AscensionConfig::new(ascension).double_boss() {
        monsters.push(monster_state_for_ascension(
            definition,
            MonsterId::new(2),
            ascension,
        ));
    }
    monsters
}

#[must_use]
pub fn donu_deca_boss_monsters_for_ascension(ascension: u8) -> Vec<MonsterState> {
    let mut deca = monster_state_for_ascension(&DECA_A0, MonsterId::new(1), ascension);
    let mut donu = monster_state_for_ascension(&DONU_A0, MonsterId::new(2), ascension);
    let max_hp = if ascension >= 9 { 265 } else { 250 };
    let artifact = if ascension >= 19 { 3 } else { 2 };
    deca.hp = max_hp;
    deca.max_hp = max_hp;
    deca.powers.artifact = artifact;
    donu.hp = max_hp;
    donu.max_hp = max_hp;
    donu.powers.artifact = artifact;
    vec![deca, donu]
}

pub fn prepare_monster_intent_for_ascension(
    monster: &MonsterState,
    ascension: u8,
) -> SimResult<MonsterIntent> {
    let definition = get_monster_definition(monster.content_id)
        .ok_or(SimError::UnknownContent(monster.content_id))?;
    if monster.content_id == TIME_EATER_ID {
        // The normal encounter path supplies Time Eater's AI roll. This
        // fallback is only used by callers that request a non-rolled intent;
        // the source's opening move is Ripple.
        return Ok(MonsterIntent::AttackAndBlock {
            damage: 0,
            block: TIME_EATER_RIPPLE_BLOCK,
        });
    }
    if is_unsupported_approximate_monster_intent(monster.content_id) {
        return Err(SimError::UnsupportedMechanic(monster.content_id));
    }
    if requires_rolled_attack_damage(monster.content_id) && monster.rolled_attack_damage.is_none() {
        return Err(SimError::InvalidState(
            "monster requires rolled attack damage",
        ));
    }
    if monster.content_id == TRANSIENT_ID {
        return Ok(MonsterIntent::Attack {
            damage: transient_attack_damage(monster.moves_executed, ascension)?,
        });
    }
    let mut intent = prepare_monster_intent_for_monster(
        definition,
        monster.moves_executed,
        ascension,
        monster.sleep_turns_remaining,
        monster.has_siphoned,
        monster.defensive_turns_remaining,
        monster.in_defensive_mode,
        monster.mode_shift,
        monster.rolled_attack_damage,
    );
    if matches!(intent, MonsterIntent::PendingAiRoll) {
        return Err(SimError::UnsupportedMechanic(monster.content_id));
    }
    if monster.content_id != TRANSIENT_ID {
        if let Some(damage) = monster.rolled_attack_damage {
            if let MonsterIntent::Attack {
                damage: ref mut attack,
            } = intent
            {
                *attack = damage;
            }
        }
    }
    if monster.content_id == ACID_SLIME_ID
        && monster.max_hp > ACID_SLIME_S_A7_HP_RANGE.max
        && matches!(intent, MonsterIntent::Attack { .. })
    {
        let MonsterIntent::Attack { damage } = intent else {
            unreachable!("matches! above guarantees Attack intent")
        };
        let count = if monster.max_hp > ACID_SLIME_M_A7_HP_RANGE.max {
            2
        } else {
            1
        };
        intent = MonsterIntent::AttackAddSlimedToDiscard { damage, count };
    }
    if monster.content_id == SPIKE_SLIME_ID
        && monster.hp > SPIKE_SLIME_S_A7_HP_RANGE.max
        && matches!(intent, MonsterIntent::Attack { .. })
    {
        let MonsterIntent::Attack { .. } = intent else {
            unreachable!("matches! above guarantees Attack intent")
        };
        let damage = if monster.max_hp > SPIKE_SLIME_M_A7_HP_RANGE.max {
            SPIKE_SLIME_L_SPIT_DAMAGE
        } else {
            SPIKE_SLIME_M_SPIT_DAMAGE
        };
        let count = if monster.max_hp > SPIKE_SLIME_M_A7_HP_RANGE.max {
            2
        } else {
            1
        };
        intent = MonsterIntent::AttackAddSlimedToDiscard { damage, count };
    }
    if monster.content_id == SPIKE_SLIME_ID
        && monster.hp > SPIKE_SLIME_S_A7_HP_RANGE.max
        && matches!(intent, MonsterIntent::ApplyPlayerWeak { .. })
    {
        intent = MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: spike_slime_frail_amount(monster.max_hp, ascension),
            weak: 0,
        };
    }
    Ok(intent)
}

#[must_use]
pub(crate) fn requires_rolled_attack_damage(content_id: ContentId) -> bool {
    matches!(content_id, RED_LOUSE_ID | GREEN_LOUSE_ID | DARKLING_ID)
}

#[must_use]
#[allow(clippy::too_many_arguments)]
fn prepare_monster_intent_for_monster(
    definition: &MonsterDefinition,
    moves_executed: u32,
    ascension: u8,
    sleep_turns_remaining: u32,
    _has_siphoned: bool,
    defensive_turns_remaining: u32,
    in_defensive_mode: bool,
    mode_shift: i32,
    rolled_attack_damage: Option<i32>,
) -> MonsterIntent {
    if definition.content_id == AWAKENED_ONE_ID {
        return if mode_shift == 0 {
            MonsterIntent::Attack {
                damage: AWAKENED_ONE_SLASH_DAMAGE,
            }
        } else {
            MonsterIntent::Attack {
                damage: AWAKENED_ONE_DARK_ECHO_DAMAGE,
            }
        };
    }
    if definition.content_id == LAGAVULIN_ID {
        return lagavulin_intent(sleep_turns_remaining, moves_executed, ascension);
    }
    if definition.content_id == GUARDIAN_ID {
        return guardian_intent(
            in_defensive_mode,
            defensive_turns_remaining,
            moves_executed,
            ascension,
        );
    }
    if definition.content_id == LOOTER_ID {
        return looter_intent(moves_executed, ascension);
    }
    if definition.content_id == MUGGER_ID {
        return mugger_intent(moves_executed, ascension);
    }
    if definition.content_id == SPHERIC_GUARDIAN_ID {
        return target_spheric_guardian_next_intent_from_roll(moves_executed, &[], ascension);
    }
    if definition.content_id == BOOK_OF_STABBING_ID {
        return book_of_stabbing_intent(moves_executed, ascension);
    }
    if definition.content_id == GREMLIN_NOB_ID {
        return gremlin_nob_intent(moves_executed, ascension);
    }
    if definition.content_id == SNECKO_ID {
        return snecko_intent(moves_executed, ascension);
    }
    if definition.content_id == CENTURION_ID {
        return centurion_intent(moves_executed, ascension);
    }
    if definition.content_id == HEALER_ID {
        return healer_intent(moves_executed, ascension);
    }
    if definition.content_id == CHOSEN_ID {
        return chosen_intent(moves_executed, ascension);
    }
    if definition.content_id == SNAKE_PLANT_ID {
        return snake_plant_intent(moves_executed, ascension);
    }
    if definition.content_id == BYRD_ID {
        return byrd_intent(moves_executed, ascension);
    }
    if definition.content_id == SHELLED_PARASITE_ID {
        return shelled_parasite_intent(moves_executed, ascension);
    }
    if definition.content_id == FUNGI_BEAST_ID {
        return fungi_beast_intent(moves_executed, ascension);
    }
    if definition.content_id == SLAVER_BLUE_ID {
        return slaver_blue_intent(moves_executed, ascension);
    }
    if definition.content_id == SLAVER_RED_ID {
        return slaver_red_intent(moves_executed, ascension);
    }
    if definition.content_id == GREMLIN_LEADER_ID {
        return gremlin_leader_intent(moves_executed, ascension);
    }
    if definition.content_id == GREMLIN_NOB_ID {
        return gremlin_nob_intent(moves_executed, ascension);
    }
    if definition.content_id == SENTRY_ID {
        return sentry_intent(moves_executed, ascension);
    }
    if definition.content_id == BRONZE_AUTOMATON_ID {
        return bronze_automaton_intent(moves_executed, ascension);
    }
    if definition.content_id == BRONZE_ORB_ID {
        return bronze_orb_intent(moves_executed);
    }
    if definition.content_id == ORB_WALKER_ID {
        return orb_walker_intent(moves_executed, ascension);
    }
    if definition.content_id == DARKLING_ID {
        return darkling_intent(moves_executed, rolled_attack_damage);
    }
    if definition.content_id == EXPLODER_ID {
        return exploder_intent(moves_executed, ascension);
    }
    if definition.content_id == SPIKER_ID {
        return spiker_intent(moves_executed, ascension);
    }
    if definition.content_id == REPULSOR_ID {
        return repulsor_intent(moves_executed, ascension);
    }
    if definition.content_id == SLIME_BOSS_ID {
        return slime_boss_intent(moves_executed, ascension);
    }
    if let Some(intent) =
        source_backed_complex_monster_intent(definition.content_id, moves_executed, ascension)
    {
        return intent;
    }
    if let Some(intent) =
        source_backed_gremlin_leader_minion_intent(definition.content_id, moves_executed, ascension)
    {
        return intent;
    }
    let _ = mode_shift;
    prepare_monster_intent_for(definition, moves_executed, rolled_attack_damage)
}

#[must_use]
fn prepare_monster_intent_for(
    definition: &MonsterDefinition,
    moves_executed: u32,
    rolled_attack_damage: Option<i32>,
) -> MonsterIntent {
    let complex_intent =
        source_backed_complex_monster_intent(definition.content_id, moves_executed, 0);
    let source_backed_minion_intent =
        source_backed_gremlin_leader_minion_intent(definition.content_id, moves_executed, 0);
    match definition.content_id {
        AWAKENED_ONE_ID => MonsterIntent::Attack {
            damage: if moves_executed == 0 {
                AWAKENED_ONE_SLASH_DAMAGE
            } else {
                AWAKENED_ONE_DARK_ECHO_DAMAGE
            },
        },
        CULTIST_ID if moves_executed == 0 => MonsterIntent::Ritual {
            amount: definition.ritual_amount,
        },
        CULTIST_ID => MonsterIntent::Attack {
            damage: definition.attack_damage,
        },
        JAW_WORM_ID => jaw_worm_intent(moves_executed),
        GREMLIN_NOB_ID => gremlin_nob_intent(moves_executed, 0),
        RED_LOUSE_ID => red_louse_intent(moves_executed, rolled_attack_damage),
        GREEN_LOUSE_ID => green_louse_intent(moves_executed, rolled_attack_damage),
        LOOTER_ID => looter_intent(
            moves_executed,
            ascension_from_damage_roll(rolled_attack_damage),
        ),
        MUGGER_ID => mugger_intent(moves_executed, 0),
        CHOSEN_ID => chosen_intent(moves_executed, 0),
        SNAKE_PLANT_ID => snake_plant_intent(moves_executed, 0),
        SNECKO_ID => snecko_intent(moves_executed, 0),
        CENTURION_ID => centurion_intent(moves_executed, 0),
        HEALER_ID => healer_intent(moves_executed, 0),
        BYRD_ID => byrd_intent(moves_executed, 0),
        SHELLED_PARASITE_ID => shelled_parasite_intent(moves_executed, 0),
        BOOK_OF_STABBING_ID => book_of_stabbing_intent(moves_executed, 0),
        TASKMASTER_ID => taskmaster_intent(),
        GREMLIN_LEADER_ID => gremlin_leader_intent(moves_executed, 0),
        BRONZE_AUTOMATON_ID => bronze_automaton_intent(moves_executed, 0),
        THE_COLLECTOR_ID => collector_intent(moves_executed),
        TORCH_HEAD_ID => MonsterIntent::Attack {
            damage: TORCH_HEAD_ATTACK_DAMAGE,
        },
        BRONZE_ORB_ID => bronze_orb_intent(moves_executed),
        ORB_WALKER_ID => orb_walker_intent(moves_executed, 0),
        DARKLING_ID => darkling_intent(moves_executed, rolled_attack_damage),
        EXPLODER_ID => exploder_intent(moves_executed, 0),
        SPIKER_ID => spiker_intent(moves_executed, 0),
        REPULSOR_ID => repulsor_intent(moves_executed, 0),
        _ if complex_intent.is_some() => {
            complex_intent.expect("guard established a source-backed complex monster intent")
        }
        FUNGI_BEAST_ID => fungi_beast_intent(moves_executed, 0),
        SLAVER_BLUE_ID => slaver_blue_intent(moves_executed, 0),
        SLAVER_RED_ID => slaver_red_intent(moves_executed, 0),
        _ if source_backed_minion_intent.is_some() => source_backed_minion_intent
            .expect("guard established a source-backed Gremlin Leader minion intent"),
        SPIKE_SLIME_ID => spike_slime_s_intent(moves_executed),
        ACID_SLIME_ID => acid_slime_intent(moves_executed),
        SENTRY_ID => sentry_intent(moves_executed, 0),
        SPHERIC_GUARDIAN_ID => {
            target_spheric_guardian_next_intent_from_roll(moves_executed, &[], 0)
        }
        HEXAGHOST_ID => hexaghost_intent(moves_executed),
        SLIME_BOSS_ID => slime_boss_intent(moves_executed, 0),
        FIXED_SIMPLE_MONSTER_ID => MonsterIntent::Attack {
            damage: definition.attack_damage,
        },
        _ => MonsterIntent::PendingAiRoll,
    }
}

fn ascension_from_damage_roll(_rolled_attack_damage: Option<i32>) -> u8 {
    0
}

const LOUSE_ATTACK_MOVE: u8 = 3;
const LOUSE_NON_ATTACK_MOVE: u8 = 4;

/// Deterministic Red Louse move cycle: Curl ? Bite, keyed on `moves_executed`.
#[must_use]
fn red_louse_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::StrengthAndBlock {
            strength: LOUSE_CURL_STRENGTH,
            block: 0,
        },
        _ => rolled_attack_damage.map_or(MonsterIntent::PendingAiRoll, |damage| {
            MonsterIntent::Attack { damage }
        }),
    }
}

#[must_use]
fn green_louse_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::ApplyPlayerWeak {
            amount: GREEN_LOUSE_WEAK,
        },
        _ => rolled_attack_damage.map_or(MonsterIntent::PendingAiRoll, |damage| {
            MonsterIntent::Attack { damage }
        }),
    }
}

pub fn target_louse_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    attack_damage: i32,
    non_attack_intent: MonsterIntent,
) -> MonsterIntent {
    if last_two_moves(move_history, LOUSE_NON_ATTACK_MOVE) {
        return MonsterIntent::Attack {
            damage: attack_damage,
        };
    }
    if last_two_moves(move_history, LOUSE_ATTACK_MOVE) {
        return non_attack_intent;
    }
    target_louse_entry_intent_from_roll(roll, attack_damage, non_attack_intent)
}

#[must_use]
pub fn target_louse_entry_intent_from_roll(
    roll: i32,
    attack_damage: i32,
    non_attack_intent: MonsterIntent,
) -> MonsterIntent {
    if roll >= 25 {
        MonsterIntent::Attack {
            damage: attack_damage,
        }
    } else {
        non_attack_intent
    }
}

pub fn target_darkling_next_intent_from_roll_with_rng(
    move_history: &[u8],
    roll: i32,
    monster_index: usize,
    attack_damage: i32,
    ascension: u8,
    rng: &mut StsRng,
) -> MonsterIntent {
    target_darkling_next_intent_from_roll_inner(
        move_history,
        roll,
        monster_index,
        attack_damage,
        ascension,
        rng,
    )
}

fn target_darkling_next_intent_from_roll_inner(
    move_history: &[u8],
    roll: i32,
    monster_index: usize,
    attack_damage: i32,
    ascension: u8,
    rng: &mut StsRng,
) -> MonsterIntent {
    if move_history.is_empty() {
        return if roll < 50 {
            darkling_block_intent(ascension)
        } else {
            MonsterIntent::Attack {
                damage: attack_damage,
            }
        };
    }

    if roll < 40 {
        if !move_history.ends_with(&[1]) && monster_index.is_multiple_of(2) {
            MonsterIntent::AttackMultiple {
                damage: DARKLING_CHOMP_DAMAGE,
                hits: 2,
            }
        } else {
            let reroll = rng.random_int_range(40, 99);
            target_darkling_next_intent_from_roll_inner(
                move_history,
                reroll,
                monster_index,
                attack_damage,
                ascension,
                rng,
            )
        }
    } else if roll < 70 {
        if !move_history.ends_with(&[2]) {
            darkling_block_intent(ascension)
        } else {
            MonsterIntent::Attack {
                damage: attack_damage,
            }
        }
    } else if !move_history.ends_with(&[3, 3]) {
        MonsterIntent::Attack {
            damage: attack_damage,
        }
    } else {
        let reroll = rng.random_int_range(0, 99);
        target_darkling_next_intent_from_roll_inner(
            move_history,
            reroll,
            monster_index,
            attack_damage,
            ascension,
            rng,
        )
    }
}

fn darkling_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    if moves_executed == 0 {
        rolled_attack_damage.map_or(MonsterIntent::PendingAiRoll, |damage| {
            MonsterIntent::Attack { damage }
        })
    } else {
        darkling_block_intent(0)
    }
}

fn darkling_block_intent(ascension: u8) -> MonsterIntent {
    if ascension >= 17 {
        MonsterIntent::StrengthAndBlock {
            strength: 2,
            block: DARKLING_BLOCK,
        }
    } else {
        MonsterIntent::Block {
            block: DARKLING_BLOCK,
        }
    }
}

#[must_use]
fn exploder_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if moves_executed < 2 {
        MonsterIntent::Attack {
            damage: exploder_attack_damage(ascension),
        }
    } else {
        MonsterIntent::Stun
    }
}

#[must_use]
pub fn target_exploder_next_intent_from_roll(moves_executed: u32, ascension: u8) -> MonsterIntent {
    exploder_intent(moves_executed, ascension)
}

#[must_use]
pub fn target_maw_next_intent_from_roll(
    moves_executed: u32,
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if moves_executed == 0 {
        let amount = if ascension >= 17 {
            MAW_A17_ROAR_DEBUFF
        } else {
            MAW_ROAR_DEBUFF
        };
        return MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: amount,
            weak: amount,
        };
    }
    if roll < 50 && !last_move(move_history, 5) {
        let hits = ((moves_executed + 2) / 2) as i32;
        if hits <= 1 {
            return MonsterIntent::Attack {
                damage: MAW_NOM_DAMAGE,
            };
        }
        return MonsterIntent::AttackMultiple {
            damage: MAW_NOM_DAMAGE,
            hits,
        };
    }
    if last_move(move_history, 3) || last_move(move_history, 5) {
        return MonsterIntent::StrengthSelf {
            amount: if ascension >= 17 {
                MAW_A17_STRENGTH
            } else {
                MAW_STRENGTH
            },
        };
    }
    MonsterIntent::Attack {
        damage: asc_damage(ascension, MAW_SLAM_DAMAGE, MAW_A2_SLAM_DAMAGE, 2),
    }
}

#[must_use]
pub fn target_spire_growth_next_intent_from_roll(
    moves_executed: u32,
    move_history: &[u8],
    roll: i32,
    player_constricted: bool,
    ascension: u8,
) -> MonsterIntent {
    if moves_executed == 0 && ascension >= 17 && !player_constricted {
        return spire_growth_constrict_intent(ascension);
    }
    if roll < 50 && !last_two_moves(move_history, 1) {
        return spire_growth_quick_tackle_intent(ascension);
    }
    if !player_constricted && !last_move(move_history, 2) {
        return spire_growth_constrict_intent(ascension);
    }
    if !last_two_moves(move_history, 3) {
        return spire_growth_smash_intent(ascension);
    }
    spire_growth_quick_tackle_intent(ascension)
}

#[must_use]
pub fn target_giant_head_next_intent_from_roll(
    moves_executed: u32,
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    let count_before = giant_head_initial_count(ascension) - moves_executed as i32;
    if count_before <= 1 {
        return MonsterIntent::Attack {
            damage: giant_head_it_is_time_damage(count_before, ascension),
        };
    }
    if roll < 50 {
        if !last_two_moves(move_history, 1) {
            MonsterIntent::ApplyPlayerWeak {
                amount: GIANT_HEAD_GLARE_WEAK,
            }
        } else {
            giant_head_count_intent()
        }
    } else if !last_two_moves(move_history, 3) {
        giant_head_count_intent()
    } else {
        MonsterIntent::ApplyPlayerWeak {
            amount: GIANT_HEAD_GLARE_WEAK,
        }
    }
}

fn giant_head_initial_count(ascension: u8) -> i32 {
    if ascension >= 18 {
        4
    } else {
        5
    }
}

fn giant_head_death_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        GIANT_HEAD_A3_DEATH_DAMAGE
    } else {
        GIANT_HEAD_DEATH_DAMAGE
    }
}

fn giant_head_it_is_time_damage(count_before: i32, ascension: u8) -> i32 {
    let count_after = (count_before - 1).max(-6);
    giant_head_death_damage(ascension) - count_after * GIANT_HEAD_DAMAGE_INCREMENT
}

fn giant_head_count_intent() -> MonsterIntent {
    MonsterIntent::Attack {
        damage: GIANT_HEAD_COUNT_DAMAGE,
    }
}

#[must_use]
pub fn target_nemesis_next_intent_from_roll(
    moves_executed: u32,
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    if moves_executed == 0 {
        return if roll < 50 {
            nemesis_tri_attack_intent(ascension)
        } else {
            nemesis_burn_intent(ascension)
        };
    }

    let scythe_available = !last_move(move_history, 3);
    if roll < 30 {
        if scythe_available {
            return nemesis_scythe_intent();
        }
        if rng.random_bool() {
            if !last_two_moves(move_history, 2) {
                return nemesis_tri_attack_intent(ascension);
            }
            return nemesis_burn_intent(ascension);
        }
        if !last_move(move_history, 4) {
            return nemesis_burn_intent(ascension);
        }
        return nemesis_tri_attack_intent(ascension);
    }
    if roll < 65 {
        if !last_two_moves(move_history, 2) {
            return nemesis_tri_attack_intent(ascension);
        }
        if rng.random_bool() {
            if scythe_available {
                return nemesis_scythe_intent();
            }
            return nemesis_burn_intent(ascension);
        }
        return nemesis_burn_intent(ascension);
    }
    if !last_move(move_history, 4) {
        return nemesis_burn_intent(ascension);
    }
    if rng.random_bool() && scythe_available {
        return nemesis_scythe_intent();
    }
    nemesis_tri_attack_intent(ascension)
}

fn nemesis_tri_attack_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: asc_damage(
            ascension,
            NEMESIS_TRI_ATTACK_DAMAGE,
            NEMESIS_A3_TRI_ATTACK_DAMAGE,
            3,
        ),
        hits: NEMESIS_TRI_ATTACK_HITS,
    }
}

fn nemesis_scythe_intent() -> MonsterIntent {
    MonsterIntent::Attack {
        damage: NEMESIS_SCYTHE_DAMAGE,
    }
}

fn nemesis_burn_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AddBurnToDiscard {
        count: if ascension >= 18 {
            NEMESIS_A18_BURNS
        } else {
            NEMESIS_BURNS
        },
        damage: 0,
    }
}

fn spire_growth_quick_tackle_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: asc_damage(
            ascension,
            SPIRE_GROWTH_QUICK_TACKLE_DAMAGE,
            SPIRE_GROWTH_A2_QUICK_TACKLE_DAMAGE,
            2,
        ),
    }
}

fn spire_growth_smash_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: asc_damage(
            ascension,
            SPIRE_GROWTH_SMASH_DAMAGE,
            SPIRE_GROWTH_A2_SMASH_DAMAGE,
            2,
        ),
    }
}

fn spire_growth_constrict_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::ApplyPlayerConstricted {
        amount: if ascension >= 17 {
            SPIRE_GROWTH_A17_CONSTRICT
        } else {
            SPIRE_GROWTH_CONSTRICT
        },
    }
}

#[must_use]
fn spiker_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if moves_executed == 0 {
        spiker_attack_intent(ascension)
    } else {
        spiker_buff_intent()
    }
}

#[must_use]
pub fn target_spiker_next_intent_from_roll(
    move_history: &[u8],
    thorns_buffs: i32,
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if thorns_buffs > 5 || (roll < 50 && !last_move(move_history, 1)) {
        spiker_attack_intent(ascension)
    } else {
        spiker_buff_intent()
    }
}

#[must_use]
fn repulsor_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if moves_executed == 1 {
        MonsterIntent::Attack {
            damage: repulsor_attack_damage(ascension),
        }
    } else {
        MonsterIntent::AddDazedToDraw {
            count: REPULSOR_DAZES,
        }
    }
}

#[must_use]
pub fn target_repulsor_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if roll < 20 && !last_move(move_history, 2) {
        MonsterIntent::Attack {
            damage: repulsor_attack_damage(ascension),
        }
    } else {
        MonsterIntent::AddDazedToDraw {
            count: REPULSOR_DAZES,
        }
    }
}

fn transient_attack_base_damage(ascension: u8) -> i32 {
    if ascension >= 4 {
        TRANSIENT_A4_ATTACK_DAMAGE
    } else {
        TRANSIENT_ATTACK_DAMAGE
    }
}

pub fn transient_attack_damage(moves_executed: u32, ascension: u8) -> SimResult<i32> {
    let moves = i32::try_from(moves_executed)
        .map_err(|_| SimError::InvalidState("transient attack damage exceeds supported range"))?;
    moves
        .checked_mul(TRANSIENT_ATTACK_DAMAGE_STEP)
        .and_then(|increase| transient_attack_base_damage(ascension).checked_add(increase))
        .ok_or(SimError::InvalidState(
            "transient attack damage exceeds supported range",
        ))
}

#[must_use]
fn exploder_attack_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        EXPLODER_A2_ATTACK_DAMAGE
    } else {
        EXPLODER_ATTACK_DAMAGE
    }
}

#[must_use]
fn spiker_attack_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SPIKER_A2_ATTACK_DAMAGE
    } else {
        SPIKER_ATTACK_DAMAGE
    }
}

#[must_use]
fn spiker_starting_thorns(ascension: u8) -> i32 {
    let base = if ascension >= 2 {
        SPIKER_A2_THORNS
    } else {
        SPIKER_THORNS
    };
    if ascension >= 17 {
        base + SPIKER_A17_THORNS_BONUS
    } else {
        base
    }
}

#[must_use]
fn spiker_attack_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: spiker_attack_damage(ascension),
    }
}

#[must_use]
fn spiker_buff_intent() -> MonsterIntent {
    MonsterIntent::StrengthAndBlock {
        strength: 0,
        block: 0,
    }
}

#[must_use]
fn repulsor_attack_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        REPULSOR_A2_ATTACK_DAMAGE
    } else {
        REPULSOR_ATTACK_DAMAGE
    }
}

#[must_use]
fn looter_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 | 1 => MonsterIntent::AttackStealGold {
            damage: looter_swipe_damage(ascension),
            amount: looter_theft(ascension),
        },
        2 => MonsterIntent::AttackStealGold {
            damage: looter_lunge_damage(ascension),
            amount: looter_theft(ascension),
        },
        3 => MonsterIntent::Block {
            block: LOOTER_SMOKE_BOMB_BLOCK,
        },
        _ => MonsterIntent::Escape,
    }
}

#[must_use]
fn looter_lunge_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        LOOTER_A2_LUNGE_DAMAGE
    } else {
        LOOTER_LUNGE_DAMAGE
    }
}

#[must_use]
pub fn target_looter_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if last_two_moves(move_history, 1) {
        if roll < 50 {
            MonsterIntent::Block {
                block: LOOTER_SMOKE_BOMB_BLOCK,
            }
        } else {
            MonsterIntent::AttackStealGold {
                damage: looter_lunge_damage(ascension),
                amount: looter_theft(ascension),
            }
        }
    } else if last_move(move_history, 4) {
        MonsterIntent::Block {
            block: LOOTER_SMOKE_BOMB_BLOCK,
        }
    } else if last_move(move_history, 2) || last_move(move_history, 3) {
        MonsterIntent::Escape
    } else {
        MonsterIntent::AttackStealGold {
            damage: looter_swipe_damage(ascension),
            amount: looter_theft(ascension),
        }
    }
}

#[must_use]
pub fn target_looter_direct_next_intent_after_turn(
    move_history: &[u8],
    moves_executed: u32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    if last_move(move_history, 1) && moves_executed == 1 {
        let _ = rng.random_float() < 0.6;
        return MonsterIntent::AttackStealGold {
            damage: looter_swipe_damage(ascension),
            amount: looter_theft(ascension),
        };
    }
    if last_move(move_history, 1) && moves_executed == 2 {
        if rng.random_float() < 0.5 {
            return MonsterIntent::Block {
                block: LOOTER_SMOKE_BOMB_BLOCK,
            };
        }
        return MonsterIntent::AttackStealGold {
            damage: looter_lunge_damage(ascension),
            amount: looter_theft(ascension),
        };
    }
    if last_move(move_history, 4) {
        return MonsterIntent::Block {
            block: LOOTER_SMOKE_BOMB_BLOCK,
        };
    }
    MonsterIntent::Escape
}

#[must_use]
fn mugger_swipe_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        MUGGER_A2_SWIPE_DAMAGE
    } else {
        MUGGER_SWIPE_DAMAGE
    }
}

#[must_use]
fn mugger_big_swipe_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        MUGGER_A2_BIG_SWIPE_DAMAGE
    } else {
        MUGGER_BIG_SWIPE_DAMAGE
    }
}

#[must_use]
fn mugger_theft(ascension: u8) -> i32 {
    if ascension >= 17 {
        MUGGER_A17_THEFT
    } else {
        MUGGER_THEFT
    }
}

#[must_use]
fn mugger_escape_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        MUGGER_A17_SMOKE_BOMB_BLOCK
    } else {
        MUGGER_SMOKE_BOMB_BLOCK
    }
}

#[must_use]
fn mugger_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 | 1 => MonsterIntent::AttackStealGold {
            damage: mugger_swipe_damage(ascension),
            amount: mugger_theft(ascension),
        },
        2 => MonsterIntent::AttackStealGold {
            damage: mugger_big_swipe_damage(ascension),
            amount: mugger_theft(ascension),
        },
        3 => MonsterIntent::Block {
            block: mugger_escape_block(ascension),
        },
        _ => MonsterIntent::Escape,
    }
}

#[must_use]
pub fn target_mugger_direct_next_intent_after_turn(
    move_history: &[u8],
    moves_executed: u32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    if last_move(move_history, 1) && moves_executed == 1 {
        let _ = rng.random_int(2);
        return MonsterIntent::AttackStealGold {
            damage: mugger_swipe_damage(ascension),
            amount: mugger_theft(ascension),
        };
    }
    if last_move(move_history, 1) && moves_executed == 2 {
        let _ = rng.random_int(2);
        let _ = rng.random_float() < 0.6;
        if rng.random_float() < 0.5 {
            return MonsterIntent::Block {
                block: mugger_escape_block(ascension),
            };
        }
        return MonsterIntent::AttackStealGold {
            damage: mugger_big_swipe_damage(ascension),
            amount: mugger_theft(ascension),
        };
    }
    if last_move(move_history, 4) {
        let _ = rng.random_int(2);
        return MonsterIntent::Block {
            block: mugger_escape_block(ascension),
        };
    }
    MonsterIntent::Escape
}

fn chosen_poke_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        CHOSEN_A2_POKE_DAMAGE
    } else {
        CHOSEN_POKE_DAMAGE
    }
}

fn chosen_zap_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        CHOSEN_A2_ZAP_DAMAGE
    } else {
        CHOSEN_ZAP_DAMAGE
    }
}

fn chosen_debilitate_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        CHOSEN_A2_DEBILITATE_DAMAGE
    } else {
        CHOSEN_DEBILITATE_DAMAGE
    }
}

#[must_use]
fn chosen_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if ascension >= 17 {
        return match moves_executed {
            0 => MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX },
            1 => MonsterIntent::AttackApplyPlayerVulnerable {
                damage: chosen_debilitate_damage(ascension),
                vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
            },
            2 => MonsterIntent::ApplyPlayerWeakStrengthSelf {
                weak: CHOSEN_DRAIN_WEAK,
                strength: CHOSEN_DRAIN_STRENGTH,
            },
            3 => MonsterIntent::Attack {
                damage: chosen_zap_damage(ascension),
            },
            _ => MonsterIntent::AttackMultiple {
                damage: chosen_poke_damage(ascension),
                hits: CHOSEN_POKE_HITS,
            },
        };
    }

    match moves_executed {
        0 => MonsterIntent::AttackMultiple {
            damage: chosen_poke_damage(ascension),
            hits: CHOSEN_POKE_HITS,
        },
        1 => MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX },
        2 => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: chosen_debilitate_damage(ascension),
            vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
        },
        3 => MonsterIntent::ApplyPlayerWeakStrengthSelf {
            weak: CHOSEN_DRAIN_WEAK,
            strength: CHOSEN_DRAIN_STRENGTH,
        },
        4 => MonsterIntent::Attack {
            damage: chosen_zap_damage(ascension),
        },
        _ => MonsterIntent::AttackMultiple {
            damage: chosen_poke_damage(ascension),
            hits: CHOSEN_POKE_HITS,
        },
    }
}

#[must_use]
pub fn target_chosen_next_intent(
    move_history: &[u8],
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let roll = rng.random_int(99);
    target_chosen_next_intent_from_roll(move_history, roll, ascension)
}

#[must_use]
pub fn target_chosen_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if ascension >= 17 && !move_history.contains(&4) {
        return chosen_hex_intent();
    }
    if ascension < 17 && move_history.is_empty() {
        return chosen_poke_intent(ascension);
    }
    if !move_history.contains(&4) {
        return chosen_hex_intent();
    }

    if !last_move(move_history, 3) && !last_move(move_history, 2) {
        if roll < 50 {
            chosen_debilitate_intent(ascension)
        } else {
            chosen_drain_intent()
        }
    } else if roll < 40 {
        chosen_zap_intent(ascension)
    } else {
        chosen_poke_intent(ascension)
    }
}

#[must_use]
fn chosen_hex_intent() -> MonsterIntent {
    MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX }
}

#[must_use]
fn chosen_debilitate_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackApplyPlayerVulnerable {
        damage: chosen_debilitate_damage(ascension),
        vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
    }
}

#[must_use]
fn chosen_drain_intent() -> MonsterIntent {
    MonsterIntent::ApplyPlayerWeakStrengthSelf {
        weak: CHOSEN_DRAIN_WEAK,
        strength: CHOSEN_DRAIN_STRENGTH,
    }
}

#[must_use]
fn chosen_zap_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: chosen_zap_damage(ascension),
    }
}

#[must_use]
fn chosen_poke_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: chosen_poke_damage(ascension),
        hits: CHOSEN_POKE_HITS,
    }
}

pub fn target_champ_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    hp: i32,
    max_hp: i32,
    ascension: u8,
) -> MonsterIntent {
    if hp < max_hp / 2 && !move_history.contains(&7) {
        return MonsterIntent::StrengthSelf {
            amount: champ_strength_amount(ascension) * 3,
        };
    }
    if move_history.contains(&7)
        && !last_move(move_history, 3)
        && !last_move_before(move_history, 3)
    {
        return MonsterIntent::AttackMultiple {
            damage: CHAMP_EXECUTE_DAMAGE,
            hits: CHAMP_EXECUTE_HITS,
        };
    }
    if turns_since_champ_taunt(move_history) >= 3 && !move_history.contains(&7) {
        // Champ's Taunt move (move 6) applies Weak 2 and Vulnerable 2.
        // The zero-Frail form is also used for other source-backed combined
        // debuffs, so preserve both player powers in the authoritative intent.
        return MonsterIntent::ApplyPlayerFrailWeakVulnerable {
            frail: 0,
            weak: 2,
            vulnerable: 2,
        };
    }
    if !last_move(move_history, 2) && champ_defensive_stance_count(move_history) < 2 && roll <= 15 {
        return MonsterIntent::StrengthAndBlock {
            strength: CHAMP_DEFENSIVE_METALLICIZE,
            block: CHAMP_DEFENSIVE_BLOCK,
        };
    }
    if !last_move(move_history, 5) && !last_move(move_history, 2) && roll <= 30 {
        return MonsterIntent::StrengthSelf {
            amount: champ_strength_amount(ascension),
        };
    }
    if !last_move(move_history, 4) && roll <= 55 {
        return champ_face_slap_intent(ascension);
    }
    if !last_move(move_history, 1) {
        MonsterIntent::Attack {
            damage: asc_damage(
                ascension,
                CHAMP_HEAVY_SLASH_DAMAGE,
                CHAMP_A4_HEAVY_SLASH_DAMAGE,
                4,
            ),
        }
    } else {
        champ_face_slap_intent(ascension)
    }
}

fn champ_face_slap_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackApplyPlayerFrailAndVulnerable {
        damage: asc_damage(
            ascension,
            CHAMP_FACE_SLAP_DAMAGE,
            CHAMP_A4_FACE_SLAP_DAMAGE,
            4,
        ),
        frail: CHAMP_FACE_SLAP_FRAIL,
        vulnerable: 2,
    }
}

pub fn champ_strength_amount(ascension: u8) -> i32 {
    if ascension >= 19 {
        4
    } else if ascension >= 4 {
        3
    } else {
        2
    }
}

fn champ_defensive_stance_count(move_history: &[u8]) -> usize {
    move_history.iter().filter(|move_id| **move_id == 2).count()
}

fn turns_since_champ_taunt(move_history: &[u8]) -> usize {
    move_history
        .iter()
        .rev()
        .take_while(|move_id| **move_id != 6)
        .count()
}

fn snake_plant_chompy_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SNAKE_PLANT_A2_CHOMPY_DAMAGE
    } else {
        SNAKE_PLANT_CHOMPY_DAMAGE
    }
}

#[must_use]
fn snake_plant_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::AttackMultiple {
            damage: snake_plant_chompy_damage(ascension),
            hits: SNAKE_PLANT_CHOMPY_HITS,
        },
        _ => MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: SNAKE_PLANT_SPORES_DEBUFF,
            weak: SNAKE_PLANT_SPORES_DEBUFF,
        },
    }
}

#[must_use]
pub fn target_snake_plant_next_intent(
    move_history: &[u8],
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let roll = rng.random_int(99);
    target_snake_plant_next_intent_from_roll(move_history, roll, ascension)
}

#[must_use]
pub fn target_snake_plant_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if ascension >= 17 {
        if roll < 65 {
            if last_two_moves(move_history, 1) {
                return snake_plant_spores_intent();
            }
            return snake_plant_chompy_intent(ascension);
        }
        if last_move(move_history, 2) || last_move_before(move_history, 2) {
            return snake_plant_chompy_intent(ascension);
        }
        return snake_plant_spores_intent();
    }

    if roll < 65 {
        if last_two_moves(move_history, 1) {
            return snake_plant_spores_intent();
        }
        return snake_plant_chompy_intent(ascension);
    }
    if last_move(move_history, 2) {
        return snake_plant_chompy_intent(ascension);
    }
    snake_plant_spores_intent()
}

#[must_use]
pub fn target_awakened_one_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    phase: i32,
    _ascension: u8,
) -> MonsterIntent {
    if phase == 0 {
        if move_history.is_empty() {
            return MonsterIntent::Attack {
                damage: AWAKENED_ONE_SLASH_DAMAGE,
            };
        }
        if last_two_moves(move_history, 1) || (last_move(move_history, 1) && roll < 25) {
            return MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_SOUL_STRIKE_DAMAGE,
                hits: AWAKENED_ONE_SOUL_STRIKE_HITS,
            };
        }
        return MonsterIntent::Attack {
            damage: AWAKENED_ONE_SLASH_DAMAGE,
        };
    }

    if move_history.is_empty() {
        return MonsterIntent::Attack {
            damage: AWAKENED_ONE_DARK_ECHO_DAMAGE,
        };
    }
    // AwakenedOne.getMove uses AbstractMonster's common AI roll directly:
    // below 50 prefers Sludge, while 50+ prefers Tackle. Each branch only
    // changes when its preferred move was used twice consecutively.
    if roll < 50 {
        if last_two_moves(move_history, 6) {
            MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_TACKLE_DAMAGE,
                hits: AWAKENED_ONE_TACKLE_HITS,
            }
        } else {
            MonsterIntent::AttackAddVoidToDraw {
                damage: 18,
                count: 1,
            }
        }
    } else if last_two_moves(move_history, 8) {
        MonsterIntent::AttackAddVoidToDraw {
            damage: 18,
            count: 1,
        }
    } else {
        MonsterIntent::AttackMultiple {
            damage: AWAKENED_ONE_TACKLE_DAMAGE,
            hits: AWAKENED_ONE_TACKLE_HITS,
        }
    }
}

/// The Awakened One's first death is a delayed phase transition, not combat
/// victory. The target leaves the monster in a half-dead state until its next
/// monster turn, then revives (REBIRTH) with remaining buffs and starts Dark Echo.
///
/// Source (`AwakenedOne.damage`): on first form death, remove DEBUFF powers,
/// Curiosity, Unawakened, and Shackled (`GainStrengthPower`). `StrengthPower` is
/// a DEBUFF when amount is negative and a BUFF when positive, so only non-negative
/// Strength survives into phase two.
pub fn awakened_one_is_half_dead(monster: &MonsterState) -> bool {
    monster.content_id == AWAKENED_ONE_ID && !monster.alive && monster.mode_shift < 0
}

pub fn awaken_one_after_first_death(monster: &mut MonsterState) -> bool {
    if !awakened_one_is_half_dead(monster) || !matches!(monster.intent, MonsterIntent::Stun) {
        return false;
    }

    // REBIRTH heals to max HP. Debuffs/Shackled were already stripped on the
    // first death; surviving buff Strength is left as-is.
    monster.alive = true;
    monster.hp = monster.max_hp;
    monster.mode_shift = 1;
    monster.intent = MonsterIntent::Attack {
        damage: AWAKENED_ONE_DARK_ECHO_DAMAGE,
    };
    record_target_move(monster);
    true
}

pub fn mark_awakened_one_half_dead(monster: &mut MonsterState) -> bool {
    if monster.content_id != AWAKENED_ONE_ID || monster.mode_shift != 0 {
        return false;
    }

    monster.alive = false;
    monster.hp = 0;
    monster.block = 0;
    monster.mode_shift = -1;
    strip_awakened_one_half_death_powers(monster);
    monster.intent = MonsterIntent::Stun;
    record_target_move(monster);
    true
}

/// Source-backed half-death power cleanup for Awakened One.
///
/// Mirrors `AwakenedOne.damage`: drop DEBUFF-typed powers and Shackled. Curiosity
/// / Unawakened are not stored as power fields; `mode_shift` already gates
/// Curiosity after this transition.
fn strip_awakened_one_half_death_powers(monster: &mut MonsterState) {
    // StrengthPower.type is DEBUFF when amount < 0.
    if monster.powers.strength < 0 {
        monster.powers.strength = 0;
    }
    monster.powers.vulnerable = 0;
    monster.powers.weak = 0;
    monster.powers.slow = 0;
    // GainStrengthPower ID "Shackled" — temporary strength restore marker.
    monster.temp_strength_down = 0;
}

#[must_use]
fn snake_plant_chompy_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: snake_plant_chompy_damage(ascension),
        hits: SNAKE_PLANT_CHOMPY_HITS,
    }
}

#[must_use]
fn snake_plant_spores_intent() -> MonsterIntent {
    MonsterIntent::ApplyPlayerFrailAndWeak {
        frail: SNAKE_PLANT_SPORES_DEBUFF,
        weak: SNAKE_PLANT_SPORES_DEBUFF,
    }
}

fn snecko_bite_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SNECKO_A2_BITE_DAMAGE
    } else {
        SNECKO_BITE_DAMAGE
    }
}

fn snecko_tail_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SNECKO_A2_TAIL_DAMAGE
    } else {
        SNECKO_TAIL_DAMAGE
    }
}

#[must_use]
fn snecko_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::ApplyPlayerConfusion,
        1 if ascension >= 17 => MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
            damage: snecko_tail_damage(ascension),
            weak: SNECKO_A17_WEAK,
            vulnerable: SNECKO_VULNERABLE,
        },
        1 => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: snecko_tail_damage(ascension),
            vulnerable: SNECKO_VULNERABLE,
        },
        _ => MonsterIntent::Attack {
            damage: snecko_bite_damage(ascension),
        },
    }
}

#[must_use]
pub fn target_snecko_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if move_history.is_empty() {
        return MonsterIntent::ApplyPlayerConfusion;
    }
    if roll < 40 || last_two_moves(move_history, 2) {
        if ascension >= 17 {
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: snecko_tail_damage(ascension),
                weak: SNECKO_A17_WEAK,
                vulnerable: SNECKO_VULNERABLE,
            }
        } else {
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: snecko_tail_damage(ascension),
                vulnerable: SNECKO_VULNERABLE,
            }
        }
    } else {
        MonsterIntent::Attack {
            damage: snecko_bite_damage(ascension),
        }
    }
}

#[must_use]
fn centurion_slash_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        CENTURION_A2_SLASH_DAMAGE
    } else {
        CENTURION_SLASH_DAMAGE
    }
}

fn centurion_fury_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        CENTURION_A2_FURY_DAMAGE
    } else {
        CENTURION_FURY_DAMAGE
    }
}

fn centurion_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        CENTURION_A17_BLOCK
    } else {
        CENTURION_BLOCK
    }
}

#[must_use]
fn centurion_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 | 1 => MonsterIntent::Attack {
            damage: centurion_slash_damage(ascension),
        },
        2 => MonsterIntent::Block {
            block: centurion_block(ascension),
        },
        _ => MonsterIntent::AttackMultiple {
            damage: centurion_fury_damage(ascension),
            hits: CENTURION_FURY_HITS,
        },
    }
}

#[must_use]
pub fn target_centurion_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    living_monster_count: usize,
    ascension: u8,
) -> MonsterIntent {
    if roll >= 65 && !last_two_moves(move_history, 2) && !last_two_moves(move_history, 3) {
        return centurion_protect_or_fury(living_monster_count, ascension);
    }
    if !last_two_moves(move_history, 1) {
        return MonsterIntent::Attack {
            damage: centurion_slash_damage(ascension),
        };
    }
    centurion_protect_or_fury(living_monster_count, ascension)
}

#[must_use]
fn centurion_protect_or_fury(living_monster_count: usize, ascension: u8) -> MonsterIntent {
    if living_monster_count > 1 {
        MonsterIntent::Block {
            block: centurion_block(ascension),
        }
    } else {
        MonsterIntent::AttackMultiple {
            damage: centurion_fury_damage(ascension),
            hits: CENTURION_FURY_HITS,
        }
    }
}

#[must_use]
fn healer_attack_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        HEALER_A2_ATTACK_DAMAGE
    } else {
        HEALER_ATTACK_DAMAGE
    }
}

fn healer_heal(ascension: u8) -> i32 {
    if ascension >= 17 {
        HEALER_A17_HEAL
    } else {
        HEALER_HEAL
    }
}

fn healer_strength(ascension: u8) -> i32 {
    if ascension >= 17 {
        HEALER_A17_STRENGTH
    } else if ascension >= 2 {
        HEALER_A2_STRENGTH
    } else {
        HEALER_STRENGTH
    }
}

#[must_use]
fn healer_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::StrengthAllMonsters {
            amount: healer_strength(ascension),
        },
        1 => MonsterIntent::AttackApplyPlayerFrail {
            damage: healer_attack_damage(ascension),
            frail: HEALER_FRAIL,
        },
        _ => MonsterIntent::HealAllMonsters {
            amount: healer_heal(ascension),
        },
    }
}

#[must_use]
pub fn target_healer_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    living_monster_missing_hp: i32,
    ascension: u8,
) -> MonsterIntent {
    let heal_threshold = if ascension >= 17 { 20 } else { 15 };
    if living_monster_missing_hp > heal_threshold && !last_two_moves(move_history, 2) {
        return MonsterIntent::HealAllMonsters {
            amount: healer_heal(ascension),
        };
    }

    if ascension >= 17 {
        if roll >= 40 && !last_move(move_history, 1) {
            return healer_attack_intent(ascension);
        }
    } else if roll >= 40 && !last_two_moves(move_history, 1) {
        return healer_attack_intent(ascension);
    }

    if !last_two_moves(move_history, 3) {
        return MonsterIntent::StrengthAllMonsters {
            amount: healer_strength(ascension),
        };
    }
    healer_attack_intent(ascension)
}

#[must_use]
fn healer_attack_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackApplyPlayerFrail {
        damage: healer_attack_damage(ascension),
        frail: HEALER_FRAIL,
    }
}

#[must_use]
pub fn living_monster_missing_hp(monsters: &[MonsterState]) -> i32 {
    monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| (monster.max_hp - monster.hp).max(0))
        .sum()
}

#[must_use]
fn byrd_peck_hits(ascension: u8) -> i32 {
    if ascension >= 2 {
        BYRD_A2_PECK_HITS
    } else {
        BYRD_PECK_HITS
    }
}

#[must_use]
fn byrd_swoop_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        BYRD_A2_SWOOP_DAMAGE
    } else {
        BYRD_SWOOP_DAMAGE
    }
}

#[must_use]
fn byrd_flight(ascension: u8) -> i32 {
    if ascension >= 17 {
        BYRD_A17_FLIGHT
    } else {
        BYRD_FLIGHT
    }
}

#[must_use]
pub fn target_byrd_flight_amount(ascension: u8) -> i32 {
    byrd_flight(ascension)
}

#[must_use]
fn byrd_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::AttackMultiple {
            damage: BYRD_PECK_DAMAGE,
            hits: byrd_peck_hits(ascension),
        },
        1 => MonsterIntent::StrengthSelf {
            amount: BYRD_CAW_STRENGTH,
        },
        2 => MonsterIntent::Attack {
            damage: byrd_swoop_damage(ascension),
        },
        _ => MonsterIntent::Attack {
            damage: BYRD_HEADBUTT_DAMAGE,
        },
    }
}

#[must_use]
pub fn target_byrd_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    if move_history.is_empty() {
        return if rng.random_float() < 0.375 {
            MonsterIntent::StrengthSelf {
                amount: BYRD_CAW_STRENGTH,
            }
        } else {
            byrd_peck_intent(ascension)
        };
    }

    if roll < 50 {
        if last_two_moves(move_history, 1) {
            return if rng.random_float() < 0.4 {
                byrd_swoop_intent(ascension)
            } else {
                byrd_caw_intent()
            };
        }
        return byrd_peck_intent(ascension);
    }

    if roll < 70 {
        if last_move(move_history, 3) {
            return if rng.random_float() < 0.375 {
                byrd_caw_intent()
            } else {
                byrd_peck_intent(ascension)
            };
        }
        return byrd_swoop_intent(ascension);
    }

    if last_move(move_history, 6) {
        return if rng.random_float() < 0.2857 {
            byrd_swoop_intent(ascension)
        } else {
            byrd_peck_intent(ascension)
        };
    }

    byrd_caw_intent()
}

#[must_use]
pub fn target_grounded_byrd_next_intent() -> MonsterIntent {
    MonsterIntent::Attack {
        damage: BYRD_HEADBUTT_DAMAGE,
    }
}

#[must_use]
pub fn target_byrd_go_airborne_intent() -> MonsterIntent {
    MonsterIntent::StrengthSelf { amount: 0 }
}

#[must_use]
fn byrd_peck_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: BYRD_PECK_DAMAGE,
        hits: byrd_peck_hits(ascension),
    }
}

#[must_use]
fn byrd_swoop_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: byrd_swoop_damage(ascension),
    }
}

#[must_use]
fn byrd_caw_intent() -> MonsterIntent {
    MonsterIntent::StrengthSelf {
        amount: BYRD_CAW_STRENGTH,
    }
}

fn shelled_parasite_double_strike_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SHELLED_PARASITE_A2_DOUBLE_STRIKE_DAMAGE
    } else {
        SHELLED_PARASITE_DOUBLE_STRIKE_DAMAGE
    }
}

fn shelled_parasite_suck_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SHELLED_PARASITE_A2_SUCK_DAMAGE
    } else {
        SHELLED_PARASITE_SUCK_DAMAGE
    }
}

fn shelled_parasite_fell_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SHELLED_PARASITE_A2_FELL_DAMAGE
    } else {
        SHELLED_PARASITE_FELL_DAMAGE
    }
}

fn shelled_parasite_fell_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackApplyPlayerFrail {
        damage: shelled_parasite_fell_damage(ascension),
        frail: SHELLED_PARASITE_FELL_FRAIL,
    }
}

/// Move Shelled Parasite installs immediately before rolling after its shell-break Stun turn.
///
/// The target records this temporary Fell move in move history even though the Stun move is what
/// the player observes for the turn. That history entry affects the normal AI reroll rules.
#[must_use]
pub fn target_shelled_parasite_shell_break_roll_move(ascension: u8) -> MonsterIntent {
    shelled_parasite_fell_intent(ascension)
}

#[must_use]
fn shelled_parasite_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if moves_executed == 0 && ascension >= 17 {
        return shelled_parasite_fell_intent(ascension);
    }

    match moves_executed {
        0 => MonsterIntent::AttackMultiple {
            damage: shelled_parasite_double_strike_damage(ascension),
            hits: SHELLED_PARASITE_DOUBLE_STRIKE_HITS,
        },
        1 => MonsterIntent::AttackHealSelf {
            damage: shelled_parasite_suck_damage(ascension),
        },
        _ => shelled_parasite_fell_intent(ascension),
    }
}

pub fn target_shelled_parasite_next_intent(
    move_history: &[u8],
    rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> MonsterIntent {
    let roll = rng.random_int(99);
    shelled_parasite_intent_from_target_roll(roll, move_history, rng, ascension)
}

pub fn target_shelled_parasite_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> MonsterIntent {
    shelled_parasite_intent_from_target_roll(roll, move_history, rng, ascension)
}

fn shelled_parasite_intent_from_target_roll(
    roll: i32,
    move_history: &[u8],
    rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> MonsterIntent {
    if move_history.is_empty() {
        if ascension >= 17 {
            return shelled_parasite_fell_intent(ascension);
        }
        return if rng.random_bool() {
            shelled_parasite_double_strike_intent(ascension)
        } else {
            MonsterIntent::AttackHealSelf {
                damage: shelled_parasite_suck_damage(ascension),
            }
        };
    }

    if roll < 20 {
        if !last_move(move_history, 1) {
            return shelled_parasite_fell_intent(ascension);
        }
        return shelled_parasite_intent_from_target_roll(
            rng.random_int_range(20, 99),
            move_history,
            rng,
            ascension,
        );
    }
    if roll < 60 {
        if !last_two_moves(move_history, 2) {
            return shelled_parasite_double_strike_intent(ascension);
        }
        return MonsterIntent::AttackHealSelf {
            damage: shelled_parasite_suck_damage(ascension),
        };
    }
    if !last_two_moves(move_history, 3) {
        return MonsterIntent::AttackHealSelf {
            damage: shelled_parasite_suck_damage(ascension),
        };
    }
    shelled_parasite_double_strike_intent(ascension)
}

fn shelled_parasite_double_strike_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: shelled_parasite_double_strike_damage(ascension),
        hits: SHELLED_PARASITE_DOUBLE_STRIKE_HITS,
    }
}

/// Reproduces `WrithingMass.getMove`, including the extra RNG consumed by its
/// recursive rejection branches. `first_move` is explicit because Compulsive
/// can reroll the intent before the monster has executed its first turn.
#[must_use]
pub fn target_writhing_mass_next_intent_from_roll(
    first_move: bool,
    move_history: &[u8],
    used_mega_debuff: bool,
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    if first_move {
        return if roll < 33 {
            writhing_mass_multi_hit_intent(ascension)
        } else if roll < 66 {
            writhing_mass_attack_block_intent(ascension)
        } else {
            writhing_mass_attack_debuff_intent(ascension)
        };
    }

    if roll < 10 {
        if !last_move(move_history, 0) {
            return writhing_mass_big_hit_intent(ascension);
        }
        let reroll = rng.random_int_range(10, 99);
        return target_writhing_mass_next_intent_from_roll(
            false,
            move_history,
            used_mega_debuff,
            reroll,
            rng,
            ascension,
        );
    }
    if roll < 20 {
        if !used_mega_debuff && !last_move(move_history, 4) {
            return MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 };
        }
        if rng.random_float() < 0.1 {
            return writhing_mass_big_hit_intent(ascension);
        }
        let reroll = rng.random_int_range(20, 99);
        return target_writhing_mass_next_intent_from_roll(
            false,
            move_history,
            used_mega_debuff,
            reroll,
            rng,
            ascension,
        );
    }
    if roll < 40 {
        if !last_move(move_history, 3) {
            return writhing_mass_attack_debuff_intent(ascension);
        }
        let reroll = if rng.random_float() < 0.4 {
            rng.random_int(19)
        } else {
            rng.random_int_range(40, 99)
        };
        return target_writhing_mass_next_intent_from_roll(
            false,
            move_history,
            used_mega_debuff,
            reroll,
            rng,
            ascension,
        );
    }
    if roll < 70 {
        if !last_move(move_history, 1) {
            return writhing_mass_multi_hit_intent(ascension);
        }
        if rng.random_float() < 0.3 {
            return writhing_mass_attack_block_intent(ascension);
        }
        let reroll = rng.random_int(39);
        return target_writhing_mass_next_intent_from_roll(
            false,
            move_history,
            used_mega_debuff,
            reroll,
            rng,
            ascension,
        );
    }
    if !last_move(move_history, 2) {
        return writhing_mass_attack_block_intent(ascension);
    }
    let reroll = rng.random_int(69);
    target_writhing_mass_next_intent_from_roll(
        false,
        move_history,
        used_mega_debuff,
        reroll,
        rng,
        ascension,
    )
}

fn writhing_mass_big_hit_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: asc_damage(
            ascension,
            WRITHING_MASS_BIG_HIT_DAMAGE,
            WRITHING_MASS_A2_BIG_HIT_DAMAGE,
            2,
        ),
    }
}

fn writhing_mass_multi_hit_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: asc_damage(
            ascension,
            WRITHING_MASS_MULTI_HIT_DAMAGE,
            WRITHING_MASS_A2_MULTI_HIT_DAMAGE,
            2,
        ),
        hits: WRITHING_MASS_MULTI_HIT_HITS,
    }
}

fn writhing_mass_attack_block_intent(ascension: u8) -> MonsterIntent {
    let damage = asc_damage(
        ascension,
        WRITHING_MASS_ATTACK_BLOCK_DAMAGE,
        WRITHING_MASS_A2_ATTACK_BLOCK_DAMAGE,
        2,
    );
    MonsterIntent::AttackAndBlock {
        damage,
        block: damage,
    }
}

fn writhing_mass_attack_debuff_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
        damage: asc_damage(
            ascension,
            WRITHING_MASS_ATTACK_DEBUFF_DAMAGE,
            WRITHING_MASS_A2_ATTACK_DEBUFF_DAMAGE,
            2,
        ),
        weak: 2,
        vulnerable: 2,
    }
}

#[must_use]
pub fn target_move_byte(content_id: ContentId, intent: MonsterIntent) -> Option<u8> {
    if content_id == TIME_EATER_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(2),
            MonsterIntent::AttackAndBlock { .. } => Some(3),
            MonsterIntent::Attack { .. } => Some(4),
            MonsterIntent::StrengthSelf { amount: 0 } => Some(5),
            _ => None,
        };
    }
    if content_id == AWAKENED_ONE_ID {
        return match intent {
            MonsterIntent::Attack {
                damage: AWAKENED_ONE_SLASH_DAMAGE,
            } => Some(1),
            MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_SOUL_STRIKE_DAMAGE,
                hits: AWAKENED_ONE_SOUL_STRIKE_HITS,
            } => Some(2),
            MonsterIntent::Stun => Some(3),
            MonsterIntent::Attack {
                damage: AWAKENED_ONE_DARK_ECHO_DAMAGE,
            } => Some(5),
            MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_TACKLE_DAMAGE,
                hits: AWAKENED_ONE_TACKLE_HITS,
            } => Some(8),
            MonsterIntent::AttackAddVoidToDraw { .. } => Some(6),
            _ => None,
        };
    }
    if content_id == CULTIST_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Ritual { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == HEXAGHOST_ID {
        return match intent {
            MonsterIntent::Stun => Some(5),
            MonsterIntent::AttackMultiple { hits: 6, .. } => Some(1),
            MonsterIntent::AttackMultiple { .. } => Some(2),
            MonsterIntent::StrengthAndBlock { .. } => Some(3),
            MonsterIntent::AddBurnToDiscard { .. } => Some(4),
            MonsterIntent::AttackMultipleUpgradeBurns { .. } => Some(6),
            _ => None,
        };
    }
    if content_id == BANDIT_POINTY_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            _ => None,
        };
    }
    if content_id == BANDIT_LEADER_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Stun => Some(2),
            MonsterIntent::AttackApplyPlayerWeak { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == BANDIT_BEAR_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::SiphonPlayer { .. } => Some(2),
            MonsterIntent::AttackAndBlock { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == WRITHING_MASS_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(0),
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::AttackAndBlock { .. } => Some(2),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. } => Some(3),
            MonsterIntent::ApplyPlayerFrailAndWeak { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == GUARDIAN_ID {
        return match intent {
            MonsterIntent::GuardianCloseUp { .. } => Some(1),
            MonsterIntent::Attack {
                damage: GUARDIAN_FIERCE_BASH_DAMAGE | GUARDIAN_A4_FIERCE_BASH_DAMAGE,
            } => Some(2),
            MonsterIntent::Attack { .. } => Some(3),
            MonsterIntent::AttackMultiple {
                damage: GUARDIAN_DEFENSIVE_COMBO_DAMAGE,
                hits: 2,
            } => Some(4),
            MonsterIntent::AttackMultiple { .. } => Some(5),
            MonsterIntent::Block { .. } => Some(6),
            MonsterIntent::ApplyPlayerWeak { .. }
            | MonsterIntent::ApplyPlayerFrailWeakVulnerable { .. } => Some(7),
            _ => None,
        };
    }
    if content_id == GREMLIN_NOB_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::AttackApplyPlayerVulnerable { .. } => Some(2),
            MonsterIntent::StrengthSelf { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == CHOSEN_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::ApplyPlayerWeakStrengthSelf { .. } => Some(2),
            MonsterIntent::AttackApplyPlayerVulnerable { .. } => Some(3),
            MonsterIntent::ApplyPlayerHex { .. } => Some(4),
            MonsterIntent::AttackMultiple { .. } => Some(5),
            _ => None,
        };
    }
    if content_id == CHAMP_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::StrengthAndBlock { .. } => Some(2),
            MonsterIntent::AttackMultiple { .. } => Some(3),
            MonsterIntent::AttackApplyPlayerVulnerable { .. }
            | MonsterIntent::AttackApplyPlayerFrailAndVulnerable { .. } => Some(4),
            MonsterIntent::StrengthSelf { amount } => {
                if amount >= 6 {
                    Some(7)
                } else {
                    Some(5)
                }
            }
            MonsterIntent::ApplyPlayerWeak { .. }
            | MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 0,
                weak: 2,
                vulnerable: 2,
            } => Some(6),
            _ => None,
        };
    }
    if content_id == SNAKE_PLANT_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::ApplyPlayerFrailAndWeak { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == SNECKO_ID {
        return match intent {
            MonsterIntent::ApplyPlayerConfusion => Some(1),
            MonsterIntent::Attack { .. } => Some(2),
            MonsterIntent::AttackApplyPlayerVulnerable { .. }
            | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == BOOK_OF_STABBING_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::Attack { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == SHELLED_PARASITE_ID {
        return match intent {
            MonsterIntent::Attack { .. } | MonsterIntent::AttackApplyPlayerFrail { .. } => Some(1),
            MonsterIntent::AttackMultiple { .. } => Some(2),
            MonsterIntent::AttackHealSelf { .. } => Some(3),
            MonsterIntent::Stun => Some(4),
            _ => None,
        };
    }
    if content_id == BYRD_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::StrengthSelf { amount: 0 } => Some(2),
            MonsterIntent::StrengthSelf { .. } => Some(6),
            MonsterIntent::Attack {
                damage: BYRD_SWOOP_DAMAGE | BYRD_A2_SWOOP_DAMAGE,
            } => Some(3),
            MonsterIntent::Attack { .. } => Some(5),
            MonsterIntent::Stun => Some(4),
            _ => None,
        };
    }
    if content_id == JAW_WORM_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::StrengthAndBlock { .. } => Some(2),
            MonsterIntent::AttackAndBlock { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == LAGAVULIN_ID {
        return match intent {
            MonsterIntent::SiphonPlayer { .. } => Some(1),
            MonsterIntent::Attack { .. } => Some(3),
            MonsterIntent::Stun => Some(4),
            MonsterIntent::Sleep => Some(5),
            _ => None,
        };
    }
    if content_id == SPHERIC_GUARDIAN_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            MonsterIntent::AttackAndBlock { .. } => Some(3),
            MonsterIntent::AttackApplyPlayerFrail { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == CENTURION_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            MonsterIntent::AttackMultiple { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == HEALER_ID {
        return match intent {
            MonsterIntent::AttackApplyPlayerFrail { .. } => Some(1),
            MonsterIntent::HealAllMonsters { .. } => Some(2),
            MonsterIntent::StrengthAllMonsters { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == FUNGI_BEAST_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::StrengthSelf { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == SLAVER_BLUE_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::AttackApplyPlayerWeak { .. }
            | MonsterIntent::AttackApplyPlayerWeakAndVulnerable { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == SLAVER_RED_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::ApplyPlayerEntangled { .. } => Some(2),
            MonsterIntent::AttackApplyPlayerVulnerable { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == TASKMASTER_ID {
        return match intent {
            MonsterIntent::AttackAddWoundsToDiscard { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == GREMLIN_LEADER_ID {
        return match intent {
            MonsterIntent::SummonGremlins { .. } => Some(2),
            MonsterIntent::EncourageGremlins { .. } => Some(3),
            MonsterIntent::AttackMultiple { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == DAGGER_ID {
        return match intent {
            MonsterIntent::AttackAddWoundsToDiscard { .. } => Some(1),
            MonsterIntent::Attack { damage } if damage == DAGGER_EXPLODE_DAMAGE => Some(2),
            _ => None,
        };
    }
    if content_id == REPTOMANCER_ID {
        return match intent {
            MonsterIntent::AttackMultipleApplyPlayerWeak { .. } => Some(1),
            MonsterIntent::SummonGremlins { .. } => Some(2),
            MonsterIntent::Attack { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == GREMLIN_WARRIOR_ID || content_id == GREMLIN_THIEF_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            _ => None,
        };
    }
    if content_id == GREMLIN_FAT_ID {
        return match intent {
            MonsterIntent::AttackApplyPlayerWeak { .. }
            | MonsterIntent::AttackApplyPlayerFrailAndWeak { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == GREMLIN_TSUNDERE_ID {
        return match intent {
            MonsterIntent::Block { .. } => Some(1),
            MonsterIntent::Attack { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == GREMLIN_WIZARD_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == BRONZE_AUTOMATON_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::Attack { .. } => Some(2),
            MonsterIntent::Stun => Some(3),
            MonsterIntent::SummonGremlins { .. } => Some(4),
            MonsterIntent::StrengthAndBlock { .. } => Some(5),
            _ => None,
        };
    }
    if content_id == BRONZE_ORB_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            MonsterIntent::SiphonPlayer { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == THE_COLLECTOR_ID {
        return match intent {
            MonsterIntent::SummonGremlins { .. } => Some(1),
            MonsterIntent::SummonCollectorTorchHeads { .. } => Some(5),
            MonsterIntent::Attack { .. } => Some(2),
            MonsterIntent::StrengthAndBlock { .. } => Some(3),
            MonsterIntent::ApplyPlayerFrailAndWeak { .. }
            | MonsterIntent::ApplyPlayerFrailWeakVulnerable { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == TORCH_HEAD_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            _ => None,
        };
    }
    if content_id == ORB_WALKER_ID {
        return match intent {
            MonsterIntent::AddBurnToDiscardAndDraw { .. } => Some(1),
            MonsterIntent::Attack { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == DARKLING_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::Block { .. } | MonsterIntent::StrengthAndBlock { .. } => Some(2),
            MonsterIntent::Attack { damage: 0 } => Some(4),
            MonsterIntent::Attack { .. } => Some(3),
            MonsterIntent::Stun => Some(5),
            _ => None,
        };
    }
    if content_id == NEMESIS_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(2),
            MonsterIntent::Attack { .. } => Some(3),
            MonsterIntent::AddBurnToDiscard { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == EXPLODER_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Stun => Some(2),
            _ => None,
        };
    }
    if content_id == SPIKER_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::StrengthAndBlock { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == REPULSOR_ID {
        return match intent {
            MonsterIntent::AddDazedToDiscard { .. } | MonsterIntent::AddDazedToDraw { .. } => {
                Some(1)
            }
            MonsterIntent::Attack { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == TRANSIENT_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            _ => None,
        };
    }
    if content_id == MAW_ID {
        return match intent {
            MonsterIntent::ApplyPlayerFrailAndWeak { .. } => Some(2),
            MonsterIntent::Attack {
                damage: MAW_SLAM_DAMAGE | MAW_A2_SLAM_DAMAGE,
            } => Some(3),
            MonsterIntent::StrengthSelf { .. } => Some(4),
            MonsterIntent::Attack { damage } if damage == MAW_NOM_DAMAGE => Some(5),
            MonsterIntent::AttackMultiple {
                damage: MAW_NOM_DAMAGE,
                ..
            } => Some(5),
            _ => None,
        };
    }
    if content_id == SPIRE_GROWTH_ID {
        return match intent {
            MonsterIntent::Attack {
                damage: SPIRE_GROWTH_QUICK_TACKLE_DAMAGE | SPIRE_GROWTH_A2_QUICK_TACKLE_DAMAGE,
            } => Some(1),
            MonsterIntent::ApplyPlayerConstricted { .. } => Some(2),
            MonsterIntent::Attack {
                damage: SPIRE_GROWTH_SMASH_DAMAGE | SPIRE_GROWTH_A2_SMASH_DAMAGE,
            } => Some(3),
            _ => None,
        };
    }
    if content_id == GIANT_HEAD_ID {
        return match intent {
            MonsterIntent::ApplyPlayerWeak { .. } => Some(1),
            MonsterIntent::Attack {
                damage: GIANT_HEAD_COUNT_DAMAGE,
            } => Some(3),
            MonsterIntent::Attack { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == DECA_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. }
            | MonsterIntent::AttackMultipleAddDazedToDiscard { .. } => Some(0),
            MonsterIntent::Block { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == DONU_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(0),
            MonsterIntent::StrengthAllMonsters { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == LOOTER_ID {
        return match intent {
            MonsterIntent::AttackStealGold { damage, .. } if damage >= LOOTER_LUNGE_DAMAGE => {
                Some(4)
            }
            MonsterIntent::AttackStealGold { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            MonsterIntent::Escape => Some(3),
            _ => None,
        };
    }
    if content_id == MUGGER_ID {
        return match intent {
            MonsterIntent::AttackStealGold { damage, .. } if damage >= MUGGER_BIG_SWIPE_DAMAGE => {
                Some(4)
            }
            MonsterIntent::AttackStealGold { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            MonsterIntent::Escape => Some(3),
            _ => None,
        };
    }
    if content_id == RED_LOUSE_ID || content_id == GREEN_LOUSE_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(LOUSE_ATTACK_MOVE),
            MonsterIntent::StrengthAndBlock { .. } | MonsterIntent::ApplyPlayerWeak { .. } => {
                Some(LOUSE_NON_ATTACK_MOVE)
            }
            _ => None,
        };
    }
    if content_id == ACID_SLIME_ID {
        return match intent {
            MonsterIntent::AttackAddSlimedToDiscard { .. } => Some(1),
            MonsterIntent::Attack { damage } if damage >= ACID_SLIME_M_NORMAL_TACKLE_DAMAGE => {
                Some(2)
            }
            MonsterIntent::ApplyPlayerWeak { .. } => Some(4),
            MonsterIntent::SummonGremlins { .. } => Some(3),
            MonsterIntent::Attack { .. } => Some(1),
            _ => None,
        };
    }
    if content_id == SPIKE_SLIME_ID {
        return match intent {
            MonsterIntent::AttackAddSlimedToDiscard { .. } => Some(1),
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::SummonGremlins { .. } => Some(3),
            MonsterIntent::ApplyPlayerFrailAndWeak { .. } => Some(4),
            MonsterIntent::ApplyPlayerWeak { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == SENTRY_ID {
        return match intent {
            MonsterIntent::AddDazedToDiscard { .. } => Some(3),
            MonsterIntent::Attack { .. } => Some(4),
            _ => None,
        };
    }
    if content_id == SLIME_BOSS_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Stun => Some(2),
            MonsterIntent::AddSlimedToDiscard { .. } => Some(4),
            MonsterIntent::SummonGremlins { .. } => Some(3),
            _ => None,
        };
    }
    None
}

#[must_use]
pub fn target_move_byte_for_monster(monster: &MonsterState) -> Option<u8> {
    if monster.content_id == ACID_SLIME_ID
        && (monster.slime_size == Some(SlimeSize::Small)
            || (monster.slime_size.is_none() && monster.max_hp <= ACID_SLIME_S_A7_HP_RANGE.max))
        && matches!(monster.intent, MonsterIntent::ApplyPlayerWeak { .. })
    {
        return Some(2);
    }
    target_move_byte(monster.content_id, monster.intent)
}

pub fn record_target_move(monster: &mut MonsterState) {
    if let Some(move_byte) = target_move_byte_for_monster(monster) {
        monster.move_history.push(move_byte);
    }
}

/// Reproduces `TimeEater.getMove`, including the recursive AI rerolls which
/// consume additional monster-RNG draws in the target game.
#[must_use]
pub fn target_time_eater_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    current_hp: i32,
    max_hp: i32,
    ascension: u8,
    rng: &mut StsRng,
) -> MonsterIntent {
    if current_hp < max_hp / 2 && !move_history.contains(&5) {
        return MonsterIntent::StrengthSelf { amount: 0 };
    }

    let reverb_damage = if ascension >= 4 {
        TIME_EATER_A4_REVERBERATE_DAMAGE
    } else {
        TIME_EATER_REVERBERATE_DAMAGE
    };
    let head_slam_damage = if ascension >= 4 {
        TIME_EATER_A4_HEAD_SLAM_DAMAGE
    } else {
        TIME_EATER_HEAD_SLAM_DAMAGE
    };

    if roll < 45 {
        if !last_two_moves(move_history, 2) {
            return MonsterIntent::AttackMultiple {
                damage: reverb_damage,
                hits: 3,
            };
        }
        return target_time_eater_next_intent_from_roll(
            move_history,
            rng.random_int_range(50, 99),
            current_hp,
            max_hp,
            ascension,
            rng,
        );
    }
    if roll < 80 {
        if !last_move(move_history, 4) {
            return MonsterIntent::Attack {
                damage: head_slam_damage,
            };
        }
        return if rng.random_float() < 0.66 {
            MonsterIntent::AttackMultiple {
                damage: reverb_damage,
                hits: 3,
            }
        } else {
            MonsterIntent::AttackAndBlock {
                damage: 0,
                block: TIME_EATER_RIPPLE_BLOCK,
            }
        };
    }
    if !last_move(move_history, 3) {
        return MonsterIntent::AttackAndBlock {
            damage: 0,
            block: TIME_EATER_RIPPLE_BLOCK,
        };
    }
    target_time_eater_next_intent_from_roll(
        move_history,
        rng.random_int(74),
        current_hp,
        max_hp,
        ascension,
        rng,
    )
}

fn last_move(move_history: &[u8], move_byte: u8) -> bool {
    move_history.last().copied() == Some(move_byte)
}

fn last_two_moves(move_history: &[u8], move_byte: u8) -> bool {
    move_history
        .iter()
        .rev()
        .take(2)
        .copied()
        .eq([move_byte, move_byte])
}

fn last_move_before(move_history: &[u8], move_byte: u8) -> bool {
    move_history.iter().rev().nth(1).copied() == Some(move_byte)
}

#[must_use]
pub fn target_spheric_guardian_next_intent_from_roll(
    moves_executed: u32,
    move_history: &[u8],
    ascension: u8,
) -> MonsterIntent {
    spheric_guardian_intent(moves_executed, move_history, ascension)
}

#[must_use]
fn book_of_stabbing_stab_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        BOOK_OF_STABBING_A3_STAB_DAMAGE
    } else {
        BOOK_OF_STABBING_STAB_DAMAGE
    }
}

fn book_of_stabbing_big_stab_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        BOOK_OF_STABBING_A3_BIG_STAB_DAMAGE
    } else {
        BOOK_OF_STABBING_BIG_STAB_DAMAGE
    }
}

fn book_of_stabbing_representative_stab_hits(moves_executed: u32, ascension: u8) -> i32 {
    match moves_executed {
        0 => 2,
        1 => 3,
        4 if ascension >= 18 => 6,
        4 => 5,
        3 if ascension >= 18 => 5,
        3 => 4,
        _ => (moves_executed + 2) as i32,
    }
}

#[must_use]
fn book_of_stabbing_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 | 1 | 3 | 4 => MonsterIntent::AttackMultiple {
            damage: book_of_stabbing_stab_damage(ascension),
            hits: book_of_stabbing_representative_stab_hits(moves_executed, ascension),
        },
        _ => MonsterIntent::Attack {
            damage: book_of_stabbing_big_stab_damage(ascension),
        },
    }
}

#[must_use]
pub fn target_book_of_stabbing_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    let mut stab_count = book_of_stabbing_current_stab_count_from_history(move_history, ascension);
    target_book_of_stabbing_next_intent_from_roll_with_stab_count(
        move_history,
        &mut stab_count,
        roll,
        ascension,
    )
}

#[must_use]
pub fn target_book_of_stabbing_next_intent_from_roll_with_stab_count(
    move_history: &[u8],
    stab_count: &mut i32,
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if roll < 15 {
        if last_move(move_history, 2) {
            *stab_count += 1;
            return MonsterIntent::AttackMultiple {
                damage: book_of_stabbing_stab_damage(ascension),
                hits: *stab_count,
            };
        }
        if ascension >= 18 {
            *stab_count += 1;
        }
        return MonsterIntent::Attack {
            damage: book_of_stabbing_big_stab_damage(ascension),
        };
    }
    if last_two_moves(move_history, 1) {
        if ascension >= 18 {
            *stab_count += 1;
        }
        return MonsterIntent::Attack {
            damage: book_of_stabbing_big_stab_damage(ascension),
        };
    }
    *stab_count += 1;
    MonsterIntent::AttackMultiple {
        damage: book_of_stabbing_stab_damage(ascension),
        hits: *stab_count,
    }
}

fn book_of_stabbing_current_stab_count_from_history(move_history: &[u8], ascension: u8) -> i32 {
    let previous_stabs = move_history
        .iter()
        .filter(|move_byte| **move_byte == 1)
        .count() as i32;
    let big_stabs_that_increment = if ascension >= 18 {
        move_history
            .iter()
            .filter(|move_byte| **move_byte == 2)
            .count() as i32
    } else {
        0
    };
    1 + previous_stabs + big_stabs_that_increment
}

#[must_use]
fn taskmaster_intent() -> MonsterIntent {
    MonsterIntent::AttackAddWoundsToDiscard {
        damage: TASKMASTER_SCOURING_WHIP_DAMAGE,
        count: TASKMASTER_WOUNDS,
    }
}

pub fn target_taskmaster_wound_count(ascension: u8) -> i32 {
    if ascension >= 18 {
        TASKMASTER_A18_WOUNDS
    } else if ascension >= 3 {
        TASKMASTER_A3_WOUNDS
    } else {
        TASKMASTER_WOUNDS
    }
}

#[must_use]
fn gremlin_leader_strength(ascension: u8) -> i32 {
    if ascension >= 18 {
        GREMLIN_LEADER_A18_STRENGTH
    } else if ascension >= 3 {
        GREMLIN_LEADER_A3_STRENGTH
    } else {
        GREMLIN_LEADER_STRENGTH
    }
}

#[must_use]
fn gremlin_leader_block(ascension: u8) -> i32 {
    if ascension >= 18 {
        GREMLIN_LEADER_A18_BLOCK
    } else {
        GREMLIN_LEADER_BLOCK
    }
}

#[must_use]
fn gremlin_leader_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::EncourageGremlins {
            strength: gremlin_leader_strength(ascension),
            block: gremlin_leader_block(ascension),
        },
        1 => MonsterIntent::AttackMultiple {
            damage: GREMLIN_LEADER_STAB_DAMAGE,
            hits: GREMLIN_LEADER_STAB_HITS,
        },
        _ => MonsterIntent::SummonGremlins { count: 2 },
    }
}

#[must_use]
pub fn target_gremlin_leader_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
    alive_gremlin_count: usize,
    ascension: u8,
) -> MonsterIntent {
    if alive_gremlin_count == 0 {
        if roll < 75 {
            if !last_move(move_history, 2) {
                return gremlin_leader_rally_intent();
            }
            return gremlin_leader_stab_intent();
        }
        if !last_move(move_history, 4) {
            return gremlin_leader_stab_intent();
        }
        return gremlin_leader_rally_intent();
    }

    if alive_gremlin_count < 2 {
        if roll < 50 {
            if !last_move(move_history, 2) {
                return gremlin_leader_rally_intent();
            }
            let replacement_roll = rng.random_int_range(50, 99);
            return target_gremlin_leader_next_intent_from_roll(
                move_history,
                replacement_roll,
                rng,
                alive_gremlin_count,
                ascension,
            );
        }
        if roll < 80 {
            if !last_move(move_history, 3) {
                return gremlin_leader_encourage_intent(ascension);
            }
            return gremlin_leader_stab_intent();
        }
        if !last_move(move_history, 4) {
            return gremlin_leader_stab_intent();
        }
        let replacement_roll = rng.random_int(80);
        return target_gremlin_leader_next_intent_from_roll(
            move_history,
            replacement_roll,
            rng,
            alive_gremlin_count,
            ascension,
        );
    }

    if roll < 66 {
        if !last_move(move_history, 3) {
            return gremlin_leader_encourage_intent(ascension);
        }
        return gremlin_leader_stab_intent();
    }
    if !last_move(move_history, 4) {
        return gremlin_leader_stab_intent();
    }
    gremlin_leader_encourage_intent(ascension)
}

fn gremlin_leader_rally_intent() -> MonsterIntent {
    MonsterIntent::SummonGremlins { count: 2 }
}

fn gremlin_leader_encourage_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::EncourageGremlins {
        strength: gremlin_leader_strength(ascension),
        block: gremlin_leader_block(ascension),
    }
}

fn gremlin_leader_stab_intent() -> MonsterIntent {
    MonsterIntent::AttackMultiple {
        damage: GREMLIN_LEADER_STAB_DAMAGE,
        hits: GREMLIN_LEADER_STAB_HITS,
    }
}

#[must_use]
pub fn target_reptomancer_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    can_spawn: bool,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    if move_history.is_empty() {
        return reptomancer_spawn_intent(ascension);
    }

    if roll < 33 {
        if !last_move(move_history, 1) {
            return reptomancer_snake_strike_intent(ascension);
        }
        let replacement_roll = rng.random_int_range(33, 99);
        return target_reptomancer_next_intent_from_roll(
            move_history,
            replacement_roll,
            can_spawn,
            rng,
            ascension,
        );
    }
    if roll < 66 {
        if !last_two_moves(move_history, 2) {
            if can_spawn {
                return reptomancer_spawn_intent(ascension);
            }
            return reptomancer_snake_strike_intent(ascension);
        }
        return reptomancer_snake_strike_intent(ascension);
    }
    if !last_move(move_history, 3) {
        return reptomancer_big_bite_intent(ascension);
    }
    let replacement_roll = rng.random_int(65);
    target_reptomancer_next_intent_from_roll(
        move_history,
        replacement_roll,
        can_spawn,
        rng,
        ascension,
    )
}

fn reptomancer_spawn_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::SummonGremlins {
        count: if ascension >= 18 { 2 } else { 1 },
    }
}

fn reptomancer_snake_strike_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackMultipleApplyPlayerWeak {
        damage: asc_damage(
            ascension,
            REPTOMANCER_SNAKE_STRIKE_DAMAGE,
            REPTOMANCER_A3_SNAKE_STRIKE_DAMAGE,
            3,
        ),
        hits: REPTOMANCER_SNAKE_STRIKE_HITS,
        weak: 1,
    }
}

fn reptomancer_big_bite_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: asc_damage(
            ascension,
            REPTOMANCER_BIG_BITE_DAMAGE,
            REPTOMANCER_A3_BIG_BITE_DAMAGE,
            3,
        ),
    }
}

#[must_use]
fn gremlin_warrior_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        GREMLIN_WARRIOR_A2_SCRATCH_DAMAGE
    } else {
        GREMLIN_WARRIOR_SCRATCH_DAMAGE
    }
}

#[must_use]
fn gremlin_warrior_anger(ascension: u8) -> i32 {
    if ascension >= 17 {
        GREMLIN_WARRIOR_A17_ANGER
    } else {
        GREMLIN_WARRIOR_ANGER
    }
}

#[must_use]
fn gremlin_thief_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        GREMLIN_THIEF_A2_DAMAGE
    } else {
        GREMLIN_THIEF_DAMAGE
    }
}

#[must_use]
fn gremlin_fat_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        GREMLIN_FAT_A2_DAMAGE
    } else {
        GREMLIN_FAT_DAMAGE
    }
}

#[must_use]
fn gremlin_tsundere_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        GREMLIN_TSUNDERE_A17_BLOCK
    } else if ascension >= 7 {
        GREMLIN_TSUNDERE_A7_BLOCK
    } else {
        GREMLIN_TSUNDERE_BLOCK
    }
}

#[must_use]
fn gremlin_tsundere_bash_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        GREMLIN_TSUNDERE_A2_BASH_DAMAGE
    } else {
        GREMLIN_TSUNDERE_BASH_DAMAGE
    }
}

#[must_use]
fn gremlin_wizard_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        GREMLIN_WIZARD_A2_MAGIC_DAMAGE
    } else {
        GREMLIN_WIZARD_MAGIC_DAMAGE
    }
}

#[must_use]
pub(crate) fn source_backed_gremlin_leader_minion_intent(
    content_id: ContentId,
    moves_executed: u32,
    ascension: u8,
) -> Option<MonsterIntent> {
    Some(match content_id {
        GREMLIN_WARRIOR_ID => MonsterIntent::Attack {
            damage: gremlin_warrior_damage(ascension),
        },
        GREMLIN_THIEF_ID => MonsterIntent::Attack {
            damage: gremlin_thief_damage(ascension),
        },
        GREMLIN_FAT_ID if ascension >= 17 => MonsterIntent::AttackApplyPlayerFrailAndWeak {
            damage: gremlin_fat_damage(ascension),
            frail: GREMLIN_FAT_WEAK,
            weak: GREMLIN_FAT_WEAK,
        },
        GREMLIN_FAT_ID => MonsterIntent::AttackApplyPlayerWeak {
            damage: gremlin_fat_damage(ascension),
            weak: GREMLIN_FAT_WEAK,
        },
        GREMLIN_TSUNDERE_ID if moves_executed > 0 => MonsterIntent::Attack {
            damage: gremlin_tsundere_bash_damage(ascension),
        },
        GREMLIN_TSUNDERE_ID => MonsterIntent::Block {
            block: gremlin_tsundere_block(ascension),
        },
        GREMLIN_WIZARD_ID => {
            target_gremlin_wizard_direct_next_intent_after_turn(moves_executed, ascension)
        }
        _ => return None,
    })
}

#[must_use]
pub fn target_gremlin_wizard_direct_next_intent_after_turn(
    moves_executed: u32,
    ascension: u8,
) -> MonsterIntent {
    if moves_executed >= 2 && (ascension >= 17 || moves_executed % 4 == 2) {
        MonsterIntent::Attack {
            damage: gremlin_wizard_damage(ascension),
        }
    } else {
        MonsterIntent::Block { block: 0 }
    }
}

pub(crate) fn is_unsupported_approximate_monster_intent(content_id: ContentId) -> bool {
    matches!(
        content_id,
        CORRUPT_HEART_ID | SPIRE_SHIELD_ID | SPIRE_SPEAR_ID
    )
}

fn asc_damage(ascension: u8, base: i32, upgraded: i32, threshold: u8) -> i32 {
    if ascension >= threshold {
        upgraded
    } else {
        base
    }
}

fn spire_growth_max_hp(ascension: u8) -> i32 {
    if ascension >= 7 {
        SPIRE_GROWTH_A7_HP
    } else {
        SPIRE_GROWTH_HP
    }
}

fn giant_head_max_hp(ascension: u8) -> i32 {
    if ascension >= 8 {
        GIANT_HEAD_A8_HP
    } else {
        GIANT_HEAD_HP
    }
}

fn nemesis_max_hp(ascension: u8) -> i32 {
    if ascension >= 8 {
        NEMESIS_A8_HP
    } else {
        NEMESIS_HP
    }
}

#[must_use]
fn source_backed_complex_monster_intent(
    content_id: ContentId,
    moves_executed: u32,
    ascension: u8,
) -> Option<MonsterIntent> {
    Some(match content_id {
        BANDIT_BEAR_ID => {
            if moves_executed == 0 {
                return Some(MonsterIntent::SiphonPlayer {
                    strength: 0,
                    dexterity: if ascension >= 17 { 4 } else { 2 },
                });
            }
            if moves_executed % 2 == 1 {
                return Some(MonsterIntent::AttackAndBlock {
                    damage: asc_damage(
                        ascension,
                        BANDIT_BEAR_LUNGE_DAMAGE,
                        BANDIT_BEAR_A2_LUNGE_DAMAGE,
                        2,
                    ),
                    block: BANDIT_BEAR_LUNGE_BLOCK,
                });
            }
            MonsterIntent::Attack {
                damage: asc_damage(
                    ascension,
                    BANDIT_BEAR_MAUL_DAMAGE,
                    BANDIT_BEAR_A2_MAUL_DAMAGE,
                    2,
                ),
            }
        }
        BANDIT_POINTY_ID => MonsterIntent::AttackMultiple {
            damage: BANDIT_POINTY_DAMAGE,
            hits: BANDIT_POINTY_HITS,
        },
        BANDIT_LEADER_ID => {
            if moves_executed == 0 {
                return Some(MonsterIntent::Stun);
            }
            if moves_executed % 2 == 1 {
                return Some(MonsterIntent::AttackApplyPlayerWeak {
                    damage: asc_damage(
                        ascension,
                        BANDIT_LEADER_AGONIZE_DAMAGE,
                        BANDIT_LEADER_A2_AGONIZE_DAMAGE,
                        2,
                    ),
                    weak: if ascension >= 17 {
                        3
                    } else {
                        BANDIT_LEADER_WEAK
                    },
                });
            }
            MonsterIntent::Attack {
                damage: asc_damage(
                    ascension,
                    BANDIT_LEADER_SLASH_DAMAGE,
                    BANDIT_LEADER_A2_SLASH_DAMAGE,
                    2,
                ),
            }
        }
        CHAMP_ID => match moves_executed % 6 {
            0 => MonsterIntent::Attack {
                damage: asc_damage(
                    ascension,
                    CHAMP_HEAVY_SLASH_DAMAGE,
                    CHAMP_A4_HEAVY_SLASH_DAMAGE,
                    4,
                ),
            },
            1 => MonsterIntent::StrengthAndBlock {
                strength: CHAMP_DEFENSIVE_METALLICIZE,
                block: CHAMP_DEFENSIVE_BLOCK,
            },
            2 => MonsterIntent::AttackMultiple {
                damage: CHAMP_EXECUTE_DAMAGE,
                hits: CHAMP_EXECUTE_HITS,
            },
            3 => MonsterIntent::AttackApplyPlayerVulnerable {
                damage: asc_damage(
                    ascension,
                    CHAMP_FACE_SLAP_DAMAGE,
                    CHAMP_A4_FACE_SLAP_DAMAGE,
                    4,
                ),
                vulnerable: 2,
            },
            4 => MonsterIntent::StrengthSelf { amount: 3 },
            _ => MonsterIntent::ApplyPlayerWeak { amount: 2 },
        },
        THE_COLLECTOR_ID => match moves_executed % 4 {
            0 => MonsterIntent::SummonGremlins { count: 2 },
            1 => MonsterIntent::Attack {
                damage: asc_damage(
                    ascension,
                    COLLECTOR_FIREBALL_DAMAGE,
                    COLLECTOR_A4_FIREBALL_DAMAGE,
                    4,
                ),
            },
            2 => MonsterIntent::StrengthAndBlock {
                strength: 3,
                block: COLLECTOR_BUFF_BLOCK,
            },
            _ => MonsterIntent::ApplyPlayerFrailAndWeak { frail: 3, weak: 3 },
        },
        TORCH_HEAD_ID => MonsterIntent::Attack {
            damage: TORCH_HEAD_TACKLE_DAMAGE,
        },
        DAGGER_ID => match moves_executed % 2 {
            0 => MonsterIntent::AttackAddWoundsToDiscard {
                damage: DAGGER_WOUND_DAMAGE,
                count: 1,
            },
            _ => MonsterIntent::Attack {
                damage: DAGGER_EXPLODE_DAMAGE,
            },
        },
        DECA_ID => match moves_executed % 2 {
            0 => MonsterIntent::AttackMultipleAddDazedToDiscard {
                damage: asc_damage(ascension, DECA_BEAM_DAMAGE, DECA_A4_BEAM_DAMAGE, 4),
                hits: DECA_BEAM_HITS,
                count: 2,
            },
            _ => MonsterIntent::Block {
                block: DECA_PROTECTION_BLOCK,
            },
        },
        DONU_ID => match moves_executed % 2 {
            0 => MonsterIntent::StrengthAllMonsters { amount: 3 },
            _ => MonsterIntent::AttackMultiple {
                damage: asc_damage(ascension, DONU_BEAM_DAMAGE, DONU_A4_BEAM_DAMAGE, 4),
                hits: DONU_BEAM_HITS,
            },
        },
        EXPLODER_ID => match moves_executed % 2 {
            0 => MonsterIntent::Attack {
                damage: asc_damage(
                    ascension,
                    EXPLODER_ATTACK_DAMAGE,
                    EXPLODER_A2_ATTACK_DAMAGE,
                    2,
                ),
            },
            _ => MonsterIntent::Stun,
        },
        GIANT_HEAD_ID => {
            target_giant_head_next_intent_from_roll(moves_executed, &[], 99, ascension)
        }
        NEMESIS_ID => nemesis_burn_intent(ascension),
        REPTOMANCER_ID => {
            if moves_executed == 0 {
                reptomancer_spawn_intent(ascension)
            } else {
                reptomancer_big_bite_intent(ascension)
            }
        }
        REPULSOR_ID => match moves_executed % 2 {
            0 => MonsterIntent::AddDazedToDiscard { count: 2 },
            _ => MonsterIntent::Attack {
                damage: asc_damage(
                    ascension,
                    REPULSOR_ATTACK_DAMAGE,
                    REPULSOR_A2_ATTACK_DAMAGE,
                    2,
                ),
            },
        },
        SPIKER_ID => match moves_executed % 2 {
            0 => MonsterIntent::Attack {
                damage: asc_damage(ascension, SPIKER_ATTACK_DAMAGE, SPIKER_A2_ATTACK_DAMAGE, 2),
            },
            _ => MonsterIntent::StrengthAndBlock {
                strength: 0,
                block: SPIKER_THORNS,
            },
        },
        SPIRE_GROWTH_ID => {
            target_spire_growth_next_intent_from_roll(moves_executed, &[], 99, false, ascension)
        }
        MAW_ID => target_maw_next_intent_from_roll(moves_executed, &[], 99, ascension),
        TRANSIENT_ID => MonsterIntent::Attack {
            damage: asc_damage(
                ascension,
                TRANSIENT_ATTACK_DAMAGE,
                TRANSIENT_A4_ATTACK_DAMAGE,
                4,
            ),
        },
        WRITHING_MASS_ID => match moves_executed % 5 {
            0 => writhing_mass_big_hit_intent(ascension),
            1 => writhing_mass_multi_hit_intent(ascension),
            2 => writhing_mass_attack_block_intent(ascension),
            3 => writhing_mass_attack_debuff_intent(ascension),
            _ => MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 },
        },
        _ => return None,
    })
}

#[must_use]
fn bronze_automaton_flail_damage(ascension: u8) -> i32 {
    if ascension >= 4 {
        BRONZE_AUTOMATON_A4_FLAIL_DAMAGE
    } else {
        BRONZE_AUTOMATON_FLAIL_DAMAGE
    }
}

#[must_use]
fn bronze_automaton_hyper_beam_damage(ascension: u8) -> i32 {
    if ascension >= 4 {
        BRONZE_AUTOMATON_A4_HYPER_BEAM_DAMAGE
    } else {
        BRONZE_AUTOMATON_HYPER_BEAM_DAMAGE
    }
}

#[must_use]
fn bronze_automaton_boost_block(ascension: u8) -> i32 {
    if ascension >= 9 {
        BRONZE_AUTOMATON_A9_BOOST_BLOCK
    } else {
        BRONZE_AUTOMATON_BOOST_BLOCK
    }
}

#[must_use]
fn bronze_automaton_boost_strength(ascension: u8) -> i32 {
    if ascension >= 4 {
        BRONZE_AUTOMATON_A4_BOOST_STRENGTH
    } else {
        BRONZE_AUTOMATON_BOOST_STRENGTH
    }
}

#[must_use]
pub fn target_bronze_automaton_next_intent(
    moves_executed: u32,
    move_history: &[u8],
    ascension: u8,
) -> MonsterIntent {
    if moves_executed == 0 {
        return MonsterIntent::SummonGremlins { count: 2 };
    }
    if moves_executed % 6 == 5 {
        return MonsterIntent::Attack {
            damage: bronze_automaton_hyper_beam_damage(ascension),
        };
    }
    if last_move(move_history, 2) {
        if ascension >= 19 {
            return MonsterIntent::StrengthAndBlock {
                strength: bronze_automaton_boost_strength(ascension),
                block: bronze_automaton_boost_block(ascension),
            };
        }
        return MonsterIntent::Stun;
    }
    if last_move(move_history, 3) || last_move(move_history, 4) || last_move(move_history, 5) {
        return MonsterIntent::AttackMultiple {
            damage: bronze_automaton_flail_damage(ascension),
            hits: BRONZE_AUTOMATON_FLAIL_HITS,
        };
    }
    MonsterIntent::StrengthAndBlock {
        strength: bronze_automaton_boost_strength(ascension),
        block: bronze_automaton_boost_block(ascension),
    }
}

#[must_use]
fn bronze_automaton_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if moves_executed == 0 {
        return MonsterIntent::SummonGremlins { count: 2 };
    }
    match moves_executed % 6 {
        1 | 3 => MonsterIntent::AttackMultiple {
            damage: bronze_automaton_flail_damage(ascension),
            hits: BRONZE_AUTOMATON_FLAIL_HITS,
        },
        2 | 4 => MonsterIntent::StrengthAndBlock {
            strength: bronze_automaton_boost_strength(ascension),
            block: bronze_automaton_boost_block(ascension),
        },
        5 => MonsterIntent::Attack {
            damage: bronze_automaton_hyper_beam_damage(ascension),
        },
        0 if ascension >= 19 => MonsterIntent::StrengthAndBlock {
            strength: bronze_automaton_boost_strength(ascension),
            block: bronze_automaton_boost_block(ascension),
        },
        0 => MonsterIntent::Stun,
        _ => unreachable!("modulo six move index is covered"),
    }
}

#[must_use]
fn collector_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::SummonGremlins { count: 2 },
        1 => MonsterIntent::StrengthAndBlock {
            strength: THE_COLLECTOR_STRENGTH,
            block: THE_COLLECTOR_BLOCK,
        },
        2 => MonsterIntent::Attack { damage: 18 },
        _ => MonsterIntent::ApplyPlayerFrailWeakVulnerable {
            frail: 3,
            weak: 3,
            vulnerable: 3,
        },
    }
}

#[must_use]
pub fn target_collector_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    minion_dead: bool,
) -> MonsterIntent {
    if move_history.len() >= 3 && !move_history.contains(&4) {
        return MonsterIntent::ApplyPlayerFrailWeakVulnerable {
            frail: 3,
            weak: 3,
            vulnerable: 3,
        };
    }
    if roll <= 25 && minion_dead && !last_move(move_history, 5) {
        return MonsterIntent::SummonCollectorTorchHeads { count: 2 };
    }
    if roll <= 70 && !last_two_moves(move_history, 2) {
        return MonsterIntent::Attack { damage: 18 };
    }
    if !last_move(move_history, 3) {
        return MonsterIntent::StrengthAndBlock {
            strength: THE_COLLECTOR_STRENGTH,
            block: THE_COLLECTOR_BLOCK,
        };
    }
    MonsterIntent::Attack { damage: 18 }
}

#[must_use]
fn bronze_orb_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::SiphonPlayer {
            strength: 0,
            dexterity: 0,
        },
        1..=3 => MonsterIntent::Attack {
            damage: BRONZE_ORB_BEAM_DAMAGE,
        },
        _ => MonsterIntent::Block {
            block: BRONZE_ORB_SUPPORT_BEAM_BLOCK,
        },
    }
}

#[must_use]
pub fn target_bronze_orb_next_intent_from_roll(move_history: &[u8], roll: i32) -> MonsterIntent {
    if !move_history.contains(&3) && roll >= 25 {
        return MonsterIntent::SiphonPlayer {
            strength: 0,
            dexterity: 0,
        };
    }
    if roll >= 70 && !last_two_moves(move_history, 2) {
        return MonsterIntent::Block {
            block: BRONZE_ORB_SUPPORT_BEAM_BLOCK,
        };
    }
    if !last_two_moves(move_history, 1) {
        return MonsterIntent::Attack {
            damage: BRONZE_ORB_BEAM_DAMAGE,
        };
    }
    MonsterIntent::Block {
        block: BRONZE_ORB_SUPPORT_BEAM_BLOCK,
    }
}

#[must_use]
fn orb_walker_laser_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        ORB_WALKER_A2_LASER_DAMAGE
    } else {
        ORB_WALKER_LASER_DAMAGE
    }
}

#[must_use]
fn orb_walker_claw_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        ORB_WALKER_A2_CLAW_DAMAGE
    } else {
        ORB_WALKER_CLAW_DAMAGE
    }
}

#[must_use]
fn orb_walker_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => orb_walker_laser_intent(ascension),
        _ => MonsterIntent::Attack {
            damage: orb_walker_claw_damage(ascension),
        },
    }
}

#[must_use]
pub fn target_orb_walker_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if roll < 40 {
        if !last_two_moves(move_history, 2) {
            return MonsterIntent::Attack {
                damage: orb_walker_claw_damage(ascension),
            };
        }
        return orb_walker_laser_intent(ascension);
    }
    if !last_two_moves(move_history, 1) {
        return orb_walker_laser_intent(ascension);
    }
    MonsterIntent::Attack {
        damage: orb_walker_claw_damage(ascension),
    }
}

fn orb_walker_laser_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AddBurnToDiscardAndDraw {
        damage: orb_walker_laser_damage(ascension),
        count: 1,
    }
}

#[must_use]
fn fungi_beast_grow_strength(ascension: u8) -> i32 {
    let strength = if ascension >= 2 {
        FUNGI_BEAST_A2_GROW_STRENGTH
    } else {
        FUNGI_BEAST_GROW_STRENGTH
    };
    if ascension >= 17 {
        strength + FUNGI_BEAST_A17_GROW_BONUS
    } else {
        strength
    }
}

#[must_use]
fn fungi_beast_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Attack {
            damage: FUNGI_BEAST_BITE_DAMAGE,
        },
        _ => MonsterIntent::StrengthSelf {
            amount: fungi_beast_grow_strength(ascension),
        },
    }
}

#[must_use]
pub fn target_fungi_beast_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if roll < 60 {
        if last_two_moves(move_history, 1) {
            return fungi_beast_grow_intent(ascension);
        }
        return fungi_beast_bite_intent();
    }
    if last_move(move_history, 2) {
        fungi_beast_bite_intent()
    } else {
        fungi_beast_grow_intent(ascension)
    }
}

#[must_use]
fn fungi_beast_bite_intent() -> MonsterIntent {
    MonsterIntent::Attack {
        damage: FUNGI_BEAST_BITE_DAMAGE,
    }
}

#[must_use]
fn fungi_beast_grow_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::StrengthSelf {
        amount: fungi_beast_grow_strength(ascension),
    }
}

#[must_use]
fn slaver_blue_stab_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SLAVER_BLUE_A2_STAB_DAMAGE
    } else {
        SLAVER_BLUE_STAB_DAMAGE
    }
}

#[must_use]
fn slaver_blue_rake_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SLAVER_BLUE_A2_RAKE_DAMAGE
    } else {
        SLAVER_BLUE_RAKE_DAMAGE
    }
}

#[must_use]
fn slaver_blue_weak(ascension: u8) -> i32 {
    if ascension >= 17 {
        SLAVER_BLUE_A17_WEAK
    } else {
        SLAVER_BLUE_WEAK
    }
}

#[must_use]
fn slaver_blue_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::AttackApplyPlayerWeak {
            damage: slaver_blue_rake_damage(ascension),
            weak: slaver_blue_weak(ascension),
        },
        _ => MonsterIntent::Attack {
            damage: slaver_blue_stab_damage(ascension),
        },
    }
}

#[must_use]
pub fn target_slaver_blue_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if roll >= 40 && !last_two_moves(move_history, 1) {
        MonsterIntent::Attack {
            damage: slaver_blue_stab_damage(ascension),
        }
    } else if ascension >= 17 {
        if !last_move(move_history, 4) {
            MonsterIntent::AttackApplyPlayerWeak {
                damage: slaver_blue_rake_damage(ascension),
                weak: slaver_blue_weak(ascension),
            }
        } else {
            MonsterIntent::Attack {
                damage: slaver_blue_stab_damage(ascension),
            }
        }
    } else if !last_two_moves(move_history, 4) {
        MonsterIntent::AttackApplyPlayerWeak {
            damage: slaver_blue_rake_damage(ascension),
            weak: slaver_blue_weak(ascension),
        }
    } else {
        MonsterIntent::Attack {
            damage: slaver_blue_stab_damage(ascension),
        }
    }
}

#[must_use]
fn slaver_red_stab_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SLAVER_RED_A2_STAB_DAMAGE
    } else {
        SLAVER_RED_STAB_DAMAGE
    }
}

#[must_use]
fn slaver_red_scrape_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SLAVER_RED_A2_SCRAPE_DAMAGE
    } else {
        SLAVER_RED_SCRAPE_DAMAGE
    }
}

#[must_use]
fn slaver_red_vulnerable(ascension: u8) -> i32 {
    if ascension >= 17 {
        SLAVER_RED_A17_VULNERABLE
    } else {
        SLAVER_RED_VULNERABLE
    }
}

#[must_use]
fn slaver_red_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Attack {
            damage: slaver_red_stab_damage(ascension),
        },
        1 => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: slaver_red_scrape_damage(ascension),
            vulnerable: slaver_red_vulnerable(ascension),
        },
        _ => MonsterIntent::ApplyPlayerEntangled {
            amount: SLAVER_RED_ENTANGLED,
        },
    }
}

#[must_use]
pub fn target_slaver_red_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if move_history.is_empty() {
        return MonsterIntent::Attack {
            damage: slaver_red_stab_damage(ascension),
        };
    }
    let used_entangle = move_history.contains(&2);
    if roll >= 75 && !used_entangle {
        return MonsterIntent::ApplyPlayerEntangled {
            amount: SLAVER_RED_ENTANGLED,
        };
    }
    if roll >= 55 && used_entangle && !last_two_moves(move_history, 1) {
        return MonsterIntent::Attack {
            damage: slaver_red_stab_damage(ascension),
        };
    }
    if ascension >= 17 {
        if !last_move(move_history, 3) {
            return slaver_red_scrape_intent(ascension);
        }
    } else if !last_two_moves(move_history, 3) {
        return slaver_red_scrape_intent(ascension);
    }
    MonsterIntent::Attack {
        damage: slaver_red_stab_damage(ascension),
    }
}

fn slaver_red_scrape_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::AttackApplyPlayerVulnerable {
        damage: slaver_red_scrape_damage(ascension),
        vulnerable: slaver_red_vulnerable(ascension),
    }
}

pub fn apply_heal_all_monsters(monsters: &mut [MonsterState], amount: i32) -> SimResult<()> {
    if amount < 0 {
        return Err(SimError::InvalidState(
            "monster group healing amount is negative",
        ));
    }
    let healed_hp = monsters
        .iter()
        .map(|monster| {
            if monster.alive {
                Ok(monster.hp.saturating_add(amount).min(monster.max_hp))
            } else {
                Ok(monster.hp)
            }
        })
        .collect::<SimResult<Vec<_>>>()?;
    for (monster, hp) in monsters.iter_mut().zip(healed_hp) {
        monster.hp = hp;
    }
    Ok(())
}

pub fn apply_strength_all_monsters(monsters: &mut [MonsterState], amount: i32) -> SimResult<()> {
    let strengths = monsters
        .iter()
        .map(|monster| {
            if monster.alive {
                monster
                    .powers
                    .strength
                    .checked_add(amount)
                    .ok_or(SimError::InvalidState("monster group arithmetic overflow"))
            } else {
                Ok(monster.powers.strength)
            }
        })
        .collect::<SimResult<Vec<_>>>()?;
    for (monster, strength) in monsters.iter_mut().zip(strengths) {
        monster.powers.strength = strength;
    }
    Ok(())
}

pub fn apply_gremlin_leader_encourage(
    monsters: &mut [MonsterState],
    leader_id: MonsterId,
    strength: i32,
    block: i32,
) -> SimResult<()> {
    let values = monsters
        .iter()
        .map(|monster| {
            if !monster.alive {
                return Ok((monster.powers.strength, monster.block));
            }
            let next_strength = monster
                .powers
                .strength
                .checked_add(strength)
                .ok_or(SimError::InvalidState("monster group arithmetic overflow"))?;
            let next_block = if monster.id == leader_id {
                monster.block
            } else {
                monster
                    .block
                    .checked_add(block)
                    .ok_or(SimError::InvalidState("monster group arithmetic overflow"))?
            };
            Ok((next_strength, next_block))
        })
        .collect::<SimResult<Vec<_>>>()?;
    for (monster, (strength, block)) in monsters.iter_mut().zip(values) {
        monster.powers.strength = strength;
        monster.block = block;
    }
    Ok(())
}

fn positive_monster_spawn_count(count: i32) -> SimResult<usize> {
    let count = usize::try_from(count)
        .map_err(|_| SimError::InvalidState("monster summon count is not positive"))?;
    if count == 0 {
        return Err(SimError::InvalidState(
            "monster summon count is not positive",
        ));
    }
    Ok(count)
}

fn reserve_monster_spawn_ids(monsters: &mut Vec<MonsterState>, count: usize) -> SimResult<u64> {
    if count == 0 {
        return Err(SimError::InvalidState(
            "monster summon reserved no instance IDs",
        ));
    }
    let max_id = monsters
        .iter()
        .map(|monster| monster.id.get())
        .max()
        .unwrap_or(0);
    let count_u64 = u64::try_from(count)
        .map_err(|_| SimError::InvalidState("monster summon count exceeds the ID domain"))?;
    max_id.checked_add(count_u64).ok_or(SimError::InvalidState(
        "monster summon instance ID overflows u64",
    ))?;
    monsters
        .try_reserve(count)
        .map_err(|_| SimError::InvalidState("monster summon storage cannot be allocated"))?;
    max_id.checked_add(1).ok_or(SimError::InvalidState(
        "monster summon instance ID overflows u64",
    ))
}

pub(crate) fn apply_gremlin_leader_rally_target(
    monsters: &mut Vec<MonsterState>,
    count: i32,
    ai_rng: &mut crate::rng::StsRng,
    hp_rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_ai_rng = ai_rng.clone();
    let mut next_hp_rng = hp_rng.clone();
    apply_gremlin_leader_rally_target_inner(
        &mut next_monsters,
        count,
        &mut next_ai_rng,
        &mut next_hp_rng,
        ascension,
    )?;
    *monsters = next_monsters;
    *ai_rng = next_ai_rng;
    *hp_rng = next_hp_rng;
    Ok(())
}

fn apply_gremlin_leader_rally_target_inner(
    monsters: &mut Vec<MonsterState>,
    count: i32,
    ai_rng: &mut crate::rng::StsRng,
    hp_rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> SimResult<()> {
    let count = positive_monster_spawn_count(count)?;
    if count != 2 {
        return Err(SimError::InvalidState(
            "Gremlin Leader rally count differs from the target move",
        ));
    }
    if !monsters
        .iter()
        .any(|monster| monster.alive && monster.content_id == GREMLIN_LEADER_ID)
    {
        return Err(SimError::InvalidState(
            "Gremlin Leader rally is missing its living summoner",
        ));
    }

    let mut planned = Vec::new();
    let mut reserved_slots = Vec::new();
    for _ in 0..count {
        if gremlin_leader_live_minion_count(monsters) + planned.len() as i32 >= 3 {
            break;
        }
        let Some(slot) = gremlin_leader_first_available_slot_excluding(monsters, &reserved_slots)
        else {
            break;
        };
        reserved_slots.push(slot);
        planned.push(slot);
    }
    if planned.is_empty() {
        return Ok(());
    }
    let first_id = reserve_monster_spawn_ids(monsters, planned.len())?;
    let mut spawned = Vec::with_capacity(planned.len());

    for (offset, slot) in planned.into_iter().enumerate() {
        let next_id = first_id + offset as u64;
        let name = target_random_gremlin_name(ai_rng);
        let content_id = content_id_from_game_monster_id(name).ok_or(SimError::InvalidState(
            "random gremlin identity is not registered",
        ))?;
        let definition =
            get_monster_definition(content_id).ok_or(SimError::UnknownContent(content_id))?;
        let max_hp = target_city_monster_hp_range(name, ascension)
            .map(|range| range.roll(hp_rng))
            .ok_or(SimError::UnsupportedMechanic(content_id))?;
        let mut monster =
            monster_state_for_ascension(definition, MonsterId::new(next_id), ascension);
        monster.hp = max_hp;
        monster.max_hp = max_hp;
        monster.powers.minion = 1;
        if monster.content_id == GREMLIN_WARRIOR_ID {
            monster.powers.anger = gremlin_warrior_anger(ascension);
        }
        monster.gremlin_leader_slot = Some(slot as u8);
        spawned.push((slot, monster));
    }

    for (_, monster) in &mut spawned {
        let roll = ai_rng.random_int(99);
        monster.intent =
            source_backed_gremlin_leader_minion_intent(monster.content_id, 0, ascension)
                .ok_or(SimError::UnsupportedMechanic(monster.content_id))?;
        let _ = roll;
        record_target_move(monster);
    }

    for (slot, monster) in spawned {
        let insert_index = gremlin_leader_summon_insert_index(monsters, slot, monster.content_id);
        monsters.insert(insert_index, monster);
    }
    Ok(())
}

pub(crate) fn apply_collector_spawn_torch_heads(
    monsters: &mut Vec<MonsterState>,
    count: i32,
    ai_rng: &mut crate::rng::StsRng,
    hp_rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_ai_rng = ai_rng.clone();
    let mut next_hp_rng = hp_rng.clone();
    apply_collector_spawn_torch_heads_inner(
        &mut next_monsters,
        count,
        &mut next_ai_rng,
        &mut next_hp_rng,
        ascension,
    )?;
    *monsters = next_monsters;
    *ai_rng = next_ai_rng;
    *hp_rng = next_hp_rng;
    Ok(())
}

fn apply_collector_spawn_torch_heads_inner(
    monsters: &mut Vec<MonsterState>,
    count: i32,
    ai_rng: &mut crate::rng::StsRng,
    hp_rng: &mut crate::rng::StsRng,
    ascension: u8,
) -> SimResult<()> {
    let count = positive_monster_spawn_count(count)?;
    if count != 2 {
        return Err(SimError::InvalidState(
            "Collector summon count differs from the target move",
        ));
    }
    if !monsters
        .iter()
        .any(|monster| monster.alive && monster.content_id == THE_COLLECTOR_ID)
    {
        return Err(SimError::InvalidState(
            "Collector summon is missing its living summoner",
        ));
    }

    let range = if ascension >= 9 {
        TORCH_HEAD_A9_HP_RANGE
    } else {
        TORCH_HEAD_A0_HP_RANGE
    };
    // The Collector can have at most two living Torch Heads.  SpawnMonsterAction
    // only creates the missing slots, while dead slot occupants remain in the
    // MonsterGroup for observation and ordering.
    let live_torch_heads = monsters
        .iter()
        .filter(|monster| monster.alive && monster.content_id == TORCH_HEAD_ID)
        .count();
    let spawn_count = count.min(2usize.saturating_sub(live_torch_heads));
    if spawn_count == 0 {
        return Ok(());
    }
    let slots = (1u8..=2)
        .filter(|slot| {
            !monsters.iter().any(|monster| {
                monster.alive
                    && monster.content_id == TORCH_HEAD_ID
                    && monster.gremlin_leader_slot == Some(*slot)
            })
        })
        .take(spawn_count)
        .collect::<Vec<_>>();
    let first_id = reserve_monster_spawn_ids(monsters, slots.len())?;
    let mut hp_values = (0..slots.len())
        .map(|_| {
            // TorchHead rolls the A0 range for AbstractMonster's hp
            // argument, then its own setHp call rolls the active range.
            let _constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(hp_rng);
            range.roll(hp_rng)
        })
        .collect::<Vec<_>>();
    // The Collector constructs slot 1 before slot 2, but SpawnMonsterAction
    // inserts the farther-left slot 2 before slot 1 in MonsterGroup order.
    hp_values.reverse();

    for (slot_index, (slot, max_hp)) in slots.into_iter().zip(hp_values).enumerate() {
        let mut monster = monster_state_for_ascension(
            &TORCH_HEAD_A0,
            MonsterId::new(first_id + slot_index as u64),
            ascension,
        );
        monster.hp = max_hp;
        monster.max_hp = max_hp;
        monster.powers.minion = 1;
        monster.gremlin_leader_slot = Some(slot);
        monster.intent = MonsterIntent::Attack {
            damage: TORCH_HEAD_ATTACK_DAMAGE,
        };
        let _ = ai_rng.random_int(99);
        record_target_move(&mut monster);
        let insert_index = monsters
            .iter()
            .enumerate()
            .find_map(|(index, monster)| {
                (monster.content_id == TORCH_HEAD_ID
                    && monster.gremlin_leader_slot == Some(slot)
                    && !monster.alive)
                    .then_some(index)
            })
            .or_else(|| {
                monsters
                    .iter()
                    .position(|monster| monster.content_id == THE_COLLECTOR_ID)
            })
            .unwrap_or(monsters.len());
        monsters.insert(insert_index, monster);
    }
    Ok(())
}

pub(crate) fn apply_bronze_automaton_orb_spawn(
    monsters: &mut Vec<MonsterState>,
    automaton_id: MonsterId,
    count: i32,
    ai_rng: &mut StsRng,
    hp_rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_ai_rng = ai_rng.clone();
    let mut next_hp_rng = hp_rng.clone();
    apply_bronze_automaton_orb_spawn_inner(
        &mut next_monsters,
        automaton_id,
        count,
        &mut next_ai_rng,
        &mut next_hp_rng,
        ascension,
    )?;
    *monsters = next_monsters;
    *ai_rng = next_ai_rng;
    *hp_rng = next_hp_rng;
    Ok(())
}

fn apply_bronze_automaton_orb_spawn_inner(
    monsters: &mut Vec<MonsterState>,
    automaton_id: MonsterId,
    count: i32,
    ai_rng: &mut StsRng,
    hp_rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    if positive_monster_spawn_count(count)? != 2 {
        return Err(SimError::InvalidState(
            "Bronze Automaton summon count differs from the target move",
        ));
    }
    let Some(automaton_index) = monsters.iter().position(|monster| {
        monster.id == automaton_id && monster.alive && monster.content_id == BRONZE_AUTOMATON_ID
    }) else {
        return Err(SimError::InvalidState(
            "Bronze Automaton summon is missing its living summoner",
        ));
    };
    if monsters
        .iter()
        .any(|monster| monster.alive && monster.content_id == BRONZE_ORB_ID)
    {
        return Err(SimError::InvalidState(
            "Bronze Automaton cannot repeat its opening Orb summon",
        ));
    }

    let next_id = reserve_monster_spawn_ids(monsters, 2)?;
    let mut left = monster_state(&BRONZE_ORB_A0, MonsterId::new(next_id));
    let mut right = monster_state(&BRONZE_ORB_A0, MonsterId::new(next_id + 1));
    let hp_range = bronze_orb_hp_range(ascension);
    let _constructor_hp = BRONZE_ORB_A0_HP_RANGE.roll(hp_rng);
    let max_hp = hp_range.roll(hp_rng);
    left.hp = max_hp;
    left.max_hp = max_hp;
    left.powers.minion = 1;
    let roll = ai_rng.random_int(99);
    left.intent = target_bronze_orb_next_intent_from_roll(&left.move_history, roll);
    record_target_move(&mut left);
    let _constructor_hp = BRONZE_ORB_A0_HP_RANGE.roll(hp_rng);
    let max_hp = hp_range.roll(hp_rng);
    right.hp = max_hp;
    right.max_hp = max_hp;
    right.powers.minion = 1;
    let roll = ai_rng.random_int(99);
    right.intent = target_bronze_orb_next_intent_from_roll(&right.move_history, roll);
    record_target_move(&mut right);

    monsters.insert(automaton_index, left);
    monsters.insert(automaton_index + 2, right);
    Ok(())
}

#[must_use]
fn bronze_orb_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 9 {
        BRONZE_ORB_A9_HP_RANGE
    } else {
        BRONZE_ORB_A0_HP_RANGE
    }
}

pub(crate) fn apply_reptomancer_dagger_spawn(
    monsters: &mut Vec<MonsterState>,
    reptomancer_id: MonsterId,
    count: i32,
    ai_rng: &mut StsRng,
    hp_rng: &mut StsRng,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_ai_rng = ai_rng.clone();
    let mut next_hp_rng = hp_rng.clone();
    apply_reptomancer_dagger_spawn_inner(
        &mut next_monsters,
        reptomancer_id,
        count,
        &mut next_ai_rng,
        &mut next_hp_rng,
    )?;
    *monsters = next_monsters;
    *ai_rng = next_ai_rng;
    *hp_rng = next_hp_rng;
    Ok(())
}

fn apply_reptomancer_dagger_spawn_inner(
    monsters: &mut Vec<MonsterState>,
    reptomancer_id: MonsterId,
    count: i32,
    ai_rng: &mut StsRng,
    hp_rng: &mut StsRng,
) -> SimResult<()> {
    let count = positive_monster_spawn_count(count)?;
    if count > 2 {
        return Err(SimError::InvalidState(
            "Reptomancer summon count exceeds the target move",
        ));
    }
    let Some(_reptomancer_index) = monsters.iter().position(|monster| {
        monster.id == reptomancer_id && monster.alive && monster.content_id == REPTOMANCER_ID
    }) else {
        return Err(SimError::InvalidState(
            "Reptomancer summon is missing its living summoner",
        ));
    };
    let available_slots = (0..4)
        .filter(|slot| {
            !monsters.iter().any(|monster| {
                monster.alive
                    && monster.content_id == DAGGER_ID
                    && monster.gremlin_leader_slot == Some(*slot)
            })
        })
        .take(count)
        .collect::<Vec<_>>();
    if available_slots.is_empty() {
        return Ok(());
    }
    let first_id = reserve_monster_spawn_ids(monsters, available_slots.len())?;
    for (offset, slot) in available_slots.into_iter().enumerate() {
        let next_id = first_id + offset as u64;
        let mut dagger = monster_state(&DAGGER_A0, MonsterId::new(next_id));
        let max_hp = DAGGER_HP_RANGE.roll(hp_rng);
        dagger.hp = max_hp;
        dagger.max_hp = max_hp;
        dagger.powers.minion = 1;
        dagger.gremlin_leader_slot = Some(slot);
        let _roll = ai_rng.random_int(99);
        dagger.intent = MonsterIntent::AttackAddWoundsToDiscard {
            damage: DAGGER_WOUND_DAMAGE,
            count: 1,
        };
        record_target_move(&mut dagger);
        let insert_index = reptomancer_dagger_insert_index(monsters, slot);
        monsters.insert(insert_index, dagger);
    }
    Ok(())
}

fn reptomancer_dagger_insert_index(monsters: &[MonsterState], slot: u8) -> usize {
    let slot_key = reptomancer_position_key_for_slot(slot);
    monsters
        .iter()
        .position(|monster| reptomancer_position_key(monster) >= slot_key)
        .unwrap_or(monsters.len())
}

#[must_use]
fn reptomancer_hp_range(ascension: u8) -> MonsterHpRange {
    if ascension >= 8 {
        REPTOMANCER_A8_HP_RANGE
    } else {
        REPTOMANCER_A0_HP_RANGE
    }
}

pub fn advance_reptomancer_monster_hp_rng_for_entry(rng: &mut StsRng, ascension: u8) {
    DAGGER_HP_RANGE.roll(rng);
    REPTOMANCER_A0_HP_RANGE.roll(rng);
    reptomancer_hp_range(ascension).roll(rng);
    DAGGER_HP_RANGE.roll(rng);
}

fn reptomancer_position_key(monster: &MonsterState) -> u8 {
    if monster.content_id == REPTOMANCER_ID {
        return 2;
    }
    if monster.content_id == DAGGER_ID {
        if let Some(slot) = monster.gremlin_leader_slot {
            return reptomancer_position_key_for_slot(slot);
        }
    }
    2
}

fn reptomancer_position_key_for_slot(slot: u8) -> u8 {
    match slot {
        3 => 0,
        1 => 1,
        2 => 3,
        0 => 4,
        _ => 2,
    }
}

pub(crate) fn apply_large_acid_slime_split(
    monsters: &mut Vec<MonsterState>,
    slime_id: MonsterId,
    count: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_rng = rng.clone();
    apply_large_acid_slime_split_inner(
        &mut next_monsters,
        slime_id,
        count,
        &mut next_rng,
        ascension,
    )?;
    *monsters = next_monsters;
    *rng = next_rng;
    Ok(())
}

fn apply_large_acid_slime_split_inner(
    monsters: &mut Vec<MonsterState>,
    slime_id: MonsterId,
    count: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    if positive_monster_spawn_count(count)? != 2 {
        return Err(SimError::InvalidState(
            "Acid Slime split count differs from the target move",
        ));
    }
    let Some(slime_index) = monsters.iter().position(|monster| {
        monster.id == slime_id && monster.alive && monster.content_id == ACID_SLIME_ID
    }) else {
        return Err(SimError::InvalidState(
            "Acid Slime split is missing its living parent",
        ));
    };
    if monsters[slime_index].hp <= 0 {
        return Err(SimError::InvalidState(
            "Acid Slime split parent has no positive HP",
        ));
    }

    let next_id = reserve_monster_spawn_ids(monsters, 2)?;
    let split_hp = monsters[slime_index].hp;
    let mut left = monster_state(&ACID_SLIME_A0, MonsterId::new(next_id));
    let mut right = monster_state(&ACID_SLIME_A0, MonsterId::new(next_id + 1));
    left.hp = split_hp;
    left.max_hp = split_hp;
    left.slime_size = Some(SlimeSize::Medium);
    right.hp = split_hp;
    right.max_hp = split_hp;
    right.slime_size = Some(SlimeSize::Medium);
    let left_roll = rng.random_int(99);
    left.intent = target_medium_acid_slime_next_intent_from_roll(
        &left.move_history,
        left_roll,
        rng,
        ascension,
    );
    record_target_move(&mut left);
    let right_roll = rng.random_int(99);
    right.intent = target_medium_acid_slime_next_intent_from_roll(
        &right.move_history,
        right_roll,
        rng,
        ascension,
    );
    record_target_move(&mut right);

    monsters[slime_index].hp = 0;
    monsters[slime_index].alive = false;
    monsters[slime_index].block = 0;
    if let Some(boss_index) = monsters
        .iter()
        .position(|monster| monster.content_id == SLIME_BOSS_ID && monster.hp <= 0)
        .filter(|index| *index < slime_index)
    {
        monsters.insert(boss_index, left);
        monsters.insert(slime_index + 2, right);
    } else {
        monsters.insert(slime_index, left);
        monsters.insert(slime_index + 2, right);
    }
    Ok(())
}

pub(crate) fn apply_large_spike_slime_split(
    monsters: &mut Vec<MonsterState>,
    slime_id: MonsterId,
    count: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_rng = rng.clone();
    apply_large_spike_slime_split_inner(
        &mut next_monsters,
        slime_id,
        count,
        &mut next_rng,
        ascension,
    )?;
    *monsters = next_monsters;
    *rng = next_rng;
    Ok(())
}

fn apply_large_spike_slime_split_inner(
    monsters: &mut Vec<MonsterState>,
    slime_id: MonsterId,
    count: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    if positive_monster_spawn_count(count)? != 2 {
        return Err(SimError::InvalidState(
            "Spike Slime split count differs from the target move",
        ));
    }
    let Some(slime_index) = monsters.iter().position(|monster| {
        monster.id == slime_id && monster.alive && monster.content_id == SPIKE_SLIME_ID
    }) else {
        return Err(SimError::InvalidState(
            "Spike Slime split is missing its living parent",
        ));
    };
    if monsters[slime_index].hp <= 0 {
        return Err(SimError::InvalidState(
            "Spike Slime split parent has no positive HP",
        ));
    }

    let next_id = reserve_monster_spawn_ids(monsters, 2)?;
    let split_hp = monsters[slime_index].hp;
    let mut left = monster_state(&SPIKE_SLIME_A0, MonsterId::new(next_id));
    let mut right = monster_state(&SPIKE_SLIME_A0, MonsterId::new(next_id + 1));
    left.hp = split_hp;
    left.max_hp = split_hp;
    left.slime_size = Some(SlimeSize::Medium);
    right.hp = split_hp;
    right.max_hp = split_hp;
    right.slime_size = Some(SlimeSize::Medium);
    let left_roll = rng.random_int(99);
    left.intent = target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
        false,
        &left.move_history,
        left_roll,
        ascension,
    );
    record_target_move(&mut left);
    let right_roll = rng.random_int(99);
    right.intent = target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
        false,
        &right.move_history,
        right_roll,
        ascension,
    );
    record_target_move(&mut right);

    monsters[slime_index].hp = 0;
    monsters[slime_index].alive = false;
    monsters[slime_index].block = 0;
    let parent_roll = rng.random_int(99);
    monsters[slime_index].intent = target_medium_or_large_spike_slime_next_intent_from_roll(
        monsters[slime_index].max_hp,
        &monsters[slime_index].move_history,
        parent_roll,
        ascension,
    );
    monsters.insert(slime_index, left);
    monsters.insert(slime_index + 2, right);
    Ok(())
}

pub(crate) fn apply_slime_boss_split(
    monsters: &mut Vec<MonsterState>,
    boss_id: MonsterId,
    count: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    let mut next_monsters = monsters.clone();
    let mut next_rng = rng.clone();
    apply_slime_boss_split_inner(&mut next_monsters, boss_id, count, &mut next_rng, ascension)?;
    *monsters = next_monsters;
    *rng = next_rng;
    Ok(())
}

fn apply_slime_boss_split_inner(
    monsters: &mut Vec<MonsterState>,
    boss_id: MonsterId,
    count: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> SimResult<()> {
    if positive_monster_spawn_count(count)? != 2 {
        return Err(SimError::InvalidState(
            "Slime Boss split count differs from the target move",
        ));
    }
    let Some(boss_index) = monsters.iter().position(|monster| {
        monster.id == boss_id && monster.alive && monster.content_id == SLIME_BOSS_ID
    }) else {
        return Err(SimError::InvalidState(
            "Slime Boss split is missing its living parent",
        ));
    };
    if monsters[boss_index].hp <= 0 {
        return Err(SimError::InvalidState(
            "Slime Boss split parent has no positive HP",
        ));
    }

    let next_id = reserve_monster_spawn_ids(monsters, 2)?;
    let split_hp = monsters[boss_index].hp;
    let mut spike = monster_state(&SPIKE_SLIME_A0, MonsterId::new(next_id));
    let mut acid = monster_state(&ACID_SLIME_A0, MonsterId::new(next_id + 1));
    spike.hp = split_hp;
    spike.max_hp = split_hp;
    spike.slime_size = Some(SlimeSize::Large);
    spike.rolled_attack_damage = Some(if ascension >= 2 {
        SPIKE_SLIME_L_SPIT_DAMAGE + 2
    } else {
        SPIKE_SLIME_L_SPIT_DAMAGE
    });
    let roll = rng.random_int(99);
    spike.intent = target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
        true,
        &spike.move_history,
        roll,
        ascension,
    );
    record_target_move(&mut spike);
    acid.hp = split_hp;
    acid.max_hp = split_hp;
    acid.slime_size = Some(SlimeSize::Large);
    acid.rolled_attack_damage = Some(if ascension >= 2 {
        ACID_SLIME_L_A2_NORMAL_TACKLE_DAMAGE
    } else {
        ACID_SLIME_L_NORMAL_TACKLE_DAMAGE
    });
    let roll = rng.random_int(99);
    acid.intent =
        target_large_acid_slime_next_intent_from_roll(&acid.move_history, roll, rng, ascension);
    record_target_move(&mut acid);

    monsters[boss_index].hp = 0;
    monsters[boss_index].alive = false;
    monsters[boss_index].block = 0;
    monsters[boss_index].split_triggered = true;
    monsters.insert(boss_index, spike);
    monsters.insert(boss_index + 2, acid);
    Ok(())
}

fn gremlin_leader_live_minion_count(monsters: &[MonsterState]) -> i32 {
    monsters
        .iter()
        .filter(|monster| monster.alive && is_gremlin_leader_minion_content_id(monster.content_id))
        .count() as i32
}

fn gremlin_leader_representative_summon_index(monsters: &[MonsterState]) -> usize {
    monsters
        .iter()
        .position(|monster| monster.content_id == GREMLIN_LEADER_ID)
        .unwrap_or(monsters.len())
}

fn gremlin_leader_first_available_slot_excluding(
    monsters: &[MonsterState],
    reserved_slots: &[usize],
) -> Option<usize> {
    (0..3).find(|slot| {
        if reserved_slots.contains(slot) {
            return false;
        }
        gremlin_leader_current_slot_occupant(monsters, *slot)
            .map(|monster| !monster.alive)
            .unwrap_or(true)
    })
}

fn gremlin_leader_current_slot_occupant(
    monsters: &[MonsterState],
    slot: usize,
) -> Option<&MonsterState> {
    let leader_index = gremlin_leader_representative_summon_index(monsters);
    monsters
        .iter()
        .take(leader_index)
        .filter(|monster| {
            is_gremlin_leader_minion_content_id(monster.content_id)
                && monster.gremlin_leader_slot == Some(slot as u8)
        })
        // Gremlin Leader keeps the newest object in its source-side gremlins
        // array. Spawn IDs are allocated monotonically, even though the
        // MonsterGroup is spatially reordered when a summon is inserted.
        .max_by_key(|monster| monster.id.get())
}

fn gremlin_leader_summon_insert_index(
    monsters: &[MonsterState],
    slot: usize,
    content_id: ContentId,
) -> usize {
    let leader_index = gremlin_leader_representative_summon_index(monsters);
    let new_x = gremlin_leader_monster_draw_x(content_id, slot);
    monsters
        .iter()
        .take(leader_index)
        .filter(|monster| {
            monster
                .gremlin_leader_slot
                .map(|existing_slot| {
                    gremlin_leader_monster_draw_x(monster.content_id, existing_slot as usize)
                        < new_x
                })
                .unwrap_or(false)
        })
        .count()
        .min(leader_index)
}

fn gremlin_leader_monster_draw_x(content_id: ContentId, slot: usize) -> i32 {
    // GremlinWizard's constructor shifts the supplied x coordinate left by
    // 35; the other GremlinLeader summon constructors use it unchanged.
    gremlin_leader_slot_draw_x(slot)
        - if content_id == GREMLIN_WIZARD_ID {
            35
        } else {
            0
        }
}

fn gremlin_leader_slot_draw_x(slot: usize) -> i32 {
    match slot {
        0 => -366,
        1 => -170,
        2 => -532,
        _ => -366,
    }
}

pub fn apply_gremlin_leader_death_escape(monsters: &mut [MonsterState], monster_id: MonsterId) {
    let killed_leader = monsters
        .iter()
        .any(|monster| monster.id == monster_id && monster.content_id == GREMLIN_LEADER_ID);
    if !killed_leader {
        return;
    }

    for monster in monsters.iter_mut() {
        if monster.alive && is_gremlin_leader_minion_content_id(monster.content_id) {
            monster.alive = false;
        }
    }
}

pub fn apply_reptomancer_death_escape(monsters: &mut [MonsterState], monster_id: MonsterId) {
    let killed_reptomancer = monsters
        .iter()
        .any(|monster| monster.id == monster_id && monster.content_id == REPTOMANCER_ID);
    if !killed_reptomancer {
        return;
    }

    for monster in monsters.iter_mut() {
        if monster.alive && monster.content_id == DAGGER_ID {
            monster.alive = false;
        }
    }
}

pub fn apply_collector_death_escape(monsters: &mut [MonsterState], monster_id: MonsterId) {
    let killed_collector = monsters
        .iter()
        .any(|monster| monster.id == monster_id && monster.content_id == THE_COLLECTOR_ID);
    if !killed_collector {
        return;
    }

    for monster in monsters.iter_mut() {
        if monster.alive && monster.content_id == TORCH_HEAD_ID {
            monster.hp = 0;
            monster.alive = false;
        }
    }
}

pub(crate) fn heal_monster_to_stored_cap(monster: &mut MonsterState, amount: i32) -> SimResult<()> {
    if amount < 0 {
        return Err(SimError::InvalidState("monster healing amount is negative"));
    }
    let missing_hp = monster
        .max_hp
        .checked_sub(monster.hp)
        .ok_or(SimError::InvalidState("monster healing state is invalid"))?;
    if missing_hp < 0 {
        return Err(SimError::InvalidState("monster healing state is invalid"));
    }
    let applied = amount.min(missing_hp);
    monster.hp = monster
        .hp
        .checked_add(applied)
        .ok_or(SimError::InvalidState(
            "monster healing arithmetic overflow",
        ))?;
    Ok(())
}

/// Spike Slime (S) opens with Spit, then Lick.
#[must_use]
fn spike_slime_s_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Attack {
            damage: SPIKE_SLIME_S_SPIT_DAMAGE,
        },
        _ => MonsterIntent::ApplyPlayerWeak {
            amount: SPIKE_SLIME_LICK_WEAK,
        },
    }
}

#[must_use]
fn acid_slime_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        },
        _ => MonsterIntent::Attack {
            damage: ACID_SLIME_ATTACK_DAMAGE,
        },
    }
}

#[must_use]
pub fn target_acid_slime_entry_intent_from_roll(hp: i32, roll: i32) -> MonsterIntent {
    if hp <= ACID_SLIME_S_A7_HP_RANGE.max {
        return MonsterIntent::Attack {
            damage: ACID_SLIME_S_TACKLE_DAMAGE,
        };
    }

    if roll < 30 {
        MonsterIntent::AttackAddSlimedToDiscard {
            damage: ACID_SLIME_ATTACK_DAMAGE,
            count: 1,
        }
    } else if roll < 70 {
        MonsterIntent::Attack {
            damage: ACID_SLIME_M_NORMAL_TACKLE_DAMAGE,
        }
    } else {
        MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        }
    }
}

#[must_use]
pub fn target_small_acid_slime_entry_intent_from_bool(
    attack: bool,
    ascension: u8,
) -> MonsterIntent {
    if ascension >= 17 || !attack {
        MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        }
    } else {
        MonsterIntent::Attack {
            damage: if ascension >= 2 {
                4
            } else {
                ACID_SLIME_S_TACKLE_DAMAGE
            },
        }
    }
}

#[must_use]
pub fn target_small_acid_slime_followup_intent(
    previous_intent: MonsterIntent,
    ascension: u8,
) -> MonsterIntent {
    if matches!(previous_intent, MonsterIntent::Attack { .. }) {
        MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        }
    } else {
        MonsterIntent::Attack {
            damage: if ascension >= 2 {
                4
            } else {
                ACID_SLIME_S_TACKLE_DAMAGE
            },
        }
    }
}

#[must_use]
pub fn target_medium_acid_slime_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let wound_damage = if ascension >= 2 {
        8
    } else {
        ACID_SLIME_ATTACK_DAMAGE
    };
    let attack_damage = if ascension >= 2 {
        12
    } else {
        ACID_SLIME_M_NORMAL_TACKLE_DAMAGE
    };
    let weak = ACID_SLIME_WEAK;

    if ascension >= 17 {
        if roll < 40 {
            if last_two_moves(move_history, 1) {
                if rng.random_bool() {
                    MonsterIntent::Attack {
                        damage: attack_damage,
                    }
                } else {
                    MonsterIntent::ApplyPlayerWeak { amount: weak }
                }
            } else {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage: wound_damage,
                    count: 1,
                }
            }
        } else if roll < 80 {
            if last_two_moves(move_history, 2) {
                if rng.random_float() < 0.5 {
                    MonsterIntent::AttackAddSlimedToDiscard {
                        damage: wound_damage,
                        count: 1,
                    }
                } else {
                    MonsterIntent::ApplyPlayerWeak { amount: weak }
                }
            } else {
                MonsterIntent::Attack {
                    damage: attack_damage,
                }
            }
        } else if last_move(move_history, 4) {
            if rng.random_float() < 0.4 {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage: wound_damage,
                    count: 1,
                }
            } else {
                MonsterIntent::Attack {
                    damage: attack_damage,
                }
            }
        } else {
            MonsterIntent::ApplyPlayerWeak { amount: weak }
        }
    } else if roll < 30 {
        if last_two_moves(move_history, 1) {
            if rng.random_bool() {
                MonsterIntent::Attack {
                    damage: attack_damage,
                }
            } else {
                MonsterIntent::ApplyPlayerWeak { amount: weak }
            }
        } else {
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: wound_damage,
                count: 1,
            }
        }
    } else if roll < 70 {
        if last_move(move_history, 2) {
            if rng.random_float() < 0.4 {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage: wound_damage,
                    count: 1,
                }
            } else {
                MonsterIntent::ApplyPlayerWeak { amount: weak }
            }
        } else {
            MonsterIntent::Attack {
                damage: attack_damage,
            }
        }
    } else if last_two_moves(move_history, 4) {
        if rng.random_float() < 0.4 {
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: wound_damage,
                count: 1,
            }
        } else {
            MonsterIntent::Attack {
                damage: attack_damage,
            }
        }
    } else {
        MonsterIntent::ApplyPlayerWeak { amount: weak }
    }
}

#[must_use]
pub fn target_spike_slime_entry_intent_from_roll(hp: i32, roll: i32) -> MonsterIntent {
    if hp <= SPIKE_SLIME_S_A7_HP_RANGE.max {
        return MonsterIntent::Attack {
            damage: SPIKE_SLIME_S_SPIT_DAMAGE,
        };
    }

    if roll >= 30 {
        return MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: spike_slime_frail_amount(hp, 0),
            weak: 0,
        };
    }

    let damage = if hp > SPIKE_SLIME_M_A7_HP_RANGE.max {
        SPIKE_SLIME_L_SPIT_DAMAGE
    } else {
        SPIKE_SLIME_M_SPIT_DAMAGE
    };
    MonsterIntent::AttackAddSlimedToDiscard {
        damage,
        count: if hp > SPIKE_SLIME_M_A7_HP_RANGE.max {
            2
        } else {
            1
        },
    }
}

#[must_use]
pub fn target_medium_or_large_spike_slime_next_intent_from_roll(
    hp: i32,
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
        hp > SPIKE_SLIME_M_A7_HP_RANGE.max,
        move_history,
        roll,
        ascension,
    )
}

#[must_use]
pub(crate) fn target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
    large: bool,
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    let damage = if large {
        SPIKE_SLIME_L_SPIT_DAMAGE
    } else {
        SPIKE_SLIME_M_SPIT_DAMAGE
    };
    let count = if large { 2 } else { 1 };
    let attack = MonsterIntent::AttackAddSlimedToDiscard { damage, count };
    let debuff = MonsterIntent::ApplyPlayerFrailAndWeak {
        frail: spike_slime_frail_amount_for_profile(large, ascension),
        weak: 0,
    };

    if ascension >= 17 {
        if roll < 30 {
            if last_two_moves(move_history, 1) {
                debuff
            } else {
                attack
            }
        } else if last_move(move_history, 4) {
            attack
        } else {
            debuff
        }
    } else if roll < 30 {
        if last_two_moves(move_history, 1) {
            debuff
        } else {
            attack
        }
    } else if last_two_moves(move_history, 4) {
        attack
    } else {
        debuff
    }
}

fn spike_slime_frail_amount(hp: i32, ascension: u8) -> i32 {
    spike_slime_frail_amount_for_profile(hp > SPIKE_SLIME_M_A7_HP_RANGE.max, ascension)
}

fn spike_slime_frail_amount_for_profile(large: bool, ascension: u8) -> i32 {
    if large {
        if ascension >= 17 {
            SPIKE_SLIME_L_A17_FRAIL
        } else {
            SPIKE_SLIME_L_FRAIL
        }
    } else {
        SPIKE_SLIME_LICK_WEAK
    }
}

#[must_use]
fn sentry_attack_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        SENTRY_A3_ATTACK_DAMAGE
    } else {
        SENTRY_ATTACK_DAMAGE
    }
}

#[must_use]
fn sentry_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::AddDazedToDiscard {
            count: SENTRY_BEAM_DAZED,
        },
        _ => MonsterIntent::Attack {
            damage: sentry_attack_damage(ascension),
        },
    }
}

#[must_use]
pub fn target_sentry_next_intent(
    move_history: &[u8],
    monster_index: usize,
    ascension: u8,
) -> MonsterIntent {
    if move_history.is_empty() {
        return if monster_index.is_multiple_of(2) {
            MonsterIntent::AddDazedToDiscard {
                count: SENTRY_BEAM_DAZED,
            }
        } else {
            MonsterIntent::Attack {
                damage: sentry_attack_damage(ascension),
            }
        };
    }
    if last_move(move_history, 4) {
        MonsterIntent::AddDazedToDiscard {
            count: SENTRY_BEAM_DAZED,
        }
    } else {
        MonsterIntent::Attack {
            damage: sentry_attack_damage(ascension),
        }
    }
}

#[must_use]
fn spheric_guardian_damage(ascension: u8) -> i32 {
    if ascension >= 2 {
        SPHERIC_GUARDIAN_A2_DAMAGE
    } else {
        SPHERIC_GUARDIAN_DAMAGE
    }
}

#[must_use]
fn spheric_guardian_activate_block(ascension: u8) -> i32 {
    if ascension >= 17 {
        SPHERIC_GUARDIAN_A17_ACTIVATE_BLOCK
    } else {
        SPHERIC_GUARDIAN_ACTIVATE_BLOCK
    }
}

#[must_use]
fn spheric_guardian_intent(
    moves_executed: u32,
    move_history: &[u8],
    ascension: u8,
) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Block {
            block: spheric_guardian_activate_block(ascension),
        },
        1 => MonsterIntent::AttackApplyPlayerFrail {
            damage: spheric_guardian_damage(ascension),
            frail: SPHERIC_GUARDIAN_FRAIL,
        },
        _ if last_move(move_history, 1) => MonsterIntent::AttackAndBlock {
            damage: spheric_guardian_damage(ascension),
            block: SPHERIC_GUARDIAN_HARDEN_BLOCK,
        },
        _ => MonsterIntent::AttackMultiple {
            damage: spheric_guardian_damage(ascension),
            hits: SPHERIC_GUARDIAN_SLAM_HITS,
        },
    }
}

#[must_use]
fn guardian_intent(
    in_defensive_mode: bool,
    defensive_turns_remaining: u32,
    moves_executed: u32,
    ascension: u8,
) -> MonsterIntent {
    if in_defensive_mode {
        let turn_in_sequence =
            GUARDIAN_DEFENSIVE_SEQUENCE_TURNS.saturating_sub(defensive_turns_remaining);
        match turn_in_sequence {
            0 => MonsterIntent::GuardianCloseUp {
                sharp_hide: GUARDIAN_DEFENSIVE_SPIKES,
            },
            1 => MonsterIntent::Attack {
                damage: guardian_roll_attack_damage(ascension),
            },
            _ => MonsterIntent::AttackMultiple {
                damage: GUARDIAN_DEFENSIVE_COMBO_DAMAGE,
                hits: 2,
            },
        }
    } else {
        match moves_executed % 4 {
            0 => MonsterIntent::Block {
                block: GUARDIAN_CHARGE_BLOCK,
            },
            1 => MonsterIntent::Attack {
                damage: guardian_fierce_bash_damage(ascension),
            },
            2 => MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 0,
                weak: GUARDIAN_VENT_DEBUFF,
                vulnerable: GUARDIAN_VENT_DEBUFF,
            },
            _ => MonsterIntent::AttackMultiple {
                damage: GUARDIAN_WHIRLWIND_DAMAGE,
                hits: GUARDIAN_WHIRLWIND_HITS,
            },
        }
    }
}

#[must_use]
fn guardian_fierce_bash_damage(ascension: u8) -> i32 {
    if ascension >= 4 {
        GUARDIAN_A4_FIERCE_BASH_DAMAGE
    } else {
        GUARDIAN_FIERCE_BASH_DAMAGE
    }
}

#[must_use]
fn guardian_roll_attack_damage(ascension: u8) -> i32 {
    if ascension >= 4 {
        GUARDIAN_A4_DEFENSIVE_ATTACK_DAMAGE
    } else {
        GUARDIAN_DEFENSIVE_ATTACK_DAMAGE
    }
}

/// Enters Guardian defensive mode when Mode Shift reaches zero.
pub fn enter_guardian_defensive_mode(monster: &mut MonsterState) {
    if monster.content_id != GUARDIAN_ID || monster.in_defensive_mode {
        return;
    }
    monster.in_defensive_mode = true;
    monster.defensive_turns_remaining = GUARDIAN_DEFENSIVE_SEQUENCE_TURNS;
    monster.block += GUARDIAN_DEFENSIVE_BLOCK;
    if monster.mode_shift_threshold <= 0 {
        monster.mode_shift_threshold = GUARDIAN_MODE_SHIFT_START;
    }
    monster.mode_shift_threshold += GUARDIAN_MODE_SHIFT_INCREASE;
    monster.mode_shift = 0;
    monster.intent = guardian_intent(
        true,
        monster.defensive_turns_remaining,
        monster.moves_executed,
        0,
    );
}

fn exit_guardian_defensive_mode(monster: &mut MonsterState) {
    if monster.content_id != GUARDIAN_ID || !monster.in_defensive_mode {
        return;
    }
    monster.in_defensive_mode = false;
    monster.defensive_turns_remaining = 0;
    monster.powers.spikes = 0;
    monster.mode_shift = monster.mode_shift_threshold;
    monster.moves_executed = 2;
    monster.intent = guardian_intent(false, 0, monster.moves_executed, 0);
}

/// Decrements Mode Shift when the Guardian loses HP outside defensive mode.
pub fn guardian_on_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if monster.content_id != GUARDIAN_ID || hp_damage <= 0 || monster.in_defensive_mode {
        return;
    }
    monster.mode_shift -= hp_damage;
    if monster.mode_shift <= 0 {
        enter_guardian_defensive_mode(monster);
    }
}

/// Records Guardian Mode Shift damage without entering defensive mode yet.
///
/// Card effect queues use this so all hits (including copied multi-hit card
/// effects) resolve before the queued card action finishes and Guardian gains
/// its defensive block. End-of-turn damage (Combust, bombs) also uses this so
/// Mode Shift entry (and the 20 defensive block) can resolve after monster
/// pre-turn block clear — matching the target action queue where
/// `ChangeStateAction` → `GainBlockAction` lands after `MonsterStartTurnAction`.
/// Mid-turn non-card paths can continue using [`guardian_on_hp_damage`] for
/// immediate resolution.
pub fn guardian_accumulate_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if monster.content_id != GUARDIAN_ID
        || hp_damage <= 0
        || monster.in_defensive_mode
        || monster.mode_shift <= 0
    {
        return;
    }
    monster.mode_shift -= hp_damage;
}

/// Resolves Mode Shift entry after monster pre-turn block clear.
///
/// Target `TheGuardian.damage` queues `ChangeStateAction("Defensive Mode")`,
/// which itself queues `GainBlockAction(DEFENSIVE_BLOCK)`. When that damage is
/// itself queued from end-of-turn powers (Combust, bombs) before
/// `MonsterStartTurnAction`, GainBlock lands after `loseBlock`. Applying the
/// defensive block here after [`crate::combat::turn`]'s pre-monster clear
/// reproduces that ordering.
pub fn resolve_deferred_guardian_mode_shifts(monsters: &mut [MonsterState]) {
    for monster in monsters {
        if monster.content_id != GUARDIAN_ID
            || !monster.alive
            || monster.in_defensive_mode
            || monster.mode_shift > 0
        {
            continue;
        }
        enter_guardian_defensive_mode(monster);
    }
}

pub fn large_acid_slime_on_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if hp_damage <= 0
        || !monster.alive
        || !matches!(monster.content_id, ACID_SLIME_ID | SPIKE_SLIME_ID)
        || monster.split_triggered
        || monster.rolled_attack_damage.is_none()
        || !slime_can_split_at_current_hp(monster)
    {
        return;
    }

    monster.intent = MonsterIntent::SummonGremlins { count: 2 };
    monster.split_triggered = true;
}

fn slime_can_split_at_current_hp(monster: &MonsterState) -> bool {
    let large_or_split_child = if monster.content_id == ACID_SLIME_ID {
        monster.max_hp > ACID_SLIME_M_A7_HP_RANGE.max
            || monster
                .rolled_attack_damage
                .is_some_and(|damage| damage >= 11)
    } else {
        monster.max_hp > SPIKE_SLIME_M_A7_HP_RANGE.max
            || monster
                .rolled_attack_damage
                .is_some_and(|damage| damage >= SPIKE_SLIME_L_SPIT_DAMAGE)
    };
    large_or_split_child && monster.hp <= monster.max_hp / 2
}

fn finish_guardian_defensive_turn(monster: &mut MonsterState) {
    if monster.content_id != GUARDIAN_ID || !monster.in_defensive_mode {
        return;
    }
    if monster.defensive_turns_remaining > 0 {
        monster.defensive_turns_remaining -= 1;
    }
    if monster.defensive_turns_remaining == 0 {
        exit_guardian_defensive_mode(monster);
    } else {
        monster.intent = guardian_intent(
            true,
            monster.defensive_turns_remaining,
            monster.moves_executed,
            0,
        );
    }
}

#[must_use]
fn slime_boss_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed % 3 {
        0 => MonsterIntent::AddSlimedToDiscard {
            count: if ascension >= 19 {
                SLIME_BOSS_A19_SLIMED_COUNT
            } else {
                SLIME_BOSS_SLIMED_COUNT
            },
        },
        1 => MonsterIntent::Stun,
        _ => MonsterIntent::Attack {
            damage: SLIME_BOSS_SLAM_DAMAGE,
        },
    }
}

#[must_use]
fn hexaghost_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Stun,
        1 => MonsterIntent::AttackMultiple {
            damage: HEXAGHOST_DIVIDER_DAMAGE,
            hits: HEXAGHOST_DIVIDER_HITS,
        },
        _ => match (moves_executed - 2) % 7 {
            0 | 2 | 5 => MonsterIntent::AddBurnToDiscard {
                count: HEXAGHOST_SEAR_BURNS,
                damage: HEXAGHOST_DIVIDER_DAMAGE,
            },
            1 | 4 => MonsterIntent::AttackMultiple {
                damage: HEXAGHOST_TACKLE_DAMAGE,
                hits: HEXAGHOST_TACKLE_HITS,
            },
            3 => MonsterIntent::StrengthAndBlock {
                strength: HEXAGHOST_STRENGTHEN_STRENGTH,
                block: HEXAGHOST_STRENGTHEN_BLOCK,
            },
            _ => MonsterIntent::AttackMultipleUpgradeBurns {
                damage: HEXAGHOST_INFERNO_DAMAGE,
                hits: 6,
                count: HEXAGHOST_INFERNO_BURNS,
            },
        },
    }
}

#[must_use]
fn lagavulin_attack_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        LAGAVULIN_A3_ATTACK_DAMAGE
    } else {
        LAGAVULIN_ATTACK_DAMAGE
    }
}

#[must_use]
fn lagavulin_intent(
    sleep_turns_remaining: u32,
    moves_executed: u32,
    ascension: u8,
) -> MonsterIntent {
    if sleep_turns_remaining > 0 {
        MonsterIntent::Sleep
    } else if moves_executed % 3 == 2 {
        MonsterIntent::SiphonPlayer {
            strength: LAGAVULIN_SIPHON_STRENGTH,
            dexterity: LAGAVULIN_SIPHON_DEXTERITY,
        }
    } else {
        MonsterIntent::Attack {
            damage: lagavulin_attack_damage(ascension),
        }
    }
}

#[must_use]
pub fn target_lagavulin_direct_wake_attack_intent(ascension: u8) -> MonsterIntent {
    MonsterIntent::Attack {
        damage: lagavulin_attack_damage(ascension),
    }
}

pub fn clear_lagavulin_metallicize_if_awake(monster: &mut MonsterState) {
    if monster.content_id == LAGAVULIN_ID && monster.sleep_turns_remaining == 0 {
        monster.powers.metallicize = 0;
    }
}

/// Wakes a sleeping Lagavulin when HP damage is dealt and updates its intent for the current turn.
pub fn wake_lagavulin_on_damage(monster: &mut MonsterState, hp_damage: i32) {
    if monster.content_id == LAGAVULIN_ID && hp_damage > 0 {
        if monster.sleep_turns_remaining > 0 {
            monster.sleep_turns_remaining = 0;
            monster.intent = MonsterIntent::Stun;
        }
        monster.block = 0;
        monster.powers.metallicize = 0;
    }
}

/// Queues Slime Boss's split move when it crosses its split threshold.
pub fn check_slime_boss_split(state: &mut crate::CombatState, monster_id: MonsterId) {
    let Some(boss) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
    else {
        return;
    };
    if boss.content_id != SLIME_BOSS_ID
        || !boss.alive
        || boss.split_triggered
        || boss.hp > SLIME_BOSS_SPLIT_HP_THRESHOLD
    {
        return;
    }

    boss.intent = MonsterIntent::SummonGremlins { count: 2 };
    boss.split_triggered = true;
}

/// Gremlin Nob opens with Bellow, then Skull Bash, then Rush attacks.
#[must_use]
fn gremlin_nob_rush_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        GREMLIN_NOB_A3_RUSH_DAMAGE
    } else {
        GREMLIN_NOB_RUSH_DAMAGE
    }
}

#[must_use]
fn gremlin_nob_skull_bash_damage(ascension: u8) -> i32 {
    if ascension >= 3 {
        GREMLIN_NOB_A3_SKULL_BASH_DAMAGE
    } else {
        GREMLIN_NOB_SKULL_BASH_DAMAGE
    }
}

#[must_use]
fn gremlin_nob_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::StrengthSelf {
            amount: gremlin_nob_enrage(ascension),
        },
        1 => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: gremlin_nob_skull_bash_damage(ascension),
            vulnerable: 2,
        },
        _ => MonsterIntent::Attack {
            damage: gremlin_nob_rush_damage(ascension),
        },
    }
}

#[must_use]
pub fn target_gremlin_nob_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    ascension: u8,
) -> MonsterIntent {
    if ascension >= 18 {
        if !last_move(move_history, 2) && !last_move_before(move_history, 2) {
            return MonsterIntent::AttackApplyPlayerVulnerable {
                damage: gremlin_nob_skull_bash_damage(ascension),
                vulnerable: 2,
            };
        }
        if last_two_moves(move_history, 1) {
            return MonsterIntent::AttackApplyPlayerVulnerable {
                damage: gremlin_nob_skull_bash_damage(ascension),
                vulnerable: 2,
            };
        }
        return MonsterIntent::Attack {
            damage: gremlin_nob_rush_damage(ascension),
        };
    }
    // Target GremlinNob.getMove has no lastMove guard on the pre-A18
    // low-roll branch, so Skull Bash may be selected on consecutive turns.
    if roll < 33 {
        return MonsterIntent::AttackApplyPlayerVulnerable {
            damage: gremlin_nob_skull_bash_damage(ascension),
            vulnerable: 2,
        };
    }
    if !last_two_moves(move_history, 1) {
        return MonsterIntent::Attack {
            damage: gremlin_nob_rush_damage(ascension),
        };
    }
    MonsterIntent::AttackApplyPlayerVulnerable {
        damage: gremlin_nob_skull_bash_damage(ascension),
        vulnerable: 2,
    }
}

#[must_use]
pub fn gremlin_nob_enrage(ascension: u8) -> i32 {
    if ascension >= 17 {
        GREMLIN_NOB_A17_ENRAGE
    } else {
        GREMLIN_NOB_A0_ENRAGE
    }
}

/// Deterministic Jaw Worm move cycle: Chomp ? Thrash ? Bellow, keyed on `moves_executed`.
#[must_use]
fn jaw_worm_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 3 {
        0 => MonsterIntent::Attack {
            damage: JAW_WORM_CHOMP_DAMAGE,
        },
        1 => MonsterIntent::AttackAndBlock {
            damage: JAW_WORM_THRASH_DAMAGE,
            block: JAW_WORM_THRASH_BLOCK,
        },
        _ => MonsterIntent::StrengthAndBlock {
            strength: JAW_WORM_BELLOW_STRENGTH,
            block: JAW_WORM_BELLOW_BLOCK,
        },
    }
}

#[must_use]
pub fn target_jaw_worm_next_intent(
    previous_intent: MonsterIntent,
    rng: &mut StsRng,
) -> MonsterIntent {
    let roll = rng.random_int(99);
    target_jaw_worm_next_intent_from_previous_roll(previous_intent, roll)
}

#[must_use]
pub fn target_jaw_worm_next_intent_from_previous_roll(
    previous_intent: MonsterIntent,
    roll: i32,
) -> MonsterIntent {
    if roll < 25
        && !matches!(
            previous_intent,
            MonsterIntent::StrengthAndBlock {
                strength: JAW_WORM_BELLOW_STRENGTH,
                block: JAW_WORM_BELLOW_BLOCK,
            }
        )
    {
        return MonsterIntent::StrengthAndBlock {
            strength: JAW_WORM_BELLOW_STRENGTH,
            block: JAW_WORM_BELLOW_BLOCK,
        };
    }
    if roll < 55
        && !matches!(
            previous_intent,
            MonsterIntent::AttackAndBlock {
                damage: JAW_WORM_THRASH_DAMAGE,
                block: JAW_WORM_THRASH_BLOCK,
            }
        )
    {
        return MonsterIntent::AttackAndBlock {
            damage: JAW_WORM_THRASH_DAMAGE,
            block: JAW_WORM_THRASH_BLOCK,
        };
    }
    MonsterIntent::Attack {
        damage: JAW_WORM_CHOMP_DAMAGE,
    }
}

#[must_use]
pub fn target_jaw_worm_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
) -> MonsterIntent {
    if move_history.is_empty() {
        return jaw_worm_chomp_intent();
    }
    if roll < 25 {
        if last_move(move_history, 1) {
            return if rng.random_float() < 0.5625 {
                jaw_worm_bellow_intent()
            } else {
                jaw_worm_thrash_intent()
            };
        }
        return jaw_worm_chomp_intent();
    }
    if roll < 55 {
        if last_two_moves(move_history, 3) {
            return if rng.random_float() < 0.357 {
                jaw_worm_chomp_intent()
            } else {
                jaw_worm_bellow_intent()
            };
        }
        return jaw_worm_thrash_intent();
    }
    if last_move(move_history, 2) {
        return if rng.random_float() < 0.416 {
            jaw_worm_chomp_intent()
        } else {
            jaw_worm_thrash_intent()
        };
    }
    jaw_worm_bellow_intent()
}

fn jaw_worm_chomp_intent() -> MonsterIntent {
    MonsterIntent::Attack {
        damage: JAW_WORM_CHOMP_DAMAGE,
    }
}

fn jaw_worm_thrash_intent() -> MonsterIntent {
    MonsterIntent::AttackAndBlock {
        damage: JAW_WORM_THRASH_DAMAGE,
        block: JAW_WORM_THRASH_BLOCK,
    }
}

fn jaw_worm_bellow_intent() -> MonsterIntent {
    MonsterIntent::StrengthAndBlock {
        strength: JAW_WORM_BELLOW_STRENGTH,
        block: JAW_WORM_BELLOW_BLOCK,
    }
}

#[must_use]
pub fn target_large_acid_slime_next_intent(
    move_history: &[u8],
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let roll = rng.random_int(99);
    target_large_acid_slime_next_intent_from_roll(move_history, roll, rng, ascension)
}

#[must_use]
pub fn target_large_acid_slime_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let wound_damage = if ascension >= 2 {
        ACID_SLIME_L_A2_WOUND_TACKLE_DAMAGE
    } else {
        ACID_SLIME_L_WOUND_TACKLE_DAMAGE
    };
    let attack_damage = if ascension >= 2 { 18 } else { 16 };
    let weak = if ascension >= 17 { 3 } else { 2 };

    if ascension >= 17 {
        if roll < 40 {
            if last_two_moves(move_history, 1) {
                if rng.random_float() < 0.6 {
                    MonsterIntent::Attack {
                        damage: attack_damage,
                    }
                } else {
                    MonsterIntent::ApplyPlayerWeak { amount: weak }
                }
            } else {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage: wound_damage,
                    count: 2,
                }
            }
        } else if roll < 70 {
            if last_two_moves(move_history, 2) {
                if rng.random_float() < 0.6 {
                    MonsterIntent::AttackAddSlimedToDiscard {
                        damage: wound_damage,
                        count: 2,
                    }
                } else {
                    MonsterIntent::ApplyPlayerWeak { amount: weak }
                }
            } else {
                MonsterIntent::Attack {
                    damage: attack_damage,
                }
            }
        } else if last_move(move_history, 4) {
            if rng.random_float() < 0.4 {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage: wound_damage,
                    count: 2,
                }
            } else {
                MonsterIntent::Attack {
                    damage: attack_damage,
                }
            }
        } else {
            MonsterIntent::ApplyPlayerWeak { amount: weak }
        }
    } else if roll < 30 {
        if last_two_moves(move_history, 1) {
            if rng.random_bool() {
                MonsterIntent::Attack {
                    damage: attack_damage,
                }
            } else {
                MonsterIntent::ApplyPlayerWeak { amount: weak }
            }
        } else {
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: wound_damage,
                count: 2,
            }
        }
    } else if roll < 70 {
        if last_move(move_history, 2) {
            if rng.random_float() < 0.4 {
                MonsterIntent::AttackAddSlimedToDiscard {
                    damage: wound_damage,
                    count: 2,
                }
            } else {
                MonsterIntent::ApplyPlayerWeak { amount: weak }
            }
        } else {
            MonsterIntent::Attack {
                damage: attack_damage,
            }
        }
    } else if last_two_moves(move_history, 4) {
        if rng.random_float() < 0.4 {
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: wound_damage,
                count: 2,
            }
        } else {
            MonsterIntent::Attack {
                damage: attack_damage,
            }
        }
    } else {
        MonsterIntent::ApplyPlayerWeak { amount: weak }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_monster_intent_with_card_rng(
    monster: &mut MonsterState,
    player: &mut crate::PlayerState,
    piles: &mut CardPiles,
    allocated_card_id_through: u64,
    ascension: u8,
    player_before: &crate::PlayerState,
    relics: &[crate::Relic],
    card_random_rng: &mut StsRng,
) -> SimResult<i32> {
    apply_monster_intent_with_card_rng_and_revival(
        monster,
        player,
        piles,
        allocated_card_id_through,
        ascension,
        player_before,
        relics,
        true,
        card_random_rng,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_monster_intent_with_card_rng_and_revival(
    monster: &mut MonsterState,
    player: &mut crate::PlayerState,
    piles: &mut CardPiles,
    allocated_card_id_through: u64,
    ascension: u8,
    player_before: &crate::PlayerState,
    relics: &[crate::Relic],
    player_can_revive: bool,
    card_random_rng: &mut StsRng,
) -> SimResult<i32> {
    let local_allocated_through = monster
        .stasis_card
        .as_ref()
        .map_or_else(|| piles.max_card_instance_id(), |card| card.id.get())
        .max(piles.max_card_instance_id());
    if allocated_card_id_through < local_allocated_through {
        return Err(SimError::InvalidState(
            "monster intent card allocator trails authoritative state",
        ));
    }
    let mut next_monster = monster.clone();
    let mut next_player = player.clone();
    let mut next_piles = piles.clone();
    let mut next_card_random_rng = card_random_rng.clone();
    let damage = apply_monster_intent_with_card_rng_inner(
        &mut next_monster,
        &mut next_player,
        &mut next_piles,
        allocated_card_id_through,
        ascension,
        player_before,
        relics,
        player_can_revive,
        &mut next_card_random_rng,
    )?;
    *monster = next_monster;
    *player = next_player;
    *piles = next_piles;
    *card_random_rng = next_card_random_rng;
    Ok(damage)
}

fn checked_monster_intent_add(value: i32, amount: i32) -> SimResult<i32> {
    value
        .checked_add(amount)
        .ok_or(SimError::InvalidState("monster intent arithmetic overflow"))
}

fn checked_add_monster_intent_value(value: &mut i32, amount: i32) -> SimResult<()> {
    *value = checked_monster_intent_add(*value, amount)?;
    Ok(())
}

fn checked_add_monster_block_value(value: &mut i32, amount: i32) -> SimResult<()> {
    *value = checked_monster_intent_add(*value, amount)?.min(999);
    Ok(())
}

fn checked_monster_intent_mul(value: i32, amount: i32) -> SimResult<i32> {
    value
        .checked_mul(amount)
        .ok_or(SimError::InvalidState("monster intent arithmetic overflow"))
}

fn apply_player_weak_from_monster(
    powers: &mut crate::power::PlayerPowers,
    relics: &[crate::Relic],
    amount: i32,
) -> SimResult<()> {
    crate::relic::apply_player_weak_with_relics(powers, relics, amount)?;
    Ok(())
}

fn apply_player_vulnerable_from_monster(
    powers: &mut crate::power::PlayerPowers,
    amount: i32,
) -> SimResult<bool> {
    crate::power::apply_player_vulnerable(powers, amount)
}

fn apply_player_frail_from_monster(
    powers: &mut crate::power::PlayerPowers,
    relics: &[crate::Relic],
    amount: i32,
) -> SimResult<()> {
    crate::relic::apply_player_frail_with_relics(powers, relics, amount)?;
    Ok(())
}

fn apply_player_hex_from_monster(
    powers: &mut crate::power::PlayerPowers,
    amount: i32,
) -> SimResult<()> {
    crate::power::apply_player_hex(powers, amount)?;
    Ok(())
}

fn apply_player_entangled_from_monster(
    powers: &mut crate::power::PlayerPowers,
    amount: i32,
) -> SimResult<()> {
    crate::power::apply_player_entangled(powers, amount)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_monster_intent_with_card_rng_inner(
    monster: &mut MonsterState,
    player: &mut crate::PlayerState,
    piles: &mut CardPiles,
    allocated_card_id_through: u64,
    ascension: u8,
    player_before: &crate::PlayerState,
    relics: &[crate::Relic],
    player_can_revive: bool,
    card_random_rng: &mut StsRng,
) -> SimResult<i32> {
    use crate::combat::damage::deal_unmodified_damage_to_monster;
    use crate::combat::turn_powers::monster_damage_to_player_with_relics;
    use crate::power::{
        apply_player_confusion, apply_player_constricted, reduce_player_dexterity,
        reduce_player_strength,
    };

    let config = AscensionConfig::new(ascension);
    let source_scaled_damage = matches!(
        monster.content_id,
        SPHERIC_GUARDIAN_ID
            | MUGGER_ID
            | BOOK_OF_STABBING_ID
            | SNECKO_ID
            | CENTURION_ID
            | HEALER_ID
            | CHOSEN_ID
            | SNAKE_PLANT_ID
            | BYRD_ID
            | SHELLED_PARASITE_ID
            | FUNGI_BEAST_ID
            | SLAVER_BLUE_ID
            | SLAVER_RED_ID
            | GREMLIN_LEADER_ID
            | GREMLIN_WARRIOR_ID
            | GREMLIN_THIEF_ID
            | GREMLIN_FAT_ID
            | GREMLIN_TSUNDERE_ID
            | GREMLIN_WIZARD_ID
            | MAW_ID
            | SPIRE_GROWTH_ID
            | GIANT_HEAD_ID
            | NEMESIS_ID
            | TIME_EATER_ID
    );
    let scale_damage = |damage: i32| -> SimResult<i32> {
        if source_scaled_damage {
            Ok(damage)
        } else {
            damage
                .checked_add(config.normal_enemy_damage_bonus())
                .and_then(|damage| damage.checked_add(config.deadly_enemies_damage_bonus()))
                .ok_or(SimError::InvalidState("monster intent arithmetic overflow"))
        }
    };
    let mut block_after_thorns = 0;
    let total_thorns = checked_monster_intent_add(player.powers.thorns, player.temp_thorns)?;
    let mut thorns_already_applied = false;
    let guardian_was_in_defensive_mode =
        monster.content_id == GUARDIAN_ID && monster.in_defensive_mode;
    let monster_damage_to_player =
        |player: &crate::PlayerState, monster: &MonsterState, base: i32| {
            monster_damage_to_player_with_relics(player, monster, base, relics)
        };
    let (damage, thorns_hits) = match monster.intent {
        MonsterIntent::PendingAiRoll => {
            return Err(SimError::InvalidState(
                "combat monster intent is pending AI roll",
            ));
        }
        MonsterIntent::Attack { damage } => {
            let damage_taken =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            if monster.content_id == DAGGER_ID && damage == DAGGER_EXPLODE_DAMAGE {
                monster.hp = 0;
                monster.alive = false;
                monster.block = 0;
            }
            (damage_taken, 1)
        }
        MonsterIntent::Block { block } => {
            checked_add_monster_intent_value(&mut monster.block, block)?;
            (0, 0)
        }
        MonsterIntent::Ritual { amount } => {
            checked_add_monster_intent_value(&mut monster.powers.ritual, amount)?;
            (0, 0)
        }
        MonsterIntent::AttackAndBlock { damage, block } => {
            if monster.content_id == SPHERIC_GUARDIAN_ID {
                // Target SphericGuardian move 3 queues GainBlockAction before
                // DamageAction, so reactive thorns damage lands into the newly
                // gained block. GainBlockAction delegates to addBlock, which
                // caps creature block at the target's 999 maximum. Other
                // modeled AttackAndBlock users retain their source
                // attack-then-block ordering.
                checked_add_monster_block_value(&mut monster.block, block)?;
            } else {
                block_after_thorns = block;
            }
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::StrengthAndBlock { strength, block } => {
            if monster.content_id == SPIKER_ID {
                checked_add_monster_intent_value(&mut monster.powers.spiker_thorns_buffs, 1)?;
                checked_add_monster_intent_value(&mut monster.powers.spikes, SPIKER_THORNS_BUFF)?;
            } else if monster.content_id == CHAMP_ID {
                checked_add_monster_intent_value(&mut monster.block, block)?;
                checked_add_monster_intent_value(&mut monster.powers.metallicize, strength)?;
            } else {
                checked_add_monster_intent_value(&mut monster.powers.strength, strength)?;
                checked_add_monster_intent_value(&mut monster.block, block)?;
            }
            (0, 0)
        }
        MonsterIntent::StrengthSelf { amount } => {
            if monster.content_id == BYRD_ID && amount == 0 {
                monster.powers.flight = target_byrd_flight_amount(ascension);
            } else if monster.content_id == GREMLIN_NOB_ID {
                checked_add_monster_intent_value(&mut monster.powers.anger, amount)?;
            } else {
                checked_add_monster_intent_value(&mut monster.powers.strength, amount)?;
            }
            (0, 0)
        }
        MonsterIntent::ApplyPlayerWeak { amount } => {
            apply_player_weak_from_monster(&mut player.powers, relics, amount)?;
            (0, 0)
        }
        MonsterIntent::AttackApplyPlayerWeak { damage, weak } => {
            apply_player_weak_from_monster(&mut player.powers, relics, weak)?;
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerVulnerable { damage, vulnerable } => {
            let had_no_vulnerable = player.powers.vulnerable == 0;
            let applied = apply_player_vulnerable_from_monster(&mut player.powers, vulnerable)?;
            if had_no_vulnerable && applied {
                player.vulnerable_just_applied = true;
            }
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
            damage,
            weak,
            vulnerable,
        } => {
            apply_player_weak_from_monster(&mut player.powers, relics, weak)?;
            let had_no_vulnerable = player.powers.vulnerable == 0;
            let applied = apply_player_vulnerable_from_monster(&mut player.powers, vulnerable)?;
            if had_no_vulnerable && applied {
                player.vulnerable_just_applied = true;
            }
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerFrailAndVulnerable {
            damage,
            frail,
            vulnerable,
        } => {
            apply_player_frail_from_monster(&mut player.powers, relics, frail)?;
            let had_no_vulnerable = player.powers.vulnerable == 0;
            let applied = apply_player_vulnerable_from_monster(&mut player.powers, vulnerable)?;
            if had_no_vulnerable && applied {
                player.vulnerable_just_applied = true;
            }
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerFrailAndWeak {
            damage,
            frail,
            weak,
        } => {
            apply_player_frail_from_monster(&mut player.powers, relics, frail)?;
            apply_player_weak_from_monster(&mut player.powers, relics, weak)?;
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerFrail { damage, frail } => {
            apply_player_frail_from_monster(&mut player.powers, relics, frail)?;
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::AttackHealSelf { damage } => (
            monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
            0,
        ),
        MonsterIntent::ApplyPlayerHex { amount } => {
            apply_player_hex_from_monster(&mut player.powers, amount)?;
            (0, 0)
        }
        MonsterIntent::ApplyPlayerFrailAndWeak { frail, weak } => {
            if monster.content_id == WRITHING_MASS_ID {
                // This field is the existing per-monster one-shot move marker.
                // For Writhing Mass it mirrors `usedMegaDebuff`.
                monster.has_siphoned = true;
            } else {
                let applied_frail = if monster.content_id == SPIKE_SLIME_ID
                    && monster.max_hp > SPIKE_SLIME_M_A7_HP_RANGE.max
                {
                    spike_slime_frail_amount(monster.max_hp, ascension)
                } else {
                    frail
                };
                apply_player_frail_from_monster(&mut player.powers, relics, applied_frail)?;
                apply_player_weak_from_monster(&mut player.powers, relics, weak)?;
            }
            (0, 0)
        }
        MonsterIntent::ApplyPlayerFrailWeakVulnerable {
            frail,
            weak,
            vulnerable,
        } => {
            apply_player_frail_from_monster(&mut player.powers, relics, frail)?;
            apply_player_weak_from_monster(&mut player.powers, relics, weak)?;
            let had_no_vulnerable = player.powers.vulnerable == 0;
            let applied = apply_player_vulnerable_from_monster(&mut player.powers, vulnerable)?;
            if had_no_vulnerable && applied {
                player.vulnerable_just_applied = true;
            }
            (0, 0)
        }
        MonsterIntent::ApplyPlayerWeakStrengthSelf { weak, strength } => {
            apply_player_weak_from_monster(&mut player.powers, relics, weak)?;
            checked_add_monster_intent_value(&mut monster.powers.strength, strength)?;
            (0, 0)
        }
        MonsterIntent::ApplyPlayerConfusion => {
            apply_player_confusion(&mut player.powers)?;
            (0, 0)
        }
        MonsterIntent::ApplyPlayerEntangled { amount } => {
            apply_player_entangled_from_monster(&mut player.powers, amount)?;
            (0, 0)
        }
        MonsterIntent::ApplyPlayerConstricted { amount } => {
            apply_player_constricted(&mut player.powers, amount)?;
            (0, 0)
        }
        MonsterIntent::HealAllMonsters { .. }
        | MonsterIntent::StrengthAllMonsters { .. }
        | MonsterIntent::EncourageGremlins { .. }
        | MonsterIntent::SummonCollectorTorchHeads { .. }
        | MonsterIntent::SummonGremlins { .. } => (0, 0),
        MonsterIntent::AttackAddSlimedToDiscard { damage, .. } => (
            monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
            1,
        ),
        MonsterIntent::AttackAddVoidToDraw { damage, count } => {
            let damage_taken =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            if player.hp > 0 {
                add_cards_to_draw_random_spot(
                    piles,
                    VOID_ID,
                    count,
                    card_random_rng,
                    allocated_card_id_through,
                )?;
            }
            (damage_taken, 1)
        }
        MonsterIntent::AddSlimedToDiscard { count } => {
            add_cards_to_discard(piles, SLIMED_ID, count, allocated_card_id_through)?;
            (0, 0)
        }
        MonsterIntent::AttackAddWoundsToDiscard { damage, .. } => {
            let damage_taken =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            if monster.content_id == TASKMASTER_ID && ascension >= 18 {
                checked_add_monster_intent_value(
                    &mut monster.powers.strength,
                    TASKMASTER_A18_STRENGTH,
                )?;
            }
            (damage_taken, 1)
        }
        MonsterIntent::AttackStealGold { damage, amount } => {
            checked_add_monster_intent_value(&mut monster.stolen_gold, amount.max(0))?;
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                1,
            )
        }
        MonsterIntent::Escape => {
            monster.alive = false;
            monster.escaped = true;
            monster.block = 0;
            (0, 0)
        }
        MonsterIntent::Sleep => {
            if monster.sleep_turns_remaining > 0 {
                monster.sleep_turns_remaining -= 1;
            }
            if monster.content_id == LAGAVULIN_ID && monster.sleep_turns_remaining > 0 {
                monster.block = 8;
            }
            (0, 0)
        }
        MonsterIntent::Stun => {
            if monster.content_id == EXPLODER_ID && monster.powers.explosive > 0 {
                // Exploder's death blast is a THORNS-type DamageAction in the
                // target game: it ignores monster Strength/Weak and player
                // Vulnerable, while the later player damage pipeline still
                // applies block, Intangible, and Buffer.
                let damage_taken = EXPLODER_EXPLOSION_DAMAGE;
                monster.hp = 0;
                monster.alive = false;
                monster.block = 0;
                monster.powers.explosive = 0;
                (damage_taken, 0)
            } else {
                (0, 0)
            }
        }
        MonsterIntent::SiphonPlayer {
            strength,
            dexterity,
        } => {
            reduce_player_strength(&mut player.powers, strength)?;
            reduce_player_dexterity(&mut player.powers, dexterity)?;
            bronze_orb_apply_stasis(monster, piles, card_random_rng);
            monster.has_siphoned = true;
            (0, 0)
        }
        MonsterIntent::AddDazedToDiscard { count } => {
            add_cards_to_discard(piles, DAZED_ID, count, allocated_card_id_through)?;
            (0, 0)
        }
        MonsterIntent::AddDazedToDraw { count } => {
            add_cards_to_draw_random_spot(
                piles,
                DAZED_ID,
                count,
                card_random_rng,
                allocated_card_id_through,
            )?;
            (0, 0)
        }
        MonsterIntent::AddBurnToDiscard { count, damage } => {
            add_cards_to_discard(piles, BURN_ID, count, allocated_card_id_through)?;
            let thorns_hits = i32::from(damage > 0);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
                thorns_hits,
            )
        }
        MonsterIntent::AddBurnToDiscardAndDraw { damage, .. } => (
            monster_damage_to_player(player_before, monster, scale_damage(damage)?)?,
            1,
        ),
        MonsterIntent::AttackMultipleUpgradeBurns {
            damage,
            hits,
            count,
        } => {
            upgrade_burns_and_add_upgraded_to_discard(piles, count, allocated_card_id_through)?;
            let hit_damage =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            let effective_hits =
                apply_multi_hit_thorns(monster, total_thorns, hits, hit_damage, player_before);
            monster.intent = MonsterIntent::AttackMultipleUpgradeBurns {
                damage,
                hits: effective_hits,
                count,
            };
            thorns_already_applied = total_thorns > 0 && effective_hits > 0;
            (
                checked_monster_intent_mul(hit_damage, effective_hits)?,
                effective_hits,
            )
        }
        MonsterIntent::AttackMultiple { damage, hits } => {
            let hit_damage =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            let effective_hits =
                apply_multi_hit_thorns(monster, total_thorns, hits, hit_damage, player_before);
            monster.intent = MonsterIntent::AttackMultiple {
                damage,
                hits: effective_hits,
            };
            thorns_already_applied = total_thorns > 0 && effective_hits > 0;
            (
                checked_monster_intent_mul(hit_damage, effective_hits)?,
                effective_hits,
            )
        }
        MonsterIntent::AttackMultipleApplyPlayerWeak { damage, hits, weak } => {
            let hit_damage =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            let effective_hits =
                apply_multi_hit_thorns(monster, total_thorns, hits, hit_damage, player_before);
            monster.intent = MonsterIntent::AttackMultipleApplyPlayerWeak {
                damage,
                hits: effective_hits,
                weak,
            };
            thorns_already_applied = total_thorns > 0 && effective_hits > 0;
            (
                checked_monster_intent_mul(hit_damage, effective_hits)?,
                effective_hits,
            )
        }
        MonsterIntent::AttackMultipleAddDazedToDiscard {
            damage,
            hits,
            count,
        } => {
            let hit_damage =
                monster_damage_to_player(player_before, monster, scale_damage(damage)?)?;
            let effective_hits =
                apply_multi_hit_thorns(monster, total_thorns, hits, hit_damage, player_before);
            monster.intent = MonsterIntent::AttackMultipleAddDazedToDiscard {
                damage,
                hits: effective_hits,
                count,
            };
            thorns_already_applied = total_thorns > 0 && effective_hits > 0;
            (
                checked_monster_intent_mul(hit_damage, effective_hits)?,
                effective_hits,
            )
        }
        MonsterIntent::GuardianCloseUp { sharp_hide } => {
            monster.powers.spikes = sharp_hide;
            (0, 0)
        }
        MonsterIntent::DefensiveCharge { block, strength } => {
            checked_add_monster_intent_value(&mut monster.block, block)?;
            checked_add_monster_intent_value(&mut monster.powers.strength, strength)?;
            if monster.defensive_turns_remaining > 0 {
                monster.defensive_turns_remaining -= 1;
            }
            (0, 0)
        }
    };
    // ThornsPower queues a separate DamageAction after the incoming attack.
    // A lethal hit ends the combat action queue before that retaliation can
    // resolve, so a single-hit attack must not damage the monster after the
    // player would already be dead. Multi-hit attacks calculate this per hit
    // in `apply_multi_hit_thorns` above.
    let player_survives_single_hit =
        player_can_revive || player_survives_monster_hit(player_before, damage, relics);
    if total_thorns > 0 && thorns_hits > 0 && !thorns_already_applied && player_survives_single_hit
    {
        deal_unmodified_damage_to_monster(
            monster,
            checked_monster_intent_mul(total_thorns, thorns_hits)?,
        );
    }
    if monster.alive && block_after_thorns > 0 {
        checked_add_monster_intent_value(&mut monster.block, block_after_thorns)?;
    }
    if monster.alive && thorns_hits > 0 && monster.powers.strength_up > 0 {
        checked_add_monster_intent_value(&mut monster.powers.strength, monster.powers.strength_up)?;
    }
    if monster.content_id == GUARDIAN_ID
        && monster.in_defensive_mode
        && guardian_was_in_defensive_mode
    {
        finish_guardian_defensive_turn(monster);
    }
    if !lagavulin_sleep_or_stun(monster.content_id, monster.intent) {
        monster.moves_executed = monster
            .moves_executed
            .checked_add(1)
            .ok_or(SimError::InvalidState("monster intent arithmetic overflow"))?;
    }
    if monster.content_id == GUARDIAN_ID
        && monster.in_defensive_mode
        && !guardian_was_in_defensive_mode
    {
        monster.intent = guardian_intent(
            true,
            monster.defensive_turns_remaining,
            monster.moves_executed,
            ascension,
        );
    }
    Ok(damage)
}

fn player_survives_monster_hit(
    player: &crate::PlayerState,
    damage: i32,
    relics: &[crate::Relic],
) -> bool {
    let incoming = crate::combat::hp_loss::cap_player_damage_with_intangible(player, damage);
    let blocked = player.block.min(incoming);
    let mitigated = crate::relic::mitigate_unblocked_attack_damage(relics, incoming - blocked);
    let mut powers = player.powers.clone();
    let hp_damage = crate::relic::apply_buffer_to_hp_loss(&mut powers, mitigated);
    player.hp > hp_damage
}

fn apply_multi_hit_thorns(
    monster: &mut MonsterState,
    total_thorns: i32,
    hits: i32,
    hit_damage: i32,
    player_before: &crate::PlayerState,
) -> i32 {
    let hit_count = hits.max(1);
    if total_thorns <= 0 {
        return hit_count;
    }

    let mut effective_hits = 0;
    let mut remaining_block = player_before.block;
    let mut remaining_hp = player_before.hp;
    for _ in 0..hit_count {
        if !monster.alive {
            break;
        }
        effective_hits += 1;
        let blocked = remaining_block.min(hit_damage.max(0));
        remaining_block -= blocked;
        remaining_hp -= hit_damage.saturating_sub(blocked);
        let player_survives_hit = hit_damage <= 0 || remaining_hp > 0;
        if player_survives_hit {
            let hp_damage =
                crate::combat::damage::deal_unmodified_damage_to_monster_deferred_guardian(
                    monster,
                    total_thorns,
                );
            if monster.content_id == GUARDIAN_ID {
                guardian_accumulate_hp_damage(monster, hp_damage);
            }
        }
    }
    if monster.content_id == GUARDIAN_ID
        && monster.alive
        && !monster.in_defensive_mode
        && monster.mode_shift <= 0
    {
        enter_guardian_defensive_mode(monster);
    }
    effective_hits
}

fn lagavulin_sleep_or_stun(content_id: ContentId, intent: MonsterIntent) -> bool {
    content_id == LAGAVULIN_ID && matches!(intent, MonsterIntent::Sleep | MonsterIntent::Stun)
}

pub fn release_stasis_card_on_death(monster: &mut MonsterState, piles: &mut CardPiles) {
    let Some(card) = monster.stasis_card.take() else {
        return;
    };
    if piles.hand.len() < 10 {
        piles.hand.push(card);
    } else {
        piles.discard_pile.push(card);
    }
}

fn bronze_orb_apply_stasis(
    monster: &mut MonsterState,
    piles: &mut CardPiles,
    card_random_rng: &mut StsRng,
) {
    if monster.content_id != BRONZE_ORB_ID || monster.stasis_card.is_some() {
        return;
    }
    let Some(card) = take_stasis_card(piles, card_random_rng) else {
        return;
    };
    monster.stasis_card = Some(card);
}

fn take_stasis_card(piles: &mut CardPiles, card_random_rng: &mut StsRng) -> Option<CardInstance> {
    if !piles.draw_pile.is_empty() {
        return take_random_card_by_stasis_priority(&mut piles.draw_pile, card_random_rng);
    }
    if !piles.discard_pile.is_empty() {
        return take_random_card_by_stasis_priority(&mut piles.discard_pile, card_random_rng);
    }
    None
}

fn take_random_card_by_stasis_priority(
    pile: &mut Vec<CardInstance>,
    rng: &mut StsRng,
) -> Option<CardInstance> {
    for rarity in [CardRarity::Rare, CardRarity::Uncommon, CardRarity::Common] {
        if let Some(card) = take_random_card_of_rarity(pile, rng, rarity) {
            return Some(card);
        }
    }
    if pile.is_empty() {
        return None;
    }
    let index = rng.random_int((pile.len() - 1) as i32) as usize;
    Some(pile.remove(index))
}

fn take_random_card_of_rarity(
    pile: &mut Vec<CardInstance>,
    rng: &mut StsRng,
    rarity: CardRarity,
) -> Option<CardInstance> {
    let mut candidate_indices = pile
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            let key = crate::content::cards::get_card_definition(card.content_id)?.key;
            card_matches_stasis_rarity(card.content_id, rarity).then_some((index, key))
        })
        .collect::<Vec<_>>();
    if candidate_indices.is_empty() {
        return None;
    }
    // AbstractCard.compareTo sorts by the source cardID. The simulator's
    // content keys are not always source IDs (for example, INTIMIDATE and
    // SHOCKWAVE are internal uppercase keys), while the card name preserves
    // the source ordering for these cards. Upgraded cards retain their base
    // cardID, so remove the display-only '+' suffix before sorting.
    candidate_indices.sort_by_key(|(index, _)| stasis_card_source_id(pile[*index].content_id));
    let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
    Some(pile.remove(candidate_indices[pick].0))
}

fn stasis_card_source_id(content_id: ContentId) -> &'static str {
    get_card_definition(content_id)
        .map(|definition| definition.name.trim_end_matches('+'))
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        APPARITION_ID, APPARITION_PLUS_ID, BARRICADE_ID, BERSERK_ID, BITE_ID, BITE_PLUS_ID, JAX_ID,
        JAX_PLUS_ID, RITUAL_DAGGER_ID, WOUND_ID,
    };
    use crate::ids::CardId;

    #[test]
    fn stasis_uses_target_card_rarity() {
        for id in [
            APPARITION_ID,
            APPARITION_PLUS_ID,
            BITE_ID,
            BITE_PLUS_ID,
            RITUAL_DAGGER_ID,
            JAX_ID,
            JAX_PLUS_ID,
        ] {
            assert!(!card_matches_stasis_rarity(id, CardRarity::Rare));
            assert!(!card_matches_stasis_rarity(id, CardRarity::Uncommon));
            assert!(!card_matches_stasis_rarity(id, CardRarity::Common));
        }
        assert!(card_matches_stasis_rarity(WOUND_ID, CardRarity::Common));

        let mut pile = vec![
            CardInstance::new(CardId::new(1), APPARITION_ID),
            CardInstance::new(CardId::new(2), BARRICADE_ID),
            CardInstance::new(CardId::new(3), BERSERK_ID),
        ];
        let mut rng = StsRng::new(3);
        let selected = take_random_card_of_rarity(&mut pile, &mut rng, CardRarity::Rare)
            .expect("the rare Stasis pool contains Barricade and Berserk");

        assert_eq!(selected.content_id, BARRICADE_ID);
    }

    #[test]
    fn stasis_sorts_by_source_card_id_not_internal_content_key() {
        use crate::content::cards::{
            BATTLE_TRANCE_ID, ENTRENCH_ID, INFLAME_PLUS_ID, INTIMIDATE_ID, RAMPAGE_ID,
            SHOCKWAVE_ID, UPPERCUT_PLUS_ID, WHIRLWIND_ID,
        };

        let mut cards = [
            INTIMIDATE_ID,
            INFLAME_PLUS_ID,
            UPPERCUT_PLUS_ID,
            WHIRLWIND_ID,
            SHOCKWAVE_ID,
            RAMPAGE_ID,
            ENTRENCH_ID,
            BATTLE_TRANCE_ID,
        ];
        cards.sort_by_key(|content_id| stasis_card_source_id(*content_id));

        assert_eq!(
            cards,
            [
                BATTLE_TRANCE_ID,
                ENTRENCH_ID,
                INFLAME_PLUS_ID,
                INTIMIDATE_ID,
                RAMPAGE_ID,
                SHOCKWAVE_ID,
                UPPERCUT_PLUS_ID,
                WHIRLWIND_ID,
            ]
        );
    }

    #[test]
    fn target_hp_generation_wraps_floor_seed_like_java_long() {
        assert_eq!(
            target_cultist_hp_roll(i64::MAX, 1, 0),
            target_cultist_hp_roll(i64::MIN, 0, 0)
        );
    }

    #[test]
    fn sphere_and_two_shapes_uses_misc_rng_for_both_shape_identities() {
        let seed = 772_776_727_775_i64;
        let floor = 41;
        let mut misc_rng = StsRng::new(seed.wrapping_add(i64::from(floor)));

        let spawns = target_beyond_encounter_spawn_for_key_with_misc_rng(
            seed,
            floor,
            "Sphere and 2 Shapes",
            0,
            false,
            &mut misc_rng,
        )
        .expect("encounter is implemented");

        assert_eq!(
            spawns
                .iter()
                .map(|spawn| (spawn.name, spawn.max_hp))
                .collect::<Vec<_>>(),
            vec![("Repulsor", 32), ("Repulsor", 33), ("Spheric Guardian", 20),]
        );
        assert_eq!(spawns[2].block, SPHERIC_GUARDIAN_STARTING_BLOCK);
        assert_eq!(
            spawns[2].powers,
            vec![TargetSpawnPower {
                id: "Artifact",
                amount: SPHERIC_GUARDIAN_ARTIFACT,
            }]
        );
        assert_eq!(misc_rng.counter(), 2);
    }

    #[test]
    fn ancient_shape_opening_intents_are_deferred_to_combat_ai_roll() {
        let seed = 772_776_727_775_i64;
        let floor = 41;
        let mut misc_rng = StsRng::new(seed.wrapping_add(i64::from(floor)));

        let spawns = target_beyond_encounter_spawn_for_key_with_misc_rng(
            seed,
            floor,
            "3 Shapes",
            0,
            false,
            &mut misc_rng,
        )
        .expect("encounter is implemented");

        assert_eq!(spawns.len(), 3);
        for spawn in spawns {
            match spawn.name {
                "Spiker" | "Repulsor" => {
                    assert_eq!(spawn.intent, TargetSpawnIntent::PendingAiRoll)
                }
                "Exploder" => assert_eq!(
                    spawn.intent,
                    TargetSpawnIntent::Attack {
                        damage: EXPLODER_ATTACK_DAMAGE
                    }
                ),
                _ => unreachable!("3 Shapes contains only ancient shapes"),
            }
        }
    }

    #[test]
    fn city_gremlin_leader_entry_requires_and_consumes_misc_rng() {
        let seed = 772_776_727_775_i64;
        let floor = 33;
        let mut expected_rng = StsRng::new(seed.wrapping_add(i64::from(floor)));
        let expected_names = vec![
            target_random_gremlin_name(&mut expected_rng),
            target_random_gremlin_name(&mut expected_rng),
            "GremlinLeader",
        ];
        let mut misc_rng = StsRng::new(seed.wrapping_add(i64::from(floor)));

        let spawns = target_city_encounter_spawn_for_key_with_misc_rng(
            seed,
            floor,
            "Gremlin Leader",
            0,
            false,
            &mut misc_rng,
        )
        .expect("Gremlin Leader encounter is implemented");

        assert_eq!(
            spawns.iter().map(|spawn| spawn.name).collect::<Vec<_>>(),
            expected_names
        );
        assert_eq!(misc_rng.counter(), 2);
    }

    #[test]
    fn guardian_vent_steam_applies_weak_and_vulnerable() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.moves_executed = 2;
        monster.intent = guardian_intent(false, 0, monster.moves_executed, 0);
        let mut player = state.player.clone();
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(0));
        assert_eq!(player.powers.weak, GUARDIAN_VENT_DEBUFF);
        assert_eq!(player.powers.vulnerable, GUARDIAN_VENT_DEBUFF);
        assert!(player.vulnerable_just_applied);
    }

    #[test]
    fn guardian_thorns_enters_defensive_mode_without_consuming_first_turn() {
        let mut state = crate::CombatState::initial_fixture();
        state.player.powers.thorns = 3;
        state.player.powers.vulnerable = 2;
        let relics = [crate::Relic::OddMushroom];
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.moves_executed = 3;
        monster.mode_shift = 12;
        monster.intent = MonsterIntent::AttackMultiple {
            damage: GUARDIAN_WHIRLWIND_DAMAGE,
            hits: GUARDIAN_WHIRLWIND_HITS,
        };
        let mut player = state.player.clone();
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &relics,
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(24));
        assert_eq!(monster.hp, 228);
        assert!(monster.in_defensive_mode);
        assert_eq!(monster.defensive_turns_remaining, 3);
        assert_eq!(
            monster.intent,
            MonsterIntent::GuardianCloseUp { sharp_hide: 3 }
        );
    }

    #[test]
    fn champ_taunt_applies_weak_and_vulnerable() {
        assert_eq!(
            target_champ_next_intent_from_roll(&[1, 2, 1], 99, 349, 420, 0),
            MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                frail: 0,
                weak: 2,
                vulnerable: 2,
            }
        );

        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&CHAMP_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerFrailWeakVulnerable {
            frail: 0,
            weak: 2,
            vulnerable: 2,
        };
        let mut player = state.player.clone();
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(0));
        assert_eq!(player.powers.weak, 2);
        assert_eq!(player.powers.vulnerable, 2);
        assert!(player.vulnerable_just_applied);
    }

    #[test]
    fn champ_face_slap_applies_frail_and_vulnerable() {
        let intent = target_champ_next_intent_from_roll(&[], 31, 349, 420, 0);
        assert_eq!(
            intent,
            MonsterIntent::AttackApplyPlayerFrailAndVulnerable {
                damage: CHAMP_FACE_SLAP_DAMAGE,
                frail: CHAMP_FACE_SLAP_FRAIL,
                vulnerable: 2,
            }
        );
        assert_eq!(target_move_byte(CHAMP_ID, intent), Some(4));
    }

    #[test]
    fn monster_intent_rejects_block_overflow_without_mutating_inputs() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.block = i32::MAX;
        monster.intent = MonsterIntent::Block { block: 1 };
        let mut player = state.player.clone();
        let player_before = player.clone();
        let monster_before = monster.clone();
        let piles_before = state.piles.clone();
        let rng_before = state.rng.card_random_rng.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let result = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState("monster intent arithmetic overflow"))
        );
        assert_eq!(monster, monster_before);
        assert_eq!(player, player_before);
        assert_eq!(state.piles, piles_before);
        assert_eq!(state.rng.card_random_rng, rng_before);
    }

    #[test]
    fn spheric_guardian_gain_block_caps_at_creature_block_limit() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&SPHERIC_GUARDIAN_A0, MonsterId::new(1));
        monster.block = 994;
        monster.intent = MonsterIntent::AttackAndBlock {
            damage: 10,
            block: 15,
        };
        let mut player = state.player.clone();
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(10));
        assert_eq!(monster.block, 999);
    }

    #[test]
    fn monster_intent_rolls_back_when_move_counter_overflows_after_effect() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.moves_executed = u32::MAX;
        monster.intent = MonsterIntent::StrengthSelf { amount: 2 };
        let mut player = state.player.clone();
        let player_before = player.clone();
        let monster_before = monster.clone();
        let piles_before = state.piles.clone();
        let rng_before = state.rng.card_random_rng.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let result = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState("monster intent arithmetic overflow"))
        );
        assert_eq!(monster, monster_before);
        assert_eq!(player, player_before);
        assert_eq!(state.piles, piles_before);
        assert_eq!(state.rng.card_random_rng, rng_before);
    }

    #[test]
    fn monster_intent_rejects_player_debuff_overflow_transactionally() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak { amount: 1 };
        let mut player = state.player.clone();
        player.powers.weak = i32::MAX;
        let player_before = player.clone();
        let monster_before = monster.clone();
        let piles_before = state.piles.clone();
        let rng_before = state.rng.card_random_rng.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let result = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "player Weak application overflows i32"
            ))
        );
        assert_eq!(monster, monster_before);
        assert_eq!(player, player_before);
        assert_eq!(state.piles, piles_before);
        assert_eq!(state.rng.card_random_rng, rng_before);
    }

    #[test]
    fn guardian_multi_hit_thorns_counts_hits_absorbed_by_block() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.hp = 40;
        monster.powers.weak = 5;
        monster.intent = MonsterIntent::AttackMultiple {
            damage: GUARDIAN_WHIRLWIND_DAMAGE,
            hits: GUARDIAN_WHIRLWIND_HITS,
        };
        let mut player = state.player.clone();
        player.hp = 11;
        player.block = 8;
        player.powers.thorns = 3;
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(12));
        assert_eq!(monster.hp, 28);
        assert_eq!(player.hp, 11);
    }

    #[test]
    fn guardian_multi_hit_thorns_defer_mode_shift_block_until_all_hits() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        monster.hp = 211;
        monster.mode_shift = 1;
        monster.intent = MonsterIntent::AttackMultiple {
            damage: GUARDIAN_WHIRLWIND_DAMAGE,
            hits: GUARDIAN_WHIRLWIND_HITS,
        };
        let mut player = state.player.clone();
        player.hp = 100;
        player.block = 15;
        player.powers.thorns = 3;
        player.temp_thorns = 4;
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        )
        .expect("Guardian multi-hit attack should resolve");

        assert_eq!(monster.hp, 183);
        assert_eq!(monster.block, 20);
        assert!(monster.in_defensive_mode);
    }

    #[test]
    fn lethal_single_hit_does_not_apply_queued_thorns_damage() {
        let mut state = crate::CombatState::initial_fixture();
        state.player.hp = 1;
        state.player.powers.thorns = 3;
        let mut monster = monster_state(&CENTURION_A0, MonsterId::new(1));
        monster.hp = 10;
        monster.intent = MonsterIntent::Attack { damage: 2 };
        let mut player = state.player.clone();
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng_and_revival(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            false,
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(2));
        assert_eq!(monster.hp, 10);
    }

    #[test]
    fn gremlin_nob_a18_ignores_roll_and_uses_source_history_guards() {
        assert_eq!(
            target_gremlin_nob_next_intent_from_roll(&[3], 99, 18),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: GREMLIN_NOB_A3_SKULL_BASH_DAMAGE,
                vulnerable: 2
            }
        );
        assert_eq!(
            target_gremlin_nob_next_intent_from_roll(&[3, 2], 0, 18),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_A3_RUSH_DAMAGE
            }
        );
        assert_eq!(
            target_gremlin_nob_next_intent_from_roll(&[3, 2, 1, 1], 99, 18),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: GREMLIN_NOB_A3_SKULL_BASH_DAMAGE,
                vulnerable: 2
            }
        );
        assert_eq!(
            target_gremlin_nob_next_intent_from_roll(&[3], 99, 17),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_A3_RUSH_DAMAGE
            }
        );
    }

    #[test]
    fn gremlin_nob_a0_low_roll_can_repeat_skull_bash() {
        assert_eq!(
            target_gremlin_nob_next_intent_from_roll(&[3, 1, 1, 2], 0, 0),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: GREMLIN_NOB_SKULL_BASH_DAMAGE,
                vulnerable: 2
            }
        );
    }

    #[test]
    fn red_slaver_first_move_is_fixed_stab_despite_high_roll() {
        assert_eq!(
            target_slaver_red_next_intent_from_roll(&[], 99, 2),
            MonsterIntent::Attack {
                damage: SLAVER_RED_A2_STAB_DAMAGE
            }
        );
        assert_eq!(
            target_slaver_red_next_intent_from_roll(&[1], 99, 0),
            MonsterIntent::ApplyPlayerEntangled {
                amount: SLAVER_RED_ENTANGLED
            }
        );
    }

    #[test]
    fn gremlin_leader_helper_consumes_source_replacement_roll_ranges() {
        let mut low_branch_expected_rng = StsRng::new(123);
        let low_branch_replacement = low_branch_expected_rng.random_int_range(50, 99);
        let mut low_branch_scratch_rng = StsRng::new(0);
        let low_branch_expected = target_gremlin_leader_next_intent_from_roll(
            &[2],
            low_branch_replacement,
            &mut low_branch_scratch_rng,
            1,
            18,
        );
        let mut low_branch_rng = StsRng::new(123);

        let low_branch_actual =
            target_gremlin_leader_next_intent_from_roll(&[2], 0, &mut low_branch_rng, 1, 18);

        assert_eq!(low_branch_actual, low_branch_expected);
        assert_eq!(low_branch_scratch_rng.counter(), 0);
        assert_eq!(low_branch_rng.counter(), 1);

        let mut high_branch_expected_rng = StsRng::new(456);
        let high_branch_replacement = high_branch_expected_rng.random_int(80);
        let mut high_branch_scratch_rng = StsRng::new(0);
        let high_branch_expected = target_gremlin_leader_next_intent_from_roll(
            &[4],
            high_branch_replacement,
            &mut high_branch_scratch_rng,
            1,
            18,
        );
        let mut high_branch_rng = StsRng::new(456);

        let high_branch_actual =
            target_gremlin_leader_next_intent_from_roll(&[4], 90, &mut high_branch_rng, 1, 18);

        assert_eq!(high_branch_actual, high_branch_expected);
        assert_eq!(high_branch_scratch_rng.counter(), 0);
        assert_eq!(high_branch_rng.counter(), 1);
    }

    #[test]
    fn reptomancer_helper_uses_source_table_can_spawn_and_replacements() {
        let mut no_reroll_rng = StsRng::new(0);
        assert_eq!(
            target_reptomancer_next_intent_from_roll(&[], 99, false, &mut no_reroll_rng, 18,),
            MonsterIntent::SummonGremlins { count: 2 }
        );
        assert_eq!(
            target_reptomancer_next_intent_from_roll(&[2], 32, true, &mut no_reroll_rng, 3,),
            MonsterIntent::AttackMultipleApplyPlayerWeak {
                damage: REPTOMANCER_A3_SNAKE_STRIKE_DAMAGE,
                hits: REPTOMANCER_SNAKE_STRIKE_HITS,
                weak: 1,
            }
        );
        assert_eq!(
            target_reptomancer_next_intent_from_roll(&[1], 50, false, &mut no_reroll_rng, 3,),
            MonsterIntent::AttackMultipleApplyPlayerWeak {
                damage: REPTOMANCER_A3_SNAKE_STRIKE_DAMAGE,
                hits: REPTOMANCER_SNAKE_STRIKE_HITS,
                weak: 1,
            }
        );
        assert_eq!(
            target_reptomancer_next_intent_from_roll(&[1], 50, true, &mut no_reroll_rng, 18,),
            MonsterIntent::SummonGremlins { count: 2 }
        );
        assert_eq!(no_reroll_rng.counter(), 0);

        let mut low_branch_expected_rng = StsRng::new(123);
        let low_branch_replacement = low_branch_expected_rng.random_int_range(33, 99);
        let mut low_branch_scratch_rng = StsRng::new(0);
        let low_branch_expected = target_reptomancer_next_intent_from_roll(
            &[1],
            low_branch_replacement,
            true,
            &mut low_branch_scratch_rng,
            18,
        );
        let mut low_branch_rng = StsRng::new(123);
        let low_branch_actual =
            target_reptomancer_next_intent_from_roll(&[1], 0, true, &mut low_branch_rng, 18);
        assert_eq!(low_branch_actual, low_branch_expected);
        assert_eq!(low_branch_scratch_rng.counter(), 0);
        assert_eq!(low_branch_rng.counter(), 1);

        let mut high_branch_expected_rng = StsRng::new(456);
        let high_branch_replacement = high_branch_expected_rng.random_int(65);
        let mut high_branch_scratch_rng = StsRng::new(0);
        let high_branch_expected = target_reptomancer_next_intent_from_roll(
            &[3],
            high_branch_replacement,
            true,
            &mut high_branch_scratch_rng,
            18,
        );
        let mut high_branch_rng = StsRng::new(456);
        let high_branch_actual =
            target_reptomancer_next_intent_from_roll(&[3], 90, true, &mut high_branch_rng, 18);
        assert_eq!(high_branch_actual, high_branch_expected);
        assert_eq!(high_branch_scratch_rng.counter(), 0);
        assert_eq!(high_branch_rng.counter(), 1);
    }

    #[test]
    fn reptomancer_snake_strike_records_move_one() {
        assert_eq!(
            target_move_byte(
                REPTOMANCER_ID,
                MonsterIntent::AttackMultipleApplyPlayerWeak {
                    damage: REPTOMANCER_A3_SNAKE_STRIKE_DAMAGE,
                    hits: REPTOMANCER_SNAKE_STRIKE_HITS,
                    weak: 1,
                },
            ),
            Some(1)
        );
    }

    #[test]
    fn source_move_bytes_cover_cultist_and_small_acid_slime() {
        assert_eq!(
            target_move_byte(CULTIST_ID, MonsterIntent::Ritual { amount: 3 }),
            Some(3)
        );
        assert_eq!(
            target_move_byte(CULTIST_ID, MonsterIntent::Attack { damage: 6 }),
            Some(1)
        );

        let mut slime = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        slime.max_hp = ACID_SLIME_S_A7_HP_RANGE.max;
        slime.intent = MonsterIntent::ApplyPlayerWeak { amount: 1 };
        assert_eq!(target_move_byte_for_monster(&slime), Some(2));

        assert_eq!(
            target_move_byte(
                GUARDIAN_ID,
                MonsterIntent::ApplyPlayerFrailWeakVulnerable {
                    frail: 0,
                    weak: 2,
                    vulnerable: 2,
                },
            ),
            Some(7)
        );
    }

    #[test]
    fn hexaghost_cycle_move_bytes_match_target() {
        assert_eq!(target_move_byte(HEXAGHOST_ID, MonsterIntent::Stun), Some(5));
        assert_eq!(
            target_move_byte(
                HEXAGHOST_ID,
                MonsterIntent::AttackMultiple { damage: 5, hits: 6 }
            ),
            Some(1)
        );
        assert_eq!(
            target_move_byte(
                HEXAGHOST_ID,
                MonsterIntent::AddBurnToDiscard {
                    count: 1,
                    damage: 6,
                }
            ),
            Some(4)
        );
    }

    #[test]
    fn burn_attack_applies_player_vulnerable() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&HEXAGHOST_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AddBurnToDiscard {
            count: 1,
            damage: HEXAGHOST_DIVIDER_DAMAGE,
        };
        let mut player = state.player.clone();
        player.powers.vulnerable = 1;
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(9));
        assert_eq!(state.piles.discard_pile.len(), 1);
        assert_eq!(state.piles.discard_pile[0].content_id, BURN_ID);
    }

    #[test]
    fn exploder_death_blast_ignores_player_vulnerable() {
        let mut state = crate::CombatState::initial_fixture();
        let mut monster = monster_state(&EXPLODER_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Stun;
        monster.powers.explosive = 1;
        let mut player = state.player.clone();
        player.powers.vulnerable = 1;
        let player_before = player.clone();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();

        let damage = apply_monster_intent_with_card_rng(
            &mut monster,
            &mut player,
            &mut state.piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut state.rng.card_random_rng,
        );

        assert_eq!(damage, Ok(EXPLODER_EXPLOSION_DAMAGE));
        assert!(!monster.alive);
        assert_eq!(monster.hp, 0);
    }

    #[test]
    fn masked_bandit_move_bytes_match_scripted_sequence() {
        assert_eq!(
            target_move_byte(
                BANDIT_POINTY_ID,
                MonsterIntent::AttackMultiple { damage: 5, hits: 2 }
            ),
            Some(1)
        );
        assert_eq!(
            target_move_byte(BANDIT_LEADER_ID, MonsterIntent::Stun),
            Some(2)
        );
        assert_eq!(
            target_move_byte(
                BANDIT_BEAR_ID,
                MonsterIntent::SiphonPlayer {
                    strength: 0,
                    dexterity: 2,
                }
            ),
            Some(2)
        );
    }

    #[test]
    fn jaw_worm_source_helper_consumes_replacement_boolean_only_on_guarded_branches() {
        let mut no_guard_rng = StsRng::new(123);
        assert_eq!(
            target_jaw_worm_next_intent_from_roll(&[3], 24, &mut no_guard_rng),
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
        assert_eq!(no_guard_rng.counter(), 0);

        let mut last_chomp_rng = StsRng::new(123);
        let last_chomp = target_jaw_worm_next_intent_from_roll(&[1], 24, &mut last_chomp_rng);
        assert!(matches!(
            last_chomp,
            MonsterIntent::StrengthAndBlock { .. } | MonsterIntent::AttackAndBlock { .. }
        ));
        assert_eq!(last_chomp_rng.counter(), 1);

        let mut last_two_thrash_rng = StsRng::new(123);
        let last_two_thrash =
            target_jaw_worm_next_intent_from_roll(&[3, 3], 54, &mut last_two_thrash_rng);
        assert!(matches!(
            last_two_thrash,
            MonsterIntent::Attack { .. } | MonsterIntent::StrengthAndBlock { .. }
        ));
        assert_eq!(last_two_thrash_rng.counter(), 1);

        let mut last_bellow_rng = StsRng::new(123);
        let last_bellow = target_jaw_worm_next_intent_from_roll(&[2], 55, &mut last_bellow_rng);
        assert!(matches!(
            last_bellow,
            MonsterIntent::Attack { .. } | MonsterIntent::AttackAndBlock { .. }
        ));
        assert_eq!(last_bellow_rng.counter(), 1);
    }

    #[test]
    fn mixed_exordium_louse_rolls_curl_up_after_candidate_constructor_draws() {
        let ascension = 17;
        let floor = 12;
        let thugs_seed = (0..5000)
            .find(|seed| {
                target_exordium_thugs_spawn_states(*seed, floor, ascension, false)[0]
                    .name
                    .starts_with("Louse")
            })
            .expect("test seed with louse in Exordium Thugs weak slot");
        let wildlife_seed = (0..5000)
            .find(|seed| {
                target_exordium_wildlife_spawn_states(*seed, floor, ascension, false)[1]
                    .name
                    .starts_with("Louse")
            })
            .expect("test seed with louse in Exordium Wildlife weak slot");

        let thugs = target_exordium_thugs_spawn_states(thugs_seed, floor, ascension, false);
        let wildlife =
            target_exordium_wildlife_spawn_states(wildlife_seed, floor, ascension, false);

        assert_eq!(
            louse_curl_up_amount(&thugs[0]),
            Some(expected_exordium_thugs_louse_curl_up(
                thugs_seed, floor, ascension
            ))
        );
        assert_eq!(
            louse_curl_up_amount(&wildlife[1]),
            Some(expected_exordium_wildlife_louse_curl_up(
                wildlife_seed,
                floor,
                ascension
            ))
        );
    }

    fn louse_curl_up_amount(spawn: &TargetEncounterSpawn) -> Option<i32> {
        spawn
            .powers
            .iter()
            .find(|power| power.id == "Curl Up")
            .map(|power| power.amount)
    }

    fn expected_exordium_thugs_louse_curl_up(seed: i64, floor: u32, ascension: u8) -> i32 {
        let seed = seed.wrapping_add(i64::from(floor));
        let mut misc_rng = StsRng::new(seed);
        let mut hp_rng = StsRng::new(seed);

        let louse_is_normal = misc_rng.random_bool();
        let _weak_index = misc_rng.random_int_range(0, 2);
        let louse_hp_range = if louse_is_normal {
            target_louse_normal_hp_range(ascension)
        } else {
            target_louse_defensive_hp_range(ascension)
        };
        let _louse_hp = louse_hp_range.roll(&mut hp_rng);
        let _louse_bite_damage = target_louse_bite_damage_range(ascension).roll(&mut hp_rng);
        let _spike_hp = target_spike_slime_m_hp_range(ascension).roll(&mut hp_rng);
        let _acid_hp = target_acid_slime_m_hp_range(ascension).roll(&mut hp_rng);

        let _slaver_is_red = misc_rng.random_bool();
        let _strong_index = misc_rng.random_int_range(0, 2);
        let _cultist_hp = target_cultist_hp_range(ascension).roll(&mut hp_rng);
        let _slaver_hp = target_slaver_hp_range(ascension).roll(&mut hp_rng);
        let _looter_hp = target_looter_hp_range(ascension).roll(&mut hp_rng);

        target_louse_curl_up_range(ascension).roll(&mut hp_rng)
    }

    fn expected_exordium_wildlife_louse_curl_up(seed: i64, floor: u32, ascension: u8) -> i32 {
        let seed = seed.wrapping_add(i64::from(floor));
        let mut misc_rng = StsRng::new(seed);
        let mut hp_rng = StsRng::new(seed);

        let _fungi_hp = if ascension >= 7 {
            FUNGI_BEAST_A7_HP_RANGE
        } else {
            FUNGI_BEAST_A0_HP_RANGE
        }
        .roll(&mut hp_rng);
        let _jaw_worm_hp = target_jaw_worm_hp_range(ascension).roll(&mut hp_rng);
        let _strong_index = misc_rng.random_int_range(0, 1);

        let louse_is_normal = misc_rng.random_bool();
        let louse_hp_range = if louse_is_normal {
            target_louse_normal_hp_range(ascension)
        } else {
            target_louse_defensive_hp_range(ascension)
        };
        let _louse_hp = louse_hp_range.roll(&mut hp_rng);
        let _louse_bite_damage = target_louse_bite_damage_range(ascension).roll(&mut hp_rng);
        let _spike_hp = target_spike_slime_m_hp_range(ascension).roll(&mut hp_rng);
        let _acid_hp = target_acid_slime_m_hp_range(ascension).roll(&mut hp_rng);
        let _weak_index = misc_rng.random_int_range(0, 2);

        target_louse_curl_up_range(ascension).roll(&mut hp_rng)
    }

    #[test]
    fn large_acid_slime_source_helper_uses_last_two_history_guards() {
        let mut unguarded_rng = StsRng::new(123);
        assert_eq!(
            target_large_acid_slime_next_intent_from_roll(&[1], 29, &mut unguarded_rng, 0),
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: ACID_SLIME_L_WOUND_TACKLE_DAMAGE,
                count: 2
            }
        );
        assert_eq!(unguarded_rng.counter(), 0);

        let mut repeated_wound_rng = StsRng::new(123);
        let repeated_wound =
            target_large_acid_slime_next_intent_from_roll(&[1, 1], 29, &mut repeated_wound_rng, 0);
        assert!(matches!(
            repeated_wound,
            MonsterIntent::Attack { .. } | MonsterIntent::ApplyPlayerWeak { .. }
        ));
        assert_eq!(repeated_wound_rng.counter(), 1);

        let mut a17_unguarded_rng = StsRng::new(123);
        assert_eq!(
            target_large_acid_slime_next_intent_from_roll(&[2], 69, &mut a17_unguarded_rng, 17),
            MonsterIntent::Attack { damage: 18 }
        );
        assert_eq!(a17_unguarded_rng.counter(), 0);

        let mut repeated_normal_rng = StsRng::new(123);
        let repeated_normal = target_large_acid_slime_next_intent_from_roll(
            &[2, 2],
            69,
            &mut repeated_normal_rng,
            17,
        );
        assert!(matches!(
            repeated_normal,
            MonsterIntent::AttackAddSlimedToDiscard { .. } | MonsterIntent::ApplyPlayerWeak { .. }
        ));
        assert_eq!(repeated_normal_rng.counter(), 1);
    }

    #[test]
    fn spike_slime_medium_large_helper_uses_source_a17_history_guards() {
        assert_eq!(
            target_medium_or_large_spike_slime_next_intent_from_roll(
                SPIKE_SLIME_M_A7_HP_RANGE.max,
                &[4],
                30,
                16,
            ),
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 }
        );
        assert_eq!(
            target_medium_or_large_spike_slime_next_intent_from_roll(
                SPIKE_SLIME_M_A7_HP_RANGE.max,
                &[4, 4],
                30,
                16,
            ),
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: SPIKE_SLIME_M_SPIT_DAMAGE,
                count: 1
            }
        );
        assert_eq!(
            target_medium_or_large_spike_slime_next_intent_from_roll(
                SPIKE_SLIME_L_A7_HP_RANGE.max,
                &[4],
                30,
                17,
            ),
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: SPIKE_SLIME_L_SPIT_DAMAGE,
                count: 2
            }
        );
        assert_eq!(
            target_medium_or_large_spike_slime_next_intent_from_roll(
                SPIKE_SLIME_L_A7_HP_RANGE.max,
                &[1, 1],
                29,
                17,
            ),
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 3, weak: 0 }
        );
    }

    #[test]
    fn large_spike_slime_split_children_use_current_hp_as_max_hp() {
        let parent_id = MonsterId::new(1);
        let mut parent = monster_state(&SPIKE_SLIME_A0, parent_id);
        parent.hp = 20;
        parent.max_hp = SPIKE_SLIME_L_A7_HP_RANGE.max;
        let mut monsters = vec![parent];
        let mut rng = StsRng::new(0);

        apply_large_spike_slime_split(&mut monsters, parent_id, 2, &mut rng, 17)
            .expect("large Spike Slime split is valid");

        let children = monsters
            .iter()
            .filter(|monster| monster.alive && monster.content_id == SPIKE_SLIME_ID)
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2);
        assert!(children
            .iter()
            .all(|monster| (monster.hp, monster.max_hp) == (20, 20)));
        assert!(children
            .iter()
            .all(|monster| monster.slime_size == Some(SlimeSize::Medium)));
        assert_eq!(rng.counter(), 3);
    }

    #[test]
    fn medium_spike_slime_split_uses_medium_intent_above_hp_threshold() {
        let parent_id = MonsterId::new(1);
        let mut parent = monster_state(&SPIKE_SLIME_A0, parent_id);
        parent.hp = 35;
        parent.max_hp = SPIKE_SLIME_L_A0_HP_RANGE.max;
        let seed = (0..100)
            .find(|seed| {
                let mut rng = StsRng::new(*seed);
                rng.random_int(99) < 30
            })
            .expect("test seed range should include a Spike Slime Spit roll");
        let mut rng = StsRng::new(seed);
        let mut monsters = vec![parent];
        apply_large_spike_slime_split(&mut monsters, parent_id, 2, &mut rng, 0)
            .expect("Spike Slime split is valid");

        let mut expected_rng = StsRng::new(seed);
        let left_roll = expected_rng.random_int(99);
        let right_roll = expected_rng.random_int(99);
        let expected_intents = [left_roll, right_roll].map(|roll| {
            target_medium_or_large_spike_slime_next_intent_from_roll_with_profile(
                false,
                &[],
                roll,
                0,
            )
        });
        let children = monsters
            .iter()
            .filter(|monster| monster.alive && monster.content_id == SPIKE_SLIME_ID)
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2);
        assert_eq!(
            children
                .iter()
                .map(|monster| monster.intent)
                .collect::<Vec<_>>(),
            expected_intents
        );
        assert_eq!(
            children[0].intent,
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: SPIKE_SLIME_M_SPIT_DAMAGE,
                count: 1,
            }
        );
    }

    #[test]
    fn large_acid_slime_split_children_use_current_hp_as_max_hp() {
        let parent_id = MonsterId::new(1);
        let mut parent = monster_state(&ACID_SLIME_A0, parent_id);
        parent.hp = 17;
        parent.max_hp = ACID_SLIME_L_A7_HP_RANGE.max;
        let mut monsters = vec![parent];
        let mut rng = StsRng::new(0);

        apply_large_acid_slime_split(&mut monsters, parent_id, 2, &mut rng, 0)
            .expect("large Acid Slime split is valid");

        let children = monsters
            .iter()
            .filter(|monster| monster.alive && monster.content_id == ACID_SLIME_ID)
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2);
        assert!(children
            .iter()
            .all(|monster| (monster.hp, monster.max_hp) == (17, 17)));
        assert!(children
            .iter()
            .all(|monster| monster.slime_size == Some(SlimeSize::Medium)));
        assert_eq!(rng.counter(), 2);
    }

    #[test]
    fn slime_boss_split_spike_child_keeps_large_profile_after_low_split_hp() {
        let boss_id = MonsterId::new(1);
        let mut boss = monster_state(&SLIME_BOSS_A0, boss_id);
        boss.hp = 54;
        boss.max_hp = 140;
        let mut monsters = vec![boss];
        let seed = (0..100)
            .find(|seed| {
                let mut rng = StsRng::new(*seed);
                rng.random_int(99) < 30
            })
            .expect("test seed range should include a Spike Slime Spit roll");
        let mut rng = StsRng::new(seed);

        apply_slime_boss_split(&mut monsters, boss_id, 2, &mut rng, 0)
            .expect("Slime Boss split is valid");

        let spike = monsters
            .iter()
            .find(|monster| monster.alive && monster.content_id == SPIKE_SLIME_ID)
            .expect("Slime Boss split should spawn a Spike Slime child");
        assert_eq!((spike.hp, spike.max_hp), (54, 54));
        assert_eq!(spike.slime_size, Some(SlimeSize::Large));
        assert_eq!(
            spike.intent,
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: SPIKE_SLIME_L_SPIT_DAMAGE,
                count: 2
            }
        );
    }

    #[test]
    fn bronze_automaton_orb_spawn_consumes_source_hp_and_ai_rolls() {
        let automaton_id = MonsterId::new(7);
        let mut monsters = vec![monster_state(&BRONZE_AUTOMATON_A0, automaton_id)];
        let mut expected_hp_rng = StsRng::new(1234);
        let mut expected_ai_rng = StsRng::new(5678);
        let mut hp_rng = StsRng::new(1234);
        let mut ai_rng = StsRng::new(5678);

        let _left_constructor_hp = BRONZE_ORB_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let left_hp = BRONZE_ORB_A9_HP_RANGE.roll(&mut expected_hp_rng);
        let _right_constructor_hp = BRONZE_ORB_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let right_hp = BRONZE_ORB_A9_HP_RANGE.roll(&mut expected_hp_rng);
        let left_roll = expected_ai_rng.random_int(99);
        let left_intent = target_bronze_orb_next_intent_from_roll(&[], left_roll);
        let right_roll = expected_ai_rng.random_int(99);
        let right_intent = target_bronze_orb_next_intent_from_roll(&[], right_roll);

        apply_bronze_automaton_orb_spawn(
            &mut monsters,
            automaton_id,
            2,
            &mut ai_rng,
            &mut hp_rng,
            9,
        )
        .expect("Bronze Automaton opening summon is valid");

        assert_eq!(hp_rng.counter(), 4);
        assert_eq!(ai_rng.counter(), 2);
        assert_eq!(monsters.len(), 3);
        assert_eq!(monsters[0].content_id, BRONZE_ORB_ID);
        assert_eq!(monsters[1].content_id, BRONZE_AUTOMATON_ID);
        assert_eq!(monsters[2].content_id, BRONZE_ORB_ID);
        assert_eq!((monsters[0].hp, monsters[0].max_hp), (left_hp, left_hp));
        assert_eq!((monsters[2].hp, monsters[2].max_hp), (right_hp, right_hp));
        assert_eq!(monsters[0].intent, left_intent);
        assert_eq!(monsters[2].intent, right_intent);
        assert_eq!(monsters[0].move_history.len(), 1);
        assert_eq!(monsters[2].move_history.len(), 1);
        assert_eq!(monsters[0].powers.minion, 1);
        assert_eq!(monsters[2].powers.minion, 1);
    }

    #[test]
    fn bronze_automaton_spawn_id_overflow_rolls_back_monsters_and_rng() {
        let automaton_id = MonsterId::new(u64::MAX);
        let mut monsters = vec![monster_state(&BRONZE_AUTOMATON_A0, automaton_id)];
        let monsters_before = monsters.clone();
        let mut ai_rng = StsRng::new(5678);
        let mut hp_rng = StsRng::new(1234);
        let ai_rng_before = ai_rng.clone();
        let hp_rng_before = hp_rng.clone();

        let result = apply_bronze_automaton_orb_spawn(
            &mut monsters,
            automaton_id,
            2,
            &mut ai_rng,
            &mut hp_rng,
            0,
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "monster summon instance ID overflows u64"
            ))
        );
        assert_eq!(monsters, monsters_before);
        assert_eq!(ai_rng, ai_rng_before);
        assert_eq!(hp_rng, hp_rng_before);
    }

    #[test]
    fn bronze_automaton_spawn_rejects_invalid_count_without_mutating_state() {
        let automaton_id = MonsterId::new(1);
        let mut monsters = vec![monster_state(&BRONZE_AUTOMATON_A0, automaton_id)];
        let monsters_before = monsters.clone();
        let mut ai_rng = StsRng::new(5678);
        let mut hp_rng = StsRng::new(1234);
        let ai_rng_before = ai_rng.clone();
        let hp_rng_before = hp_rng.clone();

        let result = apply_bronze_automaton_orb_spawn(
            &mut monsters,
            automaton_id,
            0,
            &mut ai_rng,
            &mut hp_rng,
            0,
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "monster summon count is not positive"
            ))
        );
        assert_eq!(monsters, monsters_before);
        assert_eq!(ai_rng, ai_rng_before);
        assert_eq!(hp_rng, hp_rng_before);
    }

    #[test]
    fn bronze_automaton_spawn_rejects_a_duplicate_opening_without_mutating_state() {
        let automaton_id = MonsterId::new(1);
        let mut monsters = vec![
            monster_state(&BRONZE_AUTOMATON_A0, automaton_id),
            monster_state(&BRONZE_ORB_A0, MonsterId::new(2)),
        ];
        let monsters_before = monsters.clone();
        let mut ai_rng = StsRng::new(5678);
        let mut hp_rng = StsRng::new(1234);
        let ai_rng_before = ai_rng.clone();
        let hp_rng_before = hp_rng.clone();

        let result = apply_bronze_automaton_orb_spawn(
            &mut monsters,
            automaton_id,
            2,
            &mut ai_rng,
            &mut hp_rng,
            0,
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "Bronze Automaton cannot repeat its opening Orb summon"
            ))
        );
        assert_eq!(monsters, monsters_before);
        assert_eq!(ai_rng, ai_rng_before);
        assert_eq!(hp_rng, hp_rng_before);
    }

    #[test]
    fn collector_spawn_requires_a_living_collector_without_consuming_rng() {
        let mut monsters = vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))];
        let monsters_before = monsters.clone();
        let mut ai_rng = StsRng::new(5678);
        let mut hp_rng = StsRng::new(1234);
        let ai_rng_before = ai_rng.clone();
        let hp_rng_before = hp_rng.clone();

        let result =
            apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0);

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "Collector summon is missing its living summoner"
            ))
        );
        assert_eq!(monsters, monsters_before);
        assert_eq!(ai_rng, ai_rng_before);
        assert_eq!(hp_rng, hp_rng_before);
    }

    #[test]
    fn manual01_floor33_bronze_automaton_advances_hp_before_orb_spawn() {
        let automaton_id = MonsterId::new(1);
        let mut monsters = vec![monster_state(&BRONZE_AUTOMATON_A0, automaton_id)];
        let mut hp_rng = StsRng::new(1_435_099_163_226 + 33);
        let mut ai_rng = StsRng::new(1_435_099_163_226 + 33);

        MonsterHpRange::new(300, 300).roll(&mut hp_rng);
        apply_bronze_automaton_orb_spawn(
            &mut monsters,
            automaton_id,
            2,
            &mut ai_rng,
            &mut hp_rng,
            0,
        )
        .expect("Bronze Automaton opening summon is valid");

        assert_eq!((monsters[0].hp, monsters[0].max_hp), (53, 53));
        assert_eq!((monsters[2].hp, monsters[2].max_hp), (52, 52));
    }

    #[test]
    fn bronze_automaton_source_cycle_uses_history_a19_boost_and_move_bytes() {
        let opening = target_bronze_automaton_next_intent(0, &[], 0);
        assert_eq!(opening, MonsterIntent::SummonGremlins { count: 2 });
        assert_eq!(target_move_byte(BRONZE_AUTOMATON_ID, opening), Some(4));

        let flail = target_bronze_automaton_next_intent(1, &[4], 0);
        assert_eq!(
            flail,
            MonsterIntent::AttackMultiple {
                damage: BRONZE_AUTOMATON_FLAIL_DAMAGE,
                hits: BRONZE_AUTOMATON_FLAIL_HITS,
            }
        );
        assert_eq!(target_move_byte(BRONZE_AUTOMATON_ID, flail), Some(1));

        let boost = target_bronze_automaton_next_intent(2, &[4, 1], 19);
        assert_eq!(
            boost,
            MonsterIntent::StrengthAndBlock {
                strength: BRONZE_AUTOMATON_A4_BOOST_STRENGTH,
                block: BRONZE_AUTOMATON_A9_BOOST_BLOCK,
            }
        );
        assert_eq!(target_move_byte(BRONZE_AUTOMATON_ID, boost), Some(5));

        let hyper_beam = target_bronze_automaton_next_intent(5, &[4, 1, 5, 1, 5], 4);
        assert_eq!(
            hyper_beam,
            MonsterIntent::Attack {
                damage: BRONZE_AUTOMATON_A4_HYPER_BEAM_DAMAGE
            }
        );
        assert_eq!(target_move_byte(BRONZE_AUTOMATON_ID, hyper_beam), Some(2));
        assert_eq!(
            target_bronze_automaton_next_intent(11, &[5, 1, 5], 0),
            MonsterIntent::Attack {
                damage: BRONZE_AUTOMATON_HYPER_BEAM_DAMAGE
            },
            "the source numTurns reset repeats Hyper Beam every six moves"
        );
        assert_eq!(
            bronze_automaton_intent(11, 0),
            MonsterIntent::Attack {
                damage: BRONZE_AUTOMATON_HYPER_BEAM_DAMAGE
            }
        );

        assert_eq!(
            target_bronze_automaton_next_intent(6, &[4, 1, 5, 1, 5, 2], 18),
            MonsterIntent::Stun
        );
        assert_eq!(
            target_bronze_automaton_next_intent(6, &[4, 1, 5, 1, 5, 2], 19),
            MonsterIntent::StrengthAndBlock {
                strength: BRONZE_AUTOMATON_A4_BOOST_STRENGTH,
                block: BRONZE_AUTOMATON_A9_BOOST_BLOCK,
            }
        );
        assert_eq!(
            target_bronze_automaton_next_intent(7, &[4, 1, 5, 1, 5, 2, 5], 19),
            MonsterIntent::AttackMultiple {
                damage: BRONZE_AUTOMATON_A4_FLAIL_DAMAGE,
                hits: BRONZE_AUTOMATON_FLAIL_HITS,
            }
        );
        assert_eq!(
            target_move_byte(BRONZE_AUTOMATON_ID, MonsterIntent::Stun),
            Some(3)
        );
    }

    #[test]
    fn awakened_one_phase_two_uses_sludge_after_two_tackles() {
        let sludge = target_awakened_one_next_intent_from_roll(&[5, 8, 8], 91, 1, 0);
        assert_eq!(
            sludge,
            MonsterIntent::AttackAddVoidToDraw {
                damage: 18,
                count: 1,
            }
        );
        assert_eq!(target_move_byte(AWAKENED_ONE_ID, sludge), Some(6));

        assert_eq!(
            target_awakened_one_next_intent_from_roll(&[5, 8, 8, 6, 6], 91, 1, 0),
            MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_TACKLE_DAMAGE,
                hits: AWAKENED_ONE_TACKLE_HITS,
            }
        );
    }

    #[test]
    fn awakened_one_phase_two_uses_common_ai_roll_and_move_constraints() {
        assert_eq!(
            target_awakened_one_next_intent_from_roll(&[5], 49, 1, 0),
            MonsterIntent::AttackAddVoidToDraw {
                damage: 18,
                count: 1,
            }
        );
        assert_eq!(
            target_awakened_one_next_intent_from_roll(&[5], 50, 1, 0),
            MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_TACKLE_DAMAGE,
                hits: AWAKENED_ONE_TACKLE_HITS,
            }
        );
        assert_eq!(
            target_awakened_one_next_intent_from_roll(&[6, 6], 0, 1, 0),
            MonsterIntent::AttackMultiple {
                damage: AWAKENED_ONE_TACKLE_DAMAGE,
                hits: AWAKENED_ONE_TACKLE_HITS,
            }
        );
    }

    #[test]
    fn awakened_one_keeps_strength_through_first_death() {
        let mut monster = monster_state(&AWAKENED_ONE_A0, crate::ids::MonsterId::new(1));
        monster.powers.strength = 5;

        assert!(mark_awakened_one_half_dead(&mut monster));
        assert_eq!(monster.powers.strength, 5);
        assert!(awaken_one_after_first_death(&mut monster));
        assert_eq!(monster.powers.strength, 5);
    }

    #[test]
    fn awakened_one_strips_negative_strength_and_debuffs_on_first_death() {
        // Source: StrengthPower is DEBUFF when amount < 0 (e.g. Disarm), so
        // AwakenedOne.damage removes it with other debuffs and Shackled.
        let mut monster = monster_state(&AWAKENED_ONE_A0, crate::ids::MonsterId::new(1));
        monster.powers.strength = -2;
        monster.powers.vulnerable = 3;
        monster.powers.weak = 2;
        monster.powers.slow = 1;
        monster.temp_strength_down = 4;

        assert!(mark_awakened_one_half_dead(&mut monster));
        assert_eq!(monster.powers.strength, 0);
        assert_eq!(monster.powers.vulnerable, 0);
        assert_eq!(monster.powers.weak, 0);
        assert_eq!(monster.powers.slow, 0);
        assert_eq!(monster.temp_strength_down, 0);

        assert!(awaken_one_after_first_death(&mut monster));
        assert_eq!(monster.powers.strength, 0);
        assert!(monster.alive);
        assert_eq!(monster.hp, monster.max_hp);
    }

    #[test]
    fn orb_walker_laser_intent_adds_burn_to_discard_and_draw() {
        let laser = MonsterIntent::AddBurnToDiscardAndDraw {
            damage: ORB_WALKER_LASER_DAMAGE,
            count: 1,
        };

        assert_eq!(orb_walker_intent(0, 0), laser);
        assert_eq!(target_orb_walker_next_intent_from_roll(&[2], 40, 0), laser);
        assert_eq!(target_move_byte(ORB_WALKER_ID, laser), Some(1));
    }

    #[test]
    fn book_of_stabbing_helper_mutates_source_stab_count() {
        let mut stab_count = 1;
        assert_eq!(
            target_book_of_stabbing_next_intent_from_roll_with_stab_count(
                &[],
                &mut stab_count,
                14,
                18,
            ),
            MonsterIntent::Attack {
                damage: BOOK_OF_STABBING_A3_BIG_STAB_DAMAGE
            }
        );
        assert_eq!(stab_count, 2);

        assert_eq!(
            target_book_of_stabbing_next_intent_from_roll_with_stab_count(
                &[2],
                &mut stab_count,
                14,
                18,
            ),
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_A3_STAB_DAMAGE,
                hits: 3,
            }
        );
        assert_eq!(stab_count, 3);
    }

    #[test]
    fn shelled_parasite_first_move_uses_source_boolean_below_a17() {
        let mut rng = StsRng::new(123);
        let first_roll = rng.random_int(99);
        let mut expected_rng = rng.clone();
        let expected_bool = expected_rng.random_bool();
        let intent = target_shelled_parasite_next_intent_from_roll(&[], first_roll, &mut rng, 0);

        assert_eq!(rng.counter(), 2);
        if expected_bool {
            assert_eq!(
                intent,
                MonsterIntent::AttackMultiple {
                    damage: SHELLED_PARASITE_DOUBLE_STRIKE_DAMAGE,
                    hits: SHELLED_PARASITE_DOUBLE_STRIKE_HITS,
                }
            );
        } else {
            assert_eq!(
                intent,
                MonsterIntent::AttackHealSelf {
                    damage: SHELLED_PARASITE_SUCK_DAMAGE
                }
            );
        }

        let mut a17_rng = StsRng::new(123);
        let a17_roll = a17_rng.random_int(99);
        assert_eq!(
            target_shelled_parasite_next_intent_from_roll(&[], a17_roll, &mut a17_rng, 17),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SHELLED_PARASITE_A2_FELL_DAMAGE,
                frail: SHELLED_PARASITE_FELL_FRAIL,
            }
        );
        assert_eq!(a17_rng.counter(), 1);

        let mut followup_rng = StsRng::new(123);
        assert_eq!(
            target_shelled_parasite_next_intent_from_roll(&[3], 0, &mut followup_rng, 0),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SHELLED_PARASITE_FELL_DAMAGE,
                frail: SHELLED_PARASITE_FELL_FRAIL,
            }
        );
        assert_eq!(
            target_move_byte(SHELLED_PARASITE_ID, MonsterIntent::Stun),
            Some(4)
        );
        let shell_break_roll_move = target_shelled_parasite_shell_break_roll_move(0);
        assert_eq!(
            shell_break_roll_move,
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SHELLED_PARASITE_FELL_DAMAGE,
                frail: SHELLED_PARASITE_FELL_FRAIL,
            }
        );
        assert_eq!(
            target_move_byte(SHELLED_PARASITE_ID, shell_break_roll_move),
            Some(1)
        );
    }

    #[test]
    fn snecko_source_helper_uses_fixed_glare_then_roll_history_table() {
        assert_eq!(
            target_snecko_next_intent_from_roll(&[], 99, 17),
            MonsterIntent::ApplyPlayerConfusion
        );
        assert_eq!(
            target_snecko_next_intent_from_roll(&[1], 39, 17),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: SNECKO_A2_TAIL_DAMAGE,
                weak: SNECKO_A17_WEAK,
                vulnerable: SNECKO_VULNERABLE,
            }
        );
        assert_eq!(
            target_snecko_next_intent_from_roll(&[1], 40, 2),
            MonsterIntent::Attack {
                damage: SNECKO_A2_BITE_DAMAGE,
            }
        );
        assert_eq!(
            target_snecko_next_intent_from_roll(&[2, 2], 99, 0),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: SNECKO_TAIL_DAMAGE,
                vulnerable: SNECKO_VULNERABLE,
            }
        );
    }

    #[test]
    fn chosen_source_helper_uses_hex_opening_and_history_guards() {
        assert_eq!(
            target_chosen_next_intent_from_roll(&[], 99, 17),
            MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX }
        );
        assert_eq!(
            target_chosen_next_intent_from_roll(&[], 99, 16),
            MonsterIntent::AttackMultiple {
                damage: CHOSEN_A2_POKE_DAMAGE,
                hits: CHOSEN_POKE_HITS,
            }
        );
        assert_eq!(
            target_chosen_next_intent_from_roll(&[5], 99, 16),
            MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX }
        );
        assert_eq!(
            target_chosen_next_intent_from_roll(&[4], 49, 17),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: CHOSEN_A2_DEBILITATE_DAMAGE,
                vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
            }
        );
        assert_eq!(
            target_chosen_next_intent_from_roll(&[4], 50, 17),
            MonsterIntent::ApplyPlayerWeakStrengthSelf {
                weak: CHOSEN_DRAIN_WEAK,
                strength: CHOSEN_DRAIN_STRENGTH,
            }
        );
        assert_eq!(
            target_chosen_next_intent_from_roll(&[4, 2], 39, 0),
            MonsterIntent::Attack {
                damage: CHOSEN_ZAP_DAMAGE,
            }
        );
        assert_eq!(
            target_chosen_next_intent_from_roll(&[4, 3], 40, 0),
            MonsterIntent::AttackMultiple {
                damage: CHOSEN_POKE_DAMAGE,
                hits: CHOSEN_POKE_HITS,
            }
        );
    }

    #[test]
    fn snake_plant_source_helper_uses_a17_spores_guard_and_thresholds() {
        assert_eq!(
            target_snake_plant_next_intent_from_roll(&[], 64, 17),
            MonsterIntent::AttackMultiple {
                damage: SNAKE_PLANT_A2_CHOMPY_DAMAGE,
                hits: SNAKE_PLANT_CHOMPY_HITS,
            }
        );
        assert_eq!(
            target_snake_plant_next_intent_from_roll(&[1, 1], 64, 17),
            MonsterIntent::ApplyPlayerFrailAndWeak {
                frail: SNAKE_PLANT_SPORES_DEBUFF,
                weak: SNAKE_PLANT_SPORES_DEBUFF,
            }
        );
        assert_eq!(
            target_snake_plant_next_intent_from_roll(&[1, 2], 65, 17),
            MonsterIntent::AttackMultiple {
                damage: SNAKE_PLANT_A2_CHOMPY_DAMAGE,
                hits: SNAKE_PLANT_CHOMPY_HITS,
            }
        );
        assert_eq!(
            target_snake_plant_next_intent_from_roll(&[2, 1], 65, 16),
            MonsterIntent::ApplyPlayerFrailAndWeak {
                frail: SNAKE_PLANT_SPORES_DEBUFF,
                weak: SNAKE_PLANT_SPORES_DEBUFF,
            }
        );
        assert_eq!(
            target_snake_plant_next_intent_from_roll(&[1, 2], 65, 16),
            MonsterIntent::AttackMultiple {
                damage: SNAKE_PLANT_A2_CHOMPY_DAMAGE,
                hits: SNAKE_PLANT_CHOMPY_HITS,
            }
        );
    }

    #[test]
    fn snake_plant_starts_with_malleable_and_spores_apply_frail_and_weak() {
        let mut source_monster = monster_state(&SNAKE_PLANT_A0, MonsterId::new(1));
        source_monster.intent = MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: SNAKE_PLANT_SPORES_DEBUFF,
            weak: SNAKE_PLANT_SPORES_DEBUFF,
        };
        let state = crate::CombatState::initial_fixture();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        let mut player = state.player;
        let player_before = player.clone();
        let mut piles = state.piles;
        let mut card_random_rng = StsRng::new(0);

        let damage = apply_monster_intent_with_card_rng(
            &mut source_monster,
            &mut player,
            &mut piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[],
            &mut card_random_rng,
        );

        assert_eq!(damage, Ok(0));
        assert_eq!(source_monster.powers.malleable, SNAKE_PLANT_MALLEABLE);
        assert_eq!(source_monster.powers.malleable_base, SNAKE_PLANT_MALLEABLE);
        assert_eq!(player.powers.frail, SNAKE_PLANT_SPORES_DEBUFF);
        assert_eq!(player.powers.weak, SNAKE_PLANT_SPORES_DEBUFF);
    }

    #[test]
    fn turnip_blocks_frail_applied_by_monster_intents() {
        let mut source_monster = monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1));
        source_monster.intent = MonsterIntent::AttackApplyPlayerFrail {
            damage: SHELLED_PARASITE_FELL_DAMAGE,
            frail: SHELLED_PARASITE_FELL_FRAIL,
        };
        let state = crate::CombatState::initial_fixture();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        let mut player = state.player;
        let player_before = player.clone();
        let mut piles = state.piles;
        let mut card_random_rng = StsRng::new(0);

        let damage = apply_monster_intent_with_card_rng(
            &mut source_monster,
            &mut player,
            &mut piles,
            allocated_card_id_through,
            0,
            &player_before,
            &[crate::Relic::Turnip],
            &mut card_random_rng,
        );

        assert_eq!(damage, Ok(SHELLED_PARASITE_FELL_DAMAGE));
        assert_eq!(player.powers.frail, 0);
    }

    #[test]
    fn writhing_mass_starts_with_source_malleable() {
        let monster = monster_state(&WRITHING_MASS_A0, MonsterId::new(1));
        assert_eq!(monster.powers.malleable, WRITHING_MASS_MALLEABLE);
        assert_eq!(monster.powers.malleable_base, WRITHING_MASS_MALLEABLE);

        let spawns = target_beyond_encounter_spawn_for_key(1, 39, "Writhing Mass", 0, false)
            .expect("Writhing Mass encounter is generated");
        assert_eq!(
            spawns[0].powers,
            vec![TargetSpawnPower {
                id: "Malleable",
                amount: WRITHING_MASS_MALLEABLE,
            }]
        );
    }

    #[test]
    fn writhing_mass_source_move_table_uses_exact_thresholds_and_a2_values() {
        let mut rng = StsRng::new(1);
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(true, &[], false, 32, &mut rng, 0),
            MonsterIntent::AttackMultiple { damage: 7, hits: 3 }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(true, &[], false, 33, &mut rng, 0),
            MonsterIntent::AttackAndBlock {
                damage: 15,
                block: 15,
            }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(true, &[], false, 66, &mut rng, 2),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: 12,
                weak: 2,
                vulnerable: 2,
            }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(false, &[], false, 9, &mut rng, 2),
            MonsterIntent::Attack { damage: 38 }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(false, &[], false, 10, &mut rng, 0),
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(false, &[], false, 20, &mut rng, 0),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: 10,
                weak: 2,
                vulnerable: 2,
            }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(false, &[], false, 40, &mut rng, 0),
            MonsterIntent::AttackMultiple { damage: 7, hits: 3 }
        );
        assert_eq!(
            target_writhing_mass_next_intent_from_roll(false, &[], false, 70, &mut rng, 2),
            MonsterIntent::AttackAndBlock {
                damage: 16,
                block: 16,
            }
        );
    }

    #[test]
    fn writhing_mass_move_bytes_match_target_next_move_ids() {
        assert_eq!(
            target_move_byte(WRITHING_MASS_ID, writhing_mass_big_hit_intent(0)),
            Some(0)
        );
        assert_eq!(
            target_move_byte(WRITHING_MASS_ID, writhing_mass_multi_hit_intent(0)),
            Some(1)
        );
        assert_eq!(
            target_move_byte(WRITHING_MASS_ID, writhing_mass_attack_block_intent(0)),
            Some(2)
        );
        assert_eq!(
            target_move_byte(WRITHING_MASS_ID, writhing_mass_attack_debuff_intent(0)),
            Some(3)
        );
        assert_eq!(
            target_move_byte(
                WRITHING_MASS_ID,
                MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 }
            ),
            Some(4)
        );
    }

    #[test]
    fn centurion_source_helper_uses_ally_count_for_protect_or_fury() {
        assert_eq!(
            target_centurion_next_intent_from_roll(&[], 65, 2, 17),
            MonsterIntent::Block {
                block: CENTURION_A17_BLOCK,
            }
        );
        assert_eq!(
            target_centurion_next_intent_from_roll(&[], 65, 1, 2),
            MonsterIntent::AttackMultiple {
                damage: CENTURION_A2_FURY_DAMAGE,
                hits: CENTURION_FURY_HITS,
            }
        );
        assert_eq!(
            target_centurion_next_intent_from_roll(&[1, 1], 0, 2, 0),
            MonsterIntent::Block {
                block: CENTURION_BLOCK,
            }
        );
        assert_eq!(
            target_centurion_next_intent_from_roll(&[1, 1], 0, 1, 0),
            MonsterIntent::AttackMultiple {
                damage: CENTURION_FURY_DAMAGE,
                hits: CENTURION_FURY_HITS,
            }
        );
        assert_eq!(
            target_centurion_next_intent_from_roll(&[2, 2], 99, 2, 0),
            MonsterIntent::Attack {
                damage: CENTURION_SLASH_DAMAGE,
            }
        );
    }

    #[test]
    fn healer_source_helper_uses_missing_hp_and_a17_history_guards() {
        assert_eq!(
            target_healer_next_intent_from_roll(&[], 0, 16, 0),
            MonsterIntent::HealAllMonsters {
                amount: HEALER_HEAL,
            }
        );
        assert_eq!(
            target_healer_next_intent_from_roll(&[], 0, 20, 17),
            MonsterIntent::StrengthAllMonsters {
                amount: HEALER_A17_STRENGTH,
            }
        );
        assert_eq!(
            target_healer_next_intent_from_roll(&[], 0, 21, 17),
            MonsterIntent::HealAllMonsters {
                amount: HEALER_A17_HEAL,
            }
        );
        assert_eq!(
            target_healer_next_intent_from_roll(&[1], 40, 0, 17),
            MonsterIntent::StrengthAllMonsters {
                amount: HEALER_A17_STRENGTH,
            }
        );
        assert_eq!(
            target_healer_next_intent_from_roll(&[1], 40, 0, 16),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: HEALER_A2_ATTACK_DAMAGE,
                frail: HEALER_FRAIL,
            }
        );
        assert_eq!(
            target_healer_next_intent_from_roll(&[3, 3], 0, 0, 0),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: HEALER_ATTACK_DAMAGE,
                frail: HEALER_FRAIL,
            }
        );
    }

    #[test]
    fn healer_missing_hp_and_heal_caps_use_rolled_monster_max_hp() {
        let mut centurion = monster_state(&CENTURION_A0, MonsterId::new(1));
        centurion.max_hp = 80;
        centurion.hp = 70;
        let mut healer = monster_state(&HEALER_A0, MonsterId::new(2));
        healer.max_hp = 53;
        healer.hp = 47;
        let mut monsters = vec![centurion, healer];

        let missing_hp = living_monster_missing_hp(&monsters);
        assert_eq!(missing_hp, 16);
        assert_eq!(
            target_healer_next_intent_from_roll(&[], 0, missing_hp, 0),
            MonsterIntent::HealAllMonsters {
                amount: HEALER_HEAL,
            }
        );

        apply_heal_all_monsters(&mut monsters, HEALER_HEAL).expect("monster healing is valid");
        assert_eq!(monsters[0].hp, 80);
        assert_eq!(monsters[1].hp, 53);
    }

    #[test]
    fn monster_group_healing_clamps_at_the_target_hp_limit() {
        let mut monster = monster_state(&HEALER_A0, MonsterId::new(1));
        monster.hp = i32::MAX;
        monster.max_hp = i32::MAX;

        apply_heal_all_monsters(std::slice::from_mut(&mut monster), 1)
            .expect("positive healing clamps at max HP");

        assert_eq!(monster.hp, i32::MAX);
    }

    #[test]
    fn monster_group_strength_overflow_is_transactional() {
        let mut first = monster_state(&CENTURION_A0, MonsterId::new(1));
        first.powers.strength = 4;
        let mut second = monster_state(&HEALER_A0, MonsterId::new(2));
        second.powers.strength = i32::MAX;
        let mut monsters = vec![first, second];
        let before = monsters.clone();

        assert_eq!(
            apply_strength_all_monsters(&mut monsters, 1),
            Err(SimError::InvalidState("monster group arithmetic overflow"))
        );
        assert_eq!(monsters, before);
    }

    #[test]
    fn gremlin_encourage_overflow_is_transactional() {
        let leader_id = MonsterId::new(1);
        let mut leader = monster_state(&GREMLIN_LEADER_A0, leader_id);
        leader.powers.strength = 4;
        let mut minion = monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(2));
        minion.block = i32::MAX;
        let mut monsters = vec![leader, minion];
        let before = monsters.clone();

        assert_eq!(
            apply_gremlin_leader_encourage(&mut monsters, leader_id, 1, 1),
            Err(SimError::InvalidState("monster group arithmetic overflow"))
        );
        assert_eq!(monsters, before);
    }

    #[test]
    fn fungi_beast_source_helper_pins_grow_bonus_and_setup_surface() {
        assert_eq!(
            target_fungi_beast_next_intent_from_roll(&[], 59, 17),
            MonsterIntent::Attack {
                damage: FUNGI_BEAST_BITE_DAMAGE,
            }
        );
        assert_eq!(
            target_fungi_beast_next_intent_from_roll(&[1, 1], 59, 17),
            MonsterIntent::StrengthSelf {
                amount: FUNGI_BEAST_A2_GROW_STRENGTH + FUNGI_BEAST_A17_GROW_BONUS,
            }
        );
        assert_eq!(
            target_fungi_beast_next_intent_from_roll(&[2], 60, 0),
            MonsterIntent::Attack {
                damage: FUNGI_BEAST_BITE_DAMAGE,
            }
        );

        let fungi = monster_state(&FUNGI_BEAST_A0, MonsterId::new(1));
        assert_eq!(fungi.powers.spore_cloud, FUNGI_BEAST_SPORE_CLOUD);
        assert_eq!(fungi.powers.artifact, 0);
    }

    #[test]
    fn reptomancer_spawn_uses_source_group_and_hp_order() {
        let mut expected_hp_rng = StsRng::new(222);
        let left_dagger_hp = DAGGER_HP_RANGE.roll(&mut expected_hp_rng);
        let _reptomancer_constructor_hp = REPTOMANCER_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let reptomancer_hp = REPTOMANCER_A8_HP_RANGE.roll(&mut expected_hp_rng);
        let right_dagger_hp = DAGGER_HP_RANGE.roll(&mut expected_hp_rng);

        let spawns = target_reptomancer_encounter_spawn(200, 22, 8, false);

        assert_eq!(spawns.len(), 3);
        assert_eq!(spawns[0].name, "Dagger");
        assert_eq!(
            (spawns[0].current_hp, spawns[0].max_hp),
            (left_dagger_hp, left_dagger_hp)
        );
        assert_eq!(
            spawns[0].powers,
            vec![TargetSpawnPower {
                id: "Minion",
                amount: 1
            }]
        );
        assert_eq!(spawns[1].name, "Reptomancer");
        assert_eq!(
            (spawns[1].current_hp, spawns[1].max_hp),
            (reptomancer_hp, reptomancer_hp)
        );
        assert_eq!(spawns[2].name, "Dagger");
        assert_eq!(
            (spawns[2].current_hp, spawns[2].max_hp),
            (right_dagger_hp, right_dagger_hp)
        );
    }

    #[test]
    fn reptomancer_dagger_spawn_consumes_source_hp_and_opening_ai_rolls() {
        let reptomancer_id = MonsterId::new(2);
        let mut left = monster_state(&DAGGER_A0, MonsterId::new(1));
        left.gremlin_leader_slot = Some(1);
        left.powers.minion = 1;
        let reptomancer = monster_state(&REPTOMANCER_A0, reptomancer_id);
        let mut right = monster_state(&DAGGER_A0, MonsterId::new(3));
        right.gremlin_leader_slot = Some(0);
        right.powers.minion = 1;
        let mut monsters = vec![left, reptomancer, right];
        let mut hp_rng = StsRng::new(4321);
        let mut ai_rng = StsRng::new(8765);
        let mut expected_hp_rng = StsRng::new(4321);
        let slot_two_hp = DAGGER_HP_RANGE.roll(&mut expected_hp_rng);
        let slot_three_hp = DAGGER_HP_RANGE.roll(&mut expected_hp_rng);

        apply_reptomancer_dagger_spawn(&mut monsters, reptomancer_id, 2, &mut ai_rng, &mut hp_rng)
            .expect("Reptomancer summon is valid");

        assert_eq!(hp_rng.counter(), 2);
        assert_eq!(ai_rng.counter(), 2);
        assert_eq!(monsters.len(), 5);
        let slot_two = monsters
            .iter()
            .find(|monster| {
                monster.content_id == DAGGER_ID && monster.gremlin_leader_slot == Some(2)
            })
            .expect("slot 2 dagger spawned");
        let slot_three = monsters
            .iter()
            .find(|monster| {
                monster.content_id == DAGGER_ID && monster.gremlin_leader_slot == Some(3)
            })
            .expect("slot 3 dagger spawned");
        assert_eq!((slot_two.hp, slot_two.max_hp), (slot_two_hp, slot_two_hp));
        assert_eq!(
            (slot_three.hp, slot_three.max_hp),
            (slot_three_hp, slot_three_hp)
        );
        assert_eq!(slot_two.powers.minion, 1);
        assert_eq!(slot_three.powers.minion, 1);
        assert_eq!(slot_two.move_history, vec![1]);
        assert_eq!(slot_three.move_history, vec![1]);
    }

    #[test]
    fn gremlin_leader_rally_uses_latest_slot_occupant_for_availability() {
        let mut old_slot_zero = monster_state(&GREMLIN_WIZARD_A0, MonsterId::new(1));
        old_slot_zero.gremlin_leader_slot = Some(0);
        let mut slot_one = monster_state(&GREMLIN_THIEF_A0, MonsterId::new(2));
        slot_one.gremlin_leader_slot = Some(1);
        let leader = monster_state(&GREMLIN_LEADER_A0, MonsterId::new(3));
        let mut monsters = vec![old_slot_zero, slot_one, leader];

        assert_eq!(
            gremlin_leader_first_available_slot_excluding(&monsters, &[]),
            Some(2)
        );

        let mut newer_slot_zero = monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(4));
        newer_slot_zero.gremlin_leader_slot = Some(0);
        newer_slot_zero.hp = 0;
        newer_slot_zero.alive = false;
        monsters.insert(2, newer_slot_zero);

        assert_eq!(
            gremlin_leader_first_available_slot_excluding(&monsters, &[]),
            Some(0)
        );
        assert_eq!(
            gremlin_leader_first_available_slot_excluding(&monsters, &[0]),
            Some(2)
        );
    }

    #[test]
    fn gremlin_leader_rally_uses_source_slot_generation_and_draw_x_order() {
        let mut old_slot_two = monster_state(&GREMLIN_TSUNDERE_A0, MonsterId::new(5));
        old_slot_two.gremlin_leader_slot = Some(2);
        old_slot_two.hp = 0;
        old_slot_two.alive = false;

        let mut live_slot_zero = monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(4));
        live_slot_zero.gremlin_leader_slot = Some(0);

        let mut old_slot_zero = monster_state(&GREMLIN_THIEF_A0, MonsterId::new(1));
        old_slot_zero.gremlin_leader_slot = Some(0);
        old_slot_zero.hp = 0;
        old_slot_zero.alive = false;

        let mut old_slot_one = monster_state(&GREMLIN_WIZARD_A0, MonsterId::new(2));
        old_slot_one.gremlin_leader_slot = Some(1);
        old_slot_one.hp = 0;
        old_slot_one.alive = false;

        let leader = monster_state(&GREMLIN_LEADER_A0, MonsterId::new(3));
        let monsters = vec![
            old_slot_two,
            live_slot_zero,
            old_slot_zero,
            old_slot_one,
            leader,
        ];

        assert_eq!(
            gremlin_leader_first_available_slot_excluding(&monsters, &[]),
            Some(1)
        );
        assert_eq!(
            gremlin_leader_summon_insert_index(&monsters, 1, GREMLIN_WARRIOR_ID),
            4
        );
        assert_eq!(
            gremlin_leader_summon_insert_index(&monsters, 2, GREMLIN_TSUNDERE_ID),
            0
        );
    }

    #[test]
    fn gremlin_leader_rally_preserves_rolled_max_hp() {
        let leader = monster_state(&GREMLIN_LEADER_A0, MonsterId::new(1));
        let mut monsters = vec![leader];
        let mut ai_rng = StsRng::new(8765);
        let mut hp_rng = StsRng::new(4321);
        let mut expected_ai_rng = StsRng::new(8765);
        let mut expected_hp_rng = StsRng::new(4321);
        let expected = (0..2)
            .map(|slot| {
                let name = target_random_gremlin_name(&mut expected_ai_rng);
                let content_id = content_id_from_game_monster_id(name)
                    .expect("target random gremlin name is registered");
                let max_hp = target_city_monster_hp_range(name, 0)
                    .expect("rally gremlin has a target HP range")
                    .roll(&mut expected_hp_rng);
                (slot, content_id, max_hp)
            })
            .collect::<Vec<_>>();

        apply_gremlin_leader_rally_target(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0)
            .expect("Gremlin Leader rally is valid");

        for (slot, content_id, max_hp) in expected {
            let summoned = monsters
                .iter()
                .find(|monster| monster.gremlin_leader_slot == Some(slot))
                .expect("rally gremlin spawned in the planned slot");
            assert_eq!(summoned.content_id, content_id);
            assert_eq!((summoned.hp, summoned.max_hp), (max_hp, max_hp));
        }
    }

    #[test]
    fn gremlin_leader_slot_zero_summon_uses_spatial_order_not_corpse_count() {
        let mut first_slot_zero = monster_state(&GREMLIN_WIZARD_A0, MonsterId::new(1));
        first_slot_zero.gremlin_leader_slot = Some(0);
        first_slot_zero.hp = 0;
        first_slot_zero.alive = false;
        let mut second_slot_zero = monster_state(&GREMLIN_WIZARD_A0, MonsterId::new(2));
        second_slot_zero.gremlin_leader_slot = Some(0);
        second_slot_zero.hp = 0;
        second_slot_zero.alive = false;
        let mut third_slot_zero = monster_state(&GREMLIN_THIEF_A0, MonsterId::new(3));
        third_slot_zero.gremlin_leader_slot = Some(0);
        third_slot_zero.hp = 0;
        third_slot_zero.alive = false;
        let mut slot_one = monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(4));
        slot_one.gremlin_leader_slot = Some(1);
        slot_one.hp = 0;
        slot_one.alive = false;
        let leader = monster_state(&GREMLIN_LEADER_A0, MonsterId::new(5));
        let monsters = vec![
            first_slot_zero,
            second_slot_zero,
            third_slot_zero,
            slot_one,
            leader,
        ];

        assert_eq!(
            gremlin_leader_summon_insert_index(&monsters, 0, GREMLIN_WIZARD_ID),
            0
        );
    }

    #[test]
    fn taskmaster_city_spawn_consumes_constructor_and_set_hp_rolls() {
        let mut expected_hp_rng = StsRng::new(321);
        let _constructor_hp = TASKMASTER_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let taskmaster_hp = TASKMASTER_A8_HP_RANGE.roll(&mut expected_hp_rng);
        let mut hp_rng = StsRng::new(321);
        let mut misc_rng = StsRng::new(654);

        let spawn = target_city_member_spawn("Taskmaster", &mut hp_rng, &mut misc_rng, 8, false)
            .expect("Taskmaster city member spawn exists");

        assert_eq!(hp_rng.counter(), 2);
        assert_eq!(misc_rng.counter(), 0);
        assert_eq!(spawn.name, "Taskmaster");
        assert_eq!(
            (spawn.current_hp, spawn.max_hp),
            (taskmaster_hp, taskmaster_hp)
        );
    }

    #[test]
    fn collector_torch_head_spawn_uses_source_hp_and_ai_rolls() {
        let mut monsters = vec![monster_state(&THE_COLLECTOR_A0, MonsterId::new(1))];
        let mut hp_rng = StsRng::new(2468);
        let mut ai_rng = StsRng::new(1357);
        let mut expected_hp_rng = StsRng::new(2468);
        let _first_constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let first_hp = TORCH_HEAD_A9_HP_RANGE.roll(&mut expected_hp_rng);
        let _second_constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let second_hp = TORCH_HEAD_A9_HP_RANGE.roll(&mut expected_hp_rng);

        apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 9)
            .expect("Collector summon is valid");

        assert_eq!(hp_rng.counter(), 4);
        assert_eq!(ai_rng.counter(), 2);
        assert_eq!(monsters.len(), 3);
        assert_eq!(monsters[0].content_id, TORCH_HEAD_ID);
        assert_eq!(monsters[1].content_id, TORCH_HEAD_ID);
        assert_eq!(monsters[2].content_id, THE_COLLECTOR_ID);
        assert_eq!((monsters[0].hp, monsters[0].max_hp), (second_hp, second_hp));
        assert_eq!((monsters[1].hp, monsters[1].max_hp), (first_hp, first_hp));
        assert_eq!(monsters[0].intent, MonsterIntent::Attack { damage: 7 });
        assert_eq!(monsters[1].intent, MonsterIntent::Attack { damage: 7 });
        assert_eq!(monsters[0].move_history, vec![1]);
        assert_eq!(monsters[1].move_history, vec![1]);
    }

    #[test]
    fn collector_a0_torch_heads_consume_constructor_and_set_hp_rolls() {
        let mut monsters = vec![monster_state(&THE_COLLECTOR_A0, MonsterId::new(1))];
        let mut hp_rng = StsRng::new(2468);
        let mut ai_rng = StsRng::new(1357);
        let mut expected_hp_rng = StsRng::new(2468);
        let _first_constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let first_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let _second_constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let second_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);

        apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0)
            .expect("Collector summon is valid");

        assert_eq!(hp_rng.counter(), 4);
        assert_eq!(ai_rng.counter(), 2);
        assert_eq!((monsters[0].hp, monsters[0].max_hp), (second_hp, second_hp));
        assert_eq!((monsters[1].hp, monsters[1].max_hp), (first_hp, first_hp));
    }

    #[test]
    fn collector_replacement_only_fills_missing_torch_head_slot() {
        let mut monsters = vec![monster_state(&THE_COLLECTOR_A0, MonsterId::new(1))];
        let mut hp_rng = StsRng::new(2468);
        let mut ai_rng = StsRng::new(1357);
        let mut expected_hp_rng = StsRng::new(2468);
        for _ in 0..2 {
            let _constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
            let _initial_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        }
        let _constructor_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);
        let replacement_hp = TORCH_HEAD_A0_HP_RANGE.roll(&mut expected_hp_rng);

        apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0)
            .expect("Collector initial summon is valid");
        let dead = monsters
            .iter_mut()
            .find(|monster| monster.gremlin_leader_slot == Some(2))
            .expect("second Torch Head exists");
        dead.hp = 0;
        dead.alive = false;

        apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0)
            .expect("Collector replacement summon is valid");

        assert_eq!(hp_rng.counter(), 6);
        assert_eq!(ai_rng.counter(), 3);
        assert_eq!(monsters.len(), 4);
        assert_eq!(monsters[0].gremlin_leader_slot, Some(1));
        assert!(monsters[0].alive);
        assert_eq!(monsters[1].gremlin_leader_slot, Some(2));
        assert!(monsters[1].alive);
        assert_eq!(
            (monsters[1].hp, monsters[1].max_hp),
            (replacement_hp, replacement_hp)
        );
        assert_eq!(monsters[2].gremlin_leader_slot, Some(2));
        assert!(!monsters[2].alive);
        assert_eq!(monsters[3].content_id, THE_COLLECTOR_ID);
    }

    #[test]
    fn collector_fixed_hp_roll_aligns_initial_and_replacement_torch_heads() {
        let mut monsters = vec![monster_state(&THE_COLLECTOR_A0, MonsterId::new(1))];
        let mut hp_rng = StsRng::new(772_776_727_775 + 33);
        let mut ai_rng = StsRng::new(1357);

        let collector_hp = target_monster_hp_range_for_content_id(THE_COLLECTOR_ID, 0)
            .expect("Collector has a fixed setHp roll")
            .roll(&mut hp_rng);
        assert_eq!(collector_hp, 282);
        apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0)
            .expect("Collector initial summon is valid");
        assert_eq!(
            monsters
                .iter()
                .filter(|monster| monster.content_id == TORCH_HEAD_ID && monster.alive)
                .map(|monster| monster.max_hp)
                .collect::<Vec<_>>(),
            vec![39, 39]
        );

        for monster in &mut monsters {
            if monster.content_id == TORCH_HEAD_ID {
                monster.alive = false;
                monster.hp = 0;
            }
        }
        apply_collector_spawn_torch_heads(&mut monsters, 2, &mut ai_rng, &mut hp_rng, 0)
            .expect("Collector replacement summon is valid");

        assert_eq!(hp_rng.counter(), 9);
        assert_eq!(
            monsters
                .iter()
                .filter(|monster| monster.content_id == TORCH_HEAD_ID && monster.alive)
                .map(|monster| monster.max_hp)
                .collect::<Vec<_>>(),
            vec![38, 39]
        );
    }

    #[test]
    fn transient_spawn_uses_ascension_scaled_attack_without_rng_hp() {
        let spawns = target_beyond_encounter_spawn_for_key(999, 44, "Transient", 4, false)
            .expect("Transient beyond encounter spawn exists");

        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].name, "Transient");
        assert_eq!(
            (spawns[0].current_hp, spawns[0].max_hp),
            (TRANSIENT_HP, TRANSIENT_HP)
        );
        assert_eq!(
            spawns[0].intent,
            TargetSpawnIntent::Attack {
                damage: TRANSIENT_A4_ATTACK_DAMAGE
            }
        );
        assert_eq!(
            spawns[0].rolled_attack_damage,
            Some(TRANSIENT_A4_ATTACK_DAMAGE)
        );
        assert_eq!(
            transient_attack_damage(3, 4).expect("Transient damage is in range"),
            TRANSIENT_A4_ATTACK_DAMAGE + 30
        );
        assert_eq!(
            transient_attack_damage(u32::MAX, 4),
            Err(SimError::InvalidState(
                "transient attack damage exceeds supported range"
            ))
        );
    }

    #[test]
    fn generic_intent_preparation_rejects_unknown_and_approximate_content() {
        let unhandled_definition = MonsterDefinition {
            content_id: ContentId::new(u64::MAX),
            ..FIXED_SIMPLE_MONSTER
        };
        assert_eq!(
            prepare_monster_intent_for(&unhandled_definition, 0, None),
            MonsterIntent::PendingAiRoll,
            "an unhandled definition must not inherit the fixed fixture attack"
        );

        let mut unknown = monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1));
        unknown.content_id = ContentId::new(u64::MAX);
        assert_eq!(
            prepare_monster_intent_for_ascension(&unknown, 0),
            Err(SimError::UnknownContent(ContentId::new(u64::MAX)))
        );
        assert_eq!(
            source_backed_gremlin_leader_minion_intent(ContentId::new(u64::MAX), 0, 0),
            None,
            "unknown minion identity must not become a plausible Stun intent"
        );

        for (definition, content_id) in [
            (&AWAKENED_ONE_A0, AWAKENED_ONE_ID),
            (&CORRUPT_HEART_A0, CORRUPT_HEART_ID),
            (&SPIRE_SHIELD_A0, SPIRE_SHIELD_ID),
            (&SPIRE_SPEAR_A0, SPIRE_SPEAR_ID),
        ] {
            let monster = monster_state(definition, MonsterId::new(1));
            assert_eq!(
                prepare_monster_intent_for_ascension(&monster, 0),
                Err(SimError::UnsupportedMechanic(content_id))
            );
        }

        let time_eater = monster_state(&TIME_EATER_A0, MonsterId::new(1));
        assert_eq!(
            prepare_monster_intent_for_ascension(&time_eater, 0),
            Ok(MonsterIntent::AttackAndBlock {
                damage: 0,
                block: TIME_EATER_RIPPLE_BLOCK,
            })
        );

        let mut transient = monster_state(&TRANSIENT_A0, MonsterId::new(1));
        transient.moves_executed = u32::MAX;
        assert_eq!(
            prepare_monster_intent_for_ascension(&transient, 4),
            Err(SimError::InvalidState(
                "transient attack damage exceeds supported range"
            ))
        );
    }

    #[test]
    fn donu_deca_pair_uses_source_opening_state_and_move_bytes() {
        let baseline = donu_deca_boss_monsters_for_ascension(0);
        assert_eq!(baseline[0].powers.artifact, 2);
        assert_eq!(baseline[1].powers.artifact, 2);

        let monsters = donu_deca_boss_monsters_for_ascension(19);

        assert_eq!(monsters.len(), 2);
        assert_eq!(monsters[0].content_id, DECA_ID);
        assert_eq!(monsters[1].content_id, DONU_ID);
        assert_eq!(monsters[0].hp, 265);
        assert_eq!(monsters[1].hp, 265);
        assert_eq!(monsters[0].powers.artifact, 3);
        assert_eq!(monsters[1].powers.artifact, 3);

        let deca_opening = prepare_monster_intent_for_ascension(&monsters[0], 19)
            .expect("Donu and Deca have source-backed intent cycles");
        let donu_opening = prepare_monster_intent_for_ascension(&monsters[1], 19)
            .expect("Donu and Deca have source-backed intent cycles");
        assert_eq!(
            deca_opening,
            MonsterIntent::AttackMultipleAddDazedToDiscard {
                damage: DECA_A4_BEAM_DAMAGE,
                hits: DECA_BEAM_HITS,
                count: 2,
            }
        );
        assert_eq!(
            donu_opening,
            MonsterIntent::StrengthAllMonsters { amount: 3 }
        );
        assert_eq!(target_move_byte(DECA_ID, deca_opening), Some(0));
        assert_eq!(target_move_byte(DONU_ID, donu_opening), Some(2));

        let mut donu_after_circle = monsters[1].clone();
        donu_after_circle.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&donu_after_circle, 19)
                .expect("Donu has a source-backed intent cycle"),
            MonsterIntent::AttackMultiple {
                damage: DONU_A4_BEAM_DAMAGE,
                hits: DONU_BEAM_HITS,
            }
        );
    }

    #[test]
    fn repulsor_source_helper_uses_roll_threshold_and_no_repeat_attack_guard() {
        assert_eq!(
            target_repulsor_next_intent_from_roll(&[], 19, 2),
            MonsterIntent::Attack {
                damage: REPULSOR_A2_ATTACK_DAMAGE
            }
        );
        assert_eq!(
            target_repulsor_next_intent_from_roll(&[], 20, 2),
            MonsterIntent::AddDazedToDraw {
                count: REPULSOR_DAZES
            }
        );
        assert_eq!(
            target_repulsor_next_intent_from_roll(&[2], 0, 2),
            MonsterIntent::AddDazedToDraw {
                count: REPULSOR_DAZES
            }
        );
        assert_eq!(
            target_move_byte(
                REPULSOR_ID,
                MonsterIntent::AddDazedToDraw {
                    count: REPULSOR_DAZES
                }
            ),
            Some(1)
        );
    }

    #[test]
    fn exploder_source_helper_ignores_roll_value_and_uses_two_attack_countdown() {
        assert_eq!(
            target_exploder_next_intent_from_roll(0, 2),
            MonsterIntent::Attack {
                damage: EXPLODER_A2_ATTACK_DAMAGE
            }
        );
        assert_eq!(
            target_exploder_next_intent_from_roll(1, 2),
            MonsterIntent::Attack {
                damage: EXPLODER_A2_ATTACK_DAMAGE
            }
        );
        assert_eq!(
            target_exploder_next_intent_from_roll(2, 2),
            MonsterIntent::Stun
        );
        assert_eq!(target_move_byte(EXPLODER_ID, MonsterIntent::Stun), Some(2));
    }

    #[test]
    fn spiker_uses_source_thorns_state_and_forces_attack_after_six_buffs() {
        let mut source_monster =
            monster_state_for_ascension(&SPIKER_A0, crate::MonsterId::new(1), 17);
        assert_eq!(
            source_monster.powers.spikes,
            SPIKER_A2_THORNS + SPIKER_A17_THORNS_BONUS
        );

        assert_eq!(
            target_spiker_next_intent_from_roll(&[], 0, 99, 17),
            MonsterIntent::StrengthAndBlock {
                strength: 0,
                block: 0
            }
        );
        assert_eq!(
            target_spiker_next_intent_from_roll(&[], 6, 99, 17),
            MonsterIntent::Attack {
                damage: SPIKER_A2_ATTACK_DAMAGE
            }
        );

        let state = crate::CombatState::initial_fixture();
        let allocated_card_id_through = state.max_authoritative_card_instance_id();
        let mut player = state.player;
        let player_before = player.clone();
        let mut piles = state.piles;
        let mut card_random_rng = StsRng::new(0);
        source_monster.intent = MonsterIntent::StrengthAndBlock {
            strength: 0,
            block: 0,
        };

        let damage = apply_monster_intent_with_card_rng(
            &mut source_monster,
            &mut player,
            &mut piles,
            allocated_card_id_through,
            17,
            &player_before,
            &[],
            &mut card_random_rng,
        );

        assert_eq!(damage, Ok(0));
        assert_eq!(source_monster.powers.spiker_thorns_buffs, 1);
        assert_eq!(
            source_monster.powers.spikes,
            SPIKER_A2_THORNS + SPIKER_A17_THORNS_BONUS + SPIKER_THORNS_BUFF
        );
    }
}
