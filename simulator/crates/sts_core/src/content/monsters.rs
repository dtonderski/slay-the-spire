use crate::{
    combat::piles::add_cards_to_discard,
    combat::turn_powers::monster_attack_damage,
    combat::{CardPiles, MonsterIntent, MonsterState},
    content::ascension::AscensionConfig,
    content::cards::{BURN_ID, DAZED_ID},
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

const RED_LOUSE_CURL_BLOCK: i32 = 3;
const RED_LOUSE_BITE_DAMAGE: i32 = 6;

const GREEN_LOUSE_CURL_BLOCK: i32 = 3;
const GREEN_LOUSE_BITE_DAMAGE: i32 = 6;
const GREEN_LOUSE_SPIKES: i32 = 3;

const SPIKE_SLIME_LICK_WEAK: i32 = 1;
const SPIKE_SLIME_S_SPIT_DAMAGE: i32 = 5;

const ACID_SLIME_ATTACK_DAMAGE: i32 = 7;
const ACID_SLIME_WEAK: i32 = 1;

const LAGAVULIN_SLEEP_TURNS: u32 = 3;
const LAGAVULIN_SIPHON_STRENGTH: i32 = 2;
const LAGAVULIN_SIPHON_DEXTERITY: i32 = 2;
const LAGAVULIN_ATTACK_DAMAGE: i32 = 18;

const SENTRY_BEAM_DAZED: i32 = 2;
const SENTRY_ATTACK_DAMAGE: i32 = 6;

const HEXAGHOST_DIVIDER_DAMAGE: i32 = 6;
const HEXAGHOST_DIVIDER_HITS: i32 = 2;
const HEXAGHOST_TACKLE_DAMAGE: i32 = 5;
const HEXAGHOST_TACKLE_HITS: i32 = 6;
const HEXAGHOST_INFERNO_BURNS: i32 = 3;
const HEXAGHOST_INFERNO_DAMAGE: i32 = 2;

const SLIME_BOSS_SLAM_DAMAGE: i32 = 35;
const SLIME_BOSS_SPLIT_HP_THRESHOLD: i32 = 70;

const GUARDIAN_MODE_SHIFT_START: i32 = 30;
const GUARDIAN_MODE_SHIFT_RESET: i32 = 40;
const GUARDIAN_DEFENSIVE_SEQUENCE_TURNS: u32 = 7;
const GUARDIAN_DEFENSIVE_BLOCK: i32 = 20;
const GUARDIAN_DEFENSIVE_SPIKES: i32 = 3;
const GUARDIAN_CHARGE_DAMAGE: i32 = 32;
const GUARDIAN_NORMAL_ATTACK_DAMAGE: i32 = 5;
const GUARDIAN_DEFENSIVE_ATTACK_DAMAGE: i32 = 9;
const GUARDIAN_DEFENSIVE_COMBO_DAMAGE: i32 = 8;

const GREMLIN_NOB_BITE_DAMAGE: i32 = 6;
const GREMLIN_NOB_SKULL_BASH_DAMAGE: i32 = 14;

const JAW_WORM_CHOMP_DAMAGE: i32 = 11;
const JAW_WORM_THRASH_DAMAGE: i32 = 7;
const JAW_WORM_THRASH_BLOCK: i32 = 5;
const JAW_WORM_BELLOW_STRENGTH: i32 = 3;
const JAW_WORM_BELLOW_BLOCK: i32 = 6;

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
pub enum SmallSlimesVariant {
    SpikeSmallAcidMedium,
    AcidSmallSpikeMedium,
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
pub const LOUSE_NORMAL_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(10, 15);
pub const LOUSE_NORMAL_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(11, 16);
pub const LOUSE_DEFENSIVE_A0_HP_RANGE: MonsterHpRange = MonsterHpRange::new(11, 17);
pub const LOUSE_DEFENSIVE_A7_HP_RANGE: MonsterHpRange = MonsterHpRange::new(12, 18);
pub const LOUSE_A0_BITE_DAMAGE_RANGE: MonsterHpRange = MonsterHpRange::new(5, 7);
pub const LOUSE_A2_BITE_DAMAGE_RANGE: MonsterHpRange = MonsterHpRange::new(6, 8);
pub const LOUSE_A0_CURL_UP_RANGE: MonsterHpRange = MonsterHpRange::new(3, 7);
pub const LOUSE_A7_CURL_UP_RANGE: MonsterHpRange = MonsterHpRange::new(4, 8);

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

/// Act 1 Cultist at ascension 0, simplified: 50 HP, Ritual 2 on first turn, then 6-damage attacks.
pub const CULTIST_A0: MonsterDefinition = MonsterDefinition {
    content_id: CULTIST_ID,
    name: "Cultist",
    hp: 50,
    attack_damage: 6,
    ritual_amount: 2,
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

/// Act 1 Lagavulin at ascension 0: 109 HP, sleeps 3 turns then siphons and attacks.
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

/// Act 1 Sentry at ascension 0: 40 HP, Beam / Attack / Attack cycle.
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
pub fn target_small_slimes_variant(seed: i64, floor_num: u32) -> SmallSlimesVariant {
    let mut misc_rng = StsRng::new(seed + i64::from(floor_num));
    if misc_rng.random_bool() {
        SmallSlimesVariant::SpikeSmallAcidMedium
    } else {
        SmallSlimesVariant::AcidSmallSpikeMedium
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
            let mut spawn = target_combat_entry_spawn("Louse", max_hp, neow_lament, Vec::new());
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
        "2 Louse" => target_two_louse_spawn_states(seed, floor_num, ascension, neow_lament),
        _ => Vec::new(),
    }
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
        "JawWorm" => JAW_WORM_ID,
        "GremlinNob" => GREMLIN_NOB_ID,
        "Lagavulin" => LAGAVULIN_ID,
        "Sentry" => SENTRY_ID,
        "Hexaghost" => HEXAGHOST_ID,
        "SlimeBoss" => SLIME_BOSS_ID,
        "TheGuardian" => GUARDIAN_ID,
        "SpikeSlime_S" | "SpikeSlime_M" | "Spike Slime (S)" | "Spike Slime (M)" => SPIKE_SLIME_ID,
        "AcidSlime_S" | "AcidSlime_M" | "Acid Slime (S)" | "Acid Slime (M)" => ACID_SLIME_ID,
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
        _ => None,
    }
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
        hp: config.scaled_enemy_hp(definition.hp),
        block: 0,
        alive: true,
        powers: MonsterPowers {
            spikes: definition.starting_spikes,
            ..MonsterPowers::default()
        },
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
        intent: prepare_monster_intent_for_monster(
            definition,
            0,
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
    let definition = get_monster_definition(monster.content_id).unwrap_or(&FIXED_SIMPLE_MONSTER);
    let mut intent = prepare_monster_intent_for_monster(
        definition,
        monster.moves_executed,
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
    intent
}

#[must_use]
fn prepare_monster_intent_for_monster(
    definition: &MonsterDefinition,
    moves_executed: u32,
    sleep_turns_remaining: u32,
    has_siphoned: bool,
    defensive_turns_remaining: u32,
    in_defensive_mode: bool,
    mode_shift: i32,
    rolled_attack_damage: Option<i32>,
) -> MonsterIntent {
    if definition.content_id == LAGAVULIN_ID {
        return lagavulin_intent(sleep_turns_remaining, has_siphoned);
    }
    if definition.content_id == GUARDIAN_ID {
        return guardian_intent(in_defensive_mode, defensive_turns_remaining, moves_executed);
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
        SPIKE_SLIME_ID => spike_slime_s_intent(moves_executed),
        ACID_SLIME_ID => acid_slime_intent(moves_executed),
        SENTRY_ID => sentry_intent(moves_executed),
        HEXAGHOST_ID => hexaghost_intent(moves_executed),
        SLIME_BOSS_ID => slime_boss_intent(),
        _ => MonsterIntent::Attack {
            damage: definition.attack_damage,
        },
    }
}

/// Deterministic Red Louse move cycle: Curl → Bite, keyed on `moves_executed`.
#[must_use]
fn red_louse_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Block {
            block: RED_LOUSE_CURL_BLOCK,
        },
        _ => MonsterIntent::Attack {
            damage: rolled_attack_damage.unwrap_or(RED_LOUSE_BITE_DAMAGE),
        },
    }
}

#[must_use]
fn green_louse_intent(moves_executed: u32, rolled_attack_damage: Option<i32>) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Block {
            block: GREEN_LOUSE_CURL_BLOCK,
        },
        _ => MonsterIntent::Attack {
            damage: rolled_attack_damage.unwrap_or(GREEN_LOUSE_BITE_DAMAGE),
        },
    }
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
fn sentry_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 3 {
        0 => MonsterIntent::AddDazedToDiscard {
            count: SENTRY_BEAM_DAZED,
        },
        _ => MonsterIntent::Attack {
            damage: SENTRY_ATTACK_DAMAGE,
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
        if turn_in_sequence < 3 {
            MonsterIntent::Attack {
                damage: GUARDIAN_DEFENSIVE_ATTACK_DAMAGE,
            }
        } else {
            MonsterIntent::Attack {
                damage: GUARDIAN_DEFENSIVE_COMBO_DAMAGE,
            }
        }
    } else {
        match moves_executed % 2 {
            0 => MonsterIntent::Attack {
                damage: GUARDIAN_CHARGE_DAMAGE,
            },
            _ => MonsterIntent::Attack {
                damage: GUARDIAN_NORMAL_ATTACK_DAMAGE,
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
    monster.powers.spikes = GUARDIAN_DEFENSIVE_SPIKES;
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
    match moves_executed % 3 {
        0 => MonsterIntent::AttackMultiple {
            damage: HEXAGHOST_DIVIDER_DAMAGE,
            hits: HEXAGHOST_DIVIDER_HITS,
        },
        1 => MonsterIntent::AttackMultiple {
            damage: HEXAGHOST_TACKLE_DAMAGE,
            hits: HEXAGHOST_TACKLE_HITS,
        },
        _ => MonsterIntent::AddBurnToDiscard {
            count: HEXAGHOST_INFERNO_BURNS,
            damage: HEXAGHOST_INFERNO_DAMAGE,
        },
    }
}

#[must_use]
fn lagavulin_intent(sleep_turns_remaining: u32, has_siphoned: bool) -> MonsterIntent {
    if sleep_turns_remaining > 0 {
        MonsterIntent::Sleep
    } else if !has_siphoned {
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
            monster.has_siphoned = true;
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

/// Gremlin Nob move cycle: Bite → Skull Bash (vulnerable), keyed on `moves_executed`.
#[must_use]
fn gremlin_nob_intent(moves_executed: u32) -> MonsterIntent {
    match moves_executed % 2 {
        0 => MonsterIntent::Attack {
            damage: GREMLIN_NOB_BITE_DAMAGE,
        },
        _ => MonsterIntent::AttackApplyPlayerVulnerable {
            damage: GREMLIN_NOB_SKULL_BASH_DAMAGE,
            vulnerable: 2,
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

/// Execute the monster's current intent and return player damage dealt this turn.
pub fn apply_monster_intent(
    monster: &mut MonsterState,
    player: &mut crate::PlayerState,
    piles: &mut CardPiles,
    ascension: u8,
    player_before: &crate::PlayerState,
    relics: &[crate::Relic],
) -> i32 {
    use crate::combat::damage::deal_unmodified_damage_to_monster;
    use crate::combat::turn_powers::monster_damage_to_player;
    use crate::power::{apply_player_vulnerable, reduce_player_dexterity, reduce_player_strength};

    let config = AscensionConfig::new(ascension);
    let scale_damage = |damage: i32| config.scaled_attack_damage(damage);
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
        MonsterIntent::ApplyPlayerWeak { amount } => {
            crate::relic::apply_player_weak_with_relics(&mut player.powers, relics, amount);
            (0, 0)
        }
        MonsterIntent::AttackApplyPlayerVulnerable { damage, vulnerable } => {
            apply_player_vulnerable(&mut player.powers, vulnerable);
            (
                monster_damage_to_player(player_before, monster, scale_damage(damage)),
                1,
            )
        }
        MonsterIntent::Sleep => {
            if monster.sleep_turns_remaining > 0 {
                monster.sleep_turns_remaining -= 1;
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
        MonsterIntent::AttackMultiple { damage, hits } => {
            let hit_damage = monster_attack_damage(monster, scale_damage(damage));
            (hit_damage * hits, hits)
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
    if monster.content_id == GUARDIAN_ID && monster.in_defensive_mode {
        finish_guardian_defensive_turn(monster);
    }
    monster.moves_executed += 1;
    damage
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
                    name: "Louse",
                    hp: 13,
                },
                TargetMonsterHp {
                    name: "Louse",
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
            MonsterIntent::Ritual { amount: 2 }
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
            MonsterIntent::Ritual { amount: 2 }
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
        assert_eq!(monster.powers.ritual, 2);
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
    fn gremlin_nob_starts_with_bite_intent() {
        let monster = monster_state(&GREMLIN_NOB_A0, MonsterId::new(1));

        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_BITE_DAMAGE
            }
        );
    }

    #[test]
    fn gremlin_nob_move_selection_cycles_bite_skull_bash() {
        let definition = &GREMLIN_NOB_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Attack {
                damage: GREMLIN_NOB_BITE_DAMAGE
            }
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
                damage: GREMLIN_NOB_BITE_DAMAGE
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
            MonsterIntent::Block {
                block: RED_LOUSE_CURL_BLOCK
            }
        );
    }

    #[test]
    fn red_louse_move_selection_cycles_curl_bite() {
        let definition = &RED_LOUSE_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::Block {
                block: RED_LOUSE_CURL_BLOCK
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
            MonsterIntent::Block {
                block: RED_LOUSE_CURL_BLOCK
            }
        );
    }

    #[test]
    fn red_louse_curl_gains_block() {
        let mut monster = monster_state(&RED_LOUSE_A0, MonsterId::new(1));
        monster.intent = MonsterIntent::Block {
            block: RED_LOUSE_CURL_BLOCK,
        };

        assert_eq!(apply_intent(&mut monster), 0);
        assert_eq!(monster.block, 3);
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
            MonsterIntent::Block {
                block: GREEN_LOUSE_CURL_BLOCK
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
        assert_eq!(content_id_from_game_monster_id("Lagavulin"), LAGAVULIN_ID);
        assert_eq!(
            content_id_from_game_monster_id("GremlinNob"),
            GREMLIN_NOB_ID
        );
        assert_eq!(content_id_from_game_monster_id("TheGuardian"), GUARDIAN_ID);
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
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::SiphonPlayer {
                strength: LAGAVULIN_SIPHON_STRENGTH,
                dexterity: LAGAVULIN_SIPHON_DEXTERITY,
            }
        );

        monster.has_siphoned = true;
        assert_eq!(
            prepare_monster_intent(&monster),
            MonsterIntent::Attack {
                damage: LAGAVULIN_ATTACK_DAMAGE
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
        assert_eq!(player.powers.strength, 1);
        assert_eq!(player.powers.dexterity, 0);
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
    fn sentry_move_selection_cycles_beam_attack_attack() {
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
            MonsterIntent::Attack {
                damage: SENTRY_ATTACK_DAMAGE
            }
        );
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
    fn hexaghost_has_two_hundred_fifty_hp_and_starts_with_divider() {
        let monster = monster_state(&HEXAGHOST_A0, MonsterId::new(1));

        assert_eq!(HEXAGHOST_A0.hp, 250);
        assert_eq!(
            monster.intent,
            MonsterIntent::AttackMultiple {
                damage: HEXAGHOST_DIVIDER_DAMAGE,
                hits: HEXAGHOST_DIVIDER_HITS,
            }
        );
    }

    #[test]
    fn hexaghost_move_selection_cycles_divider_tackle_inferno() {
        let definition = &HEXAGHOST_A0;

        assert_eq!(
            prepare_monster_intent_for(definition, 0, None),
            MonsterIntent::AttackMultiple {
                damage: HEXAGHOST_DIVIDER_DAMAGE,
                hits: HEXAGHOST_DIVIDER_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 1, None),
            MonsterIntent::AttackMultiple {
                damage: HEXAGHOST_TACKLE_DAMAGE,
                hits: HEXAGHOST_TACKLE_HITS,
            }
        );
        assert_eq!(
            prepare_monster_intent_for(definition, 2, None),
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
    fn guardian_has_two_hundred_forty_hp_and_mode_shift_charge_intent() {
        let monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));

        assert_eq!(GUARDIAN_A0.hp, 240);
        assert_eq!(monster.mode_shift, GUARDIAN_MODE_SHIFT_START);
        assert!(!monster.in_defensive_mode);
        assert_eq!(
            monster.intent,
            MonsterIntent::Attack {
                damage: GUARDIAN_CHARGE_DAMAGE
            }
        );
    }

    #[test]
    fn guardian_mode_shift_triggers_defensive_mode() {
        let mut monster = monster_state(&GUARDIAN_A0, MonsterId::new(1));
        guardian_on_hp_damage(&mut monster, 30);

        assert!(monster.in_defensive_mode);
        assert_eq!(monster.powers.spikes, GUARDIAN_DEFENSIVE_SPIKES);
        assert_eq!(monster.block, GUARDIAN_DEFENSIVE_BLOCK);
    }
}
