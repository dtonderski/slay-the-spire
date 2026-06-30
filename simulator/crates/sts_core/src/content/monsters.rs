use crate::{
    card::{CardInstance, CardRarity},
    combat::piles::add_cards_to_discard,
    combat::turn_powers::monster_attack_damage,
    combat::{CardPiles, MonsterIntent, MonsterState},
    content::ascension::AscensionConfig,
    content::cards::{card_type_and_rarity, BURN_ID, DAZED_ID, SLIMED_ID},
    ids::{ContentId, MonsterId},
    power::MonsterPowers,
    rng::StsRng,
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

const RED_LOUSE_BITE_DAMAGE: i32 = 6;
const LOUSE_CURL_STRENGTH: i32 = 3;

const GREEN_LOUSE_BITE_DAMAGE: i32 = 6;
const GREEN_LOUSE_SPIKES: i32 = 3;

const SPIKE_SLIME_LICK_WEAK: i32 = 1;
const SPIKE_SLIME_S_SPIT_DAMAGE: i32 = 5;
const SPIKE_SLIME_M_SPIT_DAMAGE: i32 = 8;
const SPIKE_SLIME_L_SPIT_DAMAGE: i32 = 16;

const ACID_SLIME_S_TACKLE_DAMAGE: i32 = 3;
const ACID_SLIME_ATTACK_DAMAGE: i32 = 7;
const ACID_SLIME_M_NORMAL_TACKLE_DAMAGE: i32 = 10;
const ACID_SLIME_WEAK: i32 = 1;

const LAGAVULIN_SLEEP_TURNS: u32 = 3;
const LAGAVULIN_SIPHON_STRENGTH: i32 = 1;
const LAGAVULIN_SIPHON_DEXTERITY: i32 = 1;
const LAGAVULIN_ATTACK_DAMAGE: i32 = 18;

const SENTRY_BEAM_DAZED: i32 = 2;
const SENTRY_ATTACK_DAMAGE: i32 = 9;
const SENTRY_A3_ATTACK_DAMAGE: i32 = 10;

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
const HEXAGHOST_INFERNO_BURNS: i32 = 3;
const HEXAGHOST_INFERNO_DAMAGE: i32 = 2;

const SLIME_BOSS_SLAM_DAMAGE: i32 = 35;
const SLIME_BOSS_SPLIT_HP_THRESHOLD: i32 = 70;

const GUARDIAN_MODE_SHIFT_START: i32 = 30;
const GUARDIAN_MODE_SHIFT_RESET: i32 = 40;
const GUARDIAN_DEFENSIVE_SEQUENCE_TURNS: u32 = 3;
const GUARDIAN_DEFENSIVE_BLOCK: i32 = 20;
const GUARDIAN_DEFENSIVE_SPIKES: i32 = 3;
pub const GUARDIAN_CHARGE_BLOCK: i32 = 9;
const GUARDIAN_FIERCE_BASH_DAMAGE: i32 = 32;
const GUARDIAN_DEFENSIVE_ATTACK_DAMAGE: i32 = 9;
const GUARDIAN_DEFENSIVE_COMBO_DAMAGE: i32 = 8;
const GUARDIAN_WHIRLWIND_DAMAGE: i32 = 5;
const GUARDIAN_WHIRLWIND_HITS: i32 = 4;
const GUARDIAN_VENT_DEBUFF: i32 = 2;

const GREMLIN_NOB_RUSH_DAMAGE: i32 = 14;
const GREMLIN_NOB_SKULL_BASH_DAMAGE: i32 = 6;

const JAW_WORM_CHOMP_DAMAGE: i32 = 11;
const JAW_WORM_THRASH_DAMAGE: i32 = 7;
const JAW_WORM_THRASH_BLOCK: i32 = 5;
const JAW_WORM_BELLOW_STRENGTH: i32 = 3;
const JAW_WORM_BELLOW_BLOCK: i32 = 6;

pub const LOOTER_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(44, 48);
pub const LOOTER_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(46, 50);
const LOOTER_SWIPE_DAMAGE: i32 = 10;
const LOOTER_A2_SWIPE_DAMAGE: i32 = 11;
const LOOTER_THEFT: i32 = 15;
const LOOTER_A17_THEFT: i32 = 20;
const MUGGER_SWIPE_DAMAGE: i32 = 10;
const MUGGER_A2_SWIPE_DAMAGE: i32 = 11;
const MUGGER_BIG_SWIPE_DAMAGE: i32 = 16;
const MUGGER_A2_BIG_SWIPE_DAMAGE: i32 = 18;
const MUGGER_THEFT: i32 = 15;
const MUGGER_A17_THEFT: i32 = 20;
const MUGGER_ESCAPE_BLOCK: i32 = 11;
const MUGGER_A17_ESCAPE_BLOCK: i32 = 17;
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
const BRONZE_AUTOMATON_FLAIL_HITS: i32 = 2;
const BRONZE_AUTOMATON_HYPER_BEAM_DAMAGE: i32 = 45;
const BRONZE_AUTOMATON_BOOST_BLOCK: i32 = 9;
const BRONZE_ORB_BEAM_DAMAGE: i32 = 8;
const ORB_WALKER_LASER_DAMAGE: i32 = 15;
const ORB_WALKER_CLAW_DAMAGE: i32 = 10;
const ORB_WALKER_STRENGTH_UP: i32 = 3;
const DARKLING_CHOMP_DAMAGE: i32 = 8;
const DARKLING_BLOCK: i32 = 12;

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
    pub intent: &'static str,
    pub powers: Vec<TargetSpawnPower>,
    pub rolled_attack_damage: Option<i32>,
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
pub const SENTRY_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(38, 42);
pub const SENTRY_A8_HP_RANGE: MonsterHpRange = MonsterHpRange::new(39, 45);

pub const BYRD_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(25, 31);
pub const BYRD_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(26, 33);
pub const CHOSEN_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(95, 99);
pub const CHOSEN_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(98, 103);
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
    enrage_weak_on_skill: 2,
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

/// Act 1 Lagavulin at ascension 0: 109 HP, sleeps 3 turns, then attacks twice and siphons.
pub const LAGAVULIN_A0: MonsterDefinition = MonsterDefinition {
    content_id: LAGAVULIN_ID,
    name: "Lagavulin",
    hp: 109,
    attack_damage: 0,
    ritual_amount: 0,
    enrage_weak_on_skill: 0,
    starting_spikes: 0,
    starting_sleep_turns: LAGAVULIN_SLEEP_TURNS,
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
    let mut rng = StsRng::new(seed + i64::from(floor_num));
    target_cultist_hp_range(ascension).roll(&mut rng)
}

#[must_use]
pub fn target_jaw_worm_hp_roll(seed: i64, floor_num: u32, ascension: u8) -> i32 {
    let mut rng = StsRng::new(seed + i64::from(floor_num));
    target_jaw_worm_hp_range(ascension).roll(&mut rng)
}

#[must_use]
pub fn target_looter_hp_roll(seed: i64, floor_num: u32, ascension: u8) -> i32 {
    let mut rng = StsRng::new(seed + i64::from(floor_num));
    target_looter_hp_range(ascension).roll(&mut rng)
}

#[must_use]
pub fn target_small_slimes_variant(seed: i64, floor_num: u32) -> SmallSlimesVariant {
    let mut misc_rng = StsRng::new(seed + i64::from(floor_num));
    if misc_rng.random_bool() {
        SmallSlimesVariant::SpikeSmallAcidMedium
    } else {
        SmallSlimesVariant::AcidSmallSpikeMedium
    }
}

#[must_use]
pub fn target_large_slime_variant(seed: i64, floor_num: u32) -> LargeSlimeVariant {
    let mut misc_rng = StsRng::new(seed + i64::from(floor_num));
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
            let mut hp_rng = StsRng::new(seed + i64::from(floor_num));
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
            let mut hp_rng = StsRng::new(seed + i64::from(floor_num));
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
    let mut hp_rng = StsRng::new(seed + i64::from(floor_num));
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
    let mut misc_rng = StsRng::new(seed + i64::from(floor_num));
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
    let kinds = target_two_louse_kinds(seed, floor_num);
    let mut hp_rng = StsRng::new(seed + i64::from(floor_num));

    let mut spawns = kinds
        .into_iter()
        .map(|kind| {
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
    Some(target_encounter_spawn_for_key(
        seed,
        floor_num,
        &encounter_key,
        ascension,
        neow_lament,
    ))
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
        crate::map::TargetMapAct::Beyond => return None,
    };
    match act {
        crate::map::TargetMapAct::Exordium => Some(target_encounter_spawn_for_key(
            seed,
            floor_num,
            &encounter_key,
            ascension,
            neow_lament,
        )),
        crate::map::TargetMapAct::City => target_city_encounter_spawn_for_key(
            seed,
            floor_num,
            &encounter_key,
            ascension,
            neow_lament,
        ),
        crate::map::TargetMapAct::Beyond => None,
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
    match encounter_key {
        "3 Darklings" => target_darkling_encounter_spawn(seed, floor_num, ascension, neow_lament),
        "Orb Walker" => Some(vec![target_combat_entry_spawn(
            "Orb Walker",
            ORB_WALKER_A0.hp,
            neow_lament,
            vec![TargetSpawnPower {
                id: "Generic Strength Up Power",
                amount: ORB_WALKER_STRENGTH_UP,
            }],
        )]),
        _ => None,
    }
}

fn target_darkling_encounter_spawn(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut hp_rng = StsRng::new(seed + i64::from(floor_num));
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

#[must_use]
pub fn target_city_encounter_spawn_for_key(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
) -> Option<Vec<TargetEncounterSpawn>> {
    let mut misc_rng = StsRng::new(seed + i64::from(floor_num));
    target_city_encounter_spawn_for_key_with_misc_rng(
        seed,
        floor_num,
        encounter_key,
        ascension,
        neow_lament,
        Some(&mut misc_rng),
    )
}

#[must_use]
pub fn target_city_encounter_spawn_for_key_with_misc_rng(
    seed: i64,
    floor_num: u32,
    encounter_key: &str,
    ascension: u8,
    neow_lament: bool,
    mut misc_rng: Option<&mut StsRng>,
) -> Option<Vec<TargetEncounterSpawn>> {
    let group = target_city_encounter_group_for_key(encounter_key)?;
    let mut hp_rng = StsRng::new(seed + i64::from(floor_num));
    group
        .members
        .iter()
        .map(|member| {
            target_city_member_spawn(
                member.monster_name,
                &mut hp_rng,
                misc_rng.as_deref_mut(),
                ascension,
                neow_lament,
            )
        })
        .collect()
}

fn target_city_member_spawn(
    monster_name: &str,
    hp_rng: &mut StsRng,
    misc_rng: Option<&mut StsRng>,
    ascension: u8,
    neow_lament: bool,
) -> Option<TargetEncounterSpawn> {
    let monster_name = if monster_name == "random gremlin" {
        target_random_gremlin_name(misc_rng?)
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
    let max_hp = hp_range.roll(hp_rng);
    let mut spawn = target_combat_entry_spawn(name, max_hp, neow_lament, Vec::new());

    match name {
        "Looter" => spawn.powers.push(TargetSpawnPower {
            id: "Thievery",
            amount: looter_theft(ascension),
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
) -> Vec<TargetEncounterSpawn> {
    match encounter_key {
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
        "Small Slimes" => target_small_slimes_spawn_states(seed, floor_num, ascension, neow_lament)
            .unwrap_or_default(),
        "Large Slime" => {
            let roll = target_large_slime_hp_roll(seed, floor_num, ascension);
            let mut spawn = target_combat_entry_spawn(roll.name, roll.hp, neow_lament, Vec::new());
            match roll.name {
                "Acid Slime (L)" => {
                    spawn.intent = "AttackAddSlimedToDiscard";
                    spawn.rolled_attack_damage = Some(if ascension >= 2 { 12 } else { 11 });
                }
                "Spike Slime (L)" => {
                    spawn.intent = "AttackAddSlimedToDiscard";
                    spawn.rolled_attack_damage = Some(if ascension >= 2 { 18 } else { 16 });
                }
                _ => {}
            }
            vec![spawn]
        }
        "2 Louse" => target_two_louse_spawn_states(seed, floor_num, ascension, neow_lament),
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
        "GremlinNob" => vec![target_combat_entry_spawn(
            "GremlinNob",
            GREMLIN_NOB_A0.hp,
            neow_lament,
            Vec::new(),
        )],
        "Lagavulin" => {
            let mut spawn =
                target_combat_entry_spawn("Lagavulin", LAGAVULIN_A0.hp, neow_lament, Vec::new());
            spawn.block = 8;
            vec![spawn]
        }
        "3 Sentries" => target_three_sentries_spawn_states(seed, floor_num, ascension, neow_lament),
        _ => Vec::new(),
    }
}

fn target_three_sentries_spawn_states(
    seed: i64,
    floor_num: u32,
    ascension: u8,
    neow_lament: bool,
) -> Vec<TargetEncounterSpawn> {
    let mut hp_rng = StsRng::new(seed + i64::from(floor_num));
    (0..3)
        .map(|index| {
            let max_hp = target_sentry_hp_range(ascension).roll(&mut hp_rng);
            let mut spawn = target_combat_entry_spawn("Sentry", max_hp, neow_lament, Vec::new());
            if index % 2 == 1 {
                spawn.intent = "Attack";
                spawn.rolled_attack_damage = Some(target_sentry_attack_damage(ascension));
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
        intent: "DEBUG",
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
pub fn content_id_from_game_monster_id(game_id: &str) -> ContentId {
    match game_id {
        "Cultist" => CULTIST_ID,
        "JawWorm" | "Jaw Worm" => JAW_WORM_ID,
        "GremlinNob" => GREMLIN_NOB_ID,
        "Lagavulin" => LAGAVULIN_ID,
        "Sentry" => SENTRY_ID,
        "Hexaghost" => HEXAGHOST_ID,
        "SlimeBoss" => SLIME_BOSS_ID,
        "TheGuardian" => GUARDIAN_ID,
        "Looter" => LOOTER_ID,
        "SphericGuardian" => SPHERIC_GUARDIAN_ID,
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
        "BronzeAutomaton" | "Bronze Automaton" => BRONZE_AUTOMATON_ID,
        "BronzeOrb" | "Bronze Orb" | "Orb" => BRONZE_ORB_ID,
        "Orb Walker" | "OrbWalker" => ORB_WALKER_ID,
        "Darkling" => DARKLING_ID,
        "SpikeSlime_S" | "SpikeSlime_M" | "SpikeSlime_L" | "Spike Slime (S)"
        | "Spike Slime (M)" | "Spike Slime (L)" => SPIKE_SLIME_ID,
        "AcidSlime_S" | "AcidSlime_M" | "AcidSlime_L" | "Acid Slime (S)" | "Acid Slime (M)"
        | "Acid Slime (L)" => ACID_SLIME_ID,
        "FuzzyLouseDefensive" | "LouseDefensive" => GREEN_LOUSE_ID,
        "FuzzyLouseNormal" | "LouseNormal" => RED_LOUSE_ID,
        _ => CULTIST_ID,
    }
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
        ORB_WALKER_ID => Some(&ORB_WALKER_A0),
        DARKLING_ID => Some(&DARKLING_A0),
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
    MonsterState {
        id,
        hp: if definition.content_id == SPHERIC_GUARDIAN_ID {
            definition.hp
        } else {
            config.scaled_enemy_hp(definition.hp)
        },
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
            spikes: definition.starting_spikes,
            artifact: match definition.content_id {
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
            malleable: if definition.content_id == SNAKE_PLANT_ID {
                SNAKE_PLANT_MALLEABLE
            } else {
                0
            },
            malleable_base: if definition.content_id == SNAKE_PLANT_ID {
                SNAKE_PLANT_MALLEABLE
            } else {
                0
            },
            spore_cloud: if definition.content_id == FUNGI_BEAST_ID {
                FUNGI_BEAST_SPORE_CLOUD
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
        moves_executed: 0,
        sleep_turns_remaining: definition.starting_sleep_turns,
        has_siphoned: false,
        split_triggered: false,
        defensive_turns_remaining: definition.starting_defensive_turns,
        mode_shift: if definition.content_id == GUARDIAN_ID {
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
        intent: prepare_monster_intent_for_monster(
            definition,
            0,
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
pub fn prepare_monster_intent(monster: &MonsterState) -> MonsterIntent {
    prepare_monster_intent_for_ascension(monster, 0)
}

#[must_use]
pub fn prepare_monster_intent_for_ascension(
    monster: &MonsterState,
    ascension: u8,
) -> MonsterIntent {
    let definition = get_monster_definition(monster.content_id).unwrap_or(&FIXED_SIMPLE_MONSTER);
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
    if let Some(damage) = monster.rolled_attack_damage {
        if let MonsterIntent::Attack {
            damage: ref mut attack,
        } = intent
        {
            *attack = damage;
        }
    }
    if monster.content_id == ACID_SLIME_ID
        && monster.hp > ACID_SLIME_S_A7_HP_RANGE.max
        && matches!(intent, MonsterIntent::Attack { .. })
    {
        let MonsterIntent::Attack { damage } = intent else {
            unreachable!("matches! above guarantees Attack intent")
        };
        let count = if monster.hp > ACID_SLIME_M_A7_HP_RANGE.max {
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
        let damage = if monster.hp > SPIKE_SLIME_M_A7_HP_RANGE.max {
            SPIKE_SLIME_L_SPIT_DAMAGE
        } else {
            SPIKE_SLIME_M_SPIT_DAMAGE
        };
        let count = if monster.hp > SPIKE_SLIME_M_A7_HP_RANGE.max {
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
        intent = MonsterIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 };
    }
    intent
}

#[must_use]
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
    if definition.content_id == LAGAVULIN_ID {
        return lagavulin_intent(sleep_turns_remaining, moves_executed);
    }
    if definition.content_id == GUARDIAN_ID {
        return guardian_intent(in_defensive_mode, defensive_turns_remaining, moves_executed);
    }
    if definition.content_id == MUGGER_ID {
        return mugger_intent(moves_executed, ascension);
    }
    if definition.content_id == SPHERIC_GUARDIAN_ID {
        return spheric_guardian_intent(moves_executed, ascension);
    }
    if definition.content_id == BOOK_OF_STABBING_ID {
        return book_of_stabbing_intent(moves_executed, ascension);
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
    if definition.content_id == BRONZE_AUTOMATON_ID {
        return bronze_automaton_intent(moves_executed);
    }
    if definition.content_id == BRONZE_ORB_ID {
        return bronze_orb_intent(moves_executed);
    }
    if definition.content_id == ORB_WALKER_ID {
        return orb_walker_intent(moves_executed);
    }
    if definition.content_id == DARKLING_ID {
        return darkling_intent(moves_executed, rolled_attack_damage);
    }
    if is_gremlin_leader_minion_content_id(definition.content_id) {
        return gremlin_leader_minion_intent(definition.content_id, moves_executed, ascension);
    }
    let _ = mode_shift;
    prepare_monster_intent_for(definition, moves_executed, rolled_attack_damage)
}

#[must_use]
pub fn prepare_monster_intent_for(
    definition: &MonsterDefinition,
    moves_executed: u32,
    rolled_attack_damage: Option<i32>,
) -> MonsterIntent {
    match definition.content_id {
        CULTIST_ID if moves_executed == 0 => MonsterIntent::Ritual {
            amount: definition.ritual_amount,
        },
        JAW_WORM_ID => jaw_worm_intent(moves_executed),
        GREMLIN_NOB_ID => gremlin_nob_intent(moves_executed),
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
        BRONZE_AUTOMATON_ID => bronze_automaton_intent(moves_executed),
        BRONZE_ORB_ID => bronze_orb_intent(moves_executed),
        ORB_WALKER_ID => orb_walker_intent(moves_executed),
        DARKLING_ID => darkling_intent(moves_executed, rolled_attack_damage),
        FUNGI_BEAST_ID => fungi_beast_intent(moves_executed, 0),
        SLAVER_BLUE_ID => slaver_blue_intent(moves_executed, 0),
        SLAVER_RED_ID => slaver_red_intent(moves_executed, 0),
        _ if is_gremlin_leader_minion_content_id(definition.content_id) => {
            gremlin_leader_minion_intent(definition.content_id, moves_executed, 0)
        }
        SPIKE_SLIME_ID => spike_slime_s_intent(moves_executed),
        ACID_SLIME_ID => acid_slime_intent(moves_executed),
        SENTRY_ID => sentry_intent(moves_executed),
        SPHERIC_GUARDIAN_ID => spheric_guardian_intent(moves_executed, 0),
        HEXAGHOST_ID => hexaghost_intent(moves_executed),
        SLIME_BOSS_ID => slime_boss_intent(),
        _ => MonsterIntent::Attack {
            damage: definition.attack_damage,
        },
    }
}

fn ascension_from_damage_roll(_rolled_attack_damage: Option<i32>) -> u8 {
    0
}

/// Deterministic Red Louse move cycle: Curl → Bite, keyed on `moves_executed`.
#[must_use]
fn red_louse_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::StrengthAndBlock {
            strength: LOUSE_CURL_STRENGTH,
            block: 0,
        },
        _ => MonsterIntent::Attack {
            damage: rolled_attack_damage.unwrap_or(RED_LOUSE_BITE_DAMAGE),
        },
    }
}

#[must_use]
fn green_louse_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::StrengthAndBlock {
            strength: LOUSE_CURL_STRENGTH,
            block: 0,
        },
        _ => MonsterIntent::Attack {
            damage: rolled_attack_damage.unwrap_or(GREEN_LOUSE_BITE_DAMAGE),
        },
    }
}

#[must_use]
pub fn target_darkling_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    monster_index: usize,
    rolled_attack_damage: Option<i32>,
    ascension: u8,
) -> MonsterIntent {
    target_darkling_next_intent_from_roll_inner(
        move_history,
        roll,
        monster_index,
        rolled_attack_damage,
        ascension,
        None,
    )
}

pub fn target_darkling_next_intent_from_roll_with_rng(
    move_history: &[u8],
    roll: i32,
    monster_index: usize,
    rolled_attack_damage: Option<i32>,
    ascension: u8,
    rng: &mut StsRng,
) -> MonsterIntent {
    target_darkling_next_intent_from_roll_inner(
        move_history,
        roll,
        monster_index,
        rolled_attack_damage,
        ascension,
        Some(rng),
    )
}

fn target_darkling_next_intent_from_roll_inner(
    move_history: &[u8],
    roll: i32,
    monster_index: usize,
    rolled_attack_damage: Option<i32>,
    ascension: u8,
    mut rng: Option<&mut StsRng>,
) -> MonsterIntent {
    let attack_damage = rolled_attack_damage.unwrap_or(DARKLING_CHOMP_DAMAGE);
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
        if !move_history.ends_with(&[1]) && monster_index % 2 == 0 {
            MonsterIntent::AttackMultiple {
                damage: DARKLING_CHOMP_DAMAGE,
                hits: 2,
            }
        } else {
            let reroll = rng
                .as_deref_mut()
                .map_or(40, |rng| rng.random_int_range(40, 99));
            target_darkling_next_intent_from_roll_inner(
                move_history,
                reroll,
                monster_index,
                rolled_attack_damage,
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
        let reroll = rng
            .as_deref_mut()
            .map_or(0, |rng| rng.random_int_range(0, 99));
        target_darkling_next_intent_from_roll_inner(
            move_history,
            reroll,
            monster_index,
            rolled_attack_damage,
            ascension,
            rng,
        )
    }
}

fn darkling_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    if moves_executed == 0 {
        MonsterIntent::Attack {
            damage: rolled_attack_damage.unwrap_or(DARKLING_CHOMP_DAMAGE),
        }
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
fn looter_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 | 1 => MonsterIntent::AttackStealGold {
            damage: looter_swipe_damage(ascension),
            amount: looter_theft(ascension),
        },
        _ => MonsterIntent::Block { block: 6 },
    }
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
        MUGGER_A17_ESCAPE_BLOCK
    } else {
        MUGGER_ESCAPE_BLOCK
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
pub fn living_monster_missing_hp(monsters: &[MonsterState], ascension: u8) -> i32 {
    monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| monster_max_hp_for_current_definition(monster, ascension) - monster.hp)
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

#[must_use]
fn shelled_parasite_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    if moves_executed == 0 && ascension >= 17 {
        return MonsterIntent::AttackApplyPlayerFrail {
            damage: shelled_parasite_fell_damage(ascension),
            frail: SHELLED_PARASITE_FELL_FRAIL,
        };
    }

    match moves_executed {
        0 => MonsterIntent::AttackMultiple {
            damage: shelled_parasite_double_strike_damage(ascension),
            hits: SHELLED_PARASITE_DOUBLE_STRIKE_HITS,
        },
        1 => MonsterIntent::AttackHealSelf {
            damage: shelled_parasite_suck_damage(ascension),
        },
        _ => MonsterIntent::AttackApplyPlayerFrail {
            damage: shelled_parasite_fell_damage(ascension),
            frail: SHELLED_PARASITE_FELL_FRAIL,
        },
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
    if roll < 20 {
        if !last_move(move_history, 1) {
            return MonsterIntent::AttackApplyPlayerFrail {
                damage: shelled_parasite_fell_damage(ascension),
                frail: SHELLED_PARASITE_FELL_FRAIL,
            };
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

#[must_use]
pub fn target_move_byte(content_id: ContentId, intent: MonsterIntent) -> Option<u8> {
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
    if content_id == SNAKE_PLANT_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::ApplyPlayerFrailAndWeak { .. } => Some(2),
            _ => None,
        };
    }
    if content_id == SHELLED_PARASITE_ID {
        return match intent {
            MonsterIntent::AttackApplyPlayerFrail { .. } => Some(1),
            MonsterIntent::AttackMultiple { .. } => Some(2),
            MonsterIntent::AttackHealSelf { .. } => Some(3),
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
    if content_id == GREMLIN_LEADER_ID {
        return match intent {
            MonsterIntent::SummonGremlins { .. } => Some(2),
            MonsterIntent::EncourageGremlins { .. } => Some(3),
            MonsterIntent::AttackMultiple { .. } => Some(4),
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
    if content_id == BRONZE_ORB_ID {
        return match intent {
            MonsterIntent::Attack { .. } => Some(1),
            MonsterIntent::Block { .. } => Some(2),
            MonsterIntent::SiphonPlayer { .. } => Some(3),
            _ => None,
        };
    }
    if content_id == DARKLING_ID {
        return match intent {
            MonsterIntent::AttackMultiple { .. } => Some(1),
            MonsterIntent::Block { .. } | MonsterIntent::StrengthAndBlock { .. } => Some(2),
            MonsterIntent::Attack { .. } => Some(3),
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
            MonsterIntent::Attack { .. } => Some(1),
            _ => None,
        };
    }
    None
}

pub fn record_target_move(monster: &mut MonsterState) {
    if let Some(move_byte) = target_move_byte(monster.content_id, monster.intent) {
        monster.move_history.push(move_byte);
    }
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
fn taskmaster_intent() -> MonsterIntent {
    MonsterIntent::AttackAddWoundsToDiscard {
        damage: TASKMASTER_SCOURING_WHIP_DAMAGE,
        count: TASKMASTER_WOUNDS,
    }
}

fn taskmaster_wound_count(ascension: u8) -> i32 {
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
            return target_gremlin_leader_next_intent_from_roll(
                move_history,
                50,
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
        return target_gremlin_leader_next_intent_from_roll(
            move_history,
            80,
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
fn gremlin_leader_minion_intent(
    content_id: ContentId,
    moves_executed: u32,
    ascension: u8,
) -> MonsterIntent {
    match content_id {
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
        GREMLIN_WIZARD_ID if moves_executed >= 2 => MonsterIntent::Attack {
            damage: gremlin_wizard_damage(ascension),
        },
        GREMLIN_WIZARD_ID => MonsterIntent::Block { block: 0 },
        _ => MonsterIntent::Stun,
    }
}

#[must_use]
fn bronze_automaton_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::SummonGremlins { count: 2 },
        1 | 3 => MonsterIntent::AttackMultiple {
            damage: BRONZE_AUTOMATON_FLAIL_DAMAGE,
            hits: BRONZE_AUTOMATON_FLAIL_HITS,
        },
        2 | 4 => MonsterIntent::StrengthAndBlock {
            strength: ORB_WALKER_STRENGTH_UP,
            block: BRONZE_AUTOMATON_BOOST_BLOCK,
        },
        5 => MonsterIntent::Attack {
            damage: BRONZE_AUTOMATON_HYPER_BEAM_DAMAGE,
        },
        6 => MonsterIntent::Stun,
        _ => MonsterIntent::AttackMultiple {
            damage: BRONZE_AUTOMATON_FLAIL_DAMAGE,
            hits: BRONZE_AUTOMATON_FLAIL_HITS,
        },
    }
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
        _ => MonsterIntent::Block { block: 0 },
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
        return MonsterIntent::Block { block: 0 };
    }
    if !last_two_moves(move_history, 1) {
        return MonsterIntent::Attack {
            damage: BRONZE_ORB_BEAM_DAMAGE,
        };
    }
    MonsterIntent::Block { block: 0 }
}

#[must_use]
fn orb_walker_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Attack {
            damage: ORB_WALKER_LASER_DAMAGE,
        },
        _ => MonsterIntent::AddBurnToDiscardAndDraw {
            count: 1,
            damage: ORB_WALKER_CLAW_DAMAGE,
        },
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
pub fn monster_max_hp_for_current_definition(monster: &MonsterState, ascension: u8) -> i32 {
    let definition = get_monster_definition(monster.content_id).unwrap_or(&FIXED_SIMPLE_MONSTER);
    if definition.content_id == SPHERIC_GUARDIAN_ID {
        definition.hp
    } else {
        AscensionConfig::new(ascension).scaled_enemy_hp(definition.hp)
    }
}

pub fn apply_heal_all_monsters(monsters: &mut [MonsterState], ascension: u8, amount: i32) {
    for monster in monsters.iter_mut().filter(|monster| monster.alive) {
        let max_hp = monster_max_hp_for_current_definition(monster, ascension);
        monster.hp = (monster.hp + amount).min(max_hp);
    }
}

pub fn apply_strength_all_monsters(monsters: &mut [MonsterState], amount: i32) {
    for monster in monsters.iter_mut().filter(|monster| monster.alive) {
        monster.powers.strength += amount;
    }
}

pub fn apply_gremlin_leader_encourage(
    monsters: &mut [MonsterState],
    leader_id: MonsterId,
    strength: i32,
    block: i32,
) {
    for monster in monsters.iter_mut().filter(|monster| monster.alive) {
        monster.powers.strength += strength;
        if monster.id != leader_id {
            monster.block += block;
        }
    }
}

pub fn apply_gremlin_leader_rally_representative(monsters: &mut Vec<MonsterState>, count: i32) {
    if count <= 0 {
        return;
    }

    let mut next_id = monsters
        .iter()
        .map(|monster| monster.id.get())
        .max()
        .unwrap_or(0)
        + 1;
    let mut summoned = 0;
    for _ in 0..3 {
        if summoned >= count {
            break;
        }
        if gremlin_leader_live_minion_count(monsters) >= 3 {
            break;
        }
        monsters.insert(
            gremlin_leader_representative_summon_index(monsters),
            monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(next_id)),
        );
        next_id += 1;
        summoned += 1;
    }
}

pub fn apply_gremlin_leader_rally_target(
    monsters: &mut Vec<MonsterState>,
    count: i32,
    ai_rng: &mut crate::rng::StsRng,
    hp_rng: &mut crate::rng::StsRng,
    ascension: u8,
) {
    if count <= 0 {
        return;
    }

    if monsters
        .iter()
        .any(|monster| !monster.alive && is_gremlin_leader_minion_content_id(monster.content_id))
    {
        ai_rng.random_int(99);
    }

    for _ in 0..count {
        if gremlin_leader_live_minion_count(monsters) >= 3 {
            break;
        }
        let Some(slot) = gremlin_leader_first_available_slot(monsters) else {
            break;
        };
        let name = target_random_gremlin_name(ai_rng);
        let Some(definition) = get_monster_definition(content_id_from_game_monster_id(name)) else {
            continue;
        };
        let max_hp = target_city_monster_hp_range(name, ascension)
            .map(|range| range.roll(hp_rng))
            .unwrap_or_else(|| {
                monster_state_for_ascension(definition, MonsterId::new(0), ascension).hp
            });
        let next_id = monsters
            .iter()
            .map(|monster| monster.id.get())
            .max()
            .unwrap_or(0)
            + 1;
        let mut monster =
            monster_state_for_ascension(definition, MonsterId::new(next_id), ascension);
        monster.hp = max_hp;
        monster.powers.minion = 1;
        if monster.content_id == GREMLIN_WARRIOR_ID {
            monster.powers.anger = gremlin_warrior_anger(ascension);
        }
        monster.gremlin_leader_slot = Some(slot as u8);
        let roll = ai_rng.random_int(99);
        monster.intent = gremlin_leader_minion_intent(monster.content_id, 0, ascension);
        let _ = roll;
        record_target_move(&mut monster);
        monsters.insert(gremlin_leader_summon_insert_index(monsters, slot), monster);
    }
}

pub fn apply_bronze_automaton_orb_spawn(monsters: &mut Vec<MonsterState>, automaton_id: MonsterId) {
    let Some(automaton_index) = monsters.iter().position(|monster| {
        monster.id == automaton_id && monster.content_id == BRONZE_AUTOMATON_ID
    }) else {
        return;
    };
    if monsters
        .iter()
        .any(|monster| monster.alive && monster.content_id == BRONZE_ORB_ID)
    {
        return;
    }

    let next_id = monsters
        .iter()
        .map(|monster| monster.id.get())
        .max()
        .unwrap_or(0)
        + 1;
    let mut left = monster_state(&BRONZE_ORB_A0, MonsterId::new(next_id));
    let mut right = monster_state(&BRONZE_ORB_A0, MonsterId::new(next_id + 1));
    left.hp = 53;
    left.powers.minion = 1;
    right.hp = 52;
    right.powers.minion = 1;

    monsters.insert(automaton_index, left);
    monsters.insert(automaton_index + 2, right);
}

pub fn apply_large_acid_slime_split(monsters: &mut Vec<MonsterState>, slime_id: MonsterId) {
    let Some(slime_index) = monsters
        .iter()
        .position(|monster| monster.id == slime_id && monster.content_id == ACID_SLIME_ID)
    else {
        return;
    };
    if !monsters[slime_index].alive {
        return;
    }

    let next_id = monsters
        .iter()
        .map(|monster| monster.id.get())
        .max()
        .unwrap_or(0)
        + 1;
    let mut left = monster_state(&ACID_SLIME_A0, MonsterId::new(next_id));
    let mut right = monster_state(&ACID_SLIME_A0, MonsterId::new(next_id + 1));
    left.hp = 15;
    right.hp = 15;

    monsters[slime_index].hp = 0;
    monsters[slime_index].alive = false;
    monsters[slime_index].block = 0;
    monsters.insert(slime_index, left);
    monsters.insert(slime_index + 2, right);
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

fn gremlin_leader_first_available_slot(monsters: &[MonsterState]) -> Option<usize> {
    (0..3).find(|slot| {
        !monsters.iter().any(|monster| {
            monster.alive
                && is_gremlin_leader_minion_content_id(monster.content_id)
                && monster.gremlin_leader_slot == Some(*slot as u8)
        })
    })
}

fn gremlin_leader_summon_insert_index(monsters: &[MonsterState], slot: usize) -> usize {
    let leader_index = gremlin_leader_representative_summon_index(monsters);
    let new_x = gremlin_leader_slot_draw_x(slot);
    monsters
        .iter()
        .take(leader_index)
        .filter(|monster| {
            monster
                .gremlin_leader_slot
                .map(|existing_slot| gremlin_leader_slot_draw_x(existing_slot as usize) < new_x)
                .unwrap_or(false)
        })
        .count()
        .min(leader_index)
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

pub(crate) fn heal_monster_to_definition_cap(
    monster: &mut MonsterState,
    ascension: u8,
    amount: i32,
) {
    let max_hp = monster_max_hp_for_current_definition(monster, ascension);
    monster.hp = (monster.hp + amount).min(max_hp);
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
pub fn target_medium_acid_slime_next_intent_from_roll(
    move_history: &[u8],
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let wound_damage = if ascension >= 2 { 8 } else { ACID_SLIME_ATTACK_DAMAGE };
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
    if roll < 30 {
        return if hp <= SPIKE_SLIME_S_A7_HP_RANGE.max {
            MonsterIntent::ApplyPlayerWeak {
                amount: SPIKE_SLIME_LICK_WEAK,
            }
        } else {
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 }
        };
    }

    if hp <= SPIKE_SLIME_S_A7_HP_RANGE.max {
        MonsterIntent::Attack {
            damage: SPIKE_SLIME_S_SPIT_DAMAGE,
        }
    } else {
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
}

#[must_use]
fn sentry_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::AddDazedToDiscard {
            count: SENTRY_BEAM_DAZED,
        },
        _ => MonsterIntent::Attack {
            damage: SENTRY_ATTACK_DAMAGE,
        },
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
fn spheric_guardian_intent(moves_executed: u32, ascension: u8) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Block {
            block: spheric_guardian_activate_block(ascension),
        },
        1 => MonsterIntent::AttackApplyPlayerFrail {
            damage: spheric_guardian_damage(ascension),
            frail: SPHERIC_GUARDIAN_FRAIL,
        },
        _ if moves_executed % 2 == 0 => MonsterIntent::AttackMultiple {
            damage: spheric_guardian_damage(ascension),
            hits: SPHERIC_GUARDIAN_SLAM_HITS,
        },
        _ => MonsterIntent::AttackAndBlock {
            damage: spheric_guardian_damage(ascension),
            block: SPHERIC_GUARDIAN_HARDEN_BLOCK,
        },
    }
}

#[must_use]
fn guardian_intent(
    in_defensive_mode: bool,
    defensive_turns_remaining: u32,
    moves_executed: u32,
) -> MonsterIntent {
    if in_defensive_mode {
        let turn_in_sequence =
            GUARDIAN_DEFENSIVE_SEQUENCE_TURNS.saturating_sub(defensive_turns_remaining);
        match turn_in_sequence {
            0 => MonsterIntent::GuardianCloseUp {
                sharp_hide: GUARDIAN_DEFENSIVE_SPIKES,
            },
            1 => MonsterIntent::Attack {
                damage: GUARDIAN_DEFENSIVE_ATTACK_DAMAGE,
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
                damage: GUARDIAN_FIERCE_BASH_DAMAGE,
            },
            2 => MonsterIntent::ApplyPlayerWeak {
                amount: GUARDIAN_VENT_DEBUFF,
            },
            _ => MonsterIntent::AttackMultiple {
                damage: GUARDIAN_WHIRLWIND_DAMAGE,
                hits: GUARDIAN_WHIRLWIND_HITS,
            },
        }
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
    monster.mode_shift = 0;
    monster.intent = guardian_intent(
        true,
        monster.defensive_turns_remaining,
        monster.moves_executed,
    );
}

fn exit_guardian_defensive_mode(monster: &mut MonsterState) {
    if monster.content_id != GUARDIAN_ID || !monster.in_defensive_mode {
        return;
    }
    monster.in_defensive_mode = false;
    monster.defensive_turns_remaining = 0;
    monster.powers.spikes = 0;
    monster.mode_shift = GUARDIAN_MODE_SHIFT_RESET;
    monster.moves_executed = 2;
    monster.intent = guardian_intent(false, 0, monster.moves_executed);
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

pub fn large_acid_slime_on_hp_damage(monster: &mut MonsterState, hp_damage: i32) {
    if hp_damage <= 0
        || !monster.alive
        || monster.content_id != ACID_SLIME_ID
        || monster.split_triggered
        || monster.rolled_attack_damage.is_none()
        || monster.hp > 34
    {
        return;
    }

    monster.intent = MonsterIntent::SummonGremlins { count: 2 };
    monster.split_triggered = true;
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
        );
    }
}

#[must_use]
fn slime_boss_intent() -> MonsterIntent {
    MonsterIntent::Attack {
        damage: SLIME_BOSS_SLAM_DAMAGE,
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
        2 | 4 => MonsterIntent::AddBurnToDiscard {
            count: HEXAGHOST_SEAR_BURNS,
            damage: HEXAGHOST_DIVIDER_DAMAGE,
        },
        3 => MonsterIntent::AttackMultiple {
            damage: HEXAGHOST_TACKLE_DAMAGE,
            hits: HEXAGHOST_TACKLE_HITS,
        },
        5 => MonsterIntent::Stun,
        _ => MonsterIntent::AddBurnToDiscard {
            count: HEXAGHOST_INFERNO_BURNS,
            damage: HEXAGHOST_INFERNO_DAMAGE,
        },
    }
}

#[must_use]
fn lagavulin_intent(sleep_turns_remaining: u32, moves_executed: u32) -> MonsterIntent {
    if sleep_turns_remaining > 0 {
        MonsterIntent::Sleep
    } else if moves_executed % 3 == 1 {
        MonsterIntent::SiphonPlayer {
            strength: LAGAVULIN_SIPHON_STRENGTH,
            dexterity: LAGAVULIN_SIPHON_DEXTERITY,
        }
    } else {
        MonsterIntent::Attack {
            damage: LAGAVULIN_ATTACK_DAMAGE,
        }
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

/// Spawns acid slimes when the Slime Boss crosses its split threshold.
pub fn check_slime_boss_split(state: &mut crate::CombatState, monster_id: MonsterId) {
    let should_split = state
        .monsters
        .iter()
        .find(|monster| monster.id == monster_id)
        .is_some_and(|monster| {
            monster.content_id == SLIME_BOSS_ID
                && monster.alive
                && !monster.split_triggered
                && monster.hp <= SLIME_BOSS_SPLIT_HP_THRESHOLD
        });

    if !should_split {
        return;
    }

    let next_monster_id = state
        .monsters
        .iter()
        .map(|monster| monster.id.get())
        .max()
        .unwrap_or(0)
        + 1;

    if let Some(boss) = state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == monster_id)
    {
        boss.split_triggered = true;
    }

    state.monsters.push(monster_state(
        &ACID_SLIME_A0,
        MonsterId::new(next_monster_id),
    ));
    state.monsters.push(monster_state(
        &ACID_SLIME_A0,
        MonsterId::new(next_monster_id + 1),
    ));
}

/// Gremlin Nob opens with Bellow, then Skull Bash, then Rush attacks.
#[must_use]
fn gremlin_nob_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed {
        0 => MonsterIntent::Block { block: 0 },
        1 => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: GREMLIN_NOB_SKULL_BASH_DAMAGE,
            vulnerable: 2,
        },
        _ => MonsterIntent::Attack {
            damage: GREMLIN_NOB_RUSH_DAMAGE,
        },
    }
}

/// Deterministic Jaw Worm move cycle: Chomp → Thrash → Bellow, keyed on `moves_executed`.
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
pub fn target_jaw_worm_next_intent_from_roll(move_history: &[u8], roll: i32) -> MonsterIntent {
    if move_history.is_empty() {
        return jaw_worm_chomp_intent();
    }
    if roll < 25 {
        if last_move(move_history, 1) {
            return jaw_worm_thrash_intent();
        }
        return jaw_worm_chomp_intent();
    }
    if roll < 55 {
        if last_move(move_history, 3) {
            return jaw_worm_bellow_intent();
        }
        return jaw_worm_thrash_intent();
    }
    if last_move(move_history, 2) {
        return jaw_worm_chomp_intent();
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
    previous_intent: MonsterIntent,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let roll = rng.random_int(99);
    target_large_acid_slime_next_intent_from_roll(previous_intent, roll, rng, ascension)
}

#[must_use]
pub fn target_large_acid_slime_next_intent_from_roll(
    previous_intent: MonsterIntent,
    roll: i32,
    rng: &mut StsRng,
    ascension: u8,
) -> MonsterIntent {
    let wound_damage = if ascension >= 2 { 12 } else { 11 };
    let attack_damage = if ascension >= 2 { 18 } else { 16 };
    let weak = if ascension >= 17 { 3 } else { 2 };

    if ascension >= 17 {
        if roll < 40 {
            if matches!(
                previous_intent,
                MonsterIntent::AttackAddSlimedToDiscard { .. }
            ) {
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
            if matches!(previous_intent, MonsterIntent::Attack { .. }) {
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
        } else if matches!(previous_intent, MonsterIntent::ApplyPlayerWeak { .. }) {
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
    } else if matches!(
        previous_intent,
        MonsterIntent::AttackAddSlimedToDiscard { .. }
    ) {
        MonsterIntent::Attack {
            damage: attack_damage,
        }
    } else if roll < 30 {
        MonsterIntent::AttackAddSlimedToDiscard {
            damage: wound_damage,
            count: 2,
        }
    } else if roll < 70 {
        if matches!(previous_intent, MonsterIntent::Attack { .. }) {
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
    } else if matches!(previous_intent, MonsterIntent::ApplyPlayerWeak { .. }) {
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

/// Execute the monster's current intent and return player damage dealt this turn.
pub fn apply_monster_intent(
    monster: &mut MonsterState,
    player: &mut crate::PlayerState,
    piles: &mut CardPiles,
    ascension: u8,
    player_before: &crate::PlayerState,
    relics: &[crate::Relic],
) -> i32 {
    apply_monster_intent_with_card_rng(
        monster,
        player,
        piles,
        ascension,
        player_before,
        relics,
        None,
    )
}

pub fn apply_monster_intent_with_card_rng(
    monster: &mut MonsterState,
    player: &mut crate::PlayerState,
    piles: &mut CardPiles,
    ascension: u8,
    player_before: &crate::PlayerState,
    relics: &[crate::Relic],
    card_random_rng: Option<&mut StsRng>,
) -> i32 {
    use crate::combat::damage::deal_unmodified_damage_to_monster;
    use crate::combat::turn_powers::monster_damage_to_player;
    use crate::power::{
        apply_player_confusion, apply_player_entangled, apply_player_frail, apply_player_hex,
        apply_player_vulnerable, reduce_player_dexterity, reduce_player_strength,
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
    );
    let scale_damage = |damage: i32| {
        if source_scaled_damage {
            damage
        } else {
            config.scaled_attack_damage(damage)
        }
    };
    let (damage, thorns_hits) = match monster.intent {
        MonsterIntent::Attack { damage } => (
            monster_damage_to_player(player_before, monster, scale_damage(damage)),
            1,
        ),
        MonsterIntent::Block { block } => {
            monster.block += block;
            (0, 0)
        }
        MonsterIntent::Ritual { amount } => {
            monster.powers.ritual += amount;
            (0, 0)
        }
        MonsterIntent::AttackAndBlock { damage, block } => {
            monster.block += block;
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::StrengthAndBlock { strength, block } => {
            monster.powers.strength += strength;
            monster.block += block;
            (0, 0)
        }
        MonsterIntent::StrengthSelf { amount } => {
            monster.powers.strength += amount;
            (0, 0)
        }
        MonsterIntent::ApplyPlayerWeak { amount } => {
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, amount);
            (0, 0)
        }
        MonsterIntent::AttackApplyPlayerWeak { damage, weak } => {
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, weak);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerVulnerable { damage, vulnerable } => {
            let starts_new_vulnerable =
                vulnerable > 0 && player.powers.vulnerable == 0 && player.powers.artifact == 0;
            apply_player_vulnerable(&mut player.powers, vulnerable);
            if starts_new_vulnerable {
                player.vulnerable_just_applied = true;
            }
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
            damage,
            weak,
            vulnerable,
        } => {
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, weak);
            let starts_new_vulnerable =
                vulnerable > 0 && player.powers.vulnerable == 0 && player.powers.artifact == 0;
            apply_player_vulnerable(&mut player.powers, vulnerable);
            if starts_new_vulnerable {
                player.vulnerable_just_applied = true;
            }
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerFrailAndWeak {
            damage,
            frail,
            weak,
        } => {
            apply_player_frail(&mut player.powers, frail);
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, weak);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::AttackApplyPlayerFrail { damage, frail } => {
            apply_player_frail(&mut player.powers, frail);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::AttackHealSelf { damage } => (
            monster_damage_to_player(player_before, monster, scale_damage(damage)),
            1,
        ),
        MonsterIntent::ApplyPlayerHex { amount } => {
            apply_player_hex(&mut player.powers, amount);
            (0, 0)
        }
        MonsterIntent::ApplyPlayerFrailAndWeak { frail, weak } => {
            apply_player_frail(&mut player.powers, frail);
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, weak);
            (0, 0)
        }
        MonsterIntent::ApplyPlayerWeakStrengthSelf { weak, strength } => {
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, weak);
            monster.powers.strength += strength;
            (0, 0)
        }
        MonsterIntent::ApplyPlayerConfusion => {
            apply_player_confusion(&mut player.powers);
            (0, 0)
        }
        MonsterIntent::ApplyPlayerEntangled { amount } => {
            apply_player_entangled(&mut player.powers, amount);
            (0, 0)
        }
        MonsterIntent::HealAllMonsters { .. }
        | MonsterIntent::StrengthAllMonsters { .. }
        | MonsterIntent::EncourageGremlins { .. }
        | MonsterIntent::SummonGremlins { .. } => (0, 0),
        MonsterIntent::AttackAddSlimedToDiscard { damage, count } => {
            add_cards_to_discard(piles, SLIMED_ID, count);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::AttackAddWoundsToDiscard { damage, count } => {
            let count = if monster.content_id == TASKMASTER_ID {
                taskmaster_wound_count(ascension)
            } else {
                count
            };
            add_cards_to_discard(piles, crate::content::cards::WOUND_ID, count);
            let damage_taken =
                monster_damage_to_player(player_before, monster, scale_damage(damage));
            if monster.content_id == TASKMASTER_ID && ascension >= 18 {
                monster.powers.strength += TASKMASTER_A18_STRENGTH;
            }
            (damage_taken, 1)
        }
        MonsterIntent::AttackStealGold { damage, amount } => {
            monster.stolen_gold += amount.max(0);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
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
        MonsterIntent::Stun => (0, 0),
        MonsterIntent::SiphonPlayer {
            strength,
            dexterity,
        } => {
            reduce_player_strength(&mut player.powers, strength);
            reduce_player_dexterity(&mut player.powers, dexterity);
            bronze_orb_apply_stasis(monster, piles, card_random_rng);
            monster.has_siphoned = true;
            (0, 0)
        }
        MonsterIntent::AddDazedToDiscard { count } => {
            add_cards_to_discard(piles, DAZED_ID, count);
            (0, 0)
        }
        MonsterIntent::AddBurnToDiscard { count, damage } => {
            add_cards_to_discard(piles, BURN_ID, count);
            (monster_attack_damage(monster, scale_damage(damage)), 1)
        }
        MonsterIntent::AddBurnToDiscardAndDraw { damage, .. } => {
            (monster_attack_damage(monster, scale_damage(damage)), 1)
        }
        MonsterIntent::AttackMultiple { damage, hits } => {
            let hit_damage = monster_damage_to_player(player_before, monster, scale_damage(damage));
            (hit_damage * hits, hits)
        }
        MonsterIntent::GuardianCloseUp { sharp_hide } => {
            monster.powers.spikes = sharp_hide;
            (0, 0)
        }
        MonsterIntent::DefensiveCharge { block, strength } => {
            monster.block += block;
            monster.powers.strength += strength;
            if monster.defensive_turns_remaining > 0 {
                monster.defensive_turns_remaining -= 1;
            }
            (0, 0)
        }
    };
    let total_thorns = player.powers.thorns + player.temp_thorns;
    if total_thorns > 0 && thorns_hits > 0 {
        deal_unmodified_damage_to_monster(monster, total_thorns * thorns_hits);
    }
    if monster.alive && thorns_hits > 0 && monster.powers.strength_up > 0 {
        monster.powers.strength += monster.powers.strength_up;
    }
    if monster.content_id == GUARDIAN_ID && monster.in_defensive_mode {
        finish_guardian_defensive_turn(monster);
    }
    monster.moves_executed += 1;
    damage
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
    card_random_rng: Option<&mut StsRng>,
) {
    if monster.content_id != BRONZE_ORB_ID || monster.stasis_card.is_some() {
        return;
    }
    let Some(card) = take_stasis_card(piles, card_random_rng) else {
        return;
    };
    monster.stasis_card = Some(card);
}

fn take_stasis_card(
    piles: &mut CardPiles,
    card_random_rng: Option<&mut StsRng>,
) -> Option<CardInstance> {
    let rng = card_random_rng?;
    if !piles.draw_pile.is_empty() {
        return take_random_card_by_stasis_priority(&mut piles.draw_pile, rng);
    }
    if !piles.discard_pile.is_empty() {
        return take_random_card_by_stasis_priority(&mut piles.discard_pile, rng);
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
            let (_, card_rarity) = card_type_and_rarity(card.content_id)?;
            let key = crate::content::cards::get_card_definition(card.content_id)?.key;
            (card_rarity == rarity).then_some((index, key))
        })
        .collect::<Vec<_>>();
    if candidate_indices.is_empty() {
        return None;
    }
    candidate_indices.sort_by(|(_, left), (_, right)| left.cmp(right));
    let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
    Some(pile.remove(candidate_indices[pick].0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::MonsterIntent, power::PlayerPowers, PlayerState};

    fn dummy_player() -> PlayerState {
        PlayerState {
            hp: 80,
            max_hp: 80,
            block: 0,
            energy: 3,
            max_energy: 3,
            powers: PlayerPowers::default(),
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
        }
    }

    fn dummy_piles() -> CardPiles {
        CardPiles {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        }
    }

    fn apply_intent(monster: &mut MonsterState) -> i32 {
        let mut player = dummy_player();
        let mut piles = dummy_piles();
        let player_before = player.clone();
        apply_monster_intent(monster, &mut player, &mut piles, 0, &player_before, &[])
    }

    #[test]
    fn cultist_has_fifty_hp() {
        assert_eq!(CULTIST_A0.hp, 50);
    }

    #[test]
    fn target_version_hp_ranges_match_decoded_act1_constructor_bytecode() {
        assert_eq!(target_cultist_hp_range(0), MonsterHpRange::new(48, 54));
        assert_eq!(target_cultist_hp_range(7), MonsterHpRange::new(50, 56));
        assert_eq!(target_jaw_worm_hp_range(0), MonsterHpRange::new(40, 44));
        assert_eq!(target_jaw_worm_hp_range(7), MonsterHpRange::new(42, 46));
        assert_eq!(
            target_spike_slime_s_hp_range(0),
            MonsterHpRange::new(10, 14)
        );
        assert_eq!(
            target_spike_slime_s_hp_range(7),
            MonsterHpRange::new(11, 15)
        );
        assert_eq!(target_acid_slime_s_hp_range(0), MonsterHpRange::new(8, 12));
        assert_eq!(target_acid_slime_s_hp_range(7), MonsterHpRange::new(9, 13));
        assert_eq!(
            target_spike_slime_m_hp_range(0),
            MonsterHpRange::new(28, 32)
        );
        assert_eq!(
            target_spike_slime_m_hp_range(7),
            MonsterHpRange::new(29, 34)
        );
        assert_eq!(target_acid_slime_m_hp_range(0), MonsterHpRange::new(28, 32));
        assert_eq!(target_acid_slime_m_hp_range(7), MonsterHpRange::new(29, 34));
        assert_eq!(target_louse_normal_hp_range(0), MonsterHpRange::new(10, 15));
        assert_eq!(
            target_louse_defensive_hp_range(0),
            MonsterHpRange::new(11, 17)
        );
        assert_eq!(target_louse_bite_damage_range(0), MonsterHpRange::new(5, 7));
        assert_eq!(target_louse_bite_damage_range(2), MonsterHpRange::new(6, 8));
    }

    #[test]
    fn city_encounter_groups_match_target_monster_helper_source() {
        let thieves = target_city_encounter_group_for_key("2 Thieves").expect("2 Thieves group");
        assert_eq!(thieves.display_name, "2 Thieves");
        assert_eq!(
            thieves.members,
            vec![
                TargetEncounterMember {
                    monster_name: "Looter",
                    x: Some("-200.0"),
                    y: Some("15.0"),
                },
                TargetEncounterMember {
                    monster_name: "Mugger",
                    x: Some("80.0"),
                    y: Some("0.0"),
                },
            ]
        );

        let chosen_and_byrds =
            target_city_encounter_group_for_key("Chosen and Byrds").expect("Chosen and Byrds");
        assert_eq!(chosen_and_byrds.display_name, "Chosen and Byrds");
        assert_eq!(
            chosen_and_byrds.members,
            vec![
                TargetEncounterMember {
                    monster_name: "Byrd",
                    x: Some("-170.0"),
                    y: Some("random(25.0, 70.0)"),
                },
                TargetEncounterMember {
                    monster_name: "Chosen",
                    x: Some("80.0"),
                    y: Some("0.0"),
                },
            ]
        );

        let slavers = target_city_encounter_group_for_key("Slavers").expect("Slavers group");
        assert_eq!(slavers.display_name, "Taskmaster");
        assert_eq!(
            slavers
                .members
                .iter()
                .map(|member| member.monster_name)
                .collect::<Vec<_>>(),
            vec!["SlaverBlue", "Taskmaster", "SlaverRed"]
        );
    }

    #[test]
    fn city_encounter_group_lookup_follows_city_normal_key_list() {
        let encounter_key =
            crate::content::encounters::city_normal_encounter_key_at_combat_index(1_218_623, 0)
                .expect("city encounter key");

        let group = target_city_normal_encounter_group_at_combat_index(1_218_623, 0)
            .expect("city encounter group");

        assert_eq!(group.encounter_key, encounter_key);
        assert!(!group.members.is_empty());
    }

    #[test]
    fn executable_city_encounter_spawns_supported_single_and_multi_member_groups() {
        let sphere = executable_city_encounter_monsters_for_key("Spheric Guardian")
            .expect("Spheric Guardian is executable");
        assert_eq!(sphere.len(), 1);
        assert_eq!(sphere[0].content_id, SPHERIC_GUARDIAN_ID);
        assert_eq!(sphere[0].block, SPHERIC_GUARDIAN_STARTING_BLOCK);
        assert_eq!(sphere[0].powers.artifact, SPHERIC_GUARDIAN_ARTIFACT);

        let byrds =
            executable_city_encounter_monsters_for_key("3 Byrds").expect("3 Byrds executable");
        assert_eq!(byrds.len(), 3);
        assert!(byrds
            .iter()
            .all(|monster| monster.content_id == BYRD_ID && monster.powers.flight == BYRD_FLIGHT));
        assert_eq!(
            byrds
                .iter()
                .map(|monster| monster.id.get())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        let centurion_healer = executable_city_encounter_monsters_for_key("Centurion and Healer")
            .expect("Centurion and Healer executable");
        assert_eq!(
            centurion_healer
                .iter()
                .map(|monster| monster.content_id)
                .collect::<Vec<_>>(),
            vec![CENTURION_ID, HEALER_ID]
        );

        let book = executable_city_encounter_monsters_for_key("Book of Stabbing")
            .expect("Book of Stabbing executable");
        assert_eq!(book[0].content_id, BOOK_OF_STABBING_ID);
        assert_eq!(book[0].powers.painful_stabs, 1);
    }

    #[test]
    fn executable_city_encounter_spawns_supported_mixed_act_one_members() {
        let thieves =
            executable_city_encounter_monsters_for_key("2 Thieves").expect("2 Thieves executable");
        assert_eq!(
            thieves
                .iter()
                .map(|monster| monster.content_id)
                .collect::<Vec<_>>(),
            vec![LOOTER_ID, MUGGER_ID]
        );

        let sentry_sphere = executable_city_encounter_monsters_for_key("Sentry and Sphere")
            .expect("Sentry and Sphere executable");
        assert_eq!(
            sentry_sphere
                .iter()
                .map(|monster| monster.content_id)
                .collect::<Vec<_>>(),
            vec![SENTRY_ID, SPHERIC_GUARDIAN_ID]
        );
    }

    #[test]
    fn executable_city_encounter_refuses_unsupported_group_members() {
        assert!(executable_city_member_definition("DefinitelyUnknown").is_none());
    }

    #[test]
    fn executable_city_encounter_spawns_fungi_and_slavers_groups() {
        let parasite_fungi =
            executable_city_encounter_monsters_for_key("Shelled Parasite and Fungi")
                .expect("Shelled Parasite and Fungi executable");
        assert_eq!(
            parasite_fungi
                .iter()
                .map(|monster| monster.content_id)
                .collect::<Vec<_>>(),
            vec![SHELLED_PARASITE_ID, FUNGI_BEAST_ID]
        );
        assert_eq!(
            parasite_fungi[1].powers.spore_cloud,
            FUNGI_BEAST_SPORE_CLOUD
        );

        let slavers = executable_city_encounter_monsters_for_key("Slavers")
            .expect("Slavers executable after red/blue slavers");
        assert_eq!(
            slavers
                .iter()
                .map(|monster| monster.content_id)
                .collect::<Vec<_>>(),
            vec![SLAVER_BLUE_ID, TASKMASTER_ID, SLAVER_RED_ID]
        );
    }

    #[test]
    fn executable_city_encounter_spawns_representative_gremlin_leader_group() {
        let group = executable_city_encounter_monsters_for_key("Gremlin Leader")
            .expect("Gremlin Leader representative group executable");
        assert_eq!(
            group
                .iter()
                .map(|monster| monster.content_id)
                .collect::<Vec<_>>(),
            vec![GREMLIN_WARRIOR_ID, GREMLIN_WARRIOR_ID, GREMLIN_LEADER_ID]
        );
        assert_eq!(group[0].powers.minion, 1);
        assert_eq!(group[0].powers.anger, GREMLIN_WARRIOR_ANGER);
        assert_eq!(group[1].powers.minion, 1);
    }

    #[test]
    fn target_city_encounter_spawn_metadata_rolls_supported_groups() {
        let sphere =
            target_city_encounter_spawn_for_key(1_218_623, 18, "Spheric Guardian", 0, false)
                .expect("sphere spawn metadata");
        assert_eq!(sphere.len(), 1);
        assert_eq!(sphere[0].name, "SphericGuardian");
        assert_eq!(sphere[0].max_hp, SPHERIC_GUARDIAN_A0.hp);
        assert_eq!(sphere[0].current_hp, SPHERIC_GUARDIAN_A0.hp);
        assert_eq!(sphere[0].block, SPHERIC_GUARDIAN_STARTING_BLOCK);
        assert_eq!(
            sphere[0].powers,
            vec![TargetSpawnPower {
                id: "Artifact",
                amount: SPHERIC_GUARDIAN_ARTIFACT,
            }]
        );

        let byrds = target_city_encounter_spawn_for_key(1_218_623, 18, "3 Byrds", 0, false)
            .expect("byrd spawn metadata");
        assert_eq!(byrds.len(), 3);
        assert!(byrds.iter().all(|spawn| {
            spawn.name == "Byrd"
                && BYRD_A0_HP_RANGE.contains(spawn.max_hp)
                && spawn.powers
                    == vec![TargetSpawnPower {
                        id: "Flight",
                        amount: BYRD_FLIGHT,
                    }]
        }));

        let book = target_city_encounter_spawn_for_key(1_218_623, 23, "Book of Stabbing", 0, true)
            .expect("book spawn metadata");
        assert_eq!(book[0].name, "BookOfStabbing");
        assert!(BOOK_OF_STABBING_A0_HP_RANGE.contains(book[0].max_hp));
        assert_eq!(book[0].current_hp, 1);
        assert_eq!(
            book[0].powers,
            vec![TargetSpawnPower {
                id: "Painful Stabs",
                amount: 1,
            }]
        );
    }

    #[test]
    fn target_city_encounter_spawn_metadata_rolls_fungi_and_slavers_groups() {
        let parasite_fungi = target_city_encounter_spawn_for_key(
            1_218_623,
            18,
            "Shelled Parasite and Fungi",
            0,
            false,
        )
        .expect("Shelled Parasite and Fungi spawn metadata");
        assert_eq!(parasite_fungi.len(), 2);
        assert_eq!(parasite_fungi[1].name, "FungiBeast");
        assert!(FUNGI_BEAST_A0_HP_RANGE.contains(parasite_fungi[1].max_hp));
        assert_eq!(
            parasite_fungi[1].powers,
            vec![TargetSpawnPower {
                id: "Spore Cloud",
                amount: FUNGI_BEAST_SPORE_CLOUD,
            }]
        );

        let slavers = target_city_encounter_spawn_for_key(1_218_623, 23, "Slavers", 0, false)
            .expect("Slavers spawn metadata");
        assert_eq!(
            slavers.iter().map(|spawn| spawn.name).collect::<Vec<_>>(),
            vec!["SlaverBlue", "Taskmaster", "SlaverRed"]
        );
        assert!(SLAVER_A0_HP_RANGE.contains(slavers[0].max_hp));
        assert!(TASKMASTER_A0_HP_RANGE.contains(slavers[1].max_hp));
        assert!(SLAVER_A0_HP_RANGE.contains(slavers[2].max_hp));
    }

    #[test]
    fn target_city_encounter_spawn_metadata_rolls_random_gremlin_leader_group() {
        let group = target_city_encounter_spawn_for_key(1_218_623, 23, "Gremlin Leader", 0, false)
            .expect("Gremlin Leader spawn metadata");
        assert_eq!(
            group.iter().map(|spawn| spawn.name).collect::<Vec<_>>(),
            vec!["GremlinThief", "GremlinFat", "GremlinLeader"]
        );
        assert!(GREMLIN_THIEF_A0_HP_RANGE.contains(group[0].max_hp));
        assert!(GREMLIN_FAT_A0_HP_RANGE.contains(group[1].max_hp));
        assert!(GREMLIN_LEADER_A0_HP_RANGE.contains(group[2].max_hp));
        assert_eq!(
            group[0].powers,
            vec![TargetSpawnPower {
                id: "Minion",
                amount: 1,
            }]
        );
        assert_eq!(
            group[1].powers,
            vec![TargetSpawnPower {
                id: "Minion",
                amount: 1,
            }]
        );
    }

    #[test]
    fn trace_seed_gremlin_leader_spawn_matches_target_random_gremlins() {
        let mut misc_rng = StsRng::new(1_435_099_163_257);
        let group = target_city_encounter_spawn_for_key_with_misc_rng(
            1_435_099_163_226,
            31,
            "Gremlin Leader",
            0,
            false,
            Some(&mut misc_rng),
        )
        .expect("Gremlin Leader trace spawn metadata");

        assert_eq!(
            group.iter().map(|spawn| spawn.name).collect::<Vec<_>>(),
            vec!["GremlinTsundere", "GremlinFat", "GremlinLeader"]
        );
        assert_eq!(group[0].max_hp, 14);
        assert_eq!(group[1].max_hp, 15);
        assert_eq!(group[2].max_hp, 140);
        assert_eq!(misc_rng.counter(), 2);
    }

    #[test]
    fn city_monster_hp_ranges_match_target_constructor_sources() {
        assert_eq!(
            target_city_monster_hp_range("Byrd", 0),
            Some(MonsterHpRange::new(25, 31))
        );
        assert_eq!(
            target_city_monster_hp_range("Byrd", 7),
            Some(MonsterHpRange::new(26, 33))
        );
        assert_eq!(
            target_city_monster_hp_range("Chosen", 0),
            Some(MonsterHpRange::new(95, 99))
        );
        assert_eq!(
            target_city_monster_hp_range("Chosen", 7),
            Some(MonsterHpRange::new(98, 103))
        );
        assert_eq!(
            target_city_monster_hp_range("SphericGuardian", 20),
            Some(MonsterHpRange::new(20, 20))
        );
        assert_eq!(
            target_city_monster_hp_range("Snecko", 0),
            Some(MonsterHpRange::new(114, 120))
        );
        assert_eq!(
            target_city_monster_hp_range("Snecko", 7),
            Some(MonsterHpRange::new(120, 125))
        );
        assert_eq!(
            target_city_monster_hp_range("BookOfStabbing", 7),
            Some(MonsterHpRange::new(160, 164))
        );
        assert_eq!(
            target_city_monster_hp_range("BookOfStabbing", 8),
            Some(MonsterHpRange::new(168, 172))
        );
        assert_eq!(
            target_city_monster_hp_range("GremlinLeader", 8),
            Some(MonsterHpRange::new(145, 155))
        );
        assert_eq!(
            target_city_monster_hp_range("Taskmaster", 8),
            Some(MonsterHpRange::new(57, 64))
        );
    }

    #[test]
    fn city_group_members_have_hp_inventory_only_when_decoded() {
        let chosen_and_byrds =
            target_city_encounter_group_for_key("Chosen and Byrds").expect("Chosen and Byrds");
        assert!(chosen_and_byrds
            .members
            .iter()
            .all(|member| target_city_monster_hp_range(member.monster_name, 0).is_some()));

        let slavers = target_city_encounter_group_for_key("Slavers").expect("Slavers group");
        assert_eq!(
            slavers
                .members
                .iter()
                .filter(|member| target_city_monster_hp_range(member.monster_name, 0).is_some())
                .map(|member| member.monster_name)
                .collect::<Vec<_>>(),
            vec!["SlaverBlue", "Taskmaster", "SlaverRed"]
        );
    }

    #[test]
    fn city_monster_profiles_include_source_backed_damage_and_status_constants() {
        let chosen_a0 = target_city_monster_profile("Chosen", 0).expect("chosen profile");
        assert_eq!(chosen_a0.hp_range, CHOSEN_A0_HP_RANGE);
        assert!(chosen_a0.constants.contains(&TargetMonsterConstant {
            name: "zap_damage",
            value: 18,
        }));
        assert!(chosen_a0.constants.contains(&TargetMonsterConstant {
            name: "poke_damage",
            value: 5,
        }));

        let chosen_a2 = target_city_monster_profile("Chosen", 2).expect("chosen profile");
        assert!(chosen_a2.constants.contains(&TargetMonsterConstant {
            name: "zap_damage",
            value: 21,
        }));
        assert!(chosen_a2.constants.contains(&TargetMonsterConstant {
            name: "poke_damage",
            value: 6,
        }));

        let byrd_a17 = target_city_monster_profile("Byrd", 17).expect("byrd profile");
        assert!(byrd_a17.constants.contains(&TargetMonsterConstant {
            name: "flight_amount",
            value: 4,
        }));
        assert!(byrd_a17.constants.contains(&TargetMonsterConstant {
            name: "peck_hits",
            value: 6,
        }));

        let sphere_a17 =
            target_city_monster_profile("Spheric Guardian", 17).expect("sphere profile");
        assert_eq!(sphere_a17.monster_name, "SphericGuardian");
        assert!(sphere_a17.constants.contains(&TargetMonsterConstant {
            name: "activate_block",
            value: 35,
        }));
    }

    #[test]
    fn city_elite_profiles_use_target_elite_thresholds() {
        let book_a2 = target_city_monster_profile("BookOfStabbing", 2).expect("book profile");
        assert_eq!(book_a2.hp_range, BOOK_OF_STABBING_A0_HP_RANGE);
        assert!(book_a2.constants.contains(&TargetMonsterConstant {
            name: "stab_damage",
            value: 6,
        }));

        let book_a8 = target_city_monster_profile("Book of Stabbing", 8).expect("book profile");
        assert_eq!(book_a8.monster_name, "BookOfStabbing");
        assert_eq!(book_a8.hp_range, BOOK_OF_STABBING_A8_HP_RANGE);
        assert!(book_a8.constants.contains(&TargetMonsterConstant {
            name: "stab_damage",
            value: 7,
        }));

        let leader_a18 = target_city_monster_profile("Gremlin Leader", 18).expect("leader profile");
        assert!(leader_a18.constants.contains(&TargetMonsterConstant {
            name: "strength",
            value: 5,
        }));
        assert!(leader_a18.constants.contains(&TargetMonsterConstant {
            name: "block",
            value: 10,
        }));

        let taskmaster_a18 = target_city_monster_profile("Taskmaster", 18).expect("taskmaster");
        assert!(taskmaster_a18.constants.contains(&TargetMonsterConstant {
            name: "wounds",
            value: 3,
        }));
    }

    #[test]
    fn floor_one_cultist_hp_roll_matches_verify01_trace() {
        assert_eq!(target_cultist_hp_roll(1_957_307_888_551, 1, 0), 49);
    }

    #[test]
    fn floor_one_cultist_hp_roll_matches_codex04_trace() {
        let mut codex04 = StsRng::new(22_079_335_079);

        assert_eq!(target_cultist_hp_range(0).roll(&mut codex04), 53);
        assert_eq!(target_cultist_hp_roll(22_079_335_079, 1, 0), 54);
    }

    #[test]
    fn floor_three_louse_spawn_powers_match_captured_traces() {
        assert_eq!(
            target_two_louse_kinds(22_079_335_079, 3),
            [LouseKind::Defensive, LouseKind::Defensive]
        );
        assert_eq!(
            target_two_louse_kinds(22_079_335_078, 3),
            [LouseKind::Normal, LouseKind::Defensive]
        );

        let codex04 = target_two_louse_spawn_states(22_079_335_079, 3, 0, false);
        assert_eq!(codex04.len(), 2);
        assert_eq!(codex04[0].max_hp, 13);
        assert_eq!(codex04[1].max_hp, 15);
        assert_eq!(
            codex04
                .iter()
                .map(|spawn| spawn.powers.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![TargetSpawnPower {
                    id: "Curl Up",
                    amount: 3,
                }],
                vec![TargetSpawnPower {
                    id: "Curl Up",
                    amount: 3,
                }],
            ]
        );

        let codex03 = target_two_louse_spawn_states(22_079_335_078, 3, 0, true);
        assert_eq!(codex03[0].max_hp, 12);
        assert_eq!(codex03[1].max_hp, 16);
        assert_eq!(
            codex03
                .iter()
                .map(|spawn| spawn.powers.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![TargetSpawnPower {
                    id: "Curl Up",
                    amount: 3,
                }],
                vec![TargetSpawnPower {
                    id: "Curl Up",
                    amount: 7,
                }],
            ]
        );
    }

    #[test]
    fn floor_one_codex03_jaw_worm_hp_roll_matches_lament_trace_max_hp() {
        assert_eq!(target_jaw_worm_hp_roll(22_079_335_078, 1, 0), 43);
    }

    #[test]
    fn later_codex04_hp_rolls_need_encounter_constructor_order_before_claiming_parity() {
        let mut rng = StsRng::new(22_079_335_079 + 2);
        let _floor_one_cultist = target_cultist_hp_range(0).roll(&mut rng);
        let naive_spike_slime_s = target_spike_slime_s_hp_range(0).roll(&mut rng);
        let naive_acid_slime_m = target_acid_slime_m_hp_range(0).roll(&mut rng);

        assert_ne!(naive_spike_slime_s, 11);
        assert_ne!(naive_acid_slime_m, 32);
    }

    #[test]
    fn floor_two_codex04_small_slimes_variant_and_hp_match_trace() {
        assert_eq!(
            target_small_slimes_variant(22_079_335_079, 2),
            SmallSlimesVariant::SpikeSmallAcidMedium
        );
        assert_eq!(
            target_small_slimes_hp_rolls(22_079_335_079, 2, 0).expect("decoded reached variant"),
            vec![
                TargetMonsterHp {
                    name: "Spike Slime (S)",
                    hp: 11,
                },
                TargetMonsterHp {
                    name: "Acid Slime (M)",
                    hp: 32,
                },
            ]
        );
    }

    #[test]
    fn floor_two_test_small_slimes_variant_and_hp_match_trace() {
        assert_eq!(
            target_small_slimes_variant(1_218_623, 2),
            SmallSlimesVariant::AcidSmallSpikeMedium
        );
        assert_eq!(
            target_small_slimes_hp_rolls(1_218_623, 2, 0).expect("decoded reached variant"),
            vec![
                TargetMonsterHp {
                    name: "Acid Slime (S)",
                    hp: 10,
                },
                TargetMonsterHp {
                    name: "Spike Slime (M)",
                    hp: 29,
                },
            ]
        );
    }

    #[test]
    fn floor_three_codex04_two_louse_kinds_and_hp_match_trace() {
        assert_eq!(
            target_two_louse_kinds(22_079_335_079, 3),
            [LouseKind::Defensive, LouseKind::Defensive]
        );
        assert_eq!(
            target_two_louse_hp_rolls(22_079_335_079, 3, 0),
            vec![
                TargetMonsterHp {
                    name: "LouseDefensive",
                    hp: 13,
                },
                TargetMonsterHp {
                    name: "LouseDefensive",
                    hp: 15,
                },
            ]
        );
    }

    #[test]
    fn cultist_starts_with_ritual_intent() {
        let monster = monster_state(&CULTIST_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Ritual {
                amount: CULTIST_A0.ritual_amount
            }
        );
    }

    #[test]
    fn cultist_move_selection_ritual_then_attack() {
        let definition = &CULTIST_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Ritual { amount: 3 }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::Attack { damage: 6 }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
            MonsterIntent::Attack { damage: 6 }
        );
    }

    #[test]
    fn cultist_prepare_monster_intent_tracks_moves_executed() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));

        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Ritual { amount: 3 }
        );

        monster.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack { damage: 6 }
        );
    }

    #[test]
    fn cultist_ritual_intent_grants_ritual_power() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.powers.ritual, 3);
        assert_eq!(monster.moves_executed, 1);
    }

    #[test]
    fn cultist_attack_intent_deals_six_plus_strength() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));
        monster.powers.strength = 2;
        monster.intent = MonsterIntent::Attack { damage: 6 };

        assert_eq!(apply_intent(&mut monster), 8);
    }

    #[test]
    fn player_thorns_damage_attacking_monster() {
        let mut monster = monster_state(&CULTIST_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Attack { damage: 6 };
        let mut player = dummy_player();
        player.powers.thorns = 3;
        let mut piles = dummy_piles();
        let player_before = player.clone();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 6);
        assert_eq!(monster.hp, CULTIST_A0.hp - 3);
    }

    #[test]
    fn player_thorns_reflects_each_multi_attack_hit() {
        let mut monster = monster_state(&HEXAGHOST_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackMultiple { damage: 1, hits: 6 };
        let mut player = dummy_player();
        player.powers.thorns = 3;
        let mut piles = dummy_piles();
        let player_before = player.clone();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 6);
        assert_eq!(monster.hp, HEXAGHOST_A0.hp - 18);
    }

    #[test]
    fn player_artifact_blocks_monster_weak() {
        let mut monster = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak { amount: 1 };
        let mut player = dummy_player();
        player.powers.artifact = 1;
        let mut piles = dummy_piles();
        let player_before = player.clone();

        apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(player.powers.artifact, 0);
        assert_eq!(player.powers.weak, 0);
    }

    #[test]
    fn ginger_blocks_monster_weak_without_consuming_artifact() {
        let mut monster = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak { amount: 1 };
        let mut player = dummy_player();
        player.powers.artifact = 1;
        let mut piles = dummy_piles();
        let player_before = player.clone();

        apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[crate::Relic::Ginger],
        );

        assert_eq!(player.powers.artifact, 1);
        assert_eq!(player.powers.weak, 0);
    }

    #[test]
    fn player_artifact_blocks_one_lagavulin_siphon_debuff() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::SiphonPlayer {
            strength: 1,
            dexterity: 1,
        };
        let mut player = dummy_player();
        player.powers.artifact = 1;
        let mut piles = dummy_piles();
        let player_before = player.clone();

        apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(player.powers.artifact, 0);
        assert_eq!(player.powers.strength, 0);
        assert_eq!(player.powers.dexterity, -1);
    }

    #[test]
    fn fixed_simple_monster_always_attacks() {
        let monster = monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1));

        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&FIXED_SIMPLE_MONSTER, 5, None),
            MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage
            }
        );
    }

    #[test]
    fn jaw_worm_has_forty_two_hp() {
        assert_eq!(JAW_WORM_A0.hp, 42);
    }

    #[test]
    fn jaw_worm_starts_with_chomp_intent() {
        let monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
    }

    #[test]
    fn jaw_worm_move_selection_cycles_chomp_thrash_bellow() {
        let definition = &JAW_WORM_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::AttackAndBlock {
                damage: JAW_WORM_THRASH_DAMAGE,
                block: JAW_WORM_THRASH_BLOCK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
            MonsterIntent::StrengthAndBlock {
                strength: JAW_WORM_BELLOW_STRENGTH,
                block: JAW_WORM_BELLOW_BLOCK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 3, None),
            MonsterIntent::Attack {
                damage: JAW_WORM_CHOMP_DAMAGE
            }
        );
    }

    #[test]
    fn jaw_worm_chomp_deals_eleven_plus_strength() {
        let mut monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));
        monster.powers.strength = 3;
        monster.intent = MonsterIntent::Attack {
            damage: JAW_WORM_CHOMP_DAMAGE,
        };

        assert_eq!(apply_intent(&mut monster), 14);
    }

    #[test]
    fn jaw_worm_thrash_deals_damage_and_gains_block() {
        let mut monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackAndBlock {
            damage: JAW_WORM_THRASH_DAMAGE,
            block: JAW_WORM_THRASH_BLOCK,
        };

        assert_eq!(apply_intent(&mut monster), 7);
        assert_eq!(monster.block, 5);
    }

    #[test]
    fn jaw_worm_bellow_gains_strength_and_block() {
        let mut monster = monster_state(&JAW_WORM_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::StrengthAndBlock {
            strength: JAW_WORM_BELLOW_STRENGTH,
            block: JAW_WORM_BELLOW_BLOCK,
        };

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.powers.strength, 3);
        assert_eq!(monster.block, 6);
    }

    #[test]
    fn gremlin_nob_has_eighty_two_hp() {
        assert_eq!(GREMLIN_NOB_A0.hp, 82);
    }

    #[test]
    fn gremlin_nob_starts_with_bellow_intent() {
        let monster = monster_state(&GREMLIN_NOB_A0, MonsterId::new(1));

        assert_eq!(monster.intent, MonsterIntent::Block { block: 0 });
    }

    #[test]
    fn gremlin_nob_opens_bellow_then_skull_bash_then_rush() {
        let definition = &GREMLIN_NOB_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Block { block: 0 }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: GREMLIN_NOB_SKULL_BASH_DAMAGE,
                vulnerable: 2,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_RUSH_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 3, None),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_RUSH_DAMAGE
            }
        );
    }

    #[test]
    fn gremlin_nob_enrages_on_skill() {
        assert_eq!(GREMLIN_NOB_A0.enrage_weak_on_skill, 2);
    }

    #[test]
    fn red_louse_has_eleven_hp() {
        assert_eq!(RED_LOUSE_A0.hp, 11);
    }

    #[test]
    fn red_louse_starts_with_curl_intent() {
        let monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::StrengthAndBlock {
                strength: LOUSE_CURL_STRENGTH,
                block: 0
            }
        );
    }

    #[test]
    fn red_louse_move_selection_cycles_curl_bite() {
        let definition = &RED_LOUSE_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::StrengthAndBlock {
                strength: LOUSE_CURL_STRENGTH,
                block: 0
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::Attack {
                damage: RED_LOUSE_BITE_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
            MonsterIntent::StrengthAndBlock {
                strength: LOUSE_CURL_STRENGTH,
                block: 0
            }
        );
    }

    #[test]
    fn red_louse_curl_gains_strength_without_move_block() {
        let mut monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::StrengthAndBlock {
            strength: LOUSE_CURL_STRENGTH,
            block: 0,
        };

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.block, 0);
        assert_eq!(monster.powers.strength, 3);
    }

    #[test]
    fn red_louse_bite_deals_six_damage() {
        let mut monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Attack {
            damage: RED_LOUSE_BITE_DAMAGE,
        };

        assert_eq!(apply_intent(&mut monster), 6);
    }

    #[test]
    fn green_louse_has_twelve_hp_and_starting_spikes() {
        let monster = monster_state(&GREEN_LOUSE_A0, MonsterId::new(1));

        assert_eq!(GREEN_LOUSE_A0.hp, 12);
        assert_eq!(monster.powers.spikes, GREEN_LOUSE_SPIKES);
    }

    #[test]
    fn green_louse_move_selection_cycles_curl_bite() {
        let definition = &GREEN_LOUSE_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::StrengthAndBlock {
                strength: LOUSE_CURL_STRENGTH,
                block: 0
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::Attack {
                damage: GREEN_LOUSE_BITE_DAMAGE
            }
        );
    }

    #[test]
    fn spike_slime_has_fourteen_hp_and_spit_lick_cycle() {
        let definition = &SPIKE_SLIME_A0;

        assert_eq!(SPIKE_SLIME_A0.hp, 14);
        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Attack {
                damage: SPIKE_SLIME_S_SPIT_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::ApplyPlayerWeak {
                amount: SPIKE_SLIME_LICK_WEAK
            }
        );
    }

    #[test]
    fn spike_slime_lick_applies_weak_to_player() {
        let mut monster = monster_state(&SPIKE_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak {
            amount: SPIKE_SLIME_LICK_WEAK,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            apply_monster_intent(
                &mut monster,
                &mut player,
                &mut piles,
                0,
                &player_before,
                &[]
            ),
            0
        );
        assert_eq!(player.powers.weak, 1);
    }

    #[test]
    fn large_spike_slime_spit_adds_two_slimed_to_discard() {
        let mut monster = monster_state(&SPIKE_SLIME_A0, MonsterId::new(1));
        monster.hp = SPIKE_SLIME_M_A7_HP_RANGE.max + 1;
        monster.intent = prepare_monster_intent(&monster);
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            monster.intent,
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: SPIKE_SLIME_L_SPIT_DAMAGE,
                count: 2
            }
        );
        apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(
            piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == SLIMED_ID)
                .count(),
            2
        );
    }

    #[test]
    fn medium_spike_slime_lick_applies_frail_not_weak() {
        let mut monster = monster_state(&SPIKE_SLIME_A0, MonsterId::new(1));
        monster.hp = SPIKE_SLIME_M_A7_HP_RANGE.min;
        monster.moves_executed = 1;
        monster.intent = prepare_monster_intent(&monster);
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            monster.intent,
            MonsterIntent::ApplyPlayerFrailAndWeak { frail: 1, weak: 0 }
        );
        apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(player.powers.frail, 1);
        assert_eq!(player.powers.weak, 0);
    }

    #[test]
    fn medium_spike_slime_spit_adds_one_slimed_with_medium_damage() {
        let mut monster = monster_state(&SPIKE_SLIME_A0, MonsterId::new(1));
        monster.hp = SPIKE_SLIME_M_A7_HP_RANGE.min;
        monster.intent = prepare_monster_intent(&monster);

        assert_eq!(
            monster.intent,
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: SPIKE_SLIME_M_SPIT_DAMAGE,
                count: 1
            }
        );
    }

    #[test]
    fn acid_slime_has_twelve_hp_and_weak_attack_cycle() {
        let definition = &ACID_SLIME_A0;

        assert_eq!(ACID_SLIME_A0.hp, 12);
        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::ApplyPlayerWeak {
                amount: ACID_SLIME_WEAK
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::Attack {
                damage: ACID_SLIME_ATTACK_DAMAGE
            }
        );
    }

    #[test]
    fn acid_slime_wound_tackle_adds_slimed_to_discard() {
        let mut monster = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackAddSlimedToDiscard {
            damage: ACID_SLIME_ATTACK_DAMAGE,
            count: 1,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = CardPiles {
            hand: vec![],
            draw_pile: vec![],
            discard_pile: vec![],
            exhaust_pile: vec![],
        };

        assert_eq!(
            apply_monster_intent(
                &mut monster,
                &mut player,
                &mut piles,
                0,
                &player_before,
                &[]
            ),
            ACID_SLIME_ATTACK_DAMAGE
        );
        assert_eq!(piles.discard_pile.len(), 1);
        assert_eq!(piles.discard_pile[0].content_id, SLIMED_ID);
    }

    #[test]
    fn medium_acid_slime_entry_roll_matches_target_move_table() {
        let hp = ACID_SLIME_M_A0_HP_RANGE.max;

        assert_eq!(
            target_acid_slime_entry_intent_from_roll(hp, 0),
            MonsterIntent::AttackAddSlimedToDiscard {
                damage: ACID_SLIME_ATTACK_DAMAGE,
                count: 1
            }
        );
        assert_eq!(
            target_acid_slime_entry_intent_from_roll(hp, 30),
            MonsterIntent::Attack {
                damage: ACID_SLIME_M_NORMAL_TACKLE_DAMAGE
            }
        );
        assert_eq!(
            target_acid_slime_entry_intent_from_roll(hp, 70),
            MonsterIntent::ApplyPlayerWeak {
                amount: ACID_SLIME_WEAK
            }
        );
    }

    #[test]
    fn medium_acid_slime_next_roll_matches_target_move_table() {
        let mut rng = StsRng::new(1);

        assert_eq!(
            target_medium_acid_slime_next_intent_from_roll(&[1], 30, &mut rng, 0),
            MonsterIntent::Attack {
                damage: ACID_SLIME_M_NORMAL_TACKLE_DAMAGE
            }
        );
        assert_eq!(
            target_medium_acid_slime_next_intent_from_roll(&[1], 70, &mut rng, 0),
            MonsterIntent::ApplyPlayerWeak {
                amount: ACID_SLIME_WEAK
            }
        );
    }

    #[test]
    fn acid_slime_records_target_move_bytes() {
        let mut monster = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        monster.hp = ACID_SLIME_M_A0_HP_RANGE.max;
        monster.intent = MonsterIntent::AttackAddSlimedToDiscard {
            damage: ACID_SLIME_ATTACK_DAMAGE,
            count: 1,
        };
        record_target_move(&mut monster);
        monster.intent = MonsterIntent::Attack {
            damage: ACID_SLIME_M_NORMAL_TACKLE_DAMAGE,
        };
        record_target_move(&mut monster);
        monster.intent = MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        };
        record_target_move(&mut monster);

        assert_eq!(monster.move_history, vec![1, 2, 4]);
    }

    #[test]
    fn acid_slime_weak_applies_weak_to_player() {
        let mut monster = monster_state(&ACID_SLIME_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeak {
            amount: ACID_SLIME_WEAK,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            apply_monster_intent(
                &mut monster,
                &mut player,
                &mut piles,
                0,
                &player_before,
                &[]
            ),
            0
        );
        assert_eq!(player.powers.weak, 1);
    }

    #[test]
    fn content_id_from_game_id_maps_elite_monsters() {
        assert_eq!(content_id_from_game_monster_id("Jaw Worm"), JAW_WORM_ID);
        assert_eq!(content_id_from_game_monster_id("Lagavulin"), LAGAVULIN_ID);
        assert_eq!(
            content_id_from_game_monster_id("GremlinNob"),
            GREMLIN_NOB_ID
        );
        assert_eq!(content_id_from_game_monster_id("TheGuardian"), GUARDIAN_ID);
        assert_eq!(
            content_id_from_game_monster_id("SphericGuardian"),
            SPHERIC_GUARDIAN_ID
        );
    }

    #[test]
    fn content_id_from_game_id_maps_large_slimes() {
        assert_eq!(
            content_id_from_game_monster_id("SpikeSlime_L"),
            SPIKE_SLIME_ID
        );
        assert_eq!(
            content_id_from_game_monster_id("Acid Slime (L)"),
            ACID_SLIME_ID
        );
    }

    #[test]
    fn spheric_guardian_definition_is_registered() {
        let definition =
            get_monster_definition(SPHERIC_GUARDIAN_ID).expect("spheric guardian definition");

        assert_eq!(definition.name, "Spheric Guardian");
        assert_eq!(definition.hp, SPHERIC_GUARDIAN_HP_RANGE.min);
        assert_eq!(definition.attack_damage, SPHERIC_GUARDIAN_DAMAGE);
    }

    #[test]
    fn mugger_definition_is_registered() {
        assert_eq!(content_id_from_game_monster_id("Mugger"), MUGGER_ID);

        let definition = get_monster_definition(MUGGER_ID).expect("mugger definition");

        assert_eq!(definition.name, "Mugger");
        assert_eq!(definition.hp, 50);
    }

    #[test]
    fn mugger_representative_sequence_covers_two_mugs_big_swipe_smoke_bomb_and_escape() {
        assert_eq!(
            prepare_monster_intent_for(&MUGGER_A0, 0, None),
            MonsterIntent::AttackStealGold {
                damage: MUGGER_SWIPE_DAMAGE,
                amount: MUGGER_THEFT,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&MUGGER_A0, 1, None),
            MonsterIntent::AttackStealGold {
                damage: MUGGER_SWIPE_DAMAGE,
                amount: MUGGER_THEFT,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&MUGGER_A0, 2, None),
            MonsterIntent::AttackStealGold {
                damage: MUGGER_BIG_SWIPE_DAMAGE,
                amount: MUGGER_THEFT,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&MUGGER_A0, 3, None),
            MonsterIntent::Block {
                block: MUGGER_ESCAPE_BLOCK
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&MUGGER_A0, 4, None),
            MonsterIntent::Escape
        );
    }

    #[test]
    fn mugger_ascension_variants_use_source_backed_damage_theft_and_escape_block() {
        let mut mugger = monster_state_for_ascension(&MUGGER_A0, MonsterId::new(1), 17);

        assert_eq!(
            prepare_monster_intent_for_ascension(&mugger, 17),
            MonsterIntent::AttackStealGold {
                damage: MUGGER_A2_SWIPE_DAMAGE,
                amount: MUGGER_A17_THEFT,
            }
        );
        mugger.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&mugger, 17),
            MonsterIntent::AttackStealGold {
                damage: MUGGER_A2_BIG_SWIPE_DAMAGE,
                amount: MUGGER_A17_THEFT,
            }
        );
        mugger.moves_executed = 3;
        assert_eq!(
            prepare_monster_intent_for_ascension(&mugger, 17),
            MonsterIntent::Block {
                block: MUGGER_A17_ESCAPE_BLOCK,
            }
        );
    }

    #[test]
    fn mugger_theft_attack_steals_gold_and_deals_damage() {
        let mut monster = monster_state(&MUGGER_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, MUGGER_SWIPE_DAMAGE);
        assert_eq!(monster.stolen_gold, MUGGER_THEFT);
    }

    #[test]
    fn mugger_source_scaled_big_swipe_does_not_double_apply_ascension_bonus() {
        let mut monster = monster_state_for_ascension(&MUGGER_A0, MonsterId::new(1), 2);
        monster.intent = MonsterIntent::AttackStealGold {
            damage: MUGGER_A2_BIG_SWIPE_DAMAGE,
            amount: MUGGER_THEFT,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            2,
            &player_before,
            &[],
        );

        assert_eq!(damage, MUGGER_A2_BIG_SWIPE_DAMAGE);
        assert_eq!(monster.stolen_gold, MUGGER_THEFT);
    }

    #[test]
    fn mugger_escape_marks_monster_escaped_and_not_alive() {
        let mut monster = monster_state(&MUGGER_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Escape;
        monster.block = MUGGER_ESCAPE_BLOCK;
        monster.stolen_gold = MUGGER_THEFT;
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 0);
        assert!(!monster.alive);
        assert!(monster.escaped);
        assert_eq!(monster.block, 0);
        assert_eq!(monster.stolen_gold, MUGGER_THEFT);
    }

    #[test]
    fn chosen_definition_is_registered() {
        assert_eq!(content_id_from_game_monster_id("Chosen"), CHOSEN_ID);

        let definition = get_monster_definition(CHOSEN_ID).expect("chosen definition");

        assert_eq!(definition.name, "Chosen");
        assert_eq!(definition.hp, 97);
    }

    #[test]
    fn chosen_representative_sequence_covers_poke_hex_debilitate_drain_and_zap() {
        assert_eq!(
            prepare_monster_intent_for(&CHOSEN_A0, 0, None),
            MonsterIntent::AttackMultiple {
                damage: CHOSEN_POKE_DAMAGE,
                hits: CHOSEN_POKE_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&CHOSEN_A0, 1, None),
            MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX }
        );
        assert_eq!(
            prepare_monster_intent_for(&CHOSEN_A0, 2, None),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: CHOSEN_DEBILITATE_DAMAGE,
                vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&CHOSEN_A0, 3, None),
            MonsterIntent::ApplyPlayerWeakStrengthSelf {
                weak: CHOSEN_DRAIN_WEAK,
                strength: CHOSEN_DRAIN_STRENGTH,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&CHOSEN_A0, 4, None),
            MonsterIntent::Attack {
                damage: CHOSEN_ZAP_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&CHOSEN_A0, 5, None),
            MonsterIntent::AttackMultiple {
                damage: CHOSEN_POKE_DAMAGE,
                hits: CHOSEN_POKE_HITS,
            }
        );
    }

    #[test]
    fn chosen_ascension_variants_use_source_backed_damage_and_a17_hex_open() {
        let mut chosen = monster_state_for_ascension(&CHOSEN_A0, MonsterId::new(1), 2);
        assert_eq!(
            prepare_monster_intent_for_ascension(&chosen, 2),
            MonsterIntent::AttackMultiple {
                damage: CHOSEN_A2_POKE_DAMAGE,
                hits: CHOSEN_POKE_HITS,
            }
        );
        chosen.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&chosen, 2),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: CHOSEN_A2_DEBILITATE_DAMAGE,
                vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
            }
        );
        chosen.moves_executed = 4;
        assert_eq!(
            prepare_monster_intent_for_ascension(&chosen, 2),
            MonsterIntent::Attack {
                damage: CHOSEN_A2_ZAP_DAMAGE,
            }
        );

        let mut chosen = monster_state_for_ascension(&CHOSEN_A0, MonsterId::new(1), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&chosen, 17),
            MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX }
        );
        chosen.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&chosen, 17),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: CHOSEN_A2_DEBILITATE_DAMAGE,
                vulnerable: CHOSEN_DEBILITATE_VULNERABLE,
            }
        );
    }

    #[test]
    fn chosen_source_scaled_attack_damage_does_not_double_apply_ascension_bonus() {
        let mut monster = monster_state_for_ascension(&CHOSEN_A0, MonsterId::new(1), 17);
        monster.intent = MonsterIntent::Attack {
            damage: CHOSEN_A2_ZAP_DAMAGE,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            17,
            &player_before,
            &[],
        );

        assert_eq!(damage, CHOSEN_A2_ZAP_DAMAGE);
    }

    #[test]
    fn chosen_hex_intent_applies_player_hex_power() {
        let mut monster = monster_state(&CHOSEN_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerHex { amount: CHOSEN_HEX };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 0);
        assert_eq!(player.powers.hex, CHOSEN_HEX);
    }

    #[test]
    fn chosen_drain_applies_player_weak_and_self_strength() {
        let mut monster = monster_state(&CHOSEN_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerWeakStrengthSelf {
            weak: CHOSEN_DRAIN_WEAK,
            strength: CHOSEN_DRAIN_STRENGTH,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 0);
        assert_eq!(player.powers.weak, CHOSEN_DRAIN_WEAK);
        assert_eq!(monster.powers.strength, CHOSEN_DRAIN_STRENGTH);
    }

    #[test]
    fn snake_plant_definition_is_registered_with_malleable() {
        assert_eq!(
            content_id_from_game_monster_id("SnakePlant"),
            SNAKE_PLANT_ID
        );

        let definition = get_monster_definition(SNAKE_PLANT_ID).expect("snake plant definition");
        let monster = monster_state(definition, MonsterId::new(1));

        assert_eq!(definition.name, "Snake Plant");
        assert_eq!(definition.hp, 77);
        assert_eq!(monster.powers.malleable, SNAKE_PLANT_MALLEABLE);
        assert_eq!(monster.powers.malleable_base, SNAKE_PLANT_MALLEABLE);
    }

    #[test]
    fn snake_plant_partial_intents_cover_chompy_and_spores() {
        assert_eq!(
            prepare_monster_intent_for(&SNAKE_PLANT_A0, 0, None),
            MonsterIntent::AttackMultiple {
                damage: SNAKE_PLANT_CHOMPY_DAMAGE,
                hits: SNAKE_PLANT_CHOMPY_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SNAKE_PLANT_A0, 1, None),
            MonsterIntent::ApplyPlayerFrailAndWeak {
                frail: SNAKE_PLANT_SPORES_DEBUFF,
                weak: SNAKE_PLANT_SPORES_DEBUFF,
            }
        );
    }

    #[test]
    fn snake_plant_a2_chompy_damage_is_source_backed_and_not_double_scaled() {
        let mut monster = monster_state_for_ascension(&SNAKE_PLANT_A0, MonsterId::new(1), 2);
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 2),
            MonsterIntent::AttackMultiple {
                damage: SNAKE_PLANT_A2_CHOMPY_DAMAGE,
                hits: SNAKE_PLANT_CHOMPY_HITS,
            }
        );
        monster.intent = prepare_monster_intent_for_ascension(&monster, 2);
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            2,
            &player_before,
            &[],
        );

        assert_eq!(
            damage,
            SNAKE_PLANT_A2_CHOMPY_DAMAGE * SNAKE_PLANT_CHOMPY_HITS
        );
    }

    #[test]
    fn snake_plant_spores_apply_frail_and_weak() {
        let mut monster = monster_state(&SNAKE_PLANT_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::ApplyPlayerFrailAndWeak {
            frail: SNAKE_PLANT_SPORES_DEBUFF,
            weak: SNAKE_PLANT_SPORES_DEBUFF,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 0);
        assert_eq!(player.powers.frail, SNAKE_PLANT_SPORES_DEBUFF);
        assert_eq!(player.powers.weak, SNAKE_PLANT_SPORES_DEBUFF);
    }

    #[test]
    fn snecko_definition_is_registered() {
        assert_eq!(content_id_from_game_monster_id("Snecko"), SNECKO_ID);

        let definition = get_monster_definition(SNECKO_ID).expect("snecko definition");

        assert_eq!(definition.name, "Snecko");
        assert_eq!(definition.hp, 117);
    }

    #[test]
    fn snecko_representative_sequence_covers_glare_tail_whip_and_bite() {
        assert_eq!(
            prepare_monster_intent_for(&SNECKO_A0, 0, None),
            MonsterIntent::ApplyPlayerConfusion
        );
        assert_eq!(
            prepare_monster_intent_for(&SNECKO_A0, 1, None),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: SNECKO_TAIL_DAMAGE,
                vulnerable: SNECKO_VULNERABLE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SNECKO_A0, 2, None),
            MonsterIntent::Attack {
                damage: SNECKO_BITE_DAMAGE,
            }
        );
    }

    #[test]
    fn snecko_ascension_tail_whip_and_bite_variants_are_source_backed() {
        let mut monster = monster_state_for_ascension(&SNECKO_A0, MonsterId::new(1), 2);
        monster.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 2),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: SNECKO_A2_TAIL_DAMAGE,
                vulnerable: SNECKO_VULNERABLE,
            }
        );

        monster.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 2),
            MonsterIntent::Attack {
                damage: SNECKO_A2_BITE_DAMAGE,
            }
        );

        monster.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 17),
            MonsterIntent::AttackApplyPlayerWeakAndVulnerable {
                damage: SNECKO_A2_TAIL_DAMAGE,
                weak: SNECKO_A17_WEAK,
                vulnerable: SNECKO_VULNERABLE,
            }
        );
    }

    #[test]
    fn snecko_a17_tail_whip_applies_weak_vulnerable_and_source_scaled_damage() {
        let mut monster = monster_state_for_ascension(&SNECKO_A0, MonsterId::new(1), 17);
        monster.moves_executed = 1;
        monster.intent = prepare_monster_intent_for_ascension(&monster, 17);
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            17,
            &player_before,
            &[],
        );

        assert_eq!(damage, SNECKO_A2_TAIL_DAMAGE);
        assert_eq!(player.powers.weak, SNECKO_A17_WEAK);
        assert_eq!(player.powers.vulnerable, SNECKO_VULNERABLE);
    }

    #[test]
    fn snecko_confusion_intent_applies_player_confusion_and_respects_artifact() {
        let mut monster = monster_state(&SNECKO_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, 0);
        assert_eq!(player.powers.confusion, 1);

        let mut player_with_artifact = dummy_player();
        player_with_artifact.powers.artifact = 1;
        let before_artifact = player_with_artifact.clone();
        let mut monster = monster_state(&SNECKO_A0, MonsterId::new(1));

        apply_monster_intent(
            &mut monster,
            &mut player_with_artifact,
            &mut piles,
            0,
            &before_artifact,
            &[],
        );

        assert_eq!(player_with_artifact.powers.confusion, 0);
        assert_eq!(player_with_artifact.powers.artifact, 0);
    }

    #[test]
    fn centurion_and_healer_definitions_are_registered() {
        assert_eq!(content_id_from_game_monster_id("Centurion"), CENTURION_ID);
        assert_eq!(content_id_from_game_monster_id("Healer"), HEALER_ID);

        let centurion = get_monster_definition(CENTURION_ID).expect("centurion definition");
        let healer = get_monster_definition(HEALER_ID).expect("healer definition");

        assert_eq!(centurion.name, "Centurion");
        assert_eq!(centurion.hp, 78);
        assert_eq!(healer.name, "Mystic");
        assert_eq!(healer.hp, 52);
    }

    #[test]
    fn centurion_partial_sequence_covers_slash_protect_and_fury() {
        assert_eq!(
            prepare_monster_intent_for(&CENTURION_A0, 0, None),
            MonsterIntent::Attack {
                damage: CENTURION_SLASH_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&CENTURION_A0, 2, None),
            MonsterIntent::Block {
                block: CENTURION_BLOCK
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&CENTURION_A0, 3, None),
            MonsterIntent::AttackMultiple {
                damage: CENTURION_FURY_DAMAGE,
                hits: CENTURION_FURY_HITS,
            }
        );
    }

    #[test]
    fn healer_partial_sequence_covers_buff_attack_and_heal() {
        assert_eq!(
            prepare_monster_intent_for(&HEALER_A0, 0, None),
            MonsterIntent::StrengthAllMonsters {
                amount: HEALER_STRENGTH
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&HEALER_A0, 1, None),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: HEALER_ATTACK_DAMAGE,
                frail: HEALER_FRAIL,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&HEALER_A0, 2, None),
            MonsterIntent::HealAllMonsters {
                amount: HEALER_HEAL
            }
        );
    }

    #[test]
    fn centurion_and_healer_ascension_variants_are_source_backed() {
        let mut centurion = monster_state_for_ascension(&CENTURION_A0, MonsterId::new(1), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&centurion, 17),
            MonsterIntent::Attack {
                damage: CENTURION_A2_SLASH_DAMAGE,
            }
        );
        centurion.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&centurion, 17),
            MonsterIntent::Block {
                block: CENTURION_A17_BLOCK,
            }
        );
        centurion.moves_executed = 3;
        assert_eq!(
            prepare_monster_intent_for_ascension(&centurion, 17),
            MonsterIntent::AttackMultiple {
                damage: CENTURION_A2_FURY_DAMAGE,
                hits: CENTURION_FURY_HITS,
            }
        );

        let mut healer = monster_state_for_ascension(&HEALER_A0, MonsterId::new(2), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&healer, 17),
            MonsterIntent::StrengthAllMonsters {
                amount: HEALER_A17_STRENGTH,
            }
        );
        healer.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&healer, 17),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: HEALER_A2_ATTACK_DAMAGE,
                frail: HEALER_FRAIL,
            }
        );
        healer.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&healer, 17),
            MonsterIntent::HealAllMonsters {
                amount: HEALER_A17_HEAL,
            }
        );
    }

    #[test]
    fn centurion_and_healer_source_scaled_attacks_do_not_double_apply_ascension_bonus() {
        let mut centurion = monster_state_for_ascension(&CENTURION_A0, MonsterId::new(1), 17);
        centurion.intent = MonsterIntent::Attack {
            damage: CENTURION_A2_SLASH_DAMAGE,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut centurion,
            &mut player,
            &mut piles,
            17,
            &player_before,
            &[],
        );
        assert_eq!(damage, CENTURION_A2_SLASH_DAMAGE);

        let mut healer = monster_state_for_ascension(&HEALER_A0, MonsterId::new(2), 17);
        healer.intent = MonsterIntent::AttackApplyPlayerFrail {
            damage: HEALER_A2_ATTACK_DAMAGE,
            frail: HEALER_FRAIL,
        };
        let mut player = dummy_player();
        let player_before = player.clone();

        let damage = apply_monster_intent(
            &mut healer,
            &mut player,
            &mut piles,
            17,
            &player_before,
            &[],
        );
        assert_eq!(damage, HEALER_A2_ATTACK_DAMAGE);
        assert_eq!(player.powers.frail, HEALER_FRAIL);
    }

    #[test]
    fn healer_group_effects_apply_to_living_monsters_and_cap_healing() {
        let mut monsters = vec![
            monster_state(&CENTURION_A0, MonsterId::new(1)),
            monster_state(&HEALER_A0, MonsterId::new(2)),
        ];
        monsters[0].hp = 60;
        monsters[1].hp = 50;

        apply_strength_all_monsters(&mut monsters, HEALER_STRENGTH);

        assert_eq!(monsters[0].powers.strength, HEALER_STRENGTH);
        assert_eq!(monsters[1].powers.strength, HEALER_STRENGTH);

        apply_heal_all_monsters(&mut monsters, 0, HEALER_HEAL);

        assert_eq!(monsters[0].hp, 76);
        assert_eq!(monsters[1].hp, 52);
    }

    #[test]
    fn healer_profile_uses_source_backed_a17_heal_and_strength() {
        let profile = target_city_monster_profile("Healer", 17).expect("healer profile");

        assert!(profile.constants.contains(&TargetMonsterConstant {
            name: "heal",
            value: 20,
        }));
        assert!(profile.constants.contains(&TargetMonsterConstant {
            name: "strength",
            value: 4,
        }));
    }

    #[test]
    fn byrd_definition_is_registered_with_flight() {
        assert_eq!(content_id_from_game_monster_id("Byrd"), BYRD_ID);

        let definition = get_monster_definition(BYRD_ID).expect("byrd definition");
        let monster = monster_state(&BYRD_A0, MonsterId::new(1));

        assert_eq!(definition.name, "Byrd");
        assert_eq!(definition.hp, 28);
        assert_eq!(monster.powers.flight, BYRD_FLIGHT);
        assert_eq!(
            monster.intent,
            MonsterIntent::AttackMultiple {
                damage: BYRD_PECK_DAMAGE,
                hits: BYRD_PECK_HITS,
            }
        );
    }

    #[test]
    fn byrd_partial_sequence_covers_peck_caw_swoop_and_headbutt() {
        assert_eq!(
            prepare_monster_intent_for(&BYRD_A0, 0, None),
            MonsterIntent::AttackMultiple {
                damage: BYRD_PECK_DAMAGE,
                hits: BYRD_PECK_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BYRD_A0, 1, None),
            MonsterIntent::StrengthSelf {
                amount: BYRD_CAW_STRENGTH
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BYRD_A0, 2, None),
            MonsterIntent::Attack {
                damage: BYRD_SWOOP_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BYRD_A0, 3, None),
            MonsterIntent::Attack {
                damage: BYRD_HEADBUTT_DAMAGE
            }
        );
    }

    #[test]
    fn byrd_ascension_variants_use_source_backed_flight_peck_and_swoop() {
        let mut byrd = monster_state_for_ascension(&BYRD_A0, MonsterId::new(1), 17);

        assert_eq!(byrd.powers.flight, BYRD_A17_FLIGHT);
        assert_eq!(
            prepare_monster_intent_for_ascension(&byrd, 17),
            MonsterIntent::AttackMultiple {
                damage: BYRD_PECK_DAMAGE,
                hits: BYRD_A2_PECK_HITS,
            }
        );
        byrd.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&byrd, 17),
            MonsterIntent::StrengthSelf {
                amount: BYRD_CAW_STRENGTH,
            }
        );
        byrd.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&byrd, 17),
            MonsterIntent::Attack {
                damage: BYRD_A2_SWOOP_DAMAGE,
            }
        );
        byrd.moves_executed = 3;
        assert_eq!(
            prepare_monster_intent_for_ascension(&byrd, 17),
            MonsterIntent::Attack {
                damage: BYRD_HEADBUTT_DAMAGE,
            }
        );
    }

    #[test]
    fn byrd_source_scaled_peck_does_not_double_apply_ascension_bonus() {
        let mut byrd = monster_state_for_ascension(&BYRD_A0, MonsterId::new(1), 2);
        byrd.intent = MonsterIntent::AttackMultiple {
            damage: BYRD_PECK_DAMAGE,
            hits: BYRD_A2_PECK_HITS,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage =
            apply_monster_intent(&mut byrd, &mut player, &mut piles, 2, &player_before, &[]);

        assert_eq!(damage, BYRD_PECK_DAMAGE * BYRD_A2_PECK_HITS);
    }

    #[test]
    fn shelled_parasite_definition_is_registered_with_plated_armor_and_block() {
        assert_eq!(
            content_id_from_game_monster_id("ShelledParasite"),
            SHELLED_PARASITE_ID
        );
        assert_eq!(
            content_id_from_game_monster_id("Shelled Parasite"),
            SHELLED_PARASITE_ID
        );

        let definition =
            get_monster_definition(SHELLED_PARASITE_ID).expect("shelled parasite definition");
        let monster = monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1));

        assert_eq!(definition.name, "Shelled Parasite");
        assert_eq!(definition.hp, 70);
        assert_eq!(monster.block, SHELLED_PARASITE_PLATED_ARMOR);
        assert_eq!(monster.powers.plated_armor, SHELLED_PARASITE_PLATED_ARMOR);
    }

    #[test]
    fn shelled_parasite_partial_sequence_covers_double_strike_life_suck_and_fell() {
        assert_eq!(
            prepare_monster_intent_for(&SHELLED_PARASITE_A0, 0, None),
            MonsterIntent::AttackMultiple {
                damage: SHELLED_PARASITE_DOUBLE_STRIKE_DAMAGE,
                hits: SHELLED_PARASITE_DOUBLE_STRIKE_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SHELLED_PARASITE_A0, 1, None),
            MonsterIntent::AttackHealSelf {
                damage: SHELLED_PARASITE_SUCK_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SHELLED_PARASITE_A0, 2, None),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SHELLED_PARASITE_FELL_DAMAGE,
                frail: SHELLED_PARASITE_FELL_FRAIL,
            }
        );
    }

    #[test]
    fn shelled_parasite_a2_damage_variants_are_source_backed() {
        let mut monster = monster_state_for_ascension(&SHELLED_PARASITE_A0, MonsterId::new(1), 2);
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 2),
            MonsterIntent::AttackMultiple {
                damage: SHELLED_PARASITE_A2_DOUBLE_STRIKE_DAMAGE,
                hits: SHELLED_PARASITE_DOUBLE_STRIKE_HITS,
            }
        );
        monster.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 2),
            MonsterIntent::AttackHealSelf {
                damage: SHELLED_PARASITE_A2_SUCK_DAMAGE,
            }
        );
        monster.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 2),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SHELLED_PARASITE_A2_FELL_DAMAGE,
                frail: SHELLED_PARASITE_FELL_FRAIL,
            }
        );
    }

    #[test]
    fn shelled_parasite_a17_opens_with_source_backed_fell() {
        let monster = monster_state_for_ascension(&SHELLED_PARASITE_A0, MonsterId::new(1), 17);

        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 17),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SHELLED_PARASITE_A2_FELL_DAMAGE,
                frail: SHELLED_PARASITE_FELL_FRAIL,
            }
        );
    }

    #[test]
    fn shelled_parasite_source_scaled_fell_damage_does_not_double_apply_ascension_bonus() {
        let mut monster = monster_state_for_ascension(&SHELLED_PARASITE_A0, MonsterId::new(1), 2);
        monster.intent = MonsterIntent::AttackApplyPlayerFrail {
            damage: SHELLED_PARASITE_A2_FELL_DAMAGE,
            frail: SHELLED_PARASITE_FELL_FRAIL,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            2,
            &player_before,
            &[],
        );

        assert_eq!(damage, SHELLED_PARASITE_A2_FELL_DAMAGE);
        assert_eq!(player.powers.frail, SHELLED_PARASITE_FELL_FRAIL);
    }

    #[test]
    fn shelled_parasite_plated_armor_gains_block_and_decrements_on_hp_damage() {
        let mut monster = monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1));
        monster.block = 0;

        crate::combat::turn_powers::apply_end_of_monster_turn_powers(&mut monster);

        assert_eq!(monster.block, SHELLED_PARASITE_PLATED_ARMOR);

        let hp_damage = crate::combat::damage::deal_unmodified_damage_to_monster(
            &mut monster,
            SHELLED_PARASITE_PLATED_ARMOR + 1,
        );

        assert_eq!(hp_damage, 1);
        assert_eq!(
            monster.powers.plated_armor,
            SHELLED_PARASITE_PLATED_ARMOR - 1
        );
    }

    #[test]
    fn shelled_parasite_life_suck_intent_reports_attack_damage() {
        let mut monster = monster_state(&SHELLED_PARASITE_A0, MonsterId::new(1));
        monster.hp = 60;
        monster.block = 0;
        monster.intent = MonsterIntent::AttackHealSelf {
            damage: SHELLED_PARASITE_SUCK_DAMAGE,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, SHELLED_PARASITE_SUCK_DAMAGE);
        assert_eq!(monster.hp, 60);
    }

    #[test]
    fn book_of_stabbing_definition_is_registered_with_painful_stabs() {
        assert_eq!(
            content_id_from_game_monster_id("BookOfStabbing"),
            BOOK_OF_STABBING_ID
        );
        assert_eq!(
            content_id_from_game_monster_id("Book of Stabbing"),
            BOOK_OF_STABBING_ID
        );

        let definition =
            get_monster_definition(BOOK_OF_STABBING_ID).expect("book of stabbing definition");
        let monster = monster_state(&BOOK_OF_STABBING_A0, MonsterId::new(1));

        assert_eq!(definition.name, "Book of Stabbing");
        assert_eq!(definition.hp, 162);
        assert_eq!(monster.powers.painful_stabs, BOOK_OF_STABBING_PAINFUL_STABS);
    }

    #[test]
    fn book_of_stabbing_partial_sequence_grows_stabs_then_big_stabs() {
        assert_eq!(
            prepare_monster_intent_for(&BOOK_OF_STABBING_A0, 0, None),
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_STAB_DAMAGE,
                hits: 2,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BOOK_OF_STABBING_A0, 1, None),
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_STAB_DAMAGE,
                hits: 3,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BOOK_OF_STABBING_A0, 2, None),
            MonsterIntent::Attack {
                damage: BOOK_OF_STABBING_BIG_STAB_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BOOK_OF_STABBING_A0, 3, None),
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_STAB_DAMAGE,
                hits: 4,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&BOOK_OF_STABBING_A0, 4, None),
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_STAB_DAMAGE,
                hits: 5,
            }
        );
    }

    #[test]
    fn book_of_stabbing_attack_intents_deal_expected_a0_damage() {
        let mut monster = monster_state(&BOOK_OF_STABBING_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        monster.intent = MonsterIntent::AttackMultiple {
            damage: BOOK_OF_STABBING_STAB_DAMAGE,
            hits: 2,
        };
        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, BOOK_OF_STABBING_STAB_DAMAGE * 2);

        let mut monster = monster_state(&BOOK_OF_STABBING_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Attack {
            damage: BOOK_OF_STABBING_BIG_STAB_DAMAGE,
        };
        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, BOOK_OF_STABBING_BIG_STAB_DAMAGE);
    }

    #[test]
    fn book_of_stabbing_ascension_variants_use_source_backed_damage_and_stab_growth() {
        let monster = monster_state_for_ascension(&BOOK_OF_STABBING_A0, MonsterId::new(1), 3);
        assert_eq!(
            monster.intent,
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_A3_STAB_DAMAGE,
                hits: 2,
            }
        );

        assert_eq!(
            prepare_monster_intent_for_ascension(
                &MonsterState {
                    moves_executed: 2,
                    ..monster.clone()
                },
                3,
            ),
            MonsterIntent::Attack {
                damage: BOOK_OF_STABBING_A3_BIG_STAB_DAMAGE,
            }
        );

        assert_eq!(
            prepare_monster_intent_for_ascension(
                &MonsterState {
                    moves_executed: 3,
                    ..monster.clone()
                },
                18,
            ),
            MonsterIntent::AttackMultiple {
                damage: BOOK_OF_STABBING_A3_STAB_DAMAGE,
                hits: 5,
            }
        );
    }

    #[test]
    fn fungi_beast_definition_is_registered_with_spore_cloud() {
        assert_eq!(
            content_id_from_game_monster_id("FungiBeast"),
            FUNGI_BEAST_ID
        );
        let definition = get_monster_definition(FUNGI_BEAST_ID).expect("fungi definition");
        let monster = monster_state(&FUNGI_BEAST_A0, MonsterId::new(1));

        assert_eq!(definition.name, "Fungi Beast");
        assert_eq!(definition.hp, 25);
        assert_eq!(monster.powers.spore_cloud, FUNGI_BEAST_SPORE_CLOUD);
    }

    #[test]
    fn fungi_beast_partial_sequence_covers_bite_then_grow() {
        assert_eq!(
            prepare_monster_intent_for(&FUNGI_BEAST_A0, 0, None),
            MonsterIntent::Attack {
                damage: FUNGI_BEAST_BITE_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&FUNGI_BEAST_A0, 1, None),
            MonsterIntent::StrengthSelf {
                amount: FUNGI_BEAST_GROW_STRENGTH,
            }
        );
    }

    #[test]
    fn slaver_definitions_and_partial_sequences_are_registered() {
        assert_eq!(
            content_id_from_game_monster_id("SlaverBlue"),
            SLAVER_BLUE_ID
        );
        assert_eq!(content_id_from_game_monster_id("SlaverRed"), SLAVER_RED_ID);

        assert_eq!(
            prepare_monster_intent_for(&SLAVER_BLUE_A0, 0, None),
            MonsterIntent::AttackApplyPlayerWeak {
                damage: SLAVER_BLUE_RAKE_DAMAGE,
                weak: SLAVER_BLUE_WEAK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SLAVER_BLUE_A0, 1, None),
            MonsterIntent::Attack {
                damage: SLAVER_BLUE_STAB_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SLAVER_RED_A0, 0, None),
            MonsterIntent::Attack {
                damage: SLAVER_RED_STAB_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SLAVER_RED_A0, 1, None),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: SLAVER_RED_SCRAPE_DAMAGE,
                vulnerable: SLAVER_RED_VULNERABLE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&SLAVER_RED_A0, 2, None),
            MonsterIntent::ApplyPlayerEntangled {
                amount: SLAVER_RED_ENTANGLED,
            }
        );
    }

    #[test]
    fn slaver_debuff_intents_apply_weak_and_entangled() {
        let mut blue = monster_state(&SLAVER_BLUE_A0, MonsterId::new(1));
        blue.intent = MonsterIntent::AttackApplyPlayerWeak {
            damage: SLAVER_BLUE_RAKE_DAMAGE,
            weak: SLAVER_BLUE_WEAK,
        };
        let mut player = dummy_player();
        let before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(&mut blue, &mut player, &mut piles, 0, &before, &[]);

        assert_eq!(damage, SLAVER_BLUE_RAKE_DAMAGE);
        assert_eq!(player.powers.weak, SLAVER_BLUE_WEAK);

        let mut red = monster_state(&SLAVER_RED_A0, MonsterId::new(1));
        red.intent = MonsterIntent::ApplyPlayerEntangled {
            amount: SLAVER_RED_ENTANGLED,
        };
        let before = player.clone();

        let damage = apply_monster_intent(&mut red, &mut player, &mut piles, 0, &before, &[]);

        assert_eq!(damage, 0);
        assert_eq!(player.powers.entangled, SLAVER_RED_ENTANGLED);
    }

    #[test]
    fn fungi_and_slaver_ascension_variants_are_source_backed() {
        let mut fungi = monster_state_for_ascension(&FUNGI_BEAST_A0, MonsterId::new(1), 17);
        fungi.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&fungi, 17),
            MonsterIntent::StrengthSelf {
                amount: FUNGI_BEAST_A2_GROW_STRENGTH + FUNGI_BEAST_A17_GROW_BONUS,
            }
        );

        let blue = monster_state_for_ascension(&SLAVER_BLUE_A0, MonsterId::new(2), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&blue, 17),
            MonsterIntent::AttackApplyPlayerWeak {
                damage: SLAVER_BLUE_A2_RAKE_DAMAGE,
                weak: SLAVER_BLUE_A17_WEAK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for_ascension(
                &MonsterState {
                    moves_executed: 1,
                    ..blue
                },
                17,
            ),
            MonsterIntent::Attack {
                damage: SLAVER_BLUE_A2_STAB_DAMAGE,
            }
        );

        let mut red = monster_state_for_ascension(&SLAVER_RED_A0, MonsterId::new(3), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&red, 17),
            MonsterIntent::Attack {
                damage: SLAVER_RED_A2_STAB_DAMAGE,
            }
        );
        red.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&red, 17),
            MonsterIntent::AttackApplyPlayerVulnerable {
                damage: SLAVER_RED_A2_SCRAPE_DAMAGE,
                vulnerable: SLAVER_RED_A17_VULNERABLE,
            }
        );
    }

    #[test]
    fn fungi_and_slaver_source_scaled_damage_does_not_double_apply_ascension_bonus() {
        let mut fungi = monster_state_for_ascension(&FUNGI_BEAST_A0, MonsterId::new(1), 2);
        fungi.intent = MonsterIntent::Attack {
            damage: FUNGI_BEAST_BITE_DAMAGE,
        };
        let mut player = dummy_player();
        let before = player.clone();
        let mut piles = dummy_piles();
        let damage = apply_monster_intent(&mut fungi, &mut player, &mut piles, 2, &before, &[]);
        assert_eq!(damage, FUNGI_BEAST_BITE_DAMAGE);

        let mut blue = monster_state_for_ascension(&SLAVER_BLUE_A0, MonsterId::new(2), 2);
        blue.intent = MonsterIntent::AttackApplyPlayerWeak {
            damage: SLAVER_BLUE_A2_RAKE_DAMAGE,
            weak: SLAVER_BLUE_WEAK,
        };
        let mut player = dummy_player();
        let before = player.clone();
        let damage = apply_monster_intent(&mut blue, &mut player, &mut piles, 2, &before, &[]);
        assert_eq!(damage, SLAVER_BLUE_A2_RAKE_DAMAGE);

        let mut red = monster_state_for_ascension(&SLAVER_RED_A0, MonsterId::new(3), 2);
        red.intent = MonsterIntent::AttackApplyPlayerVulnerable {
            damage: SLAVER_RED_A2_SCRAPE_DAMAGE,
            vulnerable: SLAVER_RED_VULNERABLE,
        };
        let mut player = dummy_player();
        let before = player.clone();
        let damage = apply_monster_intent(&mut red, &mut player, &mut piles, 2, &before, &[]);
        assert_eq!(damage, SLAVER_RED_A2_SCRAPE_DAMAGE);
    }

    #[test]
    fn taskmaster_definition_is_registered() {
        assert_eq!(content_id_from_game_monster_id("SlaverBoss"), TASKMASTER_ID);
        assert_eq!(content_id_from_game_monster_id("Taskmaster"), TASKMASTER_ID);

        let definition = get_monster_definition(TASKMASTER_ID).expect("taskmaster definition");
        let monster = monster_state(&TASKMASTER_A0, MonsterId::new(1));

        assert_eq!(definition.name, "Taskmaster");
        assert_eq!(definition.hp, 57);
        assert_eq!(
            monster.intent,
            MonsterIntent::AttackAddWoundsToDiscard {
                damage: TASKMASTER_SCOURING_WHIP_DAMAGE,
                count: TASKMASTER_WOUNDS,
            }
        );
    }

    #[test]
    fn taskmaster_scouring_whip_adds_wound_to_discard() {
        let mut monster = monster_state(&TASKMASTER_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(damage, TASKMASTER_SCOURING_WHIP_DAMAGE);
        assert_eq!(piles.discard_pile.len(), 1);
        assert_eq!(
            piles.discard_pile[0].content_id,
            crate::content::cards::WOUND_ID
        );
    }

    #[test]
    fn taskmaster_ascension_scouring_whip_wound_counts_and_a18_strength_are_source_backed() {
        let mut monster = monster_state(&TASKMASTER_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            3,
            &player_before,
            &[],
        );

        assert_eq!(
            damage,
            AscensionConfig::new(3).scaled_attack_damage(TASKMASTER_SCOURING_WHIP_DAMAGE)
        );
        assert_eq!(
            piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == crate::content::cards::WOUND_ID)
                .count(),
            TASKMASTER_A3_WOUNDS as usize
        );
        assert_eq!(monster.powers.strength, 0);

        let mut monster = monster_state(&TASKMASTER_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            18,
            &player_before,
            &[],
        );

        assert_eq!(
            damage,
            AscensionConfig::new(18).scaled_attack_damage(TASKMASTER_SCOURING_WHIP_DAMAGE)
        );
        assert_eq!(
            piles
                .discard_pile
                .iter()
                .filter(|card| card.content_id == crate::content::cards::WOUND_ID)
                .count(),
            TASKMASTER_A18_WOUNDS as usize
        );
        assert_eq!(monster.powers.strength, TASKMASTER_A18_STRENGTH);
    }

    #[test]
    fn gremlin_leader_definition_is_registered() {
        assert_eq!(
            content_id_from_game_monster_id("GremlinLeader"),
            GREMLIN_LEADER_ID
        );
        assert_eq!(
            content_id_from_game_monster_id("Gremlin Leader"),
            GREMLIN_LEADER_ID
        );

        let definition =
            get_monster_definition(GREMLIN_LEADER_ID).expect("gremlin leader definition");

        assert_eq!(definition.name, "Gremlin Leader");
        assert_eq!(definition.hp, 144);
    }

    #[test]
    fn gremlin_leader_partial_sequence_covers_encourage_stab_and_rally() {
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_LEADER_A0, 0, None),
            MonsterIntent::EncourageGremlins {
                strength: GREMLIN_LEADER_STRENGTH,
                block: GREMLIN_LEADER_BLOCK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_LEADER_A0, 1, None),
            MonsterIntent::AttackMultiple {
                damage: GREMLIN_LEADER_STAB_DAMAGE,
                hits: GREMLIN_LEADER_STAB_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_LEADER_A0, 2, None),
            MonsterIntent::SummonGremlins { count: 2 }
        );
    }

    #[test]
    fn gremlin_leader_ascension_encourage_variants_are_source_backed() {
        let leader_a3 = monster_state_for_ascension(&GREMLIN_LEADER_A0, MonsterId::new(10), 3);
        assert_eq!(
            prepare_monster_intent_for_ascension(&leader_a3, 3),
            MonsterIntent::EncourageGremlins {
                strength: GREMLIN_LEADER_A3_STRENGTH,
                block: GREMLIN_LEADER_BLOCK,
            }
        );

        let leader_a18 = monster_state_for_ascension(&GREMLIN_LEADER_A0, MonsterId::new(10), 18);
        assert_eq!(
            prepare_monster_intent_for_ascension(&leader_a18, 18),
            MonsterIntent::EncourageGremlins {
                strength: GREMLIN_LEADER_A18_STRENGTH,
                block: GREMLIN_LEADER_A18_BLOCK,
            }
        );
    }

    #[test]
    fn gremlin_leader_source_scaled_stab_does_not_double_apply_ascension_bonus() {
        let mut leader = monster_state_for_ascension(&GREMLIN_LEADER_A0, MonsterId::new(10), 18);
        leader.intent = MonsterIntent::AttackMultiple {
            damage: GREMLIN_LEADER_STAB_DAMAGE,
            hits: GREMLIN_LEADER_STAB_HITS,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut leader,
            &mut player,
            &mut piles,
            18,
            &player_before,
            &[],
        );

        assert_eq!(
            damage,
            GREMLIN_LEADER_STAB_DAMAGE * GREMLIN_LEADER_STAB_HITS
        );
    }

    #[test]
    fn gremlin_leader_encourage_strengthens_all_and_blocks_non_leaders() {
        let mut monsters = vec![
            monster_state(&GREMLIN_LEADER_A0, MonsterId::new(10)),
            monster_state(&CULTIST_A0, MonsterId::new(11)),
            monster_state(&JAW_WORM_A0, MonsterId::new(12)),
        ];
        monsters[2].alive = false;

        apply_gremlin_leader_encourage(
            &mut monsters,
            MonsterId::new(10),
            GREMLIN_LEADER_STRENGTH,
            GREMLIN_LEADER_BLOCK,
        );

        assert_eq!(monsters[0].powers.strength, GREMLIN_LEADER_STRENGTH);
        assert_eq!(monsters[0].block, 0);
        assert_eq!(monsters[1].powers.strength, GREMLIN_LEADER_STRENGTH);
        assert_eq!(monsters[1].block, GREMLIN_LEADER_BLOCK);
        assert_eq!(monsters[2].powers.strength, 0);
        assert_eq!(monsters[2].block, 0);
    }

    #[test]
    fn gremlin_leader_rally_summons_representative_minions_before_leader() {
        let mut monsters = vec![monster_state(&GREMLIN_LEADER_A0, MonsterId::new(10))];

        apply_gremlin_leader_rally_representative(&mut monsters, 2);

        assert_eq!(monsters.len(), 3);
        assert_eq!(monsters[0].content_id, GREMLIN_WARRIOR_ID);
        assert_eq!(monsters[1].content_id, GREMLIN_WARRIOR_ID);
        assert_eq!(monsters[2].content_id, GREMLIN_LEADER_ID);
        assert_eq!(monsters[0].powers.minion, 1);
        assert_eq!(monsters[1].powers.anger, GREMLIN_WARRIOR_ANGER);
    }

    #[test]
    fn gremlin_leader_rally_caps_at_three_living_minions() {
        let mut monsters = vec![
            monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(1)),
            monster_state(&GREMLIN_THIEF_A0, MonsterId::new(2)),
            monster_state(&GREMLIN_LEADER_A0, MonsterId::new(10)),
        ];

        apply_gremlin_leader_rally_representative(&mut monsters, 2);

        assert_eq!(monsters.len(), 4);
        assert_eq!(
            monsters
                .iter()
                .filter(|monster| is_gremlin_leader_minion_content_id(monster.content_id))
                .count(),
            3
        );
    }

    #[test]
    fn gremlin_leader_death_escape_marks_living_minions_not_alive() {
        let mut monsters = vec![
            monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(1)),
            monster_state(&GREMLIN_THIEF_A0, MonsterId::new(2)),
            monster_state(&GREMLIN_LEADER_A0, MonsterId::new(10)),
            monster_state(&CULTIST_A0, MonsterId::new(11)),
        ];
        monsters[1].alive = false;

        apply_gremlin_leader_death_escape(&mut monsters, MonsterId::new(10));

        assert!(!monsters[0].alive);
        assert!(!monsters[1].alive);
        assert!(monsters[2].alive);
        assert!(monsters[3].alive);
    }

    #[test]
    fn gremlin_leader_minions_are_registered_with_representative_intents() {
        let warrior = monster_state(&GREMLIN_WARRIOR_A0, MonsterId::new(1));
        assert_eq!(
            content_id_from_game_monster_id("GremlinWarrior"),
            GREMLIN_WARRIOR_ID
        );
        assert_eq!(warrior.powers.minion, 1);
        assert_eq!(warrior.powers.anger, GREMLIN_WARRIOR_ANGER);
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_WARRIOR_A0, 0, None),
            MonsterIntent::Attack {
                damage: GREMLIN_WARRIOR_SCRATCH_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_THIEF_A0, 0, None),
            MonsterIntent::Attack {
                damage: GREMLIN_THIEF_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_FAT_A0, 0, None),
            MonsterIntent::AttackApplyPlayerWeak {
                damage: GREMLIN_FAT_DAMAGE,
                weak: GREMLIN_FAT_WEAK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_TSUNDERE_A0, 0, None),
            MonsterIntent::Block {
                block: GREMLIN_TSUNDERE_BLOCK,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&GREMLIN_WIZARD_A0, 2, None),
            MonsterIntent::Attack {
                damage: GREMLIN_WIZARD_MAGIC_DAMAGE,
            }
        );
    }

    #[test]
    fn gremlin_leader_minion_ascension_variants_are_source_backed() {
        let warrior = monster_state_for_ascension(&GREMLIN_WARRIOR_A0, MonsterId::new(1), 17);
        assert_eq!(warrior.powers.anger, GREMLIN_WARRIOR_A17_ANGER);
        assert_eq!(
            prepare_monster_intent_for_ascension(&warrior, 17),
            MonsterIntent::Attack {
                damage: GREMLIN_WARRIOR_A2_SCRATCH_DAMAGE,
            }
        );

        let thief = monster_state_for_ascension(&GREMLIN_THIEF_A0, MonsterId::new(2), 2);
        assert_eq!(
            prepare_monster_intent_for_ascension(&thief, 2),
            MonsterIntent::Attack {
                damage: GREMLIN_THIEF_A2_DAMAGE,
            }
        );

        let fat = monster_state_for_ascension(&GREMLIN_FAT_A0, MonsterId::new(3), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&fat, 17),
            MonsterIntent::AttackApplyPlayerFrailAndWeak {
                damage: GREMLIN_FAT_A2_DAMAGE,
                frail: GREMLIN_FAT_WEAK,
                weak: GREMLIN_FAT_WEAK,
            }
        );

        let mut tsundere = monster_state_for_ascension(&GREMLIN_TSUNDERE_A0, MonsterId::new(4), 17);
        assert_eq!(
            prepare_monster_intent_for_ascension(&tsundere, 17),
            MonsterIntent::Block {
                block: GREMLIN_TSUNDERE_A17_BLOCK,
            }
        );
        tsundere.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&tsundere, 17),
            MonsterIntent::Attack {
                damage: GREMLIN_TSUNDERE_A2_BASH_DAMAGE,
            }
        );

        let mut wizard = monster_state_for_ascension(&GREMLIN_WIZARD_A0, MonsterId::new(5), 2);
        wizard.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&wizard, 2),
            MonsterIntent::Attack {
                damage: GREMLIN_WIZARD_A2_MAGIC_DAMAGE,
            }
        );
    }

    #[test]
    fn gremlin_leader_minion_source_scaled_attacks_do_not_double_apply_ascension_bonus() {
        let mut fat = monster_state_for_ascension(&GREMLIN_FAT_A0, MonsterId::new(3), 17);
        fat.intent = MonsterIntent::AttackApplyPlayerFrailAndWeak {
            damage: GREMLIN_FAT_A2_DAMAGE,
            frail: GREMLIN_FAT_WEAK,
            weak: GREMLIN_FAT_WEAK,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage =
            apply_monster_intent(&mut fat, &mut player, &mut piles, 17, &player_before, &[]);

        assert_eq!(damage, GREMLIN_FAT_A2_DAMAGE);
        assert_eq!(player.powers.frail, GREMLIN_FAT_WEAK);
        assert_eq!(player.powers.weak, GREMLIN_FAT_WEAK);

        let mut wizard = monster_state_for_ascension(&GREMLIN_WIZARD_A0, MonsterId::new(5), 2);
        wizard.intent = MonsterIntent::Attack {
            damage: GREMLIN_WIZARD_A2_MAGIC_DAMAGE,
        };
        let mut player = dummy_player();
        let player_before = player.clone();

        let damage =
            apply_monster_intent(&mut wizard, &mut player, &mut piles, 2, &player_before, &[]);

        assert_eq!(damage, GREMLIN_WIZARD_A2_MAGIC_DAMAGE);
    }

    #[test]
    fn spheric_guardian_initial_state_applies_source_backed_prebattle_powers() {
        let monster = monster_state(&SPHERIC_GUARDIAN_A0, MonsterId::new(1));

        assert_eq!(monster.hp, 20);
        assert_eq!(monster.block, SPHERIC_GUARDIAN_STARTING_BLOCK);
        assert_eq!(monster.powers.artifact, SPHERIC_GUARDIAN_ARTIFACT);
        assert_eq!(
            monster.intent,
            MonsterIntent::Block {
                block: SPHERIC_GUARDIAN_ACTIVATE_BLOCK
            }
        );
    }

    #[test]
    fn bronze_automaton_initial_state_applies_artifact() {
        let monster = monster_state(&BRONZE_AUTOMATON_A0, MonsterId::new(1));

        assert_eq!(monster.powers.artifact, BRONZE_AUTOMATON_ARTIFACT);
    }

    #[test]
    fn spheric_guardian_source_sequence_starts_with_harden_then_frail_attack() {
        let mut monster = monster_state(&SPHERIC_GUARDIAN_A0, MonsterId::new(1));
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );
        monster.intent =
            prepare_monster_intent_for(&SPHERIC_GUARDIAN_A0, monster.moves_executed, None);

        assert_eq!(damage, 0);
        assert_eq!(
            monster.block,
            SPHERIC_GUARDIAN_STARTING_BLOCK + SPHERIC_GUARDIAN_ACTIVATE_BLOCK
        );
        assert_eq!(
            monster.intent,
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SPHERIC_GUARDIAN_DAMAGE,
                frail: SPHERIC_GUARDIAN_FRAIL
            }
        );
    }

    #[test]
    fn spheric_guardian_frail_attack_applies_frail_and_then_cycles_attacks() {
        let mut monster = monster_state(&SPHERIC_GUARDIAN_A0, MonsterId::new(1));
        monster.moves_executed = 1;
        monster.intent = prepare_monster_intent_for(&SPHERIC_GUARDIAN_A0, 1, None);
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );
        let double_attack = prepare_monster_intent_for(&SPHERIC_GUARDIAN_A0, 2, None);
        let attack_and_block = prepare_monster_intent_for(&SPHERIC_GUARDIAN_A0, 3, None);

        assert_eq!(damage, SPHERIC_GUARDIAN_DAMAGE);
        assert_eq!(player.powers.frail, SPHERIC_GUARDIAN_FRAIL);
        assert_eq!(
            double_attack,
            MonsterIntent::AttackMultiple {
                damage: SPHERIC_GUARDIAN_DAMAGE,
                hits: SPHERIC_GUARDIAN_SLAM_HITS
            }
        );
        assert_eq!(
            attack_and_block,
            MonsterIntent::AttackAndBlock {
                damage: SPHERIC_GUARDIAN_DAMAGE,
                block: SPHERIC_GUARDIAN_HARDEN_BLOCK
            }
        );
    }

    #[test]
    fn spheric_guardian_ascension_variants_use_source_backed_block_and_damage() {
        let mut monster = monster_state_for_ascension(&SPHERIC_GUARDIAN_A0, MonsterId::new(1), 17);

        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 17),
            MonsterIntent::Block {
                block: SPHERIC_GUARDIAN_A17_ACTIVATE_BLOCK,
            }
        );
        monster.moves_executed = 1;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 17),
            MonsterIntent::AttackApplyPlayerFrail {
                damage: SPHERIC_GUARDIAN_A2_DAMAGE,
                frail: SPHERIC_GUARDIAN_FRAIL,
            }
        );
        monster.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 17),
            MonsterIntent::AttackMultiple {
                damage: SPHERIC_GUARDIAN_A2_DAMAGE,
                hits: SPHERIC_GUARDIAN_SLAM_HITS,
            }
        );
        monster.moves_executed = 3;
        assert_eq!(
            prepare_monster_intent_for_ascension(&monster, 17),
            MonsterIntent::AttackAndBlock {
                damage: SPHERIC_GUARDIAN_A2_DAMAGE,
                block: SPHERIC_GUARDIAN_HARDEN_BLOCK,
            }
        );
    }

    #[test]
    fn spheric_guardian_source_scaled_damage_does_not_double_apply_ascension_bonus() {
        let mut monster = monster_state_for_ascension(&SPHERIC_GUARDIAN_A0, MonsterId::new(1), 2);
        monster.intent = MonsterIntent::AttackApplyPlayerFrail {
            damage: SPHERIC_GUARDIAN_A2_DAMAGE,
            frail: SPHERIC_GUARDIAN_FRAIL,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        let damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            2,
            &player_before,
            &[],
        );

        assert_eq!(damage, SPHERIC_GUARDIAN_A2_DAMAGE);
        assert_eq!(player.powers.frail, SPHERIC_GUARDIAN_FRAIL);
    }

    #[test]
    fn lagavulin_has_one_hundred_nine_hp_and_starts_asleep() {
        let monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));

        assert_eq!(LAGAVULIN_A0.hp, 109);
        assert_eq!(monster.sleep_turns_remaining, LAGAVULIN_SLEEP_TURNS);
        assert_eq!(monster.intent, MonsterIntent::Sleep);
    }

    #[test]
    fn lagavulin_intent_progresses_sleep_siphon_attack() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        assert_eq!(monster.intent, MonsterIntent::Sleep);

        monster.sleep_turns_remaining = 0;
        monster.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: LAGAVULIN_ATTACK_DAMAGE
            }
        );

        monster.moves_executed = 3;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: LAGAVULIN_ATTACK_DAMAGE
            }
        );

        monster.moves_executed = 4;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::SiphonPlayer {
                strength: LAGAVULIN_SIPHON_STRENGTH,
                dexterity: LAGAVULIN_SIPHON_DEXTERITY,
            }
        );
    }

    #[test]
    fn lagavulin_sleep_decrements_remaining_turns() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Sleep;

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.sleep_turns_remaining, 2);
    }

    #[test]
    fn lagavulin_siphon_reduces_player_strength_and_dexterity() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        monster.sleep_turns_remaining = 0;
        monster.intent = MonsterIntent::SiphonPlayer {
            strength: LAGAVULIN_SIPHON_STRENGTH,
            dexterity: LAGAVULIN_SIPHON_DEXTERITY,
        };
        let mut player = dummy_player();
        player.powers.strength = 3;
        player.powers.dexterity = 2;
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            apply_monster_intent(
                &mut monster,
                &mut player,
                &mut piles,
                0,
                &player_before,
                &[]
            ),
            0
        );
        assert_eq!(player.powers.strength, 2);
        assert_eq!(player.powers.dexterity, 1);
        assert!(monster.has_siphoned);
    }

    #[test]
    fn lagavulin_wake_on_damage_clears_sleep_and_sets_siphon_intent() {
        let mut monster = monster_state(&LAGAVULIN_A0, MonsterId::new(1));
        monster.block = 8;
        monster.powers.metallicize = 8;

        wake_lagavulin_on_damage(&mut monster, 1);

        assert_eq!(monster.sleep_turns_remaining, 0);
        assert_eq!(monster.block, 0);
        assert_eq!(monster.powers.metallicize, 0);
        assert_eq!(monster.intent, MonsterIntent::Stun);
        assert!(!monster.has_siphoned);
        monster.moves_executed = 2;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: LAGAVULIN_ATTACK_DAMAGE
            }
        );
    }

    #[test]
    fn sentry_has_forty_hp_and_starts_with_beam_intent() {
        let monster = monster_state(&SENTRY_A0, MonsterId::new(1));

        assert_eq!(SENTRY_A0.hp, 40);
        assert_eq!(
            monster.intent,
            MonsterIntent::AddDazedToDiscard {
                count: SENTRY_BEAM_DAZED
            }
        );
    }

    #[test]
    fn sentry_move_selection_alternates_beam_and_attack() {
        let definition = &SENTRY_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::AddDazedToDiscard {
                count: SENTRY_BEAM_DAZED
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::Attack {
                damage: SENTRY_ATTACK_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
            MonsterIntent::AddDazedToDiscard {
                count: SENTRY_BEAM_DAZED
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 3, None),
            MonsterIntent::Attack {
                damage: SENTRY_ATTACK_DAMAGE
            }
        );
    }

    #[test]
    fn orb_walker_claw_adds_burn_to_discard() {
        assert_eq!(
            prepare_monster_intent_for(&ORB_WALKER_A0, 0, None),
            MonsterIntent::Attack {
                damage: ORB_WALKER_LASER_DAMAGE
            }
        );
        assert_eq!(
            prepare_monster_intent_for(&ORB_WALKER_A0, 1, None),
            MonsterIntent::AddBurnToDiscardAndDraw {
                count: 1,
                damage: ORB_WALKER_CLAW_DAMAGE
            }
        );
    }

    #[test]
    fn orb_walker_strength_up_power_increases_later_attacks() {
        let mut monster = monster_state(&ORB_WALKER_A0, MonsterId::new(1));
        monster.powers.weak = 6;
        monster.powers.strength_up = 3;
        monster.intent = MonsterIntent::Attack {
            damage: ORB_WALKER_LASER_DAMAGE,
        };
        let mut player = dummy_player();
        let mut piles = dummy_piles();
        let player_before = player.clone();

        let first_damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(first_damage, 11);
        assert_eq!(monster.powers.strength, 3);

        monster.intent = MonsterIntent::AddBurnToDiscardAndDraw {
            count: 1,
            damage: ORB_WALKER_CLAW_DAMAGE,
        };
        let player_before = player.clone();
        let second_damage = apply_monster_intent(
            &mut monster,
            &mut player,
            &mut piles,
            0,
            &player_before,
            &[],
        );

        assert_eq!(second_damage, 9);
        assert_eq!(monster.powers.strength, 6);
        assert!(piles.discard_pile.is_empty());
        assert!(piles.draw_pile.is_empty());
    }

    #[test]
    fn sentry_beam_adds_dazed_to_discard() {
        let mut monster = monster_state(&SENTRY_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AddDazedToDiscard {
            count: SENTRY_BEAM_DAZED,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            apply_monster_intent(
                &mut monster,
                &mut player,
                &mut piles,
                0,
                &player_before,
                &[]
            ),
            0
        );
        assert_eq!(piles.discard_pile.len(), 2);
        assert!(piles
            .discard_pile
            .iter()
            .all(|card| card.content_id == DAZED_ID));
    }

    #[test]
    fn hexaghost_has_two_hundred_fifty_hp_and_starts_with_activate() {
        let monster = monster_state(&HEXAGHOST_A0, MonsterId::new(1));

        assert_eq!(HEXAGHOST_A0.hp, 250);
        assert_eq!(monster.intent, MonsterIntent::Stun);
    }

    #[test]
    fn hexaghost_move_selection_activates_then_cycles_divider_sear_inferno() {
        let definition = &HEXAGHOST_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Stun
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::AttackMultiple {
                damage: HEXAGHOST_DIVIDER_DAMAGE,
                hits: HEXAGHOST_DIVIDER_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
            MonsterIntent::AddBurnToDiscard {
                count: HEXAGHOST_SEAR_BURNS,
                damage: HEXAGHOST_DIVIDER_DAMAGE,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 3, None),
            MonsterIntent::AttackMultiple {
                damage: HEXAGHOST_TACKLE_DAMAGE,
                hits: HEXAGHOST_TACKLE_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 5, None),
            MonsterIntent::Stun
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 6, None),
            MonsterIntent::AddBurnToDiscard {
                count: HEXAGHOST_INFERNO_BURNS,
                damage: HEXAGHOST_INFERNO_DAMAGE,
            }
        );
    }

    #[test]
    fn hexaghost_divider_deals_damage_twice() {
        let mut monster = monster_state(&HEXAGHOST_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AttackMultiple {
            damage: HEXAGHOST_DIVIDER_DAMAGE,
            hits: HEXAGHOST_DIVIDER_HITS,
        };

        assert_eq!(apply_intent(&mut monster), 12);
    }

    #[test]
    fn hexaghost_inferno_adds_burns_and_deals_damage() {
        let mut monster = monster_state(&HEXAGHOST_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::AddBurnToDiscard {
            count: HEXAGHOST_INFERNO_BURNS,
            damage: HEXAGHOST_INFERNO_DAMAGE,
        };
        let mut player = dummy_player();
        let player_before = player.clone();
        let mut piles = dummy_piles();

        assert_eq!(
            apply_monster_intent(
                &mut monster,
                &mut player,
                &mut piles,
                0,
                &player_before,
                &[]
            ),
            2
        );
        assert_eq!(piles.discard_pile.len(), 3);
        assert!(piles
            .discard_pile
            .iter()
            .all(|card| card.content_id == BURN_ID));
    }

    #[test]
    fn slime_boss_has_one_hundred_forty_hp_and_slam_intent() {
        let monster = monster_state(&SLIME_BOSS_A0, MonsterId::new(1));

        assert_eq!(SLIME_BOSS_A0.hp, 140);
        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: SLIME_BOSS_SLAM_DAMAGE
            }
        );
    }

    #[test]
    fn guardian_has_two_hundred_forty_hp_and_mode_shift_charge_up_intent() {
        let monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));

        assert_eq!(GUARDIAN_A0.hp, 240);
        assert_eq!(monster.mode_shift, GUARDIAN_MODE_SHIFT_START);
        assert!(!monster.in_defensive_mode);
        assert_eq!(
            monster.intent,
            MonsterIntent::Block {
                block: GUARDIAN_CHARGE_BLOCK
            }
        );
    }

    #[test]
    fn guardian_charge_up_gains_nine_block_then_fierce_bash() {
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.block, GUARDIAN_CHARGE_BLOCK);
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: GUARDIAN_FIERCE_BASH_DAMAGE
            }
        );
    }

    #[test]
    fn guardian_mode_shift_triggers_defensive_mode() {
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        guardian_on_hp_damage(&mut monster, 30);

        assert!(monster.in_defensive_mode);
        assert_eq!(monster.block, GUARDIAN_DEFENSIVE_BLOCK);
        assert_eq!(monster.powers.spikes, 0);
        assert_eq!(
            monster.intent,
            MonsterIntent::GuardianCloseUp {
                sharp_hide: GUARDIAN_DEFENSIVE_SPIKES
            }
        );
    }

    #[test]
    fn guardian_close_up_applies_sharp_hide_then_roll_attack() {
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        guardian_on_hp_damage(&mut monster, 30);
        monster.block = 0;

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.powers.spikes, GUARDIAN_DEFENSIVE_SPIKES);
        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: GUARDIAN_DEFENSIVE_ATTACK_DAMAGE
            }
        );
    }
}
